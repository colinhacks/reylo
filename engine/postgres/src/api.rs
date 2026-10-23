//! The Postgres side of the request protocol ([`reylo_engine_core::api`]): what a describe is
//! answered with, and [`Postgres`], whose round settles what the server's `pgreylo` extension
//! offers before the statement asks for anything ([`crate::extension`]).

use crate::extension::{self, Server};
use crate::host::Host;
use reylo_engine_core::database::{with_facts, Database};
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::fresh::Tables;
use reylo_engine_core::host::Lazy;
use serde::Deserialize;

pub use reylo_engine_core::api::{Analysis, Column, DbError, Diagnostic, Need, Param, Response, Scope};

/// What the host learned for one need: a describe is a [`RawDescribe`].
pub type Answer = reylo_engine_core::api::Answer<RawDescribe>;
pub type Answers = reylo_engine_core::api::Answers<RawDescribe>;

/// A Parse/Describe result as the driver reports it: parameter type oids in order, and result
/// columns with their type and, for a plain column, the table oid and attribute number.
#[derive(Debug, Clone, Deserialize)]
pub struct RawDescribe {
    pub params: Vec<u32>,
    pub columns: Vec<RawColumn>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawColumn {
    pub name: String,
    pub oid: u32,
    #[serde(default)]
    pub table: u32,
    #[serde(default)]
    pub attnum: i32,
}

/// Postgres, for [`reylo_engine_core::api::call`].
pub struct Postgres;

impl Database for Postgres {
    const DIALECT: Dialect = Dialect::Postgres;
    const FINGERPRINT: &'static str = crate::FINGERPRINT;
    type Describe = RawDescribe;

    /// Never probes for `pgreylo`: the extension itself calls this, and an engine in the server
    /// that asked the server's engine would not end.
    fn analyze(sql: &str, host: &Host) -> Lazy<Analysis> {
        crate::postgres::analyze(sql, host)
    }

    /// The tables libpg_query reads in the statement as it runs.
    fn tables(analysis: &Analysis) -> Option<Tables> {
        let parsed = crate::analyze::analyze(&analysis.rewritten.sql).ok()?;
        Some(Tables { read: parsed.read_set, write: parsed.write_set })
    }

    /// One round of the replay, through the server's `pgreylo` extension where there is one: the
    /// whole analysis from a server with this engine inside, a round's needs in one query from a
    /// server with another build, and the needs as they are otherwise.
    fn round(sql: &str, answers: &Answers) -> Response {
        let host = Host::new(answers);
        // What the server offers is settled before the statement asks for anything: were the
        // statement's own needs asked in the same round, a statement that settles in one round would
        // be done before the answer came, and the extension would never be used.
        let Some(server) = extension::server(&host) else { return Response::Need { needs: host.needs() } };
        // A structured parameter, a typed JSON one and a cast to `nullable(…)` are expanded before
        // the analysis, and the server's answer carries neither the expansion nor the lists the
        // run time writes again, so such a statement is analyzed here, with the server answering
        // its needs.
        if server == Server::Same && !reylo_engine_core::structured::present(sql, Dialect::Postgres) {
            match extension::analyze(sql, &host) {
                Some(Some(analysis)) => return with_facts::<Postgres>(analysis, &host),
                Some(None) => {}
                None => return Response::Need { needs: host.needs() },
            }
        }
        match Self::analyze(sql, &host) {
            Some(analysis) => with_facts::<Postgres>(analysis, &host),
            None => {
                let needs = host.needs();
                let bundled = matches!(server, Server::Same | Server::Other { answers: true }) && !extension::bundle_failed(&needs, answers);
                Response::Need { needs: if bundled { extension::bundle(needs) } else { needs } }
            }
        }
    }

