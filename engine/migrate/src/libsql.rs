//! A libSQL database over HTTP: Turso Cloud's, or any libSQL server's (`sqld`, `turso dev`), spoken
//! in Hrana 3, the protocol `@libsql/client` speaks over HTTP. A URL names it as that client's does:
//! `libsql://<host>` (HTTPS, or HTTP with `?tls=0`), or `https://`, `http://`, `wss://` and `ws://`,
//! with the auth token in `?authToken=`. The connection is one stream on the server, held from
//! request to request by the baton each response hands back, so a transaction and a pragma last as
//! they do on a connection to a file. A server ends a stream left idle for a few seconds: one that
//! ended outside a transaction is opened again, as a new connection; one inside a transaction ended
//! it, which the server rolled back, and is an error.

use crate::db::Row;
use crate::Error;
use serde_json::{json, Value};

/// The schemes a libSQL server's URL takes.
const SCHEMES: [&str; 5] = ["libsql", "https", "http", "wss", "ws"];

/// Whether `url` names a libSQL server rather than a SQLite file.
pub fn is_url(url: &str) -> bool {
    url.split_once("://").is_some_and(|(scheme, _)| SCHEMES.iter().any(|s| s.eq_ignore_ascii_case(scheme)))
}

/// What a libSQL URL says: the server's HTTP base, and the auth token.
#[derive(Debug, PartialEq, Eq)]
pub struct Target {
    pub base: String,
    pub token: Option<String>,
}

/// The server and the token a libSQL URL names, read as `@libsql/client` reads it.
pub fn target(url: &str) -> Result<Target, Error> {
    let (scheme, rest) = url.split_once("://").ok_or_else(|| Error(format!("not a libSQL URL: {}", identity(url))))?;
    let scheme = scheme.to_ascii_lowercase();
    let (rest, query) = match rest.split_once('?') {
        Some((rest, query)) => (rest, query),
        None => (rest, ""),
    };
    let rest = rest.split('#').next().unwrap_or_default().trim_end_matches('/');
    let mut token = None;
    let mut tls = None;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "authToken" => token = Some(crate::percent_decode(value)).filter(|t| !t.is_empty()),
            "tls" => match value {
                "0" => tls = Some(false),
                "1" => tls = Some(true),
                _ => return Err(Error(format!("tls in a libSQL URL is 0 or 1, not {value}"))),
            },
            _ => return Err(Error(format!("a libSQL URL takes authToken and tls, not {key}: {}", identity(url)))),
        }
    }
    let secure = match scheme.as_str() {
        "libsql" => tls.unwrap_or(true),
        "https" | "wss" => true,
        _ => false,
    };
    if rest.is_empty() || rest.contains('@') {
        return Err(Error(format!("a libSQL URL names its server as libsql://<host>, its token in ?authToken=: {}", identity(url))));
    }
    Ok(Target { base: format!("{}://{rest}", if secure { "https" } else { "http" }), token })
}

/// The URL as it names the database, for a message or a key: no token and no settings.
pub fn identity(url: &str) -> String {
    let bare = &url[..url.find(['?', '#']).unwrap_or(url.len())];
    bare.trim_end_matches('/').to_string()
}

/// One connection to a libSQL server: a stream it keeps from request to request.
#[cfg(feature = "cli")]
pub struct Remote {
    agent: ureq::Agent,
    target: Target,
    /// Where requests go once the server has said (`base_url`), else the URL's server.
    base_url: Option<String>,
    baton: Option<String>,
    /// Whether the stream was outside a transaction after the last request, as the server said.
    autocommit: bool,
}

#[cfg(feature = "cli")]
impl Remote {
    /// A connection to the server `url` names, checked by a first request, which also says whether
    /// the token is taken.
    pub fn open(url: &str) -> Result<Remote, Error> {
        let target = target(url)?;
        let agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_connect(Some(std::time::Duration::from_secs(30))).build().new_agent();
        let mut remote = Remote { agent, target, base_url: None, baton: None, autocommit: true };
        remote.one(json!({ "type": "execute", "stmt": { "sql": "select 1", "want_rows": true } }))?;
        Ok(remote)
    }

