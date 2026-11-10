//! The commands of `reylo migrate` and `reylo db`, and `reylo run`. Each takes the project's settings and a log it
//! writes its lines to, and answers the process's exit code; an `Err` is a refusal to start (a
//! setting, a file or a connection), which the command line prints.

use crate::catalog::{catalog, difference, HISTORY_TABLE};
use crate::db::{shown_url, Db, Server};
use crate::lock;
use crate::replay::{Shadows, Step};
use crate::sql::{atomicity_problem, changes_data, runs_alone, shadow_problem, split_statements};
use crate::{time, Dialect, Error};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

/// A project's `migrate` settings, resolved.
#[derive(Clone, Debug)]
pub struct Options {
    pub url: String,
    /// A server to make shadow databases on, where the target's refuses `create database`.
    pub shadow_url: Option<String>,
    pub dir: PathBuf,
    pub schema: PathBuf,
    /// The lock's file, `reylo/reylo-lock.yaml`.
    pub lock: PathBuf,
    /// The cache of replays and the SQLite lock files, `reylo/.cache/`.
    pub cache: PathBuf,
    /// The project's directory: a relative SQLite path is its.
    pub root: PathBuf,
}

pub type Log<'a> = &'a mut dyn FnMut(&str);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Migration {
    pub name: String,
    pub time: String,
    pub path: PathBuf,
    pub sql: String,
}

#[derive(Clone, Debug)]
struct HistoryRow {
    name: String,
    sql: String,
    applied: bool,
    /// The live schema's fingerprint right after the run that applied it ([`fingerprint`]), on the
    /// last row of each run.
    fingerprint: Option<String>,
}

const INIT: &str = "init.sql";
static NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{14})_([a-z0-9_]+)\.sql$").expect("a valid pattern"));

static COLOR: AtomicBool = AtomicBool::new(false);

/// Whether lines carry ANSI colors: the command line turns it on for a terminal.
/// Whether the command runs with `--json`: each refusal it prints is kept for the JSON it ends with.
static JSON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static REPORTED: std::sync::Mutex<Vec<Problem>> = std::sync::Mutex::new(Vec::new());
/// What a command gives `--json` beside its refusals: `status`'s rows, for one.
static RESULT: std::sync::Mutex<Option<serde_json::Value>> = std::sync::Mutex::new(None);

pub fn set_json(on: bool) {
    JSON.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The refusals printed since the last call, as `--json` gives them.
pub fn reported() -> Vec<serde_json::Value> {
    std::mem::take(&mut *REPORTED.lock().unwrap_or_else(|e| e.into_inner())).iter().map(Problem::json).collect()
}

/// What the command gave `--json` beside its refusals, if anything.
pub fn result() -> Option<serde_json::Value> {
    RESULT.lock().unwrap_or_else(|e| e.into_inner()).take()
}

fn set_result(value: serde_json::Value) {
    if JSON.load(std::sync::atomic::Ordering::Relaxed) {
        *RESULT.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
    }
}

pub fn set_color(on: bool) {
    COLOR.store(on, Ordering::Relaxed);
}

pub fn paint(style: &str, text: &str) -> String {
    if !COLOR.load(Ordering::Relaxed) {
        return text.to_string();
    }
    let code = match style {
        "red" => "31",
        "green" => "32",
        "yellow" => "33",
        "dim" => "2",
        "cyan" => "36",
        "bold red" => "1;31",
        "bold" => "1",
        _ => "0",
    };
    format!("\x1b[{code}m{text}\x1b[0m")
}

fn normalized(sql: &str) -> String {
    sql.replace("\r\n", "\n").replace('\r', "\n").trim_end().to_string()
}

/// A path as the person running the command reads it: relative to where they are.
fn shown(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    match path.strip_prefix(&cwd) {
        Ok(rel) if rel.as_os_str().is_empty() => ".".into(),
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

/// The migration files of `dir`, in time order (`init.sql` first), and the files that are not migrations.
pub fn read_migrations(dir: &Path) -> Result<(Vec<Migration>, Vec<String>), Error> {
    if !dir.exists() {
        return Ok((vec![], vec![]));
    }
    let mut names: Vec<String> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut files = Vec::new();
    let mut strays = Vec::new();
    for name in names {
        let path = dir.join(&name);
        if name == INIT {
            files.push(Migration { sql: std::fs::read_to_string(&path)?, name, time: "00000000000000".into(), path });
        } else if let Some(m) = NAME.captures(&name) {
            let time = m[1].to_string();
            files.push(Migration { sql: std::fs::read_to_string(&path)?, name, time, path });
        } else if !name.starts_with('.') {
            strays.push(name);
        }
    }
    files.sort_by(|a, b| a.time.cmp(&b.time).then(a.name.cmp(&b.name)));
    Ok((files, strays))
}

// ---------------------------------------------------------------------------------------------
// The history table.

fn history_ddl(d: Dialect) -> String {
    match d {
        Dialect::Postgres => format!("create table if not exists {HISTORY_TABLE} (name text primary key, sql text not null, applied_at timestamptz, fingerprint text, catalog text)"),
        Dialect::Mysql => format!("create table if not exists {HISTORY_TABLE} (name varchar(255) primary key, `sql` longtext not null, applied_at datetime(6) null, fingerprint text null, catalog longtext null) engine=InnoDB"),
        Dialect::Sqlite => format!("create table if not exists {HISTORY_TABLE} (name text primary key, sql text not null, applied_at text, fingerprint text, catalog text)"),
    }
}

/// The time a row is applied at: now, and after every row already there, so that rows written in
/// one run read back in the order they were written. On SQLite, whose clock has milliseconds, a tie
/// is broken by the row's rowid instead ([`read_history`]).
fn now_sql(d: Dialect) -> String {
    match d {
        Dialect::Postgres => format!("greatest(clock_timestamp(), (select max(applied_at) + interval '1 microsecond' from {HISTORY_TABLE}))"),
        Dialect::Mysql => format!("greatest(now(6), coalesce((select max(h.applied_at) + interval 1 microsecond from (select applied_at from {HISTORY_TABLE}) h), now(6)))"),
        Dialect::Sqlite => "strftime('%Y-%m-%d %H:%M:%f', 'now')".into(),
    }
}

fn sql_col(d: Dialect) -> &'static str {
    if d == Dialect::Mysql {
        "`sql`"
    } else {
        "sql"
    }
}

async fn has_history(db: &mut Db) -> Result<bool, Error> {
    let sql = match db.dialect() {
        Dialect::Postgres => format!("select to_regclass('{HISTORY_TABLE}') is not null as ok"),
        Dialect::Mysql => format!("select count(*) > 0 as ok from information_schema.tables where table_schema = database() and table_name = '{HISTORY_TABLE}'"),
        Dialect::Sqlite => format!("select count(*) > 0 as ok from sqlite_schema where name = '{HISTORY_TABLE}'"),
    };
    Ok(db.rows(&sql).await?.first().is_some_and(|r| r.flag("ok")))
}

async fn read_history(db: &mut Db) -> Result<Vec<HistoryRow>, Error> {
    if !has_history(db).await? {
        return Ok(vec![]);
    }
    let d = db.dialect();
    // Rows are written each after the last (`now_sql`); on SQLite, whose clock has milliseconds, rows
    // written in one run tie, and are read in the order they were written, by rowid.
    let applied_at = if d == Dialect::Postgres { "applied_at::text" } else { "applied_at" };
    let tie = if d == Dialect::Sqlite { "rowid, " } else { "" };
    // A table an earlier build made has no fingerprint column until the next run adds it.
    let fingerprint = if has_fingerprint(db).await? { "fingerprint" } else { "null as fingerprint" };
    let rows = db
        .rows(&format!(
            "select name, {} as body, {applied_at} as applied_at, {fingerprint} from {HISTORY_TABLE} order by applied_at is null, applied_at, {tie}case when name = '{INIT}' then '' else name end",
            sql_col(d)
        ))
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| HistoryRow { name: r.text("name").into(), sql: r.text("body").into(), applied: r.get("applied_at").is_some(), fingerprint: r.get("fingerprint").map(str::to_string) })
        .collect())
}

async fn has_fingerprint(db: &mut Db) -> Result<bool, Error> {
    has_column(db, "fingerprint").await
}

/// Whether the history table has `column`, which a table an earlier build made may lack.
async fn has_column(db: &mut Db, column: &str) -> Result<bool, Error> {
    let sql = match db.dialect() {
        Dialect::Postgres => format!("select count(*) > 0 as ok from pg_attribute where attrelid = to_regclass('{HISTORY_TABLE}') and attname = '{column}' and not attisdropped"),
        Dialect::Mysql => format!("select count(*) > 0 as ok from information_schema.columns where table_schema = database() and table_name = '{HISTORY_TABLE}' and column_name = '{column}'"),
        Dialect::Sqlite => format!("select count(*) > 0 as ok from pragma_table_info('{HISTORY_TABLE}') where name = '{column}'"),
    };
    Ok(db.rows(&sql).await?.first().is_some_and(|r| r.flag("ok")))
}

/// Makes the history table, or adds what an earlier build's table lacks.
async fn ensure_history(db: &mut Db) -> Result<(), Error> {
    let d = db.dialect();
    db.exec(&history_ddl(d)).await?;
    if !has_fingerprint(db).await? {
        db.exec(&format!("alter table {HISTORY_TABLE} add column fingerprint text")).await?;
    }
    if !has_column(db, "catalog").await? {
        db.exec(&format!("alter table {HISTORY_TABLE} add column catalog {}", if d == Dialect::Mysql { "longtext null" } else { "text" })).await?;
    }
    Ok(())
}

/// The lines a fingerprint is the digest of: what deploy shows of a database changed since.
fn fingerprinted(lines: &[String]) -> Vec<String> {
    crate::catalog::without_children(&crate::catalog::without_comments(lines))
}

/// Keeps the lines `name`'s fingerprint is of, on its row alone, so that a refusal can show what
/// changed since with no shadow. Nothing where the table has no column for them.
async fn store_catalog(db: &mut Db, name: &str, lines: &[String]) -> Result<(), Error> {
    if !has_column(db, "catalog").await? {
        return Ok(());
    }
    let text = fingerprinted(lines).join("\n");
    db.exec_with(&format!("update {HISTORY_TABLE} set catalog = null where name <> {} and catalog is not null", db.param(1)), &[&name]).await?;
    db.exec_with(&format!("update {HISTORY_TABLE} set catalog = {} where name = {}", db.param(1), db.param(2)), &[&text, &name]).await
}

/// The lines stored with `name`'s fingerprint, where they were.
async fn stored_catalog(db: &mut Db, name: &str) -> Result<Option<Vec<String>>, Error> {
    if !has_column(db, "catalog").await? {
        return Ok(None);
    }
    let rows = db.rows(&format!("select catalog from {HISTORY_TABLE} where name = {}", crate::db::literal(name))).await?;
    Ok(rows.first().and_then(|r| r.get("catalog")).map(|text| if text.is_empty() { vec![] } else { text.split('\n').map(str::to_string).collect() }))
}

/// The live schema's fingerprint: a digest of its catalog as the comparator reads it, with the
/// server's version and this build's catalog code, which both change what the catalog reads as. A
/// database whose fingerprint equals the one its last run stored has not been changed since, its
/// comments aside.
async fn fingerprint(db: &mut Db) -> Result<String, Error> {
    let lines = catalog(db).await?;
    fingerprint_of(db, &lines).await
}

/// The live schema's fingerprint, stored on `name`'s row with the lines it is of.
async fn store_fingerprint(db: &mut Db, name: &str) -> Result<(), Error> {
    let lines = catalog(db).await?;
    let value = fingerprint_of(db, &lines).await?;
    set_fingerprint(db, name, &value).await?;
    store_catalog(db, name, &lines).await
}

/// The fingerprint of catalog lines already read from `db`.
async fn fingerprint_of(db: &mut Db, lines: &[String]) -> Result<String, Error> {
    // The server's major version: a minor release fixes bugs and leaves the catalog's text alone, so
    // that an upgrade a managed host makes on its own leaves the fingerprint comparable.
    let version = match db.dialect() {
        Dialect::Postgres => "select (current_setting('server_version_num')::int / 10000)::text as v",
        Dialect::Mysql => "select substring_index(version(), '.', 2) as v",
        Dialect::Sqlite => "select sqlite_version() as v",
    };
    let version = db.rows(version).await?.first().map(|r| r.text("v").to_string()).unwrap_or_default();
    let mut digest = crate::cache::Sha256::new();
    // A child table may have been made at run time, as data: only a replay can tell.
    for line in fingerprinted(lines) {
        digest.part(line.as_bytes());
    }
    Ok(format!("{}:{version}:{}", crate::cache::code_id(), crate::cache::hex(&digest.finish())))
}

async fn set_fingerprint(db: &mut Db, name: &str, value: &str) -> Result<(), Error> {
    db.exec_with(&format!("update {HISTORY_TABLE} set fingerprint = {} where name = {}", db.param(1), db.param(2)), &[&value, &name]).await
}

/// Waits until no other run of `reylo migrate` works on this database, so that two that start at
/// once (two instances of an app, each migrating as it starts) neither decide alike nor run alike:
/// a session lock on Postgres and MySQL, held until the connection closes. SQLite's own locks would
/// stop the app's queries for the length of the check, so a SQLite database is locked through a
/// file named after its path in the user's temporary directory, which every checkout and worktree
/// of theirs that opens the database shares, held while the answer lives.
async fn lock(db: &mut Db, opts: &Options) -> Result<Option<std::fs::File>, Error> {
    match db.dialect() {
        Dialect::Postgres => db.exec("select pg_advisory_lock(hashtextextended('reylo_migrate', 0))").await.map(|_| None),
        // A lock of this database's alone (its name is at most 64 characters), which answers 0
        // once the wait runs out.
        Dialect::Mysql => {
            let rows = db.rows("select get_lock(concat('reylo_migrate_', left(sha1(coalesce(database(), '')), 32)), 600) as ok").await?;
            if !rows.first().is_some_and(|r| r.flag("ok")) {
                return Err(Error("another run of reylo migrate held this database for ten minutes; try again once it finishes".into()));
            }
            Ok(None)
        }
        Dialect::Sqlite => {
            let path = crate::db::sqlite_path(&opts.url, &opts.root);
            if path == ":memory:" {
                return Ok(None);
            }
            let canonical = std::fs::canonicalize(&path).map(|p| p.to_string_lossy().into_owned()).unwrap_or(path);
            let mut digest = crate::cache::Sha256::new();
            digest.part(canonical.as_bytes());
            let dir = std::env::temp_dir().join(format!("reylo-migrate-locks{}", user_suffix()));
            std::fs::create_dir_all(&dir).map_err(|e| Error(format!("cannot make {}: {e}", dir.display())))?;
            let path = dir.join(crate::cache::hex(&digest.finish()[..8]));
            let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path).map_err(|e| Error(format!("cannot open {}: {e}", path.display())))?;
            file.lock()?;
            Ok(Some(file))
        }
    }
}

/// What names a user's own directory of run locks in a temporary directory every user shares, as
/// Linux's `/tmp` is: their id. macOS and Windows give each user a temporary directory of their own.
fn user_suffix() -> String {
    #[cfg(unix)]
    {
        // SAFETY: getuid has no preconditions and cannot fail.
        format!("-{}", unsafe { libc::getuid() })
    }
    #[cfg(not(unix))]
    {
        String::new()
    }
}

/// What a row's fingerprint says while its run hasn't read it yet: written in the migration's own
/// transaction, so that a run cut off before it reads one leaves a row the next deploy refuses
/// rather than one with nothing to compare. The read follows the commit, so the migration's locks
/// are not held for it.
const PENDING: &str = "pending";

/// The migration's row: applied now, or marked started; with the fingerprint of the schema it left,
/// where the run that applied it read one.
async fn insert_row(db: &mut Db, m: &Migration, applied: bool, fingerprint: Option<&str>) -> Result<(), Error> {
    let d = db.dialect();
    let at = if applied { now_sql(d) } else { "null".to_string() };
    let stamp = if fingerprint.is_some() { db.param(3) } else { "null".to_string() };
    let sql = format!("insert into {HISTORY_TABLE} (name, {}, applied_at, fingerprint) values ({}, {}, {at}, {stamp})", sql_col(d), db.param(1), db.param(2));
    match fingerprint {
        Some(value) => db.exec_with(&sql, &[&m.name, &m.sql, value]).await,
        None => db.exec_with(&sql, &[&m.name, &m.sql]).await,
    }
}

// ---------------------------------------------------------------------------------------------
// Running a migration: in one transaction with its history row, or, for a file that cannot run in
// one, alone, with a row marked started before it and finished after it. The row then gets the
// fingerprint of the schema the migration left: after the commit, once its locks are released, over
// a row marked pending in the same transaction; and for a lone statement in the update that finishes
// its row. No run cut off between the two leaves an applied row with nothing to compare.

pub(crate) async fn run(db: &mut Db, m: &Migration, record: bool) -> Result<(), Error> {
    let d = db.dialect();
    if runs_alone(&m.sql, d) {
        if record {
            insert_row(db, m, false, None).await?;
        }
        // A failed `create index concurrently` (or `reindex concurrently`) leaves an invalid index
        // behind; the ones this statement left are dropped, so the file can run again as written.
        let invalid = async |db: &mut Db| -> Result<Vec<String>, Error> {
            if d != Dialect::Postgres {
                return Ok(vec![]);
            }
            Ok(db.rows("select indexrelid::regclass::text as i from pg_index where not indisvalid").await?.iter().map(|r| r.text("i").to_string()).collect())
        };
        let before = invalid(db).await?;
        if let Err(e) = db.script(&m.sql).await {
            for index in invalid(db).await.unwrap_or_default() {
                if !before.contains(&index) {
                    let _ = db.exec(&format!("drop index concurrently if exists {index}")).await;
                }
            }
            if record {
                let _ = db.exec_with(&format!("delete from {HISTORY_TABLE} where name = {}", db.param(1)), &[&m.name]).await;
            }
            return Err(e);
        }
        if record {
            let lines = catalog(db).await?;
            let value = fingerprint_of(db, &lines).await?;
            db.exec_with(&format!("update {HISTORY_TABLE} set applied_at = {}, fingerprint = {} where name = {}", now_sql(d), db.param(1), db.param(2)), &[&value, &m.name]).await?;
            store_catalog(db, &m.name, &lines).await?;
        }
        return Ok(());
    }
    // SQLite cannot turn foreign keys off inside a transaction, and a table rebuilt in a migration
    // (create, copy, drop, rename) must not cascade its drop: they are off for the transaction and
    // checked before it commits.
    if d == Dialect::Sqlite {
        db.exec("pragma foreign_keys = off").await?;
    }
    db.exec(if d == Dialect::Mysql { "start transaction" } else { "begin" }).await?;
    let result = async {
        db.script(&m.sql).await?;
        if d == Dialect::Sqlite {
            let broken = db.rows("pragma foreign_key_check").await?;
            if !broken.is_empty() {
                return Err(Error(format!("{} rows break a foreign key after this migration", broken.len())));
            }
        }
        if record {
            insert_row(db, m, true, Some(PENDING)).await?;
        }
        db.exec("commit").await
    }
    .await;
    if result.is_err() {
        let _ = db.exec("rollback").await;
    }
    if d == Dialect::Sqlite {
        db.exec("pragma foreign_keys = on").await?;
    }
    result?;
    // The migration has committed: a fingerprint that can't be read now leaves its row pending,
    // which the next deploy refuses, rather than reading as a migration that failed.
    if record {
        let _ = store_fingerprint(db, &m.name).await;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// The check.

fn describe_diff(label_a: &str, a: &[String], label_b: &str, b: &[String]) -> Vec<String> {
    const MAX: usize = 20;
    let (only_a, only_b) = difference(a, b);
    let mut out: Vec<String> = only_a.iter().take(MAX).map(|l| paint("red", &format!("  - {label_a}: {}", readable(l)))).collect();
    out.extend(only_b.iter().take(MAX).map(|l| paint("green", &format!("  + {label_b}: {}", readable(l)))));
    let more = only_a.len() + only_b.len() - out.len();
    if more > 0 {
        out.push(format!("  … and {more} more"));
    }
    out
}

/// A comparison line as a person reads it: `public.` left out, and an index or a constraint as SQL
/// writes it, its keywords in lowercase, rather than as the catalog prints it.
fn readable(line: &str) -> String {
    static INDEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^index CREATE (UNIQUE )?INDEX (\S+) ON (?:ONLY )?(\S+) USING (\w+) (.*)$").expect("a valid pattern"));
    static CONSTRAINT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^constraint (\S+) (\S+) (.*)$").expect("a valid pattern"));
    static PUBLIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(^|[\s(,])(?:public|"public")\."#).expect("a valid pattern"));
    let line = PUBLIC.replace_all(line, "$1").into_owned();
    if let Some(c) = INDEX.captures(&line) {
        let using = if &c[4] == "btree" { String::new() } else { format!("using {} ", &c[4]) };
        return format!("{}index {} on {} {using}{}", if c.get(1).is_some() { "unique " } else { "" }, &c[2], &c[3], lowercase_keywords(&c[5]));
    }
    if let Some(c) = CONSTRAINT.captures(&line) {
        return format!("constraint {} on {}: {}", &c[2], &c[1], lowercase_keywords(&c[3]));
    }
    line
}

/// SQL's keywords in lowercase, outside its quotes: `PRIMARY KEY (id)` is `primary key (id)`.
fn lowercase_keywords(sql: &str) -> String {
    let mut out = String::new();
    let mut quote: Option<char> = None;
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if word.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
            out.push_str(&word.to_lowercase());
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for ch in sql.chars() {
        match quote {
            Some(q) => {
                out.push(ch);
                if ch == q {
                    quote = None;
                }
            }
            None if ch == '\'' || ch == '"' => {
                flush(&mut word, &mut out);
                quote = Some(ch);
                out.push(ch);
            }
            None if ch.is_ascii_alphanumeric() || ch == '_' => word.push(ch),
            None => {
                flush(&mut word, &mut out);
                out.push(ch);
            }
        }
    }
    flush(&mut word, &mut out);
    out
}

/// What changed from `before` to `after`, a line each, readable: `+` what `after` has, `-` what it
/// lost. An index a constraint of the same name makes (a primary key's, a unique constraint's) goes
/// with it.
fn changed_lines(before: &[String], after: &[String]) -> Vec<String> {
    const MAX: usize = 20;
    let (gone, came) = difference(before, after);
    let constraint_names = |lines: &[String]| -> Vec<String> { lines.iter().filter_map(|l| l.strip_prefix("constraint ")).filter_map(|rest| rest.split(' ').nth(1)).map(str::to_string).collect() };
    let made_by_constraint = |line: &str, names: &[String]| names.iter().any(|n| line.starts_with("index ") && line.contains(&format!(" INDEX {n} ON ")));
    let (gone_names, came_names) = (constraint_names(&gone), constraint_names(&came));
    let gone: Vec<&String> = gone.iter().filter(|l| !made_by_constraint(l, &gone_names)).collect();
    let came: Vec<&String> = came.iter().filter(|l| !made_by_constraint(l, &came_names)).collect();
    let mut out: Vec<String> = came.iter().take(MAX).map(|l| paint("green", &format!("  + {}", readable(l)))).collect();
    out.extend(gone.iter().take(MAX).map(|l| paint("red", &format!("  - {}", readable(l)))));
    let more = gone.len() + came.len() - out.len();
    if more > 0 {
        out.push(format!("  … and {more} more"));
    }
    out
}

/// Prints the refusals of a command that changes something, and says that it changed nothing.
fn refuse(problems: &[Problem], log: Log<'_>) -> i32 {
    print_problems(problems, log);
    log("");
    log(&paint("dim", "Nothing was changed."));
    1
}

/// What every command calls a database changed by hand since its last migration.
const CHANGED_OUTSIDE: &str = "This database was changed outside its migrations";

/// One thing a check refuses: its heading, and the lines under it, the SQL first and what to run last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// What kind of refusal it is, which stays the same from one version to the next: for a program
    /// reading `--json`, where the title's words may change.
    pub code: &'static str,
    pub title: String,
    pub lines: Vec<String>,
    /// The parts of `lines`, as `--json` gives them.
    pub detail: Vec<String>,
    pub sql: Option<String>,
    pub notes: Vec<String>,
    pub fixes: Vec<(String, Vec<String>)>,
    fixed: bool,
    /// Whether it is the lock's drafts, which some commands pass over.
    drafts: bool,
}

impl Problem {
    fn new(code: &'static str, title: impl Into<String>) -> Problem {
        Problem { code, title: title.into(), lines: vec![], detail: vec![], sql: None, notes: vec![], fixes: vec![], fixed: false, drafts: false }
    }

    /// The refusal as `--json` gives it.
    pub fn json(&self) -> serde_json::Value {
        let mut out = serde_json::json!({ "code": self.code, "title": self.title });
        let detail: Vec<String> = self.detail.iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        if !detail.is_empty() {
            out["detail"] = detail.into();
        }
        if let Some(sql) = &self.sql {
            out["sql"] = sql.clone().into();
        }
        if !self.notes.is_empty() {
            out["notes"] = self.notes.clone().into();
        }
        if !self.fixes.is_empty() {
            out["fixes"] = self.fixes.iter().map(|(what, commands)| serde_json::json!({ "what": what, "commands": commands })).collect();
        }
        out
    }

    fn is_drafts(&self) -> bool {
        self.drafts
    }

    /// SQL under the heading, set off by blank lines: what differs, never a fix to run.
    fn sql(mut self, sql: &str) -> Problem {
        self.sql = Some(sql.trim_end().to_string());
        self.lines.push(String::new());
        self.lines.extend(sql.trim_end().lines().map(|l| format!("    {l}")));
        self.lines.push(String::new());
        self
    }

    fn note(mut self, text: impl AsRef<str>) -> Problem {
        self.notes.push(text.as_ref().to_string());
        self.lines.push(format!("  {}", text.as_ref()));
        self
    }

    /// What to do, then each command that does it on a line of its own.
    fn fix(mut self, what: impl AsRef<str>, commands: &[&str]) -> Problem {
        if !self.fixed && self.lines.last().is_none_or(|l| !l.is_empty()) {
            self.lines.push(String::new());
        }
        self.fixed = true;
        self.fixes.push((what.as_ref().to_string(), commands.iter().map(|c| c.to_string()).collect()));
        self.lines.push(format!("  {}:", what.as_ref()));
        self.lines.extend(commands.iter().map(|c| format!("    {}", paint("cyan", c))));
        self
    }

    fn lines(mut self, lines: impl IntoIterator<Item = String>) -> Problem {
        let lines: Vec<String> = lines.into_iter().collect();
        self.detail.extend(lines.iter().cloned());
        self.lines.extend(lines);
        self
    }
}

#[derive(Debug, Default)]
pub struct Plan {
    pub problems: Vec<Problem>,
    /// Pending migrations recorded without running: lone statements a cut-off run left, which landed.
    pub records: Vec<Migration>,
    /// Migrations to run, in order.
    pub pending: Vec<Migration>,
    /// Whether the live schema is the one Reylo's last run on this database left, by the
    /// fingerprint that run stored; None where no run stored one, or another server version or
    /// catalog reader read it.
    pub unchanged: Option<bool>,
    /// The records that are lone statements cut off before their row was finished, which landed.
    pub landed: Vec<String>,
    /// The invalid indexes such statements left, which `deploy` drops before running them again.
    pub leftovers: Vec<Leftover>,
    /// The migrations the database ran, in its order: with no lock, a pending one that sorts before
    /// any of them must be shown to end where name order does.
    pub ran: Vec<Migration>,
}

/// An index a lone `create index concurrently` (or `reindex concurrently`) left invalid when it was
/// cut off.
#[derive(Debug, Clone, Default)]
pub struct Leftover {
    /// The migration whose row is marked started.
    pub migration: String,
    /// The index, schema-qualified.
    pub index: String,
    /// Its catalog line, up to its `(invalid)` mark.
    line: String,
}

/// The invalid indexes on a Postgres database that the started rows' statements left: found as the
/// server found what each names, on its own table.
async fn leftovers(db: &mut Db, history: &[HistoryRow]) -> Result<Vec<Leftover>, Error> {
    let started: Vec<&HistoryRow> = history.iter().filter(|r| !r.applied).collect();
    if db.dialect() != Dialect::Postgres || started.is_empty() {
        return Ok(vec![]);
    }
    const NAME: &str = r#"(?:"(?:[^"]|"")*"|[^\s(".;]+)(?:\.(?:"(?:[^"]|"")*"|[^\s(".;]+))*"#;
    let create = regex::Regex::new(&format!(r"(?is)^create\s+(?:unique\s+)?index\s+concurrently\s+(?:if\s+not\s+exists\s+)?(?:({NAME})\s+)?on\s+(?:only\s+)?({NAME})")).unwrap();
    let drop = regex::Regex::new(&format!(r"(?is)^drop\s+index\s+concurrently\s+(?:if\s+exists\s+)?({NAME})")).unwrap();
    let reindex = regex::Regex::new(&format!(r"(?is)^reindex\s+(?:\([^)]*\)\s*)?(index|table)\s+concurrently\s+({NAME})")).unwrap();
    let literal = |s: &str| format!("'{}'", s.replace('\'', "''"));
    // A name as the server stores it: unquoted folded to lower case.
    let stored = |name: &str| match name.strip_prefix('"').and_then(|n| n.strip_suffix('"')) {
        Some(inner) => inner.replace("\"\"", "\""),
        None => name.to_lowercase(),
    };
    let mut out = Vec::new();
    for m in started {
        // The one statement, without the comments above it.
        let Some(statement) = split_statements(&m.sql, Dialect::Postgres).into_iter().next() else { continue };
        let sql = statement.text.trim().trim_end_matches(';').trim();
        let found = if let Some(c) = create.captures(sql) {
            // An index is made in its table's schema, and named without one.
            let on = format!("i.indrelid = to_regclass({})", literal(&c[2]));
            match c.get(1) {
                Some(name) => format!("{on} and c.relname = {}", literal(&stored(name.as_str()))),
                None => on,
            }
        } else if let Some(c) = drop.captures(sql) {
            format!("c.oid = to_regclass({})", literal(&c[1]))
        } else if let Some(c) = reindex.captures(sql) {
            // A concurrent reindex builds <index>_ccnew beside each index, and swaps the old one to <index>_ccold.
            let copies = |base: &str| format!("(c.relname like {} or c.relname like {})", literal(&format!("{}\\_ccnew%", base.replace('_', "\\_"))), literal(&format!("{}\\_ccold%", base.replace('_', "\\_"))));
            if c[1].eq_ignore_ascii_case("index") {
                let bare = stored(&crate::diff::last_written(&c[2]));
                format!("i.indrelid = (select x.indrelid from pg_index x where x.indexrelid = to_regclass({})) and {}", literal(&c[2]), copies(&bare))
            } else {
                format!("i.indrelid = to_regclass({}) and {}", literal(&c[2]), copies("%"))
            }
        } else {
            continue;
        };
        for r in db
            .rows(&format!("select format('%I.%I', n.nspname, c.relname) as qualified, pg_get_indexdef(c.oid) as def from pg_index i join pg_class c on c.oid = i.indexrelid join pg_namespace n on n.oid = c.relnamespace where not i.indisvalid and {found}"))
            .await?
        {
            out.push(Leftover { migration: m.name.clone(), index: r.text("qualified").to_string(), line: format!("index {} (invalid)", r.text("def")) });
        }
    }
    Ok(out)
}

async fn drop_leftovers(db: &mut Db, leftovers: &[Leftover], log: Log<'_>) -> Result<(), Error> {
    for x in leftovers {
        db.exec(&format!("drop index concurrently if exists {}", x.index)).await?;
        log(&format!("{} Dropped {}, which {} left invalid when it was cut off", paint("yellow", "!"), x.index, x.migration));
    }
    Ok(())
}

impl Plan {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Whether two texts are the same statements, comments and whitespace aside.
fn same_statements(a: &str, b: &str, d: Dialect) -> bool {
    let tokens = |sql: &str| split_statements(sql, d).into_iter().map(|s| s.tokens.into_iter().map(|t| t.text).collect::<Vec<_>>()).collect::<Vec<_>>();
    tokens(a) == tokens(b)
}

/// The statements of a migration that change data, as their first words: such a migration always
/// runs, since no comparison of schemas can see whether its rows were written.
fn data_statements(sql: &str, d: Dialect) -> Vec<String> {
    split_statements(sql, d).iter().filter(|s| changes_data(s)).map(|s| format!("{}…", s.text.split_whitespace().take(3).collect::<Vec<_>>().join(" ").to_lowercase())).collect()
}

/// The lines of `old` and `new` as a unified diff without its hunk headers: unchanged lines more
/// than one away from a change are left out.
fn line_diff(old: &str, new: &str) -> Vec<String> {
    let a: Vec<&str> = old.trim_end().lines().collect();
    let b: Vec<&str> = new.trim_end().lines().collect();
    let (n, m) = (a.len(), b.len());
    if n.saturating_mul(m) > 4_000_000 {
        return vec!["    (the texts differ; too long to show line by line)".into()];
    }
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let mut ops: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            ops.push((' ', a[i]));
            i += 1;
            j += 1;
        } else if i < n && (j == m || lcs[i + 1][j] >= lcs[i][j + 1]) {
            ops.push(('-', a[i]));
            i += 1;
        } else {
            ops.push(('+', b[j]));
            j += 1;
        }
    }
    let changed = |k: usize| ops.get(k).is_some_and(|(op, _)| *op != ' ');
    let mut out = vec![paint("dim", "    --- as applied"), paint("dim", "    +++ on disk")];
    let mut skipped = false;
    for (k, (op, text)) in ops.iter().enumerate() {
        if *op == ' ' && !(k > 0 && changed(k - 1)) && !changed(k + 1) {
            if !skipped {
                out.push(paint("dim", "    …"));
                skipped = true;
            }
            continue;
        }
        skipped = false;
        let line = format!("    {op} {text}");
        out.push(match op {
            '-' => paint("red", &line),
            '+' => paint("green", &line),
            _ => line,
        });
    }
    out
}

/// How an entry of the lock stands against the migration files.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Link {
    /// Its file, chained to the entry before it.
    Ok,
    /// No file of its name.
    Missing,
    /// Its file's text isn't what was locked.
    Edited,
    /// Locked after another entry than the one before it (`after`): two branches' lines merged.
    Moved { after: Option<String> },
}

