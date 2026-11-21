//! A port of Postgres's scanner (`scan.l`) and of the lookahead `parser.c` adds over it. Each rule
//! is matched as flex matches it: the longest match, and among equals the rule written first.

use super::generated::{tok, CHAR_TOKENS, KEYWORDS};
use crate::lalr::Lexeme;

/// The longest identifier Postgres keeps, in bytes (`NAMEDATALEN - 1`).
const MAX_IDENTIFIER: usize = 63;

/// A token's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    None,
    /// A name or a literal's value, which an escape may have left outside UTF-8.
    Bytes(Vec<u8>),
    Int(i32),
    /// A keyword, by its lowercase name.
    Keyword(&'static str),
}

/// A token of the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: u16,
    pub start: u32,
    pub end: u32,
    pub value: Value,
}

/// Where the scanner stopped, with what it had read.
#[derive(Debug)]
pub struct ScanError {
    pub message: String,
    pub at: u32,
    /// The tokens read before the error, and where the trivia after them starts.
    pub read: (Vec<Lexeme>, u32),
}

/// A scanner error before the tokens read are attached.
#[derive(Debug)]
pub struct Stop {
    pub message: String,
    pub at: u32,
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0c | 0x0b)
}
fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
}
fn is_ident_cont(c: u8) -> bool {
    is_ident_start(c) || c.is_ascii_digit() || c == b'$'
}
fn is_dolq_cont(c: u8) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}
fn is_op_char(c: u8) -> bool {
    matches!(c, b'~' | b'!' | b'@' | b'#' | b'^' | b'&' | b'|' | b'`' | b'?' | b'+' | b'-' | b'*' | b'/' | b'%' | b'<' | b'>' | b'=')
}

/// A keyword's entry, whatever its case.
pub fn keyword(word: &[u8]) -> Option<&'static (&'static str, u16, u8)> {
    if word.len() > 32 || !word.is_ascii() {
        return None;
    }
    let mut lower = [0u8; 32];
    for (l, c) in lower.iter_mut().zip(word) {
        *l = c.to_ascii_lowercase();
    }
    let lower = &lower[..word.len()];
    KEYWORDS.binary_search_by(|k| k.0.as_bytes().cmp(lower)).ok().map(|i| &KEYWORDS[i])
}

/// `truncate_identifier`: at most 63 bytes, on a character's boundary.
fn truncate(mut name: Vec<u8>) -> Vec<u8> {
    if name.len() > MAX_IDENTIFIER {
        let mut end = MAX_IDENTIFIER;
        while end > 0 && name[end] & 0xC0 == 0x80 {
            end -= 1;
        }
        name.truncate(end);
    }
    name
}

fn downcase_truncate(word: &[u8]) -> Vec<u8> {
    truncate(word.iter().map(u8::to_ascii_lowercase).collect())
}

/// `pg_strtoint32_safe` over a token: `None` where it overflows.
fn int32(text: &[u8]) -> Option<i32> {
    let (base, digits): (u32, &[u8]) = match text {
        [b'0', b'x' | b'X', rest @ ..] => (16, rest),
        [b'0', b'o' | b'O', rest @ ..] => (8, rest),
        [b'0', b'b' | b'B', rest @ ..] => (2, rest),
        _ => (10, text),
    };
    let mut value: u64 = 0;
    for &c in digits {
        if c == b'_' {
            continue;
        }
        value = value * base as u64 + (c as char).to_digit(base)? as u64;
        if value > i32::MAX as u64 {
            return None;
        }
    }
    Some(value as i32)
}

/// `decinteger`, from `i`: where it ends, or `i` if none starts there.
fn decinteger(b: &[u8], i: usize) -> usize {
    if !b.get(i).is_some_and(u8::is_ascii_digit) {
        return i;
    }
    let mut j = i + 1;
    loop {
        match (b.get(j), b.get(j + 1)) {
            (Some(c), _) if c.is_ascii_digit() => j += 1,
            (Some(b'_'), Some(d)) if d.is_ascii_digit() => j += 2,
            _ => return j,
        }
    }
}

