//! MySQL's types as the format describes them (`spec/index.ts`): what a prepare's protocol
//! type code, flags, character set and display length say of a value, under MySQL's own name for
//! the type (`bigint`, `tinyint(1)`, `datetime`).

use crate::api::MysqlColumn;
use crate::types::{ArrayType, BytesType, Bounds, DateTimeType, DateType, DecimalType, DurationType, InstantType, IntegerType, Kind, Position, SetType, Size, TextType, Type, MICROSECOND};

/// The protocol's flag for an UNSIGNED integer column.
const UNSIGNED: u32 = 32;
/// The protocol's flags for an ENUM and a SET column, which come as strings.
const ENUM: u32 = 256;
const SET: u32 = 2048;
/// The character set of binary strings.
pub(crate) const BINARY: u32 = 63;

/// A whole number of `bits`, unsigned where the flags say so.
fn integer(bits: u32, flags: u32, name: &str) -> Type {
    Type::new(Kind::Integer(IntegerType { bits, unsigned: flags & UNSIGNED != 0, ..IntegerType::default() })).named(name)
}

/// MySQL's `SET`: each of its members once, as text.
pub(crate) fn set() -> Type {
    set_of(None)
}

/// MySQL's `ENUM`: one of its labels, where they are known, else text.
pub(crate) fn enumeration(labels: Option<Vec<String>>) -> Type {
    labels.map_or_else(Type::text, Type::text_of).named("enum")
}

/// MySQL's `SET`: each of its members once, one of its labels where they are known.
pub(crate) fn set_of(labels: Option<Vec<String>>) -> Type {
    let member = labels.map_or_else(Type::text, Type::text_of);
    Type::new(Kind::Set(SetType { element: Box::new(Position::non_null(member)), size: None, duplicates: false })).named("set")
}

/// The labels of an `ENUM` or a `SET` column as `information_schema.columns` writes its
/// `column_type`: `enum('it''s','a\\b')`, each label quoted, a quote inside it doubled and a
/// backslash escaped. None for text of any other form.
pub(crate) fn labels(column_type: &str) -> Option<Vec<String>> {
    let open = column_type.find('(')?;
    let body = column_type[open + 1..].strip_suffix(')')?;
    let mut labels = Vec::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {}
            ',' => continue,
            _ => return None,
        }
        let mut label = String::new();
        loop {
            match chars.next()? {
                '\'' if chars.peek() == Some(&'\'') => {
                    chars.next();
                    label.push('\'');
                }
                '\'' => break,
                '\\' => label.push(chars.next()?),
                c => label.push(c),
            }
        }
        labels.push(label);
    }
    Some(labels)
}

/// MySQL's `BIT(n)`, a string of `n` bits: exactly `n` where the length is known.
pub(crate) fn bits(length: Option<u32>) -> Type {
    Type::new(Kind::Array(ArrayType { element: Box::new(Position::non_null(Type::boolean())), size: length.map(|n| Size::exact(n.into())), any_depth: false })).named("bit")
}

/// A `DATETIME`: wall-clock time with no zone.
pub(crate) fn datetime() -> Type {
    Type::new(Kind::DateTime(DateTimeType { tick: MICROSECOND.into(), ..DateTimeType::default() })).named("datetime")
}

pub(crate) fn date() -> Type {
    Type::new(Kind::Date(DateType::default())).named("date")
}

/// A `TIME`, which is a length of time, up to 838:59:59 either way.
pub(crate) fn time() -> Type {
    Type::new(Kind::Duration(DurationType { max: "PT838H59M59S".into(), tick: Some(MICROSECOND.into()), infinity: false })).named("time")
}

pub(crate) fn bigint() -> Type {
    integer(64, 0, "bigint")
}

pub(crate) fn double() -> Type {
    Type::double().named("double")
}

pub(crate) fn decimal() -> Type {
    Type::decimal(Default::default()).named("decimal")
}

