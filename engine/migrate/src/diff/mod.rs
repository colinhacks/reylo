//! The diff engine: the SQL that takes one database's schema to another's. Each database's reader
//! (`postgres`, …) reads a schema into objects, each with its definition, the statements that
//! create and drop it, how to change it in place where it can be, and what it depends on, read
//! from the server's own dependency records. The plan is then the same for every database: drop
//! what the target lacks, create what the source lacks, change what differs (in place, or by
//! dropping and creating it again, with everything that depends on it), in an order the
//! dependencies allow. Nothing here trusts the plan: `reylo migrate new` runs it on a shadow
//! of the migrations and compares the result with `schema.sql` before it writes the file.

pub mod mysql;
pub mod postgres;
pub mod sqlite;

use crate::db::Db;
use crate::{Dialect, Error};
use std::collections::{BTreeMap, BTreeSet};

/// The schema of the database `db` is connected to, as its database's reader reads it.
pub async fn read(db: &mut Db) -> Result<Schema, Error> {
    match db.dialect() {
        Dialect::Postgres => postgres::read(db).await,
        Dialect::Mysql => mysql::read(db).await,
        Dialect::Sqlite => sqlite::read(db).await,
    }
}

/// The alterer of the database `db` is connected to.
pub async fn alterer(db: &mut Db) -> Result<Box<dyn Alterer>, Error> {
    Ok(match db.dialect() {
        Dialect::Postgres => {
            let version = db.rows("select current_setting('server_version_num') as v").await?.first().and_then(|r| r.first()?.parse().ok()).unwrap_or(0);
            Box::new(postgres::PostgresAlterer::read(db, version).await?)
        }
        Dialect::Mysql => {
            let mode = db.rows("select @@session.sql_mode as m").await?.first().map(|r| r.text("m").to_string()).unwrap_or_default();
            Box::new(mysql::MysqlAlterer::for_mode(&mode))
        }
        Dialect::Sqlite => Box::new(sqlite::SqliteAlterer::default()),
    })
}

/// What an object is, in the order objects of no dependency between them are created.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Schema,
    Extension,
    Collation,
    Type,
    Sequence,
    Table,
    Column,
    SequenceOwner,
    /// A column's storage, compression or statistics target, set once the column is there.
    ColumnSetting,
    Constraint,
    Index,
    View,
    /// After tables and views: a SQL function's body names them, and its server reads it when the
    /// function is made, while a function a table or a view uses is a dependency they record.
    Function,
    Operator,
    Cast,
    Server,
    Rule,
    Trigger,
    Policy,
    Rls,
    Statistics,
    Grant,
    Comment,
}

/// One object of a schema.
#[derive(Clone, Debug, Default)]
pub struct Object {
    pub id: String,
    pub kind: Option<Kind>,
    /// The object it belongs to and goes with: a column's, an index's or a trigger's table.
    pub owner: Option<String>,
    /// What is compared: two objects of one id with one definition are the same.
    pub def: String,
    pub create: Vec<String>,
    pub drop: Vec<String>,
    /// Whether creating its owner creates it too (a new table's columns).
    pub inline: bool,
    /// The objects it depends on, by id, in its own database.
    pub deps: BTreeSet<String>,
    /// What a change of definition needs beyond its own statements, decided by its database.
    pub change: Change,
    /// A warning written above its create when it is created on its own (a not-null column with
    /// no default, which fails on a table with rows).
    pub caution: Option<String>,
    /// What its database's alterer needs to know beyond the definition: for an enum, the columns
    /// of its type, each `table|column|type|default`.
    pub extra: Vec<String>,
    /// An object the server drops it with, which is not its owner: a serial column's sequence.
    pub goes_with: Option<String>,
    /// Objects it is made after that it doesn't depend on: a partitioned table's index, made after
    /// the partitions' indexes it attaches as it is made.
    pub after: BTreeSet<String>,
}

/// How an object's definition changes when it differs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Change {
    /// Dropped and created again, with everything that depends on it.
    #[default]
    Replace,
    /// Changed in place by statements its database decides from both definitions; `blocks` says
    /// whether the change is refused while a view, rule, policy or trigger depends on it (a
    /// column's type), so those are dropped before it and created again after.
    Alter { blocks: bool },
}

/// One schema, as its database's reader read it.
#[derive(Clone, Debug, Default)]
pub struct Schema {
    pub objects: BTreeMap<String, Object>,
    /// Statements taken as written from the file that declared the schema, which keep its text.
    pub verbatim: BTreeSet<String>,
    /// Read from the database a command changes, not from a replay: its partitions that no
    /// migration made (made at run time) are its data, which no plan from it drops.
    pub live: bool,
}

impl Schema {
    pub fn add(&mut self, object: Object) {
        self.objects.insert(object.id.clone(), object);
    }
}

/// Statements that change an object in place from one definition to another, or None where it
/// cannot be and is replaced.
pub trait Alterer: Send {
    fn alter(&self, from: &Object, to: &Object) -> Option<Vec<String>>;
    /// Statements that rename a column in place, keeping its rows.
    fn rename(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        let _ = (from, to);
        None
    }
    /// Both schemas, given before it asks for any statement: an alterer whose statements for one
    /// table reach others (SQLite's rebuild, the tables that reference it) reads them.
    fn schemas(&self, from: &Schema, to: &Schema) {
        let _ = (from, to);
    }
    /// The columns the planner renames, each an old column's id and a new one's, given before it asks
    /// for any statement: an alterer whose table statements write a table's renames (MySQL's alter
    /// table, SQLite's rebuild) writes these, and no others.
    fn decide(&self, renames: &[(String, String)]) {
        let _ = renames;
    }
    /// Where a column's change of type keeps it but can lose what its values hold without failing
    /// (a timestamp to a date, a smaller scale), what it loses, as the end of a sentence: "changes
    /// from timestamptz to date, which keeps only the date".
    fn narrows(&self, from: &Object, to: &Object) -> Option<String> {
        let _ = (from, to);
        None
    }
    /// Whether a blocking change of `from` to `to` changes the object under the functions that
    /// depend on it (an enum made again, a column's type), so that each is made again around the
    /// change rather than left on what it was, or refusing the change.
    fn remakes(&self, from: &Object, to: &Object) -> bool {
        let _ = (from, to);
        false
    }
    /// The objects a change of `from` to `to` makes again itself (an enum's domains, a composite
    /// type's attributes it puts in order), which the plan then leaves alone.
    fn takes_along(&self, from: &Object, to: &Object) -> Vec<String> {
        let _ = (from, to);
        Vec::new()
    }
    /// The columns, of other objects, that a change of `from` to `to` converts in place (through text,
    /// around a type's attribute changing under them), whose views, policies, rules, triggers and
    /// function bodies then go and come back around it, as a column's whose type changes do.
    fn converts(&self, from: &Object, to: &Object) -> Vec<String> {
        let _ = (from, to);
        Vec::new()
    }
    /// Where a column's default reads an object made again (a function), the statements that take
    /// the default off before the object goes, and those that set the target's after it is back,
    /// which keep the column and its rows; None where it can't (and the column is made again).
    fn unhook(&self, from: &Object, to: &Object) -> Option<(Vec<String>, Vec<String>)> {
        let _ = (from, to);
        None
    }
    /// The plan's statements once ordered, for the settings that let them run in that order: a
    /// Postgres function's body calls others the server keeps no record of, so no order is safe.
    fn finish(&self, statements: &mut Vec<String>) {
        let _ = statements;
    }
}

/// A column type, as far as a change between two of them can lose values without failing.
#[derive(Clone, Debug, PartialEq)]
pub enum TypeClass {
    /// An integer of this many bits.
    Int(u32),
    /// A float of about this many significant digits.
    Float(u32),
    /// A decimal of this scale, or any.
    Numeric(Option<u32>),
    Date,
    Timestamp { zone: bool, precision: u32 },
    Time { precision: u32 },
    /// An interval, with the fields it keeps (`day`, `hour to minute`), or all of them.
    Interval(Option<String>),
    /// Text: a string type, or SQLite's text or no affinity, for which numbers are kept as written.
    Text,
    /// JSON kept as written (Postgres's `json`).
    Json,
    /// JSON kept as parsed (Postgres's `jsonb`, MySQL's `json`): duplicate keys, key order and spacing go.
    Jsonb,
    Other,
}

/// What a generated column's object carries, whose values are its expression's.
pub const GENERATED: &str = "generated";

