//! A dialect's lexer and a structural reading over it, for the databases whose grammar Reylo doesn't generate from: MySQL's is GPL, and ClickHouse and SQL Server publish none in a form to generate from. It reads statements, clauses and parenthesized groups, and takes any text: what it reports is an unbalanced parenthesis or a token that doesn't end, never that a statement is invalid, which is the database's to say.

use crate::tree::{span, Node, NodeId, SyntaxError, Tree};
use std::collections::HashMap;

pub const ROOT: u16 = 0;
pub const STATEMENT: u16 = 1;
pub const CLAUSE: u16 = 2;
pub const GROUP: u16 = 3;
pub const WORD: u16 = 4;
pub const KEYWORD: u16 = 5;
pub const QUOTED: u16 = 6;
pub const STRING: u16 = 7;
pub const NUMBER: u16 = 8;
pub const VARIABLE: u16 = 9;
pub const OPERATOR: u16 = 10;
pub const LPAREN: u16 = 11;
pub const RPAREN: u16 = 12;
pub const COMMA: u16 = 13;
pub const DOT: u16 = 14;
pub const SEMI: u16 = 15;
pub const ILLEGAL: u16 = 16;
const SPACE: u16 = 17;
/// `/*!50003` or the `*/` that closes it: MySQL runs the text between them as SQL.
pub const CONDITIONAL: u16 = 18;

static NAMES: [&str; 19] = ["root", "statement", "clause", "group", "word", "keyword", "quoted identifier", "string", "number", "variable", "operator", "(", ")", ",", ".", ";", "illegal", "space", "conditional comment"];

/// The name of a node's kind.
pub fn kind_name(kind: u16) -> &'static str {
    if kind == crate::tree::ERROR_KIND {
        return "error";
    }
    NAMES.get(kind as usize).copied().unwrap_or("?")
}

/// Which database's lexical rules to read with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    /// `backslash_escapes` is off under `NO_BACKSLASH_ESCAPES`, and `ansi_quotes` makes `"` quote an identifier.
    Mysql { backslash_escapes: bool, ansi_quotes: bool },
    ClickHouse,
    SqlServer,
}

impl Flavor {
    /// MySQL under its default `sql_mode`.
    pub const MYSQL: Flavor = Flavor::Mysql { backslash_escapes: true, ansi_quotes: false };
}

/// The words that open a clause, in capitals and sorted, for every flavor.
const CLAUSES: &[&str] = &["DELETE", "EXCEPT", "FETCH", "FOR", "FROM", "GROUP", "HAVING", "INSERT", "INTERSECT", "INTO", "JOIN", "LIMIT", "MERGE", "OFFSET", "ON", "ORDER", "REPLACE", "RETURNING", "SELECT", "SET", "UNION", "UPDATE", "USING", "VALUES", "WHERE", "WINDOW", "WITH"];
const CLICKHOUSE_CLAUSES: &[&str] = &["ARRAY", "FINAL", "FORMAT", "PREWHERE", "SAMPLE", "SETTINGS"];
const SQLSERVER_CLAUSES: &[&str] = &["OPTION", "OUTPUT", "PIVOT", "TOP", "UNPIVOT"];

/// Words to offer as keywords where a dialect has no list of its own in this crate.
pub const KEYWORDS: &[&str] = &["ALL", "ALTER", "AND", "AS", "ASC", "BEGIN", "BETWEEN", "BY", "CASE", "CAST", "COMMIT", "CREATE", "CROSS", "DEFAULT", "DELETE", "DESC", "DISTINCT", "DROP", "ELSE", "END", "EXCEPT", "EXISTS", "FALSE", "FETCH", "FOR", "FOREIGN", "FROM", "FULL", "GROUP", "HAVING", "IN", "INDEX", "INNER", "INSERT", "INTERSECT", "INTO", "IS", "JOIN", "KEY", "LEFT", "LIKE", "LIMIT", "MERGE", "NOT", "NULL", "OFFSET", "ON", "OR", "ORDER", "OUTER", "OVER", "PARTITION", "PRIMARY", "REFERENCES", "REPLACE", "RETURNING", "RIGHT", "ROLLBACK", "SELECT", "SET", "TABLE", "THEN", "TRUE", "UNION", "UNIQUE", "UPDATE", "USING", "VALUES", "VIEW", "WHEN", "WHERE", "WINDOW", "WITH"];

