//! The errors a Postgres statement may raise, where it runs only the server's own code
//! ([`reylo_engine_core::errors`]): one catalog query says whether anything the statement sets
//! off is a user's or an extension's code, and else which constraints, tables, columns and domains
//! each constraint error may name.

use crate::analyze;
use crate::cardinality::relation_named;
use crate::host::Host;
use reylo_engine_core::api::{Analysis, Scope};
use reylo_engine_core::errors::Errors;
use reylo_engine_core::host::Lazy;
use serde_json::{json, Value};

/// The comparisons a statement makes without writing their operators: a `CASE x WHEN`, a `USING`
/// or a natural join, a `BETWEEN`, `DISTINCT`, `GROUP BY` and `ORDER BY`.
const IMPLICIT_OPERATORS: [&str; 6] = ["=", "<>", "<", "<=", ">", ">="];

/// The server's functions that run SQL they are given as text, which may call anything.
const RUNS_SQL: [&str; 14] = [
    "query_to_xml",
    "query_to_xmlschema",
    "query_to_xml_and_xmlschema",
    "cursor_to_xml",
    "cursor_to_xmlschema",
    "table_to_xml",
    "table_to_xmlschema",
    "table_to_xml_and_xmlschema",
    "schema_to_xml",
    "schema_to_xmlschema",
    "schema_to_xml_and_xmlschema",
    "database_to_xml",
    "database_to_xmlschema",
    "database_to_xml_and_xmlschema",
];

