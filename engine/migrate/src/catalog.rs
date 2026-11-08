//! Each database's catalog as canonical text: one line per object or property, sorted, so that two
//! databases built differently compare equal exactly when their schemas are the same. The server's
//! own renderings (Postgres's `pg_get_*def`, MySQL's information_schema, SQLite's stored `create`
//! text, rendered again token by token) are kept as they are, with internal references resolved to
//! names. Columns are compared as a set, by name; what changes as rows are written (a sequence's
//! current value, MySQL's next AUTO_INCREMENT, `sqlite_sequence`) is left out, and so is Reylo's own
//! history table.

use crate::db::{Db, Row};
use crate::sql::{tokenize, Token, TokenKind};
use crate::{Dialect, Error};
use std::collections::{BTreeMap, BTreeSet};

pub const HISTORY_TABLE: &str = "__reylo_migrations";

/// The database's schema as sorted lines.
pub async fn catalog(db: &mut Db) -> Result<Vec<String>, Error> {
    let lines = match db.dialect() {
        Dialect::Postgres => postgres(db).await?,
        Dialect::Mysql => mysql(db).await?,
        Dialect::Sqlite => sqlite(db).await?,
    };
    Ok(lines.into_iter().collect::<BTreeSet<_>>().into_iter().collect())
}

/// The child tables the lines hold (a partition, or a table that inherits another, as TimescaleDB's
/// chunks do), each with its parent, where the parent is a table the lines hold too.
fn children(lines: &[String]) -> Vec<(String, String)> {
    let tables: BTreeSet<&str> = lines.iter().filter_map(|l| l.strip_prefix("table ")).map(name_at).collect();
    lines
        .iter()
        .filter_map(|l| l.strip_prefix("table "))
        .filter_map(|l| {
            let name = name_at(l);
            let tail = &l[name.len()..];
            // MySQL's line goes on to its engine and options, whose words are no clause of it.
            if tail.starts_with(" engine=") {
                return None;
            }
            let parent = tail.split_once(" partition of ").or_else(|| tail.split_once(" inherits ")).map(|(_, rest)| name_at(rest))?;
            tables.contains(parent).then(|| (name.to_string(), parent.to_string()))
        })
        .collect()
}

/// The qualified name `text` opens with, as `format('%I.%I')` writes it: quoted parts keep their spaces.
fn name_at(text: &str) -> &str {
    let mut quoted = false;
    for (k, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ' ' if !quoted => return &text[..k],
            _ => {}
        }
    }
    text
}

/// What follows the first `word` in `text` that is outside quotes: the table an index, trigger or
/// rule is on, in the server's own text of it.
fn after_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let mut quoted = false;
    for (k, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ' ' if !quoted && text[k..].starts_with(word) => return Some(&text[k + word.len()..]),
            _ => {}
        }
    }
    None
}

/// Whether the line is one of the table `name`'s: the table, its columns, and what is on it. Read
/// from where each kind of line names its table, so that a view or function that mentions the table,
/// a key that references it, or a schema that shares its name is none of its own.
fn of_table(line: &str, name: &str) -> bool {
    let Some((kind, rest)) = line.split_once(' ') else { return false };
    let first = name_at(rest);
    let column = first.strip_prefix(name).is_some_and(|tail| tail.starts_with('.'));
    match kind {
        "table" | "constraint" | "policy" | "replica" | "owner" => first == name,
        "column" => column,
        "grant" | "comment" => first == name || column,
        "index" | "trigger" => after_word(rest, " ON ").is_some_and(|on| name_at(on.strip_prefix("ONLY ").unwrap_or(on)) == name),
        "rule" => after_word(rest, " TO ").is_some_and(|on| name_at(on) == name),
        _ => false,
    }
}

/// `live` without the partitions (and inheriting children) of a table `reference` has too that
/// `reference` lacks: children made at run time (pg_partman's, TimescaleDB's chunks) are data, not
/// schema. A child `reference` has is compared as any table, missing or changed.
pub fn without_extra_children(live: &[String], reference: &[String]) -> Vec<String> {
    // A default privilege the reference doesn't have is the environment's (a platform sets its own
    // as it makes a database), as Reylo copies it into every shadow: only one a migration made and
    // the live database lacks is a difference.
    let live: Vec<String> = live.iter().filter(|l| !l.starts_with(DEFAULT_PRIVILEGES) || reference.contains(l)).cloned().collect();
    let live = &live[..];
    let theirs: BTreeSet<String> = children(reference).into_iter().map(|(name, _)| name).collect();
    let parents: BTreeSet<&str> = reference.iter().filter_map(|l| l.strip_prefix("table ")).map(name_at).collect();
    let extra: Vec<String> = children(live).into_iter().filter(|(name, parent)| !theirs.contains(name) && parents.contains(parent.as_str()) && !parents.contains(name.as_str())).map(|(name, _)| name).collect();
    if extra.is_empty() {
        return live.to_vec();
    }
    live.iter().filter(|l| !extra.iter().any(|name| of_table(l, name))).cloned().collect()
}

/// `lines` without any partition or inheriting child, which the fingerprint leaves out: one may have
/// been made at run time, as data, and only a replay can tell. Default privileges are left out of
/// it too: one may be the environment's, which only a replay against the migrations can tell.
pub fn without_children(lines: &[String]) -> Vec<String> {
    let kids: Vec<String> = children(lines).into_iter().map(|(name, _)| name).collect();
    lines.iter().filter(|l| !l.starts_with(DEFAULT_PRIVILEGES) && !kids.iter().any(|name| of_table(l, name))).cloned().collect()
}

/// How a default privilege's line begins: the statement that makes it, without its `alter`.
pub(crate) const DEFAULT_PRIVILEGES: &str = "default privileges ";

/// `lines` without comments. A migration writes a comment, and two replays compare theirs, but a
/// live database's is never compared: one added in a dashboard changes nothing the app runs on, so
/// it is no drift and stops no deploy.
pub fn without_comments(lines: &[String]) -> Vec<String> {
    lines.iter().filter(|l| !l.starts_with("comment ")).cloned().collect()
}

/// Whether the live database has what `reference` lists: comments, and children made at run time, aside.
pub fn same_schema(live: &[String], reference: &[String]) -> bool {
    without_comments(&without_extra_children(live, reference)) == without_comments(reference)
}

/// The lines only in `a`, and those only in `b`.
pub fn difference(a: &[String], b: &[String]) -> (Vec<String>, Vec<String>) {
    let sa: BTreeSet<&String> = a.iter().collect();
    let sb: BTreeSet<&String> = b.iter().collect();
    (a.iter().filter(|l| !sb.contains(l)).cloned().collect(), b.iter().filter(|l| !sa.contains(l)).cloned().collect())
}

pub fn same(a: &[String], b: &[String]) -> bool {
    a == b
}

