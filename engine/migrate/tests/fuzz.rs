//! A fuzzer for the diff engine, run by hand: `REYLO_FUZZ=<steps>` (and `REYLO_FUZZ_SEED`,
//! `REYLO_FUZZ_OUT`) with the server `REYLO_TEST_PG` names. Each step makes a random schema, changes it
//! by a few random mutations, fills the first with rows both accept, plans from one to the other and
//! runs the plan as a migration runs it. The plan must run, the result must read as a fresh build of
//! the target, every column kept must keep its values unless the plan says it loses them, a second
//! plan must be empty, and the way back (from the target without rows) must work too. A step that
//! fails is shrunk to the fewest objects that still fail the same way and written to `REYLO_FUZZ_OUT`
//! as a case for `tests/diff/postgres/`. Without `REYLO_FUZZ` it does nothing.
//!
//! The schemas lean on what objects do to each other: functions whose signatures use the enum, a
//! domain or a composite type, called from defaults, checks, views and policies, or reading a table's
//! column from a SQL-standard body; domains and composite types as columns' types; triggers, row
//! level security, sequences a column owns, identity, partitions, comments and grants; and names and
//! labels that need quoting.

use reylo_migrate::catalog::{catalog, difference};
use reylo_migrate::db::{with_database, Db};
use reylo_migrate::diff;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// A small, seeded random source (xorshift).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() { None } else { Some(&items[self.below(items.len())]) }
    }
}

/// A name as SQL writes it: quoted unless it is a plain lowercase identifier.
fn q(name: &str) -> String {
    let plain = name.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '_') && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if plain { name.into() } else { format!("\"{}\"", name.replace('"', "\"\"")) }
}

/// A string literal.
fn lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// A name made from `base`, most often as it is, else in one of the forms that need quoting.
fn styled(rng: &mut Rng, base: String) -> String {
    if !rng.chance(30) {
        return base;
    }
    match rng.below(6) {
        0 => base[..1].to_uppercase() + &base[1..],
        1 => format!("{base} x"),
        2 => format!("{base}|x"),
        3 => format!("{base},x"),
        4 => format!("{base}.x"),
        _ => format!("{base}\"x"),
    }
}

const NOTES: &[&str] = &["note", "it's here", "a, b | c.d", "two\nlines"];

#[derive(Clone, Debug, PartialEq)]
enum Ty {
    SmallInt,
    Int,
    BigInt,
    Numeric,
    Text,
    Varchar(u32),
    Bool,
    Date,
    Timestamp,
    Jsonb,
    Uuid,
    IntArray,
    TextArray,
    Enum(String),
    EnumArray(String),
    Domain(String),
    Composite(String),
}

impl Ty {
    fn sql(&self) -> String {
        match self {
            Ty::SmallInt => "smallint".into(),
            Ty::Int => "integer".into(),
            Ty::BigInt => "bigint".into(),
            Ty::Numeric => "numeric".into(),
            Ty::Text => "text".into(),
            Ty::Varchar(n) => format!("varchar({n})"),
            Ty::Bool => "boolean".into(),
            Ty::Date => "date".into(),
            Ty::Timestamp => "timestamp".into(),
            Ty::Jsonb => "jsonb".into(),
            Ty::Uuid => "uuid".into(),
            Ty::IntArray => "integer[]".into(),
            Ty::TextArray => "text[]".into(),
            Ty::Enum(e) | Ty::Domain(e) | Ty::Composite(e) => q(e),
            Ty::EnumArray(e) => format!("{}[]", q(e)),
        }
    }
    fn integral(&self) -> bool {
        matches!(self, Ty::SmallInt | Ty::Int | Ty::BigInt)
    }
    fn stringy(&self) -> bool {
        matches!(self, Ty::Text | Ty::Varchar(_))
    }
    /// Numbers and strings, which the fuzzer's values of each cast to the others.
    fn family(&self) -> bool {
        self.integral() || self.stringy() || *self == Ty::Numeric
    }
    /// What the type names, so that a column of it goes with that.
    fn names(&self) -> Option<&str> {
        match self {
            Ty::Enum(e) | Ty::EnumArray(e) | Ty::Domain(e) | Ty::Composite(e) => Some(e),
            _ => None,
        }
    }
}

/// A function's body: a constant (`k` the row its value is of) written as a quoted body, a
/// SQL-standard body or a `return`, or a SQL-standard body that reads a table's column.
#[derive(Clone, Debug, PartialEq)]
enum Body {
    Quoted(usize),
    Atomic(usize),
    Return(usize),
    Count(String, String),
    Min(String, String),
}

#[derive(Clone, Debug)]
struct Func {
    name: String,
    args: Vec<Ty>,
    ret: Ty,
    body: Body,
    revoked: bool,
    comment: Option<String>,
}

impl Func {
    fn reads(&self) -> Option<(&str, &str)> {
        match &self.body {
            Body::Count(t, c) | Body::Min(t, c) => Some((t, c)),
            _ => None,
        }
    }
    fn signature(&self) -> String {
        format!("{}({})", q(&self.name), self.args.iter().map(|a| a.sql()).collect::<Vec<_>>().join(", "))
    }
}

#[derive(Clone, Debug)]
struct Domain {
    name: String,
    base: Ty,
    default: bool,
    /// The name of its check, which every value the fuzzer writes passes.
    check: Option<String>,
    not_null: bool,
    comment: Option<String>,
}

#[derive(Clone, Debug)]
struct Composite {
    name: String,
    attrs: Vec<(String, Ty)>,
    comment: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum Cexpr {
    Positive,
    Length,
    /// `f(col) = f(col)`, or `f() = f()` for a function without arguments: true or null for every row.
    Call(String),
}

#[derive(Clone, Debug)]
struct Check {
    name: String,
    /// The column it reads, empty where it reads none.
    col: String,
    expr: Cexpr,
}

#[derive(Clone, Debug)]
struct View {
    name: String,
    table: String,
    cols: Vec<String>,
    /// A function it selects, of one of the table's columns or of constants.
    call: Option<(String, Option<String>)>,
    /// The view it selects from, of the same table, in place of the table.
    over: Option<String>,
    materialized: bool,
    invoker: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Pexpr {
    True,
    NotNull(String),
    Call(String, Option<String>),
}

#[derive(Clone, Debug)]
struct Policy {
    name: String,
    table: String,
    expr: Pexpr,
}

#[derive(Clone, Debug)]
struct Trigger {
    name: String,
    table: String,
    /// Which body its plpgsql function has.
    variant: usize,
    /// The column `update of` names.
    of: Option<String>,
    /// The column its `when` reads.
    when: Option<String>,
}

/// A table partitioned by `id`: each partition's bounds, `lo` to `hi` (list partitions hold each
/// value between), or none for the default partition.
#[derive(Clone, Debug)]
struct Part {
    list: bool,
    children: Vec<(String, Option<(i64, i64)>)>,
}

#[derive(Clone, Debug)]
struct Col {
    name: String,
    ty: Ty,
    not_null: bool,
    default: Option<String>,
    /// A function its default calls, in place of `default`.
    call: Option<String>,
    /// A stored generated column's expression, over another integral column.
    generated: Option<String>,
    /// A function a generated column's expression calls, of another column, in place of `generated`.
    gen_call: Option<(String, String)>,
    identity: bool,
    start: Option<i64>,
    /// A default that takes the next value of a sequence the column owns.
    seq: bool,
    comment: Option<String>,
}

fn col(name: &str, ty: Ty) -> Col {
    Col { name: name.into(), ty, not_null: false, default: None, call: None, generated: None, gen_call: None, identity: false, start: None, seq: false, comment: None }
}

#[derive(Clone, Debug)]
struct Table {
    name: String,
    cols: Vec<Col>,
    pk: Vec<String>,
    uniques: Vec<(String, Vec<String>)>,
    checks: Vec<Check>,
    indexes: Vec<(String, Vec<String>, Option<String>)>,
    /// Indexes on a function of a column.
    fn_indexes: Vec<(String, String, String)>,
    /// Extended statistics over columns.
    stats: Vec<(String, Vec<String>)>,
    fks: Vec<(String, String, String, String)>,
    part: Option<Part>,
    rls: bool,
    granted: bool,
    comment: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct Model {
    enums: Vec<(String, Vec<String>)>,
    enum_comment: Option<String>,
    domains: Vec<Domain>,
    composites: Vec<Composite>,
    funcs: Vec<Func>,
    tables: Vec<Table>,
    views: Vec<View>,
    triggers: Vec<Trigger>,
    policies: Vec<Policy>,
}

impl Model {
    fn labels(&self, e: &str) -> Vec<String> {
        self.enums.iter().find(|(n, _)| n == e).map(|(_, l)| l.clone()).unwrap_or_default()
    }
    fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
    }
    fn func(&self, name: &str) -> Option<&Func> {
        self.funcs.iter().find(|f| f.name == name)
    }
    fn domain(&self, name: &str) -> Option<&Domain> {
        self.domains.iter().find(|d| d.name == name)
    }
    fn composite(&self, name: &str) -> Option<&Composite> {
        self.composites.iter().find(|c| c.name == name)
    }

    /// Every name the model uses, for a fresh one.
    fn names(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        out.extend(self.enums.iter().map(|e| e.0.clone()));
        out.extend(self.domains.iter().flat_map(|d| [Some(d.name.clone()), d.check.clone()].into_iter().flatten()));
        out.extend(self.composites.iter().flat_map(|c| std::iter::once(c.name.clone()).chain(c.attrs.iter().map(|a| a.0.clone()))));
        out.extend(self.funcs.iter().map(|f| f.name.clone()));
        out.extend(self.views.iter().map(|v| v.name.clone()));
        out.extend(self.triggers.iter().map(|v| v.name.clone()));
        out.extend(self.policies.iter().map(|v| v.name.clone()));
        for t in &self.tables {
            out.insert(t.name.clone());
            out.extend(t.cols.iter().map(|c| c.name.clone()));
            out.extend(t.indexes.iter().map(|i| i.0.clone()));
            out.extend(t.fn_indexes.iter().map(|i| i.0.clone()));
            out.extend(t.stats.iter().map(|i| i.0.clone()));
            out.extend(t.checks.iter().map(|c| c.name.clone()));
            out.extend(t.uniques.iter().map(|u| u.0.clone()));
            out.extend(t.fks.iter().map(|u| u.0.clone()));
            out.extend(t.part.iter().flat_map(|p| p.children.iter().map(|c| c.0.clone())));
        }
        out
    }

    fn fresh(&self, prefix: &str) -> String {
        let names: Vec<String> = self.names().into_iter().map(|n| n.to_lowercase()).collect();
        (0..)
            .map(|k| format!("{prefix}{k}"))
            .find(|n| !names.iter().any(|x| x.strip_prefix(n.to_lowercase().as_str()).is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_digit()))))
            .unwrap()
    }

