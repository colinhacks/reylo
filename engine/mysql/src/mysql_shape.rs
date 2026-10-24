//! MySQL's JSON constructors as nested shapes, the way the Postgres engine's `jsonshape` reads Postgres's:
//! the constructor's value expressions are selected as real columns of a synthesized statement
//! over the constructor's own `FROM`, and the server's prepare types them, with the join's own
//! nullability. Recognised: `json_object(k, v, …)`, `json_arrayagg(x)`, `coalesce` / `ifnull`
//! guarding a `json_arrayagg` with an empty array (`json_array()`, `cast('[]' as json)`),
//! `cast(… as json)`, and a scalar subquery around any of them. A probe keeps the statement's `?`
//! where they are: a prepare binds nothing, and the server types a `?` from its context there too.
//!
//! The server types a truth value (`a = b`, `x is null`) as an integer and writes it into a JSON
//! document as `true` or `false`, but only where the document's function reads the comparison
//! itself. Where it reads a copy (a column of the temporary table an aggregate, a window function
//! or SQL_BUFFER_RESULT is computed through, or a subquery's result) it writes 1 or 0; measured on
//! 8.4, `json_object('t', id = 1, 'm', max(n))` holds `"t": 1`. So a value is typed `bool` only
//! where the comparison is certain to be read itself ([`Reads`]), and as either where it is not.
//!
//! The same probes repair one pessimism of the server's own flags (measured on 8.4): a correlated
//! ungrouped aggregate subquery in the select list marks every outer column and the subquery
//! itself nullable, while a GROUP BY, an EXISTS or a `limit 1` subquery leave them alone. So a
//! scalar subquery item is planned like a constructor (its item prepared over its own `FROM`,
//! kept only when the subquery is exactly one row), and [`outer_probe`] prepares the statement
//! without its subquery items; a column is NOT NULL when either prepare says so.

use crate::shape::{Probe, Shape};
use sqlparser::ast::{self, DataType, Expr, FunctionArg, FunctionArgExpr, FunctionArguments, Query, SelectItem, SetExpr, TableFactor, TableWithJoins, Value};
use std::collections::{HashMap, HashSet};

/// The columns of the tables a statement names, by the table's name as the statement writes it,
/// each column in lower case, which is how MySQL compares a column name. What says whose an
/// aggregate in a scalar subquery is ([`Builder::owns`]).
pub type Columns = HashMap<String, HashSet<String>>;

/// How many synthesized statements one statement may cost; past it the statement's shapes are
/// abandoned rather than turning one prepare into dozens.
const MAX_PROBES: usize = 8;

#[derive(Debug, Default)]
pub struct Plan {
    /// Result column alias → its shape, in projection order.
    pub columns: Vec<(String, Shape)>,
    pub probes: Vec<Probe>,
    /// The leaves (probe, index) whose expression is a truth value: MySQL types a comparison as
    /// an integer, but writes it into a JSON document as `true` or `false`.
    pub booleans: Vec<(usize, usize)>,
    /// The leaves the document may hold as `true` or `false` or as the server's integer: a truth
    /// value the document's function may read a copy of, and an expression not known to be a number.
    pub either: Vec<(usize, usize)>,
    /// The tables whose columns the plan needed and `columns` did not have: with them it may
    /// read a subquery's aggregate over a bare column as the subquery's own. The probes do not
    /// depend on the answer.
    pub wanted: Vec<String>,
}

/// Read one statement's JSON constructors; the probes come back as text for the build to prepare.
pub fn plan(q: &Query, columns: &Columns) -> Plan {
    let SetExpr::Select(select) = q.body.as_ref() else { return Plan::default() };
    // A probe of a statement read materialized is read so too ([`crate::mysql_derived`]).
    let hint = if crate::mysql_derived::is_materialized(q) { crate::mysql_derived::hint() } else { String::new() };
    let grouped = crate::mysql_nulls::grouped(&select.group_by);
    let top = Level { ctx: 0, from: select.from.clone(), with: q.with.clone() };
    let mut b = Builder { stack: vec![top], columns, ctes: with_names(q), wanted: Vec::new(), next_ctx: 0, drafts: Vec::new(), overflowed: false, booleans: Vec::new(), either: Vec::new(), reads: Reads::Itself, grouped };
    let mut columns = Vec::new();
    for item in &select.projection {
        // The server names an unaliased expression by its text, which no build reproduces
        // reliably, so only an aliased constructor is matched to a result column.
        let SelectItem::ExprWithAlias { expr, alias } = item else { continue };
        if !is_constructor(expr) && !matches!(expr, Expr::Subquery(_)) {
            continue;
        }
        let before = b.drafts.len();
        let shape = b.walk(expr);
        if b.overflowed {
            b.drafts.truncate(before);
            break;
        }
        if shape != Shape::Unknown {
            columns.push((alias.value.clone(), shape));
        }
    }
    let wider = |d: &Draft, from: &[TableWithJoins]| match &d.wrap {
        Some(w) => wrapped_text(w, &hint, &d.items, from),
        None => text(&d.with, &hint, &d.items, from),
    };
    let probes = b.drafts.iter().map(|d| Probe { sql: text(&d.with, &hint, &d.items, &d.from), wider: d.wider.as_ref().map(|f| wider(d, f)) }).collect();
    b.wanted.sort_unstable();
    b.wanted.dedup();
    Plan { columns, probes, booleans: b.booleans, either: b.either, wanted: b.wanted }
}

/// A WITH query in scope: its name as written, and the names of its columns.
struct Cte {
    name: String,
    columns: HashSet<String>,
}

/// A query's own WITH queries.
fn with_names(q: &Query) -> Vec<Cte> {
    q.with.iter().flat_map(|w| w.cte_tables.iter().map(|c| Cte { name: c.alias.name.value.clone(), columns: output_columns(&c.query, &c.alias.columns) })).collect()
}

/// The names, in lower case, of the columns of a derived table or a WITH query that are certain:
/// its column list, or else each item of its (first) select list that is a column or has an
/// alias. An item named by its expression's text, and a wildcard, are left out, which only
/// leaves a read of them unproven.
fn output_columns(q: &Query, list: &[ast::TableAliasColumnDef]) -> HashSet<String> {
    if !list.is_empty() {
        return list.iter().map(|c| c.name.value.to_ascii_lowercase()).collect();
    }
    let mut body = q.body.as_ref();
    let select = loop {
        match body {
            SetExpr::Select(select) => break select,
            SetExpr::Query(q) => body = q.body.as_ref(),
            SetExpr::SetOperation { left, .. } => body = left.as_ref(),
            _ => return HashSet::new(),
        }
    };
    select
        .projection
        .iter()
        .filter_map(|item| match item {
            SelectItem::ExprWithAlias { alias, .. } => Some(&alias.value),
            SelectItem::UnnamedExpr(Expr::Identifier(i)) => Some(&i.value),
            SelectItem::UnnamedExpr(Expr::CompoundIdentifier(parts)) => parts.last().map(|p| &p.value),
            _ => None,
        })
        .map(|name| name.to_ascii_lowercase())
        .collect()
}

/// Where the columns of a table in a FROM are known.
enum Source {
    /// The catalog's table of this name.
    Catalog(String),
    /// A derived table's or a WITH query's, in lower case.
    Columns(HashSet<String>),
    Unknown,
}

/// The statement without its subquery items, and the projection indexes it keeps: prepared, it
/// reports the outer columns' flags free of the subquery's pessimism. `None` when nothing would
/// change or a wildcard makes the positions unreadable.
pub fn outer_probe(q: &Query) -> Option<(String, Vec<usize>)> {
    let SetExpr::Select(select) = q.body.as_ref() else { return None };
    if select.projection.iter().any(|i| matches!(i, SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..))) {
        return None;
    }
    let kept: Vec<usize> = select
        .projection
        .iter()
        .enumerate()
        .filter(|(_, item)| match item {
            SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => !contains_subquery(e),
            _ => true,
        })
        .map(|(i, _)| i)
        .collect();
    if kept.is_empty() || kept.len() == select.projection.len() {
        return None;
    }
    let mut probe = (**select).clone();
    probe.projection = kept.iter().map(|&i| select.projection[i].clone()).collect();
    let with = q.with.as_ref().map(|w| format!("{w} ")).unwrap_or_default();
    Some((format!("{with}{probe}"), kept))
}

