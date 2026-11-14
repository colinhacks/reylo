//! Scenarios for `reylo migrate`: each builds a project and a fresh database, then does what teams
//! do (apply, edit, drift, a branch switch, history push, migrations that cannot be atomic, a run cut off
//! halfway) and checks what the command refuses and what the database ends up as. The adversarial
//! ones are chosen to break it: ways a shadow can differ from the target that are not the
//! migrations' doing, objects the comparison does not read, and commands that record what they did
//! not run. Postgres's and MySQL's run against the servers `REYLO_TEST_PG` and `REYLO_TEST_MYSQL`
//! name, and are skipped where the variable is not set; SQLite's always run; DuckDB's run on the
//! DuckDB library `REYLO_DUCKDB_LIBRARY` names, and are skipped where it names none.

use reylo_migrate::catalog::catalog;
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
        Dialect::Duckdb => std::env::var_os("REYLO_DUCKDB_LIBRARY").map(|_| String::new()),
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
            Dialect::Duckdb => format!("duckdb:{}", dir.join("dev.duckdb").display()),
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
            // DuckDB has no column that numbers itself but through a sequence: a random key takes
            // rows that name no id, and those that do.
            Dialect::Duckdb => Types { id: "bigint primary key default (random() * 1e15)::bigint", text: "varchar", int: "bigint" },
        };
        Some(P { d, dir, url, name, lines: vec![], t, strict: false })
    }

    /// A second project directory of the same dialect, with no database of its own yet.
    fn sibling(p: &P) -> P {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("reylo-migrate-{}-sib_{}_{n}", p.d.name(), std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("migrations")).unwrap();
        let t = match p.d {
            Dialect::Postgres => Types { id: p.t.id, text: p.t.text, int: p.t.int },
            _ => Types { id: p.t.id, text: p.t.text, int: p.t.int },
        };
        let url = match p.d {
            Dialect::Sqlite => format!("sqlite:{}", dir.join("dev.db").display()),
            Dialect::Duckdb => format!("duckdb:{}", dir.join("dev.duckdb").display()),
            _ => p.url.clone(),
        };
        P { d: p.d, dir, url, name: p.name.clone(), lines: vec![], t, strict: p.strict }
    }

    fn opts(&self) -> Options {
        Options {
            url: self.url.clone(),
            shadow_url: None,
            dir: self.dir.join("migrations"),
            schema: self.dir.join("schema.sql"),
            lock: self.dir.join(reylo_migrate::lock::LOCK),
            cache: self.dir.join(".cache"),
            root: self.dir.clone(),
        }
    }

    async fn run<'a, F>(&mut self, f: F) -> i32
    where
        F: AsyncFnOnce(&Options, &mut dyn FnMut(&str)) -> Result<i32, reylo_migrate::Error>,
    {
        if !self.strict {
            self.relock();
        }
        let opts = self.opts();
        let mut lines = std::mem::take(&mut self.lines);
        let code = f(&opts, &mut |l: &str| lines.push(l.to_string())).await.unwrap_or_else(|e| panic!("refused to start: {e}"));
        self.lines = lines;
        code
    }

    async fn migrate(&mut self) -> i32 {
        self.run(async |o, log| reylo_migrate::deploy(o, log).await).await
    }

    async fn history_push(&mut self) -> i32 {
        self.run(async |o, log| reylo_migrate::history_push(o, log).await).await
    }

    /// `db push` alone.
    async fn db_push(&mut self) -> i32 {
        self.run(async |o, log| reylo_migrate::db_push(o, false, log).await).await
    }

    /// `dev`, then `db push` where it succeeded: a development database taken to schema.sql.
    async fn dev_then_push(&mut self) -> i32 {
        let code = self.run(async |o, log| reylo_migrate::dev(o, false, log).await).await;
        if code != 0 {
            return code;
        }
        self.run(async |o, log| reylo_migrate::db_push(o, false, log).await).await
    }

    /// `deploy --dry-run`: what deploy checks, and the migrations against schema.sql.
    async fn dry_run(&mut self) -> i32 {
        self.run(async |o, log| reylo_migrate::dry_run(o, log).await).await
    }

    /// `deploy --dry-run` with the replay cache on or off, and what it printed, but for what differs
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
        let _ = std::fs::remove_dir_all(self.dir.join(".cache").join("migrate").join("lines"));
        let copied = self.check_with(true).await;
        assert_eq!(cold, off, "a cold cache changed the check");
        assert_eq!(warm, off, "a warm cache changed the check");
        assert_eq!(copied, off, "the cache's databases changed the check");
        off
    }

    /// The lock as development leaves it, listing every file by name, for a test that isn't about the
    /// lock (a test that is sets `strict`, and keeps it itself). What it holds of schema.sql stays where
    /// its lines do.
    fn relock(&self) {
        let path = self.dir.join(reylo_migrate::lock::LOCK);
        let Ok((files, _)) = reylo_migrate::commands::read_migrations(&self.dir.join("migrations")) else { return };
        let before = match reylo_migrate::lock::read(&path) {
            reylo_migrate::lock::State::Lock(l) => Some(l),
            _ => None,
        };
        if files.is_empty() && before.is_none() {
            return;
        }
        let texts: Vec<(String, String)> = files.iter().map(|f| (f.name.clone(), f.sql.replace("\r\n", "\n"))).collect();
        let entries = reylo_migrate::lock::chain(texts.iter().map(|(n, t)| (n.as_str(), t.as_str())));
        if before.as_ref().map(|l| &l.entries) != Some(&entries) {
            reylo_migrate::lock::write(&path, &reylo_migrate::lock::Lock { schema: None, entries }).unwrap();
        }
    }

    /// A project with linear history, as `reylo migrate init` or `reylo migrate lock` starts one: an
    /// empty lock, which the commands themselves keep from here.
    fn linear(&mut self) {
        self.strict = true;
        reylo_migrate::lock::write(&self.dir.join(reylo_migrate::lock::LOCK), &reylo_migrate::lock::Lock::default()).unwrap();
    }

    /// A lock listing `names`, chained as Reylo chains it, from the files' texts.
    fn lock(&self, names: &[&str]) {
        let texts: Vec<(String, String)> = names.iter().map(|n| (n.to_string(), std::fs::read_to_string(self.dir.join("migrations").join(n)).unwrap().replace("\r\n", "\n").trim_end().to_string())).collect();
        let l = reylo_migrate::lock::Lock { schema: None, entries: reylo_migrate::lock::chain(texts.iter().map(|(n, t)| (n.as_str(), t.as_str()))) };
        reylo_migrate::lock::write(&self.dir.join(reylo_migrate::lock::LOCK), &l).unwrap();
    }

    fn locked(&self) -> Vec<String> {
        match reylo_migrate::lock::read(&self.dir.join(reylo_migrate::lock::LOCK)) {
            reylo_migrate::lock::State::Lock(l) => l.names(),
            other => panic!("the lock reads {other:?}"),
        }
    }

    async fn dev(&mut self) -> i32 {
        self.run(async |o, log| reylo_migrate::dev(o, false, log).await).await
    }

    async fn status(&mut self) -> i32 {
        self.run(async |o, log| reylo_migrate::status(o, log).await).await
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

    /// The applied migrations, in the order Reylo reads them: by time, and on SQLite, whose clock has
    /// milliseconds, a tie by the order the rows were written.
    async fn history(&self) -> Vec<String> {
        let tie = if self.d == Dialect::Sqlite { "rowid, " } else { "" };
        self.try_rows(&format!("select name from __reylo_migrations where applied_at is not null order by applied_at, {tie}case when name = 'init.sql' then '' else name end"))
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
    named.lines().filter(|l| !has(l, r"^· Dropped \d+ shadow databases? a run left behind$")).collect::<Vec<_>>().join("\n")
}

/// Drops the replay caches only this test's target used, which would otherwise stay on the test
/// server for a week.
async fn forget(url: &str) {
    let _ = reylo_migrate::cache::forget(url).await;
}

/// The SQL of the `reylo run` a refusal gives to undo a change, as the shell would pass it.
fn undo_in(out: &str) -> String {
    let quoted = regex::Regex::new(r"reylo run '((?:[^']|'\\'')*)'").unwrap().captures(out).unwrap_or_else(|| panic!("no undo in:\n{out}"))[1].to_string();
    quoted.replace("'\\''", "'")
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
            #[tokio::test(flavor = "current_thread")]
            async fn duckdb() {
                if let Some(p) = super::P::new(super::Dialect::Duckdb).await { super::$scenario(p).await }
            }
        }
    )*};
}

for_each_dialect!(
    dev_that_fails_on_a_locked_migration_keeps_the_lock,
    dev_that_fails_on_a_draft_locks_the_drafts_that_ran,
    the_undo_of_a_change_by_hand_leaves_what_db_push_made,
    migrate_lock_refuses_a_lock_that_exists,
    status_refuses_a_lock_with_no_checksum_of_schema_sql,
    dev_relocks_an_edited_locked_migration_where_every_pair_agreed,
    without_a_lock_a_late_migration_runs_where_both_orders_agree,
    without_a_lock_init_sql_still_runs_first,
    status_shows_what_ran_what_is_pending_and_what_changed,
    lock_writes_the_lock_from_what_a_database_ran,
    db_reset_rebuilds_a_database_from_its_migrations,
    db_push_refuses_a_schema_that_declares_nothing,
    a_merge_conflict_left_in_a_file_is_refused,
    new_without_schema_sql_writes_what_the_database_has,
    a_change_by_hand_before_dev_without_schema_sql_is_refused,
    applies_pending_migrations_once,
    refuses_migrations_that_do_not_add_up,
    column_order_is_not_compared,
    refuses_drift,
    an_edited_migration_is_refused,
    history_pull_takes_back_what_the_lock_refuses,
    ddl_run_by_hand_then_written_is_recorded,
    a_change_by_hand_with_schema_sql_is_told_to_write_the_migration,
    init_writes_and_records_the_baseline,
    a_migration_the_lock_lacks_is_a_draft,
    a_rebased_migration_is_rewound_by_dev,
    history_pull_writes_the_lock_and_keeps_what_is_still_to_deploy,
    db_introspect_prints_the_schema,
    a_hotfix_by_hand_is_recorded_once_it_is_a_migration,
    history_push_follows_a_renamed_migration,
    a_failed_migration_leaves_nothing,
    files_that_cannot_run_atomically_are_refused,
    a_lone_statement_cut_off_that_landed,
    a_lone_statement_cut_off_that_did_not_land,
    a_lone_statement_cut_off_with_no_fingerprint_to_compare,
    a_shadow_left_by_a_killed_run_is_swept,
    a_rebuilt_table_does_not_cascade,
    one_schema_built_two_ways_has_one_catalog,
    cached_and_uncached_checks_print_the_same_lines,
    a_cached_prefix_replays_only_what_follows_it,
    an_edited_migration_misses_the_cache,
    concurrent_runs_building_one_cache_both_succeed,
    create_drafts_one_migration_cached_or_not,
    two_applies_at_once_apply_each_migration_once,
    create_states_what_it_drops_and_renames,
    wipe_deletes_rows_and_keeps_the_schema,
    deploy_builds_no_shadow_database_and_trusts_the_fingerprint,
    deploy_goes_on_where_it_has_no_fingerprint_to_compare,
    deploy_refuses_tables_with_no_history,
    a_lock_whose_order_is_not_the_names_survives_one_run,
    a_fixed_migration_no_database_ran_is_locked_again,
    init_locks_the_baseline_in_a_locked_project,
    history_push_keeps_a_data_migration_that_ran,
    create_guesses_no_rename_of_unrelated_tables,
    the_drift_advice_in_a_locked_project_locks_the_migration_first,
    create_names_a_type_change_that_loses_values_in_drop,
    create_assumes_a_rename_where_one_goes_and_one_comes,
    db_push_watch_syncs_again_when_schema_sql_changes,
    db_push_takes_a_development_database_to_schema_sql,
    db_push_rewinds_what_no_longer_agrees,
    db_push_keeps_what_no_migration_makes_yet,
    apply_dev_locks_a_draft_after_what_is_locked,
    a_locked_project_runs_its_lock,
    a_lock_conflict_is_resolved_by_apply_dev,
    create_resolves_the_names_drop_gives,
    create_renames_a_table_keeping_its_rows,
    create_lists_every_rename_it_assumes,
    commands_create_prints_for_names_with_spaces_run_as_printed,
    deploy_compares_with_what_dev_replayed,
    deploy_refuses_any_schema_with_no_history,
    init_on_an_empty_database_writes_an_empty_schema_and_lock,
    a_broken_draft_keeps_what_sync_made,
    a_migrations_directory_that_isnt_there_is_refused,
    a_run_cut_off_between_migrations_is_still_compared,
    create_settles_every_guess_with_one_command,
    db_push_renames_a_table_keeping_its_rows,
    db_push_renames_a_table_back_when_it_rewinds,
    db_push_keeps_what_it_records_when_it_undoes_the_rest,
    db_push_runs_or_records_a_migration_cut_off,
    db_push_rewinds_only_what_the_rewound_migrations_made,
    create_guesses_no_rename_a_table_that_stays_could_explain,
    every_applied_row_holds_the_fingerprint_it_left,
    a_run_cut_off_before_its_fingerprint_is_refused,
);

async fn the_undo_of_a_change_by_hand_leaves_what_db_push_made(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.schema(&format!("create table users (id {id}, name {text});"));
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, name {text});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.schema(&format!("create table users (id {id}, name {text}, bio {text});"));
    assert_eq!(p.db_push().await, 0, "{}", p.peek());
    p.exec("insert into users (name, bio) values ('ada', 'pushed')").await;
    p.exec("create index hand_idx on users (name)").await;
    p.output();
    // Only the index is refused: schema.sql explains the column db push made.
    assert_eq!(p.dev().await, 1);
    let out = p.output();
    // On MySQL a table's definition holds its columns, so its index and the pushed column can't be
    // undone apart: no command is offered, rather than one that undoes too much.
    let undo = if p.d == Dialect::Mysql {
        assert_has!(&out, "To undo it, change it back by hand");
        "drop index hand_idx on users".to_string()
    } else {
        undo_in(&out)
    };
    assert!(undo.contains("hand_idx") && !undo.contains("bio"), "{out}");
    p.exec(&undo).await;
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select bio from users").await[0].text("bio"), "pushed");
}

async fn migrate_lock_refuses_a_lock_that_exists(mut p: P) {
    let int = p.t.int;
    p.linear();
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.run(async |o, log| reylo_migrate::lock_from_history(o, log).await).await, 1);
    assert_has!(p.output(), r"✗ \S*reylo-lock\.yaml exists already");
    assert_eq!(p.locked(), ["20261027100000_a.sql"]);
}

