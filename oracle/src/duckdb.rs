//! DuckDB, through its C API (`libduckdb-sys`, built from DuckDB's sources). A case whose
//! `dialect` file says `duckdb` runs in a fresh DuckDB file. Its host answers the engine as the
//! loader's does through `@duckdb/node-api` (src/duckdb.ts): a describe is the prepared
//! statement's types, and a catalog query's rows are its values, each as its text (a list as
//! JSON text, which the engine's queries ask for).
//!
//! The soundness half seeds every table of the case in a database of its own per state, the
//! seeded rows committed, and runs each statement once per assignment of its parameters in a
//! transaction that is rolled back, since DuckDB has no savepoint to undo a failed run within one.
//! A parameter is bound as its text, which DuckDB casts to the parameter's type. DuckDB says each
//! column's type exactly, so a run checks what the engine says of NULL and of the row count: a
//! column typed non-null holds no NULL, and a statement proven `one` returns one row and `maybeOne`
//! at most one. A run is stopped at the timeout by `duckdb_interrupt` from a watchdog thread.

use crate::seed::{self, Report, Scenario, Tally, SCENARIOS};
use libduckdb_sys as ffi;
use reylo_engine::{AnswerCache, DbError};
use serde_json::{json, Map, Value};
use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// An open DuckDB database and one connection to it.
struct Db {
    db: ffi::duckdb_database,
    conn: ffi::duckdb_connection,
}

/// The connection, sent to the watchdog thread that interrupts a run: `duckdb_interrupt` may be
/// called from any thread.
#[derive(Clone, Copy)]
struct Interrupt(ffi::duckdb_connection);
unsafe impl Send for Interrupt {}
unsafe impl Sync for Interrupt {}

/// A query's outcome: its rows, the time it ran out of, or DuckDB's error.
enum Ran<T> {
    Done(T),
    TimedOut,
    Failed(String),
}

