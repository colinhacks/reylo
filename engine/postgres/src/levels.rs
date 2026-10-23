//! Which query an aggregate call in a subquery belongs to.
//!
//! An aggregate belongs to the innermost query one of the columns it reads comes from, in its
//! arguments, its ORDER BY and its FILTER; one that reads no column is the query's it stands in.
//! So `count(u.id)` in a subquery over `posts` is the aggregate of the query around it that reads
//! `users u`: the subquery itself aggregates nothing, returns a row per row of `posts`, and as a
//! scalar subquery is NULL where `posts` holds no row. A rule that takes a subquery for one row
//! because its select list aggregates asks [`Level::owns`] first.
//!
//! A qualified column says where it comes from. A bare one (`count(name)`) comes from the
//! innermost query whose FROM has a column of that name, so the answer needs the columns of the
//! subquery's own FROM items: a table's from the catalog ([`Columns`]), a derived table's, a WITH
//! query's and a join's from the statement. A proof needs the name found there for certain, and
//! everything else is the weaker answer: a FROM item whose columns are not known (a function that
//! returns a row type, a relation the catalog did not report, a column the statement does not
//! name), a system column, a reference to a FROM item as a whole row, and a call that holds a
//! subquery of its own are all read as possibly the outer query's.
//!
//! The same columns answer the opposite question for `to_jsonb(u)`: a bare name is a column
//! before it is a FROM item, so `u` is the item's whole row only where no FROM in scope shows a
//! column `u`, for certain ([`Level::lacks`]).

use crate::jsonshape::{as_select, last_string};
use pg_query::protobuf::{self, node::Node as NodeEnum, Node, SelectStmt};
use serde_json::Value;
use std::collections::HashMap;

/// The columns of the relations a statement names, in order, by the name the statement writes
/// (`posts`, `app.posts`): the first two columns of [`crate::cardinality::COLUMNS_QUERY`]'s rows.
/// A relation that is missing is one whose columns are not known.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Columns(HashMap<String, Vec<String>>);

impl Columns {
    pub fn from_rows(rows: &[Vec<Value>]) -> Columns {
        let named = |row: &Vec<Value>| Some((crate::host::as_str(row.first()?)?.to_string(), crate::host::as_strings(row.get(1)?)?));
        Columns(rows.iter().filter_map(named).collect())
    }

    pub fn insert(&mut self, relation: impl Into<String>, columns: Vec<String>) {
        self.0.insert(relation.into(), columns);
    }

    fn of(&self, relation: &str) -> Option<&Vec<String>> {
        self.0.get(relation)
    }
}

impl<R: Into<String>, C: Into<String>> FromIterator<(R, Vec<C>)> for Columns {
    fn from_iter<I: IntoIterator<Item = (R, Vec<C>)>>(iter: I) -> Self {
        Columns(iter.into_iter().map(|(r, c)| (r.into(), c.into_iter().map(Into::into).collect())).collect())
    }
}

/// Whether a statement holds a scalar subquery, the one place a rule asks whose an aggregate is:
/// a statement without one needs no [`Columns`] for that.
pub fn needed(sql: &str) -> bool {
    crate::stack::kept(&NEEDED, sql, || crate::stack::run(|| needed_here(sql)))
}