async fn status_refuses_a_lock_with_no_checksum_of_schema_sql(mut p: P) {
    let int = p.t.int;
    p.linear();
    p.schema(&format!("create table a (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    // The lock written again where the migrations don't make schema.sql holds no checksum of it.
    p.schema(&format!("create table a (id {int}); create table b (id {int});"));
    std::fs::remove_file(p.dir.join(reylo_migrate::lock::LOCK)).unwrap();
    assert_eq!(p.run(async |o, log| reylo_migrate::lock_from_history(o, log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.status().await, 1);
    assert_has!(p.output(), r"✗ \S*reylo-lock\.yaml holds no checksum of \S*schema\.sql");
}

async fn dev_that_fails_on_a_locked_migration_keeps_the_lock(mut p: P) {
    let int = p.t.int;
    p.linear();
    p.file("20261027100000_a.sql", &format!("create table a (id {int} primary key);"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.exec("insert into a (id) values (1)").await;
    // Main's lines, merged in: a2 runs, b then fails on this database's rows, and c comes after it.
    p.file("20261027105000_a2.sql", &format!("create table a2 (id {int});"));
    p.file("20261027110000_b.sql", "insert into a (id) values (1);");
    p.file("20261027120000_c.sql", &format!("create table c (id {int});"));
    p.lock(&["20261027100000_a.sql", "20261027105000_a2.sql", "20261027110000_b.sql", "20261027120000_c.sql"]);
    // And a draft of this branch's own, so that the run goes through the shadows.
    p.file("20261027130000_d.sql", &format!("create table d (id {int});"));
    let lock = std::fs::read_to_string(p.dir.join(reylo_migrate::lock::LOCK)).unwrap();
    p.output();
    assert_eq!(p.dev().await, 1);
    assert_has!(p.output(), r"✓ 20261027105000_a2\.sql +applied[\s\S]*✗ 20261027110000_b\.sql +failed");
    assert_eq!(std::fs::read_to_string(p.dir.join(reylo_migrate::lock::LOCK)).unwrap(), lock, "the lock keeps the lines every database runs");
}

async fn dev_that_fails_on_a_draft_locks_the_drafts_that_ran(mut p: P) {
    let int = p.t.int;
    p.linear();
    p.file("20261027100000_a.sql", &format!("create table a (id {int} primary key);"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.exec("insert into a (id) values (1)").await;
    // This branch's two: x runs, then y fails on this database's rows.
    p.file("20261027110000_x.sql", &format!("create table x (id {int});"));
    p.file("20261027120000_y.sql", "insert into a (id) values (1);");
    p.output();
    assert_eq!(p.dev().await, 1);
    let out = p.output();
    assert_has!(&out, r"✓ 20261027110000_x\.sql +applied[\s\S]*✗ 20261027120000_y\.sql +failed[\s\S]*✓ Wrote \S*reylo-lock\.yaml, through 20261027110000_x\.sql");
    assert_eq!(p.locked(), ["20261027100000_a.sql", "20261027110000_x.sql"]);
}

async fn dev_relocks_an_edited_locked_migration_where_every_pair_agreed(mut p: P) {
    let int = p.t.int;
    p.linear();
    p.file("20261027100000_one.sql", &format!("create table a (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    // Another database runs and locks the next migration; this one hasn't run it, and it is fixed.
    p.file("20261027110000_two.sql", &format!("create table b (id {int});"));
    let other = P::new(p.d).await.unwrap();
    let dev_url = std::mem::replace(&mut p.url, other.url.clone());
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.url = dev_url;
    p.file("20261027110000_two.sql", &format!("create table c (id {int});"));
    p.output();
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Wrote \S*reylo-lock\.yaml");
    assert_eq!(p.status().await, 0, "{}", p.peek());
}

async fn without_a_lock_a_late_migration_runs_where_both_orders_agree(mut p: P) {
    let int = p.t.int;
    p.strict = true;
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027120000_c.sql", &format!("create table c (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert!(!p.dir.join(reylo_migrate::lock::LOCK).exists(), "dev writes no lock where there is none");
    assert_eq!(p.status().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"No lock: migrations run in name order");
    let other = P::new(p.d).await.unwrap();
    // Another database deploys them; then one that sorts between them arrives, as from a merge.
    let deploy_on = async |p: &mut P, url: &str| {
        let dev_url = std::mem::replace(&mut p.url, url.to_string());
        let code = p.migrate().await;
        p.url = dev_url;
        code
    };
    assert_eq!(deploy_on(&mut p, &other.url).await, 0, "{}", p.peek());
    p.file("20261027110000_b.sql", &format!("create table b (id {int});"));
    p.output();
    // Its order and name order end the same, so it runs where it is, in development and at deploy.
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_eq!(p.history().await, ["20261027100000_a.sql", "20261027120000_c.sql", "20261027110000_b.sql"]);
    assert_eq!(deploy_on(&mut p, &other.url).await, 0, "{}", p.peek());
    assert_eq!(other.history().await, ["20261027100000_a.sql", "20261027120000_c.sql", "20261027110000_b.sql"]);
    // One whose two orders end differently is refused in both, with the way out.
    p.file("20261027140000_d.sql", &format!("alter table a add column x {int};"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_eq!(deploy_on(&mut p, &other.url).await, 0, "{}", p.peek());
    p.file("20261027130000_e.sql", "create table e as select * from a;");
    p.output();
    assert_eq!(p.dev().await, 1);
    let out = p.output();
    assert_has!(&out, r"✗ 20261027130000_e\.sql sorts before 20261027140000_d\.sql, which this database already ran, and the two orders end differently");
    assert_has!(&out, r"Give 20261027130000_e\.sql a later name, so that every database runs it after 20261027140000_d\.sql");
    assert_eq!(deploy_on(&mut p, &other.url).await, 1);
    assert_has!(p.output(), r"sorts before 20261027140000_d\.sql");
    // Renamed to sort last, it runs.
    std::fs::rename(p.dir.join("migrations/20261027130000_e.sql"), p.dir.join("migrations/20261027150000_e.sql")).unwrap();
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_eq!(deploy_on(&mut p, &other.url).await, 0, "{}", p.peek());
}

async fn without_a_lock_init_sql_still_runs_first(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.exec(&format!("create table users (id {id}, email {text})")).await;
    assert_eq!(p.run(async |o, log| reylo_migrate::init(o, &[], log).await).await, 0, "{}", p.peek());
    p.strict = true;
    std::fs::remove_file(p.dir.join(reylo_migrate::lock::LOCK)).unwrap();
    std::fs::remove_file(p.dir.join("schema.sql")).unwrap();
    p.file("20261028100000_nick.sql", &format!("alter table users add column nick {text};"));
    p.output();
    // init.sql sorts before every migration, as it runs: nothing here is late.
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    let q = P::new(p.d).await.unwrap();
    p.url = q.url.clone();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(q.history().await, ["init.sql", "20261028100000_nick.sql"]);
}

async fn status_shows_what_ran_what_is_pending_and_what_changed(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.linear();
    p.schema(&format!("create table a (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.file("20261027110000_b.sql", &format!("create table b (id {int});"));
    p.output();
    // A migration this database hasn't run is out of step: status exits 1.
    assert_eq!(p.status().await, 1);
    let out = p.output();
    assert_has!(&out, r"✓ 20261027100000_a\.sql +applied \d{4}-\d\d-\d\d \d\d:\d\d");
    assert_has!(&out, r"· 20261027110000_b\.sql +pending");
    assert_has!(&out, r"✓ Unchanged since its last migration");
    assert_has!(&out, r"✓ The migrations match \S*schema\.sql");
    assert_has!(&out, r"! 1 migration hasn't run here: reylo migrate deploy runs it");
    assert_has!(&out, r"✗ 20261027110000_b\.sql isn't locked yet");
    // Run, it is in step: status exits 0.
    p.schema(&format!("create table a (id {int}); create table b (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.status().await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"✓ 20261027110000_b\.sql +applied");
    assert_has!(&out, r"✓ \S*reylo-lock\.yaml matches the migration files\n✓ The migrations match \S*schema\.sql$");
    // A change by hand shows; status changes nothing, and exits 1.
    p.exec(&format!("alter table a add column note {text}")).await;
    assert_eq!(p.status().await, 1);
    assert_has!(p.output(), r"✗ This database was changed outside its migrations\n\n  \+ column a\.note");
    assert_eq!(p.history().await, ["20261027100000_a.sql", "20261027110000_b.sql"]);
}

async fn lock_writes_the_lock_from_what_a_database_ran(mut p: P) {
    let int = p.t.int;
    p.linear();
    p.schema(&format!("create table a (id {int}); create table b (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027110000_b.sql", &format!("create table b (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    let before = std::fs::read_to_string(p.dir.join(reylo_migrate::lock::LOCK)).unwrap();
    std::fs::remove_file(p.dir.join(reylo_migrate::lock::LOCK)).unwrap();
    p.output();
    assert_eq!(p.run(async |o, log| reylo_migrate::lock_from_history(o, log).await).await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"✓ Wrote \S*reylo-lock\.yaml: 2 migrations, in the order this database ran them");
    assert_has!(&out, r"✓ The migrations match \S*schema\.sql");
    assert_eq!(std::fs::read_to_string(p.dir.join(reylo_migrate::lock::LOCK)).unwrap(), before);
    // A migration the database ran that isn't in the migrations as it ran it is refused.
    std::fs::remove_file(p.dir.join(reylo_migrate::lock::LOCK)).unwrap();
    p.file("20261027110000_b.sql", &format!("create table b (id {int}, x {int});"));
    assert_eq!(p.run(async |o, log| reylo_migrate::lock_from_history(o, log).await).await, 1);
    assert_has!(p.output(), r"Nothing was changed");
    assert!(!p.dir.join(reylo_migrate::lock::LOCK).exists());
}

async fn db_reset_rebuilds_a_database_from_its_migrations(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.schema(&format!("create table users (id {id}, name {text});"));
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, name {text});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.exec("insert into users (name) values ('ada')").await;
    p.exec(&format!("create table stray (id {int})")).await;
    p.output();
    assert_eq!(p.run(async |o, log| reylo_migrate::db_reset(o, false, log).await).await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"Dropped [^\n]*users \(1 row\)");
    assert_has!(&out, r"✓ 20261027100000_users\.sql +applied");
    assert!(p.try_rows("select 1 from stray").await.is_err(), "{out}");
    assert!(p.rows("select name from users").await.is_empty());
    assert_eq!(p.history().await, ["20261027100000_users.sql"]);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // Never in CI.
    assert_eq!(p.run(async |o, log| reylo_migrate::db_reset(o, true, log).await).await, 1);
    assert_has!(p.output(), r"CI is set, and reylo db reset is for development databases");
}

async fn db_push_refuses_a_schema_that_declares_nothing(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.schema(&format!("create table users (id {id}, name {text});"));
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, name {text});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.exec("insert into users (name) values ('ada')").await;
    p.schema("-- emptied\n\n");
    p.output();
    assert_eq!(p.db_push().await, 1);
    assert_has!(p.output(), r"✗ \S*schema\.sql declares nothing\n  Taking this database to it would drop every table, with its rows\.");
    assert_eq!(p.rows("select name from users").await.len(), 1);
}

async fn a_merge_conflict_left_in_a_file_is_refused(mut p: P) {
    let int = p.t.int;
    p.schema(&format!("create table a (id {int});\n<<<<<<< HEAD\ncreate table b (id {int});\n=======\ncreate table c (id {int});\n>>>>>>> main\n"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.dev().await, 1);
    assert_has!(p.output(), r"schema\.sql has a merge conflict in it");
    assert_eq!(p.status().await, 1);
    assert_has!(p.output(), r"✗ \S*schema\.sql has a merge conflict in it");
}

async fn new_without_schema_sql_writes_what_the_database_has(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, name {text});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    // Changed by hand on the development database, then written as a migration.
    p.exec(&format!("create table tags (id {int})")).await;
    p.output();
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tags", &Default::default(), log).await).await, 0, "{}", p.peek());
    let written = p.files().into_iter().find(|f| f.ends_with("_tags.sql")).unwrap();
    let sql = std::fs::read_to_string(p.dir.join("migrations").join(&written)).unwrap();
    assert_has!(&sql, r"(?i)create table [^\n]*tags");
    // dev records it as in place, and the database is its migrations again.
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_eq!(p.history().await, ["20261027100000_users.sql".to_string(), written.clone()]);
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"✓ Up to date");
    // With nothing changed, the migration is empty, for a change of data.
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "backfill", &Default::default(), log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"empty: this database has nothing new");
}

async fn a_change_by_hand_before_dev_without_schema_sql_is_refused(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, name {text});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    // Another branch's migration, merged and not yet run, then a change by hand.
    p.file("20261027110000_orders.sql", &format!("create table orders (id {int});"));
    p.exec(&format!("alter table users add column nickname {text}")).await;
    p.output();
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "nickname", &Default::default(), log).await).await, 1);
    assert_has!(p.output(), r"20261027110000_orders\.sql isn't applied to this database");
    assert_eq!(p.dev().await, 1);
    let out = p.output();
    assert_has!(&out, r"✗ This database has a change no migration makes yet");
    assert!(!out.contains("To keep it"), "{out}");
    p.exec(&undo_in(&out)).await;
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_eq!(p.history().await, ["20261027100000_users.sql", "20261027110000_orders.sql"]);
}

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
    assert_has!(p.output(), r"\S*schema\.sql has what no migration makes yet\n\n    alter table users add column name [^\n]+;\n\n  Write it as a migration:\n    reylo migrate new <name>");
    assert!(p.try_rows("select 1 from users").await.is_err(), "nothing was applied");
    // apply moves the migrations into the database and never reads schema.sql.
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert!(p.try_rows("select 1 from users").await.is_ok());
    // The other way round: a migration makes what schema.sql doesn't declare.
    p.schema(&format!("create table users (id {}, email {} not null);", p.t.id, p.t.text));
    p.file("20261027110000_tags.sql", &format!("create table tags (id {});", p.t.int));
    p.output();
    assert_eq!(p.dry_run().await, 1);
    assert_has!(p.output(), r"The migrations make what \S*schema\.sql doesn't declare[\s\S]*tags");
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
    // deploy sees that it changed; check says how.
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"✗ This database was changed outside its migrations\n\n  \+ column users\.hotfix [^\n]+\n\n[\s\S]*reylo migrate deploy --dry-run\n\nNothing was changed\.$");
    assert_eq!(p.dry_run().await, 1);
    let out = p.output();
    assert_has!(&out, r"✗ This database was changed outside its migrations\n\n    alter table users add column hotfix [^\n]+;\n");
    assert_has!(&out, r"To keep it, add it to \S*schema\.sql and write its migration in development:\n    reylo migrate new <name>\n    reylo migrate dev\n  Then record it here, running nothing:\n    reylo migrate history push");
    assert_eq!(p.history().await, ["20261027100000_users.sql"]);
}

async fn an_edited_migration_is_refused(mut p: P) {
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
    assert_has!(&out, r"✗ 20261027100000_users\.sql changed after this database applied it\n\n    --- as applied\n    \+\+\+ on disk\n    - create table users \(id [^\n]+\);\n    \+ create table users \(id [^\n]+, email [^\n]+\);\n\n  Write the change as a new migration\.\n\n  Take back the text this database ran:\n    reylo migrate history pull\n  On a development database, run it again as it now reads:\n    reylo migrate dev");
    p.exec(&format!("alter table users add column email {text}")).await;
    assert_eq!(p.migrate().await, 1, "a database that matches the edit records nothing without being told: {}", p.peek());
    assert_has!(p.output(), "changed after this database applied it");
    p.exec("alter table users drop column email").await;
    // pull takes back the text it ran.
    assert_eq!(p.run(async |o, log| reylo_migrate::history_pull(o, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Pulling from [^\n]+\n  ↓ 20261027100000_users\.sql  written over: it differed from what ran");
    assert_eq!(std::fs::read_to_string(p.dir.join("migrations").join("20261027100000_users.sql")).unwrap(), original);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), "Up to date");
    // A deleted one is written again; a draft is left as it is.
    std::fs::remove_file(p.dir.join("migrations").join("20261027100000_users.sql")).unwrap();
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"This database applied 20261027100000_users\.sql, which isn't in [^\n]*migrations\n\n    create table users \(id [^\n]+\);\n\n  Write it back as it ran:\n    reylo migrate history pull\n  On a development database, undo it instead:\n    reylo migrate dev");
    p.file("20261027110000_draft.sql", "select 1;");
    assert_eq!(p.run(async |o, log| reylo_migrate::history_pull(o, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"↓ 20261027100000_users\.sql  written: it wasn't here\nNot run on this database, and left as they are: 20261027110000_draft\.sql");
    assert_eq!(std::fs::read_to_string(p.dir.join("migrations").join("20261027100000_users.sql")).unwrap(), original);
    assert_eq!(p.run(async |o, log| reylo_migrate::history_pull(o, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"^✓ \S*migrations holds every migration this database ran, as it ran it\n");
}

async fn history_pull_takes_back_what_the_lock_refuses(mut p: P) {
    let int = p.t.int;
    p.linear();
    let original = format!("create table a (id {int});");
    p.schema(&original);
    p.file("20261027100000_a.sql", &original);
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.output();
    // A formatter's reflow reads as an edit to the lock, so pull writes it back.
    p.file("20261027100000_a.sql", &format!("create table a (\n  id {int}\n);"));
    assert_eq!(p.dry_run().await, 1);
    assert_has!(p.output(), r"changed after it was locked[\s\S]*reylo migrate history pull");
    assert_eq!(p.run(async |o, log| reylo_migrate::history_pull(o, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), "written over");
    assert_eq!(std::fs::read_to_string(p.dir.join("migrations").join("20261027100000_a.sql")).unwrap(), original);
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    p.output();
    // A comment isn't, and stays.
    let commented = format!("-- the table\n{original}");
    p.file("20261027100000_a.sql", &commented);
    assert_eq!(p.run(async |o, log| reylo_migrate::history_pull(o, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), "holds every migration");
    assert_eq!(std::fs::read_to_string(p.dir.join("migrations").join("20261027100000_a.sql")).unwrap(), commented);
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    p.output();
    // A database changed by hand isn't pulled from: the files its history gives wouldn't make it.
    p.exec(&format!("create table stray (id {int})")).await;
    std::fs::remove_file(p.dir.join(reylo_migrate::lock::LOCK)).unwrap();
    assert_eq!(p.run(async |o, log| reylo_migrate::history_pull(o, log).await).await, 1);
    assert_has!(p.output(), r"✗ This database was changed outside its migrations\n[\s\S]*stray[\s\S]*reylo migrate history push[\s\S]*Nothing was changed\.");
    assert!(!p.dir.join(reylo_migrate::lock::LOCK).exists(), "it wrote nothing");
}

async fn ddl_run_by_hand_then_written_is_recorded(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.schema(&format!("create table users (id {int});"));
    p.file("20261027100000_users.sql", &format!("create table users (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec(&format!("alter table users add column nick {text}")).await;
    p.file("20261027110000_nick.sql", &format!("alter table users add column nick {text};"));
    p.schema(&format!("create table users (id {int}, nick {text});"));
    // Never recorded by deploy, which refuses it; check says the migration makes the change, and how to
    // record it.
    assert_eq!(p.migrate().await, 1);
    p.output();
    assert_eq!(p.dry_run().await, 1);
    assert_has!(p.output(), r"20261027110000_nick\.sql makes exactly this change: record it here, running nothing:\n    reylo migrate history push");
    assert_eq!(p.history_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"\+ 20261027110000_nick\.sql +recorded: already in place[\s\S]*✓ Recorded 1 migration, running nothing");
    assert_eq!(p.history().await, ["20261027100000_users.sql", "20261027110000_nick.sql"]);
    // One that also changes data is recorded where the schema needs it, and says its data never ran
    // (MySQL keeps a DDL statement in a file of its own).
    p.exec(&format!("alter table users add column score {int}")).await;
    let score = if p.d == Dialect::Mysql { format!("alter table users add column score {int};") } else { format!("alter table users add column score {int}; update users set score = 0;") };
    p.file("20261027120000_score.sql", &score);
    p.schema(&format!("create table users (id {int}, nick {text}, score {int});"));
    assert_eq!(p.history_push().await, 0, "{}", p.peek());
    if p.d != Dialect::Mysql {
        assert_has!(p.output(), r"\+ 20261027120000_score\.sql +recorded: its data statements \([^)]*\) may not have run here");
    }
    // One that only changes data, at the end, can't be told to have run: it stays pending, to run.
    p.file("20261027130000_seed.sql", "insert into users (id) values (1);");
    // And one the database doesn't have stays pending too.
    p.file("20261027140000_bio.sql", &format!("alter table users add column bio {text};"));
    p.schema(&format!("create table users (id {int}, nick {text}, score {int}, bio {text});"));
    assert_eq!(p.history_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"2 pending, which reylo migrate deploy runs: 20261027130000_seed\.sql, 20261027140000_bio\.sql");
    assert_eq!(p.history().await.len(), 3);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select id from users").await.len(), 1);
}

async fn a_change_by_hand_with_schema_sql_is_told_to_write_the_migration(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.schema(&format!("create table users (id {int});"));
    p.file("20261027100000_users.sql", &format!("create table users (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec(&format!("alter table users add column nick {text}")).await;
    p.schema(&format!("create table users (id {int}, nick {text});"));
    assert_eq!(p.dry_run().await, 1);
    assert_has!(p.output(), r"This database was changed outside its migrations[\s\S]*reylo migrate new <name>");
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

async fn a_migration_the_lock_lacks_is_a_draft(mut p: P) {
    let int = p.t.int;
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027120000_c.sql", &format!("create table c (id {int});"));
    p.schema(&format!("create table a (id {int}); create table c (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.schema(&format!("create table a (id {int}); create table b (id {int}); create table c (id {int});"));
    p.file("20261027110000_b.sql", &format!("create table b (id {int});"));
    // Locked, the lock's order is the order, whatever the names: b is a draft until development
    // locks it after c.
    p.linear();
    p.lock(&["20261027100000_a.sql", "20261027120000_c.sql"]);
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"20261027110000_b\.sql isn't locked yet\n\n  Run it in development, which locks it:\n    reylo migrate dev");
}

async fn a_rebased_migration_is_rewound_by_dev(mut p: P) {
    let int = p.t.int;
    p.schema(&format!("create table a (id {int}); create table mine (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027110000_mine.sql", &format!("create table mine (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // Main merged a migration from another branch, newer than mine.
    p.file("20261027120000_theirs.sql", &format!("create table theirs (id {int});"));
    p.schema(&format!("create table a (id {int}); create table mine (id {int}); create table theirs (id {int});"));
    std::fs::rename(p.dir.join("migrations/20261027110000_mine.sql"), p.dir.join("migrations/20261027130000_mine.sql")).unwrap();
    let files = p.files();
    assert_eq!(files[1], "20261027120000_theirs.sql");
    assert!(has(&files[2], r"^\d{14}_mine\.sql$") && files[2].as_str() > "20261027120000", "{files:?}");
    p.output();
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), "which isn't in");
    p.exec("insert into a values (1)").await;
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_eq!(p.history().await, ["20261027100000_a.sql".to_string(), "20261027120000_theirs.sql".to_string(), files[2].clone()]);
    assert_eq!(p.rows("select * from a").await.len(), 1, "a rewind leaves what it doesn't undo");
}

async fn history_pull_writes_the_lock_and_keeps_what_is_still_to_deploy(mut p: P) {
    let int = p.t.int;
    let (a, b) = ("20261027100000_a.sql", "20261027110000_b.sql");
    p.linear();
    p.file(a, &format!("create table a (id {int});"));
    p.schema(&format!("create table a (id {int});"));
    p.lock(&[a]);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    // b is locked, on its way to this database, which hasn't run it.
    p.file(b, &format!("create table b (id {int});"));
    p.lock(&[a, b]);
    assert_eq!(p.run(async |o, log| reylo_migrate::history_pull(o, log).await).await, 0, "{}", p.peek());
    assert!(!p.peek().contains("Wrote"), "{}", p.peek());
    assert_eq!(p.locked(), [a, b]);
    p.output();
    // A lock lost turns linear history off: history pull keeps it off, and reylo migrate lock writes
    // what this database ran; b is left as it is, a draft again.
    std::fs::remove_file(p.dir.join(reylo_migrate::lock::LOCK)).unwrap();
    assert_eq!(p.run(async |o, log| reylo_migrate::history_pull(o, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Not run on this database, and left as they are: 20261027110000_b\.sql");
    assert!(!p.dir.join(reylo_migrate::lock::LOCK).exists());
    assert_eq!(p.run(async |o, log| reylo_migrate::lock_from_history(o, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"✓ Wrote \S*reylo-lock\.yaml: 1 migration");
    assert_eq!(p.locked(), [a]);
}

async fn db_introspect_prints_the_schema(p: P) {
    let int = p.t.int;
    assert_eq!(reylo_migrate::db_introspect(&p.opts(), &[]).await.unwrap(), "", "an empty database has no schema");
    p.exec(&format!("create table hand_made (id {int});")).await;
    let text = reylo_migrate::db_introspect(&p.opts(), &[]).await.unwrap();
    assert!(has(&text, "(?i)create table .*hand_made"), "{text}");
    assert!(!p.dir.join("schema.sql").exists(), "it writes no file");
}

async fn a_hotfix_by_hand_is_recorded_once_it_is_a_migration(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.schema(&format!("create table users (id {int}, email {text});"));
    p.file("20261027100000_users.sql", &format!("create table users (id {int}, email {text});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into users values (1, 'a@b.co')").await;
    p.exec("create index users_email on users (email)").await;
    p.output();
    assert_eq!(p.dry_run().await, 1, "drift until the hotfix is a migration");
    assert_has!(p.output(), "This database was changed outside its migrations");
    p.file("20261027110000_hotfix.sql", "create index users_email on users (email);");
    // Recorded once schema.sql has it too: everything agrees but the history it writes.
    assert_eq!(p.history_push().await, 1);
    assert_has!(p.output(), r"The migrations make what \S*schema\.sql doesn't declare[\s\S]*users_email[\s\S]*Nothing was changed\.");
    assert_eq!(p.history().await, ["20261027100000_users.sql"]);
    p.schema(&format!("create table users (id {int}, email {text}); create index users_email on users (email);"));
    assert_eq!(p.dry_run().await, 1, "recorded only when told");
    assert_has!(p.output(), r"20261027110000_hotfix\.sql makes exactly this change[^\n]*\n    reylo migrate history push");
    assert_eq!(p.history_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"\+ 20261027110000_hotfix\.sql +recorded");
    assert_eq!(p.history().await, ["20261027100000_users.sql", "20261027110000_hotfix.sql"]);
    assert_eq!(p.rows("select * from users").await.len(), 1);
}

async fn history_push_follows_a_renamed_migration(mut p: P) {
    let int = p.t.int;
    p.schema(&format!("create table a (id {int}); create table mine (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027110000_mine.sql", &format!("create table mine (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into a values (1)").await;
    std::fs::rename(p.dir.join("migrations/20261027110000_mine.sql"), p.dir.join("migrations/20261027130000_mine.sql")).unwrap();
    p.output();
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), "which isn't in");
    assert_eq!(p.history_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"\+ 20261027130000_mine\.sql +recorded: already in place");
    assert_has!(&out, r"- 20261027110000_mine\.sql +taken out of the history");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), "Up to date");
    assert_eq!(p.rows("select * from a").await.len(), 1);
    // A database whose schema no run of the migrations makes is refused, and nothing changes.
    p.exec(&format!("create table stray (id {int})")).await;
    assert_eq!(p.history_push().await, 1);
    assert_has!(p.output(), r"✗ No run of the migrations makes this database's schema\n  The closest is after 20261027130000_mine\.sql, which differs by:[\s\S]*stray[\s\S]*Nothing was changed\.");
    assert_eq!(p.history().await, ["20261027100000_a.sql", "20261027130000_mine.sql"]);
}

async fn every_applied_row_holds_the_fingerprint_it_left(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.schema(&format!("create table a (id {int}, note {text});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027110000_note.sql", &format!("alter table a add column note {text};"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // Written with its row, so that no run cut off between the two leaves a row with nothing to compare.
    let rows = p.rows("select name, fingerprint from __reylo_migrations order by name").await;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.get("fingerprint").is_some()), "{rows:?}");
    assert_ne!(rows[0].get("fingerprint"), rows[1].get("fingerprint"), "each holds the schema it left");
}

async fn a_run_cut_off_before_its_fingerprint_is_refused(mut p: P) {
    let int = p.t.int;
    p.schema(&format!("create table a (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // As the row reads where a run was cut off after the migration committed, before the
    // fingerprint was read.
    p.exec("update __reylo_migrations set fingerprint = 'pending'").await;
    p.output();
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"✗ A run was cut off after 20261027100000_a\.sql applied, before it stored this database's fingerprint[\s\S]*reylo migrate deploy --dry-run[\s\S]*reylo migrate history push[\s\S]*Nothing was changed\.");
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    assert_has!(p.output(), "until its fingerprint is stored again: reylo migrate history push");
    assert_eq!(p.history_push().await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), "Up to date");
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
    assert_has!(p.output(), "It has its own begin");
    p.file("20261027100000_ab.sql", &format!("create table a (id {int}); create table b (id {int});"));
    let code = p.migrate().await;
    if p.d == Dialect::Mysql {
        assert_eq!(code, 1);
        assert_has!(p.output(), "MySQL commits each on its own");
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
    if landed {
        // Its change is in place, so this database isn't as Reylo's last finished run left it: deploy
        // refuses, a dry run says what happened, and history push records it.
        assert_eq!(p.migrate().await, 1);
        assert_has!(p.output(), "was changed outside its migrations");
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        assert_has!(p.output(), r"20261027110000_index\.sql was cut off, and its change is in place: reylo migrate history push records it");
        assert_eq!(p.history_push().await, 0, "{}", p.peek());
        let out = p.output();
        assert_has!(&out, r"\+ 20261027110000_index\.sql +recorded");
        assert!(!out.contains("taken out"), "{out}");
    } else {
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), r"✓ 20261027110000_index\.sql +applied");
    }
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

async fn a_lone_statement_cut_off_with_no_fingerprint_to_compare(mut p: P) {
    if p.d == Dialect::Sqlite {
        return;
    }
    let int = p.t.int;
    let lone = if p.d == Dialect::Postgres { "create index concurrently a_id on a (id)" } else { "create index a_id on a (id)" };
    p.schema(&format!("create table a (id {int}); create index a_id on a (id);"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.file("20261027110000_index.sql", &format!("{lone};"));
    let mut db = Db::connect(&p.url, &p.dir).await.unwrap();
    let column = if p.d == Dialect::Mysql { "`sql`" } else { "sql" };
    let insert = format!("insert into __reylo_migrations (name, {column}, applied_at) values ({}, {}, null)", db.param(1), db.param(2));
    db.exec_with(&insert, &["20261027110000_index.sql", &format!("{lone};")]).await.unwrap();
    db.exec(lone).await.unwrap();
    db.exec("update __reylo_migrations set fingerprint = null").await.unwrap();
    db.close().await;
    p.output();
    // Run again, it fails where it landed, and says how to find out.
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"✗ 20261027110000_index\.sql  failed[^\n]*\n[\s\S]*A run before this one was cut off here, so its change may be in place: reylo migrate deploy --dry-run says, and reylo migrate history push records it");
    assert_eq!(p.dry_run().await, 1);
    assert_has!(p.output(), r"20261027110000_index\.sql makes exactly this change[^\n]*\n    reylo migrate history push");
    assert_eq!(p.history_push().await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), "Up to date");
}

async fn a_shadow_left_by_a_killed_run_is_swept(mut p: P) {
    if p.d == Dialect::Sqlite {
        return;
    }
    let int = p.t.int;
    if p.d == Dialect::Duckdb {
        // DuckDB's shadows are in memory: a run, held or killed, leaves no database to sweep, in
        // the target's file or beside it.
        p.schema(&format!("create table a (id {int});"));
        p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        let mut target = Db::connect(&p.url, &p.dir).await.unwrap();
        let mut server = Server::open(&mut target, &p.url, None).await.unwrap();
        let held = server.shadow().await.unwrap();
        let databases: Vec<String> = target.rows("select database_name as n from duckdb_databases() where not internal order by 1").await.unwrap().iter().map(|r| r.text("n").to_string()).collect();
        assert_eq!(databases, ["dev"], "a shadow is no database of the target's");
        server.drop_shadow(held).await;
        target.close().await;
        let mut files: Vec<String> = std::fs::read_dir(&p.dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|f| f.contains("duckdb")).collect();
        files.sort();
        assert_eq!(files, ["dev.duckdb"], "no file beside the target's");
        return;
    }
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
        out.push(catalog(&mut s.db).await.unwrap());
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
    assert_has!(out, "This database was changed outside its migrations");
    // DuckDB drops no column of a table with an index: the index goes around it.
    p.exec(if p.d == Dialect::Duckdb { "drop index users_email; alter table users drop column hotfix; create index users_email on users (email)" } else { "alter table users drop column hotfix" }).await;
    // A migration that does not add up, then one that fails.
    p.file("20261027130000_nick.sql", &format!("alter table users add column nick {text};"));
    let (code, out) = p.check_every_way().await;
    assert_eq!(code, 1);
    assert_has!(out, r"The migrations make what \S*schema\.sql doesn't declare[\s\S]*nick");
    p.file("20261027130000_nick.sql", &format!("alter table nobody add column nick {text};"));
    let (code, out) = p.check_every_way().await;
    assert_eq!(code, 1);
    assert_has!(out, "20261027130000_nick.sql fails");
    std::fs::remove_file(p.dir.join("migrations").join("20261027130000_nick.sql")).unwrap();
    // A new migration, which the replay runs in its place.
    p.file("20261027140000_posts.sql", &format!("create table posts (id {int});"));
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
    // A check builds shadows, which the cache keeps; deploy builds none.
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    // With its lines gone, the check starts each shadow from the database the last run kept.
    let _ = std::fs::remove_dir_all(p.dir.join(".cache").join("migrate").join("lines"));
    reylo_migrate::cache::take_stats();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    let stats = reylo_migrate::cache::take_stats();
    // MySQL has no copy of a database, and DuckDB's shadows are in memory: the cache keeps their lines.
    let keeps_databases = !matches!(p.d, Dialect::Mysql | Dialect::Duckdb);
    if keeps_databases {
        assert_eq!((stats.replayed, stats.copies), (0, 2), "the databases the last check kept answer: {stats:?}");
    } else {
        assert_eq!(stats.replayed, 4, "MySQL and DuckDB replay it all: {stats:?}");
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
    assert_has!(p.output(), r"\S*schema\.sql has what no migration makes yet[\s\S]*note");
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

async fn db_push_takes_a_development_database_to_schema_sql(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, name {text});"));
    p.schema(&format!("create table users (id {id}, name {text});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"✓ 20261027100000_users\.sql +applied");
    // schema.sql moves first: sync makes the change, unrecorded, and says what to write.
    p.schema(&format!("create table users (id {id}, name {text}, bio {text});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Pushing \S*schema\.sql to \S+\n  \+ alter table users add column bio [^\n]+;\n✓ This database matches \S*schema\.sql\n! Not in a migration yet: reylo migrate new <name> writes it");
    assert_eq!(p.history().await, ["20261027100000_users.sql"]);
    p.exec("insert into users (name, bio) values ('ada', 'hi')").await;
    // The migration written for it is +recorded, and the rows stay. sync alone refuses it
    // first: a database is brought to its migrations by apply --dev.
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "bio", &Default::default(), log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.db_push().await, 1);
    assert_has!(p.output(), r"✗ \d{14}_bio\.sql hasn't run on this database\n\n  Run it first:\n    reylo migrate dev");
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"\d{14}_bio\.sql +recorded");
    assert_eq!(p.rows("select bio from users").await[0].text("bio"), "hi");
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"This database matches \S*schema\.sql and its migrations");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), "Up to date");
    // Never in CI.
    assert_eq!(p.run(async |o, log| reylo_migrate::db_push(o, true, log).await).await, 1);
    assert_has!(p.output(), "CI is set, and reylo db push is for development databases");
}

async fn db_push_rewinds_what_no_longer_agrees(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, name {text});"));
    p.file("20261027110000_tags.sql", &format!("create table tags (id {int});"));
    p.schema(&format!("create table users (id {id}, name {text}); create table tags (id {int});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    p.exec("insert into users (name) values ('ada')").await;
    p.output();
    // An applied migration edited: rewound, then run as it reads now.
    p.file("20261027110000_tags.sql", &format!("create table tags (id {int}, label {text});"));
    p.schema(&format!("create table users (id {id}, name {text}); create table tags (id {int}, label {text});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"↺ 20261027110000_tags\.sql +rewound: it changed since it ran");
    assert_has!(&out, r"✓ 20261027110000_tags\.sql +applied");
    assert_eq!(p.rows("select name from users").await.len(), 1, "what agrees keeps its rows");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // Switched to a branch without it: rewound away.
    std::fs::remove_file(p.dir.join("migrations").join("20261027110000_tags.sql")).unwrap();
    p.schema(&format!("create table users (id {id}, name {text});"));
    p.output();
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"↺ 20261027110000_tags\.sql +rewound: it isn't in the migrations");
    assert!(p.try_rows("select 1 from tags").await.is_err());
    assert_eq!(p.history().await, ["20261027100000_users.sql"]);
    // A change made by hand, which schema.sql doesn't declare: refused, with the command that undoes it.
    p.exec(&format!("alter table users add column hotfix {int}")).await;
    p.output();
    assert_eq!(p.dev().await, 1, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"✗ This database was changed outside \S*schema\.sql and its migrations\n\n    alter table users add column hotfix");
    p.exec(&undo_in(&out)).await;
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert!(p.try_rows("select hotfix from users").await.is_err());
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
}

async fn db_push_keeps_what_no_migration_makes_yet(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    let users = format!("create table users (id {id}, name {text});");
    let notes = format!("create table notes (id {int}, body {text});");
    p.file("20261027100000_users.sql", &users);
    p.schema(&format!("{users} {notes}"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Pushing \S*schema\.sql to \S+");
    p.exec("insert into notes values (1, 'kept')").await;
    // Nothing changed: what the last sync made stays, with its rows.
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert!(!out.contains("Undid"), "{out}");
    assert_has!(&out, r"✓ This database matches \S*schema\.sql\n! Not in a migration yet:");
    assert_eq!(p.rows("select * from notes").await.len(), 1);
    // schema.sql moves again: only the new change is made.
    p.schema(&format!("{users} {notes} create table tags (id {int});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Pushing \S*schema\.sql to \S+\n  \+ create table tags");
    assert_eq!(p.rows("select * from notes").await.len(), 1);
    // A migration written for part of it is recorded; the rest stays unrecorded.
    p.file("20261027110000_notes.sql", &notes);
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"20261027110000_notes\.sql +recorded");
    assert_has!(&out, r"Not in a migration yet");
    assert_eq!(p.rows("select * from notes").await.len(), 1);
    // A migration that can't run on what sync made: that is undone first.
    p.file("20261027120000_tags.sql", &format!("create table tags (id {int}, label {text});"));
    p.schema(&format!("{users} {notes} create table tags (id {int}, label {text});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"↺ Undid what no migration made, so that 20261027120000_tags\.sql runs on what the migrations make\n    ! Dropped tags\n");
    assert_has!(&out, r"✓ 20261027120000_tags\.sql +applied[\s\S]*This database matches \S*schema\.sql and its migrations");
    assert_eq!(p.history().await, ["20261027100000_users.sql", "20261027110000_notes.sql", "20261027120000_tags.sql"]);
    assert_eq!(p.rows("select * from notes").await.len(), 1);
}

async fn apply_dev_locks_a_draft_after_what_is_locked(mut p: P) {
    let int = p.t.int;
    p.linear();
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027120000_c.sql", &format!("create table c (id {int});"));
    p.schema(&format!("create table a (id {int}); create table c (id {int});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"(?m)✓ 20261027120000_c\.sql +applied[^\n]*\n✓ Wrote \S*reylo-lock\.yaml$");
    p.exec("insert into c values (1)").await;
    // A draft named older than c runs after it, where the lock puts it: nothing is rewound.
    p.file("20261027110000_b.sql", &format!("create table b (id {int});"));
    p.schema(&format!("create table a (id {int}); create table b (id {int}); create table c (id {int});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert!(!out.contains("rewound"), "{out}");
    assert_eq!(p.history().await, ["20261027100000_a.sql", "20261027120000_c.sql", "20261027110000_b.sql"]);
    assert_eq!(p.locked(), ["20261027100000_a.sql", "20261027120000_c.sql", "20261027110000_b.sql"]);
    assert_eq!(p.rows("select * from c").await.len(), 1);
}

async fn a_locked_project_runs_its_lock(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.linear();
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, email {text});"));
    p.schema(&format!("create table users (id {id}, email {text});"));
    // A project with linear history starts with a lock, empty: apply runs nothing it doesn't list, and
    // development applies the migrations, and that locks them.
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"20261027100000_users\.sql isn't locked yet");
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"(?m)^✓ Wrote \S*reylo-lock\.yaml$");
    // The lock certifies schema.sql too, since the migrations reach it.
    assert_eq!(p.status().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"matches the migration files\n✓ The migrations match \S*schema\.sql");
    // Another database runs what the lock lists.
    let q = P::new(p.d).await.unwrap();
    let dev_url = std::mem::replace(&mut p.url, q.url.clone());
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(q.history().await, ["20261027100000_users.sql"]);
    p.url = dev_url;
    p.output();
    // An edit to a locked migration is seen by status, and refused by deploy.
    let locked_text = std::fs::read_to_string(p.dir.join("migrations/20261027100000_users.sql")).unwrap();
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, email {text}, nick {text});"));
    assert_eq!(p.status().await, 1);
    assert_has!(p.output(), r"20261027100000_users\.sql changed after it was locked");
    // Deploy refuses it once, as the migration this database ran, not again as the lock's.
    assert_eq!(p.migrate().await, 1);
    let out = p.output();
    assert_has!(&out, "changed after this database applied it");
    assert!(!out.contains("changed after it was locked"), "{out}");
    p.file("20261027100000_users.sql", &locked_text);
    // A draft beside the lock is refused by apply, and run by development.
    p.file("20261027110000_nick.sql", &format!("alter table users add column nick {text};"));
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"20261027110000_nick\.sql isn't locked yet");
    std::fs::remove_file(p.dir.join("migrations/20261027110000_nick.sql")).unwrap();
    // schema.sql that moves with no migration is seen by status too.
    p.schema(&format!("create table users (id {id}, email {text}, bio {int});"));
    assert_eq!(p.status().await, 1);
    assert_has!(p.output(), r"schema\.sql changed since the lock was written\n  The lock holds a checksum");
    p.schema(&format!("create table users (id {id}, email {text});"));
    // Deleting the lock turns linear history off, and reylo migrate lock turns it back on.
    std::fs::remove_file(p.dir.join(reylo_migrate::lock::LOCK)).unwrap();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.run(async |o, log| reylo_migrate::lock_from_history(o, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"✓ Wrote \S*reylo-lock\.yaml: 1 migration, in the order this database ran them");
    assert_eq!(p.locked(), ["20261027100000_users.sql"]);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
}

async fn a_lock_conflict_is_resolved_by_apply_dev(mut p: P) {
    let int = p.t.int;
    p.linear();
    p.file("20261027100000_base.sql", &format!("create table base (id {int});"));
    p.file("20261027120000_mine.sql", &format!("create table mine (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_eq!(p.locked(), ["20261027100000_base.sql", "20261027120000_mine.sql"]);
    p.exec("insert into base values (1)").await;
    // Main locked another migration after base; its lines are kept, and this branch's left out.
    p.file("20261027110000_theirs.sql", &format!("create table theirs (id {int});"));
    p.lock(&["20261027100000_base.sql", "20261027110000_theirs.sql"]);
    p.output();
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"↺ 20261027120000_mine\.sql +rewound[\s\S]*✓ 20261027110000_theirs\.sql +applied[\s\S]*✓ 20261027120000_mine\.sql +applied[^\n]*\n✓ Wrote \S*reylo-lock\.yaml");
    assert_eq!(p.locked(), ["20261027100000_base.sql", "20261027110000_theirs.sql", "20261027120000_mine.sql"]);
    assert_eq!(p.rows("select * from base").await.len(), 1);
    // Accepting both sides with this branch's line first puts main's line, which this database never
    // ran, out of its chain: refused, by dev and, with no database at all, by every deploy.
    let q = P::new(p.d).await.unwrap();
    let mut r = P { url: q.url.clone(), ..P::sibling(&p) };
    r.strict = true;
    r.file("20261027100000_base.sql", &format!("create table base (id {int});"));
    assert_eq!(r.dev().await, 0, "{}", r.peek());
    r.file("20261027110000_theirs.sql", &format!("create table theirs (id {int});"));
    r.file("20261027120000_mine.sql", &format!("create table mine (id {int});"));
    // Each line was locked right after base, on its own branch.
    let base = reylo_migrate::lock::chain([("20261027100000_base.sql", format!("create table base (id {int});").as_str())]);
    let after = |name: &str, sql: &str| reylo_migrate::lock::Entry { name: name.into(), sum: reylo_migrate::lock::checksum(&base[0].sum, name, sql) };
    let wrong = reylo_migrate::lock::Lock { schema: None, entries: vec![base[0].clone(), after("20261027120000_mine.sql", &format!("create table mine (id {int});")), after("20261027110000_theirs.sql", &format!("create table theirs (id {int});"))] };
    reylo_migrate::lock::write(&r.dir.join(reylo_migrate::lock::LOCK), &wrong).unwrap();
    r.output();
    assert_eq!(r.dev().await, 1);
    assert_has!(r.output(), r"was merged out of order at 20261027110000_theirs\.sql, which this database never ran\n  It came from another branch, and was locked right after 20261027100000_base\.sql");
    let main = P::new(p.d).await.unwrap();
    let mut m = P { url: main.url.clone(), ..P::sibling(&p) };
    m.strict = true;
    m.file("20261027100000_base.sql", &format!("create table base (id {int});"));
    m.file("20261027110000_theirs.sql", &format!("create table theirs (id {int});"));
    m.lock(&["20261027100000_base.sql", "20261027110000_theirs.sql"]);
    assert_eq!(m.migrate().await, 0, "{}", m.peek());
    m.output();
    m.file("20261027120000_mine.sql", &format!("create table mine (id {int});"));
    std::fs::copy(r.dir.join(reylo_migrate::lock::LOCK), m.dir.join(reylo_migrate::lock::LOCK)).unwrap();
    assert_eq!(m.migrate().await, 1);
    assert_has!(m.output(), r"was merged out of order at 20261027110000_theirs\.sql\n  It was locked right after 20261027100000_base\.sql");
}

async fn create_resolves_the_names_drop_gives(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, nick {text}, email {text}, legacy {int});"));
    p.file("20261027110000_posts.sql", &format!("create table posts (id {int}, nick {text});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into posts values (1, 'p')").await;
    p.schema(&format!("create table users (id {id}); create table posts (id {int}, handle {text});"));
    let made = |p: &P| p.files().into_iter().filter(|f| f.contains("_tidy")).collect::<Vec<_>>();
    // A column's bare name names no column, so it can't reach posts.nick, renamed to handle.
    let choices = reylo_migrate::Choices { drop: vec!["nick".into()], rename: vec![] };
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 1);
    let out = p.output();
    assert_has!(&out, r"--drop names nick, which this migration doesn't drop\n  A column is named with its table: users\.nick\.");
    // What the suggestion runs keeps what was stated and holds.
    let choices = reylo_migrate::Choices { drop: vec!["users.nick".into()], rename: vec![] };
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 1);
    assert_has!(p.output(), r"drops the columns users\.email and users\.legacy, with their data\n\n  To drop them:\n    reylo migrate new tidy --drop users\.nick,users\.email,users\.legacy\n");
    assert!(made(&p).is_empty());
    let choices = reylo_migrate::Choices { drop: vec!["users.nick".into(), "users.email".into(), "users.legacy".into()], rename: vec![] };
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select handle from posts").await[0].text("handle"), "p", "posts.nick was renamed, not dropped");
}

async fn create_lists_every_rename_it_assumes(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.file("20261027100000_t.sql", &format!("create table users (id {id}, bio {text});"));
    p.file("20261027110000_t.sql", &format!("create table posts (id {id}, title {text});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    p.schema(&format!("create table users (id {id}, about {text}); create table posts (id {id}, heading {text});"));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "names", &Default::default(), log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"  ! Written as renames, keeping their rows:\n      posts\.title → heading\n      users\.bio   → about\n    Where any aren't renames, delete the file, then:\n      reylo migrate new [a-z_]+ --drop posts\.title,users\.bio");
}

/// What a suggested `reylo migrate new` line states.
fn choices_of(line: &str) -> reylo_migrate::Choices {
    let mut choices = reylo_migrate::Choices::default();
    let words: Vec<&str> = line.split_whitespace().filter(|w| *w != "\\").collect();
    for pair in words.windows(2) {
        match pair[0] {
            "--drop" => choices.drop.extend(pair[1].split(',').map(String::from)),
            "--rename" => {
                let (old, new) = pair[1].split_once('=').unwrap();
                choices.rename.push((old.to_string(), new.to_string()));
            }
            _ => {}
        }
    }
    choices
}

async fn create_settles_every_guess_with_one_command(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    // DuckDB keeps no length of a varchar: the change is the column's nullability, as on SQLite.
    let not_null = format!("{text} not null default ''");
    let (short, long) = if matches!(p.d, Dialect::Sqlite | Dialect::Duckdb) { (text, not_null.as_str()) } else { ("varchar(20)", "varchar(40)") };
    p.file("20261027100000_t.sql", &format!("create table users (id {id}, nick {short});"));
    p.file("20261027110000_t.sql", &format!("create table orders (id {id}, a {text}, b {text}, c {text}, legacy {text});"));
    p.file("20261027120000_t.sql", &format!("create table posts (id {id}, old_a {text}, old_b {text});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into users (nick) values ('ada'); insert into orders (a, legacy) values ('x', 'y')").await;
    p.output();
    p.schema(&format!("create table users (id {id}, handle {long}); create table invoices (id {id}, a {text}, b {text}, c {text}, total {int}); create table posts (id {id});"));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &Default::default(), log).await).await, 1);
    let out = p.output();
    assert_has!(&out, r"✗ These 2 look like renames, but each changes more than its name\n\n    orders     → invoices  adds total, drops legacy\n    users\.nick → handle    [^\n]+\n\n  It also drops the columns invoices\.legacy, posts\.old_a and posts\.old_b, with their data\.\n\n  If they are renames, keeping their rows:\n    reylo migrate new tidy \\\n      --drop invoices\.legacy,posts\.old_a,posts\.old_b \\\n      --rename orders=invoices \\\n      --rename users\.nick=handle\n  If they are new, dropping the old ones and their data:\n    reylo migrate new tidy --drop orders,users\.nick,posts\.old_a,posts\.old_b\n");
    // A command, and the lines a backslash continues it on.
    let command = |out: &str, after: &str| out.lines().skip_while(|l| !l.contains(after)).skip(1).take_while(|l| l.starts_with("    ")).collect::<Vec<_>>().join("\n");
    let (keeps, drops) = (command(&out, "If they are renames"), command(&out, "If they are new"));
    let made = |p: &P| p.files().into_iter().filter(|f| f.contains("_tidy")).collect::<Vec<_>>();
    // The second command, as printed, is one the next create takes.
    let choices = choices_of(&drops);
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 0, "{}", p.peek());
    let sql: String = made(&p).iter().map(|f| std::fs::read_to_string(p.dir.join("migrations").join(f)).unwrap()).collect();
    assert!(sql.contains("drop table orders") && !sql.contains(" rename "), "{sql}");
    for f in made(&p) {
        std::fs::remove_file(p.dir.join("migrations").join(f)).unwrap();
    }
    p.output();
    // So is the first, and it keeps the rows.
    let choices = choices_of(&keeps);
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select handle from users").await[0].text("handle"), "ada");
    assert_eq!(p.rows("select a from invoices").await[0].text("a"), "x");
    p.output();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
}

/// A suggested command's flags, as a POSIX shell reads the command.
fn choices_of_shell(command: &str) -> reylo_migrate::Choices {
    let out = std::process::Command::new("sh").args(["-c", &command.replacen("reylo migrate new", "printf '%s\\n'", 1)]).output().unwrap();
    let words: Vec<String> = String::from_utf8(out.stdout).unwrap().lines().map(String::from).collect();
    let mut choices = reylo_migrate::Choices::default();
    for pair in words.windows(2) {
        match pair[0].as_str() {
            "--drop" => choices.drop.extend(pair[1].split(',').map(String::from)),
            "--rename" => {
                let (old, new) = pair[1].split_once('=').unwrap();
                choices.rename.push((old.to_string(), new.to_string()));
            }
            _ => {}
        }
    }
    choices
}

async fn commands_create_prints_for_names_with_spaces_run_as_printed(mut p: P) {
    if cfg!(windows) {
        return;
    }
    let (id, text) = (p.t.id, p.t.text);
    let d = p.d;
    let q = |name: &str| if d == Dialect::Mysql { format!("`{name}`") } else { format!("\"{name}\"") };
    let (short, long) = if matches!(p.d, Dialect::Sqlite | Dialect::Duckdb) { (text.to_string(), format!("{text} not null default ''")) } else { ("varchar(20)".to_string(), "varchar(40)".to_string()) };
    let before = format!("create table {} (id {id}, {} {short});", q("My Users"), q("Full Name"));
    p.file("20261027100000_t.sql", &before);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    p.schema(&format!("create table {} (id {id}, {} {long});", q("My Users"), q("Display Name")));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "names", &Default::default(), log).await).await, 1);
    let out = p.output();
    let command = |out: &str, after: &str| out.lines().skip_while(|l| !l.contains(after)).skip(1).take_while(|l| l.starts_with("    ")).collect::<Vec<_>>().join("\n");
    let (keeps, drops) = (command(&out, "If it is a rename"), command(&out, "If it is new"));
    let choices = choices_of_shell(&drops);
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "names", &choices, log).await).await, 0, "{drops}\n{}", p.peek());
    for f in p.files().into_iter().filter(|f| f.contains("_names")) {
        std::fs::remove_file(p.dir.join("migrations").join(f)).unwrap();
    }
    p.output();
    let choices = choices_of_shell(&keeps);
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "names", &choices, log).await).await, 0, "{keeps}\n{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
}

async fn deploy_compares_with_what_dev_replayed(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.linear();
    p.schema(&format!("create table a (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.output();
    // As dev left it: nothing to note, and nothing to refuse.
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"^✓ Up to date$");
    // What sync adds is no migration's, so deploy sees that the database changed, as after a deploy.
    p.schema(&format!("create table a (id {int}, note {text});"));
    assert_eq!(p.db_push().await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 1);
    let out = p.output();
    assert_has!(&out, "was changed outside its migrations");
    assert_has!(&out, r"schema\.sql changed since the lock was written");
    // dev keeps what sync made, says the migrations don't make it, and keeps the lock's last checksum
    // of schema.sql, so status still refuses; deploy still sees it isn't the migrations'.
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"! \S*schema\.sql has what no migration makes yet: reylo migrate new <name> writes it");
    assert_eq!(p.status().await, 1);
    assert_has!(p.output(), r"schema\.sql changed since the lock was written");
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), "was changed outside its migrations");
    // Written as a migration, it is the migrations'.
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "note", &Default::default(), log).await).await, 0, "{}", p.peek());
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"^✓ Up to date$");
}

async fn a_broken_draft_keeps_what_sync_made(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.linear();
    p.schema(&format!("create table a (id {int});"));
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.schema(&format!("create table a (id {int}); create table notes (body {text});"));
    assert_eq!(p.db_push().await, 0, "{}", p.peek());
    p.exec("insert into notes values ('kept')").await;
    p.output();
    p.file("20261027110000_b.sql", "insert into nope values (1);");
    assert_eq!(p.dev().await, 1);
    let out = p.output();
    assert_has!(&out, r"20261027110000_b\.sql fails when replayed in a shadow database");
    assert!(!out.contains("Undid"), "{out}");
    assert_eq!(p.rows("select * from notes").await.len(), 1);
}

async fn a_migrations_directory_that_isnt_there_is_refused(mut p: P) {
    let int = p.t.int;
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.exec("insert into a values (1)").await;
    p.output();
    let elsewhere = |o: &Options| {
        let mut o = o.clone();
        o.dir = o.root.join("reylo-typo").join("migrations");
        o.lock = o.root.join("reylo-typo").join(reylo_migrate::lock::LOCK);
        o
    };
    assert_eq!(p.run(async |o, log| reylo_migrate::dev(&elsewhere(o), false, log).await).await, 1);
    assert_has!(p.output(), r"reylo-typo/migrations doesn't exist\n  This database ran 1 migration from it");
    assert_eq!(p.run(async |o, log| reylo_migrate::history_push(&elsewhere(o), log).await).await, 1);
    assert_has!(p.output(), "doesn't exist");
    assert_eq!(p.rows("select * from a").await.len(), 1);
    assert_eq!(p.history().await, ["20261027100000_a.sql"]);
}

async fn a_run_cut_off_between_migrations_is_still_compared(mut p: P) {
    let int = p.t.int;
    p.file("20261027100000_a.sql", &format!("create table a (id {int});"));
    p.file("20261027110000_b.sql", &format!("create table b (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    // Each row holds the fingerprint its migration left; a change after the last is seen.
    let rows = p.rows("select name from __reylo_migrations where fingerprint is not null").await;
    assert_eq!(rows.len(), 2);
    p.exec(&format!("create table stray (id {int})")).await;
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), "was changed outside its migrations");
}

async fn init_on_an_empty_database_writes_an_empty_schema_and_lock(mut p: P) {
    assert_eq!(p.run(async |o, log| reylo_migrate::init(o, &[], log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"✓ Started \S+ for an empty database\n  \S+\n  ├── schema\.sql +empty: declare your tables here");
    assert!(p.files().is_empty(), "{:?}", p.files());
    assert_eq!(p.history().await, Vec::<String>::new());
    // Linear from the first migration: the lock is there, empty, and status passes.
    p.strict = true;
    assert!(p.locked().is_empty());
    assert_eq!(p.status().await, 0, "{}", p.peek());
    // The first migration is create's.
    p.schema(&format!("create table t (id {});", p.t.int));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "t", &Default::default(), log).await).await, 0, "{}", p.peek());
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_eq!(p.history().await.len(), 1);
}

async fn deploy_refuses_any_schema_with_no_history(mut p: P) {
    p.exec("create view v as select 1 as x").await;
    p.file("20261027100000_t.sql", &format!("create table t (id {});", p.t.int));
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), "This database has a schema, and no migration history");
}

async fn create_renames_a_table_keeping_its_rows(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, email {text});"));
    p.file("20261027110000_posts.sql", &format!("create table posts (id {int}, user_id {int}, foreign key (user_id) references users (id));"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // DuckDB's keys aren't numbered from 1 (it has no identity column), so the row names its own.
    p.exec(if p.d == Dialect::Duckdb { "insert into users (id, email) values (1, 'a'); insert into posts values (1, 1)" } else { "insert into users (email) values ('a'); insert into posts values (1, 1)" }).await;
    let sql_of = |p: &P, name: &str| p.files().into_iter().filter(|f| f.ends_with(&format!("_{name}.sql"))).map(|f| std::fs::read_to_string(p.dir.join("migrations").join(f)).unwrap()).collect::<String>();
    // One table goes while one of its columns comes: renamed, and the foreign key on it follows.
    p.schema(&format!("create table accounts (id {id}, email {text}); create table posts (id {int}, user_id {int}, foreign key (user_id) references accounts (id));"));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "accounts", &Default::default(), log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"✓ Wrote [^\n]+\n  ! users → accounts is written as a rename, keeping its rows\n    If they're different tables, delete the file, then:");
    let sql = sql_of(&p, "accounts");
    // DuckDB's foreign keys would name the old table after a rename, so a table another references
    // is made again under its new name, its rows copied across.
    if p.d == Dialect::Duckdb {
        assert!(sql.contains("create table accounts") && sql.contains("insert into accounts (id, email) select id, email from temp.main.reylo_copy_users"), "{sql}");
    } else {
        assert!(sql.contains("rename to accounts") && !sql.contains("drop table"), "{sql}");
    }
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select email from accounts").await[0].text("email"), "a");
    p.output();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    p.output();
    // A table renamed with its columns changed is a drop, unless --rename states it.
    p.schema(&format!("create table members (id {id}, address {text}, joined {int}); create table posts (id {int}, user_id {int}, foreign key (user_id) references members (id));"));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "members", &Default::default(), log).await).await, 1);
    assert_has!(p.output(), r"drops the table accounts and its rows\n\n  To drop it:\n    reylo migrate new members --drop accounts\n  If it was renamed instead, as members:\n    reylo migrate new members --rename accounts=<new name>");
    let choices = reylo_migrate::Choices { drop: vec![], rename: vec![("accounts".into(), "members".into()), ("accounts.email".into(), "address".into())] };
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "members", &choices, log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select address from members").await[0].text("address"), "a");
    p.exec("insert into members (address) values ('b')").await;
    assert_eq!(p.rows("select distinct id from members").await.len(), 2);
    p.output();
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
}

async fn db_push_renames_a_table_keeping_its_rows(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, email {text});"));
    p.schema(&format!("create table users (id {id}, email {text});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    p.exec("insert into users (email) values ('a')").await;
    p.output();
    p.schema(&format!("create table accounts (id {id}, email {text});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Pushing \S*schema\.sql to \S+\n    ! users is renamed accounts, keeping its rows[^\n]*\n  \+ alter table \S*users\S* rename to \S*accounts\S*;");
    assert_eq!(p.rows("select email from accounts").await[0].text("email"), "a");
    // The migration written for it is recorded, and the row stays.
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "accounts", &Default::default(), log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"\d{14}_accounts\.sql +recorded[\s\S]*This database matches \S*schema\.sql and its migrations");
    assert_eq!(p.rows("select email from accounts").await[0].text("email"), "a");
}

async fn db_push_renames_a_table_back_when_it_rewinds(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, email {text});"));
    p.schema(&format!("create table users (id {id}, email {text});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    p.exec("insert into users (email) values ('a')").await;
    // A branch renames the table; then main again, without it.
    p.file("20261027110000_accounts.sql", "alter table users rename to accounts;");
    p.schema(&format!("create table accounts (id {id}, email {text});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    std::fs::remove_file(p.dir.join("migrations").join("20261027110000_accounts.sql")).unwrap();
    p.schema(&format!("create table users (id {id}, email {text});"));
    p.output();
    // On Postgres the push renamed the primary key after the table, which nothing on main explains.
    if p.dev().await == 1 {
        let out = p.output();
        assert_has!(&out, r"✗ This database was changed outside \S*schema\.sql and its migrations");
        p.exec(&undo_in(&out)).await;
    }
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"↺ 20261027110000_accounts\.sql +rewound: it isn't in the migrations");
    assert!(!out.contains("drops"), "{out}");
    assert_eq!(p.rows("select email from users").await[0].text("email"), "a");
}

async fn db_push_keeps_what_it_records_when_it_undoes_the_rest(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id});"));
    p.schema(&format!("create table users (id {id}); create table notes (id {int}, body {text}); create table tags (id {int});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    p.exec("insert into notes values (1, 'kept')").await;
    // notes is written as made; tags as something else, which can't run on what sync made.
    p.file("20261027110000_notes.sql", &format!("create table notes (id {int}, body {text});"));
    p.file("20261027120000_tags.sql", &format!("create table tags (id {int}, label {text});"));
    p.schema(&format!("create table users (id {id}); create table notes (id {int}, body {text}); create table tags (id {int}, label {text});"));
    p.output();
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"↺ Undid what no migration made, so that 20261027120000_tags\.sql runs on what the migrations make\n    ! Dropped tags\n");
    assert_has!(&out, r"20261027110000_notes\.sql +recorded[\s\S]*✓ 20261027120000_tags\.sql +applied");
    assert_eq!(p.rows("select body from notes").await[0].text("body"), "kept");
    assert_eq!(p.history().await, ["20261027100000_users.sql", "20261027110000_notes.sql", "20261027120000_tags.sql"]);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
}

async fn db_push_runs_or_records_a_migration_cut_off(mut p: P) {
    let (id, int) = (p.t.id, p.t.int);
    p.file("20261027100000_users.sql", &format!("create table users (id {id});"));
    p.schema(&format!("create table users (id {id}); create table a (id {int}); create table b (id {int});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    // Two runs cut off with their rows marked started: one whose change landed, one whose didn't.
    p.exec("drop table a; drop table b").await;
    let (a, b) = (format!("create table a (id {int});"), format!("create table b (id {int});"));
    p.file("20261027110000_a.sql", &a);
    p.file("20261027120000_b.sql", &b);
    p.exec(&a).await;
    let column = if p.d == Dialect::Mysql { "`sql`" } else { "sql" };
    for (name, sql) in [("20261027110000_a.sql", &a), ("20261027120000_b.sql", &b)] {
        p.exec(&format!("insert into __reylo_migrations (name, {column}, applied_at) values ('{name}', '{sql}', null)")).await;
    }
    p.output();
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"20261027110000_a\.sql +recorded: already in place[\s\S]*✓ 20261027120000_b\.sql +applied");
    assert_eq!(p.history().await, ["20261027100000_users.sql", "20261027110000_a.sql", "20261027120000_b.sql"]);
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"This database matches \S*schema\.sql and its migrations");
}

async fn db_push_rewinds_only_what_the_rewound_migrations_made(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id});"));
    p.file("20261027110000_tags.sql", &format!("create table tags (id {int});"));
    p.schema(&format!("create table users (id {id}); create table tags (id {int}); create table notes (id {int}, body {text});"));
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    p.exec("insert into notes values (1, 'kept')").await;
    // Back on a branch without tags: it goes, and notes, which schema.sql still declares, stays.
    std::fs::remove_file(p.dir.join("migrations").join("20261027110000_tags.sql")).unwrap();
    p.schema(&format!("create table users (id {id}); create table notes (id {int}, body {text});"));
    p.output();
    assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert_has!(&out, r"↺ 20261027110000_tags\.sql +rewound: it isn't in the migrations\n    ! Dropped tags\n");
    assert!(!out.contains("notes"), "{out}");
    assert_eq!(p.rows("select body from notes").await[0].text("body"), "kept");
    assert!(p.try_rows("select 1 from tags").await.is_err());
}

async fn create_guesses_no_rename_a_table_that_stays_could_explain(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, email {text});"));
    p.file("20261027110000_accounts.sql", &format!("create table accounts (id {id}, email {text});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // accounts became members and users became accounts, or users became members: no telling.
    p.schema(&format!("create table accounts (id {id}, email {text}); create table members (id {id}, email {text});"));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "members", &Default::default(), log).await).await, 1);
    assert_has!(p.output(), r"drops the table users and its rows[\s\S]*reylo migrate new members --rename users=<new name>");
    // Stated, it is done as stated, and names ignore case.
    let choices = reylo_migrate::Choices { drop: vec![], rename: vec![("USERS".into(), "Members".into())] };
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "members", &choices, log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
}

async fn deploy_builds_no_shadow_database_and_trusts_the_fingerprint(mut p: P) {
    let (id, int) = (p.t.id, p.t.int);
    reylo_migrate::cache::set_enabled(Some(true));
    reylo_migrate::cache::take_stats();
    // A first run, a later one, and one with nothing to run replay nothing at all.
    p.file("20261027100000_users.sql", &format!("create table users (id {id});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Applying to [^\n]+\n  ✓ 20261027100000_users\.sql  applied · \d+ ms");
    p.file("20261027110000_tags.sql", &format!("create table tags (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"✓ 20261027110000_tags\.sql  applied");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"^✓ Up to date$");
    let stats = reylo_migrate::cache::take_stats();
    reylo_migrate::cache::set_enabled(None);
    assert_eq!((stats.replayed, stats.copies), (0, 0), "{stats:?}");
    // A change by hand no longer has the fingerprint: refused, and check says how it differs.
    p.exec(&format!("create table stray (id {int})")).await;
    assert_eq!(p.migrate().await, 1);
    let out = p.output();
    assert_has!(&out, r"✗ This database was changed outside its migrations\n\n  \+ column stray\.id [^\n]+\n  \+ table stray[^\n]*\n\n  If a migration makes this change, record it here, running nothing:\n    reylo migrate history push\n  If none does, write one and run it in development, then record it here:\n    reylo migrate new <name>\n    reylo migrate dev\n    reylo migrate history push\n  To see which migrations it matches:\n    reylo migrate deploy --dry-run\n\nNothing was changed\.");
    p.schema(&format!("create table users (id {id}); create table tags (id {int});"));
    assert_eq!(p.dry_run().await, 1);
    assert_has!(p.output(), "This database was changed outside its migrations");
    // Undone, it has the fingerprint again.
    p.exec("drop table stray").await;
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), "Up to date");
}

async fn deploy_goes_on_where_it_has_no_fingerprint_to_compare(mut p: P) {
    let int = p.t.int;
    p.file("20261027100000_users.sql", &format!("create table users (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    // A new version of Reylo or the server: read another way, a fingerprint says nothing.
    p.exec("update __reylo_migrations set fingerprint = 'old:1:abc'").await;
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"can't be compared with what its last migration left[\s\S]*Up to date");
    // It is stored again, so a change after that is seen.
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    let out = p.output();
    assert!(!out.contains("no fingerprint"), "{out}");
    p.exec(&format!("create table stray (id {int})")).await;
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), "was changed outside its migrations");
    p.exec("drop table stray").await;
    // A pending migration runs on one with none, and stores it.
    p.exec("update __reylo_migrations set fingerprint = null").await;
    p.file("20261027110000_tags.sql", &format!("create table tags (id {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"can't be compared with what its last migration left[\s\S]*applied");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"^✓ Up to date$");
}

async fn deploy_refuses_tables_with_no_history(mut p: P) {
    let int = p.t.int;
    p.exec(&format!("create table legacy (id {int})")).await;
    p.file("20261027100000_users.sql", &format!("create table users (id {int});"));
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"✗ This database has a schema, and no migration history\n\n  Where the migrations describe it, record them, running nothing:\n    reylo migrate history push\n  Or start the project's migrations from it:\n    reylo migrate init");
    assert_eq!(p.try_rows("select name from __reylo_migrations").await.map(|r| r.len()).unwrap_or(0), 0);
}

async fn create_names_a_type_change_that_loses_values_in_drop(mut p: P) {
    let id = p.t.id;
    // Each database's change that keeps the column but loses what its values hold.
    let (before, after, why) = match p.d {
        Dialect::Postgres => ("timestamptz", "date", r"users\.stamp changes from timestamp with time zone to date, which keeps only the date"),
        Dialect::Mysql => ("datetime(6)", "datetime", r"users\.stamp changes from datetime\(6\) to datetime, which rounds to whole seconds"),
        Dialect::Sqlite => ("text", "integer", r"users\.stamp changes from text to integer, which stores text that reads as a number as that number"),
        Dialect::Duckdb => ("timestamptz", "date", r"users\.stamp changes from timestamp with time zone to date, which keeps only the date"),
    };
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, stamp {before});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.schema(&format!("create table users (id {id}, stamp {after});"));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "narrow", &Default::default(), log).await).await, 1);
    let out = p.output();
    assert_has!(&out, &format!(r"✗ {why}[^\n]*\n\n  To accept that:\n    reylo migrate new narrow --drop users\.stamp"));
    assert!(p.files().iter().all(|f| !f.contains("_narrow")), "nothing is written");
    let choices = reylo_migrate::Choices { drop: vec!["users.stamp".into()], rename: vec![] };
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "narrow", &choices, log).await).await, 0, "{}", p.peek());
    let sql = p.files().into_iter().filter(|f| f.contains("_narrow")).map(|f| std::fs::read_to_string(p.dir.join("migrations").join(f)).unwrap()).collect::<String>();
    assert!(sql.contains("-- loses data: users.stamp changes from"), "{sql}");
    // A widening change needs nothing.
    let wider = match p.d {
        Dialect::Postgres => "timestamptz",
        Dialect::Mysql => "datetime(6)",
        Dialect::Sqlite => "text",
        Dialect::Duckdb => "timestamptz",
    };
    for f in p.files().into_iter().filter(|f| f.contains("_narrow")) {
        std::fs::remove_file(p.dir.join("migrations").join(f)).unwrap();
    }
    p.file("20261027110000_wider.sql", &format!("{};", match p.d {
        Dialect::Postgres => "alter table users alter column stamp type timestamp(3) with time zone",
        Dialect::Mysql => "alter table users modify stamp datetime(3)",
        Dialect::Sqlite => "alter table users add column note text",
        Dialect::Duckdb => "alter table users alter column stamp type timestamp",
    }));
    p.schema(&format!("create table users (id {id}, stamp {wider}{});", if p.d == Dialect::Sqlite { ", note text" } else { "" }));
    p.output();
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "widen", &Default::default(), log).await).await, 0, "{}", p.peek());
}

