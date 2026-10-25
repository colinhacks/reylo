//! Expression columns, read off the SQL that computes them.
//!
//! The describe tier settles a plain column from its provenance: the table and attribute the
//! engine reports, the catalog's `NOT NULL`, the join analysis. An expression column has no
//! provenance. Postgres reports table oid 0 for `count(*)`, so there is no `attnotnull` to read,
//! and SQLite reports no declared type for it at all. The expression is in the statement,
//! though, and a small set of SQL constructs carries its own nullability and type:
//!
//! - `count(…)` is never NULL, over no rows and under `GROUP BY` alike. Every other aggregate
//!   is NULL over no rows and proves nothing, unless it always has a row: under a plain `GROUP
//!   BY`, or as a window function whose frame always holds the current row, it follows its
//!   argument.
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
//! a user or an extension created ([`Trust`]): a call of a name a user also defined reads as the
//! built-in only where its operands' types resolve it to the built-in ([`crate::resolution`]),
//! which is settled once the leaf probe has typed the columns it reads ([`Expr::Call`]).
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
use crate::jsonshape::{as_select, last_string, select_text, single_target, target_named_by_with, top_context};
use crate::levels::{Columns, Level, Withs};
use crate::pgtypes::oid;
use crate::resolution::{Kind, Operand, Shared};
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
/// The functions the rules read with a built-in of one argument that takes a row (`"any"`,
/// `anyelement`, `anynonarray`, `record`, or a variadic `"any"`): a call of one over a row is that
/// function, never a column of the row.
const POSTGRES_ROW_TAKERS: &[&str] = &[
    "count", "row_to_json", "to_json", "to_jsonb", "array_agg", "json_agg", "jsonb_agg", "concat", "num_nulls", "num_nonnulls",
    "quote_nullable", "quote_literal", "json_build_object", "json_build_array", "jsonb_build_object", "jsonb_build_array",
];
/// The functions the rules read with a built-in that may return a row: `min` and `max` of a
/// `record` (PostgreSQL 16 on), and `lag` and `lead` of an `anyelement`. What they return proves
/// nothing of its type.
const POSTGRES_ROW_RESULTS: &[&str] = &["min", "max", "lag", "lead"];
/// The never-null functions the rules read that take no argument but two (`random(min, max)`):
/// a call of one is no call of theirs.
const POSTGRES_NO_ARGUMENT: &[&str] = &["now", "clock_timestamp", "statement_timestamp", "transaction_timestamp", "gen_random_uuid", "uuidv4", "random"];
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
    for list in [FUNCTIONS, POSTGRES_TOTAL, &typed, POSTGRES_NEVER_NULL, POSTGRES_FIRST_ARG, POSTGRES_GROUPED, POSTGRES_GROUPED_NEVER_NULL, POSTGRES_GROUPED_JSON, POSTGRES_STRICT, POSTGRES_WINDOW, POSTGRES_SET_RETURNING, crate::jsonshape::POSTGRES_FUNCTIONS] {
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
    /// The JSON a column holds read at literal keys (`doc -> 'a'`, `doc #>> '{a,b}'`): the column,
    /// the keys, whether the read gives text (`->>`, `#>>`) rather than JSON, and whether a key
    /// Postgres reads as an integer may index an array (`#>`, `#>>`). Typed and non-null as the JSON
    /// Schema a CHECK holds the column to says ([`json_read`]).
    JsonPath { base: Box<Expr>, path: Vec<String>, text: bool, indexes: bool },
    /// A call of an operator or a function whose name a user or an extension also defined: what
    /// the built-in's rule says of it where its arguments' types resolve it to the built-in
    /// ([`Check`]), and nothing elsewhere.
    Call(Box<Call>),
    /// The one argument of a call Postgres may read as a column of it: non-null as the inner
    /// expression is only where its type is one the build knows is no row ([`is_scalar`]), its
    /// type the inner one's. A call `f(x)` no function matches is `x.f` where `x` is a row
    /// (`ParseFuncOrColumn`), which is NULL where that column is.
    NotRow(Box<Expr>, Option<Shared>),
    /// `then` where every check of one of the sets in `when` holds, `otherwise` elsewhere: a
    /// column a strict test narrows, where the test calls a name a user also defined.
    Trusted(Box<Trusted>),
}

/// A call [`Expr::Call`] reads: its arguments, each read once, and what holds of it where the
/// check resolves it to the built-in.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub check: Check,
    pub args: Vec<Expr>,
    pub claim: Claim,
}

/// What a built-in's rule says of a call, in terms of its arguments.
#[derive(Debug, Clone, PartialEq)]
pub enum Claim {
    /// Non-null when the first `n` arguments are, or all of them for `None`; of the type the
    /// built-in returns, or else the one `Ty` names.
    Every(Option<usize>, Ty),
    /// Never NULL, of this type or else the one the built-in returns.
    NonNull(Option<u32>),
    /// An array of the arguments' values (`array_agg`), never NULL itself where the flag says so.
    Array(bool),
}

/// See [`Expr::Trusted`].
#[derive(Debug, Clone, PartialEq)]
pub struct Trusted {
    pub when: Vec<Vec<Check>>,
    pub then: Expr,
    pub otherwise: Expr,
}

/// Whether a call of `name` over these operands resolves to the built-in ([`Trust::resolves`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    kind: Kind,
    name: String,
    args: Vec<Arg>,
    overloads: Option<Shared>,
}

/// One operand of a [`Check`].
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    /// Its type as the server reads it, settled by the SQL: an untyped literal, a parameter, a
    /// typed literal.
    Fixed(Operand),
    /// The type the call's argument at this position resolves to.
    At(usize),
    /// The type this expression resolves to.
    Of(Expr),
}

impl Check {
    /// What [`crate::resolution::Overloads::resolve`] says of the call, the arguments' types read
    /// from their facts, or from `leaves` for an operand of its own.
    fn resolve(&self, args: &[Fact], leaves: Option<&[DescribedColumn]>) -> Option<Option<u32>> {
        let overloads = self.overloads.as_ref()?;
        let typed = |t: Option<u32>| t.map_or(Operand::Opaque, Operand::Is);
        let operands: Vec<Operand> = self
            .args
            .iter()
            .map(|a| match a {
                Arg::Fixed(o) => *o,
                Arg::At(i) => typed(args.get(*i).and_then(|f| f.type_oid)),
                Arg::Of(e) => typed(resolve(e, leaves).type_oid),
            })
            .collect();
        overloads.0.resolve(self.kind, &self.name, &operands)
    }

    fn reads_leaf(&self) -> bool {
        self.args.iter().any(|a| matches!(a, Arg::Of(e) if reads_leaf(e)))
    }
}

impl Trusted {
    fn holds(&self, leaves: Option<&[DescribedColumn]>) -> bool {
        self.when.iter().any(|set| set.iter().all(|c| c.resolve(&[], leaves).is_some()))
    }

    fn reads_leaf(&self) -> bool {
        reads_leaf(&self.then) || reads_leaf(&self.otherwise) || self.when.iter().flatten().any(Check::reads_leaf)
    }
}

impl Call {
    /// The fact of the call, with its arguments' facts, where the check holds.
    fn fact(&self, leaves: Option<&[DescribedColumn]>) -> Option<(Fact, Vec<Fact>)> {
        let facts: Vec<Fact> = self.args.iter().map(|a| resolve(a, leaves)).collect();
        let result = self.check.resolve(&facts, leaves)?;
        let fact = match &self.claim {
            Claim::Every(first, ty) => {
                let read = &facts[..first.unwrap_or(facts.len()).min(facts.len())];
                Fact { non_null: !read.is_empty() && read.iter().all(|f| f.non_null), type_oid: result.or(ty.of(&facts)) }
            }
            Claim::NonNull(ty) => Fact { non_null: true, type_oid: ty.or(result) },
            Claim::Array(non_null) => Fact { non_null: *non_null, type_oid: None },
        };
        Some((fact, facts))
    }
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
    /// The operands' type when they all agree on a number type the built-in arithmetic has an
    /// operator of that type for (`int4 + int4` is `int4`), `%` having none for a float: an
    /// arithmetic operator over two values of one type, or a prefix sign.
    Arith { modulo: bool },
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
/// does. A rule whose name is shadowed proves nothing, but where its operands' types resolve the
/// call to the built-in ([`Trust::overloads`]).
#[derive(Debug, Clone, Default)]
pub struct Trust {
    /// Operator and function names with a definition outside the built-ins.
    pub shadowed: HashSet<String>,
    /// What the catalog says of the overloads of the shadowed names ([`crate::resolution`]): a
    /// call of such a name still reads as the built-in where its operands' types resolve it to
    /// one. `None` leaves every shadowed name proving nothing.
    pub overloads: Option<Shared>,
    /// The statement's parameter types in `$n` order, from its describe, which a parameter has
    /// at every read of it but the first; empty where the build has no describe of the statement.
    pub param_types: Vec<u32>,
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
    /// Whether `name`, an operator or a function a rule reads, can only resolve to a built-in,
    /// whatever its operands.
    pub fn builtin(&self, name: &str) -> bool {
        !self.shadowed.contains(name)
    }

    /// Whether a call of `name` over operands of these types resolves to the built-in, and the
    /// type it returns where the catalog says ([`crate::resolution::Overloads::resolve`]).
    pub fn resolves(&self, kind: Kind, name: &str, operands: &[Operand]) -> Option<Option<u32>> {
        // A cast is named by its target type, which no operator or function shadows: whether a
        // user's cast function may be the one is the cast targets' word.
        let checked = match kind {
            Kind::Cast => !self.cast(name),
            _ => self.checked(name),
        };
        if !checked {
            return Some(None);
        }
        self.overloads.as_ref()?.0.resolve(kind, name, operands)
    }

    /// Whether a call of `name` is the built-in only for some operands' types: a user or an
    /// extension defined the name, or an implicit cast between built-in types by a function of
    /// theirs, which a built-in's candidate may run on its operand.
    pub(crate) fn checked(&self, name: &str) -> bool {
        !self.builtin(name) || self.overloads.as_ref().is_some_and(|o| o.0.casts_implicitly())
    }

    /// Whether a call a strict claim rests on is the built-in only for some operands' types: a
    /// cast to a type a user's cast function may target, or an operator or a function [`Trust::checked`].
    pub(crate) fn call_checked(&self, call: &StrictCall) -> bool {
        match call.kind {
            Kind::Cast => !self.cast(&call.name),
            _ => self.checked(&call.name),
        }
    }

    /// A parameter as an operand: untyped where the statement first reads it, of its type after.
    pub fn param(&self, number: i32) -> Operand {
        Operand::Param(usize::try_from(number).ok().and_then(|n| self.param_types.get(n.checked_sub(1)?)).copied().filter(|t| *t != 0))
    }

    /// Whether a call of the function `name` with `arity` arguments aggregates, as the built-in
    /// does, whichever overload of it the server picks: none of a user's, or every one an aggregate.
    pub(crate) fn aggregates(&self, name: &str, arity: usize) -> bool {
        self.builtin(name) || self.overloads.as_ref().and_then(|o| o.0.every_aggregate(name, arity)) == Some(true)
    }

    /// Whether some row type of the catalog may have an attribute named `name`: as the trust
    /// query says, and any where it did not.
    pub(crate) fn row_attribute(&self, name: &str) -> bool {
        self.overloads.as_ref().is_none_or(|o| o.0.row_attribute(name))
    }

    /// Whether no overload of a user's of `name`, of this kind, returns a set.
    fn returns_no_set(&self, kind: Kind, name: &str) -> bool {
        self.builtin(name) || self.overloads.as_ref().and_then(|o| o.0.returns_set(kind, name)) == Some(false)
    }

    /// Whether no operator of a user's named `name`, infix or prefix, returns a set.
    fn operator_returns_no_set(&self, name: &str) -> bool {
        self.builtin(name) || self.overloads.as_ref().and_then(|o| o.0.operator_returns_set(name)) == Some(false)
    }

    /// The check a call of the shadowed `name` needs, where the catalog says what its overloads
    /// are: `None` where it does not, and the call proves nothing.
    fn check(&self, kind: Kind, name: &str, args: Vec<Arg>) -> Option<Check> {
        self.overloads.as_ref().map(|o| Check { kind, name: name.to_string(), args, overloads: Some(o.clone()) })
    }

