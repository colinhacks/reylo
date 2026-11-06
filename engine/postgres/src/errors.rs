//! The errors a Postgres statement may raise, where it runs only the server's own code
//! ([`reylo_engine_core::errors`]): one catalog query says whether anything the statement sets
//! off is a user's or an extension's code, and else which constraints, tables, columns and domains
//! each constraint error may name.

use crate::analyze;
use crate::cardinality::{names, relation_named};
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

/// The oid of the system catalog `$name`, as `pg_depend` names a class.
macro_rules! catalog {
    ($name:literal) => {
        concat!("'pg_catalog.", $name, "'::pg_catalog.regclass::pg_catalog.oid")
    };
}

/// The oid of `array_in`, the input function of every array type.
macro_rules! array_in {
    () => {
        "'pg_catalog.array_in'::pg_catalog.regproc::pg_catalog.oid"
    };
}

/// The name of the relation of oid `$oid`, as text.
macro_rules! relname {
    ($oid:literal) => {
        concat!("(SELECT c.relname::pg_catalog.text FROM pg_catalog.pg_class c WHERE c.oid OPERATOR(pg_catalog.=) ", $oid, ")")
    };
}

/// `$1` and `$2` are `text[]` of the relations the statement writes and reads, as it writes them;
/// `$3` and `$4` the functions and the types it names; `$5` whether a row it
/// writes may change a key another table's foreign key refers to; `$6` and `$7` how many names
/// `$1` and `$2` hold. One row `('open', why)`
/// for each thing the statement may set off that a user or an extension wrote (an object with an
/// oid of 16384 or more, the first a user's database gives), and one `('name', code, field, name)`
/// for each name an error may carry.
///
/// What a write reaches: the written relations, their partitions and inheritance children, a
/// view's base relations, and, where `$5`, the tables a foreign key's action writes in turn. What a read
/// reaches: the read relations, their partitions and inheritance children, whose indexes a scan of
/// the parent may use, and, through views, the relations they read. Code runs there in a
/// trigger or a rule; in a default, a generated column, a check, an index's expression, a
/// partition key, a domain's default or check or a policy, which `pg_depend` ties to the functions
/// and operators they call; in the operator class of an index or a partition key, on a read too,
/// since a scan of the index or the pruning of the partitions calls it; in a type's input and output; across a foreign table; in a user's or an
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
/// A server newer than 18 may raise a code the run time's list lacks. An `open` row's last column is
/// `reads` where what it finds cannot write ([`read_only`]): a type's input and output, or a function
/// that is not volatile. One row `('present', oid)` for
/// each type of a user's present, which [`OPERATORS_QUERY`] takes. One row `('runs', name, 'built in' or
/// NULL)` for each volatile function, or aggregate over one, that a view or a policy a read reaches
/// calls, read from the stored expression, since `pg_depend` records nothing for a built-in and every
/// aggregate is labelled immutable ([`read_only`]). A policy's relations are among what a read reaches, and never among what a write does: it only reads them.
pub const ERRORS_QUERY: &str = concat!(
    "WITH RECURSIVE \
    named(oid, written) AS ( \
      SELECT c.oid, true FROM ",
    names!("$1::pg_catalog.text[]", "$6::pg_catalog.int4", "x"),
    " JOIN pg_catalog.pg_class c ON c.oid OPERATOR(pg_catalog.=) ",
    relation_named!("x.name"),
    " UNION ALL SELECT c.oid, false FROM ",
    names!("$2::pg_catalog.text[]", "$7::pg_catalog.int4", "x"),
    " JOIN pg_catalog.pg_class c ON c.oid OPERATOR(pg_catalog.=) ",
    relation_named!("x.name"),
    "), \
    edge(parent, child, read, written) AS ( \
      SELECT h.inhparent, h.inhrelid, true, true FROM pg_catalog.pg_inherits h \
      UNION ALL SELECT q.ev_class, d.refobjid, true, true FROM pg_catalog.pg_rewrite q JOIN pg_catalog.pg_depend d \
        ON d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_rewrite"), " AND d.objid OPERATOR(pg_catalog.=) q.oid \
        AND d.refclassid OPERATOR(pg_catalog.=) ", catalog!("pg_class"), " AND d.refobjid OPERATOR(pg_catalog.<>) q.ev_class \
        WHERE q.rulename OPERATOR(pg_catalog.=) '_RETURN'::pg_catalog.name \
      UNION ALL SELECT k.confrelid, k.conrelid, false, true FROM pg_catalog.pg_constraint k WHERE $5::pg_catalog.bool AND k.contype OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\" \
        AND (k.confdeltype OPERATOR(pg_catalog.=) ANY ('{c,n,d}'::pg_catalog.\"char\"[]) OR k.confupdtype OPERATOR(pg_catalog.=) ANY ('{c,n,d}'::pg_catalog.\"char\"[])) \
      UNION ALL SELECT y.polrelid, d.refobjid, true, false FROM pg_catalog.pg_policy y JOIN pg_catalog.pg_depend d \
        ON d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_policy"), " AND d.objid OPERATOR(pg_catalog.=) y.oid \
        AND d.refclassid OPERATOR(pg_catalog.=) ", catalog!("pg_class"), " AND d.refobjid OPERATOR(pg_catalog.<>) y.polrelid), \
    w(oid) AS (SELECT n.oid FROM named n WHERE n.written UNION SELECT e.child FROM edge e JOIN w ON e.parent OPERATOR(pg_catalog.=) w.oid WHERE e.written), \
    r(oid) AS (SELECT n.oid FROM named n UNION SELECT e.child FROM edge e JOIN r ON e.parent OPERATOR(pg_catalog.=) r.oid WHERE e.read), \
    rel(oid) AS (SELECT w.oid FROM w UNION SELECT r.oid FROM r), \
    dom(oid) AS ( \
      SELECT t.oid FROM pg_catalog.pg_attribute a JOIN w ON a.attrelid OPERATOR(pg_catalog.=) w.oid JOIN pg_catalog.pg_type t ON t.oid OPERATOR(pg_catalog.=) a.atttypid \
        WHERE a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped AND t.typtype OPERATOR(pg_catalog.=) 'd'::pg_catalog.\"char\" \
      UNION SELECT t.oid FROM pg_catalog.pg_type t WHERE t.typname OPERATOR(pg_catalog.=) ANY ($4::pg_catalog.text[]) AND t.typtype OPERATOR(pg_catalog.=) 'd'::pg_catalog.\"char\" \
      UNION SELECT t.typbasetype FROM dom JOIN pg_catalog.pg_type t ON t.oid OPERATOR(pg_catalog.=) dom.oid \
        JOIN pg_catalog.pg_type b ON b.oid OPERATOR(pg_catalog.=) t.typbasetype WHERE b.typtype OPERATOR(pg_catalog.=) 'd'::pg_catalog.\"char\"), \
    userio(oid) AS (SELECT t.oid FROM pg_catalog.pg_type t WHERE t.oid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid \
      AND t.typtype OPERATOR(pg_catalog.=) ANY ('{b,r,m,c}'::pg_catalog.\"char\"[]) AND t.typinput::pg_catalog.oid OPERATOR(pg_catalog.<>) ", array_in!(), "), \
    coded(oid) AS ( \
      SELECT u.oid FROM userio u \
      UNION SELECT a.oid FROM pg_catalog.pg_type a JOIN userio u ON a.typelem OPERATOR(pg_catalog.=) u.oid WHERE a.typinput::pg_catalog.oid OPERATOR(pg_catalog.=) ", array_in!(), " \
      UNION SELECT d.oid FROM pg_catalog.pg_type d JOIN pg_catalog.pg_type b ON b.oid OPERATOR(pg_catalog.=) d.typbasetype WHERE d.typtype OPERATOR(pg_catalog.=) 'd'::pg_catalog.\"char\" \
        AND (b.oid OPERATOR(pg_catalog.=) ANY (SELECT u.oid FROM userio u) OR b.typtype OPERATOR(pg_catalog.=) 'd'::pg_catalog.\"char\" \
          OR (b.typinput::pg_catalog.oid OPERATOR(pg_catalog.=) ", array_in!(), " AND b.typelem OPERATOR(pg_catalog.=) ANY (SELECT u.oid FROM userio u)))), \
    cons(oid) AS (SELECT c.oid FROM pg_catalog.pg_constraint c \
      WHERE c.conrelid OPERATOR(pg_catalog.=) ANY (SELECT w.oid FROM w) OR c.contypid OPERATOR(pg_catalog.=) ANY (SELECT dom.oid FROM dom)), \
    present(oid) AS ( \
      SELECT a.atttypid FROM pg_catalog.pg_attribute a JOIN rel ON a.attrelid OPERATOR(pg_catalog.=) rel.oid WHERE a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped \
      UNION SELECT c.reltype FROM pg_catalog.pg_class c JOIN rel ON c.oid OPERATOR(pg_catalog.=) rel.oid \
      UNION SELECT t.oid FROM pg_catalog.pg_type t WHERE t.typname OPERATOR(pg_catalog.=) ANY ($4::pg_catalog.text[]) \
      UNION SELECT p.prorettype FROM pg_catalog.pg_proc p WHERE p.proname OPERATOR(pg_catalog.=) ANY ($3::pg_catalog.text[]) \
      UNION SELECT t.oid FROM pg_catalog.pg_type t WHERE t.oid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid AND t.typispreferred \
      UNION SELECT c.casttarget FROM pg_catalog.pg_cast c WHERE c.castcontext OPERATOR(pg_catalog.=) 'i'::pg_catalog.\"char\" AND c.casttarget OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid \
      UNION SELECT y.oid FROM present p JOIN pg_catalog.pg_type t ON t.oid OPERATOR(pg_catalog.=) p.oid CROSS JOIN LATERAL ( \
        SELECT t.typbasetype UNION ALL SELECT t.typelem UNION ALL SELECT t.typarray \
        UNION ALL SELECT a.atttypid FROM pg_catalog.pg_attribute a WHERE a.attrelid OPERATOR(pg_catalog.=) t.typrelid AND a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped \
        UNION ALL SELECT g.rngsubtype FROM pg_catalog.pg_range g WHERE g.rngtypid OPERATOR(pg_catalog.=) t.oid) AS y(oid) WHERE y.oid OPERATOR(pg_catalog.<>) '0'::pg_catalog.oid) \
    SELECT 'open'::pg_catalog.text, 'a server newer than 18'::pg_catalog.text, NULL::pg_catalog.text, NULL::pg_catalog.text \
      WHERE pg_catalog.to_number(pg_catalog.current_setting('server_version_num'::pg_catalog.text), '999999'::pg_catalog.text) OPERATOR(pg_catalog.>=) '190000'::pg_catalog.numeric \
    UNION ALL SELECT 'open', 'trigger', g.tgname::pg_catalog.text, NULL FROM pg_catalog.pg_trigger g JOIN w ON g.tgrelid OPERATOR(pg_catalog.=) w.oid \
      WHERE NOT g.tgisinternal AND g.tgenabled OPERATOR(pg_catalog.<>) 'D'::pg_catalog.\"char\" \
    UNION ALL SELECT 'open', 'rule', q.rulename::pg_catalog.text, NULL FROM pg_catalog.pg_rewrite q JOIN w ON q.ev_class OPERATOR(pg_catalog.=) w.oid \
      WHERE q.rulename OPERATOR(pg_catalog.<>) '_RETURN'::pg_catalog.name AND q.ev_enabled OPERATOR(pg_catalog.<>) 'D'::pg_catalog.\"char\" \
    UNION ALL SELECT 'open', 'foreign table', c.relname::pg_catalog.text, NULL FROM pg_catalog.pg_class c JOIN rel ON c.oid OPERATOR(pg_catalog.=) rel.oid \
      WHERE c.relkind OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\" \
    UNION ALL SELECT 'open', 'depends on', pg_catalog.textin(pg_catalog.oidout(d.refobjid)), \
        CASE WHEN d.refclassid OPERATOR(pg_catalog.=) ANY (ARRAY[", catalog!("pg_opclass"), ", ", catalog!("pg_opfamily"), "]) \
          OR NOT EXISTS (SELECT 1 FROM pg_catalog.pg_proc p WHERE p.provolatile OPERATOR(pg_catalog.=) 'v'::pg_catalog.\"char\" AND ( \
            (d.refclassid OPERATOR(pg_catalog.=) ", catalog!("pg_proc"), " AND p.oid OPERATOR(pg_catalog.=) d.refobjid) \
            OR (d.refclassid OPERATOR(pg_catalog.=) ", catalog!("pg_operator"), " AND p.oid OPERATOR(pg_catalog.=) (SELECT o.oprcode::pg_catalog.oid FROM pg_catalog.pg_operator o WHERE o.oid OPERATOR(pg_catalog.=) d.refobjid)))) \
          THEN 'reads'::pg_catalog.text END FROM pg_catalog.pg_depend d \
      WHERE d.refclassid OPERATOR(pg_catalog.=) ANY (ARRAY[", catalog!("pg_proc"), ", ", catalog!("pg_operator"), ", ", catalog!("pg_opclass"), ", ", catalog!("pg_opfamily"), "]) \
        AND d.refobjid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid AND ( \
        (d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_type"), " AND d.objid OPERATOR(pg_catalog.=) ANY (SELECT dom.oid FROM dom)) \
        OR (d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_attrdef"), " \
          AND d.objid OPERATOR(pg_catalog.=) ANY (SELECT ad.oid FROM pg_catalog.pg_attrdef ad JOIN rel ON ad.adrelid OPERATOR(pg_catalog.=) rel.oid)) \
        OR (d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_constraint"), " AND d.objid OPERATOR(pg_catalog.=) ANY (SELECT cons.oid FROM cons)) \
        OR (d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_class"), " AND d.objid OPERATOR(pg_catalog.=) ANY (SELECT w.oid FROM w \
          UNION SELECT i.indexrelid FROM pg_catalog.pg_index i JOIN w ON i.indrelid OPERATOR(pg_catalog.=) w.oid)) \
        OR (d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_class"), " AND d.refclassid OPERATOR(pg_catalog.=) ANY (ARRAY[", catalog!("pg_opclass"), ", ", catalog!("pg_opfamily"), "]) \
          AND d.objid OPERATOR(pg_catalog.=) ANY (SELECT rel.oid FROM rel UNION SELECT i.indexrelid FROM pg_catalog.pg_index i JOIN rel ON i.indrelid OPERATOR(pg_catalog.=) rel.oid)) \
        OR (d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_policy"), " \
          AND d.objid OPERATOR(pg_catalog.=) ANY (SELECT p.oid FROM pg_catalog.pg_policy p JOIN rel ON p.polrelid OPERATOR(pg_catalog.=) rel.oid)) \
        OR (d.classid OPERATOR(pg_catalog.=) ", catalog!("pg_rewrite"), " \
          AND d.objid OPERATOR(pg_catalog.=) ANY (SELECT q.oid FROM pg_catalog.pg_rewrite q JOIN rel ON q.ev_class OPERATOR(pg_catalog.=) rel.oid))) \
    UNION ALL SELECT 'open', 'column type', t.typname::pg_catalog.text, 'reads' FROM pg_catalog.pg_attribute a JOIN rel ON a.attrelid OPERATOR(pg_catalog.=) rel.oid \
      JOIN pg_catalog.pg_type t ON t.oid OPERATOR(pg_catalog.=) a.atttypid \
      WHERE a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped AND t.oid OPERATOR(pg_catalog.=) ANY (SELECT coded.oid FROM coded) \
    UNION ALL SELECT 'open', 'function', p.proname::pg_catalog.text, 'reads' FROM pg_catalog.pg_proc p \
      WHERE p.oid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid AND p.proname OPERATOR(pg_catalog.=) ANY ($3::pg_catalog.text[]) \
    UNION ALL SELECT 'open', 'type', t.typname::pg_catalog.text, 'reads' FROM pg_catalog.pg_type t \
      WHERE t.typname OPERATOR(pg_catalog.=) ANY ($4::pg_catalog.text[]) AND t.oid OPERATOR(pg_catalog.=) ANY (SELECT coded.oid FROM coded) \
    UNION ALL SELECT 'open', 'cast', pg_catalog.textin(pg_catalog.oidout(c.oid)), \
        CASE WHEN (SELECT p.provolatile FROM pg_catalog.pg_proc p WHERE p.oid OPERATOR(pg_catalog.=) c.castfunc) OPERATOR(pg_catalog.<>) 'v'::pg_catalog.\"char\" THEN 'reads'::pg_catalog.text END FROM pg_catalog.pg_cast c \
      WHERE c.oid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid AND c.castmethod OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\" \
      AND ((c.castsource OPERATOR(pg_catalog.<) '16384'::pg_catalog.oid AND c.casttarget OPERATOR(pg_catalog.<) '16384'::pg_catalog.oid) \
        OR c.castsource OPERATOR(pg_catalog.=) ANY (SELECT present.oid FROM present) OR c.casttarget OPERATOR(pg_catalog.=) ANY (SELECT present.oid FROM present)) \
    UNION ALL SELECT 'name', k.code, k.field, k.name FROM ( \
      SELECT CASE WHEN i.indisexclusion THEN '23P01'::pg_catalog.text ELSE '23505'::pg_catalog.text END AS code, 'constraint'::pg_catalog.text AS field, ic.relname::pg_catalog.text AS name \
        FROM pg_catalog.pg_index i JOIN w ON i.indrelid OPERATOR(pg_catalog.=) w.oid JOIN pg_catalog.pg_class ic ON ic.oid OPERATOR(pg_catalog.=) i.indexrelid \
        WHERE i.indisunique OR i.indisexclusion \
      UNION ALL SELECT CASE WHEN i.indisexclusion THEN '23P01' ELSE '23505' END, 'table', c.relname::pg_catalog.text \
        FROM pg_catalog.pg_index i JOIN w ON i.indrelid OPERATOR(pg_catalog.=) w.oid JOIN pg_catalog.pg_class c ON c.oid OPERATOR(pg_catalog.=) i.indrelid \
        WHERE i.indisunique OR i.indisexclusion \
      UNION ALL SELECT '23P01', 'constraint', ic.relname::pg_catalog.text \
        FROM pg_catalog.pg_index i JOIN w ON i.indrelid OPERATOR(pg_catalog.=) w.oid JOIN pg_catalog.pg_class ic ON ic.oid OPERATOR(pg_catalog.=) i.indexrelid \
        WHERE i.indisunique AND i.indisexclusion \
      UNION ALL SELECT '23P01', 'table', c.relname::pg_catalog.text \
        FROM pg_catalog.pg_index i JOIN w ON i.indrelid OPERATOR(pg_catalog.=) w.oid JOIN pg_catalog.pg_class c ON c.oid OPERATOR(pg_catalog.=) i.indrelid \
        WHERE i.indisunique AND i.indisexclusion \
      UNION ALL SELECT '23502', 'column', a.attname::pg_catalog.text FROM pg_catalog.pg_attribute a JOIN w ON a.attrelid OPERATOR(pg_catalog.=) w.oid \
        WHERE a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped AND a.attnotnull \
      UNION ALL SELECT '23502', 'table', c.relname::pg_catalog.text FROM pg_catalog.pg_class c JOIN w ON c.oid OPERATOR(pg_catalog.=) w.oid \
      UNION ALL SELECT '23502', 'dataType', t.typname::pg_catalog.text FROM pg_catalog.pg_type t JOIN dom ON t.oid OPERATOR(pg_catalog.=) dom.oid WHERE t.typnotnull \
      UNION ALL SELECT f.code, f.field, f.name FROM pg_catalog.pg_constraint k \
        CROSS JOIN LATERAL (VALUES ('23503'::pg_catalog.text, 'constraint'::pg_catalog.text, k.conname::pg_catalog.text), ('23503', 'table', ", relname!("k.conrelid"), "), \
          ('23503', 'table', ", relname!("k.confrelid"), ")) AS f(code, field, name) \
        WHERE k.contype OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\" \
          AND (k.conrelid OPERATOR(pg_catalog.=) ANY (SELECT w.oid FROM w) OR ($5::pg_catalog.bool AND k.confrelid OPERATOR(pg_catalog.=) ANY (SELECT w.oid FROM w))) \
      UNION ALL SELECT f.code, f.field, f.name FROM pg_catalog.pg_constraint k \
        CROSS JOIN LATERAL (VALUES ('23001'::pg_catalog.text, 'constraint'::pg_catalog.text, k.conname::pg_catalog.text), ('23001', 'table', ", relname!("k.conrelid"), "), \
          ('23001', 'table', ", relname!("k.confrelid"), ")) AS f(code, field, name) \
        WHERE $5::pg_catalog.bool AND k.contype OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\" AND k.confrelid OPERATOR(pg_catalog.=) ANY (SELECT w.oid FROM w) \
          AND (k.confdeltype OPERATOR(pg_catalog.=) 'r'::pg_catalog.\"char\" OR k.confupdtype OPERATOR(pg_catalog.=) 'r'::pg_catalog.\"char\") \
      UNION ALL SELECT '23514', 'constraint', k.conname::pg_catalog.text FROM pg_catalog.pg_constraint k \
        WHERE k.contype OPERATOR(pg_catalog.=) 'c'::pg_catalog.\"char\" AND k.oid OPERATOR(pg_catalog.=) ANY (SELECT cons.oid FROM cons) \
      UNION ALL SELECT '23514', 'table', c.relname::pg_catalog.text FROM pg_catalog.pg_class c JOIN w ON c.oid OPERATOR(pg_catalog.=) w.oid \
      UNION ALL SELECT '23514', 'dataType', t.typname::pg_catalog.text FROM pg_catalog.pg_type t JOIN dom ON t.oid OPERATOR(pg_catalog.=) dom.oid \
      UNION ALL SELECT '23P01', 'constraint', k.conname::pg_catalog.text FROM pg_catalog.pg_constraint k JOIN w ON k.conrelid OPERATOR(pg_catalog.=) w.oid \
        WHERE k.contype OPERATOR(pg_catalog.=) 'x'::pg_catalog.\"char\" \
    ) AS k(code, field, name) WHERE k.name IS NOT NULL \
    UNION ALL SELECT 'present', pg_catalog.textin(pg_catalog.oidout(p.oid)), NULL, NULL FROM present p WHERE p.oid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid \
    UNION ALL SELECT DISTINCT 'runs', p.proname::pg_catalog.text, CASE WHEN p.oid OPERATOR(pg_catalog.<) '16384'::pg_catalog.oid THEN 'built in'::pg_catalog.text END, NULL FROM r \
      CROSS JOIN LATERAL (SELECT q.ev_action::pg_catalog.text FROM pg_catalog.pg_rewrite q WHERE q.ev_class OPERATOR(pg_catalog.=) r.oid AND q.rulename OPERATOR(pg_catalog.=) '_RETURN'::pg_catalog.name \
        UNION ALL SELECT y.polqual::pg_catalog.text FROM pg_catalog.pg_policy y WHERE y.polrelid OPERATOR(pg_catalog.=) r.oid) AS t(tree) \
      CROSS JOIN LATERAL pg_catalog.regexp_matches(t.tree, ':(?:funcid|opfuncid|aggfnoid|winfnoid) ([0-9]+)'::pg_catalog.text, 'g'::pg_catalog.text) AS m(found) \
      JOIN pg_catalog.pg_proc p ON p.oid OPERATOR(pg_catalog.=) (m.found[1])::pg_catalog.oid \
      WHERE p.provolatile OPERATOR(pg_catalog.=) 'v'::pg_catalog.\"char\" OR EXISTS (SELECT 1 FROM pg_catalog.pg_aggregate a JOIN pg_catalog.pg_proc f \
        ON f.oid OPERATOR(pg_catalog.=) ANY (ARRAY[a.aggtransfn::pg_catalog.oid, a.aggfinalfn::pg_catalog.oid, a.aggcombinefn::pg_catalog.oid, a.aggmtransfn::pg_catalog.oid, a.aggminvtransfn::pg_catalog.oid, a.aggmfinalfn::pg_catalog.oid]) \
        WHERE a.aggfnoid::pg_catalog.oid OPERATOR(pg_catalog.=) p.oid AND f.provolatile OPERATOR(pg_catalog.=) 'v'::pg_catalog.\"char\")"
);

