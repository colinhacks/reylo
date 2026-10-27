//! The SQLite program: the database file opened read-only over rusqlite, its schema and the
//! functions of the SQLite the program carries read by the engine's catalog queries, and each
//! statement analyzed by the replay loop of `reylo-engine`, which prepares it inside the engine's own
//! SQLite and keeps the connection's answers between statements. Preparing each statement on the
//! app's own SQLite drivers too is the host's (src/sqlite-drivers.ts): they are the app's, not the
//! engine's.

use crate::Program;
use reylo_engine::clients::rusqlite::{self as answers, Answers, Describe};
use reylo_engine::{AnswerCache, Answerer};
use rusqlite::{Connection as Db, OpenFlags};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Default)]
pub struct Sqlite {
    connections: HashMap<u64, Connection>,
    next: u64,
}

/// A database file, opened afresh for each request, so that each reads the file as it is then: a
/// connection kept open would keep the schema it read first (see `open`).
struct Connection {
    path: String,
    /// The engine's answers, kept between statements as the loader's session keeps them.
    answers: AnswerCache<Describe>,
}

impl Program for Sqlite {
    fn call(&self, request: &str) -> String {
        reylo_engine_core::api::call::<reylo_engine::sqlite::Sqlite>(request)
    }

    async fn serve(&mut self, request: &Value) -> Option<Value> {
        let op = request["op"].as_str()?;
        let id = request["conn"].as_u64().unwrap_or(0);
        let sql = request["sql"].as_str().unwrap_or_default();
        Some(match op {
            // The host passes the file's path, resolved as the project names it (`sqlitePath` in src/project.ts).
            "open" => match open(request["url"].as_str().unwrap_or_default()) {
                Ok(_) => {
                    self.next += 1;
                    self.connections.insert(self.next, Connection { path: request["url"].as_str().unwrap_or_default().to_string(), answers: AnswerCache::default() });
                    json!({ "status": "opened", "conn": self.next })
                }
                // A file that cannot be opened is a database that cannot be reached: a build runs on `.reylo/`.
                Err(e) => json!({ "status": "unreachable", "message": e.to_string() }),
            },
            "run" => match self.connections.get_mut(&id) {
                None => closed(id),
                Some(c) => match open(&c.path) {
                    Err(e) => json!({ "status": "unreachable", "message": e.to_string() }),
                    Ok(db) => match answers::analyze(&db, sql, &mut c.answers) {
                        Ok(analysis) => json!({ "status": "analysis", "analysis": analysis }),
                        Err(e) => json!({ "status": "error", "message": e.to_string() }),
                    },
                },
            },
            "query" => match self.connections.get(&id) {
                None => closed(id),
                Some(c) => match open(&c.path) {
                    Err(e) => json!({ "status": "unreachable", "message": e.to_string() }),
                    Ok(db) => {
                        let params = request["params"].as_array().cloned().unwrap_or_default();
                        match Answers(&db).query(sql, &params) {
                            Ok(Ok(rows)) => json!({ "status": "rows", "rows": rows }),
                            Ok(Err(e)) => json!({ "status": "error", "message": e.message }),
                            Err(e) => json!({ "status": "error", "message": e.0 }),
                        }
                    }
                },
            },
            // The schema changed: what the engine was told about the old one is stale.
            "forget" => match self.connections.get_mut(&id) {
                None => closed(id),
                Some(c) => {
                    c.answers.clear();
                    json!({ "status": "ok" })
                }
            },
            "close" => {
                self.connections.remove(&id);
                json!({ "status": "ok" })
            }
            _ => return None,
        })
    }
}

fn closed(id: u64) -> Value {
    json!({ "status": "unreachable", "message": format!("connection {id} is closed") })
}

/// The database at `path`, read-only; `:memory:` is an empty one.
fn open(path: &str) -> rusqlite::Result<Db> {
    if path == ":memory:" {
        return Db::open_in_memory();
    }
    let db = Db::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI)?;
    // A database in WAL mode keeps its index in shared memory, the `-shm` file mapped, which
    // WebAssembly cannot map: in exclusive locking mode SQLite keeps the index in its own memory
    // and reads the log whole. WASI's file locks are no-ops, so the mode holds nothing against the
    // app's own connections; a native build keeps SQLite's usual locking, which would. The mode also
    // trusts that nothing else writes the file, so the connection lives for one request.
    #[cfg(target_family = "wasm")]
    db.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
    Ok(db)
}
