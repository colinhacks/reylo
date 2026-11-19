//! What an interactive terminal shows beyond the lines a command prints: a spinner on one line of
//! stderr while the command works, repainted in place and cleared when the work is done, and SQL
//! coloured by its tokens. The command line turns each on only for a terminal, so that a pipe or a
//! redirected file receives the plain text, byte for byte.

use crate::Dialect;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Duration;

static SPIN: AtomicBool = AtomicBool::new(false);
static LINE: Mutex<Line> = Mutex::new(Line { text: None, drawn: false, frame: 0 });

/// The spinner's frames, one each tick.
pub const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const TICK: Duration = Duration::from_millis(80);

struct Line {
    /// What the spinner says, while one spins.
    text: Option<String>,
    /// Whether a frame is on the terminal now, to be cleared before anything else is written.
    drawn: bool,
    /// The next frame, which runs on from one spinner to the next, so that a command's short steps
    /// still turn it.
    frame: usize,
}

/// Whether a spinner is drawn: the command line turns it on where stdout and stderr are both a
/// terminal that can repaint a line, and never with `--json`.
pub fn set_spinner(on: bool) {
    SPIN.store(on, Ordering::Relaxed);
}

fn line() -> std::sync::MutexGuard<'static, Line> {
    LINE.lock().unwrap_or_else(|e| e.into_inner())
}

/// A spinner saying `text` until it is dropped. One started while another spins says nothing of
/// its own: the outer one says what the command is doing.
pub fn spinner(text: impl Into<String>) -> Spinner {
    if !SPIN.load(Ordering::Relaxed) {
        return Spinner { ticker: None };
    }
    let mut l = line();
    if l.text.is_some() {
        return Spinner { ticker: None };
    }
    l.text = Some(text.into());
    drop(l);
    let (stop, stopped) = mpsc::channel::<()>();
    let ticker = std::thread::spawn(move || loop {
        draw();
        if !matches!(stopped.recv_timeout(TICK), Err(mpsc::RecvTimeoutError::Timeout)) {
            break;
        }
    });
    Spinner { ticker: Some((stop, ticker)) }
}

/// A running spinner; dropping it clears its line.
pub struct Spinner {
    ticker: Option<(mpsc::Sender<()>, JoinHandle<()>)>,
}

impl Spinner {
    /// Says `text` from the next frame on, where this is the spinner on the terminal.
    pub fn say(&self, text: impl Into<String>) {
        if self.ticker.is_some() {
            line().text = Some(text.into());
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        let Some((stop, ticker)) = self.ticker.take() else { return };
        drop(stop);
        let _ = ticker.join();
        let mut l = line();
        clear(&mut l);
        l.text = None;
    }
}

fn draw() {
    let mut l = line();
    let Some(text) = &l.text else { return };
    let frame = FRAMES[l.frame % FRAMES.len()];
    let shown = fitted(text, width().saturating_sub(3));
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "\r\x1b[2K{} {shown}", crate::commands::paint("cyan", &frame.to_string()));
    let _ = err.flush();
    l.frame += 1;
    l.drawn = true;
}

fn clear(l: &mut Line) {
    if l.drawn {
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "\r\x1b[2K");
        let _ = err.flush();
        l.drawn = false;
    }
}

/// Writes one line of a command's output on stdout, the spinner first cleared off the terminal so
/// that the two never share a line; the spinner is drawn again under it at its next frame.
pub fn println(text: &str) {
    let mut l = line();
    clear(&mut l);
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{text}");
    let _ = out.flush();
}

/// `text` cut to `columns` of the terminal's cells, so that the spinner's line never wraps, which
/// would leave a line behind that a repaint can't reach.
fn fitted(text: &str, columns: usize) -> String {
    if text.chars().map(cells).sum::<usize>() <= columns {
        return text.to_string();
    }
    let mut cut = String::new();
    let mut used = 0;
    for c in text.chars() {
        if used + cells(c) + 1 > columns {
            break;
        }
        used += cells(c);
        cut.push(c);
    }
    cut.push('…');
    cut
}