async fn create_assumes_a_rename_where_one_goes_and_one_comes(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    let not_null = format!("{text} not null default ''");
    let (short, long) = if matches!(p.d, Dialect::Sqlite | Dialect::Duckdb) { (text, not_null.as_str()) } else { ("varchar(20)", "varchar(40)") };
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, nick {short});"));
    p.file("20261027110000_accounts.sql", &format!("create table accounts (id {int}, email {short}, name {short});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into users (nick) values ('ada'); insert into accounts values (1, 'a@b.co', 'Ada')").await;
    // One column goes and one of its family comes: renamed, its type changed. One table goes and
    // one comes with most of its columns: renamed, and given the new one.
    p.schema(&format!("create table users (id {id}, handle {long}); create table members (id {int}, email {short}, name {short}, joined {int});"));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "renames", &Default::default(), log).await).await, 1);
    let out = p.output();
    assert_has!(&out, r"✗ These 2 look like renames, but each changes more than its name\n\n    accounts   → members  adds joined\n    users\.nick → handle   [^\n]+ → [^\n]+\n\n  If they are renames, keeping their rows:\n    reylo migrate new renames \\\n      --rename accounts=members \\\n      --rename users\.nick=handle\n  If they are new, dropping the old ones and their data:\n    reylo migrate new renames --drop accounts,users\.nick\n\nNothing was changed\.");
    assert_eq!(p.files().len(), 2, "nothing is written");
    // The second command, which takes them for new ones.
    let choices = reylo_migrate::Choices { drop: vec!["accounts".into(), "users.nick".into()], rename: vec![] };
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "renames", &choices, log).await).await, 0, "{}", p.peek());
    let made: Vec<String> = p.files().into_iter().filter(|f| f.contains("_renames")).collect();
    let sql: String = made.iter().map(|f| std::fs::read_to_string(p.dir.join("migrations").join(f)).unwrap()).collect();
    assert!(sql.contains("drop table accounts") && !sql.contains(" rename "), "{sql}");
    for f in made {
        std::fs::remove_file(p.dir.join("migrations").join(f)).unwrap();
    }
    p.output();
    // The first, which takes them for renames.
    let choices = reylo_migrate::Choices { drop: vec![], rename: vec![("accounts".into(), "members".into()), ("users.nick".into(), "handle".into())] };
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "renames", &choices, log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select handle from users").await[0].text("handle"), "ada");
    assert_eq!(p.rows("select name from members").await[0].text("name"), "Ada");
    assert_eq!(p.dry_run().await, 0, "{}", p.peek());
}

