//! The columns of derived tables and WITH queries, which the server's flags for a statement say
//! too little and sometimes too much about (measured on 8.4):
//!
//! - Too much: the server merges a derived table, a WITH query or a view into the query that reads
//!   it where it can, and a merged expression keeps its own NOT NULL flag on the null-extended side
//!   of an outer join: `select d.n from users x left join (select id, id + 1 as n from users) d on
//!   d.id = x.id + 1000` flags `n` NOT NULL, and every row holds NULL in it. Materialized, the
//!   derived table is a temporary table on that side and its columns are flagged as such. So the
//!   flags are read from a copy of the statement whose first query block sets
//!   `derived_merge=off` for it ([`materialized`]), which materializes every derived table, WITH
//!   query and view without an ALGORITHM of its own. A view created with `ALGORITHM = MERGE` is
//!   merged whatever the hint says, NO_MERGE included, and lies alike, so the copy reads each view
//!   through a derived table of its own, `(SELECT * FROM view)`, which is materialized. The copy
//!   is only prepared, never run, so it drops the statement's own optimizer hints, which never
//!   change a value, and a MERGE among them would merge the table again.
//! - Too little: the server takes a derived table's flags from its own prepare of the table's
//!   query, without what the engine proves of that query. So the engine analyzes the query on its
//!   own ([`standalone`]), as the WITH queries it reads are in scope, and writes each column it
//!   proves never NULL and the server does not as `coalesce(column, filler)` ([`wrapper`]), which
//!   is the same value: the server's flags for the statement with those columns so written carry
//!   the proof through whatever reads the derived table, an outer join included.

use crate::api::MysqlColumn;
use sqlparser::ast::{Cte, JoinOperator, ObjectNamePart, OptimizerHint, OptimizerHintStyle, Query, Select, SelectItem, SetExpr, TableAlias, TableFactor, TableWithJoins, With};
use std::collections::HashSet;

/// The hint the copy's first query block carries: it holds for the whole statement.
const MATERIALIZE: &str = " SET_VAR(optimizer_switch = 'derived_merge=off') ";

/// The statement with every derived table, WITH query and view the server would merge
/// materialized instead, and with no optimizer hint of its own in the query blocks this walks;
/// `None` for a statement that reads no derived table and no view (`views`, in lower case), has
/// no WITH query and no outer join, which leaves nothing merged that a flag could be wrong about,
/// or one whose first query block the hint cannot go in.
pub fn materialized(q: &Query, views: &HashSet<String>) -> Option<Query> {
    let mut copy = q.clone();
    let mut merged = copy.with.is_some();
    each_block(&mut copy, &mut |select| {
        select.optimizer_hints.clear();
        merged |= select.from.iter().any(reads_merged);
        // A block that only selects every column of one table has nothing to null-extend the
        // view's columns with, and is the shape a view is read through.
        let alone = matches!(select.projection.as_slice(), [SelectItem::Wildcard(_)]) && matches!(select.from.as_slice(), [item] if item.joins.is_empty() && matches!(item.relation, TableFactor::Table { .. }));
        if !alone {
            for item in &mut select.from {
                merged |= views_through_derived(item, views);
            }
        }
    });
    if !merged {
        return None;
    }
    let first = first_block(&mut copy.body)?;
    first.optimizer_hints.push(OptimizerHint { prefix: String::new(), text: MATERIALIZE.to_string(), style: OptimizerHintStyle::MultiLine });
    Some(copy)
}

/// Whether a query's first query block carries the hint [`materialized`] adds.
pub fn is_materialized(q: &Query) -> bool {
    let mut body = q.body.as_ref();
    loop {
        match body {
            SetExpr::Select(select) => return select.optimizer_hints.iter().any(|h| h.text == MATERIALIZE),
            SetExpr::SetOperation { left, .. } => body = left,
            SetExpr::Query(q) => body = q.body.as_ref(),
            _ => return false,
        }
    }
}

/// The hint, as a probe's text writes it after its SELECT.
pub fn hint() -> String {
    format!("/*+{MATERIALIZE}*/ ")
}

