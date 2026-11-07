//! A SQL function's body, read as the query it is.
//!
//! A statement that calls a user's `language sql` function is analyzed twice: as written, which
//! settles its parameters and each column's name and type, and with each call replaced by the
//! function's body, which is what a call returns. The second analysis counts only where it describes
//! to the same columns of the same types, and only for what narrows the first: a column that is not
//! null, a smaller cardinality, a JSON shape. So a body the engine does not read, or reads and cannot
//! describe, leaves the statement's result as it was.
//!
//! The expanded statement is prepared and never run, and nothing in it depends on the planner
//! inlining the function: what a body returns is what the call returns, inlined or not.
//!
//! **A body stays alone.** A body is read as a view is, a statement of its own: the caller's
//! expressions are never written into it, so no name of the caller's can meet a name of the body's.
//! An argument that names nothing, a constant or a parameter, is written in as it is. Any other
//! is a parameter of its own that may be null, and in FROM a one-row `reylo_args` beside the body reads
//! the caller's expression, so the item still depends on the row it does. The one exception is a body
//! that is one expression with no FROM ([`inline`]), which binds no name at all and can take the
//! caller's columns, as the planner takes them when it inlines one.
//!
//! Where a call is not its body otherwise, the expansion says so or the call is left alone:
//!
//! - **One row.** A function that returns no set returns the first row of its body, or NULLs where
//!   the body returns none. In FROM it is read only where the body returns exactly one row, and in an
//!   expression it is a scalar subquery, which is NULL for no row.
//! - **Strict.** A NULL argument returns NULL, or no rows, without running the body.
//! - **Names.** A column in scope beats an argument of the same name, and the server settles each
//!   name ([`resolve`]). A WITH query of the caller's must not take a table the body reads.
//! - **Position and coercion.** Result columns go by position, under the names the function declares,
//!   each coerced to the declared type.
//! - **One statement.** Only the last statement of a body returns rows; a body of several is not read.
//! - **Settings.** A string body parses under the function's own settings, which the session describing
//!   it does not hold: one with any setting but a search path is not read, and one with a search path
//!   must name its schemas.

use crate::analyze;
use crate::cardinality;
use crate::dialect::Dialect;
use crate::api::Scope;
use crate::host::{as_bool, as_str, as_strings, as_u32, Host, Lazy};
use crate::lex;
use crate::postgres;
use serde_json::{json, Map, Value};
use std::collections::HashSet;

/// `$1` is a `text[]` of function names: one row per function of each name, built-ins included,
/// since which overloads are in scope decides whether a call's function is known.
pub const BODIES_QUERY: &str = "SELECT p.proname::pg_catalog.text, n.nspname::pg_catalog.text, pg_catalog.pg_function_is_visible(p.oid), \
    l.lanname OPERATOR(pg_catalog.=) 'sql'::pg_catalog.name, p.prokind::pg_catalog.text, p.prosrc, \
    CASE WHEN p.prosqlbody IS NOT NULL THEN pg_catalog.pg_get_function_sqlbody(p.oid) END, \
    p.proretset, p.proisstrict, p.proconfig, p.prosupport::pg_catalog.oid OPERATOR(pg_catalog.<>) 0::pg_catalog.oid, \
    p.provariadic OPERATOR(pg_catalog.<>) 0::pg_catalog.oid, pg_catalog.pg_get_expr(p.proargdefaults, 0::pg_catalog.oid), p.pronargdefaults::pg_catalog.int4, \
    ARRAY(SELECT pg_catalog.format_type(t.oid, -1) FROM pg_catalog.generate_series(0, p.pronargs::pg_catalog.int4 OPERATOR(pg_catalog.-) 1) AS i \
      JOIN pg_catalog.pg_type t ON t.oid OPERATOR(pg_catalog.=) p.proargtypes[i] ORDER BY i), \
    ARRAY(SELECT t.typtype::pg_catalog.text FROM pg_catalog.generate_series(0, p.pronargs::pg_catalog.int4 OPERATOR(pg_catalog.-) 1) AS i \
      JOIN pg_catalog.pg_type t ON t.oid OPERATOR(pg_catalog.=) p.proargtypes[i] ORDER BY i), \
    p.proargnames, p.proargmodes::pg_catalog.text[], p.proallargtypes::pg_catalog.oid[], p.prorettype::pg_catalog.oid, r.typtype::pg_catalog.text, \
    ARRAY(SELECT a.attname::pg_catalog.text FROM pg_catalog.pg_attribute a WHERE a.attrelid OPERATOR(pg_catalog.=) r.typrelid AND a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped ORDER BY a.attnum), \
    ARRAY(SELECT a.atttypid::pg_catalog.oid FROM pg_catalog.pg_attribute a WHERE a.attrelid OPERATOR(pg_catalog.=) r.typrelid AND a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped ORDER BY a.attnum), \
    ARRAY(SELECT pg_catalog.format_type(t.oid, -1) FROM pg_catalog.generate_series(1, 200) AS i \
      JOIN pg_catalog.pg_type t ON t.oid OPERATOR(pg_catalog.=) p.proallargtypes[i] ORDER BY i), \
    ARRAY(SELECT pg_catalog.format_type(a.atttypid, -1) FROM pg_catalog.pg_attribute a WHERE a.attrelid OPERATOR(pg_catalog.=) r.typrelid AND a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped ORDER BY a.attnum), \
    pg_catalog.format_type(p.prorettype, -1), \
    pg_catalog.textin(pg_catalog.regprocedureout(p.oid::pg_catalog.regprocedure)), \
    EXISTS (SELECT 1 FROM pg_catalog.pg_type t WHERE t.typname OPERATOR(pg_catalog.=) p.proname), \
    p.provolatile::pg_catalog.text, \
    ARRAY(SELECT (t.typcollation OPERATOR(pg_catalog.<>) 0::pg_catalog.oid)::pg_catalog.text FROM pg_catalog.generate_series(0, p.pronargs::pg_catalog.int4 OPERATOR(pg_catalog.-) 1) AS i \
      JOIN pg_catalog.pg_type t ON t.oid OPERATOR(pg_catalog.=) p.proargtypes[i] ORDER BY i), \
    ARRAY(SELECT t.typname::pg_catalog.text FROM pg_catalog.generate_series(0, p.pronargs::pg_catalog.int4 OPERATOR(pg_catalog.-) 1) AS i \
      JOIN pg_catalog.pg_type t ON t.oid OPERATOR(pg_catalog.=) p.proargtypes[i] ORDER BY i), \
    p.prosecdef \
    FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_namespace n ON n.oid OPERATOR(pg_catalog.=) p.pronamespace \
    JOIN pg_catalog.pg_language l ON l.oid OPERATOR(pg_catalog.=) p.prolang JOIN pg_catalog.pg_type r ON r.oid OPERATOR(pg_catalog.=) p.prorettype \
    WHERE p.proname::pg_catalog.text OPERATOR(pg_catalog.=) ANY ($1::pg_catalog.text[])";

/// How many levels of calls are read: a function's body reading a function is one more. A call past
/// the last stays a call.
const MAX_LEVELS: usize = 6;
/// How long an expanded statement may grow.
const MAX_TEXT: usize = 256 * 1024;

/// A column a function declares.
#[derive(Debug, Clone)]
struct Output {
    name: String,
    oid: u32,
    /// The type as a cast writes it.
    ty: String,
}

#[derive(Debug, Clone)]
struct Function {
    name: String,
    /// The function as `to_regprocedure` reads it back.
    signature: String,
    /// Whether a type goes by its name, where a call of one argument may be a cast to that type.
    names_a_type: bool,
    schema: String,
    visible: bool,
    sql: bool,
    kind: String,
    source: String,
    atomic: Option<String>,
    set: bool,
    strict: bool,
    /// What it sets for its run (`SET search_path = …`), as `name=value`.
    settings: Vec<String>,
    volatile: bool,
    security_definer: bool,
    /// Whether its one value is no row: a composite's one field is the row in an expression.
    one_value: bool,
    supported: bool,
    variadic: bool,
    defaults: Option<String>,
    default_count: usize,
    input_types: Vec<String>,
    /// Whether each input's type has a collation.
    input_collatable: Vec<bool>,
    /// Each input's type as the catalog names it.
    input_typenames: Vec<String>,
    pseudo_input: bool,
    input_names: Vec<Option<String>>,
    /// The columns the function declares, in order. None where it declares none of its own (a
    /// pseudo-type such as `record`, `void` or a polymorphic type).
    outputs: Vec<Output>,
    /// A function of one value of a type that is no row, whose column goes by the FROM item's name.
    scalar: bool,
}

