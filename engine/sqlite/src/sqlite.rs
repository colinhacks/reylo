//! The SQLite analysis, over SQLite's own resolved tree ([`crate::sqlite_tree`], produced by
//! `csrc/reylo_sqlite.c` from a patched amalgamation). SQLite resolves the names, the join sides,
//! every parameter's context and the DML target columns itself, and reports each expression's
//! affinity. What SQLite has no facts for stays here, as explicit tables: the result type and
//! nullability of the built-in functions, the type a function gives its arguments, the arithmetic
//! rule, and the row-count proof from the unique keys. The host runs two queries, one for the
//! schema DDL and one for the functions its SQLite has; everything else happens inside the engine.
//!
//! Two entries share the code: [`analyze`], replayed through the [`Host`] like every dialect, and
//! [`analyze_ddl`], for a host that holds a live connection and runs [`DDL_QUERY`] and
//! [`FUNCTIONS_QUERY`] on it itself, with [`Extras`] naming the scalar functions it registers there.

use crate::analysis::{bare, constant_key, diag, KeyClause};
use crate::api::{Analysis, Column, Param, Scope};
use crate::dialect::Dialect;
use crate::fresh::Tables;
use crate::host::{as_str, Host, Lazy};
use crate::lex::{self, Lexed, Target};
use crate::sqlite_tree::{self as tree, From, Node, Sel, Tree};
use crate::types::{Field, Type};
use std::collections::{HashMap, HashSet};

/// The schema, as SQLite stores it: its `sql` column is the DDL the engine replays into its
/// in-memory copy. The replayed entry asks the host for it; a host with a connection of its own
/// runs it there and hands the rows to [`analyze_ddl`].
pub const DDL_QUERY: &str = "select sql from sqlite_schema where sql is not null and type in ('table', 'view', 'index', 'trigger') order by rowid";

/// The functions the host's SQLite has, which decide what a statement may call: SQLite builds
/// differ in them (the math functions, `soundex`, a newer release's additions), and the embedded
/// one has every optional family, so it alone would accept a call the host then refuses to run.
/// The replayed entry asks the host for it; a host with a connection of its own runs it there and
/// hands the rows to [`Extras::host`] through [`host_functions`]. A SQLite that cannot answer (an
/// old one whose pragma lacks these columns, or a service that refuses the pragma) is taken to
/// have what the embedded one has.
pub const FUNCTIONS_QUERY: &str = "select name, narg, type from pragma_function_list";

/// One row of [`FUNCTIONS_QUERY`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFunction {
    pub name: String,
    /// The argument count as SQLite keeps it: -1 for any number.
    pub arity: i32,
    /// `s` for a scalar function, `a` for an aggregate, `w` for a window function.
    pub kind: char,
}

/// The rows of [`FUNCTIONS_QUERY`] as the host's function list; `None` when there is none to
/// read, since every SQLite has functions and an empty list would refuse every call.
pub fn host_functions(rows: &[Vec<serde_json::Value>]) -> Option<Vec<HostFunction>> {
    let listed: Vec<HostFunction> = rows
        .iter()
        .filter_map(|row| {
            let name = row.first().and_then(as_str)?.to_string();
            let arity = row.get(1).and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok())).unwrap_or(-1) as i32;
            let kind = row.get(2).and_then(as_str).and_then(|k| k.chars().next()).unwrap_or('s');
            Some(HostFunction { name, arity, kind })
        })
        .collect();
    (!listed.is_empty()).then_some(listed)
}

/// A scalar function the host registers on its own connection, which the engine's replay cannot
/// see: the replay registers a stub under the same name and argument count, so a statement calling
/// it prepares, and the function table types a call as `returns`, NULL only when an argument is.
/// A function the host vouches for as total, Postgres's `Trust::total` in SQLite's terms: NULL only
/// when an argument is, with the argument count SQLite resolves a call by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotalFunction {
    pub name: String,
    /// How many arguments it takes; -1 for any number.
    pub arity: i32,
    /// The storage class its value lands in.
    pub returns: Affinity,
}

/// A storage class a value lands in, as SQLite names a column's affinity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affinity {
    Integer,
    Real,
    Text,
    Blob,
    Numeric,
    /// Any storage class: SQLite's "none".
    Any,
}

impl Affinity {
    /// The affinity's name, as the tree and the type model spell it.
    pub fn name(self) -> &'static str {
        match self {
            Affinity::Integer => "integer",
            Affinity::Real => "real",
            Affinity::Text => "text",
            Affinity::Blob => "blob",
            Affinity::Numeric => "numeric",
            Affinity::Any => "none",
        }
    }
}

/// What a host says of the SQLite on its own connection: the functions it registers there, and
/// the functions that SQLite has ([`FUNCTIONS_QUERY`]; `None` when it cannot list them).
#[derive(Debug, Clone, Default)]
pub struct Extras {
    pub total: Vec<TotalFunction>,
    pub host: Option<Vec<HostFunction>>,
}

impl Extras {
    /// The registration a call resolves to: SQLite prefers a host function over a built-in of the
    /// same name and argument count.
    fn function(&self, name: &str, args: usize) -> Option<&TotalFunction> {
        self.total.iter().find(|f| f.name.eq_ignore_ascii_case(name) && (f.arity < 0 || f.arity as usize == args))
    }
}

/// [`analyze_ddl`]'s answer: the analysis, and what the tree adds for a host that types a result
/// column by the spelling its table declares.
#[derive(Debug, Clone)]
pub struct SqliteAnalysis {
    pub analysis: Analysis,
    /// Per result column, the declared type of the table column it reads
    /// (`sqlite3_column_decltype`, through subqueries and views); `None` for an expression. Empty
    /// when the statement did not prepare.
    pub declared: Vec<Option<String>>,
    /// `sqlite3_stmt_readonly`; false when the statement did not prepare.
    pub readonly: bool,
    /// The unique keys of every table the statement reads or writes (through views and
    /// subqueries), by the table's name as its DDL spells it: every unique, non-partial index over
    /// plain columns, the `INTEGER PRIMARY KEY`, and `rowid` for a rowid table without one. What
    /// the row-count proof reads, for a host that runs that proof over text of its own. Empty when
    /// the statement did not prepare.
    pub unique_keys: HashMap<String, Vec<Vec<String>>>,
    /// Per result column, whether it is a truth value: a comparison, `IN`, `BETWEEN`, `EXISTS`,
    /// `IS`, `IS NULL`, `NOT`, `AND`, `OR`, `TRUE` or `FALSE`, a `like`, `glob`, `regexp` or `match`
    /// call, or a `CASE`, `coalesce`, `ifnull`, `iif`, scalar subquery or subquery column whose
    /// every value is one. SQLite stores one as the integer 0 or 1, and its type stays `integer`;
    /// a host that types by meaning, as the framework does, reads it as a boolean. Empty when the
    /// statement did not prepare.
    pub booleans: Vec<bool>,
}

/// The replayed entry: the host answers [`DDL_QUERY`] and [`FUNCTIONS_QUERY`] (session-scoped,
/// asked in one round), the rest is [`run`].
pub fn analyze(sql: &str, lexed: &Lexed, host: &Host) -> Lazy<Analysis> {
    let schema = host.query(DDL_QUERY, Vec::new(), Scope::Session);
    let functions = host.query(FUNCTIONS_QUERY, Vec::new(), Scope::Session);
    let (rows, functions) = (schema?, functions?);
    let ddl: Vec<String> = match rows {
        Ok(rows) => rows.iter().filter_map(|r| r.first().and_then(as_str)).map(String::from).collect(),
        Err(e) => return Some(bare(sql, lexed, Dialect::Sqlite, vec![diag(format!("Cannot read the schema: {}", e.message), 0, 1, 5)])),
    };
    // A host that cannot list its functions keeps the embedded SQLite's.
    let extras = Extras { total: Vec::new(), host: functions.ok().and_then(|rows| host_functions(&rows)) };
    Some(run(sql, lexed, &ddl, &extras).analysis)
}

/// The direct entry, for a host with a connection of its own: `ddl` is the `sql` column of
/// [`DDL_QUERY`] run there, `extras` the functions it registers there. No round trip; a lex
/// error, a prepare error (SQLite's own message, at its offset) and a replay failure come back as
/// diagnostics, as on the replayed path.
pub fn analyze_ddl(sql: &str, ddl: &[String], extras: &Extras) -> SqliteAnalysis {
    crate::stack::run(|| analyze_ddl_here(sql, ddl, extras))
}

fn analyze_ddl_here(sql: &str, ddl: &[String], extras: &Extras) -> SqliteAnalysis {
    match crate::analysis::lexed(sql, Dialect::Sqlite) {
        Ok(lexed) => run(sql, &lexed, ddl, extras),
        Err(analysis) => SqliteAnalysis { analysis, declared: Vec::new(), readonly: false, unique_keys: HashMap::new(), booleans: Vec::new() },
    }
}

/// The statements the SQLite analysis reads; any other is a command ([`lex::Statement`]).
const KINDS: &[&str] = &["select", "with", "values", "insert", "replace", "update", "delete"];

fn run(sql: &str, lexed: &Lexed, ddl: &[String], extras: &Extras) -> SqliteAnalysis {
    let failed = |analysis: Analysis| SqliteAnalysis { analysis, declared: Vec::new(), readonly: false, unique_keys: HashMap::new(), booleans: Vec::new() };
    let statement = match lex::one_statement(sql, lexed, KINDS) {
        Ok(statement) => statement,
        Err(message) => return failed(bare(sql, lexed, Dialect::Sqlite, vec![diag(message, 0, sql.trim_end().len(), 10)])),
    };
    // SQLite prepares the `?n` rewrite, not the author's text: its own `:name` ends at the first
    // dot, so `:auth.user_id` survives only as a number. `?n` is the lexer's n-th parameter, which
    // is how `params` matches them back, and an error offset maps back through the rewrite.
    let numbered = lex::rewrite(sql, lexed, Target::Numbered);
    let functions: Vec<(&str, i32)> = extras.total.iter().map(|f| (f.name.as_str(), f.arity)).collect();
    // Triggers are not replayed (a body may call a function only the application registers);
    // their headers say which statements a trigger can skip a row of.
    let (triggers, replay): (Vec<String>, Vec<String>) = ddl.iter().cloned().partition(|s| trigger_header(s).is_some());
    let skipping = skipping_triggers(&triggers);
    let listed: Option<Vec<tree::Listed>> = extras.host.as_ref().map(|host| host.iter().map(|f| (f.name.as_str(), f.arity, f.kind)).collect());
    let t = match tree::analyze(&replay, &numbered.sql, &functions, listed.as_deref()) {
        Ok(t) => t,
        Err(message) => return failed(bare(sql, lexed, Dialect::Sqlite, vec![diag(message, 0, sql.trim_end().len(), 6)])),
    };
    let mut diagnostics = Vec::new();
    if let Some(err) = &t.error {
        let start = usize::try_from(err.offset).map_or(0, |o| numbered.to_original(o)).min(sql.len());
        let length = match err.length {
            Some(length) if err.offset >= 0 => length,
            _ if err.offset >= 0 => token_len(&sql[start..]),
            _ => sql.trim_end().len(),
        };
        let message = match (&err.missing_function, &err.missing_schema) {
            // The embedded SQLite has the function; the SQLite the statement will run on does not.
            (Some(name), _) => match &err.view {
                Some(view) => format!("no such function: {name} on this database's SQLite (the view {view} calls it)"),
                None => format!("no such function: {name} on this database's SQLite"),
            },
            // The replay holds the database file's own schema. An ATTACH is an act of the
            // application's connection at run time, and nothing in the file names it.
            (None, Some(schema)) => format!("{}: the database has no schema `{schema}`. A database the application attaches at run time (ATTACH ... AS {schema}) is not in the file the type check reads, so a table in it cannot be typed", err.message),
            (None, None) => err.message.clone(),
        };
        let message = if matches!(statement, lex::Statement::Command(_)) { lex::refused_command(&message) } else { message };
        diagnostics.push(diag(message, start, length, 6));
        // A statement that fails against the replayed schema often fails because a replay step did.
        for e in &t.schema_errors {
            let message = match &e.function {
                Some(name) => format!("The schema replay could not register the host function {name}: {}", e.message),
                None => format!("The schema replay skipped statement {}: {}", e.index + 1, e.message),
            };
            diagnostics.push(diag(message, 0, 1, 8));
        }
        return failed(bare(sql, lexed, Dialect::Sqlite, diagnostics));
    }
    // A JSON object of an odd count of arguments, which SQLite prepares and refuses when it runs;
    // only the built-in constructors, which a function the host registers under the name overrides
    // (`Extras::function`). The host's list names the built-ins with any argument count (-1).
    let registered = |name: &str| extras.total.iter().any(|f| f.name.eq_ignore_ascii_case(name)) || extras.host.iter().flatten().any(|f| f.name.eq_ignore_ascii_case(name) && f.arity >= 0);
    let constructors: Vec<&str> = ["json_object", "jsonb_object"].into_iter().filter(|n| !registered(n)).collect();
    diagnostics.extend(crate::analysis::odd_pairs(sql, lexed, &constructors));
    // A parameter as a whole sort or group key is an error; the rest is typed all the same.
    diagnostics.extend(constant_keys(sql, lexed, &t));
    let a = Analyzer { tree: &t, extras, skipping: &skipping, nullable_params: coalesced_params(&t, extras), level: std::cell::RefCell::new(Level::default()), shapes: std::cell::RefCell::new(HashMap::new()), recursion: std::cell::RefCell::new(Vec::new()), settled: std::cell::RefCell::new(HashMap::new()) };
    // A command (DDL, a PRAGMA, a transaction's control) has no tree to read: SQLite's prepare
    // says what it takes and returns, and nothing says where a column comes from, so each is of
    // no known type and may be NULL. One that returns no rows runs as a write does.
    if let lex::Statement::Command(kind) = statement {
        let columns: Vec<Column> = t.columns.iter().map(|c| Column { name: c.name.clone(), ty: Type::Unknown, nullable: true, type_name: c.declared.clone().unwrap_or_default(), comment: None }).collect();
        let cardinality = if columns.is_empty() { "none" } else { "many" };
        let analysis = Analysis {
            kind,
            params: a.params(lexed),
            columns,
            cardinality: cardinality.into(),
            cardinality_reason: None,
            rewritten: lex::rewrite(sql, lexed, Target::Question),
            style: lexed.style,
            diagnostics,
            unresolved: Vec::new(),
            structured: Vec::new(),
            fresh: None,
            tables: None,
        };
        return SqliteAnalysis { analysis, declared: t.columns.iter().map(|c| c.declared.clone()).collect(), readonly: t.readonly, unique_keys: HashMap::new(), booleans: vec![false; t.columns.len()] };
    }
    let (mut columns, kind) = a.columns();
    let booleans = a.booleans();
    // A truth value, and a column declared BOOLEAN, are booleans to the loader's run time, which
    // reads 0 and 1 and refuses any other value (src/values.ts). A truth value SQLite computes is
    // named `bool`, which the oracle holds to 0 and 1; a column keeps its affinity's name, since it
    // holds whatever was stored in it.
    for ((column, flag), computed) in columns.iter_mut().zip(a.flags()).zip(&booleans) {
        if flag {
            column.ty = Type::Scalar { name: "bool".into() };
            if *computed {
                column.type_name = "bool".into();
            }
        }
    }
    let params = a.params(lexed);
    let (cardinality, reason) = a.cardinality();
    let mut analysis = Analysis {
        kind,
        params,
        columns,
        cardinality,
        cardinality_reason: reason,
        rewritten: lex::rewrite(sql, lexed, Target::Question),
        style: lexed.style,
        diagnostics,
        unresolved: Vec::new(),
        structured: Vec::new(),
        fresh: None,
        tables: Some(tables(&t)),
    };
    crate::analysis::clashes(sql, &mut analysis, &|_| None);
    let unique_keys = t.tables.iter().map(|(name, info)| (name.clone(), info.unique_keys.clone())).collect();
    SqliteAnalysis { analysis, declared: t.columns.iter().map(|c| c.declared.clone()).collect(), readonly: t.readonly, unique_keys, booleans }
}

/// Whether a WITH query reads itself: an arm of its statement has a FROM item SQLite marks as the
/// query's recursive read of itself.
fn reads_itself(sel: &Sel) -> bool {
    let mut arm = Some(sel);
    while let Some(a) = arm {
        if a.from.iter().any(|f| f.recursive) {
            return true;
        }
        arm = a.prior.as_deref();
    }
    false
}

/// The tables the statement names, as SQLite resolved them, for the freshness facts: each table
/// and view a FROM clause reads, in subqueries, WITH queries and expressions too, and the table it
/// writes. A view is named, never what it reads; a WITH query, a subquery and a table-valued
/// function are no table. SQLite expands a view into a subquery as it does a derived table, and
/// marks both a view: a view keeps the name the statement writes, a derived table has none. A
/// recursive WITH query's reference to itself is marked as nothing at all, so a name the statement
/// gives a WITH query is no table anywhere in it.
fn tables(t: &Tree) -> Tables {
    #[derive(Default)]
    struct Walk {
        read: Vec<String>,
        ctes: Vec<String>,
    }
    impl Walk {
        fn node(&mut self, n: &Node) {
            if let Some(q) = &n.select {
                self.sel(q);
            }
            for child in n.left.iter().chain(&n.right) {
                self.node(child);
            }
            for child in n.list.iter().chain(&n.args) {
                self.node(child);
            }
        }
        fn sel(&mut self, s: &Sel) {
            for c in &s.columns {
                self.node(&c.expr);
            }
            for f in &s.from {
                if f.cte {
                    self.ctes.extend(f.table.clone());
                }
                if f.cte || f.subquery.is_some() && f.table.is_none() {
                    if let Some(q) = &f.subquery {
                        self.sel(q);
                    }
                } else if !f.tab_func {
                    self.read.extend(f.table.clone());
                }
                for n in f.args.iter().chain(&f.on) {
                    self.node(n);
                }
            }
            for n in s.rows.iter().flatten().chain(&s.where_).chain(&s.group_by).chain(&s.having).chain(&s.order_by).chain(&s.limit).chain(&s.offset) {
                self.node(n);
            }
            if let Some(prior) = &s.prior {
                self.sel(prior);
            }
        }
    }
    let mut walk = Walk::default();
    if let Some(s) = &t.statement {
        walk.sel(s);
    }
    for c in t.returning.iter().flatten() {
        walk.node(&c.expr);
    }
    let mut write = Vec::new();
    if let Some(dml) = &t.dml {
        write.push(dml.table.clone());
        let upsert = dml.upsert.iter().flat_map(|u| u.set.iter().map(|s| &s.expr).chain(&u.where_));
        for n in dml.values.iter().flatten().chain(dml.set.iter().map(|s| &s.expr)).chain(&dml.where_).chain(upsert) {
            walk.node(n);
        }
    }
    let Walk { mut read, ctes } = walk;
    read.retain(|table| !ctes.iter().any(|cte| cte.eq_ignore_ascii_case(table)));
    Tables { read, write }
}

/// The error of each parameter that is, bare or under casts and collations, a whole key of an
/// ORDER BY (a statement's, a window's or an aggregate's), a GROUP BY or a window's PARTITION BY,
/// anywhere in the statement, at the placeholder as the author wrote it. A key that only holds a
/// parameter (`case :sort when 'name' then name end`) is none.
fn constant_keys(sql: &str, lexed: &Lexed, t: &Tree) -> Vec<crate::api::Diagnostic> {
    fn param(n: &Node) -> Option<i64> {
        match n.op.as_str() {
            "variable" => Some(n.index),
            "cast" | "collate" => n.left.as_deref().and_then(param),
            _ => None,
        }
    }
    /// Every parameter an expression reads, at any depth.
    fn reads(n: &Node, out: &mut HashSet<i64>) {
        if n.op == "variable" {
            out.insert(n.index);
        }
        for child in n.left.iter().chain(&n.right).map(|b| b.as_ref()).chain(&n.list).chain(&n.args).chain(&n.order_by).chain(&n.partition_by) {
            reads(child, out);
        }
    }
    #[derive(Default)]
    struct Walk {
        found: Vec<(i64, KeyClause, bool)>,
    }
    impl Walk {
        fn keys(&mut self, list: &[Node], clause: KeyClause) {
            self.found.extend(list.iter().filter_map(param).map(|index| (index, clause, list.len() == 1)));
        }
        /// A select's own ORDER BY and GROUP BY. SQLite resolves a key that is a position or a
        /// result column's alias (`order by 2`, `group by tag`) to a copy of that column's
        /// expression, so a key that is a parameter the select list reads is such a copy: the
        /// author writes each parameter once.
        fn select_keys(&mut self, s: &Sel) {
            let mut listed = HashSet::new();
            s.columns.iter().for_each(|c| reads(&c.expr, &mut listed));
            for (list, clause) in [(&s.group_by, KeyClause::GroupBy), (&s.order_by, KeyClause::OrderBy)] {
                let alone = list.len() == 1;
                self.found.extend(list.iter().filter_map(param).filter(|index| !listed.contains(index)).map(|index| (index, clause, alone)));
            }
        }
        fn node(&mut self, n: &Node) {
            self.keys(&n.order_by, KeyClause::OrderBy);
            self.keys(&n.partition_by, KeyClause::PartitionBy);
            if let Some(q) = &n.select {
                self.sel(q);
            }
            for child in n.left.iter().chain(&n.right) {
                self.node(child);
            }
            for child in n.list.iter().chain(&n.args).chain(&n.order_by).chain(&n.partition_by) {
                self.node(child);
            }
        }
        fn sel(&mut self, s: &Sel) {
            self.select_keys(s);
            for c in &s.columns {
                self.node(&c.expr);
            }
            for f in &s.from {
                if let Some(q) = &f.subquery {
                    self.sel(q);
                }
                for n in f.args.iter().chain(&f.on) {
                    self.node(n);
                }
            }
            for n in s.rows.iter().flatten().chain(&s.where_).chain(&s.group_by).chain(&s.having).chain(&s.order_by).chain(&s.limit).chain(&s.offset) {
                self.node(n);
            }
            if let Some(prior) = &s.prior {
                self.sel(prior);
            }
        }
    }
    let mut walk = Walk::default();
    if let Some(s) = &t.statement {
        walk.sel(s);
    }
    for c in t.returning.iter().flatten() {
        walk.node(&c.expr);
    }
    if let Some(dml) = &t.dml {
        let upsert = dml.upsert.iter().flat_map(|u| u.set.iter().map(|s| &s.expr).chain(&u.where_));
        for n in dml.values.iter().flatten().chain(dml.set.iter().map(|s| &s.expr)).chain(&dml.where_).chain(upsert) {
            walk.node(n);
        }
    }
    // SQLite prepared the `?n` rewrite, whose n is the lexer's n-th parameter, which appears once.
    let mut out: Vec<_> = walk
        .found
        .into_iter()
        .filter_map(|(index, clause, alone)| {
            let name = lexed.params.get(usize::try_from(index).ok()?.checked_sub(1)?)?;
            let p = lexed.placeholders.iter().find(|p| &p.name == name)?;
            Some(constant_key(sql, p.start, p.end, clause, alone))
        })
        .collect();
    out.sort_by_key(|d| d.start);
    out.dedup_by_key(|d| d.start);
    out
}

fn token_len(rest: &str) -> usize {
    rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':' || *c == '$' || *c == '@' || *c == '?').count().max(1)
}

/// What an expression is: its affinity as the type model, and whether it can be NULL.
#[derive(Debug, Clone)]
struct Fact {
    ty: Type,
    non_null: bool,
}

impl Fact {
    fn new(aff: &str, non_null: bool) -> Fact {
        Fact { ty: affinity_type(aff), non_null }
    }
    fn unknown() -> Fact {
        Fact { ty: Type::Unknown, non_null: false }
    }
}

/// The type of a table column's values: its affinity, except that a column declared with no type,
/// or `ANY` in a STRICT table, holds a value of any storage class, and that a column declared with
/// a 64-bit integer's name ([`is_int8_declared`]) is `int8`, the type Postgres's `int8` and MySQL's
/// `BIGINT` are: the declared width decides, on every database (wiki/driver-decoding.md). (`ANY` in
/// a table that is not STRICT is a declared type like any other, with NUMERIC affinity.)
fn column_type(declared: &str, aff: &str, strict: bool) -> Type {
    let declared = declared.trim();
    if declared.is_empty() || (strict && declared.eq_ignore_ascii_case("any")) {
        Type::Unknown
    } else if aff == "integer" && is_int8_declared(declared) {
        Type::Scalar { name: "int8".into() }
    } else {
        affinity_type(aff)
    }
}

/// Whether a declared type names a 64-bit integer: `BIGINT`, `INT8` or `UNSIGNED BIG INT`, the
/// names SQLite's own documentation lists for one, in any case, with any spacing between words and
/// a size after it ignored (`bigint(20)`, as a MySQL dump writes it). `INTEGER`, `INT` and the
/// other integer names are SQLite's INTEGER, a `number`; so is the rowid, whatever its alias is
/// declared, since only `INTEGER PRIMARY KEY` is one.
fn is_int8_declared(declared: &str) -> bool {
    let base = declared.split('(').next().unwrap_or("").split_whitespace().map(str::to_ascii_lowercase).collect::<Vec<_>>().join(" ");
    matches!(base.as_str(), "bigint" | "int8" | "unsigned big int")
}

/// An affinity name as the type model; `none` is unknown.
fn affinity_type(aff: &str) -> Type {
    match aff {
        "integer" | "real" | "text" | "blob" | "numeric" => Type::Scalar { name: aff.to_string() },
        _ => Type::Unknown,
    }
}

fn scalar_name(t: &Type) -> Option<&str> {
    match t {
        Type::Scalar { name } => Some(name.as_str()),
        _ => None,
    }
}

fn is_numeric(t: &Type) -> bool {
    matches!(scalar_name(t), Some("integer" | "int8" | "real" | "numeric"))
}

/// Whether a type is an integer's: SQLite's INTEGER, or a column declared 64 bits wide.
fn is_integral(t: &Type) -> bool {
    matches!(scalar_name(t), Some("integer" | "int8"))
}

/// The type of a value computed from one of type `t`: a 64-bit column's value passes through
/// unchanged only as itself (a column, a subquery's or view's column, `min`, `max`, `coalesce`, a
/// `CASE` arm, a window's `lag`), and what SQLite computes from it (`-id`, `abs(id)`, `sum(id)`,
/// `id + 1`) is SQLite's INTEGER, as `count(*)` is.
fn computed(t: Type) -> Type {
    if scalar_name(&t) == Some("int8") {
        Type::Scalar { name: "integer".into() }
    } else {
        t
    }
}

/// The type several arms share: one type when they agree, `int8` for a mix of integers of which
/// one is a 64-bit column's (`coalesce(user_id, 0)`: any integer is exact as a BigInt), `real` for
/// another mix of numbers, unknown otherwise.
fn shared(facts: &[&Fact]) -> Type {
    let known: Vec<&Type> = facts.iter().map(|f| &f.ty).filter(|t| !matches!(t, Type::Unknown)).collect();
    let Some(first) = known.first() else { return Type::Unknown };
    // Built JSON is text to SQLite: beside a value of no known type (a parameter:
    // `coalesce(:doc, json_array())`), or another type, it is the text it is.
    if known.len() < facts.len() && known.iter().any(|t| is_built(t)) {
        return Type::Scalar { name: "text".into() };
    }
    if known.iter().all(|t| t == first) {
        return (*first).clone();
    }
    if let Some(ty) = same_json(&known) {
        return ty;
    }
    let known: Vec<Type> = known.iter().map(|t| textual(t)).collect();
    let known: Vec<&Type> = known.iter().collect();
    let first = known[0];
    if known.iter().all(|t| *t == first) {
        return first.clone();
    }
    if known.iter().all(|t| is_integral(t)) {
        return Type::Scalar { name: "int8".into() };
    }
    if known.iter().all(|t| is_numeric(t)) {
        return Type::Scalar { name: "real".into() };
    }
    Type::Unknown
}

/// The arguments of `coalesce` or `ifnull` it can hand back: each up to the first that is never
/// NULL, after which none is reached.
fn reached(args: &[Fact]) -> &[Fact] {
    let count = args.iter().position(|f| f.non_null).map_or(args.len(), |i| i + 1);
    &args[..count]
}

/// The type of a value SQLite hands back from one of `values` (a compound's arms, `CASE`'s results,
/// `coalesce`'s arguments): the type [`shared`] finds over them. A value of no known type that is no
/// parameter, such as a column declared without a type or `json_extract`, may be of any storage
/// class, so the result is unknown too; a parameter takes the others' type, and a NULL literal says
/// nothing of the type.
fn values_type(values: &[(&Node, &Fact)]) -> Type {
    let valued: Vec<&Fact> = values.iter().filter(|(e, _)| e.op != "null").map(|(_, f)| *f).collect();
    if values.iter().any(|(e, f)| matches!(f.ty, Type::Unknown) && e.op != "null" && e.op != "variable") {
        return Type::Unknown;
    }
    shared(&valued)
}

/// Whether a type is JSON a constructor built, which SQLite returns as text.
fn is_built(t: &Type) -> bool {
    matches!(t, Type::Object { .. } | Type::List { .. } | Type::Json { .. })
}

/// A type as SQLite's storage class: built JSON is text.
fn textual(t: &Type) -> Type {
    if is_built(t) { Type::Scalar { name: "text".into() } } else { t.clone() }
}

/// The one JSON shape several types are, where they differ only in that one is an empty array
/// (`json_array()`) and the others one list: the list.
fn same_json(types: &[&Type]) -> Option<Type> {
    let empty = |t: &Type| matches!(t, Type::Json { schema: Some(s) } if s.get("prefixItems").is_none() && s["type"] == "array" && s["items"] == false);
    let lists: Vec<&Type> = types.iter().copied().filter(|t| !empty(t)).collect();
    let first = lists.first()?;
    (matches!(first, Type::List { .. }) && lists.iter().all(|t| t == first)).then(|| (*first).clone())
}

/// Whether a value carries SQLite's JSON subtype, which makes a JSON function that reads it nest
/// it as JSON, where text without it is a string. Every JSON function sets it on what it returns,
/// and a `jsonb_*` blob is read as JSON alike. On SQLite 3.53.4 it passed through `coalesce`,
/// `ifnull`, `nullif`, `iif`, `CASE`, `likely`, `COLLATE`, unary plus, a cast to text, `min` and
/// `max` (the aggregates and the scalars), an aggregate with a FILTER and a scalar subquery, and
/// was dropped by a subquery's or view's column, a WITH query's, `lag`, a scalar subquery that
/// sorts its rows and a window call in a statement with a second window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Carry {
    /// Never: the value is read as the type it has.
    No,
    /// Always, where it is not NULL.
    Yes,
    /// On some SQLite builds or for some values: the engine says nothing of how it reads.
    Maybe,
}

/// Several values', one of which is the value: no subtype only where none has it.
fn combine(carries: &[Carry]) -> Carry {
    if carries.iter().all(|c| *c == Carry::No) {
        Carry::No
    } else if carries.iter().all(|c| *c == Carry::Yes) {
        Carry::Yes
    } else {
        Carry::Maybe
    }
}

/// A value as a JSON function reads it ([`Analyzer::json_fact`]).
struct JsonFact {
    ty: Type,
    non_null: bool,
    carry: Carry,
}

/// Whether a built-in returns JSON: every JSON function that returns a document or a value of
/// one (`json_extract`, `->`, `json_quote`), and every `jsonb_*` one, whose blob a JSON function
/// reads as JSON. `->>`, `json_type`, `json_valid` and `json_array_length` return plain values,
/// and `json_pretty` is the engine's to leave unsaid.
fn is_json_result(name: &str) -> bool {
    name.starts_with("jsonb")
        || matches!(
            name,
            "json" | "json_object" | "json_array" | "json_group_array" | "json_group_object" | "json_insert" | "json_replace" | "json_set" | "json_remove" | "json_patch" | "json_extract" | "->" | "json_quote"
        )
}

/// The built-ins that hand back one of their arguments as it is, subtype or not.
const PASSING: &[&str] = &["min", "max", "lag", "lead", "first_value", "last_value", "nth_value"];

