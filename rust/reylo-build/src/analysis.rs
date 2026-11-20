//! A statement's analysis at compile time: from `reylo/.cache/` when its entry matches the database's
//! schema, else from the engine replayed against the database, else, where the database cannot be
//! reached or no URL names it, from the newest entry `reylo/.cache/` holds for the statement. The
//! connection is the engine's own (`reylo_engine`), which reads the URL as the loader does,
//! TLS included. A statement built from `reylo/.cache/` without the database's schema to hold it to
//! says so on standard error, once per process and reason, in the loader's words.
//!
//! The cache is the TypeScript loader's (`src/analyze.ts`): the same file names, the same entry and
//! the same schema fingerprint, so a crate and a package in one directory would share it.
//!
//! Each database is behind the feature of its name, which brings its engine and its client, so a
//! crate that runs on one database builds neither of the others. With none on, nothing is read, and
//! the module still builds, for a crate's check.
#![cfg_attr(not(any(feature = "postgres", feature = "mysql", feature = "sqlite")), allow(dead_code, unused_imports, unreachable_code))]

use crate::Dialect;
use reylo_engine::AnswerCache;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
#[cfg(any(feature = "postgres", feature = "mysql"))]
use std::sync::{MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(any(feature = "postgres", feature = "mysql"))]
use reylo_engine::Failure;
#[cfg(feature = "postgres")]
use tokio_postgres::Client;

const ENTRY_VERSION: u64 = 1;
#[cfg(any(feature = "postgres", feature = "mysql"))]
/// How long a schema fingerprint is trusted, as the loader trusts one (`SCHEMA_TTL` in
/// `src/analyze.ts`): the statements of one compile are checked against one fingerprint, and a
/// migration shows up in an editor's next expansion within seconds.
const SCHEMA_TTL: Duration = Duration::from_secs(5);
#[cfg(any(feature = "postgres", feature = "mysql"))]
/// Once the database is found unreachable, statements are read from `reylo/.cache/` for a minute
/// instead of each paying the connect timeout (`OFFLINE_TTL` in `src/analyze.ts`).
const OFFLINE_TTL: Duration = Duration::from_secs(60);

/// A database's connection and what was learned over it, shared by every statement the macro
/// expands in one process: a compiler's run over a crate, or an editor's macro server for as long
/// as it runs. A cached statement then costs a file read, not a connection and a fingerprint.
#[cfg(any(feature = "postgres", feature = "mysql"))]
struct Session<C, D> {
    url: String,
    conn: Option<C>,
    /// The schema's fingerprint, or why the database could not be reached, and when.
    schema: Option<(Result<String, String>, Instant)>,
    answers: AnswerCache<D>,
}

#[cfg(any(feature = "postgres", feature = "mysql"))]
impl<C, D> Session<C, D> {
    fn fresh(&self) -> Option<Result<String, String>> {
        let (schema, at) = self.schema.as_ref()?;
        (at.elapsed() < if schema.is_ok() { SCHEMA_TTL } else { OFFLINE_TTL }).then(|| schema.clone())
    }

    fn learned(&mut self, schema: Result<String, String>) {
        // The engine's answers describe the old schema too.
        if let (Some((Ok(old), _)), Ok(new)) = (&self.schema, &schema) {
            if old != new {
                self.answers.clear();
            }
        }
        if schema.is_err() {
            self.conn = None;
        }
        self.schema = Some((schema, Instant::now()));
    }
}

#[cfg(feature = "postgres")]
static POSTGRES: Mutex<Option<Session<Client, reylo_engine::clients::tokio_postgres::Describe>>> = Mutex::new(None);
#[cfg(feature = "mysql")]
static MYSQL: Mutex<Option<Session<mysql_async::Conn, reylo_engine::clients::mysql_async::Describe>>> = Mutex::new(None);
#[cfg(any(feature = "postgres", feature = "mysql"))]
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// The runtime the shared connections live on: its worker keeps a connection's task running
/// between expansions.
#[cfg(any(feature = "postgres", feature = "mysql"))]
fn runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    if RUNTIME.get().is_none() {
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().map_err(|e| e.to_string())?;
        let _ = RUNTIME.set(runtime);
    }
    Ok(RUNTIME.get().expect("set above"))
}

