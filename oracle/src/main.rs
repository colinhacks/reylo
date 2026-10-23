//! The engine against live databases.
//!
//! A case is a directory: `setup.sql` builds a schema, every other `.sql` file is one statement,
//! and `expected.txt` holds one line per statement in the form this tool prints. A directory with
//! no `expected.txt`, such as the parity check's statements, is checked for soundness alone. A Postgres case
//! runs in a database of its own, created on the server `--url` names and dropped afterwards, so
//! the server must be a throwaway one: cases create collations, operators and roles. A case whose
//! `dialect` file says `sqlite` runs in a fresh SQLite file instead. A case whose `dialect` file
//! says `mysql` runs in a database of its own on the MySQL server `--mysql-url` (or
//! `ORACLE_MYSQL_URL`) names, also a throwaway one, which it refuses on port 3306; how its host
//! answers the engine and how its runs are isolated is in `mysql.rs`. A case with a `min_version`
//! file (a `server_version_num`, such as `180000`, or for MySQL the version as one number, such
//! as `80400`) is skipped on an older server. A MySQL case's `sql_mode` file sets its sessions'
//! `sql_mode`.
//!
//!   cargo run -p reylo-oracle -- --url postgres://postgres@localhost:55432/postgres oracle/cases
//!   cargo run -p reylo-oracle -- --mysql-url mysql://root@127.0.0.1:33411/mysql oracle/cases/mysql_types
//!   cargo run -p reylo-oracle -- --url … --bless oracle/cases/not_null   # rewrite expected.txt
//!   cargo run -p reylo-oracle -- --url … --analyses old.json oracle/cases  # check other answers
//!   cargo run -p reylo-oracle -- --url … --dump new.json oracle/cases      # write the answers
//!   cargo run -p reylo-oracle -- --url … --strict-coverage oracle/cases    # fail on a statement that never ran
//!   cargo run -p reylo-oracle -- --url … --timeout 2000 oracle/cases       # milliseconds per run
//!
//! `--analyses` names a JSON object of analyses keyed `<case>/<file>`, which the seeding checks
//! in place of the engine's own: how a check that passes shows it can fail, run on the answers of
//! an engine that had a defect. `--dump` writes that object, for the cases of every dialect.
//!
//! Every run of a statement is bounded: 5000 milliseconds, or `--timeout`, or the number in the
//! case's `timeout` file, which wins. A run the timeout stopped is evidence of nothing.
//!
//! A statement that failed or timed out under every assignment of its parameters, over every
//! seeded state, was checked for nothing. Each case prints how many of its statements never ran,
//! and each one's first error, under `UNCHECKED`; the run's last lines count them. A case's
//! `unchecked.txt` holds those lines: the statements a case means not to run (the timeout, a type
//! no value is seeded for), or no line at all where a case is about statements that must run. It
//! is compared like `expected.txt`, a difference fails the case, and `--bless` rewrites it where
//! it exists or a statement never ran. `--strict-coverage` also fails every case with a statement
//! that never ran and no `unchecked.txt`, which is how a set of statements from elsewhere shows
//! what the oracle did not check.
//!
//! Each statement goes through the engine's replay exactly as a host runs it: the describe comes
//! from the extended protocol (on MySQL a server-side prepare) and every catalog query runs on
//! the case's database. A catalog query that fails is a failure of the case, never an empty answer.

mod mysql;
mod seed;

use reylo_engine_core::api::DbError;
use reylo_engine_core::database::Database;
use reylo_engine_core::replay::{self, AnswerCache, Answerer, AsyncAnswerer};
use serde_json::{json, Map, Value};
use std::future::Future;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use tokio_postgres::types::{ToSql, Type};
use tokio_postgres::{Client, Config, NoTls};

