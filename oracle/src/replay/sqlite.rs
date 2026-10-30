//! SQLite's own tests: the SQL of the TCL scripts in `test/*.test`, the brace-quoted bodies of `do_execsql_test`, `do_catchsql_test`, `do_eqp_test`, `execsql`, `catchsql` and `db eval`, each split into its statements and run in the order the script writes them, on a fresh database file per script and again at each `reset_db`. The suite is read from a checkout at the version of the SQLite the oracle runs statements on (rusqlite's bundled one):
//!
//!   git clone --depth 1 --branch version-3.51.3 https://github.com/sqlite/sqlite
//!   cargo run -p reylo-oracle -- --replay sqlite sqlite/test
//!
//! TCL that the runner does not follow it skips: a script that registers its own functions, collations or modules, or uses test controls or fault simulation, is skipped whole; a body inside a loop runs once; a body for a second connection (`} db2`) is skipped. A `$name` inside a body is a parameter TCL binds, so the statement is not run.

use super::{Returned, Tally};
use reylo_engine::AnswerCache;
use std::path::{Path, PathBuf};

/// A script that registers or controls what plain SQLite lacks, skipped whole.
const SKIPPED: &[&str] = &["load_static_extension", "sqlite3_test_control", "optimization_control", "do_faultsim_test", "db func", "db function", "db collate", "sqlite3_db_config", "register_"];

/// The commands whose brace-quoted argument is SQL.
const RUNS_SQL: &[&str] = &["do_execsql_test", "do_catchsql_test", "do_eqp_test", "execsql", "catchsql", "db eval"];

/// Replays the scripts under `paths`; whether it found no contradiction.
pub fn run(paths: &[PathBuf], timeout_ms: u64) -> bool {
    let mut t = Tally::default();
    let dir = std::env::temp_dir().join(format!("reylo-replay-sqlite-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let paths: Vec<PathBuf> = paths.iter().map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone())).collect();
    // The scripts name files relative to where they run (`ATTACH 'test2.db'`): here, the runner's own directory.
    let back = std::env::current_dir().ok();
    let _ = std::env::set_current_dir(&dir);
    for file in super::files(&paths, "test") {
        t.files += 1;
        let Ok(text) = std::fs::read_to_string(&file) else {
            t.files_skipped += 1;
            continue;
        };
        if text.lines().any(|l| SKIPPED.iter().any(|s| l.trim_start().starts_with(s))) {
            t.files_skipped += 1;
            continue;
        }
        run_script(&file, &text, &dir.join("test.db"), timeout_ms, &mut t);
    }
    if let Some(back) = back {
        let _ = std::env::set_current_dir(back);
    }
    let _ = std::fs::remove_dir_all(&dir);
    t.report()
}

fn fresh(path: &Path) -> Option<rusqlite::Connection> {
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
    rusqlite::Connection::open(path).ok()
}

fn run_script(file: &Path, text: &str, path: &Path, timeout_ms: u64, t: &mut Tally) {
    let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let Some(mut db) = fresh(path) else { return };
    for block in blocks(text) {
        let body = match block {
            Block::Reset => {
                drop(db);
                let Some(next) = fresh(path) else { return };
                db = next;
                continue;
            }
            Block::Sql(body) => body,
        };
        for sql in statements(&body) {
            if !super::analyzed(&sql) || sql.to_ascii_lowercase().contains("load_extension") {
                let _ = crate::seed::with_deadline(&db, timeout_ms, || db.execute_batch(&sql));
                continue;
            }
            t.statements += 1;
            let analysis = match reylo_engine::clients::rusqlite::analyze(&db, &sql, &mut AnswerCache::default()) {
                Ok(a) => serde_json::to_value(&a).unwrap_or_default(),
                Err(_) => {
                    t.analysis_failed += 1;
                    let _ = crate::seed::with_deadline(&db, timeout_ms, || db.execute_batch(&sql));
                    continue;
                }
            };
            if analysis["params"].as_array().is_some_and(|p| !p.is_empty()) {
                t.with_params += 1;
                continue;
            }
            match crate::seed::with_deadline(&db, timeout_ms, || returned(&db, &sql)) {
                Ok(r) => t.judge(&name, &sql, &analysis, &r),
                Err(_) => t.failed += 1,
            }
        }
    }
}

/// The rows a statement returned, and per column whether any held NULL.
fn returned(db: &rusqlite::Connection, sql: &str) -> rusqlite::Result<Returned> {
    let mut statement = db.prepare(sql)?;
    let mut out = Returned { rows: 0, nulls: vec![false; statement.column_count()] };
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        out.rows += 1;
        for (i, null) in out.nulls.iter_mut().enumerate() {
            *null |= matches!(row.get_ref(i)?, rusqlite::types::ValueRef::Null);
        }
    }
    Ok(out)
}

enum Block {
    /// The SQL argument of a command that runs it.
    Sql(String),
    /// `reset_db`, or `forcedelete test.db`: a fresh database from here on.
    Reset,
}

/// The SQL blocks of a script, in order. A command's SQL is its first brace-quoted argument, after at most one other (a test's name); one followed by another connection's name (`} db2`) is that connection's, and left out.
fn blocks(text: &str) -> Vec<Block> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let at_word = |at: usize| at == 0 || !(b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_');
    while i < b.len() {
        if !text.is_char_boundary(i) || !at_word(i) {
            i += 1;
            continue;
        }
        let rest = &text[i..];
        if rest.starts_with("reset_db") || rest.starts_with("forcedelete test.db") {
            out.push(Block::Reset);
            i += 8;
            continue;
        }
        let Some(command) = RUNS_SQL.iter().find(|c| rest.starts_with(**c) && !rest[c.len()..].starts_with(|ch: char| ch.is_alphanumeric() || ch == '_')) else {
            i += 1;
            continue;
        };
        let mut j = i + command.len();
        let skip_space = |mut at: usize| {
            while at < b.len() && (b[at] == b' ' || b[at] == b'\t' || (b[at] == b'\\' && b.get(at + 1) == Some(&b'\n'))) {
                at += if b[at] == b'\\' { 2 } else { 1 };
            }
            at
        };
        j = skip_space(j);
        // A test's name before its SQL: a word, or a brace-quoted one on one line.
        let named = command.starts_with("do_");
        if named && j < b.len() && b[j] != b'{' {
            let start = j;
            while j < b.len() && !b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j == start {
                i += 1;
                continue;
            }
            j = skip_space(j);
        } else if let Some(close) = text[j..].find('}').map(|e| j + e).filter(|e| named && !text[j + 1..*e].contains(['{', '\n'])) {
            let after = skip_space(close + 1);
            if after < b.len() && b[after] == b'{' {
                j = after;
            }
        }
        if j >= b.len() || b[j] != b'{' {
            i += 1;
            continue;
        }
        let (body, end) = braced(text, j + 1);
        let next = skip_space(end);
        let other_connection = text.get(next..).is_some_and(|r| r.starts_with("db") && r[2..].starts_with(|c: char| c.is_ascii_digit()));
        if !other_connection && !body.trim().is_empty() {
            out.push(Block::Sql(body.to_string()));
        }
        i = end;
    }
    out
}

/// The text from `start` to its matching `}`, and the offset after it; a backslash escapes the character after it.
fn braced(text: &str, start: usize) -> (&str, usize) {
    let b = text.as_bytes();
    let (mut depth, mut j) = (1usize, start);
    while j < b.len() {
        match b[j] {
            b'\\' => j += 1,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return (&text[start..j], j + 1);
                }
            }
            _ => {}
        }
        j += 1;
    }
    (&text[start..], b.len())
}

