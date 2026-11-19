//! SQLite's resolved parse tree, as `csrc/reylo_sqlite.c` serializes it: what SQLite itself
//! resolved while preparing the statement against an in-memory replay of the schema DDL.
//!
//! Every member the walker always writes is required, so that one it stops writing, or spells
//! otherwise, fails the analysis with an error naming it, where a default would read it as
//! `false`, `0` or nothing: for a fact such as "on the null-extended side of a join", the unsound
//! answer. A member the walker writes for some objects only has `#[serde(default)]` and says when
//! it is absent; where that turns on the object's kind (a node's `op`, a write's `kind`), the
//! members of each kind are required once the object is read ([`Members`]). A member no struct
//! declares is refused, so the walker writes nothing the analysis does not read.
use serde::de::{self, value::MapAccessDeserializer, DeserializeSeed, Deserializer, IntoDeserializer, MapAccess, Visitor};
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

/// The serializer keeps one schema replay and one capture in globals, so calls are serialized.
static LOCK: Mutex<()> = Mutex::new(());

/// The version of the tree's format, which the walker writes as the tree's first member
/// (`REYLO_TREE_VERSION` in `csrc/reylo_sqlite.c`): a tree of another is refused, by both numbers,
/// before anything in it is read. A change to what either side reads or writes moves both.
pub const TREE_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(remote = "Self", rename_all = "camelCase", deny_unknown_fields)]
pub struct Tree {
    #[serde(deserialize_with = "tree_version")]
    pub version: u32,
    // Absent, with `columns`, `params`, `readonly` and `tables`, when `error` is set.
    /// The top-level SELECT, or the SELECT that feeds an INSERT.
    #[serde(default)]
    pub statement: Option<Sel>,
    #[serde(default)]
    pub dml: Option<Dml>,
    #[serde(default)]
    pub returning: Option<Vec<Col>>,
    /// The public column metadata: the names tsc sees and the declared type.
    #[serde(default)]
    pub columns: Vec<PublicColumn>,
    #[serde(default)]
    pub params: Vec<PublicParam>,
    #[serde(default)]
    pub readonly: bool,
    #[serde(default)]
    pub tables: HashMap<String, TableInfo>,
    /// Absent when the statement prepared.
    #[serde(default)]
    pub error: Option<TreeError>,
    pub schema_errors: Vec<SchemaError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TreeError {
    pub message: String,
    pub offset: i64,
    /// The length of what `offset` points at, where the serializer knows it; absent otherwise.
    #[serde(default)]
    pub length: Option<usize>,
    /// The function, as the statement spells it, that the embedded SQLite has and the host's
    /// SQLite does not list; `view` names the view that calls it, when one does. Each is absent
    /// where it names nothing.
    #[serde(default)]
    pub missing_function: Option<String>,
    #[serde(default)]
    pub view: Option<String>,
    /// The schema of a `schema.table` the statement reads, where the database has no such
    /// schema; absent otherwise.
    #[serde(default)]
    pub missing_schema: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(remote = "Self", rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaError {
    /// The DDL step that failed; absent where `function` is set.
    #[serde(default)]
    pub index: usize,
    /// Set when the failed step registered a host function rather than replaying DDL; absent
    /// otherwise.
    #[serde(default)]
    pub function: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicColumn {
    pub name: String,
    #[serde(deserialize_with = "present")]
    pub declared: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicParam {
    #[serde(deserialize_with = "present")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TableInfo {
    pub strict: bool,
    #[serde(deserialize_with = "present")]
    pub rowid_alias: Option<String>,
    /// A NOT NULL, UNIQUE or PRIMARY KEY of the table is declared ON CONFLICT IGNORE: a write
    /// that breaks it, with no OR clause of its own, skips the row.
    pub ignores: bool,
    /// The PRIMARY KEY or a UNIQUE of the table is declared ON CONFLICT REPLACE: a write that
    /// breaks it, with no OR clause of its own, deletes the row it conflicts with.
    pub replaces: bool,
    pub columns: Vec<ColumnInfo>,
    pub unique_keys: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColumnInfo {
    pub name: String,
    pub declared: String,
    pub affinity: String,
    pub not_null: bool,
    /// A CHECK of the table tests the column IS NOT NULL among its conjuncts: a write of NULL fails.
    pub checked_not_null: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sel {
    pub columns: Vec<Col>,
    /// The rows after the first of a multi-row VALUES, whose first row is `columns`; absent for
    /// any other SELECT.
    #[serde(default)]
    pub rows: Vec<Vec<Node>>,
    pub from: Vec<From>,
    #[serde(rename = "where", deserialize_with = "present")]
    pub where_: Option<Node>,
    pub group_by: Vec<Node>,
    #[serde(deserialize_with = "present")]
    pub having: Option<Node>,
    pub order_by: Vec<Node>,
    #[serde(deserialize_with = "present")]
    pub limit: Option<Node>,
    #[serde(deserialize_with = "present")]
    pub offset: Option<Node>,
    pub distinct: bool,
    pub aggregate: bool,
    pub values: bool,
    pub window: bool,
    /// The set operator between this arm and `prior`; both are absent from an arm with none
    /// before it.
    #[serde(default)]
    pub op: Option<String>,
    #[serde(default)]
    pub prior: Option<Box<Sel>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Col {
    #[serde(deserialize_with = "present")]
    pub name: Option<String>,
    pub named: bool,
    pub expr: Node,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct From {
    pub cursor: i64,
    /// A table by the name its DDL spells, whatever case the statement writes it in, and a
    /// table-valued function by its module's name; a view, a WITH query or a subquery as written.
    #[serde(deserialize_with = "present")]
    pub table: Option<String>,
    #[serde(deserialize_with = "present")]
    pub alias: Option<String>,
    /// On the nullable side of an outer join.
    pub outer: bool,
    /// Joined by a RIGHT or FULL JOIN, which keeps the rows of the side before it that match nothing.
    pub right: bool,
    /// Before a RIGHT or FULL JOIN, which null-extends it.
    pub left_of_right: bool,
    pub tab_func: bool,
    /// A table-valued function's arguments (`json_each(:ids)`); empty for anything else.
    pub args: Vec<Node>,
    pub cte: bool,
    /// A recursive WITH query's read of itself, inside the query, which has no subquery of its own.
    pub recursive: bool,
    pub view: bool,
    #[serde(rename = "virtual")]
    pub virtual_: bool,
    #[serde(deserialize_with = "present")]
    pub subquery: Option<Box<Sel>>,
    #[serde(deserialize_with = "present")]
    pub on: Option<Node>,
}

/// An INSERT, an UPDATE (`kind` `update`, or `upsert` for an upsert's DO UPDATE) or a DELETE: each
/// kind's members are required by [`Members`], and absent from the other kinds.
#[derive(Debug, Deserialize)]
#[serde(remote = "Self", rename_all = "camelCase", deny_unknown_fields)]
pub struct Dml {
    pub kind: String,
    pub table: String,
    // An INSERT's and an UPDATE's.
    #[serde(default)]
    pub on_error: String,
    // An INSERT's.
    /// Each ON CONFLICT clause, in the order SQLite tries them.
    #[serde(default)]
    pub upserts: Vec<Upsert>,
    /// The target column per value position; `None` where SQLite could not name one.
    #[serde(default)]
    pub columns: Vec<Option<String>>,
    #[serde(default)]
    pub values: Option<Vec<Node>>,
    /// 1 for one VALUES row, the row count for a multi-row VALUES (whose rows are `statement`'s),
    /// -1 for a SELECT source.
    #[serde(default)]
    pub rows: i64,
    // An UPDATE's.
    #[serde(default)]
    pub set: Vec<SetItem>,
    // An UPDATE's and a DELETE's.
    #[serde(rename = "where", default)]
    pub where_: Option<Node>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Upsert {
    pub do_update: bool,
    pub set: Vec<SetItem>,
    #[serde(rename = "where", deserialize_with = "present")]
    pub where_: Option<Node>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetItem {
    #[serde(deserialize_with = "present")]
    pub column: Option<String>,
    pub expr: Node,
}

/// One expression node. `op` names the kind; `op`, `aff`, `outerOn` and `innerOn` are every node's,
/// and the members after them belong to some kinds only, absent from the others: [`Members`]
/// requires the ones a node's kind always has.
#[derive(Debug, Deserialize)]
#[serde(remote = "Self", rename_all = "camelCase", deny_unknown_fields)]
pub struct Node {
    pub op: String,
    /// `sqlite3ExprAffinity`: integer, real, text, blob, numeric or none.
    pub aff: String,
    /// A WHERE term that came from an outer join's ON clause, which SQLite moves into the WHERE:
    /// it restricts the join's right side (the cursor `join`, present with it) and removes no row.
    pub outer_on: bool,
    #[serde(default)]
    pub join: i64,
    /// A WHERE term that came from an inner join's ON clause.
    pub inner_on: bool,
    // A column's (`column`), and `index` a variable's and a select_column's too. A column SQLite
    // resolved to no table has `table` null and nothing after `register`; a rowid read by no
    // declared column has no `declared` and no `coll`.
    #[serde(default)]
    pub cursor: i64,
    #[serde(default)]
    pub index: i64,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub column: Option<String>,
    #[serde(default)]
    pub declared: Option<String>,
    /// A column's, and a cast's: the affinity it converts to.
    #[serde(default)]
    pub affinity: Option<String>,
    #[serde(default)]
    pub not_null: bool,
    #[serde(default)]
    pub rowid: bool,
    #[serde(default)]
    pub outer: bool,
    /// The column's table is STRICT.
    #[serde(default)]
    pub strict: bool,
    /// The column is generated.
    #[serde(default)]
    pub generated: bool,
    /// The column is VIRTUAL and computed by a function or from another generated column: its
    /// value, computed where it is read, may carry the JSON subtype.
    #[serde(default)]
    pub subtype: bool,
    /// The column is a view's, as a write through the view's INSTEAD OF trigger reads it.
    #[serde(default)]
    pub view: bool,
    /// The column of the row the statement wrote, as its RETURNING reads it: from the registers
    /// that hold the row, with no cursor (`cursor` is a register's number).
    #[serde(default)]
    pub register: bool,
    /// The column's collation, which a comparison with it uses where the column decides.
    #[serde(default)]
    pub coll: Option<String>,
    // A variable's and a function's.
    #[serde(default)]
    pub name: Option<String>,
    // A literal's.
    #[serde(default)]
    pub value: Option<String>,
    // A function's.
    #[serde(default)]
    pub agg: bool,
    /// A window call, or an aggregate with a FILTER, which SQLite holds as a window as well.
    #[serde(default)]
    pub window: bool,
    /// A window call: one with an OVER clause, which has every member from `partition_by` to
    /// `filter`.
    #[serde(default)]
    pub over: bool,
    #[serde(default)]
    pub distinct: bool,
    #[serde(default)]
    pub args: Vec<Node>,
    /// An aggregate's own ORDER BY, absent where it has none, or a window call's.
    #[serde(default)]
    pub order_by: Vec<Node>,
    /// A window call's PARTITION BY.
    #[serde(default)]
    pub partition_by: Vec<Node>,
    /// A window call's frame: where it starts (`unbounded`, which is UNBOUNDED PRECEDING,
    /// `preceding`, `current` or `following`) and ends (`unbounded` is UNBOUNDED FOLLOWING), and
    /// what it excludes (`no`, `current`, `group` or `ties`), a named window's included.
    #[serde(default)]
    pub frame_start: Option<String>,
    #[serde(default)]
    pub frame_end: Option<String>,
    #[serde(default)]
    pub exclude: Option<String>,
    /// A window call with a FILTER.
    #[serde(default)]
    pub filter: bool,
    // The operands: `left` a cast's, an IN's, a BETWEEN's, a CASE's, a row value's and a
    // select_column's, null where it has none; `list` an IN's (or `select`), a BETWEEN's, a CASE's
    // and a row value's; `select` a subquery's. Any other operator writes the operands it has.
    #[serde(default)]
    pub left: Option<Box<Node>>,
    #[serde(default)]
    pub right: Option<Box<Node>>,
    #[serde(default)]
    pub list: Vec<Node>,
    #[serde(default)]
    pub select: Option<Box<Sel>>,
}

/// An object whose members turn on its kind, which its struct, holding every kind's members, cannot
/// require: once it is read, each member [`Members::required`] names must have been in it.
trait Members: Sized {
    /// The struct's own reader, which `#[serde(remote = "Self")]` derives.
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error>;
    /// What it is, for a message: `a column node`.
    fn kind(&self) -> String;
    /// The members an object of its kind always has.
    fn required(&self) -> Vec<&'static str>;
}

impl Members for Tree {
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Tree::deserialize(d)
    }
    fn kind(&self) -> String {
        "the tree of a statement that prepared".into()
    }
    fn required(&self) -> Vec<&'static str> {
        if self.error.is_some() { Vec::new() } else { vec!["statement", "dml", "returning", "columns", "params", "readonly", "tables"] }
    }
}

impl Members for SchemaError {
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        SchemaError::deserialize(d)
    }
    fn kind(&self) -> String {
        "a failed DDL step".into()
    }
    fn required(&self) -> Vec<&'static str> {
        if self.function.is_some() { Vec::new() } else { vec!["index"] }
    }
}

impl Members for Dml {
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Dml::deserialize(d)
    }
    fn kind(&self) -> String {
        format!("a write of kind `{}`", self.kind)
    }
    fn required(&self) -> Vec<&'static str> {
        match self.kind.as_str() {
            "insert" => vec!["onError", "upserts", "columns", "values", "rows"],
            "update" | "upsert" => vec!["onError", "set", "where"],
            "delete" => vec!["where"],
            _ => Vec::new(),
        }
    }
}

impl Members for Node {
    fn read<'de, D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Node::deserialize(d)
    }
    fn kind(&self) -> String {
        format!("a `{}` node", self.op)
    }
    fn required(&self) -> Vec<&'static str> {
        let mut members = Vec::new();
        if self.outer_on {
            members.push("join");
        }
        match self.op.as_str() {
            "column" => {
                members.extend(["cursor", "index", "table", "register"]);
                if self.table.is_some() {
                    members.extend(["column", "affinity", "notNull", "rowid", "outer", "strict", "view", "generated", "subtype"]);
                }
            }
            "variable" => members.extend(["name", "index"]),
            "integer" | "float" | "string" | "blob" | "truefalse" | "id" => members.push("value"),
            "function" => {
                members.extend(["name", "agg", "window", "over", "distinct", "args"]);
                if self.over {
                    members.extend(["partitionBy", "orderBy", "frameStart", "frameEnd", "exclude", "filter"]);
                }
            }
            "cast" => members.extend(["affinity", "left"]),
            "in" => members.push("left"),
            "between" | "case" | "vector" => members.extend(["left", "list"]),
            "select" | "exists" => members.push("select"),
            "select_column" => members.extend(["index", "left"]),
            _ => {}
        }
        members
    }
}

macro_rules! read_with_members {
    ($($t:ty),*) => {$(
        impl<'de> Deserialize<'de> for $t {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                d.deserialize_map(Checked(PhantomData))
            }
        }
    )*};
}
read_with_members!(Tree, SchemaError, Dml, Node);

/// Reads an object through [`Members::read`], noting each member's name, then refuses it if a
/// member [`Members::required`] names was not among them.
struct Checked<T>(PhantomData<T>);

impl<'de, T: Members> Visitor<'de> for Checked<T> {
    type Value = T;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an object")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
        let mut seen = Vec::new();
        let value = T::read(MapAccessDeserializer::new(Noting { map, seen: &mut seen }))?;
        match value.required().into_iter().find(|member| !seen.iter().any(|s| s == member)) {
            Some(member) => Err(de::Error::custom(format!("missing field `{member}` of {}", value.kind()))),
            None => Ok(value),
        }
    }
}

/// A map's entries, as they are, with each key's name noted.
struct Noting<'a, A> {
    map: A,
    seen: &'a mut Vec<String>,
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for Noting<'_, A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, A::Error> {
        let Some(key) = self.map.next_key::<String>()? else { return Ok(None) };
        let read = seed.deserialize(<&str as IntoDeserializer<'de, A::Error>>::into_deserializer(key.as_str()))?;
        self.seen.push(key);
        Ok(Some(read))
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, A::Error> {
        self.map.next_value_seed(seed)
    }
}

/// The tree's version, refused unless it is [`TREE_VERSION`].
fn tree_version<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let version = u32::deserialize(d)?;
    if version != TREE_VERSION {
        return Err(de::Error::custom(format!("the tree is of version {version}, and this engine reads version {TREE_VERSION}")));
    }
    Ok(version)
}

