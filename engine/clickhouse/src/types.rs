//! ClickHouse's types: a type's name as the server writes it, read into its parts ([`parse`]), written
//! back on one line ([`Named`]'s `Display`), and as a type descriptor ([`slot`]).
//!
//! The name says everything: `Nullable(T)` is the one way a value may be NULL, `Array`, `Tuple` and
//! `Map` nest, and `LowCardinality(T)` is `T` stored another way. As the format's rules say
//! (spec/README.md): a `String` is ill-formed UTF-8 text, since ClickHouse holds any bytes in one,
//! and a `FixedString(N)` the same of exactly N bytes; a `DateTime` and a `DateTime64` are instants,
//! whose zone is how the column prints, never part of a value; an `Enum8` or `Enum16` is text that
//! is one of its labels; a `Tuple` with names is a struct and one without a tuple; a `Map`, whose
//! keys may repeat, is an array of `{ key, value }` structs; a `Variant` is a union, and holds NULL
//! with no `Nullable` around it; the geometric types are the tuples and arrays they are sugar for.
//!
//! A type the run time has no reading of yet is refused by name ([`Unread`]), so that a statement
//! is never typed with a value it could not decode: the semi-structured types (`JSON`, `Dynamic`),
//! an aggregate function's state, the intervals and the times of day.

use reylo_engine_core::types::{Bounds, DateType, DecimalType, Field, FloatType, InstantType, Kind, Size, SizeUnit, Slot, TextType, TupleType, Type};

/// A type as ClickHouse names it: its name, and the arguments in its parentheses.
#[derive(Debug, Clone, PartialEq)]
pub struct Named {
    pub name: String,
    pub args: Vec<Arg>,
}

/// One argument of a type's name.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    /// A type: the `String` of `Array(String)`.
    Type(Named),
    /// A member and its type: the `x Float64` of `Tuple(x Float64)`.
    Member(String, Named),
    /// A number: the `18` of `Decimal(18, 4)`.
    Number(String),
    /// A string, unescaped: the `UTC` of `DateTime('UTC')`.
    Text(String),
    /// An enum's label, unescaped, and its number: `'admin' = 1`.
    Label(String, i64),
    /// Anything else, as written: a `JSON` type's `max_dynamic_paths = 8` or `SKIP a.b`.
    Other(String),
}

/// A type the run time does not read yet, by its name.
#[derive(Debug, Clone, PartialEq)]
pub struct Unread(pub String);

/// The type `text` names, or `None` where it is no type's name.
pub fn parse(text: &str) -> Option<Named> {
    let mut p = Parser { text, b: text.as_bytes(), i: 0 };
    let named = p.named()?;
    p.space();
    (p.i == p.b.len()).then_some(named)
}

