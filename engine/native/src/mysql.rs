//! The MySQL program: connections over mysql_async, opened from the URL a mysql2 project writes,
//! with its TLS settings read here, since mysql_async refuses a URL parameter it does not know. A
//! MariaDB server is refused before anything is analyzed on it, as the type check refuses it
//! (src/connection.ts, `notMysql`), and each statement is analyzed over a connection by the replay
//! loop of `reylo-engine`, which keeps the connection's answers between statements.

use crate::Program;
use mysql_async::prelude::Queryable;
use mysql_async::{Conn, Opts, OptsBuilder, SslOpts};
use reylo_engine::clients::mysql_async::{self as answers, Describe};
use reylo_engine::{AnswerCache, ReplayError};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

/// How long a connection attempt may take, as mysql2's default `connectTimeout`.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The error a server answers a refused login with.
const ACCESS_DENIED: u16 = 1045;

#[derive(Default)]
pub struct Mysql {
    connections: HashMap<u64, Connection>,
    next: u64,
}

struct Connection {
    conn: Conn,
    /// The engine's answers, kept between statements as the loader's session keeps them.
    answers: AnswerCache<Describe>,
}

impl Program for Mysql {
    fn call(&self, request: &str) -> String {
        reylo_engine_core::api::call::<reylo_engine::mysql::Mysql>(request)
    }

    async fn serve(&mut self, request: &Value) -> Option<Value> {
        let op = request["op"].as_str()?;
        let id = request["conn"].as_u64().unwrap_or(0);
        let sql = request["sql"].as_str().unwrap_or_default();
        Some(match op {
            "open" => match connect(request["url"].as_str().unwrap_or_default()).await {
                Ok(conn) => {
                    self.next += 1;
                    self.connections.insert(self.next, Connection { conn, answers: AnswerCache::default() });
                    json!({ "status": "opened", "conn": self.next })
                }
                Err(failure) => failure,
            },
            "run" => match self.connections.get_mut(&id) {
                None => closed(id),
                Some(c) => match answers::analyze(&mut c.conn, sql, &mut c.answers).await {
                    Ok(analysis) => json!({ "status": "analysis", "analysis": analysis }),
                    // The connection went while the engine asked: the database cannot be reached.
                    Err(ReplayError::Host(e)) if e.0.starts_with("the prepare did not reach the server") => json!({ "status": "unreachable", "message": e.0 }),
                    Err(e) => json!({ "status": "error", "message": e.to_string() }),
                },
            },
            // A query the host runs itself (the schema's fingerprint), its rows as mysql2 reads them,
            // and its failure told apart as `open`'s is: a refused login, a lost server, an error.
            "query" => match self.connections.get_mut(&id) {
                None => closed(id),
                Some(c) => {
                    let params = request["params"].as_array().cloned().unwrap_or_default();
                    match answers::interpolate(sql, &params) {
                        None => json!({ "status": "error", "message": format!("cannot write {params:?} into the query") }),
                        Some(text) => match rows(&mut c.conn, text).await {
                            Ok(rows) => json!({ "status": "rows", "rows": rows }),
                            Err(e) => failure(&e),
                        },
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
                    let _ = c.conn.disconnect().await;
                }
                json!({ "status": "ok" })
            }
            _ => return None,
        })
    }
}

/// A query's rows, each value as mysql2 reads it (`reylo_engine::clients::mysql_async::decoded`).
async fn rows(conn: &mut Conn, text: String) -> mysql_async::Result<Vec<Vec<Value>>> {
    let mut result = conn.query_iter(text).await?;
    let types: Vec<u8> = result.columns_ref().iter().map(|c| c.column_type() as u8).collect();
    let mut rows = Vec::new();
    while let Some(row) = result.next().await? {
        rows.push(row.unwrap().into_iter().zip(&types).map(|(value, code)| answers::decoded(value, *code)).collect());
    }
    Ok(rows)
}

fn closed(id: u64) -> Value {
    json!({ "status": "unreachable", "message": format!("connection {id} is closed") })
}

/// A connection to the MySQL server a URL names, refused where the server is MariaDB.
async fn connect(url: &str) -> Result<Conn, Value> {
    let opts = parse(url).map_err(|message| json!({ "status": "error", "message": message }))?;
    let mut conn = match tokio::time::timeout(CONNECT_TIMEOUT, Conn::new(opts)).await {
        Err(_) => return Err(json!({ "status": "unreachable", "message": "no answer within 10 seconds" })),
        Ok(Err(e)) => return Err(failure(&e)),
        Ok(Ok(conn)) => conn,
    };
    let version: Option<String> = conn.query_first("select version()").await.map_err(|e| failure(&e))?;
    if let Some(server) = mariadb(version.as_deref().unwrap_or_default()) {
        let _ = conn.disconnect().await;
        return Err(json!({ "status": "notMysql", "server": server }));
    }
    Ok(conn)
}

/// The server a `version()` names where it is MariaDB, as src/connection.ts's `notMysql` reads it.
fn mariadb(version: &str) -> Option<String> {
    let lower = version.to_ascii_lowercase();
    if !lower.contains("mariadb") {
        return None;
    }
    let digits = version.strip_prefix("5.5.5-").unwrap_or(version);
    let number: String = digits.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let number = number.trim_end_matches('.');
    Some(if number.is_empty() { "MariaDB".to_string() } else { format!("MariaDB {number}") })
}

/// The URL as mysql_async's options. The TLS settings mysql2 projects write are read here: mysql2's
/// `ssl` (a profile name, `true`, or JSON with `rejectUnauthorized`), MySQL's own `ssl-mode`, and
/// `sslaccept`; mysql2's `socketPath` is the socket. Every other parameter is mysql2's alone and is
/// left out.
fn parse(url: &str) -> Result<Opts, String> {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    // None: no TLS. Some((chain, name)): TLS, checking the certificate chain and the host name or not.
    let mut tls: Option<(bool, bool)> = None;
    let mut socket = None;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = crate::decode(value);
        match key {
            "ssl" => {
                tls = match value.trim() {
                    "" | "false" | "0" => None,
                    json if json.starts_with('{') => {
                        let reject = serde_json::from_str::<Value>(json).ok().and_then(|v| v["rejectUnauthorized"].as_bool()).unwrap_or(true);
                        Some((reject, reject))
                    }
                    // `true`, or a profile name such as "Amazon RDS": checked in full.
                    _ => Some((true, true)),
                }
            }
            "ssl-mode" | "sslmode" | "ssl_mode" => {
                tls = match value.to_ascii_uppercase().replace('-', "_").as_str() {
                    "DISABLED" | "DISABLE" => None,
                    "PREFERRED" | "PREFER" | "REQUIRED" | "REQUIRE" => Some((false, false)),
                    "VERIFY_CA" => Some((true, false)),
                    "VERIFY_IDENTITY" | "VERIFY_FULL" => Some((true, true)),
                    other => return Err(format!("ssl-mode={other} is none of DISABLED, PREFERRED, REQUIRED, VERIFY_CA and VERIFY_IDENTITY")),
                }
            }
            "sslaccept" => tls = Some(if value == "accept_invalid_certs" { (false, false) } else { (true, true) }),
            "socketPath" | "socket" => socket = Some(value),
            _ => {}
        }
    }
    let opts = Opts::from_url(base).map_err(|e| format!("cannot read the URL: {e}"))?;
    // The two settings mysql_async otherwise asks the server for on every connection, as the server's
    // defaults: a connection then sends nothing before the engine's own statements.
    let mut builder = OptsBuilder::from_opts(opts).socket(socket).prefer_socket(false).max_allowed_packet(Some(64 << 20)).wait_timeout(Some(28_800));
    if let Some((chain, name)) = tls {
        builder = builder.ssl_opts(SslOpts::default().with_danger_accept_invalid_certs(!chain).with_danger_skip_domain_validation(!name));
    }
    Ok(builder.into())
}

