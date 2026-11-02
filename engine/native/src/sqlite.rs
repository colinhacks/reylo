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
    file: File,
    /// The engine's answers, kept between statements as the loader's session keeps them.
    answers: AnswerCache<Describe>,
}

/// A database file as a connection reads it (`File::open`).
struct File {
    path: String,
    /// Whether another process held the file the last time: the next request reads a copy at once.
    #[cfg(not(target_family = "wasm"))]
    held: std::cell::Cell<bool>,
    /// Where the copies of a held file are made, from the first time it is held to the connection's close.
    #[cfg(not(target_family = "wasm"))]
    copies: std::cell::RefCell<Option<Copies>>,
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
            "open" => {
                let file = File::new(request["url"].as_str().unwrap_or_default());
                match file.open() {
                    Ok(_) => {
                        self.next += 1;
                        self.connections.insert(self.next, Connection { file, answers: AnswerCache::default() });
                        json!({ "status": "opened", "conn": self.next })
                    }
                    // A file that cannot be opened is a database that cannot be reached: a build runs on `.reylo/`.
                    Err(e) => json!({ "status": "unreachable", "message": e.to_string() }),
                }
            }
            "run" => match self.connections.get_mut(&id) {
                None => closed(id),
                Some(c) => match c.file.open() {
                    Err(e) => json!({ "status": "unreachable", "message": e.to_string() }),
                    Ok(db) => match answers::analyze(&db, sql, &mut c.answers) {
                        Ok(analysis) => json!({ "status": "analysis", "analysis": analysis }),
                        Err(e) => json!({ "status": "error", "message": e.to_string() }),
                    },
                },
            },
            "query" => match self.connections.get(&id) {
                None => closed(id),
                Some(c) => match c.file.open() {
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

impl File {
    fn new(path: &str) -> File {
        File {
            path: path.to_string(),
            #[cfg(not(target_family = "wasm"))]
            held: std::cell::Cell::new(false),
            #[cfg(not(target_family = "wasm"))]
            copies: std::cell::RefCell::new(None),
        }
    }

    /// The database, read-only; `:memory:` is an empty one.
    fn open(&self) -> rusqlite::Result<Db> {
        if self.path == ":memory:" {
            return Db::open_in_memory();
        }
        let db = Db::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI)?;
        // A database in WAL mode keeps its index in shared memory, the `-shm` file mapped, which
        // WebAssembly cannot map: in exclusive locking mode SQLite keeps the index in its own memory
        // and reads the log whole. WASI's file locks are no-ops, so the mode holds nothing against the
        // app's own connections; a native build keeps SQLite's usual locking, which would. The mode also
        // trusts that nothing else writes the file, so the connection lives for one request.
        #[cfg(target_family = "wasm")]
        db.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
        // Natively the locks are real, and an engine that keeps the file locked for as long as its
        // connection is open (Turso's) leaves a reader waiting forever: after a short wait the program
        // reads a copy of the file and its write-ahead log, as they stand, which sees every committed
        // row and table; once a file is held, the next request goes to the copy without waiting.
        #[cfg(not(target_family = "wasm"))]
        {
            db.busy_timeout(std::time::Duration::from_millis(if self.held.get() { 0 } else { 250 }))?;
            match db.query_row("select count(*) from sqlite_schema", [], |_| Ok(())) {
                Err(rusqlite::Error::SqliteFailure(e, _)) if matches!(e.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
                    self.held.set(true);
                    return self.open_copy();
                }
                Err(e) => return Err(e),
                Ok(()) => self.held.set(false),
            }
        }
        Ok(db)
    }

    /// A copy of the database and its write-ahead log, opened: made again for each request, so that
    /// it reads the file as it is then. The log is copied after the file, and both again when the
    /// log's header changed meanwhile: a checkpoint that restarts the log between the two copies
    /// would pair the file as it was with a log that no longer holds what the file lacks, while a
    /// checkpoint that only writes the file leaves the log holding every page it wrote.
    #[cfg(not(target_family = "wasm"))]
    fn open_copy(&self) -> rusqlite::Result<Db> {
        let failed = |e: std::io::Error| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN), Some(format!("cannot copy the database the app holds: {e}")));
        let mut copies = self.copies.borrow_mut();
        if copies.is_none() {
            *copies = Some(Copies::create().map_err(failed)?);
        }
        let dir = &copies.as_ref().unwrap().0;
        let (copy, wal, wal_copy) = (dir.join("db"), std::path::PathBuf::from(format!("{}-wal", self.path)), dir.join("db-wal"));
        for _ in 0..5 {
            for stale in [&wal_copy, &dir.join("db-shm")] {
                let _ = std::fs::remove_file(stale);
            }
            let before = wal_header(&wal);
            std::fs::copy(&self.path, &copy).map_err(failed)?;
            // A log removed since its header was read is a log that changed.
            match before.map(|_| std::fs::copy(&wal, &wal_copy)) {
                Some(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Some(Err(e)) => return Err(failed(e)),
                _ => {}
            }
            if before == wal_header(&wal_copy) && before == wal_header(&wal) {
                return Db::open_with_flags(&copy, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX);
            }
        }
        Err(failed(std::io::Error::other("its write-ahead log was restarted under every copy")))
    }
}

/// The first 32 bytes of a write-ahead log, its header: a restarted log has another checkpoint
/// sequence number and salts. None where there is no log, or none yet.
#[cfg(not(target_family = "wasm"))]
fn wal_header(path: &std::path::Path) -> Option<[u8; 32]> {
    let mut header = [0; 32];
    std::io::Read::read_exact(&mut std::fs::File::open(path).ok()?, &mut header).ok()?;
    Some(header)
}

/// A directory of the program's own for the copies of a held file, removed with its connection:
/// made where nothing was, so that no directory or link another user put in the shared temporary
/// directory is written through, and on Unix open to its own user alone.
#[cfg(not(target_family = "wasm"))]
struct Copies(std::path::PathBuf);

#[cfg(not(target_family = "wasm"))]
impl Copies {
    fn create() -> std::io::Result<Copies> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        let mut tries = 0;
        loop {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
            let dir = std::env::temp_dir().join(format!("reylo-sqlite-{}-{}-{nanos:08x}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
            // `create` refuses a path that exists, where `create_dir_all` would take it.
            match builder.create(&dir) {
                Ok(()) => return Ok(Copies(dir)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && tries < 16 => tries += 1,
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for Copies {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    #[test]
    fn a_held_file_is_read_through_a_copy_of_its_own_removed_with_the_connection() {
        let dir = std::env::temp_dir().join(format!("reylo-sqlite-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.db");
        // The app's connection: the rows committed to the log alone, and the file held as Turso's engine holds it.
        let app = Db::open(&path).unwrap();
        app.execute_batch("pragma journal_mode = wal; pragma wal_autocheckpoint = 0; create table users (id integer primary key); insert into users values (1); pragma locking_mode = exclusive; begin exclusive; commit;").unwrap();
        let file = File::new(path.to_str().unwrap());
        let db = file.open().unwrap();
        assert!(file.held.get());
        assert_eq!(db.query_row("select count(*) from users", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        let copies = file.copies.borrow().as_ref().unwrap().0.clone();
        assert!(copies.join("db").exists());
        #[cfg(unix)]
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&copies).unwrap().permissions()) & 0o777, 0o700);
        drop(db);
        drop(file);
        assert!(!copies.exists());
        drop(app);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
