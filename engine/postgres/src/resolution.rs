//! Which overload the server picks for a call of an operator or a function whose name a user or
//! an extension also defined, as far as the build can tell from the operands' types.
//!
//! A rule of [`crate::infer`] reads a built-in by its name, and an extension that defines `=`,
//! `+`, `sum` or `lower` for its own types (pgvector, PostGIS, citext, hstore and most others do)
//! shadows the name. The server picks an overload by the operands' types (`oper` and
//! `func_get_detail` in the server's `parse_oper.c` and `parse_func.c`):
//!
//! 1. An exact match on the argument types wins. For an infix operator one untyped operand, a
//!    string literal, a NULL or a parameter read for the first time, takes the other one's type
//!    (`binary_oper_exact`); a function's arguments are matched as they are.
//! 2. Otherwise the candidates are the overloads every argument converts to implicitly
//!    (`func_match_argtypes`, by `can_coerce_type`), an untyped one converting to anything, and
//!    the server ranks them (`func_select_candidate`).
//!
//! So a call still reads as the built-in where its operands are of built-in types the build can
//! name and either a built-in overload takes exactly those types and no overload of a user does,
//! or no overload of a user is among the candidates at all. Anything else proves nothing: an
//! operand whose type the build cannot name, one of a type a user defined (a `vector` or `citext`
//! column, whose operator is the extension's), and a call that may reach a user's overload through
//! an implicit cast with no exact built-in to stop it. A parameter is read both ways, untyped and
//! of the type its describe gave it, since only its first read in the statement is untyped. The
//! ranking of candidates is never read: where a user's overload is among them, it may be the one.
//!
//! The catalog answers in one session-scoped query, [`crate::postgres::TRUST_QUERY`], which also
//! says which built-in types a user's overload's argument converts from (with the server's own
//! rules: a domain by its base type, an array by its elements), and which built-in types a user's
//! implicit cast function converts, which a candidate over built-in types would then call.

use crate::host::{as_str, as_u32};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The first oid a database assigns to an object of its own (`FirstNormalObjectId`).
const FIRST_USER_OID: u32 = 16384;
/// `record` and `record[]`, which the server converts to and from any composite type.
const RECORD_TYPES: &[u32] = &[2249, 2287];

/// How a call names what it calls: an infix operator (`a + b`), a prefix one (`-a`), or a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Infix,
    Prefix,
    Function,
    /// A cast to the built-in type of the name.
    Cast,
}

/// An operand's type, as the server's resolution reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    /// A string literal: `unknown`, which takes the type of the overload's argument.
    Unknown,
    /// A parameter, `unknown` where the statement first reads it and of the type its describe gave
    /// it after; `None` where the build has no describe of the statement.
    Param(Option<u32>),
    /// A value of this type.
    Is(u32),
    /// A value whose type the build cannot name: an expression the rules do not type, or a NULL,
    /// which a probe writes in place of a parameter of any type.
    Opaque,
}

/// One overload a user or an extension defined of a name the rules read.
#[derive(Debug, Clone, PartialEq)]
struct UserOverload {
    args: Vec<u32>,
    /// How many of a function's last arguments have defaults.
    defaults: usize,
    variadic: bool,
}

/// One built-in overload whose arguments are all types of their own (no polymorphic one), with
/// the type it returns.
#[derive(Debug, Clone, PartialEq)]
struct Builtin {
    args: Vec<u32>,
    defaults: usize,
    result: u32,
}

/// Bits of [`Overloads::flags`], over every overload of a user's of one name and kind.
const RETURNS_SET: u32 = 2;
const VOLATILE: u32 = 4;
const NOT_AGGREGATE: u32 = 8;

