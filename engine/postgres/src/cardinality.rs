//! Static cardinality: whether a statement returns exactly one row, proven from its shape.
//!
//! A middleware returns exactly one row, and the build refuses one it cannot prove does. An
//! endpoint's `returns:` is checked against the same reading, but only for a contradiction it
//! proves — `returns: one` over `LIMIT 0` — never for a shape it merely cannot see through,
//! because `WHERE id = :id` is unprovable and correct.
//!
//! The proof is small and fail-closed: anything it does not recognise is unproven, and
//! "unproven" never means "proven otherwise". The runtime still counts every middleware's rows,
//! because the database the request runs against is not the one the build read (a function can
//! be recreated as set-returning after the build, and a prepared statement re-plans onto it).
//!
//! A `SELECT` returns exactly one row when all of these hold:
//!
//! 1. It has the shape of one row, which is [`infer::single_row`]'s reading: no set operation, no
//!    `VALUES`, no `GROUP BY` (a row per group, and none over no rows), no `HAVING` (it can drop
//!    the one group), no `LIMIT` or `OFFSET` that can drop the row (`LIMIT 0`, `LIMIT $1`,
//!    `OFFSET 1`; a constant `LIMIT` of one row or more drops nothing).
//! 2. It aggregates, or it has neither a `FROM` nor a `WHERE`, or its `FROM` is one row and it has
//!    no `WHERE`. An aggregate with no `GROUP BY` is one group over any input, empty included
//!    (`SELECT count(*) FROM t WHERE false` and even `SELECT count(*) WHERE false` are one row on
//!    both dialects); whether a target aggregates is [`infer::aggregates`]'s reading. Without
//!    one, a `FROM` is a row per row it matches, and a `WHERE` drops the one row when it is false
//!    (`SELECT 1 WHERE :x = 1`).
//! 3. Nothing at its own level returns a set (Postgres only; see below). `SELECT
//!    generate_series(1, 3)` is three rows, `SELECT unnest('{}'::int[])` none, and an aggregate
//!    beside one is as many rows as the set (`SELECT count(*), generate_series(1, 3) FROM t`).
//!
//! A `FROM` is one row when it is never empty ([`never_empty`]) and at most one row ([`From`]).
//! The items that are never empty: a WITH query or a subquery that is itself exactly one row, a
//! function in `FROM` that returns no set (`FROM now()`, `FROM one_text($1)`; a function that is
//! not `SETOF` returns one row, of NULLs when it answers NULL), and a sequence, which holds one
//! row. A cross join of such items is never empty, and so is an outer join whose preserved side
//! is not; an inner join under a condition can drop the row.
//!
//! A `WITH` changes none of this: the main statement reads a CTE only through a `FROM` or a
//! subquery. A subquery answers one value to the level above it whatever it returns: `EXISTS`
//! is one boolean, and a scalar subquery with two rows is an error on Postgres and its first row
//! on SQLite, never two outer rows.
//!
//! # Set-returning calls on Postgres
//!
//! In a select list, Postgres multiplies a row only by evaluating a set-returning function or
//! operator, and the parser only produces one from a call or an operator spelled in the
//! statement: a cast function must not return a set (`CREATE CAST` refuses one), and `CASE`,
//! `COALESCE`, `IS TRUE`, `IN (…)`, `WHERE`, `HAVING`, `LIMIT` and a window definition refuse
//! one outright. But one nested anywhere else in an expression at this level still multiplies the
//! row — under a function (`abs(generate_series(-1, 1))`), a cast, `IS NOT NULL`, `IS DISTINCT
//! FROM`, `NULLIF`'s argument — and so does one in `ORDER BY` or `DISTINCT ON`, which the planner
//! evaluates with the select list. `CREATE OPERATOR` over a `SETOF` function makes an operator
//! that multiplies rows exactly as the function does.
//!
//! So the proof reads every function name and every operator name at this level — the select
//! list, `ORDER BY`, `DISTINCT ON`, window definitions, and each subquery's tested expression
//! (`x IN (SELECT …)`), but not a subquery's own statement — and looks each up in the catalog by
//! name alone, across every overload and schema ([`Catalog`]). The resolver can only pick an
//! overload of a name it was given, so reading all of them is a superset of whatever it picks. A
//! name the catalog does not have is unproven too.
//!
//! SQLite has no set-returning function a select list may call — `json_each` and
//! `generate_series` are table-valued, live in `FROM`, and are "no such function" in a select
//! list — and no way to define an operator, so there the rule is empty.
//!
//! SQLite statements are read with libpg_query, as `infer` and `jsonshape` read them, after their
//! `?n` placeholders become `$n`. Clause structure is the SQL both dialects share, and SQLite's
//! own syntax — `LIMIT 5, 0`, `[bracketed]` names — does not parse there, which proves nothing.

use crate::dialect::Dialect;
use crate::infer::{self, Trust};
use crate::resolution::{Kind, Operand};
use crate::jsonshape::last_string;
use crate::levels::{Columns, Level, Withs};
use crate::pgtypes::oid;
use pg_query::protobuf::{self, node::Node as NodeEnum, SelectStmt};
use serde_json::Value;
use std::collections::HashSet;

/// The shapes the build proves return exactly one row, for the diagnostic that refuses a
/// middleware it cannot prove.
pub const PROVABLE: &str = "a SELECT with no FROM and no WHERE, such as `SELECT EXISTS (SELECT 1 FROM org_members WHERE org_id = :org_id AND user_id = :auth.user_id) AS authorized` or `SELECT (SELECT role FROM org_members WHERE org_id = :org_id AND user_id = :auth.user_id) AS role`; or a SELECT whose select list calls an aggregate (count, sum, avg, min, max) outside a subquery and without OVER, such as `SELECT count(*) > 0 AS authorized FROM org_members WHERE org_id = :org_id AND user_id = :auth.user_id`. Either may start with a WITH. Neither may have GROUP BY, HAVING, LIMIT, OFFSET, UNION, INTERSECT, EXCEPT, or VALUES, or call a set-returning function such as generate_series or unnest outside a subquery";

/// The catalog facts the proof needs and the parse tree cannot carry, all by name. The host
/// answers the three queries below and the orchestration fills this in.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    /// Function names with at least one entry in `pg_proc`.
    pub functions: HashSet<String>,
    /// Function names with at least one set-returning overload (`proretset`), in any schema.
    pub set_returning: HashSet<String>,
    /// Operator names some set-returning function implements, in any schema.
    pub set_returning_operators: HashSet<String>,
    /// Per relation, the column lists of its unique keys: every unique, valid, non-partial index
    /// over plain columns. This is the fact [`at_most_one`] rests on, and it is a fact about the
    /// database rather than about the statement — adding an index makes a read provably single
    /// and dropping one makes it plural again, which is the inference doing its job.
    pub unique_keys: std::collections::HashMap<String, Vec<Vec<String>>>,
    /// Per relation, the unique keys that hold only among the rows a predicate admits (a partial
    /// unique index, `… (email) WHERE deleted_at IS NULL`), with the predicate as the server
    /// prints it. One bounds a read only where the WHERE restricts the relation to such rows by
    /// conjuncts of its own ([`From`]).
    pub partial_keys: std::collections::HashMap<String, Vec<(Vec<String>, String)>>,
    /// Per relation as named in `unique_keys`, the type oid of each of its columns (a domain's by
    /// its base type). A key column is equated by a value of a type its index's operator family
    /// compares without converting the column (see `equality_bounds`), and a column on the other
    /// side of the equality has the type given here; a relation missing here was read by a host
    /// that does not report types, and its keys take any value, as before.
    pub column_types: std::collections::HashMap<String, std::collections::HashMap<String, u32>>,
    /// Per relation as named in `unique_keys`, its columns under a nondeterministic collation,
    /// whose equality is not identity: `'a' = 'A'` there, and a function of the column can tell
    /// the two apart.
    pub nondeterministic: std::collections::HashMap<String, HashSet<String>>,
    /// The relations, as named in `unique_keys`, that hold exactly one row whatever is written: a
    /// sequence (`SELECT last_value FROM my_sequence`).
    pub single_row: HashSet<String>,
    /// Function names with a volatile overload (`provolatile = 'v'`). A volatile call is evaluated
    /// for each row, so `id = random_id()` is no value a key can be pinned to.
    pub volatile: HashSet<String>,
    /// Operator names a user or an extension defined, as the trust check reports them: such an
    /// operator may be the one a comparison or an arithmetic resolves to, and may be volatile.
    pub user_operators: HashSet<String>,
    /// The built-in type names no user's cast function targets, as the trust check reports them,
    /// or `None` where the host did not ask. A cast to another type may call such a function,
    /// which may be volatile.
    pub cast_targets: Option<HashSet<String>>,
    /// Every alias the whole statement gives a relation, to its name as `unique_keys` keys it, for
    /// a subquery proven on its own whose equality reads a column of an enclosing query
    /// (`u.id = p.author_id`): that column's type is the enclosing relation's. An alias the
    /// statement gives two relations is absent.
    pub outer_aliases: std::collections::HashMap<String, String>,
    /// The statement's parameter types in `$n` order, from its describe. Empty when the host did
    /// not describe it, and then an uncast parameter is taken to have the column's type, which is
    /// what the server infers for `col = $1`.
    pub param_types: Vec<u32>,
    /// Whether a user or an extension defined an operator named `=`, which may then be the one a
    /// comparison of other types than the key column's resolves to.
    pub equality_shadowed: bool,
    /// What the trust check says of the overloads of the names a user also defined, which says
    /// for which operand types such an `=` still resolves to the built-in.
    pub overloads: Option<crate::resolution::Shared>,
    /// Per relation as named in `column_types`, its columns whose type is a domain, by name.
    pub domains: std::collections::HashMap<String, HashSet<String>>,
    /// The base types of every such column: a column of one of these types, as `column_types`
    /// gives it, may be one of a domain.
    pub domain_bases: HashSet<u32>,
    /// Per relation as named in `column_types`, each column's declared type, a domain's by itself.
    pub declared_types: std::collections::HashMap<String, std::collections::HashMap<String, u32>>,
    /// Per relation, the events (`TRIGGER_ROW_INSERT`, `_UPDATE`, `_DELETE` as bits) for which it,
    /// or a partition or an inheritance child of it, has an enabled row trigger that runs BEFORE
    /// the change or INSTEAD of it. Such a trigger can return NULL and skip the row, so an INSERT
    /// of one row returns at most one.
    pub skipping_triggers: std::collections::HashMap<String, i32>,
    /// Per relation, the events (the same bits) a rewrite rule replaces or adds actions to, and
    /// for a view every event it has no INSTEAD OF trigger for, as it hands such a write to a table
    /// whose triggers and rules this catalog did not read. RETURNING then gives any number of rows.
    pub rewritten: std::collections::HashMap<String, i32>,
    /// The names of the WITH queries of the statements enclosing the one being proven, which a
    /// subquery, a WITH query's body or a set operation's arm is proven on its own as: such a
    /// name read there unqualified is that WITH query, never the table of the same name.
    pub enclosing_ctes: Vec<String>,
    /// Those WITH queries in the order they are in scope, so that a reference to one is read
    /// through its statement: each name, its SELECT as the parser deparses it, and the column names
    /// its column list gives, if any. One the engine does not read (a recursive one, a write, one
    /// that does not deparse) has no statement, and still hides an outer one of its name.
    pub enclosing_bodies: Vec<(String, Option<String>, Vec<String>)>,
    /// Whether the statement being proven is a subquery of another one, whose columns it may read
    /// (a `SUBQ` site of the framework, proven on its own). An aggregate over such columns alone
    /// belongs to that statement and leaves the subquery a row per row of its FROM, so a nested
    /// statement is one row by an aggregate only where [`crate::levels`] finds the aggregate its
    /// own, from `columns`. A host that sets this lists the enclosing statement's WITH queries in
    /// `enclosing_ctes` ([`with_queries`]).
    pub nested: bool,
    /// Every column of each relation, in order, as [`COLUMNS_QUERY`] reports them: what says
    /// which query a bare column name in a nested statement belongs to.
    pub columns: Columns,
    /// The columns the rows the statement being proven builds may have ([`infer::row_fields`]), set
    /// where the proof starts from the statement's text; `None` for a statement nested in one the
    /// proof does not see, and for a view's defining statement.
    pub row_fields: Option<HashSet<String>>,
    /// Per view, as named in `unique_keys`, its defining SELECT as `pg_get_viewdef` prints it:
    /// read as a subquery, it gives the view the keys its statement keeps ([`output_keys`]). The
    /// tables it reads are among the relations the rest of this catalog covers.
    pub views: std::collections::HashMap<String, String>,
}

impl Catalog {
    /// This catalog for a view's defining statement, which sees none of the WITH queries of the
    /// statement that reads the view.
    fn for_view(&self) -> Catalog {
        Catalog { enclosing_ctes: Vec::new(), enclosing_bodies: Vec::new(), nested: false, row_fields: None, ..self.clone() }
    }

    /// This catalog for a statement nested in one whose WITH queries are `names`.
    fn enclosed(&self, names: &[String]) -> Catalog {
        let mut inner = self.clone();
        inner.enclosing_ctes.extend(names.iter().cloned());
        inner
    }

    /// [`Catalog::enclosed`] for a statement nested in one whose WITH queries are `names`, with the
    /// statements of the first `upto` of `with`'s queries (every one, for `None`) in scope, so
    /// that a reference to one reads its keys. A WITH query's body sees those before it.
    fn scoped(&self, names: &[String], with: Option<&protobuf::WithClause>, upto: Option<usize>) -> Catalog {
        let mut inner = self.enclosed(names);
        let recursive = with.is_some_and(|w| w.recursive);
        for cte in with.into_iter().flat_map(|w| w.ctes.iter().take(upto.unwrap_or(usize::MAX))) {
            let Some(NodeEnum::CommonTableExpr(cte)) = cte.node.as_ref() else { continue };
            let text = match cte.ctequery.as_deref().and_then(|q| q.node.as_ref()) {
                Some(NodeEnum::SelectStmt(body)) if !recursive => deparse_select(body),
                _ => None,
            };
            inner.enclosing_bodies.push((cte.ctename.clone(), text, cte.aliascolnames.iter().filter_map(field_name).map(String::from).collect()));
        }
        inner
    }
}

/// A SELECT as text, deparsed by the library that parsed it.
fn deparse_select(select: &SelectStmt) -> Option<String> {
    let version = crate::stack::parse("SELECT 1").ok()?.protobuf.version;
    let tree = protobuf::ParseResult { version, stmts: vec![protobuf::RawStmt { stmt: Some(Box::new(protobuf::Node { node: Some(NodeEnum::SelectStmt(Box::new(select.clone()))) })), stmt_location: 0, stmt_len: 0 }] };
    crate::stack::deparse(&tree).ok()
}

/// `f` of the statement of the WITH query of an enclosing statement named `name`, under the catalog
/// it is read in (the WITH queries before it in scope), with its column list; `None` where none is
/// in scope.
fn enclosing_body<T>(catalog: &Catalog, name: &str, f: impl FnOnce(&SelectStmt, &Catalog, &[String]) -> T) -> Option<T> {
    let at = catalog.enclosing_bodies.iter().rposition(|(n, _, _)| n == name)?;
    let (_, text, columns) = &catalog.enclosing_bodies[at];
    let text = text.as_deref()?;
    let mut scoped = catalog.clone();
    scoped.enclosing_bodies.truncate(at);
    view_select(text, |body| f(body, &scoped, columns))
}

/// The names of a SELECT's own WITH queries.
fn with_names(s: &SelectStmt) -> Vec<String> {
    s.with_clause.iter().flat_map(|w| w.ctes.iter().filter_map(cte_name).cloned()).collect()
}

/// The oid of the relation named in the text column `$name`, as the server resolves the name the
/// statement writes: through the search path when unqualified, and each part quoted, so a name
/// that needs quotes (`"Users"`) resolves as written.
macro_rules! relation_named {
    ($name:literal) => {
        concat!(
            "pg_catalog.to_regclass(CASE WHEN pg_catalog.strpos(", $name, ", '.'::pg_catalog.text) OPERATOR(pg_catalog.>) 0 \
      THEN ((pg_catalog.quote_ident(pg_catalog.split_part(", $name, ", '.'::pg_catalog.text, 1)) OPERATOR(pg_catalog.||) '.'::pg_catalog.text) \
        OPERATOR(pg_catalog.||) pg_catalog.quote_ident(pg_catalog.split_part(", $name, ", '.'::pg_catalog.text, 2))) \
      ELSE pg_catalog.quote_ident(", $name, ") END)::pg_catalog.oid"
        )
    };
}

/// The names in `$1`, a `text[]` of `$2` of them, one row each in the column `name`, as a
/// `FROM` item: read by subscript, since `unnest` takes any array and so has overloads it could
/// lose to.
macro_rules! names {
    ($names:literal, $count:literal, $alias:literal) => {
        concat!("(SELECT (", $names, ")[n] AS name FROM pg_catalog.generate_series(1, ", $count, ") AS n) AS ", $alias)
    };
}
pub(crate) use names;

/// The key parts of index `i`, `k` from 0 (`indkey` and `indcollation` count from 0), each with
/// its `pg_attribute` row `a`: the index's own for an expression part, else the table's column.
macro_rules! key_parts {
    () => {
        "FROM pg_catalog.generate_series(0, i.indnkeyatts::pg_catalog.int4 OPERATOR(pg_catalog.-) 1) AS k \
      JOIN pg_catalog.pg_attribute a ON CASE WHEN i.indkey[k] OPERATOR(pg_catalog.=) 0 \
        THEN a.attrelid OPERATOR(pg_catalog.=) i.indexrelid AND a.attnum OPERATOR(pg_catalog.=) (k OPERATOR(pg_catalog.+) 1) \
        ELSE a.attrelid OPERATOR(pg_catalog.=) i.indrelid AND a.attnum OPERATOR(pg_catalog.=) i.indkey[k] AND NOT a.attisdropped END"
    };
}

/// One value per column of relation `c`, in attribute order, as an array; `a` is the column's
/// `pg_attribute` row and `t` its type's `pg_type` row.
macro_rules! relation_columns {
    ($value:literal) => {
        concat!(
            "ARRAY(SELECT ", $value, " FROM pg_catalog.pg_attribute a JOIN pg_catalog.pg_type t ON t.oid OPERATOR(pg_catalog.=) a.atttypid \
      WHERE a.attrelid OPERATOR(pg_catalog.=) c.oid AND a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped ORDER BY a.attnum)"
        )
    };
}

pub(crate) use relation_named;

/// The `pg_trigger.tgtype` bits [`Catalog::skipping_triggers`] reports.
pub const TRIGGER_ROW_INSERT: i32 = 4;
pub const TRIGGER_ROW_DELETE: i32 = 8;
pub const TRIGGER_ROW_UPDATE: i32 = 16;

/// `$1` is `text[]` of relation names as the statement writes them, `$2` how many: one row per relation a write
/// to which may return no row or many, `(the name, the events that may skip the row, the events a
/// rule rewrites)`, each as tgtype bits. An event may skip the row where the relation, or a
/// partition or an inheritance child a row is routed to, has an enabled row-level trigger that
/// runs BEFORE the change or INSTEAD of it (internal triggers, the foreign keys', run AFTER and
/// skip nothing), or is a foreign table, whose wrapper may write nothing. A rule (`pg_rewrite`) on
/// the relation rewrites its event, and so does a view every event it has no INSTEAD OF trigger
/// for, since a write through it reaches a table whose triggers and rules these rows do not cover;
/// an INSTEAD OF trigger returns its own row or none.
pub const TRIGGERS_QUERY: &str = concat!(
    "WITH RECURSIVE rels AS (SELECT c.oid AS root, c.relkind, x.name FROM ",
    names!("$1::pg_catalog.text[]", "$2::pg_catalog.int4", "x"),
    " JOIN pg_catalog.pg_class c ON c.oid OPERATOR(pg_catalog.=) ",
    relation_named!("x.name"),
    "), \
    tree(root, rel) AS (SELECT rels.root, rels.root FROM rels \
      UNION SELECT t.root, i.inhrelid FROM tree t JOIN pg_catalog.pg_inherits i ON i.inhparent OPERATOR(pg_catalog.=) t.rel), \
    found AS (SELECT r.name, \
      (coalesce((SELECT pg_catalog.bit_or(g.tgtype::pg_catalog.int4 OPERATOR(pg_catalog.&) 28) FROM tree t JOIN pg_catalog.pg_trigger g ON g.tgrelid OPERATOR(pg_catalog.=) t.rel \
          WHERE t.root OPERATOR(pg_catalog.=) r.root AND NOT g.tgisinternal AND g.tgenabled OPERATOR(pg_catalog.<>) 'D'::pg_catalog.\"char\" \
            AND (g.tgtype::pg_catalog.int4 OPERATOR(pg_catalog.&) 1) OPERATOR(pg_catalog.=) 1 AND (g.tgtype::pg_catalog.int4 OPERATOR(pg_catalog.&) 66) OPERATOR(pg_catalog.<>) 0), 0) \
        OPERATOR(pg_catalog.|) CASE WHEN EXISTS (SELECT 1 FROM tree t JOIN pg_catalog.pg_class f ON f.oid OPERATOR(pg_catalog.=) t.rel \
          WHERE t.root OPERATOR(pg_catalog.=) r.root AND f.relkind OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\") THEN 28 ELSE 0 END) AS skipping, \
      ((CASE WHEN r.relkind OPERATOR(pg_catalog.=) 'v'::pg_catalog.\"char\" THEN 28 OPERATOR(pg_catalog.&) (OPERATOR(pg_catalog.~) coalesce((SELECT pg_catalog.bit_or(g.tgtype::pg_catalog.int4 OPERATOR(pg_catalog.&) 28) \
          FROM pg_catalog.pg_trigger g WHERE g.tgrelid OPERATOR(pg_catalog.=) r.root AND g.tgenabled OPERATOR(pg_catalog.<>) 'D'::pg_catalog.\"char\" \
            AND (g.tgtype::pg_catalog.int4 OPERATOR(pg_catalog.&) 65) OPERATOR(pg_catalog.=) 65), 0)) ELSE 0 END) \
        OPERATOR(pg_catalog.|) coalesce((SELECT pg_catalog.bit_or(CASE WHEN w.ev_type OPERATOR(pg_catalog.=) '2'::pg_catalog.\"char\" THEN 16 \
            WHEN w.ev_type OPERATOR(pg_catalog.=) '3'::pg_catalog.\"char\" THEN 4 ELSE 8 END) FROM pg_catalog.pg_rewrite w \
          WHERE w.ev_class OPERATOR(pg_catalog.=) r.root AND w.ev_type OPERATOR(pg_catalog.=) ANY ('{2,3,4}'::pg_catalog.\"char\"[]) \
            AND w.ev_enabled OPERATOR(pg_catalog.<>) 'D'::pg_catalog.\"char\"), 0)) AS rewritten \
      FROM rels r) \
    SELECT f.name, f.skipping, f.rewritten FROM found f WHERE f.skipping OPERATOR(pg_catalog.<>) 0 OR f.rewritten OPERATOR(pg_catalog.<>) 0"
);

/// `$1` is `text[]` of relation names as the statement writes them (`users`, `archive.users`), `$2` how many:
/// one row per unique index that bounds what a scan of that relation reads, `(the name, key
/// parts, the type oid of each part that is an expression, the predicate of a partial index)`. A
/// part is a column's name, or an expression as the server prints it, in parentheses:
/// `(lower(email))`. The planner's own conditions (`relation_has_unique_index_ext`): the index is
/// valid and immediate (a DEFERRABLE constraint lets duplicates stand inside a transaction that
/// defers it); it is no exclusion constraint's, which is what PostgreSQL 18 builds for a key
/// `WITHOUT OVERLAPS` and marks unique all the same (`PRIMARY KEY (room, during WITHOUT OVERLAPS)`
/// keeps two rows of one room apart only where their ranges overlap, so the room names no row, and
/// the planner takes no such index for a unique one); each part's operator class compares with the
/// btree equality of its type's default one (`record_image_ops` compares bytes, and holds both
/// `(1.0)` and `(1.00)`, which `=` finds equal, and so may an operator class of a superuser's: a
/// key is pinned by the `=` the comparison resolves to, a built-in one by its type's default
/// equality); the relation is not a traditional
/// inheritance parent, whose scan also reads
/// children the index does not cover (a partitioned table's unique index holds across its
/// partitions); and each part's index collation agrees with the column's on equality (the same
/// collation, or both deterministic), since a comparison under the column's collation is what the
/// WHERE makes. An expression part takes the collation of what it reads, which SQL cannot read
/// back, so it counts only where that is certainly deterministic: an expression of a type with no
/// collation, or one with no COLLATE of its own over columns of deterministic collations only (a
/// deterministic equality is identity, which every collation's equality holds to). A partial
/// index's predicate is the proof's to check. A name resolves as the server resolves it, through
/// the search path when unqualified.
pub const UNIQUE_KEYS_QUERY: &str = concat!(
    "SELECT x.name, \
      ARRAY(SELECT CASE WHEN i.indkey[k] OPERATOR(pg_catalog.=) 0 \
          THEN ('('::pg_catalog.text OPERATOR(pg_catalog.||) pg_catalog.pg_get_indexdef(i.indexrelid, k OPERATOR(pg_catalog.+) 1, true)) OPERATOR(pg_catalog.||) ')'::pg_catalog.text \
          ELSE a.attname::pg_catalog.text END ",
    key_parts!(),
    " ORDER BY k), \
      ARRAY(SELECT CASE WHEN i.indkey[k] OPERATOR(pg_catalog.=) 0 THEN a.atttypid::pg_catalog.int8 END ",
    key_parts!(),
    " ORDER BY k), \
      pg_catalog.pg_get_expr(i.indpred, i.indrelid, true) \
    FROM (SELECT DISTINCT n.name FROM ",
    names!("$1::pg_catalog.text[]", "$2::pg_catalog.int4", "n"),
    ") AS x \
    JOIN pg_catalog.pg_class c ON c.oid OPERATOR(pg_catalog.=) ",
    relation_named!("x.name"),
    " JOIN pg_catalog.pg_index i ON i.indrelid OPERATOR(pg_catalog.=) c.oid \
    WHERE i.indisunique AND NOT i.indisexclusion AND i.indisvalid AND i.indimmediate \
      AND (c.relkind OPERATOR(pg_catalog.=) 'p'::pg_catalog.\"char\" OR NOT EXISTS (SELECT 1 FROM pg_catalog.pg_inherits h WHERE h.inhparent OPERATOR(pg_catalog.=) c.oid)) \
      AND (SELECT pg_catalog.count(*) ",
    key_parts!(),
    ") OPERATOR(pg_catalog.=) i.indnkeyatts \
      AND NOT EXISTS (SELECT 1 FROM pg_catalog.generate_series(0, i.indnkeyatts::pg_catalog.int4 OPERATOR(pg_catalog.-) 1) AS k \
        JOIN pg_catalog.pg_opclass oc ON oc.oid OPERATOR(pg_catalog.=) i.indclass[k] \
        WHERE NOT EXISTS (SELECT 1 FROM pg_catalog.pg_amop m \
          JOIN pg_catalog.pg_opclass dc ON dc.opcmethod OPERATOR(pg_catalog.=) oc.opcmethod AND dc.opcintype OPERATOR(pg_catalog.=) oc.opcintype AND dc.opcdefault \
          JOIN pg_catalog.pg_amop dm ON dm.amopfamily OPERATOR(pg_catalog.=) dc.opcfamily AND dm.amoplefttype OPERATOR(pg_catalog.=) oc.opcintype \
            AND dm.amoprighttype OPERATOR(pg_catalog.=) oc.opcintype AND dm.amopstrategy OPERATOR(pg_catalog.=) 3 \
          WHERE m.amopfamily OPERATOR(pg_catalog.=) oc.opcfamily AND m.amoplefttype OPERATOR(pg_catalog.=) oc.opcintype \
            AND m.amoprighttype OPERATOR(pg_catalog.=) oc.opcintype AND m.amopstrategy OPERATOR(pg_catalog.=) 3 AND m.amopopr OPERATOR(pg_catalog.=) dm.amopopr)) \
      AND NOT EXISTS (SELECT 1 ",
    key_parts!(),
    " WHERE NOT CASE WHEN i.indkey[k] OPERATOR(pg_catalog.=) 0 THEN i.indcollation[k] OPERATOR(pg_catalog.=) '0'::pg_catalog.oid OR ( \
        pg_catalog.pg_get_indexdef(i.indexrelid, k OPERATOR(pg_catalog.+) 1, true) OPERATOR(pg_catalog.!~*) 'collate'::pg_catalog.text \
        AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_depend d \
          JOIN pg_catalog.pg_attribute r ON r.attrelid OPERATOR(pg_catalog.=) d.refobjid AND r.attnum OPERATOR(pg_catalog.=) d.refobjsubid \
          JOIN pg_catalog.pg_collation o ON o.oid OPERATOR(pg_catalog.=) r.attcollation \
          WHERE d.classid OPERATOR(pg_catalog.=) 'pg_catalog.pg_class'::pg_catalog.regclass::pg_catalog.oid AND d.objid OPERATOR(pg_catalog.=) i.indexrelid \
            AND d.refclassid OPERATOR(pg_catalog.=) 'pg_catalog.pg_class'::pg_catalog.regclass::pg_catalog.oid \
            AND d.refobjsubid OPERATOR(pg_catalog.>) 0 AND NOT o.collisdeterministic)) \
      ELSE i.indcollation[k] OPERATOR(pg_catalog.=) a.attcollation OR ( \
        coalesce((SELECT o.collisdeterministic FROM pg_catalog.pg_collation o WHERE o.oid OPERATOR(pg_catalog.=) i.indcollation[k]), true) \
        AND coalesce((SELECT o.collisdeterministic FROM pg_catalog.pg_collation o WHERE o.oid OPERATOR(pg_catalog.=) a.attcollation), true)) END)"
);

