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
        Dialect::Postgres => format!("create table if not exists {HISTORY_TABLE} (name text primary key, sql text not null, applied_at timestamptz)"),
        Dialect::Mysql => format!("create table if not exists {HISTORY_TABLE} (name varchar(255) primary key, `sql` longtext not null, applied_at datetime(6) null) engine=InnoDB"),
        Dialect::Sqlite => format!("create table if not exists {HISTORY_TABLE} (name text primary key, sql text not null, applied_at text)"),
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
    let rows = db
        .rows(&format!(
            "select name, {} as body, {applied_at} as applied_at from {HISTORY_TABLE} order by applied_at is null, applied_at, case when name = '{INIT}' then '' else name end",
            sql_col(d)
        ))
        .await?;
    Ok(rows.into_iter().map(|r| HistoryRow { name: r.text("name").into(), sql: r.text("body").into(), applied: r.get("applied_at").is_some() }).collect())
}

async fn lock(db: &mut Db) -> Result<(), Error> {
    match db.dialect() {
        Dialect::Postgres => db.exec("select pg_advisory_lock(hashtextextended('reylo_migrate', 0))").await,
        Dialect::Mysql => db.exec("select get_lock('reylo_migrate', 600)").await,
        Dialect::Sqlite => Ok(()),
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

#[derive(Debug, Default)]
pub struct Plan {
    pub ok: bool,
    pub problems: Vec<String>,
    /// Migrations to run, in time order.
    pub pending: Vec<Migration>,
    /// Pending migrations whose change the live database already has: recorded without running.
    pub adopted: Vec<Migration>,
    /// Applied migrations edited since, which the live database matches as edited: their recorded text is updated.
    pub edited: Vec<Migration>,
}

fn data_note(m: &Migration, d: Dialect) -> String {
    let data: Vec<String> = split_statements(&m.sql, d)
        .iter()
        .filter(|s| changes_data(s))
        .map(|s| format!("{}…", s.text.split_whitespace().take(3).collect::<Vec<_>>().join(" ").to_lowercase()))
        .collect();
    if data.is_empty() {
        String::new()
    } else {
        format!("; its data statements ({}) are taken to have run", data.join(", "))
    }
}

/// What is wrong with the lock of a linear history: it must list the migration files, each once, in
/// the order of their names, which is the order they run in.
fn lock_problems(opts: &Options, files: &[Migration]) -> Vec<String> {
    let path = shown(&opts.dir.join(LOCK));
    let names = match lock::read(&opts.dir) {
        lock::State::Missing => return vec![format!("the history is linear, and {path} is missing: reylo migrate rebase writes it from the migration files")],
        lock::State::Conflicted => return vec![format!("{path} has a merge conflict: another branch added migrations. reylo migrate rebase moves this branch's after them and resolves it")],
        lock::State::Invalid(e) => return vec![format!("{path} does not read as a lock file ({e}): reylo migrate rebase writes it again from the migration files")],
        lock::State::Names(names) => names,
    };
    let order: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    if names.iter().map(String::as_str).eq(order.iter().copied()) {
        return vec![];
    }
    let mut out = vec![format!("{path} does not list the migration files in the order they run; reylo migrate rebase puts them in its order:")];
    for f in order.iter().filter(|f| !names.iter().any(|n| n == *f)) {
        out.push(format!("  + {f} is not in it"));
    }
    for n in names.iter().filter(|n| !order.contains(&n.as_str())) {
        out.push(format!("  - {n} is in it, but is not a migration file"));
    }
    if out.len() == 1 {
        out.push(format!("  it lists {}", names.join(", ")));
    }
    out
}

/// Checks the target against the files and `schema.sql`; changes nothing on the target. A database
/// that already has what migrations would do is reconciled rather than refused: when the live
/// database equals the shadow after some pending migrations, those are recorded without running
/// (DDL run by hand during development, then written as a migration); when it equals the shadow of
/// an applied migration as edited since, the recorded text is updated. Neither runs anything on the
/// target; the data statements of such a migration are taken to have run, which no shadow can see.
pub async fn check(db: &mut Db, server: &mut Server, opts: &Options, log: Log<'_>) -> Result<Plan, Error> {
    let d = db.dialect();
    let scope = opts.scope();
    let (files, strays) = read_migrations(&opts.dir)?;
    let mut problems = Vec::new();
    for s in &strays {
        problems.push(format!("{s} is not a migration: a file in {} is init.sql or <YYYYMMDDHHMMSS>_<name>.sql, the name in a-z, 0-9 and _", shown(&opts.dir)));
    }
    let schema_sql = std::fs::read_to_string(&opts.schema).ok();
    if schema_sql.is_none() {
        problems.push(format!("{} does not exist: the declared schema is required", shown(&opts.schema)));
    }
    if opts.linear {
        problems.extend(lock_problems(opts, &files));
    } else if opts.dir.join(LOCK).exists() {
        log(&paint("yellow", &format!("{} is read only when the history is linear: delete it, or set migrate.history: linear", shown(&opts.dir.join(LOCK)))));
    }
    let history = read_history(db).await?;
    // A row marked started (a lone statement cut off before its row was finished) counts as pending:
    // whether it landed is read from the live database below, as for any pending migration.
    let applied: Vec<&HistoryRow> = history.iter().filter(|r| r.applied).collect();
    let by_name = |name: &str| files.iter().find(|f| f.name == name);
    let mut edited: Vec<Migration> = Vec::new();
    for r in &applied {
        match by_name(&r.name) {
            None => problems.push(format!(
                "{} was applied to this database but is not in the directory (renamed by a rebase? then apply --reset on this development database, or repair one that cannot be wiped)",
                r.name
            )),
            // An edit to comments or spacing alone changes nothing that ran; any other edit is
            // recorded once the database matches it, below.
            Some(f) if normalized(&f.sql) != normalized(&r.sql) => edited.push(f.clone()),
            Some(_) => {}
        }
    }
    let applied_files: Vec<Migration> = applied.iter().filter_map(|r| by_name(&r.name).cloned()).collect();
    let pending: Vec<Migration> = files.iter().filter(|f| !applied_files.iter().any(|a| a.name == f.name)).cloned().collect();
    let newest = applied_files.iter().map(|f| f.time.clone()).max().unwrap_or_default();
    if opts.linear {
        for p in &pending {
            if p.time < newest {
                let other = applied_files.iter().find(|f| f.time == newest).map(|f| f.name.clone()).unwrap_or_default();
                problems.push(format!("{} is older than {other}, which this database already applied, and the history is linear: rebase it (reylo migrate rebase {})", p.name, p.name));
            }
        }
    }
    // init.sql, which `init` writes, holds the whole schema and runs only on an empty database.
    for p in pending.iter().filter(|f| f.name != INIT) {
        if let Some(why) = atomicity_problem(&p.sql, d) {
            problems.push(format!("{} cannot run atomically: {why}", p.name));
        }
    }
    let schema_name = opts.schema.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "schema.sql".into());
    for (name, sql) in files.iter().map(|f| (f.name.as_str(), f.sql.as_str())).chain(schema_sql.as_deref().map(|s| (schema_name.as_str(), s))) {
        if let Some(why) = shadow_problem(sql, d) {
            problems.push(format!("{name} cannot be replayed in a shadow database: {why}"));
        }
    }
    let fail = |problems: Vec<String>| Plan { ok: false, problems, ..Plan::default() };
    if !problems.is_empty() {
        return Ok(fail(problems));
    }
    let schema_sql = schema_sql.unwrap_or_default();

    let live = catalog(db, scope).await?;
    let texts: Vec<&str> = files.iter().map(|f| f.sql.as_str()).chain(history.iter().map(|r| r.sql.as_str())).chain([schema_sql.as_str()]).collect();
    let mut shadows = Shadows::new(server, &opts.url, &opts.root, scope, &texts).await;
    let result = check_in_shadows(&mut shadows, db, &live, &schema_sql, &files, &applied, &applied_files, &pending, edited, d, log).await;
    shadows.finish().await;
    result.map(|r| r.unwrap_or_else(fail))
}

#[allow(clippy::too_many_arguments)]
async fn check_in_shadows(
    shadows: &mut Shadows<'_>,
    target: &mut Db,
    live: &[String],
    schema_sql: &str,
    files: &[Migration],
    applied: &[&HistoryRow],
    applied_files: &[Migration],
    pending: &[Migration],
    edited: Vec<Migration>,
    d: Dialect,
    log: Log<'_>,
) -> Result<Result<Plan, Vec<String>>, Error> {
    let a_lane = shadows.lane();
    shadows.push(a_lane, [Step::script("schema.sql", schema_sql)]);
    let a = match shadows.lines(a_lane).await? {
        Ok(lines) => lines,
        Err((_, error)) => return Ok(Err(vec![format!("schema.sql fails on an empty database: {error}")])),
    };

    // B: the target's own order, as the files read now, then compared with the live target. With
    // migrations pending, the target's history is where the next check starts, so it is kept.
    let b_lane = shadows.lane();
    shadows.push(b_lane, applied_files.iter().map(Step::of));
    if !pending.is_empty() {
        shadows.mark(b_lane);
    }
    let mut checkpoint = match shadows.lines(b_lane).await? {
        Ok(lines) => lines,
        Err((m, error)) => return Ok(Err(vec![format!("{m}, applied to this database, fails when replayed: {error}")])),
    };
    let mut adopted: Vec<Migration> = Vec::new();
    if checkpoint != live {
        // An edited migration the database still matches as it ran: the edit has not reached it.
        if !edited.is_empty() {
            let as_ran = shadows.lane();
            shadows.push(as_ran, applied.iter().map(|r| Step::migration(&r.name, &r.sql)));
            if let Ok(lines) = shadows.lines(as_ran).await? {
                if lines == live {
                    let names = edited.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ");
                    let mut out = vec![format!(
                        "{names} {} edited after being applied, and this database still matches the version it ran. Change the database to match the edit (run the change, for instance with reylo run) and apply again, which records the edited text; or apply --reset on this development database. What the edit changes:",
                        if edited.len() == 1 { "was" } else { "were" }
                    )];
                    out.extend(describe_diff("as it ran", &lines, "as edited", &checkpoint));
                    return Ok(Err(out));
                }
            }
        }
        // Pending migrations whose change the database already has, from the oldest: recorded, not run.
        let mut current = checkpoint.clone();
        for m in pending {
            shadows.push(b_lane, [Step::of(m)]);
            match shadows.lines(b_lane).await? {
                Err(_) => break,
                Ok(lines) => {
                    current = lines;
                    adopted.push(m.clone());
                    if current == live {
                        break;
                    }
                }
            }
        }
        if !adopted.is_empty() && current == live {
            checkpoint = current;
            for m in &adopted {
                log(&paint("yellow", &format!("{}: the database already has it; recorded without running{}", m.name, data_note(m, d))));
            }
        } else if a == live {
            // The database already matches schema.sql, and no migration makes the change yet: the
            // developer changed both by hand and has the migration left to write.
            let mut out = vec!["The database matches schema.sql, but no migration makes this change yet. `reylo migrate create <name>` writes it, and the next run records it without running it:".to_string()];
            let prefix = shadows.lane();
            shadows.push(prefix, applied_files.iter().map(Step::of));
            match lane_plan(shadows, prefix, a_lane, Some(schema_sql)).await {
                Some(sql) => out.extend(indented(&sql)),
                None => out.extend(describe_diff("history", &checkpoint, "database", live)),
            }
            return Ok(Err(out));
        } else {
            let mut out = vec!["The database differs from its applied history (drift): something changed it outside these migrations.".to_string()];
            out.extend(describe_diff("history", &checkpoint, "database", live));
            let prefix = shadows.lane();
            shadows.push(prefix, applied_files.iter().map(Step::of));
            if let Ok(Ok(())) = shadows.open(prefix).await {
                shadows.taint(prefix);
                if let Some(sql) = plan_text(target, shadows.db(prefix), None).await {
                    out.push("  To undo it, run this (reylo run \"…\"):".to_string());
                    out.extend(indented(&sql));
                }
                if let Some(sql) = plan_text(shadows.db(prefix), target, None).await {
                    out.push("  To keep it, declare it in schema.sql and write it as a migration (reylo migrate create <name>), which the next run records without running:".to_string());
                    out.extend(indented(&sql));
                }
            }
            return Ok(Err(out));
        }
    }
    let _ = checkpoint;
    for m in &edited {
        let ran = applied.iter().find(|r| r.name == m.name).map(|r| r.sql.as_str()).unwrap_or_default();
        let tokens = |sql: &str| split_statements(sql, d).into_iter().map(|s| s.tokens.into_iter().map(|t| t.text).collect::<Vec<_>>()).collect::<Vec<_>>();
        log(&paint(
            "yellow",
            &if tokens(&m.sql) == tokens(ran) {
                format!("{}: comments edited; its recorded text is updated", m.name)
            } else {
                format!("{}: edited after it was applied, and the database matches the edit; its recorded text is updated{}", m.name, data_note(m, d))
            },
        ));
    }
    let rest: Vec<Migration> = pending[adopted.len()..].to_vec();
    shadows.push(b_lane, rest.iter().map(Step::of));
    let b = match shadows.lines(b_lane).await? {
        Ok(lines) => lines,
        Err((m, error)) => return Ok(Err(vec![format!("{m} fails on this database's history: {error}")])),
    };

    // C: every migration by time, unless that is B's own sequence.
    let b_order: Vec<&str> = applied_files.iter().chain(pending).map(|f| f.name.as_str()).collect();
    let c_order: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    let c = if b_order != c_order {
        let c_lane = shadows.lane();
        shadows.push(c_lane, files.iter().map(Step::of));
        match shadows.lines(c_lane).await? {
            Ok(lines) => lines,
            Err((m, error)) => return Ok(Err(vec![format!("{m} fails when every migration replays by time: {error}")])),
        }
    } else {
        b.clone()
    };
    let mut issues = Vec::new();
    if a != b {
        issues.push("After its pending migrations, this database would differ from schema.sql:".to_string());
        issues.extend(describe_diff("schema.sql", &a, "migrations", &b));
        if let Some(sql) = lane_plan(shadows, b_lane, a_lane, Some(schema_sql)).await {
            issues.push("  Declare in schema.sql what the migrations make, or let `reylo migrate create <name>` write the migration that takes them to schema.sql:".to_string());
            issues.extend(indented(&sql));
        }
    }
    if b != c {
        issues.push("This database's order and the order by time end in different schemas:".to_string());
        issues.extend(describe_diff("this database", &b, "by time", &c));
    }
    Ok(Ok(Plan { ok: issues.is_empty(), problems: issues, pending: rest, adopted, edited }))
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

/// SQL indented under a message, one line each.
fn indented(sql: &str) -> Vec<String> {
    sql.lines().map(|l| format!("    {l}")).collect()
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

fn print_problems(problems: &[String], log: Log<'_>) {
    for p in problems {
        if p.starts_with("  ") {
            log(p);
        } else {
            log(&format!("{} {p}", paint("red", "✗")));
        }
    }
}

/// `reylo migrate apply`: check, then apply what is pending; with `dry_run`, check alone.
pub async fn migrate(opts: &Options, dry_run: bool, log: Log<'_>) -> Result<i32, Error> {
    let (mut db, mut server, swept) = open(opts).await?;
    let result = migrate_on(&mut db, &mut server, swept, opts, dry_run, log).await;
    db.close().await;
    server.close().await;
    result
}

async fn migrate_on(db: &mut Db, server: &mut Server, swept: usize, opts: &Options, dry_run: bool, log: Log<'_>) -> Result<i32, Error> {
    if swept > 0 {
        log(&paint("dim", &format!("dropped {swept} shadow database{} a run left behind", if swept == 1 { "" } else { "s" })));
    }
    lock(db).await?;
    let plan = check(db, server, opts, log).await?;
    if !plan.ok {
        print_problems(&plan.problems, log);
        return Ok(1);
    }
    if plan.pending.is_empty() && plan.adopted.is_empty() && plan.edited.is_empty() {
        log(&format!("{} up to date", paint("green", "✓")));
        return Ok(0);
    }
    for p in &plan.pending {
        log(&format!("  {} {}", if dry_run { "would apply" } else { "apply" }, p.name));
    }
    if dry_run {
        let n = plan.pending.len();
        log(&format!("{} {n} pending migration{} check out against schema.sql", paint("green", "✓"), if n == 1 { "" } else { "s" }));
        return Ok(0);
    }
    let d = db.dialect();
    db.exec(&history_ddl(d)).await?;
    // Rows marked started belong to pending or adopted migrations: they are written again below.
    db.exec(&format!("delete from {HISTORY_TABLE} where applied_at is null")).await?;
    for m in &plan.edited {
        db.exec_with(&format!("update {HISTORY_TABLE} set {} = {} where name = {}", sql_col(d), db.param(1), db.param(2)), &[&m.sql, &m.name]).await?;
    }
    for m in &plan.adopted {
        insert_row(db, m, true).await?;
    }
    for m in &plan.pending {
        match run(db, m, true).await {
            Ok(()) => log(&format!("{} {}", paint("green", "✓"), m.name)),
            Err(e) => {
                log(&format!("{} {}: {e}", paint("red", "✗"), m.name));
                return Ok(1);
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
    let code = migrate(opts, false, log).await?;
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
    lock(db).await?;
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
        (Err((_, e)), _) => issues.push(format!("schema.sql fails on an empty database: {e}")),
        (_, Err((m, e))) => issues.push(format!("{m} fails when every migration replays by time: {e}")),
        (Ok(a), Ok(c)) => {
            if *a != live {
                issues.push("The database differs from schema.sql:".to_string());
                issues.extend(describe_diff("schema.sql", a, "database", &live));
            }
            if *c != live {
                issues.push("The database differs from the migrations:".to_string());
                issues.extend(describe_diff("migrations", c, "database", &live));
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
    db.exec(&history_ddl(d)).await?;
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

/// `reylo migrate create <name>`: a migration named with the current UTC time, refused while any
/// migration is unapplied, so that its time comes after theirs.
pub async fn create(opts: &Options, name: &str, log: Log<'_>) -> Result<i32, Error> {
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
    let mut db = Db::connect(&opts.url, &opts.root).await?;
    let history = read_history(&mut db).await;
    db.close().await;
    let history = history?;
    let unapplied: Vec<&str> = files.iter().filter(|f| !history.iter().any(|r| r.name == f.name)).map(|f| f.name.as_str()).collect();
    if !unapplied.is_empty() {
        let one = unapplied.len() == 1;
        log(&format!(
            "{} {} {} not applied to this database yet: apply {} first (reylo migrate apply), so the new migration's time comes after.",
            paint("red", "✗"),
            unapplied.join(", "),
            if one { "is" } else { "are" },
            if one { "it" } else { "them" }
        ));
        return Ok(1);
    }
    let schema_sql = std::fs::read_to_string(&opts.schema).map_err(|e| Error(format!("cannot read {}: {e}", shown(&opts.schema))))?;
    let (texts, code) = draft(opts, &files, &schema_sql, log).await?;
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
async fn draft(opts: &Options, files: &[Migration], schema_sql: &str, log: Log<'_>) -> Result<(Vec<String>, i32), Error> {
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
                return Ok((vec![String::new()], 0));
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
        let mut plan = crate::diff::plan(&current, &target, alterer.as_ref());
        style(shadows.db(c), &mut plan, &target).await?;
        if plan.statements.is_empty() && moves.is_empty() {
            log(&paint("dim", &format!("the migrations already make {}: the migration is empty, for a change of data", shown(&opts.schema))));
            return Ok((vec![String::new()], 0));
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
            log(&paint("yellow", &format!("loses data: {}", loss.trim_start_matches("column ").trim_start_matches("table "))));
        }
        for (old, new) in &plan.renames {
            log(&paint("yellow", &format!("{} is renamed {new}, keeping its rows; if they are two columns, write a drop and an add instead (the file's comment has them)", old.trim_start_matches("column "))));
        }
        if failed.is_none() && reached == wanted {
            return Ok((texts, 0));
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
        Ok((texts, 1))
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
    migrate(opts, false, log).await
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