/// `0[xX](_?hexdigit)+` and its kin, from `i`.
fn radix_integer(b: &[u8], i: usize, letter: u8, digit: fn(u8) -> bool) -> Option<usize> {
    if b.get(i) != Some(&b'0') || !b.get(i + 1).is_some_and(|c| c.eq_ignore_ascii_case(&letter)) {
        return None;
    }
    let mut j = i + 2;
    let mut any = false;
    loop {
        match (b.get(j), b.get(j + 1)) {
            (Some(&c), _) if digit(c) => {
                j += 1;
                any = true;
            }
            (Some(b'_'), Some(&d)) if digit(d) => {
                j += 2;
                any = true;
            }
            _ => return any.then_some(j),
        }
    }
}

/// `{hexfail}` and its kin: the prefix, with an underscore if one follows.
fn radix_fail(b: &[u8], i: usize, letter: u8) -> Option<usize> {
    if b.get(i) != Some(&b'0') || !b.get(i + 1).is_some_and(|c| c.eq_ignore_ascii_case(&letter)) {
        return None;
    }
    Some(if b.get(i + 2) == Some(&b'_') { i + 3 } else { i + 2 })
}

fn identifier_from(b: &[u8], i: usize) -> Option<usize> {
    if !b.get(i).copied().is_some_and(is_ident_start) {
        return None;
    }
    let mut j = i + 1;
    while b.get(j).copied().is_some_and(is_ident_cont) {
        j += 1;
    }
    Some(j)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Number {
    Dec,
    Hex,
    Oct,
    Bin,
    HexFail,
    OctFail,
    BinFail,
    Numeric,
    NumericFail,
    Real,
    RealFail,
    IntegerJunk,
    NumericJunk,
    RealJunk,
}

/// The number rule that matches at `i`, and where it ends.
fn number(b: &[u8], i: usize) -> Option<(Number, usize)> {
    let dec = decinteger(b, i);
    let has_dec = dec > i;
    let numeric = if has_dec && b.get(dec) == Some(&b'.') {
        Some(decinteger(b, dec + 1).max(dec + 1))
    } else if !has_dec && b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
        Some(decinteger(b, i + 1))
    } else {
        None
    };
    let exponent = |base: usize| -> (Option<usize>, Option<usize>) {
        if !b.get(base).is_some_and(|c| matches!(c, b'e' | b'E')) {
            return (None, None);
        }
        let signed = b.get(base + 1).is_some_and(|c| matches!(c, b'-' | b'+'));
        let digits = base + 1 + signed as usize;
        let end = decinteger(b, digits);
        (if end > digits { Some(end) } else { None }, if signed { Some(base + 2) } else { None })
    };
    let (mut real, mut real_fail) = (None::<usize>, None::<usize>);
    for base in [has_dec.then_some(dec), numeric].into_iter().flatten() {
        let (r, f) = exponent(base);
        real = real.max(r);
        real_fail = real_fail.max(f);
    }
    let junk = |end: Option<usize>| end.and_then(|e| identifier_from(b, e));
    let candidates = [
        (Number::Dec, has_dec.then_some(dec)),
        (Number::Hex, radix_integer(b, i, b'x', |c| c.is_ascii_hexdigit())),
        (Number::Oct, radix_integer(b, i, b'o', |c| (b'0'..=b'7').contains(&c))),
        (Number::Bin, radix_integer(b, i, b'b', |c| c == b'0' || c == b'1')),
        (Number::HexFail, radix_fail(b, i, b'x')),
        (Number::OctFail, radix_fail(b, i, b'o')),
        (Number::BinFail, radix_fail(b, i, b'b')),
        (Number::Numeric, numeric),
        (Number::NumericFail, (has_dec && b.get(dec) == Some(&b'.') && b.get(dec + 1) == Some(&b'.')).then_some(dec + 2)),
        (Number::Real, real),
        (Number::RealFail, real_fail),
        (Number::IntegerJunk, junk(has_dec.then_some(dec))),
        (Number::NumericJunk, junk(numeric)),
        (Number::RealJunk, junk(real)),
    ];
    let mut best: Option<(Number, usize)> = None;
    for (rule, end) in candidates {
        if let Some(end) = end {
            if best.is_none_or(|(_, e)| end > e) {
                best = Some((rule, end));
            }
        }
    }
    best
}

/// A quoted string's rules.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Quoted {
    Bit,
    Hex,
    Plain,
    Escaped,
    Unicode,
}