/// The columns an expression reads, each as its parts; false where the expression has a part this
/// does not read, a subquery among them, whose columns have a scope of their own.
fn references(e: &Expr, out: &mut Vec<Vec<String>>) -> bool {
    match e {
        Expr::Identifier(i) => {
            out.push(vec![i.value.clone()]);
            true
        }
        Expr::CompoundIdentifier(parts) => {
            out.push(parts.iter().map(|p| p.value.clone()).collect());
            true
        }
        Expr::Value(_) => true,
        Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } | Expr::Cast { expr: inner, .. } | Expr::IsNull(inner) | Expr::IsNotNull(inner) => references(inner, out),
        Expr::BinaryOp { left, right, .. } => {
            let left = references(left, out);
            references(right, out) && left
        }
        Expr::Function(f) if f.over.is_none() && f.filter.is_none() && f.within_group.is_empty() => match &f.args {
            FunctionArguments::None => true,
            FunctionArguments::List(list) => {
                let mut whole = list.clauses.is_empty();
                for arg in &list.args {
                    whole &= match arg {
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(x)) => references(x, out),
                        _ => false,
                    };
                }
                whole
            }
            FunctionArguments::Subquery(_) => false,
        },
        _ => false,
    }
}

/// Whether an expression holds a subquery anywhere the walk can see; what it cannot see counts as none, which only leaves that item in the probe.
fn contains_subquery(e: &Expr) -> bool {
    match e {
        Expr::Subquery(_) | Expr::Exists { .. } | Expr::InSubquery { .. } => true,
        Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } | Expr::Cast { expr: inner, .. } | Expr::IsNull(inner) | Expr::IsNotNull(inner) => contains_subquery(inner),
        Expr::BinaryOp { left, right, .. } => contains_subquery(left) || contains_subquery(right),
        Expr::Function(f) => args(f).is_some_and(|a| a.iter().any(|x| contains_subquery(x))),
        Expr::Case { operand, conditions, else_result, .. } => {
            operand.as_deref().is_some_and(contains_subquery) || conditions.iter().any(|c| contains_subquery(&c.condition) || contains_subquery(&c.result)) || else_result.as_deref().is_some_and(contains_subquery)
        }
        _ => false,
    }
}

/// The WITH queries of every level from the statement down to the innermost, as the one WITH a
/// probe of the innermost level starts with; empty where no level has one. The levels cannot be
/// nested as the statement nests them: MySQL hides an outer WITH inside a parenthesized query
/// that has a WITH of its own (measured on 8.4). One list reads each name as the levels do, the
/// outermost first: the server refuses a name listed twice, and a WITH query never reads one
/// listed after it, with RECURSIVE or without. RECURSIVE alone, which one list takes for every
/// query if any level has it, could change a reading: a query of a level without it that names
/// itself reads a table or an outer level's query, and with it would read itself. `None` then.
fn carried(levels: &[Level]) -> Option<String> {
    let withs: Vec<&ast::With> = levels.iter().filter_map(|l| l.with.as_ref()).collect();
    let Some(first) = withs.first() else { return Some(String::new()) };
    let recursive = withs.iter().any(|w| w.recursive);
    if recursive && withs.iter().filter(|w| !w.recursive).flat_map(|w| &w.cte_tables).any(|c| mentions(&c.query.to_string(), std::slice::from_ref(&c.alias.name.value))) {
        return None;
    }
    let cte_tables = withs.iter().flat_map(|w| w.cte_tables.iter().cloned()).collect();
    Some(format!("{} ", ast::With { with_token: first.with_token.clone(), recursive, cte_tables }))
}

/// Whether a text has a word that is one of `names`, in any case, quoted or not; true where it
/// does not tokenize.
fn mentions(text: &str, names: &[String]) -> bool {
    use sqlparser::tokenizer::{Token, Tokenizer};
    let Ok(tokens) = Tokenizer::new(&sqlparser::dialect::MySqlDialect {}, text).tokenize() else { return true };
    tokens.iter().any(|t| matches!(t, Token::Word(w) if names.iter().any(|n| n.eq_ignore_ascii_case(&w.value))))
}

/// The alias of the derived table [`wrapped_text`] reads its columns from.
const WRAPPED: &str = "reylo_probe";

/// `SELECT reylo_probe.* FROM <around>, LATERAL (SELECT <items> FROM <from>) AS reylo_probe`: a
/// probe whose FROM reads the tables of `around` as a subquery reads a query around it. Each item
/// is named by its position, since a derived table's columns need names of their own.
fn wrapped_text(wrap: &Wrap, hint: &str, items: &[Expr], wider: &[TableWithJoins]) -> String {
    let (around, from) = wider.split_at(wrap.at);
    let items: Vec<String> = items.iter().enumerate().map(|(i, e)| format!("{e} AS a{i}")).collect();
    let from: Vec<String> = from.iter().map(|f| f.to_string()).collect();
    let mut tables: Vec<String> = around.iter().map(|f| f.to_string()).collect();
    tables.push(format!("LATERAL ({}SELECT {} FROM {}) AS {WRAPPED}", wrap.inner_with, items.join(", "), from.join(", ")));
    format!("{}SELECT {hint}{WRAPPED}.* FROM {}", wrap.with, tables.join(", "))
}

/// A FROM entry with every derived table in it, joins followed, made LATERAL: it then reads the
/// tables left of it as it read the queries around it, where a name it means is found the same
/// way. A name the probe would find twice, a table of the same alias or a column two of them
/// have, fails it rather than reading another.
fn lateral(mut t: TableWithJoins) -> TableWithJoins {
    fn factor(f: &mut TableFactor) {
        match f {
            TableFactor::Derived { lateral, .. } => *lateral = true,
            TableFactor::NestedJoin { table_with_joins, .. } => **table_with_joins = lateral((**table_with_joins).clone()),
            _ => {}
        }
    }
    factor(&mut t.relation);
    t.joins.iter_mut().for_each(|j| factor(&mut j.relation));
    t
}

/// Whether a FROM entry has a join whose ON reads a table no join of the entry holds: one of a
/// query around it, as MySQL reads a name the join does not have. A name in a subquery of the ON
/// is that subquery's own to find.
fn correlated_on(t: &TableWithJoins) -> bool {
    fn tables(t: &TableWithJoins, names: &mut Vec<String>, ons: &mut Vec<Expr>) {
        for f in std::iter::once(&t.relation).chain(t.joins.iter().map(|j| &j.relation)) {
            match f {
                TableFactor::Table { name, alias, .. } => names.extend(alias.as_ref().map(|a| a.name.value.clone()).or_else(|| match name.0.last() {
                    Some(ast::ObjectNamePart::Identifier(i)) => Some(i.value.clone()),
                    _ => None,
                })),
                TableFactor::NestedJoin { table_with_joins, alias } => {
                    names.extend(alias.as_ref().map(|a| a.name.value.clone()));
                    tables(table_with_joins, names, ons);
                }
                TableFactor::Derived { alias, .. } | TableFactor::JsonTable { alias, .. } => names.extend(alias.as_ref().map(|a| a.name.value.clone())),
                _ => {}
            }
        }
        for j in &t.joins {
            use ast::{JoinConstraint, JoinOperator as J};
            let constraint = match &j.join_operator {
                J::Join(c) | J::Inner(c) | J::Left(c) | J::LeftOuter(c) | J::Right(c) | J::RightOuter(c) | J::StraightJoin(c) => Some(c),
                _ => None,
            };
            if let Some(JoinConstraint::On(on)) = constraint {
                ons.push(on.clone());
            }
        }
    }
    fn qualifiers(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::CompoundIdentifier(parts) if parts.len() >= 2 => out.push(parts[parts.len() - 2].value.clone()),
            Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } | Expr::Cast { expr: inner, .. } | Expr::IsNull(inner) | Expr::IsNotNull(inner) | Expr::IsTrue(inner) | Expr::IsFalse(inner) => qualifiers(inner, out),
            Expr::BinaryOp { left, right, .. } => {
                qualifiers(left, out);
                qualifiers(right, out);
            }
            Expr::Between { expr, low, high, .. } => [expr, low, high].into_iter().for_each(|x| qualifiers(x, out)),
            Expr::InList { expr, list, .. } => std::iter::once(&**expr).chain(list).for_each(|x| qualifiers(x, out)),
            Expr::Function(f) => args(f).into_iter().flatten().for_each(|x| qualifiers(x, out)),
            _ => {}
        }
    }
    let (mut names, mut ons) = (Vec::new(), Vec::new());
    tables(t, &mut names, &mut ons);
    let mut read = Vec::new();
    ons.iter().for_each(|on| qualifiers(on, &mut read));
    read.iter().any(|q| !names.iter().any(|n| n.eq_ignore_ascii_case(q)))
}

