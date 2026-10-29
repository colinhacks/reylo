//! Which result columns of a DuckDB statement are never NULL, read off a `sqlparser` tree of it
//! with the catalog's NOT NULL columns. DuckDB's own describe says nothing of NULL that holds (it
//! marks a left-joined column NOT NULL and `count(*)` nullable), so the engine proves each column
//! itself, and a column it cannot prove may be NULL.
//!
//! A column is never NULL when it reads a NOT NULL column of a table no outer join pads, or of
//! one a filter shows was matched; when it is a literal, a parameter that takes no NULL, a
//! `count`, or an expression DuckDB computes from such values without making NULL of them
//! (arithmetic but division, comparisons, `||`, `coalesce` with one such argument, a `case`
//! whose every branch is one); and an aggregate over such a value in a query grouped by keys.
//! A division may be NULL, since `SET error_on_division_by_zero = false` makes a division by
//! zero NULL rather than an error. What a WHERE or an inner join's ON compares, tests for NULL
//! or matches is not NULL past it, and a table it shows was matched is not padded.
//!
//! The analysis gives each result column the name it expects DuckDB to give it where it can (a
//! column's own name, an alias), and the caller holds those against the describe's: where they
//! differ, the tree is not the statement DuckDB read, and nothing is proven.

use crate::catalog::Catalog;
use sqlparser::ast::{
    BinaryOperator, CastKind, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, GroupByExpr, Ident, JoinConstraint, JoinOperator, LimitClause, ObjectName, ObjectNamePart, Query, Select, SelectItem, SetExpr, SetOperator,
    TableAlias, TableFactor, TableWithJoins, UnaryOperator, Value,
};
use std::collections::HashSet;

/// What the analysis reads besides the tree: the catalog, and the parameters the caller may pass
/// NULL for, by the number `$n` binds them at.
pub struct Ctx<'a> {
    pub catalog: &'a Catalog,
    pub nullable_params: &'a HashSet<String>,
}

/// One result column: the name DuckDB gives it where the tree says (an alias, a column's own
/// name), whether it is never NULL, and the comment of the table column it reads as it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Out {
    pub name: Option<String>,
    pub not_null: bool,
    pub comment: Option<String>,
}

/// The WITH queries in scope, innermost last: each one's name, lowercased, and its columns.
type Ctes = Vec<(String, Vec<Out>)>;

/// A relation of a FROM: the name a column is qualified with, its columns where the engine knows
/// them, and whether an outer join pads it with NULL.
#[derive(Debug, Clone)]
struct Rel {
    name: Option<String>,
    columns: Option<Vec<Out>>,
    padded: bool,
}

/// The relations a SELECT reads, and what its filters show of them.
#[derive(Debug, Clone, Default)]
struct Scope {
    rels: Vec<Rel>,
    /// The columns a filter shows are not NULL, as (relation, column).
    rejected: HashSet<(usize, usize)>,
    /// Grouped by ROLLUP, CUBE or GROUPING SETS, which make a key NULL in the rows that total it.
    grouping_sets: bool,
    /// Grouped by keys (GROUP BY with any key, or ALL), so that each group has a row.
    grouped: bool,
}

impl Scope {
    /// The relation and column a reference names: `c` where exactly one relation has a column of
    /// that name and every relation's columns are known, `r.c` (or `s.r.c`) where the relation is
    /// named. Anything else (a struct's field, a lambda's parameter, a column of a table function)
    /// is none.
    fn resolve(&self, parts: &[Ident]) -> Option<(usize, usize)> {
        let same = |a: &str, b: &Ident| a.eq_ignore_ascii_case(&b.value);
        match parts {
            [column] => {
                if self.rels.iter().any(|r| r.columns.is_none()) {
                    return None;
                }
                let mut found = None;
                for (i, rel) in self.rels.iter().enumerate() {
                    for (j, c) in rel.columns.as_ref()?.iter().enumerate() {
                        if c.name.as_deref().is_some_and(|n| same(n, column)) {
                            if found.is_some() {
                                return None;
                            }
                            found = Some((i, j));
                        }
                    }
                }
                found
            }
            [.., rel, column] => {
                // `a.b.c` is a schema's table's column only where `a` names no relation and no
                // column here: else DuckDB reads it as a struct's field, `c` of the struct `b`.
                if parts.len() > 2 {
                    let first = &parts[0];
                    let named = self.rels.iter().any(|r| r.name.as_deref().is_some_and(|n| same(n, first)) || r.columns.as_ref().is_none_or(|cs| cs.iter().any(|c| c.name.as_deref().is_some_and(|n| same(n, first)))));
                    if named {
                        return None;
                    }
                }
                let mut hits = self.rels.iter().enumerate().filter(|(_, r)| r.name.as_deref().is_some_and(|n| same(n, rel)));
                let (i, r) = hits.next()?;
                if hits.next().is_some() {
                    return None;
                }
                let columns = r.columns.as_ref()?;
                let mut matching = columns.iter().enumerate().filter(|(_, c)| c.name.as_deref().is_some_and(|n| same(n, column)));
                let (j, _) = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                Some((i, j))
            }
            [] => None,
        }
    }

    fn column(&self, (i, j): (usize, usize)) -> Out {
        let rel = &self.rels[i];
        let c = rel.columns.as_ref().map(|cs| cs[j].clone()).unwrap_or(Out { name: None, not_null: false, comment: None });
        let not_null = !self.grouping_sets && (self.rejected.contains(&(i, j)) || (!rel.padded && c.not_null));
        Out { not_null, ..c }
    }
}

/// The result columns of a query, or `None` where the engine cannot read them (a construct it does
/// not follow, a `*` over a relation whose columns it does not know).
pub fn query(q: &Query, ctx: &Ctx, ctes: &Ctes) -> Option<Vec<Out>> {
    let mut ctes = ctes.clone();
    if let Some(with) = &q.with {
        for cte in &with.cte_tables {
            // A recursive query's rows come from itself too: its columns are named by its first
            // part, and each may be NULL.
            let columns = if with.recursive {
                let first = match &*cte.query.body {
                    SetExpr::SetOperation { left, .. } => set_expr(left, ctx, &ctes),
                    _ => query(&cte.query, ctx, &ctes),
                };
                first.map(|cs| cs.into_iter().map(|c| Out { not_null: false, comment: None, ..c }).collect())
            } else {
                query(&cte.query, ctx, &ctes)
            };
            let columns = columns.map(|cs| renamed(cs, &cte.alias)).unwrap_or_default();
            ctes.push((cte.alias.name.value.to_lowercase(), columns));
        }
    }
    set_expr(&q.body, ctx, &ctes)
}

/// Columns renamed by an alias's column list (`as t(a, b)`), the rest keeping their names.
fn renamed(mut columns: Vec<Out>, alias: &TableAlias) -> Vec<Out> {
    for (c, name) in columns.iter_mut().zip(&alias.columns) {
        c.name = Some(name.name.value.clone());
    }
    columns
}

