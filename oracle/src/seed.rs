//! The soundness half: seed a case's tables with values that test the types, run every statement
//! over them, and report any value that contradicts what the engine said.
//!
//! Everything runs in a transaction that is rolled back, after `SET CONSTRAINTS ALL DEFERRED`,
//! so a deferrable unique constraint lets the duplicates it would refuse at commit stand while the
//! statements run. Each column has a pool of edge values for its type: NULL, case variants of one
//! string, an empty and an unbounded range, `'infinity'`, `NaN`, an array with a NULL element, an
//! IPv6 address and a /0 network; a literal the column's type refuses is dropped from its pool,
//! and a column left with NULL alone is named in a note. Each table gets a base row of the
//! first value of every pool, then one row per value of each pool with the rest of the base row
//! beside it, so every edge value turns up whatever the other columns demand, and a row of NULLs
//! where the table allows it. A row that breaks a unique key gets fresh values in the key's
//! columns and is tried again, so the same values stand twice wherever no immediate key forbids
//! it (a deferrable key, an inheritance parent and its child). A row the table refuses otherwise
//! is skipped (each insert sits in its own savepoint), and the passes repeat so a row that
//! references another table goes in once the referenced row is there.
//!
//! The tables are seeded twice, each time in a transaction of its own, and every statement runs
//! over both states: the edge state holds every pool whole, and the plain state leaves out the
//! values of extreme magnitude (`'infinity'`, `NaN`, SQLite's largest integer), beside which a
//! sum overflows, two dates cannot be subtracted and a series never ends, so a statement the edge
//! state only ever makes fail is still checked.
//!
//! A statement runs once per assignment of its parameters, each parameter inlined as a literal
//! cast to the type the server gave its placeholder, by oid, so a type outside `search_path`
//! carries its schema. A parameter's pool is its type's, less the values the type refuses; a
//! parameter left with no value leaves its statement unchecked. Every run is bounded by the
//! timeout (`statement_timeout`, set here on this session; in SQLite a progress handler), and a
//! run the timeout stopped is counted apart and is evidence of nothing. Every result row of a
//! run that succeeded is checked:
//!
//! - a column the engine typed non-null holds no NULL, an array it typed with non-null elements
//!   holds no NULL element, and a JSON field it typed non-null is never null;
//! - a statement proven `one` returns exactly one row, and one proven `at_most_one` at most one;
//! - no two rows agree on every column of the result key, a NULL counting as equal to another;
//! - where the order is total, no two rows in a row tie on every sort key: a sort key that is no
//!   output column is read from a copy of the statement that also selects it.
//!
//! A statement with no run that succeeded is in the report's `unchecked`, with its first error:
//! nothing about it was checked.

use reylo_engine_core::types::{ArrayType, DecimalType, DurationType, FloatType, JsonType, Kind, Size, TextType, Type};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tokio_postgres::error::SqlState;
use tokio_postgres::Client;

/// What the seeding of one case found.
pub struct Report {
    /// The values and row counts that contradict the analyses.
    pub problems: Vec<String>,
    pub notes: Vec<String>,
    /// The statements with no run that succeeded, each with why: nothing about them was checked.
    pub unchecked: Vec<(String, String)>,
}

/// A seeded state: the edge state holds every pool whole, the plain state leaves out the values of
/// extreme magnitude, and the empty state seeds no row at all (DuckDB's oracle alone runs it).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Scenario {
    Edges,
    Plain,
    #[cfg(feature = "duckdb")]
    Empty,
}

pub(crate) const SCENARIOS: [Scenario; 2] = [Scenario::Edges, Scenario::Plain];

impl Scenario {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Scenario::Edges => "edge values",
            Scenario::Plain => "plain values",
            #[cfg(feature = "duckdb")]
            Scenario::Empty => "no rows",
        }
    }
}

/// The values a Postgres pool holds in the edge state alone.
const EXTREMES: [&str; 3] = ["'NaN'", "'infinity'", "'-infinity'"];

/// What the seeded note says of a table that refused every row and holds none: a statement over
/// it alone runs, and is checked against nothing.
pub(crate) const EMPTY: &str = " (no row at all)";

/// The least time a seeding insert may take, whatever the timeout of the runs: a short timeout
/// must not thin the seeded rows out.
const SEEDING_TIMEOUT_MS: u64 = 5000;

/// How one statement's runs went, over every state and assignment.
#[derive(Default)]
pub(crate) struct Tally {
    pub(crate) ran: usize,
    pub(crate) timed_out: usize,
    pub(crate) failed: usize,
    /// The error of the first run that failed.
    pub(crate) first_error: Option<String>,
    /// Why the statement was not run at all.
    pub(crate) refused: Option<String>,
}

impl Tally {
    pub(crate) fn fail(&mut self, error: String) {
        self.failed += 1;
        self.first_error.get_or_insert(error.split_whitespace().collect::<Vec<_>>().join(" "));
    }
}

/// The report of a case: a statement without one run that succeeded is unchecked, by its first
/// error, and a run the timeout stopped is named wherever other runs went on.
pub(crate) fn report(statements: &[(String, String, Value)], tallies: &[Tally], timeout_ms: u64, mut problems: Vec<String>, mut notes: Vec<String>) -> Report {
    let mut unchecked = Vec::new();
    for ((file, _, _), tally) in statements.iter().zip(tallies) {
        let runs = tally.ran + tally.timed_out + tally.failed;
        if let Some(why) = &tally.refused {
            unchecked.push((file.clone(), why.clone()));
        } else if tally.ran == 0 {
            unchecked.push((file.clone(), tally.first_error.clone().unwrap_or_else(|| format!("every run timed out ({timeout_ms} ms)"))));
        }
        if tally.timed_out > 0 && (tally.ran > 0 || tally.failed > 0) {
            notes.push(format!("{file}: {} of {runs} run(s) timed out ({timeout_ms} ms) and count for nothing; {} ran", tally.timed_out, tally.ran));
        }
    }
    problems.sort();
    problems.dedup();
    Report { problems, notes, unchecked }
}

/// One table's columns, in attribute order.
struct Table {
    name: String,
    columns: Vec<Column>,
}

#[derive(Clone)]
struct Column {
    name: String,
    /// `format_type` of the column's type, the cast a literal is written with: it carries the
    /// schema of a type outside `search_path`.
    ty: String,
    /// The base type's `typcategory`, `typtype` and `typname`, the element type's for an array.
    category: String,
    kind: String,
    base: String,
    is_array: bool,
    /// The base type's `typdelim`, which separates an array's elements: `,` for most types, `;`
    /// for `box`, `:` for PostGIS's geometry and geography.
    delimiter: String,
    labels: Vec<String>,
    generated: bool,
}

const TABLES_QUERY: &str = "SELECT c.oid::regclass::text, a.attname::text, pg_catalog.format_type(a.atttypid, a.atttypmod), \
    bt.typcategory::text, bt.typtype::text, bt.typname::text, t.typcategory = 'A', bt.typdelim::text, \
    coalesce((SELECT array_agg(e.enumlabel::text ORDER BY e.enumsortorder) FROM pg_enum e WHERE e.enumtypid = bt.oid), '{}'), \
    a.attgenerated <> '' OR a.attidentity = 'a' \
    FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
    JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
    JOIN pg_type t ON t.oid = a.atttypid \
    JOIN pg_type bt ON bt.oid = CASE WHEN t.typcategory = 'A' THEN t.typelem WHEN t.typtype = 'd' THEN t.typbasetype ELSE t.oid END \
    WHERE c.relkind IN ('r', 'p') AND n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname !~ '^pg_' \
    ORDER BY c.oid, a.attnum";

/// One type by its oid, as `TABLES_QUERY` answers a column's. The name is asked for with no
/// type modifier (`-1`): without that argument `bpchar` comes back as `character`, which as a
/// cast is `character(1)`.
const TYPE_QUERY: &str = "SELECT pg_catalog.format_type(t.oid, -1), bt.typcategory::text, bt.typtype::text, bt.typname::text, t.typcategory = 'A', bt.typdelim::text, \
    coalesce((SELECT array_agg(e.enumlabel::text ORDER BY e.enumsortorder) FROM pg_enum e WHERE e.enumtypid = bt.oid), '{}') \
    FROM pg_type t \
    JOIN pg_type bt ON bt.oid = CASE WHEN t.typcategory = 'A' THEN t.typelem WHEN t.typtype = 'd' THEN t.typbasetype ELSE t.oid END \
    WHERE t.oid = $1";

async fn tables(client: &Client) -> Result<Vec<Table>, String> {
    let rows = client.query(TABLES_QUERY, &[]).await.map_err(|e| message(&e))?;
    let mut out: Vec<Table> = Vec::new();
    for row in rows {
        let table: String = row.get(0);
        let column = Column {
            name: row.get(1),
            ty: row.get(2),
            category: row.get(3),
            kind: row.get(4),
            base: row.get(5),
            is_array: row.get(6),
            delimiter: row.get(7),
            labels: row.get(8),
            generated: row.get(9),
        };
        match out.last_mut() {
            Some(t) if t.name == table => t.columns.push(column),
            _ => out.push(Table { name: table, columns: vec![column] }),
        }
    }
    Ok(out)
}

/// The array types whose elements are not separated by a comma, by name, each with its element
/// type's delimiter: `_box` with `;`, PostGIS's `_geometry` and `_geography` with `:`.
async fn delimiters(client: &Client) -> Result<HashMap<String, char>, String> {
    let rows = client
        .query("SELECT t.typname::text, e.typdelim::text FROM pg_type t JOIN pg_type e ON e.oid = t.typelem WHERE t.typcategory = 'A' AND e.typdelim <> ','", &[])
        .await
        .map_err(|e| message(&e))?;
    Ok(rows.iter().filter_map(|r| Some((r.get::<_, String>(0), r.get::<_, String>(1).chars().next()?))).collect())
}

/// The type a placeholder was given, read from the catalog by its oid.
async fn type_of(client: &Client, oid: u32, known: &mut HashMap<u32, Column>) -> Result<Column, String> {
    if let Some(column) = known.get(&oid) {
        return Ok(column.clone());
    }
    let row = client.query_opt(TYPE_QUERY, &[&oid]).await.map_err(|e| message(&e))?.ok_or_else(|| format!("no type has the oid {oid}"))?;
    let column = Column { name: String::new(), ty: row.get(0), category: row.get(1), kind: row.get(2), base: row.get(3), is_array: row.get(4), delimiter: row.get(5), labels: row.get(6), generated: false };
    known.insert(oid, column.clone());
    Ok(column)
}

/// The edge values of one scalar column's type in a state, as SQL literals, NULL first. An
/// array column's pool is built by `accepted_pool`, which checks its element first.
fn pool(col: &Column, scenario: Scenario) -> Vec<String> {
    with_casts(col, scalars(col, scenario))
}

/// The values of a type's pool, NULL first, each cast to the type.
fn with_casts(col: &Column, values: Vec<String>) -> Vec<String> {
    let mut out = vec!["NULL".to_string()];
    out.extend(values.into_iter().map(|v| format!("{v}::{}", col.ty)));
    out
}

