//! The locks a Postgres migration holds on tables that already have rows. A statement that checks
//! every row, builds an index or copies a table while it holds a lock that blocks writes stops the
//! app's writes to that table for as long as it runs, which on a large table is an outage. Most such
//! changes have a shape that doesn't: a constraint added `not valid` and validated in a later
//! transaction (validation blocks no writes), a `not null` proven by a validated check first, an
//! index built `concurrently` in a file of its own. `lock_safe` writes a plan in those shapes, as
//! several files; `run_watched` runs a file on a shadow and says which of its statements still hold
//! such a lock, reading the parse tree for the scans and the tables' storage for the rewrites, so
//! that whether a statement copies a table is Postgres's own answer, not a list of rules.

use crate::commands::{run, Migration};
use crate::db::Db;
use crate::diff::postgres::quote_ident;
use crate::diff::Plan;
use crate::{Dialect, Error};
use pg_query::protobuf::{node::Node as N, AlterTableType as At, ConstrType, RangeVar};
use std::collections::{BTreeMap, BTreeSet};

/// The plain tables of `db` (not partitioned ones, where neither `not valid` foreign keys nor
/// concurrent index builds are taken on every supported version), each under the name a
/// statement on the search path writes it with and under its qualified name.
pub async fn existing_tables(db: &mut Db) -> Result<BTreeSet<String>, Error> {
    let rows = db
        .rows(
            r"select c.oid::regclass::text as shown, format('%I.%I', n.nspname, c.relname) as qualified
            from pg_class c join pg_namespace n on n.oid = c.relnamespace
            where c.relkind = 'r' and n.nspname not in ('pg_catalog', 'information_schema') and n.nspname not like 'pg\_%'",
        )
        .await?;
    Ok(rows.iter().flat_map(|r| [r.text("shown").to_string(), r.text("qualified").to_string()]).collect())
}

/// A relation as a statement names it: `users`, `billing.invoices`, each part quoted as needed.
fn named(r: &RangeVar) -> String {
    if r.schemaname.is_empty() {
        quote_ident(&r.relname)
    } else {
        format!("{}.{}", quote_ident(&r.schemaname), quote_ident(&r.relname))
    }
}

/// The one statement `sql` holds, parsed.
fn parsed(sql: &str) -> Option<N> {
    let result = pg_query::parse(sql, 0).ok()?;
    let [raw] = result.protobuf.stmts.as_slice() else { return None };
    raw.stmt.as_ref()?.node.clone()
}

/// The columns of a constraint's key, where each is a plain column.
fn key_columns(keys: &[pg_query::protobuf::Node]) -> Option<Vec<String>> {
    keys.iter()
        .map(|k| match &k.node {
            Some(N::String(s)) => Some(quote_ident(&s.sval)),
            _ => None,
        })
        .collect()
}

/// The name of the check that proves a column holds no null before `set not null` reads it: the
/// column's, cut where a name would pass Postgres's 63 bytes.
fn not_null_check(column: &str) -> String {
    const SUFFIX: &str = "_not_null";
    let mut end = column.len().min(63 - SUFFIX.len());
    while !column.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{SUFFIX}", &column[..end])
}

/// Whether a parse tree's enum field holds `value`.
fn is<T: Into<i32>>(field: i32, value: T) -> bool {
    field == value.into()
}

/// What one statement of a plan becomes in the lock-safe shape.
enum Shape {
    /// Kept as it is, in the file being written.
    Kept(String),
    /// In the file being written, then statements for the last file.
    Later(String, Vec<String>),
    /// In a file of its own, then statements for the file after it.
    Alone(String, Vec<String>),
}

