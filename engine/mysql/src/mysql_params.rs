//! Which MySQL parameters the caller may pass null for, by the rule of Postgres and SQLite
//! (`postgres::nullable_params`), read off the `sqlparser` tree, since MySQL's prepare says nothing
//! of a parameter's nullability: one cast to `nullable(…)`, and one that is, bare, with no cast,
//! the whole value written to a column that may hold null (a row of an INSERT's or a REPLACE's
//! VALUES, the SELECT that feeds one, its SET list, an UPDATE's SET, ON DUPLICATE KEY UPDATE),
//! unless it also fills a column that refuses null, by its NOT NULL or by an enforced CHECK that
//! tests it IS NOT NULL. Any other cast parameter is its cast's type, which the cast states whole.
//! Every other parameter is a non-null value, one the statement coalesces, tests for null or
//! compares with `<=>` included.

use crate::api::Scope;
use crate::fresh::Tables;
use crate::host::{as_str, Host, Lazy};
use crate::lex;
use serde_json::json;
use sqlparser::ast::{self, Expr, FunctionArg, FunctionArgExpr, Query, Select, SelectItem, SetExpr, Statement, TableFactor, TableWithJoins};
use std::collections::{HashMap, HashSet};

/// Each column of the named tables of the current schema, whether it may hold NULL, and its
/// `extra`, in the order an INSERT without a column list fills them, which passes over an
/// INVISIBLE column. A view's column is taken to refuse NULL: what a write through it takes is its
/// table's to say. `?` is the comma-joined table names, because MySQL binds no arrays.
const COLUMNS_QUERY: &str = "select c.table_name, c.column_name, if(t.table_type = 'VIEW', 'NO', c.is_nullable), c.extra from information_schema.columns c \
    join information_schema.tables t on t.table_schema = c.table_schema and t.table_name = c.table_name \
    where c.table_schema = database() and find_in_set(c.table_name, ?) order by c.table_name, c.ordinal_position";

/// The enforced CHECKs of the named tables of the current schema, as the server prints each
/// (MySQL 8.0.16 and later; a server without the views answers an error, and no CHECK refuses
/// null there).
const CHECKS_QUERY: &str = "select tc.table_name, cc.check_clause from information_schema.table_constraints tc \
    join information_schema.check_constraints cc on cc.constraint_schema = tc.constraint_schema and cc.constraint_name = tc.constraint_name \
    where tc.table_schema = database() and tc.constraint_type = 'CHECK' and tc.enforced = 'YES' and find_in_set(tc.table_name, ?)";

/// Each column's type of the named tables of the current schema, for [`typed`]: its
/// `data_type`, its `column_type` (`tinyint(1)`, `bit(1)`, `bigint unsigned`), its `extra` and
/// its comment, empty for none (a view's column has its table column's).
pub(crate) const TYPES_QUERY: &str = "select c.table_name, c.column_name, c.data_type, c.column_type, c.extra, c.column_comment from information_schema.columns c \
    where c.table_schema = database() and find_in_set(c.table_name, ?) order by c.table_name, c.ordinal_position";

/// The parameter each `?` of the rewritten text is, by where it stands as the parser counts:
/// lines and characters from 1.
pub(crate) type Names = HashMap<(u64, u64), String>;

pub(crate) fn names(rewritten: &lex::Rewritten) -> Names {
    let mut out = Names::new();
    for (i, &(_, _, start, _)) in rewritten.spans.iter().enumerate() {
        let before = &rewritten.sql[..start.min(rewritten.sql.len())];
        let line = before.matches('\n').count() as u64 + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) as u64 + 1;
        if let Some(name) = rewritten.order.get(i) {
            out.insert((line, column), name.clone());
        }
    }
    out
}

/// Asks for the columns of the tables the statement writes a parameter to or compares one with,
/// so that the answer comes in the round of the statement's own prepare.
pub(crate) fn prefetch(statement: &Statement, names: &Names, host: &Host) {
    let tables = written(statement, names);
    if !tables.is_empty() {
        let _ = host.query(COLUMNS_QUERY, vec![json!(tables.join(","))], Scope::Statement);
        let _ = host.query(CHECKS_QUERY, vec![json!(tables.join(","))], Scope::Statement);
    }
    let typed = typed_tables(statement, names);
    if !typed.is_empty() {
        let _ = host.query(TYPES_QUERY, vec![json!(typed.join(","))], Scope::Statement);
    }
}

/// The tables whose columns [`typed`] reads: those a parameter is written to or compared with,
/// and those whose columns the result columns read ([`traced_from`]).
fn typed_tables(statement: &Statement, names: &Names) -> Vec<String> {
    let mut tables = written_tables(&typed_targets(statement, names));
    tables.extend(traced_from(statement).unwrap_or_default());
    tables.sort();
    tables.dedup();
    tables
}

/// The tables of a SELECT whose result columns' origins in the prepare are the columns
/// themselves: one with no WITH and no set operation, over tables (or views) of the current schema
/// alone. Through a derived table or a WITH query MySQL gives the base table and the derived
/// column's own name as a column's origin (`select doc from (select other as doc from o) s` reads
/// `o.doc`), which may be another column of that table; an expression has no origin.
pub(crate) fn traced_from(statement: &Statement) -> Option<Vec<String>> {
    fn factors(factor: &TableFactor, out: &mut Vec<String>) -> Option<()> {
        match factor {
            TableFactor::Table { name, args: None, .. } => out.push(table_name(name)?),
            TableFactor::NestedJoin { table_with_joins, .. } => {
                factors(&table_with_joins.relation, out)?;
                for join in &table_with_joins.joins {
                    factors(&join.relation, out)?;
                }
            }
            _ => return None,
        }
        Some(())
    }
    let Statement::Query(q) = statement else { return None };
    if q.with.is_some() {
        return None;
    }
    let SetExpr::Select(select) = q.body.as_ref() else { return None };
    let mut tables = Vec::new();
    for from in &select.from {
        factors(&from.relation, &mut tables)?;
        for join in &from.joins {
            factors(&join.relation, &mut tables)?;
        }
    }
    Some(tables)
}

/// The parameters written whole to a column or compared whole with one, each with that column.
fn typed_targets(statement: &Statement, names: &Names) -> Vec<(String, String, Target)> {
    let mut targets = filled(statement, names, Fill::Typed);
    targets.extend(compared(statement, names));
    targets
}

/// What the columns the statement names say of its parameters and its result columns ([`typed`]).
#[derive(Debug, Default)]
pub(crate) struct Typed {
    /// Per parameter, the type its columns give it where it differs from the prepare's.
    pub types: HashMap<String, &'static str>,
    /// Per parameter, the comment of the columns it is written to or compared with, where they
    /// share one.
    pub comments: HashMap<String, String>,
    /// Per column of the tables the result columns read ([`traced_from`]), by table and column in
    /// lower case, its comment where it has one.
    pub columns: HashMap<(String, String), String>,
}

