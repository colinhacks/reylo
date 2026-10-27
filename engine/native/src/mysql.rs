//! The MySQL program: connections over mysql_async, opened from the URL a mysql2 project writes,
//! with its TLS settings read here, since mysql_async refuses a URL parameter it does not know. A
//! MariaDB server is refused before anything is analyzed on it, as the type check refuses it
//! (src/connection.ts, `notMysql`), and each statement is analyzed over a connection by the replay
//! loop of `reylo-engine`, which keeps the connection's answers between statements. A connection
//! the server dropped while it sat idle (a restart, `wait_timeout`) is opened again for the request
//! that finds it dropped.

use crate::Program;
use mysql_async::prelude::Queryable;
use mysql_async::{ClientIdentity, Conn, Opts, OptsBuilder, SslOpts};
use reylo_engine::clients::mysql_async::{self as answers, Describe};
use reylo_engine::{AnswerCache, ReplayError};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

/// How long a connection attempt may take, as mysql2's default `connectTimeout`.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The error a server answers a refused login with.
const ACCESS_DENIED: u16 = 1045;

/// The connection attribute that carries the token of the host's bridge to a Unix socket.
const BRIDGE_ATTRIBUTE: &str = "reylo_bridge";

#[derive(Default)]
pub struct Mysql {
    connections: HashMap<u64, Connection>,
    next: u64,
}

struct Connection {
    conn: Conn,
    /// The URL it was opened from, and the token of the host's bridge to a Unix socket, to open it again.
    url: String,
    bridge: Option<String>,
    /// The engine's answers, kept between statements as the loader's session keeps them.
    answers: AnswerCache<Describe>,
}

impl Connection {
    /// Whether the connection is gone, after a call failed with something other than the server's
    /// answer: mysql_async tells a lost connection apart from no other failure, so it is asked.
    async fn lost(&mut self) -> bool {
        self.conn.ping().await.is_err()
    }