fn is_clause(flavor: Flavor, upper: &str) -> bool {
    CLAUSES.binary_search(&upper).is_ok()
        || match flavor {
            Flavor::ClickHouse => CLICKHOUSE_CLAUSES.contains(&upper),
            Flavor::SqlServer => SQLSERVER_CLAUSES.contains(&upper),
            Flavor::Mysql { .. } => false,
        }
}

#[inline]
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

#[inline]
fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

/// The length of a quoted run at `at`, its doubled quotes and (where `backslash`) its escapes read, or `None` where it doesn't end.
fn quoted(z: &[u8], at: usize, close: u8, backslash: bool) -> Option<usize> {
    let mut i = at + 1;
    while i < z.len() {
        let c = z[i];
        if backslash && c == b'\\' {
            i += 2;
        } else if c == close {
            if z.get(i + 1) == Some(&close) {
                i += 2;
            } else {
                return Some(i + 1 - at);
            }
        } else {
            i += 1;
        }
    }
    None
}

/// What's left of the text as one token that doesn't end.
fn unterminated(z: &[u8], at: usize) -> (usize, u16) {
    (z.len() - at, ILLEGAL)
}

fn string(z: &[u8], at: usize, prefix: usize, close: u8, backslash: bool, kind: u16) -> (usize, u16) {
    match quoted(z, at + prefix, close, backslash) {
        Some(n) => (prefix + n, kind),
        None => unterminated(z, at),
    }
}

fn line_comment(z: &[u8], at: usize) -> (usize, u16) {
    (z[at..].iter().position(|c| *c == b'\n').unwrap_or(z.len() - at), SPACE)
}

fn block_comment(z: &[u8], at: usize, nested: bool) -> (usize, u16) {
    let (mut i, mut depth) = (at + 2, 1);
    while i + 1 < z.len() {
        if z[i] == b'*' && z[i + 1] == b'/' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return (i - at, SPACE);
            }
        } else if nested && z[i] == b'/' && z[i + 1] == b'*' {
            depth += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    unterminated(z, at)
}

/// A number at `at`, or a word where letters run on from its digits, as `1a` does in MySQL.
fn number(z: &[u8], at: usize) -> (usize, u16) {
    let b = |i: usize| z.get(i).copied().unwrap_or(0);
    let mut i = at;
    if b(i) == b'0' && matches!(b(i + 1), b'x' | b'X') && b(i + 2).is_ascii_hexdigit() {
        i += 2;
        while b(i).is_ascii_hexdigit() {
            i += 1;
        }
    } else if b(i) == b'0' && matches!(b(i + 1), b'b' | b'B') && matches!(b(i + 2), b'0' | b'1') {
        i += 2;
        while matches!(b(i), b'0' | b'1') {
            i += 1;
        }
    } else {
        while b(i).is_ascii_digit() {
            i += 1;
        }
        if b(i) == b'.' {
            i += 1;
            while b(i).is_ascii_digit() {
                i += 1;
            }
        }
        if matches!(b(i), b'e' | b'E') && (b(i + 1).is_ascii_digit() || (matches!(b(i + 1), b'+' | b'-') && b(i + 2).is_ascii_digit())) {
            i += 2;
            while b(i).is_ascii_digit() {
                i += 1;
            }
        }
    }
    if is_word(b(i)) {
        while is_word(b(i)) {
            i += 1;
        }
        return (i - at, WORD);
    }
    (i - at, NUMBER)
}

fn word(z: &[u8], at: usize) -> (usize, u16) {
    (z[at..].iter().position(|c| !is_word(*c)).unwrap_or(z.len() - at), WORD)
}