/// The type of each parameter whose column the statement names directly, where it differs from
/// what MySQL's prepare says: the server describes every integer context as a BIGINT and a BIT
/// context as BIT(64), whatever the column, so a parameter written to or compared with a BIGINT
/// column is `int8`, one with a TINYINT(1) or BIT(1) column, which a row returns as a truth
/// value, `bool|int4`, and one with a wider BIT column `bytea`. A row's value then goes back in
/// as a parameter of its type (the round-trip rule). A parameter whose
/// columns disagree keeps the prepare's type. And the comments of those columns and of the columns
/// the result columns read, for their doc comments.
pub(crate) fn typed(statement: &Statement, names: &Names, host: &Host) -> Lazy<Typed> {
    let targets = typed_targets(statement, names);
    let tables = typed_tables(statement, names);
    if tables.is_empty() {
        return Some(Typed::default());
    }
    let rows = host.query(TYPES_QUERY, vec![json!(tables.join(","))], Scope::Statement)?;
    let mut out = Typed::default();
    // Per table as the server names it: each column in lower case, its type, whether it is INVISIBLE, and its comment.
    let mut columns: HashMap<String, Vec<(String, Option<&'static str>, bool, Option<String>)>> = HashMap::new();
    for row in rows.unwrap_or_default() {
        let (Some(table), Some(column), Some(data_type), Some(column_type)) = (row.first().and_then(as_str), row.get(1).and_then(as_str), row.get(2).and_then(as_str), row.get(3).and_then(as_str)) else { continue };
        let invisible = row.get(4).and_then(as_str).is_some_and(|extra| extra.to_ascii_uppercase().contains("INVISIBLE"));
        let column_type = column_type.to_ascii_lowercase();
        let ty = match data_type.to_ascii_lowercase().as_str() {
            "bigint" => Some("int8"),
            "tinyint" if column_type.starts_with("tinyint(1)") => Some("bool|int4"),
            "bit" if column_type.starts_with("bit(1)") => Some("bool|int4"),
            "bit" => Some("bytea"),
            _ => None,
        };
        let comment = row.get(5).and_then(as_str).filter(|c| !c.is_empty()).map(String::from);
        if let Some(comment) = &comment {
            out.columns.insert((table.to_ascii_lowercase(), column.to_ascii_lowercase()), comment.clone());
        }
        columns.entry(table.to_string()).or_default().push((column.to_ascii_lowercase(), ty, invisible, comment));
    }
    let mut found: HashMap<String, Option<&'static str>> = HashMap::new();
    let mut noted: HashMap<String, Option<String>> = HashMap::new();
    for (param, table, target) in targets {
        let known = match columns.get(&table) {
            Some(known) => Some(known),
            None => {
                let mut same = columns.iter().filter(|(name, _)| name.eq_ignore_ascii_case(&table));
                match (same.next(), same.next()) {
                    (Some((_, known)), None) => Some(known),
                    _ => None,
                }
            }
        };
        let column = known.and_then(|known| match &target {
            Target::Name(name) => known.iter().find(|(c, _, _, _)| c == name),
            Target::Position(i) => known.iter().filter(|(_, _, invisible, _)| !invisible).nth(*i),
        });
        // A column nothing narrows, or one the statement names that the table lacks, keeps the prepare's type.
        let ty = column.and_then(|(_, ty, _, _)| *ty);
        let comment = column.and_then(|(_, _, _, comment)| comment.clone());
        found.entry(param.clone()).and_modify(|seen| if *seen != ty { *seen = None }).or_insert(ty);
        noted.entry(param).and_modify(|seen| if *seen != comment { *seen = None }).or_insert(comment);
    }
    out.types = found.into_iter().filter_map(|(param, ty)| ty.map(|ty| (param, ty))).collect();
    out.comments = noted.into_iter().filter_map(|(param, comment)| comment.map(|comment| (param, comment))).collect();
    Some(out)
}

/// Every parameter compared whole with a column of the table the statement reads, in its WHERE
/// or its HAVING (`col = :p`, `<>`, `<`, `<=`, `>`, `>=`, `<=>`, `col in (:a, :b)`, `col between
/// :a and :b`): (parameter, table as the statement names it, column). A bare column is the one
/// table's where the statement reads one; a qualified one is the table its name or alias names.
fn compared(statement: &Statement, names: &Names) -> Vec<(String, String, Target)> {
    let mut out = Vec::new();
    let (from, filters): (Vec<&TableWithJoins>, Vec<&Expr>) = match statement {
        Statement::Query(q) => match q.body.as_ref() {
            SetExpr::Select(s) => (s.from.iter().collect(), s.selection.iter().chain(s.having.iter()).collect()),
            _ => return out,
        },
        Statement::Update(update) => (std::iter::once(&update.table).collect(), update.selection.iter().collect()),
        Statement::Delete(delete) => {
            let (ast::FromTable::WithFromKeyword(from) | ast::FromTable::WithoutKeyword(from)) = &delete.from;
            (from.iter().collect(), delete.selection.iter().collect())
        }
        _ => return out,
    };
    // The tables by the name the statement reads each by; `plain` where every item is a table.
    let mut tables: Vec<(String, String)> = Vec::new();
    let mut plain = true;
    for twj in &from {
        for factor in std::iter::once(&twj.relation).chain(twj.joins.iter().map(|j| &j.relation)) {
            match factor {
                TableFactor::Table { name, alias, .. } => match table_name(name) {
                    Some(table) => {
                        let by = alias.as_ref().map(|a| a.name.value.clone()).unwrap_or_else(|| table.clone());
                        tables.push((by.to_ascii_lowercase(), table));
                    }
                    None => plain = false,
                },
                _ => plain = false,
            }
        }
    }
    let column = |e: &Expr| -> Option<(String, String)> {
        match e {
            Expr::Identifier(i) if plain && tables.len() == 1 => Some((tables[0].1.clone(), i.value.to_ascii_lowercase())),
            Expr::CompoundIdentifier(parts) if parts.len() == 2 => {
                let by = parts[0].value.to_ascii_lowercase();
                tables.iter().find(|(b, _)| *b == by).map(|(_, t)| (t.clone(), parts[1].value.to_ascii_lowercase()))
            }
            _ => None,
        }
    };
    fn walk(e: &Expr, names: &Names, column: &dyn Fn(&Expr) -> Option<(String, String)>, out: &mut Vec<(String, String, Target)>) {
        use ast::BinaryOperator::*;
        let pair = |col: &Expr, p: &Expr, out: &mut Vec<(String, String, Target)>| {
            if let (Some((table, name)), Some(param)) = (column(col), direct(p, names)) {
                out.push((param, table, Target::Name(name)));
            }
        };
        match e {
            Expr::BinaryOp { left, op: Eq | NotEq | Lt | LtEq | Gt | GtEq | Spaceship, right } => {
                pair(left, right, out);
                pair(right, left, out);
            }
            Expr::BinaryOp { left, op: And | Or | Xor, right } => {
                walk(left, names, column, out);
                walk(right, names, column, out);
            }
            Expr::InList { expr, list, .. } => {
                for item in list {
                    pair(expr, item, out);
                }
            }
            Expr::Between { expr, low, high, .. } => {
                pair(expr, low, out);
                pair(expr, high, out);
            }
            Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } => walk(inner, names, column, out),
            _ => {}
        }
    }
    for filter in filters {
        walk(filter, names, &column, &mut out);
    }
    out
}

