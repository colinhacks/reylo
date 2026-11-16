//! The rows a RETURNING list reads.
//!
//! A data-modifying statement's RETURNING reads the target relation and whatever the statement
//! joined in, and since PostgreSQL 18 the row before the change and the row after it: `old` and
//! `new`, or the names `RETURNING WITH (OLD AS o, NEW AS n)` gives them. The server adds those
//! two as relations of their own (`transformReturningClause`): a name the clause gives hides the
//! default one and must be no relation's in scope, and a default name stands only where no
//! relation in scope goes by it, so in `UPDATE users AS new … RETURNING new.name` and in `INSERT
//! INTO old … RETURNING old.name` the name is the table's.
//!
//! Either row holds the target relation's columns, so every rule reads it as that relation's row
//! ([`Rows::read`] rewrites the reference), which a probe over the statement's relations can
//! describe. A row the statement may not have is NULL in every column whatever the table's `NOT
//! NULL` says: an INSERT has no old row (nor has the insert of an upsert or of a MERGE), and a
//! DELETE has no new one (nor has a MERGE that deletes). A MERGE that acts on target rows the
//! source does not match (`WHEN NOT MATCHED BY SOURCE`) returns rows with no source row at all.

use crate::levels::{Columns, Level, Withs};
use pg_query::protobuf::{self, node::Node as NodeEnum, CmdType, ColumnRef, MergeMatchKind, Node, ReturningOptionKind};
use serde_json::Value;

/// One of the two rows, by the name RETURNING reads it by.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    name: String,
    /// Whether a row the statement returns may have no such row.
    missing: bool,
    /// Whether the name is this row's for certain. A default name is some relation's instead when
    /// a FROM item goes by it, and one whose name the parse does not show may.
    certain: bool,
}

/// The rows a data-modifying statement's RETURNING can name beside its relations.
#[derive(Debug, Clone, PartialEq)]
pub struct Rows {
    /// The name the target relation's columns are qualified by: its alias, or its own name.
    target: String,
    /// Whether the statement joins nothing in, so the target is the one relation in scope.
    alone: bool,
    old: Option<Row>,
    new: Option<Row>,
    /// Whether a row the statement returns may have no source row (a MERGE's `WHEN NOT MATCHED BY
    /// SOURCE`): every column not read from the target is then NULL in it.
    source_missing: bool,
}

impl Rows {
    /// The rows of an INSERT, an UPDATE, a DELETE or a MERGE; nothing for any other statement.
    pub fn of(top: &NodeEnum) -> Option<Rows> {
        let (relation, joined, clause, old_missing, new_missing, source_missing): (_, Vec<Node>, _, _, _, _) = match top {
            NodeEnum::InsertStmt(s) => (s.relation.as_ref()?, Vec::new(), s.returning_clause.as_ref(), true, false, false),
            NodeEnum::UpdateStmt(s) => (s.relation.as_ref()?, s.from_clause.clone(), s.returning_clause.as_ref(), false, false, false),
            NodeEnum::DeleteStmt(s) => (s.relation.as_ref()?, s.using_clause.clone(), s.returning_clause.as_ref(), false, true, false),
            NodeEnum::MergeStmt(s) => {
                let clauses = || s.merge_when_clauses.iter().filter_map(|w| match w.node.as_ref() {
                    Some(NodeEnum::MergeWhenClause(c)) => Some(c),
                    _ => None,
                });
                let does = |command: CmdType| clauses().any(|c| c.command_type == command as i32);
                let by_source = clauses().any(|c| c.match_kind == MergeMatchKind::MergeWhenNotMatchedBySource as i32 && c.command_type != CmdType::CmdNothing as i32);
                (s.relation.as_ref()?, s.source_relation.as_deref().cloned().into_iter().collect(), s.returning_clause.as_ref(), does(CmdType::CmdInsert), does(CmdType::CmdDelete), by_source)
            }
            _ => return None,
        };
        let target = relation.alias.as_ref().map_or_else(|| relation.relname.clone(), |a| a.aliasname.clone());
        // The relations in scope, by name alone: the target and what the statement joined in.
        let alone = joined.is_empty();
        let mut scope = vec![Node { node: Some(NodeEnum::RangeVar(relation.clone())) }];
        scope.extend(joined);
        let scope = Level::of(&scope, &Withs::default(), &Columns::default());
        let given = |kind: ReturningOptionKind| {
            clause.iter().flat_map(|c| &c.options).find_map(|o| match o.node.as_ref() {
                Some(NodeEnum::ReturningOption(o)) if o.option == kind as i32 => Some(o.value.clone()),
                _ => None,
            })
        };
        let row = |kind: ReturningOptionKind, default: &str, missing: bool| match given(kind) {
            // The server refuses a name a relation in scope has ("table name specified more than once").
            Some(name) => Some(Row { name, missing, certain: true }),
            None => match scope.names(default) {
                Some(true) => None,
                named => Some(Row { name: default.to_string(), missing, certain: named == Some(false) }),
            },
        };
        Some(Rows { target, alone, old: row(ReturningOptionKind::ReturningOptionOld, "old", old_missing), new: row(ReturningOptionKind::ReturningOptionNew, "new", new_missing), source_missing })
    }

