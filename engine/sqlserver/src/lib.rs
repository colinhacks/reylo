//! Reylo's analysis engine for SQL Server, over the core every database shares
//! ([`reylo_engine_core`]). The server describes a statement itself: `sp_describe_undeclared_parameters`
//! types its parameters, and `sys.dm_exec_describe_first_result_set` its result's columns, their
//! nullability included, which outer joins and aggregates already null ([`sqlserver`]). The engine
//! reads a `sqlparser` tree of the statement for what the server does not say: how many rows it
//! returns ([`cardinality`]), the computed columns that are never NULL, which the server calls
//! nullable ([`nulls`]), and which parameters may be null ([`params`]).
//!
//! Its hosts are the facade, `reylo-engine`, which the oracle links, and the program built from it,
//! `engine/native`, which the `reylo` binary serves. Every need is a catalog query: SQL Server's
//! describe is two of them.

pub mod api;
pub mod cardinality;
pub mod host;
pub mod nulls;
pub mod params;
pub mod sqlserver;
pub mod types;

use reylo_engine_core::{analysis, dialect, lex};

pub use api::Sqlserver;

/// A fingerprint of the sources this engine was built from: core's and this crate's
/// ([`reylo_engine_core::fingerprint`]).
pub const FINGERPRINT: &str = reylo_engine_core::fingerprint!("REYLO_ENGINE_SQLSERVER_HASH");
