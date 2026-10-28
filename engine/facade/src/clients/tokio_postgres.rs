//! The Postgres engine's needs answered over a `tokio-postgres` client: a describe with the
//! statement's parameter types and each column's type, table and column number, and a catalog
//! query with its parameters bound by the types the server infers for them.

use crate::{AnswerCache, Analysis, AsyncAnswerer, DbError, HostError, ReplayError};
use ::tokio_postgres::types::{ToSql, Type};
use ::tokio_postgres::Client;
use reylo_engine_core::database::Database;
use reylo_engine_postgres::Postgres;
use serde_json::{json, Value};
use std::future::Future;

/// What a Postgres describe is answered with.
pub type Describe = <Postgres as Database>::Describe;

/// The analysis of `sql`, its needs answered over `client`.
pub async fn analyze(client: &Client, sql: &str, cache: &mut AnswerCache<Describe>) -> Result<Analysis, ReplayError<HostError>> {
    crate::replay::run_async::<Postgres, _>(sql, &mut Answers(client), cache).await
}

/// The engine's needs, answered over a client.
pub struct Answers<'a>(pub &'a Client);

impl AsyncAnswerer<Describe> for Answers<'_> {
    type Error = HostError;

    fn describe(&mut self, sql: &str) -> impl Future<Output = Result<Result<Describe, DbError>, HostError>> + Send {
        let client = self.0;
        async move {
            match client.prepare(sql).await {
                Ok(stmt) => {
                    let params: Vec<u32> = stmt.params().iter().map(Type::oid).collect();
                    let columns: Vec<Value> = stmt
                        .columns()
                        .iter()
                        .map(|c| json!({ "name": c.name(), "oid": c.type_().oid(), "table": c.table_oid().unwrap_or(0), "attnum": c.column_id().map_or(0, i32::from) }))
                        .collect();
                    serde_json::from_value(json!({ "params": params, "columns": columns })).map(Ok).map_err(|e| HostError(e.to_string()))
                }
                Err(e) => match e.as_db_error() {
                    // The session's end rather than the statement's answer: the replay ends as on a
                    // lost connection, and nothing is kept as the statement's error.
                    Some(_) if ended(&e) => Err(HostError(format!("{ENDED}: {}", pg_message(&e)))),
                    Some(d) => {
                        let position = match d.position() {
                            Some(::tokio_postgres::error::ErrorPosition::Original(p)) => Some(*p as usize),
                            _ => None,
                        };
                        Ok(Err(DbError { message: d.message().to_string(), code: Some(d.code().code().to_string()), position }))
                    }
                    // Not the database's answer: the connection, which ends the replay.
                    None => Err(HostError(format!("the describe did not reach the server: {e}"))),
                },
            }
        }
    }

    fn query(&mut self, sql: &str, params: &[Value]) -> impl Future<Output = Result<Result<Vec<Vec<Value>>, DbError>, HostError>> + Send {
        let client = self.0;
        async move {
            let failed = |what: &str, e: ::tokio_postgres::Error| HostError(if ended(&e) { format!("{ENDED}: {}", pg_message(&e)) } else { format!("catalog query {what}: {}\n    {sql}", pg_message(&e)) });
            let stmt = client.prepare(sql).await.map_err(|e| failed("failed to prepare", e))?;
            let mut values: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();
            for (ty, value) in stmt.params().iter().zip(params) {
                values.push(bind(ty, value).ok_or_else(|| HostError(format!("cannot bind {value} as {ty} for the catalog query\n    {sql}")))?);
            }
            let refs: Vec<&(dyn ToSql + Sync)> = values.iter().map(|v| v.as_ref() as &(dyn ToSql + Sync)).collect();
            let rows = client.query(&stmt, &refs).await.map_err(|e| failed("failed", e))?;
            Ok(Ok(rows.iter().map(|row| (0..row.len()).map(|i| cell(row, i)).collect()).collect()))
        }
    }
}

