//! What running a Postgres statement may do ([`Capabilities`]), an upper bound: the tables it
//! reaches, read through the catalog as its errors are ([`crate::errors`]), and what its kind and
//! the server's functions it calls do beyond their rows. Code of a user's or an extension's that
//! the statement sets off and the engine does not read (a trigger, a rule, a function that is not
//! immutable) may touch any table, so the tables are then unknown.

use crate::analyze;
use crate::cardinality::{names, relation_named};
use crate::errors;
use crate::host::Host;
use pg_query::protobuf::node::Node as NodeEnum;
use reylo_engine_core::api::{Analysis, Scope};
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::errors::has_word;
use reylo_engine_core::host::Lazy;
use reylo_engine_core::types::{Access, Capabilities};
use serde_json::{json, Value};
use std::collections::HashSet;

/// The oid of the system catalog `$name`, as `pg_depend` names a class.
macro_rules! catalog {
    ($name:literal) => {
        concat!("'pg_catalog.", $name, "'::pg_catalog.regclass::pg_catalog.oid")
    };
}

/// Code that touches no table: an operator class or family, which an index scan calls, and a
/// function `$p` that is immutable and written in C or built in, as an extension's are; an
/// aggregate is so where each of its support functions is. A function in SQL or a procedural
/// language is read as unknown whatever it is labelled, since a body labelled immutable may still
/// read a table.
macro_rules! harmless_function {
    ($p:literal) => {
        concat!(
            "(", $p, ".provolatile OPERATOR(pg_catalog.=) 'i'::pg_catalog.\"char\" AND ", $p, ".prolang OPERATOR(pg_catalog.=) ANY (SELECT l.oid FROM pg_catalog.pg_language l WHERE l.lanname OPERATOR(pg_catalog.=) ANY ('{c,internal}'::pg_catalog.name[])) \
            AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_aggregate g JOIN pg_catalog.pg_proc f ON f.oid OPERATOR(pg_catalog.=) ANY (ARRAY[g.aggtransfn::pg_catalog.oid, g.aggfinalfn::pg_catalog.oid, g.aggcombinefn::pg_catalog.oid, g.aggmtransfn::pg_catalog.oid, g.aggminvtransfn::pg_catalog.oid, g.aggmfinalfn::pg_catalog.oid]) \
              WHERE g.aggfnoid::pg_catalog.oid OPERATOR(pg_catalog.=) ", $p, ".oid AND (f.provolatile OPERATOR(pg_catalog.<>) 'i'::pg_catalog.\"char\" \
                OR f.prolang OPERATOR(pg_catalog.<>) ALL (SELECT l.oid FROM pg_catalog.pg_language l WHERE l.lanname OPERATOR(pg_catalog.=) ANY ('{c,internal}'::pg_catalog.name[])))))"
        )
    };
}