/// The built-ins whose value is a new one of their own, which carries no subtype.
const PLAIN: &[&str] = &[
    "row_number", "rank", "dense_rank", "ntile", "percent_rank", "cume_dist", "count", "total", "sum", "avg", "group_concat", "string_agg", "abs", "round", "length", "instr",
    "json_valid", "octet_length", "json_array_length", "json_type", "unicode", "like", "glob", "regexp", "match", "concat", "concat_ws", "substr", "substring", "printf", "format",
    "upper", "lower", "trim", "ltrim", "rtrim", "replace", "char", "date", "time", "datetime", "julianday", "unixepoch", "strftime", "timediff", "hex", "quote", "typeof",
    "soundex", "sqlite_version", "sqlite_source_id", "current_date", "current_time", "current_timestamp", "random", "last_insert_rowid", "changes", "total_changes",
    "randomblob", "zeroblob", "->>", "atan", "atan2", "asinh", "sinh", "cosh", "tanh", "exp", "degrees", "radians", "ceil", "ceiling", "floor", "trunc", "sign", "acos", "acosh",
    "asin", "atanh", "cos", "sin", "tan", "ln", "log", "log10", "log2", "mod", "pow", "power", "sqrt", "pi",
    "bm25", "highlight", "snippet", "offsets", "matchinfo",
];

/// A value without the subtype, as SQLite writes it into JSON: an INTEGER (a 64-bit column's too)
/// as a number, a REAL as a number (an infinity as `9.0e+999`, which JSON.parse reads as one), a
/// TEXT as a string, and a NUMERIC as either. A BLOB is read as JSONB (`x'01'` is `true`,
/// `x'00'` null) or is an error, and a value of no known storage class may be one: any JSON.
fn in_json(t: &Type) -> Type {
    match scalar_name(t) {
        Some("integer" | "int8" | "real" | "text" | "numeric") => t.clone(),
        _ => Type::Unknown,
    }
}

/// Whether a type already admits a JSON null, so a field of it needs no `| null`.
fn admits_null(t: &Type) -> bool {
    matches!(t, Type::Unknown | Type::Json { schema: None })
}

/// The JSON Schema of a value inside a document SQLite built, as src/schema.ts spells SQLite's
/// values there, for a tuple's items and an object's values of any key.
fn value_schema(t: &Type, nullable: bool) -> serde_json::Value {
    use serde_json::json;
    let schema = match t {
        Type::Scalar { name } => match name.as_str() {
            "integer" | "int8" => json!({"type": "integer", "format": "int64"}),
            "real" => json!({"type": "number", "format": "double"}),
            "text" => json!({"type": "string"}),
            "numeric" => json!({"type": ["number", "string"]}),
            _ => json!({}),
        },
        Type::Object { fields } => {
            let properties: serde_json::Map<String, serde_json::Value> = fields.iter().map(|f| (f.name.clone(), value_schema(&f.ty, f.nullable))).collect();
            json!({"type": "object", "properties": properties, "required": fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>(), "additionalProperties": false})
        }
        Type::List { of, nullable_items } => json!({"type": "array", "items": value_schema(of, *nullable_items)}),
        Type::Json { schema: Some(schema) } => schema.clone(),
        _ => json!({}),
    };
    if nullable { reylo_engine_core::schema::nullable(schema) } else { schema }
}

/// An object of any keys whose values are each one of `values`.
fn record(values: Vec<serde_json::Value>) -> Type {
    let mut distinct: Vec<serde_json::Value> = Vec::new();
    for v in values {
        if !distinct.contains(&v) {
            distinct.push(v);
        }
    }
    let rest = if distinct.len() == 1 { distinct.pop().unwrap_or_default() } else { serde_json::json!({"anyOf": distinct}) };
    Type::Json { schema: Some(serde_json::json!({"type": "object", "additionalProperties": rest})) }
}

/// The FROM lists in scope, innermost last: a column reference names a cursor of one of them.
type ScopeStack<'a> = Vec<&'a [From]>;

struct Analyzer<'a> {
    tree: &'a Tree,
    extras: &'a Extras,
    /// Per table (lowercase), the events (`TRIGGER_ROW_INSERT`, `_UPDATE`, `_DELETE`) a BEFORE or
    /// INSTEAD OF trigger fires on: its `SELECT RAISE(IGNORE)` skips the row.
    skipping: &'a HashMap<String, i32>,
    /// The parameters (`?n`) the caller may pass NULL for because the statement passes them first
    /// to `coalesce` or `ifnull` ([`coalesced_params`]); with the bare ones written whole to a
    /// column that may hold NULL ([`Analyzer::params`]), every other one is a non-null value, as
    /// on Postgres and MySQL.
    nullable_params: HashSet<String>,
    /// What the WHERE and the grouping of the SELECT arm whose columns are being typed prove.
    level: std::cell::RefCell<Level>,
    /// The JSON each constructor builds, by its node ([`Analyzer::shape`]).
    shapes: std::cell::RefCell<HashMap<usize, Option<Type>>>,
    /// The recursive WITH queries being settled, innermost last: each one's name and what its reads
    /// of itself are taken to be ([`Analyzer::recursive_column`]).
    recursion: std::cell::RefCell<Vec<(String, Vec<Fact>)>>,
    /// Each recursive WITH query's settled columns, by its node.
    settled: std::cell::RefCell<HashMap<usize, Vec<Fact>>>,
}

/// What one SELECT arm's WHERE and grouping prove of the expressions in its select list.
#[derive(Debug, Clone, Default)]
struct Level {
    /// Columns (cursor, name) a strict WHERE test rejects NULL in: not NULL in a returned row.
    narrowed: HashSet<(i64, String)>,
    /// Cursors of outer-joined items a strict WHERE test reads: a returned row has their row.
    reduced: HashSet<i64>,
    /// A plain GROUP BY: no group is empty.
    grouped: bool,
}

/// The parameters the statement passes first to `coalesce` or `ifnull`, the two-argument
/// coalesce, bare or cast: the caller means to pass NULL for them. Beside these only a bare
/// parameter written whole to a column that may hold NULL takes NULL ([`Analyzer::params`]); a
/// null test (`?1 IS NULL`) or `IS` lets none through. The argument may be the column of a WITH query or a subquery that selects the parameter (`with
/// input as (select :p as p) … where coalesce(input.p, 0) = 0`), since each parameter appears once
/// in a statement and a value coalesced and read again is selected once in a WITH.
fn coalesced_params(t: &Tree, extras: &Extras) -> HashSet<String> {
    /// Every FROM item's subquery, a WITH query's too, by the item's cursor: SQLite numbers the
    /// cursors of one statement apart.
    fn subqueries<'a>(sel: &'a Sel, out: &mut HashMap<i64, &'a Sel>) {
        for f in &sel.from {
            if let Some(sub) = &f.subquery {
                out.insert(f.cursor, sub);
                subqueries(sub, out);
            }
        }
        for n in sel.columns.iter().map(|c| &c.expr).chain(sel.where_.iter()).chain(sel.having.iter()) {
            nested(n, out);
        }
        if let Some(prior) = &sel.prior {
            subqueries(prior, out);
        }
    }
    fn nested<'a>(n: &'a Node, out: &mut HashMap<i64, &'a Sel>) {
        if let Some(sel) = &n.select {
            subqueries(sel, out);
        }
        for child in n.left.iter().chain(n.right.iter()).map(|b| b.as_ref()).chain(n.list.iter()).chain(n.args.iter()) {
            nested(child, out);
        }
    }
    /// The parameter an operand is, under any casts and the `json()` or `jsonb()` a `::json` cast
    /// compiles to, or that the column of a subquery selects. A compound subquery's column is any
    /// of its arms', so it selects no one parameter.
    fn variable(x: Option<&Node>, subs: &HashMap<i64, &Sel>, extras: &Extras, depth: usize) -> Option<String> {
        let x = x?;
        match x.op.as_str() {
            "variable" => x.name.clone(),
            "cast" => variable(x.left.as_deref(), subs, extras, depth),
            "function" if matches!(x.name.as_deref(), Some("json" | "jsonb")) && x.args.len() == 1 && extras.function(x.name.as_deref().unwrap_or(""), 1).is_none() => variable(x.args.first(), subs, extras, depth),
            "column" if depth < 32 => {
                let sel = subs.get(&x.cursor).filter(|sel| sel.prior.is_none() && sel.rows.is_empty())?;
                variable(sel.columns.get(usize::try_from(x.index).ok()?).map(|c| &c.expr), subs, extras, depth + 1)
            }
            _ => None,
        }
    }
    fn walk(n: &Node, subs: &HashMap<i64, &Sel>, extras: &Extras, out: &mut HashSet<String>) {
        if n.op == "function" && matches!(n.name.as_deref(), Some("coalesce" | "ifnull")) {
            out.extend(variable(n.args.first(), subs, extras, 0));
        }
        for child in n.left.iter().chain(n.right.iter()).map(|b| b.as_ref()).chain(n.list.iter()).chain(n.args.iter()) {
            walk(child, subs, extras, out);
        }
        if let Some(sel) = &n.select {
            walk_select(sel, subs, extras, out);
        }
    }
    fn walk_select(sel: &Sel, subs: &HashMap<i64, &Sel>, extras: &Extras, out: &mut HashSet<String>) {
        for n in sel.columns.iter().map(|c| &c.expr).chain(sel.rows.iter().flatten()) {
            walk(n, subs, extras, out);
        }
        for n in sel.where_.iter().chain(sel.having.iter()).chain(sel.group_by.iter()).chain(sel.order_by.iter()).chain(sel.limit.iter()).chain(sel.offset.iter()) {
            walk(n, subs, extras, out);
        }
        for f in &sel.from {
            if let Some(sub) = &f.subquery {
                walk_select(sub, subs, extras, out);
            }
            for a in &f.args {
                walk(a, subs, extras, out);
            }
        }
        if let Some(prior) = &sel.prior {
            walk_select(prior, subs, extras, out);
        }
    }
    let mut subs = HashMap::new();
    if let Some(sel) = &t.statement {
        subqueries(sel, &mut subs);
    }
    if let Some(dml) = &t.dml {
        for n in dml.values.iter().flatten().chain(dml.set.iter().map(|s| &s.expr)).chain(dml.where_.iter()) {
            nested(n, &mut subs);
        }
    }
    let mut out = HashSet::new();
    if let Some(sel) = &t.statement {
        walk_select(sel, &subs, extras, &mut out);
    }
    if let Some(dml) = &t.dml {
        for n in dml.values.iter().flatten().chain(dml.set.iter().map(|s| &s.expr)).chain(dml.where_.iter()) {
            walk(n, &subs, extras, &mut out);
        }
        if let Some(u) = &dml.upsert {
            for n in u.set.iter().map(|s| &s.expr).chain(u.where_.iter()) {
                walk(n, &subs, extras, &mut out);
            }
        }
    }
    for c in t.returning.iter().flatten() {
        walk(&c.expr, &subs, extras, &mut out);
    }
    out
}

/// What a SELECT arm's WHERE proves of its own FROM items, by the strict tests at its top level
/// (not the ON terms of outer joins SQLite moved there), and whether it groups plainly.
fn level_of(sel: &Sel, extras: &Extras) -> Level {
    let own: HashSet<i64> = sel.from.iter().map(|f| f.cursor).collect();
    let outer: HashSet<i64> = sel.from.iter().filter(|f| f.outer).map(|f| f.cursor).collect();
    let mut narrowed = HashSet::new();
    for term in conjuncts(sel.where_.as_ref()).into_iter().filter(|t| !t.outer_on) {
        narrowed.extend(strict_in(term, extras).into_iter().filter(|(cursor, _)| own.contains(cursor)));
    }
    let reduced = narrowed.iter().map(|(cursor, _)| *cursor).filter(|c| outer.contains(c)).collect();
    Level { narrowed, reduced, grouped: !sel.group_by.is_empty() }
}

/// The columns a WHERE test at the top level rejects NULL in: under OR the ones every arm does; a
/// comparison, `IN`, `BETWEEN`, `IS NOT NULL`, `LIKE` or `GLOB`, in the columns its operands
/// answer NULL for. `IS` and `IS NOT` compare NULL like a value, and reject nothing. A function the
/// host registers over a built-in's name is the one a call resolves to, and proves nothing.
fn strict_in(n: &Node, extras: &Extras) -> HashSet<(i64, String)> {
    let nulls = |x: &Node| nulls_with(x, extras);
    match n.op.as_str() {
        "and" => n.left.iter().chain(n.right.iter()).flat_map(|x| strict_in(x, extras)).collect(),
        "or" => {
            let (Some(l), Some(r)) = (n.left.as_deref(), n.right.as_deref()) else { return HashSet::new() };
            let right = strict_in(r, extras);
            strict_in(l, extras).into_iter().filter(|c| right.contains(c)).collect()
        }
        "notnull" | "in" | "between" => n.left.as_deref().map(nulls).unwrap_or_default(),
        "=" | "!=" | "<" | "<=" | ">" | ">=" => n.left.iter().chain(n.right.iter()).flat_map(|x| nulls(x)).collect(),
        "not" => match n.left.as_deref() {
            Some(inner) if inner.op == "isnull" => inner.left.as_deref().map(nulls).unwrap_or_default(),
            Some(inner) => strict_in(inner, extras).into_iter().filter(|_| matches!(inner.op.as_str(), "=" | "!=" | "<" | "<=" | ">" | ">=" | "in" | "between")).collect(),
            None => HashSet::new(),
        },
        "function" if !n.agg && !n.window && matches!(n.name.as_deref(), Some("like" | "glob")) && extras.function(n.name.as_deref().unwrap_or(""), n.args.len()).is_none() => {
            n.args.iter().flat_map(nulls).collect()
        }
        "column" => nulls(n),
        _ => HashSet::new(),
    }
}

/// The columns an expression answers NULL for: the column itself, and through arithmetic,
/// concatenation, a cast, a COLLATE or a built-in that is NULL for a NULL argument (not one the
/// host registers over it), every column an operand answers NULL for.
fn nulls_with(n: &Node, extras: &Extras) -> HashSet<(i64, String)> {
    const STRICT: &[&str] = &["upper", "lower", "length", "abs", "trim", "ltrim", "rtrim", "substr", "substring", "replace", "round", "instr", "unicode", "sign", "octet_length"];
    match n.op.as_str() {
        "column" => n.column.clone().map(|c| HashSet::from([(n.cursor, c)])).unwrap_or_default(),
        "+" | "-" | "*" | "/" | "%" | "||" => n.left.iter().chain(n.right.iter()).flat_map(|x| nulls_with(x, extras)).collect(),
        "neg" | "pos" | "cast" | "collate" => n.left.as_deref().map(|x| nulls_with(x, extras)).unwrap_or_default(),
        "function" if !n.agg && !n.window && n.name.as_deref().is_some_and(|f| STRICT.contains(&f) && extras.function(f, n.args.len()).is_none()) => n.args.iter().flat_map(|x| nulls_with(x, extras)).collect(),
        _ => HashSet::new(),
    }
}

/// The words of a statement as SQLite's tokenizer splits them: a name or a keyword, and each
/// other character alone; comments are none. A quoted name (`"a b"`, `` `a` ``, `[a]`, `'a'`) is
/// one word without its quotes, and is marked, since it is never a keyword.
fn words(sql: &str) -> Vec<(String, bool)> {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '-' && chars.get(i + 1) == Some(&'-') {
            i += chars[i..].iter().position(|c| *c == '\n').unwrap_or(chars.len() - i);
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i += 2;
        } else if matches!(c, '"' | '`' | '\'' | '[') {
            // A doubled quote inside the name is the quote itself; `[` ends at the first `]`.
            let close = if c == '[' { ']' } else { c };
            let mut name = String::new();
            i += 1;
            while i < chars.len() {
                if chars[i] == close {
                    if close != ']' && chars.get(i + 1) == Some(&close) {
                        i += 1;
                    } else {
                        break;
                    }
                }
                name.push(chars[i]);
                i += 1;
            }
            i += 1;
            out.push((name, true));
        } else if c.is_alphanumeric() || c == '_' || c == '$' || !c.is_ascii() {
            let end = chars[i..].iter().position(|c| !(c.is_alphanumeric() || *c == '_' || *c == '$' || !c.is_ascii())).map_or(chars.len(), |n| i + n);
            out.push((chars[i..end].iter().collect(), false));
            i = end;
        } else {
            out.push((c.to_string(), false));
            i += 1;
        }
    }
    out
}

/// A `CREATE TRIGGER` statement's timing (`before`, `after`, `instead`), event (`insert`,
/// `update`, `delete`) and table, the table in ASCII lowercase, as SQLite compares names; `None`
/// for any other statement.
fn trigger_header(sql: &str) -> Option<(String, String, String)> {
    // Most of a schema is tables and indexes, which need no reading here.
    if !sql.as_bytes().windows(7).any(|w| w.eq_ignore_ascii_case(b"trigger")) {
        return None;
    }
    let words = words(sql);
    let keyword = |i: usize| words.get(i).filter(|(_, quoted)| !quoted).map(|(w, _)| w.to_ascii_lowercase());
    // Steps over the next word when it is one of the keywords `want`.
    let take = |i: &mut usize, want: &[&str]| {
        let hit = keyword(*i).is_some_and(|w| want.contains(&w.as_str()));
        if hit {
            *i += 1;
        }
        hit
    };
    let mut i = 0;
    if !take(&mut i, &["create"]) {
        return None;
    }
    take(&mut i, &["temp", "temporary"]);
    if !take(&mut i, &["trigger"]) {
        return None;
    }
    if take(&mut i, &["if"]) {
        take(&mut i, &["not"]);
        take(&mut i, &["exists"]);
    }
    // The trigger's name, with its schema when it has one.
    i += 1;
    if take(&mut i, &["."]) {
        i += 1;
    }
    let timing = if take(&mut i, &["before"]) {
        "before"
    } else if take(&mut i, &["after"]) {
        "after"
    } else if take(&mut i, &["instead"]) {
        take(&mut i, &["of"]);
        "instead"
    } else {
        // "If no timing is specified, BEFORE is the default."
        "before"
    };
    let event = keyword(i)?;
    // `UPDATE OF a, b ON t`: the first ON that is no quoted column name.
    let on = (i..words.len()).find(|j| keyword(*j).as_deref() == Some("on"))?;
    let mut table = &words.get(on + 1)?.0;
    if keyword(on + 2).as_deref() == Some(".") {
        table = &words.get(on + 3)?.0;
    }
    Some((timing.to_string(), event, table.to_ascii_lowercase()))
}

/// The events of a row trigger, as bits: Postgres's `pg_trigger.tgtype` values, which the two
/// proofs have always shared.
const TRIGGER_ROW_INSERT: i32 = 4;
const TRIGGER_ROW_DELETE: i32 = 8;
const TRIGGER_ROW_UPDATE: i32 = 16;

/// The events per table that some BEFORE or INSTEAD OF trigger fires on.
fn skipping_triggers(triggers: &[String]) -> HashMap<String, i32> {
    let mut out: HashMap<String, i32> = HashMap::new();
    for sql in triggers {
        let Some((timing, event, table)) = trigger_header(sql) else { continue };
        if timing == "after" {
            continue;
        }
        let bit = match event.as_str() {
            "insert" => TRIGGER_ROW_INSERT,
            "update" => TRIGGER_ROW_UPDATE,
            "delete" => TRIGGER_ROW_DELETE,
            _ => TRIGGER_ROW_INSERT | TRIGGER_ROW_UPDATE | TRIGGER_ROW_DELETE,
        };
        *out.entry(table).or_default() |= bit;
    }
    out
}

/// When a function's result can be NULL.
#[derive(Clone, Copy)]
enum Nulls {
    /// Never, whatever the arguments.
    Never,
    /// Only when an argument can be.
    IfArgs,
    /// Always possibly (an aggregate over no rows, a lookup that can miss).
    Always,
}

