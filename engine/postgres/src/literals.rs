//! The constants a statement's result columns hold: for each column, the literals every value of
//! it is one of, where the statement writes them, and nothing where it writes anything else
//! (spec/README.md rule 2, "a constant the statement writes"). A constant is a value the parser
//! reads in full ([`reylo_engine_core::literal::Literal`]): a string with no backslash in it,
//! which `standard_conforming_strings = off` would read otherwise, a number, `true` and `false`; a
//! cast of one to a built-in type without a modifier that keeps it as written; and a `case` whose
//! every result is one, each arm of a `union`, the left of an `intersect` or an `except`, and a
//! `values` list's rows. NULL adds no value. The type the server describes the column as narrows
//! to them, or keeps every value where one is no value it holds unchanged
//! ([`reylo_engine_core::literal::narrow`]).

use pg_query::protobuf::{a_const::Val, node::Node as N, Node, SelectStmt, SetOperation};
use reylo_engine_core::literal::Literal;

/// Each result column's literals, in order, where the statement's columns are its targets one
/// for one (no `*`); `None` where it is no statement that returns rows of its own targets.
pub fn columns(sql: &str) -> Option<Vec<Option<Vec<Literal>>>> {
    let parsed = crate::stack::parse(sql).ok()?;
    let [stmt] = parsed.protobuf.stmts.as_slice() else { return None };
    match stmt.stmt.as_deref()?.node.as_ref()? {
        N::SelectStmt(s) => select(s),
        N::InsertStmt(s) => targets(&crate::jsonshape::returning(&s.returning_clause)),
        N::UpdateStmt(s) => targets(&crate::jsonshape::returning(&s.returning_clause)),
        N::DeleteStmt(s) => targets(&crate::jsonshape::returning(&s.returning_clause)),
        N::MergeStmt(s) => targets(&crate::jsonshape::returning(&s.returning_clause)),
        _ => None,
    }
}

fn select(s: &SelectStmt) -> Option<Vec<Option<Vec<Literal>>>> {
    if s.op != SetOperation::SetopNone as i32 {
        let left = select(s.larg.as_deref()?)?;
        let right = select(s.rarg.as_deref()?)?;
        if left.len() != right.len() {
            return None;
        }
        // An INTERSECT's and an EXCEPT's rows are its left arm's, as that arm wrote them.
        if s.op != SetOperation::SetopUnion as i32 {
            return Some(left);
        }
        return Some(left.into_iter().zip(right).map(|(l, r)| Some([l?, r?].concat())).collect());
    }
    if !s.values_lists.is_empty() {
        let mut columns: Option<Vec<Option<Vec<Literal>>>> = None;
        for row in &s.values_lists {
            let Some(N::List(row)) = row.node.as_ref() else { return None };
            let values: Vec<Option<Vec<Literal>>> = row.items.iter().map(expr).collect();
            columns = Some(match columns {
                None => values,
                Some(known) if known.len() == values.len() => known.into_iter().zip(values).map(|(k, v)| Some([k?, v?].concat())).collect(),
                Some(_) => return None,
            });
        }
        return columns;
    }
    targets(&s.target_list)
}

fn targets(list: &[Node]) -> Option<Vec<Option<Vec<Literal>>>> {
    let mut out = Vec::with_capacity(list.len());
    for target in list {
        let Some(N::ResTarget(t)) = target.node.as_ref() else { return None };
        let value = t.val.as_deref()?;
        // A `*` stands for as many columns as its relation has.
        let star = |fields: &[Node]| fields.iter().any(|f| matches!(f.node, Some(N::AStar(_))));
        match value.node.as_ref() {
            Some(N::ColumnRef(c)) if star(&c.fields) => return None,
            Some(N::AIndirection(i)) if star(&i.indirection) => return None,
            _ => out.push(expr(value)),
        }
    }
    Some(out)
}

/// The values an expression is one of, where it writes them all; NULL is none.
fn expr(node: &Node) -> Option<Vec<Literal>> {
    match node.node.as_ref()? {
        N::AConst(c) if c.isnull => Some(Vec::new()),
        N::AConst(c) => Some(vec![match c.val.as_ref()? {
            Val::Ival(i) => Literal::Number(i.ival.to_string()),
            Val::Fval(f) => Literal::Number(f.fval.clone()),
            Val::Boolval(b) => Literal::Boolean(b.boolval),
            Val::Sval(s) if !s.sval.contains('\\') => Literal::Text(s.sval.clone()),
            _ => return None,
        }]),
        N::TypeCast(cast) => {
            let name = cast.type_name.as_ref()?;
            if !name.typmods.is_empty() || !name.array_bounds.is_empty() || name.setof || name.pct_type {
                return None;
            }
            let inner = expr(cast.arg.as_deref()?)?;
            let target = crate::infer::builtin_name(&name.names)?;
            inner.into_iter().map(|l| cast_to(&target, l)).collect()
        }
        N::CollateClause(c) => expr(c.arg.as_deref()?),
        N::CaseExpr(c) => {
            let mut values = Vec::new();
            for arm in &c.args {
                let Some(N::CaseWhen(w)) = arm.node.as_ref() else { return None };
                values.extend(expr(w.result.as_deref()?)?);
            }
            if let Some(otherwise) = c.defresult.as_deref() {
                values.extend(expr(otherwise)?);
            }
            Some(values)
        }
        _ => None,
    }
}

