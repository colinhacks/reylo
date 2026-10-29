//! The engine as a native program. It reads one JSON request per line on standard input and writes
//! one JSON answer per line on standard output, carrying the request's `id` back. The core answers
//! the requests that need no database (`ping`, `lex`, `config`, and `analyze` with the host's own
//! answers). A database's program also opens connections of its own (`open`), analyzes a statement
//! to its end over one (`run`), runs a query on one (`query`), describes a statement on one
//! (`describe`, on Postgres), forgets the answers it keeps for one when the schema changes
//! (`forget`), and closes it (`close`). Each database's WebAssembly component (engine/wasip2) is this
//! program behind two exports, and each binary in `src/bin/` is it as a native executable.
//!
//! `<program> call` answers the one request on standard input and exits, for a host that must have
//! an answer before it can go on, as one reading the project file must.

use serde_json::{json, Value};
use std::future::Future;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[cfg(feature = "duckdb")]
pub mod duckdb;
#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(any(feature = "postgres", feature = "mysql"))]
mod tls;

/// One database's program: the core's own requests, and those that need a connection.
pub trait Program {
    /// The core's answer to a request that needs no database, as the WebAssembly engine gives it.
    fn call(&self, request: &str) -> String;
    /// The answer to a request the core does not know: `None` where this program does not know it either.
    fn serve(&mut self, request: &Value) -> impl Future<Output = Option<Value>>;
}

pub fn main<P: Program>(mut program: P) {
    if std::env::args().nth(1).as_deref() == Some("call") {
        let mut request = String::new();
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut request);
        println!("{}", program.call(&request));
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
    runtime.block_on(async move {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        let mut out = tokio::io::stdout();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut text = answer(&mut program, &line).await;
            text.push('\n');
            if out.write_all(text.as_bytes()).await.is_err() || out.flush().await.is_err() {
                break;
            }
        }
    });
}

/// The answer to one request line, as one line of JSON carrying the request's `id` back: the
/// program's own where it knows the request, else the core's. Every host of a program answers
/// through it: `main` above, and the WebAssembly component (engine/wasip2).
pub async fn answer<P: Program>(program: &mut P, line: &str) -> String {
    let request: Value = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(e) => json!({ "status": "error", "message": format!("not a request: {e}") }),
    };
    let mut answer = match program.serve(&request).await {
        Some(answer) => answer,
        None => serde_json::from_str(&program.call(line)).unwrap_or_else(|e| json!({ "status": "error", "message": e.to_string() })),
    };
    if let (Some(answer), Some(id)) = (answer.as_object_mut(), request.get("id")) {
        answer.insert("id".into(), id.clone());
    }
    answer.to_string()
}

/// A URL parameter's value, percent-decoded.
#[cfg(any(feature = "postgres", feature = "mysql"))]
pub(crate) fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match (bytes[i], bytes.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok())) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                i += 1;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
