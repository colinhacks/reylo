//! What running a SQLite statement may do ([`Capabilities`]), an upper bound: the tables it reads
//! and writes as SQLite resolved them, through views too, and, from the database's schema, the
//! tables a foreign key's action writes, the `sqlite_sequence` an AUTOINCREMENT table's insert
//! writes, and a trigger's body, which the engine does not read and so may touch any table.

use crate::sqlite_tree::{Node, Sel, Tree};
use reylo_engine_core::api::{Analysis, Scope};
use crate::host::Host;
use reylo_engine_core::host::Lazy;
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::errors::has_word;
use reylo_engine_core::types::{Access, Capabilities, CapabilityTables, StatementKind};
use serde_json::Value;

/// One row `('trigger', table)` for each trigger, `('autoincrement', table)` for each table with an
/// AUTOINCREMENT key, `('replaces', table)` for each table with a constraint declared ON CONFLICT
/// REPLACE, `('virtual', table, sql)` for each virtual table, and `('key', child, parent, on update,
/// on delete)` for each foreign key.
pub const SCHEMA_QUERY: &str = "select 'trigger', tbl_name, null, null, null from sqlite_schema where type = 'trigger' \
    union all select 'autoincrement', name, null, null, null from sqlite_schema where type = 'table' and sql like '%autoincrement%' \
    union all select 'replaces', name, null, null, null from sqlite_schema where type = 'table' and sql like '%on conflict replace%' \
    union all select 'virtual', name, lower(sql), null, null from sqlite_schema where type = 'table' and sql like 'create virtual table%' \
    union all select 'key', m.name, f.\"table\", f.on_update, f.on_delete from sqlite_schema m join pragma_foreign_key_list(m.name) f where m.type = 'table'";

/// SQLite's own functions, as its core, its date and time, math, JSON, window and aggregate
/// functions, and the extensions the engine's build holds, name them: a call to any other is to a
/// function the application registers, whose code the engine does not read.
const BUILT_IN: &[&str] = &[
    "abs", "changes", "char", "coalesce", "concat", "concat_ws", "format", "glob", "hex", "if", "ifnull", "iif", "instr", "last_insert_rowid", "length", "like", "likelihood", "likely", "lower", "ltrim", "max", "min", "nullif", "octet_length", "printf", "quote", "random", "randomblob", "replace", "round", "rtrim", "sign", "soundex", "sqlite_compileoption_get", "sqlite_compileoption_used", "sqlite_offset", "sqlite_source_id", "sqlite_version", "substr", "substring", "total_changes", "trim", "typeof", "unhex", "unicode", "unistr", "unistr_quote", "unlikely", "upper", "zeroblob",
    "date", "time", "datetime", "julianday", "unixepoch", "strftime", "timediff", "current_date", "current_time", "current_timestamp",
    "avg", "count", "group_concat", "string_agg", "sum", "total", "median", "percentile", "percentile_cont", "percentile_disc",
    "row_number", "rank", "dense_rank", "percent_rank", "cume_dist", "ntile", "lag", "lead", "first_value", "last_value", "nth_value",
    "acos", "acosh", "asin", "asinh", "atan", "atan2", "atanh", "ceil", "ceiling", "cos", "cosh", "degrees", "exp", "floor", "ln", "log", "log10", "log2", "mod", "pi", "pow", "power", "radians", "sin", "sinh", "sqrt", "tan", "tanh", "trunc",
    "json", "jsonb", "json_array", "jsonb_array", "json_array_length", "json_error_position", "json_extract", "jsonb_extract", "json_insert", "jsonb_insert", "json_object", "jsonb_object", "json_patch", "jsonb_patch", "json_pretty", "json_remove", "jsonb_remove", "json_replace", "jsonb_replace", "json_set", "jsonb_set", "json_type", "json_valid", "json_quote", "json_group_array", "jsonb_group_array", "json_group_object", "jsonb_group_object", "json_each", "json_tree", "jsonb_each", "jsonb_tree",
    "highlight", "snippet", "bm25", "matchinfo", "offsets", "fts3_tokenizer", "rtreecheck", "rtreenode", "rtreedepth", "generate_series",
    "geopoly_area", "geopoly_bbox", "geopoly_blob", "geopoly_ccw", "geopoly_contains_point", "geopoly_group_bbox", "geopoly_json", "geopoly_overlap", "geopoly_regular", "geopoly_svg", "geopoly_within", "geopoly_xform",
];

