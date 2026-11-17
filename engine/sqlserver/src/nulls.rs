//! Result columns SQL Server calls nullable and that are never NULL. The server's describe keeps a
//! column's own NOT NULL through joins and nulls what an outer join null-extends, and the engine
//! takes every column it calls NOT NULL as it is; but it calls most computed columns nullable,
//! `count(*)`, `n + 1` and `upper(name)` among them. The engine narrows those by rules over a
//! `sqlparser` tree of a lone SELECT's list:
//!
//! - a column the statement reads is never NULL where the server says so of it read alone, in the
//!   statement's own FROM and WHERE: the engine describes that probe ([`Plan::probe`]), so an
//!   outer join's null-extended columns stay nullable;
//! - and where the WHERE's top-level conjuncts filter out its NULLs (`x = …`, `x is not null`,
//!   `x like …`, `x in (…)`, `x between …`);
//! - `count` and `count_big`, the ranking functions, `concat`, `isnumeric`, the clock and `newid`
//!   are never NULL; `isnull(a, b)` is not where either is not, `coalesce(…)` where one argument is
//!   not, `iif(c, a, b)` and a `case` with an `else` where every result is not;
//! - a cast, arithmetic and a string function that is NULL only on a NULL argument (`upper`,
//!   `substring`, `dateadd`, …) are not where their arguments are not. SQL Server raises an error,
//!   not a NULL, on a division by zero or an overflow under `ANSI_WARNINGS` and `ARITHABORT`, which
//!   are on in every session SQL Server's drivers open, tedious's included;
//! - an aggregate over a column that is not NULL is not NULL where a plain GROUP BY makes every
//!   group hold a row, and a windowed one where its frame holds the current row, as one with no
//!   frame clause does.
//!
//! Under `GROUP BY ROLLUP`, `CUBE`, `GROUPING SETS` or `ALL` a grouped column is NULL in the rows
//! that sum others up, and `ALL` brings back groups no row of the WHERE reached: there the engine
//! narrows nothing a column or an aggregate reads.

use sqlparser::ast::{BinaryOperator, CastKind, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, GroupByExpr, Query, Select, SelectItem, SetExpr, Statement, UnaryOperator, Value, WindowType};
use std::collections::HashSet;

/// What narrowing a statement needs from the server: the probe, and the columns it selects.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// The statement's FROM and WHERE selecting each column its result's expressions read, which
    /// the engine describes as it describes the statement.
    pub probe: Option<String>,
    /// The columns the probe selects, each as the statement writes it.
    pub refs: Vec<String>,
}

/// The lone SELECT the rules read: a query that is one SELECT with no wildcard, no `INTO` and no
/// `FOR JSON` or `FOR XML`, which make one column of every other.
fn lone_select(statement: &Statement) -> Option<(&Query, &Select)> {
    let Statement::Query(q) = statement else { return None };
    if q.for_clause.is_some() {
        return None;
    }
    let SetExpr::Select(s) = q.body.as_ref() else { return None };
    if s.into.is_some() || s.projection.iter().any(|i| matches!(i, SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..))) {
        return None;
    }
    Some((q, s))
}

/// How the SELECT groups its rows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Grouping {
    None,
    /// A plain GROUP BY of expressions: every group holds a row.
    Plain,
    /// ROLLUP, CUBE, GROUPING SETS or ALL: a grouped column may be NULL, and a group may hold no row.
    Other,
}

fn grouping(s: &Select) -> Grouping {
    match &s.group_by {
        GroupByExpr::All(_) => Grouping::Other,
        GroupByExpr::Expressions(exprs, _) if exprs.is_empty() => Grouping::None,
        GroupByExpr::Expressions(exprs, modifiers) => {
            // SQL Server's dialect reads `ROLLUP(…)` and `CUBE(…)` as calls.
            let special = |e: &Expr| match strip(e) {
                Expr::Rollup(_) | Expr::Cube(_) | Expr::GroupingSets(_) | Expr::Tuple(_) => true,
                Expr::Function(f) => matches!(name_of(f).as_str(), "rollup" | "cube" | "grouping"),
                _ => false,
            };
            if !modifiers.is_empty() || exprs.iter().any(special) {
                Grouping::Other
            } else {
                Grouping::Plain
            }
        }
    }
}

fn item_expr(item: &SelectItem) -> Option<&Expr> {
    match item {
        SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => Some(e),
        _ => None,
    }
}

