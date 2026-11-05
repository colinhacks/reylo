//! The databases `reylo migrate` works on: the target, through the connection code the type check
//! uses (`reylo-engine-native`), and the shadows it builds beside it. A shadow is a scratch database
//! on the target's own server (`create database`), so it has the target's engine, version and
//! extensions; SQLite's live in memory. The connection that creates a shadow holds a session lock
//! named after it for the whole run, and the lock dies with the session, so a run that is killed
//! leaves a shadow nobody holds, which the next run drops before it builds its own.

use crate::{Dialect, Error};
use mysql_async::prelude::Queryable;
use std::path::Path;

/// One row, every value as text (Postgres's booleans as `t` and `f`), as its columns name them.
#[derive(Clone, Debug, Default)]
pub struct Row {
    columns: Vec<String>,
    values: Vec<Option<String>>,
}

impl Row {
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
    Postgres(tokio_postgres::Client),
    Mysql(mysql_async::Conn),
    Sqlite(rusqlite::Connection),
}

pub(crate) fn pg_error(e: tokio_postgres::Error) -> Error {
    match e.as_db_error() {
        Some(d) => {
            let mut message = d.message().to_string();
            if let Some(detail) = d.detail() {
                message = format!("{message} ({detail})");
            }
            Error(message)
        }
        None => Error(e.to_string()),
    }
}

pub(crate) fn mysql_error(e: mysql_async::Error) -> Error {
    match e {
        mysql_async::Error::Server(s) => Error(s.message),
        other => Error(other.to_string()),
    }
}

pub(crate) fn sqlite_error(e: rusqlite::Error) -> Error {
    Error(e.to_string())
}

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
pub(crate) async fn open_postgres(url: &str) -> Result<tokio_postgres::Client, Error> {
    reylo_engine_native::postgres::open(url).await.map(|(client, _)| client).map_err(|failure| Error(failure.to_string()))
}

/// A connection to the MySQL database a URL names, as the engine opens one.
pub(crate) async fn open_mysql(url: &str) -> Result<mysql_async::Conn, Error> {
    reylo_engine_native::mysql::open(url).await.map(|(conn, _)| conn).map_err(|failure| Error(failure.to_string()))
}

impl Db {
    pub async fn connect(url: &str, base: &Path) -> Result<Db, Error> {
        match Dialect::of_url(url)? {
            Dialect::Postgres => Ok(Db::Postgres(crate::db::open_postgres(url).await?)),
            Dialect::Mysql => Ok(Db::Mysql(crate::db::open_mysql(url).await?)),
            Dialect::Sqlite => Ok(Db::Sqlite(rusqlite::Connection::open(sqlite_path(url, base)).map_err(sqlite_error)?)),
        }
    }

    pub fn dialect(&self) -> Dialect {
        match self {
            Db::Postgres(_) => Dialect::Postgres,
            Db::Mysql(_) => Dialect::Mysql,
            Db::Sqlite(_) => Dialect::Sqlite,
        }
    }

    /// The rows of the statements in `sql`, run as written (Postgres's simple protocol, MySQL's text
    /// protocol), every value as text.
    pub async fn rows(&mut self, sql: &str) -> Result<Vec<Row>, Error> {
        match self {
            Db::Postgres(client) => pg_rows(client, sql).await,
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
        }
    }

    /// Runs `sql` as written: one statement or several.
    pub async fn exec(&mut self, sql: &str) -> Result<(), Error> {
        match self {
            Db::Postgres(client) => client.batch_execute(sql).await.map_err(pg_error),
            Db::Mysql(conn) => conn.query_drop(sql).await.map_err(mysql_error),
            Db::Sqlite(conn) => conn.execute_batch(sql).map_err(sqlite_error),
        }
    }

    /// Runs a script of statements: on MySQL one at a time, since a server that takes several in
    /// one query reads them as one batch; elsewhere as written.
    pub async fn script(&mut self, sql: &str) -> Result<(), Error> {
        if self.dialect() != Dialect::Mysql {
            return self.exec(sql).await;
        }
        for statement in crate::sql::split_statements(sql, Dialect::Mysql) {
            self.exec(&statement.text).await?;
        }
        Ok(())
    }

