//! The SQL Server side of the request protocol ([`reylo_engine_core::api`]). A host answers its
//! catalog queries alone: the describe is two of them ([`crate::sqlserver`]), so no describe
//! payload of its own exists.

use crate::host::Host;
use reylo_engine_core::database::Database;
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::host::Lazy;

pub use reylo_engine_core::api::{Analysis, Column, DbError, Diagnostic, NoDescribe, Need, Param, Response, Scope};

pub type Answer = reylo_engine_core::api::Answer<NoDescribe>;
pub type Answers = reylo_engine_core::api::Answers<NoDescribe>;

/// SQL Server, for [`reylo_engine_core::api::call`].
pub struct Sqlserver;

impl Database for Sqlserver {
    const DIALECT: Dialect = Dialect::Sqlserver;
    const FINGERPRINT: &'static str = crate::FINGERPRINT;
    type Describe = NoDescribe;

    fn analyze(sql: &str, host: &Host) -> Lazy<Analysis> {
        crate::sqlserver::analyze_text(sql, host)
    }

    /// A SELECT that puts its rows in no table writes nothing: a function it calls cannot either.
    fn read_only(analysis: &Analysis, _host: &Host) -> Lazy<bool> {
        Some(analysis.read_only)
    }
}

#[cfg(test)]
mod protocol {
    use super::*;
    use reylo_engine_core::api::call;
    use serde_json::Value;

    fn ask(request: &str) -> Value {
        serde_json::from_str(&call::<Sqlserver>(request)).unwrap()
    }

    #[test]
    fn the_artifact_names_its_database_and_refuses_another_s_statement() {
        let ping = ask(r#"{"op":"ping"}"#);
        assert_eq!((ping["status"].as_str(), ping["database"].as_str(), ping["fingerprint"].as_str()), (Some("ok"), Some("sqlserver"), Some(crate::FINGERPRINT)));
        assert_eq!(ask(r#"{"op":"lex","sql":"select :a","dialect":"sqlserver"}"#)["lexed"]["params"], serde_json::json!(["a"]));
        let refused = ask(r#"{"op":"analyze","sql":"select 1","dialect":"mysql"}"#);
        assert_eq!((refused["status"].as_str(), refused["message"].as_str()), (Some("error"), Some("this engine reads sqlserver statements, not mysql ones")));
    }
}
