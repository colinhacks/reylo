//! The Postgres program: connections over tokio-postgres, opened from a libpq URL with libpq's
//! defaults from the environment (`PGHOST`, `PGUSER`, `PGPASSWORD`, …), with TLS as its `sslmode`,
//! `sslrootcert`, `sslcert` and `sslkey` ask (crate::tls), and each statement analyzed over one by
//! the replay loop of `reylo-engine`, which keeps the connection's answers between statements. A
//! connection the server closed while it sat idle (a restart, an idle timeout) is opened again for
//! the request that finds it closed.

use crate::tls::{self, Check};
use crate::Program;
use reylo_engine::clients::tokio_postgres::{self as answers, Describe};
use reylo_engine::{AnswerCache, AsyncAnswerer, ReplayError};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tokio_postgres::config::SslMode;
use tokio_postgres::{Client, Config};

/// How long a connection attempt may take, as the type check's describer allows: the type check
/// waits on it, in the editor and in CI.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The URL parameters tokio-postgres reads itself. libpq's TLS settings are read here (`sslmode`,
/// `sslrootcert`, `sslcert`, `sslkey`, `sslpassword`); any other parameter is left out, since
/// tokio-postgres refuses a parameter it does not know.
const KNOWN: &[&str] = &[
    "user", "password", "dbname", "options", "application_name", "sslnegotiation", "host", "hostaddr", "port", "connect_timeout", "tcp_user_timeout", "keepalives",
    "keepalives_idle", "keepalives_interval", "keepalives_retries", "target_session_attrs", "channel_binding", "load_balance_hosts",
];

#[derive(Default)]
pub struct Postgres {
    connections: HashMap<u64, Connection>,
    next: u64,
}

struct Connection {
    client: Client,
    /// The URL it was opened from, to open it again.
    url: String,
    /// The engine's answers, kept between statements as the loader's session keeps them.
    answers: AnswerCache<Describe>,
}

impl Connection {
    /// The client opened again where the server closed it. The program learns of a closed
    /// connection only from the request that finds it so, since its connection's task runs only
    /// while a request does: that request is answered over the new one.
    async fn reopen(&mut self) -> Result<(), Value> {
        self.client = connect(&self.url).await?;
        Ok(())
    }
}

impl Program for Postgres {
    fn call(&self, request: &str) -> String {
        reylo_engine_core::api::call::<reylo_engine::postgres::Postgres>(request)
    }

