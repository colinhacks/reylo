//! The engine's needs answered over a database client: one module per database's client, each with
//! the [`crate::Answerer`] or [`crate::AsyncAnswerer`] for that client and an `analyze` that runs a
//! statement's whole replay on it.

#[cfg(feature = "clickhouse")]
pub mod clickhouse;
#[cfg(feature = "mysql")]
pub mod mysql_async;
#[cfg(feature = "sqlite")]
pub mod rusqlite;
#[cfg(feature = "postgres")]
pub mod tokio_postgres;