#[tokio::main]
async fn main() -> ExitCode {
    let mut url = std::env::var("ORACLE_URL").ok();
    let mut mysql_url = std::env::var("ORACLE_MYSQL_URL").ok();
    let mut bless = false;
    let mut keep = false;
    let mut strict_coverage = false;
    let mut timeout_ms: u64 = 5000;
    let mut given = Map::new();
    let mut dump: Option<PathBuf> = None;
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--url" => url = args.next(),
            "--mysql-url" => mysql_url = args.next(),
            "--bless" => bless = true,
            "--keep" => keep = true,
            "--strict-coverage" => strict_coverage = true,
            "--timeout" => match args.next().and_then(|ms| ms.parse::<u64>().ok()).filter(|ms| *ms > 0) {
                Some(ms) => timeout_ms = ms,
                None => {
                    eprintln!("--timeout takes a number of milliseconds");
                    return ExitCode::from(2);
                }
            },
            "--dump" => dump = args.next().map(PathBuf::from),
            "--analyses" => {
                let path = args.next().unwrap_or_default();
                match fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok()) {
                    Some(Value::Object(map)) => given = map,
                    _ => {
                        eprintln!("--analyses {path}: not a JSON object of analyses");
                        return ExitCode::from(2);
                    }
                }
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    let cases = cases(&paths);
    if cases.is_empty() {
        eprintln!("no cases: name a case directory, or a directory of them");
        return ExitCode::from(2);
    }
    let mut failed = 0;
    let mut never_ran = 0;
    let mut unnamed = 0;
    let mut dumped = Map::new();
    let mut admin: Option<(Client, Config, i32)> = None;
    let mut mysql_server: Option<mysql::Server> = None;
    for case in &cases {
        let dialect = fs::read_to_string(case.join("dialect")).map(|s| s.trim().to_string()).unwrap_or_else(|_| "postgres".into());
        let timeout_ms = fs::read_to_string(case.join("timeout")).ok().and_then(|s| s.trim().parse::<u64>().ok()).filter(|ms| *ms > 0).unwrap_or(timeout_ms);
        let min = fs::read_to_string(case.join("min_version")).ok().and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(0);
        let outcome = if dialect == "sqlite" {
            run_sqlite(case, timeout_ms, &given, &mut dumped)
        } else if dialect == "mysql" {
            if mysql_server.is_none() {
                let Some(url) = mysql_url.as_deref() else {
                    eprintln!("a MySQL case needs --mysql-url (or ORACLE_MYSQL_URL): a throwaway server, never a shared one");
                    return ExitCode::from(2);
                };
                match mysql::connect(url).await {
                    Ok(server) => mysql_server = Some(server),
                    Err(e) => {
                        eprintln!("cannot connect to {url}: {e}");
                        return ExitCode::from(2);
                    }
                }
            }
            let server = mysql_server.as_mut().expect("connected above");
            if (server.version as i32) < min {
                println!("{}: skipped (server {} < {min})", case.display(), server.version);
                continue;
            }
            mysql::run(case, server, keep, timeout_ms, &given, &mut dumped, statements(case)).await
        } else {
            if admin.is_none() {
                let Some(url) = url.as_deref() else {
                    eprintln!("a Postgres case needs --url (or ORACLE_URL): a throwaway server, never a shared one");
                    return ExitCode::from(2);
                };
                match connect_admin(url).await {
                    Ok(a) => admin = Some(a),
                    Err(e) => {
                        eprintln!("cannot connect to {url}: {e}");
                        return ExitCode::from(2);
                    }
                }
            }
            let (client, config, version) = admin.as_ref().expect("connected above");
            if *version < min {
                println!("{}: skipped (server {version} < {min})", case.display());
                continue;
            }
            run_postgres(case, client, config, keep, timeout_ms, &given, &mut dumped).await
        };
        match outcome {
            Ok((lines, unchecked)) => {
                never_ran += unchecked.len();
                if !bless && !case.join("unchecked.txt").is_file() {
                    unnamed += unchecked.len();
                }
                print_unchecked(case, lines.len(), &unchecked);
                // Both verdicts are printed, whichever fails the case.
                let matched = check(case, &lines, bless);
                if !coverage(case, &unchecked, bless, strict_coverage) || !matched {
                    failed += 1;
                }
            }
            Err(e) => {
                println!("{}: FAILED: {e}", case.display());
                failed += 1;
            }
        }
    }
    if let Some(path) = dump {
        fs::write(&path, serde_json::to_string_pretty(&Value::Object(dumped)).unwrap_or_default()).expect("the --dump file is writable");
    }
    if never_ran > 0 {
        println!("{never_ran} statement(s) never ran and were checked for nothing, {unnamed} of them in a case with no unchecked.txt{}", if strict_coverage && unnamed > 0 { ", which --strict-coverage fails" } else { "" });
    }
    println!("{} case(s), {failed} failed", cases.len());
    if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// The case directories under `paths`: a directory with a `setup.sql` is a case, and any other
/// directory is searched one level down.
fn cases(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if path.join("setup.sql").is_file() {
            out.push(path.clone());
        } else if let Ok(entries) = fs::read_dir(path) {
            let mut dirs: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.join("setup.sql").is_file()).collect();
            dirs.sort();
            out.extend(dirs);
        }
    }
    out
}

