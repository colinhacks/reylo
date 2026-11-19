//! The ClickHouse analysis.
//!
//! The server describes the statement: `DESCRIBE (…)` answers with each result column's name and
//! type and runs nothing. A column's type is the whole of what the engine says of it, nullability
//! and nested shape included ([`crate::types`]), and a parameter's is the one its placeholder
//! declares, `{name:Type}`, or the cast after it, `:name::Type`.
//!
//! What the describe leaves open is that ClickHouse writes a parameter's value into the statement
//! as a constant before it reads the statement, and a constant can decide a type: the scale of
//! `toDateTime64(x, {p:UInt8})`, the type `CAST(x, {t:String})` names, the length of a
//! `FixedString`. A constant is also computed with as the statement is read, so no one value can
//! stand for a parameter: `toDate('a')` fails where `toDate({day:String})` is a fine statement. So
//! the statement is described three ways, and the answers held against each other:
//!
//! - With each parameter a column, `materialize(defaultValueOfTypeName('T'))`, which has the
//!   parameter's type and no value to read or to compute with. ClickHouse refuses it wherever it
//!   needs a constant, and the types it answers with hold for every value. A `LIMIT`, an `OFFSET`
//!   and the right side of an `IN` take a constant and decide no type, so a parameter there is
//!   written as one.
//! - Where that is refused, each parameter is asked about alone, which finds the ones the server
//!   reads as constants. Such a parameter is written as a constant, and the statement described
//!   again with two other values of it: the same types with one of them, and they are the
//!   statement's (`formatDateTime(t, {format:String})`, `quantile({level:Float64})(x)`); other
//!   types, and the parameter is refused where it stands. A value the statement fails with says
//!   nothing of its types, since the statement fails with it as it runs too, so one of the two
//!   must describe.
//! - With every parameter a constant, as the statement runs, twice with other values. A function
//!   may type a constant otherwise than a column: `x IN (1, {n:UInt8})` is a UInt8 of constants and
//!   a Nullable(UInt8) of a column. Where the two describes answer alike, theirs are the
//!   statement's types; where they differ, a parameter's value changes a type, and it is refused;
//!   where one fails, a value was computed with, and the first describe stands.
//!
//! No list of values shows that a type holds for every value, so the run time holds the types each
//! result comes back with against the ones read here, and refuses a result that differs
//! (src/decode-clickhouse.ts). A parameter that names a column or a table, `{c:Identifier}`,
//! changes what the statement reads, and is refused.
//!
//! How many rows the statement returns is read from the server's query tree ([`crate::tree`]). A
//! write (`INSERT`, `DELETE`, `UPDATE`, `ALTER TABLE … UPDATE` or `DELETE`) returns no rows, and
//! ClickHouse describes none: an `INSERT` is checked against its table by `EXPLAIN PIPELINE`,
//! which builds the insert and writes nothing, a `DELETE` as the `SELECT` of the rows it would
//! delete, and the others for their syntax (`EXPLAIN AST`).

use crate::api::{Analysis, Column, Diagnostic, Param, Scope};
use crate::host::{as_str, Host, Lazy};
use crate::types::{self, Arg, Named};
use reylo_engine_core::analysis::{self, ResultTarget};
use reylo_engine_core::api::DbError;
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::lex::{self, Lexed, Placeholder, Rewritten, Target};
use reylo_engine_core::types::{Cardinality, StatementKind, Type};
use serde_json::Value;

/// The settings of the session that may drop the one row an aggregate returns: the three that
/// leave it are off, `0`, by default.
pub const ROW_SETTINGS: &str = "SELECT name, value FROM system.settings WHERE name IN ('empty_result_for_aggregation_by_empty_set', 'limit', 'offset')";

const DESCRIBE: &str = "DESCRIBE (\n";
const TREE: &str = "EXPLAIN QUERY TREE\n";
const PIPELINE: &str = "EXPLAIN PIPELINE\n";
const AST: &str = "EXPLAIN AST\n";
/// Asks the type the server reads a name as; the name, quoted, and `))` follow.
const TYPE_NAME: &str = "SELECT toTypeName(defaultValueOfTypeName(";

/// Where a parameter stands, as far as the analysis writes it differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    /// Anywhere a value goes.
    Value,
    /// Straight after `LIMIT`, or after the comma of `LIMIT offset, count`.
    Limit,
    /// Straight after `OFFSET`.
    Offset,
    /// The right side of an `IN`: straight after the word, or in the list in parentheses after it.
    In,
    /// Where the server takes a constant alone, and the statement describes with the same types
    /// for each value tried: found by asking the server, where the others are read off the text.
    Constant,
}

/// How a statement's parameters are written into the text the server is asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum As {
    /// Each as a column, a value that is no constant, but where its position takes a constant:
    /// there, its constant of this number.
    Columns(usize),
    /// Each as its constant of this number: the statement as it runs.
    Constants(usize),
    /// The one at this index as a column, and the others as their first constants.
    Column(usize),
    /// The one at this index as its first constant, and the others as `Columns(0)` writes them.
    Constant(usize),
    /// As `Columns(0)`, but the parameter at this index, where it is a constant, as its constant
    /// of this number.
    Varied(usize, usize),
    /// As `Constants(0)`, but the parameter at this index as its second constant.
    Other(usize),
}

fn whole(sql: &str, message: impl Into<String>, code: u32, server_code: Option<String>) -> Diagnostic {
    Diagnostic { message: message.into(), start: 0, length: sql.trim_end().len().max(1), code, server_code }
}

fn at(p: &Placeholder, message: impl Into<String>, code: u32) -> Diagnostic {
    Diagnostic { message: message.into(), start: p.start, length: p.end - p.start, code, server_code: None }
}

/// The type a placeholder declares, as written: inside its braces, or in the cast after `:name`.
fn written_type<'a>(sql: &'a str, p: &Placeholder) -> Option<&'a str> {
    let text = &sql[p.start..p.end];
    if text.starts_with('{') {
        let colon = text.find(':')?;
        return Some(text[colon + 1..text.len() - 1].trim());
    }
    let after = sql[p.end..].strip_prefix("::")?;
    Some(&after[..type_end(after)])
}

/// The length of the type's name `text` starts with: a name, and its arguments in parentheses.
fn type_end(text: &str) -> usize {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
        i += 1;
    }
    if b.get(i) != Some(&b'(') {
        return i;
    }
    let mut depth = 0usize;
    while i < b.len() {
        match b[i] {
            b'\'' => {
                i += 1;
                while i < b.len() && b[i] != b'\'' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    break;
                }
            }
            _ => i += 1,
        }
    }
    i.min(b.len())
}

/// The spans a walk over the statement's code steps over whole: its strings, quoted names and
/// comments, and every placeholder but `but`, whose type may hold parentheses of its own.
fn skipped(lexed: &Lexed, but: &Placeholder) -> Vec<(usize, usize)> {
    let mut spans = lexed.opaque.clone();
    spans.extend(lexed.placeholders.iter().filter(|p| p.start != but.start).map(|p| (p.start, p.end)));
    spans.sort_unstable();
    spans
}

fn is_comment(sql: &str, start: usize) -> bool {
    sql[start..].starts_with("--") || sql[start..].starts_with("/*") || sql[start..].starts_with('#')
}

/// The end of the code before `at`: past the whitespace and comments that stand between.
fn code_before(sql: &str, lexed: &Lexed, mut at: usize) -> usize {
    let b = sql.as_bytes();
    loop {
        while at > 0 && b[at - 1].is_ascii_whitespace() {
            at -= 1;
        }
        match lexed.opaque.iter().find(|&&(start, end)| end == at && is_comment(sql, start)) {
            Some(&(start, _)) => at = start,
            None => return at,
        }
    }
}

/// The start of the code at or after `at`: past whitespace and comments.
fn code_after(sql: &str, lexed: &Lexed, mut at: usize) -> usize {
    let b = sql.as_bytes();
    loop {
        while at < b.len() && b[at].is_ascii_whitespace() {
            at += 1;
        }
        match lexed.opaque.iter().find(|&&(start, _)| start == at && is_comment(sql, start)) {
            Some(&(_, end)) => at = end,
            None => return at,
        }
    }
}

/// The word that ends at `end`, lowercased, and where it starts.
fn word_before(sql: &str, end: usize) -> (String, usize) {
    let b = sql.as_bytes();
    let mut start = end;
    while start > 0 && (b[start - 1].is_ascii_alphanumeric() || b[start - 1] == b'_') {
        start -= 1;
    }
    (sql[start..end].to_ascii_lowercase(), start)
}

/// Where the placeholder `p` stands.
fn position(sql: &str, lexed: &Lexed, p: &Placeholder) -> Position {
    let b = sql.as_bytes();
    let before = code_before(sql, lexed, p.start);
    match word_before(sql, before).0.as_str() {
        "limit" => return Position::Limit,
        "offset" => return Position::Offset,
        // `IN`, `NOT IN` and their `GLOBAL` forms all end with the word.
        "in" => return Position::In,
        _ => {}
    }
    // `LIMIT offset, count`: the count, after a comma that follows the offset's one word.
    if before > 0 && b[before - 1] == b',' {
        let offset_end = code_before(sql, lexed, before - 1);
        let offset_start = lexed.placeholders.iter().find(|q| q.end == offset_end).map_or_else(|| word_before(sql, offset_end).1, |q| q.start);
        if offset_start < offset_end && word_before(sql, code_before(sql, lexed, offset_start)).0 == "limit" {
            return Position::Limit;
        }
    }
    // The parentheses the placeholder stands in: the list of an `IN`, as in `x IN (1, {n:UInt8})`,
    // and not the subquery an `IN` reads, whose parameters are values as anywhere.
    let skip = skipped(lexed, p);
    let (mut i, mut depth, mut open) = (p.start, 0usize, None);
    while i > 0 {
        if let Some(&(start, _)) = skip.iter().find(|&&(_, end)| end == i) {
            i = start;
            continue;
        }
        i -= 1;
        match b[i] {
            b')' => depth += 1,
            b'(' if depth == 0 => {
                open = Some(i);
                break;
            }
            b'(' => depth -= 1,
            _ => {}
        }
    }
    let Some(open) = open else { return Position::Value };
    let first = code_after(sql, lexed, open + 1);
    let word = sql[first..].bytes().take_while(u8::is_ascii_alphabetic).map(|c| c.to_ascii_lowercase() as char).collect::<String>();
    if word_before(sql, code_before(sql, lexed, open)).0 == "in" && !matches!(word.as_str(), "select" | "with" | "from") {
        return Position::In;
    }
    Position::Value
}

