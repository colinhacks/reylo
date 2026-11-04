//! Scenarios for `reylo migrate`: each builds a project and a fresh database, then does what teams
//! do (apply, edit, drift, rebase, repair, reset, migrations that cannot be atomic, a run cut off
//! halfway) and checks what the command refuses and what the database ends up as. The adversarial
//! ones are chosen to break it: ways a shadow can differ from the target that are not the
//! migrations' doing, objects the comparison does not read, and commands that record what they did
//! not run. Postgres's and MySQL's run against the servers `REYLO_TEST_PG` and `REYLO_TEST_MYSQL`
//! name, and are skipped where the variable is not set; SQLite's always run.

use reylo_migrate::catalog::{catalog, Scope};
use reylo_migrate::db::{with_database, Db, Row, Server};
use reylo_migrate::{Dialect, Options};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The dialect's spelling of the few column types the scenarios use.
struct Types {
    id: &'static str,
    text: &'static str,
    int: &'static str,
}

struct P {
    d: Dialect,
    dir: PathBuf,
    url: String,
    name: String,
    lines: Vec<String>,
    t: Types,
    strict: bool,
}

fn base(d: Dialect) -> Option<String> {
    match d {
        Dialect::Postgres => std::env::var("REYLO_TEST_PG").ok(),
        Dialect::Mysql => std::env::var("REYLO_TEST_MYSQL").ok(),
        Dialect::Sqlite => Some(String::new()),
    }
}

impl P {
    async fn new(d: Dialect) -> Option<P> {
        P::with_create(d, "").await
    }

    /// A project and a fresh database, created with `create` after its name; None where the
    /// dialect's test server is not set.
    async fn with_create(d: Dialect, create: &str) -> Option<P> {
        let Some(base) = base(d) else {
            eprintln!("skipped: no test server for {}", d.name());
            return None;
        };
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("mtest_{}_{n}", std::process::id());
        let dir = std::env::temp_dir().join(format!("reylo-migrate-{}-{name}", d.name()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("migrations")).unwrap();
        let url = match d {
            Dialect::Sqlite => format!("sqlite:{}", dir.join("dev.db").display()),
            _ => {
                let admin_url = if d == Dialect::Postgres { base.clone() } else { with_database(&base, "") };
                let mut admin = Db::connect(&admin_url, &dir).await.unwrap();
                admin.exec(&format!("drop database if exists {name}{}", if d == Dialect::Postgres { " with (force)" } else { "" })).await.unwrap();
                admin.exec(&format!("create database {name} {create}")).await.unwrap();
                admin.close().await;
                with_database(&base, &name)
            }
        };
        let t = match d {
            Dialect::Postgres => Types { id: "bigint generated always as identity primary key", text: "text", int: "bigint" },
            Dialect::Mysql => Types { id: "bigint auto_increment primary key", text: "varchar(200)", int: "bigint" },
            Dialect::Sqlite => Types { id: "integer primary key", text: "text", int: "integer" },
        };
        Some(P { d, dir, url, name, lines: vec![], t, strict: false })
    }

    fn opts(&self) -> Options {
        Options {
            url: self.url.clone(),
            shadow_url: None,
            dir: self.dir.join("migrations"),
            schema: self.dir.join("schema.sql"),
            linear: self.strict,
            comments: true,
            main_branch: None,
            post: None,
            root: self.dir.clone(),
        }
    }

    async fn run<'a, F>(&mut self, f: F) -> i32
    where
        F: AsyncFnOnce(&Options, &mut dyn FnMut(&str)) -> Result<i32, reylo_migrate::Error>,
    {
        let opts = self.opts();
        let mut lines = std::mem::take(&mut self.lines);
        let code = f(&opts, &mut |l: &str| lines.push(l.to_string())).await.unwrap_or_else(|e| panic!("refused to start: {e}"));
        self.lines = lines;
        code
    }

    async fn migrate(&mut self) -> i32 {
        self.run(async |o, log| reylo_migrate::migrate(o, &[], log).await).await
    }

    /// `apply --record <names…>`.
    async fn record(&mut self, names: &[&str]) -> i32 {
        let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        self.run(async |o, log| reylo_migrate::migrate(o, &names, log).await).await
    }

    /// `check`: what apply checks, and the migrations against schema.sql.
    async fn dry_run(&mut self) -> i32 {
        self.run(async |o, log| reylo_migrate::check_command(o, log).await).await
    }

    /// `apply --dry-run` with the replay cache on or off, and what it printed, but for what differs
    /// between any two runs (`same_run`).
    async fn check_with(&mut self, cache: bool) -> (i32, String) {
        reylo_migrate::cache::set_enabled(Some(cache));
        let code = self.dry_run().await;
        reylo_migrate::cache::set_enabled(None);
        (code, same_run(&self.output()))
    }

    /// The check four ways, each of which must print what the first does: with no cache; with the
    /// cache twice, the second answered by what the first kept; and with the cache's lines gone, so
    /// that its databases answer.
    async fn check_every_way(&mut self) -> (i32, String) {
        let off = self.check_with(false).await;
        let cold = self.check_with(true).await;
        let warm = self.check_with(true).await;
        let _ = std::fs::remove_dir_all(self.dir.join(".reylo").join("migrate").join("lines"));
        let copied = self.check_with(true).await;
        assert_eq!(cold, off, "a cold cache changed the check");
        assert_eq!(warm, off, "a warm cache changed the check");
        assert_eq!(copied, off, "the cache's databases changed the check");
        off
    }

    fn file(&self, name: &str, sql: &str) {
        std::fs::write(self.dir.join("migrations").join(name), sql).unwrap();
    }

    fn schema(&self, sql: &str) {
        std::fs::write(self.dir.join("schema.sql"), sql).unwrap();
    }

    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.dir.join("migrations")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    /// What the commands logged since the last call.
    fn output(&mut self) -> String {
        std::mem::take(&mut self.lines).join("\n")
    }

    fn peek(&self) -> String {
        self.lines.join("\n")
    }

    async fn exec(&self, sql: &str) {
        let mut db = Db::connect(&self.url, &self.dir).await.unwrap();
        db.script(sql).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
        db.close().await;
    }

    async fn try_rows(&self, sql: &str) -> Result<Vec<Row>, reylo_migrate::Error> {
        let mut db = Db::connect(&self.url, &self.dir).await.unwrap();
        let rows = db.rows(sql).await;
        db.close().await;
        rows
    }

    async fn rows(&self, sql: &str) -> Vec<Row> {
        self.try_rows(sql).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
    }

    async fn history(&self) -> Vec<String> {
        self.try_rows("select name from __reylo_migrations where applied_at is not null order by applied_at, case when name = 'init.sql' then '' else name end")
            .await
            .unwrap_or_default()
            .iter()
            .map(|r| r.text("name").to_string())
            .collect()
    }
}

/// A run's output without what any two runs differ in: the shadows' names, which MySQL's messages
/// carry, and the shadows a run sweeps, which another killed run left on the test server.
fn same_run(out: &str) -> String {
    let named = regex::Regex::new(r"reylo_shadow_[0-9a-f]+").unwrap().replace_all(out, "reylo_shadow_…").into_owned();
    named.lines().filter(|l| !has(l, r"^dropped \d+ shadow databases? a run left behind$")).collect::<Vec<_>>().join("\n")
}

/// Drops the replay caches only this test's target used, which would otherwise stay on the test
/// server for a week.
async fn forget(url: &str) {
    let _ = reylo_migrate::cache::forget(url).await;
}

fn has(text: &str, pattern: &str) -> bool {
    regex::Regex::new(pattern).unwrap().is_match(text)
}

macro_rules! assert_has {
    ($text:expr, $pattern:expr) => {{
        let text = $text;
        assert!(has(&text, $pattern), "expected /{}/ in:\n{}", $pattern, text);
    }};
}

/// One test per dialect for each scenario, `<scenario>::postgres` and so on.
macro_rules! for_each_dialect {
    ($($scenario:ident),* $(,)?) => {$(
        mod $scenario {
            #[tokio::test(flavor = "current_thread")]
            async fn postgres() {
                if let Some(p) = super::P::new(super::Dialect::Postgres).await { let url = p.url.clone(); super::$scenario(p).await; super::forget(&url).await }
            }
            #[tokio::test(flavor = "current_thread")]
            async fn mysql() {
                if let Some(p) = super::P::new(super::Dialect::Mysql).await { super::$scenario(p).await }
            }
            #[tokio::test(flavor = "current_thread")]
            async fn sqlite() {
                if let Some(p) = super::P::new(super::Dialect::Sqlite).await { super::$scenario(p).await }
            }
        }
    )*};
}

for_each_dialect!(
    applies_pending_migrations_once,
    refuses_migrations_that_do_not_add_up,
    column_order_is_not_compared,
    refuses_drift,
    an_edited_migration_is_refused_and_restored,
    ddl_run_by_hand_then_written_is_recorded,
    a_change_by_hand_with_schema_sql_is_told_to_write_the_migration,
    init_writes_and_records_the_baseline,
    a_late_migration_runs_when_both_orders_agree,
    a_late_migration_whose_order_matters_is_refused,
    a_rebased_migration_drifts_and_a_reset_replays_it,
    a_hotfix_by_hand_is_recorded_once_it_is_a_migration,
    repair_renames_the_history_after_a_rebase,
    a_failed_migration_leaves_nothing,
    files_that_cannot_run_atomically_are_refused,
    a_lone_statement_cut_off_that_landed,
    a_lone_statement_cut_off_that_did_not_land,
    a_shadow_left_by_a_killed_run_is_swept,
    a_rebuilt_table_does_not_cascade,
    one_schema_built_two_ways_has_one_catalog,
    cached_and_uncached_checks_print_the_same_lines,
    a_cached_prefix_replays_only_what_follows_it,
    an_edited_migration_misses_the_cache,
    concurrent_runs_building_one_cache_both_succeed,
    create_drafts_one_migration_cached_or_not,
    a_linear_history_keeps_its_lock,
    two_applies_at_once_apply_each_migration_once,
    create_states_what_it_drops_and_renames,
);

async fn applies_pending_migrations_once(mut p: P) {
    p.schema(&format!("create table users (id {}, email {} not null, name {});", p.t.id, p.t.text, p.t.text));
    p.file("20261027100000_users.sql", &format!("create table users (id {}, email {} not null);", p.t.id, p.t.text));
    p.file("20261027110000_name.sql", &format!("alter table users add column name {};", p.t.text));
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    assert!(p.history().await.is_empty());
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.history().await, ["20261027100000_users.sql", "20261027110000_name.sql"]);
    p.output();
    assert_eq!(p.migrate().await, 0);
    assert_has!(p.output(), "Up to date");
}

