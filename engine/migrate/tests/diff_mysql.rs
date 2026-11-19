//! The MySQL diff engine's cases, `tests/diff/mysql/<case>/`, as `tests/diff.rs` runs Postgres's:
//! `v1.sql` (with `seed.sql`'s rows) is the database as it is, `v2.sql` the schema it should have;
//! each case builds both, plans from one to the other, runs the plan on the first, and requires its
//! catalog to equal the second's, every table both have to keep its rows, and every column kept to
//! keep its values, unless the plan says it loses or narrows them or the case's `changes.txt` names
//! the table. A case's `sql_mode` file is the mode its sessions run in. `server.sql` makes
//! what a case needs of the server itself (an account), and `check.sql`, where a case has one,
//! runs after the plan, so a case can require what no catalog shows (an AUTO_INCREMENT counter
//! past the rows). The plan is kept beside the case as `plan.sql`: `REYLO_BLESS=1` writes it.
//! Then every MySQL schema the oracle's cases set up is built from an empty database by its plan
//! and dropped back to an empty one. The comparator leaves events out, so these tests compare
//! them themselves. Each case is planned back from v2 to v1 too, which must reach v1 as well
//! (that plan is not kept). After a plan has run, planning again must find nothing to do. Run against the server `REYLO_TEST_MYSQL` names, and skipped where it is not set.

use reylo_migrate::catalog::{catalog, difference};
use reylo_migrate::db::{with_database, Db};
use reylo_migrate::diff;
use std::path::{Path, PathBuf};

fn cases() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/diff/mysql");
    let mut out: Vec<PathBuf> = std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
    out.sort();
    if let Ok(only) = std::env::var("REYLO_CASE") {
        out.retain(|p| p.file_name().is_some_and(|n| n.to_string_lossy().contains(&only)));
    }
    out
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Every table's row count.
async fn counts(db: &mut Db) -> Vec<(String, String)> {
    let tables = db.rows("select table_name as t from information_schema.tables where table_schema = database() and table_type = 'BASE TABLE' order by 1").await.unwrap();
    let mut out = Vec::new();
    for t in tables {
        let n = db.rows(&format!("select count(*) as n from `{}`", t.text("t"))).await.unwrap();
        out.push((t.text("t").to_string(), n[0].text("n").to_string()));
    }
    out
}

/// Each table's rows, as text, column by column (but generated ones, whose values are their
/// expression's): `(table, column names, rows)`.
async fn values(db: &mut Db) -> Vec<(String, Vec<String>, Vec<Vec<String>>)> {
    let tables = db.rows("select table_name as t from information_schema.tables where table_schema = database() and table_type = 'BASE TABLE' order by 1").await.unwrap();
    let mut out = Vec::new();
    for t in tables {
        let table = t.text("t").to_string();
        let columns: Vec<String> = db
            .rows(&format!("select column_name as c from information_schema.columns where table_schema = database() and table_name = '{}' and extra not like '%GENERATED%' order by ordinal_position", table.replace('\'', "''")))
            .await
            .unwrap()
            .iter()
            .map(|r| r.text("c").to_string())
            .collect();
        if columns.is_empty() {
            continue;
        }
        let list = columns.iter().enumerate().map(|(k, c)| format!("coalesce(hex(`{}`), 'NULL') as v{k}", c.replace('`', "``"))).collect::<Vec<_>>().join(", ");
        let mut rows: Vec<Vec<String>> = db.rows(&format!("select {list} from `{table}`")).await.unwrap().iter().map(|r| (0..columns.len()).map(|k| r.text(&format!("v{k}")).to_string()).collect()).collect();
        rows.sort();
        out.push((table.to_lowercase(), columns.iter().map(|c| c.to_lowercase()).collect(), rows));
    }
    out
}

