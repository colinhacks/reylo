//! DuckDB's own test suite: each `.test` file of `test/sql` (sqllogictest's format) on a fresh database, its statements run in order, and each `query` analyzed on the database as it stands, then run. A file that needs an extension the oracle's DuckDB was not built with is skipped, and so are the bodies of loops, a `mode skip` region and everything after `restart` or `halt`. A path in the files is read from the directory the runner is started in, which is DuckDB's repository's root:
//!
//!   cd ~/src/duckdb && cargo run --manifest-path …/Cargo.toml -p reylo-oracle --features duckdb -- --replay duckdb test/sql

use super::{Returned, Tally};
use crate::duckdb::{Db, Host, Ran};
use reylo_engine::AnswerCache;
use std::fs;
use std::path::{Path, PathBuf};

/// The extensions a file may `require` that the oracle's DuckDB has.
const BUILT_IN: &[&str] = &["skip_reload", "no_extension_autoloading", "64bit", "vector_size", "notmusl", "notwindows", "windows", "noforcestorage", "ram", "no_alternative_verify", "exact_vector_size", "block_size", "longdouble"];

/// Replays the suite under `paths`; whether it found no contradiction.
pub fn run(paths: &[PathBuf], timeout_ms: u64) -> bool {
    let mut t = Tally::default();
    let dir = std::env::temp_dir().join(format!("reylo-replay-duckdb-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    for file in super::files(paths, "test") {
        t.files += 1;
        if run_file(&file, &dir, timeout_ms, &mut t).is_err() {
            t.files_skipped += 1;
        }
    }
    let _ = fs::remove_dir_all(&dir);
    t.report()
}

/// One record: its kind's line, its SQL, and its line number.
struct Record {
    head: String,
    sql: String,
    line: usize,
}

fn records(text: &str) -> Vec<Record> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        if line.trim().is_empty() || line.starts_with('#') {
            i += 1;
            continue;
        }
        let head = line.trim().to_string();
        let start = i + 1;
        i += 1;
        let takes_sql = head.starts_with("statement") || head.starts_with("query");
        let mut sql = Vec::new();
        if takes_sql {
            while i < lines.len() && !lines[i].trim().is_empty() && lines[i].trim() != "----" {
                sql.push(lines[i]);
                i += 1;
            }
            // The expected result or error, up to the blank line that ends the record.
            if i < lines.len() && lines[i].trim() == "----" {
                while i < lines.len() && !lines[i].trim().is_empty() {
                    i += 1;
                }
            }
        }
        out.push(Record { head, sql: sql.join("\n"), line: start });
    }
    out
}

fn run_file(file: &Path, dir: &Path, timeout_ms: u64, t: &mut Tally) -> Result<(), String> {
    let text = fs::read_to_string(file).map_err(|e| e.to_string())?;
    let records = records(&text);
    for r in &records {
        if let Some(ext) = r.head.strip_prefix("require ") {
            if !BUILT_IN.contains(&ext.trim()) {
                return Err(format!("requires {ext}"));
            }
        }
    }
    let db_file = dir.join("test.duckdb");
    let _ = fs::remove_file(&db_file);
    let _ = fs::remove_file(db_file.with_extension("duckdb.wal"));
    let db = Db::open(&db_file)?;
    let test_dir = dir.to_string_lossy().to_string();
    let shown = file.to_string_lossy().to_string();
    let (mut depth, mut skipping, mut condition_skip) = (0usize, false, false);
    for r in &records {
        let head = r.head.as_str();
        let word = head.split_whitespace().next().unwrap_or("");
        match word {
            "loop" | "foreach" | "concurrentloop" | "concurrentforeach" => {
                depth += 1;
                continue;
            }
            "endloop" => {
                depth = depth.saturating_sub(1);
                continue;
            }
            "mode" => {
                skipping = head.contains("skip") && !head.contains("unskip");
                continue;
            }
            "restart" | "halt" | "load" | "reconnect" => break,
            "onlyif" => {
                condition_skip = !head.ends_with("duckdb");
                continue;
            }
            "skipif" => {
                condition_skip = head.ends_with("duckdb");
                continue;
            }
            _ => {}
        }
        let skip = depth > 0 || skipping || std::mem::take(&mut condition_skip);
        if skip || !(word == "statement" || word == "query") {
            continue;
        }
        let sql = r.sql.replace("__TEST_DIR__", &test_dir).replace("{TEST_DIR}", &test_dir);
        if word == "statement" {
            let _ = db.exec(&sql);
            continue;
        }
        t.statements += 1;
        let analysis = match reylo_engine::replay::run::<reylo_engine::duckdb::Duckdb, _>(&sql, &mut Host(&db), &mut AnswerCache::default()).map(|a| serde_json::to_value(&a)) {
            Ok(Ok(a)) => a,
            _ => {
                let _ = db.exec(&sql);
                t.analysis_failed += 1;
                continue;
            }
        };
        if analysis["params"].as_array().is_some_and(|p| !p.is_empty()) {
            t.with_params += 1;
            let _ = db.exec(&sql);
            continue;
        }
        match db.run(&sql, &[], timeout_ms) {
            Ran::TimedOut | Ran::Failed(_) => t.failed += 1,
            Ran::Done((rows, nulls)) => t.judge(&format!("{shown}:{}", r.line), &sql, &analysis, &Returned { rows, nulls }),
        }
    }
    Ok(())
}
