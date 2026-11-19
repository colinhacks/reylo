//! The type model every database's engine answers with: Reylo Type Defs (RTD), the type format of
//! `spec/typedefs.schema.json`, as Rust types that serialize to exactly its JSON. A [`Type`] says
//! which values a slot can hold: its `kind`, that kind's fields, and the source's name for it, its
//! `label`; a [`Slot`] is where a type is used and says whether a value there may be null; a
//! [`StatementDescriptor`] is what introspecting one statement returns.
//!
//! The spec is the reference for every kind, field and rule; the comments here only say how the
//! Rust spells it. A flag is a `bool` written only when true, a value inside the format (a bound, a
//! tick, a duration's maximum, an enum's values) is a `String` in its kind's canonical text, and a
//! count (`bits`, a size, a precision, a scale) is a number. [`Type::check`] runs rule 7's
//! validity checks.

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn is_false(b: &bool) -> bool {
    !*b
}

/// A type: its kind, with that kind's fields, and what every type may carry, the source's name
/// for it and the comment the source keeps on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Type {
    #[serde(flatten)]
    pub kind: Kind,
    /// The source's name for the type, less the modifiers a field states (`numeric` for
    /// `numeric(10,2)`), qualified where a schema defines it; absent on a type the source builds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// `COMMENT ON TYPE` or `COMMENT ON DOMAIN`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// A short name for the type, as JSON Schema's `title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// The kinds, in the order the spec's `Type` lists them, which is also the order a union lists
/// its members in ([`Kind::rank`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    Boolean(BooleanType),
    Integer(IntegerType),
    Float(FloatType),
    Decimal(DecimalType),
    Text(TextType),
    Bytes(BytesType),
    Uuid(UuidType),
    /// One of a fixed list of values of another kind: an enum's labels, a CHECK's list, a literal.
    Enum(EnumType),
    Date(DateType),
    Time(TimeType),
    #[serde(rename = "datetime")]
    DateTime(DateTimeType),
    Instant(InstantType),
    /// A moment and the zone it was written in.
    #[serde(rename = "zoned_datetime")]
    ZonedDateTime(ZonedDateTimeType),
    Duration(DurationType),
    Array(ArrayType),
    Tuple(TupleType),
    Struct(StructType),
    Map(MapType),
    /// Rows of columns: a statement's result, or a table nested in a value.
    Table(TableType),
    Json(JsonType),
    Union(UnionType),
    /// Values of every member at once.
    Intersection(UnionType),
    /// One of several alternatives, each named by the tag every value carries.
    TaggedUnion(TaggedUnionType),
    Range(RangeType),
    Multirange(RangeType),
    Geometry(GeometryType),
    Xml(XmlType),
    /// The value that is always null, in a nullable slot.
    Null,
    /// A type the source knows and the format does not structure; its `label` is all it says.
    Opaque,
    /// Nothing is known about the values.
    Unknown,
}

/// A truth value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BooleanType {}

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
/// Only text names a `unit` (code points when absent), and only where it names its encoding.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<SizeUnit>,
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

    /// At most `n` bytes, in the encoding the text names.
    pub fn bytes_at_most(n: u64) -> Size {
        Size { unit: Some(SizeUnit::Byte), max: Some(n), ..Size::default() }
    }
}

/// What a text's size counts other than code points: the encoding's code units, or its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeUnit {
    CodeUnit,
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
}

/// The character sets a text's `encoding` names, by their IANA names (spec/README.md pins each to
/// one mapping table), matched without regard to case.
pub const CHARSETS: &[&str] = &[
    "UTF-8", "UTF-16", "UTF-32", "CESU-8", "GB18030", "US-ASCII", "ISO-8859-1", "ISO-8859-2", "ISO-8859-3", "ISO-8859-4", "ISO-8859-5", "ISO-8859-6", "ISO-8859-7", "ISO-8859-8", "ISO-8859-9", "ISO-8859-10", "ISO-8859-13", "ISO-8859-14",
    "ISO-8859-15", "ISO-8859-16", "windows-874", "windows-1250", "windows-1251", "windows-1252", "windows-1253", "windows-1254", "windows-1255", "windows-1256", "windows-1257", "windows-1258", "IBM437", "IBM850", "IBM852", "IBM866", "KOI8-R",
    "KOI8-U", "macintosh", "TIS-620", "DEC-MCS", "hp-roman8", "SEN_850200_B", "Shift_JIS", "Windows-31J", "EUC-JP", "ISO-2022-JP", "EUC-KR", "GB2312", "GBK", "Big5", "Big5-HKSCS",
];

/// The encodings of [`CHARSETS`] that encode every Unicode character.
const UNICODE_ENCODINGS: &[&str] = &["UTF-8", "UTF-16", "UTF-32", "CESU-8", "GB18030"];

/// The greatest code point.
pub const MAX_CODE_POINT: u32 = 0x10FFFF;

/// Text: any valid Unicode text unless `encoding` names the character set its values are held in,
/// `ill_formed` lets a value be any sequence of that set's code units, or `code_points` narrows the
/// characters.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextType {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ill_formed: bool,
    /// Inclusive ranges of code points every character falls in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_points: Option<Vec<[u32; 2]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    /// An I-Regexp (RFC 9485) every value matches whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// What the text is meant to be, by JSON Schema's format name (`email`, `uri`) or a source's
    /// own: a name alone, which says nothing of the values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BytesType {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

/// A 128-bit identifier: any 128 bits, unless `versions` names the RFC 9562 versions its values have.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UuidType {
    /// Each value is of RFC 9562's variant and has one of these versions, from 1 to 8.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<u8>>,
}

/// One of `values`, each a value of `ty` in its canonical text; one value is a literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumType {
    #[serde(rename = "type")]
    pub ty: Box<Type>,
    pub values: Vec<String>,
}

/// A sequence of elements, each in the one `element` slot; flat, an element of an array kind
/// being an array of its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrayType {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    /// No two elements are equal, by the source's own equality: a set, MySQL's `SET`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub distinct: bool,
    /// Last, so that what an array says of itself reads before its elements.
    pub element: Box<Slot>,
}

impl ArrayType {
    /// An array of `element`, of any size.
    pub fn of(element: Slot) -> ArrayType {
        ArrayType { element: Box::new(element), size: None, distinct: false }
    }
}

/// A sequence of values with no names: `elements` always there, then `optional_elements` that may be
/// missing from the end, then any number in the `rest` slot.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TupleType {
    pub elements: Vec<Slot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_elements: Vec<Slot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest: Option<Box<Slot>>,
}

/// Named fields, in order, and with `extra`, any other key holding a value in its slot.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StructType {
    pub fields: Vec<Field>,
    /// Keys beyond the fields: an open object. It says nothing of the named fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Box<Slot>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapType {
    pub key: Box<Slot>,
    pub value: Box<Slot>,
    /// How many keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
}

/// Rows of `columns`, as many as `cardinality` says, unique on `unique` and sorted on `order`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableType {
    pub cardinality: Cardinality,
    pub columns: Vec<Field>,
    /// Each set of columns no two rows agree on all of, a null equal to another null, and a
    /// table's primary key first where the rows hold it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unique: Vec<Vec<String>>,
    /// The columns the rows are sorted on, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<SortKey>,
}

/// A JSON value: what the statement built (`content`), a JSON Schema every value satisfies
/// (`schema`), or neither, any JSON.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JsonType {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Box<Slot>>,
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
    /// Each value keeps a UTC offset beside the time.
    #[serde(default, skip_serializing_if = "is_false")]
    pub offset: bool,
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

/// A point on the timeline and nothing else.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InstantType {
    pub tick: String,
    #[serde(flatten)]
    pub bounds: Bounds,
    #[serde(default, skip_serializing_if = "is_false")]
    pub infinity: bool,
}

/// A point on the timeline and the zone it was written in, a region or a UTC offset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ZonedDateTimeType {
    pub tick: String,
    /// Every value's zone is a UTC offset, never a region.
    #[serde(default, skip_serializing_if = "is_false")]
    pub offset_only: bool,
    #[serde(flatten)]
    pub bounds: Bounds,
    #[serde(default, skip_serializing_if = "is_false")]
    pub infinity: bool,
}

