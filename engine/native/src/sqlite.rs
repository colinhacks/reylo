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

/// The statements of one command line's run against a database file, which is opened afresh for
/// each, as the program's are, and keeps the engine's answers between them.
pub struct Session {
    connection: Connection,
    /// The version of the SQLite that reads the file, as `3.50.2`.
    pub version: String,
}

impl Session {
    /// The database file at `path`, resolved as the project names it (`sqlitePath` in src/project.ts).
    pub fn open(path: &str) -> Result<Session, crate::Failure> {
        let file = File::new(path);
        file.open().map_err(|e| crate::Failure::Unreachable(e.to_string()))?;
        Ok(Session { connection: Connection { file, answers: AnswerCache::default() }, version: rusqlite::version().to_string() })
    }

    pub fn analyze(&mut self, sql: &str) -> Result<reylo_engine_core::api::Analysis, crate::Failure> {
        let db = self.connection.file.open().map_err(|e| crate::Failure::Unreachable(e.to_string()))?;
        answers::analyze(&db, sql, &mut self.connection.answers).map_err(|e| crate::Failure::Error(e.to_string()))
    }
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
                        json!({ "status": "opened", "conn": self.next, "version": rusqlite::version() })
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

    /// A copy of the database and its logs, opened, as they stand: made again only once the file or
    /// a log has changed since the last copy. Each log, the write-ahead log or the rollback journal,
    /// is copied after the file, and both again when the log's header changed meanwhile: a log
    /// restarted between the two copies would pair the file as it was with a log that no longer
    /// holds what the file lacks, while a checkpoint that only writes the file leaves the log
    /// holding every page it wrote. SQLite rolls a hot journal back when it opens the copy, which so
    /// reads the last commit of a file a writer was changing.
    #[cfg(not(target_family = "wasm"))]
    fn open_copy(&self) -> rusqlite::Result<Db> {
        let failed = |e: std::io::Error| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN), Some(format!("cannot copy the database the app holds: {e}")));
        let open = |copy: &std::path::Path| Db::open_with_flags(copy, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX);
        let mut copies = self.copies.borrow_mut();
        if copies.is_none() {
            *copies = Some(Copies::create().map_err(failed)?);
        }
        let copies = copies.as_mut().unwrap();
        let copy = copies.dir.join("db");
        let logs: Vec<(std::path::PathBuf, std::path::PathBuf)> = ["-wal", "-journal"].iter().map(|suffix| (format!("{}{suffix}", self.path).into(), copies.dir.join(format!("db{suffix}")))).collect();
        // The file and its logs as they stand: their sizes and times, which a write changes.
        let now = std::time::SystemTime::now();
        let stamp: Stamp = std::iter::once(std::path::Path::new(&self.path)).chain(logs.iter().map(|(from, _)| from.as_path())).map(|path| std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()))).collect();
        if copies.made.as_ref() == Some(&stamp) {
            return open(&copy);
        }
        for _ in 0..5 {
            for stale in logs.iter().map(|(_, to)| to.clone()).chain([copies.dir.join("db-shm")]) {
                let _ = std::fs::remove_file(stale);
            }
            let before: Vec<_> = logs.iter().map(|(from, _)| log_header(from)).collect();
            std::fs::copy(&self.path, &copy).map_err(failed)?;
            let mut changed = false;
            for ((from, to), header) in logs.iter().zip(&before) {
                // A log removed since its header was read is a log that changed.
                match header.map(|_| std::fs::copy(from, to)) {
                    Some(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => changed = true,
                    Some(Err(e)) => return Err(failed(e)),
                    _ => {}
                }
            }
            if !changed && logs.iter().zip(&before).all(|((from, to), header)| *header == log_header(to) && *header == log_header(from)) {
                // A file written within the last second may be written again in the same clock tick
                // with the same size, which the stamp would not tell from this copy, so the copy is
                // kept for the next request only once every time is older than that: git's rule for
                // a racy index entry.
                let settled = stamp.iter().flatten().all(|(_, modified)| modified.is_some_and(|m| now.duration_since(m).is_ok_and(|age| age >= std::time::Duration::from_secs(1))));
                copies.made = settled.then_some(stamp);
                return open(&copy);
            }
        }
        Err(failed(std::io::Error::other("its logs were restarted under every copy")))
    }
}