fn first_block(body: &mut SetExpr) -> Option<&mut Select> {
    match body {
        SetExpr::Select(select) => Some(select),
        SetExpr::SetOperation { left, .. } => first_block(left),
        SetExpr::Query(q) => first_block(&mut q.body),
        _ => None,
    }
}

/// Read each view of a FROM item, inside its nested joins too, through a derived table of its
/// own, under the name it was read by; whether any was.
fn views_through_derived(item: &mut TableWithJoins, views: &HashSet<String>) -> bool {
    fn factor(t: &mut TableFactor, views: &HashSet<String>) -> bool {
        match t {
            TableFactor::NestedJoin { table_with_joins, .. } => views_through_derived(table_with_joins, views),
            TableFactor::Table { name, args: None, .. } => {
                let Some(ObjectNamePart::Identifier(last)) = name.0.last() else { return false };
                if !views.contains(&last.value.to_ascii_lowercase()) {
                    return false;
                }
                let last = last.clone();
                let Some(TableFactor::Derived { lateral, mut subquery, alias: Some(template), sample }) = derived_template() else { return false };
                let SetExpr::Select(select) = subquery.body.as_mut() else { return false };
                let mut view = t.clone();
                let alias = match &mut view {
                    TableFactor::Table { alias, .. } => alias.take(),
                    _ => None,
                };
                select.from[0].relation = view;
                let alias = alias.unwrap_or(TableAlias { name: last, ..template });
                *t = TableFactor::Derived { lateral, subquery, alias: Some(alias), sample };
                true
            }
            _ => false,
        }
    }
    let mut any = factor(&mut item.relation, views);
    for join in &mut item.joins {
        any |= factor(&mut join.relation, views);
    }
    any
}

/// `(SELECT * FROM t) AS t`, as the parser reads it.
fn derived_template() -> Option<TableFactor> {
    let mut statements = sqlparser::parser::Parser::parse_sql(&sqlparser::dialect::MySqlDialect {}, "SELECT * FROM (SELECT * FROM t) AS t").ok()?;
    let Some(sqlparser::ast::Statement::Query(q)) = statements.pop() else { return None };
    let SetExpr::Select(select) = *q.body else { return None };
    select.from.into_iter().next().map(|item| item.relation)
}

/// Whether a FROM item is or holds a derived table, or an outer join.
fn reads_merged(item: &TableWithJoins) -> bool {
    let factor = |f: &TableFactor| match f {
        TableFactor::Derived { .. } => true,
        TableFactor::NestedJoin { table_with_joins, .. } => reads_merged(table_with_joins),
        _ => false,
    };
    factor(&item.relation) || item.joins.iter().any(|j| !matches!(j.join_operator, JoinOperator::Join(_) | JoinOperator::Inner(_) | JoinOperator::CrossJoin(_) | JoinOperator::StraightJoin(_)) || factor(&j.relation))
}

/// Every query block of a query, its WITH queries' and its derived tables' included, at any depth
/// of them: not a subquery inside an expression.
fn each_block(q: &mut Query, f: &mut dyn FnMut(&mut Select)) {
    if let Some(with) = &mut q.with {
        for cte in &mut with.cte_tables {
            each_block(&mut cte.query, f);
        }
    }
    fn body(b: &mut SetExpr, f: &mut dyn FnMut(&mut Select)) {
        match b {
            SetExpr::Select(select) => {
                f(select);
                for item in &mut select.from {
                    from(item, f);
                }
            }
            SetExpr::SetOperation { left, right, .. } => {
                body(left, f);
                body(right, f);
            }
            SetExpr::Query(q) => each_block(q, f),
            _ => {}
        }
    }
    fn from(item: &mut TableWithJoins, f: &mut dyn FnMut(&mut Select)) {
        factor(&mut item.relation, f);
        for join in &mut item.joins {
            factor(&mut join.relation, f);
        }
    }
    fn factor(t: &mut TableFactor, f: &mut dyn FnMut(&mut Select)) {
        match t {
            TableFactor::Derived { subquery, .. } => each_block(subquery, f),
            TableFactor::NestedJoin { table_with_joins, .. } => from(table_with_joins, f),
            _ => {}
        }
    }
    body(&mut q.body, f);
}