async fn db_push_watch_syncs_again_when_schema_sql_changes(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.file("20261027100000_users.sql", &format!("create table users (id {id});"));
    p.schema(&format!("create table users (id {id});"));
    assert_eq!(p.run(async |o, log| reylo_migrate::dev(o, false, log).await).await, 0, "{}", p.peek());
    p.output();
    // A save to schema.sql a moment after the first run starts the second.
    let schema = p.dir.join("schema.sql");
    let later = format!("create table users (id {id}, bio {text});");
    let saver = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1200));
        std::fs::write(schema, later).unwrap();
    });
    assert_eq!(p.run(async |o, log| reylo_migrate::db_push_watch(o, false, Some(2), log).await).await, 0, "{}", p.peek());
    saver.join().unwrap();
    let out = p.output();
    assert_has!(&out, r"Watching \S*schema\.sql and \S*migrations: each save pushes again");
    assert_has!(&out, r"This database matches \S*schema\.sql and its migrations[\s\S]*\d\d:\d\d:\d\d[\s\S]*Pushing \S*schema\.sql to \S+\n  \+ alter table users add column bio");
    assert!(p.try_rows("select bio from users").await.is_ok());
    assert_eq!(p.run(async |o, log| reylo_migrate::db_push_watch(o, true, Some(1), log).await).await, 1);
    assert_has!(p.output(), "CI is set, and reylo db push --watch is for development databases");
}

async fn a_lock_whose_order_is_not_the_names_survives_one_run(mut p: P) {
    let int = p.t.int;
    p.linear();
    p.file("20261027100000_base.sql", &format!("create table base (id {int});"));
    p.file("20261027110000_mine.sql", &format!("create table mine (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    // Main locked an migration named after mine; mine runs after it, in one run with it.
    p.file("20261027120000_theirs.sql", &format!("create table theirs (id {int});"));
    p.lock(&["20261027100000_base.sql", "20261027120000_theirs.sql"]);
    for _ in 0..3 {
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.locked(), ["20261027100000_base.sql", "20261027120000_theirs.sql", "20261027110000_mine.sql"]);
        assert_eq!(p.history().await, ["20261027100000_base.sql", "20261027120000_theirs.sql", "20261027110000_mine.sql"]);
        p.output();
    }
    // Another database given that lock runs it in its order, and is up to date after.
    let q = P::new(p.d).await.unwrap();
    let dev = std::mem::replace(&mut p.url, q.url.clone());
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), "Up to date");
    p.url = dev;
}