    /// A value of the type for row `k` (1-based), one every type it may become reads too.
    fn value(&self, ty: &Ty, k: usize) -> String {
        match ty {
            Ty::SmallInt | Ty::Int | Ty::BigInt => format!("{k}"),
            Ty::Numeric => format!("{k}.5"),
            Ty::Text | Ty::Varchar(_) => format!("'{k}'"),
            Ty::Bool => (if k % 2 == 0 { "true" } else { "false" }).into(),
            Ty::Date => format!("'2024-01-0{}'", k.min(9)),
            Ty::Timestamp => format!("'2024-01-0{} 10:00:00'", k.min(9)),
            Ty::Jsonb => format!("'{{\"k\": {k}}}'"),
            Ty::Uuid => format!("'00000000-0000-0000-0000-00000000000{}'", k.min(9)),
            Ty::IntArray => format!("'{{{k},{}}}'", k + 1),
            Ty::TextArray => format!("'{{\"{k}\"}}'"),
            Ty::Enum(e) => lit(&self.labels(e).first().cloned().unwrap_or_default()),
            Ty::EnumArray(e) => format!("array[{}]::{}", lit(&self.labels(e).first().cloned().unwrap_or_default()), ty.sql()),
            Ty::Domain(d) => self.domain(d).map(|d| self.value(&d.base, k)).unwrap_or_else(|| "null".into()),
            Ty::Composite(c) => {
                let attrs = self.composite(c).map(|c| c.attrs.iter().map(|(_, t)| self.value(t, k)).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                format!("row({attrs})::{}", ty.sql())
            }
        }
    }

    /// A constant default of the type.
    fn default_of(&self, ty: &Ty) -> String {
        match ty {
            Ty::Jsonb | Ty::IntArray | Ty::TextArray => "'{}'".into(),
            Ty::Uuid => "'00000000-0000-0000-0000-000000000000'".into(),
            // An array literal of the first and the last label, each quoted as an array element.
            Ty::EnumArray(e) => {
                let labels = self.labels(e);
                let quote = |l: &str| format!("\"{}\"", l.replace('\\', "\\\\").replace('"', "\\\""));
                let picked: Vec<String> = labels.first().into_iter().chain(labels.last().filter(|_| labels.len() > 1)).map(|l| quote(l)).collect();
                format!("{}::{}", lit(&format!("{{{}}}", picked.join(","))), ty.sql())
            }
            Ty::Domain(d) => self.domain(d).map(|d| self.default_of(&d.base)).unwrap_or_else(|| "null".into()),
            other => self.value(other, 7),
        }
    }

    fn domain_check(&self, base: &Ty) -> String {
        if base.stringy() {
            "length(value) > 0".into()
        } else if base.integral() || *base == Ty::Numeric {
            "value > 0".into()
        } else if let Ty::Domain(d) = base {
            self.domain(d).map(|d| self.domain_check(&d.base)).unwrap_or_else(|| "true".into())
        } else if let Ty::Enum(e) = base {
            // Null passes, as a column added without a default holds it; the first label never moves.
            format!("value >= {}::{}", lit(&self.labels(e).first().cloned().unwrap_or_default()), q(e))
        } else {
            "true".into()
        }
    }

    /// Whether the type fills a column without a default of its own, or refuses null.
    fn domain_default(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::Domain(d) if self.domain(d).is_some_and(|d| d.default || self.domain_default(&d.base)))
    }
    fn domain_not_null(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::Domain(d) if self.domain(d).is_some_and(|d| d.not_null || self.domain_not_null(&d.base)))
    }

    /// A function's call, its first argument `first` where given, the rest constants.
    fn call(&self, f: &Func, first: Option<&str>) -> String {
        let args: Vec<String> = f
            .args
            .iter()
            .enumerate()
            .map(|(k, a)| match (k, first) {
                (0, Some(c)) => format!("({})::{}", q(c), a.sql()),
                _ => format!("({})::{}", self.value(a, 7), a.sql()),
            })
            .collect();
        format!("{}({})", q(&f.name), args.join(", "))
    }

    /// A column's default that calls a function, cast where the function returns another type.
    fn call_default(&self, f: &Func, ty: &Ty) -> String {
        let call = self.call(f, None);
        if f.ret == *ty { call } else { format!("({call})::{}", ty.sql()) }
    }

    fn func_sql(&self, f: &Func) -> Vec<String> {
        let args: Vec<String> = f.args.iter().enumerate().map(|(k, a)| format!("a{k} {}", a.sql())).collect();
        let expr = |k: usize| {
            let v = format!("({})::{}", self.value(&f.ret, k), f.ret.sql());
            if k % 2 == 1 && f.args.first() == Some(&f.ret) { format!("coalesce(a0, {v})") } else { v }
        };
        let body = match &f.body {
            Body::Quoted(k) => format!("immutable as $$ select {} $$", expr(*k)),
            Body::Atomic(k) => format!("immutable begin atomic select {}; end", expr(*k)),
            Body::Return(k) => format!("immutable return {}", expr(*k)),
            Body::Count(t, c) => format!("stable begin atomic select count({}) from {}; end", q(c), q(t)),
            Body::Min(t, c) => format!("stable begin atomic select min(({})::text) from {}; end", q(c), q(t)),
        };
        let mut out = vec![format!("create function {}({}) returns {} language sql {body};", q(&f.name), args.join(", "), f.ret.sql())];
        if f.revoked {
            out.push(format!("revoke all on function {} from public;", f.signature()));
        }
        if let Some(c) = &f.comment {
            out.push(format!("comment on function {} is {};", f.signature(), lit(c)));
        }
        out
    }

    fn check_sql(&self, ck: &Check) -> String {
        match &ck.expr {
            Cexpr::Positive => format!("{} > 0", q(&ck.col)),
            Cexpr::Length => format!("length({}) > 0", q(&ck.col)),
            Cexpr::Call(f) => {
                let Some(f) = self.func(f) else { return "true".into() };
                let call = self.call(f, (!f.args.is_empty()).then_some(ck.col.as_str()));
                format!("{call} = {call}")
            }
        }
    }

    fn reads_table(&self, f: &str) -> bool {
        self.func(f).is_some_and(|f| f.reads().is_some())
    }

    /// The model as Postgres SQL, each object after what it uses.
    fn sql(&self) -> String {
        let mut out = Vec::new();
        for (name, labels) in &self.enums {
            out.push(format!("create type {} as enum ({});", q(name), labels.iter().map(|l| lit(l)).collect::<Vec<_>>().join(", ")));
            if let Some(c) = &self.enum_comment {
                out.push(format!("comment on type {} is {};", q(name), lit(c)));
            }
        }
        for d in &self.domains {
            let mut s = format!("create domain {} as {}", q(&d.name), d.base.sql());
            if d.default {
                s.push_str(&format!(" default {}", self.default_of(&d.base)));
            }
            if d.not_null {
                s.push_str(" not null");
            }
            if let Some(c) = &d.check {
                s.push_str(&format!(" constraint {} check ({})", q(c), self.domain_check(&d.base)));
            }
            out.push(s + ";");
            if let Some(c) = &d.comment {
                out.push(format!("comment on domain {} is {};", q(&d.name), lit(c)));
            }
        }
        for c in &self.composites {
            out.push(format!("create type {} as ({});", q(&c.name), c.attrs.iter().map(|(n, t)| format!("{} {}", q(n), t.sql())).collect::<Vec<_>>().join(", ")));
            if let Some(x) = &c.comment {
                out.push(format!("comment on type {} is {};", q(&c.name), lit(x)));
            }
        }
        for f in self.funcs.iter().filter(|f| f.reads().is_none()) {
            out.extend(self.func_sql(f));
        }
        for t in &self.tables {
            for c in t.cols.iter().filter(|c| c.seq) {
                out.push(format!("create sequence {};", q(&format!("{}_{}_sq", t.name, c.name))));
            }
        }
        // What calls a function that reads a table comes after the function, which comes after the table.
        let mut later = Vec::new();
        for t in &self.tables {
            let mut lines: Vec<String> = Vec::new();
            for c in &t.cols {
                let mut s = format!("{} {}", q(&c.name), c.ty.sql());
                if c.identity {
                    s.push_str(" generated by default as identity");
                    if let Some(n) = c.start {
                        s.push_str(&format!(" (start with {n})"));
                    }
                } else if let Some(g) = &c.generated {
                    let g = match c.gen_call.as_ref().and_then(|(f, x)| self.func(f).map(|f| (f, x))) {
                        Some((f, x)) if f.ret == c.ty => self.call(f, Some(x)),
                        Some((f, x)) => format!("({})::{}", self.call(f, Some(x)), c.ty.sql()),
                        None => g.clone(),
                    };
                    s.push_str(&format!(" generated always as ({g}) stored"));
                } else if c.seq {
                    s.push_str(&format!(" default nextval({})", lit(&q(&format!("{}_{}_sq", t.name, c.name)))));
                } else if let Some(f) = c.call.as_ref().and_then(|f| self.func(f)) {
                    let d = self.call_default(f, &c.ty);
                    if f.reads().is_some() {
                        later.push(format!("alter table {} alter column {} set default {d};", q(&t.name), q(&c.name)));
                    } else {
                        s.push_str(&format!(" default {d}"));
                    }
                } else if let Some(d) = &c.default {
                    s.push_str(&format!(" default {d}"));
                }
                if c.not_null {
                    s.push_str(" not null");
                }
                lines.push(s);
            }
            if !t.pk.is_empty() {
                lines.push(format!("constraint {} primary key ({})", q(&format!("{}_pkey", t.name)), t.pk.iter().map(|c| q(c)).collect::<Vec<_>>().join(", ")));
            }
            for (n, cols) in &t.uniques {
                lines.push(format!("constraint {} unique ({})", q(n), cols.iter().map(|c| q(c)).collect::<Vec<_>>().join(", ")));
            }
            for ck in &t.checks {
                let s = format!("constraint {} check ({})", q(&ck.name), self.check_sql(ck));
                if matches!(&ck.expr, Cexpr::Call(f) if self.reads_table(f)) {
                    later.push(format!("alter table {} add {s};", q(&t.name)));
                } else {
                    lines.push(s);
                }
            }
            let by = match &t.part {
                Some(p) => format!(" partition by {} (id)", if p.list { "list" } else { "range" }),
                None => String::new(),
            };
            out.push(format!("create table {} (\n  {}\n){by};", q(&t.name), lines.join(",\n  ")));
            if let Some(p) = &t.part {
                for (child, bound) in &p.children {
                    let b = match bound {
                        None => "default".into(),
                        Some((lo, hi)) if p.list => format!("for values in ({})", (*lo..*hi).map(|v| v.to_string()).collect::<Vec<_>>().join(", ")),
                        Some((lo, hi)) => format!("for values from ({lo}) to ({hi})"),
                    };
                    out.push(format!("create table {} partition of {} {b};", q(child), q(&t.name)));
                }
            }
        }
        for t in &self.tables {
            for c in t.cols.iter().filter(|c| c.seq) {
                out.push(format!("alter sequence {} owned by {}.{};", q(&format!("{}_{}_sq", t.name, c.name)), q(&t.name), q(&c.name)));
            }
            for (n, col, rt, rc) in &t.fks {
                out.push(format!("alter table {} add constraint {} foreign key ({}) references {} ({});", q(&t.name), q(n), q(col), q(rt), q(rc)));
            }
            for (n, cols) in &t.stats {
                out.push(format!("create statistics {} on {} from {};", q(n), cols.iter().map(|c| q(c)).collect::<Vec<_>>().join(", "), q(&t.name)));
            }
            for (n, f, c) in &t.fn_indexes {
                if let Some(f) = self.func(f) {
                    out.push(format!("create index {} on {} (({}));", q(n), q(&t.name), self.call(f, Some(c))));
                }
            }
            for (n, cols, wh) in &t.indexes {
                out.push(format!("create index {} on {} ({}){};", q(n), q(&t.name), cols.iter().map(|c| q(c)).collect::<Vec<_>>().join(", "), wh.as_ref().map(|w| format!(" where {} is not null", q(w))).unwrap_or_default()));
            }
        }
        for f in self.funcs.iter().filter(|f| f.reads().is_some()) {
            out.extend(self.func_sql(f));
        }
        out.extend(later);
        for v in &self.views {
            let mut cols: Vec<String> = v.cols.iter().map(|c| q(c)).collect();
            if let Some(f) = v.call.as_ref().and_then(|(f, c)| self.func(f).map(|f| self.call(f, c.as_deref()))) {
                cols.push(format!("{f} as fx"));
            }
            let kind = if v.materialized { "materialized view" } else { "view" };
            let with = if v.invoker && !v.materialized { " with (security_invoker = true)" } else { "" };
            out.push(format!("create {kind} {}{with} as select {} from {};", q(&v.name), cols.join(", "), q(v.over.as_ref().unwrap_or(&v.table))));
        }
        for tg in &self.triggers {
            let body = ["", "perform 1;", "new := new;"][tg.variant % 3];
            out.push(format!("create function {}() returns trigger language plpgsql as $$ begin {body} return new; end $$;", q(&format!("{}_fn", tg.name))));
            out.push(format!(
                "create trigger {} before insert or update{} on {} for each row{} execute function {}();",
                q(&tg.name),
                tg.of.as_ref().map(|c| format!(" of {}", q(c))).unwrap_or_default(),
                q(&tg.table),
                tg.when.as_ref().map(|c| format!(" when (new.{} is not null)", q(c))).unwrap_or_default(),
                q(&format!("{}_fn", tg.name))
            ));
        }
        for t in self.tables.iter().filter(|t| t.rls) {
            out.push(format!("alter table {} enable row level security;", q(&t.name)));
        }
        for p in &self.policies {
            let e = match &p.expr {
                Pexpr::True => "true".into(),
                Pexpr::NotNull(c) => format!("{} is not null", q(c)),
                Pexpr::Call(f, c) => match self.func(f) {
                    Some(f) => {
                        let call = self.call(f, c.as_deref());
                        format!("{call} = {call}")
                    }
                    None => "true".into(),
                },
            };
            out.push(format!("create policy {} on {} using ({e});", q(&p.name), q(&p.table)));
        }
        for t in &self.tables {
            if t.granted {
                out.push(format!("grant select on table {} to public;", q(&t.name)));
            }
            if let Some(c) = &t.comment {
                out.push(format!("comment on table {} is {};", q(&t.name), lit(c)));
            }
            for c in &t.cols {
                if let Some(x) = &c.comment {
                    out.push(format!("comment on column {}.{} is {};", q(&t.name), q(&c.name), lit(x)));
                }
            }
        }
        out.join("\n") + "\n"
    }

    /// Rows for the tables, three each, in an order their keys allow.
    fn seed(&self) -> String {
        let mut out = Vec::new();
        for t in &self.tables {
            let cols: Vec<&Col> = t.cols.iter().filter(|c| c.generated.is_none() && !c.identity).collect();
            if cols.is_empty() {
                continue;
            }
            for k in 1..=3 {
                let values: Vec<String> = cols.iter().map(|c| self.value(&c.ty, k)).collect();
                out.push(format!("insert into {} ({}) values ({});", q(&t.name), cols.iter().map(|c| q(&c.name)).collect::<Vec<_>>().join(", "), values.join(", ")));
            }
        }
        out.join("\n") + "\n"
    }

    /// Whether the rows `seed` writes can be held by `to`, the model this one is changed to: a unique
    /// key over values the rows repeat, or a column added to a table with rows that refuses null and
    /// has no value to fill it with, is a target no plan reaches with those rows, and the server's
    /// refusal says so.
    fn fits(&self, to: &Model) -> bool {
        for t in &to.tables {
            let Some(old) = self.table(&t.name) else { continue };
            for c in &t.cols {
                let added = !old.cols.iter().any(|x| x.name == c.name);
                let filled = c.default.is_some() || c.call.is_some() || c.seq || c.generated.is_some() || c.identity || to.domain_default(&c.ty);
                if added && (c.not_null || to.domain_not_null(&c.ty)) && !filled {
                    return false;
                }
            }
            for name in t.uniques.iter().flat_map(|(_, cols)| cols) {
                let (now, was) = (t.cols.iter().find(|x| &x.name == name), old.cols.iter().find(|x| &x.name == name));
                let repeats = match (was, now) {
                    (Some(w), Some(n)) => matches!(w.ty, Ty::Bool | Ty::Enum(_)) || n.ty == Ty::Bool,
                    (None, Some(n)) => n.default.is_some() || n.call.is_some(),
                    _ => false,
                };
                if repeats {
                    return false;
                }
            }
        }
        true
    }

    /// Whether a column is read by anything else of the model.
    fn used(&self, table: &str, col: &str) -> bool {
        let Some(t) = self.table(table) else { return false };
        t.pk.iter().any(|c| c == col)
            || t.uniques.iter().any(|(_, cs)| cs.iter().any(|c| c == col))
            || t.checks.iter().any(|c| c.col == col)
            || t.indexes.iter().any(|(_, cs, wh)| cs.iter().any(|c| c == col) || wh.as_deref() == Some(col))
            || t.fks.iter().any(|(_, c, _, _)| c == col)
            || multiplied(t, col)
            || t.cols.iter().any(|c| c.gen_call.as_ref().is_some_and(|(_, x)| x == col))
            || t.fn_indexes.iter().any(|i| i.2 == col)
            || t.stats.iter().any(|(_, cs)| cs.iter().any(|c| c == col))
            || self.tables.iter().any(|o| o.fks.iter().any(|(_, _, rt, rc)| rt == table && rc == col))
            || self.views.iter().any(|v| v.table == table && (v.cols.iter().any(|c| c == col) || v.call.as_ref().is_some_and(|(_, c)| c.as_deref() == Some(col))))
            || self.policies.iter().any(|p| p.table == table && matches!(&p.expr, Pexpr::NotNull(c) | Pexpr::Call(_, Some(c)) if c == col))
            || self.triggers.iter().any(|tg| tg.table == table && (tg.of.as_deref() == Some(col) || tg.when.as_deref() == Some(col)))
            || self.funcs.iter().any(|f| f.reads() == Some((table, col)))
    }

    /// Whether a type is used by anything of the model.
    fn type_used(&self, name: &str) -> bool {
        let is = |t: &Ty| t.names() == Some(name);
        self.tables.iter().any(|t| t.cols.iter().any(|c| is(&c.ty)))
            || self.funcs.iter().any(|f| is(&f.ret) || f.args.iter().any(is))
            || self.composites.iter().any(|c| c.attrs.iter().any(|(_, t)| is(t)))
            || self.domains.iter().any(|d| is(&d.base))
    }

    /// What calls a function made to fit it again, after its signature changed: a call it no longer
    /// fits goes.
    fn fix_calls(&mut self) {
        let funcs = self.funcs.clone();
        let snapshot = self.clone();
        let fits_default = |f: &str, ty: &Ty| funcs.iter().find(|x| x.name == f).is_some_and(|f| call_fits(f, ty));
        let fits_arg = |f: &str, ty: Option<Ty>| funcs.iter().find(|x| x.name == f).is_some_and(|f| match (f.args.first(), ty) {
            (None, _) => true,
            (Some(a), Some(t)) => arg_fits(a, &t),
            (Some(_), None) => false,
        });
        for t in &mut self.tables {
            for c in &mut t.cols {
                if let Some(f) = c.call.clone() {
                    if !fits_default(&f, &c.ty) {
                        c.call = None;
                        c.default = Some(snapshot.default_of(&c.ty));
                    }
                }
            }
            let cols = t.cols.clone();
            let ty_of = |c: &str| cols.iter().find(|x| x.name == c).map(|x| x.ty.clone());
            t.checks.retain(|ck| match &ck.expr {
                Cexpr::Call(f) => fits_arg(f, ty_of(&ck.col)),
                _ => true,
            });
            t.fn_indexes.retain(|(_, f, c)| funcs.iter().find(|x| &x.name == f).is_some_and(|f| f.args.first().is_some_and(|a| ty_of(c).is_some_and(|t| arg_fits(a, &t)))));
        }
        let tys: Vec<(String, Vec<Col>)> = self.tables.iter().map(|t| (t.name.clone(), t.cols.clone())).collect();
        let ty_in = |t: &str, c: &str| tys.iter().find(|x| x.0 == t).and_then(|x| x.1.iter().find(|y| y.name == c)).map(|y| y.ty.clone());
        for v in &mut self.views {
            if let Some((f, c)) = v.call.clone() {
                let ok = match &c {
                    Some(c) => fits_arg(&f, ty_in(&v.table, c)),
                    None => funcs.iter().any(|x| x.name == f),
                };
                if !ok {
                    v.call = None;
                }
            }
        }
        for p in &mut self.policies {
            if let Pexpr::Call(f, c) = p.expr.clone() {
                let ok = match &c {
                    Some(c) => fits_arg(&f, ty_in(&p.table, c)),
                    None => funcs.iter().any(|x| x.name == f),
                };
                if !ok {
                    p.expr = Pexpr::True;
                }
            }
        }
    }

    /// The model without a function, and without what calls it.
    fn drop_func(&mut self, name: &str) {
        self.funcs.retain(|f| f.name != name);
        for t in &mut self.tables {
            for c in &mut t.cols {
                if c.call.as_deref() == Some(name) {
                    c.call = None;
                }
            }
            t.checks.retain(|ck| ck.expr != Cexpr::Call(name.into()));
            t.fn_indexes.retain(|i| i.1 != name);
            t.cols.retain(|c| !c.gen_call.as_ref().is_some_and(|(f, _)| f == name));
        }
        for v in &mut self.views {
            if v.call.as_ref().is_some_and(|(f, _)| f == name) {
                v.call = None;
            }
        }
        for p in &mut self.policies {
            if matches!(&p.expr, Pexpr::Call(f, _) if f == name) {
                p.expr = Pexpr::True;
            }
        }
    }

    /// The model without a table, and without what reads it.
    fn drop_table(&mut self, name: &str) {
        self.tables.retain(|t| t.name != name);
        self.views.retain(|v| v.table != name);
        self.triggers.retain(|v| v.table != name);
        self.policies.retain(|v| v.table != name);
        for t in &mut self.tables {
            t.fks.retain(|(_, _, rt, _)| rt != name);
        }
        let readers: Vec<String> = self.funcs.iter().filter(|f| f.reads().is_some_and(|(t, _)| t == name)).map(|f| f.name.clone()).collect();
        for f in readers {
            self.drop_func(&f);
        }
    }
}

