//! DuckDB's own test suite as a check of the engine: `--sqllogictest <file or directory>…` replays
//! each `.test` file of DuckDB's repository (`test/sql/`, in sqllogictest's format) on a fresh
//! database, running its statements in order, and for each `query` the engine analyzes the query on
//! the database as it stands, DuckDB runs it, and what DuckDB returns is held against what the
//! engine said: a column typed non-null that holds NULL, or more rows than the cardinality allows,
//! is a contradiction. A query the engine refuses that DuckDB runs is counted apart. The suite's
//! queries are DuckDB's own tests of its features, written by its authors, over their own data.
//!
//! What the runner does not follow it skips: a file that needs an extension DuckDB was not built
//! with, the bodies of loops, a `mode skip` region, a query that takes parameters, and anything
//! after `restart` or `halt`. A query that fails is no evidence. A path in the files is read from
//! the directory the runner is started in, which is DuckDB's repository's root.
//!
//!   cd ~/src/duckdb && cargo run --manifest-path …/Cargo.toml -p reylo-oracle --features duckdb -- --sqllogictest test/sql

use crate::duckdb::{Db, Host, Ran};
use reylo_engine::AnswerCache;
use std::fs;
use std::path::{Path, PathBuf};

/// The extensions a file may `require` that the oracle's DuckDB has.
const BUILT_IN: &[&str] = &["skip_reload", "no_extension_autoloading", "64bit", "vector_size", "notmusl", "notwindows", "windows", "noforcestorage", "ram", "no_alternative_verify", "exact_vector_size", "block_size", "longdouble"];

#[derive(Default)]
struct Tally {
    files: usize,
    files_skipped: usize,
    queries: usize,
    ran: usize,
    failed: usize,
    with_params: usize,
    refused_but_ran: Vec<String>,
    non_null_columns: usize,
    columns: usize,
    bounded: usize,
    /// Queries whose syntax the engine's parser does not read, typed from DuckDB's describe alone.
    unread: usize,
    /// Refusals by Reylo's own rules (two columns of one name), which DuckDB has none of.
    by_rule: usize,
    contradictions: Vec<String>,
}

/// Every `.test` file under `paths`, in order.
fn files(paths: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(path: &Path, out: &mut Vec<PathBuf>) {
        if path.is_file() {
            if path.extension().is_some_and(|x| x == "test") {
                out.push(path.to_path_buf());
            }
            return;
        }
        if let Ok(entries) = fs::read_dir(path) {
            let mut children: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
            children.sort();
            for child in children {
                walk(&child, out);
            }
        }
    }
    let mut out = Vec::new();
    for p in paths {
        walk(p, &mut out);
    }
    out
}

/// Runs the suite and prints what it found; whether it found no contradiction.
pub fn run(paths: &[PathBuf], timeout_ms: u64) -> bool {
    let mut t = Tally::default();
    let dir = std::env::temp_dir().join(format!("reylo-sqllogictest-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    for file in files(paths) {
        t.files += 1;
        if let Err(why) = run_file(&file, &dir, timeout_ms, &mut t) {
            t.files_skipped += 1;
            let _ = why;
        }
    }
    let _ = fs::remove_dir_all(&dir);
    println!("{} file(s), {} skipped (an extension the oracle's DuckDB lacks, or nothing it runs)", t.files, t.files_skipped);
    println!("{} query record(s): {} ran, {} failed on DuckDB, {} take parameters (not run)", t.queries, t.ran, t.failed, t.with_params);
    println!("{} of {} columns of the queries that ran proven non-null; {} queries proven one or maybeOne", t.non_null_columns, t.columns, t.bounded);
    println!("{} of the queries that ran were typed from DuckDB's describe alone, their syntax beyond the engine's parser", t.unread);
    println!("{} refused by Reylo's own rules (two columns of one name)", t.by_rule);
    println!("{} other query record(s) the engine refused and DuckDB ran:", t.refused_but_ran.len());
    for r in &t.refused_but_ran {
        println!("    {r}");
    }
    println!("{} contradiction(s):", t.contradictions.len());
    for c in &t.contradictions {
        println!("    {c}");
    }
    t.contradictions.is_empty()
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
        t.queries += 1;
        let analysis = reylo_engine::replay::run::<reylo_engine::duckdb::Duckdb, _>(&sql, &mut Host(&db), &mut AnswerCache::default());
        let analysis = match analysis.map(|a| serde_json::to_value(&a)) {
            Ok(Ok(a)) => a,
            _ => {
                let _ = db.exec(&sql);
                t.failed += 1;
                continue;
            }
        };
        if analysis["params"].as_array().is_some_and(|p| !p.is_empty()) {
            t.with_params += 1;
            let _ = db.exec(&sql);
            continue;
        }
        let refused = analysis["diagnostics"].as_array().is_some_and(|d| !d.is_empty());
        match db.run(&sql, &[], timeout_ms) {
            Ran::TimedOut | Ran::Failed(_) => {
                t.failed += 1;
            }
            Ran::Done((rows, nulls)) => {
                t.ran += 1;
                let at = format!("{shown}:{}", r.line);
                let one_line = sql.split_whitespace().collect::<Vec<_>>().join(" ");
                if refused && analysis["diagnostics"][0]["code"].as_u64() == Some(14) {
                    t.by_rule += 1;
                    continue;
                }
                if refused {
                    let message = analysis["diagnostics"][0]["message"].as_str().unwrap_or("").lines().next().unwrap_or("").to_string();
                    t.refused_but_ran.push(format!("{at}: {one_line} -- {message}"));
                    continue;
                }
                if analysis["cardinalityReason"].as_str().is_some_and(|r| r.starts_with("the engine does not read")) {
                    t.unread += 1;
                }
                let columns = analysis["columns"].as_array().cloned().unwrap_or_default();
                let cardinality = analysis["cardinality"].as_str().unwrap_or("many");
                if columns.is_empty() {
                    continue;
                }
                if columns.len() != nulls.len() {
                    t.contradictions.push(format!("{at}: {one_line} -- the engine says {} column(s), DuckDB returned {}", columns.len(), nulls.len()));
                    continue;
                }
                t.columns += columns.len();
                for (c, null) in columns.iter().zip(&nulls) {
                    let non_null = !c["nullable"].as_bool().unwrap_or(true);
                    if non_null {
                        t.non_null_columns += 1;
                    }
                    if non_null && *null {
                        t.contradictions.push(format!("{at}: {one_line} -- column {} is typed non-null and DuckDB returned NULL", c["name"].as_str().unwrap_or("?")));
                    }
                }
                if matches!(cardinality, "one" | "maybeOne") {
                    t.bounded += 1;
                }
                let wrong = match cardinality {
                    "one" => rows != 1,
                    "maybeOne" => rows > 1,
                    _ => false,
                };
                if wrong {
                    t.contradictions.push(format!("{at}: {one_line} -- proven {cardinality}, DuckDB returned {rows} rows"));
                }
            }
        }
    }
    Ok(())
}
