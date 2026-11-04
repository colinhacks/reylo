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
    /// The invalid indexes such statements left, which apply drops before running them again.
    pub leftovers: Vec<Leftover>,
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
        let sql = m.sql.trim().trim_end_matches(';').trim();
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
                let bare = stored(&last_written(&c[2]));
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

/// What is wrong with the lock of a linear history: it must list the migration files, each once, in
/// the order of their names, which is the order they run in.
fn lock_problems(opts: &Options, files: &[Migration]) -> Vec<Problem> {
    let path = shown(&opts.dir.join(LOCK));
    let names = match lock::read(&opts.dir) {
        lock::State::Missing => return vec![Problem::new(format!("The history is linear, and {path} is missing")).fix("Write it from the migration files", &["reylo migrate lock"])],
        lock::State::Conflicted => return vec![Problem::new(format!("{path} has a merge conflict: another branch added migrations too")).fix("Move this branch's migrations after the others', and resolve it", &["reylo migrate lock"])],
        lock::State::Invalid(e) => return vec![Problem::new(format!("{path} doesn't read as a lock file")).note(e).fix("Write it again from the migration files", &["reylo migrate lock"])],
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
    vec![Problem::new(format!("{path} doesn't list the migration files in the order they run")).lines(lines).fix("Put them in its order", &["reylo migrate lock"])]
}

/// Checks the target against the migration files, and in `Mode::Check` the migrations against
/// schema.sql too; changes nothing. Nothing is ever recorded without running but a lone statement
/// of apply's own, cut off after it ran and before its row was finished, which shadows show landed.
pub async fn check(db: &mut Db, server: &mut Server, opts: &Options, mode: Mode, log: Log<'_>) -> Result<Plan, Error> {
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
    // Nothing is recorded without running but what a cut-off run of apply's own left (below).
    let records: Vec<Migration> = Vec::new();
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

    // An index a cut-off `create index concurrently` left invalid is dropped before its migration
    // runs again, so it is compared as gone.
    let leftovers = leftovers(db, &history).await?;
    let live: Vec<String> = catalog(db, scope).await?.into_iter().filter(|l| !leftovers.iter().any(|x| l.starts_with(&x.line))).collect();
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
    Ok(Plan { problems, records: run.records, pending: run.pending, unchanged, landed: run.landed, leftovers })
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
        problem = problem.fix(format!("{} {} exactly that change. Record the history the database has, running nothing", hint.join(" and "), if hint.len() == 1 { "makes" } else { "make" }), &["reylo migrate sync-history"]);
    } else if !only_live.is_empty() {
        problem = problem.fix("Keep it: add it to schema.sql, write the migration, then record the history the database has, running nothing", &["reylo migrate create <name>", "reylo migrate sync-history"]);
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

/// `reylo migrate apply`: checks the database against the migrations, then applies what is pending.
/// Refuses on any disagreement, and runs nothing then.
pub async fn apply(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    let result = apply_on(&mut db, &mut server, swept, opts, log).await;
    db.close().await;
    server.close().await;
    result
}

fn swept_note(swept: usize, log: Log<'_>) {
    if swept > 0 {
        log(&paint("dim", &format!("dropped {swept} shadow database{} a run left behind", if swept == 1 { "" } else { "s" })));
    }
}

async fn apply_on(db: &mut Db, server: &mut Server, swept: usize, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    swept_note(swept, log);
    let _held = lock(db, opts).await?;
    let plan = check(db, server, opts, Mode::Apply, log).await?;
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
    drop_leftovers(db, &plan.leftovers, log).await?;
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
    if last.is_some() {
        // On the row a later read takes as the last, which a tie on the clock may make another.
        if let Some(row) = read_history(db).await?.iter().rfind(|r| r.applied) {
            let value = fingerprint(db, opts.scope()).await?;
            set_fingerprint(db, &row.name, &value).await?;
        }
    }
    Ok(code)
}

/// `reylo migrate check`: everything apply checks, and the migrations against schema.sql; runs
/// nothing, and writes nothing to the database.
pub async fn check_command(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    swept_note(swept, log);
    let result = check(&mut db, &mut server, opts, Mode::Check, log).await;
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
    for x in &plan.leftovers {
        log(&format!("{} {} was cut off and left {} invalid: apply drops it, then runs {} again", paint("yellow", "!"), x.migration, x.index, x.migration));
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
    let code = apply(opts, log).await?;
    if code != 0 {
        return Ok(code);
    }
    run_post(opts, log)
}

/// Runs `migrate.post` (a seed) in the project's directory, with `DATABASE_URL` set to the database.
fn run_post(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let Some(post) = opts.post.as_deref() else { return Ok(0) };
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

/// `reylo migrate wipe`: deletes every row of the project's tables, and nothing else: the schema and
/// the history stay, identities and sequences start again, and `post` (a seed) runs after. The
/// tables are those the comparison reads, so a platform's own (Supabase's auth.users) keep theirs.
pub async fn wipe_rows(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let result = wipe_rows_on(&mut db).await;
    db.close().await;
    let tables = result?;
    log(&format!("{} Deleted every row of {} table{}; the schema and the history stay", paint("green", "✓"), tables, if tables == 1 { "" } else { "s" }));
    run_post(opts, log)
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

/// `reylo migrate sync-history`: rewrites the database's history to the migrations its schema
/// already has, running nothing: the longest run of migrations, in their order, whose replay in a
/// shadow database equals the live database. Those after it stay pending, and so do the migrations
/// with data statements at its end, which a schema can't show ran. For a database changed by hand
/// (a hotfix, then written as a migration), and one whose history was lost or kept by another tool.
pub async fn sync_history(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    swept_note(swept, log);
    let result = sync_history_on(&mut db, &mut server, opts, log).await;
    db.close().await;
    server.close().await;
    result
}

async fn sync_history_on(db: &mut Db, server: &mut Server, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let _held = lock(db, opts).await?;
    let d = db.dialect();
    let scope = opts.scope();
    let (files, strays) = read_migrations(&opts.dir)?;
    let problems = file_problems(opts, &files, &strays, None, d);
    if !problems.is_empty() {
        print_problems(&problems, log);
        log("");
        log(&paint("dim", "Nothing was changed."));
        return Ok(1);
    }
    let order = run_order(opts, &files);
    let live = catalog(db, scope).await?;
    let texts: Vec<&str> = order.iter().map(|f| f.sql.as_str()).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, scope, &texts).await;
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
            let now = catalog(shadows.db(lane), scope).await?;
            if now == live {
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
    shadows.finish().await;
    let (matches, closest) = match replayed? {
        Ok(found) => found,
        Err(e) => {
            print_problems(&[Problem::new("The migrations can't be replayed").note(e)], log);
            return Ok(1);
        }
    };
    let Some(mut n) = matches.last().copied() else {
        let (_, k, lines) = closest;
        let after = if k == 0 { "no migration".to_string() } else { order[k - 1].name.clone() };
        print_problems(&[Problem::new(format!("This database's schema isn't what any run of the migrations makes; the closest is through {after}")).lines(describe_diff("the migrations", &lines, "this database", &live)).fix("Write what the database has into schema.sql, then a migration for it", &["reylo migrate create <name>"])], log);
        log("");
        log(&paint("dim", "Nothing was changed."));
        return Ok(1);
    };
    // A migration at the end of the run that changes data and nothing a schema shows can't be told
    // to have run: it stays pending, to run.
    while n > 0 && !data_statements(&order[n - 1].sql, d).is_empty() && matches.contains(&(n - 1)) {
        n -= 1;
    }
    let recorded = &order[..n];
    let before = read_history(db).await?;
    ensure_history(db).await?;
    db.exec(if d == Dialect::Mysql { "start transaction" } else { "begin" }).await?;
    let written = async {
        db.exec(&format!("delete from {HISTORY_TABLE}")).await?;
        for f in recorded {
            insert_row(db, f, true).await?;
        }
        db.exec("commit").await
    }
    .await;
    if let Err(e) = written {
        let _ = db.exec("rollback").await;
        return Err(e);
    }
    if let Some(last) = read_history(db).await?.iter().rfind(|r| r.applied) {
        let value = fingerprint(db, scope).await?;
        set_fingerprint(db, &last.name, &value).await?;
    }
    for f in recorded {
        let ran = before.iter().any(|r| r.applied && r.name == f.name && same_statements(&r.sql, &f.sql, d));
        if !ran {
            let data = data_statements(&f.sql, d);
            if data.is_empty() {
                log(&format!("  + {} recorded, not run: this database has its change", f.name));
            } else {
                log(&format!("  + {} recorded, not run: {} {}", f.name, paint("yellow", "its data statements never ran here"), paint("dim", &format!("({})", data.join(", ")))));
            }
        }
    }
    for r in &before {
        if !recorded.iter().any(|f| f.name == r.name && r.applied && same_statements(&r.sql, &f.sql, d)) {
            log(&format!("  - {} taken out of the history", r.name));
        }
    }
    let pending = &order[n..];
    log(&format!("{} The history lists the {} migration{} this database's schema has", paint("green", "✓"), recorded.len(), if recorded.len() == 1 { "" } else { "s" }));
    if !pending.is_empty() {
        log(&format!("  {} pending, which apply runs: {}", pending.len(), pending.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")));
    }
    Ok(0)
}

/// The order migrations run in: the files by name.
fn run_order(opts: &Options, files: &[Migration]) -> Vec<Migration> {
    let _ = opts;
    files.to_vec()
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
async fn rename_tables(db: &mut Db, current: &crate::diff::Schema, target: &Target, renamed: &[(String, String, bool)], log: Log<'_>) -> Result<Vec<String>, Error> {
    let d = db.dialect();
    let mut statements = table_rename_statements(d, current, &target.schema, renamed, log);
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
async fn plan_to(db: &mut Db, target: &Target) -> Result<(Vec<String>, crate::diff::Plan), Error> {
    let mut current = crate::diff::read(db).await?;
    let renamed = crate::diff::table_renames(&current, &target.schema, &[], &std::collections::BTreeSet::new());
    let renames = if renamed.is_empty() { vec![] } else { rename_tables(db, &current, target, &renamed, &mut |_| {}).await? };
    if !renames.is_empty() {
        current = crate::diff::read(db).await?;
    }
    let alterer = crate::diff::alterer(db).await?;
    let mut plan = crate::diff::plan(&current, &target.schema, alterer.as_ref());
    style(db, &mut plan, &target.schema).await?;
    Ok((renames, plan))
}

/// The statements that rename the tables `renamed` names (each its old id in `current`, its new id
/// in `target`, and whether --rename stated it), and on Postgres what is named after them; a guessed
/// rename is said, and written down above its statement with what to do instead.
fn table_rename_statements(d: Dialect, current: &crate::diff::Schema, target: &crate::diff::Schema, renamed: &[(String, String, bool)], log: Log<'_>) -> Vec<String> {
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
        let new_name = if d == Dialect::Postgres { last_written(&to_w) } else { to_w };
        let (old, new) = (bare(old_id), bare(new_id));
        if !was_stated {
            out.push(format!("-- {old} is renamed {new}, keeping its rows. If they are two tables, drop the one and create the other instead."));
            log(&format!("{} Renames the table {old} to {new}, keeping its rows: no other table of its columns came or went. If they are two tables, state it: --drop {old}", paint("yellow", "!")));
        }
        out.push(format!("alter {}table {from_w} rename to {new_name}", if foreign { "foreign " } else { "" }));
        names.push((old, new, old_id.clone(), new_id.clone()));
    }
    if d == Dialect::Postgres {
        out.extend(crate::diff::postgres::derived_renames(current, target, &names));
    }
    out
}

/// A dotted name's last part, as written, quotes and all: `"Accounts"` of `public."Accounts"`.
fn last_written(name: &str) -> String {
    let mut quote: Option<char> = None;
    let mut at = 0;
    for (k, ch) in name.char_indices() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if ch == '"' || ch == '`' => quote = Some(ch),
            None if ch == '.' => at = k + 1,
            None => {}
        }
    }
    name[at..].to_string()
}

fn shown_id(id: &str) -> String {
    id.split_once(' ').map_or(id, |(_, rest)| rest).to_string()
}

/// The command that writes the migration with what the person stated and what is to be added.
fn create_command(migration: &str, choices: &Choices, more_drops: &[String], rename: Option<(String, String)>) -> String {
    let drops: Vec<String> = choices.drop.iter().cloned().chain(more_drops.iter().cloned()).collect();
    let mut command = format!("reylo migrate create {migration}");
    if !drops.is_empty() {
        command.push_str(&format!(" --drop {}", drops.join(",")));
    }
    for (old, new) in choices.rename.iter().cloned().chain(rename) {
        command.push_str(&format!(" --rename {old}={new}"));
    }
    command
}

/// The problems with what `choices` states about the tables and columns a plan drops (`lost`, as
/// the plan's ids, and `shown`, as the file names them, one for one): each must be named in
/// `--drop`, and each name in `--drop` must name only what it drops.
fn drop_problems(lost: &[String], shown: &[String], choices: &Choices, migration: &str) -> Vec<Problem> {
    let display = |id: &String| lost.iter().position(|l| l == id).map(|k| shown_id(&shown[k])).unwrap_or_else(|| shown_id(id));
    let mut problems = Vec::new();
    let mut named: Vec<String> = Vec::new();
    // What --drop states that holds, which a suggested command keeps.
    let mut stated = Choices { drop: vec![], rename: choices.rename.clone() };
    for w in &choices.drop {
        match resolve(lost, w) {
            Ok(hits) if hits.is_empty() => {
                let columns: Vec<String> = lost.iter().filter(|l| l.starts_with("column ") && split_name(&shown_id(l)).last() == split_name(w).last() && split_name(w).len() == 1).map(&display).collect();
                problems.push(Problem::new(format!("--drop names {w}, which this migration doesn't drop")).note(if !columns.is_empty() {
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
            Err(could) => problems.push(Problem::new(format!("--drop names {w}, which could be any of {}", could.iter().map(&display).collect::<Vec<_>>().join(", "))).note("Name each with its table, and its schema where two share a name.")),
        }
    }
    let unstated: Vec<&String> = lost.iter().filter(|l| !named.contains(l)).collect();
    if !unstated.is_empty() {
        let list: Vec<String> = unstated.iter().map(|l| display(l)).collect();
        let mut problem = Problem::new(format!("This migration would drop {}, and {} data", list.join(", "), if list.len() == 1 { "its" } else { "their" }))
            .fix(if list.len() == 1 { "To drop it" } else { "To drop them" }, &[&create_command(migration, &stated, &list, None)]);
        if let Some(t) = unstated.iter().find(|l| l.starts_with("table ")) {
            problem = problem.fix("If a table was renamed instead", &[&create_command(migration, &stated, &[], Some((display(t), "<new name>".into())))]);
        }
        if let Some(c) = unstated.iter().find(|l| l.starts_with("column ")) {
            problem = problem.fix("If a column was renamed instead", &[&create_command(migration, &stated, &[], Some((display(c), "<new name>".into())))]);
        }
        problems.push(problem);
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
                problems.push(Problem::new(format!("--rename names {old}, which isn't a table or column this migration removes")).note("A column is named with its table, --rename users.full_name=name, and a table by its name, --rename users=accounts."));
                continue;
            }
            Err(could) => {
                problems.push(Problem::new(format!("--rename names {old}, which could be any of {}", could.iter().map(|c| shown_id(c)).collect::<Vec<_>>().join(", "))).note("Name it with its schema."));
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
            [] => problems.push(Problem::new(format!("--rename gives {old} the name {new}, which isn't a column this migration adds to its table"))),
            many => problems.push(Problem::new(format!("--rename gives {old} the name {new}, which could be any of {}", many.iter().map(|c| shown_id(c)).collect::<Vec<_>>().join(", "))).note("Quote the name as it is written: --rename 'users.n=\"Total\"'.")),
        }
    }
    if problems.is_empty() { Ok(out) } else { Err(problems) }
}

/// The new migration's text: the diff engine's plan from the migrations replayed by time to
/// `schema.sql`, run on the replay and compared with `schema.sql` before it is written. A plan that
/// does not reach it is written all the same, with what remains, for the person (or agent) to finish.
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
                    _ => problems.push(Problem::new(format!("--rename gives the table {old} the name {new}, which isn't a table this migration creates"))),
                },
                Err(could) => problems.push(Problem::new(format!("--rename names {old}, which could be any of {}", could.iter().map(|c| shown_id(c)).collect::<Vec<_>>().join(", "))).note("Name it with its schema.")),
                _ => column_renames.push((old.clone(), new.clone())),
            }
        }
        if !problems.is_empty() {
            return Ok(Err(problems));
        }
        let dropped_tables: std::collections::BTreeSet<String> = choices.drop.iter().filter_map(|w| resolve(&gone_tables, w).ok()).flatten().collect();
        let renamed_tables = crate::diff::table_renames(&current, &target, &stated_tables, &dropped_tables);
        if !renamed_tables.is_empty() {
            let indexes = if d == Dialect::Postgres { crate::diff::postgres::index_names(shadows.db(a)).await? } else { Default::default() };
            let to = Target { schema: target, indexes };
            let statements = rename_tables(shadows.db(c), &current, &to, &renamed_tables, log).await?;
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
        style(shadows.db(c), &mut plan, &target).await?;
        let problems = drop_problems(&lost, &plan.losses, &Choices { drop: columns.drop.clone(), rename: choices.rename.clone() }, migration);
        if !problems.is_empty() {
            return Ok(Err(problems));
        }
        if plan.statements.is_empty() && moves.is_empty() {
            log(&paint("dim", &format!("the migrations already make {}: the migration is empty, for a change of data", shown(&opts.schema))));
            return Ok(Ok((vec![String::new()], 0)));
        }
        // The plan runs on the replay as `apply` will run its files: one file in one transaction (on
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
/// records init.sql without running it, once a shadow shows the database already has its schema.
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
    sync_history(opts, log).await
}

// ---------------------------------------------------------------------------------------------
// The development commands: `apply --dev`, a database taken to its migrations; `sync`, one at its
// migrations taken to schema.sql.

/// Runs statements the diff engine wrote on the database, with `then` (statements of the history's own),
/// and keeps the result only if the database then reads as `expect`; `Ok(Err(lines))` says how it
/// differs. Where the database has transactional DDL, it is all one transaction, rolled back on a
/// difference, so nothing has changed; on MySQL, which commits each statement, it is checked after.
async fn run_to(db: &mut Db, plan: &[String], then: &[String], expect: &[String], scope: Scope) -> Result<Result<(), Vec<String>>, Error> {
    let d = db.dialect();
    let statements: Vec<String> = plan.iter().filter(|s| !s.starts_with("--")).map(|s| format!("{};", s.trim_end().trim_end_matches(';'))).chain(then.iter().cloned()).collect();
    let differs = |now: &[String]| describe_diff("this database", now, "the target", expect);
    if d == Dialect::Mysql || statements.iter().any(|s| runs_alone(s, d)) {
        for s in &statements {
            db.script(s).await?;
        }
        let now = catalog(db, scope).await?;
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
        let now = catalog(db, scope).await?;
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
async fn take_to(db: &mut Db, target: &Target, then: &[String], expect: &[String], scope: Scope) -> Result<Result<(Vec<String>, crate::diff::Plan), Vec<String>>, Error> {
    let d = db.dialect();
    let transactional = d != Dialect::Mysql;
    if d == Dialect::Sqlite {
        db.exec("pragma foreign_keys = off").await?;
    }
    if transactional {
        db.exec("begin").await?;
    }
    let result = async {
        let (renames, plan) = plan_to(db, target).await?;
        for s in plan.runnable().chain(then) {
            db.script(&format!("{};", s.trim_end().trim_end_matches(';'))).await?;
        }
        if d == Dialect::Sqlite && !db.rows("pragma foreign_key_check").await?.is_empty() {
            return Err(Error("rows would break a foreign key".into()));
        }
        let now = catalog(db, scope).await?;
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
    log(&paint("bold red", &format!("✗ {command} changes a development database, and CI is set")));
    log("  In CI, check the migrations, or apply them:");
    log(&format!("    {}", paint("cyan", "reylo migrate check")));
    log(&format!("    {}", paint("cyan", "reylo migrate apply")));
    1
}

/// `reylo migrate apply --dev`: takes a development database to its migrations, whatever its
/// history. The history is kept as far as it agrees with the migration files: where a migration it
/// applied was edited or is gone (another branch's), the database is rewound to that point by the
/// diff engine's plan from what those migrations made, losing what it drops. The migrations after it
/// then run for real, their data statements included; one with no data statements whose change the
/// database already has (`sync` made it) is recorded instead. Never in CI (`in_ci`, from the `CI`
/// variable).
pub async fn apply_dev(opts: &Options, in_ci: bool, log: Log<'_>) -> Result<i32, Error> {
    if in_ci {
        return Ok(refused_in_ci("reylo migrate apply --dev", log));
    }
    let (mut db, mut server, swept) = open(opts).await?;
    let result = apply_dev_on(&mut db, &mut server, swept, opts, log).await;
    db.close().await;
    server.close().await;
    result
}

/// The problems no database is needed to see: files that aren't migrations, a migration that can't
/// run atomically, and a text (a migration's, or schema.sql's) a shadow can't replay.
fn file_problems(opts: &Options, files: &[Migration], strays: &[String], schema_sql: Option<&str>, d: Dialect) -> Vec<Problem> {
    let mut problems = Vec::new();
    for s in strays {
        problems.push(Problem::new(format!("{s} isn't a migration")).note(format!("A file in {} is init.sql or <YYYYMMDDHHMMSS>_<name>.sql, the name in a-z, 0-9 and _.", shown(&opts.dir))));
    }
    for f in files.iter().filter(|f| f.name != INIT) {
        if let Some(why) = atomicity_problem(&f.sql, d) {
            problems.push(Problem::new(format!("{} can't run atomically", f.name)).note(why));
        }
    }
    let schema_name = opts.schema.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "schema.sql".into());
    for (name, sql) in files.iter().map(|f| (f.name.as_str(), f.sql.as_str())).chain(schema_sql.map(|s| (schema_name.as_str(), s))) {
        if let Some(why) = shadow_problem(sql, d) {
            problems.push(Problem::new(format!("{name} can't be replayed in a shadow database")).note(why));
        }
    }
    problems
}

async fn apply_dev_on(db: &mut Db, server: &mut Server, swept: usize, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    swept_note(swept, log);
    let _held = lock(db, opts).await?;
    let d = db.dialect();
    let scope = opts.scope();
    let (files, strays) = read_migrations(&opts.dir)?;
    let problems = file_problems(opts, &files, &strays, None, d);
    if !problems.is_empty() {
        print_problems(&problems, log);
        log("");
        log(&paint("dim", "Nothing was changed."));
        return Ok(1);
    }
    let history = read_history(db).await?;
    let left = leftovers(db, &history).await?;
    drop_leftovers(db, &left, log).await?;
    if history.iter().any(|r| !r.applied) {
        // A row marked started is a pending migration's: written again when it runs, or is recorded.
        db.exec(&format!("delete from {HISTORY_TABLE} where applied_at is null")).await?;
    }
    let applied: Vec<&HistoryRow> = history.iter().filter(|r| r.applied).collect();
    // The history as far as it agrees with the files: each row's file there, with the text it ran.
    let agree = applied.iter().take_while(|r| files.iter().any(|f| f.name == r.name && same_statements(&f.sql, &r.sql, d))).count();
    let live = catalog(db, scope).await?;
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain(history.iter().map(|r| r.sql.as_str())).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, scope, &texts).await;
    let result = dev_in_shadows(&mut shadows, db, opts, &files, &applied, agree, &live, log).await;
    shadows.finish().await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn dev_in_shadows(shadows: &mut Shadows<'_>, db: &mut Db, opts: &Options, files: &[Migration], applied: &[&HistoryRow], agree: usize, live: &[String], log: Log<'_>) -> Result<i32, Error> {
    let d = db.dialect();
    let scope = opts.scope();
    let fail = |m: &str, error: String, log: Log<'_>| {
        print_problems(&[Problem::new(m.to_string()).note(error)], log);
        Ok(1)
    };
    let kept_of = |n: usize| -> Vec<Migration> { applied[..n].iter().filter_map(|r| files.iter().find(|f| f.name == r.name).cloned()).collect() };
    let mut keep = agree;
    // A migration older than one this database applied (main's, pulled in after it) runs out of
    // order only where apply would run it so: it and every migration it passes change no data, and
    // both orders end the same. Otherwise the database is rewound to before it.
    let kept = kept_of(keep);
    let newest = kept.iter().filter(|f| f.name != INIT).map(|f| f.time.clone()).max().unwrap_or_default();
    let late: Vec<&Migration> = files.iter().filter(|f| f.name != INIT && !kept.iter().any(|k| k.name == f.name) && f.time < newest).collect();
    if let Some(first) = late.iter().map(|f| f.time.clone()).min() {
        let passed: Vec<&Migration> = kept.iter().filter(|k| k.time > first).collect();
        let quiet = late.iter().copied().chain(passed.iter().copied()).all(|m| data_statements(&m.sql, d).is_empty());
        // A linear history runs in one order, whatever commutes.
        let mut commute = false;
        if quiet && !opts.linear {
            let ours = shadows.lane();
            shadows.push(ours, kept.iter().chain(files.iter().filter(|f| !kept.iter().any(|k| k.name == f.name))).map(Step::of));
            let by_name = shadows.lane();
            shadows.push(by_name, files.iter().map(Step::of));
            if let (Ok(Ok(a)), Ok(Ok(b))) = (shadows.lines(ours).await, shadows.lines(by_name).await) {
                commute = a == b;
            }
        }
        if !commute {
            keep = kept.iter().take_while(|k| k.name == INIT || k.time < first).count();
        }
    }
    let kept = &kept_of(keep)[..];
    let k_lane = shadows.lane();
    shadows.push(k_lane, kept.iter().map(Step::of));
    let at_kept = match shadows.lines(k_lane).await? {
        Ok(lines) => lines,
        Err((m, e)) => return fail(&format!("{m} fails when replayed in a shadow database"), e, log),
    };
    let mut pending: Vec<Migration> = files.iter().filter(|f| !kept.iter().any(|k| k.name == f.name)).cloned().collect();
    let rewind_rows: Vec<&&HistoryRow> = applied[keep..].iter().collect();
    let mut records: Vec<Migration> = Vec::new();
    // The database's catalog as each step leaves it.
    let mut base: Vec<String> = live.to_vec();
    let refused = |what: &str, lines: Vec<String>, log: Log<'_>| {
        let problem = Problem::new(what.to_string()).lines(lines);
        let problem = if d == Dialect::Mysql {
            problem.fix("MySQL commits each statement, so it is part of the way there. To rebuild it from the migrations, with no rows but the seed's", &["reylo migrate apply --reset"])
        } else {
            problem.note("That step changed nothing.").fix("To rebuild it from the migrations, with no rows but the seed's", &["reylo migrate apply --reset"])
        };
        print_problems(&[problem], log);
        Ok(1)
    };
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
                let (renames, plan) = plan_to(shadows.db(h_lane), &k_target).await?;
                let (only_h, only_k) = difference(&at_applied, &at_kept);
                let expect: Vec<String> = live.iter().filter(|l| !only_h.contains(l)).cloned().chain(only_k).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                let statements: Vec<String> = renames.iter().cloned().chain(plan.runnable().cloned()).collect();
                if let Ok(Ok(())) = run_to(db, &statements, &forget, &expect, scope).await {
                    base = expect;
                    undone = Some(plan);
                }
            }
        }
        // Where that doesn't reach it, the database is taken to the history kept itself, which
        // undoes the changes no migration made too.
        let plan = match undone {
            Some(plan) => plan,
            None => match take_to(db, &k_target, &forget, &at_kept, scope).await {
                Err(e) => return fail("Rewinding this database failed", e.0, log),
                Ok(Err(lines)) => return refused("Rewinding this database wouldn't reach what its migrations make", lines, log),
                Ok(Ok((_, plan))) => {
                    base = at_kept.clone();
                    plan
                }
            },
        };
        for r in &rewind_rows {
            let data = data_statements(&r.sql, d);
            if !data.is_empty() {
                log(&format!("{} {} changes data ({}): rewinding undoes its schema, not the rows it wrote. For a database with no rows but the seed's:", paint("yellow", "!"), r.name, data.join(", ")));
                log(&format!("    {}", paint("cyan", "reylo migrate apply --reset")));
            }
            let why = match files.iter().find(|f| f.name == r.name) {
                Some(f) if !same_statements(&f.sql, &r.sql, d) => "it changed after this database applied it",
                Some(_) => "a migration before it came in later, and the two can't run in this order",
                None => "it isn't in the migrations",
            };
            log(&format!("{} Rewound {}: {why}", paint("yellow", "↺"), r.name));
        }
        if !plan.losses.is_empty() {
            log(&format!("  {}", losses_counted(&plan, &counts).replacen(", dropping", "Dropped", 1)));
        }
    }
    // Changes no migration made (an earlier sync's, from schema.sql, or made by hand) are kept,
    // unless a migration still to run cannot run on top of them.
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
                let (added, removed) = difference(&lines, &at_kept);
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
                if let Ok(Ok(())) = run_to(shadows.db(lane), &statements, &[], &base, scope).await {
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
            match take_to(db, &target, &[], &expect, scope).await {
                Err(e) => return fail("Undoing changes no migration made failed", e.0, log),
                Ok(Err(lines)) => return refused("Undoing changes no migration made wouldn't reach what the migrations make", lines, log),
                Ok(Ok((_, plan))) => log(&format!("{} Undid changes no migration made, which {} can't run on{}", paint("yellow", "↺"), pending[0].name, losses_counted(&plan, &counts))),
            }
        }
    }
    if !records.is_empty() || !pending.is_empty() {
        ensure_history(db).await?;
    }
    for m in &records {
        insert_row(db, m, true).await?;
        log(&format!("{} Recorded {} without running it: this database already had its change", paint("yellow", "!"), m.name));
    }
    for m in &pending {
        let started = std::time::Instant::now();
        if let Err(e) = run(db, m, true).await {
            return fail(&format!("{} failed, and nothing of it was applied", m.name), e.0, log);
        }
        log(&format!("{} Applied {} {}", paint("green", "✓"), m.name, paint("dim", &format!("· {} ms", started.elapsed().as_millis()))));
    }
    if rewind_rows.is_empty() && records.is_empty() && pending.is_empty() && base == at_kept {
        log(&format!("{} Up to date", paint("green", "✓")));
    }
    Ok(0)
}

/// `reylo migrate sync`: takes a development database that is at its migrations (`apply --dev`
/// brings it there) to schema.sql: what schema.sql has that no migration makes is made, unrecorded,
/// for `create` to write next. Refuses a database with a migration it hasn't applied, or one whose
/// history disagrees with the files. Never in CI.
pub async fn sync(opts: &Options, in_ci: bool, log: Log<'_>) -> Result<i32, Error> {
    if in_ci {
        return Ok(refused_in_ci("reylo migrate sync", log));
    }
    let (mut db, mut server, swept) = open(opts).await?;
    let result = sync_on(&mut db, &mut server, swept, opts, log).await;
    db.close().await;
    server.close().await;
    result
}

async fn sync_on(db: &mut Db, server: &mut Server, swept: usize, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    swept_note(swept, log);
    let _held = lock(db, opts).await?;
    let d = db.dialect();
    let scope = opts.scope();
    let (files, strays) = read_migrations(&opts.dir)?;
    let schema_sql = std::fs::read_to_string(&opts.schema).ok();
    let mut problems = file_problems(opts, &files, &strays, schema_sql.as_deref(), d);
    if schema_sql.is_none() {
        problems.push(Problem::new(format!("{} doesn't exist", shown(&opts.schema))).note("sync takes the database to it."));
    }
    // The database is at its migrations, as apply --dev leaves it: every migration applied, each
    // with the text on disk, and nothing else.
    let history = read_history(db).await?;
    let applied: Vec<&HistoryRow> = history.iter().filter(|r| r.applied).collect();
    let mut behind = Vec::new();
    for r in &applied {
        match files.iter().find(|f| f.name == r.name) {
            None => behind.push(format!("  {} isn't in the migrations", r.name)),
            Some(f) if !same_statements(&f.sql, &r.sql, d) => behind.push(format!("  {} changed after this database applied it", r.name)),
            Some(_) => {}
        }
    }
    for f in files.iter().filter(|f| !applied.iter().any(|r| r.name == f.name)) {
        behind.push(format!("  {} isn't applied", f.name));
    }
    if !behind.is_empty() {
        problems.push(Problem::new("This database isn't at its migrations").lines(behind).fix("Bring it to them first", &["reylo migrate apply --dev"]));
    }
    if !problems.is_empty() {
        print_problems(&problems, log);
        log("");
        log(&paint("dim", "Nothing was changed."));
        return Ok(1);
    }
    let schema_sql = schema_sql.unwrap_or_default();
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain([schema_sql.as_str()]).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, scope, &texts).await;
    let in_order: Vec<Migration> = applied.iter().filter_map(|r| files.iter().find(|f| f.name == r.name).cloned()).collect();
    let result = sync_in_shadows(&mut shadows, db, opts, &in_order, &schema_sql, log).await;
    shadows.finish().await;
    result
}

/// What schema.sql has that no migration makes, made on the database, unrecorded; `applied` is the
/// migrations in the order the database ran them.
async fn sync_in_shadows(shadows: &mut Shadows<'_>, db: &mut Db, opts: &Options, applied: &[Migration], schema_sql: &str, log: Log<'_>) -> Result<i32, Error> {
    let d = db.dialect();
    let scope = opts.scope();
    let fail = |m: &str, error: String, log: Log<'_>| {
        print_problems(&[Problem::new(m.to_string()).note(error)], log);
        Ok(1)
    };
    let s_lane = shadows.lane();
    shadows.push(s_lane, [Step::script("schema.sql", schema_sql)]);
    match shadows.open(s_lane).await? {
        Err((_, e)) => return fail("schema.sql fails on an empty database", e, log),
        Ok(()) => shadows.taint(s_lane),
    }
    let wanted = catalog(shadows.db(s_lane), scope).await?;
    let now = catalog(db, scope).await?;
    let pushed = if now != wanted {
        let target = read_target(shadows.db(s_lane), Some(schema_sql)).await?;
        let counts = table_rows(db).await?;
        let (renames, plan) = match take_to(db, &target, &[], &wanted, scope).await {
            Err(e) => return fail("Taking this database to schema.sql failed", e.0, log),
            Ok(Err(lines)) => {
                let problem = Problem::new("Reylo can't take this database the rest of the way to schema.sql").lines(lines);
                let problem = if d == Dialect::Mysql { problem } else { problem.note("Nothing was changed.") };
                print_problems(&[problem.fix("Write the change as a migration, finishing by hand what create leaves out", &["reylo migrate create <name>"])], log);
                return Ok(1);
            }
            Ok(Ok(done)) => done,
        };
        log(&format!("{} Brought this database to schema.sql, unrecorded{}:", paint("green", "✓"), losses_counted(&plan, &counts)));
        log("");
        for line in renames.iter().map(|s| if s.starts_with("--") { s.clone() } else { format!("{s};") }).chain(plan.sql().trim_end().lines().filter(|l| !l.starts_with("-- loses data")).map(str::to_string)) {
            log(&format!("    {line}"));
        }
        log("");
        true
    } else {
        false
    };
    // Whether the migrations themselves make schema.sql, or this database holds more than they do.
    let m_lane = shadows.lane();
    shadows.push(m_lane, applied.iter().map(Step::of));
    let made = match shadows.lines(m_lane).await? {
        Ok(lines) => lines == wanted,
        Err(_) => false,
    };
    if made && !pushed {
        log(&format!("{} This database matches schema.sql and its migrations", paint("green", "✓")));
    } else if !made {
        if !pushed {
            log(&format!("{} This database matches schema.sql", paint("green", "✓")));
        }
        log("  schema.sql has changes not reflected in any migration file. Write them as a migration:");
        log(&format!("    {}", paint("cyan", "reylo migrate create <name>")));
    }
    Ok(0)
}