/// The session for `url`, a new one where the last expansion named another database.
#[cfg(any(feature = "postgres", feature = "mysql"))]
fn session<C, D>(cell: &'static Mutex<Option<Session<C, D>>>, url: &str) -> MutexGuard<'static, Option<Session<C, D>>> {
    let mut guard = cell.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.as_ref().map_or(true, |s| s.url != url) {
        *guard = Some(Session { url: url.to_string(), conn: None, schema: None, answers: AnswerCache::default() });
    }
    guard
}
/// The schema fingerprints of Postgres and MySQL: the loader's own queries (`FINGERPRINT` in
/// `src/analyze.ts`), as its definitions write them out, so that an entry either writes is current
/// for both.
#[cfg(feature = "postgres")]
const POSTGRES_FINGERPRINT: &str = include_str!("fingerprint-postgres.sql");
#[cfg(feature = "mysql")]
const MYSQL_FINGERPRINT: &str = include_str!("fingerprint-mysql.sql");
/// SQLite's whole schema, and the functions of the SQLite that opened it, as the loader reads them.
#[cfg(feature = "sqlite")]
const SQLITE_FINGERPRINT: &str = include_str!("fingerprint-sqlite.sql");

impl Dialect {
    fn name(self) -> &'static str {
        match self {
            Dialect::Postgres => "postgres",
            Dialect::Sqlite => "sqlite",
            Dialect::Mysql => "mysql",
        }
    }

    /// The engine's fingerprint, where the crate builds the database's engine.
    fn fingerprint(self) -> Option<&'static str> {
        match self {
            #[cfg(feature = "postgres")]
            Dialect::Postgres => Some(reylo_engine::engines::postgres::FINGERPRINT),
            #[cfg(feature = "sqlite")]
            Dialect::Sqlite => Some(reylo_engine::engines::sqlite::FINGERPRINT),
            #[cfg(feature = "mysql")]
            Dialect::Mysql => Some(reylo_engine::engines::mysql::FINGERPRINT),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// The build's error for a database whose feature is off.
    fn off(self) -> String {
        format!("the database URL names {}, and reylo's {} feature is off: turn it on in Cargo.toml, as `reylo = {{ version = \"…\", features = [\"{}\"] }}`", self.title(), self.name(), self.name())
    }

    fn title(self) -> &'static str {
        match self {
            Dialect::Postgres => "Postgres",
            Dialect::Sqlite => "SQLite",
            Dialect::Mysql => "MySQL",
        }
    }

    /// The database the URL's scheme names.
    fn of(url: &str) -> Result<Dialect, String> {
        let scheme = url.split(':').next().unwrap_or_default().to_ascii_lowercase();
        match scheme.as_str() {
            "postgres" | "postgresql" => Ok(Dialect::Postgres),
            "sqlite" | "file" => Ok(Dialect::Sqlite),
            "mysql" => Ok(Dialect::Mysql),
            "clickhouse+http" | "clickhouse+https" => Err("the Rust crate runs its queries on sqlx, which has no ClickHouse driver: type ClickHouse's statements with the npm package `reylo`".into()),
            other => Err(format!("cannot tell the database from the URL's scheme {other:?}: reylo reads postgres://, postgresql://, mysql://, file: and sqlite: URLs")),
        }
    }
}

/// The analysis of `sql` and the database it is for, or the message a build shows for it. The
/// project is the one `dir` is in: the crate's directory for the macro and a build script, the
/// `.sql` file's for the generator.
pub fn analyze(dir: &Path, sql: &str) -> Result<(Dialect, Value), String> {
    let located = locate(dir)?;
    let queries = located.cache.join("queries");
    let Some(url) = located.url.clone() else {
        // No URL: whichever database the cache holds an analysis of the statement for.
        for dialect in [Dialect::Postgres, Dialect::Sqlite, Dialect::Mysql] {
            let Some(key) = statement_key(dialect, sql) else { continue };
            if let Some(entry) = newest_entry(&queries, &key, "") {
                unverified(&located, "no database URL is set", &entry);
                return Ok((dialect, entry["analysis"].clone()));
            }
        }
        return Err(located.missing);
    };
    let dialect = Dialect::of(&url)?;
    let analysis = match dialect {
        #[cfg(feature = "postgres")]
        Dialect::Postgres => runtime()?.block_on(async {
            let mut guard = session(&POSTGRES, &url);
            postgres(&located, &queries, sql, guard.as_mut().expect("made by session")).await
        })?,
        #[cfg(feature = "sqlite")]
        Dialect::Sqlite => sqlite(&located, &queries, &url, sql)?,
        #[cfg(feature = "mysql")]
        Dialect::Mysql => runtime()?.block_on(async {
            let mut guard = session(&MYSQL, &url);
            mysql(&located, &queries, sql, guard.as_mut().expect("made by session")).await
        })?,
        #[allow(unreachable_patterns)]
        off => return Err(off.off()),
    };
    Ok((dialect, analysis))
}