    /// The connection opened again where the server dropped it, as the old one is.
    async fn reopen(&mut self) -> Result<(), Value> {
        self.conn = connect(&self.url, self.bridge.as_deref()).await?;
        Ok(())
    }
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
            "open" => {
                let url = request["url"].as_str().unwrap_or_default();
                let bridge = request["bridge"].as_str().map(String::from);
                match connect(url, bridge.as_deref()).await {
                    Ok(conn) => {
                        self.next += 1;
                        self.connections.insert(self.next, Connection { conn, url: url.to_string(), bridge, answers: AnswerCache::default() });
                        json!({ "status": "opened", "conn": self.next })
                    }
                    Err(failure) => failure,
                }
            }
            "run" => match self.connections.get_mut(&id) {
                None => closed(id),
                Some(c) => {
                    let mut result = answers::analyze(&mut c.conn, sql, &mut c.answers).await;
                    if matches!(result, Err(ReplayError::Host(_))) && c.lost().await {
                        if let Err(failure) = c.reopen().await {
                            return Some(failure);
                        }
                        result = answers::analyze(&mut c.conn, sql, &mut c.answers).await;
                    }
                    match result {
                        Ok(analysis) => json!({ "status": "analysis", "analysis": analysis }),
                        // The connection went while the engine asked: the database cannot be reached.
                        Err(ReplayError::Host(e)) if e.0.starts_with("the prepare did not reach the server") || c.lost().await => {
                            json!({ "status": "unreachable", "message": e.0.lines().next().unwrap_or_default() })
                        }
                        Err(e) => json!({ "status": "error", "message": e.to_string() }),
                    }
                }
            },
            // A query the host runs itself (the schema's fingerprint), its rows as mysql2 reads them,
            // and its failure told apart as `open`'s is: a refused login, a lost server, an error.
            "query" => match self.connections.get_mut(&id) {
                None => closed(id),
                Some(c) => {
                    let params = request["params"].as_array().cloned().unwrap_or_default();
                    match answers::interpolate(sql, &params) {
                        None => json!({ "status": "error", "message": format!("cannot write {params:?} into the query") }),
                        Some(text) => {
                            let mut result = rows(&mut c.conn, text.clone()).await;
                            if matches!(result, Err(ref e) if !matches!(e, mysql_async::Error::Server(_))) && c.lost().await {
                                if let Err(failure) = c.reopen().await {
                                    return Some(failure);
                                }
                                result = rows(&mut c.conn, text).await;
                            }
                            match result {
                                Ok(rows) => json!({ "status": "rows", "rows": rows }),
                                Err(e) => failure(&e),
                            }
                        }
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

/// A connection to the MySQL server a URL names, refused where the server is MariaDB. Where the URL
/// is the host's bridge to a Unix socket (src/host-url.ts), the login carries the bridge's `token`
/// as a connection attribute, which is how the bridge knows it from any other local process's.
async fn connect(url: &str, bridge: Option<&str>) -> Result<Conn, Value> {
    let mut opts = parse(url).map_err(|message| json!({ "status": "error", "message": message }))?;
    if let Some(token) = bridge {
        opts = OptsBuilder::from_opts(opts).connect_attribute(BRIDGE_ATTRIBUTE, token).into();
    }
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
/// `ssl` (`true`, or JSON with `rejectUnauthorized`, `ca`, `cert` and `key`, each certificate and
/// key as PEM text), MySQL's own `ssl-mode`, and `sslaccept`; mysql2's `socketPath` is the socket.
/// Every other parameter is mysql2's alone and is left out. The certificates are checked against
/// the roots mysql_async carries (Mozilla's), those `NODE_EXTRA_CA_CERTS` names, and the `ca` the
/// URL gives.
fn parse(url: &str) -> Result<Opts, String> {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    // None: no TLS. Some((chain, name)): TLS, checking the certificate chain and the host name or not.
    let mut tls: Option<(bool, bool)> = None;
    let mut socket = None;
    // mysql_async's certificate type is private to it: each is made by its `From`, as `with_root_certs` takes it.
    let (mut roots, mut identity) = (Vec::new(), None::<ClientIdentity>);
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = crate::decode(value);
        match key {
            "ssl" => {
                tls = match value.trim() {
                    "" | "false" | "0" => None,
                    json if json.starts_with('{') => {
                        let ssl: Value = serde_json::from_str(json).map_err(|e| format!("ssl={json} is not JSON: {e}"))?;
                        let reject = ssl["rejectUnauthorized"].as_bool().unwrap_or(true);
                        let pem = |key: &str| match &ssl[key] {
                            Value::String(text) => vec![text.clone()],
                            Value::Array(items) => items.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
                            _ => vec![],
                        };
                        roots.extend(pem("ca").into_iter().map(|text| text.into_bytes().into()));
                        identity = match (pem("cert").concat(), pem("key").concat()) {
                            (cert, key) if cert.is_empty() && key.is_empty() => None,
                            (cert, key) if !cert.is_empty() && !key.is_empty() => Some(ClientIdentity::new(cert.into_bytes().into(), key.into_bytes().into())),
                            _ => return Err("ssl's cert and key name a client certificate together: give both".to_string()),
                        };
                        Some((reject, reject))
                    }
                    "true" | "1" => Some((true, true)),
                    // mysql2's profiles carry their own certificates, which this engine does not.
                    profile => return Err(format!("ssl={profile} names one of mysql2's certificate profiles, which this engine does not carry: write ssl={{\"rejectUnauthorized\":true}} and name the profile's certificates in NODE_EXTRA_CA_CERTS")),
                }
            }
            "ssl-mode" | "sslmode" | "ssl_mode" => {
                tls = match value.to_ascii_uppercase().replace('-', "_").as_str() {
                    // A server without TLS answers PREFERRED in plain text, which mysql_async cannot
                    // fall back to once TLS is asked for: as mysql2, which reads no `ssl-mode`, it
                    // connects without.
                    "DISABLED" | "DISABLE" | "PREFERRED" | "PREFER" => None,
                    "REQUIRED" | "REQUIRE" => Some((false, false)),
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
    // WASI has sockets for TCP and UDP alone: a Unix socket is opened by the host, which bridges it
    // over TCP (src/host-url.ts), so one that reaches the component is refused by name.
    #[cfg(target_family = "wasm")]
    if let Some(path) = &socket {
        return Err(format!("socketPath={path} is a Unix socket, which this engine cannot open: connect over TCP"));
    }
    let opts = Opts::from_url(base).map_err(|e| format!("cannot read the URL: {e}"))?;
    // The two settings mysql_async otherwise asks the server for on every connection, as the server's
    // defaults: a connection then sends nothing before the engine's own statements.
    let mut builder = OptsBuilder::from_opts(opts).socket(socket).prefer_socket(false).max_allowed_packet(Some(64 << 20)).wait_timeout(Some(28_800));
    if let Some((chain, name)) = tls {
        if let Some(file) = std::env::var("NODE_EXTRA_CA_CERTS").ok().filter(|f| !f.is_empty()) {
            roots.push(std::path::PathBuf::from(file).into());
        }
        builder = builder.ssl_opts(SslOpts::default().with_danger_accept_invalid_certs(!chain).with_danger_skip_domain_validation(!name).with_root_certs(roots).with_client_identity(identity));
    }
    Ok(builder.into())
}

/// What a failed call means to the host: a refused login, a server that cannot be reached (no
/// answer, or a connection the driver lost), a TLS handshake that failed (a setting to fix), or the
/// server's error for what it was asked.
fn failure(e: &mysql_async::Error) -> Value {
    match e {
        e if matches!(e, mysql_async::Error::Io(mysql_async::IoError::Tls(_))) || crate::tls::failed(e) => json!({ "status": "error", "message": e.to_string() }),
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
        assert_eq!(tls("mysql://root@h/app?ssl-mode=REQUIRED"), Some((true, true)));
        assert_eq!(tls("mysql://root@h/app?ssl-mode=PREFERRED"), None);
        assert_eq!(tls("mysql://root@h/app?sslaccept=strict"), Some((false, false)));
        assert_eq!(tls("mysql://root@h/app?ssl=true"), Some((false, false)));
        assert!(parse("mysql://root@h/app?ssl-mode=sometimes").is_err());
        assert!(parse("mysql://root@h/app?charset=utf8mb4&ssl=Amazon%20RDS").unwrap_err().contains("NODE_EXTRA_CA_CERTS"));
    }

    #[test]
    fn the_certificates_ssl_gives_are_presented_and_trusted() {
        let pem = "-----BEGIN CERTIFICATE-----\\nMIIB\\n-----END CERTIFICATE-----";
        let opts = parse(&format!(r#"mysql://root@h/app?ssl={{"ca":"{pem}","cert":"{pem}","key":"k"}}"#)).unwrap();
        let ssl = opts.ssl_opts().unwrap();
        assert_eq!(ssl.root_certs().len(), 1);
        assert!(ssl.client_identity().is_some());
        assert!(parse(r#"mysql://root@h/app?ssl={"cert":"c"}"#).is_err());
    }
}