/// Whether a generated column calls the function, which then keeps its arguments.
fn gen_uses(m: &Model, f: &str) -> bool {
    m.tables.iter().any(|t| t.cols.iter().any(|c| c.gen_call.as_ref().is_some_and(|(x, _)| x == f)))
}

/// The type a generated column's function takes the column as, where one does.
fn gen_arg(m: &Model, t: &Table, col: &str) -> Option<Ty> {
    t.cols.iter().filter_map(|c| c.gen_call.as_ref()).filter(|(_, x)| x == col).find_map(|(f, _)| m.func(f).and_then(|f| f.args.first().cloned()))
}

/// Whether a generated column of the table multiplies the column.
fn multiplied(t: &Table, col: &str) -> bool {
    t.cols.iter().any(|c| c.generated.as_ref().is_some_and(|g| g.split(" * ").next() == Some(q(col).as_str())))
}

/// Whether a function's result fills a column of type `ty` through a cast that holds its value.
fn call_fits(f: &Func, ty: &Ty) -> bool {
    f.ret == *ty || (f.ret.family() && ty.family() && !matches!(f.body, Body::Min(..)))
}

/// Whether a column of type `ty` passes, cast, as an argument of type `arg`.
fn arg_fits(arg: &Ty, ty: &Ty) -> bool {
    arg == ty || (arg.family() && ty.family())
}

const TYPES: &[Ty] = &[Ty::SmallInt, Ty::Int, Ty::BigInt, Ty::Numeric, Ty::Text, Ty::Varchar(20), Ty::Bool, Ty::Date, Ty::Timestamp, Ty::Jsonb, Ty::Uuid, Ty::IntArray, Ty::TextArray];