/// The longest operator at `at`.
fn operator(z: &[u8], at: usize, flavor: Flavor) -> (usize, u16) {
    const LONG: &[&str] = &["<=>", "->>", "<<", ">>", "<=", ">=", "<>", "!=", "||", "&&", "->", ":=", "::"];
    const COMPOUND: &[&str] = &["+=", "-=", "*=", "/=", "%=", "&=", "^=", "|=", "!<", "!>"];
    let rest = &z[at..];
    if LONG.iter().any(|o| rest.starts_with(o.as_bytes()) && o.len() == 3) {
        return (3, OPERATOR);
    }
    if LONG.iter().any(|o| rest.starts_with(o.as_bytes())) || (flavor == Flavor::SqlServer && COMPOUND.iter().any(|o| rest.starts_with(o.as_bytes()))) {
        return (2, OPERATOR);
    }
    match z[at] {
        b'+' | b'-' | b'*' | b'/' | b'%' | b'<' | b'>' | b'=' | b'!' | b'&' | b'|' | b'^' | b'~' | b':' | b'[' | b']' | b'{' | b'}' | b'?' => (1, OPERATOR),
        c => (utf8_len(c), ILLEGAL),
    }
}

fn utf8_len(c: u8) -> usize {
    match c {
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xff => 4,
        _ => 1,
    }
}

/// `@name`, `@@name`, or `@` before a quoted name, as MySQL and SQL Server write variables.
fn at_variable(z: &[u8], at: usize, quotes: &[u8], backslash: bool) -> (usize, u16) {
    let mut i = at + 1;
    if z.get(i) == Some(&b'@') {
        i += 1;
    }
    match z.get(i) {
        Some(q) if quotes.contains(q) => match quoted(z, i, *q, backslash && *q != b'`') {
            Some(n) => (i + n - at, VARIABLE),
            None => unterminated(z, at),
        },
        _ => {
            while z.get(i).is_some_and(|c| is_word(*c) || *c == b'.' || *c == b'#' || *c == b'@') {
                i += 1;
            }
            (i - at, VARIABLE)
        }
    }
}

/// `:name`, Reylo's placeholder, where a word follows the colon.
fn colon_name(z: &[u8], at: usize) -> Option<(usize, u16)> {
    let next = *z.get(at + 1)?;
    if !(next.is_ascii_alphabetic() || next == b'_' || next >= 0x80) {
        return None;
    }
    Some((1 + word(z, at + 1).0, VARIABLE))
}

/// What a lexer looks ahead for, found once for the whole text, so that a text full of openers with no closer stays linear.
#[derive(Default)]
struct Ahead<'z> {
    /// Where each `$tag$` is written.
    dollars: Option<HashMap<&'z [u8], Vec<usize>>>,
    /// The first `}` at or after some place already asked about, or `usize::MAX` where none follows.
    brace: Option<usize>,
}

impl Ahead<'_> {
    fn next_brace(&mut self, z: &[u8], at: usize) -> Option<usize> {
        let known = self.brace.filter(|b| *b >= at);
        let found = known.unwrap_or_else(|| z[at..].iter().position(|c| *c == b'}').map_or(usize::MAX, |p| at + p));
        self.brace = Some(found);
        (found != usize::MAX).then_some(found)
    }
}

/// Every `$tag$` of the text, by its tag, with where each starts.
fn dollar_tags(z: &[u8]) -> HashMap<&[u8], Vec<usize>> {
    let mut out: HashMap<&[u8], Vec<usize>> = HashMap::new();
    let mut i = 0;
    while i < z.len() {
        if z[i] != b'$' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while z.get(j).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
            j += 1;
        }
        if z.get(j) == Some(&b'$') {
            out.entry(&z[i..=j]).or_default().push(i);
        }
        i = j;
    }
    out
}

