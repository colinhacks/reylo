//! Nested JSON shapes, recovered from the SQL that builds them.
//!
//! A column built with `jsonb_build_object(…)` or `jsonb_agg(…)` describes as bare `jsonb` and
//! nothing more: the server reports the constructor's return type, never the shape it
//! constructed. The shape is not lost, though — the constructor's arguments are ordinary SQL
//! expressions in an ordinary `FROM` context, and the database can type those. So the build
//! reads the constructor out of the parse tree, synthesizes a statement that selects those
//! expressions as real columns, and describes it through the path every other statement takes.
//! The nested fields then get type oids with table-and-attribute provenance, which means the
//! same catalog `NOT NULL` and the same outer-join degradation apply inside the nested shape.
//!
//! Recognised: `json_build_object` / `jsonb_build_object`, `json_agg` / `jsonb_agg`,
//! `coalesce(json[b]_agg(…), '[]')` — which is what makes a nested array non-null —
//! `to_json` / `to_jsonb` / `row_to_json` over a whole row, and any nesting of those. SQLite
//! spells the first two `json_object` and `json_group_array`, which are recognised too; it has
//! no row constructor. Everything else is JSON the build cannot explain — a stored column, a
//! `||`, a function's return — and nothing is claimed for it.
//!
//! This module is pure: `plan` reads the parse tree and synthesizes statement text, `resolve`
//! turns the describes of that text into the column's `Type`. The build owns the round trips.
//! The framework reads it too, and renders the `Type` to JSON Schema itself.

use crate::dialect::Dialect;
use crate::infer::Trust;
use crate::levels::{Columns, Level, Withs};
use crate::pgtypes::{oid, TypeRegistry};
use crate::returning::Rows;
use crate::types::{DescribedColumn, Field, Type};
use pg_query::protobuf::{self, node::Node as NodeEnum, AStar, ColumnRef, Node, ParseResult, RawStmt, ResTarget, SelectStmt};
use serde_json::Value;

/// `json_build_object(k, v, …)` and its jsonb twin: the main case. `json_object` is SQLite's
/// spelling of it; Postgres's own `json_object` takes arrays, whose arguments are never the
/// literal keys this reads, so it falls through unrecognised there.
const OBJECT_FNS: &[&str] = &["json_build_object", "jsonb_build_object", "json_object"];
/// `json_agg(x)` and its jsonb twin: the same shape, as an array. `json_group_array` is
/// SQLite's, and it answers `[]` rather than NULL over an empty group.
const AGG_FNS: &[&str] = &["json_agg", "jsonb_agg", "json_group_array"];
/// The aggregates that are never NULL, so a nested array under one needs no `coalesce`.
const TOTAL_AGG_FNS: &[&str] = &["json_group_array"];
/// A whole row, so the shape is the relation's own columns.
const ROW_FNS: &[&str] = &["to_json", "to_jsonb", "row_to_json"];
/// Every function name the shapes read on Postgres, for the trust check ([`Trust::shadowed`]): a
/// name a user has also defined may resolve to that function, which builds what it likes.
pub const POSTGRES_FUNCTIONS: &[&str] = &["json_build_object", "jsonb_build_object", "json_object", "json_agg", "jsonb_agg", "json_group_array", "to_json", "to_jsonb", "row_to_json"];

/// How many synthesized statements one statement may cost. A nested fetch needs one; a shape
/// with an aggregate under an object needs two. The cap keeps a pathological statement from
/// turning one describe into dozens.
const MAX_PROBES: usize = 8;

pub use reylo_engine_core::shape::{Probe, Shape};

/// What one statement's JSON columns need: a shape per result column the build could explain,
/// and the statements to describe to fill it in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    /// Result column name → its shape, in target-list order.
    pub columns: Vec<(String, Shape)>,
    pub probes: Vec<Probe>,
}

/// One probe's describe, as the resolver needs it.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub columns: Vec<DescribedColumn>,
    /// Whether the probe's own analysis saw an outer join, which is the one thing that can null
    /// a whole row under `to_jsonb(alias)`.
    pub outer_join: bool,
    /// Per column, the JSON a WITH query or a derived table builds in it, for one the probe reads
    /// straight from such an item: it stands for the column's type. Empty where there is none.
    pub built: Vec<Option<Built>>,
}

/// The JSON a column read from a WITH query or a derived table holds, as that item's own
/// statement builds it: its type, whether the item's result never has it NULL, and the paths
/// inside it whose type the engine could not derive.
#[derive(Debug, Clone)]
pub struct Built {
    pub ty: Type,
    pub non_null: bool,
    pub unresolved: Vec<String>,
}

/// A resolved shape: the column's type, whether the column can be SQL NULL, and the paths
/// inside it whose type the engine could not derive.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub ty: Type,
    pub non_null: bool,
    pub unresolved: Vec<String>,
}

/// Read one rewritten statement's JSON constructors. Nothing runs against the database here;
/// the probes come back as text for the build to describe.
pub fn plan(sql: &str, dialect: Dialect, trust: &Trust, columns: &Columns) -> Plan {
    crate::stack::run(|| plan_here(sql, dialect, trust, columns))
}

fn plan_here(sql: &str, dialect: Dialect, trust: &Trust, columns: &Columns) -> Plan {
    plan_with(sql, dialect, trust, columns, false)
}

/// Whether [`plan`] reads the columns of the statement's relations: for a scalar subquery, whose
/// aggregate may be the outer query's ([`crate::levels`]), and for `to_json`, `to_jsonb` and
/// `row_to_json`, whose bare argument is a whole row only where it is no column.
pub fn needs_columns(sql: &str) -> bool {
    crate::stack::run(|| needs_columns_here(sql))
}

fn needs_columns_here(sql: &str) -> bool {
    sql.to_ascii_lowercase().contains("to_json") || crate::levels::needed(sql)
}

/// Every probe [`plan`] may ask for, whatever the trust check and the relations' columns turn out
/// to say: what a host can describe ahead of them, in its first round.
pub fn probes_ahead(sql: &str) -> Vec<Probe> {
    crate::stack::run(|| probes_ahead_here(sql))
}

fn probes_ahead_here(sql: &str) -> Vec<Probe> {
    plan_with(sql, Dialect::Postgres, &Trust::default(), &Columns::default(), true).probes
}

fn plan_with(sql: &str, dialect: Dialect, trust: &Trust, columns: &Columns, ahead: bool) -> Plan {
    let Ok(parsed) = crate::stack::parse(sql) else { return Plan::default() };
    let version = parsed.protobuf.version;
    let Some(top) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) else {
        return Plan::default();
    };
    let Some((targets, from, with)) = top_context(top) else { return Plan::default() };
    // Every row the select list reads passed the WHERE, so the top context's probe filters by it.
    let filter = match top {
        NodeEnum::SelectStmt(s) => s.where_clause.as_deref().cloned(),
        _ => None,
    };
    let withs = vec![Withs::default().with(with.as_ref(), columns)];
    // A plain GROUP BY: every group has a row. A grouping set can make one of none (`GROUP BY ()`
    // over no rows is one row), so it is no plain one.
    let grouped = matches!(top, NodeEnum::SelectStmt(s) if !s.group_clause.is_empty() && !s.group_clause.iter().any(|g| matches!(g.node, Some(NodeEnum::GroupingSet(_)))));
    let mut builder = Builder { stack: vec![(0, from, filter)], dialect, trust, columns, withs, rows: Rows::of(top), ahead, next_ctx: 0, drafts: Vec::new(), overflowed: false, grouped };
    let mut columns: Vec<(String, Shape)> = Vec::new();
    for target in &targets {
        let Some(NodeEnum::ResTarget(rt)) = target.node.as_ref() else { continue };
        let Some(val) = rt.val.as_deref() else { continue };
        if !is_constructor(val) {
            continue;
        }
        let Some(name) = target_name(rt) else { continue };
        let before = builder.drafts.len();
        let shape = builder.walk(val);
        if builder.overflowed {
            builder.drafts.truncate(before);
            break;
        }
        if shape != Shape::Unknown {
            columns.push((name, shape));
        }
    }
    // Every probe carries the statement's own CTEs, so a constructor reading one still
    // resolves; a CTE holding a placeholder is left out, because a probe binds nothing.
    let with = with.filter(|w| !contains_param(w));
    let probes = builder
        .drafts
        .iter()
        .map(|d| Probe {
            sql: select_text(version, &d.items, &d.from, d.filter.as_ref(), with.as_ref()).unwrap_or_default(),
            wider: d.wider.as_ref().and_then(|f| select_text(version, &d.items, f, d.filter.as_ref(), with.as_ref())),
        })
        .collect();
    Plan { columns, probes }
}