/// A length of time in the parts it has, each signed on its own: `max` writes each part (months,
/// days, time) at the magnitude it reaches, as one ISO 8601 duration, and `tick` is the time
/// part's step, present exactly when `max` has a time part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DurationType {
    pub max: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub infinity: bool,
}

/// A union's or an intersection's members: flat, at least two, in kind order ([`Type::union`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnionType {
    pub members: Vec<Type>,
}

/// One of several alternatives, each named by the tag every value carries, in the order the type
/// declares them: two may hold one type, and a value says which it is, even holding NULL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaggedUnionType {
    pub alternatives: Vec<Alternative>,
}

/// An alternative of a tagged union: its tag, unique in the type, and the slot of the value it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alternative {
    pub tag: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "is_false")]
    pub nullable: bool,
}

impl Alternative {
    pub fn slot(&self) -> Slot {
        Slot { ty: self.ty.clone(), nullable: self.nullable }
    }
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
// Slots
// ---------------------------------------------------------------------------------------------

/// Where a type is used: a column, a parameter, a field, an element, a map's key or value, JSON
/// content. Nullability lives here, never in a type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    #[serde(rename = "type")]
    pub ty: Type,
    /// The value may be null (inside JSON content, JSON null).
    #[serde(default, skip_serializing_if = "is_false")]
    pub nullable: bool,
}

impl Slot {
    pub fn new(ty: Type, nullable: bool) -> Slot {
        Slot { ty, nullable }
    }

    pub fn non_null(ty: Type) -> Slot {
        Slot { ty, nullable: false }
    }

    pub fn nullable(ty: Type) -> Slot {
        Slot { ty, nullable: true }
    }
}

/// A struct's field or a table's column.
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
    /// The value an optional field takes where its key is absent, in its type's canonical text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// What the field is, as its source documents it: `COMMENT ON COLUMN`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// A short name for the field, as JSON Schema's `title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl Field {
    pub fn new(name: impl Into<String>, ty: Type, nullable: bool) -> Field {
        Field { name: name.into(), ty, nullable, optional: false, default: None, comment: None, title: None }
    }

    pub fn slot(&self) -> Slot {
        Slot { ty: self.ty.clone(), nullable: self.nullable }
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
pub const SCHEMA: &str = "https://reylo.dev/typedefs/0.1.1.json";

/// A column a table's rows are sorted on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortKey {
    pub column: String,
    /// Sorted from the highest value down.
    #[serde(default, skip_serializing_if = "is_false")]
    pub desc: bool,
    /// Where NULLs sort.
    pub nulls: NullsOrder,
}

/// Where NULLs sort among a column's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NullsOrder {
    First,
    Last,
}

/// What a statement returns: a table. It reads and writes as a [`Type`], and refuses any but a
/// table, as the format does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Type", into = "Type")]
pub struct ResultRows(Type);

impl ResultRows {
    /// The rows of `columns`, as many as `cardinality` says, unique on `unique` and sorted on
    /// `order`, which a table holds only where it can hold more than one row.
    pub fn new(cardinality: Cardinality, columns: Vec<Field>, unique: Vec<Vec<String>>, order: Vec<SortKey>) -> ResultRows {
        let many = cardinality == Cardinality::Many;
        let (unique, order) = if many { (unique, order) } else { (Vec::new(), Vec::new()) };
        ResultRows(Kind::Table(TableType { cardinality, columns, unique, order }).into())
    }

    /// The table the rows are.
    pub fn table(&self) -> &TableType {
        match &self.0.kind {
            Kind::Table(table) => table,
            _ => unreachable!("a result is a table"),
        }
    }

    /// The columns.
    pub fn columns(&self) -> &[Field] {
        &self.table().columns
    }
}

impl TryFrom<Type> for ResultRows {
    type Error = String;

    fn try_from(ty: Type) -> Result<Self, String> {
        match &ty.kind {
            Kind::Table(_) => Ok(ResultRows(ty)),
            other => Err(format!("a statement's result is a table, not {}", other.name())),
        }
    }
}

impl From<ResultRows> for Type {
    fn from(result: ResultRows) -> Self {
        result.0
    }
}

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
    /// What the statement returns.
    pub result: ResultRows,
    /// What running the statement may do; absent where nothing is said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Capabilities>,
    pub diagnostics: Vec<Diagnostic>,
}

/// What running a statement may do, an upper bound: a flag it lacks, or a table it does not name,
/// is something it cannot do.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Each table it touches, with the most it may do to its rows, or `unknown` where it may touch
    /// tables the analysis cannot name.
    pub tables: CapabilityTables,
    /// Creates, alters or drops what the database defines, or changes who may use it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ddl: bool,
    /// Begins or ends a transaction.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub transaction: bool,
    /// Leaves state on the connection after it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub session: bool,
    /// Reaches past the connection and the data: the server's settings, another session, a file,
    /// the network.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub external: bool,
}

/// The tables a statement touches, by the name the database qualifies each with, or `unknown`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CapabilityTables {
    Named(std::collections::BTreeMap<String, Access>),
    Unknown(Unknown),
}

impl Default for CapabilityTables {
    fn default() -> Self {
        CapabilityTables::Named(Default::default())
    }
}

/// The literal `unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unknown {
    #[serde(rename = "unknown")]
    Unknown,
}

/// The most a statement may do to a table's rows, each covering the ones before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Read,
    /// Reads them and locks them until its transaction ends.
    Readlock,
    /// Inserts, updates or deletes them.
    Write,
}

impl Capabilities {
    /// That it may do `access` to `table`, beside what it may already do to it.
    pub fn touch(&mut self, table: impl Into<String>, access: Access) {
        if let CapabilityTables::Named(tables) = &mut self.tables {
            let at = tables.entry(table.into()).or_insert(access);
            *at = (*at).max(access);
        }
    }

    /// That it may touch tables the analysis cannot name.
    pub fn unknown_tables(&mut self) {
        self.tables = CapabilityTables::Unknown(Unknown::Unknown);
    }

    /// Whether it may touch tables the analysis cannot name.
    pub fn tables_unknown(&self) -> bool {
        matches!(self.tables, CapabilityTables::Unknown(_))
    }

    /// Rule 7: a table is named.
    pub fn check(&self) -> Result<(), String> {
        match &self.tables {
            CapabilityTables::Named(tables) if tables.keys().any(String::is_empty) => Err("a table of capabilities has no name".into()),
            _ => Ok(()),
        }
    }
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

/// How many rows a table holds.
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

/// A microsecond, the tick of Postgres's and MySQL's default times.
pub const MICROSECOND: &str = "1/1000000";

/// The draft the engine's canonical JSON Schema ([`crate::schema_type`]) is read by: its keywords
/// (`prefixItems` among them) are draft 2020-12's.
pub const CANONICAL_SCHEMA: &str = "https://json-schema.org/draft/2020-12/schema";

impl Kind {
    /// Where the kind stands in the spec's list of kinds, which orders a union's members.
    pub fn rank(&self) -> usize {
        match self {
            Kind::Boolean(_) => 0,
            Kind::Integer(_) => 1,
            Kind::Float(_) => 2,
            Kind::Decimal(_) => 3,
            Kind::Text(_) => 4,
            Kind::Bytes(_) => 5,
            Kind::Uuid(_) => 6,
            Kind::Enum(_) => 7,
            Kind::Date(_) => 8,
            Kind::Time(_) => 9,
            Kind::DateTime(_) => 10,
            Kind::Instant(_) => 11,
            Kind::ZonedDateTime(_) => 12,
            Kind::Duration(_) => 13,
            Kind::Array(_) => 14,
            Kind::Tuple(_) => 15,
            Kind::Struct(_) => 16,
            Kind::Map(_) => 17,
            Kind::Table(_) => 18,
            Kind::Json(_) => 19,
            Kind::Union(_) => 20,
            Kind::Intersection(_) => 21,
            Kind::TaggedUnion(_) => 22,
            Kind::Range(_) => 23,
            Kind::Multirange(_) => 24,
            Kind::Geometry(_) => 25,
            Kind::Xml(_) => 26,
            Kind::Null => 27,
            Kind::Opaque => 28,
            Kind::Unknown => 29,
        }
    }

