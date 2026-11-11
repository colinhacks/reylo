//! Reylo's analysis engine for a Rust host.
//!
//! The engine types a SQL statement from the database itself: its parameters, its columns and
//! their nullability, the shapes of the JSON it builds, and how many rows it returns. It never
//! opens a connection. It replays an analysis, naming the statements it needs described and the
//! catalog queries it needs run, and a host answers them ([`replay`]).
//!
//! Each database's engine is behind a feature (`postgres`, `sqlite`, `mysql`, `duckdb`). A client feature
//! (`tokio-postgres`, `rusqlite`, `mysql_async`) adds the answers over that client and an
//! `analyze` that runs a statement's whole replay on it ([`clients`]). A host that answers the
//! engine its own way implements [`Answerer`] or [`AsyncAnswerer`] and calls [`replay::run`] or
//! [`replay::run_async`] with its database's engine.
//!
//! ```ignore
//! let (client, connection) = tokio_postgres::connect(url, tokio_postgres::NoTls).await?;
//! tokio::spawn(connection);
//! let mut cache = reylo_engine::AnswerCache::default();
//! let analysis = reylo_engine::clients::tokio_postgres::analyze(&client, "select id, name from users where id = :id", &mut cache).await?;
//! assert_eq!(analysis.cardinality, "maybeOne");
//! ```

pub use reylo_engine_core::api::{Analysis, Column, DbError, Diagnostic, Param};
pub use reylo_engine_core::database::Database;
pub use reylo_engine_core::dialect::Dialect;
pub use reylo_engine_core::replay::{self, AnswerCache, Answerer, AsyncAnswerer, ReplayError};
pub use reylo_engine_core::types::{Field, Type};

/// The Postgres engine: libpg_query's parse tree over the server's describe and catalog.
#[cfg(feature = "postgres")]
pub use reylo_engine_postgres as postgres;
/// The SQLite engine: SQLite's own resolved tree, from the copy of SQLite inside it.
#[cfg(feature = "sqlite")]
pub use reylo_engine_sqlite as sqlite;
/// The MySQL engine: the server's prepare metadata, read with a parse of the statement.
#[cfg(feature = "mysql")]
pub use reylo_engine_mysql as mysql;
/// The DuckDB engine: the types DuckDB's prepared statement reports, read with a parse of the
/// statement. Its host is the app's own DuckDB, so it comes with no client here.
#[cfg(feature = "duckdb")]
pub use reylo_engine_duckdb as duckdb;

#[cfg(any(feature = "tokio-postgres", feature = "rusqlite", feature = "mysql_async"))]
pub mod clients;

/// Why a client could not answer the engine at all: a lost connection, a catalog query the
/// database refused. A refused describe is not one: the engine reads it as an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostError(pub String);

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HostError {}