fn text(p: *const std::os::raw::c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

/// A string DuckDB allocated, read and freed.
fn owned(p: *mut std::os::raw::c_char) -> String {
    let s = text(p);
    if !p.is_null() {
        unsafe { ffi::duckdb_free(p.cast()) };
    }
    s
}

impl Db {
    fn open(path: &Path) -> Result<Db, String> {
        let c = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        let mut db: ffi::duckdb_database = std::ptr::null_mut();
        let mut conn: ffi::duckdb_connection = std::ptr::null_mut();
        unsafe {
            if ffi::duckdb_open(c.as_ptr(), &mut db) != ffi::duckdb_state_DuckDBSuccess {
                return Err(format!("cannot open {}", path.display()));
            }
            if ffi::duckdb_connect(db, &mut conn) != ffi::duckdb_state_DuckDBSuccess {
                ffi::duckdb_close(&mut db);
                return Err("cannot connect".into());
            }
        }
        Ok(Db { db, conn })
    }

    /// Runs `sql` and reads its result with `read`, or DuckDB's error.
    fn query<T>(&self, sql: &str, read: impl FnOnce(&mut ffi::duckdb_result) -> T) -> Result<T, String> {
        let c = CString::new(sql).map_err(|e| e.to_string())?;
        unsafe {
            let mut result: ffi::duckdb_result = std::mem::zeroed();
            let state = ffi::duckdb_query(self.conn, c.as_ptr(), &mut result);
            let out = if state == ffi::duckdb_state_DuckDBSuccess { Ok(read(&mut result)) } else { Err(text(ffi::duckdb_result_error(&mut result))) };
            ffi::duckdb_destroy_result(&mut result);
            out
        }
    }

    fn exec(&self, sql: &str) -> Result<(), String> {
        self.query(sql, |_| ())
    }

    /// The rows of `sql`, each value its text or NULL.
    fn rows(&self, sql: &str) -> Result<Vec<Vec<Option<String>>>, String> {
        self.query(sql, |r| unsafe { values(r) })
    }

    /// The prepared statement's types, as the engine reads a describe (`DuckdbDescribe`).
    fn describe(&self, sql: &str) -> Result<Value, DbError> {
        let c = CString::new(sql).map_err(|e| DbError { message: e.to_string(), code: None, position: None })?;
        unsafe {
            let mut stmt: ffi::duckdb_prepared_statement = std::ptr::null_mut();
            if ffi::duckdb_prepare(self.conn, c.as_ptr(), &mut stmt) != ffi::duckdb_state_DuckDBSuccess {
                let message = text(ffi::duckdb_prepare_error(stmt));
                ffi::duckdb_destroy_prepare(&mut stmt);
                return Err(DbError { message, code: None, position: None });
            }
            let mut params = Vec::new();
            for i in 1..=ffi::duckdb_nparams(stmt) {
                let mut t = ffi::duckdb_param_logical_type(stmt, i);
                let ty = if t.is_null() || matches!(id_name(ffi::duckdb_get_type_id(t)), "INVALID" | "ANY" | "SQLNULL") { Value::Null } else { logical(t) };
                if !t.is_null() {
                    ffi::duckdb_destroy_logical_type(&mut t);
                }
                params.push(json!({ "name": owned(ffi::duckdb_parameter_name(stmt, i) as *mut _), "type": ty }));
            }
            let mut columns = Vec::new();
            for i in 0..ffi::duckdb_prepared_statement_column_count(stmt) {
                let mut t = ffi::duckdb_prepared_statement_column_logical_type(stmt, i);
                columns.push(json!({ "name": text(ffi::duckdb_prepared_statement_column_name(stmt, i)), "type": logical(t) }));
                ffi::duckdb_destroy_logical_type(&mut t);
            }
            let statement = statement_name(ffi::duckdb_prepared_statement_type(stmt));
            ffi::duckdb_destroy_prepare(&mut stmt);
            Ok(json!({ "statement": statement, "params": params, "columns": columns }))
        }
    }

    /// Runs `sql` with `params` bound as text (NULL where `None`), stopped at `timeout_ms`: how many
    /// rows it returned, and which columns held a NULL.
    fn run(&self, sql: &str, params: &[Option<String>], timeout_ms: u64) -> Ran<(usize, Vec<bool>)> {
        let Ok(c) = CString::new(sql) else { return Ran::Failed("a NUL in the statement".into()) };
        unsafe {
            let mut stmt: ffi::duckdb_prepared_statement = std::ptr::null_mut();
            if ffi::duckdb_prepare(self.conn, c.as_ptr(), &mut stmt) != ffi::duckdb_state_DuckDBSuccess {
                let message = text(ffi::duckdb_prepare_error(stmt));
                ffi::duckdb_destroy_prepare(&mut stmt);
                return Ran::Failed(message);
            }
            for (i, p) in params.iter().enumerate() {
                let state = match p {
                    None => ffi::duckdb_bind_null(stmt, i as u64 + 1),
                    Some(v) => match CString::new(v.as_str()) {
                        Ok(v) => ffi::duckdb_bind_varchar(stmt, i as u64 + 1, v.as_ptr()),
                        Err(_) => ffi::duckdb_state_DuckDBError,
                    },
                };
                if state != ffi::duckdb_state_DuckDBSuccess {
                    ffi::duckdb_destroy_prepare(&mut stmt);
                    return Ran::Failed(format!("cannot bind parameter {}", i + 1));
                }
            }
            let mut result: ffi::duckdb_result = std::mem::zeroed();
            let fired = AtomicBool::new(false);
            let done = AtomicBool::new(false);
            let conn = Interrupt(self.conn);
            let state = std::thread::scope(|s| {
                s.spawn(|| {
                    let start = Instant::now();
                    while !done.load(Ordering::Acquire) {
                        if start.elapsed() > Duration::from_millis(timeout_ms) {
                            fired.store(true, Ordering::Release);
                            let conn = conn;
                            ffi::duckdb_interrupt(conn.0);
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    }
                });
                let state = ffi::duckdb_execute_prepared(stmt, &mut result);
                done.store(true, Ordering::Release);
                state
            });
            let out = if state != ffi::duckdb_state_DuckDBSuccess {
                if fired.load(Ordering::Acquire) {
                    Ran::TimedOut
                } else {
                    Ran::Failed(text(ffi::duckdb_result_error(&mut result)))
                }
            } else {
                // Chunk by chunk, which reads a column of any type, a nested one too.
                let columns = ffi::duckdb_column_count(&mut result);
                let mut nulls = vec![false; columns as usize];
                let mut rows = 0usize;
                loop {
                    let mut chunk = ffi::duckdb_fetch_chunk(result);
                    if chunk.is_null() {
                        break;
                    }
                    let size = ffi::duckdb_data_chunk_get_size(chunk);
                    rows += size as usize;
                    for (col, null) in nulls.iter_mut().enumerate() {
                        let validity = ffi::duckdb_vector_get_validity(ffi::duckdb_data_chunk_get_vector(chunk, col as u64));
                        if !validity.is_null() && (0..size).any(|row| !ffi::duckdb_validity_row_is_valid(validity, row)) {
                            *null = true;
                        }
                    }
                    ffi::duckdb_destroy_data_chunk(&mut chunk);
                }
                Ran::Done((rows, nulls))
            };
            ffi::duckdb_destroy_result(&mut result);
            ffi::duckdb_destroy_prepare(&mut stmt);
            out
        }
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        unsafe {
            ffi::duckdb_disconnect(&mut self.conn);
            ffi::duckdb_close(&mut self.db);
        }
    }
}

/// Every value of a result, as its text or NULL.
unsafe fn values(r: &mut ffi::duckdb_result) -> Vec<Vec<Option<String>>> {
    let (rows, columns) = (ffi::duckdb_row_count(r), ffi::duckdb_column_count(r));
    (0..rows).map(|row| (0..columns).map(|col| if ffi::duckdb_value_is_null(r, col, row) { None } else { Some(owned(ffi::duckdb_value_varchar(r, col, row))) }).collect()).collect()
}

/// DuckDB's name for a type id (`duckdb_type`), as node-api's `DuckDBTypeId` names it.
fn id_name(id: ffi::duckdb_type) -> &'static str {
    match id {
        1 => "BOOLEAN",
        2 => "TINYINT",
        3 => "SMALLINT",
        4 => "INTEGER",
        5 => "BIGINT",
        6 => "UTINYINT",
        7 => "USMALLINT",
        8 => "UINTEGER",
        9 => "UBIGINT",
        10 => "FLOAT",
        11 => "DOUBLE",
        12 => "TIMESTAMP",
        13 => "DATE",
        14 => "TIME",
        15 => "INTERVAL",
        16 => "HUGEINT",
        17 => "VARCHAR",
        18 => "BLOB",
        19 => "DECIMAL",
        20 => "TIMESTAMP_S",
        21 => "TIMESTAMP_MS",
        22 => "TIMESTAMP_NS",
        23 => "ENUM",
        24 => "LIST",
        25 => "STRUCT",
        26 => "MAP",
        27 => "UUID",
        28 => "UNION",
        29 => "BIT",
        30 => "TIME_TZ",
        31 => "TIMESTAMP_TZ",
        32 => "UHUGEINT",
        33 => "ARRAY",
        34 => "ANY",
        35 => "BIGNUM",
        36 => "SQLNULL",
        39 => "TIME_NS",
        40 => "GEOMETRY",
        41 => "VARIANT",
        42 => "TIMESTAMP_TZ_NS",
        _ => "INVALID",
    }
}

/// DuckDB's name for a statement type (`duckdb_statement_type`), as node-api's `StatementType` names it.
fn statement_name(t: ffi::duckdb_statement_type) -> &'static str {
    const NAMES: [&str; 31] = [
        "INVALID", "SELECT", "INSERT", "UPDATE", "EXPLAIN", "DELETE", "PREPARE", "CREATE", "EXECUTE", "ALTER", "TRANSACTION", "COPY", "ANALYZE", "VARIABLE_SET", "CREATE_FUNC", "DROP", "EXPORT", "PRAGMA",
        "VACUUM", "CALL", "SET", "LOAD", "RELATION", "EXTENSION", "LOGICAL_PLAN", "ATTACH", "DETACH", "MULTI", "COPY_DATABASE", "UPDATE_EXTENSIONS", "MERGE_INTO",
    ];
    NAMES.get(t as usize).copied().unwrap_or("INVALID")
}