impl Function {
    fn read(row: &[Value]) -> Option<Function> {
        let text = |i: usize| row.get(i).and_then(as_str).map(String::from);
        let flag = |i: usize| row.get(i).and_then(as_bool).unwrap_or(false);
        let name = text(0)?;
        let strings = |i: usize| row.get(i).and_then(as_strings);
        let modes = strings(17);
        let names: Option<Vec<String>> = strings(16);
        let oids = |i: usize| row.get(i).and_then(Value::as_array).map(|a| a.iter().filter_map(as_u32).collect::<Vec<u32>>());
        let all_types = oids(18);
        let input_types = strings(14)?;
        let is_input = |mode: &str| matches!(mode, "i" | "b" | "v");
        let input_names: Vec<Option<String>> = match (&modes, &names) {
            (Some(modes), Some(names)) => modes.iter().enumerate().filter(|(_, m)| is_input(m)).map(|(i, _)| names.get(i).filter(|n| !n.is_empty()).cloned()).collect(),
            (None, Some(names)) => (0..input_types.len()).map(|i| names.get(i).filter(|n| !n.is_empty()).cloned()).collect(),
            _ => vec![None; input_types.len()],
        };
        let returned = row.get(19).and_then(as_u32)?;
        let kind = text(20)?;
        let (outputs, scalar): (Vec<Output>, bool) = match (&modes, &names, &all_types) {
            (Some(modes), Some(names), Some(types)) if modes.iter().any(|m| !is_input(m) || m == "b") => {
                let type_names = strings(23)?;
                let out: Option<Vec<Output>> = modes.iter().enumerate().filter(|(_, m)| matches!(m.as_str(), "o" | "b" | "t")).map(|(i, _)| Some(Output { name: names.get(i).filter(|n| !n.is_empty())?.clone(), oid: *types.get(i)?, ty: type_names.get(i)?.clone() })).collect();
                (out.unwrap_or_default(), false)
            }
            _ if kind == "c" => {
                let (names, oids, types) = (strings(21)?, oids(22)?, strings(24)?);
                (names.into_iter().zip(oids).zip(types).map(|((name, oid), ty)| Output { name, oid, ty }).collect(), false)
            }
            _ if kind == "p" => (Vec::new(), false),
            _ => (vec![Output { name: name.clone(), oid: returned, ty: text(25)? }], true),
        };
        let kinds = strings(15)?;
        Some(Function {
            signature: text(26)?,
            names_a_type: flag(27),
            schema: text(1)?,
            visible: flag(2),
            sql: flag(3),
            kind: text(4)?,
            source: text(5).unwrap_or_default(),
            atomic: text(6),
            set: flag(7),
            strict: flag(8),
            settings: strings(9).unwrap_or_default(),
            volatile: text(28).as_deref() == Some("v"),
            security_definer: flag(31),
            one_value: outputs.len() == 1 && kind != "c",
            supported: flag(10),
            variadic: flag(11),
            defaults: text(12),
            default_count: row.get(13).and_then(as_u32).unwrap_or(0) as usize,
            input_types,
            input_collatable: strings(29)?.iter().map(|c| c == "true").collect(),
            input_typenames: strings(30)?,
            pseudo_input: kinds.iter().any(|k| k == "p"),
            input_names,
            outputs,
            scalar,
            name,
        })
    }

    /// Whether this function's body may be read at all, whatever calls it.
    fn readable(&self) -> bool {
        self.sql && self.kind == "f" && !self.supported && !self.names_a_type && !self.pseudo_input && !self.outputs.is_empty() && !matches!(self.schema.as_str(), "pg_catalog" | "information_schema")
    }

    fn input(&self, name: &str) -> Option<usize> {
        self.input_names.iter().position(|n| n.as_deref() == Some(name))
    }
}

/// `value` in the tree's JSON at `path` of object keys.
fn at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |v, key| v.get(key))
}

fn node_of(value: &Value) -> Option<(&str, &Map<String, Value>)> {
    let map = value.get("node")?.as_object()?;
    let (key, inner) = map.iter().next()?;
    Some((key.as_str(), inner.as_object()?))
}

fn names_of(list: Option<&Value>) -> Vec<String> {
    list.and_then(Value::as_array).into_iter().flatten().filter_map(|n| n.pointer("/node/String/sval").and_then(Value::as_str).map(String::from)).collect()
}

/// `f` of every object under `value` with the key it is under, depth first.
fn each_object<'a>(key: &str, value: &'a Value, f: &mut dyn FnMut(&str, &'a Map<String, Value>)) {
    match value {
        Value::Object(map) => {
            f(key, map);
            for (k, v) in map {
                each_object(k, v, f);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| each_object(key, v, f)),
        _ => {}
    }
}

fn tree(sql: &str) -> Option<(Value, crate::stack::Parsed)> {
    let parsed = crate::stack::parse(sql).ok()?;
    Some((serde_json::to_value(&parsed.protobuf).ok()?, parsed))
}

fn identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// The one SELECT of a function's body, deparsed: a string body, or the `BEGIN ATOMIC` or `RETURN`
/// the server prints for a body stored parsed. `None` for any other body.
fn body_statement(f: &Function) -> Option<String> {
    let text = match &f.atomic {
        Some(atomic) => {
            let atomic = atomic.trim();
            match atomic.strip_prefix("RETURN ") {
                Some(value) => format!("SELECT {value}"),
                None => atomic.strip_prefix("BEGIN ATOMIC")?.trim().strip_suffix("END")?.to_string(),
            }
        }
        None => f.source.clone(),
    };
    let parsed = crate::stack::parse(&text).ok()?;
    let [stmt] = parsed.protobuf.stmts.as_slice() else { return None };
    match stmt.stmt.as_deref()?.node.as_ref()? {
        pg_query::protobuf::node::Node::SelectStmt(s) if s.into_clause.is_none() => crate::stack::deparse(&parsed.protobuf).ok(),
        _ => None,
    }
}

/// The byte offset of the 1-based character position the server reported.
fn byte_at(text: &str, position: usize) -> Option<usize> {
    text.char_indices().nth(position.checked_sub(1)?).map(|(i, _)| i)
}

/// Where a column reference that starts at `start` ends.
fn reference_end(text: &str, start: usize) -> usize {
    let b = text.as_bytes();
    let mut i = start;
    loop {
        if b.get(i) == Some(&b'"') {
            i += 1;
            while i < b.len() {
                if b[i] == b'"' {
                    if b.get(i + 1) == Some(&b'"') {
                        i += 2;
                        continue;
                    }
                    break;
                }
                i += 1;
            }
            i += 1;
        } else {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$' || b[i] >= 0x80) {
                i += 1;
            }
        }
        if b.get(i) == Some(&b'.') {
            i += 1;
        } else {
            return i;
        }
    }
}

/// What a name the server does not know stands for in `f`'s body, as Postgres reads one
/// (`sql_fn_post_column_ref`): `a` an argument; `a.b` an argument `b` of the function `a`, or else a
/// field of the argument `a`; `a.b.c` a field of the argument `b` of the function `a`. A whole row
/// (`a.*`) is left unread.
fn argument_reference(f: &Function, fields: &[String]) -> Option<String> {
    let typed = |n: usize| format!("(${}::{})", n + 1, f.input_types[n]);
    match fields {
        [a] => Some(typed(f.input(a)?)),
        [a, b] if *a == f.name && f.input(b).is_some() => Some(typed(f.input(b)?)),
        [a, b] => Some(format!("({}).{}", typed(f.input(a)?), identifier(b))),
        [a, b, c] if *a == f.name => Some(format!("({}).{}", typed(f.input(b)?), identifier(c))),
        _ => None,
    }
}

/// Every `$n` of a body, typed as its argument is.
fn typed_numbers(text: &str, f: &Function) -> Option<String> {
    let lexed = lex::lex(text, Dialect::Postgres).ok()?;
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for p in &lexed.placeholders {
        let n: usize = p.name.parse().ok()?;
        out.push_str(&text[last..p.start]);
        out.push_str(&format!("(${n}::{})", f.input_types.get(n.checked_sub(1)?)?));
        last = p.end;
    }
    out.push_str(&text[last..]);
    Some(out)
}