fn token<'z>(flavor: Flavor, z: &'z [u8], at: usize, ahead: &mut Ahead<'z>) -> (usize, u16) {
    let c = z[at];
    let next = z.get(at + 1).copied().unwrap_or(0);
    // SQL Server reads every control character as whitespace.
    let space = |c: u8| is_space(c) || (flavor == Flavor::SqlServer && c <= b' ');
    if space(c) {
        return (z[at..].iter().position(|c| !space(*c)).unwrap_or(z.len() - at), SPACE);
    }
    match c {
        b'(' => return (1, LPAREN),
        b')' => return (1, RPAREN),
        b',' => return (1, COMMA),
        b';' => return (1, SEMI),
        b'.' if !next.is_ascii_digit() => return (1, DOT),
        b'.' | b'0'..=b'9' => return number(z, at),
        _ => {}
    }
    match flavor {
        Flavor::Mysql { backslash_escapes, ansi_quotes } => match c {
            // `--` opens a comment only before whitespace or a control character.
            b'-' if next == b'-' && z.get(at + 2).is_none_or(|d| *d <= b' ') => line_comment(z, at),
            b'#' => line_comment(z, at),
            b'/' if next == b'*' => block_comment(z, at, false),
            // `\N` is NULL.
            b'\\' if next == b'N' => (2, WORD),
            b'$' => heredoc(z, at, ahead).unwrap_or_else(|| word(z, at)),
            b'\'' => string(z, at, 0, b'\'', backslash_escapes, STRING),
            b'"' => string(z, at, 0, b'"', backslash_escapes && !ansi_quotes, if ansi_quotes { QUOTED } else { STRING }),
            b'`' => string(z, at, 0, b'`', false, QUOTED),
            b'x' | b'X' | b'b' | b'B' | b'n' | b'N' if next == b'\'' => string(z, at, 1, b'\'', backslash_escapes, STRING),
            b'@' => at_variable(z, at, b"'\"`", backslash_escapes),
            b':' if next != b'=' && next != b':' => colon_name(z, at).unwrap_or((1, OPERATOR)),
            _ if is_word(c) => word(z, at),
            _ => operator(z, at, flavor),
        },
        Flavor::ClickHouse => match c {
            b'-' if next == b'-' => line_comment(z, at),
            b'#' if next == b'!' || next == b' ' => line_comment(z, at),
            b'/' if next == b'*' => block_comment(z, at, false),
            b'\'' => string(z, at, 0, b'\'', true, STRING),
            b'"' => string(z, at, 0, b'"', true, QUOTED),
            b'`' => string(z, at, 0, b'`', true, QUOTED),
            b'$' => heredoc(z, at, ahead).unwrap_or_else(|| word(z, at)),
            b'{' => parameter(z, at, ahead).unwrap_or((1, OPERATOR)),
            // `json.@path`, `user@'host'` and `@@setting`.
            b'@' => at_variable(z, at, b"'\"`", true),
            b':' if next != b':' && next != b'=' => colon_name(z, at).unwrap_or((1, OPERATOR)),
            _ if is_word(c) => word(z, at),
            _ => operator(z, at, flavor),
        },
        Flavor::SqlServer => match c {
            b'-' if next == b'-' => line_comment(z, at),
            b'/' if next == b'*' => block_comment(z, at, true),
            b'\'' => string(z, at, 0, b'\'', false, STRING),
            b'"' => string(z, at, 0, b'"', false, QUOTED),
            b'[' => string(z, at, 0, b']', false, QUOTED),
            b'n' | b'N' if next == b'\'' => string(z, at, 1, b'\'', false, STRING),
            b'@' => at_variable(z, at, b"", false),
            b'#' => {
                let mut i = at + 1;
                while z.get(i).is_some_and(|c| is_word(*c) || *c == b'#' || *c == b'@') {
                    i += 1;
                }
                (i - at, WORD)
            }
            // `$1.50` is money; `$action` and `$identity` are words.
            b'$' if next.is_ascii_digit() || next == b'.' => {
                let (n, _) = number(z, at + 1);
                (1 + n, NUMBER)
            }
            b':' if next != b':' => colon_name(z, at).unwrap_or((1, OPERATOR)),
            _ if is_word(c) => word(z, at),
            _ => operator(z, at, flavor),
        },
    }
}

/// `$tag$ … $tag$`, a dollar-quoted string. With no closing tag it is an identifier, which `$` may start and run through.
fn heredoc<'z>(z: &'z [u8], at: usize, ahead: &mut Ahead<'z>) -> Option<(usize, u16)> {
    let mut i = at + 1;
    while z.get(i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_') {
        i += 1;
    }
    if z.get(i) != Some(&b'$') {
        return None;
    }
    let tag = &z[at..=i];
    let places = ahead.dollars.get_or_insert_with(|| dollar_tags(z)).get(tag)?;
    let close = *places.get(places.partition_point(|p| *p <= i))?;
    Some((close + tag.len() - at, STRING))
}