/// The statements of a case, by file name.
fn statements(case: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = fs::read_dir(case)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "sql") && p.file_name().is_some_and(|n| n != "setup.sql"))
                .filter_map(|p| Some((p.file_name()?.to_string_lossy().to_string(), fs::read_to_string(&p).ok()?)))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Compare a case's lines with its `expected.txt`, or write them there under `--bless`.
fn check(case: &Path, lines: &[String], bless: bool) -> bool {
    let path = case.join("expected.txt");
    let actual = lines.join("\n") + "\n";
    if bless {
        fs::write(&path, &actual).expect("expected.txt is writable");
        println!("{}: blessed {} statement(s)", case.display(), lines.len());
        return true;
    }
    let Ok(expected) = fs::read_to_string(&path) else {
        println!("{}: sound ({} statement(s), no expected.txt)", case.display(), lines.len());
        return true;
    };
    if expected == actual {
        println!("{}: ok ({} statement(s))", case.display(), lines.len());
        return true;
    }
    println!("{}: MISMATCH", case.display());
    let want: Vec<&str> = expected.lines().collect();
    for line in lines {
        if !want.contains(&line.as_str()) {
            println!("  + {line}");
        }
    }
    for line in &want {
        if !lines.iter().any(|l| l == line) {
            println!("  - {line}");
        }
    }
    false
}

/// Compare the statements of a case that never ran with its `unchecked.txt`, or write them there
/// under `--bless`. Without that file a statement that never ran fails the case under
/// `--strict-coverage` alone.
fn coverage(case: &Path, unchecked: &[String], bless: bool, strict: bool) -> bool {
    let path = case.join("unchecked.txt");
    if bless {
        // An empty unchecked.txt says that every statement of the case ran, and stays.
        if !unchecked.is_empty() || path.is_file() {
            fs::write(&path, unchecked.iter().map(|line| format!("{line}\n")).collect::<String>()).expect("unchecked.txt is writable");
        }
        return true;
    }
    let Ok(expected) = fs::read_to_string(&path) else {
        if strict && !unchecked.is_empty() {
            println!("{}: FAILED: --strict-coverage, and no unchecked.txt names the {} statement(s) that never ran", case.display(), unchecked.len());
            return false;
        }
        return true;
    };
    let want: Vec<&str> = expected.lines().collect();
    if want == unchecked {
        return true;
    }
    println!("{}: MISMATCH (unchecked.txt)", case.display());
    for line in unchecked {
        if !want.contains(&line.as_str()) {
            println!("  + {line}");
        }
    }
    for line in &want {
        if !unchecked.iter().any(|l| l == line) {
            println!("  - {line}");
        }
    }
    false
}

/// The statements of a case that never ran, each with its first error.
fn print_unchecked(case: &Path, statements: usize, unchecked: &[String]) {
    if !unchecked.is_empty() {
        println!("{}: UNCHECKED: {} of {statements} statement(s) never ran, so nothing about them was checked:", case.display(), unchecked.len());
        for line in unchecked {
            println!("    {line}");
        }
    }
}

/// What a case's seeding found: its notes are printed, a contradiction fails the case, and the
/// statements that never ran come back as the lines `unchecked.txt` holds.
fn seeded(case: &Path, statements: usize, report: seed::Report) -> Result<Vec<String>, String> {
    for note in &report.notes {
        println!("{}: note: {note}", case.display());
    }
    let unchecked: Vec<String> = report.unchecked.iter().map(|(file, why)| format!("{file}: {why}")).collect();
    if !report.problems.is_empty() {
        // The case fails here, and what never ran is said all the same.
        print_unchecked(case, statements, &unchecked);
        return Err(format!("{} value(s) contradict the types:\n    {}", report.problems.len(), report.problems.join("\n    ")));
    }
    Ok(unchecked)
}

