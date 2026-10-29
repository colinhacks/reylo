//! A DuckDB logical type as the engine's [`Type`]: a scalar by DuckDB's own name for it, lowercased
//! (`integer`, `hugeint`, `timestamptz`), which the loader's table of representations reads
//! (src/values.ts); a `LIST` or an `ARRAY` as an array whose elements may be NULL; a `STRUCT`, a
//! `MAP` and a `UNION` as themselves; an `ENUM` as its labels; a `VARCHAR` with the alias `JSON`
//! as JSON. A type the table does not name (`GEOMETRY`, a type an extension adds) is unknown.

use crate::api::LogicalType;
use reylo_engine_core::types::{Field, Type};

/// The engine's type of a DuckDB logical type.
pub fn of(t: &LogicalType) -> Type {
    if t.alias.as_deref().is_some_and(|a| a.eq_ignore_ascii_case("json")) {
        return Type::Json { schema: None };
    }
    let scalar = |name: &str| Type::Scalar { name: name.to_string() };
    match t.id.as_str() {
        "BOOLEAN" => scalar("boolean"),
        "TINYINT" => scalar("tinyint"),
        "SMALLINT" => scalar("smallint"),
        "INTEGER" => scalar("integer"),
        "BIGINT" => scalar("bigint"),
        "HUGEINT" => scalar("hugeint"),
        "UTINYINT" => scalar("utinyint"),
        "USMALLINT" => scalar("usmallint"),
        "UINTEGER" => scalar("uinteger"),
        "UBIGINT" => scalar("ubigint"),
        "UHUGEINT" => scalar("uhugeint"),
        "BIGNUM" | "VARINT" => scalar("bignum"),
        "FLOAT" => scalar("float"),
        "DOUBLE" => scalar("double"),
        "DECIMAL" => scalar("decimal"),
        "VARCHAR" => scalar("varchar"),
        "BLOB" => scalar("blob"),
        "BIT" => scalar("bit"),
        "UUID" => scalar("uuid"),
        "DATE" => scalar("date"),
        "TIME" => scalar("time"),
        "TIME_NS" => scalar("time_ns"),
        "TIME_TZ" => scalar("timetz"),
        "TIMESTAMP" => scalar("timestamp"),
        "TIMESTAMP_S" => scalar("timestamp_s"),
        "TIMESTAMP_MS" => scalar("timestamp_ms"),
        "TIMESTAMP_NS" => scalar("timestamp_ns"),
        "TIMESTAMP_TZ" => scalar("timestamptz"),
        "TIMESTAMP_TZ_NS" => scalar("timestamptz_ns"),
        "INTERVAL" => scalar("interval"),
        "ENUM" => Type::Enum { name: t.text.clone(), labels: t.values.clone() },
        "LIST" | "ARRAY" => match &t.child {
            Some(child) => Type::Array { of: Box::new(of(child)), nullable_items: None, delimiter: None },
            None => Type::Unknown,
        },
        // A struct whose fields have no names is a tuple.
        "STRUCT" if !t.fields.is_empty() && t.fields.iter().all(|f| f.name.is_empty()) => Type::Tuple { items: t.fields.iter().map(|f| of(&f.ty)).collect() },
        "STRUCT" => Type::Struct { fields: t.fields.iter().map(|f| Field { name: f.name.clone(), ty: of(&f.ty), nullable: true }).collect() },
        "MAP" => match (&t.key, &t.value) {
            (Some(key), Some(value)) => Type::Map { key: Box::new(of(key)), value: Box::new(of(value)) },
            _ => Type::Unknown,
        },
        "UNION" => Type::Union { members: t.fields.iter().map(|f| Field { name: f.name.clone(), ty: of(&f.ty), nullable: true }).collect() },
        _ => Type::Unknown,
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
        assert_eq!(of(&t(r#"{"id":"HUGEINT"}"#)), Type::Scalar { name: "hugeint".into() });
        assert_eq!(of(&t(r#"{"id":"VARCHAR","alias":"JSON"}"#)), Type::Json { schema: None });
        assert_eq!(of(&t(r#"{"id":"LIST","child":{"id":"VARCHAR"}}"#)), Type::Array { of: Box::new(Type::Scalar { name: "varchar".into() }), nullable_items: None, delimiter: None });
        let s = of(&t(r#"{"id":"STRUCT","fields":[{"name":"host","type":{"id":"VARCHAR"}},{"name":"pid","type":{"id":"INTEGER"}}]}"#));
        assert!(matches!(&s, Type::Struct { fields } if fields.len() == 2 && fields.iter().all(|f| f.nullable)));
        assert_eq!(of(&t(r#"{"id":"ENUM","text":"ENUM('a', 'b')","values":["a","b"]}"#)), Type::Enum { name: "ENUM('a', 'b')".into(), labels: vec!["a".into(), "b".into()] });
        assert_eq!(of(&t(r#"{"id":"GEOMETRY"}"#)), Type::Unknown);
        assert_eq!(of(&t(r#"{"id":"STRUCT","fields":[{"name":"","type":{"id":"INTEGER"}},{"name":"","type":{"id":"VARCHAR"}}]}"#)), Type::Tuple { items: vec![Type::Scalar { name: "integer".into() }, Type::Scalar { name: "varchar".into() }] });
    }
}
