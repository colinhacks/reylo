//! A libSQL database over HTTP: Turso Cloud's, or any libSQL server's (`sqld`, `turso dev`), spoken
//! in Hrana 3, the protocol `@libsql/client` speaks over HTTP. A URL names it as that client's does:
//! `libsql://<host>` (HTTPS, or HTTP with `?tls=0`), or `https://`, `http://`, `wss://` and `ws://`,
//! with the auth token in `?authToken=`. The connection is one stream on the server, held from
//! request to request by the baton each response hands back, so a transaction and a pragma last as
//! they do on a connection to a file. A server ends a stream left idle: `sqld` after a few seconds,
//! Turso Cloud a transaction's after about twenty. One that ended outside a transaction is opened
//! again, with every pragma the connection set run on it again, so that a migration that turned
//! foreign keys off finds them off; one inside a transaction ended it, which the server rolled back,
//! and is an error.

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

/// Whether `sql` uses SQL of libSQL's own, which Reylo's SQLite refuses: a call to one of its
/// vector functions (`libsql_vector_idx`, `vector`, `vector_distance_cos`, …), a vector type
/// (`F32_BLOB(3)`), or `alter table … alter column`. A project whose migrations or schema.sql do
/// runs its development file and shadows on libSQL's engine ([`crate::libsql_local`]). A word
/// alone is no call, and nor is a name a statement gives a table (`create table vector (…)`,
/// `insert into vector (…)`, `references vector (id)`): only those stay SQLite's.
pub fn uses_own_sql(sql: &str) -> bool {
    use crate::sql::TokenKind::{Symbol, Word};
    const CALLS: [&str; 14] = ["libsql_vector_idx", "vector", "vector32", "vector64", "vector16", "vectorb16", "vector8", "vector1bit", "vector_extract", "vector_distance_cos", "vector_distance_l2", "vector_top_k", "vector_concat", "vector_slice"];
    const TYPES: [&str; 12] = ["f32_blob", "f64_blob", "f16_blob", "fb16_blob", "f8_blob", "f1bit_blob", "float32", "float64", "float16", "floatb16", "float8", "float1bit"];
    let tokens = crate::sql::tokenize(sql, crate::Dialect::Sqlite);
    let word = |i: usize| tokens.get(i).filter(|t| t.kind == Word).map(|t| t.text.to_ascii_lowercase());
    let open = |i: usize| tokens.get(i).is_some_and(|t| t.kind == Symbol && t.text == "(");
    // The words after which a name followed by `(` is a table's or a query's, not a call's.
    const NAMES: [&str; 11] = ["table", "into", "references", "exists", "view", "trigger", "index", "from", "join", "update", "with"];
    // `on` names a table in `create index … on` and `create trigger … on`, and starts a condition
    // in a join: the nearest of these words before it says which.
    let on_names = |i: usize| {
        (0..i).rev().filter_map(word).find(|w| matches!(w.as_str(), "index" | "trigger" | "join" | "select" | "where" | "from")).is_some_and(|w| w == "index" || w == "trigger")
    };
    (0..tokens.len()).any(|i| {
        let Some(w) = word(i) else { return false };
        let named = i > 0 && word(i - 1).is_some_and(|p| NAMES.contains(&p.as_str()) || (p == "on" && on_names(i - 1)));
        if open(i + 1) && !named && (CALLS.contains(&w.as_str()) || TYPES.contains(&w.as_str())) {
            return true;
        }
        // `alter table <name> alter column`, the name qualified or not.
        w == "alter" && word(i + 1).as_deref() == Some("table") && (i + 3..i + 6).any(|j| word(j).as_deref() == Some("alter") && word(j + 1).as_deref() == Some("column"))
    })
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
    /// The pragmas the connection set (`pragma foreign_keys = off`), the last of each name, in the
    /// order they were set: a stream opened again sets them again before anything else.
    pragmas: Vec<(String, String)>,
}

#[cfg(feature = "cli")]
impl Remote {
    /// A connection to the server `url` names, checked by a first request, which also says whether
    /// the token is taken.
    pub fn open(url: &str) -> Result<Remote, Error> {
        let target = target(url)?;
        let agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_connect(Some(std::time::Duration::from_secs(30))).build().new_agent();
        let mut remote = Remote { agent, target, base_url: None, baton: None, autocommit: true, pragmas: vec![] };
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
        self.one(json!({ "type": "sequence", "sql": sql }))?;
        self.remember_pragmas(sql);
        Ok(())
    }

