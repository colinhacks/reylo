//! The MySQL half: the host the engine's replay asks, and the seeding and the runs.
//!
//! The host answers as the loader's does (`packages/mysql/describe.ts`, through mysql2). A describe is
//! a server-side prepare, and the answer is the prepare's own column and parameter definitions:
//! the protocol type code, the flags and the character set of each, and a column's display length
//! and the table it is a column of, untouched. A catalog query is
//! sent as text with its values written into it the way mysql2's `query()` writes them, and each
//! value comes back as mysql2 decodes it: a number for an integer or a float, a string for
//! anything else. The session is mysql2's too: `utf8mb4` with `utf8mb4_unicode_ci` as the
//! connection's collation, and the server's own `sql_mode` with IGNORE_SPACE, which mysql2's
//! client flags add, or the one in the case's `sql_mode` file, which a project would set on its
//! sessions. A prepare the server refuses is the engine's to report, by the server's message and the
//! error's name as mysql2 gives it ([`error_name`]); a catalog query that fails fails the case.
//!
//! MySQL has no transactional DDL and no deferred constraints, so the isolation is not Postgres's
//! one transaction. Each state is seeded in a transaction that is never committed, and every run
//! sits between `SAVEPOINT q` and `ROLLBACK TO SAVEPOINT q`. When that rollback finds no
//! savepoint, the run ended the transaction (an implicit commit, or an error that rolled all of
//! it back): the database is then dropped, built again from `setup.sql` on a new session and
//! seeded again, and the same is done when a table no rollback restores (MyISAM, MEMORY) has
//! another checksum than it had when seeded, and between the two states of a case that has such
//! a table. A user variable a run set is set back to NULL. An AUTO_INCREMENT counter is not rolled
//! back either: each run of a statement that writes moves one a little, which changes no answer,
//! but a key a run wrote may leave it at its type's largest value, where every later INSERT without
//! a key fails, so a counter that moved by a million or more has the database built again.
//!
//! The edge state is seeded under a `sql_mode` without `NO_ZERO_DATE` and `NO_ZERO_IN_DATE`,
//! still strict, so the zero dates an older row may hold are there; the statements then run under
//! the session's own mode. A column's pool is its type's: an integer's bounds, signed or unsigned,
//! the two 64-bit integers one double stands for, a DECIMAL's largest value and two that differ
//! past a double's precision, strings that differ by case, by an accent and by a trailing space,
//! every ENUM label, a SET of none and of two, the zero dates, JSON `null`, a BIT of ones. An
//! AUTO_INCREMENT column never takes its type's largest value: the next INSERT without a key
//! would find no value left.
//!
//! A statement is run as each kind of driver the loader takes runs it. First prepared and executed
//! over the binary protocol, as mysql2's `execute()` runs it, once per assignment of its parameters. A parameter's values are the JavaScript values its
//! TypeScript type admits, each sent as mysql2 sends it: a number as a DOUBLE, or as an integer
//! where the prepare typed the placeholder as one and the number is a safe integer that fits
//! (both forms are tried, an older mysql2 knowing only the first), a string as a VAR_STRING, a
//! `Date` as a DATETIME. Two things are not mysql2's: a `Uint8Array` goes as a VAR_STRING where
//! mysql2 sends a BLOB, and a JSON value as its text where mysql2 sends the JSON type, because the
//! client here has no way to name a parameter's wire type.
//!
//! Then as text, for a statement with parameters: mariadb's `query()` and PlanetScale's `execute()`
//! bind nothing and write each value into the statement as a literal, the way `sqlstring` does (a
//! number as JavaScript prints it, a string and a `Date` quoted, bytes as `X'…'`). The server then
//! types the statement by its literals and not by the prepare's placeholders: `:n + 1` is a
//! DECIMAL for 1.5 where a bound 1.5 makes it a DOUBLE, and a DECIMAL key compared with `'1.5'`
//! is compared as two doubles.
//!
//! Beyond what the other dialects' runs check (no NULL in a column typed non-null, the row count
//! within the proof, a JSON document's nulls), a MySQL run checks the type itself. The result set
//! of an execution carries its own column definitions, which are what a driver decodes by, and
//! they may differ from the prepare's: the engine's type of each column must be the type of what
//! came back. And every value inside a JSON document must be of its field's type: a number, a
//! string, a boolean, an object or a list. Where the engine types a column or a field as either
//! of two types (`int8|float8`), the value must be of one of them.
//!
//! The server types some values by the plan: an integer it computes is an INT or a BIGINT in a
//! temporary table by its display length, whatever the prepare said, and a truth value copied
//! into one is written into a JSON document as 1 where the comparison itself is written as
//! `true`. The seeded tables bring out one plan, so every statement that starts with SELECT runs
//! a second time with SQL_BUFFER_RESULT, which puts the whole result in a temporary table and
//! changes nothing else; a problem found there says so.

use crate::seed::{report, Report, Scenario, Tally, EMPTY, SCENARIOS};
use mysql_async::prelude::Queryable;
use mysql_async::{Conn, Opts, OptsBuilder, Statement, Value as My};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

/// The session of mysql2's default connection: character set 224.
const SESSION: &str = "SET NAMES utf8mb4 COLLATE utf8mb4_unicode_ci";

/// mysql2's IGNORE_SPACE client flag, which the server turns into IGNORE_SPACE in the session's
/// `sql_mode` (mariadb's connector sends it too). The client here sends no such flag.
const IGNORE_SPACE: &str = "SET SESSION sql_mode = concat_ws(',', nullif(@@session.sql_mode, ''), 'IGNORE_SPACE')";

/// Every AUTO_INCREMENT counter of the database, as the storage engine has it now.
const COUNTERS_QUERY: &str = "select /*+ SET_VAR(information_schema_stats_expiry = 0) */ table_name, auto_increment from information_schema.tables where table_schema = database() and auto_increment is not null";

/// How far a counter moves before it is taken for a key a run wrote.
const COUNTER_JUMP: u64 = 1_000_000;

/// The strict mode the edge state is seeded under: a value a column refuses is an error, and a
/// zero date is no such value.
const EDGE_MODE: &str = "STRICT_ALL_TABLES,ERROR_FOR_DIVISION_BY_ZERO,NO_ENGINE_SUBSTITUTION";

/// The throwaway server the MySQL cases run on.
pub struct Server {
    admin: Conn,
    opts: Opts,
    /// `80411` for 8.4.11.
    pub version: u32,
}

pub async fn connect(url: &str) -> Result<Server, String> {
    let opts = Opts::from_url(url).map_err(|e| e.to_string())?;
    if opts.tcp_port() == 3306 {
        return Err("port 3306 is the shared server; the oracle runs only on a throwaway one".into());
    }
    let mut admin = session(&opts, None, None).await?;
    let version: String = admin.query_first("select version()").await.map_err(|e| e.to_string())?.unwrap_or_default();
    let mut parts = version.split(|c: char| !c.is_ascii_digit()).filter_map(|p| p.parse::<u32>().ok());
    let version = parts.next().unwrap_or(0) * 10000 + parts.next().unwrap_or(0) * 100 + parts.next().unwrap_or(0);
    Ok(Server { admin, opts, version })
}

/// A session as mysql2 opens it, in `db`, under the case's `sql_mode` if it names one. Nothing is
/// kept by the client: every prepare is the server's.
async fn session(opts: &Opts, db: Option<&str>, mode: Option<&str>) -> Result<Conn, String> {
    // One statement sets the mode: the client may run its statements in any order.
    let init = vec![format!("{SESSION}; {}", mode.map_or(IGNORE_SPACE.to_string(), |mode| format!("SET SESSION sql_mode = {}", quote(mode))))];
    let builder = OptsBuilder::from_opts(opts.clone()).db_name(db).prefer_socket(false).stmt_cache_size(0).init(init);
    Conn::new(builder).await.map_err(|e| format!("cannot open a session: {e}"))
}

/// Drop the case's database and build it again from `setup.sql`, on a session of its own, so
/// nothing of the setup's session (a variable, a temporary table) reaches the statements.
async fn build(server: &mut Server, db: &str, setup: &str) -> Result<(), String> {
    server.admin.query_drop(format!("DROP DATABASE IF EXISTS `{db}`")).await.map_err(|e| format!("drop {db}: {}", message(&e)))?;
    server.admin.query_drop(format!("CREATE DATABASE `{db}`")).await.map_err(|e| format!("create {db}: {}", message(&e)))?;
    let mut conn = session(&server.opts, Some(db), None).await?;
    let outcome = match conn.query_iter(setup).await {
        Ok(result) => result.drop_result().await,
        Err(e) => Err(e),
    };
    let _ = conn.disconnect().await;
    outcome.map_err(|e| format!("setup.sql: {}", message(&e)))
}

pub async fn run(case: &Path, server: &mut Server, keep: bool, timeout_ms: u64, given: &Map<String, Value>, dumped: &mut Map<String, Value>, statements: Vec<(String, String)>) -> Result<(Vec<String>, Vec<String>), String> {
    let name: String = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let db = format!("reylo_oracle_{}", name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect::<String>());
    let setup = fs::read_to_string(case.join("setup.sql")).map_err(|e| e.to_string())?;
    let mode = fs::read_to_string(case.join("sql_mode")).ok().map(|m| m.trim().to_string());
    build(server, &db, &setup).await?;
    let mut conn = session(&server.opts, Some(&db), mode.as_deref()).await?;
    let mut lines = Vec::new();
    let mut analyzed = Vec::new();
    for (file, sql) in statements {
        let analysis = match given.get(&format!("{name}/{file}")) {
            Some(answer) => answer.clone(),
            None => analyze(&mut conn, &sql).await.map_err(|e| format!("{file}: {e}"))?,
        };
        lines.push(format!("{file}: {}", crate::line(&analysis)));
        dumped.insert(format!("{name}/{file}"), analysis.clone());
        analyzed.push((file, sql, analysis));
    }
    // A value that contradicts a type is a failure whatever expected.txt says.
    let mut state = State { db: &db, setup: &setup, mode: mode.as_deref(), conn, tables: Vec::new(), counters: HashMap::new() };
    let report = check(server, &mut state, &analyzed, timeout_ms).await;
    let _ = state.conn.disconnect().await;
    if !keep {
        let _ = server.admin.query_drop(format!("DROP DATABASE IF EXISTS `{db}`")).await;
    }
    let unchecked = crate::seeded(case, lines.len(), report.map_err(|e| format!("seeding: {e}"))?)?;
    Ok((lines, unchecked))
}