fn json(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// Postgres. Every query runs with an empty search_path, so every rendering names its schema.

pub(crate) const PG_USER_NS: &str = r"n.nspname not in ('pg_catalog', 'information_schema') and n.nspname not like 'pg\_toast%' and n.nspname not like 'pg\_temp%'";

/// The roles a hosting platform creates its own objects as: what they own is the platform's, never
/// compared (the comparison would report every platform upgrade as drift). Supabase's are the 21 its
/// own diff engine leaves out (pg-delta's `SUPABASE_SYSTEM_ROLES`); Neon's system role is
/// `cloud_admin`. A role that does not exist on the server owns nothing, so the list needs no
/// detection. Policies are never the platform's, and a trigger is when its function is.
pub const PLATFORM_ROLES: &[&str] = &[
    "anon",
    "authenticated",
    "authenticator",
    "cli_login_postgres",
    "dashboard_user",
    "pgbouncer",
    "pgsodium_keyholder",
    "pgsodium_keyiduser",
    "pgsodium_keymaker",
    "pgtle_admin",
    "service_role",
    "supabase_admin",
    "supabase_auth_admin",
    "supabase_etl_admin",
    "supabase_functions_admin",
    "supabase_privileged_role",
    "supabase_read_only_user",
    "supabase_realtime_admin",
    "supabase_replication_admin",
    "supabase_storage_admin",
    "supabase_superuser",
    "cloud_admin",
];

pub(crate) fn not_platform(owner: &str) -> String {
    let roles = PLATFORM_ROLES.iter().map(|r| format!("'{r}'")).collect::<Vec<_>>().join(", ");
    format!("pg_get_userbyid({owner}) <> all (array[{roles}])")
}

pub(crate) fn not_extension(catalog: &str, alias: &str) -> String {
    format!("not exists (select 1 from pg_depend e where e.classid = '{catalog}'::regclass and e.objid = {alias}.oid and e.deptype = 'e')")
}

/// An object's privileges as `grantee:privilege[*]`, sorted and comma-separated, its owner's own left
/// out and its default (`acldefault`) read where it has none of its own, so that a revoke from
/// PUBLIC shows: `kind` is `acldefault`'s letter for the object.
fn acl(column: &str, owner: &str, kind: char) -> String {
    format!(
        "(select string_agg(distinct format('%s:%s%s', case when x.grantee = 0 then 'PUBLIC' else pg_get_userbyid(x.grantee) end, x.privilege_type, case when x.is_grantable then '*' else '' end), ',')
           from aclexplode(coalesce({column}, acldefault('{kind}', {owner}))) x where x.grantee <> {owner})"
    )
}

async fn postgres(db: &mut Db) -> Result<Vec<String>, Error> {
    db.exec("set search_path = ''").await?;
    let result = postgres_lines(db).await;
    db.exec("reset search_path").await?;
    result
}

