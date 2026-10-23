//! The errors a statement may raise, where the engine sees every piece of code it runs: the type
//! of `.try()`'s error, narrowed for the statement (wiki/run-safe.md).
//!
//! A statement is closed where it runs only its database's own code: no trigger, no rule, no
//! function or operator a user or an extension defined, in the statement or in anything it sets
//! off (a default, a check, a policy, a view, a foreign key's action), and a server no newer than
//! the run time's list of codes. Every error such a statement raises is one its database defines,
//! and an error about a constraint names one of the constraints the statement writes through. A
//! statement that is not closed is not narrowed: a trigger may raise any code with any name.

use crate::dialect::Dialect;
use crate::lex;
use serde::Serialize;
use std::collections::BTreeMap;

/// By the error's code (Postgres's SQLSTATE, MySQL's error name, SQLite's result code's name), each
/// field the error names and the names it may hold, sorted. A code with no entry may still be
/// raised, with names the type does not narrow.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(transparent)]
pub struct Errors(pub BTreeMap<String, BTreeMap<String, Vec<String>>>);

impl Errors {
    /// `name` among the names `field` of an error `code` may hold.
    pub fn add(&mut self, code: &str, field: &str, name: &str) {
        let names = self.0.entry(code.to_string()).or_default().entry(field.to_string()).or_default();
        if let Err(at) = names.binary_search_by(|n| n.as_str().cmp(name)) {
            names.insert(at, name.to_string());
        }
    }
}

/// The words of `sql` outside its strings and comments, lowercased, each with the character after
/// it that is neither a space nor in a comment. A quoted identifier is a word too, unquoted: a
/// backquoted, double-quoted or bracketed one, where a dialect reads a double quote as a string
/// as well, which leaves the scan the more cautious. MySQL runs what a `/*!` comment holds, so the
/// scan reads it as text.
fn words(sql: &str, dialect: Dialect) -> Vec<(String, Option<char>)> {
    let opaque = lex::lex(sql, dialect).map(|l| l.opaque).unwrap_or_default();
    let span = |i: usize| opaque.iter().find(|&&(s, _)| s == i).map(|&(_, e)| e.max(i + 1));
    let b = sql.as_bytes();
    let comment = |i: usize| [&b"/*"[..], b"--", b"#"].iter().any(|c| b[i..].starts_with(c)) && !b[i..].starts_with(b"/*!");
    // The character after `i` that is neither a space nor in a comment.
    let next = |mut i: usize| loop {
        let c = sql[i..].chars().next()?;
        match span(i) {
            Some(e) if comment(i) => i = e,
            _ if c.is_whitespace() => i += c.len_utf8(),
            _ => return Some(c),
        }
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"/*!") {
            i += 3;
            continue;
        }
        if let Some(e) = span(i) {
            if let (Some(close), true) = (match b[i] { b'`' => Some('`'), b'"' => Some('"'), b'[' => Some(']'), _ => None }, e >= i + 2) {
                let open = b[i] as char;
                let inner = &sql[i + 1..e - 1];
                let word = if open == close { inner.replace(&format!("{close}{close}"), &close.to_string()) } else { inner.to_string() };
                out.push((word.to_lowercase(), next(e)));
            }
            i = e;
            continue;
        }
        // A name may hold letters beyond ASCII.
        if b[i].is_ascii_alphabetic() || b[i] == b'_' || b[i] >= 0x80 {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$' || b[i] >= 0x80) {
                i += 1;
            }
            out.push((sql[start..i].to_lowercase(), next(i)));
            continue;
        }
        i += 1;
    }
    out
}

/// Every word `sql` calls as a function: a word a parenthesis follows. A function of the name a user
/// defined may be the one it calls.
pub fn called(sql: &str, dialect: Dialect) -> Vec<String> {
    let mut out: Vec<String> = words(sql, dialect).into_iter().filter(|(_, next)| *next == Some('(')).map(|(w, _)| w).collect();
    out.sort();
    out.dedup();
    out
}

/// Whether `sql` holds one of `any` as a word, outside its strings and comments.
pub fn has_word(sql: &str, dialect: Dialect, any: &[&str]) -> bool {
    words(sql, dialect).iter().any(|(w, _)| any.contains(&w.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_kept_once_and_sorted() {
        let mut errors = Errors::default();
        errors.add("23505", "constraint", "users_pkey");
        errors.add("23505", "constraint", "users_email_key");
        errors.add("23505", "constraint", "users_pkey");
        errors.add("23505", "table", "users");
        assert_eq!(serde_json::to_value(&errors).unwrap(), serde_json::json!({ "23505": { "constraint": ["users_email_key", "users_pkey"], "table": ["users"] } }));
    }

    #[test]
    fn a_call_is_a_word_a_parenthesis_follows_outside_strings() {
        assert_eq!(called("select tax (price), 'f(x)', count(*) /* g(y) */ from t where h = 1", Dialect::Mysql), vec!["count", "tax"]);
        assert!(has_word("insert into t (a) values (1) on duplicate key update a = 2", Dialect::Mysql, &["update"]));
        assert!(!has_word("insert into t (a) values ('update')", Dialect::Sqlite, &["update", "replace"]));
        // A quoted name is the name, and a comment may sit before the parenthesis.
        assert_eq!(called("select `Tax`(price), other.rate /* why */ (price), \"f\"\"g\"(1) from t", Dialect::Mysql), vec!["f\"g", "rate", "tax"]);
        assert_eq!(called("select 1 /*!80000 + tax(2) */", Dialect::Mysql), vec!["tax"]);
        assert_eq!(called("select 'é', ré(1), `é`(2)", Dialect::Mysql), vec!["ré", "é"]);
    }
}
