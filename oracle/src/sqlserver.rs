//! The SQL Server half: the host the engine's replay asks, and the seeding and the runs.
//!
//! The host is the native program's own connection (`reylo_engine_native::sqlserver`, tiberius over
//! TLS), and the engine's needs are its two describe queries, `sp_describe_undeclared_parameters`
//! and `sys.dm_exec_describe_first_result_set`, which it runs on the case's database. A case runs
//! in a database of its own, made on the server `--sqlserver-url` (or `ORACLE_SQLSERVER_URL`) names,
//! which must be a throwaway one: it is refused on port 1433. Its `setup.sql` runs batch by batch, as
//! its `GO` lines part it.
//!
//! Each state is seeded in a transaction that is never committed, and every run sits between
//! `SAVE TRANSACTION q` and `ROLLBACK TRANSACTION q`. A run that ends the transaction or dooms it
//! (`XACT_STATE() = -1`), or that the timeout stops (the session is then closed, since tiberius
//! cannot cancel a request), leaves the state to be built again on a new session before the next run.
//! A column's pool is its type's: an integer's bounds, a decimal's largest value, money's bounds,
//! strings that differ by case and by a trailing space, the longest string a column takes, a
//! character outside the Basic Multilingual Plane, the first and the last date, time and offset each
//! type holds. Identity, computed and rowversion columns take what the server gives them.
//!
//! A statement runs as the `mssql` run time runs it: through `sp_executesql`, its `:name`
//! placeholders written `@p_1`, `@p_2`, … and each declared as the run time declares it from the
//! parameter's type, with each value as a literal the declared type converts. Every row of a run
//! that succeeded is checked: no NULL in a column typed non-null, the row count within the
//! cardinality, no repeat of the result key, the order where it is total, each value within its
//! type (a text's length in its unit, a decimal's precision and scale, a time's tick), and the
//! type each value comes back as, which is what a driver decodes by: a column the engine types
//! `int` that comes back as a `bigint` contradicts it.

use crate::seed::{assignments, contradiction, key_repeats, length_contradiction, out_of_order, report, tick_digits, Report, Scenario, Tally, EMPTY, SCENARIOS};
use reylo_engine::AnswerCache;
use reylo_engine_core::types::{Kind, SizeUnit, Type};
use reylo_engine_native::sqlserver::Conn;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tiberius::ColumnData;

pub struct Server {
    admin: Conn,
    url: String,
    /// The major version: 15 for SQL Server 2019, 16 for 2022, 17 for 2025.
    pub version: i32,
}

/// `url` with its database named `db`.
fn with_database(url: &str, db: &str) -> String {
    let Some(scheme) = url.find("://") else { return url.to_string() };
    let rest = &url[scheme + 3..];
    let authority = rest.find(['/', '?']).unwrap_or(rest.len());
    let query = rest[authority..].find('?').map(|q| &rest[authority + q..]).unwrap_or("");
    format!("{}{}/{db}{query}", &url[..scheme + 3], &rest[..authority])
}

fn port_of(url: &str) -> Option<u16> {
    let rest = &url[url.find("://")? + 3..];
    let authority = &rest[..rest.find(['/', '?']).unwrap_or(rest.len())];
    let host = authority.rsplit('@').next()?;
    Some(host.rsplit_once(':').and_then(|(_, p)| p.parse().ok()).unwrap_or(1433))
}

pub async fn connect(url: &str) -> Result<Server, String> {
    if port_of(url) == Some(1433) {
        return Err("port 1433 is SQL Server's own; the oracle runs only on a throwaway server on another port".into());
    }
    let (admin, version) = reylo_engine_native::sqlserver::open(&with_database(url, "master")).await.map_err(|f| f.to_string())?;
    let version = version.split('.').next().and_then(|v| v.parse().ok()).unwrap_or(0);
    Ok(Server { admin, url: url.to_string(), version })
}

async fn session(url: &str) -> Result<Conn, String> {
    let (mut conn, _) = reylo_engine_native::sqlserver::open(url).await.map_err(|f| f.to_string())?;
    exec(&mut conn, "set xact_abort off").await?;
    Ok(conn)
}

fn message(e: &tiberius::error::Error) -> String {
    match e {
        tiberius::error::Error::Server(t) => format!("{} ({})", t.message(), t.code()),
        other => other.to_string(),
    }
}

fn code(e: &tiberius::error::Error) -> u32 {
    match e {
        tiberius::error::Error::Server(t) => t.code(),
        _ => 0,
    }
}

async fn exec(conn: &mut Conn, sql: &str) -> Result<(), String> {
    conn.simple_query(sql).await.map_err(|e| message(&e))?.into_results().await.map_err(|e| message(&e))?;
    Ok(())
}

/// The first value of every row of the first result set, as text.
async fn texts(conn: &mut Conn, sql: &str) -> Result<Vec<Vec<Option<String>>>, String> {
    let results = conn.simple_query(sql).await.map_err(|e| message(&e))?.into_results().await.map_err(|e| message(&e))?;
    Ok(results.into_iter().next().unwrap_or_default().into_iter().map(|row| row.into_iter().map(|v| text(&v)).collect()).collect())
}

/// A SQL Server script's batches, as its `GO` lines part them.
fn batches(sql: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    for line in sql.lines() {
        if line.trim().eq_ignore_ascii_case("go") {
            out.push(String::new());
        } else {
            let last = out.last_mut().expect("one batch at least");
            last.push_str(line);
            last.push('\n');
        }
    }
    out.into_iter().filter(|b| !b.trim().is_empty()).collect()
}

async fn drop_database(admin: &mut Conn, db: &str) -> Result<(), String> {
    exec(admin, &format!("if db_id(N'{db}') is not null begin alter database [{db}] set single_user with rollback immediate; drop database [{db}]; end")).await
}

