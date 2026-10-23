//! The Postgres analysis, round by round.
//!
//! The statement's placeholders are rewritten to `$n` and the host describes it. From the
//! describe come the parameter types and the result columns; from the catalog, `NOT NULL` for
//! every column with provenance; from the parse tree, the outer joins that null a table's
//! columns, the expression rules ([`crate::infer`]), the JSON constructors
//! ([`crate::jsonshape`]) and the row-count proof ([`crate::cardinality`]). Everything that needs
//! the database goes through [`Host`], which records what it cannot answer yet; `analyze` then
//! returns `None`, the host answers, and the next call replays with more.

use crate::analysis::{bare, bare_of, constant_key, diag, lexed, KeyClause};
use crate::analyze::{self, Analysis as Parsed};
use crate::api::{Analysis, Column, DbError, Diagnostic, Param, RawDescribe, Scope};
use crate::cardinality::{self, Catalog};
use crate::dialect::Dialect;
use crate::host::{as_bool, as_str, as_strings, as_u32, Host, Lazy};
use crate::infer::{self, Trust};
use crate::jsonshape;
use crate::levels::Columns;
use crate::lex::{self, Lexed, Rewritten, Target};
use crate::pgtypes::{not_null_rows, TypeRegistry, RELATION_TYPES_QUERY, TYPE_QUERY};
use crate::returning::Rows;
use crate::types::{DescribedColumn, DescribedStatement, Type};
use pg_query::protobuf::node::Node as NodeEnum;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};

/// How many statements one analysis may synthesize to settle its columns: one per set-operation
/// arm, one per statement whose expressions read a column, one per JSON constructor context, one
/// per WITH query or derived table a column is read from. Each is a describe, asked together with
/// the others ([`Engine::prewarm_tree`]), so the cap bounds the work rather than the rounds. A
/// statement of a hundred lines with four WITH queries, a derived table and two JSON columns takes
/// thirteen; past the cap a column is as nullable as the probes left unsettled leave it.
const MAX_PROBES: usize = 24;
/// How many times one analysis may settle a probe at all, the same statement under another
/// assumption included.
const MAX_PROBE_CALLS: usize = 64;

// The catalog queries below are public so that a host with a driver of its own can bind their
// parameters by type: each casts every parameter in its text, and the doc comment names them.

/// `$1` is `oid[]` of relations, `$2` the `int2[]` of attribute numbers beside them.
pub const ATTNOTNULL_QUERY: &str = concat!(
    "SELECT a.attrelid::int8, a.attnum::int4 FROM pg_attribute a \
    JOIN unnest($1::oid[], $2::int2[]) AS u(rel, att) ON a.attrelid = u.rel AND a.attnum = u.att WHERE a.attnotnull AND ",
    not_null_rows!("a.attrelid", "a.attname")
);
/// `$1` is `oid[]` of relations, `$2` the `int2[]` of attribute numbers beside them: one row per
/// validated, enforced CHECK on such a column, the table's on that column alone (and inherited by
/// any child a scan reads) or the column's domain's, where the column holds `text` or `varchar`
/// under a deterministic collation, or `json` or `jsonb`: `(relation, attnum, the column's name,
/// the CHECK as the server prints it, and per function the CHECK calls, its name, its extension's
/// name and that extension's version, in three arrays)`. `conenforced` is read through `to_jsonb`,
/// since only PostgreSQL 18 has the column. A table's CHECK says nothing where the table, or a
/// partition or child a scan reads, is a foreign table, whose rows the server never checks; a
/// domain's input function checks every value. Nothing is read under `standard_conforming_strings`
/// off, where the server prints a string's backslashes doubled and the parser would read them so.
pub const CHECKS_QUERY: &str = "SELECT a.attrelid::int8, a.attnum::int4, a.attname::text, pg_catalog.pg_get_expr(k.conbin, k.conrelid, true), \
      f.names, f.extensions, f.versions \
    FROM unnest($1::oid[], $2::int2[]) AS u(rel, att) \
    JOIN pg_attribute a ON a.attrelid = u.rel AND a.attnum = u.att AND NOT a.attisdropped \
    JOIN pg_type t ON t.oid = a.atttypid \
    JOIN pg_constraint k ON k.contype = 'c' AND k.convalidated AND coalesce((pg_catalog.to_jsonb(k) ->> 'conenforced')::bool, true) \
      AND ((k.conrelid = a.attrelid AND k.conkey = ARRAY[a.attnum]::int2[] \
          AND (NOT k.connoinherit OR NOT EXISTS (SELECT 1 FROM pg_inherits h WHERE h.inhparent = a.attrelid)) \
          AND NOT EXISTS (WITH RECURSIVE tree(rel) AS (SELECT a.attrelid UNION SELECT i.inhrelid FROM tree t JOIN pg_inherits i ON i.inhparent = t.rel) \
            SELECT 1 FROM tree t JOIN pg_class f ON f.oid = t.rel WHERE f.relkind = 'f')) \
        OR (t.typtype = 'd' AND k.contypid = t.oid)) \
    LEFT JOIN LATERAL (SELECT pg_catalog.array_agg(p.proname::text ORDER BY p.oid) AS names, \
        pg_catalog.array_agg(coalesce(e.extname::text, '') ORDER BY p.oid) AS extensions, pg_catalog.array_agg(coalesce(e.extversion, '') ORDER BY p.oid) AS versions \
      FROM pg_depend d JOIN pg_proc p ON p.oid = d.refobjid \
      LEFT JOIN pg_depend x ON x.classid = 'pg_catalog.pg_proc'::pg_catalog.regclass AND x.objid = p.oid AND x.deptype = 'e' \
      LEFT JOIN pg_extension e ON e.oid = x.refobjid \
      WHERE d.classid = 'pg_catalog.pg_constraint'::pg_catalog.regclass AND d.objid = k.oid AND d.refclassid = 'pg_catalog.pg_proc'::pg_catalog.regclass) f ON true \
    WHERE pg_catalog.current_setting('standard_conforming_strings') = 'on' \
      AND ((CASE WHEN t.typtype = 'd' THEN t.typbasetype ELSE t.oid END) IN ('pg_catalog.text'::pg_catalog.regtype, 'pg_catalog.varchar'::pg_catalog.regtype) \
        AND coalesce((SELECT o.collisdeterministic FROM pg_collation o WHERE o.oid = a.attcollation), true) \
        OR (CASE WHEN t.typtype = 'd' THEN t.typbasetype ELSE t.oid END) IN ('pg_catalog.json'::pg_catalog.regtype, 'pg_catalog.jsonb'::pg_catalog.regtype))";
/// Whether each column of the named relations refuses null, for a parameter written to it
/// ([`Engine::nullable_params`]): `(relation, attnum, column, whether its NOT NULL or a domain of
/// its type's declared NOT NULL refuses it, or the relation is a view, whose columns say nothing
/// of what the table or the rule or trigger behind it takes, the enforced CHECKs of the table that read the column,
/// the CHECKs of every domain of its type)`, each CHECK as the server prints it. A CHECK refuses
/// null where it tests the column `IS NOT NULL` among its conjuncts ([`refuses_null`]); a NOT VALID
/// one still checks every row written. `$1` is `text[]` of relation names.
pub const REFUSES_NULL_QUERY: &str = "WITH RECURSIVE domains(attrelid, attnum, oid) AS ( \
      SELECT a.attrelid, a.attnum, t.oid FROM pg_class c JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
        JOIN pg_type t ON t.oid = a.atttypid AND t.typtype = 'd' \
      WHERE c.relname = ANY($1::text[]) AND pg_catalog.pg_table_is_visible(c.oid) \
      UNION SELECT d.attrelid, d.attnum, t.oid FROM domains d JOIN pg_type b ON b.oid = d.oid JOIN pg_type t ON t.oid = b.typbasetype AND t.typtype = 'd') \
    SELECT c.relname::text, a.attnum::int4, a.attname::text, \
      a.attnotnull OR c.relkind IN ('v', 'm') OR EXISTS (SELECT 1 FROM domains d JOIN pg_type t ON t.oid = d.oid WHERE d.attrelid = a.attrelid AND d.attnum = a.attnum AND t.typnotnull), \
      coalesce((SELECT pg_catalog.array_agg(pg_catalog.pg_get_expr(k.conbin, k.conrelid, true)) FROM pg_constraint k \
        WHERE k.contype = 'c' AND k.conrelid = c.oid AND a.attnum = ANY(k.conkey) AND coalesce((pg_catalog.to_jsonb(k) ->> 'conenforced')::bool, true)), '{}'::text[]), \
      coalesce((SELECT pg_catalog.array_agg(pg_catalog.pg_get_expr(k.conbin, 0, true)) FROM domains d JOIN pg_constraint k ON k.contypid = d.oid AND k.contype = 'c' \
        WHERE d.attrelid = a.attrelid AND d.attnum = a.attnum), '{}'::text[]) \
    FROM pg_class c JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
    WHERE c.relname = ANY($1::text[]) AND pg_catalog.pg_table_is_visible(c.oid)";
/// The comment of each column that has one (`COMMENT ON COLUMN`): `(relation, attnum, the
/// comment)`. `$1` is `oid[]` of relations, `$2` the `int2[]` of attribute numbers beside them.
pub const COMMENTS_QUERY: &str = "SELECT u.rel::int8, u.att::int4, d.description FROM unnest($1::oid[], $2::int2[]) AS u(rel, att) \
    JOIN pg_description d ON d.objoid = u.rel AND d.classoid = 'pg_catalog.pg_class'::pg_catalog.regclass AND d.objsubid = u.att";
/// `$1` is `oid[]` of relations.
pub const RELNAMES_QUERY: &str = "SELECT c.oid::int8, c.relname::text FROM pg_class c WHERE c.oid = ANY($1::oid[])";
/// The columns of the relation an INSERT writes (`$1`, its name as the statement writes it), in
/// attnum order, each with whether a row must write it: `NOT NULL`, its own or a domain's along the
/// domain's chain, and nothing to fill it, neither a default, an identity, a generation expression
/// nor a domain's default. A `NOT NULL` is the one a write meets, valid or not.
pub const REQUIRED_COLUMNS_QUERY: &str = concat!(
    "WITH RECURSIVE cols AS (SELECT a.attname, a.attnum, a.attnotnull, a.atthasdef, a.attidentity, a.attgenerated, a.atttypid \
      FROM pg_catalog.pg_attribute a WHERE a.attrelid = ",
    crate::cardinality::relation_named!("$1::text"),
    " AND a.attnum > 0 AND NOT a.attisdropped), \
    chain(attnum, typ, depth) AS (SELECT attnum, atttypid, 0 FROM cols \
      UNION ALL SELECT c.attnum, t.typbasetype, c.depth + 1 FROM chain c JOIN pg_catalog.pg_type t ON t.oid = c.typ WHERE t.typtype = 'd' AND c.depth < 32) \
    SELECT cols.attname::text, cols.attnum::int4, \
      ((cols.attnotnull OR coalesce(bool_or(t.typnotnull), false)) AND NOT cols.atthasdef AND cols.attidentity = '' AND cols.attgenerated = '' \
        AND NOT coalesce(bool_or(t.typdefaultbin IS NOT NULL), false)) AS required \
    FROM cols LEFT JOIN chain ON chain.attnum = cols.attnum LEFT JOIN pg_catalog.pg_type t ON t.oid = chain.typ AND t.typtype = 'd' \
    GROUP BY cols.attname, cols.attnum, cols.attnotnull, cols.atthasdef, cols.attidentity, cols.attgenerated ORDER BY cols.attnum"
);
/// Every column of the relations the parse names, by name, so provenance and `NOT NULL` are
/// settled without a second round for the tables the statement itself reads. `$1` is `text[]` of
/// relation names. The fourth column is the `NOT NULL` a write meets, which checks every new row
/// whether or not the constraint is valid; the seventh is the one a read may trust (see
/// `not_null_rows`).
pub const RELATION_COLUMNS_QUERY: &str = concat!(
    "SELECT c.oid::int8, c.relname::text, a.attnum::int4, a.attnotnull, \
    CASE WHEN c.relkind IN ('v', 'm') AND a.attnum = 1 THEN pg_catalog.pg_get_viewdef(c.oid) END, a.attname::text, \
    a.attnotnull AND ",
    not_null_rows!("c.oid", "a.attname"),
    " FROM pg_class c JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
    WHERE c.relname = ANY($1::text[]) AND pg_catalog.pg_table_is_visible(c.oid)"
);
/// Which of the names the expression rules read a user or an extension has defined (`false`
/// beside the name), and which built-in types a cast may target (`true`; see
/// [`crate::infer::Trust`]). `$1` is `text[]` of operator names, `$2` the `text[]` of function
/// names. One more row, a NULL beside [`JSONB_CASTS_RAISE`], comes from a server before 18, whose
/// cast of a JSON null to a number or a boolean raises an error where 18 answers NULL.
pub const TRUST_QUERY: &str = "SELECT false, o.oprname::text FROM pg_operator o WHERE o.oid >= 16384 AND o.oprname::text = ANY($1::text[]) \
    UNION ALL SELECT false, p.proname::text FROM pg_proc p WHERE p.oid >= 16384 AND p.proname::text = ANY($2::text[]) \
    UNION ALL SELECT true, t.typname::text FROM pg_type t WHERE t.oid < 16384 AND t.typnamespace = 'pg_catalog'::regnamespace \
      AND NOT EXISTS (SELECT 1 FROM pg_type u WHERE u.oid >= 16384 AND u.typname = t.typname) \
      AND NOT EXISTS (SELECT 1 FROM pg_cast c WHERE c.oid >= 16384 AND c.casttarget = t.oid AND c.castmethod = 'f') \
    UNION ALL SELECT NULL, 'jsonb casts raise' WHERE pg_catalog.current_setting('server_version_num')::int4 < 180000";
/// The name of [`TRUST_QUERY`]'s row that says the server is one before PostgreSQL 18.
const JSONB_CASTS_RAISE: &str = "jsonb casts raise";

/// `$1` is `text[]` of relation names as the statement writes them: one row per unique index of
/// each that is a plain or a partitioned table, `(the name, the index's key columns with NULL for
/// a key part that is an expression, the index's predicate as the server prints it)`, and for such
/// a table with no unique index one row of the name beside NULLs. These are the indexes an `ON
/// CONFLICT (…)` may infer as its arbiter (the planner's `infer_arbiter_indexes`), and more: an
/// invalid one is listed too, so a missing row is a missing index. The index of a key `WITHOUT
/// OVERLAPS` (PostgreSQL 18) is unique by name and an exclusion constraint in fact, which the
/// planner infers for no column list, so it is none of them. A view, whose write goes to another
/// relation, has no row.
pub const ARBITERS_QUERY: &str = concat!(
    "SELECT x.name, \
      (SELECT array_agg(a.attname::text ORDER BY k.ord) FROM unnest(i.indkey::int2[]) WITH ORDINALITY AS k(attnum, ord) \
        LEFT JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = k.attnum AND k.attnum > 0 WHERE k.ord <= i.indnkeyatts), \
      pg_catalog.pg_get_expr(i.indpred, i.indrelid, true) \
    FROM unnest($1::text[]) AS x(name) \
    JOIN pg_class c ON c.oid = ",
    cardinality::relation_named!("x.name"),
    " AND c.relkind IN ('r', 'p') \
    LEFT JOIN pg_index i ON i.indrelid = c.oid AND i.indisunique AND NOT i.indisexclusion"
);

/// The analysis of a Postgres statement as its author wrote it: lexed, its structured parameters
/// expanded, and replayed through `host`. The direct entry for a host that links this crate
/// (`pgreylo`, which calls it from inside the server, so it never probes for the extension).
pub fn analyze(sql: &str, host: &Host) -> Lazy<Analysis> {
    host.on_engine_stack(|host| analyze_here(sql, host))
}

fn analyze_here(sql: &str, host: &Host) -> Lazy<Analysis> {
    crate::structured::analyze(sql, Dialect::Postgres, host, &|sql, marked| analyze_plain(sql, host, marked), &|diagnostics| bare_of(sql, Dialect::Postgres, diagnostics))
}

/// The analysis of a statement without structured parameters; `marked` names the parameters cast
/// to `nullable(…)`.
fn analyze_plain(sql: &str, host: &Host, marked: &[String]) -> Lazy<Analysis> {
    let lexed = match lexed(sql, Dialect::Postgres) {
        Ok(l) => l,
        Err(bare) => return Some(bare),
    };
    let engine = Engine::new(host);
    engine.run(sql, &lexed, marked)
}

/// The described statement alone, for a host that renders its own types and reads its own JSON
/// shapes and row counts (the framework's `describe.rs`): the parameter types in `$n` order and
/// each result column's type and nullability, settled the way [`analyze`] settles them. `sql` is
/// already in the server's `$n` form and is described as it is, and a parameter may be null,
/// because such a host binds whatever its caller sends. `Some(Err)` is the server's refusal of
/// the statement itself.
pub fn describe(sql: &str, host: &Host) -> Lazy<Result<DescribedStatement, DbError>> {
    host.on_engine_stack(|host| {
        let engine = Engine::new(host);
        let parsed = analyze::analyze(sql).ok();
        let trust = trust(host);
        let relations = relations_of(parsed.as_ref());
        engine.described(sql, parsed.as_ref(), &relations, trust.as_ref(), &[]).map(|r| r.map(|(_, described)| described))
    })
}

/// Which of the names the expression rules read are the built-ins, as the catalog says: one
/// session-scoped query.
pub fn trust(host: &Host) -> Lazy<Trust> {
    let mut operators: Vec<&str> = infer::POSTGRES_OPERATORS.to_vec();
    operators.extend(infer::POSTGRES_STRICT_OPERATORS.iter().filter(|o| !infer::POSTGRES_OPERATORS.contains(o)));
    let operators: Vec<Value> = operators.into_iter().map(|s| json!(s)).collect();
    let functions: Vec<Value> = infer::postgres_function_names().into_iter().map(|s| json!(s)).collect();
    let rows = host.query(TRUST_QUERY, vec![json!(operators), json!(functions)], Scope::Session)?;
    let mut trust = Trust { shadowed: HashSet::new(), cast_targets: Some(HashSet::new()), ..Default::default() };
    if let Ok(rows) = rows {
        for row in rows {
            let Some(name) = row.get(1).and_then(as_str) else { continue };
            match row.first().and_then(as_bool) {
                Some(true) => {
                    trust.cast_targets.get_or_insert_with(HashSet::new).insert(name.to_string());
                }
                Some(false) => {
                    trust.shadowed.insert(name.to_string());
                }
                None => trust.jsonb_casts_raise |= name == JSONB_CASTS_RAISE,
            }
        }
    }
    Some(trust)
}

/// The catalog the row-count proof reads for a statement that calls `functions` and reads
/// `relations`: which of those functions return a set, the unique keys of those relations, and
/// every operator a set-returning function implements.
pub fn catalog(host: &Host, functions: &[String], relations: &[String]) -> Lazy<Catalog> {
    let operators = set_returning_operators(host);
    let rows = catalog_rows(host, functions, &sorted(relations.to_vec()));
    let trust = trust(host);
    let (Some(operators), Some(rows), Some(trust)) = (operators, rows, trust) else { return None };
    let mut catalog = build_catalog(&operators, rows);
    trusted(&mut catalog, &trust);
    Some(catalog)
}

/// The columns of the relations `sql` reads, for a statement with a scalar subquery, where an
/// aggregate is the subquery's own only if a column it reads is one of the subquery's FROM
/// ([`crate::levels`]); no columns, and no question to the host, for any other statement. `sql`
/// is in the server's `$n` form. For the main statement this is the question the row-count proof
/// asks of the same relations, so it costs no query of its own.
fn level_columns(sql: &str, host: &Host) -> Lazy<Columns> {
    columns_where(crate::levels::needed(sql), sql, host)
}

/// The columns [`jsonshape::plan`] reads for `sql`, for a host that plans the shapes itself: none,
/// and no question, for a statement whose shapes do not turn on them.
pub fn shape_columns(sql: &str, host: &Host) -> Lazy<Columns> {
    host.on_engine_stack(|host| columns_where(jsonshape::needs_columns(sql), sql, host))
}

fn columns_where(needed: bool, sql: &str, host: &Host) -> Lazy<Columns> {
    if !needed {
        return Some(Columns::default());
    }
    let relations = relations_of(analyze::analyze(sql).ok().as_ref());
    if relations.is_empty() {
        return Some(Columns::default());
    }
    let rows = host.query(cardinality::COLUMNS_QUERY, vec![json!(relations)], Scope::Statement)?;
    Some(Columns::from_rows(&rows.unwrap_or_default()))
}

/// What the trust check says that the row-count proof reads: whether `=` is only the built-in
/// one, which operators a user defined, and which types no user's cast function targets.
fn trusted(catalog: &mut Catalog, trust: &Trust) {
    catalog.equality_shadowed = !trust.builtin("=");
    catalog.user_operators = trust.shadowed.clone();
    catalog.cast_targets = trust.cast_targets.clone();
}

fn set_returning_operators(host: &Host) -> Lazy<HashSet<String>> {
    let rows = host.query(cardinality::SET_RETURNING_OPERATORS_QUERY, vec![], Scope::Session)?;
    Some(rows.unwrap_or_default().iter().filter_map(|r| r.first().and_then(as_str).map(String::from)).collect())
}

/// The rows the row-count proof's catalog is built from: the unique keys, columns and skipping
/// triggers of `relations`, and which of `functions` return a set or are volatile.
fn catalog_rows(host: &Host, functions: &[String], relations: &[String]) -> Lazy<CatalogRows> {
    let none = || Some(Ok(Vec::new()));
    let keys = if relations.is_empty() { none() } else { host.query(cardinality::UNIQUE_KEYS_QUERY, vec![json!(relations)], Scope::Statement) };
    let columns = if relations.is_empty() { none() } else { host.query(cardinality::COLUMNS_QUERY, vec![json!(relations)], Scope::Statement) };
    let functions = if functions.is_empty() { none() } else { host.query(cardinality::FUNCTIONS_QUERY, vec![json!(functions)], Scope::Statement) };
    let triggers = if relations.is_empty() { none() } else { host.query(cardinality::TRIGGERS_QUERY, vec![json!(relations)], Scope::Statement) };
    let (Some(keys), Some(columns), Some(functions), Some(triggers)) = (keys, columns, functions, triggers) else { return None };
    Some(CatalogRows {
        keys: keys.unwrap_or_default(),
        columns: columns.unwrap_or_default(),
        functions: functions.unwrap_or_default(),
        triggers: triggers.unwrap_or_default(),
    })
}

/// The rows of the row-count proof's catalog queries, as the host answered them.
struct CatalogRows {
    keys: Vec<Vec<Value>>,
    columns: Vec<Vec<Value>>,
    functions: Vec<Vec<Value>>,
    triggers: Vec<Vec<Value>>,
}

/// The comments the main statement's types carry ([`Engine::comments`]).
#[derive(Default)]
struct Comments {
    /// Per result column, its column's comment.
    columns: Vec<Option<String>>,
    /// Per column a parameter is written to, by relation and attribute, its comment.
    written: HashMap<(u32, i32), String>,
}

struct Engine<'a> {
    host: &'a Host<'a>,
    registry: RefCell<TypeRegistry>,
    /// The parse-named relations' columns: relation oid → (name, attnum → NOT NULL).
    relations: RefCell<HashMap<u32, (String, HashMap<i32, bool>)>>,
    /// The same columns by relation name, in attnum order: column name → NOT NULL. An INSERT
    /// or an UPDATE names the columns it writes, or their positions.
    columns_by_name: RefCell<HashMap<String, BTreeMap<i32, (String, bool)>>>,
    /// The defining SELECT of each of those that is a view or a materialized view. The catalog
    /// holds no NOT NULL for a view's columns, so they are settled from this statement instead.
    views: RefCell<HashMap<u32, String>>,
    /// The WITH queries being settled, outermost first: the statement text of each, the columns
    /// taken as nullable for a read of the query from inside itself, and whether such a read
    /// happened. See `with_columns`.
    with_stack: RefCell<Vec<(String, HashSet<String>, bool)>>,
    /// The statements probed so far. The budget is on distinct statements, which is what costs
    /// a describe; settling one again under another assumption costs nothing.
    probed: RefCell<HashSet<String>>,
    /// The statements whose describe was asked ahead ([`Engine::prewarm_tree`]), the analyzed
    /// statement first: one set for the columns' probes and the JSON shapes', as the budget is.
    prewarmed: RefCell<HashSet<String>>,
    /// The statements being probed right now, outermost first. A statement that comes to probe
    /// itself (a whole-row leaf reads as a whole row in its own leaf probe) proves nothing by it.
    probing: RefCell<Vec<String>>,
    /// Every `probe` call so far, whatever the statement: a bound on the work one analysis does.
    probe_calls: Cell<usize>,
    /// What each probe settled to, for the rest of the analysis: settled again, a probe would
    /// settle every probe it reads again, which a chain of WITH queries doubles at each link.
    settled: RefCell<HashMap<String, Option<DescribedStatement>>>,
    /// How many times a probe met itself among the probes being settled ([`Engine::probing`]),
    /// or a WITH query read itself and took the assumption it is being settled under
    /// ([`Engine::with_stack`]): an outcome that rests on either is not kept.
    assumptions: Cell<usize>,
    /// Per placeholder of the main statement, the cast a probe pins on it.
    casts: RefCell<Vec<Option<String>>>,
    /// Whether `casts` holds the statement's describe's answer yet.
    pinned: Cell<bool>,
    /// The JSON Schemas the CHECKs of each column a parameter is written to hold it to, with the
    /// validators that read them ([`Engine::checks`]).
    written: RefCell<HashMap<(u32, i32), Vec<(String, Validators)>>>,
    /// The comment of each result column of the main statement and of each column it writes a
    /// parameter to ([`Engine::comments`]).
    comments: RefCell<Comments>,
    budget: Cell<usize>,
}

/// The validators one CHECK's call is read by ([`crate::schema_type::postgres_validator`]).
type Validators = &'static [&'static crate::schema_type::Validator];

/// How many WITH queries or derived tables deep a JSON column is followed into the statement that
/// builds it: `WITH a AS (…), b AS (SELECT j FROM a) SELECT j FROM b` is two.
const MAX_ITEM_DEPTH: usize = 4;

/// The parameters the caller may pass null for, by number, and each one cast to `nullable(…)`
/// that is written whole to a column refusing null ([`Engine::nullable_params`]).
struct Nullable {
    params: HashSet<i32>,
    refused: Vec<(i32, String, String)>,
}

