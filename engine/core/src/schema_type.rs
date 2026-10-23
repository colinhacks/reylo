//! The type a JSON Schema gives a JSON value: a column that a CHECK holds to a schema
//! (`jsonb_matches_schema('…', doc)` on Postgres, `json_schema_valid('…', doc)` on MySQL) holds only
//! values the database's validator accepted.
//!
//! The validator decides which values reach the column, so the type is a superset of the values
//! it accepts: a keyword narrows the type only where the validator enforces it, under the draft it
//! applies ([`Validator`]). A keyword that only checks (`minLength`, `pattern`, `format`,
//! `uniqueItems`, the dependencies, `contains`) narrows nothing, and so does a keyword whose effect
//! no type can state exactly (`not` of anything but whole JSON types). Where a reading is in doubt
//! — an unknown draft, a reference that does not resolve, a schema too deep or too large to read —
//! the type is any value, which every value fits. Where the validator may apply one of several
//! drafts, the type is the union of each draft's reading.
//!
//! The answer is JSON Schema again, in a small canonical vocabulary that every printer of Reylo's
//! reads ([`type_schema`]): `true`, `false`, `{}`, `type` (one name or a list), `enum`, `anyOf`, an
//! array's `prefixItems`, `items` and `minItems`, and an object's `properties`, `required` and
//! `additionalProperties`, which is always stated, so that `true` there says the object may hold
//! other keys. [`admits`] checks a value against it.

use serde_json::{json, Map, Number, Value};
use std::collections::HashMap;

/// How deeply a schema is read: a subschema below this depth is any value.
const MAX_DEPTH: usize = 64;
/// How many subschemas one reading reads, references followed again included: past it, any value.
const MAX_READS: usize = 20_000;
/// How many object or array shapes a union keeps before it widens them to any object or array.
const MAX_SHAPES: usize = 32;
/// How deeply the text of a schema may nest before it is read at all.
const MAX_TEXT_DEPTH: usize = 512;

/// A JSON Schema draft, as a validator applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Draft {
    Four,
    Six,
    Seven,
    /// 2019-09.
    Nineteen,
    /// 2020-12.
    Twenty,
}

impl Draft {
    pub const ALL: [Draft; 5] = [Draft::Four, Draft::Six, Draft::Seven, Draft::Nineteen, Draft::Twenty];

    /// The draft a `$schema` URI names, read loosely: either scheme, with or without the empty
    /// fragment. A validator may know only some of these spellings ([`Validator::known`]).
    pub fn named(uri: &str) -> Option<Draft> {
        let uri = uri.trim().trim_end_matches('#');
        let rest = uri.strip_prefix("http://").or_else(|| uri.strip_prefix("https://"))?;
        match rest {
            "json-schema.org/draft-04/schema" => Some(Draft::Four),
            "json-schema.org/draft-06/schema" => Some(Draft::Six),
            "json-schema.org/draft-07/schema" => Some(Draft::Seven),
            "json-schema.org/draft/2019-09/schema" => Some(Draft::Nineteen),
            "json-schema.org/draft/2020-12/schema" => Some(Draft::Twenty),
            _ => None,
        }
    }
}

/// How a database's validator reads a schema: which draft it applies, and how.
#[derive(Debug, Clone, PartialEq)]
pub struct Validator {
    /// The `$schema` values the validator recognizes, exactly as written, and the draft each
    /// selects.
    pub known: &'static [(&'static str, Draft)],
    /// The drafts it may apply to a schema with no `$schema`, or with one it does not recognize.
    /// More than one where the validator's version leaves it open.
    pub fallback: &'static [Draft],
    /// Whether, under 2019-09 and 2020-12, the keywords beside a `$ref` apply as well, as those
    /// drafts say. Through draft 7 they are ignored whatever this says.
    pub ref_siblings: bool,
    /// Whether a JSON Pointer with a `$` in it resolves. RapidJSON's (MySQL's) does not resolve
    /// `#/$defs/s`, and the node then admits anything.
    pub dollar_pointers: bool,
    /// Whether a root `openapi` or `swagger` key, with no `$schema`, switches the validator to
    /// OpenAPI's reading (RapidJSON's: `nullable` admits null, and some keywords are dropped),
    /// which the engine does not follow: such a schema admits any value.
    pub openapi_roots: bool,
    /// Whether the validator enforces an array's prefix: a list under `items` with
    /// `additionalItems`, or `prefixItems`. adunstan's `json_schema_validate` enforces neither, and
    /// reads `items` only as a schema object (`items: false` admits anything there).
    pub tuples: bool,
    /// Whether it enforces `patternProperties`; one that does not still exempts the keys a
    /// pattern matches from `additionalProperties`, so those keys may hold anything.
    pub patterns: bool,
    /// Whether it resolves only a JSON Pointer into the root (`#/…`), and a reference by an
    /// identifier or an anchor admits anything (gavinwahl's postgres-json-schema).
    pub pointer_refs_only: bool,
}

/// A validator that applies its draft as the specification says, from which each profile below
/// departs.
const SPEC: Validator = Validator {
    known: &[],
    fallback: &[],
    ref_siblings: false,
    dollar_pointers: true,
    openapi_roots: false,
    tuples: true,
    patterns: true,
    pointer_refs_only: false,
};

impl Validator {
    /// The drafts the validator may apply to `schema`. A `$schema` it recognizes picks one; a
    /// spelling of a draft it selects otherwise, which it may or may not recognize, adds that
    /// draft to the fallback; anything else is the fallback.
    pub fn drafts(&self, schema: &Value) -> Vec<Draft> {
        let Some(uri) = schema.get("$schema").and_then(Value::as_str) else { return self.fallback.to_vec() };
        if let Some((_, draft)) = self.known.iter().find(|(known, _)| *known == uri) {
            return vec![*draft];
        }
        let mut drafts = self.fallback.to_vec();
        if let Some(draft) = Draft::named(uri).filter(|d| self.known.iter().any(|(_, k)| k == d)) {
            if !drafts.contains(&draft) {
                drafts.push(draft);
            }
        }
        drafts
    }
}

/// Reylo's own check of a schema a statement writes in a parameter's column list (`meta
/// json('…')`): 2020-12, as the specification says, whatever `$schema` says, since the decision of
/// 2026-10-12 takes JSON Schema 2020-12 in its entirety (`wiki/typed-json-params.md`).
pub const REYLO: Validator = Validator { fallback: &[Draft::Twenty], ..SPEC };

/// MySQL's `JSON_SCHEMA_VALID` (8.0.17 and later): RapidJSON's draft 4 validator, whatever `$schema`
/// says. It leaves a pointer with a `$` in it unresolved, and reads a root `openapi` or `swagger`
/// key as OpenAPI (measured on 8.0.46, 8.4.11, 9.7.2 and 26.7.0: `wiki/json-schema-checks/mysql/`).
pub const MYSQL: Validator = Validator { fallback: &[Draft::Four], dollar_pointers: false, openapi_roots: true, ..SPEC };

/// The `$schema` values the Rust `jsonschema` crate recognizes without its 2019-09 and 2020-12
/// features: draft 4, 6 and 7 exactly as their meta-schemas name themselves.
const DRAFTS_4_6_7: &[(&str, Draft)] = &[
    ("http://json-schema.org/draft-04/schema#", Draft::Four),
    ("http://json-schema.org/draft-06/schema#", Draft::Six),
    ("http://json-schema.org/draft-07/schema#", Draft::Seven),
];

/// The Rust `jsonschema` crate 0.16 and 0.17 as pg_jsonschema 0.1.0 to 0.3.3 and sqlite-jsonschema
/// call it: draft 7 for any `$schema` but draft 4's, 6's and 7's own, and the keywords beside a
/// `$ref` ignored in every draft.
pub const JSONSCHEMA_0_17: Validator = Validator { known: DRAFTS_4_6_7, fallback: &[Draft::Seven], ..SPEC };

/// sqlite-jsonschema 0.2.3's `jsonschema_matches` (the only SQLite JSON Schema validator; it
/// has not changed since 2023-08): the crate 0.16.1 (measured: `wiki/json-schema-checks/sqlite/`).
pub const SQLITE_JSONSCHEMA: Validator = JSONSCHEMA_0_17;

/// The `jsonschema` crate 0.37 as pg_jsonschema 0.3.4 calls it: 2020-12 by default, each draft by
/// its own URIs, with or without the empty fragment, and the keywords beside a `$ref` applied from
/// 2019-09 on. An unknown `$schema` is an error on every row.
pub const JSONSCHEMA_0_37: Validator = Validator {
    known: &[
        ("http://json-schema.org/draft-04/schema#", Draft::Four),
        ("http://json-schema.org/draft-04/schema", Draft::Four),
        ("http://json-schema.org/draft-06/schema#", Draft::Six),
        ("http://json-schema.org/draft-06/schema", Draft::Six),
        ("http://json-schema.org/draft-07/schema#", Draft::Seven),
        ("http://json-schema.org/draft-07/schema", Draft::Seven),
        ("https://json-schema.org/draft-07/schema#", Draft::Seven),
        ("https://json-schema.org/draft/2019-09/schema", Draft::Nineteen),
        ("https://json-schema.org/draft/2019-09/schema#", Draft::Nineteen),
        ("https://json-schema.org/draft/2020-12/schema", Draft::Twenty),
        ("https://json-schema.org/draft/2020-12/schema#", Draft::Twenty),
    ],
    fallback: &[Draft::Twenty],
    ref_siblings: true,
    ..SPEC
};

/// Supabase's pg_jsonschema, at any version: the crate 0.17's reading or 0.37's. The version the
/// catalog records does not say which code runs (Supabase's images point every version at one
/// library), so a schema is read both ways (measured: `wiki/json-schema-checks/postgres/`).
pub const PG_JSONSCHEMA: &[&Validator] = &[&JSONSCHEMA_0_17, &JSONSCHEMA_0_37];