thread_local! {
    static NEEDED: std::cell::RefCell<Vec<(String, bool)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn needed_here(sql: &str) -> bool {
    fn walk(v: &Value) -> bool {
        match v {
            Value::Object(map) => map.iter().any(|(key, child)| (key == "SubLink" && child.get("sub_link_type").and_then(Value::as_i64) == Some(protobuf::SubLinkType::ExprSublink as i64)) || walk(child)),
            Value::Array(items) => items.iter().any(walk),
            _ => false,
        }
    }
    // A subquery that aggregates is a second SELECT at the least.
    if sql.to_ascii_lowercase().matches("select").count() < 2 {
        return false;
    }
    crate::stack::parse(sql).ok().and_then(|parsed| serde_json::to_value(&parsed.protobuf).ok()).is_some_and(|tree| walk(&tree))
}

/// The columns a FROM item or a query shows. A name that is `None` is a column whose name is not
/// known, so a list that holds one may have a column of any name; `exact` says the list is the
/// item's columns in their order, which renaming the first of them by an alias's column list
/// needs.
#[derive(Debug, Clone, PartialEq)]
struct Shown {
    names: Vec<Option<String>>,
    exact: bool,
}

impl Shown {
    /// Nothing known: a column of any name.
    fn unknown() -> Shown {
        Shown { names: vec![None], exact: false }
    }

    fn has(&self, column: &str) -> bool {
        self.names.iter().any(|n| n.as_deref() == Some(column))
    }

    /// The columns under an alias's column list (`posts p (a, b)`), which renames the first ones.
    fn renamed(self, alias: Option<&protobuf::Alias>) -> Shown {
        let Some(alias) = alias.filter(|a| !a.colnames.is_empty()) else { return self };
        let given: Vec<Option<String>> = alias.colnames.iter().map(|n| last_string(std::slice::from_ref(n))).collect();
        if self.exact && given.len() <= self.names.len() {
            let rest = self.names[given.len()..].to_vec();
            Shown { names: given.into_iter().chain(rest).collect(), exact: true }
        } else {
            Shown { names: given.into_iter().chain([None]).collect(), exact: false }
        }
    }
}

/// The WITH queries in scope where a query stands, the innermost last, each with what it shows.
#[derive(Debug, Clone, Default)]
pub struct Withs(Vec<(String, Shown)>);

impl Withs {
    /// The scope inside a query that has this WITH clause.
    pub fn with(&self, clause: Option<&protobuf::WithClause>, columns: &Columns) -> Withs {
        let mut out = self.clone();
        let Some(clause) = clause else { return out };
        let ctes: Vec<&protobuf::CommonTableExpr> = clause.ctes.iter().filter_map(|c| match c.node.as_ref() {
            Some(NodeEnum::CommonTableExpr(c)) => Some(c.as_ref()),
            _ => None,
        }).collect();
        // A recursive WITH query may read itself and the ones after it, and none of those names
        // is a table's while its own columns are being read.
        let first = out.0.len();
        if clause.recursive {
            out.0.extend(ctes.iter().map(|c| (c.ctename.clone(), Shown::unknown())));
        }
        for (i, cte) in ctes.iter().enumerate() {
            // A write's RETURNING is not read, and SEARCH and CYCLE add columns of their own.
            let shown = match cte.ctequery.as_deref().and_then(as_select) {
                Some(select) if cte.search_clause.is_none() && cte.cycle_clause.is_none() => shown_by(select, &out, columns),
                _ => Shown::unknown(),
            };
            // `WITH p (a, b) AS (…)` renames the first columns.
            let alias = protobuf::Alias { aliasname: String::new(), colnames: cte.aliascolnames.clone() };
            let shown = shown.renamed(Some(&alias));
            if clause.recursive {
                out.0[first + i].1 = shown;
            } else {
                out.0.push((cte.ctename.clone(), shown));
            }
        }
        out
    }

    /// The scope under WITH queries of these names whose columns are not known.
    pub fn unknown(names: impl IntoIterator<Item = String>) -> Withs {
        Withs(names.into_iter().map(|name| (name, Shown::unknown())).collect())
    }

    fn get(&self, name: &str) -> Option<&Shown> {
        self.0.iter().rev().find(|(n, _)| n == name).map(|(_, shown)| shown)
    }
}

/// One FROM item a column reference can reach: the name that qualifies its columns, where it has
/// one, and the columns it shows.
#[derive(Debug, Clone)]
struct Item {
    name: Option<String>,
    shown: Shown,
}

/// The FROM of one query.
#[derive(Debug, Clone, Default)]
pub struct Level {
    items: Vec<Item>,
    /// A join with USING or NATURAL, which shows the merged columns first: `*` is then not the
    /// items' columns in their order.
    merged: bool,
}

impl Level {
    pub fn of(from: &[Node], withs: &Withs, columns: &Columns) -> Level {
        let mut level = Level::default();
        for item in from {
            level.read(item, withs, columns);
        }
        level
    }

    fn read(&mut self, item: &Node, withs: &Withs, columns: &Columns) {
        match item.node.as_ref() {
            Some(NodeEnum::RangeVar(rv)) => {
                let with = withs.get(&rv.relname).filter(|_| rv.schemaname.is_empty());
                let shown = match with {
                    Some(shown) => shown.clone(),
                    None => {
                        let name = if rv.schemaname.is_empty() { rv.relname.clone() } else { format!("{}.{}", rv.schemaname, rv.relname) };
                        match columns.of(&name) {
                            Some(names) => Shown { names: names.iter().cloned().map(Some).collect(), exact: true },
                            None => Shown::unknown(),
                        }
                    }
                };
                let name = rv.alias.as_ref().map_or_else(|| rv.relname.clone(), |a| a.aliasname.clone());
                self.items.push(Item { name: Some(name), shown: shown.renamed(rv.alias.as_ref()) });
            }
            Some(NodeEnum::RangeSubselect(r)) => {
                let shown = match r.subquery.as_deref().and_then(as_select) {
                    Some(select) => shown_by(select, withs, columns),
                    None => Shown::unknown(),
                };
                self.items.push(Item { name: r.alias.as_ref().map(|a| a.aliasname.clone()), shown: shown.renamed(r.alias.as_ref()) });
            }
            Some(NodeEnum::RangeFunction(r)) => {
                // `f(…) AS r (k int, v text)` is the columns it defines, and WITH ORDINALITY one
                // more. A column list without types names the first columns of a function that
                // may return more, and with no list the columns are the function's to say.
                let defined: Vec<Option<String>> = r.coldeflist.iter().map(|d| match d.node.as_ref() {
                    Some(NodeEnum::ColumnDef(d)) => Some(d.colname.clone()),
                    _ => None,
                }).collect();
                let shown = if !defined.is_empty() && !r.is_rowsfrom {
                    Shown { names: defined.into_iter().chain(r.ordinality.then(|| Some("ordinality".to_string()))).collect(), exact: true }
                } else {
                    Shown::unknown().renamed(r.alias.as_ref())
                };
                let name = r.alias.as_ref().map(|a| a.aliasname.clone()).or_else(|| crate::infer::function_item_name(r));
                self.items.push(Item { name, shown });
            }
            Some(NodeEnum::JoinExpr(j)) => {
                let mut inner = Level::default();
                for side in [j.larg.as_deref(), j.rarg.as_deref()].into_iter().flatten() {
                    inner.read(side, withs, columns);
                }
                inner.merged |= j.is_natural || !j.using_clause.is_empty();
                match &j.alias {
                    // An alias on the join hides the names of the items under it.
                    Some(alias) => self.items.push(Item { name: Some(alias.aliasname.clone()), shown: inner.all().renamed(Some(alias)) }),
                    None => {
                        // `USING (id) AS x` names the merged columns and hides nothing.
                        if let Some(alias) = &j.join_using_alias {
                            let names = j.using_clause.iter().map(|n| last_string(std::slice::from_ref(n))).collect();
                            self.items.push(Item { name: Some(alias.aliasname.clone()), shown: Shown { names, exact: false } });
                        }
                        self.merged |= inner.merged;
                        self.items.extend(inner.items);
                    }
                }
            }
            Some(NodeEnum::RangeTableSample(t)) => {
                if let Some(relation) = t.relation.as_deref() {
                    self.read(relation, withs, columns);
                }
            }
            // XMLTABLE, JSON_TABLE and whatever else a FROM takes: an item of unknown columns.
            _ => self.items.push(Item { name: None, shown: Shown::unknown() }),
        }
    }

    /// Whether no item of this FROM shows a column of this name, for certain: every item's
    /// columns are known by name and none is this one. A system column (`ctid`) is one every table
    /// has and no list holds.
    pub fn lacks(&self, column: &str) -> bool {
        const SYSTEM_COLUMNS: &[&str] = &["ctid", "xmin", "xmax", "cmin", "cmax", "tableoid", "oid", "rowid", "_rowid_"];
        !SYSTEM_COLUMNS.contains(&column) && self.items.iter().all(|i| i.shown.names.iter().all(|n| n.as_deref().is_some_and(|n| n != column)))
    }

    /// Whether an item of this FROM goes by this name: `None` where none of the names read is it
    /// and an item's name is not known.
    pub fn names(&self, name: &str) -> Option<bool> {
        if self.items.iter().any(|i| i.name.as_deref() == Some(name)) {
            return Some(true);
        }
        self.items.iter().all(|i| i.name.is_some()).then_some(false)
    }

    /// Every column of the FROM, as `*` shows them.
    fn all(&self) -> Shown {
        Shown { names: self.items.iter().flat_map(|i| i.shown.names.iter().cloned()).collect(), exact: !self.merged && self.items.iter().all(|i| i.shown.exact) }
    }

    /// Whether the query whose FROM this is, standing inside another, is the one an aggregate
    /// call in its select list belongs to, for certain: the call reads no column, or one of this
    /// FROM, a qualified one by its item's name and a bare one by an item that shows its name.
    pub fn owns(&self, call: &protobuf::FuncCall) -> bool {
        let mut reads = Vec::new();
        for node in call.args.iter().chain(&call.agg_order).chain(call.agg_filter.as_deref()) {
            if serde_json::to_value(node).ok().and_then(|tree| references(&tree, &mut reads)).is_none() {
                return false;
            }
        }
        reads.is_empty() || reads.iter().any(|fields| match fields.as_slice() {
            [Some(column)] => self.items.iter().any(|i| i.shown.has(column)),
            // `p.title` and `p.*`. A longer form (`app.posts.title`) names a relation by its
            // schema, which an alias here does not answer to.
            [Some(item), _] => self.items.iter().any(|i| i.name.as_deref() == Some(item.as_str())),
            _ => false,
        })
    }
}

/// The column references in an expression, each as its fields with `None` for a `*`; nothing when
/// the expression holds a subquery, whose references have a scope of their own.
fn references(v: &Value, out: &mut Vec<Vec<Option<String>>>) -> Option<()> {
    fn text(v: &Value) -> Option<String> {
        match v {
            Value::Object(map) => map.iter().find_map(|(key, child)| if key == "sval" { child.as_str().map(str::to_string) } else { text(child) }),
            _ => None,
        }
    }
    match v {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "SubLink" {
                    return None;
                }
                match child.get("fields").and_then(Value::as_array).filter(|_| key == "ColumnRef") {
                    Some(fields) => out.push(fields.iter().map(text).collect()),
                    None => references(child, out)?,
                }
            }
        }
        Value::Array(items) => items.iter().try_for_each(|i| references(i, out))?,
        _ => {}
    }
    Some(())
}