/// `$1` and `$2` are `text[]` of the relations the statement writes and reads, as it writes them;
/// `$3` whether a row it writes may change a key another table's foreign key refers to; `$4` and
/// `$5` how many names `$1` and `$2` hold; `$6` whether the statement may insert a row or set a
/// column to its default; `$7` a `text[]` of the sequences its calls to `nextval` and `setval` name
/// in constants, read as `regclass` reads them, and `$8` how many. One row `(name, access,
/// foreign)` for each relation the statement reaches, qualified by its schema, with the most it may
/// do to its rows (2 writes, 1 locks, 0 reads, and 3 where a function of a user's that is not
/// [`harmless_function`] takes one of its rows, which a statement may call as `t.f`, written as a
/// column) and whether it is a foreign table.
///
/// A write reaches the written relations, their partitions and inheritance children, a view's base
/// relations, the tables a foreign key's action writes in turn where `$3`, and, where `$6`, the
/// sequences a written relation's or one of its ancestors' defaults call or its identity columns own;
/// it locks the rows a foreign key checks, in the table the key refers to and, where `$3`, in the
/// tables that refer to it. A read reaches the read relations, their partitions and inheritance
/// children, and through views and policies the relations they read, as
/// [`crate::errors::ERRORS_QUERY`] reaches them.
pub const TABLES_QUERY: &str = concat!(
    "WITH RECURSIVE \
    named(oid, written) AS ( \
      SELECT c.oid, true FROM ",
    names!("$1::pg_catalog.text[]", "$4::pg_catalog.int4", "x"),
    " JOIN pg_catalog.pg_class c ON c.oid OPERATOR(pg_catalog.=) ",
    relation_named!("x.name"),
    " UNION ALL SELECT c.oid, false FROM ",
    names!("$2::pg_catalog.text[]", "$5::pg_catalog.int4", "x"),
    " JOIN pg_catalog.pg_class c ON c.oid OPERATOR(pg_catalog.=) ",
    relation_named!("x.name"),
    "), \
    edge(parent, child, read, written) AS ( \
      SELECT h.inhparent, h.inhrelid, true, true FROM pg_catalog.pg_inherits h \
      UNION ALL SELECT q.ev_class, d.refobjid, true, true FROM pg_catalog.pg_rewrite q JOIN pg_catalog.pg_depend d \
        ON d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_rewrite"), " AND d.objid OPERATOR(pg_catalog.=) q.oid \
        AND d.refclassid OPERATOR(pg_catalog.=) ", catalog!("pg_class"), " AND d.refobjid OPERATOR(pg_catalog.<>) q.ev_class \
        WHERE q.rulename OPERATOR(pg_catalog.=) '_RETURN'::pg_catalog.name \
      UNION ALL SELECT k.confrelid, k.conrelid, false, true FROM pg_catalog.pg_constraint k WHERE $3::pg_catalog.bool AND k.contype OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\" \
        AND (k.confdeltype OPERATOR(pg_catalog.=) ANY ('{c,n,d}'::pg_catalog.\"char\"[]) OR k.confupdtype OPERATOR(pg_catalog.=) ANY ('{c,n,d}'::pg_catalog.\"char\"[])) \
      UNION ALL SELECT y.polrelid, d.refobjid, true, false FROM pg_catalog.pg_policy y JOIN pg_catalog.pg_depend d \
        ON d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_policy"), " AND d.objid OPERATOR(pg_catalog.=) y.oid \
        AND d.refclassid OPERATOR(pg_catalog.=) ", catalog!("pg_class"), " AND d.refobjid OPERATOR(pg_catalog.<>) y.polrelid), \
    w(oid) AS (SELECT n.oid FROM named n WHERE n.written UNION SELECT e.child FROM edge e JOIN w ON e.parent OPERATOR(pg_catalog.=) w.oid WHERE e.written), \
    r(oid) AS (SELECT n.oid FROM named n UNION SELECT e.child FROM edge e JOIN r ON e.parent OPERATOR(pg_catalog.=) r.oid WHERE e.read), \
    up(oid) AS (SELECT w.oid FROM w UNION SELECT h.inhparent FROM pg_catalog.pg_inherits h JOIN up ON h.inhrelid OPERATOR(pg_catalog.=) up.oid), \
    reached(oid, access) AS ( \
      SELECT w.oid, 2 FROM w \
      UNION ALL SELECT d.refobjid, 2 FROM pg_catalog.pg_attrdef ad JOIN up ON ad.adrelid OPERATOR(pg_catalog.=) up.oid \
        JOIN pg_catalog.pg_depend d ON d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_attrdef"), " AND d.objid OPERATOR(pg_catalog.=) ad.oid \
          AND d.refclassid OPERATOR(pg_catalog.=) ", catalog!("pg_class"), " \
        WHERE $6::pg_catalog.bool AND EXISTS (SELECT 1 FROM pg_catalog.pg_class s WHERE s.oid OPERATOR(pg_catalog.=) d.refobjid AND s.relkind OPERATOR(pg_catalog.=) 'S'::pg_catalog.\"char\") \
      UNION ALL SELECT d.objid, 2 FROM pg_catalog.pg_depend d JOIN up ON d.refobjid OPERATOR(pg_catalog.=) up.oid \
        WHERE $6::pg_catalog.bool AND d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_class"), " AND d.refclassid OPERATOR(pg_catalog.=) ", catalog!("pg_class"), " \
          AND d.deptype OPERATOR(pg_catalog.=) ANY ('{a,i}'::pg_catalog.\"char\"[]) \
      UNION ALL SELECT pg_catalog.to_regclass(x.name)::pg_catalog.oid, 2 FROM ",
    names!("$7::pg_catalog.text[]", "$8::pg_catalog.int4", "x"),
    " WHERE pg_catalog.to_regclass(x.name) IS NOT NULL \
      UNION ALL SELECT k.confrelid, 1 FROM pg_catalog.pg_constraint k JOIN w ON k.conrelid OPERATOR(pg_catalog.=) w.oid WHERE k.contype OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\" \
      UNION ALL SELECT k.conrelid, 1 FROM pg_catalog.pg_constraint k JOIN w ON k.confrelid OPERATOR(pg_catalog.=) w.oid \
        WHERE $3::pg_catalog.bool AND k.contype OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\" \
      UNION ALL SELECT r.oid, 0 FROM r \
      UNION ALL SELECT c.oid, 3 FROM pg_catalog.pg_class c JOIN (SELECT w.oid FROM w UNION SELECT r.oid FROM r) x ON c.oid OPERATOR(pg_catalog.=) x.oid \
        WHERE EXISTS (SELECT 1 FROM pg_catalog.pg_proc p WHERE p.oid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid AND p.pronargs OPERATOR(pg_catalog.>=) 1 \
          AND p.proargtypes[0] OPERATOR(pg_catalog.=) c.reltype AND NOT ", harmless_function!("p"), ")) \
    SELECT pg_catalog.textcat(pg_catalog.textcat(n.nspname::pg_catalog.text, '.'::pg_catalog.text), c.relname::pg_catalog.text), \
      pg_catalog.max(a.access), c.relkind OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\" \
    FROM reached a JOIN pg_catalog.pg_class c ON c.oid OPERATOR(pg_catalog.=) a.oid JOIN pg_catalog.pg_namespace n ON n.oid OPERATOR(pg_catalog.=) c.relnamespace \
    GROUP BY 1, 3"
);