struct Parser<'a> {
    text: &'a str,
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self.b.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.space();
        self.b.get(self.i).copied()
    }

    /// A name: letters, digits and `_`, with dots between its parts as a JSON path has them, or one
    /// quoted with backquotes or double quotes.
    fn name(&mut self) -> Option<String> {
        match self.peek()? {
            quote @ (b'`' | b'"') => self.quoted(quote),
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = self.i;
                while self.b.get(self.i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
                    self.i += 1;
                }
                Some(self.text[start..self.i].to_string())
            }
            _ => None,
        }
    }

    /// A string or a quoted name from its opening quote, unescaped: a backslash escapes the
    /// character after it, and the quote written twice is itself.
    fn quoted(&mut self, quote: u8) -> Option<String> {
        let mut out = Vec::new();
        self.i += 1;
        loop {
            match *self.b.get(self.i)? {
                b'\\' => {
                    let (byte, length) = match *self.b.get(self.i + 1)? {
                        b'n' => (b'\n', 2),
                        b't' => (b'\t', 2),
                        b'r' => (b'\r', 2),
                        b'0' => (0, 2),
                        b'b' => (8, 2),
                        b'f' => (12, 2),
                        b'a' => (7, 2),
                        b'v' => (11, 2),
                        b'e' => (27, 2),
                        b'x' => (u8::from_str_radix(self.text.get(self.i + 2..self.i + 4)?, 16).ok()?, 4),
                        other => (other, 2),
                    };
                    out.push(byte);
                    self.i += length;
                }
                c if c == quote && self.b.get(self.i + 1) == Some(&quote) => {
                    out.push(quote);
                    self.i += 2;
                }
                c if c == quote => {
                    self.i += 1;
                    break;
                }
                c => {
                    out.push(c);
                    self.i += 1;
                }
            }
        }
        // A label that is no UTF-8 is read as the text a client would show for it.
        Some(String::from_utf8_lossy(&out).into_owned())
    }

    fn number(&mut self) -> Option<String> {
        let start = self.i;
        if self.b.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        let digits = self.i;
        while self.b.get(self.i).is_some_and(|c| c.is_ascii_digit() || *c == b'.') {
            self.i += 1;
        }
        (self.i > digits).then(|| self.text[start..self.i].to_string())
    }

    fn named(&mut self) -> Option<Named> {
        let name = self.name()?;
        self.arguments(name)
    }

    /// The type of `name`, with the arguments that follow it in parentheses.
    fn arguments(&mut self, name: String) -> Option<Named> {
        let mut args = Vec::new();
        if self.peek() == Some(b'(') {
            self.i += 1;
            if self.peek() == Some(b')') {
                self.i += 1;
                return Some(Named { name, args });
            }
            loop {
                args.push(self.arg()?);
                match self.peek()? {
                    b',' => self.i += 1,
                    b')' => {
                        self.i += 1;
                        break;
                    }
                    _ => return None,
                }
            }
        }
        Some(Named { name, args })
    }

    /// One argument: read as what it is where it ends at a `,` or a `)`, else kept as written.
    fn arg(&mut self) -> Option<Arg> {
        self.space();
        let start = self.i;
        if let Some(arg) = self.known() {
            if matches!(self.peek(), Some(b',' | b')')) {
                return Some(arg);
            }
        }
        self.i = start;
        self.raw().map(Arg::Other)
    }

    fn known(&mut self) -> Option<Arg> {
        match self.peek()? {
            b'\'' => {
                let text = self.quoted(b'\'')?;
                if self.peek() != Some(b'=') {
                    return Some(Arg::Text(text));
                }
                self.i += 1;
                self.space();
                Some(Arg::Label(text, self.number()?.parse().ok()?))
            }
            c if c.is_ascii_digit() || c == b'-' => self.number().map(Arg::Number),
            _ => {
                let mut name = self.name()?;
                // A JSON type's path, which only ever names a member.
                let mut path = false;
                while self.b.get(self.i) == Some(&b'.') {
                    self.i += 1;
                    name = format!("{name}.{}", self.name()?);
                    path = true;
                }
                match self.peek() {
                    // `name Type`: a member.
                    Some(c) if c.is_ascii_alphabetic() || matches!(c, b'_' | b'`' | b'"') => Some(Arg::Member(name, self.named()?)),
                    _ if path => None,
                    _ => self.arguments(name).map(Arg::Type),
                }
            }
        }
    }

    /// The text up to the `,` or `)` that ends an argument, past its own parentheses and strings.
    fn raw(&mut self) -> Option<String> {
        let start = self.i;
        let mut depth = 0usize;
        loop {
            match *self.b.get(self.i)? {
                quote @ (b'\'' | b'`' | b'"') => {
                    self.quoted(quote)?;
                }
                b'(' => {
                    depth += 1;
                    self.i += 1;
                }
                b')' if depth > 0 => {
                    depth -= 1;
                    self.i += 1;
                }
                b',' | b')' if depth == 0 => break,
                _ => self.i += 1,
            }
        }
        let text = self.text[start..self.i].trim();
        (!text.is_empty()).then(|| text.to_string())
    }
}

