//! libSQL's engine, Turso's fork of SQLite, which a SQLite case with a `libsql` file runs on: the
//! amalgamation of `libsql-ffi` 0.10.0, SQLite 3.47.0 with libSQL's vectors (`build.rs`,
//! `csrc/libsql.c`). Its API is private to its own translation unit, beside rusqlite's SQLite, so a
//! connection is reached through the few functions `csrc/libsql.c` exports, and answers the seeding
//! as a rusqlite connection does ([`crate::seed::SqliteDb`]): its values are rusqlite's, and its
//! errors rusqlite's `SqliteFailure` with libSQL's code and message.

use crate::seed::SqliteDb;
use rusqlite::types::Value;
use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;
use std::time::Instant;

#[repr(C)]
struct Db {
    _private: [u8; 0],
}
#[repr(C)]
struct Stmt {
    _private: [u8; 0],
}

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;
const SQLITE_INTEGER: c_int = 1;
const SQLITE_FLOAT: c_int = 2;
const SQLITE_TEXT: c_int = 3;
const SQLITE_BLOB: c_int = 4;

extern "C" {
    fn reylo_oracle_libsql_open(path: *const c_char, db: *mut *mut Db) -> c_int;
    fn reylo_oracle_libsql_close(db: *mut Db) -> c_int;
    fn reylo_oracle_libsql_errmsg(db: *mut Db) -> *const c_char;
    fn reylo_oracle_libsql_extended_errcode(db: *mut Db) -> c_int;
    fn reylo_oracle_libsql_version() -> *const c_char;
    fn reylo_oracle_libsql_autocommit(db: *mut Db) -> c_int;
    fn reylo_oracle_libsql_changes(db: *mut Db) -> c_int;
    fn reylo_oracle_libsql_progress(db: *mut Db, n: c_int, callback: Option<extern "C" fn(*mut c_void) -> c_int>, arg: *mut c_void);
    fn reylo_oracle_libsql_prepare(db: *mut Db, sql: *const c_char, stmt: *mut *mut Stmt, tail: *mut *const c_char) -> c_int;
    fn reylo_oracle_libsql_finalize(stmt: *mut Stmt) -> c_int;
    fn reylo_oracle_libsql_step(stmt: *mut Stmt) -> c_int;
    fn reylo_oracle_libsql_bind_null(stmt: *mut Stmt, i: c_int) -> c_int;
    fn reylo_oracle_libsql_bind_int64(stmt: *mut Stmt, i: c_int, v: i64) -> c_int;
    fn reylo_oracle_libsql_bind_double(stmt: *mut Stmt, i: c_int, v: f64) -> c_int;
    fn reylo_oracle_libsql_bind_text(stmt: *mut Stmt, i: c_int, v: *const c_char, n: c_int) -> c_int;
    fn reylo_oracle_libsql_bind_blob(stmt: *mut Stmt, i: c_int, v: *const c_void, n: c_int) -> c_int;
    fn reylo_oracle_libsql_column_count(stmt: *mut Stmt) -> c_int;
    fn reylo_oracle_libsql_column_name(stmt: *mut Stmt, i: c_int) -> *const c_char;
    fn reylo_oracle_libsql_column_type(stmt: *mut Stmt, i: c_int) -> c_int;
    fn reylo_oracle_libsql_column_int64(stmt: *mut Stmt, i: c_int) -> i64;
    fn reylo_oracle_libsql_column_double(stmt: *mut Stmt, i: c_int) -> f64;
    fn reylo_oracle_libsql_column_blob(stmt: *mut Stmt, i: c_int) -> *const c_void;
    fn reylo_oracle_libsql_column_text(stmt: *mut Stmt, i: c_int) -> *const u8;
    fn reylo_oracle_libsql_column_bytes(stmt: *mut Stmt, i: c_int) -> c_int;
}

/// The version of the libSQL the cases run on: its SQLite's, as `3.47.0`.
pub fn version() -> String {
    unsafe { CStr::from_ptr(reylo_oracle_libsql_version()) }.to_string_lossy().into_owned()
}

/// A connection to a database file on libSQL's engine.
pub struct Connection {
    db: *mut Db,
    /// The instant the progress handler interrupts a run at, while one is set.
    deadline: RefCell<Option<Box<Instant>>>,
}

extern "C" fn past(arg: *mut c_void) -> c_int {
    let at = unsafe { &*(arg as *const Instant) };
    c_int::from(Instant::now() >= *at)
}

impl Connection {
    /// The file at `path`, made where there is none. Foreign keys are off, as they are on SQLite
    /// by default, where libSQL's build turns them on: a case seeds rows that reference nothing.
    pub fn open(path: &Path) -> Result<Connection, rusqlite::Error> {
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| failure(1, e.to_string()))?;
        let mut db = std::ptr::null_mut();
        let rc = unsafe { reylo_oracle_libsql_open(path.as_ptr(), &mut db) };
        let connection = Connection { db, deadline: RefCell::new(None) };
        if rc != SQLITE_OK {
            return Err(connection.error());
        }
        connection.run_batch("pragma foreign_keys = off")?;
        Ok(connection)
    }

    fn error(&self) -> rusqlite::Error {
        let code = unsafe { reylo_oracle_libsql_extended_errcode(self.db) };
        let message = unsafe { CStr::from_ptr(reylo_oracle_libsql_errmsg(self.db)) }.to_string_lossy().into_owned();
        failure(code, message)
    }

    /// Each statement of `sql` prepared in turn, handed to `each` with the step it is in.
    fn statements(&self, sql: &str, mut each: impl FnMut(&Statement) -> Result<(), rusqlite::Error>) -> Result<(), rusqlite::Error> {
        let text = CString::new(sql).map_err(|e| failure(1, e.to_string()))?;
        let mut rest = text.as_ptr();
        loop {
            let mut stmt = std::ptr::null_mut();
            let mut tail = std::ptr::null();
            if unsafe { reylo_oracle_libsql_prepare(self.db, rest, &mut stmt, &mut tail) } != SQLITE_OK {
                return Err(self.error());
            }
            if !stmt.is_null() {
                let statement = Statement { connection: self, stmt };
                each(&statement)?;
            }
            if tail.is_null() || unsafe { *tail } == 0 {
                return Ok(());
            }
            rest = tail;
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            reylo_oracle_libsql_progress(self.db, 0, None, std::ptr::null_mut());
            reylo_oracle_libsql_close(self.db);
        }
    }
}

