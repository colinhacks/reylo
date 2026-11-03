//! The statements of a migration file, and what each one is. `reylo migrate` runs every file
//! atomically, so it has to know where one statement ends and whether a statement can run inside a
//! transaction at all. A statement ends at a `;` outside strings, quoted names, comments and dollar
//! quotes; the body of a trigger, routine or event (`begin … end`, Postgres's `begin atomic … end`)
//! holds `;`s of its own and ends at its closing `end`.

use crate::Dialect;
use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Word,
    Quoted,
    Str,
    Number,
    Symbol,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    /// Its byte range in the text it was read from.
    pub start: usize,
    pub end: usize,
}

impl Token {
    /// The word uppercased, or "" for any other token.
    pub fn upper(&self) -> String {
        if self.kind == TokenKind::Word {
            self.text.to_ascii_uppercase()
        } else {
            String::new()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Statement {
    /// The statement's text as written, without its `;`.
    pub text: String,
    /// Its tokens, comments and whitespace left out.
    pub tokens: Vec<Token>,
}

/// Whether `sql` holds nothing but whitespace and comments.
pub fn is_blank(sql: &str, dialect: Dialect) -> bool {
    tokenize(sql, dialect).is_empty()
}

fn is_word_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn is_word_part(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80
}

/// The tokens of `sql`: words (keywords and bare names), quoted names, string literals, numbers and
/// symbols, with comments and whitespace dropped. A `;` is a symbol. The lexing follows each
/// dialect's default reading: MySQL's backslash escapes and `#` comments, Postgres's nested block
/// comments, dollar quotes and E'' strings, SQLite's `[name]` and backtick names. Every delimiter is
/// ASCII, so the text is scanned byte by byte, and a byte of a character outside ASCII is part of
/// a word.
pub fn tokenize(sql: &str, dialect: Dialect) -> Vec<Token> {
    let b = sql.as_bytes();
    let n = b.len();
    let mysql = dialect == Dialect::Mysql;
    let postgres = dialect == Dialect::Postgres;
    let mut out = Vec::new();
    let mut i = 0;
    let push = |kind: TokenKind, start: usize, end: usize, out: &mut Vec<Token>| out.push(Token { kind, text: sql[start..end].to_string(), start, end });
    while i < n {
        let ch = b[i];
        let next = b.get(i + 1).copied();
        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // Comments. MySQL reads `--` as a comment only before whitespace or a control character.
        if ch == b'-' && next == Some(b'-') && !(mysql && i + 2 < n && b[i + 2] > 32) {
            i = sql[i..].find('\n').map_or(n, |e| i + e + 1);
            continue;
        }
        if ch == b'#' && mysql {
            i = sql[i..].find('\n').map_or(n, |e| i + e + 1);
            continue;
        }
        if ch == b'/' && next == Some(b'*') {
            let mut depth = 1;
            let mut j = i + 2;
            while j < n && depth > 0 {
                if postgres && b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
                    depth += 1;
                    j += 2;
                    continue;
                }
                if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
                    depth -= 1;
                    j += 2;
                    continue;
                }
                j += 1;
            }
            i = j.min(n);
            continue;
        }
        let start = i;
        // Postgres dollar quotes: $$ … $$ and $tag$ … $tag$.
        if ch == b'$' && postgres {
            let mut j = i + 1;
            if j < n && (b[j].is_ascii_alphabetic() || b[j] == b'_') {
                while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                    j += 1;
                }
            }
            if j < n && b[j] == b'$' {
                let tag = &sql[i..=j];
                i = sql[j + 1..].find(tag).map_or(n, |at| j + 1 + at + tag.len());
                push(TokenKind::Str, start, i, &mut out);
                continue;
            }
        }
        // String literals: '…' with '' (and on MySQL, or in Postgres's E'', a backslash) escaping.
        let escaped = postgres && (ch == b'E' || ch == b'e') && next == Some(b'\'');
        if ch == b'\'' || escaped || (mysql && ch == b'"') {
            let quote = if escaped { b'\'' } else { ch };
            let mut j = if escaped { i + 2 } else { i + 1 };
            while j < n {
                if (mysql || escaped) && b[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if b[j] == quote {
                    if b.get(j + 1) == Some(&quote) {
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                j += 1;
            }
            i = j.min(n);
            push(TokenKind::Str, start, i, &mut out);
            continue;
        }
        // Quoted names.
        if ch == b'"' || ch == b'`' || (ch == b'[' && dialect == Dialect::Sqlite) {
            let close = if ch == b'[' { b']' } else { ch };
            let mut j = i + 1;
            while j < n {
                if b[j] == close {
                    if close != b']' && b.get(j + 1) == Some(&close) {
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                j += 1;
            }
            i = j.min(n);
            push(TokenKind::Quoted, start, i, &mut out);
            continue;
        }
        if ch.is_ascii_digit() || (ch == b'.' && next.is_some_and(|c| c.is_ascii_digit())) {
            let mut j = i + 1;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'.') {
                j += 1;
            }
            i = j;
            push(TokenKind::Number, start, i, &mut out);
            continue;
        }
        if is_word_start(ch) {
            let mut j = i + 1;
            while j < n && is_word_part(b[j]) {
                j += 1;
            }
            i = j;
            push(TokenKind::Word, start, i, &mut out);
            continue;
        }
        // Two-character operators read as one symbol, so `::` and `||` stay whole.
        if i + 1 < n && matches!(&b[i..i + 2], b"::" | b"||" | b"<=" | b">=" | b"<>" | b"!=" | b"->" | b"=>") {
            i += 2;
            push(TokenKind::Symbol, start, i, &mut out);
            continue;
        }
        // One character, whole even where it is not ASCII.
        let width = sql[i..].chars().next().map_or(1, char::len_utf8);
        i += width;
        push(TokenKind::Symbol, start, i, &mut out);
    }
    out
}

fn upper(t: Option<&Token>) -> String {
    t.map(Token::upper).unwrap_or_default()
}

/// Whether a BEGIN read at this point opens a body: the statement creates a trigger, routine or
/// event (MySQL and SQLite), or a Postgres function or procedure whose body is `begin atomic`.
fn opens_body(tokens: &[Token], dialect: Dialect, next: Option<&Token>) -> bool {
    if upper(tokens.first()) != "CREATE" {
        return false;
    }
    let words: Vec<String> = tokens.iter().take(12).map(Token::upper).collect();
    let has = |w: &str| words.iter().any(|x| x == w);
    if dialect == Dialect::Postgres {
        return (has("FUNCTION") || has("PROCEDURE")) && upper(next) == "ATOMIC";
    }
    ["TRIGGER", "PROCEDURE", "FUNCTION", "EVENT"].iter().any(|w| has(w))
}

/// The statements of `sql`, in order; blank statements (`;;`) are dropped.
pub fn split_statements(sql: &str, dialect: Dialect) -> Vec<Statement> {
    let tokens = tokenize(sql, dialect);
    let mut statements = Vec::new();
    let mut from = 0;
    let mut depth = 0usize;
    let mut current: Vec<Token> = Vec::new();
    let flush = |current: &mut Vec<Token>, from: usize, end: usize, statements: &mut Vec<Statement>| {
        if !current.is_empty() {
            statements.push(Statement { text: sql[from..end].trim().to_string(), tokens: std::mem::take(current) });
        }
    };
    for k in 0..tokens.len() {
        let t = &tokens[k];
        if current.is_empty() {
            from = t.start;
            depth = 0;
        }
        if t.kind == TokenKind::Symbol && t.text == ";" && depth == 0 {
            flush(&mut current, from, t.start, &mut statements);
            continue;
        }
        current.push(t.clone());
        // A body opens at BEGIN (and inside it, at a nested BEGIN or a CASE); END closes one, but for
        // END IF, END LOOP, END WHILE and END REPEAT, which close constructs never counted.
        let w = t.upper();
        let after = upper(tokens.get(k + 1));
        if w == "BEGIN" && (depth > 0 || opens_body(&current, dialect, tokens.get(k + 1))) {
            depth += 1;
        } else if w == "CASE" && depth > 0 {
            depth += 1;
        } else if w == "END" && depth > 0 && !matches!(after.as_str(), "IF" | "LOOP" | "WHILE" | "REPEAT") {
            depth -= 1;
        }
    }
    flush(&mut current, from, sql.len(), &mut statements);
    statements
}

/// The statement's first words, uppercased, for matching its kind.
pub fn head(statement: &Statement, count: usize) -> String {
    statement.tokens.iter().take(count).map(|t| if t.kind == TokenKind::Word { t.text.to_ascii_uppercase() } else { t.text.clone() }).collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Runs inside the migration's transaction.
    Transactional,
    /// Cannot run inside a transaction, and so must be the only statement of its file.
    Alone,
    /// Opens, ends or marks a transaction: Reylo owns the file's transaction.
    TransactionControl,
    /// MySQL: commits the transaction around it, as every DDL statement does.
    Commits,
}

macro_rules! re {
    ($name:ident, $pattern:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pattern).expect("a valid pattern"));
    };
}

re!(TRANSACTION_CONTROL, r"^(BEGIN|START TRANSACTION|COMMIT|END|ROLLBACK|SAVEPOINT|RELEASE|ABORT|SET TRANSACTION)\b");
re!(PG_ALONE, r"^(CREATE (UNIQUE )?INDEX CONCURRENTLY|DROP INDEX CONCURRENTLY|REINDEX\b.*\bCONCURRENTLY|VACUUM|ALTER SYSTEM|CREATE DATABASE|DROP DATABASE|CREATE TABLESPACE|DROP TABLESPACE|CLUSTER$|REINDEX (SYSTEM|DATABASE))");
re!(SQLITE_ALONE, r"^(VACUUM|ATTACH|DETACH)\b");
re!(MYSQL_DATA, r"^(INSERT|UPDATE|DELETE|REPLACE|SELECT|WITH|DO|TABLE|VALUES)\b|^SET (@|SESSION|LOCAL|NAMES|CHARACTER)");

/// How the statement runs on the dialect: inside the file's transaction, or not.
pub fn kind_of(statement: &Statement, dialect: Dialect) -> Kind {
    let h = head(statement, 6);
    if TRANSACTION_CONTROL.is_match(&h) {
        return Kind::TransactionControl;
    }
    match dialect {
        Dialect::Postgres => {
            if PG_ALONE.is_match(&h) {
                Kind::Alone
            } else {
                Kind::Transactional
            }
        }
        Dialect::Sqlite => {
            if SQLITE_ALONE.is_match(&h) {
                Kind::Alone
            } else {
                Kind::Transactional
            }
        }
        // MySQL: the statements that change data run in a transaction; every other one (DDL,
        // account and table maintenance statements) commits the transaction around it.
        Dialect::Mysql => {
            if MYSQL_DATA.is_match(&h) {
                Kind::Transactional
            } else {
                Kind::Commits
            }
        }
    }
}

/// Why `sql` cannot run as one atomic migration on the dialect, or None when it can. A file runs in
/// one transaction with its history row; so a statement that cannot run in a transaction must be
/// its file's only statement, the file must not manage the transaction itself, and on MySQL, where
/// every DDL statement commits the transaction around it, a file is one DDL statement or
/// statements that change data only.
pub fn atomicity_problem(sql: &str, dialect: Dialect) -> Option<String> {
    let statements = split_statements(sql, dialect);
    if statements.is_empty() {
        return None;
    }
    let kinds: Vec<Kind> = statements.iter().map(|s| kind_of(s, dialect)).collect();
    if let Some(k) = kinds.iter().position(|k| *k == Kind::TransactionControl) {
        return Some(format!(
            "it holds its own transaction statement ({}): Reylo runs every migration in one transaction with its history row, so leave them out",
            head(&statements[k], 2).to_lowercase()
        ));
    }
    if let Some(k) = kinds.iter().position(|k| *k == Kind::Alone) {
        if statements.len() > 1 {
            return Some(format!("{} cannot run inside a transaction, so it must be the only statement of its file", head(&statements[k], 4).to_lowercase()));
        }
    }
    let commits: Vec<&Statement> = statements.iter().zip(&kinds).filter(|(_, k)| **k == Kind::Commits).map(|(s, _)| s).collect();
    if dialect == Dialect::Mysql && !commits.is_empty() && statements.len() > 1 {
        return Some(if commits.len() > 1 {
            format!(
                "it holds {} DDL statements, and MySQL commits each one on its own, so the file cannot be atomic: give each its own file, or combine changes to one table into one alter table",
                commits.len()
            )
        } else {
            format!("MySQL commits a DDL statement on its own, so it cannot share a file with other statements: give {} a file of its own", head(commits[0], 3).to_lowercase())
        });
    }
    None
}

/// Whether the file runs outside a transaction: one statement that cannot run in one, or MySQL's DDL.
pub fn runs_alone(sql: &str, dialect: Dialect) -> bool {
    let statements = split_statements(sql, dialect);
    statements.len() == 1 && matches!(kind_of(&statements[0], dialect), Kind::Alone | Kind::Commits)
}

/// The words a table's (or routine's) name follows, so that `<word> a.b` names the database `a`.
/// `on` names a table only in `create index … on` and `create trigger … on`, and `like` only in
/// `create table … like`; elsewhere `on a.b` is a join's condition and `like a.b` a comparison.
const OBJECT_KEYWORDS: &[&str] = &["TABLE", "INTO", "FROM", "JOIN", "UPDATE", "VIEW", "TRIGGER", "PROCEDURE", "FUNCTION", "EVENT", "REFERENCES", "EXISTS", "INDEX"];

re!(PG_ROLE, r"^(CREATE|ALTER|DROP) (ROLE|USER|GROUP)\b");
re!(PG_SERVER, r"^(CREATE|ALTER|DROP) (DATABASE|TABLESPACE|SUBSCRIPTION)\b|^ALTER SYSTEM\b");
re!(PG_OWNED, r"^(REASSIGN|DROP) OWNED\b");
re!(PG_LABEL, r"^(COMMENT|SECURITY LABEL)\b");
re!(PG_LABEL_SERVER, r"\bON (ROLE|DATABASE|TABLESPACE)\b");
re!(GRANT, r"^(GRANT|REVOKE)\b");
re!(MYSQL_ACCOUNT, r"^(CREATE|ALTER|DROP|RENAME) (USER|ROLE)\b|^(GRANT|REVOKE)\b");
re!(MYSQL_SERVER, r"^(CREATE|ALTER|DROP) (DATABASE|SCHEMA|SERVER|TABLESPACE|LOGFILE|RESOURCE)\b");
re!(MYSQL_SETTING, r"^SET (GLOBAL|PERSIST)\b|^SET @ @ (GLOBAL|PERSIST)\b");
re!(MYSQL_ELSEWHERE, r"^(USE|INSTALL|UNINSTALL|FLUSH|KILL|PURGE|SHUTDOWN|RESTART|CHANGE)\b");

/// Why replaying the statement in a shadow database would reach past the shadow, or None. A shadow
/// is a scratch database on the target's own server, so a statement that changes the server itself
/// (a role, a database, a tablespace, a server setting) or, on MySQL, names a database runs against
/// the real one: `create role` in a shadow makes the role the target's migration then fails to
/// create, and `create table app.users` writes into `app`.
pub fn escapes_shadow(statement: &Statement, dialect: Dialect) -> Option<String> {
    let h = head(statement, 8);
    let words: Vec<String> = statement.tokens.iter().map(|t| if t.kind == TokenKind::Word { t.text.to_ascii_uppercase() } else { t.text.clone() }).collect();
    match dialect {
        Dialect::Postgres => {
            if PG_ROLE.is_match(&h) {
                return Some("it changes a role, which belongs to the server".into());
            }
            if PG_SERVER.is_match(&h) {
                return Some("it changes the server itself (a database, a tablespace, a subscription or a server setting)".into());
            }
            if PG_OWNED.is_match(&h) {
                return Some("it acts on everything a role owns".into());
            }
            if PG_LABEL.is_match(&h) && PG_LABEL_SERVER.is_match(&words.join(" ")) {
                return Some("it labels a role, a database or a tablespace, which belong to the server".into());
            }
            if GRANT.is_match(&h) && !words.iter().any(|w| w == "ON") {
                return Some("it grants or revokes a role, which belongs to the server".into());
            }
            None
        }
        Dialect::Mysql => {
            if MYSQL_ACCOUNT.is_match(&h) {
                return Some("it changes an account or a privilege, which belong to the server".into());
            }
            if MYSQL_SERVER.is_match(&h) {
                return Some("it changes the server itself".into());
            }
            if MYSQL_SETTING.is_match(&h) {
                return Some("it changes a server setting".into());
            }
            if MYSQL_ELSEWHERE.is_match(&h) {
                return Some("it acts on the server or another database".into());
            }
            let t = &statement.tokens;
            let created = |w: &str| words.first().is_some_and(|f| f == "CREATE") && words.iter().take(6).any(|x| x == w);
            let first_on = words.iter().position(|w| w == "ON");
            let first_like = words.iter().position(|w| w == "LIKE");
            for k in 0..t.len().saturating_sub(3) {
                let names_table = OBJECT_KEYWORDS.contains(&words[k].as_str())
                    || (Some(k) == first_on && (created("INDEX") || created("TRIGGER")))
                    || (Some(k) == first_like && created("TABLE"));
                if !names_table {
                    continue;
                }
                let (a, dot, b) = (&t[k + 1], &t[k + 2], &t[k + 3]);
                let name = |x: &Token| matches!(x.kind, TokenKind::Word | TokenKind::Quoted);
                if name(a) && dot.text == "." && name(b) {
                    return Some(format!("it names the database {}: in a shadow it would reach that database itself, so leave names unqualified", a.text));
                }
            }
            None
        }
        Dialect::Sqlite => {
            if h.starts_with("ATTACH") || h.starts_with("DETACH") {
                return Some("it attaches another database file".into());
            }
            None
        }
    }
}

/// The first statement of `sql` that would reach past a shadow, with why.
pub fn shadow_problem(sql: &str, dialect: Dialect) -> Option<String> {
    split_statements(sql, dialect).iter().find_map(|s| escapes_shadow(s, dialect).map(|why| format!("{}: {why}", head(s, 3).to_lowercase())))
}

re!(DATA, r"^(INSERT|UPDATE|DELETE|MERGE|COPY|CALL|DO|SELECT|WITH|TRUNCATE|REPLACE|LOAD)\b");

/// Whether the statement changes data rather than schema: what no shadow can see happened.
pub fn changes_data(statement: &Statement) -> bool {
    DATA.is_match(&head(statement, 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(sql: &str, d: Dialect) -> Vec<String> {
        split_statements(sql, d).into_iter().map(|s| s.text).collect()
    }

    #[test]
    fn splits_at_semicolons_outside_strings_comments_and_bodies() {
        assert_eq!(texts("create table a (x text default ';'); -- ; here\ninsert into a values ('a;b');;", Dialect::Postgres), ["create table a (x text default ';')", "insert into a values ('a;b')"]);
        assert_eq!(texts("create function f() returns int as $$ select 1; $$ language sql; select 2", Dialect::Postgres).len(), 2);
        assert_eq!(texts("create function f() returns int begin atomic select 1; select 2; end; select 3", Dialect::Postgres).len(), 2);
        assert_eq!(texts("create trigger t after insert on a begin update a set x = 1; select case when 1 then 2 end; end; select 1", Dialect::Sqlite).len(), 2);
        assert_eq!(texts("create procedure p() begin if 1 then select 1; end if; select 2; end; select 3", Dialect::Mysql).len(), 2);
        assert_eq!(texts("/* a /* nested */ ; */ select 1", Dialect::Postgres), ["select 1"]);
        assert_eq!(texts("select 'it''s'; # comment ;\nselect \"a;b\"", Dialect::Mysql), ["select 'it''s'", "select \"a;b\""]);
        assert!(is_blank("-- nothing\n/* at all */", Dialect::Sqlite));
    }

    #[test]
    fn knows_what_cannot_run_in_a_transaction() {
        assert_eq!(atomicity_problem("create index concurrently i on a (x)", Dialect::Postgres), None);
        assert!(atomicity_problem("create index concurrently i on a (x); select 1", Dialect::Postgres).unwrap().contains("only statement"));
        assert!(atomicity_problem("begin; create table a (x int); commit", Dialect::Postgres).unwrap().contains("own transaction"));
        assert!(atomicity_problem("create table a (x int); create table b (x int)", Dialect::Mysql).unwrap().contains("2 DDL statements"));
        assert!(atomicity_problem("create table a (x int); insert into a values (1)", Dialect::Mysql).unwrap().contains("a file of its own"));
        assert_eq!(atomicity_problem("insert into a values (1); update a set x = 2", Dialect::Mysql), None);
        assert!(runs_alone("alter table a add column y int", Dialect::Mysql));
        assert!(!runs_alone("alter table a add column y int", Dialect::Postgres));
    }

    #[test]
    fn refuses_what_reaches_past_a_shadow() {
        assert!(shadow_problem("create role app", Dialect::Postgres).is_some());
        assert!(shadow_problem("grant app to bob", Dialect::Postgres).is_some());
        assert_eq!(shadow_problem("grant select on a to app", Dialect::Postgres), None);
        assert!(shadow_problem("create table app.users (id int)", Dialect::Mysql).unwrap().contains("names the database app"));
        assert_eq!(shadow_problem("create table users (id int)", Dialect::Mysql), None);
        assert_eq!(shadow_problem("create view v as select u.email from users u left join posts p on p.user_id = u.id where u.name like p.title", Dialect::Mysql), None);
        assert!(shadow_problem("create index i on app.users (email)", Dialect::Mysql).is_some());
        assert!(shadow_problem("create trigger t before insert on app.users for each row set new.x = 1", Dialect::Mysql).is_some());
        assert!(shadow_problem("create table t like app.users", Dialect::Mysql).is_some());
        assert!(shadow_problem("attach 'x.db' as x", Dialect::Sqlite).is_some());
    }
}
