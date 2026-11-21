//! The ClickHouse analysis.
//!
//! The server describes the statement: `DESCRIBE (…)` answers with each result column's name and
//! type and runs nothing. A column's type is the whole of what the engine says of it, nullability
//! and nested shape included ([`crate::types`]), and a parameter's is the one its placeholder
//! declares, `{name:Type}`, or the cast after it, `:name::Type`.
//!
//! What the describe leaves open is that ClickHouse writes a parameter's value into the statement
//! as a constant before it reads the statement. A constant can decide a type: the scale of
//! `toDateTime64(x, {p:UInt8})`, the type `CAST(x, {t:String})` names, the element
//! `tupleElement(t, {i:UInt8})` reads. A constant is typed otherwise than a column, too: `x IN
//! (1, {n:UInt8})` is a UInt8 of constants and a Nullable(UInt8) of a column, and a function of a
//! `LowCardinality` column and a constant is `LowCardinality` where one of two columns is not. And
//! a constant is computed with as the statement is read, so no one value stands for a parameter:
//! `toDate('a')` fails where `toDate({day:String})` is a fine statement. So:
//!
//! - The statement's types are the ones it has as it runs, every parameter a constant. Each type
//!   has a list of values that stand for a parameter of it ([`constants`]), a string's among them
//!   the texts a statement commonly computes with: a number, a date, a time, a zone, a unit. The
//!   statement is described with each parameter's first value, then its second, and where both
//!   fail, with each later one, then with one parameter changed at a time, until it describes.
//!   Two describes that answer alike give the statement's types; two that differ show a value
//!   deciding a type, and the parameter is refused.
//! - The statement is described with each parameter a column too,
//!   `materialize(defaultValueOfTypeName('T'))`, which has the parameter's type and no value to
//!   read. ClickHouse refuses it wherever it needs a constant, which finds the parameters it reads
//!   as constants. A `LIMIT`, an `OFFSET` and the right side of an `IN` take a constant and
//!   decide no type, so a parameter there is written as one.
//! - A parameter the server reads as a constant is tried with every value of its list: the same
//!   types with another of them, and they are the statement's (`formatDateTime(t,
//!   {format:String})`, `quantile({level:Float64})(x)`, `dateDiff({unit:String}, a, b)`); other
//!   types with one, and it is refused where it stands. A value the statement fails with says
//!   nothing of its types, since the statement fails with it as it runs too.
//! - A column the statement gives no name is named by its expression's text, which holds each
//!   constant in it, a parameter's value too. One such column alone is the statement's value, and
//!   goes by no name; among others it is refused, as a name that changes from run to run.
//!
//! No list of values shows that a type holds for every value, so the run time holds the types each
//! result comes back with against the ones read here, and refuses a result that differs
//! (src/decode-clickhouse.ts). A parameter that names a column or a table, `{c:Identifier}`,
//! changes what the statement reads, and is refused.
//!
//! How many rows the statement returns is read from the server's query tree ([`crate::tree`]). A
//! write (`INSERT`, `DELETE`, `UPDATE`, `ALTER TABLE … UPDATE` or `DELETE`) returns no rows, and
//! ClickHouse describes none: an `INSERT` is checked against its table by `EXPLAIN PIPELINE`,
//! which builds the insert and writes nothing, and a `DELETE` or an `UPDATE` as the `SELECT` of
//! the rows it would change and the values it would write. An `ALTER` that does anything else
//! (`ADD COLUMN`, `FREEZE`) changes no rows, and is refused.

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

/// How each placeholder of a statement is written into a text the server is asked about, by the
/// placeholder's index: its constant of a number, or `None`, a column. One parameter is written
/// one way at each of its places.
type Choice = Vec<Option<usize>>;

/// What a describe answers with: a row a column, its name and its type's.
type Rows = Vec<Vec<Value>>;
type Described = Result<Rows, DbError>;

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

/// The texts a statement commonly computes with as it reads a string: plain letters first, then
/// a number, a date, a time, a zone, a unit, a UUID, an address and a document, two of each, so
/// that a statement that takes one of a kind describes with another of it. The units are of both
/// kinds, those of a time and those of a date: `dateTrunc` to a month is a date, and to a day a time.
const STRINGS: [&str; 22] = [
    "a", "b", "c", "1", "2", "2001-02-03", "2004-05-06", "2001-02-03 04:05:06", "2004-05-06 07:08:09", "UTC", "Asia/Tokyo", "day", "hour", "month", "61f0c404-5cb3-11e7-907b-a6006ad3dba0", "00112233-4455-6677-8899-aabbccddeeff",
    "127.0.0.1", "10.1.2.3", "::1", "2001:db8::1", "{}", "{\"a\": 1}",
];
/// The whole numbers a parameter is tried with: the first few, where a place in a tuple or a
/// number of digits is read, then some further off.
const WHOLE: [&str; 15] = ["1", "2", "3", "0", "4", "5", "6", "7", "8", "9", "10", "16", "32", "64", "100"];
/// Within 0 and 1 first, which a level and a ratio take alone.
const FRACTIONS: [&str; 9] = ["0.5", "0.25", "0.75", "1", "0", "2", "10", "100", "-1"];

