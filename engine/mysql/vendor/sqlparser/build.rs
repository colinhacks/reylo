// The hash of this copy's sources, which the MySQL engine's fingerprint combines with core's and its
// own (`reylo_engine_mysql::FINGERPRINT`), so that a change here changes what it is keyed by.
fn main() {
    reylo_engine_build::emit("REYLO_SQLPARSER_HASH", &["build.rs", "src"]);
}