/// The columns a query shows, by the names the server gives them: a target's alias, a column
/// reference's own name, a function call's function name, and a `*` as the columns of the FROM.
/// Any other target is a column whose name is not read. A set operation shows its leftmost arm's
/// columns, and a VALUES list `column1`, `column2` and so on.
fn shown_by(select: &SelectStmt, withs: &Withs, columns: &Columns) -> Shown {
    let withs = withs.with(select.with_clause.as_ref(), columns);
    if let Some(left) = select.larg.as_deref() {
        return shown_by(left, &withs, columns);
    }
    if let Some(row) = select.values_lists.first() {
        let width = match row.node.as_ref() {
            Some(NodeEnum::List(l)) => l.items.len(),
            _ => return Shown::unknown(),
        };
        return Shown { names: (1..=width).map(|i| Some(format!("column{i}"))).collect(), exact: true };
    }
    let level = Level::of(&select.from_clause, &withs, columns);
    let mut shown = Shown { names: Vec::new(), exact: true };
    for target in &select.target_list {
        let Some(NodeEnum::ResTarget(rt)) = target.node.as_ref() else { return Shown::unknown() };
        if !rt.name.is_empty() {
            shown.names.push(Some(rt.name.clone()));
            continue;
        }
        match rt.val.as_deref().and_then(|v| v.node.as_ref()) {
            Some(NodeEnum::ColumnRef(c)) => match c.fields.split_last() {
                Some((last, before)) if matches!(last.node, Some(NodeEnum::AStar(_))) => {
                    let all = match before.last() {
                        None => level.all(),
                        Some(item) => {
                            let name = last_string(std::slice::from_ref(item));
                            match level.items.iter().find(|i| name.is_some() && before.len() == 1 && i.name == name) {
                                Some(item) => item.shown.clone(),
                                None => Shown::unknown(),
                            }
                        }
                    };
                    shown.exact &= all.exact;
                    shown.names.extend(all.names);
                }
                Some((last, _)) => shown.names.push(last_string(std::slice::from_ref(last))),
                None => shown.names.push(None),
            },
            Some(NodeEnum::FuncCall(f)) => shown.names.push(last_string(&f.funcname)),
            _ => shown.names.push(None),
        }
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether the scalar subquery that is the first column of `sql` owns the aggregate call that
    /// is its own first column.
    fn owns(sql: &str, columns: &Columns) -> bool {
        let parsed = crate::stack::parse(sql).unwrap();
        let Some(NodeEnum::SelectStmt(top)) = parsed.protobuf.stmts[0].stmt.as_deref().and_then(|n| n.node.as_ref()) else { panic!("a SELECT") };
        let target = |s: &SelectStmt| match s.target_list[0].node.as_ref() {
            Some(NodeEnum::ResTarget(rt)) => rt.val.as_deref().and_then(|v| v.node.clone()),
            _ => None,
        };
        let Some(NodeEnum::SubLink(link)) = target(top) else { panic!("a subquery") };
        let select = link.subselect.as_deref().and_then(as_select).unwrap();
        let Some(NodeEnum::FuncCall(call)) = target(select) else { panic!("a call") };
        let withs = Withs::default().with(top.with_clause.as_ref(), columns).with(select.with_clause.as_ref(), columns);
        Level::of(&select.from_clause, &withs, columns).owns(&call)
    }

    fn catalog() -> Columns {
        [("users", vec!["id", "name", "title"]), ("posts", vec!["id", "user_id", "title"]), ("comments", vec!["id", "post_id", "body"]), ("app.posts", vec!["id", "body"])].into_iter().collect()
    }

    #[test]
    fn a_bare_column_is_the_innermost_from_that_has_it() {
        let c = catalog();
        for sql in [
            "select (select count(title) from posts) from users",
            "select (select count(id) from posts where user_id = u.id) from users u",
            "select (select count(length(name) + length(title)) from posts) from users",
            "select (select count(*) from posts) from users",
            "select (select count(1) from posts) from users",
            "select (select count(p.id) from posts p) from users",
            "select (select count(p.*) from posts p) from users",
            "select (select count(body) from app.posts) from users",
            "select (select count(body) from posts join comments using (id)) from users",
            "select (select count(id) from posts join comments using (id)) from users",
            "select (select count(body) from (posts join comments using (id)) as j) from users",
            "select (select count(j.body) from (posts join comments using (id)) as j) from users",
            "select (select count(x.id) from posts join comments using (id) as x) from users",
            "select (select count(title) from (select id, title from posts) p) from users",
            "select (select count(t) from (select id, title as t from posts) p) from users",
            "select (select count(t) from (select id, title from posts) p (i, t)) from users",
            "select (select count(title) from (select * from posts) p) from users",
            "select (select count(title) from (select p.* from posts p) s) from users",
            "select (select count(count) from (select count(*) from posts) p) from users",
            "select (select count(column2) from (values (1, 2)) v) from users",
            "select (select count(title) from (select id, title from posts union all select id, body from comments) p) from users",
            "with p as (select id, title from posts) select (select count(title) from p) from users",
            "with p (i, t) as (select id, title from posts) select (select count(t) from p) from users",
            "with recursive p as (select id, title from posts union all select id, title from p) select (select count(title) from p) from users",
            "select (with p as (select id, title from posts) select count(title) from p) from users",
            "select (select count(x) from generate_series(1, 0) g (x)) from users",
            "select (select count(k) from json_to_record('{}') as r (k int, v text)) from users",
            "select (select count(ordinality) from json_to_record('{}') with ordinality as r (k int, v text)) from users",
            "select (select count(b) from posts p (a, b, c)) from users",
            "select (select count(title) from posts p (a, b)) from users",
            "select (select count(title) from posts tablesample system (50)) from users",
        ] {
            assert!(owns(sql, &c), "{sql}");
        }
    }

    #[test]
    fn a_column_of_no_inner_from_is_possibly_the_outer_querys() {
        let c = catalog();
        for sql in [
            "select (select count(name) from posts) from users",
            "select (select count(u.name) from posts) from users u",
            "select (select count(*) filter (where name <> '') from posts) from users",
            "select (select count(1 order by name) from posts) from users",
            "select (select count(title) from app.posts) from users",
            "select (select count(name) from posts join comments using (id)) from users",
            "select (select count(name) from (posts join comments using (id)) as j) from users",
            // The join's alias hides `posts`, so this one is the outer query's.
            "select (select count(posts.title) from (posts join comments using (id)) as j) from users posts",
            "select (select count(name) from (select id, title from posts) p) from users",
            "select (select count(name) from (select * from posts) p) from users",
            "select (select count(title) from (select id, title from posts) p (i, t)) from users",
            "with p as (select id, title from posts) select (select count(name) from p) from users",
            "select (with p as (select id, title from posts) select count(name) from p) from users",
            // The WITH query shadows the table of its name.
            "with posts as (select id from comments) select (select count(title) from posts) from users",
            "with recursive posts as (select id from comments union all select id from posts) select (select count(title) from posts) from users",
            "with recursive p as (select id, title from posts union all select id, title from p) cycle id set looped using path select (select count(body) from p) from users",
            "select (select count(title) from posts p (a, b, c)) from users",
            "select (select count(a) from generate_series(1, 0) g (x)) from users",
            "select (select count(name) from json_to_record('{}') as r (k int, v text)) from users",
            // A whole-row reference and a system column are read as possibly outer.
            "select (select count(posts) from posts) from users",
            "select (select count(ctid) from posts) from users",
            // So are the columns of an item that does not say them, and of a relation the catalog
            // did not report.
            "select (select count(g) from generate_series(1, 0) g) from users",
            "select (select count(title) from unlisted) from users",
            "select (select count(title) from (select length(title) from posts) p) from users",
            "select (select count(title) from xmltable('/a' passing '<a/>' columns title text) x) from users",
            // A schema-qualified reference names a relation, which an alias does not answer to.
            "select (select count(public.posts.title) from comments posts) from users",
            // A subquery inside the call has names of its own.
            "select (select count((select title from posts limit 1)) from posts) from users",
        ] {
            assert!(!owns(sql, &c), "{sql}");
        }
        assert!(!owns("select (select count(title) from posts) from users", &Columns::default()), "no catalog, no proof");
    }

    #[test]
    fn only_a_scalar_subquery_needs_the_columns() {
        assert!(needed("select (select count(name) from posts) from users"));
        assert!(needed("select id from users where 0 < (select count(*) from posts)"));
        assert!(!needed("select id from users where exists (select 1 from posts)"));
        assert!(!needed("select id from users where id in (select user_id from posts)"));
        assert!(!needed("select count(name) from users"));
        assert!(!needed("with p as (select 1) select * from p"));
    }
}
