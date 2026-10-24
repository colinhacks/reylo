//! Whether a SELECT returns one row at most, from the unique keys of what it reads: tables, and
//! derived tables, WITH queries and views through the keys their own statements keep.
//!
//! One SELECT's FROM is flattened out of its joins into items and the equalities that may pin
//! one. Call an item **determined** when every row of the FROM carries the same row of it, or that
//! row's null extension; when every item is, the FROM has one row at most. The closure that finds
//! them, as the Postgres engine's:
//!
//! 1. A WHERE conjunct `r.k = v` for each column of a unique key of item `r`, where `v` reads no
//!    column of `r` and only columns of determined items (or no column), determines `r` in any
//!    join: the `=` rejects a null-extended row.
//! 2. The same equalities in the ON clause of an inner join determine either side, and of a LEFT
//!    JOIN its right side (RIGHT JOIN mirrors it). A FULL JOIN is refused.
//! 3. A derived table, a WITH query or a view that is one row at most is determined.
//! 4. A LATERAL derived table bounded so is determined once every other item is: it is computed
//!    anew for each row of the rest.
//!
//! A derived table's, a WITH query's or a view's unique keys are read from its statement
//! ([`output_keys`]), and pin it as a table's pin the table.
//!
//! An equality pins a key column only where it compares the column as it is: with a value of the
//! column's class ([`compares_as_is`]), and with a column only where both are numbers, since two
//! strings of different collations compare under one the server picks.

use crate::mysql::{compares_as_is, const_limit, deterministic, is_variable, one_before_limit, reads_column, select_of, value_class, Class, Keys};
use sqlparser::ast::{self, Distinct, Expr, GroupByExpr, JoinConstraint, JoinOperator, ObjectNamePart, Query, Select, SelectItem, SelectItemQualifiedWildcardKind, SetExpr, TableFactor, TableWithJoins};

/// The WITH queries in scope, outermost first: each name in lower case, its body (`None` for one
/// of a RECURSIVE WITH, or one that renames its columns), and how many of the entries before it
/// its body sees.
type Ctes<'a> = Vec<(String, Option<&'a Query>, usize)>;

/// `outer` with a query's own WITH queries after it.
fn ctes_of<'a>(q: &'a Query, outer: &[(String, Option<&'a Query>, usize)]) -> Ctes<'a> {
    let mut out: Ctes<'a> = outer.to_vec();
    if let Some(w) = &q.with {
        for c in &w.cte_tables {
            let body = (!w.recursive && c.alias.columns.is_empty()).then_some(c.query.as_ref());
            // A WITH query's body sees the ones before it, and with RECURSIVE itself too.
            let sees = out.len();
            out.push((c.alias.name.value.to_ascii_lowercase(), body, sees));
        }
    }
    out
}

/// One column of a key of a derived table's rows: its name in their output, in lower case, and
/// what it compares as.
#[derive(Debug, Clone)]
pub(crate) struct Part {
    name: String,
    class: Option<Class>,
}

type Key = Vec<Part>;

enum Source<'a> {
    /// A table, by the name the statement gives it.
    Table(String),
    /// A derived table, a WITH query or a view, with the WITH queries its statement sees.
    Query(&'a Query, Ctes<'a>),
    /// A LATERAL derived table.
    Lateral(&'a Query, Ctes<'a>),
    /// Anything the proof does not read: a table function, a recursive WITH query.
    Other,
}

struct Item<'a> {
    /// The name its columns go by, in lower case.
    alias: String,
    source: Source<'a>,
    /// Whether an outer join can null-extend it.
    nullable: bool,
    /// Whether its alias renames its columns.
    renamed: bool,
}

struct Conjunct<'a> {
    expr: &'a Expr,
    /// For an ON clause, the items it may pin; `None` for the WHERE, which may pin any.
    may_pin: Option<Vec<usize>>,
    /// Whether it holds of every row the FROM returns.
    inner: bool,
}

/// Where a column reference points.
enum Placed {
    Item(usize, String),
    /// A column of an enclosing query.
    Outside,
    Unknown,
}

