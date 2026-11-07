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

/// Rewrites the create statements of `schema`, which `read` read from the database `db` is
/// connected to, from the text `sql` declared each object with: `sql` is the file that built that
/// database (`schema.sql`), and its unqualified names resolve through the search_path `db` has.
/// An object the file's text cannot be matched to keeps the server's rendering.
pub async fn declare(db: &mut Db, schema: &mut Schema, sql: &str) -> Result<(), Error> {
    let rows = db.rows("select format('%s', array_to_json(current_schemas(false))) as path").await?;
    let path: Vec<String> = rows.first().and_then(|r| serde_json::from_str(r.text("path")).ok()).unwrap_or_default();
    let found = declared::read(schema, sql, &path);
    declared::apply(schema, &found);
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
            _ => None,
        }
        .or_else(|| self.members.get(&(class.to_string(), oid)).cloned())
    }
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
    let locale_column = if version >= 170000 { "co.colllocale" } else { "co.colliculocale" };
    for r in db
        .rows(&format!(
            "select co.oid, format('%I.%I', n.nspname, co.collname) as name, co.collprovider as provider, co.collisdeterministic as det,
              co.collcollate as lc_collate, co.collctype as lc_ctype, {} as locale, co.collicurules as rules
            from pg_collation co join pg_namespace n on n.oid = co.collnamespace where {ns} and {} and {}",
            locale_column,
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
              (select string_agg(format(' constraint %I %s', c.conname, pg_get_constraintdef(c.oid)), '' order by c.conname) from pg_constraint c where c.contypid = t.oid and c.contype = 'c') as checks,
              format('%s', (select json_agg(json_build_array(format('%I', c.conname), pg_get_constraintdef(c.oid)) order by c.conname) from pg_constraint c where c.contypid = t.oid and c.contype = 'c')) as checklist,
              case when t.typtype = 'd' and t.typcollation <> 0 and t.typcollation <> (select b.typcollation from pg_type b where b.oid = t.typbasetype)
                then (select format('%I.%I', cn.nspname, co.collname) from pg_collation co join pg_namespace cn on cn.oid = co.collnamespace where co.oid = t.typcollation) end as collation,
              case when t.typtype = 'r' then (select concat_ws(', ', 'subtype = ' || format_type(r.rngsubtype, null),
                  case when r.rngsubopc <> 0 and not (select o.opcdefault from pg_opclass o where o.oid = r.rngsubopc) then 'subtype_opclass = ' || (select format('%I.%I', on_.nspname, o.opcname) from pg_opclass o join pg_namespace on_ on on_.oid = o.opcnamespace where o.oid = r.rngsubopc) end,
                  case when r.rngcanonical <> 0 then 'canonical = ' || format('%s', r.rngcanonical::regproc) end,
                  case when r.rngsubdiff <> 0 then 'subtype_diff = ' || format('%s', r.rngsubdiff::regproc) end) from pg_range r where r.rngtypid = t.oid) end as range
            from pg_type t join pg_namespace n on n.oid = t.typnamespace
            where t.typtype in ('e', 'd', 'r') and {ns} and {} and {}",
            not_extension("pg_type", "t"),
            not_platform("t.typowner")
        ))
        .await?
    {
        let name = r.text("name");
        let id = format!("type {name}");
        ids.types.insert(oid(&r, "oid"), id.clone());
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
                (def.clone(), format!("create {}", def.replacen("domain ", &format!("domain {name} as "), 1)), Change::Alter { blocks: false })
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
                extra.push(format!("{}|{}|{}|{}", c.text("rel"), c.text("col"), c.text("type"), c.text("def")));
            }
        }
        s.add(Object { id, kind: Some(Kind::Type), def, create: vec![create], drop: vec![format!("drop type {name}")], change, extra, ..Object::default() });
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
              (select d.refobjsubid from pg_depend d where d.classid = 'pg_class'::regclass and d.objid = c.oid and d.deptype = 'a' and d.refobjsubid > 0) as owned_att
            from pg_sequence s join pg_class c on c.oid = s.seqrelid join pg_namespace n on n.oid = c.relnamespace
            where {ns} and {} and {}
              and not exists (select 1 from pg_depend d where d.classid = 'pg_class'::regclass and d.objid = c.oid and d.deptype = 'i')",
            acl("c.relacl", 's', "c.relowner"),
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
        let goes_with = r.get("owned").and_then(|o| o.rsplit_once('.')).map(|(t, c)| format!("column {t}.{c}"));
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
        if let Some(owned) = r.get("owned") {
            let column = owned.rsplit_once('.').map(|(t, c)| format!("column {t}.{c}")).unwrap_or_default();
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
              (select string_agg(o, ', ' order by o) from unnest(c.reloptions) o) as options,
              (select nullif(a.amname, 'heap') from pg_am a where a.oid = c.relam) as am,
              pg_get_expr(c.relpartbound, c.oid) as bound,
              (select format('%I.%I', pn.nspname, pc.relname) from pg_inherits i join pg_class pc on pc.oid = i.inhparent join pg_namespace pn on pn.oid = pc.relnamespace where i.inhrelid = c.oid order by i.inhseqno limit 1) as parent,
              case when c.relkind = 'p' then pg_get_partkeydef(c.oid) end as partkey,
              case when c.relkind in ('v', 'm') then pg_get_viewdef(c.oid, false) end as view,
              {} as acl,
              obj_description(c.oid, 'pg_class') as comment
            from pg_class c join pg_namespace n on n.oid = c.relnamespace
            where c.relkind in ('r', 'p', 'v', 'm', 'f', 'c') and {ns} and {} and {} and c.relname <> '{history}'
              and not exists (select 1 from pg_depend e where e.classid = 'pg_type'::regclass and e.objid = c.reltype and e.deptype = 'e')",
            acl("c.relacl", 'r', "c.relowner"),
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

    // Constraints of tables (not the not-null ones Postgres 18 records, which the columns carry, and
    // not those a partition takes from its parent), and the indexes they own.
    let mut constraint_indexes: BTreeMap<u32, String> = BTreeMap::new();
    let mut inline_constraints: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let partitions: BTreeSet<u32> = rels.iter().filter(|r| r.get("bound").is_some() || r.text("kind") == "c").map(|r| oid(r, "oid")).collect();
    for r in db
        .rows(&format!(
            "select co.oid, co.conindid, format('%I.%I', n.nspname, c.relname) as rel, c.oid as reloid, format('%I', co.conname) as name,
              pg_get_constraintdef(co.oid, false) as def, co.condeferrable as deferrable, co.condeferred as deferred, co.convalidated as validated, co.contype as type
            from pg_constraint co join pg_class c on c.oid = co.conrelid join pg_namespace n on n.oid = c.relnamespace
            where co.contype <> 'n' and co.conislocal and {ns} and c.relname <> '{history}' and {} and {}",
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
        // The server's rendering says DEFERRABLE and NOT VALID itself.
        let def = r.text("def").to_string();
        // A new table's own constraints are written inside its create table; a foreign key, which
        // names another table, is a statement of its own after both exist.
        let inline = r.text("type") != "f" && r.flag("validated") && !partitions.contains(&oid(&r, "reloid"));
        if inline {
            inline_constraints.entry(oid(&r, "reloid")).or_default().push(format!("constraint {name} {def}"));
        }
        s.add(Object {
            inline,
            id,
            kind: Some(Kind::Constraint),
            owner: Some(table.clone()),
            def: def.clone(),
            create: vec![format!("alter table {rel} add constraint {name} {def}")],
            drop: vec![format!("alter table {rel} drop constraint {name}")],
            ..Object::default()
        });
    }

    // Columns, by relation.
    let mut columns: BTreeMap<u32, Vec<Row>> = BTreeMap::new();
    for r in db
        .rows(&format!(
            "select a.attrelid as rel, a.attnum as num, format('%I', a.attname) as name, format_type(a.atttypid, a.atttypmod) as type, a.attnotnull as notnull,
              pg_get_expr(d.adbin, d.adrelid) as def, d.oid as defoid, a.attidentity as identity, a.attgenerated as generated, a.attislocal as local,
              case when a.attcollation <> t.typcollation then (select format('%I.%I', cn.nspname, co.collname) from pg_collation co join pg_namespace cn on cn.oid = co.collnamespace where co.oid = a.attcollation) end as collation,
              col_description(a.attrelid, a.attnum) as comment, {} as noinherit
            from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace join pg_type t on t.oid = a.atttypid
            left join pg_attrdef d on d.adrelid = a.attrelid and d.adnum = a.attnum
            where a.attnum > 0 and not a.attisdropped and c.relkind in ('r', 'p', 'v', 'm', 'f', 'c') and {ns} and {} and {} and c.relname <> '{history}'
            order by a.attrelid, a.attnum",
            // Postgres 18 records a column's NOT NULL as a constraint, which may be kept from the
            // table's children (NO INHERIT).
            if version >= 180000 { "coalesce((select co.connoinherit from pg_constraint co where co.conrelid = a.attrelid and co.contype = 'n' and co.conkey = array[a.attnum]), false)" } else { "false" },
            not_extension("pg_class", "c"),
            not_platform("c.relowner")
        ))
        .await?
    {
        columns.entry(oid(&r, "rel")).or_default().push(r);
    }

    for r in &rels {
        let rel = oid(r, "oid");
        let name = r.text("name").to_string();
        let kind = r.text("kind");
        let id = relations[&rel].0.clone();
        let cols = columns.remove(&rel).unwrap_or_default();
        let partition = r.get("bound").is_some();
        let mut col_defs = Vec::new();
        for c in &cols {
            let col_id = format!("column {name}.{}", c.text("name"));
            let target = if matches!(kind, "v" | "m") { id.clone() } else { col_id.clone() };
            ids.column.insert((rel, c.text("num").parse().unwrap_or(0)), target);
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
            if matches!(kind, "v" | "m") {
                continue;
            }
            let def = column_def(c, kind == "c");
            if c.flag("local") || !partition {
                col_defs.push(format!("{} {def}", c.text("name")));
            }
            if c.flag("local") {
                let caution = (c.flag("notnull") && c.get("def").is_none() && c.text("identity").is_empty() && kind != "c")
                    .then(|| format!("{} is not null with no default, so adding it fails on a table that has rows: give it a default, or add it nullable, fill it, then set not null", c.text("name")));
                s.add(Object {
                    caution,
                    id: col_id,
                    kind: Some(Kind::Column),
                    owner: Some(id.clone()),
                    def: def.clone(),
                    create: vec![if kind == "c" { format!("alter type {name} add attribute {} {def}", c.text("name")) } else { format!("alter table {name} add column {} {def}", c.text("name")) }],
                    drop: vec![if kind == "c" { format!("alter type {name} drop attribute {}", c.text("name")) } else { format!("alter table {name} drop column {}", c.text("name")) }],
                    inline: true,
                    change: Change::Alter { blocks: true },
                    ..Object::default()
                });
            } else if partition {
                // A partition's column is its parent's, made, changed and dropped with it: an
                // object with no statement of its own, so that a partition detached keeps it.
                s.add(Object {
                    id: col_id,
                    kind: Some(Kind::Column),
                    owner: Some(id.clone()),
                    def: def.clone(),
                    inline: true,
                    change: Change::Alter { blocks: true },
                    extra: vec!["partition".into()],
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
            "c" => (String::new(), format!("create type {name} as (\n  {}\n)", col_defs.join(",\n  ")), format!("drop type {name}")),
            _ => {
                let unlogged = if r.text("persistence") == "u" { "unlogged " } else { "" };
                let foreign = if kind == "f" { "foreign " } else { "" };
                let head = format!("create {unlogged}{foreign}table {name}");
                let body = match (r.get("bound"), r.get("parent")) {
                    (Some(bound), Some(parent)) => format!("{head} partition of {parent} {bound}"),
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
                (extra.join("\n"), format!("{body}{partkey}{using}{server}{options}"), format!("drop {foreign}table {name}"))
            }
        };
        let object_kind = match kind {
            "v" | "m" => Kind::View,
            "c" => Kind::Type,
            _ => Kind::Table,
        };
        // A table's own properties change in place; a view or a composite type is replaced.
        let change = if object_kind == Kind::Table { Change::Alter { blocks: false } } else { Change::Replace };
        s.add(Object { id: id.clone(), kind: Some(object_kind), def, create: vec![create], drop: vec![drop], change, extra, ..Object::default() });
        if r.flag("rls") {
            s.add(Object {
                id: format!("rls {name}"),
                kind: Some(Kind::Rls),
                owner: Some(id.clone()),
                def: if r.flag("force") { "force".into() } else { String::new() },
                create: [Some(format!("alter table {name} enable row level security")), r.flag("force").then(|| format!("alter table {name} force row level security"))].into_iter().flatten().collect(),
                drop: vec![format!("alter table {name} disable row level security"), format!("alter table {name} no force row level security")],
                change: Change::Alter { blocks: false },
                ..Object::default()
            });
        }
        grants(&mut s, r.text("acl"), &name, &id, "table");
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

    for r in db
        .rows(&format!(
            "select i.indexrelid as oid, i.indrelid as rel, format('%I.%I', n.nspname, c.relname) as name, pg_get_indexdef(i.indexrelid) as def,
              exists (select 1 from pg_inherits h where h.inhrelid = i.indexrelid) as inherited
            from pg_index i join pg_class c on c.oid = i.indexrelid join pg_namespace n on n.oid = c.relnamespace join pg_class t on t.oid = i.indrelid
            where {ns} and {} and {} and t.relname <> '{history}'",
            not_extension("pg_class", "c"),
            not_platform("t.relowner")
        ))
        .await?
    {
        let index = oid(&r, "oid");
        if constraint_indexes.contains_key(&index) || r.flag("inherited") {
            continue;
        }
        let Some((table, _)) = relations.get(&oid(&r, "rel")) else { continue };
        let id = format!("index {}", r.text("name"));
        ids.class.insert(index, id.clone());
        s.add(Object {
            id,
            kind: Some(Kind::Index),
            owner: Some(table.clone()),
            def: r.text("def").to_string(),
            create: vec![r.text("def").to_string()],
            drop: vec![format!("drop index {}", r.text("name"))],
            ..Object::default()
        });
    }

    for r in db
        .rows(&format!(
            "select p.oid, p.prokind as kind, {} as acl, format('%s', p.oid::regprocedure) as ident, format('%I.%I', n.nspname, p.proname) as qname, pg_get_function_result(p.oid) as result, pg_get_function_arguments(p.oid) as args,
              case when p.prokind <> 'a' then pg_get_functiondef(p.oid) end as def,
              case when p.prokind = 'a' then (select concat_ws(', ', 'sfunc = ' || format('%s', a.aggtransfn::regproc), 'stype = ' || format_type(a.aggtranstype, null),
                  case when a.aggfinalfn <> 0 then 'finalfunc = ' || format('%s', a.aggfinalfn::regproc) end,
                  case when a.aggcombinefn <> 0 then 'combinefunc = ' || format('%s', a.aggcombinefn::regproc) end,
                  case when a.agginitval is not null then 'initcond = ' || quote_literal(a.agginitval) end,
                  case when a.aggsortop <> 0 then 'sortop = ' || format('%s', a.aggsortop::regoperator) end)
                from pg_aggregate a where a.aggfnoid = p.oid) end as agg
            from pg_proc p join pg_namespace n on n.oid = p.pronamespace
            where p.prokind in ('f', 'p', 'w', 'a') and {ns} and {} and {}",
            acl("p.proacl", 'f', "p.proowner"),
            not_extension("pg_proc", "p"),
            not_platform("p.proowner")
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
        let (def, create) = if word == "aggregate" {
            let def = format!("aggregate ({})", r.text("agg"));
            (def, format!("create aggregate {}({}) ({})", r.text("qname"), r.text("args"), r.text("agg")))
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
            drop: vec![format!("drop {word} {ident}")],
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
        let Some((table, _)) = relations.get(&oid(&r, "reloid")) else { continue };
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
            owner: Some(table.clone()),
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
            "select r.oid, r.ev_class, format('%I', r.rulename) as name, format('%I.%I', n.nspname, c.relname) as rel, pg_get_ruledef(r.oid, false) as def, r.rulename = '_RETURN' as view
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
        s.add(Object {
            id,
            kind: Some(Kind::Rule),
            owner: Some(owner.clone()),
            def: r.text("def").to_string(),
            create: vec![r.text("def").trim_end_matches(';').to_string()],
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
        let drop_args = format!("({}, {})", r.get("lt").unwrap_or("none"), r.get("rt").unwrap_or("none"));
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
            from pg_cast c where c.oid >= 16384 and {}",
            not_extension("pg_cast", "c")
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

    for r in db
        .rows(&format!(
            "select s.oid, format('%I', s.srvname) as name, format('%I', w.fdwname) as fdw, s.srvtype as type, s.srvversion as version,
              (select string_agg(format('%I %L', split_part(o, '=', 1), substr(o, strpos(o, '=') + 1)), ', ') from unnest(s.srvoptions) o) as options
            from pg_foreign_server s join pg_foreign_data_wrapper w on w.oid = s.srvfdw where {}",
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
        s.add(Object { id, kind: Some(Kind::Server), create: vec![format!("create server {name} {def}")], drop: vec![format!("drop server {name}")], def, ..Object::default() });
    }

    // Publications: what each publishes, from the tables and schemas it names.
    // Postgres 15 publishes some of a table's columns, its rows that match a filter, and schemas.
    let (columns, schemas) = if version >= 150000 {
        (
            "coalesce(' (' || (select string_agg(format('%I', a.attname), ', ' order by a.attnum) from pg_attribute a where a.attrelid = c.oid and a.attnum = any(pr.prattrs::int2[])) || ')', '') || coalesce(' where (' || pg_get_expr(pr.prqual, c.oid) || ')', '')",
            "(select json_agg(format('%I', n.nspname) order by n.nspname) from pg_publication_namespace pn join pg_namespace n on n.oid = pn.pnnspid where pn.pnpubid = p.oid)",
        )
    } else {
        ("''", "null::json")
    };
    for r in db
        .rows(&format!(
            "select format('%I', p.pubname) as name, p.puballtables as all_tables, p.pubviaroot as via_root,
              concat_ws(', ', case when p.pubinsert then 'insert' end, case when p.pubupdate then 'update' end, case when p.pubdelete then 'delete' end, case when p.pubtruncate then 'truncate' end) as publish,
              format('%s', (select json_agg(json_build_array(format('%I.%I', n.nspname, c.relname), format('%I.%I', n.nspname, c.relname) || {columns}) order by 1) from pg_publication_rel pr join pg_class c on c.oid = pr.prrelid join pg_namespace n on n.oid = c.relnamespace where pr.prpubid = p.oid)) as tables,
              format('%s', {schemas}) as schemas
            from pg_publication p where {}",
            not_platform("p.pubowner")
        ))
        .await?
    {
        let name = r.text("name");
        let tables: Vec<(String, String)> = serde_json::from_str(r.text("tables")).unwrap_or_default();
        let in_schemas: Vec<String> = serde_json::from_str(r.text("schemas")).unwrap_or_default();
        let mut what = Vec::new();
        if r.flag("all_tables") {
            what.push("all tables".to_string());
        }
        if !tables.is_empty() {
            what.push(format!("table {}", tables.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(", ")));
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
        let deps = tables.iter().map(|(t, _)| format!("table {t}")).chain(in_schemas.iter().map(|n| format!("schema {n}"))).collect();
        let mut objects: Vec<String> = tables.iter().map(|(_, t)| format!("table {t}")).collect();
        objects.extend(in_schemas.iter().map(|n| format!("tables in schema {n}")));
        let extra = vec![format!("all {}", r.flag("all_tables")), format!("objects {}", objects.join(", ")), format!("with {}", with.join(", ")), format!("publish {}", r.text("publish"))];
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

    // Comments on what is not a relation or a column, which are read with them.
    for r in db
        .rows(
            "select format('%s', d.classoid::regclass) as cls, d.objoid, d.description,
              (select t.typtype from pg_type t where d.classoid = 'pg_type'::regclass and t.oid = d.objoid) as typtype,
              (select p.prokind from pg_proc p where d.classoid = 'pg_proc'::regclass and p.oid = d.objoid) as prokind
            from pg_description d where d.objsubid = 0 and d.objoid >= 16384
              and d.classoid in ('pg_namespace'::regclass, 'pg_type'::regclass, 'pg_proc'::regclass, 'pg_constraint'::regclass, 'pg_trigger'::regclass, 'pg_policy'::regclass, 'pg_class'::regclass)",
        )
        .await?
    {
        let Some(target) = ids.of(r.text("cls"), oid(&r, "objoid"), 0) else { continue };
        let (kind, name) = target.split_once(' ').unwrap_or_default();
        let on_table = |rest: &str| rest.rsplit_once('.').map(|(table, name)| format!("{name} on {table}"));
        let on = match (r.text("cls"), kind) {
            ("pg_namespace", "schema") => Some(format!("schema {name}")),
            ("pg_type", "type") if r.text("typtype") == "d" => Some(format!("domain {name}")),
            ("pg_type", "type") => Some(format!("type {name}")),
            ("pg_proc", "function") => Some(format!("{} {name}", match r.text("prokind") { "p" => "procedure", "a" => "aggregate", _ => "function" })),
            ("pg_constraint", "constraint") => on_table(name).map(|n| format!("constraint {n}")),
            ("pg_trigger", "trigger") => on_table(name).map(|n| format!("trigger {n}")),
            ("pg_policy", "policy") => on_table(name).map(|n| format!("policy {n}")),
            ("pg_class", "index") => Some(format!("index {name}")),
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

    // Array types stand for their element type.
    for r in db.rows("select t.oid, t.typelem from pg_type t where t.oid >= 16384 and t.typcategory = 'A' and t.typelem <> 0").await? {
        if let Some(id) = ids.types.get(&oid(&r, "typelem")).cloned() {
            ids.types.insert(oid(&r, "oid"), id);
        }
    }

    // What each object depends on, as the server records it.
    for r in db
        .rows(
            "select format('%s', d.classid::regclass) as cls, d.objid, d.objsubid, format('%s', d.refclassid::regclass) as rcls, d.refobjid, d.refobjsubid
            from pg_depend d where d.deptype in ('n', 'a') and d.objid >= 16384 and d.refobjid >= 16384",
        )
        .await?
    {
        let from = ids.of(r.text("cls"), oid(&r, "objid"), r.text("objsubid").parse().unwrap_or(0));
        let to = ids.of(r.text("rcls"), oid(&r, "refobjid"), r.text("refobjsubid").parse().unwrap_or(0));
        if let (Some(from), Some(to)) = (from, to) {
            if from != to {
                if let Some(o) = s.objects.get_mut(&from) {
                    o.deps.insert(to);
                }
            }
        }
    }
    // A schema's objects depend on it, as the records say; a column depends on nothing of its own
    // table, which creating the table makes.
    let tables: BTreeSet<String> = s.objects.values().filter(|o| o.kind == Some(Kind::Table)).map(|o| o.id.clone()).collect();
    for o in s.objects.values_mut() {
        if let Some(owner) = o.owner.clone() {
            if tables.contains(&owner) && o.kind == Some(Kind::Column) {
                o.deps.retain(|d| !d.starts_with(&format!("column {}.", owner.trim_start_matches("table "))) && *d != owner);
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
        } else if o.kind != Some(Kind::Column) {
            continue;
        }
        let lines: Vec<String> = on.iter().filter_map(|a| s.objects.get(a)).map(|a| around_line(a)).collect();
        if let Some(o) = s.objects.get_mut(&id) {
            o.extra.extend(lines);
        }
    }
    Ok(s)
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
    }
    out
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
        let schema = rel.rsplit_once('.').map_or("", |(s, _)| s);
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
fn acl(acl: &str, kind: char, owner: &str) -> String {
    let explode = |of: &str| {
        format!(
            "(select json_agg(json_build_object('grantee', case when x.grantee = 0 then 'public' else quote_ident(pg_get_userbyid(x.grantee)) end, 'privilege', x.privilege_type, 'grantable', x.is_grantable))
               from aclexplode({of}) x where x.grantee <> {owner})"
        )
    };
    format!("format('%s', json_build_array({}, {}))", explode(&format!("coalesce({acl}, acldefault('{kind}', {owner}))")), explode(&format!("acldefault('{kind}', {owner})")))
}

/// One object per privilege the object `name` (`on` says what it is: `table`, `function`, …)
/// grants beyond those it has by default, and one per default privilege it has revoked: a new
/// object has its defaults, which the plan revokes where the target has. `privileges` is `acl`'s.
fn grants(s: &mut Schema, privileges: &str, name: &str, id: &str, on: &str) {
    let lists: Vec<Option<Vec<serde_json::Value>>> = serde_json::from_str(privileges).unwrap_or_default();
    let (have, default) = (lists.first().cloned().flatten().unwrap_or_default(), lists.get(1).cloned().flatten().unwrap_or_default());
    let key = |g: &serde_json::Value| (g["grantee"].as_str().unwrap_or_default().to_string(), g["privilege"].as_str().unwrap_or_default().to_string(), g["grantable"].as_bool().unwrap_or(false));
    let (have, default): (BTreeSet<_>, BTreeSet<_>) = (have.iter().map(key).collect(), default.iter().map(key).collect());
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

/// What an object a change is made around is carried as: `around <id>\t<def>\t<drop>\t<create>`.
const AROUND: &str = "around ";

fn around_line(o: &Object) -> String {
    format!("{AROUND}{}\t{}\t{}\t{}", o.id, o.def.replace(['\t', '\n'], " "), o.drop.join("; "), o.create.join("; "))
}

/// The objects made around a change of `from` to `to` that both have alike, as `(drops, creates)`:
/// what changes of them otherwise is the plan's own.
fn around(from: &Object, to: &Object) -> (Vec<String>, Vec<String>) {
    let lines = |o: &Object| -> BTreeMap<String, (String, String, String)> {
        o.extra
            .iter()
            .filter_map(|l| l.strip_prefix(AROUND))
            .filter_map(|l| {
                let mut parts = l.splitn(4, '\t');
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
    match (c.text("identity"), c.text("generated")) {
        ("a", _) => parts.push("generated always as identity".into()),
        ("d", _) => parts.push("generated by default as identity".into()),
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
#[derive(Debug, Default, PartialEq, Eq)]
struct Column {
    ty: String,
    collation: Option<String>,
    default: Option<String>,
    identity: Option<String>,
    generated: Option<String>,
    not_null: bool,
    /// Its NOT NULL kept from the table's children.
    no_inherit: bool,
}

fn parse_column(def: &str) -> Column {
    let mut c = Column::default();
    let mut rest = bare(def).to_string();
    if let Some(r) = rest.strip_suffix(" no inherit") {
        c.no_inherit = true;
        rest = r.to_string();
    }
    if let Some(r) = rest.strip_suffix(" not null") {
        c.not_null = true;
        rest = r.to_string();
    }
    for marker in [" generated always as identity", " generated by default as identity"] {
        if let Some(r) = rest.strip_suffix(marker) {
            c.identity = Some(marker.trim().to_string());
            rest = r.to_string();
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
fn swap_enum(name: &str, create: &str, columns: &[String]) -> Vec<String> {
    let (schema, bare) = name.rsplit_once('.').unwrap_or(("", name));
    let aside = if bare.ends_with('"') { format!("{}__old\"", bare.trim_end_matches('"')) } else { format!("{bare}__old") };
    let old = if schema.is_empty() { aside.clone() } else { format!("{schema}.{aside}") };
    let mut out = vec![format!("alter type {name} rename to {aside}"), create.to_string()];
    for c in columns.iter().filter(|c| !c.starts_with(AROUND)) {
        let parts: Vec<&str> = c.splitn(4, '|').collect();
        let [table, column, ty, default] = parts[..] else { continue };
        let text = if ty.ends_with("[]") { "text[]" } else { "text" };
        if !default.is_empty() {
            out.push(format!("alter table {table} alter column {column} drop default"));
        }
        out.push(format!("alter table {table} alter column {column} type {ty} using {column}::{text}::{ty}"));
        if !default.is_empty() {
            let restored = default.replace(&format!("::{old}"), &format!("::{name}")).replace(&format!("::{aside}"), &format!("::{name}"));
            out.push(format!("alter table {table} alter column {column} set default {restored}"));
        }
    }
    out.push(format!("drop type {old}"));
    out
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
}

impl PostgresAlterer {
    /// The casts the server behind `db` makes in assignment.
    pub async fn read(db: &mut Db, version: i64) -> Result<PostgresAlterer, Error> {
        let rows = db.rows("select format_type(castsource, null) as s, format_type(casttarget, null) as t from pg_cast where castcontext in ('a', 'i')").await?;
        let assignment = rows.iter().map(|r| (r.text("s").to_string(), r.text("t").to_string())).collect();
        let strings = db.rows("select format_type(oid, null) as t from pg_type where typcategory = 'S'").await?.iter().map(|r| r.text("t").to_string()).collect();
        Ok(PostgresAlterer { version, assignment, strings })
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
        let why = super::narrowing(&type_class(&a), &type_class(&b))?;
        Some(format!("changes from {a} to {b}, which {why}"))
    }

    fn rename(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        let table = to.owner.as_deref()?.strip_prefix("table ")?;
        let (old, new) = (super::last_written(&from.id), super::last_written(&to.id));
        Some(vec![format!("alter table {table} rename column {old} to {new}")])
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
            Kind::Column if to.owner.as_deref().is_some_and(|o| o.starts_with("type ")) => None,
            // A partition's column changes with its parent's.
            Kind::Column if to.extra.iter().any(|l| l == "partition") => Some(vec![]),
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
                if a.default.is_some() && (b.default.is_none() || type_changes) {
                    out.push(format!("{at} drop default"));
                }
                let (unmake, remake) = if type_changes { around(from, to) } else { (vec![], vec![]) };
                if type_changes {
                    out.extend(unmake);
                    let collate = b.collation.as_deref().map(|c| format!(" collate {c}")).unwrap_or_default();
                    if self.assigns(&a.ty, &b.ty) {
                        out.push(format!("{at} type {}{collate}", b.ty));
                    } else {
                        out.push(format!("{at} type {}{collate} using {name}::{}", b.ty, b.ty));
                    }
                    out.extend(remake);
                }
                if b.default.is_some() && (a.default != b.default || reset) {
                    if let Some(d) = declared_default {
                        b.default = Some(d);
                    }
                    out.push(format!("{at} set default {}", b.default.as_deref().unwrap_or_default()));
                }
                match (&a.identity, &b.identity) {
                    (None, Some(kind)) => {
                        out.push(format!("{at} add {kind}"));
                        // A new identity starts past the rows the table holds, or its first insert collides.
                        out.push(format!("select setval(pg_get_serial_sequence({}, {}), coalesce(max({name}), 0) + 1, false) from {table}", literal(&table), literal(&name.replace('"', "")), ));
                    }
                    (Some(x), Some(y)) if x != y => out.push(format!("{at} set {}", y.trim_end_matches(" as identity"))),
                    _ => {}
                }
                // A NOT NULL kept from the table's children is added as a constraint of its own.
                if a.not_null && (!b.not_null || a.no_inherit != b.no_inherit) {
                    out.push(format!("{at} drop not null"));
                }
                if b.not_null && (!a.not_null || a.no_inherit != b.no_inherit) {
                    out.push(if b.no_inherit { format!("alter table {table} add not null {name} no inherit") } else { format!("{at} set not null") });
                }
                Some(out)
            }
            Kind::Type if to.def.starts_with("enum (") && from.def.starts_with("enum (") => {
                // New labels only, the old ones kept in their order: each added beside its neighbour.
                let labels = |def: &str| -> Vec<String> { def.trim_start_matches("enum (").trim_end_matches(')').split(", ").map(String::from).collect() };
                let (a, b) = (labels(&from.def), labels(&to.def));
                let kept: Vec<&String> = b.iter().filter(|l| a.contains(l)).collect();
                let name = to.id.strip_prefix("type ")?;
                if kept.len() != a.len() || kept.iter().zip(&a).any(|(x, y)| *x != y) {
                    let (unmake, remake) = around(from, to);
                    let mut out = unmake;
                    let mut swap = swap_enum(name, &to.create[0], &from.extra);
                    let last = swap.pop();
                    out.extend(swap);
                    out.extend(remake);
                    out.extend(last);
                    return Some(out);
                }
                let mut out = Vec::new();
                for (k, label) in b.iter().enumerate() {
                    if a.contains(label) {
                        continue;
                    }
                    match b[..k].iter().rev().find(|l| a.contains(l) || out.iter().any(|s: &String| s.contains(l.as_str()))) {
                        Some(before) => out.push(format!("alter type {name} add value {label} after {before}")),
                        None => match b.get(k + 1) {
                            Some(next) => out.push(format!("alter type {name} add value {label} before {next}")),
                            None => out.push(format!("alter type {name} add value {label}")),
                        },
                    }
                }
                Some(out)
            }
            Kind::Type if to.def.starts_with("domain ") && from.def.starts_with("domain ") => {
                // A domain's default, NOT NULL and checks change in place; its base type does not.
                let part = |o: &Object, what: &str| o.extra.iter().find_map(|l| l.strip_prefix(what)).map(String::from);
                if part(from, "base ") != part(to, "base ") {
                    return None;
                }
                let name = to.id.strip_prefix("type ")?;
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
                let checks = |o: &Object| -> BTreeMap<String, String> {
                    o.extra.iter().filter_map(|l| l.strip_prefix("check ")).filter_map(|l| l.split_once(' ')).map(|(n, d)| (n.to_string(), d.to_string())).collect()
                };
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
                        out.push(format!("alter table {parent} attach partition {name} {bound}"));
                    }
                }
                if part(from, "inherits ") != part(to, "inherits ") {
                    if let Some(parent) = part(from, "inherits ") {
                        out.push(format!("alter table {name} no inherit {parent}"));
                    }
                    if let Some(parent) = part(to, "inherits ") {
                        out.push(format!("alter table {name} inherit {parent}"));
                    }
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
                    (a, b) if b.is_empty() => out.push(format!("alter publication {name} drop {a}")),
                    (_, b) => out.push(format!("alter publication {name} set {b}")),
                }
                if part(from, "with ") != part(to, "with ") {
                    let root = to.extra.iter().any(|l| l.starts_with("with ") && l.contains("publish_via_partition_root = true"));
                    out.push(format!("alter publication {name} set (publish = {}, publish_via_partition_root = {root})", literal(&part(to, "publish "))));
                }
                Some(out)
            }
            Kind::Rls => {
                let name = to.id.strip_prefix("rls ")?;
                Some(vec![if to.def == "force" { format!("alter table {name} force row level security") } else { format!("alter table {name} no force row level security") }])
            }
            Kind::Comment => Some(to.create.clone()),
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
    }
}
