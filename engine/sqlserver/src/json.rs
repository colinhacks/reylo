//! The JSON a SQL Server statement builds, typed: a scalar subquery that ends in `for json`, and
//! SQL Server 2022's `json_object` and `json_array`. Each is read into a shape ([`Json`]) whose
//! values are typed by a probe: a statement that selects them over their own `FROM`, which the
//! server describes. What SQL Server writes there (its documentation of `FOR JSON` and of the
//! constructors):
//!
//! - `for json path` writes an array of one object per row, keyed by each column's name; a name
//!   with dots (`[author.name]`) nests its last part in objects named by the others. A column
//!   whose value is NULL is left out of its object unless `include_null_values`, which writes it
//!   as `null`. `without_array_wrapper` writes the object alone, which is JSON only where the
//!   subquery returns at most one row, and `root('r')` wraps the whole in `{"r": …}`. Over no
//!   rows the subquery is NULL, which `isnull(…, '[]')` turns into an empty array.
//! - Inside it, a column that is itself a `for json` subquery, a `json_object` or `json_array`, or
//!   `json_query(…)` of one, is written as the JSON it holds; any other text as a string.
//! - `for json auto` is read as `path` where its `FROM` is one table and no name has a dot; with
//!   joins it nests each joined table's rows, which is left untyped.
//! - `json_object('k': v, …)` writes a NULL value as `null`, unless `absent on null` leaves its
//!   key out; `json_array(…)` leaves a NULL out, unless `null on null` writes it.
//!
//! A value written as a number, a string or `true` and `false` is typed by the SQL type it was
//! written from, as on the other databases, and the run time reads the document as it is.

use crate::sqlserver::Described1;
use reylo_engine_core::types::{Field, Kind, Slot, TupleType, Type};
use sqlparser::ast::{Expr, ForClause, ForJson, FunctionArg, FunctionArgExpr, FunctionArgOperator, FunctionArgumentClause, FunctionArguments, GroupByExpr, JsonNullClause, Query, SelectItem, SetExpr, Statement, TableFactor, TableWithJoins, Value};

/// How many probes one statement may cost; past it its JSON is left untyped rather than turning
/// one describe into dozens.
const MAX_PROBES: usize = 8;

/// A JSON value a statement builds.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// An object of these keys; `nullable` where the whole may be SQL NULL. `dotted` for one a
    /// dotted name makes, which is left out where every key of it is.
    Object { keys: Vec<Key>, nullable: bool, dotted: bool },
    /// An array of these items.
    Array { items: Box<Json>, nullable: bool },
    /// `json_array(…)`: these values, in order, a NULL one left out where `skips_nulls`.
    Tuple { items: Vec<Json>, skips_nulls: bool },
    /// A SQL value written as JSON: column `index` of probe `probe`.
    Value { probe: usize, index: usize },
    /// JSON nothing types, as `json_query` of a text column.
    Any { nullable: bool },
    /// JSON written as a string inside a document, as a `without_array_wrapper` subquery is.
    Text { nullable: bool },
}

/// One key of an object.
#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    pub name: String,
    pub value: Json,
    /// A NULL value leaves the key out rather than writing `null`.
    pub absent_on_null: bool,
    /// The key may be absent whatever its value, as from the empty object `isnull(…, '{}')` gives.
    pub absent: bool,
}

/// A statement's JSON: per result column, its shape where it builds one, and the probes that
/// type their values.
#[derive(Debug, Default)]
pub struct Plan {
    pub columns: Vec<Option<Json>>,
    pub probes: Vec<String>,
}

/// One `SELECT`'s scope: its `FROM`, as written and by the names it gives tables, and the
/// expressions its JSON reads as values.
#[derive(Clone)]
struct Level {
    from: String,
    names: Vec<String>,
    group_by: String,
    /// The levels around this one, nearest last, whose names a correlated value may read.
    outer: Vec<(String, Vec<String>)>,
}

#[derive(Default)]
struct Builder {
    /// Per probe: its level and its value expressions.
    probes: Vec<(Level, Vec<Expr>)>,
    /// A value found no probe to go to, past [`MAX_PROBES`].
    too_many: bool,
}