async fn refuses_migrations_that_do_not_add_up(mut p: P) {
    p.schema(&format!("create table users (id {}, email {} not null, name {});", p.t.id, p.t.text, p.t.text));
    p.file("20261027100000_users.sql", &format!("create table users (id {}, email {} not null);", p.t.id, p.t.text));
    // check holds the migrations against schema.sql, and runs nothing.
    assert_eq!(p.dry_run().await, 1);
    assert_has!(p.output(), r"schema\.sql has changes not reflected in any migration file\n\n    alter table users add column name [^\n]+;\n\n  Write them as a migration:\n    reylo migrate create <name>");
    assert!(p.try_rows("select 1 from users").await.is_err(), "nothing was applied");
    // apply moves the migrations into the database and never reads schema.sql.
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert!(p.try_rows("select 1 from users").await.is_ok());
    // The other way round: a migration makes what schema.sql doesn't declare.
    p.schema(&format!("create table users (id {}, email {} not null);", p.t.id, p.t.text));
    p.file("20261027110000_tags.sql", &format!("create table tags (id {});", p.t.int));
    p.output();
    assert_eq!(p.dry_run().await, 1);
    assert_has!(p.output(), r"The migrations make changes schema\.sql doesn't declare[\s\S]*tags");
}

async fn column_order_is_not_compared(mut p: P) {
    p.schema(&format!("create table users (id {}, name {}, email {} not null);", p.t.id, p.t.text, p.t.text));
    p.file("20261027100000_users.sql", &format!("create table users (id {}, email {} not null);", p.t.id, p.t.text));
    p.file("20261027110000_name.sql", &format!("alter table users add column name {};", p.t.text));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
}

async fn refuses_drift(mut p: P) {
    p.schema(&format!("create table users (id {}, email {} not null);", p.t.id, p.t.text));
    p.file("20261027100000_users.sql", &format!("create table users (id {}, email {} not null);", p.t.id, p.t.text));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec(&format!("alter table users add column hotfix {}", p.t.int)).await;
    p.schema(&format!("create table users (id {}, email {} not null, nick {});", p.t.id, p.t.text, p.t.text));
    p.file("20261027110000_nick.sql", &format!("alter table users add column nick {};", p.t.text));
    assert_eq!(p.migrate().await, 1);
    let out = p.output();
    assert_has!(&out, r"✗ This database has changes no migration made\n\n    alter table users add column hotfix [^\n]+;\n");
    assert_has!(&out, r"Keep it: add it to schema\.sql, write the migration, then record it without running it:\n    reylo migrate create <name>\n    reylo migrate apply --record <name>");
    assert_has!(&out, r"Nothing was applied\.$");
    assert_eq!(p.history().await, ["20261027100000_users.sql"]);
}

