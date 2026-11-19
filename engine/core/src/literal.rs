//! The constants a statement writes, and a column's type narrowed to them: where every value a
//! column holds is a constant the statement wrote (`select 'user' as kind`, each arm of a
//! `union`, each result of a `case`), the column is an enum of those values over its type, in the
//! type's canonical text (spec/README.md, rules 2 and 5). Each database's engine reads the
//! constants from its own parse tree, and only where its server returns them as written; this
//! module says what they are as values of the type the server describes the column as, or that
//! one of them is no value it holds unchanged, which leaves the type as it was.

use crate::types::{DecimalType, EnumType, IntegerType, Kind, SizeUnit, TextType, Type};

/// A constant as the statement writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    /// A string, as its quotes and escapes give it.
    Text(String),
    /// A number in decimal, with a sign, a fraction or an exponent where it has them (`-1.50`, `1e2`).
    Number(String),
    Boolean(bool),
}

/// The longest canonical text a literal is written in: past it (`1e99999`, which a decimal holds),
/// the type keeps every value rather than list one of a hundred thousand digits.
const LONGEST: usize = 1000;

/// The canonical text of a number written in decimal (rule 5): no sign but a minus, no zeros but
/// those it needs, and no exponent (`1.50` is `1.5`, `1e2` is `100`, `-0` is `0`). `None` for
/// anything else, and for a number whose text would run past [`LONGEST`].
pub fn canonical_number(text: &str) -> Option<String> {
    let (minus, body) = match text.strip_prefix('-') {
        Some(body) => (true, body),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(at) => (&body[..at], body[at + 1..].parse::<i64>().ok()?),
        None => (body, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if (whole.is_empty() && fraction.is_empty()) || !whole.bytes().chain(fraction.bytes()).all(|b| b.is_ascii_digit()) || exponent.unsigned_abs() > LONGEST as u64 {
        return None;
    }
    // The digits, and where the point stands among them.
    let digits = format!("{whole}{fraction}");
    let point = whole.len() as i64 + exponent;
    let leading = digits.bytes().take_while(|&b| b == b'0').count();
    let digits = &digits[leading..];
    let point = point - leading as i64;
    let digits = digits.trim_end_matches('0');
    if digits.is_empty() {
        return Some("0".into());
    }
    let n = digits.len() as i64;
    let text = if point <= 0 {
        format!("0.{}{digits}", "0".repeat((-point) as usize))
    } else if point >= n {
        format!("{digits}{}", "0".repeat((point - n) as usize))
    } else {
        format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
    };
    if text.len() > LONGEST {
        return None;
    }
    Some(if minus { format!("-{text}") } else { text })
}

/// `ty` narrowed to `literals`, the only values its column holds: an enum of them over `ty`, or,
/// where `ty` is an enum already, over its base and among its values. `None` where one is no value
/// of its kind as written, or one the type could hold only changed (a string longer than its size,
/// a fraction past its scale, a number past its bits), and for a kind no constant is written as. A
/// float holds them only at 64 bits, where every client reads the value the server prints; a
/// boolean holds them only where they are fewer than both. `true` and `false` in an integer are 1
/// and 0, as MySQL and SQLite hold them.
pub fn narrow(ty: &Type, literals: &[Literal]) -> Option<Type> {
    if literals.is_empty() {
        return None;
    }
    let (base, known) = match &ty.kind {
        Kind::Enum(e) => (&*e.ty, Some(&e.values)),
        _ => (ty, None),
    };
    let mut values: Vec<String> = Vec::new();
    for l in literals {
        let value = match (&base.kind, l) {
            (Kind::Text(t), Literal::Text(s)) if fits_text(s, t) => s.clone(),
            (Kind::Integer(i), Literal::Number(n)) => canonical_number(n).filter(|n| !n.contains('.') && fits_integer(n, i))?,
            (Kind::Decimal(d), Literal::Number(n)) => canonical_number(n).filter(|n| fits_decimal(n, d))?,
            (Kind::Float(f), Literal::Number(n)) if f.exponent_bits == 11 && f.mantissa_bits == 52 => {
                let canonical = canonical_number(n)?;
                // Fifteen significant digits at most, which every double between 10^-307 and 10^308
                // writes back unchanged: a longer number is one a server may read or print a step
                // away from the double nearest it, as SQLite reads 1.918287769068991566283487 and
                // MySQL's text protocol prints 9935.636974329345.
                if canonical.trim_start_matches('-').replace('.', "").trim_matches('0').len() > 15 {
                    return None;
                }
                let parsed = canonical.parse::<f64>().ok().filter(|v| *v == 0.0 || v.is_normal())?;
                // Its text, read back as a double, is the value: `0.1` is the double nearest 0.1.
                canonical_number(&parsed.to_string())?
            }
            (Kind::Boolean(_), Literal::Boolean(b)) => b.to_string(),
            (Kind::Integer(_), Literal::Boolean(b)) => if *b { "1" } else { "0" }.to_string(),
            _ => return None,
        };
        if known.is_some_and(|known| !known.contains(&value)) {
            return None;
        }
        push(&mut values, value);
    }
    if matches!(base.kind, Kind::Boolean(_)) && values.len() > 1 {
        return None;
    }
    let label = if known.is_some() { ty.label.clone() } else { None };
    Some(Type { kind: Kind::Enum(EnumType { ty: Box::new(base.clone()), values }), label, comment: ty.comment.clone().filter(|_| known.is_some()), title: ty.title.clone().filter(|_| known.is_some()) })
}

/// Whether a text type holds a string as written: every character one its code points and its
/// encoding hold, no longer than its size, in the unit the size counts, and never in a type that
/// pads every value to one length.
fn fits_text(s: &str, t: &TextType) -> bool {
    if t.code_points.as_ref().is_some_and(|ranges| s.chars().any(|c| !ranges.iter().any(|[lo, hi]| (*lo..=*hi).contains(&(c as u32))))) {
        return false;
    }
    let encoding = t.encoding.as_deref().map(str::to_ascii_uppercase);
    if !encoding.as_deref().is_none_or(|e| s.chars().all(|c| encodes(e, c))) {
        return false;
    }
    let Some(size) = &t.size else { return true };
    if size.exact.is_some() {
        return false;
    }
    let length = match (size.unit, encoding.as_deref()) {
        (None, _) => s.chars().count(),
        (Some(SizeUnit::Byte | SizeUnit::CodeUnit), Some("UTF-8")) => s.len(),
        (Some(SizeUnit::CodeUnit), Some("UTF-16")) => s.encode_utf16().count(),
        (Some(SizeUnit::Byte), Some("UTF-16")) => s.encode_utf16().count() * 2,
        _ => return false,
    } as u64;
    size.max.is_none_or(|m| length <= m) && size.min.is_none_or(|m| length >= m)
}

/// Whether a character set, by its name in capitals, holds a character as itself: the Unicode
/// encodings every one, US-ASCII and ISO-8859-1 those of their code points, and every other set an
/// ASCII letter, digit, space or punctuation mark that each of them maps to itself (Swedish 7-bit
/// ASCII puts letters at `$`, `@`, the brackets, the braces, `|`, `~` and the backquote).
fn encodes(encoding: &str, c: char) -> bool {
    match encoding {
        "UTF-8" | "UTF-16" | "UTF-32" | "CESU-8" | "GB18030" => true,
        "US-ASCII" => c.is_ascii(),
        "ISO-8859-1" => (c as u32) <= 0xFF,
        _ => c.is_ascii_alphanumeric() || " !\"#%&'()*+,-./:;<=>?_".contains(c),
    }
}

fn push(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

/// Whether a whole number's canonical text is inside an integer type's bits.
fn fits_integer(n: &str, i: &IntegerType) -> bool {
    let Ok(v) = n.parse::<i128>() else { return false };
    if i.bits >= 128 {
        return !(i.unsigned && v < 0);
    }
    if i.unsigned {
        v >= 0 && v < (1i128 << i.bits)
    } else {
        v >= -(1i128 << (i.bits - 1)) && v < (1i128 << (i.bits - 1))
    }
}

/// Whether a decimal type holds a number's canonical text exactly: no more digits after the point
/// than its largest scale, and no more in all than its precision.
fn fits_decimal(n: &str, d: &DecimalType) -> bool {
    let digits = n.trim_start_matches('-');
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let whole = whole.trim_start_matches('0');
    let scale = fraction.len() as i64;
    if d.max_scale.is_some_and(|max| scale > i64::from(max)) {
        return false;
    }
    // The coefficient's digits at the type's least scale, which pads the fraction with zeros.
    let padded = scale.max(i64::from(d.min_scale.unwrap_or(0)).max(0));
    let significant = (whole.len() as i64 + padded).max(1);
    d.precision.is_none_or(|p| significant <= i64::from(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FloatType, Size};

    fn number(n: &str) -> Literal {
        Literal::Number(n.into())
    }

    #[test]
    fn a_number_is_written_without_an_exponent_or_a_zero_it_needs_not() {
        for (written, canonical) in [("1", "1"), ("-1", "-1"), ("+1", "1"), ("007", "7"), ("1.50", "1.5"), ("1.", "1"), (".5", "0.5"), ("-0", "0"), ("-0.0", "0"), ("0.000", "0"), ("1e2", "100"), ("1.5E-3", "0.0015"), ("12.5e1", "125"), ("-2.5e-1", "-0.25"), ("100e-2", "1")] {
            assert_eq!(canonical_number(written).as_deref(), Some(canonical), "{written}");
        }
        // A number of up to a thousand digits, and none longer.
        assert_eq!(canonical_number("1e999"), Some(format!("1{}", "0".repeat(999))));
        for not in ["", ".", "-", "1e", "e1", "0x10", "1_000", "1.2.3", "NaN", "Infinity", "1e999999", "1e1001", "0.5e-1000"] {
            assert_eq!(canonical_number(not), None, "{not}");
        }
    }

    #[test]
    fn a_type_narrows_to_the_constants_it_holds_unchanged() {
        let int4 = Type::new(Kind::Integer(IntegerType { bits: 32, ..IntegerType::default() })).named("int4");
        assert_eq!(narrow(&int4, &[number("1"), number("2"), number("1")]).unwrap().short(), "int4(1 | 2)");
        // A fraction, a string and a number past the type's bits are no value it holds as written.
        assert_eq!(narrow(&int4, &[number("1.5")]), None);
        assert_eq!(narrow(&int4, &[Literal::Text("1".into())]), None);
        assert_eq!(narrow(&int4, &[number("2147483648")]), None);
        assert!(narrow(&int4, &[number("-2147483648")]).is_some());
        let uint8 = Type::new(Kind::Integer(IntegerType { bits: 8, unsigned: true, ..IntegerType::default() }));
        assert_eq!((narrow(&uint8, &[number("255")]).is_some(), narrow(&uint8, &[number("-1")])), (true, None));

        let numeric = Type::decimal(DecimalType::default()).named("numeric");
        assert_eq!(serde_json::to_value(narrow(&numeric, &[number("1.50"), number("2")]).unwrap()).unwrap()["values"], serde_json::json!(["1.5", "2"]));
        let money = Type::decimal(DecimalType { precision: Some(4), min_scale: Some(2), max_scale: Some(2), ..DecimalType::default() });
        assert!(narrow(&money, &[number("12.30")]).is_some());
        assert_eq!(narrow(&money, &[number("1.234")]), None);
        assert_eq!(narrow(&money, &[number("123")]), None);

        let float8 = Type::new(Kind::Float(FloatType { exponent_bits: 11, mantissa_bits: 52, ..FloatType::default() })).named("float8");
        assert_eq!(serde_json::to_value(narrow(&float8, &[number("0.1"), number("1e2"), number("1.50")]).unwrap()).unwrap()["values"], serde_json::json!(["0.1", "100", "1.5"]));
        // A number of more than fifteen significant digits, or past a normal double's range, is one a
        // server may read or print otherwise.
        assert_eq!(narrow(&float8, &[number("1.918287769068991566283487")]), None);
        assert_eq!(narrow(&float8, &[number("9935.636974329345")]), None);
        assert!(narrow(&float8, &[number("123456789012345")]).is_some());
        assert_eq!(narrow(&float8, &[number("1e-320")]), None);
        let float4 = Type::new(Kind::Float(FloatType { exponent_bits: 8, mantissa_bits: 23, ..FloatType::default() }));
        assert_eq!(narrow(&float4, &[number("0.5")]), None);

        let text = Type::text().named("text");
        assert_eq!(narrow(&text, &[Literal::Text("user".into()), Literal::Text("org".into())]).unwrap().short(), "text('user' | 'org')");
        assert_eq!(narrow(&text, &[number("1")]), None);
        // A string longer than the type's size, or any in a type that pads, is no value as written.
        let varchar = |size: Size| Type::new(Kind::Text(TextType { size: Some(size), ..TextType::default() }));
        assert!(narrow(&varchar(Size { max: Some(3), ..Size::default() }), &[Literal::Text("abc".into())]).is_some());
        assert_eq!(narrow(&varchar(Size { max: Some(2), ..Size::default() }), &[Literal::Text("abc".into())]), None);
        assert_eq!(narrow(&varchar(Size { exact: Some(3), ..Size::default() }), &[Literal::Text("abc".into())]), None);
        // A string outside the type's code points is none of its values.
        let no_nul = Type::new(Kind::Text(TextType { code_points: Some(vec![[1, 0x10FFFF]]), ..TextType::default() }));
        assert_eq!((narrow(&no_nul, &[Literal::Text("a".into())]).is_some(), narrow(&no_nul, &[Literal::Text("a\0".into())])), (true, None));
        // And one its encoding does not hold, which a server writes as something else.
        let latin1 = |e: &str| Type::new(Kind::Text(TextType { encoding: Some(e.into()), ..TextType::default() }));
        assert_eq!((narrow(&latin1("ISO-8859-1"), &[Literal::Text("café".into())]).is_some(), narrow(&latin1("ISO-8859-1"), &[Literal::Text("🙂".into())])), (true, None));
        assert_eq!((narrow(&latin1("windows-1252"), &[Literal::Text("a b-c".into())]).is_some(), narrow(&latin1("windows-1252"), &[Literal::Text("café".into())])), (true, None));
        assert_eq!(narrow(&latin1("SEN_850200_B"), &[Literal::Text("a[1]".into())]), None);
        // An enum's labels narrow to those written, under the enum's own name, and a string of
        // none of them is no value of it.
        let mood = Type::text_of(vec!["happy".into(), "sad".into()]).named("public.mood");
        assert_eq!(narrow(&mood, &[Literal::Text("sad".into())]).unwrap(), Type::text_of(vec!["sad".into()]).named("public.mood"));
        assert_eq!(narrow(&mood, &[Literal::Text("calm".into())]), None);
        // A literal's enum carries no name of its own: its base's is the column's.
        assert_eq!(narrow(&int4, &[number("1")]).unwrap().label, None);

        let boolean = Type::boolean();
        assert_eq!(narrow(&boolean, &[Literal::Boolean(true)]).unwrap().short(), "boolean(true)");
        // A truth value an integer holds is 1 or 0.
        assert_eq!(narrow(&int4, &[Literal::Boolean(true), Literal::Boolean(false)]).unwrap().short(), "int4(1 | 0)");
        assert_eq!(narrow(&boolean, &[Literal::Boolean(true), Literal::Boolean(false)]), None);
        assert_eq!(narrow(&Type::uuid(), &[Literal::Text("x".into())]), None);
        assert_eq!(narrow(&text, &[]), None);
        for ty in [narrow(&int4, &[number("1")]), narrow(&numeric, &[number("1.5")]), narrow(&float8, &[number("0.5")]), narrow(&boolean, &[Literal::Boolean(false)])] {
            ty.unwrap().check().unwrap();
        }
    }
}
