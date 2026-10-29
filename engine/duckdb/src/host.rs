//! The engine's view of a DuckDB database ([`reylo_engine_core::host`]), whose describe is the
//! prepared statement's types ([`DuckdbDescribe`]).

use crate::api::DuckdbDescribe;
pub use reylo_engine_core::host::{as_bool, as_str, Lazy};

pub type Host<'a> = reylo_engine_core::host::Host<'a, DuckdbDescribe>;