/// `$1` to `$4` are `text[]` of what [`crate::errors::ERRORS_QUERY`] found the statement sets off
/// that a user or an extension wrote: the oids it depends on, the functions it calls by name, the
/// casts it may make by oid, and the operators it may apply by name; `$5` to `$8` how many each
/// holds. One row `(what, name)` for each of them that touches no table: an operator class or
/// family, which an index scan calls, and a function, an operator or a cast whose code is
/// [`harmless_function`].
pub const IMMUTABLE_QUERY: &str = concat!(
    "SELECT 'depends on'::pg_catalog.text, x.name FROM ",
    names!("$1::pg_catalog.text[]", "$5::pg_catalog.int4", "x"),
    " WHERE EXISTS (SELECT 1 FROM pg_catalog.pg_opclass o WHERE o.oid OPERATOR(pg_catalog.=) x.name::pg_catalog.oid) \
      OR EXISTS (SELECT 1 FROM pg_catalog.pg_opfamily f WHERE f.oid OPERATOR(pg_catalog.=) x.name::pg_catalog.oid) \
      OR EXISTS (SELECT 1 FROM pg_catalog.pg_proc p WHERE p.oid OPERATOR(pg_catalog.=) x.name::pg_catalog.oid AND ", harmless_function!("p"), ") \
      OR EXISTS (SELECT 1 FROM pg_catalog.pg_operator o JOIN pg_catalog.pg_proc p ON p.oid OPERATOR(pg_catalog.=) o.oprcode::pg_catalog.oid \
        WHERE o.oid OPERATOR(pg_catalog.=) x.name::pg_catalog.oid AND ", harmless_function!("p"), ") \
    UNION ALL SELECT 'function', x.name FROM ",
    names!("$2::pg_catalog.text[]", "$6::pg_catalog.int4", "x"),
    " WHERE NOT EXISTS (SELECT 1 FROM pg_catalog.pg_proc p WHERE p.proname OPERATOR(pg_catalog.=) x.name \
      AND p.oid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid AND NOT ", harmless_function!("p"), ") \
    UNION ALL SELECT 'cast', x.name FROM ",
    names!("$3::pg_catalog.text[]", "$7::pg_catalog.int4", "x"),
    " WHERE EXISTS (SELECT 1 FROM pg_catalog.pg_cast c JOIN pg_catalog.pg_proc p ON p.oid OPERATOR(pg_catalog.=) c.castfunc \
      WHERE c.oid OPERATOR(pg_catalog.=) x.name::pg_catalog.oid AND ", harmless_function!("p"), ") \
    UNION ALL SELECT 'operator', x.name FROM ",
    names!("$4::pg_catalog.text[]", "$8::pg_catalog.int4", "x"),
    " WHERE NOT EXISTS (SELECT 1 FROM pg_catalog.pg_operator o JOIN pg_catalog.pg_proc p ON p.oid OPERATOR(pg_catalog.=) o.oprcode::pg_catalog.oid \
      WHERE o.oprname OPERATOR(pg_catalog.=) x.name AND o.oid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid AND NOT ", harmless_function!("p"), ")"
);

