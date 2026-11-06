//! The commands of `reylo migration`, `reylo migrate`, `reylo schema` and `reylo db`, and `reylo run`. Each takes the project's settings and a log it
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
        Dialect::Postgres => format!("create table if not exists {HISTORY_TABLE} (name text primary key, sql text not null, applied_at timestamptz, fingerprint text)"),
        Dialect::Mysql => format!("create table if not exists {HISTORY_TABLE} (name varchar(255) primary key, `sql` longtext not null, applied_at datetime(6) null, fingerprint text null) engine=InnoDB"),
        Dialect::Sqlite => format!("create table if not exists {HISTORY_TABLE} (name text primary key, sql text not null, applied_at text, fingerprint text)"),
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
/// database whose fingerprint equals the one its last run stored has not been changed since, its
/// comments aside.
async fn fingerprint(db: &mut Db) -> Result<String, Error> {
    let lines = catalog(db).await?;
    fingerprint_of(db, &lines).await
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
    for line in crate::catalog::without_children(&crate::catalog::without_comments(lines)) {
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
/// file of the project's (in its cache, which only these runs open), held while the answer lives.
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
            let dir = opts.cache.join("migrate").join("locks");
            std::fs::create_dir_all(&dir)?;
            let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(crate::cache::hex(&digest.finish()[..8])))?;
            file.lock()?;
            Ok(Some(file))
        }
    }
}

async fn insert_row(db: &mut Db, m: &Migration, applied: bool) -> Result<(), Error> {
    let d = db.dialect();
    let sql = format!("insert into {HISTORY_TABLE} (name, {}, applied_at) values ({}, {}, {})", sql_col(d), db.param(1), db.param(2), if applied { now_sql(d) } else { "null".to_string() });
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
    /// Whether it is the lock's drafts, which some commands pass over.
    drafts: bool,
}

impl Problem {
    fn new(title: impl Into<String>) -> Problem {
        Problem { title: title.into(), lines: vec![], fixed: false, drafts: false }
    }

    fn is_drafts(&self) -> bool {
        self.drafts
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
    /// The invalid indexes such statements left, which `prod` drops before running them again.
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
fn read_lock(opts: &Options, files: &[Migration]) -> Result<lock::Lock, Problem> {
    let path = shown(&opts.lock);
    match lock::read(&opts.lock) {
        lock::State::Lock(l) => Ok(l),
        lock::State::Missing if files.is_empty() => Ok(lock::Lock::default()),
        lock::State::Missing => Err(Problem::new(format!("{path} is missing")).fix("On a development database, run the migrations, which writes it", &["reylo migrate dev"]).fix("Or write it from what a database applied", &["reylo migration history pull"])),
        lock::State::Conflicted => Err(Problem::new(format!("{path} has a merge conflict: another branch added migrations too"))
            .note("Keep main's lines, and leave out this branch's (git checkout --theirs while merging main in, --ours while rebasing onto it)")
            .fix("Then run this branch's migrations after them, which adds them back", &["reylo migrate dev"])),
        lock::State::Invalid(e) => Err(Problem::new(format!("{path} doesn't read as a lock file")).note(e).fix("Write it again from what a database applied", &["reylo migration history pull"])),
    }
}

/// The problems of the lock against the migration files, for a command that runs
/// what it lists as it is (prod, its dry run): an entry with no file, a file edited after it was locked,
/// lines merged out of their order, and drafts.
fn lock_problems(opts: &Options, files: &[Migration]) -> Vec<Problem> {
    let path = shown(&opts.lock);
    let l = match read_lock(opts, files) {
        Ok(l) => l,
        Err(p) => return vec![p],
    };
    let mut problems = Vec::new();
    for (e, link) in l.entries.iter().zip(links(&l, files)) {
        match link {
            Link::Ok => {}
            Link::Missing => problems.push(Problem::new(format!("{path} lists {}, which isn't in {}", e.name, shown(&opts.dir))).fix("Write it back from a database that ran it", &["reylo migration history pull"])),
            Link::Edited => problems.push(Problem::new(format!("{} changed after it was locked", e.name)).note("A database may have run it as it was: write the change as a new migration.").fix("Take back the text a database ran", &["reylo migration history pull"])),
            Link::Moved { after } => problems.push(
                Problem::new(format!("{path}'s lines were merged out of their order: {} was locked {}, not where it is", e.name, after.map_or("first".to_string(), |a| format!("right after {a}"))))
                    .note("Keep main's lines in main's order, and leave out this branch's.")
                    .fix("Then run this branch's migrations after them, which adds them back", &["reylo migrate dev"]),
            ),
        }
    }
    let drafts: Vec<&str> = files.iter().filter(|f| !l.entries.iter().any(|e| e.name == f.name)).map(|f| f.name.as_str()).collect();
    if !drafts.is_empty() {
        let mut problem = Problem::new(format!("{} {} in {path}: {}", drafts.join(", "), if drafts.len() == 1 { "isn't" } else { "aren't" }, if drafts.len() == 1 { "it's a draft" } else { "they're drafts" }))
            .fix(format!("Run {} in development, which adds {} to the lock", if drafts.len() == 1 { "it" } else { "them" }, if drafts.len() == 1 { "it" } else { "them" }), &["reylo migrate dev"]);
        problem.drafts = true;
        problems.push(problem);
    }
    problems
}

/// The order the migrations run in: the lock's, then its drafts by name.
fn run_order(opts: &Options, files: &[Migration]) -> Vec<Migration> {
    let Ok(l) = read_lock(opts, files) else { return files.to_vec() };
    let mut out: Vec<Migration> = l.entries.iter().filter_map(|e| files.iter().find(|f| f.name == e.name).cloned()).collect();
    out.extend(files.iter().filter(|f| !l.entries.iter().any(|e| e.name == f.name)).cloned());
    out
}

/// Checks the target against the migration files; changes nothing. With a `server` (`reylo migrate
/// check`) it replays them in shadow databases, against the live schema and schema.sql; with none
/// (`reylo migrate prod`) it replays nothing, and trusts the fingerprint Reylo's last run stored: a
/// database that no longer has it is refused, and what the fingerprint can't speak to is `--dry-run`'s.
pub async fn check(db: &mut Db, server: Option<&mut Server>, opts: &Options) -> Result<Plan, Error> {
    let d = db.dialect();
    let (files, strays) = read_migrations(&opts.dir)?;
    let mut problems = Vec::new();
    for s in &strays {
        problems.push(Problem::new(format!("{s} isn't a migration")).note(format!("A file in {} is init.sql or <YYYYMMDDHHMMSS>_<name>.sql, the name in a-z, 0-9 and _.", shown(&opts.dir))));
    }
    // What schema.sql declares is held against the migrations in development and CI, not at deploy.
    let schema_sql = if server.is_some() {
        let text = std::fs::read_to_string(&opts.schema).ok();
        if text.is_none() {
            problems.push(Problem::new(format!("{} doesn't exist", shown(&opts.schema))).note("The declared schema is required: migrations are written from it, and checked against it."));
        }
        text
    } else {
        None
    };
    problems.extend(lock_problems(opts, &files));
    let files = run_order(opts, &files);
    let history = read_history(db).await?;
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
    // Every database runs the lock's order, and nothing else.
    if problems.is_empty() {
        if let Some(k) = applied.iter().zip(&files).position(|(r, f)| r.name != f.name) {
            problems.push(Problem::new(format!("This database applied {} where {} lists {}", applied[k].name, shown(&opts.lock), files[k].name)).note("Every database runs the migrations in the lock's order. Where one ran them otherwise, the lock was merged keeping a branch's lines: keep main's.").fix("On a development database, take it to the lock's order", &["reylo migrate dev"]));
        }
    }
    // Nothing is recorded without running but what a cut-off run of Reylo's own left (below).
    let records: Vec<Migration> = Vec::new();
    let pending: Vec<Migration> = files.iter().filter(|f| !applied_files.iter().any(|a| a.name == f.name) && !records.contains(f)).cloned().collect();
    // init.sql, which `init` writes, holds the whole schema and runs only on an empty database.
    for p in pending.iter().filter(|f| f.name != INIT) {
        if let Some(why) = atomicity_problem(&p.sql, d) {
            problems.push(Problem::new(format!("{} can't run atomically", p.name)).note(why));
        }
    }
    if server.is_some() {
        let schema_name = opts.schema.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "schema.sql".into());
        for (name, sql) in files.iter().map(|f| (f.name.as_str(), f.sql.as_str())).chain(schema_sql.as_deref().map(|s| (schema_name.as_str(), s))) {
            if let Some(why) = shadow_problem(sql, d) {
                problems.push(Problem::new(format!("{name} can't be replayed in a shadow database")).note(why));
            }
        }
    }
    if !problems.is_empty() {
        return Ok(Plan { problems, ..Plan::default() });
    }

    // An index a cut-off `create index concurrently` left invalid is dropped before its migration
    // runs again, so it is compared as gone.
    let leftovers = leftovers(db, &history).await?;
    let live: Vec<String> = catalog(db).await?.into_iter().filter(|l| !leftovers.iter().any(|x| l.starts_with(&x.line))).collect();
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
    let Some(server) = server else {
        // No replay: the fingerprint says whether the database is as Reylo's last run left it.
        if unchanged == Some(false) {
            let problem = Problem::new("This database changed since Reylo last ran on it").fix("See how", &["reylo migrate prod --dry-run"]);
            return Ok(Plan { problems: vec![problem], ..Plan::default() });
        }
        // A first run is on an empty database: there is no history to say what else this one holds.
        if history.is_empty() && holds_a_schema(&live) {
            let problem = Problem::new("This database has a schema, and no migration history")
                .fix("Where the migrations describe it, record them, running nothing", &["reylo migration history push"])
                .fix("Or start the project's migrations from it", &["reylo migration init"]);
            return Ok(Plan { problems: vec![problem], ..Plan::default() });
        }
        return Ok(Plan { pending, unchanged, leftovers, ..Plan::default() });
    };
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain(history.iter().map(|r| r.sql.as_str())).chain(schema_sql.as_deref()).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &texts).await;
    // A lone statement cut off before its row was finished did run, or did not: the database says
    // which, and one that landed is recorded, as the row it started says it ran.
    let started: Vec<String> = history.iter().filter(|r| !r.applied).map(|r| r.name.clone()).collect();
    let mut run = Run { live: &live, schema_sql: schema_sql.as_deref(), applied_files: &applied_files, records, pending, started, landed: vec![], d };
    let result = check_in_shadows(&mut shadows, db, &mut run).await;
    shadows.finish().await;
    let problems = result?;
    Ok(Plan { problems, records: run.records, pending: run.pending, unchanged, landed: run.landed, leftovers, ..Plan::default() })
}

