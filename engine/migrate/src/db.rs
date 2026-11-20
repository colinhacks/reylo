//! The databases `reylo migrate` works on: the target, through the connection code the type check
//! uses (`reylo-engine`), and the shadows it builds beside it. A shadow is a scratch database
//! on the target's own server (`create database`), so it has the target's engine, version and
//! extensions; SQLite's live in memory, a libSQL server's (Turso's) among them. The connection that creates a shadow holds a session lock
//! named after it for the whole run, and the lock dies with the session, so a run that is killed
//! leaves a shadow nobody holds, which the next run drops before it builds its own.

use crate::{Dialect, Error};
#[cfg(feature = "cli")]
use mysql_async::prelude::Queryable;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

/// One row, every value as text (Postgres's booleans as `t` and `f`), as its columns name them.
#[derive(Clone, Debug, Default)]
pub struct Row {
    columns: Vec<String>,
    values: Vec<Option<String>>,
}

impl Row {
    /// A row of `values`, in the order `columns` names them; a host's [`Session`] builds its rows so.
    pub fn new(columns: Vec<String>, values: Vec<Option<String>>) -> Row {
        Row { columns, values }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        let at = self.columns.iter().position(|c| c == name)?;
        self.values[at].as_deref()
    }
    /// The value, or "" where it is null.
    pub fn text(&self, name: &str) -> &str {
        self.get(name).unwrap_or_default()
    }
    pub fn flag(&self, name: &str) -> bool {
        matches!(self.get(name), Some("t" | "true" | "1"))
    }
    /// The first column's value.
    pub fn first(&self) -> Option<&str> {
        self.values.first().and_then(|v| v.as_deref())
    }
}

pub enum Db {
    #[cfg(feature = "cli")]
    Postgres(tokio_postgres::Client),
    #[cfg(feature = "cli")]
    Mysql(mysql_async::Conn),
    #[cfg(feature = "cli")]
    Sqlite(rusqlite::Connection),
    #[cfg(feature = "cli")]
    // Boxed, as tiberius's client is larger than every other connection, and every future of a run
    // holds a `Db`.
    Sqlserver(Box<reylo_engine::sqlserver::Conn>),
    /// A libSQL server's database, Turso Cloud's among them, over HTTP.
    #[cfg(feature = "cli")]
    Libsql(crate::libsql::Remote),
    /// A database on libSQL's own engine in this process: the development file of a project whose
    /// SQL is libSQL's own, and every shadow of a libSQL database.
    #[cfg(feature = "cli")]
    LibsqlLocal(crate::libsql_local::Local),
    /// A database the host holds the connection to, as a Rust app's own pool through
    /// `reylo::migrate!()`.
    Host(Box<dyn Session>),
}

#[cfg(feature = "cli")]
pub(crate) fn sqlserver_error(e: tiberius::error::Error) -> Error {
    match e {
        tiberius::error::Error::Server(t) => Error(t.message().to_string()),
        other => Error(other.to_string()),
    }
}

/// A SQL Server value as text: a `bit` as `1` or `0`, a number in its decimal digits, binary data in
/// hexadecimal, and a date or a time as tiberius prints it.
#[cfg(feature = "cli")]
fn sqlserver_text(value: &tiberius::ColumnData<'_>) -> Option<String> {
    use tiberius::ColumnData::*;
    match value {
        U8(v) => v.map(|v| v.to_string()),
        I16(v) => v.map(|v| v.to_string()),
        I32(v) => v.map(|v| v.to_string()),
        I64(v) => v.map(|v| v.to_string()),
        F32(v) => v.map(|v| v.to_string()),
        F64(v) => v.map(|v| v.to_string()),
        Bit(v) => v.map(|v| if v { "1".into() } else { "0".into() }),
        String(v) => v.as_deref().map(str::to_string),
        Guid(v) => v.map(|v| v.to_string()),
        Binary(v) => v.as_deref().map(|b| b.iter().map(|byte| format!("{byte:02x}")).collect()),
        Numeric(v) => v.map(|v| v.to_string()),
        Xml(v) => v.as_deref().map(|x| x.to_string()),
        other => Some(format!("{other:?}")),
    }
}

/// The rows of every result set of a SQL Server batch, every value as text.
#[cfg(feature = "cli")]
async fn sqlserver_rows(client: &mut reylo_engine::sqlserver::Conn, sql: &str) -> Result<Vec<Row>, Error> {
    let results = client.simple_query(sql).await.map_err(sqlserver_error)?.into_results().await.map_err(sqlserver_error)?;
    Ok(results
        .into_iter()
        .flatten()
        .map(|row| {
            let columns = row.columns().iter().map(|c| c.name().to_string()).collect();
            let values = row.into_iter().map(|v| sqlserver_text(&v)).collect();
            Row { columns, values }
        })
        .collect())
}

/// A connection to the SQL Server database a URL names, as the engine opens one.
#[cfg(feature = "cli")]
pub(crate) async fn open_sqlserver(url: &str) -> Result<reylo_engine::sqlserver::Conn, Error> {
    reylo_engine::sqlserver::open(url).await.map(|(client, _)| client).map_err(|failure| Error(failure.to_string()))
}

/// A SQL Server name in brackets, `]` doubled.
#[cfg(feature = "cli")]
pub(crate) fn bracket(name: &str) -> String {
    format!("[{}]", name.replace(']', "]]"))
}

/// A future a [`Session`] answers with.
pub type Answer<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

/// One connection a host holds to the database deploy and status run on: each statement run as
/// written, every value of a row as text (a boolean as `t` or `true`), as [`Db::rows`] reads them.
/// The connection is the same one for the whole run, since the run's lock is the session's.
pub trait Session: Send {
    fn dialect(&self) -> Dialect;
    /// The rows of the statements in `sql`.
    fn rows<'a>(&'a mut self, sql: &'a str) -> Answer<'a, Vec<Row>>;
    /// Runs `sql`: one statement or several.
    fn exec<'a>(&'a mut self, sql: &'a str) -> Answer<'a, ()>;
    /// Runs one statement with text parameters, `$1` on Postgres and `?` elsewhere.
    fn exec_with<'a>(&'a mut self, sql: &'a str, params: &'a [&'a str]) -> Answer<'a, ()>;
}

/// How a script failed: waiting for a lock past its limit, having changed nothing, or otherwise.
#[derive(Debug)]
pub(crate) enum Ran {
    Waited(Error),
    Failed(Error),
}

impl From<Error> for Ran {
    fn from(e: Error) -> Ran {
        Ran::Failed(e)
    }
}

#[cfg(feature = "cli")]
pub(crate) fn pg_error(e: tokio_postgres::Error) -> Error {
    match e.as_db_error() {
        Some(d) => {
            let mut message = d.message().to_string();
            if let Some(detail) = d.detail() {
                message = format!("{message} ({detail})");
            }
            // The server's hint is often the way out, which a platform's own refusals give too.
            if let Some(hint) = d.hint() {
                message = format!("{message}. {hint}");
            }
            Error(message)
        }
        None => Error(e.to_string()),
    }
}

#[cfg(feature = "cli")]
pub(crate) fn mysql_error(e: mysql_async::Error) -> Error {
    match e {
        mysql_async::Error::Server(s) => Error(s.message),
        other => Error(other.to_string()),
    }
}

/// `name`, a libc locale, as `known` (a server's locale names) spells it: itself, else one that differs
/// only in case and in the encoding's punctuation (`en_US.utf8` and `en_US.UTF-8`), else none.
pub(crate) fn locale_spelled(name: &str, known: &[String]) -> Option<String> {
    let plain = |n: &str| match n.to_lowercase().split_once('.') {
        Some((language, rest)) => {
            let (encoding, modifier) = rest.split_once('@').map(|(e, m)| (e.to_string(), format!("@{m}"))).unwrap_or((rest.to_string(), String::new()));
            format!("{language}.{}{modifier}", encoding.replace(['-', '_'], ""))
        }
        None => n.to_lowercase(),
    };
    if known.iter().any(|k| k == name) {
        return Some(name.to_string());
    }
    known.iter().find(|k| plain(k) == plain(name)).cloned()
}

/// A server's refusal to create a shadow database, with what to do about it: each shadow is a new
/// database, so a role that may not create one needs a server where it may (PlanetScale, Vitess,
/// and some managed roles). A role that may (`may_create`) was refused by the server itself, which
/// can limit the databases made on it by name, as Tiger Cloud's does: only another server helps.
pub(crate) fn cannot_create(e: Error, may_create: bool) -> Error {
    if may_create {
        return Error(format!("{e}. The role may create databases, but this server refuses the shadow database each check makes beside yours: set shadow_url in .reylo.yaml to a server that takes it"));
    }
    Error(format!("{e}. Each shadow database is a new database on this server: give the role the right to create databases, or set shadow_url in .reylo.yaml to a server where it has it"))
}

/// Whether a MySQL server's error for `create database` is a refusal to make one, and if so
/// whether the role may create databases elsewhere on it: ER_DBACCESS_DENIED_ERROR and
/// ER_SPECIFIC_ACCESS_DENIED_ERROR are the role's missing CREATE; Vitess refuses every
/// `create database` itself, as VT12001 under the generic error code 1105.
#[cfg(feature = "cli")]
fn mysql_create_refused(code: u16, message: &str) -> Option<bool> {
    match code {
        1044 | 1227 => Some(false),
        1105 if message.starts_with("VT12001") && message.contains("create database") => Some(true),
        _ => None,
    }
}

#[cfg(feature = "cli")]
pub(crate) fn sqlite_error(e: rusqlite::Error) -> Error {
    Error(e.to_string())
}

#[cfg(feature = "cli")]
fn mysql_text(value: &mysql_async::Value) -> Option<String> {
    use mysql_async::Value::*;
    match value {
        NULL => None,
        Bytes(b) => Some(String::from_utf8_lossy(b).into_owned()),
        Int(i) => Some(i.to_string()),
        UInt(u) => Some(u.to_string()),
        Float(f) => Some(f.to_string()),
        Double(f) => Some(f.to_string()),
        other => Some(other.as_sql(true).trim_matches('\'').to_string()),
    }
}

/// Whether the SQLite file at `path` is libSQL's already, whatever the project's files say now (a
/// branch without the migration that made it): it holds a vector index's table, or its history
/// holds a migration of libSQL's own SQL, which only libSQL's engine replays.
#[cfg(feature = "cli")]
fn libsql_file(path: &str) -> bool {
    if path == ":memory:" || !Path::new(path).exists() {
        return false;
    }
    let Ok(db) = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI) else { return false };
    let vector = db.query_row("select count(*) from sqlite_schema where name = 'libsql_vector_meta_shadow'", [], |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    vector || {
        let history = format!("select sql from {}", crate::catalog::HISTORY_TABLE);
        db.prepare(&history).and_then(|mut s| s.query_map([], |r| r.get::<_, Option<String>>(0)).map(|rows| rows.flatten().flatten().any(|sql| crate::libsql::uses_own_sql(&sql)))).unwrap_or(false)
    }
}

/// The path a SQLite URL names: `sqlite:` or `file:` before it, `:memory:` alone; a relative path
/// is the project's.
pub fn sqlite_path(url: &str, base: &Path) -> String {
    if url == ":memory:" || url == "file::memory:" {
        return ":memory:".into();
    }
    let raw = if let Some(rest) = url.strip_prefix("file://") {
        rest.to_string()
    } else {
        let lower = url.to_ascii_lowercase();
        let rest = if lower.starts_with("sqlite:") {
            &url[7..]
        } else if lower.starts_with("file:") {
            &url[5..]
        } else {
            url
        };
        rest.strip_prefix("//").unwrap_or(rest).to_string()
    };
    let raw = crate::percent_decode(&raw);
    if Path::new(&raw).is_absolute() {
        raw
    } else {
        base.join(raw).to_string_lossy().into_owned()
    }
}

/// The rows of `sql` over Postgres's simple protocol, every value as text.
#[cfg(feature = "cli")]
pub(crate) async fn pg_rows(client: &tokio_postgres::Client, sql: &str) -> Result<Vec<Row>, Error> {
    let mut out = Vec::new();
    for message in client.simple_query(sql).await.map_err(pg_error)? {
        if let tokio_postgres::SimpleQueryMessage::Row(row) = message {
            let columns = row.columns().iter().map(|c| c.name().to_string()).collect();
            let values = (0..row.len()).map(|i| row.get(i).map(String::from)).collect();
            out.push(Row { columns, values });
        }
    }
    Ok(out)
}

/// A connection to the Postgres database a URL names, as the engine opens one (its TLS, timeout and
/// failure messages), without the server version it reports.
#[cfg(feature = "cli")]
pub(crate) async fn open_postgres(url: &str) -> Result<tokio_postgres::Client, Error> {
    reylo_engine::postgres::open(url).await.map(|(client, _)| client).map_err(|failure| Error(failure.to_string()))
}

/// A connection to the MySQL database a URL names, as the engine opens one.
#[cfg(feature = "cli")]
pub(crate) async fn open_mysql(url: &str) -> Result<mysql_async::Conn, Error> {
    reylo_engine::mysql::open(url).await.map(|(conn, _)| conn).map_err(|failure| Error(failure.to_string()))
}

impl Db {
    #[cfg(feature = "cli")]
    pub async fn connect(url: &str, base: &Path) -> Result<Db, Error> {
        match Dialect::of_url(url)? {
            Dialect::Postgres => Ok(Db::Postgres(crate::db::open_postgres(url).await?)),
            Dialect::Mysql => Ok(Db::Mysql(crate::db::open_mysql(url).await?)),
            Dialect::Sqlite if crate::libsql::is_url(url) => Ok(Db::Libsql(crate::libsql::Remote::open(url)?)),
            Dialect::Sqlite => Ok(Db::Sqlite(rusqlite::Connection::open(sqlite_path(url, base)).map_err(sqlite_error)?)),
            Dialect::Sqlserver => Ok(Db::Sqlserver(Box::new(open_sqlserver(url).await?))),
        }
    }