/// What a statement may do as far as the statement alone says: the tables it reads, through views
/// too, and writes, and the functions it calls. [`refine`] adds what the schema says.
pub fn of_statement(t: &Tree) -> Capabilities {
    let mut capabilities = Capabilities::default();
    let mut walk = Walk::default();
    if let Some(s) = &t.statement {
        walk.sel(s);
    }
    for c in t.returning.iter().flatten() {
        walk.node(&c.expr);
    }
    if let Some(dml) = &t.dml {
        let upsert = dml.upserts.iter().flat_map(|u| u.set.iter().map(|s| &s.expr).chain(&u.where_));
        for n in dml.values.iter().flatten().chain(dml.set.iter().map(|s| &s.expr)).chain(&dml.where_).chain(upsert) {
            walk.node(n);
        }
    }
    let Walk { read, ctes, functions } = walk;
    for table in read.into_iter().filter(|table| !ctes.iter().any(|cte| cte.eq_ignore_ascii_case(table))) {
        capabilities.touch(table, Access::Read);
    }
    if let Some(dml) = &t.dml {
        capabilities.touch(dml.table.clone(), Access::Write);
    }
    for f in &functions {
        let name = f.to_ascii_lowercase();
        if name == "load_extension" {
            // Loads code the engine cannot read, from a file.
            unread(&mut capabilities);
        } else if !BUILT_IN.contains(&name.as_str()) && !name.starts_with("pragma_") {
            unread(&mut capabilities);
        }
    }
    capabilities
}

/// The PRAGMAs that, read without a value, answer with a setting or the schema alone.
const SCHEMA_PRAGMAS: &[&str] = &[
    "table_info", "table_xinfo", "table_list", "index_list", "index_info", "index_xinfo", "foreign_key_list", "database_list", "collation_list", "function_list", "module_list", "pragma_list", "compile_options",
    "user_version", "application_id", "schema_version", "data_version", "page_count", "page_size", "max_page_count", "freelist_count", "encoding", "journal_mode", "journal_size_limit", "foreign_keys", "busy_timeout",
    "cache_size", "cache_spill", "synchronous", "temp_store", "auto_vacuum", "automatic_index", "case_sensitive_like", "cell_size_check", "checkpoint_fullfsync", "defer_foreign_keys", "fullfsync", "hard_heap_limit",
    "ignore_check_constraints", "legacy_alter_table", "locking_mode", "mmap_size", "query_only", "read_uncommitted", "recursive_triggers", "reverse_unordered_selects", "secure_delete", "soft_heap_limit", "threads", "trusted_schema", "writable_schema",
];

/// What a command (DDL, a PRAGMA, a transaction's control) does, by its first keyword.
pub fn of_command(sql: &str, kind: &str) -> Capabilities {
    let mut capabilities = Capabilities::default();
    match kind {
        "begin" | "commit" | "end" | "rollback" | "savepoint" | "release" => capabilities.transaction = true,
        // A PRAGMA that sets something: on the connection (`foreign_keys`), or in the file (`user_version`, `journal_mode`).
        "pragma" if sql.contains('=') || sql.contains('(') => {
            capabilities.session = true;
            capabilities.ddl = true;
        }
        // A PRAGMA that reads a setting or the schema, and a plan: no rows of a table.
        "pragma" if SCHEMA_PRAGMAS.iter().any(|p| has_word(sql, Dialect::Sqlite, &[*p])) => {}
        "explain" => {}
        // Any other PRAGMA may read every table's rows (`integrity_check`, `foreign_key_check`) or
        // change the file (`optimize`, which may run ANALYZE, `wal_checkpoint`, `incremental_vacuum`).
        "pragma" => {
            capabilities.ddl = true;
            capabilities.unknown_tables();
        }
        // A database file, attached to the connection or let go.
        "attach" | "detach" => {
            capabilities.session = true;
            capabilities.external = true;
        }
        // `VACUUM INTO` writes another file.
        "vacuum" => {
            capabilities.ddl = true;
            capabilities.unknown_tables();
            if has_word(sql, Dialect::Sqlite, &["into"]) {
                capabilities.external = true;
            }
        }
        "create" | "drop" | "alter" | "reindex" | "analyze" => {
            capabilities.ddl = true;
            capabilities.unknown_tables();
        }
        _ => {
            unread(&mut capabilities);
            capabilities.transaction = true;
        }
    }
    capabilities
}

/// The modules SQLite builds in, whose virtual tables run no code of the application's.
const BUILT_IN_MODULES: &[&str] = &["fts3", "fts4", "fts5", "rtree", "rtree_i32", "geopoly"];

