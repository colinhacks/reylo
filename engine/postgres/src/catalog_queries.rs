//! Every catalog query the engine names to its host, read by the parser, against the rules that
//! keep a user's objects from answering it ([`crate::postgres`] states them): each operator,
//! function, type and table is `pg_catalog`'s by name, each function one that takes no polymorphic
//! argument as it is called, no construct looks an operator up by name, and no literal reaches an
//! operator or a function untyped. What the server does with them is the loader's test of hostile
//! objects (`tests/catalog-queries.test.ts`).

use crate::{cardinality, errors, extension, pgtypes, postgres};
use pg_query::protobuf::{AExprKind, SubLinkType};
use serde_json::{Map, Value};
use reylo_engine_core::{fresh, structured};

/// Each query by its name, the freshness check's with two tables in place of its list.
fn queries() -> Vec<(&'static str, String)> {
    let tables = "('\"users\"'::pg_catalog.text), ('\"archive\".\"users\"'::pg_catalog.text)";
    vec![
        ("ATTNOTNULL_QUERY", postgres::ATTNOTNULL_QUERY.into()),
        ("CHECKS_QUERY", postgres::CHECKS_QUERY.into()),
        ("REFUSES_NULL_QUERY", postgres::REFUSES_NULL_QUERY.into()),
        ("COMMENTS_QUERY", postgres::COMMENTS_QUERY.into()),
        ("RELNAMES_QUERY", postgres::RELNAMES_QUERY.into()),
        ("REQUIRED_COLUMNS_QUERY", postgres::REQUIRED_COLUMNS_QUERY.into()),
        ("RELATION_COLUMNS_QUERY", postgres::RELATION_COLUMNS_QUERY.into()),
        ("TRUST_QUERY", postgres::TRUST_QUERY.into()),
        ("ARBITERS_QUERY", postgres::ARBITERS_QUERY.into()),
        ("TRIGGERS_QUERY", cardinality::TRIGGERS_QUERY.into()),
        ("UNIQUE_KEYS_QUERY", cardinality::UNIQUE_KEYS_QUERY.into()),
        ("COLUMNS_QUERY", cardinality::COLUMNS_QUERY.into()),
        ("FUNCTIONS_QUERY", cardinality::FUNCTIONS_QUERY.into()),
        ("SET_RETURNING_OPERATORS_QUERY", cardinality::SET_RETURNING_OPERATORS_QUERY.into()),
        ("TYPE_QUERY", pgtypes::TYPE_QUERY.into()),
        ("RELATION_TYPES_QUERY", pgtypes::RELATION_TYPES_QUERY.into()),
        ("ERRORS_QUERY", errors::ERRORS_QUERY.into()),
        ("PROBE_QUERY", extension::PROBE_QUERY.into()),
        ("PG_TABLE_QUERY", structured::PG_TABLE_QUERY.into()),
        ("fresh::PG_QUERY", fresh::PG_QUERY.replace("{tables}", tables)),
        ("fresh::PG_QUERY_DEFAULTS", fresh::PG_QUERY_DEFAULTS.replace("{tables}", tables)),
    ]
}

/// The functions the queries call, each in a form that is an exact match of a built-in taking no
/// polymorphic argument. A query that needs another adds it here once it is checked to be so:
/// `unnest`, `array_agg`, `cardinality`, `array_length`, `to_jsonb` and the like take any type, and
/// an overload of narrower types created in `pg_catalog` would be the closer match.
const FUNCTIONS: [&str; 24] = [
    "bit_or",
    "bool_or",
    "count",
    "current_setting",
    "format_type",
    "generate_series",
    "jsonb_in",
    "oidout",
    "pg_get_constraintdef",
    "pg_get_expr",
    "pg_get_indexdef",
    "pg_get_serial_sequence",
    "pg_get_viewdef",
    "pg_operator_is_visible",
    "pg_table_is_visible",
    "quote_ident",
    "quote_literal",
    "regclassout",
    "split_part",
    "strpos",
    "textin",
    "textout",
    "to_number",
    "to_regclass",
];

/// A name list's parts, as the parse tree's JSON writes them (`[{"node": {"String": {"sval": …}}}]`).
fn parts(names: Option<&Value>) -> Vec<&str> {
    names.and_then(Value::as_array).into_iter().flatten().filter_map(|n| n.pointer("/node/String/sval").and_then(Value::as_str)).collect()
}

/// Whether a node is a string literal or a NULL with no type of its own.
fn untyped(node: Option<&Value>) -> bool {
    node.and_then(|n| n.pointer("/node/AConst")).is_some_and(|c| c.get("isnull").and_then(Value::as_bool) == Some(true) || c.pointer("/val/Sval").is_some())
}

