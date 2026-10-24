//! SQLite's resolved parse tree, as `csrc/reylo_sqlite.c` serializes it: what SQLite itself
//! resolved while preparing the statement against an in-memory replay of the schema DDL.
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

/// The serializer keeps one schema replay and one capture in globals, so calls are serialized.
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tree {
    /// The top-level SELECT, or the SELECT that feeds an INSERT.
    pub statement: Option<Sel>,
    pub dml: Option<Dml>,
    pub returning: Option<Vec<Col>>,
    /// The public column metadata: the names tsc sees, the declared type and provenance.
    pub columns: Vec<PublicColumn>,
    pub params: Vec<PublicParam>,
    pub readonly: bool,
    /// More than one statement in the text.
    pub tail: bool,
    pub tables: HashMap<String, TableInfo>,
    pub error: Option<TreeError>,
    pub schema_errors: Vec<SchemaError>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TreeError {
    pub message: String,
    pub offset: i64,
    /// The length of what `offset` points at, where the serializer knows it.
    pub length: Option<usize>,
    /// The function, as the statement spells it, that the embedded SQLite has and the host's
    /// SQLite does not list; `view` names the view that calls it, when one does.
    pub missing_function: Option<String>,
    pub view: Option<String>,
    /// The schema of a `schema.table` the statement reads, where the database has no such schema.
    pub missing_schema: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SchemaError {
    pub index: usize,
    /// Set when the failed step registered a host function rather than replaying DDL.
    pub function: Option<String>,
    pub message: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PublicColumn {
    pub name: String,
    pub declared: Option<String>,
    pub table: Option<String>,
    pub column: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PublicParam {
    pub index: usize,
    pub name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TableInfo {
    pub strict: bool,
    pub without_rowid: bool,
    pub rowid_alias: Option<String>,
    pub columns: Vec<ColumnInfo>,
    pub unique_keys: Vec<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ColumnInfo {
    pub name: String,
    pub declared: String,
    pub affinity: String,
    pub not_null: bool,
    /// A CHECK of the table tests the column IS NOT NULL among its conjuncts: a write of NULL fails.
    pub checked_not_null: bool,
    pub pk: bool,
    pub generated: bool,
    pub hidden: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Sel {
    pub columns: Vec<Col>,
    /// The rows after the first of a multi-row VALUES, whose first row is `columns`.
    pub rows: Vec<Vec<Node>>,
    pub from: Vec<From>,
    #[serde(rename = "where")]
    pub where_: Option<Node>,
    pub group_by: Vec<Node>,
    pub having: Option<Node>,
    pub order_by: Vec<Node>,
    pub limit: Option<Node>,
    pub offset: Option<Node>,
    pub distinct: bool,
    pub has_agg: bool,
    pub aggregate: bool,
    pub values: bool,
    pub window: bool,
    /// The set operator between this arm and `prior`.
    pub op: Option<String>,
    pub prior: Option<Box<Sel>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Col {
    pub name: Option<String>,
    pub named: bool,
    pub expr: Node,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct From {
    pub cursor: i64,
    /// A table by the name its DDL spells, whatever case the statement writes it in, and a
    /// table-valued function by its module's name; a view, a WITH query or a subquery as written.
    pub table: Option<String>,
    pub alias: Option<String>,
    /// On the nullable side of an outer join.
    pub outer: bool,
    pub cross: bool,
    /// Joined by a RIGHT or FULL JOIN, which keeps the rows of the side before it that match nothing.
    pub right: bool,
    pub tab_func: bool,
    /// A table-valued function's arguments (`json_each(:ids)`).
    pub args: Vec<Node>,
    pub cte: bool,
    /// A recursive WITH query's read of itself, inside the query, which has no subquery of its own.
    pub recursive: bool,
    pub view: bool,
    #[serde(rename = "virtual")]
    pub virtual_: bool,
    pub subquery: Option<Box<Sel>>,
    pub on: Option<Node>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Dml {
    pub kind: String,
    pub table: String,
    pub on_error: String,
    pub upsert: Option<Upsert>,
    /// The target column per value position; `None` where SQLite could not name one.
    pub columns: Vec<Option<String>>,
    pub values: Option<Vec<Node>>,
    /// 1 for one VALUES row, the row count for a multi-row VALUES (whose rows are `statement`'s),
    /// -1 for a SELECT source.
    pub rows: i64,
    pub set: Vec<SetItem>,
    #[serde(rename = "where")]
    pub where_: Option<Node>,
    pub from_items: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Upsert {
    pub do_update: bool,
    pub set: Vec<SetItem>,
    #[serde(rename = "where")]
    pub where_: Option<Node>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SetItem {
    pub column: Option<String>,
    pub expr: Node,
}

/// One expression node. `op` names the kind; the other members are set where they apply.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Node {
    pub op: String,
    /// `sqlite3ExprAffinity`: integer, real, text, blob, numeric or none.
    pub aff: String,
    pub can_be_null: bool,
    /// A WHERE term that came from an outer join's ON clause, which SQLite moves into the WHERE:
    /// it restricts the join's right side (the cursor `join`) and removes no row.
    pub outer_on: bool,
    pub join: i64,
    // column
    pub cursor: i64,
    pub index: i64,
    pub table: Option<String>,
    pub column: Option<String>,
    pub declared: Option<String>,
    pub affinity: Option<String>,
    pub not_null: bool,
    pub rowid: bool,
    pub outer: bool,
    /// The column's table is STRICT.
    pub strict: bool,
    /// The column's collation, which a comparison with it uses where the column decides.
    pub coll: Option<String>,
    pub resolved: Option<bool>,
    // variable, function
    pub name: Option<String>,
    pub value: Option<String>,
    pub agg: bool,
    /// A window call, or an aggregate with a FILTER, which SQLite holds as a window as well.
    pub window: bool,
    /// A window call: one with an OVER clause.
    pub over: bool,
    pub distinct: bool,
    pub infix: bool,
    pub args: Vec<Node>,
    /// An aggregate's own ORDER BY, or a window call's.
    pub order_by: Vec<Node>,
    /// A window call's PARTITION BY.
    pub partition_by: Vec<Node>,
    /// A window call's frame: where it starts (`unbounded`, which is UNBOUNDED PRECEDING,
    /// `preceding`, `current` or `following`) and ends (`unbounded` is UNBOUNDED FOLLOWING), and
    /// what it excludes (`no`, `current`, `group` or `ties`), a named window's included.
    pub frame_start: Option<String>,
    pub frame_end: Option<String>,
    pub exclude: Option<String>,
    /// A window call with a FILTER.
    pub filter: bool,
    // cast
    pub to: Option<String>,
    // operands
    pub left: Option<Box<Node>>,
    pub right: Option<Box<Node>>,
    pub list: Vec<Node>,
    pub select: Option<Box<Sel>>,
}

extern "C" {
    fn reylo_sqlite_analyze(ddl: *const *const c_char, n: c_int, sql: *const c_char, functions: *const *const c_char, arities: *const c_int, n_functions: c_int, host: *const *const c_char, host_arities: *const c_int, host_kinds: *const c_char, n_host: c_int) -> *mut c_char;
    fn reylo_sqlite_free(p: *mut c_char);
}

/// A function the host's SQLite lists (`pragma_function_list`): its name, its argument count as
/// SQLite keeps it (-1 for any number) and its kind, `s` for a scalar, `a` for an aggregate, `w`
/// for a window function.
pub type Listed<'a> = (&'a str, i32, char);

/// Prepares `sql` against the schema `ddl` inside the embedded SQLite and returns what it resolved.
/// `functions` are the host's own scalar functions, by name and argument count (-1 for any): the
/// replay registers a stub for each, so a statement that calls one prepares. `host` is every
/// function the host's SQLite has, when the host could list them: the replay registers a stub for
/// one the embedded build has no function of that name for, and the prepare refuses a call to one
/// the embedded build has and the host does not.
pub fn analyze(ddl: &[String], sql: &str, functions: &[(&str, i32)], host: Option<&[Listed]>) -> Result<Tree, String> {
    let owned: Vec<CString> = ddl.iter().map(|d| CString::new(d.replace('\0', " ")).unwrap()).collect();
    let pointers: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
    let names: Vec<CString> = functions.iter().map(|(n, _)| CString::new(n.replace('\0', " ")).unwrap()).collect();
    let name_pointers: Vec<*const c_char> = names.iter().map(|c| c.as_ptr()).collect();
    let arities: Vec<c_int> = functions.iter().map(|(_, a)| *a as c_int).collect();
    let listed = host.unwrap_or_default();
    let host_names: Vec<CString> = listed.iter().map(|(n, _, _)| CString::new(n.replace('\0', " ")).unwrap()).collect();
    let host_pointers: Vec<*const c_char> = host_names.iter().map(|c| c.as_ptr()).collect();
    let host_arities: Vec<c_int> = listed.iter().map(|(_, a, _)| *a as c_int).collect();
    let host_kinds: Vec<c_char> = listed.iter().map(|(_, _, k)| if k.is_ascii() { *k as u8 as c_char } else { b's' as c_char }).collect();
    let n_host = if host.is_some() { listed.len() as c_int } else { -1 };
    let sql = CString::new(sql.replace('\0', " ")).map_err(|e| e.to_string())?;
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let json = unsafe {
        let p = reylo_sqlite_analyze(pointers.as_ptr(), pointers.len() as c_int, sql.as_ptr(), name_pointers.as_ptr(), arities.as_ptr(), name_pointers.len() as c_int, host_pointers.as_ptr(), host_arities.as_ptr(), host_kinds.as_ptr(), n_host);
        if p.is_null() {
            return Err("the embedded SQLite returned nothing".into());
        }
        let text = CStr::from_ptr(p).to_string_lossy().into_owned();
        reylo_sqlite_free(p);
        text
    };
    // The tree nests as deeply as the statement, which SQLite bounds at 1,000 levels of an
    // expression (SQLITE_MAX_EXPR_DEPTH), past serde_json's default limit of 128; the engine's
    // stack has the room for SQLite's own bound (`crate::stack`).
    let mut reader = serde_json::Deserializer::from_str(&json);
    reader.disable_recursion_limit();
    serde::Deserialize::deserialize(&mut reader).map_err(|e| format!("the embedded SQLite's answer did not parse: {e}"))
}