fn text(with: &str, hint: &str, items: &[Expr], from: &[TableWithJoins]) -> String {
    let items: Vec<String> = items.iter().map(|e| e.to_string()).collect();
    if from.is_empty() {
        format!("{with}SELECT {hint}{}", items.join(", "))
    } else {
        let from: Vec<String> = from.iter().map(|f| f.to_string()).collect();
        format!("{with}SELECT {hint}{} FROM {}", items.join(", "), from.join(", "))
    }
}

/// One query the builder stands in: the statement, or a scalar subquery inside it.
#[derive(Clone)]
struct Level {
    /// An id of its own, so two sibling subqueries at one depth do not pool their columns into
    /// one probe.
    ctx: usize,
    from: Vec<TableWithJoins>,
    /// The query's own WITH, which its FROM and its items may read.
    with: Option<ast::With>,
}

struct Builder<'a> {
    /// The queries in scope, outermost first.
    stack: Vec<Level>,
    columns: &'a Columns,
    /// The WITH queries in scope where the builder stands, whose names are no tables'.
    ctes: Vec<Cte>,
    wanted: Vec<String>,
    next_ctx: usize,
    drafts: Vec<Draft>,
    overflowed: bool,
    booleans: Vec<(usize, usize)>,
    either: Vec<(usize, usize)>,
    /// How the function that writes the value now walked reads a truth value.
    reads: Reads,
    /// Whether the statement's SELECT has a plain GROUP BY, where no group is empty.
    grouped: bool,
}

/// How a JSON function reads the value it writes, which decides whether a truth value is
/// written as `true` or `false` (the function reads the comparison) or as 1 or 0 (it reads a copy).
#[derive(Clone, Copy, PartialEq)]
enum Reads {
    /// The comparison itself.
    Itself,
    /// A copy of each argument that holds no aggregate and no window function, which is how the
    /// server evaluates a call that has one among its arguments: a comparison over an aggregate
    /// is read itself, any other as a copy. A literal `true` is copied nowhere.
    Split,
    /// A copy, in some plans: an aggregate's own argument, which is a column of a temporary
    /// table where the rows are materialized before they are aggregated.
    Copy,
    /// A subquery's result, where even a literal `true` is 1.
    Result,
}

/// What a value of an integer type is in a JSON document.
#[derive(PartialEq)]
enum Written {
    /// `true` or `false`, where the function that writes it reads the expression itself.
    Truth,
    Number,
    /// Not known: a function the engine does not list may be one MySQL holds for a truth value
    /// (`isnull(x)`, `strcmp(a, b)`, `json_contains(…)`, `x > all (…)`).
    Either,
}

struct Draft {
    ctx: usize,
    /// The WITH the probe starts with ([`carried`]).
    with: String,
    from: Vec<TableWithJoins>,
    /// Every enclosing `FROM`, outermost first, then this one, for a constructor whose `FROM`
    /// reads a correlated column; tried only when the narrow form fails to prepare.
    wider: Option<Vec<TableWithJoins>>,
    /// Where the wider form reads this `FROM` in a LATERAL derived table of its own
    /// ([`wrapped_text`]), so that what is around it is a query around it again.
    wrap: Option<Wrap>,
    items: Vec<Expr>,
}

/// How [`wrapped_text`] reads a `FROM` in a derived table of its own: where in the wider form it
/// begins, the WITH the probe starts with, and the one the derived table does.
struct Wrap {
    at: usize,
    with: String,
    inner_with: String,
}