    /// The target relation and what the statement joined in, as the FROM a probe of the RETURNING
    /// list selects over. A source that a returned row may lack sits on the nullable side of an
    /// outer join, where every rule already reads its columns as nullable.
    pub fn from(&self, relation: &protobuf::RangeVar, joined: &[Node]) -> Vec<Node> {
        let target = Node { node: Some(NodeEnum::RangeVar(relation.clone())) };
        match joined {
            [source] if self.source_missing => {
                let yes = protobuf::AConst { isnull: false, location: -1, val: Some(protobuf::a_const::Val::Boolval(protobuf::Boolean { boolval: true })) };
                let join = protobuf::JoinExpr {
                    jointype: protobuf::JoinType::JoinLeft as i32,
                    larg: Some(Box::new(target)),
                    rarg: Some(Box::new(source.clone())),
                    quals: Some(Box::new(Node { node: Some(NodeEnum::AConst(yes)) })),
                    ..Default::default()
                };
                vec![Node { node: Some(NodeEnum::JoinExpr(Box::new(join))) }]
            }
            _ => std::iter::once(target).chain(joined.iter().cloned()).collect(),
        }
    }

    /// Whether every row the statement returns has both rows and a source row, so no reference
    /// reads a row that is missing.
    pub fn all_present(&self) -> bool {
        !self.source_missing && [&self.old, &self.new].into_iter().flatten().all(|r| !r.missing)
    }

    fn named(&self, name: &str) -> Option<&Row> {
        [&self.old, &self.new].into_iter().flatten().find(|r| r.name == name)
    }

    /// Whether `name`, the first field of a column reference, is for certain the new row (`true`)
    /// or the old one (`false`); `None` where it is neither for certain.
    pub fn row(&self, name: &str) -> Option<bool> {
        match (&self.old, &self.new) {
            (Some(old), _) if old.name == name && old.certain => Some(false),
            (_, Some(new)) if new.name == name && new.certain => Some(true),
            _ => None,
        }
    }

    /// A column reference that reads the old or the new row for certain, as the same reference to
    /// the target relation, and whether the row may be missing: `o.name` and `o.*` by the row's
    /// name, and the bare name as the whole row where `no_column` says no relation in scope has
    /// a column of that name, which a bare name is first.
    pub fn read(&self, c: &ColumnRef, no_column: &dyn Fn(&str) -> bool) -> Option<(ColumnRef, bool)> {
        let Some(NodeEnum::String(first)) = c.fields.first()?.node.as_ref() else { return None };
        let row = self.named(&first.sval).filter(|r| r.certain)?;
        if c.fields.len() > 2 || (c.fields.len() == 1 && !no_column(&row.name)) {
            return None;
        }
        let mut fields = c.fields.clone();
        fields[0] = Node { node: Some(NodeEnum::String(protobuf::String { sval: self.target.clone() })) };
        Some((ColumnRef { fields, location: c.location }, row.missing))
    }