    /// Runs one statement with text parameters, `$1` on Postgres and `?` elsewhere.
    pub async fn exec_with(&mut self, sql: &str, params: &[&str]) -> Result<(), Error> {
        match self {
            Db::Postgres(client) => {
                let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = params.iter().map(|p| p as &(dyn tokio_postgres::types::ToSql + Sync)).collect();
                client.execute(sql, &params).await.map(|_| ()).map_err(pg_error)
            }
            Db::Mysql(conn) => {
                let values: Vec<mysql_async::Value> = params.iter().map(|p| mysql_async::Value::from(*p)).collect();
                conn.exec_drop(sql, mysql_async::Params::Positional(values)).await.map_err(mysql_error)
            }
            Db::Sqlite(conn) => conn.execute(sql, rusqlite::params_from_iter(params.iter())).map(|_| ()).map_err(sqlite_error),
        }
    }

    pub async fn close(self) {
        if let Db::Mysql(conn) = self {
            let _ = conn.disconnect().await;
        }
    }

    /// The parameter marker for the `n`th parameter.
    pub fn param(&self, n: usize) -> String {
        match self {
            Db::Postgres(_) => format!("${n}"),
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

pub(crate) fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub struct Shadow {
    pub db: Db,
    pub name: String,
}

/// Where shadows are made: the target's server, or the one `shadow_url` names.
pub enum Server {
    Postgres {
        admin: tokio_postgres::Client,
        url: String,
        /// What follows `create database "<name>"`: the target's encoding and locale, where they are not template1's.
        create: String,
        /// The target database's own settings (`alter database … set`), such as a search_path, which
        /// decide where an unqualified name goes.
        settings: Vec<String>,
        /// The target's default privileges (`alter default privileges`), which grant on what a
        /// migration creates, as a platform's do (Supabase's grant every new table to its API's
        /// roles); made again in each shadow unless the project sets its own.
        privileges: Vec<String>,
        copy_privileges: bool,
    },
    Mysql {
        admin: mysql_async::Conn,
        url: String,
        /// The target database's default character set and collation, which every table created without its own inherits.
        defaults: Option<(String, String)>,
    },
    Sqlite,
}

const PG_LISTS: &[&str] = &["search_path", "temp_tablespaces", "local_preload_libraries", "session_preload_libraries"];

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
                let locale = if version >= 170000 { "datlocale" } else { "daticulocale" };
                let props = |filter: &str| {
                    format!("select pg_encoding_to_char(encoding) as enc, datcollate as collate, datctype as ctype, datlocprovider as provider, {locale} as locale from pg_database where {filter}")
                };
                let mine = target.rows(&props("datname = current_database()")).await?.into_iter().next();
                let template = pg_rows(&admin, &props("datname = 'template1'")).await?.into_iter().next();
                let key = |r: &Row| ["enc", "collate", "ctype", "provider", "locale"].map(|k| r.get(k).map(String::from));
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
                        format!(
                            " template template0 encoding {} lc_collate {} lc_ctype {} locale_provider {provider}{locale}",
                            literal(m.text("enc")),
                            literal(m.text("collate")),
                            literal(m.text("ctype"))
                        )
                    }
                    _ => String::new(),
                };
                let settings = target
                    .rows("select unnest(setconfig) as s from pg_db_role_setting where setrole = 0 and setdatabase = (select oid from pg_database where datname = current_database())")
                    .await?
                    .iter()
                    .filter_map(|r| r.get("s").map(String::from))
                    .collect();
                let privileges = default_privileges(target).await?;
                Ok(Server::Postgres { admin, url: server_url.to_string(), create, settings, privileges, copy_privileges: true })
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
            Dialect::Sqlite => Ok(Server::Sqlite),
        }
    }

    pub fn dialect(&self) -> Dialect {
        match self {
            Server::Postgres { .. } => Dialect::Postgres,
            Server::Mysql { .. } => Dialect::Mysql,
            Server::Sqlite => Dialect::Sqlite,
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
                admin.query_drop(format!("create database `{name}`{charset}")).await.map_err(mysql_error)?;
                let db = Db::Mysql(crate::db::open_mysql(&with_database(url, &name)).await?);
                Ok(Shadow { db, name })
            }
            Server::Sqlite => Ok(Shadow { db: Db::Sqlite(rusqlite::Connection::open_in_memory().map_err(sqlite_error)?), name: ":memory:".into() }),
        }
    }

    /// A Postgres shadow: a new database made as the target's, or a copy of `template`, a replay
    /// cache, which already holds what a new shadow is given (its encoding, locale and default
    /// privileges) but not its database settings, which a copy does not take. The lock named after
    /// it is held until it is dropped; a copy that fails releases it.
    pub(crate) async fn pg_shadow(&mut self, template: Option<&str>) -> Result<Shadow, Error> {
        let Server::Postgres { admin, url, create, settings, privileges, copy_privileges } = self else { return Err(Error("not a Postgres server".into())) };
        let name = random_name();
        admin.batch_execute(&format!("select pg_advisory_lock(hashtextextended('{name}', 0))")).await.map_err(pg_error)?;
        let made = async {
            match template {
                Some(t) => admin.batch_execute(&format!("create database \"{name}\" template \"{t}\"")).await.map_err(pg_error)?,
                None => admin.batch_execute(&format!("create database \"{name}\"{create}")).await.map_err(pg_error)?,
            }
            for s in settings.iter() {
                let Some((key, value)) = s.split_once('=') else { continue };
                let value = if PG_LISTS.contains(&key) {
                    value.split(',').map(|v| literal(v.trim().trim_matches('"'))).collect::<Vec<_>>().join(", ")
                } else {
                    literal(value)
                };
                admin.batch_execute(&format!("alter database \"{name}\" set {key} = {value}")).await.map_err(pg_error)?;
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
        if *copy_privileges && template.is_none() {
            // A role or schema the shadow's server lacks leaves that one out; the comparison says what follows.
            for statement in privileges.iter() {
                let _ = db.exec(statement).await;
            }
        }
        Ok(Shadow { db, name })
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
                let _ = admin.query_drop(format!("select release_lock('{name}')")).await;
            }
            Server::Sqlite => {}
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
                    admin.batch_execute(&format!("drop database if exists \"{name}\" with (force)")).await.map_err(pg_error)?;
                    admin.batch_execute(&format!("select pg_advisory_unlock(hashtextextended('{name}', 0))")).await.map_err(pg_error)?;
                    dropped += 1;
                }
            }
            Server::Mysql { admin, .. } => {
                let names: Vec<String> = admin.query(format!("select schema_name from information_schema.schemata where schema_name like '{SHADOW_PREFIX}%'")).await.map_err(mysql_error)?;
                for name in names {
                    let free: Option<i64> = admin.query_first(format!("select is_free_lock('{name}')")).await.map_err(mysql_error)?;
                    if free != Some(1) {
                        continue;
                    }
                    admin.query_drop(format!("drop database if exists `{name}`")).await.map_err(mysql_error)?;
                    dropped += 1;
                }
            }
            Server::Sqlite => {}
        }
        // The replay caches no run will use, which are not counted: they are not a run's leftovers.
        crate::cache::sweep(self).await?;
        Ok(dropped)
    }

    pub async fn close(self) {
        if let Server::Mysql { admin, .. } = self {
            let _ = admin.disconnect().await;
        }
    }
}

