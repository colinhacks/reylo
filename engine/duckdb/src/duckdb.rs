//! The DuckDB analysis. DuckDB's prepared statement is the type source: each parameter's type
//! from where it stands, and each result column's name and type. What it says of NULL does not
//! hold, so whether each column may be NULL, and how many rows the statement returns, are read off
//! a `sqlparser` tree of the statement with the catalog's NOT NULL columns and keys
//! ([`crate::nulls`]). A statement the tree does not read (`PIVOT`, `SUMMARIZE`, an `ASOF` join)
//! is typed from the describe alone: every column may be NULL, and nothing bounds its rows.

use crate::api::{Analysis, Column, DbError, Diagnostic, DuckdbDescribe, Param};
use crate::catalog::{self, Catalog};
use crate::host::{Host, Lazy};
use crate::nulls::{self, Ctx, Out};
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::lex::{self, Lexed, Target};
use reylo_engine_core::types::{Cardinality, StatementKind, Type};
use sqlparser::ast::{AssignmentTarget, DoUpdate, Expr, FromTable, OnConflictAction, OnInsert, Query, SetExpr, SqliteOnConflict, Statement, TableFactor, TableObject, Value};
use sqlparser::dialect::DuckDbDialect;
use sqlparser::parser::Parser;
use std::borrow::Cow;
use std::collections::HashSet;
use std::ops::ControlFlow;

/// The statements the DuckDB analysis reads; any other is a command ([`lex::Statement`]).
const KINDS: &[&str] = &["select", "with", "from", "values", "table", "insert", "update", "delete", "pivot", "unpivot", "summarize"];

/// The statement types whose prepared statement returns rows whatever they are.
const RETURN_ROWS: &[&str] = &["SELECT", "EXPLAIN", "CALL", "PRAGMA", "RELATION", "EXECUTE", "LOGICAL_PLAN"];

/// The analysis of a statement, its structured parameters (`::nullable(…)`, `::json('…')`)
/// written first.
pub fn analyze_text(sql: &str, host: &Host) -> Lazy<Analysis> {
    reylo_engine_core::structured::analyze(sql, Dialect::Duckdb, host, &|sql, marked| analyze(sql, host, marked), &|diagnostics| reylo_engine_core::analysis::bare_of(sql, Dialect::Duckdb, diagnostics))
}

/// The diagnostic code of a column or a parameter that holds a struct whose fields have no names
/// ([`crate::types::unnamed`]).
pub const UNNAMED_FIELDS: u32 = 22;