async fn an_edited_migration_is_refused_and_restored(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    let original = format!("create table users (id {int});");
    p.schema(&original);
    p.file("20261027100000_users.sql", &original);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // An edit to comments alone changes nothing that ran.
    p.file("20261027100000_users.sql", &format!("-- the users\n{original}"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    // A real edit is refused, both texts shown, whatever the database has.
    p.file("20261027100000_users.sql", &format!("create table users (id {int}, email {text});"));
    p.schema(&format!("create table users (id {int}, email {text});"));
    assert_eq!(p.migrate().await, 1);
    let out = p.output();
    assert_has!(&out, r"✗ 20261027100000_users\.sql changed after this database applied it\n\n    --- as applied\n    \+\+\+ on disk\n    - create table users \(id [^\n]+\);\n    \+ create table users \(id [^\n]+, email [^\n]+\);\n\n  Restore the text this database ran:\n    reylo migrate restore 20261027100000_users\.sql");
    p.exec(&format!("alter table users add column email {text}")).await;
    assert_eq!(p.migrate().await, 1, "a database that matches the edit records nothing without being told: {}", p.peek());
    assert_has!(p.output(), "changed after this database applied it");
    p.exec("alter table users drop column email").await;
    // restore writes back the text the database ran.
    assert_eq!(p.run(async |o, log| reylo_migrate::restore(o, "20261027100000_users.sql", log).await).await, 0, "{}", p.peek());
    assert_eq!(std::fs::read_to_string(p.dir.join("migrations").join("20261027100000_users.sql")).unwrap(), original);
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), "Up to date");
    // A deleted migration comes back the same way.
    std::fs::remove_file(p.dir.join("migrations").join("20261027100000_users.sql")).unwrap();
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"This database applied 20261027100000_users\.sql, which isn't in [^\n]*migrations\n\n  Restore it from this database's history:\n    reylo migrate restore 20261027100000_users\.sql");
    assert_eq!(p.run(async |o, log| reylo_migrate::restore(o, "20261027100000_users.sql", log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
}

async fn ddl_run_by_hand_then_written_is_recorded(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.schema(&format!("create table users (id {int});"));
    p.file("20261027100000_users.sql", &format!("create table users (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec(&format!("alter table users add column nick {text}")).await;
    p.file("20261027110000_nick.sql", &format!("alter table users add column nick {text};"));
    p.schema(&format!("create table users (id {int}, nick {text});"));
    // Never recorded without being told: the refusal names the migration that makes the change.
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"20261027110000_nick\.sql makes exactly that change\. Record it without running it:\n    reylo migrate apply --record 20261027110000_nick\.sql");
    assert_eq!(p.record(&["20261027110000_nick.sql"]).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Recorded 20261027110000_nick\.sql without running it");
    assert_eq!(p.history().await, ["20261027100000_users.sql", "20261027110000_nick.sql"]);
    // A migration that also changes data is never recorded: its data statements would not run.
    p.exec(&format!("alter table users add column score {int}")).await;
    p.file("20261027120000_score.sql", &format!("alter table users add column score {int}; update users set score = 0;"));
    assert_eq!(p.record(&["20261027120000_score.sql"]).await, 1);
    assert_has!(p.output(), r"20261027120000_score\.sql changes data, so it can't be recorded without running\n  Its data statements \(update users set…\) would never run here");
    // Nor named in a drift's refusal.
    assert_eq!(p.migrate().await, 1);
    let out = p.output();
    assert!(!out.contains("--record 20261027120000_score.sql"), "{out}");
    // A migration the database does not have can't be recorded.
    p.exec("alter table users drop column score").await;
    p.file("20261027120000_score.sql", &format!("alter table users add column score {int};"));
    assert_eq!(p.record(&["20261027120000_score.sql"]).await, 1);
    assert_has!(p.output(), "This database doesn't have what 20261027120000_score.sql make");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
}

async fn a_change_by_hand_with_schema_sql_is_told_to_write_the_migration(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.schema(&format!("create table users (id {int});"));
    p.file("20261027100000_users.sql", &format!("create table users (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec(&format!("alter table users add column nick {text}")).await;
    p.schema(&format!("create table users (id {int}, nick {text});"));
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"This database has changes no migration made[\s\S]*reylo migrate create <name>");
}

async fn init_writes_and_records_the_baseline(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.exec(&format!("create table users (id {id}, email {text} not null); create table posts (id {int}, user_id {int} references users (id)); create view emails as select email from users")).await;
    assert_eq!(p.run(async |o, log| reylo_migrate::init(o, &[], log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.files(), ["init.sql"]);
    assert_eq!(p.history().await, ["init.sql"]);
    // A fresh database runs init.sql; the next migration applies on both.
    p.file("20261028100000_nick.sql", &format!("alter table users add column nick {text};"));
    let schema = std::fs::read_to_string(p.dir.join("schema.sql")).unwrap();
    p.schema(&format!("{schema}\nalter table users add column nick {text};\n"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    let q = P::new(p.d).await.unwrap();
    p.url = q.url.clone();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(q.history().await, ["init.sql", "20261028100000_nick.sql"]);
}

async fn a_late_migration_runs_when_both_orders_agree(mut p: P) {
    let int = p.t.int;
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027120000_c.sql", &format!("create table c (id {int});"));
    p.schema(&format!("create table a (id {int}); create table c (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.schema(&format!("create table a (id {int}); create table b (id {int}); create table c (id {int});"));
    p.file("20261027110000_b.sql", &format!("create table b (id {int});"));
    p.strict = true;
    reylo_migrate::lock::write(&p.dir.join("migrations"), &p.files()).unwrap();
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"older than 20261027120000_c\.sql[\s\S]*history is linear");
    p.strict = false;
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.history().await, ["20261027100000_a.sql", "20261027120000_c.sql", "20261027110000_b.sql"]);
}

async fn a_late_migration_whose_order_matters_is_refused(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    // By time: rename, then an index on the new name. This database ran the index first, on the old name.
    p.file("20261027100000_users.sql", &format!("create table users (id {int}, name {text});"));
    p.file("20261027120000_index.sql", "create index users_name on users (name);");
    p.schema(&format!("create table users (id {int}, name {text}); create index users_name on users (name);"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.file("20261027110000_rename.sql", "alter table users rename column name to full_name;");
    p.schema(&format!("create table users (id {int}, full_name {text}); create index users_name on users (full_name);"));
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), "fails on this database's history|fails when every migration runs in name order|end differently");
}

async fn a_rebased_migration_drifts_and_a_reset_replays_it(mut p: P) {
    let int = p.t.int;
    p.schema(&format!("create table a (id {int}); create table mine (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027110000_mine.sql", &format!("create table mine (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // Main merged a migration from another branch, newer than mine.
    p.file("20261027120000_theirs.sql", &format!("create table theirs (id {int});"));
    p.schema(&format!("create table a (id {int}); create table mine (id {int}); create table theirs (id {int});"));
    assert_eq!(p.run(async |o, log| reylo_migrate::rebase(o, &["20261027110000_mine.sql".to_string()], log).await).await, 0);
    let files = p.files();
    assert_eq!(files[1], "20261027120000_theirs.sql");
    assert!(has(&files[2], r"^\d{14}_mine\.sql$") && files[2].as_str() > "20261027120000", "{files:?}");
    p.output();
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), "which isn't in");
    p.exec("insert into a values (1)").await;
    assert_eq!(p.run(async |o, log| reylo_migrate::reset(o, log).await).await, 0, "{}", p.peek());
    assert_eq!(p.history().await, ["20261027100000_a.sql".to_string(), "20261027120000_theirs.sql".to_string(), files[2].clone()]);
    assert!(p.rows("select * from a").await.is_empty(), "a reset loses data");
}

async fn a_hotfix_by_hand_is_recorded_once_it_is_a_migration(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.schema(&format!("create table users (id {int}, email {text});"));
    p.file("20261027100000_users.sql", &format!("create table users (id {int}, email {text});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into users values (1, 'a@b.co')").await;
    p.exec("create index users_email on users (email)").await;
    p.output();
    assert_eq!(p.migrate().await, 1, "drift until the hotfix is a migration");
    assert_has!(p.output(), "This database has changes no migration made");
    p.file("20261027110000_hotfix.sql", "create index users_email on users (email);");
    p.schema(&format!("create table users (id {int}, email {text}); create index users_email on users (email);"));
    assert_eq!(p.migrate().await, 1, "recorded only when told");
    assert_has!(p.output(), r"reylo migrate apply --record 20261027110000_hotfix\.sql");
    assert_eq!(p.record(&["20261027110000_hotfix.sql"]).await, 0, "{}", p.peek());
    assert_has!(p.output(), "Recorded 20261027110000_hotfix.sql without running it");
    assert_eq!(p.history().await, ["20261027100000_users.sql", "20261027110000_hotfix.sql"]);
    assert_eq!(p.rows("select * from users").await.len(), 1);
}

async fn repair_renames_the_history_after_a_rebase(mut p: P) {
    let int = p.t.int;
    p.schema(&format!("create table a (id {int}); create table mine (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027110000_mine.sql", &format!("create table mine (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into a values (1)").await;
    assert_eq!(p.run(async |o, log| reylo_migrate::rebase(o, &["20261027110000_mine.sql".to_string()], log).await).await, 0);
    p.output();
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), "which isn't in");
    assert_eq!(p.run(async |o, log| reylo_migrate::repair(o, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"removed from the history[\s\S]*|recorded as applied");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select * from a").await.len(), 1);
}

async fn a_failed_migration_leaves_nothing(mut p: P) {
    let int = p.t.int;
    p.schema(&format!("create table a (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    if p.d == Dialect::Mysql {
        return;
    }
    // Fine in the shadows, where `a` is empty; fails on the live database, which has a row.
    p.exec("insert into a values (null)").await;
    let (change, column) = if p.d == Dialect::Postgres { ("alter column id set not null", " not null") } else { ("add column x integer not null", ", x integer not null") };
    p.file("20261027110000_b.sql", &format!("create table b (id {int}); alter table a {change};"));
    p.schema(&format!("create table a (id {int}{column}); create table b (id {int});"));
    assert_eq!(p.migrate().await, 1, "{}", p.peek());
    assert!(p.try_rows("select 1 from b").await.is_err(), "b rolled back with the file");
    assert_eq!(p.history().await, ["20261027100000_a.sql"]);
}

async fn files_that_cannot_run_atomically_are_refused(mut p: P) {
    let int = p.t.int;
    p.schema(&format!("create table a (id {int}); create table b (id {int});"));
    p.file("20261027100000_ab.sql", &format!("begin; create table a (id {int}); create table b (id {int}); commit;"));
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), "own transaction statement");
    p.file("20261027100000_ab.sql", &format!("create table a (id {int}); create table b (id {int});"));
    let code = p.migrate().await;
    if p.d == Dialect::Mysql {
        assert_eq!(code, 1);
        assert_has!(p.output(), "MySQL commits each one on its own");
    } else {
        assert_eq!(code, 0, "{}", p.peek());
    }
}

async fn a_lone_statement_cut_off(mut p: P, landed: bool) {
    if p.d == Dialect::Sqlite {
        return;
    }
    let int = p.t.int;
    let lone = if p.d == Dialect::Postgres { "create index concurrently a_id on a (id)" } else { "create index a_id on a (id)" };
    p.schema(&format!("create table a (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.file("20261027110000_index.sql", &format!("{lone};"));
    p.schema(&format!("create table a (id {int}); create index a_id on a (id);"));
    // The crash: the row marked started, and the statement run or not.
    let mut db = Db::connect(&p.url, &p.dir).await.unwrap();
    let column = if p.d == Dialect::Mysql { "`sql`" } else { "sql" };
    let insert = format!("insert into __reylo_migrations (name, {column}, applied_at) values ({}, {}, null)", db.param(1), db.param(2));
    db.exec_with(&insert, &["20261027110000_index.sql", &format!("{lone};")]).await.unwrap();
    if landed {
        db.exec(lone).await.unwrap();
    }
    db.close().await;
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), if landed { r"20261027110000_index\.sql was cut off before it was recorded, and its change is in place: recorded" } else { r"✓ Applied 20261027110000_index\.sql" });
    assert_eq!(p.history().await, ["20261027100000_a.sql", "20261027110000_index.sql"]);
    assert_eq!(p.migrate().await, 0);
    assert_has!(p.output(), "Up to date");
}

async fn a_lone_statement_cut_off_that_landed(p: P) {
    a_lone_statement_cut_off(p, true).await
}

async fn a_lone_statement_cut_off_that_did_not_land(p: P) {
    a_lone_statement_cut_off(p, false).await
}

async fn a_shadow_left_by_a_killed_run_is_swept(mut p: P) {
    if p.d == Dialect::Sqlite {
        return;
    }
    let int = p.t.int;
    p.schema(&format!("create table a (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    let mut target = Db::connect(&p.url, &p.dir).await.unwrap();
    let mut server = Server::open(&mut target, &p.url, None).await.unwrap();
    let held = server.shadow().await.unwrap();
    let dead = format!("reylo_shadow_dead{}", COUNTER.fetch_add(1, Ordering::Relaxed));
    target.exec(&format!("create database {dead}")).await.unwrap();
    target.close().await;
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    let list = if p.d == Dialect::Postgres { "select datname as n from pg_database where datname like 'reylo_shadow_%'" } else { r"select schema_name as n from information_schema.schemata where schema_name like 'reylo\_shadow\_%'" };
    // Another test's run may be sweeping it at the same moment, which this run's sweep leaves to
    // it: it is gone once that drop ends.
    let mut names: Vec<String> = Vec::new();
    for _ in 0..100 {
        names = p.rows(list).await.iter().map(|r| r.text("n").to_string()).collect();
        if !names.contains(&dead) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(names.contains(&held.name), "the held shadow stays: {names:?}");
    assert!(!names.contains(&dead), "the dead one is dropped: {names:?}");
    server.drop_shadow(held).await;
    server.close().await;
}

async fn a_rebuilt_table_does_not_cascade(mut p: P) {
    if p.d != Dialect::Sqlite {
        return;
    }
    p.file("20261027100000_tables.sql", "create table parent (id integer primary key, name text); create table child (id integer primary key, parent_id integer references parent (id) on delete cascade);");
    p.schema("create table parent (id integer primary key, name text); create table child (id integer primary key, parent_id integer references parent (id) on delete cascade);");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("pragma foreign_keys = on; insert into parent values (1, 'p'); insert into child values (1, 1);").await;
    p.file("20261027110000_not_null.sql", "create table parent_new (id integer primary key, name text not null); insert into parent_new select * from parent; drop table parent; alter table parent_new rename to parent;");
    p.schema("create table parent (id integer primary key, name text not null); create table child (id integer primary key, parent_id integer references parent (id) on delete cascade);");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select * from child").await.len(), 1, "the child row survives the rebuild");
}

async fn one_schema_built_two_ways_has_one_catalog(p: P) {
    let (int, text) = (p.t.int, p.t.text);
    let ways = [format!("create table users (id {int}, email {text});"), format!("create table users (id {int}); alter table users add column email {text};")];
    let mut target = Db::connect(&p.url, &p.dir).await.unwrap();
    let mut server = Server::open(&mut target, &p.url, None).await.unwrap();
    let mut out = Vec::new();
    for sql in &ways {
        let mut s = server.shadow().await.unwrap();
        s.db.script(sql).await.unwrap();
        out.push(catalog(&mut s.db, Scope::default()).await.unwrap());
        server.drop_shadow(s).await;
    }
    server.close().await;
    target.close().await;
    assert_eq!(out[0], out[1]);
}

// ---------------------------------------------------------------------------------------------
// The replay cache: a check answered from it prints what one replayed in full prints.

async fn cached_and_uncached_checks_print_the_same_lines(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    let users = format!("create table users (id {id}, email {text} not null, name {text});");
    p.schema(&users);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, email {text} not null);"));
    p.file("20261027110000_name.sql", &format!("alter table users add column name {text};"));
    assert_eq!(p.check_every_way().await.0, 0);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    let (code, out) = p.check_every_way().await;
    assert_eq!(code, 0);
    assert_has!(out, "Nothing is pending");
    // One more migration, pending on a history the cache holds.
    p.file("20261027120000_index.sql", "create index users_email on users (email);");
    p.schema(&format!("{users} create index users_email on users (email);"));
    assert_eq!(p.check_every_way().await.0, 0);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    // Drift, then undone.
    p.exec(&format!("alter table users add column hotfix {int}")).await;
    let (code, out) = p.check_every_way().await;
    assert_eq!(code, 1);
    assert_has!(out, "This database has changes no migration made");
    p.exec("alter table users drop column hotfix").await;
    // A migration that does not add up, then one that fails.
    p.file("20261027130000_nick.sql", &format!("alter table users add column nick {text};"));
    let (code, out) = p.check_every_way().await;
    assert_eq!(code, 1);
    assert_has!(out, r"The migrations make changes schema\.sql doesn't declare[\s\S]*nick");
    p.file("20261027130000_nick.sql", &format!("alter table nobody add column nick {text};"));
    let (code, out) = p.check_every_way().await;
    assert_eq!(code, 1);
    assert_has!(out, "20261027130000_nick.sql fails");
    std::fs::remove_file(p.dir.join("migrations").join("20261027130000_nick.sql")).unwrap();
    // A late migration, which the replay by time runs in its place.
    p.file("20261027115000_posts.sql", &format!("create table posts (id {int});"));
    p.schema(&format!("{users} create index users_email on users (email); create table posts (id {int});"));
    assert_eq!(p.check_every_way().await.0, 0);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    // An applied migration edited: in its comments, then in what it does.
    p.file("20261027110000_name.sql", &format!("-- the name\nalter table users add column name {text};"));
    let (code, out) = p.check_every_way().await;
    assert_eq!(code, 0, "an edit to comments alone changes nothing that ran: {out}");
    p.file("20261027110000_name.sql", &format!("alter table users add column name {text} not null default 'x';"));
    let (code, out) = p.check_every_way().await;
    assert_eq!(code, 1);
    assert_has!(out, "changed after this database applied it");
}

async fn a_cached_prefix_replays_only_what_follows_it(mut p: P) {
    let int = p.t.int;
    let tables = |n: usize| (1..=n).map(|k| format!("create table t{k} (id {int});")).collect::<Vec<_>>().join(" ");
    for k in 1..=3 {
        p.file(&format!("{}_t{k}.sql", 20261027100000u64 + k as u64), &format!("create table t{k} (id {int});"));
    }
    p.schema(&tables(3));
    reylo_migrate::cache::set_enabled(Some(true));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    // With its lines gone, the check starts each shadow from the database the last run kept.
    let _ = std::fs::remove_dir_all(p.dir.join(".reylo").join("migrate").join("lines"));
    reylo_migrate::cache::take_stats();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    let stats = reylo_migrate::cache::take_stats();
    let keeps_databases = p.d != Dialect::Mysql;
    if keeps_databases {
        // apply never reads schema.sql, so only the history's database is kept for it.
        assert_eq!((stats.replayed, stats.copies), (1, 1), "the history's database answers, and schema.sql replays: {stats:?}");
    } else {
        assert_eq!(stats.replayed, 4, "MySQL replays it all: {stats:?}");
    }
    // Then their lines do, with no shadow at all.
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    let stats = reylo_migrate::cache::take_stats();
    assert_eq!((stats.replayed, stats.copies, stats.lines), (0, 0, 2), "{stats:?}");
    // A new migration: schema.sql is built anew, and the history replays only it.
    p.file("20261027100004_t4.sql", &format!("create table t4 (id {int});"));
    p.schema(&tables(4));
    p.output();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    let stats = reylo_migrate::cache::take_stats();
    if keeps_databases {
        assert_eq!((stats.replayed, stats.copies), (2, 1), "{stats:?}");
    } else {
        assert_eq!(stats.replayed, 5, "{stats:?}");
    }
    reylo_migrate::cache::set_enabled(None);
}

async fn an_edited_migration_misses_the_cache(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.schema(&format!("create table a (id {int}); create table b (id {int}, note {text});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027110000_b.sql", &format!("create table b (id {int}, note {text});"));
    reylo_migrate::cache::set_enabled(Some(true));
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    reylo_migrate::cache::take_stats();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    assert_eq!(reylo_migrate::cache::take_stats().replayed, 0, "a warm check replays nothing");
    p.output();
    // The pending migration's text changes: what it makes no longer adds up, and the check says so,
    // where the lines kept for the text it had would answer that it does. (A database another run
    // kept for the edited history may answer it, which is the same.)
    p.file("20261027110000_b.sql", &format!("create table b (id {int});"));
    assert_eq!(p.dry_run().await, 1, "{}", p.peek());
    assert_has!(p.output(), r"schema\.sql has changes not reflected in any migration file[\s\S]*note");
    // Back as it was, and the kept lines answer again.
    p.file("20261027110000_b.sql", &format!("create table b (id {int}, note {text});"));
    reylo_migrate::cache::take_stats();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    assert_eq!(reylo_migrate::cache::take_stats().replayed, 0);
    // Renamed, the same text is another history.
    std::fs::rename(p.dir.join("migrations/20261027110000_b.sql"), p.dir.join("migrations/20261027110000_b2.sql")).unwrap();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    let stats = reylo_migrate::cache::take_stats();
    assert!(stats.replayed + stats.copies >= 1, "the renamed history is not answered by the old one's lines: {stats:?}");
    reylo_migrate::cache::set_enabled(None);
    p.output();
    let (code, _) = p.check_every_way().await;
    assert_eq!(code, 0);
}

async fn create_states_what_it_drops_and_renames(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, middle_name {text}, legacy {text});"));
    p.schema(&format!("create table users (id {id}, middle_name {text}, legacy {text});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into users (middle_name, legacy) values ('m', 'l')").await;
    p.schema(&format!("create table users (id {id}, nickname {text});"));
    let create = |drop: &[&str], rename: &[(&str, &str)]| reylo_migrate::Choices { drop: drop.iter().map(|d| d.to_string()).collect(), rename: rename.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() };
    let made = |p: &P| p.files().into_iter().filter(|f| f.contains("_tidy")).collect::<Vec<_>>();
    let take_back = |p: &P| {
        for f in made(p) {
            std::fs::remove_file(p.dir.join("migrations").join(f)).unwrap();
        }
    };
    // Two text columns go and one comes: no rename is guessed, and nothing is dropped unstated.
    let choices = create(&[], &[]);
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 1);
    let out = p.output();
    assert_has!(&out, r"This migration would drop users\.legacy, users\.middle_name, and their data\n\n  To drop them:\n    reylo migrate create tidy --drop users\.legacy,users\.middle_name\n  If a column was renamed instead:\n    reylo migrate create tidy --rename users\.legacy=<new name>");
    assert!(made(&p).is_empty(), "nothing is written");
    // Dropping one leaves the other alone with its definition: renamed, keeping its rows.
    let choices = create(&["users.legacy"], &[]);
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Renames users\.middle_name to nickname, keeping its rows");
    let sql = made(&p).iter().map(|f| std::fs::read_to_string(p.dir.join("migrations").join(f)).unwrap()).collect::<String>();
    assert!(sql.contains("rename column middle_name to nickname") && sql.contains("drop column legacy"), "{sql}");
    take_back(&p);
    // Both named: both dropped, nickname added.
    let choices = create(&["users.legacy", "users.middle_name"], &[]);
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 0, "{}", p.peek());
    let sql = made(&p).iter().map(|f| std::fs::read_to_string(p.dir.join("migrations").join(f)).unwrap()).collect::<String>();
    assert!(!sql.contains("rename") && sql.contains("drop column middle_name"), "{sql}");
    take_back(&p);
    p.output();
    // A name --drop gives that the change does not drop is refused, as a typo would be.
    let choices = create(&["users.legacy", "users.middle_nam"], &[]);
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 1);
    assert_has!(p.output(), r"--drop names users\.middle_nam, which this migration doesn't drop");
    // A stated rename, with a type change, keeps the rows; the data moves by assignment.
    let narrow = if p.d == Dialect::Sqlite { "text" } else { "varchar(40)" };
    p.schema(&format!("create table users (id {id}, nickname {narrow});"));
    let choices = create(&["users.legacy"], &[("users.middle_name", "nickname")]);
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select nickname from users").await[0].text("nickname"), "m");
}

async fn two_applies_at_once_apply_each_migration_once(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    for k in 0..5u64 {
        p.file(&format!("{}_t{k}.sql", 20261027100000u64 + k), &format!("create table t{k} (id {int}, note {text});"));
    }
    let opts = p.opts();
    // Two instances of an app, each migrating as it starts, on one database.
    let runs: Vec<(i32, String)> = std::thread::scope(|s| {
        let started: Vec<_> = (0..2)
            .map(|_| {
                let opts = &opts;
                s.spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                    let mut lines = Vec::new();
                    let code = rt.block_on(reylo_migrate::migrate(opts, &[], &mut |l: &str| lines.push(l.to_string()))).unwrap();
                    (code, lines.join("\n"))
                })
            })
            .collect();
        started.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (code, out) in &runs {
        assert_eq!(*code, 0, "{out}");
    }
    let applied: usize = runs.iter().map(|(_, out)| out.matches("✓ Applied").count()).sum();
    assert_eq!(applied, 5, "each migration ran once: {runs:?}");
    assert!(runs.iter().any(|(_, out)| out.contains("Up to date")), "{runs:?}");
    assert_eq!(p.history().await.len(), 5);
    p.output();
}

async fn concurrent_runs_building_one_cache_both_succeed(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    for k in 0..20u64 {
        p.file(&format!("{}_t{k}.sql", 20261027100000u64 + k), &format!("create table t{k} (id {int}, note {text});"));
    }
    p.schema(&(0..20).map(|k| format!("create table t{k} (id {int}, note {text});")).collect::<Vec<_>>().join("\n"));
    // A second target of the same project, whose shadows share the first's keys.
    let q = P::new(p.d).await.unwrap();
    let both = [p.opts(), Options { url: q.url.clone(), ..p.opts() }];
    for round in 0..3 {
        let _ = std::fs::remove_dir_all(p.dir.join(".reylo"));
        forget(&p.url).await;
        forget(&q.url).await;
        let runs: Vec<(i32, Vec<String>)> = std::thread::scope(|s| {
            let started: Vec<_> = both
                .iter()
                .map(|o| {
                    s.spawn(move || {
                        reylo_migrate::cache::set_enabled(Some(true));
                        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                        let mut lines = Vec::new();
                        let code = rt.block_on(reylo_migrate::check_command(o, &mut |l: &str| lines.push(l.to_string()))).unwrap();
                        (code, lines)
                    })
                })
                .collect();
            started.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for (code, lines) in &runs {
            assert_eq!(*code, 0, "round {round}:\n{}", lines.join("\n"));
        }
        assert_eq!(same_run(&runs[0].1.join("\n")), same_run(&runs[1].1.join("\n")), "round {round}");
    }
    // What they kept answers the next check.
    reylo_migrate::cache::set_enabled(Some(true));
    reylo_migrate::cache::take_stats();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    assert_eq!(reylo_migrate::cache::take_stats().replayed, 0);
    reylo_migrate::cache::set_enabled(None);
    forget(&q.url).await;
}

async fn create_drafts_one_migration_cached_or_not(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, email {text});"));
    p.file("20261027110000_posts.sql", &format!("create table posts (id {int});"));
    p.schema(&format!("create table users (id {id}, email {text}); create table posts (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    p.schema(&format!("create table users (id {id}, email {text}, nick {text}); create table posts (id {int}, title {text});"));
    // Each way, `create` writes the same files (on MySQL one per statement) and prints the same;
    // they are taken back each time.
    let mut drafts = Vec::new();
    for cache in [false, true, true] {
        reylo_migrate::cache::set_enabled(Some(cache));
        let code = p.run(async |o, log| reylo_migrate::create(o, "more", &Default::default(), log).await).await;
        reylo_migrate::cache::set_enabled(None);
        let made: Vec<String> = p.files().into_iter().filter(|f| has(f, r"_more(_\d+)?\.sql$")).collect();
        let texts: Vec<String> = made.iter().map(|f| std::fs::read_to_string(p.dir.join("migrations").join(f)).unwrap()).collect();
        for f in &made {
            std::fs::remove_file(p.dir.join("migrations").join(f)).unwrap();
        }
        let out = regex::Regex::new(r"\d{14}_more").unwrap().replace_all(&same_run(&p.output()), "<time>_more").into_owned();
        drafts.push((code, out, texts));
    }
    assert_eq!(drafts[0].0, 0, "{:?}", drafts[0]);
    assert_eq!(drafts[1], drafts[0], "a cold cache changed the draft");
    assert_eq!(drafts[2], drafts[0], "a warm cache changed the draft");
    // The draft, written, checks out, from what `create` kept.
    reylo_migrate::cache::set_enabled(Some(true));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "more", &Default::default(), log).await).await, 0, "{}", p.peek());
    p.output();
    reylo_migrate::cache::set_enabled(None);
    let (code, out) = p.check_every_way().await;
    assert_eq!(code, 0, "{out}");
}

async fn a_linear_history_keeps_its_lock(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.strict = true;
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, email {text});"));
    p.schema(&format!("create table users (id {id}, email {text});"));
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"history is linear, and \S*migrations/lock\.yaml is missing");
    assert_eq!(p.run(async |o, log| reylo_migrate::rebase(o, &[], log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // `create` appends its migration to the lock.
    p.schema(&format!("create table users (id {id}, email {text}, nick {text});"));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "nick", &Default::default(), log).await).await, 0, "{}", p.peek());
    let lock = std::fs::read_to_string(p.dir.join("migrations").join("lock.yaml")).unwrap();
    let created = p.files().into_iter().filter(|f| f.ends_with(".sql")).collect::<Vec<_>>();
    assert_eq!(lock, reylo_migrate::lock::render(&created));
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // A migration written by hand, older than what the database applied: not in the lock, and refused.
    p.file("20261027090000_bio.sql", &format!("alter table users add column bio {text};"));
    p.schema(&format!("create table users (id {id}, email {text}, nick {text}, bio {text});"));
    assert_eq!(p.migrate().await, 1);
    let out = p.output();
    assert_has!(&out, r"20261027090000_bio\.sql isn't in it");
    assert_has!(&out, r"20261027090000_bio\.sql is older than \d{14}_\w+\.sql, which this database already applied");
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "more", &Default::default(), log).await).await, 1, "create refuses a lock out of order");
    assert_has!(p.output(), "isn't in it");
    // A bare rebase moves it after the newest and lists it last.
    assert_eq!(p.run(async |o, log| reylo_migrate::rebase(o, &[], log).await).await, 0, "{}", p.peek());
    let files = p.files().into_iter().filter(|f| f.ends_with(".sql")).collect::<Vec<_>>();
    assert!(files.last().unwrap().ends_with("_bio.sql"), "{files:?}");
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select bio from users").await.len(), 0);
    // Turned off, the history is loose again, and the lock is only reported.
    p.strict = false;
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"lock\.yaml is read only when the history is linear");
}

// ---------------------------------------------------------------------------------------------
// Adversarial cases, on the server databases they concern.

macro_rules! on {
    ($dialect:ident, $($scenario:ident),* $(,)?) => {$(
        #[tokio::test(flavor = "current_thread")]
        async fn $scenario() {
            if let Some(p) = P::new(Dialect::$dialect).await { let url = p.url.clone(); adversarial::$scenario(p).await; forget(&url).await }
        }
    )*};
}

on!(
    Postgres,
    a_database_search_path_resolves_names_as_the_target_does,
    a_database_time_zone_does_not_change_defaults,
    grants_compare_without_their_grantor,
    a_migration_that_creates_a_role_is_refused,
    an_object_the_comparison_does_not_read_is_not_a_match,
    a_failed_concurrent_index_leaves_nothing,
    a_platforms_objects_are_left_out,
    reylo_run_runs_inline_sql_and_files,
    repair_records_a_data_migration_as_run,
    a_platforms_default_privileges_are_not_drift,
    function_privileges_and_index_comments_are_compared,
    a_server_setting_change_misses_the_cache,
    a_migrations_session_setting_is_never_carried_by_the_cache,
);
on!(Mysql, a_migration_that_names_a_database_is_refused);
on!(Postgres, a_narrowing_type_change_fails_rather_than_cuts);

#[tokio::test(flavor = "current_thread")]
async fn a_database_whose_default_collation_differs() {
    if let Some(mut p) = P::with_create(Dialect::Mysql, "character set utf8mb4 collate utf8mb4_unicode_ci").await {
        p.schema("create table t (name varchar(10));");
        p.file("20261027100000_t.sql", "create table t (name varchar(10));");
        assert_eq!(p.migrate().await, 0, "the shadows must take the target's default collation:\n{}", p.peek());
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
    }
}

mod adversarial {
    use super::*;

    pub async fn a_database_search_path_resolves_names_as_the_target_does(mut p: P) {
        p.exec(&format!("alter database {} set search_path = app, public", p.name)).await;
        p.schema("create schema app; create table app.users (id int);");
        p.file("20261027100000_app.sql", "create schema app;");
        p.file("20261027110000_users.sql", "create table users (id int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.migrate().await, 0, "a second run must not report drift:\n{}", p.peek());
    }

    pub async fn a_database_time_zone_does_not_change_defaults(mut p: P) {
        p.exec(&format!("alter database {} set timezone = 'America/Los_Angeles'", p.name)).await;
        p.schema("create table e (at timestamptz default '2026-01-01 00:00:00+00');");
        p.file("20261027100000_e.sql", "create table e (at timestamptz default '2026-01-01 00:00:00+00');");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.migrate().await, 0, "a second run must not report drift:\n{}", p.peek());
    }

    pub async fn grants_compare_without_their_grantor(mut p: P) {
        p.exec("do $$ begin if not exists (select 1 from pg_roles where rolname = 'madv_reader') then create role madv_reader; end if; if not exists (select 1 from pg_roles where rolname = 'madv_deployer') then create role madv_deployer superuser; end if; end $$").await;
        p.schema("create table t (id int); grant select on t to madv_reader;");
        p.file("20261027100000_t.sql", "create table t (id int); grant select on t to madv_reader;");
        // Applied by another role, as a deploy role would: the grantor differs from the shadows'.
        p.exec("set role madv_deployer; create table t (id int); grant select on t to madv_reader; reset role; create table __reylo_migrations (name text primary key, sql text not null, applied_at timestamptz); insert into __reylo_migrations values ('20261027100000_t.sql', 'create table t (id int); grant select on t to madv_reader;', now())").await;
        assert_eq!(p.migrate().await, 0, "expected no drift from the grantor alone:\n{}", p.peek());
    }

    pub async fn a_migration_that_creates_a_role_is_refused(mut p: P) {
        let role = format!("madv_role_{}", std::process::id());
        p.exec(&format!("drop role if exists {role}")).await;
        p.schema("create table t (id int);");
        p.file("20261027100000_t.sql", "create table t (id int);");
        p.file("20261027110000_role.sql", &format!("create role {role};"));
        assert_eq!(p.migrate().await, 1);
        assert_has!(p.output(), r"can't be replayed in a shadow database\n  [^\n]*create role");
        assert!(p.rows(&format!("select 1 from pg_roles where rolname = '{role}'")).await.is_empty(), "no shadow created the role on the server");
    }

    pub async fn an_object_the_comparison_does_not_read_is_not_a_match(mut p: P) {
        p.schema("create table t (x int); create aggregate total (int) (sfunc = int4pl, stype = int);");
        p.file("20261027100000_t.sql", "create table t (x int);");
        assert_eq!(p.dry_run().await, 1, "schema.sql declares an aggregate no migration makes:\n{}", p.peek());
    }

    pub async fn a_failed_concurrent_index_leaves_nothing(mut p: P) {
        p.schema("create table t (x int);");
        p.file("20261027100000_t.sql", "create table t (x int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into t values (1), (1)").await;
        p.file("20261027110000_unique.sql", "create unique index concurrently t_x on t (x);");
        p.schema("create table t (x int); create unique index t_x on t (x);");
        p.output();
        assert_eq!(p.migrate().await, 1);
        assert_has!(p.output(), "could not create unique index");
        assert!(p.rows("select 1 from pg_class where relname = 't_x'").await.is_empty(), "the invalid index was dropped");
        p.exec("delete from t where ctid = (select max(ctid) from t)").await;
        assert_eq!(p.migrate().await, 0, "the file runs again as written:\n{}", p.peek());
    }

    pub async fn a_platforms_objects_are_left_out(mut p: P) {
        // A Supabase-like platform, made by hand: its roles, its schema, its table.
        p.exec("do $$ begin if not exists (select 1 from pg_roles where rolname = 'supabase_auth_admin') then create role supabase_auth_admin; end if; end $$").await;
        p.exec("create schema auth authorization supabase_auth_admin; create table auth.users (id int primary key); alter table auth.users owner to supabase_auth_admin; create function auth.uid() returns int language sql as 'select 1'; alter function auth.uid() owner to supabase_auth_admin").await;
        p.schema("create table profiles (id int primary key);");
        p.file("20261029100000_profiles.sql", "create table profiles (id int primary key);");
        assert_eq!(p.migrate().await, 0, "the platform's schema, table and function are not drift:\n{}", p.peek());
        // The project's own objects on the platform's table are the project's: one added by hand is drift.
        p.exec("create policy own on auth.users using (true)").await;
        p.output();
        assert_eq!(p.migrate().await, 1);
        assert_has!(p.output(), r"This database has changes no migration made[\s\S]*policy own on auth\.users");
    }

    pub async fn reylo_run_runs_inline_sql_and_files(mut p: P) {
        let items = [("create".to_string(), "create table t (x int); insert into t values (1)".to_string()), ("seed.sql".to_string(), "insert into t values (2)".to_string())];
        let (url, dir) = (p.url.clone(), p.dir.clone());
        assert_eq!(p.run(async |_, log| reylo_migrate::run_sql(&url, &dir, &items, log).await).await, 0, "{}", p.peek());
        assert_eq!(p.rows("select x from t order by x").await.iter().map(|r| r.text("x").to_string()).collect::<Vec<_>>(), ["1", "2"]);
        assert_eq!(p.rows("select to_regclass('__reylo_migrations') as t").await[0].get("t"), None, "records nothing");
    }

    pub async fn repair_records_a_data_migration_as_run(mut p: P) {
        p.schema("create table t (x int);");
        p.file("20261027100000_t.sql", "create table t (x int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into t values (1)").await;
        p.file("20261027110000_backfill.sql", "update t set x = x * 10;");
        p.output();
        assert_eq!(p.run(async |o, log| reylo_migrate::repair(o, log).await).await, 0);
        assert_has!(p.output(), r"20261027110000_backfill\.sql recorded as applied, not run");
        assert_eq!(p.rows("select x from t").await[0].text("x"), "1", "repair ran nothing");
    }

    pub async fn a_platforms_default_privileges_are_not_drift(mut p: P) {
        // As a platform sets them before any migration: every new table and function its API's role may use.
        p.exec("do $$ begin if not exists (select 1 from pg_roles where rolname = 'madv_api') then create role madv_api; end if; end $$").await;
        p.exec("alter default privileges in schema public grant select on tables to madv_api; alter default privileges grant execute on functions to madv_api").await;
        p.schema("create table t (id int); create function f() returns int language sql as 'select 1';");
        p.file("20261030100000_t.sql", "create table t (id int); create function f() returns int language sql as 'select 1';");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.migrate().await, 0, "the grants the defaults made are not drift:\n{}", p.peek());
        assert!(!p.rows("select 1 from information_schema.role_table_grants where grantee = 'madv_api' and table_name = 't'").await.is_empty(), "the target granted it");
    }

    pub async fn function_privileges_and_index_comments_are_compared(mut p: P) {
        p.schema("create table t (x int); create index t_x on t (x); comment on index t_x is 'by x'; create function f() returns int language sql as 'select 1'; revoke execute on function f() from public;");
        p.file("20261030100000_t.sql", "create table t (x int); create index t_x on t (x); create function f() returns int language sql as 'select 1';");
        assert_eq!(p.dry_run().await, 1);
        let out = p.output();
        assert!(has(&out, r"revoke execute on function f\(\) from public"), "{out}");
        assert!(has(&out, r"comment on index t_x is 'by x'"), "{out}");
    }

    pub async fn a_server_setting_change_misses_the_cache(mut p: P) {
        p.schema("create schema app; create table t (id int);");
        p.file("20261027100000_app.sql", "create schema app;");
        p.file("20261027110000_t.sql", "create table t (id int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.output();
        assert_eq!(p.check_every_way().await.0, 0);
        // The same server, through sessions whose search_path is app's: every replay now puts t in
        // app, which the live table is not in. A cache keyed without the session's settings would
        // answer that nothing changed.
        let plain = p.url.clone();
        p.url = format!("{plain}?options=-c%20search_path%3Dapp%2Cpublic");
        let (code, out) = p.check_every_way().await;
        assert_eq!(code, 1);
        assert_has!(out, r"This database (differs from what its migrations made|has changes no migration made|is missing something its migrations made)");
        forget(&p.url).await;
        // The target's own database setting, which every shadow takes, likewise.
        p.url = plain;
        p.exec(&format!("alter database {} set search_path = app, public", p.name)).await;
        let (code, out) = p.check_every_way().await;
        assert_eq!(code, 1);
        assert_has!(out, r"This database (differs from what its migrations made|has changes no migration made|is missing something its migrations made)");
    }

    pub async fn a_migrations_session_setting_is_never_carried_by_the_cache(mut p: P) {
        // The first migration leaves its session's search_path on app, which puts the second's
        // table there in a replay; the second sets it back. A shadow copied from a cache opens a new
        // session, without it.
        p.file("20261027100000_app.sql", "create schema app; set search_path = app, public;");
        p.file("20261027110000_t.sql", "create table t (id int); reset search_path;");
        p.file("20261027120000_u.sql", "create table u (id int);");
        p.schema("create schema app; create table app.t (id int); create table u (id int);");
        assert_eq!(p.check_every_way().await.0, 0);
        // The target applies the first alone, so the check's history stops in the middle.
        let later = ["20261027110000_t.sql", "20261027120000_u.sql"].map(|f| (f, std::fs::read_to_string(p.dir.join("migrations").join(f)).unwrap()));
        for (f, _) in &later {
            std::fs::remove_file(p.dir.join("migrations").join(f)).unwrap();
        }
        p.schema("create schema app;");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.output();
        for (f, sql) in &later {
            p.file(f, sql);
        }
        p.schema("create schema app; create table app.t (id int); create table u (id int);");
        p.check_every_way().await;
    }

    pub async fn a_narrowing_type_change_fails_rather_than_cuts(mut p: P) {
        p.schema("create table t (code text);");
        p.file("20261027100000_t.sql", "create table t (code text);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into t values ('abcdef')").await;
        // create writes the change as the server's assignment conversion, which refuses a value
        // the new type cannot hold; an explicit cast to varchar(3) would cut it to 'abc'.
        p.schema("create table t (code varchar(3));");
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "narrow", &Default::default(), log).await).await, 0, "{}", p.peek());
        let made = p.files().into_iter().find(|f| f.ends_with("_narrow.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(&made)).unwrap();
        assert!(!sql.contains("using"), "{sql}");
        p.output();
        assert_eq!(p.migrate().await, 1, "the value does not fit");
        assert_has!(p.output(), "too long");
        assert_eq!(p.rows("select code from t").await[0].text("code"), "abcdef");
    }

    pub async fn a_migration_that_names_a_database_is_refused(mut p: P) {
        p.schema("create table t (id int);");
        p.file("20261027100000_t.sql", &format!("create table {}.t (id int);", p.name));
        assert_eq!(p.migrate().await, 1);
        assert_has!(p.output(), "names the database");
        assert!(p.rows(&format!("select 1 from information_schema.tables where table_schema = '{}' and table_name = 't'", p.name)).await.is_empty());
    }
}