/// `{name:Type}`, ClickHouse's query parameter.
fn parameter(z: &[u8], at: usize, ahead: &mut Ahead<'_>) -> Option<(usize, u16)> {
    let (n, _) = word(z, at + 1);
    if n == 0 || z.get(at + 1 + n) != Some(&b':') {
        return None;
    }
    Some((ahead.next_brace(z, at)? + 1 - at, VARIABLE))
}

struct Lexeme {
    kind: u16,
    lead: u32,
    start: u32,
    end: u32,
}

fn lexemes(flavor: Flavor, text: &str) -> (Vec<Lexeme>, u32) {
    let z = text.as_bytes();
    let mut out: Vec<Lexeme> = Vec::with_capacity(z.len() / 4 + 1);
    let (mut at, mut lead) = (0, 0);
    let mut ahead = Ahead::default();
    // Inside `/*! … */`, whose text MySQL runs as SQL: the markers are trivia, and what's between them tokens.
    let mut conditional = false;
    while at < z.len() {
        if matches!(flavor, Flavor::Mysql { .. }) {
            if conditional && z[at..].starts_with(b"*/") {
                conditional = false;
                out.push(Lexeme { kind: CONDITIONAL, lead: lead as u32, start: at as u32, end: at as u32 + 2 });
                at += 2;
                lead = at;
                continue;
            }
            if !conditional && z[at..].starts_with(b"/*!") && z[at..].windows(2).any(|w| w == b"*/") {
                conditional = true;
                let start = at;
                at += 3;
                while z.get(at).is_some_and(u8::is_ascii_digit) {
                    at += 1;
                }
                out.push(Lexeme { kind: CONDITIONAL, lead: lead as u32, start: start as u32, end: at as u32 });
                lead = at;
                continue;
            }
        }
        let (mut n, mut kind) = token(flavor, z, at, &mut ahead);
        // A colon right after a word is a label's (`outer:BEGIN`), not a placeholder's.
        if kind == VARIABLE && z[at] == b':' && out.last().is_some_and(|l| l.end as usize == at && matches!(l.kind, WORD | KEYWORD)) {
            (n, kind) = (1, OPERATOR);
        }
        // A token never stops inside a character, whatever its rule read past.
        n = n.max(1).min(z.len() - at);
        while !text.is_char_boundary(at + n) {
            n += 1;
        }
        if kind == SPACE {
            at += n;
            continue;
        }
        if kind == WORD {
            let after_dot = out.iter().rev().find(|l| l.kind != CONDITIONAL).is_some_and(|l| l.kind == DOT);
            if !after_dot && n <= 9 && is_clause(flavor, &text[at..at + n].to_ascii_uppercase()) {
                kind = KEYWORD;
            }
            if flavor == Flavor::SqlServer {
                if let Some(whole) = go_length(z, at, n) {
                    (n, kind) = (whole, SEMI);
                }
            }
        }
        out.push(Lexeme { kind, lead: lead as u32, start: at as u32, end: (at + n) as u32 });
        at += n;
        lead = at;
    }
    (out, lead as u32)
}

/// The length of `GO` and its count, SQL Server's batch separator, where they stand alone on their line. Each scan stops at the first thing that rules it out, so a long line of words stays linear.
fn go_length(z: &[u8], at: usize, n: usize) -> Option<usize> {
    if n != 2 || !z[at..at + 2].eq_ignore_ascii_case(b"go") {
        return None;
    }
    if !z[..at].iter().rev().take_while(|c| **c != b'\n').all(|c| is_space(*c)) {
        return None;
    }
    let mut end = at + 2;
    let mut i = end;
    while let Some(&c) = z.get(i).filter(|c| **c != b'\n') {
        // A comment may follow it on its line.
        if z[i..].starts_with(b"--") {
            break;
        }
        if c.is_ascii_digit() {
            end = i + 1;
        } else if !is_space(c) {
            return None;
        }
        i += 1;
    }
    Some(end - at)
}

struct Frame {
    kind: u16,
    kids: Vec<NodeId>,
    clause: Vec<NodeId>,
    open: (u32, u32),
}

impl Frame {
    fn new(kind: u16, open: (u32, u32)) -> Frame {
        Frame { kind, kids: Vec::new(), clause: Vec::new(), open }
    }
}

struct Builder {
    nodes: Vec<Node>,
    child_ids: Vec<NodeId>,
}