/// The analysis of a statement whose placeholders are Reylo's and DuckDB's own; `marked` names the
/// parameters cast to `nullable(…)`.
fn analyze(sql: &str, host: &Host, marked: &[String]) -> Lazy<Analysis> {
    let lexed = match reylo_engine_core::analysis::lexed(sql, Dialect::Duckdb) {
        Ok(lexed) => lexed,
        Err(bare) => return Some(bare),
    };
    let whole = |message: String, code: u32| Some(reylo_engine_core::analysis::bare(sql, &lexed, Dialect::Duckdb, vec![Diagnostic { message, start: 0, length: sql.trim_end().len().max(1), code, server_code: None }]));
    let statement = match lex::one_statement(sql, &lexed, KINDS) {
        Ok(statement) => statement,
        Err(message) => return whole(message, 10),
    };
    // DuckDB numbers its parameters from 1 without a gap, and reads `$1 … $3` as two.
    if lexed.style == lex::Style::Positional {
        if let Some(k) = lexed.params.iter().enumerate().position(|(k, name)| name.parse::<usize>() != Ok(k + 1)) {
            let at = lexed.placeholders.iter().find(|p| p.name == lexed.params[k]).map_or((0, sql.trim_end().len()), |p| (p.start, p.end - p.start));
            let message = format!("DuckDB numbers parameters from 1 without a gap, and this statement has no ${}: number them in order", k + 1);
            return Some(reylo_engine_core::analysis::bare(sql, &lexed, Dialect::Duckdb, vec![Diagnostic { message, start: at.0, length: at.1.max(1), code: 1, server_code: None }]));
        }
    }
    let rewritten = lex::rewrite(sql, &lexed, Target::Dollar);
    // Asked in one round: the describe, and the catalog of every table the statement names.
    let raw = host.describe(&rewritten.sql);
    let parsed = parse(&rewritten.sql);
    let catalog = catalog::read(host, &parsed.as_ref().map(relations).unwrap_or_default());
    let (raw, catalog) = (raw?, catalog?);
    let raw = match raw {
        Ok(raw) => raw,
        Err(e) => return Some(refused(sql, &lexed, &rewritten, &e, matches!(statement, lex::Statement::Command(_)))),
    };
    // A parameter DuckDB cannot type from where it stands (`id + :n`, `select :x`) leaves it
    // unable to type the statement at all: it describes one column, `unknown`, of no type.
    if raw.columns.len() == 1 && raw.columns[0].ty.id == "INVALID" && raw.params.iter().any(|p| p.ty.is_none()) {
        let diagnostics = raw
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| p.ty.is_none())
            .filter_map(|(i, _)| lexed.params.get(i))
            .filter_map(|name| lexed.placeholders.iter().find(|p| &p.name == name))
            .map(|p| {
                let written = &sql[p.start..p.end];
                let message = format!("DuckDB cannot tell the type of {written} from where it stands, and describes none of the statement's columns until it can: cast it, as in {written}::integer");
                Diagnostic { message, start: p.start, length: p.end - p.start, code: 6, server_code: None }
            })
            .collect::<Vec<_>>();
        if !diagnostics.is_empty() {
            return Some(reylo_engine_core::analysis::bare(sql, &lexed, Dialect::Duckdb, diagnostics));
        }
    }
    // DuckDB runs a PIVOT as several statements, the columns of the last made from the values the
    // first reads, so it describes none before it runs.
    if raw.statement == "MULTI" {
        return whole("DuckDB does not describe the columns of this statement before it runs, since they are made from the values it reads: name them, as in PIVOT … ON level IN ('info', 'warn')".to_string(), 10);
    }
    // Where the engine's lexer and DuckDB part text from code differently, a placeholder of the
    // one is text to the other.
    if raw.params.len() != lexed.params.len() {
        let message = format!("DuckDB finds {} parameter(s) in this statement and the engine {}, so the engine does not read its strings and comments as DuckDB does", raw.params.len(), lexed.params.len());
        return whole(message, 1);
    }
    // The parameters the caller may pass NULL for, by the number `$n` binds each at.
    let number = |name: &String| lexed.params.iter().position(|p| p == name).map(|i| (i + 1).to_string());
    let mut nullable: HashSet<String> = marked.iter().filter_map(number).collect();
    if let Some(statement) = &parsed {
        nullable.extend(nullable_params(statement, &catalog));
    }
    let ctx = Ctx { catalog: &catalog, nullable_params: &nullable, macro_params: &[], macro_level: 0 };
    let shaped = parsed.as_ref().map(write_of);
    let rows = returns_rows(&raw, shaped.as_deref());
    // What the tree proves of each column, where its columns are the describe's: as many, and
    // named alike where the tree names one.
    let proven = shaped.as_deref().filter(|_| rows).and_then(|s| outputs(s, &ctx)).filter(|out| {
        out.len() == raw.columns.len() && out.iter().zip(&raw.columns).all(|(o, c)| o.name.as_deref().is_none_or(|n| n.eq_ignore_ascii_case(&c.name)))
    });
    let columns: Vec<Column> = if rows {
        raw.columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let out = proven.as_ref().map(|p| &p[i]);
                Column { comment: out.and_then(|o| o.comment.clone()), ..Column::new(c.name.clone(), crate::types::of(&c.ty), !out.is_some_and(|o| o.not_null)) }
            })
            .collect()
    } else {
        Vec::new()
    };
    let params: Vec<Param> = lexed
        .params
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let n = (i + 1).to_string();
            let described = raw.params.iter().find(|p| p.name == n).or_else(|| raw.params.get(i));
            let ty = described.and_then(|p| p.ty.as_ref()).map_or_else(Type::unknown, crate::types::of);
            Param { nullable: nullable.contains(&n), ..Param::new(name.clone(), ty) }
        })
        .collect();
    let kind = match (shaped.as_deref(), &statement) {
        (Some(Statement::Query(_)), _) => "select".to_string(),
        (Some(Statement::Insert(_)), _) => "insert".to_string(),
        (Some(Statement::Update(_)), _) => "update".to_string(),
        (Some(Statement::Delete(_)), _) => "delete".to_string(),
        (_, lex::Statement::Command(word)) => word.clone(),
        _ => match raw.statement.as_str() {
            "" => lex::first_word(sql, &lexed),
            other => other.to_ascii_lowercase(),
        },
    };
    let (cardinality, reason) = if !rows {
        (Cardinality::None, None)
    } else {
        match shaped.as_deref() {
            Some(statement) => {
                let (rows, reason) = cardinality(statement, &ctx);
                (match rows {
                    "one" => Cardinality::One,
                    "maybeOne" => Cardinality::AtMostOne,
                    _ => Cardinality::Many,
                }, reason)
            }
            None => (Cardinality::Many, Some("the engine does not read this statement's syntax, so nothing bounds its rows".to_string())),
        }
    };
    // A struct whose fields have no names is refused where a column or a parameter has one.
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    if rows {
        for c in raw.columns.iter().filter(|c| crate::types::unnamed(&c.ty)) {
            let message = format!("Column \"{}\" holds a struct whose fields have no names, as (1, 'a') and row(…) build, and a value's fields are read by their names: name them, as in {{'a': 1, 'b': 'x'}} or struct_pack(a := 1, b := 'x')", c.name);
            diagnostics.push(Diagnostic { message, start: 0, length: sql.trim_end().len().max(1), code: UNNAMED_FIELDS, server_code: None });
        }
    }
    for (i, name) in lexed.params.iter().enumerate() {
        let n = (i + 1).to_string();
        let described = raw.params.iter().find(|p| p.name == n).or_else(|| raw.params.get(i));
        if !described.and_then(|p| p.ty.as_ref()).is_some_and(crate::types::unnamed) {
            continue;
        }
        let Some(p) = lexed.placeholders.iter().find(|p| &p.name == name) else { continue };
        let written = &sql[p.start..p.end];
        let message = format!("{written} takes a struct whose fields have no names, and a value's fields are read by their names: cast it to a struct that names them, as in {written}::struct(a integer, b varchar)");
        diagnostics.push(Diagnostic { message, start: p.start, length: p.end - p.start, code: UNNAMED_FIELDS, server_code: None });
    }
    let rewritten_sql = rewritten.sql.clone();
    let mut analysis = Analysis {
        kind: (!kind.is_empty()).then(|| StatementKind::of(&kind)),
        comment: None,
        params,
        columns,
        cardinality,
        cardinality_reason: reason,
        key: None,
        ordered: false,
        rewritten,
        style: lexed.style,
        diagnostics,
        unresolved: Vec::new(),
        structured: Vec::new(),
        fresh: None,
        errors: None,
        read_only: false,
        notes: Vec::new(),
        tables: parsed.as_ref().map(|p| tables_through_macros(p, &catalog, &rewritten_sql)),
        domains: Default::default(),
    };
    reylo_engine_core::analysis::clashes(sql, &mut analysis, &|_| None);
    Some(analysis)
}