/// The parameters the caller may pass null for, by name, and each one cast to `nullable(…)` that is
/// written whole to a column refusing null, as `(parameter, table, column)`.
pub(crate) struct Nullable {
    pub params: HashSet<String>,
    pub refused: Vec<(String, String, String)>,
}

/// The parameters of `statement` the caller may pass null for: those `marked` with `nullable(…)`,
/// and the bare ones written whole to a column that may hold null.
pub(crate) fn nullable(statement: &Statement, names: &Names, host: &Host, marked: &[String]) -> Lazy<Nullable> {
    let mut out = Nullable { params: marked.iter().cloned().collect(), refused: Vec::new() };
    let filled_all = filled(statement, names, Fill::Written);
    let bare = filled(statement, names, Fill::Bare);
    let marked_writes: Vec<&(String, String, Target)> = filled_all.iter().filter(|(param, _, _)| marked.contains(param)).collect();
    if bare.is_empty() && marked_writes.is_empty() {
        return Some(out);
    }
    // Every table a parameter is written to whole, as `prefetch` asked.
    let tables = json!(written(statement, names).join(","));
    let (rows, checks) = (host.query(COLUMNS_QUERY, vec![tables.clone()], Scope::Statement), host.query(CHECKS_QUERY, vec![tables], Scope::Statement));
    let (rows, checks) = (rows?, checks?);
    // Per table as the server names it, the CHECKs.
    let mut checked: HashMap<String, Vec<String>> = HashMap::new();
    for row in checks.unwrap_or_default() {
        if let (Some(table), Some(clause)) = (row.first().and_then(as_str), row.get(1).and_then(as_str)) {
            checked.entry(table.to_string()).or_default().push(clause.to_string());
        }
    }
    // Per table as the server names it: each column in lower case, whether it may hold NULL, and
    // whether it is INVISIBLE.
    let mut columns: HashMap<String, Vec<(String, bool, bool)>> = HashMap::new();
    for row in rows.unwrap_or_default() {
        let (Some(table), Some(column), Some(nullable)) = (row.first().and_then(as_str), row.get(1).and_then(as_str), row.get(2).and_then(as_str)) else { continue };
        let invisible = row.get(3).and_then(as_str).is_some_and(|extra| extra.to_ascii_uppercase().contains("INVISIBLE"));
        let column = column.to_ascii_lowercase();
        let refused = checked.get(table).is_some_and(|clauses| clauses.iter().any(|clause| refuses_null(clause, &column)));
        columns.entry(table.to_string()).or_default().push((column, nullable.eq_ignore_ascii_case("YES") && !refused, invisible));
    }
    // The column a target names, and whether it may hold NULL.
    let column_of = |table: &String, target: &Target| {
        // The server's name for the table: the query compares names as the server does, so a
        // row under another case came back because the server holds the two for one table.
        let known = match columns.get(table) {
            Some(known) => known,
            None => {
                let mut same = columns.iter().filter(|(name, _)| name.eq_ignore_ascii_case(table));
                match (same.next(), same.next()) {
                    (Some((_, known)), None) => known,
                    _ => return None,
                }
            }
        };
        let (column, nullable, _) = match target {
            Target::Name(name) => known.iter().find(|(c, _, _)| c == name),
            Target::Position(i) => known.iter().filter(|(_, _, invisible)| !invisible).nth(*i),
        }?;
        Some((column.clone(), *nullable))
    };
    let mut fills: HashMap<String, bool> = HashMap::new();
    for (param, table, target) in bare {
        if let Some((_, nullable)) = column_of(&table, &target) {
            *fills.entry(param).or_insert(true) &= nullable;
        }
    }
    out.params.extend(fills.into_iter().filter(|(_, nullable)| *nullable).map(|(param, _)| param));
    for (param, table, target) in marked_writes {
        if let Some((column, false)) = column_of(table, target) {
            out.refused.push((param.clone(), table.clone(), column));
        }
    }
    Some(out)
}

/// Whether a CHECK, as the server prints it (`(`email` is not null)`), refuses null in `column`
/// (lower case): one of its conjuncts tests the column IS NOT NULL, which a null makes false where
/// any other test of it would be unknown, and so pass.
fn refuses_null(clause: &str, column: &str) -> bool {
    use sqlparser::dialect::MySqlDialect;
    use sqlparser::parser::Parser;
    fn conjunct(e: &Expr, column: &str) -> bool {
        match e {
            Expr::Nested(inner) => conjunct(inner, column),
            Expr::BinaryOp { left, op: ast::BinaryOperator::And, right } => conjunct(left, column) || conjunct(right, column),
            Expr::IsNotNull(inner) => match inner.as_ref() {
                Expr::Identifier(i) => i.value.eq_ignore_ascii_case(column),
                Expr::Nested(n) => matches!(n.as_ref(), Expr::Identifier(i) if i.value.eq_ignore_ascii_case(column)),
                _ => false,
            },
            _ => false,
        }
    }
    let dialect = MySqlDialect {};
    Parser::new(&dialect).try_with_sql(clause).and_then(|mut p| p.parse_expr()).is_ok_and(|e| conjunct(&e, column))
}

/// The tables of the current schema the statement reads and writes, as it names them, for the
/// freshness facts ([`crate::fresh`]): every table a FROM item reads, in subqueries and WITH
/// queries too, and the tables an INSERT, an UPDATE or a DELETE writes: a multi-table UPDATE's or
/// DELETE's every table, any of which it may write. A WITH query is no table, and a table of another
/// schema is left out, since the facts are read from the current one; a write that names one, or
/// that writes through something that is no table, lists nothing it writes.
pub(crate) fn tables(statement: &Statement) -> Tables {
    let mut walk = Walk { tables: Vec::new() };
    walk.statement(statement, &Env::default());
    let target = |relation: &TableFactor| match relation {
        TableFactor::Table { name, .. } => table_name(name),
        _ => None,
    };
    let write = match statement {
        Statement::Insert(insert) => match &insert.table {
            ast::TableObject::TableName(name) => table_name(name).into_iter().collect(),
            _ => Vec::new(),
        },
        Statement::Update(update) if update.table.joins.is_empty() && update.from.is_none() => target(&update.table.relation).into_iter().collect(),
        Statement::Update(update) => {
            let from = match &update.from {
                Some(ast::UpdateTableFromKind::BeforeSet(from) | ast::UpdateTableFromKind::AfterSet(from)) => from.as_slice(),
                None => &[],
            };
            every_table(std::iter::once(&update.table).chain(from)).unwrap_or_default()
        }
        Statement::Delete(delete) => {
            let (ast::FromTable::WithFromKeyword(from) | ast::FromTable::WithoutKeyword(from)) = &delete.from;
            match from.as_slice() {
                [only] if delete.tables.is_empty() && only.joins.is_empty() && delete.using.is_none() => target(&only.relation).into_iter().collect(),
                _ => every_table(from.iter().chain(delete.using.iter().flatten())).unwrap_or_default(),
            }
        }
        _ => Vec::new(),
    };
    Tables { read: walk.tables, write }
}