/// A member the walker always writes, null or not. Serde reads a missing `Option` as `None`, unless
/// a function of the member's own reads it, as this one does, changing nothing else.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

extern "C" {
    fn reylo_sqlite_analyze(ddl: *const *const c_char, n: c_int, sql: *const c_char, sent: *const c_char, functions: *const *const c_char, arities: *const c_int, n_functions: c_int, host: *const *const c_char, host_arities: *const c_int, host_kinds: *const c_char, n_host: c_int) -> *mut c_char;
    fn reylo_sqlite_free(p: *mut c_char);
    fn reylo_sqlite_version() -> *const c_char;
}

/// The version of the embedded SQLite, as `3.53.4`: the one that prepares every statement the
/// engine types, whichever SQLite reads the database file or runs the statement.
pub fn version() -> &'static str {
    // A string constant of the library.
    unsafe { CStr::from_ptr(reylo_sqlite_version()) }.to_str().unwrap_or_default()
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
/// the embedded build has and the host does not. `sent` is the statement as the application runs it,
/// whose text names the columns.
pub fn analyze(ddl: &[String], sql: &str, sent: &str, functions: &[(&str, i32)], host: Option<&[Listed]>) -> Result<Tree, String> {
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
    let sent = CString::new(sent.replace('\0', " ")).map_err(|e| e.to_string())?;
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let json = unsafe {
        let p = reylo_sqlite_analyze(pointers.as_ptr(), pointers.len() as c_int, sql.as_ptr(), sent.as_ptr(), name_pointers.as_ptr(), arities.as_ptr(), name_pointers.len() as c_int, host_pointers.as_ptr(), host_arities.as_ptr(), host_kinds.as_ptr(), n_host);
        if p.is_null() {
            return Err("the embedded SQLite returned nothing".into());
        }
        let text = CStr::from_ptr(p).to_string_lossy().into_owned();
        reylo_sqlite_free(p);
        text
    };
    parse(&json)
}

