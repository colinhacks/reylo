//! What the analysis reads of DuckDB's catalog: the columns of the tables and views a statement
//! names, whether each may be NULL and its comment, and the tables' primary and unique keys. Both
//! are asked in one round, the names written into the query as lowercase string literals, since
//! DuckDB compares a table's name without case.

use crate::host::{as_bool, as_str, Host, Lazy};
use reylo_engine_core::api::{Analysis, Scope};
use reylo_engine_core::dialect::Dialect;
use serde_json::Value;
use sqlparser::ast::{Query, Statement};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;
use std::collections::HashSet;

/// One row per column of every table and view whose name is one of `{names}`, in each one's
/// order: where it is, whether it may be NULL, its comment, whether it is in the database and
/// schema an unqualified name reads first, whether it is in that database, its type, and whether
/// its table was made with a collation, which only the table's own text says.
const COLUMNS_QUERY: &str = "select c.database_name, c.schema_name, c.table_name, c.column_name, c.is_nullable, c.comment, \
    c.database_name = current_database() and c.schema_name = current_schema() as current, c.database_name = current_database() as current_database, c.data_type, \
    exists (select 1 from duckdb_tables() t where t.database_name = c.database_name and t.schema_name = c.schema_name and t.table_name = c.table_name and t.sql ilike '%collate%') as collated \
    from duckdb_columns() c where lower(c.table_name) in ({names}) order by c.database_name, c.schema_name, c.table_name, c.column_index";

/// One row per primary key and UNIQUE constraint of every table whose name is one of `{names}`,
/// its columns as JSON text, which every host reads alike.
const KEYS_QUERY: &str = "select database_name, schema_name, table_name, to_json(constraint_column_names)::varchar \
    from duckdb_constraints() where constraint_type in ('PRIMARY KEY', 'UNIQUE') and lower(table_name) in ({names}) \
    order by database_name, schema_name, table_name, constraint_index";

/// Every macro and its definition: a macro may call `unnest`, itself or through another macro,
/// and so make a row of each element (`generate_subscripts`, `regexp_split_to_table`).
const MACROS_QUERY: &str = "select lower(function_name), macro_definition from duckdb_functions() where function_type = 'macro' and macro_definition is not null";

/// Every macro and view a user defined, by lowercased name: its definition, and whether it is a view.
const DEFINITIONS_QUERY: &str = "select lower(function_name), macro_definition, false from duckdb_functions() where function_type in ('macro', 'table_macro') and macro_definition is not null and not internal \
    union all select lower(view_name), sql, true from duckdb_views() where not internal and sql is not null";

/// Every table macro and table function, built in or a user's: which it is, whether it is a user's
/// in the session's own schema, its definition and its parameters' names as JSON text. A macro is
/// read by its name alone, so one that shares it with another, an overload or a function, is not.
const TABLE_FUNCTIONS_QUERY: &str = "select lower(function_name), function_type = 'table_macro' and not internal and database_name = current_database() and schema_name = current_schema(), \
    macro_definition, to_json(parameters)::varchar from duckdb_functions() where function_type in ('table', 'table_macro')";

/// How many levels of table macros are read: a macro whose body reads a macro is one more.
pub const MAX_MACRO_LEVELS: usize = 6;

/// The functions that move a sequence, or run SQL they are given as text, which may.
const MOVES_A_SEQUENCE: [&str; 4] = ["nextval", "setval", "query", "query_table"];