impl<'a> Engine<'a> {
    fn new(host: &'a Host<'a>) -> Engine<'a> {
        Engine { host, registry: RefCell::new(TypeRegistry::with_builtins()), relations: RefCell::new(HashMap::new()), columns_by_name: RefCell::new(HashMap::new()), views: RefCell::new(HashMap::new()), with_stack: RefCell::new(Vec::new()), probed: RefCell::new(HashSet::new()), prewarmed: RefCell::new(HashSet::new()), probing: RefCell::new(Vec::new()), probe_calls: Cell::new(0), settled: RefCell::new(HashMap::new()), assumptions: Cell::new(0), casts: RefCell::new(Vec::new()), pinned: Cell::new(false), written: RefCell::new(HashMap::new()), comments: RefCell::new(Comments::default()), budget: Cell::new(MAX_PROBES) }
    }

    fn run(&self, sql: &str, lexed: &Lexed, marked: &[String]) -> Lazy<Analysis> {
        let mut diagnostics = Vec::new();
        let rewritten = lex::rewrite(sql, lexed, Target::Dollar);
        // The parameters cast to `nullable(…)`, by the number the rewrite gave each.
        let marked: HashSet<i32> = rewritten.order.iter().enumerate().filter(|(_, name)| marked.contains(name)).map(|(k, _)| k as i32 + 1).collect();
        let parsed = match analyze::analyze(&rewritten.sql) {
            Ok(p) => Some(p),
            // A statement of a kind the analysis does not read (DDL, a session setting, EXPLAIN)
            // is a command, typed from the describe alone. A file of several statements, which
            // the server would refuse to describe, says what a typed file holds and how to leave
            // a migration out of the type check, as on the other databases. `AnalyzeError` words
            // both for Reylo Framework, which has no tsconfig.json to leave a file out of.
            Err(analyze::AnalyzeError::UnsupportedStatement) => return self.command(sql, lexed, rewritten),
            Err(analyze::AnalyzeError::NotOneStatement(n)) if n > 1 => {
                diagnostics.push(diag(lex::several_statements(), 0, sql.trim_end().len(), 10));
                return Some(bare(sql, lexed, Dialect::Postgres, diagnostics));
            }
            Err(_) => None,
        };
        // A parameter as a whole sort or group key is an error the parse alone finds, reported
        // beside whatever the server says of the statement.
        diagnostics.extend(constant_key_diagnostics(sql, lexed, &rewritten));
        // The first round asks for everything at once: the session facts, the describe, and
        // what the parse alone names — the relations' columns, column types and unique keys,
        // the functions, and every probe that carries no placeholder. Only what depends on the
        // describe (a type it reports that no named relation holds, a probe that pins a
        // parameter's type) waits for a second round.
        let relations = relations_of(parsed.as_ref());
        // A parameter's TypeScript type carries null only where the statement lets the caller
        // pass one (see `nullable_params`); every other parameter is a non-null value.
        let nullable = self.nullable_params(parsed.as_ref(), &relations, &marked);
        // A parameter cast to `nullable(…)` and written whole to a column that refuses null would
        // be refused at run time for the null its type allows: an error at the parameter.
        for (n, relation, column) in nullable.iter().flat_map(|n| &n.refused) {
            let Some(at) = rewritten.order.get(*n as usize - 1).and_then(|name| lexed.placeholders.iter().find(|p| &p.name == name)) else { continue };
            let written = &sql[at.start..at.end];
            diagnostics.push(diag(format!("The parameter {written} is cast to nullable(…), but it is written to {relation}.{column}, which refuses null: cast it to its type alone."), at.start, at.end - at.start, crate::structured::CODE));
        }
        let nullable = nullable.map(|n| n.params);
        let trust = trust(self.host).zip(nullable.clone()).map(|(mut trust, nullable)| {
            trust.non_null_params = true;
            trust.nullable_params = nullable;
            trust
        });
        let operators = set_returning_operators(self.host);
        let written = self.written_columns(parsed.as_ref(), &relations);
        let described = match &written {
            Some(written) => self.described(&rewritten.sql, parsed.as_ref(), &relations, trust.as_ref(), &written.iter().map(|(_, rel, att)| (*rel, *att)).collect::<Vec<_>>()),
            None => None,
        };
        let functions: &[String] = parsed.as_ref().map(|p| p.functions.as_slice()).unwrap_or(&[]);
        let catalog_rows = catalog_rows(self.host, functions, &relations);
        for probe in &jsonshape::probes_ahead(&rewritten.sql) {
            self.prewarm(&probe.sql);
        }
        // The JSON shapes' probes, and every probe that settles them, pinned, once the describe
        // has said what they are pinned with: the shapes are read only after the columns are
        // settled, rounds later.
        if let (true, Some(trust), Some(rows)) = (self.pinned.get(), &trust, &catalog_rows) {
            self.prewarmed.borrow_mut().insert(rewritten.sql.clone());
            for probe in jsonshape::plan(&rewritten.sql, Dialect::Postgres, trust, &crate::levels::Columns::from_rows(&rows.columns)).statements() {
                self.prewarm_probe(&probe.sql, trust);
            }
        }
        // The unique indexes of every table an `ON CONFLICT (…)` names its arbiter's columns for.
        let conflicts = conflict_targets(&rewritten.sql);
        let arbiters = if conflicts.is_empty() {
            Some(Ok(Vec::new()))
        } else {
            self.host.query(ARBITERS_QUERY, vec![json!(sorted(conflicts.iter().map(|c| c.relation.clone()).collect()))], Scope::Statement)
        };
        // The columns an INSERT's rows must write, asked in this first round too.
        let insert = insert_target(&rewritten.sql);
        let required = match &insert {
            Some(target) => self.host.query(REQUIRED_COLUMNS_QUERY, vec![json!(target.relation)], Scope::Statement),
            None => Some(Ok(Vec::new())),
        };
        let (Some(described), Some(trust), Some(operators), Some(catalog_rows), Some(arbiters), Some(written), Some(required)) = (described, trust, operators, catalog_rows, arbiters, written, required) else { return None };
        let (raw, described) = match described {
            Ok(d) => d,
            Err(e) => {
                diagnostics.push(db_diagnostic(sql, &rewritten, &e));
                return Some(bare(sql, lexed, Dialect::Postgres, diagnostics));
            }
        };
        // The catalog the row-count proof reads, with what the describe and the trust settled.
        let mut catalog = build_catalog(&operators, catalog_rows);
        catalog.param_types = raw.params.clone();
        trusted(&mut catalog, &trust);
        let arbiters = arbiters.unwrap_or_default();
        for target in &conflicts {
            if let Some(message) = unmatched_conflict(target, &arbiters, &catalog) {
                let start = rewritten.to_original(target.start).min(sql.len());
                let length = sql[start..].find(')').map_or(1, |close| close + 1);
                diagnostics.push(diag(message, start, length, 13));
            }
        }
        // A JSON object of an odd count of arguments, which Postgres prepares and refuses when it
        // runs; only the built-in constructors, which a user's function of the name may shadow.
        let constructors: Vec<&str> = ["json_build_object", "jsonb_build_object"].into_iter().filter(|n| trust.builtin(n)).collect();
        diagnostics.extend(crate::analysis::odd_pairs(sql, lexed, &constructors));
        // An INSERT that leaves out a column its rows must write, or writes DEFAULT to one: the
        // server prepares it and refuses each row it inserts. A row trigger that runs before the
        // insert or in its place may fill the column, and a rule may rewrite the insert, so then
        // nothing is said.
        if let Some(target) = &insert {
            let filled_elsewhere = catalog.skipping_triggers.get(&target.relation).is_some_and(|events| events & cardinality::TRIGGER_ROW_INSERT != 0)
                || catalog.rewritten.get(&target.relation).is_some_and(|events| events & cardinality::TRIGGER_ROW_INSERT != 0);
            if !filled_elsewhere {
                if let Some(message) = unwritten(target, &required.unwrap_or_default()) {
                    let start = rewritten.to_original(target.start).min(sql.len());
                    let length = rewritten.to_original(target.end).min(sql.len()).saturating_sub(start);
                    diagnostics.push(diag(message, start, length, 16));
                }
            }
        }

        // JSON shapes: what the statement's own constructors build, and for a JSON column it reads
        // straight from a WITH query or a derived table, what that item's statement builds.
        let (plan, outcomes) = self.shapes(&rewritten.sql, &trust, &catalog.columns, &raw.params, MAX_ITEM_DEPTH)?;
        let open = described.columns.iter().any(|c| !plan.columns.iter().any(|(n, _)| *n == c.name));
        let built = if open { self.built(&rewritten.sql, &described.columns, &trust, &catalog.columns, &raw.params, MAX_ITEM_DEPTH)? } else { vec![None; described.columns.len()] };
        let mut shape_oids = Vec::new();
        for (_, shape) in &plan.columns {
            jsonshape::oids(shape, &outcomes, &raw.params, &mut shape_oids);
        }
        self.types(&shape_oids)?;
        self.stored_checks(outcomes.iter().flatten().flat_map(|o| o.columns.iter()).chain(&described.columns), &trust)?;

        let registry = self.registry.borrow();
        let mut unresolved = Vec::new();
        let mut columns = Vec::with_capacity(described.columns.len());
        let mut shapes_used: HashSet<usize> = HashSet::new();
        let comments = self.comments.borrow();
        for (position, (col, built)) in described.columns.iter().zip(&built).enumerate() {
            let shape = plan.columns.iter().enumerate().find(|(i, (n, _))| n == &col.name && !shapes_used.contains(i));
            let (ty, nullable) = match (shape, built) {
                (Some((i, (_, shape))), _) if jsonshape::is_json(&registry, col.type_oid) => {
                    shapes_used.insert(i);
                    let resolved = jsonshape::resolve(shape, &outcomes, &registry, &raw.params);
                    unresolved.extend(resolved.unresolved.iter().map(|p| format!("{}: {p}", col.name)));
                    (resolved.ty, !resolved.non_null)
                }
                // The item's own result settled whether the column is NULL, an outer join here
                // included; what it builds is its document.
                (None, Some(built)) => {
                    unresolved.extend(built.unresolved.iter().map(|p| format!("{}: {p}", col.name)));
                    (built.ty.clone(), col.nullable || !built.non_null)
                }
                _ => {
                    let ty = if col.whole_row { registry.render_row(col.type_oid) } else { registry.render(col.type_oid) };
                    (with_document(with_values(with_items(ty, col.nullable_items), col.values.as_deref()), col.document.as_ref()), col.nullable)
                }
            };
            let comment = comments.columns.get(position).cloned().flatten();
            columns.push(Column { name: col.name.clone(), ty, nullable, type_name: col.type_name.clone(), comment });
        }
        let params = lexed
            .params
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let o = raw.params.get(i).copied().unwrap_or(0);
                let nullable = nullable.as_ref().is_some_and(|n| n.contains(&(i as i32 + 1)));
                // A parameter written whole to a JSON column a CHECK holds to a JSON Schema takes
                // the values that schema admits, and those of every such column it fills.
                let sources = self.written.borrow();
                let schemas: Vec<(String, Validators)> = written.iter().filter(|(n, _, _)| *n == i as i32 + 1).flat_map(|(_, rel, att)| sources.get(&(*rel, *att)).cloned().unwrap_or_default()).collect();
                let document = if schemas.is_empty() { None } else { crate::schema_type::checked_type(&schemas) };
                // A parameter written to columns that share one comment carries it.
                let mut noted = written.iter().filter(|(n, _, _)| *n == i as i32 + 1).map(|(_, rel, att)| comments.written.get(&(*rel, *att)));
                let first = noted.next().flatten();
                let comment = first.filter(|c| noted.all(|other| other == Some(*c))).cloned();
                Param { name: name.clone(), ty: with_document(registry.render(o), document.as_ref()), type_name: registry.name(o), nullable, schema: None, cast: None, comment }
            })
            .collect();

        // Row count.
        let (cardinality, reason) = self.cardinality(&rewritten.sql, parsed.as_ref(), &described, &trust, &catalog);

        let spans = column_spans(sql, &rewritten);
        let mut analysis = Analysis {
            kind: parsed.as_ref().map(|p| p.statement.to_string()).unwrap_or_default(),
            params,
            columns,
            cardinality,
            cardinality_reason: reason,
            rewritten,
            style: lexed.style,
            diagnostics,
            unresolved,
            structured: Vec::new(),
            fresh: None,
            tables: None,
        };
        crate::analysis::clashes(sql, &mut analysis, &|i| spans.get(i).copied());
        Some(analysis)
    }

    /// A statement of a kind the analysis does not read (DDL, a session setting, a transaction's
    /// control, EXPLAIN, CALL), typed from the server's describe alone: its parameters take the
    /// types the server gives them, and its columns, where it returns rows, may each be NULL, since
    /// nothing reads where they come from. One that returns none runs as a write does.
    fn command(&self, sql: &str, lexed: &Lexed, rewritten: Rewritten) -> Lazy<Analysis> {
        if copies_through_the_connection(&rewritten.sql) {
            let message = "COPY ... FROM STDIN and COPY ... TO STDOUT move their data through the connection, outside a statement's parameters and rows, so a typed .sql file cannot hold one: run it through the driver's own COPY support";
            return Some(bare(sql, lexed, Dialect::Postgres, vec![diag(message, 0, sql.trim_end().len(), 10)]));
        }
        let raw = match self.host.describe(&rewritten.sql)? {
            Ok(raw) => raw,
            Err(e) => {
                let mut d = db_diagnostic(sql, &rewritten, &e);
                d.message = lex::refused_command(&d.message);
                return Some(bare(sql, lexed, Dialect::Postgres, vec![d]));
            }
        };
        let mut oids = raw.params.clone();
        oids.extend(raw.columns.iter().map(|c| c.oid));
        self.types(&oids)?;
        let kind = lex::first_word(sql, lexed);
        // Postgres binds a parameter only in the commands it analyzes when it prepares them
        // (EXPLAIN, CALL, CREATE TABLE ... AS); in any other the server takes none, and a run that
        // passes one fails.
        let mut diagnostics = Vec::new();
        if let Some(at) = lexed.params.get(raw.params.len()).and_then(|name| lexed.placeholders.iter().find(|p| &p.name == name)) {
            let written = &sql[at.start..at.end];
            diagnostics.push(diag(format!("Postgres binds no parameter in this {} statement, so {written} cannot be passed: write its value into the statement", kind.to_ascii_uppercase()), at.start, at.end - at.start, 1));
            return Some(bare(sql, lexed, Dialect::Postgres, diagnostics));
        }
        let registry = self.registry.borrow();
        let columns: Vec<Column> = raw.columns.iter().map(|c| Column { name: c.name.clone(), ty: registry.render(c.oid), nullable: true, type_name: registry.name(c.oid), comment: None }).collect();
        let params = lexed
            .params
            .iter()
            .zip(&raw.params)
            .map(|(name, &o)| Param { name: name.clone(), ty: registry.render(o), type_name: registry.name(o), nullable: false, schema: None, cast: None, comment: None })
            .collect();
        let cardinality = if columns.is_empty() { "none" } else { "many" };
        Some(Analysis {
            kind,
            params,
            columns,
            cardinality: cardinality.into(),
            cardinality_reason: None,
            rewritten,
            style: lexed.style,
            diagnostics,
            unresolved: Vec::new(),
            structured: Vec::new(),
            fresh: None,
            tables: None,
        })
    }

    fn cardinality(&self, sql: &str, parsed: Option<&Parsed>, described: &DescribedStatement, trust: &Trust, catalog: &Catalog) -> (String, Option<String>) {
        let Some(parsed) = parsed else { return ("many".into(), Some("the statement did not parse locally".into())) };
        // By the top statement, not `kind`: a SELECT over a data-modifying WITH is a mutation,
        // and its rows are still a SELECT's.
        if parsed.statement != "select" {
            if !parsed.has_returning || described.columns.is_empty() {
                return ("none".into(), None);
            }
            return match cardinality::dml(sql, catalog) {
                Some(Ok(cardinality::Cardinality::One)) => ("one".into(), None),
                Some(Ok(cardinality::Cardinality::AtMostOne)) => ("maybeOne".into(), None),
                Some(Err(why)) => ("many".into(), Some(why)),
                None => ("many".into(), None),
            };
        }
        match cardinality::exactly_one(sql, Dialect::Postgres, trust, catalog) {
            Ok(()) => ("one".into(), None),
            Err(_) => match cardinality::at_most_one(sql, Dialect::Postgres, trust, catalog) {
                Ok(()) => ("maybeOne".into(), None),
                Err(why) => ("many".into(), Some(why)),
            },
        }
    }

    // -----------------------------------------------------------------------------------------
    // Catalog
    // -----------------------------------------------------------------------------------------

    /// Load the types `oids` need, element and base types included, asking the host for the
    /// ones the registry lacks. One round per level of nesting.
    fn types(&self, oids: &[u32]) -> Lazy<()> {
        loop {
            let mut missing = Vec::new();
            {
                let registry = self.registry.borrow();
                for o in oids {
                    if *o != 0 {
                        registry.missing(*o, &mut missing);
                    }
                }
            }
            if missing.is_empty() {
                return Some(());
            }
            missing.sort_unstable();
            let rows = self.host.query(TYPE_QUERY, vec![json!(missing)], Scope::Statement)?;
            let mut registry = self.registry.borrow_mut();
            registry.insert_rows(&rows.unwrap_or_default());
            for o in &missing {
                registry.insert_unknown(*o);
            }
        }
    }

    /// Every column and every column type of the named relations, in one round, so `settle`
    /// and `types` have their answers before the describe is back.
    fn prefetch(&self, relations: &[String]) -> Lazy<()> {
        if relations.is_empty() {
            return Some(());
        }
        let columns = self.host.query(RELATION_COLUMNS_QUERY, vec![json!(relations)], Scope::Statement);
        let types = self.host.query(RELATION_TYPES_QUERY, vec![json!(relations)], Scope::Statement);
        let (Some(columns), Some(types)) = (columns, types) else { return None };
        let mut known = self.relations.borrow_mut();
        for row in columns.unwrap_or_default() {
            let (Some(oid), Some(name), Some(attnum), Some(written)) = (row.first().and_then(as_u32), row.get(1).and_then(as_str), row.get(2).and_then(as_u32), row.get(3).and_then(as_bool)) else { continue };
            // A read trusts only a valid NOT NULL that holds through the inheritance tree; a
            // write meets the constraint as it stands, valid or not.
            let read = row.get(6).and_then(as_bool).unwrap_or(false);
            known.entry(oid).or_insert_with(|| (name.to_string(), HashMap::new())).1.insert(attnum as i32, read);
            if let Some(column) = row.get(5).and_then(as_str) {
                self.columns_by_name.borrow_mut().entry(name.to_string()).or_default().insert(attnum as i32, (column.to_string(), written));
            }
            if let Some(definition) = row.get(4).and_then(as_str) {
                self.views.borrow_mut().insert(oid, definition.trim_end().trim_end_matches(';').to_string());
            }
        }
        self.registry.borrow_mut().insert_rows(&types.unwrap_or_default());
        Some(())
    }

    /// The parameters the caller may pass null for, by number: each one cast to `nullable(…)`
    /// (`marked`), and each one that is, bare, with no cast, the whole value written to a column
    /// that may hold null, wherever it is written (a parameter that also fills a column that
    /// refuses null cannot be null). A column refuses null by its `NOT NULL`, by its domain's
    /// (declared `not null`, or checking `VALUE IS NOT NULL`), or by a CHECK that tests it `IS NOT
    /// NULL` ([`refuses_null`]). Any other cast parameter is its cast's type, which the cast states
    /// whole, and nothing else lets a parameter take null: not a `coalesce` of it, not a null test.
    /// The rules take every other parameter as a non-null value, which is what its TypeScript type
    /// promises. Beside them, each marked parameter written whole to a column that refuses null, as
    /// `(parameter, relation, column)`.
    fn nullable_params(&self, parsed: Option<&Parsed>, relations: &[String], marked: &HashSet<i32>) -> Lazy<Nullable> {
        let mut out = Nullable { params: marked.clone(), refused: Vec::new() };
        let Some(parsed) = parsed else { return Some(out) };
        let checked: Vec<&(i32, String, analyze::Filled)> = parsed.filled.iter().chain(&parsed.selected).filter(|(n, _, _)| marked.contains(n)).collect();
        if parsed.bare.is_empty() && checked.is_empty() {
            return Some(out);
        }
        let mut written: Vec<String> = parsed.bare.iter().chain(checked.iter().copied()).map(|(_, relation, _)| relation.clone()).collect();
        written.sort();
        written.dedup();
        let refusing = self.host.query(REFUSES_NULL_QUERY, vec![json!(written)], Scope::Statement);
        let prefetched = self.prefetch(relations);
        let (refusing, ()) = (refusing?, prefetched?);
        // Per relation, by attribute: whether the column refuses null.
        let mut refuses: HashMap<(String, i32), bool> = HashMap::new();
        for row in refusing.unwrap_or_default() {
            let (Some(relation), Some(attnum), Some(column), Some(not_null)) = (row.first().and_then(as_str), row.get(1).and_then(as_u32), row.get(2).and_then(as_str), row.get(3).and_then(as_bool)) else { continue };
            let checked = row.get(4).and_then(as_strings).unwrap_or_default().iter().any(|check| refuses_null(check, column));
            let domain = row.get(5).and_then(as_strings).unwrap_or_default().iter().any(|check| refuses_null(check, "value"));
            refuses.insert((relation.to_string(), attnum as i32), not_null || checked || domain);
        }
        let columns = self.columns_by_name.borrow();
        // The column `column` of `relation` names, and whether it refuses null; a column the
        // refusals do not name keeps its NOT NULL alone.
        let column_of = |relation: &String, column: &analyze::Filled| {
            let known = columns.get(relation)?;
            let (attnum, (name, not_null)) = match column {
                analyze::Filled::Name(name) => known.iter().find(|(_, (c, _))| c == name),
                analyze::Filled::Position(i) => known.iter().nth(*i),
            }?;
            Some((name.clone(), refuses.get(&(relation.clone(), *attnum)).copied().unwrap_or(*not_null)))
        };
        let mut fills: HashMap<i32, bool> = HashMap::new();
        for (n, relation, column) in &parsed.bare {
            if let Some((_, refused)) = column_of(relation, column) {
                *fills.entry(*n).or_insert(true) &= !refused;
            }
        }
        out.params.extend(fills.into_iter().filter(|(_, nullable)| *nullable).map(|(n, _)| n));
        for (n, relation, column) in checked {
            if let Some((name, true)) = column_of(relation, column) {
                out.refused.push((*n, relation.clone(), name));
            }
        }
        Some(out)
    }

    /// Each parameter written whole to a column, as `(parameter, relation, attribute)`: the
    /// columns whose CHECKs may type it ([`Engine::checks`]).
    fn written_columns(&self, parsed: Option<&Parsed>, relations: &[String]) -> Lazy<Vec<(i32, u32, i32)>> {
        let Some(parsed) = parsed.filter(|p| !p.filled.is_empty()) else { return Some(Vec::new()) };
        self.prefetch(relations)?;
        let columns = self.columns_by_name.borrow();
        let oids: HashMap<String, u32> = self.relations.borrow().iter().map(|(oid, (name, _))| (name.clone(), *oid)).collect();
        let mut out = Vec::new();
        for (n, relation, column) in &parsed.filled {
            let (Some(known), Some(oid)) = (columns.get(relation), oids.get(relation)) else { continue };
            let attnum = match column {
                analyze::Filled::Name(name) => known.iter().find(|(_, (c, _))| c == name).map(|(a, _)| *a),
                analyze::Filled::Position(i) => known.keys().nth(*i).copied(),
            };
            if let Some(attnum) = attnum {
                out.push((*n, *oid, attnum));
            }
        }
        Some(out)
    }

    /// The JSON shapes `sql`'s own constructors build, with the describes of the probes they read
    /// and the JSON each probe column reads from a WITH query or a derived table ([`Engine::built`]).
    fn shapes(&self, sql: &str, trust: &Trust, columns: &Columns, params: &[u32], depth: usize) -> Lazy<(jsonshape::Plan, Vec<Option<jsonshape::Outcome>>)> {
        let plan = jsonshape::plan(sql, Dialect::Postgres, trust, columns);
        let mut pending = false;
        // Every probe is described before the items any of them reads are followed, so those
        // take what the probe budget leaves and never a probe the statement's own JSON needs.
        let mut described = Vec::with_capacity(plan.probes.len());
        for probe in &plan.probes {
            let mut one = (probe.sql.as_str(), self.probe(&probe.sql, trust));
            if let (Some(None), Some(wider)) = (&one.1, &probe.wider) {
                one = (wider.as_str(), self.probe(wider, trust));
            }
            pending |= one.1.is_none();
            described.push(one);
        }
        // A whole row an outer join may null is read again with the row there, over the same
        // `FROM` its probe was described over, for the fields an object that exists holds: after
        // every probe, since a field without it is only less precise.
        let mut present = Vec::with_capacity(plan.probes.len());
        for (i, (probe, (text, one))) in plan.probes.iter().zip(&described).enumerate() {
            let read = match (plan.present.get(i).and_then(Option::as_ref), one) {
                (Some(read), Some(Some(_))) => if *text == probe.sql { Some(read.sql.as_str()) } else { read.wider.as_deref() },
                _ => None,
            };
            present.push(match read.map(|read| self.probe(read, trust)) {
                Some(None) => {
                    pending = true;
                    None
                }
                Some(Some(found)) => found,
                None => None,
            });
        }
        let mut outcomes: Vec<Option<jsonshape::Outcome>> = Vec::with_capacity(plan.probes.len());
        for (probe, ((text, one), present)) in plan.probes.iter().zip(described.into_iter().zip(present)) {
            let Some(Some(d)) = one else {
                outcomes.push(None);
                continue;
            };
            let present = present.filter(|p| p.columns.len() == d.columns.len()).map(|p| p.columns.iter().map(|c| c.nullable).collect());
            let outer_join = analyze::analyze(&probe.sql).map(|a| a.has_outer_join).unwrap_or(true);
            let built = match self.built(text, &d.columns, trust, columns, params, depth) {
                Some(built) => built,
                None => {
                    pending = true;
                    Vec::new()
                }
            };
            outcomes.push(Some(jsonshape::Outcome { columns: d.columns, outer_join, present, built }));
        }
        if pending {
            return None;
        }
        Some((plan, outcomes))
    }

    /// Per column of `sql`, described as `described`, the JSON a WITH query or a derived table
    /// builds in it, for a JSON column `sql` reads straight from one ([`infer::ItemRead::Column`]):
    /// the shape of the item's constructor, resolved, or for a column the item reads the same way
    /// from an item of its own, that one's, `depth` levels down at most. Such a column is as
    /// nullable as the item's result and the joins of `sql` make it, which settling it already
    /// says; `coalesce(c, '[]')` of one is its array and never NULL. The item's column goes by the
    /// name its alias or the WITH clause gives it and is found by its place, so a name the item's
    /// result holds twice finds none.
    fn built(&self, sql: &str, described: &[DescribedColumn], trust: &Trust, columns: &Columns, params: &[u32], depth: usize) -> Lazy<Vec<Option<jsonshape::Built>>> {
        let none = vec![None; described.len()];
        let json = |c: &DescribedColumn| jsonshape::is_json(&self.registry.borrow(), c.type_oid);
        if depth == 0 || !described.iter().any(json) {
            return Some(none);
        }
        let Some((reads, queries)) = infer::item_reads(sql).filter(|(reads, _)| reads.len() == described.len()) else { return Some(none) };
        let mut out = Vec::with_capacity(described.len());
        let mut pending = false;
        for (read, col) in reads.into_iter().zip(described) {
            let (item, column, defaulted) = match read {
                infer::ItemRead::Column { item, column } if json(col) => (item, column, false),
                infer::ItemRead::Defaulted { item, column } if json(col) => (item, column, true),
                _ => {
                    out.push(None);
                    continue;
                }
            };
            match self.item_built(&queries[item], &column, trust, columns, params, depth) {
                Some(found) => out.push(found.and_then(|b| match defaulted {
                    false => Some(b),
                    true => matches!(b.ty, Type::List { .. }).then_some(jsonshape::Built { non_null: true, ..b }),
                })),
                None => {
                    pending = true;
                    out.push(None);
                }
            }
        }
        if pending {
            return None;
        }
        Some(out)
    }

    /// The JSON one item's column `column` holds ([`Engine::built`]).
    fn item_built(&self, query: &infer::ItemQuery, column: &str, trust: &Trust, columns: &Columns, params: &[u32], depth: usize) -> Lazy<Option<jsonshape::Built>> {
        let Some(described) = self.probe(&query.sql, trust)? else { return Some(None) };
        let named = |i: usize, c: &DescribedColumn| query.columns.get(i).cloned().unwrap_or_else(|| c.name.clone());
        let mut at = described.columns.iter().enumerate().filter(|(i, c)| named(*i, c) == column).map(|(i, _)| i);
        let (Some(at), None) = (at.next(), at.next()) else { return Some(None) };
        let own = &described.columns[at].name;
        if described.columns.iter().filter(|c| c.name == *own).count() != 1 {
            return Some(None);
        }
        let (plan, outcomes) = self.shapes(&query.sql, trust, columns, params, depth - 1)?;
        let Some((_, shape)) = plan.columns.iter().find(|(n, _)| n == own) else {
            let mut inner = self.built(&query.sql, &described.columns, trust, columns, params, depth - 1)?;
            return Some(inner.swap_remove(at));
        };
        let mut oids = Vec::new();
        jsonshape::oids(shape, &outcomes, params, &mut oids);
        self.types(&oids)?;
        self.stored_checks(outcomes.iter().flatten().flat_map(|o| o.columns.iter()), trust)?;
        let resolved = jsonshape::resolve(shape, &outcomes, &self.registry.borrow(), params);
        Some(Some(jsonshape::Built { ty: resolved.ty, non_null: resolved.non_null, unresolved: resolved.unresolved }))
    }

    /// A stored row among `columns`, read whole or inside a JSON document, keeps the JSON Schema
    /// a CHECK holds its JSON columns to and the strings a CHECK keeps its string columns to, as a
    /// bare read of them does: the registry learns them. The strings only where `=` is the
    /// built-in one a CHECK resolved to.
    fn stored_checks<'c>(&self, columns: impl Iterator<Item = &'c DescribedColumn>, trust: &Trust) -> Lazy<()> {
        let stored: Vec<u32> = columns.filter(|c| c.whole_row).map(|c| c.type_oid).collect();
        let mut rows: Vec<(u32, i32)> = stored.iter().flat_map(|o| self.registry.borrow().row_json_columns(*o)).collect();
        if trust.builtin("=") {
            for key in stored.iter().flat_map(|o| self.registry.borrow().row_string_columns(*o)) {
                if !rows.contains(&key) {
                    rows.push(key);
                }
            }
        }
        if !rows.is_empty() {
            let checks = self.check_rows(&rows)?;
            let mut registry = self.registry.borrow_mut();
            registry.documents.extend(checks.documents);
            registry.values.extend(checks.values);
        }
        Some(())
    }

    /// Ask for a probe's describe ahead of the round that reads it, when nothing has to be
    /// pinned on it first: `probe` finds the answer waiting.
    fn prewarm(&self, sql: &str) {
        if lex::lex(sql, Dialect::Postgres).is_ok_and(|l| l.placeholders.is_empty()) {
            let _ = self.host.describe(sql);
        }
    }

    /// Ask for the describe of every probe settling `sql` may read, its own and theirs in turn,
    /// once the casts a probe is pinned with are known: the statements its FROM items run, its
    /// leaf probe, its set operation's arms. Each probe settles the next only once described, so
    /// asked one by one they would take a round each; every one of their texts follows from the
    /// SQL, so all of them are asked at once. A guess `probe` never reads costs one describe, and
    /// no more of them than the probes one analysis may settle are asked, the columns' and the
    /// JSON shapes' together ([`Engine::prewarmed`]).
    fn prewarm_tree(&self, sql: &str, trust: &Trust) {
        if let Some((_, queries)) = infer::item_reads(sql) {
            for query in &queries {
                self.prewarm_probe(&query.sql, trust);
            }
        }
        // The columns a leaf plan reads are a catalog query, answered in a later round when not
        // yet asked; the leaves wait for it.
        let Some(columns) = level_columns(sql, self.host) else { return };
        let plan = infer::plan(sql, Dialect::Postgres, trust, &columns);
        if let Some(set) = &plan.set_op {
            self.prewarm_probe(&set.left, trust);
            self.prewarm_probe(&set.right, trust);
        }
        if let (Some(leaves), true) = (&plan.leaves, plan.reads_leaves()) {
            self.prewarm_probe(leaves, trust);
        }
    }

    fn prewarm_probe(&self, sql: &str, trust: &Trust) {
        {
            let mut prewarmed = self.prewarmed.borrow_mut();
            if prewarmed.len() > MAX_PROBES || !prewarmed.insert(sql.to_string()) {
                return;
            }
        }
        let _ = self.host.describe(&self.pin(sql));
        self.prewarm_tree(sql, trust);
    }

    // -----------------------------------------------------------------------------------------
    // Describe, settle, refine
    // -----------------------------------------------------------------------------------------

    /// The describe of `sql` (the `$n` form) with its columns' nullability settled and refined.
    /// What it needs is asked in the same round as the parse-named relations' columns and types
    /// and every probe that carries no placeholder. `Some(Err)` is the server's refusal.
    fn described(&self, sql: &str, parsed: Option<&Parsed>, relations: &[String], trust: Option<&Trust>, written: &[(u32, i32)]) -> Lazy<Result<(RawDescribe, DescribedStatement), DbError>> {
        let raw = self.host.describe(sql);
        let prefetched = self.prefetch(relations);
        if let Some((_, queries)) = infer::item_reads(sql) {
            for query in &queries {
                self.prewarm(&query.sql);
            }
        }
        // The columns that say whose an aggregate in a scalar subquery is, which `refine` reads:
        // no probe waits on them.
        let columns = level_columns(sql, self.host).unwrap_or_default();
        if let Some(trust) = trust {
            let plan = infer::plan(sql, Dialect::Postgres, trust, &columns);
            if let Some(set) = &plan.set_op {
                self.prewarm(&set.left);
                self.prewarm(&set.right);
            }
            if let (Some(leaves), true) = (&plan.leaves, plan.reads_leaves()) {
                self.prewarm(leaves);
            }
        }
        let (Some(raw), Some(trust), Some(())) = (raw, trust, prefetched) else { return None };
        let raw = match raw {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };
        // Types for the parameters and the columns, and the casts probes pin on the parameters.
        let mut oids: Vec<u32> = raw.params.clone();
        oids.extend(raw.columns.iter().map(|c| c.oid));
        let types = self.types(&oids);
        let checks = self.checks(sql, &raw, parsed, written, false);
        let comments = self.comments(sql, &raw, parsed, written);
        // The casts are set before the first probe, which is pinned with them: a probe described
        // before them and again after would be two texts, each a describe and a round.
        if raw.params.iter().any(|o| *o != 0 && self.registry.borrow().get(*o).is_none()) {
            return None;
        }
        *self.casts.borrow_mut() = raw.params.iter().map(|o| self.registry.borrow().get(*o).map(|t| t.cast_name.clone()).filter(|c| !c.is_empty())).collect();
        self.pinned.set(true);
        self.prewarmed.borrow_mut().insert(sql.to_string());
        self.prewarm_tree(sql, trust);
        // Nullability from provenance, then from the SQL.
        let settled = self.settle(sql, &raw, parsed, trust);
        let (Some(()), Some(mut described), Some(checks), Some(comments)) = (types, settled, checks, comments) else { return None };
        *self.comments.borrow_mut() = comments;
        self.stored_sublinks(sql, &mut described, trust);
        for (col, raw) in described.columns.iter_mut().zip(&raw.columns) {
            if let (true, Some(values)) = (trust.builtin("="), checks.values.get(&(raw.table, raw.attnum))) {
                col.values = Some(values.clone());
            }
            if let Some(document) = checks.documents.get(&(raw.table, raw.attnum)) {
                col.document = Some(document.clone());
            }
        }
        *self.written.borrow_mut() = written.iter().filter_map(|key| Some((*key, checks.sources.get(key)?.clone()))).collect();
        self.refine(sql, &mut described, trust)?;
        self.without_missing_rows(sql, parsed, &raw, &mut described);
        Some(Ok((raw, described)))
    }

    /// PostgreSQL 18's `RETURNING old.*` and `new.*`, and a MERGE's source where the MERGE acts
    /// without one, read a row that may not exist, whatever the provenance the describe reports
    /// for them says: such a column is nullable. A probe is settled this way too, since it carries
    /// the statement's WITH queries, and a data-modifying one may return such a row.
    fn without_missing_rows(&self, sql: &str, parsed: Option<&Parsed>, raw: &RawDescribe, described: &mut DescribedStatement) {
        if parsed.is_none_or(|p| p.kind != analyze::StatementKind::Mutation) {
            return;
        }
        let known = self.columns_by_name.borrow();
        let columns = |relation: &str| known.get(relation).map(|columns| columns.values().map(|(name, _)| name.clone()).collect::<Vec<_>>());
        let origins: Vec<bool> = raw.columns.iter().map(|c| c.table != 0).collect();
        for (col, missing) in described.columns.iter_mut().zip(missing_rows(sql, &origins, &columns)) {
            if missing {
                col.nullable = true;
            }
        }
    }

    /// Per result column that reads a table's column as it is, and per column of `written`, what
    /// its CHECKs say of its values: the strings they keep it to ([`check_values`]; the caller
    /// takes them only where `=` is the built-in one a CHECK resolved to), and the JSON Schema
    /// they hold it to ([`check_documents`]). The describe names a column's table and attribute
    /// only for a bare reference (through a view or a subquery too), so an expression over the
    /// column is never one. Nothing read through a set operation, whose provenance names the
    /// leftmost arm only ([`own_provenance`]). With `checked_only`, only the JSON and string
    /// columns, the ones a CHECK says something of.
    fn checks(&self, sql: &str, raw: &RawDescribe, parsed: Option<&Parsed>, written: &[(u32, i32)], checked_only: bool) -> Lazy<Checks> {
        let own = own_provenance(sql, parsed, raw.columns.len());
        let json = |oid: u32| !checked_only || jsonshape::is_json(&self.registry.borrow(), oid) || matches!(self.registry.borrow().resolve_domain(oid), crate::pgtypes::oid::TEXT | crate::pgtypes::oid::VARCHAR);
        let mut pairs: Vec<(u32, i32)> = raw.columns.iter().zip(&own).filter(|(c, own)| **own && c.table != 0 && c.attnum > 0 && json(c.oid)).map(|(c, _)| (c.table, c.attnum)).collect();
        pairs.extend(written.iter().filter(|p| !pairs.contains(p)).copied().collect::<Vec<_>>());
        self.check_rows(&pairs)
    }

    /// The comment of each result column, by position, that reads a column as it is, and of each
    /// column of `written`, by relation and attribute. The describe names a column's table and attribute only for a
    /// bare reference, so an expression over the column has none; nothing read through a set
    /// operation, whose provenance names the leftmost arm only ([`own_provenance`]). A column read
    /// through a view names the view's column, whose own comment is the one it takes; one the view
    /// leaves without a comment takes that of the column the view's definition reads it from, where
    /// it reads one as it is.
    fn comments(&self, sql: &str, raw: &RawDescribe, parsed: Option<&Parsed>, written: &[(u32, i32)]) -> Lazy<Comments> {
        let own = own_provenance(sql, parsed, raw.columns.len());
        let mut pairs: Vec<(u32, i32)> = raw.columns.iter().zip(&own).filter(|(c, own)| **own && c.table != 0 && c.attnum > 0).map(|(c, _)| (c.table, c.attnum)).collect();
        pairs.extend(written.iter().copied());
        // Each view column's source, through views of views as far as the definitions are known.
        let mut sources: HashMap<(u32, i32), (u32, i32)> = HashMap::new();
        let mut pending = false;
        let mut i = 0;
        while i < pairs.len() {
            let (relation, attnum) = pairs[i];
            i += 1;
            let Some(definition) = self.views.borrow().get(&relation).cloned() else { continue };
            match self.host.describe(&definition) {
                Some(Ok(view)) => {
                    let own = own_provenance(&definition, analyze::analyze(&definition).ok().as_ref(), view.columns.len());
                    let at = attnum as usize - 1;
                    let Some(source) = view.columns.get(at).filter(|c| own[at] && c.table != 0 && c.attnum > 0).map(|c| (c.table, c.attnum)) else { continue };
                    sources.insert((relation, attnum), source);
                    if !pairs.contains(&source) {
                        pairs.push(source);
                    }
                }
                Some(Err(_)) => {}
                None => pending = true,
            }
        }
        if pending {
            return None;
        }
        if pairs.is_empty() {
            return Some(Comments { columns: vec![None; raw.columns.len()], written: HashMap::new() });
        }
        pairs.sort_unstable();
        pairs.dedup();
        let rs: Vec<u32> = pairs.iter().map(|(r, _)| *r).collect();
        let atts: Vec<i32> = pairs.iter().map(|(_, a)| *a).collect();
        let rows = self.host.query(COMMENTS_QUERY, vec![json!(rs), json!(atts)], Scope::Statement)?;
        let stored: HashMap<(u32, i32), String> = rows
            .unwrap_or_default()
            .iter()
            .filter_map(|r| Some(((as_u32(r.first()?)?, as_u32(r.get(1)?)? as i32), as_str(r.get(2)?)?.to_string())))
            .filter(|(_, comment)| !comment.is_empty())
            .collect();
        let mut by_column = HashMap::new();
        for pair in &pairs {
            let mut at = *pair;
            // A view's source is never itself: a definition reads other relations.
            for _ in 0..=sources.len() {
                if let Some(comment) = stored.get(&at) {
                    by_column.insert(*pair, comment.clone());
                    break;
                }
                match sources.get(&at) {
                    Some(source) => at = *source,
                    None => break,
                }
            }
        }
        // By position, so that a column read through a set operation takes no comment where another
        // column of the statement reads the one its provenance names.
        let columns = raw.columns.iter().zip(&own).map(|(c, own)| by_column.get(&(c.table, c.attnum)).filter(|_| *own).cloned()).collect();
        let written = written.iter().filter_map(|key| Some((*key, by_column.get(key)?.clone()))).collect();
        Some(Comments { columns, written })
    }

    /// What the CHECKs of each column of `pairs`, by relation and attribute, say of its values
    /// ([`Engine::checks`]).
    fn check_rows(&self, pairs: &[(u32, i32)]) -> Lazy<Checks> {
        if pairs.is_empty() {
            return Some(Checks::default());
        }
        let rs: Vec<u32> = pairs.iter().map(|(r, _)| *r).collect();
        let atts: Vec<i32> = pairs.iter().map(|(_, a)| *a).collect();
        let rows = self.host.query(CHECKS_QUERY, vec![json!(rs), json!(atts)], Scope::Statement)?;
        let mut out = Checks::default();
        let mut schemas: HashMap<(u32, i32), Vec<(String, Validators)>> = HashMap::new();
        for row in rows.unwrap_or_default() {
            let (Some(rel), Some(att), Some(name), Some(text)) = (row.first().and_then(as_u32), row.get(1).and_then(as_u32), row.get(2).and_then(as_str), row.get(3).and_then(as_str)) else { continue };
            let key = (rel, att as i32);
            if let Some(values) = check_values(text, name) {
                // Every CHECK holds, so a value is in each one's list.
                out.values.entry(key).and_modify(|known| known.retain(|v| values.contains(v))).or_insert(values);
            }
            let calls: Vec<(String, String, String)> = match (row.get(4).and_then(as_strings), row.get(5).and_then(as_strings), row.get(6).and_then(as_strings)) {
                (Some(names), Some(extensions), Some(versions)) => names.into_iter().zip(extensions).zip(versions).map(|((n, e), v)| (n, e, v)).collect(),
                _ => Vec::new(),
            };
            for (function, args) in check_documents(text, name) {
                // The function the CHECK calls under that name, which the catalog names by its
                // extension: every function of that name it depends on must be the one validator.
                let mut called = calls.iter().filter(|(n, _, _)| *n == function);
                let Some((_, extension, version)) = called.next() else { continue };
                if called.any(|(_, e, v)| e != extension || v != version) {
                    continue;
                }
                let Some((validators, arity, schema_at, document_at)) = crate::schema_type::postgres_validator(&function, extension) else { continue };
                if let (true, Some(CheckArg::Schema(schema)), Some(CheckArg::Column)) = (args.len() == arity, args.get(schema_at), args.get(document_at)) {
                    schemas.entry(key).or_default().push((schema.clone(), validators));
                }
            }
        }
        for (key, texts) in schemas {
            if let Some(document) = crate::schema_type::checked_type(&texts) {
                out.documents.insert(key, document);
            }
            out.sources.insert(key, texts);
        }
        Some(out)
    }

    /// Mark the result columns a scalar or `ARRAY` subquery fills with stored rows of a table, or
    /// an `array_agg` of a table's whole row aggregates them into ([`sublink_rows`]), where the
    /// name can only be that table's whole row: it names no column of any table the statement
    /// reads, and every FROM item of the statement is a table whose columns the engine read. The
    /// column's type confirms it: the table's row type, or an array of it. A user's `array_agg`
    /// may build any row, so the built-in one alone counts.
    fn stored_sublinks(&self, sql: &str, described: &mut DescribedStatement, trust: &Trust) {
        let Some(mut rows) = sublink_rows(sql) else { return };
        if !trust.builtin("array_agg") {
            for target in &mut rows.targets {
                if target.as_ref().is_some_and(|t| t.3) {
                    *target = None;
                }
            }
        }
        if rows.targets.len() != described.columns.len() || rows.targets.iter().all(Option::is_none) {
            return;
        }
        let columns = self.columns_by_name.borrow();
        let relations = self.relations.borrow();
        let registry = self.registry.borrow();
        let unambiguous = |name: &str| !SYSTEM_COLUMNS.contains(&name) && rows.tables.iter().all(|t| columns.get(t).is_some_and(|c| c.values().all(|(column, _)| column != name)));
        for (col, target) in described.columns.iter_mut().zip(&rows.targets) {
            let Some((relation, name, array, _)) = target else { continue };
            let oid = relations.iter().find(|(_, (n, _))| n == relation).map(|(o, _)| *o);
            let row_type = if *array { registry.get(col.type_oid).and_then(|t| t.elem) } else { Some(col.type_oid) };
            let of = row_type.and_then(|o| registry.get(o)).and_then(|t| t.relid);
            if oid.is_some() && of == oid && unambiguous(name) {
                col.whole_row = true;
            }
        }
    }

    /// A described statement's columns with their nullability from provenance: the catalog's
    /// `NOT NULL`, unless the column's table sits on the nullable side of an outer join, or the
    /// statement's shape hides which relation produced a column.
    fn settle(&self, sql: &str, raw: &RawDescribe, parsed: Option<&Parsed>, trust: &Trust) -> Lazy<DescribedStatement> {
        // A column of a view: the view's own statement is settled like any other (its columns
        // trace to the base tables, its joins null a side, its expressions follow the rules),
        // and column `attnum` of the view is column `attnum - 1` of that result.
        let mut view_columns: HashMap<u32, Option<Vec<DescribedColumn>>> = HashMap::new();
        let definitions: Vec<(u32, String)> = {
            let views = self.views.borrow();
            let mut oids: Vec<u32> = raw.columns.iter().map(|c| c.table).filter(|t| views.contains_key(t)).collect();
            oids.sort_unstable();
            oids.dedup();
            oids.into_iter().map(|o| (o, views[&o].clone())).collect()
        };
        let mut pending = false;
        for (oid, definition) in definitions {
            match self.probe(&definition, trust) {
                Some(outcome) => {
                    view_columns.insert(oid, outcome.map(|d| d.columns));
                }
                None => pending = true,
            }
        }
        if pending {
            return None;
        }
        // The parse-named relations were prefetched; a column from any other relation (a view's
        // base table, a relation the parse could not name) is asked for by oid.
        let known = self.relations.borrow();
        let pairs: Vec<(u32, i32)> = raw.columns.iter().filter(|c| c.table != 0 && c.attnum > 0 && !known.contains_key(&c.table)).map(|c| (c.table, c.attnum)).collect();
        let rels: Vec<u32> = {
            let mut r: Vec<u32> = pairs.iter().map(|(r, _)| *r).collect();
            r.sort_unstable();
            r.dedup();
            r
        };
        let not_null = if pairs.is_empty() {
            Some(Ok(Vec::new()))
        } else {
            let rs: Vec<u32> = pairs.iter().map(|(r, _)| *r).collect();
            let atts: Vec<i32> = pairs.iter().map(|(_, a)| *a).collect();
            self.host.query(ATTNOTNULL_QUERY, vec![json!(rs), json!(atts)], Scope::Statement)
        };
        let nullable_tables: Vec<String> = parsed.map(|p| unreduced(p, trust)).unwrap_or_default();
        let nullable_tables: &[String] = &nullable_tables;
        let relnames = if nullable_tables.is_empty() || rels.is_empty() {
            Some(Ok(Vec::new()))
        } else {
            self.host.query(RELNAMES_QUERY, vec![json!(rels)], Scope::Statement)
        };
        let (Some(not_null), Some(relnames)) = (not_null, relnames) else { return None };
        let mut not_null: HashSet<(u32, i32)> = not_null
            .unwrap_or_default()
            .iter()
            .filter_map(|r| Some((as_u32(r.first()?)?, as_u32(r.get(1)?)? as i32)))
            .collect();
        let mut nulled_rels: HashSet<u32> = relnames
            .unwrap_or_default()
            .iter()
            .filter_map(|r| {
                let (o, name) = (as_u32(r.first()?)?, as_str(r.get(1)?)?);
                nullable_tables.iter().any(|t| t == name).then_some(o)
            })
            .collect();
        for col in &raw.columns {
            if let Some((name, atts)) = known.get(&col.table) {
                if atts.get(&col.attnum).copied().unwrap_or(false) {
                    not_null.insert((col.table, col.attnum));
                }
                if nullable_tables.iter().any(|t| t == name) {
                    nulled_rels.insert(col.table);
                }
            }
        }
        // Without a parse tree nothing degrades and nothing is nulled by a join, which is
        // wrong in the unsafe direction, so a statement libpg_query cannot read keeps every
        // column nullable.
        let grouping = parsed.is_none_or(|p| p.has_grouping_sets);
        // A table read twice where an outer join nulls one reading: the describe names the table
        // of a column, never which reading. An outer join nulls a table by its name, though, so
        // every reading of it, and the relations of a WITH query it nulls: a column of a nulled
        // table is nullable whichever reading it comes from, and one of a table no join nulls is
        // as the catalog says.
        let repeated = parsed.is_some_and(|p| p.has_repeated_table) && !nullable_tables.is_empty();
        let hidden = grouping;
        // A set operation anywhere: provenance names the leftmost arm's column, which says
        // nothing about the other arms ([`own_provenance`]). Such a column stays nullable; one
        // read from a WITH query is settled from that query's own probe, whose arms are combined.
        let own = own_provenance(sql, parsed, raw.columns.len());
        // A table read twice: a top-level `alias.column` names its reading, though, the table the
        // alias is a reading of and whether an outer join of the top FROM nulls it, where no set
        // operation leaves the provenance unsure.
        // A table's width is its columns the catalog read, where one relation goes by the name.
        let width = |name: &str| {
            let mut tables = known.values().filter(|(n, _)| n == name);
            match (tables.next(), tables.next()) {
                (Some((_, columns)), None) => Some(columns.len()),
                _ => None,
            }
        };
        let aliased = if repeated && !grouping && !parsed.is_some_and(|p| p.has_set_op) { aliased_columns(sql, raw.columns.len(), &width) } else { Vec::new() };
        // A column read straight from a WITH query, a derived table or a function in FROM is as
        // nullable as that item's own result says, whatever the catalog says of the table column
        // behind it. Each item answers by its place in the FROM, so a table read twice changes
        // nothing there.
        // Following an item can prefetch a relation its statement reads, which writes the cache:
        // the borrow is let go for it and taken again after.
        drop(known);
        let item_columns = if grouping { Some(Vec::new()) } else { self.item_columns(sql, raw.columns.len(), trust) };
        let Some(item_columns) = item_columns else { return None };
        let known = self.relations.borrow();
        let registry = self.registry.borrow();
        let columns: Vec<DescribedColumn> = raw
            .columns
            .iter()
            .zip(&own)
            .enumerate()
            .map(|(i, (col, own))| {
                let degraded = hidden || !own;
                // A whole-row reference (attribute 0) of the relation whose row type the column is.
                let own_row = col.table != 0 && col.attnum == 0 && !degraded && registry.get(col.oid).and_then(|t| t.relid) == Some(col.table);
                let declared = || match view_columns.get(&col.table) {
                    Some(columns) => columns.as_ref().and_then(|c| c.get(col.attnum as usize - 1)).is_none_or(|c| c.nullable),
                    None => !not_null.contains(&(col.table, col.attnum)),
                };
                let reading = aliased.get(i).and_then(Option::as_ref).filter(|(relation, _)| known.get(&col.table).is_some_and(|(name, _)| name == relation));
                let nullable = if col.table != 0 && col.attnum > 0 && !degraded {
                    declared() || reading.map_or(nulled_rels.contains(&col.table), |(_, nulled)| *nulled)
                } else if own_row && known.contains_key(&col.table) {
                    // A row the scan read is a row, whatever its columns hold: NULL only where an
                    // outer join nulls the relation, which the statement names, so its name says.
                    nulled_rels.contains(&col.table)
                } else {
                    true
                };
                // A view's column holds what the view's own statement gives it, and so keeps the
                // JSON Schema a CHECK holds the column it reads to.
                let document = match view_columns.get(&col.table) {
                    Some(Some(columns)) if col.attnum > 0 && !degraded => columns.get(col.attnum as usize - 1).and_then(|c| c.document.clone()),
                    _ => None,
                };
                // A whole-row reference (attribute 0) of the relation whose row type the column is,
                // or a view's column that is one in the view's own statement.
                let from_view = match view_columns.get(&col.table) {
                    Some(Some(columns)) if col.attnum > 0 && !degraded => columns.get(col.attnum as usize - 1).is_some_and(|c| c.whole_row),
                    _ => false,
                };
                let whole_row = from_view || own_row;
                DescribedColumn { name: col.name.clone(), type_oid: col.oid, type_name: registry.name(col.oid), nullable, nullable_items: None, values: None, document, whole_row }
            })
            .collect();
        let columns = columns
            .into_iter()
            .enumerate()
            .map(|(i, col)| match item_columns.get(i) {
                // An outer join nulls a WITH query by its name, and any other item by its place.
                // What the item settled of its column's values holds here too.
                Some(Some((with, item))) => DescribedColumn {
                    nullable: item.nullable || with.as_ref().is_some_and(|query| nullable_tables.iter().any(|t| t == query)),
                    document: col.document.clone().or_else(|| item.document.clone()),
                    values: col.values.clone().or_else(|| item.values.clone()),
                    ..col
                },
                _ => col,
            })
            .collect();
        Some(DescribedStatement { param_oids: raw.params.clone(), columns })
    }

    /// Per result column of `sql` that is a bare reference to a column of one of its FROM items
    /// with a statement of its own ([`infer::ItemQuery`]: a WITH query, a derived table, a
    /// function): the WITH query's name for one, and whether the item's own settled result has
    /// the column nullable, or an outer join nulls an item that is no WITH query. A `*` over such
    /// an item is its columns in order.
    ///
    /// A recursive WITH query reads itself, so its columns' nullability is a fixed point, and
    /// the greatest one is sound. Every column starts as non-null for the reads the query makes
    /// of itself; the query is settled under that assumption; a column that comes out nullable
    /// all the same joins the assumption as nullable, and the query is settled again, until
    /// nothing changes. What survives holds by induction over the iterations: the rows of the
    /// non-recursive arm are non-null on their own account, each later row is non-null given
    /// the rows it was computed from, and a UNION is non-null only where both arms are.
    fn item_columns(&self, sql: &str, count: usize, trust: &Trust) -> Lazy<Vec<Option<(Option<String>, ItemColumn)>>> {
        use infer::ItemRead;
        let Some((reads, queries)) = infer::item_reads(sql) else { return Some(Vec::new()) };
        // One `*` at most: its width is what the other targets leave of the result's columns.
        let stars = reads.iter().filter(|r| matches!(r, ItemRead::Star { .. })).count();
        let Some(width) = (count + stars).checked_sub(reads.len()).filter(|w| stars == 1 || (stars == 0 && *w == 0)) else { return Some(Vec::new()) };
        let read = |i: usize| reads.iter().any(|r| matches!(r, ItemRead::Column { item, .. } | ItemRead::Star { item: Some(item) } if *item == i));
        let mut settled: HashMap<usize, Vec<ItemColumn>> = HashMap::new();
        let mut pending = false;
        for (i, query) in queries.iter().enumerate().filter(|(i, _)| read(*i)) {
            // The query reading itself: the answer is the assumption it is being settled under,
            // for the columns it reads by name. A `*` there finds no column list of its width. A
            // probe binds nothing, so the statement read from may be one with NULL in place of the
            // query's placeholders (`jsonshape::select_text`): it is the same query.
            let same = jsonshape::without_placeholders(&query.sql);
            if let Some(entry) = self.with_stack.borrow_mut().iter_mut().find(|(text, _, _)| *text == query.sql || jsonshape::without_placeholders(text) == same) {
                entry.2 = true;
                self.assumptions.set(self.assumptions.get() + 1);
                let assumed = &entry.1;
                let named = reads.iter().filter_map(|r| match r {
                    ItemRead::Column { item, column } if *item == i => Some(ItemColumn { name: column.clone(), nullable: assumed.contains(column), document: None, values: None }),
                    _ => None,
                });
                settled.insert(i, named.collect());
                continue;
            }
            let mut assumed: HashSet<String> = HashSet::new();
            loop {
                self.with_stack.borrow_mut().push((query.sql.clone(), assumed.clone(), false));
                let outcome = self.probe(&query.sql, trust);
                let (_, _, read_itself) = self.with_stack.borrow_mut().pop().expect("the entry pushed above");
                let described = match outcome {
                    Some(Some(described)) => described,
                    // An item the server will not describe alone proves nothing.
                    Some(None) => break,
                    None => {
                        pending = true;
                        break;
                    }
                };
                let columns: Vec<ItemColumn> = described
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(i, c)| ItemColumn { name: query.columns.get(i).cloned().unwrap_or_else(|| c.name.clone()), nullable: c.nullable, document: c.document.clone(), values: c.values.clone() })
                    .collect();
                let refuted: Vec<String> = columns.iter().filter(|c| c.nullable && !assumed.contains(&c.name)).map(|c| c.name.clone()).collect();
                if !read_itself || refuted.is_empty() {
                    settled.insert(i, columns);
                    break;
                }
                assumed.extend(refuted);
            }
        }
        if pending {
            return None;
        }
        let answer = |item: usize, column: &ItemColumn| (queries[item].with.clone(), ItemColumn { nullable: column.nullable || (queries[item].with.is_none() && queries[item].nulled), ..column.clone() });
        let mut out = Vec::with_capacity(count);
        for read in reads {
            match read {
                ItemRead::Other | ItemRead::Table | ItemRead::Defaulted { .. } => out.push(None),
                ItemRead::Column { item, column } => {
                    let mut named = settled.get(&item).into_iter().flatten().filter(|c| c.name == column);
                    out.push(match (named.next(), named.next()) {
                        (Some(found), None) => Some(answer(item, found)),
                        _ => None,
                    });
                }
                ItemRead::Star { item } => match item.and_then(|item| Some((item, settled.get(&item).filter(|columns| columns.len() == width)?))) {
                    Some((item, columns)) => out.extend(columns.iter().map(|c| Some(answer(item, c)))),
                    None => out.extend(std::iter::repeat(None).take(width)),
                },
            }
        }
        Some(out)
    }

    /// Settle what the describe left open about expression columns from the SQL itself. A column
    /// only ever becomes non-null here, and a probe that cannot be described proves nothing.
    fn refine(&self, sql: &str, described: &mut DescribedStatement, trust: &Trust) -> Lazy<()> {
        let columns = level_columns(sql, self.host)?;
        let plan = infer::plan(sql, Dialect::Postgres, trust, &columns);
        // Per column: whether it is non-null, and what the statement says of an array's elements.
        let mut reads: Vec<Option<infer::JsonRead>> = Vec::new();
        let facts: Vec<Option<(bool, Option<bool>)>> = match &plan.set_op {
            Some(set) => {
                let left = self.probe(&set.left, trust);
                let right = self.probe(&set.right, trust);
                let (Some(left), Some(right)) = (left, right) else { return None };
                let (Some(left), Some(right)) = (left, right) else { return Some(()) };
                let Some(facts) = infer::combine(set.kind, &left.columns, &right.columns) else { return Some(()) };
                // A set operation's array is one of an arm's, and an INTERSECT's is one of each.
                let items = left.columns.iter().zip(&right.columns).map(|(l, r)| match set.kind {
                    infer::SetKind::Union => match (l.nullable_items, r.nullable_items) {
                        (Some(true), _) | (_, Some(true)) => Some(true),
                        (Some(false), Some(false)) => Some(false),
                        _ => None,
                    },
                    infer::SetKind::Intersect => match (l.nullable_items, r.nullable_items) {
                        (Some(false), _) | (_, Some(false)) => Some(false),
                        (Some(true), _) | (_, Some(true)) => Some(true),
                        _ => None,
                    },
                    infer::SetKind::Except => l.nullable_items,
                });
                facts.into_iter().zip(items).map(|(f, items)| Some((f.non_null, items))).collect()
            }
            None => {
                let claims = plan.claims(described.columns.len());
                let registry = self.registry.borrow();
                let is_array = |o: u32| registry.get(o).is_some_and(|t| t.kind == crate::pgtypes::TypeKind::Array);
                let open = claims.iter().zip(&described.columns).any(|(claim, col)| claim.is_some() && (col.nullable || is_array(col.type_oid)));
                drop(registry);
                if !open {
                    return Some(());
                }
                let leaves = match &plan.leaves {
                    Some(probe) if plan.reads_leaves() => match self.probe(probe, trust) {
                        Some(outcome) => outcome.map(|d| d.columns),
                        None => return None,
                    },
                    _ => None,
                };
                // A type the registry has not read may be an array, whose elements say nothing.
                let registry = self.registry.borrow();
                let is_array = |o: u32| registry.get(o).is_none_or(|t| t.kind == crate::pgtypes::TypeKind::Array);
                reads = claims.iter().map(|claim| claim.and_then(|e| infer::json_read(e, leaves.as_deref()))).collect();
                claims.iter().map(|claim| claim.map(|e| (infer::resolve(e, leaves.as_deref()).non_null, infer::items_nullable(e, leaves.as_deref(), &is_array)))).collect()
            }
        };
        if facts.len() != described.columns.len() {
            return Some(());
        }
        for (col, fact) in described.columns.iter_mut().zip(facts) {
            if let Some((non_null, items)) = fact {
                if non_null {
                    col.nullable = false;
                }
                col.nullable_items = items;
            }
        }
        // A read of a JSON column at literal keys holds what the column's schema says is there:
        // that schema, or the strings it keeps a text read to.
        for (col, read) in described.columns.iter_mut().zip(reads) {
            match read {
                Some(read) if read.text => col.values = read.strings(),
                Some(read) if !read.schema.is_boolean() => col.document = Some(read.schema),
                _ => {}
            }
        }
        Some(())
    }

    /// Describe a synthesized statement the way the main one is described, refinement included.
    /// `Some(None)` is a probe the database refused, which proves nothing.
    fn probe(&self, sql: &str, trust: &Trust) -> Lazy<Option<DescribedStatement>> {
        if self.probing.borrow().iter().any(|p| p == sql) {
            self.assumptions.set(self.assumptions.get() + 1);
            return Some(None);
        }
        if let Some(outcome) = self.settled.borrow().get(sql) {
            return Some(outcome.clone());
        }
        if self.probe_calls.get() >= MAX_PROBE_CALLS {
            return Some(None);
        }
        self.probe_calls.set(self.probe_calls.get() + 1);
        if !self.probed.borrow().contains(sql) {
            if self.budget.get() == 0 {
                return Some(None);
            }
            self.budget.set(self.budget.get() - 1);
            self.probed.borrow_mut().insert(sql.to_string());
        }
        let assumptions = self.assumptions.get();
        self.probing.borrow_mut().push(sql.to_string());
        let outcome = self.probe_inner(sql, trust);
        self.probing.borrow_mut().pop();
        if let (true, Some(settled)) = (self.assumptions.get() == assumptions, &outcome) {
            self.settled.borrow_mut().insert(sql.to_string(), settled.clone());
        }
        outcome
    }

    fn probe_inner(&self, sql: &str, trust: &Trust) -> Lazy<Option<DescribedStatement>> {
        let pinned = self.pin(sql);
        let raw = self.host.describe(&pinned)?;
        let Ok(raw) = raw else { return Some(None) };
        let parsed = analyze::analyze(sql).ok();
        // A view's statement may read relations the top statement does not name, a view among
        // them, whose own statement then settles its columns: their catalog rows, read as the top
        // statement's are. A probe over the top statement's relations asks nothing more.
        let unread: Vec<String> = {
            let known = self.relations.borrow();
            relations_of(parsed.as_ref()).into_iter().filter(|n| !known.values().any(|(name, _)| name == n)).collect()
        };
        self.prefetch(&unread)?;
        let mut oids: Vec<u32> = raw.columns.iter().map(|c| c.oid).collect();
        oids.extend(raw.params.iter().copied());
        let types = self.types(&oids);
        // A JSON column a JSON constructor reads keeps the schema its CHECKs hold it to, and a
        // string column the strings they keep it to.
        let checks = self.checks(sql, &raw, parsed.as_ref(), &[], true);
        let settled = self.settle(sql, &raw, parsed.as_ref(), trust);
        let (Some(()), Some(mut described), Some(checks)) = (types, settled, checks) else { return None };
        self.stored_sublinks(sql, &mut described, trust);
        for (col, raw) in described.columns.iter_mut().zip(&raw.columns) {
            if let (true, Some(values)) = (trust.builtin("="), checks.values.get(&(raw.table, raw.attnum))) {
                col.values = Some(values.clone());
            }
            if let Some(document) = checks.documents.get(&(raw.table, raw.attnum)) {
                col.document = Some(document.clone());
            }
        }
        self.refine(sql, &mut described, trust)?;
        self.without_missing_rows(sql, parsed.as_ref(), &raw, &mut described);
        Some(Some(described))
    }

    /// A probe reads the main statement's placeholders, some of them only, and the server cannot
    /// type `$2` in a statement that never mentions `$1`: pin the type the main describe settled
    /// on each, and number them from 1 in their order, which a probe that is only described is
    /// free to.
    fn pin(&self, sql: &str) -> String {
        let Ok(lexed) = lex::lex(sql, Dialect::Postgres) else { return sql.to_string() };
        if lexed.placeholders.is_empty() {
            return sql.to_string();
        }
        let mut numbers: Vec<usize> = lexed.placeholders.iter().filter_map(|p| p.name.parse::<usize>().ok()).collect();
        numbers.sort_unstable();
        numbers.dedup();
        let casts = self.casts.borrow();
        lex::rewrite_with(sql, &lexed, Target::Dollar, &|name, rendered| {
            let Some(n) = name.parse::<usize>().ok() else { return rendered.to_string() };
            let at = numbers.iter().position(|m| *m == n).map_or(n, |i| i + 1);
            match casts.get(n.wrapping_sub(1)).cloned().flatten() {
                Some(c) => format!("(${at}::{c})"),
                None => format!("${at}"),
            }
        })
        .sql
    }
}