/// What a check holds the shadows against.
struct Run<'a> {
    live: &'a [String],
    schema_sql: Option<&'a str>,
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
        Err((m, error)) => return Ok(vec![Problem::new(format!("{m} fails when replayed in a shadow database")).note(error)]),
    };
    if !crate::catalog::same_schema(run.live, &checkpoint) {
        return drift(shadows, target, run, &checkpoint).await;
    }
    shadows.push(b_lane, run.pending.iter().map(Step::of));
    let b = match shadows.lines(b_lane).await? {
        Ok(lines) => lines,
        Err((m, error)) => return Ok(vec![Problem::new(format!("{m} fails on this database's history")).note(error)]),
    };

    let mut problems = Vec::new();
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
            (false, true) => Problem::new("schema.sql has changes not reflected in any migration file").sql(&lane_plan(shadows, b_lane, a_lane, Some(schema_sql)).await.unwrap_or_default()).fix("Write them as a migration", &["reylo migration create <name>"]),
            (true, false) => Problem::new("The migrations make changes schema.sql doesn't declare").sql(&lane_plan(shadows, a_lane, b_lane, None).await.unwrap_or_default()).note("Add them to schema.sql, or take them out of the migration."),
            _ => Problem::new("schema.sql and the migrations disagree")
                .sql(&lane_plan(shadows, b_lane, a_lane, Some(schema_sql)).await.unwrap_or_default())
                .note("Change schema.sql or the migrations so that they agree; what schema.sql has and no migration makes is written as a migration by:")
                .fix("Write the migration", &["reylo migration create <name>"]),
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
    let checkpoint = crate::catalog::without_comments(checkpoint);
    let live = crate::catalog::without_comments(&crate::catalog::without_extra_children(run.live, &checkpoint));
    let (only_history, only_live) = difference(&checkpoint, &live);
    if !run.records.is_empty() {
        let names = run.records.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ");
        let without = shadows.lane();
        shadows.push(without, run.applied_files.iter().map(Step::of));
        if let Ok(Ok(lines)) = shadows.lines(without).await {
            if crate::catalog::without_comments(&lines) == live {
                return Ok(vec![Problem::new(format!("This database doesn't have what {names} make, so they can't be recorded without running")).fix("Run them instead", &["reylo migrate prod"])]);
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
        problem = problem.fix(format!("{} {} exactly that change. Record the history the database has, running nothing", hint.join(" and "), if hint.len() == 1 { "makes" } else { "make" }), &["reylo migration history push"]);
    } else if !only_live.is_empty() {
        // A new migration is a draft until development locks it; then this database's history can
        // be recorded with it.
        problem = problem
            .fix("To keep it, add it to schema.sql, write the migration and lock it", &["reylo migration create <name>", "reylo migrate dev"])
            .fix("Then, with the lock committed, record it here, running nothing", &["reylo migration history push"]);
    }
    problem = problem.fix("To start over from the migrations, drop this database and create it again, then", &["reylo migrate dev"]);
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
            log(&paint("dim", &self.heading));
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
        Problem::new(format!("{} doesn't exist", shown(&opts.dir))).note(format!("This database ran {ran} migration{} from it: check `dir` in .reylo.yaml.", if ran == 1 { "" } else { "s" }))
    })
}