/// The constants a parameter of `ty` is written as where the server reads one: each as the
/// expression that stands in its place, and as a message shows it. Three where the type has them,
/// so that a type that changes with the value shows; one, the type's default, for a type whose
/// values are not written here, which no describe can vary.
fn constants(ty: &Named) -> Vec<(String, String)> {
    let name = types::quoted(&ty.to_string());
    let cast = |value: &str| format!("CAST({value}, {name})");
    let of = |values: &[&str]| values.iter().map(|v| (cast(v), v.to_string())).collect::<Vec<_>>();
    match (ty.name.as_str(), ty.args.as_slice()) {
        // A value, NULL, and another value.
        ("Nullable", [Arg::Type(inner)]) => {
            let inner = constants(inner);
            let mut out = vec![(cast(&inner[0].0), inner[0].1.clone()), (cast("NULL"), "NULL".to_string())];
            out.extend(inner.get(1).map(|(sql, shown)| (cast(sql), shown.clone())));
            out
        }
        ("LowCardinality", [Arg::Type(inner)]) => constants(inner).into_iter().map(|(sql, shown)| (cast(&sql), shown)).collect(),
        // One element, none, and two.
        ("Array", [Arg::Type(inner)]) => {
            let inner = constants(inner);
            let (first, last) = (&inner[0], &inner[inner.len() - 1]);
            vec![(cast(&format!("[{}]", first.0)), format!("[{}]", first.1)), (cast("[]"), "[]".to_string()), (cast(&format!("[{}, {}]", last.0, first.0)), format!("[{}, {}]", last.1, first.1))]
        }
        ("Bool", _) => of(&["true", "false"]),
        ("String" | "FixedString", _) => of(&["'a'", "'b'", "'c'"]),
        // A date or a time of a number: so many days, or seconds, after 1970.
        ("Date" | "Date32" | "DateTime" | "DateTime64", _) => of(&["1", "2", "3"]),
        // Within 0 and 1, which a level and a ratio take alone.
        ("Float32" | "Float64" | "BFloat16", _) => of(&["0.5", "0.25", "0.75"]),
        _ if ty.is_number() => of(&["1", "2", "3"]),
        _ => vec![(format!("defaultValueOfTypeName({name})"), "its type's default value".to_string())],
    }
}

/// The statement's parameters as the analysis reads them: each placeholder's type, where it
/// stands, and the constants that stand for it.
struct Parameters {
    types: Vec<Named>,
    positions: Vec<Position>,
    constants: Vec<Vec<(String, String)>>,
}

impl Parameters {
    fn of(types: Vec<Named>, positions: Vec<Position>) -> Parameters {
        let constants = types.iter().map(constants).collect();
        Parameters { types, positions, constants }
    }

    /// The placeholder at `k` as a column: a value of its type that is no constant, which the
    /// server neither reads nor computes with.
    fn column(&self, k: usize) -> String {
        format!("materialize(defaultValueOfTypeName({}))", types::quoted(&self.types[k].to_string()))
    }

    /// The placeholder's constant numbered `j`, or its last where it has fewer: the expression,
    /// and the value as a message shows it.
    fn constant(&self, k: usize, j: usize) -> &(String, String) {
        let all = &self.constants[k];
        &all[j.min(all.len() - 1)]
    }

    /// What stands for the placeholder at `k`.
    fn written(&self, lexed: &Lexed, k: usize, how: As) -> String {
        let same = |one: usize| lexed.placeholders[k].name == lexed.placeholders[one].name;
        let fixed = self.positions[k] != Position::Value;
        match how {
            As::Columns(j) if fixed => self.constant(k, j).0.clone(),
            As::Columns(_) => self.column(k),
            As::Constants(j) => self.constant(k, j).0.clone(),
            As::Column(one) if k == one => self.column(k),
            As::Column(_) => self.constant(k, 0).0.clone(),
            As::Constant(one) if k == one || fixed => self.constant(k, 0).0.clone(),
            As::Constant(_) => self.column(k),
            As::Varied(one, j) if fixed => self.constant(k, if same(one) { j } else { 0 }).0.clone(),
            As::Varied(..) => self.column(k),
            As::Other(one) => self.constant(k, usize::from(same(one))).0.clone(),
        }
    }

    /// The statement `body` with each placeholder written as `how` says, and the map from that
    /// text back to the author's.
    fn text(&self, body: &str, lexed: &Lexed, how: As) -> Rewritten {
        let mut out = String::with_capacity(body.len());
        let mut spans = Vec::with_capacity(lexed.placeholders.len());
        let mut last = 0;
        for (k, p) in lexed.placeholders.iter().enumerate() {
            out.push_str(&body[last..p.start]);
            let start = out.len();
            out.push_str(&self.written(lexed, k, how));
            spans.push((p.start, p.end, start, out.len()));
            last = p.end;
        }
        out.push_str(&body[last..]);
        Rewritten { sql: out, order: Vec::new(), spans, inserted: Vec::new() }
    }

    /// The server's text with each stand-in read back as the author wrote its parameter, where one
    /// parameter alone has that stand-in: the longest first, since one may hold another.
    fn authored(&self, sql: &str, lexed: &Lexed, message: &str) -> String {
        let mut stood: Vec<(String, usize)> = Vec::new();
        for k in 0..lexed.placeholders.len() {
            stood.push((self.column(k), k));
            stood.extend(self.constants[k].iter().map(|(text, _)| (text.clone(), k)));
        }
        stood.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        let mut out = message.to_string();
        for (text, k) in &stood {
            let p = &lexed.placeholders[*k];
            if !stood.iter().any(|(other, j)| other == text && lexed.placeholders[*j].name != p.name) {
                out = out.replace(text, &sql[p.start..p.end]);
            }
        }
        out
    }
}

/// The server's error as the author reads it: without the code it opens with and the version it
/// ends with, which the diagnostic carries apart.
fn message_of(e: &DbError) -> String {
    let mut text = e.message.trim();
    if let Some(rest) = text.strip_prefix("Code: ") {
        if let Some((_, after)) = rest.split_once(". ") {
            text = after;
        }
    }
    text = text.strip_prefix("DB::Exception: ").unwrap_or(text);
    if let Some(at) = text.rfind(" (version ") {
        text = &text[..at];
    }
    text.trim().to_string()
}

/// The 0-based offset in the text sent that a syntax error names: `failed at position 39`.
fn position_of(message: &str) -> Option<usize> {
    let digits: String = message.split("failed at position ").nth(1)?.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<usize>().ok()?.checked_sub(1)
}

/// A syntax error's parts: what it opens with (`Syntax error`), the token it failed at, and what
/// follows the place, which opens with the text sent from there on: `Syntax error: failed at
/// position 40 (JSON) (line 2, col 29): JSON\n). Expected one of: …`, the line and column only
/// where the text has more than one line.
fn syntax_parts(message: &str) -> Option<(&str, &str, &str)> {
    let (head, rest) = message.split_once("failed at position ")?;
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit()).strip_prefix(" (")?;
    let (token, rest) = match (rest.find(") (line "), rest.find("): ")) {
        (Some(line), Some(colon)) if line < colon => (&rest[..line], rest[line + 2..].split_once("): ")?.1),
        (_, Some(colon)) => (&rest[..colon], &rest[colon + 3..]),
        _ => return None,
    };
    Some((head.trim_end().trim_end_matches(':'), token, rest))
}

/// The diagnostic of an error the server answered a question about `text` with, the question
/// opening with `prefix`: at the place a syntax error names, else the whole statement. A syntax
/// error's message is the server's without the place, which it counts in the text it was sent, and
/// without that text's echo: the diagnostic's own place says where.
fn server_error(sql: &str, lexed: &Lexed, parameters: &Parameters, text: &Rewritten, prefix: &str, e: &DbError) -> Diagnostic {
    let own = message_of(e);
    match position_of(&e.message).and_then(|at| at.checked_sub(prefix.len())) {
        Some(at) => {
            // An error at the end of the statement names the `)` the question closes with.
            let start = text.to_original(at.min(text.sql.len().saturating_sub(1))).min(sql.trim_end().len().saturating_sub(1));
            let after = sql.get(start..).unwrap_or_default();
            let hint = after.strip_prefix(':').filter(|rest| rest.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')).map(|rest| {
                let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                format!(" A ClickHouse parameter is written with its type: {{{name}:Type}}, or :{name}::Type.")
            });
            let (message, length) = match syntax_parts(&own) {
                Some((head, token, rest)) => {
                    let sent = format!("{}\n)", text.sql);
                    let echo = sent.get(at..).unwrap_or_default();
                    let mut cut = rest.bytes().zip(echo.bytes()).take_while(|(a, b)| a == b).count();
                    while !rest.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    let rest = rest[cut..].trim_start().trim_start_matches('.').trim_start();
                    // The token is the author's where the statement has it at that place, and not a stand-in's.
                    let length = if !token.is_empty() && after.starts_with(token) { token.len() } else { 1 };
                    (format!("{head}. {rest}"), length)
                }
                None => (own.clone(), 1),
            };
            Diagnostic { message: format!("{}{}", parameters.authored(sql, lexed, &message), hint.unwrap_or_default()), start, length, code: 6, server_code: e.code.clone() }
        }
        None => whole(sql, parameters.authored(sql, lexed, &own), 6, e.code.clone()),
    }
}