/// How long a project's database URL is kept: a `direct_url` that runs a command (a cloud
/// provider's short-lived token) runs once in that while, not once per statement.
const URL_TTL: Duration = Duration::from_secs(300);

/// A crate's project, as the CLI and the loader read it.
#[derive(Clone)]
struct Located {
    /// The directory a relative path resolves against: the project file's, else the crate's.
    root: PathBuf,
    /// `.cache/` in the project's `reylo/` directory (the project file's `dir` names another).
    cache: PathBuf,
    /// `direct_url`, else `DATABASE_URL`, from the environment or the `.env` files.
    url: Option<String>,
    /// Where the URL came from, for a message: `"direct_url" in .reylo.yaml, DB_PASSWORD in .env`.
    source: String,
    /// What to say where no URL is set.
    missing: String,
}

static LOCATED: Mutex<Vec<(PathBuf, Instant, Located)>> = Mutex::new(Vec::new());

/// The project of the crate at `manifest`: `.reylo.yaml` where the crate has one above it, its
/// `direct_url`, the `.env` files and `DATABASE_URL`, resolved as `reylo analyze` resolves them
/// (`reylo_engine::project`).
fn locate(manifest: &Path) -> Result<Located, String> {
    let mut held = LOCATED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, _, located)) = held.iter().find(|(m, at, _)| m == manifest && at.elapsed() < URL_TTL) {
        return Ok(located.clone());
    }
    let project = reylo_engine::project::Project::resolve(manifest, std::env::vars().collect())?;
    let (url, source) = project.type_check_url()?.map_or((None, String::new()), |(url, source)| (Some(url), source));
    let cache = project.root.join(project.config.dir.as_deref().unwrap_or("reylo")).join(".cache");
    let located = Located { root: project.root.clone(), cache, url, source, missing: project.missing_url() };
    held.retain(|(m, _, _)| m != manifest);
    held.push((manifest.to_path_buf(), Instant::now(), located.clone()));
    Ok(located)
}

/// The project of the crate at `dir`, as `reylo migrate` resolves it: its directory, and the
/// directory of its own files (`reylo`, or the one the project file's `dir` names), where its
/// migrations, lock and `schema.sql` are.
pub fn home(dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    let project = reylo_engine::project::Project::resolve(dir, std::env::vars().collect())?;
    let home = home_of(&project);
    Ok((project.root, home))
}

fn home_of(project: &reylo_engine::project::Project) -> PathBuf {
    project.root.join(project.config.dir.as_deref().unwrap_or("reylo"))
}

/// Why a project whose files are in `home` (`shown` as its messages write it) can't have its
/// migrations read: it holds the lock under its name before 0.8.0, `reylo-lock.yaml`, alone or
/// beside `reylo.lock`, which `reylo migrate` refuses too. Read as no lock, it would run the
/// migrations in name order.
pub fn old_lock(home: &Path, shown: &str) -> Option<String> {
    if !home.join("reylo-lock.yaml").is_file() {
        return None;
    }
    Some(if home.join("reylo.lock").is_file() {
        format!("Both {shown}/reylo-lock.yaml and {shown}/reylo.lock are here: {shown}/reylo.lock is the lock, and {shown}/reylo-lock.yaml its name before 0.8.0, so keep {shown}/reylo.lock and delete the other, with git rm {shown}/reylo-lock.yaml")
    } else {
        format!("The lock's old name is {shown}/reylo-lock.yaml: rename it to {shown}/reylo.lock, with git mv {shown}/reylo-lock.yaml {shown}/reylo.lock")
    })
}

