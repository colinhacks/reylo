//! The type model every database's engine answers with: the type descriptor format of
//! `spec/type-descriptor.ts`, as Rust types that serialize to exactly its JSON. A [`Type`] says
//! which values a parameter or a column can hold, across databases: its `kind`, that kind's
//! fields, and the database's `name` for it; a [`Position`] says where a value stands and whether
//! it may be null; a [`StatementDescriptor`] is what introspecting one statement returns.
//!
//! The spec is the reference for every kind, field and rule; the comments here only say how the
//! Rust spells it. A flag is a `bool` written only when true, a value inside the format (a bound, a
//! tick, a duration's maximum) is a `String` in its kind's canonical text, and a count (`bits`, a
//! size, a precision, a scale) is a number. [`Type::check`] runs rule 7's validity checks.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

fn is_false(b: &bool) -> bool {
    !*b
}

/// A type: its kind, with that kind's fields, and what every type may carry, the database's name
/// for it and the comment the database keeps on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Type {
    #[serde(flatten)]
    pub kind: Kind,
    /// The database's name for the type, less the modifiers a field states (`numeric` for
    /// `numeric(10,2)`), qualified where a schema defines it; absent on a type the statement builds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `COMMENT ON TYPE` or `COMMENT ON DOMAIN`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// The kinds, in the order the spec's `Type` lists them, which is also the order a union lists
/// its members in ([`Kind::rank`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    Boolean,
    Integer(IntegerType),
    Float(FloatType),
    Decimal(DecimalType),
    Text(TextType),
    Bytes(BytesType),
    Uuid,
    Array(ArrayType),
    Record(RecordType),
    Json(JsonType),
    Date(DateType),
    Time(TimeType),
    #[serde(rename = "datetime")]
    DateTime(DateTimeType),
    Instant(InstantType),
    Duration(DurationType),
    Union(UnionType),
    Set(SetType),
    Map(MapType),
    Range(RangeType),
    Multirange(RangeType),
    Geometry(GeometryType),
    Xml(XmlType),
    /// A type the database knows and the format does not structure; its `name` is all it says.
    Opaque,
    /// A value of any type, each carrying its type: SQL Server `sql_variant`, Oracle `ANYDATA`.
    Any,
    /// Nothing is known about the values.
    Unknown,
    /// One of several alternatives, each named by the tag every value carries: DuckDB's `UNION`.
    /// Appended, as a new kind is, so the order of the kinds never changes.
    TaggedUnion(TaggedUnionType),
}

/// Bounds on an ordered kind's finite values, each a value in its kind's canonical text. A side
/// takes one of its two fields, and a whole-number kind only `min` and `max`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_min: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_max: Option<String>,
}

impl Bounds {
    pub fn is_empty(&self) -> bool {
        self.min.is_none() && self.max.is_none() && self.exclusive_min.is_none() && self.exclusive_max.is_none()
    }

    /// Inclusive bounds at `min` and `max`.
    pub fn between(min: impl Into<String>, max: impl Into<String>) -> Bounds {
        Bounds { min: Some(min.into()), max: Some(max.into()), ..Bounds::default() }
    }
}

/// How long a value is, in whole numbers of its kind's unit: `exact`, or `min`, `max` or both.
/// Only valid Unicode text names a `unit` (code points when absent), and only a byte size an
/// `encoding`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<SizeUnit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
}

impl Size {
    pub fn exact(n: u64) -> Size {
        Size { exact: Some(n), ..Size::default() }
    }

    pub fn at_most(n: u64) -> Size {
        Size { max: Some(n), ..Size::default() }
    }

    /// At most `n` bytes in `encoding`.
    pub fn bytes_at_most(n: u64, encoding: impl Into<String>) -> Size {
        Size { unit: Some(SizeUnit::Byte), encoding: Some(encoding.into()), max: Some(n), ..Size::default() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeUnit {
    Utf16Unit,
    Byte,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IntegerType {
    pub bits: u32,
    /// From 0 to 2^bits − 1.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unsigned: bool,
    #[serde(flatten)]
    pub bounds: Bounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiple_of: Option<String>,
    /// The only values, as JSON Schema's `enum`.
    #[serde(default, rename = "enum", skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
}

/// An IEEE 754 binary float by its exponent and mantissa widths (the mantissa's without its
/// implicit bit).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FloatType {
    pub exponent_bits: u32,
    pub mantissa_bits: u32,
    #[serde(flatten)]
    pub bounds: Bounds,
    #[serde(default, skip_serializing_if = "is_false")]
    pub infinity: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub nan: bool,
}

/// An exact decimal: a coefficient of at most `precision` digits times 10^-scale, the scale from
/// `min_scale` to `max_scale`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DecimalType {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_scale: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_scale: Option<i32>,
    #[serde(flatten)]
    pub bounds: Bounds,
    #[serde(default, skip_serializing_if = "is_false")]
    pub infinity: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub nan: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiple_of: Option<String>,
    #[serde(default, rename = "enum", skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
}

/// Text, in one of three shapes by `ill_formed`: valid Unicode (absent), bytes meant as UTF-8 and
/// never validated, or UTF-16 units that may hold a lone surrogate.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextType {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "ill_formed")]
    pub ill_formed: Option<IllFormed>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<Pattern>,
    #[serde(default, rename = "enum", skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IllFormed {
    Utf8,
    Utf16,
}

/// `ill_formed: false` is valid Unicode, as an absent one is.
fn ill_formed<'de, D: Deserializer<'de>>(d: D) -> Result<Option<IllFormed>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Written {
        Flag(bool),
        Shape(IllFormed),
    }
    match Option::<Written>::deserialize(d)? {
        None | Some(Written::Flag(false)) => Ok(None),
        Some(Written::Flag(true)) => Err(serde::de::Error::custom("ill_formed is false, \"utf8\" or \"utf16\"")),
        Some(Written::Shape(shape)) => Ok(Some(shape)),
    }
}