    /// Keeps each pragma `sql` set, for a stream opened again: its statements as SQLite reads them,
    /// so that text in a string or a comment is never taken for one.
    fn remember_pragmas(&mut self, sql: &str) {
        for (name, statement) in pragmas_set(sql) {
            self.pragmas.retain(|(n, _)| *n != name);
            self.pragmas.push((name, statement));
        }
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
    /// ran. A request that opens a stream runs the pragmas the connection set first, whether the
    /// server said the last one expired or handed back no baton for it: one of those that fails
    /// fails the requests, which would otherwise run on a connection set otherwise.
    fn pipeline(&mut self, requests: Vec<Value>) -> Result<Vec<Result<Value, Error>>, Error> {
        let mut retried = false;
        loop {
            let restore: Vec<Value> = if self.baton.is_none() { self.pragmas.iter().map(|(_, sql)| json!({ "type": "sequence", "sql": sql })).collect() } else { vec![] };
            let sent: Vec<&Value> = restore.iter().chain(requests.iter()).collect();
            let body = json!({ "baton": self.baton, "requests": sent }).to_string();
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
                // sqld puts its message in `message`, Turso Cloud in `error`.
                let message = answer["message"].as_str().or_else(|| answer["error"].as_str()).map(String::from).unwrap_or_else(|| text.trim().to_string());
                // sqld says a stream it ended expired; Turso Cloud answers 404, that it has no such stream.
                let expired = answer["code"] == "STREAM_EXPIRED" || message == "Received an invalid baton" || (status == 404 && message.starts_with("stream not found"));
                if expired && self.baton.is_some() && !retried {
                    if self.autocommit {
                        self.baton = None;
                        self.base_url = None;
                        retried = true;
                        continue;
                    }
                    return Err(Error("the libSQL server ended this connection inside a transaction, which it rolled back: it was idle too long".into()));
                }
                // Turso Cloud refuses a token it can't read with a 400, which names the JWT.
                let refused = matches!(status, 401 | 403) || message.starts_with("JWT error");
                return Err(Error(match status {
                    _ if refused => format!("the libSQL server at {} refused the auth token: {message}. Put the database's token in the URL, as ?authToken=…", self.target.base),
                    404 => format!("no libSQL server answers at {} (its Hrana 3 pipeline answered 404)", self.target.base),
                    _ => format!("the libSQL server at {} answered {status}: {message}", self.target.base),
                }));
            }
            self.baton = answer["baton"].as_str().map(String::from);
            if let Some(base) = answer["base_url"].as_str() {
                self.base_url = Some(base.trim_end_matches('/').to_string());
            }
            let mut results = answer["results"].as_array().cloned().unwrap_or_default();
            for (r, (_, sql)) in results.drain(..restore.len().min(results.len())).zip(&self.pragmas) {
                if r["type"] != "ok" {
                    return Err(Error(format!("the libSQL server ended this connection, and the new one refused `{sql}`: {}", sqlite_message(r["error"]["message"].as_str().unwrap_or_default()))));
                }
            }
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

/// Each statement of `sql` that sets a pragma, with the pragma's name.
fn pragmas_set(sql: &str) -> Vec<(String, String)> {
    static PRAGMA: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"(?is)^pragma\s+([\w.]+)\s*=").expect("a valid pattern"));
    crate::sql::split_statements(sql, crate::Dialect::Sqlite)
        .into_iter()
        .filter_map(|s| {
            let text = s.text.trim();
            PRAGMA.captures(text).map(|c| (c[1].to_ascii_lowercase(), text.to_string()))
        })
        .collect()
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
    fn remembers_only_statements_that_set_a_pragma() {
        let names = |sql: &str| pragmas_set(sql).into_iter().map(|(n, _)| n).collect::<Vec<_>>();
        assert_eq!(names("pragma foreign_keys = off"), ["foreign_keys"]);
        assert_eq!(names("PRAGMA main.journal_mode=wal; select 1; pragma foreign_keys = on;"), ["main.journal_mode", "foreign_keys"]);
        assert!(names("insert into log values ('note; pragma user_version = 42; end')").is_empty());
        assert!(names("select 1; -- pragma user_version = 42;\n/* pragma user_version = 7; */ select 2").is_empty());
        assert!(names("pragma foreign_key_check").is_empty());
    }

    /// A stand-in libSQL server: answers each request in turn with the next of `answers`, a status
    /// and a body, and keeps each request's body.
    #[cfg(feature = "cli")]
    fn stand_in(answers: Vec<(u16, Value)>) -> (String, std::sync::Arc<std::sync::Mutex<Vec<Value>>>) {
        use std::io::{BufRead, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("libsql://127.0.0.1:{}?tls=0&authToken=secret-token", listener.local_addr().unwrap().port());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let kept = seen.clone();
        std::thread::spawn(move || {
            for (status, body) in answers {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = std::io::BufReader::new(stream);
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some((_, value)) = line.split_once(':').filter(|(name, _)| name.eq_ignore_ascii_case("content-length")) {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut request = vec![0; length];
                reader.read_exact(&mut request).unwrap();
                kept.lock().unwrap().push(serde_json::from_slice(&request).unwrap());
                let body = body.to_string();
                write!(reader.get_mut(), "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        (url, seen)
    }

    /// A response on stream `baton`, with `results` and then the stream's state.
    #[cfg(feature = "cli")]
    fn answer(baton: Option<&str>, autocommit: bool, mut results: Vec<Value>) -> (u16, Value) {
        results.push(json!({ "type": "ok", "response": { "type": "get_autocommit", "is_autocommit": autocommit } }));
        (200, json!({ "baton": baton, "base_url": null, "results": results }))
    }

    #[cfg(feature = "cli")]
    fn one_row() -> Value {
        json!({ "type": "ok", "response": { "type": "execute", "result": { "cols": [{ "name": "x" }], "rows": [[{ "type": "integer", "value": "1" }]] } } })
    }

    #[cfg(feature = "cli")]
    fn ran() -> Value {
        json!({ "type": "ok", "response": { "type": "sequence" } })
    }

    /// Turso Cloud says a stream it ended is not found, with a 404: outside a transaction, the
    /// connection opens a new one, with its pragmas set first.
    #[cfg(feature = "cli")]
    #[test]
    fn a_stream_turso_cloud_no_longer_has_opens_again_with_its_pragmas() {
        let (url, seen) = stand_in(vec![
            answer(Some("b1"), true, vec![one_row()]),
            answer(Some("b2"), true, vec![ran()]),
            (404, json!({ "error": "stream not found: d41fd8ff:7aaaaa" })),
            answer(Some("b3"), true, vec![ran(), one_row()]),
        ]);
        let mut remote = Remote::open(&url).unwrap();
        remote.exec("pragma foreign_keys = off").unwrap();
        let rows = remote.rows("select 1 as x").unwrap();
        assert_eq!(rows.len(), 1);
        let seen = seen.lock().unwrap();
        assert_eq!(seen[2]["baton"], "b2");
        assert_eq!(seen[3]["baton"], Value::Null);
        assert_eq!(seen[3]["requests"][0]["sql"], "pragma foreign_keys = off");
        assert_eq!(seen[3]["requests"][1]["stmt"]["sql"], "select 1 as x");
    }

    /// A response that hands back no baton ends the stream: the next request opens one, with the
    /// connection's pragmas set first.
    #[cfg(feature = "cli")]
    #[test]
    fn a_stream_ended_with_no_baton_opens_again_with_its_pragmas() {
        let (url, seen) = stand_in(vec![answer(Some("b1"), true, vec![one_row()]), answer(None, true, vec![ran()]), answer(Some("b2"), true, vec![ran(), one_row()])]);
        let mut remote = Remote::open(&url).unwrap();
        remote.exec("pragma foreign_keys = off").unwrap();
        remote.rows("select 1 as x").unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(seen[2]["baton"], Value::Null);
        assert_eq!(seen[2]["requests"][0]["sql"], "pragma foreign_keys = off");
    }

    /// Inside a transaction, a stream Turso Cloud no longer has is an error that says the
    /// transaction was rolled back, not that no server answers.
    #[cfg(feature = "cli")]
    #[test]
    fn a_stream_turso_cloud_no_longer_has_inside_a_transaction_is_said_to_be_rolled_back() {
        let (url, _) = stand_in(vec![answer(Some("b1"), true, vec![one_row()]), answer(Some("b2"), false, vec![ran()]), (404, json!({ "error": "stream not found: d41fd8ff:7aaaaa" }))]);
        let mut remote = Remote::open(&url).unwrap();
        remote.exec("begin").unwrap();
        let error = remote.rows("select 1 as x").unwrap_err().0;
        assert!(error.contains("inside a transaction, which it rolled back"), "{error}");
    }

    /// Turso Cloud refuses a token it can't read with a 400 naming the JWT: the message says the
    /// token was refused, and shows neither the token nor the server's JSON.
    #[cfg(feature = "cli")]
    #[test]
    fn a_token_turso_cloud_cannot_read_is_said_to_be_refused() {
        let (url, _) = stand_in(vec![(400, json!({ "error": "JWT error: InvalidToken" }))]);
        let error = Remote::open(&url).err().unwrap().0;
        assert!(error.contains("refused the auth token: JWT error: InvalidToken."), "{error}");
        assert!(!error.contains("secret-token") && !error.contains('{'), "{error}");
    }

    #[test]
    fn tells_libsqls_own_sql_from_sqlites() {
        assert!(uses_own_sql("create index docs_emb on docs (libsql_vector_idx(emb));"));
        assert!(uses_own_sql("create table docs (id integer primary key, emb F32_BLOB(3));"));
        assert!(uses_own_sql("ALTER TABLE main.docs ALTER COLUMN emb TO emb F32_BLOB(3) NOT NULL;"));
        assert!(uses_own_sql("select id from vector_top_k('docs_emb', vector('[1,2,3]'), 5);"));
        assert!(!uses_own_sql("create table t (vector text, f32_blob blob); alter table t add column note text;"));
        assert!(!uses_own_sql("insert into t values ('libsql_vector_idx(x)'); -- vector(1)"));
        assert!(!uses_own_sql("create table vector (x real check (floor(x) = x)); insert into vector (x) values (1); create index i on vector (x);"));
        assert!(!uses_own_sql("create table t (v integer references vector (id)); create table if not exists vector (id integer); with vector (n) as (select 1) select n from vector;"));
        assert!(uses_own_sql("create table vector (emb F32_BLOB(3)); insert into vector values (vector('[1]'));"));
        assert!(uses_own_sql("insert into near select a.id from a join b on vector_distance_cos(a.x, b.x) < 0.5;"));
        assert!(!uses_own_sql("create trigger t after insert on vector (x) begin select 1; end; create unique index if not exists i on vector (x);"));
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