/// A migration a database ran whose file is gone: the text it ran is the way back.
fn missing_problem(name: &str, ran: &str, dir: &str) -> Problem {
    Problem::new(format!("This database applied {name}, which isn't in {dir}"))
        .sql(ran)
        .fix("Write it back as it ran", &["reylo migration history pull"])
        .fix("On a development database, undo it instead", &["reylo migrate dev"])
}

/// A migration edited after a database ran it.
fn edited_problem(name: &str, ran: &str, now: &str) -> Problem {
    Problem::new(format!("{name} changed after this database applied it"))
        .lines(std::iter::once(String::new()).chain(line_diff(ran, now)).chain([String::new()]))
        .note("Write the change as a new migration.")
        .fix("Take back the text this database ran", &["reylo migration history pull"])
        .fix("On a development database, run it again as it now reads", &["reylo migrate dev"])
}

/// Whether a database's catalog lines hold more than an empty database's do.
fn holds_a_schema(lines: &[String]) -> bool {
    const EMPTY: [&str; 4] = ["schema ", "grant schema ", "comment schema ", "extension "];
    lines.iter().any(|l| !EMPTY.iter().any(|kind| l.starts_with(kind)))
}

/// Whether a migration that fails can leave part of what it did: on MySQL, whose statements commit as
/// they go, and a statement that runs outside a transaction.
fn may_leave_part(sql: &str, d: Dialect) -> bool {
    d == Dialect::Mysql || runs_alone(sql, d)
}

/// `reylo migrate prod`: runs what is pending on a database that is as Reylo's last run left it, and
/// refuses one that isn't, running nothing. It builds no shadow database: `--dry-run` is what replays.
pub async fn prod(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let result = prod_on(&mut db, opts, log).await;
    db.close().await;
    result
}

fn swept_note(swept: usize, log: Log<'_>) {
    if swept > 0 {
        log(&paint("dim", &format!("dropped {swept} shadow database{} a run left behind", if swept == 1 { "" } else { "s" })));
    }
}