/// Tembo's (now theory's) `jsonschema` extension, the boon crate: the draft a `$schema` names, and
/// for a schema with none whichever draft the session's `jsonschema.default_draft` names, which
/// any session may set.
pub const BOON: Validator = Validator {
    known: &[
        ("http://json-schema.org/draft-04/schema#", Draft::Four),
        ("http://json-schema.org/draft-06/schema#", Draft::Six),
        ("http://json-schema.org/draft-07/schema#", Draft::Seven),
        ("https://json-schema.org/draft/2019-09/schema", Draft::Nineteen),
        ("https://json-schema.org/draft/2020-12/schema", Draft::Twenty),
    ],
    fallback: &[Draft::Four, Draft::Six, Draft::Seven, Draft::Nineteen, Draft::Twenty],
    ref_siblings: true,
    ..SPEC
};

/// gavinwahl's postgres-json-schema (PL/pgSQL): a subset of draft 4, `$schema` ignored, and only a
/// `#/…` pointer resolved.
pub const GAVINWAHL: Validator = Validator { fallback: &[Draft::Four], pointer_refs_only: true, ..SPEC };

/// furstenheim's is_jsonb_valid: `is_jsonb_valid` reads draft 4, `is_jsonb_valid_draft_v7` draft 7,
/// whatever `$schema` says.
pub const IS_JSONB_VALID_4: Validator = Validator { fallback: &[Draft::Four], ..SPEC };
pub const IS_JSONB_VALID_7: Validator = Validator { fallback: &[Draft::Seven], ..SPEC };

/// adunstan's json_schema_validate: most of draft 7, `$schema` ignored, with no tuples and no
/// `patternProperties`.
pub const ADUNSTAN: Validator = Validator { fallback: &[Draft::Seven], tuples: false, patterns: false, ..SPEC };

/// The validators behind a function a Postgres CHECK calls, by the extension that defines it (the
/// same names mean different validators in pg_jsonschema and theory's `jsonschema`), with the
/// number of arguments the call takes and the positions of the schema and the document among
/// them; `None` for a function the engine does not know as one. Every one of them is STRICT, or
/// returns true or false for a value that is not NULL. A schema is read under each validator of
/// the list, and its values are those any of them admits.
pub fn postgres_validator(function: &str, extension: &str) -> Option<(&'static [&'static Validator], usize, usize, usize)> {
    Some(match (extension, function) {
        ("pg_jsonschema", "json_matches_schema" | "jsonb_matches_schema") => (PG_JSONSCHEMA, 2, 0, 1),
        ("jsonschema", "json_matches_schema" | "jsonb_matches_schema") => (&[&BOON], 2, 0, 1),
        ("jsonschema", "jsonschema_validates") => (&[&BOON], 2, 1, 0),
        ("postgres-json-schema", "validate_json_schema") => (&[&GAVINWAHL], 2, 0, 1),
        ("is_jsonb_valid", "is_jsonb_valid") => (&[&IS_JSONB_VALID_4], 2, 0, 1),
        ("is_jsonb_valid", "is_jsonb_valid_draft_v7") => (&[&IS_JSONB_VALID_7], 2, 0, 1),
        ("json_schema_validate", "jsonschema_is_valid") => (&[&ADUNSTAN], 2, 1, 0),
        _ => return None,
    })
}

/// The canonical schema of the values `schema` admits under `validator`, or a superset of them.
pub fn type_schema(schema: &Value, validator: &Validator) -> Value {
    emit(&read_schema(schema, validator))
}

/// The canonical schema of the values `schema` admits under any of `validators`.
pub fn type_schema_any(schema: &Value, validators: &[&Validator]) -> Value {
    emit(&read_any(schema, validators))
}

fn read_any(schema: &Value, validators: &[&Validator]) -> Ty {
    validators.iter().map(|v| read_schema(schema, v)).reduce(|a, b| a.union(&b)).unwrap_or_else(Ty::any)
}

/// The canonical schema of the values every schema of `texts` admits under its validator, as the
/// CHECKs of one column give them: `None` where that is any value, for texts that say nothing the
/// engine reads or are not JSON.
pub fn checked_type(texts: &[(String, &[&Validator])]) -> Option<Value> {
    let mut ty = Ty::any();
    for (text, validators) in texts {
        if text_depth(text) > MAX_TEXT_DEPTH {
            continue;
        }
        if let Ok(schema) = serde_json::from_str::<Value>(text) {
            ty = ty.and(&read_any(&schema, validators));
        }
    }
    (!ty.is_any()).then(|| emit(&ty))
}

/// [`type_schema`] of a schema given as text, as a CHECK carries it; `None` for text that is not
/// JSON, or nests deeper than the engine reads.
pub fn type_schema_text(text: &str, validator: &Validator) -> Option<Value> {
    if text_depth(text) > MAX_TEXT_DEPTH {
        return None;
    }
    let schema: Value = serde_json::from_str(text).ok()?;
    Some(type_schema(&schema, validator))
}

fn read_schema(schema: &Value, validator: &Validator) -> Ty {
    if validator.openapi_roots && schema.get("$schema").is_none() && (schema.get("openapi").is_some() || schema.get("swagger").is_some()) {
        return Ty::any();
    }
    let mut out: Option<Ty> = None;
    for draft in validator.drafts(schema) {
        let ty = Reader::new(schema, draft, validator).read_root();
        out = Some(match out {
            Some(prior) => prior.union(&ty),
            None => ty,
        });
    }
    out.unwrap_or_else(Ty::any)
}

/// How deeply JSON text nests, strings skipped.
fn text_depth(text: &str) -> usize {
    let (mut depth, mut max, mut in_string, mut escaped) = (0usize, 0usize, false, false);
    for c in text.chars() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '[' | '{' => {
                depth += 1;
                max = max.max(depth);
            }
            ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max
}

// ---------------------------------------------------------------------------------------------
// The type: per JSON type, which of its values
// ---------------------------------------------------------------------------------------------

/// Which values of a JSON type with no inner structure: none, all, or these.
#[derive(Debug, Clone, PartialEq)]
enum Lits<T> {
    No,
    All,
    Some(Vec<T>),
}

/// Which numbers: none, all of them or the integers among them, or these.
#[derive(Debug, Clone, PartialEq)]
enum Num {
    No,
    All { integer: bool },
    Some(Vec<Number>),
}

/// Which arrays or objects: all of them, or a union of shapes (none where it is empty).
#[derive(Debug, Clone, PartialEq)]
enum Shapes<S> {
    All,
    Some(Vec<S>),
}

/// A set of JSON values, per JSON type.
#[derive(Debug, Clone, PartialEq)]
struct Ty {
    null: bool,
    boolean: Lits<bool>,
    number: Num,
    string: Lits<String>,
    arrays: Shapes<Arr>,
    objects: Shapes<Obj>,
}

/// Arrays whose element `i` is `prefix[i]`, or `rest` past the prefix, and which hold at least
/// `min` elements.
#[derive(Debug, Clone, PartialEq)]
struct Arr {
    prefix: Vec<Ty>,
    rest: Box<Ty>,
    min: u64,
}

/// Objects whose key `name` holds `ty` (and must be there where `required`), and whose every
/// other key holds `rest`.
#[derive(Debug, Clone, PartialEq)]
struct Obj {
    props: Vec<Prop>,
    rest: Box<Ty>,
}

#[derive(Debug, Clone, PartialEq)]
struct Prop {
    name: String,
    ty: Ty,
    required: bool,
}

impl Ty {
    fn any() -> Ty {
        Ty { null: true, boolean: Lits::All, number: Num::All { integer: false }, string: Lits::All, arrays: Shapes::All, objects: Shapes::All }
    }

    fn never() -> Ty {
        Ty { null: false, boolean: Lits::No, number: Num::No, string: Lits::No, arrays: Shapes::Some(Vec::new()), objects: Shapes::Some(Vec::new()) }
    }

    fn is_any(&self) -> bool {
        *self == Ty::any()
    }

    fn is_never(&self) -> bool {
        *self == Ty::never()
    }

    /// Any value of the JSON type `name`; `None` for a name that is no JSON type.
    fn kind(name: &str) -> Option<Ty> {
        let none = Ty::never();
        Some(match name {
            "null" => Ty { null: true, ..none },
            "boolean" => Ty { boolean: Lits::All, ..none },
            "integer" => Ty { number: Num::All { integer: true }, ..none },
            "number" => Ty { number: Num::All { integer: false }, ..none },
            "string" => Ty { string: Lits::All, ..none },
            "array" => Ty { arrays: Shapes::All, ..none },
            "object" => Ty { objects: Shapes::All, ..none },
            _ => return None,
        })
    }

    /// Exactly the value `v`.
    fn literal(v: &Value, depth: usize) -> Ty {
        if depth > MAX_DEPTH {
            return Ty::any();
        }
        let none = Ty::never();
        match v {
            Value::Null => Ty { null: true, ..none },
            Value::Bool(b) => Ty { boolean: Lits::Some(vec![*b]), ..none },
            Value::Number(n) => Ty { number: Num::Some(vec![n.clone()]), ..none },
            Value::String(s) => Ty { string: Lits::Some(vec![s.clone()]), ..none },
            Value::Array(items) => {
                let prefix = items.iter().map(|x| Ty::literal(x, depth + 1)).collect();
                Ty { arrays: Arr::shapes(Arr::new(prefix, Ty::never(), items.len() as u64, None)), ..none }
            }
            Value::Object(map) => {
                let props = map.iter().map(|(k, x)| Prop { name: k.clone(), ty: Ty::literal(x, depth + 1), required: true }).collect();
                Ty { objects: Obj::shapes(Obj::new(props, Ty::never())), ..none }
            }
        }
    }