/// A string as ClickHouse writes one: in single quotes, a quote and a backslash escaped.
pub fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// A member's name: as it is where it reads as a name, else in backquotes.
fn member(name: &str) -> String {
    let plain = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    if plain {
        name.to_string()
    } else {
        format!("`{}`", name.replace('\\', "\\\\").replace('`', "\\`"))
    }
}

/// The type on one line, as ClickHouse names it beside a result's rows: `DateTime64(3, 'UTC')`.
impl std::fmt::Display for Named {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)?;
        if self.args.is_empty() {
            // A tuple of no elements keeps its parentheses, which tell it from a name alone.
            return if self.name == "Tuple" { f.write_str("()") } else { Ok(()) };
        }
        f.write_str("(")?;
        for (k, arg) in self.args.iter().enumerate() {
            if k > 0 {
                f.write_str(", ")?;
            }
            match arg {
                Arg::Type(t) => write!(f, "{t}")?,
                Arg::Member(name, t) => write!(f, "{} {t}", member(name))?,
                Arg::Number(n) => f.write_str(n)?,
                Arg::Text(text) => f.write_str(&quoted(text))?,
                Arg::Label(label, number) => write!(f, "{} = {number}", quoted(label))?,
                Arg::Other(text) => f.write_str(text)?,
            }
        }
        f.write_str(")")
    }
}

impl Named {
    /// The type inside the wrappers that change no value's kind: `Nullable` and `LowCardinality`.
    pub fn bare(&self) -> &Named {
        match (self.name.as_str(), self.args.as_slice()) {
            ("Nullable" | "LowCardinality", [Arg::Type(inner)]) => inner.bare(),
            _ => self,
        }
    }

    /// Whether the type is `name`, or holds a type of that name in its arguments.
    pub fn holds(&self, name: &str) -> bool {
        self.name == name || self.args.iter().any(|a| matches!(a, Arg::Type(t) | Arg::Member(_, t) if t.holds(name)))
    }

    /// Whether the type is a number's: an integer, a float or a decimal.
    pub fn is_number(&self) -> bool {
        let name = self.bare().name.as_str();
        integer(name).is_some() || matches!(name, "Float32" | "Float64" | "BFloat16") || name.starts_with("Decimal")
    }

    fn types(&self) -> Option<Vec<&Named>> {
        self.args.iter().map(|a| if let Arg::Type(t) = a { Some(t) } else { None }).collect()
    }

    fn number(&self, at: usize) -> Option<u32> {
        match self.args.get(at)? {
            Arg::Number(n) => n.parse().ok(),
            _ => None,
        }
    }
}