/// A pattern every value matches, in its own dialect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pattern {
    pub dialect: PatternDialect,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternDialect {
    Postgres,
    Mysql,
    Posix,
    Ecma,
    Like,
    Ilike,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BytesType {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
}

/// A sequence of `element`s; `any_depth` where an element may itself be an array of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrayType {
    pub element: Box<Position>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub any_depth: bool,
}

/// Fields, in order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RecordType {
    pub fields: Vec<Field>,
}

/// A JSON value: what the statement built (`content`), a JSON Schema every value satisfies
/// (`schema`), or neither, any JSON.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JsonType {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Box<Position>>,
    /// A JSON Schema every value satisfies, whose `$schema` names the draft it is read by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DateType {
    #[serde(flatten)]
    pub bounds: Bounds,
    #[serde(default, skip_serializing_if = "is_false")]
    pub infinity: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TimeType {
    /// The smallest step, in seconds, in lowest terms: `"1/1000000"`.
    pub tick: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<Zone>,
    #[serde(flatten)]
    pub bounds: Bounds,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DateTimeType {
    pub tick: String,
    #[serde(flatten)]
    pub bounds: Bounds,
    #[serde(default, skip_serializing_if = "is_false")]
    pub infinity: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InstantType {
    pub tick: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<Zone>,
    #[serde(flatten)]
    pub bounds: Bounds,
    #[serde(default, skip_serializing_if = "is_false")]
    pub infinity: bool,
}

/// What each value keeps besides the moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Zone {
    Offset,
    Region,
    RegionOrOffset,
}

/// A length of time in the parts it has, each signed on its own.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DurationType {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub months: Option<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<TimePart>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub infinity: bool,
}

/// A duration's `months` or `days`: its magnitude's bound, of either sign, as an ISO 8601 duration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Part {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimePart {
    pub tick: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
}

/// One of several types, each value one of them, or with `per: "execution"` every value of one
/// execution the same one. Flat, its members in kind order ([`Type::union`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnionType {
    pub members: Vec<Type>,
    /// Written only for `execution`: a union is per value unless it says so.
    #[serde(default, skip_serializing_if = "Per::is_value")]
    pub per: Per,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Per {
    #[default]
    Value,
    Execution,
}

impl Per {
    fn is_value(&self) -> bool {
        *self == Per::Value
    }
}

/// One of several alternatives, each named by the tag every value carries, in the order the type
/// declares them: two may hold one type, and a value says which it is, even holding NULL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaggedUnionType {
    pub alternatives: Vec<Alternative>,
}

/// An alternative of a tagged union: its tag, unique in the type, and the value it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alternative {
    pub tag: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "is_false")]
    pub nullable: bool,
}

impl Alternative {
    pub fn position(&self) -> Position {
        Position { ty: self.ty.clone(), nullable: self.nullable }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetType {
    pub element: Box<Position>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub duplicates: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapType {
    pub key: Box<Type>,
    pub value: Box<Position>,
}

/// A range, or a multirange, of an ordered `subtype`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RangeType {
    pub subtype: Box<Type>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GeometryType {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<GeometryModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub srid: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<GeometrySubtype>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Coordinates>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryModel {
    Planar,
    Geodetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometrySubtype {
    Point,
    Linestring,
    Polygon,
    Multipoint,
    Multilinestring,
    Multipolygon,
    Geometrycollection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coordinates {
    Xy,
    Xyz,
    Xym,
    Xyzm,
}

/// XML: a document, valid against any `schemas` given, or a `fragment`, which names none.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct XmlType {
    #[serde(default, skip_serializing_if = "is_false")]
    pub fragment: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schemas: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------------------------
// Positions
// ---------------------------------------------------------------------------------------------

/// Where a value stands: a record's field, an array's element, a map's value, or JSON content.
/// Nullability lives here, never in a type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    #[serde(rename = "type")]
    pub ty: Type,
    /// The value may be null (inside JSON content, JSON null).
    #[serde(default, skip_serializing_if = "is_false")]
    pub nullable: bool,
}

impl Position {
    pub fn new(ty: Type, nullable: bool) -> Position {
        Position { ty, nullable }
    }

    pub fn non_null(ty: Type) -> Position {
        Position { ty, nullable: false }
    }

    pub fn nullable(ty: Type) -> Position {
        Position { ty, nullable: true }
    }
}

/// A result's column: a row always has every column, and columns can share a name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "is_false")]
    pub nullable: bool,
    /// `COMMENT ON COLUMN`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// A record's field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "is_false")]
    pub nullable: bool,
    /// The key may be absent: a structured parameter's `?` member, a JSON object's key.
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
}

impl Field {
    pub fn new(name: impl Into<String>, ty: Type, nullable: bool) -> Field {
        Field { name: name.into(), ty, nullable, optional: false }
    }

    pub fn position(&self) -> Position {
        Position { ty: self.ty.clone(), nullable: self.nullable }
    }
}

/// A statement's parameter. Where the statement writes it whole into a column, its type is that
/// column's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    /// The statement accepts null.
    #[serde(default, skip_serializing_if = "is_false")]
    pub nullable: bool,
    /// The caller may leave the parameter out.
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    /// The comment the database keeps on the column the parameter is written to whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// The statement
// ---------------------------------------------------------------------------------------------

/// The URL of the JSON Schema of the format version this crate writes, which every
/// [`StatementDescriptor`] names in `$schema`: the `$id` of `spec/typedefs.schema.json`.
pub const SCHEMA: &str = "https://reylo.dev/typedefs/0.1.0.json";

/// What introspecting one statement returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatementDescriptor {
    /// The format version the descriptor is written in, as the URL of its JSON Schema ([`SCHEMA`]).
    #[serde(rename = "$schema")]
    pub schema: String,
    pub dialect: DialectVersion,
    /// The statement as the database runs it, with its own placeholders, in the order `params`
    /// lists them.
    pub sql: String,
    /// Absent when the statement does not parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<StatementKind>,
    /// The prose written in comments before the statement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub params: Vec<Param>,
    pub columns: Vec<Column>,
    pub cardinality: Cardinality,
    pub diagnostics: Vec<Diagnostic>,
}

/// The database a document of descriptors describes: its name and server version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialectVersion {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatementKind {
    Select,
    Insert,
    Update,
    Delete,
    Merge,
    Other,
}

impl StatementKind {
    /// The kind of a statement whose first keyword is `word`: one of the five, else `other`.
    pub fn of(word: &str) -> StatementKind {
        match word.to_ascii_lowercase().as_str() {
            "select" => StatementKind::Select,
            "insert" => StatementKind::Insert,
            "update" => StatementKind::Update,
            "delete" => StatementKind::Delete,
            "merge" => StatementKind::Merge,
            _ => StatementKind::Other,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            StatementKind::Select => "select",
            StatementKind::Insert => "insert",
            StatementKind::Update => "update",
            StatementKind::Delete => "delete",
            StatementKind::Merge => "merge",
            StatementKind::Other => "other",
        }
    }
}

/// How many rows the statement returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    One,
    AtMostOne,
    Many,
    None,
}

impl Cardinality {
    pub fn as_str(self) -> &'static str {
        match self {
            Cardinality::One => "one",
            Cardinality::AtMostOne => "at_most_one",
            Cardinality::Many => "many",
            Cardinality::None => "none",
        }
    }
}