async fn postgres_lines(db: &mut Db) -> Result<Vec<String>, Error> {
    let mut lines = Vec::new();
    let ns = PG_USER_NS;
    let version: i64 = db.rows("select current_setting('server_version_num') as v").await?.first().and_then(|r| r.first()?.parse().ok()).unwrap_or(0);
    let history = HISTORY_TABLE;

    for r in db
        .rows(&format!(
            "select n.nspname as name, {} as acl, obj_description(n.oid, 'pg_namespace') as comment from pg_namespace n where {ns} and {} and {}",
            acl("n.nspacl", "n.nspowner", 'n'),
            not_extension("pg_namespace", "n"),
            not_platform("n.nspowner")
        ))
        .await?
    {
        let name = r.text("name");
        lines.push(format!("schema {name}"));
        if let Some(acl) = r.get("acl") {
            lines.push(format!("grant schema {name} {acl}"));
        }
        if let Some(comment) = r.get("comment") {
            lines.push(format!("comment schema {name} {}", json(comment)));
        }
    }
    // An extension's version is the server's: an unversioned `create extension` replays at today's
    // default, so a database that installed an older one would drift after every server upgrade.
    for r in db
        .rows(&format!(
            "select x.extname as name, n.nspname as schema from pg_extension x join pg_namespace n on n.oid = x.extnamespace where x.extname <> 'plpgsql' and {}",
            not_platform("x.extowner")
        ))
        .await?
    {
        lines.push(format!("extension {} schema={}", r.text("name"), r.text("schema")));
    }

    // Relations: tables, partitioned tables, views, materialized views, foreign tables, composite types.
    for r in db
        .rows(&format!(
            "select format('%I.%I', n.nspname, c.relname) as name, c.relkind as kind, c.relpersistence as persistence,
              c.relrowsecurity as rls, c.relforcerowsecurity as force,
              (select string_agg(o, ', ' order by o) from unnest(c.reloptions) o) as options,
              (select nullif(a.amname, 'heap') from pg_am a where a.oid = c.relam) as am,
              pg_get_expr(c.relpartbound, c.oid) as bound,
              (select format('%I.%I', pn.nspname, pc.relname) from pg_inherits i join pg_class pc on pc.oid = i.inhparent join pg_namespace pn on pn.oid = pc.relnamespace where i.inhrelid = c.oid limit 1) as parent,
              case when c.relkind = 'p' then pg_get_partkeydef(c.oid) end as partkey,
              (select string_agg(distinct format('%s:%s%s', case when x.grantee = 0 then 'PUBLIC' else pg_get_userbyid(x.grantee) end, x.privilege_type, case when x.is_grantable then '*' else '' end), ',')
                 from aclexplode(c.relacl) x where x.grantee <> c.relowner) as acl,
              obj_description(c.oid, 'pg_class') as comment,
              case when c.relkind in ('r', 'p') then case c.relreplident when 'f' then 'full' when 'n' then 'nothing'
                when 'i' then (select format('using index %I', ic.relname) from pg_index i join pg_class ic on ic.oid = i.indexrelid where i.indrelid = c.oid and i.indisreplident) end end as replica,
              (select format('server %I', fs.srvname) || coalesce(' options (' || (select string_agg(o, ', ' order by o) from unnest(ft.ftoptions) o) || ')', '')
                 from pg_foreign_table ft join pg_foreign_server fs on fs.oid = ft.ftserver where ft.ftrelid = c.oid) as foreign_server
            from pg_class c join pg_namespace n on n.oid = c.relnamespace
            where c.relkind in ('r', 'p', 'v', 'm', 'f', 'c') and {ns} and {} and {} and c.relname <> '{history}'
              and not exists (select 1 from pg_depend e where e.classid = 'pg_type'::regclass and e.objid = c.reltype and e.deptype = 'e')",
            not_extension("pg_class", "c"),
            not_platform("c.relowner")
        ))
        .await?
    {
        let kind = match r.text("kind") {
            "r" | "p" => "table",
            "v" => "view",
            "m" => "materialized view",
            "f" => "foreign table",
            _ => "type",
        };
        let name = r.text("name");
        let mut parts = vec![format!("{kind} {name}")];
        if r.text("kind") == "p" {
            parts.push(format!("partition by {}", r.text("partkey")));
        }
        match (r.get("bound"), r.get("parent")) {
            (Some(bound), _) => parts.push(format!("partition of {} {bound}", r.text("parent"))),
            (None, Some(parent)) if kind == "table" => parts.push(format!("inherits {parent}")),
            _ => {}
        }
        if r.text("persistence") == "u" {
            parts.push("unlogged".into());
        }
        // A table's storage, where it is not Postgres's own heap: a column store's, such as Citus
        // columnar's or pg_duckdb's.
        if let Some(am) = r.get("am") {
            parts.push(format!("using {am}"));
        }
        if r.flag("rls") {
            parts.push("row level security".into());
        }
        if r.flag("force") {
            parts.push("force row level security".into());
        }
        if let Some(options) = r.get("options") {
            parts.push(format!("with ({options})"));
        }
        // A foreign table's rows are its server's, read with its options.
        if let Some(server) = r.get("foreign_server") {
            parts.push(server.to_string());
        }
        lines.push(parts.join(" "));
        // What logical replication sends of a row an update or delete changes: Supabase Realtime
        // needs `full` to send the old row.
        if let Some(replica) = r.get("replica") {
            lines.push(format!("replica {name} identity {replica}"));
        }
        if let Some(acl) = r.get("acl") {
            lines.push(format!("grant {name} {acl}"));
        }
        if let Some(comment) = r.get("comment") {
            lines.push(format!("comment {name} {}", json(comment)));
        }
    }

    for r in db
        .rows(&format!(
            "select format('%I.%I', n.nspname, c.relname) as rel, a.attname as name, format_type(a.atttypid, a.atttypmod) as type, a.attnotnull as notnull,
              pg_get_expr(d.adbin, d.adrelid) as def, a.attidentity as identity, a.attgenerated as generated,
              case when a.attcollation <> t.typcollation then (select format('%I.%I', cn.nspname, co.collname) from pg_collation co join pg_namespace cn on cn.oid = co.collnamespace where co.oid = a.attcollation) end as collation,
              col_description(c.oid, a.attnum) as comment,
              exists (select 1 from pg_constraint nn where nn.conrelid = c.oid and nn.contype = 'n' and nn.connoinherit and nn.conkey = array[a.attnum]) as noinherit,
              (select string_agg(distinct format('%s:%s%s', case when x.grantee = 0 then 'PUBLIC' else pg_get_userbyid(x.grantee) end, x.privilege_type, case when x.is_grantable then '*' else '' end), ',')
                 from aclexplode(a.attacl) x where x.grantee <> c.relowner) as acl
            from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace join pg_type t on t.oid = a.atttypid
            left join pg_attrdef d on d.adrelid = a.attrelid and d.adnum = a.attnum
            where a.attnum > 0 and not a.attisdropped and c.relkind in ('r', 'p', 'v', 'm', 'f', 'c') and {ns} and {} and {} and c.relname <> '{history}'
              and not exists (select 1 from pg_depend e where e.classid = 'pg_type'::regclass and e.objid = c.reltype and e.deptype = 'e')",
            not_extension("pg_class", "c"),
            not_platform("c.relowner")
        ))
        .await?
    {
        let column = format!("{}.{}", r.text("rel"), r.text("name"));
        let mut parts = vec![format!("column {column} {}", r.text("type"))];
        if r.flag("notnull") {
            parts.push(if r.flag("noinherit") { "not null no inherit".into() } else { "not null".into() });
        }
        match (r.text("identity"), r.text("generated")) {
            ("a", _) => parts.push("generated always as identity".into()),
            ("d", _) => parts.push("generated by default as identity".into()),
            (_, g) if !g.is_empty() => parts.push(format!("generated always as ({}) {}", r.text("def"), if g == "s" { "stored" } else { "virtual" })),
            _ => {
                if let Some(def) = r.get("def") {
                    parts.push(format!("default {def}"));
                }
            }
        }
        if let Some(collation) = r.get("collation") {
            parts.push(format!("collate {collation}"));
        }
        lines.push(parts.join(" "));
        // A privilege on the column alone: `grant select (email) on users to …`.
        if let Some(acl) = r.get("acl") {
            lines.push(format!("grant {column} {acl}"));
        }
        if let Some(comment) = r.get("comment") {
            lines.push(format!("comment {column} {}", json(comment)));
        }
    }

    // Constraints (not the not-null ones Postgres 18 records, which the columns carry).
    for r in db
        .rows(&format!(
            "select case when c.oid is not null then format('%I.%I', n.nspname, c.relname) else format('%I.%I', tn.nspname, t.typname) end as rel, co.conname as name,
              pg_get_constraintdef(co.oid, false) as def, co.condeferrable as deferrable, co.condeferred as deferred, co.convalidated as validated
            from pg_constraint co
            left join pg_class c on c.oid = co.conrelid left join pg_namespace n on n.oid = c.relnamespace
            left join pg_type t on t.oid = co.contypid left join pg_namespace tn on tn.oid = t.typnamespace
            where co.contype <> 'n' and coalesce(n.nspname, tn.nspname) not in ('pg_catalog', 'information_schema') and coalesce(c.relname, '') <> '{history}'
              and not exists (select 1 from pg_depend e where e.classid = 'pg_class'::regclass and e.objid = co.conrelid and e.deptype = 'e')
              and {} and {}",
            not_platform("coalesce(c.relowner, t.typowner)"),
            not_extension("pg_constraint", "co")
        ))
        .await?
    {
        let deferral = match (r.flag("deferrable"), r.flag("deferred")) {
            (true, true) => " deferrable initially deferred",
            (true, false) => " deferrable",
            _ => "",
        };
        lines.push(format!("constraint {} {} {}{deferral}{}", r.text("rel"), r.text("name"), r.text("def"), if r.flag("validated") { "" } else { " not valid" }));
    }

    for r in db
        .rows(&format!(
            "select pg_get_indexdef(i.indexrelid) as def, i.indisvalid as valid, i.indisready as ready
            from pg_index i join pg_class c on c.oid = i.indexrelid join pg_namespace n on n.oid = c.relnamespace join pg_class t on t.oid = i.indrelid
            where {ns} and {} and {} and t.relname <> '{history}'
              and not exists (select 1 from pg_depend e where e.classid = 'pg_class'::regclass and e.objid = t.oid and e.deptype = 'e')",
            not_extension("pg_class", "c"),
            not_platform("t.relowner")
        ))
        .await?
    {
        lines.push(format!("index {}{}{}", r.text("def"), if r.flag("valid") { "" } else { " (invalid)" }, if r.flag("ready") { "" } else { " (not ready)" }));
    }

    for r in db
        .rows(&format!(
            "select format('%I.%I', n.nspname, c.relname) as name, pg_get_viewdef(c.oid, false) as def
            from pg_class c join pg_namespace n on n.oid = c.relnamespace where c.relkind in ('v', 'm') and {ns} and {} and {}",
            not_extension("pg_class", "c"),
            not_platform("c.relowner")
        ))
        .await?
    {
        lines.push(format!("view {} as {}", r.text("name"), one_line(r.text("def"))));
    }

    for r in db
        .rows(&format!(
            "select format('%I.%I', n.nspname, c.relname) as name, format_type(s.seqtypid, null) as type, s.seqstart as start, s.seqincrement as inc,
              s.seqmin as min, s.seqmax as max, s.seqcache as cache, s.seqcycle as cycle,
              (select format('%I.%I.%I', tn.nspname, t.relname, a.attname) from pg_depend d join pg_class t on t.oid = d.refobjid join pg_namespace tn on tn.oid = t.relnamespace
                 join pg_attribute a on a.attrelid = d.refobjid and a.attnum = d.refobjsubid where d.classid = 'pg_class'::regclass and d.objid = c.oid and d.deptype = 'a') as owned
            from pg_sequence s join pg_class c on c.oid = s.seqrelid join pg_namespace n on n.oid = c.relnamespace
            where {ns} and {} and {}
              and not exists (select 1 from pg_depend d where d.classid = 'pg_class'::regclass and d.objid = c.oid and d.deptype = 'i')",
            not_extension("pg_class", "c"),
            not_platform("c.relowner")
        ))
        .await?
    {
        lines.push(format!(
            "sequence {} as {} start {} increment {} min {} max {} cache {}{}{}",
            r.text("name"),
            r.text("type"),
            r.text("start"),
            r.text("inc"),
            r.text("min"),
            r.text("max"),
            r.text("cache"),
            if r.flag("cycle") { " cycle" } else { "" },
            r.get("owned").map(|o| format!(" owned by {o}")).unwrap_or_default()
        ));
    }

    for r in db
        .rows(&format!(
            "select format('%I.%I', n.nspname, t.typname) as name, t.typtype as kind,
              (select string_agg(format('%s', to_json(e.enumlabel::text)), ', ' order by e.enumsortorder) from pg_enum e where e.enumtypid = t.oid) as labels,
              case when t.typtype = 'd' then format_type(t.typbasetype, t.typtypmod) end as base, t.typnotnull as notnull, t.typdefault as def,
              (select concat_ws(' ', 'subtype=' || format_type(rg.rngsubtype, null),
                  case when rg.rngcollation <> 0 then 'collate=' || (select format('%I.%I', cn.nspname, co.collname) from pg_collation co join pg_namespace cn on cn.oid = co.collnamespace where co.oid = rg.rngcollation) end,
                  'opclass=' || (select format('%I.%I', on_.nspname, oc.opcname) from pg_opclass oc join pg_namespace on_ on on_.oid = oc.opcnamespace where oc.oid = rg.rngsubopc),
                  case when rg.rngcanonical <> 0 then 'canonical=' || rg.rngcanonical::regproc::text end,
                  case when rg.rngsubdiff <> 0 then 'diff=' || rg.rngsubdiff::regproc::text end)
                from pg_range rg where rg.rngtypid = t.oid) as range
            from pg_type t join pg_namespace n on n.oid = t.typnamespace
            where t.typtype in ('e', 'd', 'r') and {ns} and {} and {}",
            not_extension("pg_type", "t"),
            not_platform("t.typowner")
        ))
        .await?
    {
        let name = r.text("name");
        match r.text("kind") {
            "e" => lines.push(format!("enum {name} ({})", r.text("labels"))),
            "d" => lines.push(format!(
                "domain {name} as {}{}{}",
                r.text("base"),
                if r.flag("notnull") { " not null" } else { "" },
                r.get("def").map(|d| format!(" default {d}")).unwrap_or_default()
            )),
            _ => lines.push(format!("range {name} {}", r.text("range"))),
        }
    }

    for r in db
        .rows(&format!(
            "select pg_get_functiondef(p.oid) as def from pg_proc p join pg_namespace n on n.oid = p.pronamespace
            where p.prokind in ('f', 'p', 'w') and {ns} and {} and {}",
            not_extension("pg_proc", "p"),
            not_platform("p.proowner")
        ))
        .await?
    {
        lines.push(format!("function {}", r.text("def").trim()));
    }

    for r in db
        .rows(&format!(
            "select pg_get_triggerdef(tg.oid, false) as def, tg.tgenabled as enabled
            from pg_trigger tg join pg_class c on c.oid = tg.tgrelid join pg_namespace n on n.oid = c.relnamespace join pg_proc f on f.oid = tg.tgfoid
            where not tg.tgisinternal and {ns} and {} and {}",
            not_extension("pg_class", "c"),
            not_platform("f.proowner")
        ))
        .await?
    {
        let enabled = r.text("enabled");
        lines.push(format!("trigger {}{}", r.text("def"), if enabled == "O" { String::new() } else { format!(" enabled={enabled}") }));
    }

    for r in db
        .rows(
            "select format('%I.%I', n.nspname, c.relname) as rel, p.polname as name, p.polcmd as cmd, p.polpermissive as permissive,
              (select string_agg(case when r = 0 then 'PUBLIC' else pg_get_userbyid(r) end, ',' order by 1) from unnest(p.polroles) r) as roles,
              pg_get_expr(p.polqual, p.polrelid) as qual, pg_get_expr(p.polwithcheck, p.polrelid) as check
            from pg_policy p join pg_class c on c.oid = p.polrelid join pg_namespace n on n.oid = c.relnamespace",
        )
        .await?
    {
        lines.push(format!(
            "policy {} {} {} for {} to {}{}{}",
            r.text("rel"),
            r.text("name"),
            if r.flag("permissive") { "permissive" } else { "restrictive" },
            r.text("cmd"),
            r.text("roles"),
            r.get("qual").map(|q| format!(" using ({q})")).unwrap_or_default(),
            r.get("check").map(|c| format!(" with check ({c})")).unwrap_or_default()
        ));
    }

    for r in db
        .rows(&format!(
            "select format('aggregate %s sfunc=%s stype=%s%s%s kind=%s', p.oid::regprocedure, a.aggtransfn::regproc, format_type(a.aggtranstype, null),
              coalesce(' initcond=' || a.agginitval, ''), case when a.aggfinalfn <> 0 then ' final=' || format('%s', a.aggfinalfn::regproc) else '' end, a.aggkind) as line
            from pg_aggregate a join pg_proc p on p.oid = a.aggfnoid join pg_namespace n on n.oid = p.pronamespace
            where {ns} and {} and {}
            union all select 'rule ' || pg_get_ruledef(r.oid, false) from pg_rewrite r join pg_class c on c.oid = r.ev_class join pg_namespace n on n.oid = c.relnamespace
            where r.rulename <> '_RETURN' and {ns} and {}
            union all select 'statistics ' || pg_get_statisticsobjdef(s.oid) from pg_statistic_ext s join pg_namespace n on n.oid = s.stxnamespace where {ns} and {}",
            not_extension("pg_proc", "p"),
            not_platform("p.proowner"),
            not_platform("c.relowner"),
            not_platform("s.stxowner")
        ))
        .await?
    {
        lines.push(r.text("line").to_string());
    }

    // Privileges on sequences, functions and types, and comments on what is not a relation.
    for r in db
        .rows(&format!(
            "select 'sequence ' || format('%I.%I', n.nspname, c.relname) as name, {} as acl, obj_description(c.oid, 'pg_class') as comment
            from pg_class c join pg_namespace n on n.oid = c.relnamespace where c.relkind = 'S' and {ns} and {} and {}
            union all select 'function ' || format('%s', p.oid::regprocedure), {}, obj_description(p.oid, 'pg_proc')
            from pg_proc p join pg_namespace n on n.oid = p.pronamespace where {ns} and {} and {}
            union all select 'type ' || format('%I.%I', n.nspname, t.typname), {}, obj_description(t.oid, 'pg_type')
            from pg_type t join pg_namespace n on n.oid = t.typnamespace where t.typtype in ('e', 'd', 'r', 'b') and t.typelem = 0 and {ns} and {} and {}
            union all select 'index ' || format('%I.%I', n.nspname, c.relname), null, obj_description(c.oid, 'pg_class')
            from pg_class c join pg_namespace n on n.oid = c.relnamespace where c.relkind in ('i', 'I') and {ns} and {} and {}
            union all select 'constraint ' || format('%I', co.conname) || ' on ' || case when cc.oid is not null then format('%I.%I', cn.nspname, cc.relname) else format('%I.%I', tn.nspname, ct.typname) end, null, obj_description(co.oid, 'pg_constraint')
            from pg_constraint co left join pg_class cc on cc.oid = co.conrelid left join pg_namespace cn on cn.oid = cc.relnamespace
              left join pg_type ct on ct.oid = co.contypid left join pg_namespace tn on tn.oid = ct.typnamespace
            where co.contype <> 'n' and coalesce(cn.nspname, tn.nspname) not in ('pg_catalog', 'information_schema') and coalesce(cc.relname, '') <> '{history}'
            union all select 'trigger ' || format('%I', tg.tgname) || ' on ' || format('%I.%I', n.nspname, c.relname), null, obj_description(tg.oid, 'pg_trigger')
            from pg_trigger tg join pg_class c on c.oid = tg.tgrelid join pg_namespace n on n.oid = c.relnamespace where not tg.tgisinternal and {ns}
            union all select 'policy ' || format('%I', po.polname) || ' on ' || format('%I.%I', n.nspname, c.relname), null, obj_description(po.oid, 'pg_policy')
            from pg_policy po join pg_class c on c.oid = po.polrelid join pg_namespace n on n.oid = c.relnamespace",
            acl("c.relacl", "c.relowner", 's'),
            not_extension("pg_class", "c"),
            not_platform("c.relowner"),
            acl("p.proacl", "p.proowner", 'f'),
            not_extension("pg_proc", "p"),
            not_platform("p.proowner"),
            acl("t.typacl", "t.typowner", 'T'),
            not_extension("pg_type", "t"),
            not_platform("t.typowner"),
            not_extension("pg_class", "c"),
            not_platform("c.relowner"),
        ))
        .await?
    {
        let name = r.text("name");
        if let Some(acl) = r.get("acl") {
            lines.push(format!("grant {name} {acl}"));
        }
        if let Some(comment) = r.get("comment") {
            lines.push(format!("comment {name} {}", json(comment)));
        }
    }

    // Publications, with what each publishes; base types, with the functions that make them.
    for r in db
        .rows(&format!(
            "select format('publication %I%s%s operations=%s%s', p.pubname, case when p.puballtables then ' all tables' else '' end,
              coalesce(' tables=' || (select string_agg(format('%I.%I', n.nspname, c.relname) || coalesce(' where (' || pg_get_expr(pr.prqual, pr.prrelid) || ')', '')
                  || coalesce(' (' || (select string_agg(format('%I', a.attname), ', ' order by a.attnum) from pg_attribute a where a.attrelid = pr.prrelid and a.attnum = any (pr.prattrs)) || ')', ''), ', ' order by 1)
                from pg_publication_rel pr join pg_class c on c.oid = pr.prrelid join pg_namespace n on n.oid = c.relnamespace where pr.prpubid = p.oid), ''),
              concat_ws(',', case when p.pubinsert then 'insert' end, case when p.pubupdate then 'update' end, case when p.pubdelete then 'delete' end, case when p.pubtruncate then 'truncate' end),
              case when p.pubviaroot then ' via root' else '' end) || {} as line
            from pg_publication p where {}
            union all select format('base type %I.%I input=%s output=%s receive=%s send=%s typmod_in=%s typmod_out=%s analyze=%s length=%s byval=%s align=%s storage=%s category=%s preferred=%s delimiter=%s collatable=%s default=%s',
              n.nspname, t.typname, t.typinput, t.typoutput, t.typreceive, t.typsend, t.typmodin, t.typmodout, t.typanalyze, t.typlen, t.typbyval, t.typalign, t.typstorage, t.typcategory, t.typispreferred, t.typdelim, t.typcollation <> 0, coalesce(t.typdefault, ''))
            from pg_type t join pg_namespace n on n.oid = t.typnamespace where t.typtype = 'b' and t.typelem = 0 and t.typarray <> 0 and {ns} and {} and {}",
            if version >= 150000 {
                "coalesce(' schemas=' || (select string_agg(format('%I', pn.nspname), ', ' order by 1) from pg_publication_namespace pp join pg_namespace pn on pn.oid = pp.pnnspid where pp.pnpubid = p.oid), '')"
            } else {
                "''"
            },
            not_platform("p.pubowner"),
            not_extension("pg_type", "t"),
            not_platform("t.typowner"),
        ))
        .await?
    {
        lines.push(r.text("line").to_string());
    }

    // Collations: what decides how text sorts and compares, which the server's library version
    // aside, so that an upgrade of it is no drift.
    for r in db
        .rows(&format!(
            "select format('collation %I.%I provider=%s deterministic=%s', n.nspname, c.collname, c.collprovider, c.collisdeterministic)
              || coalesce(' collate=' || nullif(c.collcollate, ''), '') || coalesce(' ctype=' || nullif(c.collctype, ''), '')
              || coalesce(' locale=' || coalesce(to_jsonb(c) ->> 'colllocale', to_jsonb(c) ->> 'colliculocale'), '')
              || coalesce(' rules=' || (to_jsonb(c) ->> 'collicurules'), '') as line
            from pg_collation c join pg_namespace n on n.oid = c.collnamespace where {ns} and {} and {}",
            not_extension("pg_collation", "c"),
            not_platform("c.collowner")
        ))
        .await?
    {
        lines.push(r.text("line").to_string());
    }

    // Foreign servers, with their wrapper and options.
    for r in db
        .rows(&format!(
            "select format('server %I wrapper=%I', s.srvname, w.fdwname) || coalesce(' type=' || s.srvtype, '') || coalesce(' version=' || s.srvversion, '')
              || coalesce(' options (' || (select string_agg(o, ', ' order by o) from unnest(s.srvoptions) o) || ')', '') as line
            from pg_foreign_server s join pg_foreign_data_wrapper w on w.oid = s.srvfdw where {}",
            not_platform("s.srvowner")
        ))
        .await?
    {
        lines.push(r.text("line").to_string());
    }

    // Every other kind of object, by name alone: its definition is not read, but an object one side
    // has and the other lacks is still a difference rather than a silent match.
    let others = [
        ("pg_operator", "oprnamespace", "oprowner"),
        ("pg_opclass", "opcnamespace", "opcowner"),
        ("pg_opfamily", "opfnamespace", "opfowner"),
        ("pg_conversion", "connamespace", "conowner"),
        ("pg_ts_config", "cfgnamespace", "cfgowner"),
        ("pg_ts_dict", "dictnamespace", "dictowner"),
    ];
    let mut kinds: Vec<String> = others
        .iter()
        .map(|(table, nsp, owner)| {
            format!(
                "select '{table}'::regclass as classid, x.oid as objid from {table} x join pg_namespace n on n.oid = x.{nsp} where {ns} and {} and {}",
                not_extension(table, "x"),
                not_platform(&format!("x.{owner}"))
            )
        })
        .collect();
    kinds.push(format!("select 'pg_cast'::regclass, x.oid from pg_cast x where x.oid >= 16384 and {}", not_extension("pg_cast", "x")));
    kinds.push(format!("select 'pg_event_trigger'::regclass, x.oid from pg_event_trigger x where {} and {}", not_extension("pg_event_trigger", "x"), not_platform("x.evtowner")));
    kinds.push(format!("select 'pg_foreign_data_wrapper'::regclass, x.oid from pg_foreign_data_wrapper x where {} and {}", not_extension("pg_foreign_data_wrapper", "x"), not_platform("x.fdwowner")));
    for r in db
        .rows(&format!("select 'object ' || o.type || ' ' || o.identity || ' (definition not compared)' as line from ({}) k, lateral pg_identify_object(k.classid, k.objid, 0) o", kinds.join(" union all ")))
        .await?
    {
        lines.push(r.text("line").to_string());
    }

    // Owners: what a view, a security definer function and row-level security are checked as. An
    // object a migration made is its runner's, so only one owned by another role has a line, and
    // a database built by one role compares with shadows built by another.
    for r in db.rows(&format!("select format('owner %s is %I (%s)', o.identity, pg_get_userbyid(k.owner), o.type) as line from ({}) k, lateral pg_identify_object(k.classid, k.objid, 0) o where k.owner <> (select oid from pg_roles where rolname = current_user)", owned_objects(&history))).await? {
        lines.push(r.text("line").to_string());
    }

    // Default privileges, a line per grant, as the statements that make them.
    for statement in crate::db::default_privileges(db).await? {
        if let Some(rest) = statement.strip_prefix("alter ") {
            lines.push(rest.to_string());
        }
    }
    Ok(lines)
}