/// Whether a result column's name is one a statement can rely on: a name, as an alias or a column
/// read as it is has, and not an expression's text, which ClickHouse names an unnamed column by.
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_') && chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// The length of the name `text` opens with: letters, digits and `_`, or a name in backquotes or
/// double quotes, in which a backslash escapes.
fn name_length(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    match *bytes.first()? {
        quote @ (b'`' | b'"') => {
            let mut i = 1;
            while i < bytes.len() && bytes[i] != quote {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
            (i < bytes.len()).then_some(i + 1)
        }
        b if b.is_ascii_alphabetic() || b == b'_' || b >= 0x80 => Some(bytes.iter().position(|b| !(b.is_ascii_alphanumeric() || *b == b'_' || *b >= 0x80)).unwrap_or(bytes.len())),
        _ => None,
    }
}

/// Whether an item of a select list is a `*`: alone, a table's, with its modifiers (`* EXCEPT (a)`),
/// or a `COLUMNS('…')` matcher. Each stands for any number of columns.
fn is_star(item: &str) -> bool {
    let item = item.trim();
    let after_table = item.rsplit_once(".*").map(|(_, rest)| rest);
    item.starts_with('*') || after_table.is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace)) || item.get(..8).is_some_and(|head| head.eq_ignore_ascii_case("columns("))
}

/// Whether an item's own text says its column's name: a column read as it is, by its name behind
/// any table's or tuple's (`u.name`, `spot.x`, `` `odd name` ``), or an alias after `AS`.
fn names_itself(item: &str) -> bool {
    let item = item.trim();
    // Names joined by dots, and nothing else.
    let mut rest = item;
    let column = loop {
        let Some(length) = name_length(rest) else { break false };
        rest = &rest[length..];
        match rest.strip_prefix('.') {
            Some(after) => rest = after,
            None => break rest.is_empty(),
        }
    };
    // `… AS name`: the item's last name, after the word.
    let alias = || {
        let bytes = item.as_bytes();
        let start = match *bytes.last()? {
            quote @ (b'`' | b'"') => (0..bytes.len() - 1).rev().find(|&i| bytes[i] == quote && (i == 0 || bytes[i - 1] != b'\\'))?,
            _ => bytes.iter().rposition(|b| !(b.is_ascii_alphanumeric() || *b == b'_' || *b >= 0x80)).map_or(0, |i| i + 1),
        };
        (name_length(&item[start..]) == Some(item.len() - start)).then_some(())?;
        let before = item[..start].trim_end();
        let word = before.len().checked_sub(2).and_then(|at| before.get(at..))?;
        (word.eq_ignore_ascii_case("as") && before[..before.len() - 2].ends_with(|c: char| c.is_whitespace() || c == ')' || c == ']' || c == '\'' || c == '`' || c == '"')).then_some(())
    };
    column || alias().is_some()
}

/// The statement's select list, as far as its text and its columns' names say which of its items
/// name their column. An item is named where the server's name for its column reads as one, as an
/// alias and a column's own do, or where its text says the name, as a quoted alias and a member
/// read by its path do (`items.sku`); else the server made the name up from the expression. Where
/// the text's items cannot be the columns', each column is judged by its name alone.
fn targets(sql: &str, columns: &[Column]) -> Vec<ResultTarget> {
    let by_name = || columns.iter().map(|c| if is_identifier(&c.name) { ResultTarget::Named } else { ResultTarget::Unnamed(None) }).collect::<Vec<_>>();
    let Some(items) = analysis::result_items(sql, Dialect::Clickhouse) else { return by_name() };
    let texts: Vec<&str> = items.iter().map(|&(start, length)| &sql[start..start + length]).collect();
    let stars = texts.iter().filter(|text| is_star(text)).count();
    if (stars == 0 && texts.len() != columns.len()) || texts.len() - stars > columns.len() {
        return by_name();
    }
    let first = texts.iter().position(|text| is_star(text)).unwrap_or(texts.len());
    let last = texts.iter().rposition(|text| is_star(text)).unwrap_or(0);
    texts
        .iter()
        .enumerate()
        .map(|(k, text)| {
            if is_star(text) {
                return ResultTarget::Star;
            }
            // The item's column: counted from the left before the first `*`, from the right after
            // the last, and unknown between two.
            let column = if k < first { columns.get(k) } else if k > last { columns.get(columns.len() - (texts.len() - k)) } else { None };
            match column {
                Some(column) if !is_identifier(&column.name) && !names_itself(text) => ResultTarget::Unnamed(Some(items[k])),
                _ => ResultTarget::Named,
            }
        })
        .collect()
}

