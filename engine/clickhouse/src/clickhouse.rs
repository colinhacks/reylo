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
//! `FixedString`. So the describe is asked of the statement with each parameter written as a value
//! that is no constant, `materialize(defaultValueOfTypeName('T'))`, which has the parameter's type
//! and no value to read: ClickHouse then refuses every place that needs a constant, and the types
//! it answers with hold for every value. Two places need a constant and decide no type, so a
//! parameter there stays one: a `LIMIT` or an `OFFSET`, and an aggregate function's parameters
//! (`quantile({level:Float64})(x)`). A parameter that names a column or a table,
//! `{c:Identifier}`, changes what the statement reads, and is refused.
//!
//! How many rows the statement returns is read from the server's query tree ([`crate::tree`]). A
//! write (`INSERT`, `DELETE`, `UPDATE`, `ALTER TABLE … UPDATE` or `DELETE`) returns no rows, and
//! ClickHouse describes none: an `INSERT` is checked against its table by `EXPLAIN PIPELINE`,
//! which builds the insert and writes nothing, a `DELETE` as the `SELECT` of the rows it would
//! delete, and the others for their syntax (`EXPLAIN AST`).

use crate::api::{Analysis, Column, Diagnostic, Param, Scope};
use crate::host::{as_str, Host, Lazy};
use crate::types::{self, Named};
use reylo_engine_core::analysis::{self, ResultTarget};
use reylo_engine_core::api::DbError;
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::lex::{self, Lexed, Placeholder, Rewritten, Target};
use reylo_engine_core::types::{Cardinality, StatementKind, Type};

/// The settings of the session that may drop the one row an aggregate returns: the three that
/// leave it are off, `0`, by default.
pub const ROW_SETTINGS: &str = "SELECT name, value FROM system.settings WHERE name IN ('empty_result_for_aggregation_by_empty_set', 'limit', 'offset')";

const DESCRIBE: &str = "DESCRIBE (\n";
const TREE: &str = "EXPLAIN QUERY TREE\n";
const PIPELINE: &str = "EXPLAIN PIPELINE\n";
const AST: &str = "EXPLAIN AST\n";

/// Where a parameter stands, as far as the analysis writes it differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    /// Anywhere a value goes.
    Value,
    /// Straight after `LIMIT`, or after the comma of `LIMIT offset, count`.
    Limit,
    /// Straight after `OFFSET`.
    Offset,
    /// In an aggregate function's parameters: the first parentheses of `quantile(0.9)(x)`.
    Aggregate,
}