/// Each entry's link, in the lock's order.
fn links(l: &lock::Lock, files: &[Migration]) -> Vec<Link> {
    let mut out = Vec::new();
    for (k, e) in l.entries.iter().enumerate() {
        let Some(f) = files.iter().find(|f| f.name == e.name) else {
            out.push(Link::Missing);
            continue;
        };
        let text = normalized(&f.sql);
        let prev = if k == 0 { String::new() } else { l.entries[k - 1].sum.clone() };
        if lock::checksum(&prev, &e.name, &text) == e.sum {
            out.push(Link::Ok);
            continue;
        }
        // Chained after an earlier entry, or as the first: moved there from another branch's lock.
        let parents = std::iter::once((None, String::new())).chain(l.entries[..k.saturating_sub(1)].iter().map(|p| (Some(p.name.clone()), p.sum.clone())));
        match parents.filter(|(_, sum)| *sum != prev).find(|(_, sum)| lock::checksum(sum, &e.name, &text) == e.sum) {
            Some((after, _)) => out.push(Link::Moved { after }),
            None => out.push(Link::Edited),
        }
    }
    out
}

/// The project's lock, or what keeps it from being read. A project with no migrations
/// needs none yet.
fn read_lock(opts: &Options) -> Result<lock::Lock, Problem> {
    let path = shown(&opts.lock);
    match lock::read(&opts.lock) {
        lock::State::Lock(l) => Ok(l),
        // No lock: the history isn't linear, and the migrations run in name order.
        lock::State::Missing => Ok(lock::Lock::default()),
        lock::State::Conflicted => Err(Problem::new("lock-conflict", format!("{path} has a merge conflict: another branch added migrations too"))
            .note("Keep main's lines and delete this branch's (git checkout --theirs while merging main in, --ours while rebasing).")
            .fix("Then run this branch's migrations after main's, which locks them again", &["reylo migrate dev"])),
        lock::State::Invalid(e) => Err(Problem::new("lock-unreadable", format!("{path} isn't a lock file")).note(unreadable_why(&e)).fix("Write it again from what a database applied", &["reylo migrate history pull"])),
    }
}

/// Why a lock doesn't read, as a person can act on it: the line the YAML parser stopped at.
fn unreadable_why(error: &str) -> String {
    static LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"line (\d+)").expect("a valid pattern"));
    match LINE.captures(error) {
        Some(c) => format!("Its line {} doesn't parse.", &c[1]),
        None => format!("It doesn't parse: {error}."),
    }
}

/// The problems of the lock against the migration files, for a command that runs
/// what it lists as it is (deploy, its dry run): an entry with no file, a file edited after it was locked,
/// lines merged out of their order, and drafts.
fn lock_problems(opts: &Options, files: &[Migration]) -> Vec<Problem> {
    if lock_free(opts) {
        return vec![];
    }
    let path = shown(&opts.lock);
    let l = match read_lock(opts) {
        Ok(l) => l,
        Err(p) => return vec![p],
    };
    let mut problems = Vec::new();
    for (e, link) in l.entries.iter().zip(links(&l, files)) {
        match link {
            Link::Ok => {}
            Link::Missing => problems.push(Problem::new("lock-lists-missing-file", format!("{path} lists {}, which isn't in {}", e.name, shown(&opts.dir))).fix("Write it back from a database that ran it", &["reylo migrate history pull"])),
            Link::Edited => problems.push(
                Problem::new("locked-migration-edited", format!("{} changed after it was locked", e.name))
                    .note("A database that ran the old text refuses it: if one did, put the change in a new migration instead.")
                    .fix("If no database ran the old text (it failed everywhere), lock it as it now reads", &["reylo migrate dev"])
                    .fix("To take back the text a database ran", &["reylo migrate history pull"]),
            ),
            Link::Moved { after } => problems.push(
                Problem::new("lock-out-of-order", format!("{path} was merged out of order at {}", e.name))
                    .note(format!("It was locked {}. Keep main's lines, in main's order, and delete this branch's.", after.map_or("first".to_string(), |a| format!("right after {a}"))))
                    .fix("Then run this branch's migrations after main's, which locks them again", &["reylo migrate dev"]),
            ),
        }
    }
    let drafts: Vec<&str> = files.iter().filter(|f| !l.entries.iter().any(|e| e.name == f.name)).map(|f| f.name.as_str()).collect();
    if !drafts.is_empty() {
        let one = drafts.len() == 1;
        let title = if one { format!("{} isn't locked yet", drafts[0]) } else { format!("{} migrations aren't locked yet", drafts.len()) };
        let listed: Vec<String> = if one { vec![] } else { drafts.iter().map(|d| format!("    {d}")).collect() };
        let mut problem = Problem::new("migration-draft", title).lines(listed).fix(format!("Run {} in development, which locks {}", if one { "it" } else { "them" }, if one { "it" } else { "them" }), &["reylo migrate dev"]);
        problem.drafts = true;
        problems.push(problem);
    }
    problems
}

/// Whether the project keeps no lock: its history isn't linear, and its migrations run in name order.
fn lock_free(opts: &Options) -> bool {
    lock::read(&opts.lock) == lock::State::Missing
}

/// Where a project with no lock has migrations that sort before one this database already ran: their
/// replay after what it ran, and the replay of every migration in name order, as a fresh database runs
/// them, must leave the same schema (tables' columns compared as a set, as everywhere). None where
/// there are none, or they do.
async fn late_problem(shadows: &mut Shadows<'_>, ran: &[Migration], pending: &[Migration], all: &[Migration]) -> Result<Option<Problem>, Error> {
    let Some(last) = ran.iter().map(|m| name_order(&m.name)).max() else { return Ok(None) };
    let late: Vec<&Migration> = pending.iter().filter(|m| name_order(&m.name) < last).collect();
    // A database that already ran its migrations out of name order (as a lock, since deleted, had
    // them) is held to name order too, pending or not.
    let in_order = ran.windows(2).all(|w| name_order(&w[0].name) <= name_order(&w[1].name));
    let Some(first) = late.first() else {
        if in_order {
            return Ok(None);
        }
        let here = shadows.lane();
        shadows.push(here, ran.iter().chain(pending).map(Step::of));
        let fresh = shadows.lane();
        let mut by_name = all.to_vec();
        by_name.sort_by(|a, b| name_order(&a.name).cmp(&name_order(&b.name)));
        shadows.push(fresh, by_name.iter().map(Step::of));
        return Ok(match (shadows.lines(here).await?, shadows.lines(fresh).await?) {
            (Ok(a), Ok(b)) if crate::catalog::same_schema(&a, &b) => None,
            (Ok(a), Ok(b)) => Some(
                Problem::new("history-out-of-order", "This database ran its migrations in another order than a fresh database runs them, and the two end differently")
                    .lines(describe_diff("this database", &crate::catalog::without_comments(&a), "a fresh database", &crate::catalog::without_comments(&b)))
                    .note("Elsewhere, write a migration that takes both to the same schema.")
                    .fix("On a development database, rebuild it in name order", &["reylo db reset"]),
            ),
            (Err((m, e)), _) | (_, Err((m, e))) => Some(Problem::new("replay-fails", format!("{m} fails when replayed in a shadow database")).note(e)),
        });
    };
    let after = ran.iter().filter(|m| name_order(&m.name) > name_order(&first.name)).map(|m| m.name.clone()).collect::<Vec<_>>();
    let here = shadows.lane();
    shadows.push(here, ran.iter().chain(pending).map(Step::of));
    let fresh = shadows.lane();
    let mut by_name = all.to_vec();
    by_name.sort_by(|a, b| name_order(&a.name).cmp(&name_order(&b.name)));
    shadows.push(fresh, by_name.iter().map(Step::of));
    let names = late.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ");
    let rename = format!("Give {} a later name, so that every database runs {} after {}", names, if late.len() == 1 { "it" } else { "them" }, after.join(", "));
    let problem = match (shadows.lines(here).await?, shadows.lines(fresh).await?) {
        (Ok(a), Ok(b)) if crate::catalog::same_schema(&a, &b) => return Ok(None),
        (Ok(a), Ok(b)) => Problem::new("late-migration-differs", format!("{names} {} before {}, which this database already ran, and the two orders end differently", if late.len() == 1 { "sorts" } else { "sort" }, after.join(", ")))
            .lines(describe_diff("this database", &crate::catalog::without_comments(&a), "a fresh database", &crate::catalog::without_comments(&b))),
        (Err((m, e)), _) => Problem::new("late-migration-fails", format!("{m} fails when run after what this database already ran")).note(e),
        (_, Err((m, e))) => Problem::new("late-migration-fails", format!("{m} fails when the migrations run in name order, as a fresh database runs them")).note(e),
    };
    Ok(Some(problem.note(rename)))
}

/// Where a migration falls in name order: init.sql, which holds the schema a project started
/// from, first; then the rest by name, which starts with the time each was written.
fn name_order(name: &str) -> (bool, &str) {
    (name != INIT, name)
}

/// The order the migrations run in: the lock's, then its drafts by name.
fn run_order(opts: &Options, files: &[Migration]) -> Vec<Migration> {
    let Ok(l) = read_lock(opts) else { return files.to_vec() };
    let mut out: Vec<Migration> = l.entries.iter().filter_map(|e| files.iter().find(|f| f.name == e.name).cloned()).collect();
    out.extend(files.iter().filter(|f| !l.entries.iter().any(|e| e.name == f.name)).cloned());
    out
}

/// Checks the target against the migration files; changes nothing. With a `server` (`reylo migrate
/// check`) it replays them in shadow databases, against the live schema and schema.sql; with none
/// (`reylo migrate deploy`) it replays nothing, and trusts the fingerprint Reylo's last run stored: a
/// database that no longer has it is refused, and what the fingerprint can't speak to is `--dry-run`'s.
pub async fn check(db: &mut Db, server: Option<&mut Server>, opts: &Options) -> Result<Plan, Error> {
    let d = db.dialect();
    let (files, strays) = read_migrations(&opts.dir)?;
    let mut problems: Vec<Problem> = stray_problem(opts, &strays).into_iter().collect();
    // What schema.sql declares is held against the migrations in development and CI, not at deploy;
    // a project without one keeps its schema in its migrations alone.
    let schema_sql = if server.is_some() { std::fs::read_to_string(&opts.schema).ok() } else { None };
    // At deploy, with no shadow, schema.sql is held to the lock's checksum alone (CI's lint refuses
    // the same), after whatever else deploy refuses.
    let stamped = (server.is_none() && stamp(opts, std::fs::read_to_string(&opts.schema).ok().as_deref()) == Stamp::Changed).then(|| stamp_problem(opts));
    let with_stamp = |mut problems: Vec<Problem>| {
        problems.extend(stamped.clone());
        problems
    };
    let history = read_history(db).await?;
    // A migrations directory that isn't there, on a database that ran migrations, is one refusal: a
    // typo in dir, not every migration taken away.
    if let Some(problem) = missing_dir(opts, &history) {
        return Ok(Plan { problems: vec![problem], ..Plan::default() });
    }
    // A migration this database ran, edited since, is refused as what this database ran, not twice.
    let ran_edited: Vec<String> = history.iter().filter(|r| r.applied && files.iter().any(|f| f.name == r.name && !same_statements(&f.sql, &r.sql, d))).map(|r| r.name.clone()).collect();
    problems.extend(lock_problems(opts, &files).into_iter().filter(|p| !(p.code == "locked-migration-edited" && ran_edited.iter().any(|n| p.title.starts_with(&format!("{n} "))))));
    let files = run_order(opts, &files);
    // A row marked started (a lone statement cut off before its row was finished) counts as pending:
    // whether it landed is read from the live database below, as for any pending migration.
    let applied: Vec<&HistoryRow> = history.iter().filter(|r| r.applied).collect();
    let by_name = |name: &str| files.iter().find(|f| f.name == name);
    let dir = shown(&opts.dir);
    for r in &applied {
        match by_name(&r.name) {
            None => problems.push(missing_problem(&r.name, &r.sql, &dir)),
            // An edit to comments or spacing alone changes nothing that ran.
            Some(f) if !same_statements(&f.sql, &r.sql, d) => problems.push(edited_problem(&r.name, &r.sql, &f.sql)),
            Some(_) => {}
        }
    }
    let applied_files: Vec<Migration> = applied.iter().filter_map(|r| by_name(&r.name).cloned()).collect();
    // Every database runs the lock's order, and nothing else; with no lock, a late one is proven at deploy.
    if problems.is_empty() && !lock_free(opts) {
        if let Some(k) = applied.iter().zip(&files).position(|(r, f)| r.name != f.name) {
            problems.push(Problem::new("history-out-of-order", format!("This database applied {} where {} lists {}", applied[k].name, shown(&opts.lock), files[k].name)).note("Every database runs the migrations in the lock's order. Where one ran them otherwise, the lock was merged keeping a branch's lines: keep main's.").fix("On a development database, take it to the lock's order", &["reylo migrate dev"]));
        }
    }
    // Nothing is recorded without running but what a cut-off run of Reylo's own left (below).
    let records: Vec<Migration> = Vec::new();
    let pending: Vec<Migration> = files.iter().filter(|f| !applied_files.iter().any(|a| a.name == f.name) && !records.contains(f)).cloned().collect();
    // init.sql, which `init` writes, holds the whole schema and runs only on an empty database.
    for p in pending.iter().filter(|f| f.name != INIT) {
        if let Some(why) = atomicity_problem(&p.sql, d) {
            problems.push(Problem::new("not-atomic", format!("{} can't run atomically", p.name)).note(why));
        }
    }
    if server.is_some() {
        let schema_name = opts.schema.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "schema.sql".into());
        for (name, sql) in files.iter().map(|f| (f.name.as_str(), f.sql.as_str())).chain(schema_sql.as_deref().map(|s| (schema_name.as_str(), s))) {
            if let Some(why) = shadow_problem(sql, d) {
                problems.push(Problem::new("not-replayable", format!("{name} can't be replayed in a shadow database")).note(why));
            }
        }
    }
    if !problems.is_empty() {
        return Ok(Plan { problems: with_stamp(problems), ..Plan::default() });
    }

    // An index a cut-off `create index concurrently` left invalid is dropped before its migration
    // runs again, so it is compared as gone.
    let leftovers = leftovers(db, &history).await?;
    let live: Vec<String> = catalog(db).await?.into_iter().filter(|l| !leftovers.iter().any(|x| l.starts_with(&x.line))).collect();
    let stored = applied.last().and_then(|r| r.fingerprint.clone());
    // A run cut off after a migration committed, before it read the schema the migration left.
    let unread = stored.as_deref() == Some(PENDING);
    if unread && server.is_none() {
        let problem = Problem::new("fingerprint-cut-off", format!("A run was cut off after {} applied, before it stored this database's fingerprint", applied.last().map(|r| r.name.as_str()).unwrap_or_default()))
            .note("So nothing says whether anything else has changed this database since.")
            .fix("See whether it is as its migrations make it", &["reylo migrate deploy --dry-run"])
            .fix("Then store its fingerprint, running nothing", &["reylo migrate history push"]);
        return Ok(Plan { problems: with_stamp(vec![problem]), ..Plan::default() });
    }
    let unchanged = match stored {
        Some(_) if unread => Some(false),
        Some(stored) => {
            let now = fingerprint_of(db, &live).await?;
            // Only a fingerprint read the same way says anything: the reader and the server's version.
            let how = |f: &str| f.rsplit_once(':').map(|(how, _)| how.to_string());
            (how(&stored) == how(&now)).then_some(stored == now)
        }
        None => None,
    };
    let Some(server) = server else {
        // No replay: the fingerprint says whether the database is as Reylo's last run left it.
        if unchanged == Some(false) {
            let mut problem = Problem::new("database-changed", CHANGED_OUTSIDE);
            if let Some(stored) = match applied.last() {
                Some(last) => stored_catalog(db, &last.name).await?,
                None => None,
            } {
                problem = problem.lines(std::iter::once(String::new()).chain(changed_lines(&stored, &fingerprinted(&live))).chain([String::new()]));
            }
            let problem = problem
                .fix("If a migration makes this change, record it here, running nothing", &["reylo migrate history push"])
                .fix("If none does, write one and run it in development, then record it here", &["reylo migrate new <name>", "reylo migrate dev", "reylo migrate history push"])
                .fix("To see which migrations it matches", &["reylo migrate deploy --dry-run"]);
            return Ok(Plan { problems: with_stamp(vec![problem]), ..Plan::default() });
        }
        // A first run is on an empty database: there is no history to say what else this one holds.
        if history.is_empty() && holds_a_schema(&live) {
            let problem = Problem::new("schema-without-history", "This database has a schema, and no migration history")
                .fix("Where the migrations describe it, record them, running nothing", &["reylo migrate history push"])
                .fix("Or start the project's migrations from it", &["reylo migrate init"]);
            return Ok(Plan { problems: with_stamp(vec![problem]), ..Plan::default() });
        }
        return Ok(Plan { problems: with_stamp(vec![]), pending, unchanged, leftovers, ran: applied_files.clone(), ..Plan::default() });
    };
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain(history.iter().map(|r| r.sql.as_str())).chain(schema_sql.as_deref()).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &texts).await;
    // A lone statement cut off before its row was finished did run, or did not: the database says
    // which, and one that landed is recorded, as the row it started says it ran.
    let started: Vec<String> = history.iter().filter(|r| !r.applied).map(|r| r.name.clone()).collect();
    let mut run = Run { live: &live, schema_sql: schema_sql.as_deref(), schema: shown(&opts.schema), applied_files: &applied_files, records, pending, started, landed: vec![], d };
    let result = check_in_shadows(&mut shadows, db, &mut run).await;
    shadows.finish().await;
    let problems = result?;
    Ok(Plan { problems, records: run.records, pending: run.pending, unchanged, landed: run.landed, leftovers, ran: applied_files.clone() })
}

/// What a check holds the shadows against.
struct Run<'a> {
    live: &'a [String],
    schema_sql: Option<&'a str>,
    /// schema.sql's path, as a refusal names it.
    schema: String,
    applied_files: &'a [Migration],
    records: Vec<Migration>,
    pending: Vec<Migration>,
    /// Migrations whose row is marked started: a lone statement cut off before its row was finished.
    started: Vec<String>,
    landed: Vec<String>,
    d: Dialect,
}

async fn check_in_shadows(shadows: &mut Shadows<'_>, target: &mut Db, run: &mut Run<'_>) -> Result<Vec<Problem>, Error> {
    let landed: Vec<Migration> = run.pending.iter().filter(|m| run.started.contains(&m.name)).cloned().collect();
    if !landed.is_empty() {
        let lane = shadows.lane();
        shadows.push(lane, run.applied_files.iter().chain(&run.records).chain(&landed).map(Step::of));
        if let Ok(Ok(lines)) = shadows.lines(lane).await {
            if crate::catalog::same_schema(run.live, &lines) {
                run.pending.retain(|m| !landed.contains(m));
                run.landed = landed.iter().map(|m| m.name.clone()).collect();
                run.records.extend(landed);
            }
        }
    }
    let run = &*run;
    // B: the target's history, as the files read now, then compared with the live target. With
    // migrations pending, the target's history is where the next check starts, so it is kept.
    let b_lane = shadows.lane();
    shadows.push(b_lane, run.applied_files.iter().chain(&run.records).map(Step::of));
    if !run.pending.is_empty() {
        shadows.mark(b_lane);
    }
    let checkpoint = match shadows.lines(b_lane).await? {
        Ok(lines) => lines,
        Err((m, error)) => return Ok(vec![Problem::new("replay-fails", format!("{m} fails when replayed in a shadow database")).note(error)]),
    };
    if !crate::catalog::same_schema(run.live, &checkpoint) {
        return drift(shadows, target, run, &checkpoint).await;
    }
    shadows.push(b_lane, run.pending.iter().map(Step::of));
    let b = match shadows.lines(b_lane).await? {
        Ok(lines) => lines,
        Err((m, error)) => return Ok(vec![Problem::new("history-replay-fails", format!("{m} fails on this database's history")).note(error)]),
    };

    let Some(schema_sql) = run.schema_sql else { return Ok(vec![]) };
    schema_problems(shadows, b_lane, &b, schema_sql, &run.schema).await
}

/// schema.sql held against `b`, what the migrations in `b_lane` make: where they differ, the SQL that
/// takes one to the other.
async fn schema_problems(shadows: &mut Shadows<'_>, b_lane: usize, b: &[String], schema_sql: &str, schema: &str) -> Result<Vec<Problem>, Error> {
    let mut problems = Vec::new();
    let a_lane = shadows.lane();
    shadows.push(a_lane, [Step::script("schema.sql", schema_sql)]);
    let a = match shadows.lines(a_lane).await? {
        Ok(lines) => lines,
        Err((_, error)) => {
            problems.push(Problem::new("schema-sql-fails", format!("{schema} doesn't run on an empty database")).note(error));
            return Ok(problems);
        }
    };
    if a != b {
        let (only_schema, only_migrations) = difference(&a, b);
        let problem = match (only_schema.is_empty(), only_migrations.is_empty()) {
            (false, true) => Problem::new("schema-sql-ahead", format!("{schema} has what no migration makes yet")).sql(&lane_plan(shadows, b_lane, a_lane, Some(schema_sql)).await.unwrap_or_default()).fix("Write it as a migration", &["reylo migrate new <name>"]),
            (true, false) => Problem::new("migrations-ahead", format!("The migrations make what {schema} doesn't declare")).sql(&lane_plan(shadows, a_lane, b_lane, None).await.unwrap_or_default()).note(format!("Add it to {schema}, or take it out of the migration that makes it.")),
            _ => Problem::new("schema-sql-disagrees", format!("{schema} and the migrations disagree"))
                .sql(&lane_plan(shadows, b_lane, a_lane, Some(schema_sql)).await.unwrap_or_default())
                .note(format!("Change {schema} or the migrations so that they agree."))
                .fix(format!("Then write what {schema} has and no migration makes"), &["reylo migrate new <name>"]),
        };
        problems.push(problem);
    }
    Ok(problems)
}