/// Every operator a user or an extension defined, by name: the only ones [`OPERATORS_QUERY`] can find.
pub const USER_OPERATORS_QUERY: &str = "SELECT DISTINCT o.oprname::pg_catalog.text FROM pg_catalog.pg_operator o WHERE o.oid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid";

/// `$1` is a `text[]` of the operators the statement writes, `$2` an `oid[]` of the types of a user's
/// present, as [`ERRORS_QUERY`] reports them, and `$3` how many: one `('open', 'operator', its name)`
/// per operator of a user's or an extension's it may call, on the rule [`ERRORS_QUERY`] states. Apart
/// from it, since whether one may be called rests on the types present and nothing else of the
/// statement, which few statements differ in.
pub const OPERATORS_QUERY: &str = concat!(
    "WITH present(oid) AS (SELECT ($2::pg_catalog.oid[])[n] FROM pg_catalog.generate_series(1, $3::pg_catalog.int4) AS n) \
    SELECT 'open'::pg_catalog.text, 'operator'::pg_catalog.text, o.oprname::pg_catalog.text, \
        CASE WHEN (SELECT p.provolatile FROM pg_catalog.pg_proc p WHERE p.oid OPERATOR(pg_catalog.=) o.oprcode::pg_catalog.oid) OPERATOR(pg_catalog.<>) 'v'::pg_catalog.\"char\" THEN 'reads'::pg_catalog.text END FROM pg_catalog.pg_operator o \
      WHERE o.oid OPERATOR(pg_catalog.>=) '16384'::pg_catalog.oid AND o.oprname OPERATOR(pg_catalog.=) ANY ($1::pg_catalog.text[]) \
      AND ((o.oprleft OPERATOR(pg_catalog.<) '16384'::pg_catalog.oid OR o.oprleft OPERATOR(pg_catalog.=) ANY (SELECT present.oid FROM present)) \
          AND (o.oprright OPERATOR(pg_catalog.<) '16384'::pg_catalog.oid OR o.oprright OPERATOR(pg_catalog.=) ANY (SELECT present.oid FROM present)) \
        OR o.oprleft OPERATOR(pg_catalog.=) '0'::pg_catalog.oid \
        OR NOT EXISTS (SELECT 1 FROM pg_catalog.pg_operator b WHERE b.oid OPERATOR(pg_catalog.<) '16384'::pg_catalog.oid AND b.oprname OPERATOR(pg_catalog.=) o.oprname \
          AND b.oprleft OPERATOR(pg_catalog.=) b.oprright AND b.oprleft OPERATOR(pg_catalog.=) CASE \
          WHEN o.oprleft OPERATOR(pg_catalog.<) '16384'::pg_catalog.oid OR o.oprleft OPERATOR(pg_catalog.=) ANY (SELECT present.oid FROM present) THEN o.oprleft \
          WHEN o.oprright OPERATOR(pg_catalog.<) '16384'::pg_catalog.oid OR o.oprright OPERATOR(pg_catalog.=) ANY (SELECT present.oid FROM present) THEN o.oprright \
          ELSE 'pg_catalog.text'::pg_catalog.regtype::pg_catalog.oid END))"
);