    /// The database `opts` names, a SQLite file on libSQL's engine where the project runs there
    /// ([`crate::Options::libsql_engine`]).
    #[cfg(feature = "cli")]
    pub async fn open(opts: &crate::Options) -> Result<Db, Error> {
        if Dialect::of_url(&opts.url)? == Dialect::Sqlite && !crate::libsql::is_url(&opts.url) {
            let path = sqlite_path(&opts.url, &opts.root);
            if opts.libsql_engine() || libsql_file(&path) {
                return Ok(Db::LibsqlLocal(crate::libsql_local::Local::open(&path)?));
            }
        }
        Db::connect(&opts.url, &opts.root).await
    }

    /// Whether the database runs libSQL: a libSQL server's, or one on libSQL's engine here.
    pub fn is_libsql(&self) -> bool {
        #[cfg(feature = "cli")]
        if matches!(self, Db::Libsql(_) | Db::LibsqlLocal(_)) {
            return true;
        }
        false
    }

    pub fn dialect(&self) -> Dialect {
        match self {
            #[cfg(feature = "cli")]
            Db::Postgres(_) => Dialect::Postgres,
            #[cfg(feature = "cli")]
            Db::Mysql(_) => Dialect::Mysql,
            #[cfg(feature = "cli")]
            Db::Sqlite(_) | Db::Libsql(_) | Db::LibsqlLocal(_) => Dialect::Sqlite,
            #[cfg(feature = "cli")]
            Db::Sqlserver(_) => Dialect::Sqlserver,
            Db::Host(session) => session.dialect(),
        }
    }

