//! Expression columns, read off the SQL that computes them.
//!
//! The describe tier settles a plain column from its provenance: the table and attribute the
//! engine reports, the catalog's `NOT NULL`, the join analysis. An expression column has no
//! provenance. Postgres reports table oid 0 for `count(*)`, so there is no `attnotnull` to read,
//! and SQLite reports no declared type for it at all. The expression is in the statement,
//! though, and a small set of SQL constructs carries its own nullability and type:
//!
//! - `count(…)` is never NULL, over no rows and under `GROUP BY` alike. Every other aggregate
//!   is NULL over no rows and proves nothing.
//! - A literal other than `NULL` is never NULL.
//! - A cast, a `||`, a comparison, `AND`/`OR`/`NOT`, and (on Postgres) arithmetic are non-null
//!   when every operand is. SQLite turns a division by zero and a NaN into NULL, so it gets no
//!   arithmetic at all.
//! - `coalesce(a, b, …)` is non-null when any argument is; a `CASE` when every branch and an
//!   `ELSE` are; `EXISTS`, `IS [NOT] NULL`, `IS [NOT] TRUE` and `IS [NOT] DISTINCT FROM` never
//!   are NULL.
//! - A scalar subquery is non-null only when it is an ungrouped aggregate, which returns exactly
//!   one row, and its one column is non-null. The aggregate has to be the subquery's own: one
//!   that reads only columns of a query around it belongs to that query ([`crate::levels`]).
//! - A column reference inside an expression is a leaf. Its answer is the provenance one: the
//!   describer selects the leaves over the statement's own `FROM` as a probe and describes that,
//!   so the catalog and the outer-join degradation apply to it exactly as to a plain column.
//! - A set operation's arms are statements of their own. The describer describes each, so an arm
//!   that selects plain columns keeps their provenance, and a column is non-null when it is in
//!   every arm of a `UNION`, in either arm of an `INTERSECT`, and in the left arm of an `EXCEPT`.
//!
//! Everything else proves nothing and stays nullable, because a column typed non-null that can be
//! NULL crashes a client and a column left nullable only costs it a check. On Postgres a rule
//! also stands down when the database could resolve its operator, function, or cast to something
//! a user or an extension created ([`Trust`]).
//!
//! SQLite additionally gets a type for an expression column from the same rules: `count` is
//! `int8` as on Postgres, a literal is its kind, a cast is its target, `||` is text, a comparison
//! is a boolean, and `reylo_html` is text. Postgres already describes every type, so there the
//! rules only ever settle nullability.
//!
//! SQLite has no parse tree of its own to offer, so both dialects are read with libpg_query, the
//! parser `jsonshape` already reads SQLite statements with. What it cannot parse, and what parses
//! as something the rules do not recognise, proves nothing.
//!
//! This module is pure: `plan` reads the parse tree, `resolve` and `combine` turn the probes'
//! describes into answers. The describer owns the round trips.

use crate::dialect::Dialect;
use crate::types::DescribedColumn;
use crate::jsonshape::{as_select, last_string, select_text, single_target, top_context};
use crate::levels::{Columns, Level, Withs};
use crate::pgtypes::oid;
use crate::returning::Rows;
use pg_query::protobuf::{self, node::Node as NodeEnum, Node, ParseResult, RawStmt, SelectStmt};
use std::collections::{HashMap, HashSet};

/// The operators whose Postgres built-ins return a value for every pair of non-null operands (or
/// raise: a division by zero is an error, never NULL). `!=` parses as `<>`. Every built-in
/// overload of each was read in `pg_operator` on PostgreSQL 18: the pattern matches (`LIKE` is
/// `~~`, `ILIKE` `~~*`, `SIMILAR TO` and the regular expressions `~`), the bit, network and
/// shift operators, the containment and overlap of arrays, ranges, geometry, JSON and text
/// search, JSON's key tests and path deletion, and the prefix `|/`, `||/`, `@` and `~`. Not `#`,
/// whose geometric overloads (`line_interpt`, `lseg_interpt`, `box_intersect`) are NULL where
/// nothing intersects, nor `->`, `->>` and `#>>`, NULL for a missing key.
pub const POSTGRES_OPERATORS: &[&str] = &[
    "+", "-", "*", "/", "%", "||", "=", "<>", "<", ">", "<=", ">=",
    "~~", "!~~", "~~*", "!~~*", "~", "!~", "~*", "!~*",
    "&", "|", "<<", ">>", "^", "&&", "@>", "<@", "?", "?|", "?&", "#-", "|/", "||/", "@",
];
/// The operators a prefix form of is in [`POSTGRES_OPERATORS`]: the signs, the roots, absolute
/// value and bitwise not.
const POSTGRES_PREFIX_OPERATORS: &[&str] = &["-", "+", "|/", "||/", "@", "~"];
/// The pattern matches, whose result is a truth value: `LIKE`, `ILIKE`, `SIMILAR TO` and the
/// regular expressions.
const POSTGRES_MATCH_OPERATORS: &[&str] = &["~~", "!~~", "~~*", "!~~*", "~", "!~", "~*", "!~*"];
/// SQLite's: concatenation and comparison only. Its arithmetic answers NULL for a division by
/// zero and for a NaN (`1e999 - 1e999`), so no arithmetic operator is total there.
const SQLITE_OPERATORS: &[&str] = &["||", "=", "==", "<>", "!=", "<", ">", "<=", ">="];
/// The functions a rule reads by name: `count`, plus the aggregates that make a subquery
/// return exactly one row.
pub const FUNCTIONS: &[&str] = &["count", "sum", "avg", "min", "max", "json_agg", "jsonb_agg"];

/// Postgres built-ins that answer NULL only for a NULL argument: a non-null input never yields
/// NULL (an invalid one raises an error instead). Nothing here answers NULL for a valid input
/// the way `nullif`, `substring` with no match or `array_length` of an empty array do, and no
/// aggregate is here: an aggregate over no rows is NULL.
///
/// The list is by name, so a name stays here only when no overload of it answers NULL for valid
/// arguments. The ones that do are by overload in [`POSTGRES_TOTAL_FOR`] or nowhere: `date_part`
/// and `extract` answer NULL for `'infinity'` whatever the field. The overloads were checked
/// against the routines that the PostgreSQL 18 sources let return NULL for non-null input
/// (typedpg derives that list from them, `builtin_nullability.rs`).
pub const POSTGRES_TOTAL: &[&str] = &[
    "initcap", "casefold", "length", "char_length", "character_length", "octet_length", "btrim", "ltrim", "rtrim", "md5",
    "reverse", "repeat", "replace", "left", "right", "lpad", "rpad", "translate", "ascii", "chr", "split_part", "strpos", "starts_with",
    "regexp_replace", "quote_ident", "quote_literal", "encode", "decode", "to_hex", "to_timestamp", "to_date", "uuidv7",
    "abs", "round", "floor", "ceil", "ceiling", "trunc", "sign", "sqrt", "cbrt", "exp", "ln", "log", "power", "mod",
    "date_trunc", "age", "make_date", "make_timestamp", "make_timestamptz", "nextval",
    "cardinality", "array_to_string", "array_append", "array_prepend", "array_cat", "array_remove",
    "to_json", "to_jsonb", "row_to_json", "json_typeof", "jsonb_typeof", "jsonb_strip_nulls", "jsonb_pretty",
    // What `x SIMILAR TO p` and `x LIKE p ESCAPE e` call on their pattern.
    "similar_to_escape", "like_escape",
    // A set-returning function whose rows each hold a value: there may be none, never a NULL.
    "generate_series", "generate_subscripts",
];
/// The set-returning functions among the ones the rules read, every name of the lists above with
/// a built-in overload that is `proretset` on PostgreSQL 18, and `unnest`: a target that calls one
/// may be no row or several ([`one_value`]).
const POSTGRES_SET_RETURNING: &[&str] = &["generate_series", "generate_subscripts", "unnest"];
/// Built-in types none of which is an array: the cast targets [`POSTGRES_CASTS`] names.
const POSTGRES_SCALARS: &[u32] = &[
    oid::TEXT, oid::VARCHAR, oid::BPCHAR, oid::NAME, oid::INT2, oid::INT4, oid::INT8, oid::NUMERIC, oid::FLOAT4, oid::FLOAT8, oid::BOOL,
    oid::UUID, oid::BYTEA, oid::DATE, oid::TIME, oid::TIMESTAMP, oid::TIMESTAMPTZ, oid::INTERVAL, oid::JSON, oid::JSONB,
];
/// Postgres built-ins that are [`POSTGRES_TOTAL`] only for some types of their first argument:
/// `lower` and `upper` of an empty or unbounded range are NULL, and so is `to_char` of an
/// `'infinity'` timestamp or interval, while their text and number overloads never are. A first
/// argument whose type the build cannot name proves nothing.
pub const POSTGRES_TOTAL_FOR: &[(&str, &[u32])] = &[("lower", TEXT_TYPES), ("upper", TEXT_TYPES), ("to_char", NUMBER_TYPES)];
/// The string types the text overloads take, as a leaf or a cast reports them.
const TEXT_TYPES: &[u32] = &[oid::TEXT, oid::VARCHAR, oid::BPCHAR, oid::NAME];
/// The number types `to_char`'s number overloads take.
const NUMBER_TYPES: &[u32] = &[oid::INT2, oid::INT4, oid::INT8, oid::NUMERIC, oid::FLOAT4, oid::FLOAT8];
/// The [`POSTGRES_TOTAL`] functions that return `jsonb`, which the cast rule below asks about.
const POSTGRES_JSONB: &[&str] = &["to_jsonb", "jsonb_strip_nulls"];
/// The cast targets `jsonb` has a cast function to. Since PostgreSQL 18 each answers SQL NULL
/// for a JSON null (`'null'::jsonb::int`), where 17 and earlier raised an error: on 18 a cast to
/// one of them keeps its argument's nullability only where the argument is no `jsonb` value
/// ([`Trust::jsonb_casts_raise`] says which server it is).
const POSTGRES_JSONB_NULL_CASTS: &[&str] = &["bool", "numeric", "int2", "int4", "int8", "float4", "float8"];
/// The first oid a database assigns to an object of its own (`FirstNormalObjectId`): a type from
/// there on may be a domain over any type.
const FIRST_USER_OID: u32 = 16384;
/// Postgres built-ins that never answer NULL, whatever their arguments: the clock and random
/// functions, and the constructors that turn a NULL argument into an empty string or a JSON null.
/// A `VARIADIC` call is read as nothing, since `concat(VARIADIC NULL::text[])` is NULL.
/// `uuidv4` is PostgreSQL 18's; `uuidv7` is NULL for a NULL shift, so it is in [`POSTGRES_TOTAL`],
/// and never NULL without one.
pub const POSTGRES_NEVER_NULL: &[&str] = &[
    "now", "clock_timestamp", "statement_timestamp", "transaction_timestamp", "gen_random_uuid", "uuidv4", "random",
    "concat", "quote_nullable", "num_nulls", "num_nonnulls",
    "json_build_object", "json_build_array", "jsonb_build_object", "jsonb_build_array",
];
/// Postgres built-ins that answer NULL only when their first argument is NULL: `concat_ws` skips
/// a NULL value but not a NULL separator.
pub const POSTGRES_FIRST_ARG: &[&str] = &["concat_ws"];
/// Postgres built-ins that answer NULL only when every argument is NULL.
pub const POSTGRES_ANY: &[&str] = &["greatest", "least"];
/// Built-in operators that answer NULL for a NULL operand: the comparisons, arithmetic, JSON
/// access and pattern matching. `||` is not one: `NULL::int[] || ARRAY[1]` is `{1}`.
pub const POSTGRES_STRICT_OPERATORS: &[&str] = &[
    "=", "<>", "<", ">", "<=", ">=", "+", "-", "*", "/", "%", "^", "->", "->>", "#>", "#>>", "~~", "~~*", "!~~", "!~~*", "~", "~*", "!~", "!~*",
    "&", "|", "#", "<<", ">>", "&&", "@>", "<@", "?", "?|", "?&", "#-",
];
/// Built-in functions that answer NULL for a NULL argument in every overload (`proisstrict`),
/// which is all a strict predicate asks of them, whatever they answer for other input: `lower` of
/// an empty range is NULL too, and that only removes more rows.
pub const POSTGRES_STRICT: &[&str] = &[
    "lower", "upper", "initcap", "casefold", "length", "char_length", "character_length", "octet_length", "bit_length", "btrim", "ltrim", "rtrim",
    "md5", "reverse", "repeat", "replace", "left", "right", "lpad", "rpad", "translate", "ascii", "chr", "split_part", "strpos", "position",
    "starts_with", "substr", "substring", "regexp_replace", "quote_ident", "to_hex", "to_char", "to_date", "to_timestamp",
    "abs", "round", "floor", "ceil", "ceiling", "trunc", "sign", "sqrt", "cbrt", "exp", "ln", "log", "power", "mod",
    "date_trunc", "date_part", "extract", "age",
];
/// The window functions a rule reads (see `Reader::call`).
pub const POSTGRES_WINDOW: &[&str] = &["row_number", "rank", "dense_rank", "percent_rank", "cume_dist", "ntile", "lag", "lead"];
/// Postgres aggregates that answer NULL only over no rows or over nothing but NULLs. Under a
/// plain `GROUP BY` a group is never empty, so there they are non-null on a non-null argument.
pub const POSTGRES_GROUPED: &[&str] = &["sum", "avg", "min", "max", "bit_and", "bit_or", "bool_and", "bool_or", "every", "string_agg"];
/// Postgres aggregates that answer NULL only over no rows: a group's `array_agg` is an array,
/// NULL elements and all.
pub const POSTGRES_GROUPED_NEVER_NULL: &[&str] = &["array_agg"];
/// Postgres's JSON aggregates, which answer NULL only over no rows too: a group's `json_agg` is a
/// JSON array, a NULL input written as a JSON null, and its `json_object_agg` an object, a NULL
/// value written as a JSON null (a NULL key is an error).
pub const POSTGRES_GROUPED_JSON: &[&str] = &["json_agg", "jsonb_agg", "json_object_agg", "jsonb_object_agg"];

/// Every function name a Postgres rule reads, which is what a host asks the catalog about: a
/// name a user or an extension has also defined is shadowed, and its rule proves nothing
/// ([`Trust::shadowed`]).
pub fn postgres_function_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    let typed = POSTGRES_TOTAL_FOR.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    for list in [FUNCTIONS, POSTGRES_TOTAL, &typed, POSTGRES_NEVER_NULL, POSTGRES_FIRST_ARG, POSTGRES_ANY, POSTGRES_GROUPED, POSTGRES_GROUPED_NEVER_NULL, POSTGRES_GROUPED_JSON, POSTGRES_STRICT, POSTGRES_WINDOW, POSTGRES_SET_RETURNING, crate::jsonshape::POSTGRES_FUNCTIONS] {
        for name in list {
            if !names.contains(name) {
                names.push(name);
            }
        }
    }
    names
}
/// SQLite's cast targets with a type of their own, by the name libpg_query gives them
/// (`INTEGER` parses as `int4`, `REAL` as `float4`). A cast to anything else still carries its
/// argument's nullability on SQLite, but not a type: `CAST(x AS NUMERIC)` may be either class.
const SQLITE_CASTS: &[(&str, u32)] = &[
    ("int2", oid::INT2),
    ("int4", oid::INT4),
    ("int8", oid::INT8),
    ("float4", oid::FLOAT8),
    ("float8", oid::FLOAT8),
    ("text", oid::TEXT),
    ("varchar", oid::VARCHAR),
];
/// Postgres's built-in cast targets by the name libpg_query gives them, so a cast names its type
/// for a rule that reads one ([`Expr::Guarded`]). A target outside the list proves its argument's
/// nullability as before and names no type.
const POSTGRES_CASTS: &[(&str, u32)] = &[
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
    ("bytea", oid::BYTEA),
    ("date", oid::DATE),
    ("time", oid::TIME),
    ("timestamp", oid::TIMESTAMP),
    ("timestamptz", oid::TIMESTAMPTZ),
    ("interval", oid::INTERVAL),
    ("json", oid::JSON),
    ("jsonb", oid::JSONB),
];
/// SQLite cast targets whose result is never NULL for a non-null argument, shown for `INTEGER`,
/// `REAL` and `TEXT`: `CAST('abc' AS INTEGER)` is 0, not NULL.
const SQLITE_TOTAL_CASTS: &[&str] = &["int2", "int4", "int8", "float4", "float8", "text", "varchar"];

/// What the SQL says about one expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Never NULL, of this type where the SQL names one.
    NonNull(Option<u32>),
    /// Nothing the rules recognise: nullable and untyped.
    Unknown,
    /// A column the reader has no leaf probe for: one inside a scalar subquery, whose `FROM` the
    /// probe over the outer one does not read, or any under grouping sets. Nullable and untyped,
    /// as `Unknown`; an array built from it says nothing of its elements, as a column's array.
    Unread,
    /// The inner expression's type, nullable whatever the inner proves: a `CASE` with no `ELSE`,
    /// a scalar subquery that may match no row, SQLite's `total()`.
    Nullable(Box<Expr>),
    /// Column `n` of the leaf probe: a column reference, answered by its provenance.
    Leaf(usize),
    /// Column `n` of the leaf probe, read as the whole row of this relation: non-null when the
    /// probe says it is that relation's row type (and so not a column that shares the alias's
    /// name), because a row of a table that no outer join nulls always exists.
    Row(usize, String),
    /// Non-null when every operand is: an operator, a cast, a `CASE` with an `ELSE`.
    Every(Vec<Expr>, Ty),
    /// Non-null when any operand is: `coalesce`.
    Any(Vec<Expr>, Ty),
    /// The inner expression, which proves non-null only when its type is one of these: the first
    /// argument of a function that is total for some overloads ([`POSTGRES_TOTAL_FOR`]).
    Guarded(Box<Expr>, &'static [u32]),
    /// The inner expression, which proves non-null only where it holds no `jsonb` value for
    /// certain: the argument of a cast to a number or a boolean ([`POSTGRES_JSONB_NULL_CASTS`]).
    NotJsonb(Box<Expr>),
    /// The inner expression, which cannot be NULL where it is read: a column inside a CASE arm
    /// that runs only where it is not NULL. Its type is the inner one's.
    Narrowed(Box<Expr>),
    /// An array built from these elements: `ARRAY[a, b]`, `array_agg(x)` whose elements are the
    /// values of `x`, or a literal. The array itself is never NULL when `non_null` says so; its
    /// elements are as these are ([`items_nullable`]).
    Array { non_null: bool, items: Vec<Expr> },
}

/// The type an [`Expr::Every`] or [`Expr::Any`] carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ty {
    /// This type, whatever the operands are: `||` is text, a comparison a boolean.
    Is(u32),
    /// The operands' type when they all agree on one: `coalesce`, `CASE`.
    Shared,
    /// `text` when every operand is a string type: Postgres's `||` over strings. Over anything
    /// else (an array, `jsonb`, a string beside a number) the build names no type.
    Concat,
    /// Nothing the build can name.
    Unknown,
}

/// The two arms of a set operation at the top of a statement, as statements of their own.
#[derive(Debug, Clone, PartialEq)]
pub struct SetOp {
    pub kind: SetKind,
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SetKind {
    Union,
    Intersect,
    Except,
}

/// What one statement's expression columns need.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    /// The result columns before the first `*` of the target list, in order: `None` where the
    /// build claims nothing, which is every plain column reference, since its provenance already
    /// answers.
    pub head: Vec<Option<Expr>>,
    /// The result columns after the last `*`, in order. Empty unless the target list has one.
    pub tail: Vec<Option<Expr>>,
    /// Whether a `*` sits between the two, so the result has columns the plan cannot count.
    pub star: bool,
    /// `SELECT <the leaves> FROM <the statement's FROM>`, when an expression reads a column.
    pub leaves: Option<String>,
    /// A set operation at the top of the statement; nothing else in the plan is set then.
    pub set_op: Option<SetOp>,
}

