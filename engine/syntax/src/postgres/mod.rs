//! Postgres's syntax: its own grammar (`gram.y`), through tables Bison generated from it with its C
//! actions removed (`gen/postgres.py`), and a port of its scanner (`scan.l`).

#[path = "generated/tokens.rs"]
pub mod generated;
#[path = "generated/names.rs"]
pub mod generated_names;
#[path = "generated/pb.rs"]
pub mod pb;
#[path = "generated/actions.rs"]
mod actions_table;
#[path = "generated/deparse.rs"]
mod deparse_generated;
mod deparse_rt;
mod helpers;
pub mod nodes;
pub mod plpgsql;
pub mod scan;
#[path = "generated/walk.rs"]
pub mod walk;

use crate::lalr::{self, Actions, Lexeme, NoActions, Tables};
use crate::tree::{SyntaxError, Tree};
use generated::{FINAL, LAST, NAMES, NTOKENS, PACT_NINF, SEGMENTS, TABLE_NINF};
use std::sync::OnceLock;

static TABLE_BYTES: &[u8] = include_bytes!("generated/tables.bin");

/// The grammar's tables.
pub fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| Tables::read(TABLE_BYTES, &SEGMENTS, PACT_NINF, TABLE_NINF, FINAL, LAST, NTOKENS))
}

pub use pb as protobuf;
pub use pb::node::Node as NodeEnum;
pub use pb::Node;
pub use walk::{Context, NodeRef};

/// How deep a tree [`parse`] reads, in messages.
const PARSE_DEPTH: u32 = 1_000;

/// Why a statement has no tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The grammar or the scanner refused it, in Postgres's words.
    Parse(String),
    /// The scanner refused it.
    Scan(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Parse(m) => write!(f, "Invalid statement: {m}"),
            Error::Scan(m) => write!(f, "Error scanning: {m}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = core::result::Result<T, Error>;

/// A statement's text read as Postgres reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult {
    pub protobuf: pb::ParseResult,
}

/// How to print a tree. Nothing to set: the one form is the deparser's default.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeparseOptions;

/// The tree of `sql`, as Postgres's parser builds it.
pub fn parse(sql: &str, _options: i32) -> Result<ParseResult> {
    on_deep_stack(|| parse_at_most(sql, PARSE_DEPTH).map(|protobuf| ParseResult { protobuf }))
}

/// Runs `f` with room for the deepest tree [`parse`] returns, whatever thread calls it: reading a tree recurses once per level, and a caller may be on a small stack.
#[cfg(not(target_family = "wasm"))]
fn on_deep_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    const DEEP_STACK: usize = 256 << 20;
    std::thread::scope(|s| match std::thread::Builder::new().stack_size(DEEP_STACK).spawn_scoped(s, f).expect("a thread for the parser").join() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    })
}

/// WebAssembly has no threads here: the engine's own stack is the one there is.
#[cfg(target_family = "wasm")]
fn on_deep_stack<T>(f: impl FnOnce() -> T) -> T {
    f()
}

/// The tree of `sql`, nested at most `max_depth` messages deep.
pub fn parse_at_most(sql: &str, max_depth: u32) -> Result<pb::ParseResult> {
    match parse_statements(sql, max_depth) {
        Ok(stmts) => Ok(pb::ParseResult { version: generated::PG_VERSION_NUM, stmts }),
        Err(ParseError::Syntax(message)) => Err(Error::Parse(message)),
        Err(ParseError::TooDeep) => Err(Error::Parse("parse tree is nested too deeply to read".to_string())),
    }
}

impl pb::ParseResult {
    /// The tree as SQL.
    pub fn deparse(&self, _: DeparseOptions) -> Result<String> {
        on_deep_stack(|| deparse_statements(&self.stmts).map_err(Error::Parse))
    }
}

impl pb::Node {
    /// A node as SQL, as a statement of its own.
    pub fn deparse(&self, _: DeparseOptions) -> Result<String> {
        on_deep_stack(|| deparse_statements(&[pb::RawStmt { stmt: Some(Box::new(self.clone())), stmt_location: 0, stmt_len: 0 }]).map_err(Error::Parse))
    }
}

/// A tree as SQL, as libpg_query's deparser writes it with its default options.
pub fn deparse(tree: &pb::ParseResult, _: DeparseOptions) -> Result<String> {
    on_deep_stack(|| deparse_statements(&tree.stmts).map_err(Error::Parse))
}