    /// The rows of the statements in `sql`, run as written (Postgres's simple protocol, MySQL's text
    /// protocol), every value as text.
    pub async fn rows(&mut self, sql: &str) -> Result<Vec<Row>, Error> {
        match self {
            Db::Host(session) => session.rows(sql).await,
            #[cfg(feature = "cli")]
            Db::Postgres(client) => pg_rows(client, sql).await,
            #[cfg(feature = "cli")]
            Db::Mysql(conn) => {
                let rows: Vec<mysql_async::Row> = conn.query(sql).await.map_err(mysql_error)?;
                Ok(rows
                    .into_iter()
                    .map(|row| {
                        let columns = row.columns_ref().iter().map(|c| c.name_str().into_owned()).collect();
                        let values = (0..row.len()).map(|i| row.as_ref(i).and_then(mysql_text)).collect();
                        Row { columns, values }
                    })
                    .collect())
            }
            #[cfg(feature = "cli")]
            Db::Sqlite(conn) => {
                let mut statement = conn.prepare(sql).map_err(sqlite_error)?;
                let columns: Vec<String> = statement.column_names().into_iter().map(String::from).collect();
                let count = columns.len();
                let rows = statement
                    .query_map([], |row| {
                        (0..count)
                            .map(|i| {
                                use rusqlite::types::ValueRef::*;
                                Ok(match row.get_ref(i)? {
                                    Null => None,
                                    Integer(v) => Some(v.to_string()),
                                    Real(v) => Some(v.to_string()),
                                    Text(t) | Blob(t) => Some(String::from_utf8_lossy(t).into_owned()),
                                })
                            })
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .map_err(sqlite_error)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(sqlite_error)?;
                Ok(rows.into_iter().map(|values| Row { columns: columns.clone(), values }).collect())
            }
            #[cfg(feature = "cli")]
            Db::Sqlserver(client) => sqlserver_rows(client, sql).await,
            #[cfg(feature = "cli")]
            Db::Libsql(remote) => remote.rows(sql),
            #[cfg(feature = "cli")]
            Db::LibsqlLocal(local) => local.rows(sql),
        }
    }

    /// Runs `sql` as written: one statement or several.
    pub async fn exec(&mut self, sql: &str) -> Result<(), Error> {
        match self {
            Db::Host(session) => session.exec(sql).await,
            #[cfg(feature = "cli")]
            Db::Postgres(client) => client.batch_execute(sql).await.map_err(pg_error),
            #[cfg(feature = "cli")]
            Db::Mysql(conn) => conn.query_drop(sql).await.map_err(mysql_error),
            #[cfg(feature = "cli")]
            Db::Sqlite(conn) => conn.execute_batch(sql).map_err(sqlite_error),
            #[cfg(feature = "cli")]
            Db::Sqlserver(client) => sqlserver_rows(client, sql).await.map(|_| ()),
            #[cfg(feature = "cli")]
            Db::Libsql(remote) => remote.exec(sql),
            #[cfg(feature = "cli")]
            Db::LibsqlLocal(local) => local.exec(sql),
        }
    }

    /// Runs a script of statements: on MySQL one at a time, since a server that takes several in
    /// one query reads them as one batch; on SQL Server one batch at a time, as its `GO` lines part
    /// them, and as many times as a `GO` line's count says; elsewhere as written.
    pub async fn script(&mut self, sql: &str) -> Result<(), Error> {
        if self.dialect() == Dialect::Sqlserver {
            for (batch, count) in crate::sql::batch_runs(sql) {
                for _ in 0..count {
                    self.exec(&batch).await?;
                }
            }
            return Ok(());
        }
        if self.dialect() != Dialect::Mysql {
            return self.exec(sql).await;
        }
        for statement in crate::sql::split_statements(sql, Dialect::Mysql) {
            self.exec(&statement.text).await?;
        }
        Ok(())
    }

    /// Runs a script as [`Db::script`] does, telling a statement that gave up waiting for a lock
    /// (Postgres's `lock_timeout`, MySQL's `lock_wait_timeout`, SQL Server's `lock_timeout`) from any other failure: it changed
    /// nothing, so the file can run again. A host's session can't tell, and fails as `script` does.
    pub(crate) async fn script_or_wait(&mut self, sql: &str) -> Result<(), Ran> {
        match self {
            #[cfg(feature = "cli")]
            Db::Postgres(client) => client.batch_execute(sql).await.map_err(|e| {
                let waited = e.code() == Some(&tokio_postgres::error::SqlState::LOCK_NOT_AVAILABLE);
                let e = pg_error(e);
                if waited {
                    Ran::Waited(e)
                } else {
                    Ran::Failed(e)
                }
            }),
            #[cfg(feature = "cli")]
            Db::Mysql(conn) => {
                for statement in crate::sql::split_statements(sql, Dialect::Mysql) {
                    if let Err(e) = conn.query_drop(&statement.text).await {
                        // ER_LOCK_WAIT_TIMEOUT: a metadata lock, or a row's, waited past its limit.
                        let waited = matches!(&e, mysql_async::Error::Server(s) if s.code == 1205);
                        let e = mysql_error(e);
                        return Err(if waited { Ran::Waited(e) } else { Ran::Failed(e) });
                    }
                }
                Ok(())
            }
            #[cfg(feature = "cli")]
            Db::Sqlserver(client) => {
                for (batch, count) in crate::sql::batch_runs(sql) {
                    for _ in 0..count {
                        let ran = async { client.simple_query(batch.as_str()).await?.into_results().await };
                        if let Err(e) = ran.await {
                            // 1222: a lock waited past `lock_timeout`.
                            let waited = matches!(&e, tiberius::error::Error::Server(t) if t.code() == 1222);
                            let e = sqlserver_error(e);
                            return Err(if waited { Ran::Waited(e) } else { Ran::Failed(e) });
                        }
                    }
                }
                Ok(())
            }
            _ => self.script(sql).await.map_err(Ran::Failed),
        }
    }

    /// Runs one statement with text parameters, `$1` on Postgres and `?` elsewhere.
    pub async fn exec_with(&mut self, sql: &str, params: &[&str]) -> Result<(), Error> {
        match self {
            Db::Host(session) => session.exec_with(sql, params).await,
            #[cfg(feature = "cli")]
            Db::Postgres(client) => {
                let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = params.iter().map(|p| p as &(dyn tokio_postgres::types::ToSql + Sync)).collect();
                client.execute(sql, &params).await.map(|_| ()).map_err(pg_error)
            }
            #[cfg(feature = "cli")]
            Db::Mysql(conn) => {
                let values: Vec<mysql_async::Value> = params.iter().map(|p| mysql_async::Value::from(*p)).collect();
                conn.exec_drop(sql, mysql_async::Params::Positional(values)).await.map_err(mysql_error)
            }
            #[cfg(feature = "cli")]
            Db::Sqlite(conn) => conn.execute(sql, rusqlite::params_from_iter(params.iter())).map(|_| ()).map_err(sqlite_error),
            #[cfg(feature = "cli")]
            Db::Sqlserver(client) => {
                let params: Vec<&dyn tiberius::ToSql> = params.iter().map(|p| p as &dyn tiberius::ToSql).collect();
                client.execute(sql, &params).await.map(|_| ()).map_err(sqlserver_error)
            }
            #[cfg(feature = "cli")]
            Db::Libsql(remote) => remote.exec_with(sql, params),
            #[cfg(feature = "cli")]
            Db::LibsqlLocal(local) => local.exec_with(sql, params),
        }
    }

    pub async fn close(self) {
        #[cfg(feature = "cli")]
        match self {
            Db::Mysql(conn) => {
                let _ = conn.disconnect().await;
            }
            Db::Sqlserver(client) => {
                let _ = client.close().await;
            }
            Db::Libsql(remote) => remote.close(),
            _ => {}
        }
    }

    /// The parameter marker for the `n`th parameter.
    pub fn param(&self, n: usize) -> String {
        match self.dialect() {
            Dialect::Postgres => format!("${n}"),
            Dialect::Sqlserver => format!("@P{n}"),
            _ => "?".into(),
        }
    }
}

pub const SHADOW_PREFIX: &str = "reylo_shadow_";

/// Where the host starts in what follows a URL's `://`: past the credentials, whose password may
/// hold a `#`. A `/` or `?` ends them, so an `@` in the path or the settings is no part of them.
fn host_start(rest: &str) -> usize {
    let head = &rest[..rest.find(['/', '?']).unwrap_or(rest.len())];
    head.rfind('@').map_or(0, |at| at + 1)
}

/// Why a run can't go through the transaction pooler a Postgres URL names, by the host its platform
/// gives one (Neon's endpoint with `-pooler`, Supabase's pooler on port 6543), with the connection
/// to use instead in `key`, the project file's setting that named it (`direct_url`, `shadow_url`). Such a pooler gives each transaction whichever server session is free, so a
/// run's session lock outlives the run on the pooler's session and stops the next run that connects
/// directly, and a setting a run makes for its session reaches the app's queries on it.
pub fn transaction_pooler(url: &str, key: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !matches!(scheme, "postgres" | "postgresql") {
        return None;
    }
    let rest = &rest[host_start(rest)..];
    let authority = rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())].to_ascii_lowercase();
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !port.contains(']') => (host, port.parse::<u16>().ok()),
        _ => (authority.as_str(), None),
    };
    let (first, domain) = host.split_once('.')?;
    let shown = shown_url(url);
    let flag = if key == "direct_url" { " or with --url" } else { "" };
    if let Some(endpoint) = first.strip_suffix("-pooler").filter(|_| domain.ends_with("neon.tech")) {
        return Some(format!(
            "{shown} is Neon's pooled connection, whose PgBouncer shares server sessions between clients, and a run of reylo migrate holds one: use the direct connection, host {endpoint}.{domain}, in {key} in .reylo.yaml{flag}"
        ));
    }
    if domain.ends_with("pooler.supabase.com") && port == Some(6543) {
        return Some(format!(
            "{shown} is Supabase's transaction pooler, which shares server sessions between clients, and a run of reylo migrate holds one: use its session pooler, port 5432 on the same host, or the direct connection, in {key} in .reylo.yaml{flag}"
        ));
    }
    None
}

/// The URL with its database replaced by `name`.
pub fn with_database(url: &str, name: &str) -> String {
    let Some(scheme_end) = url.find("://") else { return url.to_string() };
    let rest = &url[scheme_end + 3..];
    let start = host_start(rest);
    let authority_end = rest[start..].find(['/', '?', '#']).map_or(rest.len(), |at| start + at);
    let (authority, tail) = rest.split_at(authority_end);
    let query = match tail.find(['?', '#']) {
        Some(at) => &tail[at..],
        None => "",
    };
    format!("{}://{authority}/{name}{query}", &url[..scheme_end])
}

/// A URL as a heading says it: no credentials and no settings. Whatever is before the last `@` is
/// taken for credentials, so that a password never shows, even in a form no driver reads.
pub fn shown_url(url: &str) -> String {
    let bare = &url[..url.find('?').unwrap_or(url.len())];
    match bare.split_once("://") {
        Some((scheme, rest)) => {
            let host = rest.rsplit_once('@').map_or(rest, |(_, host)| host);
            format!("{scheme}://{}", &host[..host.find('#').unwrap_or(host.len())])
        }
        None => bare.to_string(),
    }
}

/// The database a URL names.
pub fn database_of(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else { return String::new() };
    let rest = &url[scheme_end + 3..];
    let start = host_start(rest);
    let Some(slash) = rest[start..].find('/') else { return String::new() };
    let path = &rest[start + slash + 1..];
    let end = path.find(['?', '#']).unwrap_or(path.len());
    crate::percent_decode(&path[..end])
}

pub(crate) fn random_name() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    hasher.write_u32(std::process::id());
    format!("{SHADOW_PREFIX}{:010x}", hasher.finish() & 0xff_ffff_ffff)
}