/// One column's answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fact {
    pub non_null: bool,
    pub type_oid: Option<u32>,
}

/// The names a rule reads that the database could resolve to something the build has not read.
/// On Postgres that is any operator, function, cast or type a user or an extension created: it
/// can shadow a built-in for its own argument types and answer NULL where the built-in never
/// does. A rule whose name is shadowed proves nothing.
#[derive(Debug, Clone, Default)]
pub struct Trust {
    /// Operator and function names with a definition outside the built-ins.
    pub shadowed: HashSet<String>,
    /// The built-in type names a cast may target and keep its argument's nullability. `None`
    /// trusts every name, which is what SQLite gets: nothing there can define a cast.
    pub cast_targets: Option<HashSet<String>>,
    /// Functions the host registers on its own connection, by name, with the type oid each
    /// returns: NULL only when an argument is NULL. Reylo Framework's `reylo_html` on SQLite is one.
    pub total: HashMap<String, u32>,
    /// Whether a parameter is a non-null value. A host whose callers cannot pass null (Reylo
    /// types a parameter without it) sets this; by default a placeholder proves nothing.
    pub non_null_params: bool,
    /// The parameter numbers that may be null all the same.
    pub nullable_params: HashSet<i32>,
    /// Whether the server's cast of a JSON null to a number or a boolean raises an error, which
    /// is every PostgreSQL before 18 ([`POSTGRES_JSONB_NULL_CASTS`]). A host that did not ask
    /// leaves it unset, and a cast of a `jsonb` value then proves nothing.
    pub jsonb_casts_raise: bool,
}

impl Trust {
    /// Whether `name`, an operator or a function a rule reads, can only resolve to a built-in.
    pub fn builtin(&self, name: &str) -> bool {
        !self.shadowed.contains(name)
    }

    fn cast(&self, name: &str) -> bool {
        self.cast_targets.as_ref().is_none_or(|t| t.contains(name))
    }

    /// Whether a name a [`Strict`] claim rests on is the built-in one: an operator or a function,
    /// or `::type` for a cast to that type, which no user's cast function targets.
    pub fn holds(&self, name: &str) -> bool {
        match name.strip_prefix("::") {
            Some(target) => self.cast(target),
            None => self.builtin(name),
        }
    }
}

/// Read one rewritten statement's expression columns. SQLite's `?n` placeholders are read as
/// `$n`, which is how libpg_query spells one; nothing runs against the database here.
pub fn plan(sql: &str, dialect: Dialect, trust: &Trust, columns: &Columns) -> Plan {
    crate::stack::run(|| plan_here(sql, dialect, trust, columns))
}

fn plan_here(sql: &str, dialect: Dialect, trust: &Trust, columns: &Columns) -> Plan {
    let text = match dialect {
        Dialect::Postgres => sql.to_string(),
        Dialect::Sqlite | Dialect::Mysql => dollar_placeholders(sql),
    };
    let Ok(parsed) = crate::stack::parse(&text) else { return Plan::default() };
    let version = parsed.protobuf.version;
    let Some(top) = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref()) else {
        return Plan::default();
    };
    if let NodeEnum::SelectStmt(s) = top {
        if let Some(kind) = set_kind(s) {
            let arm = |a: &Option<Box<SelectStmt>>| a.as_deref().and_then(|a| arm_text(version, a, s.with_clause.as_ref()));
            return match (arm(&s.larg), arm(&s.rarg)) {
                (Some(left), Some(right)) => Plan { set_op: Some(SetOp { kind, left, right }), ..Plan::default() },
                _ => Plan::default(),
            };
        }
        if !s.values_lists.is_empty() {
            return values_plan(s, dialect, trust, columns);
        }
    }
    let Some((targets, from, with)) = top_context(top) else { return Plan::default() };
    // A grouping set nulls a grouping column in its super-aggregate rows, which a probe over the
    // `FROM` alone would never show: a leaf there proves nothing.
    let grouped = matches!(top, NodeEnum::SelectStmt(s) if s.group_clause.iter().any(|g| matches!(g.node, Some(NodeEnum::GroupingSet(_)))));
    let scope = match top {
        NodeEnum::SelectStmt(s) if dialect == Dialect::Postgres && !grouped => Some(Scope::of(s, trust)),
        _ => None,
    };
    let plain_group_by = matches!(top, NodeEnum::SelectStmt(s) if !grouped && !s.group_clause.is_empty());
    let withs = Withs::default().with(with.as_ref(), columns);
    // The old and the new row of a RETURNING, with the relations a bare name is a column of first.
    let rows = Rows::of(top).map(|rows| (rows, Level::of(&from, &withs, columns)));
    let mut reader = Reader { dialect, trust, columns, withs, rows: rows.as_ref(), leaves: (!grouped).then(Vec::new), grouped: plain_group_by, narrowed: Vec::new() };
    let mut plan = Plan::default();
    for target in &targets {
        let Some(NodeEnum::ResTarget(rt)) = target.node.as_ref() else { continue };
        let Some(val) = rt.val.as_deref() else { continue };
        if is_star(val) {
            plan.star = true;
            plan.tail.clear();
            continue;
        }
        // A plain column reference is the describe tier's to answer, but for what the SELECT
        // around it proves: a strict WHERE on it, or that it is a table's whole row.
        let claim = match val.node.as_ref() {
            Some(NodeEnum::ColumnRef(c)) => scope.as_ref().and_then(|scope| scope.column(c, val, &mut reader)),
            _ => Some(reader.read(val)),
        };
        if plan.star {
            plan.tail.push(claim);
        } else {
            plan.head.push(claim);
        }
    }
    let leaves = reader.leaves.unwrap_or_default();
    if !leaves.is_empty() {
        // Every row the select list reads passed the WHERE, so the probe filters by it too: its
        // strict tests narrow the leaves and reduce outer joins there as they do here.
        let filter = match top {
            NodeEnum::SelectStmt(s) => s.where_clause.as_deref(),
            _ => None,
        };
        plan.leaves = select_text(version, &leaves, &from, filter, with.as_ref());
    }
    plan
}

impl Plan {
    /// The claim for each of `count` result columns, in order, or nothing when the target list
    /// and the describe disagree about how many there are.
    pub fn claims(&self, count: usize) -> Vec<Option<&Expr>> {
        let known = self.head.len() + self.tail.len();
        if (!self.star && known != count) || known > count {
            return vec![None; count];
        }
        let mut out: Vec<Option<&Expr>> = self.head.iter().map(Option::as_ref).collect();
        out.resize(count - self.tail.len(), None);
        out.extend(self.tail.iter().map(Option::as_ref));
        out
    }

    /// Whether any claim reads a leaf, so the leaf probe is worth its describe.
    pub fn reads_leaves(&self) -> bool {
        self.head.iter().chain(&self.tail).flatten().any(reads_leaf)
    }
}

/// One expression's answer, given the leaf probe's described columns.
pub fn resolve(expr: &Expr, leaves: Option<&[DescribedColumn]>) -> Fact {
    const NOTHING: Fact = Fact { non_null: false, type_oid: None };
    match expr {
        Expr::NonNull(type_oid) => Fact { non_null: true, type_oid: *type_oid },
        Expr::Unknown | Expr::Unread => NOTHING,
        Expr::Nullable(inner) => Fact { non_null: false, ..resolve(inner, leaves) },
        Expr::Leaf(i) => match leaves.and_then(|l| l.get(*i)) {
            Some(col) => Fact { non_null: !col.nullable, type_oid: (col.type_oid != oid::UNKNOWN).then_some(col.type_oid) },
            None => NOTHING,
        },
        Expr::Row(i, relation) => match leaves.and_then(|l| l.get(*i)) {
            Some(col) => Fact { non_null: !col.nullable || col.type_name == *relation, type_oid: (col.type_oid != oid::UNKNOWN).then_some(col.type_oid) },
            None => NOTHING,
        },
        Expr::Every(operands, ty) => {
            let facts: Vec<Fact> = operands.iter().map(|o| resolve(o, leaves)).collect();
            Fact { non_null: !facts.is_empty() && facts.iter().all(|f| f.non_null), type_oid: ty.of(&facts) }
        }
        Expr::Any(operands, ty) => {
            let facts: Vec<Fact> = operands.iter().map(|o| resolve(o, leaves)).collect();
            Fact { non_null: facts.iter().any(|f| f.non_null), type_oid: ty.of(&facts) }
        }
        Expr::Guarded(inner, types) => {
            let fact = resolve(inner, leaves);
            Fact { non_null: fact.non_null && fact.type_oid.is_some_and(|t| types.contains(&t)), ..fact }
        }
        Expr::Array { non_null, .. } => Fact { non_null: *non_null, type_oid: None },
        Expr::NotJsonb(inner) => {
            let fact = resolve(inner, leaves);
            Fact { non_null: fact.non_null && !may_be_jsonb(inner, leaves), ..fact }
        }
        Expr::Narrowed(inner) => Fact { non_null: true, ..resolve(inner, leaves) },
    }
}

/// Whether an expression may hold a `jsonb` value: its type is `jsonb`, or one a database
/// defined, which may be a domain over it; or the rules name no type for it and one of the values
/// it is made of may hold one. A parameter under a cast takes the cast's type, and a function
/// the rules read without a type returns none that is `jsonb` ([`POSTGRES_JSONB`] names the ones
/// that do).
fn may_be_jsonb(expr: &Expr, leaves: Option<&[DescribedColumn]>) -> bool {
    match resolve(expr, leaves).type_oid {
        Some(ty) => ty == oid::JSONB || ty >= FIRST_USER_OID,
        None => match expr {
            Expr::NonNull(_) | Expr::Array { .. } => false,
            Expr::Unknown | Expr::Unread | Expr::Leaf(_) | Expr::Row(..) => true,
            Expr::Nullable(inner) | Expr::Guarded(inner, _) | Expr::NotJsonb(inner) | Expr::Narrowed(inner) => may_be_jsonb(inner, leaves),
            Expr::Every(operands, _) | Expr::Any(operands, _) => operands.iter().any(|o| may_be_jsonb(o, leaves)),
        },
    }
}

/// What the statement says about an array expression's elements, given the leaf probe and which
/// type oids are arrays ([`DescribedColumn::nullable_items`]). Only an array the statement builds
/// says anything: `Some(true)` when an element may be NULL (`ARRAY[nickname]`, `array_agg(u.email)`
/// over a LEFT JOIN, a literal that spells NULL), `Some(false)` when every element is proven
/// non-null. A column's array, a function's, `ARRAY(SELECT …)`, an element that is itself an
/// array (`array_agg(tags)` is two-dimensional, its elements the tags') and one that reads a
/// column the leaf probe does not ([`Expr::Unread`]) say nothing: `None`. A `CASE`, a `coalesce`
/// or an operator over arrays may return any operand's elements.
pub fn items_nullable(expr: &Expr, leaves: Option<&[DescribedColumn]>, is_array: &dyn Fn(u32) -> bool) -> Option<bool> {
    match expr {
        Expr::Array { items, .. } => join_items(items.iter().map(|e| match e {
            // A nested `ARRAY[…]` is a row of a multidimensional array, its elements the result's.
            Expr::Array { .. } => items_nullable(e, leaves, is_array),
            _ if reads_unread(e) => None,
            _ => {
                let fact = resolve(e, leaves);
                match fact.type_oid {
                    _ if !fact.non_null => Some(true),
                    Some(t) if !is_array(t) => Some(false),
                    _ => None,
                }
            }
        })),
        Expr::Nullable(inner) | Expr::Guarded(inner, _) | Expr::NotJsonb(inner) | Expr::Narrowed(inner) => items_nullable(inner, leaves, is_array),
        Expr::Every(operands, _) | Expr::Any(operands, _) => join_items(operands.iter().map(|o| items_nullable(o, leaves, is_array))),
        Expr::NonNull(_) | Expr::Unknown | Expr::Unread | Expr::Leaf(_) | Expr::Row(..) => None,
    }
}

/// Whether an expression reads a column the reader had no leaf probe for ([`Expr::Unread`]).
fn reads_unread(expr: &Expr) -> bool {
    match expr {
        Expr::Unread => true,
        Expr::Nullable(inner) | Expr::Guarded(inner, _) | Expr::NotJsonb(inner) | Expr::Narrowed(inner) => reads_unread(inner),
        Expr::Every(operands, _) | Expr::Any(operands, _) => operands.iter().any(reads_unread),
        Expr::Array { items, .. } => items.iter().any(reads_unread),
        Expr::NonNull(_) | Expr::Unknown | Expr::Leaf(_) | Expr::Row(..) => false,
    }
}

/// Elements drawn from several sources: nullable when any source's may be NULL, non-null when
/// every source's are proven so, and otherwise unknown.
fn join_items(sources: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    let mut out = Some(false);
    for source in sources {
        match source {
            Some(true) => return Some(true),
            None => out = None,
            Some(false) => {}
        }
    }
    out
}

/// A set operation's columns from its arms' describes: non-null in every arm of a `UNION`, in
/// either arm of an `INTERSECT` (a row it returns is not distinct from a row of each arm, and
/// NULL is not distinct only from NULL), in the left arm of an `EXCEPT` (every row it returns is
/// one of the left arm's). A type is claimed only where both arms describe the same one.
pub fn combine(kind: SetKind, left: &[DescribedColumn], right: &[DescribedColumn]) -> Option<Vec<Fact>> {
    if left.len() != right.len() {
        return None;
    }
    Some(
        left.iter()
            .zip(right)
            .map(|(l, r)| Fact {
                non_null: match kind {
                    SetKind::Union => !l.nullable && !r.nullable,
                    SetKind::Intersect => !l.nullable || !r.nullable,
                    SetKind::Except => !l.nullable,
                },
                type_oid: (l.type_oid == r.type_oid && l.type_oid != oid::UNKNOWN).then_some(l.type_oid),
            })
            .collect(),
    )
}

/// Whether a subquery returns exactly one row once it aggregates: no grouping, no `HAVING` to
/// drop the one group, no `LIMIT` or `OFFSET` to skip it, and no set operation or `VALUES` to be
/// something other than one `SELECT`. Postgres: "If a query contains aggregate function calls,
/// but no GROUP BY clause, grouping still occurs: the result is a single group row". Whether it
/// does aggregate is the caller's to show.
pub fn single_row(select: &SelectStmt) -> bool {
    set_kind(select).is_none()
        && select.values_lists.is_empty()
        && select.group_clause.is_empty()
        && select.having_clause.is_none()
        && select.limit_count.is_none()
        && select.limit_offset.is_none()
}

/// Whether a SELECT that gives one row keeps it, as a scalar subquery reads it: [`single_row`],
/// with a `LIMIT` of a positive constant allowed, which keeps the one row an aggregate query
/// gives, and a target that is one value for that row ([`one_value`]).
pub fn keeps_one_row(select: &SelectStmt, target: &Node, dialect: Dialect, trust: &Trust) -> bool {
    let limit_keeps = match select.limit_count.as_deref().map(|n| n.node.as_ref()) {
        None => true,
        Some(Some(NodeEnum::AConst(c))) => matches!(c.val.as_ref(), Some(protobuf::a_const::Val::Ival(i)) if i.ival >= 1),
        Some(_) => false,
    };
    set_kind(select).is_none() && select.values_lists.is_empty() && select.group_clause.is_empty() && select.having_clause.is_none() && select.limit_offset.is_none() && limit_keeps && one_value(target, dialect, trust)
}

/// Whether a SELECT is one row whatever it reads: no FROM, no WHERE, nothing that groups or
/// limits it, and a target that is one value ([`one_value`]).
pub fn one_row(select: &SelectStmt, target: &Node, dialect: Dialect, trust: &Trust) -> bool {
    select.from_clause.is_empty() && select.where_clause.is_none() && single_row(select) && one_value(target, dialect, trust)
}

/// Whether a target is one value for the row it is computed over, never a set of them. A
/// set-returning call anywhere in it makes as many rows as the call returns, none included:
/// `(select generate_series(1, count(*)::int) from t)` is no row over an empty `t`, and so NULL,
/// whatever the rules say of its value, and so is `(select count(*)::text || (f() is null)::text
/// from t)` where `f` returns no row. So every function the target calls is one the rules read
/// for a built-in that returns no set (an unknown function may return one), and every operator
/// is one of the built-in operators the rules read: no built-in operator returns a set, but a
/// user's may. An aggregate's arguments, `ORDER BY` and `FILTER` are not read, since Postgres
/// refuses a set-returning call there, and a subquery is a value of its own, one row or none.
/// SQLite and MySQL take no set-returning call in a select list.
pub fn one_value(target: &Node, dialect: Dialect, trust: &Trust) -> bool {
    use serde_json::Value;
    /// A name of one part, or one in `pg_catalog`, as [`builtin_name`] reads it.
    fn name(names: Option<&Value>) -> Option<&str> {
        let parts: Vec<&str> = names?.as_array()?.iter().map(|n| n.pointer("/node/String/sval").and_then(Value::as_str)).collect::<Option<_>>()?;
        match parts.as_slice() {
            [name] | ["pg_catalog", name] => Some(name),
            _ => None,
        }
    }
    fn walk(v: &Value, trust: &Trust) -> bool {
        match v {
            Value::Object(map) => {
                if map.contains_key("SubLink") {
                    return true;
                }
                if let Some(f) = map.get("FuncCall") {
                    let Some(n) = name(f.get("funcname")).filter(|n| trust.builtin(n)) else { return false };
                    if [FUNCTIONS, POSTGRES_GROUPED, POSTGRES_GROUPED_NEVER_NULL, POSTGRES_GROUPED_JSON].iter().any(|list| list.contains(&n)) {
                        return true;
                    }
                    if POSTGRES_SET_RETURNING.contains(&n) || !postgres_function_names().contains(&n) {
                        return false;
                    }
                }
                if let Some(a) = map.get("AExpr") {
                    let between = [protobuf::AExprKind::AexprBetween, protobuf::AExprKind::AexprNotBetween, protobuf::AExprKind::AexprBetweenSym, protobuf::AExprKind::AexprNotBetweenSym];
                    // `BETWEEN` is named for itself and compares with `<=` and `>=` (`<` and `>` negated).
                    let built_in = if a.get("kind").and_then(Value::as_i64).is_some_and(|k| between.iter().any(|b| *b as i64 == k)) {
                        ["<", ">", "<=", ">="].iter().all(|o| trust.builtin(o))
                    } else {
                        name(a.get("name")).is_some_and(|o| (POSTGRES_OPERATORS.contains(&o) || POSTGRES_STRICT_OPERATORS.contains(&o)) && trust.builtin(o))
                    };
                    if !built_in {
                        return false;
                    }
                }
                map.values().all(|v| walk(v, trust))
            }
            Value::Array(items) => items.iter().all(|v| walk(v, trust)),
            _ => true,
        }
    }
    dialect != Dialect::Postgres || serde_json::to_value(target).is_ok_and(|v| walk(&v, trust))
}

/// An array cast to a number or a boolean array, whose elements a JSON null among them makes
/// NULL: every element the array is built from, read as one that is non-null only where it holds
/// no `jsonb` value for certain ([`Expr::NotJsonb`]). The array itself is as it was.
fn elements_not_jsonb(expr: Expr) -> Expr {
    match expr {
        Expr::Array { non_null, items } => Expr::Array {
            non_null,
            items: items
                .into_iter()
                .map(|item| match item {
                    // A nested constructor is a row of the array, its elements the array's.
                    Expr::Array { .. } => elements_not_jsonb(item),
                    other => Expr::NotJsonb(Box::new(other)),
                })
                .collect(),
        },
        Expr::Every(operands, ty) => Expr::Every(operands.into_iter().map(elements_not_jsonb).collect(), ty),
        Expr::Any(operands, ty) => Expr::Any(operands.into_iter().map(elements_not_jsonb).collect(), ty),
        Expr::Nullable(inner) => Expr::Nullable(Box::new(elements_not_jsonb(*inner))),
        Expr::Guarded(inner, types) => Expr::Guarded(Box::new(elements_not_jsonb(*inner)), types),
        Expr::NotJsonb(inner) => Expr::NotJsonb(Box::new(elements_not_jsonb(*inner))),
        Expr::Narrowed(inner) => Expr::Narrowed(Box::new(elements_not_jsonb(*inner))),
        other => other,
    }
}