struct From<'a> {
    items: Vec<Item<'a>>,
    conjuncts: Vec<Conjunct<'a>>,
    /// Whether a USING or NATURAL join merges columns under one bare name.
    merged: bool,
    refused: Option<&'static str>,
    keys: &'a Keys,
}

impl<'a> From<'a> {
    fn read(s: &'a Select, ctes: &[(String, Option<&'a Query>, usize)], keys: &'a Keys) -> From<'a> {
        let mut from = From { items: Vec::new(), conjuncts: Vec::new(), merged: false, refused: None, keys };
        for twj in &s.from {
            from.flatten(twj, ctes, false);
        }
        if let Some(w) = &s.selection {
            for expr in conjuncts(w) {
                from.conjuncts.push(Conjunct { expr, may_pin: None, inner: true });
            }
        }
        from
    }

    fn flatten(&mut self, twj: &'a TableWithJoins, ctes: &[(String, Option<&'a Query>, usize)], nullable: bool) {
        let start = self.items.len();
        self.factor(&twj.relation, ctes, nullable);
        for join in &twj.joins {
            enum Kind {
                Inner,
                Left,
                Right,
            }
            let (kind, constraint) = match &join.join_operator {
                JoinOperator::Join(c) | JoinOperator::Inner(c) | JoinOperator::CrossJoin(c) | JoinOperator::StraightJoin(c) => (Kind::Inner, c),
                JoinOperator::Left(c) | JoinOperator::LeftOuter(c) => (Kind::Left, c),
                JoinOperator::Right(c) | JoinOperator::RightOuter(c) => (Kind::Right, c),
                _ => {
                    self.refused = Some("a FULL JOIN returns a row for each side that matches nothing, and the engine reads no other join than inner, left and right");
                    return;
                }
            };
            let middle = self.items.len();
            self.factor(&join.relation, ctes, nullable || matches!(kind, Kind::Left));
            let end = self.items.len();
            if matches!(kind, Kind::Right) {
                for item in &mut self.items[start..middle] {
                    item.nullable = true;
                }
            }
            match constraint {
                JoinConstraint::On(on) => {
                    let (may_pin, inner): (Vec<usize>, bool) = match kind {
                        Kind::Inner => ((start..end).collect(), true),
                        Kind::Left => ((middle..end).collect(), false),
                        Kind::Right => ((start..middle).collect(), false),
                    };
                    for expr in conjuncts(on) {
                        self.conjuncts.push(Conjunct { expr, may_pin: Some(may_pin.clone()), inner });
                    }
                }
                // They equate columns the tree does not spell out, so they pin nothing here.
                JoinConstraint::Using(_) | JoinConstraint::Natural => self.merged = true,
                JoinConstraint::None => {}
            }
        }
    }

    fn factor(&mut self, factor: &'a TableFactor, ctes: &[(String, Option<&'a Query>, usize)], nullable: bool) {
        let named = |a: &ast::TableAlias| (a.name.value.to_ascii_lowercase(), !a.columns.is_empty());
        match factor {
            TableFactor::Table { name, alias, args: None, .. } => {
                let table = match name.0.as_slice() {
                    [ObjectNamePart::Identifier(t)] => Some(t.value.clone()),
                    // A view's statement names each table with the view's own schema.
                    [ObjectNamePart::Identifier(s), ObjectNamePart::Identifier(t)] if self.keys.schema.as_deref().is_some_and(|own| own.eq_ignore_ascii_case(&s.value)) => Some(t.value.clone()),
                    _ => None,
                };
                let Some(table) = table else {
                    let alias = alias.as_ref().map(|a| a.name.value.to_ascii_lowercase()).unwrap_or_default();
                    self.items.push(Item { alias, source: Source::Other, nullable, renamed: true });
                    return;
                };
                let lower = table.to_ascii_lowercase();
                let (alias, renamed) = alias.as_ref().map(named).unwrap_or_else(|| (lower.clone(), false));
                let unqualified = name.0.len() == 1;
                let source = match ctes.iter().rposition(|(n, _, _)| unqualified && *n == lower) {
                    Some(at) => match ctes[at].1 {
                        Some(body) => Source::Query(body, ctes[..ctes[at].2].to_vec()),
                        None => Source::Other,
                    },
                    // A view's statement sees none of the WITH queries of the statement that reads it.
                    None => match self.keys.view(&table) {
                        Some(view) => Source::Query(view, Vec::new()),
                        None => Source::Table(table),
                    },
                };
                self.items.push(Item { alias, source, nullable, renamed });
            }
            TableFactor::Derived { lateral, subquery, alias, .. } => {
                let (alias, renamed) = alias.as_ref().map(named).unwrap_or_default();
                let source = if *lateral { Source::Lateral(subquery, ctes.to_vec()) } else { Source::Query(subquery, ctes.to_vec()) };
                self.items.push(Item { alias, source, nullable, renamed });
            }
            TableFactor::NestedJoin { table_with_joins, alias: None } => self.flatten(table_with_joins, ctes, nullable),
            TableFactor::NestedJoin { .. } => self.refused = Some("a join with an alias of its own hides the names of the tables it joins"),
            _ => {
                let alias = match factor {
                    TableFactor::Table { alias, .. } | TableFactor::JsonTable { alias, .. } | TableFactor::Function { alias, .. } | TableFactor::UNNEST { alias, .. } => alias.as_ref(),
                    _ => None,
                };
                match alias {
                    Some(a) => self.items.push(Item { alias: a.name.value.to_ascii_lowercase(), source: Source::Other, nullable, renamed: true }),
                    None => self.refused = Some("a FROM item the engine cannot name"),
                }
            }
        }
    }

    /// The columns of item `i`, in lower case, where they are known.
    fn columns(&self, i: usize) -> Option<Vec<String>> {
        let item = &self.items[i];
        if item.renamed {
            return None;
        }
        match &item.source {
            Source::Table(t) => self.keys.columns(t).map(|c| c.iter().map(|(name, _)| name.clone()).collect()),
            Source::Query(q, ctes) | Source::Lateral(q, ctes) => output_names(q, ctes, self.keys),
            Source::Other => None,
        }
    }

    /// The item a column reference reads.
    fn place(&self, parts: &[ast::Ident]) -> Placed {
        let parts: Vec<String> = parts.iter().map(|p| p.value.to_ascii_lowercase()).collect();
        let qualified = |q: &str, c: &str| match self.items.iter().position(|it| it.alias == q) {
            Some(i) if self.items.iter().filter(|it| it.alias == q).count() == 1 && !self.items[i].renamed => Placed::Item(i, c.to_string()),
            Some(_) => Placed::Unknown,
            None => Placed::Outside,
        };
        match parts.as_slice() {
            [q, c] => qualified(q, c),
            [s, q, c] if self.keys.schema.as_deref().is_some_and(|own| own.eq_ignore_ascii_case(s)) => qualified(q, c),
            [c] if !self.merged => {
                let mut known = true;
                let mut having = Vec::new();
                for i in 0..self.items.len() {
                    match self.columns(i) {
                        Some(columns) if columns.contains(c) => having.push(i),
                        Some(_) => {}
                        None => known = false,
                    }
                }
                match having.as_slice() {
                    [i] => Placed::Item(*i, c.clone()),
                    [] if known => Placed::Outside,
                    // Without the columns, a bare name is the one item's, or the one item's
                    // whose key holds a column of the name.
                    [] if self.items.len() == 1 => Placed::Item(0, c.clone()),
                    [] => {
                        let owners: Vec<usize> = (0..self.items.len())
                            .filter(|i| matches!(&self.items[*i].source, Source::Table(t) if self.keys.keys(t).is_some_and(|k| k.iter().flatten().any(|k| k == c))))
                            .collect();
                        match owners.as_slice() {
                            [i] => Placed::Item(*i, c.clone()),
                            _ => Placed::Unknown,
                        }
                    }
                    _ => Placed::Unknown,
                }
            }
            _ => Placed::Unknown,
        }
    }

    fn place_expr(&self, e: &Expr) -> Option<Placed> {
        match e {
            Expr::Identifier(i) if !is_variable(e) => Some(self.place(std::slice::from_ref(i))),
            Expr::CompoundIdentifier(parts) => Some(self.place(parts)),
            Expr::Nested(inner) => self.place_expr(inner),
            _ => None,
        }
    }

    /// The items an expression reads, or `None` when one of its columns cannot be placed or it
    /// holds what the walk does not read.
    fn reads(&self, e: &Expr) -> Option<Vec<usize>> {
        if is_variable(e) {
            return None;
        }
        match e {
            Expr::Identifier(_) | Expr::CompoundIdentifier(_) => match self.place_expr(e)? {
                Placed::Item(i, _) => Some(vec![i]),
                Placed::Outside => Some(Vec::new()),
                Placed::Unknown => None,
            },
            Expr::Value(_) | Expr::TypedString { .. } => Some(Vec::new()),
            Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } | Expr::Cast { expr: inner, .. } => self.reads(inner),
            Expr::BinaryOp { left, right, .. } => {
                let mut out = self.reads(left)?;
                out.extend(self.reads(right)?);
                Some(out)
            }
            Expr::Function(f) => match &f.args {
                ast::FunctionArguments::None => Some(Vec::new()),
                ast::FunctionArguments::List(list) => {
                    let mut out = Vec::new();
                    for a in &list.args {
                        match a {
                            ast::FunctionArg::Unnamed(ast::FunctionArgExpr::Expr(x)) => out.extend(self.reads(x)?),
                            _ => return None,
                        }
                    }
                    Some(out)
                }
                ast::FunctionArguments::Subquery(_) => None,
            },
            _ => None,
        }
    }

    /// What column `column` of item `i` compares as.
    fn class(&self, i: usize, column: &str) -> Option<Class> {
        match &self.items[i].source {
            Source::Table(t) => self.keys.class_of(t, column),
            Source::Query(q, ctes) | Source::Lateral(q, ctes) => output_class(q, ctes, self.keys, column),
            Source::Other => None,
        }
    }

    /// Whether `e` is `column = value` for this column of item `i`, where the value reads no column
    /// of `i`, only columns of `done` items and of enclosing queries, gives every row one value,
    /// and compares the column as it is.
    fn equates(&self, e: &Expr, i: usize, column: &str, class: Option<Class>, done: &[bool]) -> bool {
        let Expr::BinaryOp { left, op: ast::BinaryOperator::Eq, right } = e else { return false };
        [(left, right), (right, left)].into_iter().any(|(side, value)| {
            let own = matches!(self.place_expr(side), Some(Placed::Item(j, c)) if j == i && c == column);
            own && self.reads(value).is_some_and(|read| read.iter().all(|j| *j != i && done[*j])) && self.compares(class, value)
        })
    }

    /// Whether a key column of `class` compared with `value` compares as it is, and the value is
    /// one value for every row.
    fn compares(&self, class: Option<Class>, value: &Expr) -> bool {
        if !reads_column(value) {
            return deterministic(value) && compares_as_is(class, value_class(value));
        }
        // Another column: both must be numbers.
        match self.place_expr(value) {
            Some(Placed::Item(j, c)) => class == Some(Class::Number) && self.class(j, &c) == Some(Class::Number),
            _ => false,
        }
    }

    /// Whether the equalities pin every column of one of `keys`, the unique keys of item `i`.
    fn key_pinned(&self, i: usize, keys: &[Key], done: &[bool], constant: bool) -> bool {
        let item = &self.items[i];
        if item.renamed {
            return false;
        }
        let usable = |c: &Conjunct| match &c.may_pin {
            None => true,
            Some(may) => may.contains(&i) && (!constant || (c.inner && !item.nullable)),
        };
        keys.iter().any(|key| !key.is_empty() && key.iter().all(|part| self.conjuncts.iter().any(|c| usable(c) && self.equates(c.expr, i, &part.name, part.class, done))))
    }

    /// The unique keys of item `i`'s rows, each column by the name the item gives it.
    fn item_keys(&self, i: usize) -> Vec<Key> {
        if self.items[i].renamed {
            return Vec::new();
        }
        match &self.items[i].source {
            Source::Table(t) => self
                .keys
                .keys(t)
                .into_iter()
                .flatten()
                .map(|key| key.iter().map(|c| Part { name: c.clone(), class: self.keys.class_of(t, c) }).collect())
                .collect(),
            Source::Query(q, ctes) | Source::Lateral(q, ctes) => output_keys(q, ctes, self.keys),
            Source::Other => Vec::new(),
        }
    }

    /// Which items are determined: the closure in the module's docs. With `constant`, only items
    /// no row null-extends count, pinned by the WHERE or by inner joins: every row of the FROM then
    /// carries the same row of each, which is what grouping and DISTINCT ask. `done` names the
    /// items taken as determined from the start, and `keys` are each item's ([`From::item_keys`]).
    fn determined(&self, mut done: Vec<bool>, keys: &[Vec<Key>], constant: bool) -> Vec<bool> {
        let n = self.items.len();
        let per_row: Vec<bool> = self.items.iter().map(|it| matches!(it.source, Source::Lateral(..))).collect();
        loop {
            let mut changed = false;
            for i in 0..n {
                if done[i] {
                    continue;
                }
                let item = &self.items[i];
                let rest = || (0..n).all(|j| j == i || done[j] || (!constant && per_row[j]));
                let pinned = match &item.source {
                    Source::Table(_) => self.key_pinned(i, &keys[i], &done, constant),
                    _ if constant && item.nullable => false,
                    Source::Query(q, ctes) => at_most_one_in(q, ctes, self.keys).is_ok() || self.key_pinned(i, &keys[i], &done, constant),
                    Source::Lateral(q, ctes) => rest() && (at_most_one_in(q, ctes, self.keys).is_ok() || self.key_pinned(i, &keys[i], &done, constant)),
                    Source::Other => false,
                };
                if pinned {
                    done[i] = true;
                    changed = true;
                }
            }
            if !changed {
                return done;
            }
        }
    }

    fn all_keys(&self) -> Vec<Vec<Key>> {
        (0..self.items.len()).map(|i| self.item_keys(i)).collect()
    }

    /// Whether every row of the FROM carries the same value of `e`: a column of a `constant` item,
    /// a column the WHERE equates to a value, or a value itself.
    fn pinned(&self, e: &Expr, constant: &[bool]) -> bool {
        match self.place_expr(e) {
            Some(Placed::Item(i, column)) => {
                constant[i] || {
                    let class = self.class(i, &column);
                    let none = vec![false; self.items.len()];
                    self.conjuncts.iter().any(|c| c.may_pin.is_none() && self.equates(c.expr, i, &column, class, &none))
                }
            }
            Some(_) => false,
            None => !reads_column(e) && deterministic(e),
        }
    }

    /// Why the closure leaves the FROM more than one row, from its first undetermined item.
    fn why_not(&self, determined: &[bool]) -> String {
        let Some(i) = determined.iter().position(|d| !d) else { return "every table is pinned to one row".into() };
        let item = &self.items[i];
        match &item.source {
            Source::Table(t) => match self.keys.keys(t) {
                None => format!("`{t}` has no unique index the engine can read, so nothing bounds the rows a WHERE matches"),
                Some(keys) => {
                    let shown: Vec<String> = keys.iter().map(|k| format!("({})", k.join(", "))).collect();
                    if self.items.len() == 1 {
                        format!("the WHERE does not equate every column of a unique key of `{t}` — it has {}", shown.join(", "))
                    } else {
                        format!("a join returns a row per pair it matches, and the WHERE and ON clauses do not equate every column of a unique key of `{}` to values or to columns of tables pinned to one row — it has {}", item.alias, shown.join(", "))
                    }
                }
            },
            Source::Query(..) | Source::Lateral(..) => format!("`{}` returns any number of rows, and the WHERE does not equate every column of a unique key of its rows", item.alias),
            Source::Other => format!("`{}` in the FROM is a table function or a recursive WITH query, which returns any number of rows", item.alias),
        }
    }

    /// The output name of column `column` of item `i`, where the select list keeps it unchanged.
    fn output(&self, s: &Select, i: usize, column: &str) -> Option<String> {
        s.projection.iter().find_map(|item| match item {
            SelectItem::UnnamedExpr(e) => match self.place_expr(e)? {
                Placed::Item(j, c) if j == i && c == column => Some(c),
                _ => None,
            },
            SelectItem::ExprWithAlias { expr, alias } => match self.place_expr(expr)? {
                Placed::Item(j, c) if j == i && c == column => Some(alias.value.to_ascii_lowercase()),
                _ => None,
            },
            SelectItem::Wildcard(_) if !self.merged => Some(column.to_string()),
            SelectItem::QualifiedWildcard(SelectItemQualifiedWildcardKind::ObjectName(name), _) => match name.0.as_slice() {
                [ObjectNamePart::Identifier(q)] if q.value.to_ascii_lowercase() == self.items[i].alias => Some(column.to_string()),
                _ => None,
            },
            _ => None,
        })
    }
}