/// A body as a statement of its own: its text, every reference to an argument a typed `$n`, and for
/// each column the cast that coerces it to what the function declares, where its type is another.
#[derive(Debug, Clone)]
struct Body {
    text: String,
    casts: Vec<Option<String>>,
    /// Whether its columns already go by the names the function declares, so no column list renames
    /// them: a key is read through a list by position, and a `*` has none to read.
    named: bool,
}

/// A name in a body that may be an argument: where it stands, what writes the argument, and for a
/// select list's bare name, the name of the column it gives, which a typed `$n` would lose.
struct Candidate {
    start: usize,
    end: usize,
    text: String,
}

/// Every reference in `text` whose first name is one of `f`'s arguments, or `f`'s own name.
fn candidates(text: &str, f: &Function) -> Option<Vec<Candidate>> {
    let (value, _) = tree(text)?;
    let mut bare_targets: Vec<(u64, String)> = Vec::new();
    each_object("", &value, &mut |key, map| {
        if key == "ResTarget" && map.get("name").and_then(Value::as_str).unwrap_or("").is_empty() {
            if let Some(("ColumnRef", c)) = map.get("val").and_then(node_of) {
                let names = names_of(c.get("fields"));
                if let (Some(at), Some(last)) = (c.get("location").and_then(Value::as_u64), names.last()) {
                    bare_targets.push((at, last.clone()));
                }
            }
        }
    });
    let mut found = Vec::new();
    each_object("", &value, &mut |key, map| {
        if key != "ColumnRef" {
            return;
        }
        let names = names_of(map.get("fields"));
        // `t.*` holds a name no argument reference writes.
        let whole = map.get("fields").and_then(Value::as_array).map_or(0, Vec::len) != names.len();
        let (Some(at), Some(first), false) = (map.get("location").and_then(Value::as_u64), names.first(), whole) else { return };
        if *first != f.name && f.input(first).is_none() {
            return;
        }
        let Some(reference) = argument_reference(f, &names) else { return };
        let start = at as usize;
        let text_of = match bare_targets.iter().find(|(target, _)| *target == at) {
            Some((_, name)) => format!("{reference} AS {}", identifier(name)),
            None => reference,
        };
        found.push(Candidate { start, end: reference_end(text, start), text: text_of });
    });
    found.sort_by_key(|c| c.start);
    Some(found)
}

/// `text` with the candidates `keep` picks replaced, and where the one it leaves, `left`, now starts.
fn replaced(text: &str, candidates: &[Candidate], keep: impl Fn(usize) -> bool, left: Option<usize>) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let (mut last, mut at) = (0, 0);
    for (i, c) in candidates.iter().enumerate() {
        out.push_str(&text[last..c.start]);
        if left == Some(i) {
            at = out.len();
        }
        if keep(i) {
            out.push_str(&c.text);
            last = c.end;
        } else {
            last = c.start;
        }
    }
    out.push_str(&text[last..]);
    (out, at)
}

/// `f`'s body as a statement of its own, every reference to an argument a typed `$n`. A column in
/// scope beats an argument of the same name, as it does when Postgres parses the body, and the server
/// says which is which: each candidate is described with the others written as arguments, all in one
/// round, and it is an argument where the server reports it, at its own place, as no column. The
/// result is described once more, which settles its columns: `Some(None)` where it does not describe
/// to as many as the function declares.
fn resolve(f: &Function, host: &Host) -> Lazy<Option<Body>> {
    let Some(text) = body_statement(f).and_then(|b| typed_numbers(&b, f)) else { return Some(None) };
    let Some(found) = candidates(&text, f) else { return Some(None) };
    let tried: Vec<(Lazy<Result<crate::api::RawDescribe, crate::api::DbError>>, usize, String)> = (0..found.len())
        .map(|i| {
            let (probe, at) = replaced(&text, &found, |j| j != i, Some(i));
            (host.describe(&probe), at, probe)
        })
        .collect();
    let mut arguments = vec![false; found.len()];
    let mut pending = false;
    for (i, (answer, at, probe)) in tried.iter().enumerate() {
        match answer {
            None => pending = true,
            Some(Err(e)) => arguments[i] = matches!(e.code.as_deref(), Some("42703" | "42P01")) && e.position.and_then(|p| byte_at(probe, p)) == Some(*at),
            Some(Ok(_)) => {}
        }
    }
    if pending {
        return None;
    }
    let (text, _) = replaced(&text, &found, |i| arguments[i], None);
    match host.describe(&text)? {
        Ok(raw) if raw.columns.len() == f.outputs.len() => {
            let casts = raw.columns.iter().zip(&f.outputs).map(|(c, o)| (c.oid != o.oid).then(|| o.ty.clone())).collect();
            let named = !f.scalar && raw.columns.iter().zip(&f.outputs).all(|(c, o)| c.name == o.name);
            Some(Some(Body { text, casts, named }))
        }
        _ => Some(None),
    }
}

/// An argument as the call writes it.
#[derive(Debug, Clone)]
struct Argument {
    text: String,
    /// The columns it reads, each as written.
    columns: Vec<Vec<String>>,
    /// Whether it holds a subquery, whose names the proof of [`hygienic`] does not read.
    subquery: bool,
    /// The functions it calls, and whether it calls one as an aggregate or a window function by its
    /// form alone: such a call is the caller's, and belongs in no body.
    calls: Vec<String>,
    aggregates: bool,
    /// The types it casts to, by the name the statement writes: a cast may be a user's.
    casts: Vec<String>,
    /// Whether it writes a COLLATE.
    collates: bool,
    /// Its one bare name, which may be a column or a FROM item's whole row.
    bare: Option<String>,
    /// That name, where the statement reads no column of it: the FROM item's whole row.
    whole_row: Option<String>,
    /// Whether it is certainly not NULL: a constant, or a parameter the statement does not let be null.
    never_null: bool,
}

impl Argument {
    /// Whether it reads nothing of the caller's, so that writing it into a body changes nothing.
    fn names_nothing(&self) -> bool {
        self.columns.is_empty() && !self.subquery && self.calls.is_empty()
    }

    fn read(text: &str, nullable: &HashSet<String>) -> Option<Argument> {
        let (value, _) = tree(&format!("SELECT {text}"))?;
        let val = at(&value, &["stmts"])?.get(0)?.pointer("/stmt/node/SelectStmt/target_list/0/node/ResTarget/val")?;
        let mut columns = Vec::new();
        let mut subquery = false;
        let mut calls = Vec::new();
        let mut aggregates = false;
        let mut casts = Vec::new();
        let mut collates = false;
        each_object("", val, &mut |key, map| match key {
            "CollateClause" => collates = true,
            "ColumnRef" => columns.push(names_of(map.get("fields"))),
            "SubLink" => subquery = true,
            "TypeCast" => casts.extend(names_of(map.get("type_name").and_then(|n| n.get("names"))).last().cloned()),
            "FuncCall" => {
                calls.extend(names_of(map.get("funcname")).last().cloned());
                aggregates |= map.get("agg_star").and_then(Value::as_bool).unwrap_or(false) || !map.get("over").is_none_or(Value::is_null);
            }
            _ => {}
        });
        let inner = |node: &Value| -> bool {
            fn certain(node: &Value, nullable: &HashSet<String>) -> bool {
                match node_of(node) {
                    Some(("AConst", c)) => c.get("isnull").and_then(Value::as_bool) != Some(true),
                    Some(("ParamRef", p)) => !nullable.contains(&p.get("number").and_then(Value::as_i64).unwrap_or(0).to_string()),
                    // One cast of a value: a second may turn it to NULL, as `'null'::jsonb::int8` does.
                    Some(("TypeCast", t)) => t.get("arg").and_then(node_of).is_some_and(|(k, _)| matches!(k, "AConst" | "ParamRef")) && t.get("arg").is_some_and(|a| certain(a, nullable)),
                    _ => false,
                }
            }
            certain(node, nullable)
        };
        let bare = match node_of(val) {
            Some(("ColumnRef", c)) => match names_of(c.get("fields")).as_slice() {
                [one] => Some(one.clone()),
                _ => None,
            },
            _ => None,
        };
        Some(Argument { text: text.trim().to_string(), never_null: inner(val), columns, subquery, calls, aggregates, casts, collates, bare, whole_row: None })
    }
}