/// A cardinality compares with its name as the format writes it.
impl PartialEq<&str> for Cardinality {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

/// What the database or the introspection reports against the statement: offsets into the
/// statement as written, in UTF-16 units, and the database's own error code where it has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub message: String,
    pub start: usize,
    pub end: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Building types
// ---------------------------------------------------------------------------------------------

/// A microsecond, the tick of Postgres's, MySQL's and DuckDB's default times.
pub const MICROSECOND: &str = "1/1000000";

/// The draft the engine's canonical JSON Schema ([`crate::schema_type`]) is read by: its keywords
/// (`prefixItems` among them) are draft 2020-12's.
pub const CANONICAL_SCHEMA: &str = "https://json-schema.org/draft/2020-12/schema";

impl Kind {
    /// Where the kind stands in the spec's list of kinds, which orders a union's members.
    pub fn rank(&self) -> usize {
        match self {
            Kind::Boolean => 0,
            Kind::Integer(_) => 1,
            Kind::Float(_) => 2,
            Kind::Decimal(_) => 3,
            Kind::Text(_) => 4,
            Kind::Bytes(_) => 5,
            Kind::Uuid => 6,
            Kind::Array(_) => 7,
            Kind::Record(_) => 8,
            Kind::Json(_) => 9,
            Kind::Date(_) => 10,
            Kind::Time(_) => 11,
            Kind::DateTime(_) => 12,
            Kind::Instant(_) => 13,
            Kind::Duration(_) => 14,
            Kind::Union(_) => 15,
            Kind::Set(_) => 16,
            Kind::Map(_) => 17,
            Kind::Range(_) => 18,
            Kind::Multirange(_) => 19,
            Kind::Geometry(_) => 20,
            Kind::Xml(_) => 21,
            Kind::Opaque => 22,
            Kind::Any => 23,
            Kind::Unknown => 24,
            Kind::TaggedUnion(_) => 25,
        }
    }

    /// The kind's name as the format writes it.
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Boolean => "boolean",
            Kind::Integer(_) => "integer",
            Kind::Float(_) => "float",
            Kind::Decimal(_) => "decimal",
            Kind::Text(_) => "text",
            Kind::Bytes(_) => "bytes",
            Kind::Uuid => "uuid",
            Kind::Array(_) => "array",
            Kind::Record(_) => "record",
            Kind::Json(_) => "json",
            Kind::Date(_) => "date",
            Kind::Time(_) => "time",
            Kind::DateTime(_) => "datetime",
            Kind::Instant(_) => "instant",
            Kind::Duration(_) => "duration",
            Kind::Union(_) => "union",
            Kind::Set(_) => "set",
            Kind::Map(_) => "map",
            Kind::Range(_) => "range",
            Kind::Multirange(_) => "multirange",
            Kind::Geometry(_) => "geometry",
            Kind::Xml(_) => "xml",
            Kind::Opaque => "opaque",
            Kind::Any => "any",
            Kind::Unknown => "unknown",
            Kind::TaggedUnion(_) => "tagged_union",
        }
    }
}

impl From<Kind> for Type {
    fn from(kind: Kind) -> Type {
        Type { kind, name: None, comment: None }
    }
}

impl Type {
    pub fn new(kind: Kind) -> Type {
        kind.into()
    }

    /// The same type under the database's name for it.
    pub fn named(mut self, name: impl Into<String>) -> Type {
        self.name = Some(name.into());
        self
    }

    pub fn boolean() -> Type {
        Kind::Boolean.into()
    }

    /// A two's complement integer of `bits`.
    pub fn integer(bits: u32) -> Type {
        Kind::Integer(IntegerType { bits, ..IntegerType::default() }).into()
    }

    /// An integer from 0 to 2^bits − 1.
    pub fn unsigned(bits: u32) -> Type {
        Kind::Integer(IntegerType { bits, unsigned: true, ..IntegerType::default() }).into()
    }

    pub fn float(exponent_bits: u32, mantissa_bits: u32) -> Type {
        Kind::Float(FloatType { exponent_bits, mantissa_bits, ..FloatType::default() }).into()
    }

    /// IEEE 754's double, which holds neither infinity nor NaN as this type says.
    pub fn double() -> Type {
        Type::float(11, 52)
    }

    /// IEEE 754's single.
    pub fn single() -> Type {
        Type::float(8, 23)
    }