/// The elements an `ARRAY[…]` constructor writes, the elements of nested ones flattened.
fn array_elements(node: &Node) -> Option<Vec<&Node>> {
    let Some(NodeEnum::AArrayExpr(a)) = node.node.as_ref() else { return None };
    let mut out = Vec::new();
    for e in &a.elements {
        match e.node.as_ref() {
            Some(NodeEnum::AArrayExpr(_)) => out.extend(array_elements(e)?),
            _ => out.push(e),
        }
    }
    Some(out)
}

/// Whether an aggregate call sits anywhere in this target expression outside a subquery, which makes
/// the `SELECT` holding it an aggregate query: the reading `single_row` leans on, for a caller that
/// proves a row count (`cardinality`) rather than a nullability. `level` is the SELECT's FROM
/// where the SELECT stands inside another statement: an aggregate counts there only when it is
/// that SELECT's own ([`Level::owns`]).
pub fn aggregates(expr: &Node, dialect: Dialect, trust: &Trust, level: Option<&Level>) -> bool {
    Reader { dialect, trust, columns: &Columns::default(), withs: Withs::default(), rows: None, leaves: None, grouped: false, narrowed: Vec::new() }.aggregates(expr, true, level)
}

impl Ty {
    fn of(self, facts: &[Fact]) -> Option<u32> {
        match self {
            Ty::Is(o) => Some(o),
            Ty::Unknown => None,
            Ty::Shared => {
                let first = facts.first()?.type_oid?;
                facts.iter().all(|f| f.type_oid == Some(first)).then_some(first)
            }
            Ty::Concat => (!facts.is_empty() && facts.iter().all(|f| f.type_oid.is_some_and(|t| TEXT_TYPES.contains(&t)))).then_some(oid::TEXT),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Recognition
// ---------------------------------------------------------------------------------------------

struct Reader<'a> {
    dialect: Dialect,
    trust: &'a Trust,
    /// The columns of the statement's relations, and the WITH queries in scope where the reader
    /// stands: what says whose an aggregate in a scalar subquery is ([`crate::levels`]).
    columns: &'a Columns,
    withs: Withs,
    /// The old and the new row a RETURNING list can name ([`crate::returning`]), and the FROM its
    /// leaf probe selects over; none in a SELECT, and none inside a subquery, which has no leaves.
    rows: Option<&'a (Rows, Level)>,
    /// The column references read so far, in leaf order; `None` where a reference proves nothing.
    leaves: Option<Vec<Node>>,
    /// Whether the expressions belong to a SELECT with a plain `GROUP BY`, where no group is empty.
    grouped: bool,
    /// The columns, as written, that cannot be NULL where the reader stands: inside a CASE arm
    /// whose WHEN is strict in them, or after a WHEN that tested them `IS NULL` and failed.
    narrowed: Vec<Vec<String>>,
}

impl Reader<'_> {
    fn read(&mut self, expr: &Node) -> Expr {
        let bool_ty = Ty::Is(oid::BOOL);
        match expr.node.as_ref() {
            Some(NodeEnum::AConst(c)) => {
                if c.isnull {
                    return Expr::Unknown;
                }
                // An array literal that spells no NULL holds no NULL element, as the `'{}'` of
                // `coalesce(array_agg(x), '{}')`; one that spells it anywhere, even quoted, may.
                // It is still the text literal every other string is, and the same text read as
                // another type (`'{}'::jsonb`) is not an array: what this says of its elements is
                // then never asked.
                if let (Dialect::Postgres, Some(protobuf::a_const::Val::Sval(s))) = (self.dialect, c.val.as_ref()) {
                    let text = s.sval.trim();
                    if text.starts_with('{') && text.ends_with('}') {
                        let items = if text.to_ascii_lowercase().contains("null") { vec![Expr::Unknown] } else { Vec::new() };
                        return Expr::Every(vec![Expr::Array { non_null: true, items }], Ty::Is(oid::TEXT));
                    }
                }
                Expr::NonNull(match c.val.as_ref() {
                    Some(protobuf::a_const::Val::Ival(_)) => Some(oid::INT4),
                    // A literal past int4, or one with a fraction or an exponent.
                    Some(protobuf::a_const::Val::Fval(f)) => Some(if f.fval.parse::<i64>().is_ok() { oid::INT8 } else { oid::FLOAT8 }),
                    Some(protobuf::a_const::Val::Boolval(_)) => Some(oid::BOOL),
                    Some(protobuf::a_const::Val::Sval(_)) => Some(oid::TEXT),
                    _ => None,
                })
            }
            Some(NodeEnum::ColumnRef(c)) => {
                // A RETURNING's old or new row is the target relation's as far as the leaf probe
                // can tell, and NULL in every column where the statement has no such row.
                let (leaf, missing) = match self.rows.and_then(|(rows, from)| rows.read(c, &|name| from.lacks(name))) {
                    Some((read, missing)) => (Node { node: Some(NodeEnum::ColumnRef(read)) }, missing),
                    None => (expr.clone(), false),
                };
                let read = match self.leaves.as_mut() {
                    Some(leaves) if !is_star(expr) => {
                        leaves.push(leaf);
                        Expr::Leaf(leaves.len() - 1)
                    }
                    Some(_) => Expr::Unknown,
                    None => Expr::Unread,
                };
                let read = if missing { Expr::Nullable(Box::new(read)) } else { read };
                let fields: Option<Vec<String>> = c.fields.iter().map(|f| match f.node.as_ref() {
                    Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                    _ => None,
                }).collect();
                match fields {
                    Some(fields) if self.narrowed.contains(&fields) => Expr::Narrowed(Box::new(read)),
                    _ => read,
                }
            }
            // `ROW(a, b)` and `(a, b)` build a row whatever their fields hold, and a cast of one
            // to a composite type only relabels it: no cast can be defined from `record`.
            Some(NodeEnum::RowExpr(_)) if self.dialect == Dialect::Postgres => Expr::NonNull(None),
            Some(NodeEnum::TypeCast(t)) if self.dialect == Dialect::Postgres && matches!(t.arg.as_deref().and_then(|a| a.node.as_ref()), Some(NodeEnum::RowExpr(_))) => Expr::NonNull(None),
            Some(NodeEnum::TypeCast(t)) => {
                let (Some(arg), Some(target)) = (t.arg.as_deref(), t.type_name.as_ref()) else { return Expr::Unknown };
                if target.setof || target.pct_type {
                    return Expr::Unknown;
                }
                let Some(name) = builtin_name(&target.names) else { return Expr::Unknown };
                let operand = self.read(arg);
                // A cast to an array of a built-in type casts each element: an array stays an
                // array (`ARRAY[]::int[]`, `'{}'::text[]`), and NULL only where it was NULL. A
                // user's cast to the array type is listed under that type's own name, `_int4`.
                if !target.array_bounds.is_empty() {
                    return match self.dialect {
                        Dialect::Postgres if self.trust.cast(&name) && self.trust.cast(&format!("_{name}")) => {
                            // Each element is cast as one value is, keeping its nullability but
                            // where a JSON null cast to a number or a boolean is NULL
                            // (`ARRAY['null'::jsonb]::int[]` is `{NULL}`).
                            let operand = if POSTGRES_JSONB_NULL_CASTS.contains(&name.as_str()) && !self.trust.jsonb_casts_raise { elements_not_jsonb(operand) } else { operand };
                            Expr::Every(vec![operand], Ty::Unknown)
                        }
                        _ => Expr::Unknown,
                    };
                }
                match self.dialect {
                    Dialect::Postgres if self.trust.cast(&name) => {
                        let operand = if POSTGRES_JSONB_NULL_CASTS.contains(&name.as_str()) && !self.trust.jsonb_casts_raise { Expr::NotJsonb(Box::new(operand)) } else { operand };
                        Expr::Every(vec![operand], POSTGRES_CASTS.iter().find(|(n, _)| *n == name).map_or(Ty::Unknown, |(_, o)| Ty::Is(*o)))
                    }
                    Dialect::Sqlite | Dialect::Mysql if SQLITE_TOTAL_CASTS.contains(&name.as_str()) => {
                        let ty = SQLITE_CASTS.iter().find(|(n, _)| *n == name).map_or(Ty::Unknown, |(_, o)| Ty::Is(*o));
                        Expr::Every(vec![operand], ty)
                    }
                    _ => Expr::Unknown,
                }
            }
            Some(NodeEnum::AExpr(a)) => {
                let distinct = [protobuf::AExprKind::AexprDistinct as i32, protobuf::AExprKind::AexprNotDistinct as i32];
                if distinct.contains(&a.kind) {
                    return Expr::NonNull(Some(oid::BOOL));
                }
                // `LIKE`, `ILIKE` and `SIMILAR TO` are operators the parse marks apart, under the
                // operator's own name.
                let matching = [protobuf::AExprKind::AexprLike as i32, protobuf::AExprKind::AexprIlike as i32, protobuf::AExprKind::AexprSimilar as i32];
                if a.kind != protobuf::AExprKind::AexprOp as i32 && !(self.dialect == Dialect::Postgres && matching.contains(&a.kind)) {
                    return Expr::Unknown;
                }
                let Some(op) = builtin_name(&a.name) else { return Expr::Unknown };
                let accepted = match self.dialect {
                    Dialect::Postgres => POSTGRES_OPERATORS.contains(&op.as_str()) && self.trust.builtin(&op),
                    Dialect::Sqlite | Dialect::Mysql => SQLITE_OPERATORS.contains(&op.as_str()),
                };
                let Some(right) = a.rexpr.as_deref() else { return Expr::Unknown };
                // A prefix operator: a sign, and on Postgres a root, absolute value or bitwise not.
                let left = match a.lexpr.as_deref() {
                    Some(l) => Some(l),
                    None if op == "-" || op == "+" => None,
                    None if self.dialect == Dialect::Postgres && POSTGRES_PREFIX_OPERATORS.contains(&op.as_str()) => None,
                    None => return Expr::Unknown,
                };
                if !accepted {
                    return Expr::Unknown;
                }
                let ty = match op.as_str() {
                    "||" if self.dialect != Dialect::Postgres => Ty::Is(oid::TEXT),
                    "||" => Ty::Concat,
                    "=" | "==" | "<>" | "!=" | "<" | ">" | "<=" | ">=" => bool_ty,
                    m if self.dialect == Dialect::Postgres && left.is_some() && POSTGRES_MATCH_OPERATORS.contains(&m) => Ty::Is(oid::BOOL),
                    _ => Ty::Unknown,
                };
                let mut operands: Vec<Expr> = left.into_iter().map(|l| self.read(l)).collect();
                operands.push(self.read(right));
                Expr::Every(operands, ty)
            }
            Some(NodeEnum::BoolExpr(b)) if !b.args.is_empty() => Expr::Every(b.args.iter().map(|a| self.read(a)).collect(), bool_ty),
            Some(NodeEnum::NullTest(_)) | Some(NodeEnum::BooleanTest(_)) => Expr::NonNull(Some(oid::BOOL)),
            // `ARRAY[...]` always builds an array, whatever its elements hold; the describe already
            // names the element type, and its elements are the expressions listed.
            Some(NodeEnum::AArrayExpr(a)) => Expr::Array { non_null: true, items: a.elements.iter().map(|e| self.read(e)).collect() },
            Some(NodeEnum::CoalesceExpr(c)) if !c.args.is_empty() => Expr::Any(c.args.iter().map(|a| self.read(a)).collect(), Ty::Shared),
            // Postgres parses `greatest(…)` and `least(…)` as syntax, which no function of the name
            // shadows: NULL only where every argument is. MySQL's are NULL where any is.
            Some(NodeEnum::MinMaxExpr(m)) if self.dialect == Dialect::Postgres && !m.args.is_empty() => Expr::Any(m.args.iter().map(|a| self.read(a)).collect(), Ty::Shared),
            Some(NodeEnum::CaseExpr(c)) => {
                // In a searched CASE an arm runs only where its WHEN is true, so a column the WHEN
                // is strict in is not NULL there (`WHEN x > 0 THEN x`); every later arm runs only
                // where the WHEN was not, so after `WHEN x IS NULL` the column is not NULL
                // (`ELSE x`). A simple CASE compares its operand, and proves nothing of it.
                let searched = c.arg.is_none();
                let outer = self.narrowed.len();
                let mut results = Vec::with_capacity(c.args.len() + 1);
                for when in &c.args {
                    let Some(NodeEnum::CaseWhen(w)) = when.node.as_ref() else {
                        self.narrowed.truncate(outer);
                        return Expr::Unknown;
                    };
                    let Some(result) = w.result.as_deref() else {
                        self.narrowed.truncate(outer);
                        return Expr::Unknown;
                    };
                    let before = self.narrowed.len();
                    if let (true, Some(test)) = (searched, w.expr.as_deref()) {
                        let mut strict = Vec::new();
                        strict_columns(test, self.trust, &mut strict);
                        self.narrowed.extend(strict);
                    }
                    results.push(self.read(result));
                    self.narrowed.truncate(before);
                    if let (true, Some(NodeEnum::NullTest(t))) = (searched, w.expr.as_deref().and_then(|e| e.node.as_ref())) {
                        if let (true, Some(NodeEnum::ColumnRef(col))) = (t.nulltesttype == protobuf::NullTestType::IsNull as i32, t.arg.as_deref().and_then(|a| a.node.as_ref())) {
                            let fields: Option<Vec<String>> = col.fields.iter().map(|f| match f.node.as_ref() {
                                Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                                _ => None,
                            }).collect();
                            self.narrowed.extend(fields);
                        }
                    }
                }
                let read = match c.defresult.as_deref() {
                    Some(otherwise) => {
                        results.push(self.read(otherwise));
                        Expr::Every(results, Ty::Shared)
                    }
                    // No `ELSE` is `ELSE NULL`.
                    None => Expr::Nullable(Box::new(Expr::Every(results, Ty::Shared))),
                };
                self.narrowed.truncate(outer);
                read
            }
            Some(NodeEnum::FuncCall(f)) => self.call(f),
            Some(NodeEnum::SubLink(s)) => self.sublink(s),
            // `current_timestamp`, `current_user` and the rest of the SQL value functions.
            Some(NodeEnum::SqlvalueFunction(_)) => Expr::NonNull(None),
            // `merge_action()` names the action that produced the row a MERGE returns.
            Some(NodeEnum::MergeSupportFunc(_)) if self.dialect == Dialect::Postgres => Expr::NonNull(Some(oid::TEXT)),
            // Where the host types its parameters without null, a parameter is a non-null value.
            Some(NodeEnum::ParamRef(p)) if self.trust.non_null_params && !self.trust.nullable_params.contains(&p.number) => Expr::NonNull(None),
            _ => Expr::Unknown,
        }
    }

    /// A call, its arguments read where an aggregate's FILTER holds: only rows the FILTER passes
    /// reach an aggregate, so a column it is strict in is not NULL in what it aggregates
    /// (`array_agg(x) FILTER (WHERE x IS NOT NULL)`).
    fn call(&mut self, f: &protobuf::FuncCall) -> Expr {
        // A window function reads its arguments from other rows of its frame, of which the WHEN
        // of a CASE arm it stands in proves nothing (`lag(x) OVER …` under `WHEN x IS NOT NULL`
        // reads the previous row's `x`): only its own FILTER narrows them.
        let outer = f.over.is_some().then(|| std::mem::take(&mut self.narrowed));
        let before = self.narrowed.len();
        if let Some(filter) = f.agg_filter.as_deref() {
            let mut strict = Vec::new();
            strict_columns(filter, self.trust, &mut strict);
            self.narrowed.extend(strict);
        }
        let read = self.call_unfiltered(f);
        self.narrowed.truncate(before);
        if let Some(outer) = outer {
            self.narrowed = outer;
        }
        read
    }

    fn call_unfiltered(&mut self, f: &protobuf::FuncCall) -> Expr {
        let Some(name) = builtin_name(&f.funcname) else { return Expr::Unknown };
        if f.agg_within_group || f.func_variadic {
            return Expr::Unknown;
        }
        match (self.dialect, name.as_str(), f.args.len()) {
            // `count(*)`, `count(x)`, `count(DISTINCT x)`, filtered or windowed: a count of
            // nothing is 0.
            (_, "count", 0 | 1) if self.trust.builtin("count") => Expr::NonNull(Some(oid::INT8)),
            // A ranking window function numbers every row of its partition.
            (Dialect::Postgres, "row_number" | "rank" | "dense_rank", 0) if f.over.is_some() && self.trust.builtin(&name) => Expr::NonNull(Some(oid::INT8)),
            (Dialect::Postgres, "percent_rank" | "cume_dist", 0) if f.over.is_some() && self.trust.builtin(&name) => Expr::NonNull(Some(oid::FLOAT8)),
            // `ntile` is NULL for a NULL bucket count (windowfuncs.c), and `lag` and `lead` return
            // their default where the offset leaves the partition: non-null when the value, the
            // offset and the default all are. With no default they are NULL there.
            (Dialect::Postgres, "ntile", 1) if f.over.is_some() && self.trust.builtin("ntile") => Expr::Every(f.args.iter().map(|a| self.read(a)).collect(), Ty::Is(oid::INT4)),
            (Dialect::Postgres, "lag" | "lead", 3) if f.over.is_some() && self.trust.builtin(&name) => Expr::Every(f.args.iter().map(|a| self.read(a)).collect(), Ty::Unknown),
            // "total() returns 0.0" over no rows, but a sum of `1e999` and `-1e999` is NaN, which
            // SQLite stores as NULL: a float, and nullable.
            (Dialect::Sqlite, "total", 1) => Expr::Nullable(Box::new(Expr::NonNull(Some(oid::FLOAT8)))),
            // "The ifnull() function is equivalent to coalesce() with two arguments."
            (Dialect::Sqlite, "ifnull", 2) => Expr::Any(f.args.iter().map(|a| self.read(a)).collect(), Ty::Shared),
            // Under a plain GROUP BY every group has a row, so an aggregate that is NULL only over
            // no rows or only NULLs follows its argument. A FILTER can leave it no rows, a window
            // frame can be empty, and SQLite's sum of `1e999` and `-1e999` is NaN, stored as NULL.
            (Dialect::Postgres, n, 1 | 2) if self.grouped && f.over.is_none() && f.agg_filter.is_none() && self.trust.builtin(n) && (POSTGRES_GROUPED.contains(&n) || POSTGRES_GROUPED_NEVER_NULL.contains(&n) || POSTGRES_GROUPED_JSON.contains(&n)) => {
                if POSTGRES_GROUPED_JSON.contains(&n) {
                    Expr::NonNull(None)
                } else if POSTGRES_GROUPED_NEVER_NULL.contains(&n) {
                    Expr::Array { non_null: true, items: f.args.iter().take(1).map(|a| self.read(a)).collect() }
                } else {
                    Expr::Every(f.args.iter().take(1).map(|a| self.read(a)).collect(), Ty::Unknown)
                }
            }
            // Anywhere else `array_agg` may see no rows and be NULL, but its elements are still the
            // values of its argument.
            (Dialect::Postgres, "array_agg", 1) if self.trust.builtin("array_agg") => Expr::Array { non_null: false, items: f.args.iter().map(|a| self.read(a)).collect() },
            (Dialect::Postgres, n, _) if POSTGRES_NEVER_NULL.contains(&n) && self.trust.builtin(n) => Expr::NonNull(None),
            // `unnest(ARRAY[…])` gives the elements written, a nested array's flattened: none NULL
            // where none of them is and each is a value of a scalar type. An element that is an
            // array itself (`array[tags]`, `array['{NULL}'::int[]]`) makes the array one of more
            // dimensions, whose elements unnest gives, NULL ones among them; a string literal,
            // read as text, takes the type of the elements beside it, an array only where one of
            // them is. Any other array may hold NULL elements whatever its column says.
            (Dialect::Postgres, "unnest", 1) if self.trust.builtin("unnest") => match array_elements(&f.args[0]) {
                Some(elements) => Expr::Every(elements.into_iter().map(|e| Expr::Guarded(Box::new(self.read(e)), POSTGRES_SCALARS)).collect(), Ty::Unknown),
                None => Expr::Unknown,
            },
            (Dialect::Postgres, "uuidv7", 0) if self.trust.builtin("uuidv7") => Expr::NonNull(Some(oid::UUID)),
            (Dialect::Postgres, n, 1..) if POSTGRES_FIRST_ARG.contains(&n) && self.trust.builtin(n) => Expr::Every(f.args.iter().take(1).map(|a| self.read(a)).collect(), Ty::Is(oid::TEXT)),
            (Dialect::Postgres, n, 1..) if self.trust.builtin(n) && POSTGRES_TOTAL_FOR.iter().any(|(name, _)| *name == n) => {
                let types = POSTGRES_TOTAL_FOR.iter().find(|(name, _)| *name == n).map(|(_, t)| *t).unwrap_or(&[]);
                let mut operands: Vec<Expr> = f.args.iter().map(|a| self.read(a)).collect();
                operands[0] = Expr::Guarded(Box::new(operands[0].clone()), types);
                Expr::Every(operands, Ty::Unknown)
            }
            (Dialect::Postgres, n, _) if POSTGRES_ANY.contains(&n) && self.trust.builtin(n) => Expr::Any(f.args.iter().map(|a| self.read(a)).collect(), Ty::Shared),
            (Dialect::Postgres, n, _) if POSTGRES_TOTAL.contains(&n) && self.trust.builtin(n) => {
                let ty = if POSTGRES_JSONB.contains(&n) { Ty::Is(oid::JSONB) } else { Ty::Unknown };
                Expr::Every(f.args.iter().map(|a| self.read(a)).collect(), ty)
            }
            _ => match self.trust.total.get(name.as_str()) {
                Some(&ty) => Expr::Every(f.args.iter().map(|a| self.read(a)).collect(), Ty::Is(ty)),
                None => Expr::Unknown,
            },
        }
    }

    fn sublink(&mut self, s: &protobuf::SubLink) -> Expr {
        if s.sub_link_type == protobuf::SubLinkType::ExistsSublink as i32 {
            return Expr::NonNull(Some(oid::BOOL));
        }
        // `ARRAY(SELECT …)` is an array whatever the subquery returns: empty for no rows.
        if s.sub_link_type == protobuf::SubLinkType::ArraySublink as i32 {
            return Expr::NonNull(None);
        }
        if s.sub_link_type != protobuf::SubLinkType::ExprSublink as i32 {
            return Expr::Unknown;
        }
        let Some(select) = s.subselect.as_deref().and_then(as_select) else { return Expr::Unknown };
        let Some(inner) = single_target(select).and_then(|rt| rt.val.as_deref()) else { return Expr::Unknown };
        // The leaf probe selects over the outer `FROM`, which a reference inside a subquery does
        // not read, so the inner expression is read without leaves.
        let withs = self.withs.with(select.with_clause.as_ref(), self.columns);
        let level = Level::of(&select.from_clause, &withs, self.columns);
        let mut inner_reader = Reader { dialect: self.dialect, trust: self.trust, columns: self.columns, withs, rows: None, leaves: None, grouped: false, narrowed: Vec::new() };
        let value = inner_reader.read(inner);
        if one_row(select, inner, self.dialect, self.trust) || (keeps_one_row(select, inner, self.dialect, self.trust) && self.aggregates(inner, false, Some(&level))) {
            value
        } else {
            Expr::Nullable(Box::new(value))
        }
    }

    /// Whether an aggregate call sits anywhere in this target expression outside a subquery,
    /// which makes the `SELECT` holding it an aggregate query. Only a name that is an aggregate
    /// at this arity counts: SQLite's two-argument `max(a, b)` is a scalar function. With
    /// `ordered_sets`, which the row count of a statement asks for, a Postgres call with `WITHIN
    /// GROUP` is an aggregate whatever its name: the server takes the clause for an ordered-set or
    /// a hypothetical-set aggregate only (`percentile_disc`, `mode`, `rank(1)`), a user's
    /// included, and refuses it on any other function and beside `OVER`. With `from`, the FROM
    /// of a subquery the expression is a target of, an aggregate that is not that subquery's for
    /// certain is left out: one over columns of a query around it belongs to that query, and
    /// leaves the subquery a row per row of its FROM ([`Level::owns`]).
    fn aggregates(&self, expr: &Node, ordered_sets: bool, from: Option<&Level>) -> bool {
        let children: Vec<&Node> = match expr.node.as_ref() {
            Some(NodeEnum::FuncCall(f)) => {
                let aggregate = f.over.is_none()
                    && ((ordered_sets && self.dialect == Dialect::Postgres && f.agg_within_group)
                        || builtin_name(&f.funcname).is_some_and(|name| {
                            let arity = match name.as_str() {
                                "count" => f.args.len() <= 1,
                                "sum" | "avg" | "min" | "max" | "json_agg" | "jsonb_agg" => f.args.len() == 1,
                                "total" | "json_group_array" => self.dialect == Dialect::Sqlite && f.args.len() == 1,
                                _ => false,
                            };
                            arity && self.trust.builtin(&name)
                        }));
                if aggregate && from.is_none_or(|level| level.owns(f)) {
                    return true;
                }
                f.args.iter().collect()
            }
            Some(NodeEnum::AExpr(a)) => a.lexpr.iter().chain(a.rexpr.iter()).map(|b| b.as_ref()).collect(),
            Some(NodeEnum::TypeCast(t)) => t.arg.iter().map(|b| b.as_ref()).collect(),
            Some(NodeEnum::CoalesceExpr(c)) => c.args.iter().collect(),
            Some(NodeEnum::MinMaxExpr(m)) => m.args.iter().collect(),
            Some(NodeEnum::BoolExpr(b)) => b.args.iter().collect(),
            Some(NodeEnum::CaseExpr(c)) => {
                let mut out: Vec<&Node> = c.defresult.iter().map(|b| b.as_ref()).collect();
                for when in &c.args {
                    if let Some(NodeEnum::CaseWhen(w)) = when.node.as_ref() {
                        out.extend(w.expr.iter().chain(w.result.iter()).map(|b| b.as_ref()));
                    }
                }
                out
            }
            _ => Vec::new(),
        };
        children.into_iter().any(|c| self.aggregates(c, ordered_sets, from))
    }
}

/// What one target of a select list reads of the statement's FROM items that are statements of
/// their own ([`ItemQuery`]). Such a column's provenance names the table behind the item at best,
/// and the catalog's word on that column is not the last one: the item may filter it (`WHERE x IS
/// NOT NULL`), compute it (`count(*) AS n`), or null it with a join, so its nullability is the
/// item's own result's.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemRead {
    /// One result column the items say nothing of: an expression, or a column of an item the
    /// reader cannot name.
    Other,
    /// One result column: a bare reference to a column of a table in the statement's own FROM,
    /// whose provenance is that column whatever set operations its WITH queries and subqueries
    /// hold.
    Table,
    /// One result column: a bare reference to the column of this name of item `item`.
    Column { item: usize, column: String },
    /// One result column: `coalesce(c, '[]')` of such a reference `c`, which is that column's
    /// array or an empty one, and never NULL.
    Defaulted { item: usize, column: String },
    /// `*` or `item.*`: every column of item `item` in order, or of something else (`None`).
    Star { item: Option<usize> },
}