/// `{quotecontinue}` after an end quote at `after`: where the string goes on, if it does.
fn quote_continues(b: &[u8], after: usize) -> Option<usize> {
    let mut i = after;
    loop {
        match b.get(i) {
            Some(b' ' | b'\t' | 0x0c | 0x0b) => i += 1,
            Some(b'-') if b.get(i + 1) == Some(&b'-') => {
                while b.get(i).is_some_and(|c| !matches!(c, b'\n' | b'\r')) {
                    i += 1;
                }
            }
            Some(b'\n' | b'\r') => {
                i += 1;
                break;
            }
            _ => return None,
        }
    }
    loop {
        match b.get(i) {
            Some(&c) if is_space(c) => i += 1,
            Some(b'-') if b.get(i + 1) == Some(&b'-') => {
                while b.get(i).is_some_and(|c| !matches!(c, b'\n' | b'\r')) {
                    i += 1;
                }
                if b.get(i).is_none() {
                    return None;
                }
                i += 1;
            }
            Some(b'\'') => return Some(i + 1),
            _ => return None,
        }
    }
}

fn push_utf8(out: &mut Vec<u8>, c: u32) -> Option<()> {
    let mut buf = [0u8; 4];
    out.extend_from_slice(char::from_u32(c)?.encode_utf8(&mut buf).as_bytes());
    Some(())
}

fn hex_digits(b: &[u8], i: usize, max: usize) -> usize {
    let mut n = 0;
    while n < max && b.get(i + n).is_some_and(u8::is_ascii_hexdigit) {
        n += 1;
    }
    n
}

fn hex_value(b: &[u8]) -> u32 {
    b.iter().fold(0, |v, c| v * 16 + (*c as char).to_digit(16).unwrap_or(0))
}

struct Scanner<'a> {
    text: &'a str,
    b: &'a [u8],
}