/// The terms of a condition's top-level `AND`.
fn conjuncts(e: &Expr) -> Vec<&Expr> {
    match e {
        Expr::BinaryOp { left, op: ast::BinaryOperator::And, right } => {
            let mut out = conjuncts(left);
            out.extend(conjuncts(right));
            out
        }
        Expr::Nested(inner) => conjuncts(inner),
        other => vec![other],
    }
}

/// The SELECT a query's body is, when it is one.
fn plain(q: &Query) -> Option<&Select> {
    match q.body.as_ref() {
        SetExpr::Select(s) => Some(s),
        _ => None,
    }
}

/// The names of a query's output columns, in lower case, where the engine can tell them all.
fn output_names(q: &Query, outer: &[(String, Option<&Query>, usize)], keys: &Keys) -> Option<Vec<String>> {
    let s = plain(q)?;
    let ctes = ctes_of(q, outer);
    let from = From::read(s, &ctes, keys);
    let mut out = Vec::new();
    for item in &s.projection {
        match item {
            SelectItem::UnnamedExpr(Expr::Identifier(i)) => out.push(i.value.to_ascii_lowercase()),
            SelectItem::UnnamedExpr(Expr::CompoundIdentifier(parts)) => out.push(parts.last()?.value.to_ascii_lowercase()),
            SelectItem::ExprWithAlias { alias, .. } => out.push(alias.value.to_ascii_lowercase()),
            SelectItem::Wildcard(_) if !from.merged && from.refused.is_none() => {
                for i in 0..from.items.len() {
                    out.extend(from.columns(i)?);
                }
            }
            SelectItem::QualifiedWildcard(SelectItemQualifiedWildcardKind::ObjectName(name), _) => {
                let [ObjectNamePart::Identifier(q)] = name.0.as_slice() else { return None };
                let i = from.items.iter().position(|it| it.alias == q.value.to_ascii_lowercase())?;
                out.extend(from.columns(i)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

/// What output column `name` of a query compares as: that of the column it passes through.
fn output_class(q: &Query, outer: &[(String, Option<&Query>, usize)], keys: &Keys, name: &str) -> Option<Class> {
    let s = plain(q)?;
    let ctes = ctes_of(q, outer);
    let from = From::read(s, &ctes, keys);
    if from.refused.is_some() {
        return None;
    }
    s.projection.iter().find_map(|item| {
        let (expr, out) = match item {
            SelectItem::UnnamedExpr(e) => (e, None),
            SelectItem::ExprWithAlias { expr, alias } => (expr, Some(alias.value.to_ascii_lowercase())),
            SelectItem::Wildcard(_) if !from.merged => {
                return (0..from.items.len()).find(|i| from.columns(*i).is_some_and(|c| c.iter().any(|c| c == name))).and_then(|i| from.class(i, name));
            }
            _ => return None,
        };
        match from.place_expr(expr)? {
            Placed::Item(i, c) if out.as_deref().unwrap_or(&c) == name => from.class(i, &c),
            _ => None,
        }
    })
}

/// The unique keys of a query's rows: sets of its output columns no two of its rows share a value
/// of every one of (NULL aside, which no `=` matches).
///
/// - A unique key of one of its FROM items, every column of it in the select list unchanged, when
///   every other item is determined against that item: each of its rows then meets one row at
///   most of the rest. A WHERE, a HAVING, a LIMIT and an OFFSET only remove rows, and a window
///   function adds none. A LATERAL derived table is no such item: its rows are computed anew for
///   each row of the rest, so two of them may share a key.
/// - The GROUP BY's expressions, each a column of the select list; not WITH ROLLUP, which adds a
///   row per level.
/// - Under DISTINCT, the whole select list, every entry a column.
///
/// None for a set operation or a VALUES list.
fn output_keys(q: &Query, outer: &[(String, Option<&Query>, usize)], keys: &Keys) -> Vec<Key> {
    let Some(s) = plain(q) else { return Vec::new() };
    let ctes = ctes_of(q, outer);
    let from = From::read(s, &ctes, keys);
    if from.refused.is_some() {
        return Vec::new();
    }
    let mut out = Vec::new();
    // The output column a GROUP BY expression is: a column, a column by its position, or an alias.
    let part = |g: &Expr| -> Option<Part> {
        let placed = match g {
            Expr::Value(v) => match &v.value {
                ast::Value::Number(n, _) => match s.projection.get(n.parse::<usize>().ok()?.checked_sub(1)?)? {
                    SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => from.place_expr(e)?,
                    _ => return None,
                },
                _ => return None,
            },
            // A bare name is a column of the FROM first, and else an alias of the select list.
            _ => match from.place_expr(g)? {
                Placed::Item(i, c) => Placed::Item(i, c),
                _ => {
                    let Expr::Identifier(name) = g else { return None };
                    let aliased = s.projection.iter().find_map(|item| match item {
                        SelectItem::ExprWithAlias { expr, alias } if alias.value.eq_ignore_ascii_case(&name.value) => Some(expr),
                        _ => None,
                    })?;
                    from.place_expr(aliased)?
                }
            },
        };
        let Placed::Item(i, c) = placed else { return None };
        Some(Part { name: from.output(s, i, &c)?, class: from.class(i, &c) })
    };
    match &s.group_by {
        GroupByExpr::Expressions(exprs, modifiers) if !exprs.is_empty() => {
            if modifiers.is_empty() {
                out.extend(exprs.iter().map(part).collect::<Option<Key>>());
            }
            return out;
        }
        GroupByExpr::All(_) => return out,
        _ => {}
    }
    if matches!(s.distinct, Some(Distinct::Distinct)) {
        let key: Option<Key> = s
            .projection
            .iter()
            .map(|item| {
                let (e, alias) = match item {
                    SelectItem::UnnamedExpr(e) => (e, None),
                    SelectItem::ExprWithAlias { expr, alias } => (expr, Some(alias.value.to_ascii_lowercase())),
                    _ => return None,
                };
                let Placed::Item(i, c) = from.place_expr(e)? else { return None };
                Some(Part { name: alias.unwrap_or_else(|| c.clone()), class: from.class(i, &c) })
            })
            .collect();
        out.extend(key);
    }
    let all = from.all_keys();
    let n = from.items.len();
    for r in 0..n {
        if !matches!(from.items[r].source, Source::Table(_) | Source::Query(..)) {
            continue;
        }
        let mut seed = vec![false; n];
        seed[r] = true;
        if !from.determined(seed, &all, false).iter().all(|d| *d) {
            continue;
        }
        for key in &all[r] {
            out.extend(key.iter().map(|p| from.output(s, r, &p.name).map(|name| Part { name, class: p.class })).collect::<Option<Key>>());
        }
    }
    out
}

/// Prove that a query returns at most one row, or say why the engine cannot.
pub(crate) fn at_most_one(q: &Query, keys: &Keys) -> Result<(), String> {
    at_most_one_in(q, &[], keys)
}

fn at_most_one_in(q: &Query, outer: &[(String, Option<&Query>, usize)], keys: &Keys) -> Result<(), String> {
    if matches!(const_limit(q), Some(Some(n)) if (0..=1).contains(&n)) {
        return Ok(());
    }
    // Any other LIMIT, and an OFFSET, only remove rows: the bound is the one the statement has
    // without them.
    if one_before_limit(q).is_ok() {
        return Ok(());
    }
    let s = select_of(q)?;
    let ctes = ctes_of(q, outer);
    let from = From::read(s, &ctes, keys);
    if let Some(why) = from.refused {
        return Err(why.into());
    }
    let all = from.all_keys();
    let n = from.items.len();
    let grouped: Option<&[Expr]> = match &s.group_by {
        GroupByExpr::Expressions(_, modifiers) if !modifiers.is_empty() => return Err("GROUP BY … WITH ROLLUP adds a row for every level of the grouping".into()),
        GroupByExpr::Expressions(e, _) if !e.is_empty() => Some(e),
        GroupByExpr::All(_) => return Err("GROUP BY ALL returns one row per group".into()),
        _ => None,
    };
    if let Some(group) = grouped {
        if from.determined(vec![false; n], &all, false).iter().all(|d| *d) {
            return Ok(());
        }
        let constant = from.determined(vec![false; n], &all, true);
        if group.iter().all(|g| from.pinned(g, &constant)) {
            return Ok(());
        }
        return Err("GROUP BY returns one row per group; the engine proves one group only when every GROUP BY expression is pinned to one value: a column of a table whose unique key the WHERE pins, or a column the WHERE equates to a value".into());
    }
    if s.having.is_some() {
        return Err("HAVING leaves a row per group".into());
    }
    if s.from.is_empty() {
        return Err("a SELECT with no FROM and a WHERE can return no row or one, and the engine proves that only through a LIMIT".into());
    }
    let determined = from.determined(vec![false; n], &all, false);
    if determined.iter().all(|d| *d) {
        return Ok(());
    }
    if matches!(s.distinct, Some(Distinct::Distinct)) {
        let constant = from.determined(vec![false; n], &all, true);
        let pinned = s.projection.iter().all(|item| match item {
            SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => from.pinned(e, &constant),
            _ => false,
        });
        if pinned {
            return Ok(());
        }
    }
    Err(from.why_not(&determined))
}

/// Every table a query reads anywhere in its FROM trees, as written, a WITH query's name never.
pub(crate) fn tables_anywhere(q: &Query, schema: Option<&str>) -> Vec<String> {
    fn query(q: &Query, ctes: &mut Vec<String>, schema: Option<&str>, out: &mut Vec<String>) {
        let depth = ctes.len();
        if let Some(w) = &q.with {
            for c in &w.cte_tables {
                ctes.push(c.alias.name.value.to_ascii_lowercase());
                query(&c.query, ctes, schema, out);
            }
        }
        body(&q.body, ctes, schema, out);
        ctes.truncate(depth);
    }
    fn body(b: &SetExpr, ctes: &mut Vec<String>, schema: Option<&str>, out: &mut Vec<String>) {
        match b {
            SetExpr::Select(s) => {
                for twj in &s.from {
                    factor(&twj.relation, ctes, schema, out);
                    for j in &twj.joins {
                        factor(&j.relation, ctes, schema, out);
                    }
                }
            }
            SetExpr::SetOperation { left, right, .. } => {
                body(left, ctes, schema, out);
                body(right, ctes, schema, out);
            }
            SetExpr::Query(q) => query(q, ctes, schema, out),
            _ => {}
        }
    }
    fn factor(f: &TableFactor, ctes: &mut Vec<String>, schema: Option<&str>, out: &mut Vec<String>) {
        match f {
            TableFactor::Table { name, args: None, .. } => match name.0.as_slice() {
                [ObjectNamePart::Identifier(t)] if !ctes.contains(&t.value.to_ascii_lowercase()) => out.push(t.value.clone()),
                [ObjectNamePart::Identifier(s), ObjectNamePart::Identifier(t)] if schema.is_some_and(|own| own.eq_ignore_ascii_case(&s.value)) => out.push(t.value.clone()),
                _ => {}
            },
            TableFactor::Derived { subquery, .. } => query(subquery, ctes, schema, out),
            TableFactor::NestedJoin { table_with_joins, .. } => {
                factor(&table_with_joins.relation, ctes, schema, out);
                for j in &table_with_joins.joins {
                    factor(&j.relation, ctes, schema, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    query(q, &mut Vec::new(), schema, &mut out);
    out.sort();
    out.dedup();
    out
}