impl Builder<'_> {
    /// Whether an aggregate call in the select list of a subquery over `from` is that subquery's
    /// own, for certain. An aggregate belongs to the innermost query one of the columns it reads
    /// comes from, so one over columns of an outer query alone is that query's, and the subquery
    /// is then a row per row of its FROM: NULL as a scalar where that is empty. The call is the
    /// subquery's when it reads no column, a column qualified with the name of one of the FROM's
    /// tables (a derived table's alias too), or a bare column one of those tables has: a table's
    /// by the catalog (`columns`), a derived table's or a WITH query's by the names its select
    /// list or its column list gives. Everything else proves nothing: a table of another schema,
    /// an alias with a column list on a table, a qualifier in another case than the alias, a
    /// column a derived table selects by an expression's text or a wildcard, and a call with a
    /// part this does not read, unless it reads a column of the FROM beside it.
    fn owns(&mut self, f: &ast::Function, from: &[TableWithJoins]) -> bool {
        let mut reads: Vec<Vec<String>> = Vec::new();
        let mut whole = f.filter.is_none() && f.within_group.is_empty();
        match &f.args {
            FunctionArguments::None => {}
            FunctionArguments::List(list) => {
                whole &= list.clauses.is_empty();
                for arg in &list.args {
                    match arg {
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => whole &= references(e, &mut reads),
                        // `count(*)` reads no column.
                        FunctionArg::Unnamed(FunctionArgExpr::Wildcard) => {}
                        _ => whole = false,
                    }
                }
            }
            FunctionArguments::Subquery(_) => whole = false,
        }
        // The FROM's tables: the name that qualifies each one's columns, and where its columns
        // are known.
        let mut items: Vec<(String, Source)> = Vec::new();
        let mut factors: Vec<&TableFactor> = from.iter().flat_map(|t| std::iter::once(&t.relation).chain(t.joins.iter().map(|j| &j.relation))).collect();
        while let Some(factor) = factors.pop() {
            match factor {
                TableFactor::Table { name, alias, args: None, .. } => {
                    let table = match name.0.as_slice() {
                        [ast::ObjectNamePart::Identifier(i)] => Some(i.value.clone()),
                        _ => None,
                    };
                    let listed = alias.as_ref().is_some_and(|a| !a.columns.is_empty());
                    let source = match &table {
                        Some(_) if listed => Source::Unknown,
                        // A WITH query hides a table of its name. Only the one named exactly as
                        // the reference names it is surely the one read: a name in another case
                        // may be a table where table names are compared as written.
                        Some(t) => match self.ctes.iter().rev().find(|c| c.name.eq_ignore_ascii_case(t)) {
                            Some(cte) if cte.name == *t => Source::Columns(cte.columns.clone()),
                            Some(_) => Source::Unknown,
                            None => Source::Catalog(t.clone()),
                        },
                        None => Source::Unknown,
                    };
                    if let Some(qualifier) = alias.as_ref().map(|a| a.name.value.clone()).or(table) {
                        items.push((qualifier, source));
                    }
                }
                // A derived table's columns are the ones its select list names.
                TableFactor::Derived { subquery, alias: Some(alias), .. } => items.push((alias.name.value.clone(), Source::Columns(output_columns(subquery, &alias.columns)))),
                TableFactor::JsonTable { alias: Some(alias), .. } => items.push((alias.name.value.clone(), Source::Unknown)),
                TableFactor::NestedJoin { table_with_joins, alias: None } => {
                    factors.push(&table_with_joins.relation);
                    factors.extend(table_with_joins.joins.iter().map(|j| &j.relation));
                }
                _ => {}
            }
        }
        let mut own = false;
        for read in &reads {
            match read.as_slice() {
                [column] => {
                    let column = column.to_ascii_lowercase();
                    for (_, source) in &items {
                        match source {
                            Source::Catalog(table) => match self.columns.get(table) {
                                Some(columns) => own |= columns.contains(&column),
                                None => self.wanted.push(table.clone()),
                            },
                            Source::Columns(columns) => own |= columns.contains(&column),
                            Source::Unknown => {}
                        }
                    }
                }
                [qualifier, _] => own |= items.iter().any(|(name, _)| name == qualifier),
                _ => {}
            }
        }
        own || (whole && reads.is_empty())
    }

    fn walk(&mut self, e: &Expr) -> Shape {
        if self.overflowed {
            return Shape::Unknown;
        }
        match e {
            Expr::Nested(inner) => self.walk(inner),
            Expr::Function(f) => {
                let Some(args) = args(f) else { return self.leaf(e) };
                match name(f).as_str() {
                    "json_object" => {
                        if args.len() % 2 != 0 {
                            return Shape::Unknown;
                        }
                        let reads = if self.splits(e) { Reads::Split } else { Reads::Itself };
                        let mut fields: Vec<(String, Shape)> = Vec::with_capacity(args.len() / 2);
                        for pair in args.chunks(2) {
                            let Some(key) = string_const(pair[0]) else { return Shape::Unknown };
                            let value = self.walk_as(pair[1], reads);
                            // A key given twice holds the last value given for it.
                            match fields.iter_mut().find(|(k, _)| *k == key) {
                                Some(field) => field.1 = value,
                                None => fields.push((key, value)),
                            }
                        }
                        Shape::Object { fields, non_null: true }
                    }
                    // An aggregate over no rows is NULL, so the array is non-null only under a
                    // guard, or under a plain GROUP BY, where every group has a row, for an
                    // aggregate of the statement's own SELECT that is no window function. One
                    // inside a subquery may be the outer query's, which says nothing of its rows.
                    "json_arrayagg" => {
                        if args.len() != 1 {
                            return Shape::Unknown;
                        }
                        let non_null = self.grouped && self.stack.len() == 1 && f.over.is_none();
                        Shape::Array { items: Box::new(self.walk_as(args[0], Reads::Copy)), non_null }
                    }
                    // The value is the first argument that is not NULL. A constructor that is
                    // never NULL is the value whatever follows it; one that may be NULL keeps its
                    // shape only where every other argument has it too, which the engine knows of
                    // an empty array alone: `json_array(null)` has an item that is null, a `?` is
                    // whatever is bound, and `cast('x' as date)` is NULL.
                    "coalesce" | "ifnull" => {
                        let Some(first) = args.first().filter(|first| is_constructor(first)) else { return self.leaf(e) };
                        match self.walk(first) {
                            Shape::Array { items, non_null: false } if args.len() > 1 && args[1..].iter().all(|a| is_empty_array(a)) => Shape::Array { items, non_null: true },
                            shape @ (Shape::Object { non_null: true, .. } | Shape::Array { non_null: true, .. }) => shape,
                            shape if args.len() == 1 => shape,
                            _ => Shape::Unknown,
                        }
                    }
                    _ => self.leaf(e),
                }
            }
            // A cast to JSON leaves a constructor's document as it is, writes a truth value it
            // reads itself as `true` or `false`, and parses a string: that one is whatever the
            // server types the cast as.
            Expr::Cast { expr, data_type: DataType::JSON, .. } if is_constructor(expr) => self.walk(expr),
            Expr::Cast { expr, data_type: DataType::JSON, .. } if written(expr) == Written::Truth => self.walk_as(expr, Reads::Itself),
            Expr::Subquery(q) => {
                let SetExpr::Select(select) = q.body.as_ref() else { return Shape::Unknown };
                let Some(inner) = single_projection(q) else { return Shape::Unknown };
                let outer_ctes = self.ctes.len();
                self.ctes.extend(with_names(q));
                self.next_ctx += 1;
                self.stack.push(Level { ctx: self.next_ctx, from: select.from.clone(), with: q.with.clone() });
                let shape = self.walk_as(inner, Reads::Result);
                self.stack.pop();
                // A scalar subquery is one row only when it is an ungrouped aggregate of its own
                // (`owns`); anything else can match nothing and hand back NULL, and for a plain
                // value the server's own (nullable) verdict stands. A LIMIT is left out here, as
                // it was before the row count learned that a constant one of a row or more keeps
                // the row.
                let one_row = q.limit_clause.is_none() && crate::mysql::exactly_one(q).is_ok() && crate::mysql::aggregate_where(inner, &mut |f| self.owns(f, &select.from));
                self.ctes.truncate(outer_ctes);
                if one_row {
                    shape
                } else {
                    match shape {
                        Shape::Column { .. } => Shape::Unknown,
                        other => force_nullable(other),
                    }
                }
            }
            _ => self.leaf(e),
        }
    }

    /// Whether a call may have an aggregate or a window function of its own query among its
    /// arguments, at any depth. Every aggregate its text names counts, but for those of a scalar
    /// subquery that are all that subquery's own.
    fn splits(&mut self, e: &Expr) -> bool {
        let (mut calls, window) = aggregates_in(&e.to_string());
        if window {
            return true;
        }
        let mut inner = Vec::new();
        subqueries(e, &mut inner);
        for q in inner {
            if self.keeps_its_aggregates(q) {
                calls = calls.saturating_sub(aggregates_in(&q.to_string()).0);
            }
        }
        calls > 0
    }

    /// Whether every aggregate a scalar subquery's text names is its own for certain
    /// ([`Builder::owns`]) or, likewise, a subquery's inside its one select item.
    fn keeps_its_aggregates(&mut self, q: &Query) -> bool {
        let (mut calls, window) = aggregates_in(&q.to_string());
        let SetExpr::Select(select) = q.body.as_ref() else { return calls == 0 && !window };
        let Some(item) = single_projection(q) else { return calls == 0 && !window };
        let outer_ctes = self.ctes.len();
        self.ctes.extend(with_names(q));
        let mut own = 0;
        crate::mysql::aggregate_where(item, &mut |f| {
            own += usize::from(self.owns(f, &select.from));
            false
        });
        let mut inner = Vec::new();
        subqueries(item, &mut inner);
        for q in inner {
            if self.keeps_its_aggregates(q) {
                calls = calls.saturating_sub(aggregates_in(&q.to_string()).0);
            }
        }
        self.ctes.truncate(outer_ctes);
        !window && calls == own
    }

    /// Walk a value that the function writing it reads as `reads`.
    fn walk_as(&mut self, e: &Expr, reads: Reads) -> Shape {
        let outer = std::mem::replace(&mut self.reads, reads);
        let shape = self.walk(e);
        self.reads = outer;
        shape
    }

    /// One value expression, as a column of the probe for the constructor's own `FROM` context.
    fn leaf(&mut self, e: &Expr) -> Shape {
        let ctx = self.stack[self.stack.len() - 1].ctx;
        let probe = match self.drafts.iter().position(|d| d.ctx == ctx) {
            Some(i) => i,
            None => match self.draft() {
                Some(i) => i,
                None => return Shape::Unknown,
            },
        };
        let index = self.drafts[probe].items.len();
        self.drafts[probe].items.push(e.clone());
        let itself = match self.reads {
            Reads::Itself => true,
            Reads::Split => crate::mysql::has_aggregate(e) || is_boolean_literal(e),
            Reads::Copy => is_boolean_literal(e),
            Reads::Result => false,
        };
        match written(e) {
            Written::Truth if itself => self.booleans.push((probe, index)),
            Written::Number => {}
            _ => self.either.push((probe, index)),
        }
        Shape::Column { probe, index }
    }

    fn draft(&mut self) -> Option<usize> {
        if self.drafts.len() >= MAX_PROBES {
            self.overflowed = true;
            return None;
        }
        let depth = self.stack.len() - 1;
        let Level { ctx, from, .. } = self.stack[depth].clone();
        // Without a WITH a query reads, the probe would fail to prepare, or read a table where
        // the query reads a WITH query of the table's name.
        let with = carried(&self.stack)?;
        // An enclosing `FROM` moved into the probe reads the WITH queries of the levels inside
        // it too, which the statement does not let it see: where it names one of them, it may
        // read that query in place of its own table, so the wider form is left out.
        let mut inner: Vec<String> = Vec::new();
        let mut sound = true;
        for d in (0..depth).rev() {
            inner.extend(self.stack[d + 1].with.iter().flat_map(|w| w.cte_tables.iter().map(|c| c.alias.name.value.clone())));
            sound &= inner.is_empty() || !self.stack[d].from.iter().any(|t| mentions(&t.to_string(), &inner));
        }
        // The enclosing FROMs come first, outermost first, and each derived table of one inside
        // the statement's is LATERAL: one may read a query around it (`(select … from (select …
        // where c.post_id = p.id) c)`), which in the probe is a table left of it.
        let mut wider: Vec<TableWithJoins> = Vec::new();
        for (d, level) in self.stack[..depth].iter().enumerate() {
            wider.extend(level.from.iter().cloned().map(|t| if d == 0 { t } else { lateral(t) }));
        }
        // A join whose ON reads a query around it, which no table beside the join can stand in
        // for, and a WITH query of the subquery's own that does, which a WITH at the front of the
        // probe cannot, are read in a derived table of their own. The subquery's WITH goes inside
        // it, where no enclosing FROM reads it, and only where no level around has one: MySQL
        // hides an outer WITH inside a query that has its own.
        let own = depth > 0 && self.stack[depth].with.is_some();
        let outer = self.stack[..depth].iter().any(|l| l.with.is_some());
        let wrap = (depth > 0 && !wider.is_empty() && !from.is_empty() && (from.iter().any(correlated_on) || (own && !outer))).then(|| Wrap {
            at: wider.len(),
            with: if own { String::new() } else { with.clone() },
            inner_with: if own { self.stack[depth].with.as_ref().map(|w| format!("{w} ")).unwrap_or_default() } else { String::new() },
        });
        let sound = sound || wrap.as_ref().is_some_and(|w| !w.inner_with.is_empty());
        wider.extend(from.iter().cloned().map(|t| if depth == 0 || wrap.is_some() { t } else { lateral(t) }));
        // A subquery with no `FROM` of its own only ever resolves against the enclosing one.
        let (from, wider) = match (from.is_empty() && !wider.is_empty(), sound) {
            (true, true) => (wider, None),
            (true, false) => return None,
            (false, true) => (from, Some(wider)),
            (false, false) => (from, None),
        };
        let wider = wider.filter(|w| w.len() != from.len());
        let wrap = wrap.filter(|_| wider.is_some());
        self.drafts.push(Draft { ctx, with, from, wider, wrap, items: Vec::new() });
        Some(self.drafts.len() - 1)
    }
}

