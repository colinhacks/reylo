//! The MySQL engine's needs answered over a `mysql_async` connection: a describe from the
//! server's prepare, its column and parameter definitions as mysql2 reports them, and a catalog
//! query with its values written into the text, as mysql2's `query()` writes them.

use crate::{AnswerCache, Analysis, AsyncAnswerer, DbError, HostError, ReplayError};
use ::mysql_async::prelude::Queryable;
use ::mysql_async::{Conn, Value as My};
use reylo_engine_core::database::Database;
use reylo_engine_mysql::Mysql;
use serde_json::{json, Value};
use std::future::Future;

/// What a MySQL describe is answered with.
pub type Describe = <Mysql as Database>::Describe;

/// The analysis of `sql`, its needs answered over `conn`.
pub async fn analyze(conn: &mut Conn, sql: &str, cache: &mut AnswerCache<Describe>) -> Result<Analysis, ReplayError<HostError>> {
    crate::replay::run_async::<Mysql, _>(sql, &mut Answers(conn), cache).await
}

/// The engine's needs, answered over a connection.
pub struct Answers<'a>(pub &'a mut Conn);

impl AsyncAnswerer<Describe> for Answers<'_> {
    type Error = HostError;

    fn describe(&mut self, sql: &str) -> impl Future<Output = Result<Result<Describe, DbError>, HostError>> + Send {
        let conn = &mut *self.0;
        async move {
            match conn.prep(sql).await {
                Ok(statement) => {
                    let columns: Vec<Value> = statement.columns().iter().map(|c| json!({ "name": c.name_str(), "type": c.column_type() as u8, "flags": c.flags().bits(), "charset": c.character_set(), "length": c.column_length(), "org_table": c.org_table_str(), "org_name": c.org_name_str() })).collect();
                    let params: Vec<Value> = statement.params().iter().map(|p| json!({ "type": p.column_type() as u8, "flags": p.flags().bits(), "charset": p.character_set() })).collect();
                    conn.close(statement).await.map_err(|e| HostError(e.to_string()))?;
                    serde_json::from_value(json!({ "columns": columns, "params": params })).map(Ok).map_err(|e| HostError(e.to_string()))
                }
                Err(::mysql_async::Error::Server(e)) => Ok(Err(DbError { message: e.message, code: error_name(e.code).map(String::from), position: None })),
                Err(e) => Err(HostError(format!("the prepare did not reach the server: {e}"))),
            }
        }
    }

    fn query(&mut self, sql: &str, params: &[Value]) -> impl Future<Output = Result<Result<Vec<Vec<Value>>, DbError>, HostError>> + Send {
        let conn = &mut *self.0;
        async move {
            let text = interpolate(sql, params).ok_or_else(|| HostError(format!("cannot write {params:?} into the catalog query\n    {sql}")))?;
            let mut result = conn.query_iter(text).await.map_err(|e| HostError(format!("catalog query failed: {}\n    {sql}", message(&e))))?;
            let types: Vec<u8> = result.columns_ref().iter().map(|c| c.column_type() as u8).collect();
            let mut rows = Vec::new();
            while let Some(row) = result.next().await.map_err(|e| HostError(format!("catalog query failed: {}\n    {sql}", message(&e))))? {
                rows.push(row.unwrap().into_iter().zip(&types).map(|(value, code)| decoded(value, *code)).collect());
            }
            Ok(Ok(rows))
        }
    }
}

