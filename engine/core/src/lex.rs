//! Placeholders, read off the raw statement text.
//!
//! One named form works in every dialect, `:name`, which is also the form mysql2's
//! `namedPlaceholders` and SQLite accept; SQLite's own `@name` and `$name` are named placeholders
//! there too. Beside them a statement may use its database's own positional form instead, never
//! both: `$1` on Postgres, `?` on MySQL, `?` and `?NNN` on SQLite, and `$1`, `?` and `?NNN` on
//! DuckDB. A form the database does not
//! have is refused where the database would misread it: SQLite takes `$1` for a parameter named
//! `$1`, a Postgres `$name` outside a dollar quote is a syntax error, and DuckDB takes `$name` for a
//! parameter Reylo does not bind, so each is an error that
//! says what to write; MySQL's `@name` is a user variable and `$` a character of its identifiers,
//! so both stay text there, as `@` (an operator) does on Postgres. A parameter's name drops the
//! prefix: `:id`, `@id` and `$id` are `id`, and `$2` and `?2` are both `2`. SQLite reads `:id` and
//! `@id` as two parameters, so the named placeholders of one statement share one prefix. A bare `?` takes the number after the largest one so far, as SQLite numbers it, so `?, ?`
//! is `1`, `2`. Each parameter appears once in a statement, so that one place types it: a value
//! needed twice is selected once in a `with` and read from there, and every later use of a name
//! is an error at its place ([`repeated`]); a bare `?` is a parameter of its own. A name may be
//! dotted (`:auth.user_id`): each `.segment` starts like a name, so `arr[1:2]` and `:t.*` end
//! where they should. The engine rewrites placeholders to whatever the
//! database binds by position (`$n` on Postgres and DuckDB, `?` on MySQL, `?n` for the SQLite prepare, where
//! SQLite's own `:name` would end at the first dot) and keeps a map back to the author's text, so
//! a server error at a rewritten position lands on the right characters of the `.sql` file.
//!
//! The pass knows what the parser knows about text: single-quoted literals (with `''`), `E''`
//! escape strings (with backslashes), dollar quoting (Postgres's and DuckDB's), double-quoted identifiers, line comments,
//! nested block comments, and `::` casts. MySQL reads text its own way, by the session's
//! `sql_mode` ([`MysqlText`]): a backslash in a string escapes the character after it, a
//! double-quoted word is a string, `#` starts a line comment as `-- ` does (`--` followed by
//! anything else is two minus signs), a block comment ends at its first `*/`, and backquotes quote
//! a name. The spans it skipped as text are returned as [`Lexed::opaque`], so a caller that looks
//! for its own construct in the statement does not need a scanner of its own.

use crate::dialect::Dialect;
use serde::Serialize;

/// One placeholder occurrence, by byte offsets in the original text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Placeholder {
    /// The parameter name: the author's for a named placeholder (dots included, prefix dropped),
    /// and the position's number for a positional one (`$3` and SQLite's `?3` are both `3`; a
    /// bare `?` is one past the largest number before it).
    pub name: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    None,
    Named,
    Positional,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lexed {
    /// Distinct parameter names in binding order: first appearance for named parameters, and
    /// by number for positional ones, so `params[k]` is the statement's `$(k + 1)`.
    pub params: Vec<String>,
    pub placeholders: Vec<Placeholder>,
    pub style: Style,
    /// Every span the walk read as text rather than code, `(start, end)` in byte offsets, in
    /// source order and never overlapping: string literals (an `E''` string from its `E`), dollar
    /// quotes, quoted identifiers, line comments (up to their newline) and block comments.
    pub opaque: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LexError {
    pub message: String,
    pub offset: usize,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (at byte {})", self.message, self.offset)
    }
}

impl std::error::Error for LexError {}

/// The placeholder syntax a rewrite emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// `$n`, numbered by binding order.
    Dollar,
    /// `?`, one per occurrence, in text order; the binding order repeats a name where it repeats.
    Question,
    /// `?n`, numbered by binding order (the parameter's 1-based index in [`Lexed::params`]), so a
    /// repeated name is one position. An author's `$1 ... $n` keeps its numbers; unlike
    /// [`Target::Dollar`], a gap closes (`$3` alone is `?1`): the binding order is `params` either
    /// way, and SQLite reads `?n` as position `n`.
    Numbered,
}

/// A rewritten statement plus the map back to the author's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rewritten {
    pub sql: String,
    /// The binding order: for `?` one entry per occurrence, for `$n` and `?n` one per distinct name.
    pub order: Vec<String>,
    /// Per placeholder in text order: `(original start, original end, rewritten start, rewritten end)`.
    pub spans: Vec<(usize, usize, usize, usize)>,
    /// Text the rewrite adds that the original does not hold, in text order: `(original offset,
    /// rewritten start, rewritten end)`, as the collation a MySQL statement's own text needs
    /// (the MySQL engine's `mysql_collation`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inserted: Vec<(usize, usize, usize)>,
}

impl Rewritten {
    /// The byte offset in the original text that a byte offset in the rewritten text refers to.
    pub fn to_original(&self, pos: usize) -> usize {
        let mut delta: isize = 0;
        let mut regions = self.spans.clone();
        regions.extend(self.inserted.iter().map(|&(at, rs, re)| (at, at, rs, re)));
        regions.sort_by_key(|&(_, _, rs, _)| rs);
        for (os, oe, rs, re) in regions {
            if pos >= re {
                delta += (oe - os) as isize - (re - rs) as isize;
            } else if pos >= rs {
                return os;
            } else {
                break;
            }
        }
        (pos as isize + delta).max(0) as usize
    }
}

fn is_name_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn prev_is_name_char(b: &[u8], i: usize) -> bool {
    i > 0 && is_name_char(b[i - 1])
}

fn name_end(b: &[u8], start: usize) -> usize {
    let mut j = start;
    while j < b.len() && is_name_char(b[j]) {
        j += 1;
    }
    j
}

/// A placeholder name from `start`: a name, then any `.segment` that starts like one
/// (`:auth.user_id`). `:t.*` stops before the dot.
fn dotted_name_end(b: &[u8], start: usize) -> usize {
    let mut j = name_end(b, start);
    while b.get(j) == Some(&b'.') && b.get(j + 1).is_some_and(|c| is_name_start(*c)) {
        j = name_end(b, j + 1);
    }
    j
}

