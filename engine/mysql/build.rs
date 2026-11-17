// The hash of this crate's sources, which its fingerprint combines with core's and with its copy of
// `sqlparser`'s, a crate of its own (`src/lib.rs`).
fn main() {
    reylo_engine_build::emit("REYLO_ENGINE_MYSQL_HASH", &["build.rs", "src"]);
}