// ---------------------------------------------------------------------------------------------
// The host
// ---------------------------------------------------------------------------------------------

/// The engine's replay with every need answered on `conn`, in as many rounds as the loader gives it.
async fn analyze(conn: &mut Conn, sql: &str) -> Result<Value, String> {
    let mut answers = Map::new();
    for _ in 0..16 {
        let request = json!({ "op": "analyze", "sql": sql, "dialect": "mysql", "answers": Value::Object(answers.clone()) });
        let response: Value = serde_json::from_str(&crate::engine(&request.to_string())).map_err(|e| e.to_string())?;
        match response["status"].as_str() {
            Some("analysis") => return Ok(response["analysis"].clone()),
            Some("need") => {
                for need in response["needs"].as_array().cloned().unwrap_or_default() {
                    let id = need["id"].as_str().unwrap_or_default().to_string();
                    let answer = fulfil(conn, &need).await?;
                    answers.insert(id, answer);
                }
            }
            _ => return Err(format!("the engine answered {response}")),
        }
    }
    Err("the replay did not converge".into())
}

async fn fulfil(conn: &mut Conn, need: &Value) -> Result<Value, String> {
    let sql = need["sql"].as_str().unwrap_or_default();
    match need["kind"].as_str() {
        Some("describe") => match conn.prep(sql).await {
            Ok(statement) => {
                let columns: Vec<Value> = statement.columns().iter().map(|c| json!({ "name": c.name_str(), "type": c.column_type() as u8, "flags": c.flags().bits(), "charset": c.character_set(), "length": c.column_length(), "org_table": c.org_table_str(), "org_name": c.org_name_str() })).collect();
                let params: Vec<Value> = statement.params().iter().map(|p| json!({ "type": p.column_type() as u8, "flags": p.flags().bits(), "charset": p.character_set() })).collect();
                conn.close(statement).await.map_err(|e| e.to_string())?;
                Ok(json!({ "mysql": { "columns": columns, "params": params } }))
            }
            Err(mysql_async::Error::Server(e)) => Ok(match error_name(e.code) {
                Some(name) => json!({ "error": { "message": e.message, "code": name } }),
                None => json!({ "error": { "message": e.message } }),
            }),
            Err(e) => Err(format!("the prepare did not reach the server: {e}")),
        },
        Some("query") => {
            let params = need["params"].as_array().cloned().unwrap_or_default();
            let text = interpolate(sql, &params).ok_or_else(|| format!("cannot write {params:?} into the catalog query\n    {sql}"))?;
            let mut result = conn.query_iter(text).await.map_err(|e| format!("catalog query failed: {}\n    {sql}", message(&e)))?;
            let types: Vec<u8> = result.columns_ref().iter().map(|c| c.column_type() as u8).collect();
            let mut rows = Vec::new();
            while let Some(row) = result.next().await.map_err(|e| format!("catalog query failed: {}\n    {sql}", message(&e)))? {
                rows.push(Value::Array(row.unwrap().into_iter().zip(&types).map(|(value, code)| decoded(value, *code)).collect()));
            }
            Ok(json!({ "rows": rows }))
        }
        other => Err(format!("unknown need {other:?}")),
    }
}

