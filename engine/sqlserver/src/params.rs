//! Which parameters the caller may pass null for, which SQL Server's deduction does not say: by
//! the rules of the other databases, a parameter the statement tests for NULL (`:p is null`) or
//! falls back from (`isnull(:p, …)`, `coalesce(:p, …)`), and one written whole to a column that
//! takes NULL, in an INSERT's VALUES or an UPDATE's SET.

use crate::api::Scope;
use crate::host::{as_str, Host, Lazy};
use reylo_engine_core::host::as_bool;
use serde_json::{json, Value};
use sqlparser::ast::{AssignmentTarget, Expr, FunctionArg, FunctionArgExpr, FunctionArguments, SetExpr, Statement, TableFactor, TableObject};
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

/// Each column of the table `@P1` names, and whether it takes NULL.
pub const TABLE_COLUMNS: &str = "select c.name, c.is_nullable from sys.columns c where c.object_id = object_id(@P1)";

/// The columns of the table a statement writes, each by name with whether it takes NULL.
#[derive(Debug, Clone, Default)]
pub struct Columns {
    pub nullable: HashMap<String, bool>,
}

fn written_table(statement: &Statement) -> Option<String> {
    match statement {
        Statement::Insert(i) => match &i.table {
            TableObject::TableName(name) => Some(name.to_string()),
            _ => None,
        },
        Statement::Update(u) => match &u.table.relation {
            TableFactor::Table { name, args: None, .. } if u.table.joins.is_empty() && u.from.is_none() => Some(name.to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// The columns of the table an INSERT or an UPDATE writes, asked where the statement is one.
pub fn columns(statement: &Statement, host: &Host) -> Lazy<Columns> {
    let Some(table) = written_table(statement) else { return Some(Columns::default()) };
    let rows = match host.query(TABLE_COLUMNS, vec![json!(table)], Scope::Statement)? {
        Ok(rows) => rows,
        Err(_) => return Some(Columns::default()),
    };
    let nullable = rows.iter().filter_map(|r| Some((r.first().and_then(as_str)?.to_string(), r.get(1).and_then(as_bool).or_else(|| r.get(1).and_then(Value::as_i64).map(|n| n != 0))?))).collect();
    Some(Columns { nullable })
}

/// The parameter a placeholder of the rewritten statement is, by the name the rewrite gave it
/// (`@p_2` is the second of `order`).
fn param<'a>(e: &Expr, order: &'a [String]) -> Option<&'a String> {
    match e {
        Expr::Nested(inner) => param(inner, order),
        Expr::Identifier(i) => i.value.strip_prefix("@p_").and_then(|n| n.parse::<usize>().ok()).and_then(|n| order.get(n.checked_sub(1)?)),
        _ => None,
    }
}

/// The parameters the caller may pass null for, by name.
pub fn nullable(statement: &Statement, order: &[String], columns: Option<&Columns>) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();
    let _ = sqlparser::ast::visit_expressions(statement, |e| {
        match e {
            Expr::IsNull(x) | Expr::IsNotNull(x) => out.extend(param(x, order).cloned()),
            Expr::Function(f) => {
                let name = f.name.0.last().map(|p| p.to_string().to_ascii_lowercase()).unwrap_or_default();
                if let ("isnull" | "coalesce", FunctionArguments::List(list)) = (name.as_str(), &f.args) {
                    // Every argument but the last falls back to the next.
                    let n = list.args.len();
                    for a in list.args.iter().take(n.saturating_sub(1)) {
                        if let FunctionArg::Unnamed(FunctionArgExpr::Expr(x)) = a {
                            out.extend(param(x, order).cloned());
                        }
                    }
                }
            }
            _ => {}
        }
        ControlFlow::<()>::Continue(())
    });
    let Some(columns) = columns else { return out };
    let takes_null = |column: &str| columns.nullable.iter().any(|(c, n)| *n && c == column);
    match statement {
        Statement::Insert(i) => {
            if let Some(SetExpr::Values(v)) = i.source.as_deref().map(|s| s.body.as_ref()) {
                for row in &v.rows {
                    for (column, value) in i.columns.iter().zip(&row.content) {
                        let column = column.0.last().map(|p| p.to_string().trim_matches(|c| c == '[' || c == ']').to_string()).unwrap_or_default();
                        if let Some(p) = param(value, order) {
                            if takes_null(&column) {
                                out.insert(p.clone());
                            }
                        }
                    }
                }
            }
        }
        Statement::Update(u) => {
            for a in &u.assignments {
                let AssignmentTarget::ColumnName(column) = &a.target else { continue };
                let column = column.0.last().map(|p| p.to_string().trim_matches(|c| c == '[' || c == ']').to_string()).unwrap_or_default();
                if let Some(p) = param(&a.value, order) {
                    if takes_null(&column) {
                        out.insert(p.clone());
                    }
                }
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::MsSqlDialect;
    use sqlparser::parser::Parser;

    fn parse(sql: &str) -> Statement {
        Parser::parse_sql(&MsSqlDialect {}, sql).unwrap().pop().unwrap()
    }

    #[test]
    fn a_parameter_tested_for_null_or_written_to_a_nullable_column() {
        let order: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
        let s = parse("select id from t where (@p_1 is null or x = 1) and y = isnull(@p_2, 0) and z = coalesce(@p_3, @p_4)");
        let mut found: Vec<String> = nullable(&s, &order, None).into_iter().collect();
        found.sort();
        assert_eq!(found, ["a", "b", "c"]);
        let columns = Columns { nullable: [("note".to_string(), true), ("name".to_string(), false)].into_iter().collect() };
        let s = parse("insert into t (name, note) values (@p_1, @p_2)");
        assert_eq!(nullable(&s, &order, Some(&columns)), ["b".to_string()].into_iter().collect());
        let s = parse("update t set note = @p_1, name = @p_2 where id = @p_3");
        assert_eq!(nullable(&s, &order, Some(&columns)), ["a".to_string()].into_iter().collect());
    }
}