/// The literals of the element type's pool in a state, uncast: an array column's are its
/// element's.
fn scalars(col: &Column, scenario: Scenario) -> Vec<String> {
    let literals: &[&str] = match (col.kind.as_str(), col.category.as_str(), col.base.as_str()) {
        ("e", _, _) => &[],
        ("r", _, _) => &["'empty'", "'(,5)'", "'[1,)'", "'[1,3)'"],
        // No range, one, two with a gap between them, and one of dates for the multiranges of
        // dates and times.
        ("m", _, _) => &["'{}'", "'{[1,3)}'", "'{(,5), [7,)}'", "'{[2026-01-01,2026-02-01)}'"],
        (_, "N", _) => &["1", "2", "0", "0.1", "0.10000000000000000001", "'NaN'"],
        (_, "S", _) => &["'a'", "'A'", "'b'", "''", "'a '"],
        (_, "B", _) => &["true", "false"],
        (_, "D", _) => &["'2026-01-01'", "'infinity'", "'-infinity'", "'2026-01-01 12:00'", "'12:00'"],
        (_, "T", _) => &["'1 day'", "'infinity'"],
        (_, "U", "json" | "jsonb") => &["'null'", "'{}'", "'{\"a\": 1}'", "'[]'"],
        (_, "U", "uuid") => &["'00000000-0000-0000-0000-000000000001'", "'00000000-0000-0000-0000-000000000002'"],
        // An address of each family, a host inside a network (which cidr refuses), a network, and
        // the networks that hold every address.
        (_, "I", _) => &["'10.0.0.1'", "'192.168.1.5/24'", "'::1'", "'::ffff:10.0.0.1'", "'10.0.0.0/8'", "'2001:db8::/32'", "'0.0.0.0/0'", "'::/0'"],
        (_, "U", "macaddr") => &["'08:00:2b:01:02:03'", "'00:00:00:00:00:00'", "'ff:ff:ff:ff:ff:ff'"],
        (_, "U", "macaddr8") => &["'08:00:2b:01:02:03:04:05'", "'08:00:2b:01:02:03'", "'00:00:00:00:00:00:00:00'", "'ff:ff:ff:ff:ff:ff:ff:ff'"],
        // No key, one key, a NULL value, and a key that needs quoting.
        (_, "U", "hstore") => &["''", "'a=>1'", "'a=>1, b=>NULL'", "'\"k e\"=>\"v\"'"],
        // pgvector's vectors, of one, two and three dimensions (a typmod keeps the ones of its
        // own), and a zero vector, which has no direction for `<=>` to measure.
        (_, "U", "vector" | "halfvec") => &["'[1]'", "'[1,2]'", "'[1,2,3]'", "'[0,0,0]'"],
        (_, "U", "sparsevec") => &["'{1:1}/3'", "'{}/3'", "'{}/5'"],
        // PostGIS: a point with no SRID (which a typmod's SRID is applied to), one with an SRID,
        // an empty point, another geometry type, and an empty collection.
        (_, "U", "geometry") => &["'POINT(1 2)'", "'SRID=4326;POINT(1 2)'", "'SRID=4326;POINT EMPTY'", "'LINESTRING(0 0,1 1)'", "'GEOMETRYCOLLECTION EMPTY'"],
        (_, "U", "geography") => &["'POINT(1 2)'", "'SRID=4326;POINT EMPTY'", "'LINESTRING(0 0,1 1)'"],
        (_, "U", "box2d") => &["'BOX(0 0,1 1)'", "'BOX(1 2,1 2)'"],
        (_, "U", "box3d") => &["'BOX3D(0 0 0,1 1 1)'"],
        // A point of one and two dimensions, a box, and the point earthdistance's earth domain
        // takes, `ll_to_earth(1, 2)` as the server writes it.
        (_, "U", "cube") => &["'(1,2)'", "'(0)'", "'(0,0),(1,1)'", "'(6373311.758273381, 222560.95080971014, 111314.3802612757)'"],
        (_, "U", "seg") => &["'1 .. 2'", "'0.5'", "'1 ..'"],
        // A valid number of each isn type, short and thirteen digits long: each type takes those
        // it can hold, ean13 every thirteen-digit one, a UPC's included.
        (_, "U", "isbn" | "isbn13" | "ismn" | "ismn13" | "issn" | "issn13" | "ean13" | "upc") => {
            &["'0-306-40615-2'", "'978-0-306-40615-7'", "'M-2306-7118-7'", "'979-0-2306-7118-7'", "'0317-8471'", "'977-0317-847-00-1'", "'012345678905'", "'0012345678905'"]
        }
        _ => &["'1'", "'a'"],
    };
    let mut out: Vec<String> = if col.kind == "e" { col.labels.iter().map(|l| quote(l)).collect() } else { literals.iter().map(|v| v.to_string()).collect() };
    if scenario == Scenario::Plain {
        out.retain(|v| !EXTREMES.contains(&v.as_str()));
    }
    out
}

/// An array type's values, uncast: an empty array, one of an element, the element beside a NULL,
/// and a NULL alone. The element is quoted, so a comma inside it (`'[1,2]'` of a vector) is not
/// a separator, and stands beside the NULL with the type's delimiter. With no element there are
/// the empty array and the NULL alone.
fn arrays(col: &Column, element: Option<&str>) -> Vec<String> {
    let mut out = vec!["'{}'".to_string()];
    if let Some(element) = element {
        let text = element.strip_prefix('\'').and_then(|e| e.strip_suffix('\'')).unwrap_or(element);
        let quoted = format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
        out.push(format!("'{{{quoted}}}'"));
        out.push(format!("'{{{quoted}{}NULL}}'", col.delimiter));
    }
    out.push("'{NULL}'".to_string());
    out
}