impl Analyzer<'_> {
    // ---- result columns ---------------------------------------------------------------------

    fn columns(&self) -> (Vec<Column>, String) {
        let t = self.tree;
        let kind = t.dml.as_ref().map(|d| if d.kind == "upsert" { "insert".to_string() } else { d.kind.clone() }).unwrap_or_else(|| if t.statement.is_some() { "select".into() } else { String::new() });
        let facts: Vec<Fact> = if let Some(returning) = &t.returning {
            returning.iter().map(|c| self.type_of(&c.expr, &Vec::new())).collect()
        } else if let (Some(sel), None) = (&t.statement, &t.dml) {
            (0..sel.columns.len()).map(|i| self.select_column(sel, i, &Vec::new())).collect()
        } else {
            Vec::new()
        };
        let columns = t
            .columns
            .iter()
            .enumerate()
            .map(|(i, public)| {
                let fact = facts.get(i).cloned().unwrap_or_else(Fact::unknown);
                // Built JSON is the text SQLite returns, which the run time parses.
                let type_name = if is_built(&fact.ty) { "text".to_string() } else { scalar_name(&fact.ty).unwrap_or("").to_string() };
                Column { name: public.name.clone(), ty: fact.ty, nullable: !fact.non_null, type_name, comment: None }
            })
            .collect();
        (columns, kind)
    }

    /// Per result column, whether it is a truth value ([`SqliteAnalysis::booleans`]).
    fn booleans(&self) -> Vec<bool> {
        self.result_truths(false)
    }

    /// Per result column, whether its values are booleans to a host that decodes them: a truth
    /// value, or a column of a table that declares it `BOOLEAN` or `BOOL`, as Prisma's and other
    /// schemas do, read through any subquery or view, or what passes one of those through. SQLite
    /// holds such a column to nothing (`BOOLEAN` is a NUMERIC affinity), so a host reads 0 and 1
    /// and refuses any other value it finds there.
    fn flags(&self) -> Vec<bool> {
        self.result_truths(true)
    }

    fn result_truths(&self, declared: bool) -> Vec<bool> {
        let t = self.tree;
        let truths: Vec<bool> = if let Some(returning) = &t.returning {
            returning.iter().map(|c| self.truth(&c.expr, &Vec::new(), declared)).collect()
        } else if let (Some(sel), None) = (&t.statement, &t.dml) {
            (0..sel.columns.len()).map(|i| self.select_truth(sel, i, &Vec::new(), declared)).collect()
        } else {
            Vec::new()
        };
        (0..t.columns.len()).map(|i| truths.get(i).copied().unwrap_or(false)).collect()
    }

    /// Whether column `i` of a select is a truth value in every arm of a compound.
    fn select_truth(&self, sel: &Sel, i: usize, scope: &ScopeStack, declared: bool) -> bool {
        let mut s = Some(sel);
        while let Some(arm) = s {
            let mut inner = scope.clone();
            inner.push(&arm.from);
            if !arm.columns.get(i).is_some_and(|c| self.truth(&c.expr, &inner, declared)) || !arm.rows.iter().all(|row| row.get(i).is_some_and(|e| self.truth(e, &inner, declared))) {
                return false;
            }
            s = arm.prior.as_deref();
        }
        true
    }

    /// Whether an expression is a truth value: what a comparison, a test or a logical operator
    /// answers, and what passes one through unchanged. A NULL arm is no counterexample (`CASE`
    /// without `ELSE`), and nullability is [`Analyzer::type_of`]'s business. `declared`: a table's
    /// column declared `BOOLEAN` or `BOOL` counts as one too ([`Analyzer::flags`]).
    fn truth(&self, n: &Node, scope: &ScopeStack, declared: bool) -> bool {
        let every = |nodes: &mut dyn Iterator<Item = &Node>| {
            let mut any = false;
            for x in nodes {
                if x.op == "null" {
                    continue;
                }
                if !self.truth(x, scope, declared) {
                    return false;
                }
                any = true;
            }
            any
        };
        match n.op.as_str() {
            "=" | "!=" | "<" | "<=" | ">" | ">=" | "is" | "is not" | "isnull" | "notnull" | "in" | "between" | "exists" | "and" | "or" | "not" | "truefalse" => true,
            "collate" => n.left.as_deref().is_some_and(|l| self.truth(l, scope, declared)),
            "function" if !n.agg && !n.window => match n.name.as_deref().unwrap_or("") {
                "like" | "glob" | "regexp" | "match" => self.extras.function(n.name.as_deref().unwrap_or(""), n.args.len()).is_none(),
                "likely" | "unlikely" | "likelihood" => n.args.first().is_some_and(|a| self.truth(a, scope, declared)),
                "coalesce" | "ifnull" => every(&mut n.args.iter()),
                "iif" | "if" => every(&mut iif_values(n.args.len()).into_iter().filter_map(|i| n.args.get(i))),
                _ => false,
            },
            // `list` alternates WHEN and THEN; an odd tail is the ELSE.
            "case" => {
                let results = n.list.chunks(2).filter_map(|pair| if pair.len() == 2 { Some(&pair[1]) } else { None });
                let otherwise = if n.list.len() % 2 == 1 { n.list.last() } else { None };
                every(&mut results.chain(otherwise))
            }
            "select" => n.select.as_deref().is_some_and(|sub| self.select_truth(sub, 0, scope, declared)),
            "column" => {
                for froms in scope.iter().rev() {
                    if let Some(f) = froms.iter().find(|f| f.cursor == n.cursor) {
                        if let Some(sub) = f.subquery.as_deref() {
                            return self.select_truth(sub, n.index.max(0) as usize, scope, declared);
                        }
                        break;
                    }
                }
                declared && !n.rowid && n.declared.as_deref().is_some_and(is_boolean_declared)
            }
            _ => false,
        }
    }

    /// Column `i` of a select: for a compound, the type every arm's value shares ([`values_type`]),
    /// and any arm's nullability. Every row of a multi-row VALUES is an arm.
    fn select_column(&self, sel: &Sel, i: usize, scope: &ScopeStack) -> Fact {
        let mut arms: Vec<&Sel> = Vec::new();
        let mut s = Some(sel);
        while let Some(arm) = s {
            arms.push(arm);
            s = arm.prior.as_deref();
        }
        arms.reverse();
        // Each arm's value, and the expression it is.
        let facts: Vec<(Option<&Node>, Fact)> = arms
            .iter()
            .flat_map(|arm| {
                let mut inner = scope.clone();
                inner.push(&arm.from);
                let outer = self.level.replace(level_of(arm, self.extras));
                let rows = std::iter::once(arm.columns.get(i).map(|c| &c.expr)).chain(arm.rows.iter().map(|row| row.get(i)));
                let facts: Vec<(Option<&Node>, Fact)> = rows.map(|e| (e, e.map(|e| self.type_of(e, &inner)).unwrap_or_else(Fact::unknown))).collect();
                self.level.replace(outer);
                facts
            })
            .collect();
        let Some((_, first)) = facts.first() else { return Fact::unknown() };
        let values: Option<Vec<(&Node, &Fact)>> = facts.iter().map(|(e, f)| Some(((*e)?, f))).collect();
        let ty = match values {
            _ if facts.iter().all(|(_, f)| f.ty == first.ty) => first.ty.clone(),
            Some(values) => values_type(&values),
            None => Type::Unknown,
        };
        Fact { ty, non_null: facts.iter().all(|(_, f)| f.non_null) }
    }

    /// Column `i` of a recursive WITH query, which reads itself, so that its columns are a fixed
    /// point, and the greatest one is sound. Each column is first taken to be non-null and of the
    /// type its first arm gives it, for the query's reads of itself; the query is typed under that
    /// assumption; a column that comes out nullable, or of another type, is taken so, and the query
    /// is typed again, until nothing changes. What survives holds by induction over the recursion:
    /// the first arm's rows are so on their own account, each later row given the rows it was
    /// computed from, and a UNION's column is non-null only where every arm's is. A query that does
    /// not settle within a few rounds proves nothing.
    fn recursive_column(&self, name: &str, sub: &Sel, i: usize, scope: &ScopeStack) -> Fact {
        let key = sub as *const Sel as usize;
        if let Some(facts) = self.settled.borrow().get(&key) {
            return facts.get(i).cloned().unwrap_or_else(Fact::unknown);
        }
        let mut first = sub;
        while let Some(prior) = first.prior.as_deref() {
            first = prior;
        }
        let width = first.columns.len();
        let mut inner = scope.clone();
        inner.push(&first.from);
        let outer = self.level.replace(level_of(first, self.extras));
        let mut assumed: Vec<Fact> = first.columns.iter().map(|c| Fact { ty: self.type_of(&c.expr, &inner).ty, non_null: true }).collect();
        self.level.replace(outer);
        let mut settled = None;
        for _ in 0..=width + 1 {
            self.recursion.borrow_mut().push((name.to_string(), assumed.clone()));
            let got: Vec<Fact> = (0..width).map(|k| self.select_column(sub, k, scope)).collect();
            self.recursion.borrow_mut().pop();
            if got.iter().zip(&assumed).all(|(g, a)| g.ty == a.ty && g.non_null == a.non_null) {
                settled = Some(got);
                break;
            }
            assumed = got.iter().zip(&assumed).map(|(g, a)| Fact { ty: g.ty.clone(), non_null: g.non_null && a.non_null }).collect();
        }
        let facts = settled.unwrap_or_else(|| vec![Fact::unknown(); width]);
        let fact = facts.get(i).cloned().unwrap_or_else(Fact::unknown);
        self.settled.borrow_mut().insert(key, facts);
        fact
    }

    /// A column reference: through the cursor to a subquery's column, or the table column itself.
    fn column_ref(&self, n: &Node, scope: &ScopeStack) -> Fact {
        // A strict WHERE test on the column, or on any column of its outer-joined item, keeps
        // its NULL (or the item's null extension) out of every returned row.
        let (narrowed, reduced) = {
            let level = self.level.borrow();
            (n.column.as_ref().is_some_and(|c| level.narrowed.contains(&(n.cursor, c.clone()))), level.reduced.contains(&n.cursor))
        };
        for froms in scope.iter().rev() {
            if let Some(f) = froms.iter().find(|f| f.cursor == n.cursor) {
                let index = n.index.max(0) as usize;
                let inner = match (&f.subquery, f.table.as_deref()) {
                    (Some(sub), Some(name)) if f.cte && reads_itself(sub) => Some(self.recursive_column(name, sub, index, scope)),
                    (Some(sub), _) => Some(self.select_column(sub, index, scope)),
                    // A recursive WITH query's read of itself, inside the query: what it is taken to be.
                    (None, Some(name)) if f.recursive => self.recursion.borrow().iter().rev().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, facts)| facts.get(index).cloned().unwrap_or_else(Fact::unknown)),
                    _ => None,
                };
                if let Some(inner) = inner {
                    return Fact { ty: inner.ty, non_null: narrowed || (inner.non_null && ((!f.outer && !n.outer) || reduced)) };
                }
                break;
            }
        }
        let aff = n.affinity.as_deref().unwrap_or(&n.aff);
        let ty = match &n.declared {
            Some(declared) if !n.rowid => column_type(declared, aff, n.strict),
            _ => affinity_type(aff),
        };
        Fact { ty, non_null: narrowed || ((n.not_null || n.rowid) && (!n.outer || reduced)) }
    }

    // ---- expressions -------------------------------------------------------------------------

    fn type_of(&self, n: &Node, scope: &ScopeStack) -> Fact {
        let arg = |x: &Option<Box<Node>>| x.as_deref().map(|x| self.type_of(x, scope)).unwrap_or_else(Fact::unknown);
        match n.op.as_str() {
            "column" => self.column_ref(n, scope),
            "integer" | "truefalse" => Fact::new("integer", true),
            "float" => Fact::new("real", true),
            "string" => Fact::new("text", true),
            "blob" => Fact::new("blob", true),
            "null" => Fact::unknown(),
            // A parameter is a non-null value unless the statement means to take NULL for it.
            "variable" => Fact { ty: Type::Unknown, non_null: n.name.as_ref().is_some_and(|name| !self.nullable_params.contains(name)) },
            "function" => {
                // The JSON a `json_*` constructor builds, which SQLite returns as its text.
                if let Some(ty) = self.built_text(n, scope) {
                    // `json()` of a scalar subquery is NULL where the subquery finds no row; every
                    // other constructor reads that NULL as a null (`[null]`, `{}`) and is never NULL.
                    let non_null = match n.args.first() {
                        Some(a) if a.op == "select" && self.builtin_name(n) == Some("json") => self.type_of(a, scope).non_null,
                        _ => true,
                    };
                    return Fact { ty, non_null };
                }
                let args: Vec<Fact> = n.args.iter().map(|a| self.type_of(a, scope)).collect();
                self.function(n, &args)
            }
            "+" | "-" | "*" => {
                let (l, r) = (arg(&n.left), arg(&n.right));
                Fact { ty: arithmetic(&l.ty, &r.ty), non_null: l.non_null && r.non_null }
            }
            "/" | "%" => {
                // A zero divisor gives NULL; a literal divisor is one the statement rules out.
                let (l, r) = (arg(&n.left), arg(&n.right));
                let literal_divisor = n.right.as_deref().is_some_and(|d| matches!(d.op.as_str(), "integer" | "float") && d.value.as_deref().and_then(|v| v.parse::<f64>().ok()).is_some_and(|v| v != 0.0));
                Fact { ty: arithmetic(&l.ty, &r.ty), non_null: l.non_null && r.non_null && literal_divisor }
            }
            "||" => {
                let (l, r) = (arg(&n.left), arg(&n.right));
                Fact::new("text", l.non_null && r.non_null)
            }
            "=" | "!=" | "<" | "<=" | ">" | ">=" | "and" | "or" | "in" | "between" | "&" | "|" | "<<" | ">>" => {
                let (l, r) = (arg(&n.left), arg(&n.right));
                let list_non_null = n.list.iter().all(|x| self.type_of(x, scope).non_null);
                let right_non_null = if n.right.is_some() { r.non_null } else { list_non_null && n.select.is_none() };
                Fact::new("integer", l.non_null && right_non_null)
            }
            "is" | "is not" | "isnull" | "notnull" | "exists" => Fact::new("integer", true),
            "not" | "~" => {
                let l = arg(&n.left);
                Fact::new("integer", l.non_null)
            }
            "neg" => {
                let l = arg(&n.left);
                Fact { ty: if is_numeric(&l.ty) { computed(l.ty) } else { Type::Scalar { name: "real".into() } }, non_null: l.non_null }
            }
            // Unary plus does nothing to any storage class: `+name` is the text 'bob'. SQLite's
            // parser also writes a one-element `x IN (e)` as `x = +e`.
            "pos" => arg(&n.left),
            "cast" => {
                let l = arg(&n.left);
                Fact::new(n.affinity.as_deref().unwrap_or("none"), l.non_null)
            }
            "collate" => arg(&n.left),
            "case" => {
                // `list` alternates WHEN and THEN; an odd tail is the ELSE.
                let results: Vec<(&Node, Fact)> = n.list.chunks(2).filter_map(|pair| if pair.len() == 2 { Some((&pair[1], self.type_of(&pair[1], scope))) } else { None }).collect();
                let otherwise = if n.list.len() % 2 == 1 { n.list.last().map(|e| (e, self.type_of(e, scope))) } else { None };
                let mut all: Vec<&(&Node, Fact)> = results.iter().collect();
                if let Some(o) = &otherwise {
                    all.push(o);
                }
                let values: Vec<(&Node, &Fact)> = all.iter().map(|(e, f)| (*e, f)).collect();
                Fact { ty: values_type(&values), non_null: otherwise.is_some() && all.iter().all(|(_, f)| f.non_null) }
            }
            "select" => {
                // A scalar subquery: its first column, NULL when it finds no row, unless it always
                // finds exactly one (an ungrouped aggregate: `(SELECT count(*) FROM t)`). Its
                // column is then its own answer, so `(SELECT max(x) FROM t)` stays nullable.
                let Some(sub) = n.select.as_deref() else { return Fact::unknown() };
                let inner = self.select_column(sub, 0, scope);
                Fact { ty: inner.ty, non_null: inner.non_null && self.exactly_one(sub).is_ok() }
            }
            _ => Fact::unknown(),
        }
    }

    /// The built-in functions: their result type, and when it is NULL.
    fn function(&self, n: &Node, args: &[Fact]) -> Fact {
        let name = n.name.as_deref().unwrap_or("");
        let all_non_null = args.iter().all(|a| a.non_null);
        let first = args.first().cloned().unwrap_or_else(Fact::unknown);
        let done = |aff: &str, nulls: Nulls| {
            let non_null = match nulls {
                Nulls::Never => true,
                Nulls::IfArgs => all_non_null,
                Nulls::Always => false,
            };
            Fact::new(aff, non_null)
        };
        let typed = |ty: Type, nulls: Nulls| Fact { ty, non_null: matches!(nulls, Nulls::Never) || (matches!(nulls, Nulls::IfArgs) && all_non_null) };
        if !n.agg && !n.window {
            if let Some(f) = self.extras.function(name, args.len()) {
                return done(f.returns.name(), Nulls::IfArgs);
            }
        }
        if n.window {
            return match name {
                "row_number" | "rank" | "dense_rank" | "ntile" => done("integer", Nulls::Never),
                "percent_rank" | "cume_dist" => done("real", Nulls::Never),
                "count" => done("integer", Nulls::Never),
                "total" => done("real", total_nulls(&first)),
                "sum" => typed(if is_numeric(&first.ty) { computed(first.ty.clone()) } else { Type::Scalar { name: "real".into() } }, Nulls::Always),
                "avg" => done("real", Nulls::Always),
                // `lag` and `lead` hand back their default where the row they read is missing.
                "lag" | "lead" if args.len() >= 3 => typed(values_type(&[(&n.args[0], &args[0]), (&n.args[2], &args[2])]), Nulls::Always),
                "min" | "max" | "lag" | "lead" | "first_value" | "last_value" | "nth_value" => typed(first.ty.clone(), Nulls::Always),
                "group_concat" | "string_agg" => done("text", Nulls::Always),
                _ => Fact::unknown(),
            };
        }
        if n.agg {
            // Under a plain GROUP BY every group has a row, so an aggregate that is NULL only
            // over no rows or only NULLs follows its argument. (A FILTER makes the call a window
            // call in SQLite's tree, and is read above.) `sum` stays nullable: a sum of `1e999`
            // and `-1e999` is NaN, which SQLite stores as NULL, and so is a `total` of them.
            let grouped = self.level.borrow().grouped;
            return match name {
                "count" => done("integer", Nulls::Never),
                "total" => done("real", total_nulls(&first)),
                "sum" => typed(if is_numeric(&first.ty) { computed(first.ty.clone()) } else { Type::Scalar { name: "real".into() } }, Nulls::Always),
                "avg" => done("real", Nulls::Always),
                "min" | "max" => typed(first.ty.clone(), if grouped { Nulls::IfArgs } else { Nulls::Always }),
                "group_concat" | "string_agg" => done("text", if grouped { Nulls::IfArgs } else { Nulls::Always }),
                "json_group_array" | "json_group_object" => done("text", Nulls::Never),
                "jsonb_group_array" | "jsonb_group_object" => done("blob", Nulls::Never),
                _ => Fact::unknown(),
            };
        }
        // A number for SQLite's math functions: a value of INTEGER or REAL type. Any other gives
        // NULL, text that reads as no number included (`sqrt('abc')`, `sign('')`).
        let numbers = all_non_null && args.iter().all(|a| matches!(scalar_name(&a.ty), Some("integer" | "int8" | "real")));
        match name {
            // An argument after one that is never NULL is never reached: `coalesce((select
            // json_group_array(…) …), '[]')` is the array.
            "coalesce" | "ifnull" => Fact { ty: values_type(&n.args.iter().zip(reached(args)).collect::<Vec<_>>()), non_null: args.iter().any(|a| a.non_null) },
            "nullif" => Fact { ty: first.ty.clone(), non_null: false },
            // `iif(c1, v1, c2, v2, …, else)`: NULL where no condition holds and there is no ELSE,
            // an even count of arguments.
            "iif" | "if" => {
                let values = iif_values(args.len());
                let arms: Vec<(&Node, &Fact)> = values.iter().filter_map(|i| Some((n.args.get(*i)?, args.get(*i)?))).collect();
                Fact { ty: values_type(&arms), non_null: args.len() >= 3 && args.len() % 2 == 1 && arms.iter().all(|(_, a)| a.non_null) }
            }
            "min" | "max" => Fact { ty: values_type(&n.args.iter().zip(args).collect::<Vec<_>>()), non_null: all_non_null },
            // `abs` of the least integer is an error, not NULL; of text, 0.0.
            "abs" => typed(if is_numeric(&first.ty) { computed(first.ty.clone()) } else { Type::Scalar { name: "real".into() } }, Nulls::IfArgs),
            "round" => done("real", Nulls::IfArgs),
            "length" | "instr" | "json_valid" | "octet_length" => done("integer", Nulls::IfArgs),
            // A path that finds nothing is NULL: `json_array_length('{}', '$.a')`, `json_type('{}', '$.a')`.
            "json_array_length" => done("integer", if args.len() == 1 { Nulls::IfArgs } else { Nulls::Always }),
            "json_type" => done("text", if args.len() == 1 { Nulls::IfArgs } else { Nulls::Always }),
            // `json_remove(j, '$')` is NULL, and so is a NULL path; with no path it is `json(j)`.
            "json_remove" => done("text", if args.len() == 1 { Nulls::IfArgs } else { Nulls::Always }),
            // `unicode('')` is NULL: an empty string has no first character.
            "unicode" => done("integer", Nulls::Always),
            "like" | "glob" | "regexp" | "match" | "likely" | "unlikely" | "likelihood" => {
                if matches!(name, "likely" | "unlikely" | "likelihood") { first.clone() } else { done("integer", Nulls::IfArgs) }
            }
            // `concat` skips NULL arguments (all NULL is ''), and `concat_ws` too, NULL only for a
            // NULL separator.
            "concat" => done("text", Nulls::Never),
            "concat_ws" => Fact::new("text", first.non_null),
            // A blob's slice is a blob, and the slice of an empty blob is NULL (`substr(x'', 1)`):
            // non-null only over text. A parameter there is typed text ([`Analyzer::argument`]).
            "substr" | "substring" => {
                let blob = matches!(scalar_name(&first.ty), Some("blob"));
                let any = matches!(first.ty, Type::Unknown) && n.args.first().is_some_and(|a| a.op != "variable");
                let ty = if blob { first.ty.clone() } else if any { Type::Unknown } else { Type::Scalar { name: "text".into() } };
                Fact { ty, non_null: all_non_null && !blob && !any }
            }
            // NULL where it writes nothing, as for an empty format or one that stops at a
            // conversion SQLite does not know (`printf('%T')`).
            "printf" | "format" => Fact::new("text", n.args.first().and_then(literal_text).is_some_and(printf_writes)),
            "upper" | "lower" | "trim" | "ltrim" | "rtrim" | "replace" | "json" | "json_patch" | "json_pretty" => done("text", Nulls::IfArgs),
            // A NULL argument is a JSON null or the code point 0; a NULL path changes nothing.
            "char" | "json_array" | "json_object" | "json_quote" => done("text", Nulls::Never),
            "json_insert" | "json_replace" | "json_set" => Fact::new("text", first.non_null),
            "jsonb" => done("blob", Nulls::IfArgs),
            // The date and time functions are NULL for a time value they cannot read and for a
            // modifier that fails, so they are non-null only for the current time with modifiers
            // that cannot fail on it (`date()`, `datetime('now', 'localtime')`).
            "date" | "time" | "datetime" => Fact::new("text", current_time(&n.args)),
            "julianday" => Fact::new("real", current_time(&n.args)),
            // `unixepoch(…, 'subsec')` is a real.
            "unixepoch" => {
                let whole = n.args.iter().skip(1).all(|m| literal_text(m).is_some_and(|m| !matches!(m.to_ascii_lowercase().as_str(), "subsec" | "subsecond")));
                Fact::new(if whole { "integer" } else { "real" }, current_time(&n.args))
            }
            // A conversion it does not know makes `strftime` NULL (`strftime('%Q')`).
            "strftime" => Fact::new("text", n.args.first().and_then(literal_text).is_some_and(strftime_format) && current_time(n.args.get(1..).unwrap_or_default())),
            "timediff" => Fact::new("text", n.args.len() == 2 && current_time(&n.args[..1]) && current_time(&n.args[1..])),
            // `soundex(NULL)` is '?000'.
            "hex" | "quote" | "typeof" | "soundex" => done("text", Nulls::Never),
            "sqlite_version" | "sqlite_source_id" | "current_date" | "current_time" | "current_timestamp" => done("text", Nulls::Never),
            "random" | "last_insert_rowid" | "changes" | "total_changes" => done("integer", Nulls::Never),
            "randomblob" | "zeroblob" => done("blob", Nulls::IfArgs),
            "json_extract" | "jsonb_extract" | "->" | "->>" => Fact::unknown(),
            // Defined on every number, infinities included: NULL only for an argument that is no
            // number.
            "atan" | "atan2" | "asinh" | "sinh" | "cosh" | "tanh" | "exp" | "degrees" | "radians" | "ceil" | "ceiling" | "floor" | "trunc" => Fact::new("real", numbers),
            "sign" => Fact::new("integer", numbers),
            // NULL outside their domain (`sqrt(-1)`, `ln(0)`, `acos(2)`, `mod(1, 0)`,
            // `pow(-1, 0.5)`) and for an infinite argument (`sin(1e999)`).
            "acos" | "acosh" | "asin" | "atanh" | "cos" | "sin" | "tan" | "ln" | "log" | "log10" | "log2" | "mod" | "pow" | "power" | "sqrt" => done("real", Nulls::Always),
            "pi" => done("real", Nulls::Never),
            // The full-text functions, which an FTS table answers for the rows of its own MATCH.
            // FTS5's `bm25` is a real on every row, 0 where no MATCH ranks it; with weights it is
            // NULL where an infinite or overflowing weight makes the score NaN
            // (`bm25(t, 1e308, 1.0)` on a row that matches twice). `highlight` is NULL
            // where its column is, and so is FTS5's `snippet` where its column is (every column,
            // for -1); FTS4's `snippet` has the same name. FTS3's and FTS4's `offsets` and
            // `matchinfo` answer every row.
            "bm25" => done("real", if n.args.len() == 1 { Nulls::Never } else { Nulls::Always }),
            "highlight" | "snippet" => done("text", Nulls::Always),
            "offsets" => done("text", Nulls::Never),
            "matchinfo" => done("blob", Nulls::Never),
            _ => Fact::unknown(),
        }
    }

    // ---- built JSON ----------------------------------------------------------------------------
    //
    // SQLite's JSON functions answer with text (the `jsonb_*` ones with a blob), so a column a
    // constructor builds was `text`. Its shape is in the statement, as on Postgres and MySQL, and
    // the run time parses the text: what follows types it as the JSON it is. Inside a document a
    // value is what SQLite writes there, measured on SQLite 3.53.4 (wiki/dialects.md): an INTEGER,
    // a REAL and a TEXT as a JSON number or string, NULL as null, a truth value as 1 or 0. A value
    // that carries SQLite's JSON subtype, which every JSON function sets on what it returns, nests
    // as JSON rather than as a string, and the subtype passes through some expressions and not
    // others ([`Carry`]).

    /// The JSON a call that SQLite answers with text builds: a `json_object`, `json_array`,
    /// `json_group_array` or `json_group_object` call, or `json()` of one of those or of a
    /// `jsonb_*` constructor. A `jsonb_*` call is a blob, which stays one.
    fn built_text(&self, n: &Node, scope: &ScopeStack) -> Option<Type> {
        match self.builtin_name(n)? {
            "json_object" | "json_array" | "json_group_array" | "json_group_object" => self.shape(n, scope),
            "json" if n.args.len() == 1 && !n.agg && !n.window => self.shape(&n.args[0], scope).or_else(|| self.subquery_shape(&n.args[0], scope)),
            _ => None,
        }
    }

    /// The JSON a scalar subquery's one column builds, which `json()` of the subquery marks as
    /// JSON again whether or not the subquery's value kept SQLite's subtype ([`Analyzer::carry`]).
    /// None for a compound or a `VALUES` list, whose rows the first arm's column does not type.
    fn subquery_shape(&self, n: &Node, scope: &ScopeStack) -> Option<Type> {
        if n.op != "select" {
            return None;
        }
        let sub = n.select.as_deref()?;
        if sub.columns.len() != 1 || sub.prior.is_some() || !sub.rows.is_empty() || sub.values {
            return None;
        }
        let col = sub.columns.first()?;
        self.within(sub, scope, |inner| self.built_text(&col.expr, inner))
    }

    /// The built-in a call resolves to: none where the host registers a scalar function of that
    /// name and argument count, which SQLite prefers.
    fn builtin_name<'n>(&self, n: &'n Node) -> Option<&'n str> {
        let name = n.name.as_deref().filter(|_| n.op == "function")?;
        (n.agg || n.window || self.extras.function(name, n.args.len()).is_none()).then_some(name)
    }

    /// The JSON a constructor builds, `json_*` or `jsonb_*`, each value as SQLite writes it
    /// there ([`Analyzer::json_fact`]); `json()` and `jsonb()` of one build the same. Each node's
    /// shape is worked out once: a constructor's nullability and its parent's shape both read it.
    fn shape(&self, n: &Node, scope: &ScopeStack) -> Option<Type> {
        let key = n as *const Node as usize;
        if let Some(found) = self.shapes.borrow().get(&key) {
            return found.clone();
        }
        let shape = self.shape_here(n, scope);
        self.shapes.borrow_mut().insert(key, shape.clone());
        shape
    }

    fn shape_here(&self, n: &Node, scope: &ScopeStack) -> Option<Type> {
        let name = self.builtin_name(n)?;
        let aggregate = n.agg || n.window;
        // What a JSON function reads an argument as: a value that may be NULL is a null there,
        // which `unknown` already admits.
        let value = |x: &Node| {
            let f = self.json_fact(x, scope);
            let nullable = !f.non_null && !admits_null(&f.ty);
            (f.ty, nullable)
        };
        match name {
            "json_object" | "jsonb_object" if !aggregate => {
                // An odd count of arguments is an error, and so is a key that is no text.
                if n.args.len() % 2 != 0 {
                    return Some(Type::Json { schema: None });
                }
                let mut fields: Vec<Field> = Vec::new();
                let mut dynamic = false;
                for pair in n.args.chunks(2) {
                    let (ty, nullable) = value(&pair[1]);
                    match literal_text(&pair[0]) {
                        // A key written twice is in the text twice, and JSON.parse keeps the last.
                        Some(key) => match fields.iter_mut().find(|f| f.name == key) {
                            Some(f) => {
                                f.ty = ty;
                                f.nullable = nullable;
                            }
                            None => fields.push(Field { name: key.to_string(), ty, nullable }),
                        },
                        None => {
                            dynamic = true;
                            fields.push(Field { name: String::new(), ty, nullable });
                        }
                    }
                }
                // A key the statement computes: an object of any keys, each value one of these.
                if dynamic {
                    return Some(record(fields.iter().map(|f| value_schema(&f.ty, f.nullable)).collect()));
                }
                Some(Type::Object { fields })
            }
            // A JSON array of exactly these values, in this order: a tuple.
            "json_array" | "jsonb_array" if !aggregate => {
                let items: Vec<serde_json::Value> = n.args.iter().map(|x| {
                    let (ty, nullable) = value(x);
                    value_schema(&ty, nullable)
                }).collect();
                let mut schema = serde_json::json!({"type": "array", "items": false});
                if !items.is_empty() {
                    schema["minItems"] = serde_json::json!(items.len());
                    schema["prefixItems"] = serde_json::Value::Array(items);
                }
                Some(Type::Json { schema: Some(schema) })
            }
            // `[]` over no rows, never NULL; a FILTER and a window frame alike.
            "json_group_array" | "jsonb_group_array" if aggregate && n.args.len() == 1 => {
                let (ty, nullable) = value(&n.args[0]);
                Some(Type::List { of: Box::new(ty), nullable_items: nullable })
            }
            // `{}` over no rows; a key aggregated twice is in the text twice.
            "json_group_object" | "jsonb_group_object" if aggregate && n.args.len() == 2 => {
                let (ty, nullable) = value(&n.args[1]);
                Some(record(vec![value_schema(&ty, nullable)]))
            }
            "json" | "jsonb" if !aggregate && n.args.len() == 1 => self.shape(&n.args[0], scope).or_else(|| self.subquery_shape(&n.args[0], scope)),
            _ => None,
        }
    }

    /// A value as a JSON function reads it: its type inside the document, whether it is never
    /// NULL, and whether it carries the JSON subtype. A value that does not carries the type it
    /// has anywhere ([`in_json`]); one that does nests as the JSON it is, where every way it can
    /// take builds the same; a value the engine cannot follow is any JSON.
    fn json_fact(&self, n: &Node, scope: &ScopeStack) -> JsonFact {
        let plain = || {
            let f = self.type_of(n, scope);
            JsonFact { ty: in_json(&f.ty), non_null: f.non_null, carry: Carry::No }
        };
        let any = JsonFact { ty: Type::Unknown, non_null: false, carry: Carry::Maybe };
        match n.op.as_str() {
            "function" => {
                // A function the host registers may set the subtype on what it returns.
                let Some(name) = self.builtin_name(n) else { return any };
                // A window call's value dropped the subtype on 3.53.4 where the statement has a
                // second window (`json_object('w', json_group_array(x) over ())`).
                if n.over && is_json_result(name) {
                    return any;
                }
                if let Some(ty) = self.shape(n, scope) {
                    // `json()` of a scalar subquery is NULL where the subquery finds no row, and
                    // `jsonb()` too; every other constructor is never NULL.
                    let non_null = !(matches!(name, "json" | "jsonb") && n.args.first().is_some_and(|a| a.op == "select")) || self.type_of(n, scope).non_null;
                    return JsonFact { ty, non_null, carry: Carry::Yes };
                }
                let first = n.args.first().into_iter();
                let non_null = || self.type_of(n, scope).non_null;
                match name {
                    "coalesce" | "ifnull" => self.json_arms(n.args.iter(), true, scope).map(|f| JsonFact { non_null: f.non_null, ..f }).unwrap_or_else(plain),
                    "nullif" => self.json_arms(first, false, scope).map(|f| JsonFact { non_null: false, ..f }).unwrap_or_else(plain),
                    "likely" | "unlikely" | "likelihood" => self.json_arms(first, false, scope).unwrap_or_else(plain),
                    "iif" | "if" => {
                        let arms = iif_values(n.args.len()).into_iter().filter_map(|i| n.args.get(i));
                        self.json_arms(arms, false, scope).map(|f| JsonFact { non_null: non_null(), ..f }).unwrap_or_else(plain)
                    }
                    // The aggregate hands back one of its values.
                    "min" | "max" if n.args.len() == 1 && !n.window => self.json_arms(first, false, scope).map(|f| JsonFact { non_null: non_null(), ..f }).unwrap_or_else(plain),
                    _ if is_json_result(name) => JsonFact { ty: Type::Json { schema: None }, non_null: non_null(), carry: Carry::Yes },
                    // `max(a, b)` and `lag(x)` hand back one of their arguments: `max` kept the
                    // subtype on 3.53.4 and `lag` dropped it.
                    _ if PASSING.contains(&name) => {
                        if n.args.iter().all(|a| self.carry(a, scope) == Carry::No) { plain() } else { any }
                    }
                    _ if PLAIN.contains(&name) => plain(),
                    _ => any,
                }
            }
            // A scalar subquery's JSON may have lost the subtype ([`Analyzer::carry`]): wrapped in
            // `json()`, it nests as the JSON it is on every SQLite.
            "select" => if self.carry(n, scope) == Carry::No { plain() } else { any },
            "collate" | "pos" => n.left.as_deref().map_or(any, |l| self.json_fact(l, scope)),
            "case" => {
                let results = n.list.chunks(2).filter_map(|pair| if pair.len() == 2 { Some(&pair[1]) } else { None });
                let otherwise = if n.list.len() % 2 == 1 { n.list.last() } else { None };
                match self.json_arms(results.chain(otherwise), false, scope) {
                    Some(f) => JsonFact { non_null: self.type_of(n, scope).non_null, ..f },
                    None => plain(),
                }
            }
            // `cast(json_array(1) as text)` kept the subtype on 3.53.4, `as integer` gave 0.
            "cast" | "column" => if self.carry(n, scope) == Carry::No { plain() } else { any },
            _ => plain(),
        }
    }

    /// A value that is one of `arms`, NULL arms aside: `None` where none carries the subtype, so
    /// the value is the plain one its type says. `coalesce`: an arm after one that is never NULL
    /// is never reached.
    fn json_arms<'n>(&self, arms: impl Iterator<Item = &'n Node>, coalesce: bool, scope: &ScopeStack) -> Option<JsonFact> {
        let mut facts: Vec<JsonFact> = Vec::new();
        for arm in arms.filter(|a| a.op != "null") {
            let f = self.json_fact(arm, scope);
            let last = coalesce && f.non_null;
            facts.push(f);
            if last {
                break;
            }
        }
        if facts.iter().all(|f| f.carry == Carry::No) {
            return None;
        }
        let non_null = facts.iter().any(|f| f.non_null);
        if !facts.iter().all(|f| f.carry == Carry::Yes) {
            return Some(JsonFact { ty: Type::Unknown, non_null, carry: Carry::Maybe });
        }
        let types: Vec<&Type> = facts.iter().map(|f| &f.ty).collect();
        let ty = if types.iter().all(|t| *t == types[0]) { types[0].clone() } else { same_json(&types).unwrap_or(Type::Unknown) };
        Some(JsonFact { ty, non_null, carry: Carry::Yes })
    }

    /// Whether a value carries SQLite's JSON subtype ([`Carry`]).
    fn carry(&self, n: &Node, scope: &ScopeStack) -> Carry {
        match n.op.as_str() {
            "function" => {
                let Some(name) = self.builtin_name(n) else { return Carry::Maybe };
                match name {
                    "coalesce" | "ifnull" => self.carries(n.args.iter(), scope),
                    "nullif" | "likely" | "unlikely" | "likelihood" => self.carries(n.args.first().into_iter(), scope),
                    "iif" | "if" => self.carries(iif_values(n.args.len()).into_iter().filter_map(|i| n.args.get(i)), scope),
                    "min" | "max" if n.args.len() == 1 && !n.window => self.carries(n.args.iter(), scope),
                    _ if is_json_result(name) => if n.over { Carry::Maybe } else { Carry::Yes },
                    _ if PASSING.contains(&name) => {
                        if self.carries(n.args.iter(), scope) == Carry::No { Carry::No } else { Carry::Maybe }
                    }
                    _ if PLAIN.contains(&name) => Carry::No,
                    _ => Carry::Maybe,
                }
            }
            // A scalar subquery kept the subtype on 3.53.4 and drops it on 3.54.1 (the SQLite of
            // Node 26.11's `node:sqlite`): `json_object('n', (select json_object('a', 1)))` is
            // `{"n":{"a":1}}` on one and `{"n":"{\"a\":1}"}` on the other, so a JSON value from one may
            // be either.
            "select" => {
                let Some(sub) = n.select.as_deref() else { return Carry::No };
                match self.select_carry(sub, 0, scope) {
                    Carry::No => Carry::No,
                    _ => Carry::Maybe,
                }
            }
            "collate" | "pos" => n.left.as_deref().map_or(Carry::No, |l| self.carry(l, scope)),
            "cast" => match n.left.as_deref().map_or(Carry::No, |l| self.carry(l, scope)) {
                Carry::No => Carry::No,
                _ => Carry::Maybe,
            },
            "case" => {
                let results = n.list.chunks(2).filter_map(|pair| if pair.len() == 2 { Some(&pair[1]) } else { None });
                let otherwise = if n.list.len() % 2 == 1 { n.list.last() } else { None };
                self.carries(results.chain(otherwise), scope)
            }
            // A column of a subquery, a view or a WITH query dropped the subtype on 3.53.4
            // (`json_object('a', x) from (select json_array(1) as x)` is `{"a":"[1]"}`), which
            // a SQLite that flattens the subquery may keep.
            "column" => {
                for froms in scope.iter().rev() {
                    if let Some(f) = froms.iter().find(|f| f.cursor == n.cursor) {
                        if let Some(sub) = f.subquery.as_deref() {
                            return if self.select_carry(sub, n.index.max(0) as usize, scope) == Carry::No { Carry::No } else { Carry::Maybe };
                        }
                        break;
                    }
                }
                Carry::No
            }
            _ => Carry::No,
        }
    }

    /// Whether column `i` of a select carries the subtype, in every arm and row of a compound.
    fn select_carry(&self, sel: &Sel, i: usize, scope: &ScopeStack) -> Carry {
        let mut carries = Vec::new();
        let mut s = Some(sel);
        while let Some(arm) = s {
            self.within(arm, scope, |inner| {
                for e in std::iter::once(arm.columns.get(i).map(|c| &c.expr)).chain(arm.rows.iter().map(|row| row.get(i))).flatten() {
                    carries.push(self.carry(e, inner));
                }
            });
            s = arm.prior.as_deref();
        }
        combine(&carries)
    }

    /// Whether one of `arms` is the value, NULL arms aside, carries the subtype.
    fn carries<'n>(&self, arms: impl Iterator<Item = &'n Node>, scope: &ScopeStack) -> Carry {
        let carries: Vec<Carry> = arms.filter(|a| a.op != "null").map(|a| self.carry(a, scope)).collect();
        combine(&carries)
    }

    /// `f` over a select arm's own scope and level, as [`Analyzer::select_column`] reads its columns.
    fn within<T>(&self, sel: &Sel, scope: &ScopeStack, f: impl FnOnce(&ScopeStack) -> T) -> T {
        let mut inner = scope.clone();
        inner.push(&sel.from);
        let outer = self.level.replace(level_of(sel, self.extras));
        let out = f(&inner);
        self.level.replace(outer);
        out
    }

    // ---- parameters --------------------------------------------------------------------------

    /// Every parameter, typed by the context SQLite placed it in; the first typed context wins.
    fn params(&self, lexed: &Lexed) -> Vec<Param> {
        let mut found: Vec<(String, Fact)> = Vec::new();
        let t = self.tree;
        // A parameter that is, bare, with no cast, the whole value written to a column may be null
        // where that column may hold NULL, and no column that refuses NULL it also fills forbids
        // it. A cast one, `CAST(? AS type)` or the `json(?)` a `::json` compiles to, is its cast's
        // type, never the column's.
        let mut fills: HashMap<String, bool> = HashMap::new();
        let mut fill = |v: &Node, target: &Option<Fact>, checked: bool| {
            if let (true, Some(name), Some(target)) = (v.op == "variable", v.name.as_ref(), target) {
                *fills.entry(name.clone()).or_insert(true) &= !target.non_null && !checked;
            }
        };
        let checked = |c: Option<&str>| t.dml.as_ref().zip(c).is_some_and(|(dml, c)| self.checked_not_null(&dml.table, c));
        if let Some(sel) = &t.statement {
            let targets: Vec<Option<Fact>> = match &t.dml {
                // The SELECT or the VALUES rows that feed an INSERT: their columns type from the
                // target columns.
                Some(dml) if dml.kind == "insert" => dml.columns.iter().map(|c| c.as_deref().and_then(|c| self.table_column(&dml.table, c))).collect(),
                _ => Vec::new(),
            };
            // Every row of a VALUES list writes its values whole, as the one-row form below does,
            // and so does the one SELECT that feeds an INSERT; a compound's columns are any arm's.
            if sel.prior.is_none() {
                for (i, v) in sel.columns.iter().map(|c| &c.expr).enumerate().chain(sel.rows.iter().flat_map(|row| row.iter().enumerate())) {
                    if let Some(target) = targets.get(i) {
                        fill(v, target, checked(t.dml.as_ref().and_then(|d| d.columns.get(i)).and_then(|c| c.as_deref())));
                    }
                }
            }
            self.walk_select(sel, &Vec::new(), &targets, &mut found);
        }
        if let Some(dml) = &t.dml {
            let scope: ScopeStack = Vec::new();
            if let Some(values) = &dml.values {
                for (i, v) in values.iter().enumerate() {
                    let target = dml.columns.get(i).and_then(|c| c.as_deref()).and_then(|c| self.table_column(&dml.table, c));
                    fill(v, &target, checked(dml.columns.get(i).and_then(|c| c.as_deref())));
                    self.walk(v, Ctx::Typed(target), &scope, &mut found);
                }
            }
            for item in dml.set.iter().chain(dml.upsert.iter().flat_map(|u| u.set.iter())) {
                let target = item.column.as_deref().and_then(|c| self.table_column(&dml.table, c));
                fill(&item.expr, &target, checked(item.column.as_deref()));
                self.walk(&item.expr, Ctx::Typed(target), &scope, &mut found);
            }
            if let Some(w) = &dml.where_ {
                self.walk(w, Ctx::Bool, &scope, &mut found);
            }
            if let Some(w) = dml.upsert.as_ref().and_then(|u| u.where_.as_ref()) {
                self.walk(w, Ctx::Bool, &scope, &mut found);
            }
        }
        if let Some(returning) = &t.returning {
            for c in returning {
                self.walk(&c.expr, Ctx::None, &Vec::new(), &mut found);
            }
        }
        // The tree was prepared over the `?n` rewrite (see `analyze`): SQLite's `?k` is the lexer's
        // k-th parameter, whatever the author wrote.
        let mut typed: HashMap<&str, &Fact> = HashMap::new();
        for (name, fact) in found.iter().filter(|(_, f)| !matches!(f.ty, Type::Unknown)) {
            typed.entry(name.as_str()).or_insert(fact);
        }
        lexed
            .params
            .iter()
            .enumerate()
            .map(|(k, name)| {
                let numbered = format!("?{}", k + 1);
                let fact = typed.get(numbered.as_str()).map(|f| (*f).clone()).unwrap_or_else(Fact::unknown);
                // A parameter compared with built JSON is bound as the text it is compared with.
                let fact = Fact { ty: textual(&fact.ty), ..fact };
                let type_name = scalar_name(&fact.ty).unwrap_or("").to_string();
                let nullable = fills.get(&numbered).copied().unwrap_or(false) || self.nullable_params.contains(&numbered);
                Param { name: name.clone(), ty: fact.ty, type_name, nullable, schema: None, cast: None, comment: None }
            })
            .collect()
    }

    /// Whether a CHECK of the table refuses NULL in the column (`check (col is not null)`). A view
    /// is no table here (its INSTEAD OF trigger says what a write takes), so a parameter written
    /// to one has no target column and takes no NULL.
    fn checked_not_null(&self, table: &str, column: &str) -> bool {
        self.tree.tables.get(table).and_then(|info| info.columns.iter().find(|c| c.name.eq_ignore_ascii_case(column))).is_some_and(|c| c.checked_not_null)
    }

    fn table_column(&self, table: &str, column: &str) -> Option<Fact> {
        let info = self.tree.tables.get(table)?;
        let col = info.columns.iter().find(|c| c.name.eq_ignore_ascii_case(column))?;
        Some(Fact { ty: column_type(&col.declared, &col.affinity, info.strict), non_null: col.not_null || info.rowid_alias.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(column)) })
    }

    fn walk_select(&self, sel: &Sel, scope: &ScopeStack, targets: &[Option<Fact>], found: &mut Vec<(String, Fact)>) {
        // In a compound every arm's column i is one result column: a parameter in one arm takes
        // the type the other arms give it (`SELECT name FROM users UNION ALL SELECT :x`).
        let compound: Vec<Option<Fact>>;
        let targets = if targets.is_empty() && (sel.prior.is_some() || !sel.rows.is_empty()) {
            compound = (0..sel.columns.len())
                .map(|i| {
                    let ty = self.select_column(sel, i, scope).ty;
                    (!matches!(ty, Type::Unknown)).then_some(Fact { ty, non_null: true })
                })
                .collect();
            compound.as_slice()
        } else {
            targets
        };
        let mut inner = scope.clone();
        inner.push(&sel.from);
        for (i, e) in sel.columns.iter().map(|c| &c.expr).enumerate().chain(sel.rows.iter().flat_map(|row| row.iter().enumerate())) {
            self.walk(e, Ctx::Typed(targets.get(i).cloned().flatten()), &inner, found);
        }
        for f in &sel.from {
            if let Some(sub) = &f.subquery {
                self.walk_select(sub, &inner, &[], found);
            }
            if let Some(on) = &f.on {
                self.walk(on, Ctx::Bool, &inner, found);
            }
            // `json_each` and `json_tree` read JSON text and a path (`IN (SELECT value FROM json_each(:ids))`).
            let json = f.tab_func && matches!(f.table.as_deref(), Some("json_each" | "json_tree"));
            for (i, a) in f.args.iter().enumerate() {
                self.walk(a, if json && i < 2 { Ctx::Typed(Some(Fact::new("text", true))) } else { Ctx::None }, &inner, found);
            }
        }
        if let Some(w) = &sel.where_ {
            self.walk(w, Ctx::Bool, &inner, found);
        }
        for g in &sel.group_by {
            self.walk(g, Ctx::None, &inner, found);
        }
        if let Some(h) = &sel.having {
            self.walk(h, Ctx::Bool, &inner, found);
        }
        for o in &sel.order_by {
            self.walk(o, Ctx::None, &inner, found);
        }
        for l in sel.limit.iter().chain(sel.offset.iter()) {
            self.walk(l, Ctx::Typed(Some(Fact::new("integer", true))), &inner, found);
        }
        if let Some(prior) = &sel.prior {
            self.walk_select(prior, scope, targets, found);
        }
    }

    /// Visits every node; a variable takes the type its context gives it.
    fn walk(&self, n: &Node, ctx: Ctx, scope: &ScopeStack, found: &mut Vec<(String, Fact)>) {
        if n.op == "variable" {
            let fact = match ctx {
                Ctx::Typed(Some(f)) => f,
                Ctx::Typed(None) | Ctx::Row(_) | Ctx::None => Fact::unknown(),
                Ctx::Bool => Fact::new("integer", false),
            };
            found.push((n.name.clone().unwrap_or_default(), fact));
            return;
        }
        let sibling = |other: &Option<Box<Node>>| other.as_deref().map(|o| self.type_of(o, scope));
        let child_ctx = |slot: Slot| -> Ctx {
            match n.op.as_str() {
                "=" | "!=" | "<" | "<=" | ">" | ">=" | "is" | "is not" => {
                    let (this, other) = match slot {
                        Slot::Left => (&n.left, &n.right),
                        Slot::Right => (&n.right, &n.left),
                        _ => return Ctx::None,
                    };
                    // Row values compare element by element, so each element takes the type of
                    // the one it is compared with: `(created_at, id) < (:created_at, :id)`, and a
                    // subquery's columns: `(created_at, id) = (SELECT :at, :id)`.
                    match (this.as_deref(), other.as_deref().and_then(|o| self.row(o, scope))) {
                        (Some(t), Some(row)) if matches!(t.op.as_str(), "vector" | "select") => Ctx::Row(row),
                        _ => Ctx::Typed(sibling(other)),
                    }
                }
                "in" => match slot {
                    // `(created_at, id) IN (SELECT …)`: SQLite writes a list of row values as
                    // a VALUES subquery, which the left side types (see the select walk below).
                    Slot::Left if is_vector(&n.left) => n.select.as_deref().map_or(Ctx::None, |s| Ctx::Row(self.select_columns(s, scope))),
                    Slot::Left => Ctx::Typed(n.list.first().map(|x| self.type_of(x, scope)).or_else(|| n.select.as_deref().map(|s| self.select_column(s, 0, scope)))),
                    Slot::List(_) => Ctx::Typed(sibling(&n.left)),
                    _ => Ctx::None,
                },
                "between" => match slot {
                    Slot::Left if is_vector(&n.left) => Ctx::Row(self.rows(&n.list.iter().collect::<Vec<_>>(), scope)),
                    Slot::Left => Ctx::Typed(n.list.first().map(|x| self.type_of(x, scope))),
                    Slot::List(_) => match n.left.as_deref().and_then(|l| self.row(l, scope)) {
                        Some(row) if is_vector(&n.left) => Ctx::Row(row),
                        _ => Ctx::Typed(sibling(&n.left)),
                    },
                    _ => Ctx::None,
                },
                // An element of a row value takes its place's type.
                "vector" => match (slot, &ctx) {
                    (Slot::List(i), Ctx::Row(row)) => Ctx::Typed(row.get(i).cloned().flatten()),
                    _ => Ctx::None,
                },
                // One column of a row subquery, as `UPDATE … SET (a, b) = (SELECT …)` writes it.
                "select_column" => match (slot, &ctx) {
                    (Slot::Left, Ctx::Typed(Some(f))) => {
                        let mut row = vec![None; n.index.max(0) as usize + 1];
                        row[n.index.max(0) as usize] = Some(f.clone());
                        Ctx::Row(row)
                    }
                    _ => Ctx::None,
                },
                "cast" => Ctx::Typed(Some(Fact::new(n.affinity.as_deref().unwrap_or("none"), true))),
                "+" | "-" | "*" | "/" | "%" => {
                    let other = match slot {
                        Slot::Left => sibling(&n.right),
                        _ => sibling(&n.left),
                    };
                    Ctx::Typed(Some(match other {
                        Some(f) if is_numeric(&f.ty) => f,
                        _ => Fact::new("real", true),
                    }))
                }
                "||" => Ctx::Typed(Some(Fact::new("text", true))),
                "neg" | "~" | "&" | "|" | "<<" | ">>" => Ctx::Typed(Some(Fact::new("real", true))),
                // Unary plus hands its operand the context it stands in: `x IN (?)` is `x = +?`.
                // So does a COLLATE, which names how the value compares and leaves it as it is.
                "pos" | "collate" => ctx.clone(),
                "and" | "or" | "not" => Ctx::Bool,
                "case" => match slot {
                    // A WHEN compares with the base when there is one, else it is a condition; a
                    // THEN or ELSE shares the other results' type, or failing that the type the
                    // CASE stands in (`name = CASE WHEN … THEN :a ELSE :b END`).
                    Slot::List(i) if i % 2 == 0 && i + 1 < n.list.len() => match &n.left {
                        Some(base) if base.op == "vector" => self.row(base, scope).map_or(Ctx::None, Ctx::Row),
                        Some(base) => Ctx::Typed(Some(self.type_of(base, scope))),
                        None => Ctx::Bool,
                    },
                    Slot::List(_) => {
                        let others: Vec<Fact> = n.list.iter().enumerate().filter(|(j, _)| j % 2 == 1 || (*j == n.list.len() - 1 && n.list.len() % 2 == 1)).map(|(_, x)| self.type_of(x, scope)).collect();
                        let refs: Vec<&Fact> = others.iter().collect();
                        let ty = shared(&refs);
                        match ty {
                            Type::Unknown => outer_type(&ctx),
                            ty => Ctx::Typed(Some(Fact { ty, non_null: true })),
                        }
                    }
                    // A base compares with each WHEN value (`CASE :kind WHEN 'a' THEN …`), a row
                    // value element by element.
                    Slot::Left if is_vector(&n.left) => {
                        let values: Vec<&Node> = n.list.iter().enumerate().filter(|(j, _)| j % 2 == 0 && j + 1 < n.list.len()).map(|(_, x)| x).collect();
                        Ctx::Row(self.rows(&values, scope))
                    }
                    // A base compares with each WHEN value (`CASE :kind WHEN 'a' THEN …`).
                    Slot::Left => {
                        let values: Vec<Fact> = n.list.iter().enumerate().filter(|(j, _)| j % 2 == 0 && j + 1 < n.list.len()).map(|(_, x)| self.type_of(x, scope)).collect();
                        let refs: Vec<&Fact> = values.iter().collect();
                        match shared(&refs) {
                            Type::Unknown => Ctx::None,
                            ty => Ctx::Typed(Some(Fact { ty, non_null: true })),
                        }
                    }
                    _ => Ctx::None,
                },
                // A function whose result is one of its arguments gives an argument its siblings'
                // type, or failing that the type the call stands in (`name = coalesce(:a, :b)`).
                "function" => match (slot, self.argument(n, i_of(slot), scope)) {
                    (Slot::Arg(_), Ctx::Typed(None) | Ctx::None) if matches!(n.name.as_deref(), Some("coalesce" | "ifnull" | "nullif" | "min" | "max")) && !n.agg => outer_type(&ctx),
                    (Slot::Arg(i), Ctx::Typed(None) | Ctx::None) if matches!(n.name.as_deref(), Some("iif" | "if")) && iif_values(n.args.len()).contains(&i) => outer_type(&ctx),
                    (Slot::Arg(_), c) => c,
                    _ => Ctx::None,
                },
                _ => Ctx::None,
            }
        };
        if let Some(l) = &n.left {
            self.walk(l, child_ctx(Slot::Left), scope, found);
        }
        if let Some(r) = &n.right {
            self.walk(r, child_ctx(Slot::Right), scope, found);
        }
        for (i, x) in n.list.iter().enumerate() {
            self.walk(x, child_ctx(Slot::List(i)), scope, found);
        }
        for (i, x) in n.args.iter().enumerate() {
            self.walk(x, child_ctx(Slot::Arg(i)), scope, found);
        }
        if let Some(s) = &n.select {
            // A subquery's columns take the types they are compared with or stand in: `id IN
            // (SELECT :p)`, `id = (SELECT :p)`, `(created_at, id) IN (VALUES (:at, :id))`.
            let targets: Vec<Option<Fact>> = match (n.op.as_str(), &ctx) {
                ("select", Ctx::Row(row)) => row.clone(),
                ("select", Ctx::Typed(Some(f))) => vec![Some(f.clone())],
                ("in", _) => match n.left.as_deref() {
                    Some(l) if l.op == "vector" => self.row(l, scope).unwrap_or_default(),
                    Some(l) => vec![Some(self.type_of(l, scope))],
                    None => Vec::new(),
                },
                _ => Vec::new(),
            };
            self.walk_select(s, scope, &targets, found);
        }
    }

    /// A row value's elements, one fact per place: a vector's elements, or a subquery's columns.
    fn row(&self, n: &Node, scope: &ScopeStack) -> Option<Vec<Option<Fact>>> {
        match n.op.as_str() {
            "vector" => Some(n.list.iter().map(|x| Some(self.type_of(x, scope))).collect()),
            "select" => n.select.as_deref().map(|s| self.select_columns(s, scope)),
            _ => None,
        }
    }

    fn select_columns(&self, sel: &Sel, scope: &ScopeStack) -> Vec<Option<Fact>> {
        (0..sel.columns.len()).map(|i| Some(self.select_column(sel, i, scope))).collect()
    }

    /// The type at each place of several row values (BETWEEN's bounds, a CASE's WHEN values):
    /// the first that has one there.
    fn rows(&self, values: &[&Node], scope: &ScopeStack) -> Vec<Option<Fact>> {
        let mut out: Vec<Option<Fact>> = Vec::new();
        for row in values.iter().filter_map(|v| self.row(v, scope)) {
            for (i, f) in row.into_iter().enumerate() {
                if i >= out.len() {
                    out.push(None);
                }
                if out[i].as_ref().is_none_or(|known| matches!(known.ty, Type::Unknown)) {
                    out[i] = f;
                }
            }
        }
        out
    }

    /// The type a built-in function gives its argument `i`, for a parameter written there.
    fn argument(&self, n: &Node, i: usize, scope: &ScopeStack) -> Ctx {
        let name = n.name.as_deref().unwrap_or("");
        if !n.agg && !n.window && self.extras.function(name, n.args.len()).is_some() {
            return Ctx::None;
        }
        let text = || Ctx::Typed(Some(Fact::new("text", true)));
        let integer = || Ctx::Typed(Some(Fact::new("integer", true)));
        let real = || Ctx::Typed(Some(Fact::new("real", true)));
        let others = || {
            let facts: Vec<Fact> = n.args.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, a)| self.type_of(a, scope)).collect();
            let refs: Vec<&Fact> = facts.iter().collect();
            let ty = shared(&refs);
            Ctx::Typed(if matches!(ty, Type::Unknown) { None } else { Some(Fact { ty, non_null: true }) })
        };
        match name {
            "like" | "glob" | "regexp" | "match" | "upper" | "lower" | "trim" | "ltrim" | "rtrim" | "length" | "hex" | "unicode" | "quote" | "instr" | "replace" | "soundex"
            | "printf" | "format" | "concat" | "concat_ws" | "date" | "time" | "datetime" | "strftime" | "julianday" | "unixepoch" | "timediff" | "json"
            | "json_extract" | "json_type" | "json_valid" | "json_array_length" | "json_quote" | "json_pretty" | "json_insert" | "json_replace" | "json_set"
            | "json_remove" | "json_patch" | "->" | "->>" => text(),
            "substr" | "substring" => {
                if i == 0 { text() } else { integer() }
            }
            "zeroblob" | "randomblob" | "char" => integer(),
            "abs" | "round" | "acos" | "acosh" | "asin" | "asinh" | "atan" | "atan2" | "atanh" | "ceil" | "ceiling" | "cos" | "cosh" | "degrees" | "exp"
            | "floor" | "ln" | "log" | "log10" | "log2" | "mod" | "pow" | "power" | "radians" | "sin" | "sinh" | "sqrt" | "tan" | "tanh" | "trunc" => real(),
            "coalesce" | "ifnull" | "nullif" | "min" | "max" => others(),
            // `lag` and `lead` hand back their default where the row they read is missing, so the
            // value and the default each take the other's type.
            "lag" | "lead" if n.args.len() >= 3 && (i == 0 || i == 2) => {
                let other = self.type_of(&n.args[2 - i], scope);
                Ctx::Typed(if matches!(other.ty, Type::Unknown) { None } else { Some(Fact { ty: other.ty, non_null: true }) })
            }
            // A condition is a truth value; a value shares the other values' type.
            "iif" | "if" => {
                let values = iif_values(n.args.len());
                if !values.contains(&i) {
                    Ctx::Bool
                } else {
                    let facts: Vec<Fact> = values.iter().filter(|j| **j != i).filter_map(|j| n.args.get(*j)).map(|a| self.type_of(a, scope)).collect();
                    let ty = shared(&facts.iter().collect::<Vec<_>>());
                    Ctx::Typed(if matches!(ty, Type::Unknown) { None } else { Some(Fact { ty, non_null: true }) })
                }
            }
            "sum" | "avg" | "total" => real(),
            // After the FTS table: `highlight(t, column, open, close)` and `bm25(t, weight, …)`.
            // `snippet` takes its arguments in another order on FTS4 than on FTS5.
            "highlight" if i == 1 => integer(),
            "highlight" if i > 1 => text(),
            "bm25" if i > 0 => real(),
            _ => Ctx::None,
        }
    }

    // ---- the row count -----------------------------------------------------------------------

    fn cardinality(&self) -> (String, Option<String>) {
        let t = self.tree;
        if let Some(dml) = &t.dml {
            if t.returning.is_none() {
                return ("none".into(), None);
            }
            return match dml.kind.as_str() {
                "insert" => {
                    if dml.rows == 1 {
                        let triggered = |events: i32| self.skipping.get(&dml.table.to_ascii_lowercase()).is_some_and(|mask| mask & events != 0);
                        let can_skip = dml.on_error == "ignore"
                            || dml.upsert.as_ref().is_some_and(|u| !u.do_update || u.where_.is_some() || triggered(TRIGGER_ROW_UPDATE))
                            || triggered(TRIGGER_ROW_INSERT);
                        if can_skip { ("maybeOne".into(), Some("ON CONFLICT DO NOTHING, OR IGNORE, a DO UPDATE with a WHERE, or a BEFORE trigger's RAISE(IGNORE) can insert no row".into())) } else { ("one".into(), None) }
                    } else if dml.rows > 1 {
                        ("many".into(), Some(format!("a VALUES list of {} rows returns {} rows", dml.rows, dml.rows)))
                    } else {
                        ("many".into(), Some("an INSERT ... SELECT returns a row per row it reads".into()))
                    }
                }
                _ => {
                    // UPDATE or DELETE: the WHERE pins a unique key of the target, or any number.
                    match self.pinned_by_name(&dml.table, dml.where_.as_ref()) {
                        Ok(()) => ("maybeOne".into(), None),
                        Err(why) => ("many".into(), Some(why)),
                    }
                }
            };
        }
        let Some(sel) = &t.statement else { return ("none".into(), None) };
        match self.exactly_one(sel) {
            Ok(()) => ("one".into(), None),
            Err(one_why) => match self.at_most_one(sel) {
                Ok(()) => ("maybeOne".into(), Some(one_why)),
                Err(why) => ("many".into(), Some(why)),
            },
        }
    }

    fn exactly_one(&self, sel: &Sel) -> Result<(), String> {
        // A constant LIMIT of a row or more keeps the one row, and so does an OFFSET of 0; a
        // parameter may be 0.
        let keeps = |n: &Node, least: i64| integer(n).is_some_and(|n| n >= least);
        if sel.limit.as_ref().is_some_and(|l| !keeps(l, 1)) || sel.offset.as_ref().is_some_and(|o| integer(o) != Some(0)) {
            return Err("a LIMIT or an OFFSET can remove the row (LIMIT 0, OFFSET 1)".into());
        }
        self.one_before_limit(sel)
    }

    /// Exactly one row before the statement's LIMIT and OFFSET apply.
    fn one_before_limit(&self, sel: &Sel) -> Result<(), String> {
        if sel.prior.is_some() {
            return Err("a set operation (UNION, INTERSECT, EXCEPT) returns any number of rows".into());
        }
        if sel.values && !sel.rows.is_empty() {
            return Err("a VALUES list returns a row per row it lists".into());
        }
        if sel.values {
            return Ok(());
        }
        if !sel.group_by.is_empty() {
            return Err("GROUP BY returns one row per group, and none when nothing matches".into());
        }
        if sel.having.is_some() {
            return Err("HAVING can remove the row".into());
        }
        // SQLite marks the SELECT an aggregate belongs to: one whose arguments read only columns
        // of a query around this one (`count(a.id)` in a subquery over `b`) is that query's, and
        // leaves this one a row per row of its FROM.
        if sel.aggregate && sel.columns.iter().any(|c| has_aggregate(&c.expr)) {
            return Ok(());
        }
        if sel.from.is_empty() && sel.where_.is_none() {
            return Ok(());
        }
        // A FROM of exactly one row, as `cardinality` proves it: never empty, and one row at most.
        if !sel.from.is_empty() && self.never_empty(sel) && self.arm_at_most_one(sel).is_ok() {
            return Ok(());
        }
        Err("a FROM returns a row per row it matches, and only an aggregate with no GROUP BY collapses them to one".into())
    }

    /// Whether the FROM returns a row whatever the tables hold: every item is a subquery or a WITH
    /// query of exactly one row, or sits on the nullable side of a LEFT JOIN, which keeps the row
    /// of its other side; and no WHERE term can fail but one from an outer join's ON, which
    /// removes no row. An inner join's ON is a WHERE term here, so `ON 1 = 1` is one that holds.
    /// A VALUES in a FROM is any number of rows: SQLite reads a list of several rows as one such
    /// item, which does not say how many it lists.
    fn never_empty(&self, sel: &Sel) -> bool {
        let holds = |t: &Node| match (t.op.as_str(), t.left.as_deref().and_then(integer), t.right.as_deref().and_then(integer)) {
            ("=", Some(l), Some(r)) => l == r,
            _ => integer(t).is_some_and(|n| n != 0),
        };
        sel.from.iter().all(|f| !f.right && (f.outer || f.subquery.as_ref().is_some_and(|sub| !sub.values && self.exactly_one(sub).is_ok())))
            && conjuncts(sel.where_.as_ref()).iter().all(|t| t.outer_on || holds(t))
    }

    fn at_most_one(&self, sel: &Sel) -> Result<(), String> {
        if sel.limit.as_ref().and_then(integer).is_some_and(|n| (0..=1).contains(&n)) {
            return Ok(());
        }
        // Any other LIMIT, and an OFFSET, only remove rows: the bound is the one the statement has
        // without them.
        if !sel.values && self.one_before_limit(sel).is_ok() {
            return Ok(());
        }
        // An INTERSECT returns only rows both sides return, and an EXCEPT only rows of its left
        // side (`prior`, the arms before this one): at most one row when a side it draws every
        // row from is.
        if let Some(prior) = &sel.prior {
            let drawn = match sel.op.as_deref() {
                Some("intersect") => self.at_most_one(prior).is_ok() || self.arm_at_most_one(sel).is_ok(),
                Some("except") => self.at_most_one(prior).is_ok(),
                _ => false,
            };
            return if drawn { Ok(()) } else { Err("a set operation (UNION, INTERSECT, EXCEPT) returns any number of rows".into()) };
        }
        self.arm_at_most_one(sel)
    }

    /// The proof for one SELECT arm, its set operation aside: the closure of [`Self::determined`],
    /// as `cardinality` proves it over libpg_query's tree. Every FROM item determined is one row
    /// at most; one group at most when every GROUP BY expression is pinned; one row at most from
    /// a DISTINCT over pinned values.
    fn arm_at_most_one(&self, sel: &Sel) -> Result<(), String> {
        if sel.values {
            return Err("a VALUES list returns a row per row it lists".into());
        }
        if !sel.group_by.is_empty() {
            if sel.from.iter().all(|f| !f.right) && self.determined(sel, false).iter().all(|d| *d) {
                return Ok(());
            }
            let constant = self.determined(sel, true);
            if sel.group_by.iter().all(|g| self.pinned(sel, g, &constant)) {
                return Ok(());
            }
            return Err("GROUP BY returns one row per group; the engine proves one group only when every GROUP BY expression is pinned to one value: a column of a relation whose unique key the WHERE pins, or a column the WHERE equates to a value".into());
        }
        if sel.having.is_some() {
            return Err("HAVING leaves a row per group".into());
        }
        if sel.from.is_empty() {
            return Err("a SELECT with no FROM and a WHERE can return no row or one, and the engine proves that only through a LIMIT".into());
        }
        // A RIGHT or FULL JOIN keeps the rows of each side that match nothing: two rows where
        // each side has one.
        if sel.from.iter().any(|f| f.right) {
            return Err("a RIGHT or FULL JOIN returns a row for each side that matches nothing".into());
        }
        let determined = self.determined(sel, false);
        if determined.iter().all(|d| *d) {
            return Ok(());
        }
        if sel.distinct {
            let constant = self.determined(sel, true);
            if !sel.columns.is_empty() && sel.columns.iter().all(|c| self.pinned(sel, &c.expr, &constant)) {
                return Ok(());
            }
        }
        let i = determined.iter().position(|d| !d).unwrap_or(0);
        let f = &sel.from[i];
        let Some(table) = f.table.as_deref().filter(|_| f.subquery.is_none() && !f.tab_func) else {
            return Err(format!("`{}` in the FROM is a subquery or a table-valued function that returns any number of rows", f.alias.as_deref().or(f.table.as_deref()).unwrap_or("?")));
        };
        let keys = self.tree.tables.get(table).map(|t| t.unique_keys.clone()).unwrap_or_default();
        if keys.is_empty() {
            return Err(format!("`{table}` has no unique index the engine can read, so nothing bounds the rows a WHERE matches"));
        }
        let shown: Vec<String> = keys.iter().map(|k| format!("({})", k.join(", "))).collect();
        if sel.from.len() == 1 {
            Err(format!("the WHERE does not equate every column of a unique key of `{table}` — it has {}", shown.join(", ")))
        } else {
            let name = f.alias.as_deref().unwrap_or(table);
            Err(format!("a join returns a row per pair it matches, and the WHERE and ON clauses do not equate every column of a unique key of `{name}` to values or to columns of relations pinned to one row — it has {}", shown.join(", ")))
        }
    }

    /// Which FROM items are determined: every row carries the same row of each, or its null
    /// extension (see `cardinality`'s `From`). A table is, when a unique key's every column is
    /// equated to a value or to a column of a determined item; a subquery or a WITH query, when the
    /// proof bounds it. SQLite moves every ON term into the WHERE: a term from an outer join's ON
    /// pins only that join's right side, and with `constant`, which asks that every row carry the
    /// same row of an item, never: some rows may meet the null extension instead.
    fn determined(&self, sel: &Sel, constant: bool) -> Vec<bool> {
        let n = sel.from.len();
        let index: HashMap<i64, usize> = sel.from.iter().enumerate().map(|(i, f)| (f.cursor, i)).collect();
        let terms = conjuncts(sel.where_.as_ref());
        let mut done = vec![false; n];
        loop {
            let mut changed = false;
            for i in 0..n {
                if done[i] {
                    continue;
                }
                let f = &sel.from[i];
                let pinned = if let Some(sub) = &f.subquery {
                    !(constant && f.outer) && self.at_most_one(sub).is_ok()
                } else if let (Some(table), false, false) = (f.table.as_deref(), f.tab_func, f.virtual_) {
                    // A WHERE term's `=` rejects the null extension, so it pins even an outer
                    // join's right side for every row.
                    let usable: Vec<&Node> = terms.iter().copied().filter(|t| !t.outer_on || (!constant && t.join == f.cursor)).collect();
                    self.key_pinned(table, f.cursor, &usable, &done, &index)
                } else {
                    false
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

    /// Whether some unique key of `table` (read through `cursor`) has every column equated by
    /// one of `terms` to a value: one that reads no column of this FROM's undetermined items (a
    /// column of a determined item is one value, and so is an enclosing query's), calls nothing
    /// that changes from row to row, and carries no COLLATE.
    fn key_pinned(&self, table: &str, cursor: i64, terms: &[&Node], done: &[bool], index: &HashMap<i64, usize>) -> bool {
        let Some(info) = self.tree.tables.get(table) else { return false };
        let pins: Vec<&str> = terms.iter().filter_map(|t| self.pins(t, cursor, done, index)).collect();
        info.unique_keys.iter().any(|key| !key.is_empty() && key.iter().all(|c| pins.iter().any(|p| p.eq_ignore_ascii_case(c))))
    }

    /// The column of `cursor` a term equates to a value, if it does, under a comparison that
    /// leaves the column as it is ([`keeps_column`]) and a collation its unique index holds to:
    /// the left operand's column's, else the right's (`sqlite3BinaryCompareCollSeq`), which must
    /// be BINARY or the key's own.
    fn pins<'n>(&self, t: &'n Node, cursor: i64, done: &[bool], index: &HashMap<i64, usize>) -> Option<&'n str> {
        if t.op != "=" {
            return None;
        }
        let (l, r) = (t.left.as_deref()?, t.right.as_deref()?);
        let collation = column_of(l).or_else(|| column_of(r)).and_then(|c| c.coll.as_deref()).unwrap_or("BINARY");
        [(l, r), (r, l)].into_iter().find_map(|(col, other)| {
            let value = col.op == "column"
                && col.cursor == cursor
                && !has_op(other, "collate")
                && keeps_column(col, other)
                && (collation.eq_ignore_ascii_case("BINARY") || col.coll.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(collation)))
                && !self.calls_volatile(other)
                && other.select.is_none()
                && cursors(other).is_some_and(|read| read.iter().all(|c| *c != cursor && index.get(c).is_none_or(|i| done[*i])));
            value.then_some(col.column.as_deref()).flatten()
        })
    }

    /// Whether every row carries the same value of `e`: a column of a constant item, a column the
    /// WHERE (not an outer join's ON) equates to a value, or a value itself.
    fn pinned(&self, sel: &Sel, e: &Node, constant: &[bool]) -> bool {
        let index: HashMap<i64, usize> = sel.from.iter().enumerate().map(|(i, f)| (f.cursor, i)).collect();
        if e.op == "column" {
            if index.get(&e.cursor).is_some_and(|i| constant[*i]) {
                return true;
            }
            let terms = conjuncts(sel.where_.as_ref());
            return terms.iter().filter(|t| !t.outer_on).filter_map(|t| self.pins(t, e.cursor, constant, &index)).any(|c| e.column.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(c)));
        }
        e.select.is_none() && !self.calls_volatile(e) && cursors(e).is_some_and(|read| read.iter().all(|c| !index.contains_key(c)))
    }

    /// The DML form: the target table's columns have no stable cursor, so they match by name.
    fn pinned_by_name(&self, table: &str, where_: Option<&Node>) -> Result<(), String> {
        let equated = self.equated(where_);
        let Some(info) = self.tree.tables.get(table) else { return Err(format!("`{table}` is not a table the engine can read")) };
        let names: HashSet<String> = equated.iter().map(|(_, n)| n.to_ascii_lowercase()).collect();
        if info.unique_keys.iter().any(|key| !key.is_empty() && key.iter().all(|c| names.contains(&c.to_ascii_lowercase()))) {
            return Ok(());
        }
        if info.unique_keys.is_empty() {
            return Err(format!("`{table}` has no unique index the engine can read, so nothing bounds the rows a WHERE matches"));
        }
        let shown: Vec<String> = info.unique_keys.iter().map(|k| format!("({})", k.join(", "))).collect();
        Err(format!("the WHERE does not equate every column of a unique key of `{table}` — it has {}", shown.join(", ")))
    }

    /// The columns the WHERE equates to a value: `col = <no column anywhere>` under ANDs, where
    /// the value calls nothing that changes from row to row.
    fn equated(&self, where_: Option<&Node>) -> HashSet<(i64, String)> {
        let mut out = HashSet::new();
        for n in conjuncts(where_) {
            if n.op != "=" || n.outer_on {
                continue;
            }
            if let (Some(l), Some(r)) = (n.left.as_deref(), n.right.as_deref()) {
                for (col, other) in [(l, r), (r, l)] {
                    // An explicit COLLATE (NOCASE) may compare more loosely than the
                    // unique index distinguishes.
                    if col.op == "column" && !reads_column(other) && !has_op(other, "collate") && keeps_column(col, other) && !self.calls_volatile(other) {
                        if let Some(name) = &col.column {
                            out.insert((col.cursor, name.clone()));
                        }
                    }
                }
            }
        }
        out
    }

    /// Whether `n` calls a function that may give each row another value: `random()`,
    /// `changes()`, a date or time function (`'now'` moves), or one the host registers or the
    /// engine does not know, whose determinism it cannot tell.
    fn calls_volatile(&self, n: &Node) -> bool {
        const VOLATILE: &[&str] = &["random", "randomblob", "changes", "total_changes", "last_insert_rowid", "date", "time", "datetime", "julianday", "unixepoch", "strftime", "timediff"];
        if n.op == "function" {
            let name = n.name.as_deref().unwrap_or("").to_ascii_lowercase();
            if VOLATILE.contains(&name.as_str()) || self.extras.function(&name, n.args.len()).is_some() || !KNOWN_FUNCTIONS.contains(&name.as_str()) {
                return true;
            }
        }
        n.left.as_deref().is_some_and(|x| self.calls_volatile(x)) || n.right.as_deref().is_some_and(|x| self.calls_volatile(x)) || n.list.iter().any(|x| self.calls_volatile(x)) || n.args.iter().any(|x| self.calls_volatile(x))
    }
}

