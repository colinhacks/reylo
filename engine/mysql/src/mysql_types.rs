//! MySQL's types as the format describes them (`spec/index.ts`): what a prepare's protocol
//! type code, flags, character set and display length say of a value, under MySQL's own name for
//! the type (`bigint`, `tinyint(1)`, `datetime`).

use crate::types::{ArrayType, Bounds, DateTimeType, DateType, DurationType, InstantType, IntegerType, Kind, Position, SetType, Size, TimePart, Type, MICROSECOND};

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
    Type::new(Kind::Duration(DurationType { time: Some(TimePart { tick: MICROSECOND.into(), max: Some("PT838H59M59S".into()) }), ..DurationType::default() })).named("time")
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

/// A value one digit or one bit wide that is not a table's own column: an expression's or a view's
/// width says nothing of its values, so a `tinyint(1)` is a TINYINT and a `BIT(1)` bits of no
/// known number.
pub(crate) fn unflagged(ty: Type) -> Type {
    match (&ty.kind, ty.name.as_deref()) {
        (Kind::Integer(_), Some("tinyint(1)")) => Type { name: Some("tinyint".into()), ..ty },
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
        assert_eq!(serde_json::to_value(of(8, UNSIGNED, 0, Some(20))).unwrap(), serde_json::json!({"kind": "integer", "bits": 64, "unsigned": true, "name": "bigint"}));
        assert_eq!(of(1, 0, 0, Some(1)).short(), "tinyint(1)");
        assert_eq!(of(1, 0, 0, Some(4)).short(), "tinyint");
        assert_eq!(of(13, 0, 0, None).short(), "year(0..2155)");
        assert_eq!(of(254, ENUM, 255, None).short(), "enum");
        assert_eq!(of(254, SET, 255, None).short(), "set{text}");
        assert_eq!(serde_json::to_value(enumeration(labels("enum('x','y')"))).unwrap(), serde_json::json!({"kind": "text", "enum": ["x", "y"], "name": "enum"}));
        assert_eq!(serde_json::to_value(set_of(labels("set('a','b')"))).unwrap(), serde_json::json!({"kind": "set", "element": {"type": {"kind": "text", "enum": ["a", "b"]}}, "name": "set"}));
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
