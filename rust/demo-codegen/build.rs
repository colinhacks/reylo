//! The report queries' modules, written into OUT_DIR as the crate builds.
fn main() {
    reylo_build::rust_sqlx().input("src/reports").out("reports.rs").run().unwrap();
}
