//! The commands of `reylo migrate`, and `reylo run`. Each takes the project's settings and a log it
//! writes its lines to, and answers the process's exit code; an `Err` is a refusal to start (a
//! setting, a file or a connection), which the command line prints.

use crate::catalog::{catalog, difference, Scope, HISTORY_TABLE};
use crate::db::{wipe, Db, Server};
use crate::lock::{self, LOCK};
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
    /// A linear history (`history: linear`): a pending migration older than one already applied is
    /// refused, and `migrations/lock.yaml` lists every migration in order. Off by default: a
    /// development database that pulls migrations merged on main applies them on top of its own, in
    /// an order production never ran, and the shadows of both orders must agree.
    pub linear: bool,
    /// Whether comments on objects are compared (`strict_comments`, on by default).
    pub comments: bool,
    /// For a bare `rebase`: the main branch; git's origin/HEAD when absent.
    pub main_branch: Option<String>,
    /// A shell command run after `apply --reset` replays every migration (a seed).
    pub post: Option<String>,
    /// The project's directory: a relative SQLite path is its, and `post` runs in it.
    pub root: PathBuf,
}

impl Options {
    fn scope(&self) -> Scope {
        Scope { comments: self.comments }
    }
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
pub fn set_color(on: bool) {
    COLOR.store(on, Ordering::Relaxed);
}

fn paint(style: &str, text: &str) -> String {
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
        } else if !name.starts_with('.') && name != LOCK {
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
        Dialect::Postgres => format!("create table if not exists {HISTORY_TABLE} (name text primary key, sql text not null, applied_at timestamptz, fingerprint text)"),
        Dialect::Mysql => format!("create table if not exists {HISTORY_TABLE} (name varchar(255) primary key, `sql` longtext not null, applied_at datetime(6) null, fingerprint text null) engine=InnoDB"),
        Dialect::Sqlite => format!("create table if not exists {HISTORY_TABLE} (name text primary key, sql text not null, applied_at text, fingerprint text)"),
    }
}

fn now_sql(d: Dialect) -> &'static str {
    match d {
        Dialect::Postgres => "clock_timestamp()",
        Dialect::Mysql => "now(6)",
        Dialect::Sqlite => "strftime('%Y-%m-%d %H:%M:%f', 'now')",
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
    // Rows applied in one run share a timestamp at the clock's resolution (SQLite's is a
    // millisecond), and a run applies in time order, so a tie is broken by it: init.sql first.
    let applied_at = if d == Dialect::Postgres { "applied_at::text" } else { "applied_at" };
    // A table an earlier build made has no fingerprint column until the next apply adds it.
    let fingerprint = if has_fingerprint(db).await? { "fingerprint" } else { "null as fingerprint" };
    let rows = db
        .rows(&format!(
            "select name, {} as body, {applied_at} as applied_at, {fingerprint} from {HISTORY_TABLE} order by applied_at is null, applied_at, case when name = '{INIT}' then '' else name end",
            sql_col(d)
        ))
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| HistoryRow { name: r.text("name").into(), sql: r.text("body").into(), applied: r.get("applied_at").is_some(), fingerprint: r.get("fingerprint").map(str::to_string) })
        .collect())
}

async fn has_fingerprint(db: &mut Db) -> Result<bool, Error> {
    let sql = match db.dialect() {
        Dialect::Postgres => format!("select count(*) > 0 as ok from pg_attribute where attrelid = to_regclass('{HISTORY_TABLE}') and attname = 'fingerprint' and not attisdropped"),
        Dialect::Mysql => format!("select count(*) > 0 as ok from information_schema.columns where table_schema = database() and table_name = '{HISTORY_TABLE}' and column_name = 'fingerprint'"),
        Dialect::Sqlite => format!("select count(*) > 0 as ok from pragma_table_info('{HISTORY_TABLE}') where name = 'fingerprint'"),
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
    Ok(())
}

/// The live schema's fingerprint: a digest of its catalog as the comparator reads it, with the
/// server's version and this build's catalog code, which both change what the catalog reads as. A
/// database whose fingerprint equals the one its last run stored has not been changed since.
async fn fingerprint(db: &mut Db, scope: Scope) -> Result<String, Error> {
    let lines = catalog(db, scope).await?;
    fingerprint_of(db, &lines).await
}

/// The fingerprint of catalog lines already read from `db`.
async fn fingerprint_of(db: &mut Db, lines: &[String]) -> Result<String, Error> {
    let version = match db.dialect() {
        Dialect::Postgres => "select current_setting('server_version_num') as v",
        Dialect::Mysql => "select version() as v",
        Dialect::Sqlite => "select sqlite_version() as v",
    };
    let version = db.rows(version).await?.first().map(|r| r.text("v").to_string()).unwrap_or_default();
    let mut digest = crate::cache::Sha256::new();
    for line in lines {
        digest.part(line.as_bytes());
    }
    Ok(format!("{}:{version}:{}", crate::cache::code_id(), crate::cache::hex(&digest.finish())))
}

async fn set_fingerprint(db: &mut Db, name: &str, value: &str) -> Result<(), Error> {
    db.exec_with(&format!("update {HISTORY_TABLE} set fingerprint = {} where name = {}", db.param(1), db.param(2)), &[&value, &name]).await
}

/// Waits until no other run of `reylo migrate` works on this database, so that two that start at
/// once (two instances of an app, each migrating as it starts) neither decide alike nor apply alike:
/// a session lock on Postgres and MySQL, held until the connection closes. SQLite's own locks would
/// stop the app's queries for the length of the check, so a SQLite database is locked through a
/// file of the project's (in `.reylo/`, which only these runs open), held while the answer lives.
async fn lock(db: &mut Db, opts: &Options) -> Result<Option<std::fs::File>, Error> {
    match db.dialect() {
        Dialect::Postgres => db.exec("select pg_advisory_lock(hashtextextended('reylo_migrate', 0))").await.map(|_| None),
        Dialect::Mysql => db.exec("select get_lock('reylo_migrate', 600)").await.map(|_| None),
        Dialect::Sqlite => {
            let path = crate::db::sqlite_path(&opts.url, &opts.root);
            if path == ":memory:" {
                return Ok(None);
            }
            let canonical = std::fs::canonicalize(&path).map(|p| p.to_string_lossy().into_owned()).unwrap_or(path);
            let mut digest = crate::cache::Sha256::new();
            digest.part(canonical.as_bytes());
            let dir = opts.root.join(".reylo").join("migrate").join("locks");
            std::fs::create_dir_all(&dir)?;
            let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(crate::cache::hex(&digest.finish()[..8])))?;
            file.lock()?;
            Ok(Some(file))
        }
    }
}

async fn insert_row(db: &mut Db, m: &Migration, applied: bool) -> Result<(), Error> {
    let d = db.dialect();
    let sql = format!("insert into {HISTORY_TABLE} (name, {}, applied_at) values ({}, {}, {})", sql_col(d), db.param(1), db.param(2), if applied { now_sql(d) } else { "null" });
    db.exec_with(&sql, &[&m.name, &m.sql]).await
}