/// The refusal for a database that differs from its history: what changed, as SQL from what the
/// history makes to what the database has, and the ways out. Where the next pending migrations,
/// none of which changes data, make exactly the change, recording them is named.
async fn drift(shadows: &mut Shadows<'_>, target: &mut Db, run: &Run<'_>, checkpoint: &[String]) -> Result<Vec<Problem>, Error> {
    // Partitions made at run time are data, and comments aren't compared: no part of what differs.
    let checkpoint = crate::catalog::without_comments(&crate::catalog::without_missing_children(checkpoint, run.live));
    let live = crate::catalog::without_comments(&crate::catalog::without_extra_children(run.live, &checkpoint));
    let (only_history, only_live) = difference(&checkpoint, &live);
    if !run.records.is_empty() {
        let names = run.records.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ");
        let without = shadows.lane();
        shadows.push(without, run.applied_files.iter().map(Step::of));
        if let Ok(Ok(lines)) = shadows.lines(without).await {
            if crate::catalog::without_comments(&lines) == live {
                return Ok(vec![Problem::new("cannot-record", format!("This database doesn't have what {names} make, so they can't be recorded without running")).fix("Run them instead", &["reylo migrate deploy"])]);
            }
        }
    }
    let mut hint = Vec::new();
    for j in 1..=run.pending.len().min(3) {
        if !run.pending[..j].iter().all(|m| data_statements(&m.sql, run.d).is_empty()) {
            break;
        }
        let lane = shadows.lane();
        shadows.push(lane, run.applied_files.iter().chain(&run.records).chain(&run.pending[..j]).map(Step::of));
        if let Ok(Ok(lines)) = shadows.lines(lane).await {
            if crate::catalog::without_comments(&lines) == live {
                hint = run.pending[..j].iter().map(|m| m.name.clone()).collect();
                break;
            }
        }
    }
    let prefix = shadows.lane();
    shadows.push(prefix, run.applied_files.iter().chain(&run.records).map(Step::of));
    let mut sql = String::new();
    if let Ok(Ok(())) = shadows.open(prefix).await {
        shadows.taint(prefix);
        sql = plan_text(shadows.db(prefix), target, None).await.unwrap_or_default();
    }
    let mut problem = Problem::new("database-drift", CHANGED_OUTSIDE);
    problem = if sql.is_empty() { problem.lines(std::iter::once(String::new()).chain(changed_lines(&checkpoint, &live)).chain([String::new()])) } else { problem.sql(&sql) };
    if !hint.is_empty() {
        problem = problem.fix(format!("{} {} exactly this change: record it here, running nothing", hint.join(" and "), if hint.len() == 1 { "makes" } else { "make" }), &["reylo migrate history push"]);
    } else {
        problem = keep_fixes(problem, run.schema_sql.is_some().then_some(run.schema.as_str()), only_live.is_empty(), only_history.is_empty());
    }
    Ok(vec![problem])
}

/// The ways to keep a change made by hand on a database no development command runs on: written as
/// a migration in development, then recorded on it. `removal` and `addition` say what the change
/// is, for what to do to schema.sql, where the project has one.
fn keep_fixes(problem: Problem, schema: Option<&str>, removal: bool, addition: bool) -> Problem {
    let keep = match schema {
        Some(schema) if addition => format!("To keep it, add it to {schema} and write its migration in development"),
        Some(schema) if removal => format!("To keep it, take it out of {schema} and write its migration in development"),
        Some(schema) => format!("To keep it, make {schema} match and write its migration in development"),
        None => "To keep it, make the same change in development and write its migration there".to_string(),
    };
    problem.fix(keep, &["reylo migrate new <name>", "reylo migrate dev"]).fix("Then record it here, running nothing", &["reylo migrate history push"])
}

/// The SQL that takes the schema of `from` to that of `to`, as the diff engine plans it, written as
/// `create` writes it (from `declared`, the text that built `to`, where there is one); None where it
/// plans nothing for this database or finds no difference.
async fn plan_text(from: &mut Db, to: &mut Db, declared: Option<&str>) -> Option<String> {
    let mut target = crate::diff::read(to).await.ok()?;
    if let (Some(sql), Dialect::Postgres) = (declared, to.dialect()) {
        crate::diff::postgres::declare(to, &mut target, sql).await.ok()?;
    }
    let current = crate::diff::read(from).await.ok()?;
    // Partitions the database made at run time are data, not a difference.
    crate::diff::without_extra_children(&mut target, &current);
    let alterer = crate::diff::alterer(from).await.ok()?;
    let mut plan = crate::diff::plan(&current, &target, alterer.as_ref());
    style(from, &mut plan, &target).await.ok()?;
    (!plan.statements.is_empty()).then(|| plan.sql())
}

/// The plan written as `schema.sql` is: on Postgres, in lowercase, with the names in the first
/// schema of the search_path of `db`, the database it runs on, unqualified, in its statements and
/// in the comments that name what it loses or renames.
async fn style(db: &mut Db, plan: &mut crate::diff::Plan, target: &crate::diff::Schema) -> Result<(), Error> {
    if db.dialect() != Dialect::Postgres {
        return Ok(());
    }
    crate::diff::postgres::style(db, plan, &target.verbatim).await
}

/// `plan_text` between two lanes' databases, which the reads leave as no replay to keep.
async fn lane_plan(shadows: &mut Shadows<'_>, from: usize, to: usize, declared: Option<&str>) -> Option<String> {
    for lane in [from, to] {
        if !matches!(shadows.open(lane).await, Ok(Ok(()))) {
            return None;
        }
        shadows.taint(lane);
    }
    let mut target = crate::diff::read(shadows.db(to)).await.ok()?;
    if let (Some(sql), Dialect::Postgres) = (declared, shadows.db(to).dialect()) {
        crate::diff::postgres::declare(shadows.db(to), &mut target, sql).await.ok()?;
    }
    let current = crate::diff::read(shadows.db(from)).await.ok()?;
    let alterer = crate::diff::alterer(shadows.db(from)).await.ok()?;
    let mut plan = crate::diff::plan(&current, &target, alterer.as_ref());
    style(shadows.db(from), &mut plan, &target).await.ok()?;
    (!plan.statements.is_empty()).then(|| plan.sql())
}

// ---------------------------------------------------------------------------------------------
// The commands.

/// Whether the project's own SQL sets default privileges, so that shadows must not take the target's.
fn sets_default_privileges(opts: &Options) -> bool {
    static PRIVILEGES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\balter\s+default\s+privileges\b").expect("a valid pattern"));
    let schema = std::fs::read_to_string(&opts.schema).unwrap_or_default();
    PRIVILEGES.is_match(&schema) || read_migrations(&opts.dir).map(|(files, _)| files.iter().any(|f| PRIVILEGES.is_match(&f.sql))).unwrap_or(false)
}

/// The extensions the project's own files create, which its shadows leave to them.
fn created_extensions(opts: &Options) -> Vec<String> {
    static EXTENSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\bcreate\s+extension\s+(?:if\s+not\s+exists\s+)?"?([A-Za-z0-9_-]+)"?"#).expect("a valid pattern"));
    let mut texts = vec![std::fs::read_to_string(&opts.schema).unwrap_or_default()];
    texts.extend(read_migrations(&opts.dir).map(|(files, _)| files.into_iter().map(|f| f.sql).collect::<Vec<_>>()).unwrap_or_default());
    // What a comment says creates nothing: matched in the text with comments taken out.
    let uncommented = |t: &str| crate::sql::tokenize(t, Dialect::Postgres).iter().map(|token| token.text.clone()).collect::<Vec<_>>().join(" ");
    texts.iter().flat_map(|t| EXTENSION.captures_iter(&uncommented(t)).map(|c| c[1].to_lowercase()).collect::<Vec<_>>()).collect()
}

async fn open(opts: &Options) -> Result<(Db, Server, usize), Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let mut server = shadow_server(&mut db, opts).await?;
    let swept = server.sweep().await?;
    Ok((db, server, swept))
}

/// The server the shadows of `db` are made on, set up as the project's files say.
async fn shadow_server(db: &mut Db, opts: &Options) -> Result<Server, Error> {
    let mut server = Server::open(db, &opts.url, opts.shadow_url.as_deref()).await?;
    server.copy_default_privileges(!sets_default_privileges(opts));
    server.leave_extensions(created_extensions(opts));
    Ok(server)
}

/// Deploy's one shadow, in a project with no lock: where a pending migration sorts before one the
/// database ran, the two orders are replayed, and a difference refuses the deploy.
async fn late_on(db: &mut Db, opts: &Options, plan: &Plan) -> Result<Option<Problem>, Error> {
    let last = plan.ran.iter().map(|m| name_order(&m.name)).max();
    if !lock_free(opts) || !plan.pending.iter().any(|m| last.is_some_and(|last| name_order(&m.name) < last)) {
        return Ok(None);
    }
    let (files, _) = read_migrations(&opts.dir)?;
    let mut server = shadow_server(db, opts).await?;
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).collect();
    let mut shadows = Shadows::new(&mut server, &opts.url, &opts.root, &opts.cache, &texts).await;
    let problem = late_problem(&mut shadows, &plan.ran, &plan.pending, &files).await;
    shadows.finish().await;
    server.close().await;
    problem
}

fn print_problems(problems: &[Problem], log: Log<'_>) {
    if JSON.load(std::sync::atomic::Ordering::Relaxed) {
        REPORTED.lock().unwrap_or_else(|e| e.into_inner()).extend(problems.iter().cloned());
    }
    for (k, p) in problems.iter().enumerate() {
        if k > 0 {
            log("");
        }
        log(&paint("bold red", &format!("✗ {}", p.title)));
        for line in &p.lines {
            log(line);
        }
    }
}

/// What a run prints: a heading, once there is something to put under it, and one aligned row per
/// migration.
struct Rows {
    heading: String,
    width: usize,
    opened: bool,
}

impl Rows {
    fn new<'a>(heading: String, names: impl Iterator<Item = &'a str>) -> Rows {
        Rows { heading, width: names.map(|n| n.chars().count()).max().unwrap_or(0), opened: false }
    }

    fn open(&mut self, log: Log<'_>) {
        if !self.opened {
            if !self.heading.is_empty() {
                log(&self.heading);
            }
            self.opened = true;
        }
    }

    fn row(&mut self, log: Log<'_>, mark: &str, name: &str, what: &str) {
        self.open(log);
        log(&format!("  {mark} {name:<width$}  {what}", width = self.width));
    }
}

/// The refusal for a migrations directory that isn't there, on a database that ran migrations: a
/// typo in `dir`, which would otherwise read as every migration taken away.
fn missing_dir(opts: &Options, history: &[HistoryRow]) -> Option<Problem> {
    let ran = history.iter().filter(|r| r.applied).count();
    (!opts.dir.exists() && ran > 0).then(|| {
        Problem::new("migrations-dir-missing", format!("{} doesn't exist", shown(&opts.dir))).note(format!("This database ran {ran} migration{} from it: check dir in .reylo.yaml.", if ran == 1 { "" } else { "s" }))
    })
}

/// A migration a database ran whose file is gone: the text it ran is the way back.
fn missing_problem(name: &str, ran: &str, dir: &str) -> Problem {
    Problem::new("applied-migration-missing", format!("This database applied {name}, which isn't in {dir}"))
        .sql(ran)
        .fix("Write it back as it ran", &["reylo migrate history pull"])
        .fix("On a development database, undo it instead", &["reylo migrate dev"])
}

/// A migration edited after a database ran it.
fn edited_problem(name: &str, ran: &str, now: &str) -> Problem {
    Problem::new("applied-migration-edited", format!("{name} changed after this database applied it"))
        .lines(std::iter::once(String::new()).chain(line_diff(ran, now)).chain([String::new()]))
        .note("Write the change as a new migration.")
        .fix("Take back the text this database ran", &["reylo migrate history pull"])
        .fix("On a development database, run it again as it now reads", &["reylo migrate dev"])
}

/// Whether a database's catalog lines hold more than an empty database's do.
fn holds_a_schema(lines: &[String]) -> bool {
    const EMPTY: [&str; 5] = ["schema ", "grant schema ", "comment schema ", "extension ", crate::catalog::DEFAULT_PRIVILEGES];
    // A schema's owner too: a new database's `public` is `pg_database_owner`'s.
    lines.iter().any(|l| !EMPTY.iter().any(|kind| l.starts_with(kind)) && !(l.starts_with("owner ") && l.ends_with(" (schema)")))
}

/// Whether a migration that fails can leave part of what it did: on MySQL, whose statements commit as
/// they go, and a statement that runs outside a transaction.
fn may_leave_part(sql: &str, d: Dialect) -> bool {
    d == Dialect::Mysql || runs_alone(sql, d)
}

/// `reylo migrate deploy`: runs what is pending on a database that is as Reylo's last run left it, and
/// refuses one that isn't, running nothing. It needs no shadow database: `--dry-run` is what replays.
pub async fn deploy(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let result = deploy_on(&mut db, opts, log).await;
    db.close().await;
    result
}

/// What the lock says of schema.sql, with no database: whether its checksum is the one the lock
/// holds from the last time the migrations were shown to reach it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stamp {
    /// The project has no schema.sql: its migrations are its schema.
    NoSchema,
    /// No lock to hold a checksum.
    NoLock,
    /// A lock that can't be read (a merge conflict in it), which its own refusal names.
    Unreadable,
    /// The lock holds none: the migrations weren't shown to reach schema.sql when it was written.
    Unproven,
    Proven,
    /// schema.sql changed since: a change no migration makes yet, or formatting alone.
    Changed,
}

fn stamp(opts: &Options, schema_sql: Option<&str>) -> Stamp {
    let Some(sql) = schema_sql else { return Stamp::NoSchema };
    let l = match lock::read(&opts.lock) {
        lock::State::Lock(l) => l,
        lock::State::Missing => return Stamp::NoLock,
        _ => return Stamp::Unreadable,
    };
    let last = l.entries.last().map(|e| e.sum.clone()).unwrap_or_default();
    match l.schema {
        Some(sum) if sum == lock::schema_sum(&last, sql) => Stamp::Proven,
        Some(_) => Stamp::Changed,
        None => Stamp::Unproven,
    }
}

/// The refusal of a schema.sql whose text isn't the one the lock's checksum is of.
fn stamp_problem(opts: &Options) -> Problem {
    Problem::new("schema-sql-changed", format!("{} changed since the lock was written", shown(&opts.schema)))
        .note(format!("The lock holds a checksum of {} from when the migrations last matched it.", shown(&opts.schema)))
        .fix("If the schema changed, write its migration and run it", &["reylo migrate new <name>", "reylo migrate dev"])
        .fix("If only formatting or comments changed, check it again", &["reylo migrate dev"])
}

/// What `lint` and `status` say of the lock's checksum of schema.sql.
fn stamp_line(opts: &Options, stamped: Stamp) -> String {
    match stamped {
        Stamp::NoSchema => schema_line(opts),
        Stamp::Proven => format!("{} The migrations match {}", paint("green", "✓"), shown(&opts.schema)),
        Stamp::Changed => format!("{} {} changed since the lock was written: reylo migrate new <name>, then reylo migrate dev", paint("yellow", "!"), shown(&opts.schema)),
        Stamp::NoLock => paint("dim", &format!("· {} is checked against the migrations only with a database", shown(&opts.schema))),
        Stamp::Unreadable => paint("dim", &format!("· {} is checked once the lock can be read", shown(&opts.schema))),
        Stamp::Unproven => format!("{} The lock holds no checksum of {}: reylo migrate dev writes one once the migrations match it", paint("yellow", "!"), shown(&opts.schema)),
    }
}

/// What a check says of the declared schema: that the migrations reach it, or, where the project
/// has none, that its migrations are its schema.
fn schema_line(opts: &Options) -> String {
    if opts.schema.exists() {
        format!("{} The migrations match {}", paint("green", "✓"), shown(&opts.schema))
    } else {
        paint("dim", &format!("· No {}: the migrations are the schema", shown(&opts.schema)))
    }
}

/// What Reylo reads by name alone or not at all, listed under `heading` where there is any.
fn unmodeled_note(heading: &str, objects: &[String], log: Log<'_>) {
    if objects.is_empty() {
        return;
    }
    log(&format!("{} {heading}:", paint("yellow", "!")));
    for o in objects {
        log(&format!("    {o}"));
    }
}

fn swept_note(swept: usize, log: Log<'_>) {
    if swept > 0 {
        log(&paint("dim", &format!("· Dropped {swept} shadow database{} a run left behind", if swept == 1 { "" } else { "s" })));
    }
}

async fn deploy_on(db: &mut Db, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let _held = lock(db, opts).await?;
    let mut plan = check(db, None, opts).await?;
    if plan.ok() {
        plan.problems.extend(late_on(db, opts, &plan).await?);
    }
    if !plan.ok() {
        return Ok(refuse(&plan.problems, log));
    }
    let history = read_history(db).await?;
    if plan.unchanged.is_none() && history.iter().any(|r| r.applied) {
        log(&uncompared_line());
    }
    if plan.pending.is_empty() {
        // A fingerprint that can't be compared is stored again, for the next run to compare.
        if plan.unchanged.is_none() {
            if let Some(last) = history.iter().rfind(|r| r.applied) {
                ensure_history(db).await?;
                store_fingerprint(db, &last.name).await?;
            }
        }
        log(&format!("{} Up to date", paint("green", "✓")));
        return Ok(0);
    }
    ensure_history(db).await?;
    drop_leftovers(db, &plan.leftovers, log).await?;
    let started: Vec<&str> = history.iter().filter(|r| !r.applied).map(|r| r.name.as_str()).collect();
    // Rows marked started belong to pending migrations: they are written again below.
    db.exec(&format!("delete from {HISTORY_TABLE} where applied_at is null")).await?;
    let mut rows = Rows::new(format!("Applying to {}", shown_url(&opts.url)), plan.pending.iter().map(|m| m.name.as_str()));
    let d = db.dialect();
    let mut code = 0;
    let mut applied: Vec<serde_json::Value> = Vec::new();
    for m in &plan.pending {
        let at = std::time::Instant::now();
        match run(db, m, true).await {
            // Its row holds the fingerprint it left, so that a run cut off after it, or a migration
            // after it that fails and leaves part of itself, is seen by the next run.
            Ok(()) => {
                let ms = at.elapsed().as_millis();
                rows.row(log, &paint("green", "✓"), &m.name, &format!("applied {}", paint("dim", &format!("· {ms} ms"))));
                applied.push(serde_json::json!({ "name": m.name, "state": "applied", "ms": ms as u64 }));
            }
            Err(e) => {
                failed(&mut rows, m, &e, d, log);
                applied.push(serde_json::json!({ "name": m.name, "state": "failed", "error": e.0 }));
                if started.contains(&m.name.as_str()) {
                    log("  A run before this one was cut off here, so its change may be in place: reylo migrate deploy --dry-run says, and reylo migrate history push records it.");
                } else if !may_leave_part(&m.sql, d) {
                    log("  Fix it, then run reylo migrate dev, which locks the fix, and deploy again.");
                }
                code = 1;
                break;
            }
        }
    }
    set_result(serde_json::json!({ "migrations": applied }));
    if code == 0 {
        log(&format!("{} Applied {} migration{}", paint("green", "✓"), plan.pending.len(), if plan.pending.len() == 1 { "" } else { "s" }));
    }
    Ok(code)
}

/// A failed migration's row and its error; on MySQL, or for a statement that runs alone, what of it
/// may be in place.
fn failed(rows: &mut Rows, m: &Migration, e: &Error, d: Dialect, log: Log<'_>) {
    let partly = may_leave_part(&m.sql, d);
    rows.row(log, &paint("bold red", "✗"), &m.name, if partly { "failed" } else { "failed, and nothing of it was applied" });
    log(&format!("      {e}"));
    if partly && d == Dialect::Mysql {
        log("  MySQL commits each statement, so its first statements may be in place: reylo migrate deploy --dry-run says what is.");
    }
}

/// What a command says where the fingerprint its last run stored can't be compared.
fn uncompared_line() -> String {
    paint("dim", "· This database can't be compared with what its last migration left: reylo migrate deploy --dry-run can")
}

/// `reylo migrate deploy --dry-run`: everything `deploy` checks, the database against a replay of its
/// history, what is pending tried in a shadow, and the migrations against schema.sql; runs nothing,
/// and writes nothing to the database.
pub async fn dry_run(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    swept_note(swept, log);
    let unmodeled = crate::catalog::unmodeled(&mut db).await;
    let result = check(&mut db, Some(&mut server), opts).await;
    let result = match result {
        Ok(mut plan) if plan.ok() && lock_free(opts) => {
            let (files, _) = read_migrations(&opts.dir)?;
            let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).collect();
            let mut shadows = Shadows::new(&mut server, &opts.url, &opts.root, &opts.cache, &texts).await;
            let late = late_problem(&mut shadows, &plan.ran, &plan.pending, &files).await;
            shadows.finish().await;
            late.map(|late| {
                plan.problems.extend(late);
                plan
            })
        }
        other => other,
    };
    db.close().await;
    server.close().await;
    let plan = result?;
    unmodeled_note("This database has what Reylo doesn't fully compare, so these checks can't see every change to it", &unmodeled?, log);
    if !plan.ok() {
        print_problems(&plan.problems, log);
        return Ok(1);
    }
    if !plan.ran.is_empty() {
        log(&format!("{} This database is as its migrations left it", paint("green", "✓")));
    }
    if plan.unchanged == Some(false) {
        log(&format!("{} reylo migrate deploy refuses this database until its fingerprint is stored again: reylo migrate history push", paint("yellow", "!")));
    }
    log(&schema_line(opts));
    for x in &plan.leftovers {
        log(&format!("{} {} was cut off and left {} invalid: reylo migrate deploy drops it, then runs {} again", paint("yellow", "!"), x.migration, x.index, x.migration));
    }
    for name in &plan.landed {
        log(&format!("{} {name} was cut off, and its change is in place: reylo migrate history push records it", paint("yellow", "!")));
    }
    match plan.pending.len() {
        0 => log(&format!("{} Nothing is pending", paint("green", "✓"))),
        n => {
            log(&format!("{} {n} pending, each tried on a shadow database:", paint("green", "✓")));
            for m in &plan.pending {
                log(&format!("    {} {}", paint("dim", "·"), m.name));
            }
        }
    }
    Ok(0)
}

/// `reylo migrate lint`: every check that needs no database. The files: their names, and with the
/// URL's dialect, whether each runs atomically and replays in a shadow. The lock against them: every
/// entry's file, chained, in order, and no draft; and schema.sql against the checksum the lock holds
/// from the last time the migrations were proven to reach it.
pub fn lint(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (files, strays) = read_migrations(&opts.dir)?;
    let schema_sql = std::fs::read_to_string(&opts.schema).ok();
    let mut problems = match Dialect::of_url(&opts.url) {
        Ok(d) => file_problems(opts, &files, &strays, schema_sql.as_deref(), d),
        Err(_) => file_problems(opts, &[], &strays, None, Dialect::Postgres),
    };
    problems.extend(lock_problems(opts, &files));
    let stamped = stamp(opts, schema_sql.as_deref());
    if problems.is_empty() && stamped == Stamp::Changed {
        problems.push(stamp_problem(opts));
    }
    if problems.is_empty() && stamped == Stamp::Unproven {
        problems.push(
            Problem::new("schema-sql-unproven", format!("{} holds no checksum of {}", shown(&opts.lock), shown(&opts.schema)))
                .note(format!("The migrations didn't match {} when the lock was written.", shown(&opts.schema)))
                .fix("Write what they don't make as a migration, and run it in development, which writes the checksum", &["reylo migrate new <name>", "reylo migrate dev"]),
        );
    }
    if !problems.is_empty() {
        print_problems(&problems, log);
        return Ok(1);
    }
    if lock_free(opts) {
        log(&format!("{} {} migration{}, each named and runnable", paint("green", "✓"), files.len(), if files.len() == 1 { "" } else { "s" }));
        log(&no_lock_line());
    } else {
        log(&lock_line(opts));
    }
    log(&stamp_line(opts, stamped));
    if Dialect::of_url(&opts.url).is_err() {
        log(&paint("dim", "· With no database URL, whether each migration runs atomically wasn't checked"));
    }
    Ok(0)
}

/// `reylo db wipe`: deletes every row of the project's tables, and nothing else: the schema and
/// the history stay, and identities and sequences start again. The tables are those the comparison
/// reads, so a platform's own (Supabase's auth.users) keep theirs.
pub async fn db_wipe(opts: &Options, in_ci: bool, log: Log<'_>) -> Result<i32, Error> {
    if in_ci {
        return Ok(refused_in_ci("reylo db wipe", log));
    }
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let had = table_rows(&mut db).await.map(|counts| counts.values().filter(|n| **n > 0).count());
    let result = wipe_rows_on(&mut db).await;
    db.close().await;
    let tables = result?;
    let had = match had? {
        n if n == tables => String::new(),
        0 => " (none had rows)".to_string(),
        n => format!(" ({n} had rows)"),
    };
    log(&format!("{} Deleted every row of {} table{} on {}{had}; the schema and the history stay", paint("green", "✓"), tables, if tables == 1 { "" } else { "s" }, shown_url(&opts.url)));
    Ok(0)
}

