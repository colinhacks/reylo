//! The locks a plan holds on a table with rows, measured: each case in `tests/diff/postgres-locks/`
//! builds `v1.sql` with `seed.sql`'s rows (a few million, so that reading them all takes time),
//! plans to `v2.sql`, and runs the plan's lock-safe files (`reylo_migrate::locks`) while another
//! connection runs `writer.sql` over and over. The writer's longest wait must stay short, and the
//! files must reach `v2.sql`. The same plan, run as one file the way it was written before the
//! lock-safe shape, must make the writer wait several times longer: that is what shows the case
//! measures a lock at all. Run against the server `REYLO_TEST_PG` names, skipped where it is not set.

use reylo_migrate::catalog::{catalog, difference};
use reylo_migrate::db::{with_database, Db};
use reylo_migrate::{diff, locks, sql::runs_alone, Dialect};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The longest the writer may wait while the lock-safe files run: the brief locks they take
/// (adding a constraint `not valid`, `set not null` once a check proves it) last milliseconds.
const BRIEF: Duration = Duration::from_millis(300);

/// How many times longer the writer must wait behind the plan run as one file.
const LONGER: u32 = 3;

fn cases() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/diff/postgres-locks");
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

/// Runs `files` in order, each as `deploy` runs a file, while a second connection runs `writer`
/// over and over: the writer's longest wait, and how many times it ran.
async fn with_writer(url: &str, db: &mut Db, files: &[String], writer: &str) -> Result<(Duration, usize), String> {
    let stop = Arc::new(AtomicBool::new(false));
    let (url, writer, flag) = (url.to_string(), writer.to_string(), stop.clone());
    let task = tokio::spawn(async move {
        let mut db = Db::connect(&url, &std::env::temp_dir()).await.unwrap();
        let (mut longest, mut runs) = (Duration::ZERO, 0usize);
        while !flag.load(Ordering::Relaxed) {
            let at = Instant::now();
            db.exec(&writer).await.unwrap();
            longest = longest.max(at.elapsed());
            runs += 1;
            tokio::task::yield_now().await;
        }
        db.close().await;
        (longest, runs)
    });
    // The writer is running before the first file is.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut failed = None;
    for file in files {
        let sql = if runs_alone(file, Dialect::Postgres) { file.clone() } else { format!("begin;\n{file}\ncommit;") };
        if let Err(e) = db.exec(&sql).await {
            failed = Some(format!("{e}\n{file}"));
            let _ = db.exec("rollback").await;
            break;
        }
    }
    stop.store(true, Ordering::Relaxed);
    let (longest, runs) = task.await.unwrap();
    match failed {
        Some(e) => Err(e),
        None => Ok((longest, runs)),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn postgres_locks() {
    let Ok(base) = std::env::var("REYLO_TEST_PG") else {
        eprintln!("skipped: REYLO_TEST_PG is not set");
        return;
    };
    let here = std::env::temp_dir();
    let mut admin = Db::connect(&base, &here).await.unwrap();
    let version: i64 = admin.rows("select current_setting('server_version_num') as v").await.unwrap()[0].text("v").parse().unwrap();
    let alterer = diff::postgres::PostgresAlterer::read(&mut admin, version).await.unwrap();
    let mut failures = Vec::new();
    for (n, case) in cases().into_iter().enumerate() {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let [safe, whole, target] = ["s", "w", "t"].map(|x| format!("locks_{}_{n}_{x}", std::process::id()));
        for db in [&safe, &whole, &target] {
            admin.exec(&format!("drop database if exists {db} with (force)")).await.unwrap();
            admin.exec(&format!("create database {db}")).await.unwrap();
        }
        let mut to = Db::connect(&with_database(&base, &target), &here).await.unwrap();
        to.exec(&read(&case.join("v2.sql"))).await.unwrap_or_else(|e| panic!("{name}: v2.sql: {e}"));
        let mut wanted = diff::postgres::read(&mut to).await.unwrap();
        diff::postgres::declare(&mut to, &mut wanted, &read(&case.join("v2.sql"))).await.unwrap();
        let writer = read(&case.join("writer.sql"));
        let mut results = Vec::new();
        for (db_name, lock_safe) in [(&safe, true), (&whole, false)] {
            let url = with_database(&base, db_name);
            let mut from = Db::connect(&url, &here).await.unwrap();
            from.exec(&read(&case.join("v1.sql"))).await.unwrap_or_else(|e| panic!("{name}: v1.sql: {e}"));
            from.exec(&read(&case.join("seed.sql"))).await.unwrap_or_else(|e| panic!("{name}: seed.sql: {e}"));
            from.exec("vacuum analyze").await.unwrap();
            let mut plan = diff::plan(&diff::postgres::read(&mut from).await.unwrap(), &wanted, &alterer);
            diff::postgres::style(&mut from, &mut plan, &wanted.verbatim).await.unwrap();
            let existing = locks::existing_tables(&mut from).await.unwrap();
            let files = match (lock_safe, locks::lock_safe(&plan, &existing)) {
                (true, Some(files)) => locks::file_texts(&plan, &files),
                (true, None) => panic!("{name}: the plan has no lock-safe shape to measure:\n{}", plan.sql()),
                (false, _) => vec![plan.sql()],
            };
            let result = with_writer(&url, &mut from, &files, &writer).await;
            let (only_a, only_b) = difference(&catalog(&mut from).await.unwrap(), &catalog(&mut to).await.unwrap());
            results.push((result, only_a, only_b, files));
            from.close().await;
        }
        let mut problems = Vec::new();
        let [(safe_run, safe_a, safe_b, safe_files), (whole_run, _, _, _)] = <[_; 2]>::try_from(results).ok().unwrap();
        match (&safe_run, &whole_run) {
            (Ok((brief, runs)), Ok((long, _))) => {
                if *runs == 0 {
                    problems.push("the writer never ran".to_string());
                }
                if *brief > BRIEF {
                    problems.push(format!("the writer waited {brief:?} behind the lock-safe files, past {BRIEF:?}"));
                }
                if *long < *brief.max(&Duration::from_millis(10)) * LONGER {
                    problems.push(format!("the writer waited {long:?} behind the plan as one file, against {brief:?} behind the lock-safe files: not {LONGER} times longer, so the case measures no lock (more rows in seed.sql)"));
                }
            }
            (Err(e), _) => problems.push(format!("the lock-safe files failed: {e}")),
            (_, Err(e)) => problems.push(format!("the plan as one file failed: {e}")),
        }
        if !safe_a.is_empty() || !safe_b.is_empty() {
            problems.push(format!("after the lock-safe files the database differs from v2:\n{}", safe_a.iter().map(|l| format!("  - {l}")).chain(safe_b.iter().map(|l| format!("  + {l}"))).collect::<Vec<_>>().join("\n")));
        }
        eprintln!("{name}: lock-safe {:?}, as one file {:?}", safe_run.as_ref().map(|r| r.0), whole_run.as_ref().map(|r| r.0));
        if !problems.is_empty() {
            failures.push(format!("{name}:\n{}\nfiles:\n{}", problems.join("\n"), safe_files.join("-- next file\n")));
        }
        to.close().await;
        for db in [&safe, &whole, &target] {
            admin.exec(&format!("drop database if exists {db} with (force)")).await.unwrap();
        }
    }
    admin.close().await;
    assert!(failures.is_empty(), "{} cases failed:\n\n{}", failures.len(), failures.join("\n\n"));
}