/// How a statement's parameters are written into the text the server is asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum As {
    /// Each as a value that is no constant, but where its position needs one.
    Values,
    /// Each as a constant.
    Constants,
    /// The one at this index as a value that is no constant, and the others as constants.
    Value(usize),
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
    // The parentheses the placeholder stands in, and whether another pair follows them at once, as
    // only an aggregate function's parameters are followed (`quantile(0.9)(x)`).
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
    let (mut i, mut depth, mut close) = (p.end, 0usize, None);
    while i < b.len() {
        if let Some(&(_, end)) = skip.iter().find(|&&(start, _)| start == i) {
            i = end;
            continue;
        }
        match b[i] {
            b'(' => depth += 1,
            b')' if depth == 0 => {
                close = Some(i);
                break;
            }
            b')' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    let Some(close) = close else { return Position::Value };
    let called = word_before(sql, code_before(sql, lexed, open));
    if called.1 < open && b.get(code_after(sql, lexed, close + 1)) == Some(&b'(') {
        return Position::Aggregate;
    }
    Position::Value
}

/// What stands for a parameter of `ty` in the text the server is asked about: a value of its type
/// that is no constant, or a constant of it, 1 for a number, since a limit or an aggregate's
/// parameter may refuse 0.
fn stand_in(ty: &Named, constant: bool) -> String {
    let name = types::quoted(&ty.to_string());
    match constant {
        false => format!("materialize(defaultValueOfTypeName({name}))"),
        true if ty.is_number() => format!("CAST(1, {name})"),
        true => format!("defaultValueOfTypeName({name})"),
    }
}

/// The statement's parameters as the analysis reads them: each placeholder's type and position.
struct Parameters {
    types: Vec<Named>,
    positions: Vec<Position>,
}

impl Parameters {
    /// The statement `body` with each placeholder written as `how` says, and the map from that
    /// text back to the author's.
    fn text(&self, body: &str, lexed: &Lexed, how: As) -> Rewritten {
        let mut out = String::with_capacity(body.len());
        let mut spans = Vec::with_capacity(lexed.placeholders.len());
        let mut last = 0;
        for (k, p) in lexed.placeholders.iter().enumerate() {
            let constant = match how {
                As::Values => self.positions[k] != Position::Value,
                As::Constants => true,
                As::Value(one) => k != one,
            };
            out.push_str(&body[last..p.start]);
            let start = out.len();
            out.push_str(&stand_in(&self.types[k], constant));
            spans.push((p.start, p.end, start, out.len()));
            last = p.end;
        }
        out.push_str(&body[last..]);
        Rewritten { sql: out, order: Vec::new(), spans, inserted: Vec::new() }
    }

    /// The server's text with each stand-in read back as the author wrote its parameter, where one
    /// parameter alone has that stand-in.
    fn authored(&self, sql: &str, lexed: &Lexed, message: &str) -> String {
        let mut out = message.to_string();
        for constant in [false, true] {
            for (k, p) in lexed.placeholders.iter().enumerate() {
                let stood = stand_in(&self.types[k], constant);
                let same = lexed.placeholders.iter().enumerate().filter(|(j, q)| stand_in(&self.types[*j], constant) == stood && q.name != p.name).count();
                if same == 0 {
                    out = out.replace(&stood, &sql[p.start..p.end]);
                }
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

/// The diagnostic of an error the server answered a question about `text` with, the question
/// opening with `prefix`: at the place a syntax error names, else the whole statement.
fn server_error(sql: &str, lexed: &Lexed, parameters: &Parameters, text: &Rewritten, prefix: &str, e: &DbError) -> Diagnostic {
    let message = parameters.authored(sql, lexed, &message_of(e));
    match position_of(&e.message).and_then(|at| at.checked_sub(prefix.len())) {
        Some(at) => {
            // An error at the end of the statement names the `)` the question closes with.
            let start = text.to_original(at.min(text.sql.len().saturating_sub(1))).min(sql.trim_end().len().saturating_sub(1));
            let hint = sql.get(start..).unwrap_or_default().strip_prefix(':').filter(|rest| rest.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')).map(|rest| {
                let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                format!(" A ClickHouse parameter is written with its type: {{{name}:Type}}, or :{name}::Type.")
            });
            Diagnostic { message: format!("{message}{}", hint.unwrap_or_default()), start, length: 1, code: 6, server_code: e.code.clone() }
        }
        None => whole(sql, message, 6, e.code.clone()),
    }
}

/// Whether a result column's name is one a statement can rely on: a name, as an alias or a column
/// read as it is has, and not an expression's text, which ClickHouse names an unnamed column by.
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_') && chars.all(|c| c.is_alphanumeric() || c == '_')
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
    // Each placeholder's type, as it declares it.
    let mut declared: Vec<Named> = Vec::with_capacity(lexed.placeholders.len());
    let mut refused: Vec<Diagnostic> = Vec::new();
    for (k, p) in lexed.placeholders.iter().enumerate() {
        let written = &sql[p.start..p.end];
        let Some(ty) = written_type(sql, p).and_then(types::parse) else {
            refused.push(at(p, format!("{written} names no type: a ClickHouse parameter is written with its type, as in {{{}:UInt64}}.", p.name), 10));
            declared.push(Named { name: "Nothing".into(), args: Vec::new() });
            continue;
        };
        if ty.name == "Identifier" {
            refused.push(at(p, format!("{written} stands for a name, so the statement's columns and their types would change with its value: write the name in the statement."), 10));
        } else if let Err(types::Unread(name)) = types::slot(&ty) {
            refused.push(at(p, format!("Reylo binds no parameter of the type {name}: it is no ClickHouse type, or one whose values it does not write yet, as a JSON, a Dynamic or an interval is."), 10));
        } else if ty.holds("Variant") {
            // A Variant's text does not say which of its types a value is: 7 reads as a number and as a string.
            refused.push(at(p, format!("{written} is a Variant, whose value's text does not say which of its types it is: declare the parameter with the one type it has."), 10));
        } else if let Some(first) = lexed.placeholders[..k].iter().position(|q| q.name == p.name).filter(|&first| declared[first] != ty) {
            refused.push(at(p, format!("The parameter {} is a {} here and a {} at its first use: declare it with one type.", p.name, ty, declared[first]), 3));
        }
        declared.push(ty);
    }
    if !refused.is_empty() {
        return refuse(refused);
    }
    let parameters = Parameters { positions: lexed.placeholders.iter().map(|p| position(sql, &lexed, p)).collect(), types: declared };
    // The statement without what follows it: a `;`, and the whitespace and comments after one.
    let in_text = |i: usize| lexed.opaque.iter().any(|&(start, end)| start <= i && i < end);
    let body = &sql[..sql.bytes().enumerate().find(|&(i, c)| c == b';' && !in_text(i)).map_or(sql.len(), |(i, _)| i)];
    let asked = parameters.text(body, &lexed, As::Values);
    let kind = match word.as_str() {
        "insert" => StatementKind::Insert,
        "delete" => StatementKind::Delete,
        "update" => StatementKind::Update,
        "alter" => StatementKind::Other,
        _ => StatementKind::Select,
    };
    let mut typed = analysis::bare(sql, &lexed, Dialect::Clickhouse, Vec::new());
    typed.kind = Some(kind);
    typed.rewritten = lex::rewrite_with(sql, &lexed, Target::Braced, &|name, _| {
        let k = lexed.placeholders.iter().position(|p| p.name == name).unwrap_or(0);
        format!("{{{name}:{}}}", written_type(sql, &lexed.placeholders[k]).unwrap_or_default())
    });
    typed.params = lexed
        .params
        .iter()
        .map(|name| {
            let ty = &parameters.types[lexed.placeholders.iter().position(|p| &p.name == name).unwrap_or(0)];
            let slot = types::slot(ty).expect("a type the parameters were held to");
            Param { nullable: slot.nullable, ..Param::new(name.clone(), slot.ty) }
        })
        .collect();

    if writes {
        return write(sql, &lexed, &parameters, body, &asked, typed, host);
    }

    // One round: the describe, the query tree and the session's settings.
    let described = host.query(&format!("{DESCRIBE}{}\n)", asked.sql), Vec::new(), Scope::Statement);
    let tree = host.query(&format!("{TREE}{}", asked.sql), Vec::new(), Scope::Statement);
    let settings = host.query(ROW_SETTINGS, Vec::new(), Scope::Session);
    let (described, tree, settings) = (described?, tree?, settings?);
    let rows = match described {
        Ok(rows) => rows,
        Err(e) => return constant_needed(sql, &lexed, &parameters, body, &asked, &e, host).map(|d| analysis::bare(sql, &lexed, Dialect::Clickhouse, d)),
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
                    unread.push(whole(sql, format!("Column {} ({name}) is a {described}, and Reylo does not read a ClickHouse {unread_name} yet: select it as a type it reads, as toString({name}) or toJSONString({name}) does.", i + 1), 10, None));
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
    let limit_given = parameters.positions.contains(&Position::Limit);
    let lines: Vec<&str> = tree.as_ref().map(|rows| rows.iter().filter_map(|row| row.first().and_then(as_str)).flat_map(str::lines).collect()).unwrap_or_default();
    let proven = crate::tree::rows(&lines, limit_given);
    let defaults = settings.ok().is_some_and(|rows| rows.len() == 3 && rows.iter().all(|row| row.get(1).and_then(as_str) == Some("0")));
    let (cardinality, reason) = if proven.one && defaults {
        (Cardinality::One, None)
    } else if proven.one {
        (Cardinality::AtMostOne, Some("a setting of the session (empty_result_for_aggregation_by_empty_set, limit or offset) may drop the aggregate's one row"))
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
    let targets: Vec<ResultTarget> = typed.columns.iter().map(|c| if is_identifier(&c.name) { ResultTarget::Named } else { ResultTarget::Unnamed(None) }).collect();
    analysis::names(sql, Dialect::Clickhouse, &mut typed, Some(&targets), &|_| None);
    Some(typed)
}

/// Why the describe failed, as diagnostics. Where the statement describes with every parameter a
/// constant, the describe refused a parameter it needs as one, and each such parameter is found by
/// asking again with it alone as a value. Else the statement is wrong whatever its parameters are,
/// and the error is the describe's.
fn constant_needed(sql: &str, lexed: &Lexed, parameters: &Parameters, body: &str, asked: &Rewritten, error: &DbError, host: &Host) -> Lazy<Vec<Diagnostic>> {
    let values: Vec<usize> = (0..lexed.placeholders.len()).filter(|&k| parameters.positions[k] == Position::Value).collect();
    let own_error = || vec![server_error(sql, lexed, parameters, asked, DESCRIBE, error)];
    if values.is_empty() {
        return Some(own_error());
    }
    let constants = host.query(&format!("{DESCRIBE}{}\n)", parameters.text(body, lexed, As::Constants).sql), Vec::new(), Scope::Statement);
    let alone: Vec<_> = values.iter().map(|&k| host.query(&format!("{DESCRIBE}{}\n)", parameters.text(body, lexed, As::Value(k)).sql), Vec::new(), Scope::Statement)).collect();
    if constants?.is_err() {
        return Some(own_error());
    }
    let mut found = Vec::new();
    for (&k, answer) in values.iter().zip(alone) {
        if let Err(e) = answer? {
            let p = &lexed.placeholders[k];
            let why = parameters.authored(sql, lexed, &message_of(&e));
            found.push(at(p, format!("ClickHouse reads {} as a constant here, where its value can change the types of the result, so the statement has no one type: write the value in the statement. ({})", &sql[p.start..p.end], why.trim_end_matches('.')), 10));
        }
    }
    Some(if found.is_empty() { own_error() } else { found })
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
    let parsed = host.query(&format!("{AST}{}", asked.sql), Vec::new(), Scope::Statement);
    let built = insert.then(|| host.query(&format!("{PIPELINE}{}", asked.sql), Vec::new(), Scope::Statement));
    // `DELETE FROM t WHERE c` reads as `SELECT 1 FROM t WHERE c`, the first word alone changed.
    let deleted = (typed.kind == Some(StatementKind::Delete)).then(|| {
        let cut = (code_after(sql, lexed, 0) + "delete".len()).min(body.len());
        let select = format!("SELECT 1{}", &body[cut..]);
        let text = parameters.text(&select, &shifted(lexed, "SELECT 1".len() as isize - cut as isize, cut), As::Values).sql;
        (host.query(&format!("{AST}{text}"), Vec::new(), Scope::Statement), host.query(&format!("{DESCRIBE}{text}\n)"), Vec::new(), Scope::Statement))
    });
    if let Err(e) = parsed? {
        return refuse(vec![server_error(sql, lexed, parameters, asked, AST, &e)]);
    }
    if let Some(built) = built {
        if let Err(e) = built? {
            return refuse(vec![server_error(sql, lexed, parameters, asked, PIPELINE, &e)]);
        }
    }
    if let Some((reads, described)) = deleted {
        // Where the text is no SELECT, as with ON CLUSTER or IN PARTITION, nothing more is asked.
        if let (Ok(_), Err(e)) = (reads?, described?) {
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
    fn the_describe_is_asked_with_each_parameter_as_a_value_that_is_no_constant() {
        let asked = std::cell::RefCell::new(Vec::new());
        let sql = "SELECT quantile({level:Float64})(age) AS q FROM users WHERE name = {name:String} -- why\n LIMIT {skip:UInt64}, {count:UInt64};\n";
        run(sql, &|sent| {
            asked.borrow_mut().push(sent.to_string());
            server(&[("q", "Float64")], "QUERY id: 0")(sent)
        });
        let describe = asked.borrow().iter().find(|s| s.starts_with(DESCRIBE)).cloned().unwrap();
        assert_eq!(
            describe,
            "DESCRIBE (\nSELECT quantile(CAST(1, 'Float64'))(age) AS q FROM users WHERE name = materialize(defaultValueOfTypeName('String')) -- why\n LIMIT CAST(1, 'UInt64'), CAST(1, 'UInt64')\n)"
        );
    }

    #[test]
    fn a_parameter_written_with_a_cast_is_sent_the_server_s_way() {
        let a = run("SELECT id FROM users WHERE id = :id::UInt64 AND name IN :names::Array(String)", &server(&[("id", "UInt64")], "QUERY id: 0"));
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(a.rewritten.sql, "SELECT id FROM users WHERE id = {id:UInt64}::UInt64 AND name IN {names:Array(String)}::Array(String)");
        assert_eq!(a.params.iter().map(|p| p.ty.kind.name()).collect::<Vec<_>>(), ["integer", "array"]);
    }

    #[test]
    fn a_parameter_may_repeat_with_one_type_and_no_other() {
        let a = run("SELECT {n:UInt8} AS a, {n:UInt8} AS b", &server(&[("a", "UInt8"), ("b", "UInt8")], "QUERY id: 0"));
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(a.params.len(), 1);
        let b = run("SELECT {n:UInt8} AS a, {n:String} AS b", &server(&[], "QUERY id: 0"));
        assert_eq!(b.diagnostics.iter().map(|d| (d.message.as_str(), d.code)).collect::<Vec<_>>(), [("The parameter n is a String here and a UInt8 at its first use: declare it with one type.", 3)]);
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

    #[test]
    fn a_parameter_the_server_reads_as_a_constant_is_found_and_refused() {
        let sql = "SELECT toDateTime64(created, {scale:UInt8}) AS t FROM users WHERE id = {id:UInt64}";
        let a = run(sql, &|sent| {
            if !sent.starts_with(DESCRIBE) {
                return server(&[], "QUERY id: 0")(sent);
            }
            if sent.contains("toDateTime64(created, materialize(") {
                return Err(error("Code: 44. DB::Exception: A value of illegal type was provided as 2nd argument 'scale' to function 'toDateTime64'. Expected: const Integer, got: UInt8: In scope SELECT toDateTime64(created, materialize(defaultValueOfTypeName('UInt8'))) AS t. (ILLEGAL_COLUMN) (version 26.9.23.15 (official build))", "44"));
            }
            Ok(vec![vec![json!("t"), json!("DateTime64(1)")]])
        });
        assert_eq!(a.diagnostics.len(), 1);
        let d = &a.diagnostics[0];
        assert_eq!(&sql[d.start..d.start + d.length], "{scale:UInt8}");
        assert!(d.message.starts_with("ClickHouse reads {scale:UInt8} as a constant here"), "{}", d.message);
        assert!(d.message.contains("In scope SELECT toDateTime64(created, {scale:UInt8}) AS t"), "{}", d.message);
    }

    #[test]
    fn the_server_s_error_is_the_statement_s_at_the_place_it_names() {
        let sql = "SELECT id FROM users WHERE";
        let a = run(sql, &|sent| {
            if sent.starts_with(DESCRIBE) {
                Err(error("Code: 62. DB::Exception: Syntax error: failed at position 38 ()) (line 3, col 1): ). Expected one of: expression. (SYNTAX_ERROR) (version 26.9.23.15 (official build))", "62"))
            } else {
                server(&[], "QUERY id: 0")(sent)
            }
        });
        let d = &a.diagnostics[0];
        assert_eq!((d.start, d.code, d.server_code.as_deref()), (sql.len() - 1, 6, Some("62")));
        assert!(d.message.starts_with("Syntax error: failed at position 38"), "{}", d.message);
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
        let unnamed = run("SELECT id, age + 1 FROM users", &server(&[("id", "UInt64"), ("plus(age, 1)", "UInt16")], "QUERY id: 0"));
        assert_eq!(unnamed.diagnostics.len(), 1);
        assert_eq!(unnamed.diagnostics[0].code, analysis::UNNAMED_COLUMN);
    }

    #[test]
    fn a_type_the_run_time_does_not_read_is_refused_with_what_to_write() {
        let a = run("SELECT doc FROM events", &server(&[("doc", "JSON")], "QUERY id: 0"));
        assert_eq!(a.diagnostics[0].message, "Column 1 (doc) is a JSON, and Reylo does not read a ClickHouse JSON yet: select it as a type it reads, as toString(doc) or toJSONString(doc) does.");
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
