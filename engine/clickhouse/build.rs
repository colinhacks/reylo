// The hash of this crate's sources, which its fingerprint combines with core's (`src/lib.rs`).
fn main() {
    reylo_engine_build::emit("REYLO_ENGINE_CLICKHOUSE_HASH", &["build.rs", "src"]);
}
