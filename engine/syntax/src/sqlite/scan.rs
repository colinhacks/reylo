//! A port of SQLite's tokenizer (`tokenize.c`, `sqlite3GetToken` and the loop of `sqlite3RunParser` around it), which is in the public domain. A byte past the end of the text reads as NUL, as it does in the C string SQLite reads, and a NUL ends the text.

use super::generated::*;
use crate::lalr::Lexeme;

const CC_X: u8 = 0;
const CC_KYWD0: u8 = 1;
const CC_KYWD: u8 = 2;
const CC_DIGIT: u8 = 3;
const CC_DOLLAR: u8 = 4;
const CC_VARALPHA: u8 = 5;
const CC_VARNUM: u8 = 6;
const CC_SPACE: u8 = 7;
const CC_QUOTE: u8 = 8;
const CC_QUOTE2: u8 = 9;
const CC_PIPE: u8 = 10;
const CC_MINUS: u8 = 11;
const CC_LT: u8 = 12;
const CC_GT: u8 = 13;
const CC_EQ: u8 = 14;
const CC_BANG: u8 = 15;
const CC_SLASH: u8 = 16;
const CC_LP: u8 = 17;
const CC_RP: u8 = 18;
const CC_SEMI: u8 = 19;
const CC_PLUS: u8 = 20;
const CC_STAR: u8 = 21;
const CC_PERCENT: u8 = 22;
const CC_COMMA: u8 = 23;
const CC_AND: u8 = 24;
const CC_TILDA: u8 = 25;
const CC_DOT: u8 = 26;
const CC_ID: u8 = 27;
const CC_NUL: u8 = 29;
const CC_BOM: u8 = 30;

const DIGIT_SEPARATOR: u8 = b'_';

#[inline]
fn is_space(c: u8) -> bool {
    CTYPE_MAP[c as usize] & 0x01 != 0
}

#[inline]
fn is_digit(c: u8) -> bool {
    CTYPE_MAP[c as usize] & 0x04 != 0
}

#[inline]
fn is_xdigit(c: u8) -> bool {
    CTYPE_MAP[c as usize] & 0x08 != 0
}

#[inline]
fn is_id_char(c: u8) -> bool {
    CTYPE_MAP[c as usize] & 0x46 != 0
}

/// The keyword's token, or `TK_ID`.
fn keyword(word: &[u8]) -> u16 {
    if word.len() < 2 || word.len() > 20 {
        return TK_ID;
    }
    let mut upper = [0u8; 20];
    for (u, c) in upper.iter_mut().zip(word) {
        *u = c.to_ascii_uppercase();
    }
    let upper = &upper[..word.len()];
    KEYWORDS.binary_search_by(|(k, _)| k.as_bytes().cmp(upper)).map_or(TK_ID, |i| KEYWORDS[i].1)
}