impl Builder {
    fn inner(&mut self, kind: u16, kids: &[NodeId], at: u32) -> NodeId {
        let first = self.child_ids.len() as u32;
        let (start, end) = span(&self.nodes, kids, at);
        self.child_ids.extend_from_slice(kids);
        self.nodes.push(Node { kind, start, end, lead: first, children: kids.len() as u32, leaf: false });
        self.nodes.len() as NodeId - 1
    }

    fn close_clause(&mut self, f: &mut Frame) {
        if !f.clause.is_empty() {
            let clause = std::mem::take(&mut f.clause);
            let at = self.nodes[clause[0] as usize].start;
            let id = self.inner(CLAUSE, &clause, at);
            f.kids.push(id);
        }
    }

    fn close(&mut self, mut f: Frame) -> NodeId {
        self.close_clause(&mut f);
        let at = f.open.0;
        self.inner(f.kind, &f.kids, at)
    }
}

/// The token after `i`, past the markers of a conditional comment, which stand between no two words of the SQL.
fn next_code(tokens: &[Lexeme], i: usize) -> Option<&Lexeme> {
    tokens[i + 1..].iter().find(|l| l.kind != CONDITIONAL)
}

/// The token before `i`, past those markers.
fn previous_code(tokens: &[Lexeme], i: usize) -> Option<&Lexeme> {
    tokens[..i].iter().rev().find(|l| l.kind != CONDITIONAL)
}

/// The next word's text after token `i`, in capitals, if the next token is one.
fn next_word(tokens: &[Lexeme], text: &str, i: usize) -> Option<String> {
    let n = next_code(tokens, i)?;
    matches!(n.kind, WORD | KEYWORD).then(|| text[n.start as usize..n.end as usize].to_ascii_uppercase())
}

