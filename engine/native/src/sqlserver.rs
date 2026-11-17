//! The SQL Server program: connections over tiberius, opened from a `sqlserver://` (or `mssql://`)
//! URL, and each statement analyzed over a connection by the replay loop of `reylo-engine`, which
//! keeps the connection's answers between statements. A connection the server dropped while it sat
//! idle is opened again for the request that finds it dropped.
//!
//! The URL is `sqlserver://user:password@host:port/database`, with the settings the `mssql` package
//! takes as options: `encrypt` (`true` by default, as the package's own default is; `false` encrypts
//! the login alone) and `trustServerCertificate` (`false` by default: the certificate is checked
//! against the operating system's roots). A login is SQL Server's own, by user and password.

use crate::{Failure, Program};
use reylo_engine::clients::tiberius::{self as answers};
use reylo_engine::{AnswerCache, ReplayError};
use reylo_engine_core::api::{Analysis, NoDescribe};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tiberius::{AuthMethod, Client, Config, EncryptionLevel};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

/// How long a connection attempt may take, as the `mssql` package's default `connectionTimeout`.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// The error a server answers a refused login with.
const LOGIN_FAILED: u32 = 18456;

/// A connection to SQL Server, as the program and the migrations open one.
pub type Conn = Client<Compat<TcpStream>>;

#[derive(Default)]
pub struct Sqlserver {
    connections: HashMap<u64, Connection>,
    next: u64,
}

struct Connection {
    client: Conn,
    /// The URL it was opened from, to open it again.
    url: String,
    /// The engine's answers, kept between statements as the loader's session keeps them.
    answers: AnswerCache<NoDescribe>,
}

impl Connection {
    /// Whether the connection is gone, after a call failed with something other than the server's
    /// answer.
    async fn lost(&mut self) -> bool {
        match self.client.simple_query("select 1").await {
            Ok(stream) => stream.into_results().await.is_err(),
            Err(_) => true,
        }
    }

    async fn reopen(&mut self) -> Result<(), Value> {
        self.client = connect(&self.url).await?.0;
        Ok(())
    }

    /// The analysis of one statement over the connection, opened again once where the server
    /// dropped it; a failure is the answer the program gives for it.
    async fn analyze(&mut self, sql: &str) -> Result<Analysis, Value> {
        let mut result = answers::analyze(&mut self.client, sql, &mut self.answers).await;
        if matches!(result, Err(ReplayError::Host(_))) && self.lost().await {
            self.reopen().await?;
            result = answers::analyze(&mut self.client, sql, &mut self.answers).await;
        }
        match result {
            Ok(analysis) => Ok(analysis),
            Err(ReplayError::Host(e)) if e.0.starts_with("the query did not reach the server") || self.lost().await => {
                Err(json!({ "status": "unreachable", "message": e.0.lines().next().unwrap_or_default() }))
            }
            Err(e) => Err(json!({ "status": "error", "message": e.to_string() })),
        }
    }
}

/// The statements of one command line's run, analyzed over one connection, which keeps the engine's
/// answers between them.
pub struct Session {
    connection: Connection,
    /// The server's product version, as `16.0.4315.2`.
    pub version: String,
}

impl Session {
    pub async fn open(url: &str) -> Result<Session, Failure> {
        let (client, version) = connect(url).await.map_err(|answer| Failure::of(&answer))?;
        Ok(Session { connection: Connection { client, url: url.to_string(), answers: AnswerCache::until_cleared() }, version })
    }

    pub async fn analyze(&mut self, sql: &str) -> Result<Analysis, Failure> {
        self.connection.analyze(sql).await.map_err(|answer| Failure::of(&answer))
    }
}

/// A connection to the server a URL names, and the server's product version.
pub async fn open(url: &str) -> Result<(Conn, String), Failure> {
    connect(url).await.map_err(|answer| Failure::of(&answer))
}

impl Program for Sqlserver {
    fn call(&self, request: &str) -> String {
        reylo_engine_core::api::call::<reylo_engine::sqlserver::Sqlserver>(request)
    }