/// The one statement of the text as `sqlparser`'s DuckDB dialect reads it, if it reads it.
fn parse(sql: &str) -> Option<Statement> {
    let mut statements = Parser::parse_sql(&DuckDbDialect {}, sql).ok()?;
    if statements.len() == 1 {
        statements.pop()
    } else {
        None
    }
}

/// A write that WITH queries feed (`WITH x AS (…) INSERT …`), which the tree reads as a query whose
/// body is the write, as that write: an INSERT's source takes the WITH queries, which it reads.
fn write_of(statement: &Statement) -> Cow<'_, Statement> {
    let Statement::Query(q) = statement else { return Cow::Borrowed(statement) };
    match q.body.as_ref() {
        SetExpr::Insert(Statement::Insert(i)) => {
            let mut i = i.clone();
            if let (Some(with), Some(source)) = (&q.with, i.source.as_mut()) {
                match &mut source.with {
                    Some(inner) => {
                        inner.cte_tables.splice(0..0, with.cte_tables.iter().cloned());
                        inner.recursive |= with.recursive;
                    }
                    None => source.with = Some(with.clone()),
                }
            }
            Cow::Owned(Statement::Insert(i))
        }
        SetExpr::Update(write) | SetExpr::Delete(write) => Cow::Borrowed(write),
        _ => Cow::Borrowed(statement),
    }
}

