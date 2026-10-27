//! The Postgres program: connections over tokio-postgres, opened from a libpq URL, with TLS as its
//! `sslmode` and `sslrootcert` ask (crate::tls), and each statement analyzed over one by the replay
//! loop of `reylo-engine`, which keeps the connection's answers between statements.

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

/// The URL parameters tokio-postgres reads itself. libpq's TLS files are read here (`sslmode`,
/// `sslrootcert`); any other parameter, such as a client certificate (`sslcert`, `sslkey`), is
/// left out, since tokio-postgres refuses a parameter it does not know.
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
    /// The engine's answers, kept between statements as the loader's session keeps them.
    answers: AnswerCache<Describe>,
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
            "open" => match connect(request["url"].as_str().unwrap_or_default()).await {
                Ok(client) => {
                    self.next += 1;
                    self.connections.insert(self.next, Connection { client, answers: AnswerCache::default() });
                    json!({ "status": "opened", "conn": self.next })
                }
                Err(failure) => failure,
            },
            "run" => match self.connections.get_mut(&conn) {
                None => closed(conn),
                Some(c) => match answers::analyze(&c.client, sql, &mut c.answers).await {
                    Ok(analysis) => json!({ "status": "analysis", "analysis": analysis }),
                    // The connection went while the engine asked: the database cannot be reached.
                    Err(ReplayError::Host(e)) if c.client.is_closed() || e.0.starts_with("the describe did not reach the server") => json!({ "status": "unreachable", "message": e.0 }),
                    Err(e) => json!({ "status": "error", "message": e.to_string() }),
                },
            },
            // A statement described on the connection, as the engine's own describe reads it: for a
            // host that runs the engine's analysis over this connection itself (EngineCore's `analyze`).
            "describe" => match self.connections.get(&conn) {
                None => closed(conn),
                Some(c) => match c.client.prepare(sql).await {
                    Ok(stmt) => json!({
                        "status": "described",
                        "params": stmt.params().iter().map(tokio_postgres::types::Type::oid).collect::<Vec<_>>(),
                        "columns": stmt.columns().iter().map(|col| json!({ "name": col.name(), "oid": col.type_().oid(), "table": col.table_oid().unwrap_or(0), "attnum": col.column_id().map_or(0, i32::from) })).collect::<Vec<_>>(),
                    }),
                    Err(e) => failure(&e),
                },
            },
            // A query on the connection, its parameters bound and its values read as the engine's
            // catalog queries are (the schema's fingerprint, and a host's own catalog queries).
            "query" => match self.connections.get(&conn) {
                None => closed(conn),
                Some(c) => {
                    let params = request["params"].as_array().cloned().unwrap_or_default();
                    match answers::Answers(&c.client).query(sql, &params).await {
                        Ok(Ok(rows)) => json!({ "status": "rows", "rows": rows }),
                        Ok(Err(e)) => json!({ "status": "error", "message": e.message, "code": e.code }),
                        Err(e) if c.client.is_closed() => json!({ "status": "unreachable", "message": e.0 }),
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

/// A connection to the database a libpq URL names.
async fn connect(url: &str) -> Result<Client, Value> {
    let (config, check, root) = parse(url).map_err(|message| json!({ "status": "error", "message": message }))?;
    let tls = tls::config(check, root.as_deref()).map_err(|message| json!({ "status": "error", "message": message }))?;
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

/// A libpq URL as tokio-postgres's configuration, how much of the server's certificate to check,
/// and the root certificate file to check it against.
fn parse(url: &str) -> Result<(Config, Check, Option<String>), String> {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    let (mut mode, mut root, mut binding) = (None, None, None);
    let mut kept = Vec::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "sslmode" => mode = Some(crate::decode(value)),
            "sslrootcert" => root = Some(crate::decode(value)),
            "channel_binding" => {
                binding = Some(crate::decode(value));
                kept.push(pair);
            }
            key if KNOWN.contains(&key) => kept.push(pair),
            _ => {}
        }
    }
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
    config.ssl_mode(ssl);
    if config.get_connect_timeout().is_none() {
        config.connect_timeout(CONNECT_TIMEOUT);
    }
    Ok((config, check, file))
}

/// What a failed call means to the host: a refused login (class 28), a database that cannot be
/// reached (no answer from the server, class 08, or a FATAL error such as a database that does not
/// exist), or the server's error for what it was asked.
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
            json!({ "status": "unreachable", "message": message })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_keeps_what_tokio_postgres_reads_and_reads_the_tls_parameters_itself() {
        let (config, check, root) = parse("postgres://app:p%40ss@db.example.com:5433/app?sslmode=verify-ca&sslrootcert=%2Ftmp%2Froot.pem&sslcert=c.pem&application_name=x").unwrap();
        assert_eq!(config.get_ssl_mode(), SslMode::Require);
        assert_eq!(check, Check::Chain);
        assert_eq!(root.as_deref(), Some("/tmp/root.pem"));
        assert_eq!(config.get_application_name(), Some("x"));
        assert_eq!(config.get_password(), Some(&b"p@ss"[..]));
        assert_eq!(config.get_connect_timeout(), Some(&CONNECT_TIMEOUT));
    }

    #[test]
    fn the_tls_modes_check_what_libpq_checks() {
        let check = |url: &str| parse(url).map(|(c, check, _)| (c.get_ssl_mode(), check)).unwrap();
        assert_eq!(check("postgres://h/d"), (SslMode::Prefer, Check::Nothing));
        assert_eq!(check("postgres://h/d?sslmode=disable"), (SslMode::Disable, Check::Nothing));
        assert_eq!(check("postgres://h/d?sslmode=require"), (SslMode::Require, Check::Nothing));
        assert_eq!(check("postgres://h/d?sslmode=require&sslrootcert=r.pem"), (SslMode::Require, Check::Chain));
        assert_eq!(check("postgres://h/d?sslmode=verify-full"), (SslMode::Require, Check::Full));
        assert_eq!(check("postgres://h/d?sslrootcert=system"), (SslMode::Require, Check::Full));
        assert_eq!(check("postgres://h/d?channel_binding=require"), (SslMode::Require, Check::Nothing));
        assert!(parse("postgres://h/d?sslmode=sometimes").is_err());
    }
}
