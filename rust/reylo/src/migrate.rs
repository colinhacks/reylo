//! The project's migrations, run from the app: `reylo::migrate!()` holds the migrations, the lock
//! and `schema.sql` of the project's `reylo/` directory as the build read them, `.run(&pool)` runs
//! `reylo migrate deploy` with them on the pool's database, and `.status(&pool)` runs
//! `reylo migrate status`. Both are the command's own code (`reylo-migrate`), built without the
//! shadow databases and the diff engine the command line carries, on one connection of the pool's.
//!
//! The connection is the pool's, with two differences from what the app's queries get. On MySQL it
//! runs with the server's own `sql_mode` and time zone, as the command line does, rather than with
//! the ones sqlx sets (`PIPES_AS_CONCAT`, which makes `||` concatenate, and UTC), so a migration
//! means what it meant when `reylo migrate` checked it. And a deploy on Postgres or MySQL closes it
//! once it is done rather than handing it back to the pool, since a migration's `SET` would outlive
//! the run on it; SQLite's goes back, with `foreign_keys` as it was, since closing the last
//! connection of an in-memory database deletes it.

// With no database's feature on, there is no connection to run a session on.
#![cfg_attr(not(any(feature = "postgres", feature = "mysql", feature = "sqlite")), allow(dead_code, unused_macros, unused_imports))]

use crate::sqlx;
use reylo_migrate::db::{Answer, Db, Row, Session};
use reylo_migrate::{Dialect, Embedded, Options};
use std::path::PathBuf;
use std::sync::Arc;

/// The project's migrations, its lock and its `schema.sql`, as `reylo::migrate!()` compiled them
/// into the crate.
#[derive(Clone, Copy, Debug)]
pub struct Migrator {
    home: &'static str,
    files: &'static [(&'static str, &'static str)],
    lock: Option<&'static str>,
    schema: Option<&'static str>,
}

impl Migrator {
    #[doc(hidden)]
    pub const fn __new(home: &'static str, files: &'static [(&'static str, &'static str)], lock: Option<&'static str>, schema: Option<&'static str>) -> Migrator {
        Migrator { home, files, lock, schema }
    }

    /// Runs `reylo migrate deploy` on the pool's database: each locked migration it hasn't run, in
    /// the lock's order, each in a transaction with its row in the history table. Resolves to the
    /// names of those it ran, none where it had run them all.
    ///
    /// It refuses, running nothing, where the command would: a migration the lock doesn't list
    /// (a draft), one edited or deleted since the database ran it, a lock git left conflicted, and a
    /// database whose schema was changed by hand since the last deploy. A migration that fails rolls
    /// back with its row, and those before it stay applied ([`Error::applied`]). The error says why,
    /// as the command prints it, and [`problems`](Error::problems) lists each refusal with its fix.
    ///
    /// A deploy holds the database's migration lock while it runs, the command line's too, so two
    /// instances of the app starting at once run each migration once.
    pub async fn run<DB: Database>(&self, pool: &sqlx::Pool<DB>) -> Result<Vec<String>, Error> {
        let run = self.on(pool, Command::Deploy).await?;
        let listed = run.result.as_ref().and_then(|r| r["migrations"].as_array()).map(Vec::as_slice).unwrap_or_default();
        let with = |state: &str| -> Vec<String> { listed.iter().filter(|m| m["state"] == state).filter_map(|m| m["name"].as_str().map(String::from)).collect() };
        let applied = with("applied");
        if run.code != 0 {
            let failed = !with("failed").is_empty();
            return Err(Error::stopped(if failed { "reylo migrate deploy failed" } else { "reylo migrate deploy refused" }, &run, applied));
        }
        Ok(applied)
    }