/// Results a function's may change between, each value of one cast to another.
const RESULTS: &[Ty] = &[Ty::Int, Ty::BigInt, Ty::Numeric, Ty::Text];

/// Types a column of type `ty` holding the fuzzer's values may become.
fn becomes(m: &Model, ty: &Ty) -> Vec<Ty> {
    let mut out = match ty {
        Ty::SmallInt => vec![Ty::Int, Ty::BigInt, Ty::Numeric, Ty::Text, Ty::Varchar(10), Ty::IntArray],
        Ty::Int => vec![Ty::SmallInt, Ty::BigInt, Ty::Numeric, Ty::Text, Ty::Varchar(10), Ty::IntArray, Ty::Bool],
        Ty::BigInt => vec![Ty::Int, Ty::Numeric, Ty::Text],
        Ty::Numeric => vec![Ty::Int, Ty::Text],
        Ty::Text => vec![Ty::Varchar(20), Ty::Int, Ty::BigInt, Ty::Numeric, Ty::TextArray],
        Ty::Varchar(_) => vec![Ty::Text, Ty::Varchar(40), Ty::Int],
        Ty::Bool => vec![Ty::Text],
        Ty::Date => vec![Ty::Timestamp, Ty::Text],
        Ty::Timestamp => vec![Ty::Date, Ty::Text],
        Ty::Jsonb => vec![Ty::Text],
        Ty::Uuid => vec![Ty::Text],
        Ty::IntArray => vec![Ty::TextArray, Ty::Int],
        Ty::TextArray => vec![Ty::Text],
        Ty::Enum(e) => vec![Ty::Text, Ty::EnumArray(e.clone())],
        Ty::EnumArray(_) => vec![Ty::TextArray],
        Ty::Domain(d) => m.domain(d).map(|d| vec![d.base.clone(), Ty::Text]).unwrap_or_default(),
        Ty::Composite(_) => vec![Ty::Text],
    };
    for d in &m.domains {
        if d.base == *ty {
            out.push(Ty::Domain(d.name.clone()));
        }
    }
    out
}

/// Types a function's arguments and results take.
fn signature_types(m: &Model) -> Vec<Ty> {
    let mut out = vec![Ty::Int, Ty::BigInt, Ty::Numeric, Ty::Text, Ty::SmallInt];
    out.extend(m.enums.iter().map(|e| Ty::Enum(e.0.clone())));
    out.extend(m.domains.iter().map(|d| Ty::Domain(d.name.clone())));
    out.extend(m.composites.iter().map(|d| Ty::Composite(d.name.clone())));
    out
}

/// A type for a new column: a plain one, or most often one the model defines where it has any.
fn column_type(m: &Model, rng: &mut Rng) -> Ty {
    let defined: Vec<Ty> = m
        .enums
        .iter()
        .flat_map(|e| [Ty::Enum(e.0.clone()), Ty::EnumArray(e.0.clone())])
        .chain(m.domains.iter().map(|d| Ty::Domain(d.name.clone())))
        .chain(m.composites.iter().map(|d| Ty::Composite(d.name.clone())))
        .collect();
    if !defined.is_empty() && rng.chance(35) { rng.pick(&defined).unwrap().clone() } else { rng.pick(TYPES).unwrap().clone() }
}

fn generate(rng: &mut Rng) -> Model {
    let mut m = Model::default();
    if rng.chance(50) {
        let labels: Vec<&str> = match rng.below(3) {
            0 => vec!["a", "b", "c"],
            1 => vec!["a", "b,c", "it's", "x y"],
            _ => vec!["a", "b|c", "q\"r", "d.e"],
        };
        let name = styled(rng, "mood".into());
        m.enums.push((name, labels.into_iter().map(String::from).collect()));
    }
    for t in 0..(1 + rng.below(3)) {
        let name = styled(rng, format!("t{t}"));
        let partitioned = rng.chance(15);
        let mut id = col("id", Ty::Int);
        id.not_null = true;
        id.identity = !partitioned && rng.chance(20);
        let mut cols = vec![id];
        for c in 0..(1 + rng.below(4)) {
            let ty = if !m.enums.is_empty() && rng.chance(15) { Ty::Enum(m.enums[0].0.clone()) } else { rng.pick(TYPES).unwrap().clone() };
            let mut x = col(&styled(rng, format!("c{c}")), ty.clone());
            x.default = rng.chance(30).then(|| m.default_of(&ty));
            x.not_null = rng.chance(30);
            cols.push(x);
        }
        let part = partitioned.then(|| {
            let mut children = if rng.chance(50) { vec![(format!("{name}_p0"), Some((1, 4)))] } else { vec![(format!("{name}_p0"), Some((1, 3))), (format!("{name}_p1"), Some((3, 4)))] };
            if rng.chance(30) {
                children.push((format!("{name}_pd"), None));
            }
            Part { list: rng.chance(50), children }
        });
        let mut table = Table { name, cols, pk: vec!["id".into()], uniques: vec![], checks: vec![], indexes: vec![], fn_indexes: vec![], stats: vec![], fks: vec![], part, rls: false, granted: false, comment: None };
        if t > 0 && rng.chance(40) {
            table.cols.push(col("parent", Ty::Int));
            let t0 = m.tables[0].name.clone();
            table.fks.push((format!("{}_parent_fkey", table.name), "parent".into(), t0, "id".into()));
        }
        m.tables.push(table);
    }
    // The first schema has some of what the changes interact with already.
    for _ in 0..rng.below(5) {
        let k = *rng.pick(ADDERS).unwrap();
        mutation(&mut m, rng, k);
    }
    m
}

/// Mutations that add something, which make the first schema too.
const ADDERS: &[usize] = &[20, 20, 14, 14, 15, 22, 22, 23, 24, 25, 26, 27, 0, 6, 8, 11, 19, 30];

/// The mutations, weighted toward functions, domains, composite types and the enum, and what reads them.
const KINDS: &[usize] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 10, 10, 11, 12, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 20, 20, 21, 21, 22, 22, 23, 23, 24, 25, 26, 27, 28, 29, 30, 2, 3];

/// One random change to the model, kept consistent: what reads a column dropped goes with it.
fn mutate(m: &mut Model, rng: &mut Rng) -> String {
    for _ in 0..8 {
        let k = *rng.pick(KINDS).unwrap();
        let done = mutation(m, rng, k);
        if done != "nothing" {
            return done;
        }
    }
    "nothing".into()
}