/// An integer type's width and whether it is unsigned, by its name.
/// Whether ClickHouse, writing a value of the type `from` into a column of the type `to`, may
/// change the value and raise no error: a number past the column's range wraps into it, as one
/// read from a string does, a decimal's or an instant's digits past the column's are dropped, a
/// float is rounded to a narrower one or cut to a whole number, a whole number or a decimal is
/// rounded to the nearest float where it has more digits than a float keeps, a time is dropped
/// from a date, and a NULL becomes the default of a column that takes none. A value the column
/// cannot read at all is the server's error, and none of these.
pub fn narrows(from: &Named, to: &Named) -> bool {
    let nullable = |t: &Named| match (t.name.as_str(), t.args.as_slice()) {
        ("Nullable", _) => true,
        ("LowCardinality", [Arg::Type(inner)]) => inner.name == "Nullable",
        _ => false,
    };
    if nullable(from) && !nullable(to) {
        return true;
    }
    let (from, to) = (from.bare(), to.bare());
    // A decimal's digits after the point, and an instant's after the second.
    let scale = |t: &Named| match t.name.as_str() {
        "Decimal" => Some(t.number(1).unwrap_or(0)),
        "Decimal32" | "Decimal64" | "Decimal128" | "Decimal256" => Some(t.number(0).unwrap_or(0)),
        _ => None,
    };
    let ticks = |t: &Named| match t.name.as_str() {
        "DateTime64" => Some(t.number(0).unwrap_or(3)),
        "DateTime" => Some(0),
        _ => None,
    };
    let float = |t: &Named| matches!(t.name.as_str(), "Float32" | "Float64" | "BFloat16");
    let text = |t: &Named| matches!(t.name.as_str(), "String" | "FixedString");
    match (integer(&from.name), integer(&to.name)) {
        // Every number of the one is a number of the other: as wide with the same sign, or wider and signed.
        (Some((bits, unsigned)), Some((into, into_unsigned))) => return !(if unsigned == into_unsigned { into >= bits } else { unsigned && into > bits }),
        (None, Some(_)) => return float(from) || text(from) || scale(from).is_some_and(|digits| digits > 0),
        _ => {}
    }
    if let Some(into) = scale(to) {
        return float(from) || text(from) || scale(from).is_some_and(|digits| digits > into);
    }
    let width = |t: &Named| ["BFloat16", "Float32", "Float64"].iter().position(|name| *name == t.name);
    if float(from) && float(to) {
        return width(to) < width(from);
    }
    if float(to) {
        // The bits of a whole number a float holds exactly: 8, 24 and 53 of them.
        let exact = [8, 24, 53][width(to).unwrap_or(0)];
        return match integer(&from.name) {
            Some((bits, unsigned)) => bits - u32::from(!unsigned) > exact,
            // A decimal fraction is no binary one.
            None => scale(from).is_some(),
        };
    }
    match (ticks(from), ticks(to)) {
        (Some(digits), Some(into)) => into < digits || (from.name == "DateTime64" && to.name == "DateTime"),
        (Some(_), None) => matches!(to.name.as_str(), "Date" | "Date32"),
        _ => from.name == "Date32" && to.name == "Date",
    }
}

fn integer(name: &str) -> Option<(u32, bool)> {
    let (digits, unsigned) = match name.strip_prefix("UInt") {
        Some(digits) => (digits, true),
        None => (name.strip_prefix("Int")?, false),
    };
    matches!(digits, "8" | "16" | "32" | "64" | "128" | "256").then(|| (digits.parse().expect("a width"), unsigned))
}

/// What the geometric types are sugar for, each by its name.
fn geometry(name: &str) -> Option<&'static str> {
    Some(match name {
        "Point" => "Tuple(Float64, Float64)",
        "Ring" | "LineString" => "Array(Tuple(Float64, Float64))",
        "Polygon" | "MultiLineString" => "Array(Array(Tuple(Float64, Float64)))",
        "MultiPolygon" => "Array(Array(Array(Tuple(Float64, Float64))))",
        _ => return None,
    })
}

fn text(size: Option<Size>) -> Type {
    Type::new(Kind::Text(TextType { encoding: Some("UTF-8".into()), ill_formed: true, size, ..TextType::default() }))
}

fn float(exponent_bits: u32, mantissa_bits: u32) -> Type {
    Type::new(Kind::Float(FloatType { exponent_bits, mantissa_bits, infinity: true, nan: true, ..FloatType::default() }))
}

/// The smallest step of a time with `scale` digits after the second, in lowest terms.
fn tick(scale: u32) -> String {
    if scale == 0 {
        "1".into()
    } else {
        format!("1/{}", 10u64.pow(scale.min(9)))
    }
}

/// The slot a value of the type fills, as a statement returns one: its type descriptor, and whether
/// it may be NULL.
pub fn slot(t: &Named) -> Result<Slot, Unread> {
    read(t, false)
}

/// The slot a parameter of the type fills. It differs from a returned value's in a decimal's
/// precision alone: the server reads a parameter's text against the precision its type names.
pub fn parameter(t: &Named) -> Result<Slot, Unread> {
    read(t, true)
}