/// Reads `text` into a tree of statements, clauses and parenthesized groups.
pub fn parse(flavor: Flavor, text: &str) -> Tree<'_> {
    let (tokens, tail) = lexemes(flavor, text);
    let mut b = Builder { nodes: Vec::with_capacity(tokens.len() * 2), child_ids: Vec::with_capacity(tokens.len() * 2) };
    let mut errors: Vec<SyntaxError> = Vec::new();
    let mut statements: Vec<NodeId> = Vec::new();
    let mut stack: Vec<Frame> = vec![Frame::new(STATEMENT, (0, 0))];
    // How many BEGIN … END blocks are open: a `;` inside one doesn't end the statement.
    let mut blocks = 0u32;
    let mut first_word: Option<String> = None;
    // Whether the statement creates a stored program, whose body is what BEGIN opens on MySQL.
    let mut stored = false;

    fn close_groups(b: &mut Builder, stack: &mut Vec<Frame>, errors: &mut Vec<SyntaxError>) {
        while stack.len() > 1 {
            let f = stack.pop().unwrap();
            errors.push(SyntaxError { message: "\"(\" is never closed".to_string(), start: f.open.0, end: f.open.1 });
            let id = b.close(f);
            stack.last_mut().unwrap().clause.push(id);
        }
    }

    for (i, l) in tokens.iter().enumerate() {
        b.nodes.push(Node { kind: l.kind, start: l.start, end: l.end, lead: l.lead, children: 0, leaf: true });
        let leaf = b.nodes.len() as NodeId - 1;
        let upper = matches!(l.kind, WORD | KEYWORD).then(|| text[l.start as usize..l.end as usize].to_ascii_uppercase());
        if first_word.is_none() && l.kind != CONDITIONAL {
            first_word = Some(upper.clone().unwrap_or_default());
        }
        if let Some(word) = upper.as_deref() {
            let after_dot = previous_code(&tokens, i).is_some_and(|p| p.kind == DOT);
            let before = previous_code(&tokens, i).filter(|p| matches!(p.kind, WORD | KEYWORD)).map(|p| &text[p.start as usize..p.end as usize]);
            if matches!(word, "PROCEDURE" | "FUNCTION" | "TRIGGER" | "EVENT") && first_word.as_deref() == Some("CREATE") {
                stored = true;
            }
            // A block is written at the statement's own level: in parentheses these words are names.
            if !after_dot && stack.len() == 1 {
                match (flavor, word) {
                    (Flavor::Mysql { .. }, "BEGIN") if stored || blocks > 0 => blocks += 1,
                    (Flavor::SqlServer, "BEGIN") if !matches!(next_word(&tokens, text, i).as_deref(), Some("TRAN" | "TRANSACTION" | "DISTRIBUTED" | "DIALOG" | "CONVERSATION")) => blocks += 1,
                    // `END CASE` closes a CASE, and opens nothing.
                    (Flavor::Mysql { .. } | Flavor::SqlServer, "CASE") if blocks > 0 && !before.is_some_and(|w| w.eq_ignore_ascii_case("end")) => blocks += 1,
                    (Flavor::Mysql { .. }, "END") if blocks > 0 && !matches!(next_word(&tokens, text, i).as_deref(), Some("IF" | "LOOP" | "WHILE" | "REPEAT")) => blocks -= 1,
                    (Flavor::SqlServer, "END") if blocks > 0 => blocks -= 1,
                    _ => {}
                }
            }
        }
        match l.kind {
            LPAREN => {
                let mut group = Frame::new(GROUP, (l.start, l.end));
                group.clause.push(leaf);
                stack.push(group);
            }
            RPAREN if stack.len() > 1 => {
                let mut group = stack.pop().unwrap();
                group.clause.push(leaf);
                let id = b.close(group);
                stack.last_mut().unwrap().clause.push(id);
            }
            RPAREN => {
                errors.push(SyntaxError { message: "\")\" with no \"(\" before it".to_string(), start: l.start, end: l.end });
                stack.last_mut().unwrap().clause.push(leaf);
            }
            SEMI if blocks == 0 || is_batch_end(flavor, text, l) => {
                blocks = 0;
                close_groups(&mut b, &mut stack, &mut errors);
                let mut statement = stack.pop().unwrap();
                statement.clause.push(leaf);
                statements.push(b.close(statement));
                stack.push(Frame::new(STATEMENT, (l.end, l.end)));
                first_word = None;
                stored = false;
            }
            KEYWORD => {
                // INSERT(…) and REPLACE(…) are functions.
                let call = matches!(upper.as_deref(), Some("INSERT" | "REPLACE")) && next_code(&tokens, i).is_some_and(|n| n.kind == LPAREN);
                let f = stack.last_mut().unwrap();
                let only_paren = f.clause.len() == 1 && b.nodes[f.clause[0] as usize].kind == LPAREN;
                if !call && !only_paren {
                    b.close_clause(f);
                }
                f.clause.push(leaf);
            }
            ILLEGAL => {
                let token = &text[l.start as usize..l.end as usize];
                let shown: String = token.chars().take(32).collect();
                errors.push(SyntaxError { message: format!("unrecognized token: \"{shown}\""), start: l.start, end: l.end });
                stack.last_mut().unwrap().clause.push(leaf);
            }
            _ => stack.last_mut().unwrap().clause.push(leaf),
        }
    }
    close_groups(&mut b, &mut stack, &mut errors);
    let last = stack.pop().unwrap();
    if !last.kids.is_empty() || !last.clause.is_empty() {
        statements.push(b.close(last));
    }
    let root = b.inner(ROOT, &statements, tail);
    errors.sort_by_key(|e| e.start);
    Tree { text, nodes: b.nodes, child_ids: b.child_ids, root, tail, errors }
}

