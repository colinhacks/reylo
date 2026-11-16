//! A statement's analysis at compile time: from `reylo/.cache/` when its entry matches the database's
//! schema, else from the engine replayed against the database, else, where the database cannot be
//! reached or no URL names it, from the newest entry `reylo/.cache/` holds for the statement. The
//! connection is the engine's own (`reylo_engine_native`), which reads the URL as the loader does,
//! TLS included.
//!
//! The cache is the TypeScript loader's (`src/analyze.ts`): the same file names, the same entry and
//! the same schema fingerprint, so a crate and a package in one directory would share it.

use crate::Dialect;
use reylo_engine::AnswerCache;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use reylo_engine_native::Failure;
use tokio_postgres::Client;

const ENTRY_VERSION: u64 = 1;
/// How long a schema fingerprint is trusted, as the loader trusts one (`SCHEMA_TTL` in
/// `src/analyze.ts`): the statements of one compile are checked against one fingerprint, and a
/// migration shows up in an editor's next expansion within seconds.
const SCHEMA_TTL: Duration = Duration::from_secs(5);
/// Once the database is found unreachable, statements are read from `reylo/.cache/` for a minute
/// instead of each paying the connect timeout (`OFFLINE_TTL` in `src/analyze.ts`).
const OFFLINE_TTL: Duration = Duration::from_secs(60);

/// A database's connection and what was learned over it, shared by every statement the macro
/// expands in one process: a compiler's run over a crate, or an editor's macro server for as long
/// as it runs. A cached statement then costs a file read, not a connection and a fingerprint.
struct Session<C, D> {
    url: String,
    conn: Option<C>,
    /// The schema's fingerprint, or why the database could not be reached, and when.
    schema: Option<(Result<String, String>, Instant)>,
    answers: AnswerCache<D>,
}

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

static POSTGRES: Mutex<Option<Session<Client, reylo_engine::clients::tokio_postgres::Describe>>> = Mutex::new(None);
static MYSQL: Mutex<Option<Session<mysql_async::Conn, reylo_engine::clients::mysql_async::Describe>>> = Mutex::new(None);
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// The runtime the shared connections live on: its worker keeps a connection's task running
/// between expansions.
fn runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    if RUNTIME.get().is_none() {
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().map_err(|e| e.to_string())?;
        let _ = RUNTIME.set(runtime);
    }
    Ok(RUNTIME.get().expect("set above"))
}

/// The session for `url`, a new one where the last expansion named another database.
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
const POSTGRES_FINGERPRINT: &str = include_str!("fingerprint-postgres.sql");
const MYSQL_FINGERPRINT: &str = include_str!("fingerprint-mysql.sql");
/// SQLite's whole schema, and the functions of the SQLite that opened it, as the loader reads them.
const SQLITE_FINGERPRINT: &str = include_str!("fingerprint-sqlite.sql");

impl Dialect {
    fn name(self) -> &'static str {
        match self {
            Dialect::Postgres => "postgres",
            Dialect::Sqlite => "sqlite",
            Dialect::Mysql => "mysql",
        }
    }

    fn fingerprint(self) -> &'static str {
        match self {
            Dialect::Postgres => reylo_engine::postgres::FINGERPRINT,
            Dialect::Sqlite => reylo_engine::sqlite::FINGERPRINT,
            Dialect::Mysql => reylo_engine::mysql::FINGERPRINT,
        }
    }

    /// The database the URL's scheme names.
    fn of(url: &str) -> Result<Dialect, String> {
        let scheme = url.split(':').next().unwrap_or_default().to_ascii_lowercase();
        match scheme.as_str() {
            "postgres" | "postgresql" => Ok(Dialect::Postgres),
            "sqlite" | "file" => Ok(Dialect::Sqlite),
            "mysql" => Ok(Dialect::Mysql),
            other => Err(format!("cannot tell the database from the URL's scheme {other:?}: reylo reads postgres://, postgresql://, mysql://, file: and sqlite: URLs")),
        }
    }
}