/// A column's pool less the values its type refuses, NULL first. An array's elements are the
/// first value of the element type's pool that the element type, its typmod included, accepts:
/// `vector(3)[]` refuses `'{"[1]"}'`.
async fn accepted_pool(client: &Client, col: &Column, scenario: Scenario) -> Result<Vec<String>, String> {
    let candidates = if col.is_array {
        let element_ty = col.ty.strip_suffix("[]").unwrap_or(&col.ty);
        let mut first = None;
        for value in scalars(col, scenario) {
            if try_execute(client, &format!("SELECT {value}::{element_ty}")).await? {
                first = Some(value);
                break;
            }
        }
        with_casts(col, arrays(col, first.as_deref()))
    } else {
        pool(col, scenario)
    };
    let mut out = Vec::with_capacity(candidates.len());
    for value in candidates {
        if value == "NULL" || try_execute(client, &format!("SELECT {value}")).await? {
            out.push(value);
        }
    }
    Ok(out)
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Seed every table with a state's values, inside the transaction the caller opened; the rows
/// inserted per table. A column whose type refuses every value of its pool goes into `bare`.
async fn seed(client: &Client, tables: &[Table], scenario: Scenario, bare: &mut Vec<String>) -> Result<Vec<(String, usize)>, String> {
    let mut pools: Vec<Vec<Vec<String>>> = Vec::with_capacity(tables.len());
    for table in tables {
        let mut columns = Vec::new();
        for column in table.columns.iter().filter(|c| !c.generated) {
            let accepted = accepted_pool(client, column, scenario).await?;
            if accepted.len() == 1 {
                bare.push(format!("{}.{} ({})", table.name, column.name, column.ty));
            }
            columns.push(accepted);
        }
        pools.push(columns);
    }
    let mut inserted: Vec<(String, usize)> = tables.iter().map(|t| (t.name.clone(), 0)).collect();
    let mut fresh = 0;
    for _pass in 0..3 {
        for (t, table) in tables.iter().enumerate() {
            let columns: Vec<&Column> = table.columns.iter().filter(|c| !c.generated).collect();
            if columns.is_empty() {
                continue;
            }
            let pools = &pools[t];
            let base: Vec<String> = pools.iter().map(|p| p.iter().find(|v| *v != "NULL").cloned().unwrap_or_else(|| "NULL".into())).collect();
            let mut rows = vec![base.clone(), vec!["NULL".to_string(); columns.len()]];
            for (i, pool) in pools.iter().enumerate() {
                for value in pool {
                    let mut row = base.clone();
                    row[i] = value.clone();
                    rows.push(row);
                }
            }
            for row in rows {
                if insert(client, &table.name, &columns, row, &mut fresh).await? {
                    inserted[t].1 += 1;
                }
            }
        }
    }
    Ok(inserted)
}

/// Insert one row; a row that breaks a unique key is tried again, up to three times, with fresh
/// values in the columns the server names.
async fn insert(client: &Client, table: &str, columns: &[&Column], mut row: Vec<String>, fresh: &mut usize) -> Result<bool, String> {
    let names: Vec<String> = columns.iter().map(|c| format!("\"{}\"", c.name.replace('"', "\"\""))).collect();
    for _ in 0..4 {
        let sql = format!("SAVEPOINT s; INSERT INTO {table} ({}) VALUES ({}); RELEASE SAVEPOINT s", names.join(", "), row.join(", "));
        let error = match client.batch_execute(&sql).await {
            Ok(()) => return Ok(true),
            Err(e) => e,
        };
        client.batch_execute("ROLLBACK TO SAVEPOINT s; RELEASE SAVEPOINT s").await.map_err(|e| message(&e))?;
        let Some(db) = error.as_db_error() else { return Ok(false) };
        if db.code() != &SqlState::UNIQUE_VIOLATION {
            return Ok(false);
        }
        // "Key (a, b)=(1, 2) already exists."
        let Some(key) = db.detail().and_then(|d| d.strip_prefix("Key (")).and_then(|d| d.split_once(")=")).map(|(k, _)| k.to_string()) else { return Ok(false) };
        for name in key.split(", ") {
            let Some(i) = columns.iter().position(|c| c.name == name.trim_matches('"')) else { return Ok(false) };
            *fresh += 1;
            row[i] = match fresh_value(columns[i], *fresh) {
                Some(v) => v,
                None => return Ok(false),
            };
        }
    }
    Ok(false)
}

/// A value of the column's type no pool holds, for a key column to take.
fn fresh_value(col: &Column, n: usize) -> Option<String> {
    if col.is_array || col.kind == "e" {
        return None;
    }
    Some(match (col.category.as_str(), col.base.as_str()) {
        ("N", _) => format!("{}::{}", 1000 + n, col.ty),
        ("S", _) => format!("'k{n}'::{}", col.ty),
        ("D", _) => format!("('2000-01-01'::date + {n})::{}", col.ty),
        ("U", "uuid") => format!("'00000000-0000-0000-0000-{n:012}'::{}", col.ty),
        ("I", _) => format!("'172.{}.{}.{}'::{}", 16 + (n >> 16) % 16, (n >> 8) % 256, n % 256, col.ty),
        ("U", "macaddr") => format!("'02:00:00:{:02x}:{:02x}:{:02x}'::{}", (n >> 16) % 256, (n >> 8) % 256, n % 256, col.ty),
        ("U", "macaddr8") => format!("'02:00:00:00:00:{:02x}:{:02x}:{:02x}'::{}", (n >> 16) % 256, (n >> 8) % 256, n % 256, col.ty),
        _ => return None,
    })
}

/// Whether `sql` runs, in a savepoint that keeps a failure from ending the transaction.
async fn try_execute(client: &Client, sql: &str) -> Result<bool, String> {
    match client.batch_execute(&format!("SAVEPOINT v; {sql}; RELEASE SAVEPOINT v")).await {
        Ok(()) => Ok(true),
        Err(_) => {
            client.batch_execute("ROLLBACK TO SAVEPOINT v; RELEASE SAVEPOINT v").await.map_err(|e| message(&e))?;
            Ok(false)
        }
    }
}

/// What a statement's placeholders are bound with.
struct Bound {
    /// The type of each placeholder `$n`, as the cast that names it.
    casts: Vec<String>,
    /// The values of each parameter of the analysis, as literals of its type.
    pools: Vec<Vec<String>>,
}

/// Each statement's parameters, or why it cannot be run. The types are the server's own answer
/// for the placeholders, by oid: the engine's `type_name` is the bare catalog name, which names
/// nothing, or another type, when the type's schema is not in `search_path`.
async fn parameters(client: &Client, statements: &[(String, String, Value)]) -> Result<Vec<Result<Bound, String>>, String> {
    let mut out = Vec::with_capacity(statements.len());
    let mut known = HashMap::new();
    client.batch_execute("BEGIN").await.map_err(|e| message(&e))?;
    let outcome = async {
        for (_, sql, analysis) in statements {
            out.push(bind(client, sql, analysis, &mut known).await?);
        }
        Ok::<(), String>(())
    }
    .await;
    client.batch_execute("ROLLBACK").await.map_err(|e| message(&e))?;
    outcome?;
    Ok(out)
}

/// One statement's parameters: NULL for a parameter the caller may pass null, the values of the
/// pool the parameter's type accepts, and a value no seeded row holds, so an INSERT of a new key
/// runs at all. A parameter no value is left for leaves the statement unchecked.
async fn bind(client: &Client, sql: &str, analysis: &Value, known: &mut HashMap<u32, Column>) -> Result<Result<Bound, String>, String> {
    let params = analysis["params"].as_array().cloned().unwrap_or_default();
    let order: Vec<&str> = analysis["rewritten"]["order"].as_array().map(|o| o.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    if params.is_empty() && order.is_empty() {
        return Ok(Ok(Bound { casts: Vec::new(), pools: Vec::new() }));
    }
    let rewritten = analysis["rewritten"]["sql"].as_str().unwrap_or(sql);
    client.batch_execute("SAVEPOINT p").await.map_err(|e| message(&e))?;
    let prepared = client.prepare(rewritten).await;
    client.batch_execute("ROLLBACK TO SAVEPOINT p; RELEASE SAVEPOINT p").await.map_err(|e| message(&e))?;
    let statement = match prepared {
        Ok(statement) => statement,
        Err(e) => return Ok(Err(format!("the server does not prepare it: {}", message(&e)))),
    };
    let mut types = Vec::with_capacity(statement.params().len());
    for ty in statement.params() {
        types.push(type_of(client, ty.oid(), known).await?);
    }
    let mut pools = Vec::with_capacity(params.len());
    for p in &params {
        let name = p["name"].as_str().unwrap_or("?");
        // Each placeholder `$n` is the n-th name of `order`.
        let Some(column) = order.iter().position(|n| *n == name).and_then(|n| types.get(n)) else {
            pools.push(vec!["NULL".to_string()]);
            continue;
        };
        let mut values: Vec<String> = accepted_pool(client, column, Scenario::Edges).await?.into_iter().filter(|v| v != "NULL" || p["nullable"].as_bool().unwrap_or(false)).collect();
        if let Some(fresh) = fresh_value(column, 900_000) {
            if try_execute(client, &format!("SELECT {fresh}")).await? {
                values.push(fresh);
            }
        }
        if values.iter().all(|v| v == "NULL") {
            return Ok(Err(format!("no seed value for parameter {name} of type {}", column.ty)));
        }
        pools.push(values);
    }
    Ok(Ok(Bound { casts: types.into_iter().map(|t| t.ty).collect(), pools }))
}

/// A statement's parameters, several assignments: each takes the next value of every pool.
fn assignments(pools: &[Vec<String>]) -> Vec<Vec<String>> {
    if pools.is_empty() {
        return vec![Vec::new()];
    }
    let width = pools.iter().map(Vec::len).max().unwrap_or(1);
    (0..width).map(|r| pools.iter().map(|p| p[r % p.len().max(1)].clone()).collect()).collect()
}

/// Whether the server refused a run with a NOT NULL or a CHECK violation, which a NULL bound to a
/// parameter may cause; a timeout or any other error is no such refusal.
fn refuses_null(e: &tokio_postgres::Error) -> bool {
    matches!(e.code(), Some(c) if *c == SqlState::NOT_NULL_VIOLATION || *c == SqlState::CHECK_VIOLATION)
}

fn message(e: &tokio_postgres::Error) -> String {
    e.as_db_error().map(|d| format!("{} ({})", d.message(), d.code().code())).unwrap_or_else(|| e.to_string())
}

/// Run the statements over seeded rows and report every contradiction, as lines.
pub async fn check(client: &Client, statements: &[(String, String, Value)], timeout_ms: u64) -> Result<Report, String> {
    let mut problems = Vec::new();
    let mut notes = Vec::new();
    let mut tallies: Vec<Tally> = statements.iter().map(|_| Tally::default()).collect();
    let tables = tables(client).await?;
    let delimiters = delimiters(client).await?;
    let bound = parameters(client, statements).await?;
    for (tally, bound) in tallies.iter_mut().zip(&bound) {
        tally.refused = bound.as_ref().err().cloned();
    }
    for scenario in SCENARIOS {
        // The timeout is this session's own, and ends with the transaction.
        client
            .batch_execute(&format!("BEGIN; SET LOCAL statement_timeout = {}; SET CONSTRAINTS ALL DEFERRED", timeout_ms.max(SEEDING_TIMEOUT_MS)))
            .await
            .map_err(|e| message(&e))?;
        let outcome = async {
            let mut bare = Vec::new();
            let seeded = seed(client, &tables, scenario, &mut bare).await?;
            let mut counts = Vec::with_capacity(seeded.len());
            for (table, inserted) in &seeded {
                // A table that refused every row may still hold the rows of setup.sql, or of its partitions.
                let empty = *inserted == 0 && client.query_one(&format!("SELECT NOT EXISTS (SELECT FROM {table})"), &[]).await.map_err(|e| message(&e))?.get::<_, bool>(0);
                counts.push(format!("{table} {inserted}{}", if empty { EMPTY } else { "" }));
            }
            notes.push(format!("seeded with {}: {}", scenario.name(), counts.join(", ")));
            if scenario == Scenario::Edges && !bare.is_empty() {
                notes.push(format!("no seed value for {}: the column holds NULL alone", bare.join(", ")));
            }
            client.batch_execute(&format!("SET LOCAL statement_timeout = {timeout_ms}")).await.map_err(|e| message(&e))?;
            for (((file, sql, analysis), tally), bound) in statements.iter().zip(tallies.iter_mut()).zip(&bound) {
                let Ok(bound) = bound else { continue };
                let rewritten = analysis["rewritten"]["sql"].as_str().unwrap_or(sql).to_string();
                let order: Vec<String> = analysis["rewritten"]["order"].as_array().map(|o| o.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                let params = analysis["params"].as_array().cloned().unwrap_or_default();
                // Each placeholder `$n` is the n-th name of `order`; the value is that parameter's.
                let fill = |base: &str, values: &[String]| {
                    let mut text = base.to_string();
                    for (n, name) in order.iter().enumerate().rev() {
                        let index = params.iter().position(|p| p["name"].as_str() == Some(name)).unwrap_or(n);
                        let literal = values.get(index).cloned().unwrap_or_else(|| "NULL".into());
                        let ty = bound.casts.get(n).cloned().unwrap_or_else(|| "text".into());
                        text = text.replace(&format!("${}", n + 1), &format!("({literal})::{ty}"));
                    }
                    text
                };
                let text_of = |values: &[String]| fill(&rewritten, values);
                // An order the engine says is total is checked on each run: no two rows in a row tie
                // on every sort key. A sort key that is no output column is read from a copy of the
                // statement that selects it too.
                let probe = if analysis["ordered"].as_bool() == Some(true) {
                    let names: Vec<String> = analysis["columns"].as_array().into_iter().flatten().map(|c| c["name"].as_str().unwrap_or("").to_string()).collect();
                    let probe = reylo_engine::postgres::cardinality::sort_probe(&rewritten, &names);
                    if probe.is_none() {
                        problems.push(format!("{file}: ordered, and the oracle cannot read its ORDER BY's sort keys to check it"));
                    }
                    probe
                } else {
                    None
                };
                for values in assignments(&bound.pools) {
                    let text = text_of(&values);
                    // The savepoint goes first on its own: a statement that does not parse fails
                    // the whole query text it is sent in, a savepoint written before it included.
                    client.batch_execute("SAVEPOINT q").await.map_err(|e| message(&e))?;
                    let messages = match client.simple_query(&text).await {
                        Ok(m) => m,
                        Err(e) => {
                            client.batch_execute("ROLLBACK TO SAVEPOINT q").await.map_err(|e| message(&e))?;
                            // A run the timeout stopped says nothing of the statement, sound or not.
                            if e.code() == Some(&SqlState::QUERY_CANCELED) {
                                tally.timed_out += 1;
                            } else {
                                tally.fail(message(&e));
                            }
                            if let Some(db) = e.as_db_error() {
                                let fields = [("constraint", db.constraint()), ("table", db.table()), ("column", db.column()), ("dataType", db.datatype()), ("schema", db.schema())];
                                if let Some(why) = crate::errors::contradiction(&analysis["errors"], db.code().code(), &fields) {
                                    problems.push(format!("{file}: {why} with ({})", values.join(", ")));
                                }
                            }
                            // A NULL the engine let a parameter take, which the database refuses
                            // with a NOT NULL or a CHECK violation, is a parameter typed nullable
                            // that may not be null. The NULLs are to blame where the same run with
                            // each of them given a value of its pool is not refused so; then each
                            // that is refused alone, the others given values, is named.
                            if refuses_null(&e) {
                                let nulls: Vec<usize> = (0..params.len()).filter(|&i| values.get(i).map(String::as_str) == Some("NULL") && params[i]["nullable"].as_bool().unwrap_or(false) && bound.pools.get(i).is_some_and(|pool| pool.iter().any(|v| v != "NULL"))).collect();
                                let filled = |keep: Option<usize>| -> Vec<String> {
                                    let mut other = values.clone();
                                    for &i in nulls.iter().filter(|&&i| Some(i) != keep) {
                                        other[i] = bound.pools[i].iter().find(|v| *v != "NULL").cloned().unwrap_or_default();
                                    }
                                    other
                                };
                                let mut blamed: Vec<(usize, String)> = Vec::new();
                                if !nulls.is_empty() {
                                    client.batch_execute("SAVEPOINT q").await.map_err(|e| message(&e))?;
                                    let outcome = client.simple_query(&text_of(&filled(None))).await;
                                    client.batch_execute("ROLLBACK TO SAVEPOINT q").await.map_err(|e| message(&e))?;
                                    if !outcome.as_ref().is_err_and(refuses_null) {
                                        for &i in &nulls {
                                            if nulls.len() == 1 {
                                                blamed.push((i, message(&e)));
                                                continue;
                                            }
                                            client.batch_execute("SAVEPOINT q").await.map_err(|e| message(&e))?;
                                            let outcome = client.simple_query(&text_of(&filled(Some(i)))).await;
                                            client.batch_execute("ROLLBACK TO SAVEPOINT q").await.map_err(|e| message(&e))?;
                                            if let Err(alone) = &outcome {
                                                if refuses_null(alone) {
                                                    blamed.push((i, message(alone)));
                                                }
                                            }
                                        }
                                    }
                                }
                                for (i, why) in blamed {
                                    problems.push(format!("{file}: parameter {} typed nullable, and the database refuses its NULL: {why} with ({})", params[i]["name"].as_str().unwrap_or("?"), values.join(", ")));
                                }
                            }
                            continue;
                        }
                    };
                    client.batch_execute("ROLLBACK TO SAVEPOINT q").await.map_err(|e| message(&e))?;
                    tally.ran += 1;
                    let rows: Vec<&tokio_postgres::SimpleQueryRow> = messages.iter().filter_map(|m| match m {
                        tokio_postgres::SimpleQueryMessage::Row(r) => Some(r),
                        _ => None,
                    }).collect();
                    let cardinality = analysis["cardinality"].as_str().unwrap_or("many");
                    if (cardinality == "at_most_one" && rows.len() > 1) || (cardinality == "one" && rows.len() != 1) {
                        problems.push(format!("{file}: proven {cardinality}, returned {} rows with ({})", rows.len(), values.join(", ")));
                    }
                    let columns = analysis["columns"].as_array().cloned().unwrap_or_default();
                    if let Some(why) = key_repeats(analysis, &columns, rows.iter().map(|row| (0..columns.len()).map(|i| row.try_get(i).ok().flatten()).collect())) {
                        problems.push(format!("{file}: {why} with ({})", values.join(", ")));
                    }
                    match &probe {
                        Some((None, at)) => {
                            if let Some(why) = ties(&rows, at) {
                                problems.push(format!("{file}: {why} with ({})", values.join(", ")));
                            }
                        }
                        Some((Some(wrapped), at)) => {
                            client.batch_execute("SAVEPOINT q").await.map_err(|e| message(&e))?;
                            let outcome = client.simple_query(&fill(wrapped, &values)).await;
                            client.batch_execute("ROLLBACK TO SAVEPOINT q").await.map_err(|e| message(&e))?;
                            match outcome {
                                Ok(messages) => {
                                    let rows: Vec<&tokio_postgres::SimpleQueryRow> = messages.iter().filter_map(|m| match m {
                                        tokio_postgres::SimpleQueryMessage::Row(r) => Some(r),
                                        _ => None,
                                    }).collect();
                                    if let Some(why) = ties(&rows, at) {
                                        problems.push(format!("{file}: {why} with ({})", values.join(", ")));
                                    }
                                }
                                Err(e) if e.code() == Some(&SqlState::QUERY_CANCELED) => {}
                                Err(e) => problems.push(format!("{file}: ordered, and the copy that selects its sort keys failed: {} with ({})", message(&e), values.join(", "))),
                            }
                        }
                        None => {}
                    }
                    for row in &rows {
                        for (i, col) in columns.iter().enumerate() {
                            let value = row.try_get(i).ok().flatten();
                            let name = col["name"].as_str().unwrap_or("?");
                            if value.is_none() {
                                if !col["nullable"].as_bool().unwrap_or(false) {
                                    problems.push(format!("{file}: column {name} typed non-null is NULL with ({})", values.join(", ")));
                                }
                                continue;
                            }
                            // The delimiter the catalog gives the array type, by its name without its schema.
                            let delimiter = col["type"]["name"].as_str().map(|n| n.rsplit('.').next().unwrap_or(n).trim_matches('"')).and_then(|t| delimiters.get(t)).copied().unwrap_or(',');
                            if let Some(why) = contradiction(&col["type"], value.unwrap_or_default(), col["nullable"].as_bool().unwrap_or(false), delimiter) {
                                problems.push(format!("{file}: column {name} {why} with ({})", values.join(", ")));
                            }
                        }
                    }
                }
            }
            Ok::<(), String>(())
        }
        .await;
        client.batch_execute("ROLLBACK").await.map_err(|e| message(&e))?;
        outcome?;
    }
    Ok(report(statements, &tallies, timeout_ms, problems, notes))
}

/// Why the rows of one run contradict an order the analysis says is total, if they do: two rows in
/// a row hold the same text, or both NULL, in every column `at` names, the sort keys.
fn ties(rows: &[&tokio_postgres::SimpleQueryRow], at: &[usize]) -> Option<String> {
    let keys = |row: &tokio_postgres::SimpleQueryRow| at.iter().map(|&i| row.try_get(i).ok().flatten().map(String::from)).collect::<Vec<Option<String>>>();
    rows.windows(2).enumerate().find_map(|(k, pair)| {
        let (a, b) = (keys(pair[0]), keys(pair[1]));
        (a == b).then(|| format!("ordered, and rows {} and {} tie on every sort key, as ({})", k + 1, k + 2, a.iter().map(|v| v.as_deref().map_or_else(|| "NULL".to_string(), |v| format!("'{v}'"))).collect::<Vec<_>>().join(", ")))
    })
}

/// Why the rows of one run contradict the analysis's result key, if they do: two rows agree on
/// every column of the key, a NULL counting as equal to another NULL. Each row is its columns'
/// text, in order, `None` for NULL.
fn key_repeats<'r>(analysis: &Value, columns: &[Value], rows: impl Iterator<Item = Vec<Option<&'r str>>>) -> Option<String> {
    let key = analysis["key"].as_array()?;
    let names: Vec<&str> = key.iter().map(|k| k.as_str().unwrap_or("?")).collect();
    let Some(at) = names.iter().map(|name| columns.iter().position(|c| c["name"].as_str() == Some(name))).collect::<Option<Vec<usize>>>() else {
        return Some(format!("key ({}) names a column the statement does not return", names.join(", ")));
    };
    let mut seen = HashSet::new();
    for row in rows {
        let values: Vec<Option<&str>> = at.iter().map(|&i| row.get(i).copied().flatten()).collect();
        if !seen.insert(values.clone()) {
            if names.is_empty() {
                return Some("an empty key says one row at most, and the statement returned more".into());
            }
            let shown: Vec<String> = values.iter().map(|v| v.map_or_else(|| "NULL".to_string(), |v| format!("'{v}'"))).collect();
            return Some(format!("key ({}) repeats in two rows, as ({})", names.join(", "), shown.join(", ")));
        }
    }
    None
}

/// Why a non-NULL value contradicts its type, if it does: a value outside the strings a text is
/// kept to, a text longer or shorter than its size, a NaN or an infinity in a float or a decimal
/// typed without one, a decimal of more digits than its precision and scale, a time, timestamp or
/// interval finer than its tick or with a part its type leaves out, a bit string or a vector of
/// another length, a NULL element in an array typed without one, its elements separated by
/// `delimiter`, or a null where JSON a statement built typed a value. A JSON `null` at the top is
/// the column's null to the run time, so it contradicts a column typed non-null.
fn contradiction(ty: &Value, text: &str, nullable: bool, delimiter: char) -> Option<String> {
    let ty: Type = serde_json::from_value(ty.clone()).ok()?;
    match &ty.kind {
        // A union of literals, from an enum or a CHECK list: the value is one of them.
        Kind::Text(TextType { values: Some(values), .. }) => (!values.is_empty() && !values.iter().any(|v| v == text)).then(|| format!("typed as one of {values:?} holds {text:?}")),
        // A type modifier's length, in code points: `varchar(20)` at most, `char(5)` exactly, as
        // it pads what it stores.
        Kind::Text(TextType { size: Some(size), ill_formed: None, .. }) if size.unit.is_none() => {
            let size = if ty.name.as_deref() == Some("bpchar") { size.clone() } else { Size { exact: None, ..size.clone() } };
            length_contradiction(&size, text.chars().count() as u64, "code points")
        }
        // Postgres prints a float's or a `numeric`'s non-finite values so.
        Kind::Float(FloatType { nan, infinity, .. }) | Kind::Decimal(DecimalType { nan, infinity, .. }) if matches!(text, "NaN" | "Infinity" | "-Infinity") => match text {
            "NaN" if !nan => Some("typed without NaN holds NaN".to_string()),
            "Infinity" | "-Infinity" if !infinity => Some(format!("typed without the infinities holds {text}")),
            _ => None,
        },
        Kind::Decimal(decimal) => decimal_contradiction(decimal, text),
        Kind::Time(time) => fraction_contradiction(&time.tick, text),
        Kind::DateTime(dt) => fraction_contradiction(&dt.tick, text),
        Kind::Instant(instant) => fraction_contradiction(&instant.tick, text),
        Kind::Duration(duration) => interval_contradiction(duration, text),
        // A bit string's length is its bits (`bit(7)`, `varbit(5)`), and a vector's its elements.
        Kind::Array(ArrayType { size: Some(size), .. }) if matches!(ty.name.as_deref(), Some("bit" | "varbit")) => length_contradiction(size, text.chars().count() as u64, "bits"),
        Kind::Array(ArrayType { size: Some(size), .. }) if ty.name.as_deref().is_some_and(|n| n.ends_with("vector") || n.ends_with("halfvec")) => {
            let inner = text.trim_start_matches('[').trim_end_matches(']').trim();
            length_contradiction(size, if inner.is_empty() { 0 } else { inner.split(',').count() as u64 }, "elements")
        }
        // A Postgres array, as its text shows it ([`postgres_array`]).
        Kind::Array(array) if postgres_array(&ty) => {
            // Each level over the innermost holds arrays, never NULL, so a NULL the text spells is
            // an element of the innermost level.
            let mut innermost = array;
            while let Kind::Array(inner) = &innermost.element.ty.kind {
                if !postgres_array(&innermost.element.ty) {
                    break;
                }
                innermost = inner;
            }
            depth_contradiction(&ty, text).or_else(|| (!innermost.element.nullable && array_has_null(text, delimiter)).then(|| "typed with non-null elements holds a NULL element".to_string()))
        }
        Kind::Json(JsonType { content: Some(content), .. }) => {
            let json: Value = serde_json::from_str(text).ok()?;
            if json.is_null() {
                return (!nullable).then(|| "holds the JSON null, typed non-null".to_string());
            }
            json_contradiction(&content.ty, &json, "")
        }
        Kind::Json(JsonType { schema: Some(_), .. }) => {
            let json: Value = serde_json::from_str(text).ok()?;
            json_contradiction(&ty, &json, "")
        }
        _ => None,
    }
}

/// Why a length breaks a size, if it does.
pub(crate) fn length_contradiction(size: &Size, n: u64, unit: &str) -> Option<String> {
    let ok = size.exact.is_none_or(|e| n == e) && size.min.is_none_or(|m| n >= m) && size.max.is_none_or(|m| n <= m);
    (!ok).then(|| format!("sized {size:?} holds {n} {unit}"))
}

/// Why a decimal's text breaks its precision and scale, if it does: more digits after the point
/// than its scale, or more digits than its precision. A negative scale rounds to tens or hundreds,
/// whose zeros the precision does not count, and a scale above the precision leaves zeros after the
/// point (`0.000123` in a `numeric(3,6)`), which it does not count either.
pub(crate) fn decimal_contradiction(decimal: &DecimalType, text: &str) -> Option<String> {
    let digits = text.trim_start_matches(['-', '+']);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty() && fraction.is_empty() || !whole.chars().chain(fraction.chars()).all(|c| c.is_ascii_digit()) {
        return None;
    }
    if decimal.max_scale.is_some_and(|max| fraction.len() as i32 > max.max(0)) {
        return Some(format!("typed with a scale of at most {:?} holds {text}", decimal.max_scale));
    }
    let precision = decimal.precision?;
    let whole = whole.trim_start_matches('0');
    let zeros = (-decimal.min_scale.unwrap_or(0)).max(0) as usize;
    if zeros > 0 && !whole.is_empty() && !whole.ends_with(&"0".repeat(zeros)) {
        return Some(format!("typed with a scale of {:?} holds {text}", decimal.min_scale));
    }
    let significant = format!("{whole}{fraction}").trim_start_matches('0').len().saturating_sub(zeros);
    (significant > precision as usize).then(|| format!("typed with a precision of {precision} holds {text}"))
}

/// The fractional digits a tick allows: 3 for `"1/1000"`, 0 for `"1"`, none for a coarser one.
pub(crate) fn tick_digits(tick: &str) -> Option<usize> {
    match tick.split_once('/') {
        Some(("1", denominator)) if denominator.starts_with('1') && denominator[1..].chars().all(|c| c == '0') => Some(denominator.len() - 1),
        None if tick == "1" => Some(0),
        _ => None,
    }
}

/// The `HH:MM:SS[.f]` a time, a timestamp's or an interval's text holds, as its three parts.
fn clock(text: &str) -> Option<(&str, &str, &str)> {
    let at = text.find(':')?;
    let start = text[..at].rfind(|c: char| !c.is_ascii_digit()).map_or(0, |i| i + 1);
    let rest = &text[start..];
    let hours = &rest[..rest.find(':')?];
    let rest = &rest[hours.len() + 1..];
    let minutes = rest.get(..2)?;
    let rest = rest.get(3..)?;
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
    Some((hours, minutes, &rest[..end]))
}

/// Why a time's or a timestamp's text is finer than its tick, if it is.
fn fraction_contradiction(tick: &str, text: &str) -> Option<String> {
    let allowed = tick_digits(tick)?;
    let (_, _, seconds) = clock(text)?;
    let digits = seconds.split_once('.').map_or(0, |(_, f)| f.len());
    (digits > allowed).then(|| format!("ticking by {tick} holds {text}"))
}

/// Why an interval's text has a part its type leaves out, or is finer than its tick, if it does.
fn interval_contradiction(duration: &DurationType, text: &str) -> Option<String> {
    let has_days = duration.max.split('T').next().is_some_and(|date| date.contains('D'));
    if !has_days && text.contains("day") {
        return Some(format!("typed without days holds {text}"));
    }
    let Some((hours, minutes, seconds)) = clock(text) else { return None };
    // Postgres prints a zero interval as `00:00:00`, whatever its fields.
    let zero = hours.trim_start_matches('-').chars().all(|c| c == '0') && minutes == "00" && seconds.chars().all(|c| c == '0' || c == '.');
    // A duration has a tick exactly when it has a time part.
    let Some(tick) = &duration.tick else {
        return (!zero).then(|| format!("typed without a time holds {text}"));
    };
    let fraction = seconds.split_once('.').map_or(0, |(_, f)| f.len());
    let whole_seconds = seconds.split('.').next().unwrap_or("");
    let coarse = match tick.as_str() {
        "3600" => minutes != "00" || whole_seconds != "00" || fraction > 0,
        "60" => whole_seconds != "00" || fraction > 0,
        tick => tick_digits(tick).is_some_and(|allowed| fraction > allowed),
    };
    coarse.then(|| format!("ticking by {tick} holds {text}"))
}

/// Whether a type structured as an array is one in Postgres's text too: a type written otherwise
/// (`bit`, `vector`) is not one here.
fn postgres_array(ty: &Type) -> bool {
    ty.name.as_deref().is_none_or(|n| n.rsplit('.').next().unwrap_or(n).starts_with('_'))
}

/// Why a Postgres array's text nests deeper or shallower than its type, if it does: a type fixes
/// one dimension per level of arrays it nests, and an `any_depth` level allows any number more.
/// An empty array has no dimensions, and fits any.
fn depth_contradiction(ty: &Type, text: &str) -> Option<String> {
    let body = if text.starts_with('[') { &text[text.find('=')? + 1..] } else { text };
    if body.trim() == "{}" {
        return None;
    }
    let depth = body.chars().take_while(|c| *c == '{').count();
    let (mut levels, mut open, mut at) = (0, false, ty);
    while let Kind::Array(array) = &at.kind {
        // A type written otherwise in Postgres's text (`vector`) adds no braces.
        if !postgres_array(at) {
            break;
        }
        levels += 1;
        if array.any_depth {
            open = true;
            break;
        }
        at = &array.element.ty;
    }
    (depth < levels || (!open && depth > levels)).then(|| format!("typed with {levels}{} dimensions holds {depth}", if open { " or more" } else { "" }))
}

fn array_has_null(text: &str, delimiter: char) -> bool {
    // An unquoted NULL between delimiters is a NULL element; a quoted "NULL" is a string.
    let mut in_quotes = false;
    let mut token = String::new();
    let mut found = false;
    let mut check = |token: &mut String| {
        if token.trim().eq_ignore_ascii_case("null") {
            found = true;
        }
        token.clear();
    };
    let mut escaped = false;
    for c in text.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' => in_quotes = !in_quotes,
            '{' | '}' if !in_quotes => check(&mut token),
            _ if c == delimiter && !in_quotes => check(&mut token),
            _ if in_quotes => {}
            _ => token.push(c),
        }
    }
    check(&mut token);
    found
}