pub async fn run(case: &Path, server: &mut Server, keep: bool, timeout_ms: u64, given: &Map<String, Value>, dumped: &mut Map<String, Value>, statements: Vec<(String, String)>) -> Result<(Vec<String>, Vec<String>), String> {
    let name: String = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let db = format!("reylo_oracle_{}", name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect::<String>());
    drop_database(&mut server.admin, &db).await.map_err(|e| format!("drop {db}: {e}"))?;
    exec(&mut server.admin, &format!("create database [{db}]")).await.map_err(|e| format!("create {db}: {e}"))?;
    let url = with_database(&server.url, &db);
    let setup = fs::read_to_string(case.join("setup.sql")).map_err(|e| e.to_string())?;
    let mut conn = session(&url).await?;
    for batch in batches(&setup) {
        exec(&mut conn, &batch).await.map_err(|e| format!("setup.sql: {e}"))?;
    }
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
    same_under_other_settings(&mut conn, &name, &analyzed, given).await?;
    drop(conn);
    // A value that contradicts a type is a failure whatever expected.txt says.
    let report = check(&url, &analyzed, timeout_ms).await;
    if !keep {
        let _ = drop_database(&mut server.admin, &db).await;
    }
    let unchecked = crate::seeded(case, lines.len(), report.map_err(|e| format!("seeding: {e}"))?)?;
    Ok((lines, unchecked))
}

async fn analyze(conn: &mut Conn, sql: &str) -> Result<Value, String> {
    let analysis = reylo_engine::clients::tiberius::analyze(conn, sql, &mut AnswerCache::default()).await.map_err(|e| e.to_string())?;
    serde_json::to_value(&analysis).map_err(|e| e.to_string())
}

/// What a SQL Server session sets that changes how it reads and prints a date, and that no
/// statement's type turns on, each with another of its values, and the value to set it back to.
const OTHER_SETTINGS: [(&str, &str); 3] = [("set dateformat dmy", "set dateformat mdy"), ("set datefirst 1", "set datefirst 7"), ("set language british", "set language us_english")];

/// Each statement analyzed again under other session settings, which must change nothing.
async fn same_under_other_settings(conn: &mut Conn, case: &str, analyzed: &[(String, String, Value)], given: &Map<String, Value>) -> Result<(), String> {
    for (other, _) in OTHER_SETTINGS {
        exec(conn, other).await?;
    }
    let mut outcome = Ok(());
    for (file, sql, analysis) in analyzed.iter().filter(|(file, _, _)| !given.contains_key(&format!("{case}/{file}"))) {
        match analyze(conn, sql).await {
            Ok(again) if again == *analysis => {}
            Ok(again) => {
                outcome = Err(format!("{file}: another session's settings change its analysis: {} became {}", crate::line(analysis), crate::line(&again)));
                break;
            }
            Err(e) => {
                outcome = Err(format!("{file}: under another session's settings: {e}"));
                break;
            }
        }
    }
    for (_, back) in OTHER_SETTINGS {
        exec(conn, back).await?;
    }
    outcome
}

// ---------------------------------------------------------------------------------------------
// Values as text
// ---------------------------------------------------------------------------------------------