    /// Every value outside `self`, when `self` is whole JSON types (each one all or nothing):
    /// the complement of anything finer is no set a type states.
    fn complement(&self) -> Option<Ty> {
        let whole = matches!(self.boolean, Lits::No | Lits::All)
            && matches!(self.number, Num::No | Num::All { integer: false })
            && matches!(self.string, Lits::No | Lits::All)
            && matches!(&self.arrays, Shapes::All) | matches!(&self.arrays, Shapes::Some(v) if v.is_empty())
            && matches!(&self.objects, Shapes::All) | matches!(&self.objects, Shapes::Some(v) if v.is_empty());
        if !whole {
            return None;
        }
        Some(Ty {
            null: !self.null,
            boolean: if self.boolean == Lits::All { Lits::No } else { Lits::All },
            number: if self.number == Num::No { Num::All { integer: false } } else { Num::No },
            string: if self.string == Lits::All { Lits::No } else { Lits::All },
            arrays: if self.arrays == Shapes::All { Shapes::Some(Vec::new()) } else { Shapes::All },
            objects: if self.objects == Shapes::All { Shapes::Some(Vec::new()) } else { Shapes::All },
        })
    }

    fn union(&self, other: &Ty) -> Ty {
        Ty {
            null: self.null || other.null,
            boolean: lits_union(&self.boolean, &other.boolean),
            number: num_union(&self.number, &other.number),
            string: lits_union(&self.string, &other.string),
            arrays: shapes_union(&self.arrays, &other.arrays, Arr::widen, Arr::holds),
            objects: shapes_union(&self.objects, &other.objects, |_| Shapes::All, Obj::holds),
        }
    }

    fn and(&self, other: &Ty) -> Ty {
        Ty {
            null: self.null && other.null,
            boolean: lits_and(&self.boolean, &other.boolean),
            number: num_and(&self.number, &other.number),
            string: lits_and(&self.string, &other.string),
            arrays: shapes_and(&self.arrays, &other.arrays, Arr::and),
            objects: shapes_and(&self.objects, &other.objects, Obj::and),
        }
    }
}

fn lits_union<T: Clone + PartialEq>(a: &Lits<T>, b: &Lits<T>) -> Lits<T> {
    match (a, b) {
        (Lits::No, x) | (x, Lits::No) => x.clone(),
        (Lits::All, _) | (_, Lits::All) => Lits::All,
        (Lits::Some(x), Lits::Some(y)) => {
            let mut out = x.clone();
            out.extend(y.iter().filter(|v| !x.contains(v)).cloned());
            Lits::Some(out)
        }
    }
}

fn lits_and<T: Clone + PartialEq>(a: &Lits<T>, b: &Lits<T>) -> Lits<T> {
    match (a, b) {
        (Lits::No, _) | (_, Lits::No) => Lits::No,
        (Lits::All, x) | (x, Lits::All) => x.clone(),
        (Lits::Some(x), Lits::Some(y)) => {
            let kept: Vec<T> = x.iter().filter(|v| y.contains(v)).cloned().collect();
            if kept.is_empty() {
                Lits::No
            } else {
                Lits::Some(kept)
            }
        }
    }
}

fn num_union(a: &Num, b: &Num) -> Num {
    match (a, b) {
        (Num::No, x) | (x, Num::No) => x.clone(),
        (Num::All { integer: i }, Num::All { integer: j }) => Num::All { integer: *i && *j },
        (Num::All { integer }, Num::Some(l)) | (Num::Some(l), Num::All { integer }) => Num::All { integer: *integer && l.iter().all(is_integer) },
        (Num::Some(x), Num::Some(y)) => {
            let mut out = x.clone();
            out.extend(y.iter().filter(|v| !x.iter().any(|w| num_eq(w, v))).cloned());
            Num::Some(out)
        }
    }
}

fn num_and(a: &Num, b: &Num) -> Num {
    let kept = |l: Vec<Number>| if l.is_empty() { Num::No } else { Num::Some(l) };
    match (a, b) {
        (Num::No, _) | (_, Num::No) => Num::No,
        (Num::All { integer: i }, Num::All { integer: j }) => Num::All { integer: *i || *j },
        (Num::All { integer }, Num::Some(l)) | (Num::Some(l), Num::All { integer }) => kept(l.iter().filter(|n| !*integer || is_integer(n)).cloned().collect()),
        (Num::Some(x), Num::Some(y)) => kept(x.iter().filter(|v| y.iter().any(|w| num_eq(v, w))).cloned().collect()),
    }
}

/// Two JSON numbers are equal by value, as JSON Schema compares them: `1` is `1.0`.
fn num_eq(a: &Number, b: &Number) -> bool {
    match (a.as_i64(), b.as_i64(), a.as_u64(), b.as_u64()) {
        (Some(x), Some(y), _, _) => x == y,
        (_, _, Some(x), Some(y)) => x == y,
        _ if (a.is_f64() || b.is_f64()) => a.as_f64() == b.as_f64(),
        _ => false,
    }
}

/// An integer by value: `1.0` is one.
fn is_integer(n: &Number) -> bool {
    n.is_i64() || n.is_u64() || n.as_f64().is_some_and(|f| f.is_finite() && f.fract() == 0.0)
}

/// The union of two sets of shapes, leaving out a shape another one holds; past [`MAX_SHAPES`]
/// they widen into one.
fn shapes_union<S: Clone + PartialEq>(a: &Shapes<S>, b: &Shapes<S>, widen: impl Fn(&[S]) -> Shapes<S>, holds: impl Fn(&S, &S) -> bool) -> Shapes<S> {
    let (Shapes::Some(a), Shapes::Some(b)) = (a, b) else { return Shapes::All };
    let mut out: Vec<S> = Vec::new();
    for s in a.iter().chain(b) {
        if out.iter().any(|kept| holds(kept, s)) {
            continue;
        }
        out.retain(|kept| !holds(s, kept));
        out.push(s.clone());
    }
    if out.len() > MAX_SHAPES {
        return widen(&out);
    }
    Shapes::Some(out)
}

/// Whether every value of `b` is one of `a`'s, as far as the structure shows; `false` where it
/// cannot tell.
fn holds(a: &Ty, b: &Ty) -> bool {
    let lits = |x: &Lits<String>, y: &Lits<String>| match (x, y) {
        (_, Lits::No) | (Lits::All, _) => true,
        (Lits::Some(x), Lits::Some(y)) => y.iter().all(|v| x.contains(v)),
        _ => false,
    };
    let bools = |x: &Lits<bool>, y: &Lits<bool>| match (x, y) {
        (_, Lits::No) | (Lits::All, _) => true,
        (Lits::Some(x), Lits::Some(y)) => y.iter().all(|v| x.contains(v)),
        _ => false,
    };
    let numbers = match (&a.number, &b.number) {
        (_, Num::No) => true,
        (Num::All { integer: i }, Num::All { integer: j }) => !*i || *j,
        (Num::All { integer }, Num::Some(l)) => !*integer || l.iter().all(is_integer),
        (Num::Some(x), Num::Some(y)) => y.iter().all(|v| x.iter().any(|w| num_eq(v, w))),
        _ => false,
    };
    let shapes = |x: &Shapes<Arr>, y: &Shapes<Arr>| match (x, y) {
        (Shapes::All, _) => true,
        (Shapes::Some(_), Shapes::All) => false,
        (Shapes::Some(x), Shapes::Some(y)) => y.iter().all(|q| x.iter().any(|p| Arr::holds(p, q))),
    };
    let objects = match (&a.objects, &b.objects) {
        (Shapes::All, _) => true,
        (Shapes::Some(_), Shapes::All) => false,
        (Shapes::Some(x), Shapes::Some(y)) => y.iter().all(|q| x.iter().any(|p| Obj::holds(p, q))),
    };
    (a.null || !b.null) && bools(&a.boolean, &b.boolean) && numbers && lits(&a.string, &b.string) && shapes(&a.arrays, &b.arrays) && objects
}

/// The intersection of two sets of shapes, member by member. One that would take too many pairs
/// is left as the first, a superset of it.
fn shapes_and<S: Clone + PartialEq>(a: &Shapes<S>, b: &Shapes<S>, and: impl Fn(&S, &S) -> Shapes<S>) -> Shapes<S> {
    let (x, y) = match (a, b) {
        (Shapes::All, other) | (other, Shapes::All) => return other.clone(),
        (Shapes::Some(x), Shapes::Some(y)) => (x, y),
    };
    if x.len() * y.len() > MAX_SHAPES * 2 {
        return a.clone();
    }
    let mut out: Vec<S> = Vec::new();
    for p in x {
        for q in y {
            match and(p, q) {
                Shapes::All => return Shapes::All,
                Shapes::Some(zs) => {
                    for z in zs {
                        if !out.contains(&z) {
                            out.push(z);
                        }
                    }
                }
            }
        }
    }
    Shapes::Some(out)
}

impl Arr {
    /// A shape as a set of shapes: none for no shape, all for one that admits every array.
    fn shapes(shape: Option<Arr>) -> Shapes<Arr> {
        match shape {
            Some(a) if a.prefix.is_empty() && a.min == 0 && a.rest.is_any() => Shapes::All,
            Some(a) => Shapes::Some(vec![a]),
            None => Shapes::Some(Vec::new()),
        }
    }

    /// The shape, normalized, or `None` where no array has it: an element it requires can be no
    /// value. `max` is `maxItems`, which ends the array where it falls within the prefix.
    fn new(mut prefix: Vec<Ty>, mut rest: Ty, min: u64, max: Option<u64>) -> Option<Arr> {
        if let Some(max) = max {
            if max < min {
                return None;
            }
            if max <= prefix.len() as u64 {
                prefix.truncate(max as usize);
                rest = Ty::never();
            }
        }
        // An element that can be no value: a required one leaves no array, an optional one ends
        // the array before it.
        if let Some(i) = prefix.iter().position(Ty::is_never) {
            if (i as u64) < min {
                return None;
            }
            prefix.truncate(i);
            rest = Ty::never();
        }
        if rest.is_never() && min > prefix.len() as u64 {
            return None;
        }
        // An optional element no different from the rest is the rest.
        while prefix.len() as u64 > min && prefix.last() == Some(&rest) {
            prefix.pop();
        }
        Some(Arr { prefix, rest: Box::new(rest), min })
    }