fn json_contradiction(ty: &Type, json: &Value, path: &str) -> Option<String> {
    let at = if path.is_empty() { "the top" } else { path };
    match &ty.kind {
        // A JSON column a CHECK holds to a JSON Schema: the value is one the engine's schema
        // admits, a JSON null included.
        Kind::Json(JsonType { schema: Some(_), .. }) => {
            let schema = ty.schema().unwrap_or(Value::Bool(true));
            (!reylo_engine_core::schema_type::admits(&schema, json)).then(|| format!("holds {json} at {at}, which the schema {schema} does not admit"))
        }
        // A row value inside a document is an object of its fields, each key there.
        Kind::Record(record) if !written_as_text(ty) => {
            let Some(obj) = json.as_object() else { return Some(format!("holds {json} at {at}, typed an object")) };
            for f in &record.fields {
                let Some(v) = obj.get(&f.name) else { return Some(format!("has no key {path}.{}", f.name)) };
                // A null where the field's own schema admits one is the column's value.
                if v.is_null() && !f.ty.schema().is_some_and(|s| reylo_engine_core::schema_type::admits(&s, v)) {
                    if !f.nullable {
                        return Some(format!("has a null at {path}.{}, typed non-null", f.name));
                    }
                    continue;
                }
                if let Some(why) = json_contradiction(&f.ty, v, &format!("{path}.{}", f.name)) {
                    return Some(why);
                }
            }
            None
        }
        // An array inside a document is a JSON array of its elements, each of the element type.
        Kind::Array(array) if !written_as_text(ty) => {
            let Some(items) = json.as_array() else { return Some(format!("holds {json} at {at}, typed an array")) };
            for (i, item) in items.iter().enumerate() {
                if item.is_null() && !array.element.ty.schema().is_some_and(|s| reylo_engine_core::schema_type::admits(&s, item)) {
                    if !array.element.nullable {
                        return Some(format!("has a null element at {path}[{i}], typed non-null"));
                    }
                    continue;
                }
                if let Some(why) = json_contradiction(&array.element.ty, item, &format!("{path}[{i}]")) {
                    return Some(why);
                }
            }
            None
        }
        _ => (!json_admits(ty, json)).then(|| format!("holds {json} at {at}, typed as {}", ty.short())),
    }
}