/// An `ON CONFLICT (a, b)` that names its arbiter index by plain columns alone.
#[derive(Debug, PartialEq)]
struct ConflictTarget {
    /// The INSERT's target, as the statement writes it.
    relation: String,
    columns: Vec<String>,
    /// Whether the columns are followed by a WHERE, which a partial index's predicate is matched by.
    filtered: bool,
    /// Where the column list opens, in the statement's text.
    start: usize,
}

/// The conflict targets of every INSERT in a statement that are lists of plain columns. `ON
/// CONSTRAINT name` names its index, and a target with an expression (`(lower(email))`) is matched
/// against index expressions the build does not compare: neither is one.
fn conflict_targets(sql: &str) -> Vec<ConflictTarget> {
    let Ok(parsed) = crate::stack::parse(sql) else { return Vec::new() };
    let Ok(tree) = serde_json::to_value(&parsed.protobuf) else { return Vec::new() };
    fn walk(v: &Value, out: &mut Vec<ConflictTarget>) {
        match v {
            Value::Object(map) => {
                if let Some(target) = map.get("InsertStmt").and_then(target) {
                    out.push(target);
                }
                map.values().for_each(|child| walk(child, out));
            }
            Value::Array(items) => items.iter().for_each(|child| walk(child, out)),
            _ => {}
        }
    }
    fn target(insert: &Value) -> Option<ConflictTarget> {
        let relation = insert.get("relation")?;
        let name = relation.get("relname")?.as_str()?;
        let relation = match relation.get("schemaname").and_then(Value::as_str) {
            Some(schema) if !schema.is_empty() => format!("{schema}.{name}"),
            _ => name.to_string(),
        };
        let infer = insert.get("on_conflict_clause")?.get("infer")?;
        if infer.get("conname").and_then(Value::as_str).is_some_and(|c| !c.is_empty()) {
            return None;
        }
        let columns: Vec<String> = infer
            .get("index_elems")?
            .as_array()?
            .iter()
            .map(|e| {
                let elem = e.get("node")?.get("IndexElem")?;
                let name = elem.get("name")?.as_str().filter(|n| !n.is_empty())?;
                elem.get("expr").is_none_or(Value::is_null).then(|| name.to_string())
            })
            .collect::<Option<_>>()?;
        if columns.is_empty() {
            return None;
        }
        let filtered = infer.get("where_clause").is_some_and(|w| !w.is_null());
        let start = usize::try_from(infer.get("location")?.as_i64()?).ok()?;
        Some(ConflictTarget { relation, columns, filtered, start })
    }
    let mut out = Vec::new();
    walk(&tree, &mut out);
    out
}