/// `statement` in the shape that holds no lock that blocks writes to a table in `existing` while
/// it reads every row.
fn shape(statement: &str, existing: &BTreeSet<String>) -> Shape {
    let kept = || Shape::Kept(statement.to_string());
    let Some(node) = parsed(statement) else { return kept() };
    match node {
        N::AlterTableStmt(a) => {
            let Some(rel) = a.relation.as_ref().map(named).filter(|r| existing.contains(r)) else { return kept() };
            let [cmd] = a.cmds.as_slice() else { return kept() };
            let Some(N::AlterTableCmd(cmd)) = &cmd.node else { return kept() };
            if is(cmd.subtype, At::AtAddConstraint) {
                let Some(N::Constraint(c)) = cmd.def.as_ref().and_then(|d| d.node.as_ref()) else { return kept() };
                if c.conname.is_empty() || c.skip_validation {
                    return kept();
                }
                // Added without reading a row, then validated later, which blocks no writes.
                if is(c.contype, ConstrType::ConstrCheck) || is(c.contype, ConstrType::ConstrForeign) {
                    return Shape::Later(format!("{} not valid", statement.trim_end()), vec![format!("alter table {rel} validate constraint {}", quote_ident(&c.conname))]);
                }
                // Its index built concurrently, then taken as the constraint's.
                let plain = c.including.is_empty() && c.options.is_empty() && !c.nulls_not_distinct && !c.deferrable && c.indexname.is_empty() && c.indexspace.is_empty() && c.access_method.is_empty() && !c.without_overlaps;
                if is(c.contype, ConstrType::ConstrUnique) && plain {
                    let Some(columns) = key_columns(&c.keys) else { return kept() };
                    let name = quote_ident(&c.conname);
                    return Shape::Alone(format!("create unique index concurrently {name} on {rel} ({})", columns.join(", ")), vec![format!("alter table {rel} add constraint {name} unique using index {name}")]);
                }
                return kept();
            }
            // A validated check that the column is not null lets `set not null` skip its scan.
            if is(cmd.subtype, At::AtSetNotNull) && !cmd.name.is_empty() {
                let column = quote_ident(&cmd.name);
                let check = quote_ident(&not_null_check(&cmd.name));
                return Shape::Later(
                    format!("alter table {rel} add constraint {check} check ({column} is not null) not valid"),
                    vec![format!("alter table {rel} validate constraint {check}"), statement.trim_end().to_string(), format!("alter table {rel} drop constraint {check}")],
                );
            }
            kept()
        }
        N::IndexStmt(i) if !i.concurrent && i.relation.as_ref().map(named).is_some_and(|r| existing.contains(&r)) => {
            static CREATE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)^(\s*create\s+(?:unique\s+)?index)\b").expect("a valid pattern"));
            if !CREATE.is_match(statement) {
                return kept();
            }
            Shape::Alone(CREATE.replace(statement.trim_end(), "$1 concurrently").into_owned(), vec![])
        }
        _ => kept(),
    }
}

/// `plan`'s statements as files that hold no lock blocking writes to a table in `existing` (one
/// that already has rows) while they read every row: where any statement does, the files in order,
/// comments kept with the statement they come before; `None` where the plan holds no such lock.
pub fn lock_safe(plan: &Plan, existing: &BTreeSet<String>) -> Option<Vec<Vec<String>>> {
    let mut files: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut last: Vec<String> = Vec::new();
    let mut comments: Vec<String> = Vec::new();
    let mut changed = false;
    // The session's settings in force (`set check_function_bodies = off` around functions), which a
    // file after a lone index build repeats: a deploy cut off between the files starts the next run
    // there, in a new session.
    let mut settings: Vec<String> = Vec::new();
    for statement in &plan.statements {
        if statement.starts_with("--") {
            comments.push(statement.clone());
            continue;
        }
        let lower = statement.trim_start().to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("set ").filter(|r| !r.starts_with("local ")) {
            let name = rest.split(['=', ' ']).next().unwrap_or_default().to_string();
            settings.retain(|s| !s.to_ascii_lowercase().strip_prefix("set ").is_some_and(|r| r.starts_with(&name)));
            settings.push(statement.clone());
        } else if let Some(name) = lower.strip_prefix("reset ") {
            settings.retain(|s| !s.to_ascii_lowercase().strip_prefix("set ").is_some_and(|r| r.starts_with(name.trim())));
        }
        match shape(statement, existing) {
            Shape::Kept(s) => {
                current.append(&mut comments);
                current.push(s);
            }
            Shape::Later(s, after) => {
                changed = true;
                current.append(&mut comments);
                current.push(s);
                last.extend(after);
            }
            Shape::Alone(s, after) => {
                changed = true;
                // What came before goes in a file of its own, unless it is only settings, which
                // the file after this one repeats.
                if does_something(&current) {
                    files.push(std::mem::take(&mut current));
                } else {
                    current.retain(|s| s.starts_with("--"));
                }
                let mut alone = std::mem::take(&mut current);
                alone.append(&mut comments);
                alone.push(s);
                files.push(alone);
                current.extend(settings.iter().cloned());
                current.extend(after);
            }
        }
    }
    current.append(&mut comments);
    if !changed {
        return None;
    }
    if does_something(&current) {
        files.push(current);
    }
    if !last.is_empty() {
        files.push(last);
    }
    Some(files)
}