/// Ends a Postgres shadow's session, and waits up to a second for its server process to go: a
/// database with a session left cannot be dropped, renamed or copied without the server's own wait
/// for it, a tenth of a second at a time.
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
/// default, for the roles and in the schemas they name (the whole database where none).
async fn default_privileges(target: &mut Db) -> Result<Vec<String>, Error> {
    let rows = target
        .rows(
            "select format('%s', d.defaclrole::regrole) as role, case when d.defaclnamespace <> 0 then format('%I', n.nspname) end as schema, d.defaclobjtype as kind,
              format('%s', (select json_agg(json_build_object('grantee', case when x.grantee = 0 then 'public' else quote_ident(pg_get_userbyid(x.grantee)) end, 'privilege', x.privilege_type, 'grantable', x.is_grantable))
                 from aclexplode(d.defaclacl) x where x.grantee <> d.defaclrole)) as acl
            from pg_default_acl d left join pg_namespace n on n.oid = d.defaclnamespace",
        )
        .await?;
    let mut out = Vec::new();
    for r in rows {
        let objects = match r.text("kind") {
            "r" => "tables",
            "S" => "sequences",
            "f" => "functions",
            "T" => "types",
            "n" => "schemas",
            _ => continue,
        };
        let head = format!("alter default privileges for role {}{}", r.text("role"), r.get("schema").map(|s| format!(" in schema {s}")).unwrap_or_default());
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

/// Drops every object in the database that its migrations could have made, Reylo's history table
/// included: on Postgres the schemas the connection's role owns and everything in `public`, which is
/// made again as a new database has it; on MySQL every table, view, routine and event; on SQLite
/// every table, view, index and trigger.
pub async fn wipe(db: &mut Db) -> Result<(), Error> {
    match db.dialect() {
        Dialect::Postgres => {
            let schemas = db
                .rows(
                    r"select n.nspname from pg_namespace n
                    where n.nspname not in ('pg_catalog', 'information_schema', 'public') and n.nspname not like 'pg\_%'
                      and pg_get_userbyid(n.nspowner) = current_user
                      and not exists (select 1 from pg_depend d where d.classid = 'pg_namespace'::regclass and d.objid = n.oid and d.deptype = 'e')",
                )
                .await?;
            for row in schemas {
                db.exec(&format!("drop schema \"{}\" cascade", row.text("nspname").replace('"', "\"\""))).await?;
            }
            // `public` is made again as a new database has it: its owner, its grant and its comment.
            let comment = db.rows("select obj_description(oid, 'pg_namespace') as c from pg_namespace where nspname = 'public'").await?.first().and_then(|r| r.get("c").map(String::from));
            db.exec("drop schema if exists public cascade; create schema public; alter schema public owner to pg_database_owner; grant usage on schema public to public").await?;
            if let Some(comment) = comment {
                db.exec(&format!("comment on schema public is {}", literal(&comment))).await?;
            }
            Ok(())
        }
        Dialect::Mysql => {
            let objects = db
                .rows(
                    "select table_type as kind, table_name as name from information_schema.tables where table_schema = database()
                    union all select routine_type, routine_name from information_schema.routines where routine_schema = database()
                    union all select 'EVENT', event_name from information_schema.events where event_schema = database()",
                )
                .await?;
            db.exec("set foreign_key_checks = 0").await?;
            for o in objects {
                let kind = match o.text("kind") {
                    "VIEW" => "view".to_string(),
                    "BASE TABLE" => "table".to_string(),
                    other => other.to_ascii_lowercase(),
                };
                db.exec(&format!("drop {kind} if exists `{}`", o.text("name").replace('`', "``"))).await?;
            }
            db.exec("set foreign_key_checks = 1").await
        }
        Dialect::Sqlite => {
            let objects = db
                .rows("select type, name from sqlite_schema where name not like 'sqlite_%' and type in ('view', 'trigger', 'table', 'index') order by case type when 'view' then 0 when 'trigger' then 1 when 'index' then 2 else 3 end")
                .await?;
            db.exec("pragma foreign_keys = off").await?;
            for o in objects {
                let _ = db.exec(&format!("drop {} if exists \"{}\"", o.text("type"), o.text("name").replace('"', "\"\""))).await;
            }
            db.exec("pragma foreign_keys = on").await
        }
    }
}

#[cfg(test)]
mod url_tests {
    use super::*;

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
        // An @ past the host is no part of the credentials, where a connection is made.
        assert_eq!(with_database("postgres://u@host/d@b", "x"), "postgres://u@host/x");
        assert_eq!(with_database("postgres://host/app?application_name=a@b", "x"), "postgres://host/x?application_name=a@b");
    }
}
