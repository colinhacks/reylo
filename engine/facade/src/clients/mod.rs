//! The engine's needs answered over a database client: one module per client feature, each with
//! the [`crate::Answerer`] or [`crate::AsyncAnswerer`] for that client and an `analyze` that runs a
//! statement's whole replay on it.

#[cfg(feature = "mysql_async")]
pub mod mysql_async;
#[cfg(feature = "rusqlite")]
pub mod rusqlite;
#[cfg(feature = "tokio-postgres")]
pub mod tokio_postgres;
