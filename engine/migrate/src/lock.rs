//! `migrations/lock.yaml`, kept under `history: linear`: every migration's name, in the order they
//! run. Two branches that each add a migration both append to it, so git conflicts on it when the
//! second one merges, which is the point: the conflict says, before anything is deployed, that one
//! branch's migrations must move after the other's. `rebase` resolves it from the two sides git
//! keeps in its index. Each entry is an object, so that a field can be added without changing the
//! format; nothing but the name is kept, since the history table holds the SQL each database ran.

use crate::Error;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

pub const LOCK: &str = "lock.yaml";

const HEADER: &str = "# The order this project's migrations run in (history: linear), written by reylo migrate create
# and rebase. A conflict here means another branch added a migration first: reylo migrate rebase
# moves this branch's after it and resolves the conflict.
";

#[derive(Serialize, Deserialize)]
struct LockFile {
    migrations: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    name: String,
}

/// The lock as it reads on disk.
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Missing,
    /// It holds git's conflict markers.
    Conflicted,
    Invalid(String),
    Names(Vec<String>),
}

static MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^(<<<<<<<|=======|>>>>>>>)( |$)").expect("a valid pattern"));

fn parse(text: &str) -> Result<Vec<String>, String> {
    let file: LockFile = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    Ok(file.migrations.into_iter().map(|e| e.name).collect())
}

pub fn read(dir: &Path) -> State {
    match std::fs::read_to_string(dir.join(LOCK)) {
        Err(_) => State::Missing,
        Ok(text) if MARKER.is_match(&text) => State::Conflicted,
        Ok(text) => match parse(&text) {
            Ok(names) => State::Names(names),
            Err(e) => State::Invalid(e),
        },
    }
}

pub fn render(names: &[String]) -> String {
    let mut out = format!("{HEADER}migrations:\n");
    for n in names {
        out.push_str(&format!("  - name: {n}\n"));
    }
    out
}

pub fn write(dir: &Path, names: &[String]) -> std::io::Result<()> {
    std::fs::write(dir.join(LOCK), render(names))
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).current_dir(dir).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The two sides of a conflicted lock, from git's index: this branch's entries, then the other
/// side's. Which side is this branch's follows what git is doing: a rebase or a cherry-pick replays
/// this branch's commits onto the other side, so theirs (stage 3) is this branch's; a merge brings
/// the other side into this branch, so ours (stage 2) is, unless this is the main branch itself,
/// which a feature branch is being merged into.
pub fn sides(dir: &Path, main_branch: Option<&str>) -> Result<(Vec<String>, Vec<String>), Error> {
    let unmerged = git(dir, &["ls-files", "-u", "--", LOCK]).unwrap_or_default();
    if unmerged.trim().is_empty() {
        return Err(Error(format!(
            "{LOCK} holds conflict markers, but git does not hold it as a conflict (was it added with them?): take out the markers so that each migration is listed once, then run reylo migrate rebase"
        )));
    }
    let side = |stage: u8| -> Result<Vec<String>, Error> {
        let text = git(dir, &["show", &format!(":{stage}:./{LOCK}")]).ok_or_else(|| Error(format!("git cannot read side {stage} of the conflict in {LOCK}")))?;
        parse(&text).map_err(|e| Error(format!("side {stage} of the conflict in {LOCK} does not read as a lock file: {e}")))
    };
    let (ours, theirs) = (side(2)?, side(3)?);
    let exists = |p: &str| git(dir, &["rev-parse", "--git-path", p]).is_some_and(|path| dir.join(path.trim()).exists());
    let replaying = exists("rebase-merge") || exists("rebase-apply") || exists("CHERRY_PICK_HEAD");
    let on_main = || {
        let branch = git(dir, &["symbolic-ref", "--short", "-q", "HEAD"]).unwrap_or_default();
        let main = match main_branch {
            Some(m) => m.to_string(),
            None => git(dir, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]).unwrap_or_default(),
        };
        let main = main.trim();
        !branch.trim().is_empty() && (branch.trim() == main || Some(branch.trim()) == main.split_once('/').map(|(_, b)| b))
    };
    Ok(if replaying || on_main() { (theirs, ours) } else { (ours, theirs) })
}

/// Stages what `rebase` wrote to resolve a conflict: the lock, and each renamed file at both paths.
pub fn stage(dir: &Path, paths: &[String]) -> Result<(), Error> {
    let mut args = vec!["add", "-A", "--"];
    args.extend(paths.iter().map(String::as_str));
    git(dir, &args).map(|_| ()).ok_or_else(|| Error(format!("git could not stage {}", paths.join(", "))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_it_writes() {
        let names = vec!["init.sql".to_string(), "20261028100000_users.sql".to_string()];
        let text = render(&names);
        assert_eq!(parse(&text).unwrap(), names);
        assert!(text.ends_with("migrations:\n  - name: init.sql\n  - name: 20261028100000_users.sql\n"));
        // CRLF line endings, as a checkout on Windows writes them, read the same.
        assert_eq!(parse(&text.replace('\n', "\r\n")).unwrap(), names);
        assert_eq!(parse("migrations: []\n").unwrap(), Vec::<String>::new());
        assert!(parse("migrations:\n  - 20261028100000_users.sql\n").is_err());
    }

    #[test]
    fn knows_a_conflict() {
        let dir = std::env::temp_dir().join(format!("reylo-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(read(&dir), State::Missing);
        std::fs::write(dir.join(LOCK), "migrations:\n  - name: a.sql\n<<<<<<< HEAD\n  - name: b.sql\n=======\n  - name: c.sql\n>>>>>>> other\n").unwrap();
        assert_eq!(read(&dir), State::Conflicted);
        std::fs::write(dir.join(LOCK), "migrations: {\n").unwrap();
        assert!(matches!(read(&dir), State::Invalid(_)));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