/// A derived table or a WITH query whose columns the statement reads.
pub struct Target<'a> {
    pub query: &'a mut Query,
    /// The WITH queries in scope where it stands.
    pub scope: Option<With>,
    /// Whether a column list names its columns, which its select list then does not.
    pub listed: bool,
}

/// The derived tables of a query's FROM, through its set operations and nested joins, and its
/// WITH queries unless they are RECURSIVE, in the order the text has them. Not a LATERAL derived
/// table, which reads the tables before it and is no query of its own, nor the derived tables
/// inside these, which their own analysis reads.
pub fn each_target(q: &mut Query, f: &mut dyn FnMut(Target)) {
    let scope = q.with.clone();
    if let Some(with) = &mut q.with {
        if !with.recursive {
            let all: Vec<Cte> = with.cte_tables.clone();
            for (i, cte) in with.cte_tables.iter_mut().enumerate() {
                let earlier = (i > 0).then(|| With { with_token: with.with_token.clone(), recursive: false, cte_tables: all[..i].to_vec() });
                f(Target { listed: !cte.alias.columns.is_empty(), query: &mut cte.query, scope: earlier });
            }
        }
    }
    fn body(b: &mut SetExpr, scope: &Option<With>, f: &mut dyn FnMut(Target)) {
        match b {
            SetExpr::Select(select) => {
                for item in &mut select.from {
                    from(item, scope, f);
                }
            }
            SetExpr::SetOperation { left, right, .. } => {
                body(left, scope, f);
                body(right, scope, f);
            }
            // A parenthesized query with WITH queries of its own has another scope.
            SetExpr::Query(q) if q.with.is_none() => body(&mut q.body, scope, f),
            _ => {}
        }
    }
    fn from(item: &mut TableWithJoins, scope: &Option<With>, f: &mut dyn FnMut(Target)) {
        factor(&mut item.relation, scope, f);
        for join in &mut item.joins {
            factor(&mut join.relation, scope, f);
        }
    }
    fn factor(t: &mut TableFactor, scope: &Option<With>, f: &mut dyn FnMut(Target)) {
        match t {
            TableFactor::Derived { lateral: false, subquery, alias: Some(alias), .. } => f(Target { listed: !alias.columns.is_empty(), query: subquery, scope: scope.clone() }),
            TableFactor::NestedJoin { table_with_joins, .. } => from(table_with_joins, scope, f),
            _ => {}
        }
    }
    body(&mut q.body, &scope, f);
}

/// Whether a query is `SELECT * FROM <one table>` and nothing more, the shape [`materialized`]
/// reads a view through.
pub fn reads_one_table(q: &Query) -> bool {
    let SetExpr::Select(select) = q.body.as_ref() else { return false };
    match select.from.as_slice() {
        [item] if item.joins.is_empty() && matches!(item.relation, TableFactor::Table { .. }) => q.to_string() == format!("SELECT * FROM {}", item.relation),
        _ => false,
    }
}

/// A derived table's or a WITH query's query as a statement of its own: with the WITH queries in
/// scope where it stands, which it may read. `None` where those and its own WITH queries do not
/// make one WITH without changing what a name reads: where either is RECURSIVE.
pub fn standalone(q: &Query, scope: Option<&With>) -> Option<Query> {
    let mut q = q.clone();
    match (scope, &mut q.with) {
        (None, _) => {}
        (Some(scope), with @ None) => *with = Some(scope.clone()),
        (Some(scope), Some(own)) => {
            if scope.recursive || own.recursive {
                return None;
            }
            let mut ctes = scope.cte_tables.clone();
            ctes.append(&mut own.cte_tables);
            own.cte_tables = ctes;
        }
    }
    Some(q)
}

