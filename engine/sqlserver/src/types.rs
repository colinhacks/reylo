//! SQL Server's types as type descriptors, from the name, length, precision, scale and collation
//! its describe reports for a column or a parameter (`system_type_name`, `max_length`, …), as the
//! format's rules for SQL Server say (spec/README.md): `bit` is a boolean, `tinyint` an unsigned
//! 8-bit integer, `money` a decimal of precision 19 and scale 4, `nvarchar` ill-formed UTF-16 text
//! counted in code units, `datetime2` a wall-clock date and time, `datetime` one of tick 1/300 and
//! `smalldatetime` one of tick 60, `datetimeoffset` a zoned date and time of offsets alone, and
//! `hierarchyid`, `sql_variant` and the spatial types opaque.

use reylo_engine_core::types::{BytesType, Bounds, DateTimeType, DecimalType, Kind, Size, SizeUnit, TextType, TimeType, Type, XmlType, ZonedDateTimeType};

/// What a describe says of a type: the name with its arguments as the server writes it
/// (`decimal(10,2)`, `nvarchar(max)`), its length in bytes (-1 for `max`), precision, scale, and
/// collation for text.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Described {
    pub name: String,
    pub max_length: i64,
    pub precision: u32,
    pub scale: u32,
    pub collation: Option<String>,
}

/// The type a describe reports, labelled with its name without arguments (`decimal`).
pub fn of(d: &Described) -> Type {
    let base = base_name(&d.name);
    let ty = match base.as_str() {
        "bit" => Type::boolean(),
        "tinyint" => Type::unsigned(8),
        "smallint" => Type::integer(16),
        "int" => Type::integer(32),
        "bigint" => Type::integer(64),
        "decimal" | "numeric" => Type::decimal(DecimalType { precision: Some(d.precision), min_scale: Some(d.scale as i32), max_scale: Some(d.scale as i32), ..DecimalType::default() }),
        "money" => money(19, "-922337203685477.5808", "922337203685477.5807"),
        "smallmoney" => money(10, "-214748.3648", "214748.3647"),
        // `float(n)` is a real up to 24 bits of mantissa and a double past it; `float` alone is 53.
        "float" if d.precision > 0 && d.precision <= 24 => Type::single(),
        "float" => Type::double(),
        "real" => Type::single(),
        "char" | "varchar" | "text" => code_page_text(d, base == "char"),
        "nchar" | "nvarchar" | "ntext" | "sysname" => utf16_text(d, base == "nchar"),
        "binary" => bytes(Some(Size::exact(d.max_length.max(0) as u64))),
        "varbinary" if d.max_length >= 0 => bytes(Some(Size::at_most(d.max_length as u64))),
        "varbinary" | "image" => bytes(None),
        // `timestamp` is rowversion's old name: 8 bytes the server writes.
        "timestamp" | "rowversion" => bytes(Some(Size::exact(8))),
        "uniqueidentifier" => Type::uuid(),
        "date" => Type::new(Kind::Date(Default::default())),
        "time" => Type::new(Kind::Time(TimeType { tick: tick(d.scale), ..TimeType::default() })),
        "datetime2" => Type::new(Kind::DateTime(DateTimeType { tick: tick(d.scale), ..DateTimeType::default() })),
        "datetime" => Type::new(Kind::DateTime(DateTimeType { tick: "1/300".into(), bounds: Bounds::between("1753-01-01T00:00:00", "9999-12-31T23:59:59.997"), ..DateTimeType::default() })),
        "smalldatetime" => Type::new(Kind::DateTime(DateTimeType { tick: "60".into(), bounds: Bounds::between("1900-01-01T00:00:00", "2079-06-06T23:59:00"), ..DateTimeType::default() })),
        "datetimeoffset" => Type::new(Kind::ZonedDateTime(ZonedDateTimeType { tick: tick(d.scale), offset_only: true, ..ZonedDateTimeType::default() })),
        // Untyped XML holds a fragment as well as a document.
        "xml" => Type::new(Kind::Xml(XmlType { fragment: true, schemas: None })),
        "json" => Type::json(),
        _ => return Type::opaque(base),
    };
    ty.named(base)
}

/// The type's name without its arguments, lowercased: `decimal` of `decimal(10,2)`.
pub fn base_name(name: &str) -> String {
    name.split('(').next().unwrap_or(name).trim().to_ascii_lowercase()
}

/// The smallest step of a time with `scale` digits after the second, in lowest terms.
fn tick(scale: u32) -> String {
    if scale == 0 {
        "1".into()
    } else {
        format!("1/{}", 10u64.pow(scale.min(9)))
    }
}

fn money(precision: u32, min: &str, max: &str) -> Type {
    Type::decimal(DecimalType { precision: Some(precision), min_scale: Some(4), max_scale: Some(4), bounds: Bounds::between(min, max), ..DecimalType::default() })
}

