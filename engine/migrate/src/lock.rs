//! `reylo-lock.yaml`, the lock of a locked project (the default; `history: loose` keeps none): the
//! migrations in the order they run, each its name and a checksum chained to the line before, and
//! the checksum of `schema.sql` from the last time the migrations were proven to reach it. Two
//! branches that each add a migration both append to it, so git conflicts on it when the second one
//! merges, which is the point: before anything is deployed, one branch's migrations must move after
//! the other's (keep main's lines, then `reylo migrate apply --dev`). The chain makes an edit, a
//! deletion or a reordering visible without a database: a wrong "accept both" breaks it at the
//! first line out of place. Only Reylo writes it, after applying what it lists to a development
//! database (`apply --dev`) or reading what a database applied (`lock`), so its checksums certify
//! what ran.

use crate::cache::{hex, Sha256};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::LazyLock;

pub const LOCK: &str = "reylo-lock.yaml";

const HEADER: &str = "# The order this project's migrations run in, written by reylo migrate apply --dev and reylo migrate
# lock. A merge conflict here means another branch added migrations too: keep main's lines, then
# run reylo migrate apply --dev, which runs this branch's after them and adds them back.
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
/// (`'…'`, `"…"`, `` `…` ``, and Postgres's `$tag$…$tag$`) is kept as written.
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
            while k < b.len() && b[k] != '\n' {
                k += 1;
            }
            space = true;
        } else if c == '/' && b.get(k + 1) == Some(&'*') {
            let mut depth = 0;
            while k < b.len() {
                if b[k] == '/' && b.get(k + 1) == Some(&'*') {
                    depth += 1;
                    k += 2;
                } else if b[k] == '*' && b.get(k + 1) == Some(&'/') {
                    depth -= 1;
                    k += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    k += 1;
                }
            }
            space = true;
        } else if c.is_whitespace() {
            space = true;
            k += 1;
        } else if c == '\'' || c == '"' || c == '`' {
            let start = k;
            k += 1;
            while k < b.len() {
                if b[k] == c {
                    if b.get(k + 1) == Some(&c) {
                        k += 2;
                        continue;
                    }
                    k += 1;
                    break;
                }
                k += 1;
            }
            push(&mut out, &mut space, &b[start..k].iter().collect::<String>());
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
