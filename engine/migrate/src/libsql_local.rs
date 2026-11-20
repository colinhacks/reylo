//! A database on libSQL's own engine, in this process: a file, or a shadow in memory. Turso's
//! databases run libSQL, whose SQL is SQLite's and more (a vector index, `libsql_vector_idx`, and
//! the vectors it indexes; `alter table … alter column`), which Reylo's SQLite (rusqlite's) refuses.
//! So a project whose SQL uses it ([`crate::libsql::uses_own_sql`]) works on its development file
//! here, and every shadow of a libSQL database, a Turso one among them, is made here: the engine is
//! the one Turso's `libsql-ffi` crate carries, SQLite 3.47.0, with the sqlean extensions Turso
//! Cloud's server loads (math, stats, text, crypto, fuzzy, uuid, regexp), as it runs them.
//! `csrc/reylo_libsql.c` compiles it with its API private, beside the binary's other SQLites.

use crate::db::Row;
use crate::Error;
use std::ffi::{c_char, c_int, c_void, CStr, CString};

#[repr(C)]
struct Sqlite3 {
    _opaque: [u8; 0],
}

#[repr(C)]
struct Stmt {
    _opaque: [u8; 0],
}

extern "C" {
    fn reylo_libsql_open(path: *const c_char, db: *mut *mut Sqlite3) -> c_int;
    fn reylo_libsql_close(db: *mut Sqlite3) -> c_int;
    fn reylo_libsql_errmsg(db: *mut Sqlite3) -> *const c_char;
    fn reylo_libsql_version() -> *const c_char;
    fn reylo_libsql_exec(db: *mut Sqlite3, sql: *const c_char) -> c_int;
    fn reylo_libsql_prepare(db: *mut Sqlite3, sql: *const c_char, stmt: *mut *mut Stmt) -> c_int;
    fn reylo_libsql_bind_text(stmt: *mut Stmt, i: c_int, text: *const c_char, n: c_int) -> c_int;
    fn reylo_libsql_step(stmt: *mut Stmt) -> c_int;
    fn reylo_libsql_finalize(stmt: *mut Stmt) -> c_int;
    fn reylo_libsql_column_count(stmt: *mut Stmt) -> c_int;
    fn reylo_libsql_column_name(stmt: *mut Stmt, i: c_int) -> *const c_char;
    fn reylo_libsql_column_type(stmt: *mut Stmt, i: c_int) -> c_int;
    fn reylo_libsql_column_int64(stmt: *mut Stmt, i: c_int) -> i64;
    fn reylo_libsql_column_double(stmt: *mut Stmt, i: c_int) -> f64;
    fn reylo_libsql_column_blob(stmt: *mut Stmt, i: c_int) -> *const c_void;
    fn reylo_libsql_column_text(stmt: *mut Stmt, i: c_int) -> *const u8;
    fn reylo_libsql_column_bytes(stmt: *mut Stmt, i: c_int) -> c_int;
    fn reylo_libsql_backup(to: *mut Sqlite3, from: *mut Sqlite3) -> c_int;
}

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;
const SQLITE_INTEGER: c_int = 1;
const SQLITE_FLOAT: c_int = 2;
const SQLITE_NULL: c_int = 5;

/// One connection to a database on libSQL's engine.
pub struct Local {
    db: *mut Sqlite3,
}

// SAFETY: the engine is built serialized (`SQLITE_THREADSAFE=1`), and a connection is used by one
// task at a time, as rusqlite's is; moving it between threads is all that `Send` allows.
unsafe impl Send for Local {}

/// The version of libSQL's SQLite, as `3.47.0`.
pub fn version() -> String {
    // SAFETY: the engine returns a static, NUL-terminated string.
    unsafe { CStr::from_ptr(reylo_libsql_version()) }.to_string_lossy().into_owned()
}

fn c_string(text: &str) -> Result<CString, Error> {
    CString::new(text).map_err(|_| Error("SQL holding a NUL character can't run".into()))
}