/// A FROM item as a statement of its own: a WITH query read by name or a derived table, the
/// statement's WITH clause still in front of it, so that an earlier WITH query, or for a
/// recursive one its own name, resolves as it did in place; or a function in FROM, as `SELECT
/// f(…)`, whose one column is as nullable as the same call in a select list. A LATERAL item reads
/// columns no statement of its own has, and is none.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemQuery {
    pub sql: String,
    /// The column names the WITH clause or the item's alias gives it, `t(a, b)`; the rest keep the
    /// statement's own. A function's one column goes by the item's name.
    pub columns: Vec<String>,
    /// The WITH query's name, for one: whether an outer join nulls it is the caller's reading, by
    /// that name, which a strict WHERE can reduce.
    pub with: Option<String>,
    /// For any other item, whether it sits on the nullable side of an outer join.
    pub nulled: bool,
}

/// Per target of the select list, what it reads of the FROM items that are statements of their
/// own, and those items. `None` when the statement is not a plain SELECT that reads one, or when
/// a `(row).*` hides how many columns a target is.
pub fn item_reads(sql: &str) -> Option<(Vec<ItemRead>, Vec<ItemQuery>)> {
    let (reads, queries) = crate::stack::run(|| reads_here(sql))?;
    let read = |i: usize| reads.iter().any(|r| matches!(r, ItemRead::Column { item, .. } | ItemRead::Defaulted { item, .. } | ItemRead::Star { item: Some(item) } if *item == i));
    (0..queries.len()).any(read).then_some((reads, queries))
}

/// Per target of the select list, what it reads of the statement's FROM items, whether or not
/// any of them is a statement of its own. `None` where [`item_reads`] cannot tell.
pub fn target_reads(sql: &str) -> Option<Vec<ItemRead>> {
    crate::stack::run(|| reads_here(sql)).map(|(reads, _)| reads)
}

fn reads_here(sql: &str) -> Option<(Vec<ItemRead>, Vec<ItemQuery>)> {
    let parsed = crate::stack::parse(sql).ok()?;
    let version = parsed.protobuf.version;
    let top = parsed.protobuf.stmts.first().and_then(|s| s.stmt.as_deref()).and_then(|n| n.node.as_ref())?;
    let NodeEnum::SelectStmt(select) = top else { return None };
    if set_kind(select).is_some() || !select.values_lists.is_empty() {
        return None;
    }
    let mut items = Items { version, with: select.with_clause.as_ref(), names: Vec::new(), queries: Vec::new() };
    for item in &select.from_clause {
        items.read(item, false)?;
    }
    let strings = |fields: &[Node]| -> Option<Vec<String>> {
        fields.iter().map(|f| match f.node.as_ref() {
            Some(NodeEnum::String(s)) => Some(s.sval.clone()),
            _ => None,
        }).collect()
    };
    let mut reads = Vec::with_capacity(select.target_list.len());
    for target in &select.target_list {
        let Some(NodeEnum::ResTarget(rt)) = target.node.as_ref() else { return None };
        // A bare reference to a column of an item with a statement of its own.
        let column_of = |c: &protobuf::ColumnRef| match strings(&c.fields).as_deref() {
            Some([name, column]) => items.named(name).map(|item| (item, column.clone())),
            // A bare name says which item it reads only when there is one.
            Some([column]) => items.only().map(|item| (item, column.clone())),
            _ => None,
        };
        let read = match rt.val.as_deref()?.node.as_ref() {
            Some(NodeEnum::CoalesceExpr(c)) => match c.args.as_slice() {
                [first, default] if is_empty_array(default) => match first.node.as_ref() {
                    Some(NodeEnum::ColumnRef(c)) => column_of(c).map_or(ItemRead::Other, |(item, column)| ItemRead::Defaulted { item, column }),
                    _ => ItemRead::Other,
                },
                _ => ItemRead::Other,
            },
            // `(row).*` is as many columns as the row's type has.
            Some(NodeEnum::AIndirection(i)) if i.indirection.iter().any(|n| matches!(n.node, Some(NodeEnum::AStar(_)))) => return None,
            Some(NodeEnum::ColumnRef(c)) => match c.fields.split_last() {
                Some((last, qualifier)) if matches!(last.node, Some(NodeEnum::AStar(_))) => match strings(qualifier)?.as_slice() {
                    [] => ItemRead::Star { item: items.only() },
                    [name] => ItemRead::Star { item: items.named(name) },
                    _ => ItemRead::Star { item: None },
                },
                _ => {
                    // A bare name says which item it reads only when there is one.
                    let item = match strings(&c.fields).as_deref() {
                        Some([name, column]) => items.find(|n| n == Some(name.as_str())).map(|item| (item, column.clone())),
                        Some([column]) => items.find(|_| true).map(|item| (item, column.clone())),
                        _ => None,
                    };
                    match item {
                        Some((Item::Query(item), column)) => ItemRead::Column { item, column },
                        Some((Item::Table, _)) => ItemRead::Table,
                        _ => ItemRead::Other,
                    }
                }
            },
            _ => ItemRead::Other,
        };
        reads.push(read);
    }
    Some((reads, items.queries))
}

/// `'[]'`, as JSON text or cast to `json` or `jsonb`: the default that keeps an array an array, here
/// and in [`crate::jsonshape`].
pub(crate) fn is_empty_array(node: &Node) -> bool {
    match node.node.as_ref() {
        Some(NodeEnum::AConst(c)) => matches!(&c.val, Some(protobuf::a_const::Val::Sval(s)) if s.sval == "[]"),
        Some(NodeEnum::TypeCast(t)) => {
            let json = t.type_name.as_ref().and_then(|n| last_string(&n.names)).is_some_and(|n| n == "json" || n == "jsonb");
            json && t.arg.as_deref().is_some_and(is_empty_array)
        }
        _ => false,
    }
}

/// The FROM items of one SELECT, through its joins.
struct Items<'a> {
    version: i32,
    with: Option<&'a protobuf::WithClause>,
    /// The name each item goes by, and what it is.
    names: Vec<(Option<String>, Item)>,
    queries: Vec<ItemQuery>,
}

/// What one FROM item is to the reader.
#[derive(Debug, Clone, Copy)]
enum Item {
    /// A statement of its own, by its place in [`Items::queries`].
    Query(usize),
    /// A table, a view or anything else the catalog names, read by name and not a WITH query.
    Table,
    /// Anything else: a WITH query or a function with no statement of its own, a LATERAL item.
    Other,
}

impl Items<'_> {
    /// `None` for a FROM whose names the reader does not follow: a join with an alias of its own.
    fn read(&mut self, node: &Node, nulled: bool) -> Option<()> {
        let alias = |a: Option<&protobuf::Alias>| a.map(|a| a.aliasname.clone());
        let columns = |a: Option<&protobuf::Alias>| -> Vec<String> {
            a.iter().flat_map(|a| &a.colnames).filter_map(|n| match n.node.as_ref() {
                Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                _ => None,
            }).collect()
        };
        let (name, query) = match node.node.as_ref() {
            Some(NodeEnum::JoinExpr(j)) => {
                if j.alias.is_some() {
                    return None;
                }
                use protobuf::JoinType;
                let (left, right) = match j.jointype {
                    t if t == JoinType::JoinInner as i32 => (nulled, nulled),
                    t if t == JoinType::JoinLeft as i32 => (nulled, true),
                    t if t == JoinType::JoinRight as i32 => (true, nulled),
                    _ => (true, true),
                };
                for (side, nulled) in [(j.larg.as_deref(), left), (j.rarg.as_deref(), right)] {
                    self.read(side?, nulled)?;
                }
                return Some(());
            }
            Some(NodeEnum::RangeVar(rv)) => {
                // `public.pub` is the table, whatever the WITH clause calls `pub`.
                let cte = self.with.filter(|_| rv.schemaname.is_empty()).and_then(|w| {
                    w.ctes.iter().find_map(|c| match c.node.as_ref() {
                        Some(NodeEnum::CommonTableExpr(c)) if c.ctename == rv.relname => Some(c),
                        _ => None,
                    })
                });
                let query = cte.and_then(|cte| {
                    let body = cte.ctequery.as_deref().and_then(as_select)?;
                    let mut names = columns(rv.alias.as_ref());
                    let own: Vec<String> = cte.aliascolnames.iter().filter_map(|n| match n.node.as_ref() {
                        Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                        _ => None,
                    }).collect();
                    names.extend(own.into_iter().skip(names.len()));
                    Some(ItemQuery { sql: arm_text(self.version, body, self.with)?, columns: names, with: Some(cte.ctename.clone()), nulled })
                });
                let name = Some(alias(rv.alias.as_ref()).unwrap_or_else(|| rv.relname.clone()));
                if cte.is_none() {
                    self.names.push((name, Item::Table));
                    return Some(());
                }
                (name, query)
            }
            Some(NodeEnum::RangeSubselect(r)) => {
                let query = r.subquery.as_deref().and_then(as_select).filter(|_| !r.lateral).and_then(|body| {
                    Some(ItemQuery { sql: arm_text(self.version, body, self.with)?, columns: columns(r.alias.as_ref()), with: None, nulled })
                });
                (alias(r.alias.as_ref()), query)
            }
            Some(NodeEnum::RangeFunction(f)) => {
                // One function, as a value: `ROWS FROM (…)` has a column per function at least,
                // and a column definition list or WITH ORDINALITY more than the one.
                let call = match f.functions.as_slice() {
                    [only] if !f.ordinality && !f.is_rowsfrom && f.coldeflist.is_empty() => match only.node.as_ref() {
                        Some(NodeEnum::List(l)) if l.items.get(1).is_none_or(|defs| defs.node.is_none()) => l.items.first(),
                        _ => None,
                    },
                    _ => None,
                };
                let name = alias(f.alias.as_ref()).or_else(|| function_item_name(f));
                let query = call.zip(name.clone()).and_then(|(call, name)| {
                    let column = columns(f.alias.as_ref()).into_iter().next().unwrap_or(name);
                    Some(ItemQuery { sql: value_text(self.version, call, self.with)?, columns: vec![column], with: None, nulled })
                });
                (name, query)
            }
            Some(NodeEnum::RangeTableSample(t)) => match t.relation.as_deref().and_then(|r| r.node.as_ref()) {
                Some(NodeEnum::RangeVar(rv)) => (Some(alias(rv.alias.as_ref()).unwrap_or_else(|| rv.relname.clone())), None),
                _ => (None, None),
            },
            Some(NodeEnum::RangeTableFunc(f)) => (alias(f.alias.as_ref()), None),
            Some(NodeEnum::JsonTable(t)) => (alias(t.alias.as_ref()), None),
            _ => (None, None),
        };
        let item = match query {
            Some(q) => {
                self.queries.push(q);
                Item::Query(self.queries.len() - 1)
            }
            None => Item::Other,
        };
        self.names.push((name, item));
        Some(())
    }

    /// The one item whose name `matches`, when exactly one does.
    fn find(&self, matches: impl Fn(Option<&str>) -> bool) -> Option<Item> {
        let mut found = self.names.iter().filter(|(n, _)| matches(n.as_deref()));
        match (found.next(), found.next()) {
            (Some((_, item)), None) => Some(*item),
            _ => None,
        }
    }

    /// The statement of the one item of the FROM, when it has one item and that one has one.
    fn only(&self) -> Option<usize> {
        match self.find(|_| true) {
            Some(Item::Query(query)) => Some(query),
            _ => None,
        }
    }

    /// The statement of the item that goes by `name`, when one item does and it has one.
    fn named(&self, name: &str) -> Option<usize> {
        match self.find(|n| n == Some(name)) {
            Some(Item::Query(query)) => Some(query),
            _ => None,
        }
    }
}