    /// Runs `reylo migrate status` on the pool's database: each migration with whether it ran, and
    /// what the command checks beside them. [`Status::is_current`] says whether the database has
    /// run every migration the build holds, with nothing out of step; its `Display` is what the
    /// command prints. Status changes nothing and takes no lock.
    pub async fn status<DB: Database>(&self, pool: &sqlx::Pool<DB>) -> Result<Status, Error> {
        let run = self.on(pool, Command::Status).await?;
        let listed = run.result.as_ref().and_then(|r| r["migrations"].as_array()).map(Vec::as_slice).unwrap_or_default();
        let migrations = listed
            .iter()
            .map(|m| MigrationStatus {
                name: m["name"].as_str().unwrap_or_default().to_string(),
                state: match m["state"].as_str() {
                    Some("applied") => State::Applied,
                    Some("pending") => State::Pending,
                    Some("edited") => State::Edited,
                    Some("missing") => State::Missing,
                    _ => State::CutOff,
                },
                applied_at: m["applied_at"].as_str().map(String::from),
            })
            .collect();
        Ok(Status { migrations, problems: problems(&run.problems), lines: run.lines, current: run.code == 0 })
    }

    async fn on<DB: Database>(&self, pool: &sqlx::Pool<DB>, command: Command) -> Result<reylo_migrate::commands::HostRun, Error> {
        let home = PathBuf::from(self.home);
        let options = Options {
            url: DB::url(&*pool.connect_options()),
            shadow_url: None,
            dir: home.join("migrations"),
            schema: home.join("schema.sql"),
            lock: home.join(reylo_migrate::lock::LOCK),
            cache: home.join(".cache"),
            // A relative SQLite path is the process's, as sqlx opens it.
            root: PathBuf::new(),
            embedded: Some(Arc::new(Embedded {
                files: self.files.iter().map(|(name, text)| (name.to_string(), text.to_string())).collect(),
                lock: self.lock.map(String::from),
                schema: self.schema.map(String::from),
            })),
        };
        let mut conn = pool.acquire().await.map_err(|e| Error::new(format!("reylo::migrate!() could not take a connection from the pool: {e}")))?;
        if command == Command::Deploy && DB::DIALECT != Dialect::Sqlite {
            conn.close_on_drop();
        }
        let mut db = Db::Host(DB::session(conn));
        let kept = keep(&mut db).await.map_err(Error::of)?;
        let run = match command {
            Command::Deploy => reylo_migrate::deploy_on_host(&mut db, &options).await,
            Command::Status => reylo_migrate::status_on_host(&mut db, &options).await,
        };
        let restored = restore(&mut db, kept).await;
        let run = run.map_err(Error::of)?;
        restored.map_err(Error::of)?;
        Ok(run)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    Deploy,
    Status,
}

/// What the run changes on its connection that the app's queries on it after the run must not see:
/// MySQL's `sql_mode` and time zone, set to the server's own for the run; SQLite's `foreign_keys`,
/// which a migration's run turns off and on.
enum Kept {
    Nothing,
    Mysql { mode: String, zone: String },
    Sqlite { foreign_keys: String },
}

async fn keep(db: &mut Db) -> Result<Kept, reylo_migrate::Error> {
    match db.dialect() {
        Dialect::Postgres => Ok(Kept::Nothing),
        Dialect::Mysql => {
            let rows = db.rows("select @@session.sql_mode as mode, @@session.time_zone as zone").await?;
            let row = rows.first().cloned().unwrap_or_default();
            db.exec("set session sql_mode = @@global.sql_mode, time_zone = @@global.time_zone").await?;
            Ok(Kept::Mysql { mode: row.text("mode").to_string(), zone: row.text("zone").to_string() })
        }
        Dialect::Sqlite => {
            let rows = db.rows("pragma foreign_keys").await?;
            Ok(Kept::Sqlite { foreign_keys: rows.first().and_then(Row::first).unwrap_or("0").to_string() })
        }
    }
}

async fn restore(db: &mut Db, kept: Kept) -> Result<(), reylo_migrate::Error> {
    match kept {
        Kept::Nothing => Ok(()),
        Kept::Mysql { mode, zone } => db.exec_with("set session sql_mode = ?, time_zone = ?", &[&mode, &zone]).await,
        Kept::Sqlite { foreign_keys } => db.exec(if foreign_keys == "1" { "pragma foreign_keys = on" } else { "pragma foreign_keys = off" }).await,
    }
}

/// A database `reylo::migrate!()` runs on: Postgres, MySQL or SQLite, as `reylo`'s features of the
/// same names turn them on.
pub trait Database: sqlx::Database + private::Hosted {}

impl<DB: sqlx::Database + private::Hosted> Database for DB {}

mod private {
    use super::*;