/// The cells a terminal gives `c`: two for an East Asian wide character or an emoji, none for a
/// combining mark, else one.
fn cells(c: char) -> usize {
    match c as u32 {
        0x0300..=0x036F | 0x200B..=0x200F | 0xFE00..=0xFE0F => 0,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x1F300..=0x1F64F | 0x1F900..=0x1F9FF | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}

/// The terminal's width in columns, from stderr's terminal; 80 where it can't be read.
fn width() -> usize {
    #[cfg(unix)]
    {
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        // SAFETY: TIOCGWINSZ writes a `winsize` into the one it is given, and nothing else.
        if unsafe { libc::ioctl(libc::STDERR_FILENO, libc::TIOCGWINSZ, &mut size) } == 0 && size.ws_col > 0 {
            return size.ws_col as usize;
        }
    }
    80
}

/// What a token of SQL is, for its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Keyword,
    Type,
    /// A string, a number, or a dollar-quoted body's delimiters.
    Literal,
    Comment,
    /// A name, quoted or not, an operator or punctuation: left as the terminal writes text.
    Plain,
}

impl Token {
    /// The token's ANSI style: keywords cyan, types yellow, strings and numbers green, comments
    /// dim, as the command's other lines use those colours.
    pub fn code(self) -> Option<&'static str> {
        match self {
            Token::Keyword => Some("36"),
            Token::Type => Some("33"),
            Token::Literal => Some("32"),
            Token::Comment => Some("2"),
            Token::Plain => None,
        }
    }
}

/// `sql`, as `dialect` reads it, with ANSI colours on its tokens where lines carry colours
/// ([`crate::commands::set_color`]); else `sql` itself.
pub fn highlight(sql: &str, dialect: Dialect) -> String {
    if !crate::commands::color() {
        return sql.to_string();
    }
    colored(sql, dialect)
}

/// `sql` with ANSI colours on its tokens. Each line of a token is coloured on its own, so that no
/// colour runs on past a line's end. Without its escapes it is `sql`, byte for byte.
pub fn colored(sql: &str, dialect: Dialect) -> String {
    let mut out = String::with_capacity(sql.len() * 2);
    for (kind, text) in tokens(sql, dialect) {
        match kind.code() {
            None => out.push_str(text),
            Some(code) => {
                for (i, part) in text.split('\n').enumerate() {
                    if i > 0 {
                        out.push('\n');
                    }
                    if !part.is_empty() {
                        out.push_str(&format!("\x1b[{code}m{part}\x1b[0m"));
                    }
                }
            }
        }
    }
    out
}

/// The tokens of `sql`, in order, which together are `sql`. A dollar-quoted body (a function's) is
/// read as SQL itself, between its delimiters.
pub fn tokens(sql: &str, dialect: Dialect) -> Vec<(Token, &str)> {
    let mut out = Vec::new();
    lex(sql, dialect, &mut out);
    out
}