// ---------------------------------------------------------------------------------------------
// Running a migration: in one transaction with its history row, or, for a file that cannot run in
// one, alone, with a row marked started before it and finished after it.

pub(crate) async fn run(db: &mut Db, m: &Migration, record: bool) -> Result<(), Error> {
    let d = db.dialect();
    if runs_alone(&m.sql, d) {
        if record {
            insert_row(db, m, false).await?;
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
            db.exec_with(&format!("update {HISTORY_TABLE} set applied_at = {} where name = {}", now_sql(d), db.param(1)), &[&m.name]).await?;
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
            insert_row(db, m, true).await?;
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
    result
}

// ---------------------------------------------------------------------------------------------
// The check.

fn describe_diff(label_a: &str, a: &[String], label_b: &str, b: &[String]) -> Vec<String> {
    const MAX: usize = 20;
    let (only_a, only_b) = difference(a, b);
    let mut out: Vec<String> = only_a.iter().take(MAX).map(|l| paint("red", &format!("  - {label_a}: {l}"))).collect();
    out.extend(only_b.iter().take(MAX).map(|l| paint("green", &format!("  + {label_b}: {l}"))));
    let more = only_a.len() + only_b.len() - out.len();
    if more > 0 {
        out.push(format!("  … and {more} more"));
    }
    out
}

/// One thing a check refuses: its heading, and the lines under it, the SQL first and what to run last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub title: String,
    pub lines: Vec<String>,
    fixed: bool,
}

impl Problem {
    fn new(title: impl Into<String>) -> Problem {
        Problem { title: title.into(), lines: vec![], fixed: false }
    }

    /// SQL under the heading, set off by blank lines: what differs, never a fix to run.
    fn sql(mut self, sql: &str) -> Problem {
        self.lines.push(String::new());
        self.lines.extend(sql.trim_end().lines().map(|l| format!("    {l}")));
        self.lines.push(String::new());
        self
    }

    fn note(mut self, text: impl AsRef<str>) -> Problem {
        self.lines.push(format!("  {}", text.as_ref()));
        self
    }

    /// What to do, then each command that does it on a line of its own.
    fn fix(mut self, what: impl AsRef<str>, commands: &[&str]) -> Problem {
        if !self.fixed && self.lines.last().is_none_or(|l| !l.is_empty()) {
            self.lines.push(String::new());
        }
        self.fixed = true;
        self.lines.push(format!("  {}:", what.as_ref()));
        self.lines.extend(commands.iter().map(|c| format!("    {}", paint("cyan", c))));
        self
    }

    fn lines(mut self, lines: impl IntoIterator<Item = String>) -> Problem {
        self.lines.extend(lines);
        self
    }
}

/// What a check holds the database to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `apply`: the migrations against the database. schema.sql is not read: what it declares is
    /// held against the migrations in development and CI (`create`, `check`), not at deploy.
    Apply,
    /// `check`: everything `Apply` checks, and the migrations against schema.sql; runs nothing.
    Check,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub problems: Vec<Problem>,
    /// Pending migrations recorded without running (`--record`): the database already has them.
    pub records: Vec<Migration>,
    /// Migrations to run, in order.
    pub pending: Vec<Migration>,
    /// Whether the live schema is the one Reylo's last run on this database left, by the
    /// fingerprint that run stored; None where no run stored one, or another server version or
    /// catalog reader read it.
    pub unchanged: Option<bool>,
    /// The records that are lone statements cut off before their row was finished, which landed.
    pub landed: Vec<String>,
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

/// What is wrong with the lock of a linear history: it must list the migration files, each once, in
/// the order of their names, which is the order they run in.
fn lock_problems(opts: &Options, files: &[Migration]) -> Vec<Problem> {
    let path = shown(&opts.dir.join(LOCK));
    let names = match lock::read(&opts.dir) {
        lock::State::Missing => return vec![Problem::new(format!("The history is linear, and {path} is missing")).fix("Write it from the migration files", &["reylo migrate rebase"])],
        lock::State::Conflicted => return vec![Problem::new(format!("{path} has a merge conflict: another branch added migrations too")).fix("Move this branch's migrations after the others', and resolve it", &["reylo migrate rebase"])],
        lock::State::Invalid(e) => return vec![Problem::new(format!("{path} doesn't read as a lock file")).note(e).fix("Write it again from the migration files", &["reylo migrate rebase"])],
        lock::State::Names(names) => names,
    };
    let order: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    if names.iter().map(String::as_str).eq(order.iter().copied()) {
        return vec![];
    }
    let mut lines = Vec::new();
    for f in order.iter().filter(|f| !names.iter().any(|n| n == *f)) {
        lines.push(format!("  {f} isn't in it"));
    }
    for n in names.iter().filter(|n| !order.contains(&n.as_str())) {
        lines.push(format!("  It lists {n}, which isn't a migration file"));
    }
    if lines.is_empty() {
        lines.push(format!("  It lists {}", names.join(", ")));
    }
    vec![Problem::new(format!("{path} doesn't list the migration files in the order they run")).lines(lines).fix("Put them in its order", &["reylo migrate rebase"])]
}

/// Checks the target against the migration files, and in `Mode::Check` the migrations against
/// schema.sql too; changes nothing. Nothing is ever recorded without running unless `record` names
/// it, and only a migration without data statements, which shadows show the database already has.
pub async fn check(db: &mut Db, server: &mut Server, opts: &Options, mode: Mode, record: &[String], log: Log<'_>) -> Result<Plan, Error> {
    let d = db.dialect();
    let scope = opts.scope();
    let (files, strays) = read_migrations(&opts.dir)?;
    let mut problems = Vec::new();
    for s in &strays {
        problems.push(Problem::new(format!("{s} isn't a migration")).note(format!("A file in {} is init.sql or <YYYYMMDDHHMMSS>_<name>.sql, the name in a-z, 0-9 and _.", shown(&opts.dir))));
    }
    let schema_sql = match mode {
        Mode::Apply => None,
        Mode::Check => {
            let text = std::fs::read_to_string(&opts.schema).ok();
            if text.is_none() {
                problems.push(Problem::new(format!("{} doesn't exist", shown(&opts.schema))).note("The declared schema is required: migrations are written from it, and checked against it."));
            }
            text
        }
    };
    if opts.linear {
        problems.extend(lock_problems(opts, &files));
    } else if opts.dir.join(LOCK).exists() {
        log(&paint("yellow", &format!("! {} is read only when the history is linear: delete it, or set migrate.history: linear", shown(&opts.dir.join(LOCK)))));
    }
    let history = read_history(db).await?;
    // A row marked started (a lone statement cut off before its row was finished) counts as pending:
    // whether it landed is read from the live database below, as for any pending migration.
    let applied: Vec<&HistoryRow> = history.iter().filter(|r| r.applied).collect();
    let by_name = |name: &str| files.iter().find(|f| f.name == name);
    let dir = shown(&opts.dir);
    for r in &applied {
        match by_name(&r.name) {
            None => problems.push(Problem::new(format!("This database applied {}, which isn't in {dir}", r.name)).fix("Restore it from this database's history", &[&format!("reylo migrate restore {}", r.name)])),
            // An edit to comments or spacing alone changes nothing that ran.
            Some(f) if !same_statements(&f.sql, &r.sql, d) => problems.push(
                Problem::new(format!("{} changed after this database applied it", r.name))
                    .lines(std::iter::once(String::new()).chain(line_diff(&r.sql, &f.sql)))
                    .fix("Restore the text this database ran", &[&format!("reylo migrate restore {}", r.name)]),
            ),
            Some(_) => {}
        }
    }
    let applied_files: Vec<Migration> = applied.iter().filter_map(|r| by_name(&r.name).cloned()).collect();
    let mut records: Vec<Migration> = Vec::new();
    for named in record {
        let base = Path::new(named).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        match by_name(&base) {
            None => problems.push(Problem::new(format!("{named} isn't a migration in {dir}"))),
            Some(f) if applied_files.iter().any(|a| a.name == f.name) => problems.push(Problem::new(format!("{} is already applied to this database", f.name))),
            Some(f) => {
                let data = data_statements(&f.sql, d);
                if data.is_empty() {
                    if !records.contains(f) {
                        records.push(f.clone());
                    }
                } else {
                    problems.push(
                        Problem::new(format!("{} changes data, so it can't be recorded without running", f.name))
                            .note(format!("Its data statements ({}) would never run here. Put the schema change and the data change in migrations of their own: the first can be recorded, and the second runs.", data.join(", "))),
                    );
                }
            }
        }
    }
    records.sort_by(|a, b| a.time.cmp(&b.time).then(a.name.cmp(&b.name)));
    let pending: Vec<Migration> = files.iter().filter(|f| !applied_files.iter().any(|a| a.name == f.name) && !records.contains(f)).cloned().collect();
    let done: Vec<&Migration> = applied_files.iter().chain(&records).collect();
    let newest = done.iter().filter(|f| f.name != INIT).map(|f| f.time.clone()).max().unwrap_or_default();
    for p in pending.iter().filter(|p| p.name != INIT && p.time < newest) {
        let passed: Vec<&&Migration> = done.iter().filter(|f| f.name != INIT && f.time > p.time).collect();
        let first = passed.iter().min_by(|a, b| a.time.cmp(&b.time)).map(|f| f.name.clone()).unwrap_or_default();
        if opts.linear {
            problems.push(Problem::new(format!("{} is older than {first}, which this database already applied", p.name)).note(format!("The history is linear: give {} a time after {first}'s, by renaming the file.", p.name)));
            continue;
        }
        // Loose: a late migration runs only where both orders end the same, which the shadows
        // check below; and only where nothing it passes changes data, which no shadow can see.
        let changing: Vec<&str> = std::iter::once(&p).map(|m| m.name.as_str()).filter(|_| !data_statements(&p.sql, d).is_empty()).chain(passed.iter().filter(|m| !data_statements(&m.sql, d).is_empty()).map(|m| m.name.as_str())).collect();
        if !changing.is_empty() {
            problems.push(
                Problem::new(format!("{} comes before {first} by name, but this database ran {first} first", p.name))
                    .note(format!("{} changes data, so the two can't be run in another order safely: give {} a time after {first}'s, by renaming the file.", changing.join(" and "), p.name)),
            );
        }
    }
    // init.sql, which `init` writes, holds the whole schema and runs only on an empty database.
    for p in pending.iter().filter(|f| f.name != INIT) {
        if let Some(why) = atomicity_problem(&p.sql, d) {
            problems.push(Problem::new(format!("{} can't run atomically", p.name)).note(why));
        }
    }
    let schema_name = opts.schema.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "schema.sql".into());
    for (name, sql) in files.iter().map(|f| (f.name.as_str(), f.sql.as_str())).chain(schema_sql.as_deref().map(|s| (schema_name.as_str(), s))) {
        if let Some(why) = shadow_problem(sql, d) {
            problems.push(Problem::new(format!("{name} can't be replayed in a shadow database")).note(why));
        }
    }
    if !problems.is_empty() {
        return Ok(Plan { problems, ..Plan::default() });
    }

    let live = catalog(db, scope).await?;
    let stored = applied.last().and_then(|r| r.fingerprint.clone());
    let unchanged = match stored {
        Some(stored) => {
            let now = fingerprint_of(db, &live).await?;
            // Only a fingerprint read the same way says anything: the reader and the server's version.
            let how = |f: &str| f.rsplit_once(':').map(|(how, _)| how.to_string());
            (how(&stored) == how(&now)).then_some(stored == now)
        }
        None => None,
    };
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain(history.iter().map(|r| r.sql.as_str())).chain(schema_sql.as_deref()).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, scope, &texts).await;
    // A lone statement cut off before its row was finished did run, or did not: the database says
    // which, and one that landed is recorded, as the row it started says it ran.
    let started: Vec<String> = history.iter().filter(|r| !r.applied).map(|r| r.name.clone()).collect();
    let mut run = Run { live: &live, schema_sql: schema_sql.as_deref(), files: &files, applied_files: &applied_files, records, pending, started, landed: vec![], d };
    let result = check_in_shadows(&mut shadows, db, &mut run).await;
    shadows.finish().await;
    let problems = result?;
    Ok(Plan { problems, records: run.records, pending: run.pending, unchanged, landed: run.landed })
}