/// The rows of each of the database's tables, by table id.
async fn table_rows(db: &mut Db) -> Result<std::collections::BTreeMap<String, u64>, Error> {
    let schema = crate::diff::read(db).await?;
    let mut out = std::collections::BTreeMap::new();
    for o in schema.objects.values().filter(|o| o.kind == Some(crate::diff::Kind::Table) && !o.extra.iter().any(|l| l.starts_with("shape foreign"))) {
        let Some(name) = o.drop.first().and_then(|d| d.strip_prefix("drop table ")) else { continue };
        if let Ok(rows) = db.rows(&format!("select count(*) as n from {name}")).await {
            if let Some(n) = rows.first().and_then(|r| r.get("n")).and_then(|n| n.parse().ok()) {
                out.insert(o.id.clone(), n);
            }
        }
    }
    Ok(out)
}

/// What a plan drops, each table with how many rows it held where `counts` holds them, as the end of
/// a sentence (", dropping tags (3 rows) and users.bio"), or nothing.
fn losses_counted(plan: &crate::diff::Plan, counts: &std::collections::BTreeMap<String, u64>) -> String {
    if plan.losses.is_empty() {
        return String::new();
    }
    let what: Vec<String> = plan
        .losses
        .iter()
        .map(|l| {
            let name = l.trim_start_matches("column ").trim_start_matches("table ");
            let rows = if l.starts_with("table ") { counts.iter().find(|(id, _)| id.ends_with(&format!(".{name}")) || id.ends_with(&format!(" {name}"))).map(|(_, n)| *n) } else { None };
            match rows {
                Some(0) => format!("{name} (no rows)"),
                Some(1) => format!("{name} (1 row)"),
                Some(n) => format!("{name} ({n} rows)"),
                None => name.to_string(),
            }
        })
        .collect();
    let list = match what.as_slice() {
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        [] => String::new(),
    };
    paint("yellow", &format!(", dropping {list}"))
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