/// A logical type as the engine reads one (`LogicalType`): its id, and what the id says more of.
unsafe fn logical(t: ffi::duckdb_logical_type) -> Value {
    let id = id_name(ffi::duckdb_get_type_id(t));
    let mut out = Map::new();
    out.insert("id".into(), json!(id));
    let alias = owned(ffi::duckdb_logical_type_get_alias(t));
    if !alias.is_empty() {
        out.insert("alias".into(), json!(alias));
    }
    let child = |mut c: ffi::duckdb_logical_type| {
        let v = logical(c);
        ffi::duckdb_destroy_logical_type(&mut c);
        v
    };
    match id {
        "DECIMAL" => {
            out.insert("width".into(), json!(ffi::duckdb_decimal_width(t)));
            out.insert("scale".into(), json!(ffi::duckdb_decimal_scale(t)));
        }
        "LIST" => {
            out.insert("child".into(), child(ffi::duckdb_list_type_child_type(t)));
        }
        "ARRAY" => {
            out.insert("child".into(), child(ffi::duckdb_array_type_child_type(t)));
            out.insert("size".into(), json!(ffi::duckdb_array_type_array_size(t)));
        }
        "MAP" => {
            out.insert("key".into(), child(ffi::duckdb_map_type_key_type(t)));
            out.insert("value".into(), child(ffi::duckdb_map_type_value_type(t)));
        }
        "STRUCT" => {
            let fields: Vec<Value> = (0..ffi::duckdb_struct_type_child_count(t)).map(|i| json!({ "name": owned(ffi::duckdb_struct_type_child_name(t, i)), "type": child(ffi::duckdb_struct_type_child_type(t, i)) })).collect();
            out.insert("fields".into(), json!(fields));
        }
        "UNION" => {
            let fields: Vec<Value> = (0..ffi::duckdb_union_type_member_count(t)).map(|i| json!({ "name": owned(ffi::duckdb_union_type_member_name(t, i)), "type": child(ffi::duckdb_union_type_member_type(t, i)) })).collect();
            out.insert("fields".into(), json!(fields));
        }
        "ENUM" => {
            let labels: Vec<String> = (0..ffi::duckdb_enum_dictionary_size(t) as u64).map(|i| owned(ffi::duckdb_enum_dictionary_value(t, i))).collect();
            out.insert("values".into(), json!(labels));
        }
        _ => {}
    }
    Value::Object(out)
}