/// What a check holds the shadows against.
struct Run<'a> {
    live: &'a [String],
    schema_sql: Option<&'a str>,
    files: &'a [Migration],
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
            if lines == run.live {
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
        Err((m, error)) => return Ok(vec![Problem::new(format!("{m} fails when replayed in a shadow database")).note(error)]),
    };
    if checkpoint != run.live {
        return drift(shadows, target, run, &checkpoint).await;
    }
    shadows.push(b_lane, run.pending.iter().map(Step::of));
    let b = match shadows.lines(b_lane).await? {
        Ok(lines) => lines,
        Err((m, error)) => return Ok(vec![Problem::new(format!("{m} fails on this database's history")).note(error)]),
    };

    // C: every migration in name order, unless that is B's own sequence.
    let b_order: Vec<&str> = run.applied_files.iter().chain(&run.records).chain(&run.pending).map(|f| f.name.as_str()).collect();
    let c_order: Vec<&str> = run.files.iter().map(|f| f.name.as_str()).collect();
    let mut problems = Vec::new();
    if b_order != c_order {
        let c_lane = shadows.lane();
        shadows.push(c_lane, run.files.iter().map(Step::of));
        match shadows.lines(c_lane).await? {
            Err((m, error)) => problems.push(Problem::new(format!("{m} fails when every migration runs in name order")).note(error)),
            Ok(c) if c != b => {
                let sql = lane_plan(shadows, b_lane, c_lane, None).await.unwrap_or_default();
                problems.push(Problem::new("In this database's order and in name order, the migrations end differently").sql(&sql).note("Give this branch's migrations times after the others', by renaming the files."));
                return Ok(problems);
            }
            Ok(_) => {}
        }
    }
    let Some(schema_sql) = run.schema_sql else { return Ok(problems) };
    let a_lane = shadows.lane();
    shadows.push(a_lane, [Step::script("schema.sql", schema_sql)]);
    let a = match shadows.lines(a_lane).await? {
        Ok(lines) => lines,
        Err((_, error)) => {
            problems.push(Problem::new("schema.sql fails on an empty database").note(error));
            return Ok(problems);
        }
    };
    if a != b {
        let (only_schema, only_migrations) = difference(&a, &b);
        let problem = match (only_schema.is_empty(), only_migrations.is_empty()) {
            (false, true) => Problem::new("schema.sql has changes not reflected in any migration file").sql(&lane_plan(shadows, b_lane, a_lane, Some(schema_sql)).await.unwrap_or_default()).fix("Write them as a migration", &["reylo migrate create <name>"]),
            (true, false) => Problem::new("The migrations make changes schema.sql doesn't declare").sql(&lane_plan(shadows, a_lane, b_lane, None).await.unwrap_or_default()).note("Add them to schema.sql, or take them out of the migration."),
            _ => Problem::new("schema.sql and the migrations disagree")
                .sql(&lane_plan(shadows, b_lane, a_lane, Some(schema_sql)).await.unwrap_or_default())
                .note("Change schema.sql or the migrations so that they agree; what schema.sql has and no migration makes is written as a migration by:")
                .fix("Write the migration", &["reylo migrate create <name>"]),
        };
        problems.push(problem);
    }
    Ok(problems)
}

