//! Reylo's analysis engine for DuckDB, over the core every database shares
//! ([`reylo_engine_core`]): the types DuckDB's prepared statement reports ([`duckdb`]), with
//! whether each column may be NULL and how many rows the statement returns read off a
//! `sqlparser` tree of it and the catalog ([`nulls`], [`catalog`]).
//!
//! DuckDB runs inside the app, so its host is the app's own DuckDB: the loader answers the
//! engine's needs through `@duckdb/node-api` (src/duckdb.ts), whose prepared statement describes
//! each statement, and the oracle through DuckDB's C API. The engine itself never opens a
//! database.

pub mod api;
pub mod catalog;
pub mod duckdb;
pub mod host;
pub mod nulls;
pub mod types;

pub use api::Duckdb;

/// A fingerprint of the sources this engine was built from: core's and this crate's
/// ([`reylo_engine_core::fingerprint`]).
pub const FINGERPRINT: &str = reylo_engine_core::fingerprint!("REYLO_ENGINE_DUCKDB_HASH");