async fn prod_on(db: &mut Db, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let _held = lock(db, opts).await?;
    let plan = check(db, None, opts).await?;
    if !plan.ok() {
        print_problems(&plan.problems, log);
        log("");
        log(&paint("dim", "Nothing was applied."));
        return Ok(1);
    }
    let history = read_history(db).await?;
    if plan.unchanged.is_none() && history.iter().any(|r| r.applied) {
        log(&paint("dim", "Reylo holds no fingerprint of this database to compare (after a new version of Reylo, or a new major version of the server): reylo migrate prod --dry-run compares it with its migrations"));
    }
    if plan.pending.is_empty() {
        // A fingerprint that can't be compared is stored again, for the next run to compare.
        if plan.unchanged.is_none() {
            if let Some(last) = history.iter().rfind(|r| r.applied) {
                ensure_history(db).await?;
                let value = fingerprint(db).await?;
                set_fingerprint(db, &last.name, &value).await?;
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
    for m in &plan.pending {
        let at = std::time::Instant::now();
        match run(db, m, true).await {
            Ok(()) => {
                rows.row(log, &paint("green", "✓"), &m.name, &format!("applied {}", paint("dim", &format!("· {} ms", at.elapsed().as_millis()))));
                // On its own row as it finishes, so that a run cut off after it, or a migration
                // after it that fails and leaves part of itself, is seen by the next run.
                let value = fingerprint(db).await?;
                set_fingerprint(db, &m.name, &value).await?;
            }
            Err(e) => {
                rows.row(log, &paint("bold red", "✗"), &m.name, failed_row(&m.sql, d));
                log(&format!("      {e}"));
                if started.contains(&m.name.as_str()) {
                    log("      A run before this one was cut off here, so its change may be in place: reylo migrate prod --dry-run says, and reylo migration history push records it.");
                }
                code = 1;
                break;
            }
        }
    }
    Ok(code)
}

/// What a failed migration's row says: only a transaction can say that nothing of it was applied.
fn failed_row(sql: &str, d: Dialect) -> &'static str {
    if may_leave_part(sql, d) {
        "failed:"
    } else {
        "failed, and nothing of it was applied:"
    }
}

/// `reylo migrate prod --dry-run`: everything `prod` checks, the database against a replay of its
/// history, what is pending tried in a shadow, and the migrations against schema.sql; runs nothing,
/// and writes nothing to the database.
pub async fn dry_run(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    swept_note(swept, log);
    let result = check(&mut db, Some(&mut server), opts).await;
    db.close().await;
    server.close().await;
    let plan = result?;
    if !plan.ok() {
        print_problems(&plan.problems, log);
        return Ok(1);
    }
    log(&format!("{} The migrations reach {}", paint("green", "✓"), shown(&opts.schema)));
    log(&format!("{} This database matches its history", paint("green", "✓")));
    match plan.unchanged {
        Some(true) => log(&format!("{} Nothing has changed this database since Reylo last ran on it", paint("green", "✓"))),
        Some(false) => log(&format!("{} Its fingerprint isn't the one Reylo stored, so reylo migrate prod refuses it until reylo migration history push stores it again", paint("yellow", "!"))),
        None => {}
    }
    for x in &plan.leftovers {
        log(&format!("{} {} was cut off and left {} invalid: reylo migrate prod drops it, then runs {} again", paint("yellow", "!"), x.migration, x.index, x.migration));
    }
    for name in &plan.landed {
        log(&format!("{} {name} was cut off, and its change is in place: reylo migration history push records it", paint("yellow", "!")));
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

/// `reylo migration lint`: every check that needs no database. The files: their names, and with the
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
    if schema_sql.is_none() {
        problems.push(Problem::new(format!("{} doesn't exist", shown(&opts.schema))).note("The declared schema is required: migrations are written from it, and checked against it."));
    }
    let mut proven = None;
    problems.extend(lock_problems(opts, &files));
    if let (true, Some(sql), lock::State::Lock(l)) = (problems.is_empty(), &schema_sql, lock::read(&opts.lock)) {
        let last = l.entries.last().map(|e| e.sum.clone()).unwrap_or_default();
        match l.schema {
            Some(sum) if sum == lock::schema_sum(&last, sql) => proven = Some(true),
            Some(_) => problems.push(
                Problem::new("schema.sql has changes not reflected in any migration file")
                    .note(format!("It isn't the schema.sql the migrations reached when {} was last written.", shown(&opts.lock)))
                    .fix("Write them as a migration, then run it in development", &["reylo migration create <name>", "reylo migrate dev"]),
            ),
            None => proven = Some(false),
        }
    }
    if !problems.is_empty() {
        print_problems(&problems, log);
        return Ok(1);
    }
    log(&format!("{} {} lists every migration, in order, as written", paint("green", "✓"), shown(&opts.lock)));
    match proven {
        Some(true) => log(&format!("{} The migrations reach {}", paint("green", "✓"), shown(&opts.schema))),
        _ => log(&format!("{} {} isn't proven against the migrations without a database: reylo schema push proves it in development, and reylo migrate prod --dry-run against a database", paint("yellow", "!"), shown(&opts.schema))),
    }
    if Dialect::of_url(&opts.url).is_err() {
        log(&paint("dim", "With no database URL, whether each migration runs atomically wasn't checked."));
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
    let result = wipe_rows_on(&mut db).await;
    db.close().await;
    let tables = result?;
    log(&format!("{} Deleted every row of {} table{}; the schema and the history stay", paint("green", "✓"), tables, if tables == 1 { "" } else { "s" }));
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

/// `reylo migration history push`: rewrites the database's history to the migrations its schema
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
        print_problems(&problems, log);
        log("");
        log(&paint("dim", "Nothing was changed."));
        return Ok(1);
    }
    // The history is the lock's; its drafts are development's alone.
    let problems: Vec<Problem> = lock_problems(opts, &files).into_iter().filter(|p| !p.is_drafts()).collect();
    if !problems.is_empty() {
        print_problems(&problems, log);
        log("");
        log(&paint("dim", "Nothing was changed."));
        return Ok(1);
    }
    let l = read_lock(opts, &files).unwrap_or_default();
    let order: Vec<Migration> = l.entries.iter().filter_map(|e| files.iter().find(|f| f.name == e.name).cloned()).collect();
    let live = crate::catalog::without_comments(&catalog(db).await?);
    let texts: Vec<&str> = order.iter().map(|f| f.sql.as_str()).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &texts).await;
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
        print_problems(&[Problem::new(format!("This database's schema isn't what any run of the migrations makes; the closest is through {after}")).lines(describe_diff("the migrations", &lines, "this database", &live)).fix("Write what the database has into schema.sql, then a migration for it", &["reylo migration create <name>"])], log);
        log("");
        log(&paint("dim", "Nothing was changed."));
        return Ok(1);
    };
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
        let value = fingerprint(db).await?;
        set_fingerprint(db, &last.name, &value).await?;
    }
    for f in recorded {
        let ran = before.iter().any(|r| r.applied && r.name == f.name && same_statements(&r.sql, &f.sql, d));
        if !ran {
            let data = data_statements(&f.sql, d);
            if data.is_empty() {
                log(&format!("  + {} recorded, not run: already in place", f.name));
            } else {
                log(&format!("  + {} recorded, not run: {} {}", f.name, paint("yellow", "its data statements never ran here"), paint("dim", &format!("({})", data.join(", ")))));
            }
        }
    }
    for r in &before {
        if !recorded.iter().any(|f| f.name == r.name && same_statements(&r.sql, &f.sql, d)) {
            log(&format!("  - {} taken out of the history", r.name));
        }
    }
    let pending = &order[n..];
    log(&format!("{} The history lists the {} migration{} this database's schema has", paint("green", "✓"), recorded.len(), if recorded.len() == 1 { "" } else { "s" }));
    if !pending.is_empty() {
        log(&format!("  {} pending, which reylo migrate prod runs: {}", pending.len(), pending.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")));
    }
    Ok(0)
}

/// `reylo migration history pull`: writes each migration the database ran into its file, as it ran
/// it, from the history table, which keeps every applied migration's text: a file deleted is written
/// again, and one that reads differently to the lock is written over. A file that differs in comments
/// alone is left as it is, and so is a file the database hasn't run. The lock then lists what it ran,
/// in its order, before what the lock already listed that it hasn't run: a lock lost or merged wrong
/// is written again, and a deploy still to reach this database stays locked. Changes no database.
pub async fn history_pull(opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let history = read_history(&mut db).await;
    db.close().await;
    let history = history?;
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
    if before.as_ref().map(|l| &l.entries) != Some(&entries) {
        // What schema.sql was proven against went with the old chain's last line.
        let schema = before.filter(|l| l.entries.last() == entries.last()).and_then(|l| l.schema);
        lock::write(&opts.lock, &lock::Lock { schema, entries })?;
        log(&format!("{} Wrote {}", paint("green", "✓"), shown(&opts.lock)));
    }
    Ok(0)
}

/// `reylo migration create <name>`: a migration named with the current UTC time, refused while any
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
    // The migrations replay in the lock's order, then its drafts; a lock that can't be read is
    // resolved first.
    if let Err(problem) = read_lock(opts, &files) {
        print_problems(&[problem], log);
        return Ok(1);
    }
    let files = run_order(opts, &files);
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
        log(&format!("{} Wrote {}", paint("green", "✓"), shown(&file)));
    }
    Ok(code)
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
    let (head, gap) = (format!("reylo migration create {migration}"), if cfg!(windows) || flags.len() < 2 { " " } else { " \\\n      " });
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
fn guess_problem(guesses: &[Guess], drops: &[String], narrowed: &[(String, String)], outside: &[String], stated: &Choices, migration: &str) -> Problem {
    let one = guesses.len() == 1;
    let title = if one { "This looks like a rename, but it changes more than the name".to_string() } else { format!("These {} look like renames, but each changes more than its name", guesses.len()) };
    let (old_width, new_width) = (guesses.iter().map(|g| g.old.chars().count()).max().unwrap_or(0), guesses.iter().map(|g| g.new.chars().count()).max().unwrap_or(0));
    let mut problem = Problem::new(title).lines([String::new()]).lines(guesses.iter().map(|g| format!("    {:<old_width$} → {:<new_width$}  {}", g.old, g.new, g.change)));
    if !drops.is_empty() {
        problem = problem.lines([String::new()]).note(format!("It also drops {}, and {} data.", drops.join(", "), if drops.len() == 1 { "its" } else { "their" }));
    }
    for (name, why) in narrowed {
        problem = problem.note(format!("{name} {why}."));
    }
    let more: Vec<String> = drops.iter().cloned().chain(narrowed.iter().map(|(n, _)| n.clone())).collect();
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
    let unnamed: Vec<(&String, String, String)> = narrowed.iter().filter(|(id, _, _)| !named.contains(id)).map(|(id, shown, why)| (id, shown_id(shown), why.clone())).collect();
    if !guesses.is_empty() {
        let drops: Vec<String> = unstated.iter().map(|l| display(l)).collect();
        let lossy: Vec<(String, String)> = unnamed.iter().map(|(_, name, why)| (name.clone(), why.clone())).collect();
        let outside: Vec<String> = unstated.iter().copied().chain(unnamed.iter().map(|(id, _, _)| *id)).filter(|id| !owners(id)).map(&display).collect();
        problems.push(guess_problem(guesses, &drops, &lossy, &outside, &stated, migration));
        return problems;
    }
    if !unstated.is_empty() {
        let list: Vec<String> = unstated.iter().map(|l| display(l)).collect();
        let mut problem = Problem::new(format!("This migration would drop {}, and {} data", list.join(", "), if list.len() == 1 { "its" } else { "their" }))
            .fix(if list.len() == 1 { "To drop it" } else { "To drop them" }, &[&create_command(migration, &stated, &list, &[])]);
        // Where something comes as these go, any of them may be renamed: which became which is
        // for the person to say, so the command names no new name for them.
        if !added.is_empty() {
            let renames: Vec<(String, String)> = list.iter().map(|old| (old.clone(), "<new name>".to_string())).collect();
            let what = format!("If {} renamed instead, as {}", if list.len() == 1 { "it was" } else { "any were" }, added.join(", "));
            problem = problem.fix(what, &[&create_command(migration, &stated, &[], &renames)]);
        }
        problems.push(problem);
    }
    for (_, name, why) in &unnamed {
        problems.push(Problem::new(format!("{name} {why}")).fix("To accept that", &[&create_command(migration, &stated, std::slice::from_ref(name), &[])]));
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

/// The new migration's text: the diff engine's plan from the migrations replayed in order to
/// `schema.sql`, run on the replay and compared with `schema.sql` before it is written. A plan that
/// does not reach it is written all the same, with what remains, for the person (or agent) to finish.
#[allow(clippy::type_complexity)]
async fn draft(opts: &Options, files: &[Migration], schema_sql: &str, migration: &str, choices: &Choices, log: Log<'_>) -> Result<Result<(Vec<String>, i32), Vec<Problem>>, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let mut server = Server::open(&mut db, &opts.url, opts.shadow_url.as_deref()).await?;
    server.copy_default_privileges(!sets_default_privileges(opts));
    db.close().await;
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain([schema_sql]).collect();
    let mut shadows = Shadows::new(&mut server, &opts.url, &opts.root, &opts.cache, &texts).await;
    let result = async {
        let a = shadows.lane();
        shadows.push(a, [Step::script("schema.sql", schema_sql)]);
        if let Err((_, e)) = shadows.open(a).await? {
            return Err(Error(format!("schema.sql fails on an empty database: {e}")));
        }
        // The plan runs on the replay, which is then no longer one: the replay is kept
        // first, as the start of the check that follows.
        let c = shadows.lane();
        shadows.push(c, files.iter().map(Step::of));
        shadows.mark(c);
        if let Err((m, e)) = shadows.open(c).await? {
            return Err(Error(format!("{m} fails when every migration replays: {e}")));
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
        // A table renamed with its columns changing too is a guess only the person can settle.
        let mut guesses: Vec<Guess> = Vec::new();
        let mut guessed_tables: std::collections::BTreeSet<String> = Default::default();
        let mut assumed: Vec<(String, String)> = Vec::new();
        for (old, new, _) in renamed_tables.iter().filter(|(_, _, was_stated)| !was_stated) {
            match crate::diff::column_changes(&current, &target, old, new) {
                Some(change) => {
                    guesses.push(Guess { old: shown_name(old), new: shown_name(new), change, in_guessed_table: false });
                    guessed_tables.insert(new.clone());
                }
                None => assumed.push((shown_name(old), shown_name(new))),
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
                None => assumed.push((old, new.clone())),
            }
        }
        // What the plan adds, the names a person might mean by a rename of what it drops.
        let added: Vec<String> = target.objects.iter().filter(|(id, o)| matches!(o.kind, Some(crate::diff::Kind::Column | crate::diff::Kind::Table)) && !current.objects.contains_key(*id) && !(o.kind == Some(crate::diff::Kind::Column) && o.owner.as_ref().is_some_and(|t| !current.objects.contains_key(t)))).map(|(id, _)| shown_name(id)).collect();
        let problems = drop_problems(&lost, &plan.losses, &narrowed, &guesses, &owned_by_guess, &added, &Choices { drop: columns.drop.clone(), rename: choices.rename.clone() }, migration);
        if !problems.is_empty() {
            return Ok(Err(problems));
        }
        if plan.statements.is_empty() && moves.is_empty() {
            log(&paint("dim", &format!("the migrations already make {}: the migration is empty, for a change of data", shown(&opts.schema))));
            return Ok(Ok((vec![String::new()], 0)));
        }
        // The plan runs on the replay as `prod` will run its files: one file in one transaction (on
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
        let reached = catalog(shadows.db(c)).await?;
        plan.statements.splice(0..0, moves);
        let mut texts = plan_files(&plan, d);
        for loss in &plan.losses {
            log(&format!("{} Drops {}, and its data", paint("yellow", "!"), loss.trim_start_matches("column ").trim_start_matches("table ")));
        }
        for (column, why) in &plan.narrowings {
            log(&format!("{} {} {why}", paint("yellow", "!"), column.trim_start_matches("column ")));
        }
        if !assumed.is_empty() {
            let one = assumed.len() == 1;
            let width = assumed.iter().map(|(old, _)| old.chars().count()).max().unwrap_or(0);
            log(&format!("{} Renames, keeping rows:", paint("yellow", "!")));
            for (old, new) in &assumed {
                log(&format!("    {old:<width$} → {new}"));
            }
            let names: Vec<&str> = assumed.iter().map(|(old, _)| old.as_str()).collect();
            log(&format!("  {} two, state it: --drop {}", if one { "If they are" } else { "Where any are" }, names.join(",")));
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
            // version is looked for first (Homebrew's keg, and the directory Debian's and Ubuntu's
            // packages install it in, whose PATH wrapper picks a local cluster's version), then the
            // PATH's.
            let version: i64 = db.rows("select current_setting('server_version_num') as v").await?.first().and_then(|r| r.first()?.parse().ok()).unwrap_or(0);
            let major = version / 10000;
            let version_of = |p: &str| -> i64 {
                let out = std::process::Command::new(p).arg("--version").output().ok();
                let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
                text.split_whitespace().find_map(|w| w.split('.').next()?.parse().ok()).unwrap_or(0)
            };
            let candidates = [format!("/opt/homebrew/opt/postgresql@{major}/bin/pg_dump"), format!("/usr/lib/postgresql/{major}/bin/pg_dump"), "pg_dump".to_string()];
            let Some(pg_dump) = candidates.iter().find(|p| version_of(p) >= major) else {
                return Err(Error(format!("reading this schema needs pg_dump {major} or newer (the server is Postgres {major})")));
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

/// `reylo migration init`: writes `schema.sql` and `migrations/init.sql` from the database, then
/// records init.sql without running it, once a shadow shows the database already has its schema.
pub async fn init(opts: &Options, schemas: &[String], log: Log<'_>) -> Result<i32, Error> {
    if opts.schema.exists() {
        return Err(Error(format!("{} exists already: init writes it from the database", shown(&opts.schema))));
    }
    if !read_migrations(&opts.dir)?.0.is_empty() {
        return Err(Error(format!("{} holds migrations already: init starts a project's history", shown(&opts.dir))));
    }
    // An empty database has no history to start: the first migration is `create`'s.
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let lines = catalog(&mut db).await;
    db.close().await;
    if !holds_a_schema(&lines?) {
        if let Some(home) = opts.schema.parent() {
            std::fs::create_dir_all(home)?;
        }
        std::fs::write(&opts.schema, "")?;
        log(&format!("{} Wrote {}: this database is empty, so declare your schema there", paint("green", "✓"), shown(&opts.schema)));
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
    log(&format!("{} Wrote {}, {} and {}", paint("green", "✓"), shown(&opts.schema), shown(&init), shown(&opts.lock)));
    history_push(opts, log).await
}

/// `reylo schema pull`: writes schema.sql from the database, as `init` does, over what it holds: its
/// comments and layout go, and git keeps them. For a change made on the database by hand, which
/// `reylo migration create` then writes as a migration. Changes no database.
pub async fn schema_pull(opts: &Options, schemas: &[String], log: Log<'_>) -> Result<i32, Error> {
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let lines = catalog(&mut db).await;
    db.close().await;
    let sql = if holds_a_schema(&lines?) { dump(opts, schemas).await? } else { String::new() };
    if std::fs::read_to_string(&opts.schema).is_ok_and(|now| now == sql) {
        log(&format!("{} {} matches this database", paint("green", "✓"), shown(&opts.schema)));
        return Ok(0);
    }
    if let Some(home) = opts.schema.parent() {
        std::fs::create_dir_all(home)?;
    }
    std::fs::write(&opts.schema, &sql)?;
    log(&format!("{} Wrote {} from {}", paint("green", "✓"), shown(&opts.schema), shown_url(&opts.url)));
    Ok(0)
}

// ---------------------------------------------------------------------------------------------
// The development commands: `dev`, a database taken to its migrations; `schema push`, one at its migrations
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
    log(&paint("bold red", &format!("✗ {command} changes a development database, and CI is set")));
    log("  In CI, lint the migrations, try them, or run them:");
    log(&format!("    {}", paint("cyan", "reylo migration lint")));
    log(&format!("    {}", paint("cyan", "reylo migrate prod --dry-run")));
    log(&format!("    {}", paint("cyan", "reylo migrate prod")));
    1
}

/// `reylo migrate dev`: takes a development database to its migrations, whatever its
/// history. The history is kept as far as it agrees with the migration files: where a migration it
/// applied was edited or is gone (another branch's), the database is rewound to that point by the
/// diff engine's plan from what those migrations made, losing what it drops. The migrations after it
/// then run for real, their data statements included; one with no data statements whose change the
/// database already has (`schema push` made it) is recorded instead. Never in CI (`in_ci`, from the `CI`
/// variable).
pub async fn dev(opts: &Options, in_ci: bool, log: Log<'_>) -> Result<i32, Error> {
    if in_ci {
        return Ok(refused_in_ci("reylo migrate dev", log));
    }
    let (mut db, mut server, swept) = open(opts).await?;
    let result = dev_on(&mut db, &mut server, swept, opts, log).await;
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

/// A lock deleted, or never written on this branch, starts again from main's, where git has it, and
/// not from the files' names.
fn seed_lock(opts: &Options, files: &[Migration], log: Log<'_>) -> Result<(), Error> {
    if lock::read(&opts.lock) == lock::State::Missing {
        if let Some(main) = main_lock(opts) {
            let here = main_lines_here(&main, files);
            if !here.is_empty() {
                lock::write(&opts.lock, &lock::Lock { schema: None, entries: here })?;
                log(&paint("dim", &format!("Wrote {} from main's", shown(&opts.lock))));
            }
        }
    }
    Ok(())
}

async fn dev_on(db: &mut Db, server: &mut Server, swept: usize, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    swept_note(swept, log);
    let _held = lock(db, opts).await?;
    let d = db.dialect();
    let (files, strays) = read_migrations(&opts.dir)?;
    let mut problems = file_problems(opts, &files, &strays, None, d);
    let history = read_history(db).await?;
    problems.extend(missing_dir(opts, &history));
    if problems.is_empty() {
        seed_lock(opts, &files, log)?;
        problems.extend(dev_lock_problems(opts, &files, &history));
    }
    if !problems.is_empty() {
        print_problems(&problems, log);
        log("");
        log(&paint("dim", "Nothing was changed."));
        return Ok(1);
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
    let agree = applied.iter().zip(&files).take_while(|(r, f)| r.name == f.name && same_statements(&f.sql, &r.sql, d)).count();
    let live = catalog(db).await?;
    let schema_sql = std::fs::read_to_string(&opts.schema).ok();
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain(history.iter().map(|r| r.sql.as_str())).chain(schema_sql.as_deref()).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &texts).await;
    let result = dev_in_shadows(&mut shadows, db, opts, &files, &applied, agree, &live, log).await;
    let result = match result {
        Ok(0) => async {
            relock(&mut shadows, db, opts, &files, schema_sql.as_deref(), log).await?;
            replayed_fingerprint(&mut shadows, db, &files).await?;
            Ok(0)
        }
        .await,
        other => other,
    };
    shadows.finish().await;
    result
}

/// What `dev` leaves for `prod` to compare with: the fingerprint of what the migrations make, not of
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
    }
    Ok(())
}

/// The lock of the main branch, as git has it, where git can tell: `origin/HEAD`'s copy of it.
fn main_lock(opts: &Options) -> Option<lock::Lock> {
    let dir = opts.lock.parent()?;
    let name = opts.lock.file_name()?.to_string_lossy().into_owned();
    let out = std::process::Command::new("git").args(["show", &format!("origin/HEAD:./{name}")]).current_dir(dir).output().ok()?;
    if !out.status.success() {
        return None;
    }
    match lock::of_text(&String::from_utf8_lossy(&out.stdout)) {
        lock::State::Lock(l) => Some(l),
        _ => None,
    }
}

/// Main's lock's lines whose files this checkout has, from the first: what this checkout merged of main.
fn main_lines_here(main: &lock::Lock, files: &[Migration]) -> Vec<lock::Entry> {
    main.entries.iter().take_while(|e| files.iter().any(|f| f.name == e.name)).cloned().collect()
}

/// What keeps `dev` from taking the lock as it is. It writes the lock again,
/// chaining this branch's lines after main's; but a line locked elsewhere (on main where git can
/// say, else one this database never applied) that is out of its chain, or whose file changed,
/// would be laundered so, and is refused.
fn dev_lock_problems(opts: &Options, files: &[Migration], history: &[HistoryRow]) -> Vec<Problem> {
    let path = shown(&opts.lock);
    // A missing lock is written as the migrations apply.
    if lock::read(&opts.lock) == lock::State::Missing {
        return vec![];
    }
    let l = match read_lock(opts, files) {
        Ok(l) => l,
        Err(p) => return vec![p],
    };
    let main = main_lock(opts);
    // Main's lines this checkout has come first, as main has them: a lock that leaves one out, or
    // puts this branch's line before it, was merged keeping this branch's side.
    if let Some(m) = &main {
        let here = main_lines_here(m, files);
        for (k, e) in here.iter().enumerate() {
            if l.entries.get(k).is_some_and(|x| x.name == e.name && x.sum == e.sum) {
                continue;
            }
            let after = if k == 0 { "first".to_string() } else { format!("after {}", here[k - 1].name) };
            return vec![Problem::new(format!("{path} doesn't start with main's lines: main locks {} {after}", e.name))
                .note("Keep main's lines, in main's order, and leave out this branch's: reylo migrate dev then runs this branch's migrations after them, and adds them back.")
                .fix("Then", &["reylo migrate dev"])];
        }
    }
    let ours = |name: &str, sum: &str| match &main {
        Some(m) => !m.entries.iter().any(|e| e.name == name && e.sum == sum),
        None => history.iter().any(|r| r.applied && r.name == name),
    };
    let mut problems = Vec::new();
    for (e, link) in l.entries.iter().zip(links(&l, files)) {
        match link {
            Link::Ok => {}
            Link::Missing => problems.push(Problem::new(format!("{path} lists {}, which isn't in {}", e.name, shown(&opts.dir))).fix("Write it back from a database that ran it", &["reylo migration history pull"])),
            _ if ours(&e.name, &e.sum) => {}
            Link::Edited => problems.push(Problem::new(format!("{} changed after it was locked, on another branch", e.name)).note("A database may have run it as it was: write the change as a new migration.").fix("Take back the text a database ran", &["reylo migration history pull"])),
            Link::Moved { after } => problems.push(
                Problem::new(format!("{path}'s lines were merged out of their order: {} was locked {} on another branch", e.name, after.map_or("first".to_string(), |a| format!("right after {a}"))))
                    .note("Keep main's lines in main's order, and leave out this branch's: reylo migrate dev then runs this branch's migrations after them, and adds them back.")
                    .fix("Then", &["reylo migrate dev"]),
            ),
        }
    }
    problems
}

/// Writes the lock after `dev`: what the database applied, in the order it
/// did, chained; and schema.sql's checksum where the migrations reach it.
async fn relock(shadows: &mut Shadows<'_>, db: &mut Db, opts: &Options, files: &[Migration], schema_sql: Option<&str>, log: Log<'_>) -> Result<(), Error> {
    let history = read_history(db).await?;
    let ran: Vec<&Migration> = history.iter().filter(|r| r.applied).filter_map(|r| files.iter().find(|f| f.name == r.name)).collect();
    let texts: Vec<(String, String)> = ran.iter().map(|f| (f.name.clone(), normalized(&f.sql))).collect();
    let entries = lock::chain(texts.iter().map(|(n, t)| (n.as_str(), t.as_str())));
    let before = match lock::read(&opts.lock) {
        lock::State::Lock(l) => Some(l),
        _ => None,
    };
    // The migrations' replay against schema.sql's, where there is one.
    let mut schema = None;
    if let Some(sql) = schema_sql {
        let ours = shadows.lane();
        shadows.push(ours, ran.iter().map(|m| Step::of(m)));
        let theirs = shadows.lane();
        shadows.push(theirs, [Step::script("schema.sql", sql)]);
        if let (Ok(Ok(a)), Ok(Ok(b))) = (shadows.lines(ours).await, shadows.lines(theirs).await) {
            if a == b {
                schema = Some(lock::schema_sum(&entries.last().map(|e| e.sum.clone()).unwrap_or_default(), sql));
            }
        }
    }
    let next = lock::Lock { schema, entries };
    if before.as_ref() == Some(&next) {
        return Ok(());
    }
    lock::write(&opts.lock, &next)?;
    log(&format!("Wrote {}", shown(&opts.lock)));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn dev_in_shadows(shadows: &mut Shadows<'_>, db: &mut Db, opts: &Options, files: &[Migration], applied: &[&HistoryRow], agree: usize, live: &[String], log: Log<'_>) -> Result<i32, Error> {
    let d = db.dialect();
    let fail = |m: &str, error: String, log: Log<'_>| {
        print_problems(&[Problem::new(m.to_string()).note(error)], log);
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
    let mut rows = Rows::new(format!("Applying to {}", shown_url(&opts.url)), rewind_rows.iter().map(|r| r.name.as_str()).chain(pending.iter().map(|m| m.name.as_str())));
    let mut records: Vec<Migration> = Vec::new();
    // The database's catalog as each step leaves it.
    let mut base: Vec<String> = live.to_vec();
    let refused = |what: &str, lines: Vec<String>, log: Log<'_>| {
        let problem = Problem::new(what.to_string()).lines(lines);
        let problem = if d == Dialect::Mysql {
            problem.fix("MySQL commits each statement, so it is part of the way there. To rebuild it from the migrations, drop this database and create it again, then", &["reylo migrate dev"])
        } else {
            problem.note("That step changed nothing.").fix("To rebuild it from the migrations, drop this database and create it again, then", &["reylo migrate dev"])
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
        for r in &rewind_rows {
            let why = match files.iter().find(|f| f.name == r.name) {
                Some(f) if !same_statements(&f.sql, &r.sql, d) => "it changed after this database applied it",
                Some(_) => "a migration before it came in later, and the two can't run in this order",
                None => "it isn't in the migrations",
            };
            rows.row(log, &paint("yellow", "↺"), &r.name, &format!("rewound: {why}"));
            let data = data_statements(&r.sql, d);
            if !data.is_empty() {
                log(&format!("      {}", paint("yellow", &format!("What it wrote stays ({}): a rewind undoes its schema, not its rows. To start with none: reylo db wipe", data.join(", ")))));
            }
        }
        dropped_row(&plan, &counts, log);
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
                    log(&format!("  {} Undid changes no migration made, which {} can't run on", paint("yellow", "↺"), pending[0].name));
                    dropped_row(&plan, &counts, log);
                }
            }
        }
    }
    if !records.is_empty() || !pending.is_empty() {
        ensure_history(db).await?;
    }
    for m in &records {
        insert_row(db, m, true).await?;
        rows.row(log, &paint("yellow", "!"), &m.name, "recorded, not run: already in place");
    }
    for m in &pending {
        let started = std::time::Instant::now();
        if let Err(e) = run(db, m, true).await {
            rows.row(log, &paint("bold red", "✗"), &m.name, failed_row(&m.sql, d));
            log(&format!("      {e}"));
            return Ok(1);
        }
        rows.row(log, &paint("green", "✓"), &m.name, &format!("applied {}", paint("dim", &format!("· {} ms", started.elapsed().as_millis()))));
    }
    if rewind_rows.is_empty() && records.is_empty() && pending.is_empty() && base == at_kept {
        log(&format!("{} Up to date", paint("green", "✓")));
    }
    Ok(0)
}

/// `reylo schema push`: takes a development database that is at its migrations (`dev` brings it
/// there) to schema.sql: what schema.sql has that no migration makes is made, unrecorded,
/// for `create` to write next. Refuses a database with a migration it hasn't applied, or one whose
/// history disagrees with the files. Never in CI.
pub async fn schema_push(opts: &Options, in_ci: bool, log: Log<'_>) -> Result<i32, Error> {
    if in_ci {
        return Ok(refused_in_ci("reylo schema push", log));
    }
    let (mut db, mut server, swept) = open(opts).await?;
    let result = schema_push_on(&mut db, &mut server, swept, opts, log).await;
    db.close().await;
    server.close().await;
    result
}

/// `reylo schema push --watch`: syncs, then again each time schema.sql or a migration changes,
/// until it is stopped. Each run prints its block under the time it ran; one that changes nothing
/// prints a line. A refusal (a migration to apply first) is printed and waited out like any run.
/// `runs` bounds how many runs it makes, for a test; None runs until stopped.
pub async fn schema_push_watch(opts: &Options, in_ci: bool, runs: Option<usize>, log: Log<'_>) -> Result<i32, Error> {
    if in_ci {
        return Ok(refused_in_ci("reylo schema push --watch", log));
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
    log(&paint("dim", &format!("Watching {} and {}: each change pushes again (Ctrl-C stops)", shown(&opts.schema), shown(&opts.dir))));
    let mut made = 0;
    let mut last = Vec::new();
    loop {
        let before = snapshot();
        let mut lines: Vec<String> = Vec::new();
        // A database that went away (restarting) is waited out like a refusal: said, then watched.
        let code = match schema_push(opts, false, &mut |l: &str| lines.push(l.to_string())).await {
            Ok(code) => code,
            Err(e) => {
                lines.push(format!("{} {e}", paint("bold red", "✗")));
                1
            }
        };
        let at = time::clock();
        let quiet = code == 0 && lines.iter().all(|l| l.contains("This database matches schema.sql and its migrations"));
        if quiet && !last.is_empty() {
            log(&paint("dim", &format!("{at} · nothing to change")));
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

async fn schema_push_on(db: &mut Db, server: &mut Server, swept: usize, opts: &Options, log: Log<'_>) -> Result<i32, Error> {
    swept_note(swept, log);
    let _held = lock(db, opts).await?;
    let d = db.dialect();
    let (files, strays) = read_migrations(&opts.dir)?;
    let schema_sql = std::fs::read_to_string(&opts.schema).ok();
    let mut problems = file_problems(opts, &files, &strays, schema_sql.as_deref(), d);
    if schema_sql.is_none() {
        problems.push(Problem::new(format!("{} doesn't exist", shown(&opts.schema))).note("sync takes the database to it."));
    }
    // The database is at its migrations, as `dev` leaves it: every migration applied, each
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
    // The lock lists what the database applied, in its order.
    if behind.is_empty() {
        match lock::read(&opts.lock) {
            lock::State::Lock(l) if l.names() == applied.iter().map(|r| r.name.clone()).collect::<Vec<_>>() => {}
            lock::State::Missing if applied.is_empty() => {}
            _ => behind.push(format!("  {} doesn't list what it applied, in its order", shown(&opts.lock))),
        }
    }
    if !behind.is_empty() {
        problems.push(Problem::new("This database isn't at its migrations").lines(behind).fix("Bring it to them first", &["reylo migrate dev"]));
    }
    if !problems.is_empty() {
        print_problems(&problems, log);
        log("");
        log(&paint("dim", "Nothing was changed."));
        return Ok(1);
    }
    let schema_sql = schema_sql.unwrap_or_default();
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain([schema_sql.as_str()]).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, &opts.cache, &texts).await;
    let in_order: Vec<Migration> = applied.iter().filter_map(|r| files.iter().find(|f| f.name == r.name).cloned()).collect();
    let result = schema_push_in_shadows(&mut shadows, db, opts, &in_order, &schema_sql, log).await;
    shadows.finish().await;
    result
}

/// What schema.sql has that no migration makes, made on the database, unrecorded; `applied` is the
/// migrations in the order the database ran them.
async fn schema_push_in_shadows(shadows: &mut Shadows<'_>, db: &mut Db, opts: &Options, applied: &[Migration], schema_sql: &str, log: Log<'_>) -> Result<i32, Error> {
    let d = db.dialect();
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
    let wanted = catalog(shadows.db(s_lane)).await?;
    let now = crate::catalog::without_extra_children(&catalog(db).await?, &wanted);
    let pushed = if now != wanted {
        let target = read_target(shadows.db(s_lane), Some(schema_sql)).await?;
        let counts = table_rows(db).await?;
        let (renames, plan) = match take_to(db, &target, &[], &wanted).await {
            Err(e) => return fail("Taking this database to schema.sql failed", e.0, log),
            Ok(Err(lines)) => {
                let problem = Problem::new("Reylo can't take this database the rest of the way to schema.sql").lines(lines);
                let problem = if d == Dialect::Mysql { problem } else { problem.note("Nothing was changed.") };
                print_problems(&[problem.fix("Write the change as a migration, finishing by hand what create leaves out", &["reylo migration create <name>"])], log);
                return Ok(1);
            }
            Ok(Ok(done)) => done,
        };
        log(&paint("dim", &format!("Pushing schema.sql to {}", shown_url(&opts.url))));
        for statement in renames.iter().chain(&plan.statements) {
            statement_row(statement, log);
        }
        dropped_row(&plan, &counts, log);
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
    if made && !pushed {
        log(&format!("{} This database matches schema.sql and its migrations", paint("green", "✓")));
    } else if !made {
        if pushed {
            log("");
        } else {
            log(&format!("{} This database matches schema.sql", paint("green", "✓")));
        }
        log("  Not in any migration yet:");
        log(&format!("    {}", paint("cyan", "reylo migration create <name>")));
    }
    if wrote {
        log(&format!("Wrote {}", shown(&opts.lock)));
    }
    Ok(0)
}

/// One statement `schema push` ran: `+` for what it adds, `-` for what it takes away; a comment is dim.
fn statement_row(statement: &str, log: Log<'_>) {
    if statement.starts_with("--") {
        log(&format!("    {}", paint("dim", statement)));
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

/// The line saying what a plan drops, each table with how many rows it held where `counts` holds them
/// ("Dropped tags, 3 rows; users.bio"), if it drops anything.
fn dropped_row(plan: &crate::diff::Plan, counts: &std::collections::BTreeMap<String, u64>, log: Log<'_>) {
    if plan.losses.is_empty() {
        return;
    }
    let what: Vec<String> = plan
        .losses
        .iter()
        .map(|l| {
            let name = l.trim_start_matches("column ").trim_start_matches("table ");
            let rows = counts.get(l).copied();
            match rows {
                Some(0) => format!("{name}, no rows"),
                Some(1) => format!("{name}, 1 row"),
                Some(n) => format!("{name}, {n} rows"),
                None => name.to_string(),
            }
        })
        .collect();
    log(&format!("    {}", paint("yellow", &format!("Dropped {}", what.join("; ")))));
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
                log(&format!("{} {e}", paint("red", "✗")));
                code = 1;
                break;
            }
        }
    }
    db.close().await;
    Ok(code)
}