/// The names of a list, borrowed.
fn names(list: &[String]) -> Vec<&str> {
    list.iter().map(String::as_str).collect()
}

/// What [`errors`] asks of a statement as the analysis rewrites it, or `None` where it calls a
/// function that runs SQL it is given.
struct Request {
    /// [`ERRORS_QUERY`]'s parameters for each thing the statement names, on its own: what it says of
    /// them together is what it says of each, so a thing another statement named reads no more.
    each: Vec<Vec<Value>>,
    /// The operators the statement writes.
    operators: Vec<String>,
}

fn request(rewritten: &str) -> Option<Request> {
    let parsed = analyze::analyze(rewritten).ok()?;
    if parsed.functions.iter().any(|f| RUNS_SQL.contains(&f.as_str()) || f == "ts_stat" || f == "ts_rewrite") {
        return None;
    }
    let none: &[&str] = &[];
    let ask = |written: &[&str], read: &[&str], functions: &[&str], types: &[&str]| {
        vec![json!(written), json!(read), json!(functions), json!(types), json!(!written.is_empty() && parsed.changes_keys), json!(written.len()), json!(read.len())]
    };
    // One asked of nothing, for what holds of every statement.
    let mut each = vec![ask(none, none, none, none)];
    each.extend(names(&parsed.write_set).into_iter().map(|t| ask(&[t], none, none, none)));
    each.extend(names(&parsed.read_set).into_iter().map(|t| ask(none, &[t], none, none)));
    each.extend(names(&parsed.functions).into_iter().map(|f| ask(none, none, &[f], none)));
    each.extend(names(&parsed.types).into_iter().map(|t| ask(none, none, none, &[t])));
    // In a fixed order: the parse gives the tables in none, and a need is named by what it asks.
    each.sort_by_key(|ask| serde_json::to_string(ask).unwrap_or_default());
    each.dedup();
    let mut operators = names(&parsed.operators);
    operators.extend(IMPLICIT_OPERATORS);
    let mut operators: Vec<String> = operators.into_iter().map(String::from).collect();
    operators.sort_unstable();
    operators.dedup();
    Some(Request { each, operators })
}