/// Whether the statement returns rows: a query, a write with RETURNING, and the statements DuckDB
/// returns rows of (`EXPLAIN`, `CALL`, a `PRAGMA` that reads). A write without RETURNING, DDL and
/// a setting return none, though DuckDB describes a count or a flag for them.
fn returns_rows(raw: &DuckdbDescribe, parsed: Option<&Statement>) -> bool {
    match parsed {
        Some(Statement::Query(_)) => true,
        Some(Statement::Insert(i)) => i.returning.is_some(),
        Some(Statement::Update(u)) => u.returning.is_some(),
        Some(Statement::Delete(d)) => d.returning.is_some(),
        _ => match raw.statement.as_str() {
            "" => !raw.columns.is_empty(),
            s if RETURN_ROWS.contains(&s) => true,
            // A write the tree does not read returns its count alone, unless it has a RETURNING.
            "INSERT" | "UPDATE" | "DELETE" | "MERGE_INTO" => !(raw.columns.len() == 1 && raw.columns[0].name == "Count"),
            _ => false,
        },
    }
}

/// What the tree proves of the result columns of a query or of a write's RETURNING.
fn outputs(statement: &Statement, ctx: &Ctx) -> Option<Vec<Out>> {
    match statement {
        Statement::Query(q) => nulls::query(q, ctx, &Vec::new()),
        Statement::Insert(i) => {
            let TableObject::TableName(name) = &i.table else { return None };
            let alias = i.table_alias.as_ref().map(|a| a.alias.value.clone());
            nulls::returning(i.returning.as_ref()?, &nulls::parts(name), alias, ctx)
        }
        Statement::Update(u) => {
            if u.from.is_some() || !u.table.joins.is_empty() {
                return None;
            }
            let TableFactor::Table { name, alias, .. } = &u.table.relation else { return None };
            nulls::returning(u.returning.as_ref()?, &nulls::parts(name), alias.as_ref().map(|a| a.name.value.clone()), ctx)
        }
        Statement::Delete(d) => {
            let (TableFactor::Table { name, alias, .. }, None) = (&delete_table(d)?.relation, &d.using) else { return None };
            nulls::returning(d.returning.as_ref()?, &nulls::parts(name), alias.as_ref().map(|a| a.name.value.clone()), ctx)
        }
        _ => None,
    }
}

fn delete_table(d: &sqlparser::ast::Delete) -> Option<&sqlparser::ast::TableWithJoins> {
    match &d.from {
        FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => match tables.as_slice() {
            [one] if one.joins.is_empty() => Some(one),
            _ => None,
        },
    }
}

/// How many rows a statement that returns rows returns, and why not one where it is not proven so.
fn cardinality(statement: &Statement, ctx: &Ctx) -> (&'static str, Option<String>) {
    match statement {
        Statement::Query(q) => nulls::cardinality(q, ctx),
        Statement::Insert(i) => {
            // A row DuckDB skips on a conflict is not returned, nor one a DO UPDATE's WHERE leaves
            // as it was.
            let skips = matches!(i.or, Some(SqliteOnConflict::Ignore)) || matches!(&i.on, Some(OnInsert::OnConflict(c)) if matches!(&c.action, OnConflictAction::DoNothing | OnConflictAction::DoUpdate(DoUpdate { selection: Some(_), .. })));
            let (card, reason) = match i.source.as_deref() {
                None => ("one", None),
                Some(Query { body, .. }) if matches!(body.as_ref(), SetExpr::Values(v) if v.rows.len() != 1) => ("many", Some("it inserts several rows".to_string())),
                Some(source) => nulls::cardinality(source, ctx),
            };
            match (card, skips) {
                ("one", true) => ("maybeOne", Some("a row that conflicts is skipped, and not returned".into())),
                _ => (card, reason),
            }
        }
        Statement::Update(u) => match (&u.table.relation, &u.selection, u.from.is_none() && u.table.joins.is_empty()) {
            (TableFactor::Table { name, alias, args: None, .. }, Some(filter), true) => keyed(name, alias, filter, ctx),
            _ => ("many", Some("nothing bounds the rows it updates to one".into())),
        },
        Statement::Delete(d) => match (delete_table(d).map(|t| &t.relation), &d.selection, d.using.is_none()) {
            (Some(TableFactor::Table { name, alias, args: None, .. }), Some(filter), true) => keyed(name, alias, filter, ctx),
            _ => ("many", Some("nothing bounds the rows it deletes to one".into())),
        },
        _ => ("many", None),
    }
}

