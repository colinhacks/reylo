//! `reylo-lock.yaml`, the project's lock: the migrations in the order they run, each its name and a checksum chained to the line before, and
//! the checksum of `schema.sql` from the last time the migrations were proven to reach it. Two
//! branches that each add a migration both append to it, so git conflicts on it when the second one
//! merges, which is the point: before anything is deployed, one branch's migrations must move after
//! the other's (keep main's lines, then `reylo migrate dev`). The chain makes an edit, a
//! deletion or a reordering visible without a database: a wrong "accept both" breaks it at the
//! first line out of place. Only Reylo writes it, after applying what it lists to a development
//! database (`dev`) or reading what a database applied (`history pull`), so its checksums certify
//! what ran.

use crate::cache::{hex, Sha256};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::LazyLock;

pub const LOCK: &str = "reylo-lock.yaml";

const HEADER: &str = "# The order this project's migrations run in, written by reylo migrate dev and reylo migrate lock.
# A merge conflict here means another branch added migrations too: keep main's lines, then
# run reylo migrate dev, which runs this branch's after them and adds them back.
";

/// The lock: its migrations in order, and schema.sql's checksum where one was proven.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lock {
    pub schema: Option<String>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    /// The checksum of the migration's statements and the entry before it.
    pub sum: String,
}

impl Lock {
    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.name.clone()).collect()
    }
}

/// The lock as it reads on disk.
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Missing,
    /// It holds git's conflict markers.
    Conflicted,
    Invalid(String),
    Lock(Lock),
}

#[derive(Serialize, Deserialize)]
struct LockFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    schema: Option<String>,
    #[serde(default)]
    migrations: Vec<String>,
}

static MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^(<<<<<<<|=======|>>>>>>>)( |$)").expect("a valid pattern"));

fn digest(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.part(p.as_bytes());
    }
    hex(&h.finish()[..4])
}

/// The checksum of a migration after the entry `prev` (empty for the first): of its name and its
/// SQL as `canonical` writes it, so that comments and spacing change nothing.
pub fn checksum(prev: &str, name: &str, sql: &str) -> String {
    digest(&[prev, "\n", name, "\n", &canonical(sql)])
}

/// The checksum of schema.sql's SQL, as `canonical` writes it, with the last entry's checksum the
/// migrations it was proven against end with: a stamp that no longer holds once they change.
pub fn schema_sum(last: &str, sql: &str) -> String {
    digest(&["schema\n", last, "\n", &canonical(sql)])
}