fn lex<'a>(sql: &'a str, dialect: Dialect, out: &mut Vec<(Token, &'a str)>) {
    let b = sql.as_bytes();
    let mut i = 0;
    let mut plain = 0;
    let flush = |out: &mut Vec<(Token, &'a str)>, from: usize, to: usize| {
        if from < to {
            out.push((Token::Plain, &sql[from..to]));
        }
    };
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        let (kind, end) = if c == b'-' && next == Some(b'-') {
            (Token::Comment, sql[i..].find('\n').map_or(b.len(), |n| i + n))
        } else if c == b'/' && next == Some(b'*') {
            (Token::Comment, sql[i + 2..].find("*/").map_or(b.len(), |n| i + 2 + n + 2))
        } else if c == b'\'' {
            // A backslash escapes in MySQL's strings and in Postgres's E'…' strings; the others
            // double a quote to escape it.
            let escaped = dialect == Dialect::Mysql || i > 0 && matches!(b[i - 1], b'E' | b'e') && (i < 2 || !is_word(b[i - 2]));
            (Token::Literal, quoted(b, i, b'\'', escaped))
        } else if c == b'"' || c == b'`' {
            (Token::Plain, quoted(b, i, c, false))
        } else if c == b'$' {
            match dollar_tag(sql, i) {
                Some(tag) => {
                    flush(out, plain, i);
                    let body = i + tag.len();
                    let close = sql[body..].find(tag).map(|n| body + n);
                    out.push((Token::Literal, tag));
                    lex(&sql[body..close.unwrap_or(b.len())], dialect, out);
                    if let Some(close) = close {
                        out.push((Token::Literal, tag));
                        i = close + tag.len();
                    } else {
                        i = b.len();
                    }
                    plain = i;
                    continue;
                }
                None => {
                    // A parameter ($1) or a `$` inside a name: plain.
                    i += 1;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                    continue;
                }
            }
        } else if c.is_ascii_digit() && (i == 0 || !is_word(b[i - 1])) {
            let mut end = i;
            while end < b.len() && (b[end].is_ascii_alphanumeric() || b[end] == b'.' || b[end] == b'_') {
                end += 1;
            }
            (Token::Literal, end)
        } else if is_word(c) {
            let mut end = i;
            while end < b.len() && (is_word(b[end]) || b[end] == b'$') {
                end += 1;
            }
            let word = &sql[i..end];
            // A name qualified by a table or schema (`users.id`, `public.users`) is a name.
            let qualified = i > 0 && b[i - 1] == b'.' || b.get(end) == Some(&b'.');
            let kind = if qualified || defines(b, i, end) { Token::Plain } else { word_kind(word) };
            if kind == Token::Plain {
                i = end;
                continue;
            }
            (kind, end)
        } else {
            i += 1;
            continue;
        };
        flush(out, plain, i);
        out.push((kind, &sql[i..end]));
        i = end;
        plain = i;
    }
    flush(out, plain, b.len());
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80
}

/// Whether the word at `start..end` is the name a column or a parameter is defined with: it opens
/// a list's item and a type follows it (`(date date, type text`), so that a name that reads as a
/// keyword or a type is coloured as the name it is.
fn defines(b: &[u8], start: usize, end: usize) -> bool {
    let before = b[..start].iter().rev().find(|c| !c.is_ascii_whitespace());
    if !matches!(before, Some(b'(' | b',')) {
        return false;
    }
    let gap = b[end..].iter().take_while(|c| c.is_ascii_whitespace()).count();
    let from = end + gap;
    let len = b[from..].iter().take_while(|&&c| is_word(c)).count();
    gap > 0 && len > 0 && std::str::from_utf8(&b[from..from + len]).is_ok_and(|w| TYPES.contains(&w.to_ascii_lowercase().as_str()))
}