/// The value a protocol type code describes, with the flags, the character set and the display
/// length beside it. A `TINYINT` one digit wide is `tinyint(1)`, MySQL's `BOOLEAN`: an integer
/// still, which a client may read as a truth value by its name. MySQL refuses NaN and the
/// infinities in a float, and a `TIMESTAMP` is an instant from 1970 to 2038.
pub(crate) fn of(code: u32, flags: u32, charset: u32, length: Option<u32>) -> Type {
    match code {
        1 => integer(8, flags, if length == Some(1) { "tinyint(1)" } else { "tinyint" }),
        2 => integer(16, flags, "smallint"),
        9 => integer(24, flags, "mediumint"),
        3 => integer(32, flags, "int"),
        8 => integer(64, flags, "bigint"),
        // 0 and 1901 to 2155.
        13 => Type::new(Kind::Integer(IntegerType { bits: 16, bounds: Bounds::between("0", "2155"), ..IntegerType::default() })).named("year"),
        4 => Type::single().named("float"),
        5 => double(),
        0 | 246 => decimal(),
        7 => Type::new(Kind::Instant(InstantType { tick: MICROSECOND.into(), zone: None, bounds: Bounds::between("1970-01-01T00:00:01Z", "2038-01-19T03:14:07.999999Z"), infinity: false })).named("timestamp"),
        12 => datetime(),
        10 | 14 => date(),
        11 => time(),
        245 => Type::json().named("json"),
        // MySQL 9's VECTOR: 32-bit floats, none of them NULL.
        242 => Type::array(Position::non_null(Type::single())).named("vector"),
        16 => bits(length),
        249..=252 if charset == BINARY => Type::bytes().named("blob"),
        254 if charset == BINARY => Type::bytes().named("binary"),
        253 | 15 if charset == BINARY => Type::bytes().named("varbinary"),
        254 if flags & ENUM != 0 => Type::text().named("enum"),
        254 if flags & SET != 0 => set(),
        254 => Type::text().named("char"),
        253 | 15 => Type::text().named("varchar"),
        249..=252 => Type::text().named("text"),
        255 => Type::new(Kind::Geometry(Default::default())).named("geometry"),
        _ => Type::unknown(),
    }
}

/// What a table's own column's definition narrows its type to, as the prepare reports it: a
/// string's length, a DECIMAL's precision and scale, and a DATETIME's, TIMESTAMP's or TIME's
/// fractional seconds. Only a column read straight from a table holds to them: MySQL reports a
/// length and decimals for an expression too, which may be a display width rather than a bound
/// on its values (`group_concat` under the session's `group_concat_max_len`, a placeholder's
/// text), so the caller passes a table's column alone.
///
///   - `CHAR(n)` and `VARCHAR(n)` hold at most n code points: the protocol counts n characters
///     of the character set the column is sent in at its widest (`varchar(20)` sent as utf8mb4
///     is 80), and MySQL strips the spaces a `CHAR` pads with as it reads one, so neither is
///     exactly n.
///   - A `TEXT` holds at most its capacity in bytes (255, 65535, 16777215, 4294967295), so as
///     many code points; `utf8` says the column stores UTF-8 (`utf8mb4`, `utf8mb3`), whose bytes
///     are a client's too. Sent in another character set than it is stored in, a `TEXT` reports
///     its capacity over its narrowest character times the widest of the one it is sent in, which
///     tells the four capacities apart whatever the two are.
///   - `BINARY(n)` is exactly n bytes, which MySQL pads with zero bytes and keeps; `VARBINARY(n)`
///     at most n; a `BLOB` at most its capacity.
///   - `DECIMAL(p,s)` has precision p and scale s: the length counts p digits, the point where s
///     is above 0, and the sign unless UNSIGNED.
///   - A time of p fractional digits ticks by 10^-p, and a `TIMESTAMP`'s latest value is cut to
///     them (`2038-01-19T03:14:07.999Z` at 3).
pub(crate) fn declared(ty: Type, c: &MysqlColumn, utf8: bool) -> Type {
    let (length, decimals) = (c.length, c.decimals.filter(|&d| d <= 30));
    let fraction = decimals.filter(|&d| d <= 6);
    let kind = match (ty.kind, ty.label.as_deref()) {
        (Kind::Text(text), Some("char" | "varchar")) => {
            let per = max_bytes(c.charset);
            let n = length.zip(per).filter(|(length, per)| length % per == 0).map(|(length, per)| u64::from(length / per));
            Kind::Text(TextType { size: n.map(Size::at_most), ..text })
        }
        (Kind::Text(text), Some("text")) => {
            let size = length.and_then(capacity).map(|n| if utf8 { Size::bytes_at_most(n, "UTF-8") } else { Size::at_most(n) });
            Kind::Text(TextType { size, ..text })
        }
        (Kind::Bytes(_), Some("binary")) => Kind::Bytes(BytesType { size: length.map(|n| Size::exact(n.into())) }),
        (Kind::Bytes(_), Some("varbinary")) => Kind::Bytes(BytesType { size: length.map(|n| Size::at_most(n.into())) }),
        (Kind::Bytes(_), Some("blob")) => Kind::Bytes(BytesType { size: length.and_then(capacity).map(Size::at_most) }),
        (Kind::Decimal(decimal), _) => {
            let signed = c.flags & UNSIGNED == 0;
            let precision = length.zip(decimals).and_then(|(length, scale)| length.checked_sub(u32::from(scale > 0) + u32::from(signed)).filter(|&p| p >= scale.max(1) && p <= 65).map(|p| (p, scale)));
            match precision {
                Some((precision, scale)) => Kind::Decimal(DecimalType { precision: Some(precision), min_scale: Some(scale as i32), max_scale: Some(scale as i32), ..decimal }),
                None => Kind::Decimal(decimal),
            }
        }
        (Kind::DateTime(dt), _) => Kind::DateTime(DateTimeType { tick: fraction.map_or(dt.tick, tick), ..dt }),
        (Kind::Instant(instant), _) => match fraction {
            Some(digits) => Kind::Instant(InstantType { tick: tick(digits), bounds: Bounds { max: instant.bounds.max.map(|max| cut(&max, digits)), ..instant.bounds }, ..instant }),
            None => Kind::Instant(instant),
        },
        (Kind::Duration(duration), _) => Kind::Duration(DurationType { tick: fraction.map(tick).or(duration.tick), ..duration }),
        (kind, _) => kind,
    };
    Type { kind, ..ty }
}