fn read(t: &Named, written: bool) -> Result<Slot, Unread> {
    let slot = |t: &Named| read(t, written);
    let unread = || Unread(t.name.clone());
    let name = t.name.as_str();
    if let Some((bits, unsigned)) = integer(name) {
        let ty = if unsigned { Type::unsigned(bits) } else { Type::integer(bits) };
        return Ok(Slot::non_null(ty.named(name)));
    }
    if let Some(sugar) = geometry(name) {
        let inner = slot(&parse(sugar).expect("a type's name"))?;
        return Ok(Slot::non_null(inner.ty.named(name)));
    }
    let ty = match name {
        "Nullable" => {
            let [Arg::Type(inner)] = t.args.as_slice() else { return Err(unread()) };
            return Ok(Slot::nullable(slot(inner)?.ty));
        }
        "LowCardinality" => {
            let [Arg::Type(inner)] = t.args.as_slice() else { return Err(unread()) };
            return slot(inner);
        }
        // What an aggregate function of the simple kind keeps is a value of its argument's type.
        "SimpleAggregateFunction" => {
            let [_, Arg::Type(inner)] = t.args.as_slice() else { return Err(unread()) };
            return slot(inner);
        }
        // The type of a NULL alone, and of the elements of an array that has none.
        "Nothing" => return Ok(Slot::nullable(Type::null())),
        // A Variant holds a value of one of its types, or NULL.
        "Variant" => {
            let members = t.types().ok_or_else(unread)?.into_iter().map(|m| slot(m).map(|s| s.ty)).collect::<Result<Vec<_>, _>>()?;
            return Ok(Slot::nullable(Type::union(members)));
        }
        "Bool" => Type::boolean(),
        "Float32" => float(8, 23),
        "Float64" => float(11, 52),
        "BFloat16" => float(8, 7),
        "Decimal" => decimal(t.number(0).ok_or_else(unread)?, t.number(1).unwrap_or(0), written),
        "Decimal32" => decimal(9, t.number(0).ok_or_else(unread)?, written),
        "Decimal64" => decimal(18, t.number(0).ok_or_else(unread)?, written),
        "Decimal128" => decimal(38, t.number(0).ok_or_else(unread)?, written),
        "Decimal256" => decimal(76, t.number(0).ok_or_else(unread)?, written),
        "String" => text(None),
        "FixedString" => text(Some(Size { unit: Some(SizeUnit::Byte), exact: Some(u64::from(t.number(0).ok_or_else(unread)?)), ..Size::default() })),
        "UUID" => Type::uuid(),
        "IPv4" => Type::new(Kind::Text(TextType { format: Some("ipv4".into()), ..TextType::default() })),
        "IPv6" => Type::new(Kind::Text(TextType { format: Some("ipv6".into()), ..TextType::default() })),
        "Date" => Type::new(Kind::Date(DateType { bounds: Bounds::between("1970-01-01", "2149-06-06"), infinity: false })),
        "Date32" => Type::new(Kind::Date(DateType { bounds: Bounds::between("1900-01-01", "2299-12-31"), infinity: false })),
        "DateTime" => Type::new(Kind::Instant(InstantType { tick: "1".into(), bounds: Bounds::between("1970-01-01T00:00:00Z", "2106-02-07T06:28:15Z"), infinity: false })),
        "DateTime64" => Type::new(Kind::Instant(InstantType { tick: tick(t.number(0).ok_or_else(unread)?), ..InstantType::default() })),
        "Enum8" | "Enum16" => {
            let labels: Option<Vec<String>> = t.args.iter().map(|a| if let Arg::Label(label, _) = a { Some(label.clone()) } else { None }).collect();
            Type::text_of(labels.filter(|l| !l.is_empty()).ok_or_else(unread)?)
        }
        "Array" => {
            let [Arg::Type(element)] = t.args.as_slice() else { return Err(unread()) };
            return Ok(Slot::non_null(Type::array(slot(element)?)));
        }
        "Tuple" => return Ok(Slot::non_null(tuple(t, written)?)),
        // A Nested column is an array of its members' tuples.
        "Nested" => Type::array(Slot::non_null(tuple(t, written)?)),
        // A map's keys may repeat, so it is its pairs in order.
        "Map" => {
            let [Arg::Type(key), Arg::Type(value)] = t.args.as_slice() else { return Err(unread()) };
            let (key, value) = (slot(key)?, slot(value)?);
            Type::array(Slot::non_null(Type::structure(vec![Field::new("key", key.ty, key.nullable), Field::new("value", value.ty, value.nullable)])))
        }
        _ => return Err(unread()),
    };
    Ok(Slot::non_null(ty.named(name)))
}