/// What a change of a column's values from one type to another loses without failing, written
/// after "which": None where every value is kept or the change fails on one it can't hold.
pub fn narrowing(from: &TypeClass, to: &TypeClass) -> Option<String> {
    use TypeClass::*;
    let digits = |n: u32| format!("keeps about {n} significant digits");
    let places = |p: u32| if p == 0 { "rounds to whole seconds".to_string() } else { format!("rounds to {p} decimal places of a second") };
    match (from, to) {
        (Timestamp { .. }, Date) => Some("keeps only the date".into()),
        (Timestamp { .. }, Time { .. }) => Some("keeps only the time of day".into()),
        (Timestamp { zone: true, .. }, Timestamp { zone: false, .. }) => Some("keeps the local time, not the moment".into()),
        (Timestamp { precision: a, .. }, Timestamp { precision: b, .. }) | (Time { precision: a }, Time { precision: b }) if b < a => Some(places(*b)),
        (Numeric(a), Numeric(Some(b))) if a.is_none_or(|a| *b < a) => Some(if *b == 0 { "rounds to whole numbers".into() } else { format!("rounds to {b} decimal places") }),
        (Float(_), Numeric(Some(b))) => Some(if *b == 0 { "rounds to whole numbers".into() } else { format!("rounds to {b} decimal places") }),
        (Numeric(s), Int(_)) if *s != Some(0) => Some("rounds to whole numbers".into()),
        (Float(_), Int(_)) => Some("rounds to whole numbers".into()),
        (Float(a), Float(b)) if b < a => Some(digits(*b)),
        (Numeric(_), Float(b)) => Some(digits(*b)),
        (Int(bits), Float(b)) if (*b <= 6 && *bits > 16) || (*b <= 15 && *bits > 32) => Some(digits(*b)),
        (Interval(a), Interval(Some(b))) if a.as_deref() != Some(b.as_str()) => Some(format!("keeps only the {b} of each interval")),
        (Text, Int(_) | Float(_) | Numeric(_)) => Some("stores text that reads as a number as that number ('007' becomes 7)".into()),
        (Text, Date | Timestamp { .. } | Time { .. }) => Some("stores text that reads as a time as that time ('2026-1-2' becomes 2026-01-02)".into()),
        (Text | Json, Jsonb) => Some("keeps the JSON's values, not its duplicate keys, key order or spacing".into()),
        _ => None,
    }
}

/// The renames the planner decided, which an alterer that writes them in a table's statement keeps.
#[derive(Debug, Default)]
pub struct Decided(std::cell::RefCell<Option<Vec<(String, String)>>>);

impl Decided {
    pub fn set(&self, renames: &[(String, String)]) {
        *self.0.borrow_mut() = Some(renames.to_vec());
    }