/// The smallest step of a time of `digits` fractional digits, in lowest terms: `"1/1000"` for 3,
/// `"1"` for none.
fn tick(digits: u32) -> String {
    match digits {
        0 => "1".to_string(),
        d => format!("1/{}", 10u64.pow(d)),
    }
}

/// An instant's text with its fraction cut to `digits` (`2038-01-19T03:14:07.999999Z` to
/// `…07.999Z` at 3, `…07Z` at 0).
fn cut(text: &str, digits: u32) -> String {
    match text.split_once('.') {
        Some((whole, fraction)) => {
            let zone = fraction.trim_start_matches(|c: char| c.is_ascii_digit());
            let kept = &fraction[..(digits as usize).min(fraction.len() - zone.len())];
            if kept.is_empty() { format!("{whole}{zone}") } else { format!("{whole}.{kept}{zone}") }
        }
        None => text.to_string(),
    }
}

/// The bytes a `TEXT` or a `BLOB` holds by the length the protocol reports for it: a `BLOB`'s is
/// its capacity, and a `TEXT`'s its capacity over the narrowest character of the character set it
/// is stored in times the widest of the one it is sent in, each 1 to 4 bytes, which keeps the
/// four apart. `None` for a length none of them reports.
fn capacity(length: u32) -> Option<u64> {
    match length {
        63..=1020 => Some(255),
        16_383..=262_140 => Some(65_535),
        4_194_303..=67_108_860 => Some(16_777_215),
        1_073_741_823..=u32::MAX => Some(4_294_967_295),
        _ => None,
    }
}

/// The most bytes one character takes in the character set a collation number names, as MySQL
/// 8.4.11 numbers them (`information_schema.collations`, with each one's `character_sets.maxlen`):
/// what a string column's length is counted in. `None` for a number MySQL 8.4 does not give.
pub(crate) fn max_bytes(collation: u32) -> Option<u32> {
    Some(match collation {
        // utf8mb4, utf16, utf16le, utf32 and gb18030.
        45 | 46 | 224..=247 | 255..=271 | 273..=275 | 277..=294 | 296..=298 | 300 | 303..=323 | 54..=56 | 62 | 101..=124 | 60 | 61 | 160..=183 | 248..=250 => 4,
        // utf8mb3, ujis and eucjpms.
        33 | 76 | 83 | 192..=215 | 223 | 12 | 91 | 97 | 98 => 3,
        // ucs2, big5, sjis, euckr, gb2312, gbk and cp932.
        35 | 90 | 128..=151 | 159 | 1 | 84 | 13 | 88 | 19 | 85 | 24 | 86 | 28 | 87 | 95 | 96 => 2,
        // binary, latin1, ascii and every other single-byte set.
        2..=11 | 14..=16 | 18 | 20..=23 | 25..=27 | 29..=32 | 34 | 36..=44 | 47..=53 | 57..=59 | 63..=75 | 77..=82 | 89 | 92..=94 | 99 => 1,
        _ => return None,
    })
}