/// Asks [`ERRORS_QUERY`] in the statement's first round, beside its other needs, since it waits on
/// nothing but the parse: asked only once the analysis is done, it would take a round of its own.
pub fn prefetch(rewritten: &str, host: &Host) {
    if let Some(request) = request(rewritten) {
        let _ = host.query_each(ERRORS_QUERY, request.each, Scope::Statement);
        let _ = host.query(USER_OPERATORS_QUERY, Vec::new(), Scope::Session);
    }
}

/// [`ERRORS_QUERY`]'s rows for a statement, with those of [`OPERATORS_QUERY`] among them: `Some(None)`
/// where a query was refused.
fn catalog_rows(request: Request, host: &Host) -> Lazy<Option<Vec<Vec<Value>>>> {
    let rows = host.query_each(ERRORS_QUERY, request.each, Scope::Statement);
    let defined = host.query(USER_OPERATORS_QUERY, Vec::new(), Scope::Session);
    let (rows, defined) = (rows?, defined?);
    let (Ok(mut rows), Ok(defined)) = (rows, defined) else { return Some(None) };
    let operators: Vec<&String> = request.operators.iter().filter(|o| defined.iter().any(|row| row.first().and_then(Value::as_str) == Some(o.as_str()))).collect();
    if !operators.is_empty() {
        let mut present: Vec<u32> = rows.iter().filter(|row| row.first().and_then(Value::as_str) == Some("present")).filter_map(|row| row.get(1).and_then(Value::as_str)?.parse().ok()).collect();
        present.sort_unstable();
        present.dedup();
        let Ok(opened) = host.query(OPERATORS_QUERY, vec![json!(operators), json!(present), json!(present.len())], Scope::Statement)? else { return Some(None) };
        rows.extend(opened);
    }
    Some(Some(rows))
}