/// Turn one shape plus its probes' describes into the column's type.
pub fn resolve(shape: &Shape, probes: &[Option<Outcome>], registry: &TypeRegistry, param_oids: &[u32]) -> Resolved {
    let mut unresolved = Vec::new();
    let ctx = ResolveCtx { probes, registry, param_oids };
    let (ty, non_null) = ctx.node(shape, "", &mut unresolved);
    Resolved { ty, non_null, unresolved }
}

/// Every oid a resolved shape reads, so the host can be asked for the ones the registry lacks.
pub fn oids(shape: &Shape, probes: &[Option<Outcome>], param_oids: &[u32], out: &mut Vec<u32>) {
    match shape {
        Shape::Object { fields, .. } => fields.iter().for_each(|(_, s)| oids(s, probes, param_oids, out)),
        Shape::Array { items, .. } => oids(items, probes, param_oids, out),
        Shape::Nullable(inner) => oids(inner, probes, param_oids, out),
        Shape::Column { probe, index } => {
            if let Some(col) = probes.get(*probe).and_then(|o| o.as_ref()).and_then(|o| o.columns.get(*index)) {
                out.push(col.type_oid);
            }
        }
        Shape::Row { probe } => {
            if let Some(o) = probes.get(*probe).and_then(|o| o.as_ref()) {
                out.extend(o.columns.iter().map(|c| c.type_oid));
            }
        }
        Shape::Param(n) => out.extend(param_oids.get((*n as usize).saturating_sub(1)).copied()),
        Shape::Unknown => {}
    }
}

/// Whether a type is one the shape machinery speaks for: a JSON column and nothing else.
pub fn is_json(registry: &TypeRegistry, type_oid: u32) -> bool {
    matches!(registry.resolve_domain(type_oid), oid::JSON | oid::JSONB)
}

// ---------------------------------------------------------------------------------------------
// Recognition
// ---------------------------------------------------------------------------------------------

struct Builder<'a> {
    /// The `FROM` clauses in scope, outermost first; the innermost is the last. Each carries an
    /// id of its own, because two sibling subqueries sit at the same depth and select from
    /// different relations — keying a probe on the depth alone would pool their columns.
    stack: Vec<(usize, Vec<Node>, Option<Node>)>,
    dialect: Dialect,
    trust: &'a Trust,
    /// The columns of the statement's relations, and the WITH queries in scope at each entry of
    /// `stack`: what says whose an aggregate in a scalar subquery is, and whether a bare name is
    /// a column ([`crate::levels`]).
    columns: &'a Columns,
    withs: Vec<Withs>,
    /// The old and the new row a RETURNING list can name where the builder stands
    /// ([`crate::returning`]); none in a SELECT.
    rows: Option<Rows>,
    /// Planning ahead of the columns: every probe they could allow is drafted.
    ahead: bool,
    next_ctx: usize,
    drafts: Vec<Draft>,
    /// Set once the probe budget is spent, which abandons the statement's inference.
    overflowed: bool,
    /// Whether the top statement is a SELECT with a plain `GROUP BY`, where no group is empty.
    grouped: bool,
}

#[derive(Clone)]
struct Draft {
    ctx: usize,
    /// A `to_jsonb(alias)` probe, whose every column is one field.
    row: bool,
    from: Vec<Node>,
    /// The condition every row the context reads passed: the statement's WHERE, and an
    /// aggregate's FILTER for what it aggregates.
    filter: Option<Node>,
    /// The same `FROM` with every enclosing one appended, for a correlated constructor.
    wider: Option<Vec<Node>>,
    items: Vec<Node>,
}

impl Builder<'_> {
    fn walk(&mut self, expr: &Node) -> Shape {
        if self.overflowed {
            return Shape::Unknown;
        }
        match expr.node.as_ref() {
            Some(NodeEnum::FuncCall(f)) => {
                // A constructor is the built-in one: a name qualified with a schema is somebody
                // else's function, and so may be an unqualified one a user also defined
                // (`jsonb_build_object(text, text)` matches a call better than the built-in).
                let Some(name) = crate::infer::builtin_name(&f.funcname).filter(|n| self.trust.builtin(n)) else { return self.leaf(expr) };
                if OBJECT_FNS.contains(&name.as_str()) {
                    if f.args.len() % 2 != 0 {
                        return Shape::Unknown;
                    }
                    let mut fields = Vec::with_capacity(f.args.len() / 2);
                    for pair in f.args.chunks(2) {
                        let Some(key) = string_const(&pair[0]) else { return Shape::Unknown };
                        fields.push((key, self.walk(&pair[1])));
                    }
                    Shape::Object { fields, non_null: true }
                } else if AGG_FNS.contains(&name.as_str()) {
                    if f.args.len() != 1 {
                        return Shape::Unknown;
                    }
                    // Under a plain GROUP BY every group has a row, so an aggregate of the top
                    // statement's own is an array there, NULL inputs and all, unless a FILTER or a
                    // window frame leaves it none. One inside a subquery may be the outer query's
                    // ([`crate::levels`]), which says nothing of its rows.
                    let grouped = self.dialect == Dialect::Postgres && self.grouped && self.stack.len() == 1 && f.over.is_none() && f.agg_filter.is_none();
                    let non_null = TOTAL_AGG_FNS.contains(&name.as_str()) || grouped;
                    // Only rows its FILTER passes reach an aggregate: its argument is read in a
                    // context of its own whose probe filters by it too, so `FILTER (WHERE p.id IS
                    // NOT NULL)` over a LEFT JOIN reads `p` as a row that exists.
                    let Some(filter) = f.agg_filter.as_deref() else {
                        return Shape::Array { items: Box::new(self.walk(&f.args[0])), non_null };
                    };
                    let (_, from, outer) = self.stack[self.stack.len() - 1].clone();
                    let both = match outer {
                        Some(outer) => and(outer, filter.clone()),
                        None => filter.clone(),
                    };
                    self.next_ctx += 1;
                    self.stack.push((self.next_ctx, from, Some(both)));
                    self.withs.push(self.withs.last().cloned().unwrap_or_default());
                    let items = self.walk(&f.args[0]);
                    self.withs.pop();
                    self.stack.pop();
                    Shape::Array { items: Box::new(items), non_null }
                } else if ROW_FNS.contains(&name.as_str()) {
                    // A bare name is a column before it is a FROM item, of the innermost query
                    // that has one of that name: it is the item's whole row only when no FROM in
                    // scope has such a column, for certain.
                    match f.args.first().and_then(bare_alias) {
                        // The old or the new row of a RETURNING is the target relation's row, or
                        // NULL where the statement has no such row.
                        Some(alias) if f.args.len() == 1 && self.no_column(&alias) => match self.rows.as_ref().and_then(|r| r.whole(&alias)).map(|(target, missing)| (target.to_string(), missing)) {
                            Some((target, true)) => force_nullable(self.row(&target)),
                            Some((target, false)) => self.row(&target),
                            None => self.row(&alias),
                        },
                        _ => Shape::Unknown,
                    }
                } else {
                    self.leaf(expr)
                }
            }
            // `coalesce(x, …)` is `x` where `x` stands alone or is never NULL, and an array under
            // `'[]'` is that array or an empty one, so never NULL. Under any other default it may be
            // that default, of any shape, so it is one value the build reads as it is.
            Some(NodeEnum::CoalesceExpr(c)) => {
                let Some(first) = c.args.first() else { return Shape::Unknown };
                // What the walk drafts for `x` goes unread where the whole is one value.
                let drafted = self.drafts.clone();
                match (self.walk(first), &c.args[1..]) {
                    (shape, []) => shape,
                    (shape @ (Shape::Object { non_null: true, .. } | Shape::Array { non_null: true, .. }), _) => shape,
                    (Shape::Array { items, .. }, [default]) if is_empty_array(default) => Shape::Array { items, non_null: true },
                    // An `x` the build cannot explain leaves the whole unexplained too, and it
                    // costs no probe: one may aggregate in a way no probe without a GROUP BY takes.
                    (Shape::Unknown, _) => {
                        self.drafts = drafted;
                        Shape::Unknown
                    }
                    _ => {
                        self.drafts = drafted;
                        // An aggregate there goes in a probe of its own, under a context id of its
                        // own over the same `FROM`: a probe has no GROUP BY, so beside a bare
                        // column of the context's own probe it would fail that probe whole.
                        if !crate::infer::aggregates(expr, self.dialect, self.trust, None) {
                            return self.leaf(expr);
                        }
                        let top = self.stack.len() - 1;
                        self.next_ctx += 1;
                        let ctx = std::mem::replace(&mut self.stack[top].0, self.next_ctx);
                        let shape = self.leaf(expr);
                        self.stack[top].0 = ctx;
                        shape
                    }
                }
            }
            Some(NodeEnum::TypeCast(t)) => match (t.arg.as_deref(), casts_to_json(t)) {
                (Some(arg), true) => self.walk(arg),
                _ => self.leaf(expr),
            },
            Some(NodeEnum::SubLink(s)) => {
                if s.sub_link_type != protobuf::SubLinkType::ExprSublink as i32 {
                    return Shape::Unknown;
                }
                let Some(select) = s.subselect.as_deref().and_then(as_select) else { return Shape::Unknown };
                let Some(inner) = single_target(select).and_then(|rt| rt.val.clone()) else { return Shape::Unknown };
                let withs = self.withs.last().cloned().unwrap_or_default().with(select.with_clause.as_ref(), self.columns);
                let level = Level::of(&select.from_clause, &withs, self.columns);
                self.next_ctx += 1;
                self.stack.push((self.next_ctx, select.from_clause.clone(), None));
                self.withs.push(withs);
                let rows = self.rows.take();
                self.rows = rows.as_ref().map(|r| r.under(&select.from_clause));
                let shape = self.walk(&inner);
                self.rows = rows;
                self.withs.pop();
                self.stack.pop();
                // A scalar subquery is one row only when it is an ungrouped aggregate of its own
                // (one over columns of a query around it is that query's, `levels`); anything
                // else can match nothing and hand back NULL, whatever its value is.
                if crate::infer::one_row(select, &inner, self.dialect, self.trust) || (crate::infer::keeps_one_row(select, &inner, self.dialect, self.trust) && (aggregates(&inner, &level, self.trust) || crate::infer::aggregates(&inner, self.dialect, self.trust, Some(&level)))) {
                    shape
                } else {
                    force_nullable(shape)
                }
            }
            _ => self.leaf(expr),
        }
    }

    /// One value expression, as a column of the probe for the constructor's own `FROM` context.
    fn leaf(&mut self, expr: &Node) -> Shape {
        if let Some(n) = param_number(expr) {
            return Shape::Param(n);
        }
        // A probe binds nothing, so an expression reading a placeholder cannot go in one.
        if contains_param(expr) {
            return Shape::Unknown;
        }
        // A RETURNING's old and new rows are no relation a probe can select from: the expression
        // reads the target relation's columns in their place, and is NULL where it reads a row
        // the statement may not have.
        let (expr, missing) = match &self.rows {
            Some(rows) => rows.rewrite(expr, &|name| self.no_column(name)),
            None => (expr.clone(), false),
        };
        let ctx = self.stack[self.stack.len() - 1].0;
        let probe = match self.drafts.iter().position(|d| d.ctx == ctx && !d.row) {
            Some(i) => i,
            None => match self.draft(false) {
                Some(i) => i,
                None => return Shape::Unknown,
            },
        };
        let index = self.drafts[probe].items.len();
        self.drafts[probe].items.push(expr);
        let shape = Shape::Column { probe, index };
        if missing {
            Shape::Nullable(Box::new(shape))
        } else {
            shape
        }
    }

    /// Whether no FROM in scope shows a column of this name, for certain.
    fn no_column(&self, name: &str) -> bool {
        self.ahead || self.stack.iter().zip(&self.withs).all(|((_, from, _), withs)| Level::of(from, withs, self.columns).lacks(name))
    }

    /// `to_jsonb(u)`: a probe of its own, selecting `u.*`, so the row's own columns are the shape.
    fn row(&mut self, alias: &str) -> Shape {
        let Some(probe) = self.draft(true) else { return Shape::Unknown };
        let star = Node {
            node: Some(NodeEnum::ColumnRef(ColumnRef {
                fields: vec![string_node(alias), Node { node: Some(NodeEnum::AStar(AStar {})) }],
                location: -1,
            })),
        };
        self.drafts[probe].items.push(star);
        Shape::Row { probe }
    }

    fn draft(&mut self, row: bool) -> Option<usize> {
        if self.drafts.len() >= MAX_PROBES {
            self.overflowed = true;
            return None;
        }
        let depth = self.stack.len() - 1;
        let (ctx, from, filter) = self.stack[depth].clone();
        let mut wider = from.clone();
        for d in (0..depth).rev() {
            wider.extend(self.stack[d].1.iter().cloned());
        }
        // A subquery with no `FROM` of its own only ever resolves against the enclosing one.
        let (from, wider) = if from.is_empty() && !wider.is_empty() { (wider, None) } else { (from, Some(wider)) };
        let wider = wider.filter(|w| w.len() != from.len());
        self.drafts.push(Draft { ctx, row, from, filter, wider, items: Vec::new() });
        Some(self.drafts.len() - 1)
    }
}