    /// Whether `qualifier.*`, or `*` alone, is the target relation's columns and no others, for
    /// certain: by the target's name, by a row's, or with nothing joined in.
    pub fn star_of_target(&self, qualifier: Option<&str>) -> bool {
        match qualifier {
            Some(name) => name == self.target || self.named(name).is_some_and(|r| r.certain),
            None => self.alone,
        }
    }

    /// The target relation's name where the bare name `name` is the old or the new row for
    /// certain (`to_jsonb(o)`), and whether the row may be missing. The caller knows that no
    /// relation in scope has a column of that name.
    pub fn whole(&self, name: &str) -> Option<(&str, bool)> {
        self.named(name).filter(|r| r.certain).map(|r| (self.target.as_str(), r.missing))
    }

    /// `expr` with every reference [`Rows::read`] rewrites outside its subqueries rewritten, and
    /// whether it reads a row that may be missing anywhere ([`Rows::reads_missing`]).
    pub fn rewrite(&self, expr: &Node, no_column: &dyn Fn(&str) -> bool) -> (Node, bool) {
        let mut out = expr.clone();
        self.map(&mut out, no_column);
        (out, self.reads_missing(expr, &|name| !no_column(name)))
    }

    fn map(&self, node: &mut Node, no_column: &dyn Fn(&str) -> bool) {
        let each = |nodes: &mut [Node]| nodes.iter_mut().for_each(|n| self.map(n, no_column));
        match node.node.as_mut() {
            Some(NodeEnum::ColumnRef(c)) => {
                if let Some((read, _)) = self.read(c, no_column) {
                    *c = read;
                }
            }
            Some(NodeEnum::AExpr(a)) => {
                a.lexpr.iter_mut().chain(a.rexpr.iter_mut()).for_each(|n| self.map(n, no_column));
            }
            Some(NodeEnum::BoolExpr(b)) => each(&mut b.args),
            Some(NodeEnum::FuncCall(f)) => {
                each(&mut f.args);
                each(&mut f.agg_order);
                f.agg_filter.iter_mut().for_each(|n| self.map(n, no_column));
            }
            Some(NodeEnum::TypeCast(t)) => t.arg.iter_mut().for_each(|n| self.map(n, no_column)),
            Some(NodeEnum::CollateClause(c)) => c.arg.iter_mut().for_each(|n| self.map(n, no_column)),
            Some(NodeEnum::NamedArgExpr(a)) => a.arg.iter_mut().for_each(|n| self.map(n, no_column)),
            Some(NodeEnum::CoalesceExpr(c)) => each(&mut c.args),
            Some(NodeEnum::MinMaxExpr(m)) => each(&mut m.args),
            Some(NodeEnum::CaseExpr(c)) => {
                c.arg.iter_mut().chain(c.defresult.iter_mut()).for_each(|n| self.map(n, no_column));
                each(&mut c.args);
            }
            Some(NodeEnum::CaseWhen(w)) => w.expr.iter_mut().chain(w.result.iter_mut()).for_each(|n| self.map(n, no_column)),
            Some(NodeEnum::NullTest(t)) => t.arg.iter_mut().for_each(|n| self.map(n, no_column)),
            Some(NodeEnum::BooleanTest(t)) => t.arg.iter_mut().for_each(|n| self.map(n, no_column)),
            Some(NodeEnum::AArrayExpr(a)) => each(&mut a.elements),
            Some(NodeEnum::RowExpr(r)) => each(&mut r.args),
            Some(NodeEnum::AIndirection(i)) => i.arg.iter_mut().for_each(|n| self.map(n, no_column)),
            Some(NodeEnum::List(l)) => each(&mut l.items),
            Some(NodeEnum::SortBy(s)) => s.node.iter_mut().for_each(|n| self.map(n, no_column)),
            // A subquery has names of its own, and anything else is left as it is written: a probe
            // that still reads `o.name` fails to describe, and proves nothing.
            _ => {}
        }
    }