/// The server's functions that leave state on the connection.
const SESSION: &[&str] = &[
    "set_config",
    "setseed",
    "pg_advisory_lock",
    "pg_advisory_lock_shared",
    "pg_try_advisory_lock",
    "pg_try_advisory_lock_shared",
    "pg_advisory_unlock",
    "pg_advisory_unlock_shared",
    "pg_advisory_unlock_all",
    // A transaction's advisory lock lasts until it ends, which is after the statement, as `SET LOCAL` does.
    "pg_advisory_xact_lock",
    "pg_advisory_xact_lock_shared",
    "pg_try_advisory_xact_lock",
    "pg_try_advisory_xact_lock_shared",
    "pg_stat_clear_snapshot",
    "pg_stat_force_next_flush",
    "pg_replication_origin_session_setup",
    "pg_replication_origin_session_reset",
    "pg_replication_origin_xact_setup",
    "pg_replication_origin_xact_reset",
];

/// The server's functions that reach past the connection and the data: other sessions, the
/// server's state and settings, replication, and its files.
const EXTERNAL: &[&str] = &[
    "pg_notify",
    "pg_logical_emit_message",
    "pg_cancel_backend",
    "pg_terminate_backend",
    "pg_reload_conf",
    "pg_rotate_logfile",
    "pg_switch_wal",
    "pg_create_restore_point",
    "pg_log_backend_memory_contexts",
    "pg_log_standby_snapshot",
    "pg_promote",
    "pg_wal_replay_pause",
    "pg_wal_replay_resume",
    "pg_backup_start",
    "pg_backup_stop",
    "pg_stat_reset",
    "pg_stat_reset_backend_stats",
    "pg_stat_reset_replication_slot",
    "pg_stat_reset_shared",
    "pg_stat_reset_single_function_counters",
    "pg_stat_reset_single_table_counters",
    "pg_stat_reset_slru",
    "pg_stat_reset_subscription_stats",
    "pg_create_logical_replication_slot",
    "pg_create_physical_replication_slot",
    "pg_copy_logical_replication_slot",
    "pg_copy_physical_replication_slot",
    "pg_drop_replication_slot",
    "pg_replication_slot_advance",
    "pg_sync_replication_slots",
    "pg_logical_slot_get_changes",
    "pg_logical_slot_get_binary_changes",
    "pg_replication_origin_create",
    "pg_replication_origin_drop",
    "pg_replication_origin_advance",
    "pg_read_file",
    "pg_read_binary_file",
    "pg_stat_file",
    "pg_ls_dir",
    "pg_ls_logdir",
    "pg_ls_waldir",
    "pg_ls_tmpdir",
    "pg_ls_archive_statusdir",
    "pg_ls_logicalmapdir",
    "pg_ls_logicalsnapdir",
    "pg_ls_replslotdir",
    "pg_ls_summariesdir",
    "pg_current_logfile",
    "pg_hba_file_rules",
    "pg_ident_file_mappings",
    "pg_show_all_file_settings",
    "pg_available_wal_summaries",
    "pg_wal_summary_contents",
    "lo_import",
    "lo_export",
];