/// Reads the JSON a statement's result columns build. A statement with a `*` among its columns is
/// left untyped, since its columns can't be matched to its items.
pub fn plan(statement: &Statement) -> Plan {
    let Statement::Query(q) = statement else { return Plan::default() };
    let SetExpr::Select(select) = q.body.as_ref() else { return Plan::default() };
    if select.projection.iter().any(|item| matches!(item, SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..))) {
        return Plan::default();
    }
    let mut b = Builder::default();
    let top = level(&select.from, &select.group_by, Vec::new());
    let mut top_probe = None;
    let columns: Vec<Option<Json>> = select.projection.iter().map(|item| item_expr(item).and_then(|e| b.shape(e, &top, &mut top_probe, true))).collect();
    if b.too_many || columns.iter().all(Option::is_none) {
        return Plan { columns: vec![None; columns.len()], probes: Vec::new() };
    }
    // A probe reads the statement's `with` queries as the statement does.
    let with = q.with.as_ref().map(|w| format!("{w} ")).unwrap_or_default();
    Plan { columns, probes: b.probes.iter().map(|(l, values)| format!("{with}{}", probe_sql(l, values))).collect() }
}

fn item_expr(item: &SelectItem) -> Option<&Expr> {
    match item {
        SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => Some(e),
        _ => None,
    }
}

/// A `SELECT`'s scope, from its `FROM` and `GROUP BY`.
fn level(from: &[TableWithJoins], group_by: &GroupByExpr, outer: Vec<(String, Vec<String>)>) -> Level {
    let mut names = Vec::new();
    for t in from {
        names.push(table_name(&t.relation));
        for j in &t.joins {
            names.push(table_name(&j.relation));
        }
    }
    let group_by = match group_by {
        GroupByExpr::Expressions(e, _) if !e.is_empty() => format!(" GROUP BY {}", e.iter().map(Expr::to_string).collect::<Vec<_>>().join(", ")),
        _ => String::new(),
    };
    Level { from: from.iter().map(TableWithJoins::to_string).collect::<Vec<_>>().join(", "), names, group_by, outer }
}

/// The name a `FROM` item gives its rows: its alias, else its table's last name.
fn table_name(t: &TableFactor) -> String {
    match t {
        TableFactor::Table { alias: Some(a), .. } | TableFactor::Derived { alias: Some(a), .. } | TableFactor::Function { alias: Some(a), .. } => a.name.value.to_ascii_lowercase(),
        TableFactor::Table { name, .. } => name.0.last().map(|p| p.to_string().trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase()).unwrap_or_default(),
        _ => String::new(),
    }
}

/// The probe of a level's values: them, over its `FROM` and grouped as it is, and over the `FROM`s
/// around it too where one of them reads a table only those name. The `WHERE`, which narrows
/// nothing the server describes and is where a subquery usually reads the rows around it, is
/// left out.
fn probe_sql(l: &Level, values: &[Expr]) -> String {
    let items = values.iter().enumerate().map(|(i, e)| format!("{e} AS [v{i}]")).collect::<Vec<_>>().join(", ");
    let reads = |e: &Expr| -> Vec<String> {
        let mut out = Vec::new();
        let _ = sqlparser::ast::visit_expressions(e, |x| {
            if let Expr::CompoundIdentifier(parts) = x {
                if parts.len() >= 2 {
                    out.push(parts[parts.len() - 2].value.to_ascii_lowercase());
                }
            }
            std::ops::ControlFlow::<()>::Continue(())
        });
        out
    };
    let outer: Vec<&String> = l.outer.iter().filter(|(_, names)| values.iter().flat_map(reads).any(|n| !l.names.contains(&n) && names.contains(&n))).map(|(from, _)| from).collect();
    let from: Vec<&str> = outer.into_iter().map(String::as_str).chain((!l.from.is_empty()).then_some(l.from.as_str())).collect();
    if from.is_empty() {
        format!("SELECT {items}")
    } else {
        format!("SELECT {items} FROM {}{}", from.join(", "), l.group_by)
    }
}

impl Builder {
    /// The value `e` adds to `level`'s probe, which this call makes where the level has none yet.
    fn value(&mut self, e: &Expr, level: &Level, probe: &mut Option<usize>) -> Json {
        let at = match *probe {
            Some(at) => at,
            None => {
                if self.probes.len() == MAX_PROBES {
                    self.too_many = true;
                    return Json::Any { nullable: true };
                }
                self.probes.push((level.clone(), Vec::new()));
                *probe = Some(self.probes.len() - 1);
                self.probes.len() - 1
            }
        };
        self.probes[at].1.push(e.clone());
        Json::Value { probe: at, index: self.probes[at].1.len() - 1 }
    }