/// The engine's host: the prepared statement's describe, and each catalog query's rows as text.
struct Host<'a>(&'a Db);

impl reylo_engine::Answerer<reylo_engine::duckdb::api::DuckdbDescribe> for Host<'_> {
    type Error = String;

    fn describe(&mut self, sql: &str) -> Result<Result<reylo_engine::duckdb::api::DuckdbDescribe, DbError>, String> {
        match self.0.describe(sql) {
            Ok(v) => serde_json::from_value(v).map(Ok).map_err(|e| e.to_string()),
            Err(e) => Ok(Err(e)),
        }
    }

    fn query(&mut self, sql: &str, params: &[Value]) -> Result<Result<Vec<Vec<Value>>, DbError>, String> {
        if !params.is_empty() {
            return Err("the engine's DuckDB catalog queries take no parameter".into());
        }
        // A catalog query that fails is a failure of the case, never an empty answer.
        let rows = self.0.rows(sql)?;
        Ok(Ok(rows.into_iter().map(|r| r.into_iter().map(|v| v.map_or(Value::Null, Value::String)).collect()).collect()))
    }
}

/// Runs a DuckDB case: each statement's analysis, then its runs over the seeded tables.
pub fn run(case: &Path, timeout_ms: u64, given: &Map<String, Value>, dumped: &mut Map<String, Value>, statements: Vec<(String, String)>) -> Result<(Vec<String>, Vec<String>), String> {
    let name: String = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let setup = fs::read_to_string(case.join("setup.sql")).map_err(|e| e.to_string())?;
    let file = std::env::temp_dir().join(format!("reylo-oracle-{name}-{}.duckdb", std::process::id()));
    let fresh = |file: &Path| -> Result<Db, String> {
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(file.with_extension("duckdb.wal"));
        let db = Db::open(file)?;
        db.exec(&setup).map_err(|e| format!("setup.sql: {e}"))?;
        Ok(db)
    };
    let db = fresh(&file)?;
    let mut lines = Vec::new();
    let mut analyzed = Vec::new();
    for (file_name, sql) in statements {
        let analysis = match given.get(&format!("{name}/{file_name}")) {
            Some(answer) => answer.clone(),
            None => {
                let analysis = reylo_engine::replay::run::<reylo_engine::duckdb::Duckdb, _>(&sql, &mut Host(&db), &mut AnswerCache::default()).map_err(|e| format!("{file_name}: {e}"))?;
                serde_json::to_value(&analysis).map_err(|e| e.to_string())?
            }
        };
        lines.push(format!("{file_name}: {}", crate::line(&analysis)));
        dumped.insert(format!("{name}/{file_name}"), analysis.clone());
        analyzed.push((file_name, sql, analysis));
    }
    drop(db);
    let report = check(&file, &analyzed, timeout_ms, &fresh);
    let _ = fs::remove_file(&file);
    let _ = fs::remove_file(file.with_extension("duckdb.wal"));
    let unchecked = crate::seeded(case, lines.len(), report?)?;
    Ok((lines, unchecked))
}