/// A Postgres identifier, quoted.
pub(crate) fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// pg_dump against `url`, with `args`, its password in its environment rather than on its command
/// line. pg_dump refuses a server newer than itself, so the one of the server's own `major` version
/// is looked for first (Homebrew's keg, and the directory Debian's and Ubuntu's packages install it
/// in, whose PATH wrapper picks a local cluster's version), then the PATH's.
pub(crate) fn pg_dump(url: &str, major: i64, args: &[String]) -> Result<String, Error> {
    let version_of = |p: &str| -> i64 {
        let out = std::process::Command::new(p).arg("--version").output().ok();
        let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        text.split_whitespace().find_map(|w| w.split('.').next()?.parse().ok()).unwrap_or(0)
    };
    let candidates = [format!("/opt/homebrew/opt/postgresql@{major}/bin/pg_dump"), format!("/usr/lib/postgresql/{major}/bin/pg_dump"), "pg_dump".to_string()];
    let Some(binary) = candidates.iter().find(|p| version_of(p) >= major) else {
        return Err(Error(format!("reading this schema needs pg_dump {major} or newer (the server is Postgres {major})")));
    };
    let (bare, password) = without_password(url);
    let mut command = std::process::Command::new(binary);
    command.args(args).arg(bare);
    if let Some(password) = password {
        command.env("PGPASSWORD", password);
    }
    let out = command.output()?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        // Its message, without the prefix it puts on every line.
        let said = said.trim().trim_start_matches("pg_dump: error: ").trim_start_matches("pg_dump: ");
        if said.starts_with("no matching schemas were found") {
            return Err(Error("this database has no schema of the name --schema gives".into()));
        }
        return Err(Error(format!("pg_dump couldn't read this schema: {said}")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The URL without its password, and the password, decoded.
fn without_password(url: &str) -> (String, Option<String>) {
    let Some(scheme_end) = url.find("://") else { return (url.to_string(), None) };
    let rest = &url[scheme_end + 3..];
    let at = host_start(rest);
    if at == 0 {
        return (url.to_string(), None);
    }
    let userinfo = &rest[..at - 1];
    match userinfo.split_once(':') {
        Some((user, password)) => (format!("{}://{user}@{}", &url[..scheme_end], &rest[at..]), Some(crate::percent_decode(password))),
        None => (url.to_string(), None),
    }
}

/// The platform's own schemas on the target, as the statements that make them, without owners,
/// privileges or comments, and without the extensions, which a shadow makes itself. What the
/// project made on the platform's tables is left to the project's own SQL, which makes it again: a
/// policy (Reylo takes every policy for the project's), and a trigger whose function is outside the
/// platform's schemas (`on_auth_user_created`, calling `public.handle_new_user()`).
fn platform_ddl(url: &str, major: i64, schemas: &[String]) -> Result<Vec<String>, Error> {
    let mut args: Vec<String> = ["--schema-only", "--no-owner", "--no-privileges", "--no-comments", "--no-publications", "--no-subscriptions", "--no-security-labels", "--no-tablespaces"].map(String::from).to_vec();
    for s in schemas {
        args.push("--schema".into());
        args.push(s.clone());
    }
    let text = pg_dump(url, major, &args)?;
    let kept: Vec<&str> = text.lines().filter(|l| !l.starts_with('\\') && !l.starts_with("SET ") && !l.starts_with("SELECT pg_catalog.set_config") && !l.starts_with("--")).collect();
    Ok(crate::sql::split_statements(&kept.join("\n"), Dialect::Postgres)
        .into_iter()
        .map(|s| s.text)
        .filter(|t| {
            let head = t.trim_start().to_ascii_uppercase();
            if head.starts_with("CREATE EXTENSION") || head.starts_with("COMMENT ON EXTENSION") || head.starts_with("CREATE POLICY") {
                return false;
            }
            if head.starts_with("CREATE TRIGGER") || head.starts_with("CREATE CONSTRAINT TRIGGER") {
                static FUNCTION: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r#"(?i)\bexecute\s+(?:function|procedure)\s+"?([^".(\s]+)"?\."#).expect("a valid pattern"));
                return FUNCTION.captures(t).is_some_and(|c| schemas.iter().any(|s| s.eq_ignore_ascii_case(&c[1])));
            }
            true
        })
        .collect())
}

/// How the note opens that says what every shadow leaves out of the target.
pub const LEFT_OUT: &str = "Shadow databases leave out";

/// Runs `statements` in the new shadow `name` as `bare`, a role the connection may become, which may
/// create schemas in it and has `privileges` on every schema for the while it makes them, given back
/// after. A statement it can't run is left out.
#[cfg(feature = "cli")]
async fn make_as(admin: &tokio_postgres::Client, db: &mut Db, name: &str, bare: &str, statements: &[String], privileges: &[&str]) {
    let role = ident(bare);
    let _ = admin.batch_execute(&format!("grant create on database \"{name}\" to {role}")).await;
    let lacking = |privilege: &str| {
        format!(
            "select format('grant {} on schema %I to %s', nspname, {}) as s from pg_namespace where nspname not like 'pg\\_%' and nspname <> 'information_schema' and not has_schema_privilege({}, oid, '{privilege}')",
            privilege.to_lowercase(),
            literal(&role),
            literal(bare)
        )
    };
    let mut grants: Vec<String> = Vec::new();
    for privilege in privileges {
        grants.extend(db.rows(&lacking(privilege)).await.unwrap_or_default().iter().map(|r| r.text("s").to_string()));
    }
    for g in &grants {
        let _ = db.exec(g).await;
    }
    let _ = db.exec(&format!("set role {role}")).await;
    let _ = db.exec("set check_function_bodies = off").await;
    for statement in statements {
        let _ = db.exec(statement).await;
    }
    let _ = db.exec("reset check_function_bodies").await;
    let _ = db.exec("reset role").await;
    for g in &grants {
        let _ = db.exec(&format!("revoke{}", &g["grant".len()..]).replacen(" to ", " from ", 1)).await;
    }
    let _ = admin.batch_execute(&format!("revoke create on database \"{name}\" from {role}")).await;
}

/// What a superuser the target's role may not act as owns there that the shadows need beyond the
/// name list's schemas, which is the platform's: a shadow makes it as a role every comparison
/// leaves out, or leaves it out.
#[cfg(feature = "cli")]
#[derive(Default)]
pub struct Owned {
    /// Its schemas, whole, as pg_dump makes them, with the privileges the target gives on them, and
    /// its routines in the project's schemas.
    pub(crate) statements: Vec<String>,
    /// Its extensions (Tiger Cloud's `timescaledb`, Azure's `pgaadauth`), each `(name, schema)`.
    pub(crate) extensions: Vec<(String, String)>,
    /// The role of the shadows' server that makes them, [`crate::catalog::SHADOW_PLATFORM_ROLE`],
    /// where the connection may become it.
    pub(crate) role: Option<String>,
    pub(crate) note: Option<String>,
}

#[cfg(feature = "cli")]
impl Owned {
    /// What a replay cache's key takes of it beside its statements.
    pub(crate) fn key(&self) -> String {
        let extensions: Vec<String> = self.extensions.iter().map(|(e, s)| format!("{e} {s}")).collect();
        format!("{} {}", self.role.as_deref().unwrap_or_default(), extensions.join(","))
    }
}

/// What [`Owned`] the target has, with the role the shadows' server (`admin`'s) makes it as, where
/// that server has it and the connection may become it; else the note that says it is left out.
/// Reylo makes no role itself: a role is the whole server's, and outlives every run, so the shadows'
/// server, often the target's own, is given one only by whoever runs it.
#[cfg(feature = "cli")]
async fn owned_by_platform(target: &mut Db, admin: &tokio_postgres::Client, url: &str, version: i64) -> Result<Owned, Error> {
    use crate::catalog::{beyond_reach, in_project_schema, not_extension, not_platform_schema, PG_USER_NS, PLATFORM_ROLES, SHADOW_PLATFORM_ROLE};
    let named = PLATFORM_ROLES.iter().map(|r| literal(r)).collect::<Vec<_>>().join(", ");
    let theirs = |owner: &str| format!("{} and pg_get_userbyid({owner}) <> all (array[{named}])", beyond_reach(owner));
    let rows = target
        .rows(&format!(
            "select 's' as k, n.nspname::text as a, null::text as b, pg_get_userbyid(n.nspowner)::text as owner, 0::int8 as o from pg_namespace n
              where {} and n.nspname <> 'public' and n.nspname not like 'pg\\_%' and n.nspname <> 'information_schema' and {}
            union all
            select 'x', x.extname, n.nspname, pg_get_userbyid(x.extowner), 0 from pg_extension x join pg_namespace n on n.oid = x.extnamespace where x.extname <> 'plpgsql' and {}
            union all
            select 'f', pg_get_functiondef(p.oid), null, pg_get_userbyid(p.proowner), p.oid::int8 from pg_proc p join pg_namespace n on n.oid = p.pronamespace
              where p.prokind in ('f', 'p') and {PG_USER_NS} and {} and {} and {}
            order by 1, 5, 2",
            theirs("n.nspowner"),
            not_extension("pg_namespace", "n"),
            // One in a schema of the project's is the project's, whoever owns it.
            format!("{} and not {}", theirs("x.extowner"), in_project_schema("n")),
            theirs("p.proowner"),
            not_platform_schema("n"),
            not_extension("pg_proc", "p")
        ))
        .await?;
    if rows.is_empty() {
        return Ok(Owned::default());
    }
    let of = |k: &'static str| rows.iter().filter(move |r| r.text("k") == k);
    let schemas: Vec<String> = of("s").map(|r| r.text("a").to_string()).collect();
    let mut owners: Vec<String> = rows.iter().map(|r| r.text("owner").to_string()).collect();
    owners.sort();
    owners.dedup();
    let mut statements = if schemas.is_empty() { vec![] } else { platform_ddl(url, version / 10000, &schemas).unwrap_or_default() };
    statements.extend(schema_grants(target, &schemas).await?);
    statements.extend(of("f").map(|r| r.text("a").to_string()));
    let extensions = of("x").map(|r| (r.text("a").to_string(), r.text("b").to_string())).collect();
    let role = SHADOW_PLATFORM_ROLE;
    let may = format!("select pg_has_role(current_user, oid, case when current_setting('server_version_num')::int >= 160000 then 'SET' else 'MEMBER' end)::text as may from pg_roles where rolname = {}", literal(role));
    if pg_rows(admin, &may).await?.first().is_some_and(|r| r.text("may") == "true") {
        return Ok(Owned { statements, extensions, role: Some(role.to_string()), note: None });
    }
    // The role the shadows are made as, which takes on the role to make it: `shadow_url`'s, else
    // the target's.
    let user = pg_rows(admin, "select current_user as u").await?.first().map(|r| r.text("u").to_string()).unwrap_or_default();
    let note = format!(
        "· {LEFT_OUT} what {} {} here, the platform's, so a migration that uses it fails in them: a role {role} on their server that {user}, the role they are made as, may SET ROLE to lets them make it",
        owners.join(", "),
        if owners.len() == 1 { "owns" } else { "own" }
    );
    Ok(Owned { statements: vec![], extensions, role: None, note: Some(note) })
}

/// Gives a shadow's session the target's word on whether it is a platform's (`settings`, the target
/// database's own, carry [`crate::catalog::PLATFORM_SCHEMAS_SETTING`]).
#[cfg(feature = "cli")]
pub(crate) async fn mark_platform_names(db: &mut Db, settings: &[String]) {
    let key = format!("{}=", crate::catalog::PLATFORM_SCHEMAS_SETTING);
    if let Some(value) = settings.iter().find_map(|s| s.strip_prefix(&key)) {
        let _ = db.exec(&format!("select set_config({}, {}, false)", literal(crate::catalog::PLATFORM_SCHEMAS_SETTING), literal(value))).await;
    }
}

/// The target's privileges on `schemas`, as the grants that make them, its owner's own left out.
async fn schema_grants(target: &mut Db, schemas: &[String]) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    for schema in schemas {
        let rows = target
            .rows(&format!(
                "select format('grant %s on schema %I to %s%s', x.privilege_type, n.nspname, case when x.grantee = 0 then 'public' else quote_ident(pg_get_userbyid(x.grantee)) end, case when x.is_grantable then ' with grant option' else '' end) as g
                from pg_namespace n, aclexplode(n.nspacl) x where n.nspname = {} and x.grantee <> n.nspowner",
                literal(schema)
            ))
            .await?;
        out.extend(rows.iter().map(|r| r.text("g").to_string()));
    }
    Ok(out)
}

/// Revokes what was granted on the MySQL database `name`'s tables, columns and routines, which
/// MySQL keeps after the database is dropped: a shadow's grants would pile up on the server, and a
/// database made later under its name would have them. A column's privilege is kept with a row for
/// its table, so the tables' and the routines' rows name everything. What the connection may not
/// read or revoke is left.
#[cfg(feature = "cli")]
async fn mysql_revoke_on(admin: &mut mysql_async::Conn, name: &str) {
    let tick = |n: &str| format!("`{}`", n.replace('`', "``"));
    let rows: Vec<(String, String, String, String)> = admin
        .query(format!("select user, host, 'table', table_name from mysql.tables_priv where db = {} union all select user, host, lower(routine_type), routine_name from mysql.procs_priv where db = {}", literal(name), literal(name)))
        .await
        .unwrap_or_default();
    for (user, host, kind, object) in rows {
        let on = if kind == "table" { String::new() } else { format!("{kind} ") };
        for what in ["all privileges", "grant option"] {
            let _ = admin.query_drop(format!("revoke if exists {what} on {on}{}.{} from {}@{} ignore unknown user", tick(name), tick(&object), tick(&user), tick(&host))).await;
        }
    }
}

pub(crate) fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub struct Shadow {
    pub db: Db,
    pub name: String,
}

/// Where shadows are made: the target's server, or the one `shadow_url` names. Without the `cli`
/// feature there is none: deploy and status make no shadow.
#[cfg(not(feature = "cli"))]
pub enum Server {}

/// Where shadows are made: the target's server, or the one `shadow_url` names.
#[cfg(feature = "cli")]
pub enum Server {
    Postgres {
        admin: tokio_postgres::Client,
        url: String,
        /// What follows `create database "<name>"`: the target's encoding and locale, where they are not template1's.
        create: String,
        /// Why a shadow's `create database` may refuse the target's locale: a name the shadow server
        /// knows by no spelling, as the refusal says.
        locale: Option<String>,
        /// The target database's own settings (`alter database … set`), such as a search_path, which
        /// decide where an unqualified name goes.
        settings: Vec<String>,
        /// The target's default privileges (`alter default privileges`), which grant on what a
        /// migration creates, as a platform's do (Supabase's grant every new table to its API's
        /// roles); made again in each shadow unless the project sets its own.
        privileges: Vec<String>,
        copy_privileges: bool,
        /// The target's extensions, each `(name, schema)`, made again in each shadow but those the
        /// project's own files create (`leave`): a platform installs some for every project
        /// (Supabase's `pgcrypto` and `uuid-ossp`, in its `extensions` schema), which migrations call.
        extensions: Vec<(String, String)>,
        leave: Vec<String>,
        /// The schemas a platform's roles own on the target (Supabase's `auth` and `storage`), as the
        /// statements that make them, which a migration's `references auth.users` needs, and the
        /// platform's role they are made as in each shadow (one the connection may become), so that
        /// every comparison leaves them out there as it does on the target.
        environment: Vec<String>,
        environment_role: Option<String>,
        /// What a superuser the target's role may not act as owns there beyond those schemas (Tiger
        /// Cloud's `timescale_functions`, its functions in `public`, its extensions), the platform's
        /// too: made in each shadow as a role every comparison leaves out there, as the superuser's
        /// is on the target, and left out where the shadows' role may not become it.
        owned: Owned,
        /// The target's privileges on the schemas a shadow has without the project making them
        /// (`public`, and those its extensions are in), as `grant` statements.
        schema_grants: Vec<String>,
    },
    Mysql {
        admin: mysql_async::Conn,
        url: String,
        /// The target database's default character set and collation, which every table created without its own inherits.
        defaults: Option<(String, String)>,
    },
    /// Shadows in memory: on libSQL's engine for a libSQL database (Turso's), else Reylo's SQLite.
    Sqlite { libsql: bool },
    Sqlserver {
        /// A connection to `master`, which creates and drops the shadows and holds each one's lock.
        admin: Box<reylo_engine::sqlserver::Conn>,
        url: String,
        /// The target database's collation, which every column created without its own takes.
        collation: Option<String>,
    },
}

/// Everything about a SQL Server a replay's outcome turns on, beside its steps: its version and
/// collation, the login the shadows are made as, and the server's principals a grant may name.
#[cfg(feature = "cli")]
pub(crate) async fn sqlserver_state(admin: &mut reylo_engine::sqlserver::Conn) -> Result<String, Error> {
    let rows = sqlserver_rows(
        admin,
        "select concat_ws(char(10), @@version, cast(serverproperty('Collation') as nvarchar(128)), suser_sname(),
          (select string_agg(cast(name as nvarchar(max)), N',') within group (order by name) from sys.server_principals where type in ('S', 'U', 'G', 'R'))) as s",
    )
    .await?;
    Ok(rows.first().map(|r| r.text("s").to_string()).unwrap_or_default())
}

/// Takes the application lock `name` for the session, at once: whether it was granted.
#[cfg(feature = "cli")]
async fn sqlserver_lock(admin: &mut reylo_engine::sqlserver::Conn, name: &str) -> Result<bool, Error> {
    let rows = sqlserver_rows(admin, &format!("declare @r int; exec @r = sp_getapplock @Resource = N'{name}', @LockMode = 'Exclusive', @LockOwner = 'Session', @LockTimeout = 0; select @r as r")).await?;
    Ok(rows.first().and_then(|r| r.text("r").parse::<i64>().ok()).is_some_and(|r| r >= 0))
}

#[cfg(feature = "cli")]
async fn sqlserver_unlock(admin: &mut reylo_engine::sqlserver::Conn, name: &str) {
    let _ = sqlserver_rows(admin, &format!("exec sp_releaseapplock @Resource = N'{name}', @LockOwner = 'Session'")).await;
}

/// Drops a SQL Server database, its other sessions ended first.
#[cfg(feature = "cli")]
async fn sqlserver_drop(admin: &mut reylo_engine::sqlserver::Conn, name: &str) -> Result<(), Error> {
    let db = bracket(name);
    sqlserver_rows(admin, &format!("if db_id(N'{name}') is not null begin alter database {db} set single_user with rollback immediate; drop database {db}; end")).await.map(|_| ())
}

#[cfg(feature = "cli")]
const PG_LISTS: &[&str] = &["search_path", "temp_tablespaces", "local_preload_libraries", "session_preload_libraries"];

#[cfg(feature = "cli")]
impl Server {
    /// The server shadows are made on: `shadow_url`'s, else the target's. A shadow is made like the
    /// target, which `target` is connected to.
    pub async fn open(target: &mut Db, url: &str, shadow_url: Option<&str>) -> Result<Server, Error> {
        let server_url = shadow_url.unwrap_or(url);
        match target.dialect() {
            Dialect::Postgres => {
                // The maintenance connection: `postgres`, else `template1`, since `create database`
                // cannot run in the database it creates beside.
                let mut admin = None;
                let mut last = String::new();
                for db in ["postgres", "template1"] {
                    match crate::db::open_postgres(&with_database(server_url, db)).await {
                        Ok(client) => {
                            admin = Some(client);
                            break;
                        }
                        Err(e) => last = e.0,
                    }
                }
                let Some(admin) = admin else {
                    return Err(Error(format!("cannot connect to the server's postgres or template1 database to create shadow databases: {last}")));
                };
                let version: i64 = target.rows("select current_setting('server_version_num') as v").await?.first().and_then(|r| r.first()?.parse().ok()).unwrap_or(0);
                // The locale provider is Postgres 15's, and the locale `datlocale` from 17 and
                // `daticulocale` in 15 and 16: read through `to_jsonb`, a column the server lacks is
                // null, and a database before 15 is libc's.
                let props = |filter: &str| {
                    format!(
                        "select pg_encoding_to_char(encoding) as enc, datcollate as collate, datctype as ctype, coalesce(to_jsonb(d) ->> 'datlocprovider', 'c') as provider,
                          coalesce(to_jsonb(d) ->> 'datlocale', to_jsonb(d) ->> 'daticulocale') as locale from pg_database d where {filter}"
                    )
                };
                let mine = target.rows(&props("datname = current_database()")).await?.into_iter().next();
                let template = pg_rows(&admin, &props("datname = 'template1'")).await?.into_iter().next();
                let key = |r: &Row| ["enc", "collate", "ctype", "provider", "locale"].map(|k| r.get(k).map(String::from));
                // A libc locale is named as the shadow server's system names it, which another
                // system may spell otherwise (Linux's `en_US.utf8` is macOS's `en_US.UTF-8`).
                let known: Vec<String> = pg_rows(
                    &admin,
                    "select collcollate as n from pg_collation where collprovider = 'c' and collcollate is not null
                      union select collctype from pg_collation where collprovider = 'c' and collctype is not null
                      union select datcollate from pg_database union select datctype from pg_database",
                )
                .await?
                .iter()
                .filter_map(|r| r.get("n").map(String::from))
                .collect();
                let mut unknown_locale = None;
                let create = match (&mine, &template) {
                    (Some(m), Some(t)) if key(m) != key(t) => {
                        let provider = match m.text("provider") {
                            "i" => "icu",
                            "b" => "builtin",
                            _ => "libc",
                        };
                        let locale = match m.get("locale") {
                            Some(l) if !l.is_empty() => format!(" {} {}", if provider == "icu" { "icu_locale" } else { "builtin_locale" }, literal(l)),
                            _ => String::new(),
                        };
                        // `locale_provider` is Postgres 15's: before it, every database is libc's. The
                        // statement runs on the shadows' server, which `shadow_url` may make another.
                        let shadow_version: i64 = pg_rows(&admin, "select current_setting('server_version_num') as v").await?.first().and_then(|r| r.first()?.parse().ok()).unwrap_or(0);
                        let provider = if shadow_version >= 150000 { format!(" locale_provider {provider}{locale}") } else { String::new() };
                        let mut spelled = |name: &str| match locale_spelled(name, &known) {
                            Some(on_shadow) => on_shadow,
                            None => {
                                unknown_locale = Some(format!(
                                    "This database's locale, {name}, is one the shadow server has under no spelling (its own is {}): set shadow_url in .reylo.yaml to a server that has it",
                                    t.text("collate")
                                ));
                                name.to_string()
                            }
                        };
                        let (collate, ctype) = (spelled(m.text("collate")), spelled(m.text("ctype")));
                        format!(" template template0 encoding {} lc_collate {} lc_ctype {}{provider}", literal(m.text("enc")), literal(&collate), literal(&ctype))
                    }
                    _ => String::new(),
                };
                let mut settings: Vec<String> = target
                    .rows("select unnest(setconfig) as s from pg_db_role_setting where setrole = 0 and setdatabase = (select oid from pg_database where datname = current_database())")
                    .await?
                    .iter()
                    .filter_map(|r| r.get("s").map(String::from))
                    .collect();
                // Whether the target is a platform's, which every shadow takes from it.
                let setting = crate::catalog::PLATFORM_SCHEMAS_SETTING;
                let names = target.rows(&format!("select ({})::text as on", crate::catalog::platform_names())).await?.first().is_some_and(|r| r.text("on") == "true");
                settings.retain(|s| !s.starts_with(&format!("{setting}=")));
                settings.push(format!("{setting}={}", if names { "on" } else { "off" }));
                let privileges = default_privileges(target).await?;
                let extensions: Vec<(String, String)> = target
                    .rows("select e.extname as name, n.nspname as schema from pg_extension e join pg_namespace n on n.oid = e.extnamespace where e.extname <> 'plpgsql' order by 1")
                    .await?
                    .iter()
                    .map(|r| (r.text("name").to_string(), r.text("schema").to_string()))
                    .collect();
                let roles = crate::catalog::PLATFORM_ROLES.iter().map(|r| literal(r)).collect::<Vec<_>>().join(", ");
                let platform: Vec<String> = target
                    .rows(&format!("select nspname as n from pg_namespace where pg_get_userbyid(nspowner) in ({roles}) and nspname not like 'pg\\_%' and nspname <> 'information_schema' order by 1"))
                    .await?
                    .iter()
                    .map(|r| r.text("n").to_string())
                    .collect();
                let environment_role = if platform.is_empty() {
                    None
                } else {
                    // Postgres 16 grants `SET ROLE` apart from membership; before it, membership is it.
                    let can = if version >= 160000 { "SET" } else { "MEMBER" };
                    target.rows(&format!("select rolname as r from pg_roles where rolname in ({roles}) and pg_has_role(current_user, oid, '{can}') order by rolname limit 1")).await?.first().map(|r| r.text("r").to_string())
                };
                // Made where the shadows may become one of the platform's roles; a dump that fails
                // leaves them without, as before.
                let environment = if environment_role.is_some() { platform_ddl(url, version / 10000, &platform).unwrap_or_default() } else { vec![] };
                let schemas: Vec<String> = std::iter::once("public".to_string()).chain(extensions.iter().map(|(_, s)| s.clone())).filter(|s| !platform.contains(s)).collect();
                let schema_grants = schema_grants(target, &schemas).await?;
                let owned = owned_by_platform(target, &admin, url, version).await?;
                Ok(Server::Postgres { admin, url: server_url.to_string(), create, locale: unknown_locale, settings, privileges, copy_privileges: true, extensions, leave: vec![], environment, environment_role, owned, schema_grants })
            }
            Dialect::Mysql => {
                let admin = crate::db::open_mysql(&with_database(server_url, "")).await?;
                let defaults = target
                    .rows("select @@character_set_database as cs, @@collation_database as co")
                    .await?
                    .into_iter()
                    .next()
                    .and_then(|r| Some((r.get("cs")?.to_string(), r.get("co")?.to_string())));
                Ok(Server::Mysql { admin, url: server_url.to_string(), defaults })
            }
            Dialect::Sqlite => Ok(Server::Sqlite { libsql: target.is_libsql() }),
            Dialect::Sqlserver => {
                let admin = Box::new(open_sqlserver(&with_database(server_url, "master")).await?);
                let collation = target.rows("select cast(collation_name as nvarchar(128)) as c from sys.databases where name = db_name()").await?.first().and_then(|r| r.get("c").map(String::from));
                Ok(Server::Sqlserver { admin, url: server_url.to_string(), collation })
            }
        }
    }

    pub fn dialect(&self) -> Dialect {
        match self {
            Server::Postgres { .. } => Dialect::Postgres,
            Server::Mysql { .. } => Dialect::Mysql,
            Server::Sqlite { .. } => Dialect::Sqlite,
            Server::Sqlserver { .. } => Dialect::Sqlserver,
        }
    }

    /// A new, empty scratch database beside the target.
    pub async fn shadow(&mut self) -> Result<Shadow, Error> {
        match self {
            Server::Postgres { .. } => self.pg_shadow(None).await,
            Server::Mysql { admin, url, defaults } => {
                let name = random_name();
                admin.query_drop(format!("select get_lock('{name}', 0)")).await.map_err(mysql_error)?;
                let charset = match defaults {
                    Some((cs, co)) => format!(" character set {cs} collate {co}"),
                    None => String::new(),
                };
                admin.query_drop(format!("create database `{name}`{charset}")).await.map_err(|e| match &e {
                    mysql_async::Error::Server(s) => match mysql_create_refused(s.code, &s.message) {
                        Some(may_create) => cannot_create(mysql_error(e), may_create),
                        None => mysql_error(e),
                    },
                    _ => mysql_error(e),
                })?;
                let db = Db::Mysql(crate::db::open_mysql(&with_database(url, &name)).await?);
                Ok(Shadow { db, name })
            }
            // A libSQL database's shadows are on libSQL's engine, as the database is: its SQL, and the
            // version of SQLite Turso Cloud runs.
            Server::Sqlite { libsql: true } => Ok(Shadow { db: Db::LibsqlLocal(crate::libsql_local::Local::memory()?), name: ":memory:".into() }),
            Server::Sqlite { libsql: false } => Ok(Shadow { db: Db::Sqlite(rusqlite::Connection::open_in_memory().map_err(sqlite_error)?), name: ":memory:".into() }),
            Server::Sqlserver { admin, url, collation } => {
                let name = random_name();
                if !sqlserver_lock(admin, &name).await? {
                    return Err(Error(format!("cannot lock the shadow {name}")));
                }
                // A collation's name is a word, which the server reads unquoted alone.
                let collate = collation.as_deref().filter(|c| c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')).map(|c| format!(" collate {c}")).unwrap_or_default();
                let made = match admin.simple_query(format!("create database {}{collate}", bracket(&name))).await {
                    Ok(stream) => stream.into_results().await.map(drop),
                    Err(e) => Err(e),
                };
                if let Err(e) = made {
                    sqlserver_unlock(admin, &name).await;
                    // 262, CREATE DATABASE permission denied: the login lacks CREATE ANY DATABASE, which `dbcreator` holds.
                    let refused = matches!(&e, tiberius::error::Error::Server(t) if t.code() == 262);
                    return Err(if refused { cannot_create(sqlserver_error(e), false) } else { sqlserver_error(e) });
                }
                let db = Db::Sqlserver(Box::new(open_sqlserver(&with_database(url, &name)).await?));
                Ok(Shadow { db, name })
            }
        }
    }

    /// A Postgres shadow: a new database made as the target's, or a copy of `template`, a replay
    /// cache, which already holds what a new shadow is given (its encoding, locale and default
    /// privileges) but not its database settings, which a copy does not take. The lock named after
    /// it is held until it is dropped; a copy that fails releases it.
    pub(crate) async fn pg_shadow(&mut self, template: Option<&str>) -> Result<Shadow, Error> {
        let Server::Postgres { admin, url, create, locale, settings, privileges, copy_privileges, extensions, leave, environment, environment_role, owned, schema_grants, .. } = self else { return Err(Error("not a Postgres server".into())) };
        let name = random_name();
        admin.batch_execute(&format!("select pg_advisory_lock(hashtextextended('{name}', 0))")).await.map_err(pg_error)?;
        let made = async {
            let made = match template {
                Some(t) => admin.batch_execute(&format!("create database \"{name}\" template \"{t}\"")).await,
                None => admin.batch_execute(&format!("create database \"{name}\"{create}")).await,
            };
            if let Err(e) = made {
                let refused = e.as_db_error().map(|d| d.message().to_string()).unwrap_or_default();
                if let (Some(why), true) = (locale.as_ref(), refused.contains("locale")) {
                    return Err(Error(format!("{refused}. {why}")));
                }
                if e.code() != Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE) {
                    return Err(pg_error(e));
                }
                let may = pg_rows(admin, "select (rolcreatedb or rolsuper)::text as may from pg_roles where rolname = current_user").await?;
                return Err(cannot_create(pg_error(e), may.first().is_some_and(|r| r.text("may") == "true")));
            }
            for s in settings.iter() {
                let Some((key, value)) = s.split_once('=') else { continue };
                let value = if PG_LISTS.contains(&key) {
                    value.split(',').map(|v| literal(v.trim().trim_matches('"'))).collect::<Vec<_>>().join(", ")
                } else {
                    literal(value)
                };
                // One the connected role may not set is the platform's (Supabase's `app.settings.*`,
                // which only its administrator sets), and no statement of the project's reads it.
                if let Err(e) = admin.batch_execute(&format!("alter database \"{name}\" set {key} = {value}")).await {
                    if e.code() != Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE) {
                        return Err(pg_error(e));
                    }
                }
            }
            crate::db::open_postgres(&with_database(url, &name)).await
        }
        .await;
        let client = match made {
            Ok(client) => client,
            Err(e) if template.is_some() => {
                let _ = admin.batch_execute(&format!("drop database if exists \"{name}\" with (force)")).await;
                let _ = admin.batch_execute(&format!("select pg_advisory_unlock(hashtextextended('{name}', 0))")).await;
                return Err(e);
            }
            Err(e) => return Err(e),
        };
        let mut db = Db::Postgres(client);
        // For the session too, where the database may not take the setting from the role connected.
        mark_platform_names(&mut db, settings).await;
        if *copy_privileges && template.is_none() {
            // A role or schema the shadow's server lacks leaves that one out; the comparison says what follows.
            for statement in privileges.iter() {
                let _ = db.exec(statement).await;
            }
        }
        if template.is_none() {
            // The target's environment, as the platform made it: its extensions but those the
            // project creates, the privileges on the schemas they and `public` are in, and the
            // platform's own schemas, made as one of its roles. A statement the shadow can't run
            // (one only a superuser may) is left out; what the project then needs of it fails in
            // the replay, which says so.
            // The extensions in a schema of the project's first, which the platform's own may use;
            // those in a platform's schema after it is made, as the platform made it.
            let copied: Vec<&(String, String)> = extensions.iter().filter(|(e, _)| !leave.contains(&e.to_lowercase()) && !owned.extensions.iter().any(|(o, _)| o == e)).collect();
            let platform_schema = |schema: &str| environment.iter().chain(owned.statements.iter()).any(|st| st.trim_start().to_ascii_lowercase().starts_with(&format!("create schema {}", schema.to_ascii_lowercase())));
            for (extension, schema) in copied.iter().filter(|(_, s)| !platform_schema(s)) {
                let _ = db.exec(&format!("create schema if not exists {}", ident(schema))).await;
                let _ = db.exec(&format!("create extension if not exists {} with schema {}", ident(extension), ident(schema))).await;
            }
            for (_, schema) in owned.extensions.iter().filter(|(e, s)| owned.role.is_some() && !leave.contains(&e.to_lowercase()) && !platform_schema(s) && s != "pg_catalog") {
                let _ = db.exec(&format!("create schema if not exists {}", ident(schema))).await;
            }
            for statement in schema_grants.iter() {
                let _ = db.exec(statement).await;
            }
            if let (Some(role), false) = (environment_role.as_deref(), environment.is_empty()) {
                make_as(admin, &mut db, &name, role, environment, &["USAGE"]).await;
            }
            let owned_extensions: Vec<&(String, String)> = owned.extensions.iter().filter(|(e, _)| !leave.contains(&e.to_lowercase())).collect();
            if let Some(role) = owned.role.as_deref() {
                // Its extensions after its schemas, which they may be in, and before its routines,
                // which may use them; one no role but a superuser may make, made by a superuser
                // connected and given to the role.
                let routines = owned.statements.iter().position(|st| st.starts_with("CREATE OR REPLACE ")).unwrap_or(owned.statements.len());
                let mut statements = owned.statements[..routines].to_vec();
                statements.extend(owned_extensions.iter().map(|(e, s)| format!("create extension if not exists {} with schema {}", ident(e), ident(s))));
                statements.extend(owned.statements[routines..].iter().cloned());
                make_as(admin, &mut db, &name, role, &statements, &["USAGE", "CREATE"]).await;
                let superuser = db.rows("select rolsuper::text as s from pg_roles where rolname = current_user").await.ok().is_some_and(|r| r.first().is_some_and(|r| r.text("s") == "true"));
                for (extension, schema) in owned_extensions.iter().filter(|_| superuser) {
                    let missing = db.rows(&format!("select 1 from pg_extension where extname = {}", literal(extension))).await.is_ok_and(|r| r.is_empty());
                    if missing && db.exec(&format!("create extension if not exists {} with schema {}", ident(extension), ident(schema))).await.is_ok() {
                        let given = format!("update pg_catalog.pg_extension set extowner = {}::regrole where extname = {}", literal(&ident(role)), literal(extension));
                        if db.exec(&given).await.is_err() {
                            let _ = db.exec(&format!("drop extension if exists {}", ident(extension))).await;
                        }
                    }
                }
            }
            for (extension, schema) in copied.iter().filter(|(_, s)| platform_schema(s)) {
                let _ = db.exec(&format!("create extension if not exists {} with schema {}", ident(extension), ident(schema))).await;
            }
        }
        Ok(Shadow { db, name })
    }

    /// What every shadow leaves out of the target, and why, for the command to say once.
    pub fn note(&self) -> Option<&str> {
        match self {
            Server::Postgres { owned, .. } => owned.note.as_deref(),
            _ => None,
        }
    }

    pub async fn drop_shadow(&mut self, shadow: Shadow) {
        let Shadow { db, name } = shadow;
        match self {
            Server::Postgres { admin, .. } => {
                pg_close(admin, db, &name).await;
                let _ = admin.batch_execute(&format!("drop database if exists \"{name}\" with (force)")).await;
                let _ = admin.batch_execute(&format!("select pg_advisory_unlock(hashtextextended('{name}', 0))")).await;
            }
            Server::Mysql { admin, .. } => {
                db.close().await;
                let _ = admin.query_drop(format!("drop database if exists `{name}`")).await;
                mysql_revoke_on(admin, &name).await;
                let _ = admin.query_drop(format!("select release_lock('{name}')")).await;
            }
            Server::Sqlite { .. } => {}
            Server::Sqlserver { admin, .. } => {
                db.close().await;
                let _ = sqlserver_drop(admin, &name).await;
                sqlserver_unlock(admin, &name).await;
            }
        }
    }

    /// The extensions the project's own files create, which a shadow leaves to them.
    pub fn leave_extensions(&mut self, names: Vec<String>) {
        if let Server::Postgres { leave, .. } = self {
            *leave = names;
        }
    }

    /// The schemas the project's own files create: an extension the target keeps in one of them (one
    /// the project once created there, and no longer does) is left out of a shadow too, whose
    /// `create schema` it would otherwise stop.
    pub fn leave_schemas(&mut self, schemas: Vec<String>) {
        if let Server::Postgres { leave, extensions, .. } = self {
            leave.extend(extensions.iter().filter(|(_, s)| schemas.iter().any(|x| x.eq_ignore_ascii_case(s))).map(|(e, _)| e.to_lowercase()));
        }
    }

    /// Whether shadows take the target's default privileges: not where the project's own SQL sets
    /// default privileges, which would then grant on what its earlier migrations made.
    pub fn copy_default_privileges(&mut self, on: bool) {
        if let Server::Postgres { copy_privileges, .. } = self {
            *copy_privileges = on;
        }
    }

    /// Drops the shadows a run that died left behind, and the replay caches nobody will use; answers
    /// how many shadows.
    pub async fn sweep(&mut self) -> Result<usize, Error> {
        let mut dropped = 0;
        match self {
            Server::Postgres { admin, .. } => {
                for row in pg_rows(admin, &format!("select datname from pg_database where datname like '{SHADOW_PREFIX}%'")).await? {
                    let name = row.text("datname").to_string();
                    let free = pg_rows(admin, &format!("select pg_try_advisory_lock(hashtextextended('{name}', 0)) as free")).await?.first().is_some_and(|r| r.flag("free"));
                    if !free {
                        continue;
                    }
                    // Another role's (a teammate's, on a shared server) is that role's to drop.
                    let drop = admin.batch_execute(&format!("drop database if exists \"{name}\" with (force)")).await;
                    admin.batch_execute(&format!("select pg_advisory_unlock(hashtextextended('{name}', 0))")).await.map_err(pg_error)?;
                    match drop {
                        Ok(()) => dropped += 1,
                        Err(e) if e.code() == Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE) => {}
                        Err(e) => return Err(pg_error(e)),
                    }
                }
            }
            Server::Mysql { admin, .. } => {
                let names: Vec<String> = admin.query(format!("select schema_name from information_schema.schemata where schema_name like '{SHADOW_PREFIX}%'")).await.map_err(mysql_error)?;
                for name in names {
                    let free: Option<i64> = admin.query_first(format!("select is_free_lock('{name}')")).await.map_err(mysql_error)?;
                    if free != Some(1) {
                        continue;
                    }
                    match admin.query_drop(format!("drop database if exists `{name}`")).await {
                        Ok(()) => {
                            mysql_revoke_on(admin, &name).await;
                            dropped += 1
                        }
                        Err(mysql_async::Error::Server(s)) if s.code == 1044 => {}
                        Err(e) => return Err(mysql_error(e)),
                    }
                }
            }
            Server::Sqlite { .. } => {}
            Server::Sqlserver { admin, .. } => {
                let names: Vec<String> = sqlserver_rows(admin, &format!("select name from sys.databases where name like N'{}%'", SHADOW_PREFIX.replace('_', "[_]"))).await?.iter().map(|r| r.text("name").to_string()).collect();
                for name in names {
                    if !sqlserver_lock(admin, &name).await? {
                        continue;
                    }
                    sqlserver_drop(admin, &name).await?;
                    sqlserver_unlock(admin, &name).await;
                    dropped += 1;
                }
            }
        }
        // The replay caches no run will use, which are not counted: they are not a run's leftovers.
        crate::cache::sweep(self).await?;
        Ok(dropped)
    }

    pub async fn close(self) {
        match self {
            Server::Mysql { admin, .. } => {
                let _ = admin.disconnect().await;
            }
            Server::Sqlserver { admin, .. } => {
                let _ = admin.close().await;
            }
            _ => {}
        }
    }
}