/// What a Postgres schema holds that Reylo reads by name alone, so that a change to its definition is
/// never seen, or doesn't read at all, so that nothing of it is: each a line a person reads, the
/// object and which of the two. None on the other databases, whose every kind is read.
pub async fn unmodeled(db: &mut Db) -> Result<Vec<String>, Error> {
    if db.dialect() != Dialect::Postgres {
        return Ok(vec![]);
    }
    let ns = PG_USER_NS;
    let made_with = |table: &str| format!("not exists (select 1 from pg_depend dep where dep.classid = '{table}'::regclass and dep.objid = x.oid and dep.deptype = 'i')");
    let in_ns = |table: &str, nsp: &str, owner: &str| format!("select '{table}'::regclass as classid, x.oid as objid from {table} x join pg_namespace n on n.oid = x.{nsp} where {ns} and {} and {}", not_extension(table, "x"), not_platform(&format!("x.{owner}")));
    let by_name = [
        in_ns("pg_operator", "oprnamespace", "oprowner"),
        in_ns("pg_opclass", "opcnamespace", "opcowner"),
        in_ns("pg_opfamily", "opfnamespace", "opfowner"),
        in_ns("pg_conversion", "connamespace", "conowner"),
        in_ns("pg_ts_config", "cfgnamespace", "cfgowner"),
        in_ns("pg_ts_dict", "dictnamespace", "dictowner"),
        format!("select 'pg_cast'::regclass as classid, x.oid as objid from pg_cast x where x.oid >= 16384 and {} and {}", not_extension("pg_cast", "x"), made_with("pg_cast")),
        format!("select 'pg_event_trigger'::regclass, x.oid from pg_event_trigger x where {} and {}", not_extension("pg_event_trigger", "x"), not_platform("x.evtowner")),
        format!("select 'pg_foreign_data_wrapper'::regclass, x.oid from pg_foreign_data_wrapper x where {} and {}", not_extension("pg_foreign_data_wrapper", "x"), not_platform("x.fdwowner")),
    ];
    let not_read = [
        format!("select 'pg_ts_parser'::regclass as classid, x.oid as objid from pg_ts_parser x join pg_namespace n on n.oid = x.prsnamespace where {ns} and {}", not_extension("pg_ts_parser", "x")),
        format!("select 'pg_ts_template'::regclass, x.oid from pg_ts_template x join pg_namespace n on n.oid = x.tmplnamespace where {ns} and {}", not_extension("pg_ts_template", "x")),
        format!("select 'pg_language'::regclass, x.oid from pg_language x where x.oid >= 16384 and {}", not_extension("pg_language", "x")),
        format!("select 'pg_transform'::regclass, x.oid from pg_transform x where {}", not_extension("pg_transform", "x")),
        format!("select 'pg_am'::regclass, x.oid from pg_am x where x.oid >= 16384 and {}", not_extension("pg_am", "x")),
        format!("select 'pg_user_mapping'::regclass, x.oid from pg_user_mapping x join pg_foreign_server s on s.oid = x.umserver where {}", not_platform("s.srvowner")),
    ];
    let mut out = Vec::new();
    for (queries, why) in [(&by_name[..], "read by name only"), (&not_read[..], "not read")] {
        let sql = format!("select o.type || ' ' || o.identity as what from ({}) k, lateral pg_identify_object(k.classid, k.objid, 0) o order by 1", queries.join(" union all "));
        out.extend(db.rows(&sql).await?.iter().map(|r| format!("{}: {why}", r.text("what"))));
    }
    // Security labels, and a column's own storage, compression and statistics target.
    let labels = format!(
        "select format('security label on %s %s', l.objtype, l.objname) as what from pg_seclabels l join pg_namespace n on n.oid = l.objnamespace where {ns} and {} order by 1",
        not_platform("n.nspowner")
    );
    out.extend(db.rows(&labels).await?.iter().map(|r| format!("{}: not read", r.text("what"))));
    let columns = format!(
        "select format('%s of column %I.%I.%I', concat_ws(', ', case when a.attstorage <> t.typstorage then 'storage' end, case when a.attcompression <> '' then 'compression' end,
            case when coalesce(a.attstattarget, -1) >= 0 then 'statistics target' end), n.nspname, c.relname, a.attname) as what
          from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace join pg_type t on t.oid = a.atttypid
          where a.attnum > 0 and not a.attisdropped and c.relkind in ('r', 'p', 'm') and {ns} and {} and {}
            and (a.attstorage <> t.typstorage or a.attcompression <> '' or coalesce(a.attstattarget, -1) >= 0) order by 1",
        not_extension("pg_class", "c"),
        not_platform("c.relowner")
    );
    out.extend(db.rows(&columns).await?.iter().map(|r| format!("{}: not read", r.text("what"))));
    Ok(out)
}

