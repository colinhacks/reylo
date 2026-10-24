//! The MySQL side of the request protocol ([`reylo_engine_core::api`]): what a describe is
//! answered with.

use crate::host::Host;
use reylo_engine_core::database::Database;
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::host::Lazy;
use serde::Deserialize;

pub use reylo_engine_core::api::{Analysis, Column, DbError, Diagnostic, Need, Param, Response, Scope};

/// What the host learned for one need: a describe is a [`MysqlDescribe`], which a host writes as
/// `{"mysql": …}`.
pub type Answer = reylo_engine_core::api::Answer<MysqlDescribe>;
pub type Answers = reylo_engine_core::api::Answers<MysqlDescribe>;

/// MySQL's prepare metadata as mysql2 reports it: the protocol type code, the flags (bit 1 is
/// NOT NULL) and the charset (63 is binary) per result column, and the server's type guess per `?`.
/// A column also carries its display length and the table and column it is a column of, which
/// are empty for an expression; a host that sends none of them gets the answer that holds
/// whatever they are.
#[derive(Debug, Clone, Deserialize)]
pub struct MysqlDescribe {
    pub columns: Vec<MysqlColumn>,
    pub params: Vec<MysqlParam>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MysqlColumn {
    pub name: String,
    pub r#type: u32,
    pub flags: u32,
    #[serde(default)]
    pub charset: u32,
    #[serde(default)]
    pub length: Option<u32>,
    #[serde(default)]
    pub org_table: Option<String>,
    /// The column's own name in `org_table`, whatever the statement calls it.
    #[serde(default)]
    pub org_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MysqlParam {
    pub r#type: u32,
    #[serde(default)]
    pub flags: u32,
    #[serde(default)]
    pub charset: u32,
}

/// MySQL, for [`reylo_engine_core::api::call`].
pub struct Mysql;

impl Database for Mysql {
    const DIALECT: Dialect = Dialect::Mysql;
    const FINGERPRINT: &'static str = crate::FINGERPRINT;
    type Describe = MysqlDescribe;

    fn analyze(sql: &str, host: &Host) -> Lazy<Analysis> {
        crate::mysql::analyze_text(sql, host)
    }

    /// The keys, foreign keys, checks and columns the statement's errors may name, where it runs no
    /// trigger and no stored function.
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
        serde_json::from_str(&call::<Mysql>(request)).unwrap()
    }

    #[test]
    fn the_artifact_names_its_database_and_refuses_another_s_statement() {
        let ping = ask(r#"{"op":"ping"}"#);
        assert_eq!((ping["status"].as_str(), ping["database"].as_str(), ping["fingerprint"].as_str()), (Some("ok"), Some("mysql"), Some(crate::FINGERPRINT)));
        // Lexing and the project file are core's, for any dialect.
        assert_eq!(ask(r#"{"op":"lex","sql":"select :a","dialect":"sqlite"}"#)["lexed"]["params"], serde_json::json!(["a"]));
        assert_eq!(ask(r#"{"op":"config","text":"url: postgres://localhost/app"}"#)["status"], "config");
        let refused = ask(r#"{"op":"analyze","sql":"select 1","dialect":"sqlite","answers":{"describe:select 1":{"describe":{"params":[],"columns":[]}}}}"#);
        assert_eq!((refused["status"].as_str(), refused["message"].as_str()), (Some("error"), Some("this engine reads mysql statements, not sqlite ones")));
    }
}