/// The tokens of `sql`, comments included, numbered as the tree's `Token` enum numbers them.
pub fn scan_tokens(sql: &str) -> Result<pb::ScanResult> {
    let tokens = scan::scan(sql).map_err(|(_, stop)| Error::Scan(stop.message))?;
    let tokens = tokens
        .iter()
        .map(|t| {
            let name = kind_name(t.kind);
            let token = match name.as_bytes() {
                [b'\'', c, b'\''] => pb::Token::from_str_name(&format!("ASCII_{c}")).map_or(*c as i32, |x| x as i32),
                _ => pb::Token::from_str_name(name).map_or(0, |x| x as i32),
            };
            let keyword_kind = match &t.value {
                scan::Value::Keyword(k) => scan::keyword(k.as_bytes()).map_or(0, |e| e.2 as i32 + 1),
                _ => 0,
            };
            pb::ScanToken { start: t.start as i32, end: t.end as i32, token, keyword_kind }
        })
        .collect();
    Ok(pb::ScanResult { version: generated::PG_VERSION_NUM, tokens })
}

/// The name of a token or grammar rule.
/// Postgres's keywords, in lower case.
pub fn keywords() -> impl Iterator<Item = &'static str> {
    generated::KEYWORDS.iter().map(|k| k.0)
}

pub fn kind_name(kind: u16) -> &'static str {
    if kind == crate::tree::ERROR_KIND {
        return "error";
    }
    NAMES.get(kind as usize).copied().unwrap_or("?")
}

/// The tree of `text`, whose comments and whitespace are trivia on the token after them. What the
/// scanner or the grammar refuses is under error nodes, and listed in the tree's `errors`.
pub fn parse_tree(text: &str) -> Tree<'_> {
    let (lexemes, tail, scan_error) = match scan::lexemes(text) {
        Ok((lexemes, tail)) => (lexemes, tail, None),
        Err(e) => {
            // Everything from where the scanner stopped is one token that no rule takes.
            let (mut lexemes, lead) = e.read;
            let start = e.at.max(lead);
            let end = text.len() as u32;
            lexemes.push(Lexeme { sym: lalr::SCAN_STOP, lead, start, end: end.max(start) });
            let tail = lexemes.last().map_or(0, |l| l.end);
            (lexemes, tail, Some(SyntaxError { message: e.message, start, end }))
        }
    };
    let mut parsed = lalr::parse(tables(), text, &lexemes, tail, tok_semicolon(), statement_list(), &mut NoActions);
    for e in &mut parsed.tree.errors {
        e.message = syntax_message(text, e.start, e.end);
    }
    if let (Some(message), Some(e)) = (parsed.refused.take(), parsed.tree.errors.first_mut()) {
        e.message = message;
    }
    if let Some(e) = scan_error {
        parsed.tree.errors.retain(|o| o.start != e.start);
        parsed.tree.errors.push(e);
    }
    if parsed.tree.errors.is_empty() {
        parsed.tree.errors.extend(refusal(text));
    }
    parsed.tree
}

/// What a grammar action refuses of text the grammar's rules take, such as `FETCH FIRST ROW WITH TIES` with no ORDER BY.
fn refusal(text: &str) -> Option<SyntaxError> {
    let (read, tail) = scan::read(text).ok()?;
    let lexemes: Vec<Lexeme> = read.iter().map(|r| Lexeme { sym: r.lexeme.0, lead: r.lexeme.1, start: r.lexeme.2, end: r.lexeme.3 }).collect();
    helpers::reset_extra();
    let parsed = lalr::parse(tables(), text, &lexemes, tail, tok_semicolon(), statement_list(), &mut TreeActions { text, read: &read });
    helpers::reset_extra();
    let message = parsed.refused?;
    let (start, end) = parsed.refused_at.unwrap_or((0, text.len() as u32));
    Some(SyntaxError { message, start, end })
}

fn tok_semicolon() -> u16 {
    generated::CHAR_TOKENS[b';' as usize]
}

/// The grammar's symbol for a list of statements.
fn statement_list() -> u16 {
    static LIST: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
    *LIST.get_or_init(|| (0..u16::MAX).find(|k| kind_name(*k) == "stmtmulti").expect("the grammar names its statement list"))
}

/// Postgres's words for a syntax error at a token.
pub fn syntax_message(text: &str, start: u32, end: u32) -> String {
    if start as usize >= text.len() {
        "syntax error at end of input".to_string()
    } else {
        format!("syntax error at or near \"{}\"", &text[start as usize..end as usize])
    }
}

/// The tree was nested deeper than a caller's bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deep;

/// How deep a conversion to [`pb`] has gone.
pub struct Depth {
    pub nodes: u32,
    pub messages: u32,
    pub max: u32,
}

impl Depth {
    pub fn new(max: u32) -> Depth {
        Depth { nodes: 0, messages: 0, max }
    }