fn set_expr(body: &SetExpr, ctx: &Ctx, ctes: &Ctes) -> Option<Vec<Out>> {
    match body {
        SetExpr::Select(s) => select(s, ctx, ctes),
        SetExpr::Query(q) => query(q, ctx, ctes),
        SetExpr::SetOperation { op, set_quantifier, left, right } => {
            // `UNION BY NAME` matches columns by name, which the engine does not follow.
            if format!("{set_quantifier}").to_ascii_uppercase().contains("BY NAME") {
                return None;
            }
            let (l, r) = (set_expr(left, ctx, ctes)?, set_expr(right, ctx, ctes)?);
            if l.len() != r.len() {
                return None;
            }
            Some(
                l.into_iter()
                    .zip(r)
                    .map(|(a, b)| Out {
                        not_null: match op {
                            SetOperator::Union => a.not_null && b.not_null,
                            // Each row is one of the left's, and of the right's too.
                            SetOperator::Intersect => a.not_null || b.not_null,
                            SetOperator::Except | SetOperator::Minus => a.not_null,
                        },
                        comment: None,
                        name: a.name,
                    })
                    .collect(),
            )
        }
        SetExpr::Values(values) => {
            let scope = Scope::default();
            let width = values.rows.first()?.len();
            if values.rows.iter().any(|row| row.len() != width) {
                return None;
            }
            // DuckDB names a VALUES list's columns col0, col1, …
            Some((0..width).map(|k| Out { name: Some(format!("col{k}")), not_null: values.rows.iter().all(|row| not_null(&row[k], &scope, ctx)), comment: None }).collect())
        }
        _ => None,
    }
}

fn select(s: &Select, ctx: &Ctx, ctes: &Ctes) -> Option<Vec<Out>> {
    let mut scope = from(&s.from, ctx, ctes)?;
    if let Some(selection) = &s.selection {
        reject(selection, &mut scope);
    }
    let g = grouping(s, &|e| match e {
        Expr::Identifier(id) => scope.resolve(std::slice::from_ref(id)).is_some(),
        Expr::CompoundIdentifier(ids) => scope.resolve(ids).is_some(),
        _ => false,
    });
    scope.grouped = g.keyed;
    scope.grouping_sets = g.sets;
    // `FROM t` alone selects every column.
    let star = [SelectItem::Wildcard(Default::default())];
    let items = if s.projection.is_empty() { &star[..] } else { &s.projection[..] };
    projection(items, &scope, ctx)
}

/// The columns a write's RETURNING returns, over the row it wrote to the table `name` (as written),
/// called `alias` in the statement: its NOT NULL columns are not NULL.
pub fn returning(items: &[SelectItem], name: &[String], alias: Option<String>, ctx: &Ctx) -> Option<Vec<Out>> {
    let table = ctx.catalog.table(name)?;
    let columns = table.columns.iter().map(|c| Out { name: Some(c.name.clone()), not_null: c.not_null, comment: c.comment.clone() }).collect();
    let scope = Scope { rels: vec![Rel { name: alias.or_else(|| name.last().cloned()), columns: Some(columns), padded: false }], ..Scope::default() };
    projection(items, &scope, ctx)
}

