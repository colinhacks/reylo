//! The dialects the engine reads: the name a request carries, and the placeholder rules the lexer
//! applies, which are settled before any database's parser runs. Each dialect is read by its own
//! database's engine: Postgres with libpg_query, SQLite through SQLite's own resolved tree, MySQL
//! from the server's prepare metadata and a `sqlparser` tree, and DuckDB from its prepared
//! statement's types and a `sqlparser` tree.

use crate::lex::Target;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dialect {
    Postgres,
    Sqlite,
    Mysql,
    Duckdb,
}

impl Dialect {
    pub fn name(self) -> &'static str {
        match self {
            Dialect::Postgres => "postgres",
            Dialect::Sqlite => "sqlite",
            Dialect::Mysql => "mysql",
            Dialect::Duckdb => "duckdb",
        }
    }

    /// The placeholder form an analysis hands the statement back in, the one the driver binds.
    pub fn binds(self) -> Target {
        match self {
            Dialect::Postgres | Dialect::Duckdb => Target::Dollar,
            Dialect::Sqlite | Dialect::Mysql => Target::Question,
        }
    }
}