// ---------------------------------------------------------------------------------------------
// Postgres
// ---------------------------------------------------------------------------------------------

async fn connect(config: &Config) -> Result<Client, String> {
    let (client, connection) = config.connect(NoTls).await.map_err(|e| e.to_string())?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(client)
}

async fn connect_admin(url: &str) -> Result<(Client, Config, i32), String> {
    let config = Config::from_str(url).map_err(|e| e.to_string())?;
    if config.get_ports().contains(&5432) {
        return Err("port 5432 is the shared server; the oracle runs only on a throwaway cluster".into());
    }
    let client = connect(&config).await?;
    let version: String = client.query_one("SHOW server_version_num", &[]).await.map_err(|e| e.to_string())?.get(0);
    Ok((client, config, version.parse().unwrap_or(0)))
}

async fn run_postgres(case: &Path, admin: &Client, admin_config: &Config, keep: bool, timeout_ms: u64, given: &Map<String, Value>, dumped: &mut Map<String, Value>) -> Result<(Vec<String>, Vec<String>), String> {
    let name: String = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let db = format!("reylo_oracle_{}", name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect::<String>());
    // Two messages: in one, CREATE DATABASE would run inside the batch's implicit transaction.
    admin.batch_execute(&format!("DROP DATABASE IF EXISTS {db} WITH (FORCE)")).await.map_err(|e| format!("drop {db}: {}", pg_message(&e)))?;
    admin.batch_execute(&format!("CREATE DATABASE {db}")).await.map_err(|e| format!("create {db}: {}", pg_message(&e)))?;
    let mut config = admin_config.clone();
    config.dbname(&db);
    let client = connect(&config).await?;
    let setup = fs::read_to_string(case.join("setup.sql")).map_err(|e| e.to_string())?;
    client.batch_execute(&setup).await.map_err(|e| format!("setup.sql: {}", pg_message(&e)))?;
    let mut lines = Vec::new();
    let mut analyzed = Vec::new();
    for (file, sql) in statements(case) {
        let analysis = match given.get(&format!("{name}/{file}")) {
            Some(answer) => answer.clone(),
            None => analyze_postgres(&client, &sql).await.map_err(|e| format!("{file}: {e}"))?,
        };
        lines.push(format!("{file}: {}", line(&analysis)));
        dumped.insert(format!("{name}/{file}"), analysis.clone());
        analyzed.push((file, sql, analysis));
    }
    // A value that contradicts a type is a failure whatever expected.txt says.
    let report = seed::check(&client, &analyzed, timeout_ms).await;
    drop(client);
    if !keep {
        let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {db} WITH (FORCE)")).await;
    }
    let unchecked = seeded(case, lines.len(), report.map_err(|e| format!("seeding: {e}"))?)?;
    Ok((lines, unchecked))
}

/// The Postgres engine's describe payload, which the oracle's answers decode into.
type PgDescribe = <reylo_engine_postgres::Postgres as Database>::Describe;

async fn analyze_postgres(client: &Client, sql: &str) -> Result<Value, String> {
    let analysis = replay::run_async::<reylo_engine_postgres::Postgres, _>(sql, &mut PgAnswers(client), &mut AnswerCache::default()).await.map_err(|e| e.to_string())?;
    serde_json::to_value(&analysis).map_err(|e| e.to_string())
}

/// The engine's needs, answered over the oracle's `tokio-postgres` client: a describe with the
/// statement's parameter types and each column's type, table and column number, a catalog query
/// with its parameters bound by the types the server infers for them.
struct PgAnswers<'a>(&'a Client);