/// The server's functions that change what the database defines or keeps beside its rows.
const DDL: &[&str] = &[
    "pg_nextoid",
    "pg_import_system_collations",
    "pg_extension_config_dump",
    "brin_summarize_range",
    "brin_summarize_new_values",
    "brin_desummarize_range",
    "gin_clean_pending_list",
    "pg_clear_attribute_stats",
    "pg_clear_relation_stats",
    "pg_restore_attribute_stats",
    "pg_restore_relation_stats",
    "pg_stop_making_pinned_objects",
];

/// The server's functions that write large objects, and those that read them.
const LARGE_OBJECT_WRITES: &[&str] = &["lo_creat", "lo_create", "lo_from_bytea", "lo_put", "lo_truncate", "lo_truncate64", "lo_unlink", "lowrite", "lo_import"];
const LARGE_OBJECT_READS: &[&str] = &["lo_get", "loread", "lo_open", "lo_export"];

/// The server's functions that write the sequence their first argument names, and one that reads it.
const SEQUENCE_WRITES: &[&str] = &["nextval", "setval"];
const SEQUENCE_READS: &[&str] = &["pg_sequence_last_value", "pg_get_sequence_data"];

/// What the statement may do, or `None` where the engine says nothing: a statement it could not
/// analyze, or one of more than one statement.
pub fn capabilities(analysis: &Analysis, host: &Host) -> Lazy<Option<Capabilities>> {
    if !analysis.diagnostics.is_empty() {
        return Some(None);
    }
    let sql = &analysis.rewritten.sql;
    let Ok(tree) = crate::stack::parse(sql) else { return Some(None) };
    let [stmt] = tree.protobuf.stmts.as_slice() else { return Some(None) };
    let Some(node) = stmt.stmt.as_ref().and_then(|n| n.node.as_ref()) else { return Some(None) };
    let mut capabilities = Capabilities::default();
    match node {
        // `SELECT … INTO` creates the table it fills, which sets off any event trigger.
        NodeEnum::SelectStmt(s) if s.into_clause.is_some() => unread(&mut capabilities),
        NodeEnum::SelectStmt(_) | NodeEnum::InsertStmt(_) | NodeEnum::UpdateStmt(_) | NodeEnum::DeleteStmt(_) | NodeEnum::MergeStmt(_) => {
            reached(sql, node, host, &mut capabilities)?;
        }
        NodeEnum::LockStmt(l) => locked(l, host, &mut capabilities)?,
        other => utility(other, &mut capabilities),
    }
    Some(Some(capabilities))
}