/// A value one digit or one bit wide that is not a table's own column: an expression's or a view's
/// width says nothing of its values, so a `tinyint(1)` is a TINYINT and a `BIT(1)` bits of no
/// known number.
pub(crate) fn unflagged(ty: Type) -> Type {
    match (&ty.kind, ty.label.as_deref()) {
        (Kind::Integer(_), Some("tinyint(1)")) => Type { label: Some("tinyint".into()), ..ty },
        (Kind::Array(_), Some("bit")) => bits(None),
        _ => ty,
    }
}

/// The types a value of type `ty` may also have where a placeholder reaches it, as the execution
/// decides: a number is a `BIGINT`, a `DOUBLE` or a `DECIMAL` by what is bound, and a date a
/// `DATE`, a `DATETIME`, a `TIME` or, from a string, a string. `string`: a placeholder that takes a
/// string may be handed back as it is. `None` where nothing else is possible.
pub(crate) fn widened(ty: &Type, string: bool) -> Option<Type> {
    let others = match &ty.kind {
        Kind::Date(_) | Kind::DateTime(_) | Kind::Instant(_) => vec![date(), datetime(), time(), Type::text()],
        Kind::Integer(_) | Kind::Decimal(_) | Kind::Float(_) => {
            let mut numbers = vec![bigint(), double(), decimal()];
            if string {
                numbers.push(Type::text());
            }
            numbers
        }
        _ => return None,
    };
    let mut members = vec![ty.clone()];
    members.extend(others.into_iter().filter(|o| o != ty));
    let either = Type::union(members);
    (either != *ty).then_some(either)
}

/// A JSON document of `content`, what a shape resolves to inside one: a document as it is, and
/// anything else, an object, an array or a SQL value, the content of a document that holds it.
pub(crate) fn document(content: Type) -> Type {
    if content.is_json() { content } else { Type::json_of(Position::non_null(content)) }
}

/// What a document holds, as a value inside another document: the content a statement built, or
/// the document itself.
pub(crate) fn content(ty: Type) -> Type {
    match ty.kind {
        Kind::Json(crate::types::JsonType { content: Some(content), .. }) => content.ty,
        kind => Type { kind, ..ty },
    }
}

