//! Reylo's analysis engine for SQLite, over the core every database shares
//! ([`reylo_engine_core`]): SQLite's own resolved tree, from an amalgamation patched with the
//! engine's hooks ([`sqlite_tree`]), and the function tables and row-count proof over it
//! ([`sqlite`]). No other parser is linked: what SQLite resolves is what the analysis reads.
//!
//! Two hosts link it: the facade, `reylo-engine`, and the program built from it, `engine/native`,
//! whose WebAssembly component the package `@reylosql/sqlite` carries; and Reylo Framework, which runs [`sqlite::analyze_ddl`] over a connection of
//! its own.

pub mod api;
pub mod errors;
pub mod host;
pub mod sqlite;
pub mod sqlite_tree;

use reylo_engine_core::{analysis, dialect, fresh, lex, stack, structured, types};

pub use api::Sqlite;

/// A fingerprint of the sources this engine was built from: core's and this crate's
/// ([`reylo_engine_core::fingerprint`]).
pub const FINGERPRINT: &str = reylo_engine_core::fingerprint!("REYLO_ENGINE_SQLITE_HASH");
