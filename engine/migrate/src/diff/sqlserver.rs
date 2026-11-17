//! SQL Server's schema as the diff engine's objects, read from its catalog views (`sys.tables`,
//! `sys.columns`, `sys.key_constraints`, `sys.sql_modules`, …) and written back as T-SQL. Names
//! are bare where SQL Server reads them bare, else in brackets, and those in the schema the
//! account works in (`dbo`, usually) are unqualified, as a person writes them.
//!
//! SQL Server names a constraint written without a name itself, from its table and a random
//! number (`PK__users__3213E83F0A1B2C3D`), so two databases made from one file name theirs
//! differently. Such a constraint is known by what it holds (`primary key users`, `unique
//! users(email)`) and its name is left out of its definition; a plan creates it unnamed again and
//! drops it by looking its name up as the statement runs. A column's default is part of the
//! column, as a person writes it, so that a column added to a table with rows is added with it.
//!
//! A view, a procedure, a function or a trigger is its text, as `sys.sql_modules` keeps it, and
//! changes with `create or alter`. Its `create` must open a batch, so a plan puts a `GO` line
//! before and after it. The dependencies are the server's records (`sys.sql_expression_dependencies`,
//! the foreign keys and the indexes) and the columns a check or a computed column names.

use super::{Alterer, Change, Kind, Object, Schema, TypeClass, GO};
use crate::catalog::HISTORY_TABLE;
use crate::db::{Db, Row};
use crate::sql::{tokenize, TokenKind};
use crate::{Dialect, Error};
use std::collections::{BTreeMap, BTreeSet};

/// T-SQL's reserved keywords, which a name keeps in brackets, with `GO`, which a line of its own ends a batch at.
const RESERVED: &[&str] = &[
    "ADD", "ALL", "ALTER", "AND", "ANY", "AS", "ASC", "AUTHORIZATION", "BACKUP", "BEGIN", "BETWEEN", "BREAK", "BROWSE", "BULK", "BY", "CASCADE", "CASE", "CHECK", "CHECKPOINT", "CLOSE", "CLUSTERED", "COALESCE", "COLLATE", "COLUMN", "COMMIT",
    "COMPUTE", "CONSTRAINT", "CONTAINS", "CONTAINSTABLE", "CONTINUE", "CONVERT", "CREATE", "CROSS", "CURRENT", "CURRENT_DATE", "CURRENT_TIME", "CURRENT_TIMESTAMP", "CURRENT_USER", "CURSOR", "DATABASE", "DBCC", "DEALLOCATE", "DECLARE",
    "DEFAULT", "DELETE", "DENY", "DESC", "DISK", "DISTINCT", "DISTRIBUTED", "DOUBLE", "DROP", "DUMP", "ELSE", "END", "ERRLVL", "ESCAPE", "EXCEPT", "EXEC", "EXECUTE", "EXISTS", "EXIT", "EXTERNAL", "FETCH", "FILE", "FILLFACTOR", "FOR",
    "FOREIGN", "FREETEXT", "FREETEXTTABLE", "FROM", "FULL", "FUNCTION", "GO", "GOTO", "GRANT", "GROUP", "HAVING", "HOLDLOCK", "IDENTITY", "IDENTITY_INSERT", "IDENTITYCOL", "IF", "IN", "INDEX", "INNER", "INSERT", "INTERSECT", "INTO",
    "IS", "JOIN", "KEY", "KILL", "LEFT", "LIKE", "LINENO", "LOAD", "MERGE", "NATIONAL", "NOCHECK", "NONCLUSTERED", "NOT", "NULL", "NULLIF", "OF", "OFF", "OFFSETS", "ON", "OPEN", "OPENDATASOURCE", "OPENQUERY", "OPENROWSET", "OPENXML",
    "OPTION", "OR", "ORDER", "OUTER", "OVER", "PERCENT", "PIVOT", "PLAN", "PRECISION", "PRIMARY", "PRINT", "PROC", "PROCEDURE", "PUBLIC", "RAISERROR", "READ", "READTEXT", "RECONFIGURE", "REFERENCES", "REPLICATION", "RESTORE",
    "RESTRICT", "RETURN", "REVERT", "REVOKE", "RIGHT", "ROLLBACK", "ROWCOUNT", "ROWGUIDCOL", "RULE", "SAVE", "SCHEMA", "SECURITYAUDIT", "SELECT", "SEMANTICKEYPHRASETABLE", "SEMANTICSIMILARITYDETAILSTABLE", "SEMANTICSIMILARITYTABLE",
    "SESSION_USER", "SET", "SETUSER", "SHUTDOWN", "SOME", "STATISTICS", "SYSTEM_USER", "TABLE", "TABLESAMPLE", "TEXTSIZE", "THEN", "TO", "TOP", "TRAN", "TRANSACTION", "TRIGGER", "TRUNCATE", "TRY_CONVERT", "TSEQUAL", "UNION", "UNIQUE",
    "UNPIVOT", "UPDATE", "UPDATETEXT", "USE", "USER", "VALUES", "VARYING", "VIEW", "WAITFOR", "WHEN", "WHERE", "WHILE", "WITH", "WITHIN", "WRITETEXT",
];

/// A name as a statement writes it: bare where T-SQL reads it bare, else in brackets.
pub fn ident(name: &str) -> String {
    let plain = name.bytes().next().is_some_and(|b| b.is_ascii_alphabetic() || b == b'_') && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if plain && !RESERVED.contains(&name.to_ascii_uppercase().as_str()) {
        name.to_string()
    } else {
        format!("[{}]", name.replace(']', "]]"))
    }
}

/// A Unicode string literal.
pub fn literal(text: &str) -> String {
    format!("N'{}'", text.replace('\'', "''"))
}

/// The statement that renames the table `from` (as a statement writes it) to `new`, a bare name:
/// `sp_rename` takes the new name as it is, so it is never quoted.
pub fn rename_object(from: &str, new: &str) -> String {
    format!("exec sp_rename {}, {}", literal(from), literal(new))
}

/// What the reader knows of the database: the schema the account works in, whose names go
/// unqualified, and the collation a column takes when it names none.
struct Ctx {
    schema: String,
    collation: String,
}

impl Ctx {
    fn name(&self, schema: &str, name: &str) -> String {
        if schema == self.schema {
            ident(name)
        } else {
            format!("{}.{}", ident(schema), ident(name))
        }
    }
}