/// Whether anything in a statement could show a column called `name`: a select list's entry of the
/// name, a column list, or a FROM item whose columns the statement does not say.
fn may_show(statement: &Value, name: &str) -> bool {
    let mut shows = false;
    let lists = |list: Option<&Value>| names_of(list).iter().any(|n| n == name);
    each_object("", statement, &mut |key, map| match key {
        "ResTarget" => {
            shows |= map.get("name").and_then(Value::as_str) == Some(name);
            if let Some(("ColumnRef", c)) = map.get("val").and_then(node_of) {
                let names = names_of(c.get("fields"));
                let whole = c.get("fields").and_then(Value::as_array).map_or(0, Vec::len) != names.len();
                shows |= !whole && names.last().is_some_and(|last| last == name);
            }
        }
        // A VALUES list's columns go by `column1` and on.
        "SelectStmt" => shows |= map.get("values_lists").and_then(Value::as_array).is_some_and(|v| !v.is_empty()) && name.starts_with("column"),
        "Alias" | "alias" => shows |= lists(map.get("colnames")) || (map.get("aliasname").and_then(Value::as_str) == Some(name) && !names_of(map.get("colnames")).is_empty()),
        "CommonTableExpr" => shows |= lists(map.get("aliascolnames")),
        "RangeFunction" | "RangeTableFunc" | "JsonTable" => shows = true,
        _ => {}
    });
    shows
}

/// The names a body binds, at any level: each FROM item's alias, a table's name, a WITH query's.
fn bound_names(body: &Value) -> HashSet<String> {
    let mut names = HashSet::new();
    each_object("", body, &mut |key, map| match key {
        "RangeVar" => {
            names.extend(map.get("relname").and_then(Value::as_str).map(String::from));
        }
        "Alias" | "alias" => {
            names.extend(map.get("aliasname").and_then(Value::as_str).map(String::from));
        }
        "CommonTableExpr" => {
            names.extend(map.get("ctename").and_then(Value::as_str).map(String::from));
        }
        "FuncCall" => {
            names.extend(names_of(map.get("funcname")).last().cloned());
        }
        _ => {}
    });
    names
}

/// Whether no WITH query around the call takes the name of a table the body reads.
fn clear_of_with(body: &Value, callers_with: &HashSet<String>) -> bool {
    let mut read = HashSet::new();
    each_object("", body, &mut |key, map| {
        if key == "RangeVar" && map.get("schemaname").and_then(Value::as_str).unwrap_or("").is_empty() {
            read.extend(map.get("relname").and_then(Value::as_str).map(String::from));
        }
    });
    read.is_disjoint(callers_with)
}

/// Whether a function's settings leave its body reading as the session reads it. Any setting but a
/// search path may change how a string body parses (`transform_null_equals`). A search path is the
/// session's to find names through, so a string body under one of its own must name its schemas and hold
/// no WITH, whose scope is not read here. And every function it reaches runs under that path too, at
/// every level, so any body with a setting may call only `pg_catalog`'s.
fn settings_allow(f: &Function, body: &Value, functions: &[Function]) -> bool {
    if f.settings.is_empty() {
        return true;
    }
    let string = f.atomic.is_none();
    if string && f.settings.iter().any(|s| !s.starts_with("search_path=")) {
        return false;
    }
    let mut holds = true;
    each_object("", body, &mut |key, map| match key {
        "RangeVar" if string => holds &= !map.get("schemaname").and_then(Value::as_str).unwrap_or("").is_empty(),
        "CommonTableExpr" if string => holds = false,
        "FuncCall" => {
            let names = names_of(map.get("funcname"));
            let Some(name) = names.last() else { return };
            let mut of_name = functions.iter().filter(|f| f.name == *name).peekable();
            holds &= of_name.peek().is_some() && of_name.all(|f| f.schema == "pg_catalog");
        }
        _ => {}
    });
    holds
}

/// The names of the functions a body calls.
fn called_names(body: &Value) -> Vec<String> {
    let mut names = Vec::new();
    each_object("", body, &mut |key, map| {
        if key == "FuncCall" {
            names.extend(names_of(map.get("funcname")).last().cloned());
        }
    });
    names.sort_unstable();
    names.dedup();
    names
}

/// Whether a body's one expression may stand in the caller's query in place of a scalar subquery: no
/// subquery, and no call that is another query's there, as an aggregate, a window call or a call that
/// returns a set would be.
fn inlines(body: &Value, functions: &[Function]) -> bool {
    let mut holds = true;
    each_object("", body, &mut |key, map| match key {
        "SubLink" => holds = false,
        "FuncCall" => {
            let name = names_of(map.get("funcname")).last().cloned().unwrap_or_default();
            let mut of_name = functions.iter().filter(|f| f.name == name).peekable();
            holds &= of_name.peek().is_some() && of_name.all(|f| f.kind == "f" && !f.set) && map.get("over").is_none_or(Value::is_null);
        }
        _ => {}
    });
    holds
}

