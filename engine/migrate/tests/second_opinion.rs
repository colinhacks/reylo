//! A second opinion on the diff engine, taken from nothing Reylo reads itself: each database's own
//! description of a schema (Postgres's `pg_dump --schema-only --create`, MySQL's `show create`
//! statements, information_schema and `show grants`, SQLite's `sqlite_schema`, its pragmas and its
//! persistent settings), normalized only where a difference is by design ([`RULES`]).
//!
//! Each pair of schemas (a corpus case, a case of a directory of cases laid out as the corpus is, or a probe of
//! `tests/probes/<dialect>.sql`, two schemas that differ in one property the DDL can set) is
//! planned as the corpus plans it, both ways, and each leg is judged on four things: whether
//! Reylo's comparator sees the two differ, whether the planner plans anything, whether Reylo's
//! comparator then finds the database equal to the target, and whether the second opinion does.
//! A leg the second opinion sees and Reylo doesn't is a property Reylo never reads, so neither
//! `db push` nor `migrate new` would ever plan it.
//!
//! Opt in with `REYLO_SECOND_OPINION=1`. `REYLO_PROBES` (comma-separated substrings) picks
//! probes, `REYLO_CASE` corpus cases, `REYLO_SUITES` names a root of more cases
//! (`<root>/<set>/<dialect>/<case>/`) and `REYLO_SUITE` picks sets of them. `REYLO_PG_DUMP` is the
//! `pg_dump` to run. The report goes to `$REYLO_OUT/second-opinion/` (or the temp directory);
//! nothing here fails a run.

use regex::Regex;
use reylo_migrate::catalog::{catalog, difference, unmodeled};
use reylo_migrate::db::{with_database, Db};
use reylo_migrate::sql::{runs_alone, shadow_problem};
use reylo_migrate::{diff, locks, Dialect};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Every difference the second opinion leaves out, with why it is by design.
pub const RULES: &[(&str, &str)] = &[
    ("pg-session-lines", "pg_dump's SET, set_config, \\restrict and \\connect lines and its comments set up the restoring session; they are no part of the schema (but default_tablespace and default_table_access_method, which say where the next table goes, are kept with each table, index, constraint and materialized view)"),
    ("pg-entry-order", "pg_dump orders entries by kind, name and dependency, breaking ties by OID, so two databases with the same objects made in another order print them in another order: entries are compared as a set"),
    ("pg-database-name", "each side is its own database, so the name pg_dump writes in CREATE DATABASE, \\connect and every ALTER DATABASE is read as <db>"),
    ("column-order", "a column added by a migration goes last while a fresh table has it where it was declared: a table's column and constraint lines are compared as a set (Postgres's CREATE TABLE, MySQL's show create table, SQLite's create text); a composite type's attributes keep their order"),
    ("pg-inherited-local", "a column, default, not-null or CHECK constraint a child inherits is local too (attislocal, conislocal) where the child had it before it inherited, and pg_dump writes it in the child's CREATE TABLE only then: in a child, the inherited columns and constraints are left out of pg_dump's text, and each inherited column is compared by its effective nullability and default instead"),
    ("pg-option-order", "a storage parameter list (reloptions, an index's options) keeps the order its options were set in, which means nothing: each WITH (…) list is compared as a set"),
    ("pg-statistics-column-order", "an extended statistics object's columns are a set Postgres keeps in the table's column order, which a table that starts inheriting, or gains a column, has otherwise"),
    ("pg-event-trigger-tag-order", "an event trigger's tags are a set, kept in the order written"),
    ("pg-acl-order", "an object's privileges are kept in the order they were granted, which pg_dump follows: an ACL entry's lines are compared as a set"),
    ("pg-not-null-names", "Postgres 18 names each not-null constraint it makes <table>_<column>_not_null, after the names it had when it was made, which a rename leaves behind: a not-null constraint whose name ends in _not_null is read without its name"),
    ("pg-identity-sequence-names", "an identity column's sequence is named after its table and column when it is made, which a rename leaves behind: the SEQUENCE NAME of an identity, and its entry's name, are read as <seq>"),
    ("my-database-name", "each side is its own database, so its name, where a definition or a grant writes it, is read as <db>"),
    ("my-auto-increment-counter", "a table's AUTO_INCREMENT counter follows its rows: it is left out of a table that holds rows, and kept on an empty one, where it is what the schema set"),
    ("my-event-start", "an event made with no STARTS starts when it is made, which no two databases share: a start within three hours of now is read as `near now`, and STARTS is left out of show create event's text"),
    ("my-charset-spelling", "show create table writes a column's character set and collation where they were written or differ from the table's, and the table's collation where it isn't its character set's default, so one column can read two ways: they are left out of its text, and compared as information_schema has them (each column's and table's effective ones)"),
    ("my-expression-name-case", "MySQL reads a column's name without regard to case, and keeps an expression's names as written: names inside an index's or a partitioning's expression are read in lowercase"),
    ("my-index-type-spelling", "show create table writes an index's USING BTREE or USING HASH where it was written, even where it is the engine's only or default type: left out of the text, and compared as information_schema.statistics has it (index_type)"),
    ("my-routine-signature-spelling", "show create function and procedure keep the parameter list as written, the case of its types among it: its first line is read in lowercase, and each parameter is compared as information_schema.parameters has it"),
    ("my-index-order", "show create table lists a table's keys in the order the server keeps them, which another order of creation changes: compared as a set, with the columns' order within each index kept"),
    ("sqlite-sequence-table", "SQLite makes sqlite_sequence for the first AUTOINCREMENT table and never drops it, nor lets it be dropped, so a database that once had one keeps it: it is left out"),
    ("sqlite-type-case", "SQLite reads a declared type's affinity without regard to its case, and so do the type names it knows: a type's case is read away"),
    ("sqlite-spelling", "SQLite keeps a statement's text as written: whitespace, comments, identifier quotes and the case of keywords are read away; identifiers and literals keep their case"),
];

// ---------------------------------------------------------------------------------------------
// Pairs.

#[derive(Clone, Default, Debug)]
struct Pair {
    name: String,
    server: String,
    v1: String,
    v2: String,
    seed: String,
    mode: String,
    one_way: bool,
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn wanted(name: &str, var: &str) -> bool {
    match std::env::var(var) {
        Ok(only) if !only.is_empty() => only.split(',').any(|o| name.contains(o.trim())),
        _ => true,
    }
}

/// `tests/probes/<dialect>.sql`: probes, each opened by a line `-- probe: <name>`, with sections
/// opened by `-- base:` (in both), `-- v1:`, `-- v2:`, `-- server:` and `-- seed:` (in v1's
/// database only) and `-- mode:` (MySQL's sql_mode) alone on their lines.
fn probes(dialect: &str) -> Vec<Pair> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/probes").join(format!("{dialect}.sql"));
    let text = read(&path);
    let mut out = Vec::new();
    let mut current: Option<(String, BTreeMap<String, String>)> = None;
    let mut section = String::from("base");
    let finish = |(name, s): (String, BTreeMap<String, String>)| {
        let get = |k: &str| s.get(k).cloned().unwrap_or_default();
        Pair { name, server: get("server"), v1: get("base") + &get("v1"), v2: get("base") + &get("v2"), seed: get("seed"), mode: get("mode").trim().to_string(), one_way: false }
    };
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("-- probe:") {
            if let Some(p) = current.take() {
                out.push(finish(p));
            }
            current = Some((name.trim().to_string(), BTreeMap::new()));
            section = "base".into();
            continue;
        }
        let Some((_, sections)) = current.as_mut() else { continue };
        if let Some(s) = ["base", "v1", "v2", "server", "seed", "mode"].iter().find(|s| line.trim() == format!("-- {s}:")) {
            section = s.to_string();
            continue;
        }
        let entry = sections.entry(section.clone()).or_default();
        entry.push_str(line);
        entry.push('\n');
    }
    if let Some(p) = current.take() {
        out.push(finish(p));
    }
    out.retain(|p| wanted(&p.name, "REYLO_PROBES"));
    out
}

