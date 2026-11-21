//! Reylo's analysis engine for a Rust host, and the engine as a native program.
//!
//! The engine types a SQL statement from the database itself: its parameters, its columns and
//! their nullability, the shapes of the JSON it builds, and how many rows it returns. Its core never
//! opens a connection. It replays an analysis, naming the statements it needs described and the
//! catalog queries it needs run, and a host answers them ([`replay`]).
//!
//! Each database is behind a feature (`postgres`, `sqlite`, `mysql`, `clickhouse`), which brings
//! its engine, the answers to its needs over its client (tokio-postgres, rusqlite, mysql_async, or
//! ureq for ClickHouse's HTTP interface) with an `analyze` that runs a statement's whole replay on
//! it ([`clients`]), and its program. A host that answers the
//! engine its own way implements [`Answerer`] or [`AsyncAnswerer`] and calls [`replay::run`] or
//! [`replay::run_async`] (or, for a client that takes several requests at once,
//! [`ConcurrentAnswerer`] and [`replay::run_concurrent`]) with its database's engine, from [`engines`]
//! (`replay::run_async::<reylo_engine::engines::postgres::Postgres, _>`): the module of a database's
//! name at the crate's root, `reylo_engine::postgres`, is its program.
//!
//! ```ignore
//! let (client, connection) = tokio_postgres::connect(url, tokio_postgres::NoTls).await?;
//! tokio::spawn(connection);
//! let mut cache = reylo_engine::AnswerCache::default();
//! let analysis = reylo_engine::clients::tokio_postgres::analyze(&client, "select id, name from users where id = :id", &mut cache).await?;
//! assert_eq!(analysis.cardinality, "maybeOne");
//! ```
//!
//! The program reads one JSON request per line on standard input and writes
//! one JSON answer per line on standard output, carrying the request's `id` back. The core answers
//! the requests that need no database (`ping`, `lex`, `config`, and `analyze` with the host's own
//! answers). A database's program also opens connections of its own (`open`, answered with the
//! connection's number and the server's version, as `reylo analyze` reports it), analyzes a statement
//! to its end over one (`run`), runs a query on one (`query`), describes a statement on one
//! (`describe`, on Postgres), forgets the answers it keeps for one when the schema changes
//! (`forget`), and closes it (`close`). Each database's WebAssembly component (engine/wasip2) is this
//! program behind two exports, and each binary in `src/bin/` is it as a native executable.
//!
//! `<program> call` answers the one request on standard input and exits, for a host that must have
//! an answer before it can go on, as one reading the project file must.

pub use reylo_engine_core::api::{Analysis, Column, DbError, Diagnostic, Param};
pub use reylo_engine_core::database::Database;
pub use reylo_engine_core::dialect::Dialect;
pub use reylo_engine_core::replay::{self, AnswerCache, Answerer, AsyncAnswerer, ConcurrentAnswerer, ReplayError};
pub use reylo_engine_core::types::{Field, Type};

/// Each database's engine; its program is the module of its name at the crate's root.
pub mod engines {
    /// The Postgres engine: libpg_query's parse tree over the server's describe and catalog.
    #[cfg(feature = "postgres")]
    pub use reylo_engine_postgres as postgres;
    /// The SQLite engine: SQLite's own resolved tree, from the copy of SQLite inside it.
    #[cfg(feature = "sqlite")]
    pub use reylo_engine_sqlite as sqlite;
    /// The MySQL engine: the server's prepare metadata, read with a parse of the statement.
    #[cfg(feature = "mysql")]
    pub use reylo_engine_mysql as mysql;
    /// The ClickHouse engine: the server's own describe of a statement, and its query tree.
    #[cfg(feature = "clickhouse")]
    pub use reylo_engine_clickhouse as clickhouse;
}

#[cfg(any(feature = "postgres", feature = "sqlite", feature = "mysql", feature = "clickhouse"))]
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

use serde_json::{json, Value};
use std::future::Future;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// The hash of the sources that connect to Postgres and MySQL and read their answers (`build.rs`),
/// which the migrations' cache keys its replays by.
#[doc(hidden)]
pub const SOURCE_HASH: &str = env!("REYLO_ENGINE_HASH");

/// The project file, `.reylo.yaml`, and the database it names, as every frontend reads them.
pub mod project;
#[cfg(feature = "clickhouse")]
pub mod clickhouse;
#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(any(feature = "postgres", feature = "mysql"))]
mod tls;

/// Why a connection could not be opened or kept, by kind, as a command line tells its user: the
/// `status` of a program's failure answer, with its message (for `NotMysql`, the server it found).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The server gave no answer, or went away.
    Unreachable(String),
    /// The server refused the login.
    Refused(String),
    /// A MySQL URL reached a server that is not MySQL: MariaDB, named as its `version()` names it.
    NotMysql(String),
    /// Anything else: a URL that cannot be read, TLS settings, an error from the server.
    Error(String),
}

impl Failure {
    /// The failure a program's answer reports.
    #[cfg_attr(not(any(feature = "postgres", feature = "mysql", feature = "clickhouse")), allow(dead_code))]
    fn of(answer: &Value) -> Failure {
        let message = answer["message"].as_str().unwrap_or_default().to_string();
        match answer["status"].as_str() {
            Some("unreachable") => Failure::Unreachable(message),
            Some("refused") => Failure::Refused(message),
            Some("notMysql") => Failure::NotMysql(answer["server"].as_str().unwrap_or_default().to_string()),
            _ => Failure::Error(message),
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Unreachable(m) => write!(f, "cannot reach the database: {m}"),
            Failure::Refused(m) => write!(f, "the database refused the login: {m}"),
            Failure::NotMysql(server) => write!(f, "the URL names a MySQL database, and the server is {server}, which Reylo does not read"),
            Failure::Error(m) => f.write_str(m),
        }
    }
}

/// One database's program: the core's own requests, and those that need a connection.
pub trait Program {
    /// The core's answer to a request that needs no database, as the WebAssembly engine gives it.
    fn call(&self, request: &str) -> String;
    /// The answer to a request the core does not know: `None` where this program does not know it either.
    fn serve(&mut self, request: &Value) -> impl Future<Output = Option<Value>>;
}

pub fn main<P: Program>(program: P) {
    serve(program, std::env::args().nth(1).as_deref() == Some("call"));
}

/// Serves requests line by line, or with `call`, answers the one request on standard input.
pub fn serve<P: Program>(mut program: P, call: bool) {
    if call {
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