/// `maybeOne` where the filter holds a key of the table equal to one value each.
fn keyed(name: &sqlparser::ast::ObjectName, alias: &Option<sqlparser::ast::TableAlias>, filter: &Expr, ctx: &Ctx) -> (&'static str, Option<String>) {
    let parts = nulls::parts(name);
    let qualifier = alias.as_ref().map(|a| a.name.value.clone()).or_else(|| parts.last().cloned());
    match ctx.catalog.table(&parts) {
        Some(table) if nulls::key_equal(filter, &table.keys, qualifier.as_deref(), &[]) => ("maybeOne", None),
        _ => ("many", Some("nothing bounds its rows to one".into())),
    }
}

/// Every name the statement reads or writes as a relation's, which the catalog is asked for.
fn relations(statement: &Statement) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    let _ = sqlparser::ast::visit_relations(statement, |name| {
        found.push(nulls::parts(name));
        ControlFlow::<()>::Continue(())
    });
    found
}

/// [`tables`], with what the table macros the statement calls read: a type their bodies settled
/// holds only while those and what they read do.
fn tables_through_macros(statement: &Statement, catalog: &Catalog, sql: &str) -> reylo_engine_core::fresh::Tables {
    let mut found = tables(statement);
    let (_, reached) = catalog::through_macros(&relations(statement), &catalog.table_macros);
    if reached.is_empty() {
        return found;
    }
    found.read.retain(|t| !reached.contains(&t.to_lowercase()));
    let mut text = sql.to_string();
    for name in &reached {
        let Some(read) = catalog.table_macros.get(name) else { continue };
        text.push('\n');
        text.push_str(&read.text);
        for table in tables(&Statement::Query(Box::new(read.body.clone()))).read {
            if !reached.contains(&table.to_lowercase()) && !found.read.contains(&table) && !found.write.contains(&table) {
                found.read.push(table);
            }
        }
    }
    found.functions = reached;
    found.expanded = Some(text);
    found
}