impl Local {
    /// The database file at `path`, made where there is none, or `:memory:`, a new empty one.
    pub fn open(path: &str) -> Result<Local, Error> {
        let path = c_string(path)?;
        let mut db = std::ptr::null_mut();
        // SAFETY: `path` is NUL-terminated and outlives the call; `db` receives the handle, which
        // the engine sets even on a failure, so that its message can be read and it closed.
        let rc = unsafe { reylo_libsql_open(path.as_ptr(), &mut db) };
        let local = Local { db };
        if rc != SQLITE_OK {
            return Err(local.error());
        }
        Ok(local)
    }

    pub fn memory() -> Result<Local, Error> {
        Local::open(":memory:")
    }

    fn error(&self) -> Error {
        if self.db.is_null() {
            return Error("libSQL's engine is out of memory".into());
        }
        // SAFETY: an open handle's message is NUL-terminated and valid until the next call on it.
        Error(unsafe { CStr::from_ptr(reylo_libsql_errmsg(self.db)) }.to_string_lossy().into_owned())
    }

    /// Runs `sql`, one statement or several, in order, stopping at the first that fails.
    pub fn exec(&mut self, sql: &str) -> Result<(), Error> {
        let sql = c_string(sql)?;
        // SAFETY: an open handle and a NUL-terminated statement that outlives the call.
        if unsafe { reylo_libsql_exec(self.db, sql.as_ptr()) } != SQLITE_OK {
            return Err(self.error());
        }
        Ok(())
    }

    /// Runs one statement with text parameters, `?` each.
    pub fn exec_with(&mut self, sql: &str, params: &[&str]) -> Result<(), Error> {
        let statement = self.prepare(sql)?;
        for (i, param) in params.iter().enumerate() {
            let len = c_int::try_from(param.len()).map_err(|_| Error("a parameter too long to bind".into()))?;
            // SAFETY: a live statement; the engine copies the text (SQLITE_TRANSIENT) before returning.
            if unsafe { reylo_libsql_bind_text(statement.0, i as c_int + 1, param.as_ptr().cast(), len) } != SQLITE_OK {
                return Err(self.error());
            }
        }
        loop {
            // SAFETY: a live statement.
            match unsafe { reylo_libsql_step(statement.0) } {
                SQLITE_ROW => continue,
                SQLITE_DONE => return Ok(()),
                _ => return Err(self.error()),
            }
        }
    }

    /// The rows of one statement, every value as text, as rusqlite's are read: an integer and a
    /// real as Rust writes them, a blob's bytes read as UTF-8.
    pub fn rows(&mut self, sql: &str) -> Result<Vec<Row>, Error> {
        let statement = self.prepare(sql)?;
        let s = statement.0;
        // SAFETY: a live statement, whose column names are NUL-terminated and valid until it is
        // finalized; each value is read before the next step.
        unsafe {
            let count = reylo_libsql_column_count(s);
            let columns: Vec<String> = (0..count).map(|i| CStr::from_ptr(reylo_libsql_column_name(s, i)).to_string_lossy().into_owned()).collect();
            let mut out = Vec::new();
            loop {
                match reylo_libsql_step(s) {
                    SQLITE_ROW => {}
                    SQLITE_DONE => return Ok(out),
                    _ => return Err(self.error()),
                }
                let values = (0..count)
                    .map(|i| match reylo_libsql_column_type(s, i) {
                        SQLITE_NULL => None,
                        SQLITE_INTEGER => Some(reylo_libsql_column_int64(s, i).to_string()),
                        SQLITE_FLOAT => Some(reylo_libsql_column_double(s, i).to_string()),
                        _ => {
                            // A blob's bytes and a text's alike: the pointer is read first, then its length.
                            let data = reylo_libsql_column_blob(s, i).cast::<u8>();
                            let len = reylo_libsql_column_bytes(s, i) as usize;
                            let data = if data.is_null() { reylo_libsql_column_text(s, i) } else { data };
                            Some(if data.is_null() || len == 0 { String::new() } else { String::from_utf8_lossy(std::slice::from_raw_parts(data, len)).into_owned() })
                        }
                    })
                    .collect();
                out.push(Row::new(columns.clone(), values));
            }
        }
    }