/// The refusal for a database that differs from its history: what changed, as SQL from what the
/// history makes to what the database has, and the ways out. Where the next pending migrations,
/// none of which changes data, make exactly the change, recording them is named.
async fn drift(shadows: &mut Shadows<'_>, target: &mut Db, run: &Run<'_>, checkpoint: &[String]) -> Result<Vec<Problem>, Error> {
    let (only_history, only_live) = difference(checkpoint, run.live);
    if !run.records.is_empty() {
        let names = run.records.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ");
        let without = shadows.lane();
        shadows.push(without, run.applied_files.iter().map(Step::of));
        if let Ok(Ok(lines)) = shadows.lines(without).await {
            if lines == run.live {
                return Ok(vec![Problem::new(format!("This database doesn't have what {names} make, so they can't be recorded without running")).fix("Run them instead", &["reylo migrate apply"])]);
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
            if lines == run.live {
                hint = run.pending[..j].iter().map(|m| m.name.clone()).collect();
                break;
            }
        }
    }
    let title = match (only_history.is_empty(), only_live.is_empty()) {
        (true, false) => "This database has changes no migration made",
        (false, true) => "This database is missing something its migrations made",
        _ => "This database differs from what its migrations made",
    };
    let prefix = shadows.lane();
    shadows.push(prefix, run.applied_files.iter().chain(&run.records).map(Step::of));
    let mut sql = String::new();
    if let Ok(Ok(())) = shadows.open(prefix).await {
        shadows.taint(prefix);
        sql = plan_text(shadows.db(prefix), target, None).await.unwrap_or_default();
    }
    let mut problem = Problem::new(title);
    if !sql.is_empty() {
        problem = problem.sql(&sql);
    }
    if !hint.is_empty() {
        let commands: Vec<String> = vec![format!("reylo migrate apply {}", hint.iter().map(|h| format!("--record {h}")).collect::<Vec<_>>().join(" "))];
        let refs: Vec<&str> = commands.iter().map(String::as_str).collect();
        problem = problem.fix(format!("{} {} exactly that change. Record {} without running {}", hint.join(" and "), if hint.len() == 1 { "makes" } else { "make" }, if hint.len() == 1 { "it" } else { "them" }, if hint.len() == 1 { "it" } else { "them" }), &refs);
    } else if !only_live.is_empty() {
        problem = problem.fix("Keep it: add it to schema.sql, write the migration, then record it without running it", &["reylo migrate create <name>", "reylo migrate apply --record <name>"]);
    }
    problem = problem.fix("On a development database, start over from the migrations (loses its data)", &["reylo migrate apply --reset"]);
    Ok(vec![problem])
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

async fn open(opts: &Options) -> Result<(Db, Server, usize), Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let mut server = Server::open(&mut db, &opts.url, opts.shadow_url.as_deref()).await?;
    server.copy_default_privileges(!sets_default_privileges(opts));
    let swept = server.sweep().await?;
    Ok((db, server, swept))
}

fn print_problems(problems: &[Problem], log: Log<'_>) {
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

/// `reylo migrate apply`: checks the database against the migrations, then applies what is pending,
/// recording without running the migrations `record` names.
pub async fn migrate(opts: &Options, record: &[String], log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    let result = migrate_on(&mut db, &mut server, swept, opts, record, log).await;
    db.close().await;
    server.close().await;
    result
}

fn swept_note(swept: usize, log: Log<'_>) {
    if swept > 0 {
        log(&paint("dim", &format!("dropped {swept} shadow database{} a run left behind", if swept == 1 { "" } else { "s" })));
    }
}

async fn migrate_on(db: &mut Db, server: &mut Server, swept: usize, opts: &Options, record: &[String], log: Log<'_>) -> Result<i32, Error> {
    swept_note(swept, log);
    let _held = lock(db, opts).await?;
    let plan = check(db, server, opts, Mode::Apply, record, log).await?;
    if !plan.ok() {
        print_problems(&plan.problems, log);
        log("");
        log(&paint("dim", "Nothing was applied."));
        return Ok(1);
    }
    if plan.pending.is_empty() && plan.records.is_empty() {
        log(&format!("{} Up to date", paint("green", "✓")));
        return Ok(0);
    }
    ensure_history(db).await?;
    // Rows marked started belong to pending migrations: they are written again below.
    db.exec(&format!("delete from {HISTORY_TABLE} where applied_at is null")).await?;
    let mut last = None;
    for m in &plan.records {
        insert_row(db, m, true).await?;
        if plan.landed.contains(&m.name) {
            log(&format!("{} {} was cut off before it was recorded, and its change is in place: recorded", paint("yellow", "!"), m.name));
        } else {
            log(&format!("{} Recorded {} without running it: this database already has its change", paint("yellow", "!"), m.name));
        }
        last = Some(m.name.clone());
    }
    let mut code = 0;
    for m in &plan.pending {
        let started = std::time::Instant::now();
        match run(db, m, true).await {
            Ok(()) => {
                log(&format!("{} Applied {} {}", paint("green", "✓"), m.name, paint("dim", &format!("· {} ms", started.elapsed().as_millis()))));
                last = Some(m.name.clone());
            }
            Err(e) => {
                log(&format!("{} {} failed, and nothing of it was applied:", paint("bold red", "✗"), m.name));
                log(&format!("    {e}"));
                code = 1;
                break;
            }
        }
    }
    // The schema as this run leaves it, on the run's last row: the next run reads whether anything
    // changed the database since.
    if let Some(name) = last {
        let value = fingerprint(db, opts.scope()).await?;
        set_fingerprint(db, &name, &value).await?;
    }
    Ok(code)
}

/// `reylo migrate check`: everything apply checks, and the migrations against schema.sql; runs
/// nothing, and writes nothing to the database.
pub async fn check_command(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    swept_note(swept, log);
    let result = check(&mut db, &mut server, opts, Mode::Check, &[], log).await;
    db.close().await;
    server.close().await;
    let plan = result?;
    if !plan.ok() {
        print_problems(&plan.problems, log);
        return Ok(1);
    }
    log(&format!("{} The migrations reach {}, and this database matches its history", paint("green", "✓"), shown(&opts.schema)));
    if plan.unchanged == Some(true) {
        log(&format!("{} Nothing has changed this database since Reylo last applied to it", paint("green", "✓")));
    }
    match plan.pending.len() {
        0 => log(&format!("{} Nothing is pending", paint("green", "✓"))),
        n => {
            log(&format!("{} {n} pending, and each runs in a shadow database:", paint("green", "✓")));
            for m in &plan.pending {
                log(&format!("    {}", m.name));
            }
        }
    }
    Ok(0)
}

/// `reylo migrate apply --reset`: drops everything the migrations could have made, applies every
/// migration, then runs `post`.
pub async fn reset(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let wiped = wipe(&mut db).await;
    db.close().await;
    wiped?;
    log(&paint("dim", "dropped everything in the database"));
    let code = migrate(opts, &[], log).await?;
    let Some(post) = opts.post.as_deref().filter(|_| code == 0) else { return Ok(code) };
    log(&paint("dim", &format!("post: {post}")));
    let mut command = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.arg("/C").arg(post);
        c
    } else {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(post);
        c
    };
    let status = command.current_dir(&opts.root).env("DATABASE_URL", &opts.url).status()?;
    if !status.success() {
        log(&format!("{} post exited with {}", paint("red", "✗"), status.code().map_or_else(|| "a signal".to_string(), |c| c.to_string())));
        return Ok(1);
    }
    Ok(0)
}

/// `reylo migrate repair`: when the live database, `schema.sql` and every migration replayed by time
/// agree, rewrites the history table to the migration files, in time order, running none of them.
/// It touches nothing but the history table.
pub async fn repair(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, _) = open(opts).await?;
    let result = repair_on(&mut db, &mut server, opts, log).await;
    db.close().await;
    server.close().await;
    result
}

