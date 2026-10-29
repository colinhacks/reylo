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
use std::collections::HashSet;

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

/// How many times one statement may read what a LATERAL subquery's target builds: each read may
/// read a LATERAL subquery inside it, so nested ones that each read the next twice would otherwise
/// cost exponentially.
const MAX_LATERAL: usize = 32;

pub use reylo_engine_core::shape::{Probe, Shape};

/// What one statement's JSON columns need: a shape per result column the build could explain,
/// and the statements to describe to fill it in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    /// Result column name → its shape, in target-list order.
    pub columns: Vec<(String, Shape)>,
    pub probes: Vec<Probe>,
    /// Per probe, for a `to_jsonb(alias)` one whose alias an outer join may null, the same read
    /// with the alias's row always there ([`row_present`]): its columns are the row's fields as
    /// they are where the object exists.
    pub present: Vec<Option<Probe>>,
}

impl Plan {
    /// Every statement the plan may describe: its probes, then the reads of a row where it is
    /// there.
    pub fn statements(&self) -> impl Iterator<Item = &Probe> {
        self.probes.iter().chain(self.present.iter().flatten())
    }
}

/// One probe's describe, as the resolver needs it.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub columns: Vec<DescribedColumn>,
    /// Whether the probe's own analysis saw an outer join, which is the one thing that can null
    /// a whole row under `to_jsonb(alias)`.
    pub outer_join: bool,
    /// For a `to_jsonb(alias)` probe, per column, whether it can be NULL in a row where the
    /// alias's row is there, as the plan's read of it with that row present says: an outer join
    /// nulls every column of a row it does not match, but a row that is there holds its own.
    /// `None` where the probe's own columns say it already.
    pub present: Option<Vec<bool>>,
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
    plan_with(sql, Dialect::Postgres, &Trust::default(), &Columns::default(), true).statements().cloned().collect()
}

