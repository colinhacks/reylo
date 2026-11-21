//! The labels of an ENUM or a SET column a result column reads through derived tables, WITH
//! queries and UNIONs, which the server's prepare does not give (measured on 8.4):
//!
//! - Through a derived table or a WITH query the column keeps the ENUM or SET flag, and its origin
//!   is the base table with the derived column's own name (`select x.r from (select role as r from
//!   users) x` reads `users.r`), which may be another column of that table or none.
//! - A UNION is a VARCHAR with no flag, though each of its rows is a row of one of its sides.
//!
//! So the engine follows the column down the tree itself: in a query block, the item at its
//! position is a column reference, and the FROM item it names is a base table, whose column's
//! labels the catalog gives, or a derived table or a WITH query, whose column of that name is
//! followed the same way; in a UNION, every side's column at its position is. A column of a
//! UNION of ENUM columns holds the labels of every side's. A UNION of SET columns stays a string,
//! since typing it as a SET would change the value `.run()` returns. A reference whose FROM item is not certain (an
//! unqualified name where some FROM item's columns are unknown) and anything else (an
//! expression, a literal, a recursive WITH query, a LATERAL table, a table function) reads no
//! labels, and the column keeps the server's type. So does a statement with a WITH in a set operation's
//! operand before the last, whose scope reaches the operands after it
//! ([`crate::mysql_derived::with_outlives_operand`]).

use crate::api::{MysqlColumn, MysqlDescribe};
use crate::host::{as_str, Host, Lazy};
use crate::mysql_params::TYPES_QUERY;
use crate::types::Type;
use sqlparser::ast::{Cte, Expr, ObjectNamePart, Query, Select, SelectItem, SetExpr, SetOperator, TableFactor, TableWithJoins, With};

/// How many derived tables, WITH queries and UNION sides deep a column is followed.
const MAX_DEPTH: usize = 8;

/// The protocol's flags for an ENUM and a SET column.
const ENUM: u32 = 256;
const SET: u32 = 2048;

/// The labels a column holds: an ENUM's, or a SET's members'.
#[derive(Debug, Clone, PartialEq)]
enum Labels {
    Enum(Vec<String>),
    Set(Vec<String>),
}

impl Labels {
    /// Both columns' labels, where they are of one kind: the first's, then the second's it lacks.
    fn merge(self, other: Labels) -> Option<Labels> {
        let join = |mut a: Vec<String>, b: Vec<String>| {
            for label in b {
                if !a.contains(&label) {
                    a.push(label);
                }
            }
            a
        };
        match (self, other) {
            (Labels::Enum(a), Labels::Enum(b)) => Some(Labels::Enum(join(a, b))),
            (Labels::Set(a), Labels::Set(b)) => Some(Labels::Set(join(a, b))),
            _ => None,
        }
    }

    fn of_all(all: Vec<Labels>) -> Option<Labels> {
        let mut all = all.into_iter();
        let first = all.next()?;
        all.try_fold(first, Labels::merge)
    }
}