fn bytes(size: Option<Size>) -> Type {
    Type::new(Kind::Bytes(BytesType { size, media_type: None }))
}

/// `nchar`, `nvarchar` and `ntext`: UTF-16 code units, which may be a lone surrogate, as many as
/// the length's bytes halved; `max` and `ntext` set no bound.
fn utf16_text(d: &Described, fixed: bool) -> Type {
    let units = (d.max_length > 0).then(|| d.max_length as u64 / 2);
    let size = units.map(|n| Size { unit: Some(SizeUnit::CodeUnit), exact: fixed.then_some(n), max: (!fixed).then_some(n), min: None });
    Type::new(Kind::Text(TextType { encoding: Some("UTF-16".into()), ill_formed: true, size, ..TextType::default() }))
}

/// `char`, `varchar` and `text`: bytes in the collation's code page, UTF-8 for a `_UTF8`
/// collation. A collation whose code page is not one the format names leaves the encoding out.
fn code_page_text(d: &Described, fixed: bool) -> Type {
    let encoding = d.collation.as_deref().and_then(code_page);
    let size = (d.max_length > 0 && encoding.is_some()).then(|| {
        let n = d.max_length as u64;
        Size { unit: Some(SizeUnit::Byte), exact: fixed.then_some(n), max: (!fixed).then_some(n), min: None }
    });
    Type::new(Kind::Text(TextType { encoding: encoding.map(String::from), size, ..TextType::default() }))
}

/// The character set of a collation's code page, by the collation's name: a `_UTF8` collation is
/// UTF-8, and the Latin1 collations (Windows' and the SQL ones of code page 1252) windows-1252.
fn code_page(collation: &str) -> Option<&'static str> {
    let c = collation.to_ascii_lowercase();
    if c.contains("_utf8") {
        Some("UTF-8")
    } else if c.starts_with("latin1_general") || c.starts_with("sql_latin1_general_cp1_") {
        Some("windows-1252")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn typed(name: &str, max_length: i64, precision: u32, scale: u32, collation: Option<&str>) -> serde_json::Value {
        serde_json::to_value(of(&Described { name: name.into(), max_length, precision, scale, collation: collation.map(String::from) })).unwrap()
    }

    #[test]
    fn the_types_as_the_format_writes_them() {
        assert_eq!(typed("int", 4, 10, 0, None), json!({"kind": "integer", "bits": 32, "label": "int"}));
        assert_eq!(typed("tinyint", 1, 3, 0, None), json!({"kind": "integer", "bits": 8, "unsigned": true, "label": "tinyint"}));
        assert_eq!(typed("decimal(10,2)", 9, 10, 2, None), json!({"kind": "decimal", "precision": 10, "min_scale": 2, "max_scale": 2, "label": "decimal"}));
        assert_eq!(typed("money", 8, 19, 4, None), json!({"kind": "decimal", "precision": 19, "min_scale": 4, "max_scale": 4, "min": "-922337203685477.5808", "max": "922337203685477.5807", "label": "money"}));
        assert_eq!(typed("nvarchar(10)", 20, 0, 0, Some("SQL_Latin1_General_CP1_CI_AS")), json!({"kind": "text", "encoding": "UTF-16", "ill_formed": true, "size": {"unit": "code_unit", "max": 10}, "label": "nvarchar"}));
        assert_eq!(typed("nvarchar(max)", -1, 0, 0, None), json!({"kind": "text", "encoding": "UTF-16", "ill_formed": true, "label": "nvarchar"}));
        assert_eq!(typed("varchar(20)", 20, 0, 0, Some("Latin1_General_100_CI_AS_SC_UTF8")), json!({"kind": "text", "encoding": "UTF-8", "size": {"unit": "byte", "max": 20}, "label": "varchar"}));
        assert_eq!(typed("varchar(20)", 20, 0, 0, Some("Japanese_CI_AS")), json!({"kind": "text", "label": "varchar"}));
        assert_eq!(typed("datetime2(3)", 7, 23, 3, None), json!({"kind": "datetime", "tick": "1/1000", "label": "datetime2"}));
        assert_eq!(typed("datetimeoffset(7)", 10, 34, 7, None), json!({"kind": "zoned_datetime", "tick": "1/10000000", "offset_only": true, "label": "datetimeoffset"}));
        assert_eq!(typed("time(0)", 3, 8, 0, None), json!({"kind": "time", "tick": "1", "label": "time"}));
        assert_eq!(typed("binary(8)", 8, 0, 0, None), json!({"kind": "bytes", "size": {"exact": 8}, "label": "binary"}));
        assert_eq!(typed("uniqueidentifier", 16, 0, 0, None), json!({"kind": "uuid", "label": "uniqueidentifier"}));
        assert_eq!(typed("hierarchyid", 892, 0, 0, None), json!({"kind": "opaque", "label": "hierarchyid"}));
        assert_eq!(typed("float", 8, 53, 0, None)["mantissa_bits"], json!(52));
    }
}
