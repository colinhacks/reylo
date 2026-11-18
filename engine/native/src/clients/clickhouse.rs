//! The ClickHouse engine's queries answered over the server's HTTP interface. Each is one request:
//! the query is the body, sent as it is written with no parameters, and its rows come back as
//! JSON (`default_format=JSONCompact`). Nothing else is set: a user whose profile is read-only may
//! change no setting, and may still name a format.

use crate::{AnswerCache, Analysis, Answerer, DbError, HostError, ReplayError};
use reylo_engine_clickhouse::Clickhouse;
use reylo_engine_core::database::Database;
use serde_json::Value;
use std::time::Duration;

/// What a ClickHouse describe is answered with: never anything, its describe being a query.
pub type Describe = <Clickhouse as Database>::Describe;

/// How long a connection attempt may take, as the other programs'.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long one of the engine's queries may take, none of which reads a table's rows.
const QUERY_TIMEOUT: Duration = Duration::from_secs(60);

/// The most of an answer that is read: a describe's or a query tree's is a few kilobytes.
const ANSWER_LIMIT: u64 = 256 << 20;

/// The analysis of `sql`, its queries answered by `server`.
pub fn analyze(server: &Server, sql: &str, cache: &mut AnswerCache<Describe>) -> Result<Analysis, ReplayError<HostError>> {
    crate::replay::run::<Clickhouse, _>(sql, &mut Answers(server), cache)
}

/// A ClickHouse server's HTTP interface, as a URL names it.
pub struct Server {
    agent: ureq::Agent,
    /// Where each request goes: the scheme, the host and port, and the path before the query.
    endpoint: String,
    /// What each request's URL carries: the database, a role, and the settings the URL names.
    parameters: Vec<(String, String)>,
    /// What each request's headers carry: the user and password, and the headers the URL names.
    headers: Vec<(String, String)>,
}

/// What a request came to: the server's answer, or its failure to give one.
pub enum Outcome {
    /// The rows of the answer.
    Rows(Vec<Vec<Value>>),
    /// The server's error, with the code it carries in `X-ClickHouse-Exception-Code`.
    Refused { message: String, code: Option<String>, status: u16 },
    /// An answer that is not one of ClickHouse's HTTP interface.
    Foreign(String),
}

/// A URL's text, percent-decoded.
fn percent(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match (bytes[i], bytes.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl Server {
    /// The server a URL names, as `@clickhouse/client` reads one:
    /// `http[s]://[user[:password]@]host[:port][/database][?parameters]`. A parameter named
    /// `clickhouse_setting_<name>` or `ch_<name>` is a setting every query runs under, as the
    /// client sends it, so the statements are described as the app runs them; `role` is a role,
    /// `http_header_<name>` a header, `access_token` a bearer token in place of a password, and
    /// `pathname` a path in front of the interface, behind a proxy. The client's other parameters
    /// are its own.
    pub fn of(url: &str) -> Result<Server, String> {
        let (scheme, rest) = url.split_once("://").ok_or("cannot read the URL: it has no scheme")?;
        if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
            return Err(format!("cannot read the URL: a ClickHouse database is named by the URL of its HTTP interface, http:// or https://, not {scheme}://"));
        }
        let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (login, host) = authority.rsplit_once('@').map_or((None, authority), |(login, host)| (Some(login), host));
        if host.is_empty() {
            return Err("cannot read the URL: it names no host".to_string());
        }
        let mut parameters = Vec::new();
        let mut headers = Vec::new();
        let database = percent(path.split('/').next().unwrap_or_default());
        if !database.is_empty() {
            parameters.push(("database".to_string(), database));
        }
        if let Some(login) = login {
            let (user, password) = login.split_once(':').unwrap_or((login, ""));
            headers.push(("X-ClickHouse-User".to_string(), percent(user)));
            headers.push(("X-ClickHouse-Key".to_string(), percent(password)));
        }
        let mut pathname = String::new();
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let value = percent(&value.replace('+', " "));
            let setting = key.strip_prefix("clickhouse_setting_").or_else(|| key.strip_prefix("ch_"));
            match (key, setting, key.strip_prefix("http_header_")) {
                (_, Some(setting), _) => parameters.push((setting.to_string(), value)),
                (_, _, Some(header)) => headers.push((header.to_string(), value)),
                ("role", ..) => parameters.push(("role".to_string(), value)),
                ("pathname", ..) => pathname = format!("/{}", value.trim_matches('/')),
                ("access_token", ..) => {
                    headers.retain(|(name, _)| !name.starts_with("X-ClickHouse-"));
                    headers.push(("Authorization".to_string(), format!("Bearer {value}")));
                }
                _ => {}
            }
        }
        let config = ureq::Agent::config_builder().http_status_as_error(false).timeout_connect(Some(CONNECT_TIMEOUT)).timeout_global(Some(QUERY_TIMEOUT)).build();
        Ok(Server { agent: ureq::Agent::new_with_config(config), endpoint: format!("{}://{host}{pathname}/", scheme.to_ascii_lowercase()), parameters, headers })
    }

    /// The host and port, and the database where the URL names one, for a message.
    pub fn place(&self) -> String {
        let host = self.endpoint.split_once("://").map_or(self.endpoint.as_str(), |(_, rest)| rest).trim_end_matches('/');
        match self.parameters.iter().find(|(name, _)| name == "database") {
            Some((_, database)) => format!("{host}/{database}"),
            None => host.to_string(),
        }
    }

    /// One query's outcome, or why no answer came: the server cannot be reached, or went away.
    pub fn ask(&self, sql: &str) -> Result<Outcome, String> {
        let mut request = self.agent.post(self.endpoint.as_str()).query("default_format", "JSONCompact");
        for (name, value) in &self.parameters {
            request = request.query(name.as_str(), value.as_str());
        }
        for (name, value) in &self.headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let mut response = request.send(sql.as_bytes()).map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let header = |name: &str| response.headers().get(name).and_then(|v| v.to_str().ok()).map(String::from);
        let code = header("x-clickhouse-exception-code");
        let ours = code.is_some() || header("x-clickhouse-query-id").is_some() || header("x-clickhouse-summary").is_some();
        let body = response.body_mut().with_config().limit(ANSWER_LIMIT).read_to_string().map_err(|e| e.to_string())?;
        if status != 200 {
            return Ok(if ours { Outcome::Refused { message: body.trim().to_string(), code, status } } else { Outcome::Foreign(format!("HTTP {status}")) });
        }
        if body.trim().is_empty() {
            return Ok(Outcome::Rows(Vec::new()));
        }
        let answer: Value = match serde_json::from_str(&body) {
            Ok(answer) => answer,
            Err(_) if ours => return Ok(Outcome::Refused { message: body.trim().to_string(), code, status }),
            Err(_) => return Ok(Outcome::Foreign("an answer that is no JSON".to_string())),
        };
        // An error raised once the answer had begun is written into it.
        if let Some(exception) = answer.get("exception").and_then(Value::as_str) {
            return Ok(Outcome::Refused { message: exception.to_string(), code, status });
        }
        let rows = answer.get("data").and_then(Value::as_array).map(|rows| rows.iter().map(|row| row.as_array().cloned().unwrap_or_default()).collect());
        Ok(rows.map_or_else(|| Outcome::Foreign("an answer with no rows in it".to_string()), Outcome::Rows))
    }
}

