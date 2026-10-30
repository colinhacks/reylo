//! A database's own test suite as a check of the engine: `--replay <database> <path>` replays the tests its authors wrote for it, statement by statement and in order, on a database of the runner's own, and holds every query the engine types against what the database returns. A column typed non-null that holds NULL, or more rows than the cardinality allows, is a contradiction; the run fails on any. The suites are read from a checkout at the version of the server that runs them, never copied:
//!
//! - `duckdb`: `test/sql` of DuckDB's repository (sqllogictest), run from that repository's root ([`duckdb`]).
//! - `postgres`: `src/test/regress` of Postgres's, in the order of its `parallel_schedule`, on the server `--url` names ([`postgres`]).
//! - `sqlite`: `test/*.test` of SQLite's, the SQL its TCL scripts run ([`sqlite`]).
//! - `mysql`: `mysql-test/t/*.test` of MySQL's (mysqltest), on a server the runner starts from `--mysqld` and owns ([`mysql`]).
//!
//! What the runner does not follow it skips (a test of a feature the server lacks, the body of a loop, a second connection), and a statement that fails is no evidence. A statement with parameters is not run. The checks are of soundness alone: what a suite cannot show is whether a type is as tight as it could be, which the cases in `oracle/cases` pin.

#[cfg(feature = "duckdb")]
pub mod duckdb;
pub mod mysql;
pub mod postgres;
pub mod sqlite;

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// What a replay counted, and every contradiction it found.
#[derive(Default)]
pub struct Tally {
    pub files: usize,
    pub files_skipped: usize,
    /// The statements the engine reads (queries and writes), each analyzed.
    pub statements: usize,
    pub analysis_failed: usize,
    pub with_params: usize,
    pub ran: usize,
    pub failed: usize,
    pub columns: usize,
    pub non_null: usize,
    pub bounded: usize,
    /// Statements whose syntax the engine's parser does not read, typed from the describe alone.
    pub unread: usize,
    /// Refusals by Reylo's own rule that no two columns share a name, which no database has.
    pub by_rule: usize,
    /// Other refusals of statements the database ran.
    pub refused: Vec<String>,
    pub contradictions: Vec<String>,
}

/// What one run of a statement returned: its rows, and per column whether any held NULL.
pub struct Returned {
    pub rows: usize,
    pub nulls: Vec<bool>,
}

impl Tally {
    /// Holds what a statement returned against its analysis. `at` places the statement in its file.
    pub fn judge(&mut self, at: &str, sql: &str, analysis: &Value, returned: &Returned) {
        self.ran += 1;
        let shown = || format!("{at}: {}", one_line(sql));
        if let Some(d) = analysis["diagnostics"].as_array().and_then(|d| d.first()) {
            if d["code"].as_u64() == Some(14) {
                self.by_rule += 1;
            } else {
                self.refused.push(format!("{} -- reylo{}: {}", shown(), d["code"], d["message"].as_str().unwrap_or("").lines().next().unwrap_or("")));
            }
            return;
        }
        if analysis["cardinalityReason"].as_str().is_some_and(|r| r.starts_with("the engine does not read")) {
            self.unread += 1;
        }
        let columns = analysis["columns"].as_array().cloned().unwrap_or_default();
        if columns.is_empty() {
            return;
        }
        if columns.len() != returned.nulls.len() {
            self.contradictions.push(format!("{} -- {} column(s) inferred, {} returned", shown(), columns.len(), returned.nulls.len()));
            return;
        }
        self.columns += columns.len();
        for (c, null) in columns.iter().zip(&returned.nulls) {
            let non_null = !c["nullable"].as_bool().unwrap_or(true);
            self.non_null += usize::from(non_null);
            if non_null && *null {
                self.contradictions.push(format!("{} -- column {} is typed non-null and held NULL", shown(), c["name"].as_str().unwrap_or("?")));
            }
        }
        let cardinality = analysis["cardinality"].as_str().unwrap_or("many");
        self.bounded += usize::from(matches!(cardinality, "one" | "maybeOne"));
        let rows = returned.rows;
        if (cardinality == "one" && rows != 1) || (cardinality == "maybeOne" && rows > 1) {
            self.contradictions.push(format!("{} -- inferred {cardinality}, returned {rows} rows", shown()));
        }
    }

    /// Prints what the replay found; whether it found no contradiction.
    pub fn report(&self) -> bool {
        println!("{} file(s), {} skipped (a feature the server or the runner lacks)", self.files, self.files_skipped);
        println!("{} statement(s) the engine reads: {} ran, {} failed on the database, {} take parameters (not run), {} the engine could not analyze", self.statements, self.ran, self.failed, self.with_params, self.analysis_failed);
        println!("{} of {} columns of the statements that ran inferred non-null; {} statements inferred one or maybeOne", self.non_null, self.columns, self.bounded);
        println!("{} of the statements that ran were typed from the describe alone, their syntax beyond the engine's parser", self.unread);
        println!("{} refused by Reylo's own rule that no two columns share a name", self.by_rule);
        println!("{} other statement(s) the engine refused and the database ran:", self.refused.len());
        for r in &self.refused {
            println!("    {r}");
        }
        println!("{} contradiction(s):", self.contradictions.len());
        for c in &self.contradictions {
            println!("    {c}");
        }
        self.contradictions.is_empty()
    }
}

/// Whether a statement is one the engine analyzes and returns rows from or writes: a query or a DML write, by its first word.
pub fn analyzed(sql: &str) -> bool {
    matches!(first_word(sql).as_str(), "select" | "with" | "insert" | "update" | "delete" | "merge" | "replace" | "values" | "table" | "from")
}

pub fn first_word(sql: &str) -> String {
    sql.trim_start_matches(|c: char| c == '(' || c.is_whitespace()).split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_ascii_lowercase()
}

/// A statement on one line, cut to a length a report can show.
pub fn one_line(sql: &str) -> String {
    let joined = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    match joined.char_indices().nth(200) {
        Some((cut, _)) => format!("{}…", &joined[..cut]),
        None => joined,
    }
}

/// Every file under `paths` whose name ends with `extension`, in order.
pub fn files(paths: &[PathBuf], extension: &str) -> Vec<PathBuf> {
    fn walk(path: &Path, extension: &str, out: &mut Vec<PathBuf>) {
        if path.is_file() {
            if path.extension().is_some_and(|x| x == extension) {
                out.push(path.to_path_buf());
            }
            return;
        }
        if let Ok(entries) = fs::read_dir(path) {
            let mut children: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
            children.sort();
            for child in children {
                walk(&child, extension, out);
            }
        }
    }
    let mut out = Vec::new();
    for p in paths {
        walk(p, extension, &mut out);
    }
    out
}