/// A column's type, from its row of `sys.columns` (or `sys.types`): the length of a string in
/// characters, `max` where it has none, a decimal's precision and scale, a time's scale.
fn type_text(ctx: &Ctx, r: &Row) -> String {
    let ty = r.text("type");
    if r.flag("user_defined") {
        return ctx.name(r.text("type_schema"), ty);
    }
    let max: i64 = r.text("max_length").parse().unwrap_or(0);
    match ty {
        "char" | "varchar" | "binary" | "varbinary" => format!("{ty}({})", if max == -1 { "max".to_string() } else { max.to_string() }),
        "nchar" | "nvarchar" => format!("{ty}({})", if max == -1 { "max".to_string() } else { (max / 2).to_string() }),
        "decimal" | "numeric" => format!("{ty}({}, {})", r.text("precision"), r.text("scale")),
        "datetime2" | "time" | "datetimeoffset" => format!("{ty}({})", r.text("scale")),
        _ => ty.to_string(),
    }
}

/// The parts of a column an alterer reads, kept in its `extra` as `key=value` lines.
fn part<'a>(o: &'a Object, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    o.extra.iter().find_map(|l| l.strip_prefix(prefix.as_str()))
}

/// The columns of `table` (by object id) a text names in brackets, as SQL Server writes a check's
/// or a computed column's definition (`([total]>(0))`), by their ids.
fn named_columns(text: &str, columns: &BTreeMap<String, String>) -> BTreeSet<String> {
    tokenize(text, Dialect::Sqlserver)
        .iter()
        .filter(|t| matches!(t.kind, TokenKind::Quoted | TokenKind::Word))
        .filter_map(|t| {
            let name = if t.kind == TokenKind::Quoted { t.text[1..t.text.len() - 1].replace("]]", "]").replace("\"\"", "\"") } else { t.text.clone() };
            columns.iter().find(|(n, _)| n.eq_ignore_ascii_case(&name)).map(|(_, id)| id.clone())
        })
        .collect()
}