    /// Whether an expression of the RETURNING list may read a row that a returned row lacks:
    /// the old or the new row by its name, qualifying a column or bare (`o`, `(o).name`), or for
    /// a MERGE that acts where the source matches nothing, any column but the target's.
    /// `is_column` says whether the target relation has a column of a name, which a bare name is
    /// before it is a row. The reading is the wide one: a name that may be a row's counts, and so
    /// does a reference inside a subquery whose FROM gives the name to something else.
    pub fn reads_missing(&self, expr: &Node, is_column: &dyn Fn(&str) -> bool) -> bool {
        let Ok(tree) = serde_json::to_value(expr) else { return true };
        self.reads_missing_in(&tree, is_column)
    }

    fn reads_missing_in(&self, v: &Value, is_column: &dyn Fn(&str) -> bool) -> bool {
        match v {
            Value::Object(map) => map.iter().any(|(key, child)| {
                if key != "ColumnRef" {
                    return self.reads_missing_in(child, is_column);
                }
                let fields = child.get("fields").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
                let first = fields.first().and_then(|n| n.get("node")?.get("String")?.get("sval")?.as_str());
                // `*` alone reads every relation, the source among them.
                let Some(first) = first else { return self.source_missing };
                let bare = fields.len() == 1;
                if bare && is_column(first) {
                    return false;
                }
                match self.named(first) {
                    Some(row) => row.missing,
                    None => self.source_missing && (bare || first != self.target),
                }
            }),
            Value::Array(items) => items.iter().any(|i| self.reads_missing_in(i, is_column)),
            _ => false,
        }
    }