    async fn serve(&mut self, request: &Value) -> Option<Value> {
        let op = request["op"].as_str()?;
        let conn = request["conn"].as_u64().unwrap_or(0);
        let sql = request["sql"].as_str().unwrap_or_default();
        Some(match op {
            "open" => {
                let url = request["url"].as_str().unwrap_or_default();
                match connect(url).await {
                    Ok(client) => {
                        self.next += 1;
                        self.connections.insert(self.next, Connection { client, url: url.to_string(), answers: AnswerCache::default() });
                        json!({ "status": "opened", "conn": self.next })
                    }
                    Err(failure) => failure,
                }
            }
            "run" => match self.connections.get_mut(&conn) {
                None => closed(conn),
                Some(c) => {
                    let mut result = answers::analyze(&c.client, sql, &mut c.answers).await;
                    if matches!(&result, Err(ReplayError::Host(e)) if c.client.is_closed() || e.0.starts_with(answers::ENDED)) {
                        if let Err(failure) = c.reopen().await {
                            return Some(failure);
                        }
                        result = answers::analyze(&c.client, sql, &mut c.answers).await;
                    }
                    match result {
                        Ok(analysis) => json!({ "status": "analysis", "analysis": analysis }),
                        // The connection went while the engine asked: the database cannot be reached.
                        Err(ReplayError::Host(e)) if c.client.is_closed() || e.0.starts_with("the describe did not reach the server") || e.0.starts_with(answers::ENDED) => lost(&e.0),
                        Err(e) => json!({ "status": "error", "message": e.to_string() }),
                    }
                }
            },
            // A statement described on the connection, as the engine's own describe reads it: for a
            // host that runs the engine's analysis over this connection itself (EngineCore's `analyze`).
            "describe" => match self.connections.get_mut(&conn) {
                None => closed(conn),
                Some(c) => {
                    let mut result = c.client.prepare(sql).await;
                    if matches!(&result, Err(e) if c.client.is_closed() || answers::ended(e)) {
                        if let Err(failure) = c.reopen().await {
                            return Some(failure);
                        }
                        result = c.client.prepare(sql).await;
                    }
                    match result {
                        Ok(stmt) => json!({
                            "status": "described",
                            "params": stmt.params().iter().map(tokio_postgres::types::Type::oid).collect::<Vec<_>>(),
                            "columns": stmt.columns().iter().map(|col| json!({ "name": col.name(), "oid": col.type_().oid(), "table": col.table_oid().unwrap_or(0), "attnum": col.column_id().map_or(0, i32::from) })).collect::<Vec<_>>(),
                        }),
                        Err(e) => failure(&e),
                    }
                }
            },
            // A query on the connection, its parameters bound and its values read as the engine's
            // catalog queries are (the schema's fingerprint, and a host's own catalog queries).
            "query" => match self.connections.get_mut(&conn) {
                None => closed(conn),
                Some(c) => {
                    let params = request["params"].as_array().cloned().unwrap_or_default();
                    let mut result = answers::Answers(&c.client).query(sql, &params).await;
                    if matches!(&result, Err(e) if c.client.is_closed() || e.0.starts_with(answers::ENDED)) {
                        if let Err(failure) = c.reopen().await {
                            return Some(failure);
                        }
                        result = answers::Answers(&c.client).query(sql, &params).await;
                    }
                    match result {
                        Ok(Ok(rows)) => json!({ "status": "rows", "rows": rows }),
                        Ok(Err(e)) => json!({ "status": "error", "message": e.message, "code": e.code }),
                        Err(e) if c.client.is_closed() || e.0.starts_with(answers::ENDED) => lost(&e.0),
                        Err(e) => json!({ "status": "error", "message": e.0 }),
                    }
                }
            },
            // The schema changed: what the engine was told about the old one is stale.
            "forget" => match self.connections.get_mut(&conn) {
                None => closed(conn),
                Some(c) => {
                    c.answers.clear();
                    json!({ "status": "ok" })
                }
            },
            "close" => {
                self.connections.remove(&conn);
                json!({ "status": "ok" })
            }
            _ => return None,
        })
    }
}

fn closed(conn: u64) -> Value {
    json!({ "status": "unreachable", "message": format!("connection {conn} is closed") })
}

/// A connection lost while the engine asked: the database cannot be reached. The host's message is
/// its first line, without the catalog query under it.
fn lost(message: &str) -> Value {
    json!({ "status": "unreachable", "message": message.lines().next().unwrap_or_default() })
}

/// A connection to the database a libpq URL names.
async fn connect(url: &str) -> Result<Client, Value> {
    let Parsed { config, check, root, identity } = parse(url, &|key| std::env::var(key).ok()).map_err(|message| json!({ "status": "error", "message": message }))?;
    let identity = identity.as_ref().map(|(cert, key, password)| tls::Identity { cert, key, password: *password });
    let tls = tls::config(check, root.as_deref(), identity).map_err(|message| json!({ "status": "error", "message": message }))?;
    let connecting = config.connect(tokio_postgres_rustls::MakeRustlsConnect::new(tls));
    let (client, connection) = match tokio::time::timeout(config.get_connect_timeout().copied().unwrap_or(CONNECT_TIMEOUT) + Duration::from_secs(1), connecting).await {
        Err(_) => return Err(json!({ "status": "unreachable", "message": "no answer within the connect timeout" })),
        Ok(Err(e)) => return Err(failure(&e)),
        Ok(Ok(pair)) => pair,
    };
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(client)
}

/// A libpq URL read: tokio-postgres's configuration, how much of the server's certificate to check,
/// the root certificate file to check it against, and the client certificate to present (its file,
/// its key's, and whether `sslpassword` was given).
struct Parsed {
    config: Config,
    check: Check,
    root: Option<String>,
    identity: Option<(String, String, bool)>,
}

