//! The engine against live databases.
//!
//! A case is a directory: `setup.sql` builds a schema, every other `.sql` file is one statement,
//! and `expected.txt` holds one line per statement in the form this tool prints. A directory with
//! no `expected.txt`, such as the parity check's statements, is checked for soundness alone. A Postgres case
//! runs in a database of its own, created on the server `--url` names and dropped afterwards, so
//! the server must be a throwaway one: cases create collations, operators and roles. A case whose
//! `dialect` file says `sqlite` runs in a fresh SQLite file instead, on libSQL's engine (Turso's
//! fork of SQLite, `libsql.rs`) where the case also has a `libsql` file. A case whose `dialect` file
//! says `mysql` runs in a database of its own on the MySQL server `--mysql-url` (or
//! `ORACLE_MYSQL_URL`) names, also a throwaway one, which it refuses on port 3306; how its host
//! answers the engine and how its runs are isolated is in `mysql.rs`. A case's `min_version` file
//! holds versions (a `server_version_num`, such as `180000`, or for MySQL the version as one
//! number, such as `80400`): a line with a version alone skips the case on an older server, and a
//! line `<file> <version>` skips that statement there, whose lines in the case's files are then not
//! compared. Where a server of an older series answers a statement differently and the analysis is
//! right for it, the case's `expected-<series>.txt` (`expected-14.txt`, `expected-8.0.txt`) stands
//! in for its `expected.txt` on that series, as `unchecked-<series>.txt` and `errors-<series>.txt`
//! do for the others; `--bless` writes the file a run is compared with. A MySQL case's `sql_mode`
//! file sets its sessions' `sql_mode`.
//!
//!   cargo run -p reylo-oracle -- --url postgres://postgres@localhost:25432/postgres oracle/cases
//!   cargo run -p reylo-oracle -- --mysql-url mysql://root@127.0.0.1:33411/mysql oracle/cases/mysql_types
//!   cargo run -p reylo-oracle -- --url … --bless oracle/cases/not_null   # rewrite expected.txt
//!   cargo run -p reylo-oracle -- --url … --analyses old.json oracle/cases  # check other answers
//!   cargo run -p reylo-oracle -- --url … --dump new.json oracle/cases      # write the answers
//!   cargo run -p reylo-oracle -- --url … --strict-coverage oracle/cases    # fail on a statement that never ran
//!   cargo run -p reylo-oracle -- --url … --timeout 2000 oracle/cases       # milliseconds per run
//!   cargo run -p reylo-oracle -- --url … --mutate oracle/mutations.json oracle/cases  # each check can fail
//!
//! Each Postgres and MySQL statement is analyzed again under other session settings, and a case
//! whose analysis changes fails: a descriptor never turns on the session.
//!
//! `--analyses` names a JSON object of analyses keyed `<case>/<file>`, which the seeding checks
//! in place of the engine's own: how a check that passes shows it can fail, run on the answers of
//! an engine that had a defect. `--dump` writes that object, for the cases of every dialect.
//!
//! `--mutate` names a JSON array of mutations, which shows that each check the oracle makes can
//! fail: each entry names a statement (`statement`, as `<case>/<file>`), a JSON pointer into the
//! engine's fresh analysis of it (`pointer`), the value put there (`value`), and a text that a
//! problem the oracle then reports holds (`problem`); `check` says which check it is for. Of the
//! cases named, each one an entry names is built, its statements of an entry analyzed and changed,
//! and the changed analyses checked as any other, each under its own name; the run fails unless
//! every entry's problem is reported. A case of the file's `cases` directory that no named case is
//! (another shard's) is left for its own run.
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
//!
//! Where an analysis narrows the errors its statement may raise (it runs only its database's own
//! code), every error a run raises must be one its database defines, naming only what the
//! narrowing lists. A case's `errors.txt`, where it has one, holds each statement's narrowing, and
//! is compared and blessed like `expected.txt`.
//!
//! Every parameter's and column's type an analysis gives holds to the format's own rules: it is a
//! `Type` of the format's JSON Schema, reads as the format's type and writes again as the same JSON,
//! and passes the validity rules of spec/README.md (`validity.rs`). A type that breaks one fails its
//! case.

