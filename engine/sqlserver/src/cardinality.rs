//! How many rows a SQL Server statement returns, read off its tree, since the server's describe
//! says nothing of it.
//!
//! - A SELECT with no FROM returns one row (at most one with a WHERE); an aggregate with no GROUP
//!   BY one (at most one with a HAVING), however many rows it reads.
//! - `TOP 1` or `FETCH NEXT 1 ROWS ONLY`, without `WITH TIES` or `PERCENT`, returns at most one.
//! - A SELECT, UPDATE or DELETE of one table whose WHERE holds a column of each column of one of its
//!   unique keys equal to a value returns at most one row: a key of an index that is unique, not
//!   filtered and not disabled ([`UNIQUE_KEYS`]).
//! - A write returns its `OUTPUT` rows, one per row it writes, and none without an `OUTPUT` (or with
//!   an `OUTPUT … INTO`): an INSERT of one VALUES row or of DEFAULT VALUES returns one.
//! - Anything else returns many.

use crate::host::{as_str, Host, Lazy};
use crate::api::Scope;
use reylo_engine_core::types::Cardinality;
use serde_json::{json, Value};
use sqlparser::ast::{BinaryOperator, Expr, FromTable, GroupByExpr, OutputClause, Query, Select, SetExpr, Statement, TableFactor, TableWithJoins, TopQuantity};

/// The columns of each unique index of the table `@P1` names, in key order: an index that is
/// unique, not filtered (a filtered one holds only its filter's rows to it), not disabled and not
/// hypothetical, its included columns left out.
pub const UNIQUE_KEYS: &str = "select i.index_id, c.name from sys.indexes i \
    join sys.index_columns ic on ic.object_id = i.object_id and ic.index_id = i.index_id \
    join sys.columns c on c.object_id = ic.object_id and c.column_id = ic.column_id \
    where i.object_id = object_id(@P1) and i.is_unique = 1 and i.has_filter = 0 and i.is_disabled = 0 and i.is_hypothetical = 0 and ic.is_included_column = 0 \
    order by i.index_id, ic.key_ordinal";

/// The unique keys of the one table a statement's row count turns on, each a list of its columns'
/// names as the catalog writes them.
#[derive(Debug, Clone, Default)]
pub struct Keys {
    pub keys: Vec<Vec<String>>,
}

/// The one table a lookup reads or writes, with the name its columns are qualified by.
fn lookup_table(statement: &Statement) -> Option<(&TableFactor, Option<&Expr>)> {
    match statement {
        Statement::Query(q) => {
            let s = lone_select(q)?;
            let [TableWithJoins { relation, joins }] = s.from.as_slice() else { return None };
            joins.is_empty().then_some((relation, s.selection.as_ref()))
        }
        Statement::Update(u) if u.table.joins.is_empty() && u.from.is_none() => Some((&u.table.relation, u.selection.as_ref())),
        Statement::Delete(d) if d.tables.is_empty() && d.using.is_none() => {
            let (FromTable::WithFromKeyword(from) | FromTable::WithoutKeyword(from)) = &d.from;
            let [TableWithJoins { relation, joins }] = from.as_slice() else { return None };
            joins.is_empty().then_some((relation, d.selection.as_ref()))
        }
        _ => None,
    }
}

fn lone_select(q: &Query) -> Option<&Select> {
    match q.body.as_ref() {
        SetExpr::Select(s) => Some(s),
        _ => None,
    }
}

/// The unique keys of the table a lookup reads, asked where the statement is one.
pub fn keys(statement: &Statement, host: &Host) -> Lazy<Keys> {
    let Some((TableFactor::Table { name, args: None, .. }, Some(_))) = lookup_table(statement) else { return Some(Keys::default()) };
    let rows = match host.query(UNIQUE_KEYS, vec![json!(name.to_string())], Scope::Statement)? {
        Ok(rows) => rows,
        Err(_) => return Some(Keys::default()),
    };
    let mut by_index: Vec<(i64, Vec<String>)> = Vec::new();
    for row in &rows {
        let (Some(index), Some(column)) = (row.first().and_then(Value::as_i64), row.get(1).and_then(as_str)) else { continue };
        match by_index.last_mut() {
            Some((i, cols)) if *i == index => cols.push(column.to_string()),
            _ => by_index.push((index, vec![column.to_string()])),
        }
    }
    Some(Keys { keys: by_index.into_iter().map(|(_, cols)| cols).collect() })
}