/// A database file and its logs as they stood: each one's size and time, where it exists.
#[cfg(not(target_family = "wasm"))]
type Stamp = Vec<Option<(u64, Option<std::time::SystemTime>)>>;

/// The first 32 bytes of a log, its header: a restarted write-ahead log has another checkpoint
/// sequence number and salts, and a rollback journal another nonce. None where there is no log, or
/// none yet.
#[cfg(not(target_family = "wasm"))]
fn log_header(path: &std::path::Path) -> Option<[u8; 32]> {
    let mut header = [0; 32];
    std::io::Read::read_exact(&mut std::fs::File::open(path).ok()?, &mut header).ok()?;
    Some(header)
}

/// A directory of the program's own for the copies of a held file, removed with its connection:
/// made where nothing was, so that no directory or link another user put in the shared temporary
/// directory is written through, and on Unix open to its own user alone.
#[cfg(not(target_family = "wasm"))]
struct Copies {
    dir: std::path::PathBuf,
    /// The file and its logs as they stood when the copy in `dir` was made: while they stand so, the
    /// copy is read again.
    made: Option<Stamp>,
}

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
                Ok(()) => return Ok(Copies { dir, made: None }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && tries < 16 => tries += 1,
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for Copies {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    /// The tests that hold a file take turns: the bundled SQLite's page cache is one pool for the
    /// process, so a connection of another test lends this one's cache room, and a transaction
    /// meant to spill its pages into the file spills none.
    static TURNS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn a_held_file_is_read_through_a_copy_of_its_own_removed_with_the_connection() {
        let _turn = TURNS.lock().unwrap_or_else(|e| e.into_inner());
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
        let copies = file.copies.borrow().as_ref().unwrap().dir.clone();
        assert!(copies.join("db").exists());
        #[cfg(unix)]
        assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&copies).unwrap().permissions()) & 0o777, 0o700);
        drop(db);
        drop(file);
        assert!(!copies.exists());
        drop(app);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_held_file_mid_transaction_is_read_as_its_last_commit_and_its_copy_kept_while_it_stands() {
        let _turn = TURNS.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("reylo-sqlite-journal-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.db");
        // The app's connection, in rollback-journal mode: 2,000 committed rows, then a transaction
        // rewriting each one, whose pages a one-page cache has already spilled into the file, with
        // the hot journal beside it.
        let app = Db::open(&path).unwrap();
        app.execute_batch("pragma journal_mode = delete; create table users (id integer primary key, name text); insert into users with recursive n(i) as (select 1 union all select i + 1 from n where i < 2000) select i, 'old' || hex(randomblob(200)) from n; pragma cache_size = 1; pragma locking_mode = exclusive; begin; update users set name = 'new' || substr(name, 4);").unwrap();
        assert!(std::path::Path::new(&format!("{}-journal", path.display())).exists());
        let file = File::new(path.to_str().unwrap());
        let db = file.open().unwrap();
        assert!(file.held.get());
        // The copy is the file as of its last commit: none of the rows the transaction rewrote.
        let rewritten = |db: &Db| db.query_row("select count(*) from users where name like 'new%'", [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(rewritten(&db), 0);
        assert_eq!(db.query_row("pragma integrity_check", [], |r| r.get::<_, String>(0)).unwrap(), "ok");
        drop(db);
        // The file was written within the last second, so its copy is not kept: the next request
        // copies it again, and once the file has settled, that copy is kept while it stands.
        assert!(file.copies.borrow().as_ref().unwrap().made.is_none());
        let settled = std::time::SystemTime::now() - std::time::Duration::from_secs(10);
        for written in [path.clone(), dir.join("app.db-journal")] {
            std::fs::File::options().write(true).open(written).unwrap().set_modified(settled).unwrap();
        }
        drop(file.open().unwrap());
        assert!(file.copies.borrow().as_ref().unwrap().made.is_some());
        // A mark made in the kept copy is what the next request reads, where a new copy would not have it.
        let copy = file.copies.borrow().as_ref().unwrap().dir.join("db");
        Db::open(&copy).unwrap().execute("update users set name = 'kept' where id = 1", []).unwrap();
        let db = file.open().unwrap();
        assert_eq!(db.query_row("select name from users where id = 1", [], |r| r.get::<_, String>(0)).unwrap(), "kept");
        assert_eq!(rewritten(&db), 0);
        drop(db);
        drop(file);
        drop(app);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