/// A value of a column's type, which `coalesce(column, filler)` gives the server's flag NOT NULL
/// to: the type of the coalesce may be wider, which no flag turns on. None for a type that has no
/// such value: JSON, whose coalesce the server flags nullable whatever its other argument, BIT,
/// a spatial type, a vector.
pub fn filler(c: &MysqlColumn) -> Option<&'static str> {
    match c.r#type {
        0 | 1 | 2 | 3 | 8 | 9 | 13 | 246 => Some("0"),
        4 | 5 => Some("0e0"),
        10 | 14 => Some("DATE '2000-01-01'"),
        7 | 12 => Some("TIMESTAMP '2000-01-01 00:00:00'"),
        11 => Some("TIME '00:00:00'"),
        15 | 249..=254 if c.charset == 63 => Some("X''"),
        15 | 249..=254 => Some("''"),
        _ => None,
    }
}

/// The text of a derived table's or a WITH query's query with the columns given a filler written
/// `coalesce(column, filler)`: the query as a derived table of its own, whose columns are read by
/// position, each item named as the column was (`columns`), or by position where a column list
/// names them.
pub fn wrapper(q: &Query, columns: &[(String, Option<&str>)], listed: bool) -> String {
    let items: Vec<String> = columns
        .iter()
        .enumerate()
        .map(|(i, (name, filler))| {
            let read = format!("reylo_derived.reylo_{i}");
            let value = match filler {
                Some(filler) => format!("coalesce({read}, {filler})"),
                None => read,
            };
            let alias = if listed { format!("reylo_{i}") } else { format!("`{}`", name.replace('`', "``")) };
            format!("{value} AS {alias}")
        })
        .collect();
    let list: Vec<String> = (0..columns.len()).map(|i| format!("reylo_{i}")).collect();
    format!("SELECT {} FROM ({q}) AS reylo_derived ({})", items.join(", "), list.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::ast::Statement;
    use sqlparser::dialect::MySqlDialect;

    fn query(sql: &str) -> Query {
        match sqlparser::parser::Parser::parse_sql(&MySqlDialect {}, sql).unwrap().pop() {
            Some(Statement::Query(q)) => *q,
            other => panic!("{other:?}"),
        }
    }

    fn copy(sql: &str) -> Option<String> {
        materialized(&query(sql), &HashSet::from(["recent".to_string()])).map(|q| q.to_string())
    }

    #[test]
    fn a_statement_that_may_merge_is_prepared_materialized() {
        let hint = "/*+ SET_VAR(optimizer_switch = 'derived_merge=off') */";
        assert_eq!(copy("select d.id from (select id from users) d").unwrap(), format!("SELECT {hint} d.id FROM (SELECT id FROM users) d"));
        assert_eq!(copy("with d as (select id from users) select id from d").unwrap(), format!("WITH d AS (SELECT id FROM users) SELECT {hint} id FROM d"));
        assert_eq!(copy("select u.id from users u left join posts p on p.user_id = u.id").unwrap(), format!("SELECT {hint} u.id FROM users u LEFT JOIN posts p ON p.user_id = u.id"));
        // The first query block of a set operation, and a derived table inside a nested join.
        assert!(copy("select id from (select id from users) d union select id from posts").unwrap().starts_with(&format!("SELECT {hint} id")));
        assert!(copy("select 1 from users u join (posts p join (select id from users) d on d.id = p.id) on p.user_id = u.id").is_some());
        // The statement's own hints are dropped, in every query block the copy walks.
        let own = copy("select /*+ MERGE(d) */ d.id from (select /*+ NO_MERGE() */ id from users) d").unwrap();
        assert_eq!(own.matches("/*+").count(), 1, "{own}");
        assert!(is_materialized(&materialized(&query("select d.id from (select id from users) d"), &HashSet::new()).unwrap()));
        // A view is read through a derived table of its own, under the name it was read by, in
        // every block but one that selects it alone.
        assert_eq!(copy("select r.id from recent r").unwrap(), format!("SELECT {hint} r.id FROM (SELECT * FROM recent) r"));
        assert_eq!(copy("select Recent.id from users u left join app.Recent on Recent.id = u.id").unwrap(), format!("SELECT {hint} Recent.id FROM users u LEFT JOIN (SELECT * FROM app.Recent) AS Recent ON Recent.id = u.id"));
        assert_eq!(copy("select d.id from (select * from recent) d").unwrap(), format!("SELECT {hint} d.id FROM (SELECT * FROM recent) d"));
        assert!(copy("select d.id from (select x.id from users x join recent y on y.id = x.id) d").unwrap().contains("JOIN (SELECT * FROM recent) y"));
        // Nothing merged, nothing to read otherwise.
        for sql in ["select id from users", "select u.id from users u join posts p on p.user_id = u.id", "select (select max(id) from (select id from posts) d) from users"] {
            assert_eq!(copy(sql), None, "{sql}");
        }
    }

    #[test]
    fn the_targets_are_the_derived_tables_and_with_queries_read() {
        let targets = |sql: &str| {
            let mut q = query(sql);
            let mut found = Vec::new();
            each_target(&mut q, &mut |t| found.push((t.query.to_string(), t.scope.map(|s| s.cte_tables.len()), t.listed)));
            found
        };
        assert_eq!(targets("select d.id from (select id from users) d"), [("SELECT id FROM users".to_string(), None, false)]);
        assert_eq!(targets("select d.a from (select id from users) d (a)"), [("SELECT id FROM users".to_string(), None, true)]);
        assert_eq!(
            targets("with a as (select 1 as x), b (y) as (select x from a) select * from b join (select y from b) d using (y) union select 2 from (select 3) e"),
            [
                ("SELECT 1 AS x".to_string(), None, false),
                ("SELECT x FROM a".to_string(), Some(1), true),
                ("SELECT y FROM b".to_string(), Some(2), false),
                ("SELECT 3".to_string(), Some(2), false),
            ]
        );
        // A nested derived table is its outer one's own to read; a LATERAL one reads the tables
        // before it; a RECURSIVE WITH query is not a query of its own.
        assert_eq!(targets("select * from (select * from (select 1) e) d").len(), 1);
        assert!(targets("select * from users u, lateral (select u.id) d").is_empty());
        assert_eq!(targets("with recursive r as (select 1 as n union all select n + 1 from r where n < 3) select n from r").len(), 0);
    }

    #[test]
    fn a_derived_query_stands_alone_with_the_with_queries_in_scope() {
        let alone = |sql: &str| {
            let mut q = query(sql);
            let mut found = Vec::new();
            each_target(&mut q, &mut |t| found.push(standalone(t.query, t.scope.as_ref()).map(|q| q.to_string())));
            found
        };
        assert_eq!(alone("with a as (select 1 as x) select d.x from (select x from a) d")[1].as_deref(), Some("WITH a AS (SELECT 1 AS x) SELECT x FROM a"));
        assert_eq!(alone("with a as (select 1 as x) select d.x from (with b as (select x from a) select x from b) d")[1].as_deref(), Some("WITH a AS (SELECT 1 AS x), b AS (SELECT x FROM a) SELECT x FROM b"));
        assert_eq!(alone("with recursive a as (select 1 as x) select d.x from (with b as (select x from a) select x from b) d")[0], None);
    }

    #[test]
    fn a_wrapper_writes_the_proven_columns_as_never_null() {
        let q = query("select id, (select count(*) from posts) as n from users");
        let columns = [("id".to_string(), None), ("n".to_string(), Some("0"))];
        assert_eq!(wrapper(&q, &columns, false), "SELECT reylo_derived.reylo_0 AS `id`, coalesce(reylo_derived.reylo_1, 0) AS `n` FROM (SELECT id, (SELECT count(*) FROM posts) AS n FROM users) AS reylo_derived (reylo_0, reylo_1)");
        assert!(wrapper(&q, &columns, true).contains("coalesce(reylo_derived.reylo_1, 0) AS reylo_1"));
        assert!(wrapper(&q, &[("a`b".to_string(), None)], false).contains("AS `a``b`"));
        let column = |r#type: u32, charset: u32| MysqlColumn { name: String::new(), r#type, flags: 0, charset, length: None, org_table: None, org_name: None };
        assert_eq!(filler(&column(8, 63)), Some("0"));
        assert_eq!(filler(&column(253, 255)), Some("''"));
        assert_eq!(filler(&column(252, 63)), Some("X''"));
        assert_eq!(filler(&column(245, 63)), None);
    }
}