/// What the catalog says of the overloads of the names a rule reads that a user or an extension
/// also defined.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overloads {
    /// Every user's overload one of whose arguments a built-in type converts to.
    user: HashMap<(Kind, String), Vec<UserOverload>>,
    /// The arities of a user's overloads no built-in type converts to at any argument, each of one
    /// argument or more: only a call whose every operand is untyped may reach one. The query
    /// reports one of no argument whole, which a call of none matches exactly.
    opaque: HashMap<(Kind, String), HashSet<usize>>,
    /// The built-in overloads of those names over the common types.
    builtin: HashMap<(Kind, String), Vec<Builtin>>,
    /// Per argument type of a user's overload, the built-in types the server converts to it
    /// implicitly, itself included.
    reach: HashMap<u32, HashSet<u32>>,
    /// The argument types every type converts to: the pseudo-types the query does not spell out.
    any: HashSet<u32>,
    /// The base type of every argument type of a user's overload that is a domain.
    bases: HashMap<u32, u32>,
    /// Built-in types a user's implicit cast function converts to another built-in type.
    implicit: HashSet<u32>,
    /// Function names a user's type also has: a call of one argument with no exact match may be a
    /// cast to it.
    typed: HashSet<String>,
    /// Per name and kind, the bits of every user's overload of it, or-ed.
    flags: HashMap<(Kind, String), u32>,
    /// Per built-in type name a user's cast function targets, the type's oid and the built-in
    /// types a cast to it from may call such a function.
    casts: HashMap<String, (u32, HashSet<u32>)>,
}

impl Overloads {
    /// Read the rows of [`crate::postgres::TRUST_QUERY`] that describe overloads; the names any of
    /// them shadows go into `shadowed`.
    pub fn read(rows: &[Vec<Value>], shadowed: &mut HashSet<String>) -> Overloads {
        let mut out = Overloads::default();
        let oids = |v: Option<&Value>| -> Option<Vec<u32>> { v?.as_array()?.iter().map(as_u32).collect() };
        let kind = |k: &str| match k {
            "b" => Some(Kind::Infix),
            "l" => Some(Kind::Prefix),
            "f" => Some(Kind::Function),
            _ => None,
        };
        for row in rows {
            let Some(tag) = row.first().and_then(as_str) else { continue };
            let name = row.get(1).and_then(as_str);
            let a = row.get(3).and_then(as_u32);
            let b = row.get(4).and_then(as_u32);
            match tag.split_once(' ') {
                Some(("user", k)) => {
                    let (Some(kind), Some(name), Some(args)) = (kind(k), name, oids(row.get(2))) else { continue };
                    shadowed.insert(name.to_string());
                    let flags = b.unwrap_or(u32::MAX);
                    *out.flags.entry((kind, name.to_string())).or_default() |= flags;
                    out.user.entry((kind, name.to_string())).or_default().push(UserOverload { args, defaults: a.unwrap_or(0) as usize, variadic: flags & 1 == 1 });
                }
                Some(("opaque", k)) => {
                    let (Some(kind), Some(name), Some(arity)) = (kind(k), name, a) else { continue };
                    shadowed.insert(name.to_string());
                    *out.flags.entry((kind, name.to_string())).or_default() |= b.unwrap_or(u32::MAX);
                    out.opaque.entry((kind, name.to_string())).or_default().insert(arity as usize);
                }
                Some(("builtin", k)) => {
                    let (Some(kind), Some(name), Some(args), Some(result)) = (kind(k), name, oids(row.get(2)), a) else { continue };
                    out.builtin.entry((kind, name.to_string())).or_default().push(Builtin { args, defaults: b.unwrap_or(0) as usize, result });
                }
                _ => match tag {
                    "cast from" => {
                        if let (Some(name), Some(target), Some(sources)) = (name, a, oids(row.get(2))) {
                            out.casts.insert(name.to_string(), (target, sources.into_iter().collect()));
                        }
                    }
                    "reach" => {
                        if let (Some(target), Some(sources)) = (a, oids(row.get(2))) {
                            out.reach.entry(target).or_default().extend(sources);
                        }
                    }
                    "any" => out.any.extend(a),
                    "base" => {
                        if let (Some(domain), Some(base)) = (a, b) {
                            out.bases.insert(domain, base);
                        }
                    }
                    "implicit" => out.implicit.extend(a),
                    "type" => {
                        if let Some(name) = name {
                            shadowed.insert(name.to_string());
                            out.typed.insert(name.to_string());
                        }
                    }
                    _ => {}
                },
            }
        }
        out
    }

