//! The engine's view of a ClickHouse database ([`reylo_engine_core::host`]): it asks for no
//! describe of the protocol's kind, since the server's description of a statement is a query's rows.

use reylo_engine_core::api::NoDescribe;
pub use reylo_engine_core::host::{as_str, Lazy};

pub type Host<'a> = reylo_engine_core::host::Host<'a, NoDescribe>;