/// One prepared statement, finalized when it is dropped.
struct Statement<'c> {
    connection: &'c Connection,
    stmt: *mut Stmt,
}

impl Drop for Statement<'_> {
    fn drop(&mut self) {
        unsafe { reylo_oracle_libsql_finalize(self.stmt) };
    }
}

impl Statement<'_> {
    fn bind(&self, values: &[Value]) -> Result<(), rusqlite::Error> {
        for (i, v) in values.iter().enumerate() {
            let at = i as c_int + 1;
            let rc = unsafe {
                match v {
                    Value::Null => reylo_oracle_libsql_bind_null(self.stmt, at),
                    Value::Integer(n) => reylo_oracle_libsql_bind_int64(self.stmt, at, *n),
                    Value::Real(f) => reylo_oracle_libsql_bind_double(self.stmt, at, *f),
                    Value::Text(t) => reylo_oracle_libsql_bind_text(self.stmt, at, t.as_ptr() as *const c_char, t.len() as c_int),
                    Value::Blob(b) => reylo_oracle_libsql_bind_blob(self.stmt, at, b.as_ptr() as *const c_void, b.len() as c_int),
                }
            };
            if rc != SQLITE_OK {
                return Err(self.connection.error());
            }
        }
        Ok(())
    }

    /// Steps once: a row's values, or `None` once it is done.
    fn step(&self) -> Result<Option<Vec<Value>>, rusqlite::Error> {
        match unsafe { reylo_oracle_libsql_step(self.stmt) } {
            SQLITE_DONE => Ok(None),
            SQLITE_ROW => {
                let width = unsafe { reylo_oracle_libsql_column_count(self.stmt) };
                Ok(Some((0..width).map(|i| self.value(i)).collect()))
            }
            _ => Err(self.connection.error()),
        }
    }

    fn value(&self, i: c_int) -> Value {
        unsafe {
            match reylo_oracle_libsql_column_type(self.stmt, i) {
                SQLITE_INTEGER => Value::Integer(reylo_oracle_libsql_column_int64(self.stmt, i)),
                SQLITE_FLOAT => Value::Real(reylo_oracle_libsql_column_double(self.stmt, i)),
                SQLITE_TEXT => {
                    let p = reylo_oracle_libsql_column_text(self.stmt, i);
                    let n = reylo_oracle_libsql_column_bytes(self.stmt, i) as usize;
                    Value::Text(if p.is_null() { String::new() } else { String::from_utf8_lossy(std::slice::from_raw_parts(p, n)).into_owned() })
                }
                SQLITE_BLOB => {
                    let p = reylo_oracle_libsql_column_blob(self.stmt, i) as *const u8;
                    let n = reylo_oracle_libsql_column_bytes(self.stmt, i) as usize;
                    Value::Blob(if p.is_null() { Vec::new() } else { std::slice::from_raw_parts(p, n).to_vec() })
                }
                _ => Value::Null,
            }
        }
    }
}

/// An error as rusqlite reports one, with libSQL's extended code and message.
fn failure(code: c_int, message: String) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), Some(message))
}

impl SqliteDb for Connection {
    fn run_batch(&self, sql: &str) -> Result<(), rusqlite::Error> {
        self.statements(sql, |st| {
            while st.step()?.is_some() {}
            Ok(())
        })
    }

    fn run(&self, sql: &str, values: &[Value]) -> Result<usize, rusqlite::Error> {
        let mut changed = 0;
        self.statements(sql, |st| {
            st.bind(values)?;
            while st.step()?.is_some() {}
            changed = unsafe { reylo_oracle_libsql_changes(self.db) } as usize;
            Ok(())
        })?;
        Ok(changed)
    }

    fn each_row(&self, sql: &str, values: &[Value], each: &mut dyn FnMut(&[Value])) -> Result<(), rusqlite::Error> {
        self.statements(sql, |st| {
            st.bind(values)?;
            while let Some(row) = st.step()? {
                each(&row);
            }
            Ok(())
        })
    }

    fn column_names(&self, sql: &str) -> Option<Vec<String>> {
        let mut names = None;
        self.statements(sql, |st| {
            let width = unsafe { reylo_oracle_libsql_column_count(st.stmt) };
            names = Some((0..width).map(|i| unsafe { CStr::from_ptr(reylo_oracle_libsql_column_name(st.stmt, i)) }.to_string_lossy().into_owned()).collect());
            Ok(())
        })
        .ok()?;
        names
    }

    fn in_autocommit(&self) -> bool {
        unsafe { reylo_oracle_libsql_autocommit(self.db) != 0 }
    }

    fn set_deadline(&self, at: Option<Instant>) {
        let mut deadline = self.deadline.borrow_mut();
        unsafe { reylo_oracle_libsql_progress(self.db, 0, None, std::ptr::null_mut()) };
        *deadline = at.map(Box::new);
        if let Some(at) = deadline.as_ref() {
            unsafe { reylo_oracle_libsql_progress(self.db, 1000, Some(past), &**at as *const Instant as *mut c_void) };
        }
    }
}