impl Scanner<'_> {
    /// `message at or near "…"`, as `scanner_yyerror` writes it, the text cut where the current match ends.
    fn error(&self, message: &str, at: usize, end: usize) -> Stop {
        let message = if at >= self.b.len() {
            format!("{message} at end of input")
        } else {
            format!("{message} at or near \"{}\"", String::from_utf8_lossy(&self.b[at..end.min(self.b.len())]))
        };
        Stop { message, at: at as u32 }
    }

    /// A quoted string whose content starts at `i`; `start` is the token's start.
    fn quoted(&self, start: usize, mut i: usize, mode: Quoted, mut literal: Vec<u8>) -> Result<(Vec<u8>, usize), Stop> {
        let b = self.b;
        let n = b.len();
        let mut saw_non_ascii = false;
        loop {
            let Some(&c) = b.get(i) else {
                let what = match mode {
                    Quoted::Bit => "unterminated bit string literal",
                    Quoted::Hex => "unterminated hexadecimal string literal",
                    _ => "unterminated quoted string",
                };
                return Err(self.error(what, start, n));
            };
            if c == b'\'' {
                if b.get(i + 1) == Some(&b'\'') && !matches!(mode, Quoted::Bit | Quoted::Hex) {
                    literal.push(b'\'');
                    i += 2;
                    continue;
                }
                match quote_continues(b, i + 1) {
                    Some(next) => {
                        i = next;
                        continue;
                    }
                    None => {
                        if saw_non_ascii {
                            if let Some(message) = invalid_encoding(&literal) {
                                return Err(Stop { message, at: start as u32 });
                            }
                        }
                        return Ok((literal, i + 1));
                    }
                }
            }
            if mode != Quoted::Escaped || c != b'\\' {
                literal.push(c);
                i += 1;
                continue;
            }
            // A backslash in an escaped string.
            let Some(&e) = b.get(i + 1) else {
                literal.push(c);
                i += 1;
                continue;
            };
            match e {
                b'u' | b'U' => {
                    let want = if e == b'u' { 4 } else { 8 };
                    if hex_digits(b, i + 2, want) < want {
                        return Err(Stop { message: "invalid Unicode escape".to_string(), at: i as u32 });
                    }
                    let mut code = hex_value(&b[i + 2..i + 2 + want]);
                    let mut next = i + 2 + want;
                    if (0xD800..0xDC00).contains(&code) {
                        let (hi, at) = (code, next);
                        let w = match (b.get(at), b.get(at + 1)) {
                            (Some(b'\\'), Some(b'u')) => 4,
                            (Some(b'\\'), Some(b'U')) => 8,
                            _ => return Err(self.error("invalid Unicode surrogate pair", at, at + 1)),
                        };
                        if hex_digits(b, at + 2, w) < w {
                            return Err(Stop { message: "invalid Unicode escape".to_string(), at: at as u32 });
                        }
                        let lo = hex_value(&b[at + 2..at + 2 + w]);
                        if !(0xDC00..0xE000).contains(&lo) {
                            return Err(self.error("invalid Unicode surrogate pair", at, at + 2 + w));
                        }
                        code = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                        next = at + 2 + w;
                    } else if (0xDC00..0xE000).contains(&code) {
                        return Err(self.error("invalid Unicode surrogate pair", i, next));
                    }
                    if code == 0 || code > 0x10FFFF || push_utf8(&mut literal, code).is_none() {
                        return Err(self.error("invalid Unicode escape value", i, next));
                    }
                    i = next;
                }
                b'0'..=b'7' => {
                    let mut j = i + 1;
                    let mut value = 0u32;
                    while j < i + 4 && b.get(j).is_some_and(|d| (b'0'..=b'7').contains(d)) {
                        value = value * 8 + (b[j] - b'0') as u32;
                        j += 1;
                    }
                    let v = value as u8;
                    literal.push(v);
                    saw_non_ascii |= v == 0 || v >= 0x80;
                    i = j;
                }
                b'x' if hex_digits(b, i + 2, 2) > 0 => {
                    let d = hex_digits(b, i + 2, 2);
                    let v = hex_value(&b[i + 2..i + 2 + d]) as u8;
                    literal.push(v);
                    saw_non_ascii |= v == 0 || v >= 0x80;
                    i += 2 + d;
                }
                _ => {
                    let v = match e {
                        b'b' => 8,
                        b'f' => 12,
                        b'n' => b'\n',
                        b'r' => b'\r',
                        b't' => b'\t',
                        b'v' => 11,
                        other => {
                            saw_non_ascii |= other >= 0x80;
                            other
                        }
                    };
                    literal.push(v);
                    i += 2;
                }
            }
        }
    }

    /// A double-quoted name whose content starts at `i`.
    fn delimited(&self, start: usize, mut i: usize) -> Result<(Vec<u8>, usize), Stop> {
        let b = self.b;
        let mut name = Vec::new();
        loop {
            match b.get(i) {
                None => return Err(self.error("unterminated quoted identifier", start, b.len())),
                Some(b'"') if b.get(i + 1) == Some(&b'"') => {
                    name.push(b'"');
                    i += 2;
                }
                Some(b'"') => {
                    if name.is_empty() {
                        return Err(self.error("zero-length delimited identifier", start, i + 1));
                    }
                    return Ok((name, i + 1));
                }
                Some(&c) => {
                    name.push(c);
                    i += 1;
                }
            }
        }
    }

    /// `$tag$` at `i`: where it ends.
    fn dollar_delim(&self, i: usize) -> Option<usize> {
        let b = self.b;
        let mut j = i + 1;
        if b.get(j).copied().is_some_and(is_ident_start) {
            j += 1;
            while b.get(j).copied().is_some_and(is_dolq_cont) {
                j += 1;
            }
        }
        (b.get(j) == Some(&b'$')).then_some(j + 1)
    }

    fn scan(&self, out: &mut Vec<Token>) -> Result<(), Stop> {
        let b = self.b;
        let n = b.len();
        let mut i = 0usize;
        macro_rules! emit {
            ($kind:expr, $start:expr, $end:expr, $value:expr) => {{
                out.push(Token { kind: $kind, start: $start as u32, end: $end as u32, value: $value });
                i = $end;
                continue;
            }};
        }
        while i < n {
            let c = b[i];
            let next = b.get(i + 1).copied();
            if c == 0 {
                return Err(self.error("syntax error", i, i + 1));
            }
            if is_space(c) {
                while b.get(i).copied().is_some_and(is_space) {
                    i += 1;
                }
                continue;
            }
            if c == b'-' && next == Some(b'-') {
                let mut j = i;
                while b.get(j).is_some_and(|c| !matches!(c, b'\n' | b'\r')) {
                    j += 1;
                }
                emit!(tok::SQL_COMMENT, i, j, Value::None);
            }
            if c == b'/' && next == Some(b'*') {
                let start = i;
                let (mut j, mut depth) = (i + 2, 0i32);
                loop {
                    match b.get(j) {
                        None => return Err(self.error("unterminated /* comment", start, n)),
                        Some(b'/') if b.get(j + 1) == Some(&b'*') => {
                            depth += 1;
                            j += 2;
                        }
                        Some(b'*') => {
                            let mut k = j;
                            while b.get(k) == Some(&b'*') {
                                k += 1;
                            }
                            if b.get(k) == Some(&b'/') {
                                j = k + 1;
                                if depth <= 0 {
                                    break;
                                }
                                depth -= 1;
                            } else {
                                j = k;
                            }
                        }
                        Some(_) => j += 1,
                    }
                }
                emit!(tok::C_COMMENT, start, j, Value::None);
            }
            if next == Some(b'\'') {
                match c {
                    b'b' | b'B' => {
                        let (v, e) = self.quoted(i, i + 2, Quoted::Bit, vec![b'b'])?;
                        emit!(tok::BCONST, i, e, Value::Bytes(v));
                    }
                    b'x' | b'X' => {
                        let (v, e) = self.quoted(i, i + 2, Quoted::Hex, vec![b'x'])?;
                        emit!(tok::XCONST, i, e, Value::Bytes(v));
                    }
                    b'n' | b'N' => {
                        let k = keyword(b"nchar").expect("nchar is a keyword");
                        emit!(k.1, i, i + 1, Value::Keyword(k.0));
                    }
                    b'e' | b'E' => {
                        let (v, e) = self.quoted(i, i + 2, Quoted::Escaped, Vec::new())?;
                        emit!(tok::SCONST, i, e, Value::Bytes(v));
                    }
                    _ => {}
                }
            }
            if matches!(c, b'u' | b'U') && next == Some(b'&') {
                match b.get(i + 2) {
                    Some(b'\'') => {
                        let (v, e) = self.quoted(i, i + 3, Quoted::Unicode, Vec::new())?;
                        emit!(tok::USCONST, i, e, Value::Bytes(v));
                    }
                    Some(b'"') => {
                        let (v, e) = self.delimited(i, i + 3)?;
                        emit!(tok::UIDENT, i, e, Value::Bytes(v));
                    }
                    _ => emit!(tok::IDENT, i, i + 1, Value::Bytes(vec![c.to_ascii_lowercase()])),
                }
            }
            if is_ident_start(c) {
                let end = identifier_from(b, i).expect("an identifier starts here");
                let word = &b[i..end];
                if let Some(k) = keyword(word) {
                    emit!(k.1, i, end, Value::Keyword(k.0));
                }
                emit!(tok::IDENT, i, end, Value::Bytes(downcase_truncate(word)));
            }
            if c == b'\'' {
                let (v, e) = self.quoted(i, i + 1, Quoted::Plain, Vec::new())?;
                emit!(tok::SCONST, i, e, Value::Bytes(v));
            }
            if c == b'"' {
                let (v, e) = self.delimited(i, i + 1)?;
                emit!(tok::IDENT, i, e, Value::Bytes(truncate(v)));
            }
            if c == b'$' {
                if let Some(end) = self.dollar_delim(i) {
                    let delim = &b[i..end];
                    let Some(at) = b[end..].windows(delim.len()).position(|w| w == delim) else {
                        return Err(self.error("unterminated dollar-quoted string", i, n));
                    };
                    let close = end + at;
                    emit!(tok::SCONST, i, close + delim.len(), Value::Bytes(b[end..close].to_vec()));
                }
                if next.is_some_and(|d| d.is_ascii_digit()) {
                    let mut j = i + 1;
                    while b.get(j).is_some_and(u8::is_ascii_digit) {
                        j += 1;
                    }
                    let Some(v) = int32(&b[i + 1..j]) else {
                        return Err(self.error("parameter number too large", i, j));
                    };
                    emit!(tok::PARAM, i, j, Value::Int(v));
                }
                emit!(char_token(c), i, i + 1, Value::None);
            }
            if c.is_ascii_digit() || (c == b'.' && next.is_some_and(|d| d.is_ascii_digit())) {
                let (rule, end) = number(b, i).expect("a number starts here");
                let text = &b[i..end];
                let integer = |text: &[u8], end: usize| match int32(text) {
                    Some(v) => (tok::ICONST, end, Value::Int(v)),
                    None => (tok::FCONST, end, Value::Bytes(text.to_vec())),
                };
                let (kind, end, value) = match rule {
                    Number::Dec | Number::Hex | Number::Oct | Number::Bin => integer(text, end),
                    Number::NumericFail => integer(&b[i..end - 2], end - 2),
                    Number::Numeric | Number::Real => (tok::FCONST, end, Value::Bytes(text.to_vec())),
                    Number::HexFail => return Err(self.error("invalid hexadecimal integer", i, end)),
                    Number::OctFail => return Err(self.error("invalid octal integer", i, end)),
                    Number::BinFail => return Err(self.error("invalid binary integer", i, end)),
                    Number::RealFail | Number::IntegerJunk | Number::NumericJunk | Number::RealJunk => return Err(self.error("trailing junk after numeric literal", i, end)),
                };
                emit!(kind, i, end, value);
            }
            match (c, next) {
                (b':', Some(b':')) => emit!(tok::TYPECAST, i, i + 2, Value::None),
                (b'.', Some(b'.')) => emit!(tok::DOT_DOT, i, i + 2, Value::None),
                (b':', Some(b'=')) => emit!(tok::COLON_EQUALS, i, i + 2, Value::None),
                _ => {}
            }
            if is_op_char(c) {
                let mut run = i;
                while b.get(run).copied().is_some_and(is_op_char) {
                    run += 1;
                }
                let len = run - i;
                if len == 2 {
                    if let Some(kind) = two_char_operator(c, b[i + 1]) {
                        emit!(kind, i, i + 2, Value::None);
                    }
                }
                if len == 1 && is_self(c) {
                    emit!(char_token(c), i, i + 1, Value::None);
                }
                let text = &b[i..run];
                let mut chars = len;
                let cut = [find(text, b"/*"), find(text, b"--")].into_iter().flatten().min();
                if let Some(cut) = cut {
                    chars = cut;
                }
                if chars > 1 && matches!(text[chars - 1], b'+' | b'-') && !text[..chars - 1].iter().any(|c| matches!(c, b'~' | b'!' | b'@' | b'#' | b'^' | b'&' | b'|' | b'`' | b'?' | b'%')) {
                    loop {
                        chars -= 1;
                        if !(chars > 1 && matches!(text[chars - 1], b'+' | b'-')) {
                            break;
                        }
                    }
                }
                if chars < len {
                    if chars == 1 && is_self(text[0]) {
                        emit!(char_token(text[0]), i, i + 1, Value::None);
                    }
                    if chars == 2 {
                        if let Some(kind) = two_char_operator(text[0], text[1]) {
                            emit!(kind, i, i + 2, Value::None);
                        }
                    }
                }
                if chars >= MAX_IDENTIFIER + 1 {
                    return Err(self.error("operator too long", i, i + chars));
                }
                emit!(tok::Op, i, i + chars, Value::Bytes(text[..chars].to_vec()));
            }
            emit!(char_token(c), i, i + 1, Value::None);
        }
        let _ = self.text;
        Ok(())
    }
}