async fn wipe_rows_on(db: &mut Db) -> Result<usize, Error> {
    let d = db.dialect();
    let names: Vec<String> = match d {
        Dialect::Postgres => {
            let schema = crate::diff::read(db).await?;
            schema.objects.values().filter(|o| o.kind == Some(crate::diff::Kind::Table) && !o.extra.iter().any(|l| l.starts_with("shape foreign"))).filter_map(|o| o.id.strip_prefix("table ").map(String::from)).collect()
        }
        Dialect::Mysql => db
            .rows(&format!("select table_name as n from information_schema.tables where table_schema = database() and table_type = 'BASE TABLE' and table_name <> '{HISTORY_TABLE}'"))
            .await?
            .iter()
            .map(|r| format!("`{}`", r.text("n").replace('`', "``")))
            .collect(),
        // A virtual table's own tables (an FTS5 index's, an R*Tree's) are its to empty: deleting
        // from them directly breaks it.
        Dialect::Sqlite => db
            .rows(&format!("select name as n from pragma_table_list where schema = 'main' and type in ('table', 'virtual') and name not like 'sqlite_%' and name <> '{HISTORY_TABLE}'"))
            .await?
            .iter()
            .map(|r| format!("\"{}\"", r.text("n").replace('"', "\"\"")))
            .collect(),
    };
    if names.is_empty() {
        return Ok(0);
    }
    match d {
        Dialect::Postgres => db.exec(&format!("truncate table {} restart identity", names.join(", "))).await?,
        Dialect::Mysql => {
            db.exec("set foreign_key_checks = 0").await?;
            let mut result = Ok(());
            for n in &names {
                result = db.exec(&format!("truncate table {n}")).await;
                if result.is_err() {
                    break;
                }
            }
            db.exec("set foreign_key_checks = 1").await?;
            result?
        }
        Dialect::Sqlite => {
            db.exec("pragma foreign_keys = off").await?;
            // A full-text table whose content is another table's, or none, is emptied by its own
            // command, after the tables (its content among them) are: deleting from it reads the
            // content it indexes, and a contentless one refuses a delete.
            let virtual_sql: std::collections::BTreeMap<String, String> = db
                .rows("select name, sql from sqlite_schema where type = 'table' and sql like 'create virtual table%'")
                .await?
                .iter()
                .map(|r| (format!("\"{}\"", r.text("name").replace('"', "\"\"")), r.text("sql").to_string()))
                .collect();
            let module = regex::Regex::new(r"(?i)\busing\s+(\w+)").unwrap();
            let content = regex::Regex::new(r#"(?i)\bcontent\s*=\s*(?:'([^']*)'|"([^"]*)"|(\w+))"#).unwrap();
            let mut sql = String::new();
            let mut after = String::new();
            for n in &names {
                let Some(text) = virtual_sql.get(n) else {
                    sql.push_str(&format!("delete from {n};"));
                    continue;
                };
                let fts = module.captures(text).map(|c| c[1].to_lowercase()).filter(|m| m.starts_with("fts"));
                let of = content.captures(text).map(|c| c.get(1).or(c.get(2)).or(c.get(3)).map_or("", |m| m.as_str()).to_string());
                match (fts.as_deref(), of.as_deref()) {
                    (Some("fts5"), Some(_)) => after.push_str(&format!("insert into {n}({n}) values ('delete-all');")),
                    (Some(_), Some("")) => after.push_str(&format!("drop table {n}; {text};")),
                    (Some(_), Some(_)) => after.push_str(&format!("insert into {n}({n}) values ('rebuild');")),
                    _ => sql.push_str(&format!("delete from {n};")),
                }
            }
            sql.push_str(&after);
            let has_sequence = db.rows("select count(*) > 0 as ok from sqlite_schema where name = 'sqlite_sequence'").await?.first().is_some_and(|r| r.flag("ok"));
            let reset = if has_sequence { "delete from sqlite_sequence;" } else { "" };
            let result = db.script(&format!("begin; {sql} {reset} commit;")).await;
            if result.is_err() {
                let _ = db.exec("rollback").await;
            }
            db.exec("pragma foreign_keys = on").await?;
            result?
        }
    }
    Ok(names.len())
}

/// `reylo migrate history push`: rewrites the database's history to the migrations its schema
/// already has, running nothing: the longest run of migrations, in their order, whose replay in a
/// shadow database equals the live database. Those after it stay pending, and so do the migrations
/// with data statements at its end, which a schema can't show ran. For a database changed by hand
/// (a hotfix, then written as a migration), and one whose history was lost or kept by another tool.
pub async fn history_push(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    swept_note(swept, log);
    let result = history_push_on(&mut db, &mut server, opts, log).await;
    db.close().await;
    server.close().await;
    result
}

async fn history_push_on(db: &mut Db, server: &mut Server, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let _held = lock(db, opts).await?;
    let d = db.dialect();
    let (files, strays) = read_migrations(&opts.dir)?;
    let before_rows = read_history(db).await?;
    let mut problems = file_problems(opts, &files, &strays, None, d);
    problems.extend(missing_dir(opts, &before_rows));
    if !problems.is_empty() {
        return Ok(refuse(&problems, log));
    }
    // The history is the lock's; its drafts are development's alone.
    let problems: Vec<Problem> = lock_problems(opts, &files).into_iter().filter(|p| !p.is_drafts()).collect();
    if !problems.is_empty() {
        return Ok(refuse(&problems, log));
    }
    let schema_sql = std::fs::read_to_string(&opts.schema).ok();
    let l = read_lock(opts).unwrap_or_default();
    let order: Vec<Migration> = if lock_free(opts) { files.clone() } else { l.entries.iter().filter_map(|e| files.iter().find(|f| f.name == e.name).cloned()).collect() };
    let live = crate::catalog::without_comments(&catalog(db).await?);
    let texts: Vec<&str> = order.iter().map(|f| f.sql.as_str()).chain(schema_sql.as_deref()).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &texts).await;
    // Everything agrees but the history it writes: the migrations reach schema.sql.
    let all = shadows.lane();
    shadows.push(all, order.iter().map(Step::of));
    let agreement = match shadows.lines(all).await {
        Ok(Ok(made)) => match schema_sql.as_deref() {
            Some(sql) => schema_problems(&mut shadows, all, &made, sql, &shown(&opts.schema)).await,
            None => Ok(vec![]),
        },
        Ok(Err((m, error))) => Ok(vec![Problem::new("replay-fails", format!("{m} fails when replayed in a shadow database")).note(error)]),
        Err(e) => Err(e),
    };
    let agreement = match agreement {
        Ok(problems) => problems,
        Err(e) => {
            shadows.finish().await;
            return Err(e);
        }
    };
    if !agreement.is_empty() {
        shadows.finish().await;
        return Ok(refuse(&agreement, log));
    }
    // The replay after each migration, from none: the longest run that equals the database, and
    // else the one closest to it.
    let replayed = async {
        let lane = shadows.lane();
        if let Err((_, e)) = shadows.open(lane).await? {
            return Ok(Err(e));
        }
        shadows.taint(lane);
        let mut matches: Vec<usize> = Vec::new();
        let mut closest: (usize, usize, Vec<String>) = (usize::MAX, 0, vec![]);
        for k in 0..=order.len() {
            let now = crate::catalog::without_comments(&catalog(shadows.db(lane)).await?);
            if crate::catalog::without_extra_children(&live, &now) == now {
                matches.push(k);
            } else {
                let (a, b) = difference(&now, &live);
                if a.len() + b.len() < closest.0 {
                    closest = (a.len() + b.len(), k, now);
                }
            }
            if k == order.len() {
                break;
            }
            if let Err(e) = run(shadows.db(lane), &order[k], false).await {
                return Ok(Err(format!("{} fails when replayed in a shadow database: {e}", order[k].name)));
            }
        }
        Ok::<_, Error>(Ok((matches, closest)))
    }
    .await;
    let (matches, closest) = match replayed {
        Ok(Ok(found)) => found,
        Ok(Err(e)) => {
            shadows.finish().await;
            return Ok(refuse(&[Problem::new("replay-fails", "The migrations can't be replayed").note(e)], log));
        }
        Err(e) => {
            shadows.finish().await;
            return Err(e);
        }
    };
    let Some(mut n) = matches.last().copied() else {
        // The closest run, and the change from what it makes to this database, as SQL.
        let (_, k, lines) = closest;
        let lane = shadows.lane();
        shadows.push(lane, order[..k].iter().map(Step::of));
        let mut sql = None;
        if let Ok(Ok(())) = shadows.open(lane).await {
            shadows.taint(lane);
            sql = plan_text(shadows.db(lane), db, None).await;
        }
        shadows.finish().await;
        let closest = if k == 0 { "The closest is with no migration run, which differs by:".to_string() } else { format!("The closest is after {}, which differs by:", order[k - 1].name) };
        let problem = Problem::new("history-push-no-match", "No run of the migrations makes this database's schema").note(closest);
        let problem = match sql {
            Some(sql) => problem.sql(&sql),
            None => problem.lines(std::iter::once(String::new()).chain(changed_lines(&lines, &live)).chain([String::new()])),
        };
        let mut problem = match schema_sql {
            Some(_) => problem.fix(format!("To keep it, write it into {}, then its migration", shown(&opts.schema)), &["reylo migrate new <name>"]),
            None => problem.fix("To keep it, write it as a migration", &["reylo migrate new <name>"]),
        };
        if k == 0 {
            problem = problem.fix("Or start a new project from this database, in a directory of its own", &["reylo migrate init"]);
        }
        return Ok(refuse(&[problem], log));
    };
    shadows.finish().await;
    // A migration at the end of the run that changes data and nothing a schema shows can't be told
    // to have run: it stays pending, to run.
    // One the history says ran, as written, stays recorded.
    let ran = |m: &Migration| before_rows.iter().any(|r| r.applied && r.name == m.name && same_statements(&r.sql, &m.sql, d));
    while n > 0 && !data_statements(&order[n - 1].sql, d).is_empty() && matches.contains(&(n - 1)) && !ran(&order[n - 1]) {
        n -= 1;
    }
    let recorded = &order[..n];
    let before = before_rows;
    ensure_history(db).await?;
    // The schema it records, which running nothing leaves as it is, on the last row.
    let lines = catalog(db).await?;
    let value = fingerprint_of(db, &lines).await?;
    // The rows that stay as they are, with the time each ran: the history's start, as far as it
    // agrees with what is recorded, in its order. The rest is written again, in order, after them.
    let kept = before.iter().zip(recorded).take_while(|(r, f)| r.applied && r.name == f.name && same_statements(&r.sql, &f.sql, d)).count();
    db.exec(if d == Dialect::Mysql { "start transaction" } else { "begin" }).await?;
    let written = async {
        let names: Vec<String> = recorded[..kept].iter().map(|f| crate::db::literal(&f.name)).collect();
        if names.is_empty() {
            db.exec(&format!("delete from {HISTORY_TABLE}")).await?;
        } else {
            db.exec(&format!("delete from {HISTORY_TABLE} where name not in ({})", names.join(", "))).await?;
        }
        for f in &recorded[kept..] {
            insert_row(db, f, true, None).await?;
        }
        if let Some(last) = recorded.last() {
            set_fingerprint(db, &last.name, &value).await?;
        }
        db.exec("commit").await
    }
    .await;
    if let Err(e) = written {
        let _ = db.exec("rollback").await;
        return Err(e);
    }
    if let Some(last) = recorded.last() {
        store_catalog(db, &last.name, &lines).await?;
    }
    let added: Vec<&Migration> = recorded.iter().filter(|f| !before.iter().any(|r| r.applied && r.name == f.name && same_statements(&r.sql, &f.sql, d))).collect();
    let taken: Vec<&HistoryRow> = before.iter().filter(|r| !recorded.iter().any(|f| f.name == r.name && same_statements(&r.sql, &f.sql, d))).collect();
    let mut rows = Rows::new(format!("Recording on {}", shown_url(&opts.url)), added.iter().map(|f| f.name.as_str()).chain(taken.iter().map(|r| r.name.as_str())));
    for f in &added {
        let data = data_statements(&f.sql, d);
        if data.is_empty() {
            rows.row(log, &paint("green", "+"), &f.name, "recorded: already in place");
        } else {
            rows.row(log, &paint("green", "+"), &f.name, &format!("recorded: {}", paint("yellow", &format!("its data statements ({}) may not have run here", data.join(", ")))));
        }
    }
    for r in &taken {
        rows.row(log, &paint("red", "-"), &r.name, "taken out of the history");
    }
    let count = |n: usize| format!("{n} migration{}", if n == 1 { "" } else { "s" });
    if added.is_empty() && taken.is_empty() {
        log(&format!("{} The history already matches this database: {}", paint("green", "✓"), count(recorded.len())));
    } else if added.is_empty() {
        log(&format!("{} The history now lists {}", paint("green", "✓"), count(recorded.len())));
    } else {
        log(&format!("{} Recorded {}, running nothing", paint("green", "✓"), count(added.len())));
    }
    let pending = &order[n..];
    if !pending.is_empty() {
        log(&paint("dim", &format!("· {} pending, which reylo migrate deploy runs: {}", pending.len(), pending.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", "))));
    }
    Ok(0)
}

/// `reylo migrate history pull`: writes each migration the database ran into its file, as it ran
/// it, from the history table, which keeps every applied migration's text: a file deleted is written
/// again, and one that reads differently to the lock is written over. A file that differs in comments
/// alone is left as it is, and so is a file the database hasn't run. The lock then lists what it ran,
/// in its order, before what the lock already listed that it hasn't run: a lock lost or merged wrong
/// is written again, and a deploy still to reach this database stays locked. Changes no database, and
/// refuses one whose tables aren't what its history makes, whose files would not make it either.
pub async fn history_pull(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    swept_note(swept, log);
    let result = history_as_it_stands(&mut db, &mut server, opts).await;
    db.close().await;
    server.close().await;
    let (history, problems) = result?;
    if !problems.is_empty() {
        return Ok(refuse(&problems, log));
    }
    let (files, _) = read_migrations(&opts.dir)?;
    let ran: Vec<&HistoryRow> = history.iter().filter(|r| r.applied).collect();
    let mut rows = Rows::new(format!("Pulling from {}", shown_url(&opts.url)), ran.iter().map(|r| r.name.as_str()));
    for r in &ran {
        let path = opts.dir.join(&r.name);
        let what = match files.iter().find(|f| f.name == r.name) {
            None => "written: it wasn't here",
            // By the lock's own reading, so that what the lock refuses as edited is written back.
            Some(f) if lock::canonical(&normalized(&f.sql)) != lock::canonical(&normalized(&r.sql)) => "written over: it differed from what ran",
            Some(_) => continue,
        };
        std::fs::create_dir_all(&opts.dir)?;
        std::fs::write(&path, &r.sql)?;
        rows.row(log, &paint("green", "↓"), &r.name, what);
    }
    if !rows.opened {
        log(&format!("{} {} holds every migration this database ran, as it ran it", paint("green", "✓"), shown(&opts.dir)));
    }
    let left: Vec<&str> = files.iter().filter(|f| !ran.iter().any(|r| r.name == f.name)).map(|f| f.name.as_str()).collect();
    if !left.is_empty() {
        log(&paint("dim", &format!("Not run on this database, and left as they are: {}", left.join(", "))));
    }
    let before = match lock::read(&opts.lock) {
        lock::State::Lock(l) => Some(l),
        _ => None,
    };
    let mut texts: Vec<(String, String)> = ran.iter().map(|r| (r.name.clone(), normalized(&r.sql))).collect();
    for e in before.iter().flat_map(|l| &l.entries) {
        if let Some(f) = files.iter().find(|f| f.name == e.name && !ran.iter().any(|r| r.name == f.name)) {
            texts.push((f.name.clone(), normalized(&f.sql)));
        }
    }
    let entries = lock::chain(texts.iter().map(|(n, t)| (n.as_str(), t.as_str())));
    // A project with no lock keeps none: reylo migrate lock writes one.
    if !lock_free(opts) && before.as_ref().map(|l| &l.entries) != Some(&entries) {
        // What schema.sql was proven against went with the old chain's last line.
        let schema = before.filter(|l| l.entries.last() == entries.last()).and_then(|l| l.schema);
        lock::write(&opts.lock, &lock::Lock { schema, entries })?;
        log(&format!("{} Wrote {}", paint("green", "✓"), shown(&opts.lock)));
    }
    Ok(0)
}

/// `reylo migrate status`: what this database ran, what's pending and what changed, read from the
/// database and the files alone, with no shadow and nothing written. It exits 0 whatever it finds:
/// `lint` and `deploy --dry-run` are the checks that decide.
pub async fn status(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    // A SQLite file that isn't there is no database to read, and opening it would make one.
    if Dialect::of_url(&opts.url)? == Dialect::Sqlite {
        let path = crate::db::sqlite_path(&opts.url, &opts.root);
        if path != ":memory:" && !Path::new(&path).exists() {
            return Err(Error(format!("no SQLite database at {path}")));
        }
    }
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let result = status_on(&mut db, opts, log).await;
    db.close().await;
    result
}

async fn status_on(db: &mut Db, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let d = db.dialect();
    let (files, strays) = read_migrations(&opts.dir)?;
    let history = if has_history(db).await? { read_history(db).await? } else { vec![] };
    let times = applied_times(db).await?;
    let order = run_order(opts, &files);
    let names: Vec<String> = history.iter().map(|r| r.name.clone()).chain(order.iter().filter(|f| !history.iter().any(|r| r.name == f.name)).map(|f| f.name.clone())).collect();
    let mut rows = Rows::new(format!("Migrations on {}", shown_url(&opts.url)), names.iter().map(String::as_str));
    // The same, for --json: each migration's state, and when it was applied.
    let mut listed: Vec<serde_json::Value> = Vec::new();
    for r in &history {
        // To the minute, in UTC, for a person.
        let at = times.get(&r.name).map(|t| format!("applied {} {} UTC", &t[..10], &t[11..16])).unwrap_or_else(|| "applied".into());
        let state = match (r.applied, files.iter().find(|f| f.name == r.name)) {
            (false, _) => {
                rows.row(log, &paint("yellow", "!"), &r.name, "cut off before it finished: reylo migrate deploy --dry-run says whether it landed");
                "cut-off"
            }
            (true, None) => {
                rows.row(log, &paint("yellow", "!"), &r.name, &format!("{at}, and not in {}", shown(&opts.dir)));
                "missing"
            }
            (true, Some(f)) if !same_statements(&f.sql, &r.sql, d) => {
                rows.row(log, &paint("yellow", "!"), &r.name, &format!("{at}, and changed since"));
                "edited"
            }
            (true, Some(_)) => {
                rows.row(log, &paint("green", "✓"), &r.name, &paint("dim", &at));
                "applied"
            }
        };
        listed.push(serde_json::json!({ "name": r.name, "state": state, "applied_at": times.get(&r.name) }));
    }
    for f in order.iter().filter(|f| !history.iter().any(|r| r.name == f.name)) {
        rows.row(log, &paint("dim", "·"), &f.name, "pending");
        listed.push(serde_json::json!({ "name": f.name, "state": "pending" }));
    }
    if names.is_empty() {
        // A new project: nothing to say of a lock or a schema.sql no migration makes yet.
        log(&if opts.schema.exists() {
            format!("No migrations yet. Declare tables in {}, then reylo migrate new <name>.", shown(&opts.schema))
        } else {
            "No migrations yet: reylo migrate new <name> writes the first from this database.".to_string()
        });
        set_result(serde_json::json!({ "migrations": [], "database": "no-history", "changes": [], "lock": if lock_free(opts) { "none" } else { "ok" }, "schema_sql": if opts.schema.exists() { "matched" } else { "none" } }));
        return Ok(0);
    }
    if let Some(problem) = stray_problem(opts, &strays) {
        log(&format!("{} {}", paint("yellow", "!"), problem.title));
    }
    // Whether anything changed the database since Reylo last ran on it, as deploy would see it.
    let stored = history.iter().rfind(|r| r.applied).and_then(|r| r.fingerprint.clone());
    let mut database = "unknown";
    let mut changes: Vec<String> = Vec::new();
    match stored.as_deref() {
        _ if history.is_empty() => database = "no-history",
        Some(PENDING) => database = "cut-off",
        _ => {}
    }
    match stored.as_deref() {
        _ if history.is_empty() => {}
        Some(PENDING) => log(&format!("{} A run was cut off before it stored this database's fingerprint: reylo migrate deploy --dry-run compares it with its migrations", paint("yellow", "!"))),
        Some(stored) => {
            let now = fingerprint(db).await?;
            let how = |f: &str| f.rsplit_once(':').map(|(how, _)| how.to_string());
            if how(stored) != how(&now) {
                log(&uncompared_line());
            } else if stored == now {
                database = "unchanged";
                log(&format!("{} Unchanged since its last migration", paint("green", "✓")));
            } else {
                database = "changed";
                log(&format!("{} {CHANGED_OUTSIDE}", paint("yellow", "!")));
                if let Some(last) = history.iter().rfind(|r| r.applied) {
                    if let Some(stored) = stored_catalog(db, &last.name).await? {
                        for line in changed_lines(&stored, &fingerprinted(&catalog(db).await?)) {
                            changes.push(line.trim().to_string());
                            log(&line);
                        }
                    }
                }
            }
        }
        None => log(&uncompared_line()),
    }
    let lock_state: serde_json::Value = match lock::read(&opts.lock) {
        lock::State::Missing => {
            log(&no_lock_line());
            "none".into()
        }
        _ => {
            let problems = lock_problems(opts, &files);
            if problems.is_empty() {
                log(&lock_line(opts));
            }
            for p in &problems {
                log(&format!("{} {}", paint("yellow", "!"), p.title));
            }
            if problems.is_empty() { "ok".into() } else { problems.iter().map(|p| p.json()).collect() }
        }
    };
    let stamped = stamp(opts, std::fs::read_to_string(&opts.schema).ok().as_deref());
    log(&stamp_line(opts, stamped));
    let schema = match stamped {
        Stamp::NoSchema => "none",
        Stamp::Proven => "matched",
        Stamp::Changed => "changed",
        Stamp::NoLock | Stamp::Unproven | Stamp::Unreadable => "unproven",
    };
    set_result(serde_json::json!({ "migrations": listed, "database": database, "changes": changes, "lock": lock_state, "schema_sql": schema }));
    Ok(0)
}

/// When each applied migration ran, by name, in UTC: ISO 8601 to the second.
async fn applied_times(db: &mut Db) -> Result<std::collections::HashMap<String, String>, Error> {
    if !has_history(db).await? {
        return Ok(Default::default());
    }
    // As seconds since the epoch: MySQL's datetime is in the session's time zone, which this reads.
    let at = match db.dialect() {
        Dialect::Postgres => "cast(floor(extract(epoch from applied_at)) as bigint)",
        Dialect::Mysql => "floor(unix_timestamp(applied_at))",
        Dialect::Sqlite => "cast(strftime('%s', applied_at) as integer)",
    };
    let rows = db.rows(&format!("select name, {at} as at from {HISTORY_TABLE} where applied_at is not null")).await?;
    Ok(rows
        .iter()
        .filter_map(|r| {
            let stamp = time::stamp(r.text("at").parse().ok()?);
            Some((r.text("name").to_string(), format!("{}-{}-{}T{}:{}:{}Z", &stamp[0..4], &stamp[4..6], &stamp[6..8], &stamp[8..10], &stamp[10..12], &stamp[12..14])))
        })
        .collect())
}

/// What a command says in a project with no lock: the order is the files' names', and nothing
/// holds every database to one.
fn no_lock_line() -> String {
    paint("dim", "· No lock: migrations run in name order (reylo migrate lock writes one)")
}

/// What `lint` and `status` say of a lock that agrees with the files.
fn lock_line(opts: &Options) -> String {
    format!("{} {} matches the migration files", paint("green", "✓"), shown(&opts.lock))
}

/// `reylo migrate lock`: writes the lock from this database's history, the order it ran its
/// migrations in, with schema.sql's checksum where the shadows show the migrations reach it. Every
/// migration it ran must be in the migrations as it ran it. This is how a project that deleted its
/// lock takes linear history back.
pub async fn lock_from_history(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    swept_note(swept, log);
    let result = lock_from_history_on(&mut db, &mut server, opts, log).await;
    db.close().await;
    server.close().await;
    result
}

async fn lock_from_history_on(db: &mut Db, server: &mut Server, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let d = db.dialect();
    // A lock lists what other databases ran too: written over from this one's history, they'd lose it.
    if !lock_free(opts) {
        return Ok(refuse(&[Problem::new("lock-exists", format!("{} exists already", shown(&opts.lock))).note("It lists what every database runs, which this one may not have run yet.").fix("To write it again from this database's history, delete it, then", &["reylo migrate lock"])], log));
    }
    let _held = lock(db, opts).await?;
    let (files, _) = read_migrations(&opts.dir)?;
    let history = if has_history(db).await? { read_history(db).await? } else { vec![] };
    let mut problems = Vec::new();
    for r in history.iter().filter(|r| r.applied) {
        match files.iter().find(|f| f.name == r.name) {
            None => problems.push(missing_problem(&r.name, &r.sql, &shown(&opts.dir))),
            Some(f) if !same_statements(&f.sql, &r.sql, d) => problems.push(edited_problem(&r.name, &r.sql, &f.sql)),
            Some(_) => {}
        }
    }
    if history.iter().any(|r| !r.applied) {
        problems.push(Problem::new("run-cut-off", "A run on this database was cut off before it finished").fix("Finish it first", &["reylo migrate dev"]));
    }
    if !problems.is_empty() {
        return Ok(refuse(&problems, log));
    }
    let schema_sql = std::fs::read_to_string(&opts.schema).ok();
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain(schema_sql.as_deref()).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &texts).await;
    let next = locked(&mut shadows, db, opts, &files, schema_sql.as_deref()).await.map(|(next, _)| next);
    shadows.finish().await;
    let ran = history.iter().filter(|r| r.applied).count();
    match next? {
        None => log(&format!("{} {} already lists what this database ran", paint("green", "✓"), shown(&opts.lock))),
        Some(next) => {
            lock::write(&opts.lock, &next)?;
            if ran == 0 {
                log(&format!("{} Wrote {}: this database has run no migrations, so it lists none yet", paint("green", "✓"), shown(&opts.lock)));
            } else {
                log(&format!("{} Wrote {}: {ran} migration{}, in the order this database ran them", paint("green", "✓"), shown(&opts.lock), if ran == 1 { "" } else { "s" }));
            }
        }
    }
    log(&stamp_line(opts, stamp(opts, schema_sql.as_deref())));
    Ok(0)
}

/// `reylo db reset`: rebuilds a development database from its migrations. Everything on it is dropped,
/// with its rows, but a platform's own objects and the extensions the project's files don't create;
/// the history is emptied; and every migration runs as `reylo migrate dev` runs it. Never in CI.
pub async fn db_reset(opts: &Options, in_ci: bool, log: Log<'_>) -> Result<i32, Error> {
    if in_ci {
        return Ok(refused_in_ci("reylo db reset", log));
    }
    let (mut db, mut server, swept) = open(opts).await?;
    swept_note(swept, log);
    let result = match emptied(&mut db, &mut server, opts, log).await {
        Ok(0) => match dev_on(&mut db, &mut server, 0, opts, "", log).await {
            Ok(0) => {
                let ran = read_history(&mut db).await.map(|h| h.iter().filter(|r| r.applied).count()).unwrap_or(0);
                if ran > 0 {
                    log(&format!("{} Rebuilt from {ran} migration{}", paint("green", "✓"), if ran == 1 { "" } else { "s" }));
                }
                Ok(0)
            }
            other => other,
        },
        other => other,
    };
    db.close().await;
    server.close().await;
    result
}

/// Takes the database to what an empty shadow holds, and forgets its history: in the same transaction
/// where the database has transactional DDL, and on MySQL, which commits each statement, the history
/// first, so that a reset cut off leaves migrations to run again, never ones recorded over nothing.
async fn emptied(db: &mut Db, server: &mut Server, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let _held = lock(db, opts).await?;
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &[]).await;
    let lane = shadows.lane();
    let empty = async {
        if let Err((_, e)) = shadows.open(lane).await? {
            return Err(Error(format!("An empty shadow database can't be made: {e}")));
        }
        let lines = catalog(shadows.db(lane)).await?;
        let target = read_target(shadows.db(lane), None).await?;
        Ok((lines, target))
    }
    .await;
    shadows.finish().await;
    let (lines, target) = empty?;
    let counts = table_rows(db).await?;
    let d = db.dialect();
    // The extensions the project's files create go too: they create them again.
    let mut then: Vec<String> = if d == Dialect::Postgres { created_extensions(opts).iter().map(|x| format!("drop extension if exists {}", crate::db::ident(x))).collect() } else { vec![] };
    let had_history = has_history(db).await? && !read_history(db).await?.is_empty();
    if has_history(db).await? {
        let forget = format!("delete from {HISTORY_TABLE}");
        if d == Dialect::Mysql {
            db.exec(&forget).await?;
        } else {
            then.push(forget);
        }
    }
    match take_to(db, &target, &then, &lines).await? {
        Err(lines) => {
            let problem = Problem::new("reset-incomplete", "Reylo can't drop everything on this database").lines(lines);
            if d != Dialect::Mysql {
                return Ok(refuse(&[problem], log));
            }
            print_problems(&[problem.note("MySQL commits each statement, so it is part of the way there, and its history is empty: reylo migrate dev runs every migration again.")], log);
            return Ok(1);
        }
        Ok((_, plan)) => {
            log(&format!("Resetting {}", shown_url(&opts.url)));
            let dropped = dropped_items(&plan, &counts);
            match (dropped, had_history) {
                (Some(dropped), true) => log(&format!("  {} {dropped} and emptied the history", paint("yellow", "!"))),
                (Some(dropped), false) => log(&format!("  {} {dropped}", paint("yellow", "!"))),
                (None, true) => log(&format!("  {} Emptied the history", paint("yellow", "!"))),
                (None, false) => {}
            }
        }
    }
    Ok(0)
}

