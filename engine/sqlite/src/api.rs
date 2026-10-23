//! The SQLite side of the request protocol ([`reylo_engine_core::api`]).

use crate::host::Host;
use reylo_engine_core::api::NoDescribe;
use reylo_engine_core::database::Database;
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::host::Lazy;

pub use reylo_engine_core::api::{Analysis, Column, DbError, Diagnostic, Need, Param, Response, Scope};

/// What the host learned for one need: never a describe.
pub type Answer = reylo_engine_core::api::Answer<NoDescribe>;
pub type Answers = reylo_engine_core::api::Answers<NoDescribe>;

/// SQLite, for [`reylo_engine_core::api::call`].
pub struct Sqlite;

impl Database for Sqlite {
    const DIALECT: Dialect = Dialect::Sqlite;
    const FINGERPRINT: &'static str = crate::FINGERPRINT;
    type Describe = NoDescribe;

    /// The replayed analysis: the host answers the schema and function queries
    /// ([`crate::sqlite::analyze`]), the statement's structured parameters expanded first.
    fn analyze(sql: &str, host: &Host) -> Lazy<Analysis> {
        host.on_engine_stack(|host| {
            let inner = |sql: &str, marked: &[String]| match crate::analysis::lexed(sql, Dialect::Sqlite) {
                Ok(lexed) => crate::sqlite::analyze(sql, &lexed, host, marked),
                Err(bare) => Some(bare),
            };
            crate::structured::analyze(sql, Dialect::Sqlite, host, &inner, &|diagnostics| crate::analysis::bare_of(sql, Dialect::Sqlite, diagnostics))
        })
    }

    /// The unique keys, NOT NULL columns and STRICT tables the statement's constraint errors may
    /// name, where it fires no trigger.
    fn errors(analysis: &Analysis, host: &Host) -> Lazy<Option<reylo_engine_core::errors::Errors>> {
        crate::errors::errors(analysis, host)
    }
}

#[cfg(test)]
mod protocol {
    use super::*;
    use reylo_engine_core::api::call;
    use serde_json::Value;

    fn ask(request: &str) -> Value {
        serde_json::from_str(&call::<Sqlite>(request)).unwrap()
    }

    #[test]
    fn the_artifact_names_its_database_and_refuses_another_s_statement() {
        let ping = ask(r#"{"op":"ping"}"#);
        assert_eq!((ping["status"].as_str(), ping["database"].as_str(), ping["fingerprint"].as_str()), (Some("ok"), Some("sqlite"), Some(crate::FINGERPRINT)));
        // Lexing and the project file are core's, for any dialect.
        assert_eq!(ask(r#"{"op":"lex","sql":"select :a","dialect":"postgres"}"#)["lexed"]["params"], serde_json::json!(["a"]));
        assert_eq!(ask(r#"{"op":"config","text":"url: postgres://localhost/app"}"#)["status"], "config");
        let refused = ask(r#"{"op":"analyze","sql":"select 1","dialect":"postgres","answers":{"describe:select 1":{"describe":{"params":[],"columns":[]}}}}"#);
        assert_eq!((refused["status"].as_str(), refused["message"].as_str()), (Some("error"), Some("this engine reads sqlite statements, not postgres ones")));
    }
}
