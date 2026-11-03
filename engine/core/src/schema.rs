//! JSON Schema as a rendering of the type model: what a value of a [`Type`] looks like once it is
//! JSON. A spelling's format tells a client which decoder a value needs, so each one is stated
//! even where JSON has a native type.
//!
//! One type reads two ways, by where the value stands. A value the caller decodes itself (a
//! result column, a parameter) takes the caller's representation: `int8` as a digit string
//! unless [`SchemaOptions::int8_as_number`], `numeric` as a string, `bytea` as base64. A value
//! the database already serialized into a JSON document ([`SchemaOptions::in_json`], and every
//! field of an `Object` or item of a `List`) takes the database's: `int8` and `numeric` as JSON
//! numbers, `bytea` as its own `\x…` hex.

use crate::types::{Field, JsonType, Kind, RecordType, Type};
use serde_json::{json, Map, Value};

/// Where a value stands and how the caller encodes what it decodes itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SchemaOptions {
    /// `int8` as a JSON number rather than a digit string where the caller decodes it (a
    /// host's `encoding.int8: number`). Inside a JSON document it is a number either way.
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
    match &ty.kind {
        Kind::Array(a) if opts.input => json!({"type": "array", "items": nullable(member_schema(&a.element.ty, opts))}),
        _ => ty.json_schema(opts),
    }
}

/// The type's name without the schema that qualifies it: `hstore` of `public.hstore`.
fn bare_name(ty: &Type) -> &str {
    ty.name.as_deref().map(|n| n.rsplit('.').next().unwrap_or(n)).unwrap_or("")
}

/// A value the database writes as its text, where its own JSON form is a string too.
fn text_of(ty: &Type) -> Value {
    json!({"type": "string", "description": format!("pg type {}", ty.name.as_deref().unwrap_or(ty.kind.name()))})
}

/// The values JSON writes as text when a float or a decimal holds them.
fn non_finite(infinity: bool, nan: bool) -> Vec<&'static str> {
    let mut out = Vec::new();
    if nan {
        out.push("NaN");
    }
    if infinity {
        out.extend(["Infinity", "-Infinity"]);
    }
    out
}