/// The target list, `FROM` context, and `WITH` clause the top statement's result columns live in.
pub fn top_context(top: &NodeEnum) -> Option<(Vec<Node>, Vec<Node>, Option<protobuf::WithClause>)> {
    match top {
        NodeEnum::SelectStmt(s) => {
            // A set operation hides which arm produced a column; the build degrades there anyway.
            if s.op != protobuf::SetOperation::SetopNone as i32 && s.op != 0 {
                return None;
            }
            Some((s.target_list.clone(), s.from_clause.clone(), s.with_clause.clone()))
        }
        NodeEnum::InsertStmt(s) => {
            Some((returning(&s.returning_clause), relation_from(s.relation.as_ref(), &[]), s.with_clause.clone()))
        }
        NodeEnum::UpdateStmt(s) => {
            Some((returning(&s.returning_clause), relation_from(s.relation.as_ref(), &s.from_clause), s.with_clause.clone()))
        }
        NodeEnum::DeleteStmt(s) => {
            Some((returning(&s.returning_clause), relation_from(s.relation.as_ref(), &s.using_clause), s.with_clause.clone()))
        }
        // A MERGE's RETURNING reads the target and the source, and the source is no row at all
        // where the MERGE acts on a target row the source does not match.
        NodeEnum::MergeStmt(s) => {
            let source: Vec<Node> = s.source_relation.as_deref().cloned().into_iter().collect();
            Some((returning(&s.returning_clause), Rows::of(top)?.from(s.relation.as_ref()?, &source), s.with_clause.clone()))
        }
        _ => None,
    }
}

/// A RETURNING clause's expressions: PostgreSQL 18 holds them beside the clause's `WITH (OLD AS …,
/// NEW AS …)` options, where earlier parsers had the bare list.
pub(crate) fn returning(clause: &Option<protobuf::ReturningClause>) -> Vec<Node> {
    clause.as_ref().map(|c| c.exprs.clone()).unwrap_or_default()
}

/// A DML statement's `RETURNING` reads the target relation plus whatever it joined in.
fn relation_from(relation: Option<&protobuf::RangeVar>, extra: &[Node]) -> Vec<Node> {
    let mut out = Vec::new();
    if let Some(r) = relation {
        out.push(Node { node: Some(NodeEnum::RangeVar(r.clone())) });
    }
    out.extend(extra.iter().cloned());
    out
}

/// Whether this expression is a JSON constructor the build speaks for, rather than an ordinary
/// expression that happens to be typed json.
fn is_constructor(expr: &Node) -> bool {
    match expr.node.as_ref() {
        Some(NodeEnum::FuncCall(f)) => {
            let name = func_name(f).unwrap_or_default();
            OBJECT_FNS.contains(&name.as_str()) || AGG_FNS.contains(&name.as_str()) || ROW_FNS.contains(&name.as_str())
        }
        Some(NodeEnum::CoalesceExpr(c)) => c.args.first().is_some_and(is_constructor),
        Some(NodeEnum::TypeCast(t)) => casts_to_json(t) && t.arg.as_deref().is_some_and(is_constructor),
        Some(NodeEnum::SubLink(s)) => {
            s.sub_link_type == protobuf::SubLinkType::ExprSublink as i32
                && s.subselect
                    .as_deref()
                    .and_then(as_select)
                    .and_then(single_target)
                    .and_then(|rt| rt.val.clone())
                    .is_some_and(|v| is_constructor(&v))
        }
        _ => false,
    }
}

/// Whether the top of this expression is an aggregate, which is what makes an ungrouped
/// subquery return exactly one row. A call with `OVER` is a window function, one row per input.
fn aggregates(expr: &Node, level: &Level, trust: &Trust) -> bool {
    match expr.node.as_ref() {
        Some(NodeEnum::FuncCall(f)) => f.over.is_none() && crate::infer::builtin_name(&f.funcname).is_some_and(|n| AGG_FNS.contains(&n.as_str()) && trust.builtin(&n)) && level.owns(f),
        Some(NodeEnum::CoalesceExpr(c)) => c.args.first().is_some_and(|first| aggregates(first, level, trust)),
        Some(NodeEnum::TypeCast(t)) => t.arg.as_deref().is_some_and(|arg| aggregates(arg, level, trust)),
        _ => false,
    }
}