/// The database's history, and where its tables aren't what the migrations it ran make, as it ran
/// them, why: what was changed by hand, with the ways to record it.
async fn history_as_it_stands(db: &mut Db, server: &mut Server, opts: &Options) -> Result<(Vec<HistoryRow>, Vec<Problem>), Error> {
    let history = read_history(db).await?;
    let leftovers = leftovers(db, &history).await?;
    let live: Vec<String> = catalog(db).await?.into_iter().filter(|l| !leftovers.iter().any(|x| l.starts_with(&x.line))).collect();
    let ran: Vec<&HistoryRow> = history.iter().filter(|r| r.applied).collect();
    let texts: Vec<&str> = ran.iter().map(|r| r.sql.as_str()).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &texts).await;
    let lane = shadows.lane();
    shadows.push(lane, ran.iter().map(|r| Step::migration(&r.name, &r.sql)));
    let replayed = shadows.lines(lane).await;
    let made = match replayed {
        Ok(Ok(made)) => made,
        Ok(Err((m, error))) => {
            shadows.finish().await;
            return Ok((history, vec![Problem::new("history-replay-fails", format!("{m} fails when replayed in a shadow database, as this database ran it")).note(error)]));
        }
        Err(e) => {
            shadows.finish().await;
            return Err(e);
        }
    };
    if crate::catalog::same_schema(&live, &made) {
        shadows.finish().await;
        return Ok((history, vec![]));
    }
    // The change as SQL, from what the history makes to this database.
    let mut sql = None;
    if let Ok(Ok(())) = shadows.open(lane).await {
        shadows.taint(lane);
        sql = plan_text(shadows.db(lane), db, None).await;
    }
    shadows.finish().await;
    let made = crate::catalog::without_comments(&crate::catalog::without_missing_children(&made, &live));
    let live = crate::catalog::without_comments(&crate::catalog::without_extra_children(&live, &made));
    let (only_history, only_live) = difference(&made, &live);
    let problem = Problem::new("database-drift", CHANGED_OUTSIDE);
    let problem = match sql {
        Some(sql) => problem.sql(&sql),
        None => problem.lines(std::iter::once(String::new()).chain(changed_lines(&made, &live)).chain([String::new()])),
    };
    let problem = problem.note("Files written from its history wouldn't make it either.");
    let schema = opts.schema.exists().then(|| shown(&opts.schema));
    Ok((history, vec![keep_fixes(problem, schema.as_deref(), only_live.is_empty(), only_history.is_empty())]))
}

/// `reylo migrate new <name>`: a migration named with the current UTC time, refused while any
/// migration is unapplied, so that its time comes after theirs.
/// What the person writing a migration states about it: the tables and columns it may drop, with
/// their rows, and the columns it renames.
#[derive(Clone, Debug, Default)]
pub struct Choices {
    /// Names as written: `users.nickname`, `audit_log`, `billing.invoices`, a schema's name.
    pub drop: Vec<String>,
    /// Each a column's old name, `users.full_name`, and its new one, `name`.
    pub rename: Vec<(String, String)>,
}

pub async fn create(opts: &Options, name: &str, choices: &Choices, log: Log<'_>) -> Result<i32, Error> {
    static VALID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9_]+$").expect("a valid pattern"));
    if name.is_empty() {
        return Err(Error("name the migration: reylo migrate new <name>".into()));
    }
    if !VALID.is_match(name) {
        // The name it was likely meant to be: lowercase, with a _ for each run of anything else.
        let mut like = String::new();
        for ch in name.chars() {
            if ch.is_ascii_alphanumeric() {
                like.push(ch.to_ascii_lowercase());
            } else if !like.ends_with('_') {
                like.push('_');
            }
        }
        let like = like.trim_matches('_');
        return Err(Error(if like.is_empty() { "a migration's name is a-z, 0-9 and _".to_string() } else { format!("a migration's name is a-z, 0-9 and _, as {like}") }));
    }
    let (files, _) = read_migrations(&opts.dir)?;
    // The migrations replay in the lock's order, then its drafts; a lock that can't be read is
    // resolved first.
    if let Err(problem) = read_lock(opts) {
        return Ok(refuse(&[problem], log));
    }
    let files = run_order(opts, &files);
    // The plan starts from every migration file, drafts not yet applied anywhere included, and is
    // named after the newest of them. Its target is schema.sql, or in a project with none, the
    // development database as it stands, written out as SQL (as `db introspect` prints it).
    let (target_sql, target_name) = match std::fs::read_to_string(&opts.schema) {
        Ok(sql) => (sql, shown(&opts.schema)),
        Err(_) => match as_its_migrations_leave_it(opts, &files).await? {
            Ok(sql) => (sql, "this database".to_string()),
            Err(problems) => return Ok(refuse(&problems, log)),
        },
    };
    let drafted = match draft(opts, &files, &target_sql, &target_name, name, choices, log).await? {
        Ok(drafted) => drafted,
        Err(problems) => return Ok(refuse(&problems, log)),
    };
    std::fs::create_dir_all(&opts.dir)?;
    // After the newest migration, should one share this second; on MySQL a plan of several DDL
    // statements is several files, a second apart, `<name>`, `<name>_2`, ….
    let newest = files.iter().filter_map(|f| time::seconds(&f.time)).max();
    let at = time::now_seconds().max(newest.map_or(0, |n| n + 1));
    let mut written = Vec::new();
    for (k, text) in drafted.texts.iter().enumerate() {
        let suffix = if k == 0 { String::new() } else { format!("_{}", k + 1) };
        let file = opts.dir.join(format!("{}_{name}{suffix}.sql", time::stamp(at + k as i64)));
        std::fs::write(&file, text)?;
        written.push(shown(&file));
    }
    match written.as_slice() {
        [one] if drafted.empty => log(&format!("{} Wrote {one}, empty: {} has nothing new, so it's for a change to data", paint("green", "✓"), drafted.target)),
        [one] => log(&format!("{} Wrote {one}", paint("green", "✓"))),
        many => {
            log(&format!("{} Wrote {} migrations, one per statement, since MySQL commits each DDL statement on its own:", paint("green", "✓"), many.len()));
            for path in many {
                log(&format!("    {path}"));
            }
        }
    }
    for note in &drafted.notes {
        log(note);
    }
    Ok(drafted.code)
}

/// The development database as SQL that makes it, for `migrate new` in a project with no schema.sql:
/// the migration is what it has that the migrations don't make. Refused unless it ran every migration,
/// each as it now reads, since one it hasn't run would read as something to drop.
async fn as_its_migrations_leave_it(opts: &Options, files: &[Migration]) -> Result<Result<String, Vec<Problem>>, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let result = async {
        let d = db.dialect();
        let history = if has_history(&mut db).await? { read_history(&mut db).await? } else { vec![] };
        let mut problems = Vec::new();
        for r in history.iter().filter(|r| r.applied) {
            match files.iter().find(|f| f.name == r.name) {
                None => problems.push(missing_problem(&r.name, &r.sql, &shown(&opts.dir))),
                Some(f) if !same_statements(&f.sql, &r.sql, d) => problems.push(edited_problem(&r.name, &r.sql, &f.sql)),
                Some(_) => {}
            }
        }
        let pending: Vec<&str> = files.iter().filter(|f| !history.iter().any(|r| r.applied && r.name == f.name)).map(|f| f.name.as_str()).collect();
        if !pending.is_empty() {
            let one = pending.len() == 1;
            problems.push(
                Problem::new("migrations-unapplied", format!("{} {} applied to this database", pending.join(", "), if one { "isn't" } else { "aren't" }))
                    .note(format!("With no {}, a migration is what this database has beyond the migrations, so it must have run them all.", shown(&opts.schema)))
                    .fix(format!("Run {} first", if one { "it" } else { "them" }), &["reylo migrate dev"]),
            );
        }
        if !problems.is_empty() {
            return Ok(Err(problems));
        }
        let schemas = if d == Dialect::Postgres { crate::catalog::user_schemas(&mut db).await? } else { vec![] };
        dump_from(&mut db, opts, &schemas).await.map(Ok)
    }
    .await;
    db.close().await;
    result
}

/// The refusal of a target that doesn't replay: schema.sql, or the database's schema as SQL.
fn fails_problem(target_name: &str, error: String) -> Problem {
    let title = if target_name.ends_with(".sql") { format!("{target_name} doesn't run on an empty database") } else { "This database's schema doesn't replay on an empty database".to_string() };
    Problem::new("schema-sql-fails", title).note(error)
}

/// A migration `draft` writes: its files' texts, the exit code, what to say under the files once
/// they are written, whether it is empty (for a change of data), and what it was drafted from.
struct Drafted {
    texts: Vec<String>,
    code: i32,
    notes: Vec<String>,
    empty: bool,
    target: String,
}

/// What a plan loses, as a sentence names it: `the column users.bio`, `the table tags`.
fn described(loss: &str) -> String {
    match (loss.strip_prefix("column "), loss.strip_prefix("table ")) {
        (Some(column), _) => format!("the column {column}"),
        (_, Some(table)) => format!("the table {table}"),
        _ => loss.to_string(),
    }
}

/// The plan as migration files: one, or on MySQL, where every DDL statement commits on its own,
/// one per statement, each with the comments written above it.
fn plan_files(plan: &crate::diff::Plan, d: Dialect) -> Vec<String> {
    if d != Dialect::Mysql {
        return vec![plan.sql()];
    }
    let mut files: Vec<String> = Vec::new();
    let mut pending = String::new();
    for loss in &plan.losses {
        pending.push_str(&format!("-- loses data: {}\n", loss.trim_start_matches("column ").trim_start_matches("table ")));
    }
    for (column, why) in &plan.narrowings {
        pending.push_str(&format!("-- loses data: {} {why}\n", column.trim_start_matches("column ")));
    }
    for s in &plan.statements {
        if s.starts_with("--") {
            pending.push_str(s);
            pending.push('\n');
            continue;
        }
        files.push(format!("{pending}{};\n", s.trim_end()));
        pending.clear();
    }
    if files.is_empty() {
        files.push(pending);
    }
    files
}

/// A dotted name's parts, each unquoted: `public."Users".nick` is public, Users, nick.
fn split_name(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut part = String::new();
    let mut quote: Option<char> = None;
    for ch in name.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => part.push(ch),
            None if ch == '"' || ch == '`' => quote = Some(ch),
            None if ch == '.' => out.push(std::mem::take(&mut part)),
            None => part.push(ch),
        }
    }
    out.push(part);
    out
}

/// The tables and columns among `ids` (`table public.users`, `column users.nick`) that a name
/// written on the command line names: a table by its name (`users`, `billing.invoices`), every
/// table of a schema by the schema's name, a column by its table and its name (`users.nick`,
/// `public.users.nick`). A column is never named by its name alone. Where the name could be more
/// than one thing, `Err` holds each.
fn resolve(ids: &[String], written: &str) -> Result<Vec<String>, Vec<String>> {
    // As written first; then, where that names nothing, ignoring case, as a shell makes quoting a
    // name awkward and MySQL's and SQLite's names ignore it.
    match resolve_by(ids, written, true) {
        Ok(hits) if hits.is_empty() => resolve_by(ids, written, false),
        other => other,
    }
}

fn resolve_by(ids: &[String], written: &str, exact: bool) -> Result<Vec<String>, Vec<String>> {
    let same = |a: &[String], b: &[String]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| if exact { x == y } else { x.eq_ignore_ascii_case(y) });
    let w = split_name(written);
    let (mut exact_hits, mut by_suffix, mut by_schema) = (Vec::new(), Vec::new(), Vec::new());
    for id in ids {
        let (kind, rest) = id.split_once(' ').unwrap_or(("", id.as_str()));
        let p = split_name(rest);
        if same(&p, &w) {
            exact_hits.push(id.clone());
            continue;
        }
        match kind {
            "table" if w.len() == 1 && same(&p[p.len() - 1..], &w) => by_suffix.push(id.clone()),
            "table" if w.len() == 1 && p.len() == 2 && same(&p[..1], &w) => by_schema.push(id.clone()),
            "column" if w.len() >= 2 && p.len() > w.len() && same(&p[p.len() - w.len()..], &w) => by_suffix.push(id.clone()),
            _ => {}
        }
    }
    match (exact_hits.is_empty(), by_suffix.len(), by_schema.is_empty()) {
        (false, 0, _) => Ok(exact_hits),
        (true, 0, _) => Ok(by_schema),
        (true, 1, true) => Ok(by_suffix),
        _ => Err(exact_hits.into_iter().chain(by_suffix).chain(by_schema).collect()),
    }
}

/// A schema to take a database to: as the diff engine reads it, and on Postgres every table's
/// indexes by name, those no object shows included.
struct Target {
    schema: crate::diff::Schema,
    indexes: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
}

/// The target `db` holds, declared by `declared` where it was built from a file of its own.
async fn read_target(db: &mut Db, declared: Option<&str>) -> Result<Target, Error> {
    let mut schema = crate::diff::read(db).await?;
    let mut indexes = Default::default();
    if db.dialect() == Dialect::Postgres {
        if let Some(sql) = declared {
            crate::diff::postgres::declare(db, &mut schema, sql).await?;
        }
        indexes = crate::diff::postgres::index_names(db).await?;
    }
    Ok(Target { schema, indexes })
}

/// Renames the tables `renamed` names on `db`, and on Postgres what is named after them, and gives
/// the statements that did, as the migration writes them.
async fn rename_tables(db: &mut Db, current: &crate::diff::Schema, target: &Target, renamed: &[(String, String, bool)]) -> Result<Vec<String>, Error> {
    let d = db.dialect();
    let mut statements = table_rename_statements(d, current, &target.schema, renamed);
    let sql: Vec<String> = statements.iter().filter(|s| !s.starts_with("--")).map(|s| format!("{s};")).collect();
    if sql.is_empty() {
        return Ok(statements);
    }
    db.script(&sql.join("\n")).await?;
    if d == Dialect::Postgres {
        let bare = |id: &str| split_name(&shown_id(id)).pop().unwrap_or_default();
        let names: Vec<(String, String, String)> = renamed.iter().map(|(old, new, _)| (bare(old), bare(new), new.clone())).collect();
        let more = crate::diff::postgres::inherited_renames(db, &target.indexes, &names).await?;
        if !more.is_empty() {
            db.script(&more.iter().map(|s| format!("{s};")).collect::<Vec<_>>().join("\n")).await?;
            statements.extend(more);
        }
        crate::diff::postgres::style_statements(db, &mut statements, &target.schema.verbatim).await?;
    }
    Ok(statements)
}

/// The plan from the database to `target`: a table that goes while one of its columns comes renamed
/// first, on the database itself, then the diff engine's plan from there, which is not run.
async fn plan_to(db: &mut Db, target: &Target, live: bool) -> Result<(Vec<String>, crate::diff::Plan), Error> {
    let mut current = crate::diff::read(db).await?;
    current.live = live;
    let renamed = crate::diff::table_renames(&current, &target.schema, &[], &std::collections::BTreeSet::new());
    let renames = if renamed.is_empty() { vec![] } else { rename_tables(db, &current, target, &renamed).await? };
    if !renames.is_empty() {
        current = crate::diff::read(db).await?;
        current.live = live;
    }
    let alterer = crate::diff::alterer(db).await?;
    let mut plan = crate::diff::plan(&current, &target.schema, alterer.as_ref());
    style(db, &mut plan, &target.schema).await?;
    Ok((renames, plan))
}

/// The statements that rename the tables `renamed` names (each its old id in `current`, its new id
/// in `target`, and whether --rename stated it), and on Postgres what is named after them; a guessed
/// rename is written down above its statement with what to do instead.
fn table_rename_statements(d: Dialect, current: &crate::diff::Schema, target: &crate::diff::Schema, renamed: &[(String, String, bool)]) -> Vec<String> {
    // A table's name as its statements write it, and whether it is a foreign table.
    let written = |schema: &crate::diff::Schema, id: &str| -> (String, bool) {
        let drop = schema.objects[id].drop.first().map(String::as_str).unwrap_or_default();
        match drop.strip_prefix("drop foreign table ") {
            Some(name) => (name.to_string(), true),
            None => (drop.strip_prefix("drop table ").map_or_else(|| shown_id(id), str::to_string), false),
        }
    };
    let bare = |id: &str| split_name(&shown_id(id)).pop().unwrap_or_default();
    let mut out = Vec::new();
    let mut names = Vec::new();
    for (old_id, new_id, was_stated) in renamed {
        let ((from_w, foreign), (to_w, _)) = (written(current, old_id), written(target, new_id));
        let new_name = if d == Dialect::Postgres { crate::diff::last_written(&to_w) } else { to_w };
        let (old, new) = (bare(old_id), bare(new_id));
        if !was_stated {
            out.push(format!("-- {old} is renamed {new}, keeping its rows. If they are two tables, drop the one and create the other instead."));
        }
        out.push(format!("alter {}table {from_w} rename to {new_name}", if foreign { "foreign " } else { "" }));
        names.push((old, new, old_id.clone(), new_id.clone()));
    }
    if d == Dialect::Postgres {
        out.extend(crate::diff::postgres::derived_renames(current, target, &names));
    }
    out
}

/// An object's id as a command names it: no kind, and no `public` schema.
fn shown_name(id: &str) -> String {
    let name = shown_id(id);
    name.strip_prefix("public.").map(str::to_string).unwrap_or(name)
}

fn shown_id(id: &str) -> String {
    id.split_once(' ').map_or(id, |(_, rest)| rest).to_string()
}

/// A name as the shell is to be given it: quoted where it isn't one word, or holds a quote. The
/// placeholder for a name to write is left to be written over.
fn shell_word(name: &str) -> String {
    if name == "<new name>" || name.chars().all(|c| c.is_ascii_alphanumeric() || "_.$-".contains(c)) {
        return name.to_string();
    }
    if cfg!(windows) {
        return format!("\"{}\"", name.replace('"', "\\\""));
    }
    format!("'{}'", name.replace('\'', "'\\''"))
}

/// The command that writes the migration with what the person stated and what is to be added.
fn create_command(migration: &str, choices: &Choices, more_drops: &[String], renames: &[(String, String)]) -> String {
    let mut drops: Vec<&String> = Vec::new();
    for name in choices.drop.iter().chain(more_drops) {
        if !drops.contains(&name) {
            drops.push(name);
        }
    }
    let mut flags: Vec<String> = Vec::new();
    if !drops.is_empty() {
        flags.push(format!("--drop {}", drops.iter().map(|n| shell_word(n)).collect::<Vec<_>>().join(",")));
    }
    flags.extend(choices.rename.iter().chain(renames).map(|(old, new)| format!("--rename {}={}", shell_word(old), shell_word(new))));
    // Several flags read one to a line, where the shell continues a line with a backslash.
    let (head, gap) = (format!("reylo migrate new {migration}"), if cfg!(windows) || flags.len() < 2 { " " } else { " \\\n      " });
    std::iter::once(head).chain(flags).collect::<Vec<_>>().join(gap)
}

/// A rename `create` won't assume, since it changes more than a name: only the person can say whether
/// it is one thing renamed or two things.
struct Guess {
    /// The old name, as a command writes it: `users.email`, `orders`.
    old: String,
    /// The new name, as `--rename` takes it: `contact`, `invoices`.
    new: String,
    /// What else changes: `text → citext`, `adds total`.
    change: String,
    /// Whether its table is itself one of the guesses, so that dropping that table says it all.
    in_guessed_table: bool,
}

/// The refusal for guesses, with what else the migration asks of `--drop`: one block, and the one
/// command for each way of settling them all.
fn guess_problem(guesses: &[Guess], drops: &[(String, String)], narrowed: &[(String, String)], outside: &[String], stated: &Choices, migration: &str) -> Problem {
    let one = guesses.len() == 1;
    let title = if one { "This looks like a rename, but it changes more than the name".to_string() } else { format!("These {} look like renames, but each changes more than its name", guesses.len()) };
    let (old_width, new_width) = (guesses.iter().map(|g| g.old.chars().count()).max().unwrap_or(0), guesses.iter().map(|g| g.new.chars().count()).max().unwrap_or(0));
    // A type as schema.sql writes it, rather than as the catalog spells it.
    let change = |g: &Guess| g.change.replace("character varying", "varchar");
    let mut problem = Problem::new("rename-guess", title).lines([String::new()]).lines(guesses.iter().map(|g| format!("    {:<old_width$} → {:<new_width$}  {}", g.old, g.new, change(g))));
    if !drops.is_empty() {
        problem = problem.lines([String::new()]).note(if drops.len() == 1 { format!("It also drops {} and its data.", drop_phrase(drops)) } else { format!("It also drops {}, with their data.", drop_phrase(drops)) });
    }
    for (name, why) in narrowed {
        problem = problem.note(format!("{name} {why}."));
    }
    let more: Vec<String> = drops.iter().map(|(_, n)| n.clone()).chain(narrowed.iter().map(|(n, _)| n.clone())).collect();
    let renames: Vec<(String, String)> = guesses.iter().map(|g| (g.old.clone(), g.new.clone())).collect();
    let dropped: Vec<String> = guesses.iter().filter(|g| !g.in_guessed_table).map(|g| g.old.clone()).chain(outside.iter().cloned()).collect();
    problem
        .fix(if one { "If it is a rename, keeping its rows" } else { "If they are renames, keeping their rows" }, &[&create_command(migration, stated, &more, &renames)])
        .fix(if one { "If it is new, dropping the old one and its data" } else { "If they are new, dropping the old ones and their data" }, &[&create_command(migration, stated, &dropped, &[])])
}

/// The problems with what `choices` states about the tables and columns a plan drops (`lost`, as
/// the plan's ids, and `shown`, as the file names them, one for one): each must be named in
/// `--drop`, and each name in `--drop` must name only what it drops. `guesses`, renames it won't
/// assume, are settled with them; `guessed_tables` are the new ids of the tables among them, whose
/// own columns a drop of the table takes along; `added` are the columns and tables the plan adds, the
/// names a person could mean by a rename.
#[allow(clippy::too_many_arguments)]
fn drop_problems(lost: &[String], shown: &[String], narrowed: &[(String, String, String)], guesses: &[Guess], owners: &dyn Fn(&str) -> bool, added: &[String], choices: &Choices, migration: &str) -> Vec<Problem> {
    let display = |id: &String| lost.iter().position(|l| l == id).map(|k| shown_id(&shown[k])).or_else(|| narrowed.iter().find(|(n, _, _)| n == id).map(|(_, s, _)| shown_id(s))).unwrap_or_else(|| shown_id(id));
    let mut problems = Vec::new();
    let mut named: Vec<String> = Vec::new();
    // What --drop states that holds, which a suggested command keeps. It names what is dropped, and
    // a column whose change of type loses what its values hold.
    let mut stated = Choices { drop: vec![], rename: choices.rename.clone() };
    let all: Vec<String> = lost.iter().cloned().chain(narrowed.iter().map(|(id, _, _)| id.clone())).collect();
    for w in &choices.drop {
        match resolve(&all, w) {
            Ok(hits) if hits.is_empty() => {
                let columns: Vec<String> = lost.iter().filter(|l| l.starts_with("column ") && split_name(&shown_id(l)).last() == split_name(w).last() && split_name(w).len() == 1).map(&display).collect();
                problems.push(Problem::new("drop-name-unknown", format!("--drop names {w}, which this migration doesn't drop")).note(if !columns.is_empty() {
                    format!("A column is named with its table: {}.", columns.join(", "))
                } else if lost.is_empty() {
                    "It drops no table or column.".to_string()
                } else {
                    format!("It drops {}.", lost.iter().map(&display).collect::<Vec<_>>().join(", "))
                }))
            }
            Ok(hits) => {
                named.extend(hits);
                stated.drop.push(w.clone());
            }
            Err(could) => problems.push(Problem::new("drop-name-ambiguous", format!("--drop names {w}, which could be any of {}", could.iter().map(&display).collect::<Vec<_>>().join(", "))).note("Name each with its table, and its schema where two share a name.")),
        }
    }
    let unstated: Vec<&String> = lost.iter().filter(|l| !named.contains(l)).collect();
    let unnamed: Vec<(&String, String, String)> = narrowed.iter().filter(|(id, _, _)| !named.contains(id)).map(|(id, shown, why)| (id, shown_id(shown), why.clone())).collect();
    if !guesses.is_empty() {
        let drops: Vec<(String, String)> = unstated.iter().map(|l| (l.split(' ').next().unwrap_or_default().to_string(), display(l))).collect();
        let lossy: Vec<(String, String)> = unnamed.iter().map(|(_, name, why)| (name.clone(), why.clone())).collect();
        let outside: Vec<String> = unstated.iter().copied().chain(unnamed.iter().map(|(id, _, _)| *id)).filter(|id| !owners(id)).map(&display).collect();
        problems.push(guess_problem(guesses, &drops, &lossy, &outside, &stated, migration));
        return problems;
    }
    if !unstated.is_empty() {
        let list: Vec<String> = unstated.iter().map(|l| display(l)).collect();
        let what: Vec<(String, String)> = unstated.iter().map(|l| (l.split(' ').next().unwrap_or_default().to_string(), display(l))).collect();
        let title = match unstated.as_slice() {
            [one] => format!("This migration drops {} and {}", drop_phrase(&what), if one.starts_with("table ") { "its rows" } else { "its data" }),
            _ => format!("This migration drops {}, with their data", drop_phrase(&what)),
        };
        let mut problem = Problem::new("drops-data", title)
            .fix(if list.len() == 1 { "To drop it" } else { "To drop them" }, &[&create_command(migration, &stated, &list, &[])]);
        // Where something comes as these go, any of them may be renamed: which became which is
        // for the person to say, so the command names no new name for them.
        // What the plan adds of the same kind: a table renamed is a table, a column a column.
        let kinds: Vec<&str> = unstated.iter().filter_map(|l| l.split(' ').next()).collect();
        let like: Vec<String> = added.iter().filter(|a| kinds.contains(&a.split(' ').next().unwrap_or_default())).map(|a| a.split_once(' ').map_or(a.clone(), |(_, n)| n.to_string())).collect();
        if !like.is_empty() {
            let renames: Vec<(String, String)> = list.iter().map(|old| (old.clone(), "<new name>".to_string())).collect();
            let what = format!("If {} renamed instead, as {}", if list.len() == 1 { "it was" } else { "any were" }, like.join(", "));
            problem = problem.fix(what, &[&create_command(migration, &stated, &[], &renames)]);
        }
        problems.push(problem);
    }
    for (_, name, why) in &unnamed {
        problems.push(Problem::new("loses-data", format!("{name} {why}")).fix("To accept that", &[&create_command(migration, &stated, std::slice::from_ref(name), &[])]));
    }
    problems
}

/// The columns `choices` renames, as (old, new) ids of `from` and `to`; or why one cannot be.
fn stated_renames(from: &crate::diff::Schema, to: &crate::diff::Schema, choices: &Choices) -> Result<Vec<(String, String)>, Vec<Problem>> {
    let gone: Vec<String> = from.objects.keys().filter(|id| id.starts_with("column ") && !to.objects.contains_key(*id)).cloned().collect();
    let mut out = Vec::new();
    let mut problems = Vec::new();
    for (old, new) in &choices.rename {
        let old_id = match resolve(&gone, old) {
            Ok(hits) if hits.len() == 1 => hits[0].clone(),
            Ok(_) => {
                problems.push(Problem::new("rename-name-unknown", format!("--rename names {old}, which isn't a table or column this migration removes")).note("A column is named with its table, --rename users.full_name=name, and a table by its name, --rename users=accounts."));
                continue;
            }
            Err(could) => {
                problems.push(Problem::new("rename-name-ambiguous", format!("--rename names {old}, which could be any of {}", could.iter().map(|c| shown_id(c)).collect::<Vec<_>>().join(", "))).note("Name it with its schema."));
                continue;
            }
        };
        let table = from.objects[&old_id].owner.clone().unwrap_or_default();
        let wanted = split_name(new).pop().unwrap_or_default();
        let added: Vec<(&String, String)> = to.objects.iter().filter(|(id, o)| o.kind == Some(crate::diff::Kind::Column) && o.owner.as_deref() == Some(table.as_str()) && !from.objects.contains_key(*id)).map(|(id, _)| (id, split_name(id.split_once(' ').map_or(id.as_str(), |(_, r)| r)).pop().unwrap_or_default())).collect();
        let exact: Vec<&String> = added.iter().filter(|(_, n)| *n == wanted).map(|(id, _)| *id).collect();
        let found = if exact.is_empty() { added.iter().filter(|(_, n)| n.eq_ignore_ascii_case(&wanted)).map(|(id, _)| *id).collect() } else { exact };
        match found.as_slice() {
            [new_id] => out.push((old_id, (*new_id).clone())),
            [] => problems.push(Problem::new("rename-target-unknown", format!("--rename gives {old} the name {new}, which isn't a column this migration adds to its table"))),
            many => problems.push(Problem::new("rename-target-ambiguous", format!("--rename gives {old} the name {new}, which could be any of {}", many.iter().map(|c| shown_id(c)).collect::<Vec<_>>().join(", "))).note("Quote the name as it is written: --rename 'users.n=\"Total\"'.")),
        }
    }
    if problems.is_empty() { Ok(out) } else { Err(problems) }
}