/// `$1` and `$2` are `text[]` of the relations the statement writes and reads, as it writes them;
/// `$3`, `$4` and `$5` the functions, the operators and the types it names; `$6` whether a row it
/// writes may change a key another table's foreign key refers to. One row `('open', why)`
/// for each thing the statement may set off that a user or an extension wrote (an object with an
/// oid of 16384 or more, the first a user's database gives), and one `('name', code, field, name)`
/// for each name an error may carry.
///
/// What a write reaches: the written relations, their partitions and inheritance children, a
/// view's base relations, and, where `$6`, the tables a foreign key's action writes in turn. What a read
/// reaches: the read relations and, through views, the relations they read. Code runs there in a
/// trigger or a rule; in a default, a generated column, a check, an index's expression, a
/// partition key, a domain's default or check or a policy, which `pg_depend` ties to the functions
/// and operators they call; in an index's operator class; in a type's input and output; across a foreign table; in a user's or an
/// extension's function of a name the statement calls, whatever it takes, since a literal or a
/// parameter of no type takes the type of the one function there is; and in a user's or an
/// extension's operator or cast of the statement, where each type of a user's it takes is present.
/// An operator also opens it where an argument of no type may take a type that is not present: a
/// prefix operator, or one where the server has no operator of the name taking the other
/// argument's type on both sides (`text` where neither type is present), which Postgres would pick
/// first. Present is
/// a column's type, a row type, a type the statement names or a function it calls returns, a
/// preferred type or one a built-in type casts to implicitly, and what those are built of (a
/// domain's base, an array's element and its array, a composite's fields, a range's subtype).
/// A server newer than 18 may raise a code the run time's list lacks.
pub const ERRORS_QUERY: &str = concat!(
    "WITH RECURSIVE \
    named(oid, written) AS ( \
      SELECT c.oid, true FROM unnest($1::text[]) AS x(name) JOIN pg_class c ON c.oid = ",
    relation_named!("x.name"),
    " UNION ALL SELECT c.oid, false FROM unnest($2::text[]) AS x(name) JOIN pg_class c ON c.oid = ",
    relation_named!("x.name"),
    "), \
    edge(parent, child, view) AS ( \
      SELECT inhparent, inhrelid, false FROM pg_inherits \
      UNION ALL SELECT q.ev_class, d.refobjid, true FROM pg_rewrite q JOIN pg_depend d ON d.classid = 'pg_rewrite'::regclass AND d.objid = q.oid \
        AND d.refclassid = 'pg_class'::regclass AND d.refobjid <> q.ev_class WHERE q.rulename = '_RETURN' \
      UNION ALL SELECT confrelid, conrelid, false FROM pg_constraint WHERE $6::bool AND contype = 'f' AND (confdeltype IN ('c', 'n', 'd') OR confupdtype IN ('c', 'n', 'd'))), \
    w(oid) AS (SELECT oid FROM named WHERE written UNION SELECT e.child FROM edge e JOIN w ON e.parent = w.oid), \
    r(oid) AS (SELECT oid FROM named UNION SELECT e.child FROM edge e JOIN r ON e.parent = r.oid WHERE e.view), \
    rel(oid) AS (SELECT oid FROM w UNION SELECT oid FROM r), \
    dom(oid) AS ( \
      SELECT t.oid FROM pg_attribute a JOIN w ON a.attrelid = w.oid JOIN pg_type t ON t.oid = a.atttypid WHERE a.attnum > 0 AND NOT a.attisdropped AND t.typtype = 'd' \
      UNION SELECT t.oid FROM pg_type t WHERE t.typname = ANY($5::text[]) AND t.typtype = 'd' \
      UNION SELECT t.typbasetype FROM dom JOIN pg_type t ON t.oid = dom.oid JOIN pg_type b ON b.oid = t.typbasetype WHERE b.typtype = 'd'), \
    userio(oid) AS (SELECT t.oid FROM pg_type t WHERE t.oid >= 16384 AND t.typtype IN ('b', 'r', 'm', 'c') AND t.typinput <> 'array_in'::regproc), \
    coded(oid) AS ( \
      SELECT oid FROM userio \
      UNION SELECT a.oid FROM pg_type a JOIN userio u ON a.typelem = u.oid WHERE a.typinput = 'array_in'::regproc \
      UNION SELECT d.oid FROM pg_type d JOIN pg_type b ON b.oid = d.typbasetype WHERE d.typtype = 'd' \
        AND (b.oid IN (SELECT oid FROM userio) OR b.typtype = 'd' OR (b.typinput = 'array_in'::regproc AND b.typelem IN (SELECT oid FROM userio)))), \
    cons(oid) AS (SELECT c.oid FROM pg_constraint c WHERE c.conrelid IN (SELECT oid FROM w) OR c.contypid IN (SELECT oid FROM dom)), \
    present(oid) AS ( \
      SELECT a.atttypid FROM pg_attribute a JOIN rel ON a.attrelid = rel.oid WHERE a.attnum > 0 AND NOT a.attisdropped \
      UNION SELECT c.reltype FROM pg_class c JOIN rel ON c.oid = rel.oid \
      UNION SELECT t.oid FROM pg_type t WHERE t.typname = ANY($5::text[]) \
      UNION SELECT p.prorettype FROM pg_proc p WHERE p.proname = ANY($3::text[]) \
      UNION SELECT t.oid FROM pg_type t WHERE t.oid >= 16384 AND t.typispreferred \
      UNION SELECT c.casttarget FROM pg_cast c WHERE c.castcontext = 'i' AND c.casttarget >= 16384 \
      UNION SELECT y.oid FROM present p JOIN pg_type t ON t.oid = p.oid CROSS JOIN LATERAL ( \
        SELECT t.typbasetype UNION ALL SELECT t.typelem UNION ALL SELECT t.typarray \
        UNION ALL SELECT a.atttypid FROM pg_attribute a WHERE a.attrelid = t.typrelid AND a.attnum > 0 AND NOT a.attisdropped \
        UNION ALL SELECT g.rngsubtype FROM pg_range g WHERE g.rngtypid = t.oid) AS y(oid) WHERE y.oid <> 0) \
    SELECT 'open', 'a server newer than 18', NULL, NULL WHERE pg_catalog.current_setting('server_version_num')::int4 >= 190000 \
    UNION ALL SELECT 'open', 'trigger', g.tgname::text, NULL FROM pg_trigger g JOIN w ON g.tgrelid = w.oid WHERE NOT g.tgisinternal AND g.tgenabled <> 'D' \
    UNION ALL SELECT 'open', 'rule', q.rulename::text, NULL FROM pg_rewrite q JOIN w ON q.ev_class = w.oid WHERE q.rulename <> '_RETURN' AND q.ev_enabled <> 'D' \
    UNION ALL SELECT 'open', 'foreign table', c.relname::text, NULL FROM pg_class c JOIN rel ON c.oid = rel.oid WHERE c.relkind = 'f' \
    UNION ALL SELECT 'open', 'depends on', d.refobjid::text, NULL FROM pg_depend d \
      WHERE d.refclassid IN ('pg_proc'::regclass, 'pg_operator'::regclass, 'pg_opclass'::regclass, 'pg_opfamily'::regclass) AND d.refobjid >= 16384 AND ( \
        (d.classid = 'pg_type'::regclass AND d.objid IN (SELECT oid FROM dom)) OR \
        (d.classid = 'pg_attrdef'::regclass AND d.objid IN (SELECT ad.oid FROM pg_attrdef ad JOIN rel ON ad.adrelid = rel.oid)) \
        OR (d.classid = 'pg_constraint'::regclass AND d.objid IN (SELECT oid FROM cons)) \
        OR (d.classid = 'pg_class'::regclass AND d.objid IN (SELECT oid FROM w UNION SELECT i.indexrelid FROM pg_index i JOIN w ON i.indrelid = w.oid)) \
        OR (d.classid = 'pg_policy'::regclass AND d.objid IN (SELECT p.oid FROM pg_policy p JOIN rel ON p.polrelid = rel.oid)) \
        OR (d.classid = 'pg_rewrite'::regclass AND d.objid IN (SELECT q.oid FROM pg_rewrite q JOIN rel ON q.ev_class = rel.oid))) \
    UNION ALL SELECT 'open', 'column type', t.typname::text, NULL FROM pg_attribute a JOIN rel ON a.attrelid = rel.oid JOIN pg_type t ON t.oid = a.atttypid \
      WHERE a.attnum > 0 AND NOT a.attisdropped AND t.oid IN (SELECT oid FROM coded) \
    UNION ALL SELECT 'open', 'function', p.proname::text, NULL FROM pg_proc p WHERE p.oid >= 16384 AND p.proname = ANY($3::text[]) \
    UNION ALL SELECT 'open', 'operator', o.oprname::text, NULL FROM pg_operator o WHERE o.oid >= 16384 AND o.oprname = ANY($4::text[]) \
      AND ((o.oprleft < 16384 OR o.oprleft IN (SELECT oid FROM present)) AND (o.oprright < 16384 OR o.oprright IN (SELECT oid FROM present)) \
        OR o.oprleft = 0 \
        OR NOT EXISTS (SELECT 1 FROM pg_operator b WHERE b.oid < 16384 AND b.oprname = o.oprname AND b.oprleft = b.oprright AND b.oprleft = CASE \
          WHEN o.oprleft < 16384 OR o.oprleft IN (SELECT oid FROM present) THEN o.oprleft \
          WHEN o.oprright < 16384 OR o.oprright IN (SELECT oid FROM present) THEN o.oprright ELSE 'text'::regtype END)) \
    UNION ALL SELECT 'open', 'type', t.typname::text, NULL FROM pg_type t WHERE t.typname = ANY($5::text[]) AND t.oid IN (SELECT oid FROM coded) \
    UNION ALL SELECT 'open', 'cast', c.oid::text, NULL FROM pg_cast c WHERE c.oid >= 16384 AND c.castmethod = 'f' \
      AND ((c.castsource < 16384 AND c.casttarget < 16384) OR c.castsource IN (SELECT oid FROM present) OR c.casttarget IN (SELECT oid FROM present)) \
    UNION ALL SELECT 'name', k.code, k.field, k.name FROM ( \
      SELECT CASE WHEN i.indisexclusion THEN '23P01' ELSE '23505' END AS code, 'constraint' AS field, ic.relname::text AS name \
        FROM pg_index i JOIN w ON i.indrelid = w.oid JOIN pg_class ic ON ic.oid = i.indexrelid WHERE i.indisunique OR i.indisexclusion \
      UNION ALL SELECT CASE WHEN i.indisexclusion THEN '23P01' ELSE '23505' END, 'table', c.relname::text \
        FROM pg_index i JOIN w ON i.indrelid = w.oid JOIN pg_class c ON c.oid = i.indrelid WHERE i.indisunique OR i.indisexclusion \
      UNION ALL SELECT '23P01', 'constraint', ic.relname::text FROM pg_index i JOIN w ON i.indrelid = w.oid JOIN pg_class ic ON ic.oid = i.indexrelid \
        WHERE i.indisunique AND i.indisexclusion \
      UNION ALL SELECT '23P01', 'table', c.relname::text FROM pg_index i JOIN w ON i.indrelid = w.oid JOIN pg_class c ON c.oid = i.indrelid \
        WHERE i.indisunique AND i.indisexclusion \
      UNION ALL SELECT '23502', 'column', a.attname::text FROM pg_attribute a JOIN w ON a.attrelid = w.oid WHERE a.attnum > 0 AND NOT a.attisdropped AND a.attnotnull \
      UNION ALL SELECT '23502', 'table', c.relname::text FROM pg_class c JOIN w ON c.oid = w.oid \
      UNION ALL SELECT '23502', 'dataType', t.typname::text FROM pg_type t JOIN dom ON t.oid = dom.oid WHERE t.typnotnull \
      UNION ALL SELECT f.code, f.field, f.name FROM pg_constraint k \
        CROSS JOIN LATERAL (VALUES ('23503', 'constraint', k.conname::text), ('23503', 'table', (SELECT relname::text FROM pg_class WHERE oid = k.conrelid)), \
          ('23503', 'table', (SELECT relname::text FROM pg_class WHERE oid = k.confrelid))) AS f(code, field, name) \
        WHERE k.contype = 'f' AND (k.conrelid IN (SELECT oid FROM w) OR ($6::bool AND k.confrelid IN (SELECT oid FROM w))) \
      UNION ALL SELECT f.code, f.field, f.name FROM pg_constraint k \
        CROSS JOIN LATERAL (VALUES ('23001', 'constraint', k.conname::text), ('23001', 'table', (SELECT relname::text FROM pg_class WHERE oid = k.conrelid)), \
          ('23001', 'table', (SELECT relname::text FROM pg_class WHERE oid = k.confrelid))) AS f(code, field, name) \
        WHERE $6::bool AND k.contype = 'f' AND k.confrelid IN (SELECT oid FROM w) AND 'r' IN (k.confdeltype, k.confupdtype) \
      UNION ALL SELECT '23514', 'constraint', k.conname::text FROM pg_constraint k WHERE k.contype = 'c' AND k.oid IN (SELECT oid FROM cons) \
      UNION ALL SELECT '23514', 'table', c.relname::text FROM pg_class c JOIN w ON c.oid = w.oid \
      UNION ALL SELECT '23514', 'dataType', t.typname::text FROM pg_type t JOIN dom ON t.oid = dom.oid \
      UNION ALL SELECT '23P01', 'constraint', k.conname::text FROM pg_constraint k JOIN w ON k.conrelid = w.oid WHERE k.contype = 'x' \
    ) AS k(code, field, name) WHERE k.name IS NOT NULL"
);