/// Whether Postgres writes a value of the type inside a document as its text, though the format
/// structures it: a bit string, pgvector's vectors, the geometric types and the network types
/// (the spec's facts on what Postgres writes inside its own JSON).
fn written_as_text(ty: &Type) -> bool {
    let name = ty.name.as_deref().map(|n| n.rsplit('.').next().unwrap_or(n)).unwrap_or("");
    matches!(name, "bit" | "varbit" | "vector" | "halfvec" | "sparsevec" | "point" | "line" | "lseg" | "box" | "path" | "polygon" | "circle" | "inet" | "cidr")
}

/// Whether a JSON value inside a document is of the form Postgres writes a value of the type in
/// there (the spec's facts): a boolean, a number for the integers but `oid` and for the floats and
/// `numeric` (or the text of a non-finite one the type holds), an object for a row whose columns were not read,
/// for an hstore and as GeoJSON for a geometry, and text for every other type the format names
/// (a structured one written as its text among them). JSON, and a type the engine does not know,
/// admit any value.
fn json_admits(ty: &Type, json: &Value) -> bool {
    let name = ty.name.as_deref().map(|n| n.rsplit('.').next().unwrap_or(n)).unwrap_or("");
    match &ty.kind {
        _ if written_as_text(ty) => json.is_string(),
        Kind::Boolean => json.is_boolean(),
        Kind::Integer(_) if name == "oid" => json.is_string(),
        Kind::Integer(_) => json.is_number(),
        Kind::Float(FloatType { nan, infinity, .. }) | Kind::Decimal(DecimalType { nan, infinity, .. }) => {
            json.is_number() || (*nan && json.as_str() == Some("NaN")) || (*infinity && matches!(json.as_str(), Some("Infinity" | "-Infinity")))
        }
        Kind::Text(_) | Kind::Uuid | Kind::Date(_) | Kind::Time(_) | Kind::DateTime(_) | Kind::Instant(_) | Kind::Duration(_) | Kind::Bytes(_) | Kind::Range(_) | Kind::Multirange(_) | Kind::Xml(_) => json.is_string(),
        Kind::Map(_) if name == "hstore" => json.as_object().is_some_and(|map| map.values().all(|v| v.is_string() || v.is_null())),
        Kind::Geometry(_) if name == "geometry" => json.get("type").is_some_and(Value::is_string),
        Kind::Geometry(_) => json.is_string(),
        Kind::Opaque if name == "record" => json.is_object(),
        Kind::Union(u) => u.members.iter().any(|m| json_admits(m, json)),
        _ => true,
    }
}

// ---------------------------------------------------------------------------------------------
// SQLite
// ---------------------------------------------------------------------------------------------

/// The key an AUTOINCREMENT table is seeded up to: far above any other value, and far enough
/// below the largest integer that every keyless INSERT after it finds a rowid.
const AUTOINCREMENT_TOP: i64 = 1 << 62;

/// One table of a SQLite database, as the seeding fills it.
struct SqliteTable {
    name: String,
    /// The visible columns, by name and declared type.
    columns: Vec<(String, String)>,
    /// The column that is an AUTOINCREMENT key, which never takes the largest integer: the next
    /// INSERT without a key would find the table full.
    autoincrement: Option<usize>,
    rows: Rows,
}

/// How a table takes its rows.
#[derive(Debug, PartialEq)]
enum Rows {
    Inserted,
    /// An external-content FTS table: its index is rebuilt from the content table named here, and
    /// a row inserted into it would be an index entry for a row that is not there.
    Rebuilt(String),
    /// A contentless FTS table, which keeps no rows to read back: it is left empty.
    Left,
}

/// The `content` option of an FTS3, FTS4 or FTS5 table's declaration: the content table of an
/// external-content table, an empty name for a contentless one, and `None` for a table that
/// keeps its own rows, or is no FTS table.
fn fts_content(sql: &str) -> Option<String> {
    let lower = sql.to_ascii_lowercase();
    let using = lower.match_indices("using").map(|(at, _)| at).find(|at| lower[..*at].ends_with(char::is_whitespace) && lower[at + 5..].starts_with(char::is_whitespace))?;
    let module = lower[using + 5..].trim_start();
    if !["fts3", "fts4", "fts5"].iter().any(|m| module.starts_with(m)) {
        return None;
    }
    let open = using + lower[using..].find('(')?;
    let close = lower.rfind(')').filter(|close| *close > open)?;
    // The arguments, split at the commas outside quotes.
    let mut arguments = vec![String::new()];
    let mut quote: Option<char> = None;
    for c in sql[open + 1..close].chars() {
        match quote {
            Some(q) if c == q => quote = None,
            None if matches!(c, '\'' | '"' | '`') => quote = Some(c),
            None if c == '[' => quote = Some(']'),
            None if c == ',' => {
                arguments.push(String::new());
                continue;
            }
            _ => {}
        }
        arguments.last_mut().expect("one argument at least").push(c);
    }
    arguments.iter().find_map(|argument| {
        let (key, value) = argument.split_once('=')?;
        if !key.trim().eq_ignore_ascii_case("content") {
            return None;
        }
        let value = value.trim();
        let bare = ["''", "\"\"", "``", "[]"].iter().find_map(|q| value.strip_prefix(&q[..1]).and_then(|v| v.strip_suffix(&q[1..]))).unwrap_or(value);
        Some(bare.to_string())
    })
}

/// The tables the seeding fills, in the order they were created: no shadow table of a virtual
/// table, whose rows are the virtual table's to write.
fn sqlite_tables(db: &rusqlite::Connection) -> Result<Vec<SqliteTable>, String> {
    let listed: Vec<(String, String, String)> = {
        let mut stmt = db
            .prepare(
                "select l.name, l.type, coalesce(s.sql, '') from pragma_table_list l left join sqlite_schema s on s.type = 'table' and s.name = l.name \
                 where l.schema = 'main' and l.type in ('table', 'virtual') and l.name not like 'sqlite_%' order by s.rowid",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    let mut out = Vec::new();
    for (name, kind, sql) in listed {
        let columns: Vec<(String, String, bool, i64)> = {
            let mut stmt = db.prepare(&format!("select name, type, hidden, pk from pragma_table_xinfo('{}')", name.replace('\'', "''"))).map_err(|e| e.to_string())?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)? != 0, r.get::<_, i64>(3)?))).map_err(|e| e.to_string())?;
            rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
        };
        let columns: Vec<(String, String, i64)> = columns.into_iter().filter(|(_, _, hidden, _)| !hidden).map(|(n, t, _, pk)| (n, t, pk)).collect();
        if columns.is_empty() {
            continue;
        }
        let autoincrement = if kind == "table" && sql.to_ascii_lowercase().contains("autoincrement") { columns.iter().position(|(_, _, pk)| *pk > 0) } else { None };
        let rows = match fts_content(&sql).filter(|_| kind == "virtual") {
            Some(content) if content.is_empty() => Rows::Left,
            Some(content) => Rows::Rebuilt(content),
            None => Rows::Inserted,
        };
        out.push(SqliteTable { name, columns: columns.into_iter().map(|(n, t, _)| (n, t)).collect(), autoincrement, rows });
    }
    Ok(out)
}

/// Run `run` with the connection interrupted once `ms` have passed.
pub(crate) fn with_deadline<T>(db: &rusqlite::Connection, ms: u64, run: impl FnOnce() -> T) -> T {
    let deadline = Instant::now() + Duration::from_millis(ms);
    let _ = db.progress_handler(1000, Some(move || Instant::now() >= deadline));
    let out = run();
    let _ = db.progress_handler(0, None::<fn() -> bool>);
    out
}

/// Whether SQLite refused a run with a NOT NULL or a CHECK constraint, which a NULL bound to a
/// parameter may cause; any other error is no such refusal.
fn sqlite_refuses_null(e: &rusqlite::Error) -> bool {
    let text = e.to_string();
    matches!(e, rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation) && (text.contains("NOT NULL constraint failed") || text.contains("CHECK constraint failed"))
}

fn interrupted(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::OperationInterrupted)
}