fn force_nullable(shape: Shape) -> Shape {
    match shape {
        Shape::Object { fields, .. } => Shape::Object { fields, non_null: false },
        Shape::Array { items, .. } => Shape::Array { items, non_null: false },
        Shape::Unknown | Shape::Nullable(_) => shape,
        // A column, a whole row or a parameter says nothing of the row it is read from.
        other => Shape::Nullable(Box::new(other)),
    }
}

// ---------------------------------------------------------------------------------------------
// Synthesis
// ---------------------------------------------------------------------------------------------

/// `SELECT <items> FROM <from>`, deparsed by the same library that parsed the original. The
/// statement is only ever prepared, never executed, so it needs no `WHERE` to stay cheap.
pub(crate) fn select_text(version: i32, items: &[Node], from: &[Node], filter: Option<&Node>, with: Option<&protobuf::WithClause>) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let target_list = items
        .iter()
        .enumerate()
        .map(|(i, val)| Node {
            node: Some(NodeEnum::ResTarget(Box::new(ResTarget {
                // A star cannot take an alias; everything else gets a positional one, since the
                // shape maps back by position.
                name: if is_star(val) { String::new() } else { format!("a{i}") },
                indirection: Vec::new(),
                val: Some(Box::new(val.clone())),
                location: -1,
            }))),
        })
        .collect();
    // Every enumerated field has to carry a value the deparser knows: a protobuf zero means
    // "unset", and libpg_query asserts rather than guessing.
    let select = SelectStmt {
        target_list,
        from_clause: from.to_vec(),
        where_clause: filter.map(|f| Box::new(f.clone())),
        with_clause: with.cloned(),
        op: protobuf::SetOperation::SetopNone as i32,
        limit_option: protobuf::LimitOption::Default as i32,
        ..Default::default()
    };
    let result = ParseResult {
        version,
        stmts: vec![RawStmt {
            stmt: Some(Box::new(Node { node: Some(NodeEnum::SelectStmt(Box::new(select))) })),
            stmt_location: 0,
            stmt_len: 0,
        }],
    };
    let text = crate::stack::deparse(&result).ok()?;
    // A probe binds nothing, so a placeholder in its condition is NULL there: a test that is
    // strict in a column stays strict (`u.id = NULL`), which is all the probe reads of it, and a
    // condition the server cannot type proves nothing, the probe failing to describe.
    Some(without_placeholders(&text))
}

/// `sql` with every `$n` outside strings, quoted identifiers and comments written as `NULL`.
pub(crate) fn without_placeholders(sql: &str) -> String {
    let Ok(lexed) = crate::lex::lex(sql, crate::dialect::Dialect::Postgres) else { return sql.to_string() };
    let mut out = String::with_capacity(sql.len());
    let mut at = 0;
    for p in &lexed.placeholders {
        out.push_str(&sql[at..p.start]);
        out.push_str("NULL");
        at = p.end;
    }
    out.push_str(&sql[at..]);
    out
}

/// `a AND b`, as the parser writes it.
fn and(a: Node, b: Node) -> Node {
    Node { node: Some(NodeEnum::BoolExpr(Box::new(protobuf::BoolExpr { xpr: None, boolop: protobuf::BoolExprType::AndExpr as i32, args: vec![a, b], location: -1 }))) }
}

// ---------------------------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------------------------

struct ResolveCtx<'a> {
    probes: &'a [Option<Outcome>],
    registry: &'a TypeRegistry,
    param_oids: &'a [u32],
}

impl ResolveCtx<'_> {
    /// One shape as (type, whether the value can never be null).
    fn node(&self, shape: &Shape, path: &str, unresolved: &mut Vec<String>) -> (Type, bool) {
        match shape {
            Shape::Object { fields, non_null } => {
                let fields = fields
                    .iter()
                    .map(|(key, value)| {
                        let (ty, non_null) = self.node(value, &join(path, key), unresolved);
                        Field { name: key.clone(), ty, nullable: !non_null }
                    })
                    .collect();
                (Type::Object { fields }, *non_null)
            }
            Shape::Array { items, non_null } => {
                let (of, items_non_null) = self.node(items, &join(path, "[]"), unresolved);
                (Type::List { of: Box::new(of), nullable_items: !items_non_null }, *non_null)
            }
            Shape::Column { probe, index } => match self.probes.get(*probe).and_then(|o| o.as_ref()).and_then(|o| Some((o.columns.get(*index)?, o.built.get(*index).and_then(Option::as_ref)))) {
                // JSON a WITH query or a derived table builds, NULL where the item's row has it so
                // or a join here nulls the item. A path the item could not type goes under this
                // one, a path inside its array (`[].tags`) as this module writes its own.
                Some((col, Some(built))) => {
                    unresolved.extend(built.unresolved.iter().map(|p| match p.as_str() {
                        WHOLE => display_path(path),
                        p if p.starts_with("[]") => display_path(&format!("{path}{p}")),
                        p => display_path(&join(path, p)),
                    }));
                    (built.ty.clone(), built.non_null && !col.nullable)
                }
                Some((col, None)) if col.type_oid != oid::UNKNOWN => {
                    let ty = if col.whole_row { self.registry.render_row(col.type_oid) } else { self.registry.render(col.type_oid) };
                    (crate::postgres::with_document(crate::postgres::with_values(ty, col.values.as_deref()), col.document.as_ref()), !col.nullable)
                }
                _ => {
                    unresolved.push(display_path(path));
                    (Type::Unknown, false)
                }
            },
            Shape::Row { probe } => match self.probes.get(*probe).and_then(|o| o.as_ref()) {
                Some(outcome) => {
                    let fields = outcome
                        .columns
                        .iter()
                        .map(|col| {
                            let ty = if col.type_oid == oid::UNKNOWN {
                                unresolved.push(display_path(&join(path, &col.name)));
                                Type::Unknown
                            } else {
                                let ty = if col.whole_row { self.registry.render_row(col.type_oid) } else { self.registry.render(col.type_oid) };
                                crate::postgres::with_document(crate::postgres::with_values(ty, col.values.as_deref()), col.document.as_ref())
                            };
                            Field { name: col.name.clone(), ty, nullable: col.nullable }
                        })
                        .collect();
                    (Type::Object { fields }, !outcome.outer_join)
                }
                None => {
                    unresolved.push(display_path(path));
                    (Type::Unknown, false)
                }
            },
            // A placeholder carries the type the statement's own describe gave it; whether it
            // arrived null is the caller's business, so the field stays nullable.
            Shape::Nullable(inner) => (self.node(inner, path, unresolved).0, false),
            Shape::Param(n) => match self.param_oids.get((*n as usize).saturating_sub(1)) {
                Some(o) => (self.registry.render(*o), false),
                None => {
                    unresolved.push(display_path(path));
                    (Type::Unknown, false)
                }
            },
            Shape::Unknown => {
                unresolved.push(display_path(path));
                (Type::Unknown, false)
            }
        }
    }
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else if key == "[]" {
        format!("{path}[]")
    } else {
        format!("{path}.{key}")
    }
}

/// The path [`display_path`] gives the value itself.
const WHOLE: &str = "the whole value";

fn display_path(path: &str) -> String {
    if path.is_empty() {
        WHOLE.to_string()
    } else {
        path.to_string()
    }
}

// ---------------------------------------------------------------------------------------------
// Parse-tree helpers
// ---------------------------------------------------------------------------------------------

pub fn as_select(node: &Node) -> Option<&SelectStmt> {
    match node.node.as_ref()? {
        NodeEnum::SelectStmt(s) => Some(s),
        _ => None,
    }
}

pub(crate) fn single_target(select: &SelectStmt) -> Option<&ResTarget> {
    if select.target_list.len() != 1 {
        return None;
    }
    match select.target_list[0].node.as_ref()? {
        NodeEnum::ResTarget(rt) => Some(rt),
        _ => None,
    }
}

/// The name the result column carries: its alias, else the name Postgres derives.
fn target_name(rt: &ResTarget) -> Option<String> {
    if !rt.name.is_empty() {
        return Some(rt.name.clone());
    }
    default_name(rt.val.as_deref()?)
}

fn default_name(node: &Node) -> Option<String> {
    match node.node.as_ref()? {
        NodeEnum::FuncCall(f) => func_name(f),
        NodeEnum::CoalesceExpr(_) => Some("coalesce".to_string()),
        NodeEnum::TypeCast(t) => t.type_name.as_ref().and_then(|tn| last_string(&tn.names)),
        NodeEnum::SubLink(s) => target_name(single_target(as_select(s.subselect.as_deref()?)?)?),
        _ => None,
    }
}

fn func_name(f: &protobuf::FuncCall) -> Option<String> {
    last_string(&f.funcname)
}