/// Every table of a multi-table UPDATE's or DELETE's tables, or `None` where one is a table of
/// another schema or something that is no table, which the write might reach through.
fn every_table<'a>(from: impl Iterator<Item = &'a ast::TableWithJoins>) -> Option<Vec<String>> {
    fn factor(f: &TableFactor, out: &mut Vec<String>) -> Option<()> {
        match f {
            TableFactor::Table { name, .. } => out.push(table_name(name)?),
            TableFactor::NestedJoin { table_with_joins, .. } => joined(table_with_joins, out)?,
            _ => return None,
        }
        Some(())
    }
    fn joined(t: &ast::TableWithJoins, out: &mut Vec<String>) -> Option<()> {
        factor(&t.relation, out)?;
        t.joins.iter().try_for_each(|j| factor(&j.relation, out))
    }
    let mut out = Vec::new();
    for t in from {
        joined(t, &mut out)?;
    }
    out.sort();
    out.dedup();
    Some(out)
}

/// The column a parameter is written to whole, by name or by its place in the table.
#[derive(Debug, PartialEq)]
enum Target {
    Name(String),
    Position(usize),
}

/// Every table `statement` writes a parameter to whole, bare or cast, the SELECT that feeds an
/// INSERT included: the tables whose columns [`nullable`] reads.
fn written(statement: &Statement, names: &Names) -> Vec<String> {
    let mut tables = written_tables(&filled(statement, names, Fill::Written));
    tables.sort();
    tables.dedup();
    tables
}

fn written_tables(filled: &[(String, String, Target)]) -> Vec<String> {
    let mut tables: Vec<String> = filled.iter().map(|(_, table, _)| table.clone()).collect();
    tables.sort();
    tables.dedup();
    tables
}

/// The table name a statement writes to, when it is one of the current schema.
fn table_name(name: &ast::ObjectName) -> Option<String> {
    match name.0.as_slice() {
        [ast::ObjectNamePart::Identifier(i)] => Some(i.value.clone()),
        _ => None,
    }
}

/// The parameter an expression is, whole: under casts, parentheses and the `JSON_COMPACT()` a
/// `::json` cast compiles to on MariaDB, which leave a NULL as it is.
fn direct(e: &Expr, names: &Names) -> Option<String> {
    match e {
        Expr::Value(v) => match &v.value {
            ast::Value::Placeholder(p) if p == "?" => names.get(&(v.span.start.line, v.span.start.column)).cloned(),
            _ => None,
        },
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } | Expr::Convert { expr: inner, .. } => direct(inner, names),
        _ => json_compact(e).and_then(|inner| direct(inner, names)),
    }
}

/// The argument of `JSON_COMPACT(x)`, which MariaDB writes for a `::json` cast (it has no
/// `CAST(… AS JSON)`), and which is NULL for NULL.
fn json_compact(e: &Expr) -> Option<&Expr> {
    let Expr::Function(f) = e else { return None };
    let ast::FunctionArguments::List(list) = &f.args else { return None };
    match list.args.as_slice() {
        [only] if crate::mysql::builtin(f) == "json_compact" && list.clauses.is_empty() && f.over.is_none() => arg_expr(only),
        _ => None,
    }
}

/// The parameter an expression is, bare: under parentheses alone, no cast.
fn bare(e: &Expr, names: &Names) -> Option<String> {
    match e {
        Expr::Value(v) => match &v.value {
            ast::Value::Placeholder(p) if p == "?" => names.get(&(v.span.start.line, v.span.start.column)).cloned(),
            _ => None,
        },
        Expr::Nested(inner) => bare(inner, names),
        _ => None,
    }
}

/// Which parameters written whole to a column [`filled`] lists.
#[derive(Clone, Copy, PartialEq)]
enum Fill {
    /// Bare or under the casts, the SELECT that feeds an INSERT aside: those that take the
    /// column's type.
    Typed,
    /// Only one with no cast, the columns of the one SELECT that feeds an INSERT included: those
    /// that take the column's nullability.
    Bare,
    /// Bare or under the casts, the columns of the one SELECT that feeds an INSERT included: every
    /// write a parameter cast to `nullable(…)` is checked against.
    Written,
}

/// Every parameter written whole to a column, as `fill` picks them: (parameter, table as the
/// statement names it, column).
fn filled(statement: &Statement, names: &Names, fill: Fill) -> Vec<(String, String, Target)> {
    let direct = |e: &Expr, names: &Names| if fill == Fill::Bare { bare(e, names) } else { direct(e, names) };
    let mut out = Vec::new();
    let column = |target: &ast::AssignmentTarget| match target {
        ast::AssignmentTarget::ColumnName(name) => name.0.last().and_then(|p| p.as_ident()).map(|i| (name.0.len(), i.value.to_ascii_lowercase())),
        ast::AssignmentTarget::Tuple(_) => None,
    };
    match statement {
        Statement::Insert(insert) => {
            let ast::TableObject::TableName(name) = &insert.table else { return out };
            let Some(table) = table_name(name) else { return out };
            let columns: Vec<String> = insert.columns.iter().filter_map(|c| c.0.last().and_then(|p| p.as_ident()).map(|i| i.value.to_ascii_lowercase())).collect();
            if columns.len() != insert.columns.len() {
                return out;
            }
            let target_at = |i: usize| if columns.is_empty() { Some(Target::Position(i)) } else { columns.get(i).map(|c| Target::Name(c.clone())) };
            match insert.source.as_ref().filter(|q| q.with.is_none()).map(|q| q.body.as_ref()) {
                Some(SetExpr::Values(values)) => {
                    for row in &values.rows {
                        for (i, value) in row.content.iter().enumerate() {
                            let Some(param) = direct(value, names) else { continue };
                            if let Some(target) = target_at(i) {
                                out.push((param, table.clone(), target));
                            }
                        }
                    }
                }
                // The SELECT that feeds the INSERT writes its columns whole, as a VALUES row does.
                Some(SetExpr::Select(select)) if fill != Fill::Typed => {
                    for (i, item) in select.projection.iter().enumerate() {
                        let (SelectItem::UnnamedExpr(value) | SelectItem::ExprWithAlias { expr: value, .. }) = item else { break };
                        let Some(param) = direct(value, names) else { continue };
                        if let Some(target) = target_at(i) {
                            out.push((param, table.clone(), target));
                        }
                    }
                }
                _ => {}
            }
            let update = match &insert.on {
                Some(ast::OnInsert::DuplicateKeyUpdate(set)) => set.as_slice(),
                _ => &[],
            };
            for a in insert.assignments.iter().chain(update) {
                if let (Some(param), Some((_, name))) = (direct(&a.value, names), column(&a.target)) {
                    out.push((param, table.clone(), Target::Name(name)));
                }
            }
        }
        Statement::Update(update) => {
            // The tables the statement names, by the name it uses for each.
            let mut tables: Vec<(String, String)> = Vec::new();
            let mut plain = update.from.is_none() && update.table.joins.is_empty();
            for factor in std::iter::once(&update.table.relation).chain(update.table.joins.iter().map(|j| &j.relation)) {
                match factor {
                    TableFactor::Table { name, alias, .. } => match table_name(name) {
                        Some(table) => {
                            let by = alias.as_ref().map(|a| a.name.value.clone()).unwrap_or_else(|| table.clone());
                            tables.push((by.to_ascii_lowercase(), table));
                        }
                        None => plain = false,
                    },
                    _ => plain = false,
                }
            }
            for a in &update.assignments {
                let (Some(param), Some((parts, name))) = (direct(&a.value, names), column(&a.target)) else { continue };
                let table = match (&a.target, parts) {
                    // A bare column is the one table's, where there is one.
                    (_, 1) if plain => tables.first().map(|(_, t)| t.clone()),
                    (ast::AssignmentTarget::ColumnName(n), 2) => {
                        let by = n.0.first().and_then(|p| p.as_ident()).map(|i| i.value.to_ascii_lowercase());
                        tables.iter().find(|(b, _)| Some(b) == by.as_ref()).map(|(_, t)| t.clone())
                    }
                    _ => None,
                };
                if let Some(table) = table {
                    out.push((param, table, Target::Name(name)));
                }
            }
        }
        _ => {}
    }
    out
}

