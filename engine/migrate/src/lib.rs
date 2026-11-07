//! `reylo migrate` and its neighbours: migrations written by hand (or by an agent), checked against
//! the declared schema. `reylo migrate prod --dry-run` builds shadows beside the target: one runs
//! `schema.sql`; the other replays what the target applied, is compared with the live target (the
//! drift check), and then runs what is pending; the two must agree. `reylo migrate prod` builds none,
//! and trusts the fingerprint its last run stored. Every migration file runs atomically with its
//! history row, so a file that cannot (a statement that cannot run in a transaction beside others;
//! MySQL's DDL beside anything) is refused.
//!
//! Postgres, MySQL and SQLite. The commands are in `commands`; the comparator is `catalog`; the
//! lexer that splits and classifies statements is `sql`; the connections and shadows are `db`; the
//! replays the shadows run are `replay`, which the cache of what they reached (`cache`) shortens;
//! `lock` is the project's lock file.

pub mod cache;
pub mod catalog;
pub mod commands;
pub mod db;
pub mod diff;
pub mod lock;
mod replay;
pub mod sql;
mod time;

pub use commands::{create, db_introspect, db_push, db_push_watch, db_wipe, dev, dry_run, history_pull, history_push, init, lint, prod, run_sql, Choices, Options};

/// What went wrong, as a sentence for the person running the command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(e.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    Postgres,
    Mysql,
    Sqlite,
}

impl Dialect {
    /// The database a URL's scheme names.
    pub fn of_url(url: &str) -> Result<Dialect, Error> {
        let lower = url.to_ascii_lowercase();
        let scheme = lower.split_once(':').map(|(s, _)| s).unwrap_or_default();
        match scheme {
            "postgres" | "postgresql" => Ok(Dialect::Postgres),
            "mysql" | "mariadb" => Ok(Dialect::Mysql),
            "sqlite" | "file" => Ok(Dialect::Sqlite),
            "duckdb" => Err(Error("reylo migrate does not support DuckDB yet".into())),
            _ if lower == ":memory:" || [".db", ".sqlite", ".sqlite3"].iter().any(|e| lower.ends_with(e)) => Ok(Dialect::Sqlite),
            _ => Err(Error(format!("no database for the URL's scheme: {}", redact(url)))),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Dialect::Postgres => "postgres",
            Dialect::Mysql => "mysql",
            Dialect::Sqlite => "sqlite",
        }
    }
}

/// The URL with its user and password left out, for a message.
pub fn redact(url: &str) -> String {
    match (url.find("://"), url.find('@')) {
        (Some(s), Some(at)) if at > s => format!("{}//…{}", &url[..s + 1], &url[at..]),
        _ => url.to_string(),
    }
}

/// `%XX` escapes decoded, as in a URL's path.
pub(crate) fn percent_decode(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_database_from_the_url() {
        assert_eq!(Dialect::of_url("postgresql://u@h/db").unwrap(), Dialect::Postgres);
        assert_eq!(Dialect::of_url("mysql://root@127.0.0.1:3306/app").unwrap(), Dialect::Mysql);
        assert_eq!(Dialect::of_url("file:./dev.db").unwrap(), Dialect::Sqlite);
        assert!(Dialect::of_url("duckdb:./x.duckdb").unwrap_err().0.contains("DuckDB"));
        assert_eq!(percent_decode("a%20b%2"), "a b%2");
        assert_eq!(redact("postgres://u:p@h/db"), "postgres://…@h/db");
    }
}
