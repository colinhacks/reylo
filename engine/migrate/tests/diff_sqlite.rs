//! The SQLite diff engine's cases, `tests/diff/sqlite/<case>/`, as `tests/diff.rs` runs Postgres's:
//! `v1.sql` with `seed.sql`'s rows in one database, `v2.sql` in another, the plan from the first to
//! the second run on the first as `reylo migrate` runs a file (foreign keys off, one transaction,
//! `pragma foreign_key_check` before the commit), the first's catalog then required to equal the
//! second's and every table to keep its rows, and the plan compared with `plan.sql`, which
//! `REYLO_BLESS=1` writes. Every SQLite schema the oracle's cases set up is planned from an empty
//! database and back to one. SQLite needs no server: every database is in memory.

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

async fn memory() -> Db {
    Db::connect(":memory:", &std::env::temp_dir()).await.unwrap()
}

async fn plan(from: &mut Db, to: &mut Db) -> Plan {
    diff::plan(&diff::sqlite::read(from).await.unwrap(), &diff::sqlite::read(to).await.unwrap(), &diff::sqlite::SqliteAlterer::default())
}

/// Runs the plan as `reylo migrate` runs a file: the statement that failed, if one did.
async fn run(db: &mut Db, plan: &Plan) -> Option<String> {
    db.exec("pragma foreign_keys = off").await.unwrap();
    db.exec("begin").await.unwrap();
    let mut failed = None;
    for s in plan.runnable() {
        if let Err(e) = db.exec(s).await {
            failed = Some(format!("{s}: {e}"));
            break;
        }
    }
    if failed.is_none() {
        let broken = db.rows("pragma foreign_key_check").await.unwrap();
        if !broken.is_empty() {
            failed = Some(format!("{} rows break a foreign key after the plan", broken.len()));
        }
    }
    db.exec(if failed.is_some() { "rollback" } else { "commit" }).await.unwrap();
    db.exec("pragma foreign_keys = on").await.unwrap();
    failed
}

async fn counts(db: &mut Db) -> Vec<(String, String)> {
    let tables = db.rows("select name from pragma_table_list where schema = 'main' and type in ('table', 'virtual') and name not like 'sqlite_%' order by name").await.unwrap();
    let mut out = Vec::new();
    for t in tables {
        let name = t.text("name").to_string();
        let n = db.rows(&format!("select count(*) as n from \"{}\"", name.replace('"', "\"\""))).await.unwrap();
        out.push((name.to_lowercase(), n[0].text("n").to_string()));
    }
    out
}

/// Each AUTOINCREMENT table's counter, which can be past every key its table holds.
async fn counters(db: &mut Db) -> Vec<(String, i64)> {
    let exists = db.rows("select count(*) as n from sqlite_schema where name = 'sqlite_sequence'").await.unwrap();
    if exists[0].text("n") == "0" {
        return vec![];
    }
    db.rows("select lower(name) as name, seq from sqlite_sequence order by 1").await.unwrap().iter().map(|r| (r.text("name").to_string(), r.text("seq").parse().unwrap())).collect()
}

fn differs(only_a: &[String], only_b: &[String]) -> Option<String> {
    (!only_a.is_empty() || !only_b.is_empty()).then(|| only_a.iter().map(|l| format!("  - {l}")).chain(only_b.iter().map(|l| format!("  + {l}"))).collect::<Vec<_>>().join("\n"))
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/diff/sqlite");
    let cases = only(std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect());
    let bless = std::env::var("REYLO_BLESS").is_ok();
    let mut failures = Vec::new();
    for case in &cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let (mut from, mut to) = (memory().await, memory().await);
        from.exec(&read(&case.join("v1.sql"))).await.unwrap_or_else(|e| panic!("{name}: v1.sql: {e}"));
        from.exec(&read(&case.join("seed.sql"))).await.unwrap_or_else(|e| panic!("{name}: seed.sql: {e}"));
        to.exec(&read(&case.join("v2.sql"))).await.unwrap_or_else(|e| panic!("{name}: v2.sql: {e}"));
        let before = counts(&mut from).await;
        let sequences = counters(&mut from).await;
        let plan = plan(&mut from, &mut to).await;
        let text = plan.sql();
        let failed = run(&mut from, &plan).await;
        let (only_a, only_b) = difference(&catalog(&mut from).await.unwrap(), &catalog(&mut to).await.unwrap());
        let after = counts(&mut from).await;
        let mut lost: Vec<String> = before.iter().filter(|(t, n)| after.iter().any(|(u, m)| u == t && m != n)).map(|(t, n)| format!("{t}: {n} rows before")).collect();
        // A table that keeps AUTOINCREMENT keeps its counter: no key it handed out is handed out again.
        let kept = counters(&mut from).await;
        let autoincrement = from.rows("select lower(name) as name from sqlite_schema where type = 'table' and sql like '%autoincrement%'").await.unwrap();
        for (t, seq) in &sequences {
            let now = kept.iter().find(|(u, _)| u == t).map(|(_, s)| *s);
            if now.is_some_and(|n| n < *seq) || (now.is_none() && autoincrement.iter().any(|r| r.text("name") == t)) {
                lost.push(format!("{t}'s counter: {seq} before, {now:?} after"));
            }
        }
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
        if !bless && read(&expected) != text {
            problems.push(format!("the plan is not plan.sql's (REYLO_BLESS=1 writes it):\n{text}"));
        }
        if !problems.is_empty() {
            failures.push(format!("{name}:\n{}\nplan:\n{text}", problems.join("\n")));
        }
    }
    assert!(failures.is_empty(), "{} of {} cases failed:\n\n{}", failures.len(), cases.len(), failures.join("\n\n"));
}

/// Every SQLite schema the oracle's cases set up (`oracle/cases/<case>/setup.sql` where `dialect`
/// says `sqlite`), built from an empty database by its plan, and dropped back to an empty one by the
/// plan the other way: each direction must end with the other side's catalog.
#[tokio::test(flavor = "current_thread")]
async fn sqlite_oracle_schemas() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/cases");
    let cases = only(std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("setup.sql").exists() && read(&p.join("dialect")).trim() == "sqlite").collect());
    let (mut failures, mut skipped, mut passed) = (Vec::new(), Vec::new(), 0);
    for case in &cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let (mut empty, mut full, mut blank) = (memory().await, memory().await, memory().await);
        if let Err(err) = full.exec(&read(&case.join("setup.sql"))).await {
            skipped.push(format!("{name}: {err}"));
            continue;
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