/// Whether the statement is certain to write nothing: a SELECT that moves no sequence, itself or
/// through a view or a macro, however deep. A view counts wherever its name stands as a word, a
/// column or an alias of that name too, which can only say a read may write.
pub fn read_only(analysis: &Analysis, host: &Host) -> Lazy<bool> {
    let sql = &analysis.rewritten.sql;
    if analysis.kind != Some(reylo_engine_core::types::StatementKind::Select) {
        return Some(false);
    }
    let Ok(rows) = host.query(DEFINITIONS_QUERY, Vec::new(), Scope::Session)? else { return Some(false) };
    let mut definitions: Vec<(String, String, bool)> = rows.iter().filter_map(|r| Some((as_str(r.first()?)?.to_string(), as_str(r.get(1)?)?.to_string(), as_bool(r.get(2)?)?))).collect();
    let mut functions: Vec<String> = MOVES_A_SEQUENCE.iter().map(|f| f.to_string()).collect();
    let mut views: Vec<String> = Vec::new();
    let moves = |sql: &str, functions: &[String], views: &[String]| {
        let names: Vec<&str> = views.iter().map(String::as_str).collect();
        reylo_engine_core::errors::called(sql, Dialect::Duckdb).iter().any(|f| functions.contains(f)) || reylo_engine_core::errors::has_word(sql, Dialect::Duckdb, &names)
    };
    loop {
        let (moving, rest): (Vec<_>, Vec<_>) = definitions.into_iter().partition(|(_, definition, _)| moves(definition, &functions, &views));
        if moving.is_empty() {
            return Some(!moves(sql, &functions, &views));
        }
        for (name, _, view) in moving {
            if view { views.push(name) } else { functions.push(name) }
        }
        definitions = rest;
    }
}

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
    /// Whether it was made with a collation: a key under one holds `'a'` and `'A'`, which compare
    /// equal, so its text keys say nothing of how many rows a value matches.
    pub collated: bool,
}

#[derive(Debug, Clone)]
pub struct TableColumn {
    pub name: String,
    pub not_null: bool,
    pub comment: Option<String>,
    /// Its type as DuckDB writes it, empty where the catalog does not say.
    pub ty: String,
}

/// The catalog's answers for the tables a statement names, and the functions that make rows.
#[derive(Debug, Clone)]
pub struct Catalog {
    tables: Vec<Table>,
    /// `unnest` and every macro that calls it, by lowercased name: a call of one in a select list
    /// makes a row of each element.
    pub row_makers: HashSet<String>,
    /// Each table macro whose body is read, by lowercased name.
    pub table_macros: std::collections::HashMap<String, TableMacro>,
}

/// A table macro: its parameters' names, lowercased, and its body, which the bind reads in its place.
#[derive(Debug, Clone)]
pub struct TableMacro {
    pub params: Vec<String>,
    pub body: Query,
    /// Its definition, whose names a freshness check reads.
    pub text: String,
}

/// The table macros in `rows` that are one name each, a user's in the session's schema, of a body
/// that reads as one query.
fn table_macros(rows: &[Vec<Value>]) -> std::collections::HashMap<String, TableMacro> {
    let mut by_name: std::collections::HashMap<String, Vec<&Vec<Value>>> = std::collections::HashMap::new();
    for row in rows {
        if let Some(name) = row.first().and_then(as_str) {
            by_name.entry(name.to_string()).or_default().push(row);
        }
    }
    let mut out = std::collections::HashMap::new();
    for (name, rows) in by_name {
        let [row] = rows.as_slice() else { continue };
        if row.get(1).and_then(as_bool) != Some(true) {
            continue;
        }
        let Some(text) = row.get(2).and_then(as_str) else { continue };
        let params: Vec<String> = match row.get(3) {
            Some(Value::String(json)) => serde_json::from_str::<Vec<String>>(json).unwrap_or_default(),
            Some(Value::Array(names)) => names.iter().filter_map(as_str).map(String::from).collect(),
            _ => continue,
        };
        let Ok(statements) = Parser::parse_sql(&DuckDbDialect {}, text) else { continue };
        let [Statement::Query(body)] = statements.as_slice() else { continue };
        out.insert(name, TableMacro { params: params.iter().map(|p| p.to_lowercase()).collect(), body: (**body).clone(), text: text.to_string() });
    }
    out
}

/// The tables and macros a macro's body names, each as a list of parts.
pub fn body_relations(body: &Query) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    let _ = sqlparser::ast::visit_relations(body, |name| {
        found.push(crate::nulls::parts(name));
        std::ops::ControlFlow::<()>::Continue(())
    });
    found
}

