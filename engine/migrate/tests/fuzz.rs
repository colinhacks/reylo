//! A fuzzer for the diff engine, run by hand: `REYLO_FUZZ=<steps>` (and `REYLO_FUZZ_SEED`,
//! `REYLO_FUZZ_OUT`) with the server `REYLO_TEST_PG` names. Each step makes a random schema, changes it
//! by a few random mutations, fills the first with rows both accept, plans from one to the other and
//! runs the plan as a migration runs it. The plan must run, the result must read as a fresh build of
//! the target, every column kept must keep its values unless the plan says it loses them, a second
//! plan must be empty, and the way back (from the target without rows) must work too. A step that
//! fails is shrunk to the fewest objects that still fail the same way and written to `REYLO_FUZZ_OUT`
//! as a case for `tests/diff/postgres/`. Without `REYLO_FUZZ` it does nothing.

use reylo_migrate::catalog::{catalog, difference};
use reylo_migrate::db::{with_database, Db};
use reylo_migrate::diff;
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
            Ty::Enum(e) => e.clone(),
        }
    }
    fn integral(&self) -> bool {
        matches!(self, Ty::SmallInt | Ty::Int | Ty::BigInt)
    }
    fn stringy(&self) -> bool {
        matches!(self, Ty::Text | Ty::Varchar(_))
    }
    /// A value of the type for row `k` (1-based), one every type it may become reads too.
    fn value(&self, k: usize, labels: &[String]) -> String {
        match self {
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
            Ty::Enum(_) => format!("'{}'", labels.first().cloned().unwrap_or_default()),
        }
    }
    /// A constant default of the type.
    fn default(&self, labels: &[String]) -> String {
        match self {
            Ty::Jsonb => "'{}'".into(),
            Ty::IntArray => "'{}'".into(),
            Ty::TextArray => "'{}'".into(),
            Ty::Uuid => "'00000000-0000-0000-0000-000000000000'".into(),
            other => other.value(7, labels),
        }
    }
}

#[derive(Clone, Debug)]
struct Col {
    name: String,
    ty: Ty,
    not_null: bool,
    default: Option<String>,
    /// A stored generated column's expression, over another integral column.
    generated: Option<String>,
    identity: bool,
}

#[derive(Clone, Debug)]
struct Table {
    name: String,
    cols: Vec<Col>,
    pk: Vec<String>,
    uniques: Vec<(String, Vec<String>)>,
    checks: Vec<(String, String, String)>,
    indexes: Vec<(String, Vec<String>, Option<String>)>,
    fks: Vec<(String, String, String, String)>,
}

#[derive(Clone, Debug, Default)]
struct Model {
    enums: Vec<(String, Vec<String>)>,
    tables: Vec<Table>,
    views: Vec<(String, String, Vec<String>)>,
}