/// `pg_verifymbstr` over a literal an escape may have left outside UTF-8: Postgres's words for
/// the first byte sequence that is no UTF-8, or a NUL.
fn invalid_encoding(literal: &[u8]) -> Option<String> {
    let nul = literal.iter().position(|c| *c == 0);
    let bad = std::str::from_utf8(literal).err().map(|e| e.valid_up_to());
    let at = match (nul, bad) {
        (Some(n), Some(b)) => n.min(b),
        (Some(n), None) => n,
        (None, Some(b)) => b,
        (None, None) => return None,
    };
    let first = literal[at];
    let width = match first {
        c if c & 0xe0 == 0xc0 => 2,
        c if c & 0xf0 == 0xe0 => 3,
        c if c & 0xf8 == 0xf0 => 4,
        _ => 1,
    };
    let bytes: Vec<String> = literal[at..(at + width).min(literal.len())].iter().map(|c| format!("0x{c:02x}")).collect();
    Some(format!("invalid byte sequence for encoding \"UTF8\": {}", bytes.join(" ")))
}

fn find(text: &[u8], what: &[u8]) -> Option<usize> {
    text.windows(what.len()).position(|w| w == what)
}

fn is_self(c: u8) -> bool {
    matches!(c, b',' | b'(' | b')' | b'[' | b']' | b'.' | b';' | b':' | b'+' | b'-' | b'*' | b'/' | b'%' | b'^' | b'<' | b'>' | b'=')
}

