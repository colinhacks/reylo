//! A DuckDB logical type as the format's type (`spec/index.ts`), under DuckDB's own name
//! for it, lowercased (`integer`, `hugeint`, `timestamptz`): a `LIST` or an `ARRAY` as an array
//! whose elements may be NULL, a `STRUCT` as a record whose fields may each be NULL (DuckDB has no
//! NOT NULL inside one; a struct whose fields have no names, a tuple, is no type the format has,
//! and the analysis refuses it, [`unnamed`]), a
//! `MAP` as a map, a `UNION` as a tagged union of its alternatives, an `ENUM` as
//! its labels, and a `VARCHAR` with the alias `JSON` as JSON. A type DuckDB has and the format does
//! not structure (`GEOMETRY`, a type an extension adds) is opaque.

use crate::api::LogicalType;
use reylo_engine_core::types::{
    Alternative, ArrayType, Bounds, DateTimeType, DateType, DecimalType, DurationType, Field, FloatType, InstantType, IntegerType, Kind, MapType, Part, Position, Size, TimePart, TimeType, Type, Zone, MICROSECOND,
};

/// DuckDB's earliest and latest dates and timestamps, the latter at microseconds (a
/// nanosecond's range is narrower, and inside it).
const DATE_MIN: &str = "-5877641-06-25";
const DATE_MAX: &str = "+5881580-07-10";
const STAMP_MIN: &str = "-290308-12-22T00:00:00";
const STAMP_MAX: &str = "+294247-01-10T04:00:54.775806";

fn integer(bits: u32, unsigned: bool) -> Type {
    Type::new(Kind::Integer(IntegerType { bits, unsigned, ..IntegerType::default() }))
}

/// DuckDB's floats hold infinity, -infinity and NaN.
fn float(exponent_bits: u32, mantissa_bits: u32) -> Type {
    Type::new(Kind::Float(FloatType { exponent_bits, mantissa_bits, infinity: true, nan: true, ..FloatType::default() }))
}

fn datetime(tick: &str) -> Type {
    Type::new(Kind::DateTime(DateTimeType { tick: tick.into(), bounds: Bounds::between(STAMP_MIN, STAMP_MAX), infinity: true }))
}

fn instant(tick: &str) -> Type {
    Type::new(Kind::Instant(InstantType { tick: tick.into(), zone: None, bounds: Bounds::between(format!("{STAMP_MIN}Z"), format!("{STAMP_MAX}Z")), infinity: true }))
}

fn time(tick: &str, zone: Option<Zone>) -> Type {
    Type::new(Kind::Time(TimeType { tick: tick.into(), zone, bounds: Bounds { max: Some("24:00:00".into()), ..Bounds::default() } }))
}

/// The format's type of a DuckDB logical type.
pub fn of(t: &LogicalType) -> Type {
    if t.alias.as_deref().is_some_and(|a| a.eq_ignore_ascii_case("json")) {
        return Type::json().named("json");
    }
    let ty = match t.id.as_str() {
        "BOOLEAN" => Type::boolean(),
        "TINYINT" => integer(8, false),
        "SMALLINT" => integer(16, false),
        "INTEGER" => integer(32, false),
        "BIGINT" => integer(64, false),
        "HUGEINT" => integer(128, false),
        "UTINYINT" => integer(8, true),
        "USMALLINT" => integer(16, true),
        "UINTEGER" => integer(32, true),
        "UBIGINT" => integer(64, true),
        "UHUGEINT" => integer(128, true),
        // An integer of no fixed width.
        "BIGNUM" | "VARINT" => Type::decimal(DecimalType { max_scale: Some(0), ..DecimalType::default() }).named("bignum"),
        "FLOAT" => float(8, 23),
        "DOUBLE" => float(11, 52),
        "DECIMAL" => {
            let scale = t.scale.map(|s| s as i32);
            Type::decimal(DecimalType { precision: t.width.map(u32::from), min_scale: scale, max_scale: scale, ..DecimalType::default() })
        }
        "VARCHAR" => Type::text(),
        "BLOB" => Type::bytes(),
        "BIT" => Type::array(Position::non_null(Type::boolean())),
        "UUID" => Type::uuid(),
        "DATE" => Type::new(Kind::Date(DateType { bounds: Bounds::between(DATE_MIN, DATE_MAX), infinity: true })),
        "TIME" => time(MICROSECOND, None),
        "TIME_NS" => time("1/1000000000", None),
        "TIME_TZ" => time(MICROSECOND, Some(Zone::Offset)),
        "TIMESTAMP" => datetime(MICROSECOND),
        "TIMESTAMP_S" => datetime("1"),
        "TIMESTAMP_MS" => datetime("1/1000"),
        "TIMESTAMP_NS" => datetime("1/1000000000"),
        "TIMESTAMP_TZ" => instant(MICROSECOND),
        "TIMESTAMP_TZ_NS" => instant("1/1000000000"),
        // Months, days and microseconds, each its own signed integer.
        "INTERVAL" => Type::new(Kind::Duration(DurationType {
            months: Some(Part { max: Some("P2147483648M".into()) }),
            days: Some(Part { max: Some("P2147483648D".into()) }),
            time: Some(TimePart { tick: MICROSECOND.into(), max: Some("PT2562047788H54.775808S".into()) }),
            infinity: false,
        })),
        "ENUM" => Type::text_of(t.values.clone()),
        "LIST" => match &t.child {
            Some(child) => return Type::array(Position::nullable(of(child))),
            None => return Type::unknown(),
        },
        "ARRAY" => match &t.child {
            Some(child) => return Type::new(Kind::Array(ArrayType { element: Box::new(Position::nullable(of(child))), size: t.size.map(|n| Size::exact(n.into())), any_depth: false })),
            None => return Type::unknown(),
        },
        "STRUCT" if t.fields.iter().any(|f| f.name.is_empty()) => return Type::unknown(),
        "STRUCT" => return Type::record(t.fields.iter().map(|f| Field::new(f.name.clone(), of(&f.ty), true)).collect()),
        "MAP" => match (&t.key, &t.value) {
            (Some(key), Some(value)) => return Type::new(Kind::Map(MapType { key: Box::new(of(key)), value: Box::new(Position::nullable(of(value))) })),
            _ => return Type::unknown(),
        },
        // One of its alternatives, by the tag every value carries, whose value may be NULL, as any
        // value inside a nested type may be: one alternative alone stays a tagged union.
        "UNION" => return Type::tagged_union(t.fields.iter().map(|f| Alternative { tag: f.name.clone(), ty: of(&f.ty), nullable: true }).collect()).named("union"),
        "" => return Type::unknown(),
        other => return Type::opaque(other.to_ascii_lowercase()),
    };
    ty.named(lowercase_name(&t.id))
}