/// `$1` is `text[]` of relation names as the statement writes them, `$2` how many: every column of each, `(the
/// name, column names, their type oids, whether each one's collation is deterministic, whether the
/// relation is a sequence, their declared type oids)`, a domain's column by its base type, whose
/// equality it uses, and then by the domain. What the other side of an equality is when it is a
/// column (`u.id = p.author_id`), which relation a bare column belongs to, whether a column's
/// equality is identity, and whether the relation holds exactly one row.
pub const COLUMNS_QUERY: &str = concat!(
    "SELECT x.name, ",
    relation_columns!("a.attname::pg_catalog.text"),
    ", ",
    relation_columns!("(CASE WHEN t.typtype OPERATOR(pg_catalog.=) 'd'::pg_catalog.\"char\" THEN t.typbasetype ELSE a.atttypid END)::pg_catalog.int8"),
    ", ",
    relation_columns!("coalesce((SELECT o.collisdeterministic FROM pg_catalog.pg_collation o WHERE o.oid OPERATOR(pg_catalog.=) a.attcollation), true)"),
    ", c.relkind OPERATOR(pg_catalog.=) 'S'::pg_catalog.\"char\", ",
    relation_columns!("a.atttypid::pg_catalog.int8"),
    " FROM (SELECT DISTINCT n.name FROM ",
    names!("$1::pg_catalog.text[]", "$2::pg_catalog.int4", "n"),
    ") AS x \
    JOIN pg_catalog.pg_class c ON c.oid OPERATOR(pg_catalog.=) ",
    relation_named!("x.name"),
    " WHERE EXISTS (SELECT 1 FROM pg_catalog.pg_attribute a WHERE a.attrelid OPERATOR(pg_catalog.=) c.oid AND a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped)"
);

/// `$1` is `text[]` of function names: `(proname, has a set-returning overload, has a volatile
/// overload)`.
pub const FUNCTIONS_QUERY: &str = "SELECT p.proname::pg_catalog.text, pg_catalog.bool_or(p.proretset), pg_catalog.bool_or(p.provolatile OPERATOR(pg_catalog.=) 'v'::pg_catalog.\"char\") \
    FROM pg_catalog.pg_proc p WHERE p.proname::pg_catalog.text OPERATOR(pg_catalog.=) ANY ($1::pg_catalog.text[]) GROUP BY p.proname";

/// Every operator some set-returning function implements.
pub const SET_RETURNING_OPERATORS_QUERY: &str = "SELECT DISTINCT o.oprname::pg_catalog.text FROM pg_catalog.pg_operator o \
    JOIN pg_catalog.pg_proc p ON p.oid OPERATOR(pg_catalog.=) o.oprcode::pg_catalog.oid WHERE p.proretset";

