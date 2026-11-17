// The hash of the code this component carries besides its database's engine: its own sources, and
// the program behind its exports with the clients it answers through (engine/native). Its `ping` reports it beside the engine's fingerprint, so that the loader's tests
// tell a committed component built from other sources (tests/fingerprint.test.ts).
fn main() {
    reylo_engine_build::emit(
        "REYLO_ENGINE_HOST_HASH",
        &["build.rs", "src", "Cargo.lock", "tokio-postgres-0.7.18.patch", "../native/Cargo.toml", "../native/src"],
    );
}
