//! The `pgreylo` Postgres extension, seen from a client.
//!
//! The extension (`pg-extension/`) hosts this same engine inside the server, where a need is a
//! function call and not a round trip. A client that finds it has two ways to use it:
//!
//! - The server's engine was built from the same sources as this one ([`crate::FINGERPRINT`]):
//!   the two give the same answer, so the client asks for the whole analysis,
//!   `SELECT pgreylo.analyze($1)`, in one query.
//! - It is another build: the rules stay the client's, and each round's statement-scoped needs
//!   travel as one query, `SELECT pgreylo.answer($1)`, which answers every one of them in the
//!   server. Session-scoped needs stay as they are, so the host goes on caching them.
//!
//! Without the extension nothing changes. Finding out costs the first statement of a session one
//! round for a session-scoped catalog read, and a second for `pgreylo.version()` where the
//! extension exists; the host keeps both answers. The statement's own needs wait for that
//! answer: asked in the same round, a statement that settles in one round would be done before
//! the answer came, and the extension would never be used.
//!
//! Only [`crate::api::Postgres`]'s round goes through here, which [`reylo_engine_core::api::call`]
//! runs. [`crate::postgres::analyze`] never does, because the extension itself calls it, and an
//! engine in the server that asked the server's engine would not end.

use crate::api::{Analysis, Answer, Column, Diagnostic, Need, Param, Scope};
use crate::dialect::Dialect;
use crate::host::{as_str, Host, Lazy};
use crate::lex::{self, Target};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Reads nothing but the catalog, so it cannot fail where the extension is absent; an error
/// here would be asked again for every statement, since a host does not keep a failure.
pub const PROBE_QUERY: &str = "SELECT extversion::text FROM pg_extension WHERE extname = 'pgreylo'";
pub const VERSION_QUERY: &str = "SELECT pgreylo.version()";
pub const ANALYZE_QUERY: &str = "SELECT pgreylo.analyze($1::text)";
/// The needs travel as text that the server casts: a parameter described as `jsonb` makes a
/// driver JSON-encode the string it is handed, and a JSON string is not the array.
pub const ANSWER_QUERY: &str = "SELECT pgreylo.answer($1::text::jsonb)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Server {
    /// No extension: every need is the host's.
    Absent,
    /// The extension, with this engine's build inside.
    Same,
    /// The extension with another build of the engine, or with no engine library beside it.
    Other { answers: bool },
}

/// What the server offers. Two session-scoped reads, the second only where the first found the
/// extension.
pub fn server(host: &Host) -> Lazy<Server> {
    let rows = host.query(PROBE_QUERY, vec![], Scope::Session)?;
    if rows.map(|r| r.is_empty()).unwrap_or(true) {
        return Some(Server::Absent);
    }
    let rows = host.query(VERSION_QUERY, vec![], Scope::Session)?;
    let version = rows.ok().and_then(|r| r.into_iter().next()).and_then(|r| r.into_iter().next()).map(json_value);
    let fingerprint = version.as_ref().and_then(|v| v.get("fingerprint")).and_then(as_str);
    Some(match fingerprint {
        Some(f) if f == crate::FINGERPRINT => Server::Same,
        Some(_) => Server::Other { answers: true },
        None => Server::Other { answers: false },
    })
}

/// What the server's analysis carries that the client cannot work out alone. The lexing (the
/// rewritten text, the style) is the same on both sides and stays local.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Remote {
    kind: String,
    params: Vec<Param>,
    columns: Vec<Column>,
    cardinality: String,
    #[serde(default)]
    cardinality_reason: Option<String>,
    #[serde(default)]
    diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    unresolved: Vec<String>,
}

/// The whole analysis from the server. `Some(None)` is a server that did not answer it (the
/// function failed, or answered something this build does not read): the caller goes on alone.
pub fn analyze(sql: &str, host: &Host) -> Lazy<Option<Analysis>> {
    let Ok(lexed) = lex::lex(sql, Dialect::Postgres) else { return Some(None) };
    let rows = host.query(ANALYZE_QUERY, vec![json!(sql)], Scope::Statement)?;
    let Some(value) = rows.ok().and_then(|r| r.into_iter().next()).and_then(|r| r.into_iter().next()) else { return Some(None) };
    let Ok(remote) = serde_json::from_value::<Remote>(json_value(value)) else { return Some(None) };
    Some(Some(Analysis {
        kind: remote.kind,
        params: remote.params,
        columns: remote.columns,
        cardinality: remote.cardinality,
        cardinality_reason: remote.cardinality_reason,
        rewritten: lex::rewrite(sql, &lexed, Target::Dollar),
        style: lexed.style,
        diagnostics: remote.diagnostics,
        unresolved: remote.unresolved,
        structured: Vec::new(),
        fresh: None,
        errors: None,
        tables: None,
    }))
}

/// One round's needs as the host should see them: the statement-scoped ones folded into one
/// query the extension answers.
pub fn bundle(needs: Vec<Need>) -> Vec<Need> {
    let (inside, mut outside): (Vec<Need>, Vec<Need>) = needs.into_iter().partition(|need| match need {
        Need::Describe { .. } => true,
        Need::Query { scope, .. } => *scope == Scope::Statement,
    });
    if inside.len() < 2 {
        outside.extend(inside);
        return outside;
    }
    // As JSON text, not as a JSON array: a driver would send an array as a Postgres array, and
    // the query casts from text so no driver JSON-encodes the text either.
    let params = vec![Value::String(serde_json::to_string(&inside).expect("needs serialize"))];
    let id = format!("query:{ANSWER_QUERY}|{}", serde_json::to_string(&params).unwrap_or_default());
    outside.push(Need::Query { id, sql: ANSWER_QUERY.to_string(), params, scope: Scope::Statement });
    outside
}