/// A block's statements, each up to the `;` that completes it as SQLite reads it.
fn statements(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in body.char_indices() {
        if c != ';' {
            continue;
        }
        let Ok(piece) = std::ffi::CString::new(&body[start..=i]) else { continue };
        // SAFETY: a NUL-terminated string that outlives the call.
        if unsafe { rusqlite::ffi::sqlite3_complete(piece.as_ptr()) } != 0 {
            let statement = body[start..i].trim();
            if !statement.is_empty() {
                out.push(statement.to_string());
            }
            start = i + 1;
        }
    }
    let rest = body[start..].trim();
    if !rest.is_empty() {
        out.push(rest.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_s_sql_is_read_in_order_and_another_connection_s_left_out() {
        let script = "do_execsql_test 1.0 {\n  CREATE TABLE t(a); INSERT INTO t VALUES('x;y');\n} {}\nexecsql {SELECT 1}\ndo_catchsql_test {name with space} { SELECT 2 } {0 2}\nreset_db\ndb eval { SELECT 3; SELECT 4 }\ndb eval {SELECT 7} { lappend r $a }\ncatchsql { SELECT 5 } db2\nmy_execsql_test {SELECT 6}\n";
        let shown: Vec<String> = blocks(script)
            .into_iter()
            .map(|b| match b {
                Block::Sql(s) => statements(&s).join(" | "),
                Block::Reset => "reset".into(),
            })
            .collect();
        assert_eq!(shown, ["CREATE TABLE t(a) | INSERT INTO t VALUES('x;y')", "SELECT 1", "SELECT 2", "reset", "SELECT 3 | SELECT 4", "SELECT 7"]);
    }
}