/// The end of a quoted run that opens at `start` with `q`, a doubled `q` (or, where `escaped`, a
/// backslash) escaping one; the text's end where it never closes.
fn quoted(b: &[u8], start: usize, q: u8, escaped: bool) -> usize {
    let mut i = start + 1;
    while i < b.len() {
        if escaped && b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == q {
            if b.get(i + 1) == Some(&q) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    b.len()
}

/// The dollar-quote delimiter opening at `i` (`$$`, `$body$`), if one does.
fn dollar_tag(sql: &str, i: usize) -> Option<&str> {
    let b = sql.as_bytes();
    if i > 0 && is_word(b[i - 1]) {
        return None;
    }
    let mut j = i + 1;
    while j < b.len() && (b[j].is_ascii_alphabetic() || b[j] == b'_' || (j > i + 1 && b[j].is_ascii_digit())) {
        j += 1;
    }
    (b.get(j) == Some(&b'$')).then(|| &sql[i..=j])
}

fn word_kind(word: &str) -> Token {
    let lower = word.to_ascii_lowercase();
    if TYPES.contains(&lower.as_str()) {
        Token::Type
    } else if KEYWORDS.contains(&lower.as_str()) {
        Token::Keyword
    } else {
        Token::Plain
    }
}

/// The words of a schema's SQL that name a type, on Postgres, MySQL and SQLite.
const TYPES: &[&str] = &[
    "bigint", "bigserial", "binary", "bit", "blob", "bool", "boolean", "box", "bytea", "char", "character", "cidr", "circle", "citext", "date", "datetime", "daterange", "decimal", "double", "enum", "float", "float4", "float8", "geography", "geometry", "inet", "int", "int2", "int4", "int8", "int4range", "int8range", "integer", "interval", "json", "jsonb", "line", "longblob", "longtext", "lseg", "macaddr", "macaddr8", "mediumblob", "mediumint", "mediumtext", "money", "numeric", "numrange", "path", "point", "polygon", "precision", "real", "serial", "smallint", "smallserial", "text", "time", "timestamp", "timestamptz", "timetz", "tinyblob", "tinyint", "tinytext", "tsquery", "tsrange", "tstzrange", "tsvector", "uuid", "varbinary", "varbit", "varchar", "varying", "vector", "xml", "year",
];

/// The keywords a schema's SQL uses, on Postgres, MySQL and SQLite.
const KEYWORDS: &[&str] = &[
    "add", "after", "aggregate", "all", "alter", "always", "and", "any", "array", "as", "asc", "auto_increment", "autoincrement", "before", "begin", "between", "by", "cascade", "case", "cast", "charset", "check", "collate", "column", "comment", "constraint", "create", "cross", "current_date", "current_time", "current_timestamp", "cycle", "declare", "default", "deferrable", "deferred", "delete", "desc", "distinct", "do", "domain", "drop", "each", "else", "enable", "end", "engine", "exclude", "execute", "exists", "extension", "false", "first", "for", "foreign", "from", "full", "function", "generated", "grant", "group", "having", "identity", "if", "immediate", "in", "include", "increment", "index", "initially", "inner", "insert", "instead", "into", "is", "join", "key", "language", "last", "left", "level", "like", "limit", "materialized", "maxvalue", "minvalue", "new", "no", "not", "null", "nulls", "of", "old", "on", "or", "order", "outer", "owned", "partition", "policy", "primary", "procedure", "raise", "range", "references", "rename", "replace", "restrict", "return", "returns", "right", "row", "rowid", "rows", "schema", "security", "select", "sequence", "set", "start", "stored", "strict", "table", "temporary", "then", "to", "trigger", "true", "type", "union", "unique", "unlogged", "unsigned", "update", "using", "values", "view", "virtual", "when", "where", "with", "without", "zone",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(s: &str) -> String {
        regex::Regex::new("\x1b\\[[0-9;]*m").expect("a valid pattern").replace_all(s, "").into_owned()
    }

    fn kinds(sql: &str) -> Vec<(Token, String)> {
        kinds_on(sql, Dialect::Postgres)
    }

    fn kinds_on(sql: &str, dialect: Dialect) -> Vec<(Token, String)> {
        tokens(sql, dialect).into_iter().filter(|(k, _)| *k != Token::Plain).map(|(k, t)| (k, t.to_string())).collect()
    }

    const SCHEMA: &str = "-- the users\ncreate table users (\n  id integer not null,\n  email varchar(255) not null default 'a''b',\n  \"Select\" text,\n  score numeric(10, 2) default 1.5,\n  constraint users_pkey primary key (id)\n);\n/* a block\n   comment */\ncreate function f() returns trigger\n  language plpgsql\nas $body$\nbegin\n  return new; -- done\nend\n$body$;\ncreate index users_email on public.users (email);\n";

    #[test]
    fn the_colours_come_off_to_leave_the_sql() {
        for sql in [SCHEMA, "", "x", "'unclosed", "/* unclosed", "$$ unclosed", "a$1 $2 é 'ü' \"q\"\"q\" `b``c`", "create table t (id int) engine=InnoDB;\n"] {
            for d in [Dialect::Postgres, Dialect::Mysql, Dialect::Sqlite] {
                assert_eq!(strip(&colored(sql, d)), sql);
                assert_eq!(tokens(sql, d).iter().map(|(_, t)| *t).collect::<String>(), sql);
            }
        }
    }

    #[test]
    fn each_token_gets_its_kind() {
        let k = kinds(SCHEMA);
        let has = |kind: Token, text: &str| k.iter().any(|(kk, t)| *kk == kind && t == text);
        assert!(has(Token::Comment, "-- the users"));
        assert!(has(Token::Comment, "/* a block\n   comment */"));
        assert!(has(Token::Comment, "-- done"));
        assert!(has(Token::Keyword, "create"));
        assert!(has(Token::Keyword, "primary"));
        assert!(has(Token::Keyword, "returns"));
        assert!(has(Token::Keyword, "begin"));
        assert!(has(Token::Type, "integer"));
        assert!(has(Token::Type, "varchar"));
        assert!(has(Token::Literal, "'a''b'"));
        assert!(has(Token::Literal, "255"));
        assert!(has(Token::Literal, "1.5"));
        assert!(has(Token::Literal, "$body$"));
        // Names stay plain: a quoted one that reads as a keyword, one qualified by its schema, and
        // a word with digits.
        assert!(!k.iter().any(|(_, t)| t == "\"Select\"" || t == "users" || t == "users_pkey" || t == "plpgsql"));
        assert!(!has(Token::Keyword, "users"));
    }

    #[test]
    fn a_column_named_as_a_keyword_or_a_type_is_a_name() {
        assert_eq!(kinds("(date date,\n  type text)"), vec![(Token::Type, "date".into()), (Token::Type, "text".into())]);
        assert_eq!(tokens("(date date", Dialect::Postgres)[1], (Token::Type, "date"));
        assert_eq!(kinds("unique (date)"), vec![(Token::Keyword, "unique".into()), (Token::Type, "date".into())]);
    }

    #[test]
    fn a_backslash_escapes_only_where_the_string_says() {
        assert_eq!(kinds("'C:\\' x"), vec![(Token::Literal, "'C:\\'".into())]);
        assert_eq!(kinds("E'it\\'s' x"), vec![(Token::Literal, "'it\\'s'".into())]);
        assert_eq!(kinds_on("default 'it\\'s fine', n int", Dialect::Mysql), vec![(Token::Keyword, "default".into()), (Token::Literal, "'it\\'s fine'".into()), (Token::Type, "int".into())]);
    }

    #[test]
    fn a_qualified_name_is_a_name() {
        assert_eq!(kinds("references public.user (id)"), vec![(Token::Keyword, "references".into())]);
        assert_eq!(kinds("t.date"), vec![]);
    }

    #[test]
    fn no_colour_runs_past_a_line() {
        let out = colored("/* one\ntwo */", Dialect::Postgres);
        assert_eq!(out, "\x1b[2m/* one\x1b[0m\n\x1b[2mtwo */\x1b[0m");
    }

    #[test]
    fn a_long_spinner_line_is_cut_to_the_terminal() {
        assert_eq!(fitted("Introspecting postgres://localhost:5432/app", 80), "Introspecting postgres://localhost:5432/app");
        assert_eq!(fitted("abcdef", 4), "abc…");
        // A wide character takes two cells.
        let cut = fitted("Introspecting sqlite:./数据库数据库数据库.db", 37);
        assert!(cut.chars().map(cells).sum::<usize>() <= 37, "{cut}");
        assert_eq!(cut, "Introspecting sqlite:./数据库数据库…");
        assert_eq!(fitted("数据库", 6), "数据库");
    }

    #[test]
    fn no_colour_unless_lines_carry_it() {
        crate::commands::set_color(false);
        assert_eq!(highlight(SCHEMA, Dialect::Postgres), SCHEMA);
    }

    #[test]
    fn no_spinner_unless_it_is_on() {
        // Off, as it is wherever stdout or stderr isn't a terminal, a spinner starts nothing and
        // writes nothing.
        set_spinner(false);
        let s = spinner("Introspecting");
        assert!(s.ticker.is_none());
        s.say("Replaying");
        assert!(line().text.is_none());
    }
}