/// Whether comparing key column `col` with `other` leaves the column's values as they are. SQLite
/// applies NUMERIC affinity to both sides where either side's affinity is INTEGER, REAL or NUMERIC,
/// and one side's affinity to both where the other has none, so a TEXT key compared with a number
/// reads '01' and '1' as 1, and a key of no affinity compared with a TEXT side reads 7 as '7'. A
/// numeric key converts nothing it holds: what stays text there is no number.
fn keeps_column(col: &Node, other: &Node) -> bool {
    let numeric = |a: &str| matches!(a, "integer" | "real" | "numeric");
    let none = |a: &str| matches!(a, "blob" | "none");
    numeric(&col.aff) || none(&other.aff) || (col.aff == "text" && other.aff == "text")
}

/// The column an operand reads through casts and unary plus, as SQLite looks for the collation.
fn column_of(n: &Node) -> Option<&Node> {
    match n.op.as_str() {
        "column" => Some(n),
        "cast" | "pos" => n.left.as_deref().and_then(column_of),
        _ => None,
    }
}

/// A string literal's text.
fn literal_text(n: &Node) -> Option<&str> {
    if n.op == "string" { n.value.as_deref() } else { None }
}

/// The arguments of `iif(c1, v1, c2, v2, …, else)` that are its values: each one after a
/// condition, and the last of an odd count, the ELSE.
fn iif_values(count: usize) -> Vec<usize> {
    let mut out: Vec<usize> = (1..count).step_by(2).collect();
    if count >= 3 && count % 2 == 1 {
        out.push(count - 1);
    }
    out
}