fn mutation(m: &mut Model, rng: &mut Rng, kind: usize) -> String {
    let ti = rng.below(m.tables.len());
    let table = m.tables[ti].name.clone();
    match kind {
        0 => {
            let ty = column_type(m, rng);
            let not_null = rng.chance(40);
            let default = (not_null || rng.chance(30)).then(|| m.default_of(&ty));
            let name = styled(rng, m.fresh("n"));
            m.tables[ti].cols.push(Col { not_null, default, ..col(&name, ty) });
            format!("add column {name}")
        }
        1 => {
            let free: Vec<String> = m.tables[ti].cols.iter().filter(|c| c.name != "id" && !m.used(&table, &c.name)).map(|c| c.name.clone()).collect();
            let Some(c) = rng.pick(&free).cloned() else { return "nothing".into() };
            m.tables[ti].cols.retain(|x| x.name != c);
            format!("drop column {table}.{c}")
        }
        2 | 3 => {
            let t = &m.tables[ti];
            let free: Vec<usize> = (0..t.cols.len()).filter(|k| t.cols[*k].name != "id" && t.cols[*k].generated.is_none() && !t.cols[*k].identity && !t.fks.iter().any(|(_, c, ..)| *c == t.cols[*k].name)).collect();
            let Some(&k) = rng.pick(&free) else { return "nothing".into() };
            let name = t.cols[k].name.clone();
            let mut to = becomes(m, &t.cols[k].ty);
            // A column a generated column multiplies stays a number.
            if multiplied(t, &name) {
                to.retain(|t| t.integral());
            }
            if let Some(a) = gen_arg(m, t, &name) {
                to.retain(|t| arg_fits(&a, t));
            }
            let Some(ty) = rng.pick(&to).cloned() else { return "nothing".into() };
            let default = m.default_of(&ty);
            let c = &mut m.tables[ti].cols[k];
            c.ty = ty;
            if c.default.is_some() || c.call.is_some() {
                c.default = Some(default);
            }
            c.call = None;
            c.seq = false;
            // What reads it by its old type goes.
            let t = &mut m.tables[ti];
            t.checks.retain(|ck| ck.col != name);
            t.indexes.retain(|(_, cols, wh)| (!cols.contains(&name) || wh.is_none()) && wh.as_deref() != Some(&name));
            t.fn_indexes.retain(|i| i.2 != name);
            m.fix_calls();
            format!("retype {table}.{name}")
        }
        4 => {
            let k = rng.below(m.tables[ti].cols.len());
            let c = &mut m.tables[ti].cols[k];
            if c.generated.is_some() || c.identity || c.name == "id" {
                return "nothing".into();
            }
            c.not_null = !c.not_null;
            format!("not null {}", c.name)
        }
        5 => {
            let k = rng.below(m.tables[ti].cols.len());
            let ty = m.tables[ti].cols[k].ty.clone();
            let default = m.default_of(&ty);
            let c = &mut m.tables[ti].cols[k];
            if c.generated.is_some() || c.identity || c.seq {
                return "nothing".into();
            }
            c.default = if (c.default.is_some() || c.call.is_some()) && rng.chance(50) { None } else { Some(default) };
            c.call = None;
            format!("default {}", c.name)
        }
        6 => {
            let t = &m.tables[ti];
            let Some(c) = rng.pick(&t.cols).cloned() else { return "nothing".into() };
            let name = styled(rng, m.fresh("i"));
            let wh = rng.chance(30).then(|| c.name.clone());
            m.tables[ti].indexes.push((name.clone(), vec![c.name.clone()], wh));
            format!("index {name}")
        }
        7 => {
            if !m.tables[ti].fn_indexes.is_empty() && rng.chance(50) {
                let k = rng.below(m.tables[ti].fn_indexes.len());
                m.tables[ti].fn_indexes.remove(k);
                return "drop function index".into();
            }
            if m.tables[ti].indexes.is_empty() {
                return "nothing".into();
            }
            let k = rng.below(m.tables[ti].indexes.len());
            m.tables[ti].indexes.remove(k);
            "drop index".into()
        }
        8 => {
            let ints: Vec<String> = m.tables[ti].cols.iter().filter(|c| c.ty.integral() && c.name != "id").map(|c| c.name.clone()).collect();
            let texts: Vec<String> = m.tables[ti].cols.iter().filter(|c| c.ty.stringy()).map(|c| c.name.clone()).collect();
            let name = styled(rng, m.fresh("ck"));
            if let Some(c) = rng.pick(&ints).cloned() {
                m.tables[ti].checks.push(Check { name: name.clone(), col: c, expr: Cexpr::Positive });
            } else if let Some(c) = rng.pick(&texts).cloned() {
                m.tables[ti].checks.push(Check { name: name.clone(), col: c, expr: Cexpr::Length });
            } else {
                return "nothing".into();
            }
            format!("check {name}")
        }
        9 => {
            let ints: Vec<String> = m.tables[ti].cols.iter().filter(|c| c.ty.integral() && c.name != "id" && c.generated.is_none()).map(|c| c.name.clone()).collect();
            let Some(c) = rng.pick(&ints).cloned() else { return "nothing".into() };
            let name = styled(rng, m.fresh("g"));
            let ty = m.tables[ti].cols.iter().find(|x| x.name == c).map(|x| x.ty.clone()).unwrap_or(Ty::Int);
            m.tables[ti].cols.push(Col { generated: Some(format!("{} * 2", q(&c))), ..col(&name, if ty == Ty::SmallInt { Ty::Int } else { ty }) });
            format!("generated {name}")
        }
        10 => {
            let Some(e) = m.enums.first_mut() else { return "nothing".into() };
            let what = match rng.below(4) {
                0 => {
                    let at = 1 + rng.below(e.1.len());
                    let label = (0..).map(|k| if k % 2 == 0 { format!("x{k}") } else { format!("y,{k}'") }).find(|l| !e.1.contains(l)).unwrap();
                    e.1.insert(at.min(e.1.len()), label);
                    "label added"
                }
                1 if e.1.len() > 2 => {
                    let k = 1 + rng.below(e.1.len() - 1);
                    e.1.remove(k);
                    "label removed"
                }
                2 if e.1.len() > 2 => {
                    let k = 1 + rng.below(e.1.len() - 1);
                    let last = e.1.len() - 1;
                    e.1.swap(k, last);
                    "labels reordered"
                }
                _ if e.1.len() > 1 => {
                    let k = 1 + rng.below(e.1.len() - 1);
                    let label = (0..).map(|n| format!("r{n} '{k}")).find(|l| !e.1.contains(l)).unwrap();
                    e.1[k] = label;
                    "label renamed"
                }
                _ => return "nothing".into(),
            };
            // An array default names the last label, which may have changed.
            let snapshot = m.clone();
            for t in &mut m.tables {
                for c in &mut t.cols {
                    if matches!(c.ty, Ty::EnumArray(_)) && c.default.is_some() {
                        c.default = Some(snapshot.default_of(&c.ty));
                    }
                }
            }
            what.into()
        }
        11 => {
            let cols: Vec<String> = m.tables[ti].cols.iter().take(1 + rng.below(3)).map(|c| c.name.clone()).collect();
            let name = styled(rng, m.fresh("v"));
            // Over a view of the table, most often its own.
            let inner: Vec<View> = m.views.iter().filter(|v| v.table == table && v.over.is_none()).cloned().collect();
            let (over, cols) = match rng.pick(&inner).filter(|_| rng.chance(40)) {
                Some(v) => (Some(v.name.clone()), v.cols.iter().take(1 + rng.below(2)).cloned().collect()),
                None => (None, cols),
            };
            m.views.push(View { name: name.clone(), table, cols, call: None, over, materialized: rng.chance(20), invoker: rng.chance(20) });
            format!("view {name}")
        }
        12 => {
            if m.views.is_empty() {
                return "nothing".into();
            }
            if !m.views.is_empty() && rng.chance(30) {
                let k = rng.below(m.views.len());
                m.views[k].invoker = !m.views[k].invoker;
                return "view security_invoker".into();
            }
            let open: Vec<usize> = (0..m.views.len()).filter(|k| !m.views.iter().any(|v| v.over.as_ref() == Some(&m.views[*k].name))).collect();
            let Some(&k) = rng.pick(&open) else { return "nothing".into() };
            m.views.remove(k);
            "drop view".into()
        }
        13 => {
            if m.tables[ti].part.is_some() {
                return "nothing".into();
            }
            // A column a function generates may hold one value in every row.
            let texts: Vec<String> = m.tables[ti].cols.iter().filter(|c| (c.ty.stringy() || c.ty.integral()) && c.gen_call.is_none()).filter(|c| c.name != "id").map(|c| c.name.clone()).collect();
            let Some(c) = rng.pick(&texts).cloned() else { return "nothing".into() };
            let name = styled(rng, m.fresh("u"));
            m.tables[ti].uniques.push((name.clone(), vec![c]));
            format!("unique {name}")
        }
        // A function of constants, and something that calls it.
        14 => {
            let pool = signature_types(m);
            let args: Vec<Ty> = (0..rng.below(3)).map(|_| rng.pick(&pool).unwrap().clone()).collect();
            let ret = if rng.chance(60) { rng.pick(RESULTS).unwrap().clone() } else { rng.pick(&pool).unwrap().clone() };
            let k = 7 + rng.below(2);
            let body = match rng.below(3) {
                0 => Body::Quoted(k),
                1 => Body::Atomic(k),
                _ => Body::Return(k),
            };
            let name = styled(rng, m.fresh("f"));
            m.funcs.push(Func { name: name.clone(), args, ret, body, revoked: rng.chance(20), comment: None });
            let used = use_func(m, rng, &name);
            format!("function {name} ({used})")
        }
        // A function that reads a table's column, and something that calls it.
        15 => {
            let Some(c) = rng.pick(&m.tables[ti].cols).map(|c| c.name.clone()) else { return "nothing".into() };
            let name = styled(rng, m.fresh("r"));
            let body = if rng.chance(50) { Body::Count(table.clone(), c) } else { Body::Min(table.clone(), c) };
            let ret = if matches!(body, Body::Count(..)) { Ty::BigInt } else { Ty::Text };
            m.funcs.push(Func { name: name.clone(), args: vec![], ret, body, revoked: false, comment: None });
            let used = use_func(m, rng, &name);
            format!("reader {name} ({used})")
        }
        // A function's body changes; a reader's from count to min changes its result too.
        16 => {
            let Some(k) = (!m.funcs.is_empty()).then(|| rng.below(m.funcs.len())) else { return "nothing".into() };
            let style = rng.below(3);
            let f = &mut m.funcs[k];
            f.body = match f.body.clone() {
                Body::Count(t, c) => {
                    f.ret = Ty::Text;
                    Body::Min(t, c)
                }
                Body::Min(t, c) => {
                    f.ret = Ty::BigInt;
                    Body::Count(t, c)
                }
                Body::Quoted(k) | Body::Atomic(k) | Body::Return(k) => match style {
                    0 => Body::Quoted(k + 1),
                    1 => Body::Atomic(k),
                    _ => Body::Return(k),
                },
            };
            let name = f.name.clone();
            m.fix_calls();
            format!("body of {name}")
        }
        // A function's result type changes.
        17 => {
            let open: Vec<usize> = (0..m.funcs.len()).filter(|k| m.funcs[*k].reads().is_none() && RESULTS.contains(&m.funcs[*k].ret)).collect();
            let Some(&k) = rng.pick(&open) else { return "nothing".into() };
            let ret = rng.pick(RESULTS).unwrap().clone();
            m.funcs[k].ret = ret;
            let name = m.funcs[k].name.clone();
            m.fix_calls();
            format!("result of {name}")
        }
        // A function's arguments change: one's type, or one more or fewer.
        18 => {
            let open: Vec<usize> = (0..m.funcs.len()).filter(|k| m.funcs[*k].reads().is_none() && !gen_uses(m, &m.funcs[*k].name)).collect();
            let Some(&k) = rng.pick(&open) else { return "nothing".into() };
            let pool = signature_types(m);
            let f = &mut m.funcs[k];
            match rng.below(3) {
                0 if !f.args.is_empty() => {
                    let i = rng.below(f.args.len());
                    f.args[i] = rng.pick(&pool).unwrap().clone();
                }
                1 if !f.args.is_empty() => {
                    f.args.pop();
                }
                _ => f.args.push(rng.pick(&pool).unwrap().clone()),
            }
            let name = f.name.clone();
            m.fix_calls();
            format!("arguments of {name}")
        }
        19 => {
            let Some(name) = rng.pick(&m.funcs).map(|f| f.name.clone()) else { return "nothing".into() };
            if rng.chance(50) && !gen_uses(m, &name) {
                m.drop_func(&name);
                format!("drop function {name}")
            } else {
                let used = use_func(m, rng, &name);
                format!("call {name} ({used})")
            }
        }
        // A domain, and a column of it.
        20 => {
            let mut bases = vec![Ty::Int, Ty::BigInt, Ty::Numeric, Ty::Text];
            bases.extend(m.enums.iter().map(|e| Ty::Enum(e.0.clone())));
            bases.extend(m.domains.iter().filter(|d| !matches!(d.base, Ty::Domain(_))).map(|d| Ty::Domain(d.name.clone())));
            let base = rng.pick(&bases).unwrap().clone();
            let name = styled(rng, m.fresh("d"));
            let check = rng.chance(50).then(|| styled(rng, format!("{name}_ck")));
            m.domains.push(Domain { name: name.clone(), base: base.clone(), default: rng.chance(40), check, not_null: rng.chance(20), comment: None });
            let ty = Ty::Domain(name.clone());
            // A column of the base made the domain, or a new column of it.
            let t = &m.tables[ti];
            let same: Vec<usize> = (0..t.cols.len()).filter(|k| t.cols[*k].ty == base && t.cols[*k].name != "id" && t.cols[*k].generated.is_none() && !t.cols[*k].identity && !t.fks.iter().any(|(_, c, ..)| *c == t.cols[*k].name) && !multiplied(t, &t.cols[*k].name)).collect();
            if let (Some(&k), true) = (rng.pick(&same), rng.chance(60)) {
                m.tables[ti].cols[k].ty = ty;
                m.tables[ti].cols[k].call = None;
                m.fix_calls();
            } else {
                let cname = styled(rng, m.fresh("n"));
                let default = rng.chance(50).then(|| m.default_of(&ty));
                m.tables[ti].cols.push(Col { default, ..col(&cname, ty) });
            }
            format!("domain {name}")
        }
        // A domain changes: its default, its check, whether it refuses null, or its base widens.
        21 => {
            let Some(k) = (!m.domains.is_empty()).then(|| rng.below(m.domains.len())) else { return "nothing".into() };
            let fresh = m.fresh("dk");
            let ty = Ty::Domain(m.domains[k].name.clone());
            let under_generated = m.tables.iter().any(|t| t.cols.iter().any(|c| c.ty == ty && (multiplied(t, &c.name) || gen_arg(m, t, &c.name).is_some())));
            let d = &mut m.domains[k];
            let what = match rng.below(5) {
                0 => {
                    d.default = !d.default;
                    "default"
                }
                1 => {
                    d.check = if d.check.is_some() { None } else { Some(fresh) };
                    "check"
                }
                2 if d.check.is_some() => {
                    d.check = Some(fresh);
                    "check renamed"
                }
                3 => {
                    d.not_null = !d.not_null;
                    "not null"
                }
                _ if under_generated => return "nothing".into(),
                _ => {
                    d.base = match d.base {
                        Ty::Int => Ty::BigInt,
                        Ty::BigInt => Ty::Numeric,
                        Ty::Numeric => Ty::Text,
                        _ => return "nothing".into(),
                    };
                    "base"
                }
            };
            let name = d.name.clone();
            // A check that compares the old base's way goes.
            if what == "base" {
                for t in &mut m.tables {
                    let cols = t.cols.clone();
                    t.checks.retain(|ck| matches!(ck.expr, Cexpr::Call(_)) || cols.iter().find(|c| c.name == ck.col).is_none_or(|c| c.ty != ty));
                }
            }
            m.fix_calls();
            format!("domain {name} {what}")
        }
        // A composite type and a column of it, or a change to one.
        22 => {
            if m.composites.is_empty() || rng.chance(40) {
                let mut pool = vec![Ty::Int, Ty::Text, Ty::Numeric, Ty::BigInt];
                pool.extend(m.enums.iter().map(|e| Ty::Enum(e.0.clone())));
                pool.extend(m.domains.iter().filter(|d| !d.not_null).map(|d| Ty::Domain(d.name.clone())));
                let name = styled(rng, m.fresh("p"));
                let attrs: Vec<(String, Ty)> = (0..2 + rng.below(2)).map(|k| (styled(rng, format!("a{k}")), rng.pick(&pool).unwrap().clone())).collect();
                m.composites.push(Composite { name: name.clone(), attrs, comment: None });
                let ty = Ty::Composite(name.clone());
                let cname = styled(rng, m.fresh("n"));
                let default = rng.chance(50).then(|| m.default_of(&ty));
                m.tables[ti].cols.push(Col { default, ..col(&cname, ty) });
                return format!("composite {name}");
            }
            let k = rng.below(m.composites.len());
            let c = &mut m.composites[k];
            let what = match rng.below(3) {
                0 => {
                    let name = (0..).map(|k| format!("b{k}")).find(|n| !c.attrs.iter().any(|a| &a.0 == n)).unwrap();
                    c.attrs.push((name, Ty::Int));
                    "attribute added"
                }
                1 if c.attrs.len() > 1 => {
                    let at = 1 + rng.below(c.attrs.len() - 1);
                    c.attrs.remove(at);
                    "attribute dropped"
                }
                _ => {
                    let at = rng.below(c.attrs.len());
                    c.attrs[at].1 = match c.attrs[at].1 {
                        Ty::Int => Ty::BigInt,
                        Ty::BigInt => Ty::Numeric,
                        Ty::Numeric => Ty::Text,
                        _ => return "nothing".into(),
                    };
                    "attribute retyped"
                }
            };
            let (name, snapshot) = (c.name.clone(), m.clone());
            // A default of the type holds a row of its attributes, which changed.
            for t in &mut m.tables {
                for x in &mut t.cols {
                    if x.ty == Ty::Composite(name.clone()) && x.default.is_some() {
                        x.default = Some(snapshot.default_of(&x.ty));
                    }
                }
            }
            format!("composite {name} {what}")
        }
        // A comment set, changed or taken off.
        23 => {
            let note = Some(rng.pick(NOTES).unwrap().to_string());
            let flip = |c: &mut Option<String>, rng: &mut Rng| *c = if c.is_some() && rng.chance(50) { None } else { note.clone() };
            match rng.below(6) {
                0 => flip(&mut m.tables[ti].comment, rng),
                1 => {
                    let k = rng.below(m.tables[ti].cols.len());
                    flip(&mut m.tables[ti].cols[k].comment, rng)
                }
                2 if !m.funcs.is_empty() => {
                    let k = rng.below(m.funcs.len());
                    flip(&mut m.funcs[k].comment, rng)
                }
                3 if !m.domains.is_empty() => {
                    let k = rng.below(m.domains.len());
                    flip(&mut m.domains[k].comment, rng)
                }
                4 if !m.composites.is_empty() => {
                    let k = rng.below(m.composites.len());
                    flip(&mut m.composites[k].comment, rng)
                }
                5 if !m.enums.is_empty() => flip(&mut m.enum_comment, rng),
                _ => return "nothing".into(),
            }
            "comment".into()
        }
        // A grant on a table, or a function's revoked from public.
        24 => {
            if !m.funcs.is_empty() && rng.chance(50) {
                let k = rng.below(m.funcs.len());
                m.funcs[k].revoked = !m.funcs[k].revoked;
                format!("revoke {}", m.funcs[k].name)
            } else {
                m.tables[ti].granted = !m.tables[ti].granted;
                format!("grant {table}")
            }
        }
        // A trigger added, dropped or changed.
        25 => {
            let cols: Vec<String> = m.tables[ti].cols.iter().filter(|c| c.generated.is_none()).map(|c| c.name.clone()).collect();
            let mine: Vec<usize> = (0..m.triggers.len()).filter(|k| m.triggers[*k].table == table).collect();
            match (rng.pick(&mine).copied(), rng.below(3)) {
                (Some(k), 0) => {
                    m.triggers.remove(k);
                    "drop trigger".into()
                }
                (Some(k), 1) => {
                    m.triggers[k].variant += 1;
                    "trigger body".into()
                }
                (Some(k), _) => {
                    let t = &mut m.triggers[k];
                    t.when = if t.when.is_some() { None } else { rng.pick(&cols).cloned() };
                    t.of = if rng.chance(50) { rng.pick(&cols).cloned() } else { None };
                    "trigger columns".into()
                }
                (None, _) => {
                    let name = styled(rng, m.fresh("tg"));
                    let of = rng.chance(30).then(|| rng.pick(&cols).cloned()).flatten();
                    let when = rng.chance(40).then(|| rng.pick(&cols).cloned()).flatten();
                    m.triggers.push(Trigger { name: name.clone(), table, variant: rng.below(3), of, when });
                    format!("trigger {name}")
                }
            }
        }
        // Row level security, and a policy added, dropped or changed.
        26 => {
            let cols: Vec<String> = m.tables[ti].cols.iter().map(|c| c.name.clone()).collect();
            let mine: Vec<usize> = (0..m.policies.len()).filter(|k| m.policies[*k].table == table).collect();
            match (rng.pick(&mine).copied(), rng.below(3)) {
                (_, 0) => {
                    m.tables[ti].rls = !m.tables[ti].rls;
                    "row level security".into()
                }
                (Some(k), 1) => {
                    m.policies.remove(k);
                    "drop policy".into()
                }
                (Some(k), _) => {
                    m.policies[k].expr = match &m.policies[k].expr {
                        Pexpr::True => Pexpr::NotNull(rng.pick(&cols).unwrap().clone()),
                        _ => Pexpr::True,
                    };
                    "policy changed".into()
                }
                (None, _) => {
                    let name = styled(rng, m.fresh("pol"));
                    let expr = if rng.chance(50) { Pexpr::NotNull(rng.pick(&cols).unwrap().clone()) } else { Pexpr::True };
                    m.tables[ti].rls = true;
                    m.policies.push(Policy { name: name.clone(), table, expr });
                    format!("policy {name}")
                }
            }
        }
        // A sequence the column owns, as its default, added or taken off.
        27 => {
            let t = &m.tables[ti];
            let open: Vec<usize> = (0..t.cols.len()).filter(|k| t.cols[*k].ty.integral() && t.cols[*k].generated.is_none() && !t.cols[*k].identity).collect();
            let Some(&k) = rng.pick(&open) else { return "nothing".into() };
            let c = &mut m.tables[ti].cols[k];
            c.seq = !c.seq;
            c.call = None;
            c.default = None;
            format!("sequence {}", c.name)
        }
        // Identity added, dropped, or started elsewhere.
        28 => {
            if m.tables[ti].part.is_some() {
                return "nothing".into();
            }
            let t = &m.tables[ti];
            let open: Vec<usize> = (0..t.cols.len()).filter(|k| t.cols[*k].ty.integral() && t.cols[*k].generated.is_none()).collect();
            let Some(&k) = rng.pick(&open) else { return "nothing".into() };
            let c = &mut m.tables[ti].cols[k];
            let what = if c.identity && rng.chance(50) {
                c.start = match c.start {
                    None => Some(100),
                    Some(_) => None,
                };
                "identity restarted"
            } else if c.identity {
                c.identity = false;
                c.start = None;
                "identity dropped"
            } else {
                c.identity = true;
                c.not_null = true;
                c.default = None;
                c.call = None;
                c.seq = false;
                "identity added"
            };
            format!("{what} {}", c.name)
        }
        // Extended statistics over two columns added, or dropped.
        30 => {
            if !m.tables[ti].stats.is_empty() && rng.chance(40) {
                let k = rng.below(m.tables[ti].stats.len());
                m.tables[ti].stats.remove(k);
                return "drop statistics".into();
            }
            let cols: Vec<String> = m.tables[ti].cols.iter().filter(|c| c.generated.is_none()).map(|c| c.name.clone()).collect();
            if cols.len() < 2 {
                return "nothing".into();
            }
            let a = rng.below(cols.len());
            let b = (a + 1 + rng.below(cols.len() - 1)) % cols.len();
            let name = styled(rng, m.fresh("st"));
            m.tables[ti].stats.push((name.clone(), vec![cols[a].clone(), cols[b].clone()]));
            format!("statistics {name}")
        }
        // A partition added or dropped.
        _ => {
            let name = m.fresh(&format!("{table}_p"));
            if m.tables[ti].part.is_none() {
                return "nothing".into();
            }
            // A partition holding rows another table's rows reference stays.
            let referenced = m.tables.iter().any(|o| o.fks.iter().any(|f| f.2 == table));
            let p = m.tables[ti].part.as_mut().unwrap();
            let open: Vec<usize> = (0..p.children.len()).filter(|k| !referenced || p.children[*k].1.is_some_and(|(lo, _)| lo >= 10)).collect();
            if p.children.len() > 1 && !open.is_empty() && rng.chance(40) {
                let k = open[rng.below(open.len())];
                p.children.remove(k);
                "drop partition".into()
            } else {
                let lo = 10 + 2 * p.children.len() as i64;
                p.children.push((name, Some((lo, lo + 2))));
                "add partition".into()
            }
        }
    }
}