/// The text of each top-level, comma-separated part of `text`, skipping strings, comments and
/// quoted names.
fn split_arguments(text: &str) -> Option<Vec<String>> {
    if text.trim().is_empty() {
        return Some(Vec::new());
    }
    let opaque = lex::lex(text, Dialect::Postgres).ok()?.opaque;
    let b = text.as_bytes();
    let (mut depth, mut i, mut last) = (0i32, 0usize, 0usize);
    let mut parts = Vec::new();
    while i < b.len() {
        if let Some(&(_, end)) = opaque.iter().find(|&&(s, _)| s == i) {
            i = end.max(i + 1);
            continue;
        }
        match b[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b',' if depth == 0 => {
                parts.push(text[last..i].to_string());
                last = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(text[last..].to_string());
    Some(parts)
}

/// Where a call's name starts and its parentheses are, in `sql`: `(name start, open, close)`.
fn call_span(sql: &str, opaque: &[(usize, usize)], start: usize) -> Option<(usize, usize, usize)> {
    let b = sql.as_bytes();
    let mut i = start;
    while i < b.len() && b[i] != b'(' {
        if let Some(&(_, end)) = opaque.iter().find(|&&(s, _)| s == i) {
            i = end;
        } else {
            i += 1;
        }
    }
    let open = i;
    let mut depth = 0;
    while i < b.len() {
        if let Some(&(_, end)) = opaque.iter().find(|&&(s, _)| s == i) {
            i = end.max(i + 1);
            continue;
        }
        match b[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, open, i));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// A call the expansion may replace.
struct Call<'a> {
    /// Whether it is a whole entry of a select list with no alias.
    target: bool,
    /// Whether it stands in a select list that neither groups nor is distinct, where its value
    /// decides nothing of how many rows there are.
    in_select_list: bool,
    schema: Option<String>,
    name: String,
    location: usize,
    call: &'a Map<String, Value>,
    /// As a FROM item: `(its alias, whether it writes LATERAL, whether the alias lists columns)`.
    item: Option<(Option<String>, bool, bool)>,
}

/// The calls that decide nothing of how many rows any query around them returns: those of a select
/// list that neither groups nor is distinct, in the statement, a WITH query, a FROM item, a set
/// operation's arm, or a subquery that itself stands in such a list. A call in a WHERE, an ON, a GROUP
/// BY or an ORDER BY, at any depth, is none of them, for a proof may read its value as one value.
fn select_list_calls(select: &Map<String, Value>, out: &mut Vec<i64>) {
    fn expressions(value: &Value, out: &mut Vec<i64>) {
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    match k.as_str() {
                        "SubLink" => {
                            if let Some(("SelectStmt", inner)) = v.get("subselect").and_then(node_of) {
                                select_list_calls(inner, out);
                            }
                        }
                        "FuncCall" => {
                            out.extend(v.get("location").and_then(Value::as_i64));
                            expressions(v, out);
                        }
                        _ => expressions(v, out),
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|v| expressions(v, out)),
            _ => {}
        }
    }
    fn from_items(value: &Value, out: &mut Vec<i64>) {
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    match k.as_str() {
                        "RangeSubselect" => {
                            if let Some(("SelectStmt", inner)) = v.get("subquery").and_then(node_of) {
                                select_list_calls(inner, out);
                            }
                        }
                        "JoinExpr" => {
                            from_items(v.get("larg").unwrap_or(&Value::Null), out);
                            from_items(v.get("rarg").unwrap_or(&Value::Null), out);
                        }
                        "node" => from_items(v, out),
                        _ => {}
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|v| from_items(v, out)),
            _ => {}
        }
    }
    let empty = |k: &str| select.get(k).and_then(Value::as_array).is_none_or(Vec::is_empty);
    if empty("distinct_clause") && empty("group_clause") {
        if let Some(targets) = select.get("target_list") {
            expressions(targets, out);
        }
    }
    if let Some(items) = select.get("from_clause") {
        from_items(items, out);
    }
    if let Some(ctes) = select.get("with_clause").and_then(|w| w.get("ctes")).and_then(Value::as_array) {
        for cte in ctes {
            if let Some(("SelectStmt", inner)) = cte.pointer("/node/CommonTableExpr/ctequery").and_then(node_of) {
                select_list_calls(inner, out);
            }
        }
    }
    for arm in ["larg", "rarg"] {
        if let Some(inner) = select.get(arm).and_then(Value::as_object) {
            select_list_calls(inner, out);
        }
    }
}

/// The calls in a parsed statement, in text order. A call in `ROWS FROM`, `WITH ORDINALITY` or with a
/// column definition list, and an aggregate's or a window function's form, is none.
fn calls_in(value: &Value) -> Vec<Call<'_>> {
    let mut items: Vec<(&Map<String, Value>, Option<String>, bool, bool)> = Vec::new();
    let mut skipped: Vec<i64> = Vec::new();
    each_object("", value, &mut |key, map| {
        if key != "RangeFunction" {
            return;
        }
        let functions = map.get("functions").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
        let call = functions.first().and_then(|f| f.pointer("/node/List/items/0/node/FuncCall")).and_then(Value::as_object);
        let plain = !map.get("is_rowsfrom").and_then(Value::as_bool).unwrap_or(false) && !map.get("ordinality").and_then(Value::as_bool).unwrap_or(false) && functions.len() == 1 && map.get("coldeflist").and_then(Value::as_array).is_none_or(Vec::is_empty);
        if let Some(call) = call {
            if plain {
                let alias = at(&Value::Object(map.clone()), &["alias", "aliasname"]).and_then(Value::as_str).map(String::from);
                let listed = !names_of(map.get("alias").and_then(|a| a.get("colnames"))).is_empty();
                items.push((call, alias, map.get("lateral").and_then(Value::as_bool).unwrap_or(false), listed));
            } else {
                skipped.extend(call.get("location").and_then(Value::as_i64));
            }
        }
    });
    let mut targets: Vec<i64> = Vec::new();
    each_object("", value, &mut |key, map| {
        if key == "ResTarget" && map.get("name").and_then(Value::as_str).unwrap_or("").is_empty() {
            if let Some(("FuncCall", c)) = map.get("val").and_then(node_of) {
                targets.extend(c.get("location").and_then(Value::as_i64));
            }
        }
    });
    let mut listed: Vec<i64> = Vec::new();
    if let Some(statement) = value.pointer("/stmts/0/stmt").and_then(node_of) {
        if statement.0 == "SelectStmt" {
            select_list_calls(statement.1, &mut listed);
        }
    }
    let mut calls = Vec::new();
    each_object("", value, &mut |key, map| {
        if key != "FuncCall" {
            return;
        }
        let location = map.get("location").and_then(Value::as_i64).unwrap_or(-1);
        let plain = map.get("agg_order").and_then(Value::as_array).is_none_or(Vec::is_empty)
            && map.get("agg_filter").is_none_or(Value::is_null)
            && map.get("over").is_none_or(Value::is_null)
            && !map.get("agg_star").and_then(Value::as_bool).unwrap_or(false)
            && !map.get("agg_distinct").and_then(Value::as_bool).unwrap_or(false)
            && !map.get("agg_within_group").and_then(Value::as_bool).unwrap_or(false);
        if !plain || location < 0 || skipped.contains(&location) {
            return;
        }
        let names = names_of(map.get("funcname"));
        let (schema, name) = match names.as_slice() {
            [name] => (None, name.clone()),
            [schema, name] => (Some(schema.clone()), name.clone()),
            _ => return,
        };
        let item = items.iter().find(|(c, _, _, _)| c.get("location").and_then(Value::as_i64) == Some(location)).map(|(_, alias, lateral, listed)| (alias.clone(), *lateral, *listed));
        calls.push(Call { target: targets.contains(&location), in_select_list: listed.contains(&location), schema, name, location: location as usize, call: map, item });
    });
    calls.sort_by_key(|c| c.location);
    calls
}

/// The arguments of a call, in the function's order, its defaults in place of those it leaves out.
fn arguments_of(sql: &str, opaque: &[(usize, usize)], call: &Call, f: &Function, nullable: &HashSet<String>) -> Option<(Vec<Argument>, usize)> {
    let (_, open, close) = call_span(sql, opaque, call.location)?;
    let written = split_arguments(&sql[open + 1..close])?;
    let nodes = call.call.get("args").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    if written.len() != nodes.len() {
        return None;
    }
    let count = f.input_types.len();
    let mut slots: Vec<Option<String>> = vec![None; count];
    let mut position = 0;
    for (text, node) in written.iter().zip(nodes) {
        if let Some(("NamedArgExpr", named)) = node_of(node) {
            let name = named.get("name").and_then(Value::as_str)?;
            let at = f.input(name)?;
            let split = text.find("=>").or_else(|| text.find(":="))?;
            *slots.get_mut(at)? = Some(text[split + 2..].to_string());
        } else {
            if position >= count && !f.variadic {
                return None;
            }
            position += 1;
        }
    }
    let positional: Vec<&String> = written.iter().zip(nodes).filter(|(_, n)| !matches!(node_of(n), Some(("NamedArgExpr", _)))).map(|(t, _)| t).collect();
    if f.variadic {
        let last = count.checked_sub(1)?;
        if call.call.get("func_variadic").and_then(Value::as_bool).unwrap_or(false) {
            let text = positional.last()?.trim();
            let lower = text.to_ascii_lowercase();
            let rest = lower.strip_prefix("variadic")?;
            slots[last] = Some(text[text.len() - rest.len()..].to_string());
            for (i, text) in positional[..positional.len() - 1].iter().enumerate() {
                *slots.get_mut(i)? = Some((*text).clone());
            }
        } else {
            for (i, text) in positional.iter().take(last).enumerate() {
                *slots.get_mut(i)? = Some((*text).clone());
            }
            let rest: Vec<String> = positional.iter().skip(last).map(|t| (*t).clone()).collect();
            if rest.is_empty() {
                return None;
            }
            slots[last] = Some(format!("ARRAY[{}]", rest.join(", ")));
        }
    } else {
        for (i, text) in positional.iter().enumerate() {
            *slots.get_mut(i)? = Some((*text).clone());
        }
    }
    let defaults: Vec<String> = match &f.defaults {
        Some(text) => split_arguments(text)?,
        None => Vec::new(),
    };
    if defaults.len() != f.default_count {
        return None;
    }
    let first_default = count.checked_sub(f.default_count)?;
    let mut args = Vec::with_capacity(count);
    let mut defaulted = 0;
    for (i, slot) in slots.into_iter().enumerate() {
        let text = match slot {
            Some(text) => text,
            None => {
                defaulted += 1;
                defaults.get(i.checked_sub(first_default)?)?.clone()
            }
        };
        args.push(Argument::read(&text, nullable)?);
    }
    Some((args, defaulted))
}