    fn cast(&self, name: &str) -> bool {
        self.cast_targets.as_ref().is_none_or(|t| t.contains(name))
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
    // A write whose WITH clause has a query named like its target writes the table, where its leaf
    // probe, carrying the clause, would read the query by that name: the describe answers alone.
    if target_named_by_with(top, with.as_ref()) {
        return Plan::default();
    }
    // A grouping set nulls a grouping column in its super-aggregate rows, which a probe over the
    // `FROM` alone would never show: a leaf there proves nothing.
    let grouped = matches!(top, NodeEnum::SelectStmt(s) if s.group_clause.iter().any(|g| matches!(g.node, Some(NodeEnum::GroupingSet(_)))));
    let plain_group_by = matches!(top, NodeEnum::SelectStmt(s) if !grouped && !s.group_clause.is_empty());
    let windows: &[Node] = match top {
        NodeEnum::SelectStmt(s) => &s.window_clause,
        _ => &[],
    };
    let withs = Withs::default().with(with.as_ref(), columns);
    // The old and the new row of a RETURNING, with the relations a bare name is a column of first.
    let rows = Rows::of(top).map(|rows| (rows, Level::of(&from, &withs, columns)));
    let froms = (dialect == Dialect::Postgres).then(|| vec![Level::of(&from, &withs, columns)]);
    let fields = row_fields(&serde_json::to_value(&parsed.protobuf).unwrap_or_default());
    let mut reader = Reader { dialect, trust, columns, withs, rows: rows.as_ref(), leaves: (!grouped).then(Vec::new), grouped: plain_group_by, narrowed: Vec::new(), scope: froms, windows, fields: Some(&fields) };
    let scope = match top {
        NodeEnum::SelectStmt(s) if dialect == Dialect::Postgres && !grouped => Some(Scope::of(s, &mut reader)),
        _ => None,
    };
    // A column the WHERE narrows only by a call of a name a user also defined is narrowed in the
    // expressions that read it too: the leaf probe writes the statement's parameters as NULLs,
    // of no type, which check nothing there.
    if let Some(scope) = &scope {
        reader.narrowed.extend(scope.strict.iter().filter(|n| !n.checks.is_empty()).cloned());
    }
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
        Expr::JsonPath { .. } => match json_read(expr, leaves) {
            Some(read) => Fact { non_null: read.non_null, type_oid: Some(read.type_oid) },
            None => NOTHING,
        },
        Expr::Call(call) => call.fact(leaves).map_or(NOTHING, |(fact, _)| fact),
        Expr::Trusted(t) => resolve(if t.holds(leaves) { &t.then } else { &t.otherwise }, leaves),
        Expr::NotRow(inner, overloads) => {
            let fact = resolve(inner, leaves);
            Fact { non_null: fact.non_null && fact.type_oid.is_some_and(|t| is_scalar(t, overloads.as_ref())), ..fact }
        }
    }
}

/// The row types below `FirstGenbkiObjectId`, the same on PostgreSQL 14 to 18: `pg_type`,
/// `pg_attribute`, `pg_proc`, `pg_class`, `pg_database`, `pg_authid`, `pg_auth_members`,
/// `pg_shseclabel`, `pg_subscription`, and `record`. Every other catalog's row type has an oid of
/// 10000 or more, which `initdb` assigns.
const ROWS_BELOW_GENBKI: &[u32] = &[71, 75, 81, 83, 1248, 2249, 2842, 2843, 4066, 6101];
/// `FirstGenbkiObjectId`: every built-in type below it is one of the server's own source.
const FIRST_GENBKI_OID: u32 = 10000;

/// Whether a value of type `ty` is no row, which a call of one argument could read a column of
/// (`ParseFuncOrColumn`): as the trust query lists the row types and the types of a user's that are
/// none ([`crate::resolution::Overloads::is_scalar`]), or where it did not, a built-in type below
/// 10000 but those [`ROWS_BELOW_GENBKI`] names.
fn is_scalar(ty: u32, overloads: Option<&Shared>) -> bool {
    overloads.and_then(|o| o.0.is_scalar(ty)).unwrap_or(ty < FIRST_GENBKI_OID && !ROWS_BELOW_GENBKI.contains(&ty))
}

/// Whether an argument is no row whatever the build knows of its type, so that a call of one
/// argument over it is never a column of it: a constant, a parameter, a test, and a call of a
/// function or an operator the rules read, none of whose built-ins returns a row. The rules prove
/// such a call non-null only where it is the built-in, a projection of its own argument included
/// ([`Expr::NotRow`]); one they do not prove non-null leaves the call over it unproven too.
fn no_row(node: &Node) -> bool {
    use protobuf::AExprKind as K;
    match node.node.as_ref() {
        Some(NodeEnum::AConst(_) | NodeEnum::ParamRef(_) | NodeEnum::BoolExpr(_) | NodeEnum::NullTest(_) | NodeEnum::BooleanTest(_) | NodeEnum::SqlvalueFunction(_)) => true,
        // A scalar subquery is the value of its one column, which may be a row; `ARRAY(…)` is an
        // array, and the others are tests.
        Some(NodeEnum::SubLink(s)) => s.sub_link_type != protobuf::SubLinkType::ExprSublink as i32,
        Some(NodeEnum::FuncCall(f)) => builtin_name(&f.funcname).is_some_and(|n| !POSTGRES_ROW_RESULTS.contains(&n.as_str()) && postgres_function_names().contains(&n.as_str())),
        // An operator returns what its function does, a user's a row if they wrote it so, and
        // `LIKE` and `SIMILAR TO` are the operators `~~` and `~`; the rest test, and the server
        // refuses an `=` beside `ANY`, `IN` or `IS DISTINCT FROM` that returns no boolean.
        Some(NodeEnum::AExpr(a)) => match K::try_from(a.kind) {
            Ok(K::AexprOp | K::AexprLike | K::AexprIlike | K::AexprSimilar) => {
                builtin_name(&a.name).is_some_and(|o| POSTGRES_OPERATORS.contains(&o.as_str()) || POSTGRES_STRICT_OPERATORS.contains(&o.as_str()))
            }
            Ok(K::AexprNullif) | Err(_) => false,
            Ok(_) => true,
        },
        _ => false,
    }
}

/// What a JSON read at literal keys finds ([`Expr::JsonPath`]), given the leaf probe.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonRead {
    /// The canonical schema of the values found there ([`crate::schema_type`]).
    pub schema: serde_json::Value,
    /// Whether the read is never NULL: the column is not, each key's object holds the key for
    /// certain, and the text read finds no JSON null, which it answers with SQL NULL.
    pub non_null: bool,
    /// What the read gives: the column's own JSON type, or `text`.
    pub type_oid: u32,
    /// Whether the read gives text.
    pub text: bool,
}

impl JsonRead {
    /// The strings a text read gives, where the schema keeps the value to a list of strings (and
    /// a JSON null, which the read answers with SQL NULL), in an `enum` or a union of them: a
    /// JSON string reads as the string.
    pub fn strings(&self) -> Option<Vec<String>> {
        fn collect(schema: &serde_json::Value, out: &mut Vec<String>) -> Option<()> {
            if let Some(members) = schema.get("anyOf").and_then(serde_json::Value::as_array) {
                return members.iter().try_for_each(|m| collect(m, out));
            }
            for value in schema.get("enum")?.as_array()? {
                match value {
                    serde_json::Value::String(s) if !out.contains(s) => out.push(s.clone()),
                    serde_json::Value::String(_) | serde_json::Value::Null => {}
                    _ => return None,
                }
            }
            Some(())
        }
        let mut strings = Vec::new();
        collect(&self.schema, &mut strings)?;
        (!strings.is_empty()).then_some(strings)
    }
}

/// What an [`Expr::JsonPath`] finds: only a `json` or `jsonb` column a CHECK holds to a JSON
/// Schema says anything, each step reading that schema's object at its key as the server reads
/// the value's ([`crate::schema_type::at_path`]). `None` for any other read.
pub fn json_read(expr: &Expr, leaves: Option<&[DescribedColumn]>) -> Option<JsonRead> {
    let Expr::JsonPath { base, path, text, indexes } = expr else { return None };
    let (column, narrowed) = match base.as_ref() {
        Expr::Leaf(i) => (*i, false),
        Expr::Narrowed(inner) => match inner.as_ref() {
            Expr::Leaf(i) => (*i, true),
            _ => return None,
        },
        _ => return None,
    };
    let col = leaves?.get(column)?;
    if col.type_oid != oid::JSONB && col.type_oid != oid::JSON {
        return None;
    }
    let keys: Vec<&str> = path.iter().map(String::as_str).collect();
    let (schema, found) = crate::schema_type::at_path(col.document.as_ref()?, &keys, *indexes)?;
    let non_null = (narrowed || !col.nullable) && found && !(*text && crate::schema_type::admits(&schema, &serde_json::Value::Null));
    Some(JsonRead { schema, non_null, type_oid: if *text { oid::TEXT } else { col.type_oid }, text: *text })
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
            Expr::Nullable(inner) | Expr::Guarded(inner, _) | Expr::NotJsonb(inner) | Expr::Narrowed(inner) | Expr::NotRow(inner, _) => may_be_jsonb(inner, leaves),
            Expr::Every(operands, _) | Expr::Any(operands, _) => operands.iter().any(|o| may_be_jsonb(o, leaves)),
            // `->` and `#>` give the column's own JSON; `->>` and `#>>` text.
            Expr::JsonPath { text, .. } => !text,
            // A user's overload may return anything.
            Expr::Call(call) => match (call.fact(leaves), &call.claim) {
                (None, _) => true,
                (Some(_), Claim::Every(..)) => call.args.iter().any(|a| may_be_jsonb(a, leaves)),
                (Some(_), Claim::NonNull(_) | Claim::Array(_)) => false,
            },
            Expr::Trusted(t) => may_be_jsonb(if t.holds(leaves) { &t.then } else { &t.otherwise }, leaves),
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
        Expr::Array { items, .. } => array_items(items, leaves, is_array),
        Expr::Nullable(inner) | Expr::Guarded(inner, _) | Expr::NotJsonb(inner) | Expr::Narrowed(inner) | Expr::NotRow(inner, _) => items_nullable(inner, leaves, is_array),
        Expr::Every(operands, _) | Expr::Any(operands, _) => join_items(operands.iter().map(|o| items_nullable(o, leaves, is_array))),
        Expr::NonNull(_) | Expr::Unknown | Expr::Unread | Expr::Leaf(_) | Expr::Row(..) | Expr::JsonPath { .. } => None,
        // A user's overload may return any elements.
        Expr::Call(call) => match (call.fact(leaves), &call.claim) {
            (None, _) | (Some(_), Claim::NonNull(_)) => None,
            (Some(_), Claim::Array(_)) => array_items(&call.args, leaves, is_array),
            (Some(_), Claim::Every(..)) => join_items(call.args.iter().map(|o| items_nullable(o, leaves, is_array))),
        },
        Expr::Trusted(t) => items_nullable(if t.holds(leaves) { &t.then } else { &t.otherwise }, leaves, is_array),
    }
}

/// What an array built from `items` says of its elements ([`items_nullable`]).
fn array_items(items: &[Expr], leaves: Option<&[DescribedColumn]>, is_array: &dyn Fn(u32) -> bool) -> Option<bool> {
    join_items(items.iter().map(|e| match e {
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
    }))
}