/// What a place in the statement sees: the WITH queries in scope, by name.
#[derive(Clone, Default)]
struct Env {
    ctes: Vec<String>,
}

/// A walk of the statement for the tables of the current schema its FROM items read.
struct Walk {
    /// Each table of the current schema a FROM item reads, as the statement names it.
    tables: Vec<String>,
}

impl Walk {
    fn statement(&mut self, statement: &Statement, env: &Env) {
        match statement {
            Statement::Query(q) => self.query(q, env),
            Statement::Insert(insert) => {
                if let Some(source) = &insert.source {
                    self.query(source, env);
                }
                let update = match &insert.on {
                    Some(ast::OnInsert::DuplicateKeyUpdate(set)) => set.as_slice(),
                    _ => &[],
                };
                for a in insert.assignments.iter().chain(update) {
                    self.expr(&a.value, env);
                }
            }
            Statement::Update(update) => {
                let mut from = vec![update.table.clone()];
                if let Some(ast::UpdateTableFromKind::BeforeSet(more) | ast::UpdateTableFromKind::AfterSet(more)) = &update.from {
                    from.extend(more.iter().cloned());
                }
                self.level(&from, env);
                for a in &update.assignments {
                    self.expr(&a.value, env);
                }
                for e in update.selection.iter().chain(update.order_by.iter().map(|o| &o.expr)).chain(update.limit.iter()) {
                    self.expr(e, env);
                }
            }
            Statement::Delete(delete) => {
                let (ast::FromTable::WithFromKeyword(from) | ast::FromTable::WithoutKeyword(from)) = &delete.from;
                let mut from = from.clone();
                from.extend(delete.using.iter().flatten().cloned());
                self.level(&from, env);
                for e in delete.selection.iter().chain(delete.order_by.iter().map(|o| &o.expr)).chain(delete.limit.iter()) {
                    self.expr(e, env);
                }
            }
            _ => {}
        }
    }

    fn query(&mut self, q: &Query, env: &Env) {
        let mut env = env.clone();
        for cte in q.with.iter().flat_map(|w| &w.cte_tables) {
            self.query(&cte.query, &env);
            env.ctes.push(cte.alias.name.value.to_ascii_lowercase());
        }
        self.body(&q.body, &env);
        for o in q.order_by.iter().flat_map(|o| match &o.kind {
            ast::OrderByKind::Expressions(list) => list.as_slice(),
            _ => &[],
        }) {
            self.expr(&o.expr, &env);
        }
    }

    fn body(&mut self, body: &SetExpr, env: &Env) {
        match body {
            SetExpr::Select(s) => self.select(s, env),
            SetExpr::Query(q) => self.query(q, env),
            SetExpr::SetOperation { left, right, .. } => {
                self.body(left, env);
                self.body(right, env);
            }
            SetExpr::Values(values) => {
                for e in values.rows.iter().flat_map(|row| &row.content) {
                    self.expr(e, env);
                }
            }
            SetExpr::Insert(s) | SetExpr::Update(s) | SetExpr::Delete(s) | SetExpr::Merge(s) => self.statement(s, env),
            _ => {}
        }
    }

    fn select(&mut self, s: &Select, env: &Env) {
        self.level(&s.from, env);
        for item in &s.projection {
            if let SelectItem::ExprWithAlias { expr, .. } | SelectItem::UnnamedExpr(expr) = item {
                self.expr(expr, env);
            }
        }
        let group_by = match &s.group_by {
            ast::GroupByExpr::Expressions(list, _) => list.as_slice(),
            _ => &[],
        };
        for e in s.selection.iter().chain(group_by).chain(s.having.iter()).chain(s.qualify.iter()) {
            self.expr(e, env);
        }
    }

    /// The FROM items of a SELECT: the tables they read, and the subqueries and the ON conditions
    /// in them walked.
    fn level(&mut self, from: &[TableWithJoins], env: &Env) {
        let mut on: Vec<&Expr> = Vec::new();
        for twj in from {
            self.items(twj, env, &mut on);
        }
        for e in on {
            self.expr(e, env);
        }
    }