fn digits_end(b: &[u8], start: usize) -> usize {
    let mut j = start;
    while j < b.len() && b[j].is_ascii_digit() {
        j += 1;
    }
    j
}

/// `$tag$` at `i`: the offset one past its closing `$`, when `i` starts a dollar-quote opener.
fn dollar_tag_end(b: &[u8], i: usize) -> Option<usize> {
    if b.get(i) != Some(&b'$') {
        return None;
    }
    let mut j = i + 1;
    if j < b.len() && is_name_start(b[j]) {
        j = name_end(b, j);
    }
    (b.get(j) == Some(&b'$')).then_some(j + 1)
}

fn find_bytes(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= hay.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

/// How a MySQL session reads strings, by its `sql_mode`. The default is the server's default
/// mode, which is also how `sqlparser`'s MySQL dialect reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MysqlText {
    /// A backslash in a string escapes the character after it (`'it\'s'`), unless the mode has
    /// NO_BACKSLASH_ESCAPES.
    pub backslash_escapes: bool,
    /// A double-quoted word is a name, not a string, where the mode has ANSI_QUOTES. A name
    /// escapes nothing with a backslash.
    pub ansi_quotes: bool,
}

impl Default for MysqlText {
    fn default() -> Self {
        MysqlText { backslash_escapes: true, ansi_quotes: false }
    }
}

impl MysqlText {
    /// Every reading a session may have.
    pub const ALL: [MysqlText; 4] = [
        MysqlText { backslash_escapes: true, ansi_quotes: false },
        MysqlText { backslash_escapes: false, ansi_quotes: false },
        MysqlText { backslash_escapes: true, ansi_quotes: true },
        MysqlText { backslash_escapes: false, ansi_quotes: true },
    ];

    /// The reading of a session whose `@@sql_mode` is `mode`. ANSI (and the modes that imply it)
    /// is expanded by the server when it is set, so the two flags are read as they are listed.
    pub fn of_mode(mode: &str) -> MysqlText {
        let set = |name: &str| mode.split(',').any(|m| m.trim().eq_ignore_ascii_case(name));
        MysqlText { backslash_escapes: !set("NO_BACKSLASH_ESCAPES"), ansi_quotes: set("ANSI_QUOTES") }
    }
}

/// From the quote at `i`, the offset one past the closing one, or `None` if the text ends first.
/// A doubled quote is one character; where `escapes`, so is a backslash and what follows it.
fn quoted_end(b: &[u8], i: usize, escapes: bool) -> Option<usize> {
    let quote = b[i];
    let mut j = i + 1;
    loop {
        match b.get(j)? {
            b'\\' if escapes => j += 2,
            &q if q == quote && b.get(j + 1) == Some(&quote) => j += 2,
            &q if q == quote => return Some(j + 1),
            _ => j += 1,
        }
    }
}

/// Which named placeholders a statement may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    /// `:name` everywhere, and SQLite's own `@name` and `$name` there: the loader's rule.
    Dialect,
    /// `:name` alone; SQLite's `@name` and `$name` are errors: the framework's rule.
    Colon,
}

pub fn lex(sql: &str, dialect: Dialect) -> Result<Lexed, LexError> {
    lex_with(sql, dialect, Named::Dialect)
}

/// MySQL reads its text by the session's mode; [`lex_with`] reads it as the default mode does.
pub fn lex_with(sql: &str, dialect: Dialect, named: Named) -> Result<Lexed, LexError> {
    lex_text(sql, dialect, named, MysqlText::default())
}

/// A MySQL statement as a session of this reading reads it.
pub fn lex_mysql(sql: &str, named: Named, text: MysqlText) -> Result<Lexed, LexError> {
    lex_text(sql, Dialect::Mysql, named, text)
}