/// The probe of a statement whose list the rules read: its FROM and WHERE selecting each column
/// the list's expressions read where a rule looks at it.
pub fn plan(statement: &Statement) -> Plan {
    let Some((q, s)) = lone_select(statement) else { return Plan::default() };
    if s.from.is_empty() {
        return Plan::default();
    }
    let g = grouping(s);
    let mut refs: Vec<String> = Vec::new();
    for item in &s.projection {
        if let Some(e) = item_expr(item) {
            never_null_by(e, g, &mut |r| {
                let key = r.to_string();
                if !refs.contains(&key) {
                    refs.push(key);
                }
                true
            });
        }
    }
    if refs.is_empty() || g == Grouping::Other {
        return Plan::default();
    }
    let mut probe_select = s.clone();
    probe_select.projection = refs.iter().map(|r| SelectItem::UnnamedExpr(parse_ref(r))).collect();
    probe_select.distinct = None;
    probe_select.top = None;
    probe_select.group_by = GroupByExpr::Expressions(Vec::new(), Vec::new());
    probe_select.having = None;
    probe_select.qualify = None;
    let mut probe = q.clone();
    probe.body = Box::new(SetExpr::Select(Box::new(probe_select)));
    probe.order_by = None;
    probe.limit_clause = None;
    probe.fetch = None;
    Plan { probe: Some(probe.to_string()), refs }
}

/// A column reference as the statement wrote it, back from its text.
fn parse_ref(text: &str) -> Expr {
    sqlparser::parser::Parser::new(&sqlparser::dialect::MsSqlDialect {})
        .try_with_sql(text)
        .and_then(|mut p| p.parse_expr())
        .unwrap_or_else(|_| Expr::Identifier(sqlparser::ast::Ident::new(text)))
}

/// Which result columns are never NULL by the rules, by position among `count` columns; none where
/// the statement is not one the rules read, or its list is not the result's.
pub fn never_null(statement: Option<&Statement>, plan: &Plan, probed: Option<&[bool]>, count: usize) -> Vec<bool> {
    let none = vec![false; count];
    let Some((_, s)) = statement.and_then(lone_select) else { return none };
    if s.projection.len() != count {
        return none;
    }
    let g = grouping(s);
    // Each column the probe described as not NULL.
    let mut known: HashSet<String> = HashSet::new();
    if let Some(probed) = probed.filter(|p| p.len() == plan.refs.len()) {
        known.extend(plan.refs.iter().zip(probed).filter(|(_, nullable)| !**nullable).map(|(r, _)| r.clone()));
    }
    if g != Grouping::Other {
        if let Some(w) = &s.selection {
            filtered(w, &mut known);
        }
    }
    s.projection
        .iter()
        .map(|item| match item_expr(item) {
            Some(e) if g != Grouping::Other => never_null_by(e, g, &mut |r| known.contains(&r.to_string())),
            Some(e) => never_null_by(e, g, &mut |_| false),
            None => false,
        })
        .collect()
}

/// The columns the WHERE's top-level conjuncts keep no NULL of: a row where one is NULL makes the
/// conjunct unknown, and the WHERE drops it.
fn filtered(e: &Expr, out: &mut HashSet<String>) {
    let column = |e: &Expr, out: &mut HashSet<String>| {
        if let Expr::Identifier(_) | Expr::CompoundIdentifier(_) = strip(e) {
            out.insert(strip(e).to_string());
        }
    };
    match e {
        Expr::Nested(inner) => filtered(inner, out),
        Expr::BinaryOp { left, op: BinaryOperator::And, right } => {
            filtered(left, out);
            filtered(right, out);
        }
        Expr::BinaryOp { left, op: BinaryOperator::Eq | BinaryOperator::NotEq | BinaryOperator::Lt | BinaryOperator::LtEq | BinaryOperator::Gt | BinaryOperator::GtEq, right } => {
            column(left, out);
            column(right, out);
        }
        Expr::IsNotNull(x) => column(x, out),
        Expr::Like { expr, pattern, .. } => {
            column(expr, out);
            column(pattern, out);
        }
        Expr::InList { expr, .. } | Expr::Between { expr, .. } => column(expr, out),
        Expr::InSubquery { expr, negated: false, .. } => column(expr, out),
        _ => {}
    }
}

fn strip(e: &Expr) -> &Expr {
    match e {
        Expr::Nested(inner) => strip(inner),
        _ => e,
    }
}

/// The functions that are never NULL.
const NEVER_NULL: &[&str] = &["count", "count_big", "concat", "isnumeric", "getdate", "getutcdate", "sysdatetime", "sysutcdatetime", "sysdatetimeoffset", "current_timestamp", "newid", "newsequentialid"];

/// The window functions that are never NULL: a row's rank or position in its partition.
const RANKING: &[&str] = &["row_number", "rank", "dense_rank", "ntile", "percent_rank", "cume_dist"];

