//! A linear history's lock under git: two branches that each add a migration conflict on
//! `migrations/lock.yaml`, and a bare `reylo migrate rebase` resolves the conflict, whether the
//! branch merges the main branch in or is rebased onto it, moving only this branch's migrations.
//! No database is involved: `rebase` only renames files and writes the lock.

use reylo_migrate::lock::{self, State};
use reylo_migrate::Options;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// git in `dir`, away from this machine's own settings (its signing, its editor, its line endings).
fn git(dir: &Path, args: &[&str]) -> (bool, String) {
    let out = Command::new("git")
        .args(["-c", "user.name=Reylo", "-c", "user.email=test@reylo.dev", "-c", "init.defaultBranch=main", "-c", "merge.conflictStyle=merge"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_EDITOR", "true")
        .output()
        .unwrap();
    (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn ok(dir: &Path, args: &[&str]) {
    let (success, out) = git(dir, args);
    assert!(success, "git {}: {out}", args.join(" "));
}

struct Repo {
    root: PathBuf,
    opts: Options,
}

impl Repo {
    /// A repository whose main branch has one migration and its lock.
    fn new() -> Repo {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("reylo-lock-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("migrations")).unwrap();
        let opts = Options {
            url: format!("sqlite:{}", root.join("dev.db").display()),
            shadow_url: None,
            dir: root.join("migrations"),
            schema: root.join("schema.sql"),
            linear: true,
            comments: true,
            main_branch: None,
            post: None,
            root: root.clone(),
        };
        let repo = Repo { root, opts };
        ok(&repo.root, &["init", "-q"]);
        repo.add("20261101100000_users.sql", "create table users (id integer primary key);");
        repo.commit("users");
        repo
    }

    /// A migration, as `create` writes it: the file, and the lock listing every file in order.
    fn add(&self, name: &str, sql: &str) {
        std::fs::write(self.opts.dir.join(name), sql).unwrap();
        self.write_lock(&self.files());
    }

    fn write_lock(&self, names: &[String]) {
        lock::write(&self.opts.dir, names).unwrap();
    }

    fn commit(&self, message: &str) {
        ok(&self.root, &["add", "-A"]);
        ok(&self.root, &["commit", "-q", "-m", message]);
    }

    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.opts.dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".sql")).collect();
        names.sort();
        names
    }

    async fn rebase(&self) -> (i32, String) {
        let mut lines = Vec::new();
        let code = reylo_migrate::rebase(&self.opts, &[], &mut |l: &str| lines.push(l.to_string())).await.unwrap_or_else(|e| panic!("refused: {e}"));
        (code, lines.join("\n"))
    }

    fn names(&self) -> Vec<String> {
        match lock::read(&self.opts.dir) {
            State::Names(names) => names,
            other => panic!("the lock reads {other:?}"),
        }
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// main gains `theirs` while a branch holds `mine`; each appended to the lock.
fn diverge(repo: &Repo, mine: &str) {
    ok(&repo.root, &["checkout", "-q", "-b", "feature"]);
    repo.add(mine, "create table mine (id integer primary key);");
    repo.commit("mine");
    ok(&repo.root, &["checkout", "-q", "main"]);
    repo.add("20261101120000_theirs.sql", "create table theirs (id integer primary key);");
    repo.commit("theirs");
    ok(&repo.root, &["checkout", "-q", "feature"]);
}

fn assert_moved_after_theirs(repo: &Repo) {
    let files = repo.files();
    assert_eq!(files[..2], ["20261101100000_users.sql", "20261101120000_theirs.sql"]);
    assert!(files[2].ends_with("_mine.sql") && files[2].as_str() > "20261101120000", "{files:?}");
    assert_eq!(repo.names(), files);
}

#[tokio::test(flavor = "current_thread")]
async fn a_merge_of_main_is_resolved_by_moving_this_branchs_migration() {
    let repo = Repo::new();
    diverge(&repo, "20261101110000_mine.sql");
    let (merged, out) = git(&repo.root, &["merge", "-q", "main"]);
    assert!(!merged, "two branches that each add a migration conflict on the lock: {out}");
    assert_eq!(lock::read(&repo.opts.dir), State::Conflicted);
    let (code, out) = repo.rebase().await;
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("20261101110000_mine.sql →") && out.contains("resolved"), "{out}");
    assert_moved_after_theirs(&repo);
    assert_eq!(git(&repo.root, &["diff", "--name-only", "--diff-filter=U"]).1.trim(), "", "nothing is left unmerged");
    ok(&repo.root, &["commit", "-q", "--no-edit"]);
    assert_eq!(git(&repo.root, &["status", "--porcelain"]).1.trim(), "", "the resolution was staged whole");
}

#[tokio::test(flavor = "current_thread")]
async fn a_rebase_onto_main_is_resolved_by_moving_this_branchs_migration() {
    let repo = Repo::new();
    diverge(&repo, "20261101110000_mine.sql");
    let (rebased, out) = git(&repo.root, &["rebase", "main"]);
    assert!(!rebased, "the replayed commit conflicts on the lock: {out}");
    let (code, out) = repo.rebase().await;
    assert_eq!(code, 0, "{out}");
    assert_moved_after_theirs(&repo);
    ok(&repo.root, &["rebase", "--continue"]);
    assert_eq!(git(&repo.root, &["log", "--format=%s"]).1.lines().collect::<Vec<_>>(), ["mine", "theirs", "users"]);
}

#[tokio::test(flavor = "current_thread")]
async fn a_migration_already_after_main_keeps_its_name() {
    let repo = Repo::new();
    diverge(&repo, "20261101130000_mine.sql");
    assert!(!git(&repo.root, &["merge", "-q", "main"]).0);
    let (code, out) = repo.rebase().await;
    assert_eq!(code, 0, "{out}");
    assert_eq!(repo.files(), ["20261101100000_users.sql", "20261101120000_theirs.sql", "20261101130000_mine.sql"]);
    assert_eq!(repo.names(), repo.files());
}

#[tokio::test(flavor = "current_thread")]
async fn on_main_the_merged_branch_moves() {
    let repo = Repo::new();
    diverge(&repo, "20261101110000_mine.sql");
    ok(&repo.root, &["checkout", "-q", "main"]);
    assert!(!git(&repo.root, &["merge", "-q", "feature"]).0);
    let mut opts = repo.opts.clone();
    opts.main_branch = Some("main".into());
    let mut lines = Vec::new();
    assert_eq!(reylo_migrate::rebase(&opts, &[], &mut |l: &str| lines.push(l.to_string())).await.unwrap(), 0);
    assert_moved_after_theirs(&repo);
}

#[tokio::test(flavor = "current_thread")]
async fn a_lock_resolved_by_hand_out_of_name_order_renames_what_follows() {
    let repo = Repo::new();
    diverge(&repo, "20261101110000_mine.sql");
    assert!(!git(&repo.root, &["merge", "-q", "main"]).0);
    // Both sides kept, main's first, as an editor's "accept both" writes them.
    repo.write_lock(&["20261101100000_users.sql", "20261101120000_theirs.sql", "20261101110000_mine.sql"].map(String::from));
    ok(&repo.root, &["add", "-A"]);
    ok(&repo.root, &["commit", "-q", "--no-edit"]);
    let (code, out) = repo.rebase().await;
    assert_eq!(code, 0, "{out}");
    assert_moved_after_theirs(&repo);
}

#[tokio::test(flavor = "current_thread")]
async fn a_missing_lock_is_written_and_a_file_it_lacks_is_added() {
    let repo = Repo::new();
    std::fs::remove_file(repo.opts.dir.join("lock.yaml")).unwrap();
    let (code, out) = repo.rebase().await;
    assert_eq!(code, 0, "{out}");
    assert_eq!(repo.names(), ["20261101100000_users.sql"]);
    // Written by hand, so not in the lock, and older than what it lists.
    std::fs::write(repo.opts.dir.join("20261101090000_hand.sql"), "create table hand (id integer primary key);").unwrap();
    let (code, out) = repo.rebase().await;
    assert_eq!(code, 0, "{out}");
    let files = repo.files();
    assert!(files[1].ends_with("_hand.sql") && files[1].as_str() > "20261101100000", "{files:?}");
    assert_eq!(repo.names(), files);
}
