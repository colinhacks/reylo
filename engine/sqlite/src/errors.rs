//! The errors a SQLite statement may raise, where it runs no trigger
//! ([`reylo_engine_core::errors`]): a function the application registers fails the type check, so
//! a trigger is the only code of a user's a statement may set off. Its constraint errors then name
//! only the tables and columns of the unique keys, the NOT NULL columns, the columns of a STRICT
//! table and the indexes of expressions of the tables it writes. A CHECK's name is not in the
//! catalog, so a CHECK's error is not narrowed.

use crate::host::Host;
use reylo_engine_core::api::{Analysis, Scope};
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::errors::{has_word, Errors};
use reylo_engine_core::host::Lazy;
use serde_json::Value;

/// `{written}` is a SELECT of the tables the statement writes; `{cascades}` whether a row it writes
/// may change a key another table's foreign key refers to. One row `('open', why)` for each trigger
/// that may fire, and one `('name', code, field, name)` for each name an error may carry: a unique
/// key's columns as SQLite's message lists them, `a, b`. What a write reaches: the written tables,
/// and, where `{cascades}`, the tables a foreign key's action writes in turn.
const ERRORS_QUERY: &str = "with recursive w(t) as ( \
      select name from sqlite_schema where type in ('table', 'view') and name in ({written}) \
      union select s.name from sqlite_schema s join pragma_foreign_key_list(s.name) f join w on f.\"table\" = w.t \
        where s.type = 'table' and {cascades} and (f.on_delete in ('CASCADE', 'SET NULL', 'SET DEFAULT') or f.on_update in ('CASCADE', 'SET NULL', 'SET DEFAULT'))), \
    keys(t, code, name, columns, expression) as ( \
      select w.t, case i.origin when 'pk' then 'SQLITE_CONSTRAINT_PRIMARYKEY' else 'SQLITE_CONSTRAINT_UNIQUE' end, i.name, \
        (select group_concat(name, ', ') from (select name from pragma_index_info(i.name) order by seqno)), \
        exists (select 1 from pragma_index_xinfo(i.name) x where x.key and x.cid = -2) \
      from w join pragma_index_list(w.t) i where i.\"unique\" \
      union all select w.t, 'SQLITE_CONSTRAINT_PRIMARYKEY', null, (select group_concat(name, ', ') from (select name from pragma_table_info(w.t) where pk > 0 order by pk)), 0 \
      from w where exists (select 1 from pragma_table_info(w.t) where pk > 0)) \
    select 'open' as kind, 'trigger' as code, g.name as field, null as name from sqlite_schema g join w on g.tbl_name = w.t where g.type = 'trigger' \
    union all select 'name', code, 'constraint', name from keys where expression \
    union all select 'name', code, 'columns', columns from keys where not expression and columns is not null \
    union all select 'name', code, 'table', t from keys \
    union all select 'name', 'SQLITE_CONSTRAINT_ROWID', 'columns', 'rowid' from w join pragma_table_list(w.t) l where l.type = 'table' and not l.wr \
    union all select 'name', 'SQLITE_CONSTRAINT_ROWID', 'table', w.t from w join pragma_table_list(w.t) l where l.type = 'table' and not l.wr \
    union all select 'name', 'SQLITE_CONSTRAINT_NOTNULL', 'columns', c.name from w join pragma_table_xinfo(w.t) c where c.\"notnull\" or c.pk > 0 \
    union all select 'name', 'SQLITE_CONSTRAINT_NOTNULL', 'table', w.t from w \
    union all select 'name', 'SQLITE_CONSTRAINT_DATATYPE', 'columns', c.name from w join pragma_table_list(w.t) l join pragma_table_xinfo(w.t) c where l.strict \
    union all select 'name', 'SQLITE_CONSTRAINT_DATATYPE', 'table', w.t from w join pragma_table_list(w.t) l where l.strict";

/// A string literal: a quote doubled.
fn literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The names each error the statement may raise can carry: `Some(None)` where it may fire a
/// trigger, or the parse named no table, or no table it writes.
pub fn errors(analysis: &Analysis, host: &Host) -> Lazy<Option<Errors>> {
    let Some(tables) = &analysis.tables else { return Some(None) };
    let written: Vec<String> = tables.write.iter().map(|t| literal(t.rsplit('.').next().unwrap_or(t))).collect();
    if analysis.kind != "select" && written.is_empty() {
        return Some(None);
    }
    // An INSERT changes no key but as a REPLACE, which deletes, or through an upsert that updates.
    let cascades = !written.is_empty() && (analysis.kind != "insert" || has_word(&analysis.rewritten.sql, Dialect::Sqlite, &["update", "replace"]));
    let query = ERRORS_QUERY.replace("{written}", &if written.is_empty() { "null".to_string() } else { written.join(", ") }).replace("{cascades}", if cascades { "1" } else { "0" });
    let rows = host.query(&query, Vec::new(), Scope::Statement)?;
    let Ok(rows) = rows else { return Some(None) };
    Some(narrowed(&rows))
}

/// What [`ERRORS_QUERY`]'s rows say: nothing where one opens the statement. A name an extended
/// code's error may carry is one the primary code's may too, which is the code sqlite3 and the
/// sqlite package give, but for the constraint: given the primary code alone, the run time reads a
/// failed CHECK's name or expression into it too, which the catalog does not hold.
fn narrowed(rows: &[Vec<Value>]) -> Option<Errors> {
    let text = |row: &[Value], k: usize| row.get(k).and_then(Value::as_str).map(str::to_string);
    let mut errors = Errors::default();
    for row in rows {
        match text(row, 0).as_deref() {
            Some("name") => {
                if let (Some(code), Some(field), Some(name)) = (text(row, 1), text(row, 2), text(row, 3)) {
                    errors.add(&code, &field, &name);
                    if field != "constraint" {
                        errors.add("SQLITE_CONSTRAINT", &field, &name);
                    }
                }
            }
            _ => return None,
        }
    }
    Some(errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_name_is_the_primary_code_s_too_and_a_trigger_opens_the_statement() {
        let rows = vec![vec![json!("name"), json!("SQLITE_CONSTRAINT_UNIQUE"), json!("columns"), json!("email")], vec![json!("name"), json!("SQLITE_CONSTRAINT_UNIQUE"), json!("table"), json!("users")]];
        assert_eq!(
            serde_json::to_value(narrowed(&rows)).unwrap(),
            json!({ "SQLITE_CONSTRAINT": { "columns": ["email"], "table": ["users"] }, "SQLITE_CONSTRAINT_UNIQUE": { "columns": ["email"], "table": ["users"] } })
        );
        let mut open = rows.clone();
        open.push(vec![json!("open"), json!("trigger"), json!("audit"), Value::Null]);
        assert_eq!(narrowed(&open), None);
        // An index's name is the extended code's alone: the primary code's may be a CHECK's.
        let index = vec![vec![json!("name"), json!("SQLITE_CONSTRAINT_UNIQUE"), json!("constraint"), json!("users_lower_email")]];
        assert_eq!(serde_json::to_value(narrowed(&index)).unwrap(), json!({ "SQLITE_CONSTRAINT_UNIQUE": { "constraint": ["users_lower_email"] } }));
    }
}