/// When `total` is NULL: never over integers, but over reals a `1e999` and a `-1e999` add up to
/// NaN, which SQLite gives as NULL.
fn total_nulls(arg: &Fact) -> Nulls {
    if is_integral(&arg.ty) { Nulls::Never } else { Nulls::Always }
}

/// Whether a date and time function always reads its time value and modifiers. SQLite gives NULL
/// for a time value it cannot read (`date('not a date')`), one outside the julian days 0 to
/// 5373484 it reads (`date(-1)`, `datetime(1e18, 'unixepoch')`), and a modifier that fails
/// (`'weekday 7'`, `'unixepoch'` after a text value) or moves the time out of that range. So this
/// holds only for no time value (the current time), the current time (`'now'` in any case, or
/// `current_timestamp`, `current_date` or `current_time`) moved less than a thousand years, or a
/// number literal (a julian day, or a unix time read by a first `'unixepoch'`) whose modifiers
/// keep it in range; and only for literal modifiers that cannot fail on it.
fn current_time(args: &[Node]) -> bool {
    let Some((value, modifiers)) = args.split_first() else { return true };
    let now = literal_text(value).is_some_and(|v| v.eq_ignore_ascii_case("now"))
        || (value.op == "function" && value.args.is_empty() && matches!(value.name.as_deref(), Some("current_timestamp" | "current_date" | "current_time")));
    let number = if matches!(value.op.as_str(), "integer" | "float") { value.value.as_deref().and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite()) } else { None };
    let mut modifiers = modifiers.iter().map(literal_text).peekable();
    let julian_day = match number {
        Some(v) if modifiers.peek().is_some_and(|m| m.is_some_and(|m| m.eq_ignore_ascii_case("unixepoch"))) => {
            modifiers.next();
            v / 86_400.0 + 2_440_587.5
        }
        Some(v) => v,
        None if now => 0.0,
        None => return false,
    };
    // The days the modifiers may move the time by, each at most.
    let mut days = 0.0;
    for m in modifiers {
        let Some(m) = m else { return false };
        let m = m.to_ascii_lowercase();
        days += match m.as_str() {
            "subsec" | "subsecond" => 0.0,
            "localtime" | "utc" | "ceiling" | "floor" => 1.0,
            "start of day" | "start of month" | "start of year" => 366.0,
            _ if m.len() == 9 && m.starts_with("weekday ") && matches!(m.as_bytes()[8], b'0'..=b'6') => 7.0,
            _ => match shift_days(&m) {
                Some(d) => d,
                None => return false,
            },
        };
    }
    if now {
        days < 365_000.0
    } else {
        // With a margin for the rounding of months and years.
        let margin = if days > 0.0 { days + 400.0 } else { 0.0 };
        julian_day - margin >= 0.0 && julian_day + margin < 5_373_484.0
    }
}

/// How far a modifier `±N unit` (`'+7 days'`, `'-1.5 hours'`) moves a time, in days at most, as
/// SQLite spells one: a number, spaces, and a unit, with or without its plural `s`.
fn shift_days(m: &str) -> Option<f64> {
    let (number, unit) = m.split_once(' ')?;
    let unit = unit.trim_start_matches(' ');
    let unit = unit.strip_suffix('s').unwrap_or(unit);
    let days = match unit {
        "second" => 1.0 / 86_400.0,
        "minute" => 1.0 / 1_440.0,
        "hour" => 1.0 / 24.0,
        "day" => 1.0,
        "month" => 31.0,
        "year" => 366.0,
        _ => return None,
    };
    let digits = number.strip_prefix(['+', '-']).unwrap_or(number);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, "0"));
    let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(whole) || !all_digits(fraction) {
        return None;
    }
    Some(digits.parse::<f64>().ok()? * days)
}

/// Whether `printf` with this format writes a character whatever its arguments are. SQLite gives
/// NULL where it wrote none, as for an empty format, and stops at a conversion it does not know
/// (`printf('%T')` is NULL, `printf('a%T')` is 'a'), so a character of the format itself or a `%%`
/// must come before any such conversion. The directive is read as SQLite's `sqlite3_str_vappendf`
/// reads it: flags, a width, a precision and a length, in that loop's order.
fn printf_writes(format: &str) -> bool {
    let b = format.as_bytes();
    let at = |i: usize| b.get(i).copied().unwrap_or(0);
    let mut i = 0;
    loop {
        match at(i) {
            0 => return false,
            b'%' => {}
            _ => return true,
        }
        i += 1;
        // A `%` that ends the format is written as it is.
        if at(i) == 0 {
            return true;
        }
        let mut c = at(i);
        loop {
            match c {
                b'-' | b'+' | b' ' | b'#' | b'!' | b'0' | b',' => {}
                b'l' => {
                    i += 1;
                    if at(i) == b'l' {
                        i += 1;
                    }
                    c = at(i);
                    break;
                }
                b'1'..=b'9' => {
                    while at(i + 1).is_ascii_digit() {
                        i += 1;
                    }
                    if !matches!(at(i + 1), b'.' | b'l') {
                        i += 1;
                        c = at(i);
                        break;
                    }
                }
                b'*' => {
                    if !matches!(at(i + 1), b'.' | b'l') {
                        i += 1;
                        c = at(i);
                        break;
                    }
                }
                b'.' => {
                    i += 1;
                    if at(i) == b'*' {
                        i += 1;
                    } else {
                        while at(i).is_ascii_digit() {
                            i += 1;
                        }
                    }
                    if at(i) != b'l' {
                        c = at(i);
                        break;
                    }
                    i -= 1;
                }
                _ => break,
            }
            i += 1;
            c = at(i);
            if c == 0 {
                break;
            }
        }
        match c {
            b'%' => return true,
            // `T` and `S` are SQLite's own, which a SQL call cannot use: it stops there.
            b's' | b'E' | b'u' | b'G' | b'w' | b'x' | b'c' | b'z' | b'd' | b'e' | b'f' | b'g' | b'Q' | b'i' | b'X' | b'n' | b'o' | b'p' | b'q' | b'r' => i += 1,
            _ => return false,
        }
    }
}

/// Whether every conversion of a `strftime` format is one SQLite knows; any other makes it NULL,
/// as does a `%` that ends the format.
fn strftime_format(format: &str) -> bool {
    let mut bytes = format.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' && !bytes.next().is_some_and(|c| b"deFfGgHkIljJmMpPRsSTuwUVWY%".contains(&c)) {
            return false;
        }
    }
    true
}

/// SQLite's built-in scalar functions the engine's table reads, by name.
const KNOWN_FUNCTIONS: &[&str] = &[
    "coalesce", "ifnull", "nullif", "iif", "if", "min", "max", "abs", "round", "length", "instr", "unicode", "json_array_length", "json_valid", "octet_length", "sign",
    "like", "glob", "regexp", "match", "likely", "unlikely", "likelihood", "upper", "lower", "trim", "ltrim", "rtrim", "substr", "substring", "replace", "char",
    "printf", "format", "concat", "concat_ws", "json", "jsonb", "json_array", "json_object", "json_insert", "json_replace", "json_set", "json_remove", "json_patch",
    "json_quote", "json_pretty", "json_type", "hex", "quote", "typeof", "soundex", "zeroblob", "json_extract", "jsonb_extract", "->", "->>", "acos", "acosh", "asin", "asinh",
    "atan", "atan2", "atanh", "ceil", "ceiling", "cos", "cosh", "degrees", "exp", "floor", "ln", "log", "log10", "log2", "mod", "pow", "power", "radians", "sin",
    "sinh", "sqrt", "tan", "tanh", "trunc", "pi",
];

/// An integer literal's value.
fn integer(n: &Node) -> Option<i64> {
    if n.op == "integer" { n.value.as_deref().and_then(|v| v.parse().ok()) } else { None }
}

/// The terms of a condition's top-level `AND`.
fn conjuncts(n: Option<&Node>) -> Vec<&Node> {
    let mut out = Vec::new();
    let mut stack: Vec<&Node> = n.into_iter().collect();
    while let Some(n) = stack.pop() {
        if n.op == "and" {
            stack.extend(n.left.as_deref());
            stack.extend(n.right.as_deref());
        } else {
            out.push(n);
        }
    }
    out
}

/// The cursors an expression reads, or `None` when it holds a subquery, whose reads the walk
/// does not place.
fn cursors(n: &Node) -> Option<Vec<i64>> {
    if n.select.is_some() {
        return None;
    }
    let mut out = Vec::new();
    if n.op == "column" {
        out.push(n.cursor);
    }
    for child in n.left.iter().chain(n.right.iter()).map(|b| b.as_ref()).chain(n.list.iter()).chain(n.args.iter()) {
        out.extend(cursors(child)?);
    }
    Some(out)
}

/// The type of `a op b` for `+`, `-`, `*`, `/`, `%`: integer stays integer, anything else is real.
fn arithmetic(l: &Type, r: &Type) -> Type {
    match (scalar_name(l), scalar_name(r)) {
        (Some("integer" | "int8"), Some("integer" | "int8")) => Type::Scalar { name: "integer".into() },
        _ => Type::Scalar { name: "real".into() },
    }
}

/// Whether a node of this operator sits anywhere in `n`.
fn has_op(n: &Node, op: &str) -> bool {
    n.op == op || n.left.as_deref().is_some_and(|x| has_op(x, op)) || n.right.as_deref().is_some_and(|x| has_op(x, op)) || n.list.iter().any(|x| has_op(x, op)) || n.args.iter().any(|x| has_op(x, op))
}

fn has_aggregate(n: &Node) -> bool {
    if n.op == "function" && n.agg && !n.window {
        return true;
    }
    n.left.as_deref().is_some_and(has_aggregate) || n.right.as_deref().is_some_and(has_aggregate) || n.list.iter().any(has_aggregate) || n.args.iter().any(has_aggregate)
}

/// Whether an expression reads any column, at any depth (a subquery's columns included).
fn reads_column(n: &Node) -> bool {
    if n.op == "column" {
        return true;
    }
    n.left.as_deref().is_some_and(reads_column) || n.right.as_deref().is_some_and(reads_column) || n.list.iter().any(reads_column) || n.args.iter().any(reads_column) || n.select.is_some()
}

/// The type the context an expression stands in gives its operand, when it gives one.
fn is_vector(n: &Option<Box<Node>>) -> bool {
    n.as_deref().is_some_and(|n| n.op == "vector")
}

fn outer_type(ctx: &Ctx) -> Ctx {
    match ctx {
        Ctx::Typed(Some(f)) => Ctx::Typed(Some(f.clone())),
        _ => Ctx::None,
    }
}

/// The argument position of a slot (0 for any other slot).
fn i_of(slot: Slot) -> usize {
    match slot {
        Slot::Arg(i) => i,
        _ => 0,
    }
}

/// Whether a column's declared type is `BOOLEAN` or `BOOL`, in any case.
fn is_boolean_declared(declared: &str) -> bool {
    let d = declared.trim();
    d.eq_ignore_ascii_case("boolean") || d.eq_ignore_ascii_case("bool")
}

/// SQLite's five affinity rules over a declared type.
pub fn affinity(declared: &str) -> &'static str {
    let d = declared.to_ascii_uppercase();
    if d.contains("INT") {
        "integer"
    } else if d.contains("CHAR") || d.contains("CLOB") || d.contains("TEXT") {
        "text"
    } else if d.contains("BLOB") || d.is_empty() {
        "blob"
    } else if d.contains("REAL") || d.contains("FLOA") || d.contains("DOUB") {
        "real"
    } else {
        "numeric"
    }
}

/// A parameter's context while walking.
#[derive(Clone)]
enum Ctx {
    Typed(Option<Fact>),
    /// A row value's place: element i takes fact i (`(created_at, id) < (:created_at, :id)`).
    Row(Vec<Option<Fact>>),
    Bool,
    None,
}

#[derive(Clone, Copy)]
enum Slot {
    Left,
    Right,
    List(usize),
    Arg(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ddl() -> Vec<String> {
        vec![
            "create table users (id integer primary key, name text not null, email text unique, score real, avatar blob, created_at text not null default current_timestamp)".to_string(),
            "create table posts (id integer primary key, user_id integer not null references users(id), title text not null, body text)".to_string(),
        ]
    }

    fn analysis(sql: &str) -> Analysis {
        analyze_ddl(sql, &ddl(), &Extras::default()).analysis
    }

    #[test]
    fn the_facts_are_read_for_every_table_the_statement_names_and_no_with_query() {
        let mut schema = ddl();
        schema.push("create table tags (id integer primary key, post_id integer)".into());
        schema.push("create view titles as select title from posts".into());
        let tables = |sql: &str| {
            let a = analyze_ddl(sql, &schema, &Extras::default()).analysis;
            assert!(a.diagnostics.is_empty(), "{sql}: {:?}", a.diagnostics);
            let mut t = a.tables.expect("the tree names its tables");
            t.read.sort();
            t.read.dedup();
            (t.read, t.write)
        };
        let names = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(tables("select u.id, (select count(*) from posts p where p.user_id = u.id) from USERS u where exists (select 1 from tags t where t.id = u.id)"), (names(&["posts", "tags", "users"]), vec![]));
        // A WITH query is no table, and a view is named, not what it reads.
        assert_eq!(tables("with recent as (select user_id from posts) select title from titles join recent on true"), (names(&["posts", "titles"]), vec![]));
        assert_eq!(tables("select value from json_each('[1]')"), (vec![], vec![]));
        assert_eq!(tables("select s.n from (select count(*) as n from posts) s join titles on true"), (names(&["posts", "titles"]), vec![]));
        assert_eq!(tables("update users set score = (with recursive c(n) as (select 1 union all select n + 1 from c where n < 3) select count(*) from c)"), (vec![], names(&["users"])));
        // What an INSERT, an UPDATE and a DELETE write, and the tables they read.
        assert_eq!(tables("insert into tags (post_id) select id from posts"), (names(&["posts"]), names(&["tags"])));
        assert_eq!(tables("update users set name = (select max(title) from posts) where id = 1"), (names(&["posts"]), names(&["users"])));
        assert_eq!(tables("delete from users where id in (select user_id from posts)"), (names(&["posts"]), names(&["users"])));
    }

    #[test]
    fn a_statement_as_deep_as_the_engine_reads_is_typed_and_one_level_past_it_is_an_error_at_the_bound() {
        // From a thread with a small stack, as a host may call from: the embedded SQLite's
        // recursion, the reading of its tree and the walks over it run on the engine's stack.
        let ddl = vec!["create table users (id integer primary key, name text not null, nick text)".to_string()];
        let small = |sql: String| std::thread::scope(|s| std::thread::Builder::new().stack_size(256 << 10).spawn_scoped(s, || analyze_ddl(&sql, &ddl, &Extras::default()).analysis).unwrap().join().unwrap());
        // SQLite's own bound is 1,000; the engine's SQLite takes 350 (`csrc/reylo_sqlite.c`).
        for (shape, n) in [("sum", 350), ("and_or", 348), ("case", 348), ("coalesce", 349), ("subquery", 23)] {
            let at = small(crate::stack::nested(shape, n));
            assert!(at.diagnostics.is_empty() && at.kind == "select", "{shape} at {n}: {:?}", at.diagnostics);
            let past = small(crate::stack::nested(shape, n + 1));
            assert_eq!(past.diagnostics.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(), ["Expression tree is too large (maximum depth 350)"], "{shape} at {}", n + 1);
        }
        // A list does not nest: SQLite's OR is a tree, which its bound counts; an IN list is not.
        let list = (0..5_000).map(|i| i.to_string()).collect::<Vec<_>>().join(", ");
        assert!(small(format!("select id from users where id in ({list})")).diagnostics.is_empty());
    }

    fn col(a: &Analysis, name: &str) -> (String, bool) {
        let c = a.columns.iter().find(|c| c.name == name).unwrap_or_else(|| panic!("no column {name}"));
        (scalar_name(&c.ty).unwrap_or("unknown").to_string(), c.nullable)
    }

    fn param(a: &Analysis, name: &str) -> String {
        let p = a.params.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("no param {name}"));
        scalar_name(&p.ty).unwrap_or("unknown").to_string()
    }

    #[test]
    fn the_full_text_functions_are_typed() {
        // Measured on SQLite 3.53.4 over rows with a NULL in each column: `bm25` is a real on
        // every row (0 with no MATCH) and NULL with an infinite weight, `highlight` and FTS5's `snippet` are NULL where their
        // column is, and FTS4's `offsets` and `matchinfo` answer every row.
        let mut schema = ddl();
        schema.push("CREATE VIRTUAL TABLE docs USING fts5(title, body)".into());
        schema.push("CREATE VIRTUAL TABLE notes USING fts4(body)".into());
        let a = |sql: &str| {
            let a = analyze_ddl(sql, &schema, &Extras::default()).analysis;
            assert!(a.diagnostics.is_empty(), "{sql}: {:?}", a.diagnostics);
            a
        };
        let fts5 = a("select rowid as id, bm25(docs) as rank, bm25(docs, :w, 1.0) as weighted, highlight(docs, :col, :open, '</b>') as hit, \
                      snippet(docs, -1, '<b>', '</b>', '…', 8) as excerpt from docs where docs match :q order by rank");
        assert_eq!(col(&fts5, "rank"), ("real".into(), false));
        assert_eq!(col(&fts5, "weighted"), ("real".into(), true));
        assert_eq!(col(&fts5, "hit"), ("text".into(), true));
        assert_eq!(col(&fts5, "excerpt"), ("text".into(), true));
        assert_eq!((param(&fts5, "w"), param(&fts5, "col"), param(&fts5, "open"), param(&fts5, "q")), ("real".into(), "integer".into(), "text".into(), "text".into()));
        let fts4 = a("select offsets(notes) as o, matchinfo(notes) as m, snippet(notes) as s from notes where notes match :q");
        assert_eq!(col(&fts4, "o"), ("text".into(), false));
        assert_eq!(col(&fts4, "m"), ("blob".into(), false));
        assert_eq!(col(&fts4, "s"), ("text".into(), true));
        // Inside built JSON, each nests as the value it is.
        let built = a("select json_object('hit', highlight(docs, 0, '<b>', '</b>'), 'rank', bm25(docs)) as j from docs where docs match :q");
        assert_eq!(shown(&built), "j: { hit: string | null; rank: number }");
    }

    #[test]
    fn a_built_in_is_non_null_only_where_sqlite_never_answers_null_for_non_null_arguments() {
        // Every argument below is non-null; each NULL noted is what SQLite 3.53 answers.
        let ddl = vec![
            "create table o (id integer primary key, placed_at text not null, x real not null, n integer not null, b blob not null, v any not null, doc text not null, fmt text not null) strict".to_string(),
            "create table loose (id integer primary key, num numeric not null)".to_string(),
        ];
        let typed = |expr: &str| {
            let a = analyze_ddl(&format!("select {expr} as v from o, loose"), &ddl, &Extras::default()).analysis;
            assert!(a.diagnostics.is_empty(), "{expr}: {:?}", a.diagnostics);
            col(&a, "v")
        };
        let nullable = |expr: &str| typed(expr).1;
        // A time value SQLite cannot read, a number outside its range, a modifier that fails.
        for expr in [
            "date(placed_at)", "time(placed_at)", "datetime(placed_at)", "julianday(placed_at)", "unixepoch(placed_at)", "strftime('%Y', placed_at)",
            "date(placed_at, '+1 day')", "timediff(placed_at, 'now')", "date(n)", "date(x, 'unixepoch')", "date('2024-01-01')", "date(:d)",
            "date('now', 'weekday 7')", "date('now', 'unixepoch')", "date('now', 'julianday')", "date('now', '+8000 years')", "date('now', '+1 day ')",
            "date('now', '+600 years', '+600 years')", "date('now', fmt)", "date('now ')", "strftime('%Q', 'now')", "strftime('%Y%')", "strftime(fmt)",
            "date(5373485)", "datetime(1e18, 'unixepoch')", "date(0, '-1 day')", "date(1700000000, 'unixepoch', 'unixepoch')", "date(1, 'auto')", "date(-1)",
        ] {
            assert!(nullable(expr), "{expr}");
        }
        // The current time with modifiers that cannot fail on it.
        for expr in [
            "date()", "time()", "datetime()", "julianday()", "unixepoch()", "date('now')", "date('NOW')", "datetime('now', 'localtime')", "datetime('now', 'utc')",
            "date('now', 'start of month', '+1 month', '-1 day')", "date('now', 'weekday 0')", "datetime('now', '-7 days', '+1.5 hours', '+30 MINUTES')",
            "date('now', 'subsec')", "strftime('%Y-%m-%d %H:%M:%f %% %j', 'now')", "strftime('%s')", "strftime('')", "timediff('now', 'now')",
            "strftime('%Y', current_timestamp)", "date(current_date, '+1 month')", "datetime(0, 'unixepoch')", "date(2460000.5)", "date(1700000000, 'unixepoch', 'start of day', '+1 day')",
        ] {
            assert!(!nullable(expr), "{expr}");
        }
        assert_eq!(typed("unixepoch()"), ("integer".into(), false));
        assert_eq!(typed("unixepoch('now', 'subsec')"), ("real".into(), false));
        assert_eq!(typed("unixepoch('now', fmt)"), ("real".into(), true));
        // Outside a function's domain (`sqrt(-1)`, `ln(0)`, `acos(2)`, `mod(1, 0)`, `pow(-1, 0.5)`)
        // or at an infinity (`sin(1e999)`).
        for f in ["acos(x)", "acosh(x)", "asin(x)", "atanh(x)", "cos(x)", "sin(x)", "tan(x)", "ln(x)", "log(x)", "log(2, x)", "log10(x)", "log2(x)", "mod(n, x)", "pow(x, 0.5)", "power(x, n)", "sqrt(x)", "sqrt(4)"] {
            assert_eq!(typed(f), ("real".into(), true), "{f}");
        }
        // Defined on every number; NULL for an argument that is no number (`sign('')`).
        for f in ["atan(x)", "atan2(x, n)", "asinh(x)", "sinh(x)", "cosh(n)", "tanh(x)", "exp(x)", "degrees(x)", "radians(n)", "ceil(x)", "ceiling(x)", "floor(n)", "trunc(x)", "pi()"] {
            assert_eq!(typed(f), ("real".into(), false), "{f}");
        }
        assert_eq!(typed("sign(x)"), ("integer".into(), false));
        for f in ["sign(placed_at)", "sign(v)", "sign(num)", "ceil(placed_at)", "floor(b)", "exp(v)", "atan2(x, placed_at)", "sinh(num)"] {
            assert!(nullable(f), "{f}");
        }
        // An empty format, or one that stops at a conversion SQLite does not know (`printf('%T')`).
        for f in ["printf(fmt, n)", "printf('', n)", "printf('%T')", "printf('%s', placed_at)", "format('%d%d', n, n)", "printf('%T x')", "printf('%5-dx', n)", "printf('%ls', n)", "printf('%.*s', n, placed_at)", "printf('%-')"] {
            assert!(nullable(f), "{f}");
        }
        for f in ["printf('x%s', null)", "format('-%d', n)", "printf('%%')", "printf('%')", "printf('%s-%d', placed_at, n)", "printf('%5.2f%%', x)", "printf('%-10s|', placed_at)", "printf('%lld:', n)", "printf('%*.*s.', n, n, placed_at)", "printf('a%T')"] {
            assert!(!nullable(f), "{f}");
        }
        // The slice of an empty blob is NULL, and of any blob a blob.
        assert_eq!(typed("substr(placed_at, 2)"), ("text".into(), false));
        assert_eq!(typed("substr(n, 2, 1)"), ("text".into(), false));
        assert_eq!(typed("substr(:s, 2)"), ("text".into(), false));
        assert_eq!(typed("substr(b, 2)"), ("blob".into(), true));
        assert_eq!(typed("substring(v, 1, 2)"), ("unknown".into(), true));
        // A path that finds nothing, and `json_remove(j, '$')`.
        for f in ["json_type(doc, '$.a')", "json_array_length(doc, '$.a')", "json_remove(doc, '$')", "json_remove(doc, '$.a')"] {
            assert!(nullable(f), "{f}");
        }
        for f in ["json_type(doc)", "json_array_length(doc)", "json_remove(doc)", "json(doc)", "json_set(doc, '$.a', null)", "json_patch(doc, doc)", "json_valid(doc)"] {
            assert!(!nullable(f), "{f}");
        }
        assert_eq!(typed("jsonb(doc)"), ("blob".into(), false));
        assert_eq!(typed("(select jsonb_group_array(id) from o)"), ("blob".into(), false));
        // `iif` with an even count of arguments has no ELSE: NULL where no condition holds.
        assert_eq!(typed("iif(n > 0, placed_at)"), ("text".into(), true));
        assert_eq!(typed("iif(n > 0, placed_at, n > 1, doc)"), ("text".into(), true));
        assert_eq!(typed("iif(n > 0, placed_at, n > 1, doc, fmt)"), ("text".into(), false));
        assert_eq!(typed("if(n > 0, x, n > 1, x, 1.5)"), ("real".into(), false));
        // A condition takes a truth value, which SQLite reads as an integer; a value the other values' type.
        assert_eq!(param(&analyze_ddl("select id from o where x = iif(n > 0, 1.5, :c, 2.5, :e)", &ddl, &Extras::default()).analysis, "c"), "integer");
        assert_eq!(param(&analyze_ddl("select id from o where x = iif(n > 0, 1.5, :c, 2.5, :e)", &ddl, &Extras::default()).analysis, "e"), "real");
        // `total` of a `1e999` and a `-1e999` is NaN, which SQLite gives as NULL; of integers never.
        assert_eq!(typed("(select total(x) from o)"), ("real".into(), true));
        assert_eq!(typed("(select total(n) from o)"), ("real".into(), false));
        assert_eq!(typed("total(x) over ()"), ("real".into(), true));
        // What never gives NULL for non-null arguments.
        for f in [
            "abs(x)", "round(x, 2)", "length(b)", "instr(placed_at, '')", "octet_length(b)", "upper(b)", "lower(placed_at)", "trim(placed_at, '')", "replace(placed_at, '', 'x')",
            "char(n)", "hex(b)", "quote(v)", "typeof(v)", "concat(placed_at, null)", "like(placed_at, 'a%')", "glob('*', placed_at)", "json_array(n)", "json_object('a', n)", "json_quote(v)",
        ] {
            assert!(!nullable(f), "{f}");
        }
        assert!(nullable("unicode(placed_at)"));
    }

    #[test]
    fn the_postgres_nullability_rules_hold_on_sqlite() {
        let nullable_param = |sql: &str, name: &str| analysis(sql).params.iter().find(|p| p.name == name).is_some_and(|p| p.nullable);
        // A parameter the statement passes first to `coalesce` or `ifnull` takes NULL; any other
        // is a value, one it tests for NULL too. Each parameter
        // appears once, so a value coalesced and read again is selected once in a WITH, and the
        // coalesce of its column is the coalesce of the parameter, however it is named.
        assert!(nullable_param("with input as (select cast(:p as integer) as p) select id from users, input where score = coalesce(input.p, score)", "p"));
        assert!(nullable_param("with input as (select :p as p) select id from users, input i where score = ifnull(p, score) and score > i.p", "p"));
        assert!(nullable_param("select id from users, (select :p as p) s where coalesce(s.p, score) = score", "p"));
        assert!(nullable_param("with input as (select :p as p) select id from users, input where score = coalesce(input.p, score)", "p"));
        assert!(!nullable_param("with input as (select :p as p) select id from users, input where score > input.p", "p"));
        assert!(!nullable_param("with input as (select :p as p union all select 1) select id from users, input where coalesce(input.p, 0) = 0", "p"));
        assert!(!nullable_param("with input as (select cast(:p as integer) as p) select id from users, input where (input.p is null or score > input.p)", "p"));
        assert!(!nullable_param("select id from users where cast(:p as integer) is null", "p"));
        assert!(!nullable_param("select id from users where :p is not null", "p"));
        assert!(!nullable_param("select id from users where email is :p", "p"));
        assert!(!nullable_param("select id from users where :p is not email", "p"));
        assert!(!nullable_param("select id from users where :p isnull", "p"));
        assert!(!nullable_param("select id from users where score = coalesce(score, :p)", "p"));
        let sql = "select id from users where (:p is null or score > :p)";
        let repeated = analysis(sql);
        assert_eq!(repeated.diagnostics.iter().map(|d| (d.code, d.start, d.length)).collect::<Vec<_>>(), [(3, sql.rfind(":p").unwrap(), 2)]);
        assert!(nullable_param("select coalesce(:s, 0) as x", "s"));
        assert!(!nullable_param("select id from users where score > :p", "p"));
        assert_eq!(col(&analysis("select upper(:s) as u"), "u"), ("text".into(), false));
        // A strict WHERE narrows the column it tests, and reduces the outer join of its item.
        assert_eq!(col(&analysis("select email from users where email is not null"), "email"), ("text".into(), false));
        assert_eq!(col(&analysis("select email from users where lower(email) = :e or email like 'a%'"), "email").1, false);
        assert_eq!(col(&analysis("select email from users where email like 'a%' or id = 1"), "email").1, true);
        assert_eq!(col(&analysis("select email from users where lower(email) = :e and email like 'a%'"), "email").1, false);
        let reduced = analysis("select p.title, u.name from posts p left join users u on u.id = p.user_id where u.email = :e");
        assert_eq!(col(&reduced, "name"), ("text".into(), false));
        assert_eq!(col(&analysis("select p.title, u.name from posts p left join users u on u.id = p.user_id"), "name"), ("text".into(), true));
        // An ON term is not a WHERE test: the posts that meet no user still have NULL there.
        assert_eq!(col(&analysis("select p.title, u.name from posts p left join users u on u.id = p.user_id and u.email = :e"), "name"), ("text".into(), true));
        // Under a plain GROUP BY no group is empty.
        let grouped = analysis("select user_id, max(title) as t, min(id) as m, group_concat(title) as g from posts group by user_id");
        assert_eq!((col(&grouped, "t").1, col(&grouped, "m").1, col(&grouped, "g").1), (false, false, false));
        assert!(col(&analysis("select max(title) as t from posts"), "t").1);
        // `concat` skips NULLs; `concat_ws` is NULL only for a NULL separator.
        let concat = analysis("select concat(email, 'x') as c, concat_ws(',', email) as w, concat_ws(email, 'a') as n from users");
        assert_eq!((col(&concat, "c").1, col(&concat, "w").1, col(&concat, "n").1), (false, false, true));
    }