/// The objects whose owner is compared, each `classid, objid, owner`: relations (but indexes, which
/// take their table's), routines (but those made with a range type), types (but arrays and a
/// relation's own row type), schemas, collations, publications and foreign servers; none an
/// extension or a platform owns, and not Reylo's history table.
pub(crate) fn owned_objects(history: &str) -> String {
    let ns = PG_USER_NS;
    [
        format!("select 'pg_class'::regclass as classid, c.oid as objid, c.relowner as owner from pg_class c join pg_namespace n on n.oid = c.relnamespace where c.relkind in ('r', 'p', 'v', 'm', 'f', 'S', 'c') and c.relname <> '{history}' and {ns} and {} and {}", not_extension("pg_class", "c"), not_platform("c.relowner")),
        format!("select 'pg_proc'::regclass, p.oid, p.proowner from pg_proc p join pg_namespace n on n.oid = p.pronamespace where {ns} and {} and {} and not exists (select 1 from pg_depend dep where dep.classid = 'pg_proc'::regclass and dep.objid = p.oid and dep.deptype = 'i')", not_extension("pg_proc", "p"), not_platform("p.proowner")),
        format!("select 'pg_type'::regclass, t.oid, t.typowner from pg_type t join pg_namespace n on n.oid = t.typnamespace where t.typtype in ('b', 'd', 'e', 'r') and t.typcategory <> 'A' and {ns} and {} and {} and not exists (select 1 from pg_depend dep where dep.classid = 'pg_type'::regclass and dep.objid = t.oid and dep.deptype = 'i')", not_extension("pg_type", "t"), not_platform("t.typowner")),
        format!("select 'pg_namespace'::regclass, n.oid, n.nspowner from pg_namespace n where {ns} and {} and {}", not_extension("pg_namespace", "n"), not_platform("n.nspowner")),
        format!("select 'pg_collation'::regclass, c.oid, c.collowner from pg_collation c join pg_namespace n on n.oid = c.collnamespace where {ns} and {} and {}", not_extension("pg_collation", "c"), not_platform("c.collowner")),
        format!("select 'pg_publication'::regclass, p.oid, p.pubowner from pg_publication p where {}", not_platform("p.pubowner")),
        format!("select 'pg_foreign_server'::regclass, s.oid, s.srvowner from pg_foreign_server s where {}", not_platform("s.srvowner")),
    ]
    .join(" union all ")
}