    fn items<'t>(&mut self, twj: &'t TableWithJoins, env: &Env, on: &mut Vec<&'t Expr>) {
        self.factor(&twj.relation, env, on);
        for join in &twj.joins {
            self.factor(&join.relation, env, on);
            use ast::JoinOperator as J;
            let constraint = match &join.join_operator {
                J::Join(c) | J::Inner(c) | J::Left(c) | J::LeftOuter(c) | J::Right(c) | J::RightOuter(c) | J::FullOuter(c) | J::CrossJoin(c) | J::Semi(c) | J::LeftSemi(c) | J::RightSemi(c) | J::Anti(c) | J::LeftAnti(c) | J::RightAnti(c) | J::StraightJoin(c) => Some(c),
                J::AsOf { constraint, .. } => Some(constraint),
                _ => None,
            };
            if let Some(ast::JoinConstraint::On(e)) = constraint {
                on.push(e);
            }
        }
    }

    fn factor<'t>(&mut self, factor: &'t TableFactor, env: &Env, on: &mut Vec<&'t Expr>) {
        match factor {
            TableFactor::Table { name, .. } => {
                // A WITH query in scope is what a bare name in FROM reads, whatever table shares it.
                let table = table_name(name).map(|t| t.to_ascii_lowercase());
                if !table.is_some_and(|t| env.ctes.contains(&t)) {
                    self.tables.extend(table_name(name));
                }
            }
            TableFactor::Derived { subquery, .. } => self.query(subquery, env),
            TableFactor::NestedJoin { table_with_joins, .. } => self.items(table_with_joins, env, on),
            TableFactor::JsonTable { json_expr, .. } => self.expr(json_expr, env),
            TableFactor::TableFunction { expr, .. } => self.expr(expr, env),
            _ => {}
        }
    }

    /// Walks an expression, for the subqueries in it.
    fn expr(&mut self, e: &Expr, env: &Env) {
        let mut children: Vec<&Expr> = Vec::new();
        let mut queries: Vec<&Query> = Vec::new();
        match e {
            Expr::IsFalse(x) | Expr::IsNotFalse(x) | Expr::IsTrue(x) | Expr::IsNotTrue(x) | Expr::IsNull(x) | Expr::IsNotNull(x) | Expr::IsUnknown(x) | Expr::IsNotUnknown(x) | Expr::Nested(x) => children.push(x),
            Expr::IsDistinctFrom(a, b) | Expr::IsNotDistinctFrom(a, b) => children.extend([a.as_ref(), b.as_ref()]),
            Expr::BinaryOp { left, right, .. } | Expr::AnyOp { left, right, .. } | Expr::AllOp { left, right, .. } => children.extend([left.as_ref(), right.as_ref()]),
            Expr::UnaryOp { expr, .. } | Expr::Cast { expr, .. } | Expr::Collate { expr, .. } | Expr::Extract { expr, .. } | Expr::Ceil { expr, .. } | Expr::Floor { expr, .. } => children.push(expr),
            Expr::Convert { expr, styles, .. } => {
                children.push(expr);
                children.extend(styles);
            }
            Expr::InList { expr, list, .. } => {
                children.push(expr);
                children.extend(list);
            }
            Expr::InSubquery { expr, subquery, .. } => {
                children.push(expr);
                queries.push(subquery);
            }
            Expr::Between { expr, low, high, .. } => children.extend([expr.as_ref(), low.as_ref(), high.as_ref()]),
            Expr::Like { expr, pattern, escape_char, .. } | Expr::ILike { expr, pattern, escape_char, .. } | Expr::SimilarTo { expr, pattern, escape_char, .. } => {
                children.extend([expr.as_ref(), pattern.as_ref()]);
                children.extend(escape_char.as_deref());
            }
            Expr::RLike { expr, pattern, .. } => children.extend([expr.as_ref(), pattern.as_ref()]),
            Expr::AtTimeZone { timestamp, time_zone } => children.extend([timestamp.as_ref(), time_zone.as_ref()]),
            Expr::Position { expr, r#in } => children.extend([expr.as_ref(), r#in.as_ref()]),
            Expr::Substring { expr, substring_from, substring_for, .. } => {
                children.push(expr);
                children.extend(substring_from.as_deref());
                children.extend(substring_for.as_deref());
            }
            Expr::Trim { expr, trim_what, trim_characters, .. } => {
                children.push(expr);
                children.extend(trim_what.as_deref());
                children.extend(trim_characters.iter().flatten());
            }
            Expr::Overlay { expr, overlay_what, overlay_from, overlay_for } => {
                children.extend([expr.as_ref(), overlay_what.as_ref(), overlay_from.as_ref()]);
                children.extend(overlay_for.as_deref());
            }
            Expr::Function(f) => {
                for args in [&f.parameters, &f.args] {
                    match args {
                        ast::FunctionArguments::List(list) => children.extend(list.args.iter().filter_map(arg_expr)),
                        ast::FunctionArguments::Subquery(q) => queries.push(q),
                        ast::FunctionArguments::None => {}
                    }
                }
                children.extend(f.filter.as_deref());
                children.extend(f.within_group.iter().map(|o| &o.expr));
            }
            Expr::Case { operand, conditions, else_result, .. } => {
                children.extend(operand.as_deref());
                for when in conditions {
                    children.extend([&when.condition, &when.result]);
                }
                children.extend(else_result.as_deref());
            }
            Expr::Exists { subquery, .. } => queries.push(subquery),
            Expr::Subquery(q) => queries.push(q),
            Expr::Tuple(list) => children.extend(list),
            Expr::Interval(i) => children.push(&i.value),
            Expr::JsonAccess { value, .. } => children.push(value),
            Expr::CompoundFieldAccess { root, .. } => children.push(root),
            Expr::MemberOf(m) => children.extend([m.value.as_ref(), m.array.as_ref()]),
            Expr::Prefixed { value, .. } => children.push(value),
            _ => {}
        }
        for child in children {
            self.expr(child, env);
        }
        for q in queries {
            self.query(q, env);
        }
    }
}