/// `names`, with what the table macros among them read, however deep; and the macros reached.
pub fn through_macros(names: &[Vec<String>], macros: &std::collections::HashMap<String, TableMacro>) -> (Vec<Vec<String>>, Vec<String>) {
    let mut all = names.to_vec();
    let mut reached: Vec<String> = Vec::new();
    let mut at = 0;
    while at < all.len() {
        if let [one] = all[at].as_slice() {
            let name = one.to_lowercase();
            if let (Some(m), false) = (macros.get(&name), reached.contains(&name)) {
                reached.push(name);
                all.extend(body_relations(&m.body));
            }
        }
        at += 1;
    }
    (all, reached)
}

impl Default for Catalog {
    fn default() -> Self {
        Catalog { tables: Vec::new(), row_makers: HashSet::from(["unnest".to_string()]), table_macros: Default::default() }
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
    let table_functions = host.query(TABLE_FUNCTIONS_QUERY, Vec::new(), Scope::Session);
    // A table macro's body is read as the macro, so the catalog holds the tables it names, a round
    // after the macros are known.
    let read_macros = table_macros(table_functions.as_ref().and_then(|r| r.as_ref().ok()).map_or(&[], |rows| rows.as_slice()));
    let (names, _) = through_macros(names, &read_macros);
    let mut wanted: Vec<String> = names.iter().filter_map(|n| n.last()).map(|n| n.to_lowercase()).collect();
    wanted.sort();
    wanted.dedup();
    if wanted.is_empty() {
        let (macros, _) = (macros?, table_functions?);
        return Some(Catalog { row_makers: row_makers(macros.unwrap_or_default()), table_macros: read_macros, ..Catalog::default() });
    }
    let list = wanted.iter().map(|n| format!("'{}'", n.replace('\'', "''"))).collect::<Vec<_>>().join(", ");
    let columns = host.query(&COLUMNS_QUERY.replace("{names}", &list), Vec::new(), Scope::Statement);
    let keys = host.query(&KEYS_QUERY.replace("{names}", &list), Vec::new(), Scope::Statement);
    // Every need is recorded before any is awaited, so these are asked in the same round.
    let (macros, _, columns, keys) = (macros?, table_functions?, columns?, keys?);
    let row_makers = row_makers(macros.unwrap_or_default());
    let text = |row: &[Value], k: usize| row.get(k).and_then(as_str).unwrap_or_default().to_string();
    let mut tables: Vec<Table> = Vec::new();
    for row in columns.unwrap_or_default() {
        let (database, schema, name) = (text(&row, 0), text(&row, 1), text(&row, 2));
        let at = match tables.iter().position(|t| t.database == database && t.schema == schema && t.name == name) {
            Some(at) => at,
            None => {
                let current = row.get(6).and_then(as_bool).unwrap_or(false);
                let current_database = row.get(7).and_then(as_bool).unwrap_or(false);
                let collated = row.get(9).and_then(as_bool).unwrap_or(false);
                tables.push(Table { database, schema, name, current, current_database, collated, ..Table::default() });
                tables.len() - 1
            }
        };
        // A column the catalog does not say is NOT NULL may hold NULL.
        let not_null = row.get(4).and_then(as_bool).map(|nullable| !nullable).unwrap_or(false);
        let comment = row.get(5).and_then(as_str).filter(|c| !c.is_empty()).map(String::from);
        tables[at].columns.push(TableColumn { name: text(&row, 3), not_null, comment, ty: text(&row, 8) });
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
    Some(Catalog { tables, row_makers, table_macros: read_macros })
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

    #[test]
    fn the_macros_columns_and_keys_are_asked_in_one_round() {
        let answers = reylo_engine_core::api::Answers::new();
        let host = Host::new(&answers);
        assert!(read(&host, &[vec!["t".to_string()]]).is_none());
        assert_eq!(host.needs().len(), 4);
    }
}