/// The engine's queries, answered by a server.
pub struct Answers<'a>(pub &'a Server);

impl Answerer<Describe> for Answers<'_> {
    type Error = HostError;

    fn describe(&mut self, sql: &str) -> Result<Result<Describe, DbError>, HostError> {
        Err(HostError(format!("the ClickHouse engine asked to describe {sql}, which it asks as a query")))
    }

    fn query(&mut self, sql: &str, _params: &[Value]) -> Result<Result<Vec<Vec<Value>>, DbError>, HostError> {
        match self.0.ask(sql) {
            Ok(Outcome::Rows(rows)) => Ok(Ok(rows)),
            Ok(Outcome::Refused { message, code, .. }) => Ok(Err(DbError { message, code, position: None })),
            Ok(Outcome::Foreign(what)) => Err(HostError(format!("the server at {} answered with {what}, which ClickHouse's HTTP interface does not", self.0.place()))),
            Err(e) => Err(HostError(format!("the query did not reach the server: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_is_read_as_the_client_reads_it() {
        let server = Server::of("https://app:s%40cret@db.example:8443/shop?clickhouse_setting_join_use_nulls=1&ch_max_threads=2&role=reader&request_timeout=5000&http_header_X-Trace=a+b").unwrap();
        assert_eq!(server.endpoint, "https://db.example:8443/");
        assert_eq!(server.parameters, [("database", "shop"), ("join_use_nulls", "1"), ("max_threads", "2"), ("role", "reader")].map(|(k, v)| (k.to_string(), v.to_string())));
        assert_eq!(server.headers, [("X-ClickHouse-User", "app"), ("X-ClickHouse-Key", "s@cret"), ("X-Trace", "a b")].map(|(k, v)| (k.to_string(), v.to_string())));
        assert_eq!(server.place(), "db.example:8443/shop");
        let plain = Server::of("http://localhost:8123").unwrap();
        assert_eq!((plain.endpoint.as_str(), plain.parameters.len(), plain.headers.len(), plain.place()), ("http://localhost:8123/", 0, 0, "localhost:8123".to_string()));
        let proxied = Server::of("HTTP://localhost/analytics?pathname=/clickhouse/&access_token=t0ken").unwrap();
        assert_eq!(proxied.endpoint, "http://localhost/clickhouse/");
        assert_eq!(proxied.headers, [("Authorization".to_string(), "Bearer t0ken".to_string())]);
    }

    #[test]
    fn a_url_of_another_kind_is_refused_by_what_to_write() {
        assert!(Server::of("clickhouse://localhost:9000/app").err().unwrap().contains("http:// or https://, not clickhouse://"));
        assert!(Server::of("localhost:8123").is_err());
        assert!(Server::of("http:///app").err().unwrap().contains("names no host"));
    }
}