/// What a failed call means to the host: a refused login, a server that cannot be reached (no
/// answer, or a connection the driver lost), or the server's error for what it was asked.
fn failure(e: &mysql_async::Error) -> Value {
    match e {
        mysql_async::Error::Server(s) if s.code == ACCESS_DENIED => json!({ "status": "refused", "message": s.message, "code": "ER_ACCESS_DENIED_ERROR" }),
        mysql_async::Error::Server(s) => json!({ "status": "error", "message": s.message, "code": s.code.to_string() }),
        mysql_async::Error::Url(u) => json!({ "status": "error", "message": u.to_string() }),
        other => json!({ "status": "unreachable", "message": other.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mariadb_is_named_as_the_type_check_names_it() {
        assert_eq!(mariadb("8.4.6").as_deref(), None);
        assert_eq!(mariadb("5.5.5-10.11.6-MariaDB-0+deb12u1").as_deref(), Some("MariaDB 10.11.6"));
        assert_eq!(mariadb("11.4.2-MariaDB").as_deref(), Some("MariaDB 11.4.2"));
    }

    #[test]
    fn the_tls_settings_mysql2_projects_write_are_read() {
        let tls = |url: &str| parse(url).unwrap().ssl_opts().map(|s| (s.accept_invalid_certs(), s.skip_domain_validation()));
        assert_eq!(tls("mysql://root@h:3306/app"), None);
        assert_eq!(tls(r#"mysql://root@h/app?ssl={"rejectUnauthorized":true}"#), Some((false, false)));
        assert_eq!(tls("mysql://root@h/app?ssl=%7B%22rejectUnauthorized%22%3Afalse%7D"), Some((true, true)));
        assert_eq!(tls("mysql://root@h/app?ssl-mode=VERIFY_CA"), Some((false, true)));
        assert_eq!(tls("mysql://root@h/app?sslaccept=strict"), Some((false, false)));
        assert_eq!(tls("mysql://root@h/app?charset=utf8mb4&ssl=Amazon%20RDS"), Some((false, false)));
        assert!(parse("mysql://root@h/app?ssl-mode=sometimes").is_err());
    }
}