    pub fn decimal(d: DecimalType) -> Type {
        Kind::Decimal(d).into()
    }

    /// Valid Unicode text.
    pub fn text() -> Type {
        Kind::Text(TextType::default()).into()
    }

    /// Text that is one of `values`.
    pub fn text_of(values: Vec<String>) -> Type {
        Kind::Text(TextType { values: Some(values), ..TextType::default() }).into()
    }

    pub fn bytes() -> Type {
        Kind::Bytes(BytesType::default()).into()
    }

    pub fn uuid() -> Type {
        Kind::Uuid.into()
    }

    pub fn array(element: Position) -> Type {
        Kind::Array(ArrayType { element: Box::new(element), size: None, any_depth: false }).into()
    }

    pub fn record(fields: Vec<Field>) -> Type {
        Kind::Record(RecordType { fields }).into()
    }

    /// Any JSON.
    pub fn json() -> Type {
        Kind::Json(JsonType::default()).into()
    }

    /// JSON the statement built, of `content`.
    pub fn json_of(content: Position) -> Type {
        Kind::Json(JsonType { content: Some(Box::new(content)), schema: None }).into()
    }

    /// JSON every value of which the engine's canonical JSON Schema `source` admits, carrying
    /// `$schema`, the draft the canonical form is read by ([`CANONICAL_SCHEMA`]). `true` and
    /// `false`, which have no keyword to carry it, are written as the objects that mean the same,
    /// `{}` and `{ "not": {} }`.
    pub fn json_schema_of(source: Value) -> Type {
        let mut keywords = match source {
            Value::Object(keywords) => keywords,
            Value::Bool(false) => serde_json::Map::from_iter([("not".to_string(), Value::Object(serde_json::Map::new()))]),
            _ => serde_json::Map::new(),
        };
        keywords.insert("$schema".into(), CANONICAL_SCHEMA.into());
        Kind::Json(JsonType { content: None, schema: Some(Value::Object(keywords)) }).into()
    }

    /// A JSON type's JSON Schema in the engine's canonical form, where it has one: without its
    /// `$schema`, and `true` and `false` as themselves ([`Type::json_schema_of`]).
    pub fn schema(&self) -> Option<Value> {
        let Kind::Json(JsonType { schema: Some(schema), .. }) = &self.kind else { return None };
        let Value::Object(keywords) = schema else { return Some(schema.clone()) };
        let mut keywords = keywords.clone();
        keywords.remove("$schema");
        Some(match keywords.len() {
            0 => Value::Bool(true),
            1 if keywords.get("not").is_some_and(|n| n.as_object().is_some_and(|n| n.is_empty())) => Value::Bool(false),
            _ => Value::Object(keywords),
        })
    }

    /// Whether the type is JSON: any JSON, JSON a statement built, or JSON a schema holds to.
    pub fn is_json(&self) -> bool {
        matches!(self.kind, Kind::Json(_))
    }

    pub fn opaque(name: impl Into<String>) -> Type {
        Type::new(Kind::Opaque).named(name)
    }