/// Put the answers a bundle brought back where the engine looks for them. A bundle that failed
/// as a whole leaves its needs unanswered, and they are asked again one by one.
pub fn unbundle(answers: &mut HashMap<String, Answer>) {
    let prefix = format!("query:{ANSWER_QUERY}|");
    let mut found: Vec<(String, Answer)> = Vec::new();
    for (id, answer) in answers.iter() {
        let (true, Answer::Rows { rows }) = (id.starts_with(&prefix), answer) else { continue };
        let Some(Value::Object(inner)) = rows.first().and_then(|r| r.first()).cloned().map(json_value) else { continue };
        for (need, value) in inner {
            if let Ok(answer) = serde_json::from_value::<Answer>(value) {
                found.push((need, answer));
            }
        }
    }
    for (need, answer) in found {
        answers.entry(need).or_insert(answer);
    }
}

/// Whether a bundle for these needs was already asked and brought nothing back for them, so
/// asking it again would not end.
pub fn bundle_failed(needs: &[Need], answers: &HashMap<String, Answer>) -> bool {
    match bundle(needs.to_vec()).into_iter().find(|n| matches!(n, Need::Query { sql, .. } if sql == ANSWER_QUERY)) {
        Some(Need::Query { id, .. }) => answers.contains_key(&id),
        _ => false,
    }
}

/// A `jsonb` column as the driver handed it over: parsed already, or still text.
fn json_value(value: Value) -> Value {
    match value {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(value: Value) -> Answer {
        Answer::Rows { rows: vec![vec![value]] }
    }

    fn probe_id() -> String {
        format!("query:{PROBE_QUERY}|[]")
    }

    fn version_id() -> String {
        format!("query:{VERSION_QUERY}|[]")
    }

    #[test]
    fn a_server_without_the_extension_is_asked_once_and_left_alone() {
        let mut answers = HashMap::new();
        assert_eq!(server(&Host::new(&answers)), None);
        answers.insert(probe_id(), Answer::Rows { rows: vec![] });
        let host = Host::new(&answers);
        assert_eq!(server(&host), Some(Server::Absent));
        assert!(host.needs().is_empty());
    }

    #[test]
    fn the_fingerprint_decides_between_the_whole_analysis_and_answered_rounds() {
        let mut answers = HashMap::new();
        answers.insert(probe_id(), rows(json!("0.1.0")));
        assert_eq!(server(&Host::new(&answers)), None);
        answers.insert(version_id(), rows(json!({ "extension": "0.1.0", "engine": "0.1.0", "fingerprint": crate::FINGERPRINT })));
        assert_eq!(server(&Host::new(&answers)), Some(Server::Same));
        // A driver that hands jsonb over as text reads the same.
        answers.insert(version_id(), rows(json!(format!("{{\"fingerprint\":\"{}\"}}", crate::FINGERPRINT))));
        assert_eq!(server(&Host::new(&answers)), Some(Server::Same));
        answers.insert(version_id(), rows(json!({ "extension": "0.1.0", "engine": "0.1.0", "fingerprint": "another build" })));
        assert_eq!(server(&Host::new(&answers)), Some(Server::Other { answers: true }));
        answers.insert(version_id(), rows(json!({ "extension": "0.1.0", "engine": null, "fingerprint": null })));
        assert_eq!(server(&Host::new(&answers)), Some(Server::Other { answers: false }));
    }

    #[test]
    fn a_round_travels_as_one_query_and_comes_back_as_its_needs() {
        let needs = vec![
            Need::Query { id: "query:S|[]".into(), sql: "S".into(), params: vec![], scope: Scope::Session },
            Need::Describe { id: "describe:select 1".into(), sql: "select 1".into() },
            Need::Query { id: "query:Q|[[\"users\"]]".into(), sql: "Q".into(), params: vec![json!(["users"])], scope: Scope::Statement },
        ];
        let bundled = bundle(needs.clone());
        assert_eq!(bundled.len(), 2);
        assert_eq!(bundled[0], needs[0]);
        let Need::Query { id, sql, params, .. } = &bundled[1] else { panic!("the bundle is a query") };
        assert_eq!(sql, ANSWER_QUERY);
        let inside: Vec<Value> = serde_json::from_str(params[0].as_str().expect("the needs travel as JSON text")).unwrap();
        assert_eq!(inside.len(), 2);
        assert_eq!(inside[0]["kind"], "describe");
        // One need alone is not worth a bundle.
        assert_eq!(bundle(needs[..2].to_vec()), needs[..2].to_vec());

        let mut answers = HashMap::new();
        assert!(!bundle_failed(&needs, &answers));
        answers.insert(id.clone(), rows(json!({
            "describe:select 1": { "describe": { "params": [], "columns": [{ "name": "?column?", "oid": 23, "table": 0, "attnum": 0 }] } },
            "query:Q|[[\"users\"]]": { "rows": [["users", true]] },
        })));
        unbundle(&mut answers);
        assert!(matches!(answers.get("describe:select 1"), Some(Answer::Describe { describe }) if describe.columns[0].oid == 23));
        assert!(matches!(answers.get("query:Q|[[\"users\"]]"), Some(Answer::Rows { rows }) if rows.len() == 1));
        assert!(bundle_failed(&needs, &answers));
    }
}