/// What a statement that reads or writes rows reaches: its relations as the catalog expands them,
/// the code a user or an extension wrote that it sets off, and the server's functions it calls.
fn reached(sql: &str, node: &NodeEnum, host: &Host, capabilities: &mut Capabilities) -> Lazy<()> {
    let Ok(parsed) = analyze::analyze(sql) else {
        unread(capabilities);
        return Some(());
    };
    // A function that runs SQL it is given as text, which may do anything.
    let Some(request) = errors::request(sql) else {
        unread(capabilities);
        return Some(());
    };
    // Each sequence a call to `nextval` or `setval` names as a constant is written: one named any other way is unknown.
    let (sequences, named_sequences) = sequences_named(node);
    let written: Vec<&str> = parsed.write_set.iter().map(String::as_str).collect();
    // The parse leaves out every table of a WITH query's name, even one read inside that query
    // (`with users as (select * from users) …`), so each WITH query's name is read as a table's too.
    let ctes = with_names(node);
    let mut read: Vec<&str> = parsed.read_set.iter().map(String::as_str).collect();
    read.extend(ctes.iter().map(String::as_str));
    let keys = !parsed.write_set.is_empty() && parsed.changes_keys;
    // An insert, a MERGE's, or a column set to its DEFAULT, in the statement or one of its WITH queries.
    let inserts = !parsed.write_set.is_empty() && has_word(sql, Dialect::Postgres, &["insert", "merge", "default"]);
    let tables = host.query(TABLES_QUERY, vec![json!(written), json!(read), json!(keys), json!(written.len()), json!(read.len()), json!(inserts), json!(sequences), json!(sequences.len())], Scope::Statement);
    let opened = errors::catalog_rows(request, host);
    let (tables, opened) = (tables?, opened?);
    let (Ok(tables), Some(opened)) = (tables, opened) else {
        unread(capabilities);
        return Some(());
    };
    for row in &tables {
        let (Some(name), Some(access)) = (row.first().and_then(Value::as_str), row.get(1).and_then(Value::as_i64)) else { continue };
        let access = match access {
            // A function of a user's that takes a row of this relation, which `t.f` calls unseen.
            3 => {
                unread(capabilities);
                continue;
            }
            2 => Access::Write,
            1 => Access::Readlock,
            // A locking clause locks the rows it reads, until the transaction ends.
            _ if parsed.has_locking => Access::Readlock,
            _ => Access::Read,
        };
        capabilities.touch(name, access);
        // A foreign table's rows are another server's.
        if row.get(2).and_then(Value::as_bool) == Some(true) {
            capabilities.external = true;
        }
    }
    let mut called: HashSet<&str> = parsed.functions.iter().map(String::as_str).collect();
    // A volatile function a view or a policy calls, read from its stored expression: a user's is
    // code the engine does not read.
    for row in opened.iter().filter(|row| row.first().and_then(Value::as_str) == Some("runs")) {
        match (row.get(1).and_then(Value::as_str), row.get(2).and_then(Value::as_str)) {
            (Some(name), Some(_)) => {
                // Inside a view, a sequence's name is not the statement's to read.
                if SEQUENCE_WRITES.contains(&name) || SEQUENCE_READS.contains(&name) {
                    unread(capabilities);
                }
                called.insert(name);
            }
            _ => unread(capabilities),
        }
    }
    for name in called {
        builtin(name, capabilities);
    }
    if !named_sequences {
        capabilities.unknown_tables();
    }
    // The code of a user's or an extension's the statement sets off, unless it cannot touch a table.
    let open: Vec<(&str, &str)> = opened.iter().filter(|row| row.first().and_then(Value::as_str) == Some("open")).filter_map(|row| Some((row.get(1)?.as_str()?, row.get(2).and_then(Value::as_str).unwrap_or("")))).collect();
    if open.iter().any(|(what, _)| *what == "foreign table") {
        capabilities.external = true;
    }
    let asked = |what: &str| -> Vec<&str> {
        let mut found: Vec<&str> = open.iter().filter(|(w, _)| *w == what).map(|(_, name)| *name).collect();
        found.sort_unstable();
        found.dedup();
        found
    };
    let (depends, functions, casts, operators) = (asked("depends on"), asked("function"), asked("cast"), asked("operator"));
    let touch_nothing = if depends.is_empty() && functions.is_empty() && casts.is_empty() && operators.is_empty() {
        Vec::new()
    } else {
        let params = vec![json!(depends), json!(functions), json!(casts), json!(operators), json!(depends.len()), json!(functions.len()), json!(casts.len()), json!(operators.len())];
        match host.query(IMMUTABLE_QUERY, params, Scope::Statement)? {
            Ok(rows) => rows,
            Err(_) => {
                unread(capabilities);
                return Some(());
            }
        }
    };
    let harmless = |what: &str, name: &str| touch_nothing.iter().any(|row| row.first().and_then(Value::as_str) == Some(what) && row.get(1).and_then(Value::as_str) == Some(name));
    // A type's input and output, which a column of a user's type calls, touch no table; neither
    // does a foreign table, whose effects `external` says.
    if open.iter().any(|(what, name)| !matches!(*what, "column type" | "type" | "foreign table") && !harmless(what, name)) {
        unread(capabilities);
    }
    Some(())
}