/// Per result column of `q`, as the server describes it (`raw`), the type with the labels of the
/// ENUM or SET columns it reads itself, where the server's own type lacks them; `None` for every
/// other column.
pub(crate) fn columns(q: &Query, raw: &MysqlDescribe, host: &Host) -> Lazy<Vec<Option<Type>>> {
    if crate::mysql_derived::with_outlives_operand(q) {
        return Some(vec![None; raw.columns.len()]);
    }
    let set_operation = holds_set_operation(q);
    let mut pending = false;
    let mut out = Vec::with_capacity(raw.columns.len());
    for (position, c) in raw.columns.iter().enumerate() {
        // A string column: one the server flags ENUM or SET, or any where a UNION may have lost the flag.
        let string = matches!(c.r#type, 15 | 253 | 254) && c.charset != crate::mysql_types::BINARY;
        if !string || (c.flags & (ENUM | SET) == 0 && !set_operation) {
            out.push(None);
            continue;
        }
        let traced = match trace(q, position, Some(c), host, 0) {
            Some(traced) => traced,
            None => {
                pending = true;
                None
            }
        };
        // A column the server flags ENUM, or a UNION's VARCHAR of ENUM columns, is one label. Only
        // a column it flags SET is a SET: a UNION of SET columns stays the string the driver has
        // always returned for it, where the type would make it a `Set`.
        let ty = match traced {
            Some(Labels::Enum(labels)) if c.flags & SET == 0 => Some(crate::mysql_types::enumeration(Some(labels))),
            Some(Labels::Set(labels)) if c.flags & SET != 0 => Some(crate::mysql_types::set_of(Some(labels))),
            _ => None,
        };
        out.push(ty);
    }
    if pending {
        None
    } else {
        Some(out)
    }
}

/// Whether a query has a set operation anywhere a column is followed: its body, its WITH
/// queries, its derived tables.
fn holds_set_operation(q: &Query) -> bool {
    fn body(b: &SetExpr) -> bool {
        match b {
            SetExpr::SetOperation { .. } => true,
            SetExpr::Query(q) => holds_set_operation(q),
            SetExpr::Select(s) => s.from.iter().any(from),
            _ => false,
        }
    }
    fn from(item: &TableWithJoins) -> bool {
        std::iter::once(&item.relation).chain(item.joins.iter().map(|j| &j.relation)).any(|f| match f {
            TableFactor::Derived { subquery, .. } => holds_set_operation(subquery),
            TableFactor::NestedJoin { table_with_joins, .. } => from(table_with_joins),
            _ => false,
        })
    }
    q.with.as_ref().is_some_and(|w| w.cte_tables.iter().any(|c| holds_set_operation(&c.query))) || body(&q.body)
}

/// The labels the column at `position` of `q` holds, a query whose WITH holds every WITH query in
/// scope. `described` is the server's description of the column where the caller has it. The
/// outer `None` is a describe or a catalog query still to be answered.
fn trace(q: &Query, position: usize, described: Option<&MysqlColumn>, host: &Host, depth: usize) -> Lazy<Option<Labels>> {
    if depth > MAX_DEPTH {
        return Some(None);
    }
    match q.body.as_ref() {
        SetExpr::Select(select) => select_column(q, select, position, described, host, depth),
        SetExpr::Query(inner) => match crate::mysql_derived::standalone(inner, q.with.as_ref()) {
            Some(inner) => trace(&inner, position, None, host, depth + 1),
            None => Some(None),
        },
        SetExpr::SetOperation { op: SetOperator::Union, .. } => {
            let mut sides = Vec::new();
            union_sides(&q.body, &mut sides);
            let mut found = Vec::new();
            let mut pending = false;
            for side in sides {
                let side = Query { body: Box::new(side.clone()), order_by: None, limit_clause: None, fetch: None, locks: Vec::new(), for_clause: None, ..q.clone() };
                match trace(&side, position, None, host, depth + 1) {
                    Some(Some(labels)) => found.push(labels),
                    Some(None) => return Some(None),
                    None => pending = true,
                }
            }
            if pending {
                None
            } else {
                Some(Labels::of_all(found))
            }
        }
        _ => Some(None),
    }
}

/// The sides of a UNION, through the UNIONs nested in it.
fn union_sides<'a>(b: &'a SetExpr, out: &mut Vec<&'a SetExpr>) {
    match b {
        SetExpr::SetOperation { op: SetOperator::Union, left, right, .. } => {
            union_sides(left, out);
            union_sides(right, out);
        }
        other => out.push(other),
    }
}

/// A column reference, as the qualifier it names its FROM item by and the column's name: `None`
/// for anything else.
fn reference(e: &Expr) -> Option<(Option<&str>, &str)> {
    match e {
        Expr::Identifier(column) => Some((None, &column.value)),
        Expr::CompoundIdentifier(parts) if parts.len() == 2 => Some((Some(&parts[0].value), &parts[1].value)),
        Expr::Nested(inner) => reference(inner),
        _ => None,
    }
}

/// The column an item of a select list references, if it is one.
fn reference_of(item: &SelectItem) -> Option<(Option<&str>, &str)> {
    match item {
        SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => reference(e),
        _ => None,
    }
}

/// A FROM item of a query block, as the column references name it.
enum Item {
    /// A base table or a view of the current schema, by its name.
    Table(String),
    /// A derived table's or a WITH query's query, standing alone with the WITH queries in its
    /// scope, and its column list where it has one.
    Query(Query, Vec<String>),
    /// Anything whose columns are not followed.
    Unknown,
}