/// Whether statements change anything: more than comments and the session's settings.
fn does_something(statements: &[String]) -> bool {
    statements.iter().any(|s| {
        let lower = s.trim_start().to_ascii_lowercase();
        !s.starts_with("--") && !lower.starts_with("set ") && !lower.starts_with("reset ")
    })
}

/// The files' texts, as `reylo migrate new` writes them: what the plan loses as comments at the top
/// of the first, then each statement.
pub fn file_texts(plan: &Plan, files: &[Vec<String>]) -> Vec<String> {
    files
        .iter()
        .enumerate()
        .map(|(k, statements)| {
            let mut head = Plan { statements: statements.clone(), ..Plan::default() };
            if k == 0 {
                head.losses = plan.losses.clone();
                head.narrowings = plan.narrowings.clone();
            }
            head.sql()
        })
        .collect()
}

/// A lock a statement holds on a table that already has rows, for as long as it reads them all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hazard {
    /// The table, as the database names it.
    pub table: String,
    /// What it does, as a sentence's predicate: `rewrites users, …`.
    pub what: String,
}

/// The check constraints that prove a column holds no null: (table, column) to (name, validated).
async fn not_null_proofs(db: &mut Db) -> Result<BTreeMap<(String, String), (String, bool)>, Error> {
    static PROOF: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r#"^CHECK \(\(?("(?:[^"]|"")+"|[a-z_][a-z0-9_$]*) IS NOT NULL\)?\)(?: NOT VALID)?$"#).expect("a valid pattern"));
    let rows = db.rows("select conrelid::regclass::text as t, conname as name, convalidated as valid, pg_get_constraintdef(oid) as def from pg_constraint where contype = 'c'").await?;
    let mut out = BTreeMap::new();
    for r in rows {
        if let Some(c) = PROOF.captures(r.text("def")) {
            let column = c[1].trim_matches('"').replace("\"\"", "\"");
            out.insert((r.text("t").to_string(), column), (r.text("name").to_string(), r.flag("valid")));
        }
    }
    Ok(out)
}

/// The storage of each table and materialized view, by the name the database gives it: a table
/// rewritten (a type change that copies it, a volatile default, `vacuum full`) gets new storage.
async fn storage(db: &mut Db) -> Result<BTreeMap<String, (String, String)>, Error> {
    let rows = db
        .rows(
            r"select c.oid::regclass::text as t, c.oid::text as oid, c.relfilenode::text as file from pg_class c join pg_namespace n on n.oid = c.relnamespace
            where c.relkind in ('r', 'm') and n.nspname not in ('pg_catalog', 'information_schema') and n.nspname not like 'pg\_%'",
        )
        .await?;
    Ok(rows.iter().map(|r| (r.text("t").to_string(), (r.text("oid").to_string(), r.text("file").to_string()))).collect())
}