/// A body with each `$n` that names nothing the argument as written, and each other a parameter of
/// its own, from `next`: with those arguments' text, which the caller's scope reads, in `$`'s order.
fn isolated(body: &str, f: &Function, args: &[Argument], next: &mut usize) -> Option<(String, Vec<String>, Vec<String>)> {
    let lexed = lex::lex(body, Dialect::Postgres).ok()?;
    let mut numbers: Vec<Option<usize>> = vec![None; args.len()];
    let (mut fresh, mut reads) = (Vec::new(), Vec::new());
    // A call has one collation, for all that it takes, which a column or a COLLATE among its arguments
    // gives it and a parameter does not: a COLLATE on each keeps any of them from pinning a key.
    let collated = args.iter().enumerate().any(|(i, a)| f.input_collatable.get(i) == Some(&true) && (!a.names_nothing() || a.collates));
    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    for p in &lexed.placeholders {
        let n: usize = p.name.parse().ok()?;
        let at = n.checked_sub(1)?;
        let arg = args.get(at)?;
        out.push_str(&body[last..p.start]);
        if arg.names_nothing() {
            out.push_str(&format!("({})", arg.text));
        } else {
            let number = *numbers[at].get_or_insert_with(|| {
                let number = *next;
                *next += 1;
                fresh.push(number.to_string());
                reads.push(arg.text.clone());
                number
            });
            out.push_str(&format!("${number}"));
        }
        last = p.end;
        let cast = format!("::{})", f.input_types.get(at)?);
        if collated && f.input_collatable.get(at) == Some(&true) && body[p.end..].starts_with(&cast) {
            out.push_str(&format!("{} COLLATE \"C\")", cast.trim_end_matches(')')));
            last = p.end + cast.len();
        }
    }
    out.push_str(&body[last..]);
    Some((out, fresh, reads))
}

/// A body of one expression with each `$n` the caller's expression: cast to the declared type, as the
/// call coerces it; and a field of a whole row (`t.archived` where the call passes `t`) as that row's
/// column, which the caller's FROM reads as its own. `None` for a whole row read any other way, and
/// for a body that reads no argument, which may be a constant that an ORDER BY reads as a position.
fn inline_text(body: &str, f: &Function, args: &[Argument]) -> Option<String> {
    let lexed = lex::lex(body, Dialect::Postgres).ok()?;
    if lexed.placeholders.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    for p in &lexed.placeholders {
        let n: usize = p.name.parse().ok()?;
        let arg = args.get(n.checked_sub(1)?)?;
        let ty = f.input_types.get(n - 1)?;
        // `((${n}::type)).field`, as `argument_reference` writes a field of a row, and `(${n}::type).field`,
        // as `typed_numbers` writes one the body numbers.
        let double = format!("::{ty})).");
        let single = format!("::{ty}).");
        let opening = if p.start >= 2 && body.get(p.start - 2..p.start) == Some("((") && body[p.end..].starts_with(&double) {
            Some((2, double.len()))
        } else if p.start >= 1 && body.get(p.start - 1..p.start) == Some("(") && body[p.end..].starts_with(&single) {
            Some((1, single.len()))
        } else {
            None
        };
        match (&arg.whole_row, opening) {
            (Some(row), Some((open, close))) => {
                out.push_str(&body[last..p.start - open]);
                out.push_str(&identifier(row));
                out.push('.');
                last = p.end + close;
            }
            (Some(_), None) => return None,
            (None, _) => {
                out.push_str(&body[last..p.start]);
                out.push('(');
                out.push_str(&arg.text);
                out.push(')');
                last = p.end;
            }
        }
    }
    out.push_str(&body[last..]);
    Some(out)
}

/// The expression of a body that is `SELECT <expression>` and nothing more, as the planner inlines it.
fn bare_expression(body: &str) -> Option<String> {
    use pg_query::protobuf::node::Node as N;
    let parsed = crate::stack::parse(body).ok()?;
    let [stmt] = parsed.protobuf.stmts.as_slice() else { return None };
    let Some(N::SelectStmt(s)) = stmt.stmt.as_deref()?.node.as_ref() else { return None };
    let simple = s.target_list.len() == 1 && s.from_clause.is_empty() && s.where_clause.is_none() && s.group_clause.is_empty() && s.having_clause.is_none() && s.distinct_clause.is_empty() && s.limit_count.is_none() && s.limit_offset.is_none() && s.with_clause.is_none() && s.larg.is_none() && s.values_lists.is_empty() && s.sort_clause.is_empty() && s.window_clause.is_empty() && s.locking_clause.is_empty();
    if !simple {
        return None;
    }
    // The server's and the deparser's own text, under an alias only this function writes.
    let text = crate::stack::deparse(&parsed.protobuf).ok()?;
    let text = text.strip_prefix("SELECT ")?;
    let name = match s.target_list[0].node.as_ref() {
        Some(N::ResTarget(r)) if !r.name.is_empty() => Some(r.name.clone()),
        _ => None,
    };
    Some(match name {
        Some(name) => text.rsplit_once(" AS ").filter(|(_, alias)| alias.trim_matches('"') == name)?.0.to_string(),
        None => text.to_string(),
    })
}

/// What replaces one call.
struct Piece {
    start: usize,
    end: usize,
    text: String,
    /// The parameters it adds for arguments, which may be null.
    fresh: Vec<String>,
}

/// An expression call's text where the body is one expression and the arguments are columns: the
/// planner's own inlining, which is safe to write because such a body binds no name. `None` for
/// any other call.
fn inline(sql: &str, f: &Function, body: &Body, value: &Value, args: &mut [Argument], functions: &[Function], host: &Host) -> Lazy<Option<String>> {
    let plain = args.iter().all(|a| a.calls.is_empty() && !a.subquery && a.columns.iter().all(|c| matches!(c.len(), 1 | 2)));
    if !plain || !args.iter().any(|a| !a.columns.is_empty()) || !inlines(value, functions) {
        return Some(None);
    }
    // A bare name is a column before it is a FROM item's whole row, so it is the whole row only where
    // no FROM in scope shows a column of that name: a table's, from the catalog, and a derived table's,
    // a WITH query's or a function's, which a statement holding one leaves unknown.
    if args.iter().any(|a| a.bare.is_some()) {
        let relations = analyze::analyze(sql).map(|a| a.read_set).unwrap_or_default();
        let rows = host.query_each(cardinality::COLUMNS_QUERY, relations.iter().map(|r| vec![json!([r]), json!(1)]), Scope::Statement)?;
        let columns: Vec<String> = rows.unwrap_or_default().iter().filter_map(|r| r.get(1).and_then(as_strings)).flatten().collect();
        let statement = tree(sql).map(|(statement, _)| statement);
        for a in args.iter_mut() {
            if let Some(name) = &a.bare {
                if !columns.contains(name) {
                    if statement.as_ref().is_none_or(|s| may_show(s, name)) {
                        return Some(None);
                    }
                    a.whole_row = Some(name.clone());
                }
            }
        }
    }
    let Some(text) = inline_text(&body.text, f, args) else { return Some(None) };
    Some(bare_expression(&text))
}