    fn at(&self, i: usize) -> Ty {
        self.prefix.get(i).cloned().unwrap_or_else(|| (*self.rest).clone())
    }

    fn and(a: &Arr, b: &Arr) -> Shapes<Arr> {
        let n = a.prefix.len().max(b.prefix.len());
        let prefix = (0..n).map(|i| a.at(i).and(&b.at(i))).collect();
        Arr::shapes(Arr::new(prefix, a.rest.and(&b.rest), a.min.max(b.min), None))
    }

    /// Whether every array of shape `b` has shape `a`.
    fn holds(a: &Arr, b: &Arr) -> bool {
        let n = a.prefix.len().max(b.prefix.len());
        a.min <= b.min && (0..n).all(|i| holds(&a.at(i), &b.at(i))) && holds(&a.rest, &b.rest)
    }

    /// One shape over all of `shapes`: arrays of any of their elements.
    fn widen(shapes: &[Arr]) -> Shapes<Arr> {
        let mut element = Ty::never();
        for s in shapes {
            for t in s.prefix.iter().chain(std::iter::once(&*s.rest)) {
                element = element.union(t);
            }
        }
        Arr::shapes(Some(Arr { prefix: Vec::new(), rest: Box::new(element), min: 0 }))
    }
}

impl Obj {
    /// A shape as a set of shapes: none for no shape, all for one that admits every object.
    fn shapes(shape: Option<Obj>) -> Shapes<Obj> {
        match shape {
            Some(o) if o.rest.is_any() && o.props.iter().all(|p| !p.required && p.ty.is_any()) => Shapes::All,
            Some(o) => Shapes::Some(vec![o]),
            None => Shapes::Some(Vec::new()),
        }
    }

    /// The shape, or `None` where no object has it: a key it requires can hold no value.
    fn new(props: Vec<Prop>, rest: Ty) -> Option<Obj> {
        if props.iter().any(|p| p.required && p.ty.is_never()) {
            return None;
        }
        Some(Obj { props, rest: Box::new(rest) })
    }

    fn get(&self, name: &str) -> Option<&Prop> {
        self.props.iter().find(|p| p.name == name)
    }

    /// Whether every object of shape `b` has shape `a`: each key `a` requires `b` requires, and
    /// every key holds in `a` what it may hold in `b`.
    fn holds(a: &Obj, b: &Obj) -> bool {
        let ty = |o: &Obj, name: &str| o.get(name).map(|p| p.ty.clone()).unwrap_or_else(|| (*o.rest).clone());
        a.props.iter().all(|p| !p.required || b.get(&p.name).is_some_and(|q| q.required))
            && a.props.iter().chain(&b.props).all(|p| holds(&ty(a, &p.name), &ty(b, &p.name)))
            && holds(&a.rest, &b.rest)
    }

    fn and(a: &Obj, b: &Obj) -> Shapes<Obj> {
        let mut props: Vec<Prop> = Vec::new();
        let names = a.props.iter().map(|p| &p.name).chain(b.props.iter().map(|p| &p.name).filter(|n| a.get(n).is_none()));
        for name in names {
            let (x, y) = (a.get(name), b.get(name));
            let tx = x.map(|p| p.ty.clone()).unwrap_or_else(|| (*a.rest).clone());
            let ty = y.map(|p| p.ty.clone()).unwrap_or_else(|| (*b.rest).clone());
            let required = x.is_some_and(|p| p.required) || y.is_some_and(|p| p.required);
            props.push(Prop { name: name.clone(), ty: tx.and(&ty), required });
        }
        Obj::shapes(Obj::new(props, a.rest.and(&b.rest)))
    }
}

// ---------------------------------------------------------------------------------------------
// Reading a schema under one draft
// ---------------------------------------------------------------------------------------------

/// The base URI a schema's relative references resolve against when it names none.
const DEFAULT_BASE: &str = "reylo:///schema.json";

/// Keywords that annotate and constrain nothing.
const ANNOTATIONS: &[&str] = &["title", "description", "$comment", "examples", "default", "deprecated", "readOnly", "writeOnly"];

/// Where a subschema's references resolve: its base URI, `None` where an identifier could not be
/// read, and every reference there is any value.
#[derive(Clone)]
struct Base {
    uri: Option<String>,
}

struct Reader<'a> {
    root: &'a Value,
    draft: Draft,
    validator: &'a Validator,
    /// Every schema resource by its absolute URI, and every anchor by its resource and name.
    resources: HashMap<String, &'a Value>,
    anchors: HashMap<(String, String), &'a Value>,
    /// The reference targets being read: one met again is a recursive type, cut to any value there.
    following: Vec<*const Value>,
    reads: usize,
}