/// The names each error the statement may raise can carry: `Some(None)` where it may run code the
/// server does not define, or names nothing the parse can read.
pub fn errors(analysis: &Analysis, host: &Host) -> Lazy<Option<Errors>> {
    if analysis.kind.is_none() {
        return Some(None);
    }
    let Some(request) = request(&analysis.rewritten.sql) else { return Some(None) };
    Some(catalog_rows(request, host)?.and_then(|rows| narrowed(&rows)))
}

/// The volatile built-ins that write nothing, only answering anew for each call.
const VOLATILE_READS: &[&str] = &["random", "random_normal", "gen_random_uuid", "uuidv4", "uuidv7", "clock_timestamp", "timeofday"];

/// Whether the engine finds nothing in the statement that could write: a SELECT with no data-modifying WITH query,
/// no locking clause and no `INTO`, which calls nothing volatile by name, what a built-in that
/// writes must be (`nextval`, `set_config`, `pg_advisory_lock`), reaches no view or policy that does,
/// and sets off nothing else of a user's or an extension's that is: a default, an operator or a
/// cast. A function is taken at its label, as Postgres asks of whoever writes one: one labelled
/// `stable` or `immutable` that calls a volatile function goes unseen. A type's input and output
/// write nothing.
pub fn read_only(analysis: &Analysis, host: &Host) -> Lazy<bool> {
    if !analysis.diagnostics.is_empty() {
        return Some(false);
    }
    only_reads(&analysis.rewritten.sql, VOLATILE_READS, host)
}