fn two_char_operator(a: u8, b: u8) -> Option<u16> {
    Some(match (a, b) {
        (b'=', b'>') => tok::EQUALS_GREATER,
        (b'>', b'=') => tok::GREATER_EQUALS,
        (b'<', b'=') => tok::LESS_EQUALS,
        (b'<', b'>') | (b'!', b'=') => tok::NOT_EQUALS,
        _ => return None,
    })
}

/// A character's token; one the grammar names none for reads as the invalid token, which no rule takes.
fn char_token(c: u8) -> u16 {
    match CHAR_TOKENS.get(c as usize) {
        Some(&t) if t != 0 => t,
        _ => 2,
    }
}

/// The text's tokens, comments included, as `scan.l` reads them.
pub fn scan(text: &str) -> Result<Vec<Token>, (Vec<Token>, Stop)> {
    let mut out = Vec::with_capacity(text.len() / 4);
    match (Scanner { text, b: text.as_bytes() }).scan(&mut out) {
        Ok(()) => Ok(out),
        Err(stop) => Err((out, stop)),
    }
}

fn is_comment(kind: u16) -> bool {
    kind == tok::SQL_COMMENT || kind == tok::C_COMMENT
}

/// A token's kind as the grammar reads it, given the token after it (`base_yylex`).
fn with_lookahead(kind: u16, next: Option<u16>) -> u16 {
    let Some(next) = next else { return kind };
    match kind {
        tok::FORMAT if next == tok::JSON => tok::FORMAT_LA,
        tok::NOT if matches!(next, tok::BETWEEN | tok::IN_P | tok::LIKE | tok::ILIKE | tok::SIMILAR) => tok::NOT_LA,
        tok::NULLS_P if matches!(next, tok::FIRST_P | tok::LAST_P) => tok::NULLS_LA,
        tok::WITH if matches!(next, tok::TIME | tok::ORDINALITY) => tok::WITH_LA,
        tok::WITHOUT if next == tok::TIME => tok::WITHOUT_LA,
        k => k,
    }
}