/// Every parameter that is, bare or under casts and collations, a whole key of an ORDER BY (a
/// statement's, a window's or an aggregate's), a GROUP BY, a window's PARTITION BY or a DISTINCT
/// ON, anywhere in the statement: its offset in `sql`, the clause, and whether the clause has no
/// other key. A key that only holds a parameter (`case :sort when 'name' then name end`) is none,
/// and neither is a grouping set's.
fn constant_keys(sql: &str) -> Vec<(usize, KeyClause, bool)> {
    let Ok(parsed) = crate::stack::parse(sql) else { return Vec::new() };
    let Ok(tree) = serde_json::to_value(&parsed.protobuf) else { return Vec::new() };
    fn param(wrapper: &Value) -> Option<usize> {
        let node = wrapper.get("node")?;
        if let Some(p) = node.get("ParamRef") {
            return usize::try_from(p.get("location")?.as_i64()?).ok();
        }
        node.get("TypeCast").or_else(|| node.get("CollateClause")).and_then(|c| c.get("arg")).and_then(param)
    }
    fn keys(list: Option<&Value>, clause: KeyClause, out: &mut Vec<(usize, KeyClause, bool)>) {
        let items = list.and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        out.extend(items.iter().filter_map(param).map(|at| (at, clause, items.len() == 1)));
    }
    fn walk(v: &Value, out: &mut Vec<(usize, KeyClause, bool)>) {
        match v {
            Value::Object(map) => {
                if let Some(sort) = map.get("SortBy") {
                    out.extend(sort.get("node").and_then(param).map(|at| (at, KeyClause::OrderBy, true)));
                }
                if let Some(select) = map.get("SelectStmt") {
                    keys(select.get("group_clause"), KeyClause::GroupBy, out);
                    keys(select.get("distinct_clause"), KeyClause::DistinctOn, out);
                }
                // A window named in a WINDOW clause, or written in a call's OVER.
                for window in [map.get("WindowDef"), map.get("FuncCall").and_then(|f| f.get("over"))].into_iter().flatten() {
                    keys(window.get("partition_clause"), KeyClause::PartitionBy, out);
                }
                map.values().for_each(|child| walk(child, out));
            }
            Value::Array(items) => items.iter().for_each(|child| walk(child, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(&tree, &mut out);
    out
}

/// The errors of [`constant_keys`], at the placeholders as the author wrote them.
fn constant_key_diagnostics(sql: &str, lexed: &Lexed, rewritten: &Rewritten) -> Vec<Diagnostic> {
    let mut found = constant_keys(&rewritten.sql);
    found.sort_by_key(|&(at, ..)| at);
    found
        .into_iter()
        .filter_map(|(at, clause, alone)| {
            let start = rewritten.to_original(at);
            let p = lexed.placeholders.iter().find(|p| p.start == start)?;
            Some(constant_key(sql, p.start, p.end, clause, alone))
        })
        .collect()
}

/// Why the server refuses every execution of an INSERT with this conflict target, when it does:
/// no unique index of the table has exactly the target's columns as its key, so the planner infers
/// no arbiter ("there is no unique or exclusion constraint matching the ON CONFLICT
/// specification", raised when the statement is planned, after it prepared). `rows` are
/// [`ARBITERS_QUERY`]'s. An index matches only when its key columns are the target's as a set, it
/// has no expression among them, and its predicate, if it has one, follows from the target's
/// WHERE: an implication the build does not judge, so a partial index of those columns is taken
/// to match a target with a WHERE, and never one without. Nothing is said of a relation the rows
/// do not list, or one a rule rewrites the INSERT of.
fn unmatched_conflict(target: &ConflictTarget, rows: &[Vec<Value>], catalog: &Catalog) -> Option<String> {
    if catalog.rewritten.get(&target.relation).is_some_and(|events| events & cardinality::TRIGGER_ROW_INSERT != 0) {
        return None;
    }
    let mut listed = false;
    let mut partial: Option<&str> = None;
    let wanted: HashSet<&str> = target.columns.iter().map(String::as_str).collect();
    for row in rows.iter().filter(|row| row.first().and_then(as_str) == Some(target.relation.as_str())) {
        listed = true;
        let Some(parts) = row.get(1).and_then(Value::as_array) else { continue };
        let key: Option<HashSet<&str>> = parts.iter().map(Value::as_str).collect();
        if key.is_none_or(|key| key != wanted) {
            continue;
        }
        match row.get(2).and_then(as_str) {
            Some(predicate) if !target.filtered => partial = Some(predicate),
            _ => return None,
        }
    }
    if !listed {
        return None;
    }
    let columns = target.columns.join(", ");
    Some(match partial {
        Some(predicate) => format!(
            "ON CONFLICT ({columns}) matches no unique index of `{}`: its unique index on ({columns}) is partial (WHERE {predicate}), and only a target that repeats the predicate, ON CONFLICT ({columns}) WHERE …, infers it. The server prepares this statement and refuses every execution (42P10).",
            target.relation
        ),
        None => format!(
            "ON CONFLICT ({columns}) matches no unique index or constraint of `{}`: none has exactly ({columns}) as its key. The server prepares this statement and refuses every execution (42P10).",
            target.relation
        ),
    })
}

/// Per result column of a data-modifying statement, whether the provenance the describe reports
/// for it is of a row the statement may not have ([`crate::returning`]): the old row of an INSERT,
/// the new row of a DELETE, either by the name RETURNING gives it, or a MERGE's source where the
/// MERGE acts on a target row the source does not match. Such a column is NULL there whatever its
/// table's `NOT NULL` says. `origins` says which result columns the describe traced to a table's
/// column (a reference, also under a cast that changes nothing, and a field of a whole row,
/// `(old).name`); an expression that carries no provenance is the expression rules' to settle,
/// which read the same rows. `columns` are a relation's column names, in order: what a `*` expands
/// to, and what a bare name is before it is a row. When the statement's columns cannot be told
/// apart (a `*` over more than the target relation, or over a relation of unknown columns, or a
/// data-modifying WITH query that reads a missing row, whose columns the statement can read
/// through any expression), every column may be NULL.
fn missing_rows(sql: &str, origins: &[bool], columns: &dyn Fn(&str) -> Option<Vec<String>>) -> Vec<bool> {
    let count = origins.len();
    let none = vec![false; count];
    let Ok(parsed) = crate::stack::parse(sql) else { return none };
    let Some(top) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) else { return none };
    // (the rows RETURNING can name, the target relation, the RETURNING list)
    let shape = |node: &NodeEnum| -> Option<(Rows, String, Vec<pg_query::protobuf::Node>)> {
        let (relation, clause) = match node {
            NodeEnum::InsertStmt(s) => (s.relation.as_ref()?, &s.returning_clause),
            NodeEnum::UpdateStmt(s) => (s.relation.as_ref()?, &s.returning_clause),
            NodeEnum::DeleteStmt(s) => (s.relation.as_ref()?, &s.returning_clause),
            NodeEnum::MergeStmt(s) => (s.relation.as_ref()?, &s.returning_clause),
            _ => return None,
        };
        Some((Rows::of(node)?, relation.relname.clone(), jsonshape::returning(clause)))
    };
    let reads_missing = |rows: &Rows, relation: &str, node: &pg_query::protobuf::Node| {
        let known = columns(relation);
        rows.reads_missing(node, &|name| known.as_ref().is_some_and(|k| k.iter().any(|c| c == name)))
    };
    // A data-modifying WITH query under the statement: its RETURNING can feed any column.
    let with = match top {
        NodeEnum::SelectStmt(s) => s.with_clause.as_ref(),
        NodeEnum::InsertStmt(i) => i.with_clause.as_ref(),
        NodeEnum::UpdateStmt(u) => u.with_clause.as_ref(),
        NodeEnum::DeleteStmt(d) => d.with_clause.as_ref(),
        NodeEnum::MergeStmt(m) => m.with_clause.as_ref(),
        _ => None,
    };
    for cte in with.map(|w| w.ctes.as_slice()).unwrap_or_default() {
        let Some(NodeEnum::CommonTableExpr(c)) = cte.node.as_ref() else { continue };
        let Some((rows, relation, returning)) = c.ctequery.as_deref().and_then(|q| q.node.as_ref()).and_then(&shape) else { continue };
        if !rows.all_present() && returning.iter().any(|r| reads_missing(&rows, &relation, r)) {
            return vec![true; count];
        }
    }
    let Some((rows, relation, returning)) = shape(top) else { return none };
    if rows.all_present() {
        return none;
    }
    let mut out = Vec::with_capacity(count);
    for target in &returning {
        let Some(NodeEnum::ResTarget(rt)) = target.node.as_ref() else { return vec![true; count] };
        let Some(val) = rt.val.as_deref() else { return vec![true; count] };
        let star = match val.node.as_ref() {
            Some(NodeEnum::ColumnRef(c)) => match c.fields.split_last() {
                Some((last, qualifier)) if matches!(last.node, Some(NodeEnum::AStar(_))) => Some(qualifier),
                _ => None,
            },
            _ => None,
        };
        let missing = reads_missing(&rows, &relation, val);
        match star {
            None => out.push(missing),
            // `o.*`, the target's own `*`, and a `*` with nothing joined in are the target
            // relation's columns; any other has a width the parse does not show.
            Some(qualifier) => {
                let qualifier = match qualifier {
                    [] => Some(None),
                    [one] => jsonshape::last_string(std::slice::from_ref(one)).map(Some),
                    _ => None,
                };
                let known = qualifier.filter(|q| rows.star_of_target(q.as_deref())).and_then(|_| columns(&relation));
                let Some(known) = known else { return vec![true; count] };
                out.extend(std::iter::repeat(missing).take(known.len()));
            }
        }
    }
    if out.len() != count {
        return vec![true; count];
    }
    out.iter().zip(origins).map(|(missing, origin)| *missing && *origin).collect()
}

/// The system columns every table has, which a bare name reads before any row.
const SYSTEM_COLUMNS: &[&str] = &["tableoid", "xmin", "cmin", "xmax", "cmax", "ctid"];

/// What [`sublink_rows`] finds in a SELECT.
struct SublinkRows {
    /// Per target of the SELECT: for a scalar or `ARRAY` subquery whose one target is the whole
    /// row of a table of its own FROM, a scalar subquery whose one target is an `array_agg` of
    /// such a row, or an `array_agg` of the whole row of a table of the SELECT's FROM (either
    /// `array_agg` alone or under `coalesce(…, <empty array literal>)`), that table, the name the
    /// row goes by there, whether the value is an array of rows, and whether it is an `array_agg`.
    targets: Vec<Option<(String, String, bool, bool)>>,
    /// Every table a FROM anywhere in the statement names.
    tables: Vec<String>,
}

/// Per result column of `sql`, whether the provenance the describe reports for it is its own. A
/// set operation's columns trace to its leftmost arm, so where the statement holds one anywhere,
/// only a bare reference to a column of a table in its own FROM is sure to be that column,
/// whatever its WITH queries and subqueries hold: one read through a WITH query, a derived table
/// or a `*` may come from any arm. A statement libpg_query cannot read owns none.
fn own_provenance(sql: &str, parsed: Option<&Parsed>, count: usize) -> Vec<bool> {
    match parsed {
        Some(p) if !p.has_set_op => return vec![true; count],
        None => return vec![false; count],
        Some(_) => {}
    }
    // The reads line up with the columns only where every target is one column: a `*` among
    // targets as many as the columns is one wide when it is the only one, but two of them may be
    // two columns and none (a WITH query `(select)` has no columns), as [`Engine::item_columns`]
    // refuses too.
    let aligned = |reads: &Vec<infer::ItemRead>| reads.len() == count && reads.iter().filter(|r| matches!(r, infer::ItemRead::Star { .. })).count() <= 1;
    match infer::target_reads(sql).filter(aligned) {
        Some(reads) => reads.iter().map(|r| matches!(r, infer::ItemRead::Table)).collect(),
        None => vec![false; count],
    }
}

/// The targets of a SELECT that are a scalar or `ARRAY` subquery returning the whole rows of one
/// of its own FROM's tables (`(select c from child c …)`, `array(select c from child c …)`), or an
/// `array_agg` of the whole row of one of the SELECT's own (`array_agg(e)`, `array_agg(e.*)`) or
/// of a scalar subquery's own (`(select array_agg(c) from child c …)`), alone or under
/// `coalesce(…, '{}')`, and every table a FROM of the statement names. `None` where a name could mean something the
/// statement's text does not show: a FROM item that is no plain table (a function, a derived
/// table, `JSON_TABLE`), a table named with its schema, or one named like a WITH query. A bare
/// name reads a column of any FROM in scope before it reads a row, which
/// [`Engine::stored_sublinks`] rules out with the tables' columns.
fn sublink_rows(sql: &str) -> Option<SublinkRows> {
    use pg_query::protobuf::node::Node as N;
    use pg_query::protobuf::{Node, SetOperation, SubLinkType};
    use pg_query::NodeRef;
    let parsed = crate::stack::parse(sql).ok()?;
    let (mut tables, mut ctes) = (Vec::new(), Vec::new());
    for (node, _, _, _) in parsed.protobuf.nodes() {
        match node {
            NodeRef::RangeVar(r) if r.schemaname.is_empty() => tables.push(r.relname.clone()),
            NodeRef::CommonTableExpr(c) => ctes.push(c.ctename.clone()),
            NodeRef::RangeVar(_) | NodeRef::RangeSubselect(_) | NodeRef::RangeFunction(_) | NodeRef::RangeTableFunc(_) | NodeRef::RangeTableSample(_) | NodeRef::JsonTable(_) => return None,
            _ => {}
        }
    }
    if tables.iter().any(|t| ctes.contains(t)) {
        return None;
    }
    /// The table of a FROM item that goes by `name`: the alias where it has one, else its name.
    fn named(item: &Node, name: &str) -> Option<String> {
        match item.node.as_ref()? {
            N::RangeVar(r) => (r.alias.as_ref().map_or(r.relname.as_str(), |a| a.aliasname.as_str()) == name).then(|| r.relname.clone()),
            N::JoinExpr(j) if j.alias.is_none() => j.larg.as_deref().and_then(|l| named(l, name)).or_else(|| j.rarg.as_deref().and_then(|r| named(r, name))),
            _ => None,
        }
    }
    /// The name a bare `e` or `e.*` reads a whole row by.
    fn whole(node: &Node) -> Option<String> {
        let N::ColumnRef(c) = node.node.as_ref()? else { return None };
        match c.fields.as_slice() {
            [name] | [name, Node { node: Some(N::AStar(_)) }] => match name.node.as_ref()? {
                N::String(s) => Some(s.sval.clone()),
                _ => None,
            },
            _ => None,
        }
    }
    /// Whether a value is an empty array written as a literal: `'{}'`, `array[]`, cast or not.
    fn empty(node: &Node) -> bool {
        match node.node.as_ref() {
            Some(N::TypeCast(c)) => c.arg.as_deref().is_some_and(empty),
            Some(N::AConst(c)) => matches!(&c.val, Some(pg_query::protobuf::a_const::Val::Sval(s)) if s.sval.trim() == "{}"),
            Some(N::AArrayExpr(a)) => a.elements.is_empty(),
            _ => false,
        }
    }
    /// The table and the name of the whole row an `array_agg` of a table's whole row in `from`
    /// aggregates (`array_agg(e)`, `array_agg(e.*)`), alone or with an empty array literal for no
    /// rows (`coalesce(array_agg(e), '{}')`): an array of the rows the query aggregates, each a
    /// stored row, or NULL where an outer join nulled the table, which the array's own elements
    /// say.
    fn aggregated(value: &Node, from: &[Node]) -> Option<(String, String)> {
        match value.node.as_ref()? {
            N::CoalesceExpr(c) => match c.args.as_slice() {
                [agg, default] if empty(default) => aggregated(agg, from).filter(|_| matches!(agg.node, Some(N::FuncCall(_)))),
                _ => None,
            },
            N::FuncCall(f) => {
                let [arg] = f.args.as_slice() else { return None };
                if crate::infer::builtin_name(&f.funcname).as_deref() != Some("array_agg") || f.agg_star || f.agg_within_group {
                    return None;
                }
                let name = whole(arg)?;
                Some((from.iter().find_map(|item| named(item, &name))?, name))
            }
            _ => None,
        }
    }
    let N::SelectStmt(select) = parsed.protobuf.stmts.first()?.stmt.as_deref()?.node.as_ref()? else { return None };
    let row = |target: &Node| -> Option<(String, String, bool, bool)> {
        let N::ResTarget(t) = target.node.as_ref()? else { return None };
        let value = t.val.as_deref()?;
        if matches!(value.node.as_ref()?, N::FuncCall(_) | N::CoalesceExpr(_)) {
            let (relation, name) = aggregated(value, &select.from_clause)?;
            return Some((relation, name, true, true));
        }
        let N::SubLink(s) = value.node.as_ref()? else { return None };
        let array = s.sub_link_type == SubLinkType::ArraySublink as i32;
        if !array && s.sub_link_type != SubLinkType::ExprSublink as i32 {
            return None;
        }
        let N::SelectStmt(inner) = s.subselect.as_deref()?.node.as_ref()? else { return None };
        if inner.op != SetOperation::SetopNone as i32 {
            return None;
        }
        let [only] = inner.target_list.as_slice() else { return None };
        let N::ResTarget(only) = only.node.as_ref()? else { return None };
        // A scalar subquery's own `array_agg` of a whole row of its FROM: the subquery's, since no
        // table the statement reads has a column of the name ([`Engine::stored_sublinks`]).
        if !array {
            if let Some((relation, name)) = aggregated(only.val.as_deref()?, &inner.from_clause) {
                return Some((relation, name, true, true));
            }
        }
        let N::ColumnRef(c) = only.val.as_deref()?.node.as_ref()? else { return None };
        let [field] = c.fields.as_slice() else { return None };
        let N::String(name) = field.node.as_ref()? else { return None };
        let relation = inner.from_clause.iter().find_map(|item| named(item, &name.sval))?;
        Some((relation, name.sval.clone(), array, false))
    };
    if select.op != SetOperation::SetopNone as i32 {
        return None;
    }
    Some(SublinkRows { targets: select.target_list.iter().map(row).collect(), tables })
}

/// What a column's CHECKs say of its values ([`Engine::checks`]), by relation and attribute.
#[derive(Default)]
struct Checks {
    values: HashMap<(u32, i32), Vec<String>>,
    documents: HashMap<(u32, i32), Value>,
    /// The schemas behind each document, which a parameter written to several columns meets all of.
    sources: HashMap<(u32, i32), Vec<(String, Validators)>>,
}

/// One column of a FROM item's own settled result ([`Engine::item_columns`]): its name, whether it
/// may be NULL, and the schema and the strings the item settled its values to, as for a read of
/// a JSON column at literal keys inside it ([`infer::json_read`]).
#[derive(Debug, Clone)]
struct ItemColumn {
    name: String,
    nullable: bool,
    document: Option<Value>,
    values: Option<Vec<String>>,
}

/// `ty` as the values `document` admits, when it is a JSON value a CHECK holds to a JSON Schema
/// ([`crate::schema_type`]).
pub fn with_document(ty: Type, document: Option<&Value>) -> Type {
    match (ty, document) {
        (Type::Json { schema: None }, Some(schema)) => Type::Json { schema: Some(schema.clone()) },
        (ty, _) => ty,
    }
}

/// One argument of a function a CHECK calls, as [`check_documents`] reads it.
#[derive(Debug, Clone, PartialEq)]
enum CheckArg {
    /// A string constant, cast or not to `json`, `jsonb` or `text`: a schema's text.
    Schema(String),
    /// The column itself (`VALUE` in a domain's), cast or not to `json` or `jsonb`.
    Column,
    Other,
}

/// The calls of a CHECK, as the server prints it, that hold the column `column` (`VALUE` in a
/// domain's) to something: each function's name and arguments, where the call stands alone,
/// among the conjuncts of an AND (each one holds), or beside `column IS NULL` under an OR, which
/// lets NULL through as well. A strict function that answers true or false for every value is
/// NULL only for a NULL, so a row such a CHECK lets in holds a value the call is true of; which
/// functions are such a validator, and which arguments hold the schema and the value, is
/// [`crate::schema_type::postgres_validator`]'s to say.
fn check_documents(text: &str, column: &str) -> Vec<(String, Vec<CheckArg>)> {
    use pg_query::protobuf::node::Node as N;
    use pg_query::protobuf::Node;
    let Ok(parsed) = crate::stack::parse(&format!("SELECT {text}")) else { return Vec::new() };
    let Some(N::SelectStmt(select)) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|s| s.node.as_ref()) else { return Vec::new() };
    let Some(N::ResTarget(target)) = select.target_list.first().and_then(|t| t.node.as_ref()) else { return Vec::new() };
    fn uncast<'n>(node: &'n Node, through: &[&str]) -> &'n Node {
        match node.node.as_ref() {
            Some(N::TypeCast(t)) => {
                let name = t.type_name.as_ref().and_then(|n| crate::jsonshape::last_string(&n.names)).unwrap_or_default();
                match (through.contains(&name.as_str()), t.arg.as_deref()) {
                    (true, Some(arg)) => uncast(arg, through),
                    _ => node,
                }
            }
            _ => node,
        }
    }
    let is_column = |node: &Node| match uncast(node, &["json", "jsonb"]).node.as_ref() {
        Some(N::ColumnRef(c)) => matches!(c.fields.as_slice(), [f] if matches!(f.node.as_ref(), Some(N::String(s)) if s.sval == column || s.sval == "value")),
        _ => false,
    };
    let arg = |node: &Node| match uncast(node, &["json", "jsonb", "text"]).node.as_ref() {
        Some(N::AConst(c)) => match &c.val {
            Some(pg_query::protobuf::a_const::Val::Sval(s)) => CheckArg::Schema(s.sval.clone()),
            _ => CheckArg::Other,
        },
        _ if is_column(node) => CheckArg::Column,
        _ => CheckArg::Other,
    };
    fn calls(node: &Node, is_column: &dyn Fn(&Node) -> bool, arg: &dyn Fn(&Node) -> CheckArg, out: &mut Vec<(String, Vec<CheckArg>)>) {
        use pg_query::protobuf::{BoolExprType, NullTestType};
        match node.node.as_ref() {
            Some(N::BoolExpr(b)) if b.boolop == BoolExprType::AndExpr as i32 => {
                for a in &b.args {
                    calls(a, is_column, arg, out);
                }
            }
            Some(N::BoolExpr(b)) if b.boolop == BoolExprType::OrExpr as i32 && b.args.len() == 2 => {
                let null_test = |a: &Node| matches!(a.node.as_ref(), Some(N::NullTest(t)) if t.nulltesttype == NullTestType::IsNull as i32 && t.arg.as_deref().is_some_and(is_column));
                match (null_test(&b.args[0]), null_test(&b.args[1])) {
                    (true, false) => calls(&b.args[1], is_column, arg, out),
                    (false, true) => calls(&b.args[0], is_column, arg, out),
                    _ => {}
                }
            }
            Some(N::FuncCall(f)) if !f.agg_star && !f.func_variadic && f.agg_filter.is_none() && f.over.is_none() => {
                if let Some(name) = crate::jsonshape::last_string(&f.funcname) {
                    out.push((name, f.args.iter().map(arg).collect()));
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Some(val) = target.val.as_deref() {
        calls(val, &is_column, &arg, &mut out);
    }
    out
}

/// `ty` as the union of `values` when it is a string a CHECK keeps to them; the labels of an enum
/// say the same of a column whose type is one.
pub fn with_values(ty: Type, values: Option<&[String]>) -> Type {
    match (ty, values) {
        (Type::Scalar { name }, Some(values)) if (name == "text" || name == "varchar") && !values.is_empty() => Type::Enum { name, labels: values.to_vec() },
        (ty, _) => ty,
    }
}

/// The strings a CHECK, as the server prints it, keeps the column `column` to (`VALUE` in a
/// domain's): `col = ANY (ARRAY['a'::text, …])` or `col = 'a'`, alone, among the conjuncts of an
/// AND (each one of them restricts), or beside `col IS NULL` under an OR, which only lets NULL
/// through as well. A cast to a string type around the column or a value is the server's own
/// (`v::text = ANY (ARRAY['x'::character varying]::text[])` for a `varchar`). Anything else
/// proves nothing.
/// Whether a CHECK, as the server prints it, refuses null in `column` (`value` for a domain's):
/// one of its conjuncts tests the column `IS NOT NULL`, which a null makes false where any other
/// test of it would be unknown, and so pass.
fn refuses_null(text: &str, column: &str) -> bool {
    use pg_query::protobuf::node::Node as N;
    use pg_query::protobuf::{BoolExprType, NullTestType};
    let Ok(parsed) = crate::stack::parse(&format!("SELECT {text}")) else { return false };
    let Some(N::SelectStmt(select)) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|s| s.node.as_ref()) else { return false };
    let Some(N::ResTarget(target)) = select.target_list.first().and_then(|t| t.node.as_ref()) else { return false };
    fn conjunct(node: &pg_query::protobuf::Node, column: &str) -> bool {
        match node.node.as_ref() {
            Some(N::BoolExpr(b)) if b.boolop == BoolExprType::AndExpr as i32 => b.args.iter().any(|a| conjunct(a, column)),
            Some(N::NullTest(t)) if t.nulltesttype == NullTestType::IsNotNull as i32 => match t.arg.as_deref().and_then(|a| a.node.as_ref()) {
                Some(N::ColumnRef(c)) => matches!(c.fields.as_slice(), [f] if matches!(f.node.as_ref(), Some(N::String(s)) if s.sval == column)),
                _ => false,
            },
            _ => false,
        }
    }
    target.val.as_deref().is_some_and(|v| conjunct(v, column))
}

fn check_values(text: &str, column: &str) -> Option<Vec<String>> {
    use pg_query::protobuf::node::Node as N;
    let parsed = crate::stack::parse(&format!("SELECT {text}")).ok()?;
    let Some(N::SelectStmt(select)) = parsed.protobuf.stmts.first()?.stmt.as_deref()?.node.as_ref() else { return None };
    let Some(N::ResTarget(target)) = select.target_list.first()?.node.as_ref() else { return None };
    fn uncast(node: &pg_query::protobuf::Node) -> &pg_query::protobuf::Node {
        match node.node.as_ref() {
            Some(N::TypeCast(t)) => {
                let name = t.type_name.as_ref().and_then(|n| crate::jsonshape::last_string(&n.names)).unwrap_or_default();
                match (["text", "varchar"].contains(&name.as_str()), t.arg.as_deref()) {
                    (true, Some(arg)) => uncast(arg),
                    _ => node,
                }
            }
            _ => node,
        }
    }
    let is_column = |node: &pg_query::protobuf::Node| match uncast(node).node.as_ref() {
        Some(N::ColumnRef(c)) => matches!(c.fields.as_slice(), [f] if matches!(f.node.as_ref(), Some(N::String(s)) if s.sval == column || s.sval == "value")),
        _ => false,
    };
    let string = |node: &pg_query::protobuf::Node| match uncast(node).node.as_ref() {
        Some(N::AConst(c)) => match &c.val {
            Some(pg_query::protobuf::a_const::Val::Sval(s)) => Some(s.sval.clone()),
            _ => None,
        },
        _ => None,
    };
    fn list(node: &pg_query::protobuf::Node, is_column: &dyn Fn(&pg_query::protobuf::Node) -> bool, string: &dyn Fn(&pg_query::protobuf::Node) -> Option<String>) -> Option<Vec<String>> {
        use pg_query::protobuf::{AExprKind, BoolExprType, NullTestType};
        match node.node.as_ref()? {
            N::BoolExpr(b) if b.boolop == BoolExprType::AndExpr as i32 => {
                let mut lists = b.args.iter().filter_map(|a| list(a, is_column, string));
                let first = lists.next()?;
                Some(lists.fold(first, |kept, other| kept.into_iter().filter(|v| other.contains(v)).collect()))
            }
            N::BoolExpr(b) if b.boolop == BoolExprType::OrExpr as i32 && b.args.len() == 2 => {
                let null_test = |a: &pg_query::protobuf::Node| matches!(a.node.as_ref(), Some(N::NullTest(t)) if t.nulltesttype == NullTestType::IsNull as i32 && t.arg.as_deref().is_some_and(is_column));
                match (null_test(&b.args[0]), null_test(&b.args[1])) {
                    (true, false) => list(&b.args[1], is_column, string),
                    (false, true) => list(&b.args[0], is_column, string),
                    _ => None,
                }
            }
            N::AExpr(e) if crate::jsonshape::last_string(&e.name).as_deref() == Some("=") && e.name.len() == 1 => {
                let (l, r) = (e.lexpr.as_deref()?, e.rexpr.as_deref()?);
                if e.kind == AExprKind::AexprOpAny as i32 && is_column(l) {
                    let array = match r.node.as_ref()? {
                        N::TypeCast(t) => t.arg.as_deref()?,
                        _ => r,
                    };
                    let Some(N::AArrayExpr(a)) = array.node.as_ref() else { return None };
                    a.elements.iter().map(string).collect()
                } else if e.kind == AExprKind::AexprOp as i32 {
                    match (is_column(l), is_column(r)) {
                        (true, false) => string(r).map(|v| vec![v]),
                        (false, true) => string(l).map(|v| vec![v]),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    list(target.val.as_deref()?, &is_column, &string)
}

/// `ty` with what the statement says of its elements, when it is an array.
pub fn with_items(ty: Type, nullable_items: Option<bool>) -> Type {
    match ty {
        Type::Array { of, delimiter, .. } => Type::Array { of, nullable_items, delimiter },
        other => other,
    }
}

/// The tables on the nullable side of an outer join, with the ones a strict WHERE reduced only by
/// operators the catalog says a user or an extension also defined put back: such an operator may
/// be true of NULL, so its conjunct does not prove the joined row exists. A WITH query among them
/// brings the relations it reads, which is where the describe traces its columns.
fn unreduced(parsed: &Parsed, trust: &Trust) -> Vec<String> {
    let mut tables = parsed.nullable_tables.clone();
    for (relation, witnesses) in &parsed.reductions {
        if witnesses.iter().all(|names| names.iter().any(|n| !trust.holds(n))) {
            tables.push(relation.clone());
        }
    }
    parsed.nulled_tables(tables)
}

/// The relations a statement reads or writes, as the parse names them, sorted. libpg_query
/// collects them through a hash set, so each parse lists them in another order; a query that
/// carried that order would be a new need every round, and the replay would settle only when two
/// rounds happened to agree.
fn relations_of(parsed: Option<&Parsed>) -> Vec<String> {
    let Some(parsed) = parsed else { return Vec::new() };
    sorted(parsed.read_set.iter().chain(&parsed.write_set).cloned().collect())
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort_unstable();
    names.dedup();
    names
}

fn build_catalog(operators: &HashSet<String>, rows: CatalogRows) -> Catalog {
    let mut catalog = Catalog { set_returning_operators: operators.clone(), columns: Columns::from_rows(&rows.columns), ..Catalog::default() };
    for row in rows.columns {
        if let (Some(rel), Some(cols)) = (row.first().and_then(as_str), row.get(1).and_then(as_strings)) {
            let types: Vec<Option<u32>> = row.get(2).and_then(Value::as_array).map(|a| a.iter().map(as_u32).collect()).unwrap_or_default();
            // A host that answers without the fourth column reports no collation: every column
            // is then taken as one whose equality is not identity.
            let deterministic: Vec<bool> = row.get(3).and_then(Value::as_array).map(|a| a.iter().map(|v| as_bool(v).unwrap_or(false)).collect()).unwrap_or_default();
            // A host that answers without the fifth column reports no sequence.
            if row.get(4).and_then(as_bool).unwrap_or(false) {
                catalog.single_row.insert(rel.to_string());
            }
            if types.len() == cols.len() {
                for (i, col) in cols.iter().enumerate() {
                    if !deterministic.get(i).copied().unwrap_or(false) {
                        catalog.nondeterministic.entry(rel.to_string()).or_default().insert(col.clone());
                    }
                }
                let known = catalog.column_types.entry(rel.to_string()).or_default();
                for (col, ty) in cols.into_iter().zip(types) {
                    if let Some(ty) = ty {
                        known.insert(col, ty);
                    }
                }
            }
        }
    }
    for row in rows.keys {
        if let (Some(rel), Some(parts)) = (row.first().and_then(as_str), row.get(1).and_then(as_strings)) {
            // An expression part's type goes under the part's own name (`(lower(email))`), beside
            // the relation's columns, for the comparison with it.
            let types: Vec<Option<u32>> = row.get(2).and_then(Value::as_array).map(|a| a.iter().map(as_u32).collect()).unwrap_or_default();
            if let Some(known) = catalog.column_types.get_mut(rel) {
                for (part, ty) in parts.iter().zip(types) {
                    if let (true, Some(ty)) = (part.starts_with('('), ty) {
                        known.insert(part.clone(), ty);
                    }
                }
            }
            match row.get(3).and_then(as_str) {
                Some(predicate) => catalog.partial_keys.entry(rel.to_string()).or_default().push((parts, predicate.to_string())),
                None => catalog.unique_keys.entry(rel.to_string()).or_default().push(parts),
            }
        }
    }
    for row in rows.triggers {
        if let (Some(rel), Some(events)) = (row.first().and_then(as_str), row.get(1).and_then(as_u32)) {
            if events != 0 {
                catalog.skipping_triggers.insert(rel.to_string(), events as i32);
            }
            // A host that answers without the third column reports no rules: every write is then
            // taken as one a rule may rewrite.
            let rewritten = row.get(2).map_or(Some(28), as_u32).unwrap_or(0);
            if rewritten != 0 {
                catalog.rewritten.insert(rel.to_string(), rewritten as i32);
            }
        }
    }
    for row in rows.functions {
        if let (Some(name), Some(retset)) = (row.first().and_then(as_str), row.get(1).and_then(as_bool)) {
            if retset {
                catalog.set_returning.insert(name.to_string());
            }
            // A host that answers without the third column reports no volatility: every call is
            // then taken as volatile, which refuses a key pinned to any call.
            if row.get(2).and_then(as_bool).unwrap_or(true) {
                catalog.volatile.insert(name.to_string());
            }
            catalog.functions.insert(name.to_string());
        }
    }
    catalog
}

/// For each result column that the top SELECT's target list writes as `alias.column`, or that
/// `alias.*` expands to, where the alias is a table of the top FROM: that table's name, and
/// whether an outer join there nulls the alias (a WHERE that rules the NULLs out is not read,
/// which leaves the column nullable). `width` is how many columns a table has, which `alias.*`
/// expands to. Nothing where a bare `*`, or a star of an item `width` does not answer for, hides
/// which target is which column, or the targets are not the columns.
fn aliased_columns(sql: &str, count: usize, width: &dyn Fn(&str) -> Option<usize>) -> Vec<Option<(String, bool)>> {
    use pg_query::protobuf::{JoinType, Node};
    fn items(node: &Node, nulled: bool, ctes: &[String], out: &mut HashMap<String, Option<(String, bool)>>, qualified: &mut Vec<String>) {
        match node.node.as_ref() {
            Some(NodeEnum::RangeVar(rv)) => {
                let alias = rv.alias.as_ref().map_or(rv.relname.clone(), |a| a.aliasname.clone());
                let table = (!(rv.schemaname.is_empty() && ctes.contains(&rv.relname))).then(|| (rv.relname.clone(), nulled));
                if !rv.schemaname.is_empty() {
                    qualified.push(alias.clone());
                }
                out.insert(alias, table);
            }
            Some(NodeEnum::JoinExpr(j)) => {
                // A join with an alias of its own hides the names inside it.
                if j.alias.is_some() {
                    return;
                }
                let (left, right) = match j.jointype {
                    t if t == JoinType::JoinLeft as i32 => (nulled, true),
                    t if t == JoinType::JoinRight as i32 => (true, nulled),
                    t if t == JoinType::JoinFull as i32 => (true, true),
                    _ => (nulled, nulled),
                };
                for (side, nulled) in [(j.larg.as_deref(), left), (j.rarg.as_deref(), right)] {
                    if let Some(side) = side {
                        items(side, nulled, ctes, out, qualified);
                    }
                }
            }
            Some(NodeEnum::RangeSubselect(r)) => {
                if let Some(a) = &r.alias {
                    out.insert(a.aliasname.clone(), None);
                }
            }
            Some(NodeEnum::RangeFunction(f)) => {
                if let Some(a) = &f.alias {
                    out.insert(a.aliasname.clone(), None);
                }
            }
            _ => {}
        }
    }
    let Ok(parsed) = crate::stack::parse(sql) else { return Vec::new() };
    let Some(NodeEnum::SelectStmt(select)) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) else { return Vec::new() };
    if select.op != pg_query::protobuf::SetOperation::SetopNone as i32 && select.op != 0 {
        return Vec::new();
    }
    let ctes: Vec<String> = select
        .with_clause
        .iter()
        .flat_map(|w| w.ctes.iter())
        .filter_map(|c| match c.node.as_ref() {
            Some(NodeEnum::CommonTableExpr(c)) => Some(c.ctename.clone()),
            _ => None,
        })
        .collect();
    let (mut readings, mut qualified) = (HashMap::new(), Vec::new());
    for item in &select.from_clause {
        items(item, false, &ctes, &mut readings, &mut qualified);
    }
    let mut out = Vec::with_capacity(count);
    for target in &select.target_list {
        let Some(NodeEnum::ResTarget(rt)) = target.node.as_ref() else { return Vec::new() };
        // `(x).*` is as many columns as `x` has fields, which nothing here counts.
        if matches!(rt.val.as_deref().and_then(|v| v.node.as_ref()), Some(NodeEnum::AIndirection(i)) if i.indirection.iter().any(|n| matches!(n.node, Some(NodeEnum::AStar(_))))) {
            return Vec::new();
        }
        if rt.val.as_deref().is_some_and(jsonshape::is_star) {
            // `a.*` is every column of the table `a` is a reading of, in order, as `a.column` is one.
            // A table named with its schema may be another than the one its name finds.
            let alias = match rt.val.as_deref().and_then(|v| v.node.as_ref()) {
                Some(NodeEnum::ColumnRef(c)) if c.fields.len() == 2 => match c.fields[0].node.as_ref() {
                    Some(NodeEnum::String(alias)) if !qualified.contains(&alias.sval) => Some(alias.sval.as_str()),
                    _ => None,
                },
                _ => None,
            };
            let Some((table, nulled)) = alias.and_then(|a| readings.get(a).cloned().flatten()) else { return Vec::new() };
            let Some(n) = width(&table) else { return Vec::new() };
            out.extend(std::iter::repeat_n(Some((table, nulled)), n));
            continue;
        }
        let reading = match rt.val.as_deref().and_then(|v| v.node.as_ref()) {
            Some(NodeEnum::ColumnRef(c)) if c.fields.len() == 2 => match c.fields[0].node.as_ref() {
                Some(NodeEnum::String(alias)) => readings.get(&alias.sval).cloned().flatten(),
                _ => None,
            },
            _ => None,
        };
        out.push(reading);
    }
    if out.len() != count {
        return Vec::new();
    }
    out
}

/// The relation an INSERT writes and which of its columns each row writes.
struct InsertTarget {
    /// As the statement names it (`users`, `archive.users`).
    relation: String,
    /// The bytes of the name in the statement's `$n` text.
    start: usize,
    end: usize,
    /// The column list, where the statement writes one.
    columns: Option<Vec<String>>,
    /// How many values each row has, where the statement writes no column list: they fill the
    /// first columns. `None` where a `*` hides the count.
    width: Option<usize>,
    /// The positions (in the column list, or among the columns) a row writes `DEFAULT` to.
    defaults: Vec<usize>,
}

/// The INSERT the top statement is (a WITH before it allowed), as [`InsertTarget`] reads it.
fn insert_target(sql: &str) -> Option<InsertTarget> {
    let parsed = crate::stack::parse(sql).ok()?;
    let Some(NodeEnum::InsertStmt(insert)) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) else { return None };
    let rv = insert.relation.as_ref()?;
    let relation = if rv.schemaname.is_empty() { rv.relname.clone() } else { format!("{}.{}", rv.schemaname, rv.relname) };
    let start = usize::try_from(rv.location).ok()?;
    let end = sql[start..].find(|c: char| c.is_whitespace() || c == '(').map_or(sql.len(), |i| start + i);
    let columns = (!insert.cols.is_empty()).then(|| {
        insert.cols.iter().filter_map(|c| match c.node.as_ref() {
            Some(NodeEnum::ResTarget(rt)) => Some(rt.name.clone()),
            _ => None,
        }).collect::<Vec<_>>()
    });
    let (mut width, mut defaults) = (Some(0), Vec::new());
    match insert.select_stmt.as_deref().and_then(|n| n.node.as_ref()) {
        // DEFAULT VALUES writes no column.
        None => {}
        Some(NodeEnum::SelectStmt(select)) if !select.values_lists.is_empty() => {
            width = None;
            let mut starred = false;
            for row in &select.values_lists {
                let Some(NodeEnum::List(items)) = row.node.as_ref() else { return None };
                width = Some(width.map_or(items.items.len(), |w: usize| w.min(items.items.len())));
                // The items after a `(composite).*` are not at their own positions.
                let star = items.items.iter().position(expands);
                starred |= star.is_some();
                for (i, item) in items.items.iter().enumerate().take(star.unwrap_or(usize::MAX)) {
                    if matches!(item.node.as_ref(), Some(NodeEnum::SetToDefault(_))) && !defaults.contains(&i) {
                        defaults.push(i);
                    }
                }
            }
            if starred {
                width = None;
            }
        }
        Some(NodeEnum::SelectStmt(select)) if select.op == pg_query::protobuf::SetOperation::SetopNone as i32 => {
            let star = select.target_list.iter().any(|t| matches!(t.node.as_ref(), Some(NodeEnum::ResTarget(rt)) if rt.val.as_deref().is_some_and(expands)));
            width = (!star).then_some(select.target_list.len());
        }
        _ => width = None,
    }
    Some(InsertTarget { relation, start, end, columns, width, defaults })
}

/// Whether a SELECT's target or a VALUES item stands for as many values as a row has columns:
/// `t.*`, or `(composite).*`, which parses as an indirection ending in the star.
fn expands(node: &pg_query::protobuf::Node) -> bool {
    let last_is_star = |items: &[pg_query::protobuf::Node]| items.last().is_some_and(|f| matches!(f.node.as_ref(), Some(NodeEnum::AStar(_))));
    match node.node.as_ref() {
        Some(NodeEnum::ColumnRef(c)) => last_is_star(&c.fields),
        Some(NodeEnum::AIndirection(i)) => last_is_star(&i.indirection),
        _ => false,
    }
}

/// What the INSERT leaves unwritten of the columns [`REQUIRED_COLUMNS_QUERY`] says each row must
/// write, as the message that says so.
fn unwritten(target: &InsertTarget, rows: &[Vec<Value>]) -> Option<String> {
    let columns: Vec<(String, bool)> = rows.iter().filter_map(|r| Some((as_str(r.first()?)?.to_string(), as_bool(r.get(2)?)?))).collect();
    let missing: Vec<&str> = columns
        .iter()
        .enumerate()
        .filter(|(_, (_, required))| *required)
        .filter(|(i, (name, _))| match &target.columns {
            Some(list) => match list.iter().position(|c| c == name) {
                Some(at) => target.defaults.contains(&at),
                None => true,
            },
            // Positional values fill the first columns; with no count, nothing is known.
            None => target.width.is_some_and(|w| *i >= w || target.defaults.contains(i)),
        })
        .map(|(_, (name, _))| name.as_str())
        .collect();
    let quoted: Vec<String> = missing.iter().map(|m| format!("\"{m}\"")).collect();
    match quoted.as_slice() {
        [] => None,
        [one] => Some(format!("Column {one} of \"{}\" is NOT NULL with no default, and this insert leaves it out or writes DEFAULT to it, so Postgres refuses every row it inserts.", target.relation)),
        many => Some(format!("Columns {} of \"{}\" are NOT NULL with no default, and this insert leaves them out or writes DEFAULT to them, so Postgres refuses every row it inserts.", many.join(", "), target.relation)),
    }
}

/// Where each result column is written, by its index, as far as the top statement's target list
/// says: the name an alias gives it (`as x`), else a column's own name (the `id` of `mt.id`), else
/// the target's first word. Nothing from a `*` on, which stands for columns the list does not name.
fn column_spans(sql: &str, rewritten: &Rewritten) -> Vec<(usize, usize)> {
    let Ok(parsed) = crate::stack::parse(&rewritten.sql) else { return Vec::new() };
    let Some(top) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) else { return Vec::new() };
    let Some((items, _, _)) = jsonshape::top_context(top) else { return Vec::new() };
    let targets: Vec<&pg_query::protobuf::ResTarget> = items.iter().map_while(|i| match i.node.as_ref() {
        Some(NodeEnum::ResTarget(rt)) => Some(&**rt),
        _ => None,
    }).collect();
    let starts: Vec<usize> = targets.iter().map(|rt| rewritten.to_original(rt.location.max(0) as usize).min(sql.len())).collect();
    let mut spans = Vec::new();
    for (k, rt) in targets.iter().enumerate() {
        if rt.val.as_deref().is_some_and(jsonshape::is_star) {
            break;
        }
        let start = starts[k];
        let text = &sql[start..starts.get(k + 1).copied().unwrap_or(sql.len()).max(start)];
        let found = if !rt.name.is_empty() {
            word_after_as(text, &rt.name)
        } else {
            match rt.val.as_deref().and_then(|v| v.node.as_ref()) {
                Some(NodeEnum::ColumnRef(c)) => jsonshape::last_string(&c.fields).and_then(|name| word(text, &name)),
                _ => None,
            }
        };
        spans.push(match found {
            Some((at, length)) => (start + at, length),
            None => (start, sql[start..].bytes().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'"')).count()),
        });
    }
    spans
}

/// The first place `name` stands in `text` as a whole word, unquoted in any case or quoted as it is.
fn word(text: &str, name: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let quoted = format!("\"{}\"", name.replace('"', "\"\""));
    let mut i = 0;
    while i < bytes.len() {
        if text[i..].starts_with(&quoted) {
            return Some((i, quoted.len()));
        }
        let boundary = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_' || bytes[i - 1] == b'"');
        if boundary && text.len() >= i + name.len() && text.is_char_boundary(i + name.len()) && text[i..i + name.len()].eq_ignore_ascii_case(name) {
            let next = bytes.get(i + name.len());
            if !next.is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'"') {
                return Some((i, name.len()));
            }
        }
        i += text[i..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

/// The alias `name` where `as` gives it: the first `as` in `text` followed by the name.
fn word_after_as(text: &str, name: &str) -> Option<(usize, usize)> {
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find("as") {
        let i = from + at;
        let before = i == 0 || !(lower.as_bytes()[i - 1].is_ascii_alphanumeric() || lower.as_bytes()[i - 1] == b'_');
        let rest = &text[i + 2..];
        let gap = rest.len() - rest.trim_start().len();
        if before && gap > 0 {
            if let Some((0, length)) = word(&rest[gap..], name) {
                return Some((i + 2 + gap, length));
            }
        }
        from = i + 2;
    }
    None
}

/// Whether the statement is a COPY that streams through the connection (`FROM STDIN`, `TO
/// STDOUT`), which no driver's query call carries.
fn copies_through_the_connection(sql: &str) -> bool {
    let Ok(parsed) = crate::stack::parse(sql) else { return false };
    matches!(parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()), Some(NodeEnum::CopyStmt(c)) if c.filename.is_empty())
}

/// A failed describe as a diagnostic on the author's text: the server's message, at the
/// characters its position points at.
fn db_diagnostic(sql: &str, rewritten: &Rewritten, e: &DbError) -> Diagnostic {
    let at = e.position.filter(|p| *p > 0).map(|p| rewritten.sql.char_indices().nth(p - 1).map_or(sql.len(), |(b, _)| rewritten.to_original(b).min(sql.len())));
    let start = match at {
        // A position at the end of the text, or in the whitespace after it, is the end of input
        // ("syntax error at end of input"): the statement's last word, where it stops short.
        // Postgres's whitespace is ASCII alone (a no-break space is part of a name to it), which
        // also keeps the start after the separator on a character's boundary.
        Some(at) if sql[at..].trim_matches(is_space).is_empty() => {
            let end = sql.trim_end_matches(is_space).len();
            sql[..end].rfind(|c: char| is_space(c) || matches!(c, '(' | ',' | ';')).map_or(0, |i| i + 1).min(end.saturating_sub(1))
        }
        Some(at) => at,
        None => 0,
    };
    let length = sql[start..].bytes().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'"' | b'$' | b':' | b'@')).count();
    let message = match &e.code {
        Some(code) => format!("{} ({code})", e.message),
        None => e.message.clone(),
    };
    diag(message, start, length, 6)
}

/// Whitespace as Postgres's scanner reads it: space, tab, newline, carriage return, form feed and
/// vertical tab.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c' | '\x0b')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Answer, RawColumn, RawDescribe};

    /// A server that describes every statement as one text column per target and answers every
    /// catalog query with no rows: enough for the replay to run to its end.
    /// How many result columns the top statement's own target list names (the leftmost arm's
    /// for a set operation), which is what the fake describer reports.
    fn target_count(sql: &str) -> usize {
        fn arm(s: &pg_query::protobuf::SelectStmt) -> usize {
            match s.larg.as_deref() {
                Some(left) => arm(left),
                None => s.target_list.len(),
            }
        }
        let parsed = crate::stack::parse(sql).expect("the probe parses");
        match parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) {
            Some(pg_query::NodeEnum::SelectStmt(s)) => arm(s).max(1),
            // DDL describes with no columns, as Postgres does.
            Some(pg_query::NodeEnum::CreateStmt(_) | pg_query::NodeEnum::AlterTableStmt(_) | pg_query::NodeEnum::CopyStmt(_)) => 0,
            _ => 1,
        }
    }

    fn replay(sql: &str) -> Analysis {
        let mut answers: HashMap<String, Answer> = HashMap::new();
        for _ in 0..64 {
            let host = Host::new(&answers);
            if let Some(analysis) = analyze(sql, &host) {
                return analysis;
            }
            let needs = host.needs();
            assert!(!needs.is_empty(), "no answer and no need");
            for need in needs {
                match need {
                    crate::api::Need::Describe { id, sql } => {
                        let columns = (0..target_count(&sql)).map(|i| RawColumn { name: format!("c{i}"), oid: 25, table: 0, attnum: 0 }).collect();
                        answers.insert(id, Answer::Describe { describe: RawDescribe { params: vec![], columns } });
                    }
                    crate::api::Need::Query { id, .. } => {
                        answers.insert(id, Answer::Rows { rows: vec![] });
                    }
                }
            }
        }
        panic!("the replay did not converge");
    }

    #[test]
    fn a_probe_numbers_its_placeholders_from_one_and_pins_their_types() {
        // A probe that reads only the statement's `$2` and `$3` would leave `$1` untyped.
        let none = HashMap::new();
        let host = Host::new(&none);
        let engine = Engine::new(&host);
        *engine.casts.borrow_mut() = vec![Some("int4".into()), Some("text".into()), None];
        assert_eq!(engine.pin("WITH a AS (SELECT $2 AS x) SELECT x FROM a WHERE x = $3"), "WITH a AS (SELECT ($1::text) AS x) SELECT x FROM a WHERE x = $2");
        assert_eq!(engine.pin("SELECT $1 + $1"), "SELECT ($1::int4) + ($1::int4)");
    }

    #[test]
    fn each_parameter_appears_once() {
        // Every later use is an error at its place, before the server is asked anything, and the
        // parameters stay listed, each once, untyped.
        let sql = "select id, name from users where name like :q or email like :q or nick like :q";
        let none = HashMap::new();
        let host = Host::new(&none);
        let a = analyze(sql, &host).expect("refused without a round trip");
        assert!(host.needs().is_empty());
        let second = sql.find(":q or nick").unwrap();
        assert_eq!(a.diagnostics.iter().map(|d| (d.code, d.start, d.length)).collect::<Vec<_>>(), [(3, second, 2), (3, sql.rfind(":q").unwrap(), 2)]);
        assert!(a.diagnostics[0].message.starts_with("The parameter :q appears more than once; use each parameter once"));
        assert_eq!(a.params.iter().map(|p| (p.name.as_str(), &p.ty)).collect::<Vec<_>>(), [("q", &Type::Unknown)]);
        let a = analyze("select $1::int + $1", &host).unwrap();
        assert_eq!(a.diagnostics.iter().map(|d| (d.code, d.start)).collect::<Vec<_>>(), [(3, 17)]);
    }

    #[test]
    fn a_column_is_placed_at_the_name_it_takes() {
        let spans = |sql: &str| {
            let lexed = lex::lex(sql, Dialect::Postgres).unwrap();
            column_spans(sql, &lex::rewrite(sql, &lexed, Target::Dollar)).into_iter().map(|(start, length)| sql[start..start + length].to_string()).collect::<Vec<_>>()
        };
        // A column's own name, an alias, a quoted alias, and a function's first word.
        assert_eq!(spans("select m.id, mt.id, count(*) from member m join member_team mt on true"), ["id", "id", "count"]);
        assert_eq!(spans("select a AS X, b as \"X\", c y from t"), ["X", "\"X\"", "c"]);
        // Through placeholders, whose rewritten text is shorter or longer than the author's.
        assert_eq!(spans("select :first::int as x, :second::text as x"), ["x", "x"]);
        // Nothing from a star on, and nothing for a set operation, which names no one target list.
        assert_eq!(spans("select id, *, id from t"), ["id"]);
        assert!(spans("select 1 as x union select 2").is_empty());
        // A RETURNING list.
        assert_eq!(spans("insert into t (a) values (1) returning id, a as id"), ["id", "id"]);
        // The fix reads the value from a WITH.
        let a = replay("with input as (select concat('%', :q::text, '%') as pattern) select 1 as one from input where input.pattern like input.pattern");
        assert!(a.diagnostics.iter().all(|d| d.code != 3), "{:?}", a.diagnostics);
    }

    #[test]
    fn a_parameter_as_a_whole_sort_or_group_key_is_an_error_at_its_place() {
        let keys = |sql: &str| replay(sql).diagnostics.iter().filter(|d| d.code == 17).map(|d| (d.start, d.length, d.message.clone())).collect::<Vec<_>>();
        let at = |sql: &str, written: &str| sql.find(written).unwrap();
        // Bare, cast either way, collated, named or positional: each sorts every row by one value.
        for sql in ["select id from users order by :sort", "select id from users order by :sort::text desc", "select id from users order by cast(:sort as text)", "select id from users order by :sort::text collate \"C\""] {
            let found = keys(sql);
            assert_eq!(found.iter().map(|(s, l, _)| (*s, *l)).collect::<Vec<_>>(), [(at(sql, ":sort"), 5)], "{sql}");
            assert_eq!(found[0].2, "order by :sort sorts every row by the same value, since a parameter is a value and not a column's name; choose the column with case :sort when 'name' then name end");
        }
        let sql = "select id from users order by $1::text";
        assert_eq!(keys(sql).iter().map(|(s, l, _)| (*s, *l)).collect::<Vec<_>>(), [(at(sql, "$1"), 2)]);
        assert!(keys(sql)[0].2.starts_with("order by $1 sorts every row"));
        // GROUP BY, alone and beside another key.
        let found = keys("select count(*) as n from users group by :g::text");
        assert_eq!(found[0].2, "group by :g puts every row in one group, since a parameter is a value and not a column's name; choose the column with case :g when 'name' then name end");
        assert!(keys("select count(*) as n from users group by name, :g::text")[0].2.starts_with("group by :g groups every row by the same value"));
        // A window's keys, DISTINCT ON, and an aggregate's ORDER BY.
        let sql = "select row_number() over (partition by :p::text order by :o::text) as n from users";
        assert_eq!(keys(sql).iter().map(|(s, _, m)| (*s, m.split(',').next().unwrap().to_string())).collect::<Vec<_>>(), [(at(sql, ":p"), "partition by :p puts every row in one partition".to_string()), (at(sql, ":o"), "order by :o sorts every row by the same value".to_string())]);
        assert_eq!(keys("select row_number() over w as n from users window w as (partition by :p::text)").len(), 1);
        assert!(keys("select distinct on (:d::text) id from users")[0].2.starts_with("distinct on (:d) keeps a single row"));
        assert!(keys("select distinct on (:d::text, name) id from users")[0].2.starts_with("distinct on :d compares every row by the same value"));
        assert_eq!(keys("select string_agg(name, ',' order by :o::text) as names from users").len(), 1);
        // In a subquery, a WITH query and a set operation's ORDER BY, as the parse finds them.
        let found = |sql: &str| constant_keys(sql).into_iter().map(|(at, clause, _)| (at, clause)).collect::<Vec<_>>();
        let sql = "select id from (select id from users order by $1::text limit 3) t";
        assert_eq!(found(sql), [(at(sql, "$1"), KeyClause::OrderBy)]);
        let sql = "with t as (select name from users group by $1::text, name) select name from t";
        assert_eq!(found(sql), [(at(sql, "$1"), KeyClause::GroupBy)]);
        let sql = "select id from users where id in (select id from users order by $1::text limit 3)";
        assert_eq!(found(sql), [(at(sql, "$1"), KeyClause::OrderBy)]);
        assert_eq!(keys("select id from users union select id from users order by :s::text").len(), 1);
        // A key that holds a parameter chooses a column with it; a position, a LIMIT, a plain
        // DISTINCT and a grouping set's key are no such key.
        for sql in [
            "select id, name from users order by case :sort::text when 'name' then name end",
            "select id from users order by case when :desc::bool then id end desc, id",
            "select id from users order by 1 limit :n::int",
            "select id, :tag::text as tag from users order by 2",
            "select distinct name from users",
            "select name, count(*) as n from users group by rollup (name)",
            "select id from users order by lower(:x::text) || name",
        ] {
            assert_eq!(keys(sql), [], "{sql}");
        }
        // The rest of the statement is typed all the same.
        let a = replay("select id from users order by :sort::text");
        assert_eq!(a.columns.len(), 1);
    }

    #[test]
    fn several_statements_say_how_to_leave_a_migration_out_and_one_command_is_typed() {
        // Several statements are refused before the server is asked, which would refuse to
        // describe them.
        let sql = "create table users (id integer primary key);\ncreate table posts (id integer primary key);\n";
        let none = HashMap::new();
        let host = Host::new(&none);
        let a = analyze(sql, &host).expect("refused without a round trip");
        assert!(host.needs().is_empty());
        assert_eq!(a.diagnostics.iter().map(|d| (d.code, d.start, d.length)).collect::<Vec<_>>(), [(10, 0, sql.trim_end().len())]);
        assert_eq!(a.diagnostics[0].message, "A typed .sql file holds one statement and this one holds several, so if it is a migration or a schema file, leave it out of the type check with \"exclude\" in tsconfig.json, as in \"exclude\": [\"db/**\"].");
        // One statement of another kind is a command, typed from the describe: no rows here.
        let a = replay("/* the tags */ CREATE TABLE tags (id integer primary key)");
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!((a.kind.as_str(), a.cardinality.as_str(), a.columns.len(), a.params.len()), ("create", "none", 0, 0));
        // A placeholder the server does not bind in it is an error at the placeholder.
        let sql = "alter table tags alter id set default :id";
        let a = replay(sql);
        assert_eq!(a.diagnostics.iter().map(|d| (d.code, &sql[d.start..d.start + d.length])).collect::<Vec<_>>(), [(1, ":id")]);
        assert_eq!(a.diagnostics[0].message, "Postgres binds no parameter in this ALTER statement, so :id cannot be passed: write its value into the statement");
        // A COPY through the connection is refused without a round trip; one from a file is a command.
        let host = Host::new(&none);
        let a = analyze("copy tags from stdin", &host).expect("refused without a round trip");
        assert!(host.needs().is_empty());
        assert_eq!(a.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [10]);
        assert!(replay("copy tags from '/tmp/tags.csv'").diagnostics.is_empty());
    }

    #[test]
    fn a_subquery_whose_one_target_is_a_tables_whole_row_returns_its_stored_rows() {
        let targets = |sql: &str| sublink_rows(sql).map(|r| r.targets);
        let row = |table: &str, name: &str, array: bool| Some((table.to_string(), name.to_string(), array, false));
        let agg = |table: &str, name: &str| Some((table.to_string(), name.to_string(), true, true));
        assert_eq!(
            targets("SELECT p.id, array(SELECT c FROM child c WHERE c.pid = p.id), (SELECT c FROM child c LIMIT 1), (SELECT child FROM child) FROM parent p"),
            Some(vec![None, row("child", "c", true), row("child", "c", false), row("child", "child", false)])
        );
        // A join of tables, and the alias hides the table's own name.
        assert_eq!(targets("SELECT (SELECT c FROM parent p JOIN child c ON c.pid = p.id LIMIT 1)"), Some(vec![row("child", "c", false)]));
        assert_eq!(targets("SELECT (SELECT child FROM child c LIMIT 1)"), Some(vec![None]));
        // What is no whole row of a table of the subquery's own FROM.
        assert_eq!(targets("SELECT (SELECT row(1, 'a')::child FROM child LIMIT 1), (SELECT c.* FROM child c LIMIT 1), (SELECT x.c FROM child x LIMIT 1)"), Some(vec![None, None, None]));
        assert_eq!(targets("SELECT (SELECT c FROM child c UNION ALL SELECT c FROM child c LIMIT 1)"), Some(vec![None]));
        // Where a name could mean what the text does not show, nothing is read.
        assert_eq!(targets("SELECT (SELECT c FROM child c, generate_series(1, 2) g LIMIT 1)"), None);
        assert_eq!(targets("SELECT (SELECT c FROM child c LIMIT 1) FROM (SELECT 1) s"), None);
        assert_eq!(targets("SELECT (SELECT c FROM public.child c LIMIT 1)"), None);
        assert_eq!(targets("WITH child AS (SELECT 1 AS x) SELECT (SELECT c FROM child c LIMIT 1)"), None);
        assert_eq!(targets("SELECT 1 UNION SELECT 2"), None);
        // An `array_agg` of a whole row of the SELECT's own FROM, bare or starred, ordered,
        // filtered or over a window, beside one of a column, of a built row and of another FROM's.
        assert_eq!(
            targets("SELECT array_agg(c), array_agg(c.* ORDER BY c.id), array_agg(c) FILTER (WHERE c.id > 0), array_agg(c) OVER (), array_agg(c.id), array_agg(row(c.id)), pg_catalog.array_agg(p) FROM parent p JOIN child c ON c.pid = p.id GROUP BY p.id"),
            Some(vec![agg("child", "c"), agg("child", "c"), agg("child", "c"), agg("child", "c"), None, None, agg("parent", "p")])
        );
        // A scalar subquery's own, and one with an empty array literal for no rows; not under
        // `ARRAY(…)`, and not with a default that may hold other rows.
        assert_eq!(
            targets("SELECT (SELECT array_agg(c) FROM child c), (SELECT coalesce(array_agg(c.* ORDER BY c.id), '{}') FROM child c WHERE c.pid = p.id), (SELECT coalesce(array_agg(c), array[]::child[]) FROM child c), coalesce(array_agg(p), '{}'::parent[]) FROM parent p GROUP BY p.id"),
            Some(vec![agg("child", "c"), agg("child", "c"), agg("child", "c"), agg("parent", "p")])
        );
        assert_eq!(
            targets("SELECT array(SELECT array_agg(c) FROM child c), (SELECT coalesce(array_agg(c), array[row(1)::child]) FROM child c), (SELECT coalesce('{}', array_agg(c)) FROM child c), (SELECT array_agg(c) || '{}' FROM child c) FROM parent p"),
            Some(vec![None, None, None, None])
        );
        assert_eq!(targets("SELECT array_agg(child) FROM child c"), Some(vec![None]));
        assert_eq!(targets("SELECT array_agg(c) FROM (SELECT 1) c"), None);
    }

    #[test]
    fn a_check_holds_a_json_column_to_the_schema_it_prints() {
        // As PostgreSQL 18 printed the CHECKs of wiki/json-schema-checks/postgres/catalog.sql.
        let schema = r#"{"type": "object", "properties": {"name": {"type": "string", "pattern": "^\\d+$", "description": "it's café é"}}, "required": ["name"]}"#;
        let printed = r#"'{"type": "object", "properties": {"name": {"type": "string", "pattern": "^\\d+$", "description": "it''s café é"}}, "required": ["name"]}'::json"#;
        let call = |f: &str| vec![(f.to_string(), vec![CheckArg::Schema(schema.into()), CheckArg::Column])];
        assert_eq!(check_documents(&format!("jsonb_matches_schema({printed}, data)"), "data"), call("jsonb_matches_schema"));
        assert_eq!(check_documents(&format!("jsonb_matches_schema({printed}, VALUE)"), "doc"), call("jsonb_matches_schema"));
        assert_eq!(check_documents(&format!("json_matches_schema({printed}, data)"), "data"), call("json_matches_schema"));
        assert_eq!(check_documents(&format!("jsonb_matches_schema({printed}, data::jsonb)"), "data"), call("jsonb_matches_schema"));
        assert_eq!(check_documents(&format!("data IS NULL OR jsonb_matches_schema({printed}, data)"), "data"), call("jsonb_matches_schema"));
        assert_eq!(check_documents(&format!("jsonb_matches_schema({printed}, data) AND jsonb_typeof(data) = 'object'::text"), "data"), call("jsonb_matches_schema"));
        assert_eq!(check_documents(&format!("jsv.jsonb_matches_schema({printed}, data)"), "data"), call("jsonb_matches_schema"));
        let jsonb = check_documents(r#"jsonb_matches_schema('{"type": "string"}'::jsonb::json, data)"#, "data");
        assert_eq!(jsonb, vec![("jsonb_matches_schema".to_string(), vec![CheckArg::Schema(r#"{"type": "string"}"#.into()), CheckArg::Column])]);
        // Another column's value, or an OR that lets more than NULL through, holds nothing.
        let two = check_documents(r#"jsonb_matches_schema('{"type": "string"}'::json, a) AND jsonb_matches_schema('{"type": "integer"}'::json, b)"#, "b");
        assert_eq!(two.iter().map(|(_, args)| args[1].clone()).collect::<Vec<_>>(), [CheckArg::Other, CheckArg::Column]);
        assert_eq!(check_documents(&format!("data = '1'::jsonb OR jsonb_matches_schema({printed}, data)"), "data"), vec![]);
    }

    #[test]
    fn a_check_keeps_a_string_column_to_the_list_it_prints() {
        let list = |items: &[&str]| Some(items.iter().map(|s| s.to_string()).collect::<Vec<String>>());
        assert_eq!(check_values("status = ANY (ARRAY['active'::text, 'banned'::text])", "status"), list(&["active", "banned"]));
        assert_eq!(check_values("v::text = ANY (ARRAY['x'::character varying, 'y'::character varying]::text[])", "v"), list(&["x", "y"]));
        assert_eq!(check_values("VALUE = ANY (ARRAY['ok'::text, 'sad'::text])", "mood"), list(&["ok", "sad"]));
        assert_eq!(check_values("status IS NULL OR status = ANY (ARRAY['a'::text, 'b'::text])", "status"), list(&["a", "b"]));
        assert_eq!(check_values("status = ANY (ARRAY['a'::text, 'b'::text]) AND length(status) > 0", "status"), list(&["a", "b"]));
        assert_eq!(check_values("status = 'active'::text", "status"), list(&["active"]));
        // A NULL in the list lets any value pass (`'z' = NULL` is NULL, and a CHECK passes on NULL).
        assert_eq!(check_values("status = ANY (ARRAY['a'::text, NULL::text])", "status"), None);
        for other in [
            "status <> ALL (ARRAY['a'::text, 'b'::text])",
            "status = ANY (ARRAY['a'::text]) OR status = 'b'::text",
            "lower(status) = ANY (ARRAY['a'::text])",
            "other = ANY (ARRAY['a'::text])",
            "status OPERATOR(public.=) 'a'::text",
        ] {
            assert_eq!(check_values(other, "status"), None, "{other}");
        }
        assert_eq!(with_values(Type::Scalar { name: "text".into() }, Some(&["a".to_string()])), Type::Enum { name: "text".into(), labels: vec!["a".into()] });
        assert_eq!(with_values(Type::Scalar { name: "bpchar".into() }, Some(&["a".to_string()])), Type::Scalar { name: "bpchar".into() });
    }

    #[test]
    fn a_conflict_target_no_unique_index_matches_is_refused() {
        let target = |sql: &str| conflict_targets(sql).into_iter().next();
        let name = target("INSERT INTO bar (id, name) VALUES ($1, $2) ON CONFLICT (name) DO NOTHING").expect("a target");
        assert_eq!(name, ConflictTarget { relation: "bar".into(), columns: vec!["name".into()], filtered: false, start: 55 });
        let pair = target("INSERT INTO app.bar AS b (id, name) VALUES ($1, $2) ON CONFLICT (name, id) WHERE id > 0 DO UPDATE SET name = excluded.name").expect("a target");
        assert_eq!((pair.relation.as_str(), pair.columns.len(), pair.filtered), ("app.bar", 2, true));
        // One inside a WITH query is read too.
        assert_eq!(conflict_targets("WITH i AS (INSERT INTO bar (id) VALUES ($1) ON CONFLICT (id) DO NOTHING RETURNING id) SELECT id FROM i").len(), 1);
        // No target, a named constraint and an expression are not lists of columns.
        for sql in [
            "INSERT INTO bar (id) VALUES ($1)",
            "INSERT INTO bar (id) VALUES ($1) ON CONFLICT DO NOTHING",
            "INSERT INTO bar (id) VALUES ($1) ON CONFLICT ON CONSTRAINT bar_pkey DO NOTHING",
            "INSERT INTO bar (id, name) VALUES ($1, $2) ON CONFLICT (lower(name)) DO NOTHING",
            "INSERT INTO bar (id, name) VALUES ($1, $2) ON CONFLICT (id, (name || 'x')) DO NOTHING",
        ] {
            assert_eq!(target(sql), None, "{sql}");
        }

        // `bar` has a primary key on id, a unique index on (a, b), one on lower(name), and a
        // partial one on email; `empty` has no unique index; `v` is a view and has no row.
        let rows = vec![
            vec![json!("bar"), json!(["id"]), Value::Null],
            vec![json!("bar"), json!(["b", "a"]), Value::Null],
            vec![json!("bar"), json!([null]), Value::Null],
            vec![json!("bar"), json!(["email"]), json!("active")],
            vec![json!("empty"), Value::Null, Value::Null],
        ];
        let check = |relation: &str, columns: &[&str], filtered: bool, catalog: &Catalog| {
            unmatched_conflict(&ConflictTarget { relation: relation.into(), columns: columns.iter().map(|c| c.to_string()).collect(), filtered, start: 0 }, &rows, catalog)
        };
        let none = Catalog::default();
        assert_eq!(check("bar", &["id"], false, &none), None);
        assert_eq!(check("bar", &["a", "b"], false, &none), None, "the key's columns in any order");
        assert_eq!(check("bar", &["id", "id"], false, &none), None, "a column named twice is one");
        assert!(check("bar", &["name"], false, &none).is_some_and(|m| m.contains("none has exactly (name)")));
        assert!(check("bar", &["a"], false, &none).is_some(), "half of a key");
        assert!(check("bar", &["a", "b", "id"], false, &none).is_some(), "more than a key");
        assert!(check("empty", &["id"], false, &none).is_some());
        // A partial index is inferred only by a target with a WHERE that implies its predicate.
        assert!(check("bar", &["email"], false, &none).is_some_and(|m| m.contains("is partial (WHERE active)")));
        assert_eq!(check("bar", &["email"], true, &none), None);
        // A relation the rows do not list, and one whose INSERT a rule rewrites.
        assert_eq!(check("v", &["id"], false, &none), None);
        let mut ruled = Catalog::default();
        ruled.rewritten.insert("empty".into(), cardinality::TRIGGER_ROW_INSERT);
        assert_eq!(check("empty", &["id"], false, &ruled), None);
    }

    #[test]
    fn returning_reads_a_row_the_statement_may_not_have() {
        let columns = |relation: &str| (relation != "unknown").then(|| vec!["id".to_string(), "name".to_string()]);
        // Every column traced to a table's column, which is what the answer is about.
        let missing = |sql: &str, count: usize| missing_rows(sql, &vec![true; count], &columns);
        assert_eq!(missing("INSERT INTO users (id, name) VALUES ($1, $2) RETURNING old.name, new.name, name", 3), vec![true, false, false]);
        assert_eq!(missing("DELETE FROM users WHERE id = $1 RETURNING old.name, new.name, new.*", 4), vec![false, true, true, true]);
        assert_eq!(missing("UPDATE users SET name = $1 WHERE id = $2 RETURNING old.name, new.name", 2), vec![false, false]);
        assert_eq!(missing("INSERT INTO users VALUES ($1, $2) ON CONFLICT (id) DO UPDATE SET name = excluded.name RETURNING old.*", 2), vec![true, true]);
        // By the names the clause gives the rows, which hide the default ones.
        assert_eq!(missing("INSERT INTO users (id, name) VALUES ($1, $2) RETURNING WITH (OLD AS o, NEW AS n) o.name, n.name, o.*, n.*", 6), vec![true, false, true, true, false, false]);
        assert_eq!(missing("DELETE FROM users WHERE id = $1 RETURNING WITH (NEW AS n) n.name, old.name, *", 4), vec![true, false, false, false]);
        // A relation of the name is read first: the table `old`, the target under the alias `new`.
        assert_eq!(missing("INSERT INTO old (id, name) VALUES ($1, $2) RETURNING old.name, new.name", 2), vec![false, false]);
        assert_eq!(missing("DELETE FROM users AS new WHERE id = $1 RETURNING new.name, old.name", 2), vec![false, false]);
        assert_eq!(missing("INSERT INTO old (id, name) VALUES ($1, $2) RETURNING WITH (OLD AS was) was.name, old.name", 2), vec![true, false]);
        // A field of the whole row and a cast that changes nothing keep their provenance.
        assert_eq!(missing("INSERT INTO users (id, name) VALUES ($1, $2) RETURNING (old).name, old.name::text, (new).name", 3), vec![true, true, false]);
        // A bare name is a column before it is a row.
        assert_eq!(missing("INSERT INTO users (id, name) VALUES ($1, $2) RETURNING WITH (OLD AS name) name, name.id", 2), vec![false, true]);
        // An expression carries no provenance: the expression rules settle it.
        assert_eq!(missing_rows("INSERT INTO users (id, name) VALUES ($1, $2) RETURNING old.name, old.name || '!', old.id IS NULL", &[true, false, false], &columns), vec![true, false, false]);
        // A MERGE that inserts has no old row for that action; one that never deletes always has a new one.
        let merge = "MERGE INTO users u USING (SELECT 1 AS id) s ON u.id = s.id WHEN MATCHED THEN UPDATE SET name = 'x' WHEN NOT MATCHED THEN INSERT (id, name) VALUES (s.id, 'n') RETURNING old.name, new.name, s.id";
        assert_eq!(missing(merge, 3), vec![true, false, false]);
        // One that acts where the source matches nothing returns rows with no source row.
        let by_source = "MERGE INTO users u USING src s ON u.id = s.id WHEN MATCHED THEN UPDATE SET name = s.name WHEN NOT MATCHED BY SOURCE THEN UPDATE SET name = 'gone' RETURNING s.name, u.name, name, label, old.name";
        assert_eq!(missing(by_source, 5), vec![true, false, false, true, false]);
        // A data-modifying WITH query that reads a missing row may feed any column.
        assert_eq!(missing("WITH i AS (INSERT INTO users VALUES ($1, 'n') RETURNING old.name AS n) SELECT n, 1 FROM i", 2), vec![true, true]);
        assert_eq!(missing("WITH i AS (INSERT INTO users VALUES ($1, 'n') RETURNING WITH (OLD AS o) o.name AS n) SELECT n, 1 FROM i", 2), vec![true, true]);
        assert_eq!(missing("WITH i AS (INSERT INTO users VALUES ($1, 'n') RETURNING new.name AS n) SELECT n, 1 FROM i", 2), vec![false, false]);
        // A `*` over a relation of unknown columns, or over more than the target, hides which column is which.
        assert_eq!(missing("INSERT INTO unknown VALUES ($1, $2) RETURNING new.*, old.name", 3), vec![true, true, true]);
        assert_eq!(missing("DELETE FROM users USING other WHERE other.id = users.id RETURNING *, new.name", 5), vec![true, true, true, true, true]);
        assert_eq!(missing("SELECT 1, 2", 2), vec![false, false]);
    }

    /// A server with one table, `users (id int primary key, name text not null)`, that describes
    /// every statement as `name` columns of it, one per name in `names`, and types `$1` and `$2`
    /// as text and int.
    fn replay_users(sql: &str, names: &[&str]) -> Analysis {
        let mut answers: HashMap<String, Answer> = HashMap::new();
        for _ in 0..64 {
            let host = Host::new(&answers);
            if let Some(analysis) = analyze(sql, &host) {
                return analysis;
            }
            for need in host.needs() {
                match need {
                    crate::api::Need::Describe { id, .. } => {
                        let columns = names.iter().map(|name| RawColumn { name: name.to_string(), oid: 25, table: 16400, attnum: 2 }).collect();
                        answers.insert(id, Answer::Describe { describe: RawDescribe { params: vec![25, 23], columns } });
                    }
                    crate::api::Need::Query { id, sql, .. } => {
                        let rows = match sql.as_str() {
                            RELATION_COLUMNS_QUERY => vec![
                                vec![json!(16400), json!("users"), json!(1), json!(true), Value::Null, json!("id"), json!(true)],
                                vec![json!(16400), json!("users"), json!(2), json!(true), Value::Null, json!("name"), json!(true)],
                            ],
                            cardinality::UNIQUE_KEYS_QUERY => vec![vec![json!("users"), json!(["id"]), json!([null]), Value::Null]],
                            cardinality::COLUMNS_QUERY => vec![vec![json!("users"), json!(["id", "name"]), json!([23, 25]), json!([true, true]), json!(false)]],
                            _ => vec![],
                        };
                        answers.insert(id, Answer::Rows { rows });
                    }
                }
            }
        }
        panic!("the replay did not converge");
    }

    #[test]
    fn a_parameter_may_be_null_cast_to_nullable_or_written_bare_to_a_column_that_holds_null() {
        // A server with one table, `users (id int primary key, name text not null, email text,
        // score numeric, n int, doc jsonb, meta json not null, nick text check (nick is not null),
        // mail required_email, code text check (code <> '' and code is not null), tag tag_text)`,
        // where the domain `required_email` is declared `not null` and the domain `tag_text`
        // checks `VALUE IS NOT NULL`, describing every statement with no result column and every
        // parameter as text.
        let columns = [("id", true), ("name", true), ("email", false), ("score", false), ("n", false), ("doc", false), ("meta", true), ("nick", false), ("mail", false), ("code", false), ("tag", false)];
        let refusals = |i: usize, name: &str| -> Vec<Value> {
            let (not_null, checks, domain): (bool, Vec<&str>, Vec<&str>) = match name {
                "id" | "name" | "meta" | "mail" => (true, vec![], vec![]),
                "nick" => (false, vec!["nick IS NOT NULL"], vec![]),
                "code" => (false, vec!["code <> ''::text AND code IS NOT NULL"], vec![]),
                "tag" => (false, vec![], vec!["VALUE IS NOT NULL"]),
                "score" => (false, vec!["score > 0::numeric"], vec![]),
                _ => (false, vec![], vec![]),
            };
            vec![json!("users"), json!(i + 1), json!(name), json!(not_null), json!(checks), json!(domain)]
        };
        let analyzed = |sql: &str| {
            let mut answers: HashMap<String, Answer> = HashMap::new();
            for _ in 0..64 {
                let host = Host::new(&answers);
                if let Some(a) = analyze(sql, &host) {
                    return a;
                }
                for need in host.needs() {
                    match need {
                        crate::api::Need::Describe { id, sql } => {
                            let count = lex::lex(&sql, Dialect::Postgres).map(|l| l.placeholders.len()).unwrap_or(0);
                            answers.insert(id, Answer::Describe { describe: RawDescribe { params: vec![25; count], columns: vec![] } });
                        }
                        crate::api::Need::Query { id, sql, .. } => {
                            let rows = match sql.as_str() {
                                RELATION_COLUMNS_QUERY => columns.iter().enumerate().map(|(i, (name, not_null))| vec![json!(16400), json!("users"), json!(i + 1), json!(not_null), Value::Null, json!(name), json!(not_null)]).collect(),
                                REFUSES_NULL_QUERY => columns.iter().enumerate().map(|(i, (name, _))| refusals(i, name)).collect(),
                                cardinality::UNIQUE_KEYS_QUERY => vec![vec![json!("users"), json!(["id"]), json!([null]), Value::Null], vec![json!("users"), json!(["email"]), json!([null]), Value::Null]],
                                _ => vec![],
                            };
                            answers.insert(id, Answer::Rows { rows });
                        }
                    }
                }
            }
            panic!("the replay did not converge");
        };
        let nullable = |sql: &str| {
            let a = analyzed(sql);
            assert!(a.diagnostics.is_empty(), "{sql}: {:?}", a.diagnostics);
            a.params.iter().map(|p| format!("{}{}", p.name, if p.nullable { "?" } else { "" })).collect::<Vec<_>>().join(" ")
        };
        // Written whole and bare to a column that may hold null, in an INSERT's rows, by name and
        // by place, the SELECT that feeds an INSERT, an UPDATE's SET and an upsert's, a row
        // value's elements included: as nullable as the column.
        assert_eq!(nullable("INSERT INTO users (name, email) VALUES (:name, :email) ON CONFLICT (email) DO UPDATE SET name = excluded.name RETURNING id"), "name email?");
        assert_eq!(nullable("INSERT INTO users (name, email, score, n, doc, meta) VALUES (:name, :email, :score, :n, :doc, :meta), (:a, :b, :c, :d, :e, :f)"), "name email? score? n? doc? meta a b? c? d? e? f");
        assert_eq!(nullable("INSERT INTO users VALUES (:id, :name, :email)"), "id name email?");
        assert_eq!(nullable("INSERT INTO users (name, email) SELECT :name, :email"), "name email?");
        assert_eq!(nullable("UPDATE users SET email = :email, name = :name, n = :n WHERE id = :id"), "email? name n? id");
        assert_eq!(nullable("UPDATE users SET (name, email) = (:name, :email) WHERE id = :id"), "name email? id");
        assert_eq!(nullable("INSERT INTO users (name) VALUES ('a') ON CONFLICT (id) DO UPDATE SET email = :email, name = :name"), "email? name");
        // A column that refuses null otherwise: a CHECK that tests it IS NOT NULL (among others),
        // a domain declared NOT NULL, a domain that checks VALUE IS NOT NULL. A CHECK a null
        // passes, `score > 0`, leaves the column nullable.
        assert_eq!(nullable("INSERT INTO users (name, nick, mail, code, tag) VALUES (:name, :nick, :mail, :code, :tag)"), "name nick mail code tag");
        assert_eq!(nullable("UPDATE users SET nick = :nick, mail = :mail, code = :code, tag = :tag, score = :score WHERE id = :id"), "nick mail code tag score? id");
        // A cast parameter is its cast's type, never null, however it is written; so is an expression.
        assert_eq!(nullable("INSERT INTO users (name, email) VALUES (:name::text, :email::text) ON CONFLICT (email) DO UPDATE SET name = excluded.name RETURNING id"), "name email");
        assert_eq!(nullable("INSERT INTO users (name, email, score, n, doc, meta) VALUES (:name::text, :email::text, :score::numeric, :n::int, :doc::jsonb, :meta::json)"), "name email score n doc meta");
        assert_eq!(nullable("UPDATE users SET email = CAST(:email AS text), n = :n::int, doc = :doc::json::jsonb, score = lower(:s)::numeric WHERE id = :id::int"), "email n doc s id");
        assert_eq!(nullable("UPDATE users SET (name, email) = (:name::text, :email::text) WHERE id = :id::int"), "name email id");
        assert_eq!(nullable("INSERT INTO users (name, email) SELECT :name::text, :email::text"), "name email");
        // Tested for NULL, or compared with IS NOT DISTINCT FROM: a value all the same.
        assert_eq!(nullable("SELECT id FROM users WHERE (:doc::json IS NULL OR n = :x::int) AND email IS NOT DISTINCT FROM :e AND :k::int IS NOT NULL"), "doc x e k");
        // Cast to `nullable(…)`, whatever the type, through the WITH query that selects it once
        // too: the caller may pass null. An argument of `coalesce`, first or not, is a value.
        assert_eq!(nullable("UPDATE users SET n = coalesce(:n::int, n), doc = :doc::nullable(jsonb), email = :email::nullable(text), name = coalesce(name, :e) WHERE id = :id::int"), "n doc? email? e id");
        assert_eq!(nullable("UPDATE users SET name = coalesce(CAST(:name AS text), name), meta = coalesce(:meta, meta) WHERE id = :id"), "name meta id");
        assert_eq!(nullable("WITH input AS (SELECT :q::nullable(timestamp with time zone) AS q) SELECT id FROM users, input WHERE input.q IS NULL OR name = input.q::text"), "q?");
        assert_eq!(nullable("WITH input AS (SELECT :q::text AS q) SELECT id FROM users, input WHERE name = coalesce(input.q, name)"), "q");
        // Cast to `nullable(…)` and written whole to a column that refuses null, by its NOT NULL,
        // a CHECK or a domain: an error at the parameter, whose null the database would refuse.
        let sql = "INSERT INTO users (name, nick, mail, email) VALUES (:name::nullable(text), :nick::nullable(text), :mail::nullable(text), :email::nullable(text))";
        let refused = analyzed(sql);
        let at = |d: &crate::api::Diagnostic| (sql[d.start..d.start + d.length].to_string(), d.message.clone(), d.code);
        let said = |p: &str, column: &str| format!("The parameter {p} is cast to nullable(…), but it is written to users.{column}, which refuses null: cast it to its type alone.");
        assert_eq!(refused.diagnostics.iter().map(at).collect::<Vec<_>>(), [
            (":name::nullable(text)".to_string(), said(":name", "name"), crate::structured::CODE),
            (":nick::nullable(text)".to_string(), said(":nick", "nick"), crate::structured::CODE),
            (":mail::nullable(text)".to_string(), said(":mail", "mail"), crate::structured::CODE),
        ]);
        // Written whole by the one SELECT that feeds the INSERT, as a VALUES item is.
        let sql = "INSERT INTO users (name, email) SELECT :name::nullable(text), :email::nullable(text)";
        let refused = analyzed(sql);
        let at = |d: &crate::api::Diagnostic| (sql[d.start..d.start + d.length].to_string(), d.message.clone(), d.code);
        assert_eq!(refused.diagnostics.iter().map(at).collect::<Vec<_>>(), [(":name::nullable(text)".to_string(), said(":name", "name"), crate::structured::CODE)]);
    }

    #[test]
    fn returning_with_names_for_the_old_and_the_new_row_is_typed_as_old_and_new_are() {
        let nullable = |a: &Analysis| a.columns.iter().map(|c| c.nullable).collect::<Vec<_>>();
        // PostgreSQL 18's clause parses, so the write by key is at most one row, and both rows of
        // an UPDATE hold the table's NOT NULL.
        let update = replay_users("UPDATE users SET name = :name WHERE id = :id RETURNING WITH (OLD AS o, NEW AS n) o.name AS old_name, n.name AS new_name", &["old_name", "new_name"]);
        assert_eq!((update.kind.as_str(), update.cardinality.as_str(), update.cardinality_reason.as_deref()), ("update", "maybeOne", None));
        assert!(update.diagnostics.is_empty(), "{:?}", update.diagnostics);
        assert_eq!(nullable(&update), [false, false]);
        assert_eq!(update.params.iter().map(|p| (p.name.as_str(), p.type_name.as_str())).collect::<Vec<_>>(), [("name", "text"), ("id", "int4")]);
        assert_eq!(nullable(&replay_users("UPDATE users SET name = :name WHERE id = :id RETURNING old.name AS old_name, new.name AS new_name", &["old_name", "new_name"])), [false, false]);
        // An INSERT has no old row and a DELETE no new one, by the clause's names as by the default ones.
        let insert = replay_users("INSERT INTO users (name, id) VALUES (:name, :id) RETURNING WITH (OLD AS o, NEW AS n) o.name AS old_name, n.name AS new_name", &["old_name", "new_name"]);
        assert_eq!((insert.cardinality.as_str(), nullable(&insert)), ("one", vec![true, false]));
        let delete = replay_users("DELETE FROM users WHERE name = :name AND id = :id RETURNING WITH (NEW AS after) old.name AS old_name, after.name AS new_name, name", &["old_name", "new_name", "name"]);
        assert_eq!((delete.cardinality.as_str(), nullable(&delete)), ("maybeOne", vec![false, true, false]));
    }

    #[test]
    fn a_statement_at_the_depth_bound_is_analyzed_whole_and_one_past_it_falls_back_to_the_describe() {
        // From a thread with a small stack, as a tokio worker or a Postgres backend may call: the
        // parse, the walks of the expression rules, the row-count proof and the JSON shapes all run
        // on the engine's own stack.
        let small = |sql: String| std::thread::scope(|s| std::thread::Builder::new().stack_size(256 << 10).spawn_scoped(s, move || replay_users(&sql, &["x"])).unwrap().join().unwrap());
        for &(shape, n) in crate::stack::BOUNDS {
            // Keyed where the shape leaves the WHERE free, so the proof has something to prove
            // (`replay_users` types `$1` as text and `$2` as int).
            let sql = crate::stack::nested(shape, n);
            let keyed = !sql.contains(" where ");
            let at = small(if keyed { format!("{sql} where name = :name and id = :id") } else { sql });
            // The key proves at most one row, but for `json_build_object`, which this replay's
            // catalog does not hold.
            let rows = if keyed && shape != "json" { "maybeOne" } else { "many" };
            assert_eq!((at.kind.as_str(), at.cardinality.as_str()), ("select", rows), "{shape} at {n}: {:?}", at.cardinality_reason);
            let past = small(crate::stack::nested(shape, n + 1));
            assert_eq!((past.kind.as_str(), past.cardinality.as_str()), ("", "many"), "{shape} at {}", n + 1);
            assert_eq!(past.cardinality_reason.as_deref(), Some("the statement did not parse locally"));
            assert!(past.columns.iter().all(|c| c.nullable), "{shape} at {}", n + 1);
            assert!(past.diagnostics.is_empty(), "{:?}", past.diagnostics);
        }
    }

    #[test]
    fn the_trust_query_says_whether_the_server_casts_a_jsonb_null() {
        let read = |rows: Vec<Vec<Value>>| {
            let mut answers: HashMap<String, Answer> = HashMap::new();
            let host = Host::new(&answers);
            assert!(trust(&host).is_none());
            let needs = host.needs();
            let [crate::api::Need::Query { id, sql, .. }] = needs.as_slice() else { panic!("one query, not {needs:?}") };
            assert_eq!(sql, TRUST_QUERY);
            answers.insert(id.clone(), Answer::Rows { rows });
            trust(&Host::new(&answers)).unwrap()
        };
        // A server before 18 answers with one row more, a NULL beside the name: its cast of a
        // JSON null raises an error, where 18 returns an SQL NULL.
        let before = read(vec![vec![json!(false), json!("lower")], vec![json!(true), json!("int4")], vec![Value::Null, json!("jsonb casts raise")]]);
        assert!(before.jsonb_casts_raise);
        assert!(before.shadowed.contains("lower") && before.cast_targets.as_ref().unwrap().contains("int4"));
        assert!(!before.shadowed.contains("jsonb casts raise") && !before.cast_targets.as_ref().unwrap().contains("jsonb casts raise"));
        let later = read(vec![vec![json!(false), json!("lower")], vec![json!(true), json!("int4")]]);
        assert!(!later.jsonb_casts_raise);
        // A NULL beside any other name says nothing.
        assert!(!read(vec![vec![Value::Null, json!("lower")]]).jsonb_casts_raise);
    }

    #[test]
    fn a_position_the_parse_reports_is_an_offset_in_the_text_it_read() {
        // libpg_query 18 leaves comments out of what it parses; a location is still an offset in
        // the text with them, which is where a diagnostic is put.
        let sql = "/* upsert */ INSERT INTO bar (id, name) -- the row\n VALUES ($1, $2) /* é */ ON CONFLICT (name) DO NOTHING";
        let target = conflict_targets(sql).into_iter().next().expect("a target");
        assert_eq!(&sql[target.start..target.start + 6], "(name)");
        let compare = "SELECT 1 /* é */ FROM bar -- by name\n WHERE name = $1 AND /* id */ id > $2";
        let at = |needle: &str| compare[..compare.find(needle).unwrap()].chars().count() + 1;
        assert_eq!(analyze::params_at_position(compare, at("= $1")), [1]);
        assert_eq!(analyze::params_at_position(compare, at("$2")), [2]);
        assert_eq!(analyze::params_at_position(compare, at("> $2")), [2]);
    }

    #[test]
    fn a_probe_that_reads_as_its_own_leaf_ends() {
        // `u` is a whole-row leaf, and in its leaf probe `SELECT u AS a0 FROM users u` it is one
        // again: without a guard the probe would probe itself without end.
        let analysis = replay("SELECT u AS whole FROM users u");
        assert_eq!(analysis.columns.len(), 1);
    }

    #[test]
    fn a_round_names_the_same_needs_however_the_parse_orders_its_tables() {
        // libpg_query lists a statement's tables in hash order, which differs between parses; a
        // need that carried that order would be new every round and the replay would not settle.
        let sql = "SELECT a.x FROM alpha a JOIN beta b ON b.id = a.id JOIN gamma g ON g.id = b.id JOIN delta d ON d.id = g.id";
        let answers: HashMap<String, Answer> = HashMap::new();
        let round = || {
            let host = Host::new(&answers);
            assert!(analyze(sql, &host).is_none());
            host.needs()
        };
        let first = round();
        for _ in 0..16 {
            assert_eq!(round(), first);
        }
    }

    #[test]
    fn a_recursive_with_query_is_settled_from_its_own_arms() {
        // The UNION ALL sits inside the WITH query, so the statement's provenance is not to be
        // trusted, but the column read from the query takes what the query's own probe proves:
        // `1` is never null, and `n + 1` is not once `n < 5` holds.
        let seq = replay("WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 5) SELECT n FROM seq");
        assert_eq!(seq.columns.iter().map(|c| c.nullable).collect::<Vec<_>>(), vec![false]);
        // `b` starts NULL and `a` takes it over: the fixed point proves neither.
        let pair = replay("WITH RECURSIVE pair(a, b) AS (SELECT 1, NULL::int UNION ALL SELECT b, b FROM pair WHERE a < 3) SELECT a, b FROM pair");
        assert_eq!(pair.columns.iter().map(|c| c.nullable).collect::<Vec<_>>(), vec![true, true]);
    }

    /// A server with the docs' tables, `users (id int primary key, name text not null)`, `posts (id
    /// int primary key, user_id int, title text not null)` and `events (id int primary key, kind
    /// text not null check (kind in ('click', 'key')), note text)`, that describes a statement by
    /// its text: `describe` gives each result column's name, type and provenance.
    fn replay_docs(sql: &str, describe: &dyn Fn(&str) -> Vec<RawColumn>) -> Analysis {
        let mut answers: HashMap<String, Answer> = HashMap::new();
        let composite = |oid: u32, name: &str, rel: u32, fields: &[(&str, u32, bool)]| {
            vec![
                json!(oid), json!(name), json!("c"), json!(0), json!(0), Value::Null, json!(name),
                json!(fields.iter().map(|f| f.0).collect::<Vec<_>>()), json!(fields.iter().map(|f| f.1).collect::<Vec<_>>()), json!(fields.iter().map(|f| f.2).collect::<Vec<_>>()),
                json!(rel), json!((1..=fields.len() as i32).collect::<Vec<_>>()),
            ]
        };
        let tables: [(u32, u32, &str, Vec<(&str, u32, bool)>); 3] = [
            (16400, 90001, "users", vec![("id", 23, true), ("name", 25, true)]),
            (16410, 90002, "posts", vec![("id", 23, true), ("user_id", 23, false), ("title", 25, true)]),
            (16420, 90003, "events", vec![("id", 23, true), ("kind", 25, true), ("note", 25, false)]),
        ];
        for _ in 0..64 {
            let host = Host::new(&answers);
            if let Some(analysis) = analyze(sql, &host) {
                assert!(analysis.diagnostics.is_empty(), "{sql}: {:?}", analysis.diagnostics);
                return analysis;
            }
            for need in host.needs() {
                match need {
                    crate::api::Need::Describe { id, sql } => {
                        answers.insert(id, Answer::Describe { describe: RawDescribe { params: vec![], columns: describe(&sql) } });
                    }
                    crate::api::Need::Query { id, sql, .. } => {
                        let rows = match sql.as_str() {
                            RELATION_COLUMNS_QUERY => tables
                                .iter()
                                .flat_map(|(rel, _, name, fields)| fields.iter().enumerate().map(move |(i, f)| vec![json!(rel), json!(name), json!(i + 1), json!(f.2), Value::Null, json!(f.0), json!(f.2)]))
                                .collect(),
                            TYPE_QUERY | RELATION_TYPES_QUERY => tables.iter().map(|(rel, oid, name, fields)| composite(*oid, name, *rel, fields)).collect(),
                            CHECKS_QUERY => vec![vec![json!(16420), json!(2), json!("kind"), json!("kind = ANY (ARRAY['click'::text, 'key'::text])"), Value::Null, Value::Null, Value::Null]],
                            _ => vec![],
                        };
                        answers.insert(id, Answer::Rows { rows });
                    }
                }
            }
        }
        panic!("the replay did not converge");
    }

    #[test]
    fn a_star_of_a_reading_is_its_tables_columns_by_that_reading() {
        let width = |name: &str| (name == "users").then_some(3);
        let users = |nulled: bool| Some(("users".to_string(), nulled));
        assert_eq!(aliased_columns("select a.*, b.name from users a left join users b on b.id > a.id", 4, &width), [users(false), users(false), users(false), users(true)]);
        assert_eq!(aliased_columns("select a.name, b.* from users a left join users b on b.id > a.id", 4, &width), [users(false), users(true), users(true), users(true)]);
        assert_eq!(aliased_columns("select a.*, b.* from users a full join users b on b.id = a.id", 6, &width), [users(true), users(true), users(true), users(true), users(true), users(true)]);
        // A bare star, a star of an item whose width is not known, and a count that does not add up say nothing.
        assert!(aliased_columns("select * from users a left join users b on b.id > a.id", 6, &width).is_empty());
        assert!(aliased_columns("select p.*, a.name from users a left join posts p on p.user_id = a.id", 4, &width).is_empty());
        assert!(aliased_columns("select a.* from users a left join users b on b.id > a.id", 4, &width).is_empty());
        // `(x).*` counts as many columns as `x` has fields, and a table named with its schema may
        // be another `users`: neither may offset the count of the other.
        assert!(aliased_columns("select a.*, b.id, (b.addr).* from other.users a left join users b on true", 5, &|name: &str| (name == "users").then_some(2)).is_empty());
        assert!(aliased_columns("select a.*, b.name from other.users a left join users b on true", 4, &width).is_empty());
        assert!(aliased_columns("select a.name, (b.addr).* from users a left join users b on true", 3, &width).is_empty());
    }

    #[test]
    fn a_tables_whole_row_is_a_row_unless_an_outer_join_nulls_it() {
        let column = |name: &str, oid: u32, table: u32, attnum: i32| RawColumn { name: name.into(), oid, table, attnum };
        // The probes select the whole row as `a0`; the statement itself has one JSON column.
        let describe = |sql: &str| -> Vec<RawColumn> {
            if sql.contains(" e AS a0") {
                vec![column("a0", 90003, 16420, 0)]
            } else if sql.contains(" p AS a0") {
                vec![column("a0", 90002, 16410, 0)]
            } else if sql.contains(" u AS a0") {
                vec![column("a0", 90001, 16400, 0)]
            } else if sql.starts_with("SELECT e,") || sql == "SELECT e FROM events e" {
                vec![column("e", 90003, 16420, 0)]
            } else {
                vec![column("rows", 3802, 0, 0)]
            }
        };
        let items = |sql: &str| -> (Type, bool) {
            let a = replay_docs(sql, &describe);
            match &a.columns[0].ty {
                Type::List { of, nullable_items } => ((**of).clone(), *nullable_items),
                other => panic!("{sql}: {other:?}"),
            }
        };
        // A row of a table no outer join nulls is never NULL, and keeps its CHECK's union.
        let (row, nullable) = items("SELECT jsonb_agg(e ORDER BY e.id) AS rows FROM events e");
        assert!(!nullable);
        let Type::Composite { fields, .. } = row else { panic!("{row:?}") };
        assert_eq!(fields[1].ty, Type::Enum { name: "text".into(), labels: vec!["click".into(), "key".into()] });
        assert!(!fields[1].nullable && fields[2].nullable);
        // The preserved side of an outer join is a row; the nulled side is NULL where nothing matched,
        // unless a strict test of the WHERE rejects those rows.
        assert!(!items("SELECT jsonb_agg(u) AS rows FROM users u LEFT JOIN posts p ON p.user_id = u.id").1);
        assert!(items("SELECT jsonb_agg(p) AS rows FROM users u LEFT JOIN posts p ON p.user_id = u.id").1);
        assert!(!items("SELECT jsonb_agg(p) AS rows FROM users u LEFT JOIN posts p ON p.user_id = u.id WHERE p.title > ''").1);
        assert!(items("SELECT jsonb_agg(p) AS rows FROM users u LEFT JOIN posts p ON p.user_id = u.id WHERE p.title > '' OR u.id = 1").1);
        assert!(items("SELECT jsonb_agg(u) AS rows FROM users u FULL JOIN posts p ON p.user_id = u.id").1);
        // A bare read of the row keeps the union too.
        let a = replay_docs("SELECT e FROM events e", &describe);
        assert!(!a.columns[0].nullable);
        let Type::Composite { fields, .. } = &a.columns[0].ty else { panic!("{:?}", a.columns[0].ty) };
        assert_eq!(fields[1].ty, Type::Enum { name: "text".into(), labels: vec!["click".into(), "key".into()] });
    }

    #[test]
    fn a_derived_table_or_a_with_query_over_a_schema_qualified_table_is_settled() {
        // A server with `users (id int not null, name text not null)` and `app.accounts (id int
        // not null, note text)`. The catalog's rows by name never match a schema-qualified name,
        // so each probe of a query over `app.accounts` asks for that relation again, while the
        // statement around it settles, and its columns' `NOT NULL` is read by oid.
        const USERS: u32 = 16400;
        const ACCOUNTS: u32 = 16600;
        let not_null: HashSet<(u32, i32)> = HashSet::from([(USERS, 1), (USERS, 2), (ACCOUNTS, 1)]);
        let column = |name: &str, oid: u32, table: u32, attnum: i32| RawColumn { name: name.into(), oid, table, attnum };
        let analyzed = |sql: &str| {
            let mut answers: HashMap<String, Answer> = HashMap::new();
            for _ in 0..64 {
                let host = Host::new(&answers);
                if let Some(a) = analyze(sql, &host) {
                    assert!(a.diagnostics.is_empty(), "{sql}: {:?}", a.diagnostics);
                    return a.columns.into_iter().map(|c| (c.name, c.ty, c.nullable)).collect::<Vec<_>>();
                }
                for need in host.needs() {
                    match need {
                        crate::api::Need::Describe { id, sql } => {
                            let columns = if sql.contains("app.accounts") {
                                vec![column("id", 23, ACCOUNTS, 1), column("note", 25, ACCOUNTS, 2)]
                            } else {
                                vec![column("id", 23, USERS, 1), column("name", 25, USERS, 2)]
                            };
                            answers.insert(id, Answer::Describe { describe: RawDescribe { params: vec![], columns } });
                        }
                        crate::api::Need::Query { id, sql, params, .. } => {
                            let rows = match sql.as_str() {
                                RELATION_COLUMNS_QUERY => {
                                    let names: Vec<String> = serde_json::from_value(params[0].clone()).unwrap();
                                    if names.iter().any(|n| n == "users") {
                                        ["id", "name"].iter().enumerate().map(|(i, c)| vec![json!(USERS), json!("users"), json!(i + 1), json!(true), Value::Null, json!(c), json!(true)]).collect()
                                    } else {
                                        vec![]
                                    }
                                }
                                ATTNOTNULL_QUERY => {
                                    let rels: Vec<u32> = serde_json::from_value(params[0].clone()).unwrap();
                                    let atts: Vec<i32> = serde_json::from_value(params[1].clone()).unwrap();
                                    rels.iter().zip(&atts).filter(|(r, a)| not_null.contains(&(**r, **a))).map(|(r, a)| vec![json!(r), json!(a)]).collect()
                                }
                                _ => vec![],
                            };
                            answers.insert(id, Answer::Rows { rows });
                        }
                    }
                }
            }
            panic!("the replay did not converge");
        };
        let int = |name: &str, nullable: bool| (name.to_string(), Type::Scalar { name: "int4".into() }, nullable);
        let text = |name: &str, nullable: bool| (name.to_string(), Type::Scalar { name: "text".into() }, nullable);
        assert_eq!(analyzed("select s.id, s.note from (select id, note from app.accounts) s"), [int("id", false), text("note", true)]);
        assert_eq!(analyzed("with a as (select id, note from app.accounts) select id, note from a"), [int("id", false), text("note", true)]);
        assert_eq!(analyzed("select s.id, s.name from (select id, name from users) s"), [int("id", false), text("name", false)]);
        assert_eq!(analyzed("with u as (select id, name from users) select id, name from u"), [int("id", false), text("name", false)]);
    }

    #[test]
    fn own_provenance_lines_up_with_the_columns() {
        let own = |sql: &str, count: usize| own_provenance(sql, analyze::analyze(sql).ok().as_ref(), count);
        let with = "WITH a AS (SELECT p.title AS t, p.title AS t2 FROM posts p UNION ALL SELECT u.email, u.email FROM users u), e AS (SELECT)";
        // A bare column of a table of the statement's own FROM is its own beside a set operation.
        assert_eq!(own(&format!("{with} SELECT a.t, u.name FROM a, users u"), 2), vec![false, true]);
        // One `*` among as many targets as columns is one column wide, and among more it is none.
        assert_eq!(own(&format!("{with}, b AS (SELECT 1 AS n) SELECT b.*, u.name FROM b, users u"), 2), vec![false, true]);
        assert_eq!(own(&format!("{with} SELECT e.*, u.name FROM e, users u"), 1), vec![false]);
        // Two may be two columns and none, so no position says which column a target is.
        assert_eq!(own(&format!("{with} SELECT a.*, u.name, e.* FROM a, users u, e"), 3), vec![false; 3]);
    }

    #[test]
    fn an_error_at_the_end_of_input_is_on_the_last_word() {
        for sql in ["select id from member where\n", "select id from member where", "select id from t where id = :id and\n\n"] {
            let lexed = lex::lex(sql, Dialect::Postgres).unwrap();
            let rewritten = lex::rewrite(sql, &lexed, Target::Dollar);
            let last = sql.trim_end().rsplit(' ').next().unwrap();
            for position in [rewritten.sql.trim_end().chars().count() + 1, rewritten.sql.chars().count() + 1] {
                let e = DbError { message: "syntax error at end of input".into(), code: Some("42601".into()), position: Some(position) };
                let d = db_diagnostic(sql, &rewritten, &e);
                assert_eq!(&sql[d.start..d.start + d.length], last, "{sql:?} at {position}");
            }
        }
        // A no-break space is part of a name to Postgres, and the start never lands inside one.
        let sql = "select 1 +\u{a0}-";
        let lexed = lex::lex(sql, Dialect::Postgres).unwrap();
        let rewritten = lex::rewrite(sql, &lexed, Target::Dollar);
        let d = db_diagnostic(sql, &rewritten, &DbError { message: "syntax error at end of input".into(), code: Some("42601".into()), position: Some(sql.chars().count() + 1) });
        assert_eq!(&sql[d.start..d.start + d.length], "+");
        // A position inside the text stays where it is.
        let sql = "select idd from t";
        let lexed = lex::lex(sql, Dialect::Postgres).unwrap();
        let rewritten = lex::rewrite(sql, &lexed, Target::Dollar);
        let d = db_diagnostic(sql, &rewritten, &DbError { message: "column \"idd\" does not exist".into(), code: None, position: Some(8) });
        assert_eq!(&sql[d.start..d.start + d.length], "idd");
    }

    #[test]
    fn a_column_read_as_it_is_carries_its_comment() {
        // A server with a table `users (id int, age int, email text)` and a view `adults` of
        // `SELECT id, age AS years FROM users`, with `COMMENT ON COLUMN` on `users.id`,
        // `users.age` and `adults.id`. The describe names each column's origin as Postgres does:
        // the view's own column for a read through the view, nothing for an expression.
        const USERS: u32 = 16400;
        const ADULTS: u32 = 16500;
        let definition = "SELECT id, age AS years FROM users";
        let column = |name: &str, oid: u32, table: u32, attnum: i32| RawColumn { name: name.into(), oid, table, attnum };
        let describe = |sql: &str| -> RawDescribe {
            let count = lex::lex(sql, Dialect::Postgres).map(|l| l.placeholders.len()).unwrap_or(0);
            let columns = match sql {
                "SELECT id, age, age AS years, lower(email), count(*) OVER () FROM users" => {
                    vec![column("id", 23, USERS, 1), column("age", 23, USERS, 2), column("years", 23, USERS, 2), column("lower", 25, 0, 0), column("count", 20, 0, 0)]
                }
                "SELECT id, years FROM adults" => vec![column("id", 23, ADULTS, 1), column("years", 23, ADULTS, 2)],
                s if s == definition => vec![column("id", 23, USERS, 1), column("years", 23, USERS, 2)],
                "SELECT age FROM users UNION ALL SELECT age FROM users" => vec![column("age", 23, USERS, 2)],
                "WITH a AS (SELECT id FROM users UNION ALL SELECT age FROM users) SELECT a.id, u.id AS uid FROM a, users u" => vec![column("id", 23, USERS, 1), column("uid", 23, USERS, 1)],
                "UPDATE users SET age = $1, email = $2 WHERE id = $3 RETURNING age" => vec![column("age", 23, USERS, 2)],
                _ => vec![],
            };
            RawDescribe { params: vec![23; count], columns }
        };
        let relation = |oid: u32, name: &str, columns: &[&str], view: Option<&str>| -> Vec<Vec<Value>> {
            columns.iter().enumerate().map(|(i, c)| vec![json!(oid), json!(name), json!(i + 1), json!(false), if i == 0 { json!(view) } else { Value::Null }, json!(c), json!(false)]).collect()
        };
        let comments: HashMap<(u32, i32), &str> = HashMap::from([((USERS, 1), "The user's id"), ((USERS, 2), "Age (in years)"), ((ADULTS, 1), "An adult's id\nfrom users")]);
        let analyzed = |sql: &str| {
            let mut answers: HashMap<String, Answer> = HashMap::new();
            for _ in 0..64 {
                let host = Host::new(&answers);
                if let Some(a) = analyze(sql, &host) {
                    assert!(a.diagnostics.is_empty(), "{sql}: {:?}", a.diagnostics);
                    return a;
                }
                for need in host.needs() {
                    match need {
                        crate::api::Need::Describe { id, sql } => {
                            answers.insert(id, Answer::Describe { describe: describe(&sql) });
                        }
                        crate::api::Need::Query { id, sql, params, .. } => {
                            let rows = match sql.as_str() {
                                RELATION_COLUMNS_QUERY => {
                                    let names: Vec<String> = serde_json::from_value(params[0].clone()).unwrap();
                                    let mut rows = Vec::new();
                                    if names.iter().any(|n| n == "users") {
                                        rows.extend(relation(USERS, "users", &["id", "age", "email"], None));
                                    }
                                    if names.iter().any(|n| n == "adults") {
                                        rows.extend(relation(ADULTS, "adults", &["id", "years"], Some(definition)));
                                    }
                                    rows
                                }
                                COMMENTS_QUERY => {
                                    let rels: Vec<u32> = serde_json::from_value(params[0].clone()).unwrap();
                                    let atts: Vec<i32> = serde_json::from_value(params[1].clone()).unwrap();
                                    rels.iter().zip(&atts).filter_map(|(r, a)| comments.get(&(*r, *a)).map(|c| vec![json!(r), json!(a), json!(c)])).collect()
                                }
                                _ => vec![],
                            };
                            answers.insert(id, Answer::Rows { rows });
                        }
                    }
                }
            }
            panic!("the replay did not converge");
        };
        let columns = |sql: &str| analyzed(sql).columns.into_iter().map(|c| c.comment).collect::<Vec<_>>();
        let some = |c: &str| Some(c.to_string());
        // A column keeps its comment under an alias; an expression over it has none.
        assert_eq!(columns("SELECT id, age, age AS years, lower(email), count(*) OVER () FROM users"), [some("The user's id"), some("Age (in years)"), some("Age (in years)"), None, None]);
        // Through a view: the view's column's own comment, else the one of the column it reads.
        assert_eq!(columns("SELECT id, years FROM adults"), [some("An adult's id\nfrom users"), some("Age (in years)")]);
        // A set operation's provenance names its leftmost arm only.
        assert_eq!(columns("SELECT age FROM users UNION ALL SELECT age FROM users"), [None]);
        // Even where another column reads the column that provenance names.
        assert_eq!(columns("WITH a AS (SELECT id FROM users UNION ALL SELECT age FROM users) SELECT a.id, u.id AS uid FROM a, users u"), [None, some("The user's id")]);
        // A parameter written to a column carries the column's comment; one compared with a column
        // is typed by the server, which names no column.
        let update = analyzed("UPDATE users SET age = :age, email = :email WHERE id = :id RETURNING age");
        assert_eq!(update.columns.iter().map(|c| c.comment.clone()).collect::<Vec<_>>(), [some("Age (in years)")]);
        assert_eq!(update.params.iter().map(|p| (p.name.as_str(), p.comment.clone())).collect::<Vec<_>>(), [("age", some("Age (in years)")), ("email", None), ("id", None)]);
    }
}