mod errors;
mod libsql;
mod mysql;
mod replay;
mod seed;
mod validity;

use reylo_engine_core::types::Type;
use reylo_engine::AnswerCache;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use tokio_postgres::{Client, Config, NoTls};

/// The stack the oracle runs on. A descriptor is as deep as its statement nests (a JSON shape 495
/// levels deep in `deep_statements`), and reading one back through serde, which buffers each level
/// of a flattened kind, takes more than a main thread's 8 MiB in an unoptimized build. Reserved
/// address space: only the pages a deep descriptor touches are backed by memory.
const STACK_BYTES: usize = 256 << 20;

fn main() -> ExitCode {
    let oracle = std::thread::Builder::new().name("oracle".into()).stack_size(STACK_BYTES).spawn(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("the oracle's runtime starts").block_on(oracle()));
    oracle.expect("the oracle's thread starts").join().unwrap_or(ExitCode::FAILURE)
}

async fn oracle() -> ExitCode {
    let mut url = std::env::var("ORACLE_URL").ok();
    let mut mysql_url = std::env::var("ORACLE_MYSQL_URL").ok();
    let mut bless = false;
    let mut keep = false;
    let mut strict_coverage = false;
    let mut replay: Option<String> = None;
    let mut mysqld: Option<PathBuf> = None;
    let mut timeout_ms: u64 = 5000;
    let mut given = Map::new();
    let mut dump: Option<PathBuf> = None;
    let mut mutate: Option<PathBuf> = None;
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--url" => url = args.next(),
            "--mysql-url" => mysql_url = args.next(),
            "--bless" => bless = true,
            "--keep" => keep = true,
            "--strict-coverage" => strict_coverage = true,
            "--replay" => replay = args.next(),
            "--mysqld" => mysqld = args.next().map(PathBuf::from),
            "--timeout" => match args.next().and_then(|ms| ms.parse::<u64>().ok()).filter(|ms| *ms > 0) {
                Some(ms) => timeout_ms = ms,
                None => {
                    eprintln!("--timeout takes a number of milliseconds");
                    return ExitCode::from(2);
                }
            },
            "--dump" => dump = args.next().map(PathBuf::from),
            "--mutate" => mutate = args.next().map(PathBuf::from),
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
    // A database's own test suite, replayed: `replay/`.
    if let Some(database) = replay {
        let outcome = match (database.as_str(), paths.as_slice()) {
            ("sqlite", _) => Ok(replay::sqlite::run(&paths, timeout_ms)),
            ("postgres", [regress]) => match &url {
                Some(url) => replay::postgres::run(url, regress, timeout_ms).await,
                None => Err("--replay postgres runs on the server --url names".to_string()),
            },
            ("mysql", _) => match &mysqld {
                Some(mysqld) => replay::mysql::run(mysqld, &paths, timeout_ms).await,
                None => Err("--replay mysql runs on a server it starts from --mysqld <path to mysqld>".to_string()),
            },
            _ => Err("--replay takes postgres, sqlite or mysql, and the suite's directory: postgres's src/test/regress alone".to_string()),
        };
        return match outcome {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        };
    }
    let cases = cases(&paths);
    if cases.is_empty() {
        eprintln!("no cases: name a case directory, or a directory of them");
        return ExitCode::from(2);
    }
    if let Some(file) = mutate {
        return mutations(&file, &cases, url.as_deref(), mysql_url.as_deref(), timeout_ms).await;
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
        let (min, statement_mins) = min_versions(case);
        let mut on = On::default();
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
            let v = server.version as i32;
            on = On::new(case, format!("{}.{}", v / 10000, v / 100 % 100), v, statement_mins);
            mysql::run(case, server, keep, timeout_ms, &given, &mut dumped, on.statements(case)).await
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
            on = On::new(case, (version / 10000).to_string(), *version, statement_mins);
            run_postgres(case, client, config, &on, keep, timeout_ms, &given, &mut dumped).await
        };
        if bless && !on.skipped.is_empty() {
            println!("{}: FAILED: not blessed, since --bless would drop the lines of the statement(s) min_version skips on this server", case.display());
            failed += 1;
            continue;
        }
        // Every type the case's statements were given holds to the format's rules, whatever else
        // fails the case.
        let files: Vec<String> = on.statements(case).into_iter().map(|(file, _)| file).collect();
        let typed = validity::check(case, &files, &dumped);
        match outcome {
            Ok((lines, unchecked)) => {
                never_ran += unchecked.len();
                if !bless && !on.path(case, "unchecked").is_file() {
                    unnamed += unchecked.len();
                }
                print_unchecked(case, lines.len(), &unchecked);
                // Every verdict is printed, whichever fails the case.
                let matched = check(case, &lines, bless, &on);
                let narrowed = errors::check(case, &errors::lines(case, &files, &dumped), bless, &on);
                if !coverage(case, &unchecked, bless, strict_coverage, &on) || !matched || !narrowed || !typed {
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

/// A case's `min_version` file: the version a line holds alone, below which the case is skipped,
/// and each `<file> <version>` line, below which that statement is.
fn min_versions(case: &Path) -> (i32, Vec<(String, i32)>) {
    let mut min = 0;
    let mut statements = Vec::new();
    for line in fs::read_to_string(case.join("min_version")).unwrap_or_default().lines() {
        match line.split_whitespace().collect::<Vec<_>>()[..] {
            [version] => min = version.parse().unwrap_or(0),
            [file, version] => statements.push((file.to_string(), version.parse().unwrap_or(0))),
            _ => {}
        }
    }
    (min, statements)
}

/// The server a case runs on, as the case's files see it: its series, whose own files stand in
/// for the case's where it has them, and the statements `min_version` skips on it.
#[derive(Default)]
struct On {
    series: Option<String>,
    skipped: Vec<String>,
}

impl On {
    fn new(case: &Path, series: String, version: i32, statement_mins: Vec<(String, i32)>) -> On {
        let skipped: Vec<String> = statement_mins.into_iter().filter(|(_, min)| version < *min).map(|(file, _)| file).collect();
        if !skipped.is_empty() {
            println!("{}: note: skipped on server {version}, below their min_version: {}", case.display(), skipped.join(", "));
        }
        On { series: Some(series), skipped }
    }

    /// The case's statements that run on this server.
    fn statements(&self, case: &Path) -> Vec<(String, String)> {
        statements(case).into_iter().filter(|(file, _)| !self.skipped.contains(file)).collect()
    }

    /// The case's `<name>-<series>.txt` where it has one, else its `<name>.txt`.
    fn path(&self, case: &Path, name: &str) -> PathBuf {
        self.series.as_ref().map(|series| case.join(format!("{name}-{series}.txt"))).filter(|path| path.is_file()).unwrap_or_else(|| case.join(format!("{name}.txt")))
    }

    /// The text of the case's file `name` without the lines of the statements skipped here, or
    /// None where the case has no such file.
    fn read(&self, case: &Path, name: &str) -> Option<String> {
        let text = fs::read_to_string(self.path(case, name)).ok()?;
        if self.skipped.is_empty() {
            return Some(text);
        }
        let skipped = |line: &str| self.skipped.iter().any(|file| line.strip_prefix(file.as_str()).is_some_and(|rest| rest.starts_with(": ")));
        Some(text.lines().filter(|line| !skipped(line)).map(|line| format!("{line}\n")).collect())
    }
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
fn check(case: &Path, lines: &[String], bless: bool, on: &On) -> bool {
    let actual = lines.join("\n") + "\n";
    if bless {
        fs::write(on.path(case, "expected"), &actual).expect("expected.txt is writable");
        println!("{}: blessed {} statement(s)", case.display(), lines.len());
        return true;
    }
    let Some(expected) = on.read(case, "expected") else {
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
fn coverage(case: &Path, unchecked: &[String], bless: bool, strict: bool, on: &On) -> bool {
    let path = on.path(case, "unchecked");
    if bless {
        // An empty unchecked.txt says that every statement of the case ran, and stays.
        if !unchecked.is_empty() || path.is_file() {
            fs::write(&path, unchecked.iter().map(|line| format!("{line}\n")).collect::<String>()).expect("unchecked.txt is writable");
        }
        return true;
    }
    let Some(expected) = on.read(case, "unchecked") else {
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
    // As the engine's own connections do: a server built with JIT compiles the engine's catalog
    // queries for seconds each, on every statement of every case. The oracle checks types, not plans.
    let _ = client.batch_execute("set jit = off").await;
    Ok(client)
}

/// The server `url` names, refused on port 5432, the shared server's: the oracle creates and drops databases, and runs only on a throwaway cluster.
fn throwaway_config(url: &str) -> Result<Config, String> {
    let config = Config::from_str(url).map_err(|e| e.to_string())?;
    if config.get_ports().contains(&5432) {
        return Err("port 5432 is the shared server; the oracle runs only on a throwaway cluster".into());
    }
    Ok(config)
}

async fn connect_admin(url: &str) -> Result<(Client, Config, i32), String> {
    let config = throwaway_config(url)?;
    let client = connect(&config).await?;
    let version: String = client.query_one("SHOW server_version_num", &[]).await.map_err(|e| e.to_string())?.get(0);
    Ok((client, config, version.parse().unwrap_or(0)))
}

/// A database of the case's own on the server, dropped first if a run left it, and built by its
/// `setup.sql`: its name and a session on it.
async fn postgres_case(case: &Path, admin: &Client, admin_config: &Config) -> Result<(String, Client), String> {
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
    Ok((db, client))
}

async fn run_postgres(case: &Path, admin: &Client, admin_config: &Config, on: &On, keep: bool, timeout_ms: u64, given: &Map<String, Value>, dumped: &mut Map<String, Value>) -> Result<(Vec<String>, Vec<String>), String> {
    let name: String = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (db, client) = postgres_case(case, admin, admin_config).await?;
    let mut lines = Vec::new();
    let mut analyzed = Vec::new();
    for (file, sql) in on.statements(case) {
        let analysis = match given.get(&format!("{name}/{file}")) {
            Some(answer) => answer.clone(),
            None => analyze_postgres(&client, &sql).await.map_err(|e| format!("{file}: {e}"))?,
        };
        lines.push(format!("{file}: {}", line(&analysis)));
        dumped.insert(format!("{name}/{file}"), analysis.clone());
        analyzed.push((file, sql, analysis));
    }
    same_under_other_settings(&client, &name, &analyzed, given).await?;
    // A value that contradicts a type is a failure whatever expected.txt says.
    let report = seed::check(&client, &analyzed, timeout_ms).await;
    drop(client);
    if !keep {
        let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {db} WITH (FORCE)")).await;
    }
    let unchecked = seeded(case, lines.len(), report.map_err(|e| format!("seeding: {e}"))?)?;
    Ok((lines, unchecked))
}

/// What a Postgres session sets that may change how the server prints a value or an expression,
/// and that no statement's meaning turns on, each with another of its values.
const OTHER_SETTINGS: [(&str, &str); 5] = [("DateStyle", "Postgres, DMY"), ("IntervalStyle", "postgres_verbose"), ("TimeZone", "Asia/Kolkata"), ("extra_float_digits", "0"), ("bytea_output", "escape")];

/// Each statement analyzed again under other session settings, which must change nothing: a
/// descriptor is written in canonical text, never the session's. `search_path` gains a schema that
/// does not exist, which leaves what every name resolves to as it was. Each setting goes back to
/// what the case's own setup left it at.
async fn same_under_other_settings(client: &Client, case: &str, analyzed: &[(String, String, Value)], given: &Map<String, Value>) -> Result<(), String> {
    let setting = |name: &str, value: &str| format!("SELECT set_config('{name}', '{}', false)", value.replace('\'', "''"));
    let mut before = Vec::new();
    for (name, value) in OTHER_SETTINGS.iter().copied().chain([("search_path", "")]) {
        let current: String = client.query_one(&format!("SELECT current_setting('{name}')"), &[]).await.map_err(|e| pg_message(&e))?.get(0);
        let other = if name == "search_path" { format!("{current}, reylo_oracle_absent") } else { value.to_string() };
        client.batch_execute(&setting(name, &other)).await.map_err(|e| format!("{name}: {}", pg_message(&e)))?;
        before.push((name, current));
    }
    let mut outcome = Ok(());
    for (file, sql, analysis) in analyzed.iter().filter(|(file, _, _)| !given.contains_key(&format!("{case}/{file}"))) {
        match analyze_postgres(client, sql).await {
            Ok(again) if again == *analysis => {}
            Ok(again) => {
                outcome = Err(format!("{file}: another session's settings change its analysis: {} became {}", line(analysis), line(&again)));
                break;
            }
            Err(e) => {
                outcome = Err(format!("{file}: under another session's settings: {e}"));
                break;
            }
        }
    }
    for (name, value) in before {
        client.batch_execute(&setting(name, &value)).await.map_err(|e| format!("{name}: {}", pg_message(&e)))?;
    }
    outcome
}

async fn analyze_postgres(client: &Client, sql: &str) -> Result<Value, String> {
    let analysis = reylo_engine::clients::tokio_postgres::analyze(client, sql, &mut AnswerCache::default()).await.map_err(|e| e.to_string())?;
    serde_json::to_value(&analysis).map_err(|e| e.to_string())
}

fn pg_message(e: &tokio_postgres::Error) -> String {
    e.as_db_error().map(|d| format!("{} ({})", d.message(), d.code().code())).unwrap_or_else(|| e.to_string())
}

// ---------------------------------------------------------------------------------------------
// SQLite
// ---------------------------------------------------------------------------------------------

/// A fresh SQLite file for the case, built by its `setup.sql`: its path and a connection to it.
fn sqlite_case(case: &Path) -> Result<(PathBuf, rusqlite::Connection), String> {
    let name: String = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let file = std::env::temp_dir().join(format!("reylo-oracle-{name}.db"));
    let _ = fs::remove_file(&file);
    let setup = fs::read_to_string(case.join("setup.sql")).map_err(|e| e.to_string())?;
    // A libSQL case's schema is made on libSQL, whose file the engine reads as the engine's program
    // reads one, with rusqlite's SQLite, and its statements run on libSQL (`check_sqlite_case`).
    // Each connection is closed before the next is opened: two SQLite libraries in one process do
    // not see each other's locks on a file.
    if case.join("libsql").exists() {
        let lib = libsql::Connection::open(&file).map_err(|e| e.to_string())?;
        seed::SqliteDb::run_batch(&lib, &setup).map_err(|e| format!("setup.sql on libSQL {}: {e}", libsql::version()))?;
        drop(lib);
        let db = rusqlite::Connection::open(&file).map_err(|e| e.to_string())?;
        return Ok((file, db));
    }
    let db = rusqlite::Connection::open(&file).map_err(|e| e.to_string())?;
    db.execute_batch(&setup).map_err(|e| format!("setup.sql: {e}"))?;
    Ok((file, db))
}

fn run_sqlite(case: &Path, timeout_ms: u64, given: &Map<String, Value>, dumped: &mut Map<String, Value>) -> Result<(Vec<String>, Vec<String>), String> {
    let name: String = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let (file, db) = sqlite_case(case)?;
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
    let report = check_sqlite_case(case, &file, db, &analyzed, timeout_ms);
    let _ = fs::remove_file(&file);
    let unchecked = seeded(case, lines.len(), report.map_err(|e| format!("seeding: {e}"))?)?;
    Ok((lines, unchecked))
}

/// The case's statements run over its seeded states, on libSQL for a libSQL case, with the
/// engine's connection to the file closed first.
fn check_sqlite_case(case: &Path, file: &Path, db: rusqlite::Connection, analyzed: &[(String, String, Value)], timeout_ms: u64) -> Result<seed::Report, String> {
    if case.join("libsql").exists() {
        drop(db);
        let lib = libsql::Connection::open(file).map_err(|e| e.to_string())?;
        return seed::check_sqlite(&lib, analyzed, timeout_ms);
    }
    seed::check_sqlite(&db, analyzed, timeout_ms)
}

fn analyze_sqlite(db: &rusqlite::Connection, sql: &str) -> Result<Value, String> {
    let analysis = reylo_engine::clients::rusqlite::analyze(db, sql, &mut AnswerCache::default()).map_err(|e| e.to_string())?;
    serde_json::to_value(&analysis).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------------------------

/// One entry of a `--mutate` file: a statement of a case, a JSON pointer into the engine's fresh
/// analysis of it, the value put there, and a text that a problem the oracle reports must hold.
pub(crate) struct Mutation {
    pub(crate) case: String,
    pub(crate) file: String,
    pointer: String,
    value: Value,
    problem: String,
}

impl Mutation {
    /// The entries of a `--mutate` file.
    fn read(path: &Path) -> Result<Vec<Mutation>, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("--mutate {}: {e}", path.display()))?;
        let Ok(Value::Array(entries)) = serde_json::from_str::<Value>(&text) else { return Err(format!("--mutate {}: not a JSON array of mutations", path.display())) };
        entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let field = |key: &str| entry[key].as_str().map(String::from).ok_or_else(|| format!("--mutate {}: mutation {} has no {key}", path.display(), i + 1));
                let statement = field("statement")?;
                let Some((case, file)) = statement.split_once('/') else { return Err(format!("--mutate {}: mutation {} names no <case>/<file>", path.display(), i + 1)) };
                let value = entry.get("value").cloned().ok_or_else(|| format!("--mutate {}: mutation {} has no value", path.display(), i + 1))?;
                Ok(Mutation { case: case.into(), file: file.into(), pointer: field("pointer")?, value, problem: field("problem")? })
            })
            .collect()
    }

    /// The name the changed statement runs under, the `n`-th of the file, which every problem it
    /// raises starts with.
    pub(crate) fn label(&self, n: usize) -> String {
        format!("{} [mutation {n}]", self.file)
    }

    /// The analysis with the value at the pointer: in place of what is there, or as a new key of
    /// the object that holds it.
    pub(crate) fn apply(&self, mut analysis: Value) -> Result<Value, String> {
        if let Some(at) = analysis.pointer_mut(&self.pointer) {
            *at = self.value.clone();
            return Ok(analysis);
        }
        let (parent, key) = self.pointer.rsplit_once('/').ok_or_else(|| format!("{}: {} is no JSON pointer", self.file, self.pointer))?;
        match analysis.pointer_mut(parent) {
            Some(Value::Object(object)) => {
                object.insert(key.replace("~1", "/").replace("~0", "~"), self.value.clone());
                Ok(analysis)
            }
            _ => Err(format!("{}: {} names nothing in its analysis", self.file, self.pointer)),
        }
    }
}

/// Run each mutation of `file` whose case is among `cases`, and report whether the oracle reported
/// its problem.
async fn mutations(file: &Path, cases: &[PathBuf], url: Option<&str>, mysql_url: Option<&str>, timeout_ms: u64) -> ExitCode {
    let entries = match Mutation::read(file) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    // A case no named directory is may be another shard's, but it is one of the file's own cases.
    let root = file.parent().unwrap_or(Path::new(".")).join("cases");
    for (i, m) in entries.iter().enumerate() {
        if !root.join(&m.case).join(&m.file).is_file() {
            eprintln!("--mutate {}: mutation {} names {}/{}, which {} does not hold", file.display(), i + 1, m.case, m.file, root.display());
            return ExitCode::from(2);
        }
    }
    let (mut caught, mut missed) = (0, 0);
    let mut admin: Option<(Client, Config, i32)> = None;
    let mut mysql_server: Option<mysql::Server> = None;
    for case in cases {
        let name = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mine: Vec<(usize, &Mutation)> = entries.iter().enumerate().filter(|(_, m)| m.case == name).map(|(i, m)| (i + 1, m)).collect();
        if mine.is_empty() {
            continue;
        }
        let dialect = fs::read_to_string(case.join("dialect")).map(|s| s.trim().to_string()).unwrap_or_else(|_| "postgres".into());
        let timeout_ms = fs::read_to_string(case.join("timeout")).ok().and_then(|s| s.trim().parse::<u64>().ok()).filter(|ms| *ms > 0).unwrap_or(timeout_ms);
        let report = if dialect == "sqlite" {
            mutate_sqlite(case, &mine, timeout_ms)
        } else if dialect == "mysql" {
            if mysql_server.is_none() {
                let Some(url) = mysql_url else {
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
            mysql::mutate(case, mysql_server.as_mut().expect("connected above"), timeout_ms, &mine).await
        } else {
            if admin.is_none() {
                let Some(url) = url else {
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
            let (client, config, _) = admin.as_ref().expect("connected above");
            mutate_postgres(case, client, config, timeout_ms, &mine).await
        };
        let report = match report {
            Ok(report) => report,
            Err(e) => {
                println!("{}: FAILED: {e}", case.display());
                missed += mine.len();
                continue;
            }
        };
        for (n, m) in &mine {
            let label = m.label(*n);
            let raised: Vec<&String> = report.problems.iter().filter(|p| p.starts_with(&format!("{label}: "))).collect();
            match raised.iter().find(|p| p.contains(&m.problem)) {
                Some(problem) => {
                    caught += 1;
                    println!("{}: mutation {n} ({} {}) caught: {problem}", case.display(), m.file, m.pointer);
                }
                None => {
                    missed += 1;
                    println!("{}: MISSED mutation {n} ({} {}): no problem says {:?}", case.display(), m.file, m.pointer, m.problem);
                    for (_, why) in report.unchecked.iter().filter(|(file, _)| *file == label) {
                        println!("    it never ran: {why}");
                    }
                    for problem in &raised {
                        println!("    {problem}");
                    }
                }
            }
        }
    }
    println!("{} mutation(s), {missed} missed", caught + missed);
    if missed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// A Postgres case's mutations, checked over its seeded states as the case's statements are.
async fn mutate_postgres(case: &Path, admin: &Client, admin_config: &Config, timeout_ms: u64, mutations: &[(usize, &Mutation)]) -> Result<seed::Report, String> {
    let (db, client) = postgres_case(case, admin, admin_config).await?;
    let mut analyzed = Vec::new();
    let outcome = async {
        for (n, m) in mutations {
            let sql = fs::read_to_string(case.join(&m.file)).map_err(|e| format!("{}: {e}", m.file))?;
            let analysis = analyze_postgres(&client, &sql).await.map_err(|e| format!("{}: {e}", m.file))?;
            analyzed.push((m.label(*n), sql, m.apply(analysis)?));
        }
        seed::check(&client, &analyzed, timeout_ms).await
    }
    .await;
    drop(client);
    let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {db} WITH (FORCE)")).await;
    outcome
}

/// A SQLite case's mutations, checked over its seeded states as the case's statements are.
fn mutate_sqlite(case: &Path, mutations: &[(usize, &Mutation)], timeout_ms: u64) -> Result<seed::Report, String> {
    let (file, db) = sqlite_case(case)?;
    let outcome = (|| {
        let mut analyzed = Vec::new();
        for (n, m) in mutations {
            let sql = fs::read_to_string(case.join(&m.file)).map_err(|e| format!("{}: {e}", m.file))?;
            let analysis = analyze_sqlite(&db, &sql).map_err(|e| format!("{}: {e}", m.file))?;
            analyzed.push((m.label(*n), sql, m.apply(analysis)?));
        }
        Ok(analyzed)
    })();
    let outcome = outcome.and_then(|analyzed| check_sqlite_case(case, &file, db, &analyzed, timeout_ms));
    let _ = fs::remove_file(&file);
    outcome
}

// ---------------------------------------------------------------------------------------------
// The printed form
// ---------------------------------------------------------------------------------------------

/// `many (id: int4) -> { id: int4, title: text, author: text? } key (id) ordered`, ` value` where the
/// result is the one column's values, then any diagnostics: each type as [`Type::short`] prints it.
fn line(analysis: &Value) -> String {
    let field = |v: &Value| format!("{}: {}{}", v["name"].as_str().unwrap_or("?"), render(&v["type"]), if v["nullable"].as_bool().unwrap_or(false) { "?" } else { "" });
    let params: Vec<String> = analysis["params"].as_array().map(|a| a.iter().map(field).collect()).unwrap_or_default();
    let columns: Vec<String> = analysis["columns"].as_array().map(|a| a.iter().map(field).collect()).unwrap_or_default();
    let mut out = format!("{} ({}) -> {{ {} }}", analysis["cardinality"].as_str().unwrap_or("?"), params.join(", "), columns.join(", "));
    // The result keys, which a result of more than one row may have.
    if let Some(keys) = analysis["unique"].as_array().filter(|k| !k.is_empty()) {
        let shown: Vec<String> = keys.iter().map(|key| format!("({})", key.as_array().into_iter().flatten().map(|k| k.as_str().unwrap_or("?")).collect::<Vec<_>>().join(", "))).collect();
        out.push_str(&format!(" key {}", shown.join(", ")));
    }
    // The columns of the result its ORDER BY sorts on.
    if let Some(order) = analysis["order"].as_array().filter(|o| !o.is_empty()) {
        let shown: Vec<String> = order.iter().map(|o| format!("{}{} nulls {}", o["column"].as_str().unwrap_or("?"), if o["desc"].as_bool() == Some(true) { " desc" } else { "" }, o["nulls"].as_str().unwrap_or("?"))).collect();
        out.push_str(&format!(" sorted ({})", shown.join(", ")));
    }
    // An ORDER BY that orders the rows totally.
    if analysis["ordered"].as_bool() == Some(true) {
        out.push_str(" ordered");
    }
    if analysis["value"].as_bool() == Some(true) {
        out.push_str(" value");
    }
    for d in analysis["diagnostics"].as_array().into_iter().flatten() {
        out.push_str(&format!(" !! reylo{}: {}", d["code"], d["message"].as_str().unwrap_or("")));
    }
    out
}

/// A type in one short line ([`Type::short`]), then the comments its fields carry (`/* a.b: '…' */`),
/// or the JSON it is where it is no type the engine writes.
fn render(ty: &Value) -> String {
    match serde_json::from_value::<Type>(ty.clone()) {
        Ok(t) => {
            let mut notes = Vec::new();
            comments(&t, "", &mut notes);
            if notes.is_empty() { t.short() } else { format!("{} /* {} */", t.short(), notes.join(", ")) }
        }
        Err(_) => ty.to_string(),
    }
}

/// Each comment a field inside `ty` carries, at its path from the type (`author.name: 'Shown publicly'`).
fn comments(ty: &Type, path: &str, out: &mut Vec<String>) {
    use reylo_engine_core::types::Kind;
    let at = |name: &str| if path.is_empty() { name.to_string() } else { format!("{path}.{name}") };
    match &ty.kind {
        Kind::Struct(s) => {
            for f in &s.fields {
                if let Some(comment) = &f.comment {
                    out.push(format!("{}: '{}'", at(&f.name), comment.replace('\'', "''").replace('\n', "\\n")));
                }
                comments(&f.ty, &at(&f.name), out);
            }
        }
        Kind::Array(a) => comments(&a.element.ty, &at("[]"), out),
        Kind::Json(j) => {
            if let Some(content) = &j.content {
                comments(&content.ty, path, out);
            }
        }
        Kind::Union(u) => u.members.iter().for_each(|m| comments(m, path, out)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_mutation_puts_its_value_at_its_pointer() {
        let m = |pointer: &str, value: Value| Mutation { case: "c".into(), file: "s.sql".into(), pointer: pointer.into(), value, problem: String::new() };
        let analysis = json!({"cardinality": "many", "columns": [{"name": "id", "nullable": true}]});
        assert_eq!(m("/cardinality", json!("one")).apply(analysis.clone()).unwrap()["cardinality"], "one");
        assert_eq!(m("/columns/0/nullable", json!(false)).apply(analysis.clone()).unwrap()["columns"][0]["nullable"], false);
        // A key the analysis leaves out goes into the object that would hold it.
        assert_eq!(m("/unique", json!([["id"]])).apply(analysis.clone()).unwrap()["unique"], json!([["id"]]));
        assert_eq!(m("/columns/0/a~1b", json!(1)).apply(analysis.clone()).unwrap()["columns"][0]["a/b"], 1);
        assert!(m("/columns/3/nullable", json!(false)).apply(analysis.clone()).is_err());
        assert!(m("/params/0", json!(1)).apply(analysis).is_err());
        assert_eq!(m("", json!(1)).label(3), "s.sql [mutation 3]");
    }
}