    #[test]
    fn a_cast_parameter_is_its_casts_type_and_a_bare_one_its_columns() {
        // A `:name::type` cast compiles to `CAST(? AS type)`, and `::json` to `json(?)`: the
        // parameter is the cast's type and takes no NULL, written whole or not. A bare one written whole (an INSERT's values, every row of a
        // VALUES list, an UPDATE's or an upsert's SET, a row value's elements) is as nullable as
        // its column; a column a CHECK refuses NULL in, by a conjunct `col is not null`, is not.
        // Both take NULL coalesced, and neither tested for NULL.
        let ddl = vec!["create table users (id integer primary key, name text not null, email text unique, score real, n integer, doc text, meta text not null default '{}', nick text check (nick is not null), code text check (length(code) > 0 and code is not null), tag text check (tag <> '')) strict".to_string()];
        let types = |sql: &str| {
            let a = analyze_ddl(sql, &ddl, &Extras::default()).analysis;
            assert!(a.diagnostics.is_empty(), "{sql}: {:?}", a.diagnostics);
            a.params.iter().map(|p| format!("{}{}", p.name, if p.nullable { "?" } else { "" })).collect::<Vec<_>>().join(" ")
        };
        for (sql, want) in [
            ("insert into users (name, email) values (:name, :email) on conflict (email) do update set name = excluded.name returning id", "name email?"),
            ("insert into users (name, email) values (CAST(:name AS TEXT), CAST(:email AS TEXT)) on conflict (email) do update set name = excluded.name returning id", "name email"),
            ("insert into users (name, email, score, n, doc, meta) values (:name, :email, :score, :n, :doc, :meta)", "name email? score? n? doc? meta"),
            ("insert into users (name, email, score, n, doc, meta) values (CAST(:name AS TEXT), CAST(:email AS TEXT), CAST(:score AS NUMERIC), CAST(:n AS INTEGER), json(:doc), jsonb(:meta))", "name email score n doc meta"),
            ("insert into users (name, email) values (:a, :b), (:c, CAST(:d AS TEXT))", "a b? c d"),
            ("update users set email = :email, name = :name, score = :score, n = CAST(:n AS INTEGER), doc = json(:doc) where id = :id", "email? name score? n doc id"),
            ("update users set (name, email) = (:name, :email) where id = :id", "name email? id"),
            ("update users set (name, email) = (CAST(:name AS TEXT), CAST(:email AS TEXT)) where id = CAST(:id AS INTEGER)", "name email id"),
            ("insert into users (name, email) values ('a', 'b') on conflict (email) do update set email = :e, n = CAST(:n AS INTEGER), name = :name", "e? n name"),
            ("update users set email = CAST(CAST(:e AS INTEGER) AS TEXT), meta = json(:m) where id = 1", "e m"),
            ("insert into users (name, nick, code, tag) values (:name, :nick, :code, :tag)", "name nick code tag?"),
            ("update users set nick = :nick, code = :code, tag = :tag where id = :id", "nick code tag? id"),
            ("update users set n = coalesce(CAST(:n AS INTEGER), n), doc = ifnull(json(:doc), doc), score = coalesce(:s, score) where id = CAST(:id AS INTEGER)", "n? doc? s? id"),
            ("select id from users where (json(:doc) is null or doc = json(:d)) and :x is not null", "doc d x"),
            ("insert into users (name, email) select CAST(:name AS TEXT), CAST(:email AS TEXT)", "name email"),
            ("insert into users (name, email) select :name, :email", "name email?"),
        ] {
            assert_eq!(types(sql), want, "{sql}");
        }
        // A host function registered over `json` may answer anything for NULL: what it wraps is no
        // parameter passed to `coalesce`.
        let host = Extras { total: vec![TotalFunction { name: "json".into(), arity: 1, returns: Affinity::Text }], host: None };
        let a = analyze_ddl("update users set doc = coalesce(json(:doc), doc) where id = 1", &ddl, &host).analysis;
        assert_eq!(a.params.iter().map(|p| p.nullable).collect::<Vec<_>>(), [false]);
    }

    #[test]
    fn a_parameter_takes_the_type_its_call_case_or_compound_stands_in() {
        assert_eq!(param(&analysis("select id from users where name = coalesce(:a, :b)"), "a"), "text");
        assert_eq!(param(&analysis("select id from users where name = coalesce(:a, :b)"), "b"), "text");
        assert_eq!(param(&analysis("select id from users where name = case when id > 1 then :a else :b end"), "a"), "text");
        assert_eq!(param(&analysis("select id from users where name = iif(id > 1, :a, 'x')"), "a"), "text");
        assert_eq!(param(&analysis("select case :kind when 'a' then 1 else 2 end as k from users"), "kind"), "text");
        assert_eq!(param(&analysis("select name from users union all select :x"), "x"), "text");
        assert_eq!(param(&analysis("select :x union all select name from users"), "x"), "text");
        // A call whose result is not an argument's gives none of its own context.
        assert_eq!(param(&analysis("select id from users where name = printf(:f, 1)"), "f"), "text");
        assert_eq!(param(&analysis("select :x as v"), "x"), "unknown");
    }

    #[test]
    fn joins_grouping_distinct_and_set_operations_prove_rows_as_on_postgres() {
        let rows = |sql: &str| analysis(sql).cardinality;
        for sql in [
            "select p.title, u.name from posts p join users u on u.id = p.user_id where p.id = :id",
            "select p.title, u.name from posts p left join users u on u.id = p.user_id where p.id = :id",
            "select p.title, u.name from posts p, users u where u.id = p.user_id and p.id = :id",
            "select a.name from users a join users b on b.id = a.id where a.id = :id",
            "select s.name from (select * from users where id = :id) s",
            "select user_id, count(*) as n from posts where user_id = :u group by user_id",
            "select distinct user_id from posts where user_id = :u",
            "select id from users where id = :id intersect select user_id from posts",
            "select id from users where id = :id except select user_id from posts",
        ] {
            assert_eq!(rows(sql), "maybeOne", "{sql}");
        }
        for sql in [
            // Many posts per user.
            "select p.title from posts p join users u on u.id = p.user_id where u.id = :id",
            "select title, count(*) as n from posts where user_id = :u group by title",
            "select user_id from posts except select id from users where id = :id",
            // A FULL JOIN keeps each side's unmatched rows.
            "select p.title, u.name from posts p full join users u on u.id = p.user_id where p.id = :id",
            // A volatile value is evaluated for each row.
            "select name from users where id = abs(random()) % 3",
            "delete from users where id = abs(random()) % 3 returning id",
        ] {
            assert_eq!(rows(sql), "many", "{sql}");
        }
    }

    #[test]
    fn an_outer_joins_on_term_pins_its_right_side_but_groups_nothing() {
        // SQLite moves the ON terms into the WHERE: `u.id = :id` restricts which user a post
        // meets, and the posts that meet none have NULL there, a second group.
        assert_eq!(analysis("select u.name, count(*) as n from posts p left join users u on u.id = :id and u.id = p.user_id group by u.name").cardinality, "many");
        assert_eq!(analysis("select u.name, count(*) as n from posts p left join users u on u.id = p.user_id where u.id = :id group by u.name").cardinality, "maybeOne");
        // The ON term still pins the right side against a pinned left one.
        assert_eq!(analysis("select p.title, u.name from posts p left join users u on u.id = :u where p.id = :id").cardinality, "maybeOne");
    }

    #[test]
    fn an_insert_returns_no_row_where_something_can_skip_it() {
        let count = |sql: &str, extra: &[&str]| {
            let mut schema = ddl();
            schema.extend(extra.iter().map(|s| s.to_string()));
            analyze_ddl(sql, &schema, &Extras::default()).analysis.cardinality
        };
        let insert = "insert into users (name) values (:n) returning id";
        assert_eq!(count(insert, &[]), "one");
        // A BEFORE trigger's RAISE(IGNORE) skips the row; an AFTER trigger cannot.
        assert_eq!(count(insert, &["create trigger skip before insert on users begin select raise(ignore) where new.name = 'x'; end"]), "maybeOne");
        assert_eq!(count(insert, &["create trigger untimed insert on users begin select 1; end"]), "maybeOne", "BEFORE is the default");
        assert_eq!(count(insert, &["create trigger later after insert on users begin select 1; end"]), "one");
        // `DO UPDATE … WHERE` leaves a conflicting row alone and returns nothing for it.
        assert_eq!(count("insert into users (id, name) values (:id, :n) on conflict (id) do update set name = excluded.name where users.score > 1 returning id", &[]), "maybeOne");
        assert_eq!(count("insert into users (id, name) values (:id, :n) on conflict (id) do update set name = excluded.name returning id", &[]), "one");
    }

    #[test]
    fn a_unique_key_bounds_only_a_comparison_under_its_own_collation() {
        let count = |sql: &str| {
            let mut schema = ddl();
            schema.push("create table c (id integer primary key, e text collate nocase, b text unique)".into());
            schema.push("create unique index c_e on c (e collate binary)".into());
            analyze_ddl(sql, &schema, &Extras::default()).analysis.cardinality
        };
        // `e` compares under NOCASE, and its unique index tells 'A' from 'a'.
        assert_eq!(count("select id from c where e = :e"), "many");
        assert_eq!(count("select id from c where b = :b"), "maybeOne");
        assert_eq!(count("select id from c where b = :b collate nocase"), "many");
        assert_eq!(count("select e from c where id = :id"), "maybeOne");
    }

    #[test]
    fn a_key_holds_only_where_the_comparison_leaves_the_column_as_it_is() {
        let count = |sql: &str| {
            let mut schema = ddl();
            schema.push("create table t (id integer primary key, code text unique)".into());
            schema.push("create table b (id integer primary key, n integer, nc text collate nocase)".into());
            schema.push("create table a (id integer primary key, k unique, kt text unique)".into());
            schema.push("create table rt (rowid integer, v text)".into());
            analyze_ddl(sql, &schema, &Extras::default()).analysis.cardinality
        };
        // A side of INTEGER affinity applies NUMERIC affinity to a TEXT or untyped key: '01' and
        // '1' compare equal to 1, and so do 7 and '7'.
        assert_eq!(count("select id from t where code = cast(:c as integer)"), "many");
        assert_eq!(count("select a.id from b join a on a.k = b.n where b.id = 1"), "many");
        assert_eq!(count("update t set id = id where code = cast(:c as integer) returning id"), "many");
        assert_eq!(count("select id from t where code = :c"), "maybeOne");
        assert_eq!(count("select a.id from b join a on a.kt = b.nc where b.id = 1"), "maybeOne");
        // A column on the left decides the collation: NOCASE meets 'x' and 'X'.
        assert_eq!(count("select a.id from b join a on b.nc = a.kt where b.id = 1"), "many");
        // A declared column named rowid hides the rowid; `_rowid_` still reads it.
        assert_eq!(count("select v from rt where rowid = :r"), "many");
        assert_eq!(count("select v from rt where _rowid_ = :r"), "maybeOne");
    }

    #[test]
    fn a_function_the_host_registers_over_a_built_in_proves_nothing_strict() {
        let lower = Extras { total: vec![TotalFunction { name: "lower".into(), arity: 1, returns: Affinity::Text }], host: None };
        let nullable = |extras: &Extras| col(&analyze_ddl("select email from users where lower(email) = :e", &ddl(), extras).analysis, "email").1;
        assert!(!nullable(&Extras::default()));
        assert!(nullable(&lower));
    }

    #[test]
    fn a_table_whose_check_calls_a_function_the_host_lacks_still_replays() {
        // sqlite-jsonschema's function, which the application's connection loads and the type
        // check's does not: CREATE TABLE resolves the CHECK, and the table was left out.
        let ddl = vec!["create table docs (id integer primary key, doc text check (doc is null or jsonschema_matches('{\"type\":\"object\"}', doc)))".to_string()];
        let without = Extras { total: vec![], host: Some(vec![HostFunction { name: "lower".into(), arity: 1, kind: 's' }]) };
        for extras in [Extras::default(), without.clone()] {
            let read = analyze_ddl("select id, doc from docs", &ddl, &extras).analysis;
            assert!(read.diagnostics.is_empty(), "{:?}", read.diagnostics);
            assert_eq!(read.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["id", "doc"]);
            let write = analyze_ddl("insert into docs (doc) values (:doc) returning id", &ddl, &extras).analysis;
            assert!(write.diagnostics.is_empty(), "{:?}", write.diagnostics);
            assert_eq!(param(&write, "doc"), "text");
        }
        // A statement that calls the function itself is still refused where the host lacks it.
        let call = analyze_ddl("select jsonschema_matches('{}', doc) as m from docs", &ddl, &without).analysis;
        assert!(!call.diagnostics.is_empty());
    }

    #[test]
    fn unary_plus_is_its_operand() {
        // `+name` returns the text itself, and SQLite writes a one-element `x IN (e)` as `x = +e`.
        let a = analysis("select +name as p, -score as m from users where name in (:n) and id in (:i)");
        assert_eq!(col(&a, "p"), ("text".into(), false));
        assert_eq!(col(&a, "m"), ("real".into(), true));
        assert_eq!(param(&a, "n"), "text");
        assert_eq!(param(&a, "i"), "integer");
    }

    #[test]
    fn columns_from_provenance_and_rules() {
        let a = analysis("select id, name, email, score, length(email) as l, coalesce(email, 'x') as e, id * 2 as d, id / 2 as h, count(*) as n from users");
        assert_eq!(col(&a, "id"), ("integer".into(), false));
        assert_eq!(col(&a, "name"), ("text".into(), false));
        assert_eq!(col(&a, "email"), ("text".into(), true));
        assert_eq!(col(&a, "score"), ("real".into(), true));
        assert_eq!(col(&a, "l"), ("integer".into(), true));
        assert_eq!(col(&a, "e"), ("text".into(), false));
        assert_eq!(col(&a, "d"), ("integer".into(), false));
        assert_eq!(col(&a, "h"), ("integer".into(), false));
        assert_eq!(col(&a, "n"), ("integer".into(), false));
        assert_eq!(a.cardinality, "one");
    }

    #[test]
    fn outer_join_and_subquery_provenance() {
        let a = analysis("select u.id, p.title from users u left join posts p on p.user_id = u.id");
        assert_eq!(col(&a, "title"), ("text".into(), true));
        let a = analysis("select t.id, t.name from (select id, name from users) t where t.id = :id");
        assert_eq!(col(&a, "id"), ("integer".into(), false));
        assert_eq!(col(&a, "name"), ("text".into(), false));
        assert_eq!(param(&a, "id"), "integer");
    }

    #[test]
    fn a_recursive_with_query_is_typed_through_its_reads_of_itself() {
        // Each column holds for every round: the first arm's rows on their own, each later row
        // given the row it was computed from.
        let a = analysis("with recursive t (id, depth) as (select id, 0 from users union all select p.id, t.depth + 1 from posts p join t on p.user_id = t.id) select id, depth from t");
        assert_eq!(col(&a, "id"), ("integer".into(), false));
        assert_eq!(col(&a, "depth"), ("integer".into(), false));
        // A round that reads a nullable column, or NULL, leaves the column nullable.
        let a = analysis("with recursive t (id, body) as (select id, title from posts union all select p.id, p.body from posts p join t on p.user_id = t.id) select id, body from t");
        assert_eq!(col(&a, "body"), ("text".into(), true));
        let a = analysis("with recursive t (n, x) as (select 1, 'a' union all select n + 1, null from t where n < 3) select n, x from t");
        assert_eq!(col(&a, "x"), ("text".into(), true));
        // A table named as a WITH query that only reads it is that table, not a read of itself.
        let a = analysis("with users as (select id, email from main.users) select id, email from users");
        assert_eq!(col(&a, "email"), ("text".into(), true));
    }

    #[test]
    fn parameters_from_context() {
        let a = analysis("select id from users where email = :email and score >= :min and id in (:a, :b) and created_at between :from and :to and name like :q and lower(name) = :n limit :lim offset :off");
        assert_eq!(param(&a, "email"), "text");
        assert_eq!(param(&a, "min"), "real");
        assert_eq!(param(&a, "a"), "integer");
        assert_eq!(param(&a, "from"), "text");
        assert_eq!(param(&a, "q"), "text");
        assert_eq!(param(&a, "n"), "text");
        assert_eq!(param(&a, "lim"), "integer");
        assert_eq!(param(&a, "off"), "integer");
        let a = analysis("insert into users (email, name, score) values (:e, upper(:n), :s * 2) returning id");
        assert_eq!(param(&a, "e"), "text");
        assert_eq!(param(&a, "n"), "text");
        assert_eq!(param(&a, "s"), "integer");
        assert_eq!(a.cardinality, "one");
        let a = analysis("update users set name = :name, score = :score where id = :id returning id");
        assert_eq!(param(&a, "name"), "text");
        assert_eq!(param(&a, "score"), "real");
        assert_eq!(param(&a, "id"), "integer");
        assert_eq!(a.cardinality, "maybeOne");
    }

    #[test]
    fn a_row_value_types_each_parameter_by_the_element_it_is_compared_with() {
        // A keyset page: each element of a row value takes the type of its partner, in every
        // comparison SQLite takes a row value in, a subquery's columns and a VALUES list too.
        let ddl = vec!["create table users (id integer primary key, name text not null, created_at text not null) strict".to_string()];
        let types = |sql: &str| {
            let a = analyze_ddl(sql, &ddl, &Extras::default()).analysis;
            assert!(a.diagnostics.is_empty(), "{sql}: {:?}", a.diagnostics);
            a.params.iter().map(|p| format!("{}: {}{}", p.name, scalar_name(&p.ty).unwrap_or("unknown"), if p.nullable { " | null" } else { "" })).collect::<Vec<_>>().join(", ")
        };
        let page = "select id, name, created_at from users where (created_at, id) < (:created_at, :id) order by created_at desc, id desc limit 20";
        assert_eq!(types(page), "created_at: text, id: integer");
        for op in ["=", "<>", "!=", "<", "<=", ">", ">=", "is", "is not"] {
            assert_eq!(types(&format!("select id from users where (created_at, id) {op} (:at, :id)")), "at: text, id: integer", "{op}");
            assert_eq!(types(&format!("select id from users where (:at, :id) {op} (created_at, id)")), "at: text, id: integer", "{op} reversed");
        }
        assert_eq!(types("select id from users where (created_at, :id) < (:at, id)"), "id: integer, at: text");
        assert_eq!(types("select id from users where (created_at, id) between (:lo_at, :lo_id) and (:hi_at, :hi_id)"), "lo_at: text, lo_id: integer, hi_at: text, hi_id: integer");
        assert_eq!(types("select id from users where (:at, :id) between (created_at, id) and ('9999', 0)"), "at: text, id: integer");
        assert_eq!(types("select id from users where (created_at, id) in (select :at, :id)"), "at: text, id: integer");
        assert_eq!(types("select id from users where (:at, :id) in (select created_at, id from users)"), "at: text, id: integer");
        assert_eq!(types("select id from users where (created_at, id) in ((:a1, :i1), (:a2, :i2))"), "a1: text, i1: integer, a2: text, i2: integer");
        assert_eq!(types("select id from users where (created_at, id) = (select :at, :id)"), "at: text, id: integer");
        assert_eq!(types("select case (created_at, id) when (:at, :id) then 1 else 0 end as hit from users"), "at: text, id: integer");
        assert_eq!(types("select case (:at, :id) when (created_at, id) then 1 else 0 end as hit from users"), "at: text, id: integer");
        assert_eq!(types("update users set (name, created_at) = (select :name, :at) where id = :id"), "name: text, at: text, id: integer");
        // One column of a subquery takes the type it is compared with, as a row's columns do.
        assert_eq!(types("select id from users where id = (select :p)"), "p: integer");
        assert_eq!(types("select id from users where id in (select :p union all select 1)"), "p: integer");
    }

    #[test]
    fn json_each_reads_text() {
        // A list of values in one parameter: the JSON text and the path are text, as written or cast.
        let a = analysis("select id from users where id in (select value from json_each(:ids))");
        assert_eq!(param(&a, "ids"), "text");
        assert!(!a.params.iter().find(|p| p.name == "ids").unwrap().nullable);
        let a = analysis("select key, value from json_tree(:doc, :path)");
        assert_eq!(param(&a, "doc"), "text");
        assert_eq!(param(&a, "path"), "text");
        assert_eq!(param(&analysis("select value from json_each(cast(:ids as text))"), "ids"), "text");
        let a = analysis("select value from json_each(coalesce(:ids, '[]'))");
        assert_eq!(param(&a, "ids"), "text");
        assert!(a.params.iter().find(|p| p.name == "ids").unwrap().nullable);
        // A parameter beside the call keeps its own context.
        let a = analysis("select users.id from users, json_each(:ids) j where users.id = j.value and users.name = :n");
        assert_eq!(param(&a, "ids"), "text");
        assert_eq!(param(&a, "n"), "text");
    }

    #[test]
    fn a_limit_only_lowers_a_bound_and_a_from_of_one_row_items_is_one_row() {
        let rows = |sql: &str| analysis(sql).cardinality;
        // A LIMIT above one, one from a parameter and an OFFSET remove rows and add none.
        assert_eq!(rows("select * from users where id = :id limit 5"), "maybeOne");
        assert_eq!(rows("select * from users where id = :id limit :n offset :skip"), "maybeOne");
        assert_eq!(rows("select * from users where id = :id limit 2, 5"), "maybeOne");
        assert_eq!(rows("select * from users limit 5"), "many");
        assert_eq!(rows("select * from users limit -1"), "many");
        assert_eq!(rows("select count(*) as n from users limit 5"), "one");
        assert_eq!(rows("select count(*) as n from users limit 5 offset 0"), "one");
        assert_eq!(rows("select count(*) as n from users limit 0"), "maybeOne");
        assert_eq!(rows("select count(*) as n from users limit :n"), "maybeOne");
        assert_eq!(rows("select count(*) as n from users limit -1"), "maybeOne");
        assert_eq!(rows("select count(*) as n from users limit 5 offset 1"), "maybeOne");
        // A derived table and a WITH query of one row, alone and joined.
        assert_eq!(rows("select * from (select count(*) as n from users) s"), "one");
        assert_eq!(rows("with s as (select count(*) as n from users) select n from s"), "one");
        assert_eq!(rows("select a.n, b.m from (select count(*) as n from users) a, (select count(*) as m from posts) b"), "one");
        assert_eq!(rows("select a.n, b.m from (select count(*) as n from users) a join (select count(*) as m from posts) b on 1 = 1"), "one");
        assert_eq!(rows("select a.n, u.name from (select count(*) as n from users) a left join users u on u.id = :id"), "one");
        // A WHERE or an inner join's condition can drop the row, an unpinned side multiplies it,
        // and a FULL JOIN is two rows where its condition fails.
        assert_eq!(rows("select s.n from (select count(*) as n from users) s where s.n > :min"), "maybeOne");
        assert_eq!(rows("select a.n, b.m from (select count(*) as n from users) a join (select count(*) as m from posts) b on a.n = b.m"), "maybeOne");
        assert_eq!(rows("select a.n, b.m from (select count(*) as n from users) a join (select count(*) as m from posts) b on 1 = 2"), "maybeOne");
        assert_eq!(rows("select s.n from (select count(*) as n from users limit :n) s"), "maybeOne");
        assert_eq!(rows("select a.n, u.name from (select count(*) as n from users) a left join users u on u.name = :name"), "many");
        assert_eq!(rows("select a.n, u.name from users u left join (select count(*) as n from users) a on 1 where u.id = :id"), "maybeOne");
        assert_eq!(rows("select a.n, b.m from (select count(*) as n from users) a full join (select count(*) as m from posts) b on a.n = b.m + 1"), "many");
        assert_eq!(rows("select a.n, u.name from (select count(*) as n from users) a, users u"), "many");
        // An aggregate over columns of the query around a subquery is that query's: the subquery
        // is a row per row of `posts`, and none when it has none.
        for sql in [
            "select (select count(u.id) from posts where 0) as n from users u",
            "select (select count(u.id) from posts where 0 limit 5) as n from users u",
            "select (select s.n from (select count(u.id) as n from posts where 0) s) as n from users u",
        ] {
            assert!(col(&analysis(sql), "n").1, "the scalar subquery may find no row: {sql}");
        }
        assert!(!col(&analysis("select (select count(p.id) from posts p where 0 limit 5) as n from users u"), "n").1);
        assert!(!col(&analysis("select (select s.n from (select count(p.id) as n from posts p where 0) s) as n from users u"), "n").1);
        // SQLite reads a VALUES of several rows as a FROM item, which does not say how many.
        assert_eq!(rows("values (1), (2)"), "many");
        assert_eq!(rows("select v.column1 from (values (1), (2)) v"), "many");
        assert_eq!(rows("select a.n, v.column1 from (select count(*) as n from users) a, (values (1), (2)) v"), "many");
    }

    #[test]
    fn cardinality_by_keys() {
        assert_eq!(analysis("select id from users where email = :email").cardinality, "maybeOne");
        assert_eq!(analysis("select id from users where name = :name").cardinality, "many");
        assert_eq!(analysis("select u.id, count(p.id) as n from users u left join posts p on p.user_id = u.id where u.id = :id group by u.id").cardinality, "maybeOne");
        assert_eq!(analysis("select id from users limit 1").cardinality, "maybeOne");
        assert_eq!(analysis("insert or ignore into users (email, name) values (:e, :n) returning id").cardinality, "maybeOne");
        assert_eq!(analysis("insert into users (email, name) values (:e, :n)").cardinality, "none");
        assert_eq!(analysis("delete from users where id = :id returning id").cardinality, "maybeOne");
        assert_eq!(analysis("select id from users union all select user_id from posts").cardinality, "many");
    }

    /// The whole path, schema query included, as the host drives it.
    fn analyzed(sql: &str) -> Analysis {
        let ddl = [
            "create table users (id integer primary key, name text not null, email text unique, score real, avatar blob, created_at text not null default current_timestamp)",
        ];
        let rows = ddl.iter().map(|d| vec![serde_json::Value::String(d.to_string())]).collect();
        let answers = HashMap::from([(format!("query:{DDL_QUERY}|[]"), crate::api::Answer::Rows { rows }), (format!("query:{FUNCTIONS_QUERY}|[]"), functions_answer(&[]))]);
        let host = Host::new(&answers);
        analyze(sql, &lex::lex(sql, Dialect::Sqlite).unwrap(), &host).expect("the schema and the functions were answered")
    }

    /// What a SQLite with none of the optional families lists, as [`FUNCTIONS_QUERY`]'s rows: the
    /// functions the tests call, and `extra` on top.
    fn listed(extra: &[(&str, i32, char)]) -> Vec<Vec<serde_json::Value>> {
        let core = [("count", 0, 'w'), ("count", 1, 'w'), ("max", 1, 'w'), ("max", -3, 's'), ("upper", 1, 's'), ("lower", 1, 's'), ("like", 2, 's'), ("like", 3, 's'), ("glob", 2, 's')];
        let more = [("coalesce", -4, 's'), ("iif", -4, 's'), ("json_extract", -1, 's'), ("->>", 2, 's'), ("current_timestamp", 0, 's'), ("row_number", 0, 'w'), ("group_concat", 1, 'w'), ("abs", 1, 's')];
        core.iter().chain(more.iter()).chain(extra.iter()).map(|(name, narg, kind)| vec![serde_json::json!(name), serde_json::json!(narg), serde_json::json!(kind.to_string())]).collect()
    }

    fn functions_answer(extra: &[(&str, i32, char)]) -> crate::api::Answer {
        crate::api::Answer::Rows { rows: listed(extra) }
    }

    /// The direct entry on a host whose SQLite lists [`listed`]'s functions.
    fn on_host(sql: &str, schema: &[String], extra: &[(&str, i32, char)], total: Vec<TotalFunction>) -> Analysis {
        analyze_ddl(sql, schema, &Extras { total, host: host_functions(&listed(extra)) }).analysis
    }