/// The tables a plan changed the values of, in the columns they kept.
fn changed(before: &[(String, Vec<String>, Vec<Vec<String>>)], after: &[(String, Vec<String>, Vec<Vec<String>>)]) -> Vec<String> {
    let mut out = Vec::new();
    for (table, columns, rows) in before {
        let Some((_, now_columns, now_rows)) = after.iter().find(|(t, ..)| t == table) else { continue };
        let both: Vec<(usize, usize)> = columns.iter().enumerate().filter_map(|(k, c)| now_columns.iter().position(|n| n == c).map(|j| (k, j))).collect();
        let pick = |rows: &[Vec<String>], side: usize| -> Vec<Vec<String>> {
            let mut out: Vec<Vec<String>> = rows.iter().map(|r| both.iter().map(|(k, j)| r[if side == 0 { *k } else { *j }].clone()).collect()).collect();
            out.sort();
            out
        };
        if pick(rows, 0) != pick(now_rows, 1) {
            out.push(table.clone());
        }
    }
    out
}

/// The database's events, which the comparator does not read, with the database's own name taken
/// out and when each starts left out (the server fixes it as the event is made).
async fn events(db: &mut Db) -> Vec<String> {
    let name = db.rows("select database() as n").await.unwrap()[0].text("n").to_string();
    db.rows(
        "select concat_ws(' ', event_name, event_type, interval_value, interval_field, status, on_completion, definer, event_comment, event_definition) as line
        from information_schema.events where event_schema = database() order by event_name",
    )
    .await
    .unwrap()
    .iter()
    .map(|r| r.text("line").replace(&format!("`{name}`."), "").replace(&format!("{name}."), ""))
    .collect()
}

/// The lines the two databases differ in, the comparator's and the events'.
async fn differences(a: &mut Db, b: &mut Db) -> (Vec<String>, Vec<String>) {
    let (mut only_a, mut only_b) = difference(&catalog(a).await.unwrap(), &catalog(b).await.unwrap());
    let (ea, eb) = difference(&events(a).await, &events(b).await);
    only_a.extend(ea.into_iter().map(|l| format!("event {l}")));
    only_b.extend(eb.into_iter().map(|l| format!("event {l}")));
    (only_a, only_b)
}

