//! The DuckDB diff engine's cases, `tests/diff/duckdb/<case>/`, as `tests/diff.rs` runs Postgres's:
//! `v1.sql` with `seed.sql`'s rows in one database, `v2.sql` in another, the plan from the first to
//! the second run on the first as `reylo migrate` runs a file (one transaction), the first's catalog
//! then required to equal the second's, every table both have to keep its rows and every column
//! both have of one type, not generated in the second, its values, and the plan
//! compared with `plan.sql`, which `REYLO_BLESS=1` writes. Each case is planned back from v2 to v1
//! too, which must reach v1 as well (that plan is not kept), and after a plan has run, planning again
//! must find nothing to do. Then every DuckDB schema the oracle's cases set up is planned from an
//! empty database and back to one. Every database is in memory, on the DuckDB library
//! `REYLO_DUCKDB_LIBRARY` names; the tests are skipped where it names none.

use reylo_migrate::catalog::{catalog, difference};
use reylo_migrate::db::Db;
use reylo_migrate::diff::{self, Plan};
use std::path::{Path, PathBuf};

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn only(mut cases: Vec<PathBuf>) -> Vec<PathBuf> {
    cases.sort();
    if let Ok(only) = std::env::var("REYLO_CASE") {
        cases.retain(|p| p.file_name().is_some_and(|n| n.to_string_lossy().contains(&only)));
    }
    cases
}

fn available() -> bool {
    if std::env::var_os("REYLO_DUCKDB_LIBRARY").is_none() {
        eprintln!("skipped: no DuckDB library (REYLO_DUCKDB_LIBRARY)");
        return false;
    }
    true
}

async fn memory() -> Db {
    Db::connect("duckdb::memory:", &std::env::temp_dir()).await.unwrap()
}

async fn plan(from: &mut Db, to: &mut Db) -> Plan {
    diff::plan(&diff::duckdb::read(from).await.unwrap(), &diff::duckdb::read(to).await.unwrap(), &diff::duckdb::DuckdbAlterer::default())
}

/// Runs the plan as `reylo migrate` runs a file, in one transaction: the statement that failed, if one did.
async fn run(db: &mut Db, plan: &Plan) -> Option<String> {
    db.exec("begin").await.unwrap();
    let mut failed = None;
    for s in plan.runnable() {
        if let Err(e) = db.exec(s).await {
            failed = Some(format!("{s}: {e}"));
            break;
        }
    }
    db.exec(if failed.is_some() { "rollback" } else { "commit" }).await.unwrap();
    failed
}

/// The values of every column of `db` that `target` has too, of the same type and not generated
/// there, each table's rows as text in order: what a plan to `target` must keep.
async fn values(db: &mut Db, target: &mut Db) -> Vec<(String, Vec<String>)> {
    let columns = "select schema_name as s, table_name as t, column_name as c, data_type as ty, is_generated as g from duckdb_columns() where database_name = current_database() and not internal order by 1, 2, column_index";
    let (ours, theirs) = (db.rows(columns).await.unwrap(), target.rows(columns).await.unwrap());
    let quote = |n: &str| format!("\"{}\"", n.replace('"', "\"\""));
    let mut tables: Vec<(String, Vec<String>)> = Vec::new();
    for c in &ours {
        let kept = theirs.iter().any(|t| t.text("s") == c.text("s") && t.text("t") == c.text("t") && t.text("c") == c.text("c") && t.text("ty") == c.text("ty") && t.text("g") == "false");
        if !kept {
            continue;
        }
        let table = format!("{}.{}", quote(c.text("s")), quote(c.text("t")));
        match tables.iter_mut().find(|(t, _)| *t == table) {
            Some((_, cols)) => cols.push(quote(c.text("c"))),
            None => tables.push((table, vec![quote(c.text("c"))])),
        }
    }
    let mut out = Vec::new();
    for (table, cols) in tables {
        let list = cols.iter().map(|c| format!("coalesce({c}::varchar, 'NULL')")).collect::<Vec<_>>().join(", ");
        let Ok(rows) = db.rows(&format!("select concat_ws('|', {list}) as r from {table} order by 1")).await else { continue };
        out.push((format!("{table} ({})", cols.join(", ")), rows.iter().map(|r| r.text("r").to_string()).collect()));
    }
    out
}