async fn a_fixed_migration_no_database_ran_is_locked_again(mut p: P) {
    let (int, text) = (p.t.int, p.t.text);
    p.linear();
    p.schema(&format!("create table base (id {int}); create table theirs (id {int});"));
    p.file("20261027100000_base.sql", &format!("create table base (id {int});"));
    p.file("20261027120000_theirs.sql", &format!("create table theirs (id {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    // Staging runs it as it was.
    let dev_url = p.url.clone();
    let staging = P::new(p.d).await.unwrap();
    p.url = staging.url.clone();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.url = dev_url;
    p.output();
    // Fixed afterwards, as one that failed on production, which recorded nothing of it: dev locks it
    // again as it now reads, a database that never ran it runs the fix, and one that ran it as it
    // was refuses the edit.
    p.file("20261027120000_theirs.sql", &format!("create table theirs (id {int}, note {text});"));
    p.schema(&format!("create table base (id {int}); create table theirs (id {int}, note {text});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    assert_has!(p.output(), r"✓ Wrote \S*reylo-lock\.yaml\n  ! 20261027120000_theirs\.sql is locked again as it now reads");
    assert_eq!(p.status().await, 0, "{}", p.peek());
    let production = P::new(p.d).await.unwrap();
    p.url = production.url.clone();
    p.output();
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select note from theirs").await.len(), 0, "the fix ran");
    p.url = staging.url.clone();
    assert_eq!(p.migrate().await, 1);
    assert_has!(p.output(), r"20261027120000_theirs\.sql changed after this database applied it");
}

async fn init_locks_the_baseline_in_a_locked_project(mut p: P) {
    let (id, text) = (p.t.id, p.t.text);
    p.linear();
    p.exec(&format!("create table users (id {id}, email {text} not null)")).await;
    assert_eq!(p.run(async |o, log| reylo_migrate::init(o, &[], log).await).await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.locked(), ["init.sql"]);
    assert_eq!(p.history().await, ["init.sql"]);
    assert_eq!(p.status().await, 0, "{}", p.peek());
    assert_has!(p.output(), "The migrations match");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
}

async fn history_push_keeps_a_data_migration_that_ran(mut p: P) {
    let int = p.t.int;
    p.schema(&format!("create table t (x {int});"));
    p.file("20261027100000_t.sql", &format!("create table t (x {int});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into t values (1)").await;
    p.file("20261027110000_backfill.sql", "update t set x = x * 10;");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.output();
    assert_eq!(p.history_push().await, 0, "{}", p.peek());
    let out = p.output();
    assert!(!out.contains("taken out") && !out.contains("pending"), "{out}");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_eq!(p.rows("select x from t").await[0].text("x"), "10");
}

async fn create_guesses_no_rename_of_unrelated_tables(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    p.file("20261027100000_sessions.sql", &format!("create table sessions (id {id}, user_id {int}, created_at {text});"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // Sharing only the columns most tables have, two tables are two.
    p.schema(&format!("create table audit_events (id {id}, user_id {int}, created_at {text}, kind {text});"));
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "audit", &Default::default(), log).await).await, 1);
    assert_has!(p.output(), r"drops the table sessions and its rows");
}

async fn the_drift_advice_in_a_locked_project_locks_the_migration_first(mut p: P) {
    let (id, int) = (p.t.id, p.t.int);
    p.linear();
    p.file("20261027100000_users.sql", &format!("create table users (id {id}, n {int});"));
    assert_eq!(p.dev().await, 0, "{}", p.peek());
    p.exec("create index users_n on users (n)").await;
    p.output();
    p.schema(&format!("create table users (id {id}, n {int});"));
    assert_eq!(p.dry_run().await, 1);
    assert_has!(p.output(), r"To keep it, add it to \S*schema\.sql and write its migration in development:\n    reylo migrate new <name>\n    reylo migrate dev\n  Then record it here, running nothing:\n    reylo migrate history push");
}

async fn wipe_deletes_rows_and_keeps_the_schema(mut p: P) {
    let (id, int, text) = (p.t.id, p.t.int, p.t.text);
    // MySQL commits each DDL statement on its own, so each is a file of its own.
    // DuckDB numbers rows through a sequence, which a wipe starts again.
    let users = if p.d == Dialect::Duckdb { format!("create sequence user_ids; create table users (id bigint primary key default nextval('user_ids'), name {text});") } else { format!("create table users (id {id}, name {text});") };
    p.file("20261027100000_users.sql", &users);
    p.file("20261027110000_posts.sql", &format!("create table posts (id {int}, user_id {int} references users (id));"));
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into users (name) values ('a'), ('b'); insert into posts values (1, 1)").await;
    p.output();
    assert_eq!(p.run(async |o, log| reylo_migrate::db_wipe(o, false, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Deleted every row of 2 tables on \S+[^;]*; the schema and the history stay");
    assert!(p.rows("select * from users").await.is_empty() && p.rows("select * from posts").await.is_empty());
    assert_eq!(p.history().await, ["20261027100000_users.sql", "20261027110000_posts.sql"]);
    // Identities start again.
    p.exec("insert into users (name) values ('c')").await;
    assert_eq!(p.rows("select id from users").await[0].text("id"), "1");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    assert_has!(p.output(), "Up to date");
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
    assert_has!(&out, r"This migration drops the columns users\.legacy and users\.middle_name, with their data\n\n  To drop them:\n    reylo migrate new tidy --drop users\.legacy,users\.middle_name\n  If any were renamed instead, as users\.nickname:\n    reylo migrate new tidy \\\n      --rename users\.legacy=<new name> \\\n      --rename users\.middle_name=<new name>");
    assert!(made(&p).is_empty(), "nothing is written");
    // Dropping one leaves the other alone with its definition: renamed, keeping its rows.
    let choices = create(&["users.legacy"], &[]);
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "tidy", &choices, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"  ! users\.middle_name → nickname is written as a rename, keeping its rows\n    If they're different columns, delete the file, then:\n      reylo migrate new tidy --drop users\.legacy,users\.middle_name");
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
    p.relock();
    let opts = p.opts();
    // Two instances of an app, each migrating as it starts, on one database.
    let runs: Vec<(i32, String)> = std::thread::scope(|s| {
        let started: Vec<_> = (0..2)
            .map(|_| {
                let opts = &opts;
                s.spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                    let mut lines = Vec::new();
                    let code = rt.block_on(reylo_migrate::deploy(opts, &mut |l: &str| lines.push(l.to_string()))).unwrap();
                    (code, lines.join("\n"))
                })
            })
            .collect();
        started.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (code, out) in &runs {
        assert_eq!(*code, 0, "{out}");
    }
    let applied: usize = runs.iter().map(|(_, out)| out.matches(" applied ·").count()).sum();
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
    p.relock();
    let both = [p.opts(), Options { url: q.url.clone(), ..p.opts() }];
    for round in 0..3 {
        let _ = std::fs::remove_dir_all(p.dir.join(".cache"));
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
                        let code = rt.block_on(reylo_migrate::dry_run(o, &mut |l: &str| lines.push(l.to_string()))).unwrap();
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
    history_push_leaves_a_data_migration_pending,
    a_platforms_default_privileges_are_not_drift,
    function_privileges_and_index_comments_are_compared,
    a_comment_made_by_hand_stops_nothing,
    a_comment_in_schema_sql_is_written,
    a_server_setting_change_misses_the_cache,
    a_migrations_session_setting_is_never_carried_by_the_cache,
    init_reads_a_function_dumped_before_the_table_it_reads,
    a_platforms_schemas_and_extensions_are_in_every_shadow,
);
on!(Mysql, a_migration_that_names_a_database_is_refused);
on!(
    Postgres,
    a_partition_of_a_run_time_partition_is_data,
    an_extension_in_a_schema_of_its_own_is_dumped_after_it,
    a_partition_dropped_at_run_time_is_data,
    init_creates_the_extensions_its_schema_uses,
    a_dump_keeps_a_function_body_as_written,
    a_history_that_does_not_replay_is_refused_before_anything_changes,
    without_a_lock_a_history_out_of_name_order_is_held_to_name_order,
);
on!(Mysql, a_recurring_event_is_in_the_dump);
on!(Sqlite, a_virtual_tables_own_tables_are_not_in_the_dump);
mod on_mysql {
    use super::*;
    on!(Mysql, a_comment_made_by_hand_stops_nothing, a_comment_in_schema_sql_is_written);
}
on!(Sqlite, a_rebuild_keeps_every_row_where_foreign_keys_are_on, a_rebuild_keeps_tables_that_reference_each_other);
on!(Mysql, a_failure_that_leaves_part_of_itself_is_seen_by_the_next_deploy);
on!(Mysql, a_table_comment_is_no_clause_of_its_table);
on!(Mysql, a_reserved_word_stays_quoted);
on!(Mysql, a_partition_added_at_run_time_is_data);
on!(
    Postgres,
    a_narrowing_type_change_fails_rather_than_cuts,
    a_number_to_a_short_varchar_fails_rather_than_cuts,
    a_replaced_column_is_a_stated_drop,
    a_rename_to_a_name_that_needs_quotes,
    an_index_left_invalid_by_a_cut_off_build_is_dropped_and_built_again,
    a_renamed_tables_sequence_keeps_counting,
    a_foreign_table_is_renamed_as_stated,
    a_partitioned_table_is_renamed_with_its_partitions,
    partitions_made_at_run_time_are_data,
    the_fingerprint_leaves_out_only_what_is_a_childs,
    a_comment_above_a_cut_off_build_is_read_past,
    table_guesses_name_their_schema,
    a_childs_column_is_renamed_with_its_parents,
    row_counts_are_the_dropped_tables_own,
    a_run_that_did_something_else_than_its_replay_is_found_by_check,
    more_type_changes_that_lose_values_are_named_in_drop,
    a_schemas_table_is_never_read_as_a_renamed_tables_column,
);

#[tokio::test(flavor = "current_thread")]
async fn a_sqlite_numeric_column_made_integer_needs_no_drop() {
    let mut p = P::new(Dialect::Sqlite).await.unwrap();
    p.file("20261027100000_t.sql", "create table t (id integer primary key, v real);");
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    // SQLite keeps 1.5 in an INTEGER column, so nothing is lost.
    p.schema("create table t (id integer primary key, v integer);");
    assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "ints", &Default::default(), log).await).await, 0, "{}", p.peek());
}

#[tokio::test(flavor = "current_thread")]
async fn wipe_keeps_full_text_and_rtree_tables_whole() {
    let mut p = P::new(Dialect::Sqlite).await.unwrap();
    p.file("20261027100000_docs.sql", "create virtual table docs using fts5(body);");
    p.file("20261027110000_box.sql", "create virtual table box using rtree(id, x0, x1);");
    p.file("20261027120000_notes.sql", "create table notes (id integer primary key, body text); create virtual table bare using fts5(body, content=''); create virtual table indexed using fts5(body, content='notes', content_rowid='id');");
    let schema = "create virtual table docs using fts5(body); create virtual table box using rtree(id, x0, x1); create table notes (id integer primary key, body text); create virtual table bare using fts5(body, content=''); create virtual table indexed using fts5(body, content='notes', content_rowid='id');";
    p.schema(schema);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
    p.exec("insert into docs values ('hello world'); insert into box values (1, 0, 1); insert into bare values ('hello'); insert into notes values (1, 'hello'); insert into indexed (rowid, body) values (1, 'hello')").await;
    p.output();
    assert_eq!(p.run(async |o, log| reylo_migrate::db_wipe(o, false, log).await).await, 0, "{}", p.peek());
    assert_has!(p.output(), r"Deleted every row of 5 tables");
    assert!(p.rows("select rowid from bare where bare match 'hello'").await.is_empty(), "a contentless index is emptied");
    assert!(p.rows("select rowid from indexed where indexed match 'hello'").await.is_empty(), "an external-content index is emptied with its content");
    assert!(p.rows("select * from docs").await.is_empty() && p.rows("select * from box").await.is_empty());
    // Each still works: their own tables were emptied by them, not under them.
    p.exec("insert into docs values ('again'); insert into box values (2, 0, 1)").await;
    assert_eq!(p.rows("select * from docs where docs match 'again'").await.len(), 1);
    assert_eq!(p.rows("select id from box where x0 <= 0.5 and x1 >= 0.5").await.len(), 1);
    assert_eq!(p.migrate().await, 0, "{}", p.peek());
}

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
        assert_eq!(p.dry_run().await, 1);
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
        p.exec("do $$ begin create role supabase_auth_admin; exception when duplicate_object or unique_violation then null; end $$").await;
        p.exec("create schema auth authorization supabase_auth_admin; create table auth.users (id int primary key); alter table auth.users owner to supabase_auth_admin; create function auth.uid() returns int language sql as 'select 1'; alter function auth.uid() owner to supabase_auth_admin").await;
        p.schema("create table profiles (id int primary key);");
        p.file("20261029100000_profiles.sql", "create table profiles (id int primary key);");
        assert_eq!(p.migrate().await, 0, "the platform's schema, table and function are not drift:\n{}", p.peek());
        // The project's own objects on the platform's table are the project's: one added by hand is drift.
        p.exec("create policy own on auth.users using (true)").await;
        p.output();
        assert_eq!(p.dry_run().await, 1);
        assert_has!(p.output(), r"This database was changed outside its migrations[\s\S]*policy own on auth\.users");
    }

    pub async fn a_platforms_schemas_and_extensions_are_in_every_shadow(mut p: P) {
        // As Supabase makes a project: a schema a platform's role owns, with the table a project's
        // key and trigger name, and an extension in a schema of the project's role.
        p.exec("do $$ begin create role supabase_auth_admin; exception when duplicate_object or unique_violation then null; end $$").await;
        p.exec("create schema auth authorization supabase_auth_admin; create table auth.users (id uuid primary key); alter table auth.users owner to supabase_auth_admin").await;
        p.exec("create schema extensions; create extension pgcrypto with schema extensions").await;
        p.schema(
            "create table profiles (id uuid primary key references auth.users on delete cascade, token text default encode(extensions.gen_random_bytes(8), 'hex'));
            create function handle_new_user() returns trigger language plpgsql as $$ begin insert into public.profiles (id) values (new.id); return new; end $$;
            create trigger on_auth_user_created after insert on auth.users for each row execute function handle_new_user();",
        );
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "profiles", &Default::default(), log).await).await, 0, "{}", p.peek());
        let written = std::fs::read_to_string(p.dir.join("migrations").join(&p.files()[0])).unwrap();
        assert!(written.contains("references auth.users") && written.contains("on auth.users"), "{written}");
        assert!(!written.contains("create schema auth") && !written.contains("create table auth.users") && !written.contains("create extension"), "{written}");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select 1 from pg_extension where extname = 'pgcrypto'").await.len(), 1, "the platform's extension stays");
        p.exec("insert into auth.users values ('00000000-0000-0000-0000-000000000001')").await;
        assert_eq!(p.rows("select token from profiles").await.len(), 1, "the trigger and the default ran");
    }

    pub async fn init_reads_a_function_dumped_before_the_table_it_reads(mut p: P) {
        // pg_dump writes functions before tables, and tables before the keys between them.
        p.exec("create table users (id int primary key, best_post int); create table posts (id int primary key, author int references users); alter table users add foreign key (best_post) references posts").await;
        p.exec("create function user_count() returns bigint language sql as $$ select count(*) from users $$; create table quotas (n int check (n <= user_count() + 100))").await;
        assert_eq!(p.run(async |o, log| reylo_migrate::init(o, &[], log).await).await, 0, "{}", p.peek());
        let text = std::fs::read_to_string(p.dir.join("schema.sql")).unwrap();
        assert!(text.contains("function user_count()") && text.contains("create table quotas"), "{text}");
        assert_eq!(reylo_migrate::db_introspect(&p.opts(), &[]).await.unwrap(), text);
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    pub async fn reylo_run_runs_inline_sql_and_files(mut p: P) {
        let items = [("create".to_string(), "create table t (x int); insert into t values (1)".to_string()), ("seed.sql".to_string(), "insert into t values (2)".to_string())];
        let (url, dir) = (p.url.clone(), p.dir.clone());
        assert_eq!(p.run(async |_, log| reylo_migrate::run_sql(&url, &dir, &items, log).await).await, 0, "{}", p.peek());
        assert_eq!(p.rows("select x from t order by x").await.iter().map(|r| r.text("x").to_string()).collect::<Vec<_>>(), ["1", "2"]);
        assert_eq!(p.rows("select to_regclass('__reylo_migrations') as t").await[0].get("t"), None, "records nothing");
    }

    pub async fn history_push_leaves_a_data_migration_pending(mut p: P) {
        p.schema("create table t (x int);");
        p.file("20261027100000_t.sql", "create table t (x int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into t values (1)").await;
        p.file("20261027110000_backfill.sql", "update t set x = x * 10;");
        p.output();
        assert_eq!(p.history_push().await, 0, "{}", p.peek());
        assert_has!(p.output(), r"1 pending, which reylo migrate deploy runs: 20261027110000_backfill\.sql");
        assert_eq!(p.rows("select x from t").await[0].text("x"), "1", "history push ran nothing");
    }

    pub async fn a_platforms_default_privileges_are_not_drift(mut p: P) {
        // As a platform sets them before any migration: every new table and function its API's role may use.
        p.exec("do $$ begin create role madv_api; exception when duplicate_object or unique_violation then null; end $$").await;
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

    pub async fn a_comment_made_by_hand_stops_nothing(mut p: P) {
        let by_hand = if p.d == Dialect::Mysql { "alter table t comment = 'by hand'" } else { "comment on table t is 'by hand'" };
        p.schema("create table t (id int);");
        p.file("20261027100000_t.sql", "create table t (id int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.output();
        p.exec(by_hand).await;
        // No drift: the next deploy runs, the replay agrees, the history stands and development leaves it be.
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.output();
        let (code, out) = p.check_every_way().await;
        assert_eq!(code, 0, "{out}");
        assert_eq!(p.history_push().await, 0, "{}", p.peek());
        p.output();
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_has!(p.output(), "Up to date");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
    }

    pub async fn a_comment_in_schema_sql_is_written(mut p: P) {
        let declared = |text: &str| if p.d == Dialect::Mysql { format!("create table t (id int) comment '{text}';") } else { format!("create table t (id int); comment on table t is '{text}';") };
        let (first, second) = (declared("the table"), declared("the only table"));
        p.schema("create table t (id int);");
        p.file("20261027100000_t.sql", "create table t (id int);");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.output();
        // In a migration, and until one writes it, the migrations don't reach schema.sql.
        p.schema(&first);
        assert_eq!(p.dry_run().await, 1, "{}", p.peek());
        p.output();
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "doc", &Default::default(), log).await).await, 0, "{}", p.peek());
        p.output();
        let written = p.files().into_iter().find(|f| f.ends_with("_doc.sql")).unwrap();
        let text = std::fs::read_to_string(p.dir.join("migrations").join(&written)).unwrap();
        assert!(has(&text, "the table"), "{text}");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.output();
        let (code, out) = p.check_every_way().await;
        assert_eq!(code, 0, "{out}");
        // And by sync, which takes the database to schema.sql, a comment alone included.
        p.schema(&second);
        assert_eq!(p.db_push().await, 0, "{}", p.peek());
        assert_has!(p.output(), "the only table");
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
        assert_has!(out, r"This database was changed outside its migrations");
        forget(&p.url).await;
        // The target's own database setting, which every shadow takes, likewise.
        p.url = plain;
        p.exec(&format!("alter database {} set search_path = app, public", p.name)).await;
        let (code, out) = p.check_every_way().await;
        assert_eq!(code, 1);
        assert_has!(out, r"This database was changed outside its migrations");
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

    pub async fn a_number_to_a_short_varchar_fails_rather_than_cuts(mut p: P) {
        p.schema("create table t (code int);");
        p.file("20261027100000_t.sql", "create table t (code int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into t values (12345)").await;
        // An integer reaches a string type by I/O conversion, which no pg_cast row lists and which
        // is an assignment cast: no USING, so the value that doesn't fit is refused, not cut.
        p.schema("create table t (code varchar(3));");
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "narrow", &Default::default(), log).await).await, 0, "{}", p.peek());
        let made = p.files().into_iter().find(|f| f.ends_with("_narrow.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(&made)).unwrap();
        assert!(!sql.contains("using"), "{sql}");
        p.output();
        assert_eq!(p.migrate().await, 1, "the value does not fit");
        assert_has!(p.output(), "too long");
        assert_eq!(p.rows("select code from t").await[0].text("code"), "12345");
    }

    pub async fn a_replaced_column_is_a_stated_drop(mut p: P) {
        p.schema("create table t (a int, total int generated always as (a * 2) stored);");
        p.file("20261027100000_t.sql", "create table t (a int, total int generated always as (a * 2) stored);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into t (a) values (2)").await;
        // Generated to plain is made in place, and the values stay.
        p.schema("create table t (a int, total int);");
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "plain", &Default::default(), log).await).await, 0, "{}", p.peek());
        let made = p.files().into_iter().find(|f| f.ends_with("_plain.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(&made)).unwrap();
        assert!(sql.contains("drop expression") && !sql.contains("drop column"), "{sql}");
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select total from t").await[0].text("total"), "4");
        // Plain to generated replaces the column, and its values with it: --drop must say so.
        p.schema("create table t (a int, total int generated always as (a * 3) stored);");
        p.output();
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "generated", &Default::default(), log).await).await, 1);
        assert_has!(p.output(), r"This migration drops the column t\.total and its data");
        let choices = reylo_migrate::Choices { drop: vec!["t.total".into()], rename: vec![] };
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "generated", &choices, log).await).await, 0, "{}", p.peek());
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select total from t").await[0].text("total"), "6");
    }

    pub async fn a_rename_to_a_name_that_needs_quotes(mut p: P) {
        p.schema("create table users (id int, n int);");
        p.file("20261027100000_users.sql", "create table users (id int, n int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into users values (1, 42)").await;
        p.schema(r#"create table users (id int, "Total" bigint);"#);
        let choices = reylo_migrate::Choices { drop: vec![], rename: vec![("users.n".into(), "Total".into())] };
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "total", &choices, log).await).await, 0, "{}", p.peek());
        let made = p.files().into_iter().find(|f| f.ends_with("_total.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(&made)).unwrap();
        assert!(sql.contains(r#"rename column n to "Total""#), "{sql}");
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.rows(r#"select "Total" as v from users"#).await[0].text("v"), "42");
    }

    pub async fn an_index_left_invalid_by_a_cut_off_build_is_dropped_and_built_again(mut p: P) {
        p.schema("create table t (x int); create unique index t_x on t (x);");
        p.file("20261027100000_t.sql", "create table t (x int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into t values (1), (1)").await;
        // The run was cut off mid-build: its row marked started, and the index it left invalid.
        let build = "create unique index concurrently t_x on t (x);";
        p.file("20261027110000_unique.sql", build);
        p.exec(&format!("insert into __reylo_migrations (name, sql, applied_at) values ('20261027110000_unique.sql', '{build}', null)")).await;
        let mut db = Db::connect(&p.url, &p.dir).await.unwrap();
        assert!(db.script(build).await.is_err());
        db.close().await;
        assert_eq!(p.rows("select 1 from pg_index where not indisvalid").await.len(), 1);
        p.exec("delete from t where ctid = (select max(ctid) from t)").await;
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        assert_has!(p.output(), r"20261027110000_unique\.sql was cut off and left public\.t_x invalid: reylo migrate deploy drops it, then runs 20261027110000_unique\.sql again");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        let out = p.output();
        assert_has!(&out, r"Dropped public\.t_x, which 20261027110000_unique\.sql left invalid when it was cut off");
        assert_has!(&out, r"✓ 20261027110000_unique\.sql +applied");
        assert!(p.rows("select 1 from pg_index where not indisvalid").await.is_empty());
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), "Up to date");
    }

    pub async fn a_renamed_tables_sequence_keeps_counting(mut p: P) {
        p.schema("create table users (id serial primary key, email text unique);");
        p.file("20261027100000_users.sql", "create table users (id serial primary key, email text unique);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into users (email) values ('a'), ('b'), ('c')").await;
        // What Postgres named after the table is renamed with it: its sequence keeps its value.
        p.schema("create table accounts (id serial primary key, email text unique);");
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "accounts", &Default::default(), log).await).await, 0, "{}", p.peek());
        let made = p.files().into_iter().find(|f| f.ends_with("_accounts.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(&made)).unwrap();
        for part in ["alter table users rename to accounts", "rename constraint users_pkey to accounts_pkey", "rename constraint users_email_key to accounts_email_key", "alter sequence users_id_seq rename to accounts_id_seq"] {
            assert!(sql.contains(part), "{part} in:\n{sql}");
        }
        assert!(!sql.lines().any(|l| !l.starts_with("--") && (l.contains("create") || l.contains("drop"))), "{sql}");
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into accounts (email) values ('d')").await;
        assert_eq!(p.rows("select id from accounts where email = 'd'").await[0].text("id"), "4");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    pub async fn a_foreign_table_is_renamed_as_stated(mut p: P) {
        let server = "create foreign data wrapper madv_w; create server madv_s foreign data wrapper madv_w;";
        p.file("20261027100000_users.sql", &format!("{server} create foreign table users (id int, email text) server madv_s;"));
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.schema(&format!("{server} create foreign table accounts (id int, email text) server madv_s;"));
        // Its rows are its server's: dropping it loses none here, and its rename is a foreign table's.
        let choices = reylo_migrate::Choices { drop: vec![], rename: vec![("users".into(), "accounts".into())] };
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "accounts", &choices, log).await).await, 0, "{}", p.peek());
        let made = p.files().into_iter().find(|f| f.ends_with("_accounts.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(&made)).unwrap();
        assert!(sql.contains("alter foreign table users rename to accounts"), "{sql}");
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    pub async fn a_partitioned_table_is_renamed_with_its_partitions(mut p: P) {
        p.file("20261027100000_events.sql", "create table events (id int, at date, primary key (id, at)) partition by range (at); create table events_2024 partition of events for values from ('2024-01-01') to ('2025-01-01');");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into events values (1, '2024-06-01')").await;
        p.schema("create table logs (id int, at date, primary key (id, at)) partition by range (at); create table logs_2024 partition of logs for values from ('2024-01-01') to ('2025-01-01');");
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "logs", &Default::default(), log).await).await, 0, "{}", p.peek());
        let made = p.files().into_iter().find(|f| f.ends_with("_logs.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(&made)).unwrap();
        assert!(sql.contains("alter table events rename to logs") && sql.contains("alter table events_2024 rename to logs_2024") && sql.contains("rename to logs_2024_pkey"), "{sql}");
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select id from logs_2024").await.len(), 1);
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    pub async fn a_run_that_did_something_else_than_its_replay_is_found_by_check(mut p: P) {
        p.schema("create table t (id int);");
        p.file("20261027100000_t.sql", "create table t (id int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into t values (1)").await;
        // DDL that depends on the rows: an index here, none in the empty replay.
        p.file("20261027110000_idx.sql", "do $$ begin if exists (select 1 from t) then create index t_id on t (id); end if; end $$;");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.output();
        // Its result is what deploy compares with next; check holds it against the replay.
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), r"^✓ Up to date$");
        assert_eq!(p.dry_run().await, 1);
        assert_has!(p.output(), r"This database was changed outside its migrations[\s\S]*t_id");
    }

    pub async fn more_type_changes_that_lose_values_are_named_in_drop(mut p: P) {
        p.file("20261027100000_t.sql", "create table t (f double precision, s text, j json, n numeric(10,0), i interval);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.schema("create table t (f numeric(10,2), s integer, j jsonb, n bigint, i interval day);");
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "narrow", &Default::default(), log).await).await, 1);
        let out = p.output();
        for line in [r"t\.f changes from double precision to numeric\(10,2\), which rounds to 2 decimal places", r"t\.s changes from text to integer, which stores text that reads as a number", r"t\.j changes from json to jsonb, which keeps the JSON's values", r"t\.i changes from interval to interval day, which keeps only the day of each interval"] {
            assert_has!(&out, line);
        }
        assert!(!out.contains("t.n changes"), "numeric(10,0) holds whole numbers: {out}");
    }

    pub async fn partitions_made_at_run_time_are_data(mut p: P) {
        let schema = "create table events (id int, at date) partition by range (at); create table events_2024 partition of events for values from ('2024-01-01') to ('2025-01-01'); create table metrics (id int, v int);";
        p.schema(schema);
        p.file("20261027100000_events.sql", schema);
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        // pg_partman makes next year's partition, and TimescaleDB-like children inherit a table.
        p.exec("create table events_2025 partition of events for values from ('2025-01-01') to ('2026-01-01'); insert into events values (1, '2025-03-01'); create table metrics_c1 () inherits (metrics); insert into metrics_c1 values (1, 2)").await;
        p.output();
        // The fingerprint leaves children out, so nothing has changed for deploy, or for check.
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), r"^✓ Up to date$");
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        p.output();
        // Development keeps them too, with their rows.
        assert_eq!(p.dev_then_push().await, 0, "{}", p.peek());
        let out = p.output();
        assert!(!out.contains("events_2025") && !out.contains("metrics_c1"), "{out}");
        assert_eq!(p.rows("select * from events_2025").await.len(), 1);
        assert_eq!(p.rows("select * from metrics_c1").await.len(), 1);
        // A partition a migration made, dropped at run time (a retention job's), is data too.
        p.exec("drop table events_2024").await;
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
    }

    pub async fn a_schemas_table_is_never_read_as_a_renamed_tables_column(mut p: P) {
        p.file("20261027100000_t.sql", "create schema billing; create table billing.invoices (id int); create table billing (id int, total int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.schema("create schema billing; create table ledger (id int, total int);");
        let choices = reylo_migrate::Choices { drop: vec!["billing.invoices".into()], rename: vec![] };
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "ledger", &choices, log).await).await, 0, "{}", p.peek());
        let made = p.files().into_iter().find(|f| f.ends_with("_ledger.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(&made)).unwrap();
        assert!(sql.contains("alter table billing rename to ledger") && sql.contains("drop table billing.invoices"), "{sql}");
    }

    pub async fn the_fingerprint_leaves_out_only_what_is_a_childs(mut p: P) {
        // A schema named as a table, and a view over a partition: neither line is the child's.
        let schema = "create schema users; create table ev (id int, at date, orders int) partition by range (at); create table ev_2024 partition of ev for values from ('2024-01-01') to ('2025-01-01'); create table users.orders (id int, at date, orders int); create table users (orders text); create view v as select id from ev_2024 where id > 0;";
        p.schema(schema);
        p.file("20261027100000_t.sql", schema);
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.output();
        p.exec("create or replace view v as select id from ev_2024 where id > 100").await;
        assert_eq!(p.migrate().await, 1, "{}", p.peek());
        assert_has!(p.output(), "was changed outside its migrations");
        p.exec("create or replace view v as select id from ev_2024 where id > 0").await;
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.output();
        // A child's own lines are out, its indexes and triggers too.
        p.exec("create table ev_2025 partition of ev for values from ('2025-01-01') to ('2026-01-01'); create index ev_2025_id on ev_2025 (id)").await;
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), r"^✓ Up to date$");
    }

    pub async fn a_comment_above_a_cut_off_build_is_read_past(mut p: P) {
        p.schema("create table t (v int); create unique index t_v on t (v);");
        p.file("20261027100000_t.sql", "create table t (v int);");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        let lone = "-- speed up lookups by v\ncreate unique index concurrently t_v on t (v);";
        p.file("20261027110000_v.sql", lone);
        // The build cut off: a row marked started, and the index left invalid (two equal rows).
        p.exec("insert into t values (1), (1)").await;
        let _ = p.try_rows("create unique index concurrently t_v on t (v)").await;
        p.exec("delete from t").await;
        let mut db = Db::connect(&p.url, &p.dir).await.unwrap();
        db.exec_with(&format!("insert into __reylo_migrations (name, sql, applied_at) values ({}, {}, null)", db.param(1), db.param(2)), &["20261027110000_v.sql", lone]).await.unwrap();
        db.close().await;
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), r"Dropped public\.t_v, which 20261027110000_v\.sql left invalid[\s\S]*✓ 20261027110000_v\.sql +applied");
    }

    pub async fn table_guesses_name_their_schema(mut p: P) {
        let tables = |t: &str, extra: &str| format!("create table a.{t} (id int, x int, y int, z int{extra}); create table b.{t} (id int, x int, y int, z int{extra});");
        p.file("20261027100000_t.sql", &format!("create schema a; create schema b; {}", tables("orders", ", old int")));
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.output();
        p.schema(&format!("create schema a; create schema b; {}", tables("invoices", ", new_one text")));
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "names", &Default::default(), log).await).await, 1);
        let out = p.output();
        assert_has!(&out, r"reylo migrate new names \\\n      --drop [^\n]+ \\\n      --rename a\.orders=a\.invoices \\\n      --rename b\.orders=b\.invoices");
        let keeps = out.lines().skip_while(|l| !l.contains("If they are renames")).skip(1).take_while(|l| l.starts_with("    ")).collect::<Vec<_>>().join("\n");
        let choices = choices_of(&keeps);
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "names", &choices, log).await).await, 0, "{keeps}\n{}", p.peek());
    }

    pub async fn a_childs_column_is_renamed_with_its_parents(mut p: P) {
        let before = "create table ev (id int, at date, val varchar(10)) partition by range (at); create table ev_2024 partition of ev for values from ('2024-01-01') to ('2025-01-01'); create table par (id int, val varchar(10)); create table kid () inherits (par);";
        p.file("20261027100000_t.sql", before);
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into ev values (1, '2024-03-01', 'x'); insert into kid values (1, 'y')").await;
        p.output();
        p.schema(&before.replace("val varchar(10)", "amount varchar(20)"));
        // Each parent's rename is listed, once; its children go with it.
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "amount", &Default::default(), log).await).await, 1);
        let out = p.output();
        assert_has!(&out, r"These 2 look like renames");
        assert!(!out.contains("ev_2024.val") && !out.contains("kid."), "{out}");
        let keeps = out.lines().skip_while(|l| !l.contains("If they are renames")).skip(1).take_while(|l| l.starts_with("    ")).collect::<Vec<_>>().join("\n");
        let choices = choices_of(&keeps);
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "amount", &choices, log).await).await, 0, "{keeps}\n{}", p.peek());
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select amount from ev_2024").await[0].text("amount"), "x");
        assert_eq!(p.rows("select amount from kid").await[0].text("amount"), "y");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    pub async fn row_counts_are_the_dropped_tables_own(mut p: P) {
        p.schema("create schema a; create table a.orders (id int primary key);");
        p.file("20261027100000_a.sql", "create schema a; create table a.orders (id int primary key);");
        p.file("20261027110000_o.sql", "create table orders (id int primary key);");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into a.orders values (1), (2), (3)").await;
        std::fs::remove_file(p.dir.join("migrations").join("20261027110000_o.sql")).unwrap();
        p.output();
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_has!(p.output(), r"(?m)! Dropped orders$");
        assert_eq!(p.rows("select * from a.orders").await.len(), 3);
    }

    pub async fn a_partition_added_at_run_time_is_data(mut p: P) {
        let table = "create table sales (id int not null, y int not null, primary key (id, y)) partition by range (y) (partition p2025 values less than (2026), partition p2026 values less than (2027));";
        p.schema(table);
        p.file("20261027100000_sales.sql", table);
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.output();
        // A job adds next year's partition: data, as a Postgres partition made at run time is.
        p.exec("alter table sales add partition (partition p2027 values less than (2028))").await;
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), "Up to date");
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        p.output();
        // A declared one dropped at run time, as a retention job drops the oldest, is data too.
        p.exec("alter table sales drop partition p2026").await;
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
    }

    pub async fn a_failure_that_leaves_part_of_itself_is_seen_by_the_next_deploy(mut p: P) {
        p.file("20261027100000_t.sql", "create table t (id int primary key);");
        p.file("20261027110000_proc.sql", "create procedure p() begin create table t2 (id int); select * from nope; end;");
        p.file("20261027120000_call.sql", "call p();");
        assert_eq!(p.migrate().await, 1);
        let out = p.output();
        assert_has!(&out, r"✓ 20261027110000_proc\.sql +applied[\s\S]*✗ 20261027120000_call\.sql  failed\n");
        assert!(!out.contains("nothing of it was applied"), "{out}");
        assert_eq!(p.rows("select * from t2").await.len(), 0);
        // What it left isn't what the last migration that ran left.
        assert_eq!(p.migrate().await, 1);
        assert_has!(p.output(), "was changed outside its migrations");
    }

    pub async fn a_table_comment_is_no_clause_of_its_table(mut p: P) {
        p.file("20261027100000_t.sql", "create table users (id int); create table a (id int) comment 'a partition of users by region';");
        // MySQL commits each statement: one to a file.
        std::fs::remove_file(p.dir.join("migrations").join("20261027100000_t.sql")).unwrap();
        p.file("20261027100000_t.sql", "create table users (id int);");
        p.file("20261027100001_a.sql", "create table a (id int) comment 'a partition of users by region';");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.output();
        p.exec("alter table a add column note int").await;
        assert_eq!(p.migrate().await, 1, "{}", p.peek());
        assert_has!(p.output(), "was changed outside its migrations");
    }

    pub async fn a_reserved_word_stays_quoted(mut p: P) {
        p.schema("create table `order` (id int primary key, `group` varchar(20));");
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "order", &Default::default(), log).await).await, 0, "{}", p.peek());
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select * from `order`").await.len(), 0);
    }

    pub async fn a_rebuild_keeps_every_row_where_foreign_keys_are_on(mut p: P) {
        // A parent, and tables that reference it every way: cascade, set null, none, at a second
        // level, and itself; each with what a rebuild takes down and makes again.
        let tables = |check: &str| {
            format!(
                "create table parents (id integer primary key, boss integer references parents (id) on delete cascade, name text{check});
create table kids (id integer primary key autoincrement, parent_id integer references parents (id) on delete cascade, note text);
create index kids_parent on kids (parent_id);
create table pets (id integer primary key, parent_id integer references parents (id) on delete set null);
create table chores (id integer primary key, kid_id integer references kids (id));
create table toys (kid_id integer references kids (id) on delete cascade, name text);
create table a_links (parent_id integer references parents (id), kid_id integer references kids (id));
create view kid_names as select kids.note, parents.name from kids join parents on parents.id = kids.parent_id;
create trigger kids_note after insert on kids begin update parents set name = name where id = new.parent_id; end;"
            )
        };
        p.schema(&tables(""));
        p.file("20261027100000_t.sql", &tables(""));
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into parents values (1, null, 'a'), (2, 1, 'b'); insert into kids values (7, 1, 'x'), (8, 2, 'y'), (9, 2, 'z'); insert into pets values (1, 1), (2, 2); insert into chores values (1, 7), (2, 8); insert into toys values (9, 'ball'); insert into a_links values (1, 7)").await;
        p.output();
        // A check forces the parent's rebuild.
        p.schema(&tables(", check (length(name) > 0)"));
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "check_name", &Default::default(), log).await).await, 0, "{}", p.peek());
        p.output();
        let made = p.files().into_iter().find(|f| f.ends_with("_check_name.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(&made)).unwrap();
        assert!(!sql.contains("pragma"), "{sql}");
        // The file as any runner would run it: on a connection with foreign keys on, in a transaction.
        let mut db = Db::connect(&p.url, &p.dir).await.unwrap();
        assert_eq!(db.rows("pragma foreign_keys").await.unwrap()[0].first().map(|v| v.to_string()), Some("1".to_string()));
        db.exec("begin").await.unwrap();
        db.script(&sql).await.unwrap_or_else(|e| panic!("{e}\n{sql}"));
        db.exec("commit").await.unwrap();
        assert!(db.rows("pragma foreign_key_check").await.unwrap().is_empty());
        let count = |name: String, rows: Vec<Row>| (name, rows[0].text("n").to_string());
        for (table, n) in [("parents", "2"), ("kids", "3"), ("pets", "2"), ("chores", "2"), ("toys", "1"), ("a_links", "1")] {
            let rows = db.rows(&format!("select count(*) as n from {table}")).await.unwrap();
            assert_eq!(count(table.into(), rows), (table.to_string(), n.to_string()), "{sql}");
        }
        assert_eq!(db.rows("select count(*) as n from pets where parent_id is null").await.unwrap()[0].text("n"), "0", "{sql}");
        assert_eq!(db.rows("select count(*) as n from parents where boss = 1").await.unwrap()[0].text("n"), "1", "{sql}");
        assert_eq!(db.rows("select count(*) as n from kid_names").await.unwrap()[0].text("n"), "3", "{sql}");
        // AUTOINCREMENT's counter came along, and the keys still hold.
        db.exec("insert into kids (parent_id, note) values (1, 'next')").await.unwrap();
        assert_eq!(db.rows("select max(id) as n from kids").await.unwrap()[0].text("n"), "10");
        assert!(db.exec("insert into kids (parent_id) values (99)").await.is_err(), "the key to the new parents holds");
        db.exec("delete from kids where note = 'next'").await.unwrap();
        db.close().await;
        // The database reads as the file says, and as schema.sql does.
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    pub async fn a_rebuild_keeps_tables_that_reference_each_other(mut p: P) {
        let tables = |check: &str| format!("create table a (id integer primary key, b_id integer references b (id), name text{check}); create table b (id integer primary key, a_id integer references a (id) on delete cascade);");
        p.schema(&tables(""));
        p.file("20261027100000_t.sql", &tables(""));
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into a values (1, null, 'x'); insert into b values (1, 1); update a set b_id = 1").await;
        p.output();
        p.schema(&tables(", check (length(name) > 0)"));
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "check_name", &Default::default(), log).await).await, 0, "{}", p.peek());
        p.output();
        let made = p.files().into_iter().find(|f| f.ends_with("_check_name.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(&made)).unwrap();
        assert!(sql.contains("pragma defer_foreign_keys = on"), "{sql}");
        let mut db = Db::connect(&p.url, &p.dir).await.unwrap();
        db.exec("begin").await.unwrap();
        db.script(&sql).await.unwrap_or_else(|e| panic!("{e}\n{sql}"));
        db.exec("commit").await.unwrap();
        assert!(db.rows("pragma foreign_key_check").await.unwrap().is_empty());
        assert_eq!(db.rows("select b_id from a").await.unwrap()[0].text("b_id"), "1", "{sql}");
        assert_eq!(db.rows("select a_id from b").await.unwrap()[0].text("a_id"), "1", "{sql}");
        db.close().await;
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    pub async fn a_migration_that_names_a_database_is_refused(mut p: P) {
        p.schema("create table t (id int);");
        p.file("20261027100000_t.sql", &format!("create table {}.t (id int);", p.name));
        assert_eq!(p.dry_run().await, 1);
        assert_has!(p.output(), "names the database");
        assert!(p.rows(&format!("select 1 from information_schema.tables where table_schema = '{}' and table_name = 't'", p.name)).await.is_empty());
    }

    pub async fn a_partition_of_a_run_time_partition_is_data(mut p: P) {
        p.schema("create table ev (id int, at date not null, region text not null) partition by range (at);");
        p.file("20261027100000_ev.sql", "create table ev (id int, at date not null, region text not null) partition by range (at);");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("create table ev_2026 partition of ev for values from ('2026-01-01') to ('2027-01-01') partition by list (region); create table ev_2026_eu partition of ev_2026 for values in ('eu'); insert into ev values (1, '2026-03-01', 'eu');").await;
        p.schema("create table ev (id int, at date not null, region text not null) partition by range (at); create table other (id int);");
        assert_eq!(p.db_push().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from ev").await[0].text("n"), "1");
        assert_eq!(p.dry_run().await, 1, "{}", p.peek());
        assert!(!p.output().contains("ev_2026"));
    }

    pub async fn a_partition_dropped_at_run_time_is_data(mut p: P) {
        let ddl = "create table ev (id int, at date not null) partition by range (at); create table ev_2025 partition of ev for values from ('2025-01-01') to ('2026-01-01');";
        p.schema(ddl);
        p.file("20261027100000_ev.sql", ddl);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("drop table ev_2025").await;
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
    }

    pub async fn init_creates_the_extensions_its_schema_uses(mut p: P) {
        p.exec("create extension citext; create table u (id int primary key, email citext);").await;
        assert_eq!(p.run(async |o, log| reylo_migrate::init(o, &[], log).await).await, 0, "{}", p.peek());
        assert_has!(std::fs::read_to_string(p.dir.join("migrations/init.sql")).unwrap(), r"(?i)create extension (if not exists )?citext");
        let q = P::new(p.d).await.unwrap();
        p.url = q.url.clone();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(q.history().await, ["init.sql"]);
    }

    pub async fn an_extension_in_a_schema_of_its_own_is_dumped_after_it(mut p: P) {
        p.exec("create schema app; create extension citext with schema app; create table app.u (id int primary key, email app.citext);").await;
        assert_eq!(p.run(async |o, log| reylo_migrate::init(o, &["app".to_string()], log).await).await, 0, "{}", p.peek());
        let q = P::new(p.d).await.unwrap();
        p.url = q.url.clone();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(q.history().await, ["init.sql"]);
    }

    pub async fn a_dump_keeps_a_function_body_as_written(mut p: P) {
        p.file("20261027100000_counters.sql", "create table counters (n int);");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("create function bump() returns void language sql as $$\nupdate counters\nSET n = n + 1;\n$$;\ncreate function two_blank() returns text language sql as $$\nselect 'a\n\n\nb'\n$$;").await;
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "bump", &Default::default(), log).await).await, 0, "{}", p.peek());
        let written = p.files().into_iter().find(|f| f.ends_with("_bump.sql")).unwrap();
        let sql = std::fs::read_to_string(p.dir.join("migrations").join(written)).unwrap();
        assert!(sql.contains("update counters\nSET n = n + 1;") && sql.contains("select 'a\n\n\nb'"), "{sql}");
    }

    pub async fn a_history_that_does_not_replay_is_refused_before_anything_changes(mut p: P) {
        p.file("20261027100000_one.sql", "create table t (id int primary key);");
        p.file("20261027110000_two.sql", "do $$ begin if current_database() not like 'mtest%' then raise exception 'only on the app database'; end if; end $$; create table b (id int);");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("create table hack (x int); insert into hack values (42);").await;
        std::fs::remove_file(p.dir.join("migrations/20261027110000_two.sql")).unwrap();
        p.output();
        assert_eq!(p.dev().await, 1);
        assert_has!(p.output(), r"✗ 20261027110000_two\.sql fails when replayed in a shadow database, as this database ran it");
        assert_eq!(p.rows("select x from hack").await.len(), 1);
    }

    pub async fn without_a_lock_a_history_out_of_name_order_is_held_to_name_order(mut p: P) {
        p.linear();
        p.file("20261027100000_t.sql", "create table t (v int);");
        p.file("20261027130000_b.sql", "alter table t alter column v set default 2;");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        // A branch's migration, written earlier and merged later, runs after b while the lock lists it.
        p.file("20261027120000_a.sql", "alter table t alter column v set default 1;");
        p.lock(&["20261027100000_t.sql", "20261027130000_b.sql", "20261027120000_a.sql"]);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        std::fs::remove_file(p.dir.join(reylo_migrate::lock::LOCK)).unwrap();
        p.output();
        assert_eq!(p.dev().await, 1);
        assert_has!(p.output(), r"✗ This database ran its migrations in another order than a fresh database runs them, and the two end differently");
        assert_eq!(p.dry_run().await, 1);
    }

    pub async fn a_recurring_event_is_in_the_dump(mut p: P) {
        p.file("20261027100000_t.sql", "create table t (id int primary key);");
        p.file("20261027110000_nightly.sql", "create event nightly on schedule every 1 day do delete from t;");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("alter table t add column extra int").await;
        p.output();
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "extra", &Default::default(), log).await).await, 0, "{}", p.peek());
        let texts: String = p.files().iter().filter(|f| f.contains("_extra")).map(|f| std::fs::read_to_string(p.dir.join("migrations").join(f)).unwrap()).collect();
        assert!(!texts.to_lowercase().contains("drop event"), "{texts}");
    }

    pub async fn a_virtual_tables_own_tables_are_not_in_the_dump(mut p: P) {
        p.file("20261027100000_docs.sql", "create table docs (id integer primary key, body text); create virtual table docs_fts using fts5(body, content='docs', content_rowid='id');");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        let sql = reylo_migrate::db_introspect(&p.opts(), &[]).await.unwrap();
        assert!(!sql.contains("docs_fts_data"), "{sql}");
        p.exec("alter table docs add column extra int").await;
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "extra", &Default::default(), log).await).await, 0, "{}", p.peek());
    }
}