async fn repair_on(db: &mut Db, server: &mut Server, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let _held = lock(db, opts).await?;
    let scope = opts.scope();
    let (files, _) = read_migrations(&opts.dir)?;
    let live = catalog(db, scope).await?;
    let schema_sql = std::fs::read_to_string(&opts.schema).map_err(|e| Error(format!("cannot read {}: {e}", shown(&opts.schema))))?;
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain([schema_sql.as_str()]).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, scope, &texts).await;
    let built = async {
        let a_lane = shadows.lane();
        shadows.push(a_lane, [Step::script("schema.sql", &schema_sql)]);
        let a = shadows.lines(a_lane).await?;
        let c_lane = shadows.lane();
        shadows.push(c_lane, files.iter().map(Step::of));
        let c = shadows.lines(c_lane).await?;
        Ok::<_, Error>((a, c))
    }
    .await;
    shadows.finish().await;
    let (a, c) = built?;
    let mut issues = Vec::new();
    match (&a, &c) {
        (Err((_, e)), _) => issues.push(Problem::new("schema.sql fails on an empty database").note(e)),
        (_, Err((m, e))) => issues.push(Problem::new(format!("{m} fails when every migration runs in name order")).note(e)),
        (Ok(a), Ok(c)) => {
            if *a != live {
                issues.push(Problem::new("The database differs from schema.sql").lines(describe_diff("schema.sql", a, "database", &live)));
            }
            if *c != live {
                issues.push(Problem::new("The database differs from the migrations").lines(describe_diff("migrations", c, "database", &live)));
            }
        }
    }
    if !issues.is_empty() {
        print_problems(&issues, log);
        log("Nothing was changed. repair only rewrites the history of a database that already matches; change the database with a migration, or reset a development database.");
        return Ok(1);
    }
    let before = read_history(db).await?;
    let d = db.dialect();
    ensure_history(db).await?;
    db.exec(if d == Dialect::Mysql { "start transaction" } else { "begin" }).await?;
    let written = async {
        db.exec(&format!("delete from {HISTORY_TABLE}")).await?;
        for f in &files {
            insert_row(db, f, true).await?;
        }
        db.exec("commit").await
    }
    .await;
    if let Err(e) = written {
        let _ = db.exec("rollback").await;
        return Err(e);
    }
    for f in &files {
        if !before.iter().any(|r| r.applied && r.name == f.name) {
            log(&format!("  + {} recorded as applied, not run", f.name));
        }
    }
    for r in &before {
        if !r.applied || !files.iter().any(|f| f.name == r.name) {
            log(&format!("  - {} removed from the history", r.name));
        }
    }
    log(&format!("{} the history matches the migration files", paint("green", "✓")));
    Ok(0)
}

/// `reylo migrate restore <migration>`: writes the text the database ran for a migration back to
/// its file, from the history table, which keeps every applied migration's text: for a migration
/// edited after it was applied, or deleted. Only the file is written.
pub async fn restore(opts: &Options, name: &str, log: Log<'_>) -> Result<i32, Error> {
    let base = Path::new(name).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let history = read_history(&mut db).await;
    db.close().await;
    let Some(row) = history?.into_iter().find(|r| r.applied && r.name == base) else {
        return Err(Error(format!("{base} isn't in this database's history: only a migration it applied can be restored from it")));
    };
    let path = opts.dir.join(&base);
    let current = std::fs::read_to_string(&path).ok();
    if current.as_deref().is_some_and(|c| normalized(c) == normalized(&row.sql)) {
        log(&format!("{} {} already holds the text this database ran", paint("green", "✓"), shown(&path)));
        return Ok(0);
    }
    std::fs::create_dir_all(&opts.dir)?;
    std::fs::write(&path, &row.sql)?;
    match current {
        Some(old) => {
            log(&format!("{} Restored {} to the text this database ran:", paint("green", "✓"), shown(&path)));
            for line in line_diff(&old, &row.sql).into_iter().skip(2) {
                log(&line);
            }
        }
        None => log(&format!("{} Restored {} from this database's history", paint("green", "✓"), shown(&path))),
    }
    Ok(0)
}