/// The name a function item with no alias goes by, as the server picks it: the name of the first
/// function it calls, or the keyword of `CURRENT_DATE` and its kin. Any other expression has none
/// the proof reads.
pub(crate) fn function_item_name(f: &protobuf::RangeFunction) -> Option<String> {
    use protobuf::SqlValueFunctionOp as Op;
    let Some(NodeEnum::List(first)) = f.functions.first()?.node.as_ref() else { return None };
    match first.items.first()?.node.as_ref()? {
        NodeEnum::FuncCall(c) => last_string(&c.funcname),
        NodeEnum::SqlvalueFunction(v) => {
            let name = match Op::try_from(v.op).ok()? {
                Op::SvfopCurrentDate => "current_date",
                Op::SvfopCurrentTime | Op::SvfopCurrentTimeN => "current_time",
                Op::SvfopCurrentTimestamp | Op::SvfopCurrentTimestampN => "current_timestamp",
                Op::SvfopLocaltime | Op::SvfopLocaltimeN => "localtime",
                Op::SvfopLocaltimestamp | Op::SvfopLocaltimestampN => "localtimestamp",
                Op::SvfopCurrentRole => "current_role",
                Op::SvfopCurrentUser => "current_user",
                Op::SvfopUser => "user",
                Op::SvfopSessionUser => "session_user",
                Op::SvfopCurrentCatalog => "current_catalog",
                Op::SvfopCurrentSchema => "current_schema",
                Op::SqlvalueFunctionOpUndefined => return None,
            };
            Some(name.to_string())
        }
        _ => None,
    }
}

/// `SELECT <value>` as a statement of its own, the statement's `WITH` in front of it.
fn value_text(version: i32, value: &Node, with: Option<&protobuf::WithClause>) -> Option<String> {
    let target = Node { node: Some(NodeEnum::ResTarget(Box::new(protobuf::ResTarget { name: "value".into(), indirection: Vec::new(), val: Some(Box::new(value.clone())), location: -1 }))) };
    let select = SelectStmt {
        target_list: vec![target],
        with_clause: with.cloned(),
        op: protobuf::SetOperation::SetopNone as i32,
        limit_option: protobuf::LimitOption::Default as i32,
        ..Default::default()
    };
    let result = ParseResult {
        version,
        stmts: vec![RawStmt {
            stmt: Some(Box::new(Node { node: Some(NodeEnum::SelectStmt(Box::new(select))) })),
            stmt_location: 0,
            stmt_len: 0,
        }],
    };
    crate::stack::deparse(&result).ok()
}

/// What a SELECT says about a plain column reference in its own target list.
struct Scope {
    /// The plain tables of the FROM by the name each goes by; empty when an outer join sits
    /// anywhere in it, since a nulled table's whole row is NULL.
    tables: HashMap<String, String>,
    /// Every name a FROM item goes by, tables or not.
    names: Vec<String>,
    /// The column names a USING join merges, and whether a NATURAL join merges any.
    using: Vec<String>,
    natural: bool,
    /// The columns the WHERE tests strictly at its top level, as written.
    strict: Vec<Vec<String>>,
}

impl Scope {
    fn of(s: &SelectStmt, trust: &Trust) -> Scope {
        let mut tables = HashMap::new();
        let mut names = Vec::new();
        let mut outer_join = false;
        let (mut using, mut natural) = (Vec::new(), false);
        for item in &s.from_clause {
            from_names(item, &mut tables, &mut names, &mut outer_join, &mut using, &mut natural);
        }
        if outer_join {
            tables.clear();
        }
        let mut strict = Vec::new();
        if let Some(w) = s.where_clause.as_deref() {
            strict_columns(w, trust, &mut strict);
        }
        Scope { tables, names, using, natural, strict }
    }

    /// The claim for a target that is a bare column reference, if the SELECT proves one.
    fn column(&self, c: &protobuf::ColumnRef, node: &Node, reader: &mut Reader) -> Option<Expr> {
        let fields: Vec<String> = c.fields.iter().map(|f| match f.node.as_ref() {
            Some(NodeEnum::String(s)) => Some(s.sval.clone()),
            _ => None,
        }).collect::<Option<_>>()?;
        if self.strict.iter().any(|tested| self.same_column(tested, &fields)) {
            return Some(Expr::NonNull(None));
        }
        let [name] = fields.as_slice() else { return None };
        let relation = self.tables.get(name)?;
        let leaves = reader.leaves.as_mut()?;
        leaves.push(node.clone());
        Some(Expr::Row(leaves.len() - 1, relation.clone()))
    }

    /// Whether a column the WHERE tests, `tested`, is the column `read`: the same text, or one of
    /// them bare and the other qualified by a FROM item of this SELECT. A bare name that resolves
    /// at all resolves to exactly one FROM item's column, so that is the one, except a name a
    /// USING or NATURAL join merges: the merged column is the left side's under a LEFT JOIN and
    /// `coalesce` of both under a FULL one, so a test of it proves nothing of a side's own. A test
    /// of a side's column does prove the merged one, which is NULL only where both sides are.
    fn same_column(&self, tested: &[String], read: &[String]) -> bool {
        if tested == read {
            return true;
        }
        let (bare, qualified) = match (tested.len(), read.len()) {
            (1, 2) if self.natural || self.using.contains(&tested[0]) => return false,
            (1, 2) => (tested, read),
            (2, 1) => (read, tested),
            _ => return false,
        };
        bare[0] == qualified[1] && self.names.contains(&qualified[0])
    }
}

/// The names a FROM item's relations go by, through its joins, and the columns its joins merge.
fn from_names(item: &Node, tables: &mut HashMap<String, String>, names: &mut Vec<String>, outer_join: &mut bool, using: &mut Vec<String>, natural: &mut bool) {
    match item.node.as_ref() {
        Some(NodeEnum::RangeVar(rv)) => {
            let name = rv.alias.as_ref().map(|a| a.aliasname.clone()).unwrap_or_else(|| rv.relname.clone());
            tables.insert(name.clone(), rv.relname.clone());
            names.push(name);
        }
        Some(NodeEnum::JoinExpr(j)) => {
            if j.jointype != protobuf::JoinType::JoinInner as i32 {
                *outer_join = true;
            }
            *natural |= j.is_natural;
            using.extend(j.using_clause.iter().filter_map(|n| match n.node.as_ref() {
                Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                _ => None,
            }));
            for side in [j.larg.as_deref(), j.rarg.as_deref()].into_iter().flatten() {
                from_names(side, tables, names, outer_join, using, natural);
            }
        }
        Some(NodeEnum::RangeSubselect(r)) => names.extend(r.alias.as_ref().map(|a| a.aliasname.clone())),
        Some(NodeEnum::RangeFunction(r)) => names.extend(r.alias.as_ref().map(|a| a.aliasname.clone())),
        _ => {}
    }
}

/// The column references a WHERE tests strictly at its top level ([`strict_in`]), where every
/// operator and function the test rests on is the built-in one. A row the SELECT returns passed
/// the test, so in its result that column is not NULL.
fn strict_columns(node: &Node, trust: &Trust, out: &mut Vec<Vec<String>>) {
    for strict in strict_in(node) {
        if strict.names.iter().all(|n| trust.holds(n)) && !out.contains(&strict.column) {
            out.push(strict.column);
        }
    }
}

/// A column a predicate cannot pass with NULL in it, and the names of the built-in operators and
/// functions that claim rests on: a user's operator or function of one of those names may hold
/// for NULL, so a caller drops the claim where the trust check shadows a name.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Strict {
    pub column: Vec<String>,
    pub names: Vec<String>,
}

/// The columns a predicate at the top level of a WHERE, or of an inner join's ON clause, is strict
/// in: a row with NULL in one of them does not pass. Under AND, either side's; under OR, the ones
/// every arm is strict in (`nickname like 'a%' or nickname like 'b%'`); `IS NOT NULL`; NOT over a
/// test strict in it, or over `IS NULL`; a comparison, `IN`, `BETWEEN`, `LIKE`, `ILIKE` or
/// `SIMILAR TO`, in the columns its operands answer NULL for ([`nulls_with`]), which reach
/// through strict functions and operators (`lower(nickname) = $1`); a bare boolean expression.
/// The planner's `find_nonnullable_vars` reads the same shapes.
pub(crate) fn strict_in(node: &Node) -> Vec<Strict> {
    let named = |mut found: Vec<Strict>, name: &str| {
        for f in &mut found {
            f.names.push(name.to_string());
        }
        found
    };
    match node.node.as_ref() {
        Some(NodeEnum::BoolExpr(b)) if b.boolop == protobuf::BoolExprType::AndExpr as i32 => b.args.iter().flat_map(strict_in).collect(),
        Some(NodeEnum::BoolExpr(b)) if b.boolop == protobuf::BoolExprType::OrExpr as i32 => {
            let mut arms = b.args.iter().map(strict_in);
            let Some(first) = arms.next() else { return Vec::new() };
            arms.fold(first, |kept, arm| {
                kept.into_iter()
                    .filter_map(|mut k| {
                        let other = arm.iter().find(|a| a.column == k.column)?;
                        k.names.extend(other.names.iter().cloned());
                        Some(k)
                    })
                    .collect()
            })
        }
        Some(NodeEnum::BoolExpr(b)) if b.boolop == protobuf::BoolExprType::NotExpr as i32 => match b.args.first().and_then(|a| a.node.as_ref()) {
            Some(NodeEnum::NullTest(t)) if t.nulltesttype == protobuf::NullTestType::IsNull as i32 => t.arg.as_deref().map(nulls_with).unwrap_or_default(),
            Some(_) => b.args.first().map(test_nulls_with).unwrap_or_default(),
            None => Vec::new(),
        },
        Some(NodeEnum::NullTest(t)) if t.nulltesttype == protobuf::NullTestType::IsNotNull as i32 => t.arg.as_deref().map(nulls_with).unwrap_or_default(),
        Some(NodeEnum::AExpr(e)) => {
            let kind = e.kind;
            let op = builtin_name(&e.name).unwrap_or_default();
            if kind == protobuf::AExprKind::AexprIn as i32 {
                // `x IN (…)` and `x NOT IN (…)` are NULL for a NULL x.
                e.lexpr.as_deref().map(|l| named(nulls_with(l), &op)).unwrap_or_default()
            } else if [protobuf::AExprKind::AexprBetween as i32, protobuf::AExprKind::AexprNotBetween as i32].contains(&kind) {
                e.lexpr.as_deref().map(|l| named(named(nulls_with(l), ">="), "<=")).unwrap_or_default()
            } else if [protobuf::AExprKind::AexprLike as i32, protobuf::AExprKind::AexprIlike as i32, protobuf::AExprKind::AexprSimilar as i32].contains(&kind) {
                let mut found: Vec<Strict> = e.lexpr.as_deref().map(nulls_with).unwrap_or_default();
                if kind != protobuf::AExprKind::AexprSimilar as i32 {
                    found.extend(e.rexpr.as_deref().map(nulls_with).unwrap_or_default());
                }
                named(found, &op)
            } else {
                test_nulls_with(node)
            }
        }
        _ => test_nulls_with(node),
    }
}

/// A test that is NULL wherever its operands answer NULL: a strict comparison, or any expression
/// [`nulls_with`] reads, taken as a boolean (`WHERE active`).
fn test_nulls_with(node: &Node) -> Vec<Strict> {
    nulls_with(node)
}