async fn counts(db: &mut Db) -> Vec<(String, String)> {
    let tables = db.rows("select schema_name as s, table_name as t from duckdb_tables() where database_name = current_database() and not temporary order by 1, 2").await.unwrap();
    let mut out = Vec::new();
    for t in tables {
        let name = format!("\"{}\".\"{}\"", t.text("s").replace('"', "\"\""), t.text("t").replace('"', "\"\""));
        let n = db.rows(&format!("select count(*) as n from {name}")).await.unwrap();
        out.push((format!("{}.{}", t.text("s"), t.text("t")), n[0].text("n").to_string()));
    }
    out
}

fn differs(only_a: &[String], only_b: &[String]) -> Option<String> {
    (!only_a.is_empty() || !only_b.is_empty()).then(|| only_a.iter().map(|l| format!("  - {l}")).chain(only_b.iter().map(|l| format!("  + {l}"))).collect::<Vec<_>>().join("\n"))
}

#[tokio::test(flavor = "current_thread")]
async fn duckdb() {
    if !available() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/diff/duckdb");
    let cases = only(std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect());
    let bless = std::env::var("REYLO_BLESS").is_ok();
    let mut failures = Vec::new();
    for case in &cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let (mut from, mut to) = (memory().await, memory().await);
        from.exec(&read(&case.join("v1.sql"))).await.unwrap_or_else(|e| panic!("{name}: v1.sql: {e}"));
        let seed = read(&case.join("seed.sql"));
        if !seed.trim().is_empty() {
            from.exec(&seed).await.unwrap_or_else(|e| panic!("{name}: seed.sql: {e}"));
        }
        to.exec(&read(&case.join("v2.sql"))).await.unwrap_or_else(|e| panic!("{name}: v2.sql: {e}"));
        let before = counts(&mut from).await;
        let kept = values(&mut from, &mut to).await;
        let plan = plan(&mut from, &mut to).await;
        let text = plan.sql();
        let failed = run(&mut from, &plan).await;
        let failed_before = failed.clone();
        let (only_a, only_b) = difference(&catalog(&mut from).await.unwrap(), &catalog(&mut to).await.unwrap());
        let after = counts(&mut from).await;
        let lost: Vec<String> = before.iter().filter(|(t, n)| after.iter().any(|(u, m)| u == t && m != n)).map(|(t, n)| format!("{t}: {n} rows before")).collect();
        let expected = case.join("plan.sql");
        if bless {
            std::fs::write(&expected, &text).unwrap();
        }
        let mut problems = Vec::new();
        if let Some(f) = failed {
            problems.push(format!("the plan failed at {f}"));
        }
        if let Some(d) = differs(&only_a, &only_b) {
            problems.push(format!("after the plan the database differs from v2:\n{d}"));
        }
        if !lost.is_empty() {
            problems.push(format!("rows lost: {}", lost.join(", ")));
        }
        // Every column kept as it was keeps its values.
        if failed_before.is_none() {
            let now = values(&mut from, &mut to).await;
            for (columns, rows) in &kept {
                if let Some((_, after)) = now.iter().find(|(c, _)| c == columns) {
                    if after != rows {
                        problems.push(format!("values changed in {columns}:\n  before {rows:?}\n  after  {after:?}"));
                    }
                }
            }
        }
        // Planned again, the database it reached needs nothing.
        let again = self::plan(&mut from, &mut to).await;
        if !again.statements.is_empty() {
            problems.push(format!("planned again, it still plans:\n{}", again.sql()));
        }
        // And back: from v2 to v1, as a rewind plans it.
        let (mut back_from, mut back_to) = (memory().await, memory().await);
        back_from.exec(&read(&case.join("v2.sql"))).await.unwrap();
        back_to.exec(&read(&case.join("v1.sql"))).await.unwrap();
        let back = self::plan(&mut back_from, &mut back_to).await;
        let back_failed = run(&mut back_from, &back).await;
        let (back_a, back_b) = difference(&catalog(&mut back_from).await.unwrap(), &catalog(&mut back_to).await.unwrap());
        if back_failed.is_some() || !back_a.is_empty() || !back_b.is_empty() {
            problems.push(format!("planned back to v1, {}:\n{}\nplan:\n{}", back_failed.unwrap_or_else(|| "it differs".into()), differs(&back_a, &back_b).unwrap_or_default(), back.sql()));
        }
        if !bless && read(&expected) != text {
            problems.push(format!("the plan is not plan.sql's (REYLO_BLESS=1 writes it):\n{text}"));
        }
        if !problems.is_empty() {
            failures.push(format!("{name}:\n{}\nplan:\n{text}", problems.join("\n")));
        }
    }
    assert!(failures.is_empty(), "{} of {} cases failed:\n\n{}", failures.len(), cases.len(), failures.join("\n\n"));
}

