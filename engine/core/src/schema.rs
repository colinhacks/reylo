//! JSON Schema as a rendering of the type model: what a value of a [`Type`] looks like once it is
//! JSON. The spellings are the framework's encoding table (`framework/docs`, the endpoints
//! page), whose formats tell a client which decoder a value needs, so each one is stated even
//! where JSON has a native type.
//!
//! One type reads two ways, by where the value stands. A value the caller decodes itself (a
//! result column, a parameter) takes the caller's representation: `int8` as a digit string
//! unless [`SchemaOptions::int8_as_number`], `numeric` as a string, `bytea` as base64. A value
//! the database already serialized into a JSON document ([`SchemaOptions::in_json`], and every
//! field of an `Object` or item of a `List`) takes the database's: `int8` and `numeric` as JSON
//! numbers, `bytea` as its own `\x…` hex.

use crate::types::{Field, Type};
use serde_json::{json, Map, Value};

/// Where a value stands and how the caller encodes what it decodes itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SchemaOptions {
    /// `int8` as a JSON number rather than a digit string where the caller decodes it (the
    /// framework's `encoding.int8: number`). Inside a JSON document it is a number either way.
    pub int8_as_number: bool,
    /// The value is inside a JSON document the database wrote.
    pub in_json: bool,
    /// The value is one the caller passes (a parameter), not one the database returns: an
    /// array's elements are then non-null unless the statement says otherwise, where a result's
    /// may be null unless the statement shows they are not.
    pub input: bool,
}

impl SchemaOptions {
    /// A value inside a JSON document, where `int8_as_number` has nothing to decide.
    pub const IN_JSON: SchemaOptions = SchemaOptions { int8_as_number: false, in_json: true, input: false };

    /// A value the caller decodes itself: a result column, a parameter.
    pub fn wire(int8_as_number: bool) -> SchemaOptions {
        SchemaOptions { int8_as_number, in_json: false, input: false }
    }

    /// The same options for a value the caller passes.
    pub fn as_input(self) -> SchemaOptions {
        SchemaOptions { input: true, ..self }
    }

    /// The same options for a value inside a JSON document.
    pub fn inside_json(self) -> SchemaOptions {
        SchemaOptions { in_json: true, ..self }
    }
}

/// The schema admitting null as well: a bare `type` becomes the two-member list, anything else
/// an `anyOf` with null; the empty schema already admits everything.
pub fn nullable(schema: Value) -> Value {
    match schema {
        Value::Object(map) if map.len() == 1 && map.contains_key("type") => match &map["type"] {
            Value::String(t) => json!({"type": [t, "null"]}),
            _ => json!({"anyOf": [Value::Object(map), {"type": "null"}]}),
        },
        Value::Object(map) if map.is_empty() => Value::Object(map),
        other => json!({"anyOf": [other, {"type": "null"}]}),
    }
}

/// A structured parameter's member: an array there keeps NULL elements, which the run time binds,
/// so an array a statement built with NULL elements goes back into a row; a top-level array
/// parameter's elements stay strict.
fn member_schema(ty: &Type, opts: SchemaOptions) -> Value {
    match ty {
        Type::Array { of, .. } if opts.input => json!({"type": "array", "items": nullable(member_schema(of, opts))}),
        _ => ty.json_schema(opts),
    }
}