/// The statement's cardinality, and why it is no tighter where only the engine's reading bounds it.
/// `columns` is how many columns the server described.
pub fn of(statement: &Statement, keys: Option<&Keys>, columns: usize) -> (Cardinality, Option<String>) {
    if columns == 0 {
        return (Cardinality::None, None);
    }
    let returns_output = |o: &Option<OutputClause>| matches!(o, Some(OutputClause::Output { into_table: None, .. }));
    match statement {
        Statement::Query(q) => query(q, statement, keys),
        Statement::Insert(i) if returns_output(&i.output) => match i.source.as_deref().map(|s| s.body.as_ref()) {
            None => (Cardinality::One, None),
            Some(SetExpr::Values(v)) if v.rows.len() == 1 => (Cardinality::One, None),
            _ => (Cardinality::Many, None),
        },
        Statement::Update(u) if returns_output(&u.output) => lookup(statement, keys),
        Statement::Delete(d) if returns_output(&d.output) => lookup(statement, keys),
        Statement::Merge(m) if returns_output(&m.output) => (Cardinality::Many, None),
        Statement::Insert(_) | Statement::Update(_) | Statement::Delete(_) | Statement::Merge(_) => (Cardinality::None, None),
        _ => (Cardinality::Many, Some("only a SELECT and a write's OUTPUT are bounded".into())),
    }
}

fn query(q: &Query, statement: &Statement, keys: Option<&Keys>) -> (Cardinality, Option<String>) {
    let Some(s) = lone_select(q) else { return (Cardinality::Many, None) };
    if s.into.is_some() {
        return (Cardinality::None, None);
    }
    let first = |c: Cardinality| -> Cardinality {
        // TOP 1 or FETCH NEXT 1 bounds whatever the rest returns.
        if c == Cardinality::Many && (top_one(s) || fetch_one(q)) {
            Cardinality::AtMostOne
        } else {
            c
        }
    };
    let filtered = s.selection.is_some();
    if s.from.is_empty() {
        return (first(if filtered { Cardinality::AtMostOne } else { Cardinality::One }), None);
    }
    let grouped = !matches!(&s.group_by, GroupByExpr::Expressions(e, m) if e.is_empty() && m.is_empty());
    if !grouped && s.projection.iter().any(|item| match item {
        sqlparser::ast::SelectItem::UnnamedExpr(e) | sqlparser::ast::SelectItem::ExprWithAlias { expr: e, .. } => aggregates(e),
        _ => false,
    }) {
        // OFFSET skips the one row, as `TOP 0` returns none.
        let skipped = q.limit_clause.is_some();
        return (first(if s.having.is_some() || skipped { Cardinality::AtMostOne } else { Cardinality::One }), None);
    }
    let (c, reason) = lookup(statement, keys);
    (first(c), reason)
}

/// Whether the expression calls an aggregate outside a window. One with a subquery is not read: the
/// visit reaches into it, and its aggregates are its own.
fn aggregates(e: &Expr) -> bool {
    const AGGREGATES: &[&str] = &["count", "count_big", "sum", "avg", "min", "max", "string_agg", "stdev", "stdevp", "var", "varp", "checksum_agg", "approx_count_distinct"];
    let mut found = false;
    let _ = sqlparser::ast::visit_expressions(e, |e| {
        if let Expr::Function(f) = e {
            let name = f.name.0.last().map(|p| p.to_string().to_ascii_lowercase()).unwrap_or_default();
            if f.over.is_none() && AGGREGATES.contains(&name.as_str()) {
                found = true;
            }
        }
        std::ops::ControlFlow::<()>::Continue(())
    });
    found && !has_subquery(e)
}

fn has_subquery(e: &Expr) -> bool {
    let mut found = false;
    let _ = sqlparser::ast::visit_expressions(e, |e| {
        if let Expr::Subquery(_) | Expr::Exists { .. } | Expr::InSubquery { .. } = e {
            found = true;
        }
        std::ops::ControlFlow::<()>::Continue(())
    });
    found
}

fn is_one(e: &Expr) -> bool {
    match e {
        Expr::Nested(inner) => is_one(inner),
        Expr::Value(v) => v.value.to_string() == "1",
        _ => false,
    }
}

fn top_one(s: &Select) -> bool {
    s.top.as_ref().is_some_and(|t| {
        !t.with_ties
            && !t.percent
            && match &t.quantity {
                Some(TopQuantity::Constant(1)) => true,
                Some(TopQuantity::Expr(e)) => is_one(e),
                _ => false,
            }
    })
}

fn fetch_one(q: &Query) -> bool {
    q.fetch.as_ref().is_some_and(|f| !f.with_ties && !f.percent && f.quantity.as_ref().is_some_and(is_one))
}

/// At most one row where the statement's one table is looked up by a whole unique key.
fn lookup(statement: &Statement, keys: Option<&Keys>) -> (Cardinality, Option<String>) {
    let Some((TableFactor::Table { name, alias, args: None, .. }, Some(selection))) = lookup_table(statement) else { return (Cardinality::Many, None) };
    let Some(keys) = keys else { return (Cardinality::Many, None) };
    let qualifier = alias.as_ref().map(|a| a.name.value.clone()).or_else(|| name.0.last().map(|p| p.to_string().trim_matches(|c| c == '[' || c == ']').to_string()));
    let mut equal: Vec<String> = Vec::new();
    pinned(selection, qualifier.as_deref(), &mut equal);
    let found = keys.keys.iter().any(|key| !key.is_empty() && key.iter().all(|c| equal.iter().any(|e| e == c)));
    if found {
        (Cardinality::AtMostOne, None)
    } else {
        (Cardinality::Many, None)
    }
}

