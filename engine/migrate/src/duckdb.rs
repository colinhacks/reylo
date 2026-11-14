//! DuckDB, through its C API, from the DuckDB library the project's own `@duckdb/node-api` installs
//! (its platform package, `@duckdb/node-bindings-<platform>`, holds the shared library): the
//! migrations run on the DuckDB the app runs on, as the type check does (`src/duckdb.ts`), and the
//! binary carries none. `reylo` (src/cli.ts) finds the library as Node resolves the package and names
//! it in `REYLO_DUCKDB_LIBRARY`; a binary run on its own looks in the project's `node_modules`.
//!
//! One process opens a database file once: DuckDB refuses a second process, but a second instance in
//! the same process would write the file beside the first, so every connection to a file is one of a
//! single instance, which closes with its last connection.

use crate::Error;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

type Database = *mut c_void;
type Connection = *mut c_void;
type Prepared = *mut c_void;
type State = i32;
const SUCCESS: State = 0;

/// `duckdb_result`, whose fields the API reads through its functions alone.
#[repr(C)]
struct RawResult {
    deprecated_column_count: u64,
    deprecated_row_count: u64,
    deprecated_rows_changed: u64,
    deprecated_columns: *mut c_void,
    deprecated_error_message: *mut c_char,
    internal_data: *mut c_void,
}

impl RawResult {
    fn empty() -> RawResult {
        RawResult { deprecated_column_count: 0, deprecated_row_count: 0, deprecated_rows_changed: 0, deprecated_columns: std::ptr::null_mut(), deprecated_error_message: std::ptr::null_mut(), internal_data: std::ptr::null_mut() }
    }
}

/// The functions of the C API used here, each in DuckDB's library since its 0.x releases.
struct Api {
    _library: libloading::Library,
    open_ext: unsafe extern "C" fn(*const c_char, *mut Database, *mut c_void, *mut *mut c_char) -> State,
    close: unsafe extern "C" fn(*mut Database),
    connect: unsafe extern "C" fn(Database, *mut Connection) -> State,
    disconnect: unsafe extern "C" fn(*mut Connection),
    query: unsafe extern "C" fn(Connection, *const c_char, *mut RawResult) -> State,
    destroy_result: unsafe extern "C" fn(*mut RawResult),
    result_error: unsafe extern "C" fn(*mut RawResult) -> *const c_char,
    column_count: unsafe extern "C" fn(*mut RawResult) -> u64,
    row_count: unsafe extern "C" fn(*mut RawResult) -> u64,
    column_name: unsafe extern "C" fn(*mut RawResult, u64) -> *const c_char,
    value_is_null: unsafe extern "C" fn(*mut RawResult, u64, u64) -> bool,
    value_varchar: unsafe extern "C" fn(*mut RawResult, u64, u64) -> *mut c_char,
    free: unsafe extern "C" fn(*mut c_void),
    prepare: unsafe extern "C" fn(Connection, *const c_char, *mut Prepared) -> State,
    prepare_error: unsafe extern "C" fn(Prepared) -> *const c_char,
    bind_varchar: unsafe extern "C" fn(Prepared, u64, *const c_char) -> State,
    execute_prepared: unsafe extern "C" fn(Prepared, *mut RawResult) -> State,
    destroy_prepare: unsafe extern "C" fn(*mut Prepared),
    library_version: unsafe extern "C" fn() -> *const c_char,
}

/// The library and the error it fails to load with, once per process.
static API: OnceLock<Result<Api, String>> = OnceLock::new();
/// Where the project is, for the search of its `node_modules`: the first connection's root.
static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// The platform package of `@duckdb/node-bindings` for this binary's platform, and its library's file.
fn platform() -> (&'static str, &'static str) {
    let musl = cfg!(target_env = "musl");
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => ("darwin-arm64", "libduckdb.dylib"),
        ("macos", _) => ("darwin-x64", "libduckdb.dylib"),
        ("linux", "aarch64") if musl => ("linux-arm64-musl", "libduckdb.so"),
        ("linux", "aarch64") => ("linux-arm64", "libduckdb.so"),
        ("linux", _) if musl => ("linux-x64-musl", "libduckdb.so"),
        ("linux", _) => ("linux-x64", "libduckdb.so"),
        ("windows", "aarch64") => ("win32-arm64", "duckdb.dll"),
        _ => ("win32-x64", "duckdb.dll"),
    }
}