// ---------------------------------------------------------------------------------------------
// MySQL, from information_schema, with the database's own name taken out of the definitions that
// carry it (a view's body names its tables with their database).

async fn mysql(db: &mut Db) -> Result<Vec<String>, Error> {
    let name = db.rows("select database() as name").await?.first().map(|r| r.text("name").to_string()).unwrap_or_default();
    let strip = |s: &str| s.replace(&format!("`{name}`."), "").replace(&format!("{name}."), "");
    let history = HISTORY_TABLE;
    let mut lines = Vec::new();

    for r in db
        .rows(&format!(
            "select table_name as name, table_type as type, engine, table_collation as collation, create_options as options, table_comment as comment
            from information_schema.tables where table_schema = database() and table_name <> '{history}'"
        ))
        .await?
    {
        if r.text("type") == "VIEW" {
            lines.push(format!("view {}", r.text("name")));
            continue;
        }
        let mut line = format!("table {} engine={} collate={}", r.text("name"), r.text("engine"), r.text("collation"));
        if !r.text("options").is_empty() {
            line += &format!(" options={}", r.text("options"));
        }
        lines.push(line);
        if !r.text("comment").is_empty() {
            lines.push(format!("comment {} {}", r.text("name"), json(r.text("comment"))));
        }
    }

    for r in db
        .rows(&format!(
            "select table_name as t, column_name as c, column_type as type, is_nullable as nullable, column_default as def, extra,
              character_set_name as charset, collation_name as collation, generation_expression as gen, column_comment as comment, srs_id as srs
            from information_schema.columns where table_schema = database() and table_name <> '{history}'"
        ))
        .await?
    {
        let mut parts = vec![format!("column {}.{} {}", r.text("t"), r.text("c"), r.text("type")), if r.text("nullable") == "YES" { "null".into() } else { "not null".into() }];
        if let Some(def) = r.get("def") {
            parts.push(format!("default {}", json(def)));
        }
        if !r.text("extra").is_empty() {
            parts.push(r.text("extra").to_lowercase());
        }
        if !r.text("gen").is_empty() {
            parts.push(format!("as ({})", strip(r.text("gen"))));
        }
        if let Some(collation) = r.get("collation") {
            parts.push(format!("collate {collation}"));
        }
        if !r.text("comment").is_empty() {
            lines.push(format!("comment {}.{} {}", r.text("t"), r.text("c"), json(r.text("comment"))));
        }
        if let Some(srs) = r.get("srs") {
            parts.push(format!("srid {srs}"));
        }
        lines.push(parts.join(" "));
    }

    let mut indexes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in db
        .rows(&format!(
            "select table_name as t, index_name as i, non_unique as nonunique, seq_in_index as seq, column_name as col, sub_part as sub,
              collation as coll, expression as expr, index_type as type, is_visible as visible
            from information_schema.statistics where table_schema = database() and table_name <> '{history}' order by table_name, index_name, seq_in_index"
        ))
        .await?
    {
        let key = format!(
            "index {}.{} {}{}{}",
            r.text("t"),
            r.text("i"),
            if r.text("nonunique") != "0" { "" } else { "unique " },
            r.text("type"),
            if r.text("visible") == "NO" { " invisible" } else { "" }
        );
        let part = format!(
            "{}{}{}",
            r.get("col").map(String::from).unwrap_or_else(|| format!("({})", strip(r.text("expr")))),
            r.get("sub").map(|s| format!("({s})")).unwrap_or_default(),
            if r.text("coll") == "D" { " desc" } else { "" }
        );
        indexes.entry(key).or_default().push(part);
    }
    for (key, parts) in indexes {
        lines.push(format!("{key} ({})", parts.join(", ")));
    }

    let mut fks: BTreeMap<String, (Vec<String>, Vec<String>, String, String, String)> = BTreeMap::new();
    for r in db
        .rows(
            "select k.table_name as t, k.constraint_name as n, k.column_name as col, k.referenced_table_name as ref, k.referenced_column_name as refcol,
              rc.update_rule as upd, rc.delete_rule as del
            from information_schema.key_column_usage k join information_schema.referential_constraints rc
              on rc.constraint_schema = k.constraint_schema and rc.constraint_name = k.constraint_name and rc.table_name = k.table_name
            where k.table_schema = database() and k.referenced_table_name is not null order by k.table_name, k.constraint_name, k.ordinal_position",
        )
        .await?
    {
        let fk = fks.entry(format!("{} {}", r.text("t"), r.text("n"))).or_insert_with(|| (vec![], vec![], r.text("ref").into(), r.text("upd").into(), r.text("del").into()));
        fk.0.push(r.text("col").into());
        fk.1.push(r.text("refcol").into());
    }
    for (key, (cols, refs, table, update, delete)) in fks {
        lines.push(format!("foreign key {key} ({}) references {table} ({}) on update {update} on delete {delete}", cols.join(", "), refs.join(", ")));
    }

    for r in db
        .rows(
            "select tc.table_name as t, tc.constraint_name as n, cc.check_clause as clause, tc.enforced
            from information_schema.table_constraints tc join information_schema.check_constraints cc
              on cc.constraint_schema = tc.constraint_schema and cc.constraint_name = tc.constraint_name
            where tc.table_schema = database() and tc.constraint_type = 'CHECK'",
        )
        .await?
    {
        lines.push(format!("check {} {} {}{}", r.text("t"), r.text("n"), strip(r.text("clause")), if r.text("enforced") == "NO" { " not enforced" } else { "" }));
    }

    for r in db.rows("select table_name as n, view_definition as def, check_option as opt, security_type as sec, definer from information_schema.views where table_schema = database()").await? {
        lines.push(format!("view {} as {} check={} security={} definer={}", r.text("n"), strip(r.text("def")), r.text("opt"), r.text("sec"), r.text("definer")));
    }

    for r in db
        .rows(
            "select r.routine_name as n, r.routine_type as type, r.dtd_identifier as ret, r.routine_definition as def, r.is_deterministic as det,
              r.sql_data_access as access, r.security_type as sec, r.definer,
              (select group_concat(concat_ws(' ', p.parameter_mode, p.parameter_name, p.dtd_identifier) order by p.ordinal_position separator ', ')
                 from information_schema.parameters p where p.specific_schema = r.routine_schema and p.specific_name = r.specific_name and p.ordinal_position > 0) as params
            from information_schema.routines r where r.routine_schema = database()",
        )
        .await?
    {
        lines.push(format!(
            "{} {}({}){} {} {} security={} definer={} as {}",
            r.text("type").to_lowercase(),
            r.text("n"),
            r.text("params"),
            r.get("ret").map(|t| format!(" returns {t}")).unwrap_or_default(),
            if r.text("det") == "YES" { "deterministic" } else { "not deterministic" },
            r.text("access"),
            r.text("sec"),
            r.text("definer"),
            strip(r.text("def"))
        ));
    }

    for r in db
        .rows(
            "select trigger_name as n, event_manipulation as ev, event_object_table as t, action_statement as body, action_timing as timing, action_order as ord, definer
            from information_schema.triggers where trigger_schema = database()",
        )
        .await?
    {
        lines.push(format!("trigger {} {} {} on {} order={} definer={} {}", r.text("n"), r.text("timing"), r.text("ev"), r.text("t"), r.text("ord"), r.text("definer"), strip(r.text("body"))));
    }

    // Recurring events, without the start the server fills in as each is made, which no two
    // databases share. A one-time event is left out: once it fires, the server drops it, or
    // disables it, as it was told to.
    for r in db
        .rows(
            "select event_name as n, event_type as type, interval_value as every, interval_field as unit, on_completion as completion, status, definer, event_definition as body, event_comment as comment
            from information_schema.events where event_schema = database() and event_type <> 'ONE TIME'",
        )
        .await?
    {
        let every = r.get("every").map(|v| format!(" every {v} {}", r.text("unit"))).unwrap_or_default();
        lines.push(format!("event {} {}{every} on completion {} {} definer={} do {}", r.text("n"), r.text("type").to_lowercase(), r.text("completion").to_lowercase(), r.text("status").to_lowercase(), r.text("definer"), strip(r.text("body"))));
        if !r.text("comment").is_empty() {
            lines.push(format!("comment event {} {}", r.text("n"), json(r.text("comment"))));
        }
    }

    // Partitioning: each table's scheme, and each partition as a child of its table, which, like a
    // Postgres partition, is data where it was made at run time (a month's partition a job adds).
    let mut schemes: BTreeMap<String, String> = BTreeMap::new();
    let mut parts: BTreeMap<(String, String), (String, Vec<String>)> = BTreeMap::new();
    for r in db
        .rows(&format!(
            "select table_name as t, partition_name as p, subpartition_name as sp, partition_method as method, partition_expression as expr,
              subpartition_method as submethod, subpartition_expression as subexpr, partition_description as bound
            from information_schema.partitions where table_schema = database() and partition_name is not null and table_name <> '{history}'
            order by table_name, partition_ordinal_position, subpartition_ordinal_position"
        ))
        .await?
    {
        let t = r.text("t").to_string();
        let sub = r.get("submethod").map(|m| format!(" subpartition by {m} ({})", strip(r.text("subexpr")))).unwrap_or_default();
        schemes.insert(t.clone(), format!("partitioning {t} by {} ({}){sub}", r.text("method"), strip(r.text("expr"))));
        let bound = r.get("bound").map(|b| format!(" values {b}")).unwrap_or_default();
        let entry = parts.entry((t, r.text("p").to_string())).or_insert_with(|| (bound, vec![]));
        if let Some(sp) = r.get("sp") {
            entry.1.push(sp.to_string());
        }
    }
    lines.extend(schemes.into_values());
    for ((t, p), (bound, subs)) in parts {
        let subs = if subs.is_empty() { String::new() } else { format!(" subpartitions ({})", subs.join(", ")) };
        lines.push(format!("table {t}.{p} partition of {t}{bound}{subs}"));
    }
    Ok(lines)
}