    fn unbundle(answers: &mut Answers) {
        extension::unbundle(answers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::{ANALYZE_QUERY, PROBE_QUERY, VERSION_QUERY};
    use serde_json::json;

    fn need_ids(response: &Response) -> Vec<String> {
        match response {
            Response::Need { needs } => needs.iter().map(|n| match n {
                Need::Describe { id, .. } | Need::Query { id, .. } => id.clone(),
            }).collect(),
            _ => panic!("a round of needs"),
        }
    }

    #[test]
    fn the_server_is_settled_before_the_statement_asks_for_anything() {
        let sql = "select 1";
        let analyze = |answers: &Answers| Postgres::round(sql, answers);
        let mut answers = Answers::new();
        // The first round of a session is the probe alone.
        assert_eq!(need_ids(&analyze(&answers)), vec![format!("query:{PROBE_QUERY}|[]")]);
        // Without the extension, the statement's own needs follow, unbundled.
        answers.insert(format!("query:{PROBE_QUERY}|[]"), Answer::Rows { rows: vec![] });
        let ids = need_ids(&analyze(&answers));
        assert!(ids.iter().any(|id| id == &format!("describe:{sql}")), "{ids:?}");
        assert!(ids.iter().all(|id| !id.contains("reylo.")), "{ids:?}");
        // With the same build inside the server, the statement is one query.
        answers.insert(format!("query:{PROBE_QUERY}|[]"), Answer::Rows { rows: vec![vec![json!("0.1.0")]] });
        assert_eq!(need_ids(&analyze(&answers)), vec![format!("query:{VERSION_QUERY}|[]")]);
        answers.insert(format!("query:{VERSION_QUERY}|[]"), Answer::Rows { rows: vec![vec![json!({ "fingerprint": crate::FINGERPRINT })]] });
        let ids = need_ids(&analyze(&answers));
        assert_eq!(ids.len(), 1);
        assert!(ids[0].starts_with(&format!("query:{ANALYZE_QUERY}|")), "{ids:?}");
    }

    #[test]
    fn a_statement_the_engine_rewrites_is_analyzed_here_and_never_by_the_server() {
        let mut answers = Answers::new();
        answers.insert(format!("query:{PROBE_QUERY}|[]"), Answer::Rows { rows: vec![vec![json!("0.1.0")]] });
        answers.insert(format!("query:{VERSION_QUERY}|[]"), Answer::Rows { rows: vec![vec![json!({ "fingerprint": crate::FINGERPRINT })]] });
        let asks_the_server = |sql: &str| need_ids(&Postgres::round(sql, &answers)).iter().any(|id| id.starts_with(&format!("query:{ANALYZE_QUERY}|")));
        // The server's answer carries the statement as the lexer writes it, so a cast the engine
        // writes otherwise is expanded here.
        assert!(!asks_the_server("select * from :ids::rows(id int8)"));
        assert!(!asks_the_server("select :doc::jsonb('{\"type\": \"object\"}') as doc"));
        assert!(!asks_the_server("select :n::nullable(int) as n"));
        assert!(!asks_the_server("select :n :: Nullable (int) as n"));
        // An ordinary cast, `json` and `jsonb` included, is the server's to analyze whole.
        assert!(asks_the_server("select :n::int as n"));
        assert!(asks_the_server("select :doc::jsonb as doc"));
    }
}

#[cfg(test)]
mod protocol {
    use super::*;
    use reylo_engine_core::api::call;
    use serde_json::Value;

    fn ask(request: &str) -> Value {
        serde_json::from_str(&call::<Postgres>(request)).unwrap()
    }

    #[test]
    fn the_artifact_names_its_database_and_refuses_another_s_statement() {
        let ping = ask(r#"{"op":"ping"}"#);
        assert_eq!((ping["status"].as_str(), ping["database"].as_str(), ping["fingerprint"].as_str()), (Some("ok"), Some("postgres"), Some(crate::FINGERPRINT)));
        // Lexing and the project file are core's, for any dialect.
        assert_eq!(ask(r#"{"op":"lex","sql":"select :a","dialect":"mysql"}"#)["lexed"]["params"], serde_json::json!(["a"]));
        assert_eq!(ask(r#"{"op":"config","text":"url: postgres://localhost/app"}"#)["status"], "config");
        let refused = ask(r#"{"op":"analyze","sql":"select 1","dialect":"mysql","answers":{"describe:select 1":{"describe":{"params":[],"columns":[]}}}}"#);
        assert_eq!((refused["status"].as_str(), refused["message"].as_str()), (Some("error"), Some("this engine reads postgres statements, not mysql ones")));
    }
}