    pub trait Hosted: sqlx::Database + Sized {
        const DIALECT: Dialect;
        /// The database's URL as a message shows it, with no password; for SQLite, the path the run
        /// lock is named after.
        fn url(options: &<Self::Connection as sqlx::Connection>::Options) -> String;
        /// The connection, as the migrations' code runs statements on it.
        fn session(conn: sqlx::pool::PoolConnection<Self>) -> Box<dyn Session>;
    }
}

/// One connection of the pool's, as the migrations' code runs statements on it: each as written,
/// every value as text, as `reylo migrate` reads them through its own clients.
struct Pooled<DB: sqlx::Database>(sqlx::pool::PoolConnection<DB>);

/// A statement for sqlx, which on 0.9 takes a string it is told is safe to send: each of these is
/// the migrations' code's own or a migration's text, run as written.
#[cfg(feature = "sqlx_0_9")]
fn text(sql: &str) -> sqlx::AssertSqlSafe<String> {
    sqlx::AssertSqlSafe(sql.to_owned())
}
#[cfg(not(feature = "sqlx_0_9"))]
fn text(sql: &str) -> &str {
    sql
}

/// The database's message, with Postgres's detail after it, as the command line writes them.
fn failed(e: sqlx::Error) -> reylo_migrate::Error {
    match e {
        sqlx::Error::Database(d) => {
            #[allow(unused_mut)]
            let mut message = d.message().to_string();
            #[cfg(feature = "postgres")]
            if let Some(detail) = d.try_downcast_ref::<sqlx::postgres::PgDatabaseError>().and_then(|p| p.detail()) {
                message = format!("{message} ({detail})");
            }
            reylo_migrate::Error(message)
        }
        other => reylo_migrate::Error(other.to_string()),
    }
}

/// A row with each of its values as `value` reads the one at an index.
fn row_of<R: sqlx::Row>(row: &R, value: impl Fn(&R, usize) -> sqlx::Result<Option<String>>) -> Result<Row, reylo_migrate::Error> {
    use sqlx::Column;
    let columns = row.columns().iter().map(|c| c.name().to_string()).collect();
    let values = (0..row.len()).map(|i| value(row, i)).collect::<sqlx::Result<_>>().map_err(failed)?;
    Ok(Row::new(columns, values))
}

/// The session's three calls, the same on each database but for how a row's values read. Each goes
/// through `Executor`'s methods, whose futures are boxed: sqlx 0.8's own `fetch_all` and `execute`
/// on a query are `async fn`s over the executor's lifetime, which a `Send` future can't hold.
macro_rules! session {
    ($db:ty, $dialect:expr, $value:expr) => {
        impl Session for Pooled<$db> {
            fn dialect(&self) -> Dialect {
                $dialect
            }
            fn rows<'a>(&'a mut self, sql: &'a str) -> Answer<'a, Vec<Row>> {
                Box::pin(async move {
                    let rows = sqlx::Executor::fetch_all(&mut *self.0, sqlx::raw_sql(text(sql))).await.map_err(failed)?;
                    rows.iter().map(|row| row_of(row, $value)).collect()
                })
            }
            fn exec<'a>(&'a mut self, sql: &'a str) -> Answer<'a, ()> {
                Box::pin(async move { sqlx::Executor::execute(&mut *self.0, sqlx::raw_sql(text(sql))).await.map(drop).map_err(failed) })
            }
            fn exec_with<'a>(&'a mut self, sql: &'a str, params: &'a [&'a str]) -> Answer<'a, ()> {
                Box::pin(async move {
                    let mut query = sqlx::query(text(sql));
                    for param in params {
                        query = query.bind(param.to_string());
                    }
                    sqlx::Executor::execute(&mut *self.0, query).await.map(drop).map_err(failed)
                })
            }
        }
    };
}

// Postgres's simple protocol sends every value as text, a boolean as `t` or `f`.
#[cfg(feature = "postgres")]
session!(sqlx::Postgres, Dialect::Postgres, |row: &sqlx::postgres::PgRow, i| sqlx::Row::try_get_unchecked::<Option<String>, _>(row, i));

#[cfg(feature = "postgres")]
impl private::Hosted for sqlx::Postgres {
    const DIALECT: Dialect = Dialect::Postgres;
    fn url(o: &sqlx::postgres::PgConnectOptions) -> String {
        format!("postgres://{}@{}:{}/{}", o.get_username(), o.get_host(), o.get_port(), o.get_database().unwrap_or_default())
    }
    fn session(conn: sqlx::pool::PoolConnection<sqlx::Postgres>) -> Box<dyn Session> {
        Box::new(Pooled(conn))
    }
}

// MySQL's text protocol sends every value as bytes, read as UTF-8 as mysql_async's are.
#[cfg(feature = "mysql")]
session!(sqlx::MySql, Dialect::Mysql, |row: &sqlx::mysql::MySqlRow, i| {
    Ok(sqlx::Row::try_get_unchecked::<Option<Vec<u8>>, _>(row, i)?.map(|b| String::from_utf8_lossy(&b).into_owned()))
});

#[cfg(feature = "mysql")]
impl private::Hosted for sqlx::MySql {
    const DIALECT: Dialect = Dialect::Mysql;
    fn url(o: &sqlx::mysql::MySqlConnectOptions) -> String {
        format!("mysql://{}@{}:{}/{}", o.get_username(), o.get_host(), o.get_port(), o.get_database().unwrap_or_default())
    }
    fn session(conn: sqlx::pool::PoolConnection<sqlx::MySql>) -> Box<dyn Session> {
        Box::new(Pooled(conn))
    }
}

// SQLite's values by their own type, each written as rusqlite's are, so that the schema's
// fingerprint a deploy stores is the one the command line reads.
#[cfg(feature = "sqlite")]
session!(sqlx::Sqlite, Dialect::Sqlite, |row: &sqlx::sqlite::SqliteRow, i| {
    use sqlx::{TypeInfo, ValueRef};
    let value = sqlx::Row::try_get_raw(row, i)?;
    if value.is_null() {
        return Ok(None);
    }
    let kind = value.type_info().name().to_string();
    Ok(Some(match kind.as_str() {
        "INTEGER" => sqlx::Row::try_get_unchecked::<i64, _>(row, i)?.to_string(),
        "REAL" => sqlx::Row::try_get_unchecked::<f64, _>(row, i)?.to_string(),
        _ => String::from_utf8_lossy(&sqlx::Row::try_get_unchecked::<Vec<u8>, _>(row, i)?).into_owned(),
    }))
});

#[cfg(feature = "sqlite")]
impl private::Hosted for sqlx::Sqlite {
    const DIALECT: Dialect = Dialect::Sqlite;
    fn url(o: &sqlx::sqlite::SqliteConnectOptions) -> String {
        format!("sqlite:{}", o.get_filename().to_string_lossy().replace('%', "%25"))
    }
    fn session(conn: sqlx::pool::PoolConnection<sqlx::Sqlite>) -> Box<dyn Session> {
        Box::new(Pooled(conn))
    }
}

/// What `reylo migrate status` found: each migration with whether the database ran it, and the
/// problems it checks for beside them. `Display` writes it as the command prints it.
#[derive(Clone, Debug)]
pub struct Status {
    migrations: Vec<MigrationStatus>,
    problems: Vec<Problem>,
    lines: Vec<String>,
    current: bool,
}

impl Status {
    /// Whether the database has run every migration the build holds, none of them edited or
    /// deleted since, with nothing else out of step.
    pub fn is_current(&self) -> bool {
        self.current
    }
    /// Each migration the database ran, in the order it ran them, then each it hasn't.
    pub fn migrations(&self) -> &[MigrationStatus] {
        &self.migrations
    }
    /// The names of the migrations the database hasn't run.
    pub fn pending(&self) -> impl Iterator<Item = &str> {
        self.migrations.iter().filter(|m| m.state == State::Pending).map(|m| m.name.as_str())
    }
    /// What would stop a deploy, each with its fix.
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.lines.join("\n"))
    }
}

