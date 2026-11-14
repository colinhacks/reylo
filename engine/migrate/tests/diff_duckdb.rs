//! The DuckDB diff engine's cases, `tests/diff/duckdb/<case>/`, as `tests/diff.rs` runs Postgres's:
//! `v1.sql` with `seed.sql`'s rows in one database, `v2.sql` in another, the plan from the first to
//! the second run on the first as `reylo migrate` runs a file (one transaction), the first's catalog
//! then required to equal the second's, every table both have to keep its rows and every column
//! both have of one type, not generated in the second, its values, and the plan
//! compared with `plan.sql`, which `REYLO_BLESS=1` writes. A case with a `mid.sql` is planned there
//! first, and from where that plan leaves it to v2. Each case is planned back from v2 to v1
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

/// The values of every table's column of `db` that `target` has too, of the same type and not
/// generated there, each table's rows as text in order: what a plan to `target` must keep.
async fn values(db: &mut Db, target: &mut Db) -> Vec<(String, Vec<String>)> {
    // A table's: a view's values are what its query makes of them, which the plan may rightly change.
    let columns = "select schema_name as s, table_name as t, column_name as c, data_type as ty, is_generated as g from duckdb_columns() where database_name = current_database() and not internal and table_oid in (select table_oid from duckdb_tables()) order by 1, 2, column_index";
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

/// What goes wrong planning `v1` (with `seed`'s rows) to `v2`, and back, and the plan: the plan run in
/// one transaction, the catalog then held to v2's, every table's rows counted, every column kept as
/// it was held to its values, the database planned again, and v2 planned back to v1.
async fn problems_of(v1: &str, seed: &str, v2: &str) -> (Vec<String>, String) {
    problems_via(v1, seed, None, v2).await
}

/// The same from the database `mid` (a case's `mid.sql`) leaves, where given: v1 with the seed's
/// rows planned to `mid` first, as an earlier migration takes it there, so that what DuckDB recorded
/// then (a default's macro, read again since by another of its name) is what the plan meets.
async fn problems_via(v1: &str, seed: &str, mid: Option<&str>, v2: &str) -> (Vec<String>, String) {
    let (mut from, mut to) = (memory().await, memory().await);
    for (db, file, sql) in [(&mut from, "v1.sql", v1), (&mut to, "v2.sql", v2)] {
        if let Err(e) = db.exec(sql).await {
            return (vec![format!("{file} doesn't run: {e}")], String::new());
        }
    }
    if !seed.trim().is_empty() {
        if let Err(e) = from.exec(seed).await {
            return (vec![format!("seed.sql doesn't run: {e}")], String::new());
        }
    }
    if let Some(mid) = mid {
        let mut at = memory().await;
        if let Err(e) = at.exec(mid).await {
            return (vec![format!("mid.sql doesn't run: {e}")], String::new());
        }
        let first = plan(&mut from, &mut at).await;
        if let Some(f) = run(&mut from, &first).await {
            return (vec![format!("the plan to mid.sql failed at {f}:\n{}", first.sql())], String::new());
        }
        let (a, b) = difference(&catalog(&mut from).await.unwrap(), &catalog(&mut at).await.unwrap());
        if let Some(d) = differs(&a, &b) {
            return (vec![format!("after the plan to mid.sql the database differs from it:\n{d}\nplan:\n{}", first.sql())], String::new());
        }
    }
    let before = counts(&mut from).await;
    let kept = values(&mut from, &mut to).await;
    let plan = plan(&mut from, &mut to).await;
    let text = plan.sql();
    let failed = run(&mut from, &plan).await;
    let (only_a, only_b) = difference(&catalog(&mut from).await.unwrap(), &catalog(&mut to).await.unwrap());
    let after = counts(&mut from).await;
    let lost: Vec<String> = before.iter().filter(|(t, n)| after.iter().any(|(u, m)| u == t && m != n)).map(|(t, n)| format!("{t}: {n} rows before")).collect();
    let mut problems = Vec::new();
    if let Some(f) = &failed {
        problems.push(format!("the plan failed at {f}"));
    }
    if let Some(d) = differs(&only_a, &only_b) {
        problems.push(format!("after the plan the database differs from v2:\n{d}"));
    }
    if !lost.is_empty() {
        problems.push(format!("rows lost: {}", lost.join(", ")));
    }
    // Every column kept as it was keeps its values.
    if failed.is_none() {
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
    back_from.exec(v2).await.unwrap();
    back_to.exec(v1).await.unwrap();
    let back = self::plan(&mut back_from, &mut back_to).await;
    let back_failed = run(&mut back_from, &back).await;
    let (back_a, back_b) = difference(&catalog(&mut back_from).await.unwrap(), &catalog(&mut back_to).await.unwrap());
    if back_failed.is_some() || !back_a.is_empty() || !back_b.is_empty() {
        problems.push(format!("planned back to v1, {}:\n{}\nplan:\n{}", back_failed.unwrap_or_else(|| "it differs".into()), differs(&back_a, &back_b).unwrap_or_default(), back.sql()));
    }
    (problems, text)
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
        let mid = case.join("mid.sql");
        let mid = mid.exists().then(|| read(&mid));
        let (mut problems, text) = problems_via(&read(&case.join("v1.sql")), &read(&case.join("seed.sql")), mid.as_deref(), &read(&case.join("v2.sql"))).await;
        let expected = case.join("plan.sql");
        if bless {
            std::fs::write(&expected, &text).unwrap();
        } else if read(&expected) != text {
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

// Sweeps: a macro, a sequence and an enum, each used on a parent and on its child every way, changing
// alone and together, with a column added to either, in schemas of their own or not.

/// A sweep: a parent and a child, each with some uses of a macro, a sequence and an enum, and
/// some of those three changing, with or without a column added on either side.
#[tokio::test(flavor = "current_thread")]
async fn a_macro_a_sequence_and_an_enum_change_under_a_parent_and_its_child() {
    if !available() {
        return;
    }
    let p_uses = ["check (m(n))", "n2 integer default m2(1)", "s bigint default nextval('seq')", "e mood", "check (cast(v as mood) is not null)", ""];
    let c_uses = ["check (m(k))", "k2 integer default m2(2)", "s bigint default nextval('seq')", "e mood", "check (cast(w as mood) is not null)", ""];
    let mut failures = vec![];
    let mut n = 0;
    for (pi, pu) in p_uses.iter().enumerate() {
        for (ci, cu) in c_uses.iter().enumerate() {
            for change in 0..8u32 {
                for grow in 0..3u32 {
                    let objs = |v2: bool| -> String {
                        let m = if v2 && change & 1 != 0 { "x > -1" } else { "x > 0" };
                        let m2 = if v2 && change & 1 != 0 { "x + 2" } else { "x + 1" };
                        let seq = if v2 && change & 2 != 0 { "create sequence seq increment by 2;" } else { "create sequence seq;" };
                        let mood = if v2 && change & 4 != 0 { "('a', 'b', 'c')" } else { "('a', 'b')" };
                        let pg = if v2 && grow == 1 { ", extra integer" } else { "" };
                        let cg = if v2 && grow == 2 { ", extra integer" } else { "" };
                        let pu = if pu.is_empty() { String::new() } else { format!(", {pu}") };
                        let cu = if cu.is_empty() { String::new() } else { format!(", {cu}") };
                        format!("create type mood as enum {mood}; {seq} create macro m(x) as {m}; create macro m2(x) as {m2}; create table p (id integer primary key, n integer, v varchar{pu}{pg}); create table c (id integer primary key, pid integer references p (id), k integer, w varchar{cu}{cg});")
                    };
                    if change == 0 && grow == 0 {
                        continue;
                    }
                    n += 1;
                    let seed = "insert into p (id, n, v) values (1, 1, 'a'), (2, 2, 'b'); insert into c (id, pid, k, w) values (1, 1, 1, 'a'), (2, 2, 2, 'b');";
                    let (problems, text) = problems_of(&objs(false), seed, &objs(true)).await;
                    if !problems.is_empty() {
                        failures.push(format!("#### p={pi} c={ci} change={change} grow={grow}\nv2: {}\n{}\nplan:\n{text}", objs(true), problems.join("\n")));
                    }
                }
            }
        }
    }
    println!("sweep: {n} cases, {} failed", failures.len());
    for f in failures.iter().take(6) {
        println!("{f}");
    }
    assert!(failures.is_empty());
}

/// Two uses on the parent at once.
#[tokio::test(flavor = "current_thread")]
async fn two_uses_change_on_a_parent_at_once() {
    if !available() {
        return;
    }
    let uses = ["check (m(n))", "n2 integer default m2(1)", "s bigint default nextval('seq')", "e mood default 'a'", "check (cast(v as mood) is not null)", "i integer, check (i is null or i > 0)"];
    let c_uses = ["check (m(k))", "k2 integer default m2(2)", ""];
    let mut failures = vec![];
    let mut n = 0;
    for a in 0..uses.len() {
        for b in a + 1..uses.len() {
            for cu in c_uses {
                for change in 1..8u32 {
                    let objs = |v2: bool| -> String {
                        let m = if v2 && change & 1 != 0 { "x > -1" } else { "x > 0" };
                        let m2 = if v2 && change & 1 != 0 { "x + 2" } else { "x + 1" };
                        let seq = if v2 && change & 2 != 0 { "create sequence seq increment by 2;" } else { "create sequence seq;" };
                        let mood = if v2 && change & 4 != 0 { "('a', 'b', 'c')" } else { "('a', 'b')" };
                        let cu = if cu.is_empty() { String::new() } else { format!(", {cu}") };
                        format!("create type mood as enum {mood}; {seq} create macro m(x) as {m}; create macro m2(x) as {m2}; create table p (id integer primary key, n integer, v varchar, {}, {}); create table c (id integer primary key, pid integer references p (id), k integer{cu});", uses[a], uses[b])
                    };
                    n += 1;
                    let seed = "insert into p (id, n, v) values (1, 1, 'a'), (2, 2, 'b'); insert into c (id, pid, k) values (1, 1, 1), (2, 2, 2);";
                    let (problems, text) = problems_of(&objs(false), seed, &objs(true)).await;
                    if !problems.is_empty() {
                        failures.push(format!("#### a={a} b={b} c={cu} change={change}\nv2: {}\n{}\nplan:\n{text}", objs(true), problems.join("\n")));
                    }
                }
            }
        }
    }
    println!("sweep pairs: {n} cases, {} failed", failures.len());
    for f in failures.iter().take(6) {
        println!("{f}");
    }
    assert!(failures.is_empty());
}

/// Objects in schema s, written qualified, used by tables in main or in s.
#[tokio::test(flavor = "current_thread")]
async fn uses_in_another_schema_written_with_it_change() {
    if !available() {
        return;
    }
    let uses = ["check (s.m(n))", "n2 integer default s.m2(1)", "q bigint default nextval('s.seq')", "e s.mood", "check (cast(v as s.mood) is not null)", "i integer"];
    let mut failures = vec![];
    let mut n = 0;
    for home in ["main", "s"] {
        for a in 0..uses.len() {
            for b in a + 1..uses.len() {
                for change in 1..8u32 {
                    let objs = |v2: bool| -> String {
                        let m = if v2 && change & 1 != 0 { "x > -1" } else { "x > 0" };
                        let m2 = if v2 && change & 1 != 0 { "x + 2" } else { "x + 1" };
                        let seq = if v2 && change & 2 != 0 { "create sequence s.seq increment by 2;" } else { "create sequence s.seq;" };
                        let mood = if v2 && change & 4 != 0 { "('a', 'b', 'c')" } else { "('a', 'b')" };
                        format!("create schema s; create type s.mood as enum {mood}; {seq} create macro s.m(x) as {m}; create macro s.m2(x) as {m2}; create table {home}.p (id integer primary key, n integer, v varchar, {}, {}); create table {home}.c (id integer primary key, pid integer references {home}.p (id), k integer, check (s.m(k)));", uses[a], uses[b])
                    };
                    n += 1;
                    let seed = format!("insert into {home}.p (id, n, v) values (1, 1, 'a'), (2, 2, 'b'); insert into {home}.c (id, pid, k) values (1, 1, 1), (2, 2, 2);");
                    let (problems, text) = problems_of(&objs(false), &seed, &objs(true)).await;
                    if !problems.is_empty() {
                        failures.push(format!("#### home={home} a={a} b={b} change={change}\nv2: {}\n{}\nplan:\n{text}", objs(true), problems.first().cloned().unwrap_or_default()));
                    }
                }
            }
        }
    }
    println!("sweep schemas: {n} cases, {} failed", failures.len());
    for f in &failures {
        println!("{}", f.lines().next().unwrap_or_default());
    }
    for f in failures.iter().take(3) {
        println!("{f}");
    }
    assert!(failures.is_empty());
}

/// Objects in schema s, used by tables in s, defaults and column types written bare.
#[tokio::test(flavor = "current_thread")]
async fn uses_written_bare_in_another_schema_change() {
    if !available() {
        return;
    }
    let uses = ["check (s.m(n))", "n2 integer default m2(1)", "q bigint default nextval('seq')", "e mood", "check (cast(v as s.mood) is not null)", "y varchar default ('a'::mood)::varchar"];
    let mut failures = vec![];
    let mut n = 0;
    for shadow_main in [false, true] {
        for a in 0..uses.len() {
            for b in a + 1..uses.len() {
                for change in 1..8u32 {
                    let objs = |v2: bool| -> String {
                        let m = if v2 && change & 1 != 0 { "x > -1" } else { "x > 0" };
                        let m2 = if v2 && change & 1 != 0 { "x + 2" } else { "x + 1" };
                        let seq = if v2 && change & 2 != 0 { "create sequence s.seq increment by 2;" } else { "create sequence s.seq;" };
                        let mood = if v2 && change & 4 != 0 { "('a', 'b', 'c')" } else { "('a', 'b')" };
                        let mains = if shadow_main { "create macro m2(x) as x + 7; create sequence seq; create type mood as enum ('z');" } else { "" };
                        format!("create schema s; {mains} create type s.mood as enum {mood}; {seq} create macro s.m(x) as {m}; create macro s.m2(x) as {m2}; create table s.p (id integer primary key, n integer, v varchar, {}, {}); create table s.c (id integer primary key, pid integer references s.p (id), k integer, check (s.m(k)));", uses[a], uses[b])
                    };
                    n += 1;
                    let seed = "insert into s.p (id, n, v) values (1, 1, 'a'), (2, 2, 'b'); insert into s.c (id, pid, k) values (1, 1, 1), (2, 2, 2);";
                    let (problems, text) = problems_of(&objs(false), seed, &objs(true)).await;
                    if !problems.is_empty() {
                        failures.push(format!("#### main={shadow_main} a={a} b={b} change={change}\nv2: {}\n{}\nplan:\n{text}", objs(true), problems.first().cloned().unwrap_or_default()));
                    }
                }
            }
        }
    }
    println!("sweep bare in s: {n} cases, {} failed", failures.len());
    for f in &failures {
        println!("{}", f.lines().next().unwrap_or_default());
    }
    for f in failures.iter().take(3) {
        println!("{f}");
    }
    assert!(failures.is_empty());
}

/// A macro, a sequence or an enum in `main`, in `b` or in both, before and after, `main`'s changing or
/// not, under a default of a table in `b` or in `main` that names it bare: DuckDB reads such a name in
/// the table's own schema first, so a default of `b` moves to `b`'s as soon as it is made.
#[tokio::test(flavor = "current_thread")]
async fn bare_defaults_move_across_schemas() {
    if !available() {
        return;
    }
    let sets: [&[&str]; 3] = [&["main"], &["b"], &["main", "b"]];
    let mut failures = vec![];
    let mut n = 0;
    for kind in ["macro", "sequence", "enum"] {
        for (i1, v1s) in sets.iter().enumerate() {
            for (i2, v2s) in sets.iter().enumerate() {
                for change in [false, true] {
                    for table_schema in ["b", "main"] {
                        let objs = |set: &[&str], v2: bool| -> String {
                            set.iter()
                                .map(|s| {
                                    let q = if *s == "main" { String::new() } else { format!("{s}.") };
                                    let k = if *s != "main" { 2 } else if v2 && change { 7 } else { 1 };
                                    match kind {
                                        "macro" => format!("create macro {q}valid() as {k};"),
                                        "sequence" => format!("create sequence {q}sq increment by {k};"),
                                        _ => format!("create type {q}mood as enum ('a', 'l{k}');"),
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(" ")
                        };
                        let default = match kind {
                            "macro" => "d integer default valid()",
                            "sequence" => "d bigint default nextval('sq')",
                            _ => "d varchar default 'a'::mood",
                        };
                        let t = if table_schema == "b" { "b.p" } else { "p" };
                        let db = |set: &[&str], v2: bool| format!("create schema b; {} create table {t} (id integer, {default}); create table {}q (id integer);", objs(set, v2), if table_schema == "b" { "" } else { "b." });
                        let (v1, v2) = (db(v1s, false), db(v2s, true));
                        if v1 == v2 {
                            continue;
                        }
                        let (problems, text) = problems_of(&v1, &format!("insert into {t} (id) values (1);"), &v2).await;
                        // A main table's default with no main object to read: DuckDB refuses it.
                        if problems.iter().any(|p| p.contains("doesn't run")) {
                            continue;
                        }
                        n += 1;
                        if !problems.is_empty() {
                            failures.push(format!("#### {kind} v1={i1} v2={i2} change={change} table={table_schema}\nv1: {v1}\nv2: {v2}\n{}\nplan:\n{text}", problems.first().cloned().unwrap_or_default()));
                        }
                    }
                }
            }
        }
    }
    println!("sweep bare defaults across schemas: {n} cases, {} failed", failures.len());
    for f in &failures {
        println!("{}", f.lines().next().unwrap_or_default());
    }
    for f in failures.iter().take(3) {
        println!("{f}");
    }
    assert!(failures.is_empty());
}

/// A parent dropped while its child drops its key on it, each with one use of a macro, a sequence or
/// an enum (a check or a default), one of which changes: where both use what changes, the child's
/// rebuild around it drops the parent between.
#[tokio::test(flavor = "current_thread")]
async fn a_parent_dropped_while_what_it_and_its_child_use_changes() {
    if !available() {
        return;
    }
    let uses = ["check (m(n))", "n2 integer default m2(1)", "s bigint default nextval('seq')", "check (cast(v as mood) is not null)", "v2 varchar default 'a'::mood", ""];
    let mut failures = vec![];
    let mut n = 0;
    for (xi, xu) in uses.iter().enumerate() {
        for (yi, yu) in uses.iter().enumerate() {
            for change in [1u32, 2, 4, 8] {
                let objs = |v2: bool| -> String {
                    let m = if v2 && change & 1 != 0 { "x > -1" } else { "x > 0" };
                    let m2 = if v2 && change & 8 != 0 { "x + 2" } else { "x + 1" };
                    let seq = if v2 && change & 2 != 0 { "create sequence seq increment by 2;" } else { "create sequence seq;" };
                    let mood = if v2 && change & 4 != 0 { "('a', 'b', 'c')" } else { "('a', 'b')" };
                    format!("create type mood as enum {mood}; {seq} create macro m(x) as {m}; create macro m2(x) as {m2};")
                };
                let xu = if xu.is_empty() { String::new() } else { format!(", {xu}") };
                let yu = if yu.is_empty() { String::new() } else { format!(", {yu}") };
                let v1 = format!("{} create table x (id integer primary key, n integer, v varchar{xu}); create table y (id integer primary key, xid integer references x (id), n integer, v varchar{yu});", objs(false));
                let v2 = format!("{} create table y (id integer primary key, xid integer, n integer, v varchar{yu});", objs(true));
                n += 1;
                let seed = "insert into x (id, n, v) values (1, 1, 'a'); insert into y (id, xid, n, v) values (1, 1, 1, 'a');";
                let (problems, text) = problems_of(&v1, seed, &v2).await;
                if !problems.is_empty() {
                    failures.push(format!("#### x={xi} y={yi} change={change}\nv1: {v1}\nv2: {v2}\n{}\nplan:\n{text}", problems.first().cloned().unwrap_or_default()));
                }
            }
        }
    }
    println!("sweep dropped parent: {n} cases, {} failed", failures.len());
    for f in &failures {
        println!("{}", f.lines().next().unwrap_or_default());
    }
    for f in failures.iter().take(3) {
        println!("{f}");
    }
    assert!(failures.is_empty());
}