/// Every object in the tree with the key it is under, depth first.
fn objects<'a>(key: &'a str, value: &'a Value, out: &mut Vec<(&'a str, &'a Map<String, Value>)>) {
    match value {
        Value::Object(map) => {
            out.push((key, map));
            for (k, v) in map {
                objects(k, v, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| objects(key, v, out)),
        _ => {}
    }
}

/// What `sql` breaks of the rules, one line each.
fn violations(sql: &str) -> Vec<String> {
    let parsed = pg_query::parse(sql, 0).unwrap_or_else(|e| panic!("{e}\n{sql}"));
    let tree = serde_json::to_value(&parsed.protobuf).unwrap();
    let mut all = Vec::new();
    objects("", &tree, &mut all);
    let ctes: Vec<&str> = all.iter().filter(|(k, _)| *k == "CommonTableExpr").filter_map(|(_, m)| m.get("ctename").and_then(Value::as_str)).collect();
    let catalog = |names: Option<&Value>| parts(names).first() == Some(&"pg_catalog");
    let mut out = Vec::new();
    for (key, m) in &all {
        match *key {
            "AExpr" => {
                let op = parts(m.get("name")).join(".");
                let kind = m.get("kind").and_then(Value::as_i64).unwrap_or(0) as i32;
                if ![AExprKind::AexprOp as i32, AExprKind::AexprOpAny as i32, AExprKind::AexprOpAll as i32].contains(&kind) {
                    out.push(format!("an expression that looks `{op}` up by name ({:?})", AExprKind::try_from(kind)));
                } else if !catalog(m.get("name")) {
                    out.push(format!("the operator `{op}`, not spelled OPERATOR(pg_catalog.{op})"));
                }
                if untyped(m.get("lexpr")) || untyped(m.get("rexpr")) {
                    out.push(format!("an untyped literal beside `{op}`"));
                }
            }
            "SubLink" => {
                let kind = m.get("sub_link_type").and_then(Value::as_i64).unwrap_or(0) as i32;
                if [SubLinkType::AnySublink as i32, SubLinkType::AllSublink as i32].contains(&kind) && !catalog(m.get("oper_name")) {
                    out.push(format!("`{}` ANY or ALL over a subquery (IN), not spelled OPERATOR(pg_catalog.…)", parts(m.get("oper_name")).join(".")));
                }
            }
            "FuncCall" => {
                let name = parts(m.get("funcname"));
                match name.as_slice() {
                    ["pg_catalog", function] if FUNCTIONS.contains(function) => {}
                    ["pgreylo", _] => {}
                    _ => out.push(format!("the function `{}`, not one of pg_catalog's the queries may call", name.join("."))),
                }
                if m.get("args").and_then(Value::as_array).is_some_and(|args| args.iter().any(|a| untyped(Some(a)))) {
                    out.push(format!("an untyped literal passed to `{}`", name.join(".")));
                }
            }
            "type_name" | "TypeName" if !catalog(m.get("names")) => out.push(format!("the type `{}`, not pg_catalog's", parts(m.get("names")).join("."))),
            "RangeVar" => {
                let schema = m.get("schemaname").and_then(Value::as_str).unwrap_or("");
                let name = m.get("relname").and_then(Value::as_str).unwrap_or("");
                if schema != "pg_catalog" && !(schema.is_empty() && ctes.contains(&name)) {
                    out.push(format!("the table `{name}`, not pg_catalog's"));
                }
            }
            "CaseExpr" if m.get("arg").is_some_and(|a| !a.is_null()) => out.push("a CASE of an operand, which looks `=` up by name".into()),
            "JoinExpr" if m.get("is_natural").and_then(Value::as_bool) == Some(true) || m.get("using_clause").and_then(Value::as_array).is_some_and(|u| !u.is_empty()) => {
                out.push("a NATURAL or USING join, which looks `=` up by name".into())
            }
            _ => {}
        }
    }
    out
}

#[test]
fn every_catalog_query_names_only_pg_catalog_s_objects() {
    let mut failures = Vec::new();
    for (name, sql) in queries() {
        for v in violations(&sql) {
            failures.push(format!("{name}: {v}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_rules_catch_what_a_user_may_capture() {
    let caught = |sql: &str| violations(sql).len();
    assert_eq!(caught("SELECT c.oid FROM pg_catalog.pg_class c WHERE c.oid OPERATOR(pg_catalog.=) '1'::pg_catalog.oid"), 0);
    assert_eq!(caught("SELECT c.oid FROM pg_class c"), 1);
    assert_eq!(caught("SELECT 1 FROM pg_catalog.pg_class c WHERE c.relname = 'x'::pg_catalog.name"), 1);
    assert_eq!(caught("SELECT 1 FROM pg_catalog.pg_class c WHERE c.relkind OPERATOR(pg_catalog.=) 'r'"), 1);
    assert_eq!(caught("SELECT 1 FROM pg_catalog.pg_class c WHERE c.relkind IN ('r'::pg_catalog.\"char\")"), 1);
    assert_eq!(caught("SELECT 1 FROM pg_catalog.pg_class c WHERE c.oid IN (SELECT t.oid FROM pg_catalog.pg_type t)"), 1);
    assert_eq!(caught("SELECT pg_catalog.unnest(c.relacl) FROM pg_catalog.pg_class c"), 1);
    assert_eq!(caught("SELECT pg_catalog.cardinality(ARRAY[1])"), 1);
    assert_eq!(caught("SELECT c.oid::int8 FROM pg_catalog.pg_class c"), 1);
    assert_eq!(caught("SELECT CASE c.relkind WHEN 'r'::pg_catalog.\"char\" THEN 1 END FROM pg_catalog.pg_class c"), 1);
    assert_eq!(caught("SELECT NULLIF(1, 2)"), 1);
    assert_eq!(caught("WITH x AS (SELECT 1 AS n) SELECT x.n FROM x JOIN x y USING (n)"), 1);
}