impl<'a> Reader<'a> {
    fn new(root: &'a Value, draft: Draft, validator: &'a Validator) -> Reader<'a> {
        let mut reader = Reader { root, draft, validator, resources: HashMap::new(), anchors: HashMap::new(), following: Vec::new(), reads: 0 };
        let base = reader.root_base();
        if let Some(uri) = &base.uri {
            reader.resources.insert(uri.clone(), root);
        }
        reader.index(root, base.uri.clone(), 0);
        reader
    }

    fn root_base(&self) -> Base {
        let at_default = Base { uri: Some(DEFAULT_BASE.to_string()) };
        match self.root.as_object() {
            Some(map) => self.enter(map, &at_default),
            None => at_default,
        }
    }

    fn id_key(&self) -> &'static str {
        if self.draft == Draft::Four {
            "id"
        } else {
            "$id"
        }
    }

    /// The base inside `node`: a new resource where its identifier names one, else `base`.
    fn enter(&self, map: &'a Map<String, Value>, base: &Base) -> Base {
        match map.get(self.id_key()).and_then(Value::as_str) {
            Some(id) if !id.starts_with('#') => match base.uri.as_deref().and_then(|b| resolve(b, id)) {
                Some(uri) => Base { uri: Some(without_fragment(&uri).to_string()) },
                None => Base { uri: None },
            },
            _ => base.clone(),
        }
    }

    /// Record every resource and anchor below `node`, whose base URI is `base`.
    fn index(&mut self, node: &'a Value, base: Option<String>, depth: usize) {
        let Some(map) = node.as_object() else { return };
        if depth > MAX_DEPTH {
            return;
        }
        let mut base = base;
        if let Some(id) = map.get(self.id_key()).and_then(Value::as_str) {
            let resolved = base.as_deref().and_then(|b| resolve(b, id));
            match (id.strip_prefix('#'), resolved) {
                // Through draft 7 an identifier that is a fragment alone names an anchor.
                (Some(name), _) if self.draft <= Draft::Seven => {
                    if let Some(b) = &base {
                        self.anchors.insert((b.clone(), name.to_string()), node);
                    }
                }
                (Some(_), _) => {}
                (None, Some(uri)) => {
                    let doc = without_fragment(&uri).to_string();
                    self.resources.entry(doc.clone()).or_insert(node);
                    if let Some(name) = fragment(&uri).filter(|f| !f.is_empty() && !f.starts_with('/')) {
                        self.anchors.insert((doc.clone(), name.to_string()), node);
                    }
                    base = Some(doc);
                }
                (None, None) => base = None,
            }
        }
        if self.draft >= Draft::Nineteen {
            for key in ["$anchor", "$dynamicAnchor"] {
                if let (Some(name), Some(b)) = (map.get(key).and_then(Value::as_str), &base) {
                    self.anchors.insert((b.clone(), name.to_string()), node);
                }
            }
        }
        for child in subschemas(map) {
            self.index(child, base.clone(), depth + 1);
        }
    }

    fn read_root(&mut self) -> Ty {
        let base = Base { uri: Some(DEFAULT_BASE.to_string()) };
        self.read(self.root, &base, 0)
    }

    fn read(&mut self, node: &'a Value, base: &Base, depth: usize) -> Ty {
        self.reads += 1;
        if depth > MAX_DEPTH || self.reads > MAX_READS {
            return Ty::any();
        }
        let map = match node {
            Value::Bool(true) => return Ty::any(),
            // A boolean schema is a draft 6 word; draft 4 reads none.
            Value::Bool(false) if self.draft >= Draft::Six => return Ty::never(),
            Value::Object(map) => map,
            _ => return Ty::any(),
        };
        let here = self.enter(map, base);
        // A `$ref` that is no string resolves nowhere, and its node admits anything.
        if map.get("$ref").is_some_and(|r| !r.is_string()) {
            return Ty::any();
        }
        let reference = map.get("$ref").and_then(Value::as_str);
        if let Some(r) = reference {
            if self.draft <= Draft::Seven {
                // The keywords beside the reference, its identifier included, are ignored: one
                // that names a base would resolve the reference otherwise in some validators.
                if map.contains_key(self.id_key()) {
                    return Ty::any();
                }
                return self.follow(r, base, depth);
            }
            if !self.validator.ref_siblings {
                return self.follow(r, &here, depth);
            }
        }
        let mut acc = match reference {
            Some(r) => self.follow(r, &here, depth),
            None => Ty::any(),
        };
        let and = |acc: &mut Ty, t: Ty| *acc = acc.and(&t);
        if let Some(t) = map.get("type") {
            and(&mut acc, types(t));
        }
        if let Some(Value::Array(values)) = map.get("enum").filter(|v| v.as_array().is_some_and(|a| !a.is_empty())) {
            and(&mut acc, values.iter().fold(Ty::never(), |t, v| t.union(&Ty::literal(v, depth))));
        }
        if self.draft >= Draft::Six {
            if let Some(v) = map.get("const") {
                and(&mut acc, Ty::literal(v, depth));
            }
        }
        if ["properties", "required", "additionalProperties", "patternProperties"].iter().any(|k| map.contains_key(*k)) {
            let t = self.object(map, &here, depth);
            and(&mut acc, t);
        }
        if ["items", "prefixItems", "additionalItems", "minItems", "maxItems"].iter().any(|k| map.contains_key(*k)) {
            let t = self.array(map, &here, depth);
            and(&mut acc, t);
        }
        if let Some(Value::Array(all)) = map.get("allOf") {
            for s in all {
                if acc.is_never() {
                    return acc;
                }
                let t = self.read(s, &here, depth + 1);
                and(&mut acc, t);
            }
        }
        for key in ["anyOf", "oneOf"] {
            if let Some(Value::Array(members)) = map.get(key).filter(|v| v.as_array().is_some_and(|a| !a.is_empty())) {
                let mut t = Ty::never();
                for s in members {
                    t = t.union(&self.read(s, &here, depth + 1));
                }
                and(&mut acc, t);
            }
        }
        // `not` of whole JSON types removes them; of anything finer it removes values a type
        // cannot single out, and says nothing.
        if let Some(complement) = map.get("not").and_then(|n| self.exact_kinds(n)).and_then(|k| k.complement()) {
            and(&mut acc, complement);
        }
        if self.draft >= Draft::Seven {
            if let Some(condition) = map.get("if") {
                let then = map.get("then").map(|s| self.read(s, &here, depth + 1));
                let otherwise = map.get("else").map(|s| self.read(s, &here, depth + 1));
                let exact = self.exact_kinds(condition);
                let t = match (exact.and_then(|k| Some((k.complement()?, k))), then, otherwise) {
                    // A condition that is whole JSON types splits the values exactly.
                    (Some((outside, inside)), then, otherwise) => inside.and(&then.unwrap_or_else(Ty::any)).union(&outside.and(&otherwise.unwrap_or_else(Ty::any))),
                    // Any other condition: the value meets one branch or the other.
                    (None, Some(then), Some(otherwise)) => then.union(&otherwise),
                    _ => Ty::any(),
                };
                and(&mut acc, t);
            }
        }
        acc
    }

    /// What `additionalProperties` or `additionalItems` admits: a boolean there is the keyword's
    /// own value in every draft, draft 4's included, where no other place takes one.
    fn keyword_schema(&mut self, schema: Option<&'a Value>, base: &Base, depth: usize) -> Ty {
        match schema {
            None | Some(Value::Bool(true)) => Ty::any(),
            Some(Value::Bool(false)) => Ty::never(),
            Some(s) => self.read(s, base, depth + 1),
        }
    }

    /// What a subschema admits when it states that exactly: `true`, `false`, or whole JSON
    /// types beside annotations alone. `None` for anything that also checks.
    fn exact_kinds(&self, node: &Value) -> Option<Ty> {
        match node {
            Value::Bool(true) => Some(Ty::any()),
            Value::Bool(false) if self.draft >= Draft::Six => Some(Ty::never()),
            Value::Object(map) => {
                if map.keys().any(|k| k != "type" && !ANNOTATIONS.contains(&k.as_str())) {
                    return None;
                }
                match map.get("type") {
                    None => Some(Ty::any()),
                    Some(Value::String(t)) if t != "integer" => Ty::kind(t),
                    Some(Value::Array(ts)) if !ts.is_empty() => ts.iter().try_fold(Ty::never(), |acc, t| match t.as_str() {
                        Some(t) if t != "integer" => Some(acc.union(&Ty::kind(t)?)),
                        _ => None,
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The objects `properties`, `required`, `additionalProperties` and `patternProperties` admit,
    /// beside every value of another type. A key no property names holds what
    /// `additionalProperties` or one of the patterns admits, whichever applies to it.
    fn object(&mut self, map: &'a Map<String, Value>, base: &Base, depth: usize) -> Ty {
        let mut required: Vec<&str> = map.get("required").and_then(Value::as_array).map(|r| r.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        required.dedup();
        let mut rest = self.keyword_schema(map.get("additionalProperties"), base, depth);
        if let Some(Value::Object(patterns)) = map.get("patternProperties").filter(|p| p.as_object().is_some_and(|p| !p.is_empty())) {
            // A validator that does not check the patterns still exempts their keys.
            if !self.validator.patterns {
                rest = Ty::any();
            }
            for s in patterns.values() {
                if rest.is_any() {
                    break;
                }
                rest = rest.union(&self.read(s, base, depth + 1));
            }
        }
        let mut props: Vec<Prop> = Vec::new();
        if let Some(Value::Object(properties)) = map.get("properties") {
            for (name, s) in properties {
                let ty = self.read(s, base, depth + 1);
                props.push(Prop { name: name.clone(), ty, required: required.contains(&name.as_str()) });
            }
        }
        for name in required {
            if !props.iter().any(|p| p.name == name) {
                props.push(Prop { name: name.to_string(), ty: rest.clone(), required: true });
            }
        }
        Ty { objects: Obj::shapes(Obj::new(props, rest)), ..Ty::any() }
    }

    /// The arrays the array keywords admit, beside every value of another type: draft 2020-12's
    /// `prefixItems` and `items`, or earlier drafts' `items` (a list is the prefix) and
    /// `additionalItems`, with `minItems` and `maxItems`.
    fn array(&mut self, map: &'a Map<String, Value>, base: &Base, depth: usize) -> Ty {
        let (prefix, rest) = if !self.validator.tuples {
            match map.get("items") {
                Some(s) if s.is_object() => (Vec::new(), self.read(s, base, depth + 1)),
                _ => (Vec::new(), Ty::any()),
            }
        } else if self.draft <= Draft::Nineteen {
            match map.get("items") {
                Some(Value::Array(items)) => {
                    let prefix = items.iter().map(|s| self.read(s, base, depth + 1)).collect();
                    let rest = self.keyword_schema(map.get("additionalItems"), base, depth);
                    (prefix, rest)
                }
                Some(s) => (Vec::new(), self.read(s, base, depth + 1)),
                None => (Vec::new(), Ty::any()),
            }
        } else {
            let prefix = map.get("prefixItems").and_then(Value::as_array).map(|items| items.iter().map(|s| self.read(s, base, depth + 1)).collect()).unwrap_or_default();
            let rest = match map.get("items") {
                Some(s) if !s.is_array() => self.read(s, base, depth + 1),
                _ => Ty::any(),
            };
            (prefix, rest)
        };
        let min = map.get("minItems").and_then(Value::as_u64).unwrap_or(0);
        let max = map.get("maxItems").and_then(Value::as_u64);
        Ty { arrays: Arr::shapes(Arr::new(prefix, rest, min, max)), ..Ty::any() }
    }

    /// What the reference `r` admits, read from `base`. A reference that does not resolve to a
    /// subschema of this document is any value: the validator may fetch it, or refuse it.
    fn follow(&mut self, r: &str, base: &Base, depth: usize) -> Ty {
        let Some(target) = self.target(r, base) else { return Ty::any() };
        let (node, resource_uri) = target;
        let key = node as *const Value;
        if self.following.contains(&key) {
            return Ty::any();
        }
        self.following.push(key);
        let t = self.read(node, &Base { uri: Some(resource_uri) }, depth + 1);
        self.following.pop();
        t
    }

    fn target(&self, r: &str, base: &Base) -> Option<(&'a Value, String)> {
        if self.validator.pointer_refs_only && !(r == "#" || r.starts_with("#/")) {
            return None;
        }
        let uri = resolve(base.uri.as_deref()?, r)?;
        let doc = without_fragment(&uri).to_string();
        let resource = *self.resources.get(&doc)?;
        let node = match fragment(&uri).unwrap_or("") {
            "" => resource,
            f if f.starts_with('/') && f.contains('$') && !self.validator.dollar_pointers => return None,
            f if f.starts_with('/') => pointer(resource, &percent_decode(f)?, self.id_key())?,
            name => *self.anchors.get(&(doc.clone(), percent_decode(name)?))?,
        };
        Some((node, doc))
    }
}

/// The subschemas of a schema object, wherever any draft puts them.
fn subschemas(map: &Map<String, Value>) -> Vec<&Value> {
    let mut out = Vec::new();
    for key in ["properties", "patternProperties", "definitions", "$defs", "dependentSchemas", "dependencies"] {
        if let Some(Value::Object(m)) = map.get(key) {
            out.extend(m.values().filter(|v| v.is_object() || v.is_boolean()));
        }
    }
    for key in ["additionalProperties", "additionalItems", "items", "contains", "propertyNames", "not", "if", "then", "else", "unevaluatedItems", "unevaluatedProperties", "contentSchema"] {
        match map.get(key) {
            Some(v @ (Value::Object(_) | Value::Bool(_))) => out.push(v),
            Some(Value::Array(items)) if key == "items" => out.extend(items.iter()),
            _ => {}
        }
    }
    for key in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(Value::Array(items)) = map.get(key) {
            out.extend(items.iter());
        }
    }
    out
}

fn types(t: &Value) -> Ty {
    match t {
        Value::String(name) => Ty::kind(name).unwrap_or_else(Ty::any),
        Value::Array(names) if !names.is_empty() => names.iter().try_fold(Ty::never(), |acc, n| Some(acc.union(&Ty::kind(n.as_str()?)?))).unwrap_or_else(Ty::any),
        _ => Ty::any(),
    }
}

/// The node a JSON Pointer names inside `resource`. One that passes through another resource,
/// a subschema with an identifier of its own, is not followed: its base would change on the way.
fn pointer<'a>(resource: &'a Value, path: &str, id_key: &str) -> Option<&'a Value> {
    let mut node = resource;
    let segments: Vec<String> = path.split('/').skip(1).map(|s| s.replace("~1", "/").replace("~0", "~")).collect();
    for (i, segment) in segments.iter().enumerate() {
        if i > 0 && node.get(id_key).and_then(Value::as_str).is_some_and(|id| !id.starts_with('#')) {
            return None;
        }
        node = match node {
            Value::Object(map) => map.get(segment)?,
            Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(node)
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn without_fragment(uri: &str) -> &str {
    uri.split_once('#').map_or(uri, |(u, _)| u)
}

fn fragment(uri: &str) -> Option<&str> {
    uri.split_once('#').map(|(_, f)| f)
}

/// A URI reference resolved against an absolute base, by RFC 3986 §5.2. `None` where the base is
/// not absolute.
fn resolve(base: &str, reference: &str) -> Option<String> {
    let r = Parts::of(reference);
    let b = Parts::of(base);
    b.scheme?;
    let (scheme, authority, path, query);
    if r.scheme.is_some() {
        (scheme, authority, path, query) = (r.scheme, r.authority, remove_dots(r.path), r.query);
    } else if r.authority.is_some() {
        (scheme, authority, path, query) = (b.scheme, r.authority, remove_dots(r.path), r.query);
    } else if r.path.is_empty() {
        (scheme, authority, path, query) = (b.scheme, b.authority, b.path.to_string(), r.query.or(b.query));
    } else if r.path.starts_with('/') {
        (scheme, authority, path, query) = (b.scheme, b.authority, remove_dots(r.path), r.query);
    } else {
        let merged = if b.authority.is_some() && b.path.is_empty() {
            format!("/{}", r.path)
        } else {
            match b.path.rfind('/') {
                Some(i) => format!("{}{}", &b.path[..=i], r.path),
                None => r.path.to_string(),
            }
        };
        (scheme, authority, path, query) = (b.scheme, b.authority, remove_dots(&merged), r.query);
    }
    let mut out = format!("{}:", scheme?);
    if let Some(a) = authority {
        out.push_str("//");
        out.push_str(a);
    }
    out.push_str(&path);
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = r.fragment {
        out.push('#');
        out.push_str(f);
    }
    Some(out)
}

struct Parts<'a> {
    scheme: Option<&'a str>,
    authority: Option<&'a str>,
    path: &'a str,
    query: Option<&'a str>,
    fragment: Option<&'a str>,
}

impl<'a> Parts<'a> {
    fn of(uri: &'a str) -> Parts<'a> {
        let (rest, fragment) = match uri.split_once('#') {
            Some((r, f)) => (r, Some(f)),
            None => (uri, None),
        };
        let (rest, query) = match rest.split_once('?') {
            Some((r, q)) => (r, Some(q)),
            None => (rest, None),
        };
        let scheme_end = rest.find(':').filter(|&i| {
            let s = &rest[..i];
            !s.is_empty() && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && s.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        });
        let (scheme, rest) = match scheme_end {
            Some(i) => (Some(&rest[..i]), &rest[i + 1..]),
            None => (None, rest),
        };
        let (authority, path) = match rest.strip_prefix("//") {
            Some(r) => match r.find('/') {
                Some(i) => (Some(&r[..i]), &r[i..]),
                None => (Some(r), ""),
            },
            None => (None, rest),
        };
        Parts { scheme, authority, path, query, fragment }
    }
}

fn remove_dots(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let absolute = path.starts_with('/');
    let segments: Vec<&str> = path.split('/').collect();
    let last = segments.len().saturating_sub(1);
    for (i, segment) in segments.iter().enumerate() {
        match *segment {
            "." => {
                if i == last {
                    out.push("");
                }
            }
            ".." => {
                if out.len() > usize::from(absolute) {
                    out.pop();
                }
                if i == last {
                    out.push("");
                }
            }
            s => out.push(s),
        }
    }
    let joined = out.join("/");
    if absolute && !joined.starts_with('/') {
        format!("/{joined}")
    } else {
        joined
    }
}

// ---------------------------------------------------------------------------------------------
// The canonical schema
// ---------------------------------------------------------------------------------------------

fn emit(ty: &Ty) -> Value {
    if ty.is_any() {
        return json!({});
    }
    if ty.is_never() {
        return json!(false);
    }
    let mut members: Vec<Value> = Vec::new();
    let mut types: Vec<&str> = Vec::new();
    let mut literals: Vec<Value> = Vec::new();
    match &ty.objects {
        Shapes::All => types.push("object"),
        Shapes::Some(shapes) => members.extend(shapes.iter().map(emit_object)),
    }
    match &ty.arrays {
        Shapes::All => types.push("array"),
        Shapes::Some(shapes) => members.extend(shapes.iter().map(emit_array)),
    }
    match &ty.string {
        Lits::All => types.push("string"),
        Lits::Some(values) => literals.extend(values.iter().map(|s| json!(s))),
        Lits::No => {}
    }
    match &ty.number {
        Num::All { integer } => types.push(if *integer { "integer" } else { "number" }),
        Num::Some(values) => literals.extend(values.iter().map(|n| Value::Number(n.clone()))),
        Num::No => {}
    }
    match &ty.boolean {
        Lits::All => types.push("boolean"),
        Lits::Some(values) => literals.extend(values.iter().map(|b| json!(b))),
        Lits::No => {}
    }
    // A null joins the literals where there are some: `"a" | null` is one list.
    if ty.null && literals.is_empty() {
        types.push("null");
    } else if ty.null {
        literals.push(Value::Null);
    }
    if !literals.is_empty() {
        members.push(json!({ "enum": literals }));
    }
    match types.as_slice() {
        [] => {}
        [one] => members.push(json!({ "type": one })),
        many => members.push(json!({ "type": many })),
    }
    if members.len() == 1 {
        members.pop().expect("one member")
    } else {
        json!({ "anyOf": members })
    }
}

fn emit_array(a: &Arr) -> Value {
    let mut out = Map::new();
    out.insert("type".into(), json!("array"));
    if !a.prefix.is_empty() {
        out.insert("prefixItems".into(), Value::Array(a.prefix.iter().map(emit).collect()));
    }
    if !a.rest.is_any() {
        out.insert("items".into(), emit(&a.rest));
    }
    if a.min > 0 {
        out.insert("minItems".into(), json!(a.min));
    }
    Value::Object(out)
}

fn emit_object(o: &Obj) -> Value {
    let mut out = Map::new();
    out.insert("type".into(), json!("object"));
    if !o.props.is_empty() {
        out.insert("properties".into(), Value::Object(o.props.iter().map(|p| (p.name.clone(), emit(&p.ty))).collect()));
    }
    let required: Vec<Value> = o.props.iter().filter(|p| p.required).map(|p| json!(p.name)).collect();
    if !required.is_empty() {
        out.insert("required".into(), Value::Array(required));
    }
    out.insert("additionalProperties".into(), if o.rest.is_any() { json!(true) } else { emit(&o.rest) });
    Value::Object(out)
}

/// What a value a canonical schema ([`type_schema`]) admits holds at `path`, read key by key as
/// Postgres's `->` and `#>` read an object: the canonical schema of the values found there, and
/// whether one is found for certain, every step landing on an object that holds its key. A step
/// on anything else finds no value, which the database answers with SQL NULL, so it narrows
/// nothing but that certainty. With `indexes`, a key that is a whole number reads an array's
/// element instead (`#>`), which this reading does not follow: `None` where a step may land on an
/// array then, and where the schema holds anything but the canonical vocabulary.
pub fn at_path(schema: &Value, path: &[&str], indexes: bool) -> Option<(Value, bool)> {
    let mut at = schema.clone();
    let mut found = true;
    for key in path {
        let (next, certain) = at_key(&at, key, indexes)?;
        at = next;
        found &= certain;
    }
    Some((at, found))
}

/// One step of [`at_path`].
fn at_key(schema: &Value, key: &str, indexes: bool) -> Option<(Value, bool)> {
    let index = indexes && !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit());
    let map = match schema {
        Value::Bool(true) => return (!index).then_some((Value::Bool(true), false)),
        Value::Bool(false) => return Some((Value::Bool(false), false)),
        Value::Object(map) if map.is_empty() => return (!index).then_some((Value::Bool(true), false)),
        Value::Object(map) => map,
        _ => return None,
    };
    if let Some(Value::Array(members)) = map.get("anyOf") {
        let mut found = !members.is_empty();
        let mut values = Vec::new();
        for member in members {
            let (value, certain) = at_key(member, key, indexes)?;
            found &= certain;
            values.push(value);
        }
        return Some((any_of(values), found));
    }
    if map.contains_key("not") {
        return None;
    }
    if let Some(Value::Array(values)) = map.get("enum") {
        let mut found = !values.is_empty();
        let mut held: Vec<Value> = Vec::new();
        for value in values {
            match value {
                Value::Object(fields) => match fields.get(key) {
                    Some(v) if !held.iter().any(|h| json_eq(h, v)) => held.push(v.clone()),
                    Some(_) => {}
                    None => found = false,
                },
                Value::Array(_) if index => return None,
                _ => found = false,
            }
        }
        return Some((if held.is_empty() { Value::Bool(false) } else { json!({ "enum": held }) }, found));
    }
    let kinds: Vec<&str> = match map.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).collect(),
        _ => return (!index).then_some((Value::Bool(true), false)),
    };
    if index && kinds.contains(&"array") {
        return None;
    }
    if !kinds.contains(&"object") {
        return Some((Value::Bool(false), false));
    }
    let declared = map.get("properties").and_then(Value::as_object).and_then(|p| p.get(key));
    let value = declared.or_else(|| map.get("additionalProperties")).cloned().unwrap_or(Value::Bool(true));
    let required = map.get("required").and_then(Value::as_array).is_some_and(|r| r.iter().any(|k| k.as_str() == Some(key)));
    Some((value, kinds.len() == 1 && required))
}

/// The canonical union of `members`: `true` when one admits anything, none of the `false` ones.
fn any_of(members: Vec<Value>) -> Value {
    let mut kept: Vec<Value> = Vec::new();
    for member in members {
        match member {
            Value::Bool(true) => return Value::Bool(true),
            Value::Object(ref m) if m.is_empty() => return Value::Bool(true),
            Value::Bool(false) => {}
            member if kept.contains(&member) => {}
            member => kept.push(member),
        }
    }
    match kept.len() {
        0 => Value::Bool(false),
        1 => kept.pop().unwrap_or(Value::Bool(false)),
        _ => json!({ "anyOf": kept }),
    }
}

/// Whether `value` is one of the values a canonical schema ([`type_schema`]) admits.
pub fn admits(schema: &Value, value: &Value) -> bool {
    let map = match schema {
        Value::Bool(b) => return *b,
        Value::Object(map) => map,
        _ => return true,
    };
    if let Some(Value::Array(members)) = map.get("anyOf") {
        return members.iter().any(|m| admits(m, value));
    }
    if let Some(Value::Array(values)) = map.get("enum") {
        return values.iter().any(|v| json_eq(v, value));
    }
    if let Some(n) = map.get("not") {
        return !admits(n, value);
    }
    let kinds: Vec<&str> = match map.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).collect(),
        _ => return true,
    };
    kinds.iter().any(|kind| match (*kind, value) {
        ("null", Value::Null) | ("boolean", Value::Bool(_)) | ("string", Value::String(_)) | ("number", Value::Number(_)) => true,
        ("integer", Value::Number(n)) => is_integer(n),
        ("array", Value::Array(items)) => {
            let prefix = map.get("prefixItems").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
            let rest = map.get("items").unwrap_or(&Value::Bool(true));
            let min = map.get("minItems").and_then(Value::as_u64).unwrap_or(0);
            items.len() as u64 >= min && items.iter().enumerate().all(|(i, item)| admits(prefix.get(i).unwrap_or(rest), item))
        }
        ("object", Value::Object(fields)) => {
            let properties = map.get("properties").and_then(Value::as_object);
            let required = map.get("required").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
            let rest = map.get("additionalProperties").unwrap_or(&Value::Bool(true));
            required.iter().filter_map(Value::as_str).all(|k| fields.contains_key(k))
                && fields.iter().all(|(k, v)| admits(properties.and_then(|p| p.get(k)).unwrap_or(rest), v))
        }
        _ => false,
    })
}

/// JSON equality as JSON Schema's `enum` and `const` read it: numbers by value, objects by their
/// keys whatever their order.
fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => num_eq(x, y),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| json_eq(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w))),
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A validator that applies the draft a schema names, and 2020-12 otherwise.
    const SPEC: Validator = Validator { fallback: &[Draft::Twenty], ref_siblings: true, ..BOON };
    const DRAFT4: Validator = Validator { fallback: &[Draft::Four], ..super::SPEC };
    const DRAFT7: Validator = Validator { fallback: &[Draft::Seven], ..super::SPEC };

    fn ty(schema: Value) -> Value {
        type_schema(&schema, &SPEC)
    }

    #[test]
    fn a_plain_object() {
        let schema = json!({"type": "object", "properties": {"name": {"type": "string"}, "age": {"type": ["integer", "null"]}}, "required": ["name"]});
        assert_eq!(ty(schema), json!({"type": "object", "properties": {"name": {"type": "string"}, "age": {"type": ["integer", "null"]}}, "required": ["name"], "additionalProperties": true}));
    }

    #[test]
    fn a_closed_object_and_a_map() {
        assert_eq!(
            ty(json!({"type": "object", "properties": {"a": {"const": 1}}, "additionalProperties": false})),
            json!({"type": "object", "properties": {"a": {"enum": [1]}}, "additionalProperties": false})
        );
        assert_eq!(ty(json!({"type": "object", "additionalProperties": {"type": "string"}})), json!({"type": "object", "additionalProperties": {"type": "string"}}));
        // A pattern's keys are not additional ones: they may hold what the pattern admits.
        assert_eq!(
            ty(json!({"type": "object", "patternProperties": {"^x": {"type": "string"}}, "additionalProperties": false})),
            json!({"type": "object", "additionalProperties": {"type": "string"}})
        );
    }

    #[test]
    fn keywords_apply_to_their_own_type_only() {
        // `required` with no `type` says nothing of a string.
        assert_eq!(
            ty(json!({"required": ["a"]})),
            json!({"anyOf": [{"type": "object", "properties": {"a": {}}, "required": ["a"], "additionalProperties": true}, {"type": ["array", "string", "number", "boolean", "null"]}]})
        );
        assert_eq!(ty(json!({"minLength": 2, "pattern": "^a", "format": "email"})), json!({}));
    }

    #[test]
    fn enums_and_types_intersect() {
        assert_eq!(ty(json!({"type": "string", "enum": ["a", 1, "b", null]})), json!({"enum": ["a", "b"]}));
        assert_eq!(ty(json!({"enum": ["a", 1, null, true]})), json!({"enum": ["a", 1, true, null]}));
        assert_eq!(ty(json!({"type": "integer", "enum": [1.0, 1.5]})), json!({"enum": [1.0]}));
        assert_eq!(ty(json!({"enum": [{"a": 1}, [1, "x"]]})), json!({"anyOf": [
            {"type": "object", "properties": {"a": {"enum": [1]}}, "required": ["a"], "additionalProperties": false},
            {"type": "array", "prefixItems": [{"enum": [1]}, {"enum": ["x"]}], "items": false, "minItems": 2}
        ]}));
    }

    #[test]
    fn unions_and_intersections() {
        let tagged = json!({"oneOf": [
            {"type": "object", "properties": {"kind": {"const": "a"}, "x": {"type": "number"}}, "required": ["kind", "x"]},
            {"type": "object", "properties": {"kind": {"const": "b"}}, "required": ["kind"]}
        ]});
        assert_eq!(
            ty(tagged),
            json!({"anyOf": [
                {"type": "object", "properties": {"kind": {"enum": ["a"]}, "x": {"type": "number"}}, "required": ["kind", "x"], "additionalProperties": true},
                {"type": "object", "properties": {"kind": {"enum": ["b"]}}, "required": ["kind"], "additionalProperties": true}
            ]})
        );
        // allOf merges objects; a key one member closes off and another requires leaves nothing.
        assert_eq!(
            ty(json!({"allOf": [{"type": "object", "properties": {"a": {"type": "string"}}}, {"required": ["a"], "properties": {"b": {"type": "integer"}}}]})),
            json!({"type": "object", "properties": {"a": {"type": "string"}, "b": {"type": "integer"}}, "required": ["a"], "additionalProperties": true})
        );
        assert_eq!(ty(json!({"allOf": [{"type": "object", "additionalProperties": false}, {"type": "object", "required": ["a"]}]})), json!(false));
        assert_eq!(ty(json!({"allOf": [{"type": "string"}, {"type": "number"}]})), json!(false));
    }

    #[test]
    fn a_union_leaves_out_what_another_member_holds() {
        // Draft 4 reads no `const`, so its reading holds the others'.
        let boon = type_schema_any(&json!({"type": "object", "properties": {"k": {"const": "x"}}, "required": ["k"]}), &[&BOON]);
        assert_eq!(boon, json!({"type": "object", "properties": {"k": {}}, "required": ["k"], "additionalProperties": true}));
        // The empty array is a tuple of two optional elements too.
        let pairs = json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "array", "prefixItems": [{"type": "string"}, {"type": "integer"}], "items": false});
        assert_eq!(type_schema_any(&pairs, PG_JSONSCHEMA), json!({"type": "array", "prefixItems": [{"type": "string"}, {"type": "integer"}], "items": false}));
        assert_eq!(ty(json!({"anyOf": [{"enum": ["a"]}, {"type": "string"}, {"enum": [1]}, {"type": "integer"}]})), json!({"type": ["string", "integer"]}));
    }

    #[test]
    fn not_removes_whole_types_only() {
        assert_eq!(ty(json!({"not": {"type": "null"}})), json!({"type": ["object", "array", "string", "number", "boolean"]}));
        // `not` of a string of length 3 or more leaves short strings: nothing is removed.
        assert_eq!(ty(json!({"type": "string", "not": {"type": "string", "minLength": 3}})), json!({"type": "string"}));
        assert_eq!(ty(json!({"not": {}})), json!(false));
    }

    #[test]
    fn conditionals() {
        // A condition on whole types splits exactly.
        assert_eq!(ty(json!({"if": {"type": "string"}, "then": {"enum": ["a"]}, "else": {"type": "null"}})), json!({"enum": ["a", null]}));
        // Any other condition: one branch or the other.
        assert_eq!(ty(json!({"if": {"minLength": 2}, "then": {"type": "string"}, "else": {"type": "integer"}})), json!({"type": ["string", "integer"]}));
        assert_eq!(ty(json!({"if": {"minLength": 2}, "then": {"type": "string"}})), json!({}));
        // Draft 4 has no conditionals.
        assert_eq!(type_schema(&json!({"if": {"type": "string"}, "then": false, "else": false}), &DRAFT4), json!({}));
    }

    #[test]
    fn arrays_and_tuples_by_draft() {
        let tuple = json!({"items": [{"type": "string"}, {"type": "integer"}], "additionalItems": false, "minItems": 1});
        assert_eq!(
            type_schema(&tuple, &DRAFT7),
            json!({"anyOf": [{"type": "array", "prefixItems": [{"type": "string"}, {"type": "integer"}], "items": false, "minItems": 1}, {"type": ["object", "string", "number", "boolean", "null"]}]})
        );
        // In 2020-12 a list under `items` is no keyword of the draft, and `prefixItems` is.
        assert_eq!(ty(json!({"type": "array", "items": [{"type": "string"}]})), json!({"type": "array"}));
        assert_eq!(
            ty(json!({"type": "array", "prefixItems": [{"type": "string"}], "items": {"type": "number"}})),
            json!({"type": "array", "prefixItems": [{"type": "string"}], "items": {"type": "number"}})
        );
        assert_eq!(ty(json!({"type": "array", "items": {"type": "string"}, "maxItems": 0})), json!({"type": "array", "items": false}));
        assert_eq!(ty(json!({"type": "array", "items": false, "minItems": 1})), json!(false));
    }

    #[test]
    fn references() {
        let schema = json!({"$defs": {"s": {"type": "string"}}, "type": "object", "properties": {"a": {"$ref": "#/$defs/s"}}});
        assert_eq!(ty(schema), json!({"type": "object", "properties": {"a": {"type": "string"}}, "additionalProperties": true}));
        // Recursion is cut to any value where it comes back.
        let tree = json!({"type": "object", "properties": {"children": {"type": "array", "items": {"$ref": "#"}}}});
        assert_eq!(
            ty(tree),
            json!({"type": "object", "properties": {"children": {"type": "array", "items": {"type": "object", "properties": {"children": {"type": "array"}}, "additionalProperties": true}}}, "additionalProperties": true})
        );
        // Through draft 7 the keywords beside a reference are ignored; later they apply.
        let beside = json!({"definitions": {"i": {"type": ["integer", "string"]}}, "$ref": "#/definitions/i", "type": "string"});
        assert_eq!(type_schema(&beside, &DRAFT7), json!({"type": ["string", "integer"]}));
        assert_eq!(ty(beside), json!({"type": "string"}));
        // An anchor, an embedded identifier, and what does not resolve.
        assert_eq!(ty(json!({"$defs": {"s": {"$anchor": "S", "type": "string"}}, "$ref": "#S"})), json!({"type": "string"}));
        assert_eq!(ty(json!({"$defs": {"s": {"$id": "http://x.test/s", "type": "string"}}, "$ref": "http://x.test/s"})), json!({"type": "string"}));
        assert_eq!(ty(json!({"$id": "http://x.test/root", "$defs": {"s": {"$id": "s", "type": "string"}}, "$ref": "s"})), json!({"type": "string"}));
        assert_eq!(ty(json!({"$ref": "http://127.0.0.1:9/nothing.json"})), json!({}));
        assert_eq!(ty(json!({"$ref": "#/definitions/nope"})), json!({}));
        // A pointer inside another resource resolves against that resource.
        let nested = json!({"$defs": {"r": {"$id": "http://x.test/r", "$defs": {"t": {"type": "integer"}}, "$ref": "#/$defs/t"}}, "properties": {"a": {"$ref": "http://x.test/r"}}, "type": "object"});
        assert_eq!(ty(nested), json!({"type": "object", "properties": {"a": {"type": "integer"}}, "additionalProperties": true}));
    }

    #[test]
    fn drafts_by_schema_keyword() {
        let named = |uri: &str| json!({"$schema": uri, "const": "a"});
        // Draft 4 has no `const`; draft 6 does.
        assert_eq!(type_schema(&named("http://json-schema.org/draft-04/schema#"), &SPEC), json!({}));
        assert_eq!(type_schema(&named("http://json-schema.org/draft-06/schema#"), &SPEC), json!({"enum": ["a"]}));
        // A spelling the validator may not recognize reads both ways: as the draft it names, and
        // as the fallback.
        let loose = Validator { known: &[("http://json-schema.org/draft-04/schema#", Draft::Four)], fallback: &[Draft::Seven], ..super::SPEC };
        assert_eq!(loose.drafts(&json!({"$schema": "https://json-schema.org/draft-04/schema"})), vec![Draft::Seven, Draft::Four]);
        assert_eq!(type_schema(&named("https://json-schema.org/draft-04/schema"), &loose), json!({}));
        assert_eq!(loose.drafts(&json!({"$schema": "http://example.com/meta"})), vec![Draft::Seven]);
    }

    #[test]
    fn boolean_schemas_by_draft() {
        assert_eq!(ty(json!({"type": "object", "properties": {"a": false}})), json!({"type": "object", "properties": {"a": false}, "additionalProperties": true}));
        // Draft 4 reads no boolean schema: the key may hold anything, so this is any object.
        assert_eq!(type_schema(&json!({"type": "object", "properties": {"a": false}}), &DRAFT4), json!({"type": "object"}));
        // Except as the value of `additionalProperties` and `additionalItems`, which draft 4 takes.
        assert_eq!(type_schema(&json!({"type": "object", "additionalProperties": false}), &DRAFT4), json!({"type": "object", "additionalProperties": false}));
        assert_eq!(
            type_schema(&json!({"type": "array", "items": [{"type": "string"}], "additionalItems": false}), &DRAFT4),
            json!({"type": "array", "prefixItems": [{"type": "string"}], "items": false})
        );
    }

    #[test]
    fn what_rapidjson_leaves_unresolved_admits_anything() {
        let mysql = MYSQL;
        assert_eq!(type_schema(&json!({"$defs": {"s": {"type": "string"}}, "$ref": "#/$defs/s"}), &mysql), json!({}));
        assert_eq!(type_schema(&json!({"definitions": {"s": {"type": "string"}}, "$ref": "#/definitions/s"}), &mysql), json!({"type": "string"}));
        assert_eq!(type_schema(&json!({"openapi": "3.0.0", "type": "string", "nullable": true}), &mysql), json!({}));
        assert_eq!(type_schema(&json!({"openapi": "3.0.0", "$schema": "http://json-schema.org/draft-04/schema#", "type": "string"}), &mysql), json!({"type": "string"}));
        assert_eq!(type_schema(&json!({"$ref": 1, "type": "string"}), &mysql), json!({}));
    }

    #[test]
    fn what_a_canonical_schema_admits() {
        let schema = ty(json!({"type": "object", "properties": {"a": {"type": "string"}, "t": {"prefixItems": [{"type": "integer"}], "items": false, "type": "array"}}, "required": ["a"], "additionalProperties": {"type": "boolean"}}));
        assert!(admits(&schema, &json!({"a": "x"})));
        assert!(admits(&schema, &json!({"a": "x", "t": [1.0], "z": true})));
        assert!(admits(&schema, &json!({"a": "x", "t": []})));
        assert!(!admits(&schema, &json!({"a": "x", "t": [1, 2]})));
        assert!(!admits(&schema, &json!({"a": "x", "z": 1})));
        assert!(!admits(&schema, &json!({"t": [1]})));
        assert!(!admits(&schema, &json!(null)));
        assert!(admits(&json!({"enum": [1, {"a": [2]}]}), &json!({"a": [2.0]})));
        assert!(!admits(&json!(false), &json!(null)));
    }

    #[test]
    fn uris_resolve_by_rfc_3986() {
        assert_eq!(resolve("http://a/b/c/d;p?q", "g").as_deref(), Some("http://a/b/c/g"));
        assert_eq!(resolve("http://a/b/c/d;p?q", "../g").as_deref(), Some("http://a/b/g"));
        assert_eq!(resolve("http://a/b/c/d;p?q", "#s").as_deref(), Some("http://a/b/c/d;p?q#s"));
        assert_eq!(resolve("http://a/b/c/d;p?q", "//g").as_deref(), Some("http://g"));
        assert_eq!(resolve("http://a/b/c/d;p?q", "/./g").as_deref(), Some("http://a/g"));
        assert_eq!(resolve(DEFAULT_BASE, "#/$defs/a").as_deref(), Some("reylo:///schema.json#/$defs/a"));
        assert_eq!(resolve("relative", "g"), None);
    }

    #[test]
    fn a_path_of_keys_reads_the_object_at_each_step() {
        let doc = type_schema(
            &json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "size": {"type": ["integer", "null"]},
                    "owner": {"type": "object", "properties": {"id": {"type": "integer"}}, "required": ["id"]},
                    "meta": {"type": ["object", "null"], "properties": {"note": {"type": "string"}}, "required": ["note"]},
                    "tags": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["name", "size", "owner"],
                "additionalProperties": {"type": "boolean"}
            }),
            &SPEC,
        );
        let at = |path: &[&str], indexes: bool| at_path(&doc, path, indexes);
        // A required key of an object, and one below it: there for certain.
        assert_eq!(at(&["name"], false), Some((json!({"type": "string"}), true)));
        assert_eq!(at(&["owner", "id"], false), Some((json!({"type": "integer"}), true)));
        // A JSON null is a value the key holds.
        assert_eq!(at(&["size"], false), Some((json!({"type": ["integer", "null"]}), true)));
        // A key left optional, one under a value that may be null, and one no property names.
        assert_eq!(at(&["tags"], false), Some((json!({"type": "array", "items": {"type": "string"}}), false)));
        assert_eq!(at(&["meta", "note"], false), Some((json!({"type": "string"}), false)));
        assert_eq!(at(&["other"], false), Some((json!({"type": "boolean"}), false)));
        // A key on a string or an array finds nothing; with indexes, a whole number on an array
        // reads an element, which is not followed.
        assert_eq!(at(&["name", "x"], false), Some((json!(false), false)));
        assert_eq!(at(&["tags", "0"], false), Some((json!(false), false)));
        assert_eq!(at(&["tags", "0"], true), None);
        assert_eq!(at(&[], false), Some((doc.clone(), true)));
        // A union reads each member; an enum of objects, the values each holds.
        let union = json!({"anyOf": [{"type": "object", "properties": {"t": {"enum": ["a"]}}, "required": ["t"], "additionalProperties": false}, {"type": "object", "properties": {"t": {"enum": ["b"]}}, "required": ["t"], "additionalProperties": false}]});
        assert_eq!(at_path(&union, &["t"], false), Some((json!({"anyOf": [{"enum": ["a"]}, {"enum": ["b"]}]}), true)));
        assert_eq!(at_path(&union, &["u"], false), Some((json!(false), false)));
        assert_eq!(at_path(&json!({"enum": [{"k": 1}, {"k": 1}, {"j": 2}]}), &["k"], false), Some((json!({"enum": [1]}), false)));
        assert_eq!(at_path(&json!(true), &["k"], false), Some((json!(true), false)));
        assert_eq!(at_path(&json!({"not": {"type": "string"}}), &["k"], false), None);
    }

    #[test]
    fn a_deep_or_malformed_text_is_not_read() {
        assert_eq!(type_schema_text("{\"type\": ", &SPEC), None);
        let deep = format!("{}{}", "[".repeat(600), "]".repeat(600));
        assert_eq!(type_schema_text(&deep, &SPEC), None);
        assert_eq!(type_schema_text(r#"{"type": "string", "pattern": "[{"}"#, &SPEC), Some(json!({"type": "string"})));
    }
}