    /// Whether a call of `name` over operands of these types resolves to a built-in, and the type
    /// that built-in returns where one built-in takes exactly those types: `Some(Some(type))` or
    /// `Some(None)`. `None` where an overload of a user's may be the one. Only a name the
    /// catalog reports a user's overload of is asked: any other is the built-in.
    pub fn resolve(&self, kind: Kind, name: &str, operands: &[Operand]) -> Option<Option<u32>> {
        // A parameter is read both ways: untyped where the statement first reads it, typed after.
        let mut readings: Vec<Vec<Operand>> = vec![Vec::with_capacity(operands.len())];
        for operand in operands {
            let ways: &[Operand] = match operand {
                Operand::Param(Some(t)) => &[Operand::Unknown, Operand::Is(*t)],
                Operand::Param(None) => &[Operand::Opaque],
                other => std::slice::from_ref(other),
            };
            readings = readings.into_iter().flat_map(|r| ways.iter().map(move |w| r.iter().copied().chain([*w]).collect())).collect();
            // Every reading is a resolution of its own; a call of many parameters proves nothing.
            if readings.len() > 16 {
                return None;
            }
        }
        let mut result: Option<Option<u32>> = None;
        for reading in &readings {
            let found = self.resolve_one(kind, name, reading)?;
            result = Some(match result {
                None => found,
                Some(known) if known == found => known,
                Some(_) => None,
            });
        }
        result
    }

    fn resolve_one(&self, kind: Kind, name: &str, operands: &[Operand]) -> Option<Option<u32>> {
        // A cast of an untyped value reads it with the type's input function; of a value of a
        // built-in type, it runs the cast between the two, which is a user's only from the
        // sources of their casts to the type (`find_coercion_pathway`, by base types).
        if kind == Kind::Cast {
            let (target, sources) = self.casts.get(name)?;
            return match operands {
                [Operand::Unknown] => Some(Some(*target)),
                [Operand::Is(t)] if *t < FIRST_USER_OID && !RECORD_TYPES.contains(t) && !sources.contains(t) => Some(Some(*target)),
                _ => None,
            };
        }
        if operands.iter().any(|o| match o {
            Operand::Opaque | Operand::Param(_) => true,
            Operand::Is(t) => *t >= FIRST_USER_OID || RECORD_TYPES.contains(t),
            Operand::Unknown => false,
        }) {
            return None;
        }
        let key = (kind, name.to_string());
        let users = self.user.get(&key).map(Vec::as_slice).unwrap_or(&[]);
        // The exact match, an infix operator's untyped operand taking the other one's type.
        let exact: Option<Vec<u32>> = match (kind, operands) {
            (Kind::Infix, [Operand::Unknown, Operand::Is(t)] | [Operand::Is(t), Operand::Unknown]) => Some(vec![*t, *t]),
            (Kind::Infix, [Operand::Is(l), Operand::Is(r)]) => Some(vec![*l, *r]),
            (Kind::Infix, _) => None,
            (Kind::Prefix, [Operand::Is(t)]) => Some(vec![*t]),
            (Kind::Prefix, _) => None,
            (Kind::Function, _) => operands.iter().map(|o| match o {
                Operand::Is(t) => Some(*t),
                _ => None,
            }).collect(),
            (Kind::Cast, _) => return None,
        };
        if let Some(exact) = &exact {
            // A user's overload of exactly those types may come first on the search path, and
            // one with defaults or a variadic argument may expand to them. A column of a domain
            // is described as of the domain's base type, which the server matched as the domain:
            // an overload of a domain matches a value of its base type too.
            let base = |t: &u32| self.bases.get(t).copied().unwrap_or(*t);
            if users.iter().any(|u| u.args == *exact || u.args.iter().map(base).eq(exact.iter().copied()) || (kind == Kind::Function && (u.defaults > 0 || u.variadic) && u.fits(exact.len()))) {
                return None;
            }
            let mut matching = self.builtin.get(&key).into_iter().flatten().filter(|b| b.matches(exact));
            if let Some(first) = matching.next() {
                let result = first.result;
                return Some(matching.all(|b| b.result == result).then_some(result));
            }
        }
        // No exact match: every candidate must be a built-in, reached by built-in conversions. A
        // function named like a type may be a cast to it.
        if kind == Kind::Function && operands.len() == 1 && self.typed.contains(name) {
            return None;
        }
        if users.iter().any(|u| u.reachable(kind, operands, self)) {
            return None;
        }
        if operands.iter().all(|o| *o == Operand::Unknown) && self.opaque.get(&key).is_some_and(|arities| arities.contains(&operands.len())) {
            return None;
        }
        if operands.iter().any(|o| matches!(o, Operand::Is(t) if self.implicit.contains(t))) {
            return None;
        }
        Some(None)
    }

