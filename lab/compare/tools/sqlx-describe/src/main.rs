// Asks sqlx for each statement's description, the one its query! macros read at compile time, and prints it as JSON: `sqlx-describe <db> <url>`, with a JSON array of statements on stdin.
use serde_json::{json, Value};
use sqlx::{AssertSqlSafe, Column, Connection, Either, Executor, SqlSafeStr, TypeInfo};
use std::io::Read;

macro_rules! describe {
    ($conn:expr, $sql:expr) => {{
        match (&mut $conn).describe(AssertSqlSafe($sql.to_string()).into_sql_str()).await {
            Ok(d) => {
                let params: Value = match d.parameters() {
                    Some(Either::Left(ps)) => json!(ps.iter().map(|p| p.name().to_string()).collect::<Vec<_>>()),
                    Some(Either::Right(n)) => json!({ "count": n }),
                    None => json!(null),
                };
                let columns: Vec<Value> = d.columns().iter().enumerate().map(|(i, c)| json!({ "name": c.name(), "type": c.type_info().name(), "nullable": d.nullable(i) })).collect();
                json!({ "ok": true, "params": params, "columns": columns })
            }
            Err(e) => json!({ "ok": false, "error": e.to_string() }),
        }
    }};
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (db, url) = (&args[1], &args[2]);
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let statements: Vec<String> = serde_json::from_str(&input).unwrap();
    let mut out = Vec::new();
    match db.as_str() {
        "postgres" => { let mut c = sqlx::PgConnection::connect(url).await.unwrap(); for s in &statements { out.push(describe!(c, s)); } }
        "mysql" => { let mut c = sqlx::MySqlConnection::connect(url).await.unwrap(); for s in &statements { out.push(describe!(c, s)); } }
        "sqlite" => { let mut c = sqlx::SqliteConnection::connect(url).await.unwrap(); for s in &statements { out.push(describe!(c, s)); } }
        _ => panic!("unknown database"),
    }
    println!("{}", Value::Array(out));
}