    pub fn enter(&mut self) -> core::result::Result<(), Deep> {
        self.messages += 1;
        if self.messages >= self.max {
            return Err(Deep);
        }
        Ok(())
    }

    pub fn leave(&mut self) {
        self.messages -= 1;
    }
}

#[allow(dead_code, non_snake_case, clippy::all)]
pub mod convert {
    use super::generated_names::{f, t};
    use super::nodes::{Obj, V};
    use super::pb;
    use super::{Deep, Depth};
    include!("generated/convert.rs");
}

#[allow(dead_code, non_snake_case, clippy::all)]
mod read {
    use super::generated_names::{f, t};
    use super::nodes::V;
    use super::pb;
    include!("generated/read.rs");
}

/// The statements as SQL, as libpg_query's deparser writes them with its default options,
/// `; ` between them.
pub fn deparse_statements(statements: &[pb::RawStmt]) -> core::result::Result<String, String> {
    let mut out = Vec::new();
    for (i, statement) in statements.iter().enumerate() {
        deparse_rt::raw_stmt(&read::raw_stmt(statement), deparse_generated::deparseStmt, &mut out)?;
        if i + 1 != statements.len() {
            out.extend_from_slice(b"; ");
        }
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// The grammar's actions over the scanner's values.
struct TreeActions<'r> {
    text: &'r str,
    read: &'r [scan::Read],
}

impl Actions for TreeActions<'_> {
    type Value = nodes::V;

    fn token(&mut self, index: usize) -> nodes::V {
        match &self.read[index].value {
            scan::Value::None => nodes::V::Null,
            scan::Value::Bytes(b) => nodes::V::bytes(b),
            scan::Value::Int(i) => nodes::V::Int(*i as i64),
            scan::Value::Keyword(k) => nodes::V::s(k),
        }
    }

    fn reduce(&mut self, rule: usize, values: &mut [nodes::V], locations: &[i32], here: i32, last_read: Option<usize>) -> core::result::Result<nodes::V, String> {
        actions_table::ACTIONS[rule](values, locations, here).map_err(|message| match message.strip_prefix(helpers::AT_TOKEN) {
            Some(words) => self.at_token(words, last_read),
            None => message,
        })
    }

    fn exhausted(&mut self, last_read: Option<usize>) -> String {
        self.at_token("memory exhausted", last_read)
    }
}

impl TreeActions<'_> {
    /// `scanner_yyerror`: the words, then the last token read.
    fn at_token(&self, words: &str, last_read: Option<usize>) -> String {
        match last_read.and_then(|i| self.read.get(i)) {
            Some(r) => format!("{words} at or near \"{}\"", &self.text[r.lexeme.2 as usize..r.lexeme.3 as usize]),
            None => format!("{words} at end of input"),
        }
    }
}

/// How a statement's text failed to give a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The scanner, the grammar or an action refused it, in Postgres's words.
    Syntax(String),
    /// It nests deeper than the bound.
    TooDeep,
}

/// The statements of `text`, as the structures Postgres's parser builds, nested at most `max_depth` messages deep.
pub fn parse_statements(text: &str, max_depth: u32) -> core::result::Result<Vec<pb::RawStmt>, ParseError> {
    let list = parse_values(text)?;
    convert::statements(&list, max_depth).map_err(|_| ParseError::TooDeep)
}

/// [`parse_statements`]' list of `RawStmt` nodes, before they become [`pb`]'s structs.
pub fn parse_values(text: &str) -> core::result::Result<nodes::V, ParseError> {
    let (read, tail, stop) = match scan::read(text) {
        Ok((read, tail)) => (read, tail, None),
        Err((read, lead, stop)) => (read, lead, Some(stop)),
    };
    let mut lexemes: Vec<Lexeme> = read.iter().map(|r| Lexeme { sym: r.lexeme.0, lead: r.lexeme.1, start: r.lexeme.2, end: r.lexeme.3 }).collect();
    if stop.is_some() {
        lexemes.push(Lexeme { sym: lalr::SCAN_STOP, lead: tail, start: tail, end: tail });
    }
    helpers::reset_extra();
    let parsed = lalr::parse(tables(), text, &lexemes, tail, tok_semicolon(), statement_list(), &mut TreeActions { text, read: &read });
    if parsed.scan_stopped {
        if let Some(stop) = stop {
            return Err(ParseError::Syntax(stop.message));
        }
    }
    // An action that refused ran before any later token could be a syntax error.
    if let Some(message) = parsed.refused {
        return Err(ParseError::Syntax(message));
    }
    if let Some(e) = parsed.tree.errors.first() {
        return Err(ParseError::Syntax(syntax_message(text, e.start, e.end)));
    }
    let list = helpers::parsetree();
    helpers::reset_extra();
    Ok(list)
}