/// The columns the WHERE's top-level conjuncts hold equal to a value: a literal, a parameter, a
/// variable or anything that reads no column of the table.
fn pinned(e: &Expr, qualifier: Option<&str>, out: &mut Vec<String>) {
    match e {
        Expr::Nested(inner) => pinned(inner, qualifier, out),
        Expr::BinaryOp { left, op: BinaryOperator::And, right } => {
            pinned(left, qualifier, out);
            pinned(right, qualifier, out);
        }
        Expr::BinaryOp { left, op: BinaryOperator::Eq, right } => {
            for (column, value) in [(left, right), (right, left)] {
                if let Some(name) = column_of(column, qualifier) {
                    if constant(value) {
                        out.push(name);
                    }
                }
            }
        }
        _ => {}
    }
}

/// The column of the table an expression is, unqualified or qualified by the table's name or alias.
fn column_of(e: &Expr, qualifier: Option<&str>) -> Option<String> {
    let bare = |i: &sqlparser::ast::Ident| i.value.clone();
    match e {
        Expr::Nested(inner) => column_of(inner, qualifier),
        Expr::Identifier(i) => Some(bare(i)),
        Expr::CompoundIdentifier(parts) if parts.len() == 2 && qualifier.is_some_and(|q| q == parts[0].value) => Some(bare(&parts[1])),
        _ => None,
    }
}

/// Whether an expression is one value for the whole statement: a literal, a parameter or a
/// variable (`@p_1`, `@x`), or a cast or arithmetic of those.
fn constant(e: &Expr) -> bool {
    match e {
        Expr::Value(v) => !matches!(v.value, sqlparser::ast::Value::Null),
        Expr::Identifier(i) => i.value.starts_with('@'),
        Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } => constant(inner),
        Expr::Cast { expr, .. } => constant(expr),
        Expr::BinaryOp { left, right, .. } => constant(left) && constant(right),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::MsSqlDialect;
    use sqlparser::parser::Parser;

    fn card(sql: &str, keys: &[&[&str]]) -> Cardinality {
        let s = Parser::parse_sql(&MsSqlDialect {}, sql).unwrap().pop().unwrap();
        let keys = Keys { keys: keys.iter().map(|k| k.iter().map(|c| c.to_string()).collect()).collect() };
        of(&s, Some(&keys), 1).0
    }

    #[test]
    fn bounds_selects() {
        assert_eq!(card("select 1 as a", &[]), Cardinality::One);
        assert_eq!(card("select 1 as a where @p_1 = 1", &[]), Cardinality::AtMostOne);
        assert_eq!(card("select count(*) as c from t where a = @p_1", &[]), Cardinality::One);
        assert_eq!(card("select count(*) as c from t having count(*) > 1", &[]), Cardinality::AtMostOne);
        assert_eq!(card("select a, count(*) as c from t group by a", &[]), Cardinality::Many);
        assert_eq!(card("select top 1 a from t order by a", &[]), Cardinality::AtMostOne);
        assert_eq!(card("select top (1) with ties a from t order by a", &[]), Cardinality::Many);
        assert_eq!(card("select a from t order by a offset 0 rows fetch next 1 rows only", &[]), Cardinality::AtMostOne);
        assert_eq!(card("select a from t where id = @p_1", &[&["id"]]), Cardinality::AtMostOne);
        assert_eq!(card("select a from dbo.t x where x.id = @p_1 and x.b = 2", &[&["b", "id"]]), Cardinality::AtMostOne);
        assert_eq!(card("select a from t where id = @p_1 or id = 2", &[&["id"]]), Cardinality::Many);
        assert_eq!(card("select a from t join u on u.t_id = t.id where t.id = @p_1", &[&["id"]]), Cardinality::Many);
        assert_eq!(card("select a from t where ID = @p_1", &[&["id"]]), Cardinality::Many);
    }

    #[test]
    fn bounds_writes_by_their_output() {
        assert_eq!(card("insert into t (a) values (@p_1)", &[]), Cardinality::None);
        assert_eq!(card("insert into t (a) output inserted.id values (@p_1)", &[]), Cardinality::One);
        assert_eq!(card("insert into t (a) output inserted.id values (1), (2)", &[]), Cardinality::Many);
        assert_eq!(card("update t set a = 1 output inserted.a where id = @p_1", &[&["id"]]), Cardinality::AtMostOne);
        assert_eq!(card("delete from t output deleted.a where a = @p_1", &[&["id"]]), Cardinality::Many);
        assert_eq!(card("delete from t where id = @p_1", &[&["id"]]), Cardinality::None);
    }
}