impl Type {
    /// The JSON Schema of a value of this type, standing where `opts` says.
    pub fn json_schema(&self, opts: SchemaOptions) -> Value {
        match self {
            Type::Scalar { name } => scalar(name, opts),
            Type::Enum { labels, .. } => json!({"type": "string", "enum": labels}),
            // Postgres lets any array hold NULL elements, whatever a column's NOT NULL says.
            Type::Array { of, nullable_items } => {
                let items = of.json_schema(opts);
                let nulls = if opts.input { *nullable_items == Some(true) } else { *nullable_items != Some(false) };
                json!({"type": "array", "items": if nulls { nullable(items) } else { items }})
            }
            // A composite's columns stand where the row does, and a column the catalog does not
            // mark `NOT NULL` is always spelled as an `anyOf` with null.
            Type::Composite { fields, .. } => object(fields, |f| {
                let schema = f.ty.json_schema(opts);
                if f.nullable {
                    json!({"anyOf": [schema, {"type": "null"}]})
                } else {
                    schema
                }
            }),
            // A constructor's keys and items are inside the document it builds.
            Type::Object { fields } => object(fields, |f| {
                let schema = f.ty.json_schema(opts.inside_json());
                if f.nullable {
                    nullable(schema)
                } else {
                    schema
                }
            }),
            Type::List { of, nullable_items } => {
                let items = of.json_schema(opts.inside_json());
                json!({"type": "array", "items": if *nullable_items { nullable(items) } else { items }})
            }
            // The values a CHECK's JSON Schema admits, as the engine reduced it; else any JSON,
            // null included.
            Type::Json { schema: Some(schema) } => schema.clone(),
            Type::Json { schema: None } | Type::Unknown => json!({}),
            // What the caller passes: each key's value as it passes a parameter of that type.
            Type::Rows { one, fields } => {
                let mut properties = Map::new();
                let mut required = Vec::new();
                for f in fields {
                    let schema = member_schema(&f.ty, opts);
                    properties.insert(f.name.clone(), if f.nullable { nullable(schema) } else { schema });
                    if !f.optional {
                        required.push(json!(f.name));
                    }
                }
                let row = json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false});
                if *one {
                    row
                } else {
                    json!({"type": "array", "items": row})
                }
            }
        }
    }
}

/// An object of `fields`, every key required, each spelled by `field`.
fn object(fields: &[Field], field: impl Fn(&Field) -> Value) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::with_capacity(fields.len());
    for f in fields {
        properties.insert(f.name.clone(), field(f));
        required.push(json!(f.name));
    }
    json!({"type": "object", "properties": properties, "required": required})
}

