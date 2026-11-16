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
                Ok(Server::Postgres { admin, url: server_url.to_string(), create, settings, privileges, copy_privileges: true, extensions, leave: vec![], environment, environment_role, schema_grants })
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
        let Server::Postgres { admin, url, create, settings, privileges, copy_privileges, extensions, leave, environment, environment_role, schema_grants } = self else { return Err(Error("not a Postgres server".into())) };
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
            let copied: Vec<&(String, String)> = extensions.iter().filter(|(e, _)| !leave.contains(&e.to_lowercase())).collect();
            let platform_schema = |schema: &str| environment.iter().any(|st| st.trim_start().to_ascii_lowercase().starts_with(&format!("create schema {}", schema.to_ascii_lowercase())));
            for (extension, schema) in copied.iter().filter(|(_, s)| !platform_schema(s)) {
                let _ = db.exec(&format!("create schema if not exists {}", ident(schema))).await;
                let _ = db.exec(&format!("create extension if not exists {} with schema {}", ident(extension), ident(schema))).await;
            }
            for statement in schema_grants.iter() {
                let _ = db.exec(statement).await;
            }
            if let (Some(bare), false) = (environment_role.as_deref(), environment.is_empty()) {
                let role = ident(bare);
                let _ = admin.batch_execute(&format!("grant create on database \"{name}\" to {role}")).await;
                // For the while it makes them, the schemas it may not use yet, given back after.
                let usage = format!(
                    "select format('grant usage on schema %I to %s', nspname, {}) as s from pg_namespace where nspname not like 'pg\\_%' and nspname <> 'information_schema' and not has_schema_privilege({}, oid, 'USAGE')",
                    literal(&role),
                    literal(bare)
                );
                let grants: Vec<String> = db.rows(&usage).await.unwrap_or_default().iter().map(|r| r.text("s").to_string()).collect();
                for g in &grants {
                    let _ = db.exec(g).await;
                }
                let _ = db.exec(&format!("set role {role}")).await;
                let _ = db.exec("set check_function_bodies = off").await;
                for statement in environment.iter() {
                    let _ = db.exec(statement).await;
                }
                let _ = db.exec("reset check_function_bodies").await;
                let _ = db.exec("reset role").await;
                for g in &grants {
                    let _ = db.exec(&g.replacen("grant usage on", "revoke usage on", 1).replacen(" to ", " from ", 1)).await;
                }
                let _ = admin.batch_execute(&format!("revoke create on database \"{name}\" from {role}")).await;
            }
            for (extension, schema) in copied.iter().filter(|(_, s)| platform_schema(s)) {
                let _ = db.exec(&format!("create extension if not exists {} with schema {}", ident(extension), ident(schema))).await;
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

    /// The extensions the project's own files create, which a shadow leaves to them.
    pub fn leave_extensions(&mut self, names: Vec<String>) {
        if let Server::Postgres { leave, .. } = self {
            *leave = names;
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
/// default, for the roles and in the schemas they name (the whole database where none). Those of the
/// role that runs the migrations name none, so that they are the defaults of whoever runs them: a
/// migration made by a developer's role runs in production as the deploy's.
pub(crate) async fn default_privileges(target: &mut Db) -> Result<Vec<String>, Error> {
    let rows = target
        .rows(
            &format!("select format('%s', d.defaclrole::regrole) as role, d.defaclrole = {} as mine,
              case when d.defaclnamespace <> 0 then format('%I', n.nspname) end as schema, d.defaclobjtype as kind,
              format('%s', (select json_agg(json_build_object('grantee', case when x.grantee = 0 then 'public' else quote_ident(pg_get_userbyid(x.grantee)) end, 'privilege', x.privilege_type, 'grantable', x.is_grantable))
                 from aclexplode(d.defaclacl) x where x.grantee <> d.defaclrole)) as acl
            from pg_default_acl d left join pg_namespace n on n.oid = d.defaclnamespace", crate::catalog::runner()),
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