impl Type {
    /// The JSON Schema of a value of this type, standing where `opts` says.
    pub fn json_schema(&self, opts: SchemaOptions) -> Value {
        match &self.kind {
            Kind::Boolean => json!({"type": "boolean"}),
            // An oid is not one of the numbers JSON has a number for: Postgres writes its text.
            Kind::Integer(_) if bare_name(self) == "oid" => text_of(self),
            Kind::Integer(i) if i.bits <= 32 && !(i.bits == 32 && i.unsigned) => json!({"type": "integer", "format": "int32"}),
            Kind::Integer(_) if opts.in_json || opts.int8_as_number => json!({"type": "integer", "format": "int64"}),
            Kind::Integer(_) => json!({"type": "string", "format": "int64"}),
            Kind::Float(f) => {
                let number = json!({"type": "number", "format": if f.exponent_bits <= 8 { "float" } else { "double" }});
                // JSON has no number for a non-finite value, so Postgres writes one inside a
                // document as text.
                let text = non_finite(f.infinity, f.nan);
                if opts.in_json && !text.is_empty() { json!({"anyOf": [number, {"enum": text}]}) } else { number }
            }
            Kind::Decimal(d) if opts.in_json => {
                let number = json!({"type": "number", "format": "decimal"});
                let text = non_finite(d.infinity, d.nan);
                if text.is_empty() { number } else { json!({"anyOf": [number, {"enum": text}]}) }
            }
            Kind::Decimal(_) => json!({"type": "string", "format": "decimal"}),
            Kind::Text(t) => match &t.values {
                Some(values) => json!({"type": "string", "enum": values}),
                None => json!({"type": "string"}),
            },
            Kind::Uuid => json!({"type": "string", "format": "uuid"}),
            Kind::DateTime(_) | Kind::Instant(_) => json!({"type": "string", "format": "date-time"}),
            Kind::Date(_) => json!({"type": "string", "format": "date"}),
            Kind::Time(_) => json!({"type": "string", "format": "time"}),
            Kind::Bytes(_) if matches!(bare_name(self), "macaddr" | "macaddr8") => text_of(self),
            // Postgres writes its own hex inside a document.
            Kind::Bytes(_) if opts.in_json => json!({"type": "string", "format": "hex"}),
            // `format: byte` is the table's word; `contentEncoding` is the JSON Schema 2020-12
            // spelling of the same base64, kept beside it for validators.
            Kind::Bytes(_) => json!({"type": "string", "format": "byte", "contentEncoding": "base64"}),
            // A bit string and pgvector's vectors are their text to a client.
            Kind::Array(_) if matches!(bare_name(self), "bit" | "varbit" | "vector" | "halfvec" | "sparsevec") => text_of(self),
            // Postgres has no cast of a geometric type to JSON, so a document holds its text.
            Kind::Array(_) | Kind::Record(_) if opts.in_json && matches!(bare_name(self), "point" | "line" | "lseg" | "box" | "path" | "polygon" | "circle") => text_of(self),
            Kind::Array(a) => {
                let items = a.element.ty.json_schema(opts);
                // A parameter's elements are strict; a result's may be NULL unless the statement
                // shows they are not.
                let nulls = a.element.nullable && !opts.input;
                json!({"type": "array", "items": if nulls { nullable(items) } else { items }})
            }
            // A row whose fields have no names, DuckDB's tuple, as its values in order.
            Kind::Record(r) if !r.fields.is_empty() && r.fields.iter().all(|f| f.name.is_empty()) => {
                json!({"type": "array", "prefixItems": r.fields.iter().map(|f| nullable(f.ty.json_schema(opts))).collect::<Vec<_>>(), "items": false, "minItems": r.fields.len()})
            }
            // A row's fields stand where the row does, and a field that may be null is always
            // spelled as an `anyOf` with null.
            Kind::Record(r) => object(&r.fields, |f| {
                let schema = f.ty.json_schema(opts);
                if f.nullable { json!({"anyOf": [schema, {"type": "null"}]}) } else { schema }
            }),
            // What a statement built: its keys and items are inside the document.
            Kind::Json(JsonType { content: Some(content), .. }) => {
                let schema = content.ty.json_schema(opts.inside_json());
                if content.nullable { nullable(schema) } else { schema }
            }
            // The values a CHECK's JSON Schema admits, as the engine reduced it; else any JSON,
            // null included.
            Kind::Json(JsonType { schema: Some(_), .. }) => self.schema().unwrap_or(Value::Bool(true)),
            Kind::Json(_) | Kind::Unknown | Kind::Any => json!({}),
            // An hstore is an object of its keys inside a document, each value text or null.
            Kind::Map(_) if bare_name(self) == "hstore" && opts.in_json => json!({"type": "object", "additionalProperties": {"type": ["string", "null"]}}),
            Kind::Map(_) if bare_name(self) == "hstore" => text_of(self),
            // A DuckDB map as its entries, each a key and its value, in order.
            Kind::Map(m) => {
                let value = m.value.ty.json_schema(opts);
                let entry = json!({"type": "object", "properties": {"key": m.key.json_schema(opts), "value": if m.value.nullable { nullable(value) } else { value }}, "required": ["key", "value"]});
                json!({"type": "array", "items": entry})
            }
            // A tagged union as a client returns one: its tag and its value, `{ tag, value }`.
            Kind::TaggedUnion(t) => json!({"anyOf": t.alternatives.iter().map(|a| {
                let value = a.ty.json_schema(opts);
                json!({"type": "object", "properties": {"tag": {"const": a.tag}, "value": if a.nullable { nullable(value) } else { value }}, "required": ["tag", "value"]})
            }).collect::<Vec<_>>()}),
            Kind::Union(_) if matches!(bare_name(self), "inet" | "cidr") => text_of(self),
            // One of several types, the execution's or each value's.
            Kind::Union(u) => json!({"anyOf": u.members.iter().map(|m| m.json_schema(opts)).collect::<Vec<_>>()}),
            // A row whose columns were not read (a subquery's) is an object of them inside a document.
            Kind::Opaque if bare_name(self) == "record" && opts.in_json => json!({"type": "object"}),
            // PostGIS writes a geometry inside a document through its cast to JSON, as GeoJSON.
            Kind::Geometry(_) if bare_name(self) == "geometry" && opts.in_json => {
                json!({"type": "object", "properties": {"type": {"type": "string"}}, "required": ["type"], "description": "a GeoJSON geometry"})
            }
            // Every other type is a plain string, which is how Postgres prints it, named in the
            // description.
            _ => text_of(self),
        }
    }

