//! The DuckDB program: the core's requests alone. DuckDB runs inside the app, so the engine opens
//! no database of its own: the loader answers its needs through the app's DuckDB
//! (src/duckdb.ts), and a request that would open one is refused, saying so.

use crate::Program;
use serde_json::{json, Value};

#[derive(Default)]
pub struct Duckdb;

impl Program for Duckdb {
    fn call(&self, request: &str) -> String {
        reylo_engine_core::api::call::<reylo_engine::duckdb::Duckdb>(request)
    }

    async fn serve(&mut self, request: &Value) -> Option<Value> {
        match request["op"].as_str()? {
            "open" | "run" | "query" | "describe" | "forget" | "close" => {
                Some(json!({ "status": "error", "message": "the DuckDB engine opens no database: its host answers its needs through the app's own DuckDB" }))
            }
            _ => None,
        }
    }
}