/// The columns a select list (or a RETURNING) selects in `scope`.
fn projection(items: &[SelectItem], scope: &Scope, ctx: &Ctx) -> Option<Vec<Out>> {
    let mut out = Vec::new();
    for item in items {
        match item {
            SelectItem::UnnamedExpr(e) => out.push(column_of(e, None, scope, ctx)),
            SelectItem::ExprWithAlias { expr, alias } => out.push(column_of(expr, Some(&alias.value), scope, ctx)),
            SelectItem::Wildcard(options) => {
                for i in 0..scope.rels.len() {
                    out.extend(expand(scope, i, options, ctx)?);
                }
            }
            SelectItem::QualifiedWildcard(kind, options) => {
                let sqlparser::ast::SelectItemQualifiedWildcardKind::ObjectName(name) = kind else { return None };
                let last = parts(name).pop()?;
                let mut hits = scope.rels.iter().enumerate().filter(|(_, r)| r.name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(&last)));
                let (i, _) = hits.next()?;
                if hits.next().is_some() {
                    return None;
                }
                out.extend(expand(scope, i, options, ctx)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

/// The columns `*` (or `r.*`) selects of relation `i`, its EXCLUDE left out and its REPLACE read.
fn expand(scope: &Scope, i: usize, options: &sqlparser::ast::WildcardAdditionalOptions, ctx: &Ctx) -> Option<Vec<Out>> {
    if options.opt_ilike.is_some() || options.opt_except.is_some() || options.opt_rename.is_some() || options.opt_alias.is_some() {
        return None;
    }
    let excluded: Vec<String> = match &options.opt_exclude {
        None => Vec::new(),
        Some(sqlparser::ast::ExcludeSelectItem::Single(name)) => vec![parts(name).pop()?.to_lowercase()],
        Some(sqlparser::ast::ExcludeSelectItem::Multiple(names)) => names.iter().map(|n| parts(n).pop().map(|p| p.to_lowercase())).collect::<Option<_>>()?,
    };
    let columns = scope.rels[i].columns.as_ref()?;
    let mut out = Vec::new();
    for (j, c) in columns.iter().enumerate() {
        let name = c.name.clone()?;
        if excluded.contains(&name.to_lowercase()) {
            continue;
        }
        let replaced = options.opt_replace.as_ref().and_then(|r| r.items.iter().find(|item| item.column_name.value.eq_ignore_ascii_case(&name)));
        match replaced {
            Some(item) => out.push(Out { name: Some(name), not_null: not_null(&item.expr, scope, ctx), comment: None }),
            None => out.push(Out { name: Some(name), ..scope.column((i, j)) }),
        }
    }
    Some(out)
}

/// A result column of an expression: its name (the alias, else a column's own), whether it is
/// never NULL, and the comment of the table column it reads as it is.
fn column_of(e: &Expr, alias: Option<&str>, scope: &Scope, ctx: &Ctx) -> Out {
    let read = match e {
        Expr::Identifier(id) => scope.resolve(std::slice::from_ref(id)),
        Expr::CompoundIdentifier(ids) => scope.resolve(ids),
        _ => None,
    };
    let own_name = match e {
        Expr::Identifier(id) => Some(id.value.clone()),
        Expr::CompoundIdentifier(ids) if read.is_some() => ids.last().map(|i| i.value.clone()),
        _ => None,
    };
    let name = alias.map(String::from).or(own_name);
    match read {
        Some(at) => Out { name, ..scope.column(at) },
        None => Out { name, not_null: not_null(e, scope, ctx), comment: None },
    }
}

/// How a SELECT groups its rows.
struct Grouping {
    /// Grouped by keys, so that each group has a row.
    keyed: bool,
    /// Grouped by ROLLUP, CUBE or GROUPING SETS.
    sets: bool,
    /// Not grouped at all: an aggregate over every row, which returns one row even over none.
    ungrouped: bool,
}

/// How `s` groups its rows. `GROUP BY ALL` groups by each select item without an aggregate that
/// reads a column, so it is keyed where such an item reads one `column` says is (outside a
/// subquery, with no lambda whose parameter it may be), and not grouped where no item without
/// an aggregate reads anything: a literal, a parameter or `random()` is no key.
fn grouping(s: &Select, column: &dyn Fn(&Expr) -> bool) -> Grouping {
    match &s.group_by {
        GroupByExpr::All(modifiers) => {
            let (mut keyed, mut ungrouped) = (false, true);
            for item in &s.projection {
                match item {
                    SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => {
                        if has_aggregate(e) {
                            continue;
                        }
                        let r = reads(e, column);
                        keyed |= r.column && !r.lambda;
                        ungrouped &= !r.column && !r.other_name && !r.query && !r.lambda;
                    }
                    _ => {
                        keyed = true;
                        ungrouped = false;
                    }
                }
            }
            Grouping { keyed, sets: !modifiers.is_empty(), ungrouped: ungrouped && modifiers.is_empty() }
        }
        GroupByExpr::Expressions(exprs, modifiers) => {
            let sets = !modifiers.is_empty() || exprs.iter().any(|e| matches!(e, Expr::Rollup(_) | Expr::Cube(_) | Expr::GroupingSets(_)));
            // `GROUP BY ()` is the one empty grouping set: no grouping at all.
            let keys = exprs.iter().any(|e| !matches!(e, Expr::Tuple(t) if t.is_empty()));
            Grouping { keyed: keys, sets, ungrouped: !keys && !sets }
        }
    }
}

/// What an expression reads outside any subquery: a name `column` says is a column, a name it
/// does not, and whether it holds a subquery or a lambda.
struct Reads {
    column: bool,
    other_name: bool,
    query: bool,
    lambda: bool,
}

fn reads(e: &Expr, column: &dyn Fn(&Expr) -> bool) -> Reads {
    struct Reader<'f> {
        depth: usize,
        column: &'f dyn Fn(&Expr) -> bool,
        reads: Reads,
    }
    impl sqlparser::ast::Visitor for Reader<'_> {
        type Break = ();
        fn pre_visit_query(&mut self, _: &Query) -> std::ops::ControlFlow<()> {
            self.depth += 1;
            self.reads.query = true;
            std::ops::ControlFlow::Continue(())
        }
        fn post_visit_query(&mut self, _: &Query) -> std::ops::ControlFlow<()> {
            self.depth -= 1;
            std::ops::ControlFlow::Continue(())
        }
        fn pre_visit_expr(&mut self, e: &Expr) -> std::ops::ControlFlow<()> {
            match e {
                Expr::Identifier(_) | Expr::CompoundIdentifier(_) if self.depth == 0 => {
                    if (self.column)(e) {
                        self.reads.column = true;
                    } else {
                        self.reads.other_name = true;
                    }
                }
                Expr::Lambda(_) => self.reads.lambda = true,
                _ => {}
            }
            std::ops::ControlFlow::Continue(())
        }
    }
    let mut reader = Reader { depth: 0, column, reads: Reads { column: false, other_name: false, query: false, lambda: false } };
    let _ = sqlparser::ast::Visit::visit(e, &mut reader);
    reader.reads
}

/// The relations of a FROM, joined, with what each ON shows; `None` for a join the engine does
/// not follow.
fn from(items: &[TableWithJoins], ctx: &Ctx, ctes: &Ctes) -> Option<Scope> {
    let mut scope = Scope::default();
    for item in items {
        scope.rels.extend(factor(&item.relation, ctx, ctes)?);
        for join in &item.joins {
            let rels = factor(&join.relation, ctx, ctes)?;
            let constraint = match &join.join_operator {
                JoinOperator::Join(c) | JoinOperator::Inner(c) => {
                    scope.rels.extend(rels);
                    // A join with no condition is no join DuckDB has: the tree misread the statement.
                    if matches!(c, JoinConstraint::None) {
                        return None;
                    }
                    Some(c)
                }
                JoinOperator::CrossJoin(_) => {
                    scope.rels.extend(rels);
                    None
                }
                JoinOperator::Left(_) | JoinOperator::LeftOuter(_) => {
                    scope.rels.extend(rels.into_iter().map(|r| Rel { padded: true, ..r }));
                    None
                }
                JoinOperator::Right(_) | JoinOperator::RightOuter(_) => {
                    // What an earlier ON showed of the left no longer holds where it is padded.
                    scope.rejected.clear();
                    for r in &mut scope.rels {
                        r.padded = true;
                    }
                    scope.rels.extend(rels);
                    None
                }
                JoinOperator::FullOuter(_) => {
                    scope.rejected.clear();
                    for r in &mut scope.rels {
                        r.padded = true;
                    }
                    scope.rels.extend(rels.into_iter().map(|r| Rel { padded: true, ..r }));
                    None
                }
                // A semi or anti join keeps the left's rows and none of the right's columns.
                JoinOperator::Semi(_) | JoinOperator::LeftSemi(_) | JoinOperator::Anti(_) | JoinOperator::LeftAnti(_) => None,
                _ => return None,
            };
            if let Some(JoinConstraint::On(on)) = constraint {
                reject(on, &mut scope);
            }
            // `USING` and `NATURAL` merge columns, which a `*` then selects once.
            if matches!(constraint, Some(JoinConstraint::Using(_) | JoinConstraint::Natural)) {
                return None;
            }
        }
    }
    Some(scope)
}

/// The relations one FROM item brings: a table, a view or a WITH query by its name, a subquery,
/// or joins in parentheses; a table function's columns are unknown.
fn factor(f: &TableFactor, ctx: &Ctx, ctes: &Ctes) -> Option<Vec<Rel>> {
    // A word DuckDB reads as a join's kind, which the tree took for an alias (`t positional join u`).
    const JOIN_WORDS: &[&str] = &["positional", "asof", "anti", "semi", "natural", "lateral"];
    let alias_name = |alias: &Option<TableAlias>| alias.as_ref().map(|a| a.name.value.clone());
    if let Some(alias) = match f {
        TableFactor::Table { alias, .. } | TableFactor::Derived { alias, .. } | TableFactor::Function { alias, .. } | TableFactor::TableFunction { alias, .. } | TableFactor::UNNEST { alias, .. } | TableFactor::NestedJoin { alias, .. } => alias.as_ref(),
        _ => None,
    } {
        if JOIN_WORDS.contains(&alias.name.value.to_ascii_lowercase().as_str()) && alias.name.quote_style.is_none() {
            return None;
        }
    }
    match f {
        TableFactor::Table { name, alias, args, .. } => {
            let parts = parts(name);
            let rel_name = alias_name(alias).or_else(|| parts.last().cloned());
            if args.is_some() {
                return Some(vec![Rel { name: rel_name, columns: None, padded: false }]);
            }
            let cte = match parts.as_slice() {
                [one] => ctes.iter().rev().find(|(n, _)| n.eq_ignore_ascii_case(one)).map(|(_, cs)| cs.clone()),
                _ => None,
            };
            let columns = match cte {
                Some(columns) => Some(columns),
                None => ctx.catalog.table(&parts).map(|t| t.columns.iter().map(|c| Out { name: Some(c.name.clone()), not_null: c.not_null, comment: c.comment.clone() }).collect()),
            };
            let columns = match (columns, alias) {
                (Some(cs), Some(alias)) => Some(renamed(cs, alias)),
                (cs, _) => cs,
            };
            Some(vec![Rel { name: rel_name, columns, padded: false }])
        }
        TableFactor::Derived { subquery, alias, .. } => {
            let columns = query(subquery, ctx, ctes).map(|cs| cs.into_iter().map(|c| Out { comment: None, ..c }).collect::<Vec<_>>());
            let columns = match (columns, alias) {
                (Some(cs), Some(alias)) => Some(renamed(cs, alias)),
                (cs, _) => cs,
            };
            Some(vec![Rel { name: alias_name(alias), columns, padded: false }])
        }
        TableFactor::NestedJoin { table_with_joins, alias } => {
            if alias.is_some() {
                return Some(vec![Rel { name: alias_name(alias), columns: None, padded: false }]);
            }
            Some(from(std::slice::from_ref(table_with_joins), ctx, ctes)?.rels)
        }
        TableFactor::Function { alias, .. } | TableFactor::TableFunction { alias, .. } | TableFactor::UNNEST { alias, .. } => Some(vec![Rel { name: alias_name(alias), columns: None, padded: false }]),
        _ => None,
    }
}

/// An object name's parts, unquoted.
pub fn parts(name: &ObjectName) -> Vec<String> {
    name.0
        .iter()
        .map(|p| match p {
            ObjectNamePart::Identifier(i) => i.value.clone(),
            other => other.to_string(),
        })
        .collect()
}

/// Marks what a filter shows is not NULL: each column one of its conjuncts compares, matches or
/// tests not to be NULL; and a padded relation such a column is of is matched, so not padded.
fn reject(filter: &Expr, scope: &mut Scope) {
    let mut found = Vec::new();
    rejected_by(filter, &mut found);
    for e in found {
        let at = match e {
            Expr::Identifier(id) => scope.resolve(std::slice::from_ref(id)),
            Expr::CompoundIdentifier(ids) => scope.resolve(ids),
            _ => None,
        };
        if let Some((i, j)) = at {
            scope.rejected.insert((i, j));
            scope.rels[i].padded = false;
        }
    }
}

/// The column references a predicate is NULL for when they are, collected from its AND-ed parts:
/// a comparison's operands, what a LIKE, an IN list or a BETWEEN tests, an `IS NOT NULL`'s.
fn rejected_by<'e>(e: &'e Expr, found: &mut Vec<&'e Expr>) {
    match e {
        Expr::Nested(inner) => rejected_by(inner, found),
        Expr::BinaryOp { left, op: BinaryOperator::And, right } => {
            rejected_by(left, found);
            rejected_by(right, found);
        }
        Expr::BinaryOp { left, op, right } if matches!(op, BinaryOperator::Eq | BinaryOperator::NotEq | BinaryOperator::Lt | BinaryOperator::LtEq | BinaryOperator::Gt | BinaryOperator::GtEq) => {
            strict_refs(left, found);
            strict_refs(right, found);
        }
        Expr::IsNotNull(inner) | Expr::IsTrue(inner) | Expr::IsFalse(inner) => strict_refs(inner, found),
        Expr::UnaryOp { op: UnaryOperator::Not, expr } => rejected_by_not(expr, found),
        Expr::Like { expr, pattern, .. } | Expr::ILike { expr, pattern, .. } | Expr::SimilarTo { expr, pattern, .. } => {
            strict_refs(expr, found);
            strict_refs(pattern, found);
        }
        Expr::InList { expr, .. } => strict_refs(expr, found),
        Expr::Between { expr, negated, low, high } => {
            strict_refs(expr, found);
            // `x NOT BETWEEN lo AND hi` is `x < lo OR x > hi`, true where one bound is NULL.
            if !negated {
                strict_refs(low, found);
                strict_refs(high, found);
            }
        }
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => found.push(e),
        _ => {}
    }
}

/// The column references `NOT e` is NULL for when they are. Negated, a predicate keeps only what
/// it is NULL for whenever an operand is: a comparison's operands, what a LIKE, an IN list or a
/// BETWEEN tests, a column; `NOT (x IS NULL)` is `x IS NOT NULL`. A predicate that is true or
/// false for NULL (`IS NOT NULL`, `IS TRUE`, a BETWEEN's bounds, an AND or OR) is true negated
/// where it was false, so it shows nothing.
fn rejected_by_not<'e>(e: &'e Expr, found: &mut Vec<&'e Expr>) {
    match e {
        Expr::Nested(inner) => rejected_by_not(inner, found),
        Expr::IsNull(inner) => strict_refs(inner, found),
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => strict_refs(e, found),
        Expr::BinaryOp { left, op, right } if matches!(op, BinaryOperator::Eq | BinaryOperator::NotEq | BinaryOperator::Lt | BinaryOperator::LtEq | BinaryOperator::Gt | BinaryOperator::GtEq) => {
            strict_refs(left, found);
            strict_refs(right, found);
        }
        Expr::Like { expr, pattern, .. } | Expr::ILike { expr, pattern, .. } | Expr::SimilarTo { expr, pattern, .. } => {
            strict_refs(expr, found);
            strict_refs(pattern, found);
        }
        Expr::InList { expr, .. } | Expr::Between { expr, .. } => strict_refs(expr, found),
        _ => {}
    }
}