/// The analysis of `sql` and the database it is for, or the message a build shows for it. The
/// project is the one `dir` is in: the crate's directory for the macro and a build script, the
/// `.sql` file's for the generator.
pub fn analyze(dir: &Path, sql: &str) -> Result<(Dialect, Value), String> {
    let Located { root, cache, url, missing } = locate(dir)?;
    let queries = cache.join("queries");
    let Some(url) = url else {
        // No URL: whichever database the cache holds an analysis of the statement for.
        for dialect in [Dialect::Postgres, Dialect::Sqlite, Dialect::Mysql] {
            if let Some(entry) = newest_entry(&queries, &statement_key(dialect, sql), "") {
                return Ok((dialect, entry));
            }
        }
        return Err(missing);
    };
    let dialect = Dialect::of(&url)?;
    let analysis = match dialect {
        Dialect::Postgres => runtime()?.block_on(async {
            let mut guard = session(&POSTGRES, &url);
            postgres(&cache, &queries, sql, guard.as_mut().expect("made by session")).await
        })?,
        Dialect::Sqlite => sqlite(&root, &cache, &queries, &url, sql)?,
        Dialect::Mysql => runtime()?.block_on(async {
            let mut guard = session(&MYSQL, &url);
            mysql(&cache, &queries, sql, guard.as_mut().expect("made by session")).await
        })?,
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
    /// What to say where no URL is set.
    missing: String,
}

static LOCATED: Mutex<Vec<(PathBuf, Instant, Located)>> = Mutex::new(Vec::new());

/// The project of the crate at `manifest`: `.reylo.yaml` where the crate has one above it, its
/// `direct_url`, the `.env` files and `DATABASE_URL`, resolved as `reylo analyze` resolves them
/// (`reylo_engine_native::project`).
fn locate(manifest: &Path) -> Result<Located, String> {
    let mut held = LOCATED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, _, located)) = held.iter().find(|(m, at, _)| m == manifest && at.elapsed() < URL_TTL) {
        return Ok(located.clone());
    }
    let project = reylo_engine_native::project::Project::resolve(manifest, std::env::vars().collect())?;
    let url = project.type_check_url()?.map(|(url, _)| url);
    let cache = project.root.join(project.config.dir.as_deref().unwrap_or("reylo")).join(".cache");
    let located = Located { root: project.root.clone(), cache, url, missing: project.missing_url() };
    held.retain(|(m, _, _)| m != manifest);
    held.push((manifest.to_path_buf(), Instant::now(), located.clone()));
    Ok(located)
}

/// The files a statement's analysis depends on beside the statement, which a build script names
/// to Cargo so that a change to one runs it again: the project file, the `.env` files, and the
/// schema and migrations in the project's `reylo/` directory, each where it exists.
pub fn watched(dir: &Path) -> Vec<PathBuf> {
    let Ok(project) = reylo_engine_native::project::Project::resolve(dir, std::env::vars().collect()) else { return Vec::new() };
    let home = project.root.join(project.config.dir.as_deref().unwrap_or("reylo"));
    let own = [home.join("schema.sql"), home.join("migrations"), home.join("reylo-lock.yaml")];
    project.file.iter().cloned().chain(project.env_files().iter().cloned()).chain(own).filter(|p| p.exists()).collect()
}

async fn postgres(cache: &Path, queries: &Path, sql: &str, session: &mut Session<Client, reylo_engine::clients::tokio_postgres::Describe>) -> Result<Value, String> {
    let statement = statement_key(Dialect::Postgres, sql);
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
        Err(why) => return offline(queries, &statement, &server, &why),
    };
    let own = queries.join(format!("{statement}-{}.json", &sha256(&server)[..16]));
    if let Some(entry) = current(&own, &schema) {
        return Ok(entry);
    }
    let client = match postgres_client(&mut session.conn, &session.url).await {
        Ok(client) => client,
        Err(Failure::Unreachable(why)) => return offline(queries, &statement, &server, &why),
        Err(other) => return Err(other.to_string()),
    };
    let analysis = reylo_engine::clients::tokio_postgres::analyze(client, sql, &mut session.answers).await.map_err(|e| e.to_string())?;
    remember(cache, &own, Dialect::Postgres, &schema, &analysis)
}