/// The names each error the statement may raise can carry: `Some(None)` where it may run code the
/// server does not define, or names nothing the parse can read.
pub fn errors(analysis: &Analysis, host: &Host) -> Lazy<Option<Errors>> {
    if analysis.kind.is_empty() {
        return Some(None);
    }
    let Ok(parsed) = analyze::analyze(&analysis.rewritten.sql) else { return Some(None) };
    if parsed.functions.iter().any(|f| RUNS_SQL.contains(&f.as_str()) || f == "ts_stat" || f == "ts_rewrite") {
        return Some(None);
    }
    let mut operators: Vec<&str> = parsed.operators.iter().map(String::as_str).collect();
    operators.extend(IMPLICIT_OPERATORS.iter().filter(|o| !parsed.operators.iter().any(|p| p == *o)));
    let params = vec![json!(parsed.write_set), json!(parsed.read_set), json!(parsed.functions), json!(operators), json!(parsed.types), json!(parsed.changes_keys)];
    let rows = host.query(ERRORS_QUERY, params, Scope::Statement)?;
    let Ok(rows) = rows else { return Some(None) };
    Some(narrowed(&rows))
}

/// What [`ERRORS_QUERY`]'s rows say: nothing where one opens the statement.
fn narrowed(rows: &[Vec<Value>]) -> Option<Errors> {
    let text = |row: &[Value], k: usize| row.get(k).and_then(Value::as_str).map(str::to_string);
    let mut errors = Errors::default();
    for row in rows {
        match text(row, 0).as_deref() {
            Some("name") => {
                if let (Some(code), Some(field), Some(name)) = (text(row, 1), text(row, 2), text(row, 3)) {
                    errors.add(&code, &field, &name);
                }
            }
            _ => return None,
        }
    }
    Some(errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_open_row_narrows_nothing() {
        let name = |code: &str, field: &str, n: &str| vec![json!("name"), json!(code), json!(field), json!(n)];
        let rows = vec![name("23505", "constraint", "users_pkey"), name("23505", "table", "users")];
        assert_eq!(serde_json::to_value(narrowed(&rows)).unwrap(), json!({ "23505": { "constraint": ["users_pkey"], "table": ["users"] } }));
        let mut open = rows.clone();
        open.push(vec![json!("open"), json!("trigger"), json!("audit"), Value::Null]);
        assert_eq!(narrowed(&open), None);
        assert_eq!(narrowed(&[]), Some(Errors::default()));
    }
}