/// A constant cast to a built-in type, where the cast keeps the value as written: a string to a
/// string, a whole number written in digits to an integer, and a number to `numeric` or a double.
/// A cast to `real` rounds to fewer digits than a client may read it with, one to an integer
/// rounds a fraction, and one to a string of a number writes the server's text of it.
fn cast_to(target: &str, l: Literal) -> Option<Literal> {
    let plain = |s: &str, fraction: bool| {
        let digits = s.strip_prefix('-').unwrap_or(s);
        let (whole, rest) = digits.split_once('.').map_or((digits, None), |(w, f)| (w, Some(f)));
        !whole.is_empty() && whole.bytes().all(|b| b.is_ascii_digit()) && rest.is_none_or(|f| fraction && !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
    };
    match (target, l) {
        ("text" | "varchar", l @ Literal::Text(_)) => Some(l),
        ("int2" | "int4" | "int8", Literal::Number(n)) if plain(&n, false) => Some(Literal::Number(n)),
        ("int2" | "int4" | "int8", Literal::Text(s)) if plain(&s, false) => Some(Literal::Number(s)),
        ("numeric" | "float8", l @ Literal::Number(_)) => Some(l),
        ("numeric" | "float8", Literal::Text(s)) if plain(&s, true) => Some(Literal::Number(s)),
        ("bool", l @ Literal::Boolean(_)) => Some(l),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(sql: &str) -> Vec<Option<Vec<String>>> {
        columns(sql).expect("columns").into_iter().map(|c| c.map(|ls| ls.into_iter().map(|l| match l {
            Literal::Text(s) => format!("'{s}'"),
            Literal::Number(n) => n,
            Literal::Boolean(b) => b.to_string(),
        }).collect())).collect()
    }

    fn v(values: &[&str]) -> Option<Vec<String>> {
        Some(values.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn each_constant_a_statement_writes() {
        assert_eq!(shown("select 'user' as kind, 1 as n, -2.50 as d, true as t, null as z, id from users"), [v(&["'user'"]), v(&["1"]), v(&["-2.50"]), v(&["true"]), v(&[]), None]);
        // Each arm of a union, the left of an intersect or an except, and each row of a values list.
        assert_eq!(shown("select 'user' as kind, id from users union all select 'org', id from orgs"), [v(&["'user'", "'org'"]), None]);
        assert_eq!(shown("select 'a' as k intersect select 'b' except select 'c'"), [v(&["'a'"])]);
        assert_eq!(shown("select 'a' as k union select k from t"), [None]);
        assert_eq!(shown("values (1, 'a'), (2, null)"), [v(&["1", "2"]), v(&["'a'"])]);
        // A case whose every result is one, and a collation, which keeps the value.
        assert_eq!(shown("select case when x then 'a' when y then 'b' end as k, case when x then 'a' else name end as m from t"), [v(&["'a'", "'b'"]), None]);
        assert_eq!(shown("select 'a' collate \"C\" as k"), [v(&["'a'"])]);
        // RETURNING lists.
        assert_eq!(shown("insert into t (id) values (1) returning id, 'created' as status"), [None, v(&["'created'"])]);
        // A `*` stands for columns the targets do not list.
        assert_eq!(columns("select 'a' as k, * from t"), None);
        assert_eq!(columns("select 'a' as k, (r).* from t"), None);
    }

    #[test]
    fn a_cast_keeps_a_constant_only_as_written() {
        assert_eq!(shown("select 1::int8 as a, '7'::integer as b, 1.5::integer as c, '1.50'::numeric as d, 0.1::double precision as e, 0.1::real as f, 1::text as g"), [v(&["1"]), v(&["7"]), None, v(&["1.50"]), v(&["0.1"]), None, None]);
        // A modifier rounds or cuts it, and a type of the user's or an extension's reads it as its own.
        assert_eq!(shown("select 'abc'::varchar(2) as a, 1.55::numeric(3,1) as b, 'x'::mood as c, 'a'::varchar as d, 'x'::public.text as e"), [None, None, None, v(&["'a'"]), None]);
        // A string with a backslash, which the server's settings may read otherwise, and a bit string.
        assert_eq!(shown("select 'a\\b' as a, E'a\\nb' as b, B'101' as c, 'it''s' as d"), [None, v(&["'a\nb'"]), None, v(&["'it's'"])]);
    }
}