    /// The columns of the table named `table` (as its id writes it) renamed, each its old name and
    /// its new one; None where no planner decided, and the alterer decides as the planner would.
    pub fn of(&self, table: &str) -> Option<Vec<(String, String)>> {
        let prefix = format!("column {table}.");
        self.0.borrow().as_ref().map(|all| all.iter().filter_map(|(old, new)| Some((old.strip_prefix(&prefix)?.to_string(), new.strip_prefix(&prefix)?.to_string()))).collect())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Op {
    Drop(String),
    Alter(String),
    Create(String),
    /// A column's default taken off before what it reads is made again, and set after.
    Unhook(String),
    Rehook(String),
}

/// What a plan does. A statement that opens with `--` is a comment for the person reading the
/// file, about the statement after it.
#[derive(Debug, Default)]
pub struct Plan {
    pub statements: Vec<String>,
    /// Data the plan loses, for the file's comments: a table or column dropped.
    pub losses: Vec<String>,
    /// Columns it assumes renamed rather than dropping one and adding the other, each the old
    /// column's id, the new name, and what else changes (`text → citext`) where the definition does,
    /// for the person running `create` to settle.
    pub renames: Vec<(String, String, Option<String>)>,
    /// Columns kept whose change of type can lose what their values hold without failing, each the
    /// column's id and what it loses ("changes from … to …, which …"), for `--drop` to accept.
    pub narrowings: Vec<(String, String)>,
    /// Columns it drops while adding one to the same table, alone there, of another family of types
    /// (text to a date): never renamed unless `--rename` says so, but as much a question as a rename
    /// it guesses. Each the old column's id, the new one's name, what else changes (`text → date`),
    /// and what a rename's change of type would lose, where it loses something.
    pub pairs: Vec<(String, String, String, Option<String>)>,
}

/// A statement of a plan that commits the transaction and begins the next: what follows uses what
/// came before in a way Postgres refuses within one transaction (a label added to an enum). A
/// migration file ends at it, and a push commits there.
pub const COMMIT: &str = "commit;\nbegin";

impl Plan {
    /// The plan as the plans each transaction runs: one, or several where it commits partway, the
    /// first with what the whole loses.
    pub fn transactions(&self) -> Vec<Plan> {
        let mut out = vec![Plan { losses: self.losses.clone(), renames: self.renames.clone(), narrowings: self.narrowings.clone(), ..Plan::default() }];
        for s in &self.statements {
            if s == COMMIT {
                out.push(Plan::default());
            } else if let Some(last) = out.last_mut() {
                last.statements.push(s.clone());
            }
        }
        out
    }

    /// The plan as a migration file: what it loses as comments, then each statement.
    pub fn sql(&self) -> String {
        let mut out = String::new();
        for loss in &self.losses {
            out.push_str(&format!("-- loses data: {}\n", loss.trim_start_matches("column ").trim_start_matches("table ")));
        }
        for (column, why) in &self.narrowings {
            out.push_str(&format!("-- loses data: {} {why}\n", column.trim_start_matches("column ")));
        }
        for s in &self.statements {
            out.push_str(s.trim_end());
            out.push_str(if s.starts_with("--") { "\n" } else { ";\n" });
        }
        out
    }

    /// The statements to run, comments left out.
    pub fn runnable(&self) -> impl Iterator<Item = &String> {
        self.statements.iter().filter(|s| !s.starts_with("--"))
    }
}

/// The objects `id` is dropped with: its owners, and what the server drops it with, nearest first.
fn owners<'a>(schema: &'a Schema, id: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut queue: std::collections::VecDeque<&str> = schema.objects.get(id).into_iter().flat_map(|o| o.owner.as_deref().into_iter().chain(o.goes_with.as_deref())).collect();
    while let Some(next) = queue.pop_front() {
        if out.contains(&next) {
            continue;
        }
        out.push(next);
        if let Some(o) = schema.objects.get(next) {
            queue.extend(o.owner.as_deref().into_iter().chain(o.goes_with.as_deref()));
        }
    }
    out
}

/// `id` and every object it owns, at any depth.
fn with_owned<'a>(schema: &'a Schema, id: &'a str) -> Vec<&'a str> {
    let mut out = vec![id];
    let mut k = 0;
    while k < out.len() {
        let at = out[k];
        out.extend(schema.objects.values().filter(|o| o.owner.as_deref() == Some(at)).map(|o| o.id.as_str()));
        k += 1;
    }
    out
}

pub use crate::sql::last_written;

/// `text` with the name `old` (a table's) written `new` wherever it stands as a name or as the start
/// of one (`users` in `users_pkey`, `nextval('users_id_seq')`), ignoring case.
pub fn swap_name(text: &str, old: &str, new: &str) -> String {
    if old.is_empty() {
        return text.to_string();
    }
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut before: Option<char> = None;
    while let Some(c) = rest.chars().next() {
        let starts = rest.len() >= old.len() && rest.is_char_boundary(old.len()) && rest[..old.len()].eq_ignore_ascii_case(old);
        if starts && !before.is_some_and(ident) {
            let after = rest[old.len()..].chars().next();
            if after.is_none_or(|a| a == '_' || !ident(a)) {
                out.push_str(new);
                rest = &rest[old.len()..];
                before = old.chars().last();
                continue;
            }
        }
        out.push(c);
        before = Some(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// The family of a column's type, from the first word of its definition, in any database's
/// spelling: numbers, strings, times, and so on; a type of none of them is its own.
fn family(def: &str) -> String {
    let first = def.trim_start().split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or_default().to_lowercase();
    let of = |words: &[&str]| words.contains(&first.as_str());
    if of(&["smallint", "integer", "int", "int2", "int4", "int8", "bigint", "tinyint", "mediumint", "serial", "bigserial", "smallserial", "numeric", "decimal", "real", "double", "float", "float4", "float8", "number", "money"]) {
        "number".into()
    } else if of(&["text", "varchar", "char", "character", "nchar", "nvarchar", "citext", "name", "string", "clob", "tinytext", "mediumtext", "longtext"]) {
        "string".into()
    } else if of(&["date", "time", "timestamp", "timestamptz", "timetz", "datetime", "year"]) {
        "time".into()
    } else if of(&["blob", "bytea", "binary", "varbinary", "tinyblob", "mediumblob", "longblob"]) {
        "binary".into()
    } else {
        first
    }
}

/// The table a partition, or a table that inherits another, belongs to, by id.
/// Whether the object is a partition, or an inheriting child, of a table.
pub fn is_child(o: &Object) -> bool {
    parent_of(o).is_some()
}

fn parent_of(o: &Object) -> Option<String> {
    o.extra.iter().find_map(|l| l.strip_prefix("partition of ").or_else(|| l.strip_prefix("inherits ")).map(|p| format!("table {}", p.split(' ').next().unwrap_or_default())))
}

/// `schema` without its partitions (and inheriting children) of a table `reference` has that
/// `reference` lacks, and what they own: children made at run time, which are data.
pub fn without_extra_children(schema: &mut Schema, reference: &Schema) {
    // A partition of a partition made at run time is made at run time too, as far down as they go.
    let mut extra: Vec<String> = Vec::new();
    loop {
        let more: Vec<String> = schema.objects.values().filter(|o| o.kind == Some(Kind::Table) && !reference.objects.contains_key(&o.id) && !extra.contains(&o.id) && parent_of(o).is_some_and(|p| reference.objects.contains_key(&p) || extra.contains(&p))).map(|o| o.id.clone()).collect();
        if more.is_empty() {
            break;
        }
        extra.extend(more);
    }
    for id in extra {
        let owned: Vec<String> = with_owned(schema, &id).into_iter().map(String::from).collect();
        for o in owned {
            schema.objects.remove(&o);
        }
    }
}

/// A table's id's parts (`table public.users` is public, users), unquoted.
fn table_parts(id: &str) -> Vec<String> {
    let rest = id.split_once(' ').map_or(id, |(_, r)| r);
    let mut out = vec![String::new()];
    let mut quoted: Option<char> = None;
    for ch in rest.chars() {
        match quoted {
            Some(q) if ch == q => quoted = None,
            Some(_) => out.last_mut().unwrap().push(ch),
            None if ch == '"' || ch == '`' => quoted = Some(ch),
            None if ch == '.' => out.push(String::new()),
            None => out.last_mut().unwrap().push(ch),
        }
    }
    out
}

/// The tables renamed from `from` to `to`, each its old id, its new id, and whether it was stated:
/// those `stated` names, and a table that goes while one comes in its schema with the same columns
/// (each its name and definition, the names derived from the table's, and from any table renamed
/// with it, following them), alone in that both ways, with no table that stays alike, and not one
/// `dropped` names. A partition is renamed with its table.
pub fn table_renames(from: &Schema, to: &Schema, stated: &[(String, String)], dropped: &BTreeSet<String>) -> Vec<(String, String, bool)> {
    let tables = |a: &Schema, b: &Schema| -> Vec<String> { a.objects.values().filter(|o| o.kind == Some(Kind::Table) && !b.objects.contains_key(&o.id)).map(|o| o.id.clone()).collect() };
    let (mut gone, come) = (tables(from, to), tables(to, from));
    if from.live {
        let kept_parent = |id: &String| from.objects[id].extra.iter().any(|l| l.strip_prefix("partition of ").or_else(|| l.strip_prefix("inherits ")).is_some_and(|p| to.objects.contains_key(&format!("table {}", p.split(' ').next().unwrap_or_default()))));
        gone.retain(|id| !kept_parent(id));
    }
    let stays: Vec<String> = from.objects.values().filter(|o| o.kind == Some(Kind::Table) && to.objects.contains_key(&o.id)).map(|o| o.id.clone()).collect();
    let mut out: Vec<(String, String, bool)> = stated.iter().filter(|(o, n)| gone.contains(o) && come.contains(n)).map(|(o, n)| (o.clone(), n.clone(), true)).collect();
    let bare = |id: &str| table_parts(id).pop().unwrap_or_default();
    let shape = |schema: &Schema, id: &str, swaps: &[(String, String)]| -> (String, Vec<(String, String)>) {
        let swapped = |text: &str| swaps.iter().fold(text.to_string(), |t, (old, new)| swap_name(&t, old, new));
        let mut columns: Vec<(String, String)> = schema.objects.values().filter(|o| o.kind == Some(Kind::Column) && o.owner.as_deref() == Some(id)).map(|o| (table_parts(&o.id).pop().unwrap_or_default(), swapped(&o.def))).collect();
        columns.sort();
        (swapped(&schema.objects[id].def), columns)
    };
    let same_schema = |a: &str, b: &str| {
        let (a, b) = (table_parts(a), table_parts(b));
        a.len() == b.len() && a[..a.len() - 1] == b[..b.len() - 1]
    };
    // Each pass may find a partition whose table the pass before renamed.
    loop {
        let known: Vec<(String, String)> = out.iter().map(|(o, n, _)| (bare(o), bare(n))).collect();
        let alike = |old: &String, new: &String| -> bool {
            let swaps: Vec<(String, String)> = std::iter::once((bare(old), bare(new))).chain(known.iter().cloned()).collect();
            let ours = shape(from, old, &swaps);
            same_schema(old, new) && !ours.1.is_empty() && ours == shape(to, new, &[])
        };
        let free_gone: Vec<&String> = gone.iter().filter(|g| !dropped.contains(*g) && !out.iter().any(|(o, _, _)| o == *g)).collect();
        let free_come: Vec<&String> = come.iter().filter(|c| !out.iter().any(|(_, n, _)| n == *c)).collect();
        let mut found = Vec::new();
        for old in &free_gone {
            let same: Vec<&&String> = free_come.iter().filter(|c| alike(old, c)).collect();
            if same.len() != 1 || free_gone.iter().filter(|g| alike(g, same[0])).count() != 1 {
                continue;
            }
            // A table that stays as the one that comes is: which of them is the old one is anyone's guess.
            if stays.iter().any(|t| same_schema(t, same[0]) && shape(to, t, &[(bare(t), bare(same[0]))]) == shape(to, same[0], &[])) {
                continue;
            }
            found.push(((*old).clone(), (*same[0]).clone(), false));
        }
        if found.is_empty() {
            // Last, a table that goes while one comes in its schema, alone both ways, with most of
            // the new one's columns by name: renamed, its columns then changed as any table's.
            // Columns most tables have (a key, timestamps) say nothing of which table it is.
            const COMMON: [&str; 5] = ["id", "created_at", "updated_at", "inserted_at", "deleted_at"];
            let names = |schema: &Schema, id: &str| -> BTreeSet<String> { schema.objects.values().filter(|o| o.kind == Some(Kind::Column) && o.owner.as_deref() == Some(id)).map(|o| table_parts(&o.id).pop().unwrap_or_default().to_lowercase()).filter(|n| !COMMON.contains(&n.as_str())).collect() };
            for old in &free_gone {
                let peers: Vec<&&String> = free_gone.iter().filter(|g| same_schema(g, old)).collect();
                let candidates: Vec<&&String> = free_come.iter().filter(|c| same_schema(c, old) && !stays.iter().any(|t| same_schema(t, c) && shape(to, t, &[(bare(t), bare(c))]) == shape(to, c, &[]))).collect();
                if peers.len() != 1 || candidates.len() != 1 {
                    continue;
                }
                let (theirs, ours) = (names(to, candidates[0]), names(from, old));
                let shared = theirs.intersection(&ours).count();
                if shared > 0 && shared * 2 > theirs.len() && shared * 2 > ours.len() {
                    found.push(((*old).clone(), (*candidates[0]).clone(), false));
                }
            }
            if found.is_empty() {
                break;
            }
        }
        out.extend(found);
    }
    out
}

/// How a column's definition changes, without the words at the end they share: `varchar(20) →
/// varchar(40)`, whatever collation both have.
fn definition_change(a: &str, b: &str) -> String {
    let (x, y): (Vec<&str>, Vec<&str>) = (a.split_whitespace().collect(), b.split_whitespace().collect());
    let same = x.iter().rev().zip(y.iter().rev()).take_while(|(p, q)| p == q).count().min(x.len().saturating_sub(1)).min(y.len().saturating_sub(1));
    format!("{} → {}", x[..x.len() - same].join(" "), y[..y.len() - same].join(" "))
}

/// How the columns of the table `old` of `from` differ from those of the table `new` of `to`, in words
/// (`adds total, drops legacy`); None where they don't.
pub fn column_changes(from: &Schema, to: &Schema, old: &str, new: &str) -> Option<String> {
    let bare = |id: &str| table_parts(id).pop().unwrap_or_default();
    let (old_name, new_name) = (bare(old), bare(new));
    let columns = |schema: &Schema, id: &str, swap: bool| -> BTreeMap<String, String> {
        schema.objects.values().filter(|o| o.kind == Some(Kind::Column) && o.owner.as_deref() == Some(id)).map(|o| (bare(&o.id), if swap { swap_name(&o.def, &old_name, &new_name) } else { o.def.clone() })).collect()
    };
    let (ours, theirs) = (columns(from, old, true), columns(to, new, false));
    let list = |word: &str, names: Vec<&String>| (!names.is_empty()).then(|| format!("{word} {}", names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")));
    let adds = list("adds", theirs.keys().filter(|n| !ours.contains_key(*n)).collect());
    let drops = list("drops", ours.keys().filter(|n| !theirs.contains_key(*n)).collect());
    let changes = list("changes", ours.iter().filter(|(n, def)| theirs.get(*n).is_some_and(|d| d != *def)).map(|(n, _)| n).collect());
    let words: Vec<String> = [adds, drops, changes].into_iter().flatten().collect();
    (!words.is_empty()).then(|| words.join(", "))
}

/// The objects `drops` names that a drop of their owner would take with it, but that are dropped on
/// their own, before it, to break a circle: two tables whose foreign keys reference each other, or a
/// table whose check calls a function that takes the table's row, can't be dropped one after the
/// other, but each drop can once one of those constraints has gone first.
fn drops_apart(from: &Schema, drops: &BTreeSet<String>) -> BTreeSet<String> {
    let mut apart: BTreeSet<String> = BTreeSet::new();
    loop {
        // Each drop that is a statement of its own, and every object it takes with it: an object goes
        // with its nearest owner that is one.
        let units: BTreeSet<String> = drops.iter().filter(|id| apart.contains(*id) || !owners(from, id).iter().any(|o| drops.contains(*o))).cloned().collect();
        let unit_of = |id: &str| -> Option<String> {
            if units.contains(id) {
                return Some(id.to_string());
            }
            owners(from, id).into_iter().find(|o| units.contains(*o)).map(String::from)
        };
        // A unit drops before each unit that what it takes depends on.
        let mut before: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for o in from.objects.values() {
            let Some(mine) = unit_of(&o.id) else { continue };
            for d in &o.deps {
                if let Some(theirs) = unit_of(d).filter(|t| *t != mine) {
                    before.entry(theirs).or_default().insert(mine.clone());
                }
            }
        }
        // Kahn's order: what is left is a circle, or waits on one.
        let mut left: BTreeSet<String> = units.clone();
        loop {
            let ready: Vec<String> = left.iter().filter(|u| before.get(*u).is_none_or(|b| b.iter().all(|x| !left.contains(x)))).cloned().collect();
            if ready.is_empty() {
                break;
            }
            for u in ready {
                left.remove(&u);
            }
        }
        if left.is_empty() {
            return apart;
        }
        // An object one of them takes with it that can go on its own, and depends on another of them:
        // a constraint, an index or the like first, else a column of a table that goes anyway.
        let breaks = |o: &&Object| {
            let Some(mine) = unit_of(&o.id).filter(|u| *u != o.id && left.contains(u)) else { return false };
            !o.drop.is_empty() && o.deps.iter().any(|d| unit_of(d).is_some_and(|t| t != mine && left.contains(&t)))
        };
        let breaker = from
            .objects
            .values()
            .filter(|o| matches!(o.kind, Some(Kind::Constraint | Kind::Index | Kind::Trigger | Kind::Policy | Kind::Rule | Kind::Statistics)))
            .find(breaks)
            .or_else(|| from.objects.values().filter(|o| o.kind == Some(Kind::Column)).find(breaks));
        match breaker {
            Some(o) => {
                apart.insert(o.id.clone());
            }
            None => return apart,
        }
    }
}

/// The plan from `from` to `to`.
pub fn plan(from: &Schema, to: &Schema, alterer: &dyn Alterer) -> Plan {
    plan_with(from, to, alterer, &[], &BTreeSet::new())
}

/// The plan from `from` to `to`, with the columns `renames` names (each an old column's id and a new
/// one's, in one table) renamed whatever their definitions: a rename the person writing the
/// migration states, which may change the column's type as well; and the columns `dropped` names
/// dropped, never guessed renamed.
pub fn plan_with(from: &Schema, to: &Schema, alterer: &dyn Alterer, renames_stated: &[(String, String)], dropped: &BTreeSet<String>) -> Plan {
    // The renames are decided first, once, for this planner and for the alterer, whose table
    // statements (MySQL's alter table, SQLite's rebuild) write them. A column that goes while one of
    // the same definition comes, alone in its table with that definition, is renamed, keeping its
    // rows, and the file says so above the rename; a rename the person states is renamed whatever
    // the definitions; a column the person drops is never guessed renamed.
    let column_of = |schema: &Schema, id: &String| -> Option<(String, String)> {
        let o = &schema.objects[id];
        (o.kind == Some(Kind::Column)).then(|| (o.owner.clone().unwrap_or_default(), o.def.clone()))
    };
    let gone: Vec<(String, (String, String))> = from.objects.keys().filter(|id| !to.objects.contains_key(*id)).filter_map(|id| column_of(from, id).map(|c| (id.clone(), c))).collect();
    let come: Vec<(String, (String, String))> = to.objects.keys().filter(|id| !from.objects.contains_key(*id)).filter_map(|id| column_of(to, id).map(|c| (id.clone(), c))).collect();
    // (new id, old id), and whether it was stated.
    let mut decided: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for (old_id, new_id) in renames_stated {
        let (Some(a), Some(b)) = (gone.iter().find(|(id, _)| id == old_id), come.iter().find(|(id, _)| id == new_id)) else { continue };
        if a.1 .0 == b.1 .0 {
            decided.insert(new_id.clone(), (old_id.clone(), true));
        }
    }
    // A child table's column follows its parent's, which the server renames with it.
    let mut followers: BTreeSet<String> = BTreeSet::new();
    let follow = |decided: &mut BTreeMap<String, (String, bool)>, followers: &mut BTreeSet<String>| loop {
        let mut more = Vec::new();
        for (new_id, (old_id, was_stated)) in decided.iter() {
            let Some(parent) = from.objects[old_id].owner.as_deref() else { continue };
            let (old_name, new_name) = (last_written(old_id), last_written(new_id));
            for child in from.objects.values().filter(|o| o.kind == Some(Kind::Table) && parent_of(o).as_deref() == Some(parent)) {
                let table = child.id.strip_prefix("table ").unwrap_or(&child.id);
                let (old, new) = (format!("column {table}.{old_name}"), format!("column {table}.{new_name}"));
                if !followers.contains(&new) && from.objects.contains_key(&old) && to.objects.contains_key(&new) && !from.objects.contains_key(&new) && !to.objects.contains_key(&old) {
                    more.push((new, old, *was_stated));
                }
            }
        }
        if more.is_empty() {
            break;
        }
        for (new, old, was_stated) in more {
            followers.insert(new.clone());
            decided.insert(new, (old, was_stated));
        }
    };
    follow(&mut decided, &mut followers);
    let stated: BTreeSet<String> = decided.values().filter(|(_, was_stated)| *was_stated).map(|(old, _)| old.clone()).collect();
    let taken: BTreeSet<String> = decided.keys().cloned().collect();
    for (old_id, (table, def)) in gone.iter().filter(|(id, _)| !stated.contains(id) && !dropped.contains(id)) {
        if !to.objects.contains_key(table) {
            continue;
        }
        let same: Vec<&String> = come.iter().filter(|(id, (t, d))| t == table && d == def && !taken.contains(id)).map(|(id, _)| id).collect();
        let rivals = gone.iter().filter(|(id, (t, d))| t == table && d == def && !dropped.contains(id) && !stated.contains(id)).count();
        if same.len() == 1 && rivals == 1 {
            decided.insert(same[0].clone(), (old_id.clone(), false));
        }
    }
    // Then a table where one column goes and one comes, of one family of types (numbers, strings,
    // times…): renamed, its type changed as the new one's (a change that loses values still needs
    // --drop). Across families (text to a date), they are two columns unless --rename says so.
    let tables: BTreeSet<&String> = gone.iter().map(|(_, (t, _))| t).filter(|t| to.objects.contains_key(*t)).collect();
    // Across families, the one that goes and the one that comes, which the person settles.
    let mut across: Vec<(String, String)> = Vec::new();
    for table in tables {
        let paired_old: BTreeSet<&String> = decided.values().map(|(old, _)| old).collect();
        let left: Vec<&String> = gone.iter().filter(|(id, (t, _))| t == table && !dropped.contains(id) && !stated.contains(id) && !paired_old.contains(id)).map(|(id, _)| id).collect();
        let came: Vec<&String> = come.iter().filter(|(id, (t, _))| t == table && !decided.contains_key(id)).map(|(id, _)| id).collect();
        if left.len() == 1 && came.len() == 1 {
            if family(&from.objects[left[0]].def) == family(&to.objects[came[0]].def) {
                decided.insert(came[0].clone(), (left[0].clone(), false));
            } else if parent_of(&from.objects[table.as_str()]).is_none() {
                across.push((left[0].clone(), came[0].clone()));
            }
        }
    }
    follow(&mut decided, &mut followers);
    alterer.schemas(from, to);
    alterer.decide(&decided.iter().map(|(new, (old, _))| (old.clone(), new.clone())).collect::<Vec<_>>());

    // A partition, or a table inheriting another, of a table the target keeps, which the target
    // doesn't name, was made at run time (pg_partman's, TimescaleDB's chunks): data, left as it is.
    // So is a partition of one of those, as far down as they go.
    let mut left: BTreeSet<String> = BTreeSet::new();
    loop {
        let more: Vec<&Object> = from.objects.values().filter(|o| from.live && o.kind == Some(Kind::Table) && !to.objects.contains_key(&o.id) && !left.contains(&o.id) && parent_of(o).is_some_and(|p| to.objects.contains_key(&p) || left.contains(&p))).collect();
        if more.is_empty() {
            break;
        }
        for o in more {
            left.extend(with_owned(from, &o.id).into_iter().map(String::from));
        }
    }
    // A default privilege of the live database the target doesn't name is its environment's (a
    // platform sets its own as it makes a database): left as it is.
    left.extend(from.objects.values().filter(|o| from.live && o.id.starts_with("default privileges ") && !to.objects.contains_key(&o.id)).map(|o| o.id.clone()));
    // So is an extension of the live database the target doesn't have (one a platform installs, as
    // Supabase's `pgcrypto`), with the schema it is in where the target lacks that too: a plan from a
    // live database never drops one. A migration that means to says so itself.
    for o in from.objects.values().filter(|o| from.live && o.kind == Some(Kind::Extension) && !to.objects.contains_key(&o.id)) {
        left.insert(o.id.clone());
        let schema = format!("schema {}", o.def);
        if from.objects.contains_key(&schema) && !to.objects.contains_key(&schema) {
            left.extend(with_owned(from, &schema).into_iter().map(String::from));
        }
    }
    let mut drops: BTreeSet<String> = BTreeSet::new();
    let mut creates: BTreeSet<String> = BTreeSet::new();
    let mut alters: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // Altered in a way a view, rule, policy or trigger on it blocks, and of those, made again.
    let mut blocking: BTreeSet<String> = BTreeSet::new();
    let mut remade: BTreeSet<String> = BTreeSet::new();
    for (id, a) in &from.objects {
        match to.objects.get(id) {
            None if left.contains(id) => {}
            None => {
                drops.insert(id.clone());
            }
            Some(b) if a.def != b.def => match (&b.change, alterer.alter(a, b)) {
                (Change::Alter { blocks }, Some(statements)) => {
                    let remakes = alterer.remakes(a, b);
                    if *blocks || remakes {
                        blocking.insert(id.clone());
                    }
                    if remakes {
                        remade.insert(id.clone());
                    }
                    alters.insert(id.clone(), statements);
                }
                _ => {
                    drops.insert(id.clone());
                    creates.insert(id.clone());
                }
            },
            Some(_) => {}
        }
    }
    for id in to.objects.keys() {
        if !from.objects.contains_key(id) {
            creates.insert(id.clone());
        }
    }
    // An index attached to one the plan drops goes with it: one the target keeps is made again after.
    for (id, o) in &from.objects {
        if o.kind == Some(Kind::Index) && o.goes_with.as_ref().is_some_and(|g| drops.contains(g)) && to.objects.contains_key(id) && !drops.contains(id) {
            alters.remove(id);
            drops.insert(id.clone());
            creates.insert(id.clone());
        }
    }
    // What a change makes again itself is no change of its own, and what needs it waits for that
    // change.
    let mut taken_by: BTreeMap<String, String> = BTreeMap::new();
    for id in alters.keys().cloned().collect::<Vec<_>>() {
        for along in alterer.takes_along(&from.objects[&id], &to.objects[&id]) {
            taken_by.insert(along.clone(), id.clone());
            alters.remove(&along);
            drops.remove(&along);
            creates.remove(&along);
        }
    }
    // What a change converts in place blocks as a column whose type changes does.
    let mut converted: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for id in alters.keys() {
        let columns: Vec<String> = alterer.converts(&from.objects[id], &to.objects[id]).into_iter().filter(|c| from.objects.contains_key(c) && to.objects.contains_key(c)).collect();
        for c in &columns {
            blocking.insert(c.clone());
            remade.insert(c.clone());
        }
        if !columns.is_empty() {
            converted.insert(id.clone(), columns);
        }
    }

    // Each rename's statements, where its table stays; a stated one then changes the column, under
    // its new name, as any column changes.
    let mut renames: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for (new_id, (old_id, was_stated)) in &decided {
        let table = from.objects[old_id].owner.clone().unwrap_or_default();
        if drops.contains(&table) {
            continue;
        }
        if followers.contains(new_id) {
            renames.insert(new_id.clone(), (old_id.clone(), vec![]));
            continue;
        }
        let Some(mut statements) = alterer.rename(&from.objects[old_id], &to.objects[new_id]) else { continue };
        let _ = was_stated;
        if from.objects[old_id].def != to.objects[new_id].def {
            match alterer.alter(&from.objects[old_id], &to.objects[new_id]) {
                Some(more) => statements.extend(more),
                None => continue,
            }
        }
        renames.insert(new_id.clone(), (old_id.clone(), statements));
    }
    for (new_id, (old_id, _)) in &renames {
        drops.remove(old_id);
        creates.remove(new_id);
    }

    // Whatever depends on an object dropped and created again goes and comes back with it, and so
    // does whatever a blocking change would refuse; an object whose owner is replaced is replaced.
    let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, o) in &from.objects {
        for d in &o.deps {
            dependents.entry(d.as_str()).or_default().push(id.as_str());
        }
        if let Some(owner) = &o.owner {
            dependents.entry(owner.as_str()).or_default().push(id.as_str());
        }
    }
    // What goes with an object made again goes and comes back too: a partition's index with its
    // table's.
    let mut goers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, o) in &from.objects {
        if let Some(with) = &o.goes_with {
            goers.entry(with.as_str()).or_default().push(id.as_str());
        }
    }
    // The columns whose defaults are taken off and set again around what they read.
    let mut hooks: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();
    let mut queue: Vec<String> = drops.iter().cloned().chain(blocking.iter().cloned()).collect();
    let mut seen: BTreeSet<String> = queue.iter().cloned().collect();
    while let Some(id) = queue.pop() {
        let replaced = drops.contains(&id) && creates.contains(&id);
        let gone = drops.contains(&id) && !creates.contains(&id);
        if replaced {
            for goer in goers.get(id.as_str()).into_iter().flatten() {
                if to.objects.contains_key(*goer) && !(drops.contains(*goer) && creates.contains(*goer)) {
                    alters.remove(*goer);
                    drops.insert(goer.to_string());
                    creates.insert(goer.to_string());
                    if seen.insert(goer.to_string()) {
                        queue.push(goer.to_string());
                    }
                }
            }
        }
        for dependent in dependents.get(id.as_str()).into_iter().flatten() {
            let d = &from.objects[*dependent];
            let owned = d.owner.as_deref() == Some(id.as_str());
            // What the target keeps but depends on an object it drops (a foreign key on a unique
            // constraint the target makes a primary key) goes and comes back, depending on what the
            // target has instead. What holds rows (a table, a column, a type its columns have) is
            // never made again for it: it changes in place, as its own definition says, or the
            // drop fails on the database.
            if gone {
                let holds_nothing = matches!(d.kind, Some(Kind::Constraint | Kind::Index | Kind::View | Kind::Trigger | Kind::Policy | Kind::Rule | Kind::Statistics | Kind::Function | Kind::Operator | Kind::Cast | Kind::Comment));
                if owned || !holds_nothing || drops.contains(*dependent) || alters.contains_key(*dependent) || !to.objects.contains_key(*dependent) {
                    continue;
                }
                alters.remove(*dependent);
                drops.insert(dependent.to_string());
                creates.insert(dependent.to_string());
                if seen.insert(dependent.to_string()) {
                    queue.push(dependent.to_string());
                }
                continue;
            }
            // A column whose default calls a function made again keeps its rows: the default goes
            // before the function, and comes back after.
            let generated = d.extra.iter().any(|l| l == GENERATED);
            if replaced && d.kind == Some(Kind::Column) && !owned && !generated && from.objects[&id].kind == Some(Kind::Function) && !drops.contains(*dependent) {
                if let Some(hook) = to.objects.get(*dependent).and_then(|t| alterer.unhook(d, t)) {
                    hooks.insert(dependent.to_string(), hook);
                    continue;
                }
            }
            // A blocking change refuses only views, rules, policies and triggers on it; the server
            // rebuilds an index or a constraint itself. A column of a type changed so changes
            // type too, and so blocks what is on it.
            if !replaced && d.kind == Some(Kind::Column) && !owned {
                // A generated column reading the column is made again around the change, which
                // the server refuses under it: it holds nothing of its own.
                if d.extra.iter().any(|l| l == GENERATED) && to.objects.contains_key(*dependent) && !drops.contains(*dependent) {
                    alters.remove(*dependent);
                    drops.insert(dependent.to_string());
                    creates.insert(dependent.to_string());
                    if seen.insert(dependent.to_string()) {
                        queue.push(dependent.to_string());
                    }
                    continue;
                }
                // A column of a type made again changes type, under what reads it.
                if remade.contains(&id) {
                    remade.insert(dependent.to_string());
                }
                if seen.insert(dependent.to_string()) {
                    queue.push(dependent.to_string());
                }
                continue;
            }
            // A type made again takes the functions that take or give it along, and a column's
            // new type those whose bodies read it: each is made again around the change, as what
            // is on a column is.
            let made_again = remade.contains(&id) && d.kind == Some(Kind::Function);
            if !replaced && !owned && !made_again && !matches!(d.kind, Some(Kind::View | Kind::Rule | Kind::Policy | Kind::Trigger)) {
                continue;
            }
            if !replaced && owned {
                continue;
            }
            if drops.contains(*dependent) && !creates.contains(*dependent) {
                continue;
            }
            if !to.objects.contains_key(*dependent) {
                continue;
            }
            alters.remove(*dependent);
            drops.insert(dependent.to_string());
            creates.insert(dependent.to_string());
            if seen.insert(dependent.to_string()) {
                queue.push(dependent.to_string());
            }
        }
    }

    // An object goes with its owner: a dropped table's columns need no statement of their own, and
    // a created table's inline columns neither, unless the drops depend on each other in a circle
    // that dropping one of them first breaks.
    let mut apart = drops_apart(from, &drops);
    // An object the target keeps that would go with one it drops (a sequence a dropped table's
    // column owns) is cut loose first: what ties it there goes on its own, before that drop.
    for kept in from.objects.values().filter(|o| o.goes_with.is_some() && !drops.contains(&o.id) && owners(from, &o.id).iter().any(|w| drops.contains(*w))) {
        apart.extend(from.objects.values().filter(|o| o.owner.as_deref() == Some(kept.id.as_str()) && drops.contains(&o.id)).map(|o| o.id.clone()));
    }
    let dropped_with_owner = |id: &str| !apart.contains(id) && owners(from, id).iter().any(|o| drops.contains(*o));
    // What goes with another object is made with its owner only where that one is there already:
    // a new partition takes its table's indexes then, while a partition's index under an index the
    // plan makes too is made by name, before that one attaches it.
    let created_with_owner = |id: &str| {
        let o = &to.objects[id];
        o.inline && o.owner.as_deref().is_some_and(|w| creates.contains(w)) && o.goes_with.as_deref().is_none_or(|g| !creates.contains(g))
    };

    // A column whose new default calls a function the plan makes, which reads the column, changes
    // first, the function is made, and then the default is set.
    let mut deferred: BTreeSet<String> = BTreeSet::new();
    for (id, statements) in alters.iter_mut() {
        let (Some(a), Some(b)) = (from.objects.get(id), to.objects.get(id)) else { continue };
        let calls_back = b.deps.iter().any(|d| creates.contains(d) && to.objects.get(d).is_some_and(|f| f.kind == Some(Kind::Function) && f.deps.contains(id)));
        // A default calling a function whose other overload the plan drops, which the call would
        // name too while both stand: it goes before that one is dropped, and comes back after.
        let base = |f: &str| f.split('(').next().unwrap_or(f).to_string();
        let swapped = b.deps.iter().any(|g| creates.contains(g) && to.objects.get(g).is_some_and(|g| g.kind == Some(Kind::Function)) && a.deps.iter().any(|f| f != g && drops.contains(f) && !creates.contains(f) && base(f) == base(g)));
        if b.kind != Some(Kind::Column) || !(calls_back || swapped) {
            continue;
        }
        let Some((unset, set)) = alterer.unhook(a, b) else { continue };
        let Some(set) = set.into_iter().next() else { continue };
        let Some(at) = set.find(" set default ").map(|k| set[..k + 13].to_string()) else { continue };
        let (later, now): (Vec<String>, Vec<String>) = statements.drain(..).partition(|s| s.starts_with(&at));
        *statements = now;
        hooks.insert(id.clone(), (if swapped { unset } else { vec![] }, if later.is_empty() { vec![set] } else { later }));
        deferred.insert(id.clone());
    }

    let mut ops: Vec<Op> = Vec::new();
    for id in &drops {
        if !dropped_with_owner(id) {
            ops.push(Op::Drop(id.clone()));
        }
    }
    for (new_id, (_, statements)) in &renames {
        alters.insert(new_id.clone(), statements.clone());
    }
    for id in alters.keys() {
        ops.push(Op::Alter(id.clone()));
    }
    for id in &creates {
        if !created_with_owner(id) {
            ops.push(Op::Create(id.clone()));
        }
    }
    for id in hooks.keys() {
        ops.push(Op::Unhook(id.clone()));
        ops.push(Op::Rehook(id.clone()));
    }

    let renamed_as: BTreeMap<&str, &str> = renames.iter().map(|(new, (old, _))| (old.as_str(), new.as_str())).collect();
    // The names the plan drops relations and types under, each with the object holding it.
    let named_alike = |k: Option<Kind>| matches!(k, Some(Kind::Table | Kind::View | Kind::Sequence | Kind::Index | Kind::Type));
    let name_of = |id: &str| id.split_once(' ').map(|(_, n)| n.to_string());
    let dropped_names: BTreeMap<String, String> = drops.iter().filter(|d| !creates.contains(*d) && named_alike(from.objects[*d].kind)).filter_map(|d| Some((name_of(d)?, d.clone()))).collect();
    // The attributes the plan gives each composite type.
    let mut members_made: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for id in &creates {
        let o = &to.objects[id];
        if let Some(owner) = o.owner.as_deref().filter(|w| o.kind == Some(Kind::Column) && to.objects.get(*w).is_some_and(|t| t.kind == Some(Kind::Type))) {
            members_made.entry(owner).or_default().push(id);
        }
    }
    // The edges: an op runs after every op in `after[op]`.
    let index: BTreeMap<Op, usize> = ops.iter().cloned().enumerate().map(|(k, op)| (op, k)).collect();
    let mut after: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); ops.len()];
    let find = |op: Op| index.get(&op).copied();
    // Where a created object's owner is created, or a dropped object's owner dropped, the owner's
    // op stands for it.
    let create_of = |id: &str| -> Option<usize> {
        let mut at = Some(id.to_string());
        while let Some(i) = at {
            if let Some(k) = find(Op::Create(i.clone())).or_else(|| find(Op::Alter(i.clone()))) {
                return Some(k);
            }
            at = to.objects.get(&i).filter(|o| o.inline).and_then(|o| o.owner.clone());
        }
        None
    };
    let drop_of = |id: &str| -> Option<usize> {
        let mut at = Some(id.to_string());
        while let Some(i) = at {
            if let Some(k) = find(Op::Drop(i.clone())) {
                return Some(k);
            }
            at = from.objects.get(&i).and_then(|o| o.owner.clone());
        }
        None
    };
    for (k, op) in ops.iter().enumerate() {
        match op {
            Op::Create(id) | Op::Alter(id) => {
                let o = &to.objects[id];
                // After what it depends on, and its owner, exist as the target has them; a created
                // table waits for its inline columns' dependencies too, and an object on a column
                // waits for what the column's type is (an enum changed in place).
                let mut needs: BTreeSet<&String> = o.deps.iter().collect();
                for d in &o.deps {
                    if let Some(c) = to.objects.get(d).filter(|c| c.kind == Some(Kind::Column)) {
                        needs.extend(c.deps.iter());
                    }
                }
                needs.extend(o.owner.iter());
                if matches!(op, Op::Create(_)) {
                    needs.extend(o.after.iter());
                }
                // A table, view, sequence, index or type made under a name one the plan drops holds
                // (a table's row type is a type of its name) is made after that drop.
                if matches!(op, Op::Create(_)) && named_alike(o.kind) {
                    if let Some(j) = name_of(id).and_then(|n| dropped_names.get(&n)).and_then(|d| find(Op::Drop(d.clone()))) {
                        after[k].insert(j);
                    }
                }
                // What needs an object a change makes again waits for that change.
                for d in needs.clone() {
                    if let Some(x) = taken_by.get(d.as_str()).filter(|x| *x != id) {
                        if let Some(j) = find(Op::Alter(x.clone())) {
                            after[k].insert(j);
                        }
                    }
                }
                // What needs a composite type needs the attributes the plan gives it.
                for d in needs.clone() {
                    needs.extend(members_made.get(d.as_str()).into_iter().flatten().filter(|m| **m != id));
                }
                // What is on a column a change converts comes back after it.
                for (x, columns) in &converted {
                    if x != id && o.deps.iter().any(|d| columns.contains(d)) {
                        if let Some(j) = find(Op::Alter(x.clone())) {
                            after[k].insert(j);
                        }
                    }
                }
                for c in to.objects.values() {
                    if c.inline && c.owner.as_deref() == Some(id.as_str()) && matches!(op, Op::Create(_)) && created_with_owner(&c.id) {
                        needs.extend(c.deps.iter().filter(|d| *d != id));
                    }
                }
                for d in needs {
                    // A default the column sets after the function it calls is made.
                    if deferred.contains(id) && to.objects.get(d.as_str()).is_some_and(|f| f.kind == Some(Kind::Function)) {
                        continue;
                    }
                    // A function reads a column by its name and type: one whose other properties
                    // change (a default that calls it) is there for it already.
                    let kept_column = |d: &str| from.objects.get(d).is_some_and(|c| c.kind == Some(Kind::Column)) && !remade.contains(d) && !renames.contains_key(d);
                    if o.kind == Some(Kind::Function) && kept_column(d.as_str()) {
                        continue;
                    }
                    if let Some(j) = create_of(d) {
                        if j != k {
                            after[k].insert(j);
                        }
                    }
                }
                // After its own drop, where it is replaced, or the drop of what it goes with.
                if let Some(j) = find(Op::Drop(id.clone())) {
                    after[k].insert(j);
                } else if drops.contains(id) {
                    for o in owners(from, id) {
                        if let Some(j) = find(Op::Drop(o.to_string())) {
                            after[k].insert(j);
                        }
                    }
                }
                // An alter after the changes of the columns leaving a type that holds what it
                // changes (a composite type with an attribute of an enum, a domain over it).
                if matches!(op, Op::Alter(_)) {
                    for dependent in dependents.get(id.as_str()).into_iter().flatten() {
                        let d = &from.objects[*dependent];
                        let holder = match d.kind {
                            Some(Kind::Column) => d.owner.as_deref().filter(|w| from.objects.get(*w).is_some_and(|t| t.kind == Some(Kind::Type))),
                            Some(Kind::Type) => Some(*dependent),
                            _ => None,
                        };
                        for user in holder.and_then(|h| dependents.get(h)).into_iter().flatten() {
                            let holder = holder.unwrap_or_default().to_string();
                            // One leaving for what the change makes again comes after it instead.
                            let leaves = from.objects[*user].deps.contains(&holder) && to.objects.get(*user).is_some_and(|u| !u.deps.contains(&holder) && !u.deps.contains(id) && !u.deps.iter().any(|d| taken_by.get(d) == Some(id)));
                            if let Some(j) = find(Op::Alter(user.to_string())).filter(|_| leaves) {
                                after[k].insert(j);
                            }
                        }
                    }
                }
                // An alter after the changes of what stops depending on it (a column moved to
                // another enum, before the one it leaves is made again), but for what moves to an
                // object the change makes again.
                if matches!(op, Op::Alter(_)) {
                    for dependent in dependents.get(id.as_str()).into_iter().flatten() {
                        let leaves = from.objects[*dependent].deps.contains(id) && to.objects.get(*dependent).is_some_and(|d| !d.deps.contains(id) && !d.deps.iter().any(|x| taken_by.get(x) == Some(id)));
                        if let Some(j) = find(Op::Alter(dependent.to_string())).filter(|_| leaves) {
                            after[k].insert(j);
                        }
                    }
                }
                // A type's change after the drops of its own attributes, which it is made without.
                if from.objects.get(id.as_str()).is_some_and(|t| t.kind == Some(Kind::Type)) {
                    for a in from.objects.values().filter(|a| a.owner.as_deref() == Some(id.as_str()) && a.kind == Some(Kind::Column)) {
                        if let Some(j) = find(Op::Drop(a.id.clone())) {
                            after[k].insert(j);
                        }
                    }
                }
                // A change that converts columns after the drops of what is on them.
                for c in converted.get(id.as_str()).into_iter().flatten() {
                    for dependent in dependents.get(c.as_str()).into_iter().flatten() {
                        if let Some(j) = drop_of(dependent) {
                            after[k].insert(j);
                        }
                    }
                }
                // An alter after the drops of what depends on it and is replaced (a view on a
                // column whose type changes).
                if matches!(op, Op::Alter(_)) {
                    // A sequence owned by the column blocks no change of it, and goes after the
                    // change takes the column's default off it; a function reading the column
                    // blocks only a change of its type.
                    let column_kept = from.objects.get(id.as_str()).is_some_and(|o| o.kind == Some(Kind::Column)) && !remade.contains(id.as_str());
                    for dependent in dependents.get(id.as_str()).into_iter().flatten().filter(|d| from.objects[**d].kind != Some(Kind::SequenceOwner) && !(column_kept && from.objects[**d].kind == Some(Kind::Function))) {
                        let mut reach = vec![*dependent];
                        if from.objects[*dependent].kind == Some(Kind::Column) {
                            reach.extend(dependents.get(dependent).into_iter().flatten());
                        }
                        for d in reach {
                            if let Some(j) = drop_of(d) {
                                after[k].insert(j);
                            }
                        }
                    }
                }
            }
            Op::Unhook(_) => {}
            Op::Rehook(id) => {
                // After what the default reads is back, what it read is gone, and the column's own
                // change.
                for d in &to.objects[id].deps {
                    if let Some(j) = create_of(d) {
                        after[k].insert(j);
                    }
                }
                for d in from.objects.get(id).into_iter().flat_map(|o| &o.deps) {
                    if let Some(j) = find(Op::Drop(d.clone())) {
                        after[k].insert(j);
                    }
                }
                if let Some(j) = find(Op::Alter(id.clone())) {
                    after[k].insert(j);
                }
            }
            Op::Drop(id) => {
                // After the drops of what it owns that go on their own, before it.
                for owned in with_owned(from, id).into_iter().filter(|o| *o != id.as_str()) {
                    if let Some(j) = find(Op::Drop(owned.to_string())) {
                        after[k].insert(j);
                    }
                }
                // After the drops (and the changes) of what depends on it, or on anything it owns:
                // another table's foreign key depends on this table's column.
                let reach: Vec<&str> = with_owned(from, id).into_iter().flat_map(|o| dependents.get(o).into_iter().flatten().copied()).collect();
                for dependent in reach.iter().filter(|d| !owners(from, d).contains(&id.as_str()) && **d != id.as_str()) {
                    if let Some(j) = drop_of(dependent) {
                        if j != k {
                            after[k].insert(j);
                        }
                    }
                    if let Some(j) = find(Op::Unhook(dependent.to_string())) {
                        after[k].insert(j);
                    }
                    // A dependent renamed changes under its new name.
                    let now = renamed_as.get(*dependent).map(|n| n.to_string()).unwrap_or_else(|| dependent.to_string());
                    if let Some(j) = find(Op::Alter(now)) {
                        after[k].insert(j);
                    }
                }
            }
        }
    }

    // What runs a function as it is made (a default or a generated column filling the rows, an
    // index, a check, a domain, a materialized view) waits for every function the plan makes: the
    // one it calls may call others, which no record names (a body the server keeps as text). Where
    // that function already waits on it, nothing is added.
    let made_function = |k: usize| matches!(&ops[k], Op::Create(id) | Op::Alter(id) if to.objects[id].kind == Some(Kind::Function));
    let functions: Vec<usize> = (0..ops.len()).filter(|k| made_function(*k)).collect();
    if functions.len() > 1 {
        let runs_one = |k: usize| -> bool {
            let (Op::Create(id) | Op::Alter(id)) = &ops[k] else { return false };
            let o = &to.objects[id];
            matches!(o.kind, Some(Kind::Column | Kind::Index | Kind::Constraint | Kind::Type | Kind::View)) && o.deps.iter().any(|d| to.objects.get(d).is_some_and(|f| f.kind == Some(Kind::Function)))
        };
        // Whether `from` already runs after `k`, through the edges so far.
        let waits_on = |after: &[BTreeSet<usize>], from: usize, k: usize| -> bool {
            let (mut stack, mut seen) = (vec![from], BTreeSet::new());
            while let Some(at) = stack.pop() {
                if at == k {
                    return true;
                }
                if seen.insert(at) {
                    stack.extend(after[at].iter().copied());
                }
            }
            false
        };
        for k in (0..ops.len()).filter(|k| runs_one(*k)) {
            for &f in &functions {
                if f != k && !waits_on(&after, f, k) {
                    after[k].insert(f);
                }
            }
        }
    }

    // Kahn's order, taking the ready op of the earliest kind (drops first, then changes, then
    // creates), so the plan reads in the order a person would write it.
    let rank = |op: &Op| -> (u8, i32, String) {
        match op {
            Op::Drop(id) => (0, -(from.objects[id].kind.map_or(0, |k| k as i32)), id.clone()),
            Op::Alter(id) => (1, to.objects[id].kind.map_or(0, |k| k as i32), id.clone()),
            Op::Create(id) => (1, to.objects[id].kind.map_or(0, |k| k as i32), id.clone()),
            Op::Unhook(id) => (0, -(Kind::Column as i32), id.clone()),
            Op::Rehook(id) => (1, Kind::ColumnSetting as i32, id.clone()),
        }
    };
    let mut done = vec![false; ops.len()];
    let mut order: Vec<usize> = Vec::new();
    while order.len() < ops.len() {
        let ready = (0..ops.len()).filter(|k| !done[*k] && after[*k].iter().all(|j| done[*j])).min_by_key(|k| rank(&ops[*k]));
        // A cycle: the op of the earliest kind goes first, and the shadow check says whether that held.
        let next = ready.unwrap_or_else(|| (0..ops.len()).filter(|k| !done[*k]).min_by_key(|k| rank(&ops[*k])).expect("an op left"));
        done[next] = true;
        order.push(next);
    }

    let mut plan = Plan::default();
    // A pair across families, where a rename could change the column's type in place.
    for (old_id, new_id) in &across {
        let (a, b) = (&from.objects[old_id], &to.objects[new_id]);
        if alterer.alter(a, b).is_some() {
            plan.pairs.push((old_id.clone(), last_written(new_id), definition_change(&a.def, &b.def), alterer.narrows(a, b)));
        }
    }
    // A column kept, or renamed, whose type changes so that values are lost without failing.
    for (id, a) in from.objects.iter().filter(|(_, o)| o.kind == Some(Kind::Column)) {
        let to_id = renames.iter().find(|(_, (old, _))| old == id).map(|(new, _)| new).unwrap_or(id);
        if followers.contains(to_id) {
            continue;
        }
        let Some(b) = to.objects.get(to_id) else { continue };
        if a.def != b.def && !(drops.contains(id) && creates.contains(id)) {
            if let Some(why) = alterer.narrows(a, b) {
                plan.narrowings.push((id.clone(), why));
            }
        }
    }
    for k in order {
        match &ops[k] {
            Op::Drop(id) => {
                let o = &from.objects[id];
                // A table or column dropped loses its rows, made again or not; a column whose table
                // goes is the table's loss.
                let in_lost_table = o.kind == Some(Kind::Column) && o.owner.as_ref().is_some_and(|t| drops.contains(t));
                // A foreign table's rows are its server's.
                let table = if o.kind == Some(Kind::Column) { o.owner.as_ref().and_then(|t| from.objects.get(t)) } else { Some(o) };
                let foreign = table.is_some_and(|t| t.extra.iter().any(|l| l.starts_with("shape foreign")));
                // A generated column's values are its expression's, which loses nothing.
                let generated = o.extra.iter().any(|l| l == GENERATED);
                if matches!(o.kind, Some(Kind::Table | Kind::Column)) && !in_lost_table && !foreign && !generated {
                    plan.losses.push(id.clone());
                }
                plan.statements.extend(o.drop.clone());
            }
            Op::Alter(id) => {
                if let Some((old_id, _)) = renames.get(id).filter(|(old_id, _)| !stated.contains(old_id) && !followers.contains(id)) {
                    // The guess is written down with what to write instead, should the two columns
                    // be two: the old one's drop and the new one's add.
                    let new_name = last_written(id);
                    let old_name = old_id.trim_start_matches("column ");
                    let instead: Vec<&String> = from.objects[old_id].drop.iter().chain(&to.objects[id].create).collect();
                    if instead.is_empty() {
                        plan.statements.push(format!("-- {old_name} is renamed {new_name}, keeping its rows. If they are two columns, drop the one and add the other instead."));
                    } else {
                        plan.statements.push(format!("-- {old_name} is renamed {new_name}, keeping its rows. If they are two columns, write instead:"));
                        plan.statements.extend(instead.iter().map(|s| format!("--   {};", s.trim_end())));
                    }
                    let (a, b) = (&from.objects[old_id].def, &to.objects[id].def);
                    plan.renames.push((old_id.clone(), new_name, (a != b).then(|| definition_change(a, b))));
                }
                plan.statements.extend(alters[id].iter().cloned());
            }
            Op::Create(id) => {
                let o = &to.objects[id];
                if let Some(caution) = &o.caution {
                    plan.statements.push(format!("-- {caution}"));
                }
                plan.statements.extend(o.create.iter().cloned());
            }
            Op::Unhook(id) => plan.statements.extend(hooks[id].0.iter().cloned()),
            Op::Rehook(id) => plan.statements.extend(hooks[id].1.iter().cloned()),
        }
    }
    alterer.finish(&mut plan.statements);
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoAlter;
    impl Alterer for NoAlter {
        fn alter(&self, _: &Object, _: &Object) -> Option<Vec<String>> {
            None
        }
    }

    fn object(id: &str, kind: Kind, def: &str, deps: &[&str]) -> Object {
        Object {
            id: id.into(),
            kind: Some(kind),
            def: def.into(),
            create: vec![format!("create {id} {def}")],
            drop: vec![format!("drop {id}")],
            deps: deps.iter().map(|d| d.to_string()).collect(),
            ..Object::default()
        }
    }

    #[test]
    fn a_replaced_object_takes_its_dependents_with_it_in_order() {
        let mut a = Schema::default();
        a.add(object("t", Kind::Table, "1", &[]));
        a.add(object("v", Kind::View, "x", &["t"]));
        a.add(object("w", Kind::View, "y", &["v"]));
        let mut b = a.clone();
        b.add(object("t", Kind::Table, "2", &[]));
        let plan = plan(&a, &b, &NoAlter);
        assert_eq!(plan.statements, ["drop w", "drop v", "drop t", "create t 2", "create v x", "create w y"]);
    }

    #[test]
    fn creates_follow_dependencies_whatever_their_names() {
        let a = Schema::default();
        let mut b = Schema::default();
        b.add(object("a_view", Kind::View, "x", &["z_table"]));
        b.add(object("z_table", Kind::Table, "1", &["m_type"]));
        b.add(object("m_type", Kind::Type, "e", &[]));
        assert_eq!(plan(&a, &b, &NoAlter).statements, ["create m_type e", "create z_table 1", "create a_view x"]);
    }
}
