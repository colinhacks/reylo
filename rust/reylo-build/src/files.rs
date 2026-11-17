//! The `.sql` files a command names: a file, a directory searched for `.sql` files, or a glob in
//! quotes, which Reylo expands itself so that `'src/**/*.sql'` reads the same in every shell.
//! `reylo analyze` and `reylo generate` read their paths with it.

use std::path::{Path, PathBuf};

/// The directories a directory's search leaves out: dependencies and build output.
const SKIPPED: &[&str] = &["node_modules", "dist", "build", "out", "target", "coverage"];

/// The `.sql` files `path` names, from `cwd`, sorted: the file itself, every `.sql` file under a
/// directory (leaving out [`SKIPPED`], hidden directories and what git ignores), or what a glob
/// matches, each directory it matches searched the same way.
pub fn sql_files(path: &str, cwd: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    if is_glob(path) {
        let pattern = cwd.join(path);
        let root = cwd.join(glob_root(path));
        // As a directory's search reads `.SQL` too, on every system.
        let options = glob::MatchOptions { case_sensitive: false, require_literal_leading_dot: true, ..Default::default() };
        let matches = glob::glob_with(&pattern.to_string_lossy(), options).map_err(|e| format!("{path} is not a glob Reylo reads: {}", e.msg))?;
        // A wildcard passes over what a directory's search leaves out, as `**` would otherwise
        // reach into `target/`.
        let left_out = |entry: &Path| entry.strip_prefix(&root).is_ok_and(|rest| rest.components().any(|c| SKIPPED.contains(&c.as_os_str().to_string_lossy().as_ref())));
        for entry in matches.flatten().filter(|entry| !left_out(entry)) {
            if entry.is_dir() {
                found.extend(search(&entry));
            } else if is_sql(&entry) {
                found.push(entry);
            }
        }
    } else {
        let full = cwd.join(path);
        if full.is_dir() {
            found = search(&full);
        } else if full.is_file() {
            found.push(full);
        } else {
            return Err(format!("{path} does not exist"));
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// Whether `path` holds a glob's wildcard, which a shell left unexpanded because it was quoted.
pub fn is_glob(path: &str) -> bool {
    path.contains(['*', '?', '['])
}

/// The directory a glob's matches are all under: its path up to the first wildcard.
pub fn glob_root(path: &str) -> &str {
    let wildcard = path.find(['*', '?', '[']).unwrap_or(path.len());
    path[..wildcard].rfind(['/', '\\']).map_or("", |slash| &path[..slash])
}

fn is_sql(path: &Path) -> bool {
    path.extension().is_some_and(|x| x.eq_ignore_ascii_case("sql"))
}

/// Every `.sql` file under `dir` that git does not ignore.
fn search(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(dir, &mut found);
    let ignored = git_ignored(dir, &found);
    found.retain(|f| !ignored.contains(f));
    found
}

/// Every `.sql` file under `dir`, leaving out the directories [`SKIPPED`] names and hidden ones but
/// `.server`, whose queries a framework keeps there.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if !SKIPPED.contains(&name.as_str()) && (!name.starts_with('.') || name == ".server") {
                walk(&path, out);
            }
        } else if kind.is_file() && is_sql(&path) {
            out.push(path);
        }
    }
}

/// The files among `files` that git ignores: none where git is missing or `dir` is in no repository.
fn git_ignored(dir: &Path, files: &[PathBuf]) -> Vec<PathBuf> {
    use std::io::Write;
    if files.is_empty() {
        return Vec::new();
    }
    let Ok(mut child) = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["check-ignore", "--stdin", "-z"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return Vec::new();
    };
    let list: Vec<u8> = files.iter().flat_map(|f| f.to_string_lossy().into_owned().into_bytes().into_iter().chain([0])).collect();
    // The list is written while git's answer is read: git writes each ignored path as it reads it,
    // and stops reading once a full pipe stops its writes.
    let writer = child.stdin.take().map(|mut stdin| std::thread::spawn(move || drop(stdin.write_all(&list))));
    let Ok(out) = child.wait_with_output() else { return Vec::new() };
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    out.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()).map(|s| PathBuf::from(String::from_utf8_lossy(s).into_owned())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("reylo-files-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        for file in ["src/a.sql", "src/users/get.sql", "src/users/notes.md", "src/posts/list.SQL", "src/target/skip.sql", "src/.hidden/skip.sql", "other/x.sql"] {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "select 1").unwrap();
        }
        dir
    }

    fn names(dir: &Path, files: Vec<PathBuf>) -> Vec<String> {
        files.iter().map(|f| f.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/")).collect()
    }

    #[test]
    fn a_directory_is_searched_leaving_out_build_output_and_hidden_ones() {
        let dir = tree();
        assert_eq!(names(&dir, sql_files("src", &dir).unwrap()), ["src/a.sql", "src/posts/list.SQL", "src/users/get.sql"]);
    }

    #[test]
    fn a_glob_is_expanded_and_the_directories_it_matches_searched() {
        let dir = tree();
        assert_eq!(names(&dir, sql_files("src/**/*.sql", &dir).unwrap()), ["src/a.sql", "src/posts/list.SQL", "src/users/get.sql"]);
        assert_eq!(names(&dir, sql_files("src/u*", &dir).unwrap()), ["src/users/get.sql"]);
    }

    #[test]
    fn a_file_is_itself_and_a_missing_path_is_refused() {
        let dir = tree();
        assert_eq!(names(&dir, sql_files("other/x.sql", &dir).unwrap()), ["other/x.sql"]);
        assert_eq!(sql_files("nowhere", &dir).unwrap_err(), "nowhere does not exist");
    }

    #[test]
    fn a_globs_root_is_its_path_up_to_the_wildcard() {
        assert_eq!(glob_root("src/**/*.sql"), "src");
        assert_eq!(glob_root("src/queries/*.sql"), "src/queries");
        assert_eq!(glob_root("*.sql"), "");
    }
}