fn force_nullable(shape: Shape) -> Shape {
    match shape {
        Shape::Object { fields, .. } => Shape::Object { fields, non_null: false },
        Shape::Array { items, .. } => Shape::Array { items, non_null: false },
        other => other,
    }
}

/// Whether an expression is one the shapes speak for, down to a constructor at its core.
pub fn is_constructor(e: &Expr) -> bool {
    match e {
        Expr::Nested(inner) => is_constructor(inner),
        Expr::Function(f) => match name(f).as_str() {
            "json_object" | "json_arrayagg" => true,
            "coalesce" | "ifnull" => args(f).and_then(|a| a.first().copied()).is_some_and(is_constructor),
            _ => false,
        },
        Expr::Cast { expr, data_type: DataType::JSON, .. } => is_constructor(expr),
        Expr::Subquery(q) => single_projection(q).is_some_and(is_constructor),
        _ => false,
    }
}

fn name(f: &ast::Function) -> String {
    crate::mysql::builtin(f)
}

/// The plain expression arguments of a call; `None` for a wildcard, a named argument or a subquery form.
fn args(f: &ast::Function) -> Option<Vec<&Expr>> {
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

fn single_projection(q: &Query) -> Option<&Expr> {
    let SetExpr::Select(select) = q.body.as_ref() else { return None };
    match select.projection.as_slice() {
        [SelectItem::UnnamedExpr(e)] | [SelectItem::ExprWithAlias { expr: e, .. }] => Some(e),
        _ => None,
    }
}

fn string_const(e: &Expr) -> Option<String> {
    match e {
        Expr::Value(v) => match &v.value {
            Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => Some(s.clone()),
            _ => None,
        },
        Expr::Nested(inner) => string_const(inner),
        _ => None,
    }
}

/// What an expression the server types as an integer is in a JSON document (measured on 8.4).
/// A parenthesis and a unary plus are not there for the server.
fn written(e: &Expr) -> Written {
    use ast::{BinaryOperator as B, UnaryOperator as U};
    // What the server computes as a number whatever its arguments are: `coalesce(a = b, false)`
    // and `max(a = b)` are 1 or 0.
    const NUMBERS: &[&str] = &[
        "count", "sum", "min", "max", "bit_and", "bit_or", "bit_xor", "coalesce", "ifnull", "if", "nullif", "abs", "length", "char_length", "character_length", "floor",
        "ceil", "ceiling", "round", "mod", "json_length", "json_depth", "year", "month", "day", "dayofmonth", "hour", "minute", "second", "datediff", "timestampdiff",
        "unix_timestamp", "last_insert_id", "row_number", "rank", "dense_rank",
    ];
    match e {
        Expr::Nested(inner) | Expr::UnaryOp { op: U::Plus, expr: inner } => written(inner),
        Expr::Value(v) => match v.value {
            Value::Boolean(_) => Written::Truth,
            Value::Number(..) => Written::Number,
            _ => Written::Either,
        },
        Expr::BinaryOp { op, .. } => match op {
            B::Eq | B::NotEq | B::Lt | B::LtEq | B::Gt | B::GtEq | B::Spaceship | B::And | B::Or | B::Xor => Written::Truth,
            B::Plus | B::Minus | B::Multiply | B::Divide | B::Modulo | B::MyIntegerDivide => Written::Number,
            _ => Written::Either,
        },
        Expr::UnaryOp { op: U::Not, .. } => Written::Truth,
        Expr::UnaryOp { op: U::Minus, .. } => Written::Number,
        Expr::IsNull(_) | Expr::IsNotNull(_) | Expr::IsTrue(_) | Expr::IsNotTrue(_) | Expr::IsFalse(_) | Expr::IsNotFalse(_) | Expr::IsUnknown(_) | Expr::IsNotUnknown(_) => Written::Truth,
        Expr::InList { .. } | Expr::InSubquery { .. } | Expr::Between { .. } | Expr::Like { .. } | Expr::RLike { .. } | Expr::Exists { .. } => Written::Truth,
        // A column is a number, whatever the expression a view or a derived table gives it.
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) | Expr::Case { .. } | Expr::Cast { .. } => Written::Number,
        Expr::Function(f) if NUMBERS.contains(&name(f).as_str()) => Written::Number,
        _ => Written::Either,
    }
}

/// Per result column of a statement the engine reads as the server does, whether it is a truth
/// value, which MySQL computes as the integer 1 or 0 (or NULL) and nothing else: a comparison,
/// `IS`, `IN`, `BETWEEN`, `LIKE`, `REGEXP`, `EXISTS`, `NOT`, `AND`, `OR`, `XOR`, `TRUE` or `FALSE`,
/// and a `CASE`, `IF`, `COALESCE`, `IFNULL`, `NULLIF` or scalar subquery whose every value is one
/// (a NULL among them). Every arm of a set operation must agree. `width` is the result's columns:
/// the items before a wildcard count from the start and those after the last one from the end,
/// and what a wildcard expands to is no truth value. Empty where a form the walk does not read
/// leaves the positions unknown.
pub fn truths(q: &Query, width: usize) -> Vec<bool> {
    let Some(items) = body_truths(q.body.as_ref()) else { return Vec::new() };
    let head = items.iter().take_while(|i| i.is_some()).count();
    if head == items.len() {
        return if items.len() == width { items.into_iter().map(|b| b == Some(Bit::Truth)).collect() } else { Vec::new() };
    }
    let tail = items.iter().rev().take_while(|i| i.is_some()).count();
    if head + tail > width {
        return Vec::new();
    }
    (0..width)
        .map(|i| {
            let item = if i < head { items[i] } else if i >= width - tail { items[items.len() - (width - i)] } else { None };
            item == Some(Bit::Truth)
        })
        .collect()
}

/// What an expression computes, as far as a truth value goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bit {
    Truth,
    Null,
    Other,
}

/// Each select item's [`Bit`], `None` for a wildcard; `None` for a body the walk does not read,
/// and for a set operation with a wildcard, whose arms may line up otherwise.
fn body_truths(body: &SetExpr) -> Option<Vec<Option<Bit>>> {
    match body {
        SetExpr::Select(select) => Some(
            select
                .projection
                .iter()
                .map(|item| match item {
                    SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => Some(bit(e)),
                    _ => None,
                })
                .collect(),
        ),
        SetExpr::Query(q) => body_truths(q.body.as_ref()),
        SetExpr::SetOperation { left, right, .. } => {
            let (left, right) = (body_truths(left)?, body_truths(right)?);
            let whole = |side: &[Option<Bit>]| side.iter().all(Option::is_some);
            (left.len() == right.len() && whole(&left) && whole(&right)).then(|| left.into_iter().zip(right).map(|(l, r)| Some(both(&[l?, r?]))).collect())
        }
        _ => None,
    }
}

/// A value that is one of `bits`: a truth value where each is one or NULL, and one is.
fn both(bits: &[Bit]) -> Bit {
    if bits.iter().any(|b| *b == Bit::Other) {
        Bit::Other
    } else if bits.contains(&Bit::Truth) {
        Bit::Truth
    } else {
        Bit::Null
    }
}