impl Model {
    fn labels(&self, e: &str) -> Vec<String> {
        self.enums.iter().find(|(n, _)| n == e).map(|(_, l)| l.clone()).unwrap_or_default()
    }
    fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
    }

    /// The model as Postgres SQL, each table after what it references.
    fn sql(&self) -> String {
        let mut out = Vec::new();
        for (name, labels) in &self.enums {
            out.push(format!("create type {name} as enum ({});", labels.iter().map(|l| format!("'{l}'")).collect::<Vec<_>>().join(", ")));
        }
        for t in &self.tables {
            let mut lines: Vec<String> = t
                .cols
                .iter()
                .map(|c| {
                    let mut s = format!("{} {}", c.name, c.ty.sql());
                    if c.identity {
                        s.push_str(" generated by default as identity");
                    } else if let Some(g) = &c.generated {
                        s.push_str(&format!(" generated always as ({g}) stored"));
                    } else if let Some(d) = &c.default {
                        s.push_str(&format!(" default {d}"));
                    }
                    if c.not_null {
                        s.push_str(" not null");
                    }
                    s
                })
                .collect();
            if !t.pk.is_empty() {
                lines.push(format!("constraint {}_pkey primary key ({})", t.name, t.pk.join(", ")));
            }
            for (n, cols) in &t.uniques {
                lines.push(format!("constraint {n} unique ({})", cols.join(", ")));
            }
            for (n, _, expr) in &t.checks {
                lines.push(format!("constraint {n} check ({expr})"));
            }
            out.push(format!("create table {} (\n  {}\n);", t.name, lines.join(",\n  ")));
        }
        for t in &self.tables {
            for (n, col, rt, rc) in &t.fks {
                out.push(format!("alter table {} add constraint {n} foreign key ({col}) references {rt} ({rc});", t.name));
            }
            for (n, cols, wh) in &t.indexes {
                out.push(format!("create index {n} on {} ({}){};", t.name, cols.join(", "), wh.as_ref().map(|w| format!(" where {w}")).unwrap_or_default()));
            }
        }
        for (n, t, cols) in &self.views {
            out.push(format!("create view {n} as select {} from {t};", cols.join(", ")));
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
                let values: Vec<String> = cols
                    .iter()
                    .map(|c| match &c.ty {
                        Ty::Enum(e) => c.ty.value(k, &self.labels(e)),
                        ty => ty.value(k, &[]),
                    })
                    .collect();
                out.push(format!("insert into {} ({}) values ({});", t.name, cols.iter().map(|c| c.name.clone()).collect::<Vec<_>>().join(", "), values.join(", ")));
            }
        }
        out.join("\n") + "\n"
    }

    /// Whether a column is read by anything else of the model.
    fn used(&self, table: &str, col: &str) -> bool {
        let Some(t) = self.table(table) else { return false };
        t.pk.iter().any(|c| c == col)
            || t.uniques.iter().any(|(_, cs)| cs.iter().any(|c| c == col))
            || t.checks.iter().any(|(_, c, _)| c == col)
            || t.indexes.iter().any(|(_, cs, _)| cs.iter().any(|c| c == col))
            || t.fks.iter().any(|(_, c, _, _)| c == col)
            || t.cols.iter().any(|c| c.generated.as_ref().is_some_and(|g| g.split_whitespace().next() == Some(col)))
            || self.tables.iter().any(|o| o.fks.iter().any(|(_, _, rt, rc)| rt == table && rc == col))
            || self.views.iter().any(|(_, vt, cs)| vt == table && cs.iter().any(|c| c == col))
    }
}

const TYPES: &[Ty] = &[Ty::SmallInt, Ty::Int, Ty::BigInt, Ty::Numeric, Ty::Text, Ty::Varchar(20), Ty::Bool, Ty::Date, Ty::Timestamp, Ty::Jsonb, Ty::Uuid, Ty::IntArray, Ty::TextArray];

/// Types a column of type `ty` holding the fuzzer's values may become.
fn becomes(ty: &Ty) -> Vec<Ty> {
    match ty {
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
        Ty::Enum(_) => vec![Ty::Text],
    }
}

fn generate(rng: &mut Rng) -> Model {
    let mut m = Model::default();
    if rng.chance(50) {
        m.enums.push(("mood".into(), vec!["a".into(), "b".into(), "c".into()]));
    }
    for t in 0..(1 + rng.below(3)) {
        let name = format!("t{t}");
        let mut cols = vec![Col { name: "id".into(), ty: Ty::Int, not_null: true, default: None, generated: None, identity: rng.chance(20) }];
        for c in 0..(1 + rng.below(4)) {
            let ty = if !m.enums.is_empty() && rng.chance(15) { Ty::Enum("mood".into()) } else { rng.pick(TYPES).unwrap().clone() };
            let labels = m.labels("mood");
            let default = rng.chance(30).then(|| ty.default(&labels));
            cols.push(Col { name: format!("c{c}"), ty, not_null: rng.chance(30), default, generated: None, identity: false });
        }
        let mut table = Table { name, cols, pk: vec!["id".into()], uniques: vec![], checks: vec![], indexes: vec![], fks: vec![] };
        if t > 0 && rng.chance(40) {
            table.cols.push(Col { name: "parent".into(), ty: Ty::Int, not_null: false, default: None, generated: None, identity: false });
            table.fks.push((format!("t{t}_parent_fkey"), "parent".into(), "t0".into(), "id".into()));
        }
        m.tables.push(table);
    }
    m
}