/// The constants a parameter of `ty` is written as where the server reads one: each as the
/// expression that stands in its place, and as a message shows it. The first two differ for every
/// type that has two values, so that a type or a name that changes with the value shows; a type
/// whose values are not written here has its default alone, which no describe can vary.
fn constants(ty: &Named) -> Vec<(String, String)> {
    let name = types::quoted(&ty.to_string());
    let cast = |value: &str| format!("CAST({value}, {name})");
    let of = |values: &[&str]| values.iter().map(|v| (cast(v), v.to_string())).collect::<Vec<_>>();
    let texts = |values: &[&str]| values.iter().map(|v| (cast(&types::quoted(v)), types::quoted(v))).collect::<Vec<_>>();
    let mut out = match (ty.name.as_str(), ty.args.as_slice()) {
        // A value, NULL, and its other values.
        ("Nullable", [Arg::Type(inner)]) => {
            let inner = constants(inner);
            let mut out = vec![(cast(&inner[0].0), inner[0].1.clone()), (cast("NULL"), "NULL".to_string())];
            out.extend(inner[1..].iter().map(|(sql, shown)| (cast(sql), shown.clone())));
            out
        }
        ("LowCardinality", [Arg::Type(inner)]) => constants(inner).into_iter().map(|(sql, shown)| (cast(&sql), shown)).collect(),
        // One element, none, two, the two the other way, three, and those the other way: two of
        // each length a statement may ask for.
        ("Array", [Arg::Type(inner)]) => {
            let inner = constants(inner);
            let at = |i: usize| &inner[i.min(inner.len() - 1)];
            let listed = |indexes: &[usize]| {
                let sql: Vec<&str> = indexes.iter().map(|&i| at(i).0.as_str()).collect();
                let shown: Vec<&str> = indexes.iter().map(|&i| at(i).1.as_str()).collect();
                (cast(&format!("[{}]", sql.join(", "))), format!("[{}]", shown.join(", ")))
            };
            vec![listed(&[0]), listed(&[]), listed(&[1, 0]), listed(&[0, 1]), listed(&[0, 1, 2]), listed(&[2, 1, 0])]
        }
        // Each element's first value, then each one's second, and third.
        ("Tuple", args) if !args.is_empty() && args.iter().all(|a| matches!(a, Arg::Type(_) | Arg::Member(..))) => {
            let elements: Vec<Vec<(String, String)>> = args.iter().map(|a| match a {
                Arg::Type(t) | Arg::Member(_, t) => constants(t),
                _ => Vec::new(),
            }).collect();
            (0..3)
                .map(|j| {
                    let at = |element: &Vec<(String, String)>| element[j.min(element.len() - 1)].clone();
                    let (sql, shown): (Vec<String>, Vec<String>) = elements.iter().map(at).unzip();
                    (cast(&format!("tuple({})", sql.join(", "))), format!("({})", shown.join(", ")))
                })
                .collect()
        }
        // One pair, none, and another pair.
        ("Map", [Arg::Type(key), Arg::Type(value)]) => {
            let (keys, values) = (constants(key), constants(value));
            let pair = |j: usize| (&keys[j.min(keys.len() - 1)], &values[j.min(values.len() - 1)]);
            let one = |j: usize| (cast(&format!("map({}, {})", pair(j).0 .0, pair(j).1 .0)), format!("{{{}: {}}}", pair(j).0 .1, pair(j).1 .1));
            vec![one(0), (cast("map()"), "{}".to_string()), one(1)]
        }
        ("Enum8" | "Enum16", labels) => {
            let labels: Vec<&str> = labels.iter().filter_map(|a| if let Arg::Label(label, _) = a { Some(label.as_str()) } else { None }).take(3).collect();
            texts(&labels)
        }
        ("Bool", _) => of(&["true", "false"]),
        ("String" | "FixedString", _) => texts(&STRINGS),
        ("UUID", _) => texts(&["61f0c404-5cb3-11e7-907b-a6006ad3dba0", "00112233-4455-6677-8899-aabbccddeeff", "00000000-0000-0000-0000-000000000000"]),
        ("IPv4", _) => texts(&["127.0.0.1", "10.1.2.3", "0.0.0.0"]),
        ("IPv6", _) => texts(&["::1", "2001:db8::1", "::"]),
        // A date or a time of a number: so many days, or seconds, after 1970.
        ("Date" | "Date32" | "DateTime" | "DateTime64", _) => of(&["1", "2", "3"]),
        ("Float32" | "Float64" | "BFloat16", _) => of(&FRACTIONS),
        // A number below zero for a type that holds one: a place counted from the end.
        (name, _) if ty.is_number() && !name.starts_with("UInt") => of(&[&WHOLE[..], &["-1", "-2"]].concat()),
        _ if ty.is_number() => of(&WHOLE),
        _ => Vec::new(),
    };
    // A value is tried once, and a type none of whose values is written here has its default.
    let mut seen = std::collections::HashSet::new();
    out.retain(|(sql, _)| seen.insert(sql.clone()));
    if out.is_empty() {
        out.push((format!("defaultValueOfTypeName({name})"), "its type's default value".to_string()));
    }
    out
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

    /// Every parameter as its constant numbered `j`: the statement as it runs.
    fn joint(&self, j: usize) -> Choice {
        vec![Some(j); self.types.len()]
    }

    /// `choice`, but each parameter that stands where a value goes as a column.
    fn columns(&self, choice: &Choice) -> Choice {
        choice.iter().enumerate().map(|(k, how)| if self.positions[k] == Position::Value { None } else { *how }).collect()
    }

    /// `choice`, but the parameter at `k`, at each of its places, written as `how`.
    fn with(&self, lexed: &Lexed, choice: &Choice, k: usize, how: Option<usize>) -> Choice {
        choice.iter().enumerate().map(|(i, own)| if lexed.placeholders[i].name == lexed.placeholders[k].name { how } else { *own }).collect()
    }

    /// Whether two choices write the parameter at `k` as two values, or one as a column.
    fn differs(&self, k: usize, a: &Choice, b: &Choice) -> bool {
        match (a[k], b[k]) {
            (Some(x), Some(y)) => self.constant(k, x).0 != self.constant(k, y).0,
            (None, None) => false,
            _ => true,
        }
    }

    /// The value a choice writes the parameter at `k` as, as a message shows it.
    fn shown(&self, k: usize, choice: &Choice) -> &str {
        choice[k].map_or("a column", |j| self.constant(k, j).1.as_str())
    }

    /// The statement `body` with each placeholder written as `choice` says, and the map from that
    /// text back to the author's.
    fn text(&self, body: &str, lexed: &Lexed, choice: &Choice) -> Rewritten {
        let mut out = String::with_capacity(body.len());
        let mut spans = Vec::with_capacity(lexed.placeholders.len());
        let mut last = 0;
        for (k, p) in lexed.placeholders.iter().enumerate() {
            out.push_str(&body[last..p.start]);
            let start = out.len();
            match choice[k] {
                Some(j) => out.push_str(&self.constant(k, j).0),
                None => out.push_str(&self.column(k)),
            }
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
    // A format the statement's own SETTINGS names outranks the one a result is asked for in, the
    // server's answers to the questions below included, so it is read off the text.
    if let Some((start, word)) = words(body, &lexed).into_iter().find(|(_, word)| word == "default_format") {
        return refuse(vec![Diagnostic { message: "The statement's SETTINGS names a format for its result, default_format, and Reylo reads the one it asks for: take the setting out.".into(), start, length: word.len(), code: 10, server_code: None }]);
    }
    let first = parameters.joint(0);
    let asked = parameters.text(body, &lexed, &parameters.columns(&first));
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

    let describe = |parameters: &Parameters, choice: &Choice| host.query(&format!("{DESCRIBE}{}\n)", parameters.text(body, &lexed, choice).sql), Vec::new(), Scope::Statement);
    // One round: the describe with each parameter a column, the query tree, the session's
    // settings, and the statement as it runs, each parameter a constant, with two values of each.
    let described = describe(&parameters, &parameters.columns(&first));
    let tree = host.query(&format!("{TREE}{}", asked.sql), Vec::new(), Scope::Statement);
    let settings = host.query(ROW_SETTINGS, Vec::new(), Scope::Session);
    let run = (!lexed.placeholders.is_empty()).then(|| (describe(&parameters, &first), describe(&parameters, &parameters.joint(1))));
    let (described, tree, settings) = (described?, tree?, settings?);
    let (rows, tree, varying) = match run {
        // No parameter: the describe is the statement's.
        None => match described {
            Ok(rows) => {
                let varying = vec![false; rows.len()];
                (rows, tree, varying)
            }
            Err(e) => return refuse(vec![server_error(sql, &lexed, &parameters, &asked, DESCRIBE, &e)]),
        },
        Some((one, two)) => {
            let (one, two) = (one?, two?);
            let ran = match as_it_runs(sql, &lexed, &parameters, body, &asked, &described, one, two, host)? {
                Ok(ran) => ran,
                Err(diagnostics) => return refuse(diagnostics),
            };
            // A parameter the server reads as a constant is written as one from here on, and each
            // with the value the statement describes with: the tree is read of that text.
            for &k in &ran.constants {
                for i in 0..lexed.placeholders.len() {
                    if lexed.placeholders[i].name == lexed.placeholders[k].name {
                        parameters.positions[i] = Position::Constant;
                    }
                }
            }
            let text = parameters.text(body, &lexed, &parameters.columns(&ran.base));
            let tree = if text.sql == asked.sql { tree } else { host.query(&format!("{TREE}{}", text.sql), Vec::new(), Scope::Statement)? };
            (ran.rows, tree, ran.varying)
        }
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
    // What a result in rows cannot carry, or a setting of the statement's own would write otherwise.
    if let Some(message) = crate::tree::unread(&lines) {
        return refuse(vec![whole(sql, message, 10, None)]);
    }
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
    // A name ClickHouse makes of an expression holds each constant in it, a parameter's value too,
    // so it changes from run to run: the one column of a statement that returns a value goes by
    // no name, and any other needs one of its own.
    if !typed.value && typed.diagnostics.is_empty() {
        let unnamed: Vec<Diagnostic> = typed
            .columns
            .iter()
            .enumerate()
            .filter(|(i, _)| varying.get(*i) == Some(&true))
            .map(|(i, column)| {
                let name = parameters.authored(sql, &lexed, &column.name);
                whole(sql, format!("Column {} is named \"{name}\" by its expression, which holds a parameter's value, so its name changes from run to run. Give it one with `as` where the statement makes it.", i + 1), analysis::UNNAMED_COLUMN, None)
            })
            .collect();
        typed.diagnostics.extend(unnamed);
    }
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

/// The statement as it runs, every parameter a constant.
struct Ran {
    /// What its describe answers with the values of `base`.
    rows: Rows,
    /// The values the statement describes with: each parameter's constant.
    base: Choice,
    /// The parameters the server reads as constants, each by the index of its first placeholder.
    constants: Vec<usize>,
    /// Which of the columns are named by text that changes with a parameter's value.
    varying: Vec<bool>,
}

/// The statement as it runs, or the diagnostics of one that has no one type so. `column` is its
/// describe with each parameter a column, and `first` and `second` its describes with each
/// parameter's first constant and its second.
#[allow(clippy::too_many_arguments)]
fn as_it_runs(sql: &str, lexed: &Lexed, parameters: &Parameters, body: &str, asked: &Rewritten, column: &Described, first: Described, second: Described, host: &Host) -> Lazy<Result<Ran, Vec<Diagnostic>>> {
    let describe = |choice: &Choice| host.query(&format!("{DESCRIBE}{}\n)", parameters.text(body, lexed, choice).sql), Vec::new(), Scope::Statement);
    let ones = once(lexed, 0..lexed.placeholders.len());
    let most = parameters.constants.iter().map(Vec::len).max().unwrap_or(1);
    // The same error with a column and with a constant, each stand-in read back as its parameter,
    // is the statement's own, whatever its parameters are.
    if let (Err(as_columns), Err(as_constants)) = (column, &first) {
        if as_columns.code == as_constants.code && parameters.authored(sql, lexed, &message_of(as_columns)) == parameters.authored(sql, lexed, &message_of(as_constants)) {
            return Some(Err(vec![server_error(sql, lexed, parameters, asked, DESCRIBE, as_columns)]));
        }
    }
    // Every describe of the statement as it runs, each with the values it was asked with.
    let mut tried: Vec<(Choice, Described)> = vec![(parameters.joint(0), first), (parameters.joint(1), second)];
    // Where it fails with the first two values of each parameter, a value was computed with: each
    // later value of every parameter at once, which finds what one kind of text describes.
    if tried.iter().all(|(_, answer)| answer.is_err()) {
        let later: Vec<_> = (2..most).map(|j| parameters.joint(j)).map(|choice| (describe(&choice), choice)).collect();
        for (answer, choice) in later {
            tried.push((choice, answer?));
        }
    }
    // Then one parameter changed at a time, keeping a change the statement fails otherwise with:
    // a zone here and a date there.
    if tried.iter().all(|(_, answer)| answer.is_err()) {
        let mut current = parameters.joint(0);
        let mut code = tried[0].1.as_ref().err().and_then(|e| e.code.clone());
        for _ in 0..ones.len() {
            let changes: Vec<Choice> = ones.iter().flat_map(|&k| (0..parameters.constants[k].len()).map(move |j| (k, j))).map(|(k, j)| parameters.with(lexed, &current, k, Some(j))).filter(|choice| *choice != current).collect();
            let asked: Vec<_> = changes.into_iter().map(|choice| (describe(&choice), choice)).collect();
            let mut answers = Vec::with_capacity(asked.len());
            for (answer, choice) in asked {
                answers.push((choice, answer?));
            }
            let described = answers.iter().any(|(_, answer)| answer.is_ok());
            let moved = answers.iter().find_map(|(choice, answer)| answer.as_ref().err().filter(|e| e.code != code).map(|e| (choice.clone(), e.code.clone())));
            tried.extend(answers);
            match moved {
                Some((choice, other)) if !described => {
                    current = choice;
                    code = other;
                }
                _ => break,
            }
        }
    }
    let Some((base, rows)) = tried.iter().find_map(|(choice, answer)| answer.as_ref().ok().map(|rows| (choice.clone(), rows.clone()))) else {
        let Err(error) = &tried[0].1 else { unreachable!("no describe answered") };
        return unvalued(sql, lexed, parameters, body, column, error, host).map(Err);
    };
    let types = types_of(&rows);

    // The parameters the server reads as constants: where the statement is refused with its
    // parameters columns, each one it is refused with as a column alone, or all of them where no
    // one is at fault alone.
    let values: Vec<usize> = ones.iter().copied().filter(|&k| parameters.positions[k] == Position::Value).collect();
    let constants = if column.is_ok() || values.is_empty() {
        Vec::new()
    } else {
        let together = describe(&parameters.columns(&base));
        let alone: Vec<_> = values.iter().map(|&k| describe(&parameters.with(lexed, &base, k, None))).collect();
        let together = together?;
        let mut found = Vec::new();
        for (&k, answer) in values.iter().zip(alone) {
            if answer?.is_err() {
                found.push(k);
            }
        }
        match together {
            Ok(_) => Vec::new(),
            Err(_) if found.is_empty() => values,
            Err(_) => found,
        }
    };
    // Each of those is tried with every value it has, alone, and with every parameter's value of
    // each number at once, since two of them may only describe together: two arrays of one length.
    if !constants.is_empty() {
        let mut more: Vec<Choice> = Vec::new();
        let singles = constants.iter().flat_map(|&k| (0..parameters.constants[k].len()).map(move |j| (k, j))).map(|(k, j)| parameters.with(lexed, &base, k, Some(j)));
        for choice in (0..most).map(|j| parameters.joint(j)).chain(singles) {
            if !more.contains(&choice) && !tried.iter().any(|(other, _)| *other == choice) {
                more.push(choice);
            }
        }
        let asked: Vec<_> = more.into_iter().map(|choice| (describe(&choice), choice)).collect();
        for (answer, choice) in asked {
            tried.push((choice, answer?));
        }
    }

    // A describe with other types shows a value deciding one: the parameter's, where one alone changed.
    let changed = |choice: &Choice| ones.iter().copied().filter(|&k| parameters.differs(k, choice, &base)).collect::<Vec<usize>>();
    let decides = |k: usize, choice: &Choice, other: &Rows| {
        let p = &lexed.placeholders[k];
        let why = difference((&rows, parameters.shown(k, &base)), (other, parameters.shown(k, choice)));
        if constants.contains(&k) {
            refused(sql, lexed, parameters, k, &format!("the result's types change with its value: {why}"), None)
        } else {
            at(p, format!("The result's types change with the value of {}: {why}. Write the value in the statement.", &sql[p.start..p.end]), 10)
        }
    };
    let differing: Vec<(&Choice, &Rows)> = tried.iter().filter_map(|(choice, answer)| answer.as_ref().ok().filter(|other| types_of(other) != types).map(|other| (choice, other))).collect();
    if let Some(&(choice, other)) = differing.first() {
        let mut out: Vec<Diagnostic> = Vec::new();
        let mut blamed: Vec<usize> = Vec::new();
        for &(choice, other) in &differing {
            if let [k] = changed(choice)[..] {
                if !blamed.contains(&k) {
                    blamed.push(k);
                    out.push(decides(k, choice, other));
                }
            }
        }
        if out.is_empty() {
            // Several changed at once: each alone, with the value it had there.
            let alone: Vec<_> = changed(choice).into_iter().map(|k| parameters.with(lexed, &base, k, choice[k])).map(|single| (describe(&single), single)).collect();
            for (answer, single) in alone {
                if let (Ok(with_other), [k]) = (answer?, &changed(&single)[..]) {
                    if types_of(&with_other) != types {
                        out.push(decides(*k, &single, &with_other));
                    }
                }
            }
        }
        if out.is_empty() {
            out.push(whole(sql, format!("The result's types change with the values of the statement's parameters: {}. Write the values in the statement.", difference((&rows, "one value of each"), (other, "another"))), 10, None));
        }
        return Some(Err(out));
    }
    // And a parameter the server reads as a constant holds its types only where another value of
    // it describes with them. A value the statement fails with says nothing of its types.
    let mut unshown = Vec::new();
    for &k in &constants {
        if tried.iter().any(|(choice, answer)| answer.is_ok() && parameters.differs(k, choice, &base)) {
            continue;
        }
        if parameters.constants[k].len() < 2 {
            unshown.push(refused(sql, lexed, parameters, k, &format!("Reylo has one value of the type {} to describe the statement with, which cannot show that the result's types hold for every value", parameters.types[k]), None));
            continue;
        }
        let failed = tried.iter().find_map(|(choice, answer)| answer.as_ref().err().filter(|_| changed(choice) == [k]).map(|e| (parameters.shown(k, choice), e)));
        let (shown, error) = match failed {
            Some((shown, e)) => (shown, Some(e)),
            None => (parameters.constant(k, 1).1.as_str(), None),
        };
        unshown.push(refused(sql, lexed, parameters, k, &format!("the statement fails with each other value tried in its place, as with {shown}, so nothing shows that the result's types hold for every value"), error));
    }
    if !unshown.is_empty() {
        return Some(Err(unshown));
    }

    // A column's name that another value changes, or that holds the text standing for a parameter.
    let name = |rows: &Rows, i: usize| rows.get(i).and_then(|row| row.first()).and_then(as_str).unwrap_or_default().to_string();
    let varying = (0..rows.len())
        .map(|i| {
            let own = name(&rows, i);
            tried.iter().any(|(_, answer)| matches!(answer, Ok(other) if other.len() == rows.len() && name(other, i) != own)) || ones.iter().any(|&k| base[k].is_some_and(|j| own.contains(parameters.constant(k, j).0.as_str())))
        })
        .collect();
    Some(Ok(Ran { rows, base, constants, varying }))
}

/// Why the statement describes with no value tried for its parameters: the diagnostics of each
/// parameter at fault, found by writing it alone as its first constant among columns. The server
/// computes with the one the statement then fails with, or fails otherwise with than as columns;
/// where none is at fault alone, the error is the statement's, as it fails with each first value.
fn unvalued(sql: &str, lexed: &Lexed, parameters: &Parameters, body: &str, column: &Described, error: &DbError, host: &Host) -> Lazy<Vec<Diagnostic>> {
    let ones = once(lexed, 0..lexed.placeholders.len());
    let first = parameters.joint(0);
    let columns = parameters.columns(&first);
    let alone: Vec<_> = ones.iter().map(|&k| host.query(&format!("{DESCRIBE}{}\n)", parameters.text(body, lexed, &parameters.with(lexed, &columns, k, Some(0))).sql), Vec::new(), Scope::Statement)).collect();
    let mut out = Vec::new();
    for (&k, answer) in ones.iter().zip(alone) {
        let p = &lexed.placeholders[k];
        let shown = &parameters.constant(k, 0).1;
        match (column, answer?) {
            // A column there, and a value the statement converts as it is read: a text that is no date.
            (Ok(_), Err(e)) => {
                let quoted = parameters.authored(sql, lexed, &message_of(&e));
                out.push(at(p, format!("ClickHouse computes with {} as it reads the statement, and the statement fails with each value tried in its place, as with {shown}. Declare the parameter as the type the statement converts it to, or write the value in the statement. ({})", &sql[p.start..p.end], quoted.trim_end_matches('.')), 10));
            }
            // A constant there alone, and one that names what the statement reads: a type's name.
            (Err(as_columns), Err(e)) if e.code != as_columns.code => out.push(refused(sql, lexed, parameters, k, &format!("the statement fails with each value tried in its place, as with {shown}, so its value is part of what the statement says"), Some(&e))),
            _ => {}
        }
    }
    if out.is_empty() {
        out.push(server_error(sql, lexed, parameters, &parameters.text(body, lexed, &first), DESCRIBE, error));
    }
    Some(out)
}

/// The analysis of a write, which returns no rows. Any write is parsed. An INSERT is built, and
/// not run, by `EXPLAIN PIPELINE`, which checks its table and columns and the SELECT it reads; a
/// DELETE or an UPDATE is described as the SELECT it reads as ([`as_select`]), which checks its
/// table, the rows it names and the values it writes. A write returns no types for a parameter's
/// value to change, so it stands where the server takes it with its parameters columns, or
/// constants of any of the values tried: a constant is computed with as the statement is read.
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
    let ask = |text: String| host.query(&text, Vec::new(), Scope::Statement);
    let most = parameters.constants.iter().map(Vec::len).max().unwrap_or(1);
    let first = parameters.joint(0);
    let columns = parameters.columns(&first);
    let select = if insert { None } else { as_select(body, lexed) };
    // What the server is asked of the statement with its parameters written one way.
    let question = |choice: &Choice| match &select {
        _ if insert => Some(format!("{PIPELINE}{}", parameters.text(body, lexed, choice).sql)),
        Some(edits) => Some(format!("{DESCRIBE}{}\n)", edited(&parameters.text(body, lexed, choice), edits))),
        None => None,
    };
    let parsed = ask(format!("{AST}{}", asked.sql));
    // The columns an INSERT … VALUES writes its parameters to, as the table has them.
    let values = if insert { inserted(body, lexed) } else { None };
    let table = values.as_ref().map(|(table, ..)| ask(format!("DESCRIBE TABLE {table}")));
    // Where the text is no SELECT, as with ON CLUSTER or IN PARTITION, nothing more is asked of it.
    let reads = select.as_ref().map(|edits| ask(format!("{AST}{}", edited(&parameters.text(body, lexed, &columns), edits))));
    let checks: Vec<_> = [&columns, &first].into_iter().filter_map(question).map(ask).collect();
    let parsed = match parsed? {
        Ok(rows) => rows,
        Err(e) => return refuse(vec![server_error(sql, lexed, parameters, asked, AST, &e)]),
    };
    // An ALTER's commands, as the server's parse names them: one that changes no rows changes the
    // table, or a part of it on disk, which is no statement of an application's data.
    if typed.kind == Some(StatementKind::Other) {
        let lines: Vec<&str> = parsed.iter().filter_map(|row| row.first().and_then(as_str)).flat_map(str::lines).collect();
        let commands: Vec<&str> = lines.iter().filter_map(|line| line.trim_start().strip_prefix("AlterCommand ")).map(|rest| rest.split_whitespace().next().unwrap_or_default()).collect();
        let other = commands.iter().find(|command| !matches!(**command, "UPDATE" | "DELETE"));
        if !lines.first().is_some_and(|line| line.starts_with("AlterQuery")) || commands.is_empty() || other.is_some() {
            let what = other.map_or_else(|| "this one is none".to_string(), |command| format!("{} is neither", command.replace('_', " ")));
            return refuse(vec![whole(sql, lex::refused_command(&format!("Reylo types an ALTER TABLE that changes rows, with UPDATE or DELETE, and {what}")), 10, None)]);
        }
    }
    let reads = match reads {
        Some(answer) => answer?.is_ok(),
        None => true,
    };
    let mut answers = Vec::with_capacity(checks.len());
    for check in checks {
        answers.push(check?);
    }
    if reads && !answers.is_empty() && answers.iter().all(Result::is_err) {
        // A value may have been computed with: each later value of every parameter at once.
        let later: Vec<_> = (1..most).filter_map(|j| question(&parameters.joint(j))).map(ask).collect();
        let mut taken = false;
        for answer in later {
            taken |= answer?.is_ok();
        }
        if let (false, Some(Err(e))) = (taken, answers.first()) {
            return refuse(vec![if insert { server_error(sql, lexed, parameters, asked, PIPELINE, e) } else { whole(sql, parameters.authored(sql, lexed, &message_of(e)), 6, e.code.clone()) }]);
        }
    }
    // A parameter written to a column as it is: ClickHouse converts what a statement inserts to
    // the column's type, and where the column cannot hold the value it changes it and says
    // nothing, so the parameter's own type has to be one whose every value the column holds.
    if let (Some((_, listed, written)), Some(answer)) = (&values, table) {
        if let Ok(rows) = answer? {
            let text = |row: &Vec<Value>, i: usize| row.get(i).and_then(as_str).unwrap_or_default().to_string();
            let columns: Vec<(String, String)> = match listed {
                Some(names) => names.iter().filter_map(|name| rows.iter().find(|row| text(row, 0) == *name).map(|row| (text(row, 0), text(row, 1)))).collect(),
                // With no list, every column but the ones the table computes itself, in its order.
                None => rows.iter().filter(|row| !matches!(text(row, 2).as_str(), "MATERIALIZED" | "ALIAS" | "EPHEMERAL")).map(|row| (text(row, 0), text(row, 1))).collect(),
            };
            let narrowed: Vec<Diagnostic> = written
                .iter()
                .filter(|_| listed.as_ref().is_none_or(|names| names.len() == columns.len()))
                .filter_map(|&(place, k)| {
                    let (name, described) = columns.get(place)?;
                    let column = types::parse(described)?;
                    let into = match (column.name.as_str(), column.args.as_slice()) {
                        ("LowCardinality", [Arg::Type(inner)]) => inner.clone(),
                        _ => column.clone(),
                    };
                    let p = &lexed.placeholders[k];
                    types::narrows(&parameters.types[k], &column).then(|| at(p, format!("{} is written to the column {name}, a {column}, and ClickHouse changes a value the column cannot hold into one it can, without an error. Declare the parameter as the column's type: {{{}:{into}}}.", &sql[p.start..p.end], p.name), 10))
                })
                .collect();
            if !narrowed.is_empty() {
                return refuse(narrowed);
            }
        }
    }
    typed.cardinality = Cardinality::None;
    Some(typed)
}

/// What an `INSERT … VALUES` writes: the table as the statement names it, the columns it lists,
/// where it lists them, and each value of a row that is a parameter alone, by its place in the row
/// and the parameter's index. `None` where the statement is another INSERT (`… SELECT`, a table
/// function's) or lists what is no column's name.
fn inserted(body: &str, lexed: &Lexed) -> Option<(String, Option<Vec<String>>, Vec<(usize, usize)>)> {
    let code = code(body, lexed);
    let b = body.as_bytes();
    let top: Vec<(usize, String)> = words(body, lexed).into_iter().filter(|(start, _)| code.iter().any(|&(at, _, depth)| at == *start && depth == 0)).collect();
    let word = |i: usize| top.get(i).map(|(_, word)| word.as_str());
    if word(0)? != "insert" || word(1)? != "into" {
        return None;
    }
    let named = if word(2) == Some("table") { 2 } else { 1 };
    let values = top.iter().position(|(_, word)| word == "values")?;
    if top[..values].iter().any(|(_, word)| matches!(word.as_str(), "function" | "select" | "with" | "format")) {
        return None;
    }
    // The table: names joined by dots, each plain or quoted.
    let start = code_after(body, lexed, top[named].0 + top[named].1.len());
    let mut end = start;
    loop {
        end += name_length(&body[end..])?;
        match b.get(end) {
            Some(b'.') => end += 1,
            _ => break,
        }
    }
    let table = body[start..end].to_string();
    // A stretch of the statement without its comments, which stand for nothing.
    let plain = |from: usize, to: usize| {
        let mut out = String::new();
        let mut at = from;
        while at < to {
            let comment = lexed.opaque.iter().find(|&&(start, end)| start <= at && at < end && is_comment(body, start));
            let next = comment.map_or(to, |&(_, end)| end.min(to));
            let until = lexed.opaque.iter().filter(|&&(start, _)| start > at && start < to && is_comment(body, start)).map(|&(start, _)| start).min().unwrap_or(to);
            match comment {
                Some(_) => {
                    out.push(' ');
                    at = next;
                }
                None => {
                    out.push_str(&body[at..until]);
                    at = until;
                }
            }
        }
        out
    };
    // The list of columns after it, where there is one: each a name, between the commas of the list's own level.
    let after = code_after(body, lexed, end);
    let listed = match b.get(after) {
        Some(b'(') if after < top[values].0 => {
            let close = code.iter().find(|&&(at, byte, depth)| at > after && byte == b')' && depth == 0)?.0;
            let mut cuts: Vec<usize> = code.iter().filter(|&&(at, byte, depth)| at > after && at < close && byte == b',' && depth == 1).map(|&(at, _, _)| at).collect();
            cuts.push(close);
            let mut start = after + 1;
            let mut names = Vec::with_capacity(cuts.len());
            for cut in cuts {
                let name = plain(start, cut);
                let name = name.trim();
                if name_length(name) != Some(name.len()) {
                    return None;
                }
                names.push(if name.starts_with(['`', '"']) { name[1..name.len() - 1].replace('\\', "") } else { name.to_string() });
                start = cut + 1;
            }
            Some(names)
        }
        _ => None,
    };
    // Each row's values, a level inside its parentheses.
    let from = top[values].0 + "values".len();
    let mut written = Vec::new();
    let (mut place, mut item) = (0usize, from);
    let mut alone = |place: usize, start: usize, end: usize| {
        let inside: Vec<usize> = (0..lexed.placeholders.len()).filter(|&k| start <= lexed.placeholders[k].start && lexed.placeholders[k].end <= end).collect();
        if let [k] = inside[..] {
            // The parameter alone, with its cast where it is written with one: a comment beside it changes nothing.
            let p = &lexed.placeholders[k];
            let (before, cast) = (plain(start, p.start), plain(p.end, end));
            let cast = cast.trim();
            if before.trim().is_empty() && (cast.is_empty() || cast.strip_prefix("::").is_some_and(|ty| type_end(ty) == ty.len())) {
                written.push((place, k));
            }
        }
    };
    for &(at, byte, depth) in code.iter().filter(|&&(at, _, _)| at >= from) {
        match (byte, depth) {
            (b'(', 0) => {
                place = 0;
                item = at + 1;
            }
            (b',', 1) => {
                alone(place, item, at);
                place += 1;
                item = at + 1;
            }
            (b')', 0) => alone(place, item, at),
            _ => {}
        }
    }
    Some((table, listed, written))
}

/// The statement's code a byte at a time, each with its place and how many parentheses, brackets
/// and braces it stands in: its strings, quoted names, comments and placeholders left out.
fn code(body: &str, lexed: &Lexed) -> Vec<(usize, u8, usize)> {
    let b = body.as_bytes();
    let skip = |i: usize| lexed.opaque.iter().copied().chain(lexed.placeholders.iter().map(|p| (p.start, p.end))).find(|&(start, end)| start <= i && i < end).map(|(_, end)| end);
    let mut out = Vec::with_capacity(b.len());
    let (mut i, mut depth) = (0, 0usize);
    while i < b.len() {
        if let Some(end) = skip(i) {
            i = end;
            continue;
        }
        match b[i] {
            b'(' | b'[' | b'{' => {
                out.push((i, b[i], depth));
                depth += 1;
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                out.push((i, b[i], depth));
            }
            byte => out.push((i, byte, depth)),
        }
        i += 1;
    }
    out
}

/// The SELECT a DELETE or an UPDATE reads as, as the edits of its text that make it one, each a
/// place, a length and what stands there. `DELETE FROM t WHERE c` is `SELECT 1 FROM t WHERE c`.
/// `UPDATE t SET a = x, b = y WHERE c` is `FROM t SELECT a , x, b , y WHERE c`, which names each
/// column written and computes each value, and `ALTER TABLE t UPDATE … WHERE c` and `ALTER TABLE t
/// DELETE WHERE c` read the same way. `None` where the statement opens otherwise.
fn as_select(body: &str, lexed: &Lexed) -> Option<Vec<(usize, usize, String)>> {
    let code = code(body, lexed);
    // The words outside every parenthesis, lowercased, each with where it starts.
    let mut words: Vec<(usize, String)> = Vec::new();
    let mut last = None;
    for &(at, byte, depth) in &code {
        if depth > 0 || !(byte.is_ascii_alphanumeric() || byte == b'_') {
            last = None;
            continue;
        }
        match words.last_mut() {
            Some((_, word)) if last == Some(at.wrapping_sub(1)) => word.push(byte.to_ascii_lowercase() as char),
            _ => words.push((at, (byte.to_ascii_lowercase() as char).to_string())),
        }
        last = Some(at);
    }
    let word = |i: usize| words.get(i).map(|(_, word)| word.as_str());
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    // The word the assignments follow, where the statement has them.
    let set = match (word(0)?, word(1)) {
        ("delete", _) => {
            edits.push((words[0].0, "delete".len(), "SELECT 1".into()));
            return Some(edits);
        }
        ("update", _) => {
            edits.push((words[0].0, "update".len(), "FROM".into()));
            let set = words.iter().position(|(_, word)| word == "set")?;
            edits.push((words[set].0, "set".len(), "SELECT".into()));
            set
        }
        ("alter", Some("table")) => {
            edits.push((words[0].0, words[1].0 + "table".len() - words[0].0, "FROM".into()));
            let command = words.iter().skip(2).position(|(_, word)| word == "update" || word == "delete")? + 2;
            if words[command].1 == "delete" {
                edits.push((words[command].0, "delete".len(), "SELECT 1".into()));
                return Some(edits);
            }
            edits.push((words[command].0, "update".len(), "SELECT".into()));
            command
        }
        _ => return None,
    };
    // Each assignment's `=`, the first of its item, as the comma between the column and the value.
    let from = words[set].0 + words[set].1.len();
    let to = words.iter().rposition(|(_, word)| word == "where").filter(|&i| i > set).map_or(body.len(), |i| words[i].0);
    let mut assigned = false;
    for &(at, byte, depth) in code.iter().filter(|&&(at, _, _)| at >= from && at < to) {
        match byte {
            b',' if depth == 0 => assigned = false,
            b'=' if depth == 0 && !assigned => {
                assigned = true;
                edits.push((at, 1, ",".into()));
            }
            _ => {}
        }
    }
    Some(edits)
}

/// A rewritten statement with `edits` made, each at a place of the author's text that stands in
/// no placeholder: the placeholders before it are each as long as what stands for them.
fn edited(text: &Rewritten, edits: &[(usize, usize, String)]) -> String {
    let moved = |at: usize| text.spans.iter().take_while(|&&(_, end, _, _)| end <= at).last().map_or(at, |&(_, end, _, rewritten_end)| rewritten_end + (at - end));
    let mut ordered: Vec<&(usize, usize, String)> = edits.iter().collect();
    ordered.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    let mut out = text.sql.clone();
    for (at, length, with) in ordered {
        let start = moved(*at);
        out.replace_range(start..start + length, with);
    }
    out
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
    }

    /// A describe that fails, as the server does with a text it cannot compute with.
    fn unparsed(name: &str, code: &str) -> Result<Vec<Vec<Value>>, DbError> {
        Err(error(&format!("Code: {code}. DB::Exception: Cannot parse it. ({name}) (version 26.9)"), code))
    }

    #[test]
    fn a_statement_is_described_with_values_it_computes_with() {
        // A function of a LowCardinality column and a constant is LowCardinality, and of two columns
        // is not: the types are the ones the statement runs with, a date standing for the string a
        // date is made of, which no letter is.
        let asked = std::cell::RefCell::new(Vec::new());
        let sql = "SELECT concat(plan, {suffix:String}) AS label FROM users WHERE created >= toDateTime({since:String})";
        let a = run(sql, &|sent| {
            asked.borrow_mut().push(sent.to_string());
            match sent.strip_prefix(DESCRIBE) {
                Some(text) if text.contains("materialize(") => Ok(vec![vec![json!("label"), json!("String")]]),
                Some(text) if text.contains("toDateTime(CAST('2001-02-03 04:05:06', 'String'))") || text.contains("toDateTime(CAST('2004-05-06 07:08:09', 'String'))") => Ok(vec![vec![json!("label"), json!("LowCardinality(String)")]]),
                Some(_) => unparsed("CANNOT_PARSE_DATETIME", "41"),
                None => server(&[], "QUERY id: 0")(sent),
            }
        });
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(a.columns[0].described.as_deref(), Some("LowCardinality(String)"));
        // No parameter is one the server reads as a constant, so none is asked about alone.
        assert_eq!(asked.borrow().iter().filter(|s| s.starts_with(DESCRIBE)).count(), 1 + STRINGS.len());

        // Two parameters that each take a text of their own kind: one is changed at a time, and a
        // change the statement fails otherwise with is kept.
        let sql = "SELECT toDate(created, {zone:String}) AS day FROM users WHERE created >= {since:String}";
        let b = run(sql, &|sent| match sent.strip_prefix(DESCRIBE) {
            Some(text) if text.contains("materialize(") => Err(error("Code: 43. DB::Exception: Illegal type of argument. (ILLEGAL_TYPE_OF_ARGUMENT) (version 26.9)", "43")),
            Some(text) if !["'UTC'", "'Asia/Tokyo'"].iter().any(|zone| text.contains(&format!("toDate(created, CAST({zone}, 'String'))"))) => unparsed("BAD_ARGUMENTS", "36"),
            Some(text) if !["'2001-02-03 04:05:06'", "'2004-05-06 07:08:09'"].iter().any(|time| text.contains(&format!(">= CAST({time}, 'String')"))) => unparsed("CANNOT_PARSE_DATETIME", "41"),
            Some(_) => Ok(vec![vec![json!("day"), json!("Date")]]),
            None => server(&[], "QUERY id: 0\n  LIMIT\n    CONSTANT id: 4, constant_value: UInt64_1, constant_value_type: UInt64")(sent),
        });
        assert!(b.diagnostics.is_empty(), "{:?}", b.diagnostics);
        assert_eq!(b.columns[0].described.as_deref(), Some("Date"));
        // Both are constants to the server, so what they decide is the caller's.
        assert_eq!(b.cardinality, Cardinality::Many);

        // A text the statement computes with, and none of the values tried is one: the parameter is refused where it stands.
        let sql = "SELECT parse({code:String}) AS n FROM users WHERE id = {id:UInt64}";
        let c = run(sql, &|sent| match sent.strip_prefix(DESCRIBE) {
            Some(text) if text.contains("parse(materialize(") => Ok(vec![vec![json!("n"), json!("UInt8")]]),
            Some(_) => Err(error("Code: 6. DB::Exception: Cannot parse the code: In scope SELECT parse(CAST('a', 'String')). (CANNOT_PARSE_TEXT) (version 26.9)", "6")),
            None => server(&[], "QUERY id: 0")(sent),
        });
        assert_eq!(c.diagnostics.len(), 1, "{:?}", c.diagnostics);
        assert_eq!(&sql[c.diagnostics[0].start..c.diagnostics[0].start + c.diagnostics[0].length], "{code:String}");
        assert_eq!(c.diagnostics[0].message, "ClickHouse computes with {code:String} as it reads the statement, and the statement fails with each value tried in its place, as with 'a'. Declare the parameter as the type the statement converts it to, or write the value in the statement. (Cannot parse the code: In scope SELECT parse({code:String}). (CANNOT_PARSE_TEXT))");
    }

    #[test]
    fn a_column_named_by_a_parameter_s_value_is_the_statement_s_value_or_refused() {
        // ClickHouse names a column with no alias by its expression, each constant in it written out.
        let named = |sent: &str| match sent.strip_prefix(DESCRIBE) {
            Some(text) => {
                let value = text.split("CAST(").nth(1).and_then(|rest| rest.split(',').next()).unwrap_or("?").to_string();
                let name = if text.contains("materialize(") { "plus(id, materialize(defaultValueOfTypeName('UInt32')))".to_string() } else { format!("plus(id, CAST({value}, 'UInt32'))") };
                Ok(if text.contains("SELECT *") { vec![vec![json!("id"), json!("UInt32")], vec![json!(name), json!("UInt64")]] } else { vec![vec![json!(name), json!("UInt64")]] })
            }
            None => server(&[], "QUERY id: 0")(sent),
        };
        // One such column alone goes by no name: the statement returns its values.
        let a = run("SELECT id + {x:UInt32} FROM users", &named);
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert!(a.value);
        // Among others, its name would change from run to run.
        let b = run("SELECT * FROM (SELECT id, id + {x:UInt32} FROM users)", &named);
        assert_eq!(b.diagnostics.len(), 1, "{:?}", b.diagnostics);
        assert_eq!(b.diagnostics[0].message, "Column 2 is named \"plus(id, {x:UInt32})\" by its expression, which holds a parameter's value, so its name changes from run to run. Give it one with `as` where the statement makes it.");
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
        // Each of the two is tried with every value of its list, alone, in the statement as it runs.
        for value in ["CAST('a', 'String')) AS shown, quantile(CAST(0.5, 'Float64'))", "CAST('b', 'String')) AS shown, quantile(CAST(0.5, 'Float64'))", "CAST('UTC', 'String')) AS shown, quantile(CAST(0.5, 'Float64'))", "CAST('a', 'String')) AS shown, quantile(CAST(0.25, 'Float64'))", "CAST('a', 'String')) AS shown, quantile(CAST(-1, 'Float64'))"] {
            assert!(asked.borrow().iter().any(|s| s.starts_with(DESCRIBE) && s.contains(value) && s.contains("id = CAST(1, 'UInt64')")), "{value}");
        }
        // The tree is read with the two written as constants, and the third parameter a column.
        assert!(asked.borrow().iter().any(|s| s.starts_with(TREE) && s.contains("CAST('a', 'String')) AS shown, quantile(CAST(0.5, 'Float64'))") && s.contains("id = materialize(")));
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
        // A place in the tuple past the first values tried is tried too, and one counted from its end.
        let sql = "SELECT tupleElement(spot, {at:Int8}) AS side FROM users";
        let by_place = |text: &str| Ok(vec![vec![json!("side"), json!(if text.contains("CAST(4, 'Int8')") || text.contains("CAST(-1, 'Int8')") { "String" } else { "Float64" })]]);
        let last = run(sql, &constant_only("tupleElement(spot, ", &by_place));
        assert_eq!(last.diagnostics.len(), 1, "{:?}", last.diagnostics);
        assert_eq!(last.diagnostics[0].message, "ClickHouse reads {at:Int8} as a constant here, and the result's types change with its value: column 1 (side) has the type Float64 with 1 and String with 4. Write the value in the statement.");
        // A unit of a date gives a date where a unit of a time gives a time: both kinds are tried.
        let sql = "SELECT dateTrunc({unit:String}, created) AS start FROM users";
        let by_unit = |text: &str| match text.split("dateTrunc(CAST('").nth(1).and_then(|rest| rest.split('\'').next()) {
            Some("day" | "hour") => Ok(vec![vec![json!("start"), json!("DateTime64(0, 'UTC')")]]),
            Some("month") => Ok(vec![vec![json!("start"), json!("Date32")]]),
            _ => unparsed("BAD_ARGUMENTS", "36"),
        };
        let unit = run(sql, &constant_only("dateTrunc(", &by_unit));
        assert_eq!(unit.diagnostics.len(), 1, "{:?}", unit.diagnostics);
        assert_eq!(unit.diagnostics[0].message, "ClickHouse reads {unit:String} as a constant here, and the result's types change with its value: column 1 (start) has the type DateTime64(0, 'UTC') with 'day' and Date32 with 'month'. Write the value in the statement.");
        // Where no other value describes, nothing shows that the types hold.
        let sql = "SELECT tupleElement(pair, {at:UInt8}) AS side FROM users";
        let failing_from = |_: u8| move |text: &str| match text.contains("CAST(1, 'UInt8')") {
            true => Ok(vec![vec![json!("side"), json!("UInt8")]]),
            false => Err(error("Code: 10. DB::Exception: Tuple doesn't have element with that index. (NOT_FOUND_COLUMN_IN_BLOCK) (version 26.9.23.15 (official build))", "10")),
        };
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
        assert_eq!(d.message, "ClickHouse reads {type:String} as a constant here, and the statement fails with each value tried in its place, as with 'a', so its value is part of what the statement says. Write the value in the statement. (Unknown data type family: a: In scope SELECT CAST(age, {type:String}) AS a FROM users. (UNKNOWN_TYPE))");
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
    fn a_result_of_more_than_rows_is_refused() {
        let a = run("SELECT plan, count() AS n FROM users GROUP BY plan WITH TOTALS", &server(&[("plan", "String"), ("n", "UInt64")], "QUERY id: 0, is_group_by_with_totals: 1\n  PROJECTION COLUMNS"));
        assert_eq!(a.diagnostics.len(), 1);
        assert!(a.diagnostics[0].message.starts_with("WITH TOTALS returns its totals beside the rows, and a typed result is its rows alone"), "{}", a.diagnostics[0].message);
        // A format the statement names is the one the server would answer every question in, so it is read off the text.
        let sql = "SELECT 'default_format' AS n SETTINGS default_format = 'JSONEachRow'";
        let b = run(sql, &|_| Err(error("unread", "1")));
        assert_eq!(b.diagnostics[0].message, "The statement's SETTINGS names a format for its result, default_format, and Reylo reads the one it asks for: take the setting out.");
        assert_eq!(b.diagnostics[0].start, sql.rfind("default_format").unwrap());
        let c = run("SELECT 1 AS n SETTINGS extremes = 1", &server(&[("n", "UInt8")], "QUERY id: 0\n  PROJECTION COLUMNS\n  SETTINGS extremes=1"));
        assert!(c.diagnostics[0].message.starts_with("The statement's SETTINGS turns on extremes"), "{}", c.diagnostics[0].message);
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
        // An UPDATE reads as the SELECT of each column it writes and each value it computes.
        asked.borrow_mut().clear();
        let update = run("UPDATE orders SET note = {note:Nullable(String)}, total = round(total, 2) + 1 WHERE id IN (SELECT id FROM gone WHERE at = 1) AND id = {id:UInt64}", &ok);
        assert_eq!((update.kind, update.cardinality, update.diagnostics.len()), (Some(StatementKind::Update), Cardinality::None, 0));
        assert!(asked.borrow().iter().any(|s| s == "DESCRIBE (\nFROM orders SELECT note , materialize(defaultValueOfTypeName('Nullable(String)')), total , round(total, 2) + 1 WHERE id IN (SELECT id FROM gone WHERE at = 1) AND id = materialize(defaultValueOfTypeName('UInt64'))\n)"), "{:?}", asked.borrow());
        // An ALTER is one of rows where each of its commands is, as the server's parse names them.
        let log = &asked;
        let parse = |commands: &'static str| move |sent: &str| {
            log.borrow_mut().push(sent.to_string());
            match sent.strip_prefix(AST) {
                Some(text) if text.starts_with("ALTER") => Ok(format!("AlterQuery  orders (children 2)\n ExpressionList (children 1)\n{commands}").lines().map(|line| vec![json!(line)]).collect()),
                _ => Ok(Vec::new()),
            }
        };
        asked.borrow_mut().clear();
        let altered = run("ALTER TABLE orders UPDATE note = {note:Nullable(String)} WHERE id = {id:UInt64}", &parse("  AlterCommand UPDATE (children 2)"));
        assert_eq!((altered.cardinality, altered.diagnostics.len()), (Cardinality::None, 0), "{:?}", altered.diagnostics);
        assert!(asked.borrow().iter().any(|s| s == "DESCRIBE (\nFROM orders SELECT note , materialize(defaultValueOfTypeName('Nullable(String)')) WHERE id = materialize(defaultValueOfTypeName('UInt64'))\n)"), "{:?}", asked.borrow());
        asked.borrow_mut().clear();
        let cut = run("ALTER TABLE orders DELETE WHERE id = {id:UInt64}", &parse("  AlterCommand DELETE (children 1)"));
        assert!(cut.diagnostics.is_empty(), "{:?}", cut.diagnostics);
        assert!(asked.borrow().iter().any(|s| s == "DESCRIBE (\nFROM orders SELECT 1 WHERE id = materialize(defaultValueOfTypeName('UInt64'))\n)"), "{:?}", asked.borrow());
        let added = run("ALTER TABLE orders ADD COLUMN z UInt8", &parse("  AlterCommand ADD_COLUMN (children 1)"));
        assert!(added.diagnostics[0].message.starts_with("Reylo types an ALTER TABLE that changes rows, with UPDATE or DELETE, and ADD COLUMN is neither, so if it is a migration"), "{}", added.diagnostics[0].message);
        let frozen = run("ALTER TABLE orders UPDATE note = '' WHERE 1, FREEZE", &parse("  AlterCommand UPDATE (children 2)\n  AlterCommand FREEZE_ALL"));
        assert!(frozen.diagnostics[0].message.contains("and FREEZE ALL is neither"), "{}", frozen.diagnostics[0].message);
        // A value a statement computes with as it is read is tried with each of a parameter's, and the statement refused where none is taken.
        let dated = run("UPDATE orders SET note = '' WHERE created >= {since:String}", &|sent| match sent.strip_prefix(DESCRIBE) {
            Some(text) if text.contains("'2001-02-03 04:05:06'") => Ok(Vec::new()),
            Some(_) => unparsed("CANNOT_PARSE_DATETIME", "41"),
            None => Ok(Vec::new()),
        });
        assert!(dated.diagnostics.is_empty(), "{:?}", dated.diagnostics);
        let missing = run("UPDATE orders SET nope = {note:String} WHERE id = 1", &|sent| if sent.starts_with(DESCRIBE) { Err(error("Code: 47. DB::Exception: Unknown expression identifier `nope`. (UNKNOWN_IDENTIFIER) (version 26.9)", "47")) } else { Ok(Vec::new()) });
        assert_eq!(missing.diagnostics[0].message, "Unknown expression identifier `nope`. (UNKNOWN_IDENTIFIER)");
        // A parameter written to a column is held to a type whose every value the column holds.
        let table = |sent: &str| match sent {
            "DESCRIBE TABLE shop.`order lines`" => Ok(vec![vec![json!("id"), json!("UInt8"), json!("")], vec![json!("total"), json!("Decimal(9, 2)"), json!("")], vec![json!("seen"), json!("UInt8"), json!("MATERIALIZED")], vec![json!("note"), json!("LowCardinality(String)"), json!("DEFAULT")]]),
            _ => Ok(Vec::new()),
        };
        let sql = "INSERT INTO shop.`order lines` VALUES ({id:UInt64}, {total:Decimal(18, 4)}, {note:Nullable(String)}), (1 + {id:UInt64}, 2, 'x')";
        let wide = run(sql, &table);
        assert_eq!(wide.diagnostics.iter().map(|d| &sql[d.start..d.start + d.length]).collect::<Vec<_>>(), ["{id:UInt64}", "{total:Decimal(18, 4)}", "{note:Nullable(String)}"]);
        assert_eq!(wide.diagnostics[0].message, "{id:UInt64} is written to the column id, a UInt8, and ClickHouse changes a value the column cannot hold into one it can, without an error. Declare the parameter as the column's type: {id:UInt8}.");
        assert!(wide.diagnostics[2].message.ends_with("Declare the parameter as the column's type: {note:String}."), "{}", wide.diagnostics[2].message);
        // By the columns the statement lists, in its order; a cast is the parameter's type too.
        let listed = run("INSERT INTO shop.`order lines` (note, id) VALUES (:note::String, :id::UInt16)", &table);
        assert_eq!(listed.diagnostics.len(), 1, "{:?}", listed.diagnostics);
        assert!(listed.diagnostics[0].message.starts_with(":id is written to the column id, a UInt8"), "{}", listed.diagnostics[0].message);
        // A comment beside a parameter, or in the list of columns, is no part of what is written.
        let sql = "INSERT INTO shop.`order lines` (note /* its text */, id -- the key\n) VALUES (/* the note */ {note:String}, {id:UInt16} /* too wide */)";
        let commented = run(sql, &table);
        assert_eq!(commented.diagnostics.iter().map(|d| &sql[d.start..d.start + d.length]).collect::<Vec<_>>(), ["{id:UInt16}"]);
        let fits = run("INSERT INTO shop.`order lines` (id, total, note) VALUES ({id:UInt8}, {total:Decimal(9, 2)}, {note:String}), ({id:UInt8}, toDecimal32({total:Decimal(9, 2)}, 2), concat({note:String}, 'x'))", &table);
        assert!(fits.diagnostics.is_empty(), "{:?}", fits.diagnostics);
        let format = run("INSERT INTO orders FORMAT JSONEachRow", &ok);
        assert!(format.diagnostics[0].message.starts_with("An INSERT … FORMAT takes its rows"));
        let ddl = run("CREATE TABLE t (id UInt64) ENGINE = MergeTree ORDER BY id", &ok);
        assert!(ddl.diagnostics[0].message.starts_with("Reylo types a ClickHouse SELECT, INSERT, DELETE or UPDATE, and CREATE is none of them"), "{}", ddl.diagnostics[0].message);
    }
}