    /// Whether a user's function converts some built-in type to another implicitly, which a call
    /// of any name over a value of that type may run ([`Overloads::implicit`]).
    pub fn casts_implicitly(&self) -> bool {
        !self.implicit.is_empty()
    }

    /// Whether every overload of a user's of a function `name` is an aggregate, and none returns a
    /// set, so a call of the name aggregates whichever overload the server picks. `None` where
    /// the catalog reports none.
    pub fn every_aggregate(&self, name: &str) -> Option<bool> {
        // A call of one argument with no exact match may be a cast to a type of the name
        // (`func_get_detail`), which reads each row's value.
        if self.typed.contains(name) {
            return Some(false);
        }
        let flags = self.flags.get(&(Kind::Function, name.to_string()))?;
        Some(flags & (NOT_AGGREGATE | RETURNS_SET) == 0)
    }

    /// Whether some overload of a user's of `name`, of this kind, returns a set; `None` where the
    /// catalog reports none of that kind.
    pub fn returns_set(&self, kind: Kind, name: &str) -> Option<bool> {
        self.flags.get(&(kind, name.to_string())).map(|f| f & RETURNS_SET != 0)
    }

    /// Whether some operator of a user's named `name`, infix or prefix, returns a set; `None`
    /// where the catalog reports none.
    pub fn operator_returns_set(&self, name: &str) -> Option<bool> {
        let flags = [Kind::Infix, Kind::Prefix].iter().filter_map(|k| self.flags.get(&(*k, name.to_string()))).fold(None, |acc: Option<u32>, f| Some(acc.unwrap_or(0) | f))?;
        Some(flags & RETURNS_SET != 0)
    }

    /// Whether some overload of a user's of the operator `name` is volatile, or returns a set;
    /// `None` where the catalog reports none.
    pub fn operator_volatile(&self, name: &str) -> Option<bool> {
        let flags = [Kind::Infix, Kind::Prefix].iter().filter_map(|k| self.flags.get(&(*k, name.to_string()))).fold(None, |acc: Option<u32>, f| Some(acc.unwrap_or(0) | f))?;
        Some(flags & (VOLATILE | RETURNS_SET) != 0)
    }

    /// Whether `source`, a built-in type, converts to `target` implicitly, as `can_coerce_type`
    /// reads it.
    fn converts(&self, source: u32, target: u32) -> bool {
        source == target || self.any.contains(&target) || self.reach.get(&target).is_some_and(|s| s.contains(&source))
    }
}

impl UserOverload {
    /// Whether a function of this overload's arguments takes `n` of them.
    fn fits(&self, n: usize) -> bool {
        let required = self.args.len().saturating_sub(self.defaults);
        // A variadic argument takes any number of values, none included.
        if self.variadic {
            n + 1 >= required
        } else {
            n >= required && n <= self.args.len()
        }
    }

    /// Whether the server takes this overload for a candidate of a call over `operands`: each
    /// operand converts to the argument at its position, an untyped one to anything. One with
    /// defaults or a variadic argument is a candidate whenever it takes that many arguments.
    fn reachable(&self, kind: Kind, operands: &[Operand], catalog: &Overloads) -> bool {
        if kind == Kind::Function && (self.defaults > 0 || self.variadic) {
            return self.fits(operands.len());
        }
        self.args.len() == operands.len()
            && self.args.iter().zip(operands).all(|(arg, operand)| match operand {
                Operand::Is(t) => catalog.converts(*t, *arg),
                _ => true,
            })
    }
}

impl Builtin {
    /// Whether the server's exact match takes this overload for a call over exactly `args`, a
    /// function's defaults filling the arguments the call leaves out.
    fn matches(&self, args: &[u32]) -> bool {
        args.len() <= self.args.len() && args.len() + self.defaults >= self.args.len() && self.args[..args.len()] == *args
    }
}

/// The catalog's overloads, shared by every plan of a session; two are equal only when they are
/// the same answer, which keeps comparing expressions cheap.
#[derive(Debug, Clone)]
pub struct Shared(pub Arc<Overloads>);