/// The column references an expression is NULL for: itself, or through a cast, a sign or
/// arithmetic that is NULL for a NULL operand.
fn strict_refs<'e>(e: &'e Expr, found: &mut Vec<&'e Expr>) {
    match e {
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => found.push(e),
        Expr::Nested(inner) | Expr::Collate { expr: inner, .. } => strict_refs(inner, found),
        Expr::Cast { kind, expr, .. } if matches!(kind, CastKind::Cast | CastKind::DoubleColon) => strict_refs(expr, found),
        Expr::UnaryOp { op: UnaryOperator::Minus | UnaryOperator::Plus, expr } => strict_refs(expr, found),
        Expr::BinaryOp { left, op, right } if matches!(op, BinaryOperator::Plus | BinaryOperator::Minus | BinaryOperator::Multiply | BinaryOperator::StringConcat) => {
            strict_refs(left, found);
            strict_refs(right, found);
        }
        _ => {}
    }
}

/// The aggregate functions DuckDB has, by name: a call of one without OVER makes a query
/// aggregate.
pub const AGGREGATES: &[&str] = &[
    "count", "count_star", "count_if", "countif", "sum", "sum_no_overflow", "fsum", "sumkahan", "kahan_sum", "avg", "mean", "favg", "min", "max", "first", "last", "any_value", "arbitrary", "list", "array_agg",
    "string_agg", "group_concat", "listagg", "bool_and", "bool_or", "bit_and", "bit_or", "bit_xor", "bitstring_agg", "product", "median", "mode", "quantile", "quantile_cont", "quantile_disc", "percentile_cont",
    "percentile_disc", "approx_quantile", "approx_count_distinct", "stddev", "stddev_pop", "stddev_samp", "variance", "var_pop", "var_samp", "corr", "covar_pop", "covar_samp", "regr_avgx", "regr_avgy",
    "regr_count", "regr_intercept", "regr_r2", "regr_slope", "regr_sxx", "regr_sxy", "regr_syy", "arg_min", "arg_max", "argmin", "argmax", "min_by", "max_by", "histogram", "entropy", "kurtosis",
    "kurtosis_pop", "skewness", "geomean", "geometric_mean", "weighted_avg", "json_group_array", "json_group_object", "json_group_structure", "map_agg", "mad", "reservoir_quantile",
];