/// SQL Server's `GO` ends a batch even inside an unfinished block.
fn is_batch_end(flavor: Flavor, text: &str, l: &Lexeme) -> bool {
    flavor == Flavor::SqlServer && text[l.start as usize..l.end as usize].get(..2).is_some_and(|t| t.eq_ignore_ascii_case("go"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Flavor; 4] = [Flavor::MYSQL, Flavor::Mysql { backslash_escapes: false, ansi_quotes: true }, Flavor::ClickHouse, Flavor::SqlServer];

    fn statements(flavor: Flavor, text: &str) -> usize {
        let t = parse(flavor, text);
        t.children(t.root).len()
    }

    #[test]
    fn prints_what_it_read() {
        for flavor in ALL {
            for text in ["", "  \n", "select 1", "select a from t; -- x\n", "select (", ")", "'open", "/* open", "select `a", "é ü", "select 'é', [b] from \"c\" where d = @e;", "$tag$ open", "{x:UInt8} {y", "\u{1F600}"] {
                assert_eq!(parse(flavor, text).print(), text, "{flavor:?}");
            }
        }
    }

    #[test]
    fn splits_statements() {
        assert_eq!(statements(Flavor::MYSQL, "select ';'; select \"a;b\"; -- c;\nselect `d;`"), 3);
        assert_eq!(statements(Flavor::MYSQL, "create procedure p() begin select 1; if a then select 2; end if; end; select 3"), 2);
        assert_eq!(statements(Flavor::MYSQL, "begin; select 1; commit"), 3);
        assert_eq!(statements(Flavor::MYSQL, "create table t (begin int); select 1"), 2);
        assert_eq!(statements(Flavor::MYSQL, "create table begin (a int); select 1"), 2);
        assert_eq!(statements(Flavor::MYSQL, "create procedure p(x int) begin case x when 1 then select 1; else select 2; end case; end; select 3"), 2);
        assert_eq!(statements(Flavor::MYSQL, "create procedure p() begin a:begin select 1; end a; select 2; end; select 3"), 2);
        assert_eq!(statements(Flavor::MYSQL, "create function f() returns int language javascript as $$ return 1; $$; select 2"), 2);
        assert_eq!(statements(Flavor::MYSQL, "/*!50003 create procedure p() begin select 1; end */; select 2"), 2);
        assert_eq!(statements(Flavor::MYSQL, "create procedure p() begin if 1 then select 1; end /*!80000 if */; select 2; end; select 3"), 2);
        assert_eq!(statements(Flavor::SqlServer, "begin select 1; select 2; end; select 3"), 2);
        assert_eq!(statements(Flavor::SqlServer, "begin tran; select 1; commit"), 3);
        assert_eq!(statements(Flavor::SqlServer, "select 1\nGO\nselect 2\ngo 5\nselect 3"), 3);
        assert_eq!(statements(Flavor::SqlServer, "select 1\nGO -- next batch\nselect 2"), 2);
        assert_eq!(statements(Flavor::ClickHouse, "select $x$;$x$; select {a:Array(UInt8)}"), 2);
    }

    #[test]
    fn reads_each_dialects_quotes() {
        assert!(parse(Flavor::MYSQL, r"select 'it\'s'").errors.is_empty());
        assert!(!parse(Flavor::Mysql { backslash_escapes: false, ansi_quotes: false }, r"select 'it\'s'").errors.is_empty());
        assert!(parse(Flavor::SqlServer, "select [a]]b], N'c''d' /* x /* y */ z */").errors.is_empty());
        assert!(parse(Flavor::ClickHouse, r"select 'a\'b', `c`, 1::Int8").errors.is_empty());
        assert_eq!(statements(Flavor::MYSQL, "select 1 --2; select 3"), 2);
        assert!(parse(Flavor::ClickHouse, "select 1 as $a$b$, json.@c, @@d").errors.is_empty());
        assert!(parse(Flavor::SqlServer, "select\u{1}1").errors.is_empty());
    }

    #[test]
    fn stays_linear_on_openers_with_no_closer() {
        let tags: String = (0..40_000).map(|i| format!("$a{i}$ ")).collect();
        for (flavor, text) in [(Flavor::ClickHouse, tags.clone()), (Flavor::MYSQL, tags), (Flavor::ClickHouse, "{a:".repeat(160_000)), (Flavor::SqlServer, "go ".repeat(80_000))] {
            let started = std::time::Instant::now();
            assert_eq!(parse(flavor, &text).print(), text);
            assert!(started.elapsed().as_secs() < 2, "{flavor:?}");
        }
    }

    #[test]
    fn reads_a_dollar_quote_to_its_own_tag() {
        assert_eq!(statements(Flavor::ClickHouse, "select $a$ ; $b$ ; $a$; select 2"), 2);
        assert_eq!(statements(Flavor::ClickHouse, "select $$;$$; select 2"), 2);
    }

    #[test]
    fn reports_what_doesnt_balance() {
        for (text, n) in [("select (1", 1), ("select 1)", 1), ("select ((1)", 1), ("select 'a", 1), ("select (1); select 2", 0)] {
            assert_eq!(parse(Flavor::MYSQL, text).errors.len(), n, "{text}");
        }
    }
}
