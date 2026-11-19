//! Postgres's schema as the diff engine's objects. Every definition is the server's own rendering
//! (`pg_get_viewdef`, `pg_get_constraintdef`, `pg_get_functiondef`, `pg_get_triggerdef`, …), read
//! with an empty search_path so that every name is qualified, and every dependency is the server's
//! own record of it (`pg_depend`): nothing of the database is parsed. The objects are those the
//! comparator reads, with the same exclusions: the platform's roles' objects, extensions' members,
//! and Reylo's history table, and also the comments and privileges it does not read. What the plan
//! creates an object with is then the text `schema.sql` declared it with, where `declare` can match
//! the two, since the server's rendering of an expression is not always one it reads back to itself.

use super::{Alterer, Change, Kind, Object, Schema, TypeClass};
use crate::catalog::{not_extension, not_platform, HISTORY_TABLE, PG_USER_NS};
use crate::db::{Db, Row};
use crate::{Dialect, Error};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

mod declared;
pub(crate) use declared::quote_ident;

/// Rewrites the create statements of `schema`, which `read` read from the database `db` is
/// connected to, from the text `sql` declared each object with: `sql` is the file that built that
/// database (`schema.sql`), and its unqualified names resolve through the search_path `db` has.
/// An object the file's text cannot be matched to keeps the server's rendering.
pub async fn declare(db: &mut Db, schema: &mut Schema, sql: &str) -> Result<(), Error> {
    let rows = db.rows("select format('%s', array_to_json(current_schemas(false))) as path").await?;
    let path: Vec<String> = rows.first().and_then(|r| serde_json::from_str(r.text("path")).ok()).unwrap_or_default();
    let found = declared::read(schema, sql, &path);
    declared::apply(schema, &found);
    // An index whose partitions' indexes are attached to it is made on its table alone, as the
    // server renders it, whatever the file wrote: each of those is made and attached on its own.
    for o in schema.objects.values_mut() {
        if let Some(rendering) = o.extra.iter().find_map(|l| l.strip_prefix("rendering ")).map(String::from) {
            if let Some(first) = o.create.first_mut() {
                schema.verbatim.remove(first.as_str());
                *first = rendering;
            }
        }
    }
    Ok(())
}

/// Writes `statements` as a person writes `schema.sql`: words in lowercase, and names in the first
/// schema of the search_path `db` has without their schema, as they resolve to it there. The
/// server's renderings are read under an empty search_path, so they qualify every name and
/// uppercase their keywords. Both changes keep a statement's meaning: an unquoted word's case means
/// nothing to Postgres (a name that needs its case is quoted), and a name keeps its schema where
/// pg_catalog, which every search_path reads first, has a relation, type or function of that name.
/// Strings, dollar-quoted bodies, quoted names and comments keep their text, and so do comment
/// lines and the statements in `verbatim`, which `schema.sql` wrote.
pub async fn style_statements(db: &mut Db, statements: &mut [String], verbatim: &BTreeSet<String>) -> Result<(), Error> {
    let first = first_schema(db).await?;
    let builtin = builtin_names(db).await?;
    for s in statements.iter_mut() {
        if !s.starts_with("--") && !verbatim.contains(s.as_str()) {
            *s = styled(s, &first, &builtin);
        }
    }
    Ok(())
}

/// A plan written as `style_statements` writes statements, its comments naming what it loses or
/// renames without the schema its statements leave out.
pub async fn style(db: &mut Db, plan: &mut super::Plan, verbatim: &BTreeSet<String>) -> Result<(), Error> {
    style_statements(db, &mut plan.statements, verbatim).await?;
    let first = first_schema(db).await?;
    if first.is_empty() {
        return Ok(());
    }
    let qualified = format!(" {first}.");
    for loss in plan.losses.iter_mut().chain(plan.renames.iter_mut().map(|(old, _, _)| old)).chain(plan.narrowings.iter_mut().map(|(id, _)| id)) {
        *loss = loss.replacen(&qualified, " ", 1);
    }
    // A comment holding SQL (what to write instead of a guessed rename) is written as SQL is.
    let builtin = builtin_names(db).await?;
    for s in plan.statements.iter_mut().filter(|s| s.starts_with("--")) {
        *s = match s.strip_prefix("--   ") {
            Some(sql) => format!("--   {}", styled(sql, &first, &builtin)),
            None => s.replace(&qualified, " "),
        };
    }
    Ok(())
}

/// The names of pg_catalog's relations, types and functions, which an unqualified name could mean.
async fn builtin_names(db: &mut Db) -> Result<BTreeSet<String>, Error> {
    Ok(db
        .rows(
            "select relname::text as n from pg_class where relnamespace = 'pg_catalog'::regnamespace
             union select typname::text from pg_type where typnamespace = 'pg_catalog'::regnamespace
             union select proname::text from pg_proc where pronamespace = 'pg_catalog'::regnamespace",
        )
        .await?
        .iter()
        .map(|r| r.text("n").to_string())
        .collect())
}

pub async fn first_schema(db: &mut Db) -> Result<String, Error> {
    Ok(db.rows("select coalesce(current_schema(), '') as s").await?.first().map(|r| r.text("s").to_string()).unwrap_or_default())
}

fn styled(sql: &str, first: &str, builtin: &BTreeSet<String>) -> String {
    use crate::sql::TokenKind::{Quoted, Symbol, Word};
    let tokens = crate::sql::tokenize(sql, Dialect::Postgres);
    // A name as the catalog holds it: a word folded to lowercase, a quoted name unquoted.
    let name = |t: &crate::sql::Token| match t.kind {
        Word => Some(t.text.to_ascii_lowercase()),
        Quoted => Some(t.text[1..t.text.len() - 1].replace("\"\"", "\"")),
        _ => None,
    };
    let dot = |k: usize| tokens.get(k).is_some_and(|t| t.kind == Symbol && t.text == ".");
    let mut out = String::with_capacity(sql.len());
    let mut at = 0;
    let mut k = 0;
    while k < tokens.len() {
        let t = &tokens[k];
        out.push_str(&sql[at..t.start]);
        at = t.end;
        let qualifies = !first.is_empty()
            && name(t).as_deref() == Some(first)
            && dot(k + 1)
            && !(k > 0 && dot(k - 1))
            && tokens.get(k + 2).and_then(name).is_some_and(|n| !builtin.contains(&n));
        if qualifies {
            // The schema and its dot go; the name they qualify follows as it is.
            at = tokens[k + 1].end;
            k += 2;
            continue;
        }
        out.push_str(&if t.kind == Word { t.text.to_ascii_lowercase() } else { t.text.clone() });
        k += 1;
    }
    out.push_str(&sql[at..]);
    out
}

fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// Where a dependency record points, as the reader maps it to an object.
#[derive(Default)]
struct Ids {
    /// pg_class oids: tables, views, sequences, indexes, composite types.
    class: BTreeMap<u32, String>,
    /// (table oid, attnum): columns; a view's columns map to the view.
    column: BTreeMap<(u32, i32), String>,
    types: BTreeMap<u32, String>,
    procs: BTreeMap<u32, String>,
    namespaces: BTreeMap<u32, String>,
    extensions: BTreeMap<u32, String>,
    constraints: BTreeMap<u32, String>,
    rewrites: BTreeMap<u32, String>,
    attrdefs: BTreeMap<u32, String>,
    triggers: BTreeMap<u32, String>,
    policies: BTreeMap<u32, String>,
    statistics: BTreeMap<u32, String>,
    collations: BTreeMap<u32, String>,
    operators: BTreeMap<u32, String>,
    opclasses: BTreeMap<u32, String>,
    opfamilies: BTreeMap<u32, String>,
    casts: BTreeMap<u32, String>,
    servers: BTreeMap<u32, String>,
    publications: BTreeMap<u32, String>,
    event_triggers: BTreeMap<u32, String>,
    wrappers: BTreeMap<u32, String>,
    /// (catalog, oid) of an extension's member: the extension, which stands for it.
    members: BTreeMap<(String, u32), String>,
}

impl Ids {
    fn of(&self, class: &str, oid: u32, sub: i32) -> Option<String> {
        match class {
            "pg_class" if sub > 0 => self.column.get(&(oid, sub)).or_else(|| self.class.get(&oid)).cloned(),
            "pg_class" => self.class.get(&oid).cloned(),
            "pg_type" => self.types.get(&oid).cloned(),
            "pg_proc" => self.procs.get(&oid).cloned(),
            "pg_namespace" => self.namespaces.get(&oid).cloned(),
            "pg_extension" => self.extensions.get(&oid).cloned(),
            "pg_constraint" => self.constraints.get(&oid).cloned(),
            "pg_rewrite" => self.rewrites.get(&oid).cloned(),
            "pg_attrdef" => self.attrdefs.get(&oid).cloned(),
            "pg_trigger" => self.triggers.get(&oid).cloned(),
            "pg_policy" => self.policies.get(&oid).cloned(),
            "pg_statistic_ext" => self.statistics.get(&oid).cloned(),
            "pg_collation" => self.collations.get(&oid).cloned(),
            "pg_operator" => self.operators.get(&oid).cloned(),
            "pg_opclass" => self.opclasses.get(&oid).cloned(),
            "pg_opfamily" => self.opfamilies.get(&oid).cloned(),
            "pg_cast" => self.casts.get(&oid).cloned(),
            "pg_foreign_server" => self.servers.get(&oid).cloned(),
            "pg_publication" => self.publications.get(&oid).cloned(),
            "pg_event_trigger" => self.event_triggers.get(&oid).cloned(),
            "pg_foreign_data_wrapper" => self.wrappers.get(&oid).cloned(),
            _ => None,
        }
        .or_else(|| self.members.get(&(class.to_string(), oid)).cloned())
    }
}

/// Whether the object (`alias` of the catalog `table`) is one the server made with another and
/// drops with it, which no plan makes or drops itself: a range type's constructor functions, its
/// multirange's and the cast between them.
fn made_with(table: &str, alias: &str) -> String {
    format!("exists (select 1 from pg_depend dep where dep.classid = '{table}'::regclass and dep.objid = {alias}.oid and dep.deptype = 'i')")
}

fn oid(r: &Row, name: &str) -> u32 {
    r.text(name).parse().unwrap_or(0)
}

fn table_word(kind: &str) -> &'static str {
    match kind {
        "v" => "view",
        "m" => "materialized view",
        "f" => "foreign table",
        "S" => "sequence",
        "c" => "type",
        _ => "table",
    }
}

/// The schema of the database `db` is connected to.
pub async fn read(db: &mut Db) -> Result<Schema, Error> {
    db.exec("set search_path = ''").await?;
    let result = read_objects(db).await;
    db.exec("reset search_path").await?;
    result
}

