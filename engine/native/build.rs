// The hash of the sources that connect to Postgres, MySQL and SQL Server and read their answers,
// which the migrations' cache keys its replays by (`SOURCE_HASH`), so that a change to how a
// connection reads a catalog is a new key.
fn main() {
    reylo_engine_build::emit("REYLO_ENGINE_HASH", &["src/postgres.rs", "src/mysql.rs", "src/sqlserver.rs"]);
}