/// SQL without its comments, its whitespace runs as one space, as any of the three databases reads
/// it: what a lock's checksum covers, so an edit to a comment or to spacing is no edit. Quoted text
/// (`'…'`, `"…"`, `` `…` ``, and Postgres's `$tag$…$tag$`) and MySQL's `/*! … */` are kept as
/// written, and so is everything after a string, a comment, a `--` or a `#` that the databases
/// read differently, or a comment that never ends, so that text one of them runs is never dropped
/// as a comment.
pub fn canonical(sql: &str) -> String {
    let b: Vec<char> = sql.chars().collect();
    let mut out = String::new();
    let mut space = false;
    let mut k = 0;
    let push = |out: &mut String, space: &mut bool, text: &str| {
        if *space && !out.is_empty() {
            out.push(' ');
        }
        *space = false;
        out.push_str(text);
    };
    while k < b.len() {
        let c = b[k];
        if c == '-' && b.get(k + 1) == Some(&'-') {
            // MySQL reads `--` as a comment only where a space or a control character follows it
            // (`n--1` is `n - -1` there), Postgres and SQLite everywhere: where the two differ, the
            // rest is kept as written.
            if b.get(k + 2).is_some_and(|n| !n.is_whitespace() && !n.is_control()) {
                push(&mut out, &mut space, &b[k..].iter().collect::<String>());
                k = b.len();
                continue;
            }
            while k < b.len() && b[k] != '\n' {
                k += 1;
            }
            space = true;
        } else if c == '#' {
            // MySQL reads `#` as a line comment, Postgres as an operator (`#`, `#>`, `#>>`): the rest
            // is kept as written, so that a `/*` or a quote inside a MySQL comment opens nothing.
            push(&mut out, &mut space, &b[k..].iter().collect::<String>());
            k = b.len();
        } else if c == '/' && b.get(k + 1) == Some(&'*') {
            // Postgres (and DuckDB) nest block comments, while MySQL and SQLite end one at its first `*/`: where
            // the two end it at different places, the rest is kept as written. MySQL runs the text
            // of `/*! … */` (as `SHOW CREATE TABLE` writes `/*!50100 PARTITION BY … */`), so that
            // comment is kept as written too, and so is the rest after a comment that never ends.
            let flat = (k + 2..b.len().saturating_sub(1)).find(|&j| b[j] == '*' && b[j + 1] == '/').map(|j| j + 2);
            let mut depth = 0;
            let mut nested = k;
            while nested < b.len() {
                if b[nested] == '/' && b.get(nested + 1) == Some(&'*') {
                    depth += 1;
                    nested += 2;
                } else if b[nested] == '*' && b.get(nested + 1) == Some(&'/') {
                    depth -= 1;
                    nested += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    nested += 1;
                }
            }
            match flat.filter(|&end| end == nested) {
                None => {
                    push(&mut out, &mut space, &b[k..].iter().collect::<String>());
                    k = b.len();
                }
                Some(end) if b.get(k + 2) == Some(&'!') => {
                    push(&mut out, &mut space, &b[k..end].iter().collect::<String>());
                    k = end;
                }
                Some(end) => {
                    k = end;
                    space = true;
                }
            }
        } else if c.is_whitespace() {
            space = true;
            k += 1;
        } else if c == '\'' || c == '"' || c == '`' {
            // MySQL reads a backslash in a string as escaping the next character, and so does
            // Postgres in an `E'…'` string, while a standard string or an identifier ends at its
            // first lone quote. Where the two readings end a string at different places (a `\'`
            // inside it), what follows could be a comment in one and string text in the other, so
            // the rest is kept as written: an edit anywhere after it is an edit.
            let standard = quoted_end(&b, k, c, false);
            let end = if c == '`' || quoted_end(&b, k, c, true) == standard { standard } else { b.len() };
            push(&mut out, &mut space, &b[k..end].iter().collect::<String>());
            k = end;
        } else if c == '$' && (k == 0 || !(b[k - 1].is_alphanumeric() || b[k - 1] == '_')) {
            // A dollar-quoted string: $tag$ … $tag$.
            let tag_end = (k + 1..b.len()).find(|&j| !(b[j].is_alphanumeric() || b[j] == '_'));
            match tag_end.filter(|&j| b[j] == '$') {
                Some(j) => {
                    let tag: String = b[k..=j].iter().collect();
                    let body_from = j + 1;
                    let rest: String = b[body_from..].iter().collect();
                    let end = rest.find(&tag).map_or(b.len(), |at| body_from + rest[..at].chars().count() + tag.chars().count());
                    push(&mut out, &mut space, &b[k..end].iter().collect::<String>());
                    k = end;
                }
                None => {
                    push(&mut out, &mut space, "$");
                    k += 1;
                }
            }
        } else {
            push(&mut out, &mut space, &c.to_string());
            k += 1;
        }
    }
    out
}

/// Where the string or identifier quoted by `q` at `start` ends: after its closing quote (a doubled
/// quote is one quote of its text), and, with `backslash`, a backslash escaping the next character.
fn quoted_end(b: &[char], start: usize, q: char, backslash: bool) -> usize {
    let mut k = start + 1;
    while k < b.len() {
        if backslash && b[k] == '\\' {
            k += 2;
        } else if b[k] == q {
            if b.get(k + 1) == Some(&q) {
                k += 2;
                continue;
            }
            return k + 1;
        } else {
            k += 1;
        }
    }
    b.len()
}

/// The entries of migrations in the order given, each its name and SQL.
pub fn chain<'a>(migrations: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for (name, canonical) in migrations {
        let prev = out.last().map(|e| e.sum.clone()).unwrap_or_default();
        out.push(Entry { name: name.to_string(), sum: checksum(&prev, name, canonical) });
    }
    out
}

fn parse(text: &str) -> Result<Lock, String> {
    let file: LockFile = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for line in file.migrations {
        let (name, sum) = line.trim().split_once(' ').ok_or_else(|| format!("\"{line}\" is a migration's name and its checksum"))?;
        entries.push(Entry { name: name.trim().to_string(), sum: sum.trim().to_string() });
    }
    Ok(Lock { schema: file.schema, entries })
}

pub fn read(path: &Path) -> State {
    match std::fs::read_to_string(path) {
        Err(_) => State::Missing,
        Ok(text) => of_text(&text),
    }
}

/// A lock's text, as read from a file or from git.
pub fn of_text(text: &str) -> State {
    if MARKER.is_match(text) {
        return State::Conflicted;
    }
    match parse(text) {
        Ok(lock) => State::Lock(lock),
        Err(e) => State::Invalid(e),
    }
}