/// A token for the grammar: its kind and span, with its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    pub lexeme: (u16, u32, u32, u32),
    pub value: Value,
}

/// The tokens as the grammar reads them: comments out, the lookahead applied, and `U&` names and
/// strings decoded, an `UESCAPE` and its string taken into the token before them.
pub fn read(text: &str) -> Result<(Vec<Read>, u32), (Vec<Read>, u32, Stop)> {
    let (raw, stop) = match scan(text) {
        Ok(raw) => (raw, None),
        Err((raw, stop)) => (raw, Some(stop)),
    };
    let significant: Vec<usize> = (0..raw.len()).filter(|i| !is_comment(raw[*i].kind)).collect();
    let mut out = Vec::with_capacity(significant.len());
    let mut lead = 0u32;
    let mut k = 0usize;
    // Where the scanner stopped, a token that looks one ahead is never handed over: reading it is what stopped the scanner.
    let looks_ahead = |kind: u16| matches!(kind, tok::FORMAT | tok::NOT | tok::NULLS_P | tok::WITH | tok::WITHOUT | tok::UIDENT | tok::USCONST);
    let usable = match (&stop, significant.last()) {
        (Some(_), Some(&last)) if looks_ahead(raw[last].kind) => significant.len() - 1,
        _ => significant.len(),
    };
    while k < usable {
        let t = &raw[significant[k]];
        let next = significant.get(k + 1).map(|i| raw[*i].kind);
        let (mut kind, mut end, mut value) = (with_lookahead(t.kind, next), t.end, t.value.clone());
        if t.kind == tok::UIDENT || t.kind == tok::USCONST {
            let mut escape = b'\\';
            if next == Some(tok::UESCAPE) {
                let third = significant.get(k + 2).map(|i| &raw[*i]);
                let near = |message: &str| {
                    let (at, end) = third.map_or((text.len(), text.len()), |t| (t.start as usize, t.end as usize));
                    (Scanner { text, b: text.as_bytes() }).error(message, at, end)
                };
                if third.is_none() {
                    if let Some(stop) = stop {
                        return Err((out, lead, stop));
                    }
                }
                let Some(Token { kind: tok::SCONST, end: third_end, value: Value::Bytes(esc), .. }) = third else {
                    return Err((out, lead, near("UESCAPE must be followed by a simple string literal")));
                };
                if esc.len() != 1 || !valid_uescape(esc[0]) {
                    return Err((out, lead, near("invalid Unicode escape character")));
                }
                escape = esc[0];
                end = *third_end;
                k += 2;
            }
            let Value::Bytes(bytes) = &value else { unreachable!("a U& token holds its text") };
            let decoded = match deescape(bytes, escape) {
                Ok(d) => d,
                Err(message) => return Err((out, lead, Stop { message, at: t.start })),
            };
            if t.kind == tok::UIDENT {
                kind = tok::IDENT;
                value = Value::Bytes(truncate(decoded));
            } else {
                kind = tok::SCONST;
                value = Value::Bytes(decoded);
            }
        }
        out.push(Read { lexeme: (kind, lead, t.start, end), value });
        lead = end;
        k += 1;
    }
    match stop {
        Some(stop) => Err((out, lead, stop)),
        None => Ok((out, lead)),
    }
}