/// `reylo migrate create <name>`: a migration named with the current UTC time, refused while any
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
    if !VALID.is_match(name) {
        return Err(Error(format!("a migration's name is a-z, 0-9 and _: {name}")));
    }
    let (files, _) = read_migrations(&opts.dir)?;
    // A linear history's lock must be in order before a migration is added to it; a missing one is
    // written from the files.
    if opts.linear && lock::read(&opts.dir) != lock::State::Missing {
        let problems = lock_problems(opts, &files);
        if !problems.is_empty() {
            print_problems(&problems, log);
            return Ok(1);
        }
    }
    // The plan starts from every migration file, drafts not yet applied anywhere included, and is
    // named after the newest of them.
    let schema_sql = std::fs::read_to_string(&opts.schema).map_err(|e| Error(format!("cannot read {}: {e}", shown(&opts.schema))))?;
    let (texts, code) = match draft(opts, &files, &schema_sql, name, choices, log).await? {
        Ok(drafted) => drafted,
        Err(problems) => {
            print_problems(&problems, log);
            log("");
            log(&paint("dim", "No migration was written."));
            return Ok(1);
        }
    };
    std::fs::create_dir_all(&opts.dir)?;
    // After the newest migration, should one share this second; on MySQL a plan of several DDL
    // statements is several files, a second apart, `<name>`, `<name>_2`, ….
    let newest = files.iter().filter_map(|f| time::seconds(&f.time)).max();
    let at = time::now_seconds().max(newest.map_or(0, |n| n + 1));
    for (k, text) in texts.iter().enumerate() {
        let suffix = if k == 0 { String::new() } else { format!("_{}", k + 1) };
        let file = opts.dir.join(format!("{}_{name}{suffix}.sql", time::stamp(at + k as i64)));
        std::fs::write(&file, text)?;
        log(&shown(&file));
    }
    if opts.linear {
        write_lock(opts)?;
        log(&shown(&opts.dir.join(LOCK)));
    }
    Ok(code)
}

