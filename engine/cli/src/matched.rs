//! The `reylo` a Rust project locks. Installed once (Homebrew, the install script, cargo-binstall),
//! `reylo` run inside a Rust project runs the version the project's `Cargo.lock` locks the `reylo`
//! crate at, which its macro's engine is: where that is another version than this one, the release's
//! binary of it for this platform, fetched once into the user's cache and checked against the
//! release's `SHA256SUMS`, runs in its place. npm's `reylo` command pins its own version, and says
//! so to the binary it runs.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Set on the binary this one runs, and by npm's `reylo` command: a `reylo` started with it runs as
/// itself. A user sets it to run the installed version in a project that locks another.
pub const MATCHED: &str = "REYLO_VERSION_MATCHED";

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The name of this platform's binary in a release, as npm's platform packages name it.
const PLATFORM: Option<&str> = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
    Some("darwin-arm64")
} else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
    Some("darwin-x64")
} else if cfg!(all(target_os = "linux", target_arch = "x86_64", target_env = "musl")) {
    Some("linux-x64-musl")
} else if cfg!(all(target_os = "linux", target_arch = "aarch64", target_env = "musl")) {
    Some("linux-arm64-musl")
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Some("linux-x64")
} else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
    Some("linux-arm64")
} else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
    Some("win32-x64")
} else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
    Some("win32-arm64")
} else {
    None
};

const EXE: &str = if cfg!(windows) { "reylo.exe" } else { "reylo" };

/// Where the project in the working directory locks another version of `reylo` than this one: that
/// version's exit status, having run it with `args`. `None` runs this one: outside a Rust project,
/// in one that locks this version or holds `reylo` by path (a checkout of Reylo's own repository),
/// with [`MATCHED`] set, and for `reylo engine`, which a frontend starts and pins itself.
pub fn elsewhere(args: &[String]) -> Option<i32> {
    if std::env::var_os(MATCHED).is_some() || args.first().is_some_and(|a| a == "engine") {
        return None;
    }
    let (lock, wanted) = locked(&std::env::current_dir().ok()?)?;
    if wanted == VERSION {
        return None;
    }
    match binary(&wanted, &lock) {
        Ok(path) => Some(run(&path, args)),
        Err(why) => {
            eprintln!("reylo: {} locks reylo {wanted}, and this is reylo {VERSION}, so it runs reylo {wanted}: {why}. Install reylo {wanted}, or set {MATCHED}=1 to run reylo {VERSION}.", lock.display());
            Some(1)
        }
    }
}

/// The nearest `Cargo.lock` at or above `dir`, and the version it locks the `reylo` crate at, where
/// it locks one from a registry or git, the newest where it locks several.
fn locked(dir: &Path) -> Option<(PathBuf, String)> {
    let lock = dir.ancestors().map(|d| d.join("Cargo.lock")).find(|p| p.is_file())?;
    let text = std::fs::read_to_string(&lock).ok()?;
    let mut found: Vec<String> = Vec::new();
    for package in text.split("[[package]]").skip(1) {
        let field = |key: &str| package.lines().find_map(|l| l.trim().strip_prefix(key)?.trim().strip_prefix('=')?.trim().strip_prefix('"')?.strip_suffix('"').map(str::to_string));
        if field("name").as_deref() == Some("reylo") && field("source").is_some() {
            found.extend(field("version"));
        }
    }
    found.sort_by(|a, b| numbers(a).cmp(&numbers(b)));
    Some((lock, found.pop()?))
}

/// A version's numbers, to order versions by; a pre-release's suffix ranks it with its release.
fn numbers(version: &str) -> Vec<u64> {
    version.split(['.', '-', '+']).map_while(|part| part.parse().ok()).collect()
}

