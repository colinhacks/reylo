// Compiles libSQL's engine (`csrc/libsql.c`, over the amalgamation the prepare step writes into
// `engine/migrate/.libsql/`), which the oracle's libSQL cases run on (`src/libsql.rs`), as libSQL's
// own `libsql-ffi` build compiles it, but for loadable extensions, which no case loads.
use std::env;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let libsql = root.join("../engine/migrate/.libsql");
    if !libsql.join("sqlite3-private.c").exists() {
        panic!("{} is missing: run `engine/build.sh --prepare` (fetches libSQL's amalgamation)", libsql.join("sqlite3-private.c").display());
    }
    let mut build = cc::Build::new();
    build
        .file(root.join("csrc/libsql.c"))
        .include(&libsql)
        .flag_if_supported("-w")
        .opt_level(2)
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
    build.compile("reylo_oracle_libsql");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=csrc/libsql.c");
    println!("cargo:rerun-if-changed=../engine/migrate/.libsql/sqlite3-private.c");
}