async fn recreate(admin: &mut Db, name: &str) {
    admin.exec(&format!("drop database if exists `{name}`")).await.unwrap();
    admin.exec(&format!("create database `{name}`")).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn mysql() {
    let Ok(base) = std::env::var("REYLO_TEST_MYSQL") else {
        eprintln!("skipped: REYLO_TEST_MYSQL is not set");
        return;
    };
    let here = std::env::temp_dir();
    let mut admin = Db::connect(&base, &here).await.unwrap();
    let bless = std::env::var("REYLO_BLESS").is_ok();
    let mut failures = Vec::new();
    for (n, case) in cases().into_iter().enumerate() {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let (a, b) = (format!("diff_{}_{n}_a", std::process::id()), format!("diff_{}_{n}_b", std::process::id()));
        admin.script(&read(&case.join("server.sql"))).await.unwrap_or_else(|e| panic!("{name}: server.sql: {e}"));
        for db in [&a, &b] {
            recreate(&mut admin, db).await;
        }
        let mut from = Db::connect(&with_database(&base, &a), &here).await.unwrap();
        let mut to = Db::connect(&with_database(&base, &b), &here).await.unwrap();
        // A case's `sql_mode` file is the mode its sessions run in, which the alterer reads.
        let mode = read(&case.join("sql_mode")).trim().to_string();
        if !mode.is_empty() {
            for db in [&mut from, &mut to] {
                db.exec(&format!("set session sql_mode = '{mode}'")).await.unwrap();
            }
        }
        let alterer = diff::alterer(&mut from).await.unwrap();
        let alterer = alterer.as_ref();
        from.script(&read(&case.join("v1.sql"))).await.unwrap_or_else(|e| panic!("{name}: v1.sql: {e}"));
        from.script(&read(&case.join("seed.sql"))).await.unwrap_or_else(|e| panic!("{name}: seed.sql: {e}"));
        to.script(&read(&case.join("v2.sql"))).await.unwrap_or_else(|e| panic!("{name}: v2.sql: {e}"));
        let before = counts(&mut from).await;
        let rows_before = values(&mut from).await;
        let plan = diff::plan(&diff::mysql::read(&mut from).await.unwrap(), &diff::mysql::read(&mut to).await.unwrap(), alterer);
        let text = plan.sql();
        let mut failed = None;
        for s in plan.runnable() {
            if let Err(e) = from.exec(s).await {
                failed = Some(format!("{s}: {e}"));
                break;
            }
        }
        let (only_a, only_b) = differences(&mut from, &mut to).await;
        let again = diff::plan(&diff::mysql::read(&mut from).await.unwrap(), &diff::mysql::read(&mut to).await.unwrap(), alterer).sql();
        let after = counts(&mut from).await;
        let lost: Vec<String> = before.iter().filter(|(t, n)| after.iter().any(|(u, m)| u == t && m != n)).map(|(t, n)| format!("{t}: {n} rows before")).collect();
        // Every column a table keeps keeps its values, where neither the plan (as a loss or a
        // narrowing) nor the case (`changes.txt`) says the change rewrites them.
        let declared = plan.losses.iter().chain(plan.narrowings.iter().map(|(c, _)| c)).filter_map(|id| id.strip_prefix("column ")?.rsplit_once('.').map(|(t, _)| t.trim_matches('`').to_lowercase()));
        let skip: Vec<String> = read(&case.join("changes.txt")).lines().filter_map(|l| l.split_whitespace().next()).map(|t| t.to_lowercase()).chain(declared).collect();
        let rewritten: Vec<String> = if failed.is_none() { changed(&rows_before, &values(&mut from).await).into_iter().filter(|t| !skip.contains(t)).collect() } else { vec![] };
        let check = read(&case.join("check.sql"));
        let checked = if failed.is_none() && !check.is_empty() { from.script(&check).await.err() } else { None };
        let expected = case.join("plan.sql");
        if bless {
            std::fs::write(&expected, &text).unwrap();
        }
        let mut problems = Vec::new();
        if let Some(f) = failed {
            problems.push(format!("the plan failed at {f}"));
        }
        if !only_a.is_empty() || !only_b.is_empty() {
            problems.push(format!("after the plan the database differs from v2:\n{}", only_a.iter().map(|l| format!("  - {l}")).chain(only_b.iter().map(|l| format!("  + {l}"))).collect::<Vec<_>>().join("\n")));
        }
        if !lost.is_empty() {
            problems.push(format!("rows lost: {}", lost.join(", ")));
        }
        if !rewritten.is_empty() {
            problems.push(format!("values changed in: {}", rewritten.join(", ")));
        }
        if !again.is_empty() {
            problems.push(format!("after the plan, the plan again is not empty:\n{again}"));
        }
        if let Some(e) = checked {
            problems.push(format!("check.sql failed after the plan: {e}"));
        }
        if !bless && read(&expected) != text {
            problems.push(format!("the plan is not plan.sql's (REYLO_BLESS=1 writes it):\n{text}"));
        }
        // And back: v2 (without rows) to v1, which takes the opposite path through each change.
        let c = format!("diff_{}_{n}_c", std::process::id());
        recreate(&mut admin, &c).await;
        let mut back = Db::connect(&with_database(&base, &c), &here).await.unwrap();
        if !mode.is_empty() {
            back.exec(&format!("set session sql_mode = '{mode}'")).await.unwrap();
        }
        back.script(&read(&case.join("v1.sql"))).await.unwrap();
        let reverse = diff::plan(&diff::mysql::read(&mut to).await.unwrap(), &diff::mysql::read(&mut back).await.unwrap(), alterer);
        let mut failed = None;
        for s in reverse.runnable() {
            if let Err(e) = to.exec(s).await {
                failed = Some(format!("{s}: {e}"));
                break;
            }
        }
        let (only_a, only_b) = differences(&mut to, &mut back).await;
        let again = diff::plan(&diff::mysql::read(&mut to).await.unwrap(), &diff::mysql::read(&mut back).await.unwrap(), alterer).sql();
        if failed.is_some() || !only_a.is_empty() || !only_b.is_empty() || !again.is_empty() {
            let mut lines: Vec<String> = failed.into_iter().map(|f| format!("  failed at {f}")).collect();
            lines.extend(only_a.iter().map(|l| format!("  - {l}")).chain(only_b.iter().map(|l| format!("  + {l}"))));
            lines.extend(again.lines().map(|l| format!("  again: {l}")));
            problems.push(format!("back from v2 to v1:\n{}\nplan back:\n{}", lines.join("\n"), reverse.sql()));
        }
        back.close().await;
        admin.exec(&format!("drop database if exists `{c}`")).await.unwrap();
        if !problems.is_empty() {
            failures.push(format!("{name}:\n{}\nplan:\n{text}", problems.join("\n")));
        }
        from.close().await;
        to.close().await;
        for db in [&a, &b] {
            admin.exec(&format!("drop database if exists `{db}`")).await.unwrap();
        }
    }
    admin.close().await;
    assert!(failures.is_empty(), "{} cases failed:\n\n{}", failures.len(), failures.join("\n\n"));
}

/// Every MySQL schema the oracle's cases set up (`oracle/cases/<case>/setup.sql`, where the case's
/// `dialect` file says `mysql`), built from an empty database by its plan, and dropped back to an
/// empty one by the plan the other way: each direction must end with the other side's catalog. A
/// case's `sql_mode` file is the mode of every session it runs in.
#[tokio::test(flavor = "current_thread")]
async fn mysql_oracle_schemas() {
    let Ok(base) = std::env::var("REYLO_TEST_MYSQL") else {
        eprintln!("skipped: REYLO_TEST_MYSQL is not set");
        return;
    };
    let here = std::env::temp_dir();
    let mut admin = Db::connect(&base, &here).await.unwrap();
    let alterer = diff::mysql::MysqlAlterer::default();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/cases");
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("setup.sql").exists() && read(&p.join("dialect")).trim() == "mysql").collect();
    cases.sort();
    if let Ok(only) = std::env::var("REYLO_CASE") {
        cases.retain(|p| p.file_name().is_some_and(|n| n.to_string_lossy().contains(&only)));
    }
    let (mut failures, mut skipped, mut passed) = (Vec::new(), Vec::new(), 0);
    for (n, case) in cases.iter().enumerate() {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let (a, b, e) = (format!("odiff_{}_{n}_a", std::process::id()), format!("odiff_{}_{n}_b", std::process::id()), format!("odiff_{}_{n}_e", std::process::id()));
        for db in [&a, &b, &e] {
            recreate(&mut admin, db).await;
        }
        let mut empty = Db::connect(&with_database(&base, &a), &here).await.unwrap();
        let mut full = Db::connect(&with_database(&base, &b), &here).await.unwrap();
        let mut blank = Db::connect(&with_database(&base, &e), &here).await.unwrap();
        let mode = read(&case.join("sql_mode"));
        if !mode.trim().is_empty() {
            for db in [&mut empty, &mut full, &mut blank] {
                db.exec(&format!("set session sql_mode = '{}'", mode.trim())).await.unwrap();
            }
        }
        if let Err(err) = full.script(&read(&case.join("setup.sql"))).await {
            skipped.push(format!("{name}: {err}"));
        } else {
            let mut problems = Vec::new();
            for direction in ["from empty", "to empty"] {
                let (from, to) = if direction == "from empty" { (&mut empty, &mut full) } else { (&mut full, &mut blank) };
                let plan = diff::plan(&diff::mysql::read(from).await.unwrap(), &diff::mysql::read(to).await.unwrap(), &alterer);
                let mut failed = None;
                for s in plan.runnable() {
                    if let Err(err) = from.exec(s).await {
                        failed = Some(format!("{s}: {err}"));
                        break;
                    }
                }
                let (only_a, only_b) = differences(from, to).await;
                let again = diff::plan(&diff::mysql::read(from).await.unwrap(), &diff::mysql::read(to).await.unwrap(), &alterer).sql();
                if failed.is_some() || !only_a.is_empty() || !only_b.is_empty() || !again.is_empty() {
                    let mut lines: Vec<String> = failed.into_iter().map(|f| format!("  failed at {f}")).collect();
                    lines.extend(again.lines().map(|l| format!("  again: {l}")));
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
        for db in [empty, full, blank] {
            db.close().await;
        }
        for db in [&a, &b, &e] {
            admin.exec(&format!("drop database if exists `{db}`")).await.unwrap();
        }
    }
    admin.close().await;
    eprintln!("{passed} schemas planned both ways; {} could not be set up here:\n{}", skipped.len(), skipped.join("\n"));
    assert!(skipped.is_empty() && failures.is_empty(), "{} of {} schemas failed, {} could not be set up:\n\n{}\n\n{}", failures.len(), cases.len(), skipped.len(), failures.join("\n\n"), skipped.join("\n"));
}