/// The aggregates whose value is never NULL, over any rows.
const COUNTS: &[&str] = &["count", "count_star", "approx_count_distinct", "regr_count"];

/// The aggregates that are NULL only where their input is (or there is none): over a NOT NULL
/// argument in a group, never.
const OVER_VALUES: &[&str] = &["count_if", "countif", "sum", "fsum", "sumkahan", "kahan_sum", "avg", "mean", "favg", "min", "max", "first", "last", "any_value", "arbitrary", "list", "array_agg", "string_agg", "group_concat", "listagg", "bool_and", "bool_or", "product", "median", "mode", "histogram"];

/// The window functions that number rows, never NULL. Not `ntile`, which is NULL for a NULL
/// number of buckets.
const RANKS: &[&str] = &["row_number", "rank", "dense_rank", "percent_rank", "cume_dist"];

/// Functions that are never NULL whatever their arguments.
const NEVER_NULL: &[&str] = &["current_date", "current_timestamp", "now", "get_current_timestamp", "transaction_timestamp", "random", "uuid", "gen_random_uuid", "uuidv4", "uuidv7", "version", "current_database", "current_schema", "list_value", "list_pack", "struct_pack", "row", "grouping", "grouping_id"];

/// Functions that are never NULL where every argument is not: DuckDB's scalar functions of a
/// value that make NULL only of NULL. Not a part of a date or a distance between two (`year`,
/// `extract`, `epoch`, `date_diff`, `date_sub`, `age`), which is NULL for an infinite one.
const STRICT: &[&str] = &[
    "abs", "ceil", "ceiling", "floor", "round", "trunc", "sign", "sqrt", "cbrt", "exp", "ln", "log", "log2", "log10", "pow", "power", "lower", "upper", "lcase", "ucase", "length", "len", "strlen", "char_length",
    "character_length", "octet_length", "bit_length", "trim", "ltrim", "rtrim", "lpad", "rpad", "left", "right", "substr", "substring", "replace", "reverse", "repeat", "concat_ws", "md5", "sha1", "sha256", "hash",
    "contains", "starts_with", "ends_with", "prefix", "suffix", "strpos", "instr", "position", "date_trunc", "datetrunc", "strftime", "to_timestamp", "make_date", "make_time",
    "make_timestamp", "date_add", "map", "to_json", "json_quote", "list_contains", "array_contains", "list_has", "len", "array_length", "cardinality", "list_sort", "list_reverse_sort", "list_distinct", "list_concat", "array_concat",
    "string_split", "str_split", "regexp_replace", "regexp_matches", "to_hex", "hex", "bin", "ascii", "chr", "unicode", "format", "printf", "greatest", "least", "isnan", "isinf", "isfinite",
];

/// Whether an expression is never NULL in `scope`.
fn not_null(e: &Expr, scope: &Scope, ctx: &Ctx) -> bool {
    let nn = |e: &Expr| not_null(e, scope, ctx);
    match e {
        Expr::Identifier(id) => scope.resolve(std::slice::from_ref(id)).is_some_and(|at| scope.column(at).not_null),
        Expr::CompoundIdentifier(ids) => scope.resolve(ids).is_some_and(|at| scope.column(at).not_null),
        Expr::Value(v) => match &v.value {
            Value::Null => false,
            Value::Placeholder(p) => p.strip_prefix('$').is_some_and(|n| !ctx.nullable_params.contains(n)),
            _ => true,
        },
        Expr::TypedString(_) | Expr::Interval(_) => true,
        Expr::Nested(inner) | Expr::Collate { expr: inner, .. } => nn(inner),
        Expr::Cast { kind, expr, .. } => matches!(kind, CastKind::Cast | CastKind::DoubleColon) && nn(expr),
        Expr::UnaryOp { expr, .. } => nn(expr),
        Expr::BinaryOp { left, op, right } => match op {
            // A division by zero is NULL where the session says so (`error_on_division_by_zero`).
            BinaryOperator::Divide | BinaryOperator::Modulo | BinaryOperator::DuckIntegerDivide | BinaryOperator::MyIntegerDivide => false,
            BinaryOperator::Plus
            | BinaryOperator::Minus
            | BinaryOperator::Multiply
            | BinaryOperator::StringConcat
            | BinaryOperator::Eq
            | BinaryOperator::NotEq
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
            | BinaryOperator::And
            | BinaryOperator::Or
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor => nn(left) && nn(right),
            _ => false,
        },
        Expr::IsNull(_) | Expr::IsNotNull(_) | Expr::IsTrue(_) | Expr::IsNotTrue(_) | Expr::IsFalse(_) | Expr::IsNotFalse(_) | Expr::IsUnknown(_) | Expr::IsNotUnknown(_) | Expr::IsDistinctFrom(..) | Expr::IsNotDistinctFrom(..) => true,
        Expr::InList { expr, list, .. } => nn(expr) && list.iter().all(nn),
        Expr::Between { expr, low, high, .. } => nn(expr) && nn(low) && nn(high),
        Expr::Like { expr, pattern, escape_char, .. } | Expr::ILike { expr, pattern, escape_char, .. } | Expr::SimilarTo { expr, pattern, escape_char, .. } => escape_char.is_none() && nn(expr) && nn(pattern),
        Expr::Case { conditions, else_result, .. } => else_result.as_deref().is_some_and(nn) && conditions.iter().all(|w| nn(&w.result)),
        Expr::Exists { .. } => true,
        Expr::Array(_) | Expr::Struct { .. } | Expr::Dictionary(_) | Expr::Map(_) | Expr::Tuple(_) => true,
        Expr::Ceil { expr, .. } | Expr::Floor { expr, .. } => nn(expr),
        Expr::Trim { expr, trim_what, trim_characters, .. } => nn(expr) && trim_what.as_deref().is_none_or(nn) && trim_characters.as_ref().is_none_or(|cs| cs.iter().all(nn)),
        Expr::Position { expr, r#in } => nn(expr) && nn(r#in),
        Expr::Substring { expr, substring_from, substring_for, .. } => nn(expr) && substring_from.as_deref().is_none_or(nn) && substring_for.as_deref().is_none_or(nn),
        Expr::AtTimeZone { timestamp, time_zone } => nn(timestamp) && nn(time_zone),
        Expr::Function(f) => function(f, scope, ctx),
        _ => false,
    }
}

/// A function's arguments as expressions, `None` where one is not (a `*`, a named argument).
fn args(f: &Function) -> Option<Vec<&Expr>> {
    match &f.args {
        FunctionArguments::None => Some(Vec::new()),
        FunctionArguments::List(list) => list
            .args
            .iter()
            .map(|a| match a {
                FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => Some(e),
                _ => None,
            })
            .collect(),
        FunctionArguments::Subquery(_) => None,
    }
}

/// The function's name, lowercased, where it is DuckDB's own: unqualified, or in `main`'s
/// built-ins as DuckDB names them.
pub fn builtin(f: &Function) -> Option<String> {
    match parts(&f.name).as_slice() {
        [name] => Some(name.to_lowercase()),
        _ => None,
    }
}

fn function(f: &Function, scope: &Scope, ctx: &Ctx) -> bool {
    let Some(name) = builtin(f) else { return false };
    let name = name.as_str();
    // A ranking function given an ORDER BY of its own, as in `ntile(2 ORDER BY id) OVER (…)`, ranks
    // within the window's frame, and is NULL where the frame is empty.
    let framed = matches!(&f.args, FunctionArguments::List(list) if !list.clauses.is_empty());
    if COUNTS.contains(&name) || (f.over.is_some() && !framed && RANKS.contains(&name)) || NEVER_NULL.contains(&name) {
        return true;
    }
    let Some(args) = args(f) else { return false };
    let nn = |e: &&Expr| not_null(e, scope, ctx);
    match name {
        "coalesce" | "ifnull" | "nvl" => args.iter().any(nn),
        "ntile" => f.over.is_some() && !framed && !args.is_empty() && args.iter().all(nn),
        _ if AGGREGATES.contains(&name) => {
            // Over the rows of a group, which has one at least; a FILTER or a window frame may
            // leave it none.
            f.over.is_none() && f.filter.is_none() && scope.grouped && !scope.grouping_sets && OVER_VALUES.contains(&name) && !args.is_empty() && args.iter().all(nn)
        }
        _ if f.over.is_none() && f.filter.is_none() && STRICT.contains(&name) => !args.is_empty() && args.iter().all(nn),
        _ => false,
    }
}

/// Whether an expression calls a function `is` picks, outside any subquery: a subquery's calls are
/// its own rows'.
fn calls(e: &Expr, is: &dyn Fn(&Function) -> bool) -> bool {
    struct Finder<'f> {
        depth: usize,
        is: &'f dyn Fn(&Function) -> bool,
    }
    impl sqlparser::ast::Visitor for Finder<'_> {
        type Break = ();
        fn pre_visit_query(&mut self, _: &Query) -> std::ops::ControlFlow<()> {
            self.depth += 1;
            std::ops::ControlFlow::Continue(())
        }
        fn post_visit_query(&mut self, _: &Query) -> std::ops::ControlFlow<()> {
            self.depth -= 1;
            std::ops::ControlFlow::Continue(())
        }
        fn pre_visit_expr(&mut self, e: &Expr) -> std::ops::ControlFlow<()> {
            match e {
                Expr::Function(f) if self.depth == 0 && (self.is)(f) => std::ops::ControlFlow::Break(()),
                _ => std::ops::ControlFlow::Continue(()),
            }
        }
    }
    sqlparser::ast::Visit::visit(e, &mut Finder { depth: 0, is }).is_break()
}