/// The DuckDB library: `REYLO_DUCKDB_LIBRARY`, else the platform package in a `node_modules` of the
/// project or above it, as npm, Yarn and Bun hoist it, or as pnpm keeps it in its store.
fn library_path(root: &Path) -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("REYLO_DUCKDB_LIBRARY").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let (package, file) = platform();
    let mut dir = Some(root);
    while let Some(at) = dir {
        let modules = at.join("node_modules");
        let hoisted = modules.join("@duckdb").join(format!("node-bindings-{package}")).join(file);
        if hoisted.is_file() {
            return Ok(hoisted);
        }
        for store in [".pnpm", ".store"] {
            let Ok(entries) = std::fs::read_dir(modules.join(store)) else { continue };
            let prefix = format!("@duckdb+node-bindings-{package}@");
            let mut found: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
                .map(|e| e.path().join("node_modules").join("@duckdb").join(format!("node-bindings-{package}")).join(file))
                .filter(|p| p.is_file())
                .collect();
            found.sort();
            if let Some(last) = found.pop() {
                return Ok(last);
            }
        }
        dir = at.parent();
    }
    Err(format!("DuckDB's migrations run on the project's own DuckDB, and {} has no @duckdb/node-api: npm install @duckdb/node-api", root.display()))
}

fn load(root: &Path) -> Result<Api, String> {
    let path = library_path(root)?;
    // SAFETY: DuckDB's library runs no code of its own on load beyond its static initializers, and
    // each symbol is read with the signature the C API's header gives it.
    unsafe {
        let library = libloading::Library::new(&path).map_err(|e| format!("cannot load DuckDB from {}: {e}", path.display()))?;
        macro_rules! symbol {
            ($name:literal) => {
                *library.get(concat!($name, "\0").as_bytes()).map_err(|e| format!("{} lacks {}: {e}", path.display(), $name))?
            };
        }
        Ok(Api {
            open_ext: symbol!("duckdb_open_ext"),
            close: symbol!("duckdb_close"),
            connect: symbol!("duckdb_connect"),
            disconnect: symbol!("duckdb_disconnect"),
            query: symbol!("duckdb_query"),
            destroy_result: symbol!("duckdb_destroy_result"),
            result_error: symbol!("duckdb_result_error"),
            column_count: symbol!("duckdb_column_count"),
            row_count: symbol!("duckdb_row_count"),
            column_name: symbol!("duckdb_column_name"),
            value_is_null: symbol!("duckdb_value_is_null"),
            value_varchar: symbol!("duckdb_value_varchar"),
            free: symbol!("duckdb_free"),
            prepare: symbol!("duckdb_prepare"),
            prepare_error: symbol!("duckdb_prepare_error"),
            bind_varchar: symbol!("duckdb_bind_varchar"),
            execute_prepared: symbol!("duckdb_execute_prepared"),
            destroy_prepare: symbol!("duckdb_destroy_prepare"),
            library_version: symbol!("duckdb_library_version"),
            _library: library,
        })
    }
}

fn api() -> Result<&'static Api, Error> {
    let root = ROOT.get().cloned().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    API.get_or_init(|| load(&root)).as_ref().map_err(|e| Error(e.clone()))
}

/// The version of DuckDB the project installs, as it names itself: `v2.0.0`.
pub fn version(root: &Path) -> Result<String, Error> {
    let _ = ROOT.set(root.to_path_buf());
    let api = api()?;
    // SAFETY: the version is a static string of the library's.
    Ok(unsafe { CStr::from_ptr((api.library_version)()) }.to_string_lossy().into_owned())
}

/// One open database: a file, or an empty one in memory.
struct Instance {
    db: Database,
}

// SAFETY: a DuckDB database handle may be used from any thread, by connections each of one thread.
unsafe impl Send for Instance {}
unsafe impl Sync for Instance {}

impl Drop for Instance {
    fn drop(&mut self) {
        if let Ok(api) = api() {
            // SAFETY: the handle came from duckdb_open_ext, and every connection to it is closed.
            unsafe { (api.close)(&mut self.db) };
        }
    }
}

/// The files this process has open, by path, each while a connection to it lives.
static OPEN: Mutex<Option<HashMap<PathBuf, Weak<Instance>>>> = Mutex::new(None);

/// A connection to a DuckDB database.
pub struct Conn {
    conn: Connection,
    /// Last, so that it closes after the connection.
    instance: Arc<Instance>,
}

impl Drop for Conn {
    fn drop(&mut self) {
        if let Ok(api) = api() {
            // SAFETY: the connection came from duckdb_connect on the instance this holds.
            unsafe { (api.disconnect)(&mut self.conn) };
        }
    }
}

fn c_string(text: &str) -> Result<CString, Error> {
    CString::new(text).map_err(|_| Error("a statement holds a NUL character, which DuckDB's API cannot take".into()))
}

