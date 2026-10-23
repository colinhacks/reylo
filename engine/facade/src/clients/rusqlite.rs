//! The SQLite engine's catalog queries answered over a `rusqlite` connection to the database
//! file. SQLite prepares inside the engine, so it never asks to describe.

use crate::{AnswerCache, Analysis, Answerer, DbError, HostError, ReplayError};
use ::rusqlite::types::{Value as Sql, ValueRef};
use ::rusqlite::Connection;
use reylo_engine_core::database::Database;
use reylo_engine_sqlite::Sqlite;
use serde_json::{json, Value};

/// What a SQLite describe is answered with: never anything.
pub type Describe = <Sqlite as Database>::Describe;

/// The analysis of `sql`, its catalog queries answered over `conn`.
pub fn analyze(conn: &Connection, sql: &str, cache: &mut AnswerCache<Describe>) -> Result<Analysis, ReplayError<HostError>> {
    crate::replay::run::<Sqlite, _>(sql, &mut Answers(conn), cache)
}

/// The engine's catalog queries, answered over a connection.
pub struct Answers<'a>(pub &'a Connection);

impl Answerer<Describe> for Answers<'_> {
    type Error = HostError;

    fn describe(&mut self, sql: &str) -> Result<Result<Describe, DbError>, HostError> {
        Err(HostError(format!("the SQLite engine asked to describe {sql}, which it prepares itself")))
    }

    fn query(&mut self, sql: &str, params: &[Value]) -> Result<Result<Vec<Vec<Value>>, DbError>, HostError> {
        let mut stmt = self.0.prepare(sql).map_err(|e| HostError(format!("catalog query: {e}")))?;
        let width = stmt.column_count();
        let bound = params.iter().map(sql_value).collect::<Vec<_>>();
        let rows = stmt
            .query_map(::rusqlite::params_from_iter(bound), |row| {
                (0..width)
                    .map(|i| {
                        Ok(match row.get_ref(i)? {
                            ValueRef::Null => Value::Null,
                            ValueRef::Integer(n) => json!(n),
                            ValueRef::Real(f) => json!(f),
                            ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                            ValueRef::Blob(_) => Value::Null,
                        })
                    })
                    .collect::<Result<Vec<Value>, ::rusqlite::Error>>()
            })
            .map_err(|e| HostError(format!("catalog query: {e}")))?
            .collect::<Result<Vec<Vec<Value>>, ::rusqlite::Error>>()
            .map_err(|e| HostError(format!("catalog query: {e}")))?;
        Ok(Ok(rows))
    }
}

/// A catalog query's parameter as SQLite binds it.
fn sql_value(value: &Value) -> Sql {
    match value {
        Value::Null => Sql::Null,
        Value::Bool(b) => Sql::Integer(i64::from(*b)),
        Value::Number(n) => n.as_i64().map(Sql::Integer).unwrap_or_else(|| Sql::Real(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => Sql::Text(s.clone()),
        other => Sql::Text(other.to_string()),
    }
}