    /// The JSON Schema of what the caller passes for a structured parameter of this type (a
    /// record, or an array of them): each key's value as it passes a parameter of that type.
    pub fn rows_schema(&self, opts: SchemaOptions) -> Value {
        let row = |r: &RecordType| {
            let mut properties = Map::new();
            let mut required = Vec::new();
            for f in &r.fields {
                let schema = member_schema(&f.ty, opts);
                properties.insert(f.name.clone(), if f.nullable { nullable(schema) } else { schema });
                if !f.optional {
                    required.push(json!(f.name));
                }
            }
            json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
        };
        match &self.kind {
            Kind::Record(r) => row(r),
            Kind::Array(a) => match &a.element.ty.kind {
                Kind::Record(r) => json!({"type": "array", "items": row(r)}),
                _ => self.json_schema(opts),
            },
            _ => self.json_schema(opts),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DecimalType, FloatType, Position};

    const WIRE: SchemaOptions = SchemaOptions { int8_as_number: false, in_json: false, input: false };
    const INT8N: SchemaOptions = SchemaOptions { int8_as_number: true, in_json: false, input: false };
    const IN_JSON: SchemaOptions = SchemaOptions::IN_JSON;

    fn numeric() -> Type {
        Type::decimal(DecimalType { infinity: true, nan: true, ..DecimalType::default() }).named("numeric")
    }

    fn float8() -> Type {
        Type::new(Kind::Float(FloatType { exponent_bits: 11, mantissa_bits: 52, infinity: true, nan: true, ..FloatType::default() })).named("float8")
    }

    fn field(name: &str, ty: Type, nullable: bool) -> Field {
        Field::new(name, ty, nullable)
    }

    #[test]
    fn each_kind_follows_the_encoding_table() {
        for (ty, wire) in [
            (Type::boolean(), json!({"type": "boolean"})),
            (Type::integer(16), json!({"type": "integer", "format": "int32"})),
            (Type::integer(32), json!({"type": "integer", "format": "int32"})),
            (Type::integer(64), json!({"type": "string", "format": "int64"})),
            (Type::single(), json!({"type": "number", "format": "float"})),
            (float8(), json!({"type": "number", "format": "double"})),
            (numeric(), json!({"type": "string", "format": "decimal"})),
            (Type::text().named("varchar"), json!({"type": "string"})),
            (Type::uuid(), json!({"type": "string", "format": "uuid"})),
            (Type::bytes().named("bytea"), json!({"type": "string", "format": "byte", "contentEncoding": "base64"})),
            (Type::unknown(), json!({})),
            (Type::opaque("ltree"), json!({"type": "string", "description": "pg type ltree"})),
        ] {
            assert_eq!(ty.json_schema(WIRE), wire, "{ty:?}");
        }
        let either = Type::union(vec![Type::integer(32), Type::double()], crate::types::Per::Execution);
        assert_eq!(either.json_schema(WIRE), json!({"anyOf": [{"type": "integer", "format": "int32"}, {"type": "number", "format": "double"}]}));
    }

    #[test]
    fn int8_as_number_changes_only_int8_where_the_caller_decodes() {
        assert_eq!(Type::integer(64).json_schema(INT8N), json!({"type": "integer", "format": "int64"}));
        assert_eq!(numeric().json_schema(INT8N), json!({"type": "string", "format": "decimal"}));
    }

    #[test]
    fn inside_json_the_database_spells_int8_numeric_and_bytea() {
        assert_eq!(Type::integer(64).json_schema(IN_JSON), json!({"type": "integer", "format": "int64"}));
        // A non-finite float or decimal is text inside a document: `{"f": "Infinity"}`.
        assert_eq!(numeric().json_schema(IN_JSON), json!({"anyOf": [{"type": "number", "format": "decimal"}, {"enum": ["NaN", "Infinity", "-Infinity"]}]}));
        assert_eq!(float8().json_schema(IN_JSON), json!({"anyOf": [{"type": "number", "format": "double"}, {"enum": ["NaN", "Infinity", "-Infinity"]}]}));
        // A decimal that holds no infinity, `numeric(10,2)`, writes only NaN as text.
        let sized = Type::decimal(DecimalType { precision: Some(10), min_scale: Some(2), max_scale: Some(2), nan: true, ..DecimalType::default() });
        assert_eq!(sized.json_schema(IN_JSON), json!({"anyOf": [{"type": "number", "format": "decimal"}, {"enum": ["NaN"]}]}));
        assert_eq!(Type::bytes().json_schema(IN_JSON), json!({"type": "string", "format": "hex"}));
        assert_eq!(Type::opaque("oid").json_schema(IN_JSON)["type"], json!("string"));
        assert_eq!(Type::unsigned(32).named("oid").json_schema(IN_JSON)["type"], json!("string"));
    }

    #[test]
    fn inside_json_an_hstore_is_an_object_of_nullable_text() {
        let hstore = Type::new(Kind::Map(crate::types::MapType { key: Box::new(Type::text()), value: Box::new(Position::nullable(Type::text())) })).named("public.hstore");
        assert_eq!(hstore.json_schema(IN_JSON), json!({"type": "object", "additionalProperties": {"type": ["string", "null"]}}));
        assert_eq!(hstore.json_schema(WIRE), json!({"type": "string", "description": "pg type public.hstore"}));
    }

    #[test]
    fn arrays_json_and_unknown() {
        let mood = Type::text_of(vec!["ok".into(), "sad".into()]).named("public.mood");
        assert_eq!(mood.json_schema(WIRE), json!({"type": "string", "enum": ["ok", "sad"]}));
        // A result's elements may be NULL, as any Postgres array's may, unless the statement shows
        // they are not; a parameter's are strict.
        let ints = Type::array(Position::nullable(Type::integer(32)));
        assert_eq!(ints.json_schema(WIRE), json!({"type": "array", "items": {"anyOf": [{"type": "integer", "format": "int32"}, {"type": "null"}]}}));
        assert_eq!(ints.json_schema(WIRE.as_input()), json!({"type": "array", "items": {"type": "integer", "format": "int32"}}));
        let strict = Type::array(Position::non_null(Type::integer(64)));
        assert_eq!(strict.json_schema(IN_JSON)["items"], json!({"type": "integer", "format": "int64"}));
        assert_eq!(strict.json_schema(WIRE)["items"], json!({"type": "string", "format": "int64"}));
        // A structured parameter's member keeps its array's NULL elements, which the run time binds.
        let texts = Type::array(Position::nullable(Type::text()));
        let member = Type::record(vec![field("tags", texts, false)]);
        assert_eq!(member.rows_schema(WIRE.as_input())["properties"]["tags"], json!({"type": "array", "items": {"type": ["string", "null"]}}));
        assert_eq!(Type::json().json_schema(WIRE), json!({}));
        let checked = json!({"type": "object", "properties": {"a": {"type": "string"}}, "additionalProperties": true});
        assert_eq!(Type::json_schema_of(checked.clone()).json_schema(WIRE), checked);
        assert_eq!(Type::unknown().json_schema(IN_JSON), json!({}));
    }

    #[test]
    fn a_rows_fields_stand_where_the_row_does() {
        let users = Type::record(vec![field("id", Type::integer(64), false), field("name", Type::text(), true), field("meta", Type::json(), true)]).named("public.users");
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
        let many = Type::array(Position::nullable(users.clone()));
        assert_eq!(many.json_schema(WIRE), json!({"type": "array", "items": nullable(users.json_schema(WIRE))}));
        assert_eq!(many.json_schema(WIRE.as_input()), json!({"type": "array", "items": users.json_schema(WIRE)}));
    }

    #[test]
    fn a_constructors_keys_and_items_are_inside_the_document() {
        let post = Type::record(vec![field("id", Type::integer(64), false), field("title", Type::text(), true), field("body", Type::unknown(), true), field("blob", Type::bytes(), false)]);
        let expected = json!({
            "type": "object",
            "properties": {
                "id": {"type": "integer", "format": "int64"},
                "title": {"anyOf": [{"type": "string"}, {"type": "null"}]},
                "body": {"anyOf": [{}, {"type": "null"}]},
                "blob": {"type": "string", "format": "hex"},
            },
            "required": ["id", "title", "body", "blob"],
        });
        // The same whatever `int8_as_number` says.
        let built = Type::json_of(Position::non_null(post.clone()));
        assert_eq!(built.json_schema(WIRE), expected);
        assert_eq!(built.json_schema(INT8N), expected);
        let list = Type::json_of(Position::non_null(Type::array(Position::nullable(numeric()))));
        assert_eq!(list.json_schema(WIRE), json!({"type": "array", "items": {"anyOf": [{"anyOf": [{"type": "number", "format": "decimal"}, {"enum": ["NaN", "Infinity", "-Infinity"]}]}, {"type": "null"}]}}));
        let posts = Type::json_of(Position::non_null(Type::array(Position::non_null(post))));
        assert_eq!(posts.json_schema(WIRE), json!({"type": "array", "items": expected}));
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