    /// The kind's name as the format writes it.
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Boolean(_) => "boolean",
            Kind::Integer(_) => "integer",
            Kind::Float(_) => "float",
            Kind::Decimal(_) => "decimal",
            Kind::Text(_) => "text",
            Kind::Bytes(_) => "bytes",
            Kind::Uuid(_) => "uuid",
            Kind::Enum(_) => "enum",
            Kind::Date(_) => "date",
            Kind::Time(_) => "time",
            Kind::DateTime(_) => "datetime",
            Kind::Instant(_) => "instant",
            Kind::ZonedDateTime(_) => "zoned_datetime",
            Kind::Duration(_) => "duration",
            Kind::Array(_) => "array",
            Kind::Tuple(_) => "tuple",
            Kind::Struct(_) => "struct",
            Kind::Map(_) => "map",
            Kind::Table(_) => "table",
            Kind::Json(_) => "json",
            Kind::Union(_) => "union",
            Kind::Intersection(_) => "intersection",
            Kind::TaggedUnion(_) => "tagged_union",
            Kind::Range(_) => "range",
            Kind::Multirange(_) => "multirange",
            Kind::Geometry(_) => "geometry",
            Kind::Xml(_) => "xml",
            Kind::Null => "null",
            Kind::Opaque => "opaque",
            Kind::Unknown => "unknown",
        }
    }

    /// Whether the format writes the kind's values as text, so that an enum may list them.
    pub fn has_canonical_text(&self) -> bool {
        matches!(self, Kind::Boolean(_) | Kind::Integer(_) | Kind::Float(_) | Kind::Decimal(_) | Kind::Text(_) | Kind::Bytes(_) | Kind::Uuid(_) | Kind::Date(_) | Kind::Time(_) | Kind::DateTime(_) | Kind::Instant(_) | Kind::ZonedDateTime(_) | Kind::Duration(_))
    }
}

impl From<Kind> for Type {
    fn from(kind: Kind) -> Type {
        Type { kind, label: None, comment: None, title: None }
    }
}

impl Type {
    pub fn new(kind: Kind) -> Type {
        kind.into()
    }

    /// The same type under the source's name for it.
    pub fn named(mut self, name: impl Into<String>) -> Type {
        self.label = Some(name.into());
        self
    }