/// What replaces one call.
#[allow(clippy::too_many_arguments)]
fn replacement(sql: &str, opaque: &[(usize, usize)], call: &Call, f: &Function, body: &Body, callers_with: &HashSet<String>, nullable: &HashSet<String>, next: &mut usize, host: &Host) -> Lazy<Option<Piece>> {
    let Some((mut args, _)) = arguments_of(sql, opaque, call, f, nullable) else { return Some(None) };
    let Some((value, parsed)) = tree(&body.text) else { return Some(None) };
    if !clear_of_with(&value, callers_with) {
        return Some(None);
    }
    // The server prints a parameter of a body stored parsed under the function's name, which an item
    // of that name would read back as its own.
    if f.atomic.is_some() && bound_names(&value).contains(&f.name) {
        return Some(None);
    }
    let functions = functions_named(&called_names(&value), host)?;
    if !settings_allow(f, &value, &functions) {
        return Some(None);
    }
    // An argument's aggregate, window call or call that returns a set is the caller's own, and no
    // subquery may hold it.
    let argument_calls: Vec<String> = args.iter().flat_map(|a| a.calls.iter().cloned()).collect();
    let called_there = functions_named(&argument_calls, host)?;
    if args.iter().any(|a| a.aggregates) || argument_calls.iter().any(|name| !called_there.iter().any(|f| f.name == *name) || called_there.iter().any(|f| f.name == *name && (f.kind != "f" || f.set))) {
        return Some(None);
    }
    // A call of one argument that names something may be no call: `price(t)` is `t.price` where no
    // function of the name takes the row. A name qualified with its schema is only ever a function,
    // so the server says which by preparing the statement with one.
    if let ([arg], None) = (args.as_slice(), &call.schema) {
        if !arg.names_nothing() {
            let qualified = format!("{}{}.{}", &sql[..call.location], identifier(&f.schema), &sql[call.location..]);
            if host.describe(&qualified)?.is_err() {
                return Some(None);
            }
        }
    }
    let Some((start, _, close)) = call_span(sql, opaque, call.location) else { return Some(None) };
    // A user's cast can return NULL for a value, written or implicit: an argument is certain only
    // where each cast it writes, and the one to the type the function takes, keep its nullability.
    let uncertain: Vec<usize> = if f.strict {
        let trust = postgres::trust(host)?;
        let keeps = |name: &String| trust.cast_targets.as_ref().is_none_or(|t| t.contains(name));
        (0..args.len()).filter(|i| !(args[*i].never_null && args[*i].casts.iter().all(keeps) && f.input_typenames.get(*i).is_some_and(keeps))).collect()
    } else {
        Vec::new()
    };
    // In FROM a non-set function needs the proof that its body returns exactly one row.
    let body_columns = |renames: &str, names: &[String], reads: &[String], text: &str| -> String {
        let list = names.iter().zip(&body.casts).map(|(n, cast)| match cast {
            Some(ty) => format!("reylo_body.{n}::{ty} AS {n}"),
            None => format!("reylo_body.{n}"),
        }).collect::<Vec<_>>().join(", ");
        let from_args = if reads.is_empty() {
            String::new()
        } else {
            format!("(SELECT {}) AS reylo_args, ", reads.iter().enumerate().map(|(i, r)| format!("{r} AS a{i}")).collect::<Vec<_>>().join(", "))
        };
        format!("SELECT {list} FROM {from_args}({text}) AS reylo_body{renames}")
    };
    match &call.item {
        None => {
            if f.set || !f.one_value {
                return Some(None);
            }
            let inlined = inline(sql, f, body, &value, &mut args, &functions, host)?;
            // Outside such a select list a proof may read the value as one value. A subquery of the
            // body reads as one however the caller's row changes, and a volatile function runs again
            // for each row, so neither is written there: the call stays a call.
            if !call.in_select_list && (f.volatile || (inlined.is_none() && args.iter().any(|a| !a.names_nothing()))) {
                return Some(None);
            }
            let (text, fresh) = match inlined {
                Some(expression) => (expression, Vec::new()),
                None => {
                    let Some((text, fresh, _)) = isolated(&body.text, f, &args, next) else { return Some(None) };
                    (text, fresh)
                }
            };
            let value = format!("({text})");
            let expression = match &body.casts[0] {
                Some(ty) => format!("({value})::{ty}"),
                None => value,
            };
            let expression = if uncertain.is_empty() {
                expression
            } else {
                // A strict function returns NULL for a NULL argument without running its body.
                let any = uncertain.iter().map(|i| format!("({}) IS NULL", args[*i].text)).collect::<Vec<_>>().join(" OR ");
                format!("(CASE WHEN {any} THEN NULL ELSE {expression} END)")
            };
            // A select list's bare call names its column after the function.
            let text = if call.target { format!("{expression} AS {}", identifier(&f.name)) } else { expression };
            Some(Some(Piece { start, end: close + 1, text, fresh }))
        }
        Some((alias, lateral, listed)) => {
            // A one-value function's item read as a value is that value, and a derived table's is a
            // row. Only a plain function's column goes by the item's own name, so there `c` is the
            // value both ways, and `c.*` as a select list's entry its columns both ways; `c.*` as a
            // value is a row of the derived table alone.
            if f.one_value {
                let item = alias.clone().unwrap_or_else(|| f.name.clone());
                let star = |map: &serde_json::Map<String, Value>| {
                    let names = names_of(map.get("fields"));
                    names.len() == 1 && names[0] == item && map.get("fields").and_then(Value::as_array).is_some_and(|fields| fields.len() == 2)
                };
                let (mut bare, mut stars, mut entries) = (false, 0, 0);
                if let Some((statement, _)) = tree(sql) {
                    each_object("", &statement, &mut |key, map| match key {
                        "ColumnRef" => {
                            let names = names_of(map.get("fields"));
                            bare |= names.len() == 1 && names[0] == item && !star(map);
                            stars += usize::from(star(map));
                        }
                        "ResTarget" => {
                            if let Some(("ColumnRef", c)) = map.get("val").and_then(node_of) {
                                entries += usize::from(star(c));
                            }
                        }
                        _ => {}
                    });
                }
                if stars > entries || ((bare || stars > 0) && (!f.scalar || *listed)) {
                    return Some(None);
                }
            }
            let Some((text, mut fresh, reads)) = isolated(&body.text, f, &args, next) else { return Some(None) };
            if !f.set {
                // One row, whatever the body returns: read only where the body returns exactly one,
                // and is not skipped for a NULL argument.
                if !uncertain.is_empty() {
                    return Some(None);
                }
                let Some(statement) = parsed_text(&parsed) else { return Some(None) };
                let analyzed = analyze::analyze(&statement).ok();
                let functions = analyzed.as_ref().map(|a| a.functions.clone()).unwrap_or_default();
                let relations: Vec<String> = analyzed.iter().flat_map(|a| a.read_set.iter().cloned()).collect();
                let catalog = postgres::catalog(host, &functions, &relations);
                let trust = postgres::trust(host);
                let (Some(catalog), Some(trust)) = (catalog, trust) else { return None };
                if cardinality::exactly_one(&text, Dialect::Postgres, &trust, &catalog).is_err() {
                    return Some(None);
                }
            }
            let names: Vec<String> = f.outputs.iter().enumerate().map(|(i, o)| if i == 0 && f.scalar { identifier(alias.as_deref().unwrap_or(&o.name)) } else { identifier(&o.name) }).collect();
            let reads_columns = args.iter().any(|a| !a.names_nothing());
            let keyword = if reads_columns && !lateral { "LATERAL " } else { "" };
            let name = if alias.is_none() { format!(" AS {}", identifier(&f.name)) } else { String::new() };
            let renames = if body.named { String::new() } else { format!(" ({})", names.join(", ")) };
            let mut inner = body_columns(&renames, &names, &reads, &text);
            // A strict set function returns no row for a NULL argument without running its body.
            let tests: Vec<String> = uncertain.iter().map(|i| {
                let number = *next;
                *next += 1;
                fresh.push(number.to_string());
                format!("(${number}::{}) IS NOT NULL", f.input_types[*i])
            }).collect();
            if !tests.is_empty() {
                inner.push_str(&format!(" WHERE {}", tests.join(" AND ")));
            }
            Some(Some(Piece { start, end: close + 1, text: format!("{keyword}({inner}){name}"), fresh }))
        }
    }
}

fn parsed_text(parsed: &crate::stack::Parsed) -> Option<String> {
    crate::stack::deparse(&parsed.protobuf).ok()
}

/// Which of `rows` a call of `call`'s name is: the one function in scope of that name, else none:
/// which overload a call picks is the server's to say, and the statement cannot ask.
fn called<'f>(call: &Call, functions: &'f [Function]) -> Option<&'f Function> {
    let mut found = functions.iter().filter(|f| {
        f.name == call.name
            && match &call.schema {
                Some(schema) => f.schema == *schema,
                None => f.visible,
            }
    });
    let (Some(one), None) = (found.next(), found.next()) else { return None };
    one.readable().then_some(one)
}

/// The functions of `names`, as the catalog says they are.
fn functions_named(names: &[String], host: &Host) -> Lazy<Vec<Function>> {
    let rows = host.query_each(BODIES_QUERY, names.iter().map(|n| vec![json!([n])]), Scope::Statement)?;
    Some(rows.unwrap_or_default().iter().filter_map(|r| Function::read(r)).collect())
}

/// Asks for everything reading the bodies of `names` will: their catalog rows, each body's names, and
/// the same of what those bodies call, all as soon as each is known. So levels of calls take a round
/// each, side by side with the rounds the statement's own analysis takes, not after them.
pub fn ask(names: &[String], host: &Host) {
    let mut seen = HashSet::new();
    ask_level(names, MAX_LEVELS, &mut seen, host);
}

fn ask_level(names: &[String], levels: usize, seen: &mut HashSet<String>, host: &Host) {
    if levels == 0 {
        return;
    }
    let Some(functions) = functions_named(names, host) else { return };
    let mut called = Vec::new();
    for f in functions.iter().filter(|f| f.readable()) {
        if !seen.insert(f.signature.clone()) {
            continue;
        }
        let _ = resolve(f, host);
        if let Some((value, _)) = body_statement(f).and_then(|b| tree(&b)) {
            called.extend(called_names(&value));
        }
    }
    called.sort_unstable();
    called.dedup();
    if !called.is_empty() {
        ask_level(&called, levels - 1, seen, host);
    }
}