/// The functions NULL only on a NULL argument: on any other they return a value or raise an error.
const STRICT: &[&str] = &[
    "upper", "lower", "ltrim", "rtrim", "trim", "reverse", "len", "datalength", "abs", "sign", "ceiling", "floor", "round", "replace", "substring", "left", "right", "charindex", "patindex", "year", "month", "day", "eomonth", "power", "square",
    "exp", "sqrt", "log", "log10", "datefromparts", "lower", "translate",
];

/// The functions whose first argument is a date part's keyword (`dateadd(day, …)`), which reads as
/// a column and is none.
const DATE_PART_FIRST: &[&str] = &["datepart", "datename", "dateadd", "datediff", "datediff_big", "datetrunc"];

/// The aggregates that are not NULL over a group of rows whose argument is never NULL.
const AGGREGATES: &[&str] = &["sum", "avg", "min", "max", "string_agg"];

/// A built-in function's name, lowercased. T-SQL calls a user's scalar function by its schema too
/// (`dbo.upper(…)`), so a name with a schema is none of the built-ins these rules know.
fn name_of(f: &Function) -> String {
    match f.name.0.as_slice() {
        [only] => only.to_string().trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase(),
        _ => String::new(),
    }
}

fn args(f: &Function) -> Vec<&Expr> {
    match &f.args {
        FunctionArguments::List(list) => list
            .args
            .iter()
            .filter_map(|a| match a {
                FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => Some(e),
                FunctionArg::Named { arg: FunctionArgExpr::Expr(e), .. } | FunctionArg::ExprNamed { arg: FunctionArgExpr::Expr(e), .. } => Some(e),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Whether `e` is never NULL, `column` saying it of each column it reads where a rule looks at
/// one. Every argument a rule reads is asked, even past one that decides it, so that a probe's
/// plan sees every column.
fn never_null_by(e: &Expr, g: Grouping, column: &mut dyn FnMut(&Expr) -> bool) -> bool {
    match e {
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => column(e),
        Expr::Value(v) => !matches!(v.value, Value::Null),
        Expr::Nested(inner) => never_null_by(inner, g, column),
        Expr::UnaryOp { op: UnaryOperator::Minus | UnaryOperator::Plus | UnaryOperator::BitwiseNot, expr } => never_null_by(expr, g, column),
        Expr::BinaryOp { left, op, right } if matches!(op, BinaryOperator::Plus | BinaryOperator::Minus | BinaryOperator::Multiply | BinaryOperator::Divide | BinaryOperator::Modulo | BinaryOperator::StringConcat | BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor) => {
            let (l, r) = (never_null_by(left, g, column), never_null_by(right, g, column));
            l && r
        }
        Expr::Cast { kind: CastKind::Cast, expr, .. } => never_null_by(expr, g, column),
        Expr::Convert { is_try: false, expr, styles, .. } => {
            let operand = never_null_by(expr, g, column);
            let styles: Vec<bool> = styles.iter().map(|s| never_null_by(s, g, column)).collect();
            operand && styles.iter().all(|s| *s)
        }
        Expr::Case { conditions, else_result: Some(otherwise), .. } => {
            let results: Vec<bool> = conditions.iter().map(|c| never_null_by(&c.result, g, column)).collect();
            let otherwise = never_null_by(otherwise, g, column);
            otherwise && results.iter().all(|r| *r)
        }
        Expr::Function(f) => function(f, g, column),
        Expr::Subquery(q) => one_count(q),
        _ => false,
    }
}

/// Whether a scalar subquery is a count over its rows with no `GROUP BY`, `HAVING`, `TOP`,
/// `OFFSET` or `FETCH`: it returns exactly one row, and a count is never NULL.
fn one_count(q: &Query) -> bool {
    let SetExpr::Select(s) = q.body.as_ref() else { return false };
    let grouped = !matches!(&s.group_by, GroupByExpr::Expressions(e, m) if e.is_empty() && m.is_empty());
    if grouped || s.having.is_some() || s.top.is_some() || q.limit_clause.is_some() || q.fetch.is_some() || q.for_clause.is_some() || s.projection.len() != 1 {
        return false;
    }
    match &s.projection[0] {
        SelectItem::UnnamedExpr(Expr::Function(f)) | SelectItem::ExprWithAlias { expr: Expr::Function(f), .. } => matches!(name_of(f).as_str(), "count" | "count_big") && f.over.is_none() && f.filter.is_none(),
        _ => false,
    }
}

fn function(f: &Function, g: Grouping, column: &mut dyn FnMut(&Expr) -> bool) -> bool {
    let name = name_of(f);
    let name = name.as_str();
    if f.filter.is_some() {
        return false;
    }
    if NEVER_NULL.contains(&name) {
        return true;
    }
    if RANKING.contains(&name) {
        return f.over.is_some();
    }
    let args = args(f);
    let mut each = |from: usize| -> Vec<bool> { args.iter().skip(from).map(|a| never_null_by(a, g, column)).collect() };
    match name {
        "isnull" | "coalesce" => {
            let each = each(0);
            each.iter().any(|a| *a)
        }
        "iif" if args.len() == 3 => {
            let each = each(1);
            each.iter().all(|a| *a)
        }
        _ if STRICT.contains(&name) && f.over.is_none() => {
            let each = each(0);
            !each.is_empty() && each.iter().all(|a| *a)
        }
        _ if DATE_PART_FIRST.contains(&name) && f.over.is_none() => {
            let each = each(1);
            !each.is_empty() && each.iter().all(|a| *a)
        }
        _ if AGGREGATES.contains(&name) => {
            // Over a frame that holds the current row, or a group that holds a row, the aggregate
            // reads at least one value.
            let holds_a_row = match &f.over {
                Some(WindowType::WindowSpec(spec)) => spec.window_frame.is_none() && spec.window_name.is_none(),
                Some(WindowType::NamedWindow(_)) => false,
                None => g == Grouping::Plain && f.within_group.is_empty(),
            };
            let each = each(0);
            holds_a_row && !each.is_empty() && each.iter().all(|a| *a)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::MsSqlDialect;
    use sqlparser::parser::Parser;

    fn parse(sql: &str) -> Statement {
        Parser::parse_sql(&MsSqlDialect {}, sql).unwrap().pop().unwrap()
    }

    #[test]
    fn plans_a_probe_of_the_columns_the_rules_read() {
        let s = parse("select top 5 upper(p.name) as n, count(*) as c, dateadd(day, 1, c.at) as d, coalesce(c.title, p.name) as t from parent p left join child c on c.parent_id = p.id where p.id > @p_1 group by p.name, c.at, c.title order by n");
        let plan = plan(&s);
        assert_eq!(plan.refs, ["p.name", "c.at", "c.title"]);
        let probe = plan.probe.as_deref().unwrap();
        assert!(probe.starts_with("SELECT p.name, c.at, c.title FROM parent"), "{probe}");
        assert!(probe.ends_with("WHERE p.id > @p_1") && !probe.contains("GROUP") && !probe.contains("TOP") && !probe.contains("ORDER"), "{probe}");
        // p.name is not NULL, c.at and c.title are; count(*) never is.
        assert_eq!(never_null(Some(&s), &plan, Some(&[false, true, true]), 4), [true, true, false, true]);
        // With no probe answered, only what needs no column holds.
        assert_eq!(never_null(Some(&s), &plan, None, 4), [false, true, false, false]);
    }

    #[test]
    fn a_where_conjunct_keeps_no_null_of_its_column() {
        let s = parse("select c.title + N'!' as t, c.at as a from parent p left join child c on c.parent_id = p.id where c.title like @p_1 and (c.at is not null or p.id = 1)");
        let plan = plan(&s);
        assert_eq!(never_null(Some(&s), &plan, Some(&[true, true]), 2), [true, false]);
    }

    #[test]
    fn aggregates_under_a_plain_group_by_alone() {
        let plain = parse("select max(n) as m, sum(n) over (partition by g) as w, sum(n) over (order by g rows between 1 following and 2 following) as f from t group by g, n");
        let plan_plain = plan(&plain);
        assert_eq!(never_null(Some(&plain), &plan_plain, Some(&[false]), 3), [true, true, false]);
        let total = parse("select max(n) as m from t");
        assert_eq!(never_null(Some(&total), &plan(&total), Some(&[false]), 1), [false]);
        let rollup = parse("select g, count(*) as c, max(n) as m from t where g = 1 group by rollup(g)");
        assert!(plan(&rollup).probe.is_none());
        assert_eq!(never_null(Some(&rollup), &plan(&rollup), None, 3), [false, true, false]);
    }

    #[test]
    fn a_literal_and_the_functions_that_need_no_column() {
        let s = parse("select 1 + 2 as a, null as b, newid() as c, concat(null, null) as d, row_number() over (order by (select 1)) as e, iif(1 = 1, 'a', null) as f, try_cast('x' as int) as g");
        assert!(plan(&s).probe.is_none());
        assert_eq!(never_null(Some(&s), &plan(&s), None, 7), [true, false, true, true, true, false, false]);
    }
}