/// Every DuckDB schema the oracle's cases set up (`oracle/cases/<case>/setup.sql` where `dialect`
/// says `duckdb`), built from an empty database by its plan, and dropped back to an empty one by the
/// plan the other way: each direction must end with the other side's catalog.
#[tokio::test(flavor = "current_thread")]
async fn duckdb_oracle_schemas() {
    if !available() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/cases");
    let cases = only(std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("setup.sql").exists() && read(&p.join("dialect")).trim() == "duckdb").collect());
    let (mut failures, mut skipped, mut passed) = (Vec::new(), Vec::new(), 0);
    for case in &cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let (mut empty, mut full, mut blank) = (memory().await, memory().await, memory().await);
        if let Err(err) = full.exec(&read(&case.join("setup.sql"))).await {
            skipped.push(format!("{name}: {err}"));
            continue;
        }
        // A temporary table is the session's, which no migration's session starts with: one that a
        // setup leaves shadows its table's name in the statements of a plan.
        for t in full.rows("select table_name as t from duckdb_tables() where temporary").await.unwrap() {
            full.exec(&format!("drop table temp.main.\"{}\"", t.text("t").replace('"', "\"\""))).await.unwrap();
        }
        let mut problems = Vec::new();
        for direction in ["from empty", "to empty"] {
            let (from, to) = if direction == "from empty" { (&mut empty, &mut full) } else { (&mut full, &mut blank) };
            let plan = plan(from, to).await;
            let failed = run(from, &plan).await;
            let (only_a, only_b) = difference(&catalog(from).await.unwrap(), &catalog(to).await.unwrap());
            if failed.is_some() || !only_a.is_empty() || !only_b.is_empty() {
                let mut lines: Vec<String> = failed.into_iter().map(|f| format!("  failed at {f}")).collect();
                lines.extend(only_a.iter().take(8).map(|l| format!("  - {l}")));
                lines.extend(only_b.iter().take(8).map(|l| format!("  + {l}")));
                problems.push(format!("{direction}:\n{}\nplan:\n{}", lines.join("\n"), plan.sql()));
            }
        }
        if problems.is_empty() {
            passed += 1;
        } else {
            failures.push(format!("{name}: {}", problems.join("\n")));
        }
    }
    eprintln!("{passed} schemas planned both ways; {} could not be set up here:\n{}", skipped.len(), skipped.join("\n"));
    assert!(skipped.is_empty(), "{} schemas could not be set up:\n{}", skipped.len(), skipped.join("\n"));
    assert!(failures.is_empty(), "{} of {} schemas failed:\n\n{}", failures.len(), cases.len(), failures.join("\n\n"));
}