pub fn render(lock: &Lock) -> String {
    let mut out = HEADER.to_string();
    if let Some(schema) = &lock.schema {
        out.push_str(&format!("schema: {schema}\n"));
    }
    out.push_str("migrations:\n");
    for e in &lock.entries {
        out.push_str(&format!("  - {} {}\n", e.name, e.sum));
    }
    out
}

pub fn write(path: &Path, lock: &Lock) -> std::io::Result<()> {
    std::fs::write(path, render(lock))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_reads_back_as_written() {
        let entries = chain([("20261101000000_a.sql", "create table a (id int)"), ("20261102000000_b.sql", "create table b (id int)")]);
        let lock = Lock { schema: Some(schema_sum(&entries[1].sum, "create table a (id int)")), entries };
        let text = render(&lock);
        let dir = std::env::temp_dir().join(format!("reylo-lock-unit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write(&dir.join(LOCK), &lock).unwrap();
        assert_eq!(read(&dir.join(LOCK)), State::Lock(lock.clone()));
        assert!(text.contains("  - 20261101000000_a.sql "), "{text}");
        assert_eq!(lock.entries[1].sum, checksum(&lock.entries[0].sum, "20261102000000_b.sql", "create table b (id int)"));
        std::fs::write(dir.join(LOCK), format!("{text}<<<<<<< HEAD\n")).unwrap();
        assert_eq!(read(&dir.join(LOCK)), State::Conflicted);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn comments_and_spacing_are_no_edit() {
        let a = "create table a (\n  id int -- the key\n);\n/* made by hand */";
        assert_eq!(canonical(a), canonical("create table a ( id int );"));
        assert_ne!(canonical("insert into a values ('-- not a comment')"), canonical("insert into a values ('')"));
        assert_eq!(canonical("select $$ two  spaces $$"), "select $$ two  spaces $$");
        assert_eq!(canonical("do $f$ begin -- x\n end $f$;"), "do $f$ begin -- x\n end $f$;");
        // MySQL's `\'` is a quote inside the string, so `-- b` is string text there, not a comment.
        assert_ne!(canonical("insert into t values ('a\\' -- b')"), canonical("insert into t values ('a\\' -- c')"));
        assert_ne!(canonical("select E'a\\', '-- b'"), canonical("select E'a\\', '-- c'"));
        assert_ne!(canonical("insert into t values (\"a\\\" -- b\")"), canonical("insert into t values (\"a\\\" -- c\")"));
        // MySQL runs `/*! … */`, and reads `n--1` as `n - -1`.
        assert_ne!(canonical("create table t (a int) /*!50100 PARTITION BY HASH (a) */"), canonical("create table t (a int) /*!50100 PARTITION BY KEY (a) */"));
        assert_ne!(canonical("select 1--1"), canonical("select 1--2"));
        assert_eq!(canonical("select 1 --\tx\n, 2 --\n"), "select 1 , 2");
        // A comment Postgres nests and MySQL ends at its first `*/`.
        assert_ne!(canonical("/* a /* b */ create table t (a int); /* */"), canonical("/* a /* b */ create table t (a bigint); /* */"));
        assert_eq!(canonical("select /* a /* b */ c */  1"), "select /* a /* b */ c */  1");
        assert_eq!(canonical("select /* a */  1"), "select 1");
        // MySQL's `#` comments, and a comment that never ends.
        assert_ne!(canonical("# see reylo/*.sql\ncreate table t (a int);"), canonical("# see reylo/*.sql\ncreate table t (a bigint);"));
        assert_ne!(canonical("select 1; /* create table t (a int);"), canonical("select 1; /* create table t (a bigint);"));
        assert_eq!(canonical("select 1 /* a */"), "select 1");
        // A backslash both readings agree on changes nothing.
        assert_eq!(canonical("select 'a\\n' -- x\n,  'b\\\\'"), "select 'a\\n' , 'b\\\\'");
        assert_ne!(checksum("", "a.sql", "create table a (id int)"), checksum("", "a.sql", "create table a (id bigint)"));
    }

    #[test]
    fn the_chain_depends_on_order_and_text() {
        let ab = chain([("a.sql", "x"), ("b.sql", "y")]);
        let ba = chain([("b.sql", "y"), ("a.sql", "x")]);
        assert_ne!(ab[1].sum, ba[0].sum);
        assert_ne!(chain([("a.sql", "x2")])[0].sum, ab[0].sum);
    }
}
