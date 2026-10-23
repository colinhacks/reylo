//! Reylo's analysis engine for Postgres, over the core every database shares
//! ([`reylo_engine_core`]): libpg_query's parse tree (the patched `pg_query` crate), the type
//! registry read from `pg_type`, the expression rules ([`infer`]), the row-count proof
//! ([`cardinality`]), the nested JSON shapes ([`jsonshape`]) and the `pgreylo` handshake
//! ([`extension`]).
//!
//! Three hosts link it: its WebAssembly artifact (`engine/wasm/postgres`, the package
//! `@reylosql/postgres`), `pgreylo`, which calls [`postgres::analyze`] inside the server, and Reylo
//! Framework, which replays [`postgres::describe`], [`postgres::trust`] and [`postgres::catalog`]
//! through its own driver and reads the parse-tree modules directly.

pub mod analyze;
pub mod api;
pub mod cardinality;
pub mod extension;
pub mod host;
pub mod infer;
pub mod jsonshape;
pub mod levels;
pub mod pgtypes;
pub mod postgres;
mod raw;
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