fn case_pair(dir: &Path) -> Pair {
    Pair {
        name: dir.file_name().unwrap().to_string_lossy().into_owned(),
        server: read(&dir.join("server.sql")),
        v1: read(&dir.join("v1.sql")),
        v2: read(&dir.join("v2.sql")),
        seed: read(&dir.join("seed.sql")),
        mode: read(&dir.join("sql_mode")).trim().to_string(),
        one_way: !read(&dir.join("one-way.txt")).trim().is_empty(),
    }
}

fn dirs(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(root).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    out.sort();
    out
}

fn corpus(dialect: &str) -> Vec<Pair> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/diff").join(dialect);
    dirs(&root).iter().map(|d| case_pair(d)).filter(|p| wanted(&p.name, "REYLO_CASE")).collect()
}

/// The cases of a dialect under `REYLO_SUITES`, each named `<set>/<case>`.
fn suites(dialect: &str) -> Vec<Pair> {
    let Ok(root) = std::env::var("REYLO_SUITES") else { return vec![] };
    let mut out = Vec::new();
    for suite in dirs(Path::new(&root)) {
        let s = suite.file_name().unwrap().to_string_lossy().into_owned();
        if !wanted(&s, "REYLO_SUITE") {
            continue;
        }
        for case in dirs(&suite.join(dialect)) {
            let mut p = case_pair(&case);
            p.name = format!("{s}/{}", p.name);
            if wanted(&p.name, "REYLO_CASE") {
                out.push(p);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// One leg: from one schema to another, judged four ways.

#[derive(Default, Debug)]
struct Leg {
    refused: Option<String>,
    warned: Vec<String>,
    /// Reylo's comparator before the plan: the lines only one side has.
    sees: Vec<String>,
    /// The second opinion before the plan.
    second_sees: Vec<String>,
    /// The objects the planner's reader reads differently on the two sides, by id: where a leg
    /// plans nothing, whether the reader missed it or the alterer wrote nothing for it.
    reader: Vec<String>,
    planned: Vec<String>,
    failed: Option<String>,
    reylo_after: Vec<String>,
    second_after: Vec<String>,
    error: Option<String>,
}

impl Leg {
    fn verdict(&self) -> &'static str {
        if self.error.is_some() {
            "error"
        } else if self.sees.is_empty() && self.second_sees.is_empty() {
            "vacuous"
        } else if self.failed.is_some() {
            "plan-failed"
        } else if self.sees.is_empty() && !self.planned.is_empty() && self.second_after.is_empty() {
            // Reylo's comparator reads nothing of it, but the planner plans it: `migrate new` writes
            // it, while `db push`, and every check that compares a database with its migrations,
            // finds nothing to do.
            "UNCOMPARED"
        } else if self.sees.is_empty() {
            // Reylo reads nothing of it: `db push` says the database matches, `migrate new` that
            // schema.sql has nothing new.
            "UNSEEN"
        } else if self.planned.is_empty() {
            // Reylo's comparator sees it, but the planner plans nothing: `migrate new` writes an
            // empty migration; `db push` refuses, as it can't reach schema.sql.
            "UNPLANNED"
        } else if !self.reylo_after.is_empty() {
            "incomplete"
        } else if !self.second_after.is_empty() {
            // Reylo's comparator finds the database equal to the target, which it isn't.
            "PARTIAL"
        } else {
            "ok"
        }
    }

    fn flags(&self) -> String {
        let mut out = String::new();
        if self.refused.is_some() {
            out.push_str(" [refused]");
        }
        if !self.warned.is_empty() {
            out.push_str(" [warned]");
        }
        if !self.sees.is_empty() && self.second_sees.is_empty() {
            out.push_str(" [second-blind]");
        }
        out
    }

    fn details(&self) -> String {
        let mut out = String::new();
        let section = |out: &mut String, title: &str, lines: &[String]| {
            if !lines.is_empty() {
                let _ = writeln!(out, "    {title}:");
                for l in lines.iter().take(40) {
                    let _ = writeln!(out, "      {}", l.replace('\n', "\n        "));
                }
                if lines.len() > 40 {
                    let _ = writeln!(out, "      … {} more", lines.len() - 40);
                }
            }
        };
        if let Some(e) = &self.error {
            let _ = writeln!(out, "    error: {e}");
        }
        if let Some(r) = &self.refused {
            let _ = writeln!(out, "    refused: {r}");
        }
        section(&mut out, "warned", &self.warned);
        section(&mut out, "Reylo before", &self.sees);
        section(&mut out, "second opinion before", &self.second_sees);
        section(&mut out, "planner's reader sees", &self.reader);
        section(&mut out, "planned", &self.planned);
        if let Some(f) = &self.failed {
            let _ = writeln!(out, "    failed: {f}");
        }
        section(&mut out, "Reylo after", &self.reylo_after);
        section(&mut out, "second opinion after", &self.second_after);
        out
    }
}

fn lines(a: &[String], b: &[String]) -> Vec<String> {
    let (only_a, only_b) = difference(a, b);
    only_a.into_iter().map(|l| format!("- {l}")).chain(only_b.into_iter().map(|l| format!("+ {l}"))).collect()
}

/// The ids of the objects two schemas define differently, each with both definitions.
fn reader_differences(a: &diff::Schema, b: &diff::Schema) -> Vec<String> {
    let ids: BTreeSet<&String> = a.objects.keys().chain(b.objects.keys()).collect();
    let short = |d: Option<&diff::Object>| d.map(|o| o.def.replace('\n', " ").chars().take(160).collect::<String>()).unwrap_or_else(|| "(none)".into());
    ids.into_iter()
        .filter(|id| a.objects.get(*id).map(|o| &o.def) != b.objects.get(*id).map(|o| &o.def))
        .map(|id| format!("{id}: {} → {}", short(a.objects.get(id)), short(b.objects.get(id))))
        .collect()
}

fn s<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// The report of one set of pairs: a line per leg, and what each leg that is not `ok` saw.
struct Report {
    title: String,
    text: String,
    counts: BTreeMap<String, usize>,
}

impl Report {
    fn new(title: &str) -> Report {
        Report { title: title.into(), text: String::new(), counts: BTreeMap::new() }
    }

    fn add(&mut self, name: &str, direction: &str, leg: &Leg, details_for_ok: bool) {
        let v = leg.verdict();
        *self.counts.entry(v.to_string()).or_default() += 1;
        let _ = writeln!(self.text, "{v:<12} {name} ({direction}){}", leg.flags());
        if v != "ok" || details_for_ok {
            self.text.push_str(&leg.details());
        }
    }

    fn write(&self) {
        let dir = std::env::var("REYLO_OUT").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir()).join("second-opinion");
        std::fs::create_dir_all(&dir).unwrap();
        let summary = self.counts.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ");
        let file = dir.join(format!("{}.txt", self.title));
        let rules = RULES.iter().map(|(name, why)| format!("  {name}: {why}")).collect::<Vec<_>>().join("\n");
        std::fs::write(&file, format!("{}: {summary}\n\nNormalized by design:\n{rules}\n\n{}", self.title, self.text)).unwrap();
        eprintln!("{}: {summary} ({})", self.title, file.display());
    }
}

fn enabled() -> bool {
    std::env::var("REYLO_SECOND_OPINION").is_ok_and(|v| !v.is_empty())
}

// ---------------------------------------------------------------------------------------------
// Postgres.

struct Pg {
    prefix: String,
    base: String,
    admin: Db,
    alterer: diff::postgres::PostgresAlterer,
    n: usize,
    here: PathBuf,
}

impl Pg {
    async fn open(prefix: &str) -> Option<Pg> {
        let base = std::env::var("REYLO_TEST_PG").ok()?;
        let here = std::env::temp_dir();
        let mut admin = Db::connect(&base, &here).await.unwrap();
        let version: i64 = admin.rows("select current_setting('server_version_num') as v").await.unwrap()[0].text("v").parse().unwrap();
        let alterer = diff::postgres::PostgresAlterer::read(&mut admin, version).await.unwrap();
        // Roles the probes grant to and give objects to: the server's, so made once.
        for role in ["gap_r1", "gap_r2", "gap_owner", "gap_super superuser"] {
            admin.exec(&format!("do $$ begin create role {role}; exception when duplicate_object then null; end $$")).await.unwrap();
        }
        Some(Pg { prefix: prefix.replace('-', "_"), base, admin, alterer, n: 0, here })
    }

    async fn fresh(&mut self, tag: &str) -> (String, Db) {
        self.n += 1;
        let name = format!("gap_{}_{}_{}_{tag}", self.prefix, std::process::id(), self.n);
        let _ = self.admin.exec(&format!("drop database if exists {name} with (force)")).await;
        self.admin.exec(&format!("create database {name}")).await.unwrap();
        let mut db = Db::connect(&with_database(&self.base, &name), &self.here).await.unwrap();
        // A statement of a case that runs away holds up nothing else.
        db.exec("set statement_timeout = '60s'").await.unwrap();
        (name, db)
    }

    async fn drop(&mut self, name: &str) {
        let _ = self.admin.exec(&format!("drop database if exists {name} with (force)")).await;
    }
}

fn pg_dump(url: &str) -> Result<String, String> {
    let bin = std::env::var("REYLO_PG_DUMP").unwrap_or_else(|_| "pg_dump".into());
    let out = std::process::Command::new(bin).args(["--schema-only", "--create", "--no-password", "--dbname", url]).output().map_err(s)?;
    if !out.status.success() {
        return Err(format!("pg_dump: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The columns and CHECK constraints each table inherits, as pg_dump names them: rule
/// `pg-inherited-local`.
const PG_INHERITED: &str = "select format('%I.%I', n.nspname, c.relname) as t, format('%I', a.attname) as c
  from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace where a.attinhcount > 0 and a.attnum > 0 and not a.attisdropped
  union all select format('%I.%I', n.nspname, c.relname), 'CONSTRAINT ' || format('%I', co.conname)
  from pg_constraint co join pg_class c on c.oid = co.conrelid join pg_namespace n on n.oid = c.relnamespace where co.coninhcount > 0";

/// What `pg_dump --schema-only` leaves out that is still the schema's: whether a materialized
/// view is populated, whether an index is valid, each procedural language's privileges, every
/// comment (an extension's own objects' among them, which pg_dump leaves out), and what belongs
/// to an extension or depends on one.
const PG_SUPPLEMENT: &str = "select format('materialized view %I.%I populated=%s', n.nspname, c.relname, c.relispopulated) as line
  from pg_class c join pg_namespace n on n.oid = c.relnamespace where c.relkind = 'm'
  union all select format('index %I.%I valid=%s ready=%s live=%s', n.nspname, c.relname, i.indisvalid, i.indisready, i.indislive)
  from pg_index i join pg_class c on c.oid = i.indexrelid join pg_namespace n on n.oid = c.relnamespace where n.nspname not in ('pg_catalog', 'information_schema') and n.nspname not like 'pg\\_toast%'
  union all select format('language %s acl=%s owner=%s', l.lanname, (select string_agg(x::text, ',' order by x::text) from unnest(coalesce(l.lanacl, acldefault('l', l.lanowner))) x), pg_get_userbyid(l.lanowner)) from pg_language l
  union all select format('inherited column %I.%I.%I notnull=%s default=%s', n.nspname, c.relname, a.attname, a.attnotnull, coalesce(pg_get_expr(d.adbin, d.adrelid), 'none'))
  from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace left join pg_attrdef d on d.adrelid = a.attrelid and d.adnum = a.attnum
  where a.attinhcount > 0 and a.attnum > 0 and not a.attisdropped
  union all select format('description %s: %s', pg_describe_object(d.classoid, d.objoid, d.objsubid), d.description) from pg_description d
  union all select format('dependency %s %s on %s', pg_describe_object(d.classid, d.objid, d.objsubid), d.deptype, pg_describe_object(d.refclassid, d.refobjid, d.refobjsubid)) from pg_depend d where d.deptype in ('x', 'e') and d.objid >= 16384";

async fn pg_second(db: &mut Db, url: &str, name: &str) -> Result<Vec<String>, String> {
    let text = pg_dump(url)?;
    let mut inherited: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for r in db.rows(PG_INHERITED).await.map_err(s)? {
        inherited.entry(r.text("t").to_string()).or_default().insert(r.text("c").to_string());
    }
    let mut out = pg_blocks(&text, name, &inherited);
    out.extend(db.rows(PG_SUPPLEMENT).await.map_err(s)?.iter().map(|r| r.text("line").to_string()));
    out.sort();
    out.dedup();
    Ok(out)
}

fn pg_blocks(text: &str, dbname: &str, inherited: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    static CREATE_TABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^CREATE (?:UNLOGGED |FOREIGN )?TABLE (\S+)").unwrap());
    static NN_INLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"CONSTRAINT (?:"[^"]*_not_null\d*"|[^\s"]*_not_null\d*) NOT NULL"#).unwrap());
    static NN_ADD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"ADD CONSTRAINT (?:"[^"]*_not_null\d*"|[^\s"]*_not_null\d*) NOT NULL"#).unwrap());
    static NN_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""[^"]*_not_null\d*"|[^\s";]*_not_null\d*"#).unwrap());
    static SEQ_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"SEQUENCE NAME \S+").unwrap());
    let text = text.replace(dbname, "<db>");
    // (header, body, where it goes)
    let mut blocks: Vec<(String, Vec<String>, String)> = Vec::new();
    let (mut tablespace, mut am) = (String::new(), String::new());
    let mut header = String::from("preamble");
    let mut body: Vec<String> = Vec::new();
    let mut place = String::new();
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("-- Name: ").or_else(|| line.strip_prefix("-- Data for Name: ")) {
            blocks.push((std::mem::replace(&mut header, h.to_string()), std::mem::take(&mut body), std::mem::take(&mut place)));
            continue;
        }
        if line.starts_with("--") || line.trim().is_empty() {
            continue;
        }
        if let Some(v) = line.strip_prefix("SET default_tablespace = ") {
            tablespace = v.trim_end_matches(';').to_string();
            continue;
        }
        if let Some(v) = line.strip_prefix("SET default_table_access_method = ") {
            am = v.trim_end_matches(';').to_string();
            continue;
        }
        // A grant made by another role than the owner is written under that role's session
        // authorization, which is the grant's grantor: kept.
        if (line.starts_with("SET ") && !line.starts_with("SET SESSION AUTHORIZATION")) || line.starts_with("SELECT pg_catalog.set_config(") || line.starts_with('\\') {
            continue;
        }
        if body.is_empty() {
            place = format!(" [tablespace={tablespace} am={am}]");
        }
        body.push(line.to_string());
    }
    blocks.push((header, body, place));
    static INHERITED_DEFAULT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^ALTER TABLE ONLY (\S+) ALTER COLUMN (\S+) SET DEFAULT ").unwrap());
    static OPTIONS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"WITH \(([^()]*)\)").unwrap());
    static STATISTICS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(CREATE STATISTICS \S+(?: \([^)]*\))? ON )(.*)( FROM \S+;)$").unwrap());
    static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"WHEN TAG IN \(([^()]*)\)").unwrap());
    let sorted = |list: &str| {
        let mut items: Vec<&str> = list.split(", ").collect();
        items.sort();
        items.join(", ")
    };
    let mut out = Vec::new();
    for (mut header, mut body, place) in blocks {
        if body.is_empty() {
            continue;
        }
        // A default of a column the table inherits: compared as the column's effective default
        // (rule `pg-inherited-local`).
        if header.contains("Type: DEFAULT;") {
            if let Some(c) = body.first().and_then(|l| INHERITED_DEFAULT.captures(l)) {
                if inherited.get(&c[1]).is_some_and(|s| s.contains(&c[2])) {
                    continue;
                }
            }
        }
        // Rules `pg-option-order`, `pg-statistics-column-order` and `pg-event-trigger-tag-order`.
        for line in body.iter_mut() {
            *line = OPTIONS.replace_all(line, |c: &regex::Captures| format!("WITH ({})", sorted(&c[1]))).into_owned();
            *line = TAGS.replace_all(line, |c: &regex::Captures| format!("WHEN TAG IN ({})", sorted(&c[1]))).into_owned();
            if let Some(c) = STATISTICS.captures(line) {
                *line = format!("{}{}{}", &c[1], sorted(&c[2]), &c[3]);
            }
        }
        // A table's columns and constraints as a set, without what it inherits.
        if let Some(at) = body.iter().position(|l| CREATE_TABLE.is_match(l) && l.ends_with('(')) {
            let table = CREATE_TABLE.captures(&body[at]).unwrap()[1].to_string();
            let end = body[at + 1..].iter().position(|l| l.starts_with(')')).map(|k| at + 1 + k).unwrap_or(body.len());
            let skip = inherited.get(&table);
            let mut items: Vec<String> = body[at + 1..end]
                .iter()
                .map(|l| l.trim().trim_end_matches(',').to_string())
                .filter(|l| {
                    // `NOT NULL col`, a table's own not-null on a column it inherits; the rest
                    // open with the column's name, or the constraint's.
                    let key = if let Some(rest) = l.strip_prefix("CONSTRAINT ") {
                        format!("CONSTRAINT {}", first_name(rest))
                    } else if let Some(rest) = l.strip_prefix("NOT NULL ") {
                        first_name(rest)
                    } else {
                        first_name(l)
                    };
                    !skip.is_some_and(|s| s.contains(&key))
                })
                .collect();
            items.sort();
            body.splice(at + 1..end, items);
        }
        // Rule `pg-acl-order`: an entry's GRANT and REVOKE lines, in the order the privileges were
        // granted, as a set.
        if header.contains("Type: ACL;") {
            body.sort();
        }
        let mut joined = body.join("\n");
        joined = NN_INLINE.replace_all(&joined, "NOT NULL").into_owned();
        joined = NN_ADD.replace_all(&joined, "ADD CONSTRAINT <nn> NOT NULL").into_owned();
        if joined.contains("AS IDENTITY") {
            joined = SEQ_NAME.replace_all(&joined, "SEQUENCE NAME <seq>").into_owned();
            if let Some((_, rest)) = header.split_once("; ") {
                header = format!("<identity>; {rest}");
            }
        }
        if header.contains("Type: CONSTRAINT;") && joined.contains("<nn> NOT NULL") {
            header = NN_NAME.replace_all(&header, "<nn>").into_owned();
        }
        let placed = ["Type: TABLE;", "Type: INDEX;", "Type: MATERIALIZED VIEW;", "Type: CONSTRAINT;"].iter().any(|t| header.contains(t));
        out.push(format!("{header}{} :: {joined}", if placed { place.as_str() } else { "" }));
    }
    out
}

/// The name a line of a column list opens with, quotes and all.
fn first_name(line: &str) -> String {
    if let Some(rest) = line.strip_prefix('"') {
        let mut out = String::from("\"");
        let mut chars = rest.chars().peekable();
        while let Some(c) = chars.next() {
            out.push(c);
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    out.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
        }
        out
    } else {
        line.split_whitespace().next().unwrap_or_default().to_string()
    }
}

/// The plan files, as `reylo migrate new` writes them and `deploy` runs them, as `tests/diff.rs`
/// runs its corpus: the moves first, then the plan in schema.sql's style.
async fn pg_apply(db: &mut Db, target: &diff::Schema, alterer: &diff::postgres::PostgresAlterer) -> Result<(Vec<String>, Option<String>), String> {
    let current = diff::postgres::read(db).await.map_err(s)?;
    let mut moves = diff::postgres::moves(&current, target);
    diff::postgres::style_statements(db, &mut moves, &target.verbatim).await.map_err(s)?;
    let mut failed = None;
    for m in &moves {
        if let Err(e) = db.exec(m).await {
            failed = Some(format!("{m}: {e}"));
        }
    }
    let mut plan = diff::plan(&diff::postgres::read(db).await.map_err(s)?, target, alterer);
    diff::postgres::style(db, &mut plan, &target.verbatim).await.map_err(s)?;
    let existing = locks::existing_tables(db).await.map_err(s)?;
    let files = match locks::lock_safe(&plan, &existing) {
        Some(files) => locks::file_texts(&plan, &files),
        None => vec![plan.runnable().map(|s| format!("{};", s.trim_end())).collect::<Vec<_>>().join("\n")],
    };
    if failed.is_none() {
        for file in &files {
            if file.trim().is_empty() {
                continue;
            }
            let sql = if runs_alone(file, Dialect::Postgres) { file.clone() } else { format!("begin;\n{file}\ncommit;") };
            if let Err(e) = db.exec(&sql).await {
                let _ = db.exec("rollback").await;
                failed = Some(format!("{file}: {e}"));
                break;
            }
        }
    }
    Ok((moves.into_iter().chain(plan.runnable().cloned()).collect(), failed))
}

async fn pg_leg(pg: &mut Pg, server: &str, source: &str, seed: &str, target: &str) -> Leg {
    let mut leg = Leg { refused: [source, target].iter().find_map(|t| shadow_problem(t, Dialect::Postgres)), ..Leg::default() };
    if !server.trim().is_empty() {
        if let Err(e) = pg.admin.exec(server).await {
            leg.error = Some(format!("server.sql: {e}"));
            return leg;
        }
    }
    let (a, mut from) = pg.fresh("a").await;
    let (b, mut to) = pg.fresh("b").await;
    if let Err(e) = pg_leg_on(pg, &mut leg, &mut from, &a, &mut to, &b, source, seed, target).await {
        leg.error = Some(e);
    }
    from.close().await;
    to.close().await;
    pg.drop(&a).await;
    pg.drop(&b).await;
    leg
}

#[allow(clippy::too_many_arguments)]
async fn pg_leg_on(pg: &Pg, leg: &mut Leg, from: &mut Db, a: &str, to: &mut Db, b: &str, source: &str, seed: &str, target: &str) -> Result<(), String> {
    from.exec(source).await.map_err(|e| format!("source: {e}"))?;
    from.exec(seed).await.map_err(|e| format!("seed: {e}"))?;
    to.exec(target).await.map_err(|e| format!("target: {e}"))?;
    let mut warned: BTreeSet<String> = unmodeled(from, true).await.map_err(s)?.into_iter().collect();
    warned.extend(unmodeled(to, true).await.map_err(s)?);
    leg.warned = warned.into_iter().collect();
    let cb = catalog(to).await.map_err(s)?;
    leg.sees = lines(&catalog(from).await.map_err(s)?, &cb);
    let (url_a, url_b) = (with_database(&pg.base, a), with_database(&pg.base, b));
    let sb = pg_second(to, &url_b, b).await?;
    leg.second_sees = lines(&pg_second(from, &url_a, a).await?, &sb);
    let mut target_schema = diff::postgres::read(to).await.map_err(s)?;
    diff::postgres::declare(to, &mut target_schema, target).await.map_err(s)?;
    leg.reader = reader_differences(&diff::postgres::read(from).await.map_err(s)?, &target_schema);
    let (planned, failed) = pg_apply(from, &target_schema, &pg.alterer).await?;
    leg.planned = planned;
    leg.failed = failed;
    leg.reylo_after = lines(&catalog(from).await.map_err(s)?, &cb);
    leg.second_after = lines(&pg_second(from, &url_a, a).await?, &sb);
    Ok(())
}

async fn pg_run(title: &str, pairs: Vec<Pair>, both_ways: bool) {
    let Some(mut pg) = Pg::open(title).await else {
        eprintln!("skipped: REYLO_TEST_PG is not set");
        return;
    };
    let mut report = Report::new(title);
    for p in &pairs {
        let leg = pg_leg(&mut pg, &p.server, &p.v1, &p.seed, &p.v2).await;
        report.add(&p.name, "v1 to v2", &leg, false);
        if both_ways && !p.one_way {
            let leg = pg_leg(&mut pg, &p.server, &p.v2, "", &p.v1).await;
            report.add(&p.name, "v2 to v1", &leg, false);
        }
    }
    pg.admin.close().await;
    report.write();
}

#[tokio::test(flavor = "current_thread")]
async fn postgres_probes() {
    if enabled() {
        pg_run("postgres-probes", probes("postgres"), true).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn postgres_corpus() {
    if enabled() && std::env::var("REYLO_PROBES").is_err() {
        pg_run("postgres-corpus", corpus("postgres"), true).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn postgres_suites() {
    if enabled() && std::env::var("REYLO_SUITES").is_ok() {
        pg_run("postgres-suites", suites("postgres"), false).await;
    }
}

// ---------------------------------------------------------------------------------------------
// MySQL.

struct My {
    prefix: String,
    base: String,
    admin: Db,
    n: usize,
    here: PathBuf,
}

impl My {
    async fn open(prefix: &str) -> Option<My> {
        let base = std::env::var("REYLO_TEST_MYSQL").ok()?;
        let here = std::env::temp_dir();
        let mut admin = Db::connect(&base, &here).await.unwrap();
        for account in ["'gap_u1'@'%'", "'gap_u2'@'localhost'"] {
            admin.exec(&format!("create user if not exists {account}")).await.unwrap();
        }
        admin.exec("create role if not exists 'gap_role'").await.unwrap();
        Some(My { prefix: prefix.replace('-', "_"), base, admin, n: 0, here })
    }

    async fn fresh(&mut self, tag: &str, mode: &str) -> (String, Db) {
        self.n += 1;
        let name = format!("gap_{}_{}_{}_{tag}", self.prefix, std::process::id(), self.n);
        let _ = self.admin.exec(&format!("drop database if exists `{name}`")).await;
        self.admin.exec(&format!("create database `{name}`")).await.unwrap();
        let mut db = Db::connect(&with_database(&self.base, &name), &self.here).await.unwrap();
        if !mode.is_empty() {
            db.exec(&format!("set session sql_mode = '{mode}'")).await.unwrap();
        }
        (name, db)
    }

    async fn drop(&mut self, name: &str) {
        let _ = self.admin.exec(&format!("drop database if exists `{name}`")).await;
    }
}

/// `concat_ws` of each field, a null as NULL.
fn fields(kind: &str, fields: &[&str]) -> String {
    format!("concat_ws(' | ', '{kind}', {}) as line", fields.iter().map(|f| format!("ifnull(cast({f} as char), 'NULL')")).collect::<Vec<_>>().join(", "))
}

async fn my_second(db: &mut Db) -> Result<Vec<String>, String> {
    let name = db.rows("select database() as n").await.map_err(s)?[0].text("n").to_string();
    db.exec("set session information_schema_stats_expiry = 0").await.map_err(s)?;
    // Read under one mode, whatever the schema set its session to: ANSI_QUOTES, say, changes how
    // every statement show create writes is quoted. Put back as it was after.
    let mode = db.rows("select @@session.sql_mode as m").await.map_err(s)?[0].text("m").to_string();
    db.exec("set session sql_mode = ''").await.map_err(s)?;
    let out = my_second_lines(db, &name).await;
    db.exec(&format!("set session sql_mode = '{mode}'")).await.map_err(s)?;
    out
}

async fn my_second_lines(db: &mut Db, name: &str) -> Result<Vec<String>, String> {
    static AUTO_INCREMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" AUTO_INCREMENT=\d+").unwrap());
    static STARTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" STARTS '[^']*'").unwrap());
    static USING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" USING (?:BTREE|HASH)").unwrap());
    static CHARSET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" (?:DEFAULT )?(?:CHARACTER SET|CHARSET)[ =]\w+| COLLATE[ =]\w+").unwrap());
    let norm = |t: &str| t.replace(&format!("`{name}`"), "`<db>`").replace(name, "<db>");
    let mut out: Vec<String> = Vec::new();
    let queries = [
        format!("select {} from information_schema.schemata where schema_name = database()", fields("schema", &["default_character_set_name", "default_collation_name", "default_encryption"])),
        format!("select {} from information_schema.schemata_extensions where schema_name = database()", fields("schema options", &["options"])),
        format!("select {} from information_schema.tables where table_schema = database()", fields("table", &["table_name", "table_type", "engine", "row_format", "table_collation", "create_options", "table_comment"])),
        format!(
            "select {} from information_schema.columns where table_schema = database()",
            fields("column", &["table_name", "column_name", "column_default", "is_nullable", "column_type", "character_set_name", "collation_name", "extra", "column_comment", "generation_expression", "srs_id"])
        ),
        format!(
            "select {} from information_schema.statistics where table_schema = database()",
            fields("index", &["table_name", "index_name", "seq_in_index", "column_name", "non_unique", "sub_part", "collation", "nullable", "index_type", "comment", "index_comment", "is_visible", "expression"])
        ),
        format!("select {} from information_schema.table_constraints where table_schema = database()", fields("constraint", &["table_name", "constraint_name", "constraint_type", "enforced"])),
        format!("select {} from information_schema.check_constraints where constraint_schema = database()", fields("check", &["constraint_name", "check_clause"])),
        format!(
            "select {} from information_schema.referential_constraints where constraint_schema = database()",
            fields("foreign key", &["table_name", "constraint_name", "unique_constraint_name", "match_option", "update_rule", "delete_rule", "referenced_table_name"])
        ),
        format!(
            "select {} from information_schema.key_column_usage where table_schema = database()",
            fields("key column", &["table_name", "constraint_name", "column_name", "ordinal_position", "position_in_unique_constraint", "referenced_table_name", "referenced_column_name"])
        ),
        format!(
            "select {} from information_schema.partitions where table_schema = database() and partition_name is not null",
            fields(
                "partition",
                &["table_name", "partition_name", "subpartition_name", "partition_ordinal_position", "subpartition_ordinal_position", "partition_method", "subpartition_method", "partition_expression", "subpartition_expression", "partition_description", "partition_comment", "nodegroup", "tablespace_name"]
            )
        ),
        format!(
            "select {} from information_schema.views where table_schema = database()",
            fields("view", &["table_name", "view_definition", "check_option", "is_updatable", "definer", "security_type", "character_set_client", "collation_connection"])
        ),
        format!(
            "select {} from information_schema.routines where routine_schema = database()",
            fields(
                "routine",
                &[
                    "specific_name", "routine_type", "dtd_identifier", "character_set_name", "collation_name", "routine_body", "routine_definition", "external_language", "parameter_style", "is_deterministic", "sql_data_access", "sql_path", "security_type", "sql_mode", "routine_comment", "definer", "character_set_client", "collation_connection", "database_collation",
                ]
            )
        ),
        format!(
            "select {} from information_schema.parameters where specific_schema = database()",
            fields("parameter", &["specific_name", "routine_type", "ordinal_position", "parameter_mode", "parameter_name", "dtd_identifier", "character_set_name", "collation_name"])
        ),
        format!(
            "select {} from information_schema.triggers where trigger_schema = database()",
            fields(
                "trigger",
                &["trigger_name", "event_manipulation", "event_object_table", "action_order", "action_condition", "action_statement", "action_orientation", "action_timing", "sql_mode", "definer", "character_set_client", "collation_connection", "database_collation"]
            )
        ),
        format!(
            "select {} from information_schema.events where event_schema = database()",
            fields(
                "event",
                &[
                    "event_name", "definer", "time_zone", "event_body", "event_definition", "event_type", "execute_at", "interval_value", "interval_field", "sql_mode",
                    "case when starts is null then null when abs(timestampdiff(minute, starts, now())) < 180 then 'near now' else starts end", "ends", "status", "on_completion", "event_comment", "character_set_client", "collation_connection", "database_collation",
                ]
            )
        ),
        format!("select {} from information_schema.schema_privileges where table_schema = database()", fields("schema privilege", &["grantee", "privilege_type", "is_grantable"])),
        format!("select {} from information_schema.table_privileges where table_schema = database()", fields("table privilege", &["grantee", "table_name", "privilege_type", "is_grantable"])),
        format!("select {} from information_schema.column_privileges where table_schema = database()", fields("column privilege", &["grantee", "table_name", "column_name", "privilege_type", "is_grantable"])),
        format!("select {} from mysql.procs_priv where db = database()", fields("routine privilege", &["user", "host", "routine_name", "routine_type", "proc_priv"])),
    ];
    for q in &queries {
        for r in db.rows(q).await.map_err(|e| format!("{q}: {e}"))? {
            let line = norm(r.text("line"));
            // Rule `my-expression-name-case`.
            out.push(if line.starts_with("index |") || line.starts_with("partition |") { fold_backticked(&line) } else { line });
        }
    }
    // Each object's own statement.
    let tables = db.rows("select table_name as t, table_type as ty from information_schema.tables where table_schema = database()").await.map_err(s)?;
    for t in tables {
        let name_q = format!("`{}`", t.text("t").replace('`', "``"));
        if t.text("ty") == "VIEW" {
            for r in db.rows(&format!("show create view {name_q}")).await.map_err(s)? {
                out.push(norm(&format!("show create view {} | {} | {}", r.text("Create View"), r.text("character_set_client"), r.text("collation_connection"))));
            }
            continue;
        }
        let rows: i64 = db.rows(&format!("select count(*) as n from {name_q}")).await.map_err(s)?[0].text("n").parse().unwrap_or(0);
        for r in db.rows(&format!("show create table {name_q}")).await.map_err(s)? {
            let mut text = r.text("Create Table").to_string();
            if rows > 0 {
                text = AUTO_INCREMENT.replace_all(&text, "").into_owned();
            }
            // The body's lines as a set: rules column-order and my-index-order; and each column's
            // and the table's character set and collation as information_schema has them: rule
            // `my-charset-spelling`.
            text = CHARSET.replace_all(&text, "").into_owned();
            // Rule `my-index-type-spelling`.
            text = USING.replace_all(&text, "").into_owned();
            let mut ls: Vec<String> = text.lines().map(String::from).collect();
            if let Some(end) = ls.iter().rposition(|l| l.starts_with(')')) {
                let mut body: Vec<String> = ls[1..end].iter().map(|l| l.trim().trim_end_matches(',').to_string()).collect();
                body.sort();
                ls.splice(1..end, body);
            }
            out.push(norm(&format!("show create table {}", ls.join("\n"))));
        }
    }
    for r in db.rows("select routine_name as n, routine_type as ty from information_schema.routines where routine_schema = database()").await.map_err(s)? {
        let (word, column) = if r.text("ty") == "PROCEDURE" { ("procedure", "Create Procedure") } else { ("function", "Create Function") };
        for c in db.rows(&format!("show create {word} `{}`", r.text("n").replace('`', "``"))).await.map_err(s)? {
            // Rule `my-routine-signature-spelling`: the line with the parameters, in lowercase.
            let text = c.text(column);
            let (signature, rest) = text.split_once('\n').unwrap_or((text, ""));
            out.push(norm(&format!("show create {word} {}\n{rest} | {} | {} | {}", signature.to_lowercase(), c.text("sql_mode"), c.text("character_set_client"), c.text("collation_connection"))));
        }
    }
    for r in db.rows("select trigger_name as n from information_schema.triggers where trigger_schema = database()").await.map_err(s)? {
        for c in db.rows(&format!("show create trigger `{}`", r.text("n").replace('`', "``"))).await.map_err(s)? {
            out.push(norm(&format!("show create trigger {} | {}", c.text("SQL Original Statement"), c.text("sql_mode"))));
        }
    }
    for r in db.rows("select event_name as n from information_schema.events where event_schema = database()").await.map_err(s)? {
        for c in db.rows(&format!("show create event `{}`", r.text("n").replace('`', "``"))).await.map_err(s)? {
            out.push(norm(&format!("show create event {} | {} | {}", STARTS.replace_all(c.text("Create Event"), ""), c.text("sql_mode"), c.text("time_zone"))));
        }
    }
    // Every account with a privilege here, as show grants writes it, in the lines that name here.
    let grantees = db
        .rows(
            "select grantee as g from information_schema.schema_privileges where table_schema = database()
            union select grantee from information_schema.table_privileges where table_schema = database()
            union select grantee from information_schema.column_privileges where table_schema = database()
            union select concat(quote(user), '@', quote(host)) from mysql.procs_priv where db = database()",
        )
        .await
        .map_err(s)?;
    for g in grantees {
        let grantee = g.text("g").to_string();
        if let Ok(rows) = db.rows(&format!("show grants for {grantee}")).await {
            for r in rows {
                let line = r.first().unwrap_or_default().to_string();
                if line.contains(name) {
                    out.push(norm(&format!("grants {line}")));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// The names a MySQL expression quotes, lowercased: column names, which MySQL reads without regard
/// to case.
fn fold_backticked(text: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in text.chars() {
        if c == '`' {
            inside = !inside;
        }
        if inside {
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

async fn my_leg(my: &mut My, p: &Pair, source: &str, seed: &str, target: &str) -> Leg {
    let mut leg = Leg { refused: [source, target].iter().find_map(|t| shadow_problem(t, Dialect::Mysql)), ..Leg::default() };
    if !p.server.trim().is_empty() {
        if let Err(e) = my.admin.script(&p.server).await {
            leg.error = Some(format!("server.sql: {e}"));
            return leg;
        }
    }
    let (a, mut from) = my.fresh("a", &p.mode).await;
    let (b, mut to) = my.fresh("b", &p.mode).await;
    if let Err(e) = my_leg_on(&mut leg, &mut from, &mut to, source, seed, target).await {
        leg.error = Some(e);
    }
    from.close().await;
    to.close().await;
    my.drop(&a).await;
    my.drop(&b).await;
    leg
}

async fn my_leg_on(leg: &mut Leg, from: &mut Db, to: &mut Db, source: &str, seed: &str, target: &str) -> Result<(), String> {
    let alterer = diff::alterer(from).await.map_err(s)?;
    from.script(source).await.map_err(|e| format!("source: {e}"))?;
    from.script(seed).await.map_err(|e| format!("seed: {e}"))?;
    to.script(target).await.map_err(|e| format!("target: {e}"))?;
    let cb = catalog(to).await.map_err(s)?;
    leg.sees = lines(&catalog(from).await.map_err(s)?, &cb);
    let sb = my_second(to).await?;
    leg.second_sees = lines(&my_second(from).await?, &sb);
    let (ra, rb) = (diff::mysql::read(from).await.map_err(s)?, diff::mysql::read(to).await.map_err(s)?);
    leg.reader = reader_differences(&ra, &rb);
    let plan = diff::plan(&ra, &rb, alterer.as_ref());
    leg.planned = plan.runnable().cloned().collect();
    for st in plan.runnable() {
        if let Err(e) = from.exec(st).await {
            leg.failed = Some(format!("{st}: {e}"));
            break;
        }
    }
    leg.reylo_after = lines(&catalog(from).await.map_err(s)?, &cb);
    leg.second_after = lines(&my_second(from).await?, &sb);
    Ok(())
}

async fn my_run(title: &str, pairs: Vec<Pair>, both_ways: bool) {
    let Some(mut my) = My::open(title).await else {
        eprintln!("skipped: REYLO_TEST_MYSQL is not set");
        return;
    };
    let mut report = Report::new(title);
    for p in &pairs {
        let leg = my_leg(&mut my, p, &p.v1, &p.seed, &p.v2).await;
        report.add(&p.name, "v1 to v2", &leg, false);
        if both_ways && !p.one_way {
            let leg = my_leg(&mut my, p, &p.v2, "", &p.v1).await;
            report.add(&p.name, "v2 to v1", &leg, false);
        }
    }
    my.admin.close().await;
    report.write();
}

#[tokio::test(flavor = "current_thread")]
async fn mysql_probes() {
    if enabled() {
        my_run("mysql-probes", probes("mysql"), true).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn mysql_corpus() {
    if enabled() && std::env::var("REYLO_PROBES").is_err() {
        my_run("mysql-corpus", corpus("mysql"), true).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn mysql_suites() {
    if enabled() && std::env::var("REYLO_SUITES").is_ok() {
        my_run("mysql-suites", suites("mysql"), false).await;
    }
}

// ---------------------------------------------------------------------------------------------
// SQLite.

const SQLITE_KEYWORDS: &[&str] = &[
    "abort", "action", "add", "after", "all", "alter", "always", "analyze", "and", "as", "asc", "attach", "autoincrement", "before", "begin", "between", "by", "cascade", "case", "cast", "check", "collate", "column", "commit", "conflict", "constraint", "create", "cross", "current", "current_date",
    "current_time", "current_timestamp", "database", "default", "deferrable", "deferred", "delete", "desc", "detach", "distinct", "do", "drop", "each", "else", "end", "escape", "except", "exclude", "exclusive", "exists", "explain", "fail", "filter", "first", "following", "for", "foreign", "from",
    "full", "generated", "glob", "group", "groups", "having", "if", "ignore", "immediate", "in", "index", "indexed", "initially", "inner", "insert", "instead", "intersect", "into", "is", "isnull", "join", "key", "last", "left", "like", "limit", "match", "materialized", "natural", "no", "not",
    "nothing", "notnull", "null", "nulls", "of", "offset", "on", "or", "order", "others", "outer", "over", "partition", "plan", "pragma", "preceding", "primary", "query", "raise", "range", "recursive", "references", "regexp", "reindex", "release", "rename", "replace", "restrict", "returning",
    "right", "rollback", "row", "rows", "savepoint", "select", "set", "table", "temp", "temporary", "then", "ties", "to", "transaction", "trigger", "unbounded", "union", "unique", "update", "using", "vacuum", "values", "view", "virtual", "when", "where", "window", "with", "without", "strict",
    "rowid", "stored",
    // Type names, whose case SQLite's affinity rules read away: rule `sqlite-type-case`.
    "int", "integer", "tinyint", "smallint", "mediumint", "bigint", "text", "real", "blob", "numeric", "varchar", "char", "character", "clob", "boolean", "date", "datetime", "float", "double", "decimal", "any",
];

/// SQLite's text without its spelling (rule `sqlite-spelling`): tokens one space apart, comments
/// gone, identifier quotes taken off, keywords lowercased; and a table's column list as a set
/// (rule `column-order`).
fn sqlite_text(sql: &str) -> String {
    let chars: Vec<char> = sql.chars().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if c.is_whitespace() {
            k += 1;
        } else if c == '-' && chars.get(k + 1) == Some(&'-') {
            while k < chars.len() && chars[k] != '\n' {
                k += 1;
            }
        } else if c == '/' && chars.get(k + 1) == Some(&'*') {
            k += 2;
            while k + 1 < chars.len() && !(chars[k] == '*' && chars[k + 1] == '/') {
                k += 1;
            }
            k += 2;
        } else if c == '\'' {
            let mut t = String::from("'");
            k += 1;
            while k < chars.len() {
                t.push(chars[k]);
                if chars[k] == '\'' {
                    if chars.get(k + 1) == Some(&'\'') {
                        t.push('\'');
                        k += 2;
                        continue;
                    }
                    k += 1;
                    break;
                }
                k += 1;
            }
            tokens.push(t);
        } else if c == '"' || c == '`' || c == '[' {
            let close = if c == '[' { ']' } else { c };
            let mut t = String::new();
            k += 1;
            while k < chars.len() {
                if chars[k] == close {
                    if close != ']' && chars.get(k + 1) == Some(&close) {
                        t.push(close);
                        k += 2;
                        continue;
                    }
                    k += 1;
                    break;
                }
                t.push(chars[k]);
                k += 1;
            }
            tokens.push(format!("«{t}»"));
        } else if c.is_alphanumeric() || c == '_' || c == '$' {
            let mut t = String::new();
            while k < chars.len() && (chars[k].is_alphanumeric() || chars[k] == '_' || chars[k] == '$') {
                t.push(chars[k]);
                k += 1;
            }
            let lower = t.to_lowercase();
            tokens.push(if SQLITE_KEYWORDS.contains(&lower.as_str()) { lower } else { format!("«{t}»") });
        } else {
            tokens.push(c.to_string());
            k += 1;
        }
    }
    // A table's column list as a set.
    if tokens.len() > 2 && tokens[0] == "create" && tokens.iter().take(4).any(|t| t == "table") {
        if let Some(open) = tokens.iter().position(|t| t == "(") {
            let (mut depth, mut part, mut parts, mut end) = (0, Vec::new(), Vec::new(), tokens.len());
            for (j, t) in tokens.iter().enumerate().skip(open + 1) {
                if t == "(" {
                    depth += 1;
                } else if t == ")" {
                    if depth == 0 {
                        end = j;
                        break;
                    }
                    depth -= 1;
                } else if t == "," && depth == 0 {
                    parts.push(std::mem::take(&mut part).join(" "));
                    continue;
                }
                part.push(t.clone());
            }
            parts.push(part.join(" "));
            parts.sort();
            return format!("{} ( {} ) {}", tokens[..open].join(" "), parts.join(" , "), tokens[(end + 1).min(tokens.len())..].join(" ")).trim().to_string();
        }
    }
    tokens.join(" ")
}

async fn sqlite_second(db: &mut Db) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for p in ["user_version", "application_id", "auto_vacuum", "page_size", "encoding"] {
        let rows = db.rows(&format!("pragma {p}")).await.map_err(s)?;
        out.push(format!("pragma {p} = {}", rows.first().and_then(|r| r.first()).unwrap_or_default()));
    }
    for r in db.rows("select type, name, tbl_name as t, sql from sqlite_schema where name <> 'sqlite_sequence'").await.map_err(s)? {
        out.push(format!("{} {} on {}: {}", r.text("type"), r.text("name"), r.text("t"), r.get("sql").map(sqlite_text).unwrap_or_else(|| "NULL".into())));
    }
    let tables = db.rows("select name, type, ncol, wr, strict from pragma_table_list where schema = 'main' and name <> 'sqlite_sequence'").await.map_err(s)?;
    for t in tables {
        let name = t.text("name").to_string();
        let lit = format!("'{}'", name.replace('\'', "''"));
        out.push(format!("table {name} type={} ncol={} without_rowid={} strict={}", t.text("type"), t.text("ncol"), t.text("wr"), t.text("strict")));
        if name.starts_with("sqlite_") {
            continue;
        }
        for c in db.rows(&format!("select name, type, \"notnull\" as nn, dflt_value as d, pk, hidden from pragma_table_xinfo({lit})")).await.map_err(s)? {
            out.push(format!("column {name}.{} type={} notnull={} default={} pk={} hidden={}", c.text("name"), c.text("type").to_lowercase(), c.text("nn"), c.get("d").unwrap_or("NULL"), c.text("pk"), c.text("hidden")));
        }
        for i in db.rows(&format!("select name, \"unique\" as u, origin, partial from pragma_index_list({lit})")).await.map_err(s)? {
            let index = i.text("name").to_string();
            let cols = db
                .rows(&format!("select seqno, cid, name, \"desc\" as d, coll, key from pragma_index_xinfo('{}') order by seqno", index.replace('\'', "''")))
                .await
                .map_err(s)?
                .iter()
                .map(|c| format!("{}:{}:{}:{}:{}", c.get("name").unwrap_or(if c.text("cid") == "-2" { "<expr>" } else { "<rowid>" }), c.text("d"), c.text("coll"), c.text("key"), c.text("seqno")))
                .collect::<Vec<_>>()
                .join(",");
            // An automatic index is named after its table's order of constraints.
            let shown = if index.starts_with("sqlite_autoindex_") { "<auto>".to_string() } else { index.clone() };
            out.push(format!("index {name}.{shown} unique={} origin={} partial={} ({cols})", i.text("u"), i.text("origin"), i.text("partial")));
        }
        let mut fks: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut actions: BTreeMap<String, String> = BTreeMap::new();
        for f in db.rows(&format!("select id, seq, \"table\" as t, \"from\" as f, \"to\" as o, on_update as u, on_delete as d, \"match\" as m from pragma_foreign_key_list({lit}) order by id, seq")).await.map_err(s)? {
            fks.entry(f.text("id").to_string()).or_default().push(format!("{}->{}", f.text("f"), f.get("o").unwrap_or("NULL")));
            actions.insert(f.text("id").to_string(), format!("{} on update {} on delete {} match {}", f.text("t"), f.text("u"), f.text("d"), f.text("m")));
        }
        for (id, cols) in fks {
            out.push(format!("foreign key {name} ({}) references {}", cols.join(", "), actions[&id]));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Runs the plan as `reylo migrate` runs a file: foreign keys off, one transaction, checked.
async fn sqlite_apply(db: &mut Db, plan: &diff::Plan) -> Option<String> {
    let _ = db.exec("pragma foreign_keys = off").await;
    let _ = db.exec("begin").await;
    let mut failed = None;
    for st in plan.runnable() {
        if let Err(e) = db.exec(st).await {
            failed = Some(format!("{st}: {e}"));
            break;
        }
    }
    if failed.is_none() {
        if let Ok(broken) = db.rows("pragma foreign_key_check").await {
            if !broken.is_empty() {
                failed = Some(format!("{} rows break a foreign key after the plan", broken.len()));
            }
        }
    }
    let _ = db.exec(if failed.is_some() { "rollback" } else { "commit" }).await;
    let _ = db.exec("pragma foreign_keys = on").await;
    failed
}

async fn sqlite_leg(source: &str, seed: &str, target: &str) -> Leg {
    let mut leg = Leg { refused: [source, target].iter().find_map(|t| shadow_problem(t, Dialect::Sqlite)), ..Leg::default() };
    let here = std::env::temp_dir();
    let mut from = Db::connect(":memory:", &here).await.unwrap();
    let mut to = Db::connect(":memory:", &here).await.unwrap();
    let r: Result<(), String> = async {
        from.exec(source).await.map_err(|e| format!("source: {e}"))?;
        from.exec(seed).await.map_err(|e| format!("seed: {e}"))?;
        to.exec(target).await.map_err(|e| format!("target: {e}"))?;
        let cb = catalog(&mut to).await.map_err(s)?;
        leg.sees = lines(&catalog(&mut from).await.map_err(s)?, &cb);
        let sb = sqlite_second(&mut to).await?;
        leg.second_sees = lines(&sqlite_second(&mut from).await?, &sb);
        let (ra, rb) = (diff::sqlite::read(&mut from).await.map_err(s)?, diff::sqlite::read(&mut to).await.map_err(s)?);
        leg.reader = reader_differences(&ra, &rb);
        let plan = diff::plan(&ra, &rb, &diff::sqlite::SqliteAlterer::default());
        leg.planned = plan.runnable().cloned().collect();
        leg.failed = sqlite_apply(&mut from, &plan).await;
        leg.reylo_after = lines(&catalog(&mut from).await.map_err(s)?, &cb);
        leg.second_after = lines(&sqlite_second(&mut from).await?, &sb);
        Ok(())
    }
    .await;
    if let Err(e) = r {
        leg.error = Some(e);
    }
    leg
}

async fn sqlite_run(title: &str, pairs: Vec<Pair>, both_ways: bool) {
    let mut report = Report::new(title);
    for p in &pairs {
        let leg = sqlite_leg(&p.v1, &p.seed, &p.v2).await;
        report.add(&p.name, "v1 to v2", &leg, false);
        if both_ways && !p.one_way {
            let leg = sqlite_leg(&p.v2, "", &p.v1).await;
            report.add(&p.name, "v2 to v1", &leg, false);
        }
    }
    report.write();
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_probes() {
    if enabled() {
        sqlite_run("sqlite-probes", probes("sqlite"), true).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_corpus() {
    if enabled() && std::env::var("REYLO_PROBES").is_err() {
        sqlite_run("sqlite-corpus", corpus("sqlite"), true).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_suites() {
    if enabled() && std::env::var("REYLO_SUITES").is_ok() {
        sqlite_run("sqlite-suites", suites("sqlite"), false).await;
    }
}

#[test]
fn sqlite_text_reads_spelling_away() {
    assert_eq!(sqlite_text("CREATE TABLE \"T\" ( b TEXT, a INT ) STRICT"), sqlite_text("create table T (a INT,b TEXT) strict"));
    assert_ne!(sqlite_text("create table t (a text default 'X')"), sqlite_text("create table t (a text default 'x')"));
    assert_ne!(sqlite_text("create table t (A text)"), sqlite_text("create table t (a text)"));
}