/// The binary of `version` for this platform: from the cache, else fetched from the release.
fn binary(version: &str, lock: &Path) -> Result<PathBuf, String> {
    let platform = PLATFORM.ok_or("Reylo's releases have no binary for this platform")?;
    let dir = cache()?.join("versions").join(version);
    let path = dir.join(EXE);
    if path.is_file() {
        return Ok(path);
    }
    let base = format!("{}/releases/download/v{version}", env!("CARGO_PKG_REPOSITORY"));
    let asset = format!("reylo-{platform}.tar.gz");
    eprintln!("reylo: {} locks reylo {version}: fetching it once, into {}", lock.display(), dir.display());
    let sums = String::from_utf8(fetch(&format!("{base}/SHA256SUMS"))?).map_err(|_| "the release's SHA256SUMS is not text")?;
    let expected = sums
        .lines()
        .find_map(|line| {
            let (sum, name) = line.split_once(char::is_whitespace)?;
            (name.trim().trim_start_matches('*') == asset).then(|| sum.to_ascii_lowercase())
        })
        .ok_or_else(|| format!("the release's SHA256SUMS lists no {asset}"))?;
    let archive = fetch(&format!("{base}/{asset}"))?;
    let actual: String = Sha256::digest(&archive).iter().map(|b| format!("{b:02x}")).collect();
    if actual != expected {
        return Err(format!("{asset}'s SHA-256 is {actual}, and the release's SHA256SUMS says {expected}"));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    let mut entries = tar::Archive::new(flate2::read::GzDecoder::new(archive.as_slice()));
    let mut binary = None;
    for entry in entries.entries().map_err(|e| format!("cannot read {asset}: {e}"))? {
        let mut entry = entry.map_err(|e| format!("cannot read {asset}: {e}"))?;
        if entry.path().ok().is_some_and(|p| p.file_name().is_some_and(|n| n == EXE)) {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| format!("cannot read {asset}: {e}"))?;
            binary = Some(bytes);
            break;
        }
    }
    let bytes = binary.ok_or_else(|| format!("{asset} holds no {EXE}"))?;
    // Written beside its place, then moved there, so that a run cut off leaves no half a binary.
    let partial = dir.join(format!("{EXE}.{}.partial", std::process::id()));
    std::fs::write(&partial, bytes).map_err(|e| format!("cannot write {}: {e}", partial.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755)).map_err(|e| format!("cannot make {} executable: {e}", partial.display()))?;
    }
    std::fs::rename(&partial, &path).map_err(|e| format!("cannot move {} into place: {e}", partial.display()))?;
    Ok(path)
}

/// The user's cache directory for Reylo: `~/Library/Caches/reylo` on macOS, `%LOCALAPPDATA%\reylo`
/// on Windows, else `$XDG_CACHE_HOME/reylo` or `~/.cache/reylo`.
fn cache() -> Result<PathBuf, String> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Caches"))
    } else if cfg!(windows) {
        var("LOCALAPPDATA")
    } else {
        var("XDG_CACHE_HOME").or_else(|| var("HOME").map(|h| h.join(".cache")))
    };
    Ok(base.ok_or("no cache directory: HOME is not set")?.join("reylo"))
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let mut response = ureq::get(url).call().map_err(|e| format!("cannot fetch {url}: {e}"))?;
    response.body_mut().with_config().limit(512 * 1024 * 1024).read_to_vec().map_err(|e| format!("cannot fetch {url}: {e}"))
}

/// Runs `binary` with `args` in this process's place, as far as the platform allows: on Unix it
/// replaces this process, and elsewhere this one waits for it and exits with its status.
fn run(binary: &Path, args: &[String]) -> i32 {
    let mut command = std::process::Command::new(binary);
    command.args(args).env(MATCHED, "1");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        eprintln!("reylo: cannot run {}: {error}", binary.display());
        1
    }
    #[cfg(not(unix))]
    match command.status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("reylo: cannot run {}: {error}", binary.display());
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{locked, numbers};

    #[test]
    fn a_projects_lock_names_the_version_it_runs() {
        let dir = std::env::temp_dir().join(format!("reylo-matched-{}", std::process::id()));
        let nested = dir.join("crates/app");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            dir.join("Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"reylo\"\nversion = \"0.8.2\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"00\"\n\n[[package]]\nname = \"reylo-macros\"\nversion = \"0.8.2\"\n",
        )
        .unwrap();
        // Found from a directory under the lock's.
        assert_eq!(locked(&nested).map(|(_, v)| v), Some("0.8.2".to_string()));
        // `reylo` by path, as a checkout of Reylo's own repository holds it, is no version to run.
        std::fs::write(dir.join("Cargo.lock"), "[[package]]\nname = \"reylo\"\nversion = \"0.8.2\"\n").unwrap();
        assert_eq!(locked(&nested), None);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(numbers("0.10.0") > numbers("0.9.1"));
    }
}