/// One random change to the model, kept consistent: what reads a column dropped goes with it.
fn mutate(m: &mut Model, rng: &mut Rng) -> String {
    let ti = rng.below(m.tables.len());
    let labels = m.labels("mood");
    let fresh = |m: &Model, prefix: &str| -> String { (0..).map(|k| format!("{prefix}{k}")).find(|n| !m.tables.iter().any(|t| t.cols.iter().any(|c| &c.name == n) || t.indexes.iter().any(|(i, ..)| i == n) || t.checks.iter().any(|(c, ..)| c == n) || t.uniques.iter().any(|(u, _)| u == n)) && !m.views.iter().any(|(v, ..)| v == n)).unwrap() };
    match rng.below(14) {
        0 => {
            let ty = rng.pick(TYPES).unwrap().clone();
            let not_null = rng.chance(40);
            let default = (not_null || rng.chance(30)).then(|| ty.default(&labels));
            let name = fresh(m, "n");
            m.tables[ti].cols.push(Col { name: name.clone(), ty, not_null, default, generated: None, identity: false });
            format!("add column {name}")
        }
        1 => {
            let table = m.tables[ti].name.clone();
            let free: Vec<String> = m.tables[ti].cols.iter().filter(|c| c.name != "id" && !m.used(&table, &c.name)).map(|c| c.name.clone()).collect();
            let Some(c) = rng.pick(&free).cloned() else { return "nothing".into() };
            m.tables[ti].cols.retain(|x| x.name != c);
            format!("drop column {table}.{c}")
        }
        2 | 3 => {
            let table = m.tables[ti].name.clone();
            let free: Vec<usize> = (0..m.tables[ti].cols.len()).filter(|k| m.tables[ti].cols[*k].name != "id" && m.tables[ti].cols[*k].generated.is_none() && !m.tables[ti].fks.iter().any(|(_, c, ..)| *c == m.tables[ti].cols[*k].name)).collect();
            let Some(&k) = rng.pick(&free) else { return "nothing".into() };
            let to = becomes(&m.tables[ti].cols[k].ty);
            let Some(ty) = rng.pick(&to).cloned() else { return "nothing".into() };
            let c = &mut m.tables[ti].cols[k];
            c.ty = ty.clone();
            c.default = c.default.as_ref().map(|_| ty.default(&labels));
            let name = c.name.clone();
            // What reads it by its old type goes.
            let t = &mut m.tables[ti];
            t.checks.retain(|(_, col, _)| *col != name);
            t.indexes.retain(|(_, cols, wh)| !cols.contains(&name) || wh.is_none());
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
            let c = &mut m.tables[ti].cols[k];
            if c.generated.is_some() || c.identity {
                return "nothing".into();
            }
            c.default = if c.default.is_some() && rng.chance(50) { None } else { Some(c.ty.default(&labels)) };
            format!("default {}", c.name)
        }
        6 => {
            let t = &m.tables[ti];
            let Some(c) = rng.pick(&t.cols).cloned() else { return "nothing".into() };
            let name = fresh(m, "i");
            let wh = (rng.chance(30)).then(|| format!("{} is not null", c.name));
            m.tables[ti].indexes.push((name.clone(), vec![c.name.clone()], wh));
            format!("index {name}")
        }
        7 => {
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
            let name = fresh(m, "ck");
            if let Some(c) = rng.pick(&ints).cloned() {
                m.tables[ti].checks.push((name.clone(), c.clone(), format!("{c} > 0")));
            } else if let Some(c) = rng.pick(&texts).cloned() {
                m.tables[ti].checks.push((name.clone(), c.clone(), format!("length({c}) > 0")));
            } else {
                return "nothing".into();
            }
            format!("check {name}")
        }
        9 => {
            let ints: Vec<String> = m.tables[ti].cols.iter().filter(|c| c.ty.integral() && c.name != "id" && c.generated.is_none()).map(|c| c.name.clone()).collect();
            let Some(c) = rng.pick(&ints).cloned() else { return "nothing".into() };
            let name = fresh(m, "g");
            let ty = m.tables[ti].cols.iter().find(|x| x.name == c).map(|x| x.ty.clone()).unwrap_or(Ty::Int);
            m.tables[ti].cols.push(Col { name: name.clone(), ty: if ty == Ty::SmallInt { Ty::Int } else { ty }, not_null: false, default: None, generated: Some(format!("{c} * 2")), identity: false });
            format!("generated {name}")
        }
        10 => {
            let Some(e) = m.enums.first_mut() else { return "nothing".into() };
            match rng.below(3) {
                0 => {
                    let at = 1 + rng.below(e.1.len());
                    let label = (0..).map(|k| format!("x{k}")).find(|l| !e.1.contains(l)).unwrap();
                    e.1.insert(at.min(e.1.len()), label);
                    "label added".into()
                }
                1 if e.1.len() > 2 => {
                    let k = 1 + rng.below(e.1.len() - 1);
                    e.1.remove(k);
                    "label removed".into()
                }
                _ if e.1.len() > 2 => {
                    let k = 1 + rng.below(e.1.len() - 1);
                    let last = e.1.len() - 1;
                    e.1.swap(k, last);
                    "labels reordered".into()
                }
                _ => "nothing".into(),
            }
        }
        11 => {
            let table = m.tables[ti].name.clone();
            let cols: Vec<String> = m.tables[ti].cols.iter().take(1 + rng.below(3)).map(|c| c.name.clone()).collect();
            let name = fresh(m, "v");
            m.views.push((name.clone(), table, cols));
            format!("view {name}")
        }
        12 => {
            if m.views.is_empty() {
                return "nothing".into();
            }
            let k = rng.below(m.views.len());
            m.views.remove(k);
            "drop view".into()
        }
        _ => {
            let texts: Vec<String> = m.tables[ti].cols.iter().filter(|c| c.ty.stringy() || c.ty.integral()).filter(|c| c.name != "id").map(|c| c.name.clone()).collect();
            let Some(c) = rng.pick(&texts).cloned() else { return "nothing".into() };
            let name = fresh(m, "u");
            m.tables[ti].uniques.push((name.clone(), vec![c]));
            format!("unique {name}")
        }
    }
}