/// The files a statement's analysis depends on beside the statement, which a build script names
/// to Cargo so that a change to one runs it again: the project file, the `.env` files, and the
/// schema and migrations in the project's `reylo/` directory, each where it exists.
pub fn watched(dir: &Path) -> Vec<PathBuf> {
    let Ok(project) = reylo_engine::project::Project::resolve(dir, std::env::vars().collect()) else { return Vec::new() };
    let home = home_of(&project);
    let own = [home.join("schema.sql"), home.join("migrations"), home.join("reylo.lock")];
    project.file.iter().cloned().chain(project.env_files().iter().cloned()).chain(own).filter(|p| p.exists()).collect()
}

#[cfg(feature = "postgres")]
async fn postgres(located: &Located, queries: &Path, sql: &str, session: &mut Session<Client, reylo_engine::clients::tokio_postgres::Describe>) -> Result<Value, String> {
    let statement = statement_key(Dialect::Postgres, sql).expect("the feature builds the engine");
    let server = server(&session.url);
    let schema = match session.fresh() {
        Some(schema) => schema,
        None => {
            let schema = match postgres_client(&mut session.conn, &session.url).await {
                Ok(client) => {
                    let rows = client.query(POSTGRES_FINGERPRINT, &[]).await.map_err(|e| format!("the schema fingerprint failed: {}", e.as_db_error().map_or_else(|| e.to_string(), |d| d.message().to_string())))?;
                    Ok(fingerprint(&rows.iter().map(|row| vec![row.get::<_, Option<String>>(0).map_or(Value::Null, Value::from)]).collect::<Vec<_>>()))
                }
                // Out of reach: the cache answers. A refused login or a URL's bad TLS settings is the build's error.
                Err(Failure::Unreachable(why)) => Err(why),
                Err(other) => return Err(other.to_string()),
            };
            session.learned(schema.clone());
            schema
        }
    };
    // Out of reach: the entries are used as they are.
    let schema = match schema {
        Ok(schema) => schema,
        Err(why) => return offline(located, queries, &statement, &server, &located.cannot_connect(&session.url, &why)),
    };
    let own = queries.join(format!("{statement}-{}.json", &sha256(&server)[..16]));
    if let Some(entry) = current(&own, &schema) {
        return Ok(entry);
    }
    let client = match postgres_client(&mut session.conn, &session.url).await {
        Ok(client) => client,
        Err(Failure::Unreachable(why)) => return offline(located, queries, &statement, &server, &located.cannot_connect(&session.url, &why)),
        Err(other) => return Err(other.to_string()),
    };
    let analysis = reylo_engine::clients::tokio_postgres::analyze(client, sql, &mut session.answers).await.map_err(|e| e.to_string())?;
    remember(&located.cache, &own, Dialect::Postgres, &schema, &analysis)
}

/// The session's client, connected again where the server closed it.
#[cfg(feature = "postgres")]
async fn postgres_client<'c>(conn: &'c mut Option<Client>, url: &str) -> Result<&'c Client, Failure> {
    if conn.as_ref().map_or(true, Client::is_closed) {
        *conn = Some(reylo_engine::postgres::open(url).await?.0);
    }
    Ok(conn.as_ref().expect("connected above"))
}

#[cfg(feature = "mysql")]
async fn mysql(located: &Located, queries: &Path, sql: &str, session: &mut Session<mysql_async::Conn, reylo_engine::clients::mysql_async::Describe>) -> Result<Value, String> {
    let statement = statement_key(Dialect::Mysql, sql).expect("the feature builds the engine");
    let server = server(&session.url);
    let schema = match session.fresh() {
        Some(schema) => schema,
        None => {
            // A kept connection the server has since dropped fails the fingerprint: once more on a new one.
            let kept = session.conn.is_some();
            let mut rows = mysql_fingerprint(&mut session.conn, &session.url).await;
            if kept && matches!(rows, Ok(Err(_))) {
                session.conn = None;
                rows = mysql_fingerprint(&mut session.conn, &session.url).await;
            }
            let schema = match rows {
                Ok(Ok(rows)) => Ok(fingerprint(&rows)),
                Ok(Err(e)) => return Err(format!("the schema fingerprint failed: {e}")),
                Err(Failure::Unreachable(why)) => Err(why),
                Err(other) => return Err(other.to_string()),
            };
            session.learned(schema.clone());
            schema
        }
    };
    let schema = match schema {
        Ok(schema) => schema,
        Err(why) => return offline(located, queries, &statement, &server, &located.cannot_connect(&session.url, &why)),
    };
    let own = queries.join(format!("{statement}-{}.json", &sha256(&server)[..16]));
    if let Some(entry) = current(&own, &schema) {
        return Ok(entry);
    }
    let conn = match mysql_conn(&mut session.conn, &session.url).await {
        Ok(conn) => conn,
        Err(Failure::Unreachable(why)) => return offline(located, queries, &statement, &server, &located.cannot_connect(&session.url, &why)),
        Err(other) => return Err(other.to_string()),
    };
    let analysis = reylo_engine::clients::mysql_async::analyze(conn, sql, &mut session.answers).await;
    if analysis.is_err() {
        // The connection may be what failed: the next statement opens a new one.
        session.conn = None;
    }
    remember(&located.cache, &own, Dialect::Mysql, &schema, &analysis.map_err(|e| e.to_string())?)
}