    /// The JSON `e` builds, read at `level`; at the top, `None` where it builds none, or none a
    /// parse could read. Inside a document (`top` false), SQL Server writes as JSON a `for json`
    /// subquery, `json_query(…)`, `json_object`, `json_array` and `isnull` of one of them, which
    /// keeps its first argument's type, and every other expression as a value; a
    /// `without_array_wrapper` subquery's result is text there, written as a string.
    fn shape(&mut self, e: &Expr, level: &Level, probe: &mut Option<usize>, top: bool) -> Option<Json> {
        match e {
            Expr::Nested(inner) => self.shape(inner, level, probe, top),
            Expr::Subquery(q) if is_for_json(q) => {
                if !top && unwrapped(q) {
                    return Some(Json::Text { nullable: true });
                }
                match self.for_json(q, level) {
                    Some(s) => Some(s),
                    None if top => None,
                    None => Some(Json::Text { nullable: true }),
                }
            }
            Expr::Function(f) => {
                let name = f.name.to_string().to_ascii_lowercase();
                let args: Vec<&FunctionArg> = match &f.args {
                    FunctionArguments::List(list) => list.args.iter().collect(),
                    _ => Vec::new(),
                };
                let clauses = match &f.args {
                    FunctionArguments::List(list) => list.clauses.as_slice(),
                    _ => &[],
                };
                let arg = |a: &FunctionArg| match a {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(e)) => Some(e.clone()),
                    _ => None,
                };
                match name.as_str() {
                    "json_object" => {
                        let absent = clauses.iter().any(|c| matches!(c, FunctionArgumentClause::JsonNullClause(JsonNullClause::AbsentOnNull)));
                        let mut keys = Vec::new();
                        for a in &args {
                            let FunctionArg::ExprNamed { name, arg: FunctionArgExpr::Expr(v), operator: FunctionArgOperator::Colon } = a else { return if top { None } else { Some(Json::Any { nullable: false }) } };
                            let Some(key) = text(name) else { return if top { None } else { Some(Json::Any { nullable: false }) } };
                            let value = self.embedded(v, level, probe);
                            keys.push(Key { name: key, value, absent_on_null: absent, absent: false });
                        }
                        Some(Json::Object { keys, nullable: false, dotted: false })
                    }
                    "json_array" => {
                        let skips_nulls = !clauses.iter().any(|c| matches!(c, FunctionArgumentClause::JsonNullClause(JsonNullClause::NullOnNull)));
                        let items: Option<Vec<Json>> = args.iter().map(|a| arg(a).map(|v| self.embedded(&v, level, probe))).collect();
                        Some(Json::Tuple { items: items?, skips_nulls })
                    }
                    // `json_query(x)` is the document `x` holds; a path picks part of it, which may be
                    // missing or no object or array, so NULL.
                    "json_query" => {
                        let inner = args.first().and_then(|a| arg(a))?;
                        let whole = args.len() == 1 || args.get(1).and_then(|a| arg(a)).and_then(|p| text(&p)).is_some_and(|p| p.trim() == "$");
                        match self.shape(&inner, level, probe, true) {
                            Some(s) if whole => Some(s),
                            _ => Some(Json::Any { nullable: true }),
                        }
                    }
                    // `isnull(subquery, '[]')`: the empty document in place of the NULL over no
                    // rows. The empty object has none of the keys. Inside a document `coalesce`'s
                    // result is text, written as a value.
                    "isnull" | "coalesce" if args.len() == 2 && (top || name == "isnull") => {
                        let (first, empty) = (arg(args[0])?, arg(args[1])?);
                        // Inside a document a `without_array_wrapper` subquery stays text, and so
                        // does `isnull` of it.
                        let mut inner = &first;
                        while let Expr::Nested(e) = inner {
                            inner = e;
                        }
                        if !top && matches!(inner, Expr::Subquery(q) if is_for_json(q) && unwrapped(q)) {
                            return Some(Json::Text { nullable: text(&empty).is_none() });
                        }
                        let mut s = self.shape(&first, level, probe, true)?;
                        match (&mut s, text(&empty).as_deref()) {
                            (Json::Array { .. }, Some("[]")) => {}
                            (Json::Object { keys, .. }, Some("{}")) => keys.iter_mut().for_each(|k| k.absent = true),
                            _ => return None,
                        }
                        set_nullable(&mut s, false);
                        Some(s)
                    }
                    _ if top => None,
                    _ => Some(self.value(e, level, probe)),
                }
            }
            _ if top => None,
            _ => Some(self.value(e, level, probe)),
        }
    }

    /// A value inside a document: the JSON it holds, or the SQL value written as JSON.
    fn embedded(&mut self, e: &Expr, level: &Level, probe: &mut Option<usize>) -> Json {
        self.shape(e, level, probe, false).unwrap_or_else(|| self.value(e, level, probe))
    }

    /// A scalar subquery that ends in `for json`; `None` where what it writes may be no JSON
    /// document, as `without_array_wrapper` over several rows writes objects one after another.
    fn for_json(&mut self, q: &Query, around: &Level) -> Option<Json> {
        let Some(ForClause::Json { for_json, root, include_null_values, without_array_wrapper }) = &q.for_clause else { return Some(Json::Any { nullable: true }) };
        if *without_array_wrapper && !one_row(q) {
            return None;
        }
        let SetExpr::Select(select) = q.body.as_ref() else { return Some(Json::Any { nullable: true }) };
        let mut outer = around.outer.clone();
        outer.push((around.from.clone(), around.names.clone()));
        let level = level(&select.from, &select.group_by, outer);
        let names: Option<Vec<(Vec<String>, &Expr)>> = select
            .projection
            .iter()
            .map(|item| match item {
                SelectItem::ExprWithAlias { expr, alias } => Some((alias.value.split('.').map(String::from).collect(), expr)),
                SelectItem::UnnamedExpr(e @ Expr::Identifier(i)) => Some((vec![i.value.clone()], e)),
                SelectItem::UnnamedExpr(e @ Expr::CompoundIdentifier(parts)) => Some((vec![parts.last()?.value.clone()], e)),
                _ => None,
            })
            .collect();
        let Some(names) = names else { return Some(Json::Any { nullable: true }) };
        // AUTO nests each joined table's rows in an array of its own, and writes a dotted name as it is.
        if matches!(for_json, ForJson::Auto) && (select.from.len() != 1 || !select.from[0].joins.is_empty() || names.iter().any(|(path, _)| path.len() > 1)) {
            return Some(Json::Any { nullable: true });
        }
        let mut probe = None;
        let mut keys: Vec<Key> = Vec::new();
        for (path, e) in names {
            let value = self.embedded(e, &level, &mut probe);
            insert(&mut keys, &path, value, !*include_null_values);
        }
        let row = Json::Object { keys, nullable: false, dotted: false };
        let whole = if *without_array_wrapper { set_nullable_owned(row, true) } else { Json::Array { items: Box::new(row), nullable: true } };
        Some(match root {
            Some(r) => Json::Object { keys: vec![Key { name: r.clone(), value: whole, absent_on_null: false, absent: false }], nullable: true, dotted: false },
            None => whole,
        })
    }
}