/// Whether the server computes an expression as a truth value. A parenthesis and a unary plus are
/// not there for the server; a function is the built-in only under its plain name ([`name`]).
fn bit(e: &Expr) -> Bit {
    use ast::{BinaryOperator as B, UnaryOperator as U};
    match e {
        Expr::Nested(inner) | Expr::UnaryOp { op: U::Plus, expr: inner } => bit(inner),
        Expr::Value(v) => match v.value {
            Value::Boolean(_) => Bit::Truth,
            Value::Null => Bit::Null,
            _ => Bit::Other,
        },
        Expr::BinaryOp { op: B::Eq | B::NotEq | B::Lt | B::LtEq | B::Gt | B::GtEq | B::Spaceship | B::And | B::Or | B::Xor, .. } => Bit::Truth,
        Expr::UnaryOp { op: U::Not, .. } => Bit::Truth,
        Expr::IsNull(_) | Expr::IsNotNull(_) | Expr::IsTrue(_) | Expr::IsNotTrue(_) | Expr::IsFalse(_) | Expr::IsNotFalse(_) | Expr::IsUnknown(_) | Expr::IsNotUnknown(_) => Bit::Truth,
        Expr::InList { .. } | Expr::InSubquery { .. } | Expr::Between { .. } | Expr::Like { .. } | Expr::RLike { .. } | Expr::Exists { .. } => Bit::Truth,
        // A scalar subquery is its one item's value, or NULL where it finds no row.
        Expr::Subquery(q) => match body_truths(q.body.as_ref()).as_deref() {
            Some([Some(b)]) => both(&[*b, Bit::Null]),
            _ => Bit::Other,
        },
        Expr::Case { conditions, else_result, .. } => {
            let mut bits: Vec<Bit> = conditions.iter().map(|when| bit(&when.result)).collect();
            bits.push(else_result.as_deref().map_or(Bit::Null, bit));
            both(&bits)
        }
        Expr::Function(f) if f.over.is_none() => {
            let Some(args) = args(f) else { return Bit::Other };
            match (name(f).as_str(), args.as_slice()) {
                ("coalesce", [_, ..]) | ("ifnull", [_, _]) => both(&args.iter().map(|a| bit(a)).collect::<Vec<_>>()),
                ("if", [_, then, otherwise]) => both(&[bit(then), bit(otherwise)]),
                ("nullif", [first, _]) => both(&[bit(first), Bit::Null]),
                _ => Bit::Other,
            }
        }
        _ => Bit::Other,
    }
}

fn is_boolean_literal(e: &Expr) -> bool {
    match e {
        Expr::Nested(inner) => is_boolean_literal(inner),
        Expr::Value(v) => matches!(v.value, Value::Boolean(_)),
        _ => false,
    }
}

/// How many aggregate calls a text has, a name inside a string among them, and whether it may
/// have a window function.
fn aggregates_in(text: &str) -> (usize, bool) {
    let text = text.to_ascii_lowercase();
    let calls = |name: &&str| text.match_indices(&format!("{name}(")).filter(|(at, _)| !text[..*at].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_')).count();
    (crate::mysql::AGGREGATES.iter().map(calls).sum(), text.contains(" over ") || text.contains(" over("))
}

/// The scalar subqueries among an expression's parts, as far as the walk reads; it does not
/// look inside one.
fn subqueries<'e>(e: &'e Expr, out: &mut Vec<&'e Query>) {
    match e {
        Expr::Subquery(q) => out.push(q),
        Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } | Expr::Cast { expr: inner, .. } | Expr::IsNull(inner) | Expr::IsNotNull(inner) => subqueries(inner, out),
        Expr::BinaryOp { left, right, .. } => {
            subqueries(left, out);
            subqueries(right, out);
        }
        Expr::Function(f) => {
            for arg in args(f).unwrap_or_default() {
                subqueries(arg, out);
            }
        }
        _ => {}
    }
}