async fn read_objects(db: &mut Db) -> Result<Schema, Error> {
    let mut s = Schema::default();
    let mut ids = Ids::default();
    let ns = PG_USER_NS;
    let history = HISTORY_TABLE;

    for r in db
        .rows(&format!(
            "select n.oid, format('%I', n.nspname) as name, {} as acl from pg_namespace n where {ns} and {} and {}",
            acl("n.nspacl", 'n', "n.nspowner"),
            not_extension("pg_namespace", "n"),
            not_platform("n.nspowner")
        ))
        .await?
    {
        let id = format!("schema {}", r.text("name"));
        ids.namespaces.insert(oid(&r, "oid"), id.clone());
        let create = if r.text("name") == "public" { "create schema if not exists public".to_string() } else { format!("create schema {}", r.text("name")) };
        s.add(Object { id: id.clone(), kind: Some(Kind::Schema), def: String::new(), create: vec![create], drop: vec![format!("drop schema {}", r.text("name"))], ..Object::default() });
        grants(&mut s, r.text("acl"), r.text("name"), &id, "schema");
    }

    for r in db
        .rows(&format!(
            "select x.oid, format('%I', x.extname) as name, format('%I', n.nspname) as schema from pg_extension x join pg_namespace n on n.oid = x.extnamespace where x.extname <> 'plpgsql' and {}",
            not_platform("x.extowner")
        ))
        .await?
    {
        let id = format!("extension {}", r.text("name"));
        ids.extensions.insert(oid(&r, "oid"), id.clone());
        s.add(Object {
            id,
            kind: Some(Kind::Extension),
            def: r.text("schema").to_string(),
            create: vec![format!("create extension if not exists {} with schema {}", r.text("name"), r.text("schema"))],
            drop: vec![format!("drop extension {}", r.text("name"))],
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
    }

    for r in db.rows("select format('%s', d.classid::regclass) as cls, d.objid, d.refobjid from pg_depend d where d.deptype = 'e' and d.refclassid = 'pg_extension'::regclass").await? {
        if let Some(ext) = ids.extensions.get(&oid(&r, "refobjid")).cloned() {
            ids.members.insert((r.text("cls").to_string(), oid(&r, "objid")), ext);
        }
    }

    let version: i64 = db.rows("select current_setting('server_version_num') as v").await?.first().and_then(|r| r.first()?.parse().ok()).unwrap_or(0);
    // A collation's locale is `colllocale` from Postgres 17, `colliculocale` in 15 and 16, and an
    // ICU collation's `collcollate` before; its rules are Postgres 16's. Read through `to_jsonb`, a
    // column the server lacks is null rather than an error.
    for r in db
        .rows(&format!(
            "select co.oid, format('%I.%I', n.nspname, co.collname) as name, co.collprovider as provider, co.collisdeterministic as det,
              co.collcollate as lc_collate, co.collctype as lc_ctype,
              coalesce(to_jsonb(co) ->> 'colllocale', to_jsonb(co) ->> 'colliculocale', co.collcollate) as locale, to_jsonb(co) ->> 'collicurules' as rules
            from pg_collation co join pg_namespace n on n.oid = co.collnamespace where {ns} and {} and {}",
            not_extension("pg_collation", "co"),
            not_platform("co.collowner")
        ))
        .await?
    {
        let name = r.text("name");
        let id = format!("collation {name}");
        ids.collations.insert(oid(&r, "oid"), id.clone());
        let provider = match r.text("provider") {
            "i" => "icu",
            "b" => "builtin",
            _ => "libc",
        };
        let mut options = vec![format!("provider = {provider}")];
        match provider {
            "libc" => options.extend([format!("lc_collate = {}", literal(r.text("lc_collate"))), format!("lc_ctype = {}", literal(r.text("lc_ctype")))]),
            _ => options.push(format!("locale = {}", literal(r.text("locale")))),
        }
        if !r.flag("det") {
            options.push("deterministic = false".into());
        }
        if let Some(rules) = r.get("rules") {
            options.push(format!("rules = {}", literal(rules)));
        }
        let def = options.join(", ");
        s.add(Object { id, kind: Some(Kind::Collation), create: vec![format!("create collation {name} ({def})")], drop: vec![format!("drop collation {name}")], def, ..Object::default() });
    }

    // Types: enums, domains and ranges; composite types are relations, below.
    for r in db
        .rows(&format!(
            "select t.oid, format('%I.%I', n.nspname, t.typname) as name, t.typtype as kind,
              format('%s', (select json_agg(e.enumlabel order by e.enumsortorder) from pg_enum e where e.enumtypid = t.oid)) as labels,
              case when t.typtype = 'd' then format_type(t.typbasetype, t.typtypmod) end as base, t.typnotnull as notnull, t.typdefault as def,
              (select string_agg(format(' constraint %I %s', c.conname, pg_get_constraintdef(c.oid)), '' order by c.conname) from pg_constraint c where c.contypid = t.oid and c.contype = 'c' and c.convalidated) as checks,

              format('%s', (select json_agg(json_build_array(format('%I', c.conname), pg_get_constraintdef(c.oid)) order by c.conname) from pg_constraint c where c.contypid = t.oid and c.contype = 'c')) as checklist,
              case when t.typtype = 'd' and t.typcollation <> 0 and t.typcollation <> (select b.typcollation from pg_type b where b.oid = t.typbasetype)
                then (select format('%I.%I', cn.nspname, co.collname) from pg_collation co join pg_namespace cn on cn.oid = co.collnamespace where co.oid = t.typcollation) end as collation,
              case when t.typtype = 'r' then (select concat_ws(', ', 'subtype = ' || format_type(r.rngsubtype, null),
                  case when r.rngsubopc <> 0 and not (select o.opcdefault from pg_opclass o where o.oid = r.rngsubopc) then 'subtype_opclass = ' || (select format('%I.%I', on_.nspname, o.opcname) from pg_opclass o join pg_namespace on_ on on_.oid = o.opcnamespace where o.oid = r.rngsubopc) end,
                  case when r.rngcollation <> 0 and r.rngcollation <> (select st.typcollation from pg_type st where st.oid = r.rngsubtype) then 'collation = ' || (select format('%I.%I', cn.nspname, co.collname) from pg_collation co join pg_namespace cn on cn.oid = co.collnamespace where co.oid = r.rngcollation) end,
                  case when r.rngcanonical <> 0 then 'canonical = ' || format('%s', r.rngcanonical::regproc) end,
                  case when r.rngsubdiff <> 0 then 'subtype_diff = ' || format('%s', r.rngsubdiff::regproc) end,
                  (select 'multirange_type_name = ' || format('%I.%I', mn.nspname, m.typname) from pg_type m join pg_namespace mn on mn.oid = m.typnamespace where m.oid = r.rngmultitypid
                    and (mn.oid <> t.typnamespace or m.typname <> case when position('range' in t.typname) > 0 then overlay(t.typname placing 'multirange' from position('range' in t.typname) for 5) else t.typname || '_multirange' end))) from pg_range r where r.rngtypid = t.oid) end as range,
              {} as acl
            from pg_type t join pg_namespace n on n.oid = t.typnamespace
            where t.typtype in ('e', 'd', 'r') and {ns} and {} and {}",
            acl_in("t.typacl", 'T', "t.typowner", Some("t.typnamespace")),
            not_extension("pg_type", "t"),
            not_platform("t.typowner")
        ))
        .await?
    {
        let name = r.text("name");
        let id = format!("type {name}");
        ids.types.insert(oid(&r, "oid"), id.clone());
        // A domain's checks left unvalidated, which no `create domain` can make.
        let unvalidated: Vec<(String, String)> = serde_json::from_str::<Vec<(String, String)>>(r.text("checklist")).unwrap_or_default().into_iter().filter(|(_, d)| d.ends_with(" NOT VALID")).collect();
        let (def, create, change) = match r.text("kind") {
            "e" => {
                let labels: Vec<String> = serde_json::from_str(r.text("labels")).unwrap_or_default();
                let list = labels.iter().map(|l| literal(l)).collect::<Vec<_>>().join(", ");
                // A label removed or moved changes every column of the type, which blocks what is on them.
                (format!("enum ({list})"), format!("create type {name} as enum ({list})"), Change::Alter { blocks: true })
            }
            "d" => {
                let def = format!(
                    "domain {}{}{}{}{}",
                    r.text("base"),
                    r.get("collation").map(|c| format!(" collate {c}")).unwrap_or_default(),
                    r.get("def").map(|d| format!(" default {d}")).unwrap_or_default(),
                    if r.flag("notnull") { " not null" } else { "" },
                    r.text("checks")
                );
                let unvalidated: String = unvalidated.iter().map(|(n, d)| format!(" constraint {n} {d}")).collect();
                (format!("{def}{unvalidated}"), format!("create {}", def.replacen("domain ", &format!("domain {name} as "), 1)), Change::Alter { blocks: false })
            }
            _ => (format!("range ({})", r.text("range")), format!("create type {name} as range ({})", r.text("range")), Change::Replace),
        };
        let mut extra = Vec::new();
        if r.text("kind") == "d" {
            // Its parts, which its alterer changes one by one.
            extra.push(format!("base {}{}", r.text("base"), r.get("collation").map(|c| format!(" collate {c}")).unwrap_or_default()));
            if let Some(d) = r.get("def") {
                extra.push(format!("default {d}"));
            }
            if r.flag("notnull") {
                extra.push("not null".into());
            }
            for check in serde_json::from_str::<Vec<(String, String)>>(r.text("checklist")).unwrap_or_default() {
                extra.push(format!("check {} {}", check.0, check.1));
            }
        }
        if r.text("kind") == "e" {
            for c in db
                .rows(&format!(
                    "select format('%I.%I', n.nspname, c.relname) as rel, format('%I', a.attname) as col, format_type(a.atttypid, a.atttypmod) as type, pg_get_expr(d.adbin, d.adrelid) as def
                    from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace left join pg_attrdef d on d.adrelid = a.attrelid and d.adnum = a.attnum
                    where c.relkind in ('r', 'p') and not a.attisdropped and a.attnum > 0 and a.attislocal
                      and (a.atttypid = {0} or a.atttypid = (select t.typarray from pg_type t where t.oid = {0}))
                    order by 1, 2",
                    r.text("oid")
                ))
                .await?
            {
                extra.push([c.text("rel"), c.text("col"), c.text("type"), c.text("def")].join(SEP));
            }
        }
        // A check left unvalidated is added after the domain is made, which can't take one.
        let mut create = vec![create];
        create.extend(unvalidated.iter().map(|(n, d)| format!("alter domain {name} add constraint {n} {d}")));
        grants(&mut s, r.text("acl"), name, &id, if r.text("kind") == "d" { "domain" } else { "type" });
        s.add(Object { id, kind: Some(Kind::Type), def, create, drop: vec![format!("drop type {name}")], change, extra, ..Object::default() });
    }
    // A domain's checks are the domain's: what they call, it depends on.
    for r in db.rows("select co.oid, co.contypid from pg_constraint co where co.contypid <> 0").await? {
        if let Some(t) = ids.types.get(&oid(&r, "contypid")).cloned() {
            ids.constraints.insert(oid(&r, "oid"), t);
        }
    }

    // Sequences not owned by an identity column; the column a sequence is owned by is an object
    // of its own, made after the column.
    for r in db
        .rows(&format!(
            "select c.oid, format('%I.%I', n.nspname, c.relname) as name, format_type(s.seqtypid, null) as type, s.seqstart as start, s.seqincrement as inc,
              s.seqmin as min, s.seqmax as max, s.seqcache as cache, s.seqcycle as cycle, {} as acl,
              (select format('%I.%I.%I', tn.nspname, t.relname, a.attname) from pg_depend d join pg_class t on t.oid = d.refobjid join pg_namespace tn on tn.oid = t.relnamespace
                 join pg_attribute a on a.attrelid = d.refobjid and a.attnum = d.refobjsubid where d.classid = 'pg_class'::regclass and d.objid = c.oid and d.deptype = 'a') as owned,
              (select d.refobjid from pg_depend d where d.classid = 'pg_class'::regclass and d.objid = c.oid and d.deptype = 'a' and d.refobjsubid > 0) as owned_rel,
              (select d.refobjsubid from pg_depend d where d.classid = 'pg_class'::regclass and d.objid = c.oid and d.deptype = 'a' and d.refobjsubid > 0) as owned_att,
              c.relpersistence = 'u' as unlogged
            from pg_sequence s join pg_class c on c.oid = s.seqrelid join pg_namespace n on n.oid = c.relnamespace
            where {ns} and {} and {}
              and not exists (select 1 from pg_depend d where d.classid = 'pg_class'::regclass and d.objid = c.oid and d.deptype = 'i')",
            acl_in("c.relacl", 's', "c.relowner", Some("c.relnamespace")),
            not_extension("pg_class", "c"),
            not_platform("c.relowner")
        ))
        .await?
    {
        let name = r.text("name").to_string();
        let id = format!("sequence {name}");
        ids.class.insert(oid(&r, "oid"), id.clone());
        let options = format!(
            "as {} increment by {} minvalue {} maxvalue {} start with {} cache {}{}",
            r.text("type"),
            r.text("inc"),
            r.text("min"),
            r.text("max"),
            r.text("start"),
            r.text("cache"),
            if r.flag("cycle") { " cycle" } else { " no cycle" }
        );
        let goes_with = r.get("owned").map(|o| split_last(o)).map(|(t, c)| format!("column {t}.{c}"));
        s.add(Object {
            goes_with,
            id: id.clone(),
            kind: Some(Kind::Sequence),
            def: options.clone(),
            create: vec![format!("create sequence {name} {options}")],
            drop: vec![format!("drop sequence {name}")],
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
        grants(&mut s, r.text("acl"), &name, &id, "sequence");
        // An unlogged sequence is set so after it is made, and logged again where the target's isn't.
        if r.flag("unlogged") {
            s.add(Object {
                id: format!("unlogged {id}"),
                kind: Some(Kind::ColumnSetting),
                owner: Some(id.clone()),
                def: "unlogged".into(),
                create: vec![format!("alter sequence {name} set unlogged")],
                drop: vec![format!("alter sequence {name} set logged")],
                change: Change::Alter { blocks: false },
                ..Object::default()
            });
        }
        if let Some(owned) = r.get("owned") {
            let column = { let (t, c) = split_last(owned); format!("column {t}.{c}") };
            s.add(Object {
                id: format!("owner {name}"),
                kind: Some(Kind::SequenceOwner),
                owner: Some(id),
                def: owned.to_string(),
                create: vec![format!("alter sequence {name} owned by {owned}")],
                drop: vec![format!("alter sequence {name} owned by none")],
                deps: [column].into_iter().collect(),
                change: Change::Replace,
                ..Object::default()
            });
            let _ = (r.get("owned_rel"), r.get("owned_att"));
        }
    }

    // Relations: tables, partitioned tables, views, materialized views, foreign tables, composite types.
    let mut relations: BTreeMap<u32, (String, String)> = BTreeMap::new();
    let rels = db
        .rows(&format!(
            "select c.oid, c.reltype, format('%I.%I', n.nspname, c.relname) as name, c.relkind as kind, c.relpersistence as persistence,
              c.relrowsecurity as rls, c.relforcerowsecurity as force,
              (select string_agg(o, ', ' order by o) from (select unnest(c.reloptions) as o union all select 'toast.' || unnest(tc.reloptions) from pg_class tc where tc.oid = c.reltoastrelid) x) as options,
              (select nullif(a.amname, 'heap') from pg_am a where a.oid = c.relam) as am,
              pg_get_expr(c.relpartbound, c.oid) as bound,
              (select string_agg(format('%I.%I', pn.nspname, pc.relname), ', ' order by i.inhseqno) from pg_inherits i join pg_class pc on pc.oid = i.inhparent join pg_namespace pn on pn.oid = pc.relnamespace where i.inhrelid = c.oid) as parent,
              case when c.relkind = 'p' then pg_get_partkeydef(c.oid) end as partkey,
              (select format('%I.%I', tn.nspname, ty.typname) from pg_type ty join pg_namespace tn on tn.oid = ty.typnamespace where ty.oid = c.reloftype) as of_type,
              c.relispartition and exists (select 1 from pg_constraint fk where fk.contype = 'f' and fk.confrelid in (select a.relid from pg_partition_ancestors(c.oid) a where a.relid <> c.oid)) as referenced,
              case when c.relkind in ('v', 'm') then pg_get_viewdef(c.oid, false) end as view,
              {} as acl,
              (select {} from pg_type t where t.oid = c.reltype) as type_acl,
              obj_description(c.oid, 'pg_class') as comment,
              case when c.relkind in ('r', 'p') then case c.relreplident when 'f' then 'full' when 'n' then 'nothing'
                when 'i' then (select format('using index %I', ic.relname) from pg_index i join pg_class ic on ic.oid = i.indexrelid where i.indrelid = c.oid and i.indisreplident) end end as replica,
              (select i.indexrelid from pg_index i where i.indrelid = c.oid and i.indisreplident) as replica_index
            from pg_class c join pg_namespace n on n.oid = c.relnamespace
            where c.relkind in ('r', 'p', 'v', 'm', 'f', 'c') and {ns} and {} and {} and c.relname <> '{history}'
              and not exists (select 1 from pg_depend e where e.classid = 'pg_type'::regclass and e.objid = c.reltype and e.deptype = 'e')",
            acl_in("c.relacl", 'r', "c.relowner", Some("c.relnamespace")),
            acl_in("t.typacl", 'T', "t.typowner", Some("t.typnamespace")),
            not_extension("pg_class", "c"),
            not_platform("c.relowner")
        ))
        .await?;
    // A foreign table's server and options.
    let mut servers_of: BTreeMap<u32, String> = BTreeMap::new();
    for r in db
        .rows("select ft.ftrelid, format('%I', fs.srvname) as server, (select string_agg(format('%I %L', split_part(o, '=', 1), substr(o, strpos(o, '=') + 1)), ', ') from unnest(ft.ftoptions) o) as options from pg_foreign_table ft join pg_foreign_server fs on fs.oid = ft.ftserver")
        .await?
    {
        servers_of.insert(oid(&r, "ftrelid"), format!(" server {}{}", r.text("server"), r.get("options").map(|o| format!(" options ({o})")).unwrap_or_default()));
    }
    for r in &rels {
        let name = r.text("name").to_string();
        let kind = r.text("kind");
        let id = match kind {
            "v" | "m" => format!("view {name}"),
            "c" => format!("type {name}"),
            _ => format!("table {name}"),
        };
        ids.class.insert(oid(r, "oid"), id.clone());
        ids.types.insert(oid(r, "reltype"), id.clone());
        relations.insert(oid(r, "oid"), (id, kind.to_string()));
    }

    // The composite types tables are made of, whose attributes change on those tables too.
    let typed_by: BTreeSet<String> = rels.iter().filter_map(|r| r.get("of_type").map(String::from)).collect();

    // Constraints of tables (not the not-null ones Postgres 18 records, which the columns carry, and
    // not those a partition takes from its parent), and the indexes they own.
    let mut constraint_indexes: BTreeMap<u32, String> = BTreeMap::new();
    let mut inline_constraints: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let mut keyed: BTreeSet<(u32, String)> = BTreeSet::new();
    let partitions: BTreeSet<u32> = rels.iter().filter(|r| r.get("bound").is_some() || r.text("kind") == "c").map(|r| oid(r, "oid")).collect();
    for r in db
        .rows(&format!(
            "select co.oid, co.conindid, format('%I.%I', n.nspname, c.relname) as rel, c.oid as reloid, format('%I', co.conname) as name,
              pg_get_constraintdef(co.oid, false) as def, co.condeferrable as deferrable, co.condeferred as deferred, co.convalidated as validated, co.contype as type,
              case when co.contype = 'f' then
                (select string_agg(format_type(a.atttypid, null), ', ' order by k.n) from unnest(co.conkey) with ordinality k(num, n) join pg_attribute a on a.attrelid = co.conrelid and a.attnum = k.num)
                || ' to ' || (select string_agg(format_type(a.atttypid, null), ', ' order by k.n) from unnest(co.confkey) with ordinality k(num, n) join pg_attribute a on a.attrelid = co.confrelid and a.attnum = k.num)
              end as linked,
              case when co.contype in ('p', 'u') then (select string_agg(o, ', ' order by o) from pg_class ic, unnest(ic.reloptions) o where ic.oid = co.conindid) end as index_options,
              case when co.contype in ('p', 'u', 'x') then (select format('%I.%I', inn.nspname, ic.relname) from pg_class ic join pg_namespace inn on inn.oid = ic.relnamespace where ic.oid = co.conindid) end as index_name,
              c.relkind = 'p' and co.contype in ('p', 'u', 'x') and not coalesce((select i.indisvalid from pg_index i where i.indexrelid = co.conindid), true) as alone
            from pg_constraint co join pg_class c on c.oid = co.conrelid join pg_namespace n on n.oid = c.relnamespace
            where co.contype not in ('n', 't') and co.conislocal and {ns} and c.relname <> '{history}' and {} and {}
            order by co.contype, co.conname",
            not_platform("c.relowner"),
            not_extension("pg_constraint", "co")
        ))
        .await?
    {
        let Some((table, _)) = relations.get(&oid(&r, "reloid")) else { continue };
        let (rel, name) = (r.text("rel"), r.text("name"));
        let id = format!("constraint {rel}.{name}");
        ids.constraints.insert(oid(&r, "oid"), id.clone());
        if oid(&r, "conindid") != 0 && matches!(r.text("type"), "p" | "u" | "x") {
            ids.class.insert(oid(&r, "conindid"), id.clone());
            constraint_indexes.insert(oid(&r, "conindid"), id.clone());
        }
        // The server's rendering says DEFERRABLE and NOT VALID itself, but not its index's storage
        // parameters, which are written after its columns.
        let def = with_index_options(r.text("def"), r.get("index_options"));
        // A new table's own constraints are written inside its create table; a foreign key, which
        // names another table, is a statement of its own after both exist.
        // A create table makes one index for keys alike (a primary key and a unique constraint on
        // its columns, or two unique ones), keeping the first: the second is added after it.
        let keys = |d: &str| d.trim_start_matches("PRIMARY KEY").trim_start_matches("UNIQUE").trim().to_string();
        let alike = matches!(r.text("type"), "p" | "u") && keyed.contains(&(oid(&r, "reloid"), keys(&def)));
        if matches!(r.text("type"), "p" | "u") {
            keyed.insert((oid(&r, "reloid"), keys(&def)));
        }
        // A key added to a partitioned table alone (`alter table only`), its index not yet valid:
        // added so again, its partitions' keys left as they are.
        let alone = r.flag("alone");
        let inline = r.text("type") != "f" && r.flag("validated") && !partitions.contains(&oid(&r, "reloid")) && !alike && !alone;
        if inline {
            inline_constraints.entry(oid(&r, "reloid")).or_default().push(format!("constraint {name} {def}"));
        }
        // A foreign key holds while the types of the columns it links compare; a change of either
        // side's type makes it again around the change, so that both columns can change between.
        let linked = r.get("linked").map(|l| format!("\n-- links {l}")).unwrap_or_default();
        // A key's index storage parameters change in place on its index; anything else about it
        // makes it again.
        let (change, extra) = match r.get("index_name") {
            Some(index) => (Change::Alter { blocks: false }, vec![format!("{INDEX_OF}{index}"), format!("{INDEX_OPTIONS}{}", r.text("index_options"))]),
            None => (Change::Replace, vec![]),
        };
        s.add(Object {
            inline,
            id,
            kind: Some(Kind::Constraint),
            owner: Some(table.clone()),
            def: format!("{def}{linked}{}", if alone { "\n(on its table alone)" } else { "" }),
            create: vec![format!("alter table {}{rel} add constraint {name} {def}", if alone { "only " } else { "" })],
            drop: vec![format!("alter table {rel} drop constraint {name}")],
            change,
            extra,
            ..Object::default()
        });
    }

    // Columns, by relation.
    let mut columns: BTreeMap<u32, Vec<Row>> = BTreeMap::new();
    for r in db
        .rows(&format!(
            "select a.attrelid as rel, a.attnum as num, format('%I', a.attname) as name, format_type(a.atttypid, a.atttypmod) as type, a.attnotnull as notnull,
              pg_get_expr(d.adbin, d.adrelid) as def, d.oid as defoid, a.attidentity as identity, {} as identity_options, a.attgenerated as generated, a.attislocal as local, a.attinhcount > 0 as inherited,
              (select format('%I.%I', sn.nspname, sc.relname) from pg_depend sd join pg_class sc on sc.oid = sd.objid join pg_namespace sn on sn.oid = sc.relnamespace
                where sd.classid = 'pg_class'::regclass and sd.refclassid = 'pg_class'::regclass and sd.refobjid = a.attrelid and sd.refobjsubid = a.attnum and sd.deptype = 'i'
                  and (sc.relnamespace <> c.relnamespace or sc.relname <> left(c.relname || '_' || a.attname || '_seq', 63))) as identity_name,
              case when a.attcollation <> t.typcollation then (select format('%I.%I', cn.nspname, co.collname) from pg_collation co join pg_namespace cn on cn.oid = co.collnamespace where co.oid = a.attcollation) end as collation,
              col_description(a.attrelid, a.attnum) as comment, {} as nn_valid, {} as acl,
              case when a.attstorage <> t.typstorage then case a.attstorage when 'p' then 'plain' when 'e' then 'external' when 'm' then 'main' else 'extended' end end as storage,
              case t.typstorage when 'p' then 'plain' when 'e' then 'external' when 'm' then 'main' else 'extended' end as type_storage,
              case a.attcompression when 'p' then 'pglz' when 'l' then 'lz4' end as compression,
              case when coalesce(a.attstattarget, -1) >= 0 then a.attstattarget end as stattarget,
              (select string_agg(o, ', ' order by o) from unnest(a.attoptions) o where left(o, 10) = 'n_distinct') as attoptions
            from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace join pg_type t on t.oid = a.atttypid
            left join pg_attrdef d on d.adrelid = a.attrelid and d.adnum = a.attnum
            where a.attnum > 0 and not a.attisdropped and c.relkind in ('r', 'p', 'v', 'm', 'f', 'c') and {ns} and {} and {} and c.relname <> '{history}'
            order by a.attrelid, a.attnum",
            // Postgres 18 records a column's NOT NULL as a constraint, which may be kept from the
            // table's children (NO INHERIT).
            crate::catalog::identity_options("a"),
            if version >= 180000 {
                "coalesce((select co.connoinherit from pg_constraint co where co.conrelid = a.attrelid and co.contype = 'n' and co.conkey = array[a.attnum]), false) as noinherit,
                  (select format('%I', co.conname) from pg_constraint co where co.conrelid = a.attrelid and co.contype = 'n' and co.conkey = array[a.attnum]) as nn_name,
                  coalesce((select co.convalidated from pg_constraint co where co.conrelid = a.attrelid and co.contype = 'n' and co.conkey = array[a.attnum]), true)"
            } else {
                "false as noinherit, null as nn_name, true"
            },
            acl("a.attacl", 'c', "c.relowner"),
            not_extension("pg_class", "c"),
            not_platform("c.relowner")
        ))
        .await?
    {
        columns.entry(oid(&r, "rel")).or_default().push(r);
    }

    // Each table's columns' defaults and not nulls, by name: a partition's that differ are its own.
    let names: BTreeMap<u32, String> = rels.iter().map(|r| (oid(r, "oid"), r.text("name").to_string())).collect();
    let mut column_states: BTreeMap<(String, String), (Option<String>, bool)> = BTreeMap::new();
    for (rel, cols) in &columns {
        for c in cols {
            column_states.insert((names.get(rel).cloned().unwrap_or_default(), c.text("name").to_string()), (c.get("def").filter(|_| c.text("generated").is_empty()).map(String::from), c.flag("notnull")));
        }
    }
    for r in &rels {
        let rel = oid(r, "oid");
        let name = r.text("name").to_string();
        let kind = r.text("kind");
        let id = relations[&rel].0.clone();
        let cols = columns.remove(&rel).unwrap_or_default();
        let partition = r.get("bound").is_some();
        let mut col_defs = Vec::new();
        let mut detach: Option<String> = None;
        // A composite type's attributes in order, each its name and definition.
        let mut members: Vec<(String, String)> = Vec::new();
        // What a partition says of its columns where its parent says otherwise.
        let mut overrides = Vec::new();
        let mut later: Vec<String> = Vec::new();
        // The columns of a name the table's parents have, one per parent that has it.
        let parents = r.get("parent").map(names_in).unwrap_or_default();
        // A composite type a table is of changes its attributes there too.
        let cascade = if kind == "c" && typed_by.contains(&name) { " cascade" } else { "" };
        let holders = |column: &str| -> BTreeSet<String> {
            let mut held: BTreeSet<String> = parents.iter().filter(|p| column_states.contains_key(&((*p).clone(), column.to_string()))).map(|p| format!("column {p}.{column}")).collect();
            if held.is_empty() {
                held.extend(parents.first().map(|p| format!("column {p}.{column}")));
            }
            held
        };
        for c in &cols {
            let col_id = format!("column {name}.{}", c.text("name"));
            let target = if matches!(kind, "v" | "m") { id.clone() } else { col_id.clone() };
            ids.column.insert((rel, c.text("num").parse().unwrap_or(0)), target.clone());
            if let Some(d) = c.get("defoid") {
                ids.attrdefs.insert(d.parse().unwrap_or(0), col_id.clone());
            }
            if let (Some(comment), true) = (c.get("comment"), true) {
                let on = if kind == "c" { format!("column {name}.{}", c.text("name")) } else { format!("column {name}.{}", c.text("name")) };
                s.add(Object {
                    id: format!("comment {col_id}"),
                    kind: Some(Kind::Comment),
                    owner: Some(if matches!(kind, "v" | "m") { id.clone() } else { col_id.clone() }),
                    def: comment.to_string(),
                    create: vec![format!("comment on {on} is {}", literal(comment))],
                    drop: vec![format!("comment on {on} is null")],
                    change: Change::Alter { blocks: false },
                    ..Object::default()
                });
            }
            if kind != "c" {
                column_grants(&mut s, c.text("acl"), &name, c.text("name"), &target);
            }
            // How the column's values are kept and how closely the planner samples them, where not
            // its type's defaults.
            if matches!(kind, "r" | "p" | "m" | "f") {
                let at = format!("alter {} {name} alter column {}", match kind { "m" => "materialized view", "f" => "foreign table", _ => "table" }, c.text("name"));
                // Its attribute options Postgres documents (`n_distinct`, `n_distinct_inherited`), set
                // as a list and reset by their names; a server may keep others of its own.
                if let Some(options) = c.get("attoptions").filter(|o| !o.is_empty()) {
                    let keys: Vec<&str> = options.split(", ").filter_map(|o| o.split('=').next()).collect();
                    s.add(Object {
                        id: format!("options {col_id}"),
                        kind: Some(Kind::ColumnSetting),
                        owner: Some(if matches!(kind, "m") { id.clone() } else { col_id.clone() }),
                        def: options.to_string(),
                        create: vec![format!("{at} set ({options})")],
                        drop: vec![format!("{at} reset ({})", keys.join(", "))],
                        change: Change::Replace,
                        ..Object::default()
                    });
                }
                for (what, value, default) in [("storage", c.get("storage"), c.text("type_storage").to_string()), ("compression", c.get("compression"), "default".into()), ("statistics", c.get("stattarget"), "-1".into())] {
                    let Some(value) = value else { continue };
                    s.add(Object {
                        id: format!("{what} {col_id}"),
                        kind: Some(Kind::ColumnSetting),
                        owner: Some(if matches!(kind, "m") { id.clone() } else { col_id.clone() }),
                        def: value.to_string(),
                        create: vec![format!("{at} set {what} {value}")],
                        drop: vec![format!("{at} set {what} {default}")],
                        change: Change::Alter { blocks: false },
                        ..Object::default()
                    });
                }
            }
            // A view's column default, which an insert through the view takes: set on the view, and
            // what it calls is what the default needs.
            if let (Some(default), "v") = (c.get("def"), kind) {
                let at = format!("alter view {name} alter column {}", c.text("name"));
                let default_id = format!("default {col_id}");
                if let Some(d) = c.get("defoid") {
                    ids.attrdefs.insert(d.parse().unwrap_or(0), default_id.clone());
                }
                s.add(Object {
                    id: default_id,
                    kind: Some(Kind::ColumnSetting),
                    owner: Some(id.clone()),
                    def: default.to_string(),
                    create: vec![format!("{at} set default {default}")],
                    drop: vec![format!("{at} drop default")],
                    change: Change::Alter { blocks: false },
                    ..Object::default()
                });
            }
            if matches!(kind, "v" | "m") {
                continue;
            }
            // A not null left unvalidated (Postgres 18), which no column definition can write: the
            // column is made without it, and it is added after, by its name, not valid.
            let unvalidated = c.flag("notnull") && !c.flag("nn_valid") && !c.flag("noinherit");
            let full = column_def(c, kind == "c");
            // An identity's sequence named otherwise than Postgres would name it is made under its
            // name; which name it has is no part of the definition compared.
            let named = c.get("identity_name").map(|n| named_identity(&full, n)).unwrap_or_else(|| full.clone());
            let made = if unvalidated { named.trim_end_matches(" not null").to_string() } else { named.clone() };
            let add_not_null = format!("alter table {name} add constraint {} not null {} not valid", c.text("nn_name"), c.text("name"));
            if unvalidated {
                later.push(add_not_null.clone());
            }
            // A typed table's columns are its type's attributes, made, changed and dropped through
            // the type; a default or a not null of one is the table's own, written `with options`.
            if let Some(ty) = r.get("of_type") {
                let own = made.strip_prefix(c.text("type")).unwrap_or_default().trim();
                if !own.is_empty() {
                    col_defs.push(format!("{} with options {own}", c.text("name")));
                }
                let at = format!("alter table {name} alter column {}", c.text("name"));
                s.add(Object {
                    id: col_id,
                    kind: Some(Kind::Column),
                    owner: Some(id.clone()),
                    def: if unvalidated { format!("{full} not valid") } else { full.clone() },
                    // Where the type gains the attribute under the table, what the table says of it.
                    create: [c.get("def").filter(|_| c.text("generated").is_empty() && c.text("identity").is_empty()).map(|d| format!("{at} set default {d}")), c.flag("notnull").then(|| format!("{at} set not null"))].into_iter().flatten().collect(),
                    inline: true,
                    change: Change::Alter { blocks: true },
                    extra: vec![TYPED.to_string()],
                    deps: [format!("column {ty}.{}", c.text("name"))].into_iter().collect(),
                    ..Object::default()
                });
                continue;
            }
            // A column a table inherits is no part of its statement unless the table declares it
            // too: a partition's, or an inheriting table's, comes from its parent.
            if c.flag("local") {
                col_defs.push(format!("{} {made}", c.text("name")));
                members.push((c.text("name").to_string(), made.clone()));
            }
            let def = if unvalidated { format!("{full} not valid") } else { full.clone() };
            if c.flag("local") {
                let caution = (c.flag("notnull") && c.get("def").is_none() && c.text("identity").is_empty() && kind != "c")
                    .then(|| format!("{} is not null with no default, so adding it fails on a table that has rows: give it a default, or add it nullable, fill it, then set not null", c.text("name")));
                s.add(Object {
                    caution,
                    id: col_id,
                    kind: Some(Kind::Column),
                    owner: Some(id.clone()),
                    def: def.clone(),
                    create: [Some(if kind == "c" { format!("alter type {name} add attribute {} {made}{cascade}", c.text("name")) } else { format!("alter table {name} add column {} {made}", c.text("name")) }), unvalidated.then(|| add_not_null.clone())].into_iter().flatten().collect(),
                    extra: c
                        .get("nn_name")
                        .map(|n| format!("not null name {n}"))
                        .into_iter()
                        .chain(c.get("identity_name").map(|n| format!("{IDENTITY_SEQUENCE}{n}")))
                        .chain((kind == "p").then(|| "partitioned".to_string()))
                        .chain((!c.text("generated").is_empty()).then(|| super::GENERATED.to_string()))
                        .chain((c.flag("inherited") && !partition).then(|| "inherited".to_string()))
                        .chain((kind == "f").then(|| "foreign".to_string()))
                        .chain((!cascade.is_empty()).then(|| CASCADE.to_string()))
                        .collect(),
                    // A column a parent has too changes type with the parent's, before its own change.
                    deps: if c.flag("inherited") && !partition { holders(c.text("name")) } else { BTreeSet::new() },
                    drop: vec![if kind == "c" { format!("alter type {name} drop attribute {}{cascade}", c.text("name")) } else { format!("alter table {name} drop column {}", c.text("name")) }],
                    inline: true,
                    change: Change::Alter { blocks: true },
                    ..Object::default()
                });
            } else if r.get("parent").is_some() && !partition {
                // A column a table only inherits is its parents', made, changed and dropped with
                // them, so that a table that starts or stops inheriting keeps the column and its
                // values, but for a default or a not null of its own. Its statements are for a table
                // that starts or stops inheriting it: the table has it before it takes a parent that
                // has it too, and no longer once it leaves that parent, where the parent's own
                // change hasn't made or dropped it already.
                s.add(Object {
                    id: col_id,
                    kind: Some(Kind::Column),
                    owner: Some(id.clone()),
                    def: def.clone(),
                    create: vec![format!("alter table {name} add column if not exists {} {made}", c.text("name"))],
                    drop: vec![format!("alter table {name} drop column if exists {}", c.text("name"))],
                    inline: true,
                    change: Change::Alter { blocks: true },
                    extra: vec!["inherited".to_string()],
                    deps: holders(c.text("name")),
                    ..Object::default()
                });
            } else if partition {
                // A partition's column is its parent's, made, changed and dropped with it: an
                // object with no statement of its own, so that a partition detached keeps it, but
                // for a default or a not null of its own.
                let parent = r.get("parent").and_then(|p| column_states.get(&(p.to_string(), c.text("name").to_string())));
                let mine = (c.get("def").filter(|_| c.text("generated").is_empty()).map(String::from), c.flag("notnull"));
                if let Some((default, not_null)) = parent {
                    let mut said = Vec::new();
                    if mine.0 != *default {
                        match &mine.0 {
                            Some(d) => said.push(format!("default {d}")),
                            // A default the parent has and the partition doesn't is dropped after it is made.
                            None => later.push(format!("alter table {name} alter column {} drop default", c.text("name"))),
                        }
                    }
                    if mine.1 && !not_null {
                        said.push("not null".to_string());
                    }
                    if !said.is_empty() {
                        overrides.push(format!("{} {}", c.text("name"), said.join(" ")));
                    }
                }
                s.add(Object {
                    id: col_id,
                    kind: Some(Kind::Column),
                    owner: Some(id.clone()),
                    def: def.clone(),
                    inline: true,
                    change: Change::Alter { blocks: true },
                    extra: std::iter::once("partition".to_string()).chain((kind == "p").then(|| "partitioned".to_string())).chain(c.get("nn_name").map(|n| format!("not null name {n}"))).collect(),
                    deps: r.get("parent").map(|p| format!("column {p}.{}", c.text("name"))).into_iter().collect(),
                    ..Object::default()
                });
            }
        }
        col_defs.extend(inline_constraints.remove(&rel).unwrap_or_default());
        let options = r.get("options").map(|o| format!(" with ({o})")).unwrap_or_default();
        // The storage, where it is not the heap: `using columnar` and the like.
        let using = r.get("am").map(|am| format!(" using {am}")).unwrap_or_default();
        let mut extra = Vec::new();
        let (def, create, drop) = match kind {
            "v" => {
                let body = r.text("view").trim().trim_end_matches(';').to_string();
                (format!("view{options} as {body}"), format!("create view {name}{options} as\n{body}"), format!("drop view {name}"))
            }
            "m" => {
                let body = r.text("view").trim().trim_end_matches(';').to_string();
                (format!("materialized view{using}{options} as {body}"), format!("create materialized view {name}{using}{options} as\n{body}"), format!("drop materialized view {name}"))
            }
            // Its attributes' order, which an attribute added goes to the end of.
            "c" => {
                extra.extend(members.iter().map(|(n, d)| format!("{MEMBER}{n}{SEP}{d}")));
                (format!("composite ({})", members.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")), format!("create type {name} as (\n  {}\n)", col_defs.join(",\n  ")), format!("drop type {name}"))
            }
            _ => {
                let unlogged = if r.text("persistence") == "u" { "unlogged " } else { "" };
                let foreign = if kind == "f" { "foreign " } else { "" };
                let head = format!("create {unlogged}{foreign}table {name}");
                let body = match (r.get("bound"), r.get("parent")) {
                    _ if r.get("of_type").is_some() && col_defs.is_empty() => format!("{head} of {}", r.text("of_type")),
                    _ if r.get("of_type").is_some() => format!("{head} of {} (\n  {}\n)", r.text("of_type"), col_defs.join(",\n  ")),
                    (Some(bound), Some(parent)) if overrides.is_empty() => format!("{head} partition of {parent} {bound}"),
                    (Some(bound), Some(parent)) => format!("{head} partition of {parent} (\n  {}\n) {bound}", overrides.join(",\n  ")),
                    (None, Some(parent)) => format!("{head} (\n  {}\n) inherits ({parent})", col_defs.join(",\n  ")),
                    _ => format!("{head} (\n  {}\n)", col_defs.join(",\n  ")),
                };
                let partkey = r.get("partkey").map(|k| format!(" partition by {k}")).unwrap_or_default();
                let server = servers_of.get(&rel).cloned().unwrap_or_default();
                // Its parts, which its alterer changes one by one.
                extra.push(format!("shape {foreign}table{partkey}{server}"));
                extra.push(format!("persistence {}", if unlogged.is_empty() { "logged" } else { "unlogged" }));
                extra.push(format!("options {}", r.text("options")));
                extra.push(format!("access method {}", r.get("am").unwrap_or("heap")));
                match (r.get("bound"), r.get("parent")) {
                    (Some(bound), Some(parent)) => extra.push(format!("partition of {parent} {bound}")),
                    (None, Some(parent)) => extra.push(format!("inherits {parent}")),
                    _ => {}
                }
                if let Some(ty) = r.get("of_type") {
                    extra.push(format!("of {ty}"));
                }
                // A partition of a table a foreign key references leaves it first: the key's own
                // part on the partition stops its drop.
                if let Some(parent) = r.get("parent").filter(|_| r.flag("referenced")) {
                    detach = Some(format!("alter table {parent} detach partition {name}"));
                }
                let drop = format!("drop {foreign}table {name}");
                (extra.join("\n"), format!("{body}{partkey}{using}{server}{options}"), drop)
            }
        };
        let object_kind = match kind {
            "v" | "m" => Kind::View,
            "c" => Kind::Type,
            _ => Kind::Table,
        };
        // A table's own properties change in place, and so does a composite type's order of
        // attributes; a view is replaced.
        let change = if object_kind == Kind::View { Change::Replace } else { Change::Alter { blocks: false } };
        let mut create = vec![create];
        create.extend(later);
        let drop = detach.take().into_iter().chain([drop]).collect();
        s.add(Object { id: id.clone(), kind: Some(object_kind), def, create, drop, change, extra, ..Object::default() });
        // Row-level security, enabled, forced on the table's owner, or both: each is set on its own.
        if r.flag("rls") || r.flag("force") {
            s.add(Object {
                id: format!("rls {name}"),
                kind: Some(Kind::Rls),
                owner: Some(id.clone()),
                def: [r.flag("rls").then_some("enable"), r.flag("force").then_some("force")].into_iter().flatten().collect::<Vec<_>>().join(" "),
                create: [r.flag("rls").then(|| format!("alter table {name} enable row level security")), r.flag("force").then(|| format!("alter table {name} force row level security"))].into_iter().flatten().collect(),
                drop: [r.flag("rls").then(|| format!("alter table {name} disable row level security")), r.flag("force").then(|| format!("alter table {name} no force row level security"))].into_iter().flatten().collect(),
                change: Change::Alter { blocks: false },
                ..Object::default()
            });
        }
        // A composite type's privileges are its type's.
        if kind == "c" {
            grants(&mut s, r.text("type_acl"), &name, &id, "type");
        } else {
            grants(&mut s, r.text("acl"), &name, &id, "table");
        }
        if let Some(comment) = r.get("comment") {
            let on = table_word(kind);
            s.add(Object {
                id: format!("comment {id}"),
                kind: Some(Kind::Comment),
                owner: Some(id.clone()),
                def: comment.to_string(),
                create: vec![format!("comment on {on} {name} is {}", literal(comment))],
                drop: vec![format!("comment on {on} {name} is null")],
                change: Change::Alter { blocks: false },
                ..Object::default()
            });
        }
    }

    // Grants on an identity column's sequence, which goes with the column.
    for r in db
        .rows(&format!(
            "select format('%I.%I', n.nspname, q.relname) as name, d.refobjid as rel, d.refobjsubid as att, {} as acl
            from pg_class q join pg_namespace n on n.oid = q.relnamespace
              join pg_depend d on d.classid = 'pg_class'::regclass and d.objid = q.oid and d.deptype = 'i' and d.refclassid = 'pg_class'::regclass and d.refobjsubid > 0
            where q.relkind = 'S' and {ns}",
            acl_in("q.relacl", 's', "q.relowner", Some("q.relnamespace"))
        ))
        .await?
    {
        let Some(column) = ids.column.get(&(oid(&r, "rel"), r.text("att").parse().unwrap_or(0))).cloned() else { continue };
        grants(&mut s, r.text("acl"), r.text("name"), &column, "sequence");
    }

    let indexes = db
        .rows(&format!(
            "select i.indexrelid as oid, i.indrelid as rel, format('%I.%I', n.nspname, c.relname) as name, pg_get_indexdef(i.indexrelid) as def,
              (select h.inhparent from pg_inherits h where h.inhrelid = i.indexrelid) as parent,
              exists (select 1 from pg_constraint co where co.conindid = i.indexrelid) as keyed,
              exists (select 1 from pg_inherits h where h.inhparent = i.indexrelid) as attached,
              (select pi.indisvalid from pg_inherits h join pg_index pi on pi.indexrelid = h.inhparent where h.inhrelid = i.indexrelid) as parent_valid,
              i.indisvalid as valid, t.relkind = 'p' as partitioned, i.indisclustered as clustered, format('%I.%I', tn.nspname, t.relname) as table_name,
              (select string_agg(a.attnum || '=' || a.attstattarget, ',' order by a.attnum) from pg_attribute a where a.attrelid = i.indexrelid and coalesce(a.attstattarget, -1) >= 0) as stattargets
            from pg_index i join pg_class c on c.oid = i.indexrelid join pg_namespace n on n.oid = c.relnamespace join pg_class t on t.oid = i.indrelid join pg_namespace tn on tn.oid = t.relnamespace
            where {ns} and {} and {} and t.relname <> '{history}'",
            not_extension("pg_class", "c"),
            not_platform("t.relowner")
        ))
        .await?;
    // A partitioned table's index that every partition's index is attached to is made on the table
    // itself, after those, which it attaches as it is made, keeping their names; one that some
    // partition has none for is made on the table alone (`on only`), and each partition's index is
    // attached to it after. A partition's index goes with its table's, and a partition made by the
    // plan takes its table's indexes with it.
    // A parent whose partition holds another index like the one attached to it is made on its
    // table alone, and that one attached by name: made on the table, it would attach either.
    // Postgres matches them without their names, storage options, sort order or nulls placement.
    static LOOSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" WITH \([^)]*\)| DESC\b| ASC\b| NULLS (FIRST|LAST)\b").expect("a valid pattern"));
    let unnamed = |r: &Row| {
        let bare = super::last_written(r.text("name"));
        LOOSE.replace_all(&r.text("def").replacen(&format!(" INDEX {bare} ON "), " INDEX ON ", 1), "").into_owned()
    };
    let mut by_name_only: BTreeSet<String> = BTreeSet::new();
    for r in indexes.iter().filter(|r| r.get("parent").is_some()) {
        if indexes.iter().any(|o| o.text("rel") == r.text("rel") && o.text("oid") != r.text("oid") && unnamed(o) == unnamed(r)) {
            by_name_only.insert(r.text("parent").to_string());
        }
    }
    let valid = |index: &str| indexes.iter().find(|r| r.text("oid") == index).is_some_and(|r| r.flag("valid")) && !by_name_only.contains(index);
    // Each index after the one it is attached to, at any depth of partitions, which it names.
    let depth = |r: &Row| -> usize {
        let mut depth = 0;
        let mut at = r.get("parent").map(String::from);
        while let Some(parent) = at.filter(|_| depth < 64) {
            depth += 1;
            at = indexes.iter().find(|i| i.text("oid") == parent).and_then(|i| i.get("parent")).map(String::from);
        }
        depth
    };
    let mut ordered: Vec<&Row> = indexes.iter().collect();
    ordered.sort_by_key(|r| depth(r));
    {
        for r in ordered {
            let index = oid(r, "oid");
            let child = r.get("parent").is_some();
            if constraint_indexes.contains_key(&index) || (child && r.flag("keyed")) {
                continue;
            }
            let Some((table, _)) = relations.get(&oid(r, "rel")) else { continue };
            let id = format!("index {}", r.text("name"));
            ids.class.insert(index, id.clone());
            let def = r.text("def").to_string();

            // The index a table is clustered on, which goes and comes back with it.
            if r.flag("clustered") {
                let (t, bare) = (r.text("table_name"), super::last_written(r.text("name")));
                s.add(Object {
                    id: format!("cluster {table}"),
                    kind: Some(Kind::ColumnSetting),
                    owner: Some(id.clone()),
                    def: bare.clone(),
                    create: vec![format!("alter table {t} cluster on {bare}")],
                    drop: vec![format!("alter table {t} set without cluster")],
                    change: Change::Alter { blocks: false },
                    ..Object::default()
                });
            }
            // How closely the planner samples an expression column of it, where set.
            for target in r.text("stattargets").split(',').filter(|t| !t.is_empty()) {
                let Some((column, value)) = target.split_once('=') else { continue };
                let at = format!("alter index {} alter column {column} set statistics", r.text("name"));
                s.add(Object {
                    id: format!("statistics {id} column {column}"),
                    kind: Some(Kind::ColumnSetting),
                    owner: Some(id.clone()),
                    def: value.to_string(),
                    create: vec![format!("{at} {value}")],
                    drop: vec![format!("{at} -1")],
                    change: Change::Alter { blocks: false },
                    ..Object::default()
                });
            }
            // Changed by its alterer where it can be (attached or detached in place), else made again.
            let mut o = Object { id: id.clone(), kind: Some(Kind::Index), owner: Some(table.clone()), def: def.clone(), create: vec![def.clone()], drop: vec![format!("drop index {}", r.text("name"))], change: Change::Alter { blocks: false }, ..Object::default() };
            if r.flag("attached") {
                let made = if valid(r.text("oid")) { def.replacen(" ON ONLY ", " ON ", 1) } else { def.clone() };
                o.create = vec![made.clone()];
                o.extra.push(format!("rendering {made}"));
            }
            // A partitioned table's index is valid once every partition's is attached to it: one the
            // target has valid where this one isn't is made again, attaching them as it is made.
            if r.flag("partitioned") && !r.flag("valid") {
                o.def.push_str("\n(invalid)");
            }
            if let Some(parent) = r.get("parent").and_then(|p| p.parse::<u32>().ok()).and_then(|p| ids.class.get(&p).cloned()) {
                let parent_name = parent.split_once(' ').map(|(_, n)| n.to_string()).unwrap_or_default();
                o.def = format!("{def}\nattached to {parent_name}");
                o.extra.push(format!("attached to {parent_name}"));
                o.goes_with = Some(parent.clone());
                o.inline = true;
                o.change = Change::Alter { blocks: false };
                if valid(r.text("parent")) {
                    if let Some(p) = s.objects.get_mut(&parent) {
                        p.after.insert(id.clone());
                    }
                } else {
                    // Made, where its table is attached as the plan's, by that attach already.
                    o.create = vec![if_not_exists(&def)];
                    o.create.push(format!("alter index {parent_name} attach partition {}", r.text("name")));
                    o.deps.insert(parent);
                    o.extra.push("attached by statement".into());
                }
            }
            // The table's indexes, and of those the ones under its parent's, which a table attached
            // as a partition is to have before the attach (`made_before_attach`).
            if let Some(t) = s.objects.get_mut(table) {
                t.extra.push(format!("{TABLE_INDEX}{id}"));
                if child {
                    let by_statement = o.extra.iter().any(|l| l == "attached by statement");
                    t.extra.push(format!("{ATTACHED_INDEX}{id}{SEP}{def}{SEP}{by_statement}"));
                }
            }
            s.add(o);
        }
    }

    // A table's replica identity where it isn't the default (its primary key): what logical
    // replication sends of a changed row, which Supabase Realtime needs `full` for. One that names
    // an index is made after it.
    for r in &rels {
        let Some(how) = r.get("replica") else { continue };
        let name = r.text("name");
        let deps = r.get("replica_index").and_then(|i| i.parse::<u32>().ok()).and_then(|i| ids.class.get(&i).cloned()).into_iter().collect();
        s.add(Object {
            id: format!("replica identity {name}"),
            kind: Some(Kind::Rls),
            owner: Some(relations[&oid(r, "oid")].0.clone()),
            def: how.to_string(),
            create: vec![format!("alter table {name} replica identity {how}")],
            drop: vec![format!("alter table {name} replica identity default")],
            deps,
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
    }

    for r in db
        .rows(&format!(
            "select p.oid, p.prokind as kind, {} as acl, format('%s', p.oid::regprocedure) as ident, format('%I.%I', n.nspname, p.proname) as qname, pg_get_function_result(p.oid) as result, pg_get_function_arguments(p.oid) as args,
              case when p.prokind <> 'a' then pg_get_functiondef(p.oid) end as def,
              case when p.prokind = 'a' then ({}) end as agg
            from pg_proc p join pg_namespace n on n.oid = p.pronamespace
            where p.prokind in ('f', 'p', 'w', 'a') and {ns} and {} and {} and not {}",
            acl_in("p.proacl", 'f', "p.proowner", Some("p.pronamespace")),
            crate::catalog::aggregate_options("p"),
            not_extension("pg_proc", "p"),
            not_platform("p.proowner"),
            made_with("pg_proc", "p")
        ))
        .await?
    {
        let ident = r.text("ident").to_string();
        let id = format!("function {ident}");
        ids.procs.insert(oid(&r, "oid"), id.clone());
        let word = match r.text("kind") {
            "p" => "procedure",
            "a" => "aggregate",
            _ => "function",
        };
        // An aggregate of no arguments is written `(*)` wherever it is named.
        let star = |text: &str| if word == "aggregate" && text.ends_with("()") { format!("{}(*)", &text[..text.len() - 2]) } else { text.to_string() };
        let (def, create) = if word == "aggregate" {
            let def = format!("aggregate ({})", r.text("agg"));
            let args = if r.text("args").is_empty() { "*" } else { r.text("args") };
            (def, format!("create aggregate {}({args}) ({})", r.text("qname"), r.text("agg")))
        } else {
            let text = r.text("def").trim().to_string();
            (text.clone(), text)
        };
        s.add(Object {
            id,
            kind: Some(Kind::Function),
            // Replaced in place (create or replace) when its result and arguments stay; the
            // alterer decides.
            def: format!("{def}\n-- result {}; arguments {}", r.text("result"), r.text("args")),
            create: vec![create],
            drop: vec![format!("drop {word} {}", star(&ident))],
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
        grants(&mut s, r.text("acl"), &ident, &format!("function {ident}"), if word == "procedure" { "procedure" } else { "function" });
    }

    // Base types: a shell first, then the functions that read and write it, which name it, then
    // the type itself over them. They are one object, made and dropped together: the type cannot
    // be dropped while its functions stand, nor they while it does, so it is dropped with cascade,
    // after everything else that uses it.
    for r in db
        .rows(&format!(
            "select t.oid, format('%I.%I', n.nspname, t.typname) as name, t.typinput::oid as input, t.typoutput::oid as output, t.typreceive::oid as receive, t.typsend::oid as send,
              concat_ws(', ', 'input = ' || format('%s', t.typinput::regproc), 'output = ' || format('%s', t.typoutput::regproc),
                case when t.typreceive <> 0 then 'receive = ' || format('%s', t.typreceive::regproc) end,
                case when t.typsend <> 0 then 'send = ' || format('%s', t.typsend::regproc) end,
                case when t.typmodin <> 0 then 'typmod_in = ' || format('%s', t.typmodin::regproc) end,
                case when t.typmodout <> 0 then 'typmod_out = ' || format('%s', t.typmodout::regproc) end,
                case when t.typanalyze <> 0 then 'analyze = ' || format('%s', t.typanalyze::regproc) end,
                case when t.typsubscript <> 0 then 'subscript = ' || format('%s', t.typsubscript::regproc) end,
                'internallength = ' || case when t.typlen < 0 then 'variable' else t.typlen::text end,
                case when t.typbyval then 'passedbyvalue' end,
                'alignment = ' || case t.typalign when 'c' then 'char' when 's' then 'int2' when 'i' then 'int4' else 'double' end,
                'storage = ' || case t.typstorage when 'p' then 'plain' when 'e' then 'external' when 'm' then 'main' else 'extended' end,
                case when t.typcategory <> 'U' then 'category = ' || quote_literal(t.typcategory::text) end,
                case when t.typispreferred then 'preferred = true' end,
                case when t.typdefault is not null then 'default = ' || quote_literal(t.typdefault) end,
                case when t.typelem <> 0 then 'element = ' || format_type(t.typelem, null) end,
                case when t.typdelim <> ',' then 'delimiter = ' || quote_literal(t.typdelim::text) end,
                case when t.typcollation <> 0 then 'collatable = true' end) as def
            from pg_type t join pg_namespace n on n.oid = t.typnamespace
            where t.typtype = 'b' and {ns} and {} and {} and not exists (select 1 from pg_type e where e.typarray = t.oid)",
            not_extension("pg_type", "t"),
            not_platform("t.typowner")
        ))
        .await?
    {
        let name = r.text("name");
        let id = format!("type {name}");
        ids.types.insert(oid(&r, "oid"), id.clone());
        let mut create = vec![format!("create type {name}")];
        let mut def = r.text("def").to_string();
        for f in ["input", "output", "receive", "send"] {
            let Some(function) = ids.procs.get(&oid(&r, f)).cloned() else { continue };
            if let Some(o) = s.objects.remove(&function) {
                create.extend(o.create);
                def.push_str(&format!("\n{}", o.def));
                s.objects.retain(|_, g| g.owner.as_deref() != Some(function.as_str()));
            }
            ids.procs.insert(oid(&r, f), id.clone());
        }
        create.push(format!("create type {name} ({})", r.text("def")));
        s.add(Object { id, kind: Some(Kind::Type), def, create, drop: vec![format!("drop type {name} cascade")], ..Object::default() });
    }
    // A shell type, made to be defined later: a base type's definition makes it first itself.
    for r in db
        .rows(&format!(
            "select t.oid, format('%I.%I', n.nspname, t.typname) as name from pg_type t join pg_namespace n on n.oid = t.typnamespace
            where t.typtype = 'p' and not t.typisdefined and {ns} and {} and {}",
            not_extension("pg_type", "t"),
            not_platform("t.typowner")
        ))
        .await?
    {
        let name = r.text("name");
        let id = format!("type {name}");
        ids.types.insert(oid(&r, "oid"), id.clone());
        s.add(Object { id, kind: Some(Kind::Type), def: "shell".into(), create: vec![format!("create type {name}")], drop: vec![format!("drop type {name}")], ..Object::default() });
    }

    for r in db
        .rows(&format!(
            "select tg.oid, format('%I', tg.tgname) as name, format('%I.%I', n.nspname, c.relname) as rel, c.oid as reloid, pg_get_triggerdef(tg.oid, false) as def, tg.tgenabled as enabled
            from pg_trigger tg join pg_class c on c.oid = tg.tgrelid join pg_namespace n on n.oid = c.relnamespace join pg_proc f on f.oid = tg.tgfoid
            where not tg.tgisinternal and tg.tgparentid = 0 and {ns} and {} and {}",
            not_extension("pg_class", "c"),
            not_platform("f.proowner")
        ))
        .await?
    {
        // A trigger of the project's on a platform's table (Supabase's `auth.users`) is the
        // project's, with no table of the project's to go with.
        let table = relations.get(&oid(&r, "reloid")).map(|(t, _)| t.clone());
        let (rel, name) = (r.text("rel"), r.text("name"));
        let id = format!("trigger {rel}.{name}");
        ids.triggers.insert(oid(&r, "oid"), id.clone());
        let enabled = match r.text("enabled") {
            "D" => Some(format!("alter table {rel} disable trigger {name}")),
            "R" => Some(format!("alter table {rel} enable replica trigger {name}")),
            "A" => Some(format!("alter table {rel} enable always trigger {name}")),
            _ => None,
        };
        s.add(Object {
            id,
            kind: Some(Kind::Trigger),
            owner: table,
            def: format!("{} {}", r.text("def"), r.text("enabled")),
            create: [Some(r.text("def").to_string()), enabled].into_iter().flatten().collect(),
            drop: vec![format!("drop trigger {name} on {rel}")],
            ..Object::default()
        });
    }

    for r in db
        .rows(
            "select p.oid, format('%I', p.polname) as name, format('%I.%I', n.nspname, c.relname) as rel, c.oid as reloid, p.polcmd as cmd, p.polpermissive as permissive,
              (select string_agg(case when r = 0 then 'public' else quote_ident(pg_get_userbyid(r)) end, ', ' order by 1) from unnest(p.polroles) r) as roles,
              pg_get_expr(p.polqual, p.polrelid) as qual, pg_get_expr(p.polwithcheck, p.polrelid) as check
            from pg_policy p join pg_class c on c.oid = p.polrelid join pg_namespace n on n.oid = c.relnamespace",
        )
        .await?
    {
        let (rel, name) = (r.text("rel"), r.text("name"));
        let id = format!("policy {rel}.{name}");
        ids.policies.insert(oid(&r, "oid"), id.clone());
        let table = relations.get(&oid(&r, "reloid")).map(|(t, _)| t.clone());
        let cmd = match r.text("cmd") {
            "r" => "select",
            "a" => "insert",
            "w" => "update",
            "d" => "delete",
            _ => "all",
        };
        let def = format!(
            "as {} for {cmd} to {}{}{}",
            if r.flag("permissive") { "permissive" } else { "restrictive" },
            r.text("roles"),
            r.get("qual").map(|q| format!(" using ({q})")).unwrap_or_default(),
            r.get("check").map(|c| format!(" with check ({c})")).unwrap_or_default()
        );
        s.add(Object {
            id,
            kind: Some(Kind::Policy),
            owner: table,
            create: vec![format!("create policy {name} on {rel} {def}")],
            drop: vec![format!("drop policy {name} on {rel}")],
            def,
            ..Object::default()
        });
    }

    for r in db
        .rows(&format!(
            "select r.oid, r.ev_class, format('%I', r.rulename) as name, format('%I.%I', n.nspname, c.relname) as rel, pg_get_ruledef(r.oid, false) as def, r.rulename = '_RETURN' as view, r.ev_enabled as enabled
            from pg_rewrite r join pg_class c on c.oid = r.ev_class join pg_namespace n on n.oid = c.relnamespace where {ns} and {}",
            not_platform("c.relowner")
        ))
        .await?
    {
        let Some((owner, _)) = relations.get(&oid(&r, "ev_class")) else { continue };
        if r.flag("view") {
            ids.rewrites.insert(oid(&r, "oid"), owner.clone());
            continue;
        }
        let id = format!("rule {}.{}", r.text("rel"), r.text("name"));
        ids.rewrites.insert(oid(&r, "oid"), id.clone());
        // Fired in origin and local sessions (O), never (D), in replicas (R), or always (A).
        let enabled = match r.text("enabled") {
            "D" => Some(format!("alter table {} disable rule {}", r.text("rel"), r.text("name"))),
            "R" => Some(format!("alter table {} enable replica rule {}", r.text("rel"), r.text("name"))),
            "A" => Some(format!("alter table {} enable always rule {}", r.text("rel"), r.text("name"))),
            _ => None,
        };
        s.add(Object {
            id,
            kind: Some(Kind::Rule),
            owner: Some(owner.clone()),
            def: format!("{} {}", r.text("def"), r.text("enabled")),
            create: [Some(r.text("def").trim_end_matches(';').to_string()), enabled].into_iter().flatten().collect(),
            change: Change::Alter { blocks: false },
            drop: vec![format!("drop rule {} on {}", r.text("name"), r.text("rel"))],
            ..Object::default()
        });
    }

    for r in db
        .rows(&format!(
            "select s.oid, format('%I.%I', n.nspname, s.stxname) as name, pg_get_statisticsobjdef(s.oid) as def, s.stxrelid as rel
            from pg_statistic_ext s join pg_namespace n on n.oid = s.stxnamespace where {ns} and {}",
            not_platform("s.stxowner")
        ))
        .await?
    {
        let id = format!("statistics {}", r.text("name"));
        ids.statistics.insert(oid(&r, "oid"), id.clone());
        s.add(Object {
            id,
            kind: Some(Kind::Statistics),
            owner: relations.get(&oid(&r, "rel")).map(|(t, _)| t.clone()),
            def: r.text("def").to_string(),
            create: vec![r.text("def").to_string()],
            drop: vec![format!("drop statistics {}", r.text("name"))],
            ..Object::default()
        });
    }

    for r in db
        .rows(&format!(
            "select o.oid, format('%s', o.oid::regoperator) as ident, format('%I.%s', n.nspname, o.oprname) as name, o.oprleft, o.oprright,
              case when o.oprleft <> 0 then format_type(o.oprleft, null) end as lt, case when o.oprright <> 0 then format_type(o.oprright, null) end as rt,
              format('%s', o.oprcode::regprocedure) as func, format('%s', o.oprcode::regproc) as proc,
              case when o.oprcom <> 0 then 'operator(' || (select format('%I.%s', cn.nspname, c.oprname) from pg_operator c join pg_namespace cn on cn.oid = c.oprnamespace where c.oid = o.oprcom) || ')' end as com,
              case when o.oprnegate <> 0 then 'operator(' || (select format('%I.%s', cn.nspname, c.oprname) from pg_operator c join pg_namespace cn on cn.oid = c.oprnamespace where c.oid = o.oprnegate) || ')' end as neg,
              case when o.oprrest <> 0 then format('%s', o.oprrest::regproc) end as rest, case when o.oprjoin <> 0 then format('%s', o.oprjoin::regproc) end as join, o.oprcanhash as hashes, o.oprcanmerge as merges
            from pg_operator o join pg_namespace n on n.oid = o.oprnamespace where {ns} and {} and {}",
            not_extension("pg_operator", "o"),
            not_platform("o.oprowner")
        ))
        .await?
    {
        let ident = r.text("ident");
        let id = format!("operator {ident}");
        ids.operators.insert(oid(&r, "oid"), id.clone());
        let drop_args = format!("({}, {})", r.get("lt").unwrap_or("none"), r.get("rt").unwrap_or("none"));
        // A shell operator, which an operator naming it as its commutator or negator made: made
        // again by that operator, and dropped on its own.
        if r.text("proc") == "-" {
            s.add(Object { id, kind: Some(Kind::Operator), def: "shell".into(), drop: vec![format!("drop operator {}{drop_args}", r.text("name"))], ..Object::default() });
            continue;
        }
        let mut options = vec![format!("function = {}", r.text("proc"))];
        if let Some(lt) = r.get("lt") {
            options.push(format!("leftarg = {lt}"));
        }
        if let Some(rt) = r.get("rt") {
            options.push(format!("rightarg = {rt}"));
        }
        for (key, column) in [("commutator", "com"), ("negator", "neg"), ("restrict", "rest"), ("join", "join")] {
            if let Some(v) = r.get(column) {
                options.push(format!("{key} = {v}"));
            }
        }
        if r.flag("hashes") {
            options.push("hashes".into());
        }
        if r.flag("merges") {
            options.push("merges".into());
        }
        let def = options.join(", ");
        s.add(Object { id, kind: Some(Kind::Operator), create: vec![format!("create operator {} ({def})", r.text("name"))], drop: vec![format!("drop operator {}{drop_args}", r.text("name"))], def, ..Object::default() });
    }

    // Operator families and classes: a family first, then each class in it with the members
    // bound to the class, then the family's loose members.
    let member = |r: &Row| -> String {
        if r.text("what") == "operator" {
            format!("operator {} {}({}, {}){}", r.text("num"), r.text("name"), r.text("lt"), r.text("rt"), r.get("sort").map(|s| format!(" for order by {s}")).unwrap_or_else(|| " for search".into()))
        } else {
            format!("function {} ({}, {}) {}", r.text("num"), r.text("lt"), r.text("rt"), r.text("name"))
        }
    };
    let members = db
        .rows(
            "select 'operator' as what, ao.amopfamily as family, ao.amopstrategy as num, format_type(ao.amoplefttype, null) as lt, format_type(ao.amoprighttype, null) as rt,
               format('%I.%s', n.nspname, o.oprname) as name, case when ao.amoppurpose = 'o' then (select format('%I.%I', fn.nspname, f.opfname) from pg_opfamily f join pg_namespace fn on fn.oid = f.opfnamespace where f.oid = ao.amopsortfamily) end as sort,
               (select d.refobjid from pg_depend d where d.classid = 'pg_amop'::regclass and d.objid = ao.oid and d.refclassid = 'pg_opclass'::regclass) as class, ao.amopopr as ref, 'pg_operator' as refcls
             from pg_amop ao join pg_operator o on o.oid = ao.amopopr join pg_namespace n on n.oid = o.oprnamespace where ao.amopfamily >= 16384
             union all
             select 'function', ap.amprocfamily, ap.amprocnum, format_type(ap.amproclefttype, null), format_type(ap.amprocrighttype, null), format('%s', ap.amproc::regprocedure), null,
               (select d.refobjid from pg_depend d where d.classid = 'pg_amproc'::regclass and d.objid = ap.oid and d.refclassid = 'pg_opclass'::regclass), ap.amproc, 'pg_proc'
             from pg_amproc ap where ap.amprocfamily >= 16384 order by 2, 1 desc, 3",
        )
        .await?;
    for r in db
        .rows(&format!(
            "select f.oid, format('%I.%I', n.nspname, f.opfname) as name, am.amname as am from pg_opfamily f join pg_namespace n on n.oid = f.opfnamespace join pg_am am on am.oid = f.opfmethod
            where {ns} and {} and {}",
            not_extension("pg_opfamily", "f"),
            not_platform("f.opfowner")
        ))
        .await?
    {
        let (name, am) = (r.text("name"), r.text("am"));
        let id = format!("operator family {name} using {am}");
        ids.opfamilies.insert(oid(&r, "oid"), id.clone());
        let loose: Vec<String> = members.iter().filter(|m| m.text("family") == r.text("oid") && m.get("class").is_none()).map(member).collect();
        let mut create = vec![format!("create operator family {name} using {am}")];
        if !loose.is_empty() {
            create.push(format!("alter operator family {name} using {am} add\n  {}", loose.join(",\n  ")));
        }
        s.add(Object { id, kind: Some(Kind::Operator), def: loose.join(", "), create, drop: vec![format!("drop operator family {name} using {am}")], ..Object::default() });
    }
    for r in db
        .rows(&format!(
            "select c.oid, format('%I.%I', n.nspname, c.opcname) as name, am.amname as am, c.opcdefault as is_default, format_type(c.opcintype, null) as type, c.opcfamily as family,
              case when c.opckeytype <> 0 then format_type(c.opckeytype, null) end as storage
            from pg_opclass c join pg_namespace n on n.oid = c.opcnamespace join pg_am am on am.oid = c.opcmethod where {ns} and {} and {}",
            not_extension("pg_opclass", "c"),
            not_platform("c.opcowner")
        ))
        .await?
    {
        let (name, am) = (r.text("name"), r.text("am"));
        let id = format!("operator class {name} using {am}");
        ids.opclasses.insert(oid(&r, "oid"), id.clone());
        let mine: Vec<&Row> = members.iter().filter(|m| m.get("class") == r.get("oid")).collect();
        let mut items: Vec<String> = mine.iter().map(|m| member(m)).collect();
        if let Some(storage) = r.get("storage") {
            items.push(format!("storage {storage}"));
        }
        let family = ids.opfamilies.get(&oid(&r, "family")).cloned();
        let family_clause = family.as_deref().and_then(|f| f.strip_prefix("operator family ")).and_then(|f| f.rsplit_once(" using ")).map(|(f, _)| format!(" family {f}")).unwrap_or_default();
        let def = format!("{}for type {} using {am}{family_clause} as {}", if r.flag("is_default") { "default " } else { "" }, r.text("type"), items.join(", "));
        let mut deps: BTreeSet<String> = family.into_iter().collect();
        for m in &mine {
            if let Some(d) = ids.of(m.text("refcls"), oid(m, "ref"), 0) {
                deps.insert(d);
            }
        }
        s.add(Object {
            id,
            kind: Some(Kind::Operator),
            create: vec![format!("create operator class {name} {}", def.replacen(" as ", " as\n  ", 1).replace(", operator ", ",\n  operator ").replace(", function ", ",\n  function "))],
            drop: vec![format!("drop operator class {name} using {am}")],
            def,
            deps,
            ..Object::default()
        });
    }

    for r in db
        .rows(&format!(
            "select c.oid, format_type(c.castsource, null) as src, format_type(c.casttarget, null) as tgt, case when c.castfunc <> 0 then format('%s', c.castfunc::regprocedure) end as func,
              c.castcontext as context, c.castmethod as method
            from pg_cast c where c.oid >= 16384 and {} and not {}",
            not_extension("pg_cast", "c"),
            made_with("pg_cast", "c")
        ))
        .await?
    {
        let head = format!("({} as {})", r.text("src"), r.text("tgt"));
        let id = format!("cast {head}");
        ids.casts.insert(oid(&r, "oid"), id.clone());
        let how = match (r.text("method"), r.get("func")) {
            ("f", Some(f)) => format!("with function {f}"),
            ("i", _) => "with inout".to_string(),
            _ => "without function".to_string(),
        };
        let context = match r.text("context") {
            "a" => " as assignment",
            "i" => " as implicit",
            _ => "",
        };
        let def = format!("{how}{context}");
        s.add(Object { id, kind: Some(Kind::Cast), create: vec![format!("create cast {head} {def}")], drop: vec![format!("drop cast {head}")], def, ..Object::default() });
    }

    // Foreign-data wrappers: what a server of theirs connects with.
    for r in db
        .rows(&format!(
            "select w.oid, format('%I', w.fdwname) as name, case when w.fdwhandler <> 0 then format('%s', w.fdwhandler::regproc) end as handler,
              case when w.fdwvalidator <> 0 then format('%s', w.fdwvalidator::regproc) end as validator, {} as options, {} as acl
            from pg_foreign_data_wrapper w where {} and {}",
            option_pairs("w.fdwoptions"),
            acl("w.fdwacl", 'F', "w.fdwowner"),
            crate::catalog::not_extension("pg_foreign_data_wrapper", "w"),
            not_platform("w.fdwowner")
        ))
        .await?
    {
        let name = r.text("name");
        let id = format!("foreign data wrapper {name}");
        ids.wrappers.insert(oid(&r, "oid"), id.clone());
        let options: Vec<(String, String)> = serde_json::from_str(r.text("options")).unwrap_or_default();
        let def = format!(
            "{}{}{}",
            r.get("handler").map(|h| format!(" handler {h}")).unwrap_or_default(),
            r.get("validator").map(|v| format!(" validator {v}")).unwrap_or_default(),
            options_clause(&options)
        );
        let extra = vec![format!("handler {}", r.get("handler").unwrap_or("")), format!("validator {}", r.get("validator").unwrap_or("")), format!("options {}", r.text("options"))];
        grants(&mut s, r.text("acl"), name, &id, "foreign data wrapper");
        s.add(Object {
            id,
            kind: Some(Kind::Server),
            create: vec![format!("create foreign data wrapper {name}{def}")],
            drop: vec![format!("drop foreign data wrapper {name}")],
            def,
            extra,
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
    }

    for r in db
        .rows(&format!(
            "select s.oid, format('%I', s.srvname) as name, format('%I', w.fdwname) as fdw, s.srvtype as type, s.srvversion as version,
              (select string_agg(format('%I %L', split_part(o, '=', 1), substr(o, strpos(o, '=') + 1)), ', ') from unnest(s.srvoptions) o) as options, {} as acl
            from pg_foreign_server s join pg_foreign_data_wrapper w on w.oid = s.srvfdw where {}",
            acl("s.srvacl", 'S', "s.srvowner"),
            not_platform("s.srvowner")
        ))
        .await?
    {
        let name = r.text("name");
        let id = format!("server {name}");
        ids.servers.insert(oid(&r, "oid"), id.clone());
        let def = format!(
            "{}{}foreign data wrapper {}{}",
            r.get("type").map(|t| format!("type {} ", literal(t))).unwrap_or_default(),
            r.get("version").map(|v| format!("version {} ", literal(v))).unwrap_or_default(),
            r.text("fdw"),
            r.get("options").map(|o| format!(" options ({o})")).unwrap_or_default()
        );
        grants(&mut s, r.text("acl"), name, &id, "foreign server");
        s.add(Object { id, kind: Some(Kind::Server), create: vec![format!("create server {name} {def}")], drop: vec![format!("drop server {name}")], def, ..Object::default() });
    }

    // Event triggers: a function run on DDL, as a whole database's.
    for r in db
        .rows(&format!(
            "select et.oid, format('%I', et.evtname) as name, et.evtevent as event, format('%s', et.evtfoid::regproc) as function, et.evtenabled as enabled,
              (select string_agg(quote_literal(t), ', ' order by t) from unnest(et.evttags) t) as tags
            from pg_event_trigger et where {} and {}",
            crate::catalog::not_extension("pg_event_trigger", "et"),
            not_platform("et.evtowner")
        ))
        .await?
    {
        let name = r.text("name");
        let id = format!("event trigger {name}");
        ids.event_triggers.insert(oid(&r, "oid"), id.clone());
        let when = r.get("tags").map(|t| format!(" when tag in ({t})")).unwrap_or_default();
        let enabled = match r.text("enabled") {
            "D" => Some(format!("alter event trigger {name} disable")),
            "R" => Some(format!("alter event trigger {name} enable replica")),
            "A" => Some(format!("alter event trigger {name} enable always")),
            _ => None,
        };
        s.add(Object {
            id,
            kind: Some(Kind::Trigger),
            def: format!("on {}{when} execute function {}() {}", r.text("event"), r.text("function"), r.text("enabled")),
            create: [Some(format!("create event trigger {name} on {}{when} execute function {}()", r.text("event"), r.text("function"))), enabled].into_iter().flatten().collect(),
            drop: vec![format!("drop event trigger {name}")],
            extra: vec![format!("enabled {}", r.text("enabled"))],
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
    }

    // Publications: what each publishes, from the tables and schemas it names.
    // Postgres 15 publishes some of a table's columns, its rows that match a filter, and schemas.
    // The columns a table's list names, by name, which a publication depends on.
    let (columns, schemas, listed) = if version >= 150000 {
        (
            "coalesce(' (' || (select string_agg(format('%I', a.attname), ', ' order by a.attnum) from pg_attribute a where a.attrelid = c.oid and a.attnum = any(pr.prattrs::int2[])) || ')', '') || coalesce(' where (' || pg_get_expr(pr.prqual, c.oid) || ')', '')",
            "(select json_agg(format('%I', n.nspname) order by n.nspname) from pg_publication_namespace pn join pg_namespace n on n.oid = pn.pnnspid where pn.pnpubid = p.oid)",
            "coalesce((select json_agg(format('%I', a.attname)) from pg_attribute a where a.attrelid = c.oid and a.attnum = any(pr.prattrs::int2[])), '[]'::json)",
        )
    } else {
        ("''", "null::json", "'[]'::json")
    };
    for r in db
        .rows(&format!(
            "select p.oid, format('%I', p.pubname) as name, p.puballtables as all_tables, p.pubviaroot as via_root,
              concat_ws(', ', case when p.pubinsert then 'insert' end, case when p.pubupdate then 'update' end, case when p.pubdelete then 'delete' end, case when p.pubtruncate then 'truncate' end) as publish,
              format('%s', (select json_agg(json_build_array(format('%I.%I', n.nspname, c.relname), format('%I.%I', n.nspname, c.relname) || {columns}, {listed}) order by 1) from pg_publication_rel pr join pg_class c on c.oid = pr.prrelid join pg_namespace n on n.oid = c.relnamespace where pr.prpubid = p.oid)) as tables,
              format('%s', {schemas}) as schemas
            from pg_publication p where {}",
            not_platform("p.pubowner")
        ))
        .await?
    {
        let name = r.text("name");
        let tables: Vec<(String, String, Vec<String>)> = serde_json::from_str(r.text("tables")).unwrap_or_default();
        let in_schemas: Vec<String> = serde_json::from_str(r.text("schemas")).unwrap_or_default();
        let mut what = Vec::new();
        if r.flag("all_tables") {
            what.push("all tables".to_string());
        }
        // Each table it holds as itself alone: Postgres lists a table's inheriting children as tables
        // of their own where a publication takes them, so a table named without `only` would bring
        // children the target leaves out.
        if !tables.is_empty() {
            what.push(format!("table {}", tables.iter().map(|(_, t, _)| format!("only {t}")).collect::<Vec<_>>().join(", ")));
        }
        if !in_schemas.is_empty() {
            what.push(format!("tables in schema {}", in_schemas.join(", ")));
        }
        let mut with = Vec::new();
        if r.text("publish") != "insert, update, delete, truncate" {
            with.push(format!("publish = {}", literal(r.text("publish"))));
        }
        if r.flag("via_root") {
            with.push("publish_via_partition_root = true".into());
        }
        let def = format!(
            "{}{}",
            if what.is_empty() { String::new() } else { format!(" for {}", what.join(", ")) },
            if with.is_empty() { String::new() } else { format!(" with ({})", with.join(", ")) }
        );
        let deps = tables
            .iter()
            .flat_map(|(t, _, listed)| std::iter::once(format!("table {t}")).chain(listed.iter().map(move |c| format!("column {t}.{c}"))))
            .chain(in_schemas.iter().map(|n| format!("schema {n}")))
            .collect();
        let mut objects: Vec<String> = tables.iter().map(|(_, t, _)| format!("table only {t}")).collect();
        objects.extend(in_schemas.iter().map(|n| format!("tables in schema {n}")));
        let extra = vec![format!("all {}", r.flag("all_tables")), format!("objects {}", objects.join(", ")), format!("with {}", with.join(", ")), format!("publish {}", r.text("publish"))];
        ids.publications.insert(oid(&r, "oid"), format!("publication {name}"));
        // After the tables it names, among the last objects made.
        s.add(Object {
            id: format!("publication {name}"),
            kind: Some(Kind::Statistics),
            create: vec![format!("create publication {name}{def}")],
            drop: vec![format!("drop publication {name}")],
            def,
            deps,
            extra,
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
    }

    // A partition's copy of its parent's constraint is no object of its own, which Postgres makes
    // and drops with the parent's, but a comment on it is: made again where the parent's is.
    let mut copies: BTreeMap<u32, (String, String, u32)> = BTreeMap::new();
    for r in db
        .rows(
            "select co.oid, format('%I.%I', n.nspname, c.relname) as rel, format('%I', co.conname) as name, co.conparentid as parent
            from pg_constraint co join pg_class c on c.oid = co.conrelid join pg_namespace n on n.oid = c.relnamespace where not co.conislocal and co.conparentid <> 0",
        )
        .await?
    {
        copies.insert(oid(&r, "oid"), (r.text("rel").to_string(), r.text("name").to_string(), oid(&r, "parent")));
    }
    // Constraints that are no objects of their own: a column's not null (Postgres 18) and a
    // domain's constraints, which a comment can be on. Each comment is set again where what it is
    // on changes, which may make the constraint again.
    let mut held: BTreeMap<u32, (String, String)> = BTreeMap::new();
    for r in db
        .rows(&format!(
            "select co.oid, format('%I', co.conname) as name,
              case when co.contypid <> 0 then format('%I.%I', n.nspname, t.typname) end as domain,
              case when co.conrelid <> 0 then format('%I.%I', n.nspname, c.relname) end as rel,
              (select format('%I', a.attname) from pg_attribute a where a.attrelid = co.conrelid and a.attnum = co.conkey[1]) as col
            from pg_constraint co left join pg_class c on c.oid = co.conrelid left join pg_type t on t.oid = co.contypid
              join pg_namespace n on n.oid = coalesce(c.relnamespace, t.typnamespace)
            where ((co.contype = 'n' and co.conrelid <> 0) or co.contypid <> 0) and {ns}",
        ))
        .await?
    {
        let (name, at) = match (r.get("domain"), r.get("rel")) {
            (Some(domain), _) => (format!("type {domain}"), format!("constraint {} on domain {domain}", r.text("name"))),
            (None, Some(rel)) => (format!("column {rel}.{}", r.text("col")), format!("constraint {} on {rel}", r.text("name"))),
            _ => continue,
        };
        held.insert(oid(&r, "oid"), (name, at));
    }
    // Comments on what is not a relation or a column, which are read with them.
    for r in db
        .rows(
            "select format('%s', d.classoid::regclass) as cls, d.objoid, d.description,
              (select t.typtype from pg_type t where d.classoid = 'pg_type'::regclass and t.oid = d.objoid) as typtype,
              (select p.prokind from pg_proc p where d.classoid = 'pg_proc'::regclass and p.oid = d.objoid) as prokind
            from pg_description d where d.objsubid = 0 and d.objoid >= 16384
              and d.classoid in ('pg_namespace'::regclass, 'pg_type'::regclass, 'pg_proc'::regclass, 'pg_constraint'::regclass, 'pg_trigger'::regclass, 'pg_policy'::regclass, 'pg_class'::regclass,
                'pg_collation'::regclass, 'pg_publication'::regclass, 'pg_rewrite'::regclass, 'pg_foreign_server'::regclass, 'pg_event_trigger'::regclass, 'pg_foreign_data_wrapper'::regclass, 'pg_statistic_ext'::regclass)",
        )
        .await?
    {
        if let (Some((rel, name, parent)), Some(text)) = (copies.get(&oid(&r, "objoid")).filter(|_| r.text("cls") == "pg_constraint"), r.get("description")) {
            let mut up = *parent;
            while let Some((_, _, p)) = copies.get(&up) {
                up = *p;
            }
            let Some(key) = ids.constraints.get(&up) else { continue };
            s.add(Object {
                id: format!("comment constraint {rel}.{name}"),
                kind: Some(Kind::Comment),
                owner: Some(format!("table {rel}")),
                def: text.to_string(),
                create: vec![format!("comment on constraint {name} on {rel} is {}", literal(text))],
                drop: vec![format!("comment on constraint {name} on {rel} is null")],
                deps: [key.clone()].into_iter().collect(),
                change: Change::Alter { blocks: false },
                ..Object::default()
            });
            continue;
        }
        // On a not null or a domain's constraint, or on the index a key keeps its rows in: set again
        // where what it is on changes.
        let again = |on: &str, owner: &str, s: &Schema| s.objects.get(owner).map(|o| format!("\n-- on {on}: {}", o.def)).unwrap_or_default();
        let index_of = ids.of(r.text("cls"), oid(&r, "objoid"), 0).filter(|t| r.text("cls") == "pg_class" && t.starts_with("constraint "));
        let special = match (r.text("cls"), held.get(&oid(&r, "objoid")), index_of) {
            ("pg_constraint", Some((owner, at)), _) => Some((format!("comment {at}"), owner.clone(), at.clone())),
            ("pg_class", _, Some(key)) => s.objects.get(&key).and_then(|k| k.extra.iter().find_map(|l| l.strip_prefix(INDEX_OF))).map(|index| (format!("comment index of {key}"), key.clone(), format!("index {index}"))),
            _ => None,
        };
        if let (Some((id, owner, on)), Some(text)) = (special, r.get("description")) {
            let def = format!("{text}{}", again(&on, &owner, &s));
            s.add(Object {
                id,
                kind: Some(Kind::Comment),
                owner: Some(owner.clone()),
                def,
                create: vec![format!("comment on {on} is {}", literal(text))],
                drop: vec![format!("comment on {on} is null")],
                deps: [owner].into_iter().collect(),
                change: Change::Alter { blocks: false },
                ..Object::default()
            });
            continue;
        }
        let Some(target) = ids.of(r.text("cls"), oid(&r, "objoid"), 0) else { continue };
        let (kind, name) = target.split_once(' ').unwrap_or_default();
        let on_table = |rest: &str| {
            let name = super::last_written(rest);
            rest.strip_suffix(name.as_str()).and_then(|t| t.strip_suffix('.')).map(|table| format!("{name} on {table}"))
        };
        let on = match (r.text("cls"), kind) {
            ("pg_namespace", "schema") => Some(format!("schema {name}")),
            ("pg_type", "type") if r.text("typtype") == "d" => Some(format!("domain {name}")),
            ("pg_type", "type") => Some(format!("type {name}")),
            ("pg_proc", "function") if r.text("prokind") == "a" && name.ends_with("()") => Some(format!("aggregate {}(*)", &name[..name.len() - 2])),
            ("pg_proc", "function") => Some(format!("{} {name}", match r.text("prokind") { "p" => "procedure", "a" => "aggregate", _ => "function" })),
            ("pg_constraint", "constraint") => on_table(name).map(|n| format!("constraint {n}")),
            ("pg_trigger", "trigger") => on_table(name).map(|n| format!("trigger {n}")),
            ("pg_policy", "policy") => on_table(name).map(|n| format!("policy {n}")),
            ("pg_class", "index") => Some(format!("index {name}")),
            ("pg_class", "sequence") => Some(format!("sequence {name}")),
            ("pg_collation", "collation") => Some(format!("collation {name}")),
            ("pg_publication", "publication") => Some(format!("publication {name}")),
            ("pg_rewrite", "rule") => on_table(name).map(|n| format!("rule {n}")),
            ("pg_foreign_server", "server") => Some(format!("server {name}")),
            ("pg_event_trigger", "event") => Some(format!("event trigger {}", name.trim_start_matches("trigger "))),
            ("pg_foreign_data_wrapper", "foreign") => Some(format!("foreign data wrapper {}", name.trim_start_matches("data wrapper "))),
            ("pg_statistic_ext", "statistics") => Some(format!("statistics {name}")),
            _ => None,
        };
        let (Some(on), Some(text)) = (on, r.get("description")) else { continue };
        s.add(Object {
            id: format!("comment {target}"),
            kind: Some(Kind::Comment),
            owner: Some(target.clone()),
            def: text.to_string(),
            create: vec![format!("comment on {on} is {}", literal(text))],
            drop: vec![format!("comment on {on} is null")],
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
    }

    // Owners, where an object's isn't the role the plan runs as (a migration's objects are its
    // runner's): what a view, a security definer function and row-level security are checked as.
    // Changed in place, and given back to the runner where the target has no other owner.
    for r in db
        .rows(&format!(
            "select format('%s', k.classid::regclass) as cls, k.objid, o.type, o.identity, format('%I', pg_get_userbyid(k.owner)) as owner
            from ({}) k, lateral pg_identify_object(k.classid, k.objid, 0) o where k.owner <> {}",
            crate::catalog::owned_objects(HISTORY_TABLE),
            crate::catalog::runner()
        ))
        .await?
    {
        let target = match r.text("cls") {
            "pg_publication" => Some(format!("publication {}", r.text("identity"))),
            cls => ids.of(cls, oid(&r, "objid"), 0),
        };
        let Some(target) = target else { continue };
        let word = match r.text("type") {
            "composite type" => "type",
            other => other,
        };
        let (identity, role) = (r.text("identity"), r.text("owner"));
        s.add(Object {
            id: format!("owner {target}"),
            kind: Some(Kind::Grant),
            owner: Some(target.clone()),
            def: role.to_string(),
            create: vec![format!("alter {word} {identity} owner to {role}")],
            drop: vec![format!("alter {word} {identity} owner to current_user")],
            ..Object::default()
        });
    }

    // Default privileges, one object per grant, as the statements that make them: what every table,
    // sequence, function or type a role makes later is granted.
    for statement in crate::db::default_privileges(db).await? {
        let Some(rest) = statement.strip_prefix("alter default privileges ") else { continue };
        // What precedes the grant or the revoke: its role and schema, where it names them.
        let padded = format!(" {rest}");
        let undo = if let Some((head, tail)) = padded.split_once(" grant ") {
            let tail = tail.trim_end_matches(" with grant option");
            let Some((what, to)) = tail.rsplit_once(" to ") else { continue };
            format!("alter default privileges{head} revoke {what} from {to}")
        } else if let Some((head, tail)) = padded.split_once(" revoke ") {
            let Some((what, from)) = tail.rsplit_once(" from ") else { continue };
            format!("alter default privileges{head} grant {what} to {from}")
        } else {
            continue;
        };
        s.add(Object {
            id: format!("default privileges {}", rest.trim_end_matches(" with grant option")),
            kind: Some(Kind::Grant),
            def: rest.ends_with(" with grant option").to_string(),
            create: vec![statement.clone()],
            drop: vec![undo],
            ..Object::default()
        });
    }

    // Array types stand for their element type.
    for r in db.rows("select t.oid, t.typelem from pg_type t where t.oid >= 16384 and t.typcategory = 'A' and t.typelem <> 0").await? {
        if let Some(id) = ids.types.get(&oid(&r, "typelem")).cloned() {
            ids.types.insert(oid(&r, "oid"), id);
        }
    }

    // What each object depends on, as the server records it.
    for r in db
        .rows(
            "select format('%s', d.classid::regclass) as cls, d.objid, d.objsubid, format('%s', d.refclassid::regclass) as rcls, d.refobjid, d.refobjsubid, d.deptype
            from pg_depend d where d.deptype in ('n', 'a') and d.objid >= 16384 and d.refobjid >= 16384",
        )
        .await?
    {
        let from = ids.of(r.text("cls"), oid(&r, "objid"), r.text("objsubid").parse().unwrap_or(0));
        let to = ids.of(r.text("rcls"), oid(&r, "refobjid"), r.text("refobjsubid").parse().unwrap_or(0));
        if let (Some(from), Some(to)) = (from, to) {
            // A sequence a column owns goes with the column, which its owner object says, while the
            // column's default uses the sequence: made first, and dropped after the default goes.
            let owned_by = r.text("deptype") == "a" && from.starts_with("sequence ") && to.starts_with("column ");
            if from != to && !owned_by {
                if let Some(o) = s.objects.get_mut(&from) {
                    o.deps.insert(to);
                }
            }
        }
    }
    // A schema's objects depend on it, as the records say; a column depends on nothing of its own
    // table, which creating the table makes, but a generated column on the columns it reads, which
    // an added or a renamed one must be first.
    let tables: BTreeSet<String> = s.objects.values().filter(|o| o.kind == Some(Kind::Table)).map(|o| o.id.clone()).collect();
    for o in s.objects.values_mut() {
        if let Some(owner) = o.owner.clone() {
            if tables.contains(&owner) && o.kind == Some(Kind::Column) {
                let generated = parse_column(&o.def).generated.is_some();
                o.deps.retain(|d| (generated || !d.starts_with(&format!("column {}.", owner.trim_start_matches("table ")))) && *d != owner);
            }
        }
    }
    // A check that calls a function of its own table's row can't be made with the table, before the
    // function: it is added after both. So is a column of a domain whose check calls one that reads
    // the column's table.
    let late: Vec<(String, String)> = s
        .objects
        .values()
        .filter(|o| matches!(o.kind, Some(Kind::Constraint | Kind::Column)) && o.inline)
        .filter(|o| {
            let table = o.owner.clone().unwrap_or_default();
            let reads_table = |d: &String| d.starts_with("function ") && s.objects.get(d).is_some_and(|f| f.deps.contains(&table));
            o.deps.iter().any(|d| reads_table(d) || (d.starts_with("type ") && s.objects.get(d).is_some_and(|t| t.def.starts_with("domain ") && t.deps.iter().any(reads_table))))
        })
        .map(|o| (o.id.clone(), o.owner.clone().unwrap_or_default()))
        .collect();
    for (id, table) in late {
        let Some(o) = s.objects.get_mut(&id) else { continue };
        o.inline = false;
        let line = o.create.first().and_then(|c| c.split_once(" add ")).map(|(_, l)| l.strip_prefix("column ").unwrap_or(l).to_string()).unwrap_or_default();
        if let Some(t) = s.objects.get_mut(&table) {
            if let Some(create) = t.create.first_mut() {
                for (from, to) in [(format!("\n  {line},"), String::new()), (format!(",\n  {line}\n"), "\n".to_string())] {
                    if create.contains(&from) {
                        *create = create.replacen(&from, &to, 1);
                        break;
                    }
                }
            }
        }
    }
    // An expression names a function by its name alone, which a new signature keeps, while the
    // server binds it to one signature: what calls a function says which, so that it changes
    // when the function it calls is dropped for another.
    for o in s.objects.values_mut() {
        if !matches!(o.kind, Some(Kind::Column | Kind::View | Kind::Constraint | Kind::Index | Kind::Policy | Kind::Trigger | Kind::Rule)) {
            continue;
        }
        let calls: Vec<&String> = o.deps.iter().filter(|d| d.starts_with("function ")).collect();
        if !calls.is_empty() {
            o.def.push_str(&format!("{CALLS}{}", calls.iter().map(|c| c.trim_start_matches("function ")).collect::<Vec<_>>().join(", ")));
        }
    }
    // A column's type change makes its check constraints and partial indexes again from their
    // rendering, which is not always the expression the target stores (a list a varchar is kept
    // to), and an enum swapped for a new one cannot convert a column while a check compares it with
    // a label of the old type: those are dropped around the change and made again after it from
    // the target's own statements. Each column and enum carries them, `AROUND`-prefixed.
    let mut around: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for o in s.objects.values() {
        let expression = match o.kind {
            Some(Kind::Constraint) => bare(&o.def).starts_with("CHECK ") || bare(&o.def).starts_with("EXCLUDE "),
            Some(Kind::Index) => bare(&o.def).contains(" WHERE "),
            _ => false,
        };
        if !expression {
            continue;
        }
        for d in &o.deps {
            if d.starts_with("column ") || d.starts_with("type ") {
                around.entry(d.clone()).or_default().push(o.id.clone());
            }
        }
    }
    // The columns of each type.
    let mut columns_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for o in s.objects.values().filter(|o| o.kind == Some(Kind::Column)) {
        for t in o.deps.iter().filter(|d| d.starts_with("type ")) {
            columns_of.entry(t.clone()).or_default().push(o.id.clone());
        }
    }
    for (id, o) in s.objects.clone() {
        let mut on: BTreeSet<String> = around.get(&id).cloned().unwrap_or_default().into_iter().collect();
        if o.kind == Some(Kind::Type) && o.def.starts_with("enum (") {
            for c in columns_of.get(&id).into_iter().flatten() {
                on.extend(around.get(c).cloned().unwrap_or_default());
            }
            // What calls a function taking or giving the type is the plan's to make again, with
            // the function.
            let functions: BTreeSet<&String> = s.objects.values().filter(|f| f.kind == Some(Kind::Function) && f.deps.contains(&id)).map(|f| &f.id).collect();
            on.retain(|a| !s.objects.get(a).is_some_and(|a| a.deps.iter().any(|d| functions.contains(d))));
        } else if o.kind != Some(Kind::Column) {
            continue;
        }
        let lines: Vec<String> = on.iter().filter_map(|a| s.objects.get(a)).map(|a| around_line(a)).collect();
        if let Some(o) = s.objects.get_mut(&id) {
            o.extra.extend(lines);
        }
    }
    // What names an enum's label uses it, as a default or a check does: a view, a policy, a
    // trigger's condition, an index's predicate, a function, and a partition's bound over a column
    // of the type. A label added in the plan that one of them uses can't be added in place: Postgres
    // refuses a value added in the transaction that uses it.
    // The columns a partitioned table is partitioned by.
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for r in db
        .rows(
            "select format('%I.%I', n.nspname, c.relname) as rel, format('%I', a.attname) as col
            from pg_partitioned_table p join pg_class c on c.oid = p.partrelid join pg_namespace n on n.oid = c.relnamespace
              join pg_attribute a on a.attrelid = p.partrelid and a.attnum = any(p.partattrs::int2[])",
        )
        .await?
    {
        keys.insert(format!("column {}.{}", r.text("rel"), r.text("col")));
    }
    let enum_type = |d: &String| s.objects.get(d).is_some_and(|t| t.def.starts_with("enum ("));
    let mut uses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut enums_of: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for o in s.objects.values() {
        for t in o.deps.iter().filter(|d| enum_type(d)) {
            if o.kind == Some(Kind::Column) {
                if let Some(table) = &o.owner {
                    enums_of.entry(table.clone()).or_default().insert(t.clone());
                }
            } else {
                uses.entry(t.clone()).or_default().push(format!("{USES}{}", o.def.replace('\n', " ")));
            }
        }
    }
    // An enum a table is partitioned by can't be made again under it: a label its partitions use
    // is added and committed before them, and each partition waits for the type's change.
    let mut keyed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for o in s.objects.values().filter(|o| keys.contains(&o.id)) {
        for t in o.deps.iter().filter(|d| enum_type(d)) {
            keyed.entry(o.owner.clone().unwrap_or_default()).or_default().insert(t.clone());
        }
    }
    let mut waits: Vec<(String, BTreeSet<String>)> = Vec::new();
    for o in s.objects.values().filter(|o| o.kind == Some(Kind::Table)) {
        let Some(parent) = o.def.lines().find_map(|l| l.strip_prefix("partition of ")).and_then(|l| l.split_once(' ')).map(|(p, _)| format!("table {p}")) else { continue };
        for t in enums_of.get(&parent).into_iter().flatten() {
            uses.entry(t.clone()).or_default().push(format!("{USES}{}", o.def.replace('\n', " ")));
        }
        if let Some(types) = keyed.get(&parent) {
            waits.push((o.id.clone(), types.clone()));
        }
    }
    for (id, types) in waits {
        if let Some(o) = s.objects.get_mut(&id) {
            o.deps.extend(types);
        }
    }
    for t in keyed.into_values().flatten() {
        uses.entry(t).or_default().push(PARTITION_KEY.to_string());
    }
    for (id, lines) in uses {
        if let Some(o) = s.objects.get_mut(&id) {
            o.extra.extend(lines);
        }
    }
    // What else an enum made again takes to the new type: a domain over it is made again with its
    // columns moved to it, its grants and comment set again, and a composite type's attribute of it
    // changes type, the columns of the composite type moved through their text around the change.
    // (A function taking or giving it is the planner's to make again around the change.)
    let split = |id: &str| -> Option<(String, String)> {
        let rest = id.strip_prefix("column ")?;
        let column = super::last_written(rest);
        Some((rest.strip_suffix(column.as_str())?.strip_suffix('.')?.to_string(), column))
    };
    // The columns of a type, each its table, its name, its type (the type or an array of it) and
    // its default (or none), as fields of the line that carries them. A partition's column is its
    // parent's, which changes it.
    let users = |s: &Schema, ty: &str| -> Vec<String> {
        s.objects
            .values()
            .filter(|c| c.kind == Some(Kind::Column) && c.deps.contains(ty) && c.owner.as_deref().is_some_and(|w| w.starts_with("table ")) && !c.extra.iter().any(|l| l == "partition"))
            .filter_map(|c| {
                let parts = parse_column(&c.def);
                split(&c.id).map(|(t, n)| [t, n, parts.ty, parts.default.unwrap_or_default()])
            })
            .flatten()
            .collect()
    };
    // A composite type's attribute changes only with no column of the type: each carries the
    // columns, which its change moves through their text.
    let mut used: BTreeMap<String, String> = BTreeMap::new();
    for a in s.objects.values().filter(|o| o.def.starts_with("composite (") || (o.kind == Some(Kind::Column) && o.owner.as_deref().is_some_and(|w| w.starts_with("type ")))) {
        let fields = users(&s, if a.kind == Some(Kind::Type) { a.id.as_str() } else { a.owner.as_deref().unwrap_or_default() });
        if !fields.is_empty() {
            used.insert(a.id.clone(), format!("{USED_BY}{}", fields.join(SEP)));
        }
    }
    // A domain carries its own columns, which a change of its base type moves.
    let mut own: BTreeMap<String, String> = BTreeMap::new();
    for d in s.objects.values().filter(|o| o.def.starts_with("domain ")) {
        let fields = users(&s, &d.id);
        if !fields.is_empty() {
            own.insert(d.id.clone(), format!("{COLUMNS_OF}{}", fields.join(SEP)));
        }
    }
    for (id, line) in own {
        if let Some(o) = s.objects.get_mut(&id) {
            o.extra.push(line);
        }
    }
    // A domain an attribute of a composite type has carries that type's columns, which a change
    // that checks its values moves.
    for d in s.objects.values().filter(|o| o.def.starts_with("domain ")) {
        let mut fields: Vec<String> = Vec::new();
        for a in s.objects.values().filter(|o| o.kind == Some(Kind::Column) && o.deps.contains(&d.id)) {
            if let Some(owner) = a.owner.as_deref().filter(|w| w.starts_with("type ")) {
                fields.extend(users(&s, owner));
            }
        }
        if !fields.is_empty() {
            used.insert(d.id.clone(), format!("{USED_BY}{}", fields.join(SEP)));
        }
    }
    for (id, line) in used {
        if let Some(o) = s.objects.get_mut(&id) {
            o.extra.push(line);
        }
    }
    let mut carried: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unhooked: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in s.objects.values().filter(|o| o.def.starts_with("enum (")).map(|o| o.id.clone()).collect::<Vec<_>>() {
        // A column whose default calls a function taking or giving the type has the default taken
        // off and set again by the plan, around the function: its line carries none.
        let functions: BTreeSet<&String> = s.objects.values().filter(|f| f.kind == Some(Kind::Function) && f.deps.contains(&t)).map(|f| &f.id).collect();
        let calls = |line: &str| -> bool {
            let mut fields = line.split(SEP);
            let (Some(rel), Some(col)) = (fields.next(), fields.next()) else { return false };
            s.objects.get(&format!("column {rel}.{col}")).is_some_and(|c| c.deps.iter().any(|d| functions.contains(d)))
        };
        let lines: Vec<String> = s.objects[&t].extra.iter().map(|l| if is_column_line(l) && calls(l) { l.splitn(4, SEP).take(3).chain([""]).collect::<Vec<_>>().join(SEP) } else { l.clone() }).collect();
        unhooked.insert(t.clone(), lines);
        let mut lines = Vec::new();
        for d in s.objects.values().filter(|o| o.def.starts_with("domain ") && o.deps.contains(&t)) {
            lines.push(domain_line(&s, d, &users));
        }
        // A composite type's attribute of the type, or of a domain over it, which the domain's
        // making again leaves on the old one.
        let domains: BTreeSet<&String> = s.objects.values().filter(|o| o.def.starts_with("domain ") && o.deps.contains(&t)).map(|o| &o.id).collect();
        for a in s.objects.values().filter(|o| o.kind == Some(Kind::Column) && (o.deps.contains(&t) || o.deps.iter().any(|d| domains.contains(d))) && o.owner.as_deref().is_some_and(|w| w.starts_with("type "))) {
            lines.extend(attribute_line(&s, a, &users));
        }
        carried.insert(t, lines);
    }
    for (id, lines) in unhooked {
        if let Some(o) = s.objects.get_mut(&id) {
            o.extra = lines;
        }
    }
    // A domain carries the composite types' attributes of it and the domains over it, which its
    // base type's change moves.
    for d in s.objects.values().filter(|o| o.def.starts_with("domain ")).map(|o| o.id.clone()).collect::<Vec<_>>() {
        let mut lines: Vec<String> = s.objects.values().filter(|o| o.kind == Some(Kind::Column) && o.deps.contains(&d) && o.owner.as_deref().is_some_and(|w| w.starts_with("type "))).filter_map(|a| attribute_line(&s, a, &users)).collect();
        for child in s.objects.values().filter(|o| o.def.starts_with("domain ") && o.deps.contains(&d)) {
            lines.push(domain_line(&s, child, &users));
        }
        carried.entry(d).or_default().extend(lines);
    }
    for (id, lines) in carried {
        if let Some(o) = s.objects.get_mut(&id) {
            o.extra.extend(lines);
        }
    }
    // A schema's owner gives up its own usage after everything else in the schema is made, which
    // a role that is no superuser can't reach without it.
    let revokes: Vec<(String, String)> = s.objects.values().filter(|o| o.id.ends_with(" from its owner") && o.owner.as_deref().is_some_and(|w| w.starts_with("schema "))).map(|o| (o.id.clone(), o.owner.clone().unwrap_or_default())).collect();
    for (id, schema) in revokes {
        let inside = format!(" {}.", schema.trim_start_matches("schema "));
        let rest: BTreeSet<String> = s.objects.keys().filter(|k| **k != id && k.contains(&inside)).cloned().collect();
        if let Some(o) = s.objects.get_mut(&id) {
            o.after.extend(rest);
        }
    }
    // An enum or a domain made again takes its grants, revokes, owner and comment back, which a new
    // type lacks.
    let mut metadata: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for o in s.objects.values().filter(|o| matches!(o.kind, Some(Kind::Grant | Kind::Comment))) {
        if let Some(t) = o.owner.as_ref().filter(|t| s.objects.get(*t).is_some_and(|t| t.def.starts_with("enum (") || t.def.starts_with("domain "))) {
            metadata.entry(t.clone()).or_default().extend(o.create.iter().map(|c| format!("{AGAIN}{c}")));
        }
    }
    for (id, lines) in metadata {
        if let Some(o) = s.objects.get_mut(&id) {
            o.extra.extend(lines);
        }
    }
    Ok(s)
}

/// Whether `text` names an enum's label (`'b'` as written in the type), as a literal or as an
/// element of an array's (`'{a,b}'`, `'{"b c"}'`, `'{"b,c"}'`).
fn names_label(text: &str, label: &str) -> bool {
    if text.contains(label) {
        return true;
    }
    let bare = label.trim_start_matches('\'').trim_end_matches('\'').replace("''", "'");
    string_literals(text).iter().filter(|l| l.starts_with('{') && l.ends_with('}')).any(|l| array_elements(l).contains(&bare))
}

/// The values of the string literals in `text` (`'it''s'` as `it's`).
fn string_literals(text: &str) -> Vec<String> {
    let (mut out, mut cur, mut inside) = (Vec::new(), String::new(), false);
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match (ch, inside) {
            ('\'', false) => inside = true,
            ('\'', true) if chars.peek() == Some(&'\'') => {
                chars.next();
                cur.push('\'');
            }
            ('\'', true) => {
                inside = false;
                out.push(std::mem::take(&mut cur));
            }
            (_, true) => cur.push(ch),
            _ => {}
        }
    }
    out
}

/// The elements of an array's literal (`{a,"b,c",{d}}`), each its value: a quoted one unquoted and
/// unescaped, an unquoted one trimmed, and a nested array's among them.
fn array_elements(literal: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut was_quoted, mut escaped) = (Vec::new(), String::new(), false, false, false);
    for ch in literal.chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            // Space around a quoted element is no part of it.
            '"' => {
                if !quoted && !was_quoted {
                    cur.clear();
                }
                quoted = !quoted;
                was_quoted = true;
            }
            c if c.is_whitespace() && was_quoted && !quoted => {}
            '{' | '}' | ',' if !quoted => {
                let value = if was_quoted { cur.clone() } else { cur.trim().to_string() };
                if was_quoted || !value.is_empty() {
                    out.push(value);
                }
                cur.clear();
                was_quoted = false;
            }
            _ => cur.push(ch),
        }
    }
    out
}

/// A list of string literals, `'a', 'b, c'`, each as written: the commas inside a literal are its own.
fn literal_list(list: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
    for ch in list.chars() {
        match ch {
            '\'' => {
                quoted = !quoted;
                cur.push(ch);
            }
            ',' if !quoted => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// A qualified name's schema and name, as the reader writes them (`public."My Table"`).
fn split_name(qualified: &str) -> Option<(&str, &str)> {
    let mut quoted = false;
    for (k, c) in qualified.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '.' if !quoted => return Some((&qualified[..k], &qualified[k + 1..])),
            _ => {}
        }
    }
    None
}

/// The tables that `from` has in one schema and `to` has in another, under the same name and with
/// the same columns, as `(from's name, to's)`: each one table moved, keeping its rows, rather than
/// one dropped and another created.
pub fn moved(from: &Schema, to: &Schema) -> Vec<(String, String)> {
    let shape = |schema: &Schema, id: &str| -> Option<(String, Vec<(String, String)>)> {
        let (_, name) = split_name(id.strip_prefix("table ")?)?;
        let mut columns: Vec<(String, String)> = schema
            .objects
            .values()
            .filter(|o| o.kind == Some(Kind::Column) && o.owner.as_deref() == Some(id))
            .map(|o| (o.id.rsplit('.').next().unwrap_or_default().to_string(), parse_column(&o.def).ty))
            .collect();
        columns.sort();
        Some((name.to_string(), columns))
    };
    let tables = |a: &Schema, b: &Schema| -> Vec<(String, (String, Vec<(String, String)>))> {
        a.objects.values().filter(|o| o.kind == Some(Kind::Table) && !b.objects.contains_key(&o.id)).filter_map(|o| Some((o.id.clone(), shape(a, &o.id)?))).collect()
    };
    let (gone, come) = (tables(from, to), tables(to, from));
    let mut out = Vec::new();
    for (old, key) in &gone {
        let same: Vec<&String> = come.iter().filter(|(_, k)| k == key).map(|(id, _)| id).collect();
        let rivals = gone.iter().filter(|(_, k)| k == key).count();
        if same.len() == 1 && rivals == 1 {
            out.push((old.trim_start_matches("table ").to_string(), same[0].trim_start_matches("table ").to_string()));
        }
    }
    out
}

/// The statements that rename, with the tables `renamed` renames (each its old name and new name,
/// and its old id and new id, in `current`'s and `target`'s), what Postgres named after each: its
/// constraints, indexes and sequences (`users_pkey`), where the target names them after the new
/// name. A sequence keeps its value, and no index is built again. They follow the tables' renames.
pub fn derived_renames(current: &Schema, target: &Schema, renamed: &[(String, String, String, String)]) -> Vec<String> {
    // The text after the last dot outside quotes, and what is before it.
    let last = |rest: &str| -> (String, String) {
        let mut quoted = false;
        let mut at = None;
        for (k, ch) in rest.char_indices() {
            match ch {
                '"' => quoted = !quoted,
                '.' if !quoted => at = Some(k),
                _ => {}
            }
        }
        at.map_or((String::new(), rest.to_string()), |k| (rest[..k].to_string(), rest[k + 1..].to_string()))
    };
    let mut out = Vec::new();
    for (old, new, old_table, new_table) in renamed {
        let related = |o: &Object| o.owner.as_deref() == Some(old_table.as_str()) || o.goes_with.as_ref().is_some_and(|c| current.objects.get(c).is_some_and(|c| c.owner.as_deref() == Some(old_table.as_str())));
        let new_rel = new_table.strip_prefix("table ").unwrap_or(new_table);
        for o in current.objects.values().filter(|o| matches!(o.kind, Some(Kind::Constraint | Kind::Index | Kind::Sequence)) && related(o)) {
            let Some((word, rest)) = o.id.split_once(' ') else { continue };
            let (prefix, name) = last(rest);
            let wanted = crate::diff::swap_name(&name, old, new);
            let at = if o.kind == Some(Kind::Constraint) { new_rel.to_string() } else { prefix.clone() };
            if wanted == name || !target.objects.get(&format!("{word} {at}.{wanted}")).is_some_and(|t| t.kind == o.kind) {
                continue;
            }
            out.push(match o.kind {
                Some(Kind::Constraint) => format!("alter table {at} rename constraint {name} to {wanted}"),
                Some(Kind::Index) => format!("alter index {rest} rename to {wanted}"),
                _ => format!("alter sequence {rest} rename to {wanted}"),
            });
        }
        // Each identity's sequence, which Postgres names after the table and keeps through its
        // rename, where the target's is named after the new one. A partition's identity is its
        // parent's, with no sequence of its own.
        let old_rel = old_table.strip_prefix("table ").unwrap_or(old_table);
        let (schema, _) = last(old_rel);
        for o in current.objects.values().filter(|o| o.kind == Some(Kind::Column) && o.owner.as_deref() == Some(old_table.as_str()) && parse_column(&o.def).identity.is_some() && !o.extra.iter().any(|l| l == "partition")) {
            let Some(column) = o.id.strip_prefix(&format!("column {old_rel}.")) else { continue };
            let Some(t) = target.objects.get(&format!("column {new_rel}.{column}")).filter(|t| parse_column(&t.def).identity.is_some()) else { continue };
            let column = column.strip_prefix('"').and_then(|c| c.strip_suffix('"')).map(|c| c.replace("\"\"", "\"")).unwrap_or_else(|| column.to_string());
            let chosen = |table: &str| {
                let mut name = format!("{table}_{column}_seq");
                while name.len() > 63 {
                    name.pop();
                }
                format!("{schema}.{}", declared::quote_ident(&name))
            };
            let (was, now) = (identity_sequence(o).unwrap_or_else(|| chosen(old)), identity_sequence(t).unwrap_or_else(|| chosen(new)));
            if was != now {
                out.push(format!("alter sequence {was} rename to {}", last(&now).1));
            }
        }
        // Each column's not null, which Postgres 18 names after the table too.
        for o in current.objects.values().filter(|o| o.kind == Some(Kind::Column) && o.owner.as_deref() == Some(old_table.as_str())) {
            let Some(column) = o.id.strip_prefix(&format!("column {old_rel}.")) else { continue };
            let Some(name) = not_null_name(o) else { continue };
            let wanted = crate::diff::swap_name(&name, old, new);
            if wanted != name && target.objects.get(&format!("column {new_rel}.{column}")).and_then(not_null_name).as_deref() == Some(wanted.as_str()) {
                out.push(format!("alter table {new_rel} rename constraint {name} to {wanted}"));
            }
        }
    }
    out
}

/// What precedes the name of an identity's sequence where it isn't the one Postgres would choose.
const IDENTITY_SEQUENCE: &str = "identity sequence ";

/// The name of an identity column's sequence, where Postgres wouldn't have chosen it.
fn identity_sequence(o: &Object) -> Option<String> {
    o.extra.iter().find_map(|l| l.strip_prefix(IDENTITY_SEQUENCE)).map(String::from)
}

/// A column definition, or an identity, with its sequence's name among the identity's options.
fn named_identity(def: &str, sequence: &str) -> String {
    if def.contains(" as identity (") {
        def.replacen(" as identity (", &format!(" as identity (sequence name {sequence} "), 1)
    } else {
        def.replacen(" as identity", &format!(" as identity (sequence name {sequence})"), 1)
    }
}

/// The name of a column's not null (Postgres 18).
fn not_null_name(o: &Object) -> Option<String> {
    o.extra.iter().find_map(|l| l.strip_prefix("not null name ")).map(String::from)
}

/// Each table's indexes by name, each table as `schema.table` (quoted where it must be), the ones a
/// constraint or a partition's parent made included: what no object of the schema shows.
pub async fn index_names(db: &mut Db) -> Result<BTreeMap<String, BTreeSet<String>>, Error> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let rows = db
        .rows(&format!(
            "select format('%I.%I', tn.nspname, t.relname) as rel, c.relname as name from pg_index i join pg_class c on c.oid = i.indexrelid join pg_class t on t.oid = i.indrelid join pg_namespace tn on tn.oid = t.relnamespace where {}",
            crate::catalog::PG_USER_NS.replace("n.nspname", "tn.nspname")
        ))
        .await?;
    for r in rows {
        out.entry(r.text("rel").to_string()).or_default().insert(r.text("name").to_string());
    }
    Ok(out)
}

/// After the tables `renamed` names were renamed on `db` (each its old name, its new name and its
/// new id), the statements that rename the indexes of each named after its old name, where the
/// target (whose indexes `target` lists) names them after its new one: those no object of the
/// schema is, as a partition's, which its table's key made.
pub async fn inherited_renames(db: &mut Db, target: &BTreeMap<String, BTreeSet<String>>, renamed: &[(String, String, String)]) -> Result<Vec<String>, Error> {
    let now = index_names(db).await?;
    let quote = |name: &str| if regex::Regex::new(r"^[a-z_][a-z0-9_$]*$").unwrap().is_match(name) { name.to_string() } else { format!("\"{}\"", name.replace('"', "\"\"")) };
    let mut out = Vec::new();
    for (old, new, id) in renamed {
        let rel = id.strip_prefix("table ").unwrap_or(id);
        let (Some(have), Some(want)) = (now.get(rel), target.get(rel)) else { continue };
        let schema = split_last(rel).0;
        for name in have.iter().filter(|n| !want.contains(*n)) {
            let wanted = crate::diff::swap_name(name, old, new);
            if wanted != *name && want.contains(&wanted) && !have.contains(&wanted) {
                out.push(format!("alter index {schema}.{} rename to {}", quote(name), quote(&wanted)));
            }
        }
    }
    Ok(out)
}

/// The statements that move the tables `moved` names, its new schemas first: run on the database
/// `from` was read from, ahead of the plan read from it afterwards, which opens with them.
pub fn moves(from: &Schema, to: &Schema) -> Vec<String> {
    let mut out = Vec::new();
    for (old, new) in moved(from, to) {
        let Some((schema, _)) = split_name(&new) else { continue };
        let create = format!("create schema {schema}");
        if !from.objects.contains_key(&format!("schema {schema}")) && !out.contains(&create) {
            out.push(create);
        }
        out.push(format!("alter table {old} set schema {schema}"));
    }
    out
}

/// The privileges an object grants others than its owner, beside those it grants by default, as
/// SQL that gives JSON: `acl` is its ACL column, `kind` its `acldefault` letter, `owner` its owner.
/// A third list holds what its owner's default privileges grant such an object as it is made
/// (`alter default privileges`, in `schema` where one is given, else anywhere), which a plan that
/// makes it doesn't grant again: a platform's (Supabase's grant every new table to its API's roles).
fn acl(acl: &str, kind: char, owner: &str) -> String {
    acl_in(acl, kind, owner, None)
}

fn acl_in(acl: &str, kind: char, owner: &str, schema: Option<&str>) -> String {
    let explode = |of: &str| {
        format!(
            "(select json_agg(json_build_object('grantee', case when x.grantee = 0 then 'public' else quote_ident(pg_get_userbyid(x.grantee)) end, 'privilege', x.privilege_type, 'grantable', x.is_grantable))
               from aclexplode({of}) x where x.grantee <> {owner})"
        )
    };
    let object = match kind {
        'r' => "r",
        's' => "S",
        'f' => "f",
        'n' => "n",
        'T' => "T",
        _ => "",
    };
    let implied = format!(
        "(select json_agg(json_build_object('grantee', case when x.grantee = 0 then 'public' else quote_ident(pg_get_userbyid(x.grantee)) end, 'privilege', x.privilege_type, 'grantable', x.is_grantable))
           from pg_default_acl d, aclexplode(d.defaclacl) x where d.defaclrole = {owner} and d.defaclobjtype = '{object}' and d.defaclnamespace in (0, {}) and x.grantee <> {owner})",
        schema.unwrap_or("0")
    );
    // What the owner has taken from itself, and how a statement names it: the role the plan runs
    // as, where it owns the object, as `current_user`.
    let lacks = format!(
        "(select json_agg(d.privilege_type order by d.privilege_type) from aclexplode(acldefault('{kind}', {owner})) d
           where d.grantee = {owner} and not exists (select 1 from aclexplode(coalesce({acl}, acldefault('{kind}', {owner}))) x where x.grantee = {owner} and x.privilege_type = d.privilege_type))"
    );
    let spelled = format!("case when {owner} = {} then 'current_user' else quote_ident(pg_get_userbyid({owner})) end", crate::catalog::runner());
    format!("format('%s', json_build_array({}, {}, {implied}, {lacks}, {spelled}))", explode(&format!("coalesce({acl}, acldefault('{kind}', {owner}))")), explode(&format!("acldefault('{kind}', {owner})")))
}

/// One object per privilege the object `name` (`on` says what it is: `table`, `function`, …)
/// grants beyond those it has by default, and one per default privilege it has revoked: a new
/// object has its defaults, which the plan revokes where the target has. `privileges` is `acl`'s.
fn grants(s: &mut Schema, privileges: &str, name: &str, id: &str, on: &str) {
    let parts: Vec<serde_json::Value> = serde_json::from_str(privileges).unwrap_or_default();
    // What the owner has taken from itself, revoked after the object is made or its owner set.
    let owner = parts.get(4).and_then(|v| v.as_str()).unwrap_or("current_user").to_string();
    for privilege in parts.get(3).and_then(|v| v.as_array()).cloned().unwrap_or_default().iter().filter_map(|p| p.as_str()) {
        s.add(Object {
            id: format!("revoke {privilege} on {name} from its owner"),
            kind: Some(Kind::Grant),
            owner: Some(id.to_string()),
            def: String::new(),
            create: vec![format!("revoke {} on {on} {name} from {owner}", privilege.to_lowercase())],
            drop: vec![format!("grant {} on {on} {name} to {owner}", privilege.to_lowercase())],
            deps: [format!("owner {id}")].into_iter().collect(),
            ..Object::default()
        });
    }
    let lists: Vec<Option<Vec<serde_json::Value>>> = parts.iter().take(3).map(|v| serde_json::from_value(v.clone()).ok()).collect();
    let (have, default) = (lists.first().cloned().flatten().unwrap_or_default(), lists.get(1).cloned().flatten().unwrap_or_default());
    let key = |g: &serde_json::Value| (g["grantee"].as_str().unwrap_or_default().to_string(), g["privilege"].as_str().unwrap_or_default().to_string(), g["grantable"].as_bool().unwrap_or(false));
    let (have, default): (BTreeSet<_>, BTreeSet<_>) = (have.iter().map(key).collect(), default.iter().map(key).collect());
    // What the owner's default privileges grant it as it is made: made with it, never granted again.
    let implied: BTreeSet<_> = lists.get(2).cloned().flatten().unwrap_or_default().iter().map(key).collect();
    let grant = |grantee: &str, privilege: &str, grantable: bool| format!("grant {} on {on} {name} to {grantee}{}", privilege.to_lowercase(), if grantable { " with grant option" } else { "" });
    let revoke = |grantee: &str, privilege: &str| format!("revoke {} on {on} {name} from {grantee}", privilege.to_lowercase());
    for (grantee, privilege, grantable) in have.difference(&default) {
        s.add(Object {
            id: format!("grant {privilege} on {name} to {grantee}"),
            kind: Some(Kind::Grant),
            owner: Some(id.to_string()),
            def: grantable.to_string(),
            create: vec![grant(grantee, privilege, *grantable)],
            drop: vec![revoke(grantee, privilege)],
            inline: implied.contains(&(grantee.clone(), privilege.clone(), *grantable)),
            ..Object::default()
        });
    }
    // What the owner's default privileges grant it as it is made, and it doesn't have: revoked
    // after it is made, which an object already there has no need of.
    for (grantee, privilege, grantable) in implied.iter().filter(|g| !have.contains(g) && !default.contains(g)) {
        s.add(Object {
            id: format!("revoke {privilege} on {name} from {grantee}"),
            kind: Some(Kind::Grant),
            owner: Some(id.to_string()),
            def: grantable.to_string(),
            create: vec![revoke(grantee, privilege)],
            drop: vec![grant(grantee, privilege, *grantable)],
            ..Object::default()
        });
    }
    for (grantee, privilege, grantable) in default.difference(&have) {
        s.add(Object {
            id: format!("revoke {privilege} on {name} from {grantee}"),
            kind: Some(Kind::Grant),
            owner: Some(id.to_string()),
            def: grantable.to_string(),
            create: vec![revoke(grantee, privilege)],
            drop: vec![grant(grantee, privilege, *grantable)],
            ..Object::default()
        });
    }
}

/// One object per privilege a column grants of its own (`grant select (email) on table users to
/// anon`), which a column has none of by default. `privileges` is `acl`'s, `table` the column's
/// table and `id` the column's.
fn column_grants(s: &mut Schema, privileges: &str, table: &str, column: &str, id: &str) {
    let parts: Vec<serde_json::Value> = serde_json::from_str(privileges).unwrap_or_default();
    for g in parts.first().and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let (grantee, privilege, grantable) = (g["grantee"].as_str().unwrap_or_default(), g["privilege"].as_str().unwrap_or_default().to_lowercase(), g["grantable"].as_bool().unwrap_or(false));
        s.add(Object {
            id: format!("grant {privilege} ({column}) on {table} to {grantee}"),
            kind: Some(Kind::Grant),
            owner: Some(id.to_string()),
            def: grantable.to_string(),
            create: vec![format!("grant {privilege} ({column}) on table {table} to {grantee}{}", if grantable { " with grant option" } else { "" })],
            drop: vec![format!("revoke {privilege} ({column}) on table {table} from {grantee}")],
            ..Object::default()
        });
    }
}

/// SQL giving an options array (`fdwoptions`, `srvoptions`) as JSON pairs of a name and a value.
fn option_pairs(column: &str) -> String {
    format!("format('%s', coalesce((select json_agg(json_build_array(split_part(o, '=', 1), substr(o, strpos(o, '=') + 1)) order by o) from unnest({column}) o), '[]'::json))")
}

/// Options as a `create` statement writes them: ` options (debug 'true')`, or nothing.
fn options_clause(options: &[(String, String)]) -> String {
    if options.is_empty() {
        return String::new();
    }
    format!(" options ({})", options.iter().map(|(k, v)| format!("{} {}", declared::quote_ident(k), literal(v))).collect::<Vec<_>>().join(", "))
}

/// The `options (…)` clause of an `alter` that takes options `from` has to those `to` has.
fn options_change(from: &[(String, String)], to: &[(String, String)]) -> Option<String> {
    let mut out = Vec::new();
    for (k, v) in to {
        match from.iter().find(|(a, _)| a == k) {
            Some((_, w)) if w == v => {}
            Some(_) => out.push(format!("set {} {}", declared::quote_ident(k), literal(v))),
            None => out.push(format!("add {} {}", declared::quote_ident(k), literal(v))),
        }
    }
    for (k, _) in from.iter().filter(|(a, _)| !to.iter().any(|(b, _)| b == a)) {
        out.push(format!("drop {}", declared::quote_ident(k)));
    }
    (!out.is_empty()).then(|| format!("options ({})", out.join(", ")))
}

/// A name as written (`public."t.x"`) split before its last part: what qualifies it (empty where
/// nothing does) and the last part, each as written. A dot inside quotes separates nothing.
fn split_last(name: &str) -> (&str, &str) {
    let at = name.len() - crate::sql::last_written(name).len();
    (name[..at].strip_suffix('.').unwrap_or(&name[..at]), &name[at..])
}

/// A name as written (`"c x"`, `k`) and the text after it, past one space.
fn name_first(text: &str) -> Option<(&str, &str)> {
    let end = if text.starts_with('"') {
        let mut chars = text.char_indices().skip(1).peekable();
        let mut end = None;
        while let Some((k, ch)) = chars.next() {
            if ch == '"' {
                if chars.peek().is_some_and(|(_, c)| *c == '"') {
                    chars.next();
                } else {
                    end = Some(k + 1);
                    break;
                }
            }
        }
        end?
    } else {
        text.find(' ')?
    };
    Some((&text[..end], text[end..].strip_prefix(' ')?))
}

/// A publication's objects as `drop` names them: each table without its column list and row
/// filter, which Postgres's grammar refuses there.
fn without_lists(objects: &str) -> String {
    let (mut out, mut item, mut depth, mut quoted) = (Vec::new(), String::new(), 0, false);
    for ch in objects.chars().chain([',']) {
        match ch {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                out.push(std::mem::take(&mut item));
                continue;
            }
            _ => {}
        }
        item.push(ch);
    }
    out.iter()
        .map(|item| {
            let item = item.trim();
            if !item.starts_with("table ") {
                return item.to_string();
            }
            let mut quoted = false;
            let cut = item.char_indices().find(|(k, ch)| {
                if *ch == '"' {
                    quoted = !quoted;
                }
                !quoted && (item[*k..].starts_with(" (") || item[*k..].starts_with(" where "))
            });
            cut.map(|(k, _)| item[..k].to_string()).unwrap_or_else(|| item.to_string())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The names in a list `format('%I.%I')` wrote, each joined by a comma and a space.
fn names_in(list: &str) -> Vec<String> {
    let (mut out, mut item, mut quoted) = (Vec::new(), String::new(), false);
    for ch in list.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                out.push(std::mem::take(&mut item).trim().to_string());
                continue;
            }
            _ => {}
        }
        item.push(ch);
    }
    out.push(item.trim().to_string());
    out.into_iter().filter(|n| !n.is_empty()).collect()
}

/// The name Postgres gives a not null it makes on a table's column, as it holds it.
fn default_not_null(table: &str, column: &str) -> String {
    let name = format!("{}_{}_not_null", unquoted(split_last(table).1), unquoted(column));
    let mut end = name.len().min(63);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_string()
}

/// A name as Postgres holds it, from how it is written: a quoted one without its quotes.
fn unquoted(name: &str) -> String {
    match name.strip_prefix('"').and_then(|n| n.strip_suffix('"')) {
        Some(inner) => inner.replace("\"\"", "\""),
        None => name.to_string(),
    }
}

/// What a composite type and each of its attributes carry: the columns of the type, which a change
/// of them moves.
const USED_BY: &str = "used by ";

/// A domain as a type it is over carries it: its name, create, grants and comment, and columns.
fn domain_line(s: &Schema, d: &Object, users: &dyn Fn(&Schema, &str) -> Vec<String>) -> String {
    let metadata: Vec<String> = s.objects.values().filter(|o| matches!(o.kind, Some(Kind::Grant | Kind::Comment)) && o.owner.as_deref() == Some(d.id.as_str())).flat_map(|o| o.create.iter().cloned()).collect();
    let fields = [vec![d.id.trim_start_matches("type ").to_string(), d.create.join(";\n"), metadata.join(";\n")], users(s, &d.id)].concat();
    format!("{DOMAIN}{}", fields.join(SEP))
}

/// A composite type's attribute as an enum or a domain carries it: the type, the attribute, its
/// type as written, and the type's columns.
fn attribute_line(s: &Schema, a: &Object, users: &dyn Fn(&Schema, &str) -> Vec<String>) -> Option<String> {
    let owner = a.owner.as_deref()?;
    let attribute = crate::sql::last_written(a.id.strip_prefix("column ")?);
    let fields = [vec![owner.trim_start_matches("type ").to_string(), attribute, parse_column(&a.def).ty], users(s, owner)].concat();
    Some(format!("{ATTRIBUTE}{}", fields.join(SEP)))
}

/// What a domain carries: its columns, which a change of its base type moves.
const COLUMNS_OF: &str = "columns of ";

/// A composite type's attribute, as the type carries it: its name and definition.
const MEMBER: &str = "member ";

/// What separates the fields of a line an object carries: no name, definition or statement holds
/// it, since Postgres keeps no NUL in its text.
const SEP: &str = "\0";

/// What an object a change is made around is carried as: `around <id>`, its definition, its drop
/// and its create, each after a `SEP`.
const AROUND: &str = "around ";

/// A statement an enum made again runs after: one of its grants, its owner or its comment.
const AGAIN: &str = "again ";

/// A view's definition that names an enum's labels, carried by the enum.
const USES: &str = "uses ";

/// What an enum carries of what uses it beside its columns: a domain over it (its name, create,
/// grants and comment, and columns), and a composite type's attribute of it (the type, the
/// attribute, and the type's columns), each field after a `SEP`, and a column as its table and its
/// name.
const DOMAIN: &str = "domain of ";
const ATTRIBUTE: &str = "attribute of ";

/// Whether a line an enum carries is one of its columns (its table, name, type and default), not
/// what else it carries.
fn is_column_line(l: &str) -> bool {
    ![AROUND, AGAIN, USES, DOMAIN, ATTRIBUTE, PARTITION_KEY].iter().any(|p| l.starts_with(p))
}

/// What an enum a table is partitioned by carries: it can't be made again.
const PARTITION_KEY: &str = "partition key";

/// An index of a table, and an index of a partition that is attached to its table's.
const TABLE_INDEX: &str = "index on table ";
const ATTACHED_INDEX: &str = "index under its table's ";

/// The indexes a table attached as a partition is to have under its table's that it lacks, each
/// its id, its statement and whether it is attached by a statement of its own (under an index made
/// on its table alone, which may be made after): made before the attach, which attaches each to
/// its table's index there, where it would otherwise make one of its own under a name it chooses.
fn made_before_attach(from: &Object, to: &Object) -> Vec<(String, String, bool)> {
    let part = |o: &Object, what: &str| o.extra.iter().find_map(|l| l.strip_prefix(what)).map(String::from);
    let attached = part(to, "partition of ").is_some() && part(from, "partition of ") != part(to, "partition of ");
    if !attached {
        return Vec::new();
    }
    let had: BTreeSet<&str> = from.extra.iter().filter_map(|l| l.strip_prefix(TABLE_INDEX)).collect();
    to.extra
        .iter()
        .filter_map(|l| {
            let mut f = l.strip_prefix(ATTACHED_INDEX)?.split(SEP);
            Some((f.next()?, f.next()?, f.next()? == "true"))
        })
        .filter(|(id, _, _)| !had.contains(id))
        .map(|(id, def, by_statement)| (id.to_string(), def.to_string(), by_statement))
        .collect()
}

/// An index's statement, made only where it isn't there already.
fn if_not_exists(def: &str) -> String {
    for head in ["CREATE UNIQUE INDEX ", "CREATE INDEX "] {
        if let Some(rest) = def.strip_prefix(head) {
            return format!("{head}IF NOT EXISTS {rest}");
        }
    }
    def.to_string()
}

/// A typed table's column, which is its type's attribute.
const TYPED: &str = "typed";
/// An attribute of a composite type a table is of, which changes on that table too.
const CASCADE: &str = "cascade";

/// The index a primary key or unique constraint keeps its rows in, and that index's storage
/// parameters, which the key's definition writes after its columns (an exclusion constraint's
/// definition has them already).
const INDEX_OF: &str = "index of ";
const INDEX_OPTIONS: &str = "index options ";

/// A key's definition as `pg_get_constraintdef` writes it, with its index's storage parameters
/// after its columns (and an `INCLUDE` list), where `add constraint` takes them.
fn with_index_options(def: &str, options: Option<&str>) -> String {
    let Some(options) = options.filter(|o| !o.is_empty()) else { return def.to_string() };
    let close = |at: usize| -> Option<usize> {
        let mut depth = 0;
        for (i, ch) in def[at..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(at + i);
                    }
                }
                _ => {}
            }
        }
        None
    };
    let Some(mut end) = def.find('(').and_then(close) else { return def.to_string() };
    if def[end + 1..].starts_with(" INCLUDE (") {
        end = close(end + 1).unwrap_or(end);
    }
    format!("{} WITH ({options}){}", &def[..=end], &def[end + 1..])
}

fn around_line(o: &Object) -> String {
    format!("{AROUND}{}", [o.id.clone(), o.def.replace('\n', " "), o.drop.join("; "), o.create.join("; ")].join(SEP))
}

/// The objects made around a change of `from` to `to` that both have alike, as `(drops, creates)`:
/// what changes of them otherwise is the plan's own.
fn around(from: &Object, to: &Object) -> (Vec<String>, Vec<String>) {
    let lines = |o: &Object| -> BTreeMap<String, (String, String, String)> {
        o.extra
            .iter()
            .filter_map(|l| l.strip_prefix(AROUND))
            .filter_map(|l| {
                let mut parts = l.splitn(4, SEP);
                Some((parts.next()?.to_string(), (parts.next()?.to_string(), parts.next()?.to_string(), parts.next()?.to_string())))
            })
            .collect()
    };
    let (a, b) = (lines(from), lines(to));
    let both: Vec<(&(String, String, String), &(String, String, String))> = a.iter().filter_map(|(id, x)| b.get(id).filter(|y| y.0 == x.0).map(|y| (x, y))).collect();
    (both.iter().map(|(x, _)| x.1.clone()).collect(), both.iter().map(|(_, y)| y.2.clone()).collect())
}

/// What follows an object's definition to name the functions it calls.
const CALLS: &str = "\n-- calls ";

/// A definition without the functions it calls.
pub(super) fn bare(def: &str) -> &str {
    def.split(CALLS).next().unwrap_or(def)
}

/// A column's definition as `create table` and `add column` write it.
fn column_def(c: &Row, attribute: bool) -> String {
    let mut parts = vec![c.text("type").to_string()];
    if let Some(collation) = c.get("collation") {
        parts.push(format!("collate {collation}"));
    }
    if attribute {
        return parts.join(" ");
    }
    let options = c.get("identity_options").map(|o| format!(" ({o})")).unwrap_or_default();
    match (c.text("identity"), c.text("generated")) {
        ("a", _) => parts.push(format!("generated always as identity{options}")),
        ("d", _) => parts.push(format!("generated by default as identity{options}")),
        (_, g) if !g.is_empty() => parts.push(format!("generated always as ({}) {}", c.text("def"), if g == "s" { "stored" } else { "virtual" })),
        _ => {
            if let Some(def) = c.get("def") {
                parts.push(format!("default {def}"));
            }
        }
    }
    if c.flag("notnull") {
        parts.push(if c.flag("noinherit") { "not null no inherit" } else { "not null" }.into());
    }
    parts.join(" ")
}

/// A column's definition read back into its parts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Column {
    ty: String,
    collation: Option<String>,
    default: Option<String>,
    identity: Option<String>,
    generated: Option<String>,
    not_null: bool,
    /// Its NOT NULL kept from the table's children.
    no_inherit: bool,
    /// Its NOT NULL not validated against the rows (Postgres 18).
    not_valid: bool,
}

fn parse_column(def: &str) -> Column {
    let mut c = Column::default();
    let mut rest = bare(def).to_string();
    if let Some(r) = rest.strip_suffix(" not valid") {
        c.not_valid = true;
        rest = r.to_string();
    }
    if let Some(r) = rest.strip_suffix(" no inherit") {
        c.no_inherit = true;
        rest = r.to_string();
    }
    if let Some(r) = rest.strip_suffix(" not null") {
        c.not_null = true;
        rest = r.to_string();
    }
    for marker in [" generated always as identity", " generated by default as identity"] {
        if let Some(at) = rest.find(marker).filter(|at| rest[at + marker.len()..].is_empty() || rest[at + marker.len()..].starts_with(" (")) {
            c.identity = Some(rest[at + 1..].to_string());
            rest = rest[..at].to_string();
        }
    }
    if let Some(at) = rest.find(" generated always as (") {
        c.generated = Some(rest[at + 1..].to_string());
        rest = rest[..at].to_string();
    }
    if let Some(at) = rest.find(" default ") {
        c.default = Some(rest[at + 9..].to_string());
        rest = rest[..at].to_string();
    }
    if let Some(at) = rest.find(" collate ") {
        c.collation = Some(rest[at + 9..].to_string());
        rest = rest[..at].to_string();
    }
    c.ty = rest;
    c
}

/// A column's definition written back from its parts, as `column_def` writes it.
fn render_column(c: &Column) -> String {
    let mut parts = vec![c.ty.clone()];
    if let Some(collation) = &c.collation {
        parts.push(format!("collate {collation}"));
    }
    if let Some(identity) = &c.identity {
        parts.push(identity.clone());
    } else if let Some(generated) = &c.generated {
        parts.push(generated.clone());
    } else if let Some(default) = &c.default {
        parts.push(format!("default {default}"));
    }
    if c.not_null {
        parts.push(if c.no_inherit { "not null no inherit" } else { "not null" }.into());
    }
    parts.join(" ")
}

/// An identity's kind (`always`, `by default`) and its sequence's options as `column_def` writes
/// them: `generated by default as identity (increment by 10 cycle)`.
fn identity_parts(identity: &str) -> (String, BTreeMap<String, String>) {
    let (head, options) = identity.split_once(" (").map(|(h, o)| (h, o.trim_end_matches(')'))).unwrap_or((identity, ""));
    let kind = head.trim_start_matches("generated ").trim_end_matches(" as identity").to_string();
    let words: Vec<&str> = options.split_whitespace().collect();
    let mut out = BTreeMap::new();
    let mut k = 0;
    while k < words.len() {
        match words[k] {
            "start" | "increment" if k + 2 < words.len() => {
                out.insert(format!("{} {}", words[k], words[k + 1]), words[k + 2].to_string());
                k += 3;
            }
            "minvalue" | "maxvalue" | "cache" if k + 1 < words.len() => {
                out.insert(words[k].to_string(), words[k + 1].to_string());
                k += 2;
            }
            word => {
                out.insert(word.to_string(), String::new());
                k += 1;
            }
        }
    }
    (kind, out)
}

/// A create statement of a function or procedure that replaces one of its signature.
fn or_replace(create: &str) -> String {
    let re = regex::Regex::new(r"(?is)^create\s+(function|procedure)\b").expect("a valid pattern");
    if re.is_match(create) {
        format!("{} or replace{}", &create[..6], &create[6..])
    } else {
        create.to_string()
    }
}

/// An enum whose labels are removed or reordered, swapped for a new type: the old one renamed
/// aside, the new one created, each column of it converted through its text (a row holding a
/// removed label fails the conversion, and the file with it), and the old one dropped.
fn swap_enum(name: &str, create: &str, columns: &[String], targets: &[String]) -> Vec<String> {
    let (schema, bare) = split_last(name);
    let aside = if bare.ends_with('"') { format!("{}__old\"", bare.trim_end_matches('"')) } else { format!("{bare}__old") };
    let old = if schema.is_empty() { aside.clone() } else { format!("{schema}.{aside}") };
    let mut out = vec![format!("alter type {name} rename to {aside}"), create.to_string()];
    for c in columns.iter().filter(|c| is_column_line(c)) {
        let parts: Vec<&str> = c.splitn(4, SEP).collect();
        let [table, column, ty, default] = parts[..] else { continue };
        let text = if ty.ends_with("[]") { "text[]" } else { "text" };
        if !default.is_empty() {
            out.push(format!("alter table {table} alter column {column} drop default"));
        }
        out.push(format!("alter table {table} alter column {column} type {ty} using {column}::{text}::{ty}"));
        // A default the target keeps is set back; one it changes is the column's own change to
        // make, after what it calls, and one the old type alone takes can't be set back at all.
        let kept = targets.iter().find(|t| t.splitn(3, SEP).take(2).eq(c.splitn(3, SEP).take(2))).and_then(|t| t.splitn(4, SEP).nth(3)) == Some(default);
        if !default.is_empty() && kept {
            let restored = default.replace(&format!("::{old}"), &format!("::{name}")).replace(&format!("::{aside}"), &format!("::{name}"));
            out.push(format!("alter table {table} alter column {column} set default {restored}"));
        }
    }
    out.push(format!("drop type {old}"));
    out
}

/// What an enum made again (set aside as `<name>__old`) takes to its new type, as the
/// statements after the type is made and those before the old one is dropped: each domain over it
/// both schemas have made again as the target has it (the plan leaves it to this), with its grants
/// and comment, and each composite type's attribute of it both have. One only the target has is
/// made after, and one only the source has dropped before, by the plan.
fn carried_over(from: &Object, to: &Object) -> (Vec<String>, Vec<String>) {
    let (mut before, mut after) = (Vec::new(), Vec::new());
    // A column leaving a domain over the type for the type itself moves to the new domain first,
    // and from there to the type as the plan changes it.
    let of_type: Vec<(String, String)> = to.extra.iter().filter(|l| is_column_line(l)).filter_map(|l| {
        let mut f = l.split(SEP);
        Some((f.next()?.to_string(), f.next()?.to_string()))
    }).collect();
    let both = |a: &[String], b: &[String]| -> Vec<Moved> {
        let mut moved = columns_both(a, b);
        for c in a.chunks(4).filter(|c| c.len() == 4) {
            if of_type.iter().any(|(t, n)| *t == c[0] && *n == c[1]) && !moved.iter().any(|m| m.table == c[0] && m.column == c[1]) {
                moved.push(Moved { table: c[0].clone(), column: c[1].clone(), ty: c[2].clone(), had_default: !c[3].is_empty(), default: String::new() });
            }
        }
        moved
    };
    let (domains_before, domains_after) = domains_moved(from, to, &both);
    before.extend(domains_before);
    after.extend(domains_after);
    before.extend(attributes_moved(from, to));
    (before, after)
}

/// A column moved to another type and back (a type's attribute changing under it): its table,
/// name and type, whether it has a default, which can't be cast with it, and the default the target
/// gives it.
struct Moved {
    table: String,
    column: String,
    ty: String,
    had_default: bool,
    default: String,
}

impl Moved {
    fn defaultless(&self) -> Option<String> {
        self.had_default.then(|| format!("alter table {} alter column {} drop default", self.table, self.column))
    }

    fn default_back(&self) -> Option<String> {
        (!self.default.is_empty()).then(|| format!("alter table {} alter column {} set default {}", self.table, self.column, self.default))
    }
}

/// The lines an object carries under `what`, each split into its fields.
fn carried(o: &Object, what: &str) -> Vec<Vec<String>> {
    o.extra.iter().filter_map(|l| l.strip_prefix(what)).map(|l| l.split(SEP).map(String::from).collect()).collect()
}

/// The domains over a type made again that `from` and `to` both carry, made again as the target
/// has them (the old set aside and dropped at the end), with their grants and comment, and their
/// columns moved to them: the statements before the old type is dropped, and the drops.
fn domains_moved(from: &Object, to: &Object, both: &dyn Fn(&[String], &[String]) -> Vec<Moved>) -> (Vec<String>, Vec<String>) {
    let (mut before, mut after) = (Vec::new(), Vec::new());
    let had = carried(from, DOMAIN);
    for d in carried(to, DOMAIN) {
        let [domain, create, metadata, users @ ..] = &d[..] else { continue };
        let Some([_, _, _, had_users @ ..]) = had.iter().find(|h| h.first() == Some(domain)).map(|h| &h[..]) else { continue };
        let (schema, bare_domain) = split_last(domain);
        let set_aside = if bare_domain.ends_with('"') { format!("{}__old\"", bare_domain.trim_end_matches('"')) } else { format!("{bare_domain}__old") };
        before.push(format!("alter domain {domain} rename to {set_aside}"));
        before.extend(create.split(";\n").map(String::from));
        before.extend(metadata.split(";\n").filter(|m| !m.is_empty()).map(String::from));
        for m in both(had_users, users) {
            before.extend(m.defaultless());
            before.push(format!("alter table {} alter column {} type {} using {}::text::{}", m.table, m.column, m.ty, m.column, m.ty));
            before.extend(m.default_back());
        }
        after.push(format!("drop domain {}", if schema.is_empty() { set_aside } else { format!("{schema}.{set_aside}") }));
    }
    (before, after)
}

/// The composite types' attributes `from` and `to` both carry made the type the target gives
/// them again (the type made again under its name), each composite type's together, its columns
/// made text once around them.
fn attributes_moved(from: &Object, to: &Object) -> Vec<String> {
    let had = carried(from, ATTRIBUTE);
    let mut composites: BTreeMap<String, (Vec<String>, Vec<Moved>)> = BTreeMap::new();
    for a in carried(to, ATTRIBUTE) {
        let [composite, attribute, ty, users @ ..] = &a[..] else { continue };
        let Some([_, _, _, had_users @ ..]) = had.iter().find(|h| h.first() == Some(composite) && h.get(1) == Some(attribute)).map(|h| &h[..]) else { continue };
        let entry = composites.entry(composite.clone()).or_insert_with(|| (Vec::new(), columns_both(had_users, users)));
        entry.0.push(format!("alter type {composite} alter attribute {attribute} type {ty}"));
    }
    composites.values().flat_map(|(statements, columns)| through_text(columns, statements)).collect()
}

/// The columns of the composite types whose attributes `from` and `to` both carry.
fn attribute_columns(from: &Object, to: &Object) -> Vec<Moved> {
    let had = carried(from, ATTRIBUTE);
    carried(to, ATTRIBUTE).iter().filter_map(|a| had.iter().find(|h| h.get(..2) == a.get(..2)).map(|h| columns_both(h.get(3..).unwrap_or_default(), a.get(3..).unwrap_or_default()))).flatten().collect()
}

/// The columns of its own a domain carries, as fields.
fn columns_of(o: &Object) -> Vec<String> {
    o.extra.iter().find_map(|l| l.strip_prefix(COLUMNS_OF)).map(|l| l.split(SEP).map(String::from).collect()).unwrap_or_default()
}

/// Whether a domain's base type changes, which makes it again.
fn domain_rebased(from: &Object, to: &Object) -> bool {
    let base = |o: &Object| o.extra.iter().find_map(|l| l.strip_prefix("base ")).map(String::from);
    from.def.starts_with("domain ") && to.def.starts_with("domain ") && base(from) != base(to)
}

/// The columns a composite type or its attribute carries, as fields.
fn used_by(o: &Object) -> Vec<String> {
    o.extra.iter().find_map(|l| l.strip_prefix(USED_BY)).map(|l| l.split(SEP).map(String::from).collect()).unwrap_or_default()
}

/// The columns (table, name, type and default) that lines of both sides carry, which are there to
/// move, each with the target's default.
fn columns_both(a: &[String], b: &[String]) -> Vec<Moved> {
    let columns = |list: &[String]| -> Vec<Vec<String>> { list.chunks(4).filter(|c| c.len() == 4).map(|c| c.to_vec()).collect() };
    let b = columns(b);
    columns(a)
        .into_iter()
        .filter_map(|c| {
            let target = b.iter().find(|d| d[0] == c[0] && d[1] == c[1])?;
            Some(Moved { table: c[0].clone(), column: c[1].clone(), ty: c[2].clone(), had_default: !c[3].is_empty(), default: target[3].clone() })
        })
        .collect()
}

/// Whether a domain's change checks its values again: a check added or changed, or not null set.
fn domain_validates(from: &Object, to: &Object) -> bool {
    if !from.def.starts_with("domain ") || !to.def.starts_with("domain ") {
        return false;
    }
    let checks = |o: &Object| -> BTreeSet<String> { o.extra.iter().filter(|l| l.starts_with("check ")).cloned().collect() };
    let not_null = |o: &Object| o.extra.iter().any(|l| l == "not null");
    !checks(to).is_subset(&checks(from)) || (not_null(to) && !not_null(from))
}

/// `statements` with each column made text before them and its type again after: the change of a
/// type's attribute, which Postgres refuses while a column has the type.
fn through_text(columns: &[Moved], statements: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in columns {
        out.extend(m.defaultless());
        out.push(format!("alter table {} alter column {} type text using {}::text", m.table, m.column, m.column));
    }
    out.extend(statements.iter().cloned());
    for m in columns {
        out.push(format!("alter table {} alter column {} type {} using {}::text::{}", m.table, m.column, m.ty, m.column, m.ty));
        out.extend(m.default_back());
    }
    out
}

/// Where a composite type's attributes would end in another order than the target's (an attribute
/// added before others, which Postgres adds last, or several added, which the plan would add by
/// their names), the attributes from the first out of place on, which are dropped and added again
/// in the target's order, the statements, and the columns they move. Where attributes the type's
/// columns hold are dropped, the columns move to a copy of the type as it is, through their text,
/// and back by each attribute's name, which keeps every value as it was; an array of the type,
/// which no single expression takes back by name, leaves the change refused (None).
fn reorder_attributes(from: &Object, to: &Object) -> Option<(Vec<String>, Vec<String>, Vec<String>)> {
    let members = |o: &Object| -> Vec<(String, String)> { o.extra.iter().filter_map(|l| l.strip_prefix(MEMBER)).filter_map(|l| l.split_once(SEP)).map(|(n, d)| (n.to_string(), d.to_string())).collect() };
    let (a, b) = (members(from), members(to));
    let names = |m: &[(String, String)]| -> Vec<String> { m.iter().map(|(n, _)| n.clone()).collect() };
    let (an, bn) = (names(&a), names(&b));
    let expected: Vec<&String> = an.iter().filter(|n| bn.contains(n)).chain(bn.iter().filter(|n| !an.contains(n))).collect();
    // Several attributes added at the end are added here too, in the target's order, which the
    // plan would otherwise give by their names.
    let added = bn.iter().filter(|n| !an.contains(n)).count();
    let first = expected.iter().zip(&bn).position(|(x, y)| *x != y).or_else(|| (added > 1).then(|| bn.len() - added))?;
    // The type's columns move only where attributes they hold are dropped and added again.
    let dropped = bn[first..].iter().any(|n| an.contains(n));
    let moved_columns = if dropped { columns_both(&used_by(from), &used_by(to)) } else { Vec::new() };
    if moved_columns.iter().any(|m| m.ty.ends_with("[]")) {
        return None;
    }
    let name = to.id.strip_prefix("type ")?;
    let (schema, bare) = split_last(name);
    let aside = if bare.ends_with('"') { format!("{}__old\"", bare.trim_end_matches('"')) } else { format!("{bare}__old") };
    let aside = if schema.is_empty() { aside } else { format!("{schema}.{aside}") };
    let mut out = Vec::new();
    // The copy is of the type as it is when this runs, after the plan drops what the target lacks.
    if !moved_columns.is_empty() {
        out.push(format!("create type {aside} as ({})", a.iter().filter(|(n, _)| bn.contains(n)).map(|(n, d)| format!("{n} {d}")).collect::<Vec<_>>().join(", ")));
    }
    for m in &moved_columns {
        out.extend(m.defaultless());
        out.push(format!("alter table {} alter column {} type {aside} using {}::text::{aside}", m.table, m.column, m.column));
    }
    for n in bn[first..].iter().filter(|n| an.contains(n)) {
        out.push(format!("alter type {name} drop attribute {n}"));
    }
    for (n, d) in &b[first..] {
        out.push(format!("alter type {name} add attribute {n} {d}"));
    }
    for m in &moved_columns {
        let fields: Vec<String> = b.iter().map(|(n, d)| if an.contains(n) { format!("({}).{n}::{d}", m.column) } else { format!("null::{d}") }).collect();
        out.push(format!("alter table {} alter column {} type {} using case when {} is null then null else row({})::{} end", m.table, m.column, m.ty, m.column, fields.join(", "), m.ty));
        out.extend(m.default_back());
    }
    if !moved_columns.is_empty() {
        out.push(format!("drop type {aside}"));
    }
    let columns = moved_columns.iter().map(|m| format!("column {}.{}", m.table, m.column)).collect();
    Some((bn[first..].to_vec(), out, columns))
}

/// How an enum's labels change, from `from` to `to`.
struct EnumChange {
    /// The labels before and after, each as written.
    before: Vec<String>,
    after: Vec<String>,
    /// Whether the target uses a label it adds (a default, a check), which a label added in a
    /// transaction can't be in it, as one of a type made in it can.
    used: bool,
    /// Whether a label goes or moves.
    reorders: bool,
    /// Whether a table is partitioned by the type, which then stays: a label used is added, and
    /// committed before what uses it, a migration of its own.
    keyed: bool,
}

impl EnumChange {
    fn of(from: &Object, to: &Object) -> EnumChange {
        let labels = |def: &str| -> Vec<String> { literal_list(def.trim_start_matches("enum (").trim_end_matches(')')) };
        let (a, b) = (labels(&from.def), labels(&to.def));
        let kept: Vec<&String> = b.iter().filter(|l| a.contains(l)).collect();
        let used = b.iter().filter(|l| !a.contains(l)).any(|l| to.extra.iter().filter(|x| !x.starts_with(AGAIN)).any(|x| names_label(x, l)));
        let reorders = kept.len() != a.len() || kept.iter().zip(&a).any(|(x, y)| *x != y);
        let keyed = to.extra.iter().any(|l| l == PARTITION_KEY);
        EnumChange { before: a, after: b, used, reorders, keyed }
    }

    /// Whether the type is made again, rather than given its new labels in place.
    fn remakes(&self) -> bool {
        (self.used && !self.keyed) || self.reorders
    }
}

/// The element of an array first in its storage order, whatever its bounds and its dimensions
/// (Postgres keeps at most six): a subscript of 1 is not it where the bounds start elsewhere
/// (`'[0:1]={10,20}'`), and is null for an array of more than one dimension.
fn first_element(name: &str) -> String {
    let arms: Vec<String> = (1..=6).map(|n| format!("when {n} then {name}{}", (1..=n).map(|d| format!("[array_lower({name}, {d})]")).collect::<String>())).collect();
    format!("case array_ndims({name}) {} end", arms.join(" "))
}

pub struct PostgresAlterer {
    /// The server's version (`server_version_num`): Postgres 17 changes a generated column's expression in place.
    pub version: i64,
    /// The casts the server makes in assignment (`pg_cast`'s implicit and assignment ones), each a
    /// source and a target type as `format_type` names them, without a modifier.
    pub assignment: BTreeSet<(String, String)>,
    /// The string types (`typcategory` S): any type converts to one in assignment, through its text
    /// form, with no `pg_cast` row.
    pub strings: BTreeSet<String>,
    /// Every other cast the server has, made only when written (`::`).
    pub explicit: BTreeSet<(String, String)>,
}

impl PostgresAlterer {
    /// The casts the server behind `db` makes in assignment.
    pub async fn read(db: &mut Db, version: i64) -> Result<PostgresAlterer, Error> {
        let rows = db.rows("select format_type(castsource, null) as s, format_type(casttarget, null) as t, castcontext as c from pg_cast").await?;
        let assignment = rows.iter().filter(|r| r.text("c") != "e").map(|r| (r.text("s").to_string(), r.text("t").to_string())).collect();
        let explicit = rows.iter().filter(|r| r.text("c") == "e").map(|r| (r.text("s").to_string(), r.text("t").to_string())).collect();
        let strings = db.rows("select format_type(oid, null) as t from pg_type where typcategory = 'S'").await?.iter().map(|r| r.text("t").to_string()).collect();
        Ok(PostgresAlterer { version, assignment, strings, explicit })
    }

    /// Whether a written cast takes a value of type `from` to `to`: one the server has, or the
    /// conversion of a string type's text into any type.
    fn casts(&self, from: &str, to: &str) -> bool {
        static MODIFIER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(\d+(,\d+)?\)").expect("a valid pattern"));
        let bare = |t: &str| MODIFIER.replace_all(t, "").into_owned();
        let (from, to) = (bare(from), bare(to));
        let (from_elem, to_elem) = (from.trim_end_matches("[]"), to.trim_end_matches("[]"));
        self.assigns(&from, &to) || self.strings.contains(from_elem) || self.explicit.contains(&(from_elem.to_string(), to_elem.to_string()))
    }

    /// Whether a column of type `from` becomes `to` without an explicit cast: the same type with
    /// another modifier, or a cast the server makes in assignment, element by element for arrays.
    /// That conversion refuses a value the new type cannot hold (a string too long for its
    /// `varchar(n)`, an integer out of range), where an explicit cast would cut it to fit.
    fn assigns(&self, from: &str, to: &str) -> bool {
        static MODIFIER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(\d+(,\d+)?\)").expect("a valid pattern"));
        let bare = |t: &str| MODIFIER.replace_all(t, "").into_owned();
        let (from, to) = (bare(from), bare(to));
        let (from_elem, to_elem) = (from.trim_end_matches("[]"), to.trim_end_matches("[]"));
        if from.ends_with("[]") != to.ends_with("[]") {
            return false;
        }
        from_elem == to_elem || self.strings.contains(to_elem) || self.assignment.contains(&(from_elem.to_string(), to_elem.to_string()))
    }
}

/// A Postgres column type, as format_type writes it, for a change that may lose values.
fn type_class(ty: &str) -> TypeClass {
    static PARAMS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(([0-9]+)(?:,\s*([0-9]+))?\)").expect("a valid pattern"));
    let ty = ty.trim().to_lowercase();
    if ty.ends_with("[]") {
        return TypeClass::Other;
    }
    let params = PARAMS.captures(&ty).map(|c| (c[1].parse::<u32>().unwrap_or(0), c.get(2).and_then(|s| s.as_str().parse::<u32>().ok())));
    let base = PARAMS.replace(&ty, "").to_string();
    match base.as_str() {
        "smallint" => TypeClass::Int(16),
        "integer" => TypeClass::Int(32),
        "bigint" => TypeClass::Int(64),
        "real" => TypeClass::Float(6),
        "double precision" => TypeClass::Float(15),
        "numeric" => TypeClass::Numeric(params.map(|(_, s)| s.unwrap_or(0))),
        "date" => TypeClass::Date,
        "timestamp with time zone" | "timestamp without time zone" => TypeClass::Timestamp { zone: base.ends_with("with time zone"), precision: params.map_or(6, |(p, _)| p) },
        "time with time zone" | "time without time zone" => TypeClass::Time { precision: params.map_or(6, |(p, _)| p) },
        "text" | "character varying" | "character" | "name" | "citext" | "\"char\"" => TypeClass::Text,
        "json" => TypeClass::Json,
        "jsonb" => TypeClass::Jsonb,
        "interval" => TypeClass::Interval(None),
        b if b.starts_with("interval ") => TypeClass::Interval(Some(b["interval ".len()..].trim().to_string())),
        _ => TypeClass::Other,
    }
}

impl Alterer for PostgresAlterer {
    /// A SQL function's body is checked as it's made, against the functions, procedures and
    /// operators it calls, but the server records none of what it calls (a `begin atomic` body
    /// aside), so where the plan makes those too it can't put them first. Its bodies are checked when
    /// they run instead, as pg_dump makes them; the replay of schema.sql has already run each one.
    fn finish(&self, statements: &mut Vec<String>) {
        static ROUTINE: std::sync::LazyLock<regex::Regex> =
            std::sync::LazyLock::new(|| regex::Regex::new(r"(?is)^create\s+(or\s+replace\s+)?(function|procedure|operator)\b").expect("a valid pattern"));
        static SQL_BODY: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"(?is)\blanguage\s+sql\b").expect("a valid pattern"));
        let checked = |s: &String| ROUTINE.is_match(s) && SQL_BODY.is_match(s) && !s.to_ascii_lowercase().contains("begin atomic");
        let routines = statements.iter().filter(|s| ROUTINE.is_match(s)).count();
        if routines < 2 || !statements.iter().any(checked) {
            return;
        }
        statements.insert(0, "set check_function_bodies = off".into());
        statements.push("reset check_function_bodies".into());
    }

    fn narrows(&self, from: &Object, to: &Object) -> Option<String> {
        let (a, b) = (parse_column(&from.def).ty, parse_column(&to.def).ty);
        // An array's elements change as values of their type do, a value made an array of one
        // among them.
        if a.ends_with("[]") && !b.ends_with("[]") {
            return Some(format!("changes from {a} to {b}, which keeps only each array's first element"));
        }
        let element = |t: &str| t.strip_suffix("[]").unwrap_or(t).to_string();
        let (x, y) = if b.ends_with("[]") { (element(&a), element(&b)) } else { (a.clone(), b.clone()) };
        let why = super::narrowing(&type_class(&x), &type_class(&y))?;
        Some(format!("changes from {a} to {b}, which {why}"))
    }

    fn remakes(&self, from: &Object, to: &Object) -> bool {
        match from.kind {
            Some(Kind::Type) => (from.def.starts_with("enum (") && to.def.starts_with("enum (") && EnumChange::of(from, to).remakes()) || domain_rebased(from, to),
            // A column's type (or collation) changes only with no function body reading it.
            Some(Kind::Column) => {
                let (a, b) = (parse_column(&from.def), parse_column(&to.def));
                a.ty != b.ty || a.collation != b.collation
            }
            _ => false,
        }
    }

    fn kept_from(&self, from: &Object, to: &Object, gone: &Object) -> Option<Vec<String>> {
        // A column a table only inherits from this parent alone, which it keeps where the parent's
        // goes: the table leaves the parent and takes it again, which leaves the column its own.
        let _ = to;
        if from.kind != Some(Kind::Column) || !from.extra.iter().any(|l| l == "inherited") || from.deps.iter().any(|d| *d != gone.id && d.starts_with("column ")) {
            return None;
        }
        let table = from.owner.as_deref()?.strip_prefix("table ")?;
        let parent = gone.owner.as_deref()?.strip_prefix("table ")?;
        Some(vec![format!("alter table {table} no inherit {parent}"), format!("alter table {table} inherit {parent}")])
    }

    fn releases(&self, from: &Object, to: &Object) -> bool {
        // A partition detached, whose indexes stop being its table's, or a table that stops
        // inheriting from a parent, whose columns of that parent's become its own.
        let part = |o: &Object, what: &str| o.extra.iter().find_map(|l| l.strip_prefix(what)).map(String::from);
        let parents = |o: &Object| part(o, "inherits ").map(|p| names_in(&p)).unwrap_or_default();
        to.kind == Some(Kind::Table) && ((part(from, "partition of ").is_some() && part(from, "partition of ") != part(to, "partition of ")) || parents(from).iter().any(|p| !parents(to).contains(p)))
    }

    fn takes_along(&self, from: &Object, to: &Object) -> Vec<String> {
        if to.kind == Some(Kind::Table) {
            // Those attached as the attach makes them; one attached by a statement of its own runs
            // that statement after, its index there already.
            return made_before_attach(from, to).into_iter().filter(|(_, _, by_statement)| !by_statement).map(|(id, _, _)| id).collect();
        }
        if let Some((moved, _, _)) = reorder_attributes(from, to) {
            let name = from.id.strip_prefix("type ").unwrap_or_default();
            return moved.iter().map(|m| format!("column {name}.{m}")).collect();
        }
        if !self.remakes(from, to) {
            return Vec::new();
        }
        // The domains over an enum both have, which its change makes again from the target's.
        let domains = |o: &Object| -> BTreeSet<String> { o.extra.iter().filter_map(|l| l.strip_prefix(DOMAIN)).filter_map(|l| l.split(SEP).next()).map(|d| format!("type {d}")).collect() };
        domains(from).intersection(&domains(to)).cloned().collect()
    }

    fn converts(&self, from: &Object, to: &Object) -> Vec<String> {
        let mut moved: Vec<Moved> = Vec::new();
        match from.kind {
            Some(Kind::Type) if domain_rebased(from, to) => {
                moved = columns_both(&columns_of(from), &columns_of(to));
                moved.extend(attribute_columns(from, to));
                let had = carried(from, DOMAIN);
                for d in carried(to, DOMAIN) {
                    if let Some(h) = had.iter().find(|h| h.first() == d.first()) {
                        moved.extend(columns_both(h.get(3..).unwrap_or_default(), d.get(3..).unwrap_or_default()));
                    }
                }
            }
            Some(Kind::Type) if self.remakes(from, to) => {
                let had = carried(from, DOMAIN);
                for d in carried(to, DOMAIN) {
                    if let Some(h) = had.iter().find(|h| h.first() == d.first()) {
                        moved.extend(columns_both(h.get(3..).unwrap_or_default(), d.get(3..).unwrap_or_default()));
                    }
                }
                moved.extend(attribute_columns(from, to));
            }
            Some(Kind::Type) if reorder_attributes(from, to).is_some() => return reorder_attributes(from, to).map(|(_, _, columns)| columns).unwrap_or_default(),
            Some(Kind::Column) if from.owner.as_deref().is_some_and(|o| o.starts_with("type ")) => moved = columns_both(&used_by(from), &used_by(to)),
            Some(Kind::Type) if domain_validates(from, to) => moved = columns_both(&used_by(from), &used_by(to)),
            _ => {}
        }
        moved.into_iter().map(|m| format!("column {}.{}", m.table, m.column)).collect()
    }

    fn unhook(&self, from: &Object, to: &Object) -> Option<(Vec<String>, Vec<String>)> {
        let table = from.owner.as_deref()?.strip_prefix("table ")?;
        let name = super::last_written(from.id.strip_prefix("column ")?);
        // A partitioned table's default is its own, as its alterer sets it: each partition's is set
        // apart.
        let only = if to.extra.iter().any(|l| l == "partitioned") && parse_column(&from.def).ty == parse_column(&to.def).ty { "only " } else { "" };
        let at = format!("alter table {only}{table} alter column {name}");
        let drop = parse_column(&from.def).default.map(|_| format!("{at} drop default"));
        let set = parse_column(&to.def).default.map(|d| format!("{at} set default {d}"));
        Some((drop.into_iter().collect(), set.into_iter().collect()))
    }

    fn rename(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        let table = to.owner.as_deref()?.strip_prefix("table ")?;
        let (old, new) = (super::last_written(&from.id), super::last_written(&to.id));
        let mut out = vec![format!("alter table {table} rename column {old} to {new}")];
        // Its not null keeps the name it had, which Postgres 18 made from the column's: renamed
        // where the target's differs.
        if let (Some(a), Some(b)) = (not_null_name(from), not_null_name(to)) {
            if a != b {
                out.push(format!("alter table {table} rename constraint {a} to {b}"));
            }
        }
        Some(out)
    }

    fn alter(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        let (table, name) = match to.kind? {
            Kind::Column => {
                let rest = to.id.strip_prefix("column ")?;
                let name = super::last_written(rest);
                (rest.strip_suffix(name.as_str())?.strip_suffix('.')?.to_string(), name)
            }
            _ => (String::new(), String::new()),
        };
        match to.kind? {
            // A composite type's attribute takes another type in place.
            Kind::Column if to.owner.as_deref().is_some_and(|o| o.starts_with("type ")) => {
                let b = parse_column(&to.def);
                let ty = to.owner.as_deref()?.strip_prefix("type ")?;
                let collate = b.collation.as_deref().map(|c| format!(" collate {c}")).unwrap_or_default();
                let cascade = if to.extra.iter().any(|l| l == CASCADE) { " cascade" } else { "" };
                Some(through_text(&columns_both(&used_by(from), &used_by(to)), &[format!("alter type {ty} alter attribute {name} type {}{collate}{cascade}", b.ty)]))
            }
            // A partition's column changes with its parent's, but for a default or a not null of
            // its own, set after the parent's.
            // A column a parent has too changes type with the parent's: what is its own changes after.
            Kind::Column if to.extra.iter().any(|l| l == "inherited" || l == TYPED) => {
                let (a, b) = (parse_column(&from.def), parse_column(&to.def));
                // What the column declares of its own beside (a generated expression, an identity)
                // changes as any column's, from the column as the parent's change leaves it: of the
                // new type already, with nothing of a type change to do again here.
                if a.generated != b.generated || a.identity != b.identity {
                    let own = |o: &Object, def: String| Object { def, extra: o.extra.iter().filter(|l| *l != "inherited" && *l != TYPED).cloned().collect(), ..o.clone() };
                    let after_parent = Column { ty: b.ty.clone(), collation: b.collation.clone(), ..a.clone() };
                    // Its not null still to validate stays so, which the rendering leaves out.
                    let def = format!("{}{}", render_column(&after_parent), if a.not_valid { " not valid" } else { "" });
                    return self.alter(&own(from, def), &own(to, to.def.clone()));
                }
                let at = format!("alter table {table} alter column {name}");
                let mut out = Vec::new();
                match (&a.default, &b.default) {
                    (Some(_), None) => out.push(format!("{at} drop default")),
                    (x, Some(y)) if x.as_ref() != Some(y) || a.ty != b.ty => out.push(format!("{at} set default {y}")),
                    _ => {}
                }
                if a.not_null != b.not_null {
                    out.push(format!("{at} {} not null", if b.not_null { "set" } else { "drop" }));
                }
                Some(out)
            }
            Kind::Column if to.extra.iter().any(|l| l == "partition") => {
                let (a, b) = (parse_column(&from.def), parse_column(&to.def));
                // A default set on a partitioned table alone, which its partitions say for themselves.
                let only = if to.extra.iter().any(|l| l == "partitioned") { "only " } else { "" };
                let (at, at_default) = (format!("alter table {table} alter column {name}"), format!("alter table {only}{table} alter column {name}"));
                let mut out = Vec::new();
                if a.ty == b.ty && a.generated.is_none() && b.generated.is_none() && a.identity.is_none() && b.identity.is_none() {
                    match (&a.default, &b.default) {
                        (Some(_), None) => out.push(format!("{at_default} drop default")),
                        (x, Some(y)) if x.as_ref() != Some(y) => out.push(format!("{at_default} set default {y}")),
                        _ => {}
                    }
                    // A not null left unvalidated is added as a constraint of its own, which no
                    // column statement writes; setting not null validates one.
                    if b.not_null && b.not_valid && (!a.not_null || !a.not_valid) {
                        if a.not_null {
                            out.push(format!("{at} drop not null"));
                        }
                        let named = to.extra.iter().find_map(|l| l.strip_prefix("not null name ")).map(|n| format!("constraint {n} ")).unwrap_or_default();
                        out.push(format!("alter table {table} add {named}not null {name} not valid"));
                    } else if a.not_null != b.not_null || (a.not_valid && !b.not_valid) {
                        out.push(format!("{at} {} not null", if b.not_null { "set" } else { "drop" }));
                    }
                }
                Some(out)
            }
            // A foreign table's column changes as a table's does, but for the rows it holds none of:
            // a new type takes no `using`, which Postgres refuses there.
            Kind::Column if to.extra.iter().any(|l| l == "foreign") => {
                let plain = Object { extra: to.extra.iter().filter(|l| *l != "foreign").cloned().collect(), ..to.clone() };
                let statements = self.alter(from, &plain)?;
                Some(statements.into_iter().map(|s| if s.contains(" type ") { s.split(" using ").next().unwrap_or(&s).to_string() } else { s }).collect())
            }
            Kind::Column => {
                let (a, mut b) = (parse_column(&from.def), parse_column(&to.def));
                // The text `schema.sql` declared its default and generation with, where it is known.
                let declared = |what: &str| to.extra.iter().find_map(|l| l.strip_prefix(what)).map(String::from);
                let declared_default = declared("default ");
                let declared_generated = declared("generated ");
                let at = format!("alter table {table} alter column {name}");
                let mut out = Vec::new();
                if a.generated != b.generated {
                    // An expression changes in place from Postgres 17; anything else about generation is a new column.
                    match (&a.generated, &b.generated) {
                        (Some(x), Some(y)) if self.version >= 170000 && x.ends_with(" stored") && y.ends_with(" stored") => {
                            let expr = declared_generated.map(|g| format!("({g})")).unwrap_or_else(|| y.trim_start_matches("generated always as ").trim_end_matches(" stored").to_string());
                            out.push(format!("{at} set expression as {expr}"));
                        }
                        // A stored column stops being generated in place, keeping its values.
                        (Some(x), None) if self.version >= 130000 && x.ends_with(" stored") => out.push(format!("{at} drop expression")),
                        _ => return None,
                    }
                }
                let type_changes = a.ty != b.ty || a.collation != b.collation;
                // The same default calling another function is set again.
                let calls = |def: &str| def.split_once(CALLS).map(|(_, c)| c.to_string());
                let recalls = calls(&from.def) != calls(&to.def);
                if recalls && a.generated.is_some() && a.generated == b.generated {
                    return None;
                }
                let reset = type_changes || recalls;
                if a.identity.is_some() && b.identity.is_none() {
                    out.push(format!("{at} drop identity"));
                }
                // Set default replaces one; one is dropped only to go, or around a type change,
                // which would cast it.
                // A partitioned table's default changes on it alone, where its type stays: each
                // partition sets its own, which may differ.
                let at_default = if !type_changes && to.extra.iter().any(|l| l == "partitioned") { format!("alter table only {table} alter column {name}") } else { at.clone() };
                if a.default.is_some() && (b.default.is_none() || type_changes) {
                    out.push(format!("{at_default} drop default"));
                }
                let (unmake, remake) = if type_changes { around(from, to) } else { (vec![], vec![]) };
                if type_changes {
                    out.extend(unmake);
                    let collate = b.collation.as_deref().map(|c| format!(" collate {c}")).unwrap_or_default();
                    let element = b.ty.strip_suffix("[]").filter(|_| !a.ty.ends_with("[]"));
                    let first = a.ty.strip_suffix("[]").filter(|_| !b.ty.ends_with("[]"));
                    if let Some(from_element) = first {
                        // An array becomes its first element, which `narrows` says loses the rest.
                        let first = first_element(&name);
                        let value = if self.assigns(from_element, &b.ty) {
                            first
                        } else if self.casts(from_element, &b.ty) {
                            format!("({first})::{}", b.ty)
                        } else {
                            format!("({first})::text::{}", b.ty)
                        };
                        out.push(format!("{at} type {}{collate} using {value}", b.ty));
                    } else if let Some(element) = element {
                        // A value becomes an array of one element, and a null stays null. Where the
                        // server converts the value in assignment, it does here too, refusing one
                        // the element's type can't hold rather than cutting it to fit.
                        let value = if self.assigns(&a.ty, element) {
                            name.clone()
                        } else if self.casts(&a.ty, element) {
                            format!("{name}::{element}")
                        } else {
                            format!("{name}::text::{element}")
                        };
                        out.push(format!("{at} type {}{collate} using case when {name} is null then null else array[{value}] end", b.ty));
                    } else if self.assigns(&a.ty, &b.ty) {
                        out.push(format!("{at} type {}{collate}", b.ty));
                    } else if self.casts(&a.ty, &b.ty) {
                        out.push(format!("{at} type {}{collate} using {name}::{}", b.ty, b.ty));
                    } else {
                        // No cast between them (one enum to another): through their text.
                        let text = if b.ty.ends_with("[]") { "text[]" } else { "text" };
                        out.push(format!("{at} type {}{collate} using {name}::{text}::{}", b.ty, b.ty));
                    }
                    out.extend(remake);
                }
                if b.default.is_some() && (a.default != b.default || reset) {
                    if let Some(d) = declared_default {
                        b.default = Some(d);
                    }
                    out.push(format!("{at_default} set default {}", b.default.as_deref().unwrap_or_default()));
                }
                // An identity needs its column not null first.
                let mut not_null_done = false;
                if a.identity.is_none() && b.identity.is_some() && b.not_null && !a.not_null && !b.no_inherit {
                    out.push(format!("{at} set not null"));
                    not_null_done = true;
                }
                match (&a.identity, &b.identity) {
                    (None, Some(kind)) => {
                        let named = identity_sequence(to).map(|n| named_identity(kind, &n)).unwrap_or_else(|| kind.clone());
                        out.push(format!("{at} add {named}"));
                        // A new identity starts past the rows the table holds, or its first insert
                        // collides; one that counts down is left where it starts.
                        let (_, options) = identity_parts(kind);
                        let sequence = format!("pg_get_serial_sequence({}, {})", literal(&table), literal(&unquoted(&name)));
                        match (options.get("increment by").map(|i| i.starts_with('-')), options.get("start with").or(options.get("minvalue"))) {
                            (Some(true), _) => {}
                            (_, None) => out.push(format!("select setval({sequence}, coalesce(max({name}), 0) + 1, false) from {table}")),
                            (_, Some(start)) => out.push(format!("select setval({sequence}, greatest(coalesce(max({name}) + 1, {start}), {start}), false) from {table}")),
                        }
                    }
                    (Some(x), Some(y)) if x != y => {
                        let ((kx, ox), (ky, oy)) = (identity_parts(x), identity_parts(y));
                        let mut set = Vec::new();
                        if kx != ky {
                            set.push(format!("set generated {ky}"));
                        }
                        // Each option as it takes effect, its default where none is written: the
                        // bounds follow the direction and the column's type, and the start the bounds.
                        let effective = |o: &BTreeMap<String, String>, ty: &str| -> BTreeMap<String, String> {
                            let (low, high) = match ty {
                                "smallint" => ("-32768", "32767"),
                                "integer" => ("-2147483648", "2147483647"),
                                _ => ("-9223372036854775808", "9223372036854775807"),
                            };
                            let descending = o.get("increment by").is_some_and(|i| i.starts_with('-'));
                            let mut out = BTreeMap::new();
                            out.insert("increment by".to_string(), o.get("increment by").cloned().unwrap_or_else(|| "1".into()));
                            out.insert("minvalue".to_string(), o.get("minvalue").cloned().unwrap_or_else(|| if descending { low.into() } else { "1".into() }));
                            out.insert("maxvalue".to_string(), o.get("maxvalue").cloned().unwrap_or_else(|| if descending { "-1".into() } else { high.into() }));
                            let start = o.get("start with").cloned().unwrap_or_else(|| out[if descending { "maxvalue" } else { "minvalue" }].clone());
                            out.insert("start with".to_string(), start);
                            out.insert("cache".to_string(), o.get("cache").cloned().unwrap_or_else(|| "1".into()));
                            out
                        };
                        let (ex, ey) = (effective(&ox, &a.ty), effective(&oy, &b.ty));
                        for key in ["increment by", "minvalue", "maxvalue", "start with", "cache"] {
                            if ex[key] != ey[key] {
                                set.push(format!("set {key} {}", ey[key]));
                            }
                        }
                        // One whose new bounds share no value with its old ones starts again: every
                        // value it gave lies outside them. Otherwise it keeps its place.
                        let bound = |e: &BTreeMap<String, String>, key: &str| e[key].parse::<i128>().unwrap_or_default();
                        let disjoint = bound(&ey, "maxvalue") < bound(&ex, "minvalue") || bound(&ey, "minvalue") > bound(&ex, "maxvalue");
                        if disjoint {
                            set.push("restart".into());
                        }
                        // Bounds that close in past where the sequence stands move it to the new
                        // bound first, which the change otherwise refuses: every value it gave lies
                        // beyond that bound, so none is given again.
                        let descending = ey["increment by"].starts_with('-');
                        let (edge, beyond) = if descending { ("maxvalue", ">") } else { ("minvalue", "<") };
                        let closes = if descending { bound(&ey, edge) < bound(&ex, edge) } else { bound(&ey, edge) > bound(&ex, edge) };
                        if closes && !disjoint {
                            let sequence = format!("pg_get_serial_sequence({}, {})", literal(&table), literal(&unquoted(&name)));
                            out.push(format!(
                                "select setval(q.seq, {0}, false) from (select {sequence} as seq) q join pg_sequences s on format('%I.%I', s.schemaname, s.sequencename) = q.seq where coalesce(s.last_value, s.start_value) {beyond} {0}",
                                ey[edge]
                            ));
                        }
                        if ox.contains_key("cycle") != oy.contains_key("cycle") {
                            set.push(if oy.contains_key("cycle") { "set cycle".into() } else { "set no cycle".into() });
                        }
                        if !set.is_empty() {
                            out.push(format!("{at} {}", set.join(" ")));
                        }
                    }
                    _ => {}
                }
                // A NOT NULL kept from the table's children is added as a constraint of its own, and
                // so is one left unvalidated, by its name; setting not null validates one.
                if a.not_null && (!b.not_null || a.no_inherit != b.no_inherit || (b.not_valid && !a.not_valid)) {
                    out.push(format!("{at} drop not null"));
                }
                if b.not_null && b.not_valid && (!a.not_null || !a.not_valid) {
                    let nn = to.extra.iter().find_map(|l| l.strip_prefix("not null name ")).unwrap_or_default();
                    out.push(format!("alter table {table} add constraint {nn} not null {name} not valid"));
                } else if b.not_null && (!a.not_null || a.no_inherit != b.no_inherit || (a.not_valid && !b.not_valid)) && !not_null_done {
                    // A not null the target names otherwise than Postgres would is added under its name.
                    let named = to.extra.iter().find_map(|l| l.strip_prefix("not null name ")).filter(|n| !a.not_null && unquoted(n) != default_not_null(&table, &name));
                    out.push(match named {
                        _ if b.no_inherit => format!("alter table {table} add not null {name} no inherit"),
                        Some(n) => format!("alter table {table} add constraint {n} not null {name}"),
                        None => format!("{at} set not null"),
                    });
                }
                Some(out)
            }
            Kind::Type if to.def.starts_with("enum (") && from.def.starts_with("enum (") => {
                // New labels only, the old ones kept in their order: each added beside its neighbour.
                let change = EnumChange::of(from, to);
                let (a, b) = (&change.before, &change.after);
                let name = to.id.strip_prefix("type ")?;
                if change.remakes() {
                    let (unmake, remake) = around(from, to);
                    let mut out = unmake;
                    // The columns both have: one the plan drops is gone before, one it adds comes after.
                    let plain = |l: &&String| is_column_line(l);
                    // A column entering a domain over the type converts with them, and enters the
                    // new domain after, as the plan changes it.
                    let entering = |c: &str| -> bool {
                        let mut f = c.split(SEP);
                        let (Some(t), Some(n)) = (f.next(), f.next()) else { return false };
                        carried(to, DOMAIN).iter().any(|d| d.get(3..).unwrap_or_default().chunks(4).any(|u| u.len() == 4 && u[0] == t && u[1] == n))
                    };
                    let kept: Vec<String> = from.extra.iter().filter(plain).filter(|c| to.extra.iter().filter(plain).any(|t| t.splitn(3, SEP).take(2).eq(c.splitn(3, SEP).take(2))) || entering(c)).cloned().collect();
                    // Each with the default the target gives it, which is set back where unchanged.
                    let targets: Vec<String> = kept.iter().filter_map(|c| to.extra.iter().filter(plain).find(|t| t.splitn(3, SEP).take(2).eq(c.splitn(3, SEP).take(2))).cloned()).collect();
                    let mut swap = swap_enum(name, &to.create[0], &kept, &targets);
                    let last = swap.pop();
                    let (before, after) = carried_over(from, to);
                    let made = swap.drain(..2.min(swap.len())).collect::<Vec<_>>();
                    out.extend(made);
                    out.extend(before);
                    out.extend(swap);
                    out.extend(remake);
                    out.extend(to.extra.iter().filter_map(|l| l.strip_prefix(AGAIN)).map(String::from));
                    out.extend(after);
                    out.extend(last);
                    return Some(out);
                }
                let mut out = Vec::new();
                for (k, label) in b.iter().enumerate() {
                    if a.contains(label) {
                        continue;
                    }
                    // Beside a label there already: the one before it, else the next the type had.
                    match b[..k].iter().rev().find(|l| a.contains(l) || out.iter().any(|s: &String| s.contains(l.as_str()))) {
                        Some(before) => out.push(format!("alter type {name} add value {label} after {before}")),
                        None => match b[k + 1..].iter().find(|l| a.contains(l)) {
                            Some(next) => out.push(format!("alter type {name} add value {label} before {next}")),
                            None => out.push(format!("alter type {name} add value {label}")),
                        },
                    }
                }
                if change.used && change.keyed {
                    out.push(super::COMMIT.to_string());
                }
                Some(out)
            }
            Kind::Type if to.def.starts_with("composite (") && from.def.starts_with("composite (") => Some(reorder_attributes(from, to).map(|(_, statements, _)| statements).unwrap_or_default()),
            Kind::Type if to.def.starts_with("domain ") && from.def.starts_with("domain ") => {
                // A domain's default, NOT NULL and checks change in place; its base type does not.
                let part = |o: &Object, what: &str| o.extra.iter().find_map(|l| l.strip_prefix(what)).map(String::from);
                let name = to.id.strip_prefix("type ")?;
                // Its base type changes by making it again under its name, its columns moved to the
                // new one through their text, and its grants and comment set again.
                if part(from, "base ") != part(to, "base ") {
                    let (schema, bare) = split_last(name);
                    let aside = if bare.ends_with('"') { format!("{}__old\"", bare.trim_end_matches('"')) } else { format!("{bare}__old") };
                    let mut out = vec![format!("alter domain {name} rename to {aside}")];
                    out.extend(to.create.iter().cloned());
                    out.extend(to.extra.iter().filter_map(|l| l.strip_prefix(AGAIN)).map(String::from));
                    let (domains_before, domains_after) = domains_moved(from, to, &columns_both);
                    out.extend(domains_before);
                    out.extend(attributes_moved(from, to));
                    for m in columns_both(&columns_of(from), &columns_of(to)) {
                        out.extend(m.defaultless());
                        out.push(format!("alter table {} alter column {} type {} using {}::text::{}", m.table, m.column, m.ty, m.column, m.ty));
                        out.extend(m.default_back());
                    }
                    out.extend(domains_after);
                    out.push(format!("drop domain {}", if schema.is_empty() { aside } else { format!("{schema}.{aside}") }));
                    return Some(out);
                }
                let at = format!("alter domain {name}");
                let mut out = Vec::new();
                match (part(from, "default "), part(to, "default ")) {
                    (Some(_), None) => out.push(format!("{at} drop default")),
                    (a, Some(b)) if a.as_ref() != Some(&b) => out.push(format!("{at} set default {}", part(to, "declared default ").unwrap_or(b))),
                    _ => {}
                }
                let not_null = |o: &Object| o.extra.iter().any(|l| l == "not null");
                match (not_null(from), not_null(to)) {
                    (true, false) => out.push(format!("{at} drop not null")),
                    (false, true) => out.push(format!("{at} set not null")),
                    _ => {}
                }
                let checks = |o: &Object| -> BTreeMap<String, String> { o.extra.iter().filter_map(|l| l.strip_prefix("check ")).filter_map(name_first).map(|(n, d)| (n.to_string(), d.to_string())).collect() };
                let (a, b) = (checks(from), checks(to));
                for (check, def) in &a {
                    if b.get(check) != Some(def) {
                        out.push(format!("{at} drop constraint {check}"));
                    }
                }
                for (check, def) in &b {
                    if a.get(check) != Some(def) {
                        let declared = to.extra.iter().find_map(|l| l.strip_prefix(&format!("declared check {check} "))).and_then(|text| {
                            let close = declared::closing(def, 6)?;
                            Some(format!("CHECK ({text}){}", &def[close + 1..]))
                        });
                        out.push(format!("{at} add constraint {check} {}", declared.unwrap_or_else(|| def.clone())));
                    }
                }
                // Postgres checks no value inside a composite column against a domain: it refuses
                // the change while such a column has the domain, so the column is text meanwhile,
                // and its values are checked as they come back.
                if domain_validates(from, to) {
                    return Some(through_text(&columns_both(&used_by(from), &used_by(to)), &out));
                }
                Some(out)
            }
            Kind::Sequence => {
                let name = to.id.strip_prefix("sequence ")?;
                Some(vec![format!("alter sequence {name} {}", to.def.replace(" start with ", " start with ").replacen("as ", "as ", 1))])
            }
            Kind::Extension => {
                let name = to.id.strip_prefix("extension ")?;
                Some(vec![format!("alter extension {name} set schema {}", to.def)])
            }
            Kind::Table => {
                // A table's logging, options, partition bound and parent change in place; what it
                // is (partitioned, foreign) does not.
                let name = to.id.strip_prefix("table ")?;
                let part = |o: &Object, what: &str| o.extra.iter().find_map(|l| l.strip_prefix(what)).map(String::from);
                if part(from, "shape ") != part(to, "shape ") {
                    return None;
                }
                let mut out = Vec::new();
                if part(from, "partition of ") != part(to, "partition of ") {
                    if let Some((parent, _)) = part(from, "partition of ").as_deref().and_then(|p| p.split_once(' ')) {
                        out.push(format!("alter table {parent} detach partition {name}"));
                    }
                    if let Some((parent, bound)) = part(to, "partition of ").as_deref().and_then(|p| p.split_once(' ')) {
                        out.extend(made_before_attach(from, to).into_iter().map(|(_, def, _)| def));
                        out.push(format!("alter table {parent} attach partition {name} {bound}"));
                    }
                }
                let (had, has) = (part(from, "inherits ").map(|p| names_in(&p)).unwrap_or_default(), part(to, "inherits ").map(|p| names_in(&p)).unwrap_or_default());
                for parent in had.iter().filter(|p| !has.contains(p)) {
                    out.push(format!("alter table {name} no inherit {parent}"));
                }
                for parent in has.iter().filter(|p| !had.contains(p)) {
                    out.push(format!("alter table {name} inherit {parent}"));
                }
                match (part(from, "of "), part(to, "of ")) {
                    (a, b) if a == b => {}
                    (_, Some(ty)) => out.push(format!("alter table {name} of {ty}")),
                    (Some(_), None) => out.push(format!("alter table {name} not of")),
                    _ => {}
                }
                match (part(from, "persistence ").as_deref(), part(to, "persistence ").as_deref()) {
                    (Some("unlogged"), Some("logged")) => out.push(format!("alter table {name} set logged")),
                    (Some("logged"), Some("unlogged")) => out.push(format!("alter table {name} set unlogged")),
                    _ => {}
                }
                // Storage moves in place, rows rewritten into it, from Postgres 15 (a partitioned
                // table's, from 17); before, the table is created again.
                if let (Some(a), Some(b)) = (part(from, "access method "), part(to, "access method ")) {
                    if a != b {
                        let partitioned = part(to, "shape ").is_some_and(|s| s.contains("partition by"));
                        if self.version < if partitioned { 170000 } else { 150000 } {
                            return None;
                        }
                        out.push(format!("alter table {name} set access method {b}"));
                    }
                }
                let opts = |o: &Object| -> Vec<String> { part(o, "options ").unwrap_or_default().split(", ").filter(|s| !s.is_empty()).map(String::from).collect() };
                let (a, b) = (opts(from), opts(to));
                let reset: Vec<String> = a.iter().filter_map(|o| o.split('=').next()).filter(|k| !b.iter().any(|x| x.starts_with(&format!("{k}=")))).map(String::from).collect();
                if !reset.is_empty() {
                    out.push(format!("alter table {name} reset ({})", reset.join(", ")));
                }
                let set: Vec<&String> = b.iter().filter(|o| !a.contains(o)).collect();
                if !set.is_empty() {
                    out.push(format!("alter table {name} set ({})", set.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
                }
                Some(out)
            }
            Kind::Statistics if to.id.starts_with("publication ") => {
                // What a publication publishes changes in place; whether it is all tables does not.
                let name = to.id.strip_prefix("publication ")?;
                let part = |o: &Object, what: &str| o.extra.iter().find_map(|l| l.strip_prefix(what)).map(String::from).unwrap_or_default();
                if part(from, "all ") != part(to, "all ") {
                    return None;
                }
                let mut out = Vec::new();
                match (part(from, "objects "), part(to, "objects ")) {
                    (a, b) if a == b => {}
                    (a, b) if b.is_empty() => out.push(format!("alter publication {name} drop {}", without_lists(&a))),
                    (_, b) => out.push(format!("alter publication {name} set {b}")),
                }
                if part(from, "with ") != part(to, "with ") {
                    let root = to.extra.iter().any(|l| l.starts_with("with ") && l.contains("publish_via_partition_root = true"));
                    out.push(format!("alter publication {name} set (publish = {}, publish_via_partition_root = {root})", literal(&part(to, "publish "))));
                }
                Some(out)
            }
            Kind::Rls if to.id.starts_with("rls ") => {
                let name = to.id.strip_prefix("rls ")?;
                let has = |o: &Object, what: &str| o.def.split(' ').any(|w| w == what);
                let mut out = Vec::new();
                if has(from, "enable") != has(to, "enable") {
                    out.push(format!("alter table {name} {} row level security", if has(to, "enable") { "enable" } else { "disable" }));
                }
                if has(from, "force") != has(to, "force") {
                    out.push(format!("alter table {name} {} row level security", if has(to, "force") { "force" } else { "no force" }));
                }
                Some(out)
            }
            Kind::Comment | Kind::ColumnSetting => Some(to.create.clone()),
            // A key whose index's storage parameters alone change sets them on the index.
            Kind::Constraint => {
                let part = |o: &Object, what: &str| o.extra.iter().find_map(|l| l.strip_prefix(what)).map(String::from);
                let index = part(to, INDEX_OF)?;
                let options = |o: &Object| -> Vec<String> { part(o, INDEX_OPTIONS).unwrap_or_default().split(", ").filter(|s| !s.is_empty()).map(String::from).collect() };
                let (a, b) = (options(from), options(to));
                let without = |o: &Object, opts: &[String]| if opts.is_empty() { o.def.clone() } else { o.def.replacen(&format!(" WITH ({})", opts.join(", ")), "", 1) };
                if part(from, INDEX_OF) != Some(index.clone()) || without(from, &a) != without(to, &b) {
                    return None;
                }
                let mut out = Vec::new();
                let reset: Vec<&str> = a.iter().filter_map(|o| o.split('=').next()).filter(|k| !b.iter().any(|x| x.split('=').next() == Some(k))).collect();
                if !reset.is_empty() {
                    out.push(format!("alter index {index} reset ({})", reset.join(", ")));
                }
                let set: Vec<&str> = b.iter().filter(|o| !a.contains(o)).map(|s| s.as_str()).collect();
                if !set.is_empty() {
                    out.push(format!("alter index {index} set ({})", set.join(", ")));
                }
                Some(out)
            }
            Kind::Index => {
                // An index of a partition, alone, is attached to its table's index in place.
                let attached = |o: &Object| o.extra.iter().find_map(|l| l.strip_prefix("attached to ")).map(String::from);
                let plain = |o: &Object| bare(&o.def).split("\nattached to ").next().unwrap_or_default().to_string();
                // Under an index the target has valid, the one made with it attaches it as it is made.
                match (attached(from), attached(to)) {
                    (None, Some(_)) if plain(from) == plain(to) && !to.extra.iter().any(|l| l == "attached by statement") => Some(vec![]),
                    (None, Some(parent)) if plain(from) == plain(to) => Some(vec![format!("alter index {parent} attach partition {}", to.id.strip_prefix("index ")?)]),
                    // One whose table is detached stands alone after, as the target has it.
                    (Some(_), None) if plain(from) == plain(to) => Some(vec![]),
                    _ => None,
                }
            }
            Kind::Trigger if to.id.starts_with("event trigger ") => {
                // Only whether it fires changes in place.
                let without = |o: &Object| bare(&o.def).rsplit_once(' ').map(|(d, _)| d.to_string()).unwrap_or_default();
                if without(from) != without(to) {
                    return None;
                }
                let name = to.id.strip_prefix("event trigger ")?;
                Some(vec![match to.extra.iter().find_map(|l| l.strip_prefix("enabled ")) {
                    Some("D") => format!("alter event trigger {name} disable"),
                    Some("R") => format!("alter event trigger {name} enable replica"),
                    Some("A") => format!("alter event trigger {name} enable always"),
                    _ => format!("alter event trigger {name} enable"),
                }])
            }
            Kind::Rule => {
                // Only whether it fires changes in place.
                let (Some((a, _)), Some((b, enabled))) = (bare(&from.def).rsplit_once(' '), bare(&to.def).rsplit_once(' ')) else { return None };
                let rest = to.id.strip_prefix("rule ")?;
                let rule = super::last_written(rest);
                let table = rest.strip_suffix(rule.as_str())?.strip_suffix('.')?;
                (a == b).then(|| {
                    vec![match enabled {
                        "D" => format!("alter table {table} disable rule {rule}"),
                        "R" => format!("alter table {table} enable replica rule {rule}"),
                        "A" => format!("alter table {table} enable always rule {rule}"),
                        _ => format!("alter table {table} enable rule {rule}"),
                    }]
                })
            }
            Kind::Server if to.id.starts_with("foreign data wrapper ") => {
                let name = to.id.strip_prefix("foreign data wrapper ")?;
                let part = |o: &Object, what: &str| o.extra.iter().find_map(|l| l.strip_prefix(what)).unwrap_or_default().to_string();
                let mut changes = Vec::new();
                for what in ["handler", "validator"] {
                    let to_value = part(to, &format!("{what} "));
                    if part(from, &format!("{what} ")) != to_value {
                        changes.push(if to_value.is_empty() { format!("no {what}") } else { format!("{what} {to_value}") });
                    }
                }
                let options = |o: &Object| -> Vec<(String, String)> { serde_json::from_str(&part(o, "options ")).unwrap_or_default() };
                changes.extend(options_change(&options(from), &options(to)));
                Some(vec![format!("alter foreign data wrapper {name} {}", changes.join(" "))])
            }
            Kind::Function => {
                // Create or replace keeps a function whose result and arguments stay.
                let tail = |def: &str| def.rsplit_once("\n-- ").map(|(_, t)| t.to_string()).unwrap_or_default();
                if tail(&from.def) != tail(&to.def) || to.create.first().is_some_and(|c| c.starts_with("create aggregate")) {
                    return None;
                }
                Some(to.create.iter().map(|c| or_replace(c)).collect())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_column_definition_back() {
        let c = parse_column("character varying(20) collate public.\"C\" default 'x'::character varying not null");
        assert_eq!(c.ty, "character varying(20)");
        assert_eq!(c.collation.as_deref(), Some("public.\"C\""));
        assert_eq!(c.default.as_deref(), Some("'x'::character varying"));
        assert!(c.not_null);
        let i = parse_column("bigint generated always as identity not null");
        assert_eq!(i.identity.as_deref(), Some("generated always as identity"));
        let o = parse_column("integer generated by default as identity (increment by 10 minvalue -200 maxvalue 200) not null");
        assert_eq!(o.identity.as_deref(), Some("generated by default as identity (increment by 10 minvalue -200 maxvalue 200)"));
        assert_eq!(o.ty, "integer");
        let (kind, options) = identity_parts(o.identity.as_deref().unwrap());
        assert_eq!(kind, "by default");
        assert_eq!(options.get("increment by").map(String::as_str), Some("10"));
        assert_eq!(options.get("minvalue").map(String::as_str), Some("-200"));
    }

    #[test]
    fn finds_a_label_in_an_arrays_literal() {
        assert!(names_label("t\0v\0e[]\0'{b}'::e[]", "'b'"));
        assert!(names_label("default '{\"b,c\"}'::mood[]", "'b,c'"));
        assert!(!names_label("default '{\"b,c\"}'::mood[]", "'b'"));
        assert!(names_label("default '{\"it''s\"}'::mood[]", "'it''s'"));
        assert_eq!(array_elements(r#"{a, "b,c",{"d\"e",NULL}}"#), ["a", "b,c", "d\"e", "NULL"]);
        assert!(names_label("default '{a,\"b c\"}'::e[]", "'b c'"));
        assert!(!names_label("default '{ab}'::e[]", "'b'"));
        assert!(names_label("check (v <> 'b'::e)", "'b'"));
    }

    #[test]
    fn reads_a_list_of_literals_with_commas_in_them() {
        assert_eq!(literal_list("'a', 'b, c', 'it''s', 'd|e'"), vec!["'a'", "'b, c'", "'it''s'", "'d|e'"]);
    }

    #[test]
    fn a_value_made_an_array_narrows_as_its_element_does() {
        let alterer = PostgresAlterer { version: 180000, assignment: BTreeSet::new(), strings: BTreeSet::new(), explicit: BTreeSet::new() };
        let column = |def: &str| Object { id: "column public.t.c".into(), kind: Some(Kind::Column), def: def.into(), ..Object::default() };
        let narrows = |a: &str, b: &str| alterer.narrows(&column(a), &column(b));
        assert!(narrows("timestamp without time zone", "date[]").is_some_and(|w| w.contains("keeps only the date")));
        assert!(narrows("numeric", "numeric(5,1)[]").is_some_and(|w| w.contains("rounds to 1 decimal places")));
        assert!(narrows("numeric[]", "numeric(5,1)[]").is_some());
        assert_eq!(narrows("integer", "integer[]"), None);
        assert!(narrows("integer[]", "integer").is_some_and(|w| w.contains("first element")));
        assert_eq!(narrows("bytea", "bytea[]"), None);
    }
}