/// Puts `value` at `path` among `keys`, a dotted name's parts nesting it in objects by those names.
fn insert(keys: &mut Vec<Key>, path: &[String], value: Json, absent_on_null: bool) {
    let Some((first, rest)) = path.split_first() else { return };
    if rest.is_empty() {
        keys.push(Key { name: first.clone(), value, absent_on_null, absent: false });
        return;
    }
    if !keys.iter().any(|k| &k.name == first && matches!(k.value, Json::Object { dotted: true, .. })) {
        keys.push(Key { name: first.clone(), value: Json::Object { keys: Vec::new(), nullable: false, dotted: true }, absent_on_null, absent: false });
    }
    if let Some(Key { value: Json::Object { keys: inner, .. }, .. }) = keys.iter_mut().rev().find(|k| &k.name == first) {
        insert(inner, rest, value, absent_on_null);
    }
}

fn is_for_json(q: &Query) -> bool {
    matches!(q.for_clause, Some(ForClause::Json { .. }))
}

/// Whether a `for json` subquery writes its rows without the array around them, which SQL Server
/// does not hold to be JSON.
fn unwrapped(q: &Query) -> bool {
    matches!(q.for_clause, Some(ForClause::Json { without_array_wrapper: true, .. }))
}

/// Whether a subquery returns at most one row on its face: `top 1`, an aggregate with no
/// `GROUP BY`, or no `FROM`.
fn one_row(q: &Query) -> bool {
    let SetExpr::Select(s) = q.body.as_ref() else { return false };
    crate::cardinality::at_most_one_row(q, s)
}