/// The tables the statement reads and writes as it names them, for the freshness facts: not a
/// WITH query's name.
fn tables(statement: &Statement) -> reylo_engine_core::fresh::Tables {
    let mut ctes: HashSet<String> = HashSet::new();
    let _ = sqlparser::ast::visit_statements(statement, |s| {
        if let Statement::Query(q) = s {
            for cte in q.with.iter().flat_map(|w| &w.cte_tables) {
                ctes.insert(cte.alias.name.value.to_lowercase());
            }
        }
        ControlFlow::<()>::Continue(())
    });
    let _ = sqlparser::ast::visit_expressions(statement, |e| {
        if let Expr::Subquery(q) | Expr::Exists { subquery: q, .. } | Expr::InSubquery { subquery: q, .. } = e {
            for cte in q.with.iter().flat_map(|w| &w.cte_tables) {
                ctes.insert(cte.alias.name.value.to_lowercase());
            }
        }
        ControlFlow::<()>::Continue(())
    });
    let write: Vec<String> = match write_of(statement).as_ref() {
        Statement::Insert(i) => match &i.table {
            TableObject::TableName(name) => vec![nulls::parts(name).join(".")],
            _ => Vec::new(),
        },
        Statement::Update(u) => match &u.table.relation {
            TableFactor::Table { name, .. } => vec![nulls::parts(name).join(".")],
            _ => Vec::new(),
        },
        Statement::Delete(d) => match delete_table(d).map(|t| &t.relation) {
            Some(TableFactor::Table { name, .. }) => vec![nulls::parts(name).join(".")],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    let mut read: Vec<String> = Vec::new();
    for parts in relations(statement) {
        let name = parts.join(".");
        if (parts.len() == 1 && ctes.contains(&parts[0].to_lowercase())) || write.contains(&name) || read.contains(&name) {
            continue;
        }
        read.push(name);
    }
    reylo_engine_core::fresh::Tables { read, write, ..Default::default() }
}

/// The parameters (by the number `$n` binds each at) the statement lets be NULL: one it tests
/// for NULL or coalesces, and one written whole and bare to a column that may hold NULL.
fn nullable_params(statement: &Statement, catalog: &Catalog) -> HashSet<String> {
    fn placeholder(e: &Expr) -> Option<String> {
        match e {
            Expr::Value(v) => match &v.value {
                Value::Placeholder(p) => p.strip_prefix('$').map(String::from),
                _ => None,
            },
            Expr::Nested(inner) => placeholder(inner),
            Expr::Cast { expr, .. } => placeholder(expr),
            _ => None,
        }
    }
    let bare = |e: &Expr| match e {
        Expr::Value(v) => match &v.value {
            Value::Placeholder(p) => p.strip_prefix('$').map(String::from),
            _ => None,
        },
        _ => None,
    };
    let mut found = HashSet::new();
    let _ = sqlparser::ast::visit_expressions(statement, |e| {
        match e {
            Expr::IsNull(inner) | Expr::IsNotNull(inner) => found.extend(placeholder(inner)),
            Expr::IsDistinctFrom(a, b) | Expr::IsNotDistinctFrom(a, b) => {
                found.extend(placeholder(a));
                found.extend(placeholder(b));
            }
            Expr::Function(f) if nulls::builtin(f).is_some_and(|n| matches!(n.as_str(), "coalesce" | "ifnull" | "nvl")) => {
                if let sqlparser::ast::FunctionArguments::List(list) = &f.args {
                    for a in &list.args {
                        if let sqlparser::ast::FunctionArg::Unnamed(sqlparser::ast::FunctionArgExpr::Expr(e)) = a {
                            found.extend(placeholder(e));
                        }
                    }
                }
            }
            _ => {}
        }
        ControlFlow::<()>::Continue(())
    });
    let may_be_null = |table: &[String], column: &str| catalog.table(table).and_then(|t| t.columns.iter().find(|c| c.name.eq_ignore_ascii_case(column))).is_some_and(|c| !c.not_null);
    match write_of(statement).as_ref() {
        Statement::Insert(i) => {
            let TableObject::TableName(name) = &i.table else { return found };
            let table = nulls::parts(name);
            let columns: Vec<String> = if i.columns.is_empty() {
                catalog.table(&table).map(|t| t.columns.iter().map(|c| c.name.clone()).collect()).unwrap_or_default()
            } else {
                i.columns.iter().filter_map(|c| nulls::parts(c).pop()).collect()
            };
            if let Some(SetExpr::Values(values)) = i.source.as_deref().map(|q| q.body.as_ref()) {
                for row in &values.rows {
                    for (e, column) in row.iter().zip(&columns) {
                        if let Some(p) = bare(e) {
                            if may_be_null(&table, column) {
                                found.insert(p);
                            }
                        }
                    }
                }
            }
        }
        Statement::Update(u) => {
            if let TableFactor::Table { name, .. } = &u.table.relation {
                let table = nulls::parts(name);
                for a in &u.assignments {
                    if let (AssignmentTarget::ColumnName(column), Some(p)) = (&a.target, bare(&a.value)) {
                        if nulls::parts(column).pop().is_some_and(|c| may_be_null(&table, &c)) {
                            found.insert(p);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    found
}

/// The analysis of a statement DuckDB refused to prepare: its error at the place DuckDB names.
fn refused(sql: &str, lexed: &Lexed, rewritten: &lex::Rewritten, e: &DbError, command: bool) -> Analysis {
    let (message, at) = place(&e.message, &rewritten.sql);
    let message = if command { lex::refused_command(&message) } else { message };
    let (start, length) = match at {
        Some(at) => {
            let start = rewritten.to_original(at).min(sql.len());
            let word = sql[start..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').map(char::len_utf8).sum::<usize>();
            (start, word.max(1))
        }
        None => (0, sql.trim_end().len().max(1)),
    };
    reylo_engine_core::analysis::bare(sql, lexed, Dialect::Duckdb, vec![Diagnostic { message, start, length, code: 6, server_code: None }])
}

/// DuckDB's error message without the context it prints under it, and the byte offset in `text`
/// its caret points at: `LINE 2: select nope from t` over a line of spaces and a `^`, the line
/// cut to a window with `...` around it where it is long.
pub fn place(message: &str, text: &str) -> (String, Option<usize>) {
    let Some(at) = message.find("\nLINE ") else { return (message.trim_end().to_string(), None) };
    let head = message[..at].trim_end().to_string();
    let rest = &message[at + 1..];
    let mut lines = rest.lines();
    let (Some(context), Some(caret)) = (lines.next(), lines.next()) else { return (head, None) };
    let Some(colon) = context.find(": ") else { return (head, None) };
    let Ok(number) = context["LINE ".len()..colon].trim().parse::<usize>() else { return (head, None) };
    let prefix = colon + 2;
    let Some(column) = caret.find('^').and_then(|c| c.checked_sub(prefix)) else { return (head, None) };
    let shown = &context[prefix..];
    // The line the context shows, by its number, as offsets into `text`.
    let mut start = 0;
    for _ in 1..number {
        match text[start..].find('\n') {
            Some(nl) => start += nl + 1,
            None => return (head, None),
        }
    }
    let end = text[start..].find('\n').map_or(text.len(), |nl| start + nl);
    let line = &text[start..end];
    let offset = match shown.strip_prefix("...") {
        Some(cut) => {
            let snippet = cut.strip_suffix("...").unwrap_or(cut);
            line.find(snippet).map(|base| base + column.saturating_sub(3))
        }
        None => Some(column),
    };
    let offset = offset.map(|o| start + o.min(line.len()));
    let offset = offset.map(|mut o| {
        while o > 0 && !text.is_char_boundary(o) {
            o -= 1;
        }
        o
    });
    (head, offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_is_placed_at_duckdb_s_caret() {
        let text = "select id,\n  nope from logs";
        let message = "Binder Error: Referenced column \"nope\" not found in FROM clause!\nCandidate bindings: \"id\"\n\nLINE 2:   nope from logs\n          ^";
        let (head, at) = place(message, text);
        assert_eq!(head, "Binder Error: Referenced column \"nope\" not found in FROM clause!\nCandidate bindings: \"id\"");
        assert_eq!(&text[at.unwrap()..at.unwrap() + 4], "nope");
        assert_eq!(place("Parser Error: syntax error at end of input", "select"), ("Parser Error: syntax error at end of input".to_string(), None));
        // A long line is shown cut to a window around the place, between `...`.
        let long = format!("select {} 1 from t where x = nope and y = 2", "a, ".repeat(100));
        let window = &long[long.len() - 40..long.len() - 10];
        let caret = " ".repeat("LINE 1: ...".len() + window.find("nope").unwrap());
        let message = format!("Binder Error: no\n\nLINE 1: ...{window}...\n{caret}^");
        let (_, at) = place(&message, &long);
        assert_eq!(&long[at.unwrap()..at.unwrap() + 4], "nope");
    }

    #[test]
    fn a_write_with_queries_feed_is_read_as_the_write() {
        use crate::catalog::{Table, TableColumn};
        let table = Table { database: "app".into(), schema: "main".into(), name: "teams".into(), current: true, current_database: true, columns: vec![TableColumn { name: "id".into(), not_null: true, comment: None }], keys: vec![vec!["id".into()]] };
        let catalog = catalog::of(vec![table]);
        let nullable = HashSet::new();
        let ctx = Ctx { catalog: &catalog, nullable_params: &nullable, macro_params: &[], macro_level: 0 };
        let shaped = |sql: &str| write_of(&parse(sql).unwrap()).into_owned();
        // The WITH queries move onto the INSERT's source, whose rows a key of `teams` does not bound.
        let insert = shaped("with teams as (select 1 as id union all select 1) insert into teams select * from teams where id = 1 returning id");
        let Statement::Insert(i) = &insert else { panic!("{insert:?}") };
        assert!(i.source.as_ref().unwrap().with.is_some());
        assert_eq!(cardinality(&insert, &ctx).0, "many");
        assert_eq!(outputs(&insert, &ctx).unwrap().iter().map(|o| o.not_null).collect::<Vec<_>>(), [true]);
        assert_eq!(tables(&parse("with x as (select 1 as id) insert into teams select id from x").unwrap()).write, ["teams"]);
        let update = shaped("with x as (select 1 as id) update teams set id = 2 from x where teams.id = 1 returning id");
        assert!(matches!(update, Statement::Update(_)));
        assert_eq!(cardinality(&update, &ctx).0, "many");
        let delete = shaped("with x as (select 1 as id) delete from teams where id = 1 returning id");
        assert_eq!(cardinality(&delete, &ctx).0, "maybeOne");
    }
}