impl PartialEq for Shared {
    fn eq(&self, other: &Shared) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pgtypes::oid;
    use serde_json::json;

    const VECTOR: u32 = 20000;
    const CITEXT: u32 = 20100;
    const GEOMETRY: u32 = 20200;

    /// A catalog of the overloads the rows name, as the trust query reports them.
    fn catalog(rows: Vec<Vec<Value>>) -> (Overloads, HashSet<String>) {
        let mut shadowed = HashSet::new();
        let overloads = Overloads::read(&rows, &mut shadowed);
        (overloads, shadowed)
    }

    fn user(kind: &str, name: &str, args: &[u32]) -> Vec<Value> {
        vec![json!(format!("user {kind}")), json!(name), json!(args), json!(0), json!(0)]
    }

    fn builtin(kind: &str, name: &str, args: &[u32], result: u32) -> Vec<Value> {
        vec![json!(format!("builtin {kind}")), json!(name), json!(args), json!(result), Value::Null]
    }

    fn reach(target: u32, sources: &[u32]) -> Vec<Value> {
        vec![json!("reach"), Value::Null, json!(sources), json!(target), Value::Null]
    }

    /// pgvector's `=` and `+`, no built-in type converting to `vector`; PostGIS's `=`, which
    /// `text` converts to implicitly; and the built-in overloads over the common types.
    fn extensions() -> Overloads {
        catalog(vec![
            vec![json!("opaque b"), json!("="), Value::Null, json!(2), json!(0)],
            vec![json!("opaque b"), json!("+"), Value::Null, json!(2), json!(0)],
            user("b", "=", &[GEOMETRY, GEOMETRY]),
            reach(GEOMETRY, &[oid::TEXT, oid::BYTEA]),
            builtin("b", "=", &[oid::INT8, oid::INT8], oid::BOOL),
            builtin("b", "=", &[oid::INT8, oid::INT4], oid::BOOL),
            builtin("b", "=", &[oid::TEXT, oid::TEXT], oid::BOOL),
            builtin("b", "+", &[oid::INT4, oid::INT4], oid::INT4),
            builtin("l", "-", &[oid::INT4], oid::INT4),
        ])
        .0
    }