impl AsyncAnswerer<PgDescribe> for PgAnswers<'_> {
    type Error = String;

    fn describe(&mut self, sql: &str) -> impl Future<Output = Result<Result<PgDescribe, DbError>, String>> + Send {
        let client = self.0;
        async move {
            match client.prepare(sql).await {
                Ok(stmt) => {
                    let params: Vec<u32> = stmt.params().iter().map(Type::oid).collect();
                    let columns: Vec<Value> = stmt
                        .columns()
                        .iter()
                        .map(|c| json!({ "name": c.name(), "oid": c.type_().oid(), "table": c.table_oid().unwrap_or(0), "attnum": c.column_id().map_or(0, i32::from) }))
                        .collect();
                    serde_json::from_value(json!({ "params": params, "columns": columns })).map(Ok).map_err(|e| e.to_string())
                }
                Err(e) => match pg_error(&e) {
                    Some(refusal) => Ok(Err(refusal)),
                    None => Err(format!("the describe did not reach the server: {e}")),
                },
            }
        }
    }

    fn query(&mut self, sql: &str, params: &[Value]) -> impl Future<Output = Result<Result<Vec<Vec<Value>>, DbError>, String>> + Send {
        let client = self.0;
        async move {
            let stmt = client.prepare(sql).await.map_err(|e| format!("catalog query failed to prepare: {}\n    {sql}", pg_message(&e)))?;
            let mut values: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();
            for (ty, value) in stmt.params().iter().zip(params) {
                values.push(bind(ty, value).ok_or_else(|| format!("cannot bind {value} as {ty} for the catalog query\n    {sql}"))?);
            }
            let refs: Vec<&(dyn ToSql + Sync)> = values.iter().map(|v| v.as_ref() as &(dyn ToSql + Sync)).collect();
            let rows = client.query(&stmt, &refs).await.map_err(|e| format!("catalog query failed: {}\n    {sql}", pg_message(&e)))?;
            Ok(Ok(rows.iter().map(|row| (0..row.len()).map(|i| cell(row, i)).collect()).collect()))
        }
    }
}

/// One parameter of a catalog query, by the type the server inferred from the cast in its text.
fn bind(ty: &Type, value: &Value) -> Option<Box<dyn ToSql + Sync + Send>> {
    let items = || value.as_array().cloned().unwrap_or_default();
    Some(match *ty {
        Type::OID_ARRAY => Box::new(items().iter().map(|v| v.as_u64().map(|n| n as u32)).collect::<Option<Vec<u32>>>()?),
        Type::INT2_ARRAY => Box::new(items().iter().map(|v| v.as_i64().map(|n| n as i16)).collect::<Option<Vec<i16>>>()?),
        Type::INT4_ARRAY => Box::new(items().iter().map(|v| v.as_i64().map(|n| n as i32)).collect::<Option<Vec<i32>>>()?),
        Type::INT8_ARRAY => Box::new(items().iter().map(|v| v.as_i64()).collect::<Option<Vec<i64>>>()?),
        Type::TEXT_ARRAY | Type::NAME_ARRAY | Type::VARCHAR_ARRAY => Box::new(items().iter().map(|v| v.as_str().map(String::from)).collect::<Option<Vec<String>>>()?),
        Type::TEXT | Type::VARCHAR | Type::NAME => Box::new(value.as_str().map(String::from).unwrap_or_else(|| value.to_string())),
        Type::OID => Box::new(value.as_u64()? as u32),
        Type::INT2 => Box::new(value.as_i64()? as i16),
        Type::INT4 => Box::new(value.as_i64()? as i32),
        Type::INT8 => Box::new(value.as_i64()?),
        Type::BOOL => Box::new(value.as_bool()?),
        _ => return None,
    })
}