pub fn analyze(sql: &str, host: &Host) -> Lazy<Analysis> {
    let lexed = match lex::lex(sql, Dialect::Clickhouse) {
        Ok(lexed) => lexed,
        Err(e) => return Some(analysis::bare(sql, &analysis::none(), Dialect::Clickhouse, vec![analysis::diag(e.message, e.offset, 1, 1)])),
    };
    let refuse = |d: Vec<Diagnostic>| Some(analysis::bare(sql, &lexed, Dialect::Clickhouse, d));
    let statement = match lex::one_statement(sql, &lexed, &["select", "with", "from", "insert", "delete", "update", "alter"]) {
        Ok(statement) => statement,
        Err(message) => return refuse(vec![whole(sql, message, 10, None)]),
    };
    let word = lex::first_word(sql, &lexed);
    // Any other first word is read as a query's, and the server's error names it (`selec`).
    let writes = matches!(word.as_str(), "insert" | "delete" | "update" | "alter");
    if let lex::Statement::Command(command) = &statement {
        return refuse(vec![whole(sql, lex::refused_command(&format!("Reylo types a ClickHouse SELECT, INSERT, DELETE or UPDATE, and {} is none of them", command.to_ascii_uppercase())), 10, None)]);
    }
    // Each placeholder's type, as it declares it: the type its text names, or, where that is no
    // name Reylo reads, the type the server reads it as, which is how an alias (`int`, `varchar`)
    // and a name in other letters (`date`) are the types they stand for.
    let texts: Vec<Option<&str>> = lexed.placeholders.iter().map(|p| written_type(sql, p).map(str::trim).filter(|text| !text.is_empty())).collect();
    let written: Vec<Option<Named>> = texts.iter().map(|text| text.and_then(types::parse)).collect();
    let named: Vec<_> = texts
        .iter()
        .zip(&written)
        .map(|(text, ty)| {
            let known = ty.as_ref().is_some_and(|ty| ty.name == "Identifier" || types::parameter(ty).is_ok());
            text.filter(|_| !known).map(|text| host.query(&format!("{TYPE_NAME}{}))", types::quoted(text)), Vec::new(), Scope::Session))
        })
        .collect();
    let mut resolved: Vec<Option<Named>> = Vec::with_capacity(named.len());
    for answer in named {
        resolved.push(match answer {
            Some(answer) => answer?.ok().and_then(|rows| rows.first().and_then(|row| row.first()).and_then(as_str).and_then(types::parse)),
            None => None,
        });
    }
    let mut declared: Vec<Named> = Vec::with_capacity(lexed.placeholders.len());
    let mut refused: Vec<Diagnostic> = Vec::new();
    for (k, p) in lexed.placeholders.iter().enumerate() {
        let text = &sql[p.start..p.end];
        let Some(ty) = resolved[k].clone().or_else(|| written[k].clone()) else {
            refused.push(at(p, format!("{text} names no type: a ClickHouse parameter is written with its type, as in {{{}:UInt64}}.", p.name), 10));
            declared.push(Named { name: "Nothing".into(), args: Vec::new() });
            continue;
        };
        if ty.name == "Identifier" {
            refused.push(at(p, format!("{text} stands for a name, so the statement's columns and their types would change with its value: write the name in the statement."), 10));
        } else if let Err(types::Unread(name)) = types::parameter(&ty) {
            refused.push(at(p, format!("Reylo binds no parameter of the type {name}: it is no ClickHouse type, or one whose values it does not write yet, as a JSON, a Dynamic or an interval is."), 10));
        } else if ty.holds("Variant") {
            // A Variant's text does not say which of its types a value is: 7 reads as a number and as a string.
            refused.push(at(p, format!("{text} is a Variant, whose value's text does not say which of its types it is: declare the parameter with the one type it has."), 10));
        } else if let Some(first) = lexed.placeholders[..k].iter().position(|q| q.name == p.name).filter(|&first| declared[first] != ty) {
            refused.push(at(p, format!("The parameter {} is declared {} here and {} at its first use: declare it with one type.", p.name, ty, declared[first]), 3));
        }
        declared.push(ty);
    }
    if !refused.is_empty() {
        return refuse(refused);
    }
    let mut parameters = Parameters::of(declared, lexed.placeholders.iter().map(|p| position(sql, &lexed, p)).collect());
    // The statement without what follows it: a `;`, and the whitespace and comments after one.
    let in_text = |i: usize| lexed.opaque.iter().any(|&(start, end)| start <= i && i < end);
    let body = &sql[..sql.bytes().enumerate().find(|&(i, c)| c == b';' && !in_text(i)).map_or(sql.len(), |(i, _)| i)];
    let asked = parameters.text(body, &lexed, As::Columns(0));
    let kind = match word.as_str() {
        "insert" => StatementKind::Insert,
        "delete" => StatementKind::Delete,
        "update" => StatementKind::Update,
        "alter" => StatementKind::Other,
        _ => StatementKind::Select,
    };
    let mut typed = analysis::bare(sql, &lexed, Dialect::Clickhouse, Vec::new());
    typed.kind = Some(kind);
    // The statement is sent as it is written, but for a type the server named, which is written
    // by that name, so that the run time reads every parameter's type by the name ClickHouse gives it.
    typed.rewritten = lex::rewrite_with(sql, &lexed, Target::Braced, &|name, _| {
        let k = lexed.placeholders.iter().position(|p| p.name == name).unwrap_or(0);
        match &resolved[k] {
            Some(_) => format!("{{{name}:{}}}", parameters.types[k]),
            None => format!("{{{name}:{}}}", written_type(sql, &lexed.placeholders[k]).unwrap_or_default()),
        }
    });
    typed.params = lexed
        .params
        .iter()
        .map(|name| {
            let ty = &parameters.types[lexed.placeholders.iter().position(|p| &p.name == name).unwrap_or(0)];
            let slot = types::parameter(ty).expect("a type the parameters were held to");
            Param { nullable: slot.nullable, ..Param::new(name.clone(), slot.ty) }
        })
        .collect();

    if writes {
        return write(sql, &lexed, &parameters, body, &asked, typed, host);
    }

    let describe = |parameters: &Parameters, how: As| host.query(&format!("{DESCRIBE}{}\n)", parameters.text(body, &lexed, how).sql), Vec::new(), Scope::Statement);
    // One round: the describe, the query tree, the session's settings, and the statement as it
    // runs, each parameter a constant, with two values of each.
    let described = describe(&parameters, As::Columns(0));
    let tree = host.query(&format!("{TREE}{}", asked.sql), Vec::new(), Scope::Statement);
    let settings = host.query(ROW_SETTINGS, Vec::new(), Scope::Session);
    let run = (!lexed.placeholders.is_empty()).then(|| (describe(&parameters, As::Constants(0)), describe(&parameters, As::Constants(1))));
    let (described, tree, settings) = (described?, tree?, settings?);
    let run = match run {
        Some((first, second)) => Some((first?, second?)),
        None => None,
    };
    let (rows, tree) = match described {
        Ok(rows) => (rows, tree),
        Err(error) => {
            // The parameters the server reads as constants, each written as one from here on.
            let found = match constant_needed(sql, &lexed, &parameters, body, &asked, &error, host)? {
                Ok(found) => found,
                Err(diagnostics) => return refuse(diagnostics),
            };
            for &k in &found {
                parameters.positions[k] = Position::Constant;
            }
            let asked = parameters.text(body, &lexed, As::Columns(0));
            let described = describe(&parameters, As::Columns(0));
            let tree = host.query(&format!("{TREE}{}", asked.sql), Vec::new(), Scope::Statement);
            // The statement has one type only where another value of each leaves its types alone.
            let ones = once(&lexed, found.iter().copied());
            let others: Vec<_> = ones.iter().map(|&k| [1, 2].map(|j| describe(&parameters, As::Varied(k, j)))).collect();
            let (described, tree) = (described?, tree?);
            let rows = match described {
                Ok(rows) => rows,
                Err(e) => return refuse(vec![server_error(sql, &lexed, &parameters, &asked, DESCRIBE, &e)]),
            };
            let mut refusals = Vec::new();
            for (&k, [second, third]) in ones.iter().zip(others) {
                refusals.extend(held(sql, &lexed, &parameters, k, &rows, [second?, third?]));
            }
            if !refusals.is_empty() {
                return refuse(refusals);
            }
            (rows, tree)
        }
    };
    // As the statement runs, every parameter is a constant, which a function may type otherwise
    // than a column. Two describes of that statement that agree give its types; two that differ
    // show a value changing a type; one that fails computed with a value, and says nothing.
    let rows = match run {
        Some((Ok(first), Ok(second))) if types_of(&first) == types_of(&second) => first,
        Some((Ok(first), Ok(second))) => return changing(sql, &lexed, &parameters, body, &first, &second, host).map(|d| analysis::bare(sql, &lexed, Dialect::Clickhouse, d)),
        _ => rows,
    };
    let mut unread = Vec::new();
    let columns: Vec<Column> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let name = row.first().and_then(as_str).unwrap_or_default().to_string();
            let described = row.get(1).and_then(as_str).unwrap_or_default();
            match types::parse(described).map(|ty| (types::slot(&ty), ty)) {
                Some((Ok(slot), ty)) => Column { described: Some(ty.to_string()), ..Column::new(name, slot.ty, slot.nullable) },
                Some((Err(types::Unread(unread_name)), _)) => {
                    unread.push(whole(sql, format!("Column {} ({name}) has the type {described}, and Reylo does not read a ClickHouse {unread_name} yet: select it as a type it reads, as toString({name}) or toJSONString({name}) does.", i + 1), 10, None));
                    Column::new(name, Type::unknown(), true)
                }
                None => {
                    unread.push(whole(sql, format!("Column {} ({name}) has the type {described}, whose name Reylo cannot read.", i + 1), 10, None));
                    Column::new(name, Type::unknown(), true)
                }
            }
        })
        .collect();
    if !unread.is_empty() {
        return refuse(unread);
    }
    // How many rows: what the tree proves, where the session's settings leave an aggregate its row.
    // A limit a parameter gives is the caller's, and so is whatever a parameter the server reads as
    // a constant decides, whose first value the tree was read with.
    let limit_given = parameters.positions.iter().any(|p| matches!(p, Position::Limit | Position::Constant));
    let lines: Vec<&str> = tree.as_ref().map(|rows| rows.iter().filter_map(|row| row.first().and_then(as_str)).flat_map(str::lines).collect()).unwrap_or_default();
    let proven = crate::tree::rows(&lines, limit_given);
    let defaults = settings.ok().is_some_and(|rows| rows.len() == 3 && rows.iter().all(|row| row.get(1).and_then(as_str) == Some("0")));
    let (cardinality, reason) = if proven.one && defaults {
        (Cardinality::One, None)
    } else if proven.one {
        (Cardinality::AtMostOne, Some("a setting of the session (empty_result_for_aggregation_by_empty_set, limit or offset) may drop the aggregate's one row"))
    } else if proven.aggregate {
        (Cardinality::AtMostOne, Some("an aggregate with no GROUP BY has one row, which the statement's HAVING, QUALIFY, OFFSET or LIMIT may drop"))
    } else if proven.at_most_one {
        (Cardinality::AtMostOne, Some("its LIMIT of 1 bounds the rows, and nothing says one is there"))
    } else {
        (Cardinality::Many, Some("ClickHouse enforces no unique key, so only a LIMIT of 1 or an aggregate with no GROUP BY bounds a result"))
    };
    typed.columns = columns;
    typed.cardinality = cardinality;
    typed.cardinality_reason = reason.map(String::from);
    typed.read_only = true;
    // ClickHouse names a column with no alias by its expression's text, which is no name to rely on.
    let targets = targets(sql, &typed.columns);
    analysis::names(sql, Dialect::Clickhouse, &mut typed, Some(&targets), &|_| None);
    Some(typed)
}

/// The types a describe answered with, in the order of its columns.
fn types_of(rows: &[Vec<Value>]) -> Vec<String> {
    rows.iter().map(|row| row.get(1).and_then(as_str).map_or_else(String::new, |ty| types::parse(ty).map_or_else(|| ty.to_string(), |ty| ty.to_string()))).collect()
}

/// How two describes of one statement differ, the parameter at `k` written as `first` in one and
/// as `second` in the other: the first column whose type changed, or the columns' count.
fn difference(first: (&[Vec<Value>], &str), second: (&[Vec<Value>], &str)) -> String {
    let (a, b) = (types_of(first.0), types_of(second.0));
    match a.iter().zip(&b).position(|(a, b)| a != b) {
        Some(i) => {
            let name = first.0[i].first().and_then(as_str).unwrap_or_default();
            format!("column {} ({name}) has the type {} with {} and {} with {}", i + 1, a[i], first.1, b[i], second.1)
        }
        None => format!("the result has {} column(s) with {} and {} with {}", a.len(), first.1, b.len(), second.1),
    }
}

/// The parameter at `k` once among those of its name, so that one parameter is asked about once.
fn once(lexed: &Lexed, indexes: impl Iterator<Item = usize>) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for k in indexes {
        if !out.iter().any(|&j| lexed.placeholders[j].name == lexed.placeholders[k].name) {
            out.push(k);
        }
    }
    out
}

/// The refusal of a parameter the server reads as a constant: `why` says what its value changes,
/// and `error` is the server's own, where it gave one.
fn refused(sql: &str, lexed: &Lexed, parameters: &Parameters, k: usize, why: &str, error: Option<&DbError>) -> Diagnostic {
    let p = &lexed.placeholders[k];
    let quoted = error.map(|e| format!(" ({})", parameters.authored(sql, lexed, &message_of(e)).trim_end_matches('.'))).unwrap_or_default();
    at(p, format!("ClickHouse reads {} as a constant here, and {why}. Write the value in the statement.{quoted}", &sql[p.start..p.end]), 10)
}