    #[test]
    fn an_exact_built_in_wins_over_a_user_overload_the_operands_convert_to() {
        let c = extensions();
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::INT8), Operand::Is(oid::INT4)]), Some(Some(oid::BOOL)));
        assert_eq!(c.resolve(Kind::Infix, "+", &[Operand::Is(oid::INT4), Operand::Is(oid::INT4)]), Some(Some(oid::INT4)));
        // `text` converts to `geometry`, but `text = text` is an exact match, and so is a column
        // beside a string literal, which takes the column's type.
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::TEXT), Operand::Is(oid::TEXT)]), Some(Some(oid::BOOL)));
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::TEXT), Operand::Unknown]), Some(Some(oid::BOOL)));
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Unknown, Operand::Is(oid::TEXT)]), Some(Some(oid::BOOL)));
    }

    #[test]
    fn a_user_overload_among_the_candidates_proves_nothing() {
        let c = extensions();
        // No `=(text, bytea)`: both convert to `geometry`, whose `=` is a candidate.
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::TEXT), Operand::Is(oid::BYTEA)]), None);
        // `varchar` converts to `text` but not to `geometry`: only built-ins are candidates.
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::VARCHAR), Operand::Is(oid::TEXT)]), Some(None));
        // Two untyped operands reach any overload, pgvector's included.
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Unknown, Operand::Unknown]), None);
        assert_eq!(c.resolve(Kind::Infix, "+", &[Operand::Unknown, Operand::Unknown]), None);
        // A prefix `-` has no user overload; an infix `-` is another lookup.
        assert_eq!(c.resolve(Kind::Prefix, "-", &[Operand::Is(oid::INT4)]), Some(Some(oid::INT4)));
    }

    #[test]
    fn an_operand_of_a_type_the_build_cannot_vouch_for_proves_nothing() {
        let c = extensions();
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(VECTOR), Operand::Is(VECTOR)]), None);
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(CITEXT), Operand::Unknown]), None);
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Opaque, Operand::Is(oid::INT4)]), None);
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(2249), Operand::Is(2249)]), None, "record");
    }

    #[test]
    fn a_parameter_is_read_untyped_and_typed() {
        let c = extensions();
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::TEXT), Operand::Param(Some(oid::TEXT))]), Some(Some(oid::BOOL)));
        // Typed by an earlier read as `bytea`, it reaches PostGIS's `=` beside a `text`.
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::TEXT), Operand::Param(Some(oid::BYTEA))]), None);
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::TEXT), Operand::Param(None)]), None, "no describe");
    }

    #[test]
    fn a_user_overload_of_a_domain_matches_a_value_of_its_base_type() {
        // A domain `mytext` over `text` with an `=` and a `lower` of its own: the describe reports
        // a column of it as a `text`, and the server matches it as the domain.
        const MYTEXT: u32 = 30000;
        let (c, _) = catalog(vec![
            user("b", "=", &[MYTEXT, MYTEXT]),
            user("f", "lower", &[MYTEXT]),
            reach(MYTEXT, &[oid::TEXT, oid::VARCHAR]),
            vec![json!("base"), Value::Null, Value::Null, json!(MYTEXT), json!(oid::TEXT)],
            builtin("b", "=", &[oid::TEXT, oid::TEXT], oid::BOOL),
            builtin("f", "lower", &[oid::TEXT], oid::TEXT),
        ]);
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::TEXT), Operand::Unknown]), None);
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::TEXT), Operand::Is(oid::TEXT)]), None);
        assert_eq!(c.resolve(Kind::Function, "lower", &[Operand::Is(oid::TEXT)]), None);
        // Over an `int4` nothing of the domain's is a candidate.
        assert_eq!(c.resolve(Kind::Infix, "=", &[Operand::Is(oid::INT4), Operand::Is(oid::INT4)]), Some(None));
    }

    #[test]
    fn a_user_overload_of_exactly_the_built_in_types_may_be_the_one() {
        // pg_trgm's `%(text, text)` and an anonymizer's own `lower(text)`, in a schema of its own.
        let (c, shadowed) = catalog(vec![user("b", "%", &[oid::TEXT, oid::TEXT]), user("f", "lower", &[oid::TEXT]), builtin("b", "%", &[oid::INT4, oid::INT4], oid::INT4), builtin("f", "lower", &[oid::TEXT], oid::TEXT)]);
        assert!(shadowed.contains("%") && shadowed.contains("lower"));
        assert_eq!(c.resolve(Kind::Infix, "%", &[Operand::Is(oid::TEXT), Operand::Is(oid::TEXT)]), None);
        assert_eq!(c.resolve(Kind::Infix, "%", &[Operand::Is(oid::INT4), Operand::Is(oid::INT4)]), Some(Some(oid::INT4)));
        assert_eq!(c.resolve(Kind::Function, "lower", &[Operand::Is(oid::TEXT)]), None);
    }

    #[test]
    fn a_function_matches_its_arguments_as_they_are() {
        // ip4r's `lower(ip4r)`: `lower(text)` is exact, an untyped argument reaches every overload.
        let (c, _) = catalog(vec![vec![json!("opaque f"), json!("lower"), Value::Null, json!(1), json!(8)], builtin("f", "lower", &[oid::TEXT], oid::TEXT)]);
        assert_eq!(c.resolve(Kind::Function, "lower", &[Operand::Is(oid::TEXT)]), Some(Some(oid::TEXT)));
        assert_eq!(c.resolve(Kind::Function, "lower", &[Operand::Unknown]), None);
        assert_eq!(c.resolve(Kind::Function, "lower", &[Operand::Is(oid::VARCHAR)]), Some(None));
        // pgvector's `sum(vector)` is an aggregate as the built-ins are.
        let (c, _) = catalog(vec![vec![json!("opaque f"), json!("sum"), Value::Null, json!(1), json!(0)], builtin("f", "sum", &[oid::INT4], oid::INT8)]);
        assert_eq!(c.resolve(Kind::Function, "sum", &[Operand::Is(oid::INT4)]), Some(Some(oid::INT8)));
        assert_eq!(c.every_aggregate("sum"), Some(true));
        assert_eq!(c.every_aggregate("count"), None);
        // A domain named `max` beside a user's aggregate `max(bytea)`: `max(v)` of a `varchar`
        // is a cast to the domain.
        let (c, _) = catalog(vec![vec![json!("opaque f"), json!("max"), Value::Null, json!(1), json!(0)], vec![json!("type"), json!("max"), Value::Null, Value::Null, Value::Null]]);
        assert_eq!(c.every_aggregate("max"), Some(false));
    }

    #[test]
    fn defaults_variadics_and_types_named_like_a_function_stand_in_for_any_match() {
        let (c, _) = catalog(vec![vec![json!("user f"), json!("round"), json!([oid::TEXT, oid::INT4]), json!(1), json!(8)], builtin("f", "round", &[oid::NUMERIC], oid::NUMERIC)]);
        assert_eq!(c.resolve(Kind::Function, "round", &[Operand::Is(oid::NUMERIC)]), None, "round(text, int default 0) takes one argument");
        assert_eq!(c.resolve(Kind::Function, "round", &[Operand::Is(oid::NUMERIC), Operand::Is(oid::INT4), Operand::Is(oid::INT4)]), Some(None));
        let (c, _) = catalog(vec![vec![json!("type"), json!("upper"), Value::Null, Value::Null, Value::Null], builtin("f", "upper", &[oid::TEXT], oid::TEXT)]);
        assert_eq!(c.resolve(Kind::Function, "upper", &[Operand::Is(oid::TEXT)]), Some(Some(oid::TEXT)));
        assert_eq!(c.resolve(Kind::Function, "upper", &[Operand::Is(oid::VARCHAR)]), None, "upper(x) may be x::upper");
    }

    #[test]
    fn a_user_implicit_cast_function_between_built_in_types_proves_nothing_without_an_exact_match() {
        let (c, _) = catalog(vec![vec![json!("opaque b"), json!("+"), Value::Null, json!(2), json!(0)], vec![json!("implicit"), Value::Null, Value::Null, json!(oid::TEXT), Value::Null], builtin("b", "+", &[oid::INT4, oid::INT4], oid::INT4)]);
        assert_eq!(c.resolve(Kind::Infix, "+", &[Operand::Is(oid::TEXT), Operand::Is(oid::INT4)]), None);
        assert_eq!(c.resolve(Kind::Infix, "+", &[Operand::Is(oid::INT4), Operand::Is(oid::INT4)]), Some(Some(oid::INT4)));
    }

    #[test]
    fn a_cast_to_a_type_a_user_casts_to_is_theirs_only_from_their_source() {
        // PostGIS's cast of a `geometry` to `text`.
        let (c, _) = catalog(vec![vec![json!("cast from"), json!("text"), json!([]), json!(oid::TEXT), Value::Null], vec![json!("cast from"), json!("int8"), json!([oid::BOOL]), json!(oid::INT8), Value::Null]]);
        assert_eq!(c.resolve(Kind::Cast, "text", &[Operand::Is(oid::INT4)]), Some(Some(oid::TEXT)));
        assert_eq!(c.resolve(Kind::Cast, "text", &[Operand::Unknown]), Some(Some(oid::TEXT)));
        assert_eq!(c.resolve(Kind::Cast, "text", &[Operand::Is(GEOMETRY)]), None);
        assert_eq!(c.resolve(Kind::Cast, "text", &[Operand::Opaque]), None);
        // A user's own cast of a `bool` to `int8`.
        assert_eq!(c.resolve(Kind::Cast, "int8", &[Operand::Is(oid::BOOL)]), None);
        assert_eq!(c.resolve(Kind::Cast, "int8", &[Operand::Param(Some(oid::BOOL))]), None);
        assert_eq!(c.resolve(Kind::Cast, "int8", &[Operand::Param(Some(oid::INT4))]), Some(Some(oid::INT8)));
        // A name no row reports: a user's type of the name, or a host that did not ask.
        assert_eq!(c.resolve(Kind::Cast, "uuid", &[Operand::Is(oid::TEXT)]), None);
    }

    #[test]
    fn a_user_function_of_no_arguments_matches_a_call_of_none() {
        // pgcrypto's own `gen_random_uuid()`, which the search path may put first.
        let (c, _) = catalog(vec![vec![json!("user f"), json!("gen_random_uuid"), json!([]), json!(0), json!(12)], builtin("f", "gen_random_uuid", &[], oid::UUID)]);
        assert_eq!(c.resolve(Kind::Function, "gen_random_uuid", &[]), None);
    }
}