    async fn serve(&mut self, request: &Value) -> Option<Value> {
        let op = request["op"].as_str()?;
        let id = request["conn"].as_u64().unwrap_or(0);
        let sql = request["sql"].as_str().unwrap_or_default();
        Some(match op {
            "open" => {
                let url = request["url"].as_str().unwrap_or_default();
                match connect(url).await {
                    Ok((client, version)) => {
                        self.next += 1;
                        self.connections.insert(self.next, Connection { client, url: url.to_string(), answers: AnswerCache::until_cleared() });
                        json!({ "status": "opened", "conn": self.next, "version": version })
                    }
                    Err(failure) => failure,
                }
            }
            "run" => match self.connections.get_mut(&id) {
                None => closed(id),
                Some(c) => match c.analyze(sql).await {
                    Ok(analysis) => json!({ "status": "analysis", "analysis": analysis }),
                    Err(failure) => failure,
                },
            },
            // A query the host runs itself (the schema's fingerprint), its values bound as `@P1`, …
            "query" => match self.connections.get_mut(&id) {
                None => closed(id),
                Some(c) => {
                    let params = request["params"].as_array().cloned().unwrap_or_default();
                    let mut result = answers::rows(&mut c.client, sql, &params).await;
                    if result.is_err() && c.lost().await {
                        if let Err(failure) = c.reopen().await {
                            return Some(failure);
                        }
                        result = answers::rows(&mut c.client, sql, &params).await;
                    }
                    match result {
                        Ok(Ok(rows)) => json!({ "status": "rows", "rows": rows }),
                        Ok(Err(e)) => json!({ "status": "error", "message": e.message, "code": e.code }),
                        Err(e) => json!({ "status": "unreachable", "message": e.0.lines().next().unwrap_or_default() }),
                    }
                }
            },
            // The schema changed: what the engine was told about the old one is stale.
            "forget" => match self.connections.get_mut(&id) {
                None => closed(id),
                Some(c) => {
                    c.answers.clear();
                    json!({ "status": "ok" })
                }
            },
            "close" => {
                if let Some(c) = self.connections.remove(&id) {
                    let _ = c.client.close().await;
                }
                json!({ "status": "ok" })
            }
            _ => return None,
        })
    }
}

fn closed(id: u64) -> Value {
    json!({ "status": "unreachable", "message": format!("connection {id} is closed") })
}

/// The session as the `mssql` driver (tedious) sets it as it logs in, which the types the engine
/// infers assume: ANSI_WARNINGS on, so that an overflow or a division by zero fails rather than
/// yielding NULL, CONCAT_NULL_YIELDS_NULL on, ANSI_NULLS on, so that `= NULL` matches nothing, and
/// QUOTED_IDENTIFIER on, which an indexed view and a filtered index need.
const SESSION: &str = "set ansi_nulls on; set ansi_null_dflt_on on; set ansi_padding on; set ansi_warnings on; set arithabort on; set concat_null_yields_null on; set datefirst 7; set dateformat mdy; set implicit_transactions off; set language us_english; set numeric_roundabort off; set quoted_identifier on; set textsize 2147483647; set transaction isolation level read committed; set xact_abort off";

/// A connection to the server a URL names, in the session `SESSION` sets, and its product version.
async fn connect(url: &str) -> Result<(Conn, String), Value> {
    let config = parse(url).map_err(|message| json!({ "status": "error", "message": message }))?;
    // tiberius takes the process's crypto provider where one is installed: ring's, as every other
    // TLS connection of the program.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let attempt = async {
        let tcp = TcpStream::connect(config.get_addr()).await.map_err(|e| json!({ "status": "unreachable", "message": e.to_string() }))?;
        let _ = tcp.set_nodelay(true);
        Client::connect(config, tcp.compat_write()).await.map_err(|e| failure(&e))
    };
    let mut client = match tokio::time::timeout(CONNECT_TIMEOUT, attempt).await {
        Err(_) => return Err(json!({ "status": "unreachable", "message": format!("no answer within {} seconds", CONNECT_TIMEOUT.as_secs()) })),
        Ok(result) => result?,
    };
    if let Err(e) = async { client.simple_query(SESSION).await?.into_results().await }.await {
        return Err(failure(&e));
    }
    let version = match client.query("select cast(serverproperty('ProductVersion') as nvarchar(128))", &[]).await {
        Ok(stream) => stream.into_row().await.ok().flatten().and_then(|row| row.get::<&str, _>(0).map(String::from)).unwrap_or_default(),
        Err(e) => return Err(failure(&e)),
    };
    Ok((client, version))
}