/// One parameter of a catalog query, by the type the server inferred from the cast in its text.
fn bind(ty: &Type, value: &Value) -> Option<Box<dyn ToSql + Sync + Send>> {
    let items = || value.as_array().cloned().unwrap_or_default();
    Some(match *ty {
        Type::OID_ARRAY => Box::new(items().iter().map(|v| v.as_u64().map(|n| n as u32)).collect::<Option<Vec<u32>>>()?),
        Type::INT2_ARRAY => Box::new(items().iter().map(|v| v.as_i64().map(|n| n as i16)).collect::<Option<Vec<i16>>>()?),
        Type::INT4_ARRAY => Box::new(items().iter().map(|v| v.as_i64().map(|n| n as i32)).collect::<Option<Vec<i32>>>()?),
        Type::INT8_ARRAY => Box::new(items().iter().map(|v| v.as_i64()).collect::<Option<Vec<i64>>>()?),
        Type::TEXT_ARRAY | Type::NAME_ARRAY | Type::VARCHAR_ARRAY => Box::new(items().iter().map(|v| v.as_str().map(String::from)).collect::<Option<Vec<String>>>()?),
        Type::TEXT | Type::VARCHAR | Type::NAME => Box::new(value.as_str().map(String::from).unwrap_or_else(|| value.to_string())),
        Type::OID => Box::new(value.as_u64()? as u32),
        Type::INT2 => Box::new(value.as_i64()? as i16),
        Type::INT4 => Box::new(value.as_i64()? as i32),
        Type::INT8 => Box::new(value.as_i64()?),
        Type::BOOL => Box::new(value.as_bool()?),
        _ => return None,
    })
}

/// One value of a catalog row as JSON, by its column's type.
fn cell(row: &::tokio_postgres::Row, i: usize) -> Value {
    fn array<T: Into<Value>>(items: Option<Vec<Option<T>>>) -> Option<Value> {
        items.map(|items| Value::Array(items.into_iter().map(|v| v.map_or(Value::Null, Into::into)).collect()))
    }
    let value = match *row.columns()[i].type_() {
        Type::BOOL => row.get::<_, Option<bool>>(i).map(Value::from),
        Type::INT2 => row.get::<_, Option<i16>>(i).map(Value::from),
        Type::INT4 => row.get::<_, Option<i32>>(i).map(Value::from),
        Type::INT8 => row.get::<_, Option<i64>>(i).map(Value::from),
        Type::OID => row.get::<_, Option<u32>>(i).map(Value::from),
        Type::TEXT | Type::VARCHAR | Type::NAME | Type::BPCHAR | Type::CHAR => row.get::<_, Option<String>>(i).map(Value::from),
        Type::TEXT_ARRAY | Type::VARCHAR_ARRAY | Type::NAME_ARRAY => array(row.get::<_, Option<Vec<Option<String>>>>(i)),
        Type::INT2_ARRAY => array(row.get::<_, Option<Vec<Option<i16>>>>(i)),
        Type::INT4_ARRAY => array(row.get::<_, Option<Vec<Option<i32>>>>(i)),
        Type::INT8_ARRAY => array(row.get::<_, Option<Vec<Option<i64>>>>(i)),
        Type::OID_ARRAY => array(row.get::<_, Option<Vec<Option<u32>>>>(i)),
        Type::BOOL_ARRAY => array(row.get::<_, Option<Vec<Option<bool>>>>(i)),
        // What the server-side extension answers with: its version, a whole analysis and a round's needs.
        Type::JSON | Type::JSONB => row.get::<_, Option<Value>>(i),
        ref other => panic!("a catalog query answered a column of type {other}, which no host binds"),
    };
    value.unwrap_or(Value::Null)
}

/// How a host error that is the session's end opens: the server sent a FATAL or PANIC error, or one
/// of the connection-exception class 08, and closes the connection (a restart, an administrator's
/// `pg_terminate_backend`, a database dropped).
pub const ENDED: &str = "the server ended the session";

/// Whether `e` is the server ending the session, rather than its answer to what it was asked.
pub fn ended(e: &::tokio_postgres::Error) -> bool {
    e.as_db_error().is_some_and(|d| matches!(d.severity(), "FATAL" | "PANIC") || d.code().code().starts_with("08"))
}

fn pg_message(e: &::tokio_postgres::Error) -> String {
    e.as_db_error().map(|d| format!("{} ({})", d.message(), d.code().code())).unwrap_or_else(|| e.to_string())
}