pub(crate) fn arg_expr(a: &FunctionArg) -> Option<&Expr> {
    match a {
        FunctionArg::Unnamed(FunctionArgExpr::Expr(x)) | FunctionArg::Named { arg: FunctionArgExpr::Expr(x), .. } | FunctionArg::ExprNamed { arg: FunctionArgExpr::Expr(x), .. } => Some(x),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{Answer, Need};
    use crate::host::Host;
    use serde_json::json;
    use std::collections::HashMap;

    /// The analysis of `sql` over a schema: `users (id NOT NULL, name NOT NULL, secret NOT NULL
    /// INVISIBLE, email, bio)`, `posts (id NOT NULL, user_id NOT NULL, title NOT NULL, tag)` and
    /// `accounts (id NOT NULL, email, nick, note)` with `check (email <> '' and email is not null)`,
    /// `check (nick is not null)` and `check (note <> '')`; each parameter with whether it may be
    /// null, and whether the columns were asked for.
    fn nullable(sql: &str) -> (Vec<(String, bool)>, bool) {
        let (analysis, asked) = analyzed(sql);
        assert!(analysis.diagnostics.is_empty(), "{sql}: {:?}", analysis.diagnostics);
        (analysis.params.iter().map(|p| (p.name.clone(), p.nullable)).collect(), asked)
    }

    /// The analysis of `sql` over a scripted catalog, and whether it asked for the columns.
    fn analyzed(sql: &str) -> (crate::api::Analysis, bool) {
        let columns = json!({ "rows": [
            ["accounts", "id", "NO", ""], ["accounts", "email", "YES", ""], ["accounts", "nick", "YES", ""], ["accounts", "note", "YES", ""],
            ["posts", "id", "NO", "auto_increment"], ["posts", "user_id", "NO", ""], ["posts", "title", "NO", ""], ["posts", "tag", "YES", ""],
            ["users", "id", "NO", "auto_increment"], ["users", "name", "NO", ""], ["users", "secret", "NO", "INVISIBLE"], ["users", "email", "YES", ""], ["users", "bio", "YES", ""],
        ] });
        let checks = json!({ "rows": [
            ["accounts", "((`email` <> _utf8mb4'') and (`email` is not null))"], ["accounts", "(`nick` is not null)"], ["accounts", "(`note` <> _utf8mb4'')"],
        ] });
        let mut answers: HashMap<String, Answer> = HashMap::new();
        let mut asked = false;
        for _ in 0..8 {
            let host = Host::new(&answers);
            if let Some(analysis) = crate::mysql::analyze_text(sql, &host) {
                return (analysis, asked);
            }
            for need in host.needs() {
                let (id, answer) = match need {
                    Need::Describe { id, sql: text } => {
                        let params: Vec<_> = text.matches('?').map(|_| json!({ "type": 253, "flags": 0, "charset": 255 })).collect();
                        (id, json!({ "mysql": { "columns": [], "params": params } }))
                    }
                    Need::Query { id, sql, .. } if sql == super::COLUMNS_QUERY => {
                        asked = true;
                        (id, columns.clone())
                    }
                    Need::Query { id, sql, .. } if sql == super::CHECKS_QUERY => (id, checks.clone()),
                    Need::Query { id, .. } => (id, json!({ "rows": [] })),
                };
                answers.insert(id, serde_json::from_value(answer).unwrap());
            }
        }
        panic!("the analysis never settled")
    }

    /// Each parameter's type, the prepare describing every `?` with `code` as MySQL 8.4 does for a context (8 an integer one, 16 a BIT one).
    fn param_types(sql: &str, code: u32) -> Vec<(String, String)> {
        let types = json!({ "rows": [
            ["t", "id", "int", "int", ""], ["t", "big", "bigint", "bigint unsigned", ""], ["t", "flag", "tinyint", "tinyint(1)", ""], ["t", "small", "tinyint", "tinyint", ""],
            ["t", "bit1", "bit", "bit(1)", ""], ["t", "bits", "bit", "bit(8)", ""], ["t", "hidden", "bigint", "bigint", "INVISIBLE"],
            ["u", "id", "bigint", "bigint", ""], ["u", "big", "int", "int", ""],
        ] });
        let mut answers: HashMap<String, Answer> = HashMap::new();
        for _ in 0..8 {
            let host = Host::new(&answers);
            if let Some(analysis) = crate::mysql::analyze_text(sql, &host) {
                assert!(analysis.diagnostics.is_empty(), "{sql}: {:?}", analysis.diagnostics);
                return analysis.params.iter().map(|p| (p.name.clone(), match &p.ty { crate::types::Type::Scalar { name } => name.clone(), other => format!("{other:?}") })).collect();
            }
            for need in host.needs() {
                let (id, answer) = match need {
                    Need::Describe { id, sql: text } => {
                        let params: Vec<_> = text.matches('?').map(|_| json!({ "type": code, "flags": 0, "charset": 63 })).collect();
                        (id, json!({ "mysql": { "columns": [], "params": params } }))
                    }
                    Need::Query { id, sql, .. } if sql == super::TYPES_QUERY => (id, types.clone()),
                    // The server's default mode, under which the engine reads `NOT` as the server does.
                    Need::Query { id, sql, .. } if sql == "select @@session.sql_mode" => (id, json!({ "rows": [["ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION"]] })),
                    Need::Query { id, .. } => (id, json!({ "rows": [] })),
                };
                answers.insert(id, serde_json::from_value(answer).unwrap());
            }
        }
        panic!("the analysis never settled")
    }

    #[test]
    fn a_parameter_written_to_or_compared_with_a_column_takes_the_values_its_rows_return() {
        let p = |name: &str, ty: &str| (name.to_string(), ty.to_string());
        // A BIGINT column's rows are BigInts, a TINYINT(1)'s and a BIT(1)'s truth values, a wider BIT's bytes.
        assert_eq!(param_types("select id from t where big = :b and flag = :f and id = :i and small = :s", 8), [p("b", "int8"), p("f", "bool|int4"), p("i", "int4"), p("s", "int4")]);
        assert_eq!(param_types("select id from t where bit1 = :one and bits = :eight", 16), [p("one", "bool|int4"), p("eight", "bytea")]);
        // Either side of a comparison, IN, BETWEEN, under NOT and OR, and a qualified column of a join.
        assert_eq!(param_types("select t.id from t join u on u.id = t.id where :b < t.big or not (u.id in (:x, :y)) and t.big between :lo and :hi", 8), [p("b", "int8"), p("x", "int8"), p("y", "int8"), p("lo", "int8"), p("hi", "int8")]);
        // A bare column of a join is ambiguous, and a column whose two uses disagree keeps the prepare's type.
        assert_eq!(param_types("select t.id from t join u on u.id = t.id where big = :b", 8), [p("b", "int4")]);
        assert_eq!(param_types("select t.id from t, u where t.big = :b and u.big = :b2 and t.id = :c and u.id = :c2", 8), [p("b", "int8"), p("b2", "int4"), p("c", "int4"), p("c2", "int8")]);
        // Written whole: VALUES by name and by place (the INVISIBLE column is no place), and an UPDATE's SET and WHERE.
        assert_eq!(param_types("insert into t (big, flag) values (:b, :f)", 8), [p("b", "int8"), p("f", "bool|int4")]);
        assert_eq!(param_types("insert into t values (:i, :b, :f, :s, :one, :eight)", 8), [p("i", "int4"), p("b", "int8"), p("f", "bool|int4"), p("s", "int4"), p("one", "bool|int4"), p("eight", "bytea")]);
        assert_eq!(param_types("update t set flag = :f where big = :b", 8), [p("f", "bool|int4"), p("b", "int8")]);
        assert_eq!(param_types("delete from t where big = :b", 8), [p("b", "int8")]);
        // An expression of the column is not the column: `big + 1` is the prepare's.
        assert_eq!(param_types("select id from t where big + 1 = :b limit :n", 8), [p("b", "int4"), p("n", "int4")]);
    }

    /// The tables `sql` reads and writes, as the freshness facts name them.
    fn tables(sql: &str) -> (Vec<String>, Vec<String>) {
        use sqlparser::dialect::MySqlDialect;
        use sqlparser::parser::Parser;
        let statement = Parser::parse_sql(&MySqlDialect {}, sql).unwrap().pop().unwrap();
        let mut t = super::tables(&statement);
        t.read.sort();
        t.read.dedup();
        (t.read, t.write)
    }

    #[test]
    fn the_facts_are_read_for_every_table_the_statement_names_and_no_with_query() {
        let names = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(tables("select u.id, (select count(*) from posts p where p.user_id = u.id) from `users` u where exists (select 1 from tags t where t.id = u.id)"), (names(&["posts", "tags", "users"]), vec![]));
        // A WITH query is no table, even one named like a table; a derived table's are read.
        assert_eq!(tables("with users as (select id from accounts) select id from users join (select user_id from posts) p on p.user_id = users.id"), (names(&["accounts", "posts"]), vec![]));
        // What an INSERT, an UPDATE and a DELETE write, and the tables they read.
        assert_eq!(tables("insert into audit (user_id) select id from users"), (names(&["users"]), names(&["audit"])));
        assert_eq!(tables("update users set name = (select max(title) from posts) where id = 1"), (names(&["posts", "users"]), names(&["users"])));
        assert_eq!(tables("delete from users where id in (select user_id from bans)"), (names(&["bans", "users"]), names(&["users"])));
        // A table of another schema is left out: the facts are read from the current one.
        assert_eq!(tables("select id from other.users"), (vec![], vec![]));
        // A multi-table UPDATE or DELETE may write any of its tables; one that names a table of
        // another schema, or joins a derived table, lists nothing it writes.
        assert_eq!(tables("delete u from users u join bans b on b.user_id = u.id").1, names(&["bans", "users"]));
        assert_eq!(tables("delete from users using users join bans on bans.user_id = users.id").1, names(&["bans", "users"]));
        assert_eq!(tables("update users u join posts p on p.user_id = u.id set p.title = u.name").1, names(&["posts", "users"]));
        assert_eq!(tables("update users u join (select 1 as id) d on d.id = u.id set u.name = 'x'").1, Vec::<String>::new());
        assert_eq!(tables("delete u from users u join other.bans b on b.user_id = u.id").1, Vec::<String>::new());
        assert_eq!(tables("insert into other.audit (user_id) values (1)").1, Vec::<String>::new());
    }

    fn may_be_null(sql: &str) -> Vec<String> {
        nullable(sql).0.into_iter().filter(|(_, n)| *n).map(|(name, _)| name).collect()
    }

    #[test]
    fn a_bare_parameter_written_whole_to_a_nullable_column_may_be_null() {
        assert_eq!(nullable("insert into users (name, email) values (:name, :email)"), (vec![("name".into(), false), ("email".into(), true)], true));
        assert_eq!(may_be_null("update users set email = :email, name = :name where id = :id"), ["email"]);
        assert_eq!(may_be_null("update users u set u.bio = :bio where u.id = :id"), ["bio"]);
        // Every row of a VALUES list; a REPLACE as an INSERT; parentheses leave it bare.
        assert_eq!(may_be_null("insert into users (name, email) values (:n1, :e1), (:n2, :e2)"), ["e1", "e2"]);
        assert_eq!(may_be_null("replace into users (name, email, bio) values (:name, :email, (:bio))"), ["email", "bio"]);
        // Without a column list, by position, past the INVISIBLE column, which takes no value.
        assert_eq!(may_be_null("insert into users values (:id, :name, :email, :bio)"), ["email", "bio"]);
        // The SET form, ON DUPLICATE KEY UPDATE, and the SELECT that feeds an INSERT.
        assert_eq!(may_be_null("insert into users set name = :name, email = :email"), ["email"]);
        assert_eq!(may_be_null("insert into users (name, email) values (:name, :email) on duplicate key update bio = :bio, name = :again"), ["email", "bio"]);
        assert_eq!(may_be_null("insert into users (name, email) select :name, :email"), ["email"]);
        assert_eq!(may_be_null("insert into users (name, email, bio) select name, :email, :bio from users"), ["email", "bio"]);
        // A table of a join, by the name the statement gives it.
        assert_eq!(may_be_null("update users u join posts p on p.user_id = u.id set u.email = :email, p.title = :title, p.tag = :tag where p.id = :id"), ["email", "tag"]);
        // A column a CHECK refuses null in: a conjunct tests it IS NOT NULL. One a null passes is nullable.
        assert_eq!(may_be_null("insert into accounts (id, email, nick, note) values (:id, :email, :nick, :note)"), ["note"]);
        assert_eq!(may_be_null("update accounts set email = :email, nick = :nick, note = :note where id = :id"), ["note"]);
        // Not the whole value: the expression it is part of decides what is written.
        assert_eq!(nullable("update users set email = lower(:email) where id = :id"), (vec![("email".into(), false), ("id".into(), false)], false));
        // A read asks for no columns.
        assert_eq!(nullable("select id from users where email = :email"), (vec![("email".into(), false)], false));
    }

    #[test]
    fn a_cast_parameter_is_its_casts_type_and_takes_no_null() {
        // What a `:name::type` cast compiles to: `CAST(? AS SIGNED)`, `CAST(? AS DECIMAL(…))`,
        // `CAST(? AS JSON)`, and on MariaDB `JSON_COMPACT(?)` for `::json`: written whole to a
        // nullable column, the parameter is a value: a cast parameter is its cast's type.
        for sql in [
            "insert into users (name, email, bio) values (:name, CAST(:email AS JSON), JSON_COMPACT(:bio))",
            "insert into users (name, email, bio) values (CAST(:name AS SIGNED), CAST(:email AS DECIMAL(10,2)), CAST(:bio AS UNSIGNED)), (JSON_COMPACT(:n2), cast(:e2 as char), CAST(:b2 AS DATETIME(6)))",
            "update users set email = CAST(:email AS SIGNED), name = JSON_COMPACT(:name), bio = JSON_COMPACT(:bio) where id = CAST(:id AS SIGNED)",
            "insert into users (name, email) values (:name, cast(:email as char)) on duplicate key update bio = CAST(:bio AS JSON), name = CAST(:again AS SIGNED)",
            "insert into users (name, email) select :name, cast(:email as char)",
            "update users set email = json_quote(:e), bio = concat(:b, '') where id = :id",
        ] {
            assert!(may_be_null(sql).is_empty(), "{sql}: {:?}", may_be_null(sql));
        }
    }

    #[test]
    fn a_null_test_or_a_null_safe_comparison_lets_no_null_through() {
        for sql in [
            "select id from users where isnull(:k) and :a <=> email and :b is not null and :c is null",
            "select id from users where cast(:p as char) is null",
            "delete from users where :email is null",
            "with input as (select :q as q) select id from users, input where input.q is null or users.name = input.q",
            "update users set email = coalesce(JSON_COMPACT(:e), email) where JSON_COMPACT(:j) is null",
        ] {
            assert!(may_be_null(sql).iter().all(|p| p == "e"), "{sql}: {:?}", may_be_null(sql));
        }
    }

    #[test]
    fn a_parameter_cast_to_nullable_may_be_null_and_a_coalesced_one_may_not() {
        assert_eq!(may_be_null("select id from users where id = coalesce(:id::nullable(int), id) and name = coalesce(:n, name) and ifnull(:m, 1) = 1"), ["id"]);
        // Through a WITH query, which selects the parameter once and tests it for null.
        assert_eq!(may_be_null("with input as (select :q::nullable(text) as q) select id from users, input where input.q is null or users.name = input.q"), ["q"]);
        assert!(may_be_null("with input as (select :q::text as q) select id from users, input where input.q is null or users.name = input.q").is_empty());
        // Written whole to a column that holds NULL, and as typed JSON.
        assert_eq!(may_be_null("update users set email = :e::nullable(text), bio = :b::nullable(json('{\"type\": \"object\"}')) where id = :id"), ["e", "b"]);
    }

    #[test]
    fn a_parameter_cast_to_nullable_and_written_to_a_column_that_refuses_null_is_an_error() {
        let messages = |sql: &str| analyzed(sql).0.diagnostics.into_iter().map(|d| d.message).collect::<Vec<_>>();
        assert_eq!(messages("insert into users (name, email) values (:name::nullable(text), :email::nullable(text))"), ["The parameter :name is cast to nullable(…), but it is written to users.name, which refuses null: cast it to its type alone."]);
        // Written whole by the one SELECT that feeds the INSERT, as a VALUES item is.
        assert_eq!(messages("insert into users (name, email) select :name::nullable(text), :email::nullable(text)"), ["The parameter :name is cast to nullable(…), but it is written to users.name, which refuses null: cast it to its type alone."]);
        // A CHECK that tests the column IS NOT NULL refuses it as NOT NULL does; one that tests
        // anything else lets NULL pass.
        assert_eq!(messages("update accounts set nick = :nick::nullable(text), note = :note::nullable(text) where id = 1"), ["The parameter :nick is cast to nullable(…), but it is written to accounts.nick, which refuses null: cast it to its type alone."]);
    }
}