    pub fn boolean() -> Type {
        Kind::Boolean(BooleanType::default()).into()
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

    /// One of `values`, each a value of `ty`; an empty list is no enum, and `ty` itself.
    pub fn one_of(ty: Type, values: Vec<String>) -> Type {
        if values.is_empty() {
            return ty;
        }
        Kind::Enum(EnumType { ty: Box::new(ty), values }).into()
    }

    /// Text that is one of `values`.
    pub fn text_of(values: Vec<String>) -> Type {
        Type::one_of(Type::text(), values)
    }

    pub fn bytes() -> Type {
        Kind::Bytes(BytesType::default()).into()
    }

    pub fn uuid() -> Type {
        Kind::Uuid(UuidType::default()).into()
    }

    pub fn array(element: Slot) -> Type {
        Kind::Array(ArrayType::of(element)).into()
    }

    pub fn structure(fields: Vec<Field>) -> Type {
        Kind::Struct(StructType { fields, extra: None }).into()
    }

    /// Any JSON.
    pub fn json() -> Type {
        Kind::Json(JsonType::default()).into()
    }

    /// JSON the statement built, of `content`.
    pub fn json_of(content: Slot) -> Type {
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

    /// The value that is always null; its slot is nullable.
    pub fn null() -> Type {
        Kind::Null.into()
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self.kind, Kind::Unknown)
    }

    /// The type an enum's values are of, or the type itself.
    pub fn base(&self) -> &Type {
        match &self.kind {
            Kind::Enum(e) => &e.ty,
            _ => self,
        }
    }

    /// A union of `members` as rule 4 spells it: flat (a member that is an unnamed union gives its
    /// members), each member once, in kind order (members of one kind in the order given), and a
    /// lone member itself. A union of none is the null kind, whose slot is nullable: the one value
    /// a slot of no other type holds.
    pub fn union(members: Vec<Type>) -> Type {
        let mut flat: Vec<Type> = Vec::new();
        for member in members {
            let inner = match member.kind {
                Kind::Union(UnionType { members }) if member.label.is_none() => members,
                _ => vec![member],
            };
            for m in inner {
                if !flat.contains(&m) {
                    flat.push(m);
                }
            }
        }
        match flat.len() {
            0 => return Type::null(),
            1 => return flat.pop().expect("one member"),
            _ => {}
        }
        flat.sort_by_key(|t| t.kind.rank());
        Kind::Union(UnionType { members: flat }).into()
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
        if let Some(name) = &self.label {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        for inner in self.inner() {
            inner.collect_names(out);
        }
    }

    /// The types directly inside this one, in order: an enum's type, an array's element, a tuple's
    /// elements, a struct's fields and extra, a map's key and value, a table's columns, JSON
    /// content, the members of a union or an intersection, a tagged union's alternatives, and a
    /// range's subtype.
    pub fn inner(&self) -> Vec<&Type> {
        match &self.kind {
            Kind::Enum(e) => vec![&e.ty],
            Kind::Array(a) => vec![&a.element.ty],
            Kind::Tuple(t) => t.elements.iter().chain(&t.optional_elements).chain(t.rest.as_deref()).map(|s| &s.ty).collect(),
            Kind::Struct(s) => s.fields.iter().map(|f| &f.ty).chain(s.extra.as_deref().map(|e| &e.ty)).collect(),
            Kind::Map(m) => vec![&m.key.ty, &m.value.ty],
            Kind::Table(t) => t.columns.iter().map(|c| &c.ty).collect(),
            Kind::Json(JsonType { content: Some(c), .. }) => vec![&c.ty],
            Kind::Union(u) | Kind::Intersection(u) => u.members.iter().collect(),
            Kind::TaggedUnion(t) => t.alternatives.iter().map(|a| &a.ty).collect(),
            Kind::Range(r) | Kind::Multirange(r) => vec![&r.subtype],
            _ => Vec::new(),
        }
    }

    /// The type with each name inside it, its own included, that `rename` answers for replaced
    /// by the answer: a domain read as its base type, where a rule keys on the base's name.
    pub fn renamed(mut self, rename: &impl Fn(&str) -> Option<String>) -> Type {
        if let Some(name) = self.label.as_deref().and_then(rename) {
            self.label = Some(name);
        }
        let slot = |s: Slot| Slot { ty: s.ty.renamed(rename), ..s };
        let field = |f: Field| Field { ty: f.ty.renamed(rename), ..f };
        self.kind = match self.kind {
            Kind::Enum(e) => Kind::Enum(EnumType { ty: Box::new(e.ty.renamed(rename)), ..e }),
            Kind::Array(a) => Kind::Array(ArrayType { element: Box::new(slot(*a.element)), ..a }),
            Kind::Tuple(t) => Kind::Tuple(TupleType { elements: t.elements.into_iter().map(slot).collect(), optional_elements: t.optional_elements.into_iter().map(slot).collect(), rest: t.rest.map(|r| Box::new(slot(*r))) }),
            Kind::Struct(s) => Kind::Struct(StructType { fields: s.fields.into_iter().map(field).collect(), extra: s.extra.map(|e| Box::new(slot(*e))) }),
            Kind::Map(m) => Kind::Map(MapType { key: Box::new(slot(*m.key)), value: Box::new(slot(*m.value)), size: m.size }),
            Kind::Table(t) => Kind::Table(TableType { columns: t.columns.into_iter().map(field).collect(), ..t }),
            Kind::Json(JsonType { content: Some(c), schema }) => Kind::Json(JsonType { content: Some(Box::new(slot(*c))), schema }),
            Kind::Union(u) => Kind::Union(UnionType { members: u.members.into_iter().map(|m| m.renamed(rename)).collect() }),
            Kind::Intersection(u) => Kind::Intersection(UnionType { members: u.members.into_iter().map(|m| m.renamed(rename)).collect() }),
            Kind::TaggedUnion(t) => Kind::TaggedUnion(TaggedUnionType { alternatives: t.alternatives.into_iter().map(|a| Alternative { ty: a.ty.renamed(rename), ..a }).collect() }),
            Kind::Range(r) => Kind::Range(RangeType { subtype: Box::new(r.subtype.renamed(rename)) }),
            Kind::Multirange(r) => Kind::Multirange(RangeType { subtype: Box::new(r.subtype.renamed(rename)) }),
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
    /// (`varchar('a' | 'b')`, `int4(1)`), a size (`varchar(≤255)`, `bytes(=6)`), a decimal's
    /// precision and scale (`numeric(10,2)`), a number's bounds (`int16(0..2155)`, `numeric(>0..<1)`)
    /// and divisor (`int4(×5)`), a uuid's versions (`uuid(v7)`), a text's encoding, code points and
    /// format (`text(email)`). A map is its key and value, then how many keys it holds (`map(text => int4, ≤64)`).
    /// An array is its element and `[]`, `?` after an element that may be null (`text?[]`), and a
    /// set its element in braces (`set{text}`, and `{int4}` for one a Postgres array type or no name names); a struct is its fields (`point{ x: float8, y: float8 }`,
    /// `?` after a field that may be null and before one that may be absent); a table its cardinality
    /// and columns (`many{ id: int8 }`); JSON a statement built is its content after its name
    /// (`json { id: int4 }`), and JSON a schema holds to the schema; a union is its members between `|`,
    /// an intersection between `&`.
    pub fn short(&self) -> String {
        let label = || self.label.clone().unwrap_or_else(|| self.kind.name().to_string());
        let with = |facts: Vec<String>| if facts.is_empty() { label() } else { format!("{}({})", label(), facts.join(", ")) };
        let size = |size: &Option<Size>| size.as_ref().map(short_size).into_iter().collect::<Vec<_>>();
        match &self.kind {
            Kind::Integer(i) => {
                let label = self.label.clone().unwrap_or_else(|| format!("{}int{}", if i.unsigned { "u" } else { "" }, i.bits));
                let mut facts = Vec::new();
                facts.extend(short_bounds(&i.bounds));
                facts.extend(i.multiple_of.as_ref().map(|m| format!("×{m}")));
                if facts.is_empty() { label } else { format!("{label}({})", facts.join(", ")) }
            }
            Kind::Float(f) if self.label.is_none() => format!("float({},{})", f.exponent_bits, f.mantissa_bits),
            Kind::Float(f) => with(short_bounds(&f.bounds).into_iter().collect()),
            Kind::Decimal(d) => {
                let mut facts = match (d.precision, d.min_scale, d.max_scale) {
                    (Some(p), Some(min), Some(max)) if min == max => vec![format!("{p},{min}")],
                    (Some(p), _, _) => vec![p.to_string()],
                    _ => Vec::new(),
                };
                facts.extend(short_bounds(&d.bounds));
                facts.extend(d.multiple_of.as_ref().map(|m| format!("×{m}")));
                with(facts)
            }
            Kind::Text(t) => {
                let mut facts = size(&t.size);
                facts.extend(t.encoding.clone());
                if t.ill_formed {
                    facts.push("ill-formed".into());
                }
                // Every character but NUL, which every Postgres text is, reads as `¬NUL`.
                match t.code_points.as_deref() {
                    Some([[1, MAX_CODE_POINT]]) => facts.push("¬NUL".into()),
                    Some(points) => facts.push(points.iter().map(|[lo, hi]| format!("U+{lo:04X}..U+{hi:04X}")).collect::<Vec<_>>().join(" ")),
                    None => {}
                }
                facts.extend(t.pattern.as_ref().map(|p| format!("/{p}/")));
                facts.extend(t.media_type.clone());
                facts.extend(t.format.clone());
                with(facts)
            }
            Kind::Bytes(b) => with(size(&b.size).into_iter().chain(b.media_type.clone()).collect()),
            Kind::Uuid(u) => with(u.versions.iter().flatten().map(|v| format!("v{v}")).collect()),
            Kind::Enum(e) => {
                let quoted = matches!(e.ty.kind, Kind::Text(_));
                let values = e.values.iter().map(|v| if quoted { quote(v) } else { v.clone() }).collect::<Vec<_>>().join(" | ");
                let base = e.ty.short();
                let base = match &self.label {
                    // The enum's own name stands for its base's.
                    Some(name) => match base.find('(') {
                        Some(at) => format!("{name}{}", &base[at..]),
                        None => name.clone(),
                    },
                    None => base,
                };
                match base.strip_suffix(')') {
                    Some(open) if base.contains('(') && !base.ends_with("[])") => format!("{open}, {values})"),
                    _ => format!("{base}({values})"),
                }
            }
            // A Postgres array type's own name (`_int4`), like no name at all, says no more than its element.
            Kind::Array(a) if a.distinct => {
                let name = self.label.as_deref().filter(|n| !n.rsplit('.').next().unwrap_or(*n).starts_with('_')).unwrap_or("");
                format!("{name}{{{}}}", slot_short(&a.element))
            }
            Kind::Array(a) => {
                let element = slot_short(&a.element);
                let sized = match &a.size {
                    Some(Size { exact: Some(n), .. }) => n.to_string(),
                    Some(s) => format!("{}..{}", s.min.map(|n| n.to_string()).unwrap_or_default(), s.max.map(|n| n.to_string()).unwrap_or_default()),
                    None => String::new(),
                };
                let element = if spaced(&element) { format!("({element})") } else { element };
                // A Postgres array type's own name (`_int4`) says no more than its element.
                match self.label.as_deref() {
                    Some(name) if !name.rsplit('.').next().unwrap_or(name).starts_with('_') => format!("{name}({element}[{sized}])"),
                    _ => format!("{element}[{sized}]"),
                }
            }
            Kind::Struct(s) => {
                let mut fields: Vec<String> = s.fields.iter().map(field_short).collect();
                fields.extend(s.extra.as_ref().map(|e| format!("*: {}", slot_short(e))));
                format!("{}{{ {} }}", self.label.clone().unwrap_or_default(), fields.join(", "))
            }
            Kind::Table(t) => format!("{}{{ {} }}", t.cardinality.as_str(), t.columns.iter().map(field_short).collect::<Vec<_>>().join(", ")),
            Kind::Json(j) => match (&j.content, self.schema()) {
                (Some(content), _) => format!("{} {}", label(), slot_short(content)),
                (None, Some(schema)) => format!("{} {}", label(), schema),
                (None, None) => label(),
            },
            Kind::Union(u) | Kind::Intersection(u) => {
                let mark = if matches!(self.kind, Kind::Union(_)) { " | " } else { " & " };
                let inner = u.members.iter().map(Type::short).collect::<Vec<_>>().join(mark);
                match &self.label {
                    Some(name) => format!("{name}({inner})"),
                    None => inner,
                }
            }
            Kind::TaggedUnion(t) => {
                let alternatives: Vec<String> = t.alternatives.iter().map(|a| format!("{}: {}", a.tag, slot_short(&a.slot()))).collect();
                format!("{}<{}>", self.label.as_deref().unwrap_or("tagged"), alternatives.join(" | "))
            }
            Kind::Tuple(t) => {
                let mut items: Vec<String> = t.elements.iter().map(slot_short).collect();
                items.extend(t.optional_elements.iter().map(|s| format!("{}=", slot_short(s))));
                items.extend(t.rest.as_ref().map(|r| format!("...{}", slot_short(r))));
                format!("({})", items.join(", "))
            }
            Kind::Map(m) => {
                let keys = m.size.as_ref().map(|s| format!(", {}", short_size(s))).unwrap_or_default();
                format!("{}({} => {}{keys})", label(), slot_short(&m.key), slot_short(&m.value))
            }
            Kind::Time(t) if t.offset => format!("{}(offset)", label()),
            Kind::ZonedDateTime(z) if z.offset_only => format!("{}(offset)", label()),
            Kind::Range(r) | Kind::Multirange(r) if self.label.is_none() => format!("{}({})", self.kind.name(), r.subtype.short()),
            _ => label(),
        }
    }
}

/// A text value in quotes, on one line: a quote doubled, a backslash, a line break, a carriage
/// return, a tab and any other control character escaped (`'it''s'`, `'a\\nb'`).
fn quote(v: &str) -> String {
    let mut out = String::from("'");
    for c in v.chars() {
        match c {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// Whether a short form holds a space outside its parentheses, its brackets and its quoted text,
/// so that an array of it takes parentheses (`(int4 | text)[]`, `({ a: int4 })[]`) and one of
/// `varchar(≤8, ¬NUL)` takes none.
fn spaced(short: &str) -> bool {
    let (mut depth, mut quoted) = (0usize, false);
    for c in short.chars() {
        match c {
            '\'' => quoted = !quoted,
            _ if quoted => {}
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn slot_short(s: &Slot) -> String {
    format!("{}{}", s.ty.short(), if s.nullable { "?" } else { "" })
}

fn field_short(f: &Field) -> String {
    format!("{}{}: {}", f.name, if f.optional { "?" } else { "" }, slot_short(&f.slot()))
}

fn short_size(size: &Size) -> String {
    let unit = match size.unit {
        Some(SizeUnit::Byte) => " bytes",
        Some(SizeUnit::CodeUnit) => " units",
        None => "",
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

/// Rule 7 on a table's keys and order: only where it can hold more than one row, each naming its
/// columns, an order naming each once, and no column optional, since a row has every column.
fn check_table(t: &TableType) -> Result<(), String> {
    if let Some(c) = t.columns.iter().find(|c| c.optional || c.default.is_some()) {
        return Err(format!("column {:?} is optional, and a row has every column", c.name));
    }
    if t.unique.is_empty() && t.order.is_empty() {
        return Ok(());
    }
    if t.cardinality != Cardinality::Many {
        return Err("unique and order are only for a table of many rows".into());
    }
    let fits = |name: &str| t.columns.iter().any(|c| c.name == name);
    for key in &t.unique {
        if key.is_empty() {
            return Err("a set of unique columns holds at least one".into());
        }
        if let Some(k) = key.iter().find(|k| !fits(k)) {
            return Err(format!("unique column {k:?} is not a column"));
        }
    }
    if let Some(o) = t.order.iter().find(|o| !fits(&o.column)) {
        return Err(format!("order column {:?} is not a column", o.column));
    }
    if let Some((at, o)) = t.order.iter().enumerate().find(|(at, o)| t.order[..*at].iter().any(|p| p.column == o.column)) {
        return Err(format!("order names column {:?} twice, at {at}", o.column));
    }
    Ok(())
}

/// Rule 7 on text: `ill_formed` and a unit only with an `encoding`, of the names the format
/// accepts, a Unicode encoding only where it says more than any Unicode text would, and code
/// points sorted, disjoint, never adjacent and no greater than U+10FFFF.
fn check_text(t: &TextType) -> Result<(), String> {
    let encoding = t.encoding.as_deref();
    if let Some(e) = encoding {
        if !CHARSETS.iter().any(|c| c.eq_ignore_ascii_case(e)) {
            return Err(format!("{e:?} is no character set the format names"));
        }
    }
    if t.ill_formed && encoding.is_none() {
        return Err("ill_formed names the encoding whose code units a value is".into());
    }
    let unit = t.size.as_ref().and_then(|s| s.unit);
    if unit.is_some() && encoding.is_none() {
        return Err("a size counts code units or bytes only of an encoding the text names".into());
    }
    if encoding.is_some_and(|e| UNICODE_ENCODINGS.iter().any(|u| u.eq_ignore_ascii_case(e))) && !t.ill_formed && unit.is_none() {
        return Err("a Unicode encoding is written only beside ill_formed, or a size that counts its code units or bytes".into());
    }
    if let Some(points) = &t.code_points {
        if points.is_empty() {
            return Err("code_points lists at least one range".into());
        }
        for (i, [lo, hi]) in points.iter().enumerate() {
            if lo > hi || *hi > MAX_CODE_POINT {
                return Err(format!("code point range {lo}..{hi} is not one"));
            }
            if i > 0 && points[i - 1][1].saturating_add(1) >= *lo {
                return Err("code point ranges are sorted, disjoint and never adjacent".into());
            }
        }
    }
    if let Some(size) = &t.size {
        check_size(size)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Rule 7: what no shape rules out
// ---------------------------------------------------------------------------------------------

impl Type {
    /// The first of rule 7's validity checks this type fails, at any depth, with where it is, or
    /// `Ok` for a valid type.
    pub fn check(&self) -> Result<(), String> {
        self.check_at("")
    }

    fn check_at(&self, at: &str) -> Result<(), String> {
        let here = if at.is_empty() { self.kind.name().to_string() } else { format!("{at} ({})", self.kind.name()) };
        let fail = |why: String| Err(format!("{here}: {why}"));
        let slot = |s: &Slot, step: &str| -> Result<(), String> {
            if matches!(s.ty.kind, Kind::Null) && !s.nullable {
                return Err(format!("{here}: the null kind stands only in a nullable slot"));
            }
            s.ty.check_at(&format!("{at}{step}"))
        };
        let field = |f: &Field| -> Result<(), String> {
            if f.default.is_some() && !f.optional {
                return Err(format!("{here}: field {:?} has a default but is not optional", f.name));
            }
            slot(&f.slot(), &format!(".{}", f.name))
        };
        let unsized_ = |size: &Option<Size>| -> Result<(), String> {
            match size {
                Some(s) if s.unit.is_some() => Err(format!("{here}: only a text's size names a unit")),
                Some(s) => check_size(s).or_else(fail),
                None => Ok(()),
            }
        };
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
            Kind::Uuid(u) => {
                if let Some(versions) = &u.versions {
                    if versions.is_empty() || versions.iter().any(|v| !(1..=8).contains(v)) {
                        return fail("a uuid's versions are one or more from 1 to 8".into());
                    }
                    if versions.windows(2).any(|w| w[0] >= w[1]) {
                        return fail("a uuid's versions are listed once each, in order".into());
                    }
                }
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
            Kind::Text(t) => check_text(t).or_else(fail)?,
            Kind::Bytes(b) => unsized_(&b.size)?,
            Kind::Enum(e) => {
                if !e.ty.kind.has_canonical_text() {
                    return fail(format!("an enum lists values of a kind written as text, not of {}", e.ty.kind.name()));
                }
                if e.values.is_empty() {
                    return fail("an enum lists at least one value".into());
                }
                if let Some((i, v)) = e.values.iter().enumerate().find(|(i, v)| e.values[..*i].contains(v)) {
                    return fail(format!("value {v:?} is listed twice, at {i}"));
                }
                if matches!(e.ty.kind, Kind::Boolean(_)) {
                    if e.values.iter().any(|v| v != "true" && v != "false") {
                        return fail(format!("a boolean's values are true and false, not {:?}", e.values));
                    }
                    if e.values.len() == 2 {
                        return fail("an enum of both booleans is a boolean".into());
                    }
                }
                e.ty.check_at(&format!("{at}<enum>"))?;
            }
            Kind::Array(a) => {
                unsized_(&a.size)?;
                slot(&a.element, "[]")?;
            }
            Kind::Struct(s) => {
                for f in &s.fields {
                    field(f)?;
                }
                if let Some(extra) = &s.extra {
                    if s.fields.is_empty() {
                        return fail("a struct of no fields with extra keys is a map".into());
                    }
                    slot(extra, ".*")?;
                }
            }
            Kind::Table(t) => {
                for c in &t.columns {
                    field(c)?;
                }
                check_table(t).or_else(fail)?;
            }
            Kind::Tuple(t) => {
                if t.elements.is_empty() && t.optional_elements.is_empty() {
                    return fail("a tuple holds at least one of elements and optional_elements; one with neither is an array".into());
                }
                for (i, e) in t.elements.iter().chain(&t.optional_elements).enumerate() {
                    slot(e, &format!(".{i}"))?;
                }
                if let Some(rest) = &t.rest {
                    slot(rest, ".*")?;
                }
            }
            Kind::Json(j) => {
                if let Some(content) = &j.content {
                    slot(content, "<json>")?;
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
            Kind::ZonedDateTime(t) => {
                check_tick(&t.tick).or_else(fail)?;
                check_bounds(&t.bounds, text_order).or_else(fail)?;
            }
            Kind::Duration(d) => match (duration_has_time(&d.max).map_err(|why| format!("{here}: {why}"))?, &d.tick) {
                (true, Some(tick)) => check_tick(tick).or_else(fail)?,
                (false, None) => {}
                _ => return fail("a duration has a tick exactly when its max has a time part".into()),
            },
            Kind::Union(u) | Kind::Intersection(u) => {
                if u.members.len() < 2 {
                    return fail("a union or an intersection has at least two members".into());
                }
                for (i, m) in u.members.iter().enumerate() {
                    if matches!(m.kind, Kind::Null) {
                        return fail("null is no member; the slot says a value may be null".into());
                    }
                    m.check_at(&format!("{at}|{i}"))?;
                }
                if matches!(self.kind, Kind::Intersection(_)) {
                    let structs = u.members.iter().filter(|m| matches!(m.kind, Kind::Struct(_))).count();
                    let text_maps = u.members.iter().filter(|m| matches!(&m.kind, Kind::Map(map) if matches!(map.key.ty.kind, Kind::Text(_)))).count();
                    if structs == u.members.len() || (structs == 1 && structs + text_maps == u.members.len()) {
                        return fail("an intersection one struct can write is written as that struct".into());
                    }
                }
            }
            Kind::TaggedUnion(t) => {
                let mut tags = std::collections::HashSet::new();
                for a in &t.alternatives {
                    if !tags.insert(a.tag.as_str()) {
                        return fail(format!("tag {:?} names two alternatives", a.tag));
                    }
                    slot(&a.slot(), &format!("<{}>", a.tag))?;
                }
            }
            Kind::Map(m) => {
                unsized_(&m.size)?;
                slot(&m.key, "<key>")?;
                slot(&m.value, "<value>")?;
            }
            Kind::Range(r) | Kind::Multirange(r) => r.subtype.check_at(&format!("{at}<subtype>"))?,
            Kind::Opaque if self.label.is_none() => return fail("an opaque type has its label".into()),
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

/// A number's bounds in the short notation (`0..150`, `>0..<1`), but for those an exponent writes:
/// the reach of the type itself (`numeric`'s `1e131072`), which says nothing of the column.
fn short_bounds(b: &Bounds) -> Option<String> {
    let side = |inclusive: &Option<String>, exclusive: &Option<String>, mark: &str| match (inclusive, exclusive) {
        (Some(v), _) if !v.contains('e') => v.clone(),
        (_, Some(v)) if !v.contains('e') => format!("{mark}{v}"),
        _ => String::new(),
    };
    let (low, high) = (side(&b.min, &b.exclusive_min, ">"), side(&b.max, &b.exclusive_max, "<"));
    (!low.is_empty() || !high.is_empty()).then(|| format!("{low}..{high}"))
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

fn check_size(size: &Size) -> Result<(), String> {
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
    Ok(())
}

/// Whether a duration's `max` has a time part, once it reads as the spec writes it: `P`, then
/// months in `M` and days in `D`, then `T` and time in `H`, `M` and `S`, each part at most once,
/// in that order, above zero and with no leading zero, only the seconds with a fraction, the
/// minutes and seconds below 60, and at least one part.
fn duration_has_time(max: &str) -> Result<bool, String> {
    let refused = || format!("the duration's max {max:?} is not one ISO 8601 duration in the spec's units and order");
    let rest = max.strip_prefix('P').ok_or_else(refused)?;
    let (date, time) = match rest.split_once('T') {
        Some((date, time)) => (date, Some(time)),
        None => (rest, None),
    };
    let mut parts = 0;
    for (text, units) in [(date, &['M', 'D'][..]), (time.unwrap_or_default(), &['H', 'M', 'S'][..])] {
        let mut next = 0;
        let mut start = 0;
        for (i, unit) in text.char_indices().filter(|(_, c)| !c.is_ascii_digit() && *c != '.') {
            let at = next + units[next..].iter().position(|&u| u == unit).ok_or_else(refused)?;
            let number = &text[start..i];
            let (whole, fraction) = number.split_once('.').map_or((number, None), |(w, f)| (w, Some(f)));
            let canonical = !whole.is_empty()
                && whole.bytes().all(|b| b.is_ascii_digit())
                && (whole == "0" || !whole.starts_with('0'))
                && fraction.is_none_or(|f| unit == 'S' && !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()) && !f.ends_with('0'))
                && (whole != "0" || fraction.is_some())
                && (unit == 'H' || units.len() == 2 || whole.parse::<u8>().is_ok_and(|n| n < 60));
            if !canonical {
                return Err(refused());
            }
            (next, start, parts) = (at + 1, i + 1, parts + 1);
        }
        if start != text.len() {
            return Err(refused());
        }
    }
    if parts == 0 || time == Some("") {
        return Err(refused());
    }
    Ok(time.is_some())
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

    /// The examples of spec/examples.ts, which `nub run typedefs` copies into spec/examples.json: each descriptor, whole or in part, reads and writes back as itself, and every type in it passes rule 7.
    #[test]
    fn the_spec_examples_read_and_write_as_they_are_written() {
        let examples: Value = serde_json::from_str(include_str!("../../../spec/examples.json")).unwrap();
        let examples = examples.as_object().unwrap();
        assert!(!examples.is_empty());
        let mut types = 0;
        for (name, example) in examples {
            let descriptor = &example["descriptor"];
            for value in descriptor["params"].as_array().into_iter().flatten() {
                let param: Param = serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{name}: {value}: {e}"));
                assert_eq!(serde_json::to_value(&param).unwrap(), *value, "{name}");
                param.ty.check().unwrap_or_else(|e| panic!("{name}: {}: {e}", param.name));
                types += 1;
            }
            for value in descriptor["result"]["columns"].as_array().into_iter().flatten() {
                let column: Field = serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{name}: {value}: {e}"));
                assert_eq!(serde_json::to_value(&column).unwrap(), *value, "{name}");
                column.ty.check().unwrap_or_else(|e| panic!("{name}: {}: {e}", column.name));
                types += 1;
            }
            // The table itself, whose `unique` sets and `order` rule 7 holds to its columns.
            if let Some(result) = descriptor.get("result") {
                round_trip(result.clone());
            }
            // A whole descriptor, which names the format's version.
            if descriptor.get("$schema").is_some() {
                let whole: StatementDescriptor = serde_json::from_value(descriptor.clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(serde_json::to_value(&whole).unwrap(), *descriptor, "{name}");
                assert_eq!(whole.schema, SCHEMA, "{name}");
            }
        }
        assert!(types > 0);
    }

    #[test]
    fn capabilities_read_as_the_spec_writes_them() {
        let read = |value: Value| -> Capabilities {
            let capabilities: Capabilities = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(&capabilities).unwrap(), value);
            capabilities
        };
        let named = read(json!({"tables": {"public.jobs": "readlock", "public.posts": "write"}}));
        assert!(!named.tables_unknown());
        assert!(read(json!({"tables": "unknown", "ddl": true, "session": true, "external": true})).tables_unknown());
        assert_eq!(read(json!({"tables": {}, "transaction": true})).tables, CapabilityTables::default());
        assert!(serde_json::from_value::<Capabilities>(json!({"tables": "all"})).is_err());
        assert!(serde_json::from_value::<Capabilities>(json!({"tables": {"t": "lock"}})).is_err());
        let mut touched = Capabilities::default();
        touched.touch("public.posts", Access::Readlock);
        touched.touch("public.posts", Access::Read);
        touched.touch("public.users", Access::Write);
        assert_eq!(serde_json::to_value(&touched).unwrap(), json!({"tables": {"public.posts": "readlock", "public.users": "write"}}));
        touched.unknown_tables();
        touched.touch("public.posts", Access::Write);
        assert!(touched.tables_unknown());
        assert!(read(json!({"tables": {"": "read"}})).check().is_err());
    }

    /// The kinds and fields no example in spec/examples.ts shows, and their short forms.
    #[test]
    fn what_no_example_shows_reads_and_writes_as_the_spec_spells_it() {
        let tagged = round_trip(json!({"kind": "tagged_union", "label": "union", "alternatives": [{"tag": "num", "nullable": true, "type": {"kind": "integer", "bits": 32}}, {"tag": "str", "type": {"kind": "text"}}]}));
        assert_eq!(tagged.short(), "union<num: int32? | str: text>");
        assert!(serde_json::from_value::<Type>(json!({"kind": "tagged_union", "alternatives": [{"tag": "a", "type": {"kind": "text"}}, {"tag": "a", "type": {"kind": "text"}}]})).unwrap().check().is_err());
        assert_eq!(round_trip(json!({"kind": "text", "encoding": "UTF-8", "size": {"unit": "byte", "max": 65535}, "label": "text"})).short(), "text(≤65535 bytes, UTF-8)");
        round_trip(json!({"kind": "text", "encoding": "UTF-16", "ill_formed": true, "size": {"unit": "code_unit", "max": 100}, "label": "nvarchar"}));
        round_trip(json!({"kind": "text", "encoding": "UTF-32", "ill_formed": true, "code_points": [[0, 1114111]]}));
        assert_eq!(round_trip(json!({"kind": "text", "code_points": [[1, 1114111]], "label": "text"})).short(), "text(¬NUL)");
        assert_eq!(round_trip(json!({"kind": "text", "code_points": [[0, 65535]], "label": "varchar"})).short(), "varchar(U+0000..U+FFFF)");
        round_trip(json!({"kind": "text", "encoding": "windows-1252", "size": {"max": 255}, "pattern": "[^@]+@[^@]+", "media_type": "text/plain"}));
        round_trip(json!({"kind": "bytes", "size": {"exact": 6}, "media_type": "image/png"}));
        round_trip(json!({"kind": "duration", "max": "PT838H59M59S", "tick": "1/1000000", "label": "time"}));
        round_trip(json!({"kind": "duration", "max": "P1199M"}));
        round_trip(json!({"kind": "duration", "max": "PT0.5S", "tick": "1/1000"}));
        assert_eq!(round_trip(json!({"kind": "array", "size": {"max": 2}, "label": "set", "distinct": true, "element": {"type": {"kind": "enum", "type": {"kind": "text"}, "values": ["a", "b"]}}})).short(), "set{text('a' | 'b')}");
        assert_eq!(round_trip(json!({"kind": "tuple", "elements": [{"type": {"kind": "integer", "bits": 32}}], "optional_elements": [{"nullable": true, "type": {"kind": "text"}}], "rest": {"type": {"kind": "boolean"}}})).short(), "(int32, text?=, ...boolean)");
        assert_eq!(round_trip(json!({"kind": "struct", "fields": [{"name": "id", "type": {"kind": "integer", "bits": 32}}, {"name": "tag", "optional": true, "default": "none", "comment": "What it is", "type": {"kind": "text"}}], "extra": {"type": {"kind": "text"}}})).short(), "{ id: int32, tag?: text, *: text }");
        round_trip(json!({"kind": "struct", "title": "User", "fields": [{"name": "id", "title": "Id", "type": {"kind": "uuid"}}, {"name": "password", "type": {"kind": "text", "format": "password"}}]}));
        assert_eq!(round_trip(json!({"kind": "text", "format": "email"})).short(), "text(email)");
        assert_eq!(round_trip(json!({"kind": "text", "size": {"max": 255}, "media_type": "text/plain", "format": "email", "label": "varchar"})).short(), "varchar(≤255, text/plain, email)");
        assert_eq!(round_trip(json!({"kind": "map", "key": {"type": {"kind": "text"}}, "value": {"type": {"kind": "integer", "bits": 32}}, "size": {"max": 64}})).short(), "map(text => int32, ≤64)");
        assert_eq!(round_trip(json!({"kind": "map", "label": "hstore", "key": {"type": {"kind": "text"}}, "value": {"nullable": true, "type": {"kind": "text"}}})).short(), "hstore(text => text?)");
        assert_eq!(round_trip(json!({"kind": "enum", "type": {"kind": "boolean"}, "values": ["true"]})).short(), "boolean(true)");
        assert_eq!(round_trip(json!({"kind": "enum", "type": {"kind": "integer", "bits": 32, "label": "int4"}, "values": ["1", "2"]})).short(), "int4(1 | 2)");
        assert_eq!(round_trip(json!({"kind": "enum", "type": {"kind": "text", "size": {"max": 20}, "label": "varchar"}, "values": ["a", "b"]})).short(), "varchar(≤20, 'a' | 'b')");
        assert_eq!(round_trip(json!({"kind": "enum", "type": {"kind": "text"}, "values": ["sad", "ok"], "label": "public.mood"})).short(), "public.mood('sad' | 'ok')");
        round_trip(json!({"kind": "enum", "type": {"kind": "float", "exponent_bits": 11, "mantissa_bits": 52, "label": "float8"}, "values": ["0.5"]}));
        round_trip(json!({"kind": "map", "label": "hstore", "key": {"type": {"kind": "text"}}, "value": {"nullable": true, "type": {"kind": "text"}}}));
        round_trip(json!({"kind": "map", "key": {"nullable": true, "type": {"kind": "text"}}, "value": {"type": {"kind": "integer", "bits": 32}}}));
        assert_eq!(round_trip(json!({"kind": "intersection", "members": [{"kind": "struct", "fields": []}, {"kind": "opaque", "label": "row"}]})).short(), "{  } & row");
        round_trip(json!({"kind": "geometry", "model": "geodetic", "srid": 4326, "subtype": "point", "coordinates": "xy", "label": "geography"}));
        round_trip(json!({"kind": "xml", "fragment": true, "label": "xml"}));
        round_trip(json!({"kind": "opaque", "label": "ltree"}));
        round_trip(json!({"kind": "uuid", "label": "uuid"}));
        assert_eq!(round_trip(json!({"kind": "uuid", "versions": [4, 7], "label": "uuid"})).short(), "uuid(v4, v7)");
        round_trip(json!({"kind": "datetime", "tick": "1/1000000", "label": "timestamp"}));
        assert_eq!(round_trip(json!({"kind": "time", "tick": "1/1000000", "offset": true, "label": "timetz"})).short(), "timetz(offset)");
        round_trip(json!({"kind": "zoned_datetime", "tick": "1/10000000", "offset_only": true, "label": "datetimeoffset"}));
        round_trip(json!({"kind": "multirange", "subtype": {"kind": "integer", "bits": 32, "label": "int4"}, "label": "int4multirange"}));
        let nothing = Slot::nullable(Type::null());
        assert_eq!(serde_json::to_value(Type::json_of(nothing)).unwrap(), json!({"kind": "json", "content": {"type": {"kind": "null"}, "nullable": true}}));
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
        let nested = Type::union(vec![Type::bytes(), text.clone()]);
        let u = Type::union(vec![nested, Type::integer(64), text.clone()]);
        assert_eq!(serde_json::to_value(&u).unwrap(), json!({"kind": "union", "members": [{"kind": "integer", "bits": 64}, {"kind": "text"}, {"kind": "bytes"}]}));
        assert_eq!(Type::union(vec![text.clone(), text.clone()]), text);
        // A union of none is the null kind.
        assert_eq!(Type::union(Vec::new()), Type::null());
        // A named union stays a member: its name is the database's type.
        let named = Type::union(vec![Type::integer(64), Type::double()]).named("NUMERIC");
        assert_eq!(Type::union(vec![named, Type::text()]).members().len(), 2);
    }

    #[test]
    fn rule_7_refuses_what_no_shape_rules_out() {
        let refused = |value: Value| serde_json::from_value::<Type>(value.clone()).unwrap().check().err().unwrap_or_else(|| panic!("{value} passed"));
        refused(json!({"kind": "float", "exponent_bits": 11, "mantissa_bits": 52, "min": "0", "exclusive_min": "0"}));
        refused(json!({"kind": "integer", "bits": 32, "exclusive_min": "0"}));
        refused(json!({"kind": "map", "key": {"type": {"kind": "text"}}, "value": {"type": {"kind": "text"}}, "size": {"min": 4, "max": 2}}));
        refused(json!({"kind": "map", "key": {"type": {"kind": "text"}}, "value": {"type": {"kind": "text"}}, "size": {"unit": "byte", "max": 2}}));
        refused(json!({"kind": "integer", "bits": 32, "min": "5", "max": "1"}));
        refused(json!({"kind": "uuid", "versions": [9]}));
        refused(json!({"kind": "uuid", "versions": []}));
        refused(json!({"kind": "uuid", "versions": [7, 4]}));
        refused(json!({"kind": "text", "size": {"exact": 3, "max": 4}}));
        refused(json!({"kind": "text", "size": {"min": 4, "max": 4}}));
        // An encoding the format names, ill_formed and a unit only beside one, a Unicode encoding only where it counts.
        refused(json!({"kind": "text", "encoding": "EBCDIC-US"}));
        refused(json!({"kind": "text", "ill_formed": true}));
        refused(json!({"kind": "text", "size": {"unit": "byte", "max": 4}}));
        refused(json!({"kind": "text", "encoding": "UTF-8"}));
        refused(json!({"kind": "text", "code_points": [[5, 1]]}));
        refused(json!({"kind": "text", "code_points": [[0, 1114112]]}));
        refused(json!({"kind": "text", "code_points": [[0, 10], [11, 20]]}));
        refused(json!({"kind": "text", "code_points": [[0, 10], [5, 20]]}));
        refused(json!({"kind": "bytes", "size": {"unit": "byte", "max": 4}}));
        refused(json!({"kind": "time", "tick": "0"}));
        // A duration's max: its tick exactly with a time part, and only the spec's units, in order, above zero.
        refused(json!({"kind": "duration", "max": "PT838H59M59S"}));
        refused(json!({"kind": "duration", "max": "P1199M", "tick": "1"}));
        for max in ["P", "PT", "P1MT", "P99Y11M", "P2W", "P1D1M", "PT1H1H", "PT90M", "PT1M60S", "PT01H", "P0D", "P1.5D", "PT1.50S", "-P1D", "P1M2"] {
            refused(json!({"kind": "duration", "max": max, "tick": "1"}));
        }
        refused(json!({"kind": "decimal", "min_scale": 3, "max_scale": 2}));
        refused(json!({"kind": "opaque"}));
        refused(json!({"kind": "array", "element": {"type": {"kind": "float", "exponent_bits": 0, "mantissa_bits": 3}}}));
        // An enum: of a kind written as text, values once each, not of both booleans.
        refused(json!({"kind": "enum", "type": {"kind": "struct", "fields": []}, "values": ["x"]}));
        refused(json!({"kind": "enum", "type": {"kind": "enum", "type": {"kind": "text"}, "values": ["a"]}, "values": ["a"]}));
        refused(json!({"kind": "enum", "type": {"kind": "text"}, "values": ["a", "a"]}));
        refused(json!({"kind": "enum", "type": {"kind": "boolean"}, "values": ["true", "false"]}));
        refused(json!({"kind": "enum", "type": {"kind": "boolean"}, "values": ["yes"]}));
        // Unions and intersections: two members at least, null never one, and no intersection one struct writes.
        refused(json!({"kind": "union", "members": [{"kind": "text"}]}));
        refused(json!({"kind": "union", "members": [{"kind": "text"}, {"kind": "null"}]}));
        refused(json!({"kind": "intersection", "members": [{"kind": "struct", "fields": []}, {"kind": "struct", "fields": []}]}));
        refused(json!({"kind": "intersection", "members": [{"kind": "struct", "fields": []}, {"kind": "map", "key": {"type": {"kind": "text"}}, "value": {"type": {"kind": "text"}}}]}));
        // The null kind stands in a nullable slot alone.
        refused(json!({"kind": "array", "element": {"type": {"kind": "null"}}}));
        refused(json!({"kind": "tuple", "elements": []}));
        refused(json!({"kind": "struct", "fields": [{"name": "a", "default": "1", "type": {"kind": "integer", "bits": 32}}]}));
        refused(json!({"kind": "struct", "fields": [], "extra": {"type": {"kind": "text"}}}));
        let id = json!({"name": "id", "type": {"kind": "integer", "bits": 32}});
        let table = |extra: Value| {
            let mut table = json!({"kind": "table", "cardinality": "many", "columns": [id]});
            table.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            table
        };
        let passes = |value: Value| serde_json::from_value::<Type>(value).unwrap().check().unwrap();
        refused(table(json!({"unique": [["missing"]]})));
        refused(table(json!({"unique": [[]]})));
        refused(table(json!({"cardinality": "at_most_one", "unique": [["id"]]})));
        refused(table(json!({"cardinality": "one", "order": [{"column": "id", "nulls": "last"}]})));
        passes(table(json!({"unique": [["id"]]})));
        refused(table(json!({"order": [{"column": "missing", "nulls": "last"}]})));
        refused(table(json!({"order": [{"column": "id", "nulls": "last"}, {"column": "id", "desc": true, "nulls": "first"}]})));
        passes(table(json!({"order": [{"column": "id", "desc": true, "nulls": "first"}]})));
        // A row has every column.
        refused(json!({"kind": "table", "cardinality": "many", "columns": [{"name": "id", "optional": true, "type": {"kind": "integer", "bits": 32}}]}));
    }

    #[test]
    fn a_statements_result_is_a_table_and_nothing_else() {
        let result = |result: Value| serde_json::from_value::<StatementDescriptor>(json!({"$schema": SCHEMA, "dialect": {"name": "postgres", "version": "18"}, "sql": "select 1", "params": [], "result": result, "diagnostics": []}));
        assert!(result(json!({"kind": "integer", "bits": 32})).is_err());
        assert!(result(json!({"kind": "array", "element": {"type": {"kind": "struct", "fields": []}}})).is_err());
        let rows = json!({"kind": "table", "cardinality": "one", "columns": []});
        let read = result(rows.clone()).unwrap();
        assert_eq!(read.result.table().cardinality, Cardinality::One);
        assert_eq!(serde_json::to_value(&read).unwrap()["result"], rows);
        // A table of one row, or of none, keeps no keys and no order.
        let id = Field::new("id", Type::integer(32), false);
        let key = vec![vec!["id".to_string()]];
        let one = ResultRows::new(Cardinality::One, vec![id.clone()], key.clone(), Vec::new());
        assert!(one.table().unique.is_empty());
        assert_eq!(ResultRows::new(Cardinality::Many, vec![id], key.clone(), Vec::new()).table().unique, key);
    }

    #[test]
    fn the_schema_url_is_the_one_the_spec_names() {
        let spec: serde_json::Value = serde_json::from_str(include_str!("../../../spec/typedefs.schema.json")).unwrap();
        assert_eq!(spec["$id"], SCHEMA);
        assert_eq!(spec["$defs"]["StatementDescriptor"]["properties"]["$schema"]["const"], SCHEMA);
    }

    /// The character sets the format accepts are the ones its JSON Schema lists, and its kinds the ones `Type` lists.
    #[test]
    fn the_names_are_the_ones_the_schema_lists() {
        let spec: serde_json::Value = serde_json::from_str(include_str!("../../../spec/typedefs.schema.json")).unwrap();
        let listed: Vec<&str> = spec["$defs"]["Charset"]["enum"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(listed, CHARSETS);
        let kinds: Vec<String> = spec["$defs"]["Type"]["anyOf"].as_array().unwrap().iter().map(|r| r["$ref"].as_str().unwrap().rsplit('/').next().unwrap().to_string()).collect();
        assert_eq!(kinds.len(), 30);
    }
}
