//! The SQL Server engine's needs answered over a `tiberius` connection. Every need is a query
//! (the describe is two catalog queries, [`reylo_engine_sqlserver::sqlserver`]), its values bound as
//! `@P1`, `@P2`, … and its first result set's values handed back as JSON.

use crate::{AnswerCache, Analysis, AsyncAnswerer, DbError, HostError, ReplayError};
use ::tiberius::{Client, ColumnData, ToSql};
use futures_io::{AsyncRead, AsyncWrite};
use reylo_engine_core::api::NoDescribe;
use reylo_engine_sqlserver::Sqlserver;
use serde_json::{json, Value};
use std::future::Future;

/// The analysis of `sql`, its needs answered over `client`.
pub async fn analyze<S: AsyncRead + AsyncWrite + Unpin + Send>(client: &mut Client<S>, sql: &str, cache: &mut AnswerCache<NoDescribe>) -> Result<Analysis, ReplayError<HostError>> {
    crate::replay::run_async::<Sqlserver, _>(sql, &mut Answers(client), cache).await
}

/// The engine's needs, answered over a connection.
pub struct Answers<'a, S: AsyncRead + AsyncWrite + Unpin + Send>(pub &'a mut Client<S>);

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncAnswerer<NoDescribe> for Answers<'_, S> {
    type Error = HostError;

    fn describe(&mut self, _sql: &str) -> impl Future<Output = Result<Result<NoDescribe, DbError>, HostError>> + Send {
        async { Err(HostError("the SQL Server engine asks for no describe".into())) }
    }

    fn query(&mut self, sql: &str, params: &[Value]) -> impl Future<Output = Result<Result<Vec<Vec<Value>>, DbError>, HostError>> + Send {
        let client = &mut *self.0;
        let (sql, params) = (sql.to_string(), params.to_vec());
        async move { rows(client, &sql, &params).await }
    }
}

/// A value bound as the type its JSON is: text, an integer, a float, a truth value, or NULL.
enum Bound {
    Text(Option<String>),
    Integer(i64),
    Float(f64),
    Truth(bool),
}

impl Bound {
    fn of(v: &Value) -> Bound {
        match v {
            Value::Null => Bound::Text(None),
            Value::Bool(b) => Bound::Truth(*b),
            Value::Number(n) => n.as_i64().map(Bound::Integer).unwrap_or_else(|| Bound::Float(n.as_f64().unwrap_or_default())),
            Value::String(s) => Bound::Text(Some(s.clone())),
            other => Bound::Text(Some(other.to_string())),
        }
    }

    fn sql(&self) -> &dyn ToSql {
        match self {
            Bound::Text(t) => t,
            Bound::Integer(n) => n,
            Bound::Float(f) => f,
            Bound::Truth(b) => b,
        }
    }
}

/// The first result set of a query, its values bound in order as `@P1`, `@P2`, …; the server's
/// error is the answer, and anything else that stops it is the host's.
pub async fn rows<S: AsyncRead + AsyncWrite + Unpin + Send>(client: &mut Client<S>, sql: &str, params: &[Value]) -> Result<Result<Vec<Vec<Value>>, DbError>, HostError> {
    let bound: Vec<Bound> = params.iter().map(Bound::of).collect();
    let refs: Vec<&dyn ToSql> = bound.iter().map(Bound::sql).collect();
    let stream = match client.query(sql, &refs).await {
        Ok(stream) => stream,
        Err(e) => return failed(e, sql),
    };
    match stream.into_first_result().await {
        Ok(rows) => Ok(Ok(rows.into_iter().map(|row| row.into_iter().map(|v| value(&v)).collect()).collect())),
        Err(e) => failed(e, sql),
    }
}

fn failed<T>(e: ::tiberius::error::Error, sql: &str) -> Result<Result<T, DbError>, HostError> {
    match e {
        ::tiberius::error::Error::Server(e) => Ok(Err(DbError { message: e.message().to_string(), code: Some(e.code().to_string()), position: None })),
        other => Err(HostError(format!("the query did not reach the server: {other}\n    {sql}"))),
    }
}

/// One value of a result as JSON: a number for an integer or a float, a truth value for a `bit`,
/// and text for the rest, a `uniqueidentifier` in lowercase and binary data as hexadecimal.
pub fn value(v: &ColumnData<'_>) -> Value {
    match v {
        ColumnData::U8(n) => json!(n),
        ColumnData::I16(n) => json!(n),
        ColumnData::I32(n) => json!(n),
        ColumnData::I64(n) => json!(n),
        ColumnData::F32(n) => json!(n),
        ColumnData::F64(n) => json!(n),
        ColumnData::Bit(b) => json!(b),
        ColumnData::String(s) => json!(s.as_deref()),
        ColumnData::Guid(g) => json!(g.map(|g| g.to_string())),
        ColumnData::Binary(b) => json!(b.as_deref().map(|b| b.iter().map(|byte| format!("{byte:02x}")).collect::<String>())),
        ColumnData::Numeric(n) => json!(n.map(|n| n.to_string())),
        ColumnData::Xml(x) => json!(x.as_deref().map(|x| x.to_string())),
        other => Value::String(format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn values_as_json() {
        assert_eq!(value(&ColumnData::I32(Some(3))), json!(3));
        assert_eq!(value(&ColumnData::Bit(Some(false))), json!(false));
        assert_eq!(value(&ColumnData::String(Some(Cow::Borrowed("decimal(10,2)")))), json!("decimal(10,2)"));
        assert_eq!(value(&ColumnData::String(None)), Value::Null);
        assert_eq!(value(&ColumnData::Binary(Some(Cow::Borrowed(&[0x0a, 0xff])))), json!("0aff"));
    }
}
