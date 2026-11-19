// The hash of this copy's sources, which the MySQL engine's fingerprint combines with core's and its
// own (`reylo_engine_mysql::FINGERPRINT`), so that a change here changes what it is keyed by.
//
// The hash is the engine crates' own (engine/build-support), written here rather than taken from
// that crate: this copy keeps the version of what it copies across Reylo's releases, so its manifest
// names no crate of the workspace, whose version would move it at every release.
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("a build script runs under cargo"));
    println!("cargo:rustc-env=REYLO_SQLPARSER_HASH={:016x}", source_hash(&root, &["build.rs", "src"]));
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
}

/// FNV-1a over `Cargo.toml` as written (`Cargo.toml.orig` in a package) and each of `entries`, file
/// by file in name order, a carriage return skipped: the same on every platform and toolchain.
fn source_hash(root: &Path, entries: &[&str]) -> u64 {
    let mut files: Vec<PathBuf> = vec![root.join("Cargo.toml")];
    for entry in entries {
        let path = root.join(entry);
        if path.is_file() {
            files.push(path);
            continue;
        }
        let mut found = Vec::new();
        walk(&path, &mut found);
        found.sort();
        files.extend(found);
    }
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            if *b == b'\r' {
                continue;
            }
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    let manifest = root.join("Cargo.toml");
    let written = root.join("Cargo.toml.orig");
    for file in &files {
        eat(file.strip_prefix(root).unwrap_or(file).to_string_lossy().replace('\\', "/").as_bytes());
        eat(&[0]);
        let read = if *file == manifest && written.is_file() { &written } else { file };
        eat(&fs::read(read).unwrap_or_default());
        eat(&[0]);
    }
    hash
}

/// Every file under `dir` that is no dotfile and in no dot-directory.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) {
            continue;
        }
        if path.is_dir() {
            walk(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}