/// A catalog query with its values written into the text, as mysql2's `query()` writes them
/// (`sqlstring`): each `?` in turn, a string quoted with its special characters escaped.
fn interpolate(sql: &str, params: &[Value]) -> Option<String> {
    let mut out = String::with_capacity(sql.len());
    let mut params = params.iter();
    for c in sql.chars() {
        if c != '?' {
            out.push(c);
            continue;
        }
        match params.next()? {
            Value::Null => out.push_str("NULL"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::String(s) => out.push_str(&quote(s)),
            _ => return None,
        }
    }
    params.next().is_none().then_some(out)
}

/// A string as a MySQL literal, by `sqlstring`'s escapes.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\0' => out.push_str("\\0"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{1a}' => out.push_str("\\Z"),
            '"' => out.push_str("\\\""),
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

fn backticked(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

/// One value of a text result as mysql2 hands it to the host: a number for the integer and
/// floating types, a string for the rest.
fn decoded(value: My, code: u8) -> Value {
    match value {
        My::NULL => Value::Null,
        My::Bytes(bytes) => {
            let text = String::from_utf8_lossy(&bytes).to_string();
            match code {
                1 | 2 | 3 | 8 | 9 | 13 => text.parse::<i64>().map(Value::from).or_else(|_| text.parse::<u64>().map(Value::from)).unwrap_or(Value::String(text)),
                4 | 5 => text.parse::<f64>().map(Value::from).unwrap_or(Value::String(text)),
                _ => Value::String(text),
            }
        }
        My::Int(n) => json!(n),
        My::UInt(n) => json!(n),
        My::Float(f) => json!(f),
        My::Double(f) => json!(f),
        other => Value::String(other.as_sql(true)),
    }
}

fn message(e: &mysql_async::Error) -> String {
    match e {
        mysql_async::Error::Server(e) => format!("{} ({})", e.message, e.code),
        other => other.to_string(),
    }
}

/// The name mysql2 gives a server error (`err.code`), which the loader passes to the engine and
/// the engine prints: the errors a prepare of a statement answers with. An error not listed here
/// goes without a name, where the loader would have one.
fn error_name(code: u16) -> Option<&'static str> {
    Some(match code {
        1046 => "ER_NO_DB_ERROR",
        1048 => "ER_BAD_NULL_ERROR",
        1051 => "ER_BAD_TABLE_ERROR",
        1052 => "ER_NON_UNIQ_ERROR",
        1054 => "ER_BAD_FIELD_ERROR",
        1055 => "ER_WRONG_FIELD_WITH_GROUP",
        1056 => "ER_WRONG_GROUP_FIELD",
        1060 => "ER_DUP_FIELDNAME",
        1064 => "ER_PARSE_ERROR",
        1066 => "ER_NONUNIQ_TABLE",
        1093 => "ER_UPDATE_TABLE_USED",
        1109 => "ER_UNKNOWN_TABLE",
        1111 => "ER_INVALID_GROUP_FUNC_USE",
        1136 => "ER_WRONG_VALUE_COUNT_ON_ROW",
        1140 => "ER_MIX_OF_GROUP_FUNC_AND_FIELDS",
        1146 => "ER_NO_SUCH_TABLE",
        1210 => "ER_WRONG_ARGUMENTS",
        1222 => "ER_WRONG_NUMBER_OF_COLUMNS_IN_SELECT",
        1235 => "ER_NOT_SUPPORTED_YET",
        1241 => "ER_OPERAND_COLUMNS",
        1242 => "ER_SUBQUERY_NO_1_ROW",
        1247 => "ER_ILLEGAL_REFERENCE",
        1248 => "ER_DERIVED_MUST_HAVE_ALIAS",
        1250 => "ER_TABLENAME_NOT_ALLOWED_HERE",
        1267 => "ER_CANT_AGGREGATE_2COLLATIONS",
        1295 => "ER_UNSUPPORTED_PS",
        1305 => "ER_SP_DOES_NOT_EXIST",
        1318 => "ER_SP_WRONG_NO_OF_ARGS",
        1327 => "ER_SP_UNDECLARED_VAR",
        1353 => "ER_VIEW_WRONG_LIST",
        1356 => "ER_VIEW_INVALID",
        1364 => "ER_NO_DEFAULT_FOR_FIELD",
        1390 => "ER_PS_MANY_PARAM",
        1582 => "ER_WRONG_PARAMCOUNT_TO_NATIVE_FCT",
        1583 => "ER_WRONG_PARAMETERS_TO_NATIVE_FCT",
        1630 => "ER_FUNC_INEXISTENT_NAME_COLLISION",
        3140 => "ER_INVALID_JSON_TEXT",
        3143 => "ER_INVALID_JSON_PATH",
        3554 => "ER_NO_SYSTEM_TABLE_ACCESS",
        3579 => "ER_WINDOW_NO_SUCH_WINDOW",
        3593 => "ER_WINDOW_INVALID_WINDOW_FUNC_USE",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// The seeding
// ---------------------------------------------------------------------------------------------

/// A base table of the case's database, as the seeding fills it.
struct Table {
    name: String,
    /// Whether a rollback restores its rows (InnoDB).
    transactional: bool,
    columns: Vec<Column>,
    /// The unique keys by index name; a key with a part over an expression has no columns.
    keys: HashMap<String, Option<Vec<String>>>,
}

struct Column {
    name: String,
    /// `information_schema.columns.data_type`: `int`, `varchar`, `enum`.
    data_type: String,
    unsigned: bool,
    /// The labels of an ENUM or a SET.
    labels: Vec<String>,
    nullable: bool,
    generated: bool,
    auto_increment: bool,
    precision: u32,
    scale: u32,
    /// The longest string the column takes, in characters.
    length: u64,
    /// The fractional digits of a time's seconds.
    fraction: u32,
}

const COLUMNS_QUERY: &str = "select c.table_name, coalesce(t.engine, ''), c.column_name, c.data_type, c.column_type, c.extra, c.is_nullable = 'YES', \
    coalesce(c.numeric_precision, 0), coalesce(c.numeric_scale, 0), coalesce(c.character_maximum_length, 0), coalesce(c.datetime_precision, 0) \
    from information_schema.columns c join information_schema.tables t on t.table_schema = c.table_schema and t.table_name = c.table_name \
    where c.table_schema = database() and t.table_type = 'BASE TABLE' order by t.create_time, c.table_name, c.ordinal_position";

const KEYS_QUERY: &str = "select table_name, index_name, column_name from information_schema.statistics \
    where table_schema = database() and non_unique = 0 order by table_name, index_name, seq_in_index";

async fn tables(conn: &mut Conn) -> Result<Vec<Table>, String> {
    let rows: Vec<(String, String, String, String, String, String, bool, u32, u32, u64, u32)> = conn.query(COLUMNS_QUERY).await.map_err(|e| e.to_string())?;
    let mut out: Vec<Table> = Vec::new();
    for (table, engine, name, data_type, column_type, extra, nullable, precision, scale, length, fraction) in rows {
        let extra = extra.to_ascii_lowercase();
        let column = Column {
            name,
            data_type: data_type.to_ascii_lowercase(),
            unsigned: column_type.to_ascii_lowercase().contains("unsigned"),
            labels: labels(&column_type),
            nullable,
            generated: extra.contains("generated") && !extra.contains("default_generated"),
            auto_increment: extra.contains("auto_increment"),
            precision,
            scale,
            length,
            fraction,
        };
        match out.last_mut() {
            Some(t) if t.name == table => t.columns.push(column),
            _ => out.push(Table { name: table, transactional: engine.eq_ignore_ascii_case("InnoDB"), columns: vec![column], keys: HashMap::new() }),
        }
    }
    let keys: Vec<(String, String, Option<String>)> = conn.query(KEYS_QUERY).await.map_err(|e| e.to_string())?;
    for (table, index, column) in keys {
        let Some(table) = out.iter_mut().find(|t| t.name == table) else { continue };
        match (table.keys.entry(index).or_insert_with(|| Some(Vec::new())), column) {
            (Some(columns), Some(column)) => columns.push(column),
            (key, _) => *key = None,
        }
    }
    Ok(out)
}

/// The labels of `enum('a','b')` or `set('a','b')`, a quote inside one written twice.
fn labels(column_type: &str) -> Vec<String> {
    let Some(open) = column_type.find('(').filter(|_| ["enum", "set"].iter().any(|k| column_type.to_ascii_lowercase().starts_with(k))) else { return Vec::new() };
    let mut out = Vec::new();
    let mut chars = column_type[open + 1..].chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\'' {
            continue;
        }
        let mut label = String::new();
        while let Some(c) = chars.next() {
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    label.push('\'');
                    continue;
                }
                break;
            }
            label.push(c);
        }
        out.push(label);
    }
    out
}

/// The least and the greatest value of an integer type.
fn bounds(data_type: &str, unsigned: bool) -> Option<(i128, i128)> {
    let bits = match data_type {
        "tinyint" => 8,
        "smallint" => 16,
        "mediumint" => 24,
        "int" | "integer" => 32,
        "bigint" => 64,
        _ => return None,
    };
    Some(if unsigned { (0, (1i128 << bits) - 1) } else { (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1) })
}

/// The two integers above 2^53 that one double stands for.
const TWINS: [&str; 2] = ["9007199254740992", "9007199254740993"];

/// Two times a microsecond apart, which are one double as numbers.
const MICROSECONDS: [&str; 2] = ["'2026-01-01 12:00:00.000001'", "'2026-01-01 12:00:00.000002'"];

/// Instants an hour apart, in UTC, that a session in a zone with daylight saving time shows as
/// one wall-clock time: the hour the clocks repeat on 2026-11-01 in each zone of the United
/// States, and on 2026-10-25 in Europe. The seeding writes a TIMESTAMP under UTC.
const REPEATED_HOURS: [&str; 7] = [
    "'2026-11-01 05:30:00'",
    "'2026-11-01 06:30:00'",
    "'2026-11-01 07:30:00'",
    "'2026-11-01 08:30:00'",
    "'2026-11-01 09:30:00'",
    "'2026-10-25 00:30:00'",
    "'2026-10-25 01:30:00'",
];

/// The edge values of one column's type in a state, as SQL literals, NULL first. The values of
/// extreme magnitude, the zero dates and the twins are in the edge state alone.
fn pool(col: &Column, scenario: Scenario) -> Vec<String> {
    let edges = scenario == Scenario::Edges;
    let mut out: Vec<String> = vec!["NULL".into()];
    let mut push = |values: &[&str]| out.extend(values.iter().map(|v| v.to_string()));
    match col.data_type.as_str() {
        ty if bounds(ty, col.unsigned).is_some() => {
            let (min, max) = bounds(ty, col.unsigned).expect("an integer type");
            push(&["1", "2", "0"]);
            if min < 0 {
                push(&["-1"]);
            }
            if edges {
                if min < 0 {
                    push(&[&min.to_string()]);
                }
                // The next INSERT without a key takes the value after the largest one.
                push(&[&(if col.auto_increment { max / 2 } else { max }).to_string()]);
                if ty == "bigint" {
                    push(&TWINS);
                }
            }
        }
        "decimal" | "numeric" => {
            push(&["1", "2", "0", "0.1"]);
            if !col.unsigned {
                push(&["-1"]);
            }
            if edges {
                let (whole, fraction) = ((col.precision - col.scale) as usize, col.scale as usize);
                let largest = format!("{}{}{}", "9".repeat(whole), if fraction > 0 { "." } else { "" }, "9".repeat(fraction));
                push(&[&largest]);
                if !col.unsigned {
                    push(&[&format!("-{largest}")]);
                }
                // Beside 1 and beside each other, equal as doubles.
                if fraction >= 17 {
                    push(&[&format!("1.{}1", "0".repeat(fraction - 1))]);
                }
                if whole >= 16 {
                    push(&TWINS);
                }
            }
        }
        "float" | "double" | "real" => {
            push(&["1", "2", "0", "-0e0", "0.1", "1.5"]);
            if !col.unsigned {
                push(&["-1"]);
            }
            if edges {
                push(&[if col.data_type == "float" { "3.402823466e38" } else { "1.7976931348623157e308" }]);
            }
        }
        "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" => push(&["'a'", "'A'", "'b'", "''", "'a '", "'1'", "'01'", "'1x'", "'á'"]),
        "binary" | "varbinary" | "tinyblob" | "blob" | "mediumblob" | "longblob" => push(&["x'61'", "x'41'", "x''", "x'00'", "x'6100'", "x'ff'"]),
        "enum" => out.extend(col.labels.iter().map(|l| quote(l))),
        "set" => {
            out.push("''".into());
            out.extend(col.labels.iter().take(2).map(|l| quote(l)));
            if col.labels.len() > 1 {
                out.push(quote(&format!("{},{}", col.labels[0], col.labels[1])));
            }
        }
        "date" => {
            push(&["'2026-01-01'", "'2026-01-02'"]);
            if edges {
                push(&["'1000-01-01'", "'9999-12-31'", "'0000-00-00'", "'2026-00-00'"]);
            }
        }
        "datetime" | "timestamp" => {
            push(&["'2026-01-01 00:00:00'", "'2026-01-01 12:00:00'", "'2026-01-01 12:00:00.5'"]);
            if edges {
                push(&["'1000-01-01 00:00:00'", "'1970-01-02 00:00:01'", "'2038-01-18 03:14:07'", "'9999-12-31 23:59:59'", "'0000-00-00 00:00:00'"]);
                if col.fraction == 6 {
                    push(&MICROSECONDS);
                }
                if col.data_type == "timestamp" {
                    push(&REPEATED_HOURS);
                }
            }
        }
        "time" => {
            push(&["'12:00:00'", "'00:00:00'", "'01:02:03'"]);
            if edges {
                push(&["'-838:59:59'", "'838:59:59'"]);
                if col.fraction == 6 {
                    push(&["'12:00:00.000001'", "'12:00:00.000002'"]);
                }
            }
        }
        "year" => {
            push(&["2026", "2000"]);
            if edges {
                push(&["1901", "2155", "0"]);
            }
        }
        "json" => push(&["'{\"a\": 1}'", "'null'", "'{}'", "'[]'", "'[1, null]'", "'1'", "'\"a\"'", "'true'"]),
        "bit" => {
            push(&["b'1'", "b'0'"]);
            if edges {
                push(&[&format!("b'{}'", "1".repeat(col.precision.max(1) as usize))]);
                if col.precision == 64 {
                    push(&TWINS);
                }
            }
        }
        "point" | "geometry" => push(&["ST_GeomFromText('POINT(1 1)')", "ST_GeomFromText('POINT(0 0)')"]),
        _ => push(&["'1'", "'a'"]),
    }
    out
}

/// A value of the column's type no pool holds, for a key column to take.
fn fresh_value(col: &Column, n: usize) -> Option<String> {
    let text = format!("k{n}");
    Some(match col.data_type.as_str() {
        ty if bounds(ty, col.unsigned).is_some() => {
            let (_, max) = bounds(ty, col.unsigned).expect("an integer type");
            // Above every pool value but the bounds, and inside the narrowest type.
            if max >= 1_000_000 { (1000 + n).to_string() } else { (3 + n as i128 % (max / 2 - 3)).to_string() }
        }
        "decimal" | "numeric" if col.precision - col.scale >= 7 => (1000 + n).to_string(),
        "float" | "double" | "real" => (1000 + n).to_string(),
        "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" if col.length as usize >= text.len() => quote(&text),
        "binary" | "varbinary" | "tinyblob" | "blob" | "mediumblob" | "longblob" if col.length as usize >= text.len() => format!("x'{}'", text.bytes().map(|b| format!("{b:02x}")).collect::<String>()),
        "date" => format!("date_add('2000-01-01', interval {n} day)"),
        "datetime" | "timestamp" => format!("date_add('2000-01-01 00:00:00', interval {n} minute)"),
        "time" => format!("sec_to_time({n})"),
        "year" => (1902 + n % 250).to_string(),
        _ => return None,
    })
}

/// Seed every table with a state's values, inside the transaction the caller opened: what each
/// table got, and the columns that hold NULL alone. A row the table refuses is skipped, a row
/// that breaks a unique key is tried again with fresh values in the key's columns, and the passes
/// repeat so a row that references another table goes in once the referenced row is there.
async fn seed(conn: &mut Conn, tables: &[Table], scenario: Scenario) -> Result<(String, Vec<String>), String> {
    let mut inserted: Vec<usize> = vec![0; tables.len()];
    let mut fresh = 0;
    for _pass in 0..3 {
        for (t, table) in tables.iter().enumerate() {
            let columns: Vec<&Column> = table.columns.iter().filter(|c| !c.generated).collect();
            if columns.is_empty() {
                continue;
            }
            let pools: Vec<Vec<String>> = columns.iter().map(|c| pool(c, scenario)).collect();
            let base: Vec<String> = pools.iter().map(|p| p.get(1).cloned().unwrap_or_else(|| "NULL".into())).collect();
            let mut rows = vec![base.clone(), vec!["NULL".to_string(); columns.len()]];
            for (i, pool) in pools.iter().enumerate() {
                for value in pool {
                    let mut row = base.clone();
                    row[i] = value.clone();
                    rows.push(row);
                }
            }
            for row in rows {
                if insert(conn, table, &columns, row, &mut fresh).await? {
                    inserted[t] += 1;
                }
            }
        }
    }
    let mut counts = Vec::with_capacity(tables.len());
    let mut bare = Vec::new();
    for (table, inserted) in tables.iter().zip(&inserted) {
        // How many rows each column holds a value in, then how many rows there are.
        let held: Vec<String> = table.columns.iter().map(|c| format!("count({})", backticked(&c.name))).collect();
        let row: mysql_async::Row = conn.query_first(format!("select {}, count(*) from {}", held.join(", "), backticked(&table.name))).await.map_err(|e| e.to_string())?.ok_or("a count returned no row")?;
        let held: Vec<u64> = row.unwrap().into_iter().map(|v| mysql_async::from_value_opt::<u64>(v).unwrap_or(0)).collect();
        let rows = held.last().copied().unwrap_or(0);
        counts.push(format!("{} {inserted}{}", table.name, if rows == 0 { EMPTY } else { "" }));
        if rows > 0 {
            bare.extend(table.columns.iter().zip(&held).filter(|(c, held)| **held == 0 && !c.generated).map(|(c, _)| format!("{}.{} ({})", table.name, c.name, c.data_type)));
        }
    }
    Ok((counts.join(", "), bare))
}

/// Insert one row; a row that breaks a unique key is tried again, once per key the table may
/// have, with fresh values in the columns of the key the server names, or NULL in a column whose
/// type has no fresh value and that takes one.
async fn insert(conn: &mut Conn, table: &Table, columns: &[&Column], mut row: Vec<String>, fresh: &mut usize) -> Result<bool, String> {
    let names: Vec<String> = columns.iter().map(|c| backticked(&c.name)).collect();
    for _ in 0..=table.keys.len() {
        let error = match conn.query_drop(format!("INSERT INTO {} ({}) VALUES ({})", backticked(&table.name), names.join(", "), row.join(", "))).await {
            Ok(()) => return Ok(true),
            Err(mysql_async::Error::Server(e)) => e,
            Err(e) => return Err(e.to_string()),
        };
        if error.code != 1062 {
            return Ok(false);
        }
        // "Duplicate entry '1-a' for key 'users.PRIMARY'"
        let Some(key) = error.message.rsplit_once(" for key '").map(|(_, key)| key.trim_end_matches('\'')) else { return Ok(false) };
        let key = key.strip_prefix(&format!("{}.", table.name)).unwrap_or(key);
        let Some(Some(key)) = table.keys.get(key) else { return Ok(false) };
        for name in key {
            let Some(i) = columns.iter().position(|c| c.name.eq_ignore_ascii_case(name)) else { return Ok(false) };
            *fresh += 1;
            row[i] = match fresh_value(columns[i], *fresh) {
                Some(v) => v,
                None if columns[i].nullable && row[i] != "NULL" => "NULL".into(),
                None => return Ok(false),
            };
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------------------------------------
// The parameters
// ---------------------------------------------------------------------------------------------

/// A JavaScript value a caller may pass for a parameter.
#[derive(Clone, Debug, PartialEq)]
enum Js {
    Number(f64),
    Text(&'static str),
    Bytes(&'static [u8]),
    /// A `Date`: year, month, day, hour.
    Date(u16, u8, u8, u8),
}

/// The values the TypeScript type of a parameter admits (`renderParamType` in `src/render.ts`),
/// by the engine's type for it: a number for an integer or a float, a string for a decimal or a
/// time, a `Date` or a string for a date, a `Uint8Array` for a binary value and anything for
/// JSON. The last of each is a value no seeded row holds, so an INSERT of a new key runs at all.
fn admitted(ty: &Value) -> Vec<Js> {
    use Js::{Bytes, Date, Number, Text};
    match (ty["kind"].as_str(), ty["name"].as_str()) {
        // 2^53 and 2^64 are numbers and no safe integers.
        (Some("scalar"), Some("int2" | "int4" | "int8")) => vec![Number(1.0), Number(2.0), Number(0.0), Number(-1.0), Number(1.5), Number(9007199254740991.0), Number(9007199254740992.0), Number(18446744073709551616.0), Number(900000.0)],
        (Some("scalar"), Some("float4" | "float8")) => vec![Number(1.5), Number(1.0), Number(2.0), Number(0.0), Number(-0.0), Number(0.1), Number(-1.0), Number(1e308), Number(900000.5)],
        (Some("scalar"), Some("numeric")) => vec![Text("1"), Text("2"), Text("0"), Text("1.50"), Text("0.1"), Text("0.10000000000000000001"), Text("9007199254740993"), Text("1e0"), Text("x"), Text("900000")],
        // With the wall-clock times of the hours the clocks repeat ([`REPEATED_HOURS`]).
        (Some("scalar"), Some("timestamp" | "date")) => vec![
            Date(2026, 1, 1, 0),
            Date(2026, 1, 1, 12),
            Text("2026-01-01"),
            Text("2026-01-01 12:00:00"),
            Text("2026-01-01 12:00:00.000001"),
            Text("2026-11-01 01:30:00"),
            Text("2026-10-25 01:30:00"),
            Text("2026-10-25 02:30:00"),
            Text("2026-10-25 03:30:00"),
            Text("0000-00-00"),
            Text("x"),
            Text("2001-02-03"),
        ],
        (Some("scalar"), Some("time")) => vec![Text("12:00:00"), Text("00:00:00"), Text("12:00:00.000001"), Text("-838:59:59"), Text("x"), Text("01:02:04")],
        (Some("scalar"), Some("bytea")) => vec![Bytes(b"a"), Bytes(b"A"), Bytes(b""), Bytes(b"\0"), Bytes(b"a\0"), Bytes(b"\xff"), Bytes(b"k900000")],
        (Some("scalar"), Some("text")) => vec![Text("a"), Text("A"), Text("b"), Text(""), Text("a "), Text("1"), Text("01"), Text("1x"), Text("á"), Text("2026-01-01 12:00:00"), Text("$.a"), Text("k900000")],
        _ => vec![Text("{\"a\": 1}"), Text("[]"), Text("null"), Text("1"), Text("a"), Number(1.0), Number(1.5), Text("k900000")],
    }
}

/// One value as it is bound, as a driver that binds nothing writes it into the statement's text,
/// and as a problem shows it.
#[derive(Clone, Debug, PartialEq)]
struct Bound {
    value: My,
    literal: String,
    shown: String,
}

/// A JavaScript value as mysql2's `execute()` sends it (`encode_parameter.js`). `hint` is the
/// prepare's definition of the placeholder, its type code and whether it is unsigned: a number
/// goes as that integer type when the definition is TINY, SHORT, LONG or LONGLONG and the number
/// is a safe integer inside it, and as a DOUBLE otherwise, which is also how a mysql2 that does
/// not read the definition sends every number (`hinted` false).
fn bound(js: &Js, hint: Option<(u8, bool)>, hinted: bool) -> Bound {
    match js {
        Js::Number(n) => {
            let unsigned = hint.is_some_and(|(_, unsigned)| unsigned);
            let bits: u32 = match hint.filter(|_| hinted) {
                Some((1, _)) => 8,
                Some((2, _)) => 16,
                Some((3, _)) => 32,
                Some((8, _)) => 64,
                _ => 0,
            };
            let fits = bits > 0 && n.fract() == 0.0 && n.abs() <= 9007199254740991.0 && {
                let (min, max) = if unsigned { (0, (1i128 << bits) - 1) } else { (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1) };
                (min..=max).contains(&(*n as i128))
            };
            // JavaScript writes a number by its shortest digits (2^64 as 18446744073709552000), and from 1e21 on with an exponent.
            let literal = if n.abs() < 1e21 { format!("{n}") } else { format!("{n:e}") };
            if fits {
                Bound { value: if unsigned { My::UInt(*n as u64) } else { My::Int(*n as i64) }, literal, shown: format!("{}", *n as i128) }
            } else {
                Bound { value: My::Double(*n), literal, shown: format!("{n:?} as a double") }
            }
        }
        Js::Text(s) => Bound { value: My::Bytes(s.as_bytes().to_vec()), literal: quote(s), shown: format!("'{s}'") },
        Js::Bytes(b) => {
            let hex = b.iter().map(|b| format!("{b:02x}")).collect::<String>();
            Bound { value: My::Bytes(b.to_vec()), literal: format!("X'{hex}'"), shown: format!("x'{hex}'") }
        }
        Js::Date(y, m, d, h) => Bound { value: My::Date(*y, *m, *d, *h, 0, 0, 0), literal: format!("'{y:04}-{m:02}-{d:02} {h:02}:00:00.000'"), shown: format!("Date {y:04}-{m:02}-{d:02} {h:02}:00:00") },
    }
}

/// Every way a parameter may be bound: each admitted value as today's mysql2 sends it, then the
/// integers once more as DOUBLEs.
fn bindings(ty: &Value, hint: Option<(u8, bool)>) -> Vec<Bound> {
    let values = admitted(ty);
    let mut out: Vec<Bound> = values.iter().map(|js| bound(js, hint, true)).collect();
    for js in &values {
        let plain = bound(js, hint, false);
        if !out.contains(&plain) {
            out.push(plain);
        }
    }
    out
}

/// A statement's parameters, several assignments: each takes the next value of every pool, and
/// then, as the seeding lays its rows, each value of one pool stands beside the first value of
/// every other, so a key of two columns meets every value of each beside a row that holds it.
fn assignments(pools: &[Vec<Bound>]) -> Vec<Vec<Bound>> {
    if pools.is_empty() {
        return vec![Vec::new()];
    }
    let width = pools.iter().map(Vec::len).max().unwrap_or(1);
    let mut out: Vec<Vec<Bound>> = (0..width).map(|r| pools.iter().map(|p| p[r % p.len()].clone()).collect()).collect();
    if pools.len() > 1 {
        for (i, pool) in pools.iter().enumerate() {
            for value in pool.iter().skip(1) {
                let mut row: Vec<Bound> = pools.iter().map(|p| p[0].clone()).collect();
                row[i] = value.clone();
                if !out.contains(&row) {
                    out.push(row);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// The runs
// ---------------------------------------------------------------------------------------------

/// The case's database and the session its statements run on.
struct State<'a> {
    db: &'a str,
    setup: &'a str,
    mode: Option<&'a str>,
    conn: Conn,
    tables: Vec<Table>,
    /// Each AUTO_INCREMENT counter as the seeding left it.
    counters: HashMap<String, u64>,
}

impl State<'_> {
    /// The tables a rollback does not restore.
    fn loose(&self) -> Vec<&Table> {
        self.tables.iter().filter(|t| !t.transactional).collect()
    }

    /// Drop the database, build it from `setup.sql` and open a new session on it. The old
    /// session goes first: its open transaction holds the locks the drop waits for.
    async fn rebuild(&mut self, server: &mut Server) -> Result<(), String> {
        let idle = session(&server.opts, None, None).await?;
        let _ = std::mem::replace(&mut self.conn, idle).disconnect().await;
        build(server, self.db, self.setup).await?;
        let conn = session(&server.opts, Some(self.db), self.mode).await?;
        let _ = std::mem::replace(&mut self.conn, conn).disconnect().await;
        Ok(())
    }

    /// Open the transaction and seed it: what each table got, the columns left with NULL alone,
    /// and the checksum of the tables no rollback restores.
    async fn begin_seeded(&mut self, scenario: Scenario) -> Result<(String, Vec<String>, String), String> {
        let conn = &mut self.conn;
        let (mode, zone): (String, String) = conn.query_first("select @@session.sql_mode, @@session.time_zone").await.map_err(|e| e.to_string())?.unwrap_or_default();
        conn.query_drop("START TRANSACTION").await.map_err(|e| e.to_string())?;
        // A TIMESTAMP literal is an instant only under a zone without daylight saving time.
        conn.query_drop("SET SESSION time_zone = '+00:00'").await.map_err(|e| e.to_string())?;
        if scenario == Scenario::Edges {
            conn.query_drop(format!("SET SESSION sql_mode = {}", quote(EDGE_MODE))).await.map_err(|e| e.to_string())?;
        }
        let seeded = seed(conn, &self.tables, scenario).await;
        conn.query_drop(format!("SET SESSION sql_mode = {}, time_zone = {}", quote(&mode), quote(&zone))).await.map_err(|e| e.to_string())?;
        let (counts, bare) = seeded?;
        let checksum = self.checksum().await?;
        self.counters = self.conn.query::<(String, u64), _>(COUNTERS_QUERY).await.map_err(|e| e.to_string())?.into_iter().collect();
        Ok((counts, bare, checksum))
    }

    /// Whether a run left an AUTO_INCREMENT counter far from where the seeding did.
    async fn counter_jumped(&mut self) -> Result<bool, String> {
        let now: Vec<(String, u64)> = self.conn.query(COUNTERS_QUERY).await.map_err(|e| e.to_string())?;
        Ok(now.iter().any(|(table, counter)| self.counters.get(table).is_some_and(|seeded| counter.saturating_sub(*seeded) >= COUNTER_JUMP)))
    }

    async fn checksum(&mut self) -> Result<String, String> {
        let names: Vec<String> = self.loose().iter().map(|t| backticked(&t.name)).collect();
        if names.is_empty() {
            return Ok(String::new());
        }
        let rows: Vec<(String, Option<u64>)> = self.conn.query(format!("CHECKSUM TABLE {}", names.join(", "))).await.map_err(|e| e.to_string())?;
        Ok(rows.iter().map(|(table, sum)| format!("{table}={sum:?}")).collect::<Vec<_>>().join(","))
    }
}

/// One column of a result set, as the execution defined it.
struct Returned {
    name: String,
    code: u8,
    charset: u16,
}

/// Execute a prepared statement and hand each row to `each` as it comes: the result's columns and
/// how many rows it returned.
async fn execute(conn: &mut Conn, statement: &Statement, values: Vec<My>, each: &mut dyn FnMut(&[Returned], &[My])) -> Result<(Vec<Returned>, usize), mysql_async::Error> {
    let params = if values.is_empty() { mysql_async::Params::Empty } else { mysql_async::Params::Positional(values) };
    let mut result = conn.exec_iter(statement, params).await?;
    let columns: Vec<Returned> = result.columns_ref().iter().map(|c| Returned { name: c.name_str().to_string(), code: c.column_type() as u8, charset: c.character_set() }).collect();
    let mut returned = 0;
    while let Some(row) = result.next().await? {
        returned += 1;
        each(&columns, &row.unwrap());
    }
    result.drop_result().await?;
    Ok((columns, returned))
}

/// Run a statement as text, its values already in it, and hand each row to `each`.
async fn query(conn: &mut Conn, text: &str, each: &mut dyn FnMut(&[Returned], &[My])) -> Result<(Vec<Returned>, usize), mysql_async::Error> {
    let mut result = conn.query_iter(text).await?;
    let columns: Vec<Returned> = result.columns_ref().iter().map(|c| Returned { name: c.name_str().to_string(), code: c.column_type() as u8, charset: c.character_set() }).collect();
    let mut returned = 0;
    while let Some(row) = result.next().await? {
        returned += 1;
        each(&columns, &row.unwrap());
    }
    result.drop_result().await?;
    Ok((columns, returned))
}

/// Seed the case's tables twice and run every statement over both states, each run under the
/// timeout, which a second session enforces with `KILL QUERY`.
async fn check(server: &mut Server, state: &mut State<'_>, statements: &[(String, String, Value)], timeout_ms: u64) -> Result<Report, String> {
    let mut problems = Vec::new();
    let mut notes = Vec::new();
    let mut tallies: Vec<Tally> = statements.iter().map(|_| Tally::default()).collect();
    for ((_, _, analysis), tally) in statements.iter().zip(tallies.iter_mut()) {
        // A statement the engine refused has no types to check, and may be one that is not run
        // lightly: a statement of a migration. A parameter as a whole sort or group key is an
        // error that leaves the statement typed, which is checked as any other.
        let refusal = |d: &&Value| d["code"].as_u64() != Some(u64::from(reylo_engine_core::analysis::CONSTANT_KEY));
        if let Some(d) = analysis["diagnostics"].as_array().and_then(|d| d.iter().find(refusal)) {
            tally.refused = Some(format!("the engine refused it: reylo{}: {}", d["code"], d["message"].as_str().unwrap_or("")));
        }
    }
    state.tables = tables(&mut state.conn).await?;
    let mut dirty = false;
    for scenario in SCENARIOS {
        if dirty {
            state.rebuild(server).await?;
        }
        let (counts, bare, mut checksum) = state.begin_seeded(scenario).await?;
        notes.push(format!("seeded with {}: {counts}", scenario.name()));
        if scenario == Scenario::Edges && !bare.is_empty() {
            notes.push(format!("no seed value for {}: the column holds NULL alone", bare.join(", ")));
        }
        let mut again = 0;
        for ((file, sql, analysis), tally) in statements.iter().zip(tallies.iter_mut()) {
            if tally.refused.is_some() {
                continue;
            }
            let text = analysis["rewritten"]["sql"].as_str().unwrap_or(sql).to_string();
            let order: Vec<String> = analysis["rewritten"]["order"].as_array().map(|o| o.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            let params = analysis["params"].as_array().cloned().unwrap_or_default();
            let cardinality = analysis["cardinality"].as_str().unwrap_or("many");
            let columns = analysis["columns"].as_array().cloned().unwrap_or_default();
            // A SELECT runs once more through a temporary table, where a plan the seeded tables
            // do not bring out would put it: the types and the documents are then the copies'.
            let mut forms = vec![(text.clone(), "", false)];
            forms.extend(buffered(&text).map(|text| (text, "; through SQL_BUFFER_RESULT", false)));
            // And once as a driver that binds nothing sends it: each value a literal in the text.
            let marks: Vec<(usize, usize)> = analysis["rewritten"]["spans"].as_array().map(|spans| spans.iter().filter_map(|s| Some((s[2].as_u64()? as usize, s[3].as_u64()? as usize))).collect()).unwrap_or_default();
            if !params.is_empty() && marks.len() == order.len() {
                forms.push((text.clone(), "; written into the text", true));
            }
            for (text, how, written) in &forms {
                if tally.refused.is_some() {
                    break;
                }
                let mut prepared: Option<(Option<Statement>, Vec<Vec<Bound>>)> = None;
                let mut r = 0;
                loop {
                    // The statement stays prepared across its runs, as in mysql2's statement cache, and
                    // is prepared again on the session a rebuild opened.
                    if prepared.is_none() && *written {
                        // One literal per JavaScript value, however a binding driver would send it.
                        let pools = params
                            .iter()
                            .map(|p| {
                                let mut pool: Vec<Bound> = Vec::new();
                                for bound in bindings(&p["type"], None) {
                                    if !pool.iter().any(|b| b.literal == bound.literal) {
                                        pool.push(Bound { shown: bound.literal.clone(), ..bound });
                                    }
                                }
                                pool
                            })
                            .collect();
                        prepared = Some((None, pools));
                    }
                    if prepared.is_none() {
                        let statement = match state.conn.prep(text).await {
                            Ok(statement) => statement,
                            // The second form is the oracle's own: where the server does not take it, it is left out.
                            Err(mysql_async::Error::Server(_)) if !how.is_empty() => break,
                            Err(mysql_async::Error::Server(e)) => {
                                tally.fail(format!("the server does not prepare it: {} ({})", e.message, e.code));
                                break;
                            }
                            Err(e) => return Err(e.to_string()),
                        };
                        // The client reads `:name` as a parameter of its own and would rewrite it.
                        if statement.num_params() as usize != order.len() {
                            tally.refused = Some(format!("prepared with {} placeholder(s) where the analysis names {}", statement.num_params(), order.len()));
                            state.conn.close(statement).await.map_err(|e| e.to_string())?;
                            break;
                        }
                        let hints: Vec<(u8, bool)> = statement.params().iter().map(|p| (p.column_type() as u8, p.flags().contains(mysql_async::consts::ColumnFlags::UNSIGNED_FLAG))).collect();
                        // The first `?` a name binds carries the definition mysql2 reads for it. A
                        // parameter typed nullable is bound as NULL too, last, as the caller may.
                        let pools = params
                            .iter()
                            .map(|p| {
                                let mut pool = bindings(&p["type"], order.iter().position(|n| Some(n.as_str()) == p["name"].as_str()).and_then(|i| hints.get(i).copied()));
                                if p["nullable"].as_bool().unwrap_or(false) {
                                    pool.push(Bound { value: My::NULL, literal: "NULL".into(), shown: "NULL".into() });
                                }
                                pool
                            })
                            .collect();
                        prepared = Some((Some(statement), pools));
                    }
                    let (statement, pools) = prepared.as_ref().expect("prepared above");
                    let all = assignments(pools);
                    let Some(values) = all.get(r) else { break };
                    r += 1;
                    // Each `?` is the n-th name of `order`, and the value is that parameter's.
                    let bind = |values: &[Bound]| -> Vec<My> { order.iter().map(|name| params.iter().position(|p| p["name"].as_str() == Some(name)).and_then(|i| values.get(i)).map_or(My::NULL, |b| b.value.clone())).collect() };
                    // The text with each `?` replaced, the last first so the offsets hold.
                    let literal_of = |values: &[Bound]| -> String {
                        let mut literal = text.clone();
                        if *written {
                            for (i, &(start, end)) in marks.iter().enumerate().rev() {
                                let value = params.iter().position(|p| p["name"].as_str() == order.get(i).map(String::as_str)).and_then(|p| values.get(p));
                                literal.replace_range(start..end, value.map_or("NULL", |b| b.literal.as_str()));
                            }
                        }
                        literal
                    };
                    let bound = bind(values);
                    let shown = format!("{}{how}", values.iter().map(|b| b.shown.clone()).collect::<Vec<_>>().join(", "));
                    let literal = literal_of(values);
                    state.conn.query_drop("SAVEPOINT q").await.map_err(|e| e.to_string())?;
                    // The rows are checked as they come, so a statement that returns rows without end
                    // holds none of them.
                    let mut found: Vec<String> = Vec::new();
                    let mut each = |returned: &[Returned], row: &[My]| {
                        for (i, col) in columns.iter().enumerate() {
                            let name = col["name"].as_str().unwrap_or("?");
                            match row.get(i) {
                                Some(My::NULL) if !col["nullable"].as_bool().unwrap_or(true) => found.push(format!("{file}: column {name} typed non-null is NULL with ({shown})")),
                                // A truth value the statement computes is 1 or 0; a TINYINT(1) or
                                // BIT(1) column holds what was stored, which the run time refuses
                                // unless it is one of them.
                                Some(value) if col["type_name"] == "boolean" && !matches!(value, My::NULL) && !is_truth(value) => found.push(format!("{file}: column {name} typed bool holds {value:?} with ({shown})")),
                                Some(My::Bytes(bytes)) if returned.get(i).is_some_and(|c| c.code == 245) => {
                                    if let Some(why) = serde_json::from_slice::<Value>(bytes).ok().and_then(|document| json_contradiction(&col["type"], &document, col["nullable"].as_bool().unwrap_or(true), "")) {
                                        found.push(format!("{file}: column {name} {why} with ({shown})"));
                                    }
                                }
                                _ => {}
                            }
                        }
                    };
                    let id = state.conn.id();
                    let (outcome, killed) = {
                        let mut run: std::pin::Pin<Box<dyn std::future::Future<Output = Result<(Vec<Returned>, usize), mysql_async::Error>> + '_>> = match statement {
                            Some(statement) => Box::pin(execute(&mut state.conn, statement, bound, &mut each)),
                            None => Box::pin(query(&mut state.conn, &literal, &mut each)),
                        };
                        match tokio::time::timeout(Duration::from_millis(timeout_ms), &mut run).await {
                            Ok(outcome) => (outcome, false),
                            Err(_) => {
                                server.admin.query_drop(format!("KILL QUERY {id}")).await.map_err(|e| e.to_string())?;
                                (run.await, true)
                            }
                        }
                    };
                    if killed {
                        // A kill that came after the statement ended would stop the next one instead.
                        let _ = state.conn.query_drop("DO 0").await;
                    }
                    found.sort();
                    found.dedup();
                    problems.append(&mut found);
                    let refused = !killed && matches!(&outcome, Err(mysql_async::Error::Server(e)) if refuses_null(e.code));
                    match outcome {
                        // A run the timeout stopped says nothing of the statement, sound or not.
                        _ if killed => tally.timed_out += 1,
                        Ok((returned, rows)) => {
                            tally.ran += 1;
                            if (cardinality == "maybeOne" && rows > 1) || (cardinality == "one" && rows != 1) || (cardinality == "none" && rows > 0) {
                                problems.push(format!("{file}: proven {cardinality}, returned {rows} rows with ({shown})"));
                            }
                            if returned.len() != columns.len() {
                                problems.push(format!("{file}: typed with {} column(s), returned {} with ({shown})", columns.len(), returned.len()));
                            }
                            for (col, returned) in columns.iter().zip(&returned) {
                                let name = col["name"].as_str().unwrap_or("?");
                                if returned.name != name {
                                    problems.push(format!("{file}: column {name} came back named {} with ({shown})", returned.name));
                                }
                                if let Some(why) = type_contradiction(&col["type"], returned.code, returned.charset) {
                                    problems.push(format!("{file}: column {name} {why} with ({shown})"));
                                }
                            }
                        }
                        Err(mysql_async::Error::Server(e)) => {
                            tally.fail(format!("{} ({})", e.message, e.code));
                            if let Some(why) = crate::errors::mysql_contradiction(&analysis["errors"], e.code, &e.message) {
                                problems.push(format!("{file}: {why} with ({shown})"));
                            }
                            // A NULL the engine let a parameter take, which the server refuses
                            // as a NOT NULL column's or a CHECK's, is a parameter typed nullable
                            // that may not be null. The NULLs are to blame where the same run with
                            // each of them given a value of its pool is not refused so; then each
                            // that is refused alone, the others given values, is named.
                            if refused {
                                let nulls: Vec<usize> = (0..params.len()).filter(|&i| values.get(i).is_some_and(|b| b.value == My::NULL) && params[i]["nullable"].as_bool().unwrap_or(false) && pools.get(i).is_some_and(|pool| pool.iter().any(|b| b.value != My::NULL))).collect();
                                let filled = |keep: Option<usize>| -> Vec<Bound> {
                                    let mut other = values.clone();
                                    for &i in nulls.iter().filter(|&&i| Some(i) != keep) {
                                        if let Some(value) = pools[i].iter().find(|b| b.value != My::NULL) {
                                            other[i] = value.clone();
                                        }
                                    }
                                    other
                                };
                                // Whether the run with these values is refused so; `None` where the timeout stopped it.
                                let mut tries: Vec<Vec<Bound>> = Vec::new();
                                if !nulls.is_empty() {
                                    tries.push(filled(None));
                                    if nulls.len() > 1 {
                                        tries.extend(nulls.iter().map(|&i| filled(Some(i))));
                                    }
                                }
                                // Each try's refusal, `Some(None)` where it was not refused so; `None` where the timeout stopped it.
                                let mut answers: Vec<Option<Option<String>>> = Vec::new();
                                for other in &tries {
                                    // Past the first, a try is needed only where the first was not refused.
                                    if answers.first().is_some_and(|a| a.as_ref().is_none_or(Option::is_some)) {
                                        break;
                                    }
                                    let mut none = |_: &[Returned], _: &[My]| {};
                                    let literal = literal_of(other);
                                    let (again, killed) = {
                                        let mut run: std::pin::Pin<Box<dyn std::future::Future<Output = Result<(Vec<Returned>, usize), mysql_async::Error>> + '_>> = match statement {
                                            Some(statement) => Box::pin(execute(&mut state.conn, statement, bind(other), &mut none)),
                                            None => Box::pin(query(&mut state.conn, &literal, &mut none)),
                                        };
                                        match tokio::time::timeout(Duration::from_millis(timeout_ms), &mut run).await {
                                            Ok(outcome) => (outcome, false),
                                            Err(_) => {
                                                server.admin.query_drop(format!("KILL QUERY {id}")).await.map_err(|e| e.to_string())?;
                                                (run.await, true)
                                            }
                                        }
                                    };
                                    if killed {
                                        let _ = state.conn.query_drop("DO 0").await;
                                        answers.push(None);
                                    } else {
                                        answers.push(Some(match &again {
                                            Err(mysql_async::Error::Server(e)) if refuses_null(e.code) => Some(format!("{} ({})", e.message, e.code)),
                                            _ => None,
                                        }));
                                    }
                                }
                                if answers.first() == Some(&Some(None)) {
                                    for (k, &i) in nulls.iter().enumerate() {
                                        let why = if nulls.len() == 1 { Some(format!("{} ({})", e.message, e.code)) } else { answers.get(k + 1).cloned().flatten().flatten() };
                                        if let Some(why) = why {
                                            problems.push(format!("{file}: parameter {} typed nullable, and the database refuses its NULL: {why} with ({shown})", params[i]["name"].as_str().unwrap_or("?")));
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => return Err(format!("{file}: {e}")),
                    }
                    // What the run changed is undone, and where it cannot be the state is built again.
                    let mut intact = match state.conn.query_drop("ROLLBACK TO SAVEPOINT q").await {
                        Ok(()) => true,
                        // "SAVEPOINT q does not exist": the run ended the transaction.
                        Err(mysql_async::Error::Server(e)) if e.code == 1305 => false,
                        Err(e) => return Err(e.to_string()),
                    };
                    if intact && text.contains('@') {
                        let set: Vec<String> = state.conn.query("select variable_name from performance_schema.user_variables_by_thread where thread_id = ps_current_thread_id() and variable_value is not null").await.map_err(|e| e.to_string())?;
                        for variable in set {
                            state.conn.query_drop(format!("SET @{} = NULL", backticked(&variable))).await.map_err(|e| e.to_string())?;
                        }
                    }
                    if intact && !state.loose().is_empty() {
                        intact = state.checksum().await? == checksum;
                    }
                    if intact && analysis["kind"].as_str() != Some("select") && !state.counters.is_empty() {
                        intact = !state.counter_jumped().await?;
                    }
                    if !intact {
                        again += 1;
                        prepared = None;
                        state.rebuild(server).await?;
                        checksum = state.begin_seeded(scenario).await?.2;
                    }
                }
                if let Some((Some(statement), _)) = prepared {
                    state.conn.close(statement).await.map_err(|e| e.to_string())?;
                }
            }
        }
        state.conn.query_drop("ROLLBACK").await.map_err(|e| e.to_string())?;
        if again > 0 {
            notes.push(format!("built and seeded again {again} time(s) with {}: a run ended the transaction, wrote a table no rollback restores, or moved an AUTO_INCREMENT counter far", scenario.name()));
        }
        // The rows seeded into a table no rollback restores are still there.
        dirty = !state.loose().is_empty();
    }
    Ok(report(statements, &tallies, timeout_ms, problems, notes))
}

/// Whether a server error is a NOT NULL column's refusal of NULL (1048, "Column … cannot be null")
/// or a CHECK's (3819), which a NULL bound to a parameter may cause.
fn refuses_null(code: u16) -> bool {
    code == 1048 || code == 3819
}

/// A SELECT with SQL_BUFFER_RESULT, which has the server put the result in a temporary table and
/// changes nothing else; `None` for any other statement, and for one with an optimizer hint,
/// which must follow the SELECT at once.
fn buffered(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let at = text.len() - text.trim_start().len();
    let select = lower[at..].starts_with("select") && lower[at + 6..].starts_with(|c: char| c.is_whitespace());
    (select && !lower.contains("/*+") && !lower.contains("sql_buffer_result")).then(|| format!("{} SQL_BUFFER_RESULT{}", &text[..at + 6], &text[at + 6..]))
}

/// The engine's name for the type of a column of this protocol type and character set, as
/// `column_type` in the engine's `mysql.rs` reads a prepare's: what a driver decodes the value
/// by. `None` for the NULL type, which carries no value.
fn returned_type(code: u8, charset: u16) -> Option<&'static str> {
    Some(match code {
        1 | 2 | 13 => "int2",
        3 | 9 => "int4",
        8 => "int8",
        4 => "float4",
        5 => "float8",
        0 | 246 => "numeric",
        7 | 12 => "timestamp",
        10 | 14 => "date",
        11 => "time",
        245 => "json",
        16 => "bytea",
        249..=254 | 15 => {
            if charset == 63 { "bytea" } else { "text" }
        }
        6 => return None,
        _ => "unknown",
    })
}

/// Whether a value is a truth value as MySQL computes one: the integer 1 or 0, in either protocol.
fn is_truth(value: &My) -> bool {
    match value {
        My::Int(n) => *n == 0 || *n == 1,
        My::UInt(n) => *n == 0 || *n == 1,
        My::Bytes(bytes) => bytes.as_slice() == b"0" || bytes.as_slice() == b"1",
        _ => false,
    }
}

/// Why the type the execution gave a column contradicts the engine's, if it does. The integers
/// below 64 bits and the floats are one type, a number in every driver; a 64-bit integer is not,
/// being a BigInt. A scalar typed as either of several types (`int8|float8`) may come back as any
/// of them. A truth value is an integer of any width to the protocol, or a BIT(1)'s byte.
fn type_contradiction(ty: &Value, code: u8, charset: u16) -> Option<String> {
    let class = |name: &str| match name {
        "int2" | "int4" | "float4" | "float8" => vec!["number".to_string()],
        "bool" => vec!["number".to_string(), "int8".to_string(), "bytea".to_string()],
        other => vec![other.to_string()],
    };
    let typed: Vec<String> = match ty["kind"].as_str() {
        Some("scalar") => ty["name"].as_str().unwrap_or("?").split('|').flat_map(class).collect(),
        Some("json" | "object" | "list") => vec!["json".to_string()],
        _ => return None,
    };
    let returned = class(returned_type(code, charset)?).remove(0);
    (!typed.contains(&returned)).then(|| format!("typed {} came back as {returned} (protocol type {code})", typed.join(" or ")))
}

/// Why a JSON document contradicts the shape the engine typed, if it does: a null where the type
/// has none, a missing key, or a value of another JSON type than its field's.
fn json_contradiction(ty: &Value, json: &Value, nullable: bool, path: &str) -> Option<String> {
    let at = if path.is_empty() { "the top".to_string() } else { path.to_string() };
    // A JSON column a CHECK holds to a JSON Schema: the value is one the engine's schema admits,
    // a JSON null included, unless the place may hold SQL's NULL, which a document writes alike.
    if let (Some("json"), Some(schema)) = (ty["kind"].as_str(), ty.get("schema")) {
        return (!(json.is_null() && nullable) && !reylo_engine_core::schema_type::admits(schema, json)).then(|| format!("holds {json} at {at}, which the schema {schema} does not admit"));
    }
    if json.is_null() {
        return (!nullable && matches!(ty["kind"].as_str(), Some("object" | "list" | "scalar"))).then(|| format!("has a null at {at}, typed non-null"));
    }
    let kind = match json {
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        _ => "an object",
    };
    match ty["kind"].as_str() {
        Some("object") => {
            let Some(object) = json.as_object() else { return Some(format!("holds {kind} at {at}, typed an object")) };
            for f in ty["fields"].as_array()? {
                let name = f["name"].as_str()?;
                let Some(value) = object.get(name) else { return Some(format!("has no key {path}.{name}")) };
                if let Some(why) = json_contradiction(&f["type"], value, f["nullable"].as_bool().unwrap_or(true), &format!("{path}.{name}")) {
                    return Some(why);
                }
            }
            None
        }
        Some("list") => {
            let Some(items) = json.as_array() else { return Some(format!("holds {kind} at {at}, typed a list")) };
            items.iter().enumerate().find_map(|(i, item)| json_contradiction(&ty["of"], item, ty["nullable_items"].as_bool().unwrap_or(true), &format!("{path}[{i}]")))
        }
        // What a driver finds in the parsed document, by `json` in `src/render.ts`.
        Some("scalar") => {
            let name = ty["name"].as_str()?;
            let mut wanted = Vec::new();
            for part in name.split('|') {
                wanted.push(match part {
                    "bool" => "a boolean",
                    "int2" | "int4" | "int8" | "float4" | "float8" | "numeric" => "a number",
                    "text" | "date" | "timestamp" | "time" | "bytea" => "a string",
                    _ => return None,
                });
            }
            (!wanted.contains(&kind)).then(|| format!("holds {kind} at {at}, typed {name}"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(data_type: &str, column_type: &str) -> Column {
        Column { name: "c".into(), data_type: data_type.into(), unsigned: column_type.contains("unsigned"), labels: labels(column_type), nullable: true, generated: false, auto_increment: false, precision: 30, scale: 20, length: 10, fraction: 0 }
    }

    #[test]
    fn a_pool_holds_its_types_bounds_in_the_edge_state_alone() {
        let edges = pool(&column("tinyint", "tinyint"), Scenario::Edges);
        assert_eq!(edges, ["NULL", "1", "2", "0", "-1", "-128", "127"]);
        assert_eq!(pool(&column("tinyint", "tinyint unsigned"), Scenario::Edges), ["NULL", "1", "2", "0", "255"]);
        assert_eq!(pool(&column("tinyint", "tinyint"), Scenario::Plain), ["NULL", "1", "2", "0", "-1"]);
        let bigint = pool(&column("bigint", "bigint unsigned"), Scenario::Edges);
        assert!(bigint.contains(&"18446744073709551615".to_string()) && TWINS.iter().all(|t| bigint.contains(&t.to_string())), "{bigint:?}");
        // An AUTO_INCREMENT key leaves room for the next INSERT without a key.
        let key = Column { auto_increment: true, ..column("int", "int") };
        assert!(pool(&key, Scenario::Edges).contains(&"1073741823".to_string()) && !pool(&key, Scenario::Edges).contains(&"2147483647".to_string()));
        // DECIMAL(30,20): the largest value, and one that is 1 as a double.
        let decimal = pool(&column("decimal", "decimal(30,20)"), Scenario::Edges);
        assert!(decimal.contains(&"9999999999.99999999999999999999".to_string()) && decimal.contains(&"1.00000000000000000001".to_string()), "{decimal:?}");
        assert!(pool(&column("date", "date"), Scenario::Edges).contains(&"'0000-00-00'".to_string()));
        assert!(!pool(&column("date", "date"), Scenario::Plain).contains(&"'0000-00-00'".to_string()));
        assert_eq!(pool(&column("enum", "enum('a','it''s')"), Scenario::Plain), ["NULL", "'a'", "'it\\'s'"]);
        assert_eq!(pool(&column("set", "set('x','y','z')"), Scenario::Plain), ["NULL", "''", "'x'", "'y'", "'x,y'"]);
        assert_eq!(fresh_value(&column("tinyint", "tinyint"), 500).as_deref(), Some("23"));
        assert_eq!(fresh_value(&column("varchar", "varchar(10)"), 7).as_deref(), Some("'k7'"));
        assert_eq!(fresh_value(&Column { length: 1, ..column("char", "char(1)") }, 7), None);
    }

    #[test]
    fn a_number_is_bound_as_mysql2_binds_it() {
        let int = json!({ "kind": "scalar", "name": "int4" });
        // A safe integer inside the placeholder's type goes as that integer, anything else as a DOUBLE.
        assert_eq!(bound(&Js::Number(1.0), Some((8, false)), true).value, My::Int(1));
        assert_eq!(bound(&Js::Number(1.0), Some((8, true)), true).value, My::UInt(1));
        assert_eq!(bound(&Js::Number(-1.0), Some((8, true)), true).value, My::Double(-1.0));
        assert_eq!(bound(&Js::Number(1.5), Some((8, false)), true).value, My::Double(1.5));
        assert_eq!(bound(&Js::Number(9007199254740992.0), Some((8, false)), true).value, My::Double(9007199254740992.0));
        assert_eq!(bound(&Js::Number(200.0), Some((1, false)), true).value, My::Double(200.0));
        assert_eq!(bound(&Js::Number(1.0), Some((253, false)), true).value, My::Double(1.0));
        assert_eq!(bound(&Js::Number(1.0), Some((8, false)), false).value, My::Double(1.0));
        let all = bindings(&int, Some((8, false)));
        assert!(all.iter().any(|b| b.value == My::Int(1)) && all.iter().any(|b| b.value == My::Double(1.0)), "{all:?}");
        assert_eq!(bindings(&json!({ "kind": "scalar", "name": "text" }), Some((253, false)))[0], Bound { value: My::Bytes(b"a".to_vec()), literal: "'a'".into(), shown: "'a'".into() });
        // What a driver that binds nothing writes into the text.
        let literal = |js: Js| bound(&js, None, true).literal;
        assert_eq!(literal(Js::Number(1.0)), "1");
        assert_eq!(literal(Js::Number(-1.5)), "-1.5");
        assert_eq!(literal(Js::Number(1e21)), "1e21");
        assert_eq!(literal(Js::Number(18446744073709551616.0)), "18446744073709552000");
        assert_eq!(literal(Js::Number(-0.0)), "-0");
        assert_eq!(literal(Js::Text("it's")), "'it\\'s'");
        assert_eq!(literal(Js::Bytes(b"a\0")), "X'6100'");
        assert_eq!(literal(Js::Date(2026, 1, 2, 3)), "'2026-01-02 03:00:00.000'");
    }

    #[test]
    fn a_catalog_query_takes_its_values_as_mysql2_writes_them() {
        assert_eq!(interpolate("select find_in_set(t, ?)", &[json!("users,it's\\")]).as_deref(), Some("select find_in_set(t, 'users,it\\'s\\\\')"));
        assert_eq!(interpolate("select ?, ?", &[json!(1), Value::Null]).as_deref(), Some("select 1, NULL"));
        assert_eq!(interpolate("select ?", &[]), None);
        assert_eq!(decoded(My::Bytes(b"12".to_vec()), 8), json!(12));
        assert_eq!(decoded(My::Bytes(b"12".to_vec()), 253), json!("12"));
        assert_eq!(decoded(My::NULL, 253), Value::Null);
    }

    #[test]
    fn a_select_is_run_once_more_through_a_temporary_table() {
        assert_eq!(buffered("  select id\nfrom t").as_deref(), Some("  select SQL_BUFFER_RESULT id\nfrom t"));
        assert_eq!(buffered("SELECT\tdistinct id from t").as_deref(), Some("SELECT SQL_BUFFER_RESULT\tdistinct id from t"));
        for other in ["with w as (select 1) select * from w", "selected", "insert into t select 1", "select /*+ MAX_EXECUTION_TIME(1) */ 1", "select sql_buffer_result 1", "(select 1) union (select 2)"] {
            assert_eq!(buffered(other), None, "{other}");
        }
    }

    #[test]
    fn a_returned_type_is_held_against_the_engines() {
        let scalar = |name: &str| json!({ "kind": "scalar", "name": name });
        assert_eq!(type_contradiction(&scalar("int4"), 3, 63), None);
        assert_eq!(type_contradiction(&scalar("int4"), 5, 63), None);
        assert_eq!(type_contradiction(&scalar("int8"), 5, 63).as_deref(), Some("typed int8 came back as number (protocol type 5)"));
        assert_eq!(type_contradiction(&scalar("text"), 253, 63).as_deref(), Some("typed text came back as bytea (protocol type 253)"));
        assert_eq!(type_contradiction(&scalar("date"), 12, 63).as_deref(), Some("typed date came back as timestamp (protocol type 12)"));
        assert_eq!(type_contradiction(&json!({ "kind": "object", "fields": [] }), 245, 63), None);
        assert_eq!(type_contradiction(&json!({ "kind": "json" }), 251, 63).as_deref(), Some("typed json came back as bytea (protocol type 251)"));
        // Either of two types.
        assert_eq!(type_contradiction(&scalar("int8|float8"), 5, 63), None);
        assert_eq!(type_contradiction(&scalar("int8|float8"), 8, 63), None);
        assert_eq!(type_contradiction(&scalar("int8|float8"), 246, 63).as_deref(), Some("typed int8 or number came back as numeric (protocol type 246)"));
        assert_eq!(type_contradiction(&scalar("date|timestamp"), 12, 63), None);
        // A truth value: a TINYINT(1), a BIT(1), or a comparison the server computes as a BIGINT.
        for code in [1, 16, 8, 3] {
            assert_eq!(type_contradiction(&scalar("bool"), code, 63), None);
        }
        assert_eq!(type_contradiction(&scalar("bool"), 253, 33).as_deref(), Some("typed number or int8 or bytea came back as text (protocol type 253)"));
        assert!(is_truth(&My::Int(1)) && is_truth(&My::UInt(0)) && is_truth(&My::Bytes(b"1".to_vec())));
        assert!(!is_truth(&My::Int(2)) && !is_truth(&My::Bytes(b"-1".to_vec())));
        // The NULL type carries no value, and an unknown type claims nothing.
        assert_eq!(type_contradiction(&scalar("int8"), 6, 63), None);
        assert_eq!(type_contradiction(&json!({ "kind": "unknown" }), 8, 63), None);
    }

    #[test]
    fn a_json_document_is_held_against_its_shape() {
        let shape = json!({ "kind": "object", "fields": [
            { "name": "id", "type": { "kind": "scalar", "name": "int4" }, "nullable": false },
            { "name": "ok", "type": { "kind": "scalar", "name": "bool" }, "nullable": true },
            { "name": "tags", "type": { "kind": "list", "of": { "kind": "scalar", "name": "text" }, "nullable_items": false }, "nullable": false },
        ] });
        let check = |document: Value| json_contradiction(&shape, &document, false, "");
        assert_eq!(check(json!({ "id": 1, "ok": null, "tags": ["a"] })), None);
        assert_eq!(check(json!({ "id": null, "ok": true, "tags": [] })).as_deref(), Some("has a null at .id, typed non-null"));
        assert_eq!(check(json!({ "id": "1", "ok": true, "tags": [] })).as_deref(), Some("holds a string at .id, typed int4"));
        assert_eq!(check(json!({ "id": 1, "ok": 1, "tags": [] })).as_deref(), Some("holds a number at .ok, typed bool"));
        let either = json!({ "kind": "scalar", "name": "bool|int8" });
        assert_eq!(json_contradiction(&either, &json!(true), false, ".ok"), None);
        assert_eq!(json_contradiction(&either, &json!(1), false, ".ok"), None);
        assert_eq!(json_contradiction(&either, &json!("1"), false, ".ok").as_deref(), Some("holds a string at .ok, typed bool|int8"));
        assert_eq!(check(json!({ "id": 1, "ok": true, "tags": [null] })).as_deref(), Some("has a null at .tags[0], typed non-null"));
        assert_eq!(check(json!({ "id": 1, "ok": true })).as_deref(), Some("has no key .tags"));
        assert_eq!(check(json!([1])).as_deref(), Some("holds a list at the top, typed an object"));
        assert_eq!(check(Value::Null).as_deref(), Some("has a null at the top, typed non-null"));
        assert_eq!(json_contradiction(&shape, &Value::Null, true, ""), None);
    }
}