/// A statement that drops a constraint SQL Server named itself, by looking its name up with
/// `lookup` (a query of its bracketed name) as it runs. The variable is named for the constraint,
/// so that several in one batch don't collide.
fn drop_found(table: &str, lookup: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in table.bytes().chain(lookup.bytes()) {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let var = format!("@reylo_{:012x}", hash & 0xffff_ffff_ffff);
    format!("declare {var} nvarchar(max) = (select top 1 {lookup});\nexec (N'alter table {} drop constraint ' + {var})", table.replace('\'', "''"))
}

/// A module's text from its first word on, the words that open it (`CREATE OR ALTER VIEW`,
/// `alter  view`) written `create view`, as every database holding it compares it.
fn module_text(definition: &str) -> String {
    let tokens = tokenize(definition, Dialect::Sqlserver);
    let Some(first) = tokens.first() else { return definition.trim().to_string() };
    let words: Vec<String> = tokens.iter().take(4).map(|t| t.upper()).collect();
    let at = match words.as_slice() {
        [c, o, a, ..] if c == "CREATE" && o == "OR" && a == "ALTER" => 3,
        [c, ..] if c == "CREATE" || c == "ALTER" => 1,
        _ => return definition[first.start..].trim_end().to_string(),
    };
    let Some(kind) = tokens.get(at) else { return definition[first.start..].trim_end().to_string() };
    let kind = match kind.upper().as_str() {
        "PROC" => "procedure".to_string(),
        other => other.to_ascii_lowercase(),
    };
    format!("create {kind}{}", definition[tokens[at].end..].trim_end().trim_end_matches(';').trim_end())
}

/// `create` of a module written `create or alter`, which changes it in place.
fn create_or_alter(create: &str) -> String {
    match create.strip_prefix("create ") {
        Some(rest) => format!("create or alter {rest}"),
        None => create.to_string(),
    }
}

/// Whether a statement must open its batch: a module's `create`, or `create schema`.
fn opens_batch(statement: &str) -> bool {
    let words: Vec<String> = tokenize(statement, Dialect::Sqlserver).iter().take(4).map(|t| t.upper()).collect();
    let at = match words.as_slice() {
        [c, o, a, ..] if c == "CREATE" && o == "OR" && a == "ALTER" => 3,
        [c, ..] if c == "CREATE" || c == "ALTER" => 1,
        _ => return false,
    };
    matches!(words.get(at).map(String::as_str), Some("VIEW" | "PROCEDURE" | "PROC" | "FUNCTION" | "TRIGGER")) || (words[0] == "CREATE" && words.get(1).is_some_and(|w| w == "SCHEMA"))
}

/// A key's or an index's columns, each its name and whether it is descending, then those it includes.
#[derive(Default, Clone)]
struct IndexColumns {
    keys: Vec<(String, bool)>,
    included: Vec<String>,
}

impl IndexColumns {
    fn keys_text(&self) -> String {
        self.keys.iter().map(|(n, desc)| format!("{}{}", ident(n), if *desc { " desc" } else { "" })).collect::<Vec<_>>().join(", ")
    }

    fn names(&self) -> String {
        self.keys.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(",")
    }
}

/// A table or a table type: its name, its id, and its columns by name with their ids.
struct Owner {
    name: String,
    id: String,
    columns: BTreeMap<String, String>,
    /// Whether it is a table type, whose parts are its definition and have no statements of their own.
    table_type: bool,
}

/// The schema of the database `db` is connected to.
pub async fn read(db: &mut Db) -> Result<Schema, Error> {
    let mut s = Schema::default();
    let head = db.rows("select schema_name() as s, convert(nvarchar(128), databasepropertyex(db_name(), 'Collation')) as c").await?;
    let ctx = Ctx { schema: head.first().map(|r| r.text("s").to_string()).unwrap_or_else(|| "dbo".into()), collation: head.first().map(|r| r.text("c").to_string()).unwrap_or_default() };
    let history = HISTORY_TABLE;

    // Schemas: not dbo, guest, INFORMATION_SCHEMA or sys, nor the fixed roles'.
    for r in db
        .rows(
            "select name from sys.schemas where schema_id > 4 and name not in ('db_owner', 'db_accessadmin', 'db_securityadmin', 'db_ddladmin', 'db_backupoperator', 'db_datareader', 'db_datawriter', 'db_denydatareader', 'db_denydatawriter')",
        )
        .await?
    {
        let name = ident(r.text("name"));
        s.add(Object { id: format!("schema {name}"), kind: Some(Kind::Schema), create: vec![format!("create schema {name}")], drop: vec![format!("drop schema {name}")], ..Object::default() });
    }
    let schema_dep = |schema: &str| -> BTreeSet<String> { if schema == ctx.schema || schema == "dbo" { BTreeSet::new() } else { [format!("schema {}", ident(schema))].into() } };

    // Every object an id is known for, by its object id, and a column's by its object's and its own.
    let mut objects: BTreeMap<String, String> = BTreeMap::new();
    let mut columns_of: BTreeMap<(String, String), String> = BTreeMap::new();

    // Types: aliases of a system type, and table types.
    let mut types: BTreeMap<String, String> = BTreeMap::new();
    let mut table_types: BTreeMap<String, (String, String, String)> = BTreeMap::new();
    for r in db
        .rows(
            "select t.user_type_id as tid, schema_name(t.schema_id) as s, t.name, type_name(t.system_type_id) as type, t.max_length, t.precision, t.scale, t.is_nullable as nullable, t.is_table_type as is_table,
              (select convert(nvarchar(20), tt.type_table_object_id) from sys.table_types tt where tt.user_type_id = t.user_type_id) as table_object
            from sys.types t where t.is_user_defined = 1",
        )
        .await?
    {
        let name = ctx.name(r.text("s"), r.text("name"));
        let id = format!("type {name}");
        types.insert(r.text("tid").to_string(), id.clone());
        if r.flag("is_table") {
            table_types.insert(r.text("table_object").to_string(), (name, id.clone(), r.text("s").to_string()));
            objects.insert(r.text("table_object").to_string(), id);
            continue;
        }
        let def = format!("from {}{}", type_text(&ctx, &r), if r.flag("nullable") { "" } else { " not null" });
        s.add(Object { id, kind: Some(Kind::Type), def: def.clone(), create: vec![format!("create type {name} {def}")], drop: vec![format!("drop type {name}")], deps: schema_dep(r.text("s")), ..Object::default() });
    }

    // Tables, by object id.
    let mut owners: BTreeMap<String, Owner> = BTreeMap::new();
    for r in db.rows(&format!("select convert(nvarchar(20), object_id) as oid, schema_name(schema_id) as s, name from sys.tables where is_ms_shipped = 0 and name <> N'{history}'")).await? {
        let name = ctx.name(r.text("s"), r.text("name"));
        let id = format!("table {name}");
        objects.insert(r.text("oid").to_string(), id.clone());
        s.add(Object { id: id.clone(), kind: Some(Kind::Table), deps: schema_dep(r.text("s")), ..Object::default() });
        owners.insert(r.text("oid").to_string(), Owner { name, id, columns: BTreeMap::new(), table_type: false });
    }
    for (oid, (name, id, _)) in &table_types {
        owners.insert(oid.clone(), Owner { name: name.clone(), id: id.clone(), columns: BTreeMap::new(), table_type: true });
    }

    // Columns, with their defaults.
    let mut lines: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut column_deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let rows = db
        .rows(
            "select convert(nvarchar(20), c.object_id) as oid, c.column_id as cid, c.name, type_name(c.user_type_id) as type, schema_name(ty.schema_id) as type_schema, ty.is_user_defined as user_defined,
              convert(nvarchar(20), c.user_type_id) as tid, c.max_length, c.precision, c.scale, c.is_nullable as nullable, c.collation_name as collation, c.is_sparse as sparse, c.is_rowguidcol as is_rowguid,
              c.is_identity as is_identity, convert(nvarchar(40), ic.seed_value) as seed, convert(nvarchar(40), ic.increment_value) as increment,
              cc.definition as computed, cc.is_persisted as persisted, d.name as default_name, d.definition as default_def, d.is_system_named as default_system, convert(nvarchar(20), d.object_id) as default_oid
            from sys.columns c join sys.objects o on o.object_id = c.object_id join sys.types ty on ty.user_type_id = c.user_type_id
            left join sys.identity_columns ic on ic.object_id = c.object_id and ic.column_id = c.column_id
            left join sys.computed_columns cc on cc.object_id = c.object_id and cc.column_id = c.column_id
            left join sys.default_constraints d on d.parent_object_id = c.object_id and d.parent_column_id = c.column_id
            where (o.type = 'U' and o.is_ms_shipped = 0) or o.type = 'TT'
            order by c.object_id, c.column_id",
        )
        .await?;
    let mut column_rows: Vec<(String, Row)> = Vec::new();
    for r in rows {
        let oid = r.text("oid").to_string();
        let Some(owner) = owners.get_mut(&oid) else { continue };
        let col_id = format!("column {}.{}", owner.name, ident(r.text("name")));
        owner.columns.insert(r.text("name").to_string(), col_id.clone());
        columns_of.insert((oid.clone(), r.text("cid").to_string()), col_id.clone());
        if let Some(d) = r.get("default_oid") {
            objects.insert(d.to_string(), col_id.clone());
        }
        column_rows.push((oid, r));
    }
    for (oid, r) in &column_rows {
        let owner = &owners[oid];
        let col = ident(r.text("name"));
        let col_id = owner.columns[r.text("name")].clone();
        let nullable = if r.flag("nullable") { "null" } else { "not null" };
        let mut deps: BTreeSet<String> = BTreeSet::new();
        // The column's whole text but its default, its type (with its collation) or its computed
        // expression, and what `alter column` can't change.
        let (head, ty, special) = if let Some(expr) = r.get("computed") {
            deps.extend(named_columns(expr, &owner.columns).into_iter().filter(|c| *c != col_id));
            let persisted = r.flag("persisted");
            let text = format!("as {expr}{}{}", if persisted { " persisted" } else { "" }, if persisted && !r.flag("nullable") { " not null" } else { "" });
            (text.clone(), text.clone(), text)
        } else {
            let mut ty = type_text(&ctx, r);
            if r.flag("user_defined") {
                if let Some(t) = types.get(r.text("tid")) {
                    deps.insert(t.clone());
                }
            }
            if let Some(c) = r.get("collation").filter(|c| *c != ctx.collation && !r.flag("user_defined")) {
                ty = format!("{ty} collate {c}");
            }
            let mut special = Vec::new();
            if r.flag("sparse") {
                special.push("sparse".to_string());
            }
            if r.flag("is_identity") {
                special.push(format!("identity({}, {})", r.text("seed"), r.text("increment")));
            }
            if r.flag("is_rowguid") {
                special.push("rowguidcol".to_string());
            }
            let special = special.join(" ");
            (format!("{ty}{}{special} {nullable}", if special.is_empty() { "" } else { " " }), ty, special)
        };
        let default = r.get("default_def").map(|d| (if r.flag("default_system") { String::new() } else { r.text("default_name").to_string() }, d.to_string()));
        let default_text = default.as_ref().map(|(n, d)| format!("{}default {d}", if n.is_empty() { String::new() } else { format!("constraint {} ", ident(n)) })).unwrap_or_default();
        let def = if default_text.is_empty() { head.clone() } else { format!("{head} {default_text}") };
        lines.entry(oid.clone()).or_default().push(format!("{col} {def}"));
        column_deps.insert(col_id.clone(), deps.clone());
        if owner.table_type {
            continue;
        }
        let table = &owner.name;
        let computed = r.get("computed").is_some();
        let caution = (!r.flag("nullable") && default.is_none() && !r.flag("is_identity") && !computed)
            .then(|| format!("{} is not null with no default, so adding it fails on a table that has rows: give it a default, or add it nullable, fill it, then make it not null", r.text("name")));
        let mut extra = vec![format!("table={table}"), format!("column={}", r.text("name")), format!("head={ty}"), format!("special={special}")];
        extra.push(format!("nullable={}", r.flag("nullable")));
        if let Some((name, expr)) = &default {
            extra.push(format!("default={expr}"));
            extra.push(format!("default_name={name}"));
        }
        s.add(Object {
            caution,
            id: col_id,
            kind: Some(Kind::Column),
            owner: Some(owner.id.clone()),
            def: def.clone(),
            create: vec![format!("alter table {table} add {col} {def}")],
            // SQL Server refuses to drop a column while its default stands.
            drop: default.as_ref().map(|(name, _)| drop_default(table, r.text("name"), name)).into_iter().chain([format!("alter table {table} drop column {col}")]).collect(),
            inline: true,
            deps,
            change: Change::Alter { blocks: true },
            extra,
            ..Object::default()
        });
    }

    // The columns of keys and indexes.
    let mut index_columns: BTreeMap<(String, String), IndexColumns> = BTreeMap::new();
    for r in db
        .rows(
            "select convert(nvarchar(20), ic.object_id) as oid, convert(nvarchar(20), ic.index_id) as iid, c.name, ic.is_descending_key as descending, ic.is_included_column as included, ic.key_ordinal as ord
            from sys.index_columns ic join sys.columns c on c.object_id = ic.object_id and c.column_id = ic.column_id join sys.objects o on o.object_id = ic.object_id
            where o.is_ms_shipped = 0 or o.type = 'TT' order by ic.object_id, ic.index_id, ic.key_ordinal, ic.index_column_id",
        )
        .await?
    {
        let entry = index_columns.entry((r.text("oid").to_string(), r.text("iid").to_string())).or_default();
        if r.flag("included") || r.text("ord") == "0" {
            entry.included.push(r.text("name").to_string());
        } else {
            entry.keys.push((r.text("name").to_string(), r.flag("descending")));
        }
    }
    let deps_on = |owner: &Owner, names: &[String]| -> BTreeSet<String> { names.iter().filter_map(|n| owner.columns.get(n).cloned()).collect() };

    // Views: indexes on them hang off them, as on tables.
    let mut view_names: BTreeMap<String, (String, String)> = BTreeMap::new();
    for r in db.rows("select convert(nvarchar(20), object_id) as oid, schema_name(schema_id) as s, name from sys.views where is_ms_shipped = 0").await? {
        let name = ctx.name(r.text("s"), r.text("name"));
        view_names.insert(r.text("oid").to_string(), (name.clone(), format!("view {name}")));
        objects.insert(r.text("oid").to_string(), format!("view {name}"));
    }

    // Primary keys and unique constraints, written inside their table's create.
    let mut index_ids: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut inline_constraints: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let indexes = db
        .rows(
            "select convert(nvarchar(20), i.object_id) as oid, convert(nvarchar(20), i.index_id) as iid, i.name, i.type, i.is_unique as is_unique, i.is_primary_key as pk, i.is_unique_constraint as uq,
              i.filter_definition as filter, i.ignore_dup_key as ignore_dup, convert(nvarchar(20), k.object_id) as kid, k.is_system_named as system
            from sys.indexes i join sys.objects o on o.object_id = i.object_id
            left join sys.key_constraints k on k.parent_object_id = i.object_id and k.unique_index_id = i.index_id
            where ((o.is_ms_shipped = 0 and o.type in ('U', 'V')) or o.type = 'TT') and i.type > 0 and i.is_hypothetical = 0",
        )
        .await?;
    for r in &indexes {
        let oid = r.text("oid").to_string();
        let iid = r.text("iid").to_string();
        let cols = index_columns.get(&(oid.clone(), iid.clone())).cloned().unwrap_or_default();
        let clustered = match r.text("type") {
            "1" => "clustered",
            "2" => "nonclustered",
            _ => "",
        };
        let (owner_name, owner_id, owner) = if let Some(o) = owners.get(&oid) {
            (o.name.clone(), o.id.clone(), Some(o))
        } else if let Some((n, id)) = view_names.get(&oid) {
            (n.clone(), id.clone(), None)
        } else {
            continue;
        };
        let names: Vec<String> = cols.keys.iter().map(|(n, _)| n.clone()).chain(cols.included.iter().cloned()).collect();
        let deps = owner.map(|o| deps_on(o, &names)).unwrap_or_default();
        if r.flag("pk") || r.flag("uq") {
            let Some(owner) = owner else { continue };
            let word = if r.flag("pk") { "primary key" } else { "unique" };
            let system = r.flag("system");
            let body = format!("{word} {clustered} ({})", cols.keys_text());
            let def = if system { body.clone() } else { format!("constraint {} {body}", ident(r.text("name"))) };
            let id = match (system, r.flag("pk")) {
                (true, true) => format!("primary key {owner_name}"),
                (true, false) => format!("unique {owner_name}({})", cols.names()),
                (false, _) => format!("constraint {owner_name}.{}", ident(r.text("name"))),
            };
            index_ids.insert((oid.clone(), iid.clone()), id.clone());
            objects.insert(r.text("kid").to_string(), id.clone());
            inline_constraints.entry(oid.clone()).or_default().push(def.clone());
            if owner.table_type {
                continue;
            }
            let drop = if system {
                let lookup = if r.flag("pk") {
                    format!("quotename(name) from sys.key_constraints where parent_object_id = object_id({}) and type = 'PK'", literal(&owner_name))
                } else {
                    format!(
                        "quotename(k.name) from sys.key_constraints k where k.parent_object_id = object_id({}) and k.type = 'UQ' and (select string_agg(c.name, N',') within group (order by ic.key_ordinal) from sys.index_columns ic join sys.columns c on c.object_id = ic.object_id and c.column_id = ic.column_id where ic.object_id = k.parent_object_id and ic.index_id = k.unique_index_id and ic.key_ordinal > 0) = {}",
                        literal(&owner_name),
                        literal(&cols.names())
                    )
                };
                drop_found(&owner_name, &lookup)
            } else {
                format!("alter table {owner_name} drop constraint {}", ident(r.text("name")))
            };
            s.add(Object { id, kind: Some(Kind::Constraint), owner: Some(owner_id), def: def.clone(), create: vec![format!("alter table {owner_name} add {def}")], drop: vec![drop], inline: true, deps, ..Object::default() });
            continue;
        }
        let name = ident(r.text("name"));
        let unique = if r.flag("is_unique") { "unique " } else { "" };
        let mut def = match r.text("type") {
            "5" => format!("create clustered columnstore index {name} on {owner_name}"),
            "6" => format!("create nonclustered columnstore index {name} on {owner_name} ({})", cols.included.iter().chain(cols.keys.iter().map(|(n, _)| n)).map(|n| ident(n)).collect::<Vec<_>>().join(", ")),
            "1" | "2" => format!("create {unique}{clustered} index {name} on {owner_name} ({})", cols.keys_text()),
            // An XML, spatial or other index: not compared.
            _ => continue,
        };
        if matches!(r.text("type"), "1" | "2") && !cols.included.is_empty() {
            def += &format!(" include ({})", cols.included.iter().map(|n| ident(n)).collect::<Vec<_>>().join(", "));
        }
        if let Some(filter) = r.get("filter") {
            def += &format!(" where {filter}");
        }
        if r.flag("ignore_dup") {
            def += " with (ignore_dup_key = on)";
        }
        let id = format!("index {owner_name}.{name}");
        index_ids.insert((oid.clone(), iid.clone()), id.clone());
        if owners.get(&oid).is_some_and(|o| o.table_type) {
            inline_constraints.entry(oid.clone()).or_default().push(format!("index {name} {} ({})", clustered, cols.keys_text()));
            continue;
        }
        s.add(Object { id, kind: Some(Kind::Index), owner: Some(owner_id), def: def.clone(), create: vec![def], drop: vec![format!("drop index {name} on {owner_name}")], deps, ..Object::default() });
    }

    // Check constraints, written inside their table's create.
    for r in db
        .rows("select convert(nvarchar(20), object_id) as kid, convert(nvarchar(20), parent_object_id) as oid, name, definition, is_system_named as system from sys.check_constraints where is_ms_shipped = 0 or objectproperty(parent_object_id, 'IsTableType') = 1")
        .await?
    {
        let oid = r.text("oid").to_string();
        let Some(owner) = owners.get(&oid) else { continue };
        let system = r.flag("system");
        let body = format!("check {}", r.text("definition"));
        let def = if system { body.clone() } else { format!("constraint {} {body}", ident(r.text("name"))) };
        let id = if system { format!("check {} {}", owner.name, r.text("definition")) } else { format!("constraint {}.{}", owner.name, ident(r.text("name"))) };
        objects.insert(r.text("kid").to_string(), id.clone());
        inline_constraints.entry(oid.clone()).or_default().push(def.clone());
        if owner.table_type {
            continue;
        }
        let drop = if system {
            drop_found(&owner.name, &format!("quotename(name) from sys.check_constraints where parent_object_id = object_id({}) and definition = {}", literal(&owner.name), literal(r.text("definition"))))
        } else {
            format!("alter table {} drop constraint {}", owner.name, ident(r.text("name")))
        };
        s.add(Object {
            id,
            kind: Some(Kind::Constraint),
            owner: Some(owner.id.clone()),
            def: def.clone(),
            create: vec![format!("alter table {} add {def}", owner.name)],
            drop: vec![drop],
            inline: true,
            deps: named_columns(r.text("definition"), &owner.columns),
            ..Object::default()
        });
    }

    // Foreign keys: statements of their own, after both tables are as they need them.
    let mut fk_columns: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();
    for r in db
        .rows(
            "select convert(nvarchar(20), fc.constraint_object_id) as fid, pc.name as col, rc.name as ref from sys.foreign_key_columns fc
            join sys.columns pc on pc.object_id = fc.parent_object_id and pc.column_id = fc.parent_column_id
            join sys.columns rc on rc.object_id = fc.referenced_object_id and rc.column_id = fc.referenced_column_id
            order by fc.constraint_object_id, fc.constraint_column_id",
        )
        .await?
    {
        let entry = fk_columns.entry(r.text("fid").to_string()).or_default();
        entry.0.push(r.text("col").to_string());
        entry.1.push(r.text("ref").to_string());
    }
    for r in db
        .rows(
            "select convert(nvarchar(20), object_id) as fid, convert(nvarchar(20), parent_object_id) as oid, convert(nvarchar(20), referenced_object_id) as rid, convert(nvarchar(20), key_index_id) as kiid,
              name, is_system_named as system, delete_referential_action as on_delete, update_referential_action as on_update
            from sys.foreign_keys where is_ms_shipped = 0",
        )
        .await?
    {
        let (Some(owner), Some(target)) = (owners.get(r.text("oid")), owners.get(r.text("rid"))) else { continue };
        let (cols, refs) = fk_columns.get(r.text("fid")).cloned().unwrap_or_default();
        let action = |code: &str| match code {
            "1" => "cascade",
            "2" => "set null",
            "3" => "set default",
            _ => "",
        };
        let list = |names: &[String]| names.iter().map(|n| ident(n)).collect::<Vec<_>>().join(", ");
        let mut body = format!("foreign key ({}) references {} ({})", list(&cols), target.name, list(&refs));
        for (word, code) in [("delete", r.text("on_delete")), ("update", r.text("on_update"))] {
            if !action(code).is_empty() {
                body += &format!(" on {word} {}", action(code));
            }
        }
        let system = r.flag("system");
        let def = if system { body.clone() } else { format!("constraint {} {body}", ident(r.text("name"))) };
        let id = if system { format!("foreign key {}({}) references {}", owner.name, cols.join(","), target.name) } else { format!("constraint {}.{}", owner.name, ident(r.text("name"))) };
        objects.insert(r.text("fid").to_string(), id.clone());
        let mut deps = deps_on(owner, &cols);
        deps.extend(deps_on(target, &refs));
        if let Some(key) = index_ids.get(&(r.text("rid").to_string(), r.text("kiid").to_string())) {
            deps.insert(key.clone());
        }
        let drop = if system {
            drop_found(
                &owner.name,
                &format!(
                    "quotename(f.name) from sys.foreign_keys f where f.parent_object_id = object_id({}) and f.referenced_object_id = object_id({}) and (select string_agg(c.name, N',') within group (order by fc.constraint_column_id) from sys.foreign_key_columns fc join sys.columns c on c.object_id = fc.parent_object_id and c.column_id = fc.parent_column_id where fc.constraint_object_id = f.object_id) = {}",
                    literal(&owner.name),
                    literal(&target.name),
                    literal(&cols.join(","))
                ),
            )
        } else {
            format!("alter table {} drop constraint {}", owner.name, ident(r.text("name")))
        };
        s.add(Object { id, kind: Some(Kind::Constraint), owner: Some(owner.id.clone()), def: def.clone(), create: vec![format!("alter table {} add {def}", owner.name)], drop: vec![drop], deps, ..Object::default() });
    }

    // Each table's create, with its columns and its keys and checks; a table type is one object.
    for (oid, owner) in &owners {
        let mut parts = lines.remove(oid).unwrap_or_default();
        parts.extend(inline_constraints.remove(oid).unwrap_or_default());
        let body = format!("(\n  {}\n)", parts.join(",\n  "));
        if owner.table_type {
            let deps: BTreeSet<String> = owner.columns.values().flat_map(|c| column_deps.get(c).cloned().unwrap_or_default()).filter(|d| !d.starts_with("column ")).collect();
            let schema = table_types.get(oid).map(|t| t.2.clone()).unwrap_or_default();
            let mut deps = deps;
            deps.extend(schema_dep(&schema));
            s.add(Object { id: owner.id.clone(), kind: Some(Kind::Type), def: format!("as table {body}"), create: vec![format!("create type {} as table {body}", owner.name)], drop: vec![format!("drop type {}", owner.name)], deps, ..Object::default() });
            continue;
        }
        if let Some(t) = s.objects.get_mut(&owner.id) {
            t.create = vec![format!("create table {} {body}", owner.name)];
            t.drop = vec![format!("drop table {}", owner.name)];
        }
    }

    // Sequences.
    for r in db
        .rows(
            "select convert(nvarchar(20), object_id) as oid, schema_name(schema_id) as s, name, type_name(user_type_id) as type, precision, convert(nvarchar(40), start_value) as start, convert(nvarchar(40), increment) as increment,
              convert(nvarchar(40), minimum_value) as min, convert(nvarchar(40), maximum_value) as max, is_cycling as cycling, is_cached as cached, cache_size from sys.sequences where is_ms_shipped = 0",
        )
        .await?
    {
        let name = ctx.name(r.text("s"), r.text("name"));
        let ty = match r.text("type") {
            t @ ("decimal" | "numeric") => format!("{t}({}, 0)", r.text("precision")),
            t => t.to_string(),
        };
        let cache = match (r.flag("cached"), r.get("cache_size")) {
            (false, _) => "no cache".to_string(),
            (true, Some(n)) => format!("cache {n}"),
            (true, None) => "cache".to_string(),
        };
        let def = format!(
            "as {ty} start with {} increment by {} minvalue {} maxvalue {} {} {cache}",
            r.text("start"),
            r.text("increment"),
            r.text("min"),
            r.text("max"),
            if r.flag("cycling") { "cycle" } else { "no cycle" }
        );
        let id = format!("sequence {name}");
        objects.insert(r.text("oid").to_string(), id.clone());
        s.add(Object { id, kind: Some(Kind::Sequence), def: def.clone(), create: vec![format!("create sequence {name} {def}")], drop: vec![format!("drop sequence {name}")], deps: schema_dep(r.text("s")), ..Object::default() });
    }

    // Synonyms.
    for r in db.rows("select convert(nvarchar(20), object_id) as oid, schema_name(schema_id) as s, name, base_object_name as base from sys.synonyms where is_ms_shipped = 0").await? {
        let name = ctx.name(r.text("s"), r.text("name"));
        let id = format!("synonym {name}");
        objects.insert(r.text("oid").to_string(), id.clone());
        let def = format!("for {}", r.text("base"));
        s.add(Object { id, kind: Some(Kind::View), def: def.clone(), create: vec![format!("create synonym {name} {def}")], drop: vec![format!("drop synonym {name}")], deps: schema_dep(r.text("s")), ..Object::default() });
    }

    // Views, procedures, functions and triggers: their text.
    let modules = db
        .rows(
            "select convert(nvarchar(20), o.object_id) as oid, schema_name(o.schema_id) as s, o.name, rtrim(o.type) as type, convert(nvarchar(20), o.parent_object_id) as parent, m.definition, m.is_schema_bound as bound
            from sys.objects o join sys.sql_modules m on m.object_id = o.object_id
            where o.is_ms_shipped = 0 and o.type in ('V', 'P', 'FN', 'IF', 'TF', 'TR')",
        )
        .await?;
    let word_of = |t: &str| match t {
        "V" => "view",
        "P" => "procedure",
        "TR" => "trigger",
        _ => "function",
    };
    for r in &modules {
        let name = ctx.name(r.text("s"), r.text("name"));
        let id = format!("{} {name}", word_of(r.text("type")));
        objects.insert(r.text("oid").to_string(), id);
    }
    // What each object depends on, from the server's records.
    let mut recorded: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for r in db
        .rows(
            "select convert(nvarchar(20), referencing_id) as rid, referencing_minor_id as rminor, convert(nvarchar(20), referenced_id) as did, referenced_minor_id as dminor, referenced_class as class
            from sys.sql_expression_dependencies where referenced_id is not null and referencing_class = 1 and referenced_class in (1, 6)",
        )
        .await?
    {
        let from = if r.text("rminor") != "0" { columns_of.get(&(r.text("rid").to_string(), r.text("rminor").to_string())).cloned() } else { objects.get(r.text("rid")).cloned() };
        let to = if r.text("class") == "6" {
            types.get(r.text("did")).cloned()
        } else if r.text("dminor") != "0" {
            columns_of.get(&(r.text("did").to_string(), r.text("dminor").to_string())).cloned().or_else(|| objects.get(r.text("did")).cloned())
        } else {
            objects.get(r.text("did")).cloned()
        };
        if let (Some(from), Some(to)) = (from, to) {
            if from != to {
                recorded.entry(from).or_default().insert(to);
            }
        }
    }
    for r in &modules {
        let name = ctx.name(r.text("s"), r.text("name"));
        let word = word_of(r.text("type"));
        let id = format!("{word} {name}");
        let text = module_text(r.text("definition"));
        let mut deps = recorded.remove(&id).unwrap_or_default();
        deps.extend(schema_dep(r.text("s")));
        let owner = (word == "trigger").then(|| objects.get(r.text("parent")).cloned()).flatten();
        let kind = match word {
            "view" => Kind::View,
            "trigger" => Kind::Trigger,
            _ => Kind::Function,
        };
        // A schema-bound module holds the columns it reads, which no `alter column` changes under it.
        let extra = if r.flag("bound") { vec!["schemabound".to_string()] } else { Vec::new() };
        s.add(Object { id, kind: Some(kind), owner, def: text.clone(), create: vec![text], drop: vec![format!("drop {word} {name}")], deps, change: Change::Alter { blocks: false }, extra, ..Object::default() });
    }
    // The records of what a column or a check depends on (a function, a type).
    for (from, to) in recorded {
        if let Some(o) = s.objects.get_mut(&from) {
            o.deps.extend(to);
        }
    }
    Ok(s)
}

#[derive(Default)]
pub struct SqlserverAlterer;

/// A column's type class, from the head of its definition.
fn type_class(head: &str) -> TypeClass {
    let lower = head.trim().to_lowercase();
    let Some(c) = regex::Regex::new(r"^([a-z0-9]+)(?:\(([0-9]+|max)(?:, ?([0-9]+))?\))?").unwrap().captures(&lower) else { return TypeClass::Other };
    let (p, s) = (c.get(2).and_then(|m| m.as_str().parse::<u32>().ok()), c.get(3).and_then(|m| m.as_str().parse::<u32>().ok()));
    match &c[1] {
        "tinyint" => TypeClass::Int(8),
        "smallint" => TypeClass::Int(16),
        "int" => TypeClass::Int(32),
        "bigint" => TypeClass::Int(64),
        "real" => TypeClass::Float(6),
        "float" => TypeClass::Float(15),
        "decimal" | "numeric" => TypeClass::Numeric(Some(s.unwrap_or(0))),
        "money" | "smallmoney" => TypeClass::Numeric(Some(4)),
        "date" => TypeClass::Date,
        "smalldatetime" => TypeClass::Timestamp { zone: false, precision: 0 },
        "datetime" => TypeClass::Timestamp { zone: false, precision: 3 },
        "datetime2" => TypeClass::Timestamp { zone: false, precision: p.unwrap_or(7) },
        "datetimeoffset" => TypeClass::Timestamp { zone: true, precision: p.unwrap_or(7) },
        "time" => TypeClass::Time { precision: p.unwrap_or(7) },
        "char" | "varchar" | "nchar" | "nvarchar" | "text" | "ntext" => TypeClass::Text,
        _ => TypeClass::Other,
    }
}

/// The statement that drops a column's default: by its name, or found by its column where SQL Server named it.
fn drop_default(table: &str, column: &str, name: &str) -> String {
    if name.is_empty() {
        drop_found(
            table,
            &format!(
                "quotename(d.name) from sys.default_constraints d join sys.columns c on c.object_id = d.parent_object_id and c.column_id = d.parent_column_id where d.parent_object_id = object_id({}) and c.name = {}",
                literal(table),
                literal(column)
            ),
        )
    } else {
        format!("alter table {table} drop constraint {}", ident(name))
    }
}

/// Whether a column's type, collation or nullability changes, which `alter column` writes.
fn head_changes(from: &Object, to: &Object) -> bool {
    part(from, "head") != part(to, "head") || part(from, "nullable") != part(to, "nullable")
}

impl Alterer for SqlserverAlterer {
    fn alter(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        match to.kind? {
            Kind::Column => {
                // An identity, a sparse or rowguidcol column, or a computed one, is added again.
                if part(from, "special") != part(to, "special") || part(to, "head")?.starts_with("as ") {
                    return None;
                }
                let (table, column) = (part(to, "table")?, part(to, "column")?);
                let head = head_changes(from, to);
                let default = (part(from, "default"), part(from, "default_name")) != (part(to, "default"), part(to, "default_name"));
                let mut out = Vec::new();
                if part(from, "default").is_some() && (default || head) {
                    out.push(drop_default(table, column, part(from, "default_name").unwrap_or_default()));
                }
                if head {
                    let nullable = if part(to, "nullable") == Some("true") { "null" } else { "not null" };
                    out.push(format!("alter table {table} alter column {} {} {nullable}", ident(column), part(to, "head")?));
                }
                if let (Some(expr), true) = (part(to, "default"), default || head) {
                    let name = part(to, "default_name").filter(|n| !n.is_empty()).map(|n| format!("constraint {} ", ident(n))).unwrap_or_default();
                    out.push(format!("alter table {table} add {name}default {expr} for {}", ident(column)));
                }
                Some(out)
            }
            Kind::View | Kind::Function | Kind::Trigger if !to.id.starts_with("synonym ") => Some(vec![create_or_alter(to.create.first()?)]),
            _ => None,
        }
    }

    fn rename(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        if to.kind != Some(Kind::Column) {
            return None;
        }
        let (table, old, new) = (part(to, "table")?, part(from, "column")?, part(to, "column")?);
        Some(vec![format!("exec sp_rename {}, {}, N'COLUMN'", literal(&format!("{table}.[{}]", old.replace(']', "]]"))), literal(new))])
    }

    /// SQL Server refuses `alter column` on a column a key, a check, an index or a schema-bound
    /// module holds.
    fn refused_by(&self, dependent: &Object) -> bool {
        match dependent.kind {
            Some(Kind::Constraint | Kind::Index) => true,
            Some(Kind::View | Kind::Function) => dependent.extra.iter().any(|e| e == "schemabound"),
            _ => false,
        }
    }

    fn blocks(&self, from: &Object, to: &Object) -> bool {
        from.kind != Some(Kind::Column) || head_changes(from, to)
    }

    fn narrows(&self, from: &Object, to: &Object) -> Option<String> {
        if from.kind != Some(Kind::Column) {
            return None;
        }
        let (a, b) = (part(from, "head")?, part(to, "head")?);
        let why = super::narrowing(&type_class(a), &type_class(b))?;
        Some(format!("changes from {a} to {b}, which {why}"))
    }

    /// A `GO` line before and after each statement that must open its batch, above the comments
    /// that speak of it.
    fn finish(&self, statements: &mut Vec<String>) {
        let mut out: Vec<String> = Vec::new();
        let last = statements.iter().rposition(|s| !s.starts_with("--"));
        for (k, s) in statements.iter().enumerate() {
            if s.starts_with("--") || !opens_batch(s) {
                out.push(s.clone());
                continue;
            }
            let mut at = out.len();
            while at > 0 && out[at - 1].starts_with("--") {
                at -= 1;
            }
            if out[..at].iter().any(|s| !s.starts_with("--")) && out.get(at.wrapping_sub(1)).is_none_or(|p| p != GO) {
                out.insert(at, GO.into());
            }
            out.push(s.clone());
            if Some(k) != last {
                out.push(GO.into());
            }
        }
        *statements = out;
    }
}

/// A plan's statements as a file a person reads: a blank line between statements, and a `GO`
/// line right under the statement it ends.
pub fn written(statements: &[String]) -> String {
    let mut out = String::new();
    let mut after_comment = false;
    for s in statements {
        if s == GO {
            out.push_str("GO\n");
            after_comment = false;
            continue;
        }
        if !out.is_empty() && !after_comment {
            out.push('\n');
        }
        after_comment = s.starts_with("--");
        out.push_str(&if after_comment { s.trim_end().to_string() } else { super::ended(s) });
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_names_bare_where_tsql_reads_them_so() {
        assert_eq!(ident("users"), "users");
        assert_eq!(ident("order"), "[order]");
        assert_eq!(ident("Go"), "[Go]");
        assert_eq!(ident("first name"), "[first name]");
        assert_eq!(ident("a]b"), "[a]]b]");
        assert_eq!(literal("it's"), "N'it''s'");
    }

    #[test]
    fn compares_a_module_by_its_text_from_create_on() {
        assert_eq!(module_text("-- totals\nCREATE   VIEW dbo.v AS select 1;\n"), "create view dbo.v AS select 1");
        assert_eq!(module_text("create or alter procedure p as select 1"), "create procedure p as select 1");
        assert_eq!(module_text("ALTER PROC p as select 1"), "create procedure p as select 1");
        assert_eq!(create_or_alter("create view v as select 1"), "create or alter view v as select 1");
    }

    #[test]
    fn puts_a_module_in_a_batch_of_its_own() {
        let mut statements = vec!["create table t (id int not null)".to_string(), "-- the view".to_string(), "create view v as select id from t".to_string(), "create index i on t (id)".to_string()];
        SqlserverAlterer.finish(&mut statements);
        assert_eq!(statements, ["create table t (id int not null)", "GO", "-- the view", "create view v as select id from t", "GO", "create index i on t (id)"]);
        let mut first = vec!["create schema app".to_string(), "create table app.t (id int not null)".to_string()];
        SqlserverAlterer.finish(&mut first);
        assert_eq!(first, ["create schema app", "GO", "create table app.t (id int not null)"]);
        assert_eq!(written(&first), "create schema app;\nGO\n\ncreate table app.t (id int not null);\n");
    }

    #[test]
    fn changes_a_column_around_its_default() {
        let column = |head: &str, nullable: bool, default: Option<(&str, &str)>| {
            let mut extra = vec!["table=t".to_string(), "column=c".to_string(), format!("head={head}"), "special=".to_string(), format!("nullable={nullable}")];
            if let Some((expr, name)) = default {
                extra.push(format!("default={expr}"));
                extra.push(format!("default_name={name}"));
            }
            Object { id: "column t.c".into(), kind: Some(Kind::Column), def: format!("{head}{nullable}{default:?}"), extra, ..Object::default() }
        };
        let a2 = column("int", false, Some(("((0))", "")));
        let b = column("bigint", false, Some(("((0))", "")));
        let out = SqlserverAlterer.alter(&a2, &b).unwrap();
        assert_eq!(out.len(), 3);
        assert!(out[0].starts_with("declare @reylo_") && out[0].contains("drop constraint"));
        assert_eq!(out[1], "alter table t alter column c bigint not null");
        assert_eq!(out[2], "alter table t add default ((0)) for c");
        let named_b = column("int", false, Some(("((1))", "df_c")));
        let changed = SqlserverAlterer.alter(&a2, &named_b).unwrap();
        assert_eq!(changed.len(), 2);
        assert_eq!(changed[1], "alter table t add constraint df_c default ((1)) for c");
        assert!(!SqlserverAlterer.blocks(&a2, &named_b));
        assert!(SqlserverAlterer.blocks(&a2, &b));
        assert_eq!(SqlserverAlterer.narrows(&b, &a2), None);
        let mut identity = b.clone();
        identity.extra[3] = "special=identity(1, 1)".into();
        assert_eq!(SqlserverAlterer.alter(&b, &identity), None);
    }
}
