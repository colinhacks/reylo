//! Reylo's analysis engine for Postgres, over the core every database shares
//! ([`reylo_engine_core`]): libpg_query's parse tree (the patched `pg_query` crate), the type
//! registry read from `pg_type`, the expression rules ([`infer`]), the row-count proof
//! ([`cardinality`]), the nested JSON shapes ([`jsonshape`]) and the `pgreylo` handshake
//! ([`extension`]).
//!
//! It is linked by the facade, `reylo-engine`, and the program built on it, `engine/native`, whose
//! WebAssembly component the package `@reylosql/postgres` carries; by `pgreylo`, which calls
//! [`postgres::analyze`] inside the server; and by Reylo Framework, which replays [`postgres::describe`], [`postgres::trust`] and [`postgres::catalog`]
//! through its own driver and reads the parse-tree modules directly.

pub mod analyze;
pub mod api;
pub mod cardinality;
#[cfg(test)]
mod catalog_queries;
pub mod errors;
pub mod extension;
pub mod host;
pub mod infer;
pub mod jsonshape;
pub mod levels;
pub mod pgtypes;
pub mod postgres;
mod raw;
pub mod resolution;
pub mod returning;
pub mod stack;
pub mod types;

use reylo_engine_core::{analysis, dialect, lex, schema_type, structured};

pub use api::Postgres;

/// A fingerprint of the sources this engine was built from: core's and this crate's
/// ([`reylo_engine_core::fingerprint`]). Two engines with the same fingerprint give the same
/// answer, which is how a client decides whether the engine inside a server's `pgreylo` extension
/// may answer for it ([`extension`]).
pub const FINGERPRINT: &str = reylo_engine_core::fingerprint!("REYLO_ENGINE_POSTGRES_HASH");

#[cfg(test)]
mod tests {
    /// [`FINGERPRINT`] covers every directory the engine is built from: one left out would let an
    /// artifact built before an edit to it pass for one built after.
    #[test]
    fn the_fingerprint_hashes_every_source_of_the_crate() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let build = std::fs::read_to_string(root.join("build.rs")).unwrap();
        let list = build.split("emit(\"REYLO_ENGINE_POSTGRES_HASH\", &[").nth(1).and_then(|rest| rest.split(']').next()).expect("build.rs hashes the crate's sources");
        let hashed: Vec<&str> = list.split(',').map(|entry| entry.trim().trim_matches('"')).filter(|entry| !entry.is_empty()).collect();
        for entry in std::fs::read_dir(root).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // `Cargo.toml` is always hashed, a dot-entry is a prepared input (`.patched/`), and
            // `examples/` builds no part of the engine.
            if name.starts_with('.') || name == "Cargo.toml" || name == "examples" {
                continue;
            }
            assert!(hashed.contains(&name.as_str()), "{name}/ is not in the fingerprint");
        }
    }
}
