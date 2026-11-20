//! The engine's view of a SQL Server database ([`reylo_engine_core::host`]): it asks for no
//! describe, since the server's description of a statement is the rows of two catalog queries.

use reylo_engine_core::api::NoDescribe;
pub use reylo_engine_core::host::{as_str, Lazy};

pub type Host<'a> = reylo_engine_core::host::Host<'a, NoDescribe>;
