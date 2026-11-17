//! How many rows a SQL Server statement returns, read off its tree, since the server's describe
//! says nothing of it.
//!
//! - A SELECT with no FROM returns one row, and an aggregate with no GROUP BY one, however many rows
//!   it reads; at most one with a WHERE, a HAVING, an OFFSET or FETCH, or a TOP other than a
//!   number of at least 1, any of which can leave none.
//! - `TOP 1` or `FETCH NEXT 1 ROWS ONLY`, without `WITH TIES` or `PERCENT`, returns at most one.
//! - A SELECT, UPDATE or DELETE of one table whose WHERE holds a column of each column of one of its
//!   unique keys equal to a value returns at most one row: a key of an index that is unique, not
//!   filtered and not disabled ([`UNIQUE_KEYS`]), compared with a value SQL Server converts to the
//!   column's type rather than the other way round. A column converted to the value's type
//!   compares as its conversion does, which can make two keys equal: `'01'` and `'1'` both equal
//!   `1`, two `bigint`s one `float`, and `'ß'` the `N'ss'` a SQL collation keeps apart.
//!   A variable the batch declared is such a value, of its declared type, where the statement
//!   assigns no variable as it runs (`update … set @n = @n + 1`).
//!   A common table expression named like a table is no table.
//! - A write returns its `OUTPUT` rows, one per row it writes: an INSERT of one VALUES row or of
//!   DEFAULT VALUES returns one, or at most one where a unique index ignores duplicates. A write
//!   with no `OUTPUT` (or with an `OUTPUT … INTO`) returns none, or the rows its table's triggers
//!   return, as many.
//! - Anything else returns many.

use crate::host::{as_str, Host, Lazy};
use crate::api::Scope;
use reylo_engine_core::types::Cardinality;
use serde_json::{json, Value};
use sqlparser::ast::{BinaryOperator, Expr, FromTable, GroupByExpr, OutputClause, Query, Select, SetExpr, Statement, TableFactor, TableWithJoins, TopQuantity};

/// The columns of each unique index of the table `@P1` names, in key order: an index that is
/// unique, not filtered (a filtered one holds only its filter's rows to it), not disabled and not
/// hypothetical, its included columns left out. Only a user table of this database has keys here:
/// a view or a synonym has none of its own, and a name with a database part has an object id of
/// that database's.
pub const UNIQUE_KEYS: &str = "select i.index_id, c.name, type_name(c.system_type_id), c.collation_name from sys.indexes i \
    join sys.index_columns ic on ic.object_id = i.object_id and ic.index_id = i.index_id \
    join sys.columns c on c.object_id = ic.object_id and c.column_id = ic.column_id \
    where i.object_id = object_id(@P1) and parsename(@P1, 3) is null and parsename(@P1, 4) is null and objectproperty(i.object_id, 'IsUserTable') = 1 \
    and i.is_unique = 1 and i.has_filter = 0 and i.is_disabled = 0 and i.is_hypothetical = 0 and ic.is_included_column = 0 \
    order by i.index_id, ic.key_ordinal";

/// Whether an INSERT into the table `@P1` names may be left with no row: its table has a unique
/// index that ignores a duplicate rather than refuse it, or the name is no user table of this
/// database (a view, a synonym, a table of another database), whose indexes aren't read.
pub const IGNORES_DUPLICATES: &str = "select case when parsename(@P1, 3) is null and parsename(@P1, 4) is null and objectproperty(object_id(@P1), 'IsUserTable') = 1 \
    then (select count(*) from sys.indexes where object_id = object_id(@P1) and ignore_dup_key = 1) else 1 end";

/// One column of a unique key: its name as the catalog writes it, its type's name and its collation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyColumn {
    pub name: String,
    pub ty: String,
    pub collation: Option<String>,
}

/// The unique keys of the one table a statement's row count turns on, and for an INSERT whether
/// its table ignores duplicates.
#[derive(Debug, Clone, Default)]
pub struct Keys {
    pub keys: Vec<Vec<KeyColumn>>,
    pub ignores_duplicates: bool,
}

