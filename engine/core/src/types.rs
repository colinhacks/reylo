//! The type model every database's engine answers with. A scalar's name is in its database's
//! vocabulary: Postgres's catalog names, which MySQL maps its protocol types into, and SQLite's
//! storage classes.

use serde::{Deserialize, Serialize};

/// What the mapper renders as a TypeScript type and as JSON Schema. Inside an `Object` or a
/// `List` every value is JSON-encoded, so the same `Scalar` reads differently there (a
/// `timestamptz` is a `Date` on the wire and a string inside JSON); the renderer keeps track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Type {
    /// A base type (a domain resolves to its base) by its catalog name: `int4`, `text`, `uuid`.
    Scalar { name: String },
    /// An enum: a union of its labels.
    Enum { name: String, labels: Vec<String> },
    /// A Postgres array of the element type. Any array may hold NULL elements, whatever a
    /// column's `NOT NULL`, so a result's array renders `(T | null)[]` unless the statement
    /// builds it from values it shows are not NULL; a parameter's renders `T[]`. Only an array the
    /// statement builds says more (`reylo_engine_postgres::types::DescribedColumn::nullable_items`):
    /// `Some(true)` when an element may be NULL, as `array_agg(u.email)` over a LEFT JOIN gives
    /// `{NULL}` for a team with no users; `Some(false)` when every element is non-null.
    Array {
        of: Box<Type>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nullable_items: Option<bool>,
    },
    /// `json[b]_build_object(…)`: the keys in the order the SQL writes them.
    Object { fields: Vec<Field> },
    /// A named composite (a table's row type, a `CREATE TYPE … AS (…)`) whose columns the
    /// catalog gave: each field nullable unless the column is `NOT NULL`. Unlike an `Object`, its
    /// fields are read in the position the value itself stands in: a driver hands the whole value
    /// over as the text Postgres prints, and inside a JSON document the database writes it as an
    /// object of its fields. A composite whose columns were not read stays `Scalar { "record" }`.
    Composite { name: String, fields: Vec<Field> },
    /// `json[b]_agg(…)`: a JSON array of the item type.
    List { of: Box<Type>, nullable_items: bool },
    /// A `json`/`jsonb` value: any JSON, or, where a CHECK holds it to a JSON Schema the
    /// database validates, the values that schema admits, as the canonical schema of
    /// [`crate::schema_type`].
    Json {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema: Option<serde_json::Value>,
    },
    /// A structured parameter, `:events::rows(…)` or `:user::row(…)`: one object (`one`) or an
    /// array of objects, whose keys the parameter's column list names ([`crate::structured`]).
    Rows { one: bool, fields: Vec<RowField> },
    /// Nothing the engine derived.
    Unknown,
}

/// One key of a structured parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    /// The type as the column list writes it, whose modifiers (`varchar(32)`, `numeric(12,2)`)
    /// say what the run time checks beside the type itself.
    pub written: String,
    pub nullable: bool,
    /// Whether the caller may leave the key out: a `?` member, or one that may be null.
    pub optional: bool,
    /// A `json('…')` member's JSON Schema as written, which the run time checks a value against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub nullable: bool,
}