/// Whether an expression reads a column the reader had no leaf probe for ([`Expr::Unread`]).
fn reads_unread(expr: &Expr) -> bool {
    match expr {
        Expr::Unread => true,
        Expr::Nullable(inner) | Expr::Guarded(inner, _) | Expr::NotJsonb(inner) | Expr::Narrowed(inner) | Expr::NotRow(inner, _) => reads_unread(inner),
        Expr::JsonPath { base, .. } => reads_unread(base),
        Expr::Every(operands, _) | Expr::Any(operands, _) => operands.iter().any(reads_unread),
        Expr::Array { items, .. } => items.iter().any(reads_unread),
        Expr::NonNull(_) | Expr::Unknown | Expr::Leaf(_) | Expr::Row(..) => false,
        Expr::Call(call) => call.args.iter().any(reads_unread),
        Expr::Trusted(t) => reads_unread(&t.then) || reads_unread(&t.otherwise),
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
                    // A user's overload of the name counts where none of them returns a set, and
                    // an aggregate's arguments only where every one of them aggregates.
                    let Some(n) = name(f.get("funcname")) else { return false };
                    let arity = f.get("args").and_then(Value::as_array).map_or(0, Vec::len);
                    if [FUNCTIONS, POSTGRES_GROUPED, POSTGRES_GROUPED_NEVER_NULL, POSTGRES_GROUPED_JSON].iter().any(|list| list.contains(&n)) && trust.aggregates(n, arity) {
                        return true;
                    }
                    if !trust.returns_no_set(Kind::Function, n) || POSTGRES_SET_RETURNING.contains(&n) || !postgres_function_names().contains(&n) {
                        return false;
                    }
                }
                if let Some(a) = map.get("AExpr") {
                    let between = [protobuf::AExprKind::AexprBetween, protobuf::AExprKind::AexprNotBetween, protobuf::AExprKind::AexprBetweenSym, protobuf::AExprKind::AexprNotBetweenSym];
                    // `BETWEEN` is named for itself and compares with `<=` and `>=` (`<` and `>` negated).
                    let built_in = if a.get("kind").and_then(Value::as_i64).is_some_and(|k| between.iter().any(|b| *b as i64 == k)) {
                        ["<", ">", "<=", ">="].iter().all(|o| trust.operator_returns_no_set(o))
                    } else {
                        name(a.get("name")).is_some_and(|o| (POSTGRES_OPERATORS.contains(&o) || POSTGRES_STRICT_OPERATORS.contains(&o)) && trust.operator_returns_no_set(o))
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
        Expr::NotRow(inner, overloads) => Expr::NotRow(Box::new(elements_not_jsonb(*inner)), overloads),
        Expr::Call(mut call) => {
            call.args = match call.claim {
                Claim::Array(_) => call.args.into_iter().map(|item| match item {
                    Expr::Array { .. } => elements_not_jsonb(item),
                    other => Expr::NotJsonb(Box::new(other)),
                }).collect(),
                _ => call.args.into_iter().map(elements_not_jsonb).collect(),
            };
            Expr::Call(call)
        }
        Expr::Trusted(mut t) => {
            t.then = elements_not_jsonb(t.then);
            t.otherwise = elements_not_jsonb(t.otherwise);
            Expr::Trusted(t)
        }
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
///
/// A call of one argument that Postgres may read as a column of a row counts only where it is an
/// aggregate for certain ([`aggregate_for_certain`]): `fields` are the columns the statement's own
/// rows may have ([`row_fields`]), `None` where the caller does not see the whole statement, and
/// `arg_type` gives the type of a column an argument reads where the caller knows it.
pub fn aggregates(expr: &Node, dialect: Dialect, trust: &Trust, level: Option<&Level>, fields: Option<&HashSet<String>>, arg_type: &dyn Fn(&Node) -> Option<u32>) -> bool {
    Reader { dialect, trust, columns: &Columns::default(), withs: Withs::default(), rows: None, leaves: None, grouped: false, narrowed: Vec::new(), scope: None, windows: &[], fields }.aggregates(expr, true, level, arg_type)
}

/// The aggregates the rules read that no built-in of takes a row on every server Reylo supports
/// (`min` and `max` of a `record` are PostgreSQL 16's): a call of one over a row that has a column
/// of its name is that column (`ParseFuncOrColumn`), a value per row and no aggregate.
const POSTGRES_PROJECTED_AGGREGATES: &[&str] = &["sum", "avg", "min", "max", "bit_and", "bit_or", "bool_and", "bool_or", "every"];

/// Whether Postgres may read a call as a column of its one argument, where the argument is a row
/// and no function of the name takes it (`ParseFuncOrColumn`'s projection): one argument, an
/// unqualified name, and nothing of an aggregate's or a window's around it.
fn projection_shaped(f: &protobuf::FuncCall) -> bool {
    f.args.len() == 1
        && f.funcname.len() == 1
        && f.agg_order.is_empty()
        && f.agg_filter.is_none()
        && !f.agg_star
        && !f.agg_distinct
        && f.over.is_none()
        && !f.func_variadic
        && !matches!(f.args[0].node, Some(NodeEnum::NamedArgExpr(_)))
}

/// Whether a call of the aggregate `name` is one for certain, rather than a column of its
/// argument: a call Postgres cannot read as a projection, or one whose argument is no row for
/// certain ([`no_row_for_certain`]), or is of a type that is none, or is a row of no type that has
/// a column of that name: no row type of the catalog has an attribute of it
/// ([`Trust::row_attribute`]) and no row the statement builds has a column of it (`fields`).
pub(crate) fn aggregate_for_certain(f: &protobuf::FuncCall, name: &str, dialect: Dialect, trust: &Trust, fields: Option<&HashSet<String>>, arg_type: &dyn Fn(&Node) -> Option<u32>) -> bool {
    if dialect != Dialect::Postgres || !POSTGRES_PROJECTED_AGGREGATES.contains(&name) || !projection_shaped(f) {
        return true;
    }
    let arg = &f.args[0];
    no_row_for_certain(arg, trust)
        || arg_type(arg).is_some_and(|t| is_scalar(t, trust.overloads.as_ref()))
        || (!trust.row_attribute(name) && fields.is_some_and(|fields| !fields.contains(name)))
}

/// Whether a value is no row whatever the statement's tables hold: a constant, a parameter, a test,
/// an array subquery, a built-in operator of a name no user also defined, and a call of a function
/// the rules read that no user also defined and that returns no row over an argument that is none
/// for certain. Unlike [`no_row`], nothing proves such a value non-null first.
fn no_row_for_certain(node: &Node, trust: &Trust) -> bool {
    use protobuf::AExprKind as K;
    match node.node.as_ref() {
        Some(NodeEnum::AConst(_) | NodeEnum::ParamRef(_) | NodeEnum::BoolExpr(_) | NodeEnum::NullTest(_) | NodeEnum::BooleanTest(_) | NodeEnum::SqlvalueFunction(_)) => true,
        Some(NodeEnum::SubLink(s)) => s.sub_link_type != protobuf::SubLinkType::ExprSublink as i32,
        Some(NodeEnum::AExpr(a)) => match K::try_from(a.kind) {
            Ok(K::AexprOp | K::AexprLike | K::AexprIlike | K::AexprSimilar) => builtin_name(&a.name)
                .is_some_and(|o| (POSTGRES_OPERATORS.contains(&o.as_str()) || POSTGRES_STRICT_OPERATORS.contains(&o.as_str())) && trust.builtin(&o)),
            Ok(K::AexprNullif) | Err(_) => false,
            Ok(_) => true,
        },
        // `lower` and `upper` of a range are its element, which may be a row.
        Some(NodeEnum::FuncCall(f)) => builtin_name(&f.funcname).is_some_and(|n| {
            !POSTGRES_ROW_RESULTS.contains(&n.as_str())
                && !["lower", "upper", "unnest"].contains(&n.as_str())
                && postgres_function_names().contains(&n.as_str())
                && trust.builtin(&n)
                && (!projection_shaped(f) || POSTGRES_ROW_TAKERS.contains(&n.as_str()) || no_row_for_certain(&f.args[0], trust))
        }),
        _ => false,
    }
}

/// The names of the columns a row the statement itself builds may have, which a call of one
/// argument over such a row may read: every column a subquery or a function in a FROM, a WITH
/// query or a RETURNING list shows, an alias's column list, and the columns `XMLTABLE` and
/// `JSON_TABLE` define. A target with no alias counts every name written in it, which is where the
/// server takes its column's name from (`FigureColname`), and a function in a FROM every name in
/// it. A table's and a composite type's columns are the catalog's ([`Trust::row_attribute`]).
pub fn row_fields(tree: &serde_json::Value) -> HashSet<String> {
    use serde_json::Value;
    fn strings(v: &Value, out: &mut HashSet<String>) {
        match v {
            Value::String(s) => {
                out.insert(s.clone());
            }
            Value::Object(map) => map.values().for_each(|c| strings(c, out)),
            Value::Array(items) => items.iter().for_each(|c| strings(c, out)),
            _ => {}
        }
    }
    /// The columns a query a FROM or a WITH reads shows: its targets', each arm's of a set
    /// operation, and a write's RETURNING list's.
    fn shown(v: &Value, out: &mut HashSet<String>) {
        let Some(map) = v.get("node").and_then(Value::as_object).and_then(|n| n.values().next()) else { return };
        for arm in ["larg", "rarg"] {
            if let Some(arm) = map.get(arm).filter(|a| !a.is_null()) {
                shown(&serde_json::json!({ "node": { "SelectStmt": arm } }), out);
            }
        }
        let returning = map.get("returning_clause").and_then(|c| c.get("exprs"));
        for list in [map.get("target_list"), returning] {
            for target in list.and_then(Value::as_array).into_iter().flatten() {
                let Some(rt) = target.pointer("/node/ResTarget") else { continue };
                match rt.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()) {
                    Some(name) => {
                        out.insert(name.to_string());
                    }
                    None => strings(rt.get("val").unwrap_or(&Value::Null), out),
                }
            }
        }
    }
    fn walk(v: &Value, out: &mut HashSet<String>) {
        match v {
            Value::Object(map) => {
                for (key, child) in map {
                    match key.as_str() {
                        "RangeSubselect" => shown(child.get("subquery").unwrap_or(&Value::Null), out),
                        "CommonTableExpr" => {
                            strings(child.get("aliascolnames").unwrap_or(&Value::Null), out);
                            shown(child.get("ctequery").unwrap_or(&Value::Null), out);
                        }
                        "RangeFunction" | "RangeTableFunc" | "JsonTable" => strings(child, out),
                        // A FROM item's alias is a field, `alias`, of the item; a join's too.
                        "alias" | "Alias" => strings(child.get("colnames").unwrap_or(&Value::Null), out),
                        _ => {}
                    }
                    walk(child, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|c| walk(c, out)),
            _ => {}
        }
    }
    let mut out = HashSet::new();
    walk(tree, &mut out);
    out
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
            Ty::Arith { modulo } => {
                let first = facts.first()?.type_oid?;
                let numbers: &[u32] = if modulo { &[oid::INT2, oid::INT4, oid::INT8, oid::NUMERIC] } else { NUMBER_TYPES };
                (numbers.contains(&first) && facts.iter().all(|f| f.type_oid == Some(first))).then_some(first)
            }
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
    /// whose WHEN is strict in them, or after a WHEN that tested them `IS NULL` and failed, and
    /// in a SELECT whose WHERE is strict in them by calls of names a user also defined.
    narrowed: Vec<Narrowing>,
    /// Every FROM in scope where the reader stands, the innermost last, where the reader knows
    /// them all: what says a bare name in a subquery's aggregate is an item's whole row
    /// ([`Level::within`]).
    scope: Option<Vec<Level>>,
    /// The `WINDOW` clause of the SELECT the reader stands in, which `OVER w` names a window of.
    windows: &'a [Node],
    /// The columns the statement's own rows may have ([`row_fields`]); `None` where the reader
    /// does not see the whole statement.
    fields: Option<&'a HashSet<String>>,
}

/// A column, as written, that a test is strict in, where every check holds: one per call of a
/// name a user also defined that the test rests on, none where it rests on built-ins alone.
#[derive(Debug, Clone)]
struct Narrowing {
    column: Vec<String>,
    checks: Vec<Check>,
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
                    // A literal past int4, or one with a fraction or an exponent: Postgres's
                    // `numeric` where it is no integer, SQLite's REAL.
                    Some(protobuf::a_const::Val::Fval(f)) if self.dialect == Dialect::Postgres => postgres_number(&f.fval),
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
                let Some(fields) = fields else { return read };
                let when: Vec<Vec<Check>> = self.narrowed.iter().filter(|n| n.column == fields).map(|n| n.checks.clone()).collect();
                if when.is_empty() {
                    read
                } else if when.iter().any(Vec::is_empty) {
                    Expr::Narrowed(Box::new(read))
                } else {
                    Expr::Trusted(Box::new(Trusted { when, then: Expr::Narrowed(Box::new(read.clone())), otherwise: read }))
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
                    // A type a user's cast function targets: the cast is a built-in one where the
                    // operand's type is none of that function's sources.
                    Dialect::Postgres => {
                        let operand = if POSTGRES_JSONB_NULL_CASTS.contains(&name.as_str()) && !self.trust.jsonb_casts_raise { Expr::NotJsonb(Box::new(operand)) } else { operand };
                        self.shadowed_call(Kind::Cast, &name, &[arg], vec![operand], Claim::Every(None, Ty::Unknown))
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
                    // `a IS [NOT] DISTINCT FROM b` compares two non-null values with `=`, and is
                    // NULL where that `=` answers NULL: a user's of the name may. With a NULL on
                    // either side no `=` runs.
                    let null = |n: Option<&Node>| matches!(n.and_then(|n| n.node.as_ref()), Some(NodeEnum::AConst(c)) if c.isnull);
                    let (Some(left), Some(right)) = (a.lexpr.as_deref(), a.rexpr.as_deref()) else { return Expr::Unknown };
                    if self.dialect != Dialect::Postgres || !self.trust.checked("=") || null(Some(left)) || null(Some(right)) {
                        return Expr::NonNull(Some(oid::BOOL));
                    }
                    let operands = vec![self.read(left), self.read(right)];
                    return self.shadowed_call(Kind::Infix, "=", &[left, right], operands, Claim::NonNull(Some(oid::BOOL)));
                }
                // `LIKE`, `ILIKE` and `SIMILAR TO` are operators the parse marks apart, under the
                // operator's own name.
                let matching = [protobuf::AExprKind::AexprLike as i32, protobuf::AExprKind::AexprIlike as i32, protobuf::AExprKind::AexprSimilar as i32];
                if a.kind != protobuf::AExprKind::AexprOp as i32 && !(self.dialect == Dialect::Postgres && matching.contains(&a.kind)) {
                    return Expr::Unknown;
                }
                let Some(op) = builtin_name(&a.name) else { return Expr::Unknown };
                if self.dialect == Dialect::Postgres && matches!(op.as_str(), "->" | "->>" | "#>" | "#>>") {
                    return self.json_path(&op, a);
                }
                let accepted = match self.dialect {
                    Dialect::Postgres => POSTGRES_OPERATORS.contains(&op.as_str()),
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
                    "+" | "-" | "*" | "/" if self.dialect == Dialect::Postgres => Ty::Arith { modulo: false },
                    "%" if self.dialect == Dialect::Postgres && left.is_some() => Ty::Arith { modulo: true },
                    _ => Ty::Unknown,
                };
                let mut operands: Vec<Expr> = left.into_iter().map(|l| self.read(l)).collect();
                operands.push(self.read(right));
                // A user's operator of the name proves nothing, unless the operands' types
                // resolve the call to the built-in.
                if self.dialect == Dialect::Postgres && self.trust.checked(&op) {
                    let nodes: Vec<&Node> = left.into_iter().chain([right]).collect();
                    let kind = if left.is_some() { Kind::Infix } else { Kind::Prefix };
                    return self.shadowed_call(kind, &op, &nodes, operands, Claim::Every(None, ty));
                }
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
                        let strict = self.narrowings(test);
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
                            self.narrowed.extend(fields.map(|column| Narrowing { column, checks: Vec::new() }));
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
            let strict = self.narrowings(filter);
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
        // A call of one argument no function matches is a column of it where the argument is a
        // row: a function of none (`now(t)`) is then that column alone.
        let projection = self.projection(f, &name);
        if projection && POSTGRES_NO_ARGUMENT.contains(&name.as_str()) {
            return Expr::Unknown;
        }
        // A function a user also defined proves what the built-in does where the arguments'
        // types resolve the call to it. `unnest` reads its argument's elements, of no type the
        // build can name, and so proves nothing then.
        if self.dialect == Dialect::Postgres && self.trust.checked(&name) {
            let Some((claim, guard)) = self.builtin_claim(f, &name) else { return Expr::Unknown };
            let nodes: Vec<&Node> = f.args.iter().collect();
            let mut args: Vec<Expr> = self.args(f, projection);
            if let (Some(types), Some(first)) = (guard, args.first_mut()) {
                *first = Expr::Guarded(Box::new(first.clone()), types);
            }
            return self.shadowed_call(Kind::Function, &name, &nodes, args, claim);
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
            // Under a plain GROUP BY every group has a row, and a window frame that always holds
            // the current row has that row, so there an aggregate that is NULL only over no rows
            // or only NULLs follows its argument. A FILTER can leave it no rows, any other frame
            // can be empty, and SQLite's sum of `1e999` and `-1e999` is NaN, stored as NULL.
            (Dialect::Postgres, n, 1 | 2) if f.agg_filter.is_none() && self.has_a_row(f) && self.trust.builtin(n) && (POSTGRES_GROUPED.contains(&n) || POSTGRES_GROUPED_NEVER_NULL.contains(&n) || POSTGRES_GROUPED_JSON.contains(&n)) => {
                if POSTGRES_GROUPED_JSON.contains(&n) {
                    Expr::NonNull(None)
                } else if POSTGRES_GROUPED_NEVER_NULL.contains(&n) {
                    Expr::Array { non_null: true, items: f.args.iter().take(1).map(|a| self.read(a)).collect() }
                } else {
                    Expr::Every(self.args(f, projection).into_iter().take(1).collect(), Ty::Unknown)
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
            (Dialect::Postgres, n, 1..) if POSTGRES_FIRST_ARG.contains(&n) && self.trust.builtin(n) => Expr::Every(self.args(f, projection).into_iter().take(1).collect(), Ty::Is(oid::TEXT)),
            (Dialect::Postgres, n, 1..) if self.trust.builtin(n) && POSTGRES_TOTAL_FOR.iter().any(|(name, _)| *name == n) => {
                let types = POSTGRES_TOTAL_FOR.iter().find(|(name, _)| *name == n).map(|(_, t)| *t).unwrap_or(&[]);
                let mut operands: Vec<Expr> = f.args.iter().map(|a| self.read(a)).collect();
                operands[0] = Expr::Guarded(Box::new(operands[0].clone()), types);
                Expr::Every(operands, Ty::Unknown)
            }
            (Dialect::Postgres, n, _) if POSTGRES_TOTAL.contains(&n) && self.trust.builtin(n) => {
                let ty = if POSTGRES_JSONB.contains(&n) { Ty::Is(oid::JSONB) } else { Ty::Unknown };
                Expr::Every(self.args(f, projection), ty)
            }
            _ => match self.trust.total.get(name.as_str()) {
                Some(&ty) => Expr::Every(self.args(f, projection), Ty::Is(ty)),
                None => Expr::Unknown,
            },
        }
    }

    /// Whether an aggregate call always has a row to aggregate: a plain one under a plain `GROUP
    /// BY`, and a window call whose frame always holds the current row
    /// ([`frame_holds_current_row`]).
    fn has_a_row(&self, f: &protobuf::FuncCall) -> bool {
        match f.over.as_deref() {
            Some(over) => frame_holds_current_row(over, self.windows),
            None => self.grouped,
        }
    }

    /// A read of JSON at literal keys with the built-in `->`, `->>`, `#>` or `#>>`: an
    /// [`Expr::JsonPath`] over a column, a read of a read at more keys extending it. A key that
    /// is not a literal (a parameter, an expression, an integer that indexes an array), and a read
    /// of anything but a column, claims nothing.
    fn json_path(&mut self, op: &str, a: &protobuf::AExpr) -> Expr {
        let (Some(left), Some(right)) = (a.lexpr.as_deref(), a.rexpr.as_deref()) else { return Expr::Unknown };
        if !self.trust.builtin(op) {
            return Expr::Unknown;
        }
        let indexes = op.starts_with('#');
        let keys = if indexes { text_array(right) } else { string_literal(right).map(|key| vec![key]) };
        let Some(keys) = keys else { return Expr::Unknown };
        let text = op.ends_with(">>");
        match self.read(left) {
            Expr::JsonPath { base, mut path, text: false, indexes: before } => {
                path.extend(keys);
                Expr::JsonPath { base, path, text, indexes: before || indexes }
            }
            base @ Expr::Leaf(_) => Expr::JsonPath { base: Box::new(base), path: keys, text, indexes },
            Expr::Narrowed(inner) if matches!(*inner, Expr::Leaf(_)) => Expr::JsonPath { base: Box::new(Expr::Narrowed(inner)), path: keys, text, indexes },
            _ => Expr::Unknown,
        }
    }

    /// Whether Postgres may read this call of `name` as a column of its one argument, where that
    /// argument is a row and no function matches (`ParseFuncOrColumn`'s projection): one argument,
    /// an unqualified name, nothing of an aggregate's or a window's around it, and a name whose
    /// built-ins take no row, which would match one.
    fn projection(&self, f: &protobuf::FuncCall, name: &str) -> bool {
        self.dialect == Dialect::Postgres && projection_shaped(f) && !POSTGRES_ROW_TAKERS.contains(&name)
    }

    /// A call's arguments, the one of a call that may be a column of it read as [`Expr::NotRow`]
    /// unless it is no row whatever its type ([`no_row`]).
    fn args(&mut self, f: &protobuf::FuncCall, projection: bool) -> Vec<Expr> {
        let mut args: Vec<Expr> = f.args.iter().map(|a| self.read(a)).collect();
        if let (true, Some(first)) = (projection && !no_row(&f.args[0]), args.first_mut()) {
            *first = Expr::NotRow(Box::new(first.clone()), self.trust.overloads.clone());
        }
        args
    }

    /// What the rule of a Postgres built-in says of a call of `name`, as [`Reader::call_unfiltered`]
    /// reads it, in terms of its arguments, with the types of its first argument for which it
    /// holds; `None` where no rule reads the call.
    fn builtin_claim(&self, f: &protobuf::FuncCall, name: &str) -> Option<(Claim, Option<&'static [u32]>)> {
        let window = f.over.is_some();
        Some(match (name, f.args.len()) {
            ("count", 0 | 1) => (Claim::NonNull(Some(oid::INT8)), None),
            ("row_number" | "rank" | "dense_rank", 0) if window => (Claim::NonNull(Some(oid::INT8)), None),
            ("percent_rank" | "cume_dist", 0) if window => (Claim::NonNull(Some(oid::FLOAT8)), None),
            ("ntile", 1) if window => (Claim::Every(None, Ty::Is(oid::INT4)), None),
            ("lag" | "lead", 3) if window => (Claim::Every(None, Ty::Unknown), None),
            (n, 1 | 2) if self.grouped && !window && f.agg_filter.is_none() && (POSTGRES_GROUPED.contains(&n) || POSTGRES_GROUPED_NEVER_NULL.contains(&n) || POSTGRES_GROUPED_JSON.contains(&n)) => {
                if POSTGRES_GROUPED_JSON.contains(&n) {
                    (Claim::NonNull(None), None)
                } else if POSTGRES_GROUPED_NEVER_NULL.contains(&n) {
                    (Claim::Array(true), None)
                } else {
                    (Claim::Every(Some(1), Ty::Unknown), None)
                }
            }
            ("array_agg", 1) => (Claim::Array(false), None),
            (n, _) if POSTGRES_NEVER_NULL.contains(&n) => (Claim::NonNull(None), None),
            ("uuidv7", 0) => (Claim::NonNull(Some(oid::UUID)), None),
            (n, 1..) if POSTGRES_FIRST_ARG.contains(&n) => (Claim::Every(Some(1), Ty::Is(oid::TEXT)), None),
            (n, 1..) if POSTGRES_TOTAL_FOR.iter().any(|(name, _)| *name == n) => (Claim::Every(None, Ty::Unknown), POSTGRES_TOTAL_FOR.iter().find(|(name, _)| *name == n).map(|(_, t)| *t)),
            (n, _) if POSTGRES_TOTAL.contains(&n) => (Claim::Every(None, if POSTGRES_JSONB.contains(&n) { Ty::Is(oid::JSONB) } else { Ty::Unknown }), None),
            _ => return None,
        })
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
        let scope = self.scope.as_ref().map(|outside| outside.iter().cloned().chain([level.clone()]).collect());
        let level = match &self.scope {
            Some(outside) => level.within(outside.clone()),
            None => level,
        };
        let mut inner_reader = Reader { dialect: self.dialect, trust: self.trust, columns: self.columns, withs, rows: None, leaves: None, grouped: false, narrowed: Vec::new(), scope, windows: &select.window_clause, fields: self.fields };
        let value = inner_reader.read(inner);
        if one_row(select, inner, self.dialect, self.trust) || (keeps_one_row(select, inner, self.dialect, self.trust) && self.aggregates(inner, false, Some(&level), &|n| level.column_type(n, self.columns))) {
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
    fn aggregates(&self, expr: &Node, ordered_sets: bool, from: Option<&Level>, arg_type: &dyn Fn(&Node) -> Option<u32>) -> bool {
        let children: Vec<&Node> = match expr.node.as_ref() {
            Some(NodeEnum::FuncCall(f)) => {
                let aggregate = f.over.is_none()
                    && ((ordered_sets && self.dialect == Dialect::Postgres && f.agg_within_group)
                        || builtin_name(&f.funcname).is_some_and(|name| {
                            let arity = match name.as_str() {
                                "count" => f.args.len() <= 1,
                                "sum" | "avg" | "min" | "max" | "json_agg" | "jsonb_agg" => f.args.len() == 1,
                                // Postgres's other aggregates the rules read, each at its one arity.
                                "array_agg" | "bit_and" | "bit_or" | "bool_and" | "bool_or" | "every" => self.dialect == Dialect::Postgres && f.args.len() == 1,
                                "string_agg" | "json_object_agg" | "jsonb_object_agg" => self.dialect == Dialect::Postgres && f.args.len() == 2,
                                "total" | "json_group_array" => self.dialect == Dialect::Sqlite && f.args.len() == 1,
                                _ => false,
                            };
                            arity && self.trust.aggregates(&name, f.args.len()) && aggregate_for_certain(f, &name, self.dialect, self.trust, self.fields, arg_type)
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
        children.into_iter().any(|c| self.aggregates(c, ordered_sets, from, arg_type))
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

thread_local! {
    static READS: std::cell::RefCell<Vec<(String, Option<(Vec<ItemRead>, Vec<ItemQuery>)>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Per target of the select list, what it reads of the FROM items that are statements of their
/// own, and those items. `None` when the statement is not a plain SELECT that reads one, or when
/// a `(row).*` hides how many columns a target is.
pub fn item_reads(sql: &str) -> Option<(Vec<ItemRead>, Vec<ItemQuery>)> {
    let (reads, queries) = crate::stack::kept(&READS, sql, || crate::stack::run(|| reads_here(sql)))?;
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
                    // A WITH query's body sees the queries before it, and in a recursive WITH
                    // every one: a later query's name, or its own, is a table's there.
                    let scope = self.with.map(|w| {
                        let mut w = w.clone();
                        if !w.recursive {
                            let at = w.ctes.iter().position(|c| matches!(c.node.as_ref(), Some(NodeEnum::CommonTableExpr(c)) if c.ctename == cte.ctename)).unwrap_or(0);
                            w.ctes.truncate(at);
                        }
                        w
                    }).filter(|w| !w.ctes.is_empty());
                    let mut names = columns(rv.alias.as_ref());
                    let own: Vec<String> = cte.aliascolnames.iter().filter_map(|n| match n.node.as_ref() {
                        Some(NodeEnum::String(s)) => Some(s.sval.clone()),
                        _ => None,
                    }).collect();
                    names.extend(own.into_iter().skip(names.len()));
                    Some(ItemQuery { sql: arm_text(self.version, body, scope.as_ref())?, columns: names, with: Some(cte.ctename.clone()), nulled })
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
    strict: Vec<Narrowing>,
}

impl Scope {
    fn of(s: &SelectStmt, reader: &mut Reader) -> Scope {
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
        let strict = s.where_clause.as_deref().map(|w| reader.narrowings(w)).unwrap_or_default();
        Scope { tables, names, using, natural, strict }
    }

    /// The claim for a target that is a bare column reference, if the SELECT proves one.
    fn column(&self, c: &protobuf::ColumnRef, node: &Node, reader: &mut Reader) -> Option<Expr> {
        let fields: Vec<String> = c.fields.iter().map(|f| match f.node.as_ref() {
            Some(NodeEnum::String(s)) => Some(s.sval.clone()),
            _ => None,
        }).collect::<Option<_>>()?;
        let when: Vec<Vec<Check>> = self.strict.iter().filter(|tested| self.same_column(&tested.column, &fields)).map(|tested| tested.checks.clone()).collect();
        if when.iter().any(Vec::is_empty) {
            return Some(Expr::NonNull(None));
        }
        let row = (|| {
            let [name] = fields.as_slice() else { return None };
            let relation = self.tables.get(name)?;
            let leaves = reader.leaves.as_mut()?;
            leaves.push(node.clone());
            Some(Expr::Row(leaves.len() - 1, relation.clone()))
        })();
        if when.is_empty() {
            return row;
        }
        // Narrowed by calls a user's overload may be the one of: where they resolve otherwise.
        Some(Expr::Trusted(Box::new(Trusted { when, then: Expr::NonNull(None), otherwise: row.unwrap_or(Expr::Unknown) })))
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

impl Reader<'_> {
    /// The column references a test is strict in at its top level ([`strict_in`]), where every
    /// cast it rests on keeps its argument's nullability, and every operator and function it rests
    /// on is the built-in one, or else resolves to it for its operands' types: each such call is
    /// a check of the [`Narrowing`]. A row the test passes has that column not NULL.
    fn narrowings(&mut self, node: &Node) -> Vec<Narrowing> {
        let mut out: Vec<Narrowing> = Vec::new();
        for strict in strict_in(node) {
            if out.iter().any(|n| n.column == strict.column && n.checks.is_empty()) {
                continue;
            }
            let mut checks = Vec::new();
            if strict.calls.iter().any(|c| self.trust.call_checked(c)) && (self.dialect != Dialect::Postgres || self.trust.overloads.is_none()) {
                continue;
            }
            for call in &strict.calls {
                if !self.trust.call_checked(call) {
                    continue;
                }
                let args = call.args.iter().map(|a| self.operand(a)).collect();
                checks.extend(self.trust.check(call.kind, &call.name, args));
            }
            if checks.is_empty() {
                out.retain(|n| n.column != strict.column);
            }
            out.push(Narrowing { column: strict.column, checks });
        }
        out
    }

    /// An operand of a check that stands apart from any call the reader builds: what the SQL says
    /// of its type, or the expression read.
    fn operand(&mut self, node: &Node) -> Arg {
        match self.fixed(node) {
            Some(fixed) => Arg::Fixed(fixed),
            None => Arg::Of(self.read(node)),
        }
    }

    /// An operand's type as the server reads it, where the SQL alone says it: a string literal is
    /// untyped, a NULL is anything a probe wrote in place of a parameter, a parameter is untyped on
    /// its first read and typed after, and a number or a boolean is typed as `make_const` types it.
    fn fixed(&self, node: &Node) -> Option<Operand> {
        fixed_operand(node, self.trust)
    }

    /// A call of the shadowed `name` over `nodes` read as `args`: the claim, where the arguments'
    /// types resolve it to the built-in.
    fn shadowed_call(&self, kind: Kind, name: &str, nodes: &[&Node], args: Vec<Expr>, claim: Claim) -> Expr {
        let operands = nodes.iter().enumerate().map(|(i, n)| self.fixed(n).map_or(Arg::At(i), Arg::Fixed)).collect();
        match self.trust.check(kind, name, operands) {
            Some(check) => Expr::Call(Box::new(Call { check, args, claim })),
            None => Expr::Unknown,
        }
    }
}

/// An operand's type as the server reads it, where the SQL alone says it ([`Reader::fixed`]).
pub(crate) fn fixed_operand(node: &Node, trust: &Trust) -> Option<Operand> {
    match node.node.as_ref()? {
        NodeEnum::AConst(c) if c.isnull => Some(Operand::Opaque),
        NodeEnum::AConst(c) => Some(match c.val.as_ref() {
            Some(protobuf::a_const::Val::Sval(_)) => Operand::Unknown,
            Some(protobuf::a_const::Val::Ival(_)) => Operand::Is(oid::INT4),
            Some(protobuf::a_const::Val::Fval(f)) => postgres_number(&f.fval).map_or(Operand::Opaque, Operand::Is),
            Some(protobuf::a_const::Val::Boolval(_)) => Operand::Is(oid::BOOL),
            Some(protobuf::a_const::Val::Bsval(_)) => Operand::Is(BIT),
            None => Operand::Opaque,
        }),
        NodeEnum::ParamRef(p) => Some(trust.param(p.number)),
        _ => None,
    }
}

/// `bit`, the type of a bit-string literal.
const BIT: u32 = 1560;

/// The type Postgres gives a numeric literal the parse keeps as text (`make_const`): `int4` where
/// it fits, `int8`, and `numeric` for a fraction, an exponent or an integer past `int8`. A
/// literal written otherwise (hexadecimal, with underscores) the build does not type.
pub(crate) fn postgres_number(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        return Some(match text.parse::<i64>() {
            Ok(n) if i32::try_from(n).is_ok() => oid::INT4,
            Ok(_) => oid::INT8,
            Err(_) => oid::NUMERIC,
        });
    }
    let (mantissa, exponent) = match digits.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e.strip_prefix(['+', '-']).unwrap_or(e))),
        None => (digits, None),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits_only = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    let valid = !(whole.is_empty() && fraction.is_empty()) && digits_only(whole) && digits_only(fraction) && exponent.is_none_or(|e| !e.is_empty() && digits_only(e));
    valid.then_some(oid::NUMERIC)
}

/// A column a predicate cannot pass with NULL in it, and the built-in operators, functions and
/// casts that claim rests on, each with its operands: a user's of one of those names may hold for
/// NULL, so a caller drops the claim where a call may resolve to one ([`Trust::call_checked`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Strict {
    pub column: Vec<String>,
    pub calls: Vec<StrictCall>,
}

/// One call a [`Strict`] claim rests on: a cast by its target type's name.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StrictCall {
    pub kind: Kind,
    pub name: String,
    pub args: Vec<Node>,
}

impl Strict {
    fn calling(mut self, kind: Kind, name: &str, args: Vec<Node>) -> Strict {
        self.calls.push(StrictCall { kind, name: name.to_string(), args });
        self
    }
}

/// The columns a predicate at the top level of a WHERE, or of an inner join's ON clause, is strict
/// in: a row with NULL in one of them does not pass. Under AND, either side's; under OR, the ones
/// every arm is strict in (`nickname like 'a%' or nickname like 'b%'`); `IS NOT NULL`; NOT over a
/// test strict in it, or over `IS NULL`; a comparison, `IN`, `BETWEEN`, `LIKE`, `ILIKE` or
/// `SIMILAR TO`, in the columns its operands answer NULL for ([`nulls_with`]), which reach
/// through strict functions and operators (`lower(nickname) = $1`); a bare boolean expression.
/// The planner's `find_nonnullable_vars` reads the same shapes.
pub(crate) fn strict_in(node: &Node) -> Vec<Strict> {
    // Each claim rests on these calls of the infix operator `name` too.
    let called = |found: Vec<Strict>, name: &str, pairs: &[(&Node, &Node)]| -> Vec<Strict> {
        found
            .into_iter()
            .map(|mut f| {
                f.calls.extend(pairs.iter().map(|(l, r)| StrictCall { kind: Kind::Infix, name: name.to_string(), args: vec![(*l).clone(), (*r).clone()] }));
                f
            })
            .collect()
    };
    let list = |n: Option<&Node>| -> Vec<Node> {
        match n.and_then(|n| n.node.as_ref()) {
            Some(NodeEnum::List(l)) => l.items.clone(),
            Some(_) => n.into_iter().cloned().collect(),
            None => Vec::new(),
        }
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
                        k.calls.extend(other.calls.iter().cloned());
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
            let Some(l) = e.lexpr.as_deref() else { return test_nulls_with(node) };
            if kind == protobuf::AExprKind::AexprIn as i32 {
                // `x IN (…)` and `x NOT IN (…)` are NULL for a NULL x. The server compares `x`
                // with each value, or with all of them as an array of a type common to them and
                // to `x`, which is one of theirs: either way a comparison of `x` with a value of
                // its own type or with each value.
                let items = list(e.rexpr.as_deref());
                let mut pairs: Vec<(&Node, &Node)> = vec![(l, l)];
                pairs.extend(items.iter().map(|i| (l, i)));
                called(nulls_with(l), &op, &pairs)
            } else if [protobuf::AExprKind::AexprBetween as i32, protobuf::AExprKind::AexprNotBetween as i32].contains(&kind) {
                // `x BETWEEN a AND b` is `x >= a AND x <= b`, and NOT BETWEEN `x < a OR x > b`.
                let bounds = list(e.rexpr.as_deref());
                let [low, high] = bounds.as_slice() else { return Vec::new() };
                let (below, above) = if kind == protobuf::AExprKind::AexprBetween as i32 { (">=", "<=") } else { ("<", ">") };
                called(called(nulls_with(l), below, &[(l, low)]), above, &[(l, high)])
            } else if [protobuf::AExprKind::AexprLike as i32, protobuf::AExprKind::AexprIlike as i32, protobuf::AExprKind::AexprSimilar as i32].contains(&kind) {
                let mut found: Vec<Strict> = nulls_with(l);
                if kind != protobuf::AExprKind::AexprSimilar as i32 {
                    found.extend(e.rexpr.as_deref().map(nulls_with).unwrap_or_default());
                }
                match e.rexpr.as_deref() {
                    Some(r) => called(found, &op, &[(l, r)]),
                    None => Vec::new(),
                }
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
            fields.map(|column| vec![Strict { column, calls: Vec::new() }]).unwrap_or_default()
        }
        Some(NodeEnum::AExpr(e)) if e.kind == protobuf::AExprKind::AexprOp as i32 => {
            let Some(op) = builtin_name(&e.name).filter(|o| POSTGRES_STRICT_OPERATORS.contains(&o.as_str())) else { return Vec::new() };
            let operands: Vec<Node> = e.lexpr.iter().chain(e.rexpr.iter()).map(|o| (**o).clone()).collect();
            let kind = if e.lexpr.is_some() { Kind::Infix } else { Kind::Prefix };
            operands.iter().flat_map(nulls_with).map(|f| f.calling(kind, &op, operands.clone())).collect()
        }
        Some(NodeEnum::FuncCall(f)) if f.over.is_none() && f.agg_filter.is_none() && !f.agg_star && !f.agg_distinct && !f.func_variadic && f.agg_order.is_empty() => {
            let Some(name) = builtin_name(&f.funcname).filter(|n| POSTGRES_STRICT.contains(&n.as_str())) else { return Vec::new() };
            f.args.iter().flat_map(nulls_with).map(|s| s.calling(Kind::Function, &name, f.args.clone())).collect()
        }
        Some(NodeEnum::TypeCast(t)) => {
            let target = t.type_name.as_ref().filter(|n| n.array_bounds.is_empty() && !n.setof && !n.pct_type).and_then(|n| builtin_name(&n.names));
            match (target.filter(|n| POSTGRES_CASTS.iter().any(|(c, _)| c == n)), t.arg.as_deref()) {
                (Some(target), Some(arg)) => nulls_with(arg)
                    .into_iter()
                    .map(|mut f| {
                        f.calls.push(StrictCall { kind: Kind::Cast, name: target.clone(), args: vec![arg.clone()] });
                        f
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// Whether the frame of an `OVER` clause always holds the current row ([`holds_current_row`]),
/// given the `WINDOW` clause of the SELECT it stands in. `OVER w` takes the frame of the window
/// `w` that clause defines, and nothing where it defines none; `OVER (w …)` copies `w`'s
/// partitioning and ordering but "always uses its own frame clause" (the server refuses to copy a
/// window that has one), as a window defined `AS (w …)` does.
pub(crate) fn frame_holds_current_row(over: &protobuf::WindowDef, windows: &[Node]) -> bool {
    let frame = if over.name.is_empty() {
        Some(over.frame_options)
    } else {
        windows.iter().find_map(|w| match w.node.as_ref() {
            Some(NodeEnum::WindowDef(w)) if w.name == over.name => Some(w.frame_options),
            _ => None,
        })
    };
    frame.is_some_and(holds_current_row)
}

/// Whether a window frame, as the parser's `frameOptions` bits spell it (`FRAMEOPTION_*` in
/// `parsenodes.h`), holds the current row of every row it is computed for. The default frame,
/// `RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW`, runs through the current row's last peer,
/// and the whole partition without `ORDER BY`. A frame that starts `UNBOUNDED PRECEDING`, `n
/// PRECEDING` or at `CURRENT ROW` and ends at `CURRENT ROW`, `n FOLLOWING` or `UNBOUNDED
/// FOLLOWING` holds it in every mode: an offset "must be non-null and non-negative" (or the
/// statement fails), and in RANGE mode the current row's value is no distance from itself: a
/// NULL is a peer of the NULLs, and a NaN or an infinity, an infinite offset from it included,
/// reaches its own peers (measured on PostgreSQL 18 over float8, numeric and timestamptz). A start
/// `n FOLLOWING` or an end `n PRECEDING` can leave the row out, and `EXCLUDE CURRENT ROW` and
/// `EXCLUDE GROUP` do; `EXCLUDE TIES` keeps it. The two bounds the parser refuses (a start
/// `UNBOUNDED FOLLOWING`, an end `UNBOUNDED PRECEDING`) are refused here too.
fn holds_current_row(options: i32) -> bool {
    const START_OFFSET_FOLLOWING: i32 = 0x02000;
    const END_OFFSET_PRECEDING: i32 = 0x01000;
    const START_UNBOUNDED_FOLLOWING: i32 = 0x00080;
    const END_UNBOUNDED_PRECEDING: i32 = 0x00040;
    const EXCLUDE_CURRENT_ROW: i32 = 0x08000;
    const EXCLUDE_GROUP: i32 = 0x10000;
    options & (START_OFFSET_FOLLOWING | END_OFFSET_PRECEDING | START_UNBOUNDED_FOLLOWING | END_UNBOUNDED_PRECEDING | EXCLUDE_CURRENT_ROW | EXCLUDE_GROUP) == 0
}

/// A bare `VALUES (…), (…)` arm: a column is non-null when it is in every row.
fn values_plan(s: &SelectStmt, dialect: Dialect, trust: &Trust, columns: &Columns) -> Plan {
    let withs = Withs::default().with(s.with_clause.as_ref(), columns);
    let mut reader = Reader { dialect, trust, columns, withs, rows: None, leaves: None, grouped: false, narrowed: Vec::new(), scope: None, windows: &[], fields: None };
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
    match (&arm.with_clause, with) {
        (None, _) => arm.with_clause = with.cloned(),
        // The arm's own WITH would take the place of the one around it, whose queries the arm may
        // read: a name of one would be a table's.
        (Some(_), Some(_)) => return None,
        (Some(_), None) => {}
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

/// A string literal, bare or cast to `text` or `varchar`: one key of `->`. A cast to anything
/// else is not one (`'0'::int` indexes an array).
fn string_literal(node: &Node) -> Option<String> {
    match node.node.as_ref() {
        Some(NodeEnum::AConst(c)) => match c.val.as_ref() {
            Some(protobuf::a_const::Val::Sval(s)) => Some(s.sval.clone()),
            _ => None,
        },
        Some(NodeEnum::TypeCast(t)) => {
            let target = t.type_name.as_ref().filter(|n| n.array_bounds.is_empty()).and_then(|n| builtin_name(&n.names))?;
            if target != "text" && target != "varchar" {
                return None;
            }
            string_literal(t.arg.as_deref()?)
        }
        _ => None,
    }
}

/// The keys of `#>`: a text array literal (`'{a,b}'`, cast to `text[]` or not), or `ARRAY[…]` of
/// string literals.
fn text_array(node: &Node) -> Option<Vec<String>> {
    match node.node.as_ref() {
        Some(NodeEnum::AConst(c)) => match c.val.as_ref() {
            Some(protobuf::a_const::Val::Sval(s)) => array_literal(&s.sval),
            _ => None,
        },
        Some(NodeEnum::AArrayExpr(a)) => a.elements.iter().map(string_literal).collect(),
        Some(NodeEnum::TypeCast(t)) => {
            let target = t.type_name.as_ref().filter(|n| n.array_bounds.len() == 1).and_then(|n| builtin_name(&n.names))?;
            if target != "text" && target != "varchar" {
                return None;
            }
            text_array(t.arg.as_deref()?)
        }
        _ => None,
    }
}

/// The elements of a one-dimensional array literal, as Postgres reads its input: `{a, "b c"}`,
/// a quoted element taking a backslash's next character as it is and an unquoted one trimmed.
/// `None` for a NULL element, an empty unquoted one, nesting, explicit bounds or anything else
/// the reading would doubt.
fn array_literal(text: &str) -> Option<Vec<String>> {
    let inner = text.trim().strip_prefix('{')?.strip_suffix('}')?;
    let mut out = Vec::new();
    if inner.trim().is_empty() {
        return Some(out);
    }
    let mut chars = inner.chars().peekable();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let mut element = String::new();
        if chars.next_if_eq(&'"').is_some() {
            loop {
                match chars.next()? {
                    '\\' => element.push(chars.next()?),
                    '"' => break,
                    c => element.push(c),
                }
            }
            while chars.next_if(|c| c.is_whitespace()).is_some() {}
        } else {
            while let Some(c) = chars.next_if(|c| *c != ',') {
                match c {
                    '{' | '}' | '"' | '\\' => return None,
                    c => element.push(c),
                }
            }
            element.truncate(element.trim_end().len());
            if element.is_empty() || element.eq_ignore_ascii_case("null") {
                return None;
            }
        }
        out.push(element);
        match chars.next() {
            None => return Some(out),
            Some(',') => {}
            Some(_) => return None,
        }
    }
}

fn reads_leaf(expr: &Expr) -> bool {
    match expr {
        Expr::Leaf(_) | Expr::Row(..) => true,
        Expr::Nullable(inner) => reads_leaf(inner),
        Expr::Every(operands, _) | Expr::Any(operands, _) => operands.iter().any(reads_leaf),
        Expr::Guarded(inner, _) | Expr::NotJsonb(inner) | Expr::Narrowed(inner) | Expr::NotRow(inner, _) => reads_leaf(inner),
        Expr::JsonPath { base, .. } => reads_leaf(base),
        Expr::Array { items, .. } => items.iter().any(reads_leaf),
        Expr::NonNull(_) | Expr::Unknown | Expr::Unread => false,
        Expr::Call(call) => call.args.iter().any(reads_leaf),
        Expr::Trusted(t) => t.reads_leaf(),
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

    /// The trust check of a catalog with no object of a user's, whose row types have the attributes
    /// named here among the names the rules read.
    fn answered(attributes: &[&str]) -> Trust {
        use crate::resolution::{Overloads, Shared};
        use serde_json::{json, Value};
        let rows: Vec<Vec<Value>> = attributes.iter().map(|a| vec![json!("column"), json!(a), Value::Null, Value::Null, Value::Null]).collect();
        let mut trust = Trust::default();
        trust.overloads = Some(Shared(std::sync::Arc::new(Overloads::read(&rows, &mut trust.shadowed))));
        trust
    }

    #[test]
    fn a_statement_s_own_rows_have_the_columns_its_from_items_show() {
        let fields = |sql: &str| row_fields(&serde_json::to_value(&crate::stack::parse(sql).unwrap().protobuf).unwrap());
        let f = fields("WITH w (every) AS (SELECT 1) SELECT sum(x) AS total, max(y) FROM (SELECT a AS sum, b FROM t) s, generate_series(1, 2) g (bit_or), LATERAL (SELECT avg(c) FROM u) v, (SELECT n FROM t) q (bool_and), t AS x (bool_or), (t JOIN u USING (id)) AS j (min)");
        for name in ["every", "sum", "b", "bit_or", "generate_series", "g", "avg", "c", "bool_and", "bool_or", "min"] {
            assert!(f.contains(name), "{name}");
        }
        // The top statement's own targets name no row the statement reads.
        assert!(!f.contains("total") && !f.contains("max") && !f.contains("x"));
        assert!(fields("WITH d AS (DELETE FROM t RETURNING min(1) OVER () , id AS bool_and) SELECT 1").contains("bool_and"));
        assert!(fields("SELECT sum(n) FROM t").is_empty());
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
        // Quoted, the name can only be a user's function, which proves nothing.
        assert!(!first(&pg("SELECT \"greatest\"(NULL, 0)")).non_null);
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
    fn a_window_aggregate_over_the_current_row_follows_its_argument() {
        let one = |sql: &str, non_null: bool| resolve(&pg(sql).head[0].clone().expect("a claim"), Some(&[col(!non_null, oid::INT4)])).non_null;
        // No frame clause, with ORDER BY or without, and every frame from at or before the current
        // row to at or after it, in every mode.
        for over in [
            "()",
            "(ORDER BY a)",
            "(PARTITION BY a)",
            "(ORDER BY a ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING)",
            "(ORDER BY a ROWS UNBOUNDED PRECEDING)",
            "(ORDER BY a ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING)",
            "(ORDER BY a RANGE BETWEEN 5 PRECEDING AND 0 FOLLOWING)",
            "(ORDER BY a GROUPS BETWEEN 2 PRECEDING AND CURRENT ROW)",
            "(ORDER BY a RANGE BETWEEN CURRENT ROW AND CURRENT ROW EXCLUDE TIES)",
            "(ORDER BY a RANGE UNBOUNDED PRECEDING EXCLUDE NO OTHERS)",
        ] {
            for agg in ["sum", "max", "min", "avg", "bit_or", "array_agg"] {
                let sql = format!("SELECT {agg}(a) OVER {over} FROM t");
                assert!(one(&sql, true), "{sql}");
                // `array_agg` is an array of whatever its rows hold, NULLs included.
                assert_eq!(one(&sql, false), agg == "array_agg", "{sql}: a NULL argument");
            }
        }
        assert!(one("SELECT string_agg(a::text, ',') OVER () FROM t", true));
        assert!(one("SELECT jsonb_agg(a) OVER (ORDER BY a) FROM t", false), "a JSON array of NULLs is an array");
        // A frame that can leave the current row out, and a FILTER, can leave it no row.
        for over in [
            "(ORDER BY a ROWS BETWEEN 1 FOLLOWING AND 2 FOLLOWING)",
            "(ORDER BY a ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING)",
            "(ORDER BY a ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE CURRENT ROW)",
            "(ORDER BY a RANGE BETWEEN CURRENT ROW AND CURRENT ROW EXCLUDE GROUP)",
        ] {
            let sql = format!("SELECT sum(a) OVER {over} FROM t");
            assert!(!one(&sql, true), "{sql}");
        }
        assert!(!one("SELECT sum(a) FILTER (WHERE a > 1) OVER () FROM t", true));
        // A named window lends its frame to `OVER w`; `OVER (w …)` and `AS (w …)` use their own.
        assert!(one("SELECT sum(a) OVER w FROM t WINDOW w AS (ORDER BY a)", true));
        assert!(!one("SELECT sum(a) OVER w FROM t WINDOW w AS (ORDER BY a ROWS BETWEEN 1 FOLLOWING AND 2 FOLLOWING)", true));
        assert!(!one("SELECT sum(a) OVER w FROM t WINDOW v AS (ORDER BY a)", true), "no window w");
        assert!(one("SELECT sum(a) OVER (w ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING) FROM t WINDOW w AS (PARTITION BY a)", true));
        assert!(!one("SELECT sum(a) OVER (w ROWS BETWEEN 2 PRECEDING AND 1 PRECEDING) FROM t WINDOW w AS (PARTITION BY a)", true));
        assert!(one("SELECT sum(a) OVER v FROM t WINDOW w AS (PARTITION BY a), v AS (w ORDER BY a ROWS 1 PRECEDING)", true));
        // A plain aggregate with no GROUP BY is still NULL over no rows.
        assert!(!one("SELECT sum(a) FROM t", true));
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

    /// A trust that knows pgvector's `=`, `+` and `sum`, which no built-in type converts to, and
    /// PostGIS's `=`, which `text` converts to, beside the built-ins over `int4` and `text`.
    fn extended(param_types: Vec<u32>) -> Trust {
        use crate::resolution::{Overloads, Shared};
        use serde_json::{json, Value};
        let opaque = |kind: &str, name: &str, arity: u32| vec![json!(format!("opaque {kind}")), json!(name), Value::Null, json!(arity), json!(0)];
        let builtin = |kind: &str, name: &str, args: &[u32], result: u32| vec![json!(format!("builtin {kind}")), json!(name), json!(args), json!(result), Value::Null];
        let rows = vec![
            opaque("b", "=", 2),
            opaque("b", "+", 2),
            opaque("f", "sum", 1),
            vec![json!("user b"), json!("="), json!([GEOMETRY, GEOMETRY]), json!(0), json!(0)],
            vec![json!("reach"), Value::Null, json!([oid::TEXT, oid::BYTEA]), json!(GEOMETRY), Value::Null],
            builtin("b", "=", &[oid::INT4, oid::INT4], oid::BOOL),
            builtin("b", "=", &[oid::TEXT, oid::TEXT], oid::BOOL),
            builtin("b", "+", &[oid::INT4, oid::INT4], oid::INT4),
            builtin("f", "sum", &[oid::INT4], oid::INT8),
        ];
        let mut trust = Trust { param_types, non_null_params: true, ..Trust::default() };
        trust.overloads = Some(Shared(std::sync::Arc::new(Overloads::read(&rows, &mut trust.shadowed))));
        trust
    }

    const VECTOR: u32 = 20000;
    const GEOMETRY: u32 = 20200;

    #[test]
    fn a_shadowed_name_proves_what_the_built_in_does_where_the_operands_resolve_to_it() {
        let trust = extended(vec![oid::TEXT]);
        let facts = |sql: &str, leaves: &[DescribedColumn]| -> Vec<Fact> {
            plan(sql, Dialect::Postgres, &trust).head.iter().map(|e| resolve(e.as_ref().expect("a claim"), Some(leaves))).collect()
        };
        let int = facts("SELECT a + 1 AS x, a = 2 AS y, a = 'x' AS z, (a + 1) = 2 AS n FROM t", &vec![col(false, oid::INT4); 4]);
        assert!(int.iter().all(|f| f.non_null), "{int:?}");
        assert_eq!(int[0].type_oid, Some(oid::INT4));
        // A vector's own `=` and `+`, whatever they answer.
        let vector = facts("SELECT a + a AS x, a = a AS y FROM t", &vec![col(false, VECTOR); 4]);
        assert!(vector.iter().all(|f| !f.non_null), "{vector:?}");
        // `text` converts to `geometry`, but `text = text` is an exact match; `text = bytea` is not.
        assert!(facts("SELECT a = 'x' AS y FROM t", &[col(false, oid::TEXT)])[0].non_null);
        assert!(!facts("SELECT a = b AS y FROM t", &[col(false, oid::TEXT), col(false, oid::BYTEA)])[0].non_null);
        // Two untyped operands may resolve to any overload.
        assert!(!facts("SELECT 'a' = 'b' AS y", &[])[0].non_null);
        // A grouped `sum` of an `int4` is the built-in's, an `int8`.
        let grouped = facts("SELECT sum(a) AS s FROM t GROUP BY b", &[col(false, oid::INT4)]);
        assert_eq!(grouped[0], Fact { non_null: true, type_oid: Some(oid::INT8) });
        // A trust that says nothing of the overloads proves nothing, as before.
        let bare = Trust { shadowed: trust.shadowed.clone(), ..Trust::default() };
        assert_eq!(plan("SELECT a + 1 FROM t", Dialect::Postgres, &bare).head[0], Some(Expr::Unknown));
    }

    #[test]
    fn a_cast_to_a_type_a_user_casts_to_keeps_nullability_from_any_other_source() {
        use crate::resolution::{Overloads, Shared};
        use serde_json::{json, Value};
        // PostGIS's cast of a `geometry` to `text`, which takes `text` off the trusted targets.
        let rows = vec![vec![json!("cast from"), json!("text"), json!([]), json!(oid::TEXT), Value::Null]];
        let mut trust = Trust { cast_targets: Some(HashSet::new()), non_null_params: true, param_types: vec![oid::INT4], ..Trust::default() };
        trust.overloads = Some(Shared(std::sync::Arc::new(Overloads::read(&rows, &mut trust.shadowed))));
        let fact = |sql: &str, leaves: &[DescribedColumn]| resolve(plan(sql, Dialect::Postgres, &trust).head[0].as_ref().expect("a claim"), Some(leaves));
        assert_eq!(fact("SELECT a::text FROM t", &[col(false, oid::INT4)]), Fact { non_null: true, type_oid: Some(oid::TEXT) });
        assert!(fact("SELECT 'x'::text", &[]).non_null);
        assert!(fact("SELECT $1::text", &[]).non_null);
        assert!(!fact("SELECT a::text FROM t", &[col(false, GEOMETRY)]).non_null);
        // A target no row names stays distrusted.
        assert!(!fact("SELECT a::int8 FROM t", &[col(false, oid::INT4)]).non_null);
    }

    #[test]
    fn a_cast_resolves_by_the_cast_targets_not_by_the_shadowed_names() {
        use crate::resolution::{Overloads, Shared};
        use serde_json::{json, Value};
        // A user's cast function from `bool` to `int8`: `int8` is no trusted target, and no name
        // is shadowed.
        let rows = vec![vec![json!("cast from"), json!("int8"), json!([oid::BOOL]), json!(oid::INT8), Value::Null]];
        let mut trust = Trust { cast_targets: Some(["text".to_string()].into_iter().collect()), ..Trust::default() };
        trust.overloads = Some(Shared(std::sync::Arc::new(Overloads::read(&rows, &mut trust.shadowed))));
        assert!(trust.shadowed.is_empty());
        assert_eq!(trust.resolves(Kind::Cast, "int8", &[Operand::Is(oid::BOOL)]), None);
        assert_eq!(trust.resolves(Kind::Cast, "int8", &[Operand::Is(oid::INT4)]), Some(Some(oid::INT8)));
        assert_eq!(trust.resolves(Kind::Cast, "text", &[Operand::Is(oid::BOOL)]), Some(None));
    }

    #[test]
    fn is_distinct_from_rests_on_the_equality_it_runs() {
        let trust = extended(Vec::new());
        let fact = |sql: &str, leaves: &[DescribedColumn]| resolve(plan(sql, Dialect::Postgres, &trust).head[0].as_ref().expect("a claim"), Some(leaves));
        // `=` of two `int4`s is the built-in's; of two of a type a user defined, theirs, which may
        // answer NULL and make the test NULL.
        assert_eq!(fact("SELECT a IS DISTINCT FROM b FROM t", &[col(true, oid::INT4), col(true, oid::INT4)]), Fact { non_null: true, type_oid: Some(oid::BOOL) });
        assert!(!fact("SELECT a IS NOT DISTINCT FROM b FROM t", &[col(false, VECTOR), col(false, VECTOR)]).non_null);
        // Beside a NULL no `=` runs.
        assert!(fact("SELECT a IS DISTINCT FROM NULL FROM t", &[col(false, VECTOR)]).non_null);
        // With `=` the built-in's alone, nothing to check.
        assert_eq!(pg("SELECT a IS DISTINCT FROM b FROM t").head[0], Some(Expr::NonNull(Some(oid::BOOL))));
        // NULLIF claims nothing whatever its `=` is.
        assert_eq!(pg("SELECT nullif(1, 2)").head[0], Some(Expr::Unknown));
    }

    #[test]
    fn a_call_that_may_be_a_column_of_its_row_argument_proves_nothing() {
        const PAIR: u32 = 20400;
        const CITEXT: u32 = 20500;
        let fact = |sql: &str, leaves: &[DescribedColumn], trust: &Trust| resolve(plan(sql, Dialect::Postgres, trust).head[0].as_ref().expect("a claim"), Some(leaves)).non_null;
        let plain = Trust::default();
        // `length(t)` over a row no `length` takes is the row's column `length`, which may be
        // NULL: a table's or a composite type's row, `record`, a shared catalog's row.
        assert!(fact("SELECT length(name) FROM t", &[col(false, oid::TEXT)], &plain));
        for row in [PAIR, 2249, 1248, 12001] {
            assert!(!fact("SELECT length(p) FROM t", &[col(false, row)], &plain), "{row}");
            assert!(!fact("SELECT abs(p) FROM t", &[col(false, row)], &plain), "{row}");
        }
        assert!(!fact("SELECT md5(t) FROM t", &[col(false, PAIR)], &plain));
        // Qualified or decorated, it is the function.
        assert!(fact("SELECT pg_catalog.length(p) FROM t", &[col(false, PAIR)], &plain));
        // A call or a constant the rules read is no row whatever its type, and a call over it is the
        // function where it is non-null.
        assert!(fact("SELECT length(upper(name)) FROM t", &[col(false, oid::TEXT)], &plain));
        assert!(fact("SELECT length(name || 'x') FROM t", &[col(false, oid::TEXT)], &plain));
        assert!(fact("SELECT abs(length(name)) FROM t", &[col(false, oid::TEXT)], &plain));
        assert!(fact("SELECT md5('x')", &[], &plain));
        // `max` of a row may be a row, and a column of its own over a `record` before 16.
        assert!(!fact("SELECT length(max(p)) FROM t GROUP BY id", &[col(false, oid::TEXT)], &plain));
        // A function of no argument over one is that column alone; `concat_ws` of one too.
        assert_eq!(pg("SELECT now(t) FROM t").head[0], Some(Expr::Unknown));
        assert!(!fact("SELECT concat_ws(p) FROM t", &[col(false, PAIR)], &plain));
        // Under a GROUP BY an aggregate of a row may be its column too.
        assert!(!fact("SELECT sum(p) FROM t GROUP BY id", &[col(false, PAIR)], &plain));
        assert!(fact("SELECT sum(n) FROM t GROUP BY id", &[col(false, oid::INT4)], &plain));
        // A function that takes any row is one over it.
        assert!(fact("SELECT to_jsonb(p) FROM t", &[col(false, PAIR)], &plain));
        // A type of a user's is no row where the trust query lists it a scalar, and the built-in
        // rows are the ones it lists.
        use crate::resolution::{Overloads, Shared};
        use serde_json::{json, Value};
        let rows = vec![vec![json!("rows"), Value::Null, json!([71, 1248, 2249]), Value::Null, Value::Null], vec![json!("scalars"), Value::Null, json!([CITEXT]), Value::Null, Value::Null]];
        let mut listed = Trust::default();
        listed.overloads = Some(Shared(std::sync::Arc::new(Overloads::read(&rows, &mut listed.shadowed))));
        assert!(fact("SELECT length(h) FROM t", &[col(false, CITEXT)], &listed));
        assert!(!fact("SELECT length(h) FROM t", &[col(false, CITEXT)], &plain));
        assert!(!fact("SELECT length(p) FROM t", &[col(false, PAIR)], &listed));
        assert!(!fact("SELECT length(p) FROM t", &[col(false, 1248)], &listed));
        assert!(fact("SELECT length(p) FROM t", &[col(false, 12001)], &listed));
    }

    #[test]
    fn a_strict_where_on_a_shadowed_name_narrows_where_its_operands_resolve_to_the_built_in() {
        let trust = extended(vec![oid::TEXT]);
        let fact = |sql: &str, leaves: &[DescribedColumn], at: usize| resolve(plan(sql, Dialect::Postgres, &trust).head[at].as_ref().expect("a claim"), Some(leaves)).non_null;
        // The probe reads the WHERE's column: a `text` beside a parameter typed `text`.
        assert!(fact("SELECT name FROM t WHERE name = $1", &[col(true, oid::TEXT)], 0));
        assert!(!fact("SELECT name FROM t WHERE name = $1", &[col(true, VECTOR)], 0));
        // The expressions that read the column are narrowed too.
        let p = plan("SELECT name || 'x' AS shout FROM t WHERE name = $1", Dialect::Postgres, &trust);
        assert!(p.reads_leaves());
        let leaves = [col(true, oid::TEXT), col(true, oid::TEXT)];
        assert!(resolve(p.head[0].as_ref().unwrap(), Some(&leaves)).non_null);
        // No describe of the parameter: it may be of any type.
        let untyped = extended(Vec::new());
        assert!(!resolve(plan("SELECT name FROM t WHERE name = $1", Dialect::Postgres, &untyped).head[0].as_ref().unwrap(), Some(&[col(true, oid::TEXT)])).non_null);
        // A NULL a probe wrote for a parameter is of any type too.
        assert!(!fact("SELECT name FROM t WHERE name = NULL", &[col(true, oid::TEXT)], 0));
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
        let answered = answered(&[]);
        assert!(first(&pg("SELECT (SELECT count(*) FROM todos t WHERE t.owner_id = o.id) AS n FROM owners o")).non_null);
        assert!(first(&plan("SELECT (SELECT coalesce(max(t.n), 0) FROM todos t) AS n", Dialect::Postgres, &answered)).non_null);
        // `max(t.n)` may be a column of `t.n` where a row type has a column `max`, or a catalog
        // that did not answer may, and a row the statement builds may.
        assert!(!first(&pg("SELECT (SELECT coalesce(max(t.n), 0) FROM todos t) AS n")).non_null);
        assert!(!first(&plan("SELECT (SELECT coalesce(max(t.n), 0) FROM todos t) AS n", Dialect::Postgres, &self::answered(&["max"]))).non_null);
        assert!(!first(&plan("SELECT (SELECT coalesce(max(q), 0) FROM (SELECT n AS max FROM todos) q) AS n", Dialect::Postgres, &answered)).non_null);
        // An argument that is no row for certain makes it the aggregate, and so does a column of a
        // type that is none, which the catalog gives.
        assert!(first(&plan("SELECT (SELECT coalesce(max(t.n + 1), 0) FROM todos t) AS n", Dialect::Postgres, &self::answered(&["max"]))).non_null);
        let typed = Columns::from_rows(&[
            vec![serde_json::json!("todos"), serde_json::json!(["id", "owner_id", "title", "n"]), serde_json::json!([oid::INT4, oid::INT4, oid::TEXT, oid::INT4])],
            vec![serde_json::json!("pairs"), serde_json::json!(["id", "p"]), serde_json::json!([oid::INT4, 20400])],
        ]);
        let listed = self::answered(&["max"]);
        let typed_plan = |sql: &str| first(&super::plan(sql, Dialect::Postgres, &listed, &typed)).non_null;
        assert!(typed_plan("SELECT (SELECT coalesce(max(t.n), 0) FROM todos t) AS n"));
        assert!(typed_plan("SELECT (SELECT coalesce(max(n), 0) FROM todos) AS n"));
        assert!(!typed_plan("SELECT (SELECT coalesce(max(x.p), 0) FROM pairs x) AS n"));
        assert!(!typed_plan("SELECT (SELECT coalesce(max(t), 0) FROM todos t) AS n"));
        // A bare name two items show is no column of either for certain.
        assert!(!typed_plan("SELECT (SELECT coalesce(max(id), 0) FROM todos, pairs) AS n"));
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
            assert!(first(&plan(sql, Dialect::Postgres, &answered)).non_null, "{sql}");
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
        let p = pg("SELECT user_id, sum(views) AS total, max(views) AS top, array_agg(title) AS titles, sum(views) FILTER (WHERE views > 0) AS some, sum(views) OVER (ROWS BETWEEN 1 FOLLOWING AND 1 FOLLOWING) AS next FROM posts GROUP BY user_id");
        // One leaf per column reference read: five of `views` here.
        let views = [col(false, oid::INT4), col(false, oid::INT4), col(false, oid::INT4), col(false, oid::INT4), col(false, oid::INT4)];
        assert_eq!(p.head[0], None);
        assert!(resolve(p.head[1].as_ref().unwrap(), Some(&views)).non_null);
        assert!(resolve(p.head[2].as_ref().unwrap(), Some(&views)).non_null);
        assert!(resolve(p.head[3].as_ref().unwrap(), Some(&views)).non_null);
        // A FILTER can leave the group no rows, and a frame that leaves out the current row can be
        // empty.
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
        // Only the query that is read, with the queries before it in front of it: none here.
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].with.as_deref(), Some("pub"));
        assert_eq!(queries[0].sql, "SELECT id, at FROM posts WHERE at IS NOT NULL");
        // A query's body sees the queries before it, not the ones after it nor itself, whose
        // names are tables' there.
        let (_, queries) = item_reads("WITH a AS (SELECT 1 AS x), b AS (SELECT x FROM a), c AS (SELECT x FROM c) SELECT b.x, c.x FROM b, c").unwrap();
        assert_eq!(queries[0].sql, "WITH a AS (SELECT 1 AS x) SELECT x FROM a");
        assert_eq!(queries[1].sql, "WITH a AS (SELECT 1 AS x), b AS (SELECT x FROM a) SELECT x FROM c");
        // A subquery's own WITH would hide the statement's, so it is no statement of its own.
        assert_eq!(item_reads("WITH a AS (SELECT 1 AS x) SELECT s.x FROM (WITH b AS (SELECT 2 AS y) SELECT a.x FROM a, b) s"), None);
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
    fn a_json_read_at_literal_keys_is_typed_by_the_columns_schema() {
        let path = |sql: &str| match pg(sql).head.into_iter().next().flatten() {
            Some(Expr::JsonPath { path, text, indexes, .. }) => Some((path, text, indexes)),
            _ => None,
        };
        let keys = |list: &[&str]| list.iter().map(|k| k.to_string()).collect::<Vec<_>>();
        assert_eq!(path("SELECT doc -> 'a' FROM t"), Some((keys(&["a"]), false, false)));
        assert_eq!(path("SELECT doc -> 'a' ->> 'b' FROM t"), Some((keys(&["a", "b"]), true, false)));
        assert_eq!(path("SELECT doc -> 'a'::text FROM t"), Some((keys(&["a"]), false, false)));
        assert_eq!(path(r#"SELECT doc #> '{a, "b c", "d\"e"}' FROM t"#), Some((keys(&["a", "b c", "d\"e"]), false, true)));
        assert_eq!(path("SELECT doc #>> ARRAY['a', 'b'] FROM t"), Some((keys(&["a", "b"]), true, true)));
        assert_eq!(path("SELECT doc #> '{}'::text[] FROM t"), Some((vec![], false, true)));
        // A key that is no literal, an array index, a read of a read's text and of anything but a
        // column claim nothing.
        for sql in [
            "SELECT doc -> $1 FROM t",
            "SELECT doc -> 0 FROM t",
            "SELECT doc -> '0'::int FROM t",
            "SELECT doc #> '{a,NULL}' FROM t",
            "SELECT doc #> '{{a}}' FROM t",
            "SELECT (doc ->> 'a') -> 'b' FROM t",
            "SELECT (doc || '{}') -> 'a' FROM t",
        ] {
            assert_eq!(path(sql), None, "{sql}");
        }
        let shadowed = Trust { shadowed: ["->".to_string()].into_iter().collect(), ..Trust::default() };
        assert_eq!(plan("SELECT doc -> 'a' FROM t", Dialect::Postgres, &shadowed).head, vec![Some(Expr::Unknown)]);
        // The read resolves against the column's schema: there for certain under a required key of
        // a column that is not NULL, and a text read of a value that may be a JSON null is NULL.
        let schema = serde_json::json!({"type": "object", "properties": {"a": {"type": ["string", "null"]}}, "required": ["a"], "additionalProperties": true});
        let doc = |nullable: bool, type_oid: u32| DescribedColumn { document: Some(schema.clone()), ..col(nullable, type_oid) };
        let read = |sql: &str, leaf: DescribedColumn| resolve(pg(sql).head[0].as_ref().expect("a claim"), Some(&[leaf]));
        assert_eq!(read("SELECT doc -> 'a' FROM t", doc(false, oid::JSONB)), Fact { non_null: true, type_oid: Some(oid::JSONB) });
        assert_eq!(read("SELECT doc -> 'a' FROM t", doc(false, oid::JSON)), Fact { non_null: true, type_oid: Some(oid::JSON) });
        assert_eq!(read("SELECT doc ->> 'a' FROM t", doc(false, oid::JSONB)), Fact { non_null: false, type_oid: Some(oid::TEXT) });
        assert_eq!(read("SELECT doc -> 'a' FROM t", doc(true, oid::JSONB)), Fact { non_null: false, type_oid: Some(oid::JSONB) });
        assert_eq!(read("SELECT doc -> 'b' FROM t", doc(false, oid::JSONB)), Fact { non_null: false, type_oid: Some(oid::JSONB) });
        assert_eq!(read("SELECT doc -> 'a' FROM t", col(false, oid::JSONB)).non_null, false);
        assert_eq!(read("SELECT doc -> 'a' FROM t", doc(false, oid::TEXT)).non_null, false);
    }

    #[test]
    fn what_does_not_parse_claims_nothing() {
        assert_eq!(pg("SELEKT 1"), Plan::default());
        assert_eq!(lite("SELECT json_extract('not json', '$') AS boom").head, vec![Some(Expr::Unknown)]);
    }
}
