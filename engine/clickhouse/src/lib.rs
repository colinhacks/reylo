//! Reylo's analysis engine for ClickHouse, over the core every database shares
//! ([`reylo_engine_core`]). The server describes a statement itself: `DESCRIBE (…)` names each
//! result column's type without running anything, and the type says whether a value may be NULL
//! (`Nullable(T)`) and what an array, a tuple or a map holds, so the engine infers neither
//! ([`types`]). A parameter is written with its type, `{name:Type}`, so the engine infers none of
//! those either; what it settles is that no parameter's value can change a result's type, which
//! ClickHouse otherwise allows ([`clickhouse`]). How many rows a statement returns it reads from
//! the server's query tree ([`tree`]).
//!
//! Its hosts are `reylo-engine` (`engine/native`), which the oracle links and whose program the
//! `reylo` binary serves. Every need is a query: ClickHouse's describe is one.

pub mod api;
pub mod clickhouse;
pub mod host;
pub mod tree;
pub mod types;

pub use api::Clickhouse;

/// A fingerprint of the sources this engine was built from: core's and this crate's
/// ([`reylo_engine_core::fingerprint`]).
pub const FINGERPRINT: &str = reylo_engine_core::fingerprint!("REYLO_ENGINE_CLICKHOUSE_HASH");
