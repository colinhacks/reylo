// The hash of the sources that connect to Postgres and MySQL and read their answers, which the
// migrations' cache keys its replays by (`SOURCE_HASH`), so that a change to how a connection reads
// a catalog is a new key. ClickHouse's are no part of it: it has no migrations.
fn main() {
    reylo_engine_build::emit("REYLO_ENGINE_HASH", &["src/postgres.rs", "src/mysql.rs"]);
}
