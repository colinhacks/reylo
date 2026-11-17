//! What the engine crates' build scripts share: the hash of a crate's own sources. Each crate hashes
//! only its own directory, and a database crate combines core's hash with its own in Rust
//! (`reylo_engine_core::fingerprint`), so no build script reads another package's files.

use std::fs;
use std::path::{Path, PathBuf};

/// FNV-1a over `Cargo.toml` (as written, which a package keeps as `Cargo.toml.orig`) and the
/// `entries` of the crate at `root`, a file or a directory each, file by file in name order: the
/// same on every platform and toolchain, which a std hasher does not promise. A carriage return is
/// skipped, so a checkout with CRLF line ends hashes the same.
pub fn source_hash(root: &Path, entries: &[&str]) -> u64 {
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
    // `cargo package` writes a normalized `Cargo.toml` and keeps the manifest as written in
    // `Cargo.toml.orig`: a crate built from crates.io hashes that one, so that it has the hash of
    // the same sources built here.
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

/// Every file under `dir`. A dotfile or dot-directory is never a source: macOS tar adds an
/// AppleDouble `._name` beside a file with extended attributes, a checkout on another platform
/// would hash it, and the prepared inputs (`.patched/`, `.sqlite/`) are no sources either.
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

/// For a build script: hash the crate's `entries`, hand the hash to the crate as the environment
/// variable `var` (16 hex digits), and rerun when any of them changes.
pub fn emit(var: &str, entries: &[&str]) {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("a build script runs under cargo"));
    println!("cargo:rustc-env={var}={:016x}", source_hash(&root, entries));
    println!("cargo:rerun-if-changed=Cargo.toml");
    for entry in entries {
        println!("cargo:rerun-if-changed={entry}");
    }
}