/// A catalog query with its values written into the text, as mysql2's `query()` writes them
/// (`sqlstring`): each `?` in turn, a string quoted with its special characters escaped.
pub fn interpolate(sql: &str, params: &[Value]) -> Option<String> {
    let mut out = String::with_capacity(sql.len());
    let mut params = params.iter();
    for c in sql.chars() {
        if c != '?' {
            out.push(c);
            continue;
        }
        match params.next()? {
            Value::Null => out.push_str("NULL"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::String(s) => out.push_str(&quote(s)),
            _ => return None,
        }
    }
    params.next().is_none().then_some(out)
}

/// A string as a MySQL literal, by `sqlstring`'s escapes.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\0' => out.push_str("\\0"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{1a}' => out.push_str("\\Z"),
            '"' => out.push_str("\\\""),
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// One value of a text result as mysql2 hands it to the host: a number for the integer and
/// floating types, a string for the rest.
pub fn decoded(value: My, code: u8) -> Value {
    match value {
        My::NULL => Value::Null,
        My::Bytes(bytes) => {
            let text = String::from_utf8_lossy(&bytes).to_string();
            match code {
                1 | 2 | 3 | 8 | 9 | 13 => text.parse::<i64>().map(Value::from).or_else(|_| text.parse::<u64>().map(Value::from)).unwrap_or(Value::String(text)),
                4 | 5 => text.parse::<f64>().map(Value::from).unwrap_or(Value::String(text)),
                _ => Value::String(text),
            }
        }
        My::Int(n) => json!(n),
        My::UInt(n) => json!(n),
        My::Float(f) => json!(f),
        My::Double(f) => json!(f),
        other => Value::String(other.as_sql(true)),
    }
}

/// An error as a message: the server's with its code, any other as it prints itself.
pub fn message(e: &::mysql_async::Error) -> String {
    match e {
        ::mysql_async::Error::Server(e) => format!("{} ({})", e.message, e.code),
        other => other.to_string(),
    }
}

/// The name mysql2 gives a server error (`err.code`), which the loader passes to the engine and
/// the engine prints: the errors a prepare of a statement answers with. An error not listed here
/// goes without a name, where the loader would have one.
fn error_name(code: u16) -> Option<&'static str> {
    Some(match code {
        1046 => "ER_NO_DB_ERROR",
        1048 => "ER_BAD_NULL_ERROR",
        1051 => "ER_BAD_TABLE_ERROR",
        1052 => "ER_NON_UNIQ_ERROR",
        1054 => "ER_BAD_FIELD_ERROR",
        1055 => "ER_WRONG_FIELD_WITH_GROUP",
        1056 => "ER_WRONG_GROUP_FIELD",
        1060 => "ER_DUP_FIELDNAME",
        1064 => "ER_PARSE_ERROR",
        1066 => "ER_NONUNIQ_TABLE",
        1093 => "ER_UPDATE_TABLE_USED",
        1109 => "ER_UNKNOWN_TABLE",
        1111 => "ER_INVALID_GROUP_FUNC_USE",
        1136 => "ER_WRONG_VALUE_COUNT_ON_ROW",
        1140 => "ER_MIX_OF_GROUP_FUNC_AND_FIELDS",
        1146 => "ER_NO_SUCH_TABLE",
        1210 => "ER_WRONG_ARGUMENTS",
        1222 => "ER_WRONG_NUMBER_OF_COLUMNS_IN_SELECT",
        1235 => "ER_NOT_SUPPORTED_YET",
        1241 => "ER_OPERAND_COLUMNS",
        1242 => "ER_SUBQUERY_NO_1_ROW",
        1247 => "ER_ILLEGAL_REFERENCE",
        1248 => "ER_DERIVED_MUST_HAVE_ALIAS",
        1250 => "ER_TABLENAME_NOT_ALLOWED_HERE",
        1267 => "ER_CANT_AGGREGATE_2COLLATIONS",
        1295 => "ER_UNSUPPORTED_PS",
        1305 => "ER_SP_DOES_NOT_EXIST",
        1318 => "ER_SP_WRONG_NO_OF_ARGS",
        1327 => "ER_SP_UNDECLARED_VAR",
        1353 => "ER_VIEW_WRONG_LIST",
        1356 => "ER_VIEW_INVALID",
        1364 => "ER_NO_DEFAULT_FOR_FIELD",
        1390 => "ER_PS_MANY_PARAM",
        1582 => "ER_WRONG_PARAMCOUNT_TO_NATIVE_FCT",
        1583 => "ER_WRONG_PARAMETERS_TO_NATIVE_FCT",
        1630 => "ER_FUNC_INEXISTENT_NAME_COLLISION",
        3140 => "ER_INVALID_JSON_TEXT",
        3143 => "ER_INVALID_JSON_PATH",
        3554 => "ER_NO_SYSTEM_TABLE_ACCESS",
        3579 => "ER_WINDOW_NO_SUCH_WINDOW",
        3593 => "ER_WINDOW_INVALID_WINDOW_FUNC_USE",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalog_query_takes_its_values_as_mysql2_writes_them() {
        assert_eq!(interpolate("select find_in_set(t, ?)", &[json!("users,it's\\")]).as_deref(), Some("select find_in_set(t, 'users,it\\'s\\\\')"));
        assert_eq!(interpolate("select ?, ?", &[json!(1), Value::Null]).as_deref(), Some("select 1, NULL"));
        assert_eq!(interpolate("select ?", &[]), None);
        assert_eq!(decoded(My::Bytes(b"12".to_vec()), 8), json!(12));
        assert_eq!(decoded(My::Bytes(b"12".to_vec()), 253), json!("12"));
        assert_eq!(decoded(My::NULL, 253), Value::Null);
    }
}