/// What a failed call says, from a string the API owns.
/// DuckDB's error, with what it leaves unsaid where a name is ambiguous: a database file's name is
/// its database's up to the file name's first dot, so a schema of that name is read either way.
fn explained(error: String) -> String {
    if !error.contains("Ambiguous reference to catalog or schema") {
        return error;
    }
    let (first, rest) = error.split_once('\n').map_or((error.as_str(), ""), |(a, b)| (a, b));
    let why = "DuckDB names a database after its file, up to the file name's first dot (analytics.prod.duckdb is analytics), so a schema of that name is ambiguous: rename the schema or the file";
    if rest.is_empty() { format!("{first}. {why}") } else { format!("{first}. {why}\n{rest}") }
}

unsafe fn message(text: *const c_char, fallback: &str) -> String {
    if text.is_null() {
        fallback.to_string()
    } else {
        // SAFETY: the API's error strings are NUL-terminated and live as long as what they are of.
        unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
    }
}

impl Conn {
    /// A connection to the file at `path` (`:memory:` for a new empty database), whose project is at
    /// `root`. A file this process has open already is connected to again.
    pub fn open(path: &str, root: &Path) -> Result<Conn, Error> {
        let _ = ROOT.set(root.to_path_buf());
        let api = api()?;
        // The file's canonical path, or for one not made yet its directory's and its name, so that
        // the file is known by one name before and after DuckDB makes it.
        let canonical = |p: &Path| std::fs::canonicalize(p).ok().or_else(|| Some(std::fs::canonicalize(p.parent()?).ok()?.join(p.file_name()?)));
        let key = (path != ":memory:").then(|| canonical(Path::new(path)).unwrap_or_else(|| PathBuf::from(path)));
        // Held while the file is opened, so that two threads that open it at once share one instance.
        let mut open = OPEN.lock().unwrap_or_else(|e| e.into_inner());
        let shared = key.as_ref().and_then(|k| open.get_or_insert_with(HashMap::new).get(k).and_then(Weak::upgrade));
        let instance = match shared {
            Some(instance) => instance,
            None => {
                let mut db: Database = std::ptr::null_mut();
                let mut error: *mut c_char = std::ptr::null_mut();
                let c_path = c_string(path)?;
                // SAFETY: the out-pointers are valid; a failure's message is the API's to free.
                let state = unsafe { (api.open_ext)(c_path.as_ptr(), &mut db, std::ptr::null_mut(), &mut error) };
                if state != SUCCESS {
                    let said = unsafe { message(error, "DuckDB could not open the database") };
                    if !error.is_null() {
                        unsafe { (api.free)(error as *mut c_void) };
                    }
                    return Err(Error(said));
                }
                let instance = Arc::new(Instance { db });
                if let Some(k) = key {
                    open.get_or_insert_with(HashMap::new).insert(k, Arc::downgrade(&instance));
                }
                instance
            }
        };
        drop(open);
        let mut conn: Connection = std::ptr::null_mut();
        // SAFETY: the instance is open, and the out-pointer valid.
        if unsafe { (api.connect)(instance.db, &mut conn) } != SUCCESS {
            return Err(Error("DuckDB could not connect to the database".into()));
        }
        Ok(Conn { conn, instance })
    }

    /// Another connection to the same database, with a session and a transaction of its own.
    pub fn sibling(&self) -> Result<Conn, Error> {
        let api = api()?;
        let mut conn: Connection = std::ptr::null_mut();
        // SAFETY: the instance is open while this connection holds it, and the out-pointer is valid.
        if unsafe { (api.connect)(self.instance.db, &mut conn) } != SUCCESS {
            return Err(Error("DuckDB could not connect to the database".into()));
        }
        let mut side = Conn { conn, instance: self.instance.clone() };
        // The database this one uses, which an in-memory shadow attaches under the target's name.
        let (_, rows) = self_query(self.conn, api, "select current_database() as n")?;
        if let Some(Some(name)) = rows.first().map(|r| r[0].clone()) {
            side.query(&format!("use \"{}\"", name.replace('"', "\"\"")))?;
        }
        Ok(side)
    }

    /// Runs `sql`, one statement or several, and gives the last one's rows, each value as text (a
    /// boolean as `true` or `false`), with its columns' names.
    pub fn query(&mut self, sql: &str) -> Result<(Vec<String>, Vec<Vec<Option<String>>>), Error> {
        self_query(self.conn, api()?, sql)
    }