/// One value of a catalog row as JSON, by its column's type.
fn cell(row: &tokio_postgres::Row, i: usize) -> Value {
    fn array<T: Into<Value>>(items: Option<Vec<Option<T>>>) -> Option<Value> {
        items.map(|items| Value::Array(items.into_iter().map(|v| v.map_or(Value::Null, Into::into)).collect()))
    }
    let value = match *row.columns()[i].type_() {
        Type::BOOL => row.get::<_, Option<bool>>(i).map(Value::from),
        Type::INT2 => row.get::<_, Option<i16>>(i).map(Value::from),
        Type::INT4 => row.get::<_, Option<i32>>(i).map(Value::from),
        Type::INT8 => row.get::<_, Option<i64>>(i).map(Value::from),
        Type::OID => row.get::<_, Option<u32>>(i).map(Value::from),
        Type::TEXT | Type::VARCHAR | Type::NAME | Type::BPCHAR | Type::CHAR => row.get::<_, Option<String>>(i).map(Value::from),
        Type::TEXT_ARRAY | Type::VARCHAR_ARRAY | Type::NAME_ARRAY => array(row.get::<_, Option<Vec<Option<String>>>>(i)),
        Type::INT2_ARRAY => array(row.get::<_, Option<Vec<Option<i16>>>>(i)),
        Type::INT4_ARRAY => array(row.get::<_, Option<Vec<Option<i32>>>>(i)),
        Type::INT8_ARRAY => array(row.get::<_, Option<Vec<Option<i64>>>>(i)),
        Type::OID_ARRAY => array(row.get::<_, Option<Vec<Option<u32>>>>(i)),
        Type::BOOL_ARRAY => array(row.get::<_, Option<Vec<Option<bool>>>>(i)),
        ref other => panic!("a catalog query answered a column of type {other}, which no host binds"),
    };
    value.unwrap_or(Value::Null)
}

fn pg_message(e: &tokio_postgres::Error) -> String {
    e.as_db_error().map(|d| format!("{} ({})", d.message(), d.code().code())).unwrap_or_else(|| e.to_string())
}

/// The server's refusal of what it was asked, or `None` for an error that never reached it, such as
/// a closed connection, which is the host's failure and ends the replay.
fn pg_error(e: &tokio_postgres::Error) -> Option<DbError> {
    let d = e.as_db_error()?;
    let position = match d.position() {
        Some(tokio_postgres::error::ErrorPosition::Original(p)) => Some(*p as usize),
        _ => None,
    };
    Some(DbError { message: d.message().to_string(), code: Some(d.code().code().to_string()), position })
}

// ---------------------------------------------------------------------------------------------
// SQLite
// ---------------------------------------------------------------------------------------------

fn run_sqlite(case: &Path, timeout_ms: u64, given: &Map<String, Value>, dumped: &mut Map<String, Value>) -> Result<(Vec<String>, Vec<String>), String> {
    let name: String = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let file = std::env::temp_dir().join(format!("reylo-oracle-{name}.db"));
    let _ = fs::remove_file(&file);
    let db = rusqlite::Connection::open(&file).map_err(|e| e.to_string())?;
    let setup = fs::read_to_string(case.join("setup.sql")).map_err(|e| e.to_string())?;
    db.execute_batch(&setup).map_err(|e| format!("setup.sql: {e}"))?;
    let mut lines = Vec::new();
    let mut analyzed = Vec::new();
    for (file_name, sql) in statements(case) {
        let analysis = match given.get(&format!("{name}/{file_name}")) {
            Some(answer) => answer.clone(),
            None => analyze_sqlite(&db, &sql).map_err(|e| format!("{file_name}: {e}"))?,
        };
        lines.push(format!("{file_name}: {}", line(&analysis)));
        dumped.insert(format!("{name}/{file_name}"), analysis.clone());
        analyzed.push((file_name, sql, analysis));
    }
    // A value that contradicts a type is a failure whatever expected.txt says.
    let report = seed::check_sqlite(&db, &analyzed, timeout_ms);
    drop(db);
    let _ = fs::remove_file(&file);
    let unchecked = seeded(case, lines.len(), report.map_err(|e| format!("seeding: {e}"))?)?;
    Ok((lines, unchecked))
}

fn analyze_sqlite(db: &rusqlite::Connection, sql: &str) -> Result<Value, String> {
    let analysis = replay::run::<reylo_engine_sqlite::Sqlite, _>(sql, &mut SqliteAnswers(db), &mut AnswerCache::default()).map_err(|e| e.to_string())?;
    serde_json::to_value(&analysis).map_err(|e| e.to_string())
}

/// The engine's catalog queries, answered from the case's SQLite file. SQLite prepares inside the
/// engine, so it never asks to describe.
struct SqliteAnswers<'a>(&'a rusqlite::Connection);

