//! What the analysis reads of DuckDB's catalog: the columns of the tables and views a statement
//! names, whether each may be NULL and its comment, and the tables' primary and unique keys. Both
//! are asked in one round, the names written into the query as lowercase string literals, since
//! DuckDB compares a table's name without case.

use crate::host::{as_bool, as_str, Host, Lazy};
use reylo_engine_core::api::Scope;
use serde_json::Value;
use std::collections::HashSet;

/// One row per column of every table and view whose name is one of `{names}`, in each one's
/// order: where it is, whether it may be NULL, its comment, whether it is in the database and
/// schema an unqualified name reads first, and whether it is in that database.
const COLUMNS_QUERY: &str = "select database_name, schema_name, table_name, column_name, is_nullable, comment, \
    database_name = current_database() and schema_name = current_schema() as current, database_name = current_database() as current_database \
    from duckdb_columns() where lower(table_name) in ({names}) order by database_name, schema_name, table_name, column_index";

/// One row per primary key and UNIQUE constraint of every table whose name is one of `{names}`,
/// its columns as JSON text, which every host reads alike.
const KEYS_QUERY: &str = "select database_name, schema_name, table_name, to_json(constraint_column_names)::varchar \
    from duckdb_constraints() where constraint_type in ('PRIMARY KEY', 'UNIQUE') and lower(table_name) in ({names}) \
    order by database_name, schema_name, table_name, constraint_index";

/// Every macro and its definition: a macro may call `unnest`, itself or through another macro,
/// and so make a row of each element (`generate_subscripts`, `regexp_split_to_table`).
const MACROS_QUERY: &str = "select lower(function_name), macro_definition from duckdb_functions() where function_type = 'macro' and macro_definition is not null";

