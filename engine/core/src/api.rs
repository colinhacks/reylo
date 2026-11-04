//! The request and response vocabulary every host speaks: the loader's `src/engine.ts`, the
//! parity check's copy of it (`pg-extension/parity/engine.ts`), and the oracle.
//!
//! `analyze` is a replay: the host calls it with the answers it has, the engine either returns
//! the analysis or the list of things it still needs — statements to describe, catalog queries
//! to run — and the host calls again with those answered. The engine is pure, so every round
//! starts from the statement text and the answers; nothing is kept between calls.
//!
//! Each artifact serves one database ([`call`] for one [`Database`]): `lex` and `config` are core's
//! and answer for any dialect, `ping` names the database the artifact reads, and an `analyze` for
//! another database's statement is refused by name.

use crate::database::Database;
use crate::dialect::Dialect;
use crate::lex;
use crate::types::{self, Cardinality, DialectVersion, StatementDescriptor, StatementKind, Type};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", bound(deserialize = "D: DeserializeOwned"))]
pub enum Request<D> {
    /// Round-trip check: answers with the engine version, its fingerprint and its database.
    Ping,
    /// Placeholders only; no database.
    Lex { sql: String, dialect: Dialect },
    /// The project file's text, `.reylo.yaml`.
    Config { text: String },
    /// The whole analysis, given what the host has answered so far.
    Analyze {
        sql: String,
        dialect: Dialect,
        #[serde(default)]
        answers: Answers<D>,
    },
}

/// What a database's `describe` need is answered with ([`Database::Describe`]).
pub trait Describe: DeserializeOwned + Clone + std::fmt::Debug + Send + Sync + 'static {}
impl<T: DeserializeOwned + Clone + std::fmt::Debug + Send + Sync + 'static> Describe for T {}

/// What the host learned for one need, keyed by the need's id. The describe payload is the
/// database's own: Postgres's Parse/Describe, MySQL's prepare metadata, nothing for SQLite.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Answer<D> {
    /// `{"describe": …}`. A MySQL host writes `{"mysql": …}`, which reads the same.
    Describe {
        #[serde(alias = "mysql")]
        describe: D,
    },
    Rows { rows: Vec<Vec<Value>> },
    Error { error: DbError },
}

/// The answers of a request, by need id.
pub type Answers<D> = HashMap<String, Answer<D>>;

/// The payload of a database that never asks for a describe: SQLite prepares inside the engine.
/// No describe answer reads as one.
#[derive(Debug, Clone, Deserialize)]
pub enum NoDescribe {}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DbError {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The 1-based character position the server reported, in the text it was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Need {
    /// Prepare `sql` and report its parameter types and result columns.
    Describe { id: String, sql: String },
    /// Run `sql` with `params` and hand back the rows as arrays of values.
    Query { id: String, sql: String, params: Vec<Value>, scope: Scope },
}

/// How long an answer stays true: a session answer is about the database as a whole and the
/// host may keep it across statements; a statement one is about this statement's relations.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Session,
    Statement,
}

/// A diagnostic as Reylo's own tools read it: its place in UTF-8 bytes and Reylo's code for it
/// ([`types::Diagnostic`] is the format's, in UTF-16 units and with the database's code).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Diagnostic {
    pub message: String,
    /// Byte offset in the original text.
    pub start: usize,
    pub length: usize,
    pub code: u32,
    /// The database's own code for the error the diagnostic reports (Postgres's SQLSTATE `42703`,
    /// MySQL's `1054`), where the database raised it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_code: Option<String>,
}

/// A statement's parameter: the format's ([`types::Param`]) and what Reylo's run time reads
/// beside it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    /// Whether the caller may pass null: the statement tests the parameter for null or
    /// coalesces it, or it is the whole value written to a nullable column.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nullable: bool,
    /// Whether the caller may leave the parameter out: a typed JSON parameter whose schema has a
    /// `default`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// The comment the database keeps on the column the parameter is written to (or, on MySQL,
    /// compared with), for the parameter's doc comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// A typed JSON parameter's JSON Schema as its cast writes it (`:p::jsonb('…')`), which the
    /// run time checks a value against; its type is the schema as the engine reads it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
    /// A `::type` written on MySQL or SQLite as the dialect's own cast, which turns a value it
    /// cannot read into 0, so the run time checks the parameter's value against its type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cast: Option<String>,
    /// What separates the elements of an array parameter's text, where the element type's
    /// `typdelim` is not a comma (`:` for PostGIS's `geometry`, `;` for `box`): the run time
    /// writes the array's text with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
    /// A structured parameter's members as its column list writes them, in the order of its
    /// record's fields ([`crate::structured`]), which the run time checks each value against.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<Member>,
}