    /// The rows of one statement, every value as text: an integer and a real in decimal, a blob's
    /// bytes read as UTF-8.
    pub fn rows(&mut self, sql: &str) -> Result<Vec<Row>, Error> {
        let result = self.one(json!({ "type": "execute", "stmt": { "sql": sql, "want_rows": true } }))?;
        let columns: Vec<String> = result["result"]["cols"].as_array().map(|cols| cols.iter().map(|c| c["name"].as_str().unwrap_or_default().to_string()).collect()).unwrap_or_default();
        let rows = result["result"]["rows"].as_array().cloned().unwrap_or_default();
        rows.iter().map(|row| Ok(Row::new(columns.clone(), row.as_array().map(|values| values.iter().map(text).collect::<Result<_, _>>()).transpose()?.unwrap_or_default()))).collect()
    }

    /// Runs `sql`, one statement or several, in order, stopping at the first that fails.
    pub fn exec(&mut self, sql: &str) -> Result<(), Error> {
        self.one(json!({ "type": "sequence", "sql": sql })).map(|_| ())
    }

    /// Runs one statement with text parameters, `?` each.
    pub fn exec_with(&mut self, sql: &str, params: &[&str]) -> Result<(), Error> {
        let args: Vec<Value> = params.iter().map(|p| json!({ "type": "text", "value": p })).collect();
        self.one(json!({ "type": "execute", "stmt": { "sql": sql, "args": args, "want_rows": false } })).map(|_| ())
    }

    /// Ends the stream, which the server would otherwise keep until it expires.
    pub fn close(mut self) {
        if self.baton.is_some() {
            let _ = self.pipeline(vec![json!({ "type": "close" })]);
        }
    }

    /// One request's response, on the stream, with the stream's state read after it.
    fn one(&mut self, request: Value) -> Result<Value, Error> {
        let mut results = self.pipeline(vec![request, json!({ "type": "get_autocommit" })])?;
        if let Some(Ok(state)) = results.get(1) {
            self.autocommit = state["is_autocommit"].as_bool().unwrap_or(true);
        }
        results.swap_remove(0)
    }

    /// The requests' results, in order, sent as one pipeline on the stream. A stream the server
    /// ended outside a transaction is opened again, and the requests sent on it, since none of them
    /// ran.
    fn pipeline(&mut self, requests: Vec<Value>) -> Result<Vec<Result<Value, Error>>, Error> {
        let mut retried = false;
        loop {
            let body = json!({ "baton": self.baton, "requests": requests }).to_string();
            let url = format!("{}/v3/pipeline", self.base_url.as_deref().unwrap_or(&self.target.base));
            let mut request = self.agent.post(&url).content_type("application/json");
            if let Some(token) = &self.target.token {
                request = request.header("Authorization", format!("Bearer {token}"));
            }
            let mut response = request.send(&body).map_err(|e| Error(format!("cannot reach the libSQL server at {}: {e}", self.target.base)))?;
            let status = response.status().as_u16();
            let text = response.body_mut().with_config().limit(1 << 30).read_to_string().map_err(|e| Error(format!("the libSQL server at {} answered unreadably: {e}", self.target.base)))?;
            let answer: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            if status != 200 {
                let message = answer["message"].as_str().map(String::from).unwrap_or_else(|| text.trim().to_string());
                let expired = answer["code"] == "STREAM_EXPIRED" || message == "Received an invalid baton";
                if expired && self.baton.is_some() && !retried {
                    if self.autocommit {
                        self.baton = None;
                        self.base_url = None;
                        retried = true;
                        continue;
                    }
                    return Err(Error("the libSQL server ended this connection inside a transaction, which it rolled back: it was idle too long".into()));
                }
                return Err(Error(match status {
                    401 | 403 => format!("the libSQL server at {} refused the auth token: {message}. Put the database's token in the URL, as ?authToken=…", self.target.base),
                    404 => format!("no libSQL server answers at {} (its Hrana 3 pipeline answered 404)", self.target.base),
                    _ => format!("the libSQL server at {} answered {status}: {message}", self.target.base),
                }));
            }
            self.baton = answer["baton"].as_str().map(String::from);
            if let Some(base) = answer["base_url"].as_str() {
                self.base_url = Some(base.trim_end_matches('/').to_string());
            }
            let results = answer["results"].as_array().cloned().unwrap_or_default();
            return Ok(results
                .into_iter()
                .map(|r| match r["type"].as_str() {
                    Some("ok") => Ok(r["response"].clone()),
                    _ => Err(Error(sqlite_message(r["error"]["message"].as_str().unwrap_or("the libSQL server refused the request")))),
                })
                .collect());
        }
    }
}