/// A libpq URL as tokio-postgres's configuration, what it leaves out read from the environment as
/// libpq reads it (`env`): the host (`PGHOST`, else `localhost`, where postgres.js connects too),
/// the port, the user (`PGUSER`, else the system's), the password, the database and the TLS settings.
fn parse(url: &str, env: &dyn Fn(&str) -> Option<String>) -> Result<Parsed, String> {
    let env = |key: &str| env(key).filter(|v| !v.is_empty());
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    let (mut mode, mut root, mut binding, mut cert, mut key_file, mut password) = (None, None, None, None, None, false);
    let mut kept = Vec::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "sslmode" => mode = Some(crate::decode(value)),
            "sslrootcert" => root = Some(crate::decode(value)),
            "sslcert" => cert = Some(crate::decode(value)),
            "sslkey" => key_file = Some(crate::decode(value)),
            "sslpassword" => password = true,
            "channel_binding" => {
                binding = Some(crate::decode(value));
                kept.push(pair);
            }
            key if KNOWN.contains(&key) => kept.push(pair),
            _ => {}
        }
    }
    let mode = mode.or_else(|| env("PGSSLMODE"));
    let root = root.or_else(|| env("PGSSLROOTCERT"));
    let identity = match (cert.or_else(|| env("PGSSLCERT")), key_file.or_else(|| env("PGSSLKEY"))) {
        (Some(cert), Some(key)) => Some((cert, key, password)),
        (None, None) => None,
        _ => return Err("sslcert and sslkey name a client certificate together: name both".to_string()),
    };
    let system = root.as_deref() == Some("system");
    let file = root.filter(|r| r != "system");
    // As libpq: `sslrootcert=system` makes the default `verify-full`, and a root certificate file
    // makes `require` check the chain like `verify-ca`.
    let (ssl, check) = match mode.as_deref().unwrap_or(if system { "verify-full" } else { "prefer" }) {
        "disable" => (SslMode::Disable, Check::Nothing),
        "allow" | "prefer" => (SslMode::Prefer, Check::Nothing),
        "require" => (SslMode::Require, if system { Check::Full } else if file.is_some() { Check::Chain } else { Check::Nothing }),
        "verify-ca" => (SslMode::Require, if system { Check::Full } else { Check::Chain }),
        "verify-full" => (SslMode::Require, Check::Full),
        other => return Err(format!("sslmode={other} is none of disable, allow, prefer, require, verify-ca and verify-full")),
    };
    // Channel binding needs TLS to bind to.
    let ssl = if binding.as_deref() == Some("require") { SslMode::Require } else { ssl };
    let text = if kept.is_empty() { base.to_string() } else { format!("{base}?{}", kept.join("&")) };
    let mut config: Config = text.parse().map_err(|e: tokio_postgres::Error| format!("cannot read the URL: {e}"))?;
    if config.get_hosts().is_empty() && config.get_hostaddrs().is_empty() {
        for host in env("PGHOST").unwrap_or_else(|| "localhost".to_string()).split(',') {
            config.host(host);
        }
    }
    if config.get_ports().is_empty() {
        if let Some(port) = env("PGPORT") {
            config.port(port.parse().map_err(|_| format!("PGPORT={port} is not a port"))?);
        }
    }
    if config.get_user().is_none() {
        if let Some(user) = env("PGUSER").or_else(|| env("USER")).or_else(|| env("USERNAME")) {
            config.user(user);
        }
    }
    if config.get_password().is_none() {
        if let Some(password) = env("PGPASSWORD") {
            config.password(password);
        }
    }
    if config.get_dbname().is_none() {
        if let Some(dbname) = env("PGDATABASE") {
            config.dbname(dbname);
        }
    }
    // WASI has sockets for TCP and UDP alone: a Unix socket is opened by the host, which bridges it
    // over TCP (src/host-url.ts), so one that reaches the component is refused by name.
    #[cfg(target_family = "wasm")]
    for host in config.get_hosts() {
        let tokio_postgres::config::Host::Tcp(name) = host;
        if name.starts_with('/') {
            return Err(format!("host={name} is a Unix socket's directory, which this engine cannot open: connect over TCP, as host=localhost"));
        }
    }
    config.ssl_mode(ssl);
    if config.get_connect_timeout().is_none() {
        config.connect_timeout(CONNECT_TIMEOUT);
    }
    Ok(Parsed { config, check, root: file, identity })
}