    /// Runs one statement with each `?` bound to a text value.
    pub fn execute(&mut self, sql: &str, params: &[&str]) -> Result<(), Error> {
        let api = api()?;
        let text = c_string(sql)?;
        let mut prepared: Prepared = std::ptr::null_mut();
        // SAFETY: the connection is open; the prepared statement is destroyed below in every case.
        let state = unsafe { (api.prepare)(self.conn, text.as_ptr(), &mut prepared) };
        let out = (|| {
            if state != SUCCESS {
                return Err(Error(unsafe { message((api.prepare_error)(prepared), "DuckDB could not prepare the statement") }));
            }
            for (k, value) in params.iter().enumerate() {
                let value = c_string(value)?;
                if unsafe { (api.bind_varchar)(prepared, k as u64 + 1, value.as_ptr()) } != SUCCESS {
                    return Err(Error(format!("DuckDB could not bind the statement's parameter {}", k + 1)));
                }
            }
            let mut result = RawResult::empty();
            let state = unsafe { (api.execute_prepared)(prepared, &mut result) };
            let out = unsafe { read(api, &mut result, state) };
            unsafe { (api.destroy_result)(&mut result) };
            out.map(|_| ())
        })();
        unsafe { (api.destroy_prepare)(&mut prepared) };
        out
    }
}

type Rows = (Vec<String>, Vec<Vec<Option<String>>>);

fn self_query(conn: Connection, api: &Api, sql: &str) -> Result<Rows, Error> {
    let text = c_string(sql)?;
    let mut result = RawResult::empty();
    // SAFETY: the connection is open, and the result is destroyed below, whatever the state.
    let state = unsafe { (api.query)(conn, text.as_ptr(), &mut result) };
    let out = unsafe { read(api, &mut result, state) };
    unsafe { (api.destroy_result)(&mut result) };
    out
}

/// A result's columns and rows, or its error.
unsafe fn read(api: &Api, result: &mut RawResult, state: State) -> Result<(Vec<String>, Vec<Vec<Option<String>>>), Error> {
    // SAFETY (for the calls below): `result` is a result the API filled, not yet destroyed.
    if state != SUCCESS {
        return Err(Error(explained(unsafe { message((api.result_error)(result), "DuckDB could not run the statement") })));
    }
    let columns = unsafe { (api.column_count)(result) };
    let rows = unsafe { (api.row_count)(result) };
    let names: Vec<String> = (0..columns).map(|c| unsafe { message((api.column_name)(result, c), "") }).collect();
    let mut out = Vec::with_capacity(rows as usize);
    for r in 0..rows {
        let mut row = Vec::with_capacity(columns as usize);
        for c in 0..columns {
            if unsafe { (api.value_is_null)(result, c, r) } {
                row.push(None);
                continue;
            }
            let value = unsafe { (api.value_varchar)(result, c, r) };
            // A value of a type with no text of its own is read as text by the query itself.
            row.push(if value.is_null() {
                None
            } else {
                let text = unsafe { CStr::from_ptr(value) }.to_string_lossy().into_owned();
                unsafe { (api.free)(value as *mut c_void) };
                Some(text)
            });
        }
        out.push(row);
    }
    Ok((names, out))
}

/// The path a DuckDB URL names, as the type check reads it (`duckdbPath` in src/project.ts):
/// `duckdb:./app.duckdb` and `duckdb:///abs/app.duckdb`, a relative one in the project; `duckdb:` or
/// `duckdb::memory:` alone is an empty database in memory.
pub fn path_of(url: &str, base: &Path) -> String {
    let rest = &url[url.find(':').map_or(0, |at| at + 1)..];
    let mut raw = rest.strip_prefix("//").unwrap_or(rest).to_string();
    if raw.is_empty() || raw == ":memory:" {
        return ":memory:".into();
    }
    // On Windows, `duckdb:///C:/app.duckdb` names the drive, as a file URL does.
    if cfg!(windows) && raw.starts_with('/') && raw.as_bytes().get(2) == Some(&b':') {
        raw.remove(0);
    }
    if Path::new(&raw).is_absolute() {
        raw
    } else {
        base.join(raw).to_string_lossy().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_path_as_the_type_check_does() {
        let base = Path::new("/app");
        assert_eq!(path_of("duckdb:./dev.duckdb", base), "/app/./dev.duckdb");
        assert_eq!(path_of("duckdb:///data/dev.duckdb", base), "/data/dev.duckdb");
        assert_eq!(path_of("duckdb::memory:", base), ":memory:");
        assert_eq!(path_of("duckdb:", base), ":memory:");
        assert_eq!(path_of("DUCKDB:dev.duckdb", base), "/app/dev.duckdb");
    }
}