/// The statement's capabilities, with what the database's schema adds: a trigger on a table it
/// writes runs code the engine does not read, and so does a virtual table of a module the
/// application registers; a write to a built-in virtual table writes the tables that hold its index,
/// which the statement does not name; a foreign key's action writes the table that refers to it,
/// and its check reads the other side; an AUTOINCREMENT table's insert counts in `sqlite_sequence`.
pub fn refine(analysis: &Analysis, host: &Host) -> Lazy<Option<Capabilities>> {
    let Some(mut capabilities) = analysis.capabilities.clone() else { return Some(None) };
    let CapabilityTables::Named(tables) = &capabilities.tables else { return Some(Some(capabilities)) };
    let written: Vec<String> = tables.iter().filter(|(_, access)| **access == Access::Write).map(|(name, _)| name.clone()).collect();
    let named: Vec<String> = tables.keys().cloned().collect();
    if named.is_empty() {
        return Some(Some(capabilities));
    }
    let Ok(rows) = host.query(SCHEMA_QUERY, Vec::new(), Scope::Session)? else {
        unread(&mut capabilities);
        return Some(Some(capabilities));
    };
    let text = |row: &Vec<Value>, k: usize| row.get(k).and_then(Value::as_str).map(str::to_ascii_lowercase);
    let tagged = |tag: &'static str| rows.iter().filter(move |row| row.first().and_then(Value::as_str) == Some(tag));
    let is = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    for row in tagged("virtual") {
        let (Some(table), Some(sql)) = (text(row, 1), text(row, 2)) else { continue };
        if !named.iter().any(|n| is(n, &table)) {
            continue;
        }
        let module = sql.split_once(" using ").map(|(_, rest)| rest.trim_start().split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("").to_string()).unwrap_or_default();
        if !BUILT_IN_MODULES.contains(&module.as_str()) {
            unread(&mut capabilities);
            return Some(Some(capabilities));
        }
        if written.iter().any(|w| is(w, &table)) {
            capabilities.unknown_tables();
            return Some(Some(capabilities));
        }
    }
    if written.is_empty() {
        return Some(Some(capabilities));
    }
    // An INSERT changes no key a foreign key refers to but as a REPLACE, which deletes the row it
    // conflicts with (`INSERT OR REPLACE`, `REPLACE`, or a constraint declared ON CONFLICT REPLACE),
    // or an upsert's UPDATE; a delete and an update may, and a key's action may set off another's.
    let insert = analysis.kind == Some(StatementKind::Insert);
    let replaces = written.iter().any(|w| tagged("replaces").any(|row| text(row, 1).is_some_and(|t| is(&t, w))));
    let cascades = !insert || replaces || has_word(&analysis.rewritten.sql, Dialect::Sqlite, &["replace", "update"]);
    let acts = |action: Option<String>| action.is_some_and(|a| matches!(a.as_str(), "cascade" | "set null" | "set default"));
    // Only an insert takes a new AUTOINCREMENT key, and only into the table it names.
    if insert && written.iter().any(|w| tagged("autoincrement").any(|row| text(row, 1).is_some_and(|t| is(&t, w)))) {
        capabilities.touch("sqlite_sequence", Access::Write);
    }
    let mut reached: Vec<String> = Vec::new();
    let mut queue = written;
    while let Some(table) = queue.pop() {
        if reached.iter().any(|t| is(t, &table)) {
            continue;
        }
        reached.push(table.clone());
        capabilities.touch(table.clone(), Access::Write);
        if tagged("trigger").any(|row| text(row, 1).is_some_and(|t| is(&t, &table))) {
            unread(&mut capabilities);
            return Some(Some(capabilities));
        }
        for row in tagged("key") {
            let (Some(child), Some(parent)) = (text(row, 1), text(row, 2)) else { continue };
            // The check of a key the written table holds reads the table it refers to.
            if is(&child, &table) {
                capabilities.touch(parent.clone(), Access::Read);
            }
            // A change to a key the table holds sets off the key's action in the tables that refer
            // to it, or its check, which reads them.
            if cascades && is(&parent, &table) {
                if acts(text(row, 3)) || acts(text(row, 4)) {
                    queue.push(child.clone());
                } else {
                    capabilities.touch(child.clone(), Access::Read);
                }
            }
        }
    }
    Some(Some(capabilities))
}

/// Code the engine does not read, which may touch any table, change what the database defines or
/// the connection, and reach past it.
fn unread(capabilities: &mut Capabilities) {
    capabilities.unknown_tables();
    capabilities.ddl = true;
    capabilities.session = true;
    capabilities.external = true;
}

/// The tables a statement reads, a view's included with what it reads, the names of its WITH
/// queries, and the functions it calls.
#[derive(Default)]
struct Walk {
    read: Vec<String>,
    ctes: Vec<String>,
    functions: Vec<String>,
}

impl Walk {
    fn node(&mut self, n: &Node) {
        if n.op == "function" {
            if let Some(name) = &n.name {
                self.functions.push(name.clone());
            }
        }
        if let Some(q) = &n.select {
            self.sel(q);
        }
        for child in n.left.iter().chain(&n.right) {
            self.node(child);
        }
        for child in n.list.iter().chain(&n.args).chain(&n.order_by).chain(&n.partition_by) {
            self.node(child);
        }
    }

    fn sel(&mut self, s: &Sel) {
        for c in &s.columns {
            self.node(&c.expr);
        }
        for f in &s.from {
            if f.cte {
                self.ctes.extend(f.table.clone());
            }
            // A view keeps the name the statement writes, and reads what its subquery does.
            if !f.cte && !f.tab_func {
                self.read.extend(f.table.clone());
            }
            if f.tab_func {
                self.functions.extend(f.table.clone());
            }
            if let Some(q) = &f.subquery {
                self.sel(q);
            }
            for n in f.args.iter().chain(&f.on) {
                self.node(n);
            }
        }
        for n in s.rows.iter().flatten().chain(&s.where_).chain(&s.group_by).chain(&s.having).chain(&s.order_by).chain(&s.limit).chain(&s.offset) {
            self.node(n);
        }
        if let Some(prior) = &s.prior {
            self.sel(prior);
        }
    }
}