/// Whether a type is a whole number, or a union one of whose members is.
pub(crate) fn has_integer(ty: &Type) -> bool {
    ty.members().iter().any(|m| matches!(m.kind, Kind::Integer(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_protocol_type_is_its_values_under_mysqls_name() {
        assert_eq!(of(8, UNSIGNED, 0, Some(20)).short(), "bigint");
        assert_eq!(serde_json::to_value(of(8, UNSIGNED, 0, Some(20))).unwrap(), serde_json::json!({"kind": "integer", "bits": 64, "unsigned": true, "label": "bigint"}));
        assert_eq!(of(1, 0, 0, Some(1)).short(), "tinyint(1)");
        assert_eq!(of(1, 0, 0, Some(4)).short(), "tinyint");
        assert_eq!(of(13, 0, 0, None).short(), "year(0..2155)");
        assert_eq!(of(254, ENUM, 255, None).short(), "enum");
        assert_eq!(of(254, SET, 255, None).short(), "set{text}");
        assert_eq!(serde_json::to_value(enumeration(labels("enum('x','y')"))).unwrap(), serde_json::json!({"kind": "text", "enum": ["x", "y"], "label": "enum"}));
        assert_eq!(serde_json::to_value(set_of(labels("set('a','b')"))).unwrap(), serde_json::json!({"kind": "set", "element": {"type": {"kind": "text", "enum": ["a", "b"]}}, "label": "set"}));
        assert_eq!(of(253, 0, BINARY, None).short(), "varbinary");
        assert_eq!(of(16, 0, 0, Some(7)).short(), "bit(boolean[7])");
        assert_eq!(unflagged(of(1, UNSIGNED, 0, Some(1))).short(), "tinyint");
        assert_eq!(unflagged(of(16, 0, 0, Some(1))).short(), "bit(boolean[])");
        assert_eq!(of(7, 0, 0, None).kind.name(), "instant");
        assert_eq!(of(12, 0, 0, None).kind.name(), "datetime");
        for code in [0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 242, 245, 246, 249, 250, 251, 252, 253, 254, 255] {
            of(code, 0, 0, Some(5)).check().unwrap_or_else(|e| panic!("{code}: {e}"));
        }
    }

    /// Each column as MySQL 8.4.11's prepare reports it, sent as utf8mb4 (collation 255): its
    /// protocol type, flags, character set, length and decimals.
    #[test]
    fn a_tables_column_is_narrowed_by_its_definition() {
        let column = |r#type: u32, flags: u32, charset: u32, length: u32, decimals: u32| MysqlColumn { name: "c".into(), r#type, flags, charset, length: Some(length), decimals: Some(decimals), schema: Some("app".into()), org_table: Some("t".into()), org_name: Some("c".into()) };
        let typed = |c: MysqlColumn, utf8: bool| serde_json::to_value(declared(of(c.r#type, c.flags, c.charset, c.length), &c, utf8)).unwrap();
        // varchar(20) and char(5), whatever character set they are stored in; utf8mb3 and latin1 sent as themselves.
        assert_eq!(typed(column(253, 0, 255, 80, 0), false), serde_json::json!({"kind": "text", "size": {"max": 20}, "label": "varchar"}));
        assert_eq!(typed(column(254, 0, 255, 20, 0), false), serde_json::json!({"kind": "text", "size": {"max": 5}, "label": "char"}));
        assert_eq!(typed(column(253, 0, 33, 60, 0), false)["size"], serde_json::json!({"max": 20}));
        assert_eq!(typed(column(253, 0, 8, 20, 0), false)["size"], serde_json::json!({"max": 20}));
        // A length no character set's width divides, or a character set MySQL does not number, says nothing.
        assert_eq!(typed(column(253, 0, 255, 81, 0), false).get("size"), None);
        assert_eq!(typed(column(253, 0, 2047, 80, 0), false).get("size"), None);
        // TINYTEXT, TEXT, MEDIUMTEXT and LONGTEXT: bytes where the column stores UTF-8, else code points.
        assert_eq!(typed(column(252, 16, 255, 262_140, 0), true), serde_json::json!({"kind": "text", "size": {"unit": "byte", "encoding": "UTF-8", "max": 65535}, "label": "text"}));
        assert_eq!(typed(column(252, 16, 255, 262_140, 0), false)["size"], serde_json::json!({"max": 65535}));
        assert_eq!(typed(column(252, 16, 255, 1020, 0), true)["size"]["max"], 255);
        assert_eq!(typed(column(252, 16, 255, 67_108_860, 0), true)["size"]["max"], 16_777_215);
        assert_eq!(typed(column(252, 16, 255, u32::MAX, 0), false)["size"]["max"], 4_294_967_295u64);
        // A utf16 or utf32 TEXT sent as utf8mb4, and a TEXT sent as latin1.
        assert_eq!(typed(column(252, 16, 255, 131_068, 0), false)["size"]["max"], 65535);
        assert_eq!(typed(column(252, 16, 255, 65_532, 0), false)["size"]["max"], 65535);
        assert_eq!(typed(column(252, 16, 8, 65_535, 0), false)["size"]["max"], 65535);
        // binary(3), varbinary(8), and the four BLOBs.
        assert_eq!(typed(column(254, 128, BINARY, 3, 0), false), serde_json::json!({"kind": "bytes", "size": {"exact": 3}, "label": "binary"}));
        assert_eq!(typed(column(253, 128, BINARY, 8, 0), false), serde_json::json!({"kind": "bytes", "size": {"max": 8}, "label": "varbinary"}));
        for (length, max) in [(255, 255u64), (65_535, 65_535), (16_777_215, 16_777_215), (u32::MAX, 4_294_967_295)] {
            assert_eq!(typed(column(252, 144, BINARY, length, 0), false)["size"], serde_json::json!({"max": max}));
        }
        // decimal(10,2), decimal(5,5), decimal(65,30), decimal(6,3) unsigned and decimal(1) unsigned.
        let decimal = |length: u32, decimals: u32, flags: u32| {
            let t = typed(column(246, flags, BINARY, length, decimals), false);
            (t["precision"].as_u64(), t["min_scale"].as_i64(), t["max_scale"].as_i64())
        };
        assert_eq!(decimal(12, 2, 0), (Some(10), Some(2), Some(2)));
        assert_eq!(decimal(7, 5, 0), (Some(5), Some(5), Some(5)));
        assert_eq!(decimal(67, 30, 0), (Some(65), Some(30), Some(30)));
        assert_eq!(decimal(7, 3, UNSIGNED), (Some(6), Some(3), Some(3)));
        assert_eq!(decimal(1, 0, UNSIGNED), (Some(1), Some(0), Some(0)));
        assert_eq!(typed(column(246, 0, BINARY, 12, 2), false), serde_json::json!({"kind": "decimal", "precision": 10, "min_scale": 2, "max_scale": 2, "label": "decimal"}));
        // datetime(3), datetime, timestamp(3), timestamp(0) and time(4).
        assert_eq!(typed(column(12, 128, BINARY, 23, 3), false), serde_json::json!({"kind": "datetime", "tick": "1/1000", "label": "datetime"}));
        assert_eq!(typed(column(12, 128, BINARY, 19, 0), false)["tick"], "1");
        assert_eq!(typed(column(7, 128, BINARY, 23, 3), false), serde_json::json!({"kind": "instant", "tick": "1/1000", "min": "1970-01-01T00:00:01Z", "max": "2038-01-19T03:14:07.999Z", "label": "timestamp"}));
        assert_eq!(typed(column(7, 128, BINARY, 19, 0), false)["max"], "2038-01-19T03:14:07Z");
        assert_eq!(typed(column(11, 128, BINARY, 15, 4), false), serde_json::json!({"kind": "duration", "max": "PT838H59M59S", "tick": "1/10000", "label": "time"}));
        // A type that takes no modifier, an ENUM, a SET and a FLOAT(7,2) among them, keeps its type.
        for c in [column(3, 0, BINARY, 11, 0), column(254, 256, 255, 8, 0), column(254, 2048, 255, 16, 0), column(4, 0, BINARY, 7, 2), column(10, 128, BINARY, 10, 0)] {
            assert_eq!(declared(of(c.r#type, c.flags, c.charset, c.length), &c, false), of(c.r#type, c.flags, c.charset, c.length));
        }
        // A host that sends no length or no decimals leaves the type as the protocol's code says it.
        let bare = MysqlColumn { length: None, decimals: None, ..column(246, 0, BINARY, 12, 2) };
        assert_eq!(declared(decimal_type(), &bare, false), decimal_type());
        let bare = MysqlColumn { length: None, decimals: None, ..column(12, 0, BINARY, 23, 3) };
        assert_eq!(declared(datetime(), &bare, false), datetime());
        for c in [column(253, 0, 255, 80, 0), column(252, 16, 255, 262_140, 0), column(254, 128, BINARY, 3, 0), column(246, 0, BINARY, 12, 2), column(7, 0, BINARY, 23, 3), column(11, 0, BINARY, 15, 4)] {
            declared(of(c.r#type, c.flags, c.charset, c.length), &c, true).check().unwrap();
        }
    }

    fn decimal_type() -> Type {
        decimal()
    }

    #[test]
    fn an_enums_and_a_sets_labels_are_read_from_the_column_type_the_catalog_writes() {
        assert_eq!(labels("enum('it''s','a\\\\b','x,y','\u{dc}n\u{ef}','')"), Some(vec!["it's".to_string(), "a\\b".into(), "x,y".into(), "\u{dc}n\u{ef}".into(), "".into()]));
        assert_eq!(labels("set('a','b c')"), Some(vec!["a".to_string(), "b c".into()]));
        assert_eq!(labels("varchar(10)"), None);
        assert_eq!(labels("enum('a'"), None);
    }

    #[test]
    fn a_placeholder_widens_a_number_to_what_the_execution_binds() {
        assert_eq!(widened(&of(3, 0, 0, None), false).unwrap().short(), "int | bigint | double | decimal");
        assert_eq!(widened(&bigint(), true).unwrap().short(), "bigint | double | decimal | text");
        assert_eq!(widened(&date(), false).unwrap().short(), "text | date | datetime | time");
        assert!(widened(&Type::text(), false).is_none());
    }
}