/// Whether an expression calls an aggregate (not as a window function), outside any subquery.
pub fn has_aggregate(e: &Expr) -> bool {
    calls(e, &|f| f.over.is_none() && builtin(f).is_some_and(|n| AGGREGATES.contains(&n.as_str())))
}

/// Whether an expression calls `unnest`, or a macro that does, which makes a row of each element,
/// outside any subquery. A qualified call is a user's macro's, by its last part.
pub fn has_unnest(e: &Expr, makers: &HashSet<String>) -> bool {
    calls(e, &|f| parts(&f.name).last().is_some_and(|n| makers.contains(&n.to_lowercase())))
}

/// How many rows a query returns, and why not one where it is not proven so.
pub fn cardinality(q: &Query, ctx: &Ctx) -> (&'static str, Option<String>) {
    cardinality_in(q, ctx, &[])
}

/// How many rows a query returns where the WITH queries `ctes` (lowercased) are in scope.
fn cardinality_in(q: &Query, ctx: &Ctx, ctes: &[String]) -> (&'static str, Option<String>) {
    let mut ctes = ctes.to_vec();
    if let Some(with) = &q.with {
        ctes.extend(with.cte_tables.iter().map(|c| c.alias.name.value.to_lowercase()));
    }
    let (mut card, mut reason) = body_cardinality(&q.body, ctx, &ctes);
    let literal = |e: &Expr| match e {
        Expr::Value(v) => match &v.value {
            Value::Number(n, _) => n.parse::<u64>().ok(),
            _ => None,
        },
        _ => None,
    };
    let (limit, offset) = match &q.limit_clause {
        Some(LimitClause::LimitOffset { limit, offset, .. }) => (limit.as_ref().map(|l| literal(l)), offset.as_ref().map(|o| literal(&o.value))),
        Some(LimitClause::OffsetCommaLimit { .. }) => (Some(None), None),
        None => (None, None),
    };
    let fetch = q.fetch.as_ref().map(|f| f.quantity.as_ref().and_then(literal).or(if f.quantity.is_none() { Some(1) } else { None }));
    let at_most_one = matches!(limit, Some(Some(0 | 1))) || matches!(fetch, Some(Some(0 | 1)));
    if card == "many" && at_most_one {
        card = "maybeOne";
        reason = None;
    }
    // An offset may skip the one row, and a limit of 0 returns none: so may a limit that is no
    // integer literal (a parameter, an expression, `0.4`, which DuckDB rounds to 0).
    let may_be_zero = |l: Option<Option<u64>>| matches!(l, Some(None | Some(0)));
    if card == "one" && (offset.is_some() || may_be_zero(limit) || may_be_zero(fetch)) {
        card = "maybeOne";
        reason = Some("its OFFSET or LIMIT may leave out the one row".into());
    }
    (card, reason)
}

fn body_cardinality(body: &SetExpr, ctx: &Ctx, ctes: &[String]) -> (&'static str, Option<String>) {
    match body {
        SetExpr::Query(q) => cardinality_in(q, ctx, ctes),
        SetExpr::Values(v) if v.rows.len() == 1 => ("one", None),
        SetExpr::Select(s) => select_cardinality(s, ctx, ctes),
        SetExpr::SetOperation { .. } => ("many", Some("a set operation returns the rows of both sides".into())),
        _ => ("many", None),
    }
}

fn select_cardinality(s: &Select, ctx: &Ctx, ctes: &[String]) -> (&'static str, Option<String>) {
    let items: Vec<&Expr> = s
        .projection
        .iter()
        .filter_map(|i| match i {
            SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => Some(e),
            _ => None,
        })
        .collect();
    if items.iter().any(|e| has_unnest(e, &ctx.catalog.row_makers)) {
        return ("many", Some("unnest makes a row of each element".into()));
    }
    let filtered = s.selection.is_some() || s.having.is_some() || s.qualify.is_some();
    // Any name may be a column here, so a GROUP BY ALL is taken as no grouping only where no item
    // without an aggregate reads a name at all.
    let grouped = grouping(s, &|_| true);
    let ungrouped = grouped.ungrouped;
    // Grouping sets make a row of each set, even over one row: `grouping sets ((), ())` is two.
    if grouped.sets {
        return ("many", Some("grouping sets make a row of each set".into()));
    }
    if s.from.is_empty() {
        return if filtered { ("maybeOne", Some("its WHERE, HAVING or QUALIFY may leave out the one row".into())) } else { ("one", None) };
    }
    let aggregate = items.iter().any(|e| has_aggregate(e)) || s.having.as_ref().is_some_and(has_aggregate);
    if aggregate && ungrouped {
        return if s.having.is_some() || s.qualify.is_some() { ("maybeOne", Some("its HAVING or QUALIFY may leave out the one row".into())) } else { ("one", None) };
    }
    // One table, a key of which the WHERE holds equal to one value each; not a WITH query that
    // takes a table's name, which has none of its keys.
    if let ([item], Some(selection)) = (s.from.as_slice(), &s.selection) {
        if item.joins.is_empty() {
            if let TableFactor::Table { name, alias, args: None, .. } = &item.relation {
                let shadowed = matches!(parts(name).as_slice(), [one] if ctes.contains(&one.to_lowercase()));
                if let Some(table) = ctx.catalog.table(&parts(name)).filter(|_| !shadowed) {
                    let qualifier = alias.as_ref().map(|a| a.name.value.clone()).or_else(|| parts(name).pop());
                    if key_equal(selection, &table.keys, qualifier.as_deref()) {
                        return ("maybeOne", None);
                    }
                }
            }
        }
    }
    ("many", Some("nothing bounds its rows to one".into()))
}

