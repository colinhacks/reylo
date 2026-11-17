//! `reylo::migrate!()`'s deploy and status, run on sqlx from the app, against `reylo migrate`'s own
//! on the same databases: each reads what the other wrote (the history, and the fingerprint of the
//! schema each deploy stores), so a database the app migrated is in step for the command line, and
//! the other way round. Then what the app's runs must do as the command's do (two at once, an edited
//! migration, a draft), and what they leave on the pool's connection. Postgres's and MySQL's run
//! against the servers `REYLO_TEST_PG` and `REYLO_TEST_MYSQL` name, and are skipped where the
//! variable is not set; SQLite's always run.
#![cfg(feature = "migrate")]

use reylo::migrate::{Migrator, State};
use reylo::sqlx;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Postgres,
    Mysql,
    Sqlite,
}

/// A project directory and a database of its own.
struct Project {
    kind: Kind,
    dir: PathBuf,
    url: String,
}

const NAMES: [&str; 3] = ["20260101000000_users.sql", "20260102000000_posts.sql", "20260103000000_seed.sql"];

impl Project {
    async fn new(kind: Kind) -> Option<Project> {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("rmapp_{}_{n}", std::process::id());
        let dir = std::env::temp_dir().join(format!("reylo-app-migrate-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("reylo").join("migrations")).unwrap();
        let url = match kind {
            Kind::Sqlite => format!("sqlite:{}", dir.join("app.db").display()),
            #[cfg(feature = "postgres")]
            Kind::Postgres => {
                let Ok(base) = std::env::var("REYLO_TEST_PG") else {
                    eprintln!("skipped: REYLO_TEST_PG is not set");
                    return None;
                };
                let admin = sqlx::PgPool::connect(&base).await.unwrap();
                // Each alone: Postgres runs the statements of one simple query in one transaction, which create database refuses.
                sqlx::raw_sql(text(&format!("drop database if exists {name} with (force)"))).execute(&admin).await.unwrap();
                sqlx::raw_sql(text(&format!("create database {name}"))).execute(&admin).await.unwrap();
                admin.close().await;
                format!("{}/{name}", base.rsplit_once('/').unwrap().0)
            }
            #[cfg(feature = "mysql")]
            Kind::Mysql => {
                let Ok(base) = std::env::var("REYLO_TEST_MYSQL") else {
                    eprintln!("skipped: REYLO_TEST_MYSQL is not set");
                    return None;
                };
                let base = base.trim_end_matches('/').to_string();
                let admin = sqlx::MySqlPool::connect(&base).await.unwrap();
                sqlx::raw_sql(text(&format!("drop database if exists {name}"))).execute(&admin).await.unwrap();
                sqlx::raw_sql(text(&format!("create database {name}"))).execute(&admin).await.unwrap();
                admin.close().await;
                format!("{base}/{name}")
            }
            #[allow(unreachable_patterns)]
            _ => return None,
        };
        let p = Project { kind, dir, url };
        let (int, words) = match kind {
            Kind::Postgres => ("int", "text"),
            Kind::Mysql => ("int", "varchar(100)"),
            Kind::Sqlite => ("integer", "text"),
        };
        p.write(NAMES[0], &format!("create table users (id {int} primary key, name {words} not null);\n"));
        p.write(NAMES[1], &format!("create table posts (id {int} primary key, user_id {int} not null references users (id));\n"));
        // On MySQL, `1 || 0` is an `or` with the server's own sql_mode, and `10` with sqlx's, which
        // adds PIPES_AS_CONCAT: the app's run must give the row the command line's would.
        let seed = "insert into users (id, name) values (1, 'ada');\ninsert into posts (id, user_id) values (1, 1);\n";
        p.write(NAMES[2], &if kind == Kind::Mysql { format!("{seed}insert into users (id, name) values (2, concat(1 || 0));\n") } else { seed.to_string() });
        p.lock();
        Some(p)
    }

    /// A second database with the same project, its files copied.
    async fn beside(&self) -> Project {
        let other = Project::new(self.kind).await.unwrap();
        for name in NAMES {
            other.write(name, &std::fs::read_to_string(self.dir.join("reylo/migrations").join(name)).unwrap());
        }
        other.lock();
        other
    }

    fn write(&self, name: &str, sql: &str) {
        std::fs::write(self.dir.join("reylo/migrations").join(name), sql).unwrap();
    }

    /// The lock of the migrations as they stand, as `reylo migrate dev` writes it.
    fn lock(&self) {
        let (files, _) = reylo_migrate::commands::read_migrations(&self.dir.join("reylo/migrations")).unwrap();
        let entries = reylo_migrate::lock::chain(files.iter().map(|f| (f.name.as_str(), f.sql.as_str())));
        reylo_migrate::lock::write(&self.dir.join("reylo").join(reylo_migrate::lock::LOCK), &reylo_migrate::lock::Lock { schema: None, entries }).unwrap();
    }

    /// The project's files as `reylo::migrate!()` would compile them in.
    fn migrator(&self) -> Migrator {
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        let mut names: Vec<String> = std::fs::read_dir(self.dir.join("reylo/migrations")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        let files: Vec<(&'static str, &'static str)> = names.into_iter().map(|name| (leak(name.clone()), leak(std::fs::read_to_string(self.dir.join("reylo/migrations").join(&name)).unwrap()))).collect();
        let lock = std::fs::read_to_string(self.dir.join("reylo").join(reylo_migrate::lock::LOCK)).ok().map(leak);
        Migrator::__new("reylo", Box::leak(files.into_boxed_slice()), lock, None)
    }

    fn options(&self) -> reylo_migrate::Options {
        let home = self.dir.join("reylo");
        reylo_migrate::Options {
            url: self.url.clone(),
            shadow_url: None,
            dir: home.join("migrations"),
            schema: home.join("schema.sql"),
            lock: home.join(reylo_migrate::lock::LOCK),
            cache: home.join(".cache"),
            root: self.dir.clone(),
            embedded: None,
        }
    }

    /// `reylo migrate deploy` or `reylo migrate status` from the command line: its exit status and
    /// what it printed.
    async fn cli(&self, deploy: bool) -> (i32, String) {
        let mut lines = Vec::new();
        let mut log = |line: &str| lines.push(line.to_string());
        let code = if deploy { reylo_migrate::deploy(&self.options(), &mut log).await } else { reylo_migrate::status(&self.options(), &mut log).await }.unwrap();
        (code, lines.join("\n"))
    }
}

#[cfg(feature = "sqlx_0_9")]
fn text(sql: &str) -> sqlx::AssertSqlSafe<String> {
    sqlx::AssertSqlSafe(sql.to_owned())
}
#[cfg(not(feature = "sqlx_0_9"))]
fn text(sql: &str) -> &str {
    sql
}

/// What each database's run must do the same way, on a pool of one connection, so that a lock or
/// a setting the run left on it would show.
async fn scenario<DB: reylo::migrate::Database>(p: &Project, pool: impl AsyncFn(&Project) -> sqlx::Pool<DB>) {
    let app = pool(p).await;
    let m = p.migrator();

    let status = m.status(&app).await.unwrap();
    assert!(!status.is_current(), "{status}");
    assert_eq!(status.pending().collect::<Vec<_>>(), NAMES);

    assert_eq!(m.run(&app).await.unwrap(), NAMES);
    assert_eq!(m.run(&app).await.unwrap(), Vec::<String>::new());
    let status = m.status(&app).await.unwrap();
    assert!(status.is_current(), "{status}");
    assert!(status.migrations().iter().all(|s| s.state == State::Applied && s.applied_at.is_some()), "{status:?}");

    // The command line reads the app's history and the fingerprint its deploy stored.
    let (code, out) = p.cli(false).await;
    assert_eq!(code, 0, "reylo migrate status after the app's deploy:\n{out}");
    let (code, out) = p.cli(true).await;
    assert_eq!(code, 0, "reylo migrate deploy after the app's deploy:\n{out}");

    // And the app the command line's.
    let other = p.beside().await;
    let (code, out) = other.cli(true).await;
    assert_eq!(code, 0, "{out}");
    let elsewhere = pool(&other).await;
    let status = m.status(&elsewhere).await.unwrap();
    assert!(status.is_current(), "the app's status after reylo migrate deploy:\n{status}");
    assert_eq!(m.run(&elsewhere).await.unwrap(), Vec::<String>::new());

    // Two instances starting at once run each migration once: the second waits for the first's lock.
    let fresh = p.beside().await;
    let (a, b) = (pool(&fresh).await, pool(&fresh).await);
    // On one task, so a lock waited for by holding the thread would never be released.
    let both = tokio::time::timeout(std::time::Duration::from_secs(120), async { tokio::join!(m.run(&a), m.run(&b)) });
    let (ran_a, ran_b) = both.await.expect("two deploys at once, waiting on each other");
    let mut ran = [ran_a.unwrap(), ran_b.unwrap()].concat();
    ran.sort();
    assert_eq!(ran, NAMES);

    // A migration that fails rolls back alone: those before it stay applied, and the error says so.
    let broken = p.beside().await;
    broken.write("20260104000000_broken.sql", "insert into nowhere (id) values (1);\n");
    broken.lock();
    let failed = broken.migrator().run(&pool(&broken).await).await.unwrap_err();
    assert_eq!(failed.applied(), NAMES, "{failed}");
    assert!(failed.to_string().starts_with("reylo migrate deploy failed") && failed.to_string().contains("nowhere"), "{failed}");

    // With no lock, a migration that sorts before one the database ran is replayed in a shadow
    // database, which the app has none of: refused, for the command line to deploy, even where
    // the build has the command line's code, as this test's does.
    let free = p.beside().await;
    std::fs::remove_file(free.dir.join("reylo").join(reylo_migrate::lock::LOCK)).unwrap();
    let pooled = pool(&free).await;
    assert_eq!(free.migrator().run(&pooled).await.unwrap(), NAMES);
    free.write("20260102120000_late.sql", "create table late (id int primary key);\n");
    let late = free.migrator().run(&pooled).await.unwrap_err();
    assert!(late.problems().iter().any(|problem| problem.code == "late-migration-unchecked"), "{late}");

    // A migration edited since the database ran it, relocked: refused, running nothing.
    p.write(NAMES[0], &std::fs::read_to_string(p.dir.join("reylo/migrations").join(NAMES[0])).unwrap().replace("not null", "not null default 'x'"));
    p.lock();
    let edited = p.migrator();
    let refused = edited.run(&app).await.unwrap_err();
    assert!(refused.problems().iter().any(|problem| problem.code == "applied-migration-edited"), "{refused}");
    let status = edited.status(&app).await.unwrap();
    assert!(!status.is_current());
    assert_eq!(status.migrations()[0].state, State::Edited, "{status}");

    // A migration the lock doesn't list: a draft, which only reylo migrate dev runs.
    p.write(NAMES[0], &std::fs::read_to_string(other.dir.join("reylo/migrations").join(NAMES[0])).unwrap());
    p.lock();
    p.write("20260104000000_draft.sql", "create table drafts (id int primary key);\n");
    let drafted = p.migrator();
    let refused = drafted.run(&app).await.unwrap_err();
    assert!(refused.problems().iter().any(|problem| problem.code == "migration-draft"), "{refused}");
    let status = drafted.status(&app).await.unwrap();
    assert!(!status.is_current());
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres() {
    let Some(p) = Project::new(Kind::Postgres).await else { return };
    let pool = async |p: &Project| sqlx::postgres::PgPoolOptions::new().max_connections(1).connect(&p.url).await.unwrap();
    scenario(&p, pool).await;
    // The deploy's connection is closed with its lock, rather than handed back to the pool.
    let p2 = p.beside().await;
    let app2 = pool(&p2).await;
    p2.migrator().run(&app2).await.unwrap();
    let held: i64 = sqlx::query_scalar(text("select count(*) from pg_locks where locktype = 'advisory' and database = (select oid from pg_database where datname = current_database())")).fetch_one(&app2).await.unwrap();
    assert_eq!(held, 0);
}

#[cfg(feature = "mysql")]
#[tokio::test]
async fn mysql() {
    let Some(p) = Project::new(Kind::Mysql).await else { return };
    let pool = async |p: &Project| sqlx::mysql::MySqlPoolOptions::new().max_connections(1).connect(&p.url).await.unwrap();
    scenario(&p, pool).await;
    let app = pool(&p).await;
    // The seed's `1 || 0` ran with the server's sql_mode, as the command line runs it.
    let server: String = sqlx::query_scalar(text("select @@global.sql_mode")).fetch_one(&app).await.unwrap();
    let name: String = sqlx::query_scalar(text("select name from users where id = 2")).fetch_one(&app).await.unwrap();
    assert_eq!(name, if server.contains("PIPES_AS_CONCAT") { "10" } else { "1" });
    // Status hands its connection back with sqlx's settings as they were.
    let before: String = sqlx::query_scalar(text("select @@session.sql_mode")).fetch_one(&app).await.unwrap();
    assert!(before.contains("PIPES_AS_CONCAT"), "{before}");
    p.migrator().status(&app).await.unwrap();
    let after: String = sqlx::query_scalar(text("select @@session.sql_mode")).fetch_one(&app).await.unwrap();
    assert_eq!(after, before);
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sqlite() {
    let p = Project::new(Kind::Sqlite).await.expect("SQLite needs no server");
    let pool = async |p: &Project| {
        let options = <sqlx::sqlite::SqliteConnectOptions as std::str::FromStr>::from_str(&p.url).unwrap().create_if_missing(true);
        sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
    };
    scenario(&p, pool).await;
    // The deploy's connection goes back to the pool with foreign keys on, as sqlx opened it.
    let p2 = p.beside().await;
    let app = pool(&p2).await;
    p2.migrator().run(&app).await.unwrap();
    let on: i64 = sqlx::query_scalar(text("pragma foreign_keys")).fetch_one(&app).await.unwrap();
    assert_eq!(on, 1);
}

/// The two calls' futures are `Send`, so a run can be spawned onto the runtime or awaited in a
/// handler.
#[allow(dead_code)]
fn send<DB: reylo::migrate::Database>(m: &'static Migrator, pool: &'static sqlx::Pool<DB>) {
    fn spawnable<T: Send + 'static>(_: T) {}
    spawnable(m.run(pool));
    spawnable(m.status(pool));
}