/// What went wrong with a step, the first line its kind.
async fn check(admin: &mut Db, base: &str, tag: &str, v1: &str, seed: &str, v2: &str, alterer: &diff::postgres::PostgresAlterer) -> Option<String> {
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
        let mut plan = diff::plan(&diff::postgres::read(&mut a).await.unwrap(), &target, alterer);
        diff::postgres::style(&mut a, &mut plan, &target.verbatim).await.unwrap();
        let failed = run(&mut a, &plan).await;
        if let Some(f) = failed {
            problems.push(format!("forward: the plan failed at {f}\n{}", plan.sql()));
        } else {
            let (x, y) = difference(&catalog(&mut a).await.unwrap(), &catalog(&mut b).await.unwrap());
            if !x.is_empty() || !y.is_empty() {
                problems.push(format!("forward: differs from v2\n{}\n{}", x.iter().chain(&y).cloned().collect::<Vec<_>>().join("\n"), plan.sql()));
            } else {
                let lossy: Vec<String> = plan.losses.iter().chain(plan.narrowings.iter().map(|(c, _)| c)).cloned().collect();
                let after = snapshot(&mut a).await;
                for (col, vals) in &before {
                    if lossy.iter().any(|l| l.ends_with(col.as_str()) || col.starts_with(&format!("{}.", l.trim_start_matches("table ")))) {
                        continue;
                    }
                    if let Some(now) = after.get(col) {
                        if now != vals {
                            problems.push(format!("forward: values of {col} changed: {vals:?} -> {now:?}\n{}", plan.sql()));
                        }
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
            problems.push(format!("back: the plan failed at {f}\n{}", plan.sql()));
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

async fn finish(admin: &mut Db, names: &[String], problem: Option<String>, setup: bool) -> Option<String> {
    for n in names {
        let _ = admin.exec(&format!("drop database if exists {n} with (force)")).await;
    }
    // A model the server refuses is the generator's, not the engine's.
    if setup { None } else { problem }
}

async fn run(db: &mut Db, plan: &diff::Plan) -> Option<String> {
    db.exec("begin").await.unwrap();
    for s in plan.runnable() {
        if let Err(e) = db.exec(s).await {
            let _ = db.exec("rollback").await;
            return Some(format!("{s}: {e}"));
        }
    }
    db.exec("commit").await.err().map(|e| format!("commit: {e}"))
}

/// Each column's values as text, by `table.column`, sorted by the table's key.
async fn snapshot(db: &mut Db) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut out = std::collections::BTreeMap::new();
    let cols = db
        .rows("select c.relname as t, a.attname as c from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace where n.nspname = 'public' and c.relkind = 'r' and a.attnum > 0 and not a.attisdropped and a.attgenerated = ''")
        .await
        .unwrap();
    for r in cols {
        let (t, c) = (r.text("t").to_string(), r.text("c").to_string());
        let vals = db.rows(&format!("select coalesce({c}::text, 'NULL') as v from {t} order by 1")).await.map(|rows| rows.iter().map(|r| r.text("v").to_string()).collect()).unwrap_or_default();
        out.insert(format!("{t}.{c}"), vals);
    }
    out
}

/// The same failure: its kind (the first line, without what names it).
fn kind(problem: &str) -> String {
    let first = problem.lines().next().unwrap_or_default();
    first.split(" at ").next().unwrap_or(first).split(':').take(2).collect::<Vec<_>>().join(":")
}

/// The pair, shrunk: every table, view, index, check and column whose removal from both keeps the
/// failure of the same kind is removed.
async fn shrink(admin: &mut Db, base: &str, tag: &str, mut from: Model, mut to: Model, problem: String, alterer: &diff::postgres::PostgresAlterer) -> (Model, Model, String) {
    let want = kind(&problem);
    let mut problem = problem;
    loop {
        let mut progressed = false;
        let mut candidates: Vec<(Model, Model)> = Vec::new();
        for name in from.tables.iter().map(|t| t.name.clone()).collect::<Vec<_>>() {
            let drop = |m: &Model| -> Model {
                let mut m = m.clone();
                m.tables.retain(|t| t.name != name);
                m.views.retain(|(_, t, _)| *t != name);
                for t in &mut m.tables {
                    t.fks.retain(|(_, _, rt, _)| *rt != name);
                }
                m
            };
            candidates.push((drop(&from), drop(&to)));
        }
        for v in from.views.iter().chain(&to.views).map(|v| v.0.clone()).collect::<Vec<_>>() {
            let drop = |m: &Model| -> Model {
                let mut m = m.clone();
                m.views.retain(|x| x.0 != v);
                m
            };
            candidates.push((drop(&from), drop(&to)));
        }
        for (t, c) in from.tables.iter().flat_map(|t| t.cols.iter().map(move |c| (t.name.clone(), c.name.clone()))).filter(|(_, c)| c != "id").collect::<Vec<_>>() {
            let drop = |m: &Model| -> Option<Model> {
                if m.used(&t, &c) {
                    return None;
                }
                let mut m = m.clone();
                if let Some(x) = m.tables.iter_mut().find(|x| x.name == t) {
                    x.cols.retain(|y| y.name != c);
                }
                Some(m)
            };
            if let (Some(a), Some(b)) = (drop(&from), drop(&to)) {
                candidates.push((a, b));
            }
        }
        for (a, b) in candidates {
            if a.tables.is_empty() && b.tables.is_empty() {
                continue;
            }
            if let Some(p) = check(admin, base, tag, &a.sql(), &a.seed(), &b.sql(), alterer).await {
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
    let mut rng = Rng(seed | 1);
    let mut found = std::collections::BTreeSet::new();
    for step in 0..steps {
        let from = generate(&mut rng);
        let mut to = from.clone();
        let mutations: Vec<String> = (0..(1 + rng.below(3))).map(|_| mutate(&mut to, &mut rng)).collect();
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
    eprintln!("{} kinds of failure in {steps} steps, written to {}", found.len(), out.display());
}