/// The length and kind of the token that starts at `at`.
pub fn get_token(text: &[u8], at: usize) -> (usize, u16) {
    let z = |i: usize| -> u8 { text.get(at + i).copied().unwrap_or(0) };
    let mut i: usize;
    let class = AI_CLASS[z(0) as usize];
    match class {
        CC_SPACE => {
            i = 1;
            while is_space(z(i)) {
                i += 1;
            }
            return (i, TK_SPACE);
        }
        CC_MINUS => {
            if z(1) == b'-' {
                i = 2;
                while z(i) != 0 && z(i) != b'\n' {
                    i += 1;
                }
                return (i, TK_COMMENT);
            } else if z(1) == b'>' {
                return (2 + (z(2) == b'>') as usize, TK_PTR);
            }
            return (1, TK_MINUS);
        }
        CC_LP => return (1, TK_LP),
        CC_RP => return (1, TK_RP),
        CC_SEMI => return (1, TK_SEMI),
        CC_PLUS => return (1, TK_PLUS),
        CC_STAR => return (1, TK_STAR),
        CC_SLASH => {
            if z(1) != b'*' || z(2) == 0 {
                return (1, TK_SLASH);
            }
            i = 3;
            let mut c = z(2);
            while c != b'*' || z(i) != b'/' {
                c = z(i);
                if c == 0 {
                    break;
                }
                i += 1;
            }
            if c != 0 {
                i += 1;
            }
            return (i, TK_COMMENT);
        }
        CC_PERCENT => return (1, TK_REM),
        CC_EQ => return (1 + (z(1) == b'=') as usize, TK_EQ),
        CC_LT => {
            return match z(1) {
                b'=' => (2, TK_LE),
                b'>' => (2, TK_NE),
                b'<' => (2, TK_LSHIFT),
                _ => (1, TK_LT),
            };
        }
        CC_GT => {
            return match z(1) {
                b'=' => (2, TK_GE),
                b'>' => (2, TK_RSHIFT),
                _ => (1, TK_GT),
            };
        }
        CC_BANG => return if z(1) != b'=' { (1, TK_ILLEGAL) } else { (2, TK_NE) },
        CC_PIPE => return if z(1) != b'|' { (1, TK_BITOR) } else { (2, TK_CONCAT) },
        CC_COMMA => return (1, TK_COMMA),
        CC_AND => return (1, TK_BITAND),
        CC_TILDA => return (1, TK_BITNOT),
        CC_QUOTE => {
            let delim = z(0);
            let mut c;
            i = 1;
            loop {
                c = z(i);
                if c == 0 {
                    break;
                }
                if c == delim {
                    if z(i + 1) == delim {
                        i += 1;
                    } else {
                        break;
                    }
                }
                i += 1;
            }
            return if c == b'\'' {
                (i + 1, TK_STRING)
            } else if c != 0 {
                (i + 1, TK_ID)
            } else {
                (i, TK_ILLEGAL)
            };
        }
        CC_DOT | CC_DIGIT => {
            if class == CC_DOT && !is_digit(z(1)) {
                return (1, TK_DOT);
            }
            let mut kind = TK_INTEGER;
            // A run of digits, each kept by `keep`, in which a separator makes the number a QNUMBER.
            let run = |mut at: usize, kind: &mut u16, keep: &dyn Fn(u8) -> bool| -> usize {
                loop {
                    if !keep(z(at)) {
                        if z(at) == DIGIT_SEPARATOR {
                            *kind = TK_QNUMBER;
                        } else {
                            return at;
                        }
                    }
                    at += 1;
                }
            };
            if z(0) == b'0' && (z(1) == b'x' || z(1) == b'X') && is_xdigit(z(2)) {
                i = run(3, &mut kind, &is_xdigit);
            } else {
                i = run(0, &mut kind, &is_digit);
                if z(i) == b'.' {
                    if kind == TK_INTEGER {
                        kind = TK_FLOAT;
                    }
                    i = run(i + 1, &mut kind, &is_digit);
                }
                if (z(i) == b'e' || z(i) == b'E') && (is_digit(z(i + 1)) || ((z(i + 1) == b'+' || z(i + 1) == b'-') && is_digit(z(i + 2)))) {
                    if kind == TK_INTEGER {
                        kind = TK_FLOAT;
                    }
                    i = run(i + 2, &mut kind, &is_digit);
                }
            }
            while is_id_char(z(i)) {
                kind = TK_ILLEGAL;
                i += 1;
            }
            return (i, kind);
        }
        CC_QUOTE2 => {
            i = 1;
            let mut c = z(0);
            while c != b']' {
                c = z(i);
                if c == 0 {
                    break;
                }
                i += 1;
            }
            return (i, if c == b']' { TK_ID } else { TK_ILLEGAL });
        }
        CC_VARNUM => {
            i = 1;
            while is_digit(z(i)) {
                i += 1;
            }
            return (i, TK_VARIABLE);
        }
        CC_DOLLAR | CC_VARALPHA => {
            let mut n = 0;
            let mut kind = TK_VARIABLE;
            i = 1;
            loop {
                let c = z(i);
                if c == 0 {
                    break;
                }
                if is_id_char(c) {
                    n += 1;
                } else if c == b'(' && n > 0 {
                    loop {
                        i += 1;
                        let d = z(i);
                        if d == 0 || is_space(d) || d == b')' {
                            break;
                        }
                    }
                    if z(i) == b')' {
                        i += 1;
                    } else {
                        kind = TK_ILLEGAL;
                    }
                    break;
                } else if c == b':' && z(i + 1) == b':' {
                    i += 1;
                } else {
                    break;
                }
                i += 1;
            }
            if n == 0 {
                kind = TK_ILLEGAL;
            }
            return (i, kind);
        }
        CC_KYWD0 => {
            if AI_CLASS[z(1) as usize] > CC_KYWD {
                i = 1;
            } else {
                i = 2;
                while AI_CLASS[z(i) as usize] <= CC_KYWD {
                    i += 1;
                }
                if is_id_char(z(i)) {
                    i += 1;
                } else {
                    return (i, keyword(&text[at..at + i]));
                }
            }
        }
        CC_X => {
            if z(1) == b'\'' {
                let mut kind = TK_BLOB;
                i = 2;
                while is_xdigit(z(i)) {
                    i += 1;
                }
                if z(i) != b'\'' || i % 2 != 0 {
                    kind = TK_ILLEGAL;
                    while z(i) != 0 && z(i) != b'\'' {
                        i += 1;
                    }
                }
                if z(i) != 0 {
                    i += 1;
                }
                return (i, kind);
            }
            i = 1;
        }
        CC_KYWD | CC_ID => i = 1,
        CC_BOM => {
            if z(1) == 0xbb && z(2) == 0xbf {
                return (3, TK_SPACE);
            }
            i = 1;
        }
        CC_NUL => return (0, TK_ILLEGAL),
        _ => return (1, TK_ILLEGAL),
    }
    while is_id_char(z(i)) {
        i += 1;
    }
    (i, TK_ID)
}