/// The name of each WITH query in the statement, at any depth.
fn with_names(node: &NodeEnum) -> Vec<String> {
    let Ok(tree) = serde_json::to_value(node) else { return Vec::new() };
    let mut found = Vec::new();
    fn walk(v: &Value, found: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                if let Some(name) = map.get("CommonTableExpr").and_then(|c| c.get("ctename")).and_then(Value::as_str) {
                    found.push(name.to_string());
                }
                map.values().for_each(|child| walk(child, found));
            }
            Value::Array(items) => items.iter().for_each(|i| walk(i, found)),
            _ => {}
        }
    }
    walk(&tree, &mut found);
    found.sort_unstable();
    found.dedup();
    found
}

/// The sequences calls to `nextval` and `setval` (or the read of one by name) name as a constant,
/// and whether every call names one so.
fn sequences_named(node: &NodeEnum) -> (Vec<String>, bool) {
    let Ok(tree) = serde_json::to_value(node) else { return (Vec::new(), false) };
    let mut found = Vec::new();
    let mut all = true;
    fn walk(v: &Value, found: &mut Vec<String>, all: &mut bool) {
        match v {
            Value::Object(map) => {
                if let Some(call) = map.get("FuncCall") {
                    let name = call.get("funcname").and_then(Value::as_array).and_then(|parts| parts.last()).and_then(|p| p.pointer("/node/String/sval")).and_then(Value::as_str);
                    if name.is_some_and(|n| SEQUENCE_WRITES.contains(&n) || SEQUENCE_READS.contains(&n)) {
                        let first = call.get("args").and_then(Value::as_array).and_then(|a| a.first());
                        // `nextval('posts_id_seq')`, and its `::regclass` cast.
                        let constant = first.and_then(|a| a.pointer("/node/AConst/val/Sval/sval").or_else(|| a.pointer("/node/TypeCast/arg/node/AConst/val/Sval/sval"))).and_then(Value::as_str);
                        match constant {
                            Some(sequence) => found.push(sequence.to_string()),
                            None => *all = false,
                        }
                    }
                }
                for child in map.values() {
                    walk(child, found, all);
                }
            }
            Value::Array(items) => items.iter().for_each(|i| walk(i, found, all)),
            _ => {}
        }
    }
    walk(&tree, &mut found, &mut all);
    found.sort_unstable();
    found.dedup();
    (found, all)
}

/// What a server's function of this name does beyond the rows the statement reads.
fn builtin(name: &str, capabilities: &mut Capabilities) {
    if SESSION.contains(&name) {
        capabilities.session = true;
    }
    if EXTERNAL.contains(&name) {
        capabilities.external = true;
    }
    if DDL.contains(&name) || name.starts_with("binary_upgrade_") {
        capabilities.ddl = true;
    }
    if LARGE_OBJECT_WRITES.contains(&name) {
        capabilities.touch("pg_catalog.pg_largeobject", Access::Write);
        capabilities.touch("pg_catalog.pg_largeobject_metadata", Access::Write);
    }
    if LARGE_OBJECT_READS.contains(&name) {
        capabilities.touch("pg_catalog.pg_largeobject", Access::Read);
        capabilities.touch("pg_catalog.pg_largeobject_metadata", Access::Read);
    }
}

/// Code the engine does not read, which may touch any table, change what the database defines or
/// the session, and reach past it, but not end the transaction, which a function cannot.
fn unread(capabilities: &mut Capabilities) {
    capabilities.unknown_tables();
    capabilities.ddl = true;
    capabilities.session = true;
    capabilities.external = true;
}