/// The walker's answer as a [`Tree`], or why it is not one: a member missing or not declared names it.
fn parse(json: &str) -> Result<Tree, String> {
    // The tree nests as deeply as the statement, which SQLite bounds at 1,000 levels of an
    // expression (SQLITE_MAX_EXPR_DEPTH), past serde_json's default limit of 128; the engine's
    // stack has the room for SQLite's own bound (`crate::stack`).
    let mut reader = serde_json::Deserializer::from_str(json);
    reader.disable_recursion_limit();
    <Tree as Deserialize>::deserialize(&mut reader).map_err(|e| format!("the embedded SQLite's answer did not parse: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// A prepared statement's tree, `select a from t`, as the walker writes it.
    fn tree() -> Value {
        let column = json!({ "op": "column", "aff": "none", "outerOn": false, "innerOn": false, "cursor": 0, "index": 0, "register": false, "table": "t", "column": "a", "declared": "", "affinity": "blob", "coll": "BINARY", "notNull": false, "rowid": false, "generated": false, "subtype": false, "outer": false, "strict": false, "view": false });
        let from = json!({ "cursor": 0, "table": "t", "alias": null, "outer": false, "right": false, "leftOfRight": false, "tabFunc": false, "args": [], "cte": false, "recursive": false, "view": false, "virtual": false, "subquery": null, "on": null });
        let statement = json!({ "columns": [{ "name": "a", "named": false, "expr": column }], "from": [from], "where": null, "groupBy": [], "having": null, "orderBy": [], "limit": null, "offset": null, "distinct": false, "aggregate": false, "values": false, "window": false });
        json!({ "version": TREE_VERSION, "statement": statement, "dml": null, "returning": null, "columns": [{ "name": "a", "declared": null }], "params": [], "readonly": true, "tables": {}, "schemaErrors": [] })
    }

    fn read(tree: &Value) -> Result<Tree, String> {
        parse(&tree.to_string())
    }

    #[test]
    fn a_member_the_walker_always_writes_is_required_and_one_it_does_not_is_refused() {
        assert!(read(&tree()).is_ok());
        // A column reference's outer-join mark, which a default would read as `false`.
        let mut t = tree();
        t["statement"]["columns"][0]["expr"].as_object_mut().unwrap().remove("outer");
        assert!(read(&t).unwrap_err().contains("missing field `outer` of a `column` node"), "{:?}", read(&t));
        // Every node's own, and a FROM item's.
        for (at, member) in [("/statement/columns/0/expr", "outerOn"), ("/statement/from/0", "outer"), ("/statement", "where"), ("", "readonly"), ("", "schemaErrors")] {
            let mut t = tree();
            t.pointer_mut(at).unwrap().as_object_mut().unwrap().remove(member);
            assert!(read(&t).unwrap_err().contains(&format!("`{member}`")), "{member}: {:?}", read(&t));
        }
        // A member no struct declares.
        let mut t = tree();
        t["statement"]["columns"][0]["expr"]["canBeNull"] = json!(true);
        assert!(read(&t).unwrap_err().contains("unknown field `canBeNull`"), "{:?}", read(&t));
        // A column SQLite resolved to no table has no member after `register`.
        let mut t = tree();
        t["statement"]["columns"][0]["expr"] = json!({ "op": "column", "aff": "none", "outerOn": false, "innerOn": false, "cursor": 0, "index": 0, "table": null, "register": false });
        assert!(read(&t).is_ok());
        // A failed statement's tree has no statement.
        assert!(read(&json!({ "version": TREE_VERSION, "error": { "message": "no such table: x", "offset": 14 }, "schemaErrors": [] })).is_ok());
    }

    #[test]
    fn a_tree_of_another_version_is_refused_by_both_numbers() {
        let mut t = tree();
        t["version"] = json!(TREE_VERSION + 1);
        let refused = read(&t).unwrap_err();
        assert!(refused.contains(&format!("the tree is of version {}, and this engine reads version {TREE_VERSION}", TREE_VERSION + 1)), "{refused}");
        t.as_object_mut().unwrap().remove("version");
        assert!(read(&t).unwrap_err().contains("missing field `version`"));
    }
}