/// The names of the statement's common table expressions, lowercased.
fn ctes(statement: &Statement) -> Vec<String> {
    match statement {
        Statement::Query(q) => q.with.as_ref().map(|w| w.cte_tables.iter().map(|c| c.alias.name.value.to_ascii_lowercase()).collect()).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Whether a table's name is one of the statement's common table expressions.
fn is_cte(statement: &Statement, name: &sqlparser::ast::ObjectName) -> bool {
    name.0.len() == 1 && name.0.last().is_some_and(|p| ctes(statement).contains(&p.to_string().trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase()))
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

/// The unique keys of the table a lookup reads, and whether the table an INSERT writes ignores
/// duplicates, asked where the statement is one.
pub fn keys(statement: &Statement, host: &Host) -> Lazy<Keys> {
    if let Statement::Insert(i) = statement {
        let sqlparser::ast::TableObject::TableName(name) = &i.table else { return Some(Keys::default()) };
        let ignores = match host.query(IGNORES_DUPLICATES, vec![json!(name.to_string())], Scope::Statement)? {
            Ok(rows) => rows.first().and_then(|r| r.first()).and_then(Value::as_i64) != Some(0),
            // Not known: as if it did.
            Err(_) => true,
        };
        return Some(Keys { keys: Vec::new(), ignores_duplicates: ignores });
    }
    let Some((TableFactor::Table { name, args: None, .. }, Some(_))) = lookup_table(statement) else { return Some(Keys::default()) };
    if is_cte(statement, name) {
        return Some(Keys::default());
    }
    let rows = match host.query(UNIQUE_KEYS, vec![json!(name.to_string())], Scope::Statement)? {
        Ok(rows) => rows,
        Err(_) => return Some(Keys::default()),
    };
    let mut by_index: Vec<(i64, Vec<KeyColumn>)> = Vec::new();
    for row in &rows {
        let (Some(index), Some(column)) = (row.first().and_then(Value::as_i64), row.get(1).and_then(as_str)) else { continue };
        let column = KeyColumn { name: column.to_string(), ty: row.get(2).and_then(as_str).unwrap_or_default().to_ascii_lowercase(), collation: row.get(3).and_then(as_str).map(String::from) };
        match by_index.last_mut() {
            Some((i, cols)) if *i == index => cols.push(column),
            _ => by_index.push((index, vec![column])),
        }
    }
    Some(Keys { keys: by_index.into_iter().map(|(_, cols)| cols).collect(), ignores_duplicates: false })
}

/// The statement's cardinality, and why it is no tighter where only the engine's reading bounds it.
/// `columns` is how many columns the server described.
/// `variables` are the batch's variables declared before the statement, each with its type's name.
pub fn of(statement: &Statement, keys: Option<&Keys>, columns: usize, variables: &[(String, String)]) -> (Cardinality, Option<String>) {
    if columns == 0 {
        return (Cardinality::None, None);
    }
    let returns_output = |o: &Option<OutputClause>| matches!(o, Some(OutputClause::Output { into_table: None, .. }));
    // A unique index that ignores duplicates drops the row, and its OUTPUT with it.
    let one_insert = || if keys.is_some_and(|k| !k.ignores_duplicates) { Cardinality::One } else { Cardinality::AtMostOne };
    match statement {
        Statement::Query(q) => query(q, statement, keys, variables),
        Statement::Insert(i) if returns_output(&i.output) => match i.source.as_deref().map(|s| s.body.as_ref()) {
            None => (one_insert(), None),
            Some(SetExpr::Values(v)) if v.rows.len() == 1 => (one_insert(), None),
            _ => (Cardinality::Many, None),
        },
        Statement::Update(u) if returns_output(&u.output) => lookup(statement, keys, variables),
        Statement::Delete(d) if returns_output(&d.output) => lookup(statement, keys, variables),
        Statement::Merge(m) if returns_output(&m.output) => (Cardinality::Many, None),
        // A write that outputs nothing of its own, yet whose first result set the server described:
        // the rows a trigger on its table returns.
        Statement::Insert(_) | Statement::Update(_) | Statement::Delete(_) | Statement::Merge(_) => (Cardinality::Many, Some("its rows are those a trigger on the table returns".into())),
        _ => (Cardinality::Many, Some("only a SELECT and a write's OUTPUT are bounded".into())),
    }
}

fn query(q: &Query, statement: &Statement, keys: Option<&Keys>, variables: &[(String, String)]) -> (Cardinality, Option<String>) {
    let Some(s) = lone_select(q) else { return (Cardinality::Many, None) };
    if s.into.is_some() {
        return (Cardinality::None, None);
    }
    let first = |c: Cardinality| -> Cardinality {
        match c {
            // TOP 1 or FETCH NEXT 1 bounds whatever the rest returns.
            Cardinality::Many if top_one(s) || fetch_one(q) => Cardinality::AtMostOne,
            // One row, unless a TOP, an OFFSET or a FETCH leaves none.
            Cardinality::One if top_may_leave_none(s) || q.limit_clause.is_some() || q.fetch.is_some() => Cardinality::AtMostOne,
            c => c,
        }
    };
    if s.from.is_empty() {
        return (first(if s.selection.is_some() || s.having.is_some() { Cardinality::AtMostOne } else { Cardinality::One }), None);
    }
    let grouped = !matches!(&s.group_by, GroupByExpr::Expressions(e, m) if e.is_empty() && m.is_empty());
    if !grouped && s.projection.iter().any(|item| match item {
        sqlparser::ast::SelectItem::UnnamedExpr(e) | sqlparser::ast::SelectItem::ExprWithAlias { expr: e, .. } => aggregates(e),
        _ => false,
    }) {
        return (first(if s.having.is_some() { Cardinality::AtMostOne } else { Cardinality::One }), None);
    }
    let (c, reason) = lookup(statement, keys, variables);
    (first(c), reason)
}

/// Whether the expression calls an aggregate outside a window. One with a subquery is not read: the
/// visit reaches into it, and its aggregates are its own.
fn aggregates(e: &Expr) -> bool {
    const AGGREGATES: &[&str] = &["count", "count_big", "sum", "avg", "min", "max", "string_agg", "stdev", "stdevp", "var", "varp", "checksum_agg", "approx_count_distinct"];
    let mut found = false;
    let _ = sqlparser::ast::visit_expressions(e, |e| {
        // A function named with its schema is the user's own (`dbo.max`), never an aggregate.
        if let Expr::Function(f) = e {
            let name = if f.name.0.len() == 1 { f.name.0[0].to_string().trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase() } else { String::new() };
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

/// Whether a TOP may return no row of one: any but a number of at least 1, or a percentage.
fn top_may_leave_none(s: &Select) -> bool {
    s.top.as_ref().is_some_and(|t| {
        t.percent
            || match &t.quantity {
                Some(TopQuantity::Constant(n)) => *n < 1,
                Some(TopQuantity::Expr(e)) => !at_least_one(e),
                None => true,
            }
    })
}

fn at_least_one(e: &Expr) -> bool {
    match e {
        Expr::Nested(inner) => at_least_one(inner),
        Expr::Value(v) => v.value.to_string().parse::<i64>().is_ok_and(|n| n >= 1),
        _ => false,
    }
}

fn fetch_one(q: &Query) -> bool {
    q.fetch.as_ref().is_some_and(|f| !f.with_ties && !f.percent && f.quantity.as_ref().is_some_and(is_one))
}

/// At most one row where the statement's one table is looked up by a whole unique key.
fn lookup(statement: &Statement, keys: Option<&Keys>, variables: &[(String, String)]) -> (Cardinality, Option<String>) {
    let Some((TableFactor::Table { name, alias, args: None, .. }, Some(selection))) = lookup_table(statement) else { return (Cardinality::Many, None) };
    let Some(keys) = keys else { return (Cardinality::Many, None) };
    if is_cte(statement, name) {
        return (Cardinality::Many, None);
    }
    let qualifier = alias.as_ref().map(|a| a.name.value.clone()).or_else(|| name.0.last().map(|p| p.to_string().trim_matches(|c| c == '[' || c == ']').to_string()));
    let mut equal: Vec<(String, &Expr)> = Vec::new();
    pinned(selection, qualifier.as_deref(), &mut equal);
    // An UPDATE that sets a variable changes it row by row, so the WHERE may read it as another value.
    let assigns = matches!(statement, Statement::Update(u) if u.assignments.iter().any(|a| a.target.to_string().starts_with('@')));
    let variables = if assigns { &[] } else { variables };
    let found = keys.keys.iter().any(|key| !key.is_empty() && key.iter().all(|c| equal.iter().any(|(name, value)| *name == c.name && keeps_column(c, value, variables))));
    if found {
        (Cardinality::AtMostOne, None)
    } else {
        (Cardinality::Many, None)
    }
}

/// The columns the WHERE's top-level conjuncts hold equal to an expression, with the expression.
fn pinned<'a>(e: &'a Expr, qualifier: Option<&str>, out: &mut Vec<(String, &'a Expr)>) {
    match e {
        Expr::Nested(inner) => pinned(inner, qualifier, out),
        Expr::BinaryOp { left, op: BinaryOperator::And, right } => {
            pinned(left, qualifier, out);
            pinned(right, qualifier, out);
        }
        Expr::BinaryOp { left, op: BinaryOperator::Eq, right } => {
            for (column, value) in [(left, right), (right, left)] {
                if let Some(name) = column_of(column, qualifier) {
                    out.push((name, value.as_ref()));
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

/// The type of a value a key column is compared with, where it is one value for the whole
/// statement: a parameter, which SQL Server types from the comparison itself, so as the column; a
/// variable the batch declared, of its declared type; a literal; or a cast of any of them.
#[derive(Debug, PartialEq)]
enum ValueType {
    /// A parameter standing alone: SQL Server gives it the column's type.
    Column,
    Named(String),
}

fn value_type(e: &Expr, variables: &[(String, String)]) -> Option<ValueType> {
    use sqlparser::ast::Value as V;
    match e {
        Expr::Nested(inner) => value_type(inner, variables),
        Expr::Identifier(i) if i.value.starts_with("@p_") => Some(ValueType::Column),
        Expr::Identifier(i) if i.value.starts_with('@') && !i.value.starts_with("@@") => declared(&i.value, variables).map(|ty| ValueType::Named(ty.to_string())),
        Expr::Value(v) => Some(ValueType::Named(
            match &v.value {
                V::SingleQuotedString(_) => "varchar",
                V::NationalStringLiteral(_) => "nvarchar",
                V::Number(n, _) if n.contains(['e', 'E']) => "float",
                V::Number(n, _) if n.contains('.') => "numeric",
                V::Number(_, _) => "int",
                V::HexStringLiteral(_) => "varbinary",
                _ => return None,
            }
            .into(),
        )),
        Expr::UnaryOp { op: sqlparser::ast::UnaryOperator::Minus | sqlparser::ast::UnaryOperator::Plus, expr } => match value_type(expr, variables)? {
            ValueType::Named(t) if matches!(t.as_str(), "int" | "numeric" | "float") => Some(ValueType::Named(t)),
            _ => None,
        },
        Expr::Cast { data_type, expr, .. } if value_type(expr, variables).is_some() => Some(ValueType::Named(data_type.to_string().split('(').next().unwrap_or_default().trim().to_ascii_lowercase())),
        _ => None,
    }
}

/// A variable's declared type: by its name as written, else by the one name that matches it in any
/// case, as a case-insensitive collation matches it.
fn declared<'a>(name: &str, variables: &'a [(String, String)]) -> Option<&'a str> {
    if let Some((_, ty)) = variables.iter().rev().find(|(n, _)| n == name) {
        return Some(ty);
    }
    let mut matching = variables.iter().filter(|(n, _)| n.eq_ignore_ascii_case(name));
    match (matching.next(), matching.next()) {
        (Some((_, ty)), None) => Some(ty),
        _ => None,
    }
}

/// A type's family, as SQL Server converts between them.
fn family(ty: &str) -> &'static str {
    match ty {
        "char" | "varchar" | "text" => "string",
        "nchar" | "nvarchar" | "ntext" | "sysname" => "unicode",
        "tinyint" | "smallint" | "int" | "integer" | "bigint" => "integer",
        "decimal" | "numeric" | "money" | "smallmoney" => "exact",
        "float" | "real" => "approximate",
        "bit" => "bit",
        "binary" | "varbinary" | "image" => "binary",
        "uniqueidentifier" => "uuid",
        "date" | "time" | "datetime" | "datetime2" | "smalldatetime" | "datetimeoffset" => "temporal",
        _ => "other",
    }
}

/// Whether comparing the key column with the value keeps the column as it is (the value is
/// converted to the column's type, or the column widened without two of its values meeting), so
/// that a unique key still matches one row.
fn keeps_column(column: &KeyColumn, value: &Expr, variables: &[(String, String)]) -> bool {
    let Some(value) = value_type(value, variables) else { return false };
    let ValueType::Named(ty) = value else { return true };
    match (family(&column.ty), family(&ty)) {
        ("string", "string") | ("unicode", "string" | "unicode") => true,
        // A varchar column is compared as Unicode: as its own rules under a Windows collation, but
        // a SQL collation compares Unicode by Windows rules, which equal `'ß'` and `'ss'`.
        ("string", "unicode") => column.collation.as_deref().is_some_and(|c| !c.to_ascii_uppercase().starts_with("SQL_")),
        ("integer", "integer" | "exact") | ("exact", "integer" | "exact") => true,
        // A float holds every int but not every bigint.
        ("integer", "approximate") => column.ty != "bigint",
        ("approximate", "integer" | "exact" | "approximate") => true,
        ("bit", "bit" | "integer") => true,
        ("uuid", "string" | "unicode" | "uuid") => true,
        ("temporal", "string" | "unicode" | "temporal") => true,
        ("binary", "binary") => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::MsSqlDialect;
    use sqlparser::parser::Parser;

    fn typed(sql: &str, keys: &[&[(&str, &str)]], collation: Option<&str>) -> Cardinality {
        declared_as(sql, keys, collation, &[])
    }

    fn declared_as(sql: &str, keys: &[&[(&str, &str)]], collation: Option<&str>, variables: &[(&str, &str)]) -> Cardinality {
        let s = Parser::parse_sql(&MsSqlDialect {}, sql).unwrap().pop().unwrap();
        let keys = Keys { keys: keys.iter().map(|k| k.iter().map(|(c, t)| KeyColumn { name: c.to_string(), ty: t.to_string(), collation: collation.map(String::from) }).collect()).collect(), ignores_duplicates: false };
        let variables: Vec<(String, String)> = variables.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect();
        of(&s, Some(&keys), 1, &variables).0
    }

    /// Keys of int columns.
    fn card(sql: &str, keys: &[&[&str]]) -> Cardinality {
        let keys: Vec<Vec<(&str, &str)>> = keys.iter().map(|k| k.iter().map(|c| (*c, "int")).collect()).collect();
        let refs: Vec<&[(&str, &str)]> = keys.iter().map(Vec::as_slice).collect();
        typed(sql, &refs, None)
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
        // A common table expression named like the table is not the table.
        assert_eq!(card("with t as (select id from t cross join (values (1), (2)) x (n)) select id from t where id = @p_1", &[&["id"]]), Cardinality::Many);
    }

    #[test]
    fn a_top_an_offset_or_a_having_can_leave_one_row_none() {
        assert_eq!(card("select top 0 count(*) as c from t", &[]), Cardinality::AtMostOne);
        assert_eq!(card("select top (0) percent count(*) as c from t", &[]), Cardinality::AtMostOne);
        assert_eq!(card("select top (@p_1) count(*) as c from t", &[]), Cardinality::AtMostOne);
        assert_eq!(card("select top 3 count(*) as c from t", &[]), Cardinality::One);
        assert_eq!(card("select top 0 1 as a", &[]), Cardinality::AtMostOne);
        assert_eq!(card("select 1 as a having 1 = 0", &[]), Cardinality::AtMostOne);
        assert_eq!(card("select 1 as a order by a offset 1 rows", &[]), Cardinality::AtMostOne);
        assert_eq!(card("select count(*) as c from t order by c offset 0 rows", &[]), Cardinality::AtMostOne);
        // A function with a schema is the user's own, not an aggregate.
        assert_eq!(card("select dbo.max(n) as m from t", &[]), Cardinality::Many);
    }

    #[test]
    fn a_key_counts_where_its_column_is_not_converted() {
        let code = [("code", "varchar")];
        assert_eq!(typed("select id from t where code = @p_1", &[&code], Some("Latin1_General_CI_AS")), Cardinality::AtMostOne);
        assert_eq!(typed("select id from t where code = 'a'", &[&code], Some("Latin1_General_CI_AS")), Cardinality::AtMostOne);
        assert_eq!(typed("select id from t where code = 1", &[&code], Some("Latin1_General_CI_AS")), Cardinality::Many);
        assert_eq!(typed("select id from t where code = cast(@p_1 as int)", &[&code], Some("Latin1_General_CI_AS")), Cardinality::Many);
        assert_eq!(typed("select id from t where code = N'ss'", &[&code], Some("Latin1_General_CI_AS")), Cardinality::AtMostOne);
        assert_eq!(typed("select id from t where code = N'ss'", &[&code], Some("SQL_Latin1_General_CP1_CI_AS")), Cardinality::Many);
        let k = [("k", "bigint")];
        assert_eq!(typed("select id from t where k = 9007199254740992", &[&k], None), Cardinality::AtMostOne);
        assert_eq!(typed("select id from t where k = 9007199254740992e0", &[&k], None), Cardinality::Many);
        assert_eq!(typed("select id from t where k = @p_1 + 1", &[&k], None), Cardinality::Many);
        assert_eq!(typed("select id from t where k = -5", &[&k], None), Cardinality::AtMostOne);
    }

    #[test]
    fn a_declared_variable_is_one_value_of_its_type() {
        let id = [("id", "int")];
        assert_eq!(declared_as("select a from t where id = @n", &[&id], None, &[("@n", "bigint")]), Cardinality::AtMostOne);
        assert_eq!(declared_as("select a from t where id = @N", &[&id], None, &[("@n", "int")]), Cardinality::AtMostOne);
        // A variable of a type the column converts to, one the batch never declared, and one an
        // UPDATE sets as it runs.
        assert_eq!(declared_as("select a from t where id = @n", &[&id], None, &[("@n", "float")]), Cardinality::AtMostOne);
        assert_eq!(declared_as("select a from t where id = @n", &[&id], None, &[("@n", "nvarchar")]), Cardinality::Many);
        assert_eq!(declared_as("select a from t where id = @m", &[&id], None, &[("@n", "int")]), Cardinality::Many);
        assert_eq!(declared_as("update t set @n = a, a = 1 output inserted.a where id = @n", &[&id], None, &[("@n", "int")]), Cardinality::Many);
        assert_eq!(declared_as("update t set a = 1 output inserted.a where id = @n", &[&id], None, &[("@n", "int")]), Cardinality::AtMostOne);
    }

    #[test]
    fn bounds_writes_by_their_output() {
        let s = Parser::parse_sql(&MsSqlDialect {}, "insert into t (a) values (@p_1)").unwrap().pop().unwrap();
        assert_eq!(of(&s, None, 0, &[]).0, Cardinality::None);
        assert_eq!(card("insert into t (a) output inserted.id values (@p_1)", &[]), Cardinality::One);
        let s = Parser::parse_sql(&MsSqlDialect {}, "insert into t (a) output inserted.id values (@p_1)").unwrap().pop().unwrap();
        assert_eq!(of(&s, Some(&Keys { keys: Vec::new(), ignores_duplicates: true }), 1, &[]).0, Cardinality::AtMostOne);
        assert_eq!(card("insert into t (a) output inserted.id values (1), (2)", &[]), Cardinality::Many);
        assert_eq!(card("update t set a = 1 output inserted.a where id = @p_1", &[&["id"]]), Cardinality::AtMostOne);
        assert_eq!(card("delete from t output deleted.a where a = @p_1", &[&["id"]]), Cardinality::Many);
        // A write that outputs nothing returns what its table's triggers return, where the server
        // described a result.
        assert_eq!(card("delete from t where id = @p_1", &[&["id"]]), Cardinality::Many);
        let s = Parser::parse_sql(&MsSqlDialect {}, "delete from t where id = @p_1").unwrap().pop().unwrap();
        assert_eq!(of(&s, None, 0, &[]).0, Cardinality::None);
    }
}