/// What a statement that is no query or write does, by its kind.
fn utility(node: &NodeEnum, capabilities: &mut Capabilities) {
    use pg_query::protobuf::TransactionStmtKind as T;
    use NodeEnum as N;
    match node {
        // A commit runs the deferred constraint triggers and foreign keys' checks, a user's
        // constraint triggers among them, as SET CONSTRAINTS … IMMEDIATE does.
        N::TransactionStmt(t) if [T::TransStmtCommit, T::TransStmtPrepare, T::TransStmtCommitPrepared].iter().any(|k| *k as i32 == t.kind) => {
            unread(capabilities);
            capabilities.transaction = true;
        }
        N::TransactionStmt(_) => capabilities.transaction = true,
        N::ConstraintsSetStmt(_) => unread(capabilities),
        // `SET LOCAL` lasts until the transaction ends, which is still after the statement.
        N::VariableSetStmt(_) | N::ListenStmt(_) | N::UnlistenStmt(_) | N::PrepareStmt(_) | N::DeallocateStmt(_) | N::DeclareCursorStmt(_) | N::ClosePortalStmt(_) => capabilities.session = true,
        // DISCARD drops the session's temporary tables, among the rest of its state.
        N::DiscardStmt(_) => {
            capabilities.session = true;
            capabilities.ddl = true;
            capabilities.unknown_tables();
        }
        // FETCH and MOVE run the cursor's query, and LOAD a shared library's code, neither of which the engine reads.
        N::FetchStmt(_) | N::LoadStmt(_) => unread(capabilities),
        // Reading a setting changes nothing.
        N::VariableShowStmt(_) => {}
        N::NotifyStmt(_) | N::AlterSystemStmt(_) | N::CheckPointStmt(_) => capabilities.external = true,
        // A plan alone reads nothing; `EXPLAIN ANALYZE` runs the statement, which is not read here.
        N::ExplainStmt(e) => {
            let analyzes = e.options.iter().any(|o| matches!(o.node.as_ref(), Some(N::DefElem(d)) if d.defname.eq_ignore_ascii_case("analyze")));
            if analyzes {
                unread(capabilities);
            }
        }
        // A procedure, an anonymous block and a prepared statement run code the engine does not read, and a procedure or a block may commit.
        N::CallStmt(_) | N::DoStmt(_) | N::ExecuteStmt(_) => {
            unread(capabilities);
            capabilities.transaction = true;
        }
        // COPY fires the table's triggers, TRUNCATE its TRUNCATE triggers, and REFRESH runs the view's query.
        N::CopyStmt(_) | N::TruncateStmt(_) | N::RefreshMatViewStmt(_) => unread(capabilities),
        // Every other statement defines something, or changes who may use it, may rewrite a table's
        // rows as it does, and sets off any event trigger, whose code the engine does not read.
        _ => unread(capabilities),
    }
}

/// What `LOCK TABLE` locks: each table it names, and a view's base relations and a table's
/// partitions and children, as the server locks them.
fn locked(l: &pg_query::protobuf::LockStmt, host: &Host, capabilities: &mut Capabilities) -> Lazy<()> {
    let mut names = Vec::new();
    for relation in &l.relations {
        match relation.node.as_ref() {
            Some(NodeEnum::RangeVar(rv)) => names.push(qualified(rv)),
            _ => capabilities.unknown_tables(),
        }
    }
    let none: &[&str] = &[];
    match host.query(TABLES_QUERY, vec![json!(none), json!(names), json!(false), json!(0), json!(names.len()), json!(false), json!(none), json!(0)], Scope::Statement)? {
        Ok(rows) => rows.iter().filter_map(|row| row.first().and_then(Value::as_str)).for_each(|name| capabilities.touch(name, Access::Readlock)),
        Err(_) => capabilities.unknown_tables(),
    }
    Some(())
}

/// A relation's name as a statement qualifies it.
fn qualified(rv: &pg_query::protobuf::RangeVar) -> String {
    if rv.schemaname.is_empty() {
        rv.relname.clone()
    } else {
        format!("{}.{}", rv.schemaname, rv.relname)
    }
}