/// Whether the AND-ed parts of `filter` hold every column of one of `keys` equal to a value that
/// reads no column: a literal or a parameter, cast or not.
pub fn key_equal(filter: &Expr, keys: &[Vec<String>], qualifier: Option<&str>) -> bool {
    let mut equal: Vec<String> = Vec::new();
    fn conjuncts<'e>(e: &'e Expr, out: &mut Vec<&'e Expr>) {
        match e {
            Expr::Nested(inner) => conjuncts(inner, out),
            Expr::BinaryOp { left, op: BinaryOperator::And, right } => {
                conjuncts(left, out);
                conjuncts(right, out);
            }
            other => out.push(other),
        }
    }
    fn constant(e: &Expr) -> bool {
        match e {
            Expr::Value(v) => !matches!(v.value, Value::Null),
            Expr::TypedString(_) => true,
            Expr::Nested(inner) => constant(inner),
            Expr::Cast { kind, expr, .. } => matches!(kind, CastKind::Cast | CastKind::DoubleColon) && constant(expr),
            Expr::UnaryOp { op: UnaryOperator::Minus | UnaryOperator::Plus, expr } => constant(expr),
            _ => false,
        }
    }
    let column = |e: &Expr| -> Option<String> {
        match e {
            Expr::Identifier(id) => Some(id.value.to_lowercase()),
            Expr::CompoundIdentifier(ids) if ids.len() >= 2 => {
                let rel = &ids[ids.len() - 2].value;
                qualifier.filter(|q| q.eq_ignore_ascii_case(rel)).map(|_| ids[ids.len() - 1].value.to_lowercase())
            }
            Expr::Nested(inner) => match inner.as_ref() {
                Expr::Identifier(id) => Some(id.value.to_lowercase()),
                _ => None,
            },
            _ => None,
        }
    };
    let mut parts = Vec::new();
    conjuncts(filter, &mut parts);
    for p in parts {
        if let Expr::BinaryOp { left, op: BinaryOperator::Eq, right } = p {
            match (column(left), column(right)) {
                (Some(c), None) if constant(right) => equal.push(c),
                (None, Some(c)) if constant(left) => equal.push(c),
                _ => {}
            }
        }
    }
    keys.iter().any(|key| key.iter().all(|k| equal.contains(k)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{self, Table, TableColumn};
    use sqlparser::ast::Statement;
    use sqlparser::dialect::DuckDbDialect;
    use sqlparser::parser::Parser;

    fn table(name: &str, columns: &[(&str, bool)], keys: &[&[&str]]) -> Table {
        Table {
            database: "app".into(),
            schema: "main".into(),
            name: name.into(),
            current: true,
            current_database: true,
            columns: columns.iter().map(|(c, nn)| TableColumn { name: c.to_string(), not_null: *nn, comment: None }).collect(),
            keys: keys.iter().map(|k| k.iter().map(|c| c.to_string()).collect()).collect(),
        }
    }

    fn catalog() -> Catalog {
        catalog::of(vec![table("users", &[("id", true), ("name", true), ("email", false), ("team_id", false)], &[&["id"], &["name"]]), table("teams", &[("id", true), ("title", true)], &[&["id"]])])
    }

    fn query(sql: &str) -> Query {
        match Parser::parse_sql(&DuckDbDialect {}, sql).unwrap().pop().unwrap() {
            Statement::Query(q) => *q,
            other => panic!("{other:?}"),
        }
    }

    /// Each column's name and whether the engine proves it never NULL, `-` where it does not read them.
    fn proven(sql: &str) -> String {
        let catalog = catalog();
        let nullable = HashSet::from(["2".to_string()]);
        let ctx = Ctx { catalog: &catalog, nullable_params: &nullable };
        match super::query(&query(sql), &ctx, &Vec::new()) {
            None => "-".into(),
            Some(out) => out.iter().map(|o| format!("{}{}", o.name.as_deref().unwrap_or("_"), if o.not_null { "" } else { "?" })).collect::<Vec<_>>().join(" "),
        }
    }

    fn rows(sql: &str) -> &'static str {
        let catalog = catalog();
        let nullable = HashSet::new();
        super::cardinality(&query(sql), &Ctx { catalog: &catalog, nullable_params: &nullable }).0
    }

    #[test]
    fn a_column_is_not_null_where_its_table_says_so_and_nothing_pads_it() {
        assert_eq!(proven("select id, name, email from users"), "id name email?");
        assert_eq!(proven("select * from users"), "id name email? team_id?");
        assert_eq!(proven("select u.*, t.title from users u join teams t on t.id = u.team_id"), "id name email? team_id title");
        assert_eq!(proven("select u.name, t.title from users u left join teams t on t.id = u.team_id"), "name title?");
        assert_eq!(proven("select u.name, t.title from users u left join teams t on t.id = u.team_id where t.id > 0"), "name title");
        assert_eq!(proven("select u.name, t.title from users u full join teams t on t.id = u.team_id"), "name? title?");
        // Three parts name a schema's table's column, unless the first names a relation or a
        // column here, when they are a struct's field.
        assert_eq!(proven("select main.users.name, app.main.users.id from users"), "name id");
        assert_eq!(proven("select teams.users.id as x from users, teams"), "x?");
        // A right join pads the left, whatever an earlier ON showed of it.
        assert_eq!(proven("select u.team_id from users u join teams t on t.id = u.team_id right join teams x on x.id = t.id"), "team_id?");
        assert_eq!(proven("select email from users where email is not null and name like 'a%'"), "email");
        assert_eq!(proven("select email from users where email is not null or id = 1"), "email?");
        assert_eq!(proven("select * exclude (email) replace (lower(name) as name) from users"), "id name team_id?");
        assert_eq!(proven("from users select id"), "id");
        assert_eq!(proven("from users"), "id name email? team_id?");
        // Negated, a predicate shows only what it is NULL for whenever it is.
        assert_eq!(proven("select email from users where not (email is not null)"), "email?");
        assert_eq!(proven("select email from users where not (not (email is null))"), "email?");
        assert_eq!(proven("select email from users where not (email is true)"), "email?");
        assert_eq!(proven("select email, team_id from users where id not between team_id and 10 and not (email like 'a%')"), "email team_id?");
        assert_eq!(proven("select team_id from users where not (5 between team_id and id)"), "team_id?");
        assert_eq!(proven("select team_id from users where team_id not between 1 and 2"), "team_id");
    }

    #[test]
    fn an_expression_is_not_null_where_duckdb_makes_no_null_of_its_values() {
        assert_eq!(proven("select id + 1 as a, id / 2 as b, id // 2 as c, id % 2 as d, -id as e from users"), "a b? c? d? e");
        assert_eq!(proven("select name || '!' as a, email || '!' as b, coalesce(email, name) as c, lower(name) as d, upper(email) as e from users"), "a b? c d e?");
        assert_eq!(proven("select case when id > 1 then name else 'x' end as a, case when id > 1 then name end as b from users"), "a b?");
        assert_eq!(proven("select email is null as a, id in (1, 2) as b, cast(id as varchar) as c, try_cast(name as integer) as d from users"), "a b c d?");
        assert_eq!(proven("select 1 as a, null as b, 'x' as c, [1, 2] as d, {'k': 1} as e, $1 as f, $2 as g"), "a b? c d e f g?");
        assert_eq!(proven("select year(current_date) as a, now() as b from users"), "a? b");
        assert_eq!(proven("select (select max(id) from teams) as a, exists (select 1 from teams) as b from users"), "a? b");
        assert_eq!(proven("select count(*) as a, count(email) as b, sum(id) as c, max(name) as d from users"), "a b c? d?");
        assert_eq!(proven("select team_id, count(*) as a, sum(id) as c, max(email) as d, max(name) filter (where id > 1) as e from users group by team_id"), "team_id? a c d? e?");
        assert_eq!(proven("select team_id, sum(id) as c from users group by grouping sets ((team_id), ())"), "team_id? c?");
        assert_eq!(proven("select row_number() over () as a, sum(id) over () as b, lag(id) over () as c from users"), "a b? c?");
        assert_eq!(proven("select ntile(2) over () as a, ntile(team_id) over () as b, map([id], [name]) as c, date_sub('day', current_date, current_date) as d from users"), "a b? c d?");
        // GROUP BY ALL groups by the items without an aggregate that read a column, and by nothing
        // where none does.
        assert_eq!(proven("select name, sum(id) as s from users group by all"), "name s");
        assert_eq!(proven("select sum(id) as s from users group by all"), "s?");
        assert_eq!(proven("select 1 as k, $1 as p, random() as r, (select 1) as q, sum(id) as s from users group by all"), "k p r q? s?");
        assert_eq!(proven("select list_transform([1], id -> id + 1) as l, sum(id) as s from users group by all"), "l? s?");
        assert_eq!(proven("select sum(id) as s from users group by ()"), "s?");
    }

    #[test]
    fn a_query_s_columns_follow_its_subqueries_with_queries_and_set_operations() {
        assert_eq!(proven("with x as (select id, email from users) select * from x"), "id email?");
        assert_eq!(proven("with x(a, b) as (select id, email from users) select a, b from x"), "a b?");
        assert_eq!(proven("select s.id, s.n from (select id, name as n from users) s"), "id n");
        assert_eq!(proven("select id from users union select id from teams"), "id");
        assert_eq!(proven("select id from users union all select null"), "id?");
        assert_eq!(proven("values (1, 'a'), (2, null)"), "col0 col1?");
        assert_eq!(proven("with recursive r(n) as (select 1 union all select n + 1 from r where n < 3) select n from r"), "n?");
        // What the engine does not follow, it proves nothing of.
        assert_eq!(proven("select * from users join teams using (id)"), "-");
        assert_eq!(proven("select * from read_parquet('x.parquet')"), "-");
        assert_eq!(proven("select a from read_parquet('x.parquet')"), "a?");
        assert_eq!(proven("select id from users positional join teams"), "-");
    }

    #[test]
    fn rows_are_bounded_by_a_key_an_aggregate_or_a_limit() {
        assert_eq!(rows("select * from users where id = $1"), "maybeOne");
        assert_eq!(rows("select * from users u where u.name = 'a' and u.email = $1"), "maybeOne");
        assert_eq!(rows("select * from users where email = $1"), "many");
        assert_eq!(rows("select * from users where id = $1 or id = 2"), "many");
        assert_eq!(rows("select * from users u join teams t on t.id = u.team_id where u.id = $1"), "many");
        assert_eq!(rows("select count(*) from users"), "one");
        assert_eq!(rows("select count(*) from users having count(*) > 1"), "maybeOne");
        assert_eq!(rows("select team_id, count(*) from users group by team_id"), "many");
        assert_eq!(rows("select count(*) from users group by all"), "one");
        assert_eq!(rows("select 1 as k, count(*) from users group by all"), "one");
        assert_eq!(rows("select team_id, count(*) from users group by all"), "many");
        assert_eq!(rows("select current_date as k, count(*) from users group by all"), "one");
        assert_eq!(rows("select team_id + 1 as k, count(*) from users group by all"), "many");
        assert_eq!(rows("select count(*) from users group by ()"), "one");
        // A WITH query that takes a table's name has none of its keys.
        assert_eq!(rows("with users as (select * from users union all select * from users) select id from users where id = 1"), "many");
        assert_eq!(rows("with x as (select 1) select id from users where id = 1"), "maybeOne");
        assert_eq!(rows("select 1"), "one");
        assert_eq!(rows("select unnest([1, 2])"), "many");
        // Each grouping set is a row of its own, even with no FROM.
        assert_eq!(rows("select count(*) group by grouping sets ((), ())"), "many");
        // A limit that is no integer literal may be 0.
        assert_eq!(rows("select count(*) from users limit $1"), "maybeOne");
        assert_eq!(rows("select count(*) from users limit 0.4"), "maybeOne");
        assert_eq!(rows("select count(*) from users fetch first $1 rows only"), "maybeOne");
        assert_eq!(rows("select count(*) from users limit 1"), "one");
        // A macro that unnests makes rows as unnest does, by the names the catalog gives.
        let mut catalog = catalog();
        catalog.row_makers.insert("generate_subscripts".into());
        let nullable = HashSet::new();
        let ctx = Ctx { catalog: &catalog, nullable_params: &nullable };
        assert_eq!(super::cardinality(&query("select generate_subscripts([4, 5, 6], 1)"), &ctx).0, "many");
        assert_eq!(super::cardinality(&query("select main.generate_subscripts([4, 5, 6], 1)"), &ctx).0, "many");
        // A ranking function given its own ORDER BY ranks within the frame, which may be empty.
        assert_eq!(proven("select ntile(2 order by id) over (order by id rows between 3 following and 1 following) as a, row_number() over () as c from users"), "a? c");
        assert_eq!(rows("select count(*) from users group by rollup (name)"), "many");
        // A subquery's aggregate or unnest is its own rows'.
        assert_eq!(rows("select id, (select max(id) from teams) as top from users"), "many");
        assert_eq!(rows("select 1 as one, (select count(*) from (select unnest([1, 2]))) as n"), "one");
        assert_eq!(rows("select * from users limit 1"), "maybeOne");
        assert_eq!(rows("select count(*) from users offset 1"), "maybeOne");
        assert_eq!(rows("select * from users fetch first 1 rows only"), "maybeOne");
    }
}