/// The next token after `at` that isn't space or a comment, as the window keywords' lookahead sees it, and where it ends.
fn next_token(text: &[u8], mut at: usize) -> (u16, usize) {
    let mut t;
    loop {
        let (n, kind) = get_token(text, at);
        at += n;
        t = kind;
        if t != TK_SPACE && t != TK_COMMENT {
            break;
        }
    }
    if t == TK_ID || t == TK_STRING || t == TK_JOIN_KW || t == TK_WINDOW || t == TK_OVER || YY_FALLBACK.get(t as usize) == Some(&TK_ID) {
        t = TK_ID;
    }
    (t, at)
}

fn window_keyword(text: &[u8], at: usize) -> u16 {
    let (t, at) = next_token(text, at);
    if t != TK_ID {
        return TK_ID;
    }
    if next_token(text, at).0 != TK_AS {
        return TK_ID;
    }
    TK_WINDOW
}

fn over_keyword(text: &[u8], at: usize, last: Option<u16>) -> u16 {
    if last == Some(TK_RP) {
        let t = next_token(text, at).0;
        if t == TK_LP || t == TK_ID {
            return TK_OVER;
        }
    }
    TK_ID
}

fn filter_keyword(text: &[u8], at: usize, last: Option<u16>) -> u16 {
    if last == Some(TK_RP) && next_token(text, at).0 == TK_LP {
        return TK_FILTER;
    }
    TK_ID
}

/// The tokens of `text` as SQLite's parser is handed them, a closing `;` added where the last statement has none, and where the trivia after the last token starts. A token the scanner can't read stays in the list as `TK_ILLEGAL`.
pub fn lexemes(text: &str) -> (Vec<Lexeme>, u32) {
    let z = text.as_bytes();
    let mut out: Vec<Lexeme> = Vec::with_capacity(z.len() / 4 + 2);
    let mut at = 0usize;
    let mut lead = 0usize;
    let mut last: Option<u16> = None;
    loop {
        let (n, mut t) = get_token(z, at);
        if t >= TK_WINDOW {
            if t == TK_SPACE {
                at += n;
                continue;
            }
            if z.get(at).copied().unwrap_or(0) == 0 {
                break;
            }
            match t {
                TK_WINDOW => t = window_keyword(z, at + 6),
                TK_OVER => t = over_keyword(z, at + 4, last),
                TK_FILTER => t = filter_keyword(z, at + 6, last),
                TK_COMMENT => {
                    at += n;
                    continue;
                }
                _ => {}
            }
        }
        out.push(Lexeme { sym: t, lead: lead as u32, start: at as u32, end: (at + n) as u32 });
        last = Some(t);
        at += n;
        lead = at;
    }
    if last != Some(TK_SEMI) {
        out.push(Lexeme { sym: TK_SEMI, lead: lead as u32, start: lead as u32, end: lead as u32 });
    }
    (out, lead as u32)
}