/// The new migration's text: the diff engine's plan from the migrations replayed in order to
/// `schema.sql`, run on the replay and compared with `schema.sql` before it is written. A plan that
/// does not reach it is written all the same, with what remains, for the person (or agent) to finish.
#[allow(clippy::type_complexity)]
async fn draft(opts: &Options, files: &[Migration], schema_sql: &str, target_name: &str, migration: &str, choices: &Choices, log: Log<'_>) -> Result<Result<Drafted, Vec<Problem>>, Error> {
    let drafted = |texts: Vec<String>, code: i32, notes: Vec<String>, empty: bool| Drafted { texts, code, notes, empty, target: target_name.to_string() };
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let mut server = Server::open(&mut db, &opts.url, opts.shadow_url.as_deref()).await?;
    server.copy_default_privileges(!sets_default_privileges(opts));
    server.leave_extensions(created_extensions(opts));
    db.close().await;
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain([schema_sql]).collect();
    let mut shadows = Shadows::new(&mut server, &opts.url, &opts.root, &opts.cache, &texts).await;
    let result = async {
        let a = shadows.lane();
        shadows.push(a, [Step::script("schema.sql", schema_sql)]);
        if let Err((_, e)) = shadows.open(a).await? {
            return Ok(Err(vec![fails_problem(target_name, e)]));
        }
        // The plan runs on the replay, which is then no longer one: the replay is kept
        // first, as the start of the check that follows.
        let c = shadows.lane();
        shadows.push(c, files.iter().map(Step::of));
        shadows.mark(c);
        if let Err((m, e)) = shadows.open(c).await? {
            return Ok(Err(vec![Problem::new("replay-fails", format!("{m} fails when replayed in a shadow database")).note(e)]));
        }
        shadows.taint(c);
        unmodeled_note("schema.sql has what Reylo doesn't fully compare, so a migration can leave a change to it out: check those by hand", &crate::catalog::unmodeled(shadows.db(a)).await?, log);
        let mut target = match crate::diff::read(shadows.db(a)).await {
            Ok(schema) => schema,
            Err(e) => {
                let note = format!("  {} {e}: the migration is empty, to write the change to {target_name} by hand", paint("yellow", "!"));
                return Ok(Ok(drafted(vec![String::new()], 0, vec![note], false)));
            }
        };
        let mut current = crate::diff::read(shadows.db(c)).await?;
        // On Postgres the file creates each object from the text schema.sql declares it with, and a
        // table moved to another schema is moved first, keeping its rows: the moves run on the
        // replay before the rest is planned from it.
        let mut moves = Vec::new();
        if shadows.db(c).dialect() == Dialect::Postgres {
            crate::diff::postgres::declare(shadows.db(a), &mut target, schema_sql).await?;
            moves = crate::diff::postgres::moves(&current, &target);
            crate::diff::postgres::style_statements(shadows.db(c), &mut moves, &target.verbatim).await?;
            if !moves.is_empty() {
                let file = Migration { name: "moves".into(), time: String::new(), path: PathBuf::new(), sql: moves.iter().map(|m| format!("{m};")).collect::<Vec<_>>().join("\n") };
                run(shadows.db(c), &file, false).await?;
                current = crate::diff::read(shadows.db(c)).await?;
            }
        }
        // Tables renamed, as --rename states, or where one goes while one of the same columns comes:
        // renamed first on the replay, keeping their rows, with what Postgres named after them, and
        // the rest planned from there.
        let d = shadows.db(c).dialect();
        let tables_only = |a: &crate::diff::Schema, b: &crate::diff::Schema| -> Vec<String> { a.objects.values().filter(|o| o.kind == Some(crate::diff::Kind::Table) && !b.objects.contains_key(&o.id)).map(|o| o.id.clone()).collect() };
        let (gone_tables, come_tables) = (tables_only(&current, &target), tables_only(&target, &current));
        let mut stated_tables = Vec::new();
        let mut column_renames = Vec::new();
        let mut problems = Vec::new();
        for (old, new) in &choices.rename {
            match resolve(&gone_tables, old) {
                Ok(hits) if hits.len() == 1 => match resolve(&come_tables, new) {
                    Ok(to) if to.len() == 1 => stated_tables.push((hits[0].clone(), to[0].clone())),
                    _ => problems.push(Problem::new("rename-target-unknown", format!("--rename gives the table {old} the name {new}, which isn't a table this migration creates"))),
                },
                Err(could) => problems.push(Problem::new("rename-name-ambiguous", format!("--rename names {old}, which could be any of {}", could.iter().map(|c| shown_id(c)).collect::<Vec<_>>().join(", "))).note("Name it with its schema.")),
                _ => column_renames.push((old.clone(), new.clone())),
            }
        }
        if !problems.is_empty() {
            return Ok(Err(problems));
        }
        let dropped_tables: std::collections::BTreeSet<String> = choices.drop.iter().filter_map(|w| resolve(&gone_tables, w).ok()).flatten().collect();
        let renamed_tables = crate::diff::table_renames(&current, &target, &stated_tables, &dropped_tables);
        // A table renamed with its columns changing too is a guess only the person can settle.
        let mut guesses: Vec<Guess> = Vec::new();
        let mut guessed_tables: std::collections::BTreeSet<String> = Default::default();
        // Renames written without being asked: each old name, new name, and whether it is a table.
        let mut assumed: Vec<(String, String, bool)> = Vec::new();
        for (old, new, _) in renamed_tables.iter().filter(|(_, _, was_stated)| !was_stated) {
            match crate::diff::column_changes(&current, &target, old, new) {
                Some(change) => {
                    guesses.push(Guess { old: shown_name(old), new: shown_name(new), change, in_guessed_table: false });
                    guessed_tables.insert(new.clone());
                }
                None => assumed.push((shown_name(old), shown_name(new), true)),
            }
        }
        if !renamed_tables.is_empty() {
            let indexes = if d == Dialect::Postgres { crate::diff::postgres::index_names(shadows.db(a)).await? } else { Default::default() };
            let to = Target { schema: target, indexes };
            let statements = rename_tables(shadows.db(c), &current, &to, &renamed_tables).await?;
            target = to.schema;
            current = crate::diff::read(shadows.db(c)).await?;
            moves.extend(statements);
        }
        // A name --drop or --rename gives through a renamed table's old name means it under its new one.
        let follow = |name: &str| -> String {
            if !matches!(resolve(&gone_tables, name), Ok(hits) if hits.is_empty()) {
                return name.to_string();
            }
            let w = split_name(name);
            for (old_id, new_id, _) in &renamed_tables {
                let (old, new) = (split_name(&shown_id(old_id)), split_name(&shown_id(new_id)));
                let head = &w[..w.len().saturating_sub(1)];
                if !head.is_empty() && head.len() <= old.len() && old[old.len() - head.len()..] == *head {
                    let mut out: Vec<String> = new[new.len() - head.len()..].to_vec();
                    out.push(w.last().cloned().unwrap_or_default());
                    return out.join(".");
                }
            }
            name.to_string()
        };
        let columns = Choices { drop: choices.drop.iter().map(|w| follow(w)).collect(), rename: column_renames.iter().map(|(a, b)| (follow(a), b.clone())).collect() };
        let alterer = crate::diff::alterer(shadows.db(c)).await?;
        let renames = match stated_renames(&current, &target, &columns) {
            Ok(renames) => renames,
            Err(problems) => return Ok(Err(problems)),
        };
        // The columns --drop names are never guessed renamed.
        let gone: Vec<String> = current.objects.keys().filter(|id| id.starts_with("column ") && !target.objects.contains_key(*id)).cloned().collect();
        let dropped: std::collections::BTreeSet<String> = columns.drop.iter().filter_map(|w| resolve(&gone, w).ok()).flatten().collect();
        let mut plan = crate::diff::plan_with(&current, &target, alterer.as_ref(), &renames, &dropped);
        let lost = plan.losses.clone();
        let narrowing: Vec<(String, String)> = plan.narrowings.clone();
        let guessed: Vec<String> = plan.renames.iter().map(|r| r.0.clone()).collect();
        style(shadows.db(c), &mut plan, &target).await?;
        let narrowed: Vec<(String, String, String)> = narrowing.iter().zip(&plan.narrowings).map(|((id, why), (shown, _))| (id.clone(), shown.clone(), why.clone())).collect();
        let owned_by_guess = |id: &str| current.objects.get(id).and_then(|o| o.owner.as_ref()).is_some_and(|t| guessed_tables.contains(t));
        for (raw, (old, new, change)) in guessed.iter().zip(&plan.renames) {
            let old = shown_id(old);
            match change {
                Some(change) => guesses.push(Guess { old, new: new.clone(), change: change.clone(), in_guessed_table: owned_by_guess(raw) }),
                None => assumed.push((old, new.clone(), false)),
            }
        }
        // What the plan adds, the names a person might mean by a rename of what it drops.
        let added: Vec<String> = target.objects.iter().filter(|(id, o)| matches!(o.kind, Some(crate::diff::Kind::Column | crate::diff::Kind::Table)) && !current.objects.contains_key(*id) && !(o.kind == Some(crate::diff::Kind::Column) && o.owner.as_ref().is_some_and(|t| !current.objects.contains_key(t)))).map(|(id, o)| format!("{} {}", if o.kind == Some(crate::diff::Kind::Table) { "table" } else { "column" }, shown_name(id))).collect();
        let problems = drop_problems(&lost, &plan.losses, &narrowed, &guesses, &owned_by_guess, &added, &Choices { drop: columns.drop.clone(), rename: choices.rename.clone() }, migration);
        if !problems.is_empty() {
            return Ok(Err(problems));
        }
        if plan.statements.is_empty() && moves.is_empty() {
            return Ok(Ok(drafted(vec![String::new()], 0, vec![], true)));
        }
        // The plan runs on the replay as `deploy` will run its files: one file in one transaction (on
        // SQLite with foreign keys off until it is checked), or on MySQL a file per statement.
        let pieces: Vec<String> = if d == Dialect::Mysql {
            plan.runnable().cloned().collect()
        } else {
            vec![plan.runnable().map(|s| format!("{};", s.trim_end())).collect::<Vec<_>>().join("\n")]
        };
        let mut failed = None;
        for sql in pieces {
            let file = Migration { name: "plan".into(), time: String::new(), path: PathBuf::new(), sql };
            if let Err(e) = run(shadows.db(c), &file, false).await {
                failed = Some(e.0);
                break;
            }
        }
        let wanted = match shadows.lines(a).await? {
            Ok(lines) => lines,
            Err((_, e)) => return Ok(Err(vec![fails_problem(target_name, e)])),
        };
        let reached = catalog(shadows.db(c)).await?;
        plan.statements.splice(0..0, moves);
        let mut texts = plan_files(&plan, d);
        let mut notes = Vec::new();
        for loss in &plan.losses {
            notes.push(format!("  {} Drops {} and its {}", paint("yellow", "!"), described(loss), if loss.starts_with("table ") { "rows" } else { "data" }));
        }
        for (column, why) in &plan.narrowings {
            notes.push(format!("  {} {} {why}", paint("yellow", "!"), column.trim_start_matches("column ")));
        }
        if !assumed.is_empty() {
            let one = assumed.len() == 1;
            let names: Vec<String> = assumed.iter().map(|(old, _, _)| old.clone()).collect();
            if one {
                notes.push(format!("  {} {} → {} is written as a rename, keeping its rows", paint("yellow", "!"), assumed[0].0, assumed[0].1));
            } else {
                let width = assumed.iter().map(|(old, _, _)| old.chars().count()).max().unwrap_or(0);
                notes.push(format!("  {} Written as renames, keeping their rows:", paint("yellow", "!")));
                notes.extend(assumed.iter().map(|(old, new, _)| format!("      {old:<width$} → {new}")));
            }
            let unasked = match (assumed.iter().all(|a| a.2), assumed.iter().all(|a| !a.2)) {
                (true, _) if one => "If they're different tables",
                (_, true) if one => "If they're different columns",
                _ => "Where any aren't renames",
            };
            notes.push(format!("    {unasked}, delete the file, then:"));
            notes.push(format!("      {}", paint("cyan", &create_command(migration, choices, &names, &[]))));
        }
        if failed.is_none() && reached == wanted {
            return Ok(Ok(drafted(texts, 0, notes, false)));
        }
        let mut remaining = Vec::new();
        if let Some(f) = &failed {
            remaining.push(format!("fails at {f}"));
        }
        let (only_a, only_b) = difference(&reached, &wanted);
        remaining.extend(only_a.iter().map(|l| format!("- {l}")));
        remaining.extend(only_b.iter().map(|l| format!("+ {l}")));
        notes.push(paint("bold red", &format!("✗ The migration doesn't reach {target_name}: what remains is written at its top, to finish by hand")));
        notes.extend(remaining.iter().map(|r| format!("  {r}")));
        texts[0] = format!("-- This plan does not reach {target_name} yet. What remains:\n{}\n{}", remaining.iter().map(|r| format!("--   {}", r.replace('\n', " "))).collect::<Vec<_>>().join("\n"), texts[0]);
        Ok(Ok(drafted(texts, 1, notes, false)))
    }
    .await;
    shadows.finish().await;
    server.close().await;
    result
}

async fn dump(opts: &Options, schemas: &[String]) -> Result<String, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let result = dump_from(&mut db, opts, schemas).await;
    db.close().await;
    result
}

async fn dump_from(db: &mut Db, opts: &Options, schemas: &[String]) -> Result<String, Error> {
    match db.dialect() {
        Dialect::Postgres => {
            let version: i64 = db.rows("select current_setting('server_version_num') as v").await?.first().and_then(|r| r.first()?.parse().ok()).unwrap_or(0);
            let mut args: Vec<String> = ["--schema-only", "--no-publications", "--no-subscriptions", "--no-security-labels", "--no-tablespaces"].map(String::from).to_vec();
            // Owners are kept where they aren't the role the file will run as, which owns what it makes.
            let runner = db.rows("select quote_ident(current_user) as u").await?.first().map(|r| r.text("u").to_string()).unwrap_or_default();
            args.push(format!("--exclude-table=*.{HISTORY_TABLE}"));
            let named_schemas: Vec<String> = if schemas.is_empty() { vec!["public".into()] } else { schemas.to_vec() };
            for s in &named_schemas {
                args.push("--schema".into());
                args.push(s.clone());
            }
            let text = crate::db::pg_dump(&opts.url, version / 10000, &args)?;
            // psql's meta-commands, which pg_dump writes on lines of their own around the file.
            static META: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\\(un)?restrict\b.*$").expect("a valid pattern"));
            let text = META.replace_all(&text, "");
            // What pg_dump writes for a session of its own: its settings (an empty search_path would
            // outlive the file in a shadow's session), and comments, which fall between statements.
            // Read as statements, so a body or a string that holds such a line keeps it.
            let own = format!(" OWNER TO {runner}");
            // A platform's default privileges are its own, which only its roles may set, and which
            // every shadow is given as they are.
            let platform_default = |s: &str| crate::catalog::PLATFORM_ROLES.iter().any(|r| s.starts_with(&format!("ALTER DEFAULT PRIVILEGES FOR ROLE {r} ")));
            let mut kept: Vec<String> = Vec::new();
            // The extensions the dumped schemas use, which pg_dump writes only for the schema it
            // dumps them in: the platform's own aside, made where they are, if they aren't already.
            for r in db
                .rows(&format!(
                    "select quote_ident(x.extname) as name, quote_ident(n.nspname) as schema, n.nspname as raw from pg_extension x join pg_namespace n on n.oid = x.extnamespace where x.extname <> 'plpgsql' and {}",
                    crate::catalog::not_platform("x.extowner")
                ))
                .await?
            {
                // Its schema first, dumped or not: pg_dump's own `create schema`, later, then finds it there.
                if r.text("raw") != "public" {
                    kept.push(format!("CREATE SCHEMA IF NOT EXISTS {}", r.text("schema")));
                }
                kept.push(format!("CREATE EXTENSION IF NOT EXISTS {} WITH SCHEMA {}", r.text("name"), r.text("schema")));
            }
            // What every new database has already: its public schema, as initdb makes it.
            const AS_MADE: [&str; 4] = ["CREATE SCHEMA public", "CREATE SCHEMA IF NOT EXISTS public", "ALTER SCHEMA public OWNER TO pg_database_owner", "COMMENT ON SCHEMA public IS 'standard public schema'"];
            for statement in crate::sql::split_statements(&text, Dialect::Postgres) {
                let s = statement.text.as_str();
                if s.starts_with("SET ") || s.starts_with("SELECT pg_catalog.set_config") || (s.starts_with("ALTER ") && s.ends_with(&own)) || platform_default(s) || s.starts_with("CREATE EXTENSION ") || AS_MADE.contains(&s) {
                    continue;
                }
                kept.push(match s.strip_prefix("CREATE SCHEMA ") {
                    Some(rest) if !rest.starts_with("IF NOT EXISTS ") => format!("CREATE SCHEMA IF NOT EXISTS {rest}"),
                    _ => s.to_string(),
                });
            }
            let mut joined = kept.iter().map(|s| format!("{s};")).collect::<Vec<_>>().join("\n\n");
            // What the project made on a platform's tables (Supabase's `auth.users`, `storage.objects`),
            // which no schema dumped holds: its triggers, whose function is no platform's, and its
            // policies, which pg_dump writes only with their table.
            let list = named_schemas.iter().map(|s| crate::db::literal(s)).collect::<Vec<_>>().join(", ");
            let extra = db
                .rows(&format!(
                    "select pg_get_triggerdef(t.oid) || ';' as s from pg_trigger t join pg_class c on c.oid = t.tgrelid join pg_namespace n on n.oid = c.relnamespace join pg_proc f on f.oid = t.tgfoid
                    where not t.tgisinternal and t.tgparentid = 0 and n.nspname not in ({list}) and not {} and {}
                    union all
                    select format('CREATE POLICY %I ON %I.%I AS %s FOR %s TO %s%s%s;', p.polname, n.nspname, c.relname, case when p.polpermissive then 'PERMISSIVE' else 'RESTRICTIVE' end,
                      case p.polcmd when 'r' then 'SELECT' when 'a' then 'INSERT' when 'w' then 'UPDATE' when 'd' then 'DELETE' else 'ALL' end,
                      (select string_agg(case when r = 0 then 'public' else quote_ident(pg_get_userbyid(r)) end, ', ' order by 1) from unnest(p.polroles) r),
                      coalesce(' USING (' || pg_get_expr(p.polqual, p.polrelid) || ')', ''), coalesce(' WITH CHECK (' || pg_get_expr(p.polwithcheck, p.polrelid) || ')', ''))
                    from pg_policy p join pg_class c on c.oid = p.polrelid join pg_namespace n on n.oid = c.relnamespace
                    where n.nspname not in ({list}) and not {}",
                    crate::catalog::not_platform("c.relowner"),
                    crate::catalog::not_platform("f.proowner"),
                    crate::catalog::not_platform("c.relowner")
                ))
                .await?;
            if !extra.is_empty() {
                joined.push_str("\n\n");
                joined.push_str(&extra.iter().map(|r| r.text("s").to_string()).collect::<Vec<_>>().join("\n\n"));
            }
            let sql = joined.trim().to_string();
            // pg_dump makes functions before the tables a SQL function's body may read, which Postgres
            // checks as the function is made, so the check is off while the file runs, as in pg_dump's
            // own output.
            static SQL_BODY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*LANGUAGE sql\b").expect("a valid pattern"));
            Ok(if SQL_BODY.is_match(&sql) { format!("set check_function_bodies = off;\n\n{sql}\n\nreset check_function_bodies;\n") } else { format!("{sql}\n") })
        }
        Dialect::Sqlite => {
            let rows = db
                .rows(&format!(
                    "select sql from sqlite_schema where sql is not null and name not like 'sqlite_%' and tbl_name <> '{HISTORY_TABLE}'
                      and name not in (select name from pragma_table_list where type = 'shadow')
                    order by case type when 'table' then 0 when 'index' then 1 when 'view' then 2 else 3 end, rowid"
                ))
                .await?;
            Ok(format!("{}\n", rows.iter().map(|r| format!("{};", r.text("sql"))).collect::<Vec<_>>().join("\n\n")))
        }
        Dialect::Mysql => {
            static AUTO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" AUTO_INCREMENT=\d+").expect("a valid pattern"));
            static DEFINER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" DEFINER=\S+").expect("a valid pattern"));
            let mut out = vec!["set foreign_key_checks = 0;".to_string()];
            let tables = db.rows(&format!("select table_name as n, table_type as t from information_schema.tables where table_schema = database() and table_name <> '{HISTORY_TABLE}' order by table_type, table_name")).await?;
            for t in tables.iter().filter(|t| t.text("t") == "BASE TABLE") {
                let row = db.rows(&format!("show create table `{}`", t.text("n"))).await?;
                out.push(format!("{};", AUTO.replace(row.first().map(|r| r.text("Create Table")).unwrap_or_default(), "")));
            }
            // Each view after the views it reads.
            let mut views: Vec<String> = tables.iter().filter(|t| t.text("t") == "VIEW").map(|t| t.text("n").to_string()).collect();
            let reads: Vec<(String, String)> = db
                .rows("select view_name as v, table_name as t from information_schema.view_table_usage where view_schema = database() and table_schema = database()")
                .await
                .map(|rows| rows.iter().map(|r| (r.text("v").to_string(), r.text("t").to_string())).collect())
                .unwrap_or_default();
            let mut ordered: Vec<String> = Vec::new();
            while !views.is_empty() {
                let ready = views.iter().position(|v| !reads.iter().any(|(of, t)| of == v && t != v && views.contains(t))).unwrap_or(0);
                ordered.push(views.remove(ready));
            }
            for name in &ordered {
                let row = db.rows(&format!("show create view `{name}`")).await?;
                out.push(format!("{};", DEFINER.replace(row.first().map(|r| r.text("Create View")).unwrap_or_default(), "")));
            }
            for r in db.rows("select routine_name as n, routine_type as t from information_schema.routines where routine_schema = database()").await? {
                let kind = r.text("t");
                let row = db.rows(&format!("show create {} `{}`", kind.to_lowercase(), r.text("n"))).await?;
                let column = if kind == "PROCEDURE" { "Create Procedure" } else { "Create Function" };
                out.push(format!("{};", DEFINER.replace(row.first().map(|x| x.text(column)).unwrap_or_default(), "")));
            }
            for r in db.rows("select trigger_name as n from information_schema.triggers where trigger_schema = database()").await? {
                let row = db.rows(&format!("show create trigger `{}`", r.text("n"))).await?;
                out.push(format!("{};", DEFINER.replace(row.first().map(|x| x.text("SQL Original Statement")).unwrap_or_default(), "")));
            }
            // Recurring events, as the catalog reads them: a one-time event is gone once it fires.
            for r in db.rows("select event_name as n from information_schema.events where event_schema = database() and event_type <> 'ONE TIME'").await? {
                let row = db.rows(&format!("show create event `{}`", r.text("n"))).await?;
                out.push(format!("{};", DEFINER.replace(row.first().map(|x| x.text("Create Event")).unwrap_or_default(), "")));
            }
            out.push("set foreign_key_checks = 1;".into());
            Ok(format!("{}\n", out.join("\n\n")))
        }
    }
}

/// `reylo migrate init`: writes `schema.sql` and `migrations/init.sql` from the database, then
/// records init.sql without running it, once a shadow shows the database already has its schema.
pub async fn init(opts: &Options, schemas: &[String], log: Log<'_>) -> Result<i32, Error> {
    let started = if opts.schema.exists() { Some(shown(&opts.schema)) } else if !read_migrations(&opts.dir)?.0.is_empty() { Some(shown(&opts.dir)) } else { None };
    if let Some(what) = started {
        let problem = Problem::new("project-started", format!("This project has started already ({what} {})", if what.ends_with(".sql") { "exists" } else { "holds migrations" }))
            .fix("To see this database's schema as SQL", &["reylo db introspect"]);
        return Ok(refuse(&[problem], log));
    }
    let home = opts.schema.parent().map(|p| format!("{}/", shown(p))).unwrap_or_default();
    let branch = |last: bool| if last { "└──" } else { "├──" };
    // A file of the tree, with what it is, dim.
    let entry = |prefix: &str, name: &str, what: &str| {
        let head = format!("{prefix} {name}");
        if what.is_empty() { format!("  {head}") } else { format!("  {head:<24}{}", paint("dim", what)) }
    };
    let file_name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let dir_name = format!("{}/", file_name(&opts.dir));
    // An empty database has no history to start: the first migration is `new`'s. The lock starts
    // empty, holding the checksum of the empty schema.sql the no migrations reach, so the project
    // keeps linear history from its first migration.
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let lines = catalog(&mut db).await;
    db.close().await;
    if !holds_a_schema(&lines?) {
        std::fs::create_dir_all(&opts.dir)?;
        std::fs::write(&opts.schema, "")?;
        lock::write(&opts.lock, &lock::Lock { schema: Some(lock::schema_sum("", "")), entries: vec![] })?;
        log(&format!("{} Started {home} for an empty database", paint("green", "✓")));
        log(&format!("  {home}"));
        log(&entry(branch(false), &file_name(&opts.schema), "empty: declare your tables here"));
        log(&entry(branch(false), &dir_name, ""));
        log(&entry(branch(true), &file_name(&opts.lock), ""));
        log(&format!("Then write the first migration: {}", paint("cyan", "reylo migrate new <name>")));
        return Ok(0);
    }
    let sql = dump(opts, schemas).await?;
    std::fs::create_dir_all(&opts.dir)?;
    std::fs::write(&opts.schema, &sql)?;
    let init = opts.dir.join(INIT);
    std::fs::write(&init, &sql)?;
    // The history is the lock's: init.sql is its first line, and reaches schema.sql.
    let entries = lock::chain([(INIT, sql.as_str())]);
    let schema = Some(lock::schema_sum(&entries[0].sum, &sql));
    lock::write(&opts.lock, &lock::Lock { schema, entries })?;
    // init.sql is recorded as run, which the tree says; what recording it prints is said only where
    // it didn't.
    let mut said: Vec<String> = Vec::new();
    let code = history_push(opts, &mut |l: &str| said.push(l.to_string())).await?;
    log(&format!("{} Started {home} from {}", paint("green", "✓"), shown_url(&opts.url)));
    log(&format!("  {home}"));
    log(&entry(branch(false), &file_name(&opts.schema), "this database's schema"));
    log(&entry(branch(false), &dir_name, ""));
    log(&entry(&format!("│   {}", branch(true)), INIT, if code == 0 { "the same, recorded as run here" } else { "the same" }));
    log(&entry(branch(true), &file_name(&opts.lock), "the order the migrations run in"));
    if code != 0 {
        for line in &said {
            log(line);
        }
    }
    Ok(code)
}

/// `reylo db introspect`: the database's schema as SQL that makes it, as `init` writes schema.sql;
/// nothing for an empty database. Writes no file and changes no database: saving it over schema.sql,
/// whose comments and layout it doesn't keep, is a redirect.
pub async fn db_introspect(opts: &Options, schemas: &[String]) -> Result<String, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let lines = catalog(&mut db).await;
    db.close().await;
    Ok(if holds_a_schema(&lines?) { dump(opts, schemas).await? } else { String::new() })
}