/// A base type by its catalog name. A type the table does not spell is a plain string, which is
/// how Postgres prints it, named in the description.
fn scalar(name: &str, opts: SchemaOptions) -> Value {
    // `a|b` is either (the MySQL analysis: a column whose type the execution decides).
    if name.contains('|') {
        return json!({"anyOf": name.split('|').map(|part| scalar(part, opts)).collect::<Vec<_>>()});
    }
    match name {
        "bool" => json!({"type": "boolean"}),
        "int2" | "int4" => json!({"type": "integer", "format": "int32"}),
        "int8" if opts.in_json || opts.int8_as_number => json!({"type": "integer", "format": "int64"}),
        "int8" => json!({"type": "string", "format": "int64"}),
        "float4" => json!({"type": "number", "format": "float"}),
        "float8" => json!({"type": "number", "format": "double"}),
        "numeric" if opts.in_json => json!({"type": "number", "format": "decimal"}),
        "numeric" => json!({"type": "string", "format": "decimal"}),
        "text" | "varchar" | "bpchar" | "name" => json!({"type": "string"}),
        "uuid" => json!({"type": "string", "format": "uuid"}),
        "timestamptz" | "timestamp" => json!({"type": "string", "format": "date-time"}),
        "date" => json!({"type": "string", "format": "date"}),
        "time" => json!({"type": "string", "format": "time"}),
        // Postgres writes its own hex inside a document.
        "bytea" if opts.in_json => json!({"type": "string", "format": "hex"}),
        // `format: byte` is the table's word; `contentEncoding` is the JSON Schema 2020-12
        // spelling of the same base64, kept beside it for validators.
        "bytea" => json!({"type": "string", "format": "byte", "contentEncoding": "base64"}),
        // `unknown`: an expression column nothing could type (SQLite's, through the framework's
        // registry), so any JSON.
        "json" | "jsonb" | "unknown" => json!({}),
        _ => json!({"type": "string", "description": format!("pg type {name}")}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIRE: SchemaOptions = SchemaOptions { int8_as_number: false, in_json: false, input: false };
    const INT8N: SchemaOptions = SchemaOptions { int8_as_number: true, in_json: false, input: false };
    const IN_JSON: SchemaOptions = SchemaOptions::IN_JSON;

    fn s(name: &str) -> Type {
        Type::Scalar { name: name.into() }
    }

    fn field(name: &str, ty: Type, nullable: bool) -> Field {
        Field { name: name.into(), ty, nullable }
    }

    #[test]
    fn scalars_follow_the_encoding_table() {
        for (name, wire) in [
            ("bool", json!({"type": "boolean"})),
            ("int2", json!({"type": "integer", "format": "int32"})),
            ("int4", json!({"type": "integer", "format": "int32"})),
            ("int8", json!({"type": "string", "format": "int64"})),
            ("float4", json!({"type": "number", "format": "float"})),
            ("float8", json!({"type": "number", "format": "double"})),
            ("numeric", json!({"type": "string", "format": "decimal"})),
            ("text", json!({"type": "string"})),
            ("varchar", json!({"type": "string"})),
            ("bpchar", json!({"type": "string"})),
            ("name", json!({"type": "string"})),
            ("uuid", json!({"type": "string", "format": "uuid"})),
            ("timestamptz", json!({"type": "string", "format": "date-time"})),
            ("timestamp", json!({"type": "string", "format": "date-time"})),
            ("date", json!({"type": "string", "format": "date"})),
            ("time", json!({"type": "string", "format": "time"})),
            ("bytea", json!({"type": "string", "format": "byte", "contentEncoding": "base64"})),
            ("unknown", json!({})),
            ("inet", json!({"type": "string", "description": "pg type inet"})),
            ("oid:90099", json!({"type": "string", "description": "pg type oid:90099"})),
        ] {
            assert_eq!(s(name).json_schema(WIRE), wire, "{name}");
        }
        assert_eq!(s("int4|float8").json_schema(WIRE), json!({"anyOf": [{"type": "integer", "format": "int32"}, {"type": "number", "format": "double"}]}));
    }

    #[test]
    fn int8_as_number_changes_only_int8_where_the_caller_decodes() {
        assert_eq!(s("int8").json_schema(INT8N), json!({"type": "integer", "format": "int64"}));
        assert_eq!(s("numeric").json_schema(INT8N), json!({"type": "string", "format": "decimal"}));
        assert_eq!(s("bytea").json_schema(INT8N), s("bytea").json_schema(WIRE));
    }

    #[test]
    fn inside_json_the_database_spells_int8_numeric_and_bytea() {
        assert_eq!(s("int8").json_schema(IN_JSON), json!({"type": "integer", "format": "int64"}));
        assert_eq!(s("numeric").json_schema(IN_JSON), json!({"type": "number", "format": "decimal"}));
        assert_eq!(s("bytea").json_schema(IN_JSON), json!({"type": "string", "format": "hex"}));
        assert_eq!(s("uuid").json_schema(IN_JSON), s("uuid").json_schema(WIRE));
        assert_eq!(s("timestamptz").json_schema(IN_JSON), s("timestamptz").json_schema(WIRE));
    }

    #[test]
    fn enums_arrays_json_and_unknown() {
        let mood = Type::Enum { name: "mood".into(), labels: vec!["ok".into(), "sad".into()] };
        assert_eq!(mood.json_schema(WIRE), json!({"type": "string", "enum": ["ok", "sad"]}));
        // A result's elements may be NULL, as any Postgres array's may, unless the statement shows
        // they are not; a parameter's are strict unless the statement says otherwise.
        assert_eq!(Type::Array { of: Box::new(s("int4")), nullable_items: None }.json_schema(WIRE), json!({"type": "array", "items": {"anyOf": [{"type": "integer", "format": "int32"}, {"type": "null"}]}}));
        assert_eq!(Type::Array { of: Box::new(s("int4")), nullable_items: None }.json_schema(WIRE.as_input()), json!({"type": "array", "items": {"type": "integer", "format": "int32"}}));
        // An array's element stands where the array does.
        assert_eq!(Type::Array { of: Box::new(s("int8")), nullable_items: Some(false) }.json_schema(IN_JSON)["items"], json!({"type": "integer", "format": "int64"}));
        assert_eq!(Type::Array { of: Box::new(s("int8")), nullable_items: Some(false) }.json_schema(WIRE)["items"], json!({"type": "string", "format": "int64"}));
        assert_eq!(Type::Array { of: Box::new(s("text")), nullable_items: Some(true) }.json_schema(WIRE), json!({"type": "array", "items": {"type": ["string", "null"]}}));
        assert_eq!(Type::Array { of: Box::new(s("text")), nullable_items: Some(true) }.json_schema(WIRE.as_input()), json!({"type": "array", "items": {"type": ["string", "null"]}}));
        assert_eq!(Type::Array { of: Box::new(s("text")), nullable_items: Some(false) }.json_schema(WIRE), json!({"type": "array", "items": {"type": "string"}}));
        // A structured parameter's member keeps its array's NULL elements, which the run time binds.
        let texts = Type::Array { of: Box::new(s("text")), nullable_items: None };
        let member = Type::Rows { one: true, fields: vec![crate::types::RowField { name: "tags".into(), ty: texts, written: "text[]".into(), nullable: false, optional: false, schema: None }] };
        assert_eq!(member.json_schema(WIRE.as_input())["properties"]["tags"], json!({"type": "array", "items": {"type": ["string", "null"]}}));
        assert_eq!(Type::Json { schema: None }.json_schema(WIRE), json!({}));
        let checked = json!({"type": "object", "properties": {"a": {"type": "string"}}, "additionalProperties": true});
        assert_eq!(Type::Json { schema: Some(checked.clone()) }.json_schema(WIRE), checked);
        assert_eq!(Type::Unknown.json_schema(IN_JSON), json!({}));
    }

    #[test]
    fn a_composites_columns_stand_where_the_row_does() {
        let users = Type::Composite { name: "users".into(), fields: vec![field("id", s("int8"), false), field("name", s("text"), true), field("meta", Type::Json { schema: None }, true)] };
        assert_eq!(
            users.json_schema(WIRE),
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "format": "int64"},
                    "name": {"anyOf": [{"type": "string"}, {"type": "null"}]},
                    "meta": {"anyOf": [{}, {"type": "null"}]},
                },
                "required": ["id", "name", "meta"],
            })
        );
        assert_eq!(users.json_schema(IN_JSON)["properties"]["id"], json!({"type": "integer", "format": "int64"}));
        assert_eq!(users.json_schema(INT8N)["properties"]["id"], json!({"type": "integer", "format": "int64"}));
        let many = Type::Array { of: Box::new(users.clone()), nullable_items: None };
        assert_eq!(many.json_schema(WIRE), json!({"type": "array", "items": nullable(users.json_schema(WIRE))}));
        assert_eq!(many.json_schema(WIRE.as_input()), json!({"type": "array", "items": users.json_schema(WIRE)}));
    }

    #[test]
    fn a_constructors_keys_and_items_are_inside_the_document() {
        let post = Type::Object { fields: vec![field("id", s("int8"), false), field("title", s("text"), true), field("body", Type::Unknown, true), field("blob", s("bytea"), false)] };
        let expected = json!({
            "type": "object",
            "properties": {
                "id": {"type": "integer", "format": "int64"},
                "title": {"type": ["string", "null"]},
                "body": {},
                "blob": {"type": "string", "format": "hex"},
            },
            "required": ["id", "title", "body", "blob"],
        });
        // The same at the top level and nested, and whatever `int8_as_number` says.
        assert_eq!(post.json_schema(WIRE), expected);
        assert_eq!(post.json_schema(INT8N), expected);
        assert_eq!(post.json_schema(IN_JSON), expected);
        let list = Type::List { of: Box::new(s("numeric")), nullable_items: true };
        assert_eq!(list.json_schema(WIRE), json!({"type": "array", "items": {"anyOf": [{"type": "number", "format": "decimal"}, {"type": "null"}]}}));
        let texts = Type::List { of: Box::new(s("text")), nullable_items: true };
        assert_eq!(texts.json_schema(WIRE), json!({"type": "array", "items": {"type": ["string", "null"]}}));
        let posts = Type::List { of: Box::new(post), nullable_items: false };
        assert_eq!(posts.json_schema(WIRE), json!({"type": "array", "items": expected}));
        assert_eq!(Type::Object { fields: Vec::new() }.json_schema(WIRE), json!({"type": "object", "properties": {}, "required": []}));
    }

    #[test]
    fn nullable_widens_a_bare_type_and_wraps_the_rest() {
        assert_eq!(nullable(json!({"type": "string"})), json!({"type": ["string", "null"]}));
        assert_eq!(nullable(json!({"type": "string", "format": "uuid"})), json!({"anyOf": [{"type": "string", "format": "uuid"}, {"type": "null"}]}));
        assert_eq!(nullable(json!({"type": ["string", "null"]})), json!({"anyOf": [{"type": ["string", "null"]}, {"type": "null"}]}));
        assert_eq!(nullable(json!({})), json!({}));
        assert_eq!(nullable(json!({"$ref": "#/components/schemas/X"})), json!({"anyOf": [{"$ref": "#/components/schemas/X"}, {"type": "null"}]}));
    }
}