/// The session's client, connected again where the server closed it.
async fn postgres_client<'c>(conn: &'c mut Option<Client>, url: &str) -> Result<&'c Client, Failure> {
    if conn.as_ref().map_or(true, Client::is_closed) {
        *conn = Some(reylo_engine_native::postgres::open(url).await?.0);
    }
    Ok(conn.as_ref().expect("connected above"))
}

async fn mysql(cache: &Path, queries: &Path, sql: &str, session: &mut Session<mysql_async::Conn, reylo_engine::clients::mysql_async::Describe>) -> Result<Value, String> {
    let statement = statement_key(Dialect::Mysql, sql);
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
        Err(why) => return offline(queries, &statement, &server, &why),
    };
    let own = queries.join(format!("{statement}-{}.json", &sha256(&server)[..16]));
    if let Some(entry) = current(&own, &schema) {
        return Ok(entry);
    }
    let conn = match mysql_conn(&mut session.conn, &session.url).await {
        Ok(conn) => conn,
        Err(Failure::Unreachable(why)) => return offline(queries, &statement, &server, &why),
        Err(other) => return Err(other.to_string()),
    };
    let analysis = reylo_engine::clients::mysql_async::analyze(conn, sql, &mut session.answers).await;
    if analysis.is_err() {
        // The connection may be what failed: the next statement opens a new one.
        session.conn = None;
    }
    remember(cache, &own, Dialect::Mysql, &schema, &analysis.map_err(|e| e.to_string())?)
}

/// The fingerprint's rows as the loader reads them, each value as mysql2 decodes it; the outer
/// error is a database that cannot be reached.
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
async fn mysql_conn<'c>(conn: &'c mut Option<mysql_async::Conn>, url: &str) -> Result<&'c mut mysql_async::Conn, Failure> {
    if conn.is_none() {
        *conn = Some(reylo_engine_native::mysql::open(url).await?.0);
    }
    Ok(conn.as_mut().expect("connected above"))
}

fn sqlite(root: &Path, cache: &Path, queries: &Path, url: &str, sql: &str) -> Result<Value, String> {
    let statement = statement_key(Dialect::Sqlite, sql);
    let path = PathBuf::from(reylo_engine_native::project::sqlite_path(url, root).map_err(|why| format!("Cannot read the SQLite URL {url}: {why}."))?);
    let server = path.to_string_lossy().to_string();
    // A file that is gone is a database that cannot be reached: the entries are used as they are.
    if !path.exists() {
        return offline(queries, &statement, &server, &format!("{} does not exist", path.display()));
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
    remember(cache, &own, Dialect::Sqlite, &schema, &analysis)
}

/// Where the database cannot be reached: this server's entry for the statement, else the newest.
fn offline(queries: &Path, statement: &str, server: &str, why: &str) -> Result<Value, String> {
    newest_entry(queries, statement, server).ok_or_else(|| format!("Cannot reach the database ({why}); reylo/.cache/ has no analysis of this statement, so build once where the database can be reached."))
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
fn statement_key(dialect: Dialect, sql: &str) -> String {
    sha256(&json!([ENTRY_VERSION, dialect.fingerprint(), dialect.name(), sql]).to_string())[..32].to_string()
}

/// The server, without credentials: the scheme, the user, the host and port, and the database.
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
            return Some(entry["analysis"].clone());
        }
    }
    let prefix = format!("{statement}-");
    fs::read_dir(queries)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .filter_map(|e| read_entry(&e.path()))
        .max_by_key(|entry| entry["at"].as_u64().unwrap_or(0))
        .map(|entry| entry["analysis"].clone())
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