/// Something new that calls a function: a column's default, a check, a view or a policy.
fn use_func(m: &mut Model, rng: &mut Rng, name: &str) -> &'static str {
    let Some(f) = m.func(name).cloned() else { return "nothing" };
    let ti = rng.below(m.tables.len());
    let table = m.tables[ti].name.clone();
    let cols: Vec<Col> = m.tables[ti].cols.clone();
    let open_default: Vec<&Col> = cols.iter().filter(|c| c.generated.is_none() && !c.identity && !c.seq && call_fits(&f, &c.ty)).collect();
    let arg_cols: Vec<&Col> = cols.iter().filter(|c| f.args.first().is_some_and(|a| arg_fits(a, &c.ty))).collect();
    let immutable = f.reads().is_none() && !f.args.is_empty() && !arg_cols.is_empty();
    match rng.below(7) {
        // A generated column reads no generated column.
        5 if immutable && arg_cols.iter().any(|c| c.generated.is_none()) => {
            let plain: Vec<&&Col> = arg_cols.iter().filter(|c| c.generated.is_none()).collect();
            let c = rng.pick(&plain).unwrap().name.clone();
            let cname = styled(rng, m.fresh("g"));
            m.tables[ti].cols.push(Col { generated: Some(String::new()), gen_call: Some((name.into(), c)), ..col(&cname, f.ret.clone()) });
            "generated column"
        }
        6 if immutable => {
            let c = rng.pick(&arg_cols).unwrap().name.clone();
            let iname = styled(rng, m.fresh("i"));
            m.tables[ti].fn_indexes.push((iname, name.into(), c));
            "index"
        }
        0 if !open_default.is_empty() => {
            let c = rng.pick(&open_default).unwrap().name.clone();
            let x = m.tables[ti].cols.iter_mut().find(|x| x.name == c).unwrap();
            x.call = Some(name.into());
            "default"
        }
        // Adding a column to a partitioned table evaluates its default partition by partition, so a
        // default that reads the table finds the column missing in the next: Postgres refuses it.
        0 | 1 if !(f.reads().is_some() && m.tables[ti].part.is_some()) => {
            let ty = if f.ret.family() && !matches!(f.body, Body::Min(..)) && rng.chance(30) { rng.pick(RESULTS).unwrap().clone() } else { f.ret.clone() };
            let cname = styled(rng, m.fresh("n"));
            // A default that reads a table may read only nulls, which a column refusing null can't hold.
            m.tables[ti].cols.push(Col { call: Some(name.into()), not_null: f.reads().is_none() && rng.chance(30), ..col(&cname, ty) });
            "new column's default"
        }
        2 if f.args.is_empty() || !arg_cols.is_empty() => {
            let c = if f.args.is_empty() { String::new() } else { rng.pick(&arg_cols).unwrap().name.clone() };
            let ck = styled(rng, m.fresh("ck"));
            m.tables[ti].checks.push(Check { name: ck, col: c, expr: Cexpr::Call(name.into()) });
            "check"
        }
        2 | 3 => {
            let c = rng.pick(&arg_cols).map(|c| c.name.clone());
            let vname = styled(rng, m.fresh("v"));
            let shown: Vec<String> = cols.iter().take(1 + rng.below(2)).map(|c| c.name.clone()).collect();
            if f.args.is_empty() || c.is_some() || rng.chance(50) {
                m.views.push(View { name: vname, table, cols: shown, call: Some((name.into(), if f.args.is_empty() { None } else { c })), over: None, materialized: rng.chance(20), invoker: rng.chance(20) });
            } else {
                m.views.push(View { name: vname, table, cols: shown, call: Some((name.into(), None)), over: None, materialized: rng.chance(20), invoker: rng.chance(20) });
            }
            "view"
        }
        _ => {
            let c = rng.pick(&arg_cols).map(|c| c.name.clone());
            let pname = styled(rng, m.fresh("pol"));
            m.tables[ti].rls = rng.chance(70);
            m.policies.push(Policy { name: pname, table, expr: Pexpr::Call(name.into(), if f.args.is_empty() { None } else { c }) });
            "policy"
        }
    }
}