/// The SQLite half of the same check, each state in one transaction that is rolled back. Foreign
/// keys are off, as they are by default, so a row may reference nothing and an outer join extends
/// it with NULLs. A pool per declared type, by SQLite's affinity rules; a statement runs once per
/// assignment of its parameters, each bound as a value. SQLite itself rolls the whole transaction
/// back on some errors (SQLITE_FULL on an INSERT of one row, an interrupted write): the state is
/// then seeded again, so the runs after it see the same rows.
pub fn check_sqlite(db: &rusqlite::Connection, statements: &[(String, String, Value)], timeout_ms: u64) -> Result<Report, String> {
    let mut problems = Vec::new();
    let mut notes = Vec::new();
    let mut tallies: Vec<Tally> = statements.iter().map(|_| Tally::default()).collect();
    let tables = sqlite_tables(db)?;
    for scenario in SCENARIOS {
        let mut refused = HashSet::new();
        let seeded = begin_seeded(db, &tables, scenario, timeout_ms, &mut refused)?;
        notes.push(format!("seeded with {}: {seeded}", scenario.name()));
        let mut again = 0;
        let outcome = (|| -> Result<(), String> {
            for ((file, sql, analysis), tally) in statements.iter().zip(tallies.iter_mut()) {
                let text = analysis["rewritten"]["sql"].as_str().unwrap_or(sql).to_string();
                let params = analysis["params"].as_array().cloned().unwrap_or_default();
                let order: Vec<String> = analysis["rewritten"]["order"].as_array().map(|o| o.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                // A parameter's pool ends with a value no seeded row holds, one of its own, so a
                // write of a new unique key runs at all, as on Postgres.
                let pools: Vec<Vec<rusqlite::types::Value>> = params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let declared = sqlite_declared(&p["type"]);
                        let declared = declared.as_str();
                        let mut values = sqlite_pool(declared, Scenario::Edges, false);
                        if !p["nullable"].as_bool().unwrap_or(false) {
                            values.retain(|v| *v != rusqlite::types::Value::Null);
                        }
                        values.push(sqlite_fresh(declared, 900_000 + i));
                        values
                    })
                    .collect();
                let width = pools.iter().map(Vec::len).max().unwrap_or(1).max(1);
                let cardinality = analysis["cardinality"].as_str().unwrap_or("many");
                let columns = analysis["columns"].as_array().cloned().unwrap_or_default();
                // Each `?` is the n-th name of `order`, and the value is that parameter's.
                let bind = |values: &[rusqlite::types::Value]| -> Vec<rusqlite::types::Value> {
                    order.iter().map(|name| params.iter().position(|p| p["name"].as_str() == Some(name)).and_then(|i| values.get(i).cloned()).unwrap_or(rusqlite::types::Value::Null)).collect()
                };
                for r in 0..width {
                    let values: Vec<rusqlite::types::Value> = pools.iter().map(|p| p.get(r % p.len().max(1)).cloned().unwrap_or(rusqlite::types::Value::Null)).collect();
                    let bound = bind(&values);
                    let shown = bound.iter().map(|v| format!("{v:?}")).collect::<Vec<_>>().join(", ");
                    db.execute_batch("SAVEPOINT q").map_err(|e| e.to_string())?;
                    // The rows are checked as they come, so a statement that returns rows without
                    // end holds none of them. A row returned before an error or the timeout was
                    // returned all the same, and a value in it that contradicts a type is reported.
                    let mut found = Vec::new();
                    let (returned, outcome) = run_sqlite_statement(db, &text, &bound, timeout_ms, |row| {
                        for (i, col) in columns.iter().enumerate() {
                            let name = col["name"].as_str().unwrap_or("?");
                            match row.get(i) {
                                Some(rusqlite::types::Value::Null) if !col["nullable"].as_bool().unwrap_or(false) => {
                                    found.push(format!("{file}: column {name} typed non-null is NULL with ({shown})"));
                                }
                                // A truth value SQLite computes is 1 or 0; a column declared
                                // BOOLEAN holds what was stored, which the run time refuses unless
                                // it is one of them.
                                Some(value) if col["type"]["kind"] == "boolean" && !matches!(value, rusqlite::types::Value::Null | rusqlite::types::Value::Integer(0 | 1)) => {
                                    found.push(format!("{file}: column {name} typed bool holds {value:?} with ({shown})"));
                                }
                                // A column declared 64 bits wide, and what passes one through,
                                // is a BigInt to the run time, which only an INTEGER is.
                                Some(value) if col["type"]["kind"] == "integer" && col["type"]["name"] == "BIGINT" && !matches!(value, rusqlite::types::Value::Null | rusqlite::types::Value::Integer(_)) => {
                                    found.push(format!("{file}: column {name} typed int8 holds {value:?} with ({shown})"));
                                }
                                // JSON a constructor built is text that parses as that JSON,
                                // each value of the type the engine gave it.
                                Some(value) if is_built(&col["type"]) && *value != rusqlite::types::Value::Null => {
                                    if let Some(why) = built_contradiction(&col["type"], value) {
                                        found.push(format!("{file}: column {name} {why} with ({shown})"));
                                    }
                                }
                                Some(rusqlite::types::Value::Text(text)) => {
                                    if let Some(why) = contradiction(&col["type"], text, col["nullable"].as_bool().unwrap_or(false), ',') {
                                        found.push(format!("{file}: column {name} {why} with ({shown})"));
                                    }
                                }
                                _ => {}
                            }
                        }
                    });
                    if db.is_autocommit() {
                        // The error ended the transaction, and the seeded rows went with it.
                        begin_seeded(db, &tables, scenario, timeout_ms, &mut refused)?;
                        again += 1;
                    } else {
                        db.execute_batch("ROLLBACK TO q; RELEASE q").map_err(|e| e.to_string())?;
                    }
                    found.sort();
                    found.dedup();
                    problems.extend(found);
                    match outcome {
                        Ok(()) => {
                            tally.ran += 1;
                            if (cardinality == "at_most_one" && returned > 1) || (cardinality == "one" && returned != 1) {
                                problems.push(format!("{file}: proven {cardinality}, returned {returned} rows with ({shown})"));
                            }
                        }
                        Err(e) => {
                            // A run the timeout stopped says nothing of the statement, sound or not.
                            if interrupted(&e) {
                                tally.timed_out += 1;
                            } else {
                                tally.fail(e.to_string());
                            }
                            if let rusqlite::Error::SqliteFailure(failure, message) = &e {
                                let message = message.clone().unwrap_or_default();
                                if let Some(why) = crate::errors::sqlite_contradiction(&analysis["errors"], failure.extended_code, &message) {
                                    problems.push(format!("{file}: {why} with ({shown})"));
                                }
                            }
                            if cardinality != "many" && returned > 1 {
                                problems.push(format!("{file}: proven {cardinality}, returned {returned} rows before it failed with ({shown})"));
                            }
                            // A NULL the engine let a parameter take, which SQLite refuses with a
                            // NOT NULL or a CHECK constraint, is a parameter typed nullable that
                            // may not be null. The NULLs are to blame where the same run with each
                            // of them given a value of its pool is not refused so; then each that
                            // is refused alone, the others given values, is named.
                            if sqlite_refuses_null(&e) {
                                use rusqlite::types::Value::Null;
                                let nulls: Vec<usize> = (0..params.len()).filter(|&i| values.get(i) == Some(&Null) && params[i]["nullable"].as_bool().unwrap_or(false) && pools.get(i).is_some_and(|pool| pool.iter().any(|v| *v != Null))).collect();
                                let filled = |keep: Option<usize>| -> Vec<rusqlite::types::Value> {
                                    let mut other = values.clone();
                                    for &i in nulls.iter().filter(|&&i| Some(i) != keep) {
                                        other[i] = pools[i].iter().find(|v| **v != Null).cloned().unwrap_or(Null);
                                    }
                                    other
                                };
                                let mut probe = |values: Vec<rusqlite::types::Value>| -> Result<Option<String>, String> {
                                    db.execute_batch("SAVEPOINT q").map_err(|e| e.to_string())?;
                                    let (_, outcome) = run_sqlite_statement(db, &text, &bind(&values), timeout_ms, |_| {});
                                    if db.is_autocommit() {
                                        begin_seeded(db, &tables, scenario, timeout_ms, &mut refused)?;
                                        again += 1;
                                    } else {
                                        db.execute_batch("ROLLBACK TO q; RELEASE q").map_err(|e| e.to_string())?;
                                    }
                                    Ok(outcome.err().filter(sqlite_refuses_null).map(|e| e.to_string()))
                                };
                                if !nulls.is_empty() && probe(filled(None))?.is_none() {
                                    for &i in &nulls {
                                        let why = if nulls.len() == 1 { Some(e.to_string()) } else { probe(filled(Some(i)))? };
                                        if let Some(why) = why {
                                            problems.push(format!("{file}: parameter {} typed nullable, and the database refuses its NULL: {why} with ({shown})", params[i]["name"].as_str().unwrap_or("?")));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        })();
        if !db.is_autocommit() {
            db.execute_batch("ROLLBACK").map_err(|e| e.to_string())?;
        }
        outcome?;
        if again > 0 {
            notes.push(format!("seeded again {again} time(s) with {}: a statement's error ended the transaction", scenario.name()));
        }
    }
    Ok(report(statements, &tallies, timeout_ms, problems, notes))
}

/// Whether a SQLite column's type is JSON a constructor built (an object, a list, or the JSON
/// Schema of a tuple or of an object of any keys), which the run time parses.
fn is_built(ty: &Value) -> bool {
    ty["kind"] == "json" && (ty.get("content").is_some() || ty.get("schema").is_some())
}

/// The declared type a SQLite parameter's pool is laid out by: the affinity's name the engine's
/// type for it stands for.
fn sqlite_declared(ty: &Value) -> String {
    let Ok(ty) = serde_json::from_value::<Type>(ty.clone()) else { return String::new() };
    match (&ty.kind, ty.name.as_deref()) {
        (Kind::Integer(_), Some("BIGINT")) => "int8",
        (Kind::Integer(_), _) => "integer",
        (Kind::Float(_), _) => "real",
        (Kind::Text(_), _) => "text",
        (Kind::Bytes(_), _) => "blob",
        (Kind::Boolean, _) => "bool",
        (Kind::Union(_), Some(name)) => return name.to_string(),
        _ => "",
    }
    .to_string()
}

/// Why a SQLite value typed as built JSON contradicts its type, if it does: it is no text, the
/// text is no JSON, or the JSON is not of the type, down to each value ([`built_value`]). SQLite
/// writes an infinity as `9.0e+999`, which serde_json refuses as out of range and JSON.parse
/// reads as one.
fn built_contradiction(ty: &Value, value: &rusqlite::types::Value) -> Option<String> {
    let ty: Type = serde_json::from_value(ty.clone()).ok()?;
    let rusqlite::types::Value::Text(text) = value else { return Some(format!("typed as built JSON holds {value:?}, which is no text")) };
    let parsed = serde_json::from_str::<Value>(text).or_else(|_| serde_json::from_str::<Value>(&text.replace("9.0e+999", "1.0e308")));
    let content = match &ty.kind {
        Kind::Json(JsonType { content: Some(content), .. }) => content.ty.clone(),
        _ => ty,
    };
    match parsed {
        Ok(json) => built_value(&content, &json, ""),
        Err(e) => Some(format!("typed as built JSON holds {text:?}, which is no JSON ({e})")),
    }
}

/// Why a value inside a document SQLite built contradicts its type: an INTEGER (a 64-bit column's
/// too) is a JSON integer, a REAL a number, a TEXT a string, a NUMERIC a number or a string; an
/// object has exactly its keys, each null only where it may be; a list's items likewise; a JSON
/// Schema admits the value. Any JSON (`unknown`, `json` without a schema) admits null.
fn built_value(ty: &Type, json: &Value, path: &str) -> Option<String> {
    let at = if path.is_empty() { "the top".to_string() } else { path.to_string() };
    let admits_null = |t: &Type| t.is_unknown() || matches!(&t.kind, Kind::Json(JsonType { schema: None, content: None }));
    match &ty.kind {
        Kind::Integer(_) | Kind::Float(_) | Kind::Text(_) | Kind::Union(_) => {
            let fits = ty.members().iter().any(|m| match &m.kind {
                Kind::Integer(_) => json.is_i64() || json.is_u64(),
                Kind::Float(_) => json.is_number(),
                Kind::Text(_) => json.is_string(),
                _ => false,
            });
            (!fits).then(|| format!("holds {json} at {at}, typed {}", ty.short()))
        }
        Kind::Record(record) => {
            let Some(obj) = json.as_object() else { return Some(format!("holds {json} at {at}, typed an object")) };
            if let Some(extra) = obj.keys().find(|k| !record.fields.iter().any(|f| &f.name == *k)) {
                return Some(format!("has the key {extra:?} at {at}, which its type lacks"));
            }
            for f in &record.fields {
                let Some(v) = obj.get(&f.name) else { return Some(format!("lacks the key {:?} at {at}", f.name)) };
                if v.is_null() {
                    if !f.nullable && !admits_null(&f.ty) {
                        return Some(format!("has a null at {path}.{}, typed non-null", f.name));
                    }
                    continue;
                }
                if let Some(why) = built_value(&f.ty, v, &format!("{path}.{}", f.name)) {
                    return Some(why);
                }
            }
            None
        }
        Kind::Array(array) => {
            let Some(items) = json.as_array() else { return Some(format!("holds {json} at {at}, typed a list")) };
            for (i, item) in items.iter().enumerate() {
                if item.is_null() {
                    if !array.element.nullable && !admits_null(&array.element.ty) {
                        return Some(format!("has a null item at {path}[{i}], typed non-null"));
                    }
                    continue;
                }
                if let Some(why) = built_value(&array.element.ty, item, &format!("{path}[{i}]")) {
                    return Some(why);
                }
            }
            None
        }
        Kind::Json(JsonType { schema: Some(_), .. }) => {
            let schema = ty.schema().unwrap_or(Value::Bool(true));
            (!reylo_engine_core::schema_type::admits(&schema, json)).then(|| format!("holds {json} at {at}, which the schema {schema} does not admit"))
        }
        _ => None,
    }
}

/// Run one statement under the timeout, each row handed to `each` as it comes: the rows it
/// returned, and its error if it failed.
fn run_sqlite_statement(db: &rusqlite::Connection, sql: &str, values: &[rusqlite::types::Value], timeout_ms: u64, mut each: impl FnMut(&[rusqlite::types::Value])) -> (usize, Result<(), rusqlite::Error>) {
    let mut returned = 0;
    let outcome = with_deadline(db, timeout_ms, || {
        let mut stmt = db.prepare(sql)?;
        let width = stmt.column_count();
        let mut rows = stmt.query(rusqlite::params_from_iter(values.iter()))?;
        while let Some(row) = rows.next()? {
            let row: Vec<rusqlite::types::Value> = (0..width).map(|i| row.get(i)).collect::<Result<_, _>>()?;
            returned += 1;
            each(&row);
        }
        Ok(())
    });
    (returned, outcome)
}

/// The edge values of a declared SQLite type in a state, NULL first, by the affinity SQLite gives
/// the name. The largest integer is in the edge state alone, and never in an AUTOINCREMENT key.
fn sqlite_pool(declared: &str, scenario: Scenario, autoincrement: bool) -> Vec<rusqlite::types::Value> {
    use rusqlite::types::Value::{Blob, Integer, Null, Real, Text};
    let t = declared.to_ascii_lowercase();
    let text = || vec![Text("a".into()), Text("A".into()), Text(String::new()), Text("a ".into())];
    let edges = scenario == Scenario::Edges;
    let mut out = vec![Null];
    if t.contains("int") {
        out.extend([Integer(1), Integer(2), Integer(0), Integer(-1)]);
        if edges {
            out.push(Integer(if autoincrement { AUTOINCREMENT_TOP } else { i64::MAX }));
        }
    } else if t.contains("char") || t.contains("clob") || t.contains("text") {
        out.extend(text());
    } else if t.contains("blob") {
        out.extend([Blob(vec![0]), Blob(Vec::new())]);
    } else if t.contains("real") || t.contains("floa") || t.contains("doub") {
        out.extend([Real(1.5), Real(0.0), Real(-0.0)]);
        if edges {
            out.push(Real(1e308));
        }
    } else if t.is_empty() {
        out.extend([Integer(1), Text("a".into())]);
    } else {
        out.extend([Integer(1), Real(1.5), Text("1".into()), Text("x".into())]);
    }
    out
}

/// A value of a declared SQLite type no seeded row holds, the `n`th, by the affinity SQLite gives
/// the name.
fn sqlite_fresh(declared: &str, n: usize) -> rusqlite::types::Value {
    use rusqlite::types::Value::{Blob, Integer, Real, Text};
    let t = declared.to_ascii_lowercase();
    if t.contains("int") {
        Integer(n as i64)
    } else if t.contains("char") || t.contains("clob") || t.contains("text") || t.is_empty() {
        Text(format!("k{n}"))
    } else if t.contains("blob") {
        Blob(n.to_be_bytes().to_vec())
    } else if t.contains("real") || t.contains("floa") || t.contains("doub") {
        Real(n as f64 + 0.5)
    } else {
        Integer(n as i64)
    }
}

/// Open the transaction and seed it. When SQLite ends the transaction over a row's insert, the
/// rows before it are gone too: the seeding starts again, without that row.
fn begin_seeded(db: &rusqlite::Connection, tables: &[SqliteTable], scenario: Scenario, timeout_ms: u64, refused: &mut HashSet<(usize, usize)>) -> Result<String, String> {
    loop {
        db.execute_batch("BEGIN").map_err(|e| e.to_string())?;
        if let Some(seeded) = seed_sqlite(db, tables, scenario, timeout_ms, refused)? {
            return Ok(seeded);
        }
    }
}

/// Seed every table with a base row and one row per pool value of each column; a row that breaks
/// a unique key gets fresh values in the key's columns and is tried again. An FTS table that
/// indexes another table's rows is rebuilt from them once they are in. What each table got, or
/// `None` when a row's insert ended the transaction: the row is then in `refused`.
fn seed_sqlite(db: &rusqlite::Connection, tables: &[SqliteTable], scenario: Scenario, timeout_ms: u64, refused: &mut HashSet<(usize, usize)>) -> Result<Option<String>, String> {
    let quoted = |name: &str| format!("\"{}\"", name.replace('"', "\"\""));
    let mut out = Vec::new();
    let mut fresh = 0i64;
    for (t, table) in tables.iter().enumerate() {
        if table.rows != Rows::Inserted {
            continue;
        }
        let columns = &table.columns;
        let pools: Vec<Vec<rusqlite::types::Value>> = columns.iter().enumerate().map(|(i, (_, ty))| sqlite_pool(ty, scenario, table.autoincrement == Some(i))).collect();
        let base: Vec<rusqlite::types::Value> = pools.iter().map(|p| p.get(1).cloned().unwrap_or(rusqlite::types::Value::Null)).collect();
        let mut rows = vec![base.clone(), vec![rusqlite::types::Value::Null; columns.len()]];
        for (i, pool) in pools.iter().enumerate() {
            for v in pool {
                let mut row = base.clone();
                row[i] = v.clone();
                rows.push(row);
            }
        }
        let names: Vec<String> = columns.iter().map(|(n, _)| quoted(n)).collect();
        let marks: Vec<&str> = columns.iter().map(|_| "?").collect();
        let sql = format!("insert into {} ({}) values ({})", quoted(&table.name), names.join(", "), marks.join(", "));
        let mut inserted = 0;
        for (r, mut row) in rows.into_iter().enumerate() {
            if refused.contains(&(t, r)) {
                continue;
            }
            for _ in 0..4 {
                let outcome = with_deadline(db, timeout_ms.max(SEEDING_TIMEOUT_MS), || db.execute(&sql, rusqlite::params_from_iter(row.iter())));
                if db.is_autocommit() {
                    refused.insert((t, r));
                    return Ok(None);
                }
                match outcome {
                    Ok(_) => {
                        inserted += 1;
                        break;
                    }
                    Err(rusqlite::Error::SqliteFailure(e, Some(message))) if e.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE || e.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY => {
                        // "UNIQUE constraint failed: t.a, t.b"
                        let failed: Vec<String> = message.split(": ").nth(1).unwrap_or("").split(", ").filter_map(|c| c.split('.').nth(1).map(String::from)).collect();
                        let mut changed = false;
                        for column in failed {
                            if let Some(i) = columns.iter().position(|(n, _)| *n == column) {
                                fresh += 1;
                                row[i] = if columns[i].1.to_ascii_lowercase().contains("int") { rusqlite::types::Value::Integer(1000 + fresh) } else { rusqlite::types::Value::Text(format!("k{fresh}")) };
                                changed = true;
                            }
                        }
                        if !changed {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        let empty = inserted == 0 && db.query_row(&format!("select not exists (select 1 from {})", quoted(&table.name)), [], |r| r.get::<_, bool>(0)).unwrap_or(false);
        out.push(format!("{} {inserted}{}", table.name, if empty { EMPTY } else { "" }));
    }
    for table in tables {
        match &table.rows {
            Rows::Inserted => {}
            Rows::Left => out.push(format!("{} left empty (contentless)", table.name)),
            Rows::Rebuilt(content) => {
                let outcome = with_deadline(db, timeout_ms.max(SEEDING_TIMEOUT_MS), || db.execute_batch(&format!("insert into {0} ({0}) values ('rebuild')", quoted(&table.name))));
                if db.is_autocommit() {
                    return Err(format!("rebuilding {} from {content} ended the transaction: {}", table.name, outcome.err().map(|e| e.to_string()).unwrap_or_default()));
                }
                out.push(match outcome {
                    Ok(()) => format!("{} rebuilt from {content}", table.name),
                    Err(e) => format!("{} not rebuilt from {content} ({e})", table.name),
                });
            }
        }
    }
    Ok(Some(out.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::types::Value::{Integer, Real};
    use serde_json::json;

    fn column(category: &str, base: &str) -> Column {
        Column { name: "c".into(), ty: base.into(), category: category.into(), kind: "b".into(), base: base.into(), is_array: false, delimiter: ",".into(), labels: Vec::new(), generated: false }
    }

    #[test]
    fn built_json_is_held_to_its_type_down_to_each_value() {
        let text = |s: &str| rusqlite::types::Value::Text(s.into());
        let utf8 = json!({"kind": "text", "ill_formed": "utf8"});
        let ty = json!({"kind": "json", "content": {"type": {"kind": "record", "fields": [
            {"name": "id", "type": {"kind": "integer", "bits": 64}},
            {"name": "r", "type": {"kind": "float", "exponent_bits": 11, "mantissa_bits": 52, "infinity": true}, "nullable": true},
            {"name": "doc", "type": {"kind": "unknown"}},
            {"name": "tags", "type": {"kind": "array", "element": {"type": utf8}}},
            {"name": "pair", "type": {"kind": "json", "schema": {"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "array", "prefixItems": [{"type": "integer"}, {"type": "string"}], "items": false, "minItems": 2}}},
        ]}}});
        assert_eq!(built_contradiction(&ty, &text(r#"{"id":1,"r":9.0e+999,"doc":null,"tags":["a"],"pair":[1,"a"]}"#)), None);
        assert_eq!(built_contradiction(&ty, &text(r#"{"id":1,"r":null,"doc":{"a":1},"tags":[],"pair":[1,"a"]}"#)), None);
        for (bad, why) in [
            (r#"{"id":1.5,"r":1,"doc":1,"tags":[],"pair":[1,"a"]}"#, "holds 1.5 at .id, typed int64"),
            (r#"{"id":"1","r":1,"doc":1,"tags":[],"pair":[1,"a"]}"#, "holds \"1\" at .id, typed int64"),
            (r#"{"r":1,"doc":1,"tags":[],"pair":[1,"a"]}"#, "lacks the key \"id\" at the top"),
            (r#"{"id":1,"r":1,"doc":1,"tags":[],"pair":[1,"a"],"x":1}"#, "has the key \"x\" at the top, which its type lacks"),
            (r#"{"id":null,"r":1,"doc":1,"tags":[],"pair":[1,"a"]}"#, "has a null at .id, typed non-null"),
            (r#"{"id":1,"r":1,"doc":1,"tags":[null],"pair":[1,"a"]}"#, "has a null item at .tags[0], typed non-null"),
            (r#"{"id":1,"r":1,"doc":1,"tags":"[]","pair":[1,"a"]}"#, "holds \"[]\" at .tags, typed a list"),
            (r#"[1]"#, "holds [1] at the top, typed an object"),
        ] {
            assert_eq!(built_contradiction(&ty, &text(bad)).as_deref(), Some(why), "{bad}");
        }
        assert!(built_contradiction(&ty, &text(r#"{"id":1,"r":1,"doc":1,"tags":[],"pair":[1,2]}"#)).is_some_and(|why| why.contains("does not admit")));
        assert!(built_contradiction(&ty, &text("not json")).is_some_and(|why| why.contains("which is no JSON")));
        assert_eq!(built_contradiction(&ty, &Integer(1)).as_deref(), Some("typed as built JSON holds Integer(1), which is no text"));
    }

    #[test]
    fn a_postgres_array_nests_as_deep_as_its_type_says() {
        let int4 = json!({"kind": "integer", "bits": 32, "name": "int4"});
        let column = json!({"kind": "array", "any_depth": true, "name": "_int4", "element": {"nullable": true, "type": int4}});
        let flat = json!({"kind": "array", "name": "_int4", "element": {"nullable": true, "type": int4}});
        let rows = json!({"kind": "array", "name": "_int4", "element": {"type": {"kind": "array", "name": "_int4", "element": {"type": int4}}}});
        let stacked = json!({"kind": "array", "name": "_int4", "element": {"type": column}});
        // A column's array holds any depth; one a statement builds holds the depth it shows.
        assert_eq!(contradiction(&column, "{{1,2},{3,4}}", false, ','), None);
        assert_eq!(contradiction(&flat, "{1,NULL}", false, ','), None);
        assert_eq!(contradiction(&flat, "{{1,2}}", false, ',').as_deref(), Some("typed with 1 dimensions holds 2"));
        assert_eq!(contradiction(&rows, "{{1,2},{3,4}}", false, ','), None);
        assert_eq!(contradiction(&rows, "{1,2}", false, ',').as_deref(), Some("typed with 2 dimensions holds 1"));
        assert_eq!(contradiction(&stacked, "[0:1]={{{1}},{{2}}}", false, ','), None);
        assert_eq!(contradiction(&stacked, "{1}", false, ',').as_deref(), Some("typed with 2 or more dimensions holds 1"));
        // An empty array fits any depth, and a NULL is an element of the innermost level.
        assert_eq!(contradiction(&rows, "{}", false, ','), None);
        assert_eq!(contradiction(&rows, "{{1,NULL}}", false, ',').as_deref(), Some("typed with non-null elements holds a NULL element"));
        assert_eq!(contradiction(&stacked, "{{1,NULL}}", false, ','), None);
        // A vector is an array in the type but no braces in Postgres's text, so it adds no dimension.
        let float4 = json!({"kind": "float", "exponent_bits": 8, "mantissa_bits": 23});
        let vector = json!({"kind": "array", "name": "public.vector", "element": {"type": float4}});
        let vectors = json!({"kind": "array", "name": "public._vector", "element": {"type": {"kind": "array", "name": "public._vector", "element": {"type": vector}}}});
        assert_eq!(contradiction(&vectors, r#"{{"[1,2]"}}"#, false, ','), None);
        assert_eq!(contradiction(&vectors, r#"{"[1,2]"}"#, false, ',').as_deref(), Some("typed with 2 dimensions holds 1"));
    }

    #[test]
    fn a_postgres_document_is_of_the_kind_its_type_says() {
        let int8 = json!({"kind": "integer", "bits": 64, "name": "int8"});
        let list = json!({"kind": "array", "element": {"type": int8}});
        let document = |content: &Value| json!({"kind": "json", "name": "jsonb", "content": {"type": content}});
        let object = json!({"kind": "record", "fields": [{"name": "ids", "type": list.clone()}]});
        assert_eq!(contradiction(&document(&list), "[1, 2]", false, ','), None);
        assert_eq!(contradiction(&document(&object), r#"{"ids": []}"#, false, ','), None);
        // `coalesce(jsonb_agg(…), '{}')` over no rows is `{}`, which no list type admits.
        assert_eq!(contradiction(&document(&list), "{}", false, ',').as_deref(), Some("holds {} at the top, typed an array"));
        assert_eq!(contradiction(&document(&object), "[]", false, ',').as_deref(), Some("holds [] at the top, typed an object"));
        assert_eq!(contradiction(&document(&object), r#"{"ids": {}}"#, false, ',').as_deref(), Some("holds {} at .ids, typed an array"));
        // The JSON null at the top is the column's null to the run time: wrong only where the column is non-null.
        assert_eq!(contradiction(&document(&object), "null", false, ',').as_deref(), Some("holds the JSON null, typed non-null"));
        assert_eq!(contradiction(&document(&object), "null", true, ','), None);
        // Every key is there, and a leaf is the form Postgres writes its type in.
        assert_eq!(contradiction(&document(&object), "{}", false, ',').as_deref(), Some("has no key .ids"));
        assert_eq!(contradiction(&document(&list), r#"["1"]"#, false, ',').as_deref(), Some(r#"holds "1" at [0], typed as int8"#));
        let bits: Type = serde_json::from_value(json!({"kind": "array", "name": "bit", "element": {"type": {"kind": "boolean"}}})).unwrap();
        assert_eq!(json_contradiction(&bits, &json!("101"), ".b"), None);
        assert!(json_contradiction(&bits, &json!([true]), ".b").is_some());
    }

    #[test]
    fn the_plain_state_holds_no_extreme_value() {
        for (category, base) in [("N", "numeric"), ("D", "timestamptz"), ("T", "interval")] {
            let edges = pool(&column(category, base), Scenario::Edges);
            let plain = pool(&column(category, base), Scenario::Plain);
            assert!(edges.iter().any(|v| EXTREMES.iter().any(|x| v.starts_with(x))), "{base}: the edge state keeps its extremes");
            assert!(!plain.iter().any(|v| EXTREMES.iter().any(|x| v.starts_with(x))), "{base}: {plain:?}");
            assert!(plain.len() > 1, "{base}: the plain state keeps a value");
        }
        assert!(sqlite_pool("INTEGER", Scenario::Edges, false).contains(&Integer(i64::MAX)));
        assert!(!sqlite_pool("INTEGER", Scenario::Plain, false).contains(&Integer(i64::MAX)));
        assert!(sqlite_pool("REAL", Scenario::Edges, false).contains(&Real(1e308)));
        assert!(!sqlite_pool("REAL", Scenario::Plain, false).contains(&Real(1e308)));
    }

    #[test]
    fn an_autoincrement_key_never_takes_the_largest_integer() {
        for scenario in SCENARIOS {
            let pool = sqlite_pool("INTEGER", scenario, true);
            assert!(pool.iter().all(|v| !matches!(v, Integer(n) if *n > AUTOINCREMENT_TOP)), "{scenario:?}: {pool:?}");
        }
        assert!(sqlite_pool("INTEGER", Scenario::Edges, true).contains(&Integer(AUTOINCREMENT_TOP)));
    }

    #[test]
    fn the_network_types_have_pools_of_their_own() {
        let inet = pool(&column("I", "inet"), Scenario::Edges);
        for value in ["'10.0.0.1'::inet", "'::1'::inet", "'0.0.0.0/0'::inet", "'::/0'::inet"] {
            assert!(inet.iter().any(|v| v == value), "{value} in {inet:?}");
        }
        assert!(pool(&column("U", "macaddr"), Scenario::Edges).contains(&"'08:00:2b:01:02:03'::macaddr".to_string()));
        assert!(pool(&column("U", "macaddr8"), Scenario::Edges).contains(&"'08:00:2b:01:02:03:04:05'::macaddr8".to_string()));
        // A type with no pool gets the two generic literals, which the type itself then accepts or refuses.
        assert_eq!(pool(&column("U", "tsm_handler"), Scenario::Edges), ["NULL", "'1'::tsm_handler", "'a'::tsm_handler"]);
        assert_eq!(fresh_value(&column("I", "inet"), 900_000).as_deref(), Some("'172.29.187.160'::inet"));
    }

    #[test]
    fn the_extension_types_have_pools_of_their_own() {
        // Vectors of several dimensions share a pool: a typmod keeps the ones of its own.
        let mut vector = column("U", "vector");
        vector.ty = "vector(3)".into();
        assert_eq!(pool(&vector, Scenario::Edges), ["NULL", "'[1]'::vector(3)", "'[1,2]'::vector(3)", "'[1,2,3]'::vector(3)", "'[0,0,0]'::vector(3)"]);
        assert!(pool(&column("U", "halfvec"), Scenario::Edges).contains(&"'[1,2,3]'::halfvec".to_string()));
        assert!(pool(&column("U", "sparsevec"), Scenario::Edges).contains(&"'{1:1}/3'::sparsevec".to_string()));
        assert!(pool(&column("U", "geometry"), Scenario::Edges).contains(&"'SRID=4326;POINT EMPTY'::geometry".to_string()));
        assert!(pool(&column("U", "geography"), Scenario::Edges).contains(&"'POINT(1 2)'::geography".to_string()));
        assert!(pool(&column("U", "box2d"), Scenario::Edges).contains(&"'BOX(0 0,1 1)'::box2d".to_string()));
        assert!(pool(&column("U", "box3d"), Scenario::Edges).contains(&"'BOX3D(0 0 0,1 1 1)'::box3d".to_string()));
        assert!(pool(&column("U", "seg"), Scenario::Edges).contains(&"'1 .. 2'::seg".to_string()));
        // earthdistance's earth is a domain over cube, so its pool is cube's, the earth point included.
        let mut earth = column("U", "cube");
        earth.ty = "earth".into();
        assert!(pool(&earth, Scenario::Edges).contains(&"'(6373311.758273381, 222560.95080971014, 111314.3802612757)'::earth".to_string()));
        for isn in ["isbn", "isbn13", "ean13", "issn", "ismn", "upc"] {
            assert!(pool(&column("U", isn), Scenario::Edges).contains(&format!("'0-306-40615-2'::{isn}")), "{isn}");
        }
        // A multirange, built-in or an extension's, has a pool of its own as a range has.
        let mut multirange = column("R", "int4multirange");
        multirange.kind = "m".into();
        assert_eq!(pool(&multirange, Scenario::Edges), ["NULL", "'{}'::int4multirange", "'{[1,3)}'::int4multirange", "'{(,5), [7,)}'::int4multirange", "'{[2026-01-01,2026-02-01)}'::int4multirange"]);
    }

    #[test]
    fn an_array_quotes_its_element_and_separates_it_with_the_types_delimiter() {
        let mut vectors = column("U", "vector");
        vectors.ty = "vector(2)[]".into();
        vectors.is_array = true;
        assert_eq!(with_casts(&vectors, arrays(&vectors, Some("'[1]'"))), ["NULL", "'{}'::vector(2)[]", "'{\"[1]\"}'::vector(2)[]", "'{\"[1]\",NULL}'::vector(2)[]", "'{NULL}'::vector(2)[]"]);
        assert_eq!(arrays(&vectors, Some("'[1,2]'")), ["'{}'", "'{\"[1,2]\"}'", "'{\"[1,2]\",NULL}'", "'{NULL}'"]);
        let mut geometries = column("U", "geometry");
        geometries.delimiter = ":".into();
        assert_eq!(arrays(&geometries, Some("'POINT(1 2)'"))[2], "'{\"POINT(1 2)\":NULL}'");
        let mut boxes = column("G", "box");
        boxes.delimiter = ";".into();
        assert_eq!(arrays(&boxes, Some("'(1,1),(0,0)'"))[2], "'{\"(1,1),(0,0)\";NULL}'");
        // A quote or a backslash inside the element is escaped for the array's own syntax.
        assert_eq!(arrays(&column("S", "text"), Some(r#"'a"b\c'"#))[1], r#"'{"a\"b\\c"}'"#);
        // With no element the type accepts there is the empty array and the NULL alone.
        assert_eq!(arrays(&vectors, None), ["'{}'", "'{NULL}'"]);
    }

    #[test]
    fn a_null_element_is_found_between_the_types_own_delimiters() {
        let points = json!({"kind": "array", "name": "public._geometry", "element": {"type": {"kind": "geometry", "name": "public.geometry"}}});
        let point = "0101000000000000000000F03F0000000000000040";
        // An array of geometries separates its elements with a colon, of boxes with a semicolon.
        assert_eq!(contradiction(&points, &format!("{{{point}:NULL}}"), false, ':').as_deref(), Some("typed with non-null elements holds a NULL element"));
        assert_eq!(contradiction(&points, &format!("{{{point}:{point}}}"), false, ':'), None);
        assert!(array_has_null("{(1,1),(0,0);NULL}", ';'));
        assert!(!array_has_null("{(1,1),(0,0);(2,2),(1,1)}", ';'));
        // A comma is no separator there, nor a colon in an array of text.
        assert!(!array_has_null("{NULL:x}", ','));
        assert!(array_has_null("{a,NULL}", ','));
        assert!(!array_has_null(r#"{"NULL",b}"#, ','));
    }

    #[test]
    fn a_decimal_counts_the_digits_its_precision_holds() {
        let decimal = |precision: u32, scale: i32| DecimalType { precision: Some(precision), min_scale: Some(scale), max_scale: Some(scale), ..Default::default() };
        // A scale above the precision leaves zeros after the point that are no significant digits.
        assert_eq!(decimal_contradiction(&decimal(3, 6), "0.000123"), None);
        assert!(decimal_contradiction(&decimal(3, 6), "0.001230").is_some());
        assert_eq!(decimal_contradiction(&decimal(10, 2), "-99999999.99"), None);
        assert!(decimal_contradiction(&decimal(10, 2), "999999999.99").is_some());
        assert_eq!(decimal_contradiction(&decimal(5, 2), "0.50"), None);
        // A negative scale's zeros before the point are not counted either.
        assert_eq!(decimal_contradiction(&decimal(5, -2), "9999900"), None);
        assert!(decimal_contradiction(&decimal(5, -2), "99999900").is_some());
    }

    #[test]
    fn the_content_option_of_an_fts_table() {
        assert_eq!(fts_content("CREATE VIRTUAL TABLE r USING fts5(title, content='recipes', content_rowid='id')").as_deref(), Some("recipes"));
        assert_eq!(fts_content("create virtual table r using fts5 (title, body, content = \"my recipes\")").as_deref(), Some("my recipes"));
        assert_eq!(fts_content("create virtual table r\nusing\tfts4(content=recipes, title)").as_deref(), Some("recipes"));
        assert_eq!(fts_content("create virtual table r using fts5(word, content='')").as_deref(), Some(""));
        assert_eq!(fts_content("create virtual table r using fts5(word, content=)").as_deref(), Some(""));
        assert_eq!(fts_content("create virtual table r using fts5(title, 'content=x, y', tokenize = 'porter')"), None);
        assert_eq!(fts_content("create virtual table r using fts5(title, content)"), None);
        assert_eq!(fts_content("create virtual table r using rtree(id, content='x')"), None);
        assert_eq!(fts_content("create table using_fts5 (content text default 'content=x')"), None);
    }

    #[test]
    fn a_statement_with_no_run_that_succeeded_is_unchecked() {
        let statements: Vec<(String, String, Value)> = ["ran.sql", "failed.sql", "timed_out.sql", "refused.sql", "mixed.sql"].iter().map(|f| (f.to_string(), String::new(), json!({}))).collect();
        let mut failed = Tally::default();
        failed.fail("integer\n  overflow".into());
        failed.fail("another error".into());
        failed.timed_out = 1;
        let tallies = vec![
            Tally { ran: 3, ..Tally::default() },
            failed,
            Tally { timed_out: 4, ..Tally::default() },
            Tally { refused: Some("no seed value for parameter v of type vector".into()), ..Tally::default() },
            Tally { ran: 1, timed_out: 2, ..Tally::default() },
        ];
        let report = report(&statements, &tallies, 250, vec!["b".into(), "a".into(), "b".into()], Vec::new());
        assert_eq!(
            report.unchecked,
            [
                ("failed.sql".to_string(), "integer overflow".to_string()),
                ("timed_out.sql".to_string(), "every run timed out (250 ms)".to_string()),
                ("refused.sql".to_string(), "no seed value for parameter v of type vector".to_string()),
            ]
        );
        assert_eq!(report.problems, ["a", "b"]);
        // A run the timeout stopped is named beside the runs that went on, and is never a run.
        assert_eq!(report.notes, ["failed.sql: 1 of 3 run(s) timed out (250 ms) and count for nothing; 0 ran", "mixed.sql: 2 of 3 run(s) timed out (250 ms) and count for nothing; 1 ran"]);
    }

    #[test]
    fn an_assignment_takes_the_next_value_of_every_pool() {
        assert_eq!(assignments(&[]), [Vec::<String>::new()]);
        let pools = [vec!["1".to_string(), "2".to_string(), "3".to_string()], vec!["'a'".to_string()]];
        assert_eq!(assignments(&pools), [["1", "'a'"], ["2", "'a'"], ["3", "'a'"]]);
    }
}
