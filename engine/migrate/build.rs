// Compiles libSQL's engine (`csrc/reylo_libsql.c`, over the amalgamation the prepare step writes
// into `.libsql/`) and the sqlean extensions it loads, as Turso Cloud loads them, for the `cli`
// feature, whose shadow databases and development files run on it where a project's SQL is
// libSQL's own. Without the feature nothing is compiled.
use std::env;
use std::path::{Path, PathBuf};

/// The sqlean sources of each extension the engine loads, as libsql-ffi's build lists them.
const SQLEAN: [&str; 9] = ["crypto", "fuzzy", "math", "stats", "text", "text/utf8", "uuid", "regexp", "regexp/pcre2"];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_CLI").is_none() {
        return;
    }
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let libsql = root.join(".libsql");
    if !libsql.join("sqlite3-private.c").exists() || !libsql.join("sqlean").exists() {
        panic!("{} is missing: run `engine/build.sh --prepare` (fetches libSQL's amalgamation)", libsql.display());
    }
    let target = env::var("TARGET").unwrap();
    let msvc = target.contains("msvc");
    // Both libraries reach the engine through the extension routines' table under this name alone.
    let common = |build: &mut cc::Build| {
        build.include(&libsql).include(libsql.join("sqlean")).warnings(false).opt_level(2).define("sqlite3_api", "reylo_libsql_api");
        if !msvc {
            build.flag("-w");
        }
    };
    let mut engine = cc::Build::new();
    common(&mut engine);
    engine
        .file(root.join("csrc/reylo_libsql.c"))
        // libSQL's own build of it (libsql-ffi's), its extensions loaded by `reylo_libsql_extra_init`.
        .define("SQLITE_CORE", None)
        .define("SQLITE_EXTRA_INIT", "reylo_libsql_extra_init")
        .define("SQLITE_DEFAULT_FOREIGN_KEYS", "1")
        .define("SQLITE_ENABLE_API_ARMOR", None)
        .define("SQLITE_ENABLE_COLUMN_METADATA", None)
        .define("SQLITE_ENABLE_DBSTAT_VTAB", None)
        .define("SQLITE_ENABLE_FTS3", None)
        .define("SQLITE_ENABLE_FTS3_PARENTHESIS", None)
        .define("SQLITE_ENABLE_FTS5", None)
        .define("SQLITE_ENABLE_JSON1", None)
        .define("SQLITE_ENABLE_LOAD_EXTENSION", "1")
        .define("SQLITE_ENABLE_RTREE", None)
        .define("SQLITE_ENABLE_STAT4", None)
        .define("SQLITE_SOUNDEX", None)
        .define("SQLITE_THREADSAFE", "1")
        .define("SQLITE_USE_URI", None)
        .define("HAVE_USLEEP", "1");
    if target.contains("linux") {
        engine.define("_GNU_SOURCE", None);
    }
    engine.compile("reylo_libsql");

    let mut sqlean = cc::Build::new();
    common(&mut sqlean);
    // The crypto extension's SHA-2 writes the message's length through a cast pointer, which gcc's
    // optimizer, holding to strict aliasing, reorders: `sha256('a')` came out wrong at -O2.
    if !msvc {
        sqlean.flag("-fno-strict-aliasing");
    }
    // PCRE2, which the regexp extension carries, takes its settings from a header included first.
    let constants = libsql.join("sqlean/regexp/constants.h");
    if msvc {
        sqlean.flag(format!("/FI{}", constants.display()));
    } else {
        sqlean.flag("-include").flag(constants.to_string_lossy().as_ref());
    }
    for dir in SQLEAN {
        sqlean.files(c_files(&libsql.join("sqlean").join(dir)));
    }
    sqlean.compile("reylo_libsql_sqlean");
    println!("cargo:rerun-if-changed=csrc/reylo_libsql.c");
    println!("cargo:rerun-if-changed=.libsql");
}

/// The `.c` files directly in `dir`, in name order.
fn c_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "c")).collect();
    files.sort();
    files
}