impl Answerer<<reylo_engine_sqlite::Sqlite as Database>::Describe> for SqliteAnswers<'_> {
    type Error = String;

    fn describe(&mut self, sql: &str) -> Result<Result<<reylo_engine_sqlite::Sqlite as Database>::Describe, DbError>, String> {
        Err(format!("the SQLite path asked to describe {sql}"))
    }

    fn query(&mut self, sql: &str, _params: &[Value]) -> Result<Result<Vec<Vec<Value>>, DbError>, String> {
        let mut stmt = self.0.prepare(sql).map_err(|e| format!("catalog query: {e}"))?;
        let width = stmt.column_count();
        let rows = stmt
            .query_map([], |row| {
                (0..width)
                    .map(|i| {
                        Ok(match row.get_ref(i)? {
                            rusqlite::types::ValueRef::Null => Value::Null,
                            rusqlite::types::ValueRef::Integer(n) => json!(n),
                            rusqlite::types::ValueRef::Real(f) => json!(f),
                            rusqlite::types::ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                            rusqlite::types::ValueRef::Blob(_) => Value::Null,
                        })
                    })
                    .collect::<Result<Vec<Value>, rusqlite::Error>>()
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<Vec<Value>>, rusqlite::Error>>()
            .map_err(|e| e.to_string())?;
        Ok(Ok(rows))
    }
}

// ---------------------------------------------------------------------------------------------
// The printed form
// ---------------------------------------------------------------------------------------------

/// `maybeOne (id: int4) -> { title: text, author: text? }`, then any diagnostics.
fn line(analysis: &Value) -> String {
    let field = |v: &Value| format!("{}: {}{}", v["name"].as_str().unwrap_or("?"), render(&v["type"]), if v["nullable"].as_bool().unwrap_or(false) { "?" } else { "" });
    let params: Vec<String> = analysis["params"].as_array().map(|a| a.iter().map(field).collect()).unwrap_or_default();
    let columns: Vec<String> = analysis["columns"].as_array().map(|a| a.iter().map(field).collect()).unwrap_or_default();
    let mut out = format!("{} ({}) -> {{ {} }}", analysis["cardinality"].as_str().unwrap_or("?"), params.join(", "), columns.join(", "));
    for d in analysis["diagnostics"].as_array().into_iter().flatten() {
        out.push_str(&format!(" !! reylo{}: {}", d["code"], d["message"].as_str().unwrap_or("")));
    }
    out
}

/// A type in one short line: `int4`, `'a' | 'b'`, `text[]`, `{ id: int4 }`, `[{ … }?]`,
/// `json {"type":"string"}`. An array
/// the statement builds says what it knows of its elements: `(text?)[]` when one may be NULL,
/// `(text!)[]` when none can be.
fn render(ty: &Value) -> String {
    let fields = |v: &Value| -> String {
        let parts: Vec<String> = v.as_array().into_iter().flatten().map(|f| format!("{}: {}{}", f["name"].as_str().unwrap_or("?"), render(&f["type"]), if f["nullable"].as_bool().unwrap_or(false) { "?" } else { "" })).collect();
        format!("{{ {} }}", parts.join(", "))
    };
    match ty["kind"].as_str() {
        Some("scalar") => ty["name"].as_str().unwrap_or("?").to_string(),
        Some("enum") => {
            let labels: Vec<String> = ty["labels"].as_array().into_iter().flatten().map(|l| format!("'{}'", l.as_str().unwrap_or(""))).collect();
            if labels.is_empty() { ty["name"].as_str().unwrap_or("enum").to_string() } else { labels.join(" | ") }
        }
        Some("array") => {
            let inner = render(&ty["of"]);
            match ty["nullable_items"].as_bool() {
                Some(true) => format!("({inner}?)[]"),
                Some(false) => format!("({inner}!)[]"),
                None => format!("{inner}[]"),
            }
        }
        Some("object") => fields(&ty["fields"]),
        Some("composite") => format!("{}{}", ty["name"].as_str().unwrap_or("record"), fields(&ty["fields"])),
        Some("list") => format!("[{}{}]", render(&ty["of"]), if ty["nullable_items"].as_bool().unwrap_or(false) { "?" } else { "" }),
        // A JSON column a CHECK holds to a JSON Schema: `json` and the canonical schema of the
        // values it admits.
        Some("json") => match ty.get("schema") {
            Some(schema) => format!("json {schema}"),
            None => "json".into(),
        },
        Some("unknown") => "unknown".into(),
        _ => ty.to_string(),
    }
}