/// Whether a function's body calls nothing volatile, by the same reading, and so could say `stable`:
/// the built-ins that answer anew for each call count here, since a function that calls
/// `clock_timestamp()` is rightly volatile.
pub fn calls_nothing_volatile(body: &str, host: &Host) -> Lazy<bool> {
    only_reads(body, &[], host)
}

fn only_reads(sql: &str, volatile_reads: &[&str], host: &Host) -> Lazy<bool> {
    use pg_query::protobuf::node::Node as N;
    let Ok(parsed) = analyze::analyze(sql) else { return Some(false) };
    let Ok(tree) = crate::stack::parse(sql) else { return Some(false) };
    let Some(N::SelectStmt(select)) = tree.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) else { return Some(false) };
    if parsed.kind != analyze::StatementKind::Query || parsed.has_locking || select.into_clause.is_some() {
        return Some(false);
    }
    let Some(request) = request(sql) else { return Some(false) };
    let rows = catalog_rows(request, host);
    let mut called = names(&parsed.functions);
    called.sort_unstable();
    called.dedup();
    let functions = host.query_each(crate::cardinality::FUNCTIONS_QUERY, called.iter().map(|f| vec![json!([f])]), Scope::Statement);
    let (rows, functions) = (rows?, functions?);
    let (Some(rows), Ok(functions)) = (rows, functions) else { return Some(false) };
    let tagged = |row: &&Vec<Value>, tag: &str| row.first().and_then(Value::as_str) == Some(tag);
    let text = |row: &Vec<Value>, k: usize| row.get(k).and_then(Value::as_str).map(str::to_string);
    let reads = rows.iter().filter(|row| tagged(row, "open")).all(|row| text(row, 3).as_deref() == Some("reads"));
    let reach_nothing = rows.iter().filter(|row| tagged(row, "runs")).all(|row| text(row, 2).as_deref() == Some("built in") && text(row, 1).is_some_and(|f| volatile_reads.contains(&f.as_str())));
    // A user's function of a name in VOLATILE_READS may be the one called.
    let a_users = |f: &str| rows.iter().filter(|row| tagged(row, "open")).any(|row| text(row, 1).as_deref() == Some("function") && text(row, 2).as_deref() == Some(f));
    // Each is in the catalog, none of them volatile.
    let not_volatile = called.iter().all(|f| (volatile_reads.contains(f) && !a_users(f)) || functions.iter().any(|row| row.first().and_then(Value::as_str) == Some(f) && row.get(2).and_then(reylo_engine_core::host::as_bool) == Some(false)));
    Some(reads && reach_nothing && not_volatile)
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
            // A type of a user's the statement's objects hold, which [`OPERATORS_QUERY`] reads.
            Some("present") => {}
            // What [`read_only`] reads: a built-in's errors are the server's own.
            Some("runs") => {}
            _ => return None,
        }
    }
    Some(errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_query_is_asked_alike_every_round() {
        let sql = "with p as (select o.id from orders o join users u on u.id = o.user_id join items i on i.order_id = o.id) \
            select p.id, lower(u.name), abs(r.n) from p left join users u on true left join reviews r on r.id = p.id \
            where r.n > 1 and u.name like 'a%'";
        let first = request(sql).unwrap();
        for _ in 0..32 {
            let again = request(sql).unwrap();
            assert_eq!((again.each, again.operators), (first.each.clone(), first.operators.clone()));
        }
        for table in ["items", "orders", "reviews", "users"] {
            assert!(first.each.iter().any(|ask| ask[1] == json!([table])), "{table}");
        }
        assert_eq!(first.operators, vec!["<", "<=", "<>", "=", ">", ">=", "~~"]);
    }

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
