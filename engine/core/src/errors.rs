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
/// it that is not a space.
fn words(sql: &str, dialect: Dialect) -> Vec<(String, Option<char>)> {
    let opaque = lex::lex(sql, dialect).map(|l| l.opaque).unwrap_or_default();
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if let Some(&(_, e)) = opaque.iter().find(|&&(s, _)| s == i) {
            i = e.max(i + 1);
            continue;
        }
        if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            let next = sql[i..].chars().find(|c| !c.is_whitespace());
            out.push((sql[start..i].to_ascii_lowercase(), next));
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
    }
}