/// Prove that one rewritten statement (placeholders already `$n` or `?n`) returns exactly one
/// row, or say why the build cannot, worded to finish a sentence: "a WHERE with no FROM can
/// remove the row". `trust` is the describer's ([`crate::describe::Describer::trust`]); `catalog`
/// is read on Postgres only.
pub fn exactly_one(sql: &str, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Result<(), String> {
    let s = one_select(sql, dialect)?;
    exactly_one_select(&s, dialect, trust, &with_fields(&s, catalog))
}

/// The catalog for a proof that starts from a statement's text, with the columns the statement's
/// own rows may have; a nested statement's are its enclosing statement's too, which the proof does
/// not see.
fn with_fields(s: &SelectStmt, catalog: &Catalog) -> Catalog {
    let row_fields = (!catalog.nested).then(|| infer::row_fields(&serde_json::to_value(s).unwrap_or_default()));
    Catalog { row_fields, ..catalog.clone() }
}

fn exactly_one_select(s: &SelectStmt, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Result<(), String> {
    if (s.limit_count.is_some() || s.limit_offset.is_some()) && limit_keeps_a_row(s) {
        return exactly_one_select(&unlimited(s), dialect, trust, catalog);
    }
    if !infer::single_row(s) {
        return Err(not_one_row_shaped(s).to_string());
    }
    if s.into_clause.is_some() {
        return Err("SELECT INTO creates a table and returns no rows".into());
    }
    if dialect == Dialect::Postgres {
        let at_this_level = s.target_list.iter().chain(&s.sort_clause).chain(&s.distinct_clause).chain(&s.window_clause).chain(&s.group_clause);
        if let Some(why) = calls_at_this_level(at_this_level).set_returning(catalog) {
            return Err(why);
        }
    }
    // Inside another statement, only an aggregate that is this query's own makes it one row.
    let level = catalog.nested.then(|| {
        let withs = Withs::unknown(catalog.enclosing_ctes.iter().cloned()).with(s.with_clause.as_ref(), &catalog.columns);
        Level::of(&s.from_clause, &withs, &catalog.columns)
    });
    // The type of a column an aggregate's argument reads, which says it is no row.
    let from = From::read(s, catalog);
    let aggregates = s.target_list.iter().any(|target| match target.node.as_ref() {
        Some(NodeEnum::ResTarget(rt)) => rt.val.as_deref().is_some_and(|val| infer::aggregates(val, dialect, trust, level.as_ref(), catalog.row_fields.as_ref(), &|n| from.node_type(n, catalog))),
        _ => false,
    });
    if aggregates {
        return Ok(());
    }
    if s.from_clause.is_empty() {
        return match s.where_clause {
            Some(_) => Err("a WHERE with no FROM can remove the row".into()),
            None => Ok(()),
        };
    }
    // A FROM of exactly one row: never empty, and one row at most.
    for item in &s.from_clause {
        never_empty(item, s, dialect, trust, catalog)?;
    }
    if s.where_clause.is_some() {
        return Err("a WHERE over a FROM of one row can remove its row".into());
    }
    // `FOR UPDATE SKIP LOCKED` drops a row another transaction holds, a table's under an outer
    // join's nullable side too, and its preserved row with it.
    if !s.locking_clause.is_empty() {
        return Err("a locking clause can skip a locked row".into());
    }
    one_row(s, dialect, trust, catalog)
}

/// Why a FROM item, with what it joins, may return no row; `Ok` when it returns at least one row
/// on every database state. An item of exactly one row is never empty: a WITH query or a subquery
/// the proof shows to be one (a LATERAL one is one row per row before it), a function that returns
/// no set ([`one_row_function`]), a sequence ([`Catalog::single_row`]). A cross join is never
/// empty when neither side is, and an outer join when its preserved side is not, whatever its
/// condition; an inner join under a condition that can fail is empty when it does.
fn never_empty(node: &protobuf::Node, s: &SelectStmt, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Result<(), String> {
    const ROWS: &str = "a FROM returns a row per row it matches, and only an aggregate with no GROUP BY collapses them to one";
    match node.node.as_ref() {
        Some(NodeEnum::RangeVar(rv)) => {
            // A schema-qualified name is the table, never the WITH query of that name.
            let cte = s.with_clause.as_ref().filter(|_| rv.schemaname.is_empty()).and_then(|w| {
                w.ctes.iter().find_map(|c| match c.node.as_ref() {
                    Some(NodeEnum::CommonTableExpr(c)) if c.ctename == rv.relname => Some((c, w.recursive)),
                    _ => None,
                })
            });
            match cte {
                Some((cte, false)) => {
                    let enclosed = catalog.enclosed(&with_names(s));
                    let one = match cte.ctequery.as_deref().and_then(|q| q.node.as_ref()) {
                        Some(NodeEnum::SelectStmt(inner)) => exactly_one_select(inner, dialect, trust, &enclosed),
                        Some(body @ (NodeEnum::InsertStmt(_) | NodeEnum::UpdateStmt(_) | NodeEnum::DeleteStmt(_))) => match dml_node(body, &enclosed) {
                            Some(Ok(Cardinality::One)) => Ok(()),
                            Some(Ok(Cardinality::AtMostOne)) => Err("the write may change no row".to_string()),
                            Some(Err(why)) => Err(why),
                            None => Err(ROWS.to_string()),
                        },
                        _ => Err(ROWS.to_string()),
                    };
                    one.map_err(|why| format!("the WITH query `{}` may return no row, or several: {why}", rv.relname))
                }
                Some((_, true)) => Err(format!("the WITH query `{}` is recursive, which returns any number of rows", rv.relname)),
                None if rv.schemaname.is_empty() && catalog.enclosing_ctes.contains(&rv.relname) => Err(format!("`{}` is a WITH query of an enclosing statement, which returns any number of rows", rv.relname)),
                None if catalog.single_row.contains(&catalog_name(rv)) => Ok(()),
                None => Err(ROWS.into()),
            }
        }
        Some(NodeEnum::RangeSubselect(r)) => match r.subquery.as_deref().and_then(|n| n.node.as_ref()) {
            Some(NodeEnum::SelectStmt(sub)) => exactly_one_select(sub, dialect, trust, &catalog.enclosed(&with_names(s)))
                .map_err(|why| format!("the subquery `{}` may return no row, or several: {why}", r.alias.as_ref().map_or("", |a| a.aliasname.as_str()))),
            _ => Err(ROWS.into()),
        },
        Some(NodeEnum::RangeFunction(f)) => one_row_function(f, catalog),
        Some(NodeEnum::JoinExpr(j)) => {
            use protobuf::JoinType;
            let side = |side: Option<&protobuf::Node>| side.map_or(Err(ROWS.to_string()), |n| never_empty(n, s, dialect, trust, catalog));
            if j.jointype == JoinType::JoinInner as i32 {
                side(j.larg.as_deref())?;
                side(j.rarg.as_deref())?;
                if j.is_natural || !j.using_clause.is_empty() || !j.quals.as_deref().is_none_or(|q| always_true(q, catalog)) {
                    return Err("an inner join under a condition returns no row where the condition fails".into());
                }
                Ok(())
            } else if j.jointype == JoinType::JoinLeft as i32 {
                side(j.larg.as_deref())
            } else if j.jointype == JoinType::JoinRight as i32 {
                side(j.rarg.as_deref())
            } else {
                Err("a FULL JOIN returns a row for each side that matches nothing, so two rows where each side has one".into())
            }
        }
        _ => Err(ROWS.into()),
    }
}

/// Whether a join condition holds of every pair of rows: the constant `true`, or an integer
/// literal equated to itself (`ON 1 = 1`) where `=` can only be the built-in one.
fn always_true(quals: &protobuf::Node, catalog: &Catalog) -> bool {
    let integer = |n: &protobuf::Node| match n.node.as_ref() {
        Some(NodeEnum::AConst(c)) if !c.isnull => match &c.val {
            Some(protobuf::a_const::Val::Ival(i)) => Some(i.ival),
            _ => None,
        },
        _ => None,
    };
    match quals.node.as_ref() {
        Some(NodeEnum::AConst(c)) => !c.isnull && matches!(&c.val, Some(protobuf::a_const::Val::Boolval(b)) if b.boolval),
        Some(NodeEnum::AExpr(e)) if is_equality(e) && e.name.len() == 1 && catalog.equality(Operand::Is(oid::INT4), Operand::Is(oid::INT4)) => match (e.lexpr.as_deref().and_then(integer), e.rexpr.as_deref().and_then(integer)) {
            (Some(l), Some(r)) => l == r,
            _ => false,
        },
        _ => false,
    }
}

/// Why a function item of a FROM may return another number of rows than one; `Ok` when every
/// function in it (one, or the several of `ROWS FROM (…)`) returns no set, with or without `WITH
/// ORDINALITY`. Such a function returns exactly one row: its value, a row of NULLs for a NULL
/// composite, or an error. The server refuses a set-returning call anywhere but at the top of the
/// item ("set-returning functions must appear at top level of FROM"), so the name the item calls
/// decides, read as the select list's calls are: across every overload and schema, and unproven
/// when the catalog does not have it. `CURRENT_DATE` and its kin are one value.
fn one_row_function(f: &protobuf::RangeFunction, catalog: &Catalog) -> Result<(), String> {
    for function in &f.functions {
        let call = match function.node.as_ref() {
            Some(NodeEnum::List(l)) => l.items.first().and_then(|n| n.node.as_ref()),
            _ => None,
        };
        match call {
            Some(NodeEnum::SqlvalueFunction(_)) => {}
            Some(NodeEnum::FuncCall(c)) => {
                let Some(name) = c.funcname.last().and_then(field_name) else { return Err("a function in the FROM the engine cannot name returns any number of rows".into()) };
                if catalog.set_returning.contains(name) {
                    return Err(format!("the set-returning function {name} in the FROM returns any number of rows"));
                }
                if !catalog.functions.contains(name) {
                    return Err(format!("the function {name} is not in the catalog, so the build cannot rule out that it returns a set"));
                }
            }
            _ => return Err("a table expression in the FROM the engine does not read returns any number of rows".into()),
        }
    }
    if f.functions.is_empty() {
        return Err("a table expression in the FROM the engine does not read returns any number of rows".into());
    }
    Ok(())
}

/// Whether a statement's LIMIT and OFFSET keep every row of a result of one row: a constant
/// `LIMIT` of one row or more, or `LIMIT ALL` (which `LIMIT NULL` is too), and no `OFFSET` but a
/// constant 0. A parameter may be 0 or NULL, and `LIMIT 0` and `OFFSET 1` drop the row.
fn limit_keeps_a_row(s: &SelectStmt) -> bool {
    let constant = |n: Option<&protobuf::Node>| match n.and_then(|n| n.node.as_ref()) {
        Some(NodeEnum::AConst(c)) => Some(match &c.val {
            _ if c.isnull => None,
            Some(protobuf::a_const::Val::Ival(i)) => Some(i.ival),
            _ => return None,
        }),
        _ => None,
    };
    let count = match s.limit_count.as_deref() {
        None => true,
        some => matches!(constant(some), Some(None) | Some(Some(1..))),
    };
    let offset = match s.limit_offset.as_deref() {
        None => true,
        some => matches!(constant(some), Some(None) | Some(Some(0))),
    };
    count && offset
}

/// A statement without its LIMIT and its OFFSET, which only ever remove rows: what it returns
/// with them is a subset of what it returns without.
fn unlimited(s: &SelectStmt) -> SelectStmt {
    SelectStmt { limit_count: None, limit_offset: None, limit_option: protobuf::LimitOption::Default as i32, ..s.clone() }
}

/// Prove that one rewritten statement returns **at most** one row — no row, or one — or say why
/// the build cannot, worded to finish a sentence the same way [`exactly_one`]'s does.
///
/// This is the weaker reading, and it is the one a nested `SUBQ` takes its shape from: at most one
/// row is an object or null, and anything else is a list. Three things prove it, in the order the
/// build tries them:
///
/// 1. A constant `LIMIT 0` or `LIMIT 1`, which caps the count whatever the rest of the statement
///    does — a set operation and a set-returning call included, because the limit applies last.
/// 2. Exactly one row, which is at most one row. That covers `SELECT count(*) …` and the shapes
///    with no `FROM` at all.
/// 3. A unique key of the one relation the statement reads, every column of it equated to
///    something that is not a column of that relation. Two rows cannot share a unique key, and an
///    equality never matches NULL, so at most one row survives — which holds whether or not the
///    key's columns are `NOT NULL`, since a unique index only lets NULLs repeat.
///
/// Any other LIMIT, and an OFFSET, only ever remove rows: the second and the third are read off
/// the statement without them, so `WHERE id = $1 LIMIT 5` is still at most one row.
///
/// The third is why an index changes an inferred type. The proof reads the index that exists at
/// build time; `reylo.lock` and the boot re-describe turn a later `CREATE`/`DROP INDEX` into a
/// contract mismatch the deploy reports, rather than a shape that quietly drifts.
pub fn at_most_one(sql: &str, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Result<(), String> {
    crate::stack::run(|| at_most_one_here(sql, dialect, trust, catalog))
}

fn at_most_one_here(sql: &str, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Result<(), String> {
    let s = one_select(sql, dialect)?;
    at_most_one_select(&s, dialect, trust, &with_fields(&s, catalog))
}

fn at_most_one_select(s: &SelectStmt, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Result<(), String> {
    // A negative LIMIT is an error on Postgres and no limit at all on SQLite.
    if const_limit(s).is_some_and(|n| (0..=1).contains(&n)) {
        return Ok(());
    }
    // Anything else is a LIMIT above one, or one written as an expression the build cannot read:
    // like an OFFSET it only ever removes rows, so the bound is the one the statement has
    // without them.
    if s.limit_count.is_some() || s.limit_offset.is_some() {
        return at_most_one_select(&unlimited(s), dialect, trust, catalog);
    }
    if exactly_one_select(s, dialect, trust, catalog).is_ok() {
        return Ok(());
    }
    // An INTERSECT returns only rows both arms return, and an EXCEPT only rows of its left arm,
    // with ALL or without: at most one row when an arm it draws every row from is.
    if let (Some(left), Some(right)) = (s.larg.as_deref(), s.rarg.as_deref()) {
        let inner = catalog.enclosed(&with_names(s));
        let arm = |a: &SelectStmt| at_most_one_select(a, dialect, trust, &inner).is_ok();
        if s.op == protobuf::SetOperation::SetopIntersect as i32 && (arm(left) || arm(right)) {
            return Ok(());
        }
        if s.op == protobuf::SetOperation::SetopExcept as i32 && arm(left) {
            return Ok(());
        }
    }
    let grouped = !s.group_clause.is_empty() && s.larg.is_none() && s.rarg.is_none() && s.values_lists.is_empty() && s.into_clause.is_none();
    if let Some(why) = not_key_shaped(s) {
        // A GROUP BY has its own proof below; every other shape ends here.
        if !grouped {
            return Err(why.to_string());
        }
    }
    if dialect == Dialect::Postgres {
        let at_this_level = s.target_list.iter().chain(&s.sort_clause).chain(&s.distinct_clause).chain(&s.window_clause).chain(&s.group_clause);
        if let Some(why) = calls_at_this_level(at_this_level).set_returning(catalog) {
            return Err(why);
        }
    }
    if grouped {
        return one_group(s, dialect, trust, catalog);
    }
    // A WITH query of any number of rows may still be pinned to one by a key of its own.
    match cte_passthrough(s, dialect, trust, catalog) {
        Some(Ok(_)) => Ok(()),
        Some(Err(why)) => one_row(s, dialect, trust, catalog).map_err(|_| why),
        None => one_row(s, dialect, trust, catalog),
    }
}

/// `WITH x AS (…) SELECT … FROM x [WHERE …]`: the rows are the WITH query's, at most. The WITH
/// query is an INSERT, UPDATE or DELETE with RETURNING, proven by shape, or a SELECT, proven
/// like any other. `None` when the statement is not that shape.
fn cte_passthrough(s: &SelectStmt, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Option<Result<Cardinality, String>> {
    let [only] = s.from_clause.as_slice() else { return None };
    let Some(NodeEnum::RangeVar(rv)) = only.node.as_ref() else { return None };
    // A schema-qualified name is the table, never the WITH query of that name.
    if !rv.schemaname.is_empty() {
        return None;
    }
    let with = s.with_clause.as_ref()?;
    let cte = with.ctes.iter().find(|c| cte_name(c) == Some(&rv.relname))?;
    let Some(NodeEnum::CommonTableExpr(cte)) = cte.node.as_ref() else { return None };
    if with.recursive || !s.group_clause.is_empty() || s.having_clause.is_some() || s.larg.is_some() || s.rarg.is_some() || !s.distinct_clause.is_empty() {
        return None;
    }
    if dialect == Dialect::Postgres {
        let at_this_level = s.target_list.iter().chain(&s.sort_clause).chain(&s.distinct_clause).chain(&s.window_clause).chain(&s.group_clause);
        if let Some(why) = calls_at_this_level(at_this_level).set_returning(catalog) {
            return Some(Err(why));
        }
    }
    let body = cte.ctequery.as_deref()?.node.as_ref()?;
    let enclosed = catalog.enclosed(&with_names(s));
    let inner = match body {
        NodeEnum::SelectStmt(inner) => match exactly_one_select(inner, dialect, trust, &enclosed) {
            Ok(()) => Ok(Cardinality::One),
            Err(why) => match at_most_one_select(inner, dialect, trust, &enclosed) {
                Ok(()) => Ok(Cardinality::AtMostOne),
                Err(_) => Err(format!("the WITH query `{}` returns any number of rows: {why}", rv.relname)),
            },
        },
        NodeEnum::InsertStmt(_) | NodeEnum::UpdateStmt(_) | NodeEnum::DeleteStmt(_) => match dml_node(body, catalog) {
            Some(r) => r.map_err(|why| format!("the WITH query `{}` returns any number of rows: {why}", rv.relname)),
            None => return None,
        },
        _ => return None,
    };
    Some(match inner {
        Ok(Cardinality::One) if s.where_clause.is_some() => Ok(Cardinality::AtMostOne),
        other => other,
    })
}

/// The grouped form of the proof: one group at most when every input row carries the same value
/// of every `GROUP BY` expression ([`From::pinned`]), or when every FROM item is determined and so
/// there is one input row at most. What else is joined does not matter: it can multiply the input
/// rows, never the groups. `HAVING` only drops the group. This is `SELECT u.*, count(p.id) FROM
/// users u LEFT JOIN posts p ON … WHERE u.id = $1 GROUP BY u.id`, and `SELECT author_id, count(*)
/// FROM posts WHERE author_id = $1 GROUP BY author_id`.
fn one_group(s: &SelectStmt, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Result<(), String> {
    const WHY: &str = "GROUP BY returns one row per group; the engine proves one group only when every GROUP BY expression is pinned to one value: a column of a relation whose unique key the WHERE pins, or a column the WHERE equates to a value";
    if s.group_clause.iter().any(|g| matches!(g.node.as_ref(), Some(NodeEnum::GroupingSet(_)))) {
        return Err("GROUPING SETS, ROLLUP and CUBE return a row per grouping set".into());
    }
    let from = From::read(s, catalog);
    if from.refused.is_some() {
        return Err(WHY.into());
    }
    if from.determined(false, dialect, trust, catalog).iter().all(|d| *d) {
        return Ok(());
    }
    let constant = from.determined(true, dialect, trust, catalog);
    let pinned = |g: &protobuf::Node| match g.node.as_ref() {
        // `GROUP BY 2` groups by the second output column.
        Some(NodeEnum::AConst(c)) => match &c.val {
            Some(protobuf::a_const::Val::Ival(i)) => output(s, i.ival).is_some_and(|e| from.pinned(e, &constant, catalog)),
            _ => false,
        },
        _ => from.pinned(g, &constant, catalog),
    };
    if s.group_clause.iter().all(pinned) {
        Ok(())
    } else {
        Err(WHY.into())
    }
}

/// The expression of output column `n`, counting from one.
fn output(s: &SelectStmt, n: i32) -> Option<&protobuf::Node> {
    match s.target_list.get(usize::try_from(n).ok()?.checked_sub(1)?)?.node.as_ref()? {
        NodeEnum::ResTarget(r) => r.val.as_deref(),
        _ => None,
    }
}

/// One SELECT's FROM, flattened out of its joins: the items, and the equalities that may pin a
/// unique key of one of them.
///
/// Call an item **determined** when every row of the FROM carries the same row of it, or that
/// row's null extension. When every item is, the FROM has one row at most: a join of items of one
/// row each is one row, and so is a LEFT JOIN of one row to an item determined against it. The
/// closure that finds them (the planner's `innerrel_is_unique` and Calcite's unique keys, in
/// composition):
///
/// 1. A WHERE conjunct `r.k = v` for each column of a unique key of relation `r`, where `v` reads
///    no column of `r` and only columns of determined items (or values), determines `r`, in any
///    join: the `=` rejects a null-extended row.
/// 2. The same equalities in the ON clause of an inner join determine either side, and of a LEFT
///    JOIN its right side (RIGHT JOIN mirrors it): each row of the other side meets one row of the
///    item at most, or its null extension. A FULL JOIN proves nothing, since it returns a row for
///    each side that matches nothing.
/// 3. A WITH query or a subquery the proof shows to be at most one row is determined.
/// 4. An item of one row at most per row of the rest is determined once every item that is not
///    such a one is: a LATERAL subquery the proof bounds, a function in FROM that returns no set
///    ([`one_row_function`]), a sequence ([`Catalog::single_row`]). A join with one multiplies no
///    row. What such an item holds is computed anew for each row it is joined to (a volatile
///    call, a sequence another session advances), so it pins no other item, and under `constant`
///    only a LATERAL subquery counts, once every other item does.
///
/// Each equality must also be the key's own equality ([`equality_bounds`]), and `v` must call
/// nothing volatile, which is evaluated anew for each row.
struct From<'a> {
    items: Vec<Item<'a>>,
    conjuncts: Vec<Conjunct<'a>>,
    /// The names of the statement's WITH queries, which a subquery's FROM may read too.
    ctes: Vec<String>,
    /// The column names a USING join merges, and whether a NATURAL join merges any: a bare such
    /// name reads the merged column, which is of a type common to both sides and neither one's.
    using: Vec<String>,
    natural: bool,
    /// Why no closure holds over this FROM at all.
    refused: Option<&'static str>,
    /// The statement's own WITH queries, whose statements a subquery or a WITH query reads.
    with: Option<&'a protobuf::WithClause>,
}

/// One FROM item.
struct Item<'a> {
    /// The name its columns go by: the alias, or the relation's own name.
    alias: String,
    source: Source<'a>,
    /// Whether an outer join can null-extend it: the right side of a LEFT JOIN, the left of a
    /// RIGHT JOIN, inside either.
    nullable: bool,
    /// Whether its alias renames its columns (`users u (a, b)`), so a column's name is not the
    /// catalog's.
    renamed: bool,
    /// The names its alias gives its columns, by position.
    columns: Vec<String>,
}

enum Source<'a> {
    /// A table or a view.
    Relation(&'a protobuf::RangeVar),
    /// A WITH query read by name, and whether the WITH is RECURSIVE.
    With(&'a protobuf::CommonTableExpr, bool),
    /// A subquery, and whether it is LATERAL.
    Subquery(&'a SelectStmt, bool),
    /// A WITH query of an enclosing statement, read by name: the proof does not read its body.
    Enclosing(String),
    /// A function, or the several of `ROWS FROM (…)`.
    Function(&'a protobuf::RangeFunction),
    /// `TABLESAMPLE`, `XMLTABLE`, `JSON_TABLE`: nothing the proof reads.
    Opaque,
}

/// A conjunct of the WHERE or of an ON clause.
struct Conjunct<'a> {
    node: &'a protobuf::Node,
    /// For an ON clause, the items it may pin, by index: either side of an inner join, the right
    /// side of a LEFT JOIN, the left of a RIGHT JOIN. `None` for the WHERE, which may pin any.
    may_pin: Option<Vec<usize>>,
    /// Whether it holds of every row the FROM returns: the WHERE's, and an inner join's ON.
    inner: bool,
}

/// The conjuncts of a condition's top-level `AND`.
fn conjuncts_of(node: &protobuf::Node) -> Vec<&protobuf::Node> {
    match node.node.as_ref() {
        Some(NodeEnum::BoolExpr(b)) if b.boolop == protobuf::BoolExprType::AndExpr as i32 => b.args.iter().flat_map(conjuncts_of).collect(),
        _ => vec![node],
    }
}

impl<'a> From<'a> {
    fn read(s: &'a SelectStmt, catalog: &Catalog) -> From<'a> {
        let mut ctes = with_names(s);
        ctes.extend(catalog.enclosing_ctes.iter().cloned());
        let mut from = From { items: Vec::new(), conjuncts: Vec::new(), ctes, using: Vec::new(), natural: false, refused: None, with: s.with_clause.as_ref() };
        for item in &s.from_clause {
            from.flatten(item, s.with_clause.as_ref(), false);
        }
        if let Some(w) = s.where_clause.as_deref() {
            for node in conjuncts_of(w) {
                from.conjuncts.push(Conjunct { node, may_pin: None, inner: true });
            }
        }
        from
    }

    /// One relation alone, as an UPDATE or a DELETE reads its target, with the WHERE's conjuncts.
    fn single(rv: &'a protobuf::RangeVar, where_clause: Option<&'a protobuf::Node>, with: Option<&'a protobuf::WithClause>) -> From<'a> {
        let ctes = with.iter().flat_map(|w| w.ctes.iter().filter_map(cte_name).cloned()).collect();
        let (alias, renamed) = rv.alias.as_ref().map_or_else(|| (rv.relname.clone(), false), |a| (a.aliasname.clone(), !a.colnames.is_empty()));
        let item = Item { alias, source: Source::Relation(rv), nullable: false, renamed, columns: Vec::new() };
        let conjuncts = where_clause.into_iter().flat_map(conjuncts_of).map(|node| Conjunct { node, may_pin: None, inner: true }).collect();
        From { items: vec![item], conjuncts, ctes, using: Vec::new(), natural: false, refused: None, with }
    }

    fn flatten(&mut self, node: &'a protobuf::Node, with: Option<&'a protobuf::WithClause>, nullable: bool) {
        let named = |a: &protobuf::Alias| (a.aliasname.clone(), !a.colnames.is_empty());
        let columns = |a: Option<&protobuf::Alias>| a.map(|a| a.colnames.iter().filter_map(field_name).map(String::from).collect()).unwrap_or_default();
        match node.node.as_ref() {
            Some(NodeEnum::RangeVar(rv)) => {
                let (alias, renamed) = rv.alias.as_ref().map(named).unwrap_or_else(|| (rv.relname.clone(), false));
                let cte = with.filter(|_| rv.schemaname.is_empty()).and_then(|w| {
                    w.ctes.iter().find_map(|c| match c.node.as_ref() {
                        Some(NodeEnum::CommonTableExpr(c)) if c.ctename == rv.relname => Some((c, w.recursive)),
                        _ => None,
                    })
                });
                let source = match cte {
                    Some((c, recursive)) => Source::With(c, recursive),
                    None if rv.schemaname.is_empty() && self.ctes.contains(&rv.relname) => Source::Enclosing(rv.relname.clone()),
                    None => Source::Relation(rv),
                };
                self.items.push(Item { alias, source, nullable, renamed, columns: columns(rv.alias.as_ref()) });
            }
            Some(NodeEnum::RangeSubselect(r)) => {
                let (alias, renamed) = r.alias.as_ref().map(named).unwrap_or_default();
                let source = match r.subquery.as_deref().and_then(|n| n.node.as_ref()) {
                    Some(NodeEnum::SelectStmt(sub)) => Source::Subquery(sub, r.lateral),
                    _ => Source::Opaque,
                };
                self.items.push(Item { alias, source, nullable, renamed, columns: columns(r.alias.as_ref()) });
            }
            Some(NodeEnum::JoinExpr(j)) => {
                use protobuf::JoinType;
                if j.alias.is_some() {
                    self.refused = Some("a join with an alias of its own hides the names of the relations it joins");
                }
                let kind = j.jointype;
                self.natural |= j.is_natural;
                self.using.extend(j.using_clause.iter().filter_map(|n| field_name(n).map(String::from)));
                let (left, right) = if kind == JoinType::JoinLeft as i32 {
                    (nullable, true)
                } else if kind == JoinType::JoinRight as i32 {
                    (true, nullable)
                } else if kind == JoinType::JoinInner as i32 {
                    (nullable, nullable)
                } else {
                    self.refused = Some("a FULL JOIN returns a row for each side that matches nothing, so two rows where each side has one");
                    (true, true)
                };
                let start = self.items.len();
                if let Some(l) = j.larg.as_deref() {
                    self.flatten(l, with, left);
                }
                let middle = self.items.len();
                if let Some(r) = j.rarg.as_deref() {
                    self.flatten(r, with, right);
                }
                let end = self.items.len();
                // USING and NATURAL equate columns the parse tree does not spell out, so they
                // pin nothing here; the WHERE still may.
                if let (Some(quals), true, false) = (j.quals.as_deref(), j.using_clause.is_empty(), j.is_natural) {
                    let (may_pin, inner): (Vec<usize>, bool) = if kind == JoinType::JoinInner as i32 {
                        ((start..end).collect(), true)
                    } else if kind == JoinType::JoinLeft as i32 {
                        ((middle..end).collect(), false)
                    } else if kind == JoinType::JoinRight as i32 {
                        ((start..middle).collect(), false)
                    } else {
                        (Vec::new(), false)
                    };
                    for node in conjuncts_of(quals) {
                        self.conjuncts.push(Conjunct { node, may_pin: Some(may_pin.clone()), inner });
                    }
                }
            }
            other => {
                // A function goes by its alias, or by its own name; anything else without an alias
                // could be read under a name the proof does not know.
                let alias = match other {
                    Some(NodeEnum::RangeFunction(f)) => f.alias.as_ref().map(|a| a.aliasname.clone()).or_else(|| infer::function_item_name(f)),
                    Some(NodeEnum::RangeTableSample(t)) => match t.relation.as_deref().and_then(|r| r.node.as_ref()) {
                        Some(NodeEnum::RangeVar(rv)) => Some(rv.alias.as_ref().map_or_else(|| rv.relname.clone(), |a| a.aliasname.clone())),
                        _ => None,
                    },
                    Some(NodeEnum::RangeTableFunc(f)) => f.alias.as_ref().map(|a| a.aliasname.clone()),
                    _ => None,
                };
                let source = match other {
                    Some(NodeEnum::RangeFunction(f)) => Source::Function(f),
                    _ => Source::Opaque,
                };
                match alias {
                    Some(alias) => self.items.push(Item { alias, source, nullable, renamed: true, columns: Vec::new() }),
                    None => self.refused = Some("a FROM item the engine cannot name"),
                }
            }
        }
    }

    /// Which items are determined, by index: the closure in the type's docs. With `constant`,
    /// only items no row null-extends count, pinned by the WHERE or by inner joins: every row of
    /// the FROM then carries the same row of each, which is what grouping and DISTINCT ask.
    fn determined(&self, constant: bool, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Vec<bool> {
        let keys = self.derived_keys(dialect, trust, catalog);
        self.determined_from(vec![false; self.items.len()], &keys, constant, dialect, trust, catalog)
    }

    /// The keys of each subquery, WITH query and view, by item, read from its statement
    /// ([`output_keys`]): they pin it as a unique index pins a table. None for a table.
    fn derived_keys(&self, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Vec<Vec<Key>> {
        (0..self.items.len())
            .map(|i| match &self.items[i].source {
                Source::Relation(rv) if !catalog.views.contains_key(&catalog_name(rv)) => Vec::new(),
                _ => self.item_keys(i, dialect, trust, catalog),
            })
            .collect()
    }

    /// [`From::determined`], with the items `done` names taken as determined from the start: with
    /// one item alone, whether every row of it meets one row at most of each other item. `keys`
    /// are [`From::derived_keys`].
    #[allow(clippy::too_many_arguments)]
    fn determined_from(&self, mut done: Vec<bool>, keys: &[Vec<Key>], constant: bool, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Vec<bool> {
        let n = self.items.len();
        // The items of one row at most per row of the rest (rule 4).
        let per_row: Vec<bool> = self
            .items
            .iter()
            .map(|item| match &item.source {
                Source::Relation(rv) => catalog.single_row.contains(&catalog_name(rv)),
                Source::Subquery(_, lateral) => *lateral,
                Source::Function(_) => true,
                _ => false,
            })
            .collect();
        loop {
            let mut changed = false;
            for i in 0..n {
                if done[i] {
                    continue;
                }
                let item = &self.items[i];
                let rest = || (0..n).all(|j| j == i || done[j] || (!constant && per_row[j]));
                let pinned = match &item.source {
                    Source::Relation(_) if per_row[i] => !constant && rest(),
                    Source::Relation(rv) => self.key_pinned(i, rv, &done, constant, catalog) || self.derived_pinned(i, &keys[i], &done, constant, catalog),
                    _ if constant && item.nullable => false,
                    Source::With(cte, recursive) => !*recursive && (with_query_at_most_one(cte, dialect, trust, &catalog.enclosed(&self.ctes)) || self.derived_pinned(i, &keys[i], &done, constant, catalog)),
                    Source::Subquery(sub, lateral) => (!*lateral || rest()) && (at_most_one_select(sub, dialect, trust, &catalog.enclosed(&self.ctes)).is_ok() || self.derived_pinned(i, &keys[i], &done, constant, catalog)),
                    Source::Function(f) => !constant && rest() && one_row_function(f, catalog).is_ok(),
                    Source::Enclosing(_) => self.derived_pinned(i, &keys[i], &done, constant, catalog),
                    Source::Opaque => false,
                };
                if pinned {
                    done[i] = true;
                    changed = true;
                }
            }
            if !changed {
                return done;
            }
        }
    }

    /// Whether the equalities pin every column of one of `keys`, the keys of subquery or WITH
    /// query `i` ([`output_keys`]), as [`From::key_pinned`] pins a table's.
    fn derived_pinned(&self, i: usize, keys: &[Key], done: &[bool], constant: bool, catalog: &Catalog) -> bool {
        let item = &self.items[i];
        let usable = |c: &Conjunct| match &c.may_pin {
            None => true,
            Some(may) => may.contains(&i) && (!constant || (c.inner && !item.nullable)),
        };
        keys.iter().any(|key| {
            !key.is_empty() && key.iter().all(|part| self.conjuncts.iter().any(|c| usable(c) && self.equates_as(c.node, i, &part.name, &part.relation, &part.column, done, catalog)))
        })
    }

    /// The unique keys of item `i`'s rows, each column by the name the item gives it: a table's
    /// own (a partial one where the WHERE restricts the table to its rows), and a subquery's or a
    /// WITH query's from its statement ([`output_keys`]). A key over an expression is none here.
    fn item_keys(&self, i: usize, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Vec<Key> {
        let item = &self.items[i];
        if item.renamed && matches!(item.source, Source::Relation(_)) {
            return Vec::new();
        }
        match &item.source {
            Source::Relation(rv) if catalog.views.contains_key(&catalog_name(rv)) => view_select(&catalog.views[&catalog_name(rv)], |view| output_keys(view, dialect, trust, &catalog.for_view())).unwrap_or_default(),
            // A key over an expression maps only to a select list that keeps the expression.
            Source::Relation(rv) => {
                let relation = catalog_name(rv);
                let full = catalog.unique_keys.get(&relation).into_iter().flatten();
                let partial = catalog.partial_keys.get(&relation).into_iter().flatten().filter(|(_, p)| self.restricts(i, p)).map(|(k, _)| k);
                full.chain(partial).filter(|k| !k.is_empty()).map(|k| k.iter().map(|c| Part { name: c.clone(), relation: relation.clone(), column: c.clone() }).collect()).collect()
            }
            _ => self.through(i, catalog, |body, scope, columns| renamed_keys(output_keys(body, dialect, trust, scope), body, columns)).unwrap_or_default(),
        }
    }

    /// `f` of the statement of subquery, WITH query or enclosing WITH query `i`, under the catalog
    /// it is read in, with the names its column lists give its columns by position (the WITH
    /// query's own, then the alias's).
    fn through<T>(&self, i: usize, catalog: &Catalog, f: impl FnOnce(&SelectStmt, &Catalog, &[String]) -> T) -> Option<T> {
        let item = &self.items[i];
        let overlay = |inner: &[String]| -> Vec<String> {
            let mut columns = inner.to_vec();
            for (at, name) in item.columns.iter().enumerate() {
                match columns.get_mut(at) {
                    Some(slot) => *slot = name.clone(),
                    None => columns.push(name.clone()),
                }
            }
            columns
        };
        match &item.source {
            Source::Subquery(sub, _) => Some(f(sub, &catalog.scoped(&self.ctes, self.with, None), &item.columns)),
            Source::With(cte, false) => {
                let Some(NodeEnum::SelectStmt(body)) = cte.ctequery.as_deref().and_then(|q| q.node.as_ref()) else { return None };
                let at = self.with.and_then(|w| w.ctes.iter().position(|c| matches!(c.node.as_ref(), Some(NodeEnum::CommonTableExpr(c)) if std::ptr::eq(c.as_ref(), *cte))));
                let own: Vec<String> = cte.aliascolnames.iter().filter_map(field_name).map(String::from).collect();
                Some(f(body, &catalog.scoped(&self.ctes, self.with, Some(at.unwrap_or(0))), &overlay(&own)))
            }
            Source::Enclosing(name) => enclosing_body(catalog, name, |body, scope, own| f(body, scope, &overlay(own))),
            _ => None,
        }
    }

    /// The table column behind column `column` of item `i`, passed through unchanged.
    fn base(&self, i: usize, column: &str, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Option<(String, String)> {
        let item = &self.items[i];
        if item.renamed && matches!(item.source, Source::Relation(_)) {
            return None;
        }
        match &item.source {
            Source::Relation(rv) => match catalog.views.get(&catalog_name(rv)) {
                // A view's column is the table column its statement passes through.
                Some(definition) => view_select(definition, |view| output_base(view, column, dialect, trust, &catalog.for_view())).flatten(),
                None => Some((catalog_name(rv), column.to_string())),
            },
            _ => self.through(i, catalog, |body, scope, columns| inner_name(column, body, columns).and_then(|inner| output_base(body, &inner, dialect, trust, scope))).flatten(),
        }
    }

    /// The item and column a column reference of this level's select list reads: as
    /// [`From::column`] places it, or a bare name over a FROM of one item.
    fn place_column(&self, c: &protobuf::ColumnRef, catalog: &Catalog) -> Option<(usize, String)> {
        if let Some(placed) = self.column(c, catalog) {
            return Some(placed);
        }
        match c.fields.iter().filter_map(field_name).collect::<Vec<_>>().as_slice() {
            // A table's own columns are placed above; a name it lacks is an enclosing query's.
            [column] if self.items.len() == 1 && c.fields.len() == 1 && !self.merged(column) && !matches!(self.items[0].source, Source::Relation(_)) => Some((0, column.to_string())),
            _ => None,
        }
    }

    /// The output column of SELECT `s` that is key part `p` of table `r`, an expression of an index
    /// (`(lower(email))`), where the select list keeps the expression under an alias of its own.
    fn kept_expression(&self, s: &SelectStmt, r: usize, p: &Part) -> Option<Part> {
        let alias = &self.items[r].alias;
        s.target_list.iter().find_map(|t| {
            let Some(NodeEnum::ResTarget(t)) = t.node.as_ref() else { return None };
            let value = t.val.as_deref()?;
            (!t.name.is_empty() && !self.mentions_merged(value) && matches_expression(value, &p.name, alias)).then(|| Part { name: t.name.clone(), relation: p.relation.clone(), column: p.column.clone() })
        })
    }

    /// The output column that stands for key part `p` of table `r`, which the select list leaves
    /// out: a column of another table that an equality holding of every row of the FROM (the
    /// WHERE's, or an inner join's) makes equal to it, of the same type and both compared as
    /// identity, so that equal values of the one are equal values of the other.
    fn equated_output(&self, r: usize, p: &Part, output: &dyn Fn(usize, &str) -> Option<String>, catalog: &Catalog) -> Option<Part> {
        if catalog.equality_shadowed || !matches!(self.items[r].source, Source::Relation(_)) || !identity(catalog, &p.relation, &p.column) {
            return None;
        }
        let alias = &self.items[r].alias;
        let ty = |relation: &str, column: &str| catalog.column_types.get(relation).and_then(|t| t.get(column)).copied();
        self.conjuncts.iter().filter(|c| c.inner).find_map(|c| {
            let Some(NodeEnum::AExpr(e)) = c.node.node.as_ref() else { return None };
            if !is_equality(e) || e.name.len() != 1 {
                return None;
            }
            let (l, rhs) = (e.lexpr.as_deref()?, e.rexpr.as_deref()?);
            [(l, rhs), (rhs, l)].into_iter().find_map(|(side, other)| {
                if self.own(side, alias).as_deref() != Some(p.name.as_str()) {
                    return None;
                }
                let Some(NodeEnum::ColumnRef(oc)) = other.node.as_ref() else { return None };
                let (j, column) = self.column(oc, catalog)?;
                let Source::Relation(rv) = &self.items[j].source else { return None };
                let relation = catalog_name(rv);
                let same = j != r && !self.items[j].renamed && ty(&p.relation, &p.column).is_some() && ty(&p.relation, &p.column) == ty(&relation, &column) && identity(catalog, &relation, &column);
                same.then(|| output(j, &column).map(|name| Part { name, relation, column })).flatten()
            })
        })
    }

    /// Whether the equalities pin every column of a unique key of item `i`.
    fn key_pinned(&self, i: usize, rv: &protobuf::RangeVar, done: &[bool], constant: bool, catalog: &Catalog) -> bool {
        let item = &self.items[i];
        if item.renamed {
            return false;
        }
        let relation = catalog_name(rv);
        let usable = |c: &Conjunct| match &c.may_pin {
            None => true,
            Some(may) => may.contains(&i) && (!constant || (c.inner && !item.nullable)),
        };
        let full = catalog.unique_keys.get(&relation).into_iter().flatten().map(|key| (key, None));
        let partial = catalog.partial_keys.get(&relation).into_iter().flatten().map(|(key, predicate)| (key, Some(predicate.as_str())));
        full.chain(partial).any(|(key, predicate)| {
            !key.is_empty()
                && key.iter().all(|part| self.conjuncts.iter().any(|c| usable(c) && self.equates(c.node, i, &relation, part, done, catalog)))
                && predicate.is_none_or(|p| self.restricts(i, p))
        })
    }

    /// Whether the WHERE restricts item `i` to the rows a partial index's predicate admits: each
    /// conjunct of the predicate is one of the WHERE's own top-level conjuncts, read with `i`'s
    /// name, and `col IS NOT NULL` is also met by an equality on `col`, which no NULL passes. A
    /// conjunct of an ON clause does not count: the index must bound the relation's rows before
    /// any join, as the planner learned (bug #17975). A string literal the server prints with a
    /// cast to text (`status = 'active'::text`) matches the same literal written without one.
    fn restricts(&self, i: usize, predicate: &str) -> bool {
        // A bare name in the WHERE may be a merged column, which is neither side's.
        if self.natural || !self.using.is_empty() {
            return false;
        }
        let Ok(parsed) = crate::stack::parse(&format!("SELECT 1 WHERE {predicate}")) else { return false };
        let Some(NodeEnum::SelectStmt(select)) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) else { return false };
        let Some(wanted) = select.where_clause.as_deref() else { return false };
        let alias = &self.items[i].alias;
        let have: Vec<Value> = self.conjuncts.iter().filter(|k| k.may_pin.is_none()).map(|k| normalized(k.node, alias)).collect();
        conjuncts_of(wanted).into_iter().all(|w| {
            let w = normalized(w, alias);
            have.contains(&w) || not_null_column(&w).is_some_and(|column| have.iter().any(|h| equates_column(h, &column)))
        })
    }

    /// Whether `node` is `column = value` for this column of item `i`, where the value reads no
    /// column of `i` and only columns of `done` items, columns the WHERE pins to a value
    /// ([`From::pinned_by_value`]: `u.id = p.author_id` beside `p.author_id = $1`), or columns of
    /// enclosing queries, and compares under the column's own equality. A bare name for the column is `i`'s: `i` has it, so had
    /// another item it too, the server would have refused the name as ambiguous.
    fn equates(&self, node: &protobuf::Node, i: usize, relation: &str, column: &str, done: &[bool], catalog: &Catalog) -> bool {
        self.equates_as(node, i, column, relation, column, done, catalog)
    }

    /// [`From::equates`] for the column item `i` names `name`, which is `column` of `relation`.
    #[allow(clippy::too_many_arguments)]
    fn equates_as(&self, node: &protobuf::Node, i: usize, name: &str, relation: &str, column: &str, done: &[bool], catalog: &Catalog) -> bool {
        let Some(NodeEnum::AExpr(e)) = node.node.as_ref() else { return false };
        if !is_equality(e) {
            return false;
        }
        let (Some(l), Some(r)) = (e.lexpr.as_deref(), e.rexpr.as_deref()) else { return false };
        let alias = &self.items[i].alias;
        let is_part = |side: &protobuf::Node| match name.strip_prefix('(') {
            Some(_) => !self.mentions_merged(side) && matches_expression(side, name, alias),
            None => self.own(side, alias).as_deref() == Some(name),
        };
        [(l, r), (r, l)].into_iter().any(|(side, value)| {
            is_part(side)
                && self.reads(value, catalog).is_some_and(|read| {
                    read.iter().all(|(j, read)| *j != i && (done[*j] || read.as_deref().is_some_and(|c| self.pinned_by_value(*j, c, catalog))))
                })
                && equality_bounds(catalog, relation, column, value, &|n| self.node_type(n, catalog))
        })
    }

    /// Whether the WHERE equates this column of item `j` to a value that reads no column, so every
    /// row carries the same value of it: `p.author_id = $1`. The same under the column's own
    /// equality is one value only where that equality is identity ([`identity`]): `tag = 'a'`
    /// under a nondeterministic collation admits 'a' and 'A', which `ascii(tag)` tells apart.
    fn pinned_by_value(&self, j: usize, column: &str, catalog: &Catalog) -> bool {
        let item = &self.items[j];
        let Source::Relation(rv) = &item.source else { return false };
        if item.renamed {
            return false;
        }
        let relation = catalog_name(rv);
        if !identity(catalog, &relation, column) {
            return false;
        }
        self.conjuncts.iter().filter(|k| k.may_pin.is_none()).any(|k| {
            let Some(NodeEnum::AExpr(e)) = k.node.node.as_ref() else { return false };
            let (Some(l), Some(r)) = (e.lexpr.as_deref(), e.rexpr.as_deref()) else { return false };
            is_equality(e)
                && [(l, r), (r, l)].into_iter().any(|(side, value)| {
                    self.own(side, &item.alias).as_deref() == Some(column)
                        && self.reads(value, catalog).is_some_and(|read| read.is_empty())
                        && equality_bounds(catalog, &relation, column, value, &|n| self.node_type(n, catalog))
                })
        })
    }

    /// Whether every row of the FROM carries the same value of `expr`: a column of a `constant`
    /// item, a column the WHERE equates to a value under the column's own equality (the one
    /// grouping and DISTINCT use, so a pin through an explicit COLLATE does not count), or an
    /// expression that reads no column, calls nothing volatile and no window function, which
    /// reads other rows than its own (`row_number() OVER ()`).
    fn pinned(&self, expr: &protobuf::Node, constant: &[bool], catalog: &Catalog) -> bool {
        if mentions_window(expr) {
            return false;
        }
        if let Some(NodeEnum::ColumnRef(c)) = expr.node.as_ref() {
            let Some((i, column)) = self.column(c, catalog) else { return false };
            if constant[i] {
                return true;
            }
            let Source::Relation(rv) = &self.items[i].source else { return false };
            let relation = catalog_name(rv);
            return self.conjuncts.iter().any(|k| k.may_pin.is_none() && self.equates(k.node, i, &relation, &column, constant, catalog));
        }
        self.reads(expr, catalog).is_some_and(|read| read.is_empty()) && !calls_volatile(expr, catalog)
    }

    /// The column of the item read as `alias` this expression names ([`own_column`]), unless it is
    /// a bare name a USING or NATURAL join merges, which is neither side's column.
    fn own(&self, node: &protobuf::Node, alias: &str) -> Option<String> {
        let column = own_column(node, alias)?;
        let bare = matches!(node.node.as_ref(), Some(NodeEnum::ColumnRef(c)) if c.fields.len() == 1);
        (!bare || !self.merged(&column)).then_some(column)
    }

    /// Whether a bare name may read a column a USING or NATURAL join merges.
    fn merged(&self, column: &str) -> bool {
        self.natural || self.using.iter().any(|u| u == column)
    }

    /// Whether `node` reads a merged column by a bare name anywhere.
    fn mentions_merged(&self, node: &protobuf::Node) -> bool {
        if !self.natural && self.using.is_empty() {
            return false;
        }
        let Ok(tree) = serde_json::to_value(node) else { return true };
        fn walk(v: &Value, from: &From) -> bool {
            match v {
                Value::Object(map) => map.iter().any(|(key, child)| {
                    let bare = (key == "ColumnRef")
                        .then(|| child.get("fields")?.as_array().filter(|f| f.len() == 1)?.first()?.get("node")?.get("String")?.get("sval")?.as_str())
                        .flatten();
                    bare.is_some_and(|name| from.merged(name)) || walk(child, from)
                }),
                Value::Array(items) => items.iter().any(|i| walk(i, from)),
                _ => false,
            }
        }
        walk(&tree, self)
    }

    /// The item a column belongs to, and its name there: qualified by an item's name, or a bare
    /// name exactly one item certainly has.
    fn column(&self, c: &protobuf::ColumnRef, catalog: &Catalog) -> Option<(usize, String)> {
        let parts: Vec<&str> = c.fields.iter().filter_map(field_name).collect();
        if parts.len() != c.fields.len() {
            return None;
        }
        match parts.as_slice() {
            // A renamed table's columns are not the catalog's; a renamed subquery's go by their new names.
            [q, column] => self.items.iter().position(|it| it.alias == *q && !(it.renamed && matches!(it.source, Source::Relation(_)))).map(|i| (i, column.to_string())),
            [column] if self.merged(column) => None,
            [column] => {
                let mut having = (0..self.items.len()).filter(|i| self.has_column(*i, column, catalog));
                match (having.next(), having.next()) {
                    (Some(i), None) => Some((i, column.to_string())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Whether item `i` certainly has a column of this name.
    fn has_column(&self, i: usize, column: &str, catalog: &Catalog) -> bool {
        let item = &self.items[i];
        let Source::Relation(rv) = &item.source else { return false };
        if item.renamed {
            return false;
        }
        let relation = catalog_name(rv);
        match catalog.column_types.get(&relation) {
            Some(columns) => columns.contains_key(column),
            None => catalog.unique_keys.get(&relation).is_some_and(|keys| keys.iter().flatten().any(|k| k == column)),
        }
    }

    /// Whether every column of item `i` is known, so a name it lacks is certainly not its.
    fn columns_known(&self, i: usize, catalog: &Catalog) -> bool {
        let item = &self.items[i];
        matches!(&item.source, Source::Relation(rv) if !item.renamed && catalog.column_types.contains_key(&catalog_name(rv)))
    }

    /// The type of a column or a scalar subquery on the other side of an equality.
    fn node_type(&self, n: &protobuf::Node, catalog: &Catalog) -> Option<u32> {
        match n.node.as_ref() {
            Some(NodeEnum::ColumnRef(c)) => self.type_of(c, catalog),
            Some(NodeEnum::SubLink(l)) => scalar_type(l, &self.ctes, catalog),
            _ => None,
        }
    }

    /// The type of a column a value reads: of an item here, or of an enclosing query's relation.
    fn type_of(&self, c: &protobuf::ColumnRef, catalog: &Catalog) -> Option<u32> {
        if let Some((i, column)) = self.column(c, catalog) {
            return match &self.items[i].source {
                Source::Relation(rv) => catalog.column_types.get(&catalog_name(rv))?.get(&column).copied(),
                Source::With(cte, _) if cte.aliascolnames.is_empty() => match cte.ctequery.as_deref()?.node.as_ref()? {
                    NodeEnum::SelectStmt(sub) => output_type(sub, &column, &self.ctes, catalog),
                    _ => None,
                },
                Source::Subquery(sub, _) => output_type(sub, &column, &self.ctes, catalog),
                _ => None,
            };
        }
        let local = matches!(c.fields.first().and_then(field_name), Some(q) if self.items.iter().any(|it| it.alias == q));
        if local {
            None
        } else {
            outer_column_type(c, catalog)
        }
    }

    /// The items an expression reads, by index, or `None` when one of its columns cannot be
    /// placed: a bare name more than one relation in scope may hold, or one beside a relation
    /// whose columns the catalog did not give. A column of a subquery's own relations, and one
    /// of an enclosing query (qualified by a name no item here goes by), reads no item.
    fn reads(&self, node: &protobuf::Node, catalog: &Catalog) -> Option<Vec<(usize, Option<String>)>> {
        let tree = serde_json::to_value(node).ok()?;
        let mut out = Vec::new();
        self.walk_reads(&tree, &mut Vec::new(), catalog, &mut out)?;
        Some(out)
    }

    /// `inner` holds the scopes of the subqueries the walk is inside, innermost last: each the
    /// names its FROM goes by, with the relation behind each where it is one the catalog knows.
    fn walk_reads(&self, v: &Value, inner: &mut Vec<Vec<(String, Option<String>)>>, catalog: &Catalog, out: &mut Vec<(usize, Option<String>)>) -> Option<()> {
        match v {
            Value::Object(map) => {
                if let Some(c) = map.get("ColumnRef") {
                    return self.place(c, inner, catalog, out);
                }
                if let Some(select) = map.get("SelectStmt") {
                    inner.push(self.scope_of(select));
                    let walked = select.as_object().into_iter().flat_map(|o| o.values()).try_for_each(|child| self.walk_reads(child, inner, catalog, out));
                    inner.pop();
                    return walked;
                }
                map.values().try_for_each(|child| self.walk_reads(child, inner, catalog, out))
            }
            Value::Array(items) => items.iter().try_for_each(|child| self.walk_reads(child, inner, catalog, out)),
            _ => Some(()),
        }
    }

    /// The names a subquery's FROM goes by, and the catalog relation behind each that is one.
    fn scope_of(&self, select: &Value) -> Vec<(String, Option<String>)> {
        let mut own: Vec<String> = Vec::new();
        for cte in select.get("with_clause").and_then(|w| w.get("ctes")).and_then(Value::as_array).into_iter().flatten() {
            if let Some(name) = cte.get("node").and_then(|n| n.get("CommonTableExpr")).and_then(|c| c.get("ctename")).and_then(Value::as_str) {
                own.push(name.to_string());
            }
        }
        let mut out = Vec::new();
        fn item(v: &Value, ctes: &[String], own: &[String], out: &mut Vec<(String, Option<String>)>) {
            let Some(node) = v.get("node") else { return };
            let alias = |n: &Value| n.get("alias").and_then(|a| a.get("aliasname")).and_then(Value::as_str).map(String::from);
            if let Some(rv) = node.get("RangeVar") {
                let relname = rv.get("relname").and_then(Value::as_str).unwrap_or_default().to_string();
                let schema = rv.get("schemaname").and_then(Value::as_str).unwrap_or_default();
                let renamed = rv.get("alias").and_then(|a| a.get("colnames")).and_then(Value::as_array).is_some_and(|c| !c.is_empty());
                let relation = if schema.is_empty() {
                    (!ctes.contains(&relname) && !own.contains(&relname)).then(|| relname.clone())
                } else {
                    Some(format!("{schema}.{relname}"))
                };
                out.push((alias(rv).unwrap_or(relname), relation.filter(|_| !renamed)));
            } else if let Some(j) = node.get("JoinExpr") {
                for side in ["larg", "rarg"] {
                    if let Some(s) = j.get(side) {
                        item(s, ctes, own, out);
                    }
                }
            } else if let Some(other) = node.as_object().and_then(|o| o.values().next()) {
                out.push((alias(other).unwrap_or_default(), None));
            }
        }
        for f in select.get("from_clause").and_then(Value::as_array).into_iter().flatten() {
            item(f, &self.ctes, &own, &mut out);
        }
        out
    }

    /// Place one column a walk meets, adding the item it reads and the column's name there (`None`
    /// for a whole row, `q.*`) to `out`.
    fn place(&self, c: &Value, inner: &[Vec<(String, Option<String>)>], catalog: &Catalog, out: &mut Vec<(usize, Option<String>)>) -> Option<()> {
        let fields = c.get("fields")?.as_array()?;
        let names: Vec<&str> = fields.iter().filter_map(|f| f.get("node")?.get("String")?.get("sval")?.as_str()).collect();
        match (names.as_slice(), fields.len()) {
            // `q.column`, or `q.*`.
            ([q, rest @ ..], 2) => {
                if inner.iter().rev().any(|scope| scope.iter().any(|(alias, _)| alias == q)) {
                    return Some(());
                }
                if let Some(i) = self.items.iter().position(|it| it.alias == *q) {
                    out.push((i, rest.first().map(|c| c.to_string())));
                }
                Some(())
            }
            ([column], 1) => {
                // A system column (`ctid`) is in no column list, and another value in each row.
                if SYSTEM_COLUMNS.contains(column) || self.merged(column) {
                    return None;
                }
                for scope in inner.iter().rev() {
                    for (_, relation) in scope {
                        let columns = catalog.column_types.get(relation.as_deref()?)?;
                        if columns.contains_key(*column) {
                            return Some(());
                        }
                    }
                }
                let having: Vec<usize> = (0..self.items.len()).filter(|i| self.has_column(*i, column, catalog)).collect();
                match having.as_slice() {
                    [i] => {
                        out.push((*i, Some(column.to_string())));
                        Some(())
                    }
                    // A name no relation in scope has as a column is the whole row of an item
                    // that goes by it, as `(p).author_id` reads `p`: of a subquery the walk is
                    // inside, or of an item here.
                    [] if inner.iter().any(|scope| scope.iter().any(|(alias, _)| alias == column)) => Some(()),
                    [] if self.items.iter().any(|it| it.alias == *column) => {
                        out.extend(self.items.iter().enumerate().filter(|(_, it)| it.alias == *column).map(|(i, _)| (i, None)));
                        Some(())
                    }
                    // Else a column of an enclosing query.
                    [] if (0..self.items.len()).all(|i| self.columns_known(i, catalog)) => Some(()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Why the closure leaves the FROM more than one row, from its first undetermined item.
    fn why_not(&self, determined: &[bool], catalog: &Catalog) -> String {
        // An item of one row per row of the rest only waits for the others: name one of those.
        let waits = |i: &usize| match &self.items[*i].source {
            Source::Relation(rv) => catalog.single_row.contains(&catalog_name(rv)),
            Source::Function(f) => one_row_function(f, catalog).is_ok(),
            _ => false,
        };
        let Some(i) = (0..determined.len()).filter(|i| !determined[*i]).min_by_key(waits) else { return "every relation is pinned to one row".into() };
        let item = &self.items[i];
        match &item.source {
            Source::Relation(rv) => {
                let relation = catalog_name(rv);
                let mut shown: Vec<String> = catalog.unique_keys.get(&relation).into_iter().flatten().map(|k| format!("({})", k.join(", "))).collect();
                shown.extend(catalog.partial_keys.get(&relation).into_iter().flatten().map(|(k, p)| format!("({}) where {p}", k.join(", "))));
                match shown.is_empty() {
                    true => format!("`{relation}` has no unique index the engine can read, so nothing bounds the rows a WHERE matches"),
                    false => {
                        if self.items.len() == 1 {
                            format!("the WHERE does not equate every column of a unique key of `{relation}` — it has {}", shown.join(", "))
                        } else {
                            format!(
                                "a join returns a row per pair it matches, and the WHERE and ON clauses do not equate every column of a unique key of `{}` to values or to columns of relations pinned to one row — it has {}",
                                item.alias,
                                shown.join(", ")
                            )
                        }
                    }
                }
            }
            Source::With(cte, _) => format!("the WITH query `{}` returns any number of rows", cte.ctename),
            Source::Enclosing(_) => format!("`{}` is a WITH query of an enclosing statement, which returns any number of rows", item.alias),
            Source::Subquery(..) => format!("the subquery `{}` returns any number of rows", item.alias),
            Source::Function(_) | Source::Opaque => format!("`{}` in the FROM is a function or table function, which returns any number of rows", item.alias),
        }
    }
}

/// Whether a WITH query returns one row at most: a SELECT the proof bounds, or a write it does.
fn with_query_at_most_one(cte: &protobuf::CommonTableExpr, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> bool {
    match cte.ctequery.as_deref().and_then(|q| q.node.as_ref()) {
        Some(NodeEnum::SelectStmt(inner)) => at_most_one_select(inner, dialect, trust, catalog).is_ok(),
        Some(body @ (NodeEnum::InsertStmt(_) | NodeEnum::UpdateStmt(_) | NodeEnum::DeleteStmt(_))) => matches!(dml_node(body, catalog), Some(Ok(_))),
        _ => false,
    }
}

/// Whether `node` may give each row another value: a call of a function with a volatile overload
/// (`random()`, `nextval`) or of one the catalog does not know, whose volatility it cannot tell;
/// an operator other than a built-in spelling no user defined, which a volatile function may
/// implement (`id = (0 +? 0)`); or a cast that may call a user's cast function.
fn calls_volatile(node: &protobuf::Node, catalog: &Catalog) -> bool {
    let Ok(tree) = serde_json::to_value(node) else { return true };
    fn walk(v: &Value, catalog: &Catalog) -> bool {
        match v {
            Value::Object(map) => map.iter().any(|(key, child)| {
                let volatile = match key.as_str() {
                    "FuncCall" => last_name(child.get("funcname")).is_none_or(|n| !catalog.functions.contains(&n) || catalog.volatile.contains(&n)),
                    "AExpr" => operator_names(child).iter().any(|n| !trusted_operator(n, catalog)),
                    "TypeCast" => user_cast(child, catalog),
                    _ => false,
                };
                volatile || walk(child, catalog)
            }),
            Value::Array(items) => items.iter().any(|i| walk(i, catalog)),
            _ => false,
        }
    }
    walk(&tree, catalog)
}

/// The operators an `A_Expr` applies, by name: its own, or `<=` and `>=` for a BETWEEN.
fn operator_names(aexpr: &Value) -> Vec<String> {
    use protobuf::AExprKind;
    let between = [AExprKind::AexprBetween, AExprKind::AexprNotBetween, AExprKind::AexprBetweenSym, AExprKind::AexprNotBetweenSym].map(|k| k as i64);
    if aexpr.get("kind").and_then(Value::as_i64).is_some_and(|k| between.contains(&k)) {
        return vec!["<=".into(), ">=".into()];
    }
    last_name(aexpr.get("name")).into_iter().collect()
}

/// Whether an operator is one of the built-in spellings the trust check asks about, and no user
/// or extension defined one of that name that is volatile: the built-in operators never are.
fn trusted_operator(name: &str, catalog: &Catalog) -> bool {
    (infer::POSTGRES_OPERATORS.contains(&name) || infer::POSTGRES_STRICT_OPERATORS.contains(&name))
        && (!catalog.user_operators.contains(name) || catalog.overloads.as_ref().and_then(|o| o.0.operator_volatile(name)) == Some(false))
}

/// Whether a cast may call a user's cast function: one to a type outside the built-in targets no
/// such function has, of anything but a string literal or a parameter, which the type's input
/// function reads and the server types as the target.
fn user_cast(cast: &Value, catalog: &Catalog) -> bool {
    let Some(targets) = &catalog.cast_targets else { return false };
    if last_name(cast.get("type_name").and_then(|t| t.get("names"))).is_some_and(|n| targets.contains(&n)) {
        return false;
    }
    let arg = cast.get("arg").and_then(|a| a.get("node"));
    !arg.is_some_and(|a| a.get("ParamRef").is_some() || a.get("AConst").and_then(|c| c.get("val")).is_some_and(|v| v.get("Sval").is_some()))
}

/// The type of a column of an enclosing query, through [`Catalog::outer_aliases`].
fn outer_column_type(c: &protobuf::ColumnRef, catalog: &Catalog) -> Option<u32> {
    let parts: Vec<&str> = c.fields.iter().filter_map(field_name).collect();
    let [q, column] = parts.as_slice() else { return None };
    let relation = catalog.outer_aliases.get(*q)?;
    catalog.column_types.get(relation)?.get(*column).copied()
}

/// A node as JSON with what the comparison of two spellings must ignore: every `location`, the
/// qualifier `alias` on a column (`u.email` is `email` when read for `u`), and a cast to a text
/// type around a string literal, which the server prints for a literal the statement may leave
/// to take the column's type.
fn normalized(node: &protobuf::Node, alias: &str) -> Value {
    fn walk(v: &mut Value, alias: &str) {
        match v {
            Value::Object(map) => {
                map.remove("location");
                if let Some(fields) = map.get_mut("ColumnRef").and_then(|c| c.get_mut("fields")).and_then(Value::as_array_mut) {
                    let qualified = fields.len() == 2 && fields[0].get("node").and_then(|n| n.get("String")).and_then(|s| s.get("sval")).and_then(Value::as_str) == Some(alias);
                    if qualified {
                        fields.remove(0);
                    }
                }
                let literal = map.get("TypeCast").and_then(|t| {
                    let arg = t.get("arg")?.get("node")?;
                    let name = t.get("type_name")?.get("names")?.as_array()?.last()?.get("node")?.get("String")?.get("sval")?.as_str()?;
                    let text = ["text", "varchar", "bpchar"].contains(&name) && arg.get("AConst").and_then(|c| c.get("val")).is_some_and(|v| v.get("Sval").is_some());
                    text.then(|| arg.clone())
                });
                if let Some(arg) = literal {
                    map.remove("TypeCast");
                    if let Value::Object(inner) = arg {
                        map.extend(inner);
                    }
                }
                map.values_mut().for_each(|child| walk(child, alias));
            }
            Value::Array(items) => items.iter_mut().for_each(|child| walk(child, alias)),
            _ => {}
        }
    }
    let mut tree = serde_json::to_value(node).unwrap_or(Value::Null);
    walk(&mut tree, alias);
    tree
}

/// Whether `side` spells the expression of an index key part (`(lower(email))`) for the relation
/// read as `alias`: the same parse tree once [`normalized`].
fn matches_expression(side: &protobuf::Node, part: &str, alias: &str) -> bool {
    let Ok(parsed) = crate::stack::parse(&format!("SELECT {part}")) else { return false };
    let Some(NodeEnum::SelectStmt(select)) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) else { return false };
    let Some(NodeEnum::ResTarget(target)) = select.target_list.first().and_then(|t| t.node.as_ref()) else { return false };
    let Some(expression) = target.val.as_deref() else { return false };
    normalized(side, alias) == normalized(expression, alias)
}

/// The column of a normalized `col IS NOT NULL`.
fn not_null_column(v: &Value) -> Option<Value> {
    let test = v.get("node")?.get("NullTest")?;
    let not_null = test.get("nulltesttype").and_then(Value::as_i64) == Some(protobuf::NullTestType::IsNotNull as i64);
    let arg = test.get("arg")?;
    (not_null && arg.get("node")?.get("ColumnRef").is_some()).then(|| arg.clone())
}

/// Whether a normalized conjunct is `column = …` or `… = column`.
fn equates_column(v: &Value, column: &Value) -> bool {
    let Some(e) = v.get("node").and_then(|n| n.get("AExpr")) else { return false };
    let equality = e.get("name").and_then(Value::as_array).and_then(|n| n.last()).and_then(|n| n.get("node")?.get("String")?.get("sval")?.as_str()) == Some("=")
        && e.get("kind").and_then(Value::as_i64) == Some(protobuf::AExprKind::AexprOp as i64);
    equality && (e.get("lexpr") == Some(column) || e.get("rexpr") == Some(column))
}

/// The one relation a SELECT reads, by the name the catalog keys it under, with the name its
/// columns go by: a plain SELECT over one table or view, not a WITH query (`ctes`).
fn only_relation<'s>(sub: &'s SelectStmt, ctes: &[String]) -> Option<(String, &'s str)> {
    if sub.with_clause.is_some() || sub.larg.is_some() || !sub.values_lists.is_empty() {
        return None;
    }
    let [only] = sub.from_clause.as_slice() else { return None };
    let Some(NodeEnum::RangeVar(rv)) = only.node.as_ref() else { return None };
    if rv.alias.as_ref().is_some_and(|a| !a.colnames.is_empty()) || (rv.schemaname.is_empty() && ctes.contains(&rv.relname)) {
        return None;
    }
    Some((catalog_name(rv), rv.alias.as_ref().map_or(rv.relname.as_str(), |a| a.aliasname.as_str())))
}

/// The type of a column of the one relation `sub` reads, named by `c`: bare, or qualified by the
/// relation's name.
fn projected(sub: &SelectStmt, c: &protobuf::ColumnRef, ctes: &[String], catalog: &Catalog) -> Option<u32> {
    let (relation, alias) = only_relation(sub, ctes)?;
    let parts: Vec<&str> = c.fields.iter().filter_map(field_name).collect();
    let column = match parts.as_slice() {
        [column] => *column,
        [q, column] if *q == alias => *column,
        _ => return None,
    };
    catalog.column_types.get(&relation)?.get(column).copied()
}

/// The type of the output column `name` of a subquery or WITH query over one relation, where the
/// column is one of the relation's, passed through: `*`, or a plain column under its own name or
/// an alias. A grouping or an aggregate changes no plain column's type.
fn output_type(sub: &SelectStmt, name: &str, ctes: &[String], catalog: &Catalog) -> Option<u32> {
    for target in &sub.target_list {
        let Some(NodeEnum::ResTarget(r)) = target.node.as_ref() else { continue };
        let Some(NodeEnum::ColumnRef(c)) = r.val.as_deref().and_then(|v| v.node.as_ref()) else { continue };
        if c.fields.last().is_some_and(|f| matches!(f.node, Some(NodeEnum::AStar(_)))) {
            let (relation, _) = only_relation(sub, ctes)?;
            if let Some(t) = catalog.column_types.get(&relation).and_then(|types| types.get(name)) {
                return Some(*t);
            }
            continue;
        }
        let own = c.fields.iter().filter_map(field_name).last();
        let out = if r.name.is_empty() { own } else { Some(r.name.as_str()) };
        if out == Some(name) {
            return projected(sub, c, ctes, catalog);
        }
    }
    None
}

/// The type of a scalar subquery's one column, where it is a column of the one relation the
/// subquery reads: `(SELECT author_id FROM posts WHERE id = $1)`.
fn scalar_type(l: &protobuf::SubLink, ctes: &[String], catalog: &Catalog) -> Option<u32> {
    if l.sub_link_type != protobuf::SubLinkType::ExprSublink as i32 {
        return None;
    }
    let Some(NodeEnum::SelectStmt(sub)) = l.subselect.as_deref()?.node.as_ref() else { return None };
    let [target] = sub.target_list.as_slice() else { return None };
    let Some(NodeEnum::ResTarget(r)) = target.node.as_ref() else { return None };
    let Some(NodeEnum::ColumnRef(c)) = r.val.as_deref()?.node.as_ref() else { return None };
    projected(sub, c, ctes, catalog)
}

/// The name of every WITH query anywhere in a statement: what a host puts in
/// [`Catalog::enclosing_ctes`] before it proves a subquery of the statement on its own, so that a
/// name the subquery reads is never taken for the table of the same name.
pub fn with_queries(sql: &str) -> Vec<String> {
    crate::stack::run(|| with_queries_here(sql))
}

fn with_queries_here(sql: &str) -> Vec<String> {
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                out.extend(map.get("CommonTableExpr").and_then(|c| c.get("ctename")).and_then(Value::as_str).map(String::from));
                map.values().for_each(|child| walk(child, out));
            }
            Value::Array(items) => items.iter().for_each(|child| walk(child, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Some(tree) = crate::stack::parse(sql).ok().and_then(|parsed| serde_json::to_value(&parsed.protobuf).ok()) {
        walk(&tree, &mut out);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Every alias a statement gives a relation, through every subquery, to the relation's name as
/// [`Catalog::unique_keys`] keys it: what a host puts in [`Catalog::outer_aliases`] before it
/// proves a subquery of the statement on its own. An alias given to two relations is left out.
pub fn aliases(sql: &str) -> std::collections::HashMap<String, String> {
    crate::stack::run(|| aliases_here(sql))
}

fn aliases_here(sql: &str) -> std::collections::HashMap<String, String> {
    let mut out: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
    let Ok(parsed) = crate::stack::parse(sql) else { return Default::default() };
    let Ok(tree) = serde_json::to_value(&parsed.protobuf) else { return Default::default() };
    fn walk(v: &Value, out: &mut std::collections::HashMap<String, Option<String>>) {
        match v {
            Value::Object(map) => {
                if let Some(rv) = map.get("RangeVar") {
                    let relname = rv.get("relname").and_then(Value::as_str).unwrap_or_default().to_string();
                    let schema = rv.get("schemaname").and_then(Value::as_str).unwrap_or_default();
                    let alias = rv.get("alias").and_then(|a| a.get("aliasname")).and_then(Value::as_str).map_or_else(|| relname.clone(), String::from);
                    let relation = if schema.is_empty() { relname } else { format!("{schema}.{relname}") };
                    out.entry(alias).and_modify(|known| {
                        if known.as_deref() != Some(relation.as_str()) {
                            *known = None;
                        }
                    }).or_insert(Some(relation));
                }
                map.values().for_each(|child| walk(child, out));
            }
            Value::Array(items) => items.iter().for_each(|child| walk(child, out)),
            _ => {}
        }
    }
    walk(&tree, &mut out);
    out.into_iter().filter_map(|(alias, relation)| Some((alias, relation?))).collect()
}

/// Which of the key proof's structural conditions a statement breaks, if it breaks one. A shape
/// that returns a row per group or per arm is not a shape a unique key speaks for.
/// One column of a key of a subquery's or a WITH query's rows: its name in their output, and the
/// table column it passes through unchanged, whose type and equality an equated value must suit.
#[derive(Clone, Debug)]
struct Part {
    name: String,
    relation: String,
    column: String,
}

type Key = Vec<Part>;

/// The unique keys of a SELECT's rows: sets of its output columns no two of its rows share a value
/// of every one of, NULL aside, which no equality matches. Read as a table's unique keys are, they
/// pin a subquery or a WITH query to one row ([`From`], rule 1).
///
/// - A unique key of one of its FROM items, every column of it in the select list unchanged, when
///   every other item is determined against that item ([`From::determined_from`]): each of its
///   rows then meets one row at most of the rest, and the FROM is no more rows than it. A WHERE,
///   a HAVING, a LIMIT or an OFFSET only removes rows, and a window function adds none. A LATERAL
///   subquery is no such item: its rows are computed anew for each row of the rest, so two of them
///   may share a key.
/// - The GROUP BY's expressions, each a column in the select list: one row per group. Not under
///   GROUPING SETS, ROLLUP or CUBE, which return a row per grouping set.
/// - Under DISTINCT, the whole select list; under DISTINCT ON, its expressions.
///
/// None at all for a set operation, a VALUES list, or a set-returning call at this level, which
/// repeats a row (`SELECT id, generate_series(1, 2) FROM posts`).
fn output_keys(s: &SelectStmt, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Vec<Key> {
    if let (Some(left), Some(right)) = (s.larg.as_deref(), s.rarg.as_deref()) {
        return set_keys(s, left, right, dialect, trust, &catalog.scoped(&with_names(s), s.with_clause.as_ref(), None));
    }
    if !s.values_lists.is_empty() || s.into_clause.is_some() {
        return Vec::new();
    }
    if s.group_clause.iter().any(|g| matches!(g.node.as_ref(), Some(NodeEnum::GroupingSet(_)))) {
        return Vec::new();
    }
    if dialect == Dialect::Postgres {
        let at_this_level = s.target_list.iter().chain(&s.sort_clause).chain(&s.distinct_clause).chain(&s.window_clause).chain(&s.group_clause);
        if calls_at_this_level(at_this_level).set_returning(catalog).is_some() {
            return Vec::new();
        }
    }
    let from = From::read(s, catalog);
    if from.refused.is_some() {
        return Vec::new();
    }
    // The output name of column `column` of item `i`, where the select list keeps it unchanged.
    let output = |i: usize, column: &str| -> Option<String> {
        s.target_list.iter().find_map(|target| {
            let Some(NodeEnum::ResTarget(r)) = target.node.as_ref() else { return None };
            let Some(NodeEnum::ColumnRef(c)) = r.val.as_deref().and_then(|v| v.node.as_ref()) else { return None };
            let star = c.fields.last().is_some_and(|f| matches!(f.node, Some(NodeEnum::AStar(_))));
            if star {
                // A USING or NATURAL join merges columns under one name.
                let every = c.fields.len() == 1 && !from.natural && from.using.is_empty();
                let its = c.fields.len() == 2 && c.fields.first().and_then(field_name) == Some(from.items[i].alias.as_str());
                return (every || its).then(|| column.to_string());
            }
            let (j, own) = from.place_column(c, catalog)?;
            (j == i && own == column).then(|| if r.name.is_empty() { own } else { r.name.clone() })
        })
    };
    // The output column an expression of GROUP BY or DISTINCT ON is, with the table column behind it.
    let part = |e: &protobuf::Node| -> Option<Part> {
        let (c, name) = match e.node.as_ref()? {
            NodeEnum::ColumnRef(c) => (c, None),
            // `GROUP BY 2` groups by the second output column.
            NodeEnum::AConst(k) => match &k.val {
                Some(protobuf::a_const::Val::Ival(n)) => match s.target_list.get(usize::try_from(n.ival).ok()?.checked_sub(1)?)?.node.as_ref()? {
                    NodeEnum::ResTarget(r) => match r.val.as_deref()?.node.as_ref()? {
                        NodeEnum::ColumnRef(c) => (c, Some(r.name.clone()).filter(|n| !n.is_empty())),
                        _ => return None,
                    },
                    _ => return None,
                },
                _ => return None,
            },
            _ => return None,
        };
        let (i, column) = from.place_column(c, catalog)?;
        let name = match name {
            Some(name) => name,
            None => output(i, &column)?,
        };
        let (relation, column) = from.base(i, &column, dialect, trust, catalog)?;
        Some(Part { name, relation, column })
    };
    let mut keys = Vec::new();
    if !s.group_clause.is_empty() {
        keys.extend(s.group_clause.iter().map(part).collect::<Option<Key>>());
        return keys;
    }
    if !s.distinct_clause.is_empty() {
        let plain = s.distinct_clause.iter().all(|d| d.node.is_none());
        let key: Option<Key> = if plain {
            s.target_list
                .iter()
                .map(|t| match t.node.as_ref()? {
                    NodeEnum::ResTarget(r) => match r.val.as_deref()?.node.as_ref()? {
                        NodeEnum::ColumnRef(c) if !c.fields.last().is_some_and(|f| matches!(f.node, Some(NodeEnum::AStar(_)))) => {
                            let (i, column) = from.place_column(c, catalog)?;
                            let (relation, base) = from.base(i, &column, dialect, trust, catalog)?;
                            Some(Part { name: if r.name.is_empty() { column } else { r.name.clone() }, relation, column: base })
                        }
                        _ => None,
                    },
                    _ => None,
                })
                .collect()
        } else {
            s.distinct_clause.iter().map(part).collect()
        };
        keys.extend(key);
    }
    let n = from.items.len();
    let derived = from.derived_keys(dialect, trust, catalog);
    for r in 0..n {
        if matches!(from.items[r].source, Source::Subquery(_, true)) {
            continue;
        }
        let mut seed = vec![false; n];
        seed[r] = true;
        if !from.determined_from(seed, &derived, false, dialect, trust, catalog).iter().all(|d| *d) {
            continue;
        }
        let own = match &from.items[r].source {
            Source::Relation(rv) if !catalog.views.contains_key(&catalog_name(rv)) => from.item_keys(r, dialect, trust, catalog),
            _ => derived[r].clone(),
        };
        for key in own {
            // A key column the select list leaves out still counts where the WHERE pins it to
            // one value, or where an inner equality keeps a column of the same value in the list.
            let mapped: Option<Key> = key
                .iter()
                .filter_map(|p| match output(r, &p.name) {
                    Some(name) => Some(Some(Part { name, relation: p.relation.clone(), column: p.column.clone() })),
                    None if p.name.starts_with('(') => Some(from.kept_expression(s, r, p)),
                    None if matches!(from.items[r].source, Source::Relation(_)) && from.pinned_by_value(r, &p.name, catalog) => None,
                    None => Some(from.equated_output(r, p, &output, catalog)),
                })
                .collect();
            keys.extend(mapped);
        }
    }
    keys
}

/// The keys of a set operation's rows. Without ALL every operator returns distinct rows, so the
/// whole row is a key, where each column passes one table column through on both sides and the
/// two are of one type, whose equality the row's columns then compare under. INTERSECT returns only
/// rows of its left side, and EXCEPT a subset of them, with ALL or without, so those keep its keys.
/// UNION ALL has none.
fn set_keys(s: &SelectStmt, left: &SelectStmt, right: &SelectStmt, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Vec<Key> {
    use protobuf::SetOperation as Op;
    let mut keys = Vec::new();
    if s.op == Op::SetopIntersect as i32 || s.op == Op::SetopExcept as i32 {
        keys.extend(output_keys(left, dialect, trust, catalog));
    }
    if !s.all {
        let whole = || -> Option<Key> {
            let (names, others) = (output_names(left)?, output_names(right)?);
            if names.len() != others.len() {
                return None;
            }
            names
                .iter()
                .zip(&others)
                .map(|(name, other)| {
                    let (name, other) = (name.as_deref()?, other.as_deref()?);
                    let (relation, column) = output_base(left, name, dialect, trust, catalog)?;
                    let (other_relation, other_column) = output_base(right, other, dialect, trust, catalog)?;
                    let ty = |r: &str, c: &str| catalog.column_types.get(r).and_then(|t| t.get(c)).copied();
                    (ty(&relation, &column).is_some() && ty(&relation, &column) == ty(&other_relation, &other_column)).then(|| Part { name: name.to_string(), relation, column })
                })
                .collect()
        };
        keys.extend(whole());
    }
    keys
}

/// The names of a SELECT's output columns by position: an alias, or a column's own name, and
/// `None` for an entry the engine does not name. `None` for the whole where the list holds `*`,
/// whose width the statement does not say. A set operation's are its first arm's.
fn output_names(s: &SelectStmt) -> Option<Vec<Option<String>>> {
    if let Some(left) = s.larg.as_deref() {
        return output_names(left);
    }
    s.target_list
        .iter()
        .map(|t| {
            let Some(NodeEnum::ResTarget(r)) = t.node.as_ref() else { return Some(None) };
            if !r.name.is_empty() {
                return Some(Some(r.name.clone()));
            }
            match r.val.as_deref().and_then(|v| v.node.as_ref()) {
                Some(NodeEnum::ColumnRef(c)) if c.fields.last().is_some_and(|f| matches!(f.node, Some(NodeEnum::AStar(_)))) => None,
                Some(NodeEnum::ColumnRef(c)) => Some(c.fields.iter().filter_map(field_name).last().map(String::from)),
                _ => Some(None),
            }
        })
        .collect()
}

/// A SELECT's output names after a column list renames them by position.
fn renamed_names(body: &SelectStmt, columns: &[String]) -> Option<Vec<Option<String>>> {
    let mut names = output_names(body)?;
    for (at, name) in columns.iter().enumerate() {
        *names.get_mut(at)? = Some(name.clone());
    }
    Some(names)
}

/// `keys` of a SELECT's rows, each column by the name a column list gives it. A key whose column
/// the statement's own names do not tell apart is dropped.
fn renamed_keys(keys: Vec<Key>, body: &SelectStmt, columns: &[String]) -> Vec<Key> {
    if columns.is_empty() {
        return keys;
    }
    let (Some(names), Some(renamed)) = (output_names(body), renamed_names(body, columns)) else { return Vec::new() };
    let at = |name: &str| {
        let mut found = names.iter().enumerate().filter(|(_, n)| n.as_deref() == Some(name)).map(|(i, _)| i);
        match (found.next(), found.next()) {
            (Some(i), None) => Some(i),
            _ => None,
        }
    };
    keys.into_iter()
        .filter_map(|key| key.into_iter().map(|p| Some(Part { name: renamed.get(at(&p.name)?)?.clone()?, ..p })).collect())
        .collect()
}

/// The statement's own name for the column a column list calls `column`.
fn inner_name(column: &str, body: &SelectStmt, columns: &[String]) -> Option<String> {
    if columns.is_empty() {
        return Some(column.to_string());
    }
    let (names, renamed) = (output_names(body)?, renamed_names(body, columns)?);
    let mut found = renamed.iter().enumerate().filter(|(_, n)| n.as_deref() == Some(column)).map(|(i, _)| i);
    match (found.next(), found.next()) {
        (Some(i), None) => names.get(i)?.clone(),
        _ => None,
    }
}

/// `f` of a view's defining SELECT, as `pg_get_viewdef` prints it; `None` where it does not parse
/// as one SELECT. The definition's names resolve as they do in the statement, which the server
/// prints them for.
fn view_select<T>(definition: &str, f: impl FnOnce(&SelectStmt) -> T) -> Option<T> {
    let parsed = crate::stack::parse(definition).ok()?;
    let [stmt] = parsed.protobuf.stmts.as_slice() else { return None };
    match stmt.stmt.as_deref()?.node.as_ref()? {
        NodeEnum::SelectStmt(select) => Some(f(select)),
        _ => None,
    }
}

/// The table column behind output column `name` of a SELECT, passed through unchanged.
fn output_base(s: &SelectStmt, name: &str, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Option<(String, String)> {
    if s.larg.is_some() || s.rarg.is_some() || !s.values_lists.is_empty() {
        return None;
    }
    let from = From::read(s, catalog);
    if from.refused.is_some() {
        return None;
    }
    s.target_list.iter().find_map(|target| {
        let Some(NodeEnum::ResTarget(r)) = target.node.as_ref() else { return None };
        let Some(NodeEnum::ColumnRef(c)) = r.val.as_deref().and_then(|v| v.node.as_ref()) else { return None };
        if c.fields.last().is_some_and(|f| matches!(f.node, Some(NodeEnum::AStar(_)))) {
            // `*` over one item, or `q.*`, keeps that item's column of the name.
            let i = match c.fields.len() {
                1 if from.items.len() == 1 && !from.natural && from.using.is_empty() => 0,
                2 => from.items.iter().position(|it| Some(it.alias.as_str()) == c.fields.first().and_then(field_name))?,
                _ => return None,
            };
            return from.base(i, name, dialect, trust, catalog);
        }
        let (i, column) = from.place_column(c, catalog)?;
        let out = if r.name.is_empty() { column.as_str() } else { r.name.as_str() };
        (out == name).then(|| from.base(i, &column, dialect, trust, catalog)).flatten()
    })
}

fn not_key_shaped(s: &SelectStmt) -> Option<&'static str> {
    if s.larg.is_some() || s.rarg.is_some() {
        Some("a set operation (UNION, INTERSECT, EXCEPT) returns any number of rows")
    } else if !s.values_lists.is_empty() {
        Some("a VALUES list returns a row per row it lists")
    } else if !s.group_clause.is_empty() {
        Some("GROUP BY returns one row per group")
    } else if s.having_clause.is_some() {
        Some("HAVING leaves a row per group")
    } else if s.into_clause.is_some() {
        Some("SELECT INTO creates a table and returns no rows")
    } else {
        None
    }
}

/// The key proof over the whole FROM: every item is determined ([`From`]), so the FROM has one
/// row at most; or `SELECT DISTINCT` (`DISTINCT ON`) over values that are the same in every row.
fn one_row(s: &SelectStmt, dialect: Dialect, trust: &Trust, catalog: &Catalog) -> Result<(), String> {
    // With no FROM there is one row before the WHERE, which only removes it: on Postgres, whose
    // select list the caller has found no set-returning call in, the statement returns one row
    // at most (`SELECT 1 WHERE $1`).
    if s.from_clause.is_empty() && dialect == Dialect::Postgres {
        return Ok(());
    }
    if s.from_clause.is_empty() {
        return Err("a SELECT with no FROM and a WHERE can return no row or one, and the engine proves that only through a LIMIT".into());
    }
    let from = From::read(s, catalog);
    if let Some(why) = from.refused {
        return Err(why.to_string());
    }
    let determined = from.determined(false, dialect, trust, catalog);
    if determined.iter().all(|d| *d) {
        return Ok(());
    }
    // `SELECT DISTINCT` returns one row when every output value is the same in every row, and
    // `DISTINCT ON` when every expression it is distinct on is.
    if !s.distinct_clause.is_empty() {
        let constant = from.determined(true, dialect, trust, catalog);
        let on: Vec<&protobuf::Node> = s.distinct_clause.iter().filter(|d| d.node.is_some()).collect();
        let values: Vec<&protobuf::Node> = if on.is_empty() {
            s.target_list
                .iter()
                .filter_map(|t| match t.node.as_ref() {
                    Some(NodeEnum::ResTarget(r)) => r.val.as_deref(),
                    _ => None,
                })
                .collect()
        } else {
            on
        };
        if !values.is_empty() && values.iter().all(|e| from.pinned(e, &constant, catalog)) {
            return Ok(());
        }
    }
    Err(from.why_not(&determined, catalog))
}

/// Whether `where_clause` pins one row of `rv` by a unique key, as the closure over one relation
/// does ([`From::single`]).
fn key_bounds(
    rv: &protobuf::RangeVar,
    with: Option<&protobuf::WithClause>,
    where_clause: Option<&protobuf::Node>,
    catalog: &Catalog,
) -> Result<(), String> {
    let relation = catalog_name(rv);
    // A CTE of the same name shadows the table, and a CTE has no index at all.
    if let Some(with) = with {
        if rv.schemaname.is_empty() && with.ctes.iter().any(|c| cte_name(c) == Some(&relation)) {
            return Err(format!("`{relation}` here is the WITH query of that name, which has no unique index"));
        }
    }
    let from = From::single(rv, where_clause, with);
    let determined = from.determined(false, Dialect::Postgres, &Trust::default(), catalog);
    if determined.iter().all(|d| *d) {
        Ok(())
    } else {
        Err(from.why_not(&determined, catalog))
    }
}

/// How many rows a data-modifying statement's `RETURNING` produces, from its shape: an `INSERT`
/// of one `VALUES` row is one row (or none under `ON CONFLICT DO NOTHING`); an `UPDATE` or
/// `DELETE` whose `WHERE` pins a unique key of its one target relation is at most one; anything
/// else is any number. `None` when the statement is not one of these.
pub fn dml(sql: &str, catalog: &Catalog) -> Option<Result<Cardinality, String>> {
    crate::stack::run(|| dml_here(sql, catalog))
}

fn dml_here(sql: &str, catalog: &Catalog) -> Option<Result<Cardinality, String>> {
    let parsed = crate::stack::parse(sql).ok()?;
    let mut stmts = parsed.protobuf.stmts;
    if stmts.len() != 1 {
        return None;
    }
    let node = stmts.pop().and_then(|s| s.stmt).and_then(|n| n.node)?;
    dml_node(&node, catalog)
}

fn dml_node(node: &NodeEnum, catalog: &Catalog) -> Option<Result<Cardinality, String>> {
    Some(match node {
        NodeEnum::InsertStmt(i) => {
            let rows = match i.select_stmt.as_deref().and_then(|n| n.node.as_ref()) {
                Some(NodeEnum::SelectStmt(s)) if s.values_lists.len() == 1 && s.from_clause.is_empty() => 1,
                Some(NodeEnum::SelectStmt(s)) if !s.values_lists.is_empty() => s.values_lists.len(),
                // `INSERT … DEFAULT VALUES` has no select at all.
                None => 1,
                _ => return Some(Err("an INSERT … SELECT returns a row per row the SELECT produces".into())),
            };
            if rows != 1 {
                return Some(Err("a VALUES list returns a row per row it lists".into()));
            }
            if rewritten(i.relation.as_ref(), TRIGGER_ROW_INSERT, catalog) {
                return Some(Err(REWRITTEN.into()));
            }
            // A row trigger that runs BEFORE the insert (or the update ON CONFLICT turns it into)
            // or INSTEAD of it can return NULL, and then the row is neither written nor returned.
            let skipped = |events: i32| i.relation.as_ref().and_then(|rv| catalog.skipping_triggers.get(&catalog_name(rv))).is_some_and(|mask| mask & events != 0);
            match i.on_conflict_clause.as_deref() {
                Some(c) if c.action == protobuf::OnConflictAction::OnconflictNothing as i32 => Ok(Cardinality::AtMostOne),
                // `DO UPDATE … WHERE` leaves a conflicting row alone, and returns nothing for it.
                Some(c) if c.action == protobuf::OnConflictAction::OnconflictUpdate as i32 && (c.where_clause.is_some() || skipped(TRIGGER_ROW_INSERT | TRIGGER_ROW_UPDATE)) => Ok(Cardinality::AtMostOne),
                _ if skipped(TRIGGER_ROW_INSERT) => Ok(Cardinality::AtMostOne),
                _ => Ok(Cardinality::One),
            }
        }
        NodeEnum::UpdateStmt(u) => {
            if !u.from_clause.is_empty() {
                return Some(Err("an UPDATE … FROM returns a row per pair it matches".into()));
            }
            let rv = u.relation.as_ref()?;
            if rewritten(Some(rv), TRIGGER_ROW_UPDATE, catalog) {
                return Some(Err(REWRITTEN.into()));
            }
            key_bounds(rv, u.with_clause.as_ref(), u.where_clause.as_deref(), catalog).map(|()| Cardinality::AtMostOne)
        }
        NodeEnum::DeleteStmt(d) => {
            if !d.using_clause.is_empty() {
                return Some(Err("a DELETE … USING returns a row per pair it matches".into()));
            }
            let rv = d.relation.as_ref()?;
            if rewritten(Some(rv), TRIGGER_ROW_DELETE, catalog) {
                return Some(Err(REWRITTEN.into()));
            }
            key_bounds(rv, d.with_clause.as_ref(), d.where_clause.as_deref(), catalog).map(|()| Cardinality::AtMostOne)
        }
        _ => return None,
    })
}

/// What the proofs settle on, for one statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    One,
    AtMostOne,
}

/// Why a write to a relation with a rewrite rule for it, or to a view, is any number of rows.
const REWRITTEN: &str = "a rewrite rule replaces the write with actions of its own, and a view hands it to a table whose triggers and rules run, so RETURNING can return any number of rows";

/// Whether a rewrite rule (or the relation being a view) covers one of these events.
fn rewritten(rv: Option<&protobuf::RangeVar>, events: i32, catalog: &Catalog) -> bool {
    rv.and_then(|rv| catalog.rewritten.get(&catalog_name(rv))).is_some_and(|mask| mask & events != 0)
}

fn cte_name(cte: &protobuf::Node) -> Option<&String> {
    match cte.node.as_ref() {
        Some(NodeEnum::CommonTableExpr(c)) => Some(&c.ctename),
        _ => None,
    }
}

/// The name a relation goes by in the catalog the proof reads: schema-qualified when the
/// statement qualifies it, so `archive.users` never takes the keys of the `users` on the path.
fn catalog_name(rv: &protobuf::RangeVar) -> String {
    if rv.schemaname.is_empty() {
        rv.relname.clone()
    } else {
        format!("{}.{}", rv.schemaname, rv.relname)
    }
}

/// Whether `column = value` restricts `relation` the way its unique index does, so the key it
/// belongs to may count as pinned. The planner asks that the comparison be the index's own
/// equality, under a collation that agrees with the index's (`relation_has_unique_index_ext`):
///
/// - An explicit `COLLATE` anywhere in the value may make the comparison case- or
///   accent-insensitive, matching rows the index tells apart.
/// - A value of another type than the column's makes the server convert one side. Converting the
///   value is harmless, but a conversion of the column to the value's type can map two of its
///   values to one (`numeric_col = $1::float8`: 0.1 and 0.10000000000000000001 are one `float8`).
///   So the value's type must be one the column's operator family compares as it is
///   ([`compatible`]). A literal or a parameter the server types from the column adapts to it.
/// - Where a user or an extension defined an operator named `=`, the comparison may resolve to
///   theirs, which need not be the index's equality: a user's `=` of a domain, beside a literal,
///   is the domain's own. So a comparison counts where it resolves to a built-in `=` for the two
///   types ([`crate::resolution`]), a built-in equality being as fine as any index's; across two
///   types a column of a domain, or a value that may be one, proves nothing so. A column of a type
///   of a user's (`citext`) counts beside an untyped value or one of its own type where the `=`
///   that resolves to is the btree equality of every unique index's key of that type, as the
///   trust query reads it from `pg_amop` ([`crate::resolution::Overloads::key_equality`]).
/// - A value whose type the build cannot name counts only for a column whose type no implicit cast
///   maps to a wider one with loss ([`ANY_VALUE_TYPES`]). A column, or a scalar subquery's
///   column, on the other side has the type `type_of` gives it, from the catalog.
/// - A volatile call is evaluated for each row, so `id = floor(random() * 3)::int` can match
///   several rows ([`calls_volatile`]).
fn equality_bounds(catalog: &Catalog, relation: &str, column: &str, value: &protobuf::Node, type_of: &dyn Fn(&protobuf::Node) -> Option<u32>) -> bool {
    if mentions(value, "CollateClause") || calls_volatile(value, catalog) {
        return false;
    }
    let Some(column_type) = catalog.column_types.get(relation).and_then(|types| types.get(column)).copied() else { return true };
    // The `=` beside an untyped value, or one of the column's own type, is the exact match of the
    // column's type; a column of a domain reads as its base type, whose overloads of the domain's
    // the resolution counts as exact matches too.
    let own = |value: Operand| catalog.equality(Operand::Is(column_type), value) || (value != Operand::Opaque && catalog.key_equality(relation, column));
    match value_type(value, catalog, type_of) {
        ValueType::Adapts => own(Operand::Unknown),
        ValueType::Is(t) => {
            let domain = catalog.domains.get(relation).is_some_and(|d| d.contains(column));
            let operand = |t: u32, domain: bool| if domain { Operand::Opaque } else { Operand::Is(t) };
            let value = match value.node.as_ref() {
                Some(NodeEnum::ParamRef(_)) => Operand::Param(Some(t)),
                Some(NodeEnum::ColumnRef(_)) | Some(NodeEnum::SubLink(_)) => operand(t, catalog.domain_bases.contains(&t)),
                _ => Operand::Is(t),
            };
            if t == column_type {
                return own(value);
            }
            compatible(column_type, t) && catalog.equality(operand(column_type, domain), value)
        }
        ValueType::Unnamed => !catalog.equality_shadowed && ANY_VALUE_TYPES.contains(&column_type),
    }
}

impl Catalog {
    /// Whether `=` over operands of these types can only resolve to the built-in one.
    fn equality(&self, left: Operand, right: Operand) -> bool {
        !self.equality_shadowed || self.overloads.as_ref().is_some_and(|o| o.0.resolve(Kind::Infix, "=", &[left, right]).is_some())
    }

    /// Whether the `=` an untyped value beside `relation`'s `column` resolves to is the btree
    /// equality of every unique index's key of the column's declared type.
    fn key_equality(&self, relation: &str, column: &str) -> bool {
        let Some(declared) = self.declared_types.get(relation).and_then(|t| t.get(column)) else { return false };
        self.overloads.as_ref().and_then(|o| o.0.key_equality(*declared)) == Some(true)
    }
}

/// What the build knows of a compared value's type.
enum ValueType {
    /// The server types it from the column it is compared with: a string literal, an uncast
    /// parameter whose describe the build does not have.
    Adapts,
    Is(u32),
    Unnamed,
}

fn value_type(value: &protobuf::Node, catalog: &Catalog, type_of: &dyn Fn(&protobuf::Node) -> Option<u32>) -> ValueType {
    match value.node.as_ref() {
        Some(NodeEnum::ColumnRef(_)) | Some(NodeEnum::SubLink(_)) => type_of(value).map_or(ValueType::Unnamed, ValueType::Is),
        Some(NodeEnum::AConst(c)) => match &c.val {
            Some(protobuf::a_const::Val::Ival(_)) => ValueType::Is(oid::INT4),
            Some(protobuf::a_const::Val::Fval(f)) => crate::infer::postgres_number(&f.fval).map_or(ValueType::Unnamed, ValueType::Is),
            Some(protobuf::a_const::Val::Boolval(_)) => ValueType::Is(oid::BOOL),
            _ => ValueType::Adapts,
        },
        Some(NodeEnum::ParamRef(p)) => match catalog.param_types.get((p.number as usize).wrapping_sub(1)) {
            Some(&t) if t != 0 => ValueType::Is(t),
            _ => ValueType::Adapts,
        },
        Some(NodeEnum::TypeCast(t)) => {
            let name = t.type_name.as_ref().filter(|n| n.array_bounds.is_empty() && !n.setof && !n.pct_type).and_then(|n| last_string(&n.names));
            match name.and_then(|n| CAST_TYPES.iter().find(|(c, _)| *c == n).map(|(_, o)| *o)) {
                Some(o) => ValueType::Is(o),
                None => ValueType::Unnamed,
            }
        }
        _ => built_in_result(value, catalog, type_of).map_or(ValueType::Unnamed, ValueType::Is),
    }
}

/// The type a call over values the build types is, where the call can only be the built-in one:
/// `lower` and `upper` of a string are `text` (an untyped argument takes `lower(text)`, the
/// string category being the one an unknown argument picks), and `+`, `-`, `*`, `/` and `%` over
/// integers are the wider integer (`int2 + int4` is `int4`, either side `int8` is `int8`), an
/// untyped side taking the other's type. Only a name the trust check read and no user or
/// extension defined, and only where no function of theirs converts a built-in type implicitly,
/// which a built-in's candidate may run on its operand.
fn built_in_result(value: &protobuf::Node, catalog: &Catalog, type_of: &dyn Fn(&protobuf::Node) -> Option<u32>) -> Option<u32> {
    const INTEGERS: &[u32] = &[oid::INT2, oid::INT4, oid::INT8];
    let built_in = |name: &str| !catalog.user_operators.contains(name) && catalog.overloads.as_ref().is_some_and(|o| !o.0.casts_implicitly());
    match value.node.as_ref()? {
        NodeEnum::FuncCall(f) => {
            let name = infer::builtin_name(&f.funcname)?;
            let plain = f.agg_order.is_empty() && f.agg_filter.is_none() && f.over.is_none() && !f.agg_within_group && !f.agg_star && !f.agg_distinct && !f.func_variadic;
            let [arg] = f.args.as_slice() else { return None };
            let string = match value_type(arg, catalog, type_of) {
                ValueType::Adapts => true,
                ValueType::Is(t) => [oid::TEXT, oid::VARCHAR].contains(&t),
                ValueType::Unnamed => false,
            };
            let named = matches!(arg.node.as_ref(), Some(NodeEnum::NamedArgExpr(_)));
            (plain && !named && string && ["lower", "upper"].contains(&name.as_str()) && infer::postgres_function_names().contains(&name.as_str()) && built_in(&name)).then_some(oid::TEXT)
        }
        NodeEnum::AExpr(e) if e.kind == protobuf::AExprKind::AexprOp as i32 => {
            let name = infer::builtin_name(&e.name)?;
            if !["+", "-", "*", "/", "%"].contains(&name.as_str()) || !infer::POSTGRES_OPERATORS.contains(&name.as_str()) || !built_in(&name) {
                return None;
            }
            let side = |n: Option<&protobuf::Node>| match n.map(|n| value_type(n, catalog, type_of)) {
                Some(ValueType::Is(t)) if INTEGERS.contains(&t) => Some(Some(t)),
                Some(ValueType::Adapts) => Some(None),
                _ => None,
            };
            match (side(e.lexpr.as_deref())?, side(e.rexpr.as_deref())?) {
                (Some(l), Some(r)) => INTEGERS.iter().rev().find(|t| **t == l || **t == r).copied(),
                (Some(t), None) | (None, Some(t)) => Some(t),
                (None, None) => None,
            }
        }
        _ => None,
    }
}

/// Built-in cast targets by the name libpg_query gives them.
const CAST_TYPES: &[(&str, u32)] = &[
    ("text", oid::TEXT),
    ("varchar", oid::VARCHAR),
    ("bpchar", oid::BPCHAR),
    ("name", oid::NAME),
    ("int2", oid::INT2),
    ("int4", oid::INT4),
    ("int8", oid::INT8),
    ("numeric", oid::NUMERIC),
    ("float4", oid::FLOAT4),
    ("float8", oid::FLOAT8),
    ("bool", oid::BOOL),
    ("uuid", oid::UUID),
    ("date", oid::DATE),
    ("timestamp", oid::TIMESTAMP),
    ("timestamptz", oid::TIMESTAMPTZ),
];

/// Column types that no implicit cast converts with loss, so a value of any type bounds them:
/// the server converts the value, or converts the column exactly (`int4` to `float8`, `date` to
/// `timestamp`, `varchar` to `text`), or refuses the comparison.
const ANY_VALUE_TYPES: &[u32] = &[oid::INT2, oid::INT4, oid::BOOL, oid::UUID, oid::DATE, oid::TEXT, oid::VARCHAR];

/// Whether a column of type `column` compared with a value of type `value` uses an equality of the
/// column's btree operator family on the column as it is: the cross-type members of `integer_ops`,
/// `float_ops`, `text_ops` and `datetime_ops`, or a value the server converts to the column's type
/// (an integer to `numeric`, an integer or a `numeric` to a float). One member of `datetime_ops`
/// converts the column: `timestamp = timestamptz` reads the timestamp in the session's time zone,
/// where two wall-clock times are one instant (02:30 and 03:30 on a day that skips 02:00).
fn compatible(column: u32, value: u32) -> bool {
    const INTEGERS: &[u32] = &[oid::INT2, oid::INT4, oid::INT8];
    const FLOATS: &[u32] = &[oid::FLOAT4, oid::FLOAT8];
    const STRINGS: &[u32] = &[oid::TEXT, oid::VARCHAR, oid::NAME];
    const DATES: &[u32] = &[oid::DATE, oid::TIMESTAMP, oid::TIMESTAMPTZ];
    let both = |set: &[u32]| set.contains(&column) && set.contains(&value);
    column == value
        || both(INTEGERS)
        || both(FLOATS)
        || both(STRINGS)
        || (both(DATES) && !(column == oid::TIMESTAMP && value == oid::TIMESTAMPTZ))
        || (column == oid::NUMERIC && INTEGERS.contains(&value))
        || (FLOATS.contains(&column) && (INTEGERS.contains(&value) || value == oid::NUMERIC))
}

/// Column types whose equality is identity: two equal values are the same value, so anything
/// computed from one is computed from the other. Not `numeric` (1 = 1.0, of another scale), not
/// the floats (0 = -0), and not a string under a nondeterministic collation ('a' = 'A').
const IDENTITY_TYPES: &[u32] = &[oid::INT2, oid::INT4, oid::INT8, oid::BOOL, oid::UUID, oid::DATE, oid::TIMESTAMP, oid::TIMESTAMPTZ, oid::TEXT, oid::VARCHAR, oid::NAME];

/// Whether this column's equality is identity ([`IDENTITY_TYPES`]), as far as the catalog says.
fn identity(catalog: &Catalog, relation: &str, column: &str) -> bool {
    catalog.column_types.get(relation).and_then(|types| types.get(column)).is_some_and(|t| IDENTITY_TYPES.contains(t))
        && !catalog.nondeterministic.get(relation).is_some_and(|columns| columns.contains(column))
}

/// The system columns every table has and no column list names.
const SYSTEM_COLUMNS: &[&str] = &["ctid", "xmin", "xmax", "cmin", "cmax", "tableoid"];

/// Whether a window function (a call with `OVER`) sits anywhere in `node`.
fn mentions_window(node: &protobuf::Node) -> bool {
    let Ok(tree) = serde_json::to_value(node) else { return true };
    fn walk(v: &Value) -> bool {
        match v {
            Value::Object(map) => map.iter().any(|(key, child)| (key == "FuncCall" && child.get("over").is_some_and(|o| !o.is_null())) || walk(child)),
            Value::Array(items) => items.iter().any(walk),
            _ => false,
        }
    }
    walk(&tree)
}

/// Whether a node of this kind sits anywhere in `node`.
fn mentions(node: &protobuf::Node, kind: &str) -> bool {
    let Ok(tree) = serde_json::to_value(node) else { return true };
    fn walk(v: &Value, kind: &str) -> bool {
        match v {
            Value::Object(map) => map.iter().any(|(key, child)| key == kind || walk(child, kind)),
            Value::Array(items) => items.iter().any(|i| walk(i, kind)),
            _ => false,
        }
    }
    walk(&tree, kind)
}

/// Whether this is the equality operator itself, rather than `= ANY (…)`, `IS NOT DISTINCT FROM`,
/// or an operator merely spelled with an equals sign.
fn is_equality(e: &protobuf::AExpr) -> bool {
    e.kind == protobuf::AExprKind::AexprOp as i32 && e.name.last().and_then(field_name) == Some("=")
}

/// The column name this expression is, when it names a column of `alias` — `alias.col`, or a bare
/// `col`, which SQL resolves to the innermost relation that has it, and a unique key's column is
/// one this relation has.
fn own_column(node: &protobuf::Node, alias: &str) -> Option<String> {
    let Some(NodeEnum::ColumnRef(c)) = node.node.as_ref() else { return None };
    let parts: Vec<&str> = c.fields.iter().filter_map(field_name).collect();
    match parts.as_slice() {
        [column] => Some((*column).to_string()),
        [qualifier, column] if *qualifier == alias => Some((*column).to_string()),
        _ => None,
    }
}

fn field_name(node: &protobuf::Node) -> Option<&str> {
    match node.node.as_ref() {
        Some(NodeEnum::String(s)) => Some(s.sval.as_str()),
        _ => None,
    }
}

/// A constant `LIMIT n`, when the statement has one the build can read.
fn const_limit(s: &SelectStmt) -> Option<i64> {
    // `FETCH FIRST n ROWS WITH TIES` also returns every row that ties with the last one, so its
    // count bounds nothing.
    if s.limit_option == protobuf::LimitOption::WithTies as i32 {
        return None;
    }
    match s.limit_count.as_deref()?.node.as_ref()? {
        NodeEnum::AConst(c) if !c.isnull => match &c.val {
            Some(protobuf::a_const::Val::Ival(i)) => Some(i.ival as i64),
            _ => None,
        },
        _ => None,
    }
}

/// Whether one rewritten statement provably returns no row, with the reason: a constant `LIMIT 0`
/// (`FETCH FIRST 0 ROWS ONLY` is the same node). Nothing else is claimed, so an endpoint whose
/// `returns:` expects a row is refused only over a statement that can never have one.
pub fn never_a_row(sql: &str, dialect: Dialect) -> Option<&'static str> {
    let s = one_select(sql, dialect).ok()?;
    match s.limit_count.as_deref()?.node.as_ref()? {
        NodeEnum::AConst(c) if !c.isnull => match &c.val {
            Some(protobuf::a_const::Val::Ival(i)) if i.ival == 0 => Some("LIMIT 0 returns no rows"),
            _ => None,
        },
        _ => None,
    }
}

/// The one `SELECT` a statement is, parsed as the dialect's statements are read.
fn one_select(sql: &str, dialect: Dialect) -> Result<Box<SelectStmt>, String> {
    let text = match dialect {
        Dialect::Postgres | Dialect::Duckdb => sql.to_string(),
        Dialect::Sqlite | Dialect::Mysql => infer::dollar_placeholders(sql),
    };
    let parsed = crate::stack::parse(&text).map_err(|_| match dialect {
        Dialect::Postgres => "the statement does not parse".to_string(),
        _ => format!("the engine reads a {} statement's shape in the SQL it shares with Postgres, and this one uses syntax outside it", dialect.name()),
    })?;
    let mut stmts = parsed.protobuf.stmts;
    if stmts.len() != 1 {
        return Err("it is not exactly one statement".into());
    }
    match stmts.pop().and_then(|s| s.stmt).and_then(|n| n.node) {
        Some(NodeEnum::SelectStmt(mut s)) => {
            // `GROUP BY ()` groups every row into the one empty grouping set, and beside other
            // grouping items it is the identity of their cross product: dropped, the statement is
            // the plain aggregate it is (`SELECT count(*) FROM t GROUP BY ()` is one row). Without
            // an aggregate the statement then reads as ungrouped, which is many rows, never fewer.
            s.group_clause.retain(|g| !matches!(g.node.as_ref(), Some(NodeEnum::GroupingSet(set)) if set.kind == protobuf::GroupingSetKind::GroupingSetEmpty as i32));
            Ok(s)
        }
        _ => Err("only a SELECT is proven".into()),
    }
}

/// Which of `single_row`'s conditions a statement breaks, for the diagnostic. The decision is
/// `single_row`'s; this only names it.
fn not_one_row_shaped(s: &SelectStmt) -> &'static str {
    if s.larg.is_some() || s.rarg.is_some() {
        "a set operation (UNION, INTERSECT, EXCEPT) returns any number of rows"
    } else if !s.values_lists.is_empty() {
        "a VALUES list returns a row per row it lists"
    } else if !s.group_clause.is_empty() {
        "GROUP BY returns one row per group, and none when nothing matches"
    } else if s.having_clause.is_some() {
        "HAVING can remove the row"
    } else {
        "a LIMIT or an OFFSET can remove the row (LIMIT 0, OFFSET 1)"
    }
}

/// The function and operator names of one query level.
#[derive(Debug, Default)]
struct Calls {
    functions: Vec<String>,
    operators: Vec<String>,
}

impl Calls {
    /// Why a call here may return a set, if one may.
    fn set_returning(&self, catalog: &Catalog) -> Option<String> {
        for name in &self.functions {
            if catalog.set_returning.contains(name) {
                return Some(format!("the set-returning function {name} returns a row per value, and none for an empty set"));
            }
            if !catalog.functions.contains(name) {
                return Some(format!("the function {name} is not in the catalog, so the build cannot rule out that it returns a set"));
            }
        }
        self.operators
            .iter()
            .find(|op| catalog.set_returning_operators.contains(op.as_str()))
            .map(|op| format!("the operator {op} is implemented by a set-returning function in this database"))
    }
}

fn calls_at_this_level<'a>(nodes: impl Iterator<Item = &'a protobuf::Node>) -> Calls {
    let mut calls = Calls::default();
    for node in nodes {
        if let Ok(tree) = serde_json::to_value(node) {
            walk(&tree, &mut calls);
        }
    }
    calls
}

fn last_name(names: Option<&Value>) -> Option<String> {
    names?.as_array()?.last()?.get("node")?.get("String")?.get("sval")?.as_str().map(String::from)
}

fn walk(v: &Value, calls: &mut Calls) {
    match v {
        Value::Object(map) => {
            for (key, child) in map {
                match key.as_str() {
                    // A subquery answers one value to this level; only the expression it tests
                    // (`x IN (SELECT …)`) and the operator it tests with are evaluated here.
                    "SubLink" => {
                        let tests = child.get("testexpr").is_some_and(|t| !t.is_null());
                        match last_name(child.get("oper_name")) {
                            Some(op) => calls.operators.push(op),
                            None if tests => calls.operators.push("=".into()),
                            None => {}
                        }
                        if let Some(test) = child.get("testexpr") {
                            walk(test, calls);
                        }
                    }
                    "FuncCall" => {
                        if let Some(name) = last_name(child.get("funcname")) {
                            calls.functions.push(name);
                        }
                        walk(child, calls);
                    }
                    "AExpr" => {
                        match last_name(child.get("name")) {
                            // `a BETWEEN x AND y` is spelled with the comparisons it expands to.
                            Some(op) if op.contains("BETWEEN") => calls.operators.extend(["<", ">", "<=", ">="].map(String::from)),
                            Some(op) => calls.operators.push(op),
                            None => {}
                        }
                        walk(child, calls);
                    }
                    _ => walk(child, calls),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| walk(i, calls)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        let mut c = Catalog::default();
        c.unique_keys.insert("users".into(), vec![vec!["id".into()], vec!["email".into()]]);
        c.unique_keys.insert("posts".into(), vec![vec!["id".into()]]);
        for f in ["count", "lower", "abs", "floor"] {
            c.functions.insert(f.into());
        }
        c
    }

    /// `catalog()` with every column's type, as a Postgres host reports them, and more relations.
    fn joined() -> Catalog {
        let mut c = catalog();
        c.unique_keys.insert("comments".into(), vec![vec!["id".into()]]);
        c.unique_keys.insert("profiles".into(), vec![vec!["user_id".into()]]);
        let columns = |cols: &[(&str, u32)]| cols.iter().map(|(n, t)| (n.to_string(), *t)).collect();
        c.column_types.insert("users".into(), columns(&[("id", oid::INT8), ("email", oid::TEXT), ("name", oid::TEXT), ("pinned_post", oid::INT8)]));
        c.column_types.insert("posts".into(), columns(&[("id", oid::INT8), ("author_id", oid::INT8), ("title", oid::TEXT), ("score", oid::FLOAT8)]));
        c.column_types.insert("comments".into(), columns(&[("id", oid::INT8), ("post_id", oid::INT8), ("body", oid::TEXT)]));
        c.column_types.insert("profiles".into(), columns(&[("user_id", oid::INT8), ("bio", oid::TEXT)]));
        c.functions.insert("random".into());
        c.volatile.insert("random".into());
        c
    }

    #[test]
    fn a_nested_statement_is_one_row_by_an_aggregate_of_its_own_only() {
        // A subquery proven on its own, as a host proves a site inside a statement that reads
        // `users u`: `name` and `u.id` are that statement's columns.
        let mut c = joined();
        c.nested = true;
        c.columns = [("posts", vec!["id", "author_id", "title", "score"]), ("comments", vec!["id", "post_id", "body"])].into_iter().collect();
        let one = |sql: &str, c: &Catalog| exactly_one(sql, Dialect::Postgres, &Trust::default(), c).is_ok();
        for sql in [
            "select count(*) as n from posts p where p.author_id = u.id",
            "select count(id) as n from posts where author_id = u.id",
            "select count(p.id) as n from posts p",
            "select count(score) + 1 as n from posts",
            "select count(body) as n from posts join comments c on c.post_id = posts.id",
            "select s.n from (select count(title) as n from posts) s",
            "with s as (select count(title) as n from posts) select n from s",
        ] {
            assert!(one(sql, &c), "{sql}");
        }
        for sql in [
            "select count(u.id) as n from posts",
            "select count(name) as n from posts",
            "select count(*) filter (where name <> '') as n from posts",
            "select s.n from (select count(name) as n from posts) s",
            "with s as (select count(name) as n from posts) select n from s",
            // A WITH query of the enclosing statement is no table of that name.
            "select count(title) as n from enclosing",
        ] {
            let mut c = c.clone();
            c.enclosing_ctes.push("enclosing".into());
            c.columns.insert("enclosing", vec!["title".to_string()]);
            assert!(!one(sql, &c), "{sql}");
            // A statement of its own has no query around it to give the aggregate to.
            c.nested = false;
            assert!(one(sql, &c) || sql.contains("enclosing"), "{sql}");
        }
        // Without the relation's columns a bare name proves nothing.
        c.columns = Columns::default();
        assert!(!one("select count(id) as n from posts", &c));
        assert!(one("select count(*) as n from posts", &c));
        assert_eq!(with_queries("with a as (select 1) select (with b as (select 2) select * from b) from a"), ["a", "b"]);
    }

    #[test]
    fn a_join_on_a_unique_key_of_a_pinned_row_is_at_most_one_row() {
        let mut c = joined();
        // `q` is a comment row of an enclosing query, as a host proving a subquery says.
        c.outer_aliases.insert("q".into(), "comments".into());
        let at = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        for sql in [
            // posts by the WHERE, then users through the ON from the post's author.
            "select p.title, u.email from posts p join users u on u.id = p.author_id where p.id = $1",
            "select p.title, u.email from posts p left join users u on u.id = p.author_id where p.id = $1",
            "select p.title, u.email from users u right join posts p on u.id = p.author_id where p.id = $1",
            // A chain, and the same with bare names that only one relation has.
            "select c.body, p.title, u.email from comments c join posts p on p.id = c.post_id join users u on u.id = p.author_id where c.id = $1",
            "select body, title, email from comments c join posts p on p.id = post_id join users u on u.id = author_id where c.id = $1",
            // One-to-one through a key that is also the join column.
            "select u.email, pr.bio from users u left join profiles pr on pr.user_id = u.id where u.id = $1",
            // A self-join, and comma joins with the equalities in the WHERE.
            "select a.email from users a join users b on b.id = a.id where a.id = $1",
            "select p.title, u.email from posts p, users u where u.id = p.author_id and p.id = $1",
            // A derived table and a WITH query of one row each.
            "select s.email from (select * from users where id = $1) s",
            "with one as (select * from users where id = $1) select o.email, p.title from one o join posts p on p.id = o.pinned_post",
            // A LATERAL subquery of one row per row before it, once that row is pinned.
            "select u.email, x.title from users u cross join lateral (select title from posts where author_id = u.id order by id desc limit 1) x where u.id = $1",
            // A column of an enclosing query is a value, as a parameter is.
            "select p.title, u.email from posts p join users u on u.id = p.author_id where p.id = q.post_id",
            // An uncorrelated subquery is a value too, its bare columns its own relation's.
            "select email from users where id = (select author_id from posts where id = $1)",
        ] {
            assert_eq!(at(sql), Ok(()), "{sql}");
        }
    }

    #[test]
    fn a_join_proves_nothing_unless_every_relation_is_pinned() {
        let c = joined();
        let at = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        for (sql, fragment) in [
            // Many posts per author.
            ("select p.title from posts p join users u on u.id = p.author_id where u.id = $1", "of `p`"),
            // The ON of a LEFT JOIN pins its right side only, so `u` is any number of rows.
            ("select u.email, p.title from users u left join posts p on p.id = $1", "of `u`"),
            // Nor does it filter its left side: every user comes back, whatever the ON equates.
            ("select u.email, p.title from users u left join posts p on u.id = $1 and p.id = $2", "of `u`"),
            // A FULL JOIN returns a row for each side that matches nothing.
            ("select p.title, u.email from posts p full join users u on u.id = p.author_id where p.id = $1", "FULL JOIN"),
            // USING equates columns the build does not read.
            ("select p.title from posts p join users u using (id) where p.id = $1", "of `u`"),
            // A derived table of many rows, and LATERAL before its outer row is pinned.
            ("select u.email, x.title from users u cross join lateral (select title from posts where author_id = u.id limit 1) x", "of `u`"),
            // A function in FROM, and a join with an alias of its own.
            ("select u.email from users u, generate_series(1, 2) g where u.id = $1", "`g` in the FROM"),
            ("select j.email from (users u join posts p on p.author_id = u.id) as j where j.id = $1", "alias of its own"),
            // A column of a relation that is not pinned is no value: each post has its own author.
            ("select p.title, u.email from posts p join users u on u.id = p.author_id where p.author_id = $1", "of `p`"),
        ] {
            refused(at(sql), fragment, sql);
        }
        // Without the column types, a bare name on the other side cannot be placed.
        let bare = "select p.title, u.email from posts p join users u on u.id = author_id where p.id = $1";
        assert!(at_most_one(bare, Dialect::Postgres, &Trust::default(), &catalog()).is_err());
        assert!(at(bare).is_ok());
    }

    #[test]
    fn grouping_and_distinct_over_pinned_values_are_one_row() {
        let c = joined();
        let at = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        for sql in [
            "select author_id, count(*) from posts where author_id = $1 group by author_id",
            "select author_id, count(*) from posts where author_id = $1 group by 1",
            "select distinct author_id from posts where author_id = $1",
            "select distinct on (author_id) title from posts where author_id = $1 order by author_id, id desc",
            "select distinct p.author_id, u.email from posts p join users u on u.id = p.author_id where p.author_id = $1",
            // INTERSECT draws from both arms and EXCEPT from its left one.
            "select id from users where id = $1 intersect select author_id from posts",
            "select author_id from posts intersect all select id from users where id = $1",
            "select id from users where id = $1 except select author_id from posts",
        ] {
            assert_eq!(at(sql), Ok(()), "{sql}");
        }
        for (sql, fragment) in [
            ("select title, count(*) from posts where author_id = $1 group by title", "GROUP BY"),
            // The pin's COLLATE may match values the grouping tells apart.
            ("select distinct title from posts where title = $1 collate ci", "does not equate every column"),
            ("select author_id, count(*) from posts where author_id = $1 group by grouping sets ((author_id), ())", "GROUPING SETS"),
            // Some posts meet the user and some its null extension: two groups.
            ("select u.email, count(*) from posts p left join users u on u.id = $1 and u.id = p.author_id group by u.email", "GROUP BY"),
            ("select author_id from posts except select id from users where id = $1", "a set operation"),
            ("select id from users where id = $1 union select author_id from posts", "a set operation"),
        ] {
            refused(at(sql), fragment, sql);
        }
    }

    #[test]
    fn a_unique_index_over_an_expression_or_with_a_predicate_bounds_what_it_covers() {
        let mut c = joined();
        // `create unique index on users (lower(email))`, `… on posts (title) where score is null`,
        // `… on comments (body) where body is not null`, as the catalog reports them.
        c.unique_keys.get_mut("users").expect("users").push(vec!["(lower(email))".into()]);
        c.column_types.get_mut("users").expect("users").insert("(lower(email))".into(), oid::TEXT);
        c.partial_keys.insert("posts".into(), vec![(vec!["title".into()], "score IS NULL".into())]);
        c.partial_keys.insert("comments".into(), vec![(vec!["body".into()], "body IS NOT NULL".into())]);
        c.partial_keys.insert("profiles".into(), vec![(vec!["bio".into()], "bio = 'active'::text".into())]);
        let at = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        // The server prints a literal with the cast the statement leaves to the column.
        assert_eq!(at("select user_id from profiles where bio = 'active'"), Ok(()));
        refused(at("select user_id from profiles where bio = $1"), "where bio = 'active'::text", "a parameter is no literal");
        assert_eq!(at("select id from users where lower(email) = lower($1)"), Ok(()));
        assert_eq!(at("select u.id from users u where lower(u.email) = $1"), Ok(()));
        assert_eq!(at("select id from posts where title = $1 and score is null"), Ok(()));
        assert_eq!(at("select p.id from posts p where p.title = $1 and p.score is null"), Ok(()));
        // `col IS NOT NULL` is met by the equality itself, which no NULL passes.
        assert_eq!(at("select id from comments where body = $1"), Ok(()));
        // Another expression, or a read the predicate does not cover.
        refused(at("select id from users where upper(email) = $1"), "does not equate", "another expression");
        refused(at("select id from posts where title = $1"), "where score IS NULL", "no predicate");
        refused(at("select id from posts where title = $1 and (score is null or id = 2)"), "does not equate", "the predicate under an OR");
        // A predicate met only in a join's ON clause does not count.
        refused(at("select p.id from users u join posts p on p.score is null where p.title = $1 and u.id = 1"), "of `p`", "the predicate in an ON");
        // A write reads the same keys.
        assert!(matches!(dml("delete from posts where title = $1 and score is null returning id", &c), Some(Ok(_))));
        assert!(matches!(dml("delete from posts where title = $1 returning id", &c), Some(Err(_))));
    }

    #[test]
    fn a_volatile_value_pins_nothing() {
        let c = joined();
        let at = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        // `random()` is evaluated for each row: `id = floor(random() * 3)::int` can match two.
        refused(at("select email from users where id = floor(random() * 3)::int"), "does not equate every column", "random");
        // A function the catalog does not know may be volatile.
        refused(at("select email from users where id = nextval('s')"), "does not equate every column", "nextval");
        refused(at("select author_id, count(*) from posts where author_id = random()::int group by author_id"), "GROUP BY", "group by random");
        assert!(matches!(dml("delete from users where id = random()::int returning id", &c), Some(Err(_))));
        assert_eq!(at("select id from users where email = lower($1)"), Ok(()));
    }

    #[test]
    fn a_value_that_changes_from_row_to_row_pins_nothing() {
        let mut c = joined();
        c.functions.insert("row_number".into());
        let at = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        for (sql, fragment) in [
            // A window function, a system column and a whole row read through a field read no
            // column of the FROM by name, and are another value in each row all the same.
            ("select distinct row_number() over () as n from posts", "does not equate every column"),
            ("select distinct ctid::text as c from posts", "does not equate every column"),
            ("select distinct (p).id from posts p", "does not equate every column"),
            ("select distinct on ((p).author_id) title from posts p", "does not equate every column"),
            ("select distinct u.email from posts p join users u on u.id = (p).author_id", "a join returns a row per pair"),
            ("select u.id, count(*) as n from posts p join users u on u.id = (p).author_id group by u.id", "GROUP BY"),
            // An operator the trust check never asked about may call a volatile function.
            ("select email from users where id = (0 +? 0)", "does not equate every column"),
            ("select email from users where id = (0 +? 0)::int8", "does not equate every column"),
        ] {
            refused(at(sql), fragment, sql);
        }
        assert_eq!(at("select id from users where email = 'a' || 'b'"), Ok(()));
        // A built-in spelling a user also defined may resolve to theirs.
        let mut shadowed = joined();
        shadowed.user_operators.insert("||".into());
        assert!(at_most_one("select id from users where email = 'a' || 'b'", Dialect::Postgres, &Trust::default(), &shadowed).is_err());
        // A cast to a type a user's cast function targets may call it for each row; a literal's
        // cast reads the type's input function instead.
        let mut cast = joined();
        cast.cast_targets = Some(HashSet::new());
        assert!(at_most_one("select email from users where id = (1 + 2)::int8", Dialect::Postgres, &Trust::default(), &cast).is_err());
        assert_eq!(at_most_one("select email from users where id = '7'::int8", Dialect::Postgres, &Trust::default(), &cast), Ok(()));
    }

    #[test]
    fn only_a_column_whose_equality_is_identity_is_one_value_for_another_relation() {
        let mut c = catalog();
        c.unique_keys.insert("pv_users".into(), vec![vec!["id".into()]]);
        c.unique_keys.insert("pv_posts".into(), vec![vec!["id".into()]]);
        let columns = |cols: &[(&str, u32)]| cols.iter().map(|(n, t)| (n.to_string(), *t)).collect();
        c.column_types.insert("pv_users".into(), columns(&[("id", oid::INT4), ("email", oid::TEXT)]));
        c.column_types.insert("pv_posts".into(), columns(&[("id", oid::INT4), ("tag", oid::TEXT), ("code", oid::TEXT), ("amount", oid::NUMERIC)]));
        c.nondeterministic.insert("pv_posts".into(), ["tag".to_string()].into_iter().collect());
        c.functions.extend(["ascii".to_string(), "length".to_string()]);
        let at = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        // `tag = 'a'` admits 'a' and 'A' under a nondeterministic collation, and `amount = 1`
        // admits 1 and 1.0: equal, yet `ascii` and `::text` tell them apart.
        for sql in [
            "select distinct u.email from pv_posts p join pv_users u on u.id = ascii(p.tag) where p.tag = 'a'",
            "select u.email, count(*) from pv_posts p join pv_users u on u.id = ascii(p.tag) where p.tag = 'a' group by u.email",
            "select distinct u.email from pv_posts p join pv_users u on u.id = length(p.amount::text) where p.amount = 1",
        ] {
            refused(at(sql), "", sql);
        }
        assert_eq!(at("select distinct u.email from pv_posts p join pv_users u on u.id = ascii(p.code) where p.code = 'a'"), Ok(()));
        assert_eq!(at("select distinct u.email from pv_posts p join pv_users u on u.id = p.id * 97 where p.id = 1"), Ok(()));
    }

    #[test]
    fn a_with_query_of_an_enclosing_statement_shadows_the_table_inside_it() {
        let c = joined();
        let at = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        // A subquery, a WITH query's body and a set operation's arm are proven on their own, and
        // still read `users` as the WITH query of that name, which has no key.
        for sql in [
            "select t.id from (with users as (select 1::int8 as id union all select 1) select * from (select id from users where id = 1) s) t, users u where u.id = 1",
            "with users as (select 1::int8 as id union all select 1), x as (select id from users where id = 1) select x.id from x",
            "with users as (select 1::int8 as id union all select 1) select id from users where id = 1 intersect all select id from posts",
            "with users as (select 1::int8 as id union all select 1) select s.id from (select id from users where id = 1) s",
        ] {
            refused(at(sql), "", sql);
        }
        assert_eq!(at("with one as (select 1 as n) select s.id from (select id from users where id = 1) s"), Ok(()));
    }

    #[test]
    fn a_column_merged_by_using_and_a_qualified_name_are_neither_sides_key() {
        let mut c = joined();
        c.unique_keys.insert("ua".into(), vec![vec!["id".into()], vec!["k".into()]]);
        c.unique_keys.insert("ub".into(), vec![vec!["id".into()], vec!["k".into()]]);
        let columns = |cols: &[(&str, u32)]| cols.iter().map(|(n, t)| (n.to_string(), *t)).collect();
        c.column_types.insert("ua".into(), columns(&[("id", oid::INT8), ("k", oid::FLOAT8)]));
        c.column_types.insert("ub".into(), columns(&[("id", oid::INT8), ("k", oid::NUMERIC)]));
        let at = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        // The merged `k` is `ua.k` compared with `ub.k` converted to float8, which maps two
        // numerics to one value.
        refused(at("select ua.id, ub.id from ua join ub using (k) where k = '0.1'"), "a join returns a row per pair", "using");
        refused(at("select ua.id, ub.id from ua natural join ub where k = '0.1'"), "a join returns a row per pair", "natural");
        // A schema-qualified name is the table, never the WITH query of that name.
        refused(at("with users as (select count(*) as n from users) select id from public.users"), "public.users", "with");
        assert!(exactly_one("with users as (select count(*) as n from users) select id from public.users", Dialect::Postgres, &Trust::default(), &c).is_err());
    }

    fn at_most(sql: &str) -> Result<(), String> {
        at_most_one(sql, Dialect::Postgres, &Trust::default(), &catalog())
    }

    #[test]
    fn an_insert_of_one_row_returns_it_unless_something_can_skip_it() {
        let c = catalog();
        assert_eq!(dml("insert into users (email) values ($1) returning id", &c), Some(Ok(Cardinality::One)));
        assert_eq!(dml("insert into users (email) values ($1) on conflict (email) do nothing returning id", &c), Some(Ok(Cardinality::AtMostOne)));
        assert_eq!(dml("insert into users (email) values ($1) on conflict (email) do update set email = excluded.email returning id", &c), Some(Ok(Cardinality::One)));
        // `DO UPDATE … WHERE` leaves a conflicting row alone and returns nothing for it.
        assert_eq!(dml("insert into users (email) values ($1) on conflict (email) do update set email = excluded.email where users.id > 1 returning id", &c), Some(Ok(Cardinality::AtMostOne)));
        // A row trigger that runs BEFORE the insert can return NULL; one on UPDATE matters only
        // where ON CONFLICT turns the insert into an update.
        let mut triggered = catalog();
        triggered.skipping_triggers.insert("users".into(), TRIGGER_ROW_INSERT);
        assert_eq!(dml("insert into users (email) values ($1) returning id", &triggered), Some(Ok(Cardinality::AtMostOne)));
        triggered.skipping_triggers.insert("users".into(), TRIGGER_ROW_UPDATE);
        assert_eq!(dml("insert into users (email) values ($1) returning id", &triggered), Some(Ok(Cardinality::One)));
        assert_eq!(dml("insert into users (email) values ($1) on conflict (email) do update set email = excluded.email returning id", &triggered), Some(Ok(Cardinality::AtMostOne)));
        // A rewrite rule replaces the write with actions of its own, and a view hands it to a
        // table whose triggers and rules the catalog did not read: any number of rows.
        let mut ruled = catalog();
        ruled.rewritten.insert("users".into(), TRIGGER_ROW_INSERT);
        assert!(matches!(dml("insert into users (email) values ($1) returning id", &ruled), Some(Err(_))));
        assert_eq!(dml("delete from users where id = $1 returning id", &ruled), Some(Ok(Cardinality::AtMostOne)));
        ruled.rewritten.insert("users".into(), TRIGGER_ROW_DELETE | TRIGGER_ROW_UPDATE);
        assert!(matches!(dml("delete from users where id = $1 returning id", &ruled), Some(Err(_))));
        assert!(matches!(dml("update users set email = $2 where id = $1 returning id", &ruled), Some(Err(_))));
    }

    #[test]
    fn a_key_holds_only_under_its_own_equality() {
        use serde_json::json;
        let mut c = catalog();
        c.column_types.insert("users".into(), [("id".to_string(), oid::INT8), ("email".to_string(), oid::TEXT)].into_iter().collect());
        c.param_types = vec![oid::INT8];
        let at = |sql: &str, c: &Catalog| at_most_one(sql, Dialect::Postgres, &Trust::default(), c);
        assert!(at("select email from users where id = $1", &c).is_ok());
        assert!(at("select email from users where id = 7", &c).is_ok(), "an int4 literal against int8 is integer_ops");
        assert!(at("select id from users where email = 'a@x.io'", &c).is_ok());
        // Converting the column to float8 can make two keys one.
        assert!(at("select email from users where id = $1::float8", &c).is_err());
        c.param_types = vec![oid::FLOAT8];
        assert!(at("select email from users where id = $1", &c).is_err());
        // An explicit COLLATE may compare without telling case apart.
        assert!(at("select id from users where email = 'a@x.io' COLLATE ci", &c).is_err());
        // A value whose type the build cannot name bounds a text key, not an int8 one.
        assert!(at("select id from users where email = lower('A@X.IO')", &c).is_ok());
        assert!(at("select email from users where id = abs(-7)", &c).is_err());
        // With a user's `=` in the database and nothing said of its overloads, nothing counts.
        c.equality_shadowed = true;
        assert!(at("select email from users where id = 7", &c).is_err());
        c.param_types = vec![oid::INT8];
        assert!(at("select email from users where id = $1", &c).is_err());
        assert!(at("select id from users where email = 'a@x.io'", &c).is_err());
        // An `=` of a type of a user's (pgvector's) leaves the built-ins over int8 and text the
        // exact matches; one of exactly int8's types may come first on the search path.
        let catalog_of = |rows: Vec<Vec<serde_json::Value>>| {
            let mut shadowed = HashSet::new();
            Some(crate::resolution::Shared(std::sync::Arc::new(crate::resolution::Overloads::read(&rows, &mut shadowed))))
        };
        let builtin = |l: u32, r: u32| vec![json!("builtin b"), json!("="), json!([l, r]), json!(oid::BOOL), serde_json::Value::Null];
        let opaque = vec![json!("opaque b"), json!("="), serde_json::Value::Null, json!(2), json!(0)];
        c.overloads = catalog_of(vec![opaque.clone(), builtin(oid::INT8, oid::INT8), builtin(oid::INT8, oid::INT4), builtin(oid::TEXT, oid::TEXT)]);
        assert!(at("select email from users where id = $1", &c).is_ok());
        assert!(at("select email from users where id = 7", &c).is_ok());
        assert!(at("select id from users where email = 'a@x.io'", &c).is_ok());
        c.overloads = catalog_of(vec![vec![json!("user b"), json!("="), json!([oid::INT8, oid::INT8]), json!(0), json!(0)], builtin(oid::INT8, oid::INT8)]);
        assert!(at("select email from users where id = $1", &c).is_err());
        // A domain's own `=` beside a literal is the domain's, no key's equality; a citext key's
        // `=` is its btree family's where the trust query says so.
        const MYTEXT: u32 = 30000;
        const CITEXT: u32 = 30100;
        let mut d = catalog();
        d.unique_keys.insert("u".into(), vec![vec!["d".into()], vec!["h".into()]]);
        d.column_types.insert("u".into(), [("d".to_string(), oid::TEXT), ("h".to_string(), CITEXT)].into_iter().collect());
        d.declared_types.insert("u".into(), [("d".to_string(), MYTEXT), ("h".to_string(), CITEXT)].into_iter().collect());
        d.domains.insert("u".into(), ["d".to_string()].into_iter().collect());
        d.domain_bases.insert(oid::TEXT);
        d.equality_shadowed = true;
        let key = |ty: u32, ok: i64| vec![json!("key"), serde_json::Value::Null, serde_json::Value::Null, json!(ty), json!(ok)];
        d.overloads = catalog_of(vec![
            vec![json!("user b"), json!("="), json!([MYTEXT, MYTEXT]), json!(0), json!(0)],
            vec![json!("base"), serde_json::Value::Null, serde_json::Value::Null, json!(MYTEXT), json!(oid::TEXT)],
            vec![json!("user b"), json!("="), json!([CITEXT, CITEXT]), json!(0), json!(0)],
            builtin(oid::TEXT, oid::TEXT),
            key(MYTEXT, 0),
            key(CITEXT, 1),
        ]);
        assert!(at("select 1 from u where d = 'x'", &d).is_err());
        assert!(at("select 1 from u where h = 'x'", &d).is_ok());
        d.overloads = catalog_of(vec![vec![json!("user b"), json!("="), json!([CITEXT, CITEXT]), json!(0), json!(0)], key(CITEXT, 0)]);
        assert!(at("select 1 from u where h = 'x'", &d).is_err(), "a unique index on a citext column under text_ops");
        // A schema-qualified relation never takes the keys of the one on the search path.
        assert!(at("select name from archive.users a where a.id = $1", &c).is_err());
        // A timestamp key compared with a timestamptz is converted through the session's time
        // zone, where two wall-clock times (02:30 and 03:30 on a day that skips 02:00) are one
        // instant; the other way round, and against a date, the value is converted.
        let mut t = catalog();
        t.unique_keys.insert("ev".into(), vec![vec!["ts".into()], vec!["tstz".into()]]);
        t.column_types.insert("ev".into(), [("ts".to_string(), oid::TIMESTAMP), ("tstz".to_string(), oid::TIMESTAMPTZ)].into_iter().collect());
        assert!(at("select 1 from ev where ts = '2026-03-08 03:30'::timestamptz", &t).is_err());
        assert!(at("select 1 from ev where tstz = '2026-03-08 03:30'::timestamp", &t).is_ok());
        assert!(at("select 1 from ev where ts = '2026-03-08'::date", &t).is_ok());
    }

    #[test]
    fn a_limit_with_ties_bounds_nothing() {
        assert!(at_most("select id from users order by email fetch first 1 rows only").is_ok());
        assert!(at_most("select id from users order by email limit 1").is_ok());
        // Every row that ties with the first comes along.
        assert!(at_most("select id from users order by email fetch first 1 rows with ties").is_err());
    }

    #[test]
    fn grouped_by_a_pinned_relation_is_one_group() {
        assert!(at_most("select u.id, count(p.id) from users u left join posts p on p.user_id = u.id where u.id = $1 group by u.id").is_ok());
        assert!(at_most("select u.id, u.name, count(*) from users u join posts p on p.user_id = u.id where u.id = $1 and p.title = $2 group by u.id, u.name having count(*) > 1").is_ok());
        assert!(at_most("select id, count(*) from users where email = $1 group by id").is_ok());
        // A bare column that only `users` has is its column: had `posts` it too, the server would
        // have refused the name as ambiguous.
        assert!(at_most("select u.id, count(*) from users u join posts p on p.user_id = u.id where email = $1 group by email").is_ok());
    }

    #[test]
    fn grouped_by_another_relation_or_pinned_to_a_column_is_many() {
        assert!(at_most("select u.id, p.title, count(*) from users u join posts p on p.user_id = u.id where u.id = $1 group by u.id, p.title").is_err());
        assert!(at_most("select u.id, count(*) from users u join posts p on true where u.id = p.user_id group by u.id").is_err());
        assert!(at_most("select u.id, count(*) from users u join posts p on p.user_id = u.id where u.name = $1 group by u.id").is_err());
        assert!(at_most("select u.id, count(*) from users u join posts p on p.user_id = u.id where u.id = $1 group by title").is_err());
        assert!(at_most("select u.id, count(*) from users u join posts p on p.user_id = u.id where u.id = $1 group by u.id union all select 1, 2").is_err());
    }


    /// A stock Postgres catalog for the names these tests call: `generate_series` and `unnest`
    /// return sets, the rest do not. `todos` has a primary key and `org_members` a two-column one.
    /// The trust check of a catalog with no object of a user's, whose row types have the attributes
    /// named here among the names the rules read.
    fn answered(attributes: &[&str]) -> Trust {
        use crate::resolution::{Overloads, Shared};
        let rows: Vec<Vec<serde_json::Value>> = attributes.iter().map(|a| vec![serde_json::json!("column"), serde_json::json!(a), serde_json::Value::Null, serde_json::Value::Null, serde_json::Value::Null]).collect();
        let mut trust = Trust::default();
        trust.overloads = Some(Shared(std::sync::Arc::new(Overloads::read(&rows, &mut trust.shadowed))));
        trust
    }

    #[test]
    fn an_aggregate_that_may_be_a_column_of_its_row_argument_is_no_one_row() {
        let mut c = stock();
        c.column_types.insert("todos".into(), [("id".to_string(), oid::INT4), ("n".to_string(), oid::INT4)].into_iter().collect());
        let none = answered(&[]);
        let listed = answered(&["sum"]);
        let one = |sql: &str, trust: &Trust| exactly_one(sql, Dialect::Postgres, trust, &c).is_ok();
        // No row type has a column `sum`, and no row the statement builds does.
        assert!(one("SELECT sum(n) AS total FROM todos", &none));
        assert!(one("SELECT sum(todos) AS total FROM todos", &none));
        // A row type of the catalog does: `sum(x)` over such a row is its column, a value per row,
        // unless the argument is of a type that is no row, or is no row for certain.
        assert!(one("SELECT sum(n) AS total FROM todos", &listed));
        assert!(one("SELECT sum(n + 1) AS total, count(*) AS c FROM todos", &listed));
        assert!(!one("SELECT sum(todos) AS total FROM todos", &listed));
        assert!(!one("SELECT sum(x) AS total FROM (SELECT n AS x FROM todos) s", &listed));
        // A row the statement builds has a column `sum`, which a whole row of it reads.
        assert!(!one("SELECT sum(s) AS total FROM (SELECT n AS sum FROM todos) s", &none));
        assert!(!one("WITH s AS (SELECT n AS every FROM todos) SELECT every(s) AS e FROM s", &none));
        assert!(!one("SELECT sum(s) AS total FROM (SELECT sum FROM (SELECT n AS sum FROM todos) t) s", &none));
        // A catalog that did not answer may have any row type.
        assert!(!one("SELECT sum(todos) AS total FROM todos", &Trust::default()));
        // `count`, and `sum` decorated, are the aggregates whatever the argument.
        assert!(one("SELECT count(todos) AS n FROM todos", &listed));
        assert!(one("SELECT sum(DISTINCT todos) AS n FROM todos", &listed));
    }

    fn stock() -> Catalog {
        let set = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<HashSet<String>>();
        let key = |cols: &[&str]| vec![cols.iter().map(|c| c.to_string()).collect::<Vec<String>>()];
        Catalog {
            functions: set(&["generate_series", "unnest", "count", "sum", "max", "abs", "lower"]),
            set_returning: set(&["generate_series", "unnest"]),
            set_returning_operators: HashSet::new(),
            unique_keys: [
                ("todos".to_string(), key(&["id"])),
                ("org_members".to_string(), key(&["org_id", "user_id"])),
                ("users".to_string(), vec![vec!["id".to_string()], vec!["email".to_string()]]),
            ]
            .into_iter()
            .collect(),
            ..Catalog::default()
        }
    }

    #[test]
    fn a_subquery_a_with_query_or_a_view_is_pinned_by_a_key_of_its_rows() {
        let mut c = stock();
        c.views.insert("open_todos".into(), "SELECT id, title FROM todos WHERE done = false".into());
        let at_most = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        for sql in [
            "select * from (select id, title from todos) s where s.id = $1",
            "with s as (select id, title from todos) select * from s where s.id = $1",
            "select * from (select id as todo_id from todos) s where s.todo_id = $1",
            "select * from (select * from (select id from todos) a) b where b.id = $1",
            "select * from (select id from todos order by id limit 10) s where s.id = $1",
            "select * from (select t.id, u.email from todos t join users u on u.id = t.owner_id) s where s.id = $1",
            "select * from (select t.owner_id, count(*) as n from todos t group by t.owner_id) s where s.owner_id = $1",
            "select * from (select distinct t.owner_id from todos t) s where s.owner_id = $1",
            "select * from (select org_id, user_id from org_members) s where s.org_id = $1 and s.user_id = $2",
            "select * from open_todos where id = $1",
        ] {
            assert_eq!(at_most(sql), Ok(()), "{sql}");
        }
        for sql in [
            "select * from (select title from todos) s where s.title = $1",
            "select * from (select id + 0 as id from todos) s where s.id = $1",
            "select * from (select id from todos union all select id from todos) s where s.id = $1",
            "select * from (select id, generate_series(1, 2) as n from todos) s where s.id = $1",
            "select * from (select t.id from todos t cross join users u) s where s.id = $1",
            "select * from (select org_id, user_id from org_members) s where s.org_id = $1",
            "select * from (select owner_id, count(*) as n from todos group by rollup (owner_id)) s where s.owner_id = $1",
            "select s.id from users u cross join lateral (select t.id, u.id as uid from todos t) s where u.id = s.uid and s.id = $1",
        ] {
            assert!(at_most(sql).is_err(), "{sql}");
        }
    }

    fn pg(sql: &str) -> Result<(), String> {
        exactly_one(sql, Dialect::Postgres, &Trust::default(), &stock())
    }

    fn lite(sql: &str) -> Result<(), String> {
        exactly_one(sql, Dialect::Sqlite, &Trust::default(), &Catalog::default())
    }

    fn refused(result: Result<(), String>, fragment: &str, sql: &str) {
        match result {
            Ok(()) => panic!("{sql}: proven, expected a refusal naming {fragment:?}"),
            Err(why) => assert!(why.contains(fragment), "{sql}: {why:?} does not name {fragment:?}"),
        }
    }

    #[test]
    fn the_canonical_gate_and_augmenter_are_exactly_one_row_on_both_dialects() {
        let gate = "SELECT EXISTS (SELECT 1 FROM org_members WHERE org_id = $1 AND user_id = $2) AS authorized, (SELECT role FROM org_members WHERE org_id = $1 AND user_id = $2) AS role";
        assert_eq!(pg(gate), Ok(()));
        assert_eq!(lite(&gate.replace('$', "?")), Ok(()));
        let aggregate = "SELECT count(*) > 0 AS authorized FROM org_members WHERE org_id = $1 AND user_id = $2";
        assert_eq!(pg(aggregate), Ok(()));
        assert_eq!(lite(&aggregate.replace('$', "?")), Ok(()));
        for sql in [
            "SELECT $1::text AS caller, lower($2) AS name",
            "SELECT coalesce((SELECT role = 'admin' FROM org_members WHERE user_id = $1), false) AS authorized",
            // A set-returning function inside a subquery answers one value to this level.
            "SELECT EXISTS (SELECT generate_series(1, 3)) AS authorized",
            // A WITH is fine, whatever the CTE returns.
            "WITH many AS (SELECT generate_series(1, 3) AS g) SELECT EXISTS (SELECT 1 FROM many) AS authorized",
            // An ungrouped aggregate is one row over no input, with or without a FROM.
            "SELECT count(*) AS n FROM pg_class WHERE false",
            "SELECT count(*) AS n WHERE false",
            "SELECT DISTINCT count(*) AS n FROM pg_class",
            "SELECT count(*) FILTER (WHERE false) AS n FROM pg_class",
            // The windowed sum runs over the aggregate query's one row.
            "SELECT sum(count(*)) OVER () AS n FROM pg_class",
        ] {
            assert_eq!(pg(sql), Ok(()), "{sql}");
        }
        // SQLite has no set-returning function to look up, so it needs no catalog.
        assert_eq!(lite("SELECT lower(?1) AS name, EXISTS (SELECT 1 FROM t WHERE id = ?2) AS authorized"), Ok(()));
        assert_eq!(lite("SELECT max(amount) AS top FROM t"), Ok(()));
    }

    #[test]
    fn a_where_with_no_from_can_remove_the_row() {
        refused(pg("SELECT 1 AS authorized WHERE $1 = 1"), "a WHERE with no FROM can remove the row", "pg");
        refused(lite("SELECT 1 AS authorized WHERE ?1 = 1"), "a WHERE with no FROM can remove the row", "sqlite");
        // It can only remove the one row, so on Postgres it is one row at most, unless a call at
        // its level returns a set.
        let most = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &stock());
        assert_eq!(most("SELECT 1 AS authorized WHERE $1 = 1"), Ok(()));
        refused(most("SELECT generate_series(1, 3) AS g WHERE $1 = 1"), "set-returning function", "generate_series");
        assert!(at_most_one("SELECT 1 AS authorized WHERE ?1 = 1", Dialect::Sqlite, &Trust::default(), &Catalog::default()).is_err());
    }

    #[test]
    fn an_empty_grouping_set_is_one_group() {
        // `GROUP BY ()` is the aggregate without it: one row, even over no rows, and a HAVING can
        // remove it. Beside another grouping item it changes nothing.
        let c = stock();
        assert_eq!(exactly_one("SELECT count(*) AS n FROM pg_class WHERE false GROUP BY ()", Dialect::Postgres, &Trust::default(), &c), Ok(()));
        assert!(exactly_one("SELECT count(*) AS n FROM pg_class GROUP BY () HAVING count(*) > 1", Dialect::Postgres, &Trust::default(), &c).is_err());
        assert!(at_most_one("SELECT relkind, count(*) AS n FROM pg_class GROUP BY relkind, ()", Dialect::Postgres, &Trust::default(), &c).is_err());
        // Without an aggregate it reads as ungrouped: many rows, never fewer than it returns.
        assert!(at_most_one("SELECT 1 AS x FROM pg_class GROUP BY ()", Dialect::Postgres, &Trust::default(), &c).is_err());
    }

    #[test]
    fn a_set_returning_call_at_this_level_multiplies_or_removes_the_row() {
        for sql in [
            "SELECT generate_series(1, 3) AS g",
            "SELECT unnest($1::int[]) AS u",
            "SELECT abs(generate_series(-1, 1)) AS nested",
            "SELECT pg_catalog.generate_series(1, 3) AS qualified",
            "SELECT generate_series(1, 2) IS NOT NULL AS under_a_null_test",
            "SELECT NULLIF(1, abs(generate_series(1, 2))) AS in_nullif_argument",
            "SELECT 1 AS x ORDER BY generate_series(1, 2)",
            "SELECT DISTINCT ON (generate_series(1, 2)) 1 AS x",
            "SELECT $1 IN (SELECT 1) AND generate_series(1, 2) > 0 AS x",
            // An aggregate beside one is as many rows as the set.
            "SELECT count(*) AS n, generate_series(1, 3) AS g FROM pg_class WHERE false",
        ] {
            refused(pg(sql), "set-returning function", sql);
        }
        refused(pg("SELECT mystery($1) AS authorized"), "is not in the catalog", "mystery");
        // With no catalog at all, every Postgres call is unproven, and a call-free one is not.
        refused(exactly_one("SELECT lower($1) AS x", Dialect::Postgres, &Trust::default(), &Catalog::default()), "not in the catalog", "empty");
        assert_eq!(exactly_one("SELECT $1 = 'a' AS x", Dialect::Postgres, &Trust::default(), &Catalog::default()), Ok(()));
    }

    #[test]
    fn an_operator_over_a_set_returning_function_is_a_set_returning_call() {
        let mut catalog = stock();
        let with = |c: &Catalog, sql: &str| exactly_one(sql, Dialect::Postgres, &Trust::default(), c);
        assert_eq!(with(&catalog, "SELECT 1 ##> 3 AS x"), Ok(()), "an operator no set-returning function implements is a value");
        catalog.set_returning_operators.insert("##>".into());
        refused(with(&catalog, "SELECT 1 ##> 3 AS x"), "operator ##>", "##>");
        refused(with(&catalog, "SELECT abs(1 ##> 3) IS NOT NULL AS x"), "operator ##>", "nested ##>");
        assert_eq!(with(&catalog, "SELECT 1 + 1 AS x"), Ok(()));
        catalog.set_returning_operators = ["<="].map(String::from).into_iter().collect();
        refused(with(&catalog, "SELECT $1 BETWEEN 1 AND 2 AS x"), "operator <=", "between");
        catalog.set_returning_operators = ["="].map(String::from).into_iter().collect();
        refused(with(&catalog, "SELECT $1 IN (SELECT 1) AS x"), "operator =", "in subquery");
        refused(with(&catalog, "SELECT $1 IS DISTINCT FROM 2 AS x"), "operator =", "distinct");
    }

    #[test]
    fn limits_group_by_having_and_set_operations_are_unproven() {
        for (sql, fragment) in [
            ("SELECT 1 AS x LIMIT 0", "LIMIT or an OFFSET"),
            ("SELECT 1 AS x LIMIT $1", "LIMIT or an OFFSET"),
            ("SELECT 1 AS x OFFSET 1", "LIMIT or an OFFSET"),
            ("SELECT 1 AS x FETCH FIRST 0 ROWS ONLY", "LIMIT or an OFFSET"),
            ("SELECT count(*) AS n FROM pg_class GROUP BY relkind", "GROUP BY"),
            ("SELECT 1 AS x GROUP BY 1", "GROUP BY"),
            ("SELECT count(*) AS n FROM pg_class HAVING count(*) > 1", "HAVING"),
            ("SELECT true AS authorized UNION SELECT false", "set operation"),
            ("SELECT 1 AS x EXCEPT SELECT 1", "set operation"),
            ("VALUES (true)", "VALUES"),
            ("INSERT INTO todos (title) VALUES ($1) RETURNING id", "only a SELECT"),
            ("SELECT 1; SELECT 2", "not exactly one statement"),
        ] {
            refused(pg(sql), fragment, sql);
            refused(lite(&sql.replace('$', "?")), fragment, sql);
        }
        // A window call is not an aggregate: a row per input row.
        refused(pg("SELECT count(*) OVER () AS n FROM pg_class"), "a FROM returns a row per row", "window");
        refused(pg("SELECT role FROM org_members WHERE user_id = $1"), "a FROM returns a row per row", "plain read");
        // SQLite's two-argument max is a scalar function, not an aggregate.
        refused(lite("SELECT max(a, b) AS top FROM t"), "a FROM returns a row per row", "scalar max");
        // An aggregate name somebody redefined is not the aggregate.
        let shadowed = Trust { shadowed: ["count".to_string()].into_iter().collect(), cast_targets: None, ..Default::default() };
        refused(exactly_one("SELECT count(*) AS n FROM pg_class", Dialect::Postgres, &shadowed, &stock()), "a FROM returns a row per row", "shadowed count");
        // SQLite syntax outside the shared grammar proves nothing.
        refused(lite("SELECT 1 AS x LIMIT 5, 0"), "syntax outside it", "sqlite limit");
        refused(pg("SELECT FROM WHERE"), "does not parse", "garbage");
    }

    fn pg_most(sql: &str) -> Result<(), String> {
        at_most_one(sql, Dialect::Postgres, &Trust::default(), &stock())
    }

    #[test]
    fn a_unique_key_every_column_of_which_is_equated_bounds_the_read_to_one_row() {
        for sql in [
            "SELECT title FROM todos WHERE id = $1",
            "SELECT title FROM todos t WHERE t.id = $1",
            // The key's columns in either order, beside anything else the WHERE asks for.
            "SELECT role FROM org_members WHERE user_id = $1 AND org_id = $2 AND role <> 'banned'",
            "SELECT role FROM org_members m WHERE m.org_id = $1 AND m.user_id = m2.user_id",
            // Either of two unique keys will do.
            "SELECT id FROM users WHERE email = $1",
            // An equality against a correlated column of an enclosing query, which is the
            // one-to-one nesting case.
            "SELECT u.id, u.email FROM users u WHERE u.id = p.author_id",
            // An OFFSET only ever removes rows.
            "SELECT title FROM todos WHERE id = $1 OFFSET 2",
            // A constant LIMIT caps anything, key or no key, set operation included.
            "SELECT title FROM todos WHERE owner_id = $1 ORDER BY created_at DESC LIMIT 1",
            "SELECT 1 AS x UNION SELECT 2 LIMIT 1",
            "SELECT generate_series(1, 3) AS g LIMIT 1",
            "SELECT title FROM todos LIMIT 0",
            // A join on the key of a row the WHERE pins.
            "SELECT t.title FROM todos t JOIN users u ON u.id = t.owner_id WHERE t.id = $1",
            // Exactly one row is at most one row.
            "SELECT count(*) AS n FROM todos",
            "SELECT $1::text AS caller",
        ] {
            assert_eq!(pg_most(sql), Ok(()), "{sql}");
        }
    }

    #[test]
    fn a_read_the_key_does_not_cover_is_not_bounded_to_one_row() {
        for (sql, fragment) in [
            // Half of a two-column key.
            ("SELECT role FROM org_members WHERE org_id = $1", "does not equate every column"),
            // Not an equality: a range, a pattern, `IS NOT DISTINCT FROM`, `= ANY`.
            ("SELECT title FROM todos WHERE id > $1", "does not equate every column"),
            ("SELECT title FROM todos WHERE id IS NOT DISTINCT FROM $1", "does not equate every column"),
            ("SELECT title FROM todos WHERE id = ANY($1)", "does not equate every column"),
            // An OR beside the equality widens what matches.
            ("SELECT title FROM todos WHERE id = $1 OR owner_id = $2", "does not equate every column"),
            // The other side is a column of the same relation, so it restricts nothing.
            ("SELECT title FROM todos WHERE id = owner_id", "does not equate every column"),
            ("SELECT title FROM todos t WHERE t.id = t.owner_id", "does not equate every column"),
            // No WHERE at all.
            ("SELECT title FROM todos", "does not equate every column"),
            // A table with no unique index the build can read.
            ("SELECT body FROM events WHERE id = $1", "no unique index"),
            // A WITH query of the same name is not the table.
            ("WITH todos AS (SELECT id FROM events) SELECT id FROM todos WHERE id = $1", "the WITH query `todos`"),
            // A derived table of any number of rows, and a UNION.
            ("SELECT title FROM todos WHERE id = $1 UNION SELECT title FROM todos", "a set operation"),
            // A LIMIT above one, or one the build cannot read, bounds nothing by itself.
            ("SELECT title FROM todos WHERE owner_id = $1 LIMIT 2", "does not equate every column"),
            ("SELECT title FROM todos WHERE owner_id = $1 LIMIT $2", "does not equate every column"),
            ("SELECT title FROM todos WHERE owner_id = $1 ORDER BY id FETCH FIRST 1 ROWS WITH TIES", "does not equate every column"),
            // A set-returning call multiplies the one row the key found.
            ("SELECT generate_series(1, 3) AS g FROM todos WHERE id = $1", "set-returning function"),
        ] {
            refused(pg_most(sql), fragment, sql);
        }
        // The key pins one row of `todos` and the GROUP BY reads only its columns: one group at most.
        assert_eq!(pg_most("SELECT count(*) AS n FROM todos WHERE id = $1 GROUP BY owner_id"), Ok(()));
    }

    #[test]
    fn a_limit_only_lowers_a_bound() {
        // A LIMIT above one, one the build cannot read and an OFFSET remove rows and add none:
        // the key's bound stands.
        for sql in [
            "SELECT title FROM todos WHERE id = $1 LIMIT 5",
            "SELECT title FROM todos WHERE id = $1 LIMIT $2",
            "SELECT title FROM todos WHERE id = $1 LIMIT $2 OFFSET $3",
            "SELECT title FROM todos WHERE id = $1 LIMIT ALL",
            "SELECT title FROM todos WHERE id = $1 ORDER BY title FETCH FIRST 3 ROWS WITH TIES",
            "SELECT count(*) AS n FROM todos LIMIT $1",
            "SELECT count(*) AS n FROM todos OFFSET $1",
            "SELECT id FROM users WHERE id = $1 INTERSECT SELECT owner_id FROM todos LIMIT 10",
            "SELECT s.title FROM (SELECT title FROM todos WHERE id = $1 LIMIT 5) s",
        ] {
            assert_eq!(pg_most(sql), Ok(()), "{sql}");
            refused(pg(sql), "", sql);
        }
        // Exactly one row stays one under a constant LIMIT of a row or more and no OFFSET; LIMIT 0,
        // a parameter (0, or NULL for none) and an OFFSET can drop it.
        for sql in [
            "SELECT count(*) AS n FROM todos LIMIT 5",
            "SELECT count(*) AS n FROM todos LIMIT 1",
            "SELECT count(*) AS n FROM todos LIMIT ALL",
            "SELECT count(*) AS n FROM todos LIMIT NULL",
            "SELECT count(*) AS n FROM todos LIMIT 5 OFFSET 0",
            "SELECT count(*) AS n FROM todos ORDER BY 1 FETCH FIRST 1 ROWS WITH TIES",
            "SELECT 1 AS x LIMIT 1",
        ] {
            assert_eq!(pg(sql), Ok(()), "{sql}");
        }
        for sql in [
            "SELECT count(*) AS n FROM todos LIMIT 0",
            "SELECT count(*) AS n FROM todos LIMIT $1",
            "SELECT count(*) AS n FROM todos LIMIT 1 + 1",
            "SELECT count(*) AS n FROM todos LIMIT 5 OFFSET 1",
            "SELECT count(*) AS n FROM todos LIMIT 5 OFFSET $1",
            "SELECT count(*) AS n FROM todos OFFSET 1",
        ] {
            refused(pg(sql), "LIMIT or an OFFSET", sql);
            assert_eq!(pg_most(sql), Ok(()), "{sql}");
        }
        // SQLite reads the same rule; its negative LIMIT is no limit at all.
        let catalog = Catalog { unique_keys: stock().unique_keys, ..Catalog::default() };
        let most = |sql: &str| at_most_one(sql, Dialect::Sqlite, &Trust::default(), &catalog);
        assert_eq!(most("SELECT title FROM todos WHERE id = ?1 LIMIT 5"), Ok(()));
        assert_eq!(most("SELECT title FROM todos WHERE id = ?1 LIMIT ?2 OFFSET ?3"), Ok(()));
        assert_eq!(lite("SELECT count(*) AS n FROM todos LIMIT 5"), Ok(()));
        refused(lite("SELECT count(*) AS n FROM todos LIMIT ?1"), "LIMIT or an OFFSET", "sqlite");
        refused(most("SELECT title FROM todos LIMIT -1"), "does not equate every column", "a negative limit");
        refused(most("SELECT title FROM todos WHERE owner_id = ?1 LIMIT 5"), "does not equate every column", "sqlite");
    }

    #[test]
    fn a_function_in_from_that_returns_no_set_is_one_row() {
        let mut c = stock();
        c.functions.extend(["now", "one_text", "out_pair"].map(String::from));
        let one = |sql: &str| exactly_one(sql, Dialect::Postgres, &Trust::default(), &c);
        let most = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        for sql in [
            "SELECT * FROM now()",
            "SELECT * FROM pg_catalog.now()",
            "SELECT * FROM CURRENT_DATE",
            "SELECT * FROM one_text($1)",
            "SELECT a, b FROM out_pair($1)",
            "SELECT * FROM one_text($1) WITH ORDINALITY AS t (v, n)",
            "SELECT * FROM ROWS FROM (now(), one_text($1))",
            // Each is one row per row of the other.
            "SELECT * FROM now() a, now() b",
            "SELECT * FROM now() a CROSS JOIN LATERAL one_text(a::text) b",
            "SELECT * FROM now() a JOIN one_text($1) b ON true",
        ] {
            assert_eq!(one(sql), Ok(()), "{sql}");
        }
        for (sql, fragment) in [
            ("SELECT * FROM generate_series(1, 3)", "set-returning function generate_series"),
            ("SELECT * FROM unnest($1::int[]) WITH ORDINALITY", "set-returning function unnest"),
            ("SELECT * FROM ROWS FROM (now(), generate_series(1, 3))", "set-returning function generate_series"),
            // A name the catalog does not have may return a set.
            ("SELECT * FROM mystery($1)", "is not in the catalog"),
            ("SELECT * FROM XMLTABLE('/a' PASSING '<a/>' COLUMNS v text PATH '.') x", "a FROM returns a row per row"),
            ("SELECT * FROM todos TABLESAMPLE SYSTEM (50)", "a FROM returns a row per row"),
            ("SELECT * FROM now() WHERE $1", "can remove its row"),
            ("SELECT * FROM now() a JOIN one_text($1) b ON b = a::text", "an inner join under a condition"),
            ("SELECT u.email, t FROM users u, now() t WHERE u.id = $1", "a FROM returns a row per row"),
        ] {
            refused(one(sql), fragment, sql);
        }
        // A join with one multiplies no row.
        assert_eq!(most("SELECT * FROM now() WHERE $1"), Ok(()));
        assert_eq!(most("SELECT u.email, t FROM users u, now() t WHERE u.id = $1"), Ok(()));
        assert_eq!(most("SELECT u.email, t FROM users u CROSS JOIN LATERAL lower(u.email) t WHERE u.id = $1"), Ok(()));
        assert_eq!(most("SELECT u.email, a, b FROM now() a, users u, one_text(u.email) b WHERE u.id = $1"), Ok(()));
        refused(most("SELECT u.email, t FROM users u, now() t"), "of `u`", "an unpinned relation beside it");
        refused(most("SELECT u.email, g FROM users u, generate_series(1, 2) g WHERE u.id = $1"), "`g` in the FROM", "a set");
        // What it returns is computed for each row it is joined to, so it pins no key.
        refused(most("SELECT u.id FROM one_text($1) t, users u WHERE u.email = t"), "of `u`", "a key pinned to a function's value");
        // Nor is it one value for a DISTINCT or a GROUP BY.
        refused(most("SELECT DISTINCT t FROM users u, now() t"), "", "distinct over a function");
        // SQLite's functions in FROM are table-valued, and no catalog names them.
        refused(exactly_one("SELECT value FROM json_each(?1)", Dialect::Sqlite, &Trust::default(), &Catalog::default()), "is not in the catalog", "sqlite");
        assert!(at_most_one("SELECT value FROM json_each(?1)", Dialect::Sqlite, &Trust::default(), &Catalog::default()).is_err());
    }

    #[test]
    fn a_sequence_is_one_row() {
        let mut c = stock();
        c.single_row.insert("my_sequence".into());
        let one = |sql: &str| exactly_one(sql, Dialect::Postgres, &Trust::default(), &c);
        let most = |sql: &str| at_most_one(sql, Dialect::Postgres, &Trust::default(), &c);
        assert_eq!(one("SELECT last_value FROM my_sequence"), Ok(()));
        assert_eq!(one("SELECT a.last_value, b.is_called FROM my_sequence a, my_sequence b"), Ok(()));
        refused(one("SELECT last_value FROM my_sequence WHERE is_called"), "can remove its row", "a WHERE");
        assert_eq!(most("SELECT last_value FROM my_sequence WHERE is_called"), Ok(()));
        assert_eq!(most("SELECT t.title, s.last_value FROM todos t, my_sequence s WHERE t.id = $1"), Ok(()));
        // A WITH query of the name is not the sequence, and another schema's relation is not it.
        refused(one("WITH my_sequence AS (SELECT id FROM todos) SELECT * FROM my_sequence"), "the WITH query `my_sequence`", "shadowed");
        refused(one("SELECT last_value FROM other.my_sequence"), "a FROM returns a row per row", "another schema");
        // Another session advances it between two reads of it, so it pins no key.
        refused(most("SELECT t.title FROM my_sequence s, todos t WHERE t.id = s.last_value"), "of `t`", "a key pinned to a sequence");
    }

    #[test]
    fn a_from_of_one_row_items_is_one_row() {
        let mut c = stock();
        c.functions.extend(["now", "nextval"].map(String::from));
        let trust = answered(&[]);
        let one = |sql: &str| exactly_one(sql, Dialect::Postgres, &trust, &c);
        let most = |sql: &str| at_most_one(sql, Dialect::Postgres, &trust, &c);
        for sql in [
            // A derived table is what the same query is as a WITH query.
            "SELECT * FROM (SELECT count(*) AS n FROM todos) s",
            "SELECT pk FROM (SELECT nextval('s') AS pk) alias",
            "SELECT s.n FROM (SELECT t.n FROM (SELECT count(*) AS n FROM todos) t) s",
            // A cross join of one-row items, however it is written.
            "WITH a AS (SELECT count(*) AS n FROM todos), b AS (SELECT count(*) AS m FROM users) SELECT a.n, b.m FROM a CROSS JOIN b",
            "WITH a AS (SELECT count(*) AS n FROM todos), b AS (SELECT count(*) AS m FROM users) SELECT a.n, b.m FROM a, b",
            "WITH a AS (SELECT count(*) AS n FROM todos) SELECT x.n, y.n AS m FROM a x JOIN a y ON 1 = 1",
            "WITH a AS (SELECT count(*) AS n FROM todos) SELECT x.n, y.n AS m FROM a x JOIN a y ON true",
            "SELECT a.n, b.m, t FROM (SELECT count(*) AS n FROM todos) a, (SELECT max(id) AS m FROM users) b, now() t",
            "SELECT a.n, b.m FROM (SELECT count(*) AS n FROM todos) a CROSS JOIN LATERAL (SELECT count(*) AS m FROM users WHERE id = a.n) b",
            "WITH ins AS (INSERT INTO todos (title) VALUES ($1) RETURNING id) SELECT ins.id, c.n FROM ins, (SELECT count(*) AS n FROM todos) c",
            // An outer join keeps the row of its preserved side, whatever its condition.
            "SELECT a.n, u.email FROM (SELECT count(*) AS n FROM todos) a LEFT JOIN users u ON u.id = $1",
            "SELECT a.n, u.email FROM users u RIGHT JOIN (SELECT count(*) AS n FROM todos) a ON u.id = $1",
            "WITH ins AS (INSERT INTO todos (title) VALUES ($1) RETURNING id, owner_id) SELECT ins.id, u.email FROM ins LEFT JOIN users u ON u.email = $2",
        ] {
            assert_eq!(one(sql), Ok(()), "{sql}");
        }
        for (sql, fragment) in [
            // An inner join under a condition that can fail, and a WHERE, can drop the row.
            ("WITH a AS (SELECT count(*) AS n FROM todos) SELECT x.n FROM a x JOIN a y ON x.n = y.n + $1", "an inner join under a condition"),
            ("WITH a AS (SELECT count(*) AS n FROM todos) SELECT x.n FROM a x JOIN a y ON 1 = 2", "an inner join under a condition"),
            ("WITH a AS (SELECT count(*) AS n FROM todos) SELECT x.n FROM a x JOIN a y USING (n)", "an inner join under a condition"),
            ("WITH a AS (SELECT count(*) AS n FROM todos) SELECT x.n FROM a x NATURAL JOIN a y", "an inner join under a condition"),
            ("SELECT * FROM (SELECT count(*) AS n FROM todos) s WHERE s.n > $1", "can remove its row"),
            // A FULL JOIN is two rows where the condition fails.
            ("SELECT a.n, b.m FROM (SELECT count(*) AS n FROM todos) a FULL JOIN (SELECT count(*) AS m FROM users) b ON a.n = b.m", "FULL JOIN"),
        ] {
            refused(one(sql), fragment, sql);
            if !fragment.contains("FULL") {
                assert_eq!(most(sql), Ok(()), "{sql}");
            }
        }
        for (sql, fragment) in [
            // The item on the preserved side decides, and the other side must still be one row at most.
            ("SELECT a.n, u.email FROM users u LEFT JOIN (SELECT count(*) AS n FROM todos) a ON true", "a FROM returns a row per row"),
            ("SELECT a.n, u.email FROM (SELECT count(*) AS n FROM todos) a LEFT JOIN users u ON u.name = $1", "of `u`"),
            // An item that may be empty, or many rows.
            ("SELECT * FROM (SELECT id FROM todos WHERE id = $1) s", "the subquery `s` may return no row"),
            ("SELECT * FROM (SELECT count(*) AS n FROM todos GROUP BY owner_id) s", "the subquery `s` may return no row"),
            ("SELECT * FROM (SELECT count(*) AS n FROM todos LIMIT $1) s", "the subquery `s` may return no row"),
            ("WITH RECURSIVE a AS (SELECT 1 AS n) SELECT n FROM a", "recursive"),
            // A subquery is proven on its own, and does not read the body of a WITH query around it.
            ("WITH a AS (SELECT count(*) AS n FROM todos) SELECT s.n FROM (SELECT n FROM a) s", "enclosing statement"),
            ("WITH gone AS (DELETE FROM todos WHERE id = $1 RETURNING id) SELECT id FROM gone", "the WITH query `gone`"),
            // A locking clause can skip the row a table under the outer join holds.
            ("SELECT a.n, u.email FROM (SELECT count(*) AS n FROM todos) a LEFT JOIN users u ON u.id = $1 FOR UPDATE OF u SKIP LOCKED", "locking clause"),
        ] {
            refused(one(sql), fragment, sql);
        }
        // With a user's `=` in the database, `1 = 1` may be theirs.
        c.equality_shadowed = true;
        refused(exactly_one("WITH a AS (SELECT count(*) AS n FROM todos) SELECT x.n FROM a x JOIN a y ON 1 = 1", Dialect::Postgres, &Trust::default(), &c), "an inner join under a condition", "a shadowed =");
        // SQLite reads the same shapes.
        for sql in [
            "SELECT * FROM (SELECT count(*) AS n FROM todos) s",
            "SELECT a.n, b.m FROM (SELECT count(*) AS n FROM todos) a, (SELECT count(*) AS m FROM users) b",
            "SELECT a.n, b.m FROM (SELECT count(*) AS n FROM todos) a JOIN (SELECT count(*) AS m FROM users) b ON 1 = 1",
        ] {
            assert_eq!(lite(sql), Ok(()), "{sql}");
        }
    }

    #[test]
    fn an_ordered_set_aggregate_is_an_aggregate() {
        let mut c = stock();
        c.functions.extend(["percentile_disc", "mode", "rank", "my_median"].map(String::from));
        let one = |sql: &str| exactly_one(sql, Dialect::Postgres, &Trust::default(), &c);
        for sql in [
            "SELECT percentile_disc(0.5) WITHIN GROUP (ORDER BY title) FROM todos",
            "SELECT mode() WITHIN GROUP (ORDER BY title) FROM todos WHERE owner_id = $1",
            "SELECT rank(1) WITHIN GROUP (ORDER BY id) FROM todos",
            "SELECT coalesce(mode() WITHIN GROUP (ORDER BY title), 'none') AS top FROM todos",
            // A user's ordered-set aggregate takes the clause too, and nothing else does.
            "SELECT public.my_median(0.5) WITHIN GROUP (ORDER BY id) FROM todos",
        ] {
            assert_eq!(one(sql), Ok(()), "{sql}");
        }
        refused(one("SELECT mode() WITHIN GROUP (ORDER BY title) FROM todos GROUP BY owner_id"), "GROUP BY", "grouped");
        refused(one("SELECT mode() WITHIN GROUP (ORDER BY title), generate_series(1, 2) FROM todos"), "set-returning function", "beside a set");
        refused(lite("SELECT mode() WITHIN GROUP (ORDER BY title) FROM todos"), "a FROM returns a row per row", "sqlite");
    }

    #[test]
    fn a_select_over_one_with_query_returns_that_querys_rows_at_most() {
        // One row in, one row out; a WHERE over it can only remove the row.
        assert_eq!(pg("WITH one AS (SELECT 1 AS id) SELECT id FROM one"), Ok(()));
        refused(pg("WITH one AS (SELECT 1 AS id) SELECT id FROM one WHERE id = $1"), "can remove its row", "a WHERE over a one-row WITH");
        assert_eq!(pg_most("WITH one AS (SELECT 1 AS id) SELECT id FROM one WHERE id = $1"), Ok(()));
        // A one-row INSERT with RETURNING, and a write by key.
        assert_eq!(pg("WITH ins AS (INSERT INTO todos (title) VALUES ($1) RETURNING id) SELECT id FROM ins"), Ok(()));
        assert_eq!(pg_most("WITH gone AS (DELETE FROM todos WHERE id = $1 RETURNING id) SELECT id FROM gone"), Ok(()));
        refused(pg_most("WITH gone AS (DELETE FROM todos WHERE owner_id = $1 RETURNING id) SELECT id FROM gone"), "the WITH query `gone`", "a write that is not by key");
        // A key read through the WITH query.
        assert_eq!(pg_most("WITH mine AS (SELECT * FROM todos WHERE id = $1) SELECT title FROM mine"), Ok(()));
        // The GROUP BY proof over a joined relation still refuses a column of the other side.
        refused(pg_most("SELECT t.title, count(*) AS n FROM users u JOIN todos t ON t.owner_id = u.id WHERE u.id = $1 GROUP BY t.title"), "GROUP BY", "a GROUP BY on the joined relation");
    }

    #[test]
    fn the_same_proof_runs_on_sqlite_against_the_keys_sqlite_reports() {
        let catalog = Catalog { unique_keys: stock().unique_keys, ..Catalog::default() };
        let lite = |sql: &str| at_most_one(sql, Dialect::Sqlite, &Trust::default(), &catalog);
        assert_eq!(lite("SELECT title FROM todos WHERE id = ?1"), Ok(()));
        assert_eq!(lite("SELECT title FROM todos WHERE owner_id = ?1 LIMIT 1"), Ok(()));
        refused(lite("SELECT title FROM todos WHERE owner_id = ?1"), "does not equate every column", "sqlite");
        // With no key read at all, nothing is proven — which is the fail-closed direction.
        refused(
            at_most_one("SELECT title FROM todos WHERE id = ?1", Dialect::Sqlite, &Trust::default(), &Catalog::default()),
            "no unique index",
            "empty catalog",
        );
    }

    #[test]
    fn only_a_constant_limit_zero_proves_there_is_no_row() {
        for sql in [
            "SELECT 1 AS x LIMIT 0",
            "SELECT 1 AS x FETCH FIRST 0 ROWS ONLY",
            "SELECT id FROM todos WHERE owner_id = $1 LIMIT 0",
            "SELECT 1 AS x UNION SELECT 2 LIMIT 0",
        ] {
            assert_eq!(never_a_row(sql, Dialect::Postgres), Some("LIMIT 0 returns no rows"), "{sql}");
        }
        assert_eq!(never_a_row("SELECT id FROM todos WHERE owner_id = ?1 LIMIT 0 OFFSET ?2", Dialect::Sqlite), Some("LIMIT 0 returns no rows"));
        for sql in ["SELECT 1 AS x", "SELECT 1 AS x LIMIT 1", "SELECT 1 AS x LIMIT $1", "SELECT 1 AS x LIMIT ALL", "SELECT 1 AS x OFFSET 1", "SELECT 1 AS x WHERE false"] {
            assert_eq!(never_a_row(sql, Dialect::Postgres), None, "{sql}");
        }
    }
}