// ---------------------------------------------------------------------------------------------
// The development commands: `dev`, a database taken to its migrations; `db push`, one at its migrations
// taken to schema.sql.

/// Runs statements the diff engine wrote on the database, with `then` (statements of the history's own),
/// and keeps the result only if the database then reads as `expect`; `Ok(Err(lines))` says how it
/// differs. Where the database has transactional DDL, it is all one transaction, rolled back on a
/// difference, so nothing has changed; on MySQL, which commits each statement, it is checked after.
async fn run_to(db: &mut Db, plan: &[String], then: &[String], expect: &[String]) -> Result<Result<(), Vec<String>>, Error> {
    let d = db.dialect();
    let expect = crate::catalog::without_comments(expect);
    let expect = &expect[..];
    let statements: Vec<String> = plan.iter().filter(|s| !s.starts_with("--")).map(|s| format!("{};", s.trim_end().trim_end_matches(';'))).chain(then.iter().cloned()).collect();
    let differs = |now: &[String]| describe_diff("this database", now, "the target", expect);
    if d == Dialect::Mysql || statements.iter().any(|s| runs_alone(s, d)) {
        for s in &statements {
            db.script(s).await?;
        }
        let now = crate::catalog::without_comments(&crate::catalog::without_extra_children(&catalog(db).await?, expect));
        return Ok(if now == expect { Ok(()) } else { Err(differs(&now)) });
    }
    if d == Dialect::Sqlite {
        db.exec("pragma foreign_keys = off").await?;
    }
    db.exec("begin").await?;
    let result = async {
        db.script(&statements.join("\n")).await?;
        if d == Dialect::Sqlite && !db.rows("pragma foreign_key_check").await?.is_empty() {
            return Err(Error("rows would break a foreign key".into()));
        }
        let now = crate::catalog::without_comments(&crate::catalog::without_extra_children(&catalog(db).await?, expect));
        if now != expect {
            return Ok(Err(differs(&now)));
        }
        db.exec("commit").await?;
        Ok(Ok(()))
    }
    .await;
    if !matches!(result, Ok(Ok(()))) {
        let _ = db.exec("rollback").await;
    }
    if d == Dialect::Sqlite {
        db.exec("pragma foreign_keys = on").await?;
    }
    result
}

/// Takes the database to `target`: a table that goes while one of its columns comes renamed first,
/// keeping its rows, then the diff engine's plan from there, then `then` (statements of the
/// history's own). Kept only if the database then reads as `expect`, all in one transaction where
/// the database has transactional DDL, so nothing has changed otherwise; on MySQL, which commits
/// each statement, checked after. Gives the renames and the plan it ran.
async fn take_to(db: &mut Db, target: &Target, then: &[String], expect: &[String]) -> Result<Result<(Vec<String>, crate::diff::Plan), Vec<String>>, Error> {
    let d = db.dialect();
    let expect = crate::catalog::without_comments(expect);
    let expect = &expect[..];
    let transactional = d != Dialect::Mysql;
    if d == Dialect::Sqlite {
        db.exec("pragma foreign_keys = off").await?;
    }
    if transactional {
        db.exec("begin").await?;
    }
    let result = async {
        let (renames, plan) = plan_to(db, target, true).await?;
        for s in plan.runnable().chain(then) {
            db.script(&format!("{};", s.trim_end().trim_end_matches(';'))).await?;
        }
        if d == Dialect::Sqlite && !db.rows("pragma foreign_key_check").await?.is_empty() {
            return Err(Error("rows would break a foreign key".into()));
        }
        let now = crate::catalog::without_comments(&crate::catalog::without_extra_children(&catalog(db).await?, expect));
        if now != expect {
            return Ok(Err(describe_diff("this database", &now, "the target", expect)));
        }
        if transactional {
            db.exec("commit").await?;
        }
        Ok(Ok((renames, plan)))
    }
    .await;
    if transactional && !matches!(result, Ok(Ok(_))) {
        let _ = db.exec("rollback").await;
    }
    if d == Dialect::Sqlite {
        db.exec("pragma foreign_keys = on").await?;
    }
    result
}

/// The plan from the database `from` to the database `to`, as `create` would write it (from
/// `declared`, the text that built `to`), styled as schema.sql is.
async fn plan_between(from: &mut Db, to: &mut Db, declared: Option<&str>) -> Result<crate::diff::Plan, Error> {
    let mut target = crate::diff::read(to).await?;
    if let (Some(sql), Dialect::Postgres) = (declared, to.dialect()) {
        crate::diff::postgres::declare(to, &mut target, sql).await?;
    }
    let current = crate::diff::read(from).await?;
    let alterer = crate::diff::alterer(from).await?;
    let mut plan = crate::diff::plan(&current, &target, alterer.as_ref());
    style(from, &mut plan, &target).await?;
    Ok(plan)
}


/// The refusal of a development command where CI is set: nothing it does is wanted there, and what
/// it would write (the lock) would go uncommitted.
pub fn refused_in_ci(command: &str, log: Log<'_>) -> i32 {
    let problem = Problem::new("refused-in-ci", format!("CI is set, and {command} is for development databases")).fix("In CI, lint the migrations, try them, or run them", &["reylo migrate lint", "reylo migrate deploy --dry-run", "reylo migrate deploy"]);
    refuse(&[problem], log)
}

/// `reylo migrate dev`: takes a development database to its migrations, whatever its
/// history. The history is kept as far as it agrees with the migration files: where a migration it
/// applied was edited or is gone (another branch's), the database is rewound to that point by the
/// diff engine's plan from what those migrations made, losing what it drops. The migrations after it
/// then run for real, their data statements included; one with no data statements whose change the
/// database already has (`db push` made it) is recorded instead. Never in CI (`in_ci`, from the `CI`
/// variable).
pub async fn dev(opts: &Options, in_ci: bool, log: Log<'_>) -> Result<i32, Error> {
    if in_ci {
        return Ok(refused_in_ci("reylo migrate dev", log));
    }
    let (mut db, mut server, swept) = open(opts).await?;
    let result = dev_on(&mut db, &mut server, swept, opts, &format!("Applying to {}", shown_url(&opts.url)), log).await;
    db.close().await;
    server.close().await;
    result
}

/// The files in the migrations directory that aren't migrations, as one refusal: a `.sql` file that
/// lacks only the time is told the name it needs.
fn stray_problem(opts: &Options, strays: &[String]) -> Option<Problem> {
    static BARE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9_]+\.sql$").expect("a valid pattern"));
    let dir = shown(&opts.dir);
    let title = match strays {
        [] => return None,
        [one] => format!("{one} in {dir} isn't a migration"),
        many => format!("{} files in {dir} aren't migrations: {}", many.len(), many.join(", ")),
    };
    let mut problem = Problem::new("not-a-migration", title).note(format!("A file in {dir} is init.sql or <YYYYMMDDHHMMSS>_<name>.sql, the name in a-z, 0-9 and _."));
    for s in strays.iter().filter(|s| BARE.is_match(s)) {
        problem = problem.note(format!("Rename {s} to {}_{s}.", time::stamp(time::now_seconds())));
    }
    Some(problem)
}

/// The problems no database is needed to see: files that aren't migrations, a migration that can't
/// run atomically, and a text (a migration's, or schema.sql's) a shadow can't replay.
fn file_problems(opts: &Options, files: &[Migration], strays: &[String], schema_sql: Option<&str>, d: Dialect) -> Vec<Problem> {
    let mut problems = Vec::new();
    let conflicted = |sql: &str| sql.lines().any(|l| l.starts_with("<<<<<<< ") || l.starts_with(">>>>>>> ") || l == "=======");
    for (path, sql) in files.iter().map(|f| (shown(&f.path), f.sql.as_str())).chain(schema_sql.map(|s| (shown(&opts.schema), s))) {
        if conflicted(sql) {
            problems.push(Problem::new("merge-conflict-markers", format!("{path} has a merge conflict in it")).note("Resolve git's conflict markers (<<<<<<<, =======, >>>>>>>) first."));
        }
    }
    problems.extend(stray_problem(opts, strays));
    for f in files.iter().filter(|f| f.name != INIT) {
        if let Some(why) = atomicity_problem(&f.sql, d) {
            problems.push(Problem::new("not-atomic", format!("{} can't run atomically", f.name)).note(why));
        }
    }
    let schema_name = opts.schema.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "schema.sql".into());
    for (name, sql) in files.iter().map(|f| (f.name.as_str(), f.sql.as_str())).chain(schema_sql.map(|s| (schema_name.as_str(), s))) {
        if let Some(why) = shadow_problem(sql, d) {
            problems.push(Problem::new("not-replayable", format!("{name} can't be replayed in a shadow database")).note(why));
        }
    }
    problems
}

async fn dev_on(db: &mut Db, server: &mut Server, swept: usize, opts: &Options, heading: &str, log: Log<'_>) -> Result<i32, Error> {
    swept_note(swept, log);
    let _held = lock(db, opts).await?;
    let d = db.dialect();
    let (files, strays) = read_migrations(&opts.dir)?;
    let mut problems = file_problems(opts, &files, &strays, std::fs::read_to_string(&opts.schema).ok().as_deref(), d);
    let history = read_history(db).await?;
    problems.extend(missing_dir(opts, &history));
    // What the lock's rewrite says of it, printed once it is written.
    let mut notes = Vec::new();
    if problems.is_empty() {
        match dev_lock_notes(opts, &files, &history) {
            Ok(said) => notes = said,
            Err(problem) => problems.push(problem),
        }
    }
    if !problems.is_empty() {
        return Ok(refuse(&problems, log));
    }
    let files = run_order(opts, &files);
    let left = leftovers(db, &history).await?;
    drop_leftovers(db, &left, log).await?;
    if history.iter().any(|r| !r.applied) {
        // A row marked started is a pending migration's: written again when it runs, or is recorded.
        db.exec(&format!("delete from {HISTORY_TABLE} where applied_at is null")).await?;
    }
    let applied: Vec<&HistoryRow> = history.iter().filter(|r| r.applied).collect();
    // The history as far as it agrees with the files: each row's file there, where the lock puts it,
    // with the text it ran.
    let agree = if lock_free(opts) {
        // With no lock, its order is the database's own, as far as each migration still reads as it ran.
        applied.iter().take_while(|r| files.iter().any(|f| f.name == r.name && same_statements(&f.sql, &r.sql, d))).count()
    } else {
        applied.iter().zip(&files).take_while(|(r, f)| r.name == f.name && same_statements(&f.sql, &r.sql, d)).count()
    };
    let live = catalog(db).await?;
    let schema_sql = std::fs::read_to_string(&opts.schema).ok();
    if left.is_empty() && !history.iter().any(|r| !r.applied) && agree == applied.len() {
        if let Some(code) = dev_without_shadows(db, opts, &files, &applied, schema_sql.as_deref(), &live, heading, log).await? {
            return Ok(code);
        }
    }
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain(history.iter().map(|r| r.sql.as_str())).chain(schema_sql.as_deref()).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &texts).await;
    let result = dev_in_shadows(&mut shadows, db, opts, &files, &applied, agree, &live, heading, log).await;
    let result = match result {
        Ok(code) => async {
            // A run that failed part of the way still locks what ran before it failed.
            let ran = |h: &[HistoryRow]| h.iter().filter(|r| r.applied).map(|r| (r.name.clone(), r.sql.clone())).collect::<Vec<_>>();
            if code == 0 || ran(&read_history(db).await?) != ran(&history) {
                relock(&mut shadows, db, opts, &files, schema_sql.as_deref(), &notes, code == 0, log).await?;
            }
            if code == 0 {
                replayed_fingerprint(&mut shadows, db, &files).await?;
            }
            Ok(code)
        }
        .await,
        other => other,
    };
    shadows.finish().await;
    result
}

/// `dev` where every pair already agrees, with no replay: schema.sql with the lock's checksum of it,
/// the history with the files (in the lock's order), and the database with the fingerprint its last
/// run stored. What is pending then runs as deploy runs it: already locked, or with no lock, none of
/// it sorting before what ran. None where a pair can't be shown to agree this way, for the shadows to
/// settle.
#[allow(clippy::too_many_arguments)]
async fn dev_without_shadows(db: &mut Db, opts: &Options, files: &[Migration], applied: &[&HistoryRow], schema_sql: Option<&str>, live: &[String], heading: &str, log: Log<'_>) -> Result<Option<i32>, Error> {
    let d = db.dialect();
    let Some(stored) = applied.last().and_then(|r| r.fingerprint.clone()) else { return Ok(None) };
    let pending: Vec<&Migration> = files.iter().filter(|f| !applied.iter().any(|r| r.name == f.name)).collect();
    if lock_free(opts) {
        let last = applied.iter().map(|r| name_order(&r.name)).max();
        let in_order = applied.windows(2).all(|w| name_order(&w[0].name) <= name_order(&w[1].name));
        if !in_order || pending.iter().any(|m| last.is_some_and(|last| name_order(&m.name) < last)) {
            return Ok(None);
        }
    } else {
        let Ok(l) = read_lock(opts) else { return Ok(None) };
        // Every line of the lock links to its file as it was locked: one edited or gone is relocked
        // by the shadows' path.
        if !links(&l, files).iter().all(|link| matches!(link, Link::Ok)) || !pending.iter().all(|m| l.entries.iter().any(|e| e.name == m.name)) || !matches!(stamp(opts, schema_sql), Stamp::Proven | Stamp::NoSchema) {
            return Ok(None);
        }
    }
    if stored != fingerprint_of(db, live).await? {
        return Ok(None);
    }
    let mut rows = Rows::new(heading.to_string(), pending.iter().map(|m| m.name.as_str()));
    if !pending.is_empty() {
        ensure_history(db).await?;
    }
    for m in &pending {
        let started = std::time::Instant::now();
        if let Err(e) = run(db, m, true).await {
            failed(&mut rows, m, &e, d, log);
            return Ok(Some(1));
        }
        rows.row(log, &paint("green", "✓"), &m.name, &format!("applied {}", paint("dim", &format!("· {} ms", started.elapsed().as_millis()))));
    }
    if pending.is_empty() {
        log(&format!("{} Up to date with the migrations", paint("green", "✓")));
    }
    Ok(Some(0))
}

/// What `dev` leaves for `deploy` to compare with: the fingerprint of what the migrations make, not of
/// this database, which may hold more that no migration made.
async fn replayed_fingerprint(shadows: &mut Shadows<'_>, db: &mut Db, files: &[Migration]) -> Result<(), Error> {
    let history = read_history(db).await?;
    let Some(last) = history.iter().rfind(|r| r.applied) else { return Ok(()) };
    let ran: Vec<&Migration> = history.iter().filter(|r| r.applied).filter_map(|r| files.iter().find(|f| f.name == r.name)).collect();
    let lane = shadows.lane();
    shadows.push(lane, ran.iter().map(|m| Step::of(m)));
    if let Ok(Ok(lines)) = shadows.lines(lane).await {
        let value = fingerprint_of(db, &lines).await?;
        ensure_history(db).await?;
        set_fingerprint(db, &last.name, &value).await?;
        store_catalog(db, &last.name, &lines).await?;
    }
    Ok(())
}

/// What `dev` says of the lock as it takes it, where the files no longer agree with it: it runs
/// them in the lock's order and writes the lock again, so a line edited or whose file is gone is
/// locked as the files now read, and so is one merged out of its order that this database ran (this
/// branch's own, after main's). One merged out of its order that this database never ran came from
/// another branch, placed after this branch's: refused, before it is locked so. A lock it can't
/// read (git's conflict markers in it) is refused too. Every database's deploy still refuses a
/// history that isn't a prefix of the lock's order, or an edit to a migration it ran.
fn dev_lock_notes(opts: &Options, files: &[Migration], history: &[HistoryRow]) -> Result<Vec<String>, Problem> {
    // A missing lock is written as the migrations apply.
    if lock::read(&opts.lock) == lock::State::Missing {
        return Ok(vec![]);
    }
    let l = read_lock(opts)?;
    let path = shown(&opts.lock);
    let ran_here = |name: &str| history.iter().any(|r| r.applied && r.name == name);
    let mut notes = Vec::new();
    for (e, link) in l.entries.iter().zip(links(&l, files)) {
        match link {
            Link::Ok => {}
            Link::Missing => notes.push(format!("{} comes out of the lock: it isn't in {}", e.name, shown(&opts.dir))),
            Link::Edited => notes.push(format!("{} is locked again as it now reads\nA database that ran the old text refuses it: if one did, put the change in a new migration instead.", e.name)),
            Link::Moved { .. } if ran_here(&e.name) => notes.push(format!("The lock was merged out of order at {}: it now lists them in the order they ran here", e.name)),
            Link::Moved { after } => {
                return Err(Problem::new("lock-out-of-order", format!("{path} was merged out of order at {}, which this database never ran", e.name))
                    .note(format!("It came from another branch, and was locked {}. In {path}, keep main's lines, in main's order, and delete this branch's.", after.map_or("first".to_string(), |a| format!("right after {a}"))))
                    .fix("Then run this branch's migrations after main's, which locks them again", &["reylo migrate dev"]))
            }
        }
    }
    Ok(notes)
}

/// Writes the lock after `dev`: what the database applied, in the order it
/// did, chained; and schema.sql's checksum where the migrations reach it.
#[allow(clippy::too_many_arguments)]
async fn relock(shadows: &mut Shadows<'_>, db: &mut Db, opts: &Options, files: &[Migration], schema_sql: Option<&str>, notes: &[String], whole: bool, log: Log<'_>) -> Result<(), Error> {
    let (next, reach) = locked(shadows, db, opts, files, schema_sql).await?;
    let schema = shown(&opts.schema);
    // What schema.sql is against the migrations, after a run that finished.
    match reach.filter(|_| whole) {
        Some(Reach::SchemaAhead) => log(&format!("{} {schema} has what no migration makes yet: reylo migrate new <name> writes it", paint("yellow", "!"))),
        Some(Reach::MigrationsAhead) => log(&format!("{} The migrations make what {schema} doesn't declare: add it there, or take it out of the migration", paint("yellow", "!"))),
        Some(Reach::Differ) => log(&format!("{} {schema} and the migrations disagree: reylo migrate deploy --dry-run shows how", paint("yellow", "!"))),
        Some(Reach::SchemaFails(e)) => log(&format!("{} {schema} doesn't run on an empty database: {}", paint("yellow", "!"), e.lines().next().unwrap_or_default())),
        Some(Reach::Same) | None => {}
    }
    // A project with no lock keeps none: reylo migrate lock writes one.
    if lock_free(opts) {
        return Ok(());
    }
    if let Some(next) = next {
        lock::write(&opts.lock, &next)?;
        let through = if whole { String::new() } else { next.entries.last().map(|e| format!(", through {}", e.name)).unwrap_or_default() };
        log(&format!("{} Wrote {}{through}", paint("green", "✓"), shown(&opts.lock)));
        for note in notes {
            for (k, line) in note.lines().enumerate() {
                log(&if k == 0 { format!("  {} {line}", paint("yellow", "!")) } else { format!("    {line}") });
            }
        }
    }
    Ok(())
}

/// What a replay of the migrations is against schema.sql's.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Reach {
    Same,
    /// schema.sql has what the migrations don't make.
    SchemaAhead,
    /// The migrations make what schema.sql doesn't declare.
    MigrationsAhead,
    /// Each has what the other doesn't.
    Differ,
    /// schema.sql doesn't run, with why.
    SchemaFails(String),
}

/// The lock as the database's history makes it: the migrations it ran, in its order, with
/// schema.sql's checksum where their replay reaches schema.sql. None where the lock already is that.
async fn locked(shadows: &mut Shadows<'_>, db: &mut Db, opts: &Options, files: &[Migration], schema_sql: Option<&str>) -> Result<(Option<lock::Lock>, Option<Reach>), Error> {
    let history = read_history(db).await?;
    let ran: Vec<&Migration> = history.iter().filter(|r| r.applied).filter_map(|r| files.iter().find(|f| f.name == r.name)).collect();
    let texts: Vec<(String, String)> = ran.iter().map(|f| (f.name.clone(), normalized(&f.sql))).collect();
    let entries = lock::chain(texts.iter().map(|(n, t)| (n.as_str(), t.as_str())));
    let before = match lock::read(&opts.lock) {
        lock::State::Lock(l) => Some(l),
        _ => None,
    };
    // The migrations' replay against schema.sql's, where there is one. Where they don't reach it, the
    // checksum from the last time they did stays: it no longer matches, so lint refuses until a
    // migration makes what schema.sql has.
    let mut reached = None;
    let mut schema = before.as_ref().and_then(|l| l.schema.clone());
    if let Some(sql) = schema_sql {
        let ours = shadows.lane();
        shadows.push(ours, ran.iter().map(|m| Step::of(m)));
        let theirs = shadows.lane();
        shadows.push(theirs, [Step::script("schema.sql", sql)]);
        let reach = match (shadows.lines(ours).await?, shadows.lines(theirs).await?) {
            (Ok(a), Ok(b)) if a == b => Reach::Same,
            (Ok(a), Ok(b)) => match difference(&a, &b) {
                (made, declared) if made.is_empty() && !declared.is_empty() => Reach::SchemaAhead,
                (made, declared) if !made.is_empty() && declared.is_empty() => Reach::MigrationsAhead,
                _ => Reach::Differ,
            },
            (_, Err((_, e))) => Reach::SchemaFails(e),
            (Err(_), Ok(_)) => Reach::Differ,
        };
        if reach == Reach::Same {
            schema = Some(lock::schema_sum(&entries.last().map(|e| e.sum.clone()).unwrap_or_default(), sql));
        }
        reached = Some(reach);
    }
    let next = lock::Lock { schema, entries };
    Ok(((before.as_ref() != Some(&next)).then_some(next), reached))
}

/// The refusal of a development database that has changes nothing in the project makes, before
/// anything is changed: what it has beyond what its history ran (as it ran it) that schema.sql
/// doesn't declare (`db push` made what it does), and the next migrations, none of which changes
/// data, don't make either (made by hand, then written). Shown as the SQL from its history to it.
#[allow(clippy::too_many_arguments)]
async fn unexplained(shadows: &mut Shadows<'_>, db: &mut Db, opts: &Options, kept: &[Migration], rewound: &[&&HistoryRow], pending: &[Migration], at_kept: &[String], live: &[String]) -> Result<Option<Problem>, Error> {
    let d = db.dialect();
    let ran = shadows.lane();
    shadows.push(ran, kept.iter().map(Step::of).chain(rewound.iter().map(|r| Step::migration(&r.name, &r.sql))));
    let at_ran = match shadows.lines(ran).await? {
        Ok(lines) => lines,
        // Nothing can say what changed this database, so nothing is undone: refused, as a history that
        // doesn't replay is everywhere.
        Err((m, e)) => return Ok(Some(Problem::new("history-replay-fails", format!("{m} fails when replayed in a shadow database, as this database ran it")).note(e).note("So nothing can tell what else changed this database, and nothing is rewound.").fix("Rebuild this database from the migrations", &["reylo db reset"]))),
    };
    let at_ran = crate::catalog::without_comments(&crate::catalog::without_missing_children(&at_ran, live));
    let here = crate::catalog::without_comments(&crate::catalog::without_extra_children(live, &at_ran));
    let (extra, missing) = difference(&here, &at_ran);
    if extra.is_empty() && missing.is_empty() {
        return Ok(None);
    }
    // What schema.sql declares, where the project has one.
    let mut explainers: Vec<usize> = Vec::new();
    let declared = match std::fs::read_to_string(&opts.schema) {
        Ok(sql) => {
            let lane = shadows.lane();
            shadows.push(lane, [Step::script("schema.sql", &sql)]);
            match shadows.lines(lane).await? {
                Ok(lines) => {
                    explainers.push(lane);
                    Some(crate::catalog::without_comments(&lines))
                }
                Err(_) => None,
            }
        }
        Err(_) => None,
    };
    // What the pending migrations make: all of them, or where one fails, those before it. One that
    // changes data too explains a change it makes: it can't be recorded as in place, so what was made
    // by hand is undone below for it to run.
    let (mut adds, mut removes) = (vec![], vec![]);
    for n in [pending.len(), pending.iter().take_while(|m| data_statements(&m.sql, d).is_empty()).count()] {
        if n == 0 {
            continue;
        }
        let lane = shadows.lane();
        shadows.push(lane, kept.iter().chain(&pending[..n]).map(Step::of));
        if let Ok(lines) = shadows.lines(lane).await? {
            (adds, removes) = difference(&crate::catalog::without_comments(&lines), at_kept);
            explainers.push(lane);
            break;
        }
    }
    let explained_extra = |l: &String| adds.contains(l) || declared.as_ref().is_some_and(|d| d.contains(l));
    let explained_missing = |l: &String| removes.contains(l) || declared.as_ref().is_some_and(|d| !d.contains(l));
    if extra.iter().all(explained_extra) && missing.iter().all(explained_missing) {
        return Ok(None);
    }
    // What the refused change is: something this database has, something it lost, or both.
    let (adds_some, removes_some) = (!extra.iter().all(explained_extra), !missing.iter().all(explained_missing));
    let schema = shown(&opts.schema);
    let title = if declared.is_some() { format!("This database was changed outside {schema} and its migrations") } else { "This database has a change no migration makes yet".to_string() };
    let mut problem = Problem::new("unexplained-change", title);
    // The change refused, as SQL, and the statements that undo it and nothing else: not what schema.sql
    // or a pending migration explains, comments, or partitions made or dropped at run time.
    let mut explaining = Vec::new();
    for lane in explainers {
        if let Ok(Ok(())) = shadows.open(lane).await {
            shadows.taint(lane);
            if let Ok(schema) = crate::diff::read(shadows.db(lane)).await {
                explaining.push(schema);
            }
        }
    }
    let mut undo = None;
    if let Ok(Ok(())) = shadows.open(ran).await {
        shadows.taint(ran);
        match scoped_change(db, shadows.db(ran), &explaining).await {
            Some((sql, statements)) => {
                problem = problem.sql(&sql);
                undo = Some(statements);
            }
            // Where the refused lines can't all be told apart, the whole difference is shown, and
            // no command undoes it.
            None => {
                if let Some(sql) = plan_text(shadows.db(ran), db, None).await {
                    problem = problem.sql(&sql);
                }
            }
        }
    }
    // Kept by writing it as a migration: from schema.sql, or without one, from this database, which
    // `migrate new` reads only once every migration has run here.
    if declared.is_some() {
        let keep = match (adds_some, removes_some) {
            (true, false) => format!("To keep it, add it to {schema}, then write its migration"),
            (false, true) => format!("To keep it, take it out of {schema}, then write its migration"),
            _ => format!("To keep it, make {schema} match, then write its migration"),
        };
        problem = problem.fix(keep, &["reylo migrate new <name>"]);
    } else if pending.is_empty() {
        problem = problem.fix("To keep it, write it as a migration", &["reylo migrate new <name>"]);
    }
    problem = match undo {
        Some(statements) => problem.fix("To undo it", &[&format!("reylo run {}", shell_word(&statements.join(" ")))]),
        None => problem.note("To undo it, change it back by hand."),
    };
    Ok(Some(problem.fix("To rebuild this database from the migrations, losing its rows", &["reylo db reset"])))
}