/// The civil date `days` after 0001-01-01.
fn civil(days: i64) -> String {
    // Days from 1970-01-01, then Howard Hinnant's civil_from_days.
    let z = days - 719_162 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A time of day from a count of `1/10^scale` seconds, its fraction's trailing zeros left out.
fn clock_text(increments: u64, scale: u8) -> String {
    let per = 10u64.pow(u32::from(scale));
    let (seconds, fraction) = (increments / per, increments % per);
    let base = format!("{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60);
    if scale == 0 || fraction == 0 {
        return base;
    }
    let digits = format!("{fraction:0width$}", width = usize::from(scale));
    format!("{base}.{}", digits.trim_end_matches('0'))
}

/// A decimal's exact text, its scale's digits after the point.
fn numeric_text(n: tiberius::numeric::Numeric) -> String {
    let (value, scale) = (n.value(), u32::from(n.scale()));
    let digits = value.unsigned_abs().to_string();
    let sign = if value < 0 { "-" } else { "" };
    if scale == 0 {
        return format!("{sign}{digits}");
    }
    let padded = format!("{digits:0>width$}", width = scale as usize + 1);
    let (whole, fraction) = padded.split_at(padded.len() - scale as usize);
    format!("{sign}{whole}.{fraction}")
}

/// A value as text, as the checks read it: a number in its digits, a date and a time in ISO 8601.
fn text(value: &ColumnData<'_>) -> Option<String> {
    use ColumnData::*;
    match value {
        U8(v) => v.map(|v| v.to_string()),
        I16(v) => v.map(|v| v.to_string()),
        I32(v) => v.map(|v| v.to_string()),
        I64(v) => v.map(|v| v.to_string()),
        F32(v) => v.map(|v| v.to_string()),
        F64(v) => v.map(|v| v.to_string()),
        Bit(v) => v.map(|v| if v { "1".into() } else { "0".into() }),
        String(v) => v.as_deref().map(str::to_string),
        Guid(v) => v.map(|v| v.to_string()),
        Binary(v) => v.as_deref().map(|b| b.iter().map(|byte| format!("{byte:02x}")).collect()),
        Numeric(v) => v.map(numeric_text),
        Xml(v) => v.as_deref().map(|x| x.to_string()),
        Date(v) => v.map(|d| civil(i64::from(d.days()))),
        Time(v) => v.map(|t| clock_text(t.increments(), t.scale())),
        DateTime2(v) => v.map(|dt| format!("{}T{}", civil(i64::from(dt.date().days())), clock_text(dt.time().increments(), dt.time().scale()))),
        DateTimeOffset(v) => v.map(|o| {
            let (dt, offset) = (o.datetime2(), o.offset());
            // The stored date and time are UTC: the local time is the offset's minutes later.
            let scale = dt.time().scale();
            let per_day = 86_400 * 10i64.pow(u32::from(scale));
            let local = i64::from(dt.date().days()) * per_day + dt.time().increments() as i64 + i64::from(offset) * 60 * 10i64.pow(u32::from(scale));
            let sign = if offset < 0 { '-' } else { '+' };
            format!("{}T{}{sign}{:02}:{:02}", civil(local.div_euclid(per_day)), clock_text(local.rem_euclid(per_day) as u64, scale), offset.unsigned_abs() / 60, offset.unsigned_abs() % 60)
        }),
        DateTime(v) => v.map(|dt| {
            // Days from 1900-01-01, and 1/300 seconds, which print as milliseconds.
            let ms = (u64::from(dt.seconds_fragments()) * 10 + 1) / 3;
            format!("{}T{}", civil(i64::from(dt.days()) + 693_595), clock_text(ms, 3))
        }),
        SmallDateTime(v) => v.map(|dt| format!("{}T{}", civil(i64::from(dt.days()) + 693_595), clock_text(u64::from(dt.seconds_fragments()) * 60, 0))),
    }
}

/// What kind of value a driver decodes a value of this variant as, by the engine's type: None where
/// it agrees, else why not.
fn variant_contradiction(ty: &Type, value: &ColumnData<'_>) -> Option<String> {
    use ColumnData::*;
    let label = ty.label.as_deref().unwrap_or_default();
    let ok = match (&ty.base().kind, value) {
        (Kind::Integer(i), U8(_)) => i.bits == 8,
        (Kind::Integer(i), I16(_)) => i.bits == 16,
        (Kind::Integer(i), I32(_)) => i.bits == 32,
        (Kind::Integer(i), I64(_)) => i.bits == 64,
        (Kind::Boolean(_), Bit(_)) => true,
        (Kind::Float(_), F32(_)) => label == "real" || label == "float",
        (Kind::Float(_), F64(_)) => label == "float",
        // tiberius reads money as a float; the run time's driver reads it as a decimal.
        (Kind::Decimal(_), F64(_)) => matches!(label, "money" | "smallmoney"),
        (Kind::Decimal(_), Numeric(_)) => matches!(label, "decimal" | "numeric"),
        (Kind::Text(_), String(_)) => true,
        (Kind::Bytes(_), Binary(_)) => true,
        (Kind::Uuid(_), Guid(_)) => true,
        (Kind::Date(_), Date(_)) => true,
        (Kind::Time(_), Time(_)) => true,
        (Kind::DateTime(_), DateTime2(_)) => label == "datetime2",
        (Kind::DateTime(_), DateTime(_)) => label == "datetime",
        (Kind::DateTime(_), SmallDateTime(_)) => label == "smalldatetime",
        (Kind::ZonedDateTime(_), DateTimeOffset(_)) => true,
        (Kind::Xml(_), Xml(_)) => true,
        (Kind::Json(_), String(_)) => true,
        (Kind::Opaque, _) | (Kind::Unknown, _) | (Kind::Enum(_), _) => true,
        _ => false,
    };
    (!ok).then(|| format!("typed {} comes back as {}", ty.short(), variant_name(value)))
}

fn variant_name(value: &ColumnData<'_>) -> &'static str {
    use ColumnData::*;
    match value {
        U8(_) => "a tinyint",
        I16(_) => "a smallint",
        I32(_) => "an int",
        I64(_) => "a bigint",
        F32(_) => "a real",
        F64(_) => "a float",
        Bit(_) => "a bit",
        String(_) => "a string",
        Guid(_) => "a uniqueidentifier",
        Binary(_) => "binary",
        Numeric(_) => "a decimal",
        Xml(_) => "xml",
        DateTime(_) => "a datetime",
        SmallDateTime(_) => "a smalldatetime",
        Time(_) => "a time",
        Date(_) => "a date",
        DateTime2(_) => "a datetime2",
        DateTimeOffset(_) => "a datetimeoffset",
    }
}

/// Why a value contradicts its type in what the shared checks don't read: a text's length in
/// UTF-16 code units or bytes, binary's length, and a zoned time's tick.
fn sized_contradiction(ty: &Type, value: &ColumnData<'_>, text: &str) -> Option<String> {
    match (&ty.base().kind, value) {
        (Kind::Text(t), ColumnData::String(Some(s))) => {
            let size = t.size.as_ref()?;
            match size.unit {
                Some(SizeUnit::CodeUnit) => length_contradiction(size, s.encode_utf16().count() as u64, "UTF-16 code units"),
                Some(SizeUnit::Byte) if t.encoding.as_deref() == Some("UTF-8") => length_contradiction(size, s.len() as u64, "bytes"),
                Some(SizeUnit::Byte) => length_contradiction(size, s.chars().count() as u64, "bytes"),
                _ => None,
            }
        }
        (Kind::Bytes(b), ColumnData::Binary(Some(bytes))) => b.size.as_ref().and_then(|size| length_contradiction(size, bytes.len() as u64, "bytes")),
        (Kind::ZonedDateTime(z), ColumnData::DateTimeOffset(Some(_))) => {
            let allowed = tick_digits(&z.tick)?;
            let time = text.split('T').nth(1)?;
            let seconds = time.split(['+', '-']).next()?;
            let digits = seconds.split_once('.').map_or(0, |(_, f)| f.len());
            (digits > allowed).then(|| format!("ticking by {} holds {text}", z.tick))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Seeding
// ---------------------------------------------------------------------------------------------

struct Column {
    name: String,
    /// The literals a CHECK compares the column with (`[status]='open'`), which its pool tries first.
    listed: Vec<String>,
    /// The system type it is of (an alias type's base), with its length, precision and scale.
    ty: String,
    max_length: i64,
    precision: u32,
    scale: u32,
    nullable: bool,
}

struct Table {
    name: String,
    columns: Vec<Column>,
    /// The columns of each unique key and index, by position in `columns`.
    keys: Vec<Vec<usize>>,
}

const TABLES: &str = "select quotename(schema_name(t.schema_id)) + '.' + quotename(t.name) as tbl, quotename(c.name) as col, type_name(c.system_type_id) as type, c.max_length, c.precision, c.scale, c.is_nullable,
    case when c.is_identity = 1 or c.is_computed = 1 or type_name(c.system_type_id) = 'timestamp' then 1 else 0 end as skip, c.column_id
  from sys.tables t join sys.columns c on c.object_id = t.object_id
  where t.is_ms_shipped = 0 order by t.object_id, c.column_id";

const KEYS: &str = "select quotename(schema_name(t.schema_id)) + '.' + quotename(t.name) as tbl, i.index_id, quotename(c.name) as col
  from sys.tables t join sys.indexes i on i.object_id = t.object_id and i.is_unique = 1 join sys.index_columns ic on ic.object_id = i.object_id and ic.index_id = i.index_id and ic.key_ordinal > 0
  join sys.columns c on c.object_id = ic.object_id and c.column_id = ic.column_id
  where t.is_ms_shipped = 0 order by t.object_id, i.index_id, ic.key_ordinal";

async fn tables(conn: &mut Conn) -> Result<Vec<Table>, String> {
    let mut out: Vec<Table> = Vec::new();
    for row in texts(conn, TABLES).await? {
        let v = |i: usize| row.get(i).cloned().flatten().unwrap_or_default();
        let table = v(0);
        if out.last().is_none_or(|t| t.name != table) {
            out.push(Table { name: table.clone(), columns: Vec::new(), keys: Vec::new() });
        }
        if v(7) == "1" {
            continue;
        }
        out.last_mut().expect("pushed above").columns.push(Column { name: v(1), listed: Vec::new(), ty: v(2), max_length: v(3).parse().unwrap_or(0), precision: v(4).parse().unwrap_or(0), scale: v(5).parse().unwrap_or(0), nullable: v(6) == "1" });
    }
    let mut keys: Vec<(String, String, Vec<String>)> = Vec::new();
    for row in texts(conn, KEYS).await? {
        let v = |i: usize| row.get(i).cloned().flatten().unwrap_or_default();
        if keys.last().is_none_or(|(t, i, _)| *t != v(0) || *i != v(1)) {
            keys.push((v(0), v(1), Vec::new()));
        }
        keys.last_mut().expect("pushed above").2.push(v(2));
    }
    // A CHECK's literals: each `[column]=<literal>` it compares with, as SQL Server writes an `in` list.
    let compared = regex::Regex::new(r"(\[[^\]]+\])\s*=\s*(N?'(?:[^']|'')*'|\(?-?[0-9.]+\)?)").expect("a valid pattern");
    for row in texts(conn, "select quotename(schema_name(t.schema_id)) + '.' + quotename(t.name) as tbl, c.definition from sys.check_constraints c join sys.tables t on t.object_id = c.parent_object_id").await? {
        let (Some(Some(table)), Some(Some(definition))) = (row.first(), row.get(1)) else { continue };
        let Some(t) = out.iter_mut().find(|t| t.name == *table) else { continue };
        for m in compared.captures_iter(definition) {
            if let Some(col) = t.columns.iter_mut().find(|c| c.name.eq_ignore_ascii_case(&m[1])) {
                let literal = m[2].trim_start_matches('(').trim_end_matches(')').to_string();
                if !col.listed.contains(&literal) {
                    col.listed.push(literal);
                }
            }
        }
    }
    for (table, _, columns) in keys {
        if let Some(t) = out.iter_mut().find(|t| t.name == table) {
            if let Some(at) = columns.iter().map(|c| t.columns.iter().position(|x| x.name == *c)).collect::<Option<Vec<usize>>>() {
                t.keys.push(at);
            }
        }
    }
    Ok(out)
}

/// The longest string of `n` characters a column of this length takes, or a long one where it has no length.
fn long(n: i64, unicode: bool) -> String {
    let len = if n < 0 { 9000 } else if unicode { (n / 2) as usize } else { n as usize };
    format!("{}'{}'", if unicode { "N" } else { "" }, "x".repeat(len.max(1)))
}

/// A column's or a parameter's values, as literals, by its type.
fn pool(ty: &str, max_length: i64, precision: u32, scale: u32, scenario: Scenario) -> Vec<String> {
    let edges = scenario == Scenario::Edges;
    let v = |all: &[&str], plain: &[&str]| (if edges { all } else { plain }).iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match ty {
        "bit" => v(&["0", "1"], &["0", "1"]),
        "tinyint" => v(&["0", "255"], &["0", "1"]),
        "smallint" => v(&["1", "0", "-32768", "32767"], &["1", "0", "-1"]),
        "int" => v(&["1", "0", "-2147483648", "2147483647"], &["1", "0", "-1"]),
        "bigint" => v(&["1", "0", "-9223372036854775808", "9223372036854775807"], &["1", "0", "-1"]),
        "decimal" | "numeric" => {
            let whole = precision.saturating_sub(scale) as usize;
            let largest = format!("{}{}{}", if whole == 0 { "0".to_string() } else { "9".repeat(whole) }, if scale > 0 { "." } else { "" }, "9".repeat(scale as usize));
            let smallest = if scale > 0 { format!("0.{}1", "0".repeat(scale as usize - 1)) } else { "1".into() };
            if edges {
                vec!["0".into(), largest.clone(), format!("-{largest}"), smallest]
            } else {
                vec!["0".into(), if whole > 0 { "1".into() } else { smallest }]
            }
        }
        "money" => v(&["0", "-922337203685477.5808", "922337203685477.5807", "0.0001"], &["0", "1.5"]),
        "smallmoney" => v(&["0", "-214748.3648", "214748.3647", "0.0001"], &["0", "1.5"]),
        "float" => v(&["0", "1.7976931348623157E+308", "-1.7976931348623157E+308", "2.2250738585072014E-308"], &["0", "1.5"]),
        "real" => v(&["0", "3.4E+38", "-3.4E+38"], &["0", "1.5"]),
        "char" | "varchar" | "text" => {
            let mut out = vec!["'a'".to_string(), "'A'".into(), "'a '".into(), "''".into()];
            if edges && ty != "text" {
                out.push(long(max_length, false));
            }
            out
        }
        "nchar" | "nvarchar" | "ntext" | "sysname" => {
            let mut out = vec!["N'a'".to_string(), "N'A'".into(), "N'a '".into(), "N''".into(), "N'é'".into()];
            if edges {
                out.push("N'😀'".into());
                if ty != "ntext" {
                    out.push(long(max_length, true));
                }
            }
            out
        }
        "binary" | "varbinary" | "image" => {
            let mut out = vec!["0x00".to_string(), "0xff".into(), "0x".into()];
            if edges && max_length < 0 {
                out.push(format!("0x{}", "ab".repeat(9000)));
            }
            out
        }
        "uniqueidentifier" => v(&["'00000000-0000-0000-0000-000000000000'", "'FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF'"], &["'6F9619FF-8B86-D011-B42D-00C04FC964FF'"]),
        "date" => v(&["'2026-01-02'", "'0001-01-01'", "'9999-12-31'"], &["'2026-01-02'"]),
        "datetime" => v(&["'2026-01-02T03:04:05.123'", "'1753-01-01T00:00:00'", "'9999-12-31T23:59:59.997'"], &["'2026-01-02T03:04:05.123'"]),
        "smalldatetime" => v(&["'2026-01-02T03:04:00'", "'1900-01-01T00:00:00'", "'2079-06-06T23:59:00'"], &["'2026-01-02T03:04:00'"]),
        "datetime2" => v(&["'2026-01-02T03:04:05.1234567'", "'0001-01-01T00:00:00'", "'9999-12-31T23:59:59.9999999'"], &["'2026-01-02T03:04:05.1234567'"]),
        "time" => v(&["'12:34:56.7891234'", "'00:00:00'", "'23:59:59.9999999'"], &["'12:34:56.7891234'"]),
        "datetimeoffset" => v(&["'2026-01-02T03:04:05.1234567+05:30'", "'0001-01-01T14:00:00+14:00'", "'9999-12-31T09:59:59.9999999-14:00'"], &["'2026-01-02T03:04:05.123+05:30'"]),
        "xml" => v(&["'<a/>'", "''"], &["'<a/>'"]),
        "json" => v(&["'{}'", "'[1, null]'", "'null'"], &["'{}'"]),
        "sql_variant" => v(&["1", "N'a'"], &["1"]),
        _ => vec![],
    }
}

/// A value for a key column that no other row holds: the `n`th.
fn fresh_value(col: &Column, n: usize) -> Option<String> {
    Some(match col.ty.as_str() {
        "tinyint" => ((n % 250) + 2).to_string(),
        "smallint" => (20000 + n).to_string(),
        "int" | "bigint" | "decimal" | "numeric" | "money" | "smallmoney" | "float" | "real" => (700_000 + n).to_string(),
        "char" | "varchar" | "nchar" | "nvarchar" => format!("'k{n}'"),
        "binary" | "varbinary" => format!("0x{n:04x}"),
        "uniqueidentifier" => format!("'00000000-0000-0000-0000-{n:012}'"),
        "date" => format!("dateadd(day, {n}, '2000-01-01')"),
        "datetime" | "datetime2" | "smalldatetime" | "datetimeoffset" => format!("dateadd(minute, {n}, '2000-01-01')"),
        _ => return None,
    })
}

/// The rows of each table, first value of every pool, then one row per pool value, then NULLs.
fn rows_of(table: &Table, scenario: Scenario, bare: &mut Vec<String>) -> Vec<Vec<String>> {
    let pools: Vec<Vec<String>> = table
        .columns
        .iter()
        .map(|c| {
            let mut p = c.listed.clone();
            p.extend(pool(&c.ty, c.max_length, c.precision, c.scale, scenario));
            if c.nullable {
                p.push("null".into());
            }
            if p.is_empty() || p == ["null"] {
                bare.push(format!("{}.{}", table.name, c.name));
            }
            p
        })
        .collect();
    if pools.iter().any(|p| p.is_empty()) {
        return Vec::new();
    }
    let base: Vec<String> = pools.iter().map(|p| p[0].clone()).collect();
    let mut out = vec![base.clone()];
    for (i, p) in pools.iter().enumerate() {
        for value in p.iter().skip(1) {
            let mut row = base.clone();
            row[i] = value.clone();
            out.push(row);
        }
    }
    out.push(table.columns.iter().zip(&base).map(|(c, b)| if c.nullable { "null".to_string() } else { b.clone() }).collect());
    out
}

/// The insert of a row, by its columns' values.
fn insert_sql(table: &Table, row: &[String]) -> String {
    if table.columns.is_empty() {
        return format!("insert into {} default values", table.name);
    }
    let names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
    format!("insert into {} ({}) values ({})", table.name, names.join(", "), row.join(", "))
}

/// The session's transaction: how many are open, and whether it is doomed.
async fn transaction_state(conn: &mut Conn) -> Result<(i64, i64), String> {
    let rows = texts(conn, "select @@trancount, xact_state()").await?;
    let row = rows.first().ok_or("no transaction state")?;
    let n = |i: usize| row.get(i).cloned().flatten().and_then(|v| v.parse().ok()).unwrap_or(0);
    Ok((n(0), n(1)))
}

/// Seeds every table in a transaction left open, as `scenario` says; the rows each took.
async fn seed(conn: &mut Conn, tables: &[Table], scenario: Scenario, refused: &mut HashSet<String>, bare: &mut Vec<String>) -> Result<String, String> {
    exec(conn, "begin transaction").await?;
    let mut pending: Vec<(usize, Vec<String>)> = tables.iter().enumerate().flat_map(|(t, table)| rows_of(table, scenario, bare).into_iter().map(move |r| (t, r))).collect();
    let mut counts = vec![0usize; tables.len()];
    let mut fresh = 0usize;
    // Passes, so a row that references another table goes in once the referenced row is there.
    loop {
        let mut progress = false;
        let mut left = Vec::new();
        for (t, mut row) in std::mem::take(&mut pending) {
            let table = &tables[t];
            let mut tries = 0;
            loop {
                let sql = insert_sql(table, &row);
                if refused.contains(&sql) {
                    break;
                }
                exec(conn, "save transaction s").await?;
                let outcome = match conn.simple_query(&sql).await {
                    Ok(stream) => stream.into_results().await.map(|_| ()).map_err(|e| code(&e)),
                    Err(e) => Err(code(&e)),
                };
                let Err(failed) = outcome else {
                    counts[t] += 1;
                    progress = true;
                    break;
                };
                let (n, state) = transaction_state(conn).await?;
                if n == 0 || state == -1 {
                    refused.insert(sql);
                    return Err("a seeding insert ended the transaction".into());
                }
                exec(conn, "rollback transaction s").await?;
                // A row that repeats a unique key gets fresh values in the key's columns.
                if matches!(failed, 2627 | 2601) && tries < 3 {
                    tries += 1;
                    for key in &table.keys {
                        for &c in key {
                            if let Some(v) = fresh_value(&table.columns[c], fresh) {
                                row[c] = v;
                                fresh += 1;
                            }
                        }
                    }
                    continue;
                }
                left.push((t, row));
                break;
            }
        }
        pending = left;
        if !progress || pending.is_empty() {
            break;
        }
    }
    Ok(tables.iter().zip(&counts).map(|(t, n)| format!("{} {n}{}", t.name, if *n == 0 { EMPTY } else { "" })).collect::<Vec<_>>().join(", "))
}

/// A session with the state `scenario` seeded, in a transaction left open; refused inserts are
/// kept so that a state built again skips them.
async fn seeded_session(url: &str, tables: &[Table], scenario: Scenario, refused: &mut HashSet<String>, bare: &mut Vec<String>) -> Result<(Conn, String), String> {
    for _ in 0..50 {
        let mut conn = session(url).await?;
        match seed(&mut conn, tables, scenario, refused, bare).await {
            Ok(counts) => return Ok((conn, counts)),
            Err(e) if e == "a seeding insert ended the transaction" => continue,
            Err(e) => return Err(e),
        }
    }
    Err("seeding never got through".into())
}

// ---------------------------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------------------------

/// A parameter's declaration, as the run time declares it from its type.
fn declaration(ty: &Type) -> String {
    let label = ty.label.as_deref().unwrap_or("sql_variant");
    let size = match &ty.base().kind {
        Kind::Text(t) => t.size.as_ref().and_then(|s| s.exact.or(s.max)),
        Kind::Bytes(b) => b.size.as_ref().and_then(|s| s.exact.or(s.max)),
        _ => None,
    };
    let length = || size.map_or("max".to_string(), |n| n.to_string());
    let scale = |tick: &str| tick_digits(tick).unwrap_or(7);
    match (&ty.base().kind, label) {
        (Kind::Decimal(d), "decimal" | "numeric") => format!("{label}({}, {})", d.precision.unwrap_or(18), d.max_scale.unwrap_or(0)),
        (_, "char" | "varchar" | "nchar" | "nvarchar" | "binary" | "varbinary") => format!("{label}({})", length()),
        (Kind::Time(t), "time") => format!("time({})", scale(&t.tick)),
        (Kind::DateTime(t), "datetime2") => format!("datetime2({})", scale(&t.tick)),
        (Kind::ZonedDateTime(t), "datetimeoffset") => format!("datetimeoffset({})", scale(&t.tick)),
        (_, "sysname") => "nvarchar(128)".into(),
        (_, "bit" | "tinyint" | "smallint" | "int" | "bigint" | "money" | "smallmoney" | "float" | "real" | "uniqueidentifier" | "date" | "datetime" | "smalldatetime" | "xml" | "text" | "ntext" | "image") => label.to_string(),
        _ => "sql_variant".into(),
    }
}

/// A value no seeded row holds, for the `n`th parameter of a type that keys a row, which an insert
/// can then take: a number, a string or a UUID.
fn fresh_parameter(label: &str, n: usize) -> Option<String> {
    Some(match label {
        "tinyint" => (200 + n % 50).to_string(),
        "smallint" => (30_000 + n).to_string(),
        "int" | "bigint" | "decimal" | "numeric" => (900_000 + n).to_string(),
        "char" | "varchar" => format!("'p{n}'"),
        "nchar" | "nvarchar" => format!("N'p{n}'"),
        "uniqueidentifier" => format!("'00000000-0000-0000-0001-{n:012}'"),
        _ => return None,
    })
}

/// A parameter's values: its type's pool, and NULL where it is nullable.
fn parameter_pool(param: &Value, scenario: Scenario) -> Vec<String> {
    let Ok(ty) = serde_json::from_value::<Type>(param["type"].clone()) else { return vec![] };
    let label = ty.label.clone().unwrap_or_default();
    let (precision, scale, length) = match &ty.base().kind {
        Kind::Decimal(d) => (d.precision.unwrap_or(18), d.max_scale.unwrap_or(0).max(0) as u32, 0),
        Kind::Text(t) => (0, 0, t.size.as_ref().and_then(|s| s.exact.or(s.max)).map_or(-1, |n| if label.starts_with('n') { n as i64 * 2 } else { n as i64 })),
        Kind::Bytes(b) => (0, 0, b.size.as_ref().and_then(|s| s.exact.or(s.max)).map_or(-1, |n| n as i64)),
        _ => (0, 0, 0),
    };
    let mut out = pool(&label, length, precision, scale, scenario);
    if param["nullable"].as_bool() == Some(true) || out.is_empty() {
        out.push("null".into());
    }
    out
}

/// The text a run sends: `sp_executesql` with the statement and its parameters' declarations and values.
fn run_text(rewritten: &str, decls: &[String], values: &[String]) -> String {
    let statement = format!("N'{}'", rewritten.replace('\'', "''"));
    if decls.is_empty() {
        return format!("exec sp_executesql {statement}");
    }
    let assigned: Vec<String> = values.iter().enumerate().map(|(i, v)| format!("@p_{} = {v}", i + 1)).collect();
    format!("exec sp_executesql {statement}, N'{}', {}", decls.join(", ").replace('\'', "''"), assigned.join(", "))
}

enum Outcome {
    /// The first result set's rows, and how many result sets the run returned.
    Rows(Vec<Vec<ColumnData<'static>>>, usize),
    Failed(String, u32),
    TimedOut,
}

async fn run_one(conn: &mut Conn, sql: &str, timeout_ms: u64) -> Outcome {
    let work = async {
        let stream = conn.simple_query(sql).await?;
        stream.into_results().await
    };
    match tokio::time::timeout(Duration::from_millis(timeout_ms), work).await {
        Err(_) => Outcome::TimedOut,
        Ok(Err(e)) => Outcome::Failed(message(&e), code(&e)),
        Ok(Ok(results)) => {
            let sets = results.len();
            Outcome::Rows(results.into_iter().next().unwrap_or_default().into_iter().map(|row| row.into_iter().collect()).collect(), sets)
        }
    }
}

/// Whether the server refused a NULL bound to a parameter: a NOT NULL or a CHECK, or a NULL a function or a key refuses.
fn refuses_null(code: u32) -> bool {
    matches!(code, 515 | 547)
}

pub async fn check(url: &str, statements: &[(String, String, Value)], timeout_ms: u64) -> Result<Report, String> {
    let mut problems = Vec::new();
    let mut notes = Vec::new();
    let mut tallies: Vec<Tally> = statements.iter().map(|_| Tally::default()).collect();
    let tables = {
        let mut conn = session(url).await?;
        tables(&mut conn).await?
    };
    // Each statement's text with `@p_n` placeholders, its parameters' declarations, and whether it
    // runs at all (a statement the engine refused is run for nothing).
    let mut prepared: Vec<Option<(String, Vec<String>)>> = Vec::new();
    for ((_, _, analysis), tally) in statements.iter().zip(&mut tallies) {
        if analysis["diagnostics"].as_array().is_some_and(|d| !d.is_empty()) {
            tally.refused = Some("the engine refused it".into());
            prepared.push(None);
            continue;
        }
        // The statement as the run time sends it: `@p_n` placeholders, and each `::type` written as
        // SQL Server's own cast.
        let Some(rewritten) = analysis["rewritten"]["sql"].as_str() else {
            tally.refused = Some("the analysis holds no compiled statement".into());
            prepared.push(None);
            continue;
        };
        let params = analysis["params"].as_array().cloned().unwrap_or_default();
        let decls: Vec<String> = params.iter().enumerate().map(|(i, p)| format!("@p_{} {}", i + 1, serde_json::from_value::<Type>(p["type"].clone()).map(|t| declaration(&t)).unwrap_or_else(|_| "sql_variant".into()))).collect();
        // A MERGE must end with its `;`, which the statement keeps as written.
        prepared.push(Some((rewritten.trim().to_string(), decls)));
    }
    let mut refused: HashSet<String> = HashSet::new();
    for scenario in SCENARIOS {
        let mut bare = Vec::new();
        let (mut conn, counts) = seeded_session(url, &tables, scenario, &mut refused, &mut bare).await?;
        notes.push(format!("seeded with {}: {counts}", scenario.name()));
        if !bare.is_empty() && scenario == Scenario::Edges {
            bare.sort();
            bare.dedup();
            notes.push(format!("no seed value for {}: the column holds NULL alone", bare.join(", ")));
        }
        let mut broken = false;
        for (k, ((file, _, analysis), ready)) in statements.iter().zip(&prepared).enumerate() {
            let Some((rewritten, decls)) = ready else { continue };
            let params = analysis["params"].as_array().cloned().unwrap_or_default();
            let pools: Vec<Vec<String>> = params.iter().map(|p| parameter_pool(p, scenario)).collect();
            let columns = analysis["columns"].as_array().cloned().unwrap_or_default();
            let types: Vec<Option<Type>> = columns.iter().map(|c| serde_json::from_value(c["type"].clone()).ok()).collect();
            let cardinality = analysis["cardinality"].as_str().unwrap_or("many");
            // Each parameter starts its pool at another value, so that two of one type differ.
            let pools: Vec<Vec<String>> = pools
                .into_iter()
                .enumerate()
                .map(|(i, mut p)| {
                    if !p.is_empty() {
                        let at = i % p.len();
                        p.rotate_left(at);
                    }
                    // First a value no seeded row holds, so that an insert's key is free.
                    let label = serde_json::from_value::<Type>(params[i]["type"].clone()).ok().and_then(|t| t.label);
                    if let Some(fresh) = label.as_deref().and_then(|l| fresh_parameter(l, i)) {
                        p.insert(0, fresh);
                    }
                    p
                })
                .collect();
            for values in assignments(&pools) {
                if broken {
                    let mut ignore = Vec::new();
                    (conn, _) = seeded_session(url, &tables, scenario, &mut refused, &mut ignore).await?;
                    broken = false;
                }
                let shown = values.join(", ");
                exec(&mut conn, "save transaction q").await?;
                let outcome = run_one(&mut conn, &run_text(rewritten, decls, &values), timeout_ms).await;
                match outcome {
                    Outcome::TimedOut => {
                        tallies[k].timed_out += 1;
                        broken = true;
                        continue;
                    }
                    Outcome::Failed(why, code) => {
                        if refuses_null(code) {
                            for (i, v) in values.iter().enumerate() {
                                if v == "null" && params[i]["nullable"].as_bool() == Some(true) && code == 515 {
                                    problems.push(format!("{file}: parameter {} typed nullable, and the database refuses its NULL: {why} with ({shown})", params[i]["name"].as_str().unwrap_or("?")));
                                }
                            }
                        }
                        tallies[k].fail(why);
                    }
                    Outcome::Rows(rows, sets) => {
                        // A batch's analysis types its first result set, and refuses one that may return more.
                        if sets > 1 {
                            problems.push(format!("{file}: typed as one result set, returned {sets} with ({shown})"));
                        }
                        tallies[k].ran += 1;
                        if (cardinality == "at_most_one" && rows.len() > 1) || (cardinality == "one" && rows.len() != 1) || (cardinality == "none" && !rows.is_empty()) {
                            problems.push(format!("{file}: proven {cardinality}, returned {} rows with ({shown})", rows.len()));
                        }
                        let as_text: Vec<Vec<Option<String>>> = rows.iter().map(|r| r.iter().map(text).collect()).collect();
                        let borrowed = || as_text.iter().map(|r| r.iter().map(|v| v.as_deref()).collect::<Vec<Option<&str>>>());
                        if let Some(why) = key_repeats(analysis, &columns, borrowed()) {
                            problems.push(format!("{file}: {why} with ({shown})"));
                        }
                        if analysis["ordered"].as_bool() == Some(true) || analysis["order"].as_array().is_some_and(|o| !o.is_empty()) {
                            if let Some(why) = out_of_order(analysis, &columns, borrowed()) {
                                problems.push(format!("{file}: {why} with ({shown})"));
                            }
                        }
                        for (row, texts) in rows.iter().zip(&as_text) {
                            if row.len() != columns.len() {
                                problems.push(format!("{file}: typed {} columns, returned {} with ({shown})", columns.len(), row.len()));
                                break;
                            }
                            for (i, (value, text)) in row.iter().zip(texts).enumerate() {
                                let name = columns[i]["name"].as_str().unwrap_or("?");
                                let nullable = columns[i]["nullable"].as_bool() == Some(true);
                                let Some(text) = text else {
                                    if !nullable {
                                        problems.push(format!("{file}: column {name} typed non-null is NULL with ({shown})"));
                                    }
                                    continue;
                                };
                                let Some(ty) = &types[i] else { continue };
                                let money = matches!(ty.label.as_deref(), Some("money" | "smallmoney"));
                                let why = variant_contradiction(ty, value)
                                    .or_else(|| sized_contradiction(ty, value, text))
                                    .or_else(|| if money || matches!(value, ColumnData::F32(_) | ColumnData::F64(_)) { None } else { contradiction(&columns[i]["type"], text, nullable, ',') });
                                if let Some(why) = why {
                                    problems.push(format!("{file}: column {name} {why} with ({shown})"));
                                }
                            }
                        }
                    }
                }
                let (n, state) = transaction_state(&mut conn).await?;
                if n == 0 || state == -1 {
                    if n > 0 {
                        exec(&mut conn, "rollback transaction").await?;
                    }
                    broken = true;
                } else {
                    exec(&mut conn, "rollback transaction q").await?;
                }
            }
        }
        if !broken {
            let _ = exec(&mut conn, "if @@trancount > 0 rollback transaction").await;
        }
    }
    Ok(report(statements, &tallies, timeout_ms, problems, notes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_values_as_text() {
        assert_eq!(civil(0), "0001-01-01");
        assert_eq!(civil(3_652_058), "9999-12-31");
        assert_eq!(civil(739_251), "2025-01-01");
        assert_eq!(clock_text(45_296_123_456_7, 7), "12:34:56.1234567");
        assert_eq!(clock_text(45_296_000_000_0, 7), "12:34:56");
        assert_eq!(numeric_text(tiberius::numeric::Numeric::new_with_scale(-150, 2)), "-1.50");
        assert_eq!(numeric_text(tiberius::numeric::Numeric::new_with_scale(5, 3)), "0.005");
        assert_eq!(numeric_text(tiberius::numeric::Numeric::new_with_scale(42, 0)), "42");
        assert_eq!(with_database("sqlserver://sa:x@h:14333/master?encrypt=true", "app"), "sqlserver://sa:x@h:14333/app?encrypt=true");
        assert_eq!(port_of("sqlserver://sa:x@h/app"), Some(1433));
        assert_eq!(batches("create table t (a int)\nGO\ncreate view v as select a from t\ngo\n"), ["create table t (a int)\n", "create view v as select a from t\n"]);
    }

    #[test]
    fn declares_parameters_as_the_run_time_does() {
        let ty = |v: Value| serde_json::from_value::<Type>(v).unwrap();
        assert_eq!(declaration(&ty(serde_json::json!({"kind": "decimal", "precision": 10, "min_scale": 2, "max_scale": 2, "label": "decimal"}))), "decimal(10, 2)");
        assert_eq!(declaration(&ty(serde_json::json!({"kind": "text", "encoding": "UTF-16", "ill_formed": true, "size": {"unit": "code_unit", "max": 10}, "label": "nvarchar"}))), "nvarchar(10)");
        assert_eq!(declaration(&ty(serde_json::json!({"kind": "text", "label": "varchar"}))), "varchar(max)");
        assert_eq!(declaration(&ty(serde_json::json!({"kind": "datetime", "tick": "1/1000", "label": "datetime2"}))), "datetime2(3)");
        assert_eq!(run_text("select @p_1 as a", &["@p_1 int".into()], &["1".into()]), "exec sp_executesql N'select @p_1 as a', N'@p_1 int', @p_1 = 1");
    }
}