/// Ends a Postgres shadow's session, and waits up to a second for its server process to go: a
/// database with a session left cannot be dropped, renamed or copied without the server's own wait
/// for it, a tenth of a second at a time.
#[cfg(feature = "cli")]
pub(crate) async fn pg_close(admin: &tokio_postgres::Client, db: Db, name: &str) {
    drop(db);
    for _ in 0..200 {
        match pg_rows(admin, &format!("select count(*) as n from pg_stat_activity where datname = {}", literal(name))).await {
            Ok(rows) if rows.first().is_some_and(|r| r.text("n") != "0") => tokio::time::sleep(std::time::Duration::from_millis(5)).await,
            _ => return,
        }
    }
}

/// The default privileges of the database `target` is connected to, as the statements that make
/// them: each grant, and on functions and types the revoke from PUBLIC of what Postgres grants by
/// default, for the roles and in the schemas they name (the whole database where none). Those of the
/// role that runs the migrations name none, so that they are the defaults of whoever runs them: a
/// migration made by a developer's role runs in production as the deploy's. A platform's role's are
/// left out (Neon's `cloud_admin`'s, on every database its API makes): they grant only on what that
/// role makes, which is the platform's, and only that role may set them, so no shadow and no
/// migration can.
pub(crate) async fn default_privileges(target: &mut Db) -> Result<Vec<String>, Error> {
    let rows = target
        .rows(&format!(
            "select format('%s', d.defaclrole::regrole) as role, d.defaclrole = {} as mine,
              case when d.defaclnamespace <> 0 then format('%I', n.nspname) end as schema, d.defaclobjtype as kind,
              format('%s', (select json_agg(json_build_object('grantee', case when x.grantee = 0 then 'public' else quote_ident(pg_get_userbyid(x.grantee)) end, 'privilege', x.privilege_type, 'grantable', x.is_grantable))
                 from aclexplode(d.defaclacl) x where x.grantee <> d.defaclrole)) as acl
            from pg_default_acl d left join pg_namespace n on n.oid = d.defaclnamespace
            where {} and (d.defaclnamespace = 0 or {})",
            crate::catalog::runner(),
            crate::catalog::not_platform("d.defaclrole"),
            // In a platform's schema, they are the platform's (Supabase's own in `storage`).
            crate::catalog::not_platform_schema("n")
        ))
        .await?;
    let mut out = Vec::new();
    for r in rows {
        let objects = match r.text("kind") {
            "r" => "tables",
            "S" => "sequences",
            "f" => "functions",
            "T" => "types",
            "n" => "schemas",
            "L" => "large objects",
            _ => continue,
        };
        let role = if r.flag("mine") { String::new() } else { format!(" for role {}", r.text("role")) };
        let head = format!("alter default privileges{role}{}", r.get("schema").map(|s| format!(" in schema {s}")).unwrap_or_default());
        let grants: Vec<serde_json::Value> = serde_json::from_str(r.text("acl")).unwrap_or_default();
        let public = |privilege: &str| grants.iter().any(|g| g["grantee"] == "public" && g["privilege"] == privilege);
        if r.get("schema").is_none() && objects == "functions" && !public("EXECUTE") {
            out.push(format!("{head} revoke execute on functions from public"));
        }
        if r.get("schema").is_none() && objects == "types" && !public("USAGE") {
            out.push(format!("{head} revoke usage on types from public"));
        }
        for g in &grants {
            let option = if g["grantable"].as_bool().unwrap_or(false) { " with grant option" } else { "" };
            out.push(format!("{head} grant {} on {objects} to {}{option}", g["privilege"].as_str().unwrap_or_default().to_lowercase(), g["grantee"].as_str().unwrap_or_default()));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod url_tests {
    use super::*;

    #[test]
    fn a_locale_is_spelled_as_the_shadow_server_spells_it() {
        let mac: Vec<String> = ["C", "POSIX", "en_US.UTF-8", "de_DE.ISO8859-1", "sr_YU.UTF-8@latin"].map(String::from).to_vec();
        let linux: Vec<String> = ["C", "POSIX", "C.utf8", "en_US.utf8", "de_DE.iso88591"].map(String::from).to_vec();
        assert_eq!(locale_spelled("en_US.utf8", &mac).as_deref(), Some("en_US.UTF-8"));
        assert_eq!(locale_spelled("en_US.UTF-8", &linux).as_deref(), Some("en_US.utf8"));
        assert_eq!(locale_spelled("de_DE.iso88591", &mac).as_deref(), Some("de_DE.ISO8859-1"));
        assert_eq!(locale_spelled("sr_YU.utf8@latin", &mac).as_deref(), Some("sr_YU.UTF-8@latin"));
        assert_eq!(locale_spelled("C.UTF-8", &linux).as_deref(), Some("C.utf8"));
        assert_eq!(locale_spelled("C", &linux).as_deref(), Some("C"));
        // Another language, or another encoding, is no spelling of it.
        assert_eq!(locale_spelled("en_GB.utf8", &mac), None);
        assert_eq!(locale_spelled("en_US.iso88591", &mac), None);
        assert_eq!(locale_spelled("en_US", &mac), None);
    }

    #[test]
    fn a_refused_shadow_is_told_what_helps() {
        // A role that may create databases, refused by the server by name, keeps the server's words and is sent to shadow_url alone.
        let e = cannot_create(Error("tsdb_admin: database reylo_shadow_1 is not an allowed database name. Contact your administrator".into()), true);
        assert!(e.0.starts_with("tsdb_admin: database reylo_shadow_1 is not an allowed database name. Contact your administrator. "), "{}", e.0);
        assert!(e.0.contains("shadow_url") && !e.0.contains("give the role the right"), "{}", e.0);
        let e = cannot_create(Error("permission denied to create database".into()), false);
        assert!(e.0.contains("give the role the right to create databases") && e.0.contains("shadow_url"), "{}", e.0);
    }

    #[test]
    #[cfg(feature = "cli")]
    fn a_mysql_refusal_to_create_a_database_is_told_what_helps() {
        assert_eq!(mysql_create_refused(1044, "Access denied for user 'app'@'%' to database 'reylo_shadow_1'"), Some(false));
        assert_eq!(mysql_create_refused(1227, "Access denied; you need (at least one of) the CREATE privilege(s) for this operation"), Some(false));
        assert_eq!(mysql_create_refused(1105, "VT12001: unsupported: create database by failDBDDL"), Some(true));
        assert_eq!(mysql_create_refused(1105, "VT12001: unsupported: something else"), None);
        assert_eq!(mysql_create_refused(1007, "Can't create database 'reylo_shadow_1'; database exists"), None);
    }

    #[test]
    fn a_password_holding_a_hash_stays_a_password() {
        let url = "postgres://postgres:pa#ss@localhost:5432/app?sslmode=require";
        assert_eq!(with_database(url, "shadow"), "postgres://postgres:pa#ss@localhost:5432/shadow?sslmode=require");
        assert_eq!(database_of(url), "app");
        assert_eq!(shown_url(url), "postgres://localhost:5432/app");
    }

    #[test]
    fn a_heading_never_shows_credentials_or_settings() {
        assert_eq!(shown_url("postgres://u:p@db.example.com/app"), "postgres://db.example.com/app");
        assert_eq!(shown_url("mysql://root@127.0.0.1:3306/app?ssl=true"), "mysql://127.0.0.1:3306/app");
        assert_eq!(shown_url("postgres://u:pa/ss@host/app"), "postgres://host/app");
        assert_eq!(shown_url("postgres://host/app?password=secret"), "postgres://host/app");
        assert_eq!(shown_url("sqlite:./app.db?mode=rwc"), "sqlite:./app.db");
        assert_eq!(shown_url("libsql://app-acme.turso.io?authToken=secret"), "libsql://app-acme.turso.io");
        // An @ past the host is no part of the credentials, where a connection is made.
        assert_eq!(with_database("postgres://u@host/d@b", "x"), "postgres://u@host/x");
        assert_eq!(with_database("postgres://host/app?application_name=a@b", "x"), "postgres://host/x?application_name=a@b");
    }

    #[test]
    fn a_transaction_pooler_is_refused_with_the_direct_connection() {
        let neon = transaction_pooler("postgresql://neondb_owner:secret@ep-late-darkness-bb241gty-pooler.c-15.us-east-1.aws.neon.tech/neondb?sslmode=require&channel_binding=require", "direct_url").expect("Neon's pooled host");
        assert!(neon.ends_with("host ep-late-darkness-bb241gty.c-15.us-east-1.aws.neon.tech, in direct_url in .reylo.yaml or with --url"), "{neon}");
        assert!(neon.starts_with("postgresql://ep-late-darkness-bb241gty-pooler.c-15.us-east-1.aws.neon.tech/neondb is Neon's"), "{neon}");
        assert!(!neon.contains("secret"), "{neon}");
        let supabase = transaction_pooler("postgres://postgres.abc:secret@aws-1-us-east-1.pooler.supabase.com:6543/postgres", "shadow_url").expect("Supabase's transaction pooler");
        assert!(supabase.contains("port 5432") && supabase.ends_with("in shadow_url in .reylo.yaml") && !supabase.contains("secret"), "{supabase}");
        // The direct connections, Supabase's session pooler and any other host are no transaction pooler.
        for url in [
            "postgresql://neondb_owner:secret@ep-late-darkness-bb241gty.c-15.us-east-1.aws.neon.tech/neondb?sslmode=require",
            "postgres://postgres.abc:secret@aws-1-us-east-1.pooler.supabase.com:5432/postgres",
            "postgres://postgres:secret@db.abc.supabase.co:5432/postgres",
            "postgres://u@my-pooler.example.com:6543/app",
            "postgres://u@localhost:5432/app",
            "postgres://u@[::1]:6543/app",
            "mysql://u@x-pooler.neon.tech/app",
        ] {
            assert_eq!(transaction_pooler(url, "direct_url"), None, "{url}");
        }
    }
}