    #[test]
    fn names_fold_ascii_case_as_sqlite_folds_them() {
        let schema: Vec<String> = [
            "create table Books (id integer primary key, Title text not null, ISBN text unique, note text)",
            "create table \"\u{c9}t\u{e9}\" (id integer primary key, v text)",
            "create table wr (K text primary key, v text) without rowid",
            "create table rt (RowId integer, v text)",
            "create table \"my table\" (id integer primary key, v text)",
            "create trigger \"skip one\" before insert on \"BOOKS\" begin select raise(ignore) where new.title = 'x'; end",
            "create trigger skip_two before insert on \"my table\" begin select raise(ignore) where new.v = 'x'; end",
            "create trigger [later] after update of \"on\", v on main . [my table] begin select 1; end",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let a = |sql: &str| analyze_ddl(sql, &schema, &Extras::default()).analysis;
        // The table as the statement spells it, bare, quoted or qualified, is the table the DDL
        // declares, and its keys are that table's.
        for sql in [
            "select * from books where id = :id",
            "select * from BOOKS where ID = :id",
            "select * from \"books\" where \"ID\" = :id",
            "select * from [BOOKS] b where B.Id = :id",
            "select b.* from main.bOOKS b join Books c on c.ID = b.id where C.id = :id",
            "select title from books where isbn = :i",
            "update BOOKS set TITLE = :t where ID = :id returning Title",
            "delete from books where Isbn = :i returning id",
            "select v from WR where k = :k",
            "select v from Rt where _ROWID_ = :r",
        ] {
            let got = a(sql);
            assert!(got.diagnostics.is_empty(), "{sql}: {:?}", got.diagnostics);
            assert_eq!(got.cardinality, "maybeOne", "{sql}: {:?}", got.cardinality_reason);
        }
        // A declared column named rowid in any case hides the rowid, and a table without a rowid
        // has none to name.
        assert_eq!(a("select v from RT where ROWID = :r").cardinality, "many");
        assert_eq!(a("select v from wr where rowid = 1").diagnostics[0].message, "no such column: rowid");
        // A target column in another case is the declared column: its type, and its NOT NULL.
        let insert = a("insert into BOOKS (TITLE, NOTE) values (:t, :n)");
        assert_eq!((param(&insert, "t"), param(&insert, "n")), ("text".into(), "text".into()));
        assert_eq!(insert.params.iter().map(|p| p.nullable).collect::<Vec<_>>(), [false, true]);
        assert_eq!(col(&a("select TITLE, Note from books"), "Title"), ("text".into(), false));
        // The rowid by any of its names, in any case, is the INTEGER PRIMARY KEY column.
        assert_eq!(param(&a("insert into books (ROWID, title) values (:r, :t)"), "r"), "integer");
        assert_eq!(param(&a("insert into rt (_rowid_, v) values (:r, :v)"), "r"), "unknown");
        // SQLite folds ASCII alone: `été` is another name than `Été`, and no table.
        assert_eq!(a("select v from \"\u{c9}t\u{e9}\" where id = :id").cardinality, "maybeOne");
        assert_eq!(a("select v from \"\u{e9}t\u{e9}\" where id = :id").diagnostics[0].message, "no such table: \u{e9}t\u{e9}");
        // A table-valued function in any case is the function.
        assert_eq!(param(&a("select value from JSON_EACH(:ids)"), "ids"), "text");
        // A trigger's table is read as SQLite reads it, quoted and in another case, and so is a
        // trigger whose name or column list holds a keyword.
        assert_eq!(a("insert into books (title) values (:t) returning id").cardinality, "maybeOne");
        assert_eq!(a("insert into \"My Table\" (v) values (:v) returning id").cardinality, "maybeOne");
        assert_eq!(trigger_header(&schema[5]), Some(("before".into(), "insert".into(), "books".into())));
        assert_eq!(trigger_header(&schema[7]), Some(("after".into(), "update".into(), "my table".into())));
        assert_eq!(trigger_header("CREATE TRIGGER main.\"before\" /* on x */ INSTEAD OF DELETE ON v -- on y\n BEGIN SELECT 1; END"), Some(("instead".into(), "delete".into(), "v".into())));
        assert_eq!(trigger_header("create table trigger_log (\"on\" text)"), None);
    }

    #[test]
    fn every_row_of_a_values_list_types_its_parameters() {
        let a = analysis("insert into users (id, name, email) values (:a, :b, :e), (:c, :d, :e2), (:f, upper(:g), 'x') returning id");
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        let got: Vec<(String, String, bool)> = a.params.iter().map(|p| (p.name.clone(), scalar_name(&p.ty).unwrap_or("unknown").to_string(), p.nullable)).collect();
        let want = [("a", "integer", false), ("b", "text", false), ("e", "text", true), ("c", "integer", false), ("d", "text", false), ("e2", "text", true), ("f", "integer", false), ("g", "text", false)];
        assert_eq!(got, want.map(|(n, t, null)| (n.to_string(), t.to_string(), null)));
        assert_eq!((a.cardinality.as_str(), a.cardinality_reason.as_deref()), ("many", Some("a VALUES list of 3 rows returns 3 rows")));
        // A parameter a row does not write whole cannot be null. Each parameter appears once, so
        // one cannot be written to a nullable column in one row and a NOT NULL one in another.
        let a = analysis("insert into users (name, email) values ('a', :x), (:w, 'b'), ('c', lower(:y))");
        assert_eq!(a.params.iter().map(|p| p.nullable).collect::<Vec<_>>(), [true, false, false]);
        assert_eq!(a.cardinality, "none");
        let a = analysis("insert into users (name, email) values ('a', :x), (:x, 'b')");
        assert_eq!(a.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [3]);
        // Without a column list the rows fill the columns in order; under a WITH too.
        let a = analysis("with w as (select 1) insert into posts values (:id, :u, :t, :b), (:id2, :u2, :t2, :b2)");
        assert_eq!(a.params.iter().map(|p| (scalar_name(&p.ty).unwrap_or("unknown"), p.nullable)).collect::<Vec<_>>(), [("integer", false), ("integer", false), ("text", false), ("text", true)].repeat(2));
        // A list of any length: SQLite keeps a row per arm, and the tree nests none of them.
        let rows: Vec<String> = (0..3000).map(|i| format!("(:n{i}, :e{i}, {i})")).collect();
        let a = analysis(&format!("insert into users (name, email, score) values {}", rows.join(", ")));
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!((a.params.len(), param(&a, "n2999"), param(&a, "e2999")), (6000, "text".into(), "text".into()));
        // A VALUES list on its own: a row per row, a column of the type its rows share, NULL where
        // a row has one, and a parameter typed by the rows beside it.
        let a = analysis("values (1, 'a', :p), (2, null, 3), (:q, 'c', 4.5)");
        assert_eq!((col(&a, "column1"), col(&a, "column2")), (("integer".into(), false), ("text".into(), true)));
        assert_eq!((param(&a, "p"), param(&a, "q"), a.cardinality.as_str()), ("real".into(), "integer".into(), "many"));
        assert_eq!(analysis("values (1, 'a')").cardinality, "one");
        assert_eq!(col(&analysis("values (1), ('a')"), "column1").0, "unknown");
        assert_eq!(analysis("select v.column1 from (values (1), (2)) v").cardinality, "many");
        assert_eq!(analysis("select 1 union all values (2), (3)").cardinality, "many");
        assert_eq!(col(&analysis("select (values (1), (2)) as x"), "x"), ("integer".into(), true));
    }

    #[test]
    fn a_collate_keeps_its_operands_context() {
        let count = |sql: &str| {
            let mut schema = ddl();
            schema.push("create table c (id integer primary key, b text unique, n text unique collate nocase)".into());
            let a = analyze_ddl(sql, &schema, &Extras::default()).analysis;
            assert!(a.diagnostics.is_empty(), "{sql}: {:?}", a.diagnostics);
            (param(&a, "v"), a.cardinality)
        };
        assert_eq!(count("select id from users where name = :v collate nocase"), ("text".into(), "many".into()));
        assert_eq!(count("select id from users where :v collate nocase = name"), ("text".into(), "many".into()));
        assert_eq!(count("select id from users where name collate nocase = :v"), ("text".into(), "many".into()));
        assert_eq!(count("select id from users where id = :v collate binary"), ("integer".into(), "many".into()));
        assert_eq!(count("select id from users where name in (:v collate nocase, 'a')"), ("text".into(), "many".into()));
        // The type is the column's; the key is not the comparison's: a BINARY unique index tells
        // 'a' from 'A', and NOCASE meets both, on either side of the comparison.
        for sql in ["select id from c where b = :v collate nocase", "select id from c where :v collate nocase = b", "select id from c where b collate nocase = :v", "delete from c where b = :v collate nocase returning id"] {
            assert_eq!(count(sql), ("text".into(), "many".into()), "{sql}");
        }
        // A key under the comparison's own collation still bounds it, with no COLLATE written.
        assert_eq!(count("select id from c where n = :v"), ("text".into(), "maybeOne".into()));
        assert_eq!(count("select id from c where b = :v"), ("text".into(), "maybeOne".into()));
    }

    #[test]
    fn the_select_that_feeds_an_insert_types_its_parameters() {
        let schema = vec!["create table bar (name text not null, ready bool not null, n integer)".to_string(), "create table foo (name text not null, meta text, k integer)".to_string(), "create table foo2 (name text not null, meta text, k integer)".to_string()];
        let a = |sql: &str| {
            let a = analyze_ddl(sql, &schema, &Extras::default()).analysis;
            assert!(a.diagnostics.is_empty(), "{sql}: {:?}", a.diagnostics);
            a
        };
        let params = |a: &Analysis| a.params.iter().map(|p| (scalar_name(&p.ty).unwrap_or("unknown").to_string(), p.nullable)).collect::<Vec<_>>();
        let typed = |types: &[&str]| types.iter().map(|t| (t.to_string(), false)).collect::<Vec<_>>();
        // A select-list parameter takes its target column's type; one the WHERE compares, the
        // column's: `bool` is a declared type of NUMERIC affinity, a number or a text.
        // A bare one written to a column that may hold NULL takes NULL, as in a VALUES row.
        let got = a("insert into foo (name, meta) select name, ? from bar where ready = ?");
        assert_eq!((got.kind.as_str(), got.cardinality.as_str()), ("insert", "none"));
        assert_eq!(params(&got), [("text".to_string(), true), ("numeric".to_string(), false)]);
        assert_eq!(params(&a("select name from bar where ready = ?")), typed(&["numeric"]));
        // Without a column list, through an expression, in every arm of a compound, and under a
        // LIMIT; only a bare column of the one SELECT is a value written whole.
        assert_eq!(params(&a("insert into foo select :n, :m, :k from bar")), [("text".to_string(), false), ("text".to_string(), true), ("integer".to_string(), true)]);
        assert_eq!(params(&a("insert into foo (k, name) select n + :d, upper(:u) from bar limit :l")), typed(&["integer", "text", "integer"]));
        assert_eq!(params(&a("insert into foo (name, k) select name, :a from bar union all select :b, n from bar")), typed(&["integer", "text"]));
        let returning = a("insert into foo (name, meta) select name, :m from bar where n > :n returning name, meta");
        assert_eq!((params(&returning), returning.cardinality.as_str()), (vec![("text".to_string(), true), ("integer".to_string(), false)], "many"));
        // A subquery elsewhere in the statement is not the INSERT's source: one in RETURNING
        // stands in no typed place, and one in a VALUES row is the value written to its own
        // column (k, not the first target column name), as any scalar subquery takes the type of
        // the place it stands in.
        assert_eq!(params(&a("insert into foo (name, meta) select name, 'x' from bar returning (select :p) as p")), typed(&["unknown"]));
        assert_eq!(params(&a("insert into foo (name, k) values ('a', (select :p))")), typed(&["integer"]));
        // A whole-table copy, which SQLite runs without a SELECT, is still an INSERT.
        let copy = a("insert into foo2 select * from foo");
        assert_eq!((copy.kind.as_str(), copy.cardinality.as_str()), ("insert", "none"));
    }

    #[test]
    fn the_hosts_sqlite_decides_which_functions_exist() {
        // The embedded SQLite has the math functions and `soundex`; a host may have neither.
        let sql = "select id, ACOS(score) as a from users where name = :n";
        let a = on_host(sql, &ddl(), &[], Vec::new());
        assert_eq!(a.diagnostics.len(), 1, "{:?}", a.diagnostics);
        assert_eq!(a.diagnostics[0].message, "no such function: ACOS on this database's SQLite");
        assert_eq!((a.diagnostics[0].start, a.diagnostics[0].length, a.diagnostics[0].code), (sql.find("ACOS").unwrap(), 4, 6));
        assert!(a.columns.is_empty());
        let a = on_host(sql, &ddl(), &[("acos", 1, 's')], Vec::new());
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(col(&a, "a"), ("real".into(), true));
        let a = on_host("select soundex(name) as s from users where soundex(:x) = 'A000'", &ddl(), &[("SOUNDEX", 1, 's')], Vec::new());
        assert_eq!((col(&a, "s"), param(&a, "x")), (("text".into(), false), "text".into()));
        assert_eq!(on_host("select soundex(name) as s from users", &ddl(), &[], Vec::new()).diagnostics[0].message, "no such function: soundex on this database's SQLite");
        // A host that cannot list its functions keeps every function the embedded SQLite has.
        assert_eq!(col(&analysis("select acos(score) as a, soundex(name) as s from users"), "a"), ("real".into(), true));
        // What every SQLite has stays: operators that are functions underneath, keywords, `*`.
        let a = on_host("select count(*) as n, max(name) as m, coalesce(email, 'x') as e, current_timestamp as t, email ->> '$.a' as j, row_number() over (order by id) as r from users where name like :p and email glob 'a*' and iif(id > 1, true, false)", &ddl(), &[], Vec::new());
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        // A call inside a window definition, a subquery, a WITH query and an ORDER BY is a call.
        for sql in [
            "select row_number() over (order by sqrt(score)) as r from users",
            "select id from users where id in (select id from users where sqrt(score) > 1)",
            "with w as (select sqrt(score) as s from users) select s from w",
            "select id from users order by sqrt(score)",
            "update users set score = sqrt(score) where id = :id",
            "insert into users (name, score) values (:n, sqrt(:s))",
            "delete from users where id = :id returning sqrt(score) as s",
        ] {
            let a = on_host(sql, &ddl(), &[], Vec::new());
            assert_eq!(a.diagnostics.first().map(|d| (d.message.as_str(), d.start)), Some(("no such function: sqrt on this database's SQLite", sql.find("sqrt").unwrap())), "{sql}");
        }
        // A view that calls one fails every statement that reads it, with no place in the
        // statement to point at; the replay itself keeps the view.
        let mut schema = ddl();
        schema.push("create view roots as select id, sqrt(score) as root from users".into());
        let sql = "select root from roots";
        let a = on_host(sql, &schema, &[], Vec::new());
        assert_eq!(a.diagnostics.len(), 1, "{:?}", a.diagnostics);
        assert_eq!((a.diagnostics[0].message.as_str(), a.diagnostics[0].start, a.diagnostics[0].length), ("no such function: sqrt on this database's SQLite (the view roots calls it)", 0, sql.len()));
        assert_eq!(col(&on_host(sql, &schema, &[("sqrt", 1, 's')], Vec::new()), "root"), ("real".into(), true));
        assert!(on_host("select id from users", &schema, &[], Vec::new()).diagnostics.is_empty());
        // A function the host has and the embedded SQLite lacks prepares, with no type; an
        // aggregate is one row, and a window function takes an OVER.
        let extra = [("host_only", 2, 's'), ("host_only", 3, 's'), ("host_agg", 1, 'a'), ("host_win", -1, 'w')];
        let a = on_host("select host_only(name, 1) as two, host_only(name, 1, 2) as three from users where id = :id", &ddl(), &extra, Vec::new());
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!((col(&a, "two"), col(&a, "three"), a.cardinality.as_str()), (("unknown".into(), true), ("unknown".into(), true), "maybeOne"));
        // It may change from row to row, so it pins no key.
        assert_eq!(on_host("select name from users where id = host_only(1, 2)", &ddl(), &extra, Vec::new()).cardinality, "many");
        let a = on_host("select host_agg(score) as total from users", &ddl(), &extra, Vec::new());
        assert_eq!((col(&a, "total"), a.cardinality.as_str()), (("unknown".into(), true), "one"));
        let a = on_host("select host_win(score, 1) over (partition by name) as w, host_win(id) as plain from users", &ddl(), &extra, Vec::new());
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(on_host("select host_only(name) from users", &ddl(), &extra, Vec::new()).diagnostics[0].message, "wrong number of arguments to function host_only()");
        // A function the host registers on its connection is one it has, listed or not.
        let html = vec![TotalFunction { name: "reylo_html".into(), arity: 1, returns: Affinity::Text }];
        assert_eq!(col(&on_host("select reylo_html(name) as h from users", &ddl(), &[], html), "h"), ("text".into(), false));
        // No list at all is no list: every SQLite has functions.
        assert_eq!(host_functions(&[]), None);
        assert_eq!(host_functions(&[vec![serde_json::json!("abs"), serde_json::json!("1"), serde_json::json!("s")]]), Some(vec![HostFunction { name: "abs".into(), arity: 1, kind: 's' }]));
    }

    #[test]
    fn the_replay_asks_the_host_for_its_functions_with_the_schema() {
        let sql = "select acos(score) as a from users";
        let lexed = lex::lex(sql, Dialect::Sqlite).unwrap();
        let schema = crate::api::Answer::Rows { rows: ddl().iter().map(|d| vec![serde_json::Value::String(d.clone())]).collect() };
        let ask = |answers: &HashMap<String, crate::api::Answer>| {
            let host = Host::new(answers);
            let analysis = analyze(sql, &lexed, &host);
            let needs: Vec<String> = host
                .needs()
                .iter()
                .map(|n| match n {
                    crate::api::Need::Query { sql, scope, .. } => format!("{sql} {scope:?}"),
                    crate::api::Need::Describe { sql, .. } => format!("describe {sql}"),
                })
                .collect();
            (analysis, needs)
        };
        // One round asks both, for the session.
        let mut answers = HashMap::new();
        let (analysis, needs) = ask(&answers);
        assert!(analysis.is_none());
        assert_eq!(needs, [format!("{DDL_QUERY} Session"), format!("{FUNCTIONS_QUERY} Session")]);
        answers.insert(format!("query:{DDL_QUERY}|[]"), schema);
        assert_eq!(ask(&answers).1, [format!("{FUNCTIONS_QUERY} Session")]);
        // The host's list decides.
        answers.insert(format!("query:{FUNCTIONS_QUERY}|[]"), functions_answer(&[]));
        let (analysis, needs) = ask(&answers);
        assert!(needs.is_empty());
        assert_eq!(analysis.unwrap().diagnostics[0].message, "no such function: acos on this database's SQLite");
        answers.insert(format!("query:{FUNCTIONS_QUERY}|[]"), functions_answer(&[("acos", 1, 's')]));
        assert!(ask(&answers).0.unwrap().diagnostics.is_empty());
        // A host whose SQLite refuses the question, or answers nothing, keeps the embedded one's.
        answers.insert(format!("query:{FUNCTIONS_QUERY}|[]"), crate::api::Answer::Error { error: crate::api::DbError { message: "not authorized".into(), code: None, position: None } });
        assert_eq!(col(&ask(&answers).0.unwrap(), "a"), ("real".into(), true));
        answers.insert(format!("query:{FUNCTIONS_QUERY}|[]"), crate::api::Answer::Rows { rows: Vec::new() });
        assert_eq!(col(&ask(&answers).0.unwrap(), "a"), ("real".into(), true));
    }

    #[test]
    fn a_table_in_an_attached_database_is_named_as_one() {
        let message = |schema: &str| format!("no such table: {schema}.users: the database has no schema `{schema}`. A database the application attaches at run time (ATTACH ... AS {schema}) is not in the file the type check reads, so a table in it cannot be typed");
        for (sql, reference) in [
            ("select id, name from baz.users where id = :id", "baz.users"),
            ("select u.id from \"baz\".[users] u", "\"baz\".[users]"),
            ("select 1 from users join /* x */ baz . users b on b.id = users.id", "baz . users"),
            ("insert into baz.users (id) values (:id)", "baz.users"),
            ("update baz.users set name = :n where id = :id", "baz.users"),
            ("delete from baz.users where id = :id", "baz.users"),
        ] {
            let a = analysis(sql);
            assert_eq!(a.diagnostics.len(), 1, "{sql}: {:?}", a.diagnostics);
            assert_eq!(a.diagnostics[0].message, message("baz"), "{sql}");
            assert_eq!((a.diagnostics[0].start, a.diagnostics[0].length), (sql.find(reference).unwrap(), reference.len()), "{sql}");
        }
        // The position survives the placeholder rewrite.
        let sql = "select :auth.user_id as u, id from baz.users";
        assert_eq!(analysis(sql).diagnostics[0].start, sql.find("baz").unwrap());
        // A schema the database has, and a name that only holds a dot, are SQLite's own message.
        assert_eq!(analysis("select id from main.nope").diagnostics[0].message, "no such table: main.nope");
        assert_eq!(analysis("select id from temp.nope").diagnostics[0].message, "no such table: temp.nope");
        assert_eq!(analysis("select id from \"baz.users\"").diagnostics[0].message, "no such table: baz.users");
        assert_eq!(analysis("select id from nope").diagnostics[0].message, "no such table: nope");
        assert!(analysis("select main.users.id from main.users").diagnostics.is_empty());
    }

    #[test]
    fn sqlites_own_tables_replay_without_a_note() {
        // `sqlite_schema` lists SQLite's own tables beside the application's: the sequence table
        // an AUTOINCREMENT makes, the statistics ANALYZE writes, a virtual table's shadow tables.
        let schema: Vec<String> = [
            "CREATE TABLE t (id integer primary key autoincrement, v text)",
            "CREATE TABLE sqlite_sequence(name,seq)",
            "CREATE TABLE sqlite_stat1(tbl,idx,stat)",
            "CREATE VIRTUAL TABLE ft USING fts5(body)",
            "CREATE TABLE 'ft_data'(id INTEGER PRIMARY KEY, block BLOB)",
            "CREATE TABLE 'ft_idx'(segid, term, pgno, PRIMARY KEY(segid, term)) WITHOUT ROWID",
            "CREATE TABLE 'ft_content'(id INTEGER PRIMARY KEY, c0)",
            "CREATE TABLE 'ft_docsize'(id INTEGER PRIMARY KEY, sz BLOB)",
            "CREATE TABLE 'ft_config'(k PRIMARY KEY, v) WITHOUT ROWID",
            "CREATE VIRTUAL TABLE rt USING rtree(id, minx, maxx)",
            "CREATE TABLE \"rt_rowid\"(rowid INTEGER PRIMARY KEY,nodeno)",
            "CREATE TABLE \"rt_node\"(nodeno INTEGER PRIMARY KEY,data)",
            "CREATE TABLE \"rt_parent\"(nodeno INTEGER PRIMARY KEY,parentnode)",
            "CREATE VIRTUAL TABLE f4 USING fts4(body)",
            "CREATE TABLE 'f4_content'(docid INTEGER PRIMARY KEY, 'c0body')",
            "CREATE TABLE 'f4_segments'(blockid INTEGER PRIMARY KEY, block BLOB)",
            "CREATE TABLE 'f4_segdir'(level INTEGER,idx INTEGER,start_block INTEGER,leaves_end_block INTEGER,end_block INTEGER,root BLOB,PRIMARY KEY(level, idx))",
            "CREATE TABLE 'f4_docsize'(docid INTEGER PRIMARY KEY, size BLOB)",
            "CREATE TABLE 'f4_stat'(id INTEGER PRIMARY KEY, value BLOB)",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let a = |sql: &str, schema: &[String]| analyze_ddl(sql, schema, &Extras::default()).analysis;
        // An error in the statement is the only thing reported.
        let failed = a("select nope from t", &schema);
        assert_eq!(failed.diagnostics.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(), ["no such column: nope"]);
        for sql in [
            "select name, seq from sqlite_sequence where name = :n",
            "update sqlite_sequence set seq = :s where name = :n",
            "select tbl, idx, stat from sqlite_stat1",
            "select name, sql from sqlite_schema where type = 'table'",
            "select name from sqlite_master",
            "select rowid, body from ft where ft match :q",
            "select id from rt where minx > :x",
            "select docid from f4 where f4 match :q",
            "select block from ft_data where id = :id",
            "insert into t (v) values (:v) returning id",
        ] {
            let got = a(sql, &schema);
            assert!(got.diagnostics.is_empty(), "{sql}: {:?}", got.diagnostics);
        }
        assert_eq!(a("insert into t (v) values (:v) returning id", &schema).cardinality, "one");
        // The schema table itself stays read-only, as on the host.
        assert_eq!(a("delete from sqlite_schema", &schema).diagnostics[0].message, "table sqlite_master may not be modified");
        // A sequence table that outlived its AUTOINCREMENT table, before one made later.
        let later = vec!["CREATE TABLE sqlite_sequence(name,seq)".to_string(), "CREATE TABLE u (id integer primary key autoincrement, v text)".to_string()];
        assert!(a("select seq from sqlite_sequence", &later).diagnostics.is_empty());
        assert_eq!(a("insert into u (v) values (:v) returning id", &later).cardinality, "one");
        assert_eq!(a("select nope from u", &later).diagnostics.len(), 1);
        // A step that really fails is still named beside the statement's error.
        let broken = vec!["CREATE TABLE t (id integer primary key)".to_string(), "CREATE TABLE t (id integer primary key)".to_string(), "CREATE VIEW v AS SELECT".to_string()];
        let notes: Vec<String> = a("select nope from t", &broken).diagnostics.iter().map(|d| d.message.clone()).collect();
        assert_eq!(notes, ["no such column: nope", "The schema replay skipped statement 2: table t already exists", "The schema replay skipped statement 3: incomplete input"]);
    }

    #[test]
    fn a_parameter_as_a_whole_sort_or_group_key_is_an_error_at_its_place() {
        // Through the protocol's whole path, which compiles a `::type` cast to SQLite's own.
        let full = |sql: &str| {
            let ddl = ["create table users (id integer primary key, name text not null, email text unique, score real, avatar blob, created_at text not null default current_timestamp)"];
            let rows = ddl.iter().map(|d| vec![serde_json::Value::String(d.to_string())]).collect();
            let answers = HashMap::from([(format!("query:{DDL_QUERY}|[]"), crate::api::Answer::Rows { rows }), (format!("query:{FUNCTIONS_QUERY}|[]"), functions_answer(&[]))]);
            let host = Host::new(&answers);
            <crate::api::Sqlite as reylo_engine_core::database::Database>::analyze(sql, &host).expect("answered")
        };
        let keys = |sql: &str| full(sql).diagnostics.iter().filter(|d| d.code == 17).map(|d| (d.start, d.length, d.message.clone())).collect::<Vec<_>>();
        let at = |sql: &str, written: &str| sql.find(written).unwrap();
        // A `::type` cast is part of the parameter's place, as on every diagnostic at a cast one.
        for (sql, length) in [("select id from users order by :sort", 5), ("select id from users order by :sort::text desc", 11), ("select id from users order by cast(:sort as text)", 5), ("select id from users order by :sort collate nocase", 5)] {
            let found = keys(sql);
            assert_eq!(found.iter().map(|(s, l, _)| (*s, *l)).collect::<Vec<_>>(), [(at(sql, ":sort"), length)], "{sql}");
            assert_eq!(found[0].2, "order by :sort sorts every row by the same value, since a parameter is a value and not a column's name; choose the column with case :sort when 'name' then name end");
        }
        for (sql, written) in [("select id from users order by ?", "?"), ("select id from users order by ?2", "?2"), ("select id from users order by @s", "@s"), ("select id from users order by $s", "$s")] {
            let found = keys(sql);
            assert_eq!(found.iter().map(|(s, l, _)| (*s, *l)).collect::<Vec<_>>(), [(at(sql, written), written.len())], "{sql}");
            assert!(found[0].2.starts_with(&format!("order by {written} sorts every row")), "{sql}");
        }
        assert_eq!(keys("select id from users order by ?2, ?1").iter().map(|(s, ..)| *s).collect::<Vec<_>>(), [30, 34]);
        assert_eq!(keys("select id from users order by name, ?").len(), 1);
        // GROUP BY, alone and beside another key.
        assert_eq!(keys("select count(*) as n from users group by :g")[0].2, "group by :g puts every row in one group, since a parameter is a value and not a column's name; choose the column with case :g when 'name' then name end");
        assert!(keys("select count(*) as n from users group by name, :g::text")[0].2.starts_with("group by :g groups every row by the same value"));
        // A window's keys, a named window's, and an aggregate's ORDER BY.
        let sql = "select row_number() over (partition by :p order by :o) as n from users";
        let found = keys(sql);
        assert_eq!(found.iter().map(|(s, _, m)| (*s, m.split(',').next().unwrap().to_string())).collect::<Vec<_>>(), [(at(sql, ":p"), "partition by :p puts every row in one partition".to_string()), (at(sql, ":o"), "order by :o sorts every row by the same value".to_string())]);
        assert_eq!(keys("select row_number() over w as n from users window w as (order by :o)").len(), 1);
        assert_eq!(keys("select group_concat(name, ',' order by :o) as names from users").len(), 1);
        // In a subquery, a WITH query (read twice, reported once), an expression's subquery and a
        // set operation's ORDER BY.
        assert_eq!(keys("select id from (select id from users order by :s limit 3) t").len(), 1);
        assert_eq!(keys("with t as (select name from users group by :g, name) select a.name from t a, t b").len(), 1);
        assert_eq!(keys("select id from users where id in (select id from users order by :s limit 3)").len(), 1);
        // SQLite itself refuses a compound's ORDER BY term that names no column of its result.
        let refused = full("select id from users union select id from users order by :s");
        assert_eq!(refused.diagnostics.iter().map(|d| (d.code, d.message.as_str())).collect::<Vec<_>>(), [(6, "1st ORDER BY term does not match any column in the result set")]);
        assert_eq!(keys("insert into users (name) select name from users order by :s").len(), 1);
        // A key that holds a parameter chooses a column with it; a position and a LIMIT are none,
        // and so is a position or an alias of a result column that is a parameter, which SQLite
        // resolves to a copy of the parameter.
        for sql in [
            "select id, :tag as tag from users order by 2",
            "select id, :tag as tag from users order by tag",
            "select :tenant as tenant, name, count(*) as n from users group by 1, 2",
            "select id, name from users order by case :sort when 'name' then name end",
            "select id from users order by case when :desc then id end desc, id",
            "select id from users order by 1 limit :n",
            "select id from users order by lower(:x) || name",
        ] {
            assert_eq!(keys(sql), [], "{sql}");
        }
        // The rest of the statement is typed all the same.
        let a = full("select id from users order by :sort::text");
        assert_eq!(a.columns.len(), 1);
        assert_eq!(a.params[0].type_name, "text");
    }

    #[test]
    fn prepares_the_numbered_rewrite() {
        // SQLite's own `:auth` would end at the dot; the rewrite keeps the whole name one parameter.
        let a = analyzed("select name from users where email = :auth.email and id = :auth.id and score > :auth.email");
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(param(&a, "auth.email"), "text");
        assert_eq!(param(&a, "auth.id"), "integer");
        assert_eq!(a.rewritten.sql, "select name from users where email = ? and id = ? and score > ?");
        assert_eq!(a.rewritten.order, ["auth.email", "auth.id", "auth.email"]);
        // Positional forms: SQLite's `?3` is the parameter `3`, and a bare `?` the next number.
        assert_eq!(param(&analyzed("select name from users where id = ?3"), "3"), "integer");
        assert_eq!(param(&analyzed("select name from users where email = ?"), "1"), "text");
        // An error SQLite reports in the rewrite lands on the author's text.
        let sql = "select :auth.user_id, nope from users";
        let a = analyzed(sql);
        assert_eq!(a.diagnostics.len(), 1, "{:?}", a.diagnostics);
        assert_eq!((a.diagnostics[0].start, a.diagnostics[0].length), (sql.find("nope").unwrap(), 4));
    }

    #[test]
    fn a_column_declared_without_a_type_holds_any_value() {
        let ddl = vec!["CREATE TABLE t (a, b TEXT, c ANY)".to_string(), "CREATE TABLE s (x ANY, y INTEGER) STRICT".to_string()];
        let r = analyze_ddl("SELECT t.a, t.b, s.x, s.y FROM t, s WHERE t.a = :a AND s.x = :x", &ddl, &Extras::default());
        let a = &r.analysis;
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        // `t.a = :a` and `s.x = :x` keep NULL out of the rows they return.
        assert_eq!(col(a, "a"), ("unknown".into(), false));
        assert_eq!(col(a, "b"), ("text".into(), true));
        assert_eq!(col(a, "x"), ("unknown".into(), false));
        assert_eq!(col(a, "y"), ("integer".into(), true));
        assert_eq!((param(a, "a"), param(a, "x")), ("unknown".into(), "unknown".into()));
        // In a table that is not STRICT, `ANY` is a declared type with NUMERIC affinity, not a wildcard.
        let r = analyze_ddl("SELECT c FROM t", &ddl, &Extras::default());
        assert_eq!(col(&r.analysis, "c"), ("numeric".into(), true));
    }

    #[test]
    fn a_value_of_one_of_several_types_is_unknown() {
        let ddl = vec![
            "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER NOT NULL, s TEXT, doc TEXT NOT NULL) STRICT".to_string(),
            "CREATE TABLE u (x)".to_string(),
            "CREATE VIEW w AS SELECT 'x' AS v UNION ALL SELECT n FROM t".to_string(),
        ];
        let ty = |sql: &str| {
            let r = analyze_ddl(sql, &ddl, &Extras::default());
            assert!(r.analysis.diagnostics.is_empty(), "{sql}: {:?}", r.analysis.diagnostics);
            col(&r.analysis, "v").0
        };
        // A compound's column is any arm's value: SQLite names the leftmost arm's declared type,
        // which is not every arm's.
        assert_eq!(ty("SELECT n AS v FROM t UNION ALL SELECT 'x'"), "unknown");
        assert_eq!(ty("SELECT 'x' AS v UNION ALL SELECT n FROM t"), "unknown");
        assert_eq!(ty("SELECT n AS v FROM t UNION ALL SELECT x FROM u"), "unknown");
        assert_eq!(ty("SELECT n AS v FROM t UNION ALL SELECT json_extract(doc, '$.a') FROM t"), "unknown");
        assert_eq!(ty("SELECT n AS v FROM t UNION ALL SELECT id FROM t"), "integer");
        assert_eq!(ty("SELECT n AS v FROM t UNION ALL SELECT NULL"), "integer");
        // Read through a subquery, a view or a WITH query, for which SQLite names the rightmost arm's.
        assert_eq!(ty("SELECT v FROM (SELECT 'x' AS v UNION ALL SELECT n FROM t)"), "unknown");
        assert_eq!(ty("SELECT v FROM w"), "unknown");
        assert_eq!(ty("WITH c(v) AS (SELECT 'x' UNION ALL SELECT n FROM t) SELECT v FROM c"), "unknown");
        // A parameter arm takes the others' type.
        let r = analyze_ddl("SELECT n AS v FROM t UNION ALL SELECT :p", &ddl, &Extras::default());
        assert_eq!((col(&r.analysis, "v").0, param(&r.analysis, "p")), ("integer".into(), "integer".into()));
        // So with `coalesce`, `CASE`, `iif` and `min`: a value of no known type may be any.
        assert_eq!(ty("SELECT coalesce(s, json_extract(doc, '$.a')) AS v FROM t"), "unknown");
        assert_eq!(ty("SELECT coalesce(s, x) AS v FROM t, u"), "unknown");
        assert_eq!(ty("SELECT CASE WHEN n > 0 THEN n ELSE x END AS v FROM t, u"), "unknown");
        assert_eq!(ty("SELECT iif(n > 0, n, json_extract(doc, '$.a')) AS v FROM t"), "unknown");
        assert_eq!(ty("SELECT min(n, x) AS v FROM t, u"), "unknown");
        assert_eq!(ty("SELECT coalesce(s, 'none') AS v FROM t"), "text");
        assert_eq!(ty("SELECT CASE WHEN n > 0 THEN n END AS v FROM t"), "integer");
        // `lag` and `lead` hand back their default where the row they read is missing.
        assert_eq!(ty("SELECT lead(n, 1, 'z') OVER (ORDER BY id) AS v FROM t"), "unknown");
        assert_eq!(ty("SELECT lag(s, 1, x'00') OVER (ORDER BY id) AS v FROM t"), "unknown");
        assert_eq!(ty("SELECT lag(n, 1, 0) OVER (ORDER BY id) AS v FROM t"), "integer");
        assert_eq!(ty("SELECT lag(n, 1, NULL) OVER (ORDER BY id) AS v FROM t"), "integer");
        assert_eq!(ty("SELECT lag(n) OVER (ORDER BY id) AS v FROM t"), "integer");
        // A parameter as the value or the default takes the other's type.
        for sql in ["SELECT lag(n, 1, :p) OVER (ORDER BY id) AS v FROM t", "SELECT lag(:p, 1, n) OVER (ORDER BY id) AS v FROM t"] {
            let r = analyze_ddl(sql, &ddl, &Extras::default());
            assert_eq!((col(&r.analysis, "v").0, param(&r.analysis, "p")), ("integer".into(), "integer".into()), "{sql}");
        }
    }

    #[test]
    fn the_direct_entry_reads_a_schema_it_is_handed() {
        let ddl = vec![
            "CREATE TABLE todos (id uuid PRIMARY KEY NOT NULL, owner_id text NOT NULL, org_id uuid, title varchar(200) NOT NULL, done boolean NOT NULL DEFAULT 0, UNIQUE (owner_id, title))".to_string(),
            "CREATE INDEX todos_org ON todos (org_id)".to_string(),
        ];
        let sql = "SELECT t.id, t.title, t.org_id, t.done, count(*) OVER () AS n FROM todos t WHERE t.owner_id = :auth.user_id AND t.title = :title";
        let r = analyze_ddl(sql, &ddl, &Extras::default());
        let a = &r.analysis;
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(a.kind, "select");
        assert!(r.readonly);
        assert_eq!(col(a, "id"), ("numeric".into(), false));
        assert_eq!(col(a, "title"), ("text".into(), false));
        assert_eq!(col(a, "org_id"), ("numeric".into(), true));
        assert_eq!(col(a, "n"), ("integer".into(), false));
        assert_eq!(r.declared, [Some("uuid".to_string()), Some("varchar(200)".into()), Some("uuid".into()), Some("boolean".into()), None]);
        assert_eq!(param(a, "auth.user_id"), "text");
        assert_eq!(param(a, "title"), "text");
        assert_eq!(a.cardinality, "maybeOne");
        assert_eq!(a.rewritten.sql, "SELECT t.id, t.title, t.org_id, t.done, count(*) OVER () AS n FROM todos t WHERE t.owner_id = ? AND t.title = ?");
        // A write: not read-only, and its RETURNING typed from the target.
        let r = analyze_ddl("INSERT INTO todos (id, owner_id, title) VALUES (:id, :owner, :title) RETURNING id, done", &ddl, &Extras::default());
        assert!(r.analysis.diagnostics.is_empty(), "{:?}", r.analysis.diagnostics);
        assert!(!r.readonly);
        assert_eq!((r.analysis.kind.as_str(), r.analysis.cardinality.as_str()), ("insert", "one"));
        // A column declared BOOLEAN is a boolean to the loader's run time, which refuses a value
        // other than 0 or 1 in it.
        assert_eq!(col(&r.analysis, "done"), ("bool".into(), false));
        // The unique keys of the table it read: the primary key's index and the UNIQUE constraint's.
        assert_eq!(r.unique_keys.len(), 1, "{:?}", r.unique_keys);
        let mut keys = r.unique_keys["todos"].clone();
        keys.sort();
        assert_eq!(keys, [vec!["id".to_string()], vec!["owner_id".into(), "title".into()], vec!["rowid".into()]]);
        // A lex error comes back as the replayed path reports it.
        let r = analyze_ddl("SELECT id FROM todos WHERE id = :a AND title = ?", &ddl, &Extras::default());
        assert_eq!(r.analysis.diagnostics.len(), 1, "{:?}", r.analysis.diagnostics);
        assert!(r.declared.is_empty() && !r.readonly);
    }

    #[test]
    fn the_unique_keys_of_every_table_read() {
        let mut schema = ddl();
        schema.push("create unique index users_lower_name on users (lower(name))".into());
        schema.push("create unique index posts_title on posts (title) where body is not null".into());
        schema.push("create table tags (label text not null, post_id integer not null, primary key (post_id, label)) without rowid".into());
        schema.push("create view titled as select p.id, p.title from posts p".into());
        let r = analyze_ddl("select t.title, (select count(*) from tags g where g.post_id = t.id) as n from titled t where exists (select 1 from users u where u.id = :id)", &schema, &Extras::default());
        assert!(r.analysis.diagnostics.is_empty(), "{:?}", r.analysis.diagnostics);
        let mut tables: Vec<&String> = r.unique_keys.keys().collect();
        tables.sort();
        // The view reads through to `posts`; a view has no keys of its own.
        assert_eq!(tables, ["posts", "tags", "users"]);
        // The INTEGER PRIMARY KEY and the UNIQUE column; an expression index is no key.
        assert_eq!(r.unique_keys["users"], [vec!["id".to_string()], vec!["email".into()]]);
        // A partial index is no key either.
        assert_eq!(r.unique_keys["posts"], [vec!["id".to_string()]]);
        // WITHOUT ROWID: no rowid, only the primary key.
        assert_eq!(r.unique_keys["tags"], [vec!["post_id".to_string(), "label".into()]]);
        // A statement that does not prepare has none.
        assert!(analyze_ddl("select nope from users", &schema, &Extras::default()).unique_keys.is_empty());
    }

    fn html() -> Extras {
        Extras { total: vec![TotalFunction { name: "reylo_html".into(), arity: 1, returns: Affinity::Text }], host: None }
    }

    #[test]
    fn a_host_function_prepares_and_is_typed_from_its_argument() {
        let r = analyze_ddl("select reylo_html(name) as h, reylo_html(email) as e, upper(reylo_html(:q)) as q from users", &ddl(), &html());
        let a = &r.analysis;
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(col(a, "h"), ("text".into(), false));
        assert_eq!(col(a, "e"), ("text".into(), true));
        // The parameter is a non-null value, so the call over it is too.
        assert_eq!(col(a, "q"), ("text".into(), false));
        assert_eq!(r.declared, [None, None, None]);
        // The same call without the registration, and with the wrong argument count: SQLite's own
        // message, at the call.
        let sql = "select id, reylo_html(name) as h from users";
        for (extras, message) in [(Extras::default(), "no such function: reylo_html"), (Extras { total: vec![TotalFunction { name: "reylo_html".into(), arity: 2, returns: Affinity::Text }], host: None }, "wrong number of arguments to function reylo_html()")] {
            let a = analyze_ddl(sql, &ddl(), &extras).analysis;
            assert_eq!(a.diagnostics.len(), 1, "{:?}", a.diagnostics);
            assert_eq!(a.diagnostics[0].message, message);
            assert_eq!((a.diagnostics[0].start, a.diagnostics[0].code), (sql.find("reylo_html").unwrap(), 6));
            assert!(a.columns.is_empty());
        }
        // Any argument count, and a schema that calls the function replays.
        let any = Extras { total: vec![TotalFunction { name: "reylo_html".into(), arity: -1, returns: Affinity::Text }], host: None };
        let mut schema = ddl();
        schema.push("create view safe_users as select id, reylo_html(name) as name from users".into());
        let a = analyze_ddl("select name, reylo_html(name, name) as twice from safe_users", &schema, &any).analysis;
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(col(&a, "name"), ("text".into(), false));
        assert_eq!(col(&a, "twice"), ("text".into(), false));
    }

    #[test]
    fn an_unregistered_function_fails_to_prepare() {
        let sql = "select id, nope(name) from users";
        let a = analysis(sql);
        assert_eq!(a.diagnostics.len(), 1, "{:?}", a.diagnostics);
        assert_eq!(a.diagnostics[0].message, "no such function: nope");
        assert_eq!((a.diagnostics[0].start, a.diagnostics[0].length), (sql.find("nope").unwrap(), 4));
    }

    #[test]
    fn a_scalar_subquery_is_non_null_only_when_it_always_finds_one_row() {
        let a = analysis("select (select count(*) from posts p where p.user_id = u.id) as n, (select max(title) from posts) as top, (select title from posts limit 1) as one, (select count(*) from posts group by user_id) as g, (select 1) as k from users u");
        assert_eq!(col(&a, "n"), ("integer".into(), false));
        assert_eq!(col(&a, "top"), ("text".into(), true));
        assert_eq!(col(&a, "one"), ("text".into(), true));
        assert_eq!(col(&a, "g"), ("integer".into(), true));
        assert_eq!(col(&a, "k"), ("integer".into(), false));
    }

    #[test]
    fn truth_values_are_marked_and_typed_boolean() {
        let r = analyze_ddl(
            "select id = 1 as a, exists (select 1 from posts) as b, name like 'x%' as c, not (score > 1) as d, case when id > 1 then true end as e, coalesce(email is null, false) as f, email in ('a', 'b') as g, id as h, id + 1 as i, (select p.id > 1 from posts p limit 1) as j, iif(id > 1, 0, 1) as k from users",
            &ddl(),
            &Extras::default(),
        );
        assert!(r.analysis.diagnostics.is_empty(), "{:?}", r.analysis.diagnostics);
        assert_eq!(r.booleans, [true, true, true, true, true, true, true, false, false, true, false]);
        // The storage class is an integer, 0 or 1, which the type says is a boolean.
        assert_eq!(col(&r.analysis, "a"), ("bool".into(), false));
        assert_eq!(col(&r.analysis, "b"), ("bool".into(), false));
        assert_eq!(col(&r.analysis, "h"), ("integer".into(), false));
        // Through a subquery's column, and across every arm of a compound.
        let r = analyze_ddl("select t.x, t.y from (select id > 1 as x, id as y from users) t", &ddl(), &Extras::default());
        assert_eq!(r.booleans, [true, false]);
        let r = analyze_ddl("select id > 1 as x from users union all select exists (select 1 from posts) union all select 0", &ddl(), &Extras::default());
        assert_eq!(r.booleans, [false]);
        let r = analyze_ddl("select id > 1 as x from users union all select email is null from users", &ddl(), &Extras::default());
        assert_eq!(r.booleans, [true]);
        // RETURNING, and a statement that does not prepare.
        let r = analyze_ddl("insert into users (name) values ('a') returning id, id > 0 as positive", &ddl(), &Extras::default());
        assert_eq!(r.booleans, [false, true]);
        assert!(analyze_ddl("select nope from users", &ddl(), &Extras::default()).booleans.is_empty());
    }

    #[test]
    fn a_column_declared_boolean_is_typed_boolean_and_no_truth_value() {
        let ddl = vec!["CREATE TABLE flags (id INTEGER PRIMARY KEY, done BOOLEAN NOT NULL, b bool, n INTEGER, t TINYINT(1))".to_string(), "CREATE VIEW v AS SELECT done FROM flags".to_string()];
        let r = analyze_ddl("select f.done, b, n, t, f.done + 0 as plus, coalesce(b, f.done) as either, coalesce(b, 0) as zero, case when n > 0 then f.done end as c, (select done from flags limit 1) as sub, x.done as derived from flags f, (select done from flags) x", &ddl, &Extras::default());
        assert!(r.analysis.diagnostics.is_empty(), "{:?}", r.analysis.diagnostics);
        let types: Vec<String> = r.analysis.columns.iter().map(|c| scalar_name(&c.ty).unwrap_or("?").to_string()).collect();
        assert_eq!(types, ["bool", "bool", "integer", "integer", "real", "bool", "real", "bool", "bool", "bool"]);
        // The framework's truth values are the ones SQLite computes alone.
        assert_eq!(r.booleans, [false; 10]);
        // Through a view, and across a compound only where every arm is one.
        let r = analyze_ddl("select done from v", &ddl, &Extras::default());
        assert_eq!(col(&r.analysis, "done"), ("bool".into(), false));
        let r = analyze_ddl("select done from flags union all select 5", &ddl, &Extras::default());
        assert_ne!(scalar_name(&r.analysis.columns[0].ty), Some("bool"));
        let r = analyze_ddl("select done from flags union all select id > 1 from flags", &ddl, &Extras::default());
        assert_eq!(scalar_name(&r.analysis.columns[0].ty), Some("bool"));
    }

    #[test]
    fn a_column_declared_64_bits_wide_is_int8_and_what_sqlite_computes_from_it_an_integer() {
        let ddl = vec![
            "create table orders (id integer primary key, user_id BIGINT not null, ref Int8, big unsigned  big  int, wide bigint(20), n integer not null, i int, small tinyint, r real)".to_string(),
            "create table keyed (id bigint primary key, name text)".to_string(),
            "create view v as select user_id, n from orders".to_string(),
        ];
        let types = |sql: &str| {
            let r = analyze_ddl(sql, &ddl, &Extras::default());
            assert!(r.analysis.diagnostics.is_empty(), "{sql}: {:?}", r.analysis.diagnostics);
            r.analysis.columns.iter().map(|c| scalar_name(&c.ty).unwrap_or("unknown").to_string()).collect::<Vec<_>>()
        };
        // The declared width decides: BIGINT, INT8 and UNSIGNED BIG INT in any case and spacing,
        // with a size or not; every other integer name, and the rowid, is SQLite's INTEGER.
        assert_eq!(types("select id, user_id, ref, big, wide, n, i, small, r from orders"), ["integer", "int8", "int8", "int8", "int8", "integer", "integer", "integer", "real"]);
        // A BIGINT PRIMARY KEY is no rowid alias: a column of its own, declared 64 bits wide.
        assert_eq!(types("select id, rowid as r from keyed"), ["int8", "integer"]);
        // What passes the column through unchanged keeps its type: a subquery's or view's column,
        // min and max, coalesce and a CASE over integers, a window's lag, unary plus.
        assert_eq!(
            types("select x.user_id, v.user_id as through_view, max(o.user_id) as most, coalesce(o.ref, 0) as co, ifnull(o.ref, o.n) as ifn, case when o.n > 0 then o.user_id else 0 end as cs, +o.user_id as pos, (select max(user_id) from orders) as sub from orders o, (select user_id from orders) x, v group by o.id"),
            ["int8", "int8", "int8", "int8", "int8", "int8", "int8", "int8"],
        );
        assert_eq!(types("select lag(user_id) over (order by id) as prev, min(user_id, ref) as least from orders"), ["int8", "int8"]);
        assert_eq!(types("select user_id from orders union all select 1"), ["int8"]);
        // What SQLite computes from it is SQLite's INTEGER, as count(*) is; a mix with a real is a real.
        assert_eq!(
            types("select user_id + 1 as plus, user_id * n as times, -user_id as neg, abs(user_id) as a, sum(user_id) as s, count(user_id) as c, cast(user_id as integer) as ci, coalesce(ref, 1.5) as cr, total(user_id) as t from orders"),
            ["integer", "integer", "integer", "integer", "integer", "integer", "integer", "real", "real"],
        );
        // total() over a 64-bit column is over integers, so it is never NaN.
        let r = analyze_ddl("select total(user_id) as t from orders", &ddl, &Extras::default());
        assert_eq!(col(&r.analysis, "t"), ("real".into(), false));
        // A parameter compared with or written to one takes its type.
        let r = analyze_ddl("select id from orders where user_id = :u and ref in (:a, :b)", &ddl, &Extras::default());
        assert_eq!([param(&r.analysis, "u"), param(&r.analysis, "a"), param(&r.analysis, "b")], ["int8", "int8", "int8"]);
        let r = analyze_ddl("insert into orders (user_id, n) values (:u, :n) returning user_id", &ddl, &Extras::default());
        assert_eq!([param(&r.analysis, "u"), param(&r.analysis, "n")], ["int8", "integer"]);
        assert_eq!(col(&r.analysis, "user_id"), ("int8".into(), false));
        assert_eq!(r.analysis.columns[0].type_name, "int8");
        assert!(["bigint", "BIGINT", " int8 ", "Unsigned Big Int", "bigint(20)", "unsigned big int (8)"].iter().all(|d| is_int8_declared(d)));
        assert!(!["integer", "int", "bigint unsigned", "big int", "int64", "biginteger", "unsigned bigint"].iter().any(|d| is_int8_declared(d)));
    }

    /// A type as src/render.ts writes a SQLite one, compactly: the value `.run()` returns at the
    /// top, and what JSON writes inside a document.
    fn ts(t: &Type, in_json: bool) -> String {
        let null = |s: String, nullable: bool| if nullable { format!("{s} | null") } else { s };
        let list = |s: String| if s.contains('|') { format!("({s})[]") } else { format!("{s}[]") };
        match t {
            Type::Scalar { name } => match (name.as_str(), in_json) {
                ("integer" | "real", _) | ("int8", true) => "number".into(),
                ("int8", false) => "bigint".into(),
                ("text", _) => "string".into(),
                ("numeric", _) => "number | string".into(),
                ("blob", false) => "Uint8Array".into(),
                ("bool", _) => "boolean".into(),
                _ => "unknown".into(),
            },
            Type::Object { fields } => {
                let fields: Vec<String> = fields.iter().map(|f| format!("{}: {}", f.name, null(ts(&f.ty, true), f.nullable))).collect();
                if fields.is_empty() { "Record<never, never>".into() } else { format!("{{ {} }}", fields.join("; ")) }
            }
            Type::List { of, nullable_items } => list(null(ts(of, true), *nullable_items)),
            Type::Json { schema: Some(schema) } => schema_ts(schema),
            _ => "unknown".into(),
        }
    }

    fn schema_ts(s: &serde_json::Value) -> String {
        if let Some(any) = s["anyOf"].as_array() {
            return any.iter().map(schema_ts).collect::<Vec<_>>().join(" | ");
        }
        let kinds: Vec<&str> = match &s["type"] {
            serde_json::Value::String(k) => vec![k.as_str()],
            serde_json::Value::Array(ks) => ks.iter().filter_map(|k| k.as_str()).collect(),
            _ => return "unknown".into(),
        };
        let one = |kind: &str| match kind {
            "integer" | "number" => "number".to_string(),
            "string" => "string".into(),
            "null" => "null".into(),
            "array" => match s["prefixItems"].as_array() {
                Some(items) => format!("[{}]", items.iter().map(schema_ts).collect::<Vec<_>>().join(", ")),
                None if s["items"] == false => "[]".into(),
                None => {
                    let item = schema_ts(&s["items"]);
                    if item.contains('|') { format!("({item})[]") } else { format!("{item}[]") }
                }
            },
            "object" => match s["properties"].as_object() {
                _ if s["additionalProperties"] == false && s["properties"].as_object().is_none_or(|p| p.is_empty()) => "Record<never, never>".into(),
                Some(p) if !p.is_empty() => format!("{{ {} }}", p.iter().map(|(k, v)| format!("{k}: {}", schema_ts(v))).collect::<Vec<_>>().join("; ")),
                _ => format!("{{ [key: string]: {} }}", schema_ts(&s["additionalProperties"])),
            },
            _ => "unknown".into(),
        };
        kinds.into_iter().map(one).collect::<Vec<_>>().join(" | ")
    }

    /// Each result column as `name: type`, `| null` where it may be NULL.
    fn shown(a: &Analysis) -> String {
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        a.columns.iter().map(|c| format!("{}: {}{}", c.name, ts(&c.ty, false), if c.nullable { " | null" } else { "" })).collect::<Vec<_>>().join("; ")
    }

    fn json_ddl() -> Vec<String> {
        vec![
            "create table users (id integer primary key, name text not null, email text unique, github_id bigint, score real, active boolean not null default 1, avatar blob, doc text, extra, n numeric)".to_string(),
            "create table posts (id integer primary key, user_id integer not null references users(id), title text not null, body text) strict".to_string(),
            "create view titled as select id, json_object('id', id, 'title', title) as post from posts".to_string(),
        ]
    }

    fn json_typed(sql: &str) -> String {
        shown(&analyze_ddl(sql, &json_ddl(), &Extras::default()).analysis)
    }

    #[test]
    fn a_json_constructor_is_typed_as_the_json_it_builds() {
        // The docs' profile: SQLite answers with the text, which the run time parses. A scalar
        // subquery's JSON nests as JSON in `json()`, on every SQLite.
        let profile = "select json_object('user', json_object('id', u.id, 'name', u.name), 'posts', json((select json_group_array(json_object('id', p.id, 'title', p.title)) from posts p where p.user_id = u.id))) as profile from users u where u.id = :id";
        assert_eq!(json_typed(profile), "profile: { user: { id: number; name: string }; posts: { id: number; title: string }[] }");
        // Without `json()`, SQLite 3.54.1 nests it as its text, and 3.53.4 as JSON.
        let bare = profile.replace("json((select", "((select");
        assert_eq!(json_typed(&bare), "profile: { user: { id: number; name: string }; posts: unknown }");
        // At the top, a scalar subquery's JSON is text either way, which the run time parses.
        assert_eq!(json_typed("select (select json_group_array(id) from posts) as ids"), "ids: number[]");
        assert_eq!(json_typed("select json((select json_object('n', name) from users where id = 1)) as u"), "u: { n: string } | null");
        assert_eq!(json_typed("select json_array((select name from users where id = 1)) as a, json_group_array((select name from users where id = 1)) as g, json_group_object((select name from users where id = 1), id) as o from users"), "a: [string | null]; g: (string | null)[]; o: { [key: string]: number }");
        let a = analyze_ddl(profile, &json_ddl(), &Extras::default()).analysis;
        assert_eq!((a.columns[0].type_name.as_str(), a.cardinality.as_str()), ("text", "maybeOne"));
        // Each value as SQLite writes it there: an INTEGER, a BIGINT column's too, as a number
        // (which JSON.parse rounds past 2^53), a REAL, a TEXT, a NUMERIC as either, NULL as null,
        // a truth value as 1 or 0 and a BOOLEAN column as what it holds. A BLOB is read as JSONB
        // (`x'01'` is `true`) or is an error, and a column of no type may hold one.
        assert_eq!(
            json_typed("select json_object('i', 1, 'id', id, 'big', github_id, 'r', score, 't', name, 'e', email, 'n', n, 'nul', null, 'cmp', id > 1, 'tru', true, 'flag', active, 'b', avatar, 'any', extra) as o from users"),
            "o: { i: number; id: number; big: number | null; r: number | null; t: string; e: string | null; n: number | string | null; nul: unknown; cmp: number; tru: number; flag: number | string; b: unknown; any: unknown }"
        );
        // A JSON array of these values is a tuple; an object with a key the statement computes
        // is one of any keys.
        assert_eq!(json_typed("select json_array(id, name, email, json_array(), json_object()) as a from users"), "a: [number, string, string | null, [], Record<never, never>]");
        assert_eq!(json_typed("select json_object(name, id) as o, json_object('id', id, name, score) as m from users"), "o: { [key: string]: number }; m: { [key: string]: number | number | null }");
        // A key written twice is in the text twice, and JSON.parse keeps the last.
        assert_eq!(json_typed("select json_object('k', id, 'k', name) as o from users"), "o: { k: string }");
        // The aggregates are `[]` and `{}` over no rows, never NULL, with a FILTER or as a window.
        assert_eq!(json_typed("select json_group_array(email) as e, json_group_object(name, id) as g from users where id < 0"), "e: (string | null)[]; g: { [key: string]: number }");
        assert_eq!(json_typed("select json_group_array(id) filter (where id > 1) as f, json_group_array(id) over (order by id) as w from users"), "f: number[]; w: number[]");
        assert_eq!(
            json_typed("select json_object('f', json_group_array(id) filter (where id > 1)) as f, json_object('w', json_group_array(id) over ()) as w from users"),
            "f: { f: number[] }; w: { w: unknown }"
        );
        assert_eq!(
            json_typed("select u.id, (select json_group_array(p.title) from posts p where p.user_id = u.id) as titles, (select json_group_array(p.title) from posts p where p.user_id = u.id group by p.user_id) as grouped from users u"),
            "id: number; titles: string[]; grouped: string[] | null"
        );
        // `json()` of a constructor builds the same; `jsonb_*` is a blob, read as JSON inside a
        // document and by `json()`.
        assert_eq!(json_typed("select json(json_object('a', id)) as j, json(jsonb_object('a', id)) as b, json_object('a', jsonb_array(id), 'b', jsonb(json_array(1))) as o from users"), "j: { a: number }; b: { a: number }; o: { a: [number]; b: [number] }");
        assert_eq!(json_typed("select jsonb(json_array(1)) as b, (select jsonb_group_array(id) from users) as g"), "b: Uint8Array; g: Uint8Array");
        // A column that passes built JSON through is the text, parsed all the same.
        assert_eq!(json_typed("select post from titled"), "post: { id: number; title: string }");
        assert_eq!(json_typed("with t as (select json_array(id) as x from users) select x from t"), "x: [number]");
        assert_eq!(json_typed("select max(json_array(id)) as m from users"), "m: [number] | null");
        assert_eq!(json_typed("insert into posts (user_id, title) values (1, 'a') returning json_object('id', id, 'title', title) as post"), "post: { id: number; title: string }");
    }

    #[test]
    fn a_value_nests_as_json_only_where_it_keeps_sqlites_json_subtype() {
        // Through what kept the subtype on SQLite 3.53.4 a JSON value nests as the JSON it is.
        for (expr, ty) in [
            ("coalesce(json_array(id), 'x')", "[number]"),
            ("ifnull(json_array(id), 'x')", "[number]"),
            ("nullif(json_array(id), 'x')", "[number] | null"),
            ("iif(id > 1, json_array(id), json_array(0))", "[number]"),
            ("case when id > 1 then json_object('x', id) end", "{ x: number } | null"),
            ("json_array(id) collate nocase", "[number]"),
            ("+json_array(id)", "[number]"),
            ("likely(json_array(id))", "[number]"),
            // `json()` marks a scalar subquery's JSON again, NULL where it finds no row.
            ("json((select json_object('n', name) from users u2 where u2.id = users.id))", "{ n: string } | null"),
            ("json((select json_group_array(p.id) from posts p where p.user_id = users.id order by p.id))", "number[]"),
            ("json((select json_array(1)))", "[number]"),
            // Another constructor reads the subquery's NULL as a null, and is never NULL.
            ("json_array((select name from users u2 where u2.id = users.id))", "[string | null]"),
            ("json_group_array((select name from users u2 where u2.id = users.id))", "(string | null)[]"),
            // `coalesce` never reaches an argument after one that is never NULL.
            ("coalesce(json((select json_group_array(p.id) from posts p where p.user_id = users.id)), '[]')", "number[]"),
            ("coalesce(json_object('e', email), json_array())", "{ e: string | null }"),
            ("coalesce(json((select json_group_array(p.id) from posts p where p.user_id = users.id group by p.user_id)), json_array())", "number[]"),
            // A plain value: a TEXT is a string, even one that holds JSON.
            ("doc", "string | null"),
            ("json_array(id) || ''", "string"),
            ("upper(json_array(id))", "string"),
            ("length(json_array(id))", "number"),
            ("json_type(json_array(id))", "string"),
            ("json_array_length(json_array(id))", "number"),
        ] {
            assert_eq!(json_typed(&format!("select json_object('a', {expr}) as o from users")), format!("o: {{ a: {ty} }}"), "{expr}");
        }
        // What dropped it on 3.53.4, or keeps it on one SQLite and not another, and what returns
        // any JSON: any JSON.
        for expr in [
            "x",
            "post",
            // A scalar subquery's JSON kept the subtype on 3.53.4 and lost it on 3.54.1.
            "(select json_object('n', name) from users u2 where u2.id = users.id)",
            "(select json_array(1))",
            "(select json_object('id', id) from users order by name limit 1)",
            "(select distinct json_array(id) from users)",
            "(select json_array(1) union all select json_array(2))",
            "lag(json_array(id)) over (order by id)",
            "max(json_array(id), json_array(0))",
            "cast(json_array(id) as text)",
            "json(doc)",
            "json_extract(doc, '$.a')",
            "doc -> '$.a'",
            "doc ->> '$.a'",
            "json_set(doc, '$.a', 1)",
            "json_quote(name)",
            "json_pretty(json_array(1))",
            "case when id > 1 then json_array(id) else 'x' end",
            "case when id > 1 then json_array(id) else json_object('a', 1) end",
        ] {
            let sql = format!("select json_object('a', {expr}) as o from (select *, json_array(id) as x from users) users, (select post from titled)");
            assert_eq!(json_typed(&sql), "o: { a: unknown }", "{expr}");
        }
        // A function the host registers over a built-in is its own, and may set the subtype.
        let host = Extras { total: vec![TotalFunction { name: "json_array".into(), arity: -1, returns: Affinity::Text }, TotalFunction { name: "html".into(), arity: 1, returns: Affinity::Text }], host: None };
        let a = analyze_ddl("select json_array(1) as a, json_object('b', html(name)) as b from users", &json_ddl(), &host).analysis;
        assert_eq!(shown(&a), "a: string; b: { b: unknown }");
    }

    #[test]
    fn a_deep_constructor_is_read_once_per_node() {
        // Each node's shape is worked out once, so a statement as deep as the engine reads takes
        // no longer than its size.
        let ddl = vec!["create table users (id integer primary key, name text not null)".to_string()];
        let deep = |open: &str, close: &str, n: usize| format!("select {}id{} as o from users", open.repeat(n), close.repeat(n));
        let started = std::time::Instant::now();
        for sql in [deep("json_object('a', ", ")", 240), deep("json_array(coalesce(length(json_object('a', ", ")), 1))", 60), deep("coalesce((select json_group_array(", ") from users), 1)", 12)] {
            let a = crate::stack::run(|| analyze_ddl(&sql, &ddl, &Extras::default()).analysis);
            assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "{:?}", started.elapsed());
    }

    #[test]
    fn built_json_beside_other_text_is_the_text_it_is() {
        // SQLite returns built JSON as text: where a value may be other text, it is a string.
        assert_eq!(
            json_typed("select coalesce(json_array(id), 'x') as a, case when id > 1 then json_object('a', 1) else json_object('b', 2) end as b, coalesce(doc, json_array()) as c, iif(id > 1, json_array(id), null) as d from users"),
            "a: [number]; b: string; c: string; d: [number] | null"
        );
        assert_eq!(json_typed("select json_array(id) as a from users union all select 'x'"), "a: string");
        assert_eq!(json_typed("select json_array(id) as a from users union all select null"), "a: [number] | null");
        // A parameter that may be any text the caller passes: `coalesce(:doc, json_array())` is
        // that text, and the parameter is a string.
        let a = analyze_ddl("select coalesce(:doc, json_array()) as d, json_object('a', :a) as o from users where json_array(id) = :j", &json_ddl(), &Extras::default()).analysis;
        assert_eq!(shown(&a), "d: string; o: { a: unknown }");
        assert_eq!(a.params.iter().map(|p| format!("{} {} {}", p.name, ts(&p.ty, false), p.type_name)).collect::<Vec<_>>(), ["doc string text", "a unknown ", "j string text"]);
    }

    #[test]
    fn a_json_object_of_an_odd_count_of_arguments_is_built_in_alone() {
        let sql = "select json_object('id', id, 'name') as j from users";
        let odd = |extras: &Extras| analyze_ddl(sql, &ddl(), extras).analysis.diagnostics.iter().filter(|d| d.code == 15).count();
        assert_eq!(odd(&Extras::default()), 1);
        let listed = |arities: &[i32]| Extras { total: vec![], host: Some(arities.iter().map(|&arity| HostFunction { name: "json_object".into(), arity, kind: 's' }).collect()) };
        assert_eq!(odd(&listed(&[-1])), 1);
        // A function of the name the host registers is called in the built-in's place.
        assert_eq!(odd(&listed(&[-1, 3])), 0);
        assert_eq!(odd(&Extras { total: vec![TotalFunction { name: "JSON_OBJECT".into(), arity: 3, returns: Affinity::Text }], host: None }), 0);
    }

    #[test]
    fn sqlite_only_syntax_parses() {
        let a = analysis("select id from users limit :n, :m");
        assert_eq!(param(&a, "n"), "integer");
        assert_eq!(analysis("insert or replace into users (id, name) values (:id, :name) returning id").cardinality, "one");
    }
}