/// A failure as the program's answer: a refused login, a server gone or never reached, or an error.
fn failure(e: &tiberius::error::Error) -> Value {
    match e {
        tiberius::error::Error::Server(t) if t.code() == LOGIN_FAILED => json!({ "status": "refused", "message": t.message() }),
        tiberius::error::Error::Server(t) => json!({ "status": "error", "message": t.message() }),
        tiberius::error::Error::Io { message, .. } => json!({ "status": "unreachable", "message": message }),
        other => json!({ "status": "error", "message": other.to_string() }),
    }
}

/// The URL as tiberius's configuration.
pub fn parse(url: &str) -> Result<Config, String> {
    let rest = ["sqlserver://", "mssql://"].iter().find_map(|scheme| url.get(..scheme.len()).filter(|s| s.eq_ignore_ascii_case(scheme)).map(|_| &url[scheme.len()..]));
    let Some(rest) = rest else { return Err(format!("a SQL Server URL starts with sqlserver:// or mssql://, and {} does not", redact(url))) };
    let (base, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (authority, database) = base.split_once('/').unwrap_or((base, ""));
    let (credentials, host) = authority.rsplit_once('@').unwrap_or(("", authority));
    let (user, password) = credentials.split_once(':').unwrap_or((credentials, ""));
    let mut config = Config::new();
    // A bracketed host is an IPv6 address, whose colons are its own.
    let (host, port) = match host.strip_prefix('[') {
        Some(rest) => match rest.split_once(']') {
            Some((h, p)) => (h, p.strip_prefix(':')),
            None => (rest, None),
        },
        None => match host.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (host, None),
        },
    };
    let port = port.map(|p| p.parse::<u16>().map_err(|_| format!("the port of {} is not a number", redact(url)))).transpose()?;
    config.host(if host.is_empty() { "localhost" } else { host });
    config.port(port.unwrap_or(1433));
    if !database.is_empty() {
        config.database(crate::decode(database));
    }
    if user.is_empty() {
        return Err(format!("{} names no user: Reylo signs in to SQL Server with a SQL Server login, as sqlserver://user:password@host/database", redact(url)));
    }
    config.authentication(AuthMethod::sql_server(crate::decode(user), crate::decode(password)));
    config.application_name("reylo");
    let mut encrypt = true;
    let mut trust = false;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, "true"));
        let value = crate::decode(value);
        let truth = |v: &str| match v.to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => Ok(true),
            "false" | "no" | "0" => Ok(false),
            other => Err(format!("{key}={other} is not true or false")),
        };
        match key.to_ascii_lowercase().as_str() {
            "encrypt" => encrypt = truth(&value)?,
            "trustservercertificate" => trust = truth(&value)?,
            "database" => config.database(value),
            // The package's other options are its own: timeouts, pool sizes, its app name.
            _ => {}
        }
    }
    config.encryption(if encrypt { EncryptionLevel::Required } else { EncryptionLevel::Off });
    if trust {
        config.trust_cert();
    }
    Ok(config)
}

/// The URL with its user and password left out, for a message.
fn redact(url: &str) -> String {
    match (url.find("://"), url.rfind('@')) {
        (Some(s), Some(at)) if at > s => format!("{}//…{}", &url[..s + 1], &url[at..]),
        _ => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_url() {
        let c = parse("sqlserver://sa:p%40ss@db.example:14330/app?encrypt=false&trustServerCertificate=true").unwrap();
        assert_eq!(c.get_addr(), "db.example:14330");
        assert!(parse("mssql://sa:x@localhost/app").is_ok());
        assert_eq!(parse("sqlserver://localhost/app").unwrap_err(), "sqlserver://localhost/app names no user: Reylo signs in to SQL Server with a SQL Server login, as sqlserver://user:password@host/database");
        assert!(parse("postgres://u@h/db").unwrap_err().starts_with("a SQL Server URL starts with"));
        assert_eq!(redact("sqlserver://sa:secret@h/db"), "sqlserver://…@h/db");
    }
}