/// Writes the lock of a linear history: the migration files in the order they run.
fn write_lock(opts: &Options) -> Result<(), Error> {
    let (files, _) = read_migrations(&opts.dir)?;
    lock::write(&opts.dir, &files.iter().map(|f| f.name.clone()).collect::<Vec<_>>())?;
    Ok(())
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

/// The new migration's text: the diff engine's plan from the migrations replayed by time to
/// `schema.sql`, run on the replay and compared with `schema.sql` before it is written. A plan that
/// does not reach it is written all the same, with what remains, for the person (or agent) to finish.
/// Whether an object's id (`column public.users.nick`, `table users`) is what a name written on the
/// command line names: the object's own name, or its table's or schema's, quotes aside.
fn names(id: &str, written: &str) -> bool {
    let bare = id.split_once(' ').map_or(id, |(_, rest)| rest).replace('"', "");
    let written = written.replace(['"', '`'], "");
    bare == written || bare.ends_with(&format!(".{written}")) || bare.starts_with(&format!("{written}."))
}

/// The problems with what `choices` states about `plan`: every table or column it drops must be
/// named in `--drop`, and every name in `--drop` must be one it drops.
fn drop_problems(plan: &crate::diff::Plan, choices: &Choices, migration: &str) -> Vec<Problem> {
    let shown_loss = |l: &str| l.trim_start_matches("column ").trim_start_matches("table ").to_string();
    let mut problems = Vec::new();
    let unstated: Vec<&String> = plan.losses.iter().filter(|l| !choices.drop.iter().any(|w| names(l, w))).collect();
    if !unstated.is_empty() {
        let list = unstated.iter().map(|l| shown_loss(l)).collect::<Vec<_>>();
        let columns: Vec<&String> = unstated.iter().copied().filter(|l| l.starts_with("column ")).collect();
        let mut problem = Problem::new(format!("This migration would drop {}, and {} data", list.join(", "), if list.len() == 1 { "its" } else { "their" }))
            .fix(if list.len() == 1 { "To drop it" } else { "To drop them" }, &[&format!("reylo migrate create {migration} --drop {}", list.join(","))]);
        if let Some(c) = columns.first() {
            problem = problem.fix("If a column was renamed instead", &[&format!("reylo migrate create {migration} --rename {}=<new name>", shown_loss(c))]);
        }
        problems.push(problem);
    }
    for w in &choices.drop {
        let hits: Vec<&String> = plan.losses.iter().filter(|l| names(l, w)).collect();
        if hits.is_empty() {
            problems.push(Problem::new(format!("--drop names {w}, which this migration doesn't drop")).note(if plan.losses.is_empty() {
                "It drops no table or column.".to_string()
            } else {
                format!("It drops {}.", plan.losses.iter().map(|l| shown_loss(l)).collect::<Vec<_>>().join(", "))
            }));
        }
    }
    problems
}

/// The columns `choices` renames, as (old, new) ids of `from` and `to`; or why one cannot be.
fn stated_renames(from: &crate::diff::Schema, to: &crate::diff::Schema, choices: &Choices) -> Result<Vec<(String, String)>, Vec<Problem>> {
    let mut out = Vec::new();
    let mut problems = Vec::new();
    for (old, new) in &choices.rename {
        let olds: Vec<&String> = from.objects.keys().filter(|id| id.starts_with("column ") && names(id, old) && !to.objects.contains_key(*id)).collect();
        let Some(old_id) = (olds.len() == 1).then(|| olds[0].clone()) else {
            problems.push(match olds.len() {
                0 => Problem::new(format!("--rename names {old}, which isn't a column this migration removes")).note("A table is renamed by hand in the migration: alter table … rename to …, which apply then checks."),
                _ => Problem::new(format!("--rename names {old}, which could be any of {}", olds.iter().map(|o| o.trim_start_matches("column ")).collect::<Vec<_>>().join(", "))).note("Name it with its schema."),
            });
            continue;
        };
        let table = old_id.rsplit_once('.').map(|(t, _)| t.to_string()).unwrap_or_default();
        let new_id = format!("{table}.{new}");
        if !to.objects.contains_key(&new_id) || from.objects.contains_key(&new_id) {
            problems.push(Problem::new(format!("--rename gives {old} the name {new}, which isn't a column this migration adds to its table")));
            continue;
        }
        out.push((old_id, new_id));
    }
    if problems.is_empty() { Ok(out) } else { Err(problems) }
}

#[allow(clippy::type_complexity)]
async fn draft(opts: &Options, files: &[Migration], schema_sql: &str, migration: &str, choices: &Choices, log: Log<'_>) -> Result<Result<(Vec<String>, i32), Vec<Problem>>, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let mut server = Server::open(&mut db, &opts.url, opts.shadow_url.as_deref()).await?;
    server.copy_default_privileges(!sets_default_privileges(opts));
    db.close().await;
    let scope = opts.scope();
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain([schema_sql]).collect();
    let mut shadows = Shadows::new(&mut server, &opts.url, &opts.root, scope, &texts).await;
    let result = async {
        let a = shadows.lane();
        shadows.push(a, [Step::script("schema.sql", schema_sql)]);
        if let Err((_, e)) = shadows.open(a).await? {
            return Err(Error(format!("schema.sql fails on an empty database: {e}")));
        }
        // The plan runs on the replay by time, which is then no longer one: the replay is kept
        // first, as the start of the check that follows.
        let c = shadows.lane();
        shadows.push(c, files.iter().map(Step::of));
        shadows.mark(c);
        if let Err((m, e)) = shadows.open(c).await? {
            return Err(Error(format!("{m} fails when every migration replays by time: {e}")));
        }
        shadows.taint(c);
        let mut target = match crate::diff::read(shadows.db(a)).await {
            Ok(schema) => schema,
            Err(e) => {
                log(&paint("yellow", &format!("{e}: the migration is empty, for the change to {} to be written by hand", shown(&opts.schema))));
                return Ok(Ok((vec![String::new()], 0)));
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
        let alterer = crate::diff::alterer(shadows.db(c)).await?;
        let renames = match stated_renames(&current, &target, choices) {
            Ok(renames) => renames,
            Err(problems) => return Ok(Err(problems)),
        };
        let dropped: std::collections::BTreeSet<String> = current.objects.keys().filter(|id| id.starts_with("column ") && !target.objects.contains_key(*id) && choices.drop.iter().any(|w| names(id, w))).cloned().collect();
        let mut plan = crate::diff::plan_with(&current, &target, alterer.as_ref(), &renames, &dropped);
        style(shadows.db(c), &mut plan, &target).await?;
        let problems = drop_problems(&plan, choices, migration);
        if !problems.is_empty() {
            return Ok(Err(problems));
        }
        if plan.statements.is_empty() && moves.is_empty() {
            log(&paint("dim", &format!("the migrations already make {}: the migration is empty, for a change of data", shown(&opts.schema))));
            return Ok(Ok((vec![String::new()], 0)));
        }
        // The plan runs on the replay as `apply` will run its files: one file in one transaction (on
        // SQLite with foreign keys off until it is checked), or on MySQL a file per statement.
        let d = shadows.db(c).dialect();
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
            Err((_, e)) => return Err(Error(format!("schema.sql fails on an empty database: {e}"))),
        };
        let reached = catalog(shadows.db(c), scope).await?;
        plan.statements.splice(0..0, moves);
        let mut texts = plan_files(&plan, d);
        for loss in &plan.losses {
            log(&format!("{} Drops {}, and its data", paint("yellow", "!"), loss.trim_start_matches("column ").trim_start_matches("table ")));
        }
        for (old, new) in &plan.renames {
            log(&format!("{} Renames {} to {new}, keeping its rows: no other column of its definition came or went in its table. If they are two columns, state it: --drop {}", paint("yellow", "!"), old.trim_start_matches("column "), old.trim_start_matches("column ")));
        }
        if failed.is_none() && reached == wanted {
            return Ok(Ok((texts, 0)));
        }
        let mut remaining = Vec::new();
        if let Some(f) = &failed {
            remaining.push(format!("fails at {f}"));
        }
        let (only_a, only_b) = difference(&reached, &wanted);
        remaining.extend(only_a.iter().map(|l| format!("- {l}")));
        remaining.extend(only_b.iter().map(|l| format!("+ {l}")));
        log(&format!("{} the plan does not reach {}; the file holds it with what remains, to finish by hand:", paint("red", "✗"), shown(&opts.schema)));
        for r in &remaining {
            log(&format!("  {r}"));
        }
        texts[0] = format!("-- This plan does not reach schema.sql yet. What remains:\n{}\n{}", remaining.iter().map(|r| format!("--   {}", r.replace('\n', " "))).collect::<Vec<_>>().join("\n"), texts[0]);
        Ok(Ok((texts, 1)))
    }
    .await;
    shadows.finish().await;
    server.close().await;
    result
}

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").args(args).current_dir(cwd).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `reylo migrate rebase [files…]`: gives migrations new times after the newest of the others, one
/// second apart and in their order, by renaming them. Named, it moves those files. Bare, with a
/// linear history, it puts the files in the lock's order; without one, it takes this branch's
/// migrations: those the main branch does not have, added (`git diff --diff-filter=A <main>`) or
/// untracked. A linear history's lock is written again after.
pub async fn rebase(opts: &Options, named: &[String], log: Log<'_>) -> Result<i32, Error> {
    let (files, _) = read_migrations(&opts.dir)?;
    if named.is_empty() && opts.linear {
        return rebase_linear(opts, &files, log);
    }
    let mut moving: Vec<Migration> = if !named.is_empty() {
        named
            .iter()
            .map(|n| {
                let base = Path::new(n).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                files.iter().find(|f| f.name == base).cloned().ok_or_else(|| Error(format!("{n} is not a migration in {}", shown(&opts.dir))))
            })
            .collect::<Result<_, _>>()?
    } else {
        let dir = &opts.dir;
        if git(dir, &["rev-parse", "--show-toplevel"]).is_none() {
            return Err(Error("not in a git repository: name the files to rebase".into()));
        }
        let main = match &opts.main_branch {
            Some(m) => m.clone(),
            None => git(dir, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]).map(|s| s.trim().to_string()).unwrap_or_default(),
        };
        if main.is_empty() {
            return Err(Error("no main branch: set migrate.main_branch or pass --base, or name the files to rebase".into()));
        }
        let added = git(dir, &["diff", "--name-only", "--diff-filter=A", &main, "--", "."]).ok_or_else(|| Error(format!("git cannot compare with {main}")))?;
        let untracked = git(dir, &["ls-files", "--others", "--exclude-standard", "--", "."]).unwrap_or_default();
        let mine: Vec<String> = added.lines().chain(untracked.lines()).filter(|l| !l.is_empty()).filter_map(|p| Path::new(p).file_name().map(|f| f.to_string_lossy().into_owned())).collect();
        files.iter().filter(|f| mine.contains(&f.name)).cloned().collect()
    };
    moving.retain(|f| f.name != INIT);
    moving.sort_by(|a, b| a.time.cmp(&b.time).then(a.name.cmp(&b.name)));
    if moving.is_empty() {
        log("nothing to rebase");
    } else {
        let newest = files.iter().filter(|f| f.name != INIT && !moving.iter().any(|m| m.name == f.name)).filter_map(|f| time::seconds(&f.time)).max();
        rename_after(opts, &moving, newest, log)?;
    }
    if opts.linear {
        write_lock(opts)?;
    }
    Ok(0)
}

/// Renames `moving`, in its order, to times one second apart after `newest` (and not before now).
/// Answers each file's old and new name.
fn rename_after(opts: &Options, moving: &[Migration], newest: Option<i64>, log: Log<'_>) -> Result<Vec<(String, String)>, Error> {
    let mut t = time::now_seconds().max(newest.map_or(0, |n| n + 1));
    let mut renamed = Vec::new();
    for f in moving {
        let to = format!("{}{}", time::stamp(t), &f.name[14..]);
        std::fs::rename(&f.path, opts.dir.join(&to))?;
        log(&format!("  {} → {to}", f.name));
        renamed.push((f.name.clone(), to));
        t += 1;
    }
    Ok(renamed)
}

/// A bare `rebase` with a linear history: the order is the lock's, and the files are renamed so that
/// their names sort in it. In a conflict, the order is the other side's migrations, then this
/// branch's that the other side lacks, from the two sides git keeps of the lock; the resolved lock
/// and the renamed files are staged, so the merge or rebase can go on. Otherwise it is the lock's,
/// then the files the lock lacks (written by hand), by name. Only from the first file whose name
/// sorts before one listed ahead of it are files renamed, so that nothing another branch merged,
/// and another database may have applied, is moved.
fn rebase_linear(opts: &Options, files: &[Migration], log: Log<'_>) -> Result<i32, Error> {
    let state = lock::read(&opts.dir);
    let conflicted = state == lock::State::Conflicted;
    let listed: Vec<String> = match state {
        lock::State::Names(names) => names,
        lock::State::Conflicted => {
            let (mine, theirs) = lock::sides(&opts.dir, opts.main_branch.as_deref())?;
            theirs.iter().cloned().chain(mine.into_iter().filter(|n| !theirs.contains(n))).collect()
        }
        lock::State::Missing | lock::State::Invalid(_) => vec![],
    };
    let mut order: Vec<&Migration> = listed.iter().filter_map(|n| files.iter().find(|f| &f.name == n)).collect();
    order.extend(files.iter().filter(|f| !listed.contains(&f.name)));
    let mut newest: Option<i64> = None;
    let mut moving: Vec<Migration> = Vec::new();
    for f in order {
        let t = time::seconds(&f.time);
        if f.name != INIT && (!moving.is_empty() || (t.is_some() && t <= newest)) {
            moving.push(f.clone());
        } else if f.name != INIT {
            newest = newest.max(t);
        }
    }
    let renamed = if moving.is_empty() {
        log("nothing to rebase");
        vec![]
    } else {
        rename_after(opts, &moving, newest, log)?
    };
    write_lock(opts)?;
    let path = shown(&opts.dir.join(LOCK));
    if conflicted {
        let mut paths = vec![LOCK.to_string()];
        for (from, to) in renamed {
            paths.push(from);
            paths.push(to);
        }
        lock::stage(&opts.dir, &paths)?;
        log(&format!("{} resolved {path}: go on with git rebase --continue, or git commit for a merge", paint("green", "✓")));
    } else {
        log(&format!("{} {path} lists the migrations in the order they run", paint("green", "✓")));
    }
    Ok(0)
}

// ---------------------------------------------------------------------------------------------
// `reylo migrate init`: the schema of an existing database, written as schema.sql and as the one
// migration Reylo writes, migrations/init.sql, which the database then records without running.
// Postgres's comes from pg_dump, MySQL's from the server's own `show create`, SQLite's from the
// `create` text it stores.

async fn dump(opts: &Options, schemas: &[String]) -> Result<String, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let result = dump_from(&mut db, opts, schemas).await;
    db.close().await;
    result
}

