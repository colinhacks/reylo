//! The constants a query's result columns hold: for each column, the literals every value of it is
//! one of, where the query writes them, and nothing where it writes anything else (spec/README.md
//! rule 2, "a constant the statement writes"). A constant is a value the session reads in full, as
//! the tree is read under its `sql_mode` ([`crate::mysql::analyze`] reads none otherwise): a quoted
//! string, a number, `TRUE` and `FALSE`, which MySQL holds as 1 and 0, and a number's negation; and
//! a `CASE` whose every result is one, through parentheses and a `COLLATE`. Each arm of a `UNION`
//! adds its values, the left of an `INTERSECT` or an `EXCEPT` gives its own, and so does each row
//! of a `VALUES` list. NULL adds no value. A string with an introducer (`_latin1'a'`) or of the
//! national set (`N'a'`) is the set's, and adds none. The type the server describes the column as
//! narrows to them, or keeps every value where one is no value it holds unchanged
//! ([`reylo_engine_core::literal::narrow`]).

use reylo_engine_core::literal::Literal;
use sqlparser::ast::{Expr, Query, SelectItem, SetExpr, SetOperator, Statement, UnaryOperator, Value};

/// Each result column's literals, in order, where the query's columns are its select list one for
/// one (no `*`); `None` where it is no query.
pub fn columns(statement: &Statement) -> Option<Vec<Option<Vec<Literal>>>> {
    let Statement::Query(q) = statement else { return None };
    query(q)
}

fn query(q: &Query) -> Option<Vec<Option<Vec<Literal>>>> {
    body(&q.body)
}

fn body(set: &SetExpr) -> Option<Vec<Option<Vec<Literal>>>> {
    match set {
        SetExpr::Select(s) => {
            let mut out = Vec::with_capacity(s.projection.len());
            for item in &s.projection {
                match item {
                    SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => out.push(expr(e)),
                    // A `*` stands for as many columns as its relation has.
                    _ => return None,
                }
            }
            Some(out)
        }
        SetExpr::Query(q) => query(q),
        SetExpr::SetOperation { left, op, right, .. } => {
            let (left, right) = (body(left)?, body(right)?);
            if left.len() != right.len() {
                return None;
            }
            // An INTERSECT's and an EXCEPT's rows are its left arm's, as that arm wrote them.
            if *op != SetOperator::Union {
                return Some(left);
            }
            Some(left.into_iter().zip(right).map(|(l, r)| Some([l?, r?].concat())).collect())
        }
        SetExpr::Values(values) => {
            let mut columns: Option<Vec<Option<Vec<Literal>>>> = None;
            for row in &values.rows {
                let row: Vec<Option<Vec<Literal>>> = row.content.iter().map(expr).collect();
                columns = Some(match columns {
                    None => row,
                    Some(known) if known.len() == row.len() => known.into_iter().zip(row).map(|(k, v)| Some([k?, v?].concat())).collect(),
                    Some(_) => return None,
                });
            }
            columns
        }
        _ => None,
    }
}

/// The values an expression is one of, where it writes them all; NULL is none.
fn expr(e: &Expr) -> Option<Vec<Literal>> {
    match e {
        Expr::Value(v) => Some(match &v.value {
            Value::Null => Vec::new(),
            Value::Number(n, false) => vec![Literal::Number(n.clone())],
            Value::SingleQuotedString(s) | Value::DoubleQuotedString(s) => vec![Literal::Text(s.clone())],
            Value::Boolean(b) => vec![Literal::Boolean(*b)],
            _ => return None,
        }),
        Expr::UnaryOp { op: UnaryOperator::Minus, expr: inner } => match expr(inner)?.as_slice() {
            [Literal::Number(n)] => Some(vec![Literal::Number(match n.strip_prefix('-') {
                Some(positive) => positive.to_string(),
                None => format!("-{n}"),
            })]),
            _ => None,
        },
        Expr::UnaryOp { op: UnaryOperator::Plus, expr: inner } => match expr(inner)?.as_slice() {
            [Literal::Number(n)] => Some(vec![Literal::Number(n.clone())]),
            _ => None,
        },
        Expr::Nested(inner) | Expr::Collate { expr: inner, .. } => expr(inner),
        Expr::Case { conditions, else_result, .. } => {
            let mut values = Vec::new();
            for when in conditions {
                values.extend(expr(&when.result)?);
            }
            if let Some(otherwise) = else_result {
                values.extend(expr(otherwise)?);
            }
            Some(values)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::MySqlDialect;
    use sqlparser::parser::Parser;

    fn shown(sql: &str) -> Option<Vec<Option<Vec<String>>>> {
        let statement = Parser::parse_sql(&MySqlDialect {}, sql).unwrap().pop().unwrap();
        columns(&statement).map(|cs| {
            cs.into_iter()
                .map(|c| {
                    c.map(|ls| {
                        ls.into_iter()
                            .map(|l| match l {
                                Literal::Text(s) => format!("'{s}'"),
                                Literal::Number(n) => n,
                                Literal::Boolean(b) => b.to_string(),
                            })
                            .collect()
                    })
                })
                .collect()
        })
    }

    fn v(values: &[&str]) -> Option<Vec<String>> {
        Some(values.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn each_constant_a_query_writes() {
        assert_eq!(shown("select 'user' as kind, 1 as n, -2.50 as d, true as t, null as z, id from users"), Some(vec![v(&["'user'"]), v(&["1"]), v(&["-2.50"]), v(&["true"]), v(&[]), None]));
        assert_eq!(shown("select 'user' as kind, id from users union all select 'org', id from orgs"), Some(vec![v(&["'user'", "'org'"]), None]));
        assert_eq!(shown("(select 'a' as k) except select 'c'"), Some(vec![v(&["'a'"])]));
        assert_eq!(shown("values row(1, 'a'), row(2, null)"), Some(vec![v(&["1", "2"]), v(&["'a'"])]));
        assert_eq!(shown("select case when x then 'a' when y then 'b' end as k, case x when 1 then 'a' else name end as m from t"), Some(vec![v(&["'a'", "'b'"]), None]));
        assert_eq!(shown("select ('a' collate utf8mb4_bin) as k, - -1 as n, +1 as p"), Some(vec![v(&["'a'"]), v(&["1"]), v(&["1"])]));
        // A string of a character set's own, and a hexadecimal number, are the set's and its bytes.
        assert_eq!(shown("select _latin1'a' as a, N'b' as b, X'61' as c, 0x61 as d"), Some(vec![None, None, None, None]));
        // A `*` stands for columns the select list does not list.
        assert_eq!(shown("select 'a' as k, t.* from t"), None);
        assert_eq!(shown("insert into t values (1)"), None);
    }
}
