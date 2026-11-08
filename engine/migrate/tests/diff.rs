//! The diff engine's cases, `tests/diff/<database>/<case>/`: `v1.sql` (with `seed.sql`'s rows) is
//! the database as it is, `v2.sql` the schema it should have. Each case builds both, plans from
//! one to the other, runs the plan on the first, and requires its catalog to equal the second's
//! and every table both have to keep its own rows, value for value (a partitioned table's are its partitions'); `server.sql`, where a case has one, makes what it
//! needs of the server itself (a role). The plan is kept beside the case as `plan.sql`,
//! compared like the oracle's expected output: `REYLO_BLESS=1` writes it. Postgres's cases run
//! against the server `REYLO_TEST_PG` names, and are skipped where it is not set.

use reylo_migrate::catalog::{catalog, difference};
use reylo_migrate::db::{with_database, Db};
use reylo_migrate::diff;
use std::path::{Path, PathBuf};

fn cases(dialect: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/diff").join(dialect);
    let mut out: Vec<PathBuf> = std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
    out.sort();
    if let Ok(only) = std::env::var("REYLO_CASE") {
        out.retain(|p| p.file_name().is_some_and(|n| n.to_string_lossy().contains(&only)));
    }
    out
}

/// A table's rows, column by column, as text, sorted: `columns` are `(name before the plan, type
/// before it)`.
struct Rows {
    table: String,
    columns: Vec<(String, String)>,
    rows: Vec<Vec<String>>,
}

/// Each table's rows. A generated column is left out, whose values follow its expression, and so is
/// each `table.column` in `skip`. After the plan (`before` given), `renamed` gives a table's and a
/// column's name before it for its name after, and each column the table had before and still has
/// is read as its type was, so that a type the plan changed is held to the values it had.
async fn values(db: &mut Db, before: Option<&[Rows]>, renamed: &std::collections::BTreeMap<String, String>, skip: &[String]) -> Vec<Rows> {
    let tables = db.rows("select format('%I.%I', schemaname, tablename) as t from pg_tables where schemaname not in ('pg_catalog', 'information_schema') order by 1").await.unwrap();
    let mut out = Vec::new();
    for t in tables {
        let table = t.text("t").to_string();
        let here: Vec<(String, String)> = db
            .rows(&format!("select format('%I', attname) as c, format_type(atttypid, atttypmod) as ty from pg_attribute where attrelid = '{table}'::regclass and attnum > 0 and not attisdropped and attgenerated = '' order by attnum"))
            .await
            .unwrap()
            .iter()
            .map(|r| (r.text("c").to_string(), r.text("ty").to_string()))
            .collect();
        // Each column as (its name and type before the plan, its name now).
        let columns: Vec<((String, String), String)> = match before {
            None => here.iter().map(|c| (c.clone(), c.0.clone())).collect(),
            Some(before) => {
                let Some(was) = before.iter().find(|b| renamed.get(&b.table).unwrap_or(&b.table) == &table) else { continue };
                was.columns
                    .iter()
                    .filter_map(|(c, ty)| {
                        let now = renamed.get(&format!("{table}.{c}")).cloned().unwrap_or(c.clone());
                        here.iter().any(|(n, _)| *n == now).then(|| ((c.clone(), ty.clone()), now))
                    })
                    .collect()
            }
        };
        let columns: Vec<((String, String), String)> = columns.into_iter().filter(|((c, _), _)| !skip.contains(&format!("{table}.{c}"))).collect();
        if columns.is_empty() {
            continue;
        }
        let select = columns.iter().enumerate().map(|(k, ((_, ty), now))| format!("({now}::{ty})::text as v{k}")).collect::<Vec<_>>().join(", ");
        let mut rows: Vec<Vec<String>> = match db.rows(&format!("select {select} from only {table}")).await {
            Ok(rows) => rows.iter().map(|r| (0..columns.len()).map(|k| r.get(&format!("v{k}")).map(String::from).unwrap_or_else(|| "NULL".into())).collect()).collect(),
            Err(e) => vec![vec![format!("unreadable: {e}")]],
        };
        rows.sort();
        out.push(Rows { table, columns: columns.into_iter().map(|(c, _)| c).collect(), rows });
    }
    out
}

/// `before`'s rows read only in the columns `after` has, sorted: what the plan must have kept.
fn kept(before: &Rows, after: &Rows) -> Vec<Vec<String>> {
    let at: Vec<usize> = after.columns.iter().filter_map(|c| before.columns.iter().position(|b| b == c)).collect();
    let mut rows: Vec<Vec<String>> = before.rows.iter().map(|r| at.iter().map(|k| r[*k].clone()).collect()).collect();
    rows.sort();
    rows
}