/// A table or view as the catalog has it.
#[derive(Debug, Clone, Default)]
pub struct Table {
    pub database: String,
    pub schema: String,
    pub name: String,
    /// Whether an unqualified name reads it first: it is in the current database and schema.
    pub current: bool,
    /// Whether it is in the current database, whose schema a two-part name may name.
    pub current_database: bool,
    pub columns: Vec<TableColumn>,
    /// Each primary key and UNIQUE constraint, its columns lowercased.
    pub keys: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct TableColumn {
    pub name: String,
    pub not_null: bool,
    pub comment: Option<String>,
}

/// The catalog's answers for the tables a statement names, and the functions that make rows.
#[derive(Debug, Clone)]
pub struct Catalog {
    tables: Vec<Table>,
    /// `unnest` and every macro that calls it, by lowercased name: a call of one in a select list
    /// makes a row of each element.
    pub row_makers: HashSet<String>,
}

impl Default for Catalog {
    fn default() -> Self {
        Catalog { tables: Vec::new(), row_makers: HashSet::from(["unnest".to_string()]) }
    }
}

impl Catalog {
    /// The table a name as written resolves to: `t`, `s.t` or `d.s.t`, each part compared without
    /// case. An unqualified name is the current schema's table where it has one, else the one
    /// table of that name anywhere; a name the catalog has twice otherwise resolves to none.
    pub fn table(&self, parts: &[String]) -> Option<&Table> {
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let Some((name, qualifiers)) = parts.split_last() else { return None };
        let found: Vec<&Table> = self
            .tables
            .iter()
            .filter(|t| same(&t.name, name))
            .filter(|t| match qualifiers {
                [] => true,
                // A schema of the current database, or a database whose schema is `main`.
                [q] => (same(&t.schema, q) && t.current_database) || (same(&t.database, q) && same(&t.schema, "main")),
                [d, s] => same(&t.database, d) && same(&t.schema, s),
                _ => false,
            })
            .collect();
        if qualifiers.is_empty() {
            if let Some(current) = found.iter().find(|t| t.current) {
                return Some(current);
            }
        }
        match found.as_slice() {
            [one] => Some(one),
            _ => None,
        }
    }
}

/// The catalog for the tables `names` (as written, each a list of parts), asked in one round; an
/// empty catalog where the statement names none or DuckDB refuses the queries.
pub fn read(host: &Host, names: &[Vec<String>]) -> Lazy<Catalog> {
    let macros = host.query(MACROS_QUERY, Vec::new(), Scope::Session);
    let mut wanted: Vec<String> = names.iter().filter_map(|n| n.last()).map(|n| n.to_lowercase()).collect();
    wanted.sort();
    wanted.dedup();
    let row_makers = row_makers(macros?.unwrap_or_default());
    if wanted.is_empty() {
        return Some(Catalog { row_makers, ..Catalog::default() });
    }
    let list = wanted.iter().map(|n| format!("'{}'", n.replace('\'', "''"))).collect::<Vec<_>>().join(", ");
    let columns = host.query(&COLUMNS_QUERY.replace("{names}", &list), Vec::new(), Scope::Statement);
    let keys = host.query(&KEYS_QUERY.replace("{names}", &list), Vec::new(), Scope::Statement);
    let (columns, keys) = (columns?, keys?);
    let text = |row: &[Value], k: usize| row.get(k).and_then(as_str).unwrap_or_default().to_string();
    let mut tables: Vec<Table> = Vec::new();
    for row in columns.unwrap_or_default() {
        let (database, schema, name) = (text(&row, 0), text(&row, 1), text(&row, 2));
        let at = match tables.iter().position(|t| t.database == database && t.schema == schema && t.name == name) {
            Some(at) => at,
            None => {
                let current = row.get(6).and_then(as_bool).unwrap_or(false);
                let current_database = row.get(7).and_then(as_bool).unwrap_or(false);
                tables.push(Table { database, schema, name, current, current_database, ..Table::default() });
                tables.len() - 1
            }
        };
        // A column the catalog does not say is NOT NULL may hold NULL.
        let not_null = row.get(4).and_then(as_bool).map(|nullable| !nullable).unwrap_or(false);
        let comment = row.get(5).and_then(as_str).filter(|c| !c.is_empty()).map(String::from);
        tables[at].columns.push(TableColumn { name: text(&row, 3), not_null, comment });
    }
    for row in keys.unwrap_or_default() {
        let (database, schema, name) = (text(&row, 0), text(&row, 1), text(&row, 2));
        let Some(table) = tables.iter_mut().find(|t| t.database == database && t.schema == schema && t.name == name) else { continue };
        let columns = match row.get(3) {
            Some(Value::String(json)) => serde_json::from_str::<Vec<String>>(json).unwrap_or_default(),
            Some(Value::Array(columns)) => columns.iter().filter_map(as_str).map(String::from).collect(),
            _ => continue,
        };
        let key: Vec<String> = columns.iter().map(|c| c.to_lowercase()).collect();
        if !key.is_empty() {
            table.keys.push(key);
        }
    }
    Some(Catalog { tables, row_makers })
}

/// `unnest` and every macro whose definition calls it or another such macro, by name.
fn row_makers(macros: Vec<Vec<Value>>) -> HashSet<String> {
    let definitions: Vec<(String, String)> = macros.iter().filter_map(|r| Some((as_str(r.first()?)?.to_string(), as_str(r.get(1)?)?.to_lowercase()))).collect();
    let mut makers: HashSet<String> = HashSet::from(["unnest".to_string()]);
    loop {
        let before = makers.len();
        for (name, definition) in &definitions {
            if !makers.contains(name) && makers.iter().any(|m| calls_name(definition, m)) {
                makers.insert(name.clone());
            }
        }
        if makers.len() == before {
            return makers;
        }
    }
}

/// Whether `text` calls the function `name`: the name as a word, then an opening parenthesis.
fn calls_name(text: &str, name: &str) -> bool {
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(at) = text[from..].find(name).map(|i| i + from) {
        let end = at + name.len();
        let before = at == 0 || !word(bytes[at - 1]);
        let after = text[end..].trim_start().starts_with('(');
        if before && after {
            return true;
        }
        from = end;
    }
    false
}

#[cfg(test)]
pub fn of(tables: Vec<Table>) -> Catalog {
    Catalog { tables, ..Catalog::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_macro_that_reaches_unnest_makes_rows() {
        let macros = vec![
            vec![json!("generate_subscripts"), json!("unnest(generate_series(1, array_length(arr, dim)))")],
            vec![json!("my_rows"), json!("generate_subscripts(x, 1) + 0")],
            vec![json!("my_unnest_count"), json!("list_count(unnest_like(x))")],
            vec![json!("plain"), json!("lower(x)")],
        ];
        let makers = row_makers(macros);
        assert!(makers.contains("unnest") && makers.contains("generate_subscripts") && makers.contains("my_rows"));
        assert!(!makers.contains("plain") && !makers.contains("my_unnest_count"));
    }
}