/// The fingerprint's rows as the loader reads them, each value as mysql2 decodes it; the outer
/// error is a database that cannot be reached.
#[cfg(feature = "mysql")]
async fn mysql_fingerprint(conn: &mut Option<mysql_async::Conn>, url: &str) -> Result<Result<Vec<Vec<Value>>, mysql_async::Error>, Failure> {
    use mysql_async::prelude::Queryable;
    let conn = mysql_conn(conn, url).await?;
    Ok(async {
        let mut result = conn.query_iter(MYSQL_FINGERPRINT).await?;
        let types: Vec<u8> = result.columns_ref().iter().map(|c| c.column_type() as u8).collect();
        let mut rows = Vec::new();
        while let Some(row) = result.next().await? {
            rows.push(row.unwrap().into_iter().zip(&types).map(|(value, code)| reylo_engine::clients::mysql_async::decoded(value, *code)).collect::<Vec<Value>>());
        }
        Ok(rows)
    }
    .await)
}

/// The session's connection, opened where there is none.
#[cfg(feature = "mysql")]
async fn mysql_conn<'c>(conn: &'c mut Option<mysql_async::Conn>, url: &str) -> Result<&'c mut mysql_async::Conn, Failure> {
    if conn.is_none() {
        *conn = Some(reylo_engine::mysql::open(url).await?.0);
    }
    Ok(conn.as_mut().expect("connected above"))
}