/// What the comparator does not read: the privileges of schemas and functions, each with its
/// default where it has none of its own, and every object's comment.
async fn unread(db: &mut Db) -> Vec<String> {
    let user = r"n.nspname not in ('pg_catalog', 'information_schema') and n.nspname not like 'pg\_%'";
    let member = "exists (select 1 from pg_depend d where d.objid = {}.oid and d.deptype = 'e')";
    let sql = format!(
        "select format('schema %I %s', n.nspname, coalesce(n.nspacl, acldefault('n', n.nspowner))) as line from pg_namespace n where {user} and not {}
        union all select format('function %s %s', p.oid::regprocedure, coalesce(p.proacl, acldefault('f', p.proowner))) from pg_proc p join pg_namespace n on n.oid = p.pronamespace where {user} and not {}
        union all select format('comment on %s: %s', pg_describe_object(d.classoid, d.objoid, d.objsubid), d.description) from pg_description d
          where d.objoid >= 16384 and not exists (select 1 from pg_depend e where e.classid = d.classoid and e.objid = d.objoid and e.deptype = 'e')
        order by 1",
        member.replace("{}", "n"),
        member.replace("{}", "p")
    );
    db.rows(&sql).await.unwrap().iter().map(|r| r.text("line").to_string()).collect()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[tokio::test(flavor = "current_thread")]
async fn postgres() {
    let Ok(base) = std::env::var("REYLO_TEST_PG") else {
        eprintln!("skipped: REYLO_TEST_PG is not set");
        return;
    };
    let here = std::env::temp_dir();
    let mut admin = Db::connect(&base, &here).await.unwrap();
    let version: i64 = admin.rows("select current_setting('server_version_num') as v").await.unwrap()[0].text("v").parse().unwrap();
    let alterer = diff::postgres::PostgresAlterer::read(&mut admin, version).await.unwrap();
    let bless = std::env::var("REYLO_BLESS").is_ok();
    let mut failures = Vec::new();
    for (n, case) in cases("postgres").into_iter().enumerate() {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let (a, b) = (format!("diff_{}_{n}_a", std::process::id()), format!("diff_{}_{n}_b", std::process::id()));
        // What the case needs of the server itself, such as a role.
        admin.exec(&read(&case.join("server.sql"))).await.unwrap_or_else(|e| panic!("{name}: server.sql: {e}"));
        for db in [&a, &b] {
            admin.exec(&format!("drop database if exists {db} with (force)")).await.unwrap();
            admin.exec(&format!("create database {db}")).await.unwrap();
        }
        let mut from = Db::connect(&with_database(&base, &a), &here).await.unwrap();
        let mut to = Db::connect(&with_database(&base, &b), &here).await.unwrap();
        from.exec(&read(&case.join("v1.sql"))).await.unwrap_or_else(|e| panic!("{name}: v1.sql: {e}"));
        from.exec(&read(&case.join("seed.sql"))).await.unwrap_or_else(|e| panic!("{name}: seed.sql: {e}"));
        to.exec(&read(&case.join("v2.sql"))).await.unwrap_or_else(|e| panic!("{name}: v2.sql: {e}"));
        let counts = async |db: &mut Db| -> Vec<(String, String)> {
            let tables = db.rows("select format('%I.%I', schemaname, tablename) as t from pg_tables where schemaname not in ('pg_catalog', 'information_schema') order by 1").await.unwrap();
            let mut out = Vec::new();
            for t in tables {
                let n = db.rows(&format!("select count(*) as n from only {}", t.text("t"))).await.unwrap();
                out.push((t.text("t").to_string(), n[0].text("n").to_string()));
            }
            out
        };
        let mut before = counts(&mut from).await;
        // A column whose values the change itself rewrites (a composite type given an attribute
        // prints each value with one more field), as `table.column`, one to a line.
        let skip: Vec<String> = read(&case.join("changes.txt")).lines().filter_map(|l| l.split_whitespace().next()).map(String::from).collect();
        let rows_before = values(&mut from, None, &Default::default(), &skip).await;
        // The target's objects are created from the text v2.sql declares them with, as
        // `reylo migration create` creates them from schema.sql's.
        let mut target = diff::postgres::read(&mut to).await.unwrap();
        diff::postgres::declare(&mut to, &mut target, &read(&case.join("v2.sql"))).await.unwrap();
        // A table moved to another schema is moved first, and the plan read from there: a moved
        // table keeps its rows under its new name.
        let current = diff::postgres::read(&mut from).await.unwrap();
        let mut moves = diff::postgres::moves(&current, &target);
        diff::postgres::style_statements(&mut from, &mut moves, &target.verbatim).await.unwrap();
        for (old, new) in diff::postgres::moved(&current, &target) {
            for (t, _) in before.iter_mut().filter(|(t, _)| *t == old) {
                *t = new.clone();
            }
        }
        let mut failed = None;
        for s in &moves {
            if let Err(e) = from.exec(s).await {
                failed = Some(format!("{s}: {e}"));
            }
        }
        let mut plan = diff::plan(&diff::postgres::read(&mut from).await.unwrap(), &target, &alterer);
        // Written as `create` writes it, in schema.sql's style, and run as written.
        diff::postgres::style(&mut from, &mut plan, &target.verbatim).await.unwrap();
        for s in plan.runnable().take(if failed.is_some() { 0 } else { usize::MAX }) {
            if let Err(e) = from.exec(s).await {
                failed = Some(format!("{s}: {e}"));
                break;
            }
        }
        plan.statements.splice(0..0, moves);
        let text = plan.sql();
        let (mut only_a, mut only_b) = difference(&catalog(&mut from).await.unwrap(), &catalog(&mut to).await.unwrap());
        let (pa, pb) = (unread(&mut from).await, unread(&mut to).await);
        only_a.extend(pa.iter().filter(|l| !pb.contains(l)).cloned());
        only_b.extend(pb.iter().filter(|l| !pa.contains(l)).cloned());
        let after = counts(&mut from).await;
        let lost: Vec<String> = before.iter().filter(|(t, n)| after.iter().any(|(u, m)| u == t && m != n)).map(|(t, n)| format!("{t}: {n} rows before")).collect();
        // Each table's values, under the names the plan gave it and its columns, as their types were.
        static TABLE_RENAME: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r#"(?i)^alter table (\S+) rename to (\S+)$"#).unwrap());
        static COLUMN_RENAME: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r#"(?i)^alter table (\S+) rename column (\S+) to (\S+)$"#).unwrap());
        let mut renamed: std::collections::BTreeMap<String, String> = diff::postgres::moved(&current, &target).into_iter().collect();
        for statement in plan.statements.iter().map(|s| s.trim().trim_end_matches(';')) {
            // The plan names a table of the public schema as the search path finds it.
            let qualified = |name: &str| if name.contains('.') { name.to_string() } else { format!("public.{name}") };
            if let Some(c) = TABLE_RENAME.captures(statement) {
                let old = qualified(&c[1]);
                let schema = old.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or_default();
                renamed.insert(old, format!("{schema}.{}", &c[2]));
            } else if let Some(c) = COLUMN_RENAME.captures(statement) {
                let old = qualified(&c[1]);
                let table = renamed.get(&old).cloned().unwrap_or(old);
                renamed.insert(format!("{table}.{}", &c[2]), c[3].to_string());
            }
        }
        let rows_after = values(&mut from, Some(&rows_before), &renamed, &skip).await;
        let changed: Vec<String> = rows_before
            .iter()
            .filter_map(|b| {
                let now = renamed.get(&b.table).unwrap_or(&b.table);
                let a = rows_after.iter().find(|a| &a.table == now)?;
                let was = kept(b, a);
                (was != a.rows).then(|| format!("{now}: {was:?} before, {:?} after", a.rows))
            })
            .collect();
        let expected = case.join("plan.sql");
        if bless {
            std::fs::write(&expected, &text).unwrap();
        }
        let mut problems = Vec::new();
        if let Some(f) = failed {
            problems.push(format!("the plan failed at {f}"));
        }
        if !only_a.is_empty() || !only_b.is_empty() {
            problems.push(format!("after the plan the database differs from v2:\n{}", only_a.iter().map(|l| format!("  - {l}")).chain(only_b.iter().map(|l| format!("  + {l}"))).collect::<Vec<_>>().join("\n")));
        }
        if !lost.is_empty() {
            problems.push(format!("rows lost: {}", lost.join(", ")));
        }
        if !changed.is_empty() {
            problems.push(format!("values changed: {}", changed.join("; ")));
        }
        if !bless && read(&expected) != text {
            problems.push(format!("the plan is not plan.sql's (REYLO_BLESS=1 writes it):\n{text}"));
        }
        if !problems.is_empty() {
            failures.push(format!("{name}:\n{}\nplan:\n{text}", problems.join("\n")));
        }
        from.close().await;
        to.close().await;
        for db in [&a, &b] {
            admin.exec(&format!("drop database if exists {db} with (force)")).await.unwrap();
        }
    }
    admin.close().await;
    assert!(failures.is_empty(), "{} cases failed:\n\n{}", failures.len(), failures.join("\n\n"));
}

/// Every Postgres schema the oracle's cases set up (`oracle/cases/<case>/setup.sql`), built from an
/// empty database by its plan, and dropped back to an empty one by the plan the other way: each
/// direction must end with the other side's catalog.
#[tokio::test(flavor = "current_thread")]
async fn postgres_oracle_schemas() {
    let Ok(base) = std::env::var("REYLO_TEST_PG") else {
        eprintln!("skipped: REYLO_TEST_PG is not set");
        return;
    };
    let here = std::env::temp_dir();
    let mut admin = Db::connect(&base, &here).await.unwrap();
    let version: i64 = admin.rows("select current_setting('server_version_num') as v").await.unwrap()[0].text("v").parse().unwrap();
    let alterer = diff::postgres::PostgresAlterer::read(&mut admin, version).await.unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/cases");
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("setup.sql").exists() && !p.join("dialect").exists()).collect();
    cases.sort();
    if let Ok(only) = std::env::var("REYLO_CASE") {
        cases.retain(|p| p.file_name().is_some_and(|n| n.to_string_lossy().contains(&only)));
    }
    let (mut failures, mut skipped, mut passed) = (Vec::new(), Vec::new(), 0);
    for (n, case) in cases.iter().enumerate() {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let (a, b, e) = (format!("odiff_{}_{n}_a", std::process::id()), format!("odiff_{}_{n}_b", std::process::id()), format!("odiff_{}_{n}_e", std::process::id()));
        for db in [&a, &b, &e] {
            admin.exec(&format!("drop database if exists {db} with (force)")).await.unwrap();
            admin.exec(&format!("create database {db}")).await.unwrap();
        }
        let mut empty = Db::connect(&with_database(&base, &a), &here).await.unwrap();
        let mut full = Db::connect(&with_database(&base, &b), &here).await.unwrap();
        let mut blank = Db::connect(&with_database(&base, &e), &here).await.unwrap();
        if let Err(err) = full.exec(&read(&case.join("setup.sql"))).await {
            skipped.push(format!("{name}: {err}"));
        } else {
            let mut problems = Vec::new();
            for direction in ["from empty", "to empty"] {
                let (from, to) = if direction == "from empty" { (&mut empty, &mut full) } else { (&mut full, &mut blank) };
                let label = direction;
                let mut target = diff::postgres::read(to).await.unwrap();
                if direction == "from empty" {
                    diff::postgres::declare(to, &mut target, &read(&case.join("setup.sql"))).await.unwrap();
                }
                let mut plan = diff::plan(&diff::postgres::read(from).await.unwrap(), &target, &alterer);
                diff::postgres::style(from, &mut plan, &target.verbatim).await.unwrap();
                // One case's plans are shown, to be read.
                if std::env::var("REYLO_CASE").is_ok() {
                    eprintln!("{name}, {direction}:\n{}", plan.sql());
                }
                let mut failed = None;
                for s in plan.runnable() {
                    if let Err(err) = from.exec(s).await {
                        failed = Some(format!("{s}: {err}"));
                        break;
                    }
                }
                let (mut only_a, mut only_b) = difference(&catalog(from).await.unwrap(), &catalog(to).await.unwrap());
                let (pa, pb) = (unread(from).await, unread(to).await);
                only_a.extend(pa.iter().filter(|l| !pb.contains(l)).cloned());
                only_b.extend(pb.iter().filter(|l| !pa.contains(l)).cloned());
                if failed.is_some() || !only_a.is_empty() || !only_b.is_empty() {
                    let mut lines: Vec<String> = failed.into_iter().map(|f| format!("  failed at {f}")).collect();
                    lines.extend(only_a.iter().take(8).map(|l| format!("  - {l}")));
                    lines.extend(only_b.iter().take(8).map(|l| format!("  + {l}")));
                    problems.push(format!("{label}:\n{}", lines.join("\n")));
                }
            }
            if problems.is_empty() {
                passed += 1;
            } else {
                failures.push(format!("{name}: {}", problems.join("\n")));
            }
        }
        for db in [empty, full, blank] {
            db.close().await;
        }
        for db in [&a, &b, &e] {
            admin.exec(&format!("drop database if exists {db} with (force)")).await.unwrap();
        }
    }
    admin.close().await;
    eprintln!("{passed} schemas planned both ways; {} could not be set up here:\n{}", skipped.len(), skipped.join("\n"));
    assert!(failures.is_empty(), "{} of {} schemas failed:\n\n{}", failures.len(), cases.len(), failures.join("\n\n"));
}