/// Whether a type is or holds a struct whose fields have no names, as `(1, 'a')` and `row(…)`
/// build: a record's fields are read by their names, so the format has no type for one.
pub fn unnamed(t: &LogicalType) -> bool {
    (t.id == "STRUCT" && t.fields.iter().any(|f| f.name.is_empty()))
        || t.fields.iter().any(|f| unnamed(&f.ty))
        || [&t.child, &t.key, &t.value].into_iter().any(|inner| inner.as_deref().is_some_and(unnamed))
}

/// DuckDB's name for a type, lowercased, as its id writes it (`timestamptz` for `TIMESTAMP_TZ`).
fn lowercase_name(id: &str) -> String {
    match id {
        "TIME_TZ" => "timetz".into(),
        "TIMESTAMP_TZ" => "timestamptz".into(),
        "TIMESTAMP_TZ_NS" => "timestamptz_ns".into(),
        "VARINT" => "bignum".into(),
        other => other.to_ascii_lowercase(),
    }
}

/// The type as DuckDB names it, for messages: its alias (`JSON`), else its text, else its id.
pub fn name(t: &LogicalType) -> String {
    if let Some(alias) = t.alias.as_ref().filter(|a| !a.is_empty()) {
        return alias.clone();
    }
    if !t.text.is_empty() {
        t.text.clone()
    } else {
        t.id.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(json: &str) -> LogicalType {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn each_kind_of_type() {
        assert_eq!(of(&t(r#"{"id":"HUGEINT"}"#)).short(), "hugeint");
        assert_eq!(serde_json::to_value(of(&t(r#"{"id":"UINTEGER"}"#))).unwrap(), serde_json::json!({"kind": "integer", "bits": 32, "unsigned": true, "name": "uinteger"}));
        assert_eq!(of(&t(r#"{"id":"DECIMAL","width":10,"scale":2}"#)).short(), "decimal(10,2)");
        assert_eq!(of(&t(r#"{"id":"VARCHAR","alias":"JSON"}"#)), Type::json().named("json"));
        assert_eq!(of(&t(r#"{"id":"LIST","child":{"id":"VARCHAR"}}"#)).short(), "varchar?[]");
        assert_eq!(of(&t(r#"{"id":"ARRAY","size":3,"child":{"id":"INTEGER"}}"#)).short(), "integer?[3]");
        assert_eq!(of(&t(r#"{"id":"STRUCT","fields":[{"name":"host","type":{"id":"VARCHAR"}},{"name":"pid","type":{"id":"INTEGER"}}]}"#)).short(), "{ host: varchar?, pid: integer? }");
        assert_eq!(of(&t(r#"{"id":"ENUM","text":"ENUM('a', 'b')","values":["a","b"]}"#)).short(), "enum('a' | 'b')");
        assert_eq!(of(&t(r#"{"id":"GEOMETRY"}"#)), Type::opaque("geometry"));
        // A tuple's fields have no names, which no type of the format describes, inside a list too.
        let tuple = r#"{"id":"STRUCT","fields":[{"name":"","type":{"id":"INTEGER"}},{"name":"","type":{"id":"VARCHAR"}}]}"#;
        assert!(of(&t(tuple)).is_unknown());
        assert!(unnamed(&t(tuple)) && unnamed(&t(&format!(r#"{{"id":"LIST","child":{tuple}}}"#))));
        assert!(!unnamed(&t(r#"{"id":"STRUCT","fields":[{"name":"host","type":{"id":"VARCHAR"}}]}"#)));
        assert_eq!(of(&t(r#"{"id":"UNION","fields":[{"name":"num","type":{"id":"INTEGER"}},{"name":"str","type":{"id":"VARCHAR"}}]}"#)).short(), "union<num: integer? | str: varchar?>");
        assert_eq!(of(&t(r#"{"id":"MAP","key":{"id":"VARCHAR"},"value":{"id":"INTEGER"}}"#)).short(), "map(varchar => integer?)");
        for id in ["BOOLEAN", "BIGNUM", "FLOAT", "DOUBLE", "DATE", "TIME", "TIME_NS", "TIME_TZ", "TIMESTAMP", "TIMESTAMP_S", "TIMESTAMP_MS", "TIMESTAMP_NS", "TIMESTAMP_TZ", "TIMESTAMP_TZ_NS", "INTERVAL", "UUID", "BIT", "BLOB"] {
            of(&t(&format!(r#"{{"id":"{id}"}}"#))).check().unwrap_or_else(|e| panic!("{id}: {e}"));
        }
    }
}