/// Runs `m` as `deploy` runs it, saying each lock it holds on a table that was there before it ran
/// while it reads every row: the parse tree for a scan (a validated constraint, an index built
/// without `concurrently`, a `set not null` no validated check proves), and the tables' storage for
/// a copy. Postgres only; elsewhere it runs the file and says nothing.
pub(crate) async fn run_watched(db: &mut Db, m: &Migration) -> Result<Vec<Hazard>, Error> {
    if db.dialect() != Dialect::Postgres {
        run(db, m, false).await?;
        return Ok(vec![]);
    }
    let before = storage(db).await?;
    let proofs = not_null_proofs(db).await?;
    // A statement names a table as the search path finds it; the database's own name for it is
    // what the hazard gives.
    let resolve = |name: &str, before: &BTreeMap<String, (String, String)>| -> Option<String> { before.contains_key(name).then(|| name.to_string()).or_else(|| before.keys().find(|k| k.rsplit_once('.').is_some_and(|(_, t)| t == name)).cloned()) };
    let mut hazards: Vec<Hazard> = Vec::new();
    let mut truncated: BTreeSet<String> = BTreeSet::new();
    let mut validated: BTreeSet<(String, String)> = BTreeSet::new();
    let mut add = |table: &str, what: String| {
        let hazard = Hazard { table: table.to_string(), what };
        if !hazards.contains(&hazard) {
            hazards.push(hazard);
        }
    };
    if let Ok(result) = pg_query::parse(&m.sql, 0) {
        for raw in &result.protobuf.stmts {
            match raw.stmt.as_ref().and_then(|s| s.node.as_ref()) {
                Some(N::AlterTableStmt(a)) => {
                    let Some(table) = a.relation.as_ref().and_then(|r| resolve(&named(r), &before)) else { continue };
                    for cmd in &a.cmds {
                        let Some(N::AlterTableCmd(cmd)) = &cmd.node else { continue };
                        if is(cmd.subtype, At::AtAddConstraint) {
                            let Some(N::Constraint(c)) = cmd.def.as_ref().and_then(|d| d.node.as_ref()) else { continue };
                            if is(c.contype, ConstrType::ConstrCheck) && !c.skip_validation {
                                add(&table, format!("checks every row of {table} while it blocks reads and writes to it"));
                            } else if is(c.contype, ConstrType::ConstrForeign) && !c.skip_validation {
                                add(&table, format!("checks every row of {table} while it blocks writes to it"));
                            } else if (is(c.contype, ConstrType::ConstrUnique) || is(c.contype, ConstrType::ConstrPrimary) || is(c.contype, ConstrType::ConstrExclusion)) && c.indexname.is_empty() {
                                add(&table, format!("builds an index on {table} while it blocks reads and writes to it"));
                            }
                        } else if is(cmd.subtype, At::AtValidateConstraint) {
                            validated.insert((table.clone(), cmd.name.clone()));
                        } else if is(cmd.subtype, At::AtSetNotNull) {
                            let proven = proofs.get(&(table.clone(), cmd.name.clone())).is_some_and(|(name, valid)| *valid || validated.contains(&(table.clone(), name.clone())));
                            if !proven {
                                add(&table, format!("checks every row of {table} for a null in {} while it blocks reads and writes to it", quote_ident(&cmd.name)));
                            }
                        }
                    }
                }
                Some(N::IndexStmt(i)) if !i.concurrent => {
                    if let Some(table) = i.relation.as_ref().and_then(|r| resolve(&named(r), &before)) {
                        add(&table, format!("builds an index on {table} while it blocks writes to it"));
                    }
                }
                Some(N::TruncateStmt(t)) => {
                    for r in &t.relations {
                        if let Some(N::RangeVar(r)) = &r.node {
                            truncated.extend(resolve(&named(r), &before));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    run(db, m, false).await?;
    // A table renamed keeps its storage; one given new storage was copied, but for a truncate,
    // which empties it at once.
    let after = storage(db).await?;
    for (table, (oid, file)) in &before {
        if truncated.contains(table) {
            continue;
        }
        if after.values().any(|(o, f)| o == oid && f != file) {
            hazards.retain(|h| &h.table != table);
            hazards.push(Hazard { table: table.clone(), what: format!("rewrites {table}, copying every row while it blocks reads and writes to it") });
        }
    }
    Ok(hazards)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(statements: &[&str]) -> Plan {
        Plan { statements: statements.iter().map(|s| s.to_string()).collect(), ..Plan::default() }
    }

    fn existing(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn leaves_a_plan_with_nothing_to_lock_alone() {
        assert_eq!(lock_safe(&plan(&["create table t (id int)", "alter table users add column bio text"]), &existing(&["users"])), None);
        // A new table has no rows to read.
        assert_eq!(lock_safe(&plan(&["create table t (id int)", "create index t_id on t (id)", "alter table t add constraint t_pos check (id > 0)"]), &existing(&["users"])), None);
    }

    #[test]
    fn validates_constraints_in_a_later_file() {
        let files = lock_safe(&plan(&["alter table users add constraint n_pos check (n > 0)", "alter table posts add constraint posts_user_id_fkey foreign key (user_id) references users(id)"]), &existing(&["users", "posts"])).unwrap();
        assert_eq!(
            files,
            vec![
                vec!["alter table users add constraint n_pos check (n > 0) not valid".to_string(), "alter table posts add constraint posts_user_id_fkey foreign key (user_id) references users(id) not valid".to_string()],
                vec!["alter table users validate constraint n_pos".to_string(), "alter table posts validate constraint posts_user_id_fkey".to_string()],
            ]
        );
    }

    #[test]
    fn proves_not_null_with_a_check_first() {
        let files = lock_safe(&plan(&["alter table users alter column email set not null"]), &existing(&["users"])).unwrap();
        assert_eq!(
            files,
            vec![
                vec!["alter table users add constraint email_not_null check (email is not null) not valid".to_string()],
                vec!["alter table users validate constraint email_not_null".to_string(), "alter table users alter column email set not null".to_string(), "alter table users drop constraint email_not_null".to_string()],
            ]
        );
    }

    #[test]
    fn builds_indexes_concurrently_alone() {
        let files = lock_safe(&plan(&["alter table users add column code text", "-- the index", "create unique index users_code_key on users (code)", "alter table users add constraint users_email_key unique (email)", "comment on table users is 'x'"]), &existing(&["users"])).unwrap();
        assert_eq!(
            files,
            vec![
                vec!["alter table users add column code text".to_string()],
                vec!["-- the index".to_string(), "create unique index concurrently users_code_key on users (code)".to_string()],
                vec!["create unique index concurrently users_email_key on users (email)".to_string()],
                vec!["alter table users add constraint users_email_key unique using index users_email_key".to_string(), "comment on table users is 'x'".to_string()],
            ]
        );
    }

    #[test]
    fn repeats_session_settings_after_a_lone_file() {
        let files = lock_safe(&plan(&["set check_function_bodies = off", "create index i on users (n)", "create function f() returns int language sql as 'select 1'", "reset check_function_bodies"]), &existing(&["users"])).unwrap();
        assert_eq!(
            files,
            vec![
                vec!["create index concurrently i on users (n)".to_string()],
                vec!["set check_function_bodies = off".to_string(), "create function f() returns int language sql as 'select 1'".to_string(), "reset check_function_bodies".to_string()],
            ]
        );
    }

    #[test]
    fn keeps_what_has_no_lock_safe_shape() {
        // Deferrable, or with options the index can't carry into the constraint.
        assert_eq!(lock_safe(&plan(&["alter table users add constraint k unique (email) deferrable"]), &existing(&["users"])), None);
        assert_eq!(lock_safe(&plan(&["alter table users add constraint k unique nulls not distinct (email)"]), &existing(&["users"])), None);
        // Already not valid, or already concurrent.
        assert_eq!(lock_safe(&plan(&["alter table users add constraint c check (n > 0) not valid", "create index concurrently i on users (n)"]), &existing(&["users"])), None);
        // Qualified names match the table's qualified name.
        assert!(lock_safe(&plan(&["create index i on billing.invoices (n)"]), &existing(&["billing.invoices"])).is_some());
    }

    #[test]
    fn not_null_check_names_fit() {
        let long = "c".repeat(70);
        assert!(not_null_check(&long).len() <= 63);
        assert_eq!(not_null_check("email"), "email_not_null");
    }
}
