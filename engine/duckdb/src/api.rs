//! The DuckDB side of the request protocol ([`reylo_engine_core::api`]): what a describe is
//! answered with.

use crate::host::Host;
use reylo_engine_core::database::Database;
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::host::Lazy;
use serde::Deserialize;

pub use reylo_engine_core::api::{Analysis, Column, DbError, Diagnostic, Need, Param, Response, Scope};

/// What the host learned for one need: a describe is a [`DuckdbDescribe`].
pub type Answer = reylo_engine_core::api::Answer<DuckdbDescribe>;
pub type Answers = reylo_engine_core::api::Answers<DuckdbDescribe>;

/// What DuckDB's prepared statement reports before it runs: the statement's type, each
/// parameter's type, and each result column's name and type.
#[derive(Debug, Clone, Deserialize)]
pub struct DuckdbDescribe {
    /// DuckDB's name for the statement's type: `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`,
    /// `CALL`, … (`duckdb_statement_type`), empty where the host does not say.
    #[serde(default)]
    pub statement: String,
    pub params: Vec<DuckdbParam>,
    pub columns: Vec<DuckdbColumn>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DuckdbParam {
    /// The parameter's name as DuckDB numbers it: `1` for `$1`.
    #[serde(default)]
    pub name: String,
    /// `None` where DuckDB cannot tell the type from where the parameter stands (`select $1`).
    #[serde(default, rename = "type")]
    pub ty: Option<LogicalType>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DuckdbColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: LogicalType,
}

/// A DuckDB logical type: its id (`DuckDBTypeId`'s name, `INTEGER`, `DECIMAL`, `LIST`, …), its
/// text as DuckDB prints it (`DECIMAL(10,2)`, `VARCHAR[]`), its alias (`JSON`), and what the id
/// says more of.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LogicalType {
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub alias: Option<String>,
    /// A `DECIMAL`'s width and scale.
    #[serde(default)]
    pub width: Option<u8>,
    #[serde(default)]
    pub scale: Option<u8>,
    /// A `LIST`'s or an `ARRAY`'s element type.
    #[serde(default)]
    pub child: Option<Box<LogicalType>>,
    /// An `ARRAY`'s length.
    #[serde(default)]
    pub size: Option<u32>,
    /// A `STRUCT`'s fields, or a `UNION`'s members.
    #[serde(default)]
    pub fields: Vec<NamedType>,
    /// A `MAP`'s key and value types.
    #[serde(default)]
    pub key: Option<Box<LogicalType>>,
    #[serde(default)]
    pub value: Option<Box<LogicalType>>,
    /// An `ENUM`'s labels, in order.
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamedType {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: LogicalType,
}

/// DuckDB, for [`reylo_engine_core::api::call`].
pub struct Duckdb;

impl Database for Duckdb {
    const DIALECT: Dialect = Dialect::Duckdb;
    const FINGERPRINT: &'static str = crate::FINGERPRINT;
    type Describe = DuckdbDescribe;

    fn analyze(sql: &str, host: &Host) -> Lazy<Analysis> {
        host.on_engine_stack(|host| crate::duckdb::analyze_text(sql, host))
    }
}

#[cfg(test)]
mod protocol {
    use super::*;
    use reylo_engine_core::api::call;
    use serde_json::Value;

    fn ask(request: &str) -> Value {
        serde_json::from_str(&call::<Duckdb>(request)).unwrap()
    }

    #[test]
    fn the_artifact_names_its_database_and_refuses_another_s_statement() {
        let ping = ask(r#"{"op":"ping"}"#);
        assert_eq!((ping["status"].as_str(), ping["database"].as_str(), ping["fingerprint"].as_str()), (Some("ok"), Some("duckdb"), Some(crate::FINGERPRINT)));
        // Lexing and the project file are core's, for any dialect.
        assert_eq!(ask(r#"{"op":"lex","sql":"select :a","dialect":"postgres"}"#)["lexed"]["params"], serde_json::json!(["a"]));
        assert_eq!(ask(r#"{"op":"config","text":"url: duckdb:./app.duckdb"}"#)["status"], "config");
        let refused = ask(r#"{"op":"analyze","sql":"select 1","dialect":"sqlite","answers":{}}"#);
        assert_eq!((refused["status"].as_str(), refused["message"].as_str()), (Some("error"), Some("this engine reads duckdb statements, not sqlite ones")));
    }
}