/// A structured parameter's member as its column list writes it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Member {
    /// The type as the column list writes it (`varchar(32)`, `numeric(12,2)`), for the run time's
    /// messages and the modifiers it checks.
    pub written: String,
    /// A `json('…')` member's JSON Schema as written, which the run time checks a value against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
}

impl Param {
    /// A parameter of `ty`, with nothing else known of it.
    pub fn new(name: impl Into<String>, ty: Type) -> Param {
        Param { name: name.into(), ty, nullable: false, optional: false, comment: None, schema: None, cast: None, delimiter: None, members: Vec::new() }
    }

    /// The parameter as the format writes it.
    pub fn descriptor(&self) -> types::Param {
        types::Param { name: self.name.clone(), ty: self.ty.clone(), nullable: self.nullable, optional: self.optional, comment: self.comment.clone() }
    }
}

/// A result column: the format's ([`types::Column`]) and what Reylo's run time reads beside it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nullable: bool,
    /// The comment the database keeps on the table column the result column reads as it is
    /// (`COMMENT ON COLUMN`, MySQL's `COMMENT '…'`), for the field's doc comment; none for an
    /// expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// What separates the elements of an array column's text, where it is not a comma, which the
    /// run time reads the array's text with ([`Param::delimiter`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
}

impl Column {
    /// A column of `ty`, with nothing else known of it.
    pub fn new(name: impl Into<String>, ty: Type, nullable: bool) -> Column {
        Column { name: name.into(), ty, nullable, comment: None, delimiter: None }
    }