/// What a failed call means to the host: a refused login (class 28), a database that cannot be
/// reached (no answer from the server, class 08, or a FATAL error such as a database that does not
/// exist), a TLS handshake that failed (a setting to fix), or the server's error for what it was
/// asked.
fn failure(e: &tokio_postgres::Error) -> Value {
    match e.as_db_error() {
        Some(d) => {
            let code = d.code().code();
            let status = if code.starts_with("28") {
                "refused"
            } else if code.starts_with("08") || matches!(d.severity(), "FATAL" | "PANIC") {
                "unreachable"
            } else {
                "error"
            };
            json!({ "status": status, "message": d.message(), "code": code })
        }
        None => {
            // The driver's own words, with the operating system's reason under them.
            let mut message = e.to_string();
            let mut source = std::error::Error::source(e);
            while let Some(s) = source {
                let text = s.to_string();
                if !message.contains(&text) {
                    message = format!("{message}: {text}");
                }
                source = s.source();
            }
            json!({ "status": if tls::failed(e) { "error" } else { "unreachable" }, "message": message })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn a_url_keeps_what_tokio_postgres_reads_and_reads_the_tls_parameters_itself() {
        let Parsed { config, check, root, identity } = parse("postgres://app:p%40ss@db.example.com:5433/app?sslmode=verify-ca&sslrootcert=%2Ftmp%2Froot.pem&sslcert=%2Ftmp%2Fc.pem&sslkey=%2Ftmp%2Fc.key&sslpassword=x&application_name=x", &no_env).unwrap();
        assert_eq!(config.get_ssl_mode(), SslMode::Require);
        assert_eq!(check, Check::Chain);
        assert_eq!(root.as_deref(), Some("/tmp/root.pem"));
        assert_eq!(identity, Some(("/tmp/c.pem".to_string(), "/tmp/c.key".to_string(), true)));
        assert_eq!(config.get_application_name(), Some("x"));
        assert_eq!(config.get_password(), Some(&b"p@ss"[..]));
        assert_eq!(config.get_connect_timeout(), Some(&CONNECT_TIMEOUT));
        assert!(parse("postgres://h/d?sslcert=c.pem", &no_env).is_err());
    }

    #[test]
    fn what_the_url_leaves_out_is_read_from_the_environment_as_libpq_reads_it() {
        let env = |key: &str| match key {
            "PGHOST" => Some("db.internal".to_string()),
            "PGPORT" => Some("6543".to_string()),
            "PGUSER" => Some("ci".to_string()),
            "PGPASSWORD" => Some("secret".to_string()),
            "PGDATABASE" => Some("app".to_string()),
            "PGSSLMODE" => Some("require".to_string()),
            _ => None,
        };
        let Parsed { config, .. } = parse("postgres:///", &env).unwrap();
        assert_eq!(config.get_hosts(), &[tokio_postgres::config::Host::Tcp("db.internal".to_string())]);
        assert_eq!(config.get_ports(), &[6543]);
        assert_eq!(config.get_user(), Some("ci"));
        assert_eq!(config.get_password(), Some(&b"secret"[..]));
        assert_eq!(config.get_dbname(), Some("app"));
        assert_eq!(config.get_ssl_mode(), SslMode::Require);
        // The URL's own settings come first, and a URL with no host connects to localhost.
        let Parsed { config, .. } = parse("postgres://u:p@h:1/d?sslmode=disable", &env).unwrap();
        assert_eq!((config.get_user(), config.get_ports(), config.get_ssl_mode()), (Some("u"), &[1u16][..], SslMode::Disable));
        let Parsed { config, .. } = parse("postgres:///d", &|key| (key == "USER").then(|| "me".to_string())).unwrap();
        assert_eq!((config.get_hosts(), config.get_user()), (&[tokio_postgres::config::Host::Tcp("localhost".to_string())][..], Some("me")));
    }

    #[test]
    fn the_tls_modes_check_what_libpq_checks() {
        let check = |url: &str| parse(url, &no_env).map(|p| (p.config.get_ssl_mode(), p.check)).unwrap();
        assert_eq!(check("postgres://h/d"), (SslMode::Prefer, Check::Nothing));
        assert_eq!(check("postgres://h/d?sslmode=disable"), (SslMode::Disable, Check::Nothing));
        assert_eq!(check("postgres://h/d?sslmode=require"), (SslMode::Require, Check::Nothing));
        assert_eq!(check("postgres://h/d?sslmode=require&sslrootcert=r.pem"), (SslMode::Require, Check::Chain));
        assert_eq!(check("postgres://h/d?sslmode=verify-full"), (SslMode::Require, Check::Full));
        assert_eq!(check("postgres://h/d?sslrootcert=system"), (SslMode::Require, Check::Full));
        assert_eq!(check("postgres://h/d?channel_binding=require"), (SslMode::Require, Check::Nothing));
        assert!(parse("postgres://h/d?sslmode=sometimes", &no_env).is_err());
    }
}