/// The refused change on `db`, against its history replayed in `history`, as SQL, with the
/// statements that undo it alone. An object that differs from the history is refused unless one of
/// `explaining` (schema.sql, the pending migrations) has it as the database does, it is a comment,
/// or it is a partition, which is data; what is refused comes back from the history, and everything
/// else stays as it is on the database. None where nothing would change.
async fn scoped_change(db: &mut Db, history: &mut Db, explaining: &[crate::diff::Schema]) -> Option<(String, Vec<String>)> {
    let mut current = crate::diff::read(db).await.ok()?;
    current.live = true;
    let mut target = crate::diff::read(history).await.ok()?;
    let ids: std::collections::BTreeSet<String> = current.objects.keys().chain(target.objects.keys()).cloned().collect();
    let def = |o: Option<&crate::diff::Object>| o.map(|o| o.def.clone());
    let mut kept: Vec<(String, Option<crate::diff::Object>)> = Vec::new();
    let mut refused: Vec<String> = Vec::new();
    for id in &ids {
        let (now, then) = (current.objects.get(id), target.objects.get(id));
        if def(now) == def(then) {
            continue;
        }
        let data = now.or(then).is_some_and(crate::diff::is_child);
        if data || id.starts_with("comment ") || explaining.iter().any(|e| def(e.objects.get(id)) == def(now)) {
            kept.push((id.clone(), now.cloned()));
        } else {
            refused.push(id.clone());
        }
    }
    // Where a refused object holds one that is kept (on MySQL and SQLite a table's definition holds
    // its columns), the two can't be undone apart.
    if kept.iter().any(|(id, _)| current.objects.get(id).or(target.objects.get(id)).and_then(|o| o.owner.as_ref()).is_some_and(|owner| refused.contains(owner))) {
        return None;
    }
    for (id, now) in kept {
        match now {
            Some(object) => {
                target.objects.insert(id, object);
            }
            None => {
                target.objects.remove(&id);
            }
        }
    }
    let alterer = crate::diff::alterer(db).await.ok()?;
    let mut undo = crate::diff::plan(&current, &target, alterer.as_ref());
    style(db, &mut undo, &target).await.ok()?;
    let mut change = crate::diff::plan(&target, &current, alterer.as_ref());
    style(db, &mut change, &current).await.ok()?;
    let statements: Vec<String> = undo.runnable().map(|s| format!("{};", s.trim_end().trim_end_matches(';'))).collect();
    (!statements.is_empty() && !change.statements.is_empty()).then(|| (change.sql(), statements))
}

#[allow(clippy::too_many_arguments)]
async fn dev_in_shadows(shadows: &mut Shadows<'_>, db: &mut Db, opts: &Options, files: &[Migration], applied: &[&HistoryRow], agree: usize, live: &[String], heading: &str, log: Log<'_>) -> Result<i32, Error> {
    let d = db.dialect();
    let fail = |m: &str, error: String, log: Log<'_>| {
        print_problems(&[Problem::new("dev-failed", m.to_string()).note(error)], log);
        Ok(1)
    };
    let kept_of = |n: usize| -> Vec<Migration> { applied[..n].iter().filter_map(|r| files.iter().find(|f| f.name == r.name).cloned()).collect() };
    let keep = agree;
    let kept = &kept_of(keep)[..];
    let k_lane = shadows.lane();
    shadows.push(k_lane, kept.iter().map(Step::of));
    let at_kept = match shadows.lines(k_lane).await? {
        Ok(lines) => lines,
        Err((m, e)) => return fail(&format!("{m} fails when replayed in a shadow database"), e, log),
    };
    // Children of a table made at run time are data: never compared, never rewound. Comments are
    // written by what runs, and aren't compared with this database either.
    let at_kept = crate::catalog::without_comments(&at_kept);
    let live = crate::catalog::without_comments(&crate::catalog::without_extra_children(live, &at_kept));
    let live = &live[..];
    let mut pending: Vec<Migration> = files.iter().filter(|f| !kept.iter().any(|k| k.name == f.name)).cloned().collect();
    let rewind_rows: Vec<&&HistoryRow> = applied[keep..].iter().collect();
    if lock_free(opts) {
        if let Some(problem) = late_problem(shadows, kept, &pending, files).await? {
            return Ok(refuse(&[problem], log));
        }
    }
    let mut rows = Rows::new(heading.to_string(), rewind_rows.iter().map(|r| r.name.as_str()).chain(pending.iter().map(|m| m.name.as_str())));
    let mut records: Vec<Migration> = Vec::new();
    // The database's catalog as each step leaves it.
    let mut base: Vec<String> = live.to_vec();
    let refused = |what: &str, lines: Vec<String>, log: Log<'_>| {
        let problem = Problem::new("rewind-incomplete", what.to_string()).lines(lines);
        let problem = if d == Dialect::Mysql { problem.note("MySQL commits each statement, so it is part of the way there.") } else { problem.note("That step changed nothing.") };
        print_problems(&[problem.fix("To rebuild this database from the migrations, losing its rows", &["reylo db reset"])], log);
        Ok(1)
    };
    if let Some(problem) = unexplained(shadows, db, opts, kept, &rewind_rows, &pending, &at_kept, live).await? {
        return Ok(refuse(&[problem], log));
    }
    let counts = if rewind_rows.is_empty() && (base == at_kept || pending.is_empty()) { Default::default() } else { table_rows(db).await? };
    if !rewind_rows.is_empty() {
        ensure_history(db).await?;
        let quoted = |name: &str| format!("'{}'", name.replace('\'', "''"));
        let mut forget: Vec<String> = rewind_rows.iter().map(|r| format!("delete from {HISTORY_TABLE} where name = {};", quoted(&r.name))).collect();
        if d == Dialect::Mysql {
            // MySQL commits each statement: the rows go first, so a rewind cut off leaves the
            // migrations it undid unrecorded, to run again, never recorded over a schema without them.
            for statement in forget.drain(..) {
                db.exec(&statement).await?;
            }
        }
        if let Err((_, e)) = shadows.open(k_lane).await? {
            return fail("The history fails when replayed in a shadow database", e, log);
        }
        let k_target = read_target(shadows.db(k_lane), None).await?;
        // What the rewound migrations made, as this database recorded them, is undone, and nothing
        // else: the plan from their replay to the history kept, so that changes no migration made
        // stay, with their rows, and a table one renamed is renamed back.
        let h_lane = shadows.lane();
        shadows.push(h_lane, kept.iter().map(Step::of).chain(rewind_rows.iter().map(|r| Step::migration(&r.name, &r.sql))));
        let mut undone = None;
        if let Ok(Ok(at_applied)) = shadows.lines(h_lane).await {
            if let Ok(()) = shadows.open(h_lane).await? {
                shadows.taint(h_lane);
                let (renames, plan) = plan_to(shadows.db(h_lane), &k_target, false).await?;
                let (only_h, only_k) = difference(&crate::catalog::without_comments(&at_applied), &at_kept);
                let expect: Vec<String> = live.iter().filter(|l| !only_h.contains(l)).cloned().chain(only_k).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                let statements: Vec<String> = renames.iter().cloned().chain(plan.runnable().cloned()).collect();
                if let Ok(Ok(())) = run_to(db, &statements, &forget, &expect).await {
                    base = expect;
                    undone = Some(plan);
                }
            }
        }
        // Where that doesn't reach it, the database is taken to the history kept itself, which
        // undoes the changes no migration made too.
        let plan = match undone {
            Some(plan) => plan,
            None => match take_to(db, &k_target, &forget, &at_kept).await {
                Err(e) => return fail("Rewinding this database failed", e.0, log),
                Ok(Err(lines)) => return refused("Rewinding this database wouldn't reach what its migrations make", lines, log),
                Ok(Ok((_, plan))) => {
                    base = at_kept.clone();
                    plan
                }
            },
        };
        // The first rewound in its place gives way to what the lock now runs there; the rest after it
        // run again in the lock's order.
        let mut displaced = false;
        for r in &rewind_rows {
            let why = match files.iter().find(|f| f.name == r.name) {
                Some(f) if !same_statements(&f.sql, &r.sql, d) => "rewound: it changed since it ran".to_string(),
                Some(_) if !displaced => {
                    displaced = true;
                    match files.get(keep) {
                        Some(before) if before.name != r.name => format!("rewound: the lock now runs {} before it", before.name),
                        _ => "rewound, to run in the lock's order".to_string(),
                    }
                }
                Some(_) => "rewound, to run in the lock's order".to_string(),
                None => "rewound: it isn't in the migrations".to_string(),
            };
            rows.row(log, &paint("yellow", "↺"), &r.name, &why);
            let data = data_statements(&r.sql, d);
            if !data.is_empty() {
                log(&format!("    {} {}", paint("yellow", "!"), paint("yellow", &format!("What it wrote stays ({}): a rewind undoes its schema, not its rows (reylo db wipe deletes them)", data.join(", ")))));
            }
        }
        dropped_row(&plan, &counts, log);
    }
    // Changes no migration made, which schema.sql or the next migrations explain (`unexplained`
    // refused the rest), are kept, unless a migration still to run cannot run on top of them.
    if base != at_kept && !pending.is_empty() {
        // The next migrations, none of which changes data, whose change this database already has
        // (sync, then create, then sync) are recorded rather than run again: each line they add is
        // in the database, and none they remove.
        for j in (1..=pending.len()).rev() {
            if !pending[..j].iter().all(|m| data_statements(&m.sql, d).is_empty()) {
                continue;
            }
            let lane = shadows.lane();
            shadows.push(lane, kept.iter().chain(&pending[..j]).map(Step::of));
            if let Ok(Ok(lines)) = shadows.lines(lane).await {
                let (added, removed) = difference(&crate::catalog::without_comments(&lines), &at_kept);
                if (!added.is_empty() || !removed.is_empty()) && added.iter().all(|l| base.contains(l)) && !removed.iter().any(|l| base.contains(l)) {
                    records = pending.drain(..j).collect();
                    break;
                }
            }
        }
        // The rest is tried first on a shadow database made to read as this one.
        let mut runs = pending.is_empty();
        if !runs {
            let lane = shadows.lane();
            shadows.push(lane, kept.iter().chain(&records).map(Step::of));
            if let Ok(()) = shadows.open(lane).await? {
                shadows.taint(lane);
                let plan = plan_between(shadows.db(lane), db, None).await?;
                let statements: Vec<String> = plan.runnable().cloned().collect();
                if let Ok(Ok(())) = run_to(shadows.db(lane), &statements, &[], &base).await {
                    runs = true;
                    for m in &pending {
                        if run(shadows.db(lane), m, false).await.is_err() {
                            runs = false;
                            break;
                        }
                    }
                }
            }
        }
        if !runs {
            // A migration that fails on what the migrations make, too, fails for its own reasons:
            // undoing what no migration made would lose it for nothing.
            let clean = shadows.lane();
            shadows.push(clean, kept.iter().chain(&records).chain(&pending).map(Step::of));
            if let Err((m, e)) = shadows.lines(clean).await? {
                return fail(&format!("{m} fails when replayed in a shadow database"), e, log);
            }
            // Undone, to what the migrations kept and recorded make: the recorded ones' change stays.
            let lane = shadows.lane();
            shadows.push(lane, kept.iter().chain(&records).map(Step::of));
            let expect = match shadows.lines(lane).await? {
                Ok(lines) => lines,
                Err((m, e)) => return fail(&format!("{m} fails when replayed in a shadow database"), e, log),
            };
            if let Err((_, e)) = shadows.open(lane).await? {
                return fail("The history fails when replayed in a shadow database", e, log);
            }
            let target = read_target(shadows.db(lane), None).await?;
            match take_to(db, &target, &[], &expect).await {
                Err(e) => return fail("Undoing changes no migration made failed", e.0, log),
                Ok(Err(lines)) => return refused("Undoing changes no migration made wouldn't reach what the migrations make", lines, log),
                Ok(Ok((_, plan))) => {
                    rows.open(log);
                    log(&format!("  {} Undid what no migration made, so that {} runs on what the migrations make", paint("yellow", "↺"), pending[0].name));
                    dropped_row(&plan, &counts, log);
                }
            }
        }
    }
    if !records.is_empty() || !pending.is_empty() {
        ensure_history(db).await?;
    }
    for m in &records {
        insert_row(db, m, true, None).await?;
        rows.row(log, &paint("green", "+"), &m.name, "recorded: already in place");
    }
    for m in &pending {
        let started = std::time::Instant::now();
        if let Err(e) = run(db, m, true).await {
            failed(&mut rows, m, &e, d, log);
            return Ok(1);
        }
        rows.row(log, &paint("green", "✓"), &m.name, &format!("applied {}", paint("dim", &format!("· {} ms", started.elapsed().as_millis()))));
    }
    if rewind_rows.is_empty() && records.is_empty() && pending.is_empty() && base == at_kept {
        log(&format!("{} Up to date with the migrations", paint("green", "✓")));
    }
    Ok(0)
}

/// `reylo db push`: takes a development database that is at its migrations (`dev` brings it
/// there) to schema.sql: what schema.sql has that no migration makes is made, unrecorded,
/// for `create` to write next. Refuses a database with a migration it hasn't applied, or one whose
/// history disagrees with the files. Never in CI.
pub async fn db_push(opts: &Options, in_ci: bool, log: Log<'_>) -> Result<i32, Error> {
    if in_ci {
        return Ok(refused_in_ci("reylo db push", log));
    }
    let (mut db, mut server, swept) = open(opts).await?;
    let result = db_push_on(&mut db, &mut server, swept, opts, log).await;
    db.close().await;
    server.close().await;
    result
}

/// `reylo db push --watch`: syncs, then again each time schema.sql or a migration changes,
/// until it is stopped. Each run prints its block under the time it ran; one that changes nothing
/// prints a line. A refusal (a migration to apply first) is printed and waited out like any run.
/// `runs` bounds how many runs it makes, for a test; None runs until stopped.
pub async fn db_push_watch(opts: &Options, in_ci: bool, runs: Option<usize>, log: Log<'_>) -> Result<i32, Error> {
    if in_ci {
        return Ok(refused_in_ci("reylo db push --watch", log));
    }
    let snapshot = || -> Vec<(PathBuf, Option<std::time::SystemTime>, u64)> {
        let mut paths = vec![opts.schema.clone()];
        if let Ok(entries) = std::fs::read_dir(&opts.dir) {
            let mut files: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "sql")).collect();
            files.sort();
            paths.extend(files);
        }
        paths.into_iter().map(|p| {
            let meta = std::fs::metadata(&p).ok();
            let (at, len) = (meta.as_ref().and_then(|m| m.modified().ok()), meta.map_or(0, |m| m.len()));
            (p, at, len)
        }).collect()
    };
    log(&paint("dim", &format!("Watching {} and {}: each save pushes again (Ctrl-C stops)", shown(&opts.schema), shown(&opts.dir))));
    let mut made = 0;
    let mut last = Vec::new();
    loop {
        let before = snapshot();
        let mut lines: Vec<String> = Vec::new();
        // A database that went away (restarting) is waited out like a refusal: said, then watched.
        let code = match db_push(opts, false, &mut |l: &str| lines.push(l.to_string())).await {
            Ok(code) => code,
            Err(e) => {
                lines.push(format!("{} {e}", paint("bold red", "✗")));
                1
            }
        };
        let at = time::clock();
        // A run that pushed nothing is one line, which says what it said before, if anything.
        let pushed = lines.iter().any(|l| l.starts_with("Pushing "));
        let pending = lines.iter().any(|l| l.contains("Not in a migration yet"));
        let quiet = code == 0 && !pushed && lines.len() <= 2;
        if quiet && !last.is_empty() {
            log(&paint("dim", &format!("{at} · nothing to push{}", if pending { " (not in a migration yet)" } else { "" })));
        } else {
            log("");
            log(&paint("dim", &format!("{at}")));
            for l in &lines {
                log(l);
            }
        }
        last = lines;
        made += 1;
        if runs.is_some_and(|n| made >= n) {
            return Ok(code);
        }
        // Until a file changes, and a moment longer, so an editor's save of several files is one run.
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            if snapshot() != before {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                break;
            }
        }
    }
}

async fn db_push_on(db: &mut Db, server: &mut Server, swept: usize, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    swept_note(swept, log);
    let _held = lock(db, opts).await?;
    let d = db.dialect();
    let (files, strays) = read_migrations(&opts.dir)?;
    let schema_sql = std::fs::read_to_string(&opts.schema).ok();
    let mut problems = file_problems(opts, &files, &strays, schema_sql.as_deref(), d);
    if schema_sql.as_deref().is_some_and(|sql| crate::sql::is_blank(sql, d)) {
        problems.push(
            Problem::new("schema-sql-empty", format!("{} declares nothing", shown(&opts.schema)))
                .note("Taking this database to it would drop every table, with its rows.")
                .fix("To rebuild this database from its migrations instead", &["reylo db reset"]),
        );
    }
    if schema_sql.is_none() {
        problems.push(Problem::new("schema-sql-missing", format!("No {} to push", shown(&opts.schema))).note("This project keeps its schema in its migrations: reylo migrate dev runs them."));
    }
    // The database is at its migrations, as `dev` leaves it: every migration applied, each
    // with the text on disk, and nothing else.
    let history = read_history(db).await?;
    let applied: Vec<&HistoryRow> = history.iter().filter(|r| r.applied).collect();
    let mut behind = Vec::new();
    for r in &applied {
        match files.iter().find(|f| f.name == r.name) {
            None => behind.push(format!("  {} isn't in the migrations", r.name)),
            Some(f) if !same_statements(&f.sql, &r.sql, d) => behind.push(format!("  {} changed since it ran", r.name)),
            Some(_) => {}
        }
    }
    let unrun: Vec<&Migration> = files.iter().filter(|f| !applied.iter().any(|r| r.name == f.name)).collect();
    for f in &unrun {
        behind.push(format!("  {} hasn't run here", f.name));
    }
    // The lock lists what the database applied, in its order.
    if behind.is_empty() {
        match lock::read(&opts.lock) {
            lock::State::Lock(l) if l.names() == applied.iter().map(|r| r.name.clone()).collect::<Vec<_>>() => {}
            lock::State::Missing => {}
            _ => behind.push(format!("  {} doesn't list what it applied, in its order", shown(&opts.lock))),
        }
    }
    if !behind.is_empty() {
        let title = match unrun.as_slice() {
            [one] if behind.len() == 1 => format!("{} hasn't run on this database", one.name),
            many if behind.len() == many.len() => format!("{} migrations haven't run on this database", many.len()),
            _ => "This database doesn't match its migrations".to_string(),
        };
        let listed = if behind.len() == 1 && unrun.len() == 1 { vec![] } else { behind };
        problems.push(Problem::new("database-not-at-migrations", title).lines(listed).fix(if unrun.len() == 1 { "Run it first" } else { "Take it to its migrations first" }, &["reylo migrate dev"]));
    }
    if !problems.is_empty() {
        return Ok(refuse(&problems, log));
    }
    let schema_sql = schema_sql.unwrap_or_default();
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain([schema_sql.as_str()]).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &texts).await;
    let in_order: Vec<Migration> = applied.iter().filter_map(|r| files.iter().find(|f| f.name == r.name).cloned()).collect();
    let result = db_push_in_shadows(&mut shadows, db, opts, &in_order, &schema_sql, log).await;
    shadows.finish().await;
    result
}

/// What schema.sql has that no migration makes, made on the database, unrecorded; `applied` is the
/// migrations in the order the database ran them.
async fn db_push_in_shadows(shadows: &mut Shadows<'_>, db: &mut Db, opts: &Options, applied: &[Migration], schema_sql: &str, log: Log<'_>) -> Result<i32, Error> {
    let d = db.dialect();
    let fail = |m: &str, error: String, log: Log<'_>| {
        let problem = Problem::new("db-push-failed", m.to_string()).note(error);
        if d == Dialect::Mysql {
            print_problems(&[problem.note("MySQL commits each statement, so part of the push may be in place.")], log);
            return Ok(1);
        }
        Ok(refuse(&[problem], log))
    };
    let s_lane = shadows.lane();
    shadows.push(s_lane, [Step::script("schema.sql", schema_sql)]);
    match shadows.open(s_lane).await? {
        Err((_, e)) => return fail(&format!("{} doesn't run on an empty database", shown(&opts.schema)), e, log),
        Ok(()) => shadows.taint(s_lane),
    }
    let wanted = catalog(shadows.db(s_lane)).await?;
    let now = crate::catalog::without_extra_children(&catalog(db).await?, &wanted);
    if now != wanted {
        let target = read_target(shadows.db(s_lane), Some(schema_sql)).await?;
        let counts = table_rows(db).await?;
        let (renames, plan) = match take_to(db, &target, &[], &wanted).await {
            Err(e) => return fail(if d == Dialect::Mysql { "reylo db push failed partway" } else { "reylo db push failed" }, e.0, log),
            Ok(Err(lines)) => {
                let problem = Problem::new("db-push-incomplete", format!("Reylo can't take this database the rest of the way to {}", shown(&opts.schema))).lines(lines).fix("Write the change as a migration, finishing by hand what it leaves out", &["reylo migrate new <name>"]);
                if d != Dialect::Mysql {
                    return Ok(refuse(&[problem], log));
                }
                print_problems(&[problem.note("MySQL commits each statement, so part of the push may be in place.")], log);
                return Ok(1);
            }
            Ok(Ok(done)) => done,
        };
        log(&format!("Pushing {} to {}", shown(&opts.schema), shown_url(&opts.url)));
        for statement in renames.iter().chain(&plan.statements) {
            statement_row(statement, log);
        }
        dropped_row(&plan, &counts, log);
    }
    // Whether the migrations themselves make schema.sql, or this database holds more than they do.
    let m_lane = shadows.lane();
    shadows.push(m_lane, applied.iter().map(Step::of));
    let made = match shadows.lines(m_lane).await? {
        Ok(lines) => lines == wanted,
        Err(_) => false,
    };
    // The lock certifies that its migrations reach this schema.sql.
    let mut wrote = false;
    if made {
        if let lock::State::Lock(mut l) = lock::read(&opts.lock) {
            let sum = lock::schema_sum(&l.entries.last().map(|e| e.sum.clone()).unwrap_or_default(), schema_sql);
            if l.schema.as_deref() != Some(sum.as_str()) {
                l.schema = Some(sum);
                lock::write(&opts.lock, &l)?;
                wrote = true;
            }
        }
    }
    let schema = shown(&opts.schema);
    if made {
        log(&format!("{} This database matches {schema} and its migrations", paint("green", "✓")));
    } else {
        log(&format!("{} This database matches {schema}", paint("green", "✓")));
        log(&format!("{} Not in a migration yet: reylo migrate new <name> writes it", paint("yellow", "!")));
    }
    if wrote {
        log(&format!("{} Wrote {}", paint("green", "✓"), shown(&opts.lock)));
    }
    Ok(0)
}

/// One statement `db push` ran: `+` for what it adds, `-` for what it takes away; a comment is dim.
fn statement_row(statement: &str, log: Log<'_>) {
    if let Some(said) = statement.strip_prefix("--") {
        log(&format!("    {} {}", paint("yellow", "!"), said.trim()));
        return;
    }
    let text = statement.trim_end();
    let lower = text.trim_start().to_lowercase();
    let mark = if lower.starts_with("drop ") || lower.contains(" drop ") { paint("yellow", "-") } else { paint("green", "+") };
    for (k, line) in format!("{text};").lines().enumerate() {
        if k == 0 {
            log(&format!("  {mark} {line}"));
        } else {
            log(&format!("    {line}"));
        }
    }
}

/// The rows of each of the database's tables, by table id.
async fn table_rows(db: &mut Db) -> Result<std::collections::BTreeMap<String, u64>, Error> {
    let schema = crate::diff::read(db).await?;
    // By the id a plan's losses name it by: on Postgres, without the first schema of the search_path.
    let first = if db.dialect() == Dialect::Postgres { format!(" {}.", crate::diff::postgres::first_schema(db).await?) } else { String::new() };
    let mut out = std::collections::BTreeMap::new();
    for o in schema.objects.values().filter(|o| o.kind == Some(crate::diff::Kind::Table) && !o.extra.iter().any(|l| l.starts_with("shape foreign"))) {
        let Some(name) = o.drop.first().and_then(|d| d.strip_prefix("drop table ")) else { continue };
        if let Ok(rows) = db.rows(&format!("select count(*) as n from {name}")).await {
            if let Some(n) = rows.first().and_then(|r| r.get("n")).and_then(|n| n.parse().ok()) {
                out.insert(if first.is_empty() { o.id.clone() } else { o.id.replacen(&first, " ", 1) }, n);
            }
        }
    }
    Ok(out)
}

/// The line saying what a plan drops, under the row it belongs to, if it drops anything.
fn dropped_row(plan: &crate::diff::Plan, counts: &std::collections::BTreeMap<String, u64>, log: Log<'_>) {
    if let Some(dropped) = dropped_items(plan, counts) {
        log(&format!("    {} {dropped}", paint("yellow", "!")));
    }
}

/// What a plan drops, as one phrase: each table with how many rows it held where `counts` holds any,
/// and each column ("Dropped tags (3 rows) and users.bio"); many empty tables, counted.
fn dropped_items(plan: &crate::diff::Plan, counts: &std::collections::BTreeMap<String, u64>) -> Option<String> {
    if plan.losses.is_empty() {
        return None;
    }
    let tables = plan.losses.iter().filter(|l| l.starts_with("table ")).count();
    if tables > 2 && tables == plan.losses.len() && plan.losses.iter().all(|l| counts.get(l).copied().unwrap_or(0) == 0) {
        return Some(format!("Dropped {tables} tables, none with rows"));
    }
    let what: Vec<String> = plan
        .losses
        .iter()
        .map(|l| {
            let name = l.trim_start_matches("column ").trim_start_matches("table ");
            match counts.get(l).copied() {
                Some(1) => format!("{name} (1 row)"),
                Some(n) if n > 1 => format!("{name} ({n} rows)"),
                _ => name.to_string(),
            }
        })
        .collect();
    Some(format!("Dropped {}", and_list(&what)))
}

/// What a migration drops, each kind together: `the column users.bio`, `the columns users.a and
/// users.b`, `the table tags, and the column users.bio`.
fn drop_phrase(items: &[(String, String)]) -> String {
    let mut kinds: Vec<&str> = Vec::new();
    for (kind, _) in items {
        if !kinds.contains(&kind.as_str()) {
            kinds.push(kind);
        }
    }
    kinds
        .iter()
        .map(|kind| {
            let names: Vec<String> = items.iter().filter(|(k, _)| k == kind).map(|(_, n)| n.clone()).collect();
            if names.len() == 1 { format!("the {kind} {}", names[0]) } else { format!("the {kind}s {}", and_list(&names)) }
        })
        .collect::<Vec<_>>()
        .join(", and ")
}

/// `a`, `a and b`, `a, b and c`.
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// `reylo run "<sql>"` and `reylo run --file <path>`: runs SQL on the database as written, inline
/// text or files, in the order given; records nothing. For a change typed by hand during
/// development, and a seed after `reylo migrate dev`.
pub async fn run_sql(url: &str, root: &Path, items: &[(String, String)], log: Log<'_>) -> Result<i32, Error> {
    let mut db = Db::connect(url, root).await?;
    let mut code = 0;
    for (label, sql) in items {
        match db.script(sql).await {
            Ok(()) => log(&format!("{} {label}", paint("green", "✓"))),
            Err(e) => {
                log(&paint("bold red", &format!("✗ {label}")));
                log(&format!("  {e}"));
                code = 1;
                break;
            }
        }
    }
    db.close().await;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_gives_json_its_code_and_each_part() {
        set_color(false);
        let p = Problem::new("database-changed", "This database changed since Reylo last ran on it")
            .lines(["  + now: column users.hotfix bigint".to_string()])
            .sql("alter table users add column hotfix bigint;")
            .note("A note.")
            .fix("To keep it", &["reylo migrate new <name>", "reylo migrate history push"]);
        let j = p.json();
        assert_eq!(j["code"], "database-changed");
        assert_eq!(j["title"], "This database changed since Reylo last ran on it");
        assert_eq!(j["detail"], serde_json::json!(["+ now: column users.hotfix bigint"]));
        assert_eq!(j["sql"], "alter table users add column hotfix bigint;");
        assert_eq!(j["notes"], serde_json::json!(["A note."]));
        assert_eq!(j["fixes"], serde_json::json!([{ "what": "To keep it", "commands": ["reylo migrate new <name>", "reylo migrate history push"] }]));
    }

    #[test]
    fn init_sql_comes_first_in_name_order() {
        let mut names = vec!["20261109120000_b.sql", "init.sql", "20261108120000_a.sql"];
        names.sort_by(|a, b| name_order(a).cmp(&name_order(b)));
        assert_eq!(names, ["init.sql", "20261108120000_a.sql", "20261109120000_b.sql"]);
    }
}