/// DuckDB's own: what its `alter table` refuses, which the plan rebuilds or works around, its types,
/// sequences and macros, and its shadows' catalog name.
mod on_duckdb {
    use super::*;
    use reylo_migrate::catalog::difference;

    async fn duckdb() -> Option<P> {
        P::new(Dialect::Duckdb).await
    }

    fn written(p: &P, name: &str) -> String {
        p.files().iter().filter(|f| f.ends_with(&format!("_{name}.sql"))).map(|f| std::fs::read_to_string(p.dir.join("migrations").join(f)).unwrap()).collect()
    }

    async fn new(p: &mut P, name: &str) -> i32 {
        let name = name.to_string();
        p.run(async |o, log| reylo_migrate::create(o, &name, &Default::default(), log).await).await
    }

    #[tokio::test(flavor = "current_thread")]
    async fn constraints_added_to_a_table_rebuild_it_keeping_its_rows() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261027100000_t.sql", "create table authors (id bigint primary key); create table books (id bigint primary key, title varchar, author_id bigint, pages integer);");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into authors values (1); insert into books values (1, 'a', 1, 10), (2, 'b', 1, 20)").await;
        // DuckDB adds no check and no foreign key to a table that has one already.
        p.schema("create table authors (id bigint primary key); create table books (id bigint primary key, title varchar unique, author_id bigint references authors (id), pages integer check (pages > 0));");
        assert_eq!(new(&mut p, "keys").await, 0, "{}", p.peek());
        let sql = written(&p, "keys");
        assert_has!(&sql, r"create temporary table reylo_copy_books as select \* from books;\ndrop table books;\ncreate table books \(");
        assert_has!(&sql, r"insert into books \(id, title, author_id, pages\) select id, title, author_id, pages from temp\.main\.reylo_copy_books;");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from books").await[0].text("n"), "2");
        assert!(p.try_rows("insert into books values (3, 'c', 1, 0)").await.is_err(), "the check holds");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_referenced_table_is_rebuilt_with_the_tables_that_reference_it() {
        let Some(mut p) = duckdb().await else { return };
        let before = "create table authors (id bigint primary key, name varchar); create table books (id bigint primary key, author_id bigint references authors (id), title varchar); create index books_title on books (title); comment on table books is 'every book'; comment on column books.title is 'as printed';";
        p.schema(before);
        p.file("20261027100000_t.sql", before);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into authors values (1, 'ada'); insert into books values (1, 1, 'notes')").await;
        // A check on authors: authors is rebuilt, and books, whose key references it, with it.
        p.schema(&before.replace("name varchar)", "name varchar check (name <> ''))"));
        assert_eq!(new(&mut p, "named").await, 0, "{}", p.peek());
        let sql = written(&p, "named");
        assert_has!(&sql, r"drop table books;\ndrop table authors;");
        assert_has!(&sql, r"create index books_title on books\(title\);");
        assert_has!(&sql, r"comment on table books is 'every book';");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select b.title, a.name from books b join authors a on a.id = b.author_id").await[0].text("title"), "notes");
        p.output();
        assert_eq!(p.dry_run().await, 0, "the index and the comments came back: {}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_table_with_an_index_changes_with_the_index_around_the_change() {
        let Some(mut p) = duckdb().await else { return };
        let before = "create table users (id bigint primary key, email varchar, nick varchar, age integer); create index users_email on users (email);";
        p.schema(before);
        p.file("20261027100000_t.sql", before);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into users values (1, 'a@b.co', 'ada', 36)").await;
        // DuckDB drops, renames and retypes no column of a table with an index.
        p.schema("create table users (id bigint primary key, email varchar, handle varchar, age bigint); create index users_email on users (email);");
        assert_eq!(new(&mut p, "handle").await, 0, "{}", p.peek());
        let sql = written(&p, "handle");
        assert_has!(&sql, r"drop index users_email;[\s\S]*alter table users rename column nick to handle;[\s\S]*alter table users alter column age set data type bigint;[\s\S]*create index users_email on users\(email\);");
        assert!(!sql.contains("drop table"), "{sql}");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select handle from users").await[0].text("handle"), "ada");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn what_alter_table_cannot_change_is_a_rebuild() {
        let Some(mut p) = duckdb().await else { return };
        let before = "create table t (id bigint primary key, name varchar, n integer, k integer unique, gone integer);";
        p.schema(before);
        p.file("20261027100000_t.sql", before);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into t values (1, 'Ada', 2, 3, 4)").await;
        // A collation, a generated column, and a drop before a key's column, which DuckDB refuses.
        p.schema("create table t (id bigint primary key, name varchar collate nocase, n integer, k integer unique, twice integer generated always as (n * 2) virtual);");
        let choices = reylo_migrate::Choices { drop: vec!["t.gone".into()], rename: vec![] };
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "rebuild", &choices, log).await).await, 0, "{}", p.peek());
        assert_has!(written(&p, "rebuild"), r"create temporary table reylo_copy_t");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select twice from t where name = 'ada'").await[0].text("twice"), "4");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn types_sequences_and_macros_are_written_compared_and_changed() {
        let Some(mut p) = duckdb().await else { return };
        let schema = "create type mood as enum ('sad', 'ok'); create type point as struct(x double, y double); create sequence ids start 100 increment 10; create table entries (id bigint default nextval('ids') primary key, feeling mood, place point); create macro double_it(x) as x * 2; create macro recent(n) as table select * from entries order by id desc limit n; create view moods as select feeling, count(*) as n from entries group by feeling;";
        p.schema(schema);
        assert_eq!(new(&mut p, "init").await, 0, "{}", p.peek());
        let sql = written(&p, "init");
        assert_has!(&sql, r"create type mood as enum\('sad', 'ok'\);");
        assert_has!(&sql, r"feeling mood");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into entries (feeling) values ('ok'), ('sad')").await;
        assert_eq!(p.rows("select min(id) as id from entries").await[0].text("id"), "100");
        p.output();
        // Values drawn from the sequence, and the file opened again, are no drift.
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), "Up to date");
        // A label, a macro's body and the view, changed: each written and run.
        p.schema(&schema.replace("('sad', 'ok')", "('sad', 'ok', 'glad')").replace("x * 2", "x + x"));
        assert_eq!(new(&mut p, "glad").await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into entries (feeling) values ('glad')").await;
        assert_eq!(p.rows("select double_it(21) as v").await[0].text("v"), "42");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        // A macro changed by hand is drift, which deploy refuses and shows.
        p.exec("create or replace macro double_it(x) as x * 3").await;
        assert_eq!(p.migrate().await, 1);
        assert_has!(p.output(), r"✗ This database was changed outside its migrations[\s\S]*macro double_it\(x\)");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_macros_parameter_defaults_are_written_from_schema_sql() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create macro add_to(a, b := 5) as a + b;");
        assert_eq!(new(&mut p, "macro").await, 0, "{}", p.peek());
        let out = p.output();
        // No catalog function holds a default, so the comparison can't see one change: it says so.
        assert_has!(&out, r"macro add_to: its parameters' defaults are not read");
        assert_has!(written(&p, "macro"), r"b := 5");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select add_to(1) as v").await[0].text("v"), "6");
        // introspect reads it from DuckDB's own export, defaults and all.
        let sql = reylo_migrate::db_introspect(&p.opts(), &[]).await.unwrap();
        assert_has!(&sql, r"(?i)b := 5");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_name_qualified_with_the_database_replays_in_a_shadow() {
        let Some(mut p) = duckdb().await else { return };
        // A shadow is attached under the target's catalog name, here the file's, `dev`.
        p.schema("create schema billing; create table billing.invoices (id bigint primary key);");
        p.file("20261027100000_t.sql", "create schema dev.billing; create table dev.billing.invoices (id bigint primary key);");
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from billing.invoices").await[0].text("n"), "0");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_migration_that_reaches_past_the_database_is_refused() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create table t (id bigint);");
        p.file("20261027100000_t.sql", "create table t (id bigint);");
        p.file("20261027110000_out.sql", "copy t to 'every_row.csv';");
        assert_eq!(p.dry_run().await, 1);
        assert_has!(p.output(), r"can't be replayed in a shadow database\n  [^\n]*copy t to");
        p.file("20261027110000_out.sql", "attach 'other.duckdb' as other;");
        assert_eq!(p.dry_run().await, 1);
        assert_has!(p.output(), r"attaches another database");
        assert!(!p.dir.join("every_row.csv").exists() && !p.dir.join("other.duckdb").exists(), "no replay wrote a file");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn init_reads_every_kind_back_as_it_replays() {
        let Some(mut p) = duckdb().await else { return };
        p.exec("create type mood as enum ('sad', 'ok'); create sequence ids; create table entries (id bigint default nextval('ids') primary key, feeling mood not null, note varchar collate nocase); create index entries_note on entries (note); create view sad as select * from entries where feeling = 'sad'; create macro tagged(t, sep := ':') as t || sep; comment on table entries is 'what happened'; create schema archive; create table archive.old (id bigint);").await;
        assert_eq!(p.run(async |o, log| reylo_migrate::init(o, &[], log).await).await, 0, "{}", p.peek());
        let schema = std::fs::read_to_string(p.dir.join("schema.sql")).unwrap();
        assert_has!(&schema, r"comment on table entries is 'what happened'");
        assert_has!(&schema, r"(?i)sep := ':'");
        p.output();
        // What init wrote replays to this database: deploy finds it as recorded, and a check agrees.
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), "Up to date");
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        let mut q = P::sibling(&p);
        std::fs::copy(p.dir.join("migrations").join("init.sql"), q.dir.join("migrations").join("init.sql")).unwrap();
        std::fs::copy(p.dir.join("schema.sql"), q.dir.join("schema.sql")).unwrap();
        assert_eq!(q.migrate().await, 0, "init.sql runs on an empty database: {}", q.peek());
        let mut a = Db::connect(&p.url, &p.dir).await.unwrap();
        let mut b = Db::connect(&q.url, &q.dir).await.unwrap();
        assert_eq!(catalog(&mut a).await.unwrap(), catalog(&mut b).await.unwrap());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_referenced_table_renamed_keeps_its_rows_and_the_keys_on_it() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261027100000_t.sql", "create table users (id bigint primary key, email varchar); create index users_email on users (email); create table posts (id bigint, user_id bigint references users (id));");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into users values (1, 'a'); insert into posts values (1, 1)").await;
        p.schema("create table accounts (id bigint primary key, email varchar); create index users_email on accounts (email); create table posts (id bigint, user_id bigint references accounts (id));");
        assert_eq!(new(&mut p, "accounts").await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select a.email from posts p join accounts a on a.id = p.user_id").await[0].text("email"), "a");
        // The key follows: a post of no account is refused.
        assert!(p.try_rows("insert into posts values (2, 99)").await.is_err());
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    async fn wipe(p: &mut P) -> i32 {
        p.run(async |o, log| reylo_migrate::db_wipe(o, false, log).await).await
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wipe_keeps_every_comment_and_empties_a_self_referencing_table() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261027100000_t.sql", "create sequence user_ids; comment on sequence user_ids is 'the ids of users'; create table users (id bigint primary key default nextval('user_ids'), name varchar); create index users_name on users (name); comment on index users_name is 'lookup'; comment on column users.name is 'as given'; create table node (id bigint primary key, parent bigint references node (id));");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into users (name) values ('a'), ('b'); insert into node values (1, null); insert into node values (2, 1)").await;
        p.output();
        // DuckDB deletes no row another of its table references in one statement: the tables are
        // made again, empty, in one transaction, with what is on them.
        assert_eq!(wipe(&mut p).await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from node").await[0].text("n"), "0");
        let comments = p.rows("select (select comment from duckdb_sequences() where sequence_name = 'user_ids') as s, (select comment from duckdb_indexes() where index_name = 'users_name') as i, (select comment from duckdb_columns() where table_name = 'users' and column_name = 'name') as c").await;
        assert_eq!([comments[0].text("s"), comments[0].text("i"), comments[0].text("c")], ["the ids of users", "lookup", "as given"]);
        p.exec("insert into users (name) values ('c')").await;
        assert_eq!(p.rows("select id from users").await[0].text("id"), "1", "the sequence starts again");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_self_referencing_tables_rebuild_says_what_duckdb_refuses() {
        let Some(mut p) = duckdb().await else { return };
        let v1 = "create table node (id bigint primary key, parent bigint references node (id), name varchar);";
        p.schema(v1);
        p.file("20261027100000_tree.sql", v1);
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into node values (1, null, 'root'); insert into node values (2, 1, 'leaf')").await;
        p.schema("create table node (id bigint primary key, parent bigint references node (id), name varchar check (name <> ''));");
        assert_eq!(new(&mut p, "named").await, 0, "{}", p.peek());
        // DuckDB checks a key on its own table against the rows it held when the transaction began:
        // a copy back of rows that reference each other fails, which the file says.
        assert_has!(written(&p, "named"), r"-- node references itself, and DuckDB checks such a key against the rows a table held when its transaction began");
        p.output();
        assert_eq!(p.migrate().await, 1);
        assert_has!(p.output(), r"failed, and nothing of it was applied");
        assert_eq!(p.rows("select count(*) as n from node").await[0].text("n"), "2", "the failed migration changed nothing");
        // Where no row references another, it runs.
        p.exec("delete from node where id = 2").await;
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from node").await[0].text("n"), "1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_renamed_table_takes_every_table_whose_key_reaches_it() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261027100000_t.sql", "create table users (id bigint primary key, email varchar); create table posts (id bigint primary key, user_id bigint references users (id)); create table replies (id bigint, post_id bigint references posts (id)); create table node (id bigint primary key, parent bigint references node (id));");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into users values (1, 'a'); insert into posts values (1, 1); insert into replies values (1, 1); insert into node values (1, null)").await;
        // A grandchild, and a table that references itself.
        p.schema("create table accounts (id bigint primary key, email varchar); create table posts (id bigint primary key, user_id bigint references accounts (id)); create table replies (id bigint, post_id bigint references posts (id)); create table tree (id bigint primary key, parent bigint references tree (id));");
        p.output();
        assert_eq!(new(&mut p, "renames").await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}\n{}", p.peek(), written(&p, "renames"));
        assert_eq!(p.rows("select count(*) as n from replies r join posts p on p.id = r.post_id join accounts a on a.id = p.user_id").await[0].text("n"), "1");
        assert!(p.try_rows("insert into tree values (3, 99)").await.is_err(), "the key holds on the new name");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_renamed_table_with_a_key_leaves_the_table_it_references_free() {
        let Some(mut p) = duckdb().await else { return };
        // A table that references another, renamed with `alter table`, leaves DuckDB holding the old
        // name in the other's dependencies, whose drop it then refuses: it is made again instead.
        p.file("20261027100000_t.sql", "create table users (id bigint primary key, name varchar); create table posts (id bigint primary key, user_id bigint references users (id));");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into users values (1, 'a'); insert into posts values (1, 1)").await;
        p.schema("create table users (id bigint primary key, name varchar); create table articles (id bigint primary key, user_id bigint references users (id));");
        assert_eq!(new(&mut p, "articles").await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        // The table it references can still be made again, and dropped.
        p.schema("create table users (id bigint primary key, name varchar check (name <> '')); create table articles (id bigint primary key, user_id bigint references users (id));");
        assert_eq!(new(&mut p, "named").await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from articles a join users u on u.id = a.user_id").await[0].text("n"), "1");
        p.output();
        assert_eq!(p.run(async |o, log| reylo_migrate::db_reset(o, false, log).await).await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn two_tables_renamed_at_once_child_first_keep_their_rows() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261027100000_t.sql", "create table users (id bigint primary key, name varchar); create table posts (id bigint primary key, user_id bigint references users (id), title varchar);");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into users values (1, 'a'); insert into posts values (1, 1, 't')").await;
        p.schema("create table accounts (id bigint primary key, name varchar, joined date); create table articles (id bigint primary key, user_id bigint references accounts (id), title varchar, body varchar);");
        let choices = reylo_migrate::Choices { drop: vec![], rename: vec![("posts".into(), "articles".into()), ("users".into(), "accounts".into())] };
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "renames", &choices, log).await).await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}\n{}", p.peek(), written(&p, "renames"));
        assert_eq!(p.rows("select a.title, u.name from articles a join accounts u on u.id = a.user_id").await[0].text("title"), "t");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_table_renamed_and_changed_at_once_keeps_its_rows() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261027100000_t.sql", "create table users (id bigint primary key, email varchar, age integer); create index users_email on users (email); create table posts (id bigint, user_id bigint references users (id));");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into users values (1, 'a', 3); insert into posts values (1, 1)").await;
        p.schema("create table accounts (id bigint primary key, email varchar, age bigint, joined date); create index users_email on accounts (email); create table posts (id bigint, user_id bigint references accounts (id));");
        let choices = reylo_migrate::Choices { drop: vec![], rename: vec![("users".into(), "accounts".into())] };
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "accounts", &choices, log).await).await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}\n{}", p.peek(), written(&p, "accounts"));
        assert_eq!(p.rows("select count(*) as n from posts p join accounts a on a.id = p.user_id").await[0].text("n"), "1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_collation_of_several_parts_reaches_the_database() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create table t (id bigint primary key, name varchar collate nocase.noaccent);");
        assert_eq!(new(&mut p, "init").await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into t values (1, 'É')").await;
        assert_eq!(p.rows("select count(*) as n from t where name = 'e'").await[0].text("n"), "1", "migration:\n{}", written(&p, "init"));
        // One part of it dropped by hand is drift.
        p.exec("create temporary table keep as select * from t; drop table t; create table t (id bigint primary key, name varchar collate nocase); insert into t select * from keep").await;
        p.output();
        assert_eq!(p.migrate().await, 1);
        assert_has!(p.output(), r"This database was changed outside its migrations");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_table_named_in_another_case_keeps_its_rows() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261027100000_t.sql", "create table Users (id bigint primary key, name varchar);");
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into users values (1, 'a')").await;
        p.schema("create table users (id bigint primary key, name varchar);");
        p.output();
        assert_eq!(p.db_push().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from users").await[0].text("n"), "1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_apps_ordinary_use_of_the_file_is_no_drift() {
        let Some(mut p) = duckdb().await else { return };
        let schema = "create sequence ids start 10; create type mood as enum ('a', 'b'); create table t (id bigint primary key default nextval('ids'), m mood, name varchar collate nocase, total decimal(10, 2), tags varchar[], meta struct(k varchar, v integer), twice bigint generated always as (id * 2) virtual); create index t_name on t (name); create view tv as select id, name from t; create macro bump(x) as x + 1; comment on table t is 'things'; create schema arch; create table arch.old (id bigint);";
        p.schema(schema);
        p.file("20261027100000_t.sql", schema);
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        // Rows, a sequence drawn from, a checkpoint, the file opened again, what a session makes for
        // itself, and another database attached and detached.
        p.exec("insert into t (m, name, total, tags, meta) values ('a', 'x', 1.5, ['q'], {'k': 'a', 'v': 1}), ('b', 'y', 2, [], null); select nextval('ids'); checkpoint;").await;
        p.exec("create temporary table scratch as select * from t; create temporary view tmpv as select 1; create temporary macro tm(x) as x; create temporary sequence ts; insert into arch.old values (1); delete from t where name = 'y'; update t set name = 'z';").await;
        let other = p.dir.join("other.duckdb");
        p.exec(&format!("attach '{}' as other; create table other.main.x (id integer); detach other;", other.display())).await;
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), "Up to date");
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_drops_every_kind_with_its_rows() {
        let Some(mut p) = duckdb().await else { return };
        let schema = "create schema arch; create sequence ids; create type mood as enum ('a', 'b'); create table a (id bigint primary key default nextval('ids'), m mood); create table b (id bigint primary key, a_id bigint references a (id)); create table c (id bigint, b_id bigint references b (id)); create table node (id bigint primary key, parent bigint references node (id)); create index b_a on b (a_id); create view av as select * from a; create macro twice(x) as x * 2; create table arch.old (id bigint primary key); create table arch.kid (old_id bigint references old (id)); comment on table a is 'the a';";
        p.schema(schema);
        p.file("20261027100000_t.sql", schema);
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into a (m) values ('a'); insert into b select 1, id from a; insert into c values (1, 1); insert into node values (1, null); insert into node values (2, 1); insert into arch.old values (1); insert into arch.kid values (1);").await;
        p.output();
        assert_eq!(p.run(async |o, log| reylo_migrate::db_reset(o, false, log).await).await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from node").await[0].text("n"), "0");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), "Up to date");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_sequence_change_goes_on_from_its_value_where_one_is_known() {
        let Some(mut p) = duckdb().await else { return };
        let v1 = "create sequence ids; create table users (id bigint primary key default nextval('ids'), name varchar);";
        p.schema(v1);
        p.file("20261027100000_t.sql", v1);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into users (name) values ('a'), ('b'), ('c')").await;
        let v2 = "create sequence ids increment by 1 maxvalue 1000000; create table users (id bigint primary key default nextval('ids'), name varchar);";
        p.schema(v2);
        p.output();
        // db push plans from this database, whose sequence gives 4 next.
        assert_eq!(p.db_push().await, 0, "{}", p.peek());
        assert!(p.try_rows("insert into users (name) values ('d')").await.is_ok(), "the next row takes a free id");
        assert_eq!(p.rows("select max(id) as id from users").await[0].text("id"), "4");
        // A migration runs on databases whose values no plan can know: it says where it starts again.
        assert_eq!(new(&mut p, "cap").await, 0, "{}", p.peek());
        assert_has!(written(&p, "cap"), r"-- DuckDB makes ids again, so it starts again at 1: where rows hold its values, start it past them");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_index_comment_survives_new_and_dev() {
        let Some(mut p) = duckdb().await else { return };
        let v1 = "create table t (id bigint primary key, email varchar, age integer); create index t_email on t (email); comment on index t_email is 'lookup';";
        p.schema(v1);
        assert_eq!(new(&mut p, "init").await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.schema(&v1.replace("age integer", "age bigint"));
        assert_eq!(new(&mut p, "age").await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select comment from duckdb_indexes() where index_name = 't_email'").await[0].text("comment"), "lookup", "{}", written(&p, "age"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_nested_type_that_loses_values_is_named_in_drop() {
        let Some(mut p) = duckdb().await else { return };
        let v1 = "create table t (id bigint primary key, s struct(a integer, b integer));";
        p.schema(v1);
        p.file("20261027100000_t.sql", v1);
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.schema("create table t (id bigint primary key, s struct(a integer));");
        p.output();
        assert_eq!(new(&mut p, "narrow").await, 1);
        assert_has!(p.output(), r"✗ t\.s changes from struct\(a integer, b integer\) to struct\(a integer\), which drops its field b[^\n]*\n\n  To accept that:\n    reylo migrate new narrow --drop t\.s");
        let choices = reylo_migrate::Choices { drop: vec!["t.s".into()], rename: vec![] };
        assert_eq!(p.run(async |o, log| reylo_migrate::create(o, "narrow", &choices, log).await).await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn introspect_and_init_replay_whatever_order_the_export_writes() {
        let Some(mut p) = duckdb().await else { return };
        // The export writes views in name order and tables before the macros they call.
        p.exec("create macro positive(x) as x > 0; create table users (id bigint primary key, score integer check (positive(score)), name varchar); create view users_enriched as select *, upper(name) as shout from users; create view active_users as select * from users_enriched where score > 1;").await;
        let sql = reylo_migrate::db_introspect(&p.opts(), &[]).await.unwrap();
        let mut fresh = Db::connect("duckdb::memory:", &p.dir).await.unwrap();
        fresh.exec(&sql).await.unwrap_or_else(|e| panic!("introspect's SQL doesn't run on an empty database: {e}\n{sql}"));
        assert_eq!(p.run(async |o, log| reylo_migrate::init(o, &[], log).await).await, 0, "{}", p.peek());
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_macro_that_tables_and_indexes_call_changes_around_them() {
        let Some(mut p) = duckdb().await else { return };
        // DuckDB records a table whose default or check calls a macro, and an index that does, as
        // depending on it, and makes neither before it: the first migration makes it first.
        let v1 = "create macro code(n) as 'c-' || n; create macro positive(x) as x > 0; create macro norm(x) as lower(x); create table items (id bigint primary key, label varchar default code(1), qty integer check (positive(qty))); create table lines (item_id bigint references items (id), note varchar); create index items_label on items (norm(label)); comment on table items is 'stock';";
        p.schema(v1);
        assert_eq!(new(&mut p, "init").await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}\n{}", p.peek(), written(&p, "init"));
        p.exec("insert into items (id, qty) values (1, 3); insert into lines values (1, 'x')").await;
        // Each body changed: the default is taken off and set back, the index made again, and the
        // table whose check calls one rebuilt around it, with the table that references it.
        p.schema(&v1.replace("'c-' || n", "'item-' || n").replace("x > 0", "x >= 0").replace("lower(x)", "upper(x)"));
        p.output();
        assert_eq!(new(&mut p, "bodies").await, 0, "{}", p.peek());
        let sql = written(&p, "bodies");
        assert_has!(&sql, r"drop table lines;\ndrop table items;\ndrop macro positive;\ncreate macro positive");
        assert_eq!(p.dev().await, 0, "{}\n{sql}", p.peek());
        assert_eq!(p.rows("select count(*) as n from lines l join items i on i.id = l.item_id").await[0].text("n"), "1");
        p.exec("insert into items (id, qty) values (2, 0)").await;
        assert_eq!(p.rows("select label from items where id = 2").await[0].text("label"), "item-1");
        assert_eq!(p.rows("select comment from duckdb_tables() where table_name = 'items'").await[0].text("comment"), "stock");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        // And reset drops each table before the macro it calls.
        assert_eq!(p.run(async |o, log| reylo_migrate::db_reset(o, false, log).await).await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from items").await[0].text("n"), "0");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn init_reads_back_a_table_whose_default_calls_a_macro() {
        let Some(mut p) = duckdb().await else { return };
        p.exec("create macro gen_code() as 'c-' || 1; create table items (id bigint primary key, code varchar default gen_code()); insert into items (id) values (1);").await;
        assert_eq!(p.run(async |o, log| reylo_migrate::init(o, &[], log).await).await, 0, "{}", p.peek());
        // What init wrote makes the database again, on an empty one.
        let mut q = P::sibling(&p);
        for f in p.files() {
            std::fs::copy(p.dir.join("migrations").join(&f), q.dir.join("migrations").join(&f)).unwrap();
        }
        std::fs::copy(p.dir.join("schema.sql"), q.dir.join("schema.sql")).unwrap();
        assert_eq!(q.migrate().await, 0, "{}", q.peek());
        q.exec("insert into items (id) values (1)").await;
        assert_eq!(q.rows("select code from items").await[0].text("code"), "c-1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn an_enum_a_check_casts_to_changes_with_the_table_around_it() {
        let Some(mut p) = duckdb().await else { return };
        let v1 = "create type mood as enum ('calm', 'busy'); create table people (id bigint primary key, m varchar check (m::mood is not null), n varchar default 'calm'::mood);";
        p.schema(v1);
        p.file("20261027100000_t.sql", v1);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into people (id, m) values (1, 'busy')").await;
        p.schema(&v1.replace("'busy')", "'busy', 'away')"));
        p.output();
        assert_eq!(new(&mut p, "away").await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}\n{}", p.peek(), written(&p, "away"));
        p.exec("insert into people (id, m) values (2, 'away')").await;
        assert_eq!(p.rows("select count(*) as n from people").await[0].text("n"), "2");
        p.output();
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_schema_named_as_the_database_file_is_refused_with_why() {
        let Some(p) = duckdb().await else { return };
        // The file dev.duckdb is the database dev: a schema dev in it makes `dev.items` ambiguous.
        p.schema("create schema dev; create table dev.dev.items (id bigint primary key);");
        let opts = p.opts();
        let mut said = Vec::new();
        let result = reylo_migrate::create(&opts, "init", &Default::default(), &mut |l: &str| said.push(l.to_string())).await;
        let told = format!("{}\n{}", result.as_ref().err().map(|e| e.0.as_str()).unwrap_or_default(), said.join("\n"));
        // As any change the planner can't write: the migration is left empty, for three-part names
        // (`dev.dev.items`), which this file reads, to be written by hand.
        assert_has!(&told, r"The schema dev has the name of the database it is in, so DuckDB reads every name written dev\.<name> as ambiguous: rename the schema or the database file: the migration is empty");
        let sql = written(&p, "init");
        assert!(!sql.lines().any(|l| !l.trim().is_empty() && !l.starts_with("--")), "{sql}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dry_run_and_deploy_agree_after_hand_rebuilds_rows_and_a_reopen() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261027100000_a.sql", "create type mood as enum ('a', 'b'); create sequence ids start 10; create table users (id bigint primary key default nextval('ids'), name varchar, m mood); create table posts (id bigint primary key, user_id bigint references users (id), title varchar); create index posts_title on posts (title);");
        p.file("20261027110000_b.sql", "alter table users add column age integer default 3; alter table users alter column name set default 'x'; drop index posts_title; alter table posts rename column title to heading;");
        p.file("20261027120000_c.sql", "create temporary table keep as select * from posts; drop table posts; create table posts (id bigint primary key, user_id bigint references users (id), heading varchar check (heading <> '')); insert into posts select * from keep; drop table keep; create index posts_heading on posts (heading); comment on table posts is 'what people wrote';");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into users (name, m) values ('a', 'a'), ('b', 'b'); insert into posts select id, id, 'h' from users; checkpoint;").await;
        p.exec("update users set age = 4; delete from posts where id = 11;").await;
        p.output();
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        assert_has!(p.output(), "Up to date");
        assert_eq!(p.dry_run().await, 0, "{}", p.peek());
        // db wipe makes each table again, the one whose check calls a macro with it.
        p.file("20261027130000_d.sql", "create macro positive(x) as x > 0; create table scores (id integer check (positive(id)), e varchar); create index scores_e on scores (lower(e));");
        assert_eq!(p.migrate().await, 0, "{}", p.peek());
        p.exec("insert into scores values (1, 'A')").await;
        assert_eq!(wipe(&mut p).await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from scores").await[0].text("n"), "0");
        assert!(p.try_rows("insert into scores values (0, 'x')").await.is_err(), "the check holds");
    }

    // DuckDB's own commands on projects of their own: files named every way, a schema of nearly
    // every kind, drift by hand, and renames meeting rebuilds and macro changes.

    /// A project whose database file is `<file>.duckdb`, which names its database.
    async fn named(file: &str) -> Option<P> {
        let mut p = duckdb().await?;
        p.url = format!("duckdb:{}", p.dir.join(format!("{file}.duckdb")).display());
        Some(p)
    }

    async fn new_with(p: &mut P, name: &str, drop: &[&str], rename: &[(&str, &str)]) -> i32 {
        let name = name.to_string();
        let choices = reylo_migrate::Choices { drop: drop.iter().map(|s| s.to_string()).collect(), rename: rename.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() };
        p.run(async |o, log| reylo_migrate::create(o, &name, &choices, log).await).await
    }

    async fn reset(p: &mut P) -> i32 {
        p.run(async |o, log| reylo_migrate::db_reset(o, false, log).await).await
    }

    async fn init(p: &mut P) -> i32 {
        p.run(async |o, log| reylo_migrate::init(o, &[], log).await).await
    }

    async fn introspect(p: &P) -> Result<String, reylo_migrate::Error> {
        reylo_migrate::db_introspect(&p.opts(), &[]).await
    }

    /// The column `v` of a query's first row.
    async fn one(p: &P, sql: &str) -> String {
        p.rows(sql).await.first().map(|r| r.text("v").to_string()).unwrap_or_default()
    }

    /// `new` then `dev`, both required to succeed.
    async fn new_and_dev(p: &mut P, name: &str) {
        let code = new(p, name).await;
        assert_eq!(code, 0, "new {name}:\n{}", p.output());
        let sql = written(p, name);
        let code = p.dev().await;
        assert_eq!(code, 0, "dev after new {name}:\n{}\nwritten:\n{sql}", p.output());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn new_asks_before_a_union_member_is_rounded() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create table t (id integer primary key, x union(n double, s varchar));");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into t values (1, 1.5::double)").await;
        p.schema("create table t (id integer primary key, x union(n integer, s varchar));");
        let code = new(&mut p, "two").await;
        assert_ne!(code, 0, "new wrote a migration that rounds 1.5 to 2 with no --drop:\n{}\n{}", p.output(), written(&p, "two"));
    }

    /// A schema of every kind of object and most of DuckDB's types.
    const SINK: &str = r#"
create schema app;
create type mood as enum ('sad', 'ok', 'happy');
create type app.addr as struct(city varchar, zip varchar);
create sequence ids start 100 increment by 5;
create sequence app.down start 0 increment by -1 minvalue -1000 maxvalue 0;
create macro twice(x) as x * 2;
create macro clamp(x, lo := 0, hi := 10) as least(greatest(x, lo), hi);
create table users (
  id bigint primary key default nextval('ids'),
  email varchar not null unique collate nocase,
  m mood default 'ok',
  tags varchar[] default [],
  scores double[3],
  meta struct(a integer, "B c" varchar),
  props map(varchar, integer),
  u union(n integer, s varchar),
  amount decimal(12, 3) default 0 check (amount >= 0),
  span interval,
  t_s timestamp_s, t_ms timestamp_ms, t_ns timestamp_ns, t timestamp, tz timestamptz, tt timetz, tn time_ns,
  big hugeint, ubig uhugeint, vi bignum, b bit, id2 uuid default uuid(), doc json, raw blob, var variant, geo geometry,
  doubled bigint generated always as (twice(id)) virtual,
  check (length(email) > 3)
);
create index users_m on users (m);
create index users_lower on users (lower(email));
create table app.people (id integer primary key, name varchar); create table app.events (id bigint primary key default nextval('app.down'), user_id integer references people (id), ts timestamptz default current_timestamp, home app.addr);
create macro app.recent(n) as table select * from app.events order by ts desc limit n;
create table "Order Items" ("Item ID" integer primary key, "select" varchar, qty integer default clamp(5));
create view active as select id, email from users where m <> 'sad';
create view app.latest as from app.recent(3);
comment on table users is 'people';
comment on column users.email is 'login';
comment on index users_m is 'by mood';
comment on sequence ids is 'user ids';
comment on type mood is 'feelings';
comment on view active is 'not sad';
comment on macro twice is 'x2';
comment on macro table app.recent is 'last n';
"#;

    /// The comparator's lines of the project's database, comments and all.
    async fn lines_of(p: &P) -> Vec<String> {
        let mut db = Db::connect(&p.url, &p.dir).await.unwrap();
        let l = catalog(&mut db).await.unwrap();
        db.close().await;
        l
    }

    const SINK_ROWS: &str = "insert into users (email, scores, tags, meta, props, u, amount) values ('aa@b.c', [1, 2, 3], ['x'], {'a': 1, 'B c': 'z'}, map {'k': 1}, 5::integer, 2.5), ('bb@c.d', [4, 5, 6], [], null, null, 'hi', 0);
     insert into app.people values (1, 'x'); insert into app.events (user_id) values (1), (1);
     insert into \"Order Items\" values (1, 'a', default);";

    #[tokio::test(flavor = "current_thread")]
    async fn the_sink_is_built_by_new_and_is_up_to_date() {
        let Some(mut p) = duckdb().await else { return };
        p.schema(SINK);
        new_and_dev(&mut p, "sink").await;
        p.exec(SINK_ROWS).await;
        assert_eq!(p.migrate().await, 0, "{}", p.output());
        assert_eq!(p.dry_run().await, 0, "{}", p.output());
        assert_eq!(new(&mut p, "again").await, 0, "{}", p.output());
        assert!(written(&p, "again").is_empty(), "new found something to do:\n{}", written(&p, "again"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn init_reads_back_the_sink_and_deploys_it_up_to_date() {
        let Some(mut p) = duckdb().await else { return };
        p.exec(SINK).await;
        p.exec(SINK_ROWS).await;
        p.exec("checkpoint").await;
        assert_eq!(init(&mut p).await, 0, "{}", p.output());
        assert_eq!(p.migrate().await, 0, "{}", p.output());
        assert_eq!(p.dry_run().await, 0, "{}", p.output());
        let introspected = introspect(&p).await.unwrap();
        let schema = std::fs::read_to_string(p.dir.join("schema.sql")).unwrap();
        assert_eq!(introspected, schema, "introspect and init disagree");
        // A database built from it is the database it was read from.
        let Some(mut q) = named("copy").await else { return };
        q.schema(&schema);
        new_and_dev(&mut q, "from_init").await;
        let (a, b) = difference(&lines_of(&p).await, &lines_of(&q).await);
        assert!(a.is_empty() && b.is_empty(), "built from init's schema.sql, the database differs:\n- {a:?}\n+ {b:?}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wipe_keeps_the_sinks_schema_whole() {
        let Some(mut p) = duckdb().await else { return };
        p.schema(SINK);
        new_and_dev(&mut p, "sink").await;
        p.exec(SINK_ROWS).await;
        let before = lines_of(&p).await;
        assert_eq!(wipe(&mut p).await, 0, "{}", p.output());
        let after = lines_of(&p).await;
        let (a, b) = difference(&before, &after);
        assert!(a.is_empty() && b.is_empty(), "wipe changed the schema:\n- {a:?}\n+ {b:?}");
        let mut x: Vec<String> = before.clone();
        let mut y: Vec<String> = after.clone();
        x.sort();
        y.sort();
        assert_eq!(x, y, "wipe changed a line the comparison leaves out (a comment)");
        assert_eq!(one(&p, "select count(*) as v from users").await, "0");
        assert_eq!(p.migrate().await, 0, "{}", p.output());
        p.exec("insert into users (email) values ('cc@d.e')").await;
        assert_eq!(one(&p, "select min(id) as v from users").await, "1", "the sequence starts again at its minimum (accepted)");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_rebuilds_the_sink() {
        let Some(mut p) = duckdb().await else { return };
        p.schema(SINK);
        new_and_dev(&mut p, "sink").await;
        p.exec(SINK_ROWS).await;
        let before = lines_of(&p).await;
        assert_eq!(reset(&mut p).await, 0, "{}", p.output());
        let (a, b) = difference(&before, &lines_of(&p).await);
        assert!(a.is_empty() && b.is_empty(), "reset changed the schema:\n- {a:?}\n+ {b:?}");
        assert_eq!(p.migrate().await, 0, "{}", p.output());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_sink_reopened_after_a_checkpoint_and_use_is_no_drift() {
        let Some(mut p) = duckdb().await else { return };
        p.schema(SINK);
        new_and_dev(&mut p, "sink").await;
        p.exec(SINK_ROWS).await;
        p.exec("select nextval('ids'); select nextval('app.down'); checkpoint;").await;
        p.exec("delete from app.events; insert into app.events (user_id) values (1); update users set amount = 7 where email = 'aa@b.c'; vacuum; analyze; checkpoint;").await;
        p.exec("create temp table scratch as select * from users; create temp macro tm() as 1;").await;
        assert_eq!(p.migrate().await, 0, "{}", p.output());
        assert_eq!(p.dry_run().await, 0, "{}", p.output());
        assert_eq!(p.status().await, 0, "{}", p.output());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn drift_by_hand_is_seen() {
        let Some(mut p) = duckdb().await else { return };
        let schema = "create sequence s; create macro m(x) as x + 1; create type mood as enum ('a', 'b');
            create table t (id bigint primary key default nextval('s'), e varchar, n integer default 3, k mood);
            create index t_e on t (e);
            create table u (id integer, k mood, v integer, extra integer);";
        p.schema(schema);
        new_and_dev(&mut p, "one").await;
        for (what, change, undo) in [
            ("an index dropped", "drop index t_e", "create index t_e on t (e)"),
            ("a default changed", "alter table t alter column n set default 4", "alter table t alter column n set default 3"),
            ("a macro's body", "create or replace macro m(x) as x + 2", "create or replace macro m(x) as x + 1"),
            ("a sequence's increment", "alter table t alter column id drop default; drop sequence s; create sequence s increment by 2; alter table t alter column id set default nextval('s')", "alter table t alter column id drop default; drop sequence s; create sequence s; alter table t alter column id set default nextval('s')"),
            ("an enum's labels", "drop type mood; create type mood as enum ('a', 'b', 'c')", "drop type mood; create type mood as enum ('a', 'b')"),
            ("a view added", "create view v as select 1 as one", "drop view v"),
            ("a column's type", "alter table u alter column k set data type varchar", "alter table u alter column k set data type mood"),
            ("a column dropped", "alter table u drop column extra", "alter table u add column extra integer"),
            ("a not null dropped", "alter table u alter column v set not null", "alter table u alter column v drop not null"),
        ] {
            p.exec(change).await;
            let code = p.migrate().await;
            let out = p.output();
            assert_ne!(code, 0, "deploy took {what} by hand for no drift:\n{out}");
            let code = p.dry_run().await;
            let out = p.output();
            assert_ne!(code, 0, "deploy --dry-run took {what} by hand for no drift:\n{out}");
            p.exec(undo).await;
            assert_eq!(p.migrate().await, 0, "after undoing {what}: {}", p.output());
        }
    }

    async fn every_command_on(file: &str) {
        let Some(mut p) = named(file).await else { return };
        p.schema("create schema inv; create sequence ids; create table inv.t (id bigint primary key default nextval('ids'), n integer); create index t_n on inv.t (n); create view inv.v as select * from inv.t; create macro twice(x) as x * 2;");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into inv.t (n) values (1), (2)").await;
        p.schema("create schema inv; create sequence ids; create table inv.t (id bigint primary key default nextval('ids'), n integer check (n > 0), note varchar); create index t_n on inv.t (n); create view inv.v as select * from inv.t; create macro twice(x) as x * 3;");
        new_and_dev(&mut p, "two").await;
        assert_eq!(one(&p, "select count(*) as v from inv.t").await, "2", "{file}: the rebuild kept the rows");
        assert_eq!(p.migrate().await, 0, "{file}: {}", p.output());
        assert_eq!(p.dry_run().await, 0, "{file}: {}", p.output());
        assert_eq!(p.status().await, 0, "{file}: {}", p.output());
        assert_eq!(p.db_push().await, 0, "{file}: {}", p.output());
        assert_eq!(wipe(&mut p).await, 0, "{file}: {}", p.output());
        assert_eq!(reset(&mut p).await, 0, "{file}: {}", p.output());
        let text = introspect(&p).await.unwrap_or_else(|e| panic!("{file}: introspect: {e}"));
        assert!(text.contains("twice"), "{file}: {text}");
        let Some(mut q) = named(file).await else { return };
        q.exec(&text).await;
        assert_eq!(init(&mut q).await, 0, "{file}: init: {}", q.output());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_database_file_named_with_a_hyphen_and_capitals() {
        every_command_on("My-App").await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_database_file_named_as_a_keyword() {
        every_command_on("order").await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_database_file_named_with_a_digit_first_and_a_space() {
        every_command_on("2026 data").await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_database_file_named_with_a_dot() {
        every_command_on("app.v2").await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_dotted_file_whose_first_part_names_a_schema_is_refused_with_why() {
        // DuckDB names the database of `analytics.prod.duckdb` `analytics`, its name up to the first dot.
        let Some(mut p) = named("analytics.prod").await else { return };
        p.schema("create schema analytics; create table analytics.events (id bigint primary key);");
        let code = new(&mut p, "one").await;
        let out = p.output();
        assert_ne!(code, 0);
        assert!(out.contains("DuckDB names a database after its file, up to the file name's first dot"), "{out}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_check_on_a_struct_field_read_by_a_dot() {
        let Some(mut p) = duckdb().await else { return };
        // DuckDB takes the check, and its duckdb_constraints() then fails for the whole database.
        p.schema("create table t (id integer primary key, meta struct(a integer), check (meta.a > 0));");
        let code = new(&mut p, "one").await;
        assert_eq!(code, 0, "{}", p.output());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_child_renamed_while_its_parent_is_rebuilt() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create table p (id integer primary key, name varchar); create table c (id integer primary key, p_id integer references p (id), note varchar); create index c_note on c (note); comment on column c.note is 'n';");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into p values (1, 'a'); insert into c values (1, 1, 'x');").await;
        p.schema("create table p (id integer primary key, name varchar check (name <> '')); create table child (id integer primary key, p_id integer references p (id), note varchar); create index c_note on child (note); comment on column child.note is 'n';");
        new_and_dev(&mut p, "two").await;
        assert_eq!(one(&p, "select count(*) as v from child").await, "1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_parent_renamed_while_its_child_gains_a_check() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create table p (id integer primary key, name varchar); create table c (id integer primary key, p_id integer references p (id), note varchar);");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into p values (1, 'a'); insert into c values (1, 1, 'x');").await;
        p.schema("create table parent (id integer primary key, name varchar); create table c (id integer primary key, p_id integer references parent (id), note varchar check (note <> ''));");
        new_and_dev(&mut p, "two").await;
        assert_eq!(one(&p, "select count(*) as v from c").await, "1");
        assert_eq!(one(&p, "select count(*) as v from parent").await, "1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_table_renamed_while_a_macro_its_check_calls_and_its_sequence_change() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create sequence s; create macro valid(c) as c <> ''; create table items (id bigint primary key default nextval('s'), code varchar check (valid(code)), qty integer); create index items_qty on items (qty); comment on index items_qty is 'q';");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into items (code, qty) values ('a', 1), ('b', 2);").await;
        p.schema("create sequence s increment by 2; create macro valid(c) as length(c) > 0; create table products (id bigint primary key default nextval('s'), code varchar check (valid(code)), qty integer); create index items_qty on products (qty); comment on index items_qty is 'q';");
        new_and_dev(&mut p, "two").await;
        assert_eq!(one(&p, "select count(*) as v from products").await, "2");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_table_in_another_schema_renamed_with_its_comments_and_a_child() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create schema \"My Shop\"; create table \"My Shop\".orders (id integer primary key, total integer); create table \"My Shop\".lines (id integer primary key, order_id integer references orders (id)); comment on table \"My Shop\".orders is 'o'; comment on column \"My Shop\".orders.total is 't';");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into \"My Shop\".orders values (1, 5); insert into \"My Shop\".lines values (1, 1);").await;
        p.schema("create schema \"My Shop\"; create table \"My Shop\".purchases (id integer primary key, total integer); create table \"My Shop\".lines (id integer primary key, order_id integer references purchases (id)); comment on table \"My Shop\".purchases is 'o'; comment on column \"My Shop\".purchases.total is 't';");
        new_and_dev(&mut p, "two").await;
        assert_eq!(one(&p, "select count(*) as v from \"My Shop\".lines").await, "1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_migration_making_a_check_on_a_struct_field_by_a_dot_deploys_and_is_recorded() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261101000000_one.sql", "create table t (id integer primary key, meta struct(a integer), check (meta.a > 0));");
        let code = p.migrate().await;
        let out = p.output();
        let history = p.try_rows("select name as v from __reylo_migrations").await.map(|r| r.iter().map(|x| x.text("v").to_string()).collect::<Vec<_>>());
        let table = p.try_rows("select count(*) as v from duckdb_tables() where table_name = 't'").await.map(|r| r[0].text("v").to_string());
        eprintln!("deploy: {code}\n{out}\nhistory: {history:?}\ntable: {table:?}");
        let again = p.migrate().await;
        let out2 = p.output();
        eprintln!("deploy again: {again}\n{out2}");
        assert_eq!((code, again), (0, 0), "deploy: {out}\nagain: {out2}\nhistory {history:?}, table {table:?}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn new_on_a_child_that_gains_a_column_while_a_macro_its_parents_check_calls_changes() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create macro valid(s) as s <> ''; create table parent (id integer primary key, code varchar check (valid(code))); create table child (id integer primary key, parent_id integer references parent (id), note varchar);");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into parent values (1, 'a'); insert into child values (1, 1, 'n');").await;
        p.schema("create macro valid(s) as length(s) > 0; create table parent (id integer primary key, code varchar check (valid(code))); create table child (id integer primary key, parent_id integer references parent (id), note varchar, extra integer default 0);");
        new_and_dev(&mut p, "two").await;
        assert_eq!(one(&p, "select count(*) as v from child").await, "1");
    }

    const TAGGED: &str = "create macro has_tag(tags, t) as list_contains(tags, t);
    create table posts (id integer primary key, tags varchar[], check (not has_tag(tags, 'spam')));
    create table tags (id integer primary key, post_id integer references posts (id), name varchar);";

    #[tokio::test(flavor = "current_thread")]
    async fn new_writes_a_schema_whose_macro_names_a_parameter_as_a_table() {
        let Some(mut p) = duckdb().await else { return };
        p.schema(TAGGED);
        new_and_dev(&mut p, "one").await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn init_reads_back_a_schema_whose_macro_names_a_parameter_as_a_table() {
        let Some(mut p) = duckdb().await else { return };
        p.exec(TAGGED).await;
        assert_eq!(init(&mut p).await, 0, "{}", p.output());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_and_wipe_a_schema_whose_macro_names_a_parameter_as_a_table() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261101000000_one.sql", TAGGED);
        assert_eq!(p.migrate().await, 0, "{}", p.output());
        p.exec("insert into posts values (1, ['a']); insert into tags values (1, 1, 'a');").await;
        assert_eq!(wipe(&mut p).await, 0, "wipe: {}", p.output());
        assert_eq!(reset(&mut p).await, 0, "reset: {}", p.output());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_referenced_table_renamed_and_given_a_check_with_a_grandchild() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create table users (id integer primary key, name varchar); create table posts (id integer primary key, user_id integer references users (id)); create table likes (id integer primary key, post_id integer references posts (id));");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into users values (1, 'a'); insert into posts values (1, 1); insert into likes values (1, 1);").await;
        p.schema("create table accounts (id integer primary key, name varchar check (name <> '')); create table posts (id integer primary key, user_id integer references accounts (id)); create table likes (id integer primary key, post_id integer references posts (id));");
        new_and_dev(&mut p, "two").await;
        assert_eq!(one(&p, "select count(*) as v from likes").await, "1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_table_renamed_with_a_column_renamed_and_its_index_following() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create table items (id integer primary key, qty integer, sku varchar, label varchar); create index items_sku on items (sku); comment on column items.qty is 'q';");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into items values (1, 2, 'a', 'l');").await;
        p.schema("create table products (id integer primary key, quantity integer, sku varchar, label varchar); create index items_sku on products (sku); comment on column products.quantity is 'q';");
        assert_eq!(new_with(&mut p, "two", &[], &[("items", "products")]).await, 0, "{}", p.output());
        assert_eq!(p.dev().await, 0, "{}\n{}", p.output(), written(&p, "two"));
        assert_eq!(one(&p, "select quantity as v from products").await, "2");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_a_schema_whose_check_reaches_a_macro_through_another() {
        let Some(mut p) = duckdb().await else { return };
        p.file("20261101000000_one.sql", "create macro is_pos(x) as x > 0; create macro ok_qty(x) as is_pos(x) and x < 100; create table t (id integer primary key, qty integer check (ok_qty(qty)));");
        assert_eq!(p.migrate().await, 0, "{}", p.output());
        p.exec("insert into t values (1, 5)").await;
        let code = reset(&mut p).await;
        let out = p.output();
        let rows = one(&p, "select count(*) as v from t").await;
        assert_eq!(code, 0, "reset: {out}\nrows left: {rows}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_rebuild_migration_rewound_by_dev_keeps_the_rows() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create table p (id integer primary key, name varchar); create table c (id integer primary key, p_id integer references p (id));");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into p values (1, 'a'), (2, 'b'); insert into c values (1, 1), (2, 2);").await;
        p.schema("create table p (id integer primary key, name varchar check (name <> '')); create table c (id integer primary key, p_id integer references p (id));");
        new_and_dev(&mut p, "two").await;
        // A branch switch: the migration is gone, and schema.sql is back where it was.
        for f in p.files() {
            if f.ends_with("_two.sql") {
                std::fs::remove_file(p.dir.join("migrations").join(f)).unwrap();
            }
        }
        p.schema("create table p (id integer primary key, name varchar); create table c (id integer primary key, p_id integer references p (id));");
        assert_eq!(p.dev().await, 0, "{}", p.output());
        assert_eq!(one(&p, "select count(*) as v from c").await, "2");
        assert_eq!(one(&p, "select count(*) as v from p").await, "2");
        assert_eq!(p.migrate().await, 0, "{}", p.output());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn db_push_of_a_macro_change_around_a_checked_family_with_rows() {
        let Some(mut p) = duckdb().await else { return };
        p.schema("create macro valid(s) as s <> ''; create table p (id integer primary key, code varchar check (valid(code))); create table c (id integer primary key, p_id integer references p (id)); create index c_p on c (p_id);");
        new_and_dev(&mut p, "one").await;
        p.exec("insert into p values (1, 'a'); insert into c values (1, 1);").await;
        p.schema("create macro valid(s) as length(s) > 0; create table p (id integer primary key, code varchar check (valid(code))); create table c (id integer primary key, p_id integer references p (id)); create index c_p on c (p_id);");
        assert_eq!(p.db_push().await, 0, "{}", p.output());
        assert_eq!(one(&p, "select count(*) as v from c").await, "1");
    }


    #[tokio::test(flavor = "current_thread")]
    async fn the_catalog_reads_inside_a_transaction_that_made_a_macro() {
        let Some(p) = duckdb().await else { return };
        // As db push reads it before it commits.
        let mut a = Db::connect("duckdb::memory:", &p.dir).await.unwrap();
        a.exec("create macro old_m(a, b := 2) as a + b; create table t (x integer)").await.unwrap();
        a.exec("begin").await.unwrap();
        a.exec("create macro new_m(a) as a * 2; drop table t; create table t (x integer, y integer)").await.unwrap();
        let lines = catalog(&mut a).await.unwrap();
        a.exec("commit").await.unwrap();
        assert!(lines.iter().any(|l| l.contains("new_m")), "{lines:?}");
        let s = reylo_migrate::diff::duckdb::read(&mut a).await.unwrap();
        assert!(s.objects.keys().any(|k| k.contains("new_m")));
        // And a script of nothing but a comment runs.
        a.exec("-- only a comment\n").await.unwrap();
        a.exec("").await.unwrap();
        a.exec("begin; commit;").await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_schema_named_memory_migrates() {
        // Every shadow is an in-memory database, which DuckDB opens as `memory`.
        let Some(mut p) = duckdb().await else { return };
        let v1 = "create schema memory; create table memory.notes (id integer primary key, body varchar);";
        p.file("20261101000000_init.sql", v1);
        p.schema(v1);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.schema("create schema memory; create table memory.notes (id integer primary key, body varchar, at_ms bigint);");
        new_and_dev(&mut p, "at").await;
        // One named like a database DuckDB has open in every file is refused, with why.
        p.schema("create schema memory; create table memory.notes (id integer primary key, body varchar, at_ms bigint); create schema system;");
        p.output();
        new(&mut p, "system").await;
        assert_has!(p.output(), r"The schema system has the name of DuckDB's database system, so DuckDB reads every name written system\.<name> as ambiguous: rename the schema");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_macro_changes_while_a_column_defaulting_to_it_is_dropped() {
        let Some(mut p) = duckdb().await else { return };
        let v1 = "create macro m() as 1; create table t (id integer primary key, k integer default m());";
        p.file("20261101000000_init.sql", v1);
        p.schema(v1);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into t (id) values (1)").await;
        p.schema("create macro m() as 2; create table t (id integer primary key);");
        assert_eq!(new_with(&mut p, "drop_k", &["t.k"], &[]).await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}\n{}", p.peek(), written(&p, "drop_k"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wipe_empties_a_schema_of_every_kind() {
        let Some(mut p) = duckdb().await else { return };
        let v1 = "create macro ok(x) as x > 0; create type mood as enum ('a', 'b'); create sequence s start with 100; create schema billing;
          create table users (id bigint primary key default nextval('s'), boss bigint references users (id), name varchar check (ok(length(name))), m mood default 'a');
          create table invoices (id bigint primary key, user_id bigint references users (id), total decimal(10,2)); create table billing.audit (id integer, at_ms bigint default nextval('s'));
          create index users_name on users (name); comment on table users is 'people'; comment on column users.name is 'full'; comment on index users_name is 'by name';
          create view people as select * from users;";
        p.file("20261101000000_init.sql", v1);
        p.schema(v1);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into users (name) values ('ada'); insert into users (name, boss) values ('bob', 100); insert into invoices values (1, 101, 2.5); insert into billing.audit (id) values (1)").await;
        assert_eq!(wipe(&mut p).await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from users").await[0].text("n"), "0");
        assert_eq!(p.status().await, 0, "{}", p.peek());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn use_of_a_schema_replays() {
        let refused: Vec<Option<String>> = reylo_migrate::sql::split_statements("create schema analytics; use analytics; create table events (id integer)", Dialect::Duckdb).iter().map(|s| reylo_migrate::sql::escapes_shadow(s, Dialect::Duckdb)).collect();
        assert!(refused.iter().all(|r| r.is_none()), "{refused:?}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_referenced_table_renamed_and_widened_in_one_migration() {
        let Some(mut p) = duckdb().await else { return };
        let v1 = "create table people (id integer primary key, name varchar, email varchar); create index people_email on people (email); comment on table people is 'everyone'; create table posts (id integer, author integer references people (id));";
        p.file("20261101000000_init.sql", v1);
        p.schema(v1);
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        p.exec("insert into people values (1, 'ada', 'a@b'); insert into posts values (1, 1)").await;
        p.schema("create table persons (id integer primary key, name varchar, email varchar, age integer); create index people_email on persons (email); comment on table persons is 'everyone'; create table posts (id integer, author integer references persons (id));");
        assert_eq!(new_with(&mut p, "persons", &[], &[("people", "persons")]).await, 0, "{}", p.peek());
        assert_eq!(p.dev().await, 0, "{}", p.peek());
        assert_eq!(p.rows("select count(*) as n from persons").await[0].text("n"), "1");
        assert_eq!(p.status().await, 0, "{}", p.peek());
    }
}