// ---------------------------------------------------------------------------------------------
// SQLite. The schema is the `create` text SQLite stores, which keeps the spelling it was written
// in, so it is rendered again token by token: names unquoted and lowercased (SQLite compares names
// without regard to case), keywords lowercased, one space between tokens, literals exact. A table's
// columns and constraints are split apart and compared as sets, since `alter table add column`
// appends to the text while a fresh `create table` writes the column in its place.

fn canonical_sqlite(tokens: &[Token]) -> String {
    let joined = tokens
        .iter()
        .map(|t| match t.kind {
            TokenKind::Quoted => {
                let inner = &t.text[1..t.text.len().saturating_sub(1).max(1)];
                inner.replace("\"\"", "\"").replace("``", "`").replace("]]", "]").to_lowercase()
            }
            TokenKind::Word => t.text.to_lowercase(),
            _ => t.text.clone(),
        })
        .collect::<Vec<_>>()
        .join(" ");
    // ` , ` and ` ) ` lose the space before them and keep one after; `( `, ` .` and `. ` lose theirs.
    let mut out = String::with_capacity(joined.len());
    let chars: Vec<char> = joined.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ' ' && matches!(chars.get(i + 1), Some(',' | ')')) {
            out.push(chars[i + 1]);
            out.push(' ');
            i += if chars.get(i + 2) == Some(&' ') { 3 } else { 2 };
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out.replace("( ", "(").replace(" .", ".").replace(". ", ".").trim().to_string()
}

const SQLITE_TABLE_CONSTRAINT: &[&str] = &["constraint", "primary", "unique", "check", "foreign"];

async fn sqlite(db: &mut Db) -> Result<Vec<String>, Error> {
    let mut lines = Vec::new();
    let rows = db.rows(&format!("select type, name, tbl_name as tbl, sql from sqlite_schema where name not like 'sqlite_%' and tbl_name <> '{HISTORY_TABLE}'")).await?;
    let options: BTreeMap<String, Row> = db.rows("select name, strict, wr from pragma_table_list where schema = 'main'").await?.into_iter().map(|r| (r.text("name").to_string(), r)).collect();
    for r in rows {
        let Some(sql) = r.get("sql") else { continue };
        let tokens = tokenize(sql, Dialect::Sqlite);
        let kind = r.text("type");
        if kind != "table" {
            lines.push(format!("{kind} {} {}", r.text("name").to_lowercase(), canonical_sqlite(&tokens)));
            continue;
        }
        let name = r.text("name").to_lowercase();
        let option = options.get(r.text("name"));
        lines.push(format!(
            "table {name}{}{}",
            if option.is_some_and(|o| o.text("strict") == "1") { " strict" } else { "" },
            if option.is_some_and(|o| o.text("wr") == "1") { " without rowid" } else { "" }
        ));
        // The parts of the column list, at its top level.
        let open = tokens.iter().position(|t| t.text == "(").map_or(tokens.len(), |at| at + 1);
        let mut depth = 0;
        let mut part: Vec<Token> = Vec::new();
        let mut parts: Vec<Vec<Token>> = Vec::new();
        for t in &tokens[open..] {
            if t.text == "(" {
                depth += 1;
            }
            if t.text == ")" {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            if t.text == "," && depth == 0 {
                parts.push(std::mem::take(&mut part));
                continue;
            }
            part.push(t.clone());
        }
        if !part.is_empty() {
            parts.push(part);
        }
        for p in parts {
            let first = p[0].text.to_lowercase();
            if p[0].kind == TokenKind::Word && SQLITE_TABLE_CONSTRAINT.contains(&first.as_str()) {
                lines.push(format!("constraint {name} {}", canonical_sqlite(&p)));
            } else {
                lines.push(format!("column {name}.{}", canonical_sqlite(&p)));
            }
        }
    }
    Ok(lines)
}

/// Postgres's SQL on one line: the whitespace between two tokens read as one space, and each token
/// (a string, a quoted name, a dollar-quoted body) kept as written, so that `'a  b'` and `'a b'`
/// still differ.
fn one_line(sql: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for t in tokenize(sql, Dialect::Postgres) {
        if !out.is_empty() && sql[last..t.start].chars().any(char::is_whitespace) {
            out.push(' ');
        }
        out.push_str(&sql[t.start..t.end]);
        last = t.end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_views_text_keeps_the_spacing_inside_its_strings() {
        assert_eq!(one_line(" SELECT 'a  b' AS x,\n    \"y  z\"::text\n   FROM t;"), "SELECT 'a  b' AS x, \"y  z\"::text FROM t;");
        assert_ne!(one_line("SELECT 'a  b'"), one_line("SELECT 'a b'"));
        assert_ne!(one_line("SELECT $$a  b$$"), one_line("SELECT $$a b$$"));
    }

    #[test]
    fn sqlite_text_is_canonical_whatever_its_spelling() {
        let a = canonical_sqlite(&tokenize("CREATE INDEX \"I\" ON t ( a , b )", Dialect::Sqlite));
        let b = canonical_sqlite(&tokenize("create index i on T(a,b)", Dialect::Sqlite));
        assert_eq!(a, b);
        assert_eq!(a, "create index i on t (a, b)");
    }
}
