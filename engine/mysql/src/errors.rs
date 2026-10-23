//! The errors a MySQL statement may raise, where it runs only the server's own code
//! ([`reylo_engine_core::errors`]): no trigger fires on a table it writes, and it calls no stored
//! function, itself or through a view it reads. Its errors then name only the keys, foreign keys,
//! checks and columns of the tables it writes and reads, which is all the run time reads from a
//! MySQL message (src/query-error.ts).

use crate::host::Host;
use reylo_engine_core::api::{Analysis, Scope};
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::errors::{called, has_word, Errors};
use reylo_engine_core::host::Lazy;
use serde_json::Value;

/// `{written}` and `{read}` are lists of string literals, the tables the statement writes and reads;
/// `{called}` the words it calls as functions, which a stored function of any schema may answer to,
/// since a call may name the schema; `{cascades}` whether a row it writes may change a key
/// another table's foreign key refers to. One row `('open', why)` for each trigger and stored
/// function that may run, and one `('name', code, field, name)` for each name an error may carry.
/// What a write reaches: the written tables, a view's tables, and, where `{cascades}`, the tables a
/// foreign key's action writes in turn.
const ERRORS_QUERY: &str = "with recursive w(t) as ( \
      select table_name from information_schema.tables where table_schema = database() and table_name in ({written}) \
      union select u.table_name from information_schema.view_table_usage u join w on u.view_name = w.t where u.view_schema = database() and u.table_schema = database() \
      union select r.table_name from information_schema.referential_constraints r join w on r.referenced_table_name = w.t \
        where r.constraint_schema = database() and {cascades} and (r.delete_rule in ('CASCADE', 'SET NULL', 'SET DEFAULT') or r.update_rule in ('CASCADE', 'SET NULL', 'SET DEFAULT'))), \
    r(t) as (select table_name from information_schema.tables where table_schema = database() and table_name in ({read}) \
      union select u.table_name from information_schema.view_table_usage u join r on u.view_name = r.t where u.view_schema = database() and u.table_schema = database()), \
    rel(t) as (select t from w union select t from r) \
    select 'open' as kind, 'trigger' as code, cast(g.trigger_name as char) as field, cast(null as char) as name from information_schema.triggers g join w on g.event_object_table = w.t where g.event_object_schema = database() \
    union all select 'open', 'function', cast(p.routine_name as char), null from information_schema.routines p where p.routine_type = 'FUNCTION' and lower(p.routine_name) in ({called}) \
    union all select 'open', 'view', cast(v.table_name as char), null from information_schema.view_routine_usage v join rel on v.table_name = rel.t where v.table_schema = database() \
    union all select 'name', k.code, k.field, k.name from ( \
      select 'ER_DUP_ENTRY' as code, 'key' as field, cast(concat(s.table_name, '.', s.index_name) as char) as name from information_schema.statistics s join w on s.table_name = w.t where s.table_schema = database() and s.non_unique = 0 \
      union all select 'ER_DUP_ENTRY_WITH_KEY_NAME', 'key', cast(concat(s.table_name, '.', s.index_name) as char) from information_schema.statistics s join w on s.table_name = w.t where s.table_schema = database() and s.non_unique = 0 \
      union all select 'ER_NO_REFERENCED_ROW_2', 'constraint', cast(c.constraint_name as char) from information_schema.referential_constraints c join w on c.table_name = w.t where c.constraint_schema = database() \
      union all select 'ER_NO_REFERENCED_ROW_2', 'table', cast(c.table_name as char) from information_schema.referential_constraints c join w on c.table_name = w.t where c.constraint_schema = database() \
      union all select 'ER_ROW_IS_REFERENCED_2', 'constraint', cast(c.constraint_name as char) from information_schema.referential_constraints c join w on c.referenced_table_name = w.t where c.constraint_schema = database() and {cascades} \
      union all select 'ER_ROW_IS_REFERENCED_2', 'table', cast(c.table_name as char) from information_schema.referential_constraints c join w on c.referenced_table_name = w.t where c.constraint_schema = database() and {cascades} \
      union all select 'ER_CHECK_CONSTRAINT_VIOLATED', 'constraint', cast(c.constraint_name as char) from information_schema.table_constraints c join w on c.table_name = w.t where c.table_schema = database() and c.constraint_type = 'CHECK' \
      union all select n.code, 'column', cast(c.column_name as char) from information_schema.columns c join w on c.table_name = w.t \
        cross join (select 'ER_BAD_NULL_ERROR' as code union all select 'ER_NO_DEFAULT_FOR_FIELD') n where c.table_schema = database() and c.is_nullable = 'NO' \
      union all select n.code, 'column', cast(c.column_name as char) from information_schema.columns c join rel on c.table_name = rel.t \
        cross join (select 'ER_WARN_DATA_OUT_OF_RANGE' as code union all select 'WARN_DATA_TRUNCATED' union all select 'ER_TRUNCATED_WRONG_VALUE' \
          union all select 'ER_TRUNCATED_WRONG_VALUE_FOR_FIELD' union all select 'ER_DATA_TOO_LONG') n where c.table_schema = database() \
    ) k";

/// A string literal every MySQL session reads as written: a quote doubled. A name with a backslash
/// is left out, since a session reads one by its mode.
fn literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn list<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let literals: Vec<String> = names.map(literal).collect();
    if literals.is_empty() { "null".to_string() } else { literals.join(", ") }
}

/// The names each error the statement may raise can carry: `Some(None)` where it may run a trigger
/// or a stored function, is neither a query nor a write (a CALL), or writes what the parse cannot
/// name (a table of another schema).
pub fn errors(analysis: &Analysis, host: &Host) -> Lazy<Option<Errors>> {
    let Some(tables) = &analysis.tables else { return Some(None) };
    let unqualified = |t: &String| t.rsplit('.').next().unwrap_or(t).to_string();
    let written: Vec<String> = tables.write.iter().map(unqualified).collect();
    let read: Vec<String> = tables.read.iter().map(unqualified).collect();
    if (analysis.kind != "select" && written.is_empty()) || written.iter().chain(&read).any(|t| t.contains('\\')) {
        return Some(None);
    }
    let called = called(&analysis.rewritten.sql, Dialect::Mysql);
    // An INSERT changes no key but through ON DUPLICATE KEY UPDATE or as a REPLACE, which deletes.
    let cascades = !written.is_empty() && (analysis.kind != "insert" || has_word(&analysis.rewritten.sql, Dialect::Mysql, &["update", "replace"]));
    let query = ERRORS_QUERY
        .replace("{written}", &list(written.iter().map(String::as_str)))
        .replace("{read}", &list(read.iter().map(String::as_str)))
        .replace("{called}", &list(called.iter().map(String::as_str)))
        .replace("{cascades}", if cascades { "true" } else { "false" });
    let rows = host.query(&query, Vec::new(), Scope::Statement)?;
    let Ok(rows) = rows else { return Some(None) };
    Some(narrowed(&rows))
}

/// What [`ERRORS_QUERY`]'s rows say: nothing where one opens the statement.
fn narrowed(rows: &[Vec<Value>]) -> Option<Errors> {
    let text = |row: &[Value], k: usize| row.get(k).and_then(Value::as_str).map(str::to_string);
    let mut errors = Errors::default();
    for row in rows {
        match text(row, 0).as_deref() {
            Some("name") => {
                if let (Some(code), Some(field), Some(name)) = (text(row, 1), text(row, 2), text(row, 3)) {
                    errors.add(&code, &field, &name);
                }
            }
            _ => return None,
        }
    }
    Some(errors)
}