    /// The column as the format writes it.
    pub fn descriptor(&self) -> types::Column {
        types::Column { name: self.name.clone(), ty: self.ty.clone(), nullable: self.nullable, comment: self.comment.clone() }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    /// The statement's kind: one of the five, `other` for any other, absent when it did not parse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<StatementKind>,
    /// The prose written in comments before the statement ([`crate::comment::leading`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub params: Vec<Param>,
    pub columns: Vec<Column>,
    /// How many rows the statement returns.
    pub cardinality: Cardinality,
    /// Why the proof stopped short of `one`, when it did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cardinality_reason: Option<String>,
    /// The result key: output columns, by name, that identify each row of the result, since no two
    /// rows the statement returns agree on all of them (a NULL counting as equal to another NULL).
    /// Only for a result that can hold more than one row: `None` where the cardinality is `one` or
    /// `maybeOne`, which says as much, and where no key is proven, which is always sound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Vec<String>>,
    /// Whether the statement's ORDER BY orders its rows totally: no two rows of any result tie on
    /// every sort key, so the order is the same on every run. Only for a result that can hold more
    /// than one row, as `key` is.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ordered: bool,
    pub rewritten: lex::Rewritten,
    pub style: lex::Style,
    pub diagnostics: Vec<Diagnostic>,
    /// Paths inside JSON columns the engine could not type, for a message.
    pub unresolved: Vec<String>,
    /// Each structured parameter in insert position, with the lists of `rewritten` the run time
    /// writes again for a call that leaves keys out ([`crate::structured`]); on the wire only
    /// those that have lists (MySQL's), since a Postgres or SQLite statement copies its defaults.
    #[serde(skip_serializing_if = "crate::structured::none_rewritten", serialize_with = "crate::structured::rewritten_only")]
    pub structured: Vec<crate::structured::Info>,
    /// The facts the analysis relied on, which a deployed query checks against its database
    /// ([`crate::fresh`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fresh: Option<crate::fresh::Fresh>,
    /// The names each error the statement may raise can carry, where it runs only its database's
    /// own code ([`crate::errors`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<crate::errors::Errors>,
    /// Each domain a type of the statement is named by, with the name of the base type it is made
    /// from: a client decodes a domain's values as its base type's (a Postgres domain over
    /// pgvector's `vector` comes as the vector's text), which the format leaves to the catalog.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub domains: std::collections::BTreeMap<String, String>,
    /// The tables the statement reads and writes, as its database's own parse names them, which
    /// the facts are read for ([`Database::tables`]). Not on the wire.
    #[serde(skip)]
    pub tables: Option<crate::fresh::Tables>,
}

impl Analysis {
    /// The statement as the format describes it ([`StatementDescriptor`]): `sql` is the statement
    /// as it was written, which the diagnostics' offsets index, and `dialect` the database and
    /// version the analysis read.
    pub fn descriptor(&self, sql: &str, dialect: DialectVersion) -> StatementDescriptor {
        StatementDescriptor {
            dialect,
            sql: self.rewritten.sql.clone(),
            kind: self.kind,
            comment: self.comment.clone(),
            params: self.params.iter().map(Param::descriptor).collect(),
            columns: self.columns.iter().map(Column::descriptor).collect(),
            cardinality: self.cardinality,
            diagnostics: self.diagnostics.iter().map(|d| {
                let start = utf16_offset(sql, d.start);
                types::Diagnostic { message: d.message.clone(), start, end: utf16_offset(sql, d.start + d.length).max(start), code: d.server_code.clone() }
            }).collect(),
        }
    }
}

/// The UTF-16 offset of the UTF-8 byte offset `byte` of `text`, as the format counts: a byte
/// inside a character counts from the character's start, and one past the end at the end.
pub fn utf16_offset(text: &str, byte: usize) -> usize {
    let mut at = byte.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    text[..at].encode_utf16().count()
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Response {
    Ok {
        version: String,
        fingerprint: String,
        /// The database whose statements this artifact reads.
        #[serde(skip_serializing_if = "Option::is_none")]
        database: Option<Dialect>,
    },
    Lexed { lexed: lex::Lexed },
    Config { config: crate::config::Project },
    ConfigError { error: crate::config::ConfigError },
    Need { needs: Vec<Need> },
    Analysis { analysis: Analysis },
    Error { message: String },
}

/// What a request says before its answers are read: what a host that links several databases
/// dispatches on, and what lets [`call`] refuse another database's statement by name.
#[derive(Debug, Deserialize)]
struct Head {
    #[serde(default)]
    op: String,
    #[serde(default)]
    dialect: Option<Dialect>,
}

/// The dialect a request names, if it names one it can be read for.
pub fn dialect_of(request: &str) -> Option<Dialect> {
    serde_json::from_str::<Head>(request).ok()?.dialect
}

/// One request in, one response out, both JSON text, for the database `D`: the only entry point
/// of a database's artifact.
pub fn call<D: Database>(request: &str) -> String {
    crate::stack::run(|| call_here::<D>(request))
}

fn call_here<D: Database>(request: &str) -> String {
    let head = serde_json::from_str::<Head>(request).ok();
    let response = match head {
        Some(Head { op, dialect: Some(other) }) if op == "analyze" && other != D::DIALECT => {
            Response::Error { message: format!("this engine reads {} statements, not {} ones", D::DIALECT.name(), other.name()) }
        }
        _ => match serde_json::from_str::<Request<D::Describe>>(request) {
            Ok(Request::Ping) => Response::Ok { version: env!("CARGO_PKG_VERSION").to_string(), fingerprint: D::FINGERPRINT.to_string(), database: Some(D::DIALECT) },
            Ok(Request::Lex { sql, dialect }) => match lex::lex(&sql, dialect) {
                Ok(lexed) => Response::Lexed { lexed },
                Err(e) => Response::Error { message: e.message },
            },
            Ok(Request::Config { text }) => match crate::config::parse(&text) {
                Ok(config) => Response::Config { config },
                Err(error) => Response::ConfigError { error },
            },
            Ok(Request::Analyze { sql, mut answers, .. }) => {
                D::unbundle(&mut answers);
                match D::round(&sql, &answers) {
                    Response::Analysis { mut analysis } => {
                        analysis.comment = crate::comment::prose(&sql, D::DIALECT);
                        Response::Analysis { analysis }
                    }
                    other => other,
                }
            }
            Err(e) => Response::Error { message: format!("bad request: {e}") },
        },
    };
    serde_json::to_string(&response).expect("response serializes")
}