    /// Copies this database over `to`'s, whole.
    pub fn backup_to(&self, to: &mut Local) -> Result<(), Error> {
        // SAFETY: two open handles, neither used elsewhere during the copy.
        if unsafe { reylo_libsql_backup(to.db, self.db) } != SQLITE_OK {
            return Err(Error(format!("the libSQL backup failed: {}", to.error())));
        }
        Ok(())
    }

    fn prepare(&mut self, sql: &str) -> Result<Statement, Error> {
        let sql = c_string(sql)?;
        let mut stmt = std::ptr::null_mut();
        // SAFETY: an open handle and a NUL-terminated statement that outlives the call.
        if unsafe { reylo_libsql_prepare(self.db, sql.as_ptr(), &mut stmt) } != SQLITE_OK {
            return Err(self.error());
        }
        if stmt.is_null() {
            return Err(Error("no SQL statement to run".into()));
        }
        Ok(Statement(stmt))
    }
}

impl Drop for Local {
    fn drop(&mut self) {
        // SAFETY: the handle is this connection's own, and every statement of it is finalized.
        unsafe { reylo_libsql_close(self.db) };
    }
}

/// A prepared statement, finalized when it is dropped.
struct Statement(*mut Stmt);

impl Drop for Statement {
    fn drop(&mut self) {
        // SAFETY: the statement is live and finalized once.
        unsafe { reylo_libsql_finalize(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_a_brief_lock() {
        let path = std::env::temp_dir().join(format!("reylo-libsql-lock-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let path = path.to_string_lossy().into_owned();
        let mut a = Local::open(&path).unwrap();
        a.exec("create table t (x integer); begin immediate; insert into t values (1)").unwrap();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            a.exec("commit").unwrap();
        });
        let mut b = Local::open(&path).unwrap();
        b.exec("insert into t values (2)").unwrap();
        writer.join().unwrap();
        assert_eq!(b.rows("select count(*) as n from t").unwrap()[0].text("n"), "2");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn runs_libsqls_own_sql() {
        let mut db = Local::memory().unwrap();
        assert_eq!(version(), "3.47.0");
        db.exec("create table docs (id integer primary key, emb F32_BLOB(3), note text); create index docs_emb on docs (libsql_vector_idx(emb))").unwrap();
        db.exec_with("insert into docs (id, emb, note) values (1, vector(?), ?)", &["[1,2,3]", "a"]).unwrap();
        db.exec("alter table docs alter column note to note text not null").unwrap();
        let rows = db.rows("select id, 1.0 as f, note, null as n, x'6869' as b from docs").unwrap();
        assert_eq!((rows[0].text("id"), rows[0].text("f"), rows[0].text("note"), rows[0].get("n"), rows[0].text("b")), ("1", "1", "a", None, "hi"));
        let names: Vec<String> = db.rows("select name from sqlite_schema order by name").unwrap().iter().map(|r| r.text("name").to_string()).collect();
        assert!(names.contains(&"libsql_vector_meta_shadow".to_string()), "{names:?}");
        let mut copy = Local::memory().unwrap();
        db.backup_to(&mut copy).unwrap();
        assert_eq!(copy.rows("select count(*) as n from docs").unwrap()[0].text("n"), "1");
        assert_eq!(db.exec("select * from nope").unwrap_err().0, "no such table: nope");
        // The extensions Turso Cloud loads, as it answers them.
        let r = &db.rows("select floor(1.5) as f, percentile(1, 50) as p, median(1) as m, length(uuid4()) as u, hex(sha256('a')) as h, fuzzy_leven('a', 'b') as l, text_length('a') as t, regexp_like('a', 'a') as r").unwrap()[0];
        assert_eq!([r.text("f"), r.text("p"), r.text("m"), r.text("u"), r.text("l"), r.text("t"), r.text("r")], ["1", "1", "1", "36", "1", "1", "1"]);
        assert_eq!(r.text("h"), "CA978112CA1BBDCAFAC231B39A23DC4DA786EFF8147C4E72B9807785AFEE48BB");
        assert!(db.rows("select geopoly_area('[[0,0],[1,0],[0,1],[0,0]]')").is_err(), "Turso Cloud has no geopoly");
    }
}