fn lex_text(sql: &str, dialect: Dialect, named: Named, text: MysqlText) -> Result<Lexed, LexError> {
    let sqlite_named = dialect == Dialect::Sqlite && named == Named::Dialect;
    let mysql = dialect == Dialect::Mysql;
    let b = sql.as_bytes();
    let mut i = 0usize;
    let mut params: Vec<String> = Vec::new();
    let mut placeholders: Vec<Placeholder> = Vec::new();
    let mut opaque: Vec<(usize, usize)> = Vec::new();
    // The largest position a positional placeholder has named so far; a bare `?` takes the next.
    let mut largest = 0u64;
    // The span of the first named placeholder, whose prefix every later one repeats.
    let mut first_named: Option<(usize, usize)> = None;
    let mut push = |name: String, start: usize, end: usize, params: &mut Vec<String>| {
        if !params.contains(&name) {
            params.push(name.clone());
        }
        placeholders.push(Placeholder { name, start, end });
    };
    let position = |digits: &str, largest: &mut u64| {
        let n = digits.trim_start_matches('0');
        let n = if n.is_empty() { "0" } else { n };
        *largest = (*largest).max(n.parse().unwrap_or(u64::MAX));
        n.to_string()
    };
    while i < b.len() {
        let c = b[i];
        match c {
            // A MySQL string, unless ANSI_QUOTES makes `"` a name's quote.
            b'\'' | b'"' if c == b'\'' || (mysql && !text.ansi_quotes) => {
                let Some(end) = quoted_end(b, i, mysql && text.backslash_escapes) else {
                    return Err(LexError { message: "unterminated string literal".into(), offset: i });
                };
                opaque.push((i, end));
                i = end;
            }
            b'E' | b'e' if b.get(i + 1) == Some(&b'\'') && !prev_is_name_char(b, i) && !mysql => {
                let start = i;
                i += 2;
                loop {
                    match b.get(i) {
                        None => return Err(LexError { message: "unterminated escape string".into(), offset: start }),
                        Some(b'\\') => i += 2,
                        Some(b'\'') if b.get(i + 1) == Some(&b'\'') => i += 2,
                        Some(b'\'') => {
                            i += 1;
                            break;
                        }
                        Some(_) => i += 1,
                    }
                }
                opaque.push((start, i));
            }
            b'"' | b'`' => {
                let Some(end) = quoted_end(b, i, false) else {
                    return Err(LexError { message: "unterminated quoted identifier".into(), offset: i });
                };
                opaque.push((i, end));
                i = end;
            }
            // MySQL's `--` starts a comment only before a space or a control character, or at the
            // end of the text: `:a --:b` is `:a - (-:b)`.
            b'-' if b.get(i + 1) == Some(&b'-') && !(mysql && b.get(i + 2).is_some_and(|&n| n > b' ' && n != 0x7f)) => {
                let start = i;
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                opaque.push((start, i));
            }
            b'#' if mysql => {
                let start = i;
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                opaque.push((start, i));
            }
            // A MySQL block comment does not nest: it ends at the first `*/`.
            b'/' if b.get(i + 1) == Some(&b'*') && mysql => {
                let Some(close) = find_bytes(b, b"*/", i + 2) else {
                    return Err(LexError { message: "unterminated block comment".into(), offset: i });
                };
                opaque.push((i, close + 2));
                i = close + 2;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let start = i;
                let mut depth = 1usize;
                i += 2;
                while depth > 0 {
                    match (b.get(i), b.get(i + 1)) {
                        (Some(b'/'), Some(b'*')) => {
                            depth += 1;
                            i += 2;
                        }
                        (Some(b'*'), Some(b'/')) => {
                            depth -= 1;
                            i += 2;
                        }
                        (Some(_), _) => i += 1,
                        (None, _) => return Err(LexError { message: "unterminated block comment".into(), offset: start }),
                    }
                }
                opaque.push((start, i));
            }
            b':' => {
                if b.get(i + 1) == Some(&b':') {
                    // A `::` cast: the type name after it is not a parameter.
                    i += 2;
                    if i < b.len() && is_name_start(b[i]) {
                        i = name_end(b, i);
                    }
                    continue;
                }
                match b.get(i + 1) {
                    Some(&n) if is_name_start(n) && !prev_is_name_char(b, i) => {
                        let end = dotted_name_end(b, i + 1);
                        one_prefix(sql, &mut first_named, i, end)?;
                        push(sql[i + 1..end].to_string(), i, end, &mut params);
                        i = end;
                    }
                    _ => i += 1,
                }
            }
            b'$' => {
                if matches!(dialect, Dialect::Postgres | Dialect::Duckdb) {
                    if let Some(tag_end) = dollar_tag_end(b, i) {
                        let tag = &b[i..tag_end];
                        match find_bytes(b, tag, tag_end) {
                            Some(close) => {
                                opaque.push((i, close + tag.len()));
                                i = close + tag.len();
                                continue;
                            }
                            None => {
                                return Err(LexError {
                                    message: format!("unterminated dollar-quoted string {}", sql[i..tag_end].to_string()),
                                    offset: i,
                                })
                            }
                        }
                    }
                }
                match b.get(i + 1) {
                    // `a$1` and `a$b` are identifiers.
                    _ if prev_is_name_char(b, i) => i += 1,
                    Some(&n) if n.is_ascii_digit() && matches!(dialect, Dialect::Postgres | Dialect::Duckdb) => {
                        let end = digits_end(b, i + 1);
                        push(position(&sql[i + 1..end], &mut largest), i, end, &mut params);
                        i = end;
                    }
                    Some(&n) if is_name_start(n) && sqlite_named => {
                        let end = dotted_name_end(b, i + 1);
                        one_prefix(sql, &mut first_named, i, end)?;
                        push(sql[i + 1..end].to_string(), i, end, &mut params);
                        i = end;
                    }
                    Some(&n) if (is_name_start(n) || n.is_ascii_digit()) && dialect != Dialect::Mysql => {
                        return Err(foreign(sql, i, dialect));
                    }
                    _ => i += 1,
                }
            }
            b'@' => match b.get(i + 1) {
                Some(&n) if is_name_start(n) && !prev_is_name_char(b, i) && sqlite_named => {
                    let end = dotted_name_end(b, i + 1);
                    one_prefix(sql, &mut first_named, i, end)?;
                    push(sql[i + 1..end].to_string(), i, end, &mut params);
                    i = end;
                }
                Some(&n) if is_name_start(n) && !prev_is_name_char(b, i) && dialect == Dialect::Sqlite => {
                    return Err(foreign(sql, i, dialect));
                }
                _ => i += 1,
            },
            b'?' if dialect != Dialect::Postgres => {
                if b.get(i + 1).is_some_and(u8::is_ascii_digit) && dialect == Dialect::Mysql {
                    let end = digits_end(b, i + 1);
                    return Err(LexError { message: format!("MySQL has no numbered placeholder {}; write ? for each value, or :name", &sql[i..end]), offset: i });
                }
                if b.get(i + 1).is_some_and(u8::is_ascii_digit) {
                    // SQLite's `?NNN` names position NNN, as `$NNN` does, so it takes that name.
                    let end = digits_end(b, i + 1);
                    push(position(&sql[i + 1..end], &mut largest), i, end, &mut params);
                    i = end;
                } else {
                    largest = largest.saturating_add(1);
                    push(largest.to_string(), i, i + 1, &mut params);
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    let positional = params.iter().filter(|p| is_positional(p)).count();
    let style = match (positional, params.len()) {
        (_, 0) => Style::None,
        (0, _) => Style::Named,
        (p, n) if p == n => Style::Positional,
        _ => Style::Mixed,
    };
    if style == Style::Positional {
        // `$2 ... $1` binds `$1` first: the describe reports the parameters by number.
        params.sort_by_key(|p| p.parse::<u64>().unwrap_or(u64::MAX));
    }
    Ok(Lexed { params, placeholders, style, opaque })
}

/// SQLite reads `:a`, `@a` and `$a` as three parameters, and a name here drops its prefix, so every
/// named placeholder of a statement repeats the prefix of the first.
fn one_prefix(sql: &str, first: &mut Option<(usize, usize)>, start: usize, end: usize) -> Result<(), LexError> {
    match *first {
        None => {
            *first = Some((start, end));
            Ok(())
        }
        Some((s, _)) if sql.as_bytes()[s] == sql.as_bytes()[start] => Ok(()),
        Some((s, e)) => Err(LexError {
            message: format!(
                "{} and {} have different prefixes; write every named parameter of a statement with one prefix, here {}{}",
                &sql[start..end],
                &sql[s..e],
                &sql[s..s + 1],
                &sql[start + 1..end]
            ),
            offset: start,
        }),
    }
}

/// The error for a `$name`, `$1` or `@name` the dialect would misread: Reylo's named placeholder is
/// `:name` (and, outside [`Named::Colon`], SQLite's own `@name` and `$name`), and SQLite's
/// numbered one `?1`.
fn foreign(sql: &str, i: usize, dialect: Dialect) -> LexError {
    let b = sql.as_bytes();
    let digits = b.get(i + 1).is_some_and(u8::is_ascii_digit);
    let end = if digits { digits_end(b, i + 1) } else { dotted_name_end(b, i + 1) };
    let written = &sql[i..end];
    let fix = if digits { format!("?{}", &sql[i + 1..end]) } else { format!(":{}", &sql[i + 1..end]) };
    let message = match dialect {
        Dialect::Sqlite => format!("SQLite reads {written} as a parameter that Reylo does not bind; write {fix}"),
        Dialect::Duckdb => format!("DuckDB reads {written} as a parameter that Reylo does not bind; write {fix}"),
        _ => format!("{written} is not a placeholder here; Reylo's named placeholder is {fix}"),
    };
    LexError { message, offset: i }
}

/// One use of a parameter after its first, by byte offsets in the original text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repeated {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

impl From<Repeated> for LexError {
    fn from(r: Repeated) -> LexError {
        LexError { message: r.message, offset: r.start }
    }
}

/// Every use of a parameter after its first, each with the error that says so. Each parameter
/// appears once in a statement, so that one place types it:
/// a value read twice is selected once in a `with` and read from there. The rule holds for every
/// form a parameter is named by: `:name` (a dotted global such as `:auth.user_id` too), SQLite's
/// `@name` and `$name`, and the positional `$n` and `?n`, whose name is the number, so `?1` after
/// a bare `?` that took 1 repeats it. A bare `?` is a parameter of its own and never repeats.
/// Reylo's analysis reports each one where it is written, and Reylo Framework's build fails on it.
pub fn repeated(sql: &str, lexed: &Lexed) -> Vec<Repeated> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for p in &lexed.placeholders {
        if !seen.contains(&p.name.as_str()) {
            seen.push(&p.name);
            continue;
        }
        let written = &sql[p.start..p.end];
        // The name the example selects the value as: the last segment of a dotted name, and `pn`
        // for a position.
        let column = if is_positional(&p.name) { format!("p{}", p.name) } else { p.name.rsplit('.').next().unwrap_or(&p.name).to_string() };
        // The example casts the value, since nothing else types a parameter a select list reads:
        // Postgres takes it as text, MySQL as a string, and SQLite leaves it untyped.
        out.push(Repeated {
            message: format!(
                "The parameter {written} appears more than once; use each parameter once, and select it in a with to read it twice, cast to its type, as in: with input as (select {written}::<type> as {column}) select … from input"
            ),
            start: p.start,
            end: p.end,
        });
    }
    out
}

/// Whether a parameter name is one the lexer minted for a positional placeholder: a number,
/// where a named one starts with a letter or `_`.
pub fn is_positional(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|c| c.is_ascii_digit())
}

/// Rewrite every placeholder to the target's syntax.
pub fn rewrite(sql: &str, lexed: &Lexed, target: Target) -> Rewritten {
    rewrite_with(sql, lexed, target, &|_, rendered| rendered.to_string())
}

/// Rewrite with a hook over each rendered placeholder — a probe pins a cast on it.
pub fn rewrite_with(sql: &str, lexed: &Lexed, target: Target, decorate: &dyn Fn(&str, &str) -> String) -> Rewritten {
    let mut out = String::with_capacity(sql.len());
    let mut spans = Vec::with_capacity(lexed.placeholders.len());
    let mut order: Vec<String> = Vec::new();
    let mut last = 0;
    for p in &lexed.placeholders {
        let rendered = match target {
            Target::Dollar => {
                if is_positional(&p.name) {
                    format!("${}", p.name)
                } else {
                    format!("${}", lexed.params.iter().position(|q| q == &p.name).map_or(0, |i| i + 1))
                }
            }
            Target::Question => "?".to_string(),
            Target::Numbered => format!("?{}", lexed.params.iter().position(|q| q == &p.name).map_or(0, |i| i + 1)),
        };
        let rendered = decorate(&p.name, &rendered);
        out.push_str(&sql[last..p.start]);
        let rs = out.len();
        out.push_str(&rendered);
        spans.push((p.start, p.end, rs, out.len()));
        last = p.end;
        if target == Target::Question {
            order.push(p.name.clone());
        }
    }
    out.push_str(&sql[last..]);
    if target != Target::Question {
        order = lexed.params.clone();
    }
    Rewritten { sql: out, order, spans, inserted: Vec::new() }
}

/// The first keywords of the statements SQLite and MySQL know that are not queries: DDL,
/// transaction control, session settings and the like. A first word outside this list and
/// outside the typed kinds is left to the database, whose syntax error names it (`selec`).
const NOT_QUERIES: &[&str] = &[
    "alter", "analyze", "attach", "begin", "binlog", "cache", "call", "change", "check", "checksum", "clone", "commit", "create", "deallocate",
    "describe", "desc", "detach", "do", "drop", "end", "execute", "explain", "flush", "get", "grant", "handler", "help", "import", "install",
    "kill", "load", "lock", "optimize", "pragma", "prepare", "purge", "reindex", "release", "rename", "repair", "reset", "resignal", "restart",
    "revoke", "rollback", "savepoint", "set", "show", "shutdown", "signal", "start", "stop", "truncate", "uninstall", "unlock", "use",
    "vacuum", "xa",
];

/// How a file the type check refuses (a migration or a schema file, which nothing imports, but
/// which tsconfig.json's `include` hands to the content mapper all the same) is left out of it.
const LEAVE_OUT: &str = "so if it is a migration or a schema file, leave it out of the type check with \"exclude\" in tsconfig.json, as in \"exclude\": [\"db/**\"]";

/// The error of a file that holds several statements, on every database.
pub fn several_statements() -> String {
    format!("A typed .sql file holds one statement and this one holds several, {LEAVE_OUT}.")
}

/// The database's error on a command ([`Statement::Command`]), with how to leave the file out of
/// the type check: a migration's `CREATE TABLE` prepares against a schema that already has the
/// table on SQLite, and fails there.
pub fn refused_command(message: &str) -> String {
    format!("{}, {LEAVE_OUT}.", message.trim_end().trim_end_matches('.'))
}

/// What the one statement of a typed file is.
#[derive(Debug, PartialEq, Eq)]
pub enum Statement {
    /// One of the kinds the database's analysis reads, or a first word it leaves to the database.
    Query,
    /// Any other statement, by its first keyword, lowercased: DDL, a session setting, a
    /// transaction's control, `EXPLAIN`. It is typed from the database's describe alone.
    Command(String),
}

/// The first word of the text past whitespace and comments, lowercased.
pub fn first_word(sql: &str, lexed: &Lexed) -> String {
    let start = past_comments(sql, lexed, 0);
    sql[start..].chars().take_while(|c| c.is_ascii_alphabetic()).collect::<String>().to_ascii_lowercase()
}

/// The first byte at or after `i` that is neither whitespace nor inside a comment.
fn past_comments(sql: &str, lexed: &Lexed, mut i: usize) -> usize {
    let bytes = sql.as_bytes();
    let is_comment = |start: usize| sql[start..].starts_with("--") || sql[start..].starts_with("/*") || sql[start..].starts_with('#');
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        match lexed.opaque.iter().find(|&&(start, _)| start == i && is_comment(start)) {
            Some(&(_, end)) => i = end,
            None => return i,
        }
    }
}

/// Whether the text is one statement, and of which kind: a query when its first keyword (past
/// whitespace and comments) is one of `kinds`, or an opening parenthesis, and a command when it is
/// another statement's. Nothing but whitespace, semicolons and comments may follow its first
/// semicolon outside the text spans: SQLite prepares only the first statement of the text, so a
/// migration file would otherwise come back typed from its first statement alone; Postgres's
/// parse refuses several (`AnalyzeError` in `analyze.rs`).
pub fn one_statement(sql: &str, lexed: &Lexed, kinds: &[&str]) -> Result<Statement, String> {
    let bytes = sql.as_bytes();
    let in_text = |i: usize| lexed.opaque.iter().any(|&(start, end)| start <= i && i < end);
    if let Some(semicolon) = (0..bytes.len()).find(|&i| bytes[i] == b';' && !in_text(i)) {
        let mut i = semicolon + 1;
        loop {
            i = past_comments(sql, lexed, i);
            if i < bytes.len() && bytes[i] == b';' {
                i += 1;
            } else {
                break;
            }
        }
        if i < bytes.len() {
            return Err(several_statements());
        }
    }
    let word = first_word(sql, lexed);
    if kinds.contains(&word.as_str()) || sql[past_comments(sql, lexed, 0)..].starts_with('(') || !NOT_QUERIES.contains(&word.as_str()) {
        return Ok(Statement::Query);
    }
    Ok(Statement::Command(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(sql: &str, d: Dialect) -> Vec<String> {
        lex(sql, d).unwrap().params
    }

    #[test]
    fn styles() {
        assert_eq!(names("select :a, :b, :a", Dialect::Postgres), ["a", "b"]);
        assert_eq!(names("select $1, $2, $1", Dialect::Postgres), ["1", "2"]);
        assert_eq!(names("select ?, ?", Dialect::Sqlite), ["1", "2"]);
        assert_eq!(names("select ?, ?", Dialect::Mysql), ["1", "2"]);
        assert_eq!(names("select ?, ?3, ?", Dialect::Duckdb), ["1", "3", "4"]);
        assert_eq!(names("select $2, $1", Dialect::Duckdb), ["1", "2"]);
        assert_eq!(names("select :a, :b", Dialect::Duckdb), ["a", "b"]);
        assert_eq!(lex("select :a, $1", Dialect::Postgres).unwrap().style, Style::Mixed);
        assert_eq!(lex("select 1", Dialect::Postgres).unwrap().style, Style::None);
    }

    #[test]
    fn skips_text_and_casts() {
        let sql = "select ':x' as a, \":y\" as b, e'\\':z' as c, $$:w$$ as d, x::int, arr[1:2], tags ? 'k' -- :c\n /* :d /* :e */ */ from t where id = :id";
        assert_eq!(names(sql, Dialect::Postgres), ["id"]);
        assert_eq!(names("select json_extract(j, '$.a') = ? and x = :n", Dialect::Sqlite), ["1", "n"]);
        assert_eq!(names("select `a:b`, 'x' from t where a = :a", Dialect::Mysql), ["a"]);
        let duck = "select ':x' as a, \":y\" as b, e'\\':z' as c, $$:w ?$$ as d, $t$ $1 $t$ as e, x::int, l[1:2], {'k': 1} -- :c\n /* :d /* :e */ */ from t where id = :id";
        assert_eq!(names(duck, Dialect::Duckdb), ["id"]);
    }

    #[test]
    fn one_named_form_and_the_native_positional_one() {
        // A form the dialect would misread is an error that says what to write.
        let err = |sql: &str, d: Dialect| lex(sql, d).unwrap_err();
        assert_eq!(err("select $name", Dialect::Postgres).message, "$name is not a placeholder here; Reylo's named placeholder is :name");
        assert_eq!(err("select 1 where x = $2", Dialect::Sqlite).message, "SQLite reads $2 as a parameter that Reylo does not bind; write ?2");
        assert_eq!(err("select ?2", Dialect::Mysql).message, "MySQL has no numbered placeholder ?2; write ? for each value, or :name");
        assert_eq!(err("select $name", Dialect::Duckdb).message, "DuckDB reads $name as a parameter that Reylo does not bind; write :name");
        assert_eq!(err("select $name", Dialect::Postgres).offset, 7);
        // Where the dialect reads the form as something else, it is that: MySQL's user variable and
        // `$` identifier, Postgres's `@` operator and `a$1` identifier.
        assert!(names("select @x := 1, $col, $1 from t", Dialect::Mysql).is_empty());
        assert!(names("select @ -5, a$1, b$c from t", Dialect::Postgres).is_empty());
        // A `-- @returns one` line is a comment like any other.
        assert!(names("-- @returns one\nselect 1", Dialect::Postgres).is_empty());
    }

    #[test]
    fn sqlite_named_prefixes() {
        // SQLite's `@name` and `$name` are named placeholders there, dotted like `:name`, and the
        // name drops the prefix.
        for sql in ["select @a.b, @c, @a.b", "select $a.b, $c, $a.b"] {
            let l = lex(sql, Dialect::Sqlite).unwrap();
            assert_eq!(l.params, ["a.b", "c"]);
            assert_eq!(l.style, Style::Named);
            assert_eq!(rewrite(sql, &l, Target::Numbered).sql, "select ?1, ?2, ?1");
        }
        // SQLite reads `:a` and `@a` as two parameters, so one statement keeps one prefix.
        let e = lex("select :a, @b where x = $a", Dialect::Sqlite).unwrap_err();
        assert_eq!(e.message, "@b and :a have different prefixes; write every named parameter of a statement with one prefix, here :b");
        assert_eq!(e.offset, 11);
        assert_eq!(lex("select @a, :a", Dialect::Sqlite).unwrap_err().offset, 11);
        // Named and positional still do not mix, and `a$b` and `a@b` are not placeholders.
        assert_eq!(lex("select @a, ?", Dialect::Sqlite).unwrap().style, Style::Mixed);
        assert!(names("select a$b, a@b from t", Dialect::Sqlite).is_empty());
        // Elsewhere the forms keep their meaning: Postgres `@` is an operator, MySQL `@x` a variable.
        assert!(names("select @a from t", Dialect::Postgres).is_empty());
        assert!(names("select @a, $b from t", Dialect::Mysql).is_empty());
        // The framework takes `:name` alone.
        let colon = |sql: &str| lex_with(sql, Dialect::Sqlite, Named::Colon).unwrap_err().message;
        assert_eq!(colon("select @a.b from t"), "SQLite reads @a.b as a parameter that Reylo does not bind; write :a.b");
        assert_eq!(colon("select :x, $y"), "SQLite reads $y as a parameter that Reylo does not bind; write :y");
    }

    #[test]
    fn rewrites_and_maps_back() {
        let sql = "select * from t where id = :id and name = :name and id = :id";
        let l = lex(sql, Dialect::Postgres).unwrap();
        let r = rewrite(sql, &l, Target::Dollar);
        assert_eq!(r.sql, "select * from t where id = $1 and name = $2 and id = $1");
        assert_eq!(r.order, ["id", "name"]);
        // "name" starts at the same place in both; the third placeholder shifted.
        let third = r.sql.rfind("$1").unwrap();
        assert_eq!(r.to_original(third), sql.rfind(":id").unwrap());
        assert_eq!(r.to_original(r.sql.len()), sql.len());
        let q = rewrite(sql, &l, Target::Question);
        assert_eq!(q.sql, "select * from t where id = ? and name = ? and id = ?");
        assert_eq!(q.order, ["id", "name", "id"]);
    }

    #[test]
    fn each_parameter_once() {
        let at = |sql: &str, d: Dialect| repeated(sql, &lex(sql, d).unwrap()).into_iter().map(|r| (r.start, r.end)).collect::<Vec<_>>();
        // Every use after the first is an error at its place, in every form a dialect names.
        let sql = "select id, name from users where name like :q or email like :q";
        let r = repeated(sql, &lex(sql, Dialect::Postgres).unwrap());
        assert_eq!(r.len(), 1);
        assert_eq!(&sql[r[0].start..r[0].end], ":q");
        assert_eq!(r[0].start, sql.rfind(":q").unwrap());
        assert_eq!(
            r[0].message,
            "The parameter :q appears more than once; use each parameter once, and select it in a with to read it twice, cast to its type, as in: with input as (select :q::<type> as q) select … from input"
        );
        assert_eq!(at("select :a, :b, :a, :a, :b", Dialect::Postgres), [(15, 17), (19, 21), (23, 25)]);
        assert_eq!(at("select $1, $2, $1", Dialect::Postgres), [(15, 17)]);
        assert_eq!(at("select ?3, ?3", Dialect::Sqlite), [(11, 13)]);
        assert_eq!(at("select @a, @a", Dialect::Sqlite), [(11, 13)]);
        assert_eq!(at("select $a, $a", Dialect::Sqlite), [(11, 13)]);
        assert_eq!(at("select :a, :a", Dialect::Mysql), [(11, 13)]);
        // A bare `?` takes 1 here, so `?1` names the same parameter again.
        assert_eq!(at("select ?, ?1", Dialect::Sqlite), [(10, 12)]);
        // A dotted global is one name; the example selects it by its last segment, and a
        // position by `p` and its number.
        let sql = "select * from t where owner = :auth.user_id or author = :auth.user_id";
        assert!(repeated(sql, &lex(sql, Dialect::Postgres).unwrap())[0].message.contains("with input as (select :auth.user_id::<type> as user_id)"));
        let sql = "select $1::int + $1";
        assert!(repeated(sql, &lex(sql, Dialect::Postgres).unwrap())[0].message.starts_with("The parameter $1 appears more than once;"));
        assert!(repeated(sql, &lex(sql, Dialect::Postgres).unwrap())[0].message.contains("(select $1::<type> as p1)"));
        // A bare `?` never repeats; distinct names, text and casts are not uses.
        assert!(at("select ?, ?, ?", Dialect::Mysql).is_empty());
        assert!(at("select ?, ?", Dialect::Sqlite).is_empty());
        assert!(at("select :a, :b, ':a', x::a -- :a", Dialect::Postgres).is_empty());
        assert!(at("with input as (select :q::text as q) select 1 from t, input where a = input.q or b = input.q", Dialect::Postgres).is_empty());
    }

    #[test]
    fn errors() {
        assert!(lex("select 'abc", Dialect::Postgres).is_err());
        assert!(lex("select $x$abc", Dialect::Postgres).is_err());
        assert!(lex("select /* a", Dialect::Postgres).is_err());
        assert_eq!(lex("select 'abc", Dialect::Postgres).unwrap_err().to_string(), "unterminated string literal (at byte 7)");
        // Each error points at what is left open, the quote or the comment's start, which lies
        // inside the text: an offset at its end placed the error past the end of the file.
        let at = |sql: &str, dialect| lex(sql, dialect).unwrap_err().offset;
        assert_eq!(at("select 1, 'abc", Dialect::Sqlite), 10);
        assert_eq!(at("select 1, \"abc", Dialect::Postgres), 10);
        assert_eq!(at("select 1, `abc\n", Dialect::Sqlite), 10);
        assert_eq!(at("select 1, `abc", Dialect::Mysql), 10);
        assert_eq!(at("select 1, E'abc", Dialect::Postgres), 10);
        assert_eq!(at("select 1, $x$abc", Dialect::Postgres), 10);
        assert_eq!(at("select 1, /* a /* b */", Dialect::Postgres), 10);
        assert_eq!(at("select 1, /* a", Dialect::Sqlite), 10);
        assert_eq!(at("select 1, /* a", Dialect::Mysql), 10);
    }

    #[test]
    fn dotted_names() {
        let sql = "select :a.b";
        let l = lex(sql, Dialect::Postgres).unwrap();
        assert_eq!(l.params, ["a.b"]);
        assert_eq!(l.placeholders, [Placeholder { name: "a.b".into(), start: 7, end: 11 }]);
        assert_eq!(l.style, Style::Named);
        assert_eq!(rewrite(sql, &l, Target::Dollar).sql, "select $1");
        assert_eq!(rewrite(sql, &l, Target::Numbered).sql, "select ?1");
        assert_eq!(rewrite(sql, &l, Target::Question).sql, "select ?");
        // Every segment starts like a name; the named forms agree; a cast after one is a cast.
        assert_eq!(names("select :auth.user_id, :org.name::text, :t.*, :x.1", Dialect::Postgres), ["auth.user_id", "org.name", "t", "x"]);
        // A dotted name survives the SQLite prepare only through the rewrite, and maps back.
        let sql = "select :a.b, :c";
        let l = lex(sql, Dialect::Sqlite).unwrap();
        let r = rewrite(sql, &l, Target::Numbered);
        assert_eq!(r.sql, "select ?1, ?2");
        assert_eq!(r.to_original(r.sql.find("?2").unwrap()), sql.find(":c").unwrap());
        assert_eq!(r.to_original(r.sql.len()), sql.len());
    }

    #[test]
    fn numbered_question_is_a_position() {
        let l = lex("select ?3, ?3", Dialect::Sqlite).unwrap();
        assert_eq!(l.params, ["3"]);
        assert_eq!(l.placeholders.iter().map(|p| (p.start, p.end)).collect::<Vec<_>>(), [(7, 9), (11, 13)]);
        assert_eq!(l.style, Style::Positional);
        assert_eq!(lex("select ?3, :x", Dialect::Sqlite).unwrap().style, Style::Mixed);
        // On Postgres `?` is still the operator, digits or not.
        assert!(lex("select j ? 'k', 1 ?2", Dialect::Postgres).unwrap().params.is_empty());
    }

    #[test]
    fn array_slices_are_not_placeholders() {
        for d in [Dialect::Postgres, Dialect::Sqlite, Dialect::Mysql] {
            assert!(lex("select arr[1:2], arr[2:], arr[:3], arr[1:2].x, arr[lo:hi], arr[1:n] from t", d).unwrap().params.is_empty());
        }
        // A colon after no name character still opens a name: `arr[:lo]` reads a parameter.
        assert_eq!(names("select arr[:lo] from t", Dialect::Postgres), ["lo"]);
    }

    #[test]
    fn colons_that_open_nothing() {
        // `:{` and MySQL's `:=` open nothing; a quote doubled inside a string is one quote.
        assert!(names("select '{}'::jsonb, :{a}", Dialect::Postgres).is_empty());
        assert!(names("select @x := 1", Dialect::Mysql).is_empty());
        assert_eq!(names("select 'it''s :not', :yes", Dialect::Postgres), ["yes"]);
        // An upper-case `E''` string and a tagged dollar quote hide what is inside them.
        assert_eq!(names("select E'\\':nope', $tag$ :nope $tag$, :real", Dialect::Postgres), ["real"]);
    }

    #[test]
    fn opaque_spans() {
        let sql = "select 'a''b', \"q\"\"r\", $t$ :x $t$, e'\\'' -- c :y\n/* d /* :z */ */ :w";
        let l = lex(sql, Dialect::Postgres).unwrap();
        assert_eq!(l.params, ["w"]);
        assert_eq!(l.opaque, [(7, 13), (15, 21), (23, 33), (35, 40), (41, 48), (49, 65)]);
        let text: Vec<&str> = l.opaque.iter().map(|&(s, e)| &sql[s..e]).collect();
        assert_eq!(text, ["'a''b'", "\"q\"\"r\"", "$t$ :x $t$", "e'\\''", "-- c :y", "/* d /* :z */ */"]);
        // A `-- @` line is a comment like any other; code between the spans is not listed.
        let l = lex("-- @returns one\nselect `a`, 1", Dialect::Mysql).unwrap();
        assert_eq!(l.opaque, [(0, 15), (23, 26)]);
    }

    #[test]
    fn numbered_target() {
        let sql = "select :a, :b, :a";
        let l = lex(sql, Dialect::Sqlite).unwrap();
        let r = rewrite(sql, &l, Target::Numbered);
        assert_eq!(r.sql, "select ?1, ?2, ?1");
        assert_eq!(r.order, ["a", "b"]);
        assert_eq!(r.spans, [(7, 9, 7, 9), (11, 13, 11, 13), (15, 17, 15, 17)]);
        // Positional parameters bind by number, whatever order they appear in: `?n` binds
        // `params[n - 1]`, so an author's numbering survives.
        let sql = "select ?2, ?1, ?2";
        let l = lex(sql, Dialect::Sqlite).unwrap();
        let r = rewrite(sql, &l, Target::Numbered);
        assert_eq!(r.sql, "select ?2, ?1, ?2");
        assert_eq!(r.order, ["1", "2"]);
    }

    #[test]
    fn positional_names_are_numbers() {
        // A bare `?` takes the number after the largest one so far, as SQLite numbers it.
        assert_eq!(names("select ?1, ?2, ?", Dialect::Sqlite), ["1", "2", "3"]);
        assert_eq!(names("select ?5, ?, ?2", Dialect::Sqlite), ["2", "5", "6"]);
        assert_eq!(names("select $01, $1", Dialect::Postgres), ["1"]);
        assert!(is_positional("12") && !is_positional("a1") && !is_positional(""));
        // `$2 ... $1` binds `$1` first, and the rewrite keeps the author's numbers.
        let sql = "select $2::int, $1::text";
        let l = lex(sql, Dialect::Postgres).unwrap();
        assert_eq!(l.params, ["1", "2"]);
        let r = rewrite(sql, &l, Target::Dollar);
        assert_eq!(r.sql, sql);
        assert_eq!(r.order, ["1", "2"]);
        // MySQL binds one value per `?`, in text order.
        let sql = "select ?, ?";
        let l = lex(sql, Dialect::Mysql).unwrap();
        assert_eq!(rewrite(sql, &l, Target::Question).order, ["1", "2"]);
    }

    #[test]
    fn mysql_text() {
        // By default a backslash escapes the character after it, in either quote, and a
        // double-quoted word is a string; a placeholder inside one is text.
        let sql = r#"select 'it\'s :not', 'a\\' as b, :x, "say \":no", 'a\nb' from t where id = :id"#;
        assert_eq!(names(sql, Dialect::Mysql), ["x", "id"]);
        // Postgres and SQLite read a backslash as a character, so there `'a\'` ends at its quote.
        assert_eq!(names(r"select 'C:\', :x", Dialect::Postgres), ["x"]);
        assert_eq!(names(r"select 'C:\', :x", Dialect::Sqlite), ["x"]);
        assert_eq!(lex(r"select 'C:\', :x", Dialect::Mysql).unwrap_err().message, "unterminated string literal");
        // NO_BACKSLASH_ESCAPES makes the backslash a character there too.
        let plain = MysqlText { backslash_escapes: false, ansi_quotes: false };
        assert_eq!(lex_mysql(r"select 'C:\', :x", Named::Dialect, plain).unwrap().params, ["x"]);
        assert_eq!(lex_mysql(r"select 'a\' as b, :x, 'it''s :not'", Named::Dialect, plain).unwrap().params, ["x"]);
        // Under ANSI_QUOTES a double-quoted word is a name, which escapes nothing.
        let ansi = MysqlText { backslash_escapes: true, ansi_quotes: true };
        assert_eq!(lex_mysql(r#"select "a\" from t where id = :id"#, Named::Dialect, ansi).unwrap().params, ["id"]);
        assert!(lex(r#"select "a\" from t where id = :id"#, Dialect::Mysql).is_err());
        // `#` starts a line comment; `--` does only before a space or a control character, or at the end.
        assert_eq!(names("select :a # :b\n, :c", Dialect::Mysql), ["a", "c"]);
        assert_eq!(names("select :a --:b", Dialect::Mysql), ["a", "b"]);
        assert_eq!(names("select :a -- :b\n, :c --\t:d\n, 1--", Dialect::Mysql), ["a", "c"]);
        assert_eq!(names("select :a --:b", Dialect::Postgres), ["a"]);
        assert_eq!(names("select 1 # :b", Dialect::Postgres), ["b"]);
        // A block comment ends at its first `*/`; backquotes quote a name and escape nothing.
        assert_eq!(names("select /* a /* b */ :x", Dialect::Mysql), ["x"]);
        assert_eq!(names(r"select `a\`, :x", Dialect::Mysql), ["x"]);
        // MySQL has no `E''` string: `e'\''` is an escaped quote by the default reading alone.
        assert_eq!(lex_mysql(r"select e'\', :x", Named::Dialect, plain).unwrap().params, ["x"]);
        // The spans are listed as the reading has them.
        let sql = "select 'a\\'b'':c', \"c\\\"\", `d`, :e # f\n/* g /* */";
        let l = lex(sql, Dialect::Mysql).unwrap();
        let text: Vec<&str> = l.opaque.iter().map(|&(s, e)| &sql[s..e]).collect();
        assert_eq!(text, ["'a\\'b'':c'", "\"c\\\"\"", "`d`", "# f", "/* g /* */"]);
        assert_eq!(l.params, ["e"]);
        // A comment that starts with `#` is a comment to the statement check too.
        assert_eq!(one_statement("select 1; # done", &lex("select 1; # done", Dialect::Mysql).unwrap(), &["select"]), Ok(Statement::Query));
        assert_eq!(MysqlText::of_mode("STRICT_TRANS_TABLES,NO_BACKSLASH_ESCAPES"), plain);
        assert_eq!(MysqlText::of_mode("REAL_AS_FLOAT,PIPES_AS_CONCAT,ANSI_QUOTES,IGNORE_SPACE,ONLY_FULL_GROUP_BY,ANSI"), ansi);
        assert_eq!(MysqlText::of_mode(""), MysqlText::default());
    }

    #[test]
    fn one_statement_of_any_kind() {
        const SQLITE: &[&str] = &["select", "with", "values", "insert", "replace", "update", "delete"];
        let check = |sql: &str| one_statement(sql, &lex(sql, Dialect::Sqlite).unwrap(), SQLITE);
        assert_eq!(check("select 1"), Ok(Statement::Query));
        assert_eq!(check("-- the user\n/* by id */ SELECT id from users where id = :id;"), Ok(Statement::Query));
        assert_eq!(check("select 1;\n-- a trailing comment\n;\n"), Ok(Statement::Query));
        assert_eq!(check("select ';' as a"), Ok(Statement::Query));
        assert_eq!(check("with x as (select 1) delete from t where id in (select * from x)"), Ok(Statement::Query));
        // A statement of another kind is a command, named by its first keyword.
        assert_eq!(check("create table t (id integer primary key)"), Ok(Statement::Command("create".into())));
        assert_eq!(check("/* the schema */ PRAGMA table_info(t);"), Ok(Statement::Command("pragma".into())));
        // A migration or a schema file of several statements: the error says what a typed file
        // holds, and how to leave such a file out of the type check.
        let several = "A typed .sql file holds one statement and this one holds several, so if it is a migration or a schema file, leave it out of the type check with \"exclude\" in tsconfig.json, as in \"exclude\": [\"db/**\"].";
        assert_eq!(check("select 1; select 2").unwrap_err(), several);
        assert_eq!(check("create table t (id integer primary key);\ncreate index i on t (id);").unwrap_err(), several);
        // A misspelt keyword is a syntax error the database reports, not a statement of another kind.
        assert_eq!(check("selec id from users"), Ok(Statement::Query));
        assert_eq!(refused_command("table t already exists"), "table t already exists, so if it is a migration or a schema file, leave it out of the type check with \"exclude\" in tsconfig.json, as in \"exclude\": [\"db/**\"].");
    }
}