pub(crate) fn last_string(nodes: &[Node]) -> Option<String> {
    match nodes.last()?.node.as_ref()? {
        NodeEnum::String(s) => Some(s.sval.clone()),
        _ => None,
    }
}

fn string_node(value: &str) -> Node {
    Node { node: Some(NodeEnum::String(protobuf::String { sval: value.to_string() })) }
}

pub(crate) fn is_star(node: &Node) -> bool {
    match node.node.as_ref() {
        Some(NodeEnum::ColumnRef(c)) => {
            c.fields.last().is_some_and(|f| matches!(f.node.as_ref(), Some(NodeEnum::AStar(_))))
        }
        _ => false,
    }
}

/// A literal string, which is the only thing a key may be.
fn string_const(node: &Node) -> Option<String> {
    match node.node.as_ref()? {
        NodeEnum::AConst(c) if !c.isnull => match c.val.as_ref()? {
            protobuf::a_const::Val::Sval(s) => Some(s.sval.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// `'[]'`, as JSON text or cast to `json` or `jsonb`: the default that keeps an array an array.
fn is_empty_array(node: &Node) -> bool {
    match node.node.as_ref() {
        Some(NodeEnum::TypeCast(t)) => casts_to_json(t) && t.arg.as_deref().is_some_and(is_empty_array),
        _ => string_const(node).as_deref() == Some("[]"),
    }
}

fn casts_to_json(t: &protobuf::TypeCast) -> bool {
    t.type_name
        .as_ref()
        .and_then(|tn| last_string(&tn.names))
        .is_some_and(|n| n == "json" || n == "jsonb")
}

/// A bare `u`, the alias form `to_jsonb` takes for a whole row.
fn bare_alias(node: &Node) -> Option<String> {
    match node.node.as_ref()? {
        NodeEnum::ColumnRef(c) if c.fields.len() == 1 => last_string(&c.fields),
        _ => None,
    }
}

/// The placeholder this expression is, through any casts.
fn param_number(node: &Node) -> Option<i32> {
    match node.node.as_ref()? {
        NodeEnum::ParamRef(p) => Some(p.number),
        NodeEnum::TypeCast(t) => param_number(t.arg.as_deref()?),
        _ => None,
    }
}

/// Whether a placeholder appears anywhere below this node.
fn contains_param<T: serde::Serialize>(node: &T) -> bool {
    fn hunt(v: &Value) -> bool {
        match v {
            Value::Object(map) => map.contains_key("ParamRef") || map.values().any(hunt),
            Value::Array(items) => items.iter().any(hunt),
            _ => false,
        }
    }
    serde_json::to_value(node).is_ok_and(|v| hunt(&v))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn shapes(sql: &str) -> Plan {
        let columns: Columns = [("users", vec!["id", "email", "name", "doc"]), ("posts", vec!["id", "author_id", "title", "body"]), ("comments", vec!["id", "post_id", "body", "created_at"])].into_iter().collect();
        plan(sql, Dialect::Postgres, &Trust::default(), &columns)
    }

    #[test]
    fn a_nested_aggregate_becomes_one_probe_over_the_subquery() {
        let p = shapes(
            "SELECT p.id, p.title,
               (SELECT coalesce(jsonb_agg(jsonb_build_object('id', c.id, 'body', c.body) ORDER BY c.created_at), '[]'::jsonb)
                FROM comments c WHERE c.post_id = p.id) AS comments
             FROM posts p",
        );
        assert_eq!(p.probes.len(), 1);
        assert_eq!(p.probes[0].sql, "SELECT c.id AS a0, c.body AS a1 FROM comments c");
        assert_eq!(p.probes[0].wider.as_deref(), Some("SELECT c.id AS a0, c.body AS a1 FROM comments c, posts p"));
        assert_eq!(p.columns.len(), 1);
        let (name, shape) = &p.columns[0];
        assert_eq!(name, "comments");
        let Shape::Array { items, non_null } = shape else { panic!("{shape:?}") };
        assert!(non_null, "coalesce(…, '[]') makes the array non-null");
        assert_eq!(
            **items,
            Shape::Object {
                fields: vec![
                    ("id".into(), Shape::Column { probe: 0, index: 0 }),
                    ("body".into(), Shape::Column { probe: 0, index: 1 }),
                ],
                non_null: true,
            }
        );
    }

    #[test]
    fn an_aggregate_under_a_plain_group_by_is_an_array() {
        let non_null = |sql: &str| match &shapes(sql).columns[0].1 {
            Shape::Array { non_null, .. } => *non_null,
            shape => panic!("{sql}: {shape:?}"),
        };
        // Every group has a row, and a NULL input is a JSON null in the array.
        assert!(non_null("SELECT jsonb_agg(p.title ORDER BY p.id) AS titles FROM posts p GROUP BY p.author_id"));
        assert!(non_null("SELECT json_agg(p) AS rows FROM users u LEFT JOIN posts p ON p.author_id = u.id GROUP BY u.id"));
        // Nested in an object of the grouped statement's own, the aggregate is still its own.
        let p = shapes("SELECT jsonb_build_object('titles', jsonb_agg(p.title)) AS o FROM posts p GROUP BY p.author_id");
        let Shape::Object { fields, .. } = &p.columns[0].1 else { panic!() };
        assert!(matches!(fields[0].1, Shape::Array { non_null: true, .. }));
        // A FILTER can leave a group no rows, a window frame can be empty, a grouping set's row
        // may aggregate none, and without a GROUP BY there may be no rows at all.
        assert!(!non_null("SELECT jsonb_agg(p.title) FILTER (WHERE p.id > 1) AS titles FROM posts p GROUP BY p.author_id"));
        assert!(!non_null("SELECT jsonb_agg(p.title) OVER (ROWS BETWEEN 1 FOLLOWING AND 1 FOLLOWING) AS next FROM posts p GROUP BY p.title"));
        assert!(!non_null("SELECT jsonb_agg(p.title) AS titles FROM posts p GROUP BY ROLLUP (p.author_id)"));
        assert!(!non_null("SELECT jsonb_agg(p.title) AS titles FROM posts p GROUP BY GROUPING SETS ((p.author_id), ())"));
        assert!(!non_null("SELECT jsonb_agg(p.title) AS titles FROM posts p"));
        // Inside a subquery the aggregate may be the outer query's, and a grouped subquery may
        // find no group.
        assert!(!non_null("SELECT (SELECT jsonb_agg(c.body) FROM comments c WHERE c.post_id = p.id GROUP BY c.post_id) AS bodies FROM posts p GROUP BY p.id"));
    }

    #[test]
    fn a_bare_aggregate_stays_nullable() {
        let p = shapes("SELECT (SELECT jsonb_agg(jsonb_build_object('id', c.id)) FROM comments c) AS cs FROM posts p");
        let Shape::Array { non_null, .. } = &p.columns[0].1 else { panic!() };
        assert!(!non_null);
    }

    #[test]
    fn a_subquery_that_does_not_aggregate_is_nullable() {
        // One row is not guaranteed, so the object can come back NULL even though the
        // constructor itself never does.
        let p = shapes("SELECT (SELECT jsonb_build_object('n', u.name) FROM users u WHERE u.id = p.author_id) AS author FROM posts p");
        let Shape::Object { non_null, .. } = &p.columns[0].1 else { panic!("{:?}", p.columns) };
        assert!(!non_null);
        // A GROUP BY takes the single-row guarantee away from an aggregate too.
        let g = shapes("SELECT (SELECT jsonb_agg(c.id) FROM comments c GROUP BY c.post_id) AS cs FROM posts p");
        assert!(matches!(&g.columns[0].1, Shape::Array { non_null: false, .. }));
        // So do a LIMIT or an OFFSET, which can skip the one row, and a window, which is no
        // aggregate at all.
        for sql in [
            "SELECT (SELECT coalesce(jsonb_agg(c.id), '[]'::jsonb) FROM comments c LIMIT 0) AS cs FROM posts p",
            "SELECT (SELECT coalesce(jsonb_agg(c.id), '[]'::jsonb) FROM comments c OFFSET 1) AS cs FROM posts p",
            "SELECT (SELECT coalesce(jsonb_agg(c.id) OVER (), '[]'::jsonb) FROM comments c) AS cs FROM posts p",
        ] {
            assert!(matches!(&shapes(sql).columns[0].1, Shape::Array { non_null: false, .. }), "{sql}");
        }
    }

    #[test]
    fn a_top_level_object_is_non_null_and_probes_the_outer_from() {
        let p = shapes("SELECT jsonb_build_object('id', p.id, 'name', u.name) AS post FROM posts p JOIN users u ON u.id = p.author_id");
        assert_eq!(p.probes.len(), 1);
        assert_eq!(p.probes[0].sql, "SELECT p.id AS a0, u.name AS a1 FROM posts p JOIN users u ON u.id = p.author_id");
        assert_eq!(p.probes[0].wider, None);
        assert!(matches!(&p.columns[0].1, Shape::Object { non_null: true, .. }));
    }

    #[test]
    fn objects_nest_inside_objects_on_one_probe() {
        let p = shapes("SELECT jsonb_build_object('id', p.id, 'author', jsonb_build_object('name', u.name)) AS post FROM posts p, users u");
        assert_eq!(p.probes.len(), 1);
        assert_eq!(p.probes[0].sql, "SELECT p.id AS a0, u.name AS a1 FROM posts p, users u");
        let Shape::Object { fields, .. } = &p.columns[0].1 else { panic!() };
        assert_eq!(fields[1].0, "author");
        assert!(matches!(&fields[1].1, Shape::Object { non_null: true, .. }));
    }

    #[test]
    fn an_aggregate_inside_an_object_gets_its_own_probe() {
        let p = shapes(
            "SELECT jsonb_build_object(
               'id', p.id,
               'comments', (SELECT coalesce(jsonb_agg(jsonb_build_object('body', c.body)), '[]'::jsonb) FROM comments c WHERE c.post_id = p.id)
             ) AS post FROM posts p",
        );
        assert_eq!(p.probes.len(), 2);
        assert_eq!(p.probes[0].sql, "SELECT p.id AS a0 FROM posts p");
        assert_eq!(p.probes[1].sql, "SELECT c.body AS a0 FROM comments c");
        let Shape::Object { fields, .. } = &p.columns[0].1 else { panic!() };
        assert!(matches!(&fields[1].1, Shape::Array { non_null: true, .. }));
    }

    #[test]
    fn a_coalesce_read_as_one_value_probes_an_aggregate_apart() {
        // Under `'{}'` the whole aggregate is one value, which a probe beside `title` would fail:
        // it has no GROUP BY.
        let p = shapes("SELECT jsonb_build_object('ids', coalesce(jsonb_agg(p.id), '{}'), 'titles', jsonb_agg(p.title)) AS o FROM posts p");
        assert_eq!(p.probes.iter().map(|q| q.sql.as_str()).collect::<Vec<_>>(), ["SELECT COALESCE(jsonb_agg(p.id), '{}') AS a0 FROM posts p", "SELECT p.title AS a0 FROM posts p"]);
        let Shape::Object { fields, .. } = &p.columns[0].1 else { panic!() };
        assert_eq!(fields[0].1, Shape::Column { probe: 0, index: 0 });
        // One that does not aggregate stays on its context's probe, so it costs none of the budget.
        let q = shapes("SELECT jsonb_build_object('id', u.id, 'name', coalesce(u.name, u.email), 'doc', coalesce(u.doc, '{}')) AS o FROM users u");
        assert_eq!(q.probes.iter().map(|q| q.sql.as_str()).collect::<Vec<_>>(), ["SELECT u.id AS a0, COALESCE(u.name, u.email) AS a1, COALESCE(u.doc, '{}') AS a2 FROM users u"]);
        // One over an `x` the build cannot explain is unexplained too, and probes nothing.
        let r = shapes("SELECT jsonb_build_object('name', u.name, 'titles', coalesce(to_jsonb(array_agg(p.title)), '[]')) AS o FROM users u JOIN posts p ON p.author_id = u.id GROUP BY u.name");
        assert_eq!(r.probes.iter().map(|q| q.sql.as_str()).collect::<Vec<_>>(), ["SELECT u.name AS a0 FROM users u JOIN posts p ON p.author_id = u.id"]);
        let Shape::Object { fields, .. } = &r.columns[0].1 else { panic!() };
        assert_eq!(fields[1].1, Shape::Unknown);
    }

    #[test]
    fn a_whole_row_probes_the_star() {
        let p = shapes("SELECT to_jsonb(u) AS author FROM users u");
        assert_eq!(p.probes[0].sql, "SELECT u.* FROM users u");
        assert_eq!(p.columns[0].1, Shape::Row { probe: 0 });
        let r = shapes("SELECT row_to_json(u) AS author FROM users u");
        assert_eq!(r.columns[0].1, Shape::Row { probe: 0 });
        // A bare name is a column before it is a FROM item: `doc` is a column of `users`, here
        // and from inside a subquery over another table, so neither call is a whole row. Nor is
        // one where a FROM in scope does not say its columns.
        for sql in [
            "SELECT to_jsonb(doc) AS author FROM users doc",
            "SELECT (SELECT to_jsonb(doc) FROM posts doc LIMIT 1) AS post FROM users",
            "SELECT to_jsonb(u) AS author FROM users u, generate_series(1, 2) g",
            "SELECT to_jsonb(u) AS author FROM unlisted u",
            "SELECT to_jsonb(ctid) AS author FROM users ctid",
        ] {
            assert!(shapes(sql).columns.is_empty(), "{sql}");
        }
        assert_eq!(probes_ahead("SELECT to_jsonb(u) AS author FROM unlisted u")[0].sql, "SELECT u.* FROM unlisted u");
        assert!(needs_columns("SELECT to_jsonb(u) AS author FROM users u") && needs_columns("SELECT row_to_json(u) FROM users u"));
        assert!(needs_columns("SELECT (SELECT json_agg(p.id) FROM posts p) AS ids") && !needs_columns("SELECT json_agg(p.id) AS ids FROM posts p"));
    }

    #[test]
    fn a_scalar_subquery_that_may_find_no_row_is_nullable_whatever_its_value() {
        let registry = TypeRegistry::with_builtins();
        let found = vec![column("a0", oid::TEXT, "text", false)];
        let resolved = |sql: &str| resolve(&shapes(sql).columns[0].1, &[Some(Outcome { columns: found.clone(), outer_join: false, built: Vec::new() })], &registry, &[]);
        // Whether the column is nullable, and whether the first field of the object it is.
        let whole = |sql: &str| !resolved(sql).non_null;
        let field = |sql: &str| match resolved(sql).ty {
            Type::Object { fields } => fields[0].nullable,
            other => panic!("{other:?}"),
        };
        // A column, a whole row and an aggregate of the query around the subquery.
        assert!(field("SELECT json_build_object('first', (SELECT p.title FROM posts p WHERE p.author_id = u.id LIMIT 1)) AS o FROM users u"));
        assert!(field("SELECT json_build_object('first', (SELECT p.title FROM posts p WHERE p.id = u.id)) AS o FROM users u"));
        assert!(whole("SELECT (SELECT to_jsonb(p) FROM posts p WHERE p.author_id = u.id LIMIT 1) AS o FROM users u"));
        assert!(field("SELECT json_build_object('n', (SELECT count(name) FROM posts WHERE false)) AS o FROM users u"));
        assert!(field("SELECT json_build_object('n', (SELECT count(u.id) FROM posts WHERE false)) AS o FROM users u"));
        assert!(whole("SELECT (SELECT coalesce(json_agg(name), '[]') FROM posts WHERE false) AS o FROM users u"));
        // An aggregate of its own is one row.
        assert!(!field("SELECT json_build_object('n', (SELECT count(*) FROM posts p WHERE p.author_id = u.id)) AS o FROM users u"));
        assert!(!field("SELECT json_build_object('n', (SELECT count(title) FROM posts WHERE author_id = u.id)) AS o FROM users u"));
        assert!(!whole("SELECT (SELECT coalesce(json_agg(title), '[]') FROM posts WHERE false) AS o FROM users u"));
        assert!(!whole("SELECT to_jsonb(p) AS o FROM posts p"));
        // A user's function of an aggregate's name is not one.
        let trust = Trust { shadowed: ["count".to_string()].into_iter().collect(), ..Trust::default() };
        let p = plan("SELECT json_build_object('n', (SELECT count(*) FROM posts)) AS o FROM users u", Dialect::Postgres, &trust, &Columns::default());
        let Shape::Object { fields, .. } = &p.columns[0].1 else { panic!() };
        assert!(matches!(fields[0].1, Shape::Nullable(_)));
    }

    #[test]
    fn a_constructor_is_the_built_in_one_only() {
        // A name qualified with a schema is that schema's function, and a name a user has also
        // defined may resolve to theirs: both are read as any other expression, a column of the
        // probe, whose type and nullability are the describe's.
        let leaf = |p: &Plan| matches!(p.columns[0].1, Shape::Column { .. });
        assert!(leaf(&shapes("SELECT app.json_build_object('a', u.name) AS o FROM users u")));
        assert!(leaf(&shapes("SELECT app.to_jsonb(u) AS o FROM users u")));
        assert!(matches!(shapes("SELECT pg_catalog.json_build_object('a', u.name) AS o FROM users u").columns[0].1, Shape::Object { .. }));
        let trust = Trust { shadowed: ["jsonb_build_object".to_string(), "to_jsonb".to_string(), "json_agg".to_string()].into_iter().collect(), ..Trust::default() };
        let columns: Columns = [("users", vec!["id", "name"])].into_iter().collect();
        for sql in [
            "SELECT jsonb_build_object('a', u.name) AS o FROM users u",
            "SELECT to_jsonb(u) AS o FROM users u",
            "SELECT json_agg(u.name) AS o FROM users u",
        ] {
            assert!(leaf(&plan(sql, Dialect::Postgres, &trust, &columns)), "{sql}");
        }
        // The subquery is one row by an aggregate only where the name is the built-in's.
        let p = plan("SELECT (SELECT coalesce(json_agg(u.name), '[]') FROM users u) AS o", Dialect::Postgres, &trust, &columns);
        assert!(matches!(p.columns[0].1, Shape::Nullable(_)), "{:?}", p.columns);
        assert!(crate::infer::postgres_function_names().contains(&"jsonb_build_object"));
    }

    #[test]
    fn returning_probes_the_written_relation() {
        let p = shapes("INSERT INTO comments (body) VALUES ('x') RETURNING jsonb_build_object('id', id) AS created");
        assert_eq!(p.probes[0].sql, "SELECT id AS a0 FROM comments");
    }

    #[test]
    fn returning_reads_the_old_and_the_new_row_as_the_written_relations() {
        let fields = |p: &Plan| match &p.columns[0].1 {
            Shape::Object { fields, non_null: true } => fields.iter().map(|(_, shape)| shape.clone()).collect::<Vec<_>>(),
            other => panic!("not an object: {other:?}"),
        };
        let nullable = |shape: Shape| Shape::Nullable(Box::new(shape));
        // PostgreSQL 18: an UPDATE has both rows, and a probe reads each as the written relation's.
        let u = shapes("UPDATE users SET name = 'x' RETURNING WITH (OLD AS o, NEW AS n) jsonb_build_object('was', o.name, 'is', upper(n.name), 'row', to_jsonb(o)) AS change");
        assert_eq!(u.probes.iter().map(|p| p.sql.as_str()).collect::<Vec<_>>(), ["SELECT users.name AS a0, upper(users.name) AS a1 FROM users", "SELECT users.* FROM users"]);
        assert_eq!(fields(&u), [Shape::Column { probe: 0, index: 0 }, Shape::Column { probe: 0, index: 1 }, Shape::Row { probe: 1 }]);
        // An INSERT has no old row: what reads it is NULL, whatever the relation's columns say.
        let i = shapes("INSERT INTO users (name) VALUES ('x') RETURNING jsonb_build_object('was', old.name, 'is', new.name, 'row', to_jsonb(old), 'up', upper(old.name)) AS change");
        assert_eq!(i.probes[0].sql, "SELECT users.name AS a0, users.name AS a1, upper(users.name) AS a2 FROM users");
        assert_eq!(fields(&i), [nullable(Shape::Column { probe: 0, index: 0 }), Shape::Column { probe: 0, index: 1 }, nullable(Shape::Row { probe: 1 }), nullable(Shape::Column { probe: 0, index: 2 })]);
        // A bare name is a column before it is a row: `doc` is a column of `users`.
        let c = shapes("UPDATE users SET name = 'x' RETURNING WITH (OLD AS doc) jsonb_build_object('row', to_jsonb(doc), 'was', doc.name) AS change");
        assert_eq!(fields(&c), [Shape::Unknown, Shape::Column { probe: 0, index: 0 }]);
        assert_eq!(c.probes[0].sql, "SELECT users.name AS a0 FROM users");
        // A relation of the name is the one read, the target's own alias and a subquery's FROM item.
        let named = shapes("DELETE FROM users AS new WHERE id = 1 RETURNING jsonb_build_object('name', new.name, 'row', to_jsonb(new)) AS gone");
        assert_eq!(named.probes.iter().map(|p| p.sql.as_str()).collect::<Vec<_>>(), ["SELECT new.name AS a0 FROM users new", "SELECT new.* FROM users new"]);
        assert_eq!(fields(&named), [Shape::Column { probe: 0, index: 0 }, Shape::Row { probe: 1 }]);
        let inner = shapes("DELETE FROM users WHERE id = 1 RETURNING jsonb_build_object('posts', (SELECT jsonb_agg(jsonb_build_object('title', new.title, 'author', old.name, 'now', (SELECT new.name))) FROM posts new WHERE new.author_id = old.id)) AS gone");
        assert_eq!(inner.probes[0].sql, "SELECT new.title AS a0, users.name AS a1 FROM posts new");
        assert_eq!(inner.probes[0].wider.as_deref(), Some("SELECT new.title AS a0, users.name AS a1 FROM posts new, users"));
        // A MERGE reads its source through an outer join where it acts without a source row.
        let m = shapes("MERGE INTO users u USING posts s ON u.id = s.author_id WHEN MATCHED THEN UPDATE SET name = s.title WHEN NOT MATCHED BY SOURCE THEN DELETE RETURNING jsonb_build_object('title', s.title, 'was', old.name, 'is', new.name) AS change");
        assert_eq!(m.probes[0].sql, "SELECT s.title AS a0, u.name AS a1, u.name AS a2 FROM users u LEFT JOIN posts s ON true");
        assert_eq!(fields(&m), [nullable(Shape::Column { probe: 0, index: 0 }), Shape::Column { probe: 0, index: 1 }, nullable(Shape::Column { probe: 0, index: 2 })]);
    }

    #[test]
    fn jsonb_the_build_cannot_explain_is_left_alone() {
        // A stored column, a concatenation, and an unrecognised function are not shapes.
        assert!(shapes("SELECT a.raw FROM addresses a").columns.is_empty());
        assert!(shapes("SELECT a.raw || '{}'::jsonb AS merged FROM addresses a").columns.is_empty());
        assert!(shapes("SELECT jsonb_path_query_array(a.raw, '$') AS x FROM addresses a").columns.is_empty());
        // A non-literal key is not a shape either.
        assert!(shapes("SELECT jsonb_build_object(t.title, t.id) AS x FROM todos t").columns.is_empty());
        assert!(shapes("SELECT jsonb_build_object('a') AS x FROM todos t").columns.is_empty());
    }

    #[test]
    fn a_placeholder_leaf_takes_the_statements_own_parameter_type() {
        let p = shapes("SELECT jsonb_build_object('q', $1::text, 'id', t.id) AS x FROM todos t");
        let Shape::Object { fields, .. } = &p.columns[0].1 else { panic!() };
        assert_eq!(fields[0].1, Shape::Param(1));
        assert_eq!(fields[1].1, Shape::Column { probe: 0, index: 0 });
        assert_eq!(p.probes[0].sql, "SELECT t.id AS a0 FROM todos t");
        // An expression merely containing one cannot go in a probe, which binds nothing.
        let q = shapes("SELECT jsonb_build_object('q', 'x' || $1::text) AS x FROM todos t");
        let Shape::Object { fields, .. } = &q.columns[0].1 else { panic!() };
        assert_eq!(fields[0].1, Shape::Unknown);
        assert!(q.probes.is_empty());
    }

    #[test]
    fn an_aggregates_filter_filters_the_probe_of_what_it_aggregates() {
        let p = shapes("SELECT u.id, json_agg(json_build_object('id', p.id, 'title', p.title)) FILTER (WHERE p.id IS NOT NULL) AS posts FROM users u LEFT JOIN posts p ON p.author_id = u.id WHERE u.org = $1 GROUP BY u.id");
        assert_eq!(p.probes[0].sql, "SELECT p.id AS a0, p.title AS a1 FROM users u LEFT JOIN posts p ON p.author_id = u.id WHERE u.org = NULL AND p.id IS NOT NULL");
        // Without a FILTER the aggregate reads the statement's own context.
        let q = shapes("SELECT json_agg(p.title) AS t FROM posts p WHERE p.score > 0");
        assert_eq!(q.probes[0].sql, "SELECT p.title AS a0 FROM posts p WHERE p.score > 0");
    }

    #[test]
    fn a_cte_travels_with_the_probe_unless_it_binds() {
        let p = shapes("WITH recent AS (SELECT id, body FROM comments) SELECT jsonb_build_object('b', r.body) AS x FROM recent r");
        assert_eq!(p.probes[0].sql, "WITH recent AS (SELECT id, body FROM comments) SELECT r.body AS a0 FROM recent r");
        let q = shapes("WITH input AS (SELECT $1::uuid AS id) SELECT jsonb_build_object('b', c.body) AS x FROM comments c, input i WHERE c.id = i.id");
        assert_eq!(q.probes[0].sql, "SELECT c.body AS a0 FROM comments c, input i WHERE c.id = i.id");
    }

    #[test]
    fn a_set_operation_is_left_to_the_existing_degradation() {
        assert!(shapes("SELECT jsonb_build_object('a', t.id) AS x FROM todos t UNION ALL SELECT jsonb_build_object('a', t.id) FROM todos t")
            .columns
            .is_empty());
    }

    #[test]
    fn an_unaliased_constructor_takes_the_name_postgres_gives_it() {
        assert_eq!(shapes("SELECT jsonb_build_object('a', t.id) FROM todos t").columns[0].0, "jsonb_build_object");
        assert_eq!(
            shapes("SELECT coalesce(jsonb_agg(t.id), '[]'::jsonb) FROM todos t").columns.first().map(|c| c.0.as_str()),
            Some("coalesce")
        );
    }

    #[test]
    fn sibling_subqueries_get_a_probe_each() {
        // Both sit one level down, but they select from different relations: pooling their
        // value expressions onto one probe would type each against the other's `FROM`.
        let p = shapes(
            "SELECT jsonb_build_object(
               'comments', (SELECT jsonb_agg(c.body) FROM comments c WHERE c.post_id = p.id),
               'author', (SELECT jsonb_agg(u.name) FROM users u WHERE u.id = p.author_id)
             ) AS x FROM posts p",
        );
        assert_eq!(p.probes.len(), 2);
        assert_eq!(p.probes[0].sql, "SELECT c.body AS a0 FROM comments c");
        assert_eq!(p.probes[1].sql, "SELECT u.name AS a0 FROM users u");
        let Shape::Object { fields, .. } = &p.columns[0].1 else { panic!() };
        assert_eq!(fields[0].1, Shape::Array { items: Box::new(Shape::Column { probe: 0, index: 0 }), non_null: false });
        assert_eq!(fields[1].1, Shape::Array { items: Box::new(Shape::Column { probe: 1, index: 0 }), non_null: false });
    }

    #[test]
    fn the_probe_budget_is_bounded() {
        // A whole-row constructor costs a probe each, so this is the cheapest way to overrun.
        let keys: Vec<String> = (0..MAX_PROBES + 2).map(|i| format!("'k{i}', to_jsonb(u{i})")).collect();
        let rels: Vec<String> = (0..MAX_PROBES + 2).map(|i| format!("users u{i}")).collect();
        let sql = format!("SELECT jsonb_build_object({}) AS x FROM {}", keys.join(", "), rels.join(", "));
        let p = shapes(&sql);
        assert!(p.probes.is_empty(), "{}", p.probes.len());
        assert!(p.columns.is_empty(), "a statement that overflows the budget claims nothing");
    }

    // Resolution. The shape's types come back as reylo's `Type`; the JSON Schema spelling of each
    // (a nullable field, an `int8` inside a JSON document) is the renderer's, not the resolver's.

    const NUMERIC: u32 = 1700;
    const BYTEA: u32 = 17;

    fn column(name: &str, type_oid: u32, type_name: &str, nullable: bool) -> DescribedColumn {
        DescribedColumn { name: name.into(), type_oid, type_name: type_name.into(), nullable, nullable_items: None, values: None, document: None, whole_row: false }
    }

    fn scalar(name: &str) -> Type {
        Type::Scalar { name: name.into() }
    }

    fn field(name: &str, ty: Type, nullable: bool) -> Field {
        Field { name: name.into(), ty, nullable }
    }

    #[test]
    fn resolution_maps_described_columns_back_onto_the_shape() {
        let registry = TypeRegistry::with_builtins();
        let p = shapes(
            "SELECT (SELECT coalesce(jsonb_agg(jsonb_build_object('id', c.id, 'body', c.body, 'score', c.score)), '[]'::jsonb)
                     FROM comments c) AS comments FROM posts p",
        );
        let outcome = Outcome {
            columns: vec![
                column("a0", oid::UUID, "uuid", false),
                column("a1", oid::TEXT, "text", false),
                column("a2", oid::INT4, "int4", true),
            ],
            outer_join: false,
            built: Vec::new(),
        };
        let r = resolve(&p.columns[0].1, &[Some(outcome)], &registry, &[]);
        assert!(r.non_null);
        assert!(r.unresolved.is_empty());
        assert_eq!(
            r.ty,
            Type::List {
                of: Box::new(Type::Object {
                    fields: vec![field("id", scalar("uuid"), false), field("body", scalar("text"), false), field("score", scalar("int4"), true)],
                }),
                nullable_items: false,
            }
        );
    }

    #[test]
    fn a_probe_that_could_not_be_described_names_the_paths_it_lost() {
        let registry = TypeRegistry::with_builtins();
        let p = shapes("SELECT jsonb_build_object('id', p.id, 'title', p.title) AS post FROM posts p");
        let r = resolve(&p.columns[0].1, &[None], &registry, &[]);
        assert_eq!(r.unresolved, vec!["id", "title"]);
        assert_eq!(r.ty, Type::Object { fields: vec![field("id", Type::Unknown, true), field("title", Type::Unknown, true)] });
        // The keys are still the SQL's own, and the object is still never null.
        assert!(r.non_null);
    }

    #[test]
    fn a_column_sqlite_cannot_type_is_reported_not_guessed() {
        let registry = TypeRegistry::with_builtins();
        let p = shapes("SELECT json_object('id', c.id, 'n', length(c.body)) AS x FROM comments c");
        let outcome = Outcome {
            columns: vec![column("a0", oid::UUID, "uuid", false), column("a1", oid::UNKNOWN, "unknown", true)],
            outer_join: false,
            built: Vec::new(),
        };
        let r = resolve(&p.columns[0].1, &[Some(outcome)], &registry, &[]);
        assert_eq!(r.unresolved, vec!["n"]);
        let Type::Object { fields } = &r.ty else { panic!("{:?}", r.ty) };
        assert_eq!(fields[1], field("n", Type::Unknown, true));
        assert_eq!(fields[0], field("id", scalar("uuid"), false));
    }

    /// A value the database wrote into the JSON document is still the column's own type here; how
    /// it is spelled inside JSON (an `int8` or a `numeric` as a JSON number, a `bytea` as `\x…`
    /// hex) is the renderer's rule, which `Type::json_schema` in `schema.rs` states and its tests pin.
    #[test]
    fn a_leaf_inside_a_constructor_takes_the_databases_json_spelling() {
        let registry = TypeRegistry::with_builtins();
        let p = shapes("SELECT jsonb_build_object('n', c.n, 'amount', c.amount, 'blob', c.blob) AS x FROM c");
        let outcome = Outcome {
            columns: vec![column("a0", oid::INT8, "int8", false), column("a1", NUMERIC, "numeric", false), column("a2", BYTEA, "bytea", false)],
            outer_join: false,
            built: Vec::new(),
        };
        let r = resolve(&p.columns[0].1, &[Some(outcome)], &registry, &[]);
        assert!(r.unresolved.is_empty());
        assert_eq!(
            r.ty,
            Type::Object {
                fields: vec![field("n", scalar("int8"), false), field("amount", scalar("numeric"), false), field("blob", scalar("bytea"), false)],
            }
        );
    }

    #[test]
    fn a_row_shape_becomes_the_relations_own_columns() {
        let registry = TypeRegistry::with_builtins();
        let p = shapes("SELECT to_jsonb(u) AS author FROM users u");
        let columns = vec![column("id", oid::UUID, "uuid", false), column("name", oid::TEXT, "text", true)];
        let r = resolve(&p.columns[0].1, &[Some(Outcome { columns: columns.clone(), outer_join: false, built: Vec::new() })], &registry, &[]);
        assert!(r.non_null);
        assert_eq!(r.ty, Type::Object { fields: vec![field("id", scalar("uuid"), false), field("name", scalar("text"), true)] });
        // An outer join in the probe's own analysis is what can null the whole row.
        let r = resolve(&p.columns[0].1, &[Some(Outcome { columns, outer_join: true, built: Vec::new() })], &registry, &[]);
        assert!(!r.non_null);
    }

    #[test]
    fn a_path_an_item_could_not_type_goes_under_the_path_that_reads_it() {
        let registry = TypeRegistry::with_builtins();
        let p = shapes("SELECT jsonb_build_object('posts', ps.posts) AS author FROM ps");
        let item = Type::List { of: Box::new(Type::Object { fields: vec![field("tags", Type::Unknown, true)] }), nullable_items: false };
        let built = Built { ty: item, non_null: true, unresolved: vec!["[].tags".into(), "[]".into(), WHOLE.into()] };
        let outcome = Outcome { columns: vec![column("a0", oid::JSONB, "jsonb", false)], outer_join: false, built: vec![Some(built)] };
        let r = resolve(&p.columns[0].1, &[Some(outcome)], &registry, &[]);
        assert_eq!(r.unresolved, vec!["posts[].tags", "posts[]", "posts"]);
    }
}