/// One table's columns, in order, and its keys.
struct Table {
    name: String,
    columns: Vec<Column>,
}

struct Column {
    name: String,
    ty: String,
    nullable: bool,
    /// A column of a primary key or a UNIQUE constraint, and whether one of them is it alone.
    key: bool,
    alone: bool,
}

const TABLES_QUERY: &str = "select c.table_name, c.column_name, c.data_type, c.is_nullable, \
    exists (select 1 from duckdb_constraints() k where k.table_name = c.table_name and k.constraint_type in ('PRIMARY KEY', 'UNIQUE') and list_contains(k.constraint_column_names, c.column_name)), \
    exists (select 1 from duckdb_constraints() k where k.table_name = c.table_name and k.constraint_type in ('PRIMARY KEY', 'UNIQUE') and k.constraint_column_names = [c.column_name]) \
    from duckdb_columns() c join duckdb_tables() t on t.table_name = c.table_name and t.schema_name = c.schema_name and t.database_name = c.database_name \
    where c.database_name = current_database() and not t.temporary order by t.table_oid, c.column_index";

fn tables(db: &Db) -> Result<Vec<Table>, String> {
    let mut out: Vec<Table> = Vec::new();
    for row in db.rows(TABLES_QUERY)? {
        let get = |k: usize| row.get(k).cloned().flatten().unwrap_or_default();
        let column = Column { name: get(1), ty: get(2), nullable: get(3) == "true", key: get(4) == "true", alone: get(5) == "true" };
        match out.last_mut() {
            Some(t) if t.name == get(0) => t.columns.push(column),
            _ => out.push(Table { name: get(0), columns: vec![column] }),
        }
    }
    Ok(out)
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The edge values of a type as SQL literals (each cast to the type), the extreme ones only in
/// the edge state. An ENUM's labels are its values; a type the list does not know has NULL alone.
fn pool(ty: &str, scenario: Scenario) -> Vec<String> {
    let t = ty.to_ascii_uppercase();
    let edges = scenario == Scenario::Edges;
    let mut values: Vec<&str> = Vec::new();
    let cast = |vs: Vec<String>| vs.into_iter().map(|v| format!("{v}::{ty}")).collect::<Vec<_>>();
    if let Some(labels) = t.strip_prefix("ENUM(") {
        let labels = &ty[5..5 + labels.len() - 1];
        return cast(labels.split(", ").map(String::from).collect());
    }
    if t.ends_with("[]") {
        return cast(vec!["[]".into(), "[NULL]".into()]);
    }
    if t.starts_with("STRUCT") || t.starts_with("MAP") || t.starts_with("UNION") || t.ends_with(']') {
        return Vec::new();
    }
    match t.split('(').next().unwrap_or(&t) {
        "BOOLEAN" => values.extend(["true", "false"]),
        "TINYINT" | "SMALLINT" | "INTEGER" | "BIGINT" | "HUGEINT" => {
            values.extend(["0", "1", "-1"]);
            if edges {
                let (min, max) = match t.as_str() {
                    "TINYINT" => ("-128", "127"),
                    "SMALLINT" => ("-32768", "32767"),
                    "INTEGER" => ("-2147483648", "2147483647"),
                    "BIGINT" => ("-9223372036854775808", "9223372036854775807"),
                    _ => ("-170141183460469231731687303715884105728", "170141183460469231731687303715884105727"),
                };
                return cast(["0", "1", "-1", min, max].iter().map(|v| format!("({v})")).collect());
            }
        }
        "UTINYINT" | "USMALLINT" | "UINTEGER" | "UBIGINT" | "UHUGEINT" => values.extend(["0", "1"]),
        "FLOAT" | "DOUBLE" => {
            values.extend(["0", "-1.5"]);
            if edges {
                values.extend(["'nan'", "'inf'", "'-inf'"]);
            }
        }
        "DECIMAL" => values.extend(["0", "-1"]),
        "VARCHAR" => return cast(vec!["''".into(), "'a'".into(), "'A'".into()]),
        "BLOB" => return cast(vec!["''".into(), "'\\x00'".into()]),
        "UUID" => return cast(vec![literal("00000000-0000-0000-0000-000000000000")]),
        "DATE" => {
            values.extend(["'2026-01-01'"]);
            if edges {
                values.extend(["'infinity'", "'-infinity'"]);
            }
        }
        "TIMESTAMP" | "TIMESTAMP_S" | "TIMESTAMP_MS" | "TIMESTAMP_NS" | "TIMESTAMP WITH TIME ZONE" => {
            values.extend(["'2026-01-01 00:00:00'"]);
            if edges {
                values.extend(["'infinity'", "'-infinity'"]);
            }
        }
        "TIME" => values.extend(["'00:00:00'"]),
        "INTERVAL" => values.extend(["'1 day'", "'-1 day'"]),
        "JSON" => values.extend(["'null'", "'{}'", "'[]'"]),
        _ => {}
    }
    cast(values.into_iter().map(String::from).collect())
}

/// A value of the type no other row of the column holds, for a key column: the `n`th.
fn fresh_value(ty: &str, n: usize) -> Option<String> {
    let t = ty.to_ascii_uppercase();
    let base = t.split('(').next().unwrap_or(&t);
    let v = match base {
        "TINYINT" | "SMALLINT" | "INTEGER" | "BIGINT" | "HUGEINT" | "UTINYINT" | "USMALLINT" | "UINTEGER" | "UBIGINT" | "UHUGEINT" | "DECIMAL" | "FLOAT" | "DOUBLE" => format!("{}", 1000 + n),
        "VARCHAR" | "JSON" => literal(&format!("k{n}")),
        "DATE" => format!("('2000-01-01'::date + {n})"),
        "UUID" => literal(&format!("00000000-0000-0000-0000-{n:012}")),
        _ => return None,
    };
    Some(format!("{v}::{ty}"))
}

/// Seeds every table with a base row and a row per edge value of each column, each row in a
/// statement of its own, and passes again for a row a foreign key refused until none goes in.
fn seed(db: &Db, scenario: Scenario) -> Result<Vec<String>, String> {
    let tables = tables(db)?;
    let mut notes = Vec::new();
    let mut n = 0usize;
    let mut rows: Vec<[String; 3]> = Vec::new();
    for table in &tables {
        let pools: Vec<Vec<String>> = table.columns.iter().map(|c| {
            let mut pool = pool(&c.ty, scenario);
            if c.nullable {
                pool.insert(0, "NULL".into());
            }
            pool
        }).collect();
        let base: Vec<String> = pools.iter().map(|p| p.iter().find(|v| *v != "NULL").or(p.first()).cloned().unwrap_or_else(|| "NULL".into())).collect();
        let mut variants = vec![base.clone()];
        for (k, pool) in pools.iter().enumerate() {
            for v in pool {
                let mut row = base.clone();
                row[k] = v.clone();
                variants.push(row);
            }
        }
        let columns = table.columns.iter().map(|c| quote(&c.name)).collect::<Vec<_>>().join(", ");
        for row in variants {
            // A row as it is first, so that a key holds the pool's values a foreign key can
            // reference; where it broke a key, with fresh values in the columns that are a key
            // alone, so that a column of a key of several repeats its values; and then in every
            // key's columns.
            let mut fresh = |row: &[String], all: bool| {
                let mut out = row.to_vec();
                for (k, c) in table.columns.iter().enumerate() {
                    if c.alone || (all && c.key) {
                        n += 1;
                        if let Some(v) = fresh_value(&c.ty, n) {
                            out[k] = v;
                        }
                    }
                }
                out
            };
            let (alone, every) = (fresh(&row, false), fresh(&row, true));
            let insert = |row: &[String]| format!("insert into {} ({columns}) values ({})", quote(&table.name), row.join(", "));
            rows.push([insert(&row), insert(&alone), insert(&every)]);
        }
    }
    // Each row in a statement of its own, passing again for the rows a foreign key refused.
    let mut pending = rows;
    for _ in 0..4 {
        let before = pending.len();
        pending.retain(|tries| tries.iter().all(|insert| db.exec(insert).is_err()));
        if pending.len() == before {
            break;
        }
    }
    for table in &tables {
        let count = db.rows(&format!("select count(*) from {}", quote(&table.name)))?.first().and_then(|r| r.first().cloned().flatten()).unwrap_or_default();
        if count == "0" {
            notes.push(format!("{} ({}): {}{}", table.name, scenario.name(), "seeded", seed::EMPTY));
        }
    }
    Ok(notes)
}

/// The values a parameter of the engine's type `ty` is run with, as text DuckDB casts: NULL too
/// where the statement takes it. None where the engine did not type the parameter.
fn parameter_pool(ty: &Value, nullable: bool, scenario: Scenario) -> Option<Vec<Option<String>>> {
    let name = match ty["kind"].as_str() {
        Some("scalar") => ty["name"].as_str()?.to_string(),
        Some("enum") => return Some(ty["labels"].as_array()?.iter().filter_map(|l| l.as_str().map(|s| Some(s.to_string()))).chain(nullable.then_some(None)).collect()),
        Some("json") => "json".into(),
        _ => return None,
    };
    let edges = scenario == Scenario::Edges;
    let mut values: Vec<&str> = match name.as_str() {
        "boolean" => vec!["true", "false"],
        "tinyint" | "smallint" | "integer" | "bigint" | "hugeint" | "decimal" => vec!["0", "1", "-1"],
        "utinyint" | "usmallint" | "uinteger" | "ubigint" | "uhugeint" => vec!["0", "1"],
        "float" | "double" => vec!["0", "-1.5"],
        "varchar" => vec!["", "a"],
        "date" => vec!["2026-01-01"],
        "timestamp" | "timestamp_s" | "timestamp_ms" | "timestamp_ns" => vec!["2026-01-01 00:00:00"],
        "timestamptz" => vec!["2026-01-01 00:00:00+00"],
        "time" => vec!["00:00:00"],
        "interval" => vec!["1 day"],
        "uuid" => vec!["00000000-0000-0000-0000-000000000000"],
        "json" => vec!["null", "{}"],
        _ => return None,
    };
    if edges && matches!(name.as_str(), "date" | "timestamp" | "timestamptz") {
        values.push("infinity");
    }
    let mut out: Vec<Option<String>> = values.into_iter().map(|v| Some(v.to_string())).collect();
    if nullable {
        out.push(None);
    }
    Some(out)
}

fn check(file: &Path, statements: &[(String, String, Value)], timeout_ms: u64, fresh: &dyn Fn(&Path) -> Result<Db, String>) -> Result<Report, String> {
    let mut tallies: Vec<Tally> = statements.iter().map(|_| Tally::default()).collect();
    let mut problems = Vec::new();
    let mut notes = Vec::new();
    for scenario in SCENARIOS {
        let db = fresh(file)?;
        notes.extend(seed(&db, scenario)?);
        for ((file_name, _, analysis), tally) in statements.iter().zip(tallies.iter_mut()) {
            let sql = analysis["rewritten"]["sql"].as_str().unwrap_or_default();
            let params = analysis["params"].as_array().cloned().unwrap_or_default();
            let mut pools = Vec::new();
            for p in &params {
                match parameter_pool(&p["type"], p["nullable"].as_bool().unwrap_or(false), scenario) {
                    Some(pool) => pools.push(pool),
                    None => {
                        tally.refused = Some(format!("parameter {} has a type the oracle seeds no value of", p["name"].as_str().unwrap_or("?")));
                        break;
                    }
                }
            }
            if tally.refused.is_some() {
                continue;
            }
            let width = pools.iter().map(Vec::len).max().unwrap_or(1).max(1);
            let assignments: Vec<Vec<Option<String>>> = (0..width).map(|r| pools.iter().map(|p| p[r % p.len()].clone()).collect()).collect();
            let columns = analysis["columns"].as_array().cloned().unwrap_or_default();
            let cardinality = analysis["cardinality"].as_str().unwrap_or("many");
            for values in assignments {
                if db.exec("begin transaction").is_err() {
                    return Err("cannot begin a transaction".into());
                }
                match db.run(sql, &values, timeout_ms) {
                    Ran::TimedOut => tally.timed_out += 1,
                    Ran::Failed(e) => tally.fail(e),
                    Ran::Done((rows, nulls)) => {
                        tally.ran += 1;
                        let shown = |v: &Option<String>| v.as_deref().map_or("NULL".to_string(), |s| format!("'{s}'"));
                        let with = values.iter().map(shown).collect::<Vec<_>>().join(", ");
                        for (c, null) in columns.iter().zip(&nulls) {
                            if *null && !c["nullable"].as_bool().unwrap_or(true) {
                                problems.push(format!("{file_name} ({}, with {with}): column {} is typed non-null and holds NULL", scenario.name(), c["name"].as_str().unwrap_or("?")));
                            }
                        }
                        let wrong = match cardinality {
                            "one" => rows != 1,
                            "maybeOne" => rows > 1,
                            _ => false,
                        };
                        if wrong {
                            problems.push(format!("{file_name} ({}, with {with}): proven {cardinality}, returned {rows} rows", scenario.name()));
                        }
                    }
                }
                let _ = db.exec("rollback");
            }
        }
    }
    Ok(seed::report(statements, &tallies, timeout_ms, problems, notes))
}