/// Why the describe failed: the parameters the server reads as constants, each by its index, or
/// the diagnostics of a statement that cannot be typed. Where the statement describes with every
/// parameter a constant, each parameter is asked about alone, as a column among constants, and
/// the ones the server refuses so are found. Where it fails with every parameter a constant too,
/// and in another way, a parameter's first constant is refused in its turn, as `'a'` is for the
/// type `CAST(x, {t:String})` names: it is the one parameter whose writing as a constant changes
/// the describe's answer, where one alone does. Else the statement is wrong whatever its
/// parameters are, and the error is the describe's.
fn constant_needed(sql: &str, lexed: &Lexed, parameters: &Parameters, body: &str, asked: &Rewritten, error: &DbError, host: &Host) -> Lazy<Result<Vec<usize>, Vec<Diagnostic>>> {
    let values: Vec<usize> = (0..lexed.placeholders.len()).filter(|&k| parameters.positions[k] == Position::Value).collect();
    let own_error = || vec![server_error(sql, lexed, parameters, asked, DESCRIBE, error)];
    if values.is_empty() {
        return Some(Err(own_error()));
    }
    let describe = |how: As| host.query(&format!("{DESCRIBE}{}\n)", parameters.text(body, lexed, how).sql), Vec::new(), Scope::Statement);
    let constants = describe(As::Constants(0));
    let alone: Vec<_> = values.iter().map(|&k| describe(As::Column(k))).collect();
    if let Err(with_constants) = constants? {
        // The same error either way, each stand-in read back as its parameter, is the statement's own.
        let same = |a: &DbError, b: &DbError| a.code == b.code && parameters.authored(sql, lexed, &message_of(a)) == parameters.authored(sql, lexed, &message_of(b));
        if same(&with_constants, error) {
            return Some(Err(own_error()));
        }
        let as_constant: Vec<_> = values.iter().map(|&k| describe(As::Constant(k))).collect();
        let mut passed = Vec::new();
        let mut changed = Vec::new();
        for (&k, answer) in values.iter().zip(as_constant) {
            match answer? {
                // The statement describes once this one parameter is a constant.
                Ok(_) => passed.push(k),
                Err(e) if !same(&e, error) => changed.push((k, e)),
                Err(_) => {}
            }
        }
        return Some(match (passed.is_empty(), &changed[..]) {
            (false, _) => checked(sql, lexed, parameters, passed),
            (true, [(k, e)]) => Err(vec![refused(sql, lexed, parameters, *k, &format!("the statement fails with {} in its place, so its value is part of what the statement says", parameters.constant(*k, 0).1), Some(e))]),
            _ => Err(own_error()),
        });
    }
    let mut found = Vec::new();
    for (&k, answer) in values.iter().zip(alone) {
        if answer?.is_err() {
            found.push(k);
        }
    }
    // Refused with its parameters columns, described with them constants, and no one of them at
    // fault alone: they are constants together.
    Some(checked(sql, lexed, parameters, if found.is_empty() { values } else { found }))
}

/// The parameters the server reads as constants, where each has a second value to describe the
/// statement with; else the refusal of each that has one value alone.
fn checked(sql: &str, lexed: &Lexed, parameters: &Parameters, found: Vec<usize>) -> Result<Vec<usize>, Vec<Diagnostic>> {
    let single: Vec<Diagnostic> = found
        .iter()
        .filter(|&&k| parameters.constants[k].len() < 2)
        .map(|&k| refused(sql, lexed, parameters, k, &format!("Reylo has one value of the type {} to describe the statement with, which cannot show that the result's types hold for every value", parameters.types[k]), None))
        .collect();
    if single.is_empty() {
        Ok(found)
    } else {
        Err(single)
    }
}

/// Whether the statement's types hold as the parameter at `k`, which the server reads as a
/// constant, takes its other values: `None` where one of them describes with the types of `rows`,
/// the describe with its first value, and none with other types; else its refusal. A value the
/// statement fails with says nothing of its types, since the statement fails with it as it runs
/// too: a level of 2 is no quantile's.
fn held(sql: &str, lexed: &Lexed, parameters: &Parameters, k: usize, rows: &[Vec<Value>], others: [Result<Vec<Vec<Value>>, DbError>; 2]) -> Option<Diagnostic> {
    let first = parameters.constant(k, 0).1.as_str();
    let mut agreed = false;
    let mut failed = None;
    for (j, other) in [1, 2].into_iter().zip(others) {
        let shown = parameters.constant(k, j).1.as_str();
        match other {
            Ok(with_other) if types_of(&with_other) == types_of(rows) => agreed = true,
            Ok(with_other) => return Some(refused(sql, lexed, parameters, k, &format!("the result's types change with its value: {}", difference((rows, first), (with_other.as_slice(), shown))), None)),
            Err(e) => failed = failed.or(Some((shown, e))),
        }
    }
    if agreed {
        return None;
    }
    let (shown, e) = failed?;
    Some(refused(sql, lexed, parameters, k, &format!("the statement fails with each other value tried in its place, as with {shown}, so nothing shows that the result's types hold for every value"), Some(&e)))
}

/// Why the statement has no one type as it runs, every parameter a constant: `first` and `second`
/// are its describes with each parameter's first constant and its second, and they differ. Each
/// parameter is asked about alone, the one given its second constant among the others' first.
fn changing(sql: &str, lexed: &Lexed, parameters: &Parameters, body: &str, first: &[Vec<Value>], second: &[Vec<Value>], host: &Host) -> Lazy<Vec<Diagnostic>> {
    let ones = once(lexed, (0..lexed.placeholders.len()).filter(|&k| parameters.constants[k].len() > 1));
    let asked: Vec<_> = ones.iter().map(|&k| host.query(&format!("{DESCRIBE}{}\n)", parameters.text(body, lexed, As::Other(k)).sql), Vec::new(), Scope::Statement)).collect();
    let mut out = Vec::new();
    for (&k, answer) in ones.iter().zip(asked) {
        if let Ok(with_other) = answer? {
            if types_of(&with_other) != types_of(first) {
                let p = &lexed.placeholders[k];
                let why = difference((first, parameters.constant(k, 0).1.as_str()), (with_other.as_slice(), parameters.constant(k, 1).1.as_str()));
                out.push(at(p, format!("The result's types change with the value of {}: {why}. Write the value in the statement.", &sql[p.start..p.end]), 10));
            }
        }
    }
    if out.is_empty() {
        out.push(whole(sql, format!("The result's types change with the values of the statement's parameters: {}. Write the values in the statement.", difference((first, "one value of each"), (second, "another"))), 10, None));
    }
    Some(out)
}

/// The analysis of a write, which returns no rows. An INSERT is built, and not run, by `EXPLAIN
/// PIPELINE`, which checks its table and columns and the SELECT it reads; a DELETE is described as
/// the SELECT of the rows it would delete, where that reads as one; any write is parsed.
fn write(sql: &str, lexed: &Lexed, parameters: &Parameters, body: &str, asked: &Rewritten, mut typed: Analysis, host: &Host) -> Lazy<Analysis> {
    let refuse = |d: Vec<Diagnostic>| Some(analysis::bare(sql, lexed, Dialect::Clickhouse, d));
    let insert = typed.kind == Some(StatementKind::Insert);
    // An insert of rows in a format reads them from the request's body, which a parameter is not:
    // its FORMAT comes before any VALUES or SELECT.
    if insert {
        let first = words(body, lexed).into_iter().find(|(_, word)| matches!(word.as_str(), "format" | "values" | "select" | "with" | "from"));
        if let Some((start, _)) = first.filter(|(_, word)| word == "format") {
            return refuse(vec![Diagnostic { message: "An INSERT … FORMAT takes its rows from the request's body, which a typed statement does not have: write the rows as VALUES, or as a SELECT, with parameters.".into(), start, length: "format".len(), code: 10, server_code: None }]);
        }
    }
    // A write returns no types for a parameter's value to change, so a parameter the server reads
    // as a constant is one: the statement is asked about with its parameters columns, and where
    // that is refused, with them constants, and it stands where either answers.
    let constants = parameters.text(body, lexed, As::Constants(0));
    let parsed = host.query(&format!("{AST}{}", asked.sql), Vec::new(), Scope::Statement);
    let built = insert.then(|| [asked, &constants].map(|text| host.query(&format!("{PIPELINE}{}", text.sql), Vec::new(), Scope::Statement)));
    // `DELETE FROM t WHERE c` reads as `SELECT 1 FROM t WHERE c`, the first word alone changed.
    let deleted = (typed.kind == Some(StatementKind::Delete)).then(|| {
        let cut = (code_after(sql, lexed, 0) + "delete".len()).min(body.len());
        let select = format!("SELECT 1{}", &body[cut..]);
        let moved = shifted(lexed, "SELECT 1".len() as isize - cut as isize, cut);
        let [columns, constants] = [As::Columns(0), As::Constants(0)].map(|how| parameters.text(&select, &moved, how).sql);
        (host.query(&format!("{AST}{columns}"), Vec::new(), Scope::Statement), [columns, constants].map(|text| host.query(&format!("{DESCRIBE}{text}\n)"), Vec::new(), Scope::Statement)))
    });
    if let Err(e) = parsed? {
        return refuse(vec![server_error(sql, lexed, parameters, asked, AST, &e)]);
    }
    if let Some([columns, constants]) = built {
        if let (Err(e), Err(_)) = (columns?, constants?) {
            return refuse(vec![server_error(sql, lexed, parameters, asked, PIPELINE, &e)]);
        }
    }
    if let Some((reads, [columns, constants])) = deleted {
        // Where the text is no SELECT, as with ON CLUSTER or IN PARTITION, nothing more is asked.
        if let (Ok(_), Err(e), Err(_)) = (reads?, columns?, constants?) {
            return refuse(vec![whole(sql, parameters.authored(sql, lexed, &message_of(&e)), 6, e.code.clone())]);
        }
    }
    typed.cardinality = Cardinality::None;
    Some(typed)
}

/// The words of the statement's code, lowercased, each with where it starts: its strings, quoted
/// names, comments and placeholders left out.
fn words(body: &str, lexed: &Lexed) -> Vec<(usize, String)> {
    let b = body.as_bytes();
    let skip = |i: usize| lexed.opaque.iter().map(|&(start, end)| (start, end)).chain(lexed.placeholders.iter().map(|p| (p.start, p.end))).find(|&(start, end)| start <= i && i < end).map(|(_, end)| end);
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if let Some(end) = skip(i) {
            i = end;
        } else if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((start, body[start..i].to_ascii_lowercase()));
        } else {
            i += 1;
        }
    }
    out
}

