// Compiles libSQL's engine (`csrc/reylo_libsql.c`, over the amalgamation the prepare step writes
// into `.libsql/`) for the `cli` feature, whose shadow databases and development files run on it
// where a project's SQL is libSQL's own. Without the feature nothing is compiled.
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_CLI").is_none() {
        return;
    }
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let amalgamation = root.join(".libsql/sqlite3-private.c");
    if !amalgamation.exists() {
        panic!("{} is missing: run `engine/build.sh --prepare` (fetches libSQL's amalgamation)", amalgamation.display());
    }
    let mut build = cc::Build::new();
    build
        .file(root.join("csrc/reylo_libsql.c"))
        .include(root.join(".libsql"))
        .flag_if_supported("-w")
        .opt_level(2)
        // libSQL's own build of it (libsql-ffi's), but for loadable extensions, which the
        // migrations never load.
        .define("SQLITE_CORE", None)
        .define("SQLITE_DEFAULT_FOREIGN_KEYS", "1")
        .define("SQLITE_ENABLE_API_ARMOR", None)
        .define("SQLITE_ENABLE_COLUMN_METADATA", None)
        .define("SQLITE_ENABLE_DBSTAT_VTAB", None)
        .define("SQLITE_ENABLE_FTS3", None)
        .define("SQLITE_ENABLE_FTS3_PARENTHESIS", None)
        .define("SQLITE_ENABLE_FTS5", None)
        .define("SQLITE_ENABLE_JSON1", None)
        .define("SQLITE_ENABLE_RTREE", None)
        .define("SQLITE_ENABLE_STAT4", None)
        .define("SQLITE_SOUNDEX", None)
        .define("SQLITE_THREADSAFE", "1")
        .define("SQLITE_USE_URI", None)
        .define("SQLITE_OMIT_LOAD_EXTENSION", None)
        .define("HAVE_USLEEP", "1");
    if env::var("TARGET").unwrap().contains("linux") {
        build.define("_GNU_SOURCE", None);
    }
    build.compile("reylo_libsql");
    println!("cargo:rerun-if-changed=csrc/reylo_libsql.c");
    println!("cargo:rerun-if-changed=.libsql/sqlite3-private.c");
}
