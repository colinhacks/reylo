//! The ClickHouse side of the request protocol ([`reylo_engine_core::api`]). A host answers its
//! queries alone, each sent as it is written, with no parameters: the describe is one of them
//! ([`crate::clickhouse`]), so no describe payload of its own exists.

use crate::host::Host;
use reylo_engine_core::database::Database;
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::host::Lazy;

pub use reylo_engine_core::api::{Analysis, Column, DbError, Diagnostic, Need, NoDescribe, Param, Response, Scope};

pub type Answer = reylo_engine_core::api::Answer<NoDescribe>;
pub type Answers = reylo_engine_core::api::Answers<NoDescribe>;

/// ClickHouse, for [`reylo_engine_core::api::call`].
pub struct Clickhouse;

impl Database for Clickhouse {
    const DIALECT: Dialect = Dialect::Clickhouse;
    const FINGERPRINT: &'static str = crate::FINGERPRINT;
    type Describe = NoDescribe;

    fn analyze(sql: &str, host: &Host) -> Lazy<Analysis> {
        crate::clickhouse::analyze(sql, host)
    }

    /// A SELECT writes nothing on ClickHouse: no function it calls can.
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
        serde_json::from_str(&call::<Clickhouse>(request)).unwrap()
    }

    #[test]
    fn the_artifact_names_its_database_and_refuses_another_s_statement() {
        let ping = ask(r#"{"op":"ping"}"#);
        assert_eq!((ping["status"].as_str(), ping["database"].as_str(), ping["fingerprint"].as_str()), (Some("ok"), Some("clickhouse"), Some(crate::FINGERPRINT)));
        assert_eq!(ask(r#"{"op":"lex","sql":"select {a:UInt8}","dialect":"clickhouse"}"#)["lexed"]["params"], serde_json::json!(["a"]));
        let refused = ask(r#"{"op":"analyze","sql":"select 1","dialect":"mysql"}"#);
        assert_eq!((refused["status"].as_str(), refused["message"].as_str()), (Some("error"), Some("this engine reads clickhouse statements, not mysql ones")));
    }
}