/// A migration, as `reylo migrate status` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MigrationStatus {
    pub name: String,
    pub state: State,
    /// When the database ran it, as the history table holds it, in UTC.
    pub applied_at: Option<String>,
}

/// Whether the database ran a migration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum State {
    /// It ran, and the build holds it as it ran.
    Applied,
    /// It hasn't run.
    Pending,
    /// It ran, and the build's text of it isn't the text that ran.
    Edited,
    /// It ran, and the build doesn't hold it.
    Missing,
    /// Its run was cut off before it finished: `reylo migrate deploy --dry-run` says whether it
    /// landed.
    CutOff,
}

/// One reason `reylo migrate` refuses, with what fixes it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Problem {
    /// What kind of problem it is, as `reylo migrate --json` names it: `migration-draft`,
    /// `applied-migration-edited`.
    pub code: String,
    pub title: String,
    pub detail: Vec<String>,
    pub notes: Vec<String>,
    pub fixes: Vec<Fix>,
}

/// A way to fix a problem: what it does, and the commands that do it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Fix {
    pub what: String,
    pub commands: Vec<String>,
}

fn problems(values: &[serde_json::Value]) -> Vec<Problem> {
    let texts = |v: &serde_json::Value| v.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default();
    values
        .iter()
        .map(|p| Problem {
            code: p["code"].as_str().unwrap_or_default().to_string(),
            title: p["title"].as_str().unwrap_or_default().to_string(),
            detail: texts(&p["detail"]),
            notes: texts(&p["notes"]),
            fixes: p["fixes"].as_array().map(|fixes| fixes.iter().map(|f| Fix { what: f["what"].as_str().unwrap_or_default().to_string(), commands: texts(&f["commands"]) }).collect()).unwrap_or_default(),
        })
        .collect()
}

/// Why `reylo::migrate!()` ran nothing, or stopped: a refusal, a migration that failed, or the
/// database out of reach. `Display` says it as the command line prints it.
#[derive(Clone, Debug)]
pub struct Error {
    message: String,
    problems: Vec<Problem>,
    applied: Vec<String>,
}

impl Error {
    fn new(message: String) -> Error {
        Error { message, problems: Vec::new(), applied: Vec::new() }
    }
    fn of(e: reylo_migrate::Error) -> Error {
        Error::new(format!("reylo::migrate!(): {e}"))
    }
    fn stopped(what: &str, run: &reylo_migrate::commands::HostRun, applied: Vec<String>) -> Error {
        let lines: Vec<&str> = run.lines.iter().map(String::as_str).skip_while(|l| l.is_empty()).collect();
        Error { message: format!("{what}:\n{}", lines.join("\n")), problems: problems(&run.problems), applied }
    }
    /// Each reason it refused, with its fix; none where a migration failed or the database was out
    /// of reach.
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }
    /// The migrations this deploy applied before one failed, in order; none where it refused.
    pub fn applied(&self) -> &[String] {
        &self.applied
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}