    pub fn unknown() -> Type {
        Kind::Unknown.into()
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self.kind, Kind::Unknown)
    }

    /// A union of `members` as rule 4 spells it: flat (a member that is a union of the same `per`
    /// gives its members), each member once, in kind order (members of one kind in the order
    /// given), and a lone member itself. An empty union holds no value.
    pub fn union(members: Vec<Type>, per: Per) -> Type {
        let mut flat: Vec<Type> = Vec::new();
        for member in members {
            let inner = match member.kind {
                Kind::Union(UnionType { members, per: p }) if p == per && member.name.is_none() => members,
                _ => vec![member],
            };
            for m in inner {
                if !flat.contains(&m) {
                    flat.push(m);
                }
            }
        }
        if flat.len() == 1 {
            return flat.pop().expect("one member");
        }
        flat.sort_by_key(|t| t.kind.rank());
        Kind::Union(UnionType { members: flat, per }).into()
    }

    /// A tagged union of `alternatives`, in the order the type declares them.
    pub fn tagged_union(alternatives: Vec<Alternative>) -> Type {
        Kind::TaggedUnion(TaggedUnionType { alternatives }).into()
    }

    /// Every name the type and the types inside it carry, each once, outermost first.
    pub fn names(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_names(&mut out);
        out
    }

    fn collect_names(&self, out: &mut Vec<String>) {
        if let Some(name) = &self.name {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        match &self.kind {
            Kind::Array(a) => a.element.ty.collect_names(out),
            Kind::Set(s) => s.element.ty.collect_names(out),
            Kind::Record(r) => r.fields.iter().for_each(|f| f.ty.collect_names(out)),
            Kind::Json(JsonType { content: Some(c), .. }) => c.ty.collect_names(out),
            Kind::Union(u) => u.members.iter().for_each(|m| m.collect_names(out)),
            Kind::TaggedUnion(t) => t.alternatives.iter().for_each(|a| a.ty.collect_names(out)),
            Kind::Map(m) => {
                m.key.collect_names(out);
                m.value.ty.collect_names(out);
            }
            Kind::Range(r) | Kind::Multirange(r) => r.subtype.collect_names(out),
            _ => {}
        }
    }

    /// The type with each name inside it, its own included, that `rename` answers for replaced
    /// by the answer: a domain read as its base type, where a rule keys on the base's name.
    pub fn renamed(mut self, rename: &impl Fn(&str) -> Option<String>) -> Type {
        if let Some(name) = self.name.as_deref().and_then(rename) {
            self.name = Some(name);
        }
        let position = |p: Position| Position { ty: p.ty.renamed(rename), ..p };
        self.kind = match self.kind {
            Kind::Array(a) => Kind::Array(ArrayType { element: Box::new(position(*a.element)), ..a }),
            Kind::Set(s) => Kind::Set(SetType { element: Box::new(position(*s.element)), ..s }),
            Kind::Record(r) => Kind::Record(RecordType { fields: r.fields.into_iter().map(|f| Field { ty: f.ty.renamed(rename), ..f }).collect(), ..r }),
            Kind::Json(JsonType { content: Some(c), schema }) => Kind::Json(JsonType { content: Some(Box::new(position(*c))), schema }),
            Kind::Union(u) => Kind::Union(UnionType { members: u.members.into_iter().map(|m| m.renamed(rename)).collect(), ..u }),
            Kind::TaggedUnion(t) => Kind::TaggedUnion(TaggedUnionType { alternatives: t.alternatives.into_iter().map(|a| Alternative { ty: a.ty.renamed(rename), ..a }).collect() }),
            Kind::Map(m) => Kind::Map(MapType { key: Box::new(m.key.renamed(rename)), value: Box::new(position(*m.value)), ..m }),
            Kind::Range(r) => Kind::Range(RangeType { subtype: Box::new(r.subtype.renamed(rename)), ..r }),
            Kind::Multirange(r) => Kind::Multirange(RangeType { subtype: Box::new(r.subtype.renamed(rename)), ..r }),
            other => other,
        };
        self
    }

    /// The union's members, or the type alone.
    pub fn members(&self) -> &[Type] {
        match &self.kind {
            Kind::Union(u) => &u.members,
            _ => std::slice::from_ref(self),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// One line
// ---------------------------------------------------------------------------------------------

impl Type {
    /// The type in one short line, as the oracle's `expected.txt` and the engines' tests print it:
    /// its name (else its kind), then what narrows it in parentheses: an enum's values
    /// (`varchar('a' | 'b')`), a size (`varchar(≤255)`, `bytes(=6)`), a decimal's precision and
    /// scale (`numeric(10,2)`), an integer's bounds (`int16(0..2155)`). An array is its element and
    /// `[]`, `?` after an element that may be null (`text?[]`); a record is its fields
    /// (`point{ x: float8, y: float8 }`, `?` after a field that may be null and before one that
    /// may be absent); JSON a statement built is its content after its name (`json { id: int4 }`),
    /// and JSON a schema holds to the schema (`json {"type":"string"}`); a union per value is its
    /// members between `|` and a union per execution `either(…)`.
    pub fn short(&self) -> String {
        let label = || self.name.clone().unwrap_or_else(|| self.kind.name().to_string());
        let with = |facts: Vec<String>| if facts.is_empty() { label() } else { format!("{}({})", label(), facts.join(", ")) };
        let size = |size: &Option<Size>| size.as_ref().map(short_size).into_iter().collect::<Vec<_>>();
        match &self.kind {
            Kind::Integer(i) => {
                let label = self.name.clone().unwrap_or_else(|| format!("{}int{}", if i.unsigned { "u" } else { "" }, i.bits));
                let mut facts = Vec::new();
                if !i.bounds.is_empty() {
                    facts.push(format!("{}..{}", i.bounds.min.as_deref().unwrap_or(""), i.bounds.max.as_deref().unwrap_or("")));
                }
                if let Some(values) = &i.values {
                    facts.push(values.join(" | "));
                }
                if facts.is_empty() { label } else { format!("{label}({})", facts.join(", ")) }
            }
            Kind::Float(f) if self.name.is_none() => format!("float({},{})", f.exponent_bits, f.mantissa_bits),
            Kind::Decimal(d) => match (d.precision, d.min_scale, d.max_scale) {
                (Some(p), Some(min), Some(max)) if min == max => with(vec![format!("{p},{min}")]),
                (Some(p), _, _) => with(vec![p.to_string()]),
                _ => label(),
            },
            Kind::Text(t) => {
                let mut facts = size(&t.size);
                if let Some(values) = &t.values {
                    facts.push(values.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(" | "));
                }
                if let Some(p) = &t.pattern {
                    facts.push(format!("{:?} {}", p.dialect, p.source).to_ascii_lowercase());
                }
                if let Some(shape) = t.ill_formed {
                    facts.push(format!("{shape:?}").to_ascii_lowercase());
                }
                with(facts)
            }
            Kind::Bytes(b) => with(size(&b.size)),
            Kind::Array(a) => {
                let element = position_short(&a.element);
                let sized = match &a.size {
                    Some(Size { exact: Some(n), .. }) => n.to_string(),
                    Some(s) => format!("{}..{}", s.min.map(|n| n.to_string()).unwrap_or_default(), s.max.map(|n| n.to_string()).unwrap_or_default()),
                    None => String::new(),
                };
                let element = if element.contains(' ') { format!("({element})") } else { element };
                // A Postgres array type's own name (`_int4`) says no more than its element.
                match self.name.as_deref() {
                    Some(name) if !name.rsplit('.').next().unwrap_or(name).starts_with('_') => format!("{name}({element}[{sized}])"),
                    _ => format!("{element}[{sized}]"),
                }
            }
            Kind::Record(r) => {
                let fields: Vec<String> = r.fields.iter().map(|f| format!("{}{}: {}", f.name, if f.optional { "?" } else { "" }, position_short(&f.position()))).collect();
                format!("{}{{ {} }}", self.name.clone().unwrap_or_default(), fields.join(", "))
            }
            Kind::Json(j) => match (&j.content, self.schema()) {
                (Some(content), _) => format!("{} {}", label(), position_short(content)),
                (None, Some(schema)) => format!("{} {}", label(), schema),
                (None, None) => label(),
            },
            Kind::Union(u) => {
                let members: Vec<String> = u.members.iter().map(Type::short).collect();
                let inner = match u.per {
                    Per::Value => members.join(" | "),
                    Per::Execution => format!("either({})", members.join(", ")),
                };
                match &self.name {
                    Some(name) => format!("{name}({inner})"),
                    None => inner,
                }
            }
            Kind::TaggedUnion(t) => {
                let alternatives: Vec<String> = t.alternatives.iter().map(|a| format!("{}: {}", a.tag, position_short(&a.position()))).collect();
                format!("{}<{}>", self.name.as_deref().unwrap_or("tagged"), alternatives.join(" | "))
            }
            Kind::Set(set) => format!("{}{{{}}}", label(), position_short(&set.element)),
            Kind::Map(m) => format!("{}({} => {})", label(), m.key.short(), position_short(&m.value)),
            Kind::Range(r) | Kind::Multirange(r) if self.name.is_none() => format!("{}({})", self.kind.name(), r.subtype.short()),
            _ => label(),
        }
    }
}

fn position_short(p: &Position) -> String {
    format!("{}{}", p.ty.short(), if p.nullable { "?" } else { "" })
}

fn short_size(size: &Size) -> String {
    let unit = match (size.unit, &size.encoding) {
        (Some(SizeUnit::Byte), Some(encoding)) => format!(" bytes {encoding}"),
        (Some(SizeUnit::Byte), None) => " bytes".to_string(),
        (Some(SizeUnit::Utf16Unit), _) => " utf16".to_string(),
        (None, _) => String::new(),
    };
    let n = match (size.exact, size.min, size.max) {
        (Some(n), _, _) => format!("={n}"),
        (None, Some(min), Some(max)) => format!("{min}..{max}"),
        (None, Some(min), None) => format!("≥{min}"),
        (None, None, Some(max)) => format!("≤{max}"),
        (None, None, None) => String::new(),
    };
    format!("{n}{unit}")
}

// ---------------------------------------------------------------------------------------------
// Rule 7: what no shape rules out
// ---------------------------------------------------------------------------------------------

impl Type {
    /// The first of rule 7's validity checks this type fails, at any depth, with where it is, or
    /// `Ok` for a valid type: a side of a range takes one bound, a whole-number kind no exclusive
    /// one, a minimum is at most its maximum, a size's `exact` stands alone and its `min` never
    /// equals its `max`, `encoding` comes only with a byte unit, a tick is positive and a float's
    /// widths are too; and an opaque type has its name.
    pub fn check(&self) -> Result<(), String> {
        self.check_at("")
    }

    fn check_at(&self, at: &str) -> Result<(), String> {
        let here = if at.is_empty() { self.kind.name().to_string() } else { format!("{at} ({})", self.kind.name()) };
        let fail = |why: String| Err(format!("{here}: {why}"));
        let position = |p: &Position, step: &str| p.ty.check_at(&format!("{at}{step}"));
        match &self.kind {
            Kind::Integer(i) => {
                if i.bits == 0 {
                    return fail("bits is 0".into());
                }
                if i.bounds.exclusive_min.is_some() || i.bounds.exclusive_max.is_some() {
                    return fail("a whole-number kind writes its bounds inclusive".into());
                }
                check_order(&i.bounds, decimal_order).or_else(fail)?;
            }
            Kind::Float(f) => {
                if f.exponent_bits == 0 || f.mantissa_bits == 0 {
                    return fail("a float's widths are positive".into());
                }
                check_bounds(&f.bounds, decimal_order).or_else(fail)?;
            }
            Kind::Decimal(d) => {
                check_bounds(&d.bounds, decimal_order).or_else(fail)?;
                if let (Some(min), Some(max)) = (d.min_scale, d.max_scale) {
                    if min > max {
                        return fail(format!("min_scale {min} is above max_scale {max}"));
                    }
                }
            }
            Kind::Text(t) => {
                if let Some(size) = &t.size {
                    check_size(size, t.ill_formed.is_none()).or_else(fail)?;
                }
            }
            Kind::Bytes(BytesType { size: Some(size) }) => check_size(size, false).or_else(fail)?,
            Kind::Array(a) => {
                if let Some(size) = &a.size {
                    check_size(size, false).or_else(fail)?;
                }
                position(&a.element, "[]")?;
            }
            Kind::Set(s) => {
                if let Some(size) = &s.size {
                    check_size(size, false).or_else(fail)?;
                }
                position(&s.element, "[]")?;
            }
            Kind::Record(r) => {
                for f in &r.fields {
                    f.ty.check_at(&format!("{at}.{}", f.name))?;
                }
            }
            Kind::Json(j) => {
                if let Some(content) = &j.content {
                    position(content, "<json>")?;
                }
            }
            Kind::Date(d) => check_bounds(&d.bounds, text_order).or_else(fail)?,
            Kind::Time(t) => {
                check_tick(&t.tick).or_else(fail)?;
                check_bounds(&t.bounds, text_order).or_else(fail)?;
            }
            Kind::DateTime(t) => {
                check_tick(&t.tick).or_else(fail)?;
                check_bounds(&t.bounds, text_order).or_else(fail)?;
            }
            Kind::Instant(t) => {
                check_tick(&t.tick).or_else(fail)?;
                check_bounds(&t.bounds, text_order).or_else(fail)?;
            }
            Kind::Duration(d) => {
                if let Some(time) = &d.time {
                    check_tick(&time.tick).or_else(fail)?;
                }
            }
            Kind::Union(u) => {
                for (i, m) in u.members.iter().enumerate() {
                    m.check_at(&format!("{at}|{i}"))?;
                }
            }
            Kind::TaggedUnion(t) => {
                let mut tags = std::collections::HashSet::new();
                for a in &t.alternatives {
                    if !tags.insert(a.tag.as_str()) {
                        return fail(format!("tag {:?} names two alternatives", a.tag));
                    }
                    a.ty.check_at(&format!("{at}<{}>", a.tag))?;
                }
            }
            Kind::Map(m) => {
                m.key.check_at(&format!("{at}<key>"))?;
                position(&m.value, "<value>")?;
            }
            Kind::Range(r) | Kind::Multirange(r) => r.subtype.check_at(&format!("{at}<subtype>"))?,
            Kind::Opaque if self.name.is_none() => return fail("an opaque type has its name".into()),
            _ => {}
        }
        Ok(())
    }
}

fn check_bounds(b: &Bounds, order: fn(&str, &str) -> Option<std::cmp::Ordering>) -> Result<(), String> {
    if b.min.is_some() && b.exclusive_min.is_some() {
        return Err("min beside exclusive_min".into());
    }
    if b.max.is_some() && b.exclusive_max.is_some() {
        return Err("max beside exclusive_max".into());
    }
    check_order(b, order)
}

fn check_order(b: &Bounds, order: fn(&str, &str) -> Option<std::cmp::Ordering>) -> Result<(), String> {
    let low = b.min.as_deref().or(b.exclusive_min.as_deref());
    let high = b.max.as_deref().or(b.exclusive_max.as_deref());
    if let (Some(low), Some(high)) = (low, high) {
        if order(low, high) == Some(std::cmp::Ordering::Greater) {
            return Err(format!("the minimum {low} is above the maximum {high}"));
        }
    }
    Ok(())
}

/// Two numbers in canonical decimal text, compared, where both read as `f64` without loss of
/// order (an exponent such as `1e131072` reads as infinity, which still orders it).
fn decimal_order(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let (x, y) = (a.parse::<f64>().ok()?, b.parse::<f64>().ok()?);
    x.partial_cmp(&y)
}

/// Two dates or times in ISO 8601, compared: by their text where both have four-digit years and
/// no sign, else not at all.
fn text_order(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let plain = |s: &str| !s.starts_with(['+', '-']);
    (plain(a) && plain(b)).then(|| a.cmp(b))
}

fn check_size(size: &Size, unicode: bool) -> Result<(), String> {
    if size.exact.is_some() && (size.min.is_some() || size.max.is_some()) {
        return Err("a size's exact beside its min or max".into());
    }
    if size.exact.is_none() && size.min.is_none() && size.max.is_none() {
        return Err("a size with no length".into());
    }
    if let (Some(min), Some(max)) = (size.min, size.max) {
        if min >= max {
            return Err(format!("a size's min {min} is not below its max {max}"));
        }
    }
    if size.unit.is_some() && !unicode {
        return Err("only valid Unicode text names a size's unit".into());
    }
    if size.encoding.is_some() != (size.unit == Some(SizeUnit::Byte)) {
        return Err("encoding comes with unit: \"byte\" and only with it".into());
    }
    Ok(())
}

fn check_tick(tick: &str) -> Result<(), String> {
    let (n, d) = tick.split_once('/').unwrap_or((tick, "1"));
    match (n.parse::<u64>(), d.parse::<u64>()) {
        (Ok(n), Ok(d)) if n > 0 && d > 0 => Ok(()),
        _ => Err(format!("the tick {tick:?} is not a positive fraction")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Read and written back, `value` is itself, and it passes rule 7.
    fn round_trip(value: Value) -> Type {
        let ty: Type = serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{value}: {e}"));
        assert_eq!(serde_json::to_value(&ty).unwrap(), value);
        ty.check().unwrap_or_else(|e| panic!("{value}: {e}"));
        ty
    }

    #[test]
    fn every_kind_reads_and_writes_as_the_spec_spells_it() {
        round_trip(json!({"kind": "integer", "bits": 64, "name": "int8"}));
        round_trip(json!({"kind": "integer", "bits": 8, "unsigned": true, "max": "32"}));
        let tagged = round_trip(json!({"kind": "tagged_union", "name": "union", "alternatives": [{"tag": "num", "nullable": true, "type": {"kind": "integer", "bits": 32}}, {"tag": "str", "type": {"kind": "text"}}]}));
        assert_eq!(tagged.short(), "union<num: int32? | str: text>");
        assert!(serde_json::from_value::<Type>(json!({"kind": "tagged_union", "alternatives": [{"tag": "a", "type": {"kind": "text"}}, {"tag": "a", "type": {"kind": "text"}}]})).unwrap().check().is_err());
        round_trip(json!({"kind": "float", "exponent_bits": 11, "mantissa_bits": 52, "infinity": true, "nan": true, "name": "float8"}));
        round_trip(json!({"kind": "decimal", "min_scale": 0, "max_scale": 16383, "exclusive_min": "-1e131072", "exclusive_max": "1e131072", "infinity": true, "nan": true, "name": "numeric"}));
        round_trip(json!({"kind": "text", "size": {"unit": "byte", "encoding": "UTF-8", "max": 65535}, "name": "text"}));
        round_trip(json!({"kind": "text", "enum": ["sad", "ok", "happy"], "name": "public.mood"}));
        round_trip(json!({"kind": "text", "size": {"max": 255}, "pattern": {"dialect": "like", "source": "%@%"}, "name": "public.email"}));
        round_trip(json!({"kind": "text", "ill_formed": "utf8"}));
        round_trip(json!({"kind": "array", "any_depth": true, "name": "_varchar", "element": {"nullable": true, "type": {"kind": "text", "size": {"max": 20}, "name": "varchar"}}}));
        round_trip(json!({"kind": "array", "size": {"exact": 7}, "name": "bit", "element": {"type": {"kind": "boolean"}}}));
        round_trip(json!({"kind": "json", "name": "json", "content": {"type": {"kind": "array", "element": {"type": {"kind": "text", "name": "text"}}}}}));
        round_trip(json!({"kind": "instant", "tick": "1/1000000", "min": "-4713-11-24T00:00:00Z", "max": "+294276-12-31T23:59:59.999999Z", "infinity": true, "name": "timestamptz"}));
        round_trip(json!({"kind": "duration", "months": {"max": "P2147483648M"}, "days": {"max": "P2147483648D"}, "time": {"tick": "1/1000000", "max": "PT2562047788H54.775808S"}, "infinity": true, "name": "interval"}));
        round_trip(json!({"kind": "time", "tick": "1/1000000", "max": "24:00:00", "name": "time"}));
        round_trip(json!({"kind": "range", "subtype": {"kind": "date"}, "name": "daterange"}));
        round_trip(json!({"kind": "union", "per": "execution", "members": [{"kind": "integer", "bits": 64}, {"kind": "float", "exponent_bits": 11, "mantissa_bits": 52}, {"kind": "decimal"}]}));
        round_trip(json!({"kind": "set", "size": {"max": 2}, "name": "set", "element": {"type": {"kind": "text", "enum": ["a", "b"]}}}));
        round_trip(json!({"kind": "map", "name": "hstore", "key": {"kind": "text"}, "value": {"nullable": true, "type": {"kind": "text"}}}));
        round_trip(json!({"kind": "geometry", "model": "geodetic", "srid": 4326, "subtype": "point", "coordinates": "xy", "name": "geography"}));
        round_trip(json!({"kind": "xml", "fragment": true, "name": "xml"}));
        round_trip(json!({"kind": "opaque", "name": "ltree"}));
        round_trip(json!({"kind": "record", "name": "point", "fields": [{"name": "x", "type": {"kind": "float", "exponent_bits": 11, "mantissa_bits": 52}}, {"name": "at", "nullable": true, "optional": true, "type": {"kind": "uuid"}}]}));
        round_trip(json!({"kind": "unknown"}));
    }

    #[test]
    fn a_false_flag_reads_as_an_absent_one() {
        let ty: Type = serde_json::from_value(json!({"kind": "text", "ill_formed": false})).unwrap();
        assert_eq!(serde_json::to_value(&ty).unwrap(), json!({"kind": "text"}));
        let ty: Type = serde_json::from_value(json!({"kind": "integer", "bits": 16, "unsigned": false})).unwrap();
        assert_eq!(serde_json::to_value(&ty).unwrap(), json!({"kind": "integer", "bits": 16}));
    }

    #[test]
    fn a_union_is_flat_once_each_and_in_kind_order() {
        let text = Type::text();
        let nested = Type::union(vec![Type::bytes(), text.clone()], Per::Value);
        let u = Type::union(vec![nested, Type::integer(64), text.clone()], Per::Value);
        assert_eq!(serde_json::to_value(&u).unwrap(), json!({"kind": "union", "members": [{"kind": "integer", "bits": 64}, {"kind": "text"}, {"kind": "bytes"}]}));
        assert_eq!(Type::union(vec![text.clone(), text.clone()], Per::Value), text);
        // A union is per value unless it says otherwise, and reads a `per` written as `value`.
        assert_eq!(serde_json::from_value::<Type>(json!({"kind": "union", "per": "value", "members": [{"kind": "integer", "bits": 64}, {"kind": "text"}]})).unwrap(), Type::union(vec![Type::integer(64), Type::text()], Per::Value));
        // A union of another `per` stays a member.
        let per_execution = Type::union(vec![Type::integer(64), Type::double()], Per::Execution);
        assert_eq!(Type::union(vec![per_execution.clone(), Type::text()], Per::Value).members().len(), 2);
    }

    #[test]
    fn rule_7_refuses_what_no_shape_rules_out() {
        let refused = |value: Value| serde_json::from_value::<Type>(value.clone()).unwrap().check().err().unwrap_or_else(|| panic!("{value} passed"));
        refused(json!({"kind": "float", "exponent_bits": 11, "mantissa_bits": 52, "min": "0", "exclusive_min": "0"}));
        refused(json!({"kind": "integer", "bits": 32, "exclusive_min": "0"}));
        refused(json!({"kind": "integer", "bits": 32, "min": "5", "max": "1"}));
        refused(json!({"kind": "text", "size": {"exact": 3, "max": 4}}));
        refused(json!({"kind": "text", "size": {"min": 4, "max": 4}}));
        refused(json!({"kind": "text", "size": {"unit": "utf16_unit", "encoding": "UTF-8", "max": 4}}));
        refused(json!({"kind": "bytes", "size": {"unit": "byte", "max": 4}}));
        refused(json!({"kind": "text", "ill_formed": "utf8", "size": {"unit": "byte", "encoding": "UTF-8", "max": 4}}));
        refused(json!({"kind": "time", "tick": "0"}));
        refused(json!({"kind": "decimal", "min_scale": 3, "max_scale": 2}));
        refused(json!({"kind": "opaque"}));
        refused(json!({"kind": "array", "element": {"type": {"kind": "float", "exponent_bits": 0, "mantissa_bits": 3}}}));
    }

    #[test]
    fn the_schema_url_is_the_one_the_spec_names() {
        let spec: serde_json::Value = serde_json::from_str(include_str!("../../../spec/typedefs.schema.json")).unwrap();
        assert_eq!(spec["$id"], SCHEMA);
        assert_eq!(spec["$defs"]["StatementDescriptor"]["properties"]["$schema"]["const"], SCHEMA);
    }

    #[test]
    fn a_statement_descriptor_reads_and_writes_as_the_spec_spells_it() {
        let value = json!({
            "$schema": SCHEMA,
            "dialect": {"name": "postgres", "version": "18.1"},
            "sql": "select id, email, bio from users where id = $1",
            "kind": "select",
            "comment": "One user, by id.",
            "params": [{"name": "id", "type": {"kind": "integer", "bits": 64, "name": "int8"}}],
            "columns": [
                {"name": "id", "type": {"kind": "integer", "bits": 64, "name": "int8"}},
                {"name": "email", "type": {"kind": "text", "size": {"max": 255}, "name": "varchar"}},
                {"name": "bio", "nullable": true, "type": {"kind": "text", "name": "text"}, "comment": "Shown on the profile page"}
            ],
            "cardinality": "at_most_one",
            "diagnostics": []
        });
        let d: StatementDescriptor = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(&d).unwrap(), value);
    }
}