/// A note where a call in FROM runs apart from its query for want of a word: Postgres flattens a
/// function that returns a set only where it says `stable` or `immutable`, and this one's label is the
/// default's, or was written, over a body that calls nothing volatile. Nothing else on the function
/// may stand in the way, or the note would promise what `stable` can't give.
fn volatile_note(call: &Call, f: &Function, body: &Body, host: &Host) -> Lazy<Option<String>> {
    if call.item.is_none() || !f.set || !f.volatile || f.strict || f.security_definer || !f.settings.is_empty() {
        return Some(None);
    }
    let quiet = crate::errors::calls_nothing_volatile(&body.text, host)?;
    Some(quiet.then(|| format!("{} is volatile, but its body calls nothing volatile, so Postgres can't plan it into this query. Marking it stable lets a filter on its rows use an index.", f.signature)))
}

/// A statement with the bodies of the functions it calls in place.
#[derive(Debug, Clone)]
pub struct Expanded {
    pub sql: String,
    /// The functions read, each as `to_regprocedure` reads it back.
    pub functions: Vec<String>,
    /// The parameters it adds for arguments, by the name the lexer gives each, which may be null.
    pub fresh: Vec<String>,
    /// What is worth saying of a function it read, which fails nothing.
    pub notes: Vec<String>,
}

/// The largest parameter number in `sql`.
fn largest_parameter(sql: &str) -> Option<usize> {
    Some(lex::lex(sql, Dialect::Postgres).ok()?.placeholders.iter().filter_map(|p| p.name.parse().ok()).max().unwrap_or(0))
}

/// `sql` with each call of a function whose body is read replaced by that body; `Some(None)` where
/// nothing is replaced.
pub fn expand(sql: &str, nullable: &HashSet<String>, host: &Host) -> Lazy<Option<Expanded>> {
    let mut current = sql.to_string();
    let mut read: Vec<String> = Vec::new();
    let mut fresh: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut nullable = nullable.clone();
    for _ in 0..MAX_LEVELS {
        let Some((value, _)) = tree(&current) else { break };
        let calls = calls_in(&value);
        if calls.is_empty() {
            break;
        }
        let mut names: Vec<String> = calls.iter().map(|c| c.name.clone()).collect();
        names.sort_unstable();
        names.dedup();
        let functions = functions_named(&names, host)?;
        let Ok(lexed) = lex::lex(&current, Dialect::Postgres) else { break };
        let Some(largest) = largest_parameter(&current) else { break };
        let mut next = largest + 1;
        let mut with_names = HashSet::new();
        each_object("", &value, &mut |key, map| {
            if key == "CommonTableExpr" {
                with_names.extend(map.get("ctename").and_then(Value::as_str).map(String::from));
            }
        });
        // Every call of this level, so that one round asks for all of them.
        let mut waiting = false;
        let mut pieces: Vec<(Piece, String)> = Vec::new();
        for call in &calls {
            let Some(f) = called(call, &functions) else { continue };
            let Some(resolved) = resolve(f, host) else {
                waiting = true;
                continue;
            };
            let Some(body) = resolved else { continue };
            // A body that calls itself is read no further than itself.
            let Some((body_value, _)) = tree(&body.text) else { continue };
            if called_names(&body_value).contains(&f.name) {
                continue;
            }
            match replacement(&current, &lexed.opaque, call, f, &body, &with_names, &nullable, &mut next, host) {
                None => waiting = true,
                Some(Some(piece)) => {
                    match volatile_note(call, f, &body, host) {
                        None => waiting = true,
                        Some(Some(note)) if !notes.contains(&note) => notes.push(note),
                        Some(_) => {}
                    }
                    pieces.push((piece, f.signature.clone()));
                }
                Some(None) => {}
            }
        }
        if waiting {
            return None;
        }
        // Calls nested in calls are read at the next level: the outermost of each is replaced now.
        pieces.sort_by_key(|(p, _)| p.start);
        let mut out = String::with_capacity(current.len());
        let mut last = 0;
        let mut any = false;
        for (piece, signature) in pieces {
            if piece.start < last {
                continue;
            }
            out.push_str(&current[last..piece.start]);
            out.push_str(&piece.text);
            last = piece.end;
            any = true;
            // A parameter an argument became may be null in the bodies it reaches next.
            nullable.extend(piece.fresh.iter().cloned());
            fresh.extend(piece.fresh);
            if !read.contains(&signature) {
                read.push(signature);
            }
        }
        if !any {
            break;
        }
        out.push_str(&current[last..]);
        if out.len() > MAX_TEXT {
            break;
        }
        current = out;
    }
    Some((!read.is_empty()).then_some(Expanded { sql: current, functions: read, fresh, notes }))
}

/// `sql` with its parameters numbered in the order they first appear, and the old number of each
/// new one: an expanded statement may read fewer than the statement did, and the server refuses a
/// gap in the numbers.
pub fn renumbered(sql: &str) -> Option<(String, Vec<String>)> {
    let lexed = lex::lex(sql, Dialect::Postgres).ok()?;
    let mut order: Vec<String> = Vec::new();
    let mut out = String::with_capacity(sql.len());
    let mut last = 0;
    for p in &lexed.placeholders {
        let number = match order.iter().position(|n| *n == p.name) {
            Some(i) => i + 1,
            None => {
                order.push(p.name.clone());
                order.len()
            }
        };
        out.push_str(&sql[last..p.start]);
        out.push_str(&format!("${number}"));
        last = p.end;
    }
    out.push_str(&sql[last..]);
    Some((out, order))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reference_ends_after_its_last_part() {
        let end = |text: &str| reference_end(text, 0);
        assert_eq!(end("p = 1"), 1);
        assert_eq!(end("f.p + 1"), 3);
        assert_eq!(end("\"Odd Name\".p)"), 12);
        assert_eq!(end("\"a\"\"b\" x"), 6);
    }

    #[test]
    fn arguments_part_at_the_commas_of_the_call_alone() {
        let parts = |text: &str| split_arguments(text).unwrap();
        assert!(parts("").is_empty());
        assert_eq!(parts("1, f(2, 3), array[4, 5]"), ["1", " f(2, 3)", " array[4, 5]"]);
        assert_eq!(parts("',', \"a,b\""), ["','", " \"a,b\""]);
    }

    #[test]
    fn only_a_select_list_that_decides_no_rows_holds_a_call_that_may_be_written() {
        let listed = |sql: &str| {
            let (value, _) = tree(sql).unwrap();
            calls_in(&value).iter().filter(|c| c.in_select_list).map(|c| c.name.clone()).collect::<Vec<_>>()
        };
        assert_eq!(listed("select f(x) from t"), ["f"]);
        assert_eq!(listed("select (select f(x)) from t"), ["f"]);
        assert_eq!(listed("select s.v from (select f(x) as v from t) s"), ["f"]);
        assert_eq!(listed("with w as (select f(x) as v from t) select v from w"), ["f"]);
        assert_eq!(listed("select f(x) from t union all select f(y) from u"), ["f", "f"]);
        assert!(listed("select 1 from t where f(x) = 1").is_empty());
        assert!(listed("select 1 from t where x = (select f(y))").is_empty());
        assert!(listed("select distinct f(x) from t").is_empty());
        assert!(listed("select distinct (select f(x)) from t").is_empty());
        assert!(listed("select count(*) from t group by f(x)").is_empty());
        assert!(listed("select x from t order by f(x)").is_empty());
    }

    #[test]
    fn a_name_some_item_may_show_is_no_whole_row_for_certain() {
        let shows = |sql: &str, name: &str| may_show(&tree(sql).unwrap().0, name);
        assert!(!shows("select 1 from posts t", "t"));
        assert!(shows("select 1 from (select a as t from b) s, posts t", "t"));
        assert!(shows("select 1 from posts t (a, b)", "t"));
        assert!(shows("select 1 from posts column1, (values (1)) v", "column1"));
        assert!(shows("select 1 from f() a, posts t", "t"));
        assert!(!shows("select x.* from posts x", "x"));
    }
}