/// The last panic's message and place, which the hook keeps for the step that panicked.
static PANIC: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// A future whose panic is its output's error: a panic in the engine is a failure of the step, shrunk
/// and written as any other.
struct Caught<F>(std::pin::Pin<Box<F>>);

impl<F: std::future::Future> std::future::Future for Caught<F> {
    type Output = Result<F::Output, String>;
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.0.as_mut().poll(cx))) {
            Ok(std::task::Poll::Ready(v)) => std::task::Poll::Ready(Ok(v)),
            Ok(std::task::Poll::Pending) => std::task::Poll::Pending,
            Err(_) => std::task::Poll::Ready(Err(PANIC.lock().map(|p| p.clone()).unwrap_or_default())),
        }
    }
}

/// What went wrong with a step, the first line its kind.
async fn check(admin: &mut Db, base: &str, tag: &str, v1: &str, seed: &str, v2: &str, alterer: &diff::postgres::PostgresAlterer) -> Option<String> {
    match Caught(Box::pin(checked(admin, base, tag, v1, seed, v2, alterer))).await {
        Ok(problem) => problem,
        Err(panic) => Some(format!("panic: {}", panic.replace('\n', " "))),
    }
}

async fn checked(admin: &mut Db, base: &str, tag: &str, v1: &str, seed: &str, v2: &str, alterer: &diff::postgres::PostgresAlterer) -> Option<String> {
    let here = std::env::temp_dir();
    let names: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| format!("fuzz_{tag}_{s}")).collect();
    for n in &names {
        admin.exec(&format!("drop database if exists {n} with (force)")).await.unwrap();
        admin.exec(&format!("create database {n}")).await.unwrap();
    }
    let mut problems = Vec::new();
    {
        let mut a = Db::connect(&with_database(base, &names[0]), &here).await.unwrap();
        let mut b = Db::connect(&with_database(base, &names[1]), &here).await.unwrap();
        if let Err(e) = a.exec(v1).await {
            return finish(admin, &names, Some(format!("setup v1: {e}")), true).await;
        }
        if let Err(e) = a.exec(seed).await {
            return finish(admin, &names, Some(format!("setup seed: {e}")), true).await;
        }
        if let Err(e) = b.exec(v2).await {
            return finish(admin, &names, Some(format!("setup v2: {e}")), true).await;
        }
        let mut target = diff::postgres::read(&mut b).await.unwrap();
        diff::postgres::declare(&mut b, &mut target, v2).await.unwrap();
        let before = snapshot(&mut a).await;
        // Rows a column's new type can't hold (a 1.5 made a boolean, through two changes) are the
        // generator's: no plan takes them there.
        let target_types = snapshot(&mut b).await;
        for (col, (ty, vals)) in &before {
            if let Some((to_ty, _)) = target_types.get(col).filter(|(t, _)| t != ty) {
                if converted(&mut a, vals, ty, to_ty).await.is_none() {
                    a.close().await;
                    b.close().await;
                    return finish(admin, &names, Some("setup rows: a column's values don't convert".into()), true).await;
                }
            }
        }
        // A composite type whose attributes change, or a domain whose base does, keeps its name
        // while its values read otherwise: those columns are held to their rows and their nulls.
        let (defs_a, defs_b) = (type_defs(&mut a).await, type_defs(&mut b).await);
        let mut plan = diff::plan(&diff::postgres::read(&mut a).await.unwrap(), &target, alterer);
        diff::postgres::style(&mut a, &mut plan, &target.verbatim).await.unwrap();
        let failed = run(&mut a, &plan).await;
        if let Some(f) = failed {
            problems.push(format!("forward: the plan failed: {f}\n{}", plan.sql()));
        } else {
            let (x, y) = difference(&catalog(&mut a).await.unwrap(), &catalog(&mut b).await.unwrap());
            if !x.is_empty() || !y.is_empty() {
                problems.push(format!("forward: differs from v2\n{}\n{}", x.iter().chain(&y).cloned().collect::<Vec<_>>().join("\n"), plan.sql()));
            } else {
                let lossy: Vec<String> = plan.losses.iter().chain(plan.narrowings.iter().map(|(c, _)| c)).cloned().collect();
                let after = snapshot(&mut a).await;
                for (col, (ty, vals)) in &before {
                    if lossy.iter().any(|l| loses(l, col)) {
                        continue;
                    }
                    let Some((now_ty, now)) = after.get(col) else { continue };
                    let reshaped = ty == now_ty && defs_a.get(ty) != defs_b.get(ty);
                    let want = if reshaped { None } else if ty == now_ty { Some(vals.clone()) } else { converted(&mut a, vals, ty, now_ty).await };
                    let nulls = |v: &Vec<String>| v.iter().filter(|x| *x == "NULL").count();
                    let changed = match &want {
                        Some(w) => w != now,
                        None => nulls(vals) != nulls(now) || vals.len() != now.len(),
                    };
                    if changed {
                        problems.push(format!("forward: values changed in {col} ({ty} to {now_ty}): {vals:?} -> {now:?}\n{}", plan.sql()));
                    }
                }
                let mut again = diff::plan(&diff::postgres::read(&mut a).await.unwrap(), &target, alterer);
                diff::postgres::style(&mut a, &mut again, &target.verbatim).await.unwrap();
                if again.runnable().next().is_some() {
                    problems.push(format!("forward: a second plan is not empty\n{}", again.sql()));
                }
            }
        }
        a.close().await;
        b.close().await;
    }
    if problems.is_empty() {
        let mut c = Db::connect(&with_database(base, &names[2]), &here).await.unwrap();
        let mut d = Db::connect(&with_database(base, &names[3]), &here).await.unwrap();
        c.exec(v2).await.unwrap();
        d.exec(v1).await.unwrap();
        let mut origin = diff::postgres::read(&mut d).await.unwrap();
        diff::postgres::declare(&mut d, &mut origin, v1).await.unwrap();
        let mut plan = diff::plan(&diff::postgres::read(&mut c).await.unwrap(), &origin, alterer);
        diff::postgres::style(&mut c, &mut plan, &origin.verbatim).await.unwrap();
        if let Some(f) = run(&mut c, &plan).await {
            problems.push(format!("back: the plan failed: {f}\n{}", plan.sql()));
        } else {
            let (x, y) = difference(&catalog(&mut c).await.unwrap(), &catalog(&mut d).await.unwrap());
            if !x.is_empty() || !y.is_empty() {
                problems.push(format!("back: differs from v1\n{}\n{}", x.iter().chain(&y).cloned().collect::<Vec<_>>().join("\n"), plan.sql()));
            }
        }
        c.close().await;
        d.close().await;
    }
    finish(admin, &names, problems.first().cloned(), false).await
}

/// Whether a loss the plan reports, a table's or a column's id, covers a column of the snapshot.
fn loses(loss: &str, col: &str) -> bool {
    let (kind, id) = loss.split_once(' ').unwrap_or(("", loss));
    let id = id.strip_prefix("public.").unwrap_or(id);
    match kind {
        "table" => col.starts_with(&format!("{id}.")),
        _ => id == col,
    }
}

/// The steps the generator made that no plan can take (counted, so a run that checked nothing
/// shows), by the first words of why.
static SKIPPED: std::sync::Mutex<BTreeMap<String, usize>> = std::sync::Mutex::new(BTreeMap::new());

fn skipped(why: &str) {
    let short: String = kind(why).split_whitespace().take(8).collect::<Vec<_>>().join(" ");
    *SKIPPED.lock().unwrap().entry(short).or_default() += 1;
}

async fn finish(admin: &mut Db, names: &[String], problem: Option<String>, setup: bool) -> Option<String> {
    for n in names {
        let _ = admin.exec(&format!("drop database if exists {n} with (force)")).await;
    }
    // A model the server refuses is the generator's, not the engine's.
    if setup {
        skipped(problem.as_deref().unwrap_or("setup"));
        None
    } else {
        problem
    }
}

async fn run(db: &mut Db, plan: &diff::Plan) -> Option<String> {
    db.exec("begin").await.unwrap();
    for s in plan.runnable() {
        if let Err(e) = db.exec(s).await {
            let _ = db.exec("rollback").await;
            return Some(format!("{e}\nat {s}"));
        }
    }
    db.exec("commit").await.err().map(|e| format!("commit: {e}"))
}

/// Each column's type and values as text, by `table.column` as SQL writes the names, the values sorted.
async fn snapshot(db: &mut Db) -> BTreeMap<String, (String, Vec<String>)> {
    let mut out = BTreeMap::new();
    let cols = db
        .rows("select format('%I', c.relname) as t, format('%I', a.attname) as c, format_type(a.atttypid, a.atttypmod) as ty from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace where n.nspname = 'public' and c.relkind = 'r' and a.attnum > 0 and not a.attisdropped and a.attgenerated = ''")
        .await
        .unwrap();
    for r in cols {
        let (t, c) = (r.text("t").to_string(), r.text("c").to_string());
        let mut vals: Vec<String> = db.rows(&format!("select coalesce(({c})::text, 'NULL') as v from {t}")).await.map(|rows| rows.iter().map(|r| r.text("v").to_string()).collect()).unwrap_or_default();
        vals.sort();
        out.insert(format!("{t}.{c}"), (r.text("ty").to_string(), vals));
    }
    out
}

/// What each composite type and domain is made of, by its name as `format_type` writes it.
async fn type_defs(db: &mut Db) -> BTreeMap<String, String> {
    db.rows(
        "select format_type(t.oid, null) as ty, coalesce(format_type(nullif(t.typbasetype, 0), nullif(t.typtypmod, -1)), '') || coalesce((select string_agg(a.attname || ' ' || format_type(a.atttypid, a.atttypmod), ', ' order by a.attnum) from pg_attribute a where a.attrelid = t.typrelid and a.attnum > 0 and not a.attisdropped), '') as def
         from pg_type t join pg_namespace n on n.oid = t.typnamespace where n.nspname = 'public' and t.typtype in ('c', 'd') and (t.typrelid = 0 or (select relkind from pg_class where oid = t.typrelid) = 'c')",
    )
    .await
    .unwrap()
    .iter()
    .map(|r| (r.text("ty").to_string(), r.text("def").to_string()))
    .collect()
}