/// A string literal's text, as a key or an empty document is written.
fn text(e: &Expr) -> Option<String> {
    match e {
        Expr::Value(v) => match &v.value {
            Value::SingleQuotedString(s) | Value::NationalStringLiteral(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn set_nullable(s: &mut Json, to: bool) {
    match s {
        Json::Object { nullable, .. } | Json::Array { nullable, .. } | Json::Any { nullable } | Json::Text { nullable } => *nullable = to,
        _ => {}
    }
}

fn set_nullable_owned(mut s: Json, to: bool) -> Json {
    set_nullable(&mut s, to);
    s
}

impl Json {
    /// Whether the whole may be SQL NULL.
    pub fn nullable(&self) -> bool {
        match self {
            Json::Object { nullable, .. } | Json::Array { nullable, .. } | Json::Any { nullable } | Json::Text { nullable } => *nullable,
            Json::Tuple { .. } => false,
            Json::Value { .. } => true,
        }
    }
}

/// The type of a shape, and whether it may be NULL, its values typed by the probes' descriptions.
pub fn resolve(shape: &Json, probes: &[Option<Vec<Described1>>]) -> (Type, bool) {
    match shape {
        Json::Object { keys, nullable, .. } => {
            let fields = keys
                .iter()
                .map(|k| {
                    let (ty, null) = resolve(&k.value, probes);
                    // A key a NULL leaves out is never `null`.
                    Field { optional: vanishes(k, probes), ..Field::new(k.name.clone(), ty, null && !k.absent_on_null) }
                })
                .collect();
            (Type::structure(fields), *nullable)
        }
        Json::Array { items, nullable } => {
            let (ty, null) = resolve(items, probes);
            (Type::array(Slot::new(ty, null)), *nullable)
        }
        Json::Tuple { items, skips_nulls } => {
            let elements: Vec<(Type, bool)> = items.iter().map(|i| resolve(i, probes)).collect();
            // A NULL left out moves every value after it, so where one may be, the array is typed
            // by what any of its values may be.
            if *skips_nulls && elements.iter().any(|(_, null)| *null) {
                let types: Vec<Type> = elements.into_iter().map(|(ty, _)| ty).collect();
                let element = if types.len() == 1 { types.into_iter().next().unwrap() } else { Type::union(types) };
                return (Type::array(Slot::new(element, false)), false);
            }
            let slots = elements.into_iter().map(|(ty, null)| Slot::new(ty, null && !skips_nulls)).collect();
            (Kind::Tuple(TupleType { elements: slots, optional_elements: Vec::new(), rest: None }).into(), false)
        }
        Json::Value { probe, index } => match probes.get(*probe).and_then(Option::as_ref).and_then(|columns| columns.get(*index)) {
            Some(c) => (crate::types::of(&c.ty), c.nullable),
            None => (Type::unknown(), true),
        },
        Json::Any { nullable } => (Type::json(), *nullable),
        Json::Text { nullable } => (Type::text(), *nullable),
    }
}

/// Whether a key may be absent: one that may be whatever its value, one a NULL value leaves out,
/// and an object a dotted name makes whose every key may be, at any depth.
fn vanishes(k: &Key, probes: &[Option<Vec<Described1>>]) -> bool {
    k.absent || (k.absent_on_null && resolve(&k.value, probes).1) || matches!(&k.value, Json::Object { keys, dotted: true, .. } if !keys.is_empty() && keys.iter().all(|inner| vanishes(inner, probes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::MsSqlDialect;
    use sqlparser::parser::Parser;

    fn planned(sql: &str) -> Plan {
        plan(&Parser::parse_sql(&MsSqlDialect {}, sql).unwrap().pop().unwrap())
    }

    #[test]
    fn reads_for_json_path() {
        let p = planned("select u.id, (select p.id, p.title as [info.title], p.body from posts p where p.user_id = u.id order by p.id for json path) as posts from users u");
        assert_eq!(p.columns[0], None);
        let Some(Json::Array { items, nullable: true }) = &p.columns[1] else { panic!("{:?}", p.columns[1]) };
        let Json::Object { keys, .. } = items.as_ref() else { panic!() };
        assert_eq!(keys.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), ["id", "info", "body"]);
        assert!(keys.iter().all(|k| k.absent_on_null));
        let Json::Object { keys: info, .. } = &keys[1].value else { panic!() };
        assert_eq!(info[0].name, "title");
        // The subquery's values, over its own FROM alone: its WHERE reads the users around it.
        assert_eq!(p.probes, ["SELECT p.id AS [v0], p.title AS [v1], p.body AS [v2] FROM posts p"]);
    }

    #[test]
    fn reads_the_options_and_the_constructors() {
        let p = planned("select isnull((select id from t for json path, include_null_values), '[]') as a, (select top 1 id from t for json path, without_array_wrapper) as o, (select id from t for json path, root('rows')) as r, json_object('id': 1, 'n': @p_1 absent on null) as j, json_array(1, 2) as x, (select id from t for json path, without_array_wrapper) as many from t");
        let Some(Json::Array { items, nullable: false }) = &p.columns[0] else { panic!("{:?}", p.columns[0]) };
        let Json::Object { keys, .. } = items.as_ref() else { panic!() };
        assert!(!keys[0].absent_on_null);
        assert!(matches!(&p.columns[1], Some(Json::Object { nullable: true, .. })));
        assert!(matches!(&p.columns[2], Some(Json::Object { keys, nullable: true, .. }) if keys[0].name == "rows"));
        assert!(matches!(&p.columns[3], Some(Json::Object { keys, nullable: false, .. }) if keys.len() == 2 && keys[1].absent_on_null));
        assert!(matches!(&p.columns[4], Some(Json::Tuple { skips_nulls: true, .. })));
        // Several objects with no array around them are no JSON document: the column stays text.
        assert_eq!(p.columns[5], None);
        // A `*` leaves every column untyped.
        assert!(planned("select *, (select id from t for json path) as j from t").columns.is_empty());
    }

    #[test]
    fn reads_what_sql_server_writes_inside_a_document() {
        let p = planned("select (select u.id, (select top 1 p.title from posts p for json path, without_array_wrapper) as first, json_query((select top 1 p.title from posts p for json path, without_array_wrapper)) as promoted, isnull((select p.id from posts p for json path), '[]') as wrapped, isnull((select top 1 p.id from posts p for json path, without_array_wrapper), '{}') as unwrapped from users u for json path) as doc, json_query(json_object('child': json_object('id': 1)), '$.child') as child, json_query(json_object('id': 1), '$') as whole, isnull((select top 1 id from t for json path, without_array_wrapper), '{}') as fallback from t");
        let Some(Json::Array { items, .. }) = &p.columns[0] else { panic!("{:?}", p.columns[0]) };
        let Json::Object { keys, .. } = items.as_ref() else { panic!() };
        // Unwrapped, it is text; through json_query, the object; through isnull, the array.
        assert!(matches!(keys[1].value, Json::Text { nullable: true }));
        assert!(matches!(keys[2].value, Json::Object { nullable: true, .. }));
        assert!(matches!(keys[3].value, Json::Array { nullable: false, .. }));
        // isnull of an unwrapped subquery stays text there, never NULL.
        assert!(matches!(keys[4].value, Json::Text { nullable: false }));
        // A path picks part of the document, which may be missing.
        assert!(matches!(p.columns[1], Some(Json::Any { nullable: true })));
        assert!(matches!(p.columns[2], Some(Json::Object { nullable: false, .. })));
        // The empty object has none of the keys.
        assert!(matches!(&p.columns[3], Some(Json::Object { keys, nullable: false, .. }) if keys.iter().all(|k| k.absent)));
    }

    #[test]
    fn reads_with_queries_in_a_probe() {
        let p = planned("with recent as (select top 5 id, title from posts) select isnull((select r.id, r.title from recent r for json path), '[]') as recent_posts");
        assert_eq!(p.probes, ["WITH recent AS (SELECT TOP 5 id, title FROM posts) SELECT r.id AS [v0], r.title AS [v1] FROM recent r"]);
    }

    #[test]
    fn reads_a_correlated_value_over_the_rows_around_it() {
        let p = planned("select u.id, (select p.id, u.name as author from posts p where p.user_id = u.id for json path) as posts from users u");
        assert_eq!(p.probes, ["SELECT p.id AS [v0], u.name AS [v1] FROM users u, posts p"]);
    }
}