/// The placeholders of a text whose first `cut` bytes were replaced by text `shift` bytes longer.
fn shifted(lexed: &Lexed, shift: isize, cut: usize) -> Lexed {
    let moved = |at: usize| (at as isize + shift).max(0) as usize;
    Lexed {
        params: lexed.params.clone(),
        placeholders: lexed.placeholders.iter().filter(|p| p.start >= cut).map(|p| Placeholder { name: p.name.clone(), start: moved(p.start), end: moved(p.end) }).collect(),
        style: lexed.style,
        opaque: lexed.opaque.iter().filter(|&&(start, _)| start >= cut).map(|&(start, end)| (moved(start), moved(end))).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Answer, Answers, Need};
    use serde_json::{json, Value};

    /// The analysis of `sql`, each query answered by `answer`.
    fn run(sql: &str, answer: &dyn Fn(&str) -> Result<Vec<Vec<Value>>, DbError>) -> Analysis {
        let mut answers = Answers::new();
        loop {
            let host = Host::new(&answers);
            if let Some(analysis) = analyze(sql, &host) {
                return analysis;
            }
            for need in host.needs() {
                let Need::Query { id, sql, .. } = need else { panic!("ClickHouse asks no describe") };
                answers.insert(id, match answer(&sql) {
                    Ok(rows) => Answer::Rows { rows },
                    Err(error) => Answer::Error { error },
                });
            }
        }
    }

    fn error(message: &str, code: &str) -> DbError {
        DbError { message: message.into(), code: Some(code.into()), position: None }
    }

    const DEFAULTS: fn() -> Vec<Vec<Value>> = || vec![vec![json!("empty_result_for_aggregation_by_empty_set"), json!("0")], vec![json!("limit"), json!("0")], vec![json!("offset"), json!("0")]];

    /// A server that describes every statement as `columns` and gives `tree` as its query tree.
    fn server<'a>(columns: &'a [(&'a str, &'a str)], tree: &'a str) -> impl Fn(&str) -> Result<Vec<Vec<Value>>, DbError> + 'a {
        move |sql| {
            if sql.starts_with(DESCRIBE) {
                Ok(columns.iter().map(|(name, ty)| vec![json!(name), json!(ty)]).collect())
            } else if sql.starts_with(TREE) {
                Ok(tree.lines().map(|line| vec![json!(line)]).collect())
            } else if sql.starts_with(TYPE_NAME) {
                Err(error("Code: 50. DB::Exception: Unknown data type family: nope. (UNKNOWN_TYPE) (version 26.9.23.15 (official build))", "50"))
            } else {
                Ok(DEFAULTS())
            }
        }
    }

    #[test]
    fn a_statement_is_typed_from_its_describe_and_its_parameters_from_their_declarations() {
        let sql = "SELECT id, email, tags FROM users WHERE id = {id:UInt64} AND created > {since:Nullable(DateTime64(3, 'UTC'))}";
        let a = run(sql, &server(&[("id", "UInt64"), ("email", "Nullable(String)"), ("tags", "Array(String)")], "QUERY id: 0"));
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(a.kind, Some(StatementKind::Select));
        assert_eq!(a.cardinality, Cardinality::Many);
        assert_eq!(a.columns.iter().map(|c| (c.name.as_str(), c.nullable, c.described.as_deref())).collect::<Vec<_>>(), [("id", false, Some("UInt64")), ("email", true, Some("Nullable(String)")), ("tags", false, Some("Array(String)"))]);
        assert_eq!(a.params.iter().map(|p| (p.name.as_str(), p.nullable, p.ty.label.as_deref())).collect::<Vec<_>>(), [("id", false, Some("UInt64")), ("since", true, Some("DateTime64"))]);
        // A statement written ClickHouse's way is sent as it is written.
        assert_eq!(a.rewritten.sql, sql);
        assert_eq!(a.rewritten.order, ["id", "since"]);
        assert!(a.read_only);
    }

    #[test]
    fn the_describe_is_asked_with_each_parameter_as_a_column_but_where_the_text_says_a_constant() {
        let asked = std::cell::RefCell::new(Vec::new());
        let sql = "SELECT max(age) AS q FROM users WHERE name = {name:String} AND id IN ({one:UInt8}, 5) AND id NOT IN {ids:Array(UInt64)} AND id IN (SELECT id FROM t WHERE a = {a:Nullable(UInt8)}) -- why\n LIMIT {skip:UInt64}, {count:UInt64};\n";
        let a = run(sql, &|sent| {
            asked.borrow_mut().push(sent.to_string());
            server(&[("q", "UInt8")], "QUERY id: 0")(sent)
        });
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        let describes: Vec<String> = asked.borrow().iter().filter(|s| s.starts_with(DESCRIBE)).cloned().collect();
        // A column where a value goes, the subquery of an IN included; a constant after LIMIT and on the right of an IN.
        assert_eq!(
            describes[0],
            "DESCRIBE (\nSELECT max(age) AS q FROM users WHERE name = materialize(defaultValueOfTypeName('String')) AND id IN (CAST(1, 'UInt8'), 5) AND id NOT IN CAST([CAST(1, 'UInt64')], 'Array(UInt64)') AND id IN (SELECT id FROM t WHERE a = materialize(defaultValueOfTypeName('Nullable(UInt8)'))) -- why\n LIMIT CAST(1, 'UInt64'), CAST(1, 'UInt64')\n)"
        );
        // Then the statement as it runs, each parameter a constant, with two values of each.
        assert_eq!(
            describes[1],
            "DESCRIBE (\nSELECT max(age) AS q FROM users WHERE name = CAST('a', 'String') AND id IN (CAST(1, 'UInt8'), 5) AND id NOT IN CAST([CAST(1, 'UInt64')], 'Array(UInt64)') AND id IN (SELECT id FROM t WHERE a = CAST(CAST(1, 'UInt8'), 'Nullable(UInt8)')) -- why\n LIMIT CAST(1, 'UInt64'), CAST(1, 'UInt64')\n)"
        );
        assert_eq!(
            describes[2],
            "DESCRIBE (\nSELECT max(age) AS q FROM users WHERE name = CAST('b', 'String') AND id IN (CAST(2, 'UInt8'), 5) AND id NOT IN CAST([], 'Array(UInt64)') AND id IN (SELECT id FROM t WHERE a = CAST(NULL, 'Nullable(UInt8)')) -- why\n LIMIT CAST(2, 'UInt64'), CAST(2, 'UInt64')\n)"
        );
        assert_eq!(describes.len(), 3);
        // A limit a parameter gives bounds nothing.
        assert_eq!(a.cardinality, Cardinality::Many);
    }

    #[test]
    fn a_statement_has_the_types_it_runs_with_where_a_constant_is_typed_otherwise_than_a_column() {
        // ClickHouse types `x IN (constants)` a UInt8, and a Nullable(UInt8) where one of them is a column.
        let by_text = |sent: &str| match sent.strip_prefix(DESCRIBE) {
            Some(text) if text.contains("materialize(") => Ok(vec![vec![json!("hit"), json!("Nullable(UInt8)")]]),
            Some(_) => Ok(vec![vec![json!("hit"), json!("UInt8")]]),
            None => server(&[], "QUERY id: 0")(sent),
        };
        let a = run("SELECT (id, age) IN ((1, {age:UInt8})) AS hit FROM users", &by_text);
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!((a.columns[0].nullable, a.columns[0].described.as_deref()), (false, Some("UInt8")));
        // Two values of a parameter that give two types: the statement has no one type.
        let sql = "SELECT pick(age, {n:UInt8}) AS hit FROM users";
        let b = run(sql, &|sent| match sent.strip_prefix(DESCRIBE) {
            Some(text) if text.contains("CAST(2, 'UInt8')") => Ok(vec![vec![json!("hit"), json!("String")]]),
            Some(_) => Ok(vec![vec![json!("hit"), json!("UInt8")]]),
            None => server(&[], "QUERY id: 0")(sent),
        });
        assert_eq!(b.diagnostics.len(), 1);
        assert_eq!(&sql[b.diagnostics[0].start..b.diagnostics[0].start + b.diagnostics[0].length], "{n:UInt8}");
        assert_eq!(b.diagnostics[0].message, "The result's types change with the value of {n:UInt8}: column 1 (hit) has the type UInt8 with 1 and String with 2. Write the value in the statement.");
        // A value the server computed with, and failed on, says nothing of the statement.
        let c = run("SELECT toDate({day:String}) AS day", &|sent| match sent.strip_prefix(DESCRIBE) {
            Some(text) if text.contains("materialize(") => Ok(vec![vec![json!("day"), json!("Date")]]),
            Some(_) => Err(error("Code: 38. DB::Exception: Cannot parse date: In scope SELECT toDate(CAST('a', 'String')). (CANNOT_PARSE_DATE) (version 26.9)", "38")),
            None => server(&[], "QUERY id: 0")(sent),
        });
        assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
        assert_eq!(c.columns[0].described.as_deref(), Some("Date"));
    }

    #[test]
    fn a_parameter_written_with_a_cast_is_sent_the_server_s_way() {
        let a = run("SELECT id FROM users WHERE id = :id::UInt64 AND name IN :names::Array(String)", &server(&[("id", "UInt64")], "QUERY id: 0"));
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(a.rewritten.sql, "SELECT id FROM users WHERE id = {id:UInt64}::UInt64 AND name IN {names:Array(String)}::Array(String)");
        assert_eq!(a.params.iter().map(|p| p.ty.kind.name()).collect::<Vec<_>>(), ["integer", "array"]);
    }

    #[test]
    fn a_type_written_by_another_name_is_the_type_the_server_reads_it_as() {
        let sql = "SELECT id FROM users WHERE id = {id:int} AND name IN {names:Array(varchar)} AND age > {age:UInt8} AND id < {id:Int32} AND age < {most:TINYINT UNSIGNED}";
        let a = run(sql, &|sent| match sent.strip_prefix(TYPE_NAME) {
            Some("'int'))") => Ok(vec![vec![json!("Int32")]]),
            Some("'Array(varchar)'))") => Ok(vec![vec![json!("Array(String)")]]),
            Some("'TINYINT UNSIGNED'))") => Ok(vec![vec![json!("UInt8")]]),
            _ => server(&[("id", "UInt64")], "QUERY id: 0")(sent),
        });
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(a.params.iter().map(|p| (p.name.as_str(), p.ty.label.as_deref())).collect::<Vec<_>>(), [("id", Some("Int32")), ("names", None), ("age", Some("UInt8")), ("most", Some("UInt8"))]);
        // The statement is sent with the names the server gave, which the run time reads the types by.
        assert_eq!(a.rewritten.sql, "SELECT id FROM users WHERE id = {id:Int32} AND name IN {names:Array(String)} AND age > {age:UInt8} AND id < {id:Int32} AND age < {most:UInt8}");
    }

    #[test]
    fn a_parameter_may_repeat_with_one_type_and_no_other() {
        let a = run("SELECT {n:UInt8} AS a, {n:UInt8} AS b", &server(&[("a", "UInt8"), ("b", "UInt8")], "QUERY id: 0"));
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(a.params.len(), 1);
        let b = run("SELECT {n:UInt8} AS a, {n:String} AS b", &server(&[], "QUERY id: 0"));
        assert_eq!(b.diagnostics.iter().map(|d| (d.message.as_str(), d.code)).collect::<Vec<_>>(), [("The parameter n is declared String here and UInt8 at its first use: declare it with one type.", 3)]);
    }

    #[test]
    fn a_parameter_that_names_or_holds_what_cannot_be_typed_is_refused() {
        let refused = |sql: &str| run(sql, &server(&[], "QUERY id: 0")).diagnostics.into_iter().map(|d| (d.message, sql[d.start..d.start + d.length].to_string())).collect::<Vec<_>>();
        assert_eq!(refused("SELECT {c:Identifier} FROM users")[0].1, "{c:Identifier}");
        assert!(refused("SELECT {c:Identifier} FROM users")[0].0.contains("stands for a name"));
        assert!(refused("SELECT {doc:JSON} AS d")[0].0.starts_with("Reylo binds no parameter of the type JSON"));
        assert!(refused("SELECT {n:UInt46} AS d")[0].0.starts_with("Reylo binds no parameter of the type UInt46"));
        assert!(refused("SELECT {v:Array(Variant(String, UInt8))} AS d")[0].0.contains("is a Variant"));
    }

    /// A server that takes a column nowhere `needs` names, and describes `columns(text)` elsewhere.
    fn constant_only<'a>(needs: &'a str, columns: &'a dyn Fn(&str) -> Result<Vec<Vec<Value>>, DbError>) -> impl Fn(&str) -> Result<Vec<Vec<Value>>, DbError> + 'a {
        move |sent| match sent.strip_prefix(DESCRIBE) {
            Some(text) if text.contains(&format!("{needs}materialize(")) => Err(error(&format!("Code: 44. DB::Exception: The argument must be a constant: In scope {}. (ILLEGAL_COLUMN) (version 26.9.23.15 (official build))", text.trim_end_matches("\n)")), "44")),
            Some(text) => columns(text),
            None => server(&[], "QUERY id: 0\n  LIMIT\n    CONSTANT id: 4, constant_value: UInt64_1, constant_value_type: UInt64")(sent),
        }
    }

    #[test]
    fn a_parameter_the_server_reads_as_a_constant_is_one_where_its_values_leave_the_types_alone() {
        let asked = std::cell::RefCell::new(Vec::new());
        let sql = "SELECT formatDateTime(created, {format:String}) AS shown, quantile({level:Float64})(age) AS q FROM users WHERE id = {id:UInt64}";
        let fixed = |_: &str| Ok(vec![vec![json!("shown"), json!("String")], vec![json!("q"), json!("Float64")]]);
        let by_format = constant_only("formatDateTime(created, ", &fixed);
        let a = run(sql, &|sent| {
            asked.borrow_mut().push(sent.to_string());
            if sent.contains("quantile(materialize(") {
                return Err(error("Code: 36. DB::Exception: Parameters to aggregate functions must be literals. (BAD_ARGUMENTS) (version 26.9.23.15 (official build))", "36"));
            }
            by_format(sent)
        });
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(a.columns.iter().map(|c| c.described.as_deref().unwrap()).collect::<Vec<_>>(), ["String", "Float64"]);
        // The two are written as constants, each varied in its turn, and the third parameter stays a column.
        for value in ["CAST('a', 'String')) AS shown, quantile(CAST(0.5, 'Float64'))", "CAST('b', 'String')) AS shown, quantile(CAST(0.5, 'Float64'))", "CAST('c', 'String')) AS shown, quantile(CAST(0.5, 'Float64'))", "CAST('a', 'String')) AS shown, quantile(CAST(0.25, 'Float64'))", "CAST('a', 'String')) AS shown, quantile(CAST(0.75, 'Float64'))"] {
            assert!(asked.borrow().iter().any(|s| s.contains(value) && s.contains("id = materialize(")), "{value}");
        }
        // Whatever such a parameter decides is the caller's, a limit included.
        assert_eq!(a.cardinality, Cardinality::Many);
    }

    #[test]
    fn a_parameter_the_server_reads_as_a_constant_is_refused_where_its_value_changes_a_type() {
        let sql = "SELECT toDateTime64(created, {scale:UInt8}) AS t FROM users WHERE id = {id:UInt64}";
        let by_scale = |text: &str| {
            let scale = text.split("toDateTime64(created, CAST(").nth(1).and_then(|rest| rest.split(',').next()).unwrap_or("?");
            Ok(vec![vec![json!("t"), json!(format!("DateTime64({scale})"))]])
        };
        let a = run(sql, &constant_only("toDateTime64(created, ", &by_scale));
        assert_eq!(a.diagnostics.len(), 1);
        let d = &a.diagnostics[0];
        assert_eq!(&sql[d.start..d.start + d.length], "{scale:UInt8}");
        assert_eq!(d.message, "ClickHouse reads {scale:UInt8} as a constant here, and the result's types change with its value: column 1 (t) has the type DateTime64(1) with 1 and DateTime64(2) with 2. Write the value in the statement.");
        // A value the statement fails with says nothing of its types, where another describes as the first.
        let sql = "SELECT tupleElement(pair, {at:UInt8}) AS side FROM users";
        let failing_from = |from: u8| {
            move |text: &str| match (from..=3).any(|at| text.contains(&format!("CAST({at}, 'UInt8')"))) {
                true => Err(error("Code: 10. DB::Exception: Tuple doesn't have element with that index. (NOT_FOUND_COLUMN_IN_BLOCK) (version 26.9.23.15 (official build))", "10")),
                false => Ok(vec![vec![json!("side"), json!("UInt8")]]),
            }
        };
        let b = run(sql, &constant_only("tupleElement(pair, ", &failing_from(3)));
        assert!(b.diagnostics.is_empty(), "{:?}", b.diagnostics);
        // Where no other value describes, nothing shows that the types hold.
        let b = run(sql, &constant_only("tupleElement(pair, ", &failing_from(2)));
        assert_eq!(b.diagnostics.len(), 1);
        assert_eq!(b.diagnostics[0].message, "ClickHouse reads {at:UInt8} as a constant here, and the statement fails with each other value tried in its place, as with 2, so nothing shows that the result's types hold for every value. Write the value in the statement. (Tuple doesn't have element with that index. (NOT_FOUND_COLUMN_IN_BLOCK))");
        // A type Reylo has one value of cannot be varied.
        let sql = "SELECT toString(created, {zone:Enum8('UTC' = 1)}) AS shown FROM users";
        let c = run(sql, &constant_only("toString(created, ", &|_| Ok(vec![vec![json!("shown"), json!("String")]])));
        assert_eq!(c.diagnostics.len(), 1);
        assert!(c.diagnostics[0].message.starts_with("ClickHouse reads {zone:Enum8('UTC' = 1)} as a constant here, and Reylo has one value of the type Enum8('UTC' = 1) to describe the statement with"), "{}", c.diagnostics[0].message);
    }

    #[test]
    fn a_parameter_read_as_a_constant_is_found_where_its_first_constant_is_refused_too() {
        let cast = "Code: 43. DB::Exception: Second argument to CAST must be a constant string describing type. Instead there is non-constant column of type String: In scope SELECT CAST(age, materialize(defaultValueOfTypeName('String'))) AS a FROM users. (ILLEGAL_TYPE_OF_ARGUMENT) (version 26.9.23.15 (official build))";
        let unknown_type = "Code: 50. DB::Exception: Unknown data type family: a: In scope SELECT CAST(age, CAST('a', 'String')) AS a FROM users. (UNKNOWN_TYPE) (version 26.9.23.15 (official build))";
        let answer = |sent: &str| {
            if !sent.starts_with(DESCRIBE) {
                server(&[], "QUERY id: 0")(sent)
            } else if sent.contains("CAST(age, materialize(") {
                Err(error(cast, "43"))
            } else {
                Err(error(unknown_type, "50"))
            }
        };
        let sql = "SELECT CAST(age, {type:String}) AS a FROM users WHERE id = {id:UInt64}";
        let a = run(sql, &answer);
        assert_eq!(a.diagnostics.len(), 1);
        let d = &a.diagnostics[0];
        assert_eq!(&sql[d.start..d.start + d.length], "{type:String}");
        assert_eq!(d.message, "ClickHouse reads {type:String} as a constant here, and the statement fails with 'a' in its place, so its value is part of what the statement says. Write the value in the statement. (Unknown data type family: a: In scope SELECT CAST(age, {type:String}) AS a FROM users. (UNKNOWN_TYPE))");
        // A statement that fails the same way whatever its parameters are is wrong by itself.
        let unknown = "Code: 47. DB::Exception: Unknown expression identifier `nope`. In scope SELECT nope FROM users. (UNKNOWN_IDENTIFIER) (version 26.9.23.15 (official build))";
        let b = run("SELECT nope FROM users WHERE id = {id:UInt64}", &|sent| if sent.starts_with(DESCRIBE) { Err(error(unknown, "47")) } else { server(&[], "QUERY id: 0")(sent) });
        assert_eq!((b.diagnostics[0].start, b.diagnostics[0].code), (0, 6));
        assert!(b.diagnostics[0].message.starts_with("Unknown expression identifier `nope`"), "{}", b.diagnostics[0].message);
    }

    #[test]
    fn the_server_s_error_is_the_statement_s_at_the_place_it_names() {
        let sql = "SELECT id FROM users WHERE";
        let a = run(sql, &|sent| {
            if sent.starts_with(DESCRIBE) {
                Err(error("Code: 62. DB::Exception: Syntax error: failed at position 39 ()) (line 3, col 1): ). Expected one of: expression. (SYNTAX_ERROR) (version 26.9.23.15 (official build))", "62"))
            } else {
                server(&[], "QUERY id: 0")(sent)
            }
        });
        let d = &a.diagnostics[0];
        assert_eq!((d.start, d.length, d.code, d.server_code.as_deref()), (sql.len() - 1, 1, 6, Some("62")));
        // The place is the diagnostic's own: the message leaves out the server's, counted in the text it was sent.
        assert_eq!(d.message, "Syntax error. Expected one of: expression. (SYNTAX_ERROR)");
        let sql = "SELECT id FROM users FORMAT JSON\n";
        let a = run(sql, &|sent| {
            if sent.starts_with(DESCRIBE) {
                Err(error("Code: 62. DB::Exception: Syntax error: failed at position 40 (JSON) (line 2, col 29): JSON\n\n). Expected one of: token sequence, Dot. (SYNTAX_ERROR) (version 26.9.23.15 (official build))", "62"))
            } else {
                server(&[], "QUERY id: 0")(sent)
            }
        });
        let d = &a.diagnostics[0];
        assert_eq!((&sql[d.start..d.start + d.length], d.message.as_str()), ("JSON", "Syntax error. Expected one of: token sequence, Dot. (SYNTAX_ERROR)"));
        assert_eq!(syntax_parts("Syntax error: failed at position 8 ()): ). Expected one of: expression"), Some(("Syntax error", ")", "). Expected one of: expression")));
        let unknown = run("SELECT nope FROM users", &|sent| if sent.starts_with(DESCRIBE) { Err(error("Code: 47. DB::Exception: Unknown expression identifier `nope`. In scope SELECT nope FROM users. (UNKNOWN_IDENTIFIER) (version 26.9.23.15 (official build))", "47")) } else { server(&[], "QUERY id: 0")(sent) });
        assert_eq!(unknown.diagnostics[0].message, "Unknown expression identifier `nope`. In scope SELECT nope FROM users. (UNKNOWN_IDENTIFIER)");
        assert_eq!(unknown.diagnostics[0].start, 0);
    }

    #[test]
    fn the_rows_are_what_the_query_tree_proves() {
        let aggregate = "QUERY id: 0\n  PROJECTION\n    LIST id: 1, nodes: 1\n      FUNCTION id: 2, function_name: count, function_type: aggregate, result_type: UInt64\n  JOIN TREE\n    TABLE id: 3, table_name: app.users";
        let one = run("SELECT count() AS n FROM users", &server(&[("n", "UInt64")], aggregate));
        assert_eq!(one.cardinality, Cardinality::One);
        let limited = format!("{}\n  LIMIT\n    CONSTANT id: 4, constant_value: UInt64_1, constant_value_type: UInt64", aggregate.replace("function_type: aggregate", "function_type: ordinary"));
        assert_eq!(run("SELECT id AS n FROM users LIMIT 1", &server(&[("n", "UInt64")], &limited)).cardinality, Cardinality::AtMostOne);
        assert_eq!(run("SELECT id AS n FROM users LIMIT {n:UInt64}", &server(&[("n", "UInt64")], &limited)).cardinality, Cardinality::Many);
        // A session that drops an aggregate's row over an empty input leaves at most one.
        let dropped = run("SELECT count() AS n FROM users", &|sent| if sent == ROW_SETTINGS { Ok(vec![vec![json!("empty_result_for_aggregation_by_empty_set"), json!("1")], vec![json!("limit"), json!("0")], vec![json!("offset"), json!("0")]]) } else { server(&[("n", "UInt64")], aggregate)(sent) });
        assert_eq!(dropped.cardinality, Cardinality::AtMostOne);
    }

    #[test]
    fn a_column_s_name_is_an_alias_or_a_column_s_own() {
        let value = run("SELECT count() FROM users", &server(&[("count()", "UInt64")], "QUERY id: 0"));
        assert!(value.value && value.diagnostics.is_empty());
        let sql = "SELECT id, age + 1 FROM users";
        let unnamed = run(sql, &server(&[("id", "UInt64"), ("plus(age, 1)", "UInt16")], "QUERY id: 0"));
        assert_eq!(unnamed.diagnostics.len(), 1);
        assert_eq!((unnamed.diagnostics[0].code, &sql[unnamed.diagnostics[0].start..unnamed.diagnostics[0].start + unnamed.diagnostics[0].length]), (analysis::UNNAMED_COLUMN, "age + 1"));
        // A name that reads as no identifier is still the statement's where its text says it: a
        // member read by its path, a quoted column or alias, and whatever a `*` stands for.
        let named = |sql: &str, columns: &[(&str, &str)]| run(sql, &server(columns, "QUERY id: 0")).diagnostics;
        assert!(named("SELECT items.sku, o.`odd name`, total * 2 AS `twice the total`, \"q\" FROM orders o", &[("items.sku", "Array(String)"), ("odd name", "UInt8"), ("twice the total", "UInt64"), ("q", "UInt8")]).is_empty());
        assert!(named("SELECT * FROM orders", &[("items.sku", "Array(String)"), ("odd name", "UInt8")]).is_empty());
        assert!(named("SELECT id, o.* EXCEPT (id), COLUMNS('^t'), total AS t FROM orders o", &[("id", "UInt64"), ("items.sku", "Array(String)"), ("total", "UInt64"), ("t", "UInt64")]).is_empty());
        assert_eq!(named("SELECT *, total * 2, id FROM orders", &[("odd name", "UInt8"), ("multiply(total, 2)", "UInt64"), ("id", "UInt64")]).len(), 1);
        assert!(is_star("*") && is_star("t.*") && is_star("* EXCEPT (a)") && is_star("COLUMNS('a')") && !is_star("a * b") && !is_star("count(*)") && !is_star("t.*x"));
        assert!(names_itself("a.b.c") && names_itself("`a b`") && names_itself("f(x) AS `a b`") && names_itself("x as y") && names_itself("(1)AS y"));
        assert!(!names_itself("a + b") && !names_itself("f(x)") && !names_itself("pair.1") && !names_itself("CAST(x AS String)") && !names_itself("x IS NULL"));
    }

    #[test]
    fn a_type_the_run_time_does_not_read_is_refused_with_what_to_write() {
        let a = run("SELECT doc FROM events", &server(&[("doc", "JSON")], "QUERY id: 0"));
        assert_eq!(a.diagnostics[0].message, "Column 1 (doc) has the type JSON, and Reylo does not read a ClickHouse JSON yet: select it as a type it reads, as toString(doc) or toJSONString(doc) does.");
    }

    #[test]
    fn a_write_returns_no_rows_and_is_checked_as_far_as_the_server_checks_one() {
        let asked = std::cell::RefCell::new(Vec::new());
        let ok = |sent: &str| {
            asked.borrow_mut().push(sent.to_string());
            Ok(Vec::new())
        };
        let insert = run("INSERT INTO orders (id, note) VALUES ({id:UInt64}, {note:Nullable(String)})", &ok);
        assert!(insert.diagnostics.is_empty(), "{:?}", insert.diagnostics);
        assert_eq!((insert.kind, insert.cardinality, insert.columns.len()), (Some(StatementKind::Insert), Cardinality::None, 0));
        assert_eq!(insert.params.iter().map(|p| p.nullable).collect::<Vec<_>>(), [false, true]);
        assert!(asked.borrow().iter().any(|s| s.starts_with(PIPELINE)) && asked.borrow().iter().any(|s| s.starts_with(AST)));
        asked.borrow_mut().clear();
        let delete = run("DELETE FROM orders WHERE id = {id:UInt64}", &ok);
        assert_eq!((delete.kind, delete.cardinality), (Some(StatementKind::Delete), Cardinality::None));
        assert!(asked.borrow().iter().any(|s| s == "DESCRIBE (\nSELECT 1 FROM orders WHERE id = materialize(defaultValueOfTypeName('UInt64'))\n)"), "{:?}", asked.borrow());
        let refused = run("DELETE FROM orders WHERE nope = 1", &|sent| if sent.starts_with(DESCRIBE) { Err(error("Code: 47. DB::Exception: Unknown expression identifier `nope`. (UNKNOWN_IDENTIFIER) (version 26.9)", "47")) } else { Ok(Vec::new()) });
        assert_eq!(refused.diagnostics[0].message, "Unknown expression identifier `nope`. (UNKNOWN_IDENTIFIER)");
        let format = run("INSERT INTO orders FORMAT JSONEachRow", &ok);
        assert!(format.diagnostics[0].message.starts_with("An INSERT … FORMAT takes its rows"));
        let ddl = run("CREATE TABLE t (id UInt64) ENGINE = MergeTree ORDER BY id", &ok);
        assert!(ddl.diagnostics[0].message.starts_with("Reylo types a ClickHouse SELECT, INSERT, DELETE or UPDATE, and CREATE is none of them"), "{}", ddl.diagnostics[0].message);
    }
}