/// Values of type `from` as they read once of type `to`: cast, through their text, made an array of
/// one, or an array's first element; None where none of those reads them.
async fn converted(db: &mut Db, vals: &[String], from: &str, to: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for v in vals {
        if v == "NULL" {
            out.push(v.clone());
            continue;
        }
        let lit = format!("'{}'::{from}", v.replace('\'', "''"));
        let mut tries = vec![format!("({lit})::{to}"), format!("({lit})::text::{to}")];
        if to.ends_with("[]") {
            tries.push(format!("array[{lit}]::{to}"));
        }
        if from.ends_with("[]") {
            tries.push(format!("({lit})[1]::{to}"));
        }
        let mut got = None;
        for t in tries {
            if let Ok(rows) = db.rows(&format!("select coalesce(({t})::text, 'NULL') as v")).await {
                got = Some(rows[0].text("v").to_string());
                break;
            }
        }
        // An assignment to a varchar refuses what the explicit cast above cuts short.
        let limit = to.strip_prefix("character varying(").and_then(|n| n.trim_end_matches(')').parse::<usize>().ok());
        if limit.is_some_and(|n| v.chars().count() > n) {
            return None;
        }
        out.push(got?);
    }
    out.sort();
    Some(out)
}

/// The same failure: its first line, without the names and values in it.
fn kind(problem: &str) -> String {
    let first = problem.lines().next().unwrap_or_default();
    let first = if first.contains("values changed in ") { first.split(" in ").next().unwrap_or(first) } else { first };
    // A catalog that differs differs by the kind of object first named.
    let differs = first.contains("differs from").then(|| problem.lines().nth(1).unwrap_or_default().split_whitespace().next().unwrap_or_default().to_string());
    let first = match &differs {
        Some(what) => format!("{first} in {what}"),
        None => first.to_string(),
    };
    let (mut out, mut quoted, mut depth) = (String::new(), false, 0usize);
    for ch in first.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                if quoted {
                    out.push('_');
                }
            }
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if quoted || depth > 0 || ch.is_ascii_digit() => {}
            _ => out.push(ch),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One thing the shrinker tries to take out of both models.
#[derive(Clone, Debug)]
enum Cut {
    Table(String),
    View(String),
    Col(String, String),
    Func(String),
    Domain(String),
    Composite(String),
    Enum,
    Trigger(String),
    Policy(String),
    Check(String, String),
    Index(String, String),
    Unique(String, String),
    Partition(String, String),
    Comments,
    Grants,
    Security,
}

impl Cut {
    fn all(m: &Model) -> Vec<Cut> {
        let mut out: Vec<Cut> = Vec::new();
        out.extend(m.tables.iter().map(|t| Cut::Table(t.name.clone())));
        out.extend(m.funcs.iter().map(|t| Cut::Func(t.name.clone())));
        out.extend(m.views.iter().map(|t| Cut::View(t.name.clone())));
        out.extend(m.triggers.iter().map(|t| Cut::Trigger(t.name.clone())));
        out.extend(m.policies.iter().map(|t| Cut::Policy(t.name.clone())));
        out.extend([Cut::Comments, Cut::Grants, Cut::Security]);
        for t in &m.tables {
            out.extend(t.checks.iter().map(|c| Cut::Check(t.name.clone(), c.name.clone())));
            out.extend(t.indexes.iter().map(|c| Cut::Index(t.name.clone(), c.0.clone())));
            out.extend(t.fn_indexes.iter().map(|c| Cut::Index(t.name.clone(), c.0.clone())));
            out.extend(t.stats.iter().map(|c| Cut::Index(t.name.clone(), c.0.clone())));
            out.extend(t.uniques.iter().map(|c| Cut::Unique(t.name.clone(), c.0.clone())));
            out.extend(t.part.iter().flat_map(|p| p.children.iter().map(|c| Cut::Partition(t.name.clone(), c.0.clone()))));
            out.extend(t.cols.iter().filter(|c| c.name != "id").map(|c| Cut::Col(t.name.clone(), c.name.clone())));
        }
        out.extend(m.domains.iter().map(|t| Cut::Domain(t.name.clone())));
        out.extend(m.composites.iter().map(|t| Cut::Composite(t.name.clone())));
        if !m.enums.is_empty() {
            out.push(Cut::Enum);
        }
        out
    }

    /// The model without it, or None where something still needs it.
    fn apply(&self, m: &Model) -> Option<Model> {
        let mut m = m.clone();
        match self {
            Cut::Table(t) => m.drop_table(t),
            Cut::View(v) => m.views.retain(|x| &x.name != v && x.over.as_ref() != Some(v)),
            Cut::Func(f) => m.drop_func(f),
            Cut::Trigger(v) => m.triggers.retain(|x| &x.name != v),
            Cut::Policy(v) => m.policies.retain(|x| &x.name != v),
            Cut::Col(t, c) => {
                if m.used(t, c) {
                    return None;
                }
                if let Some(x) = m.tables.iter_mut().find(|x| &x.name == t) {
                    x.cols.retain(|y| &y.name != c);
                }
            }
            Cut::Check(t, n) | Cut::Index(t, n) | Cut::Unique(t, n) | Cut::Partition(t, n) => {
                if let Some(x) = m.tables.iter_mut().find(|x| &x.name == t) {
                    x.checks.retain(|y| &y.name != n);
                    x.indexes.retain(|y| &y.0 != n);
                    x.fn_indexes.retain(|y| &y.0 != n);
                    x.stats.retain(|y| &y.0 != n);
                    x.uniques.retain(|y| &y.0 != n);
                    if let Some(p) = x.part.as_mut() {
                        if p.children.len() > 1 {
                            p.children.retain(|y| &y.0 != n);
                        }
                    }
                }
            }
            Cut::Domain(n) => {
                if m.type_used(n) {
                    return None;
                }
                m.domains.retain(|d| &d.name != n);
            }
            Cut::Composite(n) => {
                if m.type_used(n) {
                    return None;
                }
                m.composites.retain(|d| &d.name != n);
            }
            Cut::Enum => {
                if m.enums.iter().any(|e| m.type_used(&e.0)) {
                    return None;
                }
                m.enums.clear();
                m.enum_comment = None;
            }
            Cut::Comments => {
                m.enum_comment = None;
                m.funcs.iter_mut().for_each(|f| f.comment = None);
                m.domains.iter_mut().for_each(|f| f.comment = None);
                m.composites.iter_mut().for_each(|f| f.comment = None);
                for t in &mut m.tables {
                    t.comment = None;
                    t.cols.iter_mut().for_each(|c| c.comment = None);
                }
            }
            Cut::Grants => {
                m.funcs.iter_mut().for_each(|f| f.revoked = false);
                m.tables.iter_mut().for_each(|t| t.granted = false);
            }
            Cut::Security => {
                m.policies.clear();
                m.tables.iter_mut().for_each(|t| t.rls = false);
            }
        }
        Some(m)
    }
}

/// The pair, shrunk: every object whose removal from both keeps the failure of the same kind is removed.
async fn shrink(admin: &mut Db, base: &str, tag: &str, mut from: Model, mut to: Model, problem: String, alterer: &diff::postgres::PostgresAlterer) -> (Model, Model, String) {
    let want = kind(&problem);
    let mut problem = problem;
    // What the shrinker tries is no step of the run's, skipped or not.
    let skips = SKIPPED.lock().unwrap().clone();
    loop {
        let mut progressed = false;
        let mut cuts = Cut::all(&from);
        cuts.extend(Cut::all(&to));
        let (sql_a, sql_b) = (from.sql(), to.sql());
        let mut tried = BTreeSet::new();
        for cut in cuts {
            let (Some(a), Some(b)) = (cut.apply(&from), cut.apply(&to)) else { continue };
            let (x, y) = (a.sql(), b.sql());
            if (x == sql_a && y == sql_b) || !tried.insert((x.clone(), y.clone())) {
                continue;
            }
            if (a.tables.is_empty() && b.tables.is_empty()) || !a.fits(&b) {
                continue;
            }
            if let Some(p) = check(admin, base, tag, &x, &a.seed(), &y, alterer).await {
                if kind(&p) == want {
                    from = a;
                    to = b;
                    problem = p;
                    progressed = true;
                    break;
                }
            }
        }
        if !progressed {
            *SKIPPED.lock().unwrap() = skips;
            return (from, to, problem);
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn postgres() {
    let Ok(steps) = std::env::var("REYLO_FUZZ").map(|s| s.parse::<usize>().unwrap_or(100)) else {
        return;
    };
    let Ok(base) = std::env::var("REYLO_TEST_PG") else {
        eprintln!("skipped: REYLO_TEST_PG is not set");
        return;
    };
    let seed: u64 = std::env::var("REYLO_FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(0x5eed_f00d);
    let out = PathBuf::from(std::env::var("REYLO_FUZZ_OUT").unwrap_or_else(|_| std::env::temp_dir().join("reylo-fuzz").to_string_lossy().into_owned()));
    let here = std::env::temp_dir();
    let mut admin = Db::connect(&base, &here).await.unwrap();
    let version: i64 = admin.rows("select current_setting('server_version_num') as v").await.unwrap()[0].text("v").parse().unwrap();
    let alterer = diff::postgres::PostgresAlterer::read(&mut admin, version).await.unwrap();
    let tag = format!("{}", std::process::id());
    // A panic is kept for the step, not printed: `check` reports it.
    std::panic::set_hook(Box::new(|info| {
        if let Ok(mut p) = PANIC.lock() {
            *p = info.to_string();
        }
    }));
    // `REYLO_FUZZ_CASE=<dir>` checks the case written there, as it stands, and nothing else.
    if let Ok(dir) = std::env::var("REYLO_FUZZ_CASE") {
        let read = |f: &str| std::fs::read_to_string(PathBuf::from(&dir).join(f)).unwrap_or_default();
        let problem = check(&mut admin, &base, &tag, &read("v1.sql"), &read("seed.sql"), &read("v2.sql"), &alterer).await;
        let skips = SKIPPED.lock().unwrap().clone();
        eprintln!("case {dir}: {}", problem.unwrap_or_else(|| if skips.is_empty() { "passes".into() } else { format!("refused: {skips:?}") }));
        return;
    }
    // Every seed its own stream: an odd multiple of the golden ratio's, never two seeds alike.
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut found = BTreeSet::new();
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for step in 0..steps {
        let from = generate(&mut rng);
        let mut to = from.clone();
        let mutations: Vec<String> = (0..(1 + rng.below(3))).map(|_| mutate(&mut to, &mut rng)).collect();
        for m in &mutations {
            *tally.entry(m.split_whitespace().next().unwrap_or_default().to_string()).or_default() += 1;
        }
        if !from.fits(&to) {
            skipped("rows don't fit");
            continue;
        }
        let Some(problem) = check(&mut admin, &base, &tag, &from.sql(), &from.seed(), &to.sql(), &alterer).await else { continue };
        if !found.insert(kind(&problem)) {
            continue;
        }
        let (a, b, problem) = shrink(&mut admin, &base, &tag, from, to, problem, &alterer).await;
        let dir = out.join(format!("{seed}-{step}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("v1.sql"), a.sql()).unwrap();
        std::fs::write(dir.join("seed.sql"), a.seed()).unwrap();
        std::fs::write(dir.join("v2.sql"), b.sql()).unwrap();
        std::fs::write(dir.join("problem.txt"), format!("mutations: {}\n{problem}\n", mutations.join(", "))).unwrap();
        eprintln!("step {step}: {}", kind(&problem));
    }
    let skips = SKIPPED.lock().unwrap().clone();
    eprintln!("{} kinds of failure in {steps} steps ({} skipped as no plan's to take), written to {}", found.len(), skips.values().sum::<usize>(), out.display());
    for (why, n) in &skips {
        eprintln!("  skipped {n}: {why}");
    }
    eprintln!("mutations: {}", tally.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", "));
}