/// The columns an expression answers NULL for: the column itself, and through a built-in strict
/// operator ([`POSTGRES_STRICT_OPERATORS`]), a strict function ([`POSTGRES_STRICT`]) or a cast to
/// a built-in type, every column an operand answers NULL for.
fn nulls_with(node: &Node) -> Vec<Strict> {
    match node.node.as_ref() {
        Some(NodeEnum::ColumnRef(c)) => {
            let fields: Option<Vec<String>> = c.fields.iter().map(|f| match f.node.as_ref() {
                Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                _ => None,
            }).collect();
            fields.map(|column| vec![Strict { column, names: Vec::new() }]).unwrap_or_default()
        }
        Some(NodeEnum::AExpr(e)) if e.kind == protobuf::AExprKind::AexprOp as i32 => {
            let Some(op) = builtin_name(&e.name).filter(|o| POSTGRES_STRICT_OPERATORS.contains(&o.as_str())) else { return Vec::new() };
            let mut found: Vec<Strict> = e.lexpr.iter().chain(e.rexpr.iter()).flat_map(|o| nulls_with(o)).collect();
            for f in &mut found {
                f.names.push(op.clone());
            }
            found
        }
        Some(NodeEnum::FuncCall(f)) if f.over.is_none() && f.agg_filter.is_none() && !f.agg_star && !f.agg_distinct && !f.func_variadic && f.agg_order.is_empty() => {
            let Some(name) = builtin_name(&f.funcname).filter(|n| POSTGRES_STRICT.contains(&n.as_str())) else { return Vec::new() };
            let mut found: Vec<Strict> = f.args.iter().flat_map(nulls_with).collect();
            for s in &mut found {
                s.names.push(name.clone());
            }
            found
        }
        Some(NodeEnum::TypeCast(t)) => {
            let target = t.type_name.as_ref().filter(|n| n.array_bounds.is_empty() && !n.setof && !n.pct_type).and_then(|n| builtin_name(&n.names));
            match target.filter(|n| POSTGRES_CASTS.iter().any(|(c, _)| c == n)) {
                Some(target) => {
                    let mut found = t.arg.as_deref().map(nulls_with).unwrap_or_default();
                    for f in &mut found {
                        f.names.push(format!("::{target}"));
                    }
                    found
                }
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// A bare `VALUES (…), (…)` arm: a column is non-null when it is in every row.
fn values_plan(s: &SelectStmt, dialect: Dialect, trust: &Trust, columns: &Columns) -> Plan {
    let withs = Withs::default().with(s.with_clause.as_ref(), columns);
    let mut reader = Reader { dialect, trust, columns, withs, rows: None, leaves: None, grouped: false, narrowed: Vec::new() };
    let mut rows: Vec<Vec<Expr>> = Vec::new();
    for row in &s.values_lists {
        let Some(NodeEnum::List(list)) = row.node.as_ref() else { return Plan::default() };
        rows.push(list.items.iter().map(|v| reader.read(v)).collect());
    }
    let width = rows.first().map_or(0, Vec::len);
    if rows.iter().any(|r| r.len() != width) {
        return Plan::default();
    }
    let head = (0..width).map(|i| Some(Expr::Every(rows.iter().map(|r| r[i].clone()).collect(), Ty::Shared))).collect();
    Plan { head, ..Plan::default() }
}

fn set_kind(s: &SelectStmt) -> Option<SetKind> {
    match s.op {
        op if op == protobuf::SetOperation::SetopUnion as i32 => Some(SetKind::Union),
        op if op == protobuf::SetOperation::SetopIntersect as i32 => Some(SetKind::Intersect),
        op if op == protobuf::SetOperation::SetopExcept as i32 => Some(SetKind::Except),
        _ => None,
    }
}

/// One arm of a set operation as a statement of its own, carrying the statement's `WITH` so an
/// arm reading a CTE still resolves.
fn arm_text(version: i32, arm: &SelectStmt, with: Option<&protobuf::WithClause>) -> Option<String> {
    let mut arm = arm.clone();
    if arm.with_clause.is_none() {
        arm.with_clause = with.cloned();
    }
    let result = ParseResult {
        version,
        stmts: vec![RawStmt {
            stmt: Some(Box::new(Node { node: Some(NodeEnum::SelectStmt(Box::new(arm))) })),
            stmt_location: 0,
            stmt_len: 0,
        }],
    };
    crate::stack::deparse(&result).ok()
}

/// The unqualified name of an operator, function, or type the rules read, or `None` when it is
/// schema-qualified with anything but Postgres's own catalog: a qualified name is somebody else's.
/// `CAST(x AS INTEGER)` parses as `pg_catalog.int4` on SQLite too, which is the parser's
/// spelling of a keyword, not a schema SQLite has.
pub(crate) fn builtin_name(names: &[Node]) -> Option<String> {
    let name = last_string(names)?;
    match names.len() {
        1 => Some(name),
        2 if last_string(&names[..1]).as_deref() == Some("pg_catalog") => Some(name),
        _ => None,
    }
}

fn is_star(node: &Node) -> bool {
    match node.node.as_ref() {
        Some(NodeEnum::ColumnRef(c)) => c.fields.last().is_some_and(|f| matches!(f.node.as_ref(), Some(NodeEnum::AStar(_)))),
        _ => false,
    }
}

fn reads_leaf(expr: &Expr) -> bool {
    match expr {
        Expr::Leaf(_) | Expr::Row(..) => true,
        Expr::Nullable(inner) => reads_leaf(inner),
        Expr::Every(operands, _) | Expr::Any(operands, _) => operands.iter().any(reads_leaf),
        Expr::Guarded(inner, _) | Expr::NotJsonb(inner) | Expr::Narrowed(inner) => reads_leaf(inner),
        Expr::Array { items, .. } => items.iter().any(reads_leaf),
        Expr::NonNull(_) | Expr::Unknown | Expr::Unread => false,
    }
}

/// SQLite's `?n` as `$n`, outside strings, quoted identifiers, and comments. The build's rewrite
/// only ever writes the numbered form.
pub fn dollar_placeholders(sql: &str) -> String {
    let b = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    let mut copied = 0;
    while i < b.len() {
        match b[i] {
            b'\'' | b'"' | b'`' | b'[' => {
                let close = if b[i] == b'[' { b']' } else { b[i] };
                i += 1;
                while i < b.len() && b[i] != close {
                    i += 1;
                }
                i += 1;
            }
            b'-' if b.get(i + 1) == Some(&b'-') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i += 2;
            }
            b'?' if b.get(i + 1).is_some_and(u8::is_ascii_digit) => {
                out.push_str(&sql[copied..i]);
                out.push('$');
                i += 1;
                copied = i;
            }
            _ => i += 1,
        }
    }
    out.push_str(&sql[copied.min(sql.len())..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plan over a catalog of the tables the tests name in a scalar subquery.
    fn plan(sql: &str, dialect: Dialect, trust: &Trust) -> Plan {
        let columns: Columns = [("todos", vec!["id", "owner_id", "title", "n"]), ("owners", vec!["id", "name"])].into_iter().collect();
        super::plan(sql, dialect, trust, &columns)
    }

    fn pg(sql: &str) -> Plan {
        plan(sql, Dialect::Postgres, &Trust::default())
    }

    fn lite(sql: &str) -> Plan {
        plan(sql, Dialect::Sqlite, &Trust::default())
    }

    /// The first column's answer with no leaf probe.
    fn first(p: &Plan) -> Fact {
        resolve(p.head[0].as_ref().expect("a claim"), None)
    }

    fn col(nullable: bool, type_oid: u32) -> DescribedColumn {
        DescribedColumn { name: "a".into(), type_oid, type_name: String::new(), nullable, nullable_items: None, values: None, document: None, whole_row: false }
    }

    #[test]
    fn a_count_is_never_null_and_other_aggregates_are() {
        assert_eq!(first(&pg("SELECT count(*) AS n FROM todos")), Fact { non_null: true, type_oid: Some(oid::INT8) });
        assert!(first(&pg("SELECT count(t.id) FROM todos t GROUP BY t.owner_id")).non_null);
        assert!(first(&pg("SELECT count(DISTINCT t.id) FILTER (WHERE t.done) FROM todos t")).non_null);
        assert!(first(&pg("SELECT count(*) OVER (ORDER BY id ROWS BETWEEN 5 FOLLOWING AND 6 FOLLOWING) FROM todos")).non_null);
        // Every other aggregate is NULL over no rows.
        for sql in ["SELECT sum(n) FROM t", "SELECT max(n) FROM t", "SELECT min(n) FROM t", "SELECT avg(n) FROM t", "SELECT json_agg(n) FROM t"] {
            assert!(!first(&pg(sql)).non_null, "{sql}");
        }
        // A count cast keeps the count's answer.
        assert!(first(&pg("SELECT CAST(count(*) AS integer) AS total FROM todos")).non_null);
        assert!(first(&pg("SELECT count(*)::int FROM todos")).non_null);
    }

    #[test]
    fn literals_are_non_null_and_null_is_not() {
        assert!(first(&pg("SELECT 'ok'")).non_null);
        assert!(first(&pg("SELECT 1")).non_null);
        assert!(first(&pg("SELECT true")).non_null);
        assert!(!first(&pg("SELECT NULL")).non_null);
        assert!(!first(&pg("SELECT NULL::int")).non_null);
        assert_eq!(first(&lite("SELECT 'ok'")).type_oid, Some(oid::TEXT));
        assert_eq!(first(&lite("SELECT 1")).type_oid, Some(oid::INT4));
        assert_eq!(first(&lite("SELECT 9999999999")).type_oid, Some(oid::INT8));
        assert_eq!(first(&lite("SELECT 1.5")).type_oid, Some(oid::FLOAT8));
        assert_eq!(first(&lite("SELECT true")).type_oid, Some(oid::BOOL));
    }

    #[test]
    fn coalesce_needs_one_non_null_argument_and_case_needs_every_branch() {
        assert!(first(&pg("SELECT coalesce(NULL, 0)")).non_null);
        assert!(!first(&pg("SELECT coalesce(NULL, NULL::int)")).non_null);
        assert!(first(&pg("SELECT CASE WHEN true THEN 1 ELSE 2 END")).non_null);
        assert!(!first(&pg("SELECT CASE WHEN true THEN 1 ELSE NULL END")).non_null);
        // No `ELSE` is `ELSE NULL`.
        assert!(!first(&pg("SELECT CASE WHEN true THEN 1 END")).non_null);
        assert!(!first(&pg("SELECT CASE 1 WHEN 1 THEN 'a' END")).non_null);
    }

    #[test]
    fn greatest_and_least_need_one_non_null_argument_on_postgres_alone() {
        assert!(first(&pg("SELECT greatest(NULL, 0)")).non_null);
        assert!(first(&pg("SELECT least(1, NULL, 2)")).non_null);
        assert!(!first(&pg("SELECT greatest(NULL::int, NULL)")).non_null);
        // MySQL's are NULL where any argument is.
        let mysql = plan("SELECT greatest(NULL, 0)", Dialect::Mysql, &Trust::default());
        assert!(mysql.head[0].as_ref().is_none_or(|e| !resolve(e, None).non_null));
    }

    #[test]
    fn tests_and_exists_are_never_null_and_nullif_is_not_an_operator() {
        assert!(first(&pg("SELECT EXISTS (SELECT 1 FROM todos)")).non_null);
        assert!(first(&pg("SELECT NULL IS NULL")).non_null);
        assert!(first(&pg("SELECT NULL::bool IS NOT TRUE")).non_null);
        assert!(first(&pg("SELECT NULL IS DISTINCT FROM 1")).non_null);
        assert!(!first(&pg("SELECT nullif(1, 1)")).non_null);
        assert!(!first(&pg("SELECT 1 IN (1, NULL)")).non_null);
        // LIKE, ILIKE, SIMILAR TO and the regular expressions are operators the parse marks apart.
        assert!(first(&pg("SELECT 'a' LIKE 'b'")).non_null);
        assert!(first(&pg("SELECT 'a' NOT ILIKE 'b'")).non_null);
        assert!(first(&pg("SELECT 'a' SIMILAR TO 'b'")).non_null);
        assert!(first(&pg("SELECT 'a' LIKE 'b!%' ESCAPE '!'")).non_null);
        assert!(!first(&pg("SELECT NULL::text LIKE 'b'")).non_null);
        // The prefix operators of the set, and `#`, whose geometric overloads are NULL where
        // nothing intersects, left out.
        assert!(first(&pg("SELECT |/ 4.0::float8")).non_null);
        assert!(first(&pg("SELECT ~ 5")).non_null);
        assert!(!first(&pg("SELECT 5 # 3")).non_null);
        assert!(!first(&pg("SELECT '{\"a\": 1}'::jsonb ->> 'b'")).non_null);
    }

    #[test]
    fn operators_are_non_null_on_non_null_operands() {
        assert!(first(&pg("SELECT 1 + 2 * 3")).non_null);
        assert!(first(&pg("SELECT 'a' || 'b'")).non_null);
        assert!(first(&pg("SELECT 1 <> 2 AND NOT false")).non_null);
        assert!(first(&pg("SELECT -1")).non_null);
        assert!(!first(&pg("SELECT 1 + NULL")).non_null);
        // Only built-in names: a qualified operator is somebody else's.
        assert!(!first(&pg("SELECT 1 OPERATOR(myschema.+) 2")).non_null);
        assert!(first(&pg("SELECT 1 OPERATOR(pg_catalog.+) 2")).non_null);
        // `!=` parses as `<>`.
        assert!(first(&pg("SELECT 1 != 2")).non_null);
    }

    #[test]
    fn a_function_total_for_some_overloads_needs_its_first_arguments_type() {
        const INT4RANGE: u32 = 3904;
        let one = |sql: &str, leaves: &[DescribedColumn]| resolve(pg(sql).head[0].as_ref().expect("a claim"), Some(leaves)).non_null;
        // `lower` and `upper` of an empty or unbounded range are NULL; of a string they never are.
        assert!(one("SELECT lower(a) FROM t", &[col(false, oid::TEXT)]));
        assert!(one("SELECT upper(a) FROM t", &[col(false, oid::VARCHAR)]));
        assert!(!one("SELECT lower(a) FROM t", &[col(false, INT4RANGE)]));
        assert!(!one("SELECT upper(a) FROM t", &[col(false, oid::UNKNOWN)]), "a type the build cannot name proves nothing");
        assert!(!one("SELECT lower(a) FROM t", &[col(true, oid::TEXT)]));
        // A cast and a concatenation of strings name their type.
        assert!(one("SELECT lower(a::text) FROM t", &[col(false, INT4RANGE)]));
        assert!(one("SELECT lower(a || '!') FROM t", &[col(false, oid::TEXT)]));
        assert!(!one("SELECT lower(a || b) FROM t", &[col(false, oid::TEXT), col(false, oid::JSONB)]));
        // `to_char` of a number never answers NULL, of a timestamp or an interval it can.
        assert!(one("SELECT to_char(a, '999') FROM t", &[col(false, oid::NUMERIC)]));
        assert!(!one("SELECT to_char(a, 'YYYY') FROM t", &[col(false, oid::TIMESTAMP)]));
        // `date_part` and `extract` of 'infinity' are NULL for every field.
        assert!(!one("SELECT date_part('day', a) FROM t", &[col(false, oid::TIMESTAMP)]));
        assert!(!one("SELECT extract(month FROM a) FROM t", &[col(false, oid::DATE)]));
        // `concat_ws` skips a NULL value and answers NULL for a NULL separator.
        assert!(one("SELECT concat_ws(',', a, 'x') FROM t", &[col(true, oid::TEXT)]));
        assert!(!one("SELECT concat_ws(a, 'x') FROM t", &[col(true, oid::TEXT)]));
        assert!(one("SELECT concat_ws(a, 'x') FROM t", &[col(false, oid::TEXT)]));
        // `concat(VARIADIC NULL::text[])` is NULL.
        assert!(!one("SELECT concat(VARIADIC a) FROM t", &[col(false, oid::TEXT)]));
    }

    #[test]
    fn an_arrays_elements_are_nullable_only_where_the_statement_builds_it_from_a_nullable_value() {
        let claim = |sql: &str| pg(sql).head[0].clone().expect("a claim");
        let is_array = |t: u32| t == 1009;
        let items = |sql: &str, leaves: &[DescribedColumn]| items_nullable(&claim(sql), Some(leaves), &is_array);
        assert_eq!(items("SELECT ARRAY[a, 'x'] FROM t", &[col(false, oid::TEXT)]), Some(false));
        assert_eq!(items("SELECT ARRAY[a, 'x'] FROM t", &[col(true, oid::TEXT)]), Some(true));
        assert_eq!(items("SELECT array_agg(a) FROM t GROUP BY b", &[col(false, oid::TEXT)]), Some(false));
        assert_eq!(items("SELECT array_agg(a) FROM t GROUP BY b", &[col(true, oid::TEXT)]), Some(true));
        // With no group the array may be NULL, and its elements are still the argument's values.
        assert_eq!(items("SELECT array_agg(a) FROM t", &[col(false, oid::TEXT)]), Some(false));
        assert!(!resolve(&claim("SELECT array_agg(a) FROM t"), Some(&[col(false, oid::TEXT)])).non_null);
        assert_eq!(items("SELECT coalesce(array_agg(a), '{}') FROM t", &[col(false, oid::TEXT)]), Some(false));
        assert_eq!(items("SELECT coalesce(array_agg(a), '{NULL}') FROM t", &[col(false, oid::TEXT)]), Some(true));
        // A nested constructor's elements are the result's; an array element's are not known.
        assert_eq!(items("SELECT ARRAY[ARRAY[a], ARRAY['x']] FROM t", &[col(true, oid::TEXT)]), Some(true));
        assert_eq!(items("SELECT array_agg(a) FROM t", &[col(false, 1009)]), None);
        // A column's array, an operator over one and ARRAY(SELECT …) are not built here, and are T[].
        assert_eq!(items("SELECT tags || ARRAY['x'] FROM t", &[col(false, 1009)]), None);
        assert_eq!(items("SELECT ARRAY(SELECT a FROM t) AS x", &[]), None);
        // Nor is an array built inside a scalar subquery, whose columns the leaf probe over the
        // outer FROM cannot read: `p.id` is a key, and its elements are T[] as a column's are.
        assert_eq!(items("SELECT (SELECT array_agg(p.id) FROM posts p WHERE p.user_id = u.id) AS x FROM users u", &[]), None);
        assert_eq!(items("SELECT (SELECT ARRAY[p.id, NULL] FROM posts p LIMIT 1) AS x FROM users u", &[]), Some(true));
        // The literal is still text where it stands for text.
        assert_eq!(resolve(&claim("SELECT '{x}' AS a"), None), Fact { non_null: true, type_oid: Some(oid::TEXT) });
        // A cast keeps its operand's elements, but where it takes a JSON null to NULL: on
        // PostgreSQL 18 `ARRAY['null'::jsonb]::int[]` is `{NULL}`.
        assert_eq!(items("SELECT ARRAY[1]::text[]", &[]), Some(false));
        assert_eq!(items("SELECT ARRAY[1, 2]::int8[]", &[]), Some(false));
        assert_eq!(items("SELECT ARRAY[]::int8[]", &[]), Some(false));
        assert_eq!(items("SELECT ARRAY[NULL::int]::int8[]", &[]), Some(true));
        assert_eq!(items("SELECT ARRAY['null'::jsonb]::int[]", &[]), Some(true));
        assert_eq!(items("SELECT ARRAY[ARRAY['1'::jsonb]]::int[]", &[]), Some(true));
        assert_eq!(items("SELECT ARRAY['null'::jsonb]::text[]", &[]), Some(false));
        assert_eq!(items("SELECT array_agg(a)::int[] FROM t GROUP BY b", &[col(false, oid::JSONB)]), Some(true));
        assert_eq!(items("SELECT array_agg(a)::int[] FROM t GROUP BY b", &[col(false, oid::INT4)]), Some(false));
        assert!(resolve(&claim("SELECT ARRAY['null'::jsonb]::int[]"), None).non_null);
        // Before PostgreSQL 18 that cast raises an error instead.
        let raises = Trust { jsonb_casts_raise: true, ..Trust::default() };
        let before_18 = items_nullable(&plan("SELECT ARRAY['1'::jsonb]::int[]", Dialect::Postgres, &raises).head[0].clone().expect("a claim"), Some(&[]), &is_array);
        assert_eq!(before_18, Some(false));
    }

    #[test]
    fn a_where_narrows_through_strict_functions_or_arms_in_between_and_like() {
        let narrowed = |sql: &str| pg(sql).head[0].as_ref().is_some_and(|e| resolve(e, None).non_null);
        // A name USING merges is the left side's column under a LEFT JOIN, the right side's under
        // a RIGHT one: a test of it proves nothing of the other side's. A side's test proves it.
        assert!(!narrowed("SELECT b.x FROM a LEFT JOIN b USING (x) WHERE x > 0"));
        assert!(!narrowed("SELECT a.x FROM a RIGHT JOIN b USING (x) WHERE x > 0"));
        assert!(!narrowed("SELECT b.x FROM a NATURAL LEFT JOIN b WHERE x > 0"));
        assert!(narrowed("SELECT x FROM a LEFT JOIN b USING (x) WHERE b.x > 0"));
        assert!(narrowed("SELECT x FROM a FULL JOIN b USING (x) WHERE x > 0"));
        for sql in [
            "SELECT nickname FROM users WHERE lower(nickname) = $1",
            "SELECT nickname FROM users WHERE nickname LIKE 'a%' OR nickname LIKE 'b%'",
            "SELECT nickname FROM users WHERE nickname IN ('a', 'b')",
            "SELECT nickname FROM users WHERE nickname NOT IN ('a', 'b')",
            "SELECT age FROM users WHERE age BETWEEN 1 AND 9",
            "SELECT nickname FROM users WHERE nickname ILIKE 'a%'",
            "SELECT nickname FROM users WHERE NOT nickname IS NULL",
            "SELECT nickname FROM users WHERE length(nickname)::int8 > 3",
            "SELECT active FROM users WHERE active",
        ] {
            assert!(narrowed(sql), "{sql}");
        }
        for sql in [
            "SELECT nickname FROM users WHERE nickname = 'a' OR id = 1",
            "SELECT nickname FROM users WHERE coalesce(nickname, '') = ''",
            "SELECT nickname FROM users WHERE nickname IS NULL OR nickname = 'a'",
            "SELECT nickname FROM users WHERE concat(nickname, 'x') = 'x'",
            "SELECT nickname FROM users WHERE (nickname || 'x') = 'x'",
        ] {
            assert!(!narrowed(sql), "{sql}");
        }
        // A user's `lower` may hold for NULL.
        let shadowed = Trust { shadowed: ["lower".to_string()].into_iter().collect(), ..Trust::default() };
        let p = plan("SELECT nickname FROM users WHERE lower(nickname) = $1", Dialect::Postgres, &shadowed);
        assert!(!p.head[0].as_ref().is_some_and(|e| resolve(e, None).non_null));
    }

    #[test]
    fn a_case_arm_reads_what_its_when_proves() {
        let leaves = [col(true, oid::TEXT)];
        let one = |sql: &str| resolve(&pg(sql).head[0].clone().expect("a claim"), Some(&leaves)).non_null;
        assert!(one("SELECT CASE WHEN nickname IS NULL THEN 'none' ELSE nickname END FROM users"));
        assert!(one("SELECT CASE WHEN nickname IS NULL THEN 'none' WHEN nickname = '' THEN 'empty' ELSE nickname END FROM users"));
        assert!(one("SELECT CASE WHEN nickname LIKE 'a%' THEN nickname ELSE 'other' END FROM users"));
        assert!(one("SELECT CASE WHEN lower(nickname) = 'x' THEN upper(nickname) ELSE 'other' END FROM users"));
        // The arm before the test, a test that is not IS NULL, and a simple CASE prove nothing.
        assert!(!one("SELECT CASE WHEN nickname IS NOT NULL THEN 'some' ELSE nickname END FROM users"));
        assert!(!one("SELECT CASE WHEN nickname = 'a' THEN 'a' ELSE nickname END FROM users"));
        assert!(!one("SELECT CASE nickname WHEN 'a' THEN nickname ELSE 'b' END FROM users"));
        assert!(!one("SELECT CASE WHEN nickname IS NULL OR id = 1 THEN 'x' ELSE nickname END FROM users"));
        // A window function reads other rows than the one the WHEN tests: `lag` the previous one.
        assert!(!one("SELECT CASE WHEN nickname IS NOT NULL THEN lag(nickname, 1, '') OVER (ORDER BY id) ELSE '' END FROM users"));
        let is_array = |_: u32| false;
        let items = |sql: &str| items_nullable(&pg(sql).head[0].clone().expect("a claim"), Some(&leaves), &is_array);
        assert_eq!(items("SELECT CASE WHEN nickname IS NOT NULL THEN array_agg(nickname) OVER (ORDER BY id) END FROM users"), Some(true));
        assert_eq!(items("SELECT CASE WHEN nickname IS NOT NULL THEN ARRAY[nickname] END FROM users"), Some(false));
    }

    #[test]
    fn an_aggregates_filter_narrows_what_it_aggregates() {
        let is_array = |_: u32| false;
        let items = |sql: &str| items_nullable(&pg(sql).head[0].clone().expect("a claim"), Some(&[col(true, oid::TEXT)]), &is_array);
        assert_eq!(items("SELECT array_agg(nickname) FILTER (WHERE nickname IS NOT NULL) FROM users"), Some(false));
        assert_eq!(items("SELECT array_agg(nickname) FILTER (WHERE id > 1) FROM users"), Some(true));
        assert_eq!(items("SELECT array_agg(nickname) FROM users"), Some(true));
    }

    #[test]
    fn a_where_narrows_a_leaf_through_its_probe() {
        let p = pg("SELECT lower(nickname) FROM users WHERE nickname IS NOT NULL AND id = $1");
        assert_eq!(p.leaves.as_deref(), Some("SELECT nickname AS a0 FROM users WHERE nickname IS NOT NULL AND id = NULL"));
    }

    #[test]
    fn ranking_window_functions_are_never_null() {
        let one = |sql: &str| resolve(&pg(sql).head[0].clone().expect("a claim"), Some(&[col(false, oid::TEXT), col(false, oid::INT4)])).non_null;
        assert!(one("SELECT row_number() OVER () FROM t"));
        assert!(one("SELECT rank() OVER (ORDER BY a) FROM t"));
        assert!(one("SELECT cume_dist() OVER w FROM t WINDOW w AS (ORDER BY a)"));
        assert!(one("SELECT lag(a, 1, 'none') OVER () FROM t"));
        assert!(!one("SELECT lag(a) OVER () FROM t"));
        assert!(!one("SELECT lead(a, 1) OVER () FROM t"));
        assert!(one("SELECT ntile(4) OVER () FROM t"));
        assert!(!one("SELECT first_value(a) OVER () FROM t"));
    }

    #[test]
    fn sqlite_has_no_total_arithmetic() {
        // `1/0` and `1e999 - 1e999` are both NULL on SQLite.
        assert!(!first(&lite("SELECT 1 / 0")).non_null);
        assert!(!first(&lite("SELECT 1 + 2")).non_null);
        assert_eq!(first(&lite("SELECT 'a' || 1")), Fact { non_null: true, type_oid: Some(oid::TEXT) });
        assert_eq!(first(&lite("SELECT 1 < 2")), Fact { non_null: true, type_oid: Some(oid::BOOL) });
        // `total()` is a float that a NaN makes NULL; `sum` has no type, since a column declared
        // INTEGER may still hold a real.
        assert_eq!(first(&lite("SELECT total(n) FROM t")), Fact { non_null: false, type_oid: Some(oid::FLOAT8) });
        assert_eq!(first(&lite("SELECT sum(n) FROM t")), Fact { non_null: false, type_oid: None });
        assert_eq!(first(&lite("SELECT count(*) AS total FROM todos WHERE owner_id = ?1")), Fact { non_null: true, type_oid: Some(oid::INT8) });
        assert_eq!(first(&lite("SELECT ifnull(NULL, 'x')")), Fact { non_null: true, type_oid: None });
    }

    #[test]
    fn a_cast_carries_its_argument_and_on_sqlite_its_target() {
        assert_eq!(first(&lite("SELECT CAST(count(*) AS INTEGER) AS n FROM t")), Fact { non_null: true, type_oid: Some(oid::INT4) });
        assert_eq!(first(&lite("SELECT CAST(1 AS REAL)")), Fact { non_null: true, type_oid: Some(oid::FLOAT8) });
        assert_eq!(first(&lite("SELECT CAST(1 AS TEXT)")), Fact { non_null: true, type_oid: Some(oid::TEXT) });
        assert_eq!(first(&lite("SELECT CAST(1 AS BIGINT)")), Fact { non_null: true, type_oid: Some(oid::INT8) });
        // A class SQLite may answer either way is neither typed nor proven.
        assert_eq!(first(&lite("SELECT CAST(1 AS NUMERIC)")), Fact { non_null: false, type_oid: None });
        assert!(!first(&lite("SELECT CAST(NULL AS INTEGER)")).non_null);
        // On Postgres a cast needs a target the catalog says only the built-ins reach.
        let shadowed = Trust { shadowed: HashSet::new(), cast_targets: Some(["text".to_string()].into_iter().collect()), ..Default::default() };
        assert!(first(&plan("SELECT 1::text", Dialect::Postgres, &shadowed)).non_null);
        assert!(!first(&plan("SELECT 1::int4", Dialect::Postgres, &shadowed)).non_null);
        // A cast to an array of a built-in type keeps an array an array.
        assert!(first(&pg("SELECT '{}'::uuid[]")).non_null);
        assert!(first(&pg("SELECT ARRAY[]::int[]")).non_null);
        assert!(!first(&pg("SELECT NULL::int[]")).non_null);
        // A user's cast to the array type is listed under the array type's own name.
        let element_only = Trust { cast_targets: Some(["int4".to_string()].into_iter().collect()), ..Default::default() };
        assert!(!first(&plan("SELECT '{}'::int[]", Dialect::Postgres, &element_only)).non_null);
        let both = Trust { cast_targets: Some(["int4".to_string(), "_int4".to_string()].into_iter().collect()), ..Default::default() };
        assert!(first(&plan("SELECT '{}'::int[]", Dialect::Postgres, &both)).non_null);
    }

    #[test]
    fn unnest_of_an_array_constructor_gives_its_elements_where_none_is_an_array() {
        assert!(first(&pg("SELECT unnest(ARRAY[1, 2])")).non_null);
        assert!(first(&pg("SELECT unnest(ARRAY[[1, 2], [3, 4]])")).non_null);
        assert!(first(&pg("SELECT unnest(ARRAY['a', '{NULL}'])")).non_null);
        assert!(!first(&pg("SELECT unnest(ARRAY[1, NULL])")).non_null);
        // An element that is an array makes the array one of more dimensions, whose elements
        // unnest gives, and a literal beside it is an array too.
        assert!(!first(&pg("SELECT unnest(ARRAY['{NULL}'::int[]])")).non_null);
        assert!(!first(&pg("SELECT unnest(ARRAY['{NULL}', '{1}'::int[]])")).non_null);
        const INT4_ARRAY: u32 = 1007;
        let one = |sql: &str, leaves: &[DescribedColumn]| resolve(pg(sql).head[0].as_ref().expect("a claim"), Some(leaves)).non_null;
        assert!(one("SELECT unnest(ARRAY[a]) FROM t", &[col(false, oid::INT4)]));
        assert!(!one("SELECT unnest(ARRAY[a]) FROM t", &[col(false, INT4_ARRAY)]));
        assert!(!one("SELECT unnest(ARRAY[a]) FROM t", &[col(false, oid::UNKNOWN)]));
        // The catalog is asked whether a user defined an `unnest`, which then proves nothing.
        assert!(postgres_function_names().contains(&"unnest"));
        let shadowed = Trust { shadowed: ["unnest".to_string()].into_iter().collect(), ..Trust::default() };
        assert!(!first(&plan("SELECT unnest(ARRAY[1])", Dialect::Postgres, &shadowed)).non_null);
    }

    #[test]
    fn a_cast_of_a_jsonb_value_to_a_number_may_be_null() {
        // PostgreSQL 18 casts a JSON null to an SQL NULL where 17 raised an error, so a cast that
        // `jsonb` has a function for proves nothing over a value that may be `jsonb`.
        let over = |sql: &str, leaf: u32| {
            let p = pg(sql);
            resolve(p.head[0].as_ref().unwrap(), Some(&[col(false, leaf)])).non_null
        };
        for target in ["int4", "int8", "int2", "numeric", "float4", "float8", "bool"] {
            assert!(!over(&format!("SELECT j::{target} FROM docs"), oid::JSONB), "{target}");
            assert!(over(&format!("SELECT j::{target} FROM docs"), oid::TEXT), "{target}");
        }
        assert!(over("SELECT j::text FROM docs", oid::JSONB), "a cast to text prints the JSON");
        assert!(over("SELECT j::text::int4 FROM docs", oid::JSONB), "text 'null' is no integer, and raises");
        // A value built from a jsonb one, and one of a type a database defined, which may be a
        // domain over jsonb, or of a type the leaf probe did not name.
        assert!(!over("SELECT coalesce(j, '1')::int4 FROM docs", oid::JSONB));
        assert!(!over("SELECT to_jsonb(j)::int4 FROM docs", oid::JSON));
        assert!(!over("SELECT jsonb_strip_nulls(j)::int4 FROM docs", oid::JSONB));
        assert!(!over("SELECT CASE WHEN j IS NOT NULL THEN j::int4 ELSE 0 END FROM docs", oid::JSONB), "not NULL is not a JSON null");
        assert!(over("SELECT CASE WHEN j IS NOT NULL THEN j::int4 ELSE 0 END FROM docs", oid::NUMERIC));
        assert!(!over("SELECT j::int4 FROM docs", 16385));
        assert!(!over("SELECT j::int4 FROM docs", oid::UNKNOWN));
        // Everything else keeps what it proved: a number, an arithmetic over numbers, a parameter
        // and a literal, which take the cast's own type, and a constructor, which raises.
        assert!(over("SELECT (j * 2)::numeric FROM docs", oid::INT4));
        assert!(over("SELECT length(j)::int8 FROM docs", oid::TEXT));
        assert!(first(&pg("SELECT '5'::int4")).non_null);
        assert!(first(&pg("SELECT now()::date::text::int4")).non_null);
        let params = Trust { non_null_params: true, ..Default::default() };
        assert!(first(&plan("SELECT $1::int4", Dialect::Postgres, &params)).non_null);
        assert!(!first(&plan("SELECT $1::jsonb::int4", Dialect::Postgres, &params)).non_null);
        // A server before 18 raises an error for a JSON null, so the cast keeps what it proved.
        let before = Trust { jsonb_casts_raise: true, ..Default::default() };
        let p = plan("SELECT j::int4 FROM docs", Dialect::Postgres, &before);
        assert!(resolve(p.head[0].as_ref().unwrap(), Some(&[col(false, oid::JSONB)])).non_null);
    }

    #[test]
    fn a_shadowed_name_proves_nothing() {
        let trust = Trust { shadowed: ["count".to_string(), "=".to_string()].into_iter().collect(), cast_targets: None, ..Default::default() };
        assert!(!first(&plan("SELECT count(*) FROM t", Dialect::Postgres, &trust)).non_null);
        assert!(!first(&plan("SELECT 1 = 1", Dialect::Postgres, &trust)).non_null);
        assert!(first(&plan("SELECT 1 + 1", Dialect::Postgres, &trust)).non_null);
    }

    #[test]
    fn a_scalar_subquery_is_non_null_only_as_an_ungrouped_aggregate() {
        assert!(first(&pg("SELECT (SELECT count(*) FROM todos t WHERE t.owner_id = o.id) AS n FROM owners o")).non_null);
        assert!(first(&pg("SELECT (SELECT coalesce(max(t.n), 0) FROM todos t) AS n")).non_null);
        // Not an aggregate: it may match no row.
        assert!(!first(&pg("SELECT (SELECT 1 FROM todos LIMIT 1) AS n")).non_null);
        // An aggregate whose one row a clause can take away.
        for sql in [
            "SELECT (SELECT count(*) FROM todos GROUP BY owner_id) AS n",
            "SELECT (SELECT count(*) FROM todos HAVING false) AS n",
            "SELECT (SELECT count(*) FROM todos LIMIT 0) AS n",
            "SELECT (SELECT count(*) FROM todos OFFSET 1) AS n",
        ] {
            assert!(!first(&pg(sql)).non_null, "{sql}");
        }
        // A window call is not an aggregate, and neither is SQLite's two-argument `max`.
        assert!(!first(&pg("SELECT (SELECT coalesce(count(*) OVER (), 0) FROM todos) AS n")).non_null);
        assert!(!first(&lite("SELECT (SELECT coalesce(max(a, b), 0) FROM t) AS n")).non_null);
        // A non-null aggregate still needs a non-null value.
        assert!(!first(&pg("SELECT (SELECT max(n) FROM todos) AS n")).non_null);
        // A set-returning call in the target, a user's function or operator among them, makes
        // the one row as many as it returns, none included, whatever the value would be.
        for sql in [
            "SELECT (SELECT generate_series(1, count(*)::int) FROM todos) AS n",
            "SELECT (SELECT count(*) + generate_series(1, 0) FROM todos) AS n",
            "SELECT (SELECT count(*)::text || (generate_series(1, 0) IS NULL)::text FROM todos) AS n",
            "SELECT (SELECT count(*)::text || (f() IS NULL)::text FROM todos) AS n",
            "SELECT (SELECT count(*)::text || (public.abs(1) IS NULL)::text FROM todos) AS n",
            "SELECT (SELECT count(*)::text || ((1 <<->> 2) IS NULL)::text FROM todos) AS n",
            "SELECT (SELECT generate_series(1, 0)) AS n",
            "SELECT (SELECT unnest('{}'::int[]) IS NULL) AS n",
            "SELECT (SELECT (1 <<->> 2) IS NULL) AS n",
        ] {
            assert!(!first(&pg(sql)).non_null, "{sql}");
        }
        // Postgres refuses one in an aggregate's arguments, so they are not read; and a built-in
        // function or operator the rules read returns no set.
        for sql in [
            "SELECT (SELECT count(f(t.n)) FROM todos t) AS n",
            "SELECT (SELECT count(*)::text || (abs(1) IS NULL)::text FROM todos) AS n",
            "SELECT (SELECT count(*)::text || ((1 BETWEEN 0 AND 2) IS NULL)::text FROM todos) AS n",
            "SELECT (SELECT pg_catalog.count(*) FROM todos) AS n",
            "SELECT (SELECT ((1 BETWEEN 0 AND 2) IS NULL)::int + 1) AS n",
            "SELECT (SELECT abs(-1)) AS n",
        ] {
            assert!(first(&pg(sql)).non_null, "{sql}");
        }
        // An aggregate over columns of the query around the subquery only is that query's, and
        // the subquery a row per row of its own FROM; with `WITHIN GROUP` it is never read here.
        for sql in [
            "SELECT (SELECT count(o.id) FROM todos t WHERE false) AS n FROM owners o",
            "SELECT (SELECT count(o.id) FILTER (WHERE o.id > 0) FROM todos t) AS n FROM owners o",
            "SELECT (SELECT coalesce(max(o.id), 0) FROM todos) AS n FROM owners o",
            "SELECT (SELECT count((SELECT t.id FROM owners t LIMIT 1) + o.id) FROM todos t WHERE false) AS n FROM owners o",
            "SELECT (SELECT coalesce(mode() WITHIN GROUP (ORDER BY o.id), 0) FROM todos t WHERE false) AS n FROM owners o",
        ] {
            assert!(!first(&pg(sql)).non_null, "{sql}");
        }
        for sql in [
            "SELECT (SELECT count(t.id) FROM todos t WHERE t.owner_id = o.id) AS n FROM owners o",
            "SELECT (SELECT count(todos.id) FROM todos WHERE todos.owner_id = o.id) AS n FROM owners o",
            "SELECT (SELECT count(id) FROM todos WHERE owner_id = o.id) AS n FROM owners o",
            "SELECT (SELECT count(o.id + t.id) FROM todos t) AS n FROM owners o",
            "SELECT (SELECT count(t.*) FROM todos t) AS n FROM owners o",
            "SELECT (SELECT count(1) FROM todos t WHERE t.owner_id = o.id) AS n FROM owners o",
            "SELECT (SELECT count(title) FROM (SELECT id, title FROM todos) t WHERE false) AS n FROM owners",
            "WITH t AS (SELECT * FROM todos) SELECT (SELECT coalesce(sum(n), 0) FROM t) AS n FROM owners",
        ] {
            assert!(first(&pg(sql)).non_null, "{sql}");
        }
        // A bare column is the innermost FROM's that has it: `name` is a column of `owners` alone,
        // so these aggregates are the outer query's too.
        for sql in [
            "SELECT (SELECT count(name) FROM todos WHERE false) AS n FROM owners",
            "SELECT (SELECT coalesce(max(length(name)), 0) FROM todos) AS n FROM owners",
            "SELECT (SELECT count(*) FILTER (WHERE name <> '') FROM todos) AS n FROM owners",
            "SELECT (SELECT count(t.id) + (SELECT count(name) FROM todos WHERE false) FROM todos t) AS n FROM owners",
            "SELECT (SELECT count(name) FROM (SELECT id, title FROM todos) t) AS n FROM owners",
            "WITH todos AS (SELECT id FROM owners) SELECT (SELECT count(title) FROM todos) AS n FROM owners",
        ] {
            assert!(!first(&pg(sql)).non_null, "{sql}");
        }
        // Without the columns of the subquery's FROM nothing says whose a bare column is.
        let bare = "SELECT (SELECT count(id) FROM todos WHERE owner_id = o.id) AS n FROM owners o";
        assert!(!first(&super::plan(bare, Dialect::Postgres, &Trust::default(), &Columns::default())).non_null);
        assert!(!first(&pg("SELECT (SELECT count(id) FROM unlisted) AS n FROM owners")).non_null);
    }

    #[test]
    fn a_column_reference_in_an_expression_is_a_leaf_over_the_same_from() {
        let p = pg("SELECT t.title || ' by ' || u.name AS label, t.id FROM todos t LEFT JOIN users u ON u.id = t.owner_id");
        assert_eq!(p.leaves.as_deref(), Some("SELECT t.title AS a0, u.name AS a1 FROM todos t LEFT JOIN users u ON u.id = t.owner_id"));
        assert!(p.reads_leaves());
        assert_eq!(p.head[1], None, "a plain column reference is the describe tier's");
        let label = p.head[0].as_ref().unwrap();
        assert!(resolve(label, Some(&[col(false, oid::TEXT), col(false, oid::TEXT)])).non_null);
        assert!(!resolve(label, Some(&[col(false, oid::TEXT), col(true, oid::TEXT)])).non_null);
        assert!(!resolve(label, None).non_null, "no probe, no proof");
        // On SQLite a leaf carries its described type through a function the host registered.
        let host = Trust { total: [("reylo_html".to_string(), oid::TEXT)].into_iter().collect(), ..Default::default() };
        let h = plan("SELECT id, reylo_html(title) AS title_html FROM todos WHERE owner_id = ?1 ORDER BY title", Dialect::Sqlite, &host);
        // The probe filters by the WHERE, a placeholder there written as NULL.
        assert_eq!(h.leaves.as_deref(), Some("SELECT title AS a0 FROM todos WHERE owner_id = NULL"));
        assert_eq!(resolve(h.head[1].as_ref().unwrap(), Some(&[col(false, oid::TEXT)])), Fact { non_null: true, type_oid: Some(oid::TEXT) });
        // RETURNING reads the written relation.
        let r = pg("UPDATE todos SET done = true WHERE id = $1 RETURNING title || '!' AS shout");
        assert_eq!(r.leaves.as_deref(), Some("SELECT title AS a0 FROM todos"));
    }

    #[test]
    fn returning_reads_the_old_and_the_new_row_as_the_written_relations() {
        let text = |nullable: bool| col(nullable, oid::TEXT);
        let fact = |p: &Plan, i: usize, leaves: &[DescribedColumn]| resolve(p.head[i].as_ref().unwrap(), Some(leaves)).non_null;
        // PostgreSQL 18: an UPDATE has both rows, by the names the clause gives them or by `old`
        // and `new`, and the leaf probe reads each as the written relation's.
        let u = pg("UPDATE todos t SET done = true WHERE id = $1 RETURNING WITH (OLD AS o) o.title || ' -> ' || new.title AS change, upper(new.title) AS up, o.title");
        assert_eq!(u.leaves.as_deref(), Some("SELECT t.title AS a0, t.title AS a1, t.title AS a2 FROM todos t"));
        assert!(fact(&u, 0, &[text(false), text(false), text(false)]) && fact(&u, 1, &[text(false), text(false), text(false)]));
        assert_eq!(u.head[2], None, "a plain reference is the describe tier's");
        // An INSERT has no old row, so what reads one is NULL unless the expression says otherwise.
        let i = pg("INSERT INTO todos (title) VALUES ($1) ON CONFLICT (id) DO UPDATE SET title = excluded.title RETURNING old.title || '!' AS was, new.title || '!' AS now, old.id IS NULL AS inserted, coalesce(old.title, '') AS before, CASE WHEN old.title IS NOT NULL THEN old.title ELSE '' END AS tested");
        assert_eq!(i.leaves.as_deref(), Some("SELECT todos.title AS a0, todos.title AS a1, todos.title AS a2, todos.title AS a3 FROM todos"));
        let leaves = [text(false), text(false), text(false), text(false)];
        assert_eq!((0..5).map(|n| fact(&i, n, &leaves)).collect::<Vec<_>>(), [false, true, true, true, true]);
        // A DELETE has no new row.
        let d = pg("DELETE FROM todos WHERE id = $1 RETURNING WITH (NEW AS n) n.title || '' AS now, coalesce(n.title, old.title) AS either");
        assert_eq!((0..2).map(|n| fact(&d, n, &[text(false), text(false), text(false)])).collect::<Vec<_>>(), [false, true]);
        // A relation of the name is the one read: the leaf stays as written.
        let named = pg("DELETE FROM todos AS new WHERE id = $1 RETURNING new.title || '' AS title");
        assert_eq!(named.leaves.as_deref(), Some("SELECT new.title AS a0 FROM todos new"));
        assert!(fact(&named, 0, &[text(false)]));
        // A MERGE names its action for every row, and reads its source through an outer join
        // where it acts on target rows the source does not match.
        let m = pg("MERGE INTO todos t USING drafts s ON t.id = s.id WHEN MATCHED THEN UPDATE SET title = s.title WHEN NOT MATCHED BY SOURCE THEN DELETE RETURNING merge_action() AS action, s.title || '' AS source, old.title || '' AS was, new.title || '' AS now");
        assert_eq!(m.leaves.as_deref(), Some("SELECT s.title AS a0, t.title AS a1, t.title AS a2 FROM todos t LEFT JOIN drafts s ON true"));
        assert_eq!(resolve(m.head[0].as_ref().unwrap(), None), Fact { non_null: true, type_oid: Some(oid::TEXT) });
        // The source's leaf is nullable in the probe's own describe, by the join.
        assert_eq!((1..4).map(|n| fact(&m, n, &[text(true), text(false), text(false)])).collect::<Vec<_>>(), [false, true, false]);
        let matched = pg("MERGE INTO todos t USING drafts s ON t.id = s.id WHEN MATCHED THEN UPDATE SET title = s.title RETURNING s.title || '' AS source");
        assert_eq!(matched.leaves.as_deref(), Some("SELECT s.title AS a0 FROM todos t, drafts s"));
    }

    #[test]
    fn a_grouping_set_takes_the_leaves_away() {
        let p = pg("SELECT owner_id || '' AS o, count(*) AS n FROM todos GROUP BY ROLLUP (owner_id)");
        assert_eq!(p.leaves, None);
        assert!(!resolve(p.head[0].as_ref().unwrap(), None).non_null);
        assert!(resolve(p.head[1].as_ref().unwrap(), None).non_null, "a count is still a count");
    }

    #[test]
    fn a_star_splits_the_claims_around_it() {
        let p = pg("SELECT 'a' AS x, t.*, count(*) OVER () AS total FROM todos t");
        assert!(p.star);
        let claims = p.claims(5);
        assert!(claims[0].is_some() && claims[1].is_none() && claims[3].is_none() && claims[4].is_some());
        // A describe with fewer columns than the plan names claims nothing.
        assert!(pg("SELECT 'a', 'b'").claims(3).iter().all(Option::is_none));
    }

    #[test]
    fn a_set_operation_plans_its_arms() {
        let p = pg("WITH d AS (SELECT id FROM todos) SELECT id, title FROM todos WHERE done UNION ALL SELECT id, title FROM todos WHERE id IN (SELECT id FROM d)");
        let set = p.set_op.expect("a set operation");
        assert_eq!(set.kind, SetKind::Union);
        assert_eq!(set.left, "WITH d AS (SELECT id FROM todos) SELECT id, title FROM todos WHERE done");
        assert_eq!(set.right, "WITH d AS (SELECT id FROM todos) SELECT id, title FROM todos WHERE id IN (SELECT id FROM d)");
        assert!(p.head.is_empty());
        // Nested operations nest: the left arm is itself a set operation.
        let n = pg("SELECT 1 UNION SELECT 2 EXCEPT SELECT 3");
        assert_eq!(n.set_op.map(|s| (s.kind, s.left)), Some((SetKind::Except, "SELECT 1 UNION SELECT 2".to_string())));
        assert_eq!(lite("SELECT ?1 AS a INTERSECT SELECT 'x'").set_op.map(|s| s.left), Some("SELECT $1 AS a".to_string()));
    }

    #[test]
    fn set_operation_columns_combine_per_arm() {
        let (nn, nul) = (col(false, oid::UUID), col(true, oid::UUID));
        let facts = |kind, l: &DescribedColumn, r: &DescribedColumn| combine(kind, &[l.clone()], &[r.clone()]).unwrap()[0].non_null;
        assert!(facts(SetKind::Union, &nn, &nn));
        assert!(!facts(SetKind::Union, &nn, &nul), "one nullable arm makes a UNION column nullable");
        assert!(!facts(SetKind::Union, &nul, &nn));
        assert!(facts(SetKind::Intersect, &nul, &nn));
        assert!(!facts(SetKind::Intersect, &nul, &nul));
        assert!(facts(SetKind::Except, &nn, &nul));
        assert!(!facts(SetKind::Except, &nul, &nn));
        assert!(combine(SetKind::Union, &[nn.clone()], &[]).is_none());
    }

    #[test]
    fn a_values_arm_is_non_null_where_every_row_is() {
        let p = pg("VALUES (1, 'a'), (2, NULL)");
        assert!(resolve(p.head[0].as_ref().unwrap(), None).non_null);
        assert!(!resolve(p.head[1].as_ref().unwrap(), None).non_null);
    }

    #[test]
    fn a_group_is_never_empty_so_its_aggregate_follows_the_argument() {
        let p = pg("SELECT user_id, sum(views) AS total, max(views) AS top, array_agg(title) AS titles, sum(views) FILTER (WHERE views > 0) AS some, sum(views) OVER () AS running FROM posts GROUP BY user_id");
        // One leaf per column reference read: five of `views` here.
        let views = [col(false, oid::INT4), col(false, oid::INT4), col(false, oid::INT4), col(false, oid::INT4), col(false, oid::INT4)];
        assert_eq!(p.head[0], None);
        assert!(resolve(p.head[1].as_ref().unwrap(), Some(&views)).non_null);
        assert!(resolve(p.head[2].as_ref().unwrap(), Some(&views)).non_null);
        assert!(resolve(p.head[3].as_ref().unwrap(), Some(&views)).non_null);
        // A FILTER can leave the group no rows, and a window frame can be empty.
        assert!(!resolve(p.head[4].as_ref().unwrap(), Some(&views)).non_null);
        assert!(!resolve(p.head[5].as_ref().unwrap(), Some(&views)).non_null);
        // A nullable argument, no GROUP BY, a grouping set, or SQLite (a sum can be NaN, which it stores as NULL).
        assert!(!resolve(p.head[1].as_ref().unwrap(), Some(&[col(true, oid::INT4)])).non_null);
        assert!(!first(&pg("SELECT sum(views) AS total FROM posts")).non_null);
        assert!(!resolve(pg("SELECT sum(views) AS total FROM posts GROUP BY ROLLUP (user_id)").head[0].as_ref().unwrap(), Some(&views)).non_null);
        assert!(!resolve(lite("SELECT sum(views) AS total FROM posts GROUP BY user_id").head[0].as_ref().unwrap(), Some(&views)).non_null);
    }

    #[test]
    fn a_json_aggregate_under_a_group_by_is_never_null() {
        // A group has a row, and a JSON aggregate writes a NULL input as a JSON null: the value is
        // there whatever its argument holds, so no leaf is asked about.
        for sql in [
            "SELECT kind, jsonb_agg(payload ORDER BY id) AS payloads FROM events GROUP BY kind",
            "SELECT kind, json_agg(note) AS notes FROM events GROUP BY kind",
            "SELECT kind, jsonb_object_agg(id, note) AS notes FROM events GROUP BY kind",
            "SELECT kind, json_object_agg(id, payload) AS by_id FROM events GROUP BY kind",
        ] {
            assert_eq!(pg(sql).head[1], Some(Expr::NonNull(None)), "{sql}");
        }
        // A FILTER can leave a group no rows, a window frame can be empty, and without a GROUP BY,
        // or with a grouping set (`GROUP BY ()` over no rows is one row), there may be no rows.
        for sql in [
            "SELECT kind, jsonb_agg(payload) FILTER (WHERE id > 1) AS payloads FROM events GROUP BY kind",
            "SELECT kind, jsonb_agg(payload) OVER (ROWS BETWEEN 1 FOLLOWING AND 1 FOLLOWING) AS next FROM events GROUP BY kind, payload",
            "SELECT 1, jsonb_agg(payload) AS payloads FROM events",
            "SELECT kind, jsonb_agg(payload) AS payloads FROM events GROUP BY ROLLUP (kind)",
            "SELECT kind, jsonb_agg(payload) AS payloads FROM events GROUP BY GROUPING SETS ((kind), ())",
        ] {
            let p = pg(sql);
            assert!(p.head[1].as_ref().is_none_or(|e| !resolve(e, Some(&[col(false, oid::JSONB), col(false, oid::JSONB)])).non_null), "{sql}");
        }
        // A user's function of the name may return NULL.
        let shadowed = Trust { shadowed: ["jsonb_agg".to_string()].into_iter().collect(), ..Default::default() };
        assert_ne!(plan("SELECT kind, jsonb_agg(payload) FROM events GROUP BY kind", Dialect::Postgres, &shadowed).head[1], Some(Expr::NonNull(None)));
        // `string_agg` and `max` skip NULLs, so a group of NULLs is NULL: they follow the argument.
        let p = pg("SELECT kind, string_agg(note, ', ') AS notes, max(note) AS top FROM events GROUP BY kind");
        assert!(!resolve(p.head[1].as_ref().unwrap(), Some(&[col(true, oid::TEXT)])).non_null);
        assert!(resolve(p.head[1].as_ref().unwrap(), Some(&[col(false, oid::TEXT)])).non_null);
        assert!(!resolve(p.head[2].as_ref().unwrap(), Some(&[col(true, oid::TEXT), col(true, oid::TEXT)])).non_null);
        // Inside a scalar subquery the aggregate may be the outer query's.
        assert!(!first(&pg("SELECT (SELECT jsonb_agg(e.id) FROM events e GROUP BY e.kind LIMIT 1) AS ids")).non_null);
    }

    #[test]
    fn a_strict_where_proves_the_column_it_tests() {
        let p = pg("SELECT id, email, p.title, body FROM users u JOIN posts p ON p.user_id = u.id WHERE email IS NOT NULL AND title > $1 AND (body IS NOT NULL OR id = 1)");
        assert_eq!(p.head[0], None);
        assert_eq!(p.head[1], Some(Expr::NonNull(None)));
        // `title` in the WHERE and `p.title` in the list are one column: `p` is a FROM item here.
        assert_eq!(p.head[2], Some(Expr::NonNull(None)));
        // Under an OR the test may not have run.
        assert_eq!(p.head[3], None);
        // A qualifier that is no FROM item of this SELECT is some other column.
        assert_eq!(pg("SELECT email FROM users WHERE outer_row.email IS NOT NULL").head[0], None);
        // A user's own `=` may be true of NULL, and a grouping set nulls the column again.
        let shadowed = Trust { shadowed: ["=".to_string()].into_iter().collect(), ..Default::default() };
        assert_eq!(plan("SELECT email FROM users WHERE email = $1", Dialect::Postgres, &shadowed).head[0], None);
        assert_eq!(pg("SELECT email FROM users WHERE email IS NOT NULL GROUP BY ROLLUP (email)").head[0], None);
    }

    #[test]
    fn a_tables_whole_row_is_never_null_unless_a_join_can_null_it() {
        let p = pg("SELECT u AS whole, u.name FROM users u WHERE u.id = $1");
        assert_eq!(p.head[0], Some(Expr::Row(0, "users".to_string())));
        assert_eq!(p.head[1], None);
        assert_eq!(p.leaves.as_deref(), Some("SELECT u AS a0 FROM users u WHERE u.id = NULL"));
        // The probe says whether it was the row after all: a column named like the alias is not.
        let row = DescribedColumn { name: "whole".into(), type_oid: 16500, type_name: "users".into(), nullable: true, nullable_items: None, values: None, document: None, whole_row: false };
        let column = DescribedColumn { name: "whole".into(), type_oid: oid::TEXT, type_name: "text".into(), nullable: true, nullable_items: None, values: None, document: None, whole_row: false };
        assert!(resolve(p.head[0].as_ref().unwrap(), Some(&[row])).non_null);
        assert!(!resolve(p.head[0].as_ref().unwrap(), Some(&[column])).non_null);
        assert_eq!(pg("SELECT p FROM users u LEFT JOIN posts p ON p.user_id = u.id").head[0], None);
        assert_eq!(pg("SELECT s FROM (SELECT 1 AS a) s").head[0], None);
    }

    #[test]
    fn a_column_read_from_a_with_query_names_that_query_as_a_statement() {
        let column = |item: usize, column: &str| ItemRead::Column { item, column: column.into() };
        let (reads, queries) = item_reads("WITH pub AS (SELECT id, at FROM posts WHERE at IS NOT NULL), other AS (SELECT 1 AS one) SELECT id, pub.at, id + 1 AS next FROM pub").unwrap();
        assert_eq!(reads, vec![column(0, "id"), column(0, "at"), ItemRead::Other]);
        // Only the query that is read, with the whole WITH clause still in front of it.
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].with.as_deref(), Some("pub"));
        assert_eq!(queries[0].sql, "WITH pub AS (SELECT id, at FROM posts WHERE at IS NOT NULL), other AS (SELECT 1 AS one) SELECT id, at FROM posts WHERE at IS NOT NULL");
        // A recursive one keeps its own name in scope, and its column list renames its columns.
        let (reads, queries) = item_reads("WITH RECURSIVE t(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM t WHERE n < 5) SELECT n FROM t").unwrap();
        assert_eq!(reads, vec![column(0, "n")]);
        assert_eq!(queries[0].columns, vec!["n"]);
        assert!(queries[0].sql.starts_with("WITH RECURSIVE t(n) AS ("));
        // A bare name among several FROM items could be any item's; a table is not a WITH query.
        assert_eq!(item_reads("WITH a AS (SELECT 1 AS x) SELECT x FROM a, users"), None);
        assert_eq!(item_reads("WITH a AS (SELECT 1 AS x) SELECT id FROM users"), None);
        assert_eq!(item_reads("WITH a AS (SELECT 1 AS x) SELECT x FROM public.a"), None);
        assert_eq!(item_reads("SELECT id FROM users"), None);
        // A `*` is every column of the one item, or of the item it names.
        let (reads, _) = item_reads("WITH a AS (SELECT 1 AS x) SELECT *, x FROM a").unwrap();
        assert_eq!(reads, vec![ItemRead::Star { item: Some(0) }, column(0, "x")]);
        let (reads, _) = item_reads("WITH a AS (SELECT 1 AS x) SELECT u.*, a.*, a.x FROM users u, a").unwrap();
        assert_eq!(reads, vec![ItemRead::Star { item: None }, ItemRead::Star { item: Some(0) }, column(0, "x")]);
        assert_eq!(item_reads("WITH a AS (SELECT 1 AS x) SELECT * FROM a, users"), None);
        assert_eq!(item_reads("WITH a AS (SELECT ROW(1, 2)::pair AS p) SELECT (p).*, p FROM a"), None);
    }

    #[test]
    fn a_bare_column_of_a_table_is_read_from_that_table() {
        let reads = |sql: &str| target_reads(sql).unwrap();
        // By its name or its alias, or bare where the table is the one item, and through joins.
        assert_eq!(reads("SELECT id, name FROM users"), vec![ItemRead::Table, ItemRead::Table]);
        assert_eq!(reads("SELECT u.name, public.users.id FROM users u"), vec![ItemRead::Table, ItemRead::Other]);
        assert_eq!(reads("SELECT u.name, p.title, name FROM users u JOIN posts p ON p.user_id = u.id"), vec![ItemRead::Table, ItemRead::Table, ItemRead::Other]);
        assert_eq!(reads("SELECT public.users.id FROM public.users"), vec![ItemRead::Other]);
        assert_eq!(reads("SELECT x.id FROM public.users x"), vec![ItemRead::Table]);
        // A WITH query of that name is no table, whatever its statement reads; an expression
        // and a `*` read no one column.
        assert_eq!(reads("WITH users AS (SELECT 1 AS id UNION SELECT 2) SELECT users.id FROM users"), vec![ItemRead::Column { item: 0, column: "id".into() }]);
        assert_eq!(reads("WITH w AS (INSERT INTO t DEFAULT VALUES RETURNING id) SELECT w.id FROM w"), vec![ItemRead::Other]);
        assert_eq!(reads("SELECT u.id + 1, u.* FROM users u"), vec![ItemRead::Other, ItemRead::Star { item: None }]);
        assert_eq!(reads("SELECT s.id FROM users u, LATERAL (SELECT u.id) s"), vec![ItemRead::Other]);
        // A set operation at the top hides which arm a column is.
        assert_eq!(target_reads("SELECT id FROM users UNION SELECT id FROM posts"), None);
    }

    #[test]
    fn an_empty_array_default_of_a_column_read_from_a_with_query_reads_it() {
        let defaulted = |item: usize, column: &str| ItemRead::Defaulted { item, column: column.into() };
        let (reads, _) = item_reads("WITH a AS (SELECT jsonb_agg(1) AS xs) SELECT coalesce(a.xs, '[]'), coalesce(xs, '[]'::jsonb) FROM a").unwrap();
        assert_eq!(reads, vec![defaulted(0, "xs"), defaulted(0, "xs")]);
        // Any other default, a third argument or an expression is no read of the column.
        let (reads, _) = item_reads("WITH a AS (SELECT jsonb_agg(1) AS xs) SELECT xs, coalesce(xs, '{}'), coalesce(xs, '[]', '[]'), coalesce(xs || '[]', '[]') FROM a").unwrap();
        assert_eq!(reads, vec![ItemRead::Column { item: 0, column: "xs".into() }, ItemRead::Other, ItemRead::Other, ItemRead::Other]);
        // A defaulted read is a read of the item.
        assert!(item_reads("WITH a AS (SELECT jsonb_agg(1) AS xs) SELECT coalesce(a.xs, '[]') FROM a").is_some());
    }

    #[test]
    fn a_derived_table_and_a_function_in_from_are_statements_of_their_own() {
        let column = |item: usize, column: &str| ItemRead::Column { item, column: column.into() };
        let (reads, queries) = item_reads("WITH c AS (SELECT 1 AS one) SELECT s.n, u.id FROM users u LEFT JOIN (SELECT count(*) AS n FROM c) s (total) ON true").unwrap();
        assert_eq!(reads, vec![column(0, "n"), ItemRead::Table]);
        assert_eq!(queries, vec![ItemQuery { sql: "WITH c AS (SELECT 1 AS one) SELECT count(*) AS n FROM c".into(), columns: vec!["total".into()], with: None, nulled: true }]);
        // The preserved side of an outer join is not nulled; a FULL JOIN nulls both.
        let nulled = |sql: &str| item_reads(sql).unwrap().1.iter().map(|q| q.nulled).collect::<Vec<_>>();
        assert_eq!(nulled("SELECT a.x, b.y FROM (SELECT 1 AS x) a LEFT JOIN (SELECT 2 AS y) b ON true"), vec![false, true]);
        assert_eq!(nulled("SELECT a.x, b.y FROM (SELECT 1 AS x) a RIGHT JOIN (SELECT 2 AS y) b ON true"), vec![true, false]);
        assert_eq!(nulled("SELECT a.x, b.y FROM (SELECT 1 AS x) a FULL JOIN (SELECT 2 AS y) b ON true"), vec![true, true]);
        assert_eq!(nulled("SELECT a.x, b.y FROM users u LEFT JOIN ((SELECT 1 AS x) a JOIN (SELECT 2 AS y) b ON true) ON true"), vec![true, true]);
        // A function is the same call in a select list, its parameters kept, under the name the
        // item's one column goes by.
        let (reads, queries) = item_reads("SELECT * FROM lower($1)").unwrap();
        assert_eq!(reads, vec![ItemRead::Star { item: Some(0) }]);
        assert_eq!(queries, vec![ItemQuery { sql: "SELECT lower($1) AS value".into(), columns: vec!["lower".into()], with: None, nulled: false }]);
        assert_eq!(item_reads("SELECT t FROM now() AS t").unwrap().1[0].columns, vec!["t"]);
        assert_eq!(item_reads("SELECT v FROM now() AS t (v)").unwrap().1[0].columns, vec!["v"]);
        assert_eq!(item_reads("SELECT * FROM CURRENT_DATE AS d").unwrap().1[0].sql, "SELECT current_date AS value");
        // No statement of its own: LATERAL, several functions, more columns than the value, a
        // join that renames what it joins.
        for sql in [
            "SELECT s.n FROM users u, LATERAL (SELECT count(*) AS n FROM posts p WHERE p.author_id = u.id) s",
            "SELECT * FROM ROWS FROM (now(), lower('A'))",
            "SELECT * FROM lower('A') WITH ORDINALITY",
            "SELECT * FROM json_to_record('{}') AS r (a int)",
            "SELECT j.n FROM ((SELECT 1 AS n) a CROSS JOIN users u) AS j",
        ] {
            assert_eq!(item_reads(sql), None, "{sql}");
        }
    }

    #[test]
    fn a_row_constructor_is_never_null() {
        let p = pg("SELECT ROW(a, b) AS r, (a, b)::pair AS typed, (a, b) AS bare FROM t");
        for claim in &p.head {
            assert!(resolve(claim.as_ref().unwrap(), None).non_null);
        }
    }

    #[test]
    fn placeholders_are_parameters_and_prove_nothing() {
        assert!(!first(&pg("SELECT $1::uuid AS id")).non_null);
        assert!(!first(&lite("SELECT ?1 || 'x' AS id")).non_null);
        // A host that types its parameters without null says so, and names the exceptions.
        let typed = Trust { non_null_params: true, nullable_params: [2].into_iter().collect(), ..Default::default() };
        let p = plan("SELECT $1::uuid AS id, $2::uuid AS other", Dialect::Postgres, &typed);
        assert!(resolve(p.head[0].as_ref().unwrap(), None).non_null);
        assert!(!resolve(p.head[1].as_ref().unwrap(), None).non_null);
        assert_eq!(dollar_placeholders("SELECT ?1, '?2', \"?3\" -- ?4\n, ?10 /* ?5 */"), "SELECT $1, '?2', \"?3\" -- ?4\n, $10 /* ?5 */");
    }

    #[test]
    fn what_does_not_parse_claims_nothing() {
        assert_eq!(pg("SELEKT 1"), Plan::default());
        assert_eq!(lite("SELECT json_extract('not json', '$') AS boom").head, vec![Some(Expr::Unknown)]);
    }
}
