//! The modules `reylo generate rust-sqlx` writes, written by a build script instead, so that no
//! generated file is committed:
//!
//! ```no_run
//! // build.rs
//! fn main() {
//!     reylo_build::rust_sqlx().input("src/queries").out("queries.rs").run().unwrap();
//! }
//! ```
//!
//! ```ignore
//! // src/main.rs
//! mod queries {
//!     include!(concat!(env!("OUT_DIR"), "/queries.rs"));
//! }
//! ```
//!
//! Each input is read from the crate's directory, `src/` where none is given, and every module
//! goes into the one file in Cargo's `OUT_DIR`, a module per directory under its input. Cargo runs the script
//! again when a `.sql` file, the project file, a `.env` file or the project's migrations change.

pub use reylo_codegen::generate::{Error, Generator, Written};

/// The modules of sqlx queries, as `#[reylo::query]` writes them, for a build script.
pub fn rust_sqlx() -> Generator {
    reylo_codegen::generate::rust_sqlx().in_build_script()
}