/// The FROM items of a query block, each with the name its columns are qualified by.
fn items(q: &Query, select: &Select) -> Vec<(String, Item)> {
    fn factor(q: &Query, t: &TableFactor, out: &mut Vec<(String, Item)>) {
        match t {
            TableFactor::Table { name, alias, args: None, .. } => {
                let parts: Vec<&str> = name.0.iter().filter_map(|p| if let ObjectNamePart::Identifier(i) = p { Some(i.value.as_str()) } else { None }).collect();
                let [table] = parts.as_slice() else {
                    let qualifier = alias.as_ref().map(|a| a.name.value.clone()).or_else(|| parts.last().map(|p| p.to_string())).unwrap_or_default();
                    out.push((qualifier, Item::Unknown));
                    return;
                };
                let qualifier = alias.as_ref().map_or_else(|| table.to_string(), |a| a.name.value.clone());
                // A WITH query of that name in scope, the innermost first, is what the name reads.
                let ctes: &[Cte] = q.with.as_ref().map_or(&[], |w| &w.cte_tables);
                match ctes.iter().rposition(|c| c.alias.name.value.eq_ignore_ascii_case(table)) {
                    // A recursive WITH query, or one whose name is in another case, is not followed.
                    Some(i) if q.with.as_ref().is_some_and(|w| w.recursive) || ctes[i].alias.name.value != *table => out.push((qualifier, Item::Unknown)),
                    Some(i) => {
                        let earlier = q.with.as_ref().map(|w| With { cte_tables: ctes[..i].to_vec(), ..w.clone() }).filter(|w| !w.cte_tables.is_empty());
                        match crate::mysql_derived::standalone(&ctes[i].query, earlier.as_ref()) {
                            Some(alone) => out.push((qualifier, Item::Query(alone, ctes[i].alias.columns.iter().map(|c| c.name.value.clone()).collect()))),
                            None => out.push((qualifier, Item::Unknown)),
                        }
                    }
                    None => out.push((qualifier, Item::Table(table.to_string()))),
                }
            }
            TableFactor::Derived { lateral: false, subquery, alias: Some(alias), .. } => match crate::mysql_derived::standalone(subquery, q.with.as_ref()) {
                Some(alone) => out.push((alias.name.value.clone(), Item::Query(alone, alias.columns.iter().map(|c| c.name.value.clone()).collect()))),
                None => out.push((alias.name.value.clone(), Item::Unknown)),
            },
            TableFactor::NestedJoin { table_with_joins, .. } => from(q, table_with_joins, out),
            _ => out.push((String::new(), Item::Unknown)),
        }
    }
    fn from(q: &Query, item: &TableWithJoins, out: &mut Vec<(String, Item)>) {
        factor(q, &item.relation, out);
        for join in &item.joins {
            factor(q, &join.relation, out);
        }
    }
    let mut out = Vec::new();
    for item in &select.from {
        from(q, item, &mut out);
    }
    out
}

/// The labels of the column at `position` of a query block: the column it references, in each
/// FROM item that may hold it.
fn select_column(q: &Query, select: &Select, position: usize, described: Option<&MysqlColumn>, host: &Host, depth: usize) -> Lazy<Option<Labels>> {
    let wildcards = select.projection.iter().any(|i| matches!(i, SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..)));
    // The column's qualifier and name: the item at its position, or behind a wildcard, which
    // spreads no expression, the name the server gives as its origin, in any FROM item.
    let (qualifier, name): (Option<String>, String) = if !wildcards {
        let Some((qualifier, name)) = select.projection.get(position).and_then(reference_of) else { return Some(None) };
        (qualifier.map(str::to_string), name.to_string())
    } else {
        if !select.projection.iter().all(|i| matches!(i, SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..)) || reference_of(i).is_some()) {
            return Some(None);
        }
        let described = match described {
            Some(c) => c.clone(),
            None => match host.describe(&q.to_string())? {
                Ok(d) => match d.columns.get(position) {
                    Some(c) => c.clone(),
                    None => return Some(None),
                },
                Err(_) => return Some(None),
            },
        };
        match described.org_name.filter(|n| !n.is_empty()) {
            Some(name) => (None, name),
            None => return Some(None),
        }
    };
    let items = items(q, select);
    let candidates: Vec<&(String, Item)> = match &qualifier {
        Some(qualifier) => {
            let named: Vec<_> = items.iter().filter(|(n, _)| n.eq_ignore_ascii_case(qualifier)).collect();
            if named.len() != 1 {
                return Some(None);
            }
            named
        }
        None => items.iter().collect(),
    };
    let mut found = Vec::new();
    let mut pending = false;
    for (_, item) in candidates {
        let labels = match item {
            Item::Unknown => return Some(None),
            Item::Table(table) => match table_column(table, &name, host) {
                Some(Column::Labelled(labels)) => Some(Some(labels)),
                Some(Column::Absent) if qualifier.is_none() => continue,
                Some(_) => Some(None),
                None => None,
            },
            Item::Query(inner, listed) => match inner_position(inner, listed, &name, host) {
                Some(Found::At(i)) => trace(inner, i, None, host, depth + 1),
                Some(Found::Absent) if qualifier.is_none() => continue,
                Some(_) => Some(None),
                None => None,
            },
        };
        match labels {
            Some(Some(labels)) => found.push(labels),
            Some(None) => return Some(None),
            None => pending = true,
        }
    }
    if pending {
        None
    } else {
        Some(Labels::of_all(found))
    }
}