async fn dump_from(db: &mut Db, opts: &Options, schemas: &[String]) -> Result<String, Error> {
    match db.dialect() {
        Dialect::Postgres => {
            // pg_dump refuses a server newer than itself, so the one of the server's own major
            // version is looked for first (Homebrew's keg), then the PATH's.
            let version: i64 = db.rows("select current_setting('server_version_num') as v").await?.first().and_then(|r| r.first()?.parse().ok()).unwrap_or(0);
            let major = version / 10000;
            let version_of = |p: &str| -> i64 {
                let out = std::process::Command::new(p).arg("--version").output().ok();
                let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
                text.split_whitespace().find_map(|w| w.split('.').next()?.parse().ok()).unwrap_or(0)
            };
            let candidates = [format!("/opt/homebrew/opt/postgresql@{major}/bin/pg_dump"), "pg_dump".to_string()];
            let Some(pg_dump) = candidates.iter().find(|p| version_of(p) >= major) else {
                return Err(Error(format!("init needs pg_dump {major} or newer (the server is Postgres {major})")));
            };
            let mut args: Vec<String> = ["--schema-only", "--no-owner", "--no-publications", "--no-subscriptions", "--no-security-labels", "--no-tablespaces"].map(String::from).to_vec();
            args.push(format!("--exclude-table=*.{HISTORY_TABLE}"));
            let named: Vec<String> = if schemas.is_empty() { vec!["public".into()] } else { schemas.to_vec() };
            for s in named {
                args.push("--schema".into());
                args.push(s);
            }
            args.push(opts.url.clone());
            let out = std::process::Command::new(pg_dump).args(&args).output()?;
            if !out.status.success() {
                return Err(Error(format!("pg_dump failed: {}", String::from_utf8_lossy(&out.stderr).trim())));
            }
            // What pg_dump writes for psql and for a session of its own: meta-commands, its settings
            // (an empty search_path would outlive the file in a shadow's session) and comments.
            let text = String::from_utf8_lossy(&out.stdout);
            let kept: Vec<&str> = text.lines().filter(|l| !l.starts_with('\\') && !l.starts_with("SET ") && !l.starts_with("SELECT pg_catalog.set_config") && !l.starts_with("--")).collect();
            let joined = kept.join("\n").replace("\nCREATE SCHEMA public;\n", "\nCREATE SCHEMA IF NOT EXISTS public;\n");
            static BLANKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").expect("a valid pattern"));
            Ok(format!("{}\n", BLANKS.replace_all(&joined, "\n\n").trim()))
        }
        Dialect::Sqlite => {
            let rows = db
                .rows(&format!(
                    "select sql from sqlite_schema where sql is not null and name not like 'sqlite_%' and tbl_name <> '{HISTORY_TABLE}' order by case type when 'table' then 0 when 'index' then 1 when 'view' then 2 else 3 end, rowid"
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
            for t in tables.iter().filter(|t| t.text("t") == "VIEW") {
                let row = db.rows(&format!("show create view `{}`", t.text("n"))).await?;
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
            out.push("set foreign_key_checks = 1;".into());
            Ok(format!("{}\n", out.join("\n\n")))
        }
    }
}

/// `reylo migrate init`: writes `schema.sql` and `migrations/init.sql` from the database, then
/// applies, which records init.sql without running it, since the database already has its schema.
pub async fn init(opts: &Options, schemas: &[String], log: Log<'_>) -> Result<i32, Error> {
    if opts.schema.exists() {
        return Err(Error(format!("{} exists already: init writes it from the database", shown(&opts.schema))));
    }
    if !read_migrations(&opts.dir)?.0.is_empty() {
        return Err(Error(format!("{} holds migrations already: init starts a project's history", shown(&opts.dir))));
    }
    let sql = dump(opts, schemas).await?;
    std::fs::create_dir_all(&opts.dir)?;
    std::fs::write(&opts.schema, &sql)?;
    let init = opts.dir.join(INIT);
    std::fs::write(&init, &sql)?;
    log(&format!("wrote {} and {}", shown(&opts.schema), shown(&init)));
    // The database already has init.sql's schema, which holds no data statements: recorded.
    migrate(opts, &[INIT.to_string()], log).await
}

/// `reylo run "<sql>"` and `reylo run --file <path>`: runs SQL on the database as written, inline
/// text or files, in the order given; records nothing. For a change typed by hand during
/// development, and a seed after `apply --reset` (`post: reylo run --file seed.sql`).
pub async fn run_sql(url: &str, root: &Path, items: &[(String, String)], log: Log<'_>) -> Result<i32, Error> {
    let mut db = Db::connect(url, root).await?;
    let mut code = 0;
    for (label, sql) in items {
        match db.script(sql).await {
            Ok(()) => log(&format!("{} {label}", paint("green", "✓"))),
            Err(e) => {
                log(&format!("{} {e}", paint("red", "✗")));
                code = 1;
                break;
            }
        }
    }
    db.close().await;
    Ok(code)
}