/// The server's message as SQLite's own: libSQL's server puts `SQLite error: ` before it.
fn sqlite_message(message: &str) -> String {
    message.strip_prefix("SQLite error: ").unwrap_or(message).to_string()
}

/// A Hrana value as text.
fn text(value: &Value) -> Result<Option<String>, Error> {
    Ok(match value["type"].as_str() {
        Some("null") | None => None,
        Some("integer") => value["value"].as_str().map(String::from),
        Some("float") => value["value"].as_f64().map(|f| f.to_string()),
        Some("text") => value["value"].as_str().map(String::from),
        Some("blob") => Some(String::from_utf8_lossy(&base64(value["base64"].as_str().unwrap_or_default())?).into_owned()),
        Some(other) => return Err(Error(format!("the libSQL server sent a value of a type it has none of: {other}"))),
    })
}

/// Standard base64, its padding optional, as Hrana writes a blob.
fn base64(text: &str) -> Result<Vec<u8>, Error> {
    let digit = |c: u8| -> Result<u32, Error> {
        Ok(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(Error("the libSQL server sent a blob that isn't base64".into())),
        } as u32)
    };
    let bytes = text.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= digit(c)? << (18 - 6 * i);
        }
        out.extend(n.to_be_bytes()[1..chunk.len()].iter());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_url_as_libsql_client_does() {
        assert_eq!(target("libsql://app-acme.turso.io?authToken=abc").unwrap(), Target { base: "https://app-acme.turso.io".into(), token: Some("abc".into()) });
        assert_eq!(target("libsql://127.0.0.1:8080?tls=0").unwrap(), Target { base: "http://127.0.0.1:8080".into(), token: None });
        assert_eq!(target("http://127.0.0.1:8080/").unwrap().base, "http://127.0.0.1:8080");
        assert_eq!(target("wss://app-acme.turso.io").unwrap().base, "https://app-acme.turso.io");
        assert_eq!(target("libsql://h?authToken=a%2Bb").unwrap().token.as_deref(), Some("a+b"));
        assert!(target("libsql://h?auth_token=abc").unwrap_err().0.contains("not auth_token"));
        assert!(target("libsql://token@h").is_err());
        assert_eq!(identity("libsql://app-acme.turso.io/?authToken=secret"), "libsql://app-acme.turso.io");
        assert!(is_url("LIBSQL://h") && is_url("https://h") && !is_url("file:./dev.db") && !is_url("postgres://h"));
    }

    #[test]
    fn decodes_a_blob() {
        assert_eq!(base64("aGk").unwrap(), b"hi");
        assert_eq!(base64("aGk=").unwrap(), b"hi");
        assert_eq!(base64("aGVsbG8gd29ybGQ").unwrap(), b"hello world");
        assert_eq!(base64("").unwrap(), b"");
        assert!(base64("a$").is_err());
    }
}