/// Where a derived table's or a WITH query's query has the column `name`: by its column list, or
/// by the names the server gives its columns.
enum Found {
    At(usize),
    /// No column of that name.
    Absent,
    /// A query the server does not describe.
    Unknown,
}

fn inner_position(inner: &Query, listed: &[String], name: &str, host: &Host) -> Lazy<Found> {
    let at = |position: Option<usize>| position.map_or(Found::Absent, Found::At);
    if !listed.is_empty() {
        return Some(at(listed.iter().position(|c| c.eq_ignore_ascii_case(name))));
    }
    Some(match host.describe(&inner.to_string())? {
        Ok(d) => at(d.columns.iter().position(|c| c.name.eq_ignore_ascii_case(name))),
        Err(_) => Found::Unknown,
    })
}

/// What the catalog says of a base table's column.
enum Column {
    /// An ENUM or a SET column, with its labels.
    Labelled(Labels),
    /// A column of another type, or one whose labels are not read.
    Other,
    /// No column of that name in a table the catalog knows.
    Absent,
    /// A table the catalog does not know (a TEMPORARY table, one of another case).
    Unknown,
}

fn table_column(table: &str, column: &str, host: &Host) -> Lazy<Column> {
    let rows = crate::mysql::by_table(host, TYPES_QUERY, &[table.to_string()])?.unwrap_or_default();
    let rows: Vec<_> = rows.iter().filter(|row| row.first().and_then(as_str).is_some_and(|t| t == table)).collect();
    if rows.is_empty() {
        return Some(Column::Unknown);
    }
    let Some(row) = rows.iter().find(|row| row.get(1).and_then(as_str).is_some_and(|c| c.eq_ignore_ascii_case(column))) else { return Some(Column::Absent) };
    let (Some(data_type), Some(column_type)) = (row.get(2).and_then(as_str), row.get(3).and_then(as_str)) else { return Some(Column::Other) };
    let labels = crate::mysql_types::labels(column_type);
    Some(match (data_type.to_ascii_lowercase().as_str(), labels) {
        ("enum", Some(labels)) => Column::Labelled(Labels::Enum(labels)),
        ("set", Some(labels)) => Column::Labelled(Labels::Set(labels)),
        _ => Column::Other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_union_holds_every_sides_labels_of_one_kind() {
        assert_eq!(Labels::of_all(vec![Labels::Enum(s(&["a", "b"])), Labels::Enum(s(&["b", "c"]))]), Some(Labels::Enum(s(&["a", "b", "c"]))));
        assert_eq!(Labels::of_all(vec![Labels::Set(s(&["x"])), Labels::Set(s(&["y"]))]), Some(Labels::Set(s(&["x", "y"]))));
        assert_eq!(Labels::of_all(vec![Labels::Enum(s(&["a"])), Labels::Set(s(&["a"]))]), None);
        assert_eq!(Labels::of_all(Vec::new()), None);
    }

    #[test]
    fn a_reference_is_a_bare_or_qualified_column() {
        let expr = |sql: &str| match sqlparser::parser::Parser::parse_sql(&sqlparser::dialect::MySqlDialect {}, &format!("select {sql}")).unwrap().pop() {
            Some(sqlparser::ast::Statement::Query(q)) => match *q.body {
                SetExpr::Select(s) => match s.projection.into_iter().next() {
                    Some(SelectItem::UnnamedExpr(e)) => e,
                    other => panic!("{other:?}"),
                },
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
        assert_eq!(reference(&expr("role")), Some((None, "role")));
        assert_eq!(reference(&expr("x.role")), Some((Some("x"), "role")));
        assert_eq!(reference(&expr("(x.role)")), Some((Some("x"), "role")));
        assert_eq!(reference(&expr("app.x.role")), None);
        assert_eq!(reference(&expr("concat(role)")), None);
    }
}