fn plan_with(sql: &str, dialect: Dialect, trust: &Trust, columns: &Columns, ahead: bool) -> Plan {
    let Ok(parsed) = crate::stack::parse(sql) else { return Plan::default() };
    let version = parsed.protobuf.version;
    let Some(top) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) else {
        return Plan::default();
    };
    let Some((targets, from, with)) = top_context(top) else { return Plan::default() };
    // A write whose WITH clause has a query named like its target writes the table, where a probe
    // carrying the clause would read the query by that name: its shapes are left as they are.
    if target_named_by_with(top, with.as_ref()) {
        return Plan::default();
    }
    // Every row the select list reads passed the WHERE, so the top context's probe filters by it.
    let filter = match top {
        NodeEnum::SelectStmt(s) => s.where_clause.as_deref().cloned(),
        _ => None,
    };
    let withs = vec![Withs::default().with(with.as_ref(), columns)];
    let fields = crate::infer::row_fields(&serde_json::to_value(&parsed.protobuf).unwrap_or_default());
    // A plain GROUP BY: every group has a row. A grouping set can make one of none (`GROUP BY ()`
    // over no rows is one row), so it is no plain one.
    let grouping_sets = matches!(top, NodeEnum::SelectStmt(s) if has_grouping_sets(s));
    let grouped = matches!(top, NodeEnum::SelectStmt(s) if !s.group_clause.is_empty() && !grouping_sets);
    // A statement that groups or aggregates keeps every expression of its select list outside an
    // aggregate's arguments valid under its GROUP BY, which a probe of them carries; the
    // arguments themselves go in a probe without one. A GROUP BY that may name a column of the
    // select list ([`names_output`]) would name one of the probe's own, so it is none a probe can
    // carry.
    let group = match top {
        NodeEnum::SelectStmt(s) if grouped && !s.group_clause.iter().any(|g| names_output(g, &targets)) => Some(s.group_clause.clone()),
        NodeEnum::SelectStmt(s) if s.group_clause.is_empty() && aggregating(&targets, dialect, trust, &fields, &Level::of(&from, &withs[0], columns), columns) => Some(Vec::new()),
        _ => None,
    };
    let windows = match top {
        NodeEnum::SelectStmt(s) => s.window_clause.clone(),
        _ => Vec::new(),
    };
    let mut builder = Builder { stack: vec![(0, from, filter)], dialect, trust, columns, withs, rows: Rows::of(top), ahead, next_ctx: 0, drafts: Vec::new(), overflowed: false, grouped, group, in_aggregate: 0, grouping_sets, under_aggregate: false, laterals: 0, windows, fields };
    let mut columns: Vec<(String, Shape)> = Vec::new();
    for target in &targets {
        let Some(NodeEnum::ResTarget(rt)) = target.node.as_ref() else { continue };
        let Some(val) = rt.val.as_deref() else { continue };
        if !is_constructor(val) && !builder.reads_lateral(val) {
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
    // A probe that reads one of the statement's own CTEs carries them, so a constructor reading
    // one still resolves ([`select_text`]).
    let text = |from: &[Node], inner: Option<usize>, d: &Draft| match inner {
        Some(at) => wrapped_text(version, &d.items, &from[..at], &from[at..], d.filter.as_ref(), with.as_ref()),
        None => {
            let group = builder.group.as_deref().filter(|_| d.grouped).unwrap_or(&[]);
            grouped_select_text(version, &d.items, from, d.filter.as_ref(), group, with.as_ref())
        }
    };
    // A probe that reads the wider FROM wraps the FROM where its constructor stands where that
    // reads a query around it in a join's ON ([`Draft::inner`]).
    let own = |d: &Draft| d.inner.filter(|_| d.widened);
    let probes = builder
        .drafts
        .iter()
        .map(|d| Probe { sql: text(&d.from, own(d), d).unwrap_or_default(), wider: d.wider.as_ref().and_then(|f| text(f, d.inner, d)) })
        .collect();
    let present = builder
        .drafts
        .iter()
        .map(|d| {
            let alias = d.row.as_deref()?;
            let (from, wider) = (row_present(&d.from, alias), d.wider.as_ref().and_then(|w| row_present(w, alias)));
            if from.is_none() && wider.is_none() {
                return None;
            }
            let from = from.unwrap_or_else(|| d.from.clone());
            let wider = wider.or_else(|| d.wider.clone());
            Some(Probe { sql: text(&from, own(d), d).unwrap_or_default(), wider: wider.as_ref().and_then(|f| text(f, d.inner, d)) })
        })
        .collect();
    Plan { columns, probes, present }
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

/// Whether a select list aggregates outside any subquery, so that without a GROUP BY every
/// expression of it outside an aggregate's arguments is one row's.
fn aggregating(targets: &[Node], dialect: Dialect, trust: &Trust, fields: &HashSet<String>, from: &Level, columns: &Columns) -> bool {
    targets.iter().any(|t| match t.node.as_ref() {
        Some(NodeEnum::ResTarget(rt)) => rt.val.as_deref().is_some_and(|v| crate::infer::aggregates(v, dialect, trust, None, Some(fields), &|n| from.column_type(n, columns)) || constructs_aggregate(v)),
        _ => false,
    })
}

/// Whether a GROUP BY item may name a column of the select list rather than an expression over
/// the FROM: a position (`GROUP BY 1`), or a bare name that is a target's own, its alias or the
/// name Postgres derives for one that is no bare column (`lower(p.title) AS t … GROUP BY t`).
/// Postgres takes a bare name for a column of the FROM first, which the planning cannot see, so
/// a name that is both counts too: it only leaves the probe without the GROUP BY.
fn names_output(item: &Node, targets: &[Node]) -> bool {
    match item.node.as_ref() {
        Some(NodeEnum::AConst(_)) => true,
        Some(NodeEnum::ColumnRef(c)) if c.fields.len() == 1 => {
            let Some(NodeEnum::String(name)) = c.fields[0].node.as_ref() else { return false };
            // A bare column with no alias goes by its own name, which reads that column whether
            // Postgres takes the name for the FROM's or the select list's.
            let own = |rt: &ResTarget| rt.name.is_empty() && matches!(rt.val.as_deref().and_then(|v| v.node.as_ref()), Some(NodeEnum::ColumnRef(_)));
            targets.iter().any(|t| matches!(t.node.as_ref(), Some(NodeEnum::ResTarget(rt)) if !own(rt) && target_name(rt).as_deref() == Some(name.sval.as_str())))
        }
        _ => false,
    }
}

/// Whether a JSON constructor aggregates at the top of the statement: `jsonb_agg(…)`, or an
/// aggregate as one of `jsonb_build_object`'s values.
fn constructs_aggregate(expr: &Node) -> bool {
    match expr.node.as_ref() {
        Some(NodeEnum::FuncCall(f)) if f.over.is_none() => {
            let name = func_name(f).unwrap_or_default();
            AGG_FNS.contains(&name.as_str()) || f.agg_star || f.agg_distinct || f.agg_within_group || !f.agg_order.is_empty() || f.agg_filter.is_some() || f.args.iter().any(constructs_aggregate)
        }
        Some(NodeEnum::CoalesceExpr(c)) => c.args.iter().any(constructs_aggregate),
        Some(NodeEnum::TypeCast(t)) => t.arg.as_deref().is_some_and(constructs_aggregate),
        _ => false,
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
    /// For a top statement that groups or aggregates, its GROUP BY (empty for one that only
    /// aggregates): what the probe of its own expressions outside an aggregate carries.
    group: Option<Vec<Node>>,
    /// How many aggregates of the top statement the walk is inside the arguments of.
    in_aggregate: usize,
    /// Whether the top statement groups by grouping sets, which null a grouping column in the
    /// super-aggregate rows: what it reads is NULL there, but for an aggregate's argument, which
    /// is read from the rows before they are grouped.
    grouping_sets: bool,
    /// Whether the builder stands in the argument of a plain aggregate of the top statement.
    under_aggregate: bool,
    /// How many LATERAL subqueries' targets have been read, which [`MAX_LATERAL`] bounds.
    laterals: usize,
    /// The top statement's `WINDOW` clause, which `OVER w` names a window of.
    windows: Vec<Node>,
    /// The columns the statement's own rows may have ([`crate::infer::row_fields`]).
    fields: HashSet<String>,
}

/// A target of a LATERAL subquery that builds JSON ([`Builder::lateral_target`]).
struct Lateral {
    /// The subquery's statement.
    body: SelectStmt,
    target: Node,
    /// Whether an outer join of the FROM that reads the subquery can null it.
    nulled: bool,
    /// The name the subquery goes by.
    alias: String,
}

#[derive(Clone)]
struct Draft {
    ctx: usize,
    /// For a `to_jsonb(alias)` probe, whose every column is one field, the alias.
    row: Option<String>,
    from: Vec<Node>,
    /// The condition every row the context reads passed: the statement's WHERE, and an
    /// aggregate's FILTER for what it aggregates.
    filter: Option<Node>,
    /// Every enclosing `FROM`, outermost first, then this one, for a correlated constructor.
    wider: Option<Vec<Node>>,
    /// Where in `wider` this `FROM` begins, where a join of it reads a query around it in its ON,
    /// which no item beside the join can stand in for: the probe then reads it in a LATERAL
    /// subquery of its own, where what is around it is a query around it again.
    inner: Option<usize>,
    /// Whether the probe carries the top statement's GROUP BY ([`Builder::group`]).
    grouped: bool,
    /// Whether the probe reads its FROM with every enclosing one already, for a bare name the
    /// FROM alone would read as something else ([`Builder::renamed_in_probe`]).
    widened: bool,
    items: Vec<Node>,
}

impl Builder<'_> {
    fn walk(&mut self, expr: &Node) -> Shape {
        if self.overflowed {
            return Shape::Unknown;
        }
        let shape = self.build(expr);
        // A grouping set nulls a grouping expression in its super-aggregate rows, and an object
        // or an array the statement builds may be one (`GROUP BY ROLLUP (o)`), as may one inside
        // it: nothing built there is non-null.
        if self.nulled_by_grouping() {
            force_nullable(shape)
        } else {
            shape
        }
    }

    fn build(&mut self, expr: &Node) -> Shape {
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
                    // Under a plain GROUP BY every group has a row, and a window frame that always
                    // holds the current row has that row, so an aggregate of the top statement's
                    // own is an array there, NULL inputs and all, unless a FILTER leaves it none.
                    // One inside a subquery may be the outer query's ([`crate::levels`]), which
                    // says nothing of its rows.
                    let has_a_row = match f.over.as_deref() {
                        Some(over) => crate::infer::frame_holds_current_row(over, &self.windows),
                        None => self.grouped,
                    };
                    let grouped = self.dialect == Dialect::Postgres && has_a_row && self.stack.len() == 1 && f.agg_filter.is_none();
                    let non_null = TOTAL_AGG_FNS.contains(&name.as_str()) || grouped;
                    // A plain aggregate of the top statement reads its argument from the rows
                    // before a grouping set nulls them; a window call reads the grouped rows.
                    let outer = self.under_aggregate;
                    self.under_aggregate |= f.over.is_none() && self.stack.len() == 1;
                    let shape = self.aggregate(f, non_null);
                    self.under_aggregate = outer;
                    shape
                } else if ROW_FNS.contains(&name.as_str()) {
                    // A bare name is a column before it is a FROM item, of the innermost query
                    // that has one of that name: it is the item's whole row only when no FROM in
                    // scope has such a column, for certain.
                    // `u.*` says the whole row outright.
                    match f.args.first() {
                        Some(arg) if f.args.len() == 1 => match star_alias(arg).or_else(|| bare_alias(arg).filter(|alias| self.no_column(alias))) {
                            Some(alias) => self.whole(&alias),
                            None => Shape::Unknown,
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
                    (Shape::Array { items, .. }, [default]) if crate::infer::is_empty_array(default) => Shape::Array { items, non_null: true },
                    // A placeholder is the whole's type under a default that cannot widen it, and it
                    // resolves nullable. Under any other one the whole is no probe's to read, since a
                    // probe binds nothing: `coalesce($1::int, 1.5)` is a numeric.
                    (shape @ Shape::Param(_), defaults) if defaults.iter().all(keeps_param_type) => shape,
                    // An `x` the build cannot explain leaves the whole unexplained too, and it
                    // costs no probe: one may aggregate in a way no probe without a GROUP BY takes.
                    (Shape::Unknown, _) => {
                        self.drafts = drafted;
                        Shape::Unknown
                    }
                    _ => {
                        self.drafts = drafted;
                        // An aggregate there goes in a probe of its own, under a context id of its
                        // own over the same `FROM`: a probe without the statement's GROUP BY
                        // would fail beside a bare column of the context's own probe. Where the
                        // statement's own expressions have a probe that carries it, that one does.
                        let from = Level::of(&self.stack[self.stack.len() - 1].1, self.withs.last().unwrap_or(&Withs::default()), self.columns);
                        if !crate::infer::aggregates(expr, self.dialect, self.trust, None, Some(&self.fields), &|n| from.column_type(n, self.columns)) || self.outside_aggregates() {
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
            // `u.*` as a value, a function's argument (`jsonb_agg(u.*)`, `jsonb_build_object('u',
            // u.*)`), is the whole row, as `u` is: never the row's columns one by one, which is
            // what it would be in a probe's select list.
            Some(NodeEnum::ColumnRef(_)) if star_alias(expr).is_some() => self.whole(&star_alias(expr).expect("a star alias")),
            // A column of a LATERAL subquery that the subquery builds: the shape of what it builds
            // there. A WITH query's or another subquery's is a statement of its own, which the
            // probe reading the column follows ([`Outcome::built`]).
            Some(NodeEnum::ColumnRef(_)) => match self.lateral_target(expr) {
                Some(lateral) => self.lateral(lateral),
                None => self.leaf(expr),
            },
            // A CASE whose one branch builds JSON and whose others are NULL: that branch's shape,
            // nullable. Two branches that build JSON may build two shapes.
            Some(NodeEnum::CaseExpr(c)) => match case_branch(c) {
                Some(built) => force_nullable(self.walk(built)),
                None => self.leaf(expr),
            },
            Some(NodeEnum::TypeCast(t)) => match (t.arg.as_deref(), casts_to_json(t)) {
                (Some(arg), true) => self.walk(arg),
                _ => self.leaf(expr),
            },
            Some(NodeEnum::SubLink(s)) => {
                if s.sub_link_type != protobuf::SubLinkType::ExprSublink as i32 {
                    return Shape::Unknown;
                }
                let Some(select) = s.subselect.as_deref().and_then(as_select) else { return Shape::Unknown };
                // A probe carries the statement's WITH, so a subquery's own WITH, whose queries
                // hide the statement's and its tables of their names, is no context a probe can
                // stand in; nor is a subquery's grouping set, which nulls what it groups by.
                if select.with_clause.is_some() || has_grouping_sets(select) {
                    return Shape::Unknown;
                }
                let Some(inner) = single_target(select).and_then(|rt| rt.val.clone()) else { return Shape::Unknown };
                let withs = self.withs.last().cloned().unwrap_or_default().with(select.with_clause.as_ref(), self.columns);
                // The FROMs around the subquery are the builder's stack, every one of them.
                let outside = self.stack.iter().zip(&self.withs).map(|((_, from, _), withs)| Level::of(from, withs, self.columns)).collect();
                let level = Level::of(&select.from_clause, &withs, self.columns).within(outside);
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
                if crate::infer::one_row(select, &inner, self.dialect, self.trust) || (crate::infer::keeps_one_row(select, &inner, self.dialect, self.trust) && (aggregates(&inner, &level, self.trust) || crate::infer::aggregates(&inner, self.dialect, self.trust, Some(&level), Some(&self.fields), &|n| level.column_type(n, self.columns)))) {
                    shape
                } else {
                    force_nullable(shape)
                }
            }
            _ => self.leaf(expr),
        }
    }

    /// `json_agg(x)` and its kin, `non_null` where no group is empty.
    fn aggregate(&mut self, f: &protobuf::FuncCall, non_null: bool) -> Shape {
        // Only rows its FILTER passes reach an aggregate: its argument is read in a
        // context of its own whose probe filters by it too, so `FILTER (WHERE p.id IS
        // NOT NULL)` over a LEFT JOIN reads `p` as a row that exists.
        let Some(filter) = f.agg_filter.as_deref() else {
            self.in_aggregate += 1;
            let items = self.walk(&f.args[0]);
            self.in_aggregate -= 1;
            return Shape::Array { items: Box::new(items), non_null };
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
    }

    /// Whether a value read where the builder stands may be NULL for a grouping set.
    fn nulled_by_grouping(&self) -> bool {
        self.grouping_sets && !self.under_aggregate
    }

    /// The JSON a column of a LATERAL subquery builds, where `expr` names one: `x.col` of such a
    /// subquery in the FROM where the builder stands whose target `col` is a JSON constructor, or
    /// a bare `col` of the one item a FROM holds. Nothing where the subquery is a set operation,
    /// selects a `*`, groups by grouping sets, or has a WITH of its own, which its probes could not
    /// carry, nor where two of its columns go by the name, which Postgres refuses.
    fn lateral_target(&self, expr: &Node) -> Option<Lateral> {
        let Some(NodeEnum::ColumnRef(c)) = expr.node.as_ref() else { return None };
        let names: Vec<&str> = c.fields.iter().map(|f| match f.node.as_ref() {
            Some(NodeEnum::String(s)) => Some(s.sval.as_str()),
            _ => None,
        }).collect::<Option<Vec<_>>>()?;
        let (alias, column) = match names.as_slice() {
            [column] => (None, *column),
            [alias, column] => (Some(*alias), *column),
            _ => return None,
        };
        let from = &self.stack.last()?.1;
        let mut items = Vec::new();
        for item in from {
            from_items(item, false, &mut items);
        }
        let (item, nulled) = match alias {
            Some(alias) => {
                let mut named = items.iter().filter(|(item, _)| item_alias(item).as_deref() == Some(alias));
                match (named.next(), named.next()) {
                    (Some(found), None) => *found,
                    _ => return None,
                }
            }
            None => match items.as_slice() {
                [only] if from.len() == 1 => *only,
                _ => return None,
            },
        };
        let Some(NodeEnum::RangeSubselect(r)) = item.node.as_ref() else { return None };
        if !r.lateral {
            return None;
        }
        let body = r.subquery.as_deref().and_then(as_select)?.clone();
        let set_op = body.op != protobuf::SetOperation::SetopNone as i32 && body.op != 0;
        let star = body.target_list.iter().any(|t| matches!(t.node.as_ref(), Some(NodeEnum::ResTarget(rt)) if rt.val.as_deref().is_some_and(is_star)));
        if body.with_clause.is_some() || set_op || star || has_grouping_sets(&body) {
            return None;
        }
        let names = column_names(&body, &[alias_columns(r.alias.as_ref())])?;
        let mut hits = names.iter().enumerate().filter(|(_, n)| n.as_deref() == Some(column));
        let (Some((at, _)), None) = (hits.next(), hits.next()) else { return None };
        let target = match body.target_list.get(at)?.node.as_ref()? {
            NodeEnum::ResTarget(rt) => rt.val.as_deref()?.clone(),
            _ => return None,
        };
        let alias = r.alias.as_ref()?.aliasname.clone();
        is_constructor(&target).then_some(Lateral { body, target, nulled, alias })
    }

    /// Whether `expr` is a column of a LATERAL subquery that builds JSON ([`Builder::lateral_target`]),
    /// alone, under a cast to JSON, or first in a `coalesce` (`coalesce(x.items, '[]')`).
    fn reads_lateral(&self, expr: &Node) -> bool {
        match expr.node.as_ref() {
            Some(NodeEnum::CoalesceExpr(c)) => c.args.first().is_some_and(|first| self.reads_lateral(first)),
            Some(NodeEnum::TypeCast(t)) => casts_to_json(t) && t.arg.as_deref().is_some_and(|arg| self.reads_lateral(arg)),
            _ => self.lateral_target(expr).is_some(),
        }
    }

    /// What a LATERAL subquery's target builds, read in the subquery's own context: its FROM and
    /// WHERE, with every FROM around it for its correlated columns. An outer join that can null
    /// the subquery, or a grouping set of the statement, makes the value nullable.
    fn lateral(&mut self, l: Lateral) -> Shape {
        self.laterals += 1;
        if self.laterals > MAX_LATERAL {
            self.overflowed = true;
            return Shape::Unknown;
        }
        let nulled = l.nulled || self.nulled_by_grouping();
        let withs = self.withs.last().cloned().unwrap_or_default();
        // A correlated column of the subquery is one of an item left of it, which is all it can
        // read: the wider probe reads those alone, since the join that reads the subquery, an
        // outer one above all, would null the subquery's own relations there.
        let parent = self.stack.len() - 1;
        let around = self.stack[parent].1.clone();
        if let Some(left) = left_of(&around, &l.alias) {
            self.stack[parent].1 = left;
        }
        self.next_ctx += 1;
        self.stack.push((self.next_ctx, l.body.from_clause.clone(), l.body.where_clause.as_deref().cloned()));
        self.withs.push(withs);
        let rows = self.rows.take();
        // The subquery's rows are its own, which no grouping set of the statement's nulls.
        let (grouping_sets, under_aggregate) = (self.grouping_sets, self.under_aggregate);
        self.grouping_sets = false;
        let shape = self.walk(&l.target);
        self.grouping_sets = grouping_sets;
        self.under_aggregate = under_aggregate;
        self.rows = rows;
        self.withs.pop();
        self.stack.pop();
        self.stack[parent].1 = around;
        if nulled {
            force_nullable(shape)
        } else {
            shape
        }
    }

    /// A whole row by its alias: the old or the new row of a RETURNING is the target relation's
    /// row, or NULL where the statement has no such row.
    fn whole(&mut self, alias: &str) -> Shape {
        match self.rows.as_ref().and_then(|r| r.whole(alias)).map(|(target, missing)| (target.to_string(), missing)) {
            Some((target, true)) => force_nullable(self.row(&target)),
            Some((target, false)) => self.row(&target),
            None => self.row(alias),
        }
    }

    /// One value expression, as a column of the probe for the constructor's own `FROM` context.
    fn leaf(&mut self, expr: &Node) -> Shape {
        // A star expands to several columns of the probe, so it cannot be one of them.
        if is_star(expr) {
            return Shape::Unknown;
        }
        if let Some(n) = param_number(expr) {
            return Shape::Param(n);
        }
        // A probe binds nothing, so an expression reading a placeholder cannot go in one.
        if contains_param(expr) {
            return Shape::Unknown;
        }
        // A bare name the probe over this FROM alone would read as something else is read over
        // this FROM and every one around it, as one: a column there is a column before an item's
        // row, as in the statement.
        let widened = self.renamed_in_probe(expr);
        // A RETURNING's old and new rows are no relation a probe can select from: the expression
        // reads the target relation's columns in their place, and is NULL where it reads a row
        // the statement may not have.
        let (expr, missing) = match &self.rows {
            Some(rows) => rows.rewrite(expr, &|name| self.no_column(name)),
            None => (expr.clone(), false),
        };
        // An expression of the statement's own outside an aggregate's arguments may aggregate, so
        // it goes in the probe that carries the GROUP BY. A bare column is valid without it, and
        // stays beside the arguments.
        let grouped = self.outside_aggregates() && !matches!(expr.node, Some(NodeEnum::ColumnRef(_)));
        let ctx = self.stack[self.stack.len() - 1].0;
        let probe = match self.drafts.iter().position(|d| d.ctx == ctx && d.row.is_none() && d.grouped == grouped && d.widened == widened) {
            Some(i) => i,
            None => match self.draft(None) {
                Some(i) => {
                    self.drafts[i].grouped = grouped;
                    if widened {
                        let Some(wider) = self.drafts[i].wider.take() else {
                            self.drafts.pop();
                            return Shape::Unknown;
                        };
                        self.drafts[i].from = wider;
                        self.drafts[i].widened = true;
                    }
                    i
                }
                None => return Shape::Unknown,
            },
        };
        let index = self.drafts[probe].items.len();
        self.drafts[probe].items.push(expr);
        let shape = Shape::Column { probe, index };
        if missing || self.nulled_by_grouping() {
            Shape::Nullable(Box::new(shape))
        } else {
            shape
        }
    }

    /// Whether the walk stands in the select list of a top statement that groups or aggregates,
    /// outside the arguments of its aggregates.
    fn outside_aggregates(&self) -> bool {
        self.group.is_some() && self.stack.len() == 1 && self.in_aggregate == 0
    }

    /// Whether a bare name in `expr` may read something else in the probe than in the statement.
    /// The probe reads it over the FROM where the builder stands alone, so a name that may be a
    /// column of a FROM around that one, and is no column of this one for certain, may be the
    /// whole row of an item here or in a subquery of `expr` in the probe (`p` of `posts p` inside
    /// a query over `users`, which has a column `p`): a name is a column of any FROM in scope
    /// before it is an item's row.
    fn renamed_in_probe(&self, expr: &Node) -> bool {
        let depth = self.stack.len() - 1;
        let from = &self.stack[depth].1;
        if self.ahead || depth == 0 || from.is_empty() {
            return false;
        }
        let names = bare_names(expr);
        if names.is_empty() {
            return false;
        }
        let here = Level::of(from, &self.withs[depth], self.columns);
        // An aggregate's FILTER reads the same FROM again, which is no FROM around it.
        let around: Vec<Level> = self.stack[..depth].iter().zip(&self.withs).filter(|((_, f, _), _)| f != from).map(|((_, f, _), withs)| Level::of(f, withs, self.columns)).collect();
        names.iter().any(|n| !here.shows(n) && around.iter().any(|l| !l.lacks(n)))
    }

    /// Whether no FROM in scope shows a column of this name, for certain.
    fn no_column(&self, name: &str) -> bool {
        self.ahead || self.stack.iter().zip(&self.withs).all(|((_, from, _), withs)| Level::of(from, withs, self.columns).lacks(name))
    }

    /// `to_jsonb(u)`: a probe of its own, selecting `u.*`, so the row's own columns are the shape.
    fn row(&mut self, alias: &str) -> Shape {
        // A grouping set nulls the grouping columns of a row, which a probe's row type never shows.
        if self.nulled_by_grouping() {
            return Shape::Unknown;
        }
        let Some(probe) = self.draft(Some(alias.to_string())) else { return Shape::Unknown };
        let star = Node {
            node: Some(NodeEnum::ColumnRef(ColumnRef {
                fields: vec![string_node(alias), Node { node: Some(NodeEnum::AStar(AStar {})) }],
                location: -1,
            })),
        };
        self.drafts[probe].items.push(star);
        Shape::Row { probe }
    }

    fn draft(&mut self, row: Option<String>) -> Option<usize> {
        if self.drafts.len() >= MAX_PROBES {
            self.overflowed = true;
            return None;
        }
        let depth = self.stack.len() - 1;
        let (ctx, from, filter) = self.stack[depth].clone();
        // The FROMs around come first, outermost first, and each subquery of one inside the top
        // statement's is LATERAL: one may read a query around it (`(SELECT … FROM (SELECT … WHERE
        // c.post_id = p.id) c)`), which in the probe is an item left of it. An aggregate's FILTER
        // reads its FROM again, which is no FROM around it.
        let mut wider: Vec<Node> = Vec::new();
        for (d, (_, around, _)) in self.stack[..depth].iter().enumerate() {
            if *around != from {
                wider.extend(around.iter().cloned().map(|item| if d == 0 { item } else { lateral_item(item) }));
            }
        }
        // Wrapped, the FROM keeps its subqueries as they are: one made LATERAL there would read an
        // item beside it before a query around it.
        let inner = (depth > 0 && !wider.is_empty() && from.iter().any(correlated_on)).then_some(wider.len());
        wider.extend(from.iter().cloned().map(|item| if depth == 0 || inner.is_some() { item } else { lateral_item(item) }));
        // A subquery with no `FROM` of its own only ever resolves against the enclosing one.
        let (from, wider) = if from.is_empty() && !wider.is_empty() { (wider, None) } else { (from, Some(wider)) };
        let wider = wider.filter(|w| w.len() != from.len());
        let inner = inner.filter(|_| wider.is_some());
        self.drafts.push(Draft { ctx, row, from, filter, wider, inner, grouped: false, widened: false, items: Vec::new() });
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

/// Whether a write's target goes by the name of one of its WITH queries, which a probe carrying
/// the clause would read in place of the table.
pub(crate) fn target_named_by_with(top: &NodeEnum, with: Option<&protobuf::WithClause>) -> bool {
    let relation = match top {
        NodeEnum::InsertStmt(s) => s.relation.as_ref(),
        NodeEnum::UpdateStmt(s) => s.relation.as_ref(),
        NodeEnum::DeleteStmt(s) => s.relation.as_ref(),
        NodeEnum::MergeStmt(s) => s.relation.as_ref(),
        _ => None,
    };
    let (Some(relation), Some(with)) = (relation, with) else { return false };
    relation.schemaname.is_empty() && with.ctes.iter().any(|c| matches!(c.node.as_ref(), Some(NodeEnum::CommonTableExpr(c)) if c.ctename == relation.relname))
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

/// Whether a SELECT groups by grouping sets: `ROLLUP`, `CUBE` or `GROUPING SETS`.
pub(crate) fn has_grouping_sets(s: &SelectStmt) -> bool {
    s.group_clause.iter().any(|g| matches!(g.node, Some(NodeEnum::GroupingSet(_))))
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
        Some(NodeEnum::CaseExpr(c)) => case_branch(c).is_some(),
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

/// The items of a FROM entry, joins flattened, each with whether an outer join nulls it.
fn from_items<'n>(node: &'n Node, nulled: bool, out: &mut Vec<(&'n Node, bool)>) {
    match node.node.as_ref() {
        Some(NodeEnum::JoinExpr(j)) if j.alias.is_none() => {
            use protobuf::JoinType;
            let (left, right) = match j.jointype {
                t if t == JoinType::JoinLeft as i32 => (nulled, true),
                t if t == JoinType::JoinRight as i32 => (true, nulled),
                t if t == JoinType::JoinFull as i32 => (true, true),
                _ => (nulled, nulled),
            };
            for (side, nulled) in [(j.larg.as_deref(), left), (j.rarg.as_deref(), right)] {
                if let Some(side) = side {
                    from_items(side, nulled, out);
                }
            }
        }
        Some(_) => out.push((node, nulled)),
        None => {}
    }
}

/// A FROM item with every subquery in it, joins followed, made LATERAL: it then reads the items
/// left of it as it read the queries around it, where a name it means is found the same way. A
/// name the probe would find twice, an item of the same alias or a column two of them have, fails
/// it rather than reading another.
fn lateral_item(mut node: Node) -> Node {
    match node.node.as_mut() {
        Some(NodeEnum::RangeSubselect(r)) => r.lateral = true,
        Some(NodeEnum::JoinExpr(j)) => {
            for side in [&mut j.larg, &mut j.rarg] {
                if let Some(inner) = side.take() {
                    *side = Some(Box::new(lateral_item(*inner)));
                }
            }
        }
        _ => {}
    }
    node
}

/// Whether a FROM item is a join whose ON reads an item no join in it holds: one of a query
/// around it, as Postgres reads a name the join does not have. A name in a subquery of the ON is
/// that subquery's own to find.
fn correlated_on(node: &Node) -> bool {
    fn items(node: &Node, names: &mut Vec<String>, quals: &mut Vec<Value>) {
        match node.node.as_ref() {
            Some(NodeEnum::JoinExpr(j)) => {
                names.extend(j.alias.as_ref().map(|a| a.aliasname.clone()));
                quals.extend(j.quals.as_deref().and_then(|q| serde_json::to_value(q).ok()));
                for side in [j.larg.as_deref(), j.rarg.as_deref()].into_iter().flatten() {
                    items(side, names, quals);
                }
            }
            Some(_) => names.extend(item_alias(node)),
            None => {}
        }
    }
    fn qualifiers(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                if let Some(Value::Array(fields)) = map.get("ColumnRef").and_then(|c| c.get("fields")) {
                    let names: Vec<&str> = fields.iter().filter_map(|f| f.get("node")?.get("String")?.get("sval")?.as_str()).collect();
                    if names.len() >= 2 && names.len() == fields.len() {
                        out.push(names[names.len() - 2].to_string());
                    }
                }
                map.iter().filter(|(k, _)| k.as_str() != "SubLink").for_each(|(_, child)| qualifiers(child, out));
            }
            Value::Array(children) => children.iter().for_each(|child| qualifiers(child, out)),
            _ => {}
        }
    }
    if !matches!(node.node, Some(NodeEnum::JoinExpr(_))) {
        return false;
    }
    let (mut names, mut quals) = (Vec::new(), Vec::new());
    items(node, &mut names, &mut quals);
    let mut read = Vec::new();
    quals.iter().for_each(|q| qualifiers(q, &mut read));
    read.iter().any(|q| !names.contains(q))
}

/// The FROM items left of the LATERAL subquery `alias`: the entries before the one holding it, and
/// in that entry the left side of the join whose right side it is. `None` where the subquery is
/// none of those, a join's left side say, which reads nothing left of it in that entry.
fn left_of(from: &[Node], alias: &str) -> Option<Vec<Node>> {
    fn is_it(node: &Node, alias: &str) -> bool {
        matches!(node.node.as_ref(), Some(NodeEnum::RangeSubselect(r)) if r.alias.as_ref().is_some_and(|a| a.aliasname == alias))
    }
    fn left_in(node: &Node, alias: &str) -> Option<Option<Node>> {
        if is_it(node, alias) {
            return Some(None);
        }
        let Some(NodeEnum::JoinExpr(j)) = node.node.as_ref() else { return None };
        // A join with an alias of its own hides the items inside it, as `from_items` reads them.
        if j.alias.is_some() {
            return None;
        }
        if j.rarg.as_deref().is_some_and(|r| is_it(r, alias)) {
            return Some(Some(j.larg.as_deref()?.clone()));
        }
        left_in(j.larg.as_deref()?, alias).filter(|left| left.is_some())
    }
    let at = from.iter().position(|item| left_in(item, alias).is_some())?;
    let mut left: Vec<Node> = from[..at].to_vec();
    left.extend(left_in(&from[at], alias)?);
    Some(left)
}

/// The name a FROM item goes by: its alias, or a table's own name.
fn item_alias(node: &Node) -> Option<String> {
    match node.node.as_ref()? {
        NodeEnum::RangeVar(rv) => Some(rv.alias.as_ref().map_or(rv.relname.clone(), |a| a.aliasname.clone())),
        NodeEnum::RangeSubselect(r) => r.alias.as_ref().map(|a| a.aliasname.clone()),
        NodeEnum::RangeFunction(f) => f.alias.as_ref().map(|a| a.aliasname.clone()),
        _ => None,
    }
}

/// The columns an alias's column list names, which rename a FROM item's first ones.
fn alias_columns(alias: Option<&protobuf::Alias>) -> Vec<Node> {
    alias.map(|a| a.colnames.clone()).unwrap_or_default()
}

/// The names a FROM item's columns go by: its statement's targets, renamed in order by each
/// column list, which renames the first ones. A name that is `None` is not known. Nothing where a
/// list names more columns than there are, which Postgres refuses.
fn column_names(body: &SelectStmt, lists: &[Vec<Node>]) -> Option<Vec<Option<String>>> {
    let mut names: Vec<Option<String>> = body
        .target_list
        .iter()
        .map(|t| match t.node.as_ref() {
            Some(NodeEnum::ResTarget(rt)) => target_name(rt),
            _ => None,
        })
        .collect();
    for list in lists {
        if list.len() > names.len() {
            return None;
        }
        for (name, given) in names.iter_mut().zip(list) {
            *name = Some(last_string(std::slice::from_ref(given))?);
        }
    }
    Some(names)
}

/// The one branch of a CASE that builds JSON, where every other branch is NULL (no `ELSE` is
/// `ELSE NULL`). A CASE has a `WHEN`, so with one such branch it always has a NULL one too.
fn case_branch(c: &protobuf::CaseExpr) -> Option<&Node> {
    let mut built: Option<&Node> = None;
    let results = c.args.iter().map(|w| match w.node.as_ref() {
        Some(NodeEnum::CaseWhen(w)) => w.result.as_deref(),
        _ => None,
    });
    for result in results.chain(c.defresult.as_deref().map(Some)) {
        match result {
            Some(r) if is_null_const(r) => {}
            Some(r) if built.is_none() && is_constructor(r) => built = Some(r),
            _ => return None,
        }
    }
    built
}

/// A NULL constant, under casts.
fn is_null_const(node: &Node) -> bool {
    match node.node.as_ref() {
        Some(NodeEnum::AConst(c)) => c.isnull,
        Some(NodeEnum::TypeCast(t)) => t.arg.as_deref().is_some_and(is_null_const),
        _ => false,
    }
}

/// Whether the top of this expression is an aggregate, which is what makes an ungrouped
/// subquery return exactly one row. A call with `OVER` is a window function, one row per input.
fn aggregates(expr: &Node, level: &Level, trust: &Trust) -> bool {
    match expr.node.as_ref() {
        Some(NodeEnum::FuncCall(f)) => f.over.is_none() && crate::infer::builtin_name(&f.funcname).is_some_and(|n| AGG_FNS.contains(&n.as_str()) && trust.aggregates(&n, f.args.len())) && level.owns(f),
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
    grouped_select_text(version, items, from, filter, &[], with)
}

/// The alias of the subquery [`wrapped_text`] reads its columns from.
const WRAPPED: &str = "reylo_probe";

/// `SELECT * FROM <around>, LATERAL (SELECT <items> FROM <from>) AS reylo_probe`: a probe whose
/// FROM reads the items of `around` as a subquery reads a query around it.
fn wrapped_text(version: i32, items: &[Node], around: &[Node], from: &[Node], filter: Option<&Node>, with: Option<&protobuf::WithClause>) -> Option<String> {
    let inner = select_stmt(items, from, filter, &[])?;
    let mut outer = around.to_vec();
    outer.push(Node {
        node: Some(NodeEnum::RangeSubselect(Box::new(protobuf::RangeSubselect {
            lateral: true,
            subquery: Some(Box::new(Node { node: Some(NodeEnum::SelectStmt(Box::new(inner))) })),
            alias: Some(protobuf::Alias { aliasname: WRAPPED.to_string(), colnames: Vec::new() }),
        }))),
    });
    let star = Node { node: Some(NodeEnum::ColumnRef(ColumnRef { fields: vec![string_node(WRAPPED), Node { node: Some(NodeEnum::AStar(AStar {})) }], location: -1 })) };
    grouped_select_text(version, &[star], &outer, None, &[], with)
}

/// `SELECT <items> FROM <from> WHERE <filter> GROUP BY <group>`, each item named by its position.
fn select_stmt(items: &[Node], from: &[Node], filter: Option<&Node>, group: &[Node]) -> Option<SelectStmt> {
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
    Some(SelectStmt {
        target_list,
        from_clause: from.to_vec(),
        where_clause: filter.map(|f| Box::new(f.clone())),
        group_clause: group.to_vec(),
        op: protobuf::SetOperation::SetopNone as i32,
        limit_option: protobuf::LimitOption::Default as i32,
        ..Default::default()
    })
}

/// [`select_text`] with a GROUP BY.
fn grouped_select_text(version: i32, items: &[Node], from: &[Node], filter: Option<&Node>, group: &[Node], with: Option<&protobuf::WithClause>) -> Option<String> {
    let mut select = select_stmt(items, from, filter, group)?;
    let deparse = |select: &SelectStmt| {
        let result = ParseResult {
            version,
            stmts: vec![RawStmt {
                stmt: Some(Box::new(Node { node: Some(NodeEnum::SelectStmt(Box::new(select.clone()))) })),
                stmt_location: 0,
                stmt_len: 0,
            }],
        };
        crate::stack::deparse(&result).ok()
    };
    let body = deparse(&select)?;
    // A probe binds nothing, so a placeholder in its condition is NULL there: a test that is
    // strict in a column stays strict (`u.id = NULL`), which is all the probe reads of it, and a
    // condition the server cannot type proves nothing, the probe failing to describe.
    let nulled = without_placeholders(&body);
    // The statement's WITH queries go with a probe that reads one, and with nothing else, so a
    // probe that reads none can be described before the statement is. They keep their
    // placeholders, which the describe pins to the types the statement's own describe gave them:
    // a query that reads one is then the very text the statement's own read of it settled, and
    // its columns come back settled, at no cost. Written NULL, it would be another statement,
    // settled again, and a column selecting a placeholder would be nullable there.
    let Some(with) = with.filter(|w| reads_with(&select, w)) else { return Some(nulled) };
    select.with_clause = Some(with.clone());
    let text = deparse(&select)?;
    match text.strip_suffix(body.as_str()) {
        Some(head) => Some(format!("{head}{nulled}")),
        None => Some(without_placeholders(&text)),
    }
}

/// Whether `select` names one of the queries of `with`: a relation by the name of one, without a
/// schema, anywhere in it. A subquery's own WITH query of that name is read as one too, which
/// only ever carries the clause where it is not needed.
fn reads_with(select: &SelectStmt, with: &protobuf::WithClause) -> bool {
    let names: Vec<&str> = with.ctes.iter().filter_map(|c| match c.node.as_ref() {
        Some(NodeEnum::CommonTableExpr(c)) => Some(c.ctename.as_str()),
        _ => None,
    }).collect();
    fn hunt(v: &Value, names: &[&str]) -> bool {
        match v {
            Value::Object(map) => {
                let named = map.get("RangeVar").is_some_and(|r| {
                    r.get("schemaname").and_then(Value::as_str).is_none_or(str::is_empty) && r.get("relname").and_then(Value::as_str).is_some_and(|n| names.contains(&n))
                });
                named || map.values().any(|child| hunt(child, names))
            }
            Value::Array(items) => items.iter().any(|child| hunt(child, names)),
            _ => false,
        }
    }
    serde_json::to_value(select).is_ok_and(|v| hunt(&v, &names))
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
    /// A probe's column as a type: what the catalog says of it, with what the probe's own
    /// analysis said of an array it builds (`array[x, null]` holds a NULL element) and of the
    /// values and the document a CHECK keeps it to, as a result column gets them.
    fn column(&self, col: &DescribedColumn) -> Type {
        let ty = if col.whole_row { self.registry.render_row(col.type_oid) } else { self.registry.render(col.type_oid) };
        crate::postgres::with_document(crate::postgres::with_values(crate::postgres::with_items(ty, col.nullable_items), col.values.as_deref()), col.document.as_ref())
    }

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
                Some((col, None)) if col.type_oid != oid::UNKNOWN => (self.column(col), !col.nullable),
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
                        .enumerate()
                        .map(|(i, col)| {
                            let ty = if col.type_oid == oid::UNKNOWN {
                                unresolved.push(display_path(&join(path, &col.name)));
                                Type::Unknown
                            } else {
                                self.column(col)
                            };
                            // A field is as the read with the row there has it: the outer join that
                            // nulls the object nulls none of a row that exists.
                            let present = outcome.present.as_ref().and_then(|p| p.get(i)).is_none_or(|nullable| *nullable);
                            Field { name: col.name.clone(), ty, nullable: col.nullable && present }
                        })
                        .collect();
                    // An outer join can null the whole row, which is then every column NULL: a
                    // column the probe proved non-null says the row is there.
                    (Type::Object { fields }, !outcome.outer_join || outcome.columns.iter().any(|c| !c.nullable))
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

/// `from` with each outer join that may null `alias`'s row made one that keeps the row: a LEFT
/// JOIN with the alias on its right side, or a RIGHT JOIN with it on its left, an inner one, and a
/// FULL JOIN one that preserves the alias's side. The rows where the alias's row is there are the
/// same rows: an outer join puts its nullable side's row only beside a row its ON held for, as an
/// inner join does, and a FULL JOIN's side keeps every row of its own either way. `None` where no
/// join can null the alias, or none of `from` goes by it.
pub(crate) fn row_present(from: &[Node], alias: &str) -> Option<Vec<Node>> {
    use protobuf::JoinType;
    /// Whether `node` holds the item `alias` names, making the joins above it keep its row.
    fn keep(node: &mut Node, alias: &str, changed: &mut bool) -> bool {
        let named = |a: Option<&protobuf::Alias>| a.is_some_and(|a| a.aliasname == alias);
        match node.node.as_mut() {
            Some(NodeEnum::RangeVar(rv)) => rv.alias.as_ref().map_or(rv.relname.as_str(), |a| a.aliasname.as_str()) == alias,
            Some(NodeEnum::RangeSubselect(r)) => named(r.alias.as_ref()),
            Some(NodeEnum::RangeFunction(r)) => named(r.alias.as_ref()),
            Some(NodeEnum::RangeTableFunc(r)) => named(r.alias.as_ref()),
            // A join with an alias of its own hides the names inside it.
            Some(NodeEnum::JoinExpr(j)) if j.alias.is_some() => named(j.alias.as_ref()),
            Some(NodeEnum::JoinExpr(j)) => {
                let left = j.larg.as_deref_mut().is_some_and(|n| keep(n, alias, changed));
                let right = j.rarg.as_deref_mut().is_some_and(|n| keep(n, alias, changed));
                let kept = match JoinType::try_from(j.jointype) {
                    Ok(JoinType::JoinLeft) if right => Some(JoinType::JoinInner),
                    Ok(JoinType::JoinRight) if left => Some(JoinType::JoinInner),
                    Ok(JoinType::JoinFull) if left => Some(JoinType::JoinLeft),
                    Ok(JoinType::JoinFull) if right => Some(JoinType::JoinRight),
                    _ => None,
                };
                if let Some(kept) = kept {
                    j.jointype = kept as i32;
                    *changed = true;
                }
                left || right
            }
            _ => false,
        }
    }
    let mut from = from.to_vec();
    let mut changed = false;
    for item in &mut from {
        keep(item, alias, &mut changed);
    }
    changed.then_some(from)
}

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
pub(crate) fn target_name(rt: &ResTarget) -> Option<String> {
    if !rt.name.is_empty() {
        return Some(rt.name.clone());
    }
    default_name(rt.val.as_deref()?)
}

/// The name Postgres gives a target with no alias, as its `FigureColname` does for the
/// expressions a constructor target is: a function's own name, `coalesce`, a cast's operand's
/// name before the type's, and a scalar subquery's column's. `?column?` where it finds none.
fn default_name(node: &Node) -> Option<String> {
    Some(figure(node).map_or_else(|| "?column?".to_string(), |(name, _)| name))
}

/// The name `FigureColnameInternal` finds for an expression and how strongly: 2 for a name of
/// its own, 1 for a cast's type, which an outer cast's operand name outranks.
fn figure(node: &Node) -> Option<(String, u8)> {
    match node.node.as_ref()? {
        NodeEnum::ColumnRef(c) => match c.fields.last()?.node.as_ref()? {
            NodeEnum::String(s) => Some((s.sval.clone(), 2)),
            _ => None,
        },
        NodeEnum::FuncCall(f) => Some((func_name(f)?, 2)),
        NodeEnum::CoalesceExpr(_) => Some(("coalesce".to_string(), 2)),
        NodeEnum::TypeCast(t) => match t.arg.as_deref().and_then(figure) {
            Some((name, 2)) => Some((name, 2)),
            _ => Some((t.type_name.as_ref().and_then(|tn| last_string(&tn.names))?, 1)),
        },
        NodeEnum::CollateClause(c) => figure(c.arg.as_deref()?),
        // The subquery's one column, by the name it goes by there.
        NodeEnum::SubLink(s) if s.sub_link_type == protobuf::SubLinkType::ExprSublink as i32 => {
            let rt = single_target(as_select(s.subselect.as_deref()?)?)?;
            Some((target_name(rt)?, 2))
        }
        NodeEnum::SubLink(s) if s.sub_link_type == protobuf::SubLinkType::ExistsSublink as i32 => Some(("exists".to_string(), 2)),
        NodeEnum::SubLink(s) if s.sub_link_type == protobuf::SubLinkType::ArraySublink as i32 => Some(("array".to_string(), 2)),
        NodeEnum::CaseExpr(c) => match c.defresult.as_deref().and_then(figure) {
            Some((name, 2)) => Some((name, 2)),
            _ => Some(("case".to_string(), 1)),
        },
        NodeEnum::AArrayExpr(_) => Some(("array".to_string(), 2)),
        NodeEnum::RowExpr(_) => Some(("row".to_string(), 2)),
        NodeEnum::MinMaxExpr(m) => Some((if m.op == protobuf::MinMaxOp::IsGreatest as i32 { "greatest" } else { "least" }.to_string(), 2)),
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

/// A default that leaves a `coalesce` over a placeholder the placeholder's type, as JSON writes it:
/// a NULL or an untyped string, which take that type, and an integer or a boolean, which coalesce
/// only with a type of their own kind (`int2` widens to `int4`, an integer all the same).
fn keeps_param_type(node: &Node) -> bool {
    use protobuf::a_const::Val;
    matches!(node.node.as_ref(), Some(NodeEnum::AConst(c)) if c.isnull || matches!(c.val, Some(Val::Sval(_) | Val::Ival(_) | Val::Boolval(_))))
}

fn casts_to_json(t: &protobuf::TypeCast) -> bool {
    t.type_name
        .as_ref()
        .and_then(|tn| last_string(&tn.names))
        .is_some_and(|n| n == "json" || n == "jsonb")
}

/// The `u` of `u.*`, a whole row by its alias.
fn star_alias(node: &Node) -> Option<String> {
    match node.node.as_ref()? {
        NodeEnum::ColumnRef(c) if c.fields.len() == 2 && is_star(node) => last_string(&c.fields[..1]),
        _ => None,
    }
}

/// Every name a column reference of one field in `expr` reads, inside its subqueries too.
fn bare_names(expr: &Node) -> Vec<String> {
    fn sval(v: &Value) -> Option<&str> {
        match v {
            Value::Object(map) => map.iter().find_map(|(key, child)| if key == "sval" { child.as_str() } else { sval(child) }),
            _ => None,
        }
    }
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                for (key, child) in map {
                    let one = child.get("fields").and_then(Value::as_array).filter(|f| key == "ColumnRef" && f.len() == 1);
                    match one.and_then(|f| sval(&f[0])) {
                        Some(name) => out.push(name.to_string()),
                        None => walk(child, out),
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Ok(tree) = serde_json::to_value(expr) {
        walk(&tree, &mut out);
    }
    out
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
    fn a_correlated_derived_table_is_read_after_the_queries_around_it() {
        // The derived table reads `p` of the query around it, which the wider probe holds as an
        // item left of it: LATERAL, the derived table reads it there.
        let p = shapes(
            "SELECT p.id, (SELECT coalesce(json_agg(json_build_object('id', c.id)), '[]')
                FROM (SELECT c.id FROM comments c WHERE c.post_id = p.id LIMIT 2) c) AS comments
             FROM posts p",
        );
        assert_eq!(p.probes[0].sql, "SELECT c.id AS a0 FROM (SELECT c.id FROM comments c WHERE c.post_id = p.id LIMIT 2) c");
        assert_eq!(p.probes[0].wider.as_deref(), Some("SELECT c.id AS a0 FROM posts p, LATERAL (SELECT c.id FROM comments c WHERE c.post_id = p.id LIMIT 2) c"));
    }

    #[test]
    fn a_join_that_reads_a_query_around_it_in_its_on_is_read_in_a_lateral_subquery() {
        // No item beside a join is in scope in its ON, so the FROM is a subquery of the probe's.
        let p = shapes(
            "SELECT p.id, (SELECT json_agg(json_build_object('id', c.id, 'name', u.name))
                FROM comments c JOIN users u ON u.id = c.user_id AND c.post_id = p.id) AS comments
             FROM posts p",
        );
        assert_eq!(
            p.probes[0].wider.as_deref(),
            Some("SELECT reylo_probe.* FROM posts p, LATERAL (SELECT c.id AS a0, u.name AS a1 FROM comments c JOIN users u ON u.id = c.user_id AND c.post_id = p.id) reylo_probe")
        );
        // A join whose ON reads its own items alone is no such join.
        let p = shapes(
            "SELECT p.id, (SELECT json_agg(json_build_object('id', c.id, 'name', u.name))
                FROM comments c JOIN users u ON u.id = c.user_id WHERE c.post_id = p.id) AS comments
             FROM posts p",
        );
        assert_eq!(p.probes[0].wider.as_deref(), Some("SELECT c.id AS a0, u.name AS a1 FROM posts p, comments c JOIN users u ON u.id = c.user_id"));
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
        assert_eq!(p.probes[0].wider.as_deref(), Some("SELECT c.id AS a0, c.body AS a1 FROM posts p, comments c"));
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
    fn an_aggregate_beside_a_column_is_probed_under_the_group_by() {
        let probes = |sql: &str| shapes(sql).probes.into_iter().map(|p| p.sql).collect::<Vec<_>>();
        // The statement's own expressions under its GROUP BY, its bare columns and the arguments
        // of its aggregates without it.
        assert_eq!(
            probes("SELECT jsonb_build_object('name', u.name, 'n', count(p.id), 'titles', jsonb_agg(p.title)) AS j FROM users u JOIN posts p ON p.author_id = u.id GROUP BY u.id"),
            vec![
                "SELECT u.name AS a0, p.title AS a1 FROM users u JOIN posts p ON p.author_id = u.id",
                "SELECT count(p.id) AS a0 FROM users u JOIN posts p ON p.author_id = u.id GROUP BY u.id",
            ]
        );
        // A statement that aggregates without a GROUP BY probes its aggregates without one.
        assert_eq!(
            probes("SELECT jsonb_build_object('titles', jsonb_agg(p.title), 'n', count(*)) AS j FROM posts p"),
            vec!["SELECT p.title AS a0 FROM posts p", "SELECT count(*) AS a0 FROM posts p"]
        );
        // One that neither groups nor aggregates keeps its one probe, and a GROUP BY by position
        // or by a name of the select list names the probe's own columns, so it is not carried.
        assert_eq!(probes("SELECT jsonb_build_object('n', lower(u.name), 'id', u.id) AS j FROM users u"), vec!["SELECT lower(u.name) AS a0, u.id AS a1 FROM users u"]);
        assert_eq!(probes("SELECT jsonb_build_object('n', count(*), 'name', u.name) AS j FROM users u GROUP BY 1").len(), 1);
        assert_eq!(
            probes("SELECT jsonb_build_object('n', count(*), 'm', max(p.title)) AS j, lower(p.title) AS t FROM posts p GROUP BY t"),
            vec!["SELECT count(*) AS a0, max(p.title) AS a1 FROM posts p"]
        );
        assert_eq!(probes("SELECT jsonb_build_object('n', count(*)) AS j, lower(p.title) FROM posts p GROUP BY lower"), vec!["SELECT count(*) AS a0 FROM posts p"]);
        // A bare name no target carries, or a column's own, is the FROM's, and is carried.
        assert_eq!(
            probes("SELECT jsonb_build_object('n', count(*)) AS j, p.title FROM posts p GROUP BY title"),
            vec!["SELECT count(*) AS a0 FROM posts p GROUP BY title"]
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
        // `u.*` as a function's argument is the whole row too, never the row's first column,
        // under an aggregate, its FILTER, an object and `to_jsonb` alike.
        for sql in [
            "SELECT jsonb_agg(u.*) AS authors FROM users u",
            "SELECT json_agg(u.*) FILTER (WHERE u.id IS NOT NULL) AS authors FROM users u",
        ] {
            let p = shapes(sql);
            assert_eq!(p.columns[0].1, Shape::Array { items: Box::new(Shape::Row { probe: 0 }), non_null: false }, "{sql}");
            assert!(p.probes[0].sql.starts_with("SELECT u.* FROM users u"), "{sql}: {}", p.probes[0].sql);
        }
        let o = shapes("SELECT jsonb_build_object('author', u.*) AS j FROM users u");
        assert_eq!(o.columns[0].1, Shape::Object { fields: vec![("author".into(), Shape::Row { probe: 0 })], non_null: true });
        assert_eq!(shapes("SELECT to_jsonb(u.*) AS author FROM users u").columns[0].1, Shape::Row { probe: 0 });
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
        let resolved = |sql: &str| resolve(&shapes(sql).columns[0].1, &[Some(Outcome { columns: found.clone(), outer_join: false, present: None, built: Vec::new() })], &registry, &[]);
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
        assert_eq!(inner.probes[0].wider.as_deref(), Some("SELECT new.title AS a0, users.name AS a1 FROM users, posts new"));
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
        // Under a default that cannot widen it, a coalesce of one is the placeholder; under a
        // decimal it is a numeric, which no probe can read.
        let c = shapes("SELECT jsonb_build_object('q', coalesce($1::text, 'd'), 'n', coalesce($2::int, 0, NULL), 'x', coalesce($3::int, 1.5)) AS x FROM todos t");
        let Shape::Object { fields, .. } = &c.columns[0].1 else { panic!() };
        assert_eq!(fields.iter().map(|f| f.1.clone()).collect::<Vec<_>>(), [Shape::Param(1), Shape::Param(2), Shape::Unknown]);
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
    fn a_cte_travels_with_the_probe_that_reads_it() {
        let p = shapes("WITH recent AS (SELECT id, body FROM comments) SELECT jsonb_build_object('b', r.body) AS x FROM recent r");
        assert_eq!(p.probes[0].sql, "WITH recent AS (SELECT id, body FROM comments) SELECT r.body AS a0 FROM recent r");
        // Its placeholders stay, for the describe to pin; the probe's own condition reads NULL.
        let q = shapes("WITH input AS (SELECT $1::uuid AS id) SELECT jsonb_build_object('b', c.body) AS x FROM comments c, input i WHERE c.id = i.id AND c.at > $2");
        assert_eq!(q.probes[0].sql, "WITH input AS (SELECT $1::uuid AS id) SELECT c.body AS a0 FROM comments c, input i WHERE c.id = i.id AND c.at > NULL");
        // A probe that reads none of them goes without, and so binds nothing.
        let r = shapes("WITH input AS (SELECT $1::uuid AS id) SELECT (SELECT json_agg(json_build_object('b', c.body)) FROM comments c) AS x FROM input i");
        assert_eq!(r.probes[0].sql, "SELECT c.body AS a0 FROM comments c");
        // A write to a table its WITH clause names a query after reads the table, not the query.
        assert!(shapes("WITH users AS (SELECT $1::int AS id) UPDATE users SET name = 'x' RETURNING jsonb_build_object('n', name) AS j").columns.is_empty());
        // `public.input` is the table, whatever the WITH clause calls `input`.
        let s = shapes("WITH input AS (SELECT $1::uuid AS id) SELECT jsonb_build_object('b', c.body) AS x FROM public.input c");
        assert_eq!(s.probes[0].sql, "SELECT c.body AS a0 FROM public.input c");
    }

    #[test]
    fn a_dml_statement_whose_with_names_its_target_has_no_shapes() {
        assert!(shapes("WITH team AS (SELECT 1 AS motto) UPDATE team SET name = name RETURNING jsonb_build_object('m', motto) AS j").columns.is_empty());
        assert!(!shapes("WITH other AS (SELECT 1 AS motto) UPDATE team SET name = name RETURNING jsonb_build_object('m', motto) AS j").columns.is_empty());
        // A qualified target is the relation, whatever the WITH calls its name.
        assert!(!shapes("WITH team AS (SELECT 1 AS motto) UPDATE public.team SET name = name RETURNING jsonb_build_object('m', motto) AS j").columns.is_empty());
    }

    #[test]
    fn a_grouping_set_nulls_a_value_but_a_plain_aggregates_argument() {
        let leaf = |sql: &str| match &shapes(sql).columns[0].1 {
            Shape::Object { fields, .. } => fields[0].1.clone(),
            Shape::Array { items, .. } => (**items).clone(),
            other => other.clone(),
        };
        assert!(matches!(leaf("SELECT jsonb_build_object('n', name) AS o FROM team GROUP BY ROLLUP (name)"), Shape::Nullable(_)));
        assert!(matches!(leaf("SELECT jsonb_agg(name) OVER () AS o FROM team GROUP BY ROLLUP (name)"), Shape::Nullable(_)));
        assert!(matches!(leaf("SELECT jsonb_agg(name) AS o FROM team GROUP BY ROLLUP (id)"), Shape::Column { .. }));
        assert!(matches!(leaf("SELECT jsonb_build_object('n', name) AS o FROM team GROUP BY name"), Shape::Column { .. }));
        // What is built is a grouping expression where it is grouped by, by name, position or
        // expression, so nothing built is non-null; under a plain GROUP BY it is.
        for sql in ["SELECT jsonb_build_object('n', name) AS o FROM team GROUP BY ROLLUP (o)", "SELECT jsonb_build_object('a', jsonb_build_object('b', name)) AS o FROM team GROUP BY ROLLUP ((jsonb_build_object('b', name)))"] {
            let Shape::Object { non_null, fields } = &shapes(sql).columns[0].1 else { panic!("{sql}") };
            assert!(!non_null && !matches!(fields[0].1, Shape::Object { non_null: true, .. }), "{sql}");
        }
        assert!(matches!(shapes("SELECT jsonb_build_object('n', name) AS o FROM team GROUP BY o").columns[0].1, Shape::Object { non_null: true, .. }));
        // A whole row under a grouping set, and a subquery with a grouping set of its own, are read as nothing.
        assert!(shapes("SELECT to_jsonb(t) AS o FROM team t GROUP BY ROLLUP (t.id)").columns.is_empty());
        assert!(shapes("SELECT (SELECT jsonb_build_object('n', name) FROM team GROUP BY CUBE (name) LIMIT 1) AS o").columns.is_empty());
    }

    #[test]
    fn a_lateral_subquery_is_read_in_its_own_context_and_the_items_left_of_it() {
        let p = shapes("SELECT u.id, x.items FROM users u LEFT JOIN LATERAL (SELECT jsonb_agg(jsonb_build_object('sku', p.sku)) AS items FROM products p WHERE p.org_id = u.org_id) x ON true");
        assert_eq!(p.columns.len(), 1);
        assert_eq!(p.columns[0].0, "items");
        // On the nullable side of a LEFT JOIN, the whole value may be NULL.
        assert!(matches!(&p.columns[0].1, Shape::Array { non_null: false, .. }));
        assert_eq!(p.probes[0].sql, "SELECT p.sku AS a0 FROM products p WHERE p.org_id = u.org_id");
        // The wider probe reads what the subquery can: the items left of it, and not the join.
        assert_eq!(p.probes[0].wider.as_deref(), Some("SELECT p.sku AS a0 FROM users u, products p WHERE p.org_id = u.org_id"));
        let p = shapes("SELECT x.o FROM orgs o, users u CROSS JOIN LATERAL (SELECT jsonb_build_object('n', u.name) AS o) x, teams t");
        assert_eq!(p.probes[0].sql, "SELECT u.name AS a0 FROM orgs o, users u");
        // A join with an alias of its own hides a subquery of the same name inside it.
        let p = shapes("SELECT x.o FROM (orgs u CROSS JOIN LATERAL (SELECT 1 AS o) x) j, users u CROSS JOIN LATERAL (SELECT jsonb_build_object('n', u.name) AS o) x");
        assert_eq!(p.probes[0].sql, "SELECT u.name AS a0 FROM (orgs u CROSS JOIN LATERAL (SELECT 1 AS o) x ) j, users u");
        // A subquery that is no LATERAL one is a statement of its own, which the probe follows.
        let p = shapes("SELECT x.o FROM (SELECT jsonb_build_object('n', name) AS o FROM users) x");
        assert!(p.columns.is_empty());
        // Its column list renames its columns in order, and two of a name are none.
        let p = shapes("SELECT x.b FROM users u CROSS JOIN LATERAL (SELECT jsonb_build_object('n', u.name) AS a, '{}'::jsonb AS b) x (b, a)");
        assert!(matches!(&p.columns[0].1, Shape::Object { .. }));
        assert!(shapes("SELECT x.a FROM users u CROSS JOIN LATERAL (SELECT jsonb_build_object('n', u.name) AS a, jsonb_build_object('m', 1) AS a) x").columns.is_empty());
    }

    #[test]
    fn a_case_whose_one_branch_builds_json_and_the_others_are_null_is_that_branch() {
        let first = |sql: &str| shapes(sql).columns.into_iter().next().map(|(_, s)| s);
        assert!(matches!(first("SELECT CASE WHEN id > 1 THEN jsonb_build_object('a', 1) END AS o FROM t"), Some(Shape::Object { non_null: false, .. })));
        assert!(matches!(first("SELECT CASE WHEN id > 1 THEN NULL ELSE jsonb_build_object('a', 1) END AS o FROM t"), Some(Shape::Object { non_null: false, .. })));
        assert!(matches!(first("SELECT CASE WHEN id > 1 THEN jsonb_build_object('a', 1) ELSE jsonb_build_object('a', 1) END AS o FROM t"), None));
        assert!(matches!(first("SELECT CASE WHEN id > 1 THEN jsonb_build_object('a', 1) ELSE '{}' END AS o FROM t"), None));
        // A CASE whose ELSE builds goes by the constructor's name, one whose ELSE is NULL by `case`.
        assert_eq!(shapes("SELECT CASE WHEN id > 1 THEN NULL ELSE jsonb_build_object('a', 1) END FROM t").columns[0].0, "jsonb_build_object");
        assert_eq!(shapes("SELECT CASE WHEN id > 1 THEN jsonb_build_object('a', 1) END FROM t").columns[0].0, "case");
    }

    #[test]
    fn a_subquery_with_a_with_of_its_own_is_read_as_nothing() {
        assert!(shapes("SELECT (WITH team AS (SELECT 1 AS name) SELECT jsonb_agg(team.name) FROM team) AS v").columns.is_empty());
    }

    #[test]
    fn a_target_without_an_alias_goes_by_the_name_postgres_gives_it() {
        let names = |sql: &str| shapes(sql).columns.into_iter().map(|(n, _)| n).collect::<Vec<_>>();
        assert_eq!(names("SELECT jsonb_build_object('k', 1)::jsonb FROM team"), vec!["jsonb_build_object"]);
        assert_eq!(names("SELECT coalesce(jsonb_agg(id), '[]')::json FROM team"), vec!["coalesce"]);
        assert_eq!(names("SELECT (SELECT jsonb_agg(id) AS ids FROM team)::jsonb"), vec!["ids"]);
        assert_eq!(names("SELECT (SELECT jsonb_agg(id) FROM team)"), vec!["jsonb_agg"]);
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
            present: None,
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
            present: None,
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
            present: None,
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
        let r = resolve(&p.columns[0].1, &[Some(Outcome { columns: columns.clone(), outer_join: false, present: None, built: Vec::new() })], &registry, &[]);
        assert!(r.non_null);
        assert_eq!(r.ty, Type::Object { fields: vec![field("id", scalar("uuid"), false), field("name", scalar("text"), true)] });
        // An outer join in the probe's own analysis is what can null the whole row, unless a
        // column it proved non-null says the row is there (an unmatched row is every column NULL).
        let r = resolve(&p.columns[0].1, &[Some(Outcome { columns: columns.clone(), outer_join: true, present: None, built: Vec::new() })], &registry, &[]);
        assert!(r.non_null);
        let nullable: Vec<_> = columns.into_iter().map(|c| DescribedColumn { nullable: true, ..c }).collect();
        let r = resolve(&p.columns[0].1, &[Some(Outcome { columns: nullable.clone(), outer_join: true, present: None, built: Vec::new() })], &registry, &[]);
        assert!(!r.non_null);
        assert_eq!(r.ty, Type::Object { fields: vec![field("id", scalar("uuid"), true), field("name", scalar("text"), true)] });
        // The read with the row there gives the fields of an object that exists their own.
        let r = resolve(&p.columns[0].1, &[Some(Outcome { columns: nullable, outer_join: true, present: Some(vec![false, true]), built: Vec::new() })], &registry, &[]);
        assert!(!r.non_null);
        assert_eq!(r.ty, Type::Object { fields: vec![field("id", scalar("uuid"), false), field("name", scalar("text"), true)] });
    }

    #[test]
    fn a_bare_name_a_probe_would_read_as_a_row_is_read_over_every_from() {
        // `name` is a column of `users`, which the statement reads it as; over `posts name` alone
        // it would be the whole row of `posts`.
        let p = shapes("SELECT (SELECT jsonb_agg(name) FROM posts name WHERE name.id = 1) AS j FROM users");
        assert_eq!(p.probes.iter().map(|p| p.sql.as_str()).collect::<Vec<_>>(), ["SELECT name AS a0 FROM users, posts name"]);
        // Inside an expression too, and beside a leaf that keeps the narrow probe.
        let p = shapes("SELECT (SELECT jsonb_build_object('n', lower(name), 't', name.title) FROM posts name LIMIT 1) AS j FROM users");
        assert_eq!(p.probes.iter().map(|p| p.sql.as_str()).collect::<Vec<_>>(), ["SELECT lower(name) AS a0 FROM users, posts name", "SELECT name.title AS a0 FROM posts name"]);
        // So is one in a subquery inside, whose own FROM has an item of the name.
        let p = shapes("SELECT (SELECT jsonb_build_object('n', (SELECT count(name) FROM comments name)) FROM posts p LIMIT 1) AS j FROM users");
        assert_eq!(p.probes.iter().map(|p| p.sql.as_str()).collect::<Vec<_>>(), ["SELECT count(name) AS a0 FROM users, posts p, comments name"]);
        // A name of a column here, or of no column around, is read as it is.
        let p = shapes("SELECT (SELECT jsonb_agg(title) FROM posts title) AS j FROM users");
        assert_eq!(p.probes.iter().map(|p| p.sql.as_str()).collect::<Vec<_>>(), ["SELECT title AS a0 FROM posts title"]);
        let p = shapes("SELECT (SELECT jsonb_agg(c.body) FROM comments c) AS j FROM users");
        assert_eq!(p.probes.iter().map(|p| p.sql.as_str()).collect::<Vec<_>>(), ["SELECT c.body AS a0 FROM comments c"]);
    }

    #[test]
    fn a_row_an_outer_join_may_null_is_read_again_with_the_row_there() {
        let present = |sql: &str| shapes(sql).present.into_iter().map(|p| p.map(|p| p.sql)).collect::<Vec<_>>();
        assert_eq!(present("SELECT to_jsonb(p) AS j FROM users u LEFT JOIN posts p ON p.user_id = u.id"), [Some("SELECT p.* FROM users u JOIN posts p ON p.user_id = u.id".to_string())]);
        assert_eq!(present("SELECT to_jsonb(u) AS j FROM users u RIGHT JOIN posts p ON p.user_id = u.id"), [Some("SELECT u.* FROM users u JOIN posts p ON p.user_id = u.id".to_string())]);
        // A FULL JOIN keeps every row of the alias's side, matched or not.
        assert_eq!(present("SELECT to_jsonb(u) AS j FROM users u FULL JOIN posts p ON p.user_id = u.id"), [Some("SELECT u.* FROM users u LEFT JOIN posts p ON p.user_id = u.id".to_string())]);
        assert_eq!(present("SELECT to_jsonb(p) AS j FROM users u FULL JOIN posts p ON p.user_id = u.id"), [Some("SELECT p.* FROM users u RIGHT JOIN posts p ON p.user_id = u.id".to_string())]);
        // Every join above the alias that nulls it, and none that does not.
        assert_eq!(
            present("SELECT to_jsonb(c) AS j FROM users u LEFT JOIN (posts p LEFT JOIN comments c ON c.post_id = p.id) ON p.author_id = u.id LEFT JOIN users m ON m.id = u.id"),
            [Some("SELECT c.* FROM users u JOIN (posts p JOIN comments c ON c.post_id = p.id) ON p.author_id = u.id LEFT JOIN users m ON m.id = u.id".to_string())]
        );
        assert_eq!(present("SELECT to_jsonb(y) AS j FROM users x LEFT JOIN users y ON y.id > x.id"), [Some("SELECT y.* FROM users x JOIN users y ON y.id > x.id".to_string())]);
        // The preserved side, a join with an alias that hides the name, and no join at all: no read.
        assert_eq!(present("SELECT to_jsonb(u) AS j FROM users u LEFT JOIN posts p ON p.user_id = u.id"), [None]);
        assert_eq!(present("SELECT to_jsonb(j) AS j FROM users u LEFT JOIN (posts p JOIN comments c ON c.post_id = p.id) AS j ON true"), [Some("SELECT j.* FROM users u JOIN (posts p JOIN comments c ON c.post_id = p.id ) j ON true".to_string())]);
        assert_eq!(present("SELECT to_jsonb(u) AS j FROM users u"), [None]);
        assert_eq!(present("SELECT jsonb_build_object('id', p.id) AS j FROM users u LEFT JOIN posts p ON p.user_id = u.id"), [None]);
        // A derived table by its alias.
        assert_eq!(present("SELECT to_jsonb(d) AS j FROM users u LEFT JOIN (SELECT 1 AS n) d ON true"), [Some("SELECT d.* FROM users u JOIN (SELECT 1 AS n) d ON true".to_string())]);
    }

    #[test]
    fn a_path_an_item_could_not_type_goes_under_the_path_that_reads_it() {
        let registry = TypeRegistry::with_builtins();
        let p = shapes("SELECT jsonb_build_object('posts', ps.posts) AS author FROM ps");
        let item = Type::List { of: Box::new(Type::Object { fields: vec![field("tags", Type::Unknown, true)] }), nullable_items: false };
        let built = Built { ty: item, non_null: true, unresolved: vec!["[].tags".into(), "[]".into(), WHOLE.into()] };
        let outcome = Outcome { columns: vec![column("a0", oid::JSONB, "jsonb", false)], outer_join: false, present: None, built: vec![Some(built)] };
        let r = resolve(&p.columns[0].1, &[Some(outcome)], &registry, &[]);
        assert_eq!(r.unresolved, vec!["posts[].tags", "posts[]", "posts"]);
    }
}