/// [`read`]'s tokens as the parser takes them.
pub fn lexemes(text: &str) -> Result<(Vec<Lexeme>, u32), ScanError> {
    let as_lexeme = |r: &Read| Lexeme { sym: r.lexeme.0, lead: r.lexeme.1, start: r.lexeme.2, end: r.lexeme.3 };
    match read(text) {
        Ok((tokens, tail)) => Ok((tokens.iter().map(as_lexeme).collect(), tail)),
        Err((tokens, lead, stop)) => Err(ScanError { message: stop.message, at: stop.at, read: (tokens.iter().map(as_lexeme).collect(), lead) }),
    }
}

/// `check_uescapechar`.
fn valid_uescape(c: u8) -> bool {
    !(c.is_ascii_hexdigit() || c == b'+' || c == b'\'' || c == b'"' || is_space(c))
}

/// `str_udeescape`.
fn deescape(s: &[u8], escape: u8) -> Result<Vec<u8>, String> {
    let (mut i, mut out, mut pair) = (0usize, Vec::with_capacity(s.len()), 0u32);
    let bad_pair = || "invalid Unicode surrogate pair".to_string();
    while i < s.len() {
        if s[i] != escape {
            if pair != 0 {
                return Err(bad_pair());
            }
            out.push(s[i]);
            i += 1;
            continue;
        }
        if s.get(i + 1) == Some(&escape) {
            if pair != 0 {
                return Err(bad_pair());
            }
            out.push(escape);
            i += 2;
            continue;
        }
        let (mut code, width) = if hex_digits(s, i + 1, 4) == 4 {
            (hex_value(&s[i + 1..i + 5]), 5)
        } else if s.get(i + 1) == Some(&b'+') && hex_digits(s, i + 2, 6) == 6 {
            (hex_value(&s[i + 2..i + 8]), 8)
        } else {
            return Err("invalid Unicode escape".to_string());
        };
        if code == 0 || code > 0x10FFFF {
            return Err("invalid Unicode escape value".to_string());
        }
        if pair != 0 {
            if !(0xDC00..0xE000).contains(&code) {
                return Err(bad_pair());
            }
            code = 0x10000 + ((pair - 0xD800) << 10) + (code - 0xDC00);
            pair = 0;
        } else if (0xDC00..0xE000).contains(&code) {
            return Err(bad_pair());
        }
        if (0xD800..0xDC00).contains(&code) {
            pair = code;
        } else if push_utf8(&mut out, code).is_none() {
            return Err("invalid Unicode escape value".to_string());
        }
        i += width;
    }
    if pair != 0 {
        return Err(bad_pair());
    }
    Ok(out)
}