    /// The rows inside a subquery of the RETURNING list whose FROM is `from`: a FROM item there
    /// takes a name over, and one whose name the parse does not show may.
    pub fn under(&self, from: &[Node]) -> Rows {
        let level = Level::of(from, &Withs::default(), &Columns::default());
        let keep = |row: &Option<Row>| row.clone().filter(|r| level.names(&r.name) == Some(false));
        Rows { target: self.target.clone(), alone: self.alone, old: keep(&self.old), new: keep(&self.new), source_missing: self.source_missing }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top(sql: &str) -> NodeEnum {
        let parsed = crate::stack::parse(sql).expect("the statement parses");
        parsed.protobuf.stmts[0].stmt.as_ref().and_then(|s| s.node.clone()).expect("a statement")
    }

    fn rows(sql: &str) -> Rows {
        Rows::of(&top(sql)).expect("a data-modifying statement")
    }

    /// The RETURNING list's expressions, each rewritten and deparsed, with whether it reads a row
    /// that may be missing. `columns` are the target relation's.
    fn texts(sql: &str, columns: &[&str]) -> Vec<(String, bool)> {
        let top = top(sql);
        let rows = Rows::of(&top).expect("a data-modifying statement");
        let exprs = match &top {
            NodeEnum::InsertStmt(s) => crate::jsonshape::returning(&s.returning_clause),
            NodeEnum::UpdateStmt(s) => crate::jsonshape::returning(&s.returning_clause),
            NodeEnum::DeleteStmt(s) => crate::jsonshape::returning(&s.returning_clause),
            NodeEnum::MergeStmt(s) => crate::jsonshape::returning(&s.returning_clause),
            _ => unreachable!(),
        };
        let no_column = |name: &str| !columns.contains(&name);
        exprs
            .iter()
            .map(|target| {
                let Some(NodeEnum::ResTarget(rt)) = target.node.as_ref() else { panic!("a target") };
                let (expr, missing) = rows.rewrite(rt.val.as_deref().expect("a value"), &no_column);
                let text = crate::jsonshape::select_text(180000, &[expr], &[], None, None).expect("the expression deparses");
                (text.trim_start_matches("SELECT ").trim_end_matches(" AS a0").trim().to_string(), missing)
            })
            .collect()
    }

    fn want(expected: &[(&str, bool)]) -> Vec<(String, bool)> {
        expected.iter().map(|(text, missing)| (text.to_string(), *missing)).collect()
    }

    #[test]
    fn the_names_returning_gives_the_old_and_the_new_row() {
        let columns = ["id", "name"];
        // The names of the clause, and the default ones beside a name it gives only one row.
        let update = "UPDATE users SET name = $1 WHERE id = $2 RETURNING WITH (OLD AS o, NEW AS n) o.name, n.name, o.*, upper(n.name) || o.name";
        assert_eq!(texts(update, &columns), want(&[("users.name", false), ("users.name", false), ("users.*", false), ("upper(users.name) || users.name", false)]));
        assert!(rows(update).all_present());
        let one = "UPDATE users u SET name = $1 RETURNING WITH (OLD AS o) o.name, new.name, old.name";
        // `old` is hidden by the name the clause gives the row, and is left for the server to refuse.
        assert_eq!(texts(one, &columns), want(&[("u.name", false), ("u.name", false), ("old.name", false)]));
        // An INSERT has no old row and a DELETE no new one, by any name.
        let insert = "INSERT INTO users (id, name) VALUES ($1, $2) RETURNING WITH (OLD AS o, NEW AS n) o.name, n.name, name, coalesce(o.name, n.name), o.id IS NULL";
        assert_eq!(texts(insert, &columns), want(&[("users.name", true), ("users.name", false), ("name", false), ("COALESCE(users.name, users.name)", true), ("users.id IS NULL", true)]));
        assert!(!rows(insert).all_present());
        let delete = "DELETE FROM users WHERE id = $1 RETURNING old.name, new.name, new.*";
        assert_eq!(texts(delete, &columns), want(&[("users.name", false), ("users.name", true), ("users.*", true)]));
        // The upsert's insert has no old row, and a MERGE has what its actions leave it.
        let merge = "MERGE INTO users u USING src s ON u.id = s.id WHEN MATCHED THEN DELETE WHEN NOT MATCHED THEN INSERT (id) VALUES (s.id) RETURNING WITH (OLD AS o) o.name, new.name, u.name, s.id";
        assert_eq!(texts(merge, &columns), want(&[("u.name", true), ("u.name", true), ("u.name", false), ("s.id", false)]));
        let updating = "MERGE INTO users u USING src s ON u.id = s.id WHEN MATCHED THEN UPDATE SET name = s.name RETURNING old.name, new.name";
        assert_eq!(texts(updating, &columns), want(&[("u.name", false), ("u.name", false)]));
    }

    #[test]
    fn a_relation_or_a_column_of_the_name_is_read_first() {
        // A default name is a relation's where one in scope goes by it: the target by its name or
        // its alias, and an item of the FROM.
        assert_eq!(texts("INSERT INTO old (id, name) VALUES ($1, $2) RETURNING old.name, new.name", &["id", "name"]), want(&[("old.name", false), ("old.name", false)]));
        assert_eq!(texts("DELETE FROM users AS new WHERE id = $1 RETURNING new.name, old.name", &["id", "name"]), want(&[("new.name", false), ("new.name", false)]));
        assert_eq!(texts("UPDATE users SET name = old.name FROM other AS old WHERE old.id = users.id RETURNING old.name, new.name", &["id", "name"]), want(&[("old.name", false), ("users.name", false)]));
        // The name the clause gives the row leaves the table its own.
        assert_eq!(texts("INSERT INTO old (id, name) VALUES ($1, $2) RETURNING WITH (OLD AS before) before.name, old.name", &["id", "name"]), want(&[("old.name", true), ("old.name", false)]));
        // A FROM item whose name the parse does not show may go by a default name: the reference
        // is left as written, and still counts as one that may read a missing row.
        let unnamed = "DELETE FROM users USING (SELECT 1 AS id) WHERE users.id = 1 RETURNING new.name, old.name";
        assert_eq!(texts(unnamed, &["id", "name"]), want(&[("new.name", true), ("old.name", false)]));
        // A bare name is a column before it is a row.
        let column = "INSERT INTO t2 (id, n) VALUES ($1, $2) RETURNING WITH (OLD AS n) n, n.n, to_jsonb(n), (n).id";
        assert_eq!(texts(column, &["id", "n"]), want(&[("n", false), ("t2.n", true), ("to_jsonb(n)", false), ("(n).id", false)]));
        let row = "INSERT INTO t2 (id, n) VALUES ($1, $2) RETURNING WITH (OLD AS o) o, to_jsonb(o), (o).id, (new).id";
        assert_eq!(texts(row, &["id", "n"]), want(&[("t2", true), ("to_jsonb(t2)", true), ("(t2).id", true), ("(t2).id", false)]));
        // A schema qualifies a relation, never a row.
        assert_eq!(texts("DELETE FROM users WHERE id = $1 RETURNING new.users.name", &["id", "name"]), want(&[("new.users.name", true)]));
    }

    #[test]
    fn a_subquery_has_names_of_its_own() {
        let sql = "DELETE FROM users WHERE id = $1 RETURNING (SELECT new.name FROM other AS new), (SELECT new.name), new.id + (SELECT 1)";
        // A reference inside a subquery is left as written, and counts by its name alone.
        assert_eq!(texts(sql, &["id", "name"]), want(&[("(SELECT new.name FROM other new)", true), ("(SELECT new.name)", true), ("users.id + (SELECT 1)", true)]));
        let top = top(sql);
        let rows = Rows::of(&top).unwrap();
        let NodeEnum::DeleteStmt(delete) = &top else { unreachable!() };
        let inner = |i: usize| -> Vec<Node> {
            let Some(NodeEnum::ResTarget(rt)) = delete.returning_clause.as_ref().unwrap().exprs[i].node.as_ref() else { unreachable!() };
            let Some(NodeEnum::SubLink(s)) = rt.val.as_deref().and_then(|v| v.node.as_ref()) else { unreachable!() };
            crate::jsonshape::as_select(s.subselect.as_deref().unwrap()).unwrap().from_clause.clone()
        };
        assert_eq!(rows.under(&inner(0)).whole("new"), None, "the subquery's own `new`");
        assert_eq!(rows.under(&inner(0)).whole("old"), Some(("users", false)));
        assert_eq!(rows.under(&inner(1)).whole("new"), Some(("users", true)));
    }

    #[test]
    fn a_merge_that_acts_without_a_source_row_reads_no_source() {
        let sql = "MERGE INTO users u USING src s ON u.id = s.id WHEN MATCHED THEN UPDATE SET name = s.name WHEN NOT MATCHED BY SOURCE THEN DELETE RETURNING s.name, u.name, name, label, old.name, new.name, merge_action(), *";
        assert_eq!(
            texts(sql, &["id", "name"]),
            want(&[("s.name", true), ("u.name", false), ("name", false), ("label", true), ("u.name", false), ("u.name", true), ("merge_action()", false), ("*", true)])
        );
        // Doing nothing there returns no row, so every returned row has its source.
        let nothing = "MERGE INTO users u USING src s ON u.id = s.id WHEN MATCHED THEN UPDATE SET name = s.name WHEN NOT MATCHED BY SOURCE THEN DO NOTHING RETURNING s.name";
        assert_eq!(texts(nothing, &["id", "name"]), want(&[("s.name", false)]));
        // The probe's FROM puts such a source on the nullable side of a join.
        let from = |sql: &str| {
            let top = top(sql);
            let NodeEnum::MergeStmt(m) = &top else { unreachable!() };
            let from = Rows::of(&top).unwrap().from(m.relation.as_ref().unwrap(), std::slice::from_ref(m.source_relation.as_deref().unwrap()));
            let star = Node { node: Some(NodeEnum::ColumnRef(ColumnRef { fields: vec![Node { node: Some(NodeEnum::AStar(protobuf::AStar {})) }], location: -1 })) };
            crate::jsonshape::select_text(180000, &[star], &from, None, None).unwrap()
        };
        assert_eq!(from(sql), "SELECT * FROM users u LEFT JOIN src s ON true");
        assert_eq!(from(nothing), "SELECT * FROM users u, src s");
    }
}