/// An array that is empty and never NULL: `json_array()`, or the text `[]` cast to JSON.
fn is_empty_array(e: &Expr) -> bool {
    match e {
        Expr::Nested(inner) => is_empty_array(inner),
        Expr::Function(f) => name(f) == "json_array" && args(f).is_some_and(|a| a.is_empty()) && f.over.is_none(),
        Expr::Cast { expr, data_type: DataType::JSON, .. } => matches!(expr.as_ref(), Expr::Value(v) if matches!(&v.value, Value::SingleQuotedString(s) if s.split_whitespace().collect::<String>() == "[]")),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::ast::Statement;
    use sqlparser::dialect::MySqlDialect;
    use sqlparser::parser::Parser;

    fn query(sql: &str) -> Query {
        let mut v = Parser::parse_sql(&MySqlDialect {}, sql).unwrap();
        let Statement::Query(q) = v.pop().unwrap() else { panic!("not a query") };
        *q
    }

    #[test]
    fn nested_fetch_plans_two_probes() {
        let q = query("select u.id, (select coalesce(json_arrayagg(json_object('id', p.id, 'title', p.title)), json_array()) from posts p where p.user_id = u.id) as posts, json_object('email', u.email, 'n', u.id + ?) as meta from users u where u.id = ?");
        let plan = plan(&q, &Columns::new());
        assert_eq!(plan.probes.len(), 2);
        assert_eq!(plan.probes[0].sql, "SELECT p.id, p.title FROM posts p");
        assert_eq!(plan.probes[0].wider.as_deref(), Some("SELECT p.id, p.title FROM users u, posts p"));
        assert_eq!(plan.probes[1].sql, "SELECT u.email, u.id + ? FROM users u");
        assert_eq!(plan.probes[1].wider, None);
        let posts = Shape::Array { items: Box::new(Shape::Object { fields: vec![("id".into(), Shape::Column { probe: 0, index: 0 }), ("title".into(), Shape::Column { probe: 0, index: 1 })], non_null: true }), non_null: true };
        let meta = Shape::Object { fields: vec![("email".into(), Shape::Column { probe: 1, index: 0 }), ("n".into(), Shape::Column { probe: 1, index: 1 })], non_null: true };
        assert_eq!(plan.columns, vec![("posts".to_string(), posts), ("meta".to_string(), meta)]);
    }

    #[test]
    fn a_truth_value_inside_a_constructor_is_marked_boolean() {
        let q = query("select json_object('t', true, 'c', u.id > 0, 'in', u.id in (1, 2), 'not', not (u.id = 1), 'n', u.id, 's', u.name) as o from users u");
        let plan = plan(&q, &Columns::new());
        assert_eq!(plan.probes[0].sql, "SELECT true, u.id > 0, u.id IN (1, 2), NOT (u.id = 1), u.id, u.name FROM users u");
        assert_eq!(plan.booleans, vec![(0, 0), (0, 1), (0, 2), (0, 3)]);
    }

    #[test]
    fn a_truth_value_the_function_may_read_a_copy_of_is_either() {
        let marks = |sql: &str| {
            let plan = plan(&query(sql), &Columns::new());
            (plan.booleans, plan.either)
        };
        // Beside an aggregate each argument without one is copied, a literal aside; an expression
        // the engine does not know for a number may be a truth value anywhere.
        assert_eq!(marks("select json_object('t', u.id = 1, 'm', max(u.n), 'c', max(u.id) = 1, 'l', true, 'x', isnull(u.n), 'n', u.n) as o from users u group by u.id"), (vec![(0, 2), (0, 3)], vec![(0, 0), (0, 4)]));
        assert_eq!(marks("select json_object('t', u.id = 1, 'w', row_number() over ()) as o from users u"), (vec![], vec![(0, 0)]));
        // An aggregate's own argument is a copy in some plans; a document inside it reads its own.
        assert_eq!(marks("select json_arrayagg(u.id = 1) as o from users u"), (vec![], vec![(0, 0)]));
        assert_eq!(marks("select json_arrayagg(true) as o from users u"), (vec![(0, 0)], vec![]));
        assert_eq!(marks("select json_arrayagg(json_object('t', u.id = 1)) as o from users u"), (vec![(0, 0)], vec![]));
        assert_eq!(marks("select json_arrayagg(cast(u.id = 1 as json)) as o from users u"), (vec![(0, 0)], vec![]));
        assert_eq!(marks("select json_object('o', json_object('t', u.id = 1), 'm', max(u.n)) as o from users u"), (vec![(0, 0)], vec![]));
        // A subquery's result is a copy, a literal too.
        assert_eq!(marks("select json_object('t', (select p.id = 1 from posts p limit 1), 'l', (select true)) as o from users u"), (vec![], vec![(0, 0), (1, 0)]));
        // A subquery's own aggregates split nothing outside it; one that may be the outer query's does.
        let nested = |inner: &str| format!("select json_object('t', u.id = 1, 'ps', (select coalesce(json_arrayagg(json_object('t', p.id = 1, 'cs', (select json_arrayagg({inner}) from comments c where c.post_id = p.id))), json_array()) from posts p where p.user_id = u.id)) as o from users u");
        assert_eq!(marks(&nested("c.id")), (vec![(0, 0), (1, 0)], vec![]));
        assert_eq!(marks(&nested("u.id")), (vec![], vec![(0, 0), (1, 0)]));
        assert_eq!(marks("select json_object('t', u.id = 1, 'n', (select 1 from posts p where p.id = max(u.id))) as o from users u"), (vec![], vec![(0, 0)]));
    }

    #[test]
    fn a_guard_keeps_the_shape_it_has_itself() {
        let shape = |item: &str| plan(&query(&format!("select {item} as o from users u")), &Columns::new()).columns.pop().map(|(_, shape)| shape);
        let list = |non_null: bool| Some(Shape::Array { items: Box::new(Shape::Column { probe: 0, index: 0 }), non_null });
        assert_eq!(shape("json_arrayagg(u.id)"), list(false));
        for guard in ["json_array()", "cast('[]' as json)", "cast(' [ ] ' as json)", "json_array(), (json_array())"] {
            assert_eq!(shape(&format!("coalesce(json_arrayagg(u.id), {guard})")), list(true), "{guard}");
        }
        assert_eq!(shape("ifnull(json_arrayagg(u.id), json_array())"), list(true));
        // Anything else may be the value, or NULL.
        for guard in ["json_array(null)", "json_array(1)", "json_object()", "'[]'", "?", "cast('null' as json)", "cast('x' as date)", "null, json_array()", "app.json_array()"] {
            assert_eq!(shape(&format!("coalesce(json_arrayagg(u.id), {guard})")), None, "{guard}");
        }
        assert_eq!(shape("coalesce((select json_object('id', p.id) from posts p limit 1), json_object())"), None);
        // A constructor that is never NULL is the value whatever follows it.
        assert!(matches!(shape("coalesce(json_object('id', u.id), ?)"), Some(Shape::Object { non_null: true, .. })));
        // A guard over anything else is one value, of the type the server gives the whole call.
        let plan = plan(&query("select json_object('n', coalesce(u.n, 'none'), 'j', cast(u.name as json), 't', cast(u.id = 1 as json)) as o from users u"), &Columns::new());
        assert_eq!(plan.probes[0].sql, "SELECT coalesce(u.n, 'none'), CAST(u.name AS JSON), u.id = 1 FROM users u");
        assert_eq!(plan.booleans, vec![(0, 2)]);
    }

    #[test]
    fn a_key_given_twice_holds_its_last_value() {
        let plan = plan(&query("select json_object('id', u.id, 'name', u.name, 'id', u.email) as o from users u"), &Columns::new());
        let fields = vec![("id".to_string(), Shape::Column { probe: 0, index: 2 }), ("name".to_string(), Shape::Column { probe: 0, index: 1 })];
        assert_eq!(plan.columns, vec![("o".to_string(), Shape::Object { fields, non_null: true })]);
    }

    #[test]
    fn subquery_items_are_planned_and_left_out_of_the_outer_probe() {
        let q = query("select u.id, u.name, (select count(*) from posts p where p.user_id = u.id) as n, (select p.id from posts p where p.user_id = u.id limit 1) as first from users u where u.id = ?");
        let plan = plan(&q, &Columns::new());
        // The exactly-one aggregate keeps its column; the `limit 1` fetch falls back to the server's verdict.
        assert_eq!(plan.columns, vec![("n".to_string(), Shape::Column { probe: 0, index: 0 })]);
        assert_eq!(plan.probes[0].sql, "SELECT count(*) FROM posts p");
        let (sql, kept) = outer_probe(&q).unwrap();
        assert_eq!(sql, "SELECT u.id, u.name FROM users u WHERE u.id = ?");
        assert_eq!(kept, vec![0, 1]);
        assert!(outer_probe(&query("select u.id, u.name from users u")).is_none());
        assert!(outer_probe(&query("select u.*, (select count(*) from posts) as n from users u")).is_none());
    }

    #[test]
    fn an_aggregate_over_outer_columns_leaves_the_subquery_nullable() {
        let columns: Columns = [("posts", vec!["id", "user_id", "title"]), ("users", vec!["id", "name"])].into_iter().map(|(t, c)| (t.to_string(), c.into_iter().map(String::from).collect())).collect();
        // The first column's shape, and the tables whose columns the plan lacked.
        let first = |sql: &str, columns: &Columns| {
            let plan = plan(&query(sql), columns);
            (plan.columns.first().map(|(_, shape)| shape.clone()), plan.wanted)
        };
        let kept = |sql: &str| matches!(first(sql, &columns).0, Some(Shape::Column { .. } | Shape::Array { non_null: true, .. }));
        // The subquery's own: no column, a column of its FROM by qualifier, a bare one a table of
        // its FROM has, and any of those beside an outer column.
        for sql in [
            "select (select count(*) from posts p where p.user_id = u.id) as n from users u",
            "select (select count(p.id) from posts p where p.user_id = u.id) as n from users u",
            "select (select count(posts.id) from posts) as n from users u",
            "select (select count(title) from posts where user_id = u.id) as n from users u",
            "select (select coalesce(sum(length(title)), 0) from posts) as n from users u",
            "select (select count(distinct title) from posts) as n from users u",
            "select (select count(length(name) + length(title)) from posts) as n from users u",
            "select (select coalesce(json_arrayagg(title), json_array()) from posts) as titles from users u",
            "select (select group_concat(title order by name) from posts) as n from users u",
            // A derived table's or a WITH query's columns, by its alias or by the names it gives them.
            "select (select coalesce(json_arrayagg(json_object('id', p.id, 'title', p.title)), json_array()) from (select id, title from posts order by id desc limit 5) p) as recent from users u",
            "select (select ifnull(json_arrayagg(p.id), json_array()) from (select id from posts) p) as ids from users u",
            "select (select count(title) from (select title from posts) p) as n from users u",
            "select (select count(t) from (select p.title as t from posts p) p) as n from users u",
            "select (select count(n) from (select id from posts) p (n)) as n from users u",
            "select (select count(id) from (select id from posts union select id from users) p) as n from users u",
            "select (select count(x.id) from (posts p join (select id from users) x on x.id = p.user_id)) as n from users u",
            "with recent as (select id, title from posts limit 5) select (select count(r.title) from recent r) as n from users u",
            "with recent as (select id, title from posts limit 5) select (select count(title) from recent) as n from users u",
            "with recent (a, b) as (select id, title from posts) select (select count(b) from recent) as n from users u",
            "select (select count(n) from (with recent as (select id as n from posts) select n from recent) p) as n from users u",
            "select (with recent as (select id from posts) select count(id) from recent) as n from users u",
        ] {
            assert!(kept(sql), "{sql}");
        }
        // The outer query's, or not provably the subquery's.
        for sql in [
            "select (select count(u.id) from posts) as n from users u",
            "select (select count(name) from posts) as n from users u",
            "select (select coalesce(max(length(name)), 0) from posts) as n from users u",
            "select (select coalesce(json_arrayagg(name), json_array()) from posts) as names from users u",
            "select (select coalesce(json_arrayagg(u.name), json_array()) from posts) as names from users u",
            // The qualifier is another name than the alias, in case too.
            "select (select count(posts.id) from posts p) as n from users posts",
            "select (select count(P.id) from posts p) as n from users P",
            // A WITH query or a derived table that lacks the column, one that selects it by its
            // text or a wildcard, a WITH query named in another case, and a table of another
            // schema do not say the column is theirs.
            "with posts as (select id from users) select (select count(title) from posts) as n from users u",
            "select (select count(name) from (select title from posts) p) as n from users u",
            "select (select count(title) from (select title from posts) p (t)) as n from users u",
            "select (select count(name) from (select upper(title) from posts) p) as n from users u",
            "select (select count(name) from (select * from posts) p) as n from users u",
            "select (select count(u.name) from (select title from posts) p) as n from users u",
            "with Recent as (select name from posts) select (select count(name) from recent) as n from users u",
            "select (select count(title) from app.posts) as n from users u",
            "select (select count(title) from posts p (a, b, c)) as n from users u",
            // A part of the call this does not read.
            "select (select count(case when title is null then 1 end) from posts) as n from users u",
            "select (select group_concat(name order by title) from posts) as n from users u",
        ] {
            assert!(!kept(sql), "{sql}");
        }
        // A constructor qualified with a schema is a stored function of that name.
        assert!(plan(&query("select app.json_object('id', u.id) as o from users u"), &columns).columns.is_empty());
        assert!(!kept("select (select app.count(p.id) from posts p) as n from users u"));
        // Without the table's columns a bare name proves nothing, and the plan says which table.
        let bare = "select (select count(title) from posts where user_id = u.id) as n from users u";
        assert_eq!(first(bare, &Columns::new()), (None, vec!["posts".to_string()]));
        assert_eq!(first(bare, &columns).1, Vec::<String>::new());
        assert_eq!(first("select (select count(p.id) from posts p) as n from users u", &Columns::new()).1, Vec::<String>::new());
    }

    #[test]
    fn a_json_aggregate_over_a_derived_table_is_typed_like_one_over_a_with_query() {
        // The shape of the column named `recent`.
        let recent = |sql: &str| plan(&query(sql), &Columns::new()).columns.into_iter().find(|(name, _)| name == "recent").map(|(_, shape)| shape);
        let never_null = |sql: &str| matches!(recent(sql), Some(Shape::Array { non_null: true, .. }));
        let object = "json_object('id', p.id, 'title', p.title)";
        for sql in [
            format!("select (select count(*) from users) as user_count, (select coalesce(json_arrayagg({object}), json_array()) from (select id, title from posts order by id desc limit 5) p) as recent"),
            format!("with p as (select id, title from posts order by id desc limit 5) select (select count(*) from users) as user_count, (select coalesce(json_arrayagg({object}), json_array()) from p) as recent"),
            format!("select (select coalesce(json_arrayagg({object}), json_array()) from (select id, title from posts) p) as recent"),
            format!("select (select ifnull(json_arrayagg({object}), json_array()) from (select id, title from posts limit 5) p) as recent"),
            format!("select coalesce(json_arrayagg({object}), json_array()) as recent from (select id, title from posts limit 5) p"),
            "select (select coalesce(json_arrayagg(json_object('id', id, 'title', title)), json_array()) from (select id, title from posts limit 5) p) as recent".to_string(),
        ] {
            assert!(never_null(&sql), "{sql}");
        }
        // Unguarded, the aggregate over no rows is NULL; an aggregate of the outer query's leaves
        // the subquery a row per row of its FROM, which may be none.
        for sql in [
            "select (select json_arrayagg(json_object('id', p.id)) from (select id from posts limit 5) p) as recent",
            "select (select coalesce(json_arrayagg(u.name), json_array()) from (select id from posts limit 1) p) as recent from users u",
            "select (select coalesce(json_arrayagg(name), json_array()) from (select id from posts limit 1) p) as recent from users u",
        ] {
            assert!(matches!(recent(sql), Some(Shape::Array { non_null: false, .. })), "{sql}");
        }
    }

    #[test]
    fn a_subquery_probe_carries_the_with_of_every_level() {
        let probes = |sql: &str| plan(&query(sql), &Columns::new()).probes.into_iter().map(|p| (p.sql, p.wider)).collect::<Vec<_>>();
        let probe = |sql: &str, narrow: &str, wider: Option<&str>| assert_eq!(probes(sql), vec![(narrow.to_string(), wider.map(String::from))], "{sql}");
        let posts = "WITH r AS (SELECT id FROM posts) ";
        // The subquery's own WITH, the statement's before it, and each nested subquery's after its parent's.
        probe("select (with r as (select id from posts) select count(id) from r) as n", &format!("{posts}SELECT count(id) FROM r"), None);
        probe("select (with r as (select id from posts) select count(id) from r) as n from users u", &format!("{posts}SELECT count(id) FROM r"), Some(&format!("SELECT reylo_probe.* FROM users u, LATERAL ({posts}SELECT count(id) AS a0 FROM r) AS reylo_probe")));
        probe("with o as (select 1 as a) select (with r as (select id from posts) select count(id) from r, o) as n", "WITH o AS (SELECT 1 AS a), r AS (SELECT id FROM posts) SELECT count(id) FROM r, o", None);
        probe("select (with a as (select id from posts) select (with b as (select id from a) select count(id) from b)) as n", "WITH a AS (SELECT id FROM posts), b AS (SELECT id FROM a) SELECT count(id) FROM b", None);
        probe("select (with r as (select id from posts) select count(id) from (select id from r) d) as n", &format!("{posts}SELECT count(id) FROM (SELECT id FROM r) d"), None);
        // Sibling subqueries each carry their own.
        assert_eq!(
            probes("select json_object('a', (with r as (select id from posts) select count(id) from r), 'b', (with s as (select title from posts) select count(title) from s)) as o"),
            vec![(format!("{posts}SELECT count(id) FROM r"), None), ("WITH s AS (SELECT title FROM posts) SELECT count(title) FROM s".to_string(), None)]
        );
        // RECURSIVE, from any level, is the whole list's.
        probe(
            "with o as (select 1 as a) select (with recursive r (n) as (select 1 union all select n + 1 from r where n < 3) select count(n) from r, o) as c",
            "WITH RECURSIVE o AS (SELECT 1 AS a), r (n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM r WHERE n < 3) SELECT count(n) FROM r, o",
            None,
        );
        // A WITH query that shadows a table is read in the table's place.
        probe("select (with posts as (select null as title) select count(title) from posts) as n", "WITH posts AS (SELECT NULL AS title) SELECT count(title) FROM posts", None);
        // A query without RECURSIVE that names itself reads a table, which a RECURSIVE list would
        // read as itself: no probe.
        assert_eq!(probes("with posts as (select id from posts) select (with recursive r (n) as (select 1 union all select n + 1 from r where n < 3) select count(n) from r, posts) as c"), vec![]);
        // An enclosing FROM that names an inner WITH query would read it in a wider form that
        // carried the query at its front, so the query goes inside the derived table it reads.
        probe(
            "select (with posts as (select id from users) select json_object('a', posts.id, 'b', p.title) from posts limit 1) as o from posts p",
            "WITH posts AS (SELECT id FROM users) SELECT posts.id, p.title FROM posts",
            Some("SELECT reylo_probe.* FROM posts p, LATERAL (WITH posts AS (SELECT id FROM users) SELECT posts.id AS a0, p.title AS a1 FROM posts) AS reylo_probe"),
        );
        // Under a level with a WITH of its own, which MySQL hides inside a query with its own,
        // there is no such derived table, and no wider form.
        probe(
            "with o as (select 1 as a) select (select (with posts as (select id from users) select json_object('a', posts.id, 'b', p.title) from posts limit 1) from posts p, o) as j",
            "WITH o AS (SELECT 1 AS a), posts AS (SELECT id FROM users) SELECT posts.id, p.title FROM posts",
            None,
        );
        assert_eq!(probes("select (with posts as (select 1 as id) select json_object('t', p.title)) as o from posts p"), vec![]);
        probe("select (with r as (select 1 as id) select json_object('t', p.title)) as o from posts p", "WITH r AS (SELECT 1 AS id) SELECT p.title FROM posts p", None);
    }

    #[test]
    fn a_correlated_from_is_read_after_the_queries_around_it() {
        let probes = |sql: &str| plan(&query(sql), &Columns::new()).probes.into_iter().map(|p| (p.sql, p.wider)).collect::<Vec<_>>();
        // A derived table that reads a query around it is LATERAL in the wider form, right of
        // the tables it reads.
        assert_eq!(
            probes("select (select json_arrayagg(json_object('id', c.id)) from (select id from comments c where c.post_id = p.id limit 2) c) as j from posts p"),
            vec![(
                "SELECT c.id FROM (SELECT id FROM comments c WHERE c.post_id = p.id LIMIT 2) c".to_string(),
                Some("SELECT c.id FROM posts p, LATERAL (SELECT id FROM comments c WHERE c.post_id = p.id LIMIT 2) c".to_string())
            )]
        );
        // A join whose ON reads a query around it is read in a derived table of its own, where
        // that query is one around it again: no table beside a join is in scope in its ON.
        assert_eq!(
            probes("select (select json_arrayagg(json_object('id', c.id, 'name', u.name)) from comments c join users u on u.id = c.user_id and c.post_id = p.id) as j from posts p")[0].1.as_deref(),
            Some("SELECT reylo_probe.* FROM posts p, LATERAL (SELECT c.id AS a0, u.name AS a1 FROM comments c JOIN users u ON u.id = c.user_id AND c.post_id = p.id) AS reylo_probe")
        );
        // One whose ON reads its own tables alone is no such join.
        assert_eq!(
            probes("select (select json_arrayagg(json_object('id', c.id, 'name', u.name)) from comments c join users u on u.id = c.user_id where c.post_id = p.id) as j from posts p")[0].1.as_deref(),
            Some("SELECT c.id, u.name FROM posts p, comments c JOIN users u ON u.id = c.user_id")
        );
    }

    #[test]
    fn a_scalar_subquery_that_can_miss_is_nullable() {
        let q = query("select (select json_object('id', p.id) from posts p where p.user_id = u.id limit 1) as first, json_arrayagg(u.id) as ids, u.name from users u");
        let plan = plan(&q, &Columns::new());
        assert!(matches!(plan.columns[0].1, Shape::Object { non_null: false, .. }));
        assert!(matches!(plan.columns[1].1, Shape::Array { non_null: false, .. }));
        assert_eq!(plan.columns.len(), 2);
    }
}