/// A decimal's type. A parameter's text is read against the precision its type names. A value a
/// statement returns is bound by the integer it is stored in alone, which arithmetic is checked
/// against: a Decimal(18, 6) holds 8999999999999.999991, of 19 digits, so its precision is the
/// most digits that integer has (of 32, 64, 128 or 256 bits).
fn decimal(precision: u32, scale: u32, written: bool) -> Type {
    let digits = match precision {
        _ if written => precision,
        0..=9 => 10,
        10..=18 => 19,
        19..=38 => 39,
        _ => 77,
    };
    Type::decimal(DecimalType { precision: Some(digits), min_scale: Some(scale as i32), max_scale: Some(scale as i32), ..DecimalType::default() })
}

/// A tuple's type: a struct where every element has a name, else a tuple of its elements in order.
fn tuple(t: &Named, written: bool) -> Result<Type, Unread> {
    let slot = |t: &Named| read(t, written);
    let unread = || Unread(t.name.clone());
    if !t.args.is_empty() && t.args.iter().all(|a| matches!(a, Arg::Member(..))) {
        let fields = t.args.iter().map(|a| {
            let Arg::Member(name, ty) = a else { unreachable!("every argument is a member") };
            slot(ty).map(|s| Field::new(name.clone(), s.ty, s.nullable))
        });
        return Ok(Type::structure(fields.collect::<Result<Vec<_>, _>>()?));
    }
    let elements = t.types().ok_or_else(unread)?.into_iter().map(slot).collect::<Result<Vec<_>, _>>()?;
    Ok(Type::new(Kind::Tuple(TupleType { elements, ..TupleType::default() })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn typed(name: &str) -> serde_json::Value {
        serde_json::to_value(slot(&parse(name).expect(name)).expect(name)).unwrap()
    }

    #[test]
    fn a_type_narrows_into_a_column_that_changes_its_values_in_silence() {
        let narrows = |from: &str, to: &str| super::narrows(&parse(from).unwrap(), &parse(to).unwrap());
        // A number past the column's range wraps into it, whichever sign it has, and one read from a string too.
        for (from, to) in [("UInt64", "UInt8"), ("Int8", "UInt8"), ("UInt8", "Int8"), ("Int64", "Int32"), ("String", "UInt8"), ("Float64", "Int32"), ("Decimal(9, 2)", "Int64")] {
            assert!(narrows(from, to), "{from} into {to}");
        }
        // Digits past the column's are dropped, a float is rounded to a narrower one, and a time dropped from a date.
        for (from, to) in [("Decimal(18, 4)", "Decimal(9, 2)"), ("Float64", "Decimal(9, 2)"), ("String", "Decimal(9, 2)"), ("Float64", "Float32"), ("Float32", "BFloat16"), ("DateTime64(6, 'UTC')", "DateTime64(3, 'UTC')"), ("DateTime64(3)", "DateTime"), ("DateTime", "Date"), ("Date32", "Date")] {
            assert!(narrows(from, to), "{from} into {to}");
        }
        // A whole number of more bits than a float keeps is rounded to the nearest one, and so is a decimal.
        for (from, to) in [("UInt32", "Float32"), ("Int32", "Float32"), ("UInt64", "Float64"), ("Int64", "Float64"), ("UInt16", "BFloat16"), ("Decimal(9, 2)", "Float64"), ("Nullable(Int64)", "Nullable(Float64)")] {
            assert!(narrows(from, to), "{from} into {to}");
        }
        // A NULL becomes the default of a column that takes none.
        assert!(narrows("Nullable(UInt8)", "UInt8") && narrows("LowCardinality(Nullable(String))", "String"));
        // The same type, a wider one, and one the server reads or refuses aloud.
        for (from, to) in [("UInt8", "UInt8"), ("UInt8", "UInt64"), ("UInt8", "Int16"), ("UInt8", "Nullable(UInt8)"), ("String", "LowCardinality(String)"), ("Decimal(9, 2)", "Decimal(18, 4)"), ("Float32", "Float64"), ("BFloat16", "Float32"), ("DateTime", "DateTime64(3)"), ("Date", "Date32"), ("String", "Date"), ("String", "FixedString(2)"), ("UInt64", "Decimal(9, 2)"), ("UInt8", "String"), ("Decimal(9, 0)", "Int64"), ("UInt8", "Float32"), ("UInt16", "Float32"), ("Int16", "Float32"), ("UInt32", "Float64"), ("Int32", "Float64"), ("UInt8", "BFloat16"), ("String", "Float64")] {
            assert!(!narrows(from, to), "{from} into {to}");
        }
    }

    #[test]
    fn a_name_reads_into_its_parts_and_writes_back_on_one_line() {
        for name in [
            "UInt64",
            "Nullable(String)",
            "Decimal(18, 4)",
            "DateTime64(3, 'UTC')",
            "DateTime('America/New_York')",
            "Enum8('admin' = 1, 'it\\'s' = -2)",
            "Array(Nullable(UInt8))",
            "Tuple(UInt8, String)",
            "Tuple(n UInt8, s Nullable(String))",
            "Tuple(`a b` UInt8)",
            "Tuple()",
            "Map(String, Array(UInt8))",
            "LowCardinality(Nullable(String))",
            "Variant(String, UInt8)",
            "AggregateFunction(quantiles(0.5, 0.9), UInt64)",
            "SimpleAggregateFunction(sum, UInt64)",
            "JSON(a.b UInt32, max_dynamic_paths = 8, SKIP c.d)",
            "Nested(a UInt8, b String)",
        ] {
            assert_eq!(parse(name).expect(name).to_string(), name);
        }
        // As a describe writes a tuple with names: a member a line.
        assert_eq!(parse("Tuple(\n    x Float64,\n    y Tuple(\n        z String))").unwrap().to_string(), "Tuple(x Float64, y Tuple(z String))");
        assert_eq!(parse("Decimal(18,4)").unwrap().to_string(), "Decimal(18, 4)");
        assert_eq!(parse("UInt64 extra"), None);
        assert_eq!(parse("Array(String"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse("Enum8('a' = 1)").unwrap().args, vec![Arg::Label("a".into(), 1)]);
        assert_eq!(parse("JSON(a.b UInt32)").unwrap().args, vec![Arg::Member("a.b".into(), parse("UInt32").unwrap())]);
    }

    #[test]
    fn the_types_as_the_format_writes_them() {
        assert_eq!(typed("UInt8"), json!({"type": {"kind": "integer", "bits": 8, "unsigned": true, "label": "UInt8"}}));
        assert_eq!(typed("Int256"), json!({"type": {"kind": "integer", "bits": 256, "label": "Int256"}}));
        assert_eq!(typed("Nullable(Int64)"), json!({"type": {"kind": "integer", "bits": 64, "label": "Int64"}, "nullable": true}));
        assert_eq!(typed("LowCardinality(Nullable(String))"), json!({"type": {"kind": "text", "encoding": "UTF-8", "ill_formed": true, "label": "String"}, "nullable": true}));
        assert_eq!(typed("FixedString(4)"), json!({"type": {"kind": "text", "encoding": "UTF-8", "ill_formed": true, "size": {"unit": "byte", "exact": 4}, "label": "FixedString"}}));
        // A returned decimal has the digits its storage holds, and a parameter those its type names.
        assert_eq!(typed("Decimal(18, 4)"), json!({"type": {"kind": "decimal", "precision": 19, "min_scale": 4, "max_scale": 4, "label": "Decimal"}}));
        assert_eq!(typed("Decimal64(2)"), json!({"type": {"kind": "decimal", "precision": 19, "min_scale": 2, "max_scale": 2, "label": "Decimal64"}}));
        assert!(typed("Array(Decimal(9, 2))").to_string().contains("\"precision\":10"));
        assert_eq!(typed("Decimal(38, 2)")["type"]["precision"], json!(39));
        assert_eq!(typed("Decimal(76, 2)")["type"]["precision"], json!(77));
        let written = |name: &str| serde_json::to_value(parameter(&parse(name).expect(name)).expect(name)).unwrap();
        assert_eq!(written("Decimal(18, 4)")["type"]["precision"], json!(18));
        let nested = written("Tuple(a Decimal32(2), b Array(Decimal(38, 2)))").to_string();
        assert!(nested.contains("\"precision\":9") && nested.contains("\"precision\":38"), "{nested}");
        assert_eq!(typed("Float64"), json!({"type": {"kind": "float", "exponent_bits": 11, "mantissa_bits": 52, "infinity": true, "nan": true, "label": "Float64"}}));
        assert_eq!(typed("DateTime64(3, 'UTC')"), json!({"type": {"kind": "instant", "tick": "1/1000", "label": "DateTime64"}}));
        assert_eq!(typed("DateTime('Asia/Tokyo')"), json!({"type": {"kind": "instant", "tick": "1", "min": "1970-01-01T00:00:00Z", "max": "2106-02-07T06:28:15Z", "label": "DateTime"}}));
        assert_eq!(typed("Date"), json!({"type": {"kind": "date", "min": "1970-01-01", "max": "2149-06-06", "label": "Date"}}));
        assert_eq!(typed("Enum8('admin' = 1, 'member' = 2)"), json!({"type": {"kind": "enum", "type": {"kind": "text"}, "values": ["admin", "member"], "label": "Enum8"}}));
        assert_eq!(typed("Array(Nullable(UInt8))"), json!({"type": {"kind": "array", "element": {"type": {"kind": "integer", "bits": 8, "unsigned": true, "label": "UInt8"}, "nullable": true}}}));
        assert_eq!(typed("Tuple(UInt8, Nullable(String))")["type"]["elements"][1]["nullable"], json!(true));
        assert_eq!(typed("Tuple(n UInt8, s String)")["type"]["fields"][1]["name"], json!("s"));
        assert_eq!(typed("Map(String, UInt8)")["type"]["element"]["type"]["fields"][0]["name"], json!("key"));
        assert_eq!(typed("Map(String, UInt8)")["type"]["label"], json!("Map"));
        assert_eq!(typed("Variant(String, UInt8)"), json!({"type": {"kind": "union", "members": [{"kind": "integer", "bits": 8, "unsigned": true, "label": "UInt8"}, {"kind": "text", "encoding": "UTF-8", "ill_formed": true, "label": "String"}]}, "nullable": true}));
        assert_eq!(typed("Nullable(Nothing)"), json!({"type": {"kind": "null"}, "nullable": true}));
        assert_eq!(typed("Point")["type"]["label"], json!("Point"));
        assert_eq!(typed("SimpleAggregateFunction(sum, UInt64)")["type"]["bits"], json!(64));
        assert_eq!(typed("IPv4"), json!({"type": {"kind": "text", "format": "ipv4", "label": "IPv4"}}));
    }

    #[test]
    fn a_type_the_run_time_does_not_read_is_refused_by_name() {
        for (name, refused) in [("JSON", "JSON"), ("Array(Dynamic)", "Dynamic"), ("AggregateFunction(sum, UInt64)", "AggregateFunction"), ("Tuple(a IntervalDay)", "IntervalDay"), ("Nullable(Time)", "Time")] {
            assert_eq!(slot(&parse(name).unwrap()), Err(Unread(refused.into())));
        }
    }

    #[test]
    fn a_number_s_type_is_told_through_its_wrappers() {
        assert!(parse("Nullable(UInt64)").unwrap().is_number());
        assert!(parse("LowCardinality(Float32)").unwrap().is_number());
        assert!(parse("Decimal(9, 2)").unwrap().is_number());
        assert!(!parse("String").unwrap().is_number());
        assert!(!parse("Array(UInt8)").unwrap().is_number());
    }
}
