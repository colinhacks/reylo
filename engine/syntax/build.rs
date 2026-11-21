// The hash of this crate's sources, which the engines built on it combine into their fingerprints.
fn main() {
    reylo_engine_build::emit("REYLO_ENGINE_SYNTAX_HASH", &["build.rs", "src"]);
}