#[cfg(feature = "sqlite")]
fn sqlite(located: &Located, queries: &Path, url: &str, sql: &str) -> Result<Value, String> {
    let statement = statement_key(Dialect::Sqlite, sql).expect("the feature builds the engine");
    let path = PathBuf::from(reylo_engine::project::sqlite_path(url, &located.root).map_err(|why| format!("Cannot read the SQLite URL {url}: {why}."))?);
    let server = path.to_string_lossy().to_string();
    // A file that is gone is a database that cannot be reached: the entries are used as they are.
    if !path.exists() {
        return offline(located, queries, &statement, &server, &located.cannot_connect(url, "it does not exist"));
    }
    let conn = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let rows = {
        let mut stmt = conn.prepare(SQLITE_FINGERPRINT).map_err(|e| format!("the schema fingerprint failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((0..4)
                    .map(|i| match row.get_ref(i) {
                        Ok(rusqlite::types::ValueRef::Integer(n)) => json!(n),
                        Ok(rusqlite::types::ValueRef::Text(t)) => json!(String::from_utf8_lossy(t)),
                        _ => Value::Null,
                    })
                    .collect::<Vec<Value>>())
            })
            .map_err(|e| format!("the schema fingerprint failed: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("the schema fingerprint failed: {e}"))?;
        rows
    };
    let schema = fingerprint(&rows);
    let own = queries.join(format!("{statement}-{}.json", &sha256(&server)[..16]));
    if let Some(entry) = current(&own, &schema) {
        return Ok(entry);
    }
    let analysis = reylo_engine::clients::rusqlite::analyze(&conn, sql, &mut AnswerCache::default()).map_err(|e| e.to_string())?;
    remember(&located.cache, &own, Dialect::Sqlite, &schema, &analysis)
}

/// Where the database cannot be reached (`found` says which, and why): this server's entry for the
/// statement, else the newest.
fn offline(located: &Located, queries: &Path, statement: &str, server: &str, found: &str) -> Result<Value, String> {
    let entry = newest_entry(queries, statement, server).ok_or_else(|| format!("{}{}; {} has no analysis of this statement, so build once where the database can be reached.", found[..1].to_uppercase(), &found[1..], located.cache_name()))?;
    unverified(located, found, &entry);
    Ok(entry["analysis"].clone())
}

impl Located {
    /// The cache as a message names it, from the project's root: `reylo/.cache`.
    fn cache_name(&self) -> String {
        self.cache.strip_prefix(&self.root).map_or_else(|_| self.cache.display().to_string(), |p| p.to_string_lossy().replace('\\', "/"))
    }

    /// Where the database was looked for, and where its URL came from; never the credentials.
    fn cannot_connect(&self, url: &str, why: &str) -> String {
        match Dialect::of(url) {
            Ok(Dialect::Sqlite) => format!("cannot open the database {} ({}): {why}", reylo_engine::project::sqlite_path(url, &self.root).unwrap_or_else(|_| url.to_string()), self.source),
            _ => format!("cannot connect to the database at {} ({}): {why}", place(url), self.source),
        }
    }
}

/// The database as a person names it: its host, port and database.
fn place(url: &str) -> String {
    let Some((_, rest)) = url.split_once("://") else { return "the database".to_string() };
    let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (authority, path) = rest.find('/').map_or((rest, ""), |at| rest.split_at(at));
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = if host.is_empty() { query.split('&').find_map(|p| p.strip_prefix("host=")).unwrap_or("localhost") } else { host };
    format!("{host}{path}")
}

/// Says that a statement was built from `entry` in the cache without the database's schema to hold
/// it to, once per process and reason, as the loader says it (`warnUnverified` in `src/loader.ts`):
/// on standard error, or as a warning Cargo shows where a build script asked, whose standard error
/// Cargo keeps to itself.
fn unverified(located: &Located, found: &str, entry: &Value) {
    static SAID: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let mut said = SAID.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if said.iter().any(|s| s == found) {
        return;
    }
    said.push(found.to_string());
    let line = unverified_line(found, &located.cache_name(), entry);
    // Cargo sets HOST and TARGET for a build script alone.
    if std::env::var_os("HOST").is_some() && std::env::var_os("TARGET").is_some() {
        println!("cargo:warning={line}");
    } else {
        eprintln!("{line}");
    }
}

fn unverified_line(found: &str, cache: &str, entry: &Value) -> String {
    let day = entry["at"].as_u64().map_or_else(String::new, |at| format!(", analyzed {}", utc_day(at)));
    format!("reylo: {found}; building from {cache}/{day}")
}

/// The UTC day of a time in milliseconds since 1970, as `YYYY-MM-DD` (the proleptic Gregorian
/// calendar's, by days from the epoch).
fn utc_day(ms: u64) -> String {
    let z = (ms / 86_400_000) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The entry's analysis, where it was made against the schema the database has now.
fn current(file: &Path, schema: &str) -> Option<Value> {
    let entry = read_entry(file)?;
    (entry["schema"].as_str() == Some(schema)).then(|| entry["analysis"].clone())
}

fn remember(cache: &Path, file: &Path, dialect: Dialect, schema: &str, analysis: &reylo_engine::Analysis) -> Result<Value, String> {
    let analysis = serde_json::to_value(analysis).map_err(|e| e.to_string())?;
    let at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    write_entry(cache, file, &json!({ "version": ENTRY_VERSION, "dialect": dialect.name(), "schema": schema, "analysis": analysis, "at": at }));
    Ok(analysis)
}

/// The first half of an entry's file name, as `src/analyze.ts` hashes it: the engine's hash of
/// its sources is in it, so an entry another engine wrote is not read.
fn statement_key(dialect: Dialect, sql: &str) -> Option<String> {
    Some(sha256(&json!([ENTRY_VERSION, dialect.fingerprint()?, dialect.name(), sql]).to_string())[..32].to_string())
}

/// The server, without credentials: the scheme, the user, the host and port, and the database.
#[cfg(any(feature = "postgres", feature = "mysql"))]
fn server(url: &str) -> String {
    let (scheme, rest) = url.split_once("://").unwrap_or(("", url));
    let (authority, path) = rest.split_once('/').map_or((rest, String::new()), |(a, p)| (a, format!("/{}", p.split('?').next().unwrap_or(""))));
    let (user, host) = authority.rsplit_once('@').map_or(("", authority), |(u, h)| (u.split(':').next().unwrap_or(""), h));
    format!("{scheme}://{user}@{host}{path}")
}

fn sha256(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// The schema's fingerprint from the fingerprint query's rows, as the loader hashes them.
fn fingerprint(rows: &[Vec<Value>]) -> String {
    sha256(&serde_json::to_string(rows).unwrap_or_default())[..16].to_string()
}

fn read_entry(file: &Path) -> Option<Value> {
    let entry: Value = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
    (entry["version"].as_u64() == Some(ENTRY_VERSION)).then_some(entry)
}

/// This server's entry for the statement, else the one written last for it by any server.
fn newest_entry(queries: &Path, statement: &str, server_name: &str) -> Option<Value> {
    if !server_name.is_empty() {
        if let Some(entry) = read_entry(&queries.join(format!("{statement}-{}.json", &sha256(server_name)[..16]))) {
            return Some(entry);
        }
    }
    let prefix = format!("{statement}-");
    fs::read_dir(queries)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .filter_map(|e| read_entry(&e.path()))
        .max_by_key(|entry| entry["at"].as_u64().unwrap_or(0))
}

fn write_entry(cache: &Path, file: &Path, entry: &Value) {
    let dir = cache;
    let tmp = file.with_extension(format!("json.{}.tmp", std::process::id()));
    let written = (|| -> std::io::Result<()> {
        fs::create_dir_all(dir.join("queries"))?;
        let ignore = dir.join(".gitignore");
        if !ignore.exists() {
            fs::write(&ignore, "# reylo's cache: written by the type check, the build steps and the migrations; never committed.\n*\n")?;
        }
        fs::write(&tmp, format!("{entry}\n"))?;
        fs::rename(&tmp, file)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

#[cfg(test)]
mod tests {
    use super::{old_lock, place, unverified_line, utc_day};
    use serde_json::json;

    #[test]
    fn a_build_from_the_cache_says_so_in_the_loaders_words() {
        assert_eq!(utc_day(0), "1970-01-01");
        assert_eq!(utc_day(951_782_400_000), "2000-02-29");
        assert_eq!(utc_day(1_779_148_799_999), "2026-05-18");
        let entry = json!({"at": 1_779_148_799_999u64, "analysis": {}});
        assert_eq!(unverified_line("no database URL is set", "reylo/.cache", &entry), "reylo: no database URL is set; building from reylo/.cache/, analyzed 2026-05-18");
        assert_eq!(unverified_line("no database URL is set", "reylo/.cache", &json!({"analysis": {}})), "reylo: no database URL is set; building from reylo/.cache/");
        // Where, never the credentials.
        assert_eq!(place("postgres://app:secret@db.internal:5432/shop?sslmode=require"), "db.internal:5432/shop");
        assert_eq!(place("mysql://root@127.0.0.1:3306"), "127.0.0.1:3306");
        assert_eq!(place("postgres:///shop?host=/var/run/postgresql"), "/var/run/postgresql/shop");
        assert_eq!(place("postgres:///shop"), "localhost/shop");
    }

    #[test]
    fn the_lock_under_its_old_name_is_refused_alone_or_beside_the_lock() {
        let home = std::env::temp_dir().join(format!("reylo-build-old-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(old_lock(&home, "reylo"), None);
        std::fs::write(home.join("reylo.lock"), "migrations:\n").unwrap();
        assert_eq!(old_lock(&home, "reylo"), None);
        std::fs::rename(home.join("reylo.lock"), home.join("reylo-lock.yaml")).unwrap();
        assert_eq!(old_lock(&home, "reylo").as_deref(), Some("The lock's old name is reylo/reylo-lock.yaml: rename it to reylo/reylo.lock, with git mv reylo/reylo-lock.yaml reylo/reylo.lock"));
        std::fs::write(home.join("reylo.lock"), "migrations:\n").unwrap();
        assert!(old_lock(&home, "reylo").is_some_and(|e| e.starts_with("Both reylo/reylo-lock.yaml and reylo/reylo.lock are here: ") && e.ends_with("with git rm reylo/reylo-lock.yaml")));
        std::fs::remove_dir_all(&home).unwrap();
    }
}
