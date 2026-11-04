//! MySQL's schema as the diff engine's objects. Every definition is the server's own rendering
//! (`show create table`, `show create view`, `show create function`, …), tidied into the way a
//! person writes it: keywords in lowercase, names quoted only where they must be, the database's
//! own name taken out (a shadow is another database on the same server, so a plan names nothing
//! with its database), and the version comments the server wraps around newer clauses opened.
//! The dependencies are the server's records where it keeps them (foreign keys,
//! `view_table_usage`, `view_routine_usage`) and a generated column's own expression; the objects
//! are those the comparator reads, with Reylo's history table left out.
//!
//! MySQL commits every DDL statement on its own, so a plan changes each table in one `alter table`
//! with a clause per change. A table is therefore one object whose definition is its whole body
//! (its options, columns, indexes and checks) and whose alterer writes that one statement; its
//! columns are objects of their own too, which carry no statement, so that the planner sees what
//! is dropped, renamed and depended on. A foreign key names another table, so it is a statement
//! of its own, after both tables are as it needs them. Views change in place with `create or
//! replace`, routines and events are dropped and created again, and a trigger, whose place among
//! its table's triggers is part of it, is replaced with every trigger after it.

use super::{Alterer, Change, Kind, Object, Schema};
use crate::catalog::HISTORY_TABLE;
use crate::db::{Db, Row};
use crate::sql::{tokenize, Token, TokenKind};
use crate::{Dialect, Error};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "''"))
}

fn backtick(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

/// What the reader knows of the server: the database's name (taken out of every definition), its
/// reserved words (names that must stay quoted), the account it reads as (a definer that is this
/// account is left out of what it writes) and the defaults a table takes when it names none.
struct Ctx {
    db: String,
    reserved: BTreeSet<String>,
    user: String,
    engine: String,
    collation: String,
}

/// How `Ctx::tidy` rewrites a text.
#[derive(Clone, Copy)]
struct Tidy {
    /// Keywords lowercased: only for the server's own renderings, where every name is quoted.
    lower: bool,
    /// Names unquoted where they need no quotes.
    unquote: bool,
    /// Runs of whitespace collapsed to one space, and a space after each comma.
    collapse: bool,
    /// Whether a name right before a parenthesis may be a stored function's, which stays quoted
    /// so that it is never read as a built-in of its name (a view's `count`); in a table's own
    /// elements it is a column with a prefix length (`name(10)`), and in a routine's head the
    /// routine's own name, since neither calls a stored function.
    calls: bool,
}

/// The built-in functions MySQL's parser reads by their names wherever a parenthesis follows, so
/// that such a name before one is always a call, even where a name is due (`create function
/// count(…)` is refused).
const PARSED_CALLS: &[&str] = &[
    "ADDDATE", "BIT_AND", "BIT_OR", "BIT_XOR", "CAST", "COUNT", "CURDATE", "CURTIME", "DATE_ADD", "DATE_SUB", "EXTRACT", "GROUP_CONCAT", "MAX", "MID", "MIN", "NOW", "POSITION", "SESSION_USER", "STD", "STDDEV",
    "STDDEV_POP", "STDDEV_SAMP", "SUBDATE", "SUBSTR", "SUBSTRING", "SUM", "SYSDATE", "SYSTEM_USER", "TRIM", "VARIANCE", "VAR_POP", "VAR_SAMP",
];

const SERVER_TEXT: Tidy = Tidy { lower: true, unquote: true, collapse: true, calls: true };
const TABLE_TEXT: Tidy = Tidy { calls: false, ..SERVER_TEXT };
const BODY_TEXT: Tidy = Tidy { lower: false, unquote: false, collapse: false, calls: true };

impl Ctx {
    /// A name as a statement writes it: bare where MySQL reads it bare as the same name.
    fn ident(&self, name: &str) -> String {
        let plain = name.bytes().next().is_some_and(|b| b.is_ascii_alphabetic() || b == b'_') && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$');
        if plain && !self.reserved.contains(&name.to_ascii_uppercase()) {
            name.to_string()
        } else {
            backtick(name)
        }
    }

    /// `text` with the database's own name taken out of every qualified name, and rewritten as
    /// `how` says; string literals and comments are kept as they are.
    fn tidy(&self, text: &str, how: Tidy) -> String {
        let text = open_version_comments(text);
        let tokens = tokenize(&text, Dialect::Mysql);
        let mut out = String::new();
        let mut at = 0;
        let mut k = 0;
        while k < tokens.len() {
            let t = &tokens[k];
            let gap = &text[at..t.start];
            if how.collapse && gap.trim().is_empty() {
                let after_comma = k > 0 && tokens[k - 1].text == "," && !matches!(t.text.as_str(), ")" | ",");
                if !gap.is_empty() || after_comma {
                    out.push(' ');
                }
            } else {
                out.push_str(gap);
            }
            let next = tokens.get(k + 1);
            let name = name_of(t);
            // `db`.`t` and db.t: the database's own name and its dot go.
            if name.as_deref() == Some(self.db.as_str()) && next.is_some_and(|n| n.text == ".") && tokens.get(k + 2).is_some_and(|n| name_of(n).is_some()) {
                k += 2;
                // The next name follows the dropped qualifier with no gap.
                let t = &tokens[k];
                out.push_str(&self.token(t, tokens.get(k + 1), how));
                at = t.end;
                k += 1;
                continue;
            }
            out.push_str(&self.token(t, next, how));
            at = t.end;
            k += 1;
        }
        out.push_str(if how.collapse { text[at..].trim_end() } else { &text[at..] });
        if how.collapse {
            out.trim().to_string()
        } else {
            out
        }
    }

    fn token(&self, t: &Token, next: Option<&Token>, how: Tidy) -> String {
        match t.kind {
            TokenKind::Quoted if t.text.starts_with('`') && how.unquote && !(next.is_some_and(|n| n.text == "(" && n.start == t.end) && (how.calls || PARSED_CALLS.contains(&name_of(t).unwrap_or_default().to_ascii_uppercase().as_str()))) => {
                let name = name_of(t).unwrap_or_default();
                self.ident(&name)
            }
            TokenKind::Word if how.lower => t.text.to_ascii_lowercase(),
            _ => t.text.clone(),
        }
    }

    /// The definer as a clause, or nothing where it is the account that reads.
    fn definer(&self, definer: &str) -> String {
        if definer.is_empty() || definer == self.user {
            return String::new();
        }
        match definer.rsplit_once('@') {
            Some((user, host)) => format!("definer = {}@{} ", backtick(user), backtick(host)),
            None => format!("definer = {} ", backtick(definer)),
        }
    }
}

/// The name a token is: a bare word, or a backquoted name unquoted.
fn name_of(t: &Token) -> Option<String> {
    match t.kind {
        TokenKind::Word => Some(t.text.clone()),
        TokenKind::Quoted if t.text.starts_with('`') && t.text.len() >= 2 => Some(t.text[1..t.text.len() - 1].replace("``", "`")),
        _ => None,
    }
}

/// The server's version comments (`/*!80023 INVISIBLE */`) opened into the clauses they hold: the
/// plan runs on the server that wrote them.
fn open_version_comments(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut copied = 0;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == b'\\' && q != b'`' {
                i += 2;
                continue;
            }
            if c == q {
                if b.get(i + 1) == Some(&q) {
                    i += 2;
                    continue;
                }
                quote = None;
            }
            i += 1;
            continue;
        }
        if matches!(c, b'\'' | b'"' | b'`') {
            quote = Some(c);
            i += 1;
            continue;
        }
        if text[i..].starts_with("/*!") {
            let mut j = i + 3;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let Some(end) = text[j..].find("*/").map(|e| j + e) else { break };
            out.push_str(&text[copied..i]);
            out.push_str(text[j..end].trim());
            i = end + 2;
            copied = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&text[copied.min(text.len())..]);
    out
}

/// `text` with the byte ranges `cut` left out, each with the whitespace before it.
fn cut(text: &str, ranges: &[(usize, usize)]) -> String {
    let mut out = String::new();
    let mut at = 0;
    for &(start, end) in ranges {
        out.push_str(text[at..start].trim_end());
        at = end;
    }
    out.push_str(&text[at..]);
    out.trim().to_string()
}

/// A column's definition, read back from its parts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Column {
    name: String,
    /// The name as a statement writes it.
    q: String,
    ty: String,
    collation: String,
    /// Everything after the type and the collation: nullability, default, generation, comment, …
    rest: String,
}

impl Column {
    /// What is compared: the whole definition but the name.
    fn def(&self) -> String {
        let mut out = self.ty.clone();
        if !self.collation.is_empty() {
            out += &format!(" collate {}", self.collation);
        }
        if !self.rest.is_empty() {
            out += &format!(" {}", self.rest);
        }
        out
    }

    /// The definition as a statement writes it, its collation left out where it is the table's.
    fn written(&self, table_collation: Option<&str>) -> String {
        let mut c = self.clone();
        if table_collation == Some(c.collation.as_str()) {
            c.collation.clear();
        }
        format!("{} {}", self.q, c.def())
    }
}

/// A table's body: what `create table` makes and `alter table` changes.
#[derive(Clone, Debug, Default)]
struct Body {
    q: String,
    /// The table options, by name (`engine`, `collate`, `comment`, `row_format`, …), as the server wrote them.
    options: Vec<(String, String)>,
    partition: String,
    columns: Vec<Column>,
    /// Indexes and checks, each `(name, name as written, definition)`.
    indexes: Vec<(String, String, String)>,
    checks: Vec<(String, String, String)>,
}

impl Body {
    fn to_json(&self) -> String {
        json!({
            "q": self.q,
            "options": self.options,
            "partition": self.partition,
            "columns": self.columns.iter().map(|c| json!([c.name, c.q, c.ty, c.collation, c.rest])).collect::<Vec<_>>(),
            "indexes": self.indexes,
            "checks": self.checks,
        })
        .to_string()
    }

    fn from_json(text: &str) -> Option<Body> {
        let v: Value = serde_json::from_str(text).ok()?;
        let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
        let triples = |v: &Value| -> Vec<(String, String, String)> { v.as_array().into_iter().flatten().map(|t| (s(&t[0]), s(&t[1]), s(&t[2]))).collect() };
        Some(Body {
            q: s(&v["q"]),
            options: v["options"].as_array().into_iter().flatten().map(|p| (s(&p[0]), s(&p[1]))).collect(),
            partition: s(&v["partition"]),
            columns: v["columns"].as_array().into_iter().flatten().map(|c| Column { name: s(&c[0]), q: s(&c[1]), ty: s(&c[2]), collation: s(&c[3]), rest: s(&c[4]) }).collect(),
            indexes: triples(&v["indexes"]),
            checks: triples(&v["checks"]),
        })
    }

    fn option(&self, key: &str) -> Option<&str> {
        self.options.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }
}

/// The schema of the database `db` is connected to. Under ANSI_QUOTES the server quotes names with
/// double quotes in what it renders, so the session reads without it while it reads.
pub async fn read(db: &mut Db) -> Result<Schema, Error> {
    let mode = db.rows("select @@session.sql_mode as m").await?.first().map(|r| r.text("m").to_string()).unwrap_or_default();
    let plain: Vec<&str> = mode.split(',').filter(|m| !matches!(*m, "ANSI_QUOTES" | "ANSI")).collect();
    let changed = plain.len() != mode.split(',').filter(|m| !m.is_empty()).count();
    if changed {
        db.exec(&format!("set session sql_mode = {}", literal(&plain.join(",")))).await?;
    }
    let result = read_objects(db).await;
    if changed {
        db.exec(&format!("set session sql_mode = {}", literal(&mode))).await?;
    }
    result
}

async fn read_objects(db: &mut Db) -> Result<Schema, Error> {
    let history = HISTORY_TABLE;
    let first = |rows: Vec<Row>, name: &str| rows.first().map(|r| r.text(name).to_string()).unwrap_or_default();
    let ctx = Ctx {
        db: first(db.rows("select database() as n").await?, "n"),
        reserved: db.rows("select word from information_schema.keywords where reserved = 1").await?.iter().map(|r| r.text("word").to_ascii_uppercase()).collect(),
        user: first(db.rows("select current_user() as u").await?, "u"),
        engine: first(db.rows("select @@default_storage_engine as e").await?, "e"),
        collation: first(db.rows("select @@collation_database as c").await?, "c"),
    };
    let mut s = Schema::default();

    let mut infos: BTreeMap<(String, String), Row> = BTreeMap::new();
    for r in db
        .rows(&format!(
            "select table_name as t, column_name as c, column_type as type, collation_name as collation, generation_expression as gen
            from information_schema.columns where table_schema = database() and table_name <> '{history}'"
        ))
        .await?
    {
        infos.insert((r.text("t").to_string(), r.text("c").to_string()), r);
    }
    let mut checks: BTreeMap<(String, String), String> = BTreeMap::new();
    for r in db
        .rows(
            "select tc.table_name as t, tc.constraint_name as n, cc.check_clause as clause, tc.enforced
            from information_schema.table_constraints tc join information_schema.check_constraints cc
              on cc.constraint_schema = tc.constraint_schema and cc.constraint_name = tc.constraint_name
            where tc.table_schema = database() and tc.constraint_type = 'CHECK'",
        )
        .await?
    {
        let clause = format!("constraint {} check ({}){}", backtick(r.text("n")), stored_text(r.text("clause")), if r.text("enforced") == "NO" { " not enforced" } else { "" });
        checks.insert((r.text("t").to_string(), r.text("n").to_string()), clause);
    }

    // Each index's parts, to know the one a foreign key made for itself.
    let mut parts: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut unique: BTreeSet<(String, String)> = BTreeSet::new();
    for r in db
        .rows(&format!(
            "select table_name as t, index_name as i, non_unique as nonunique, column_name as col, sub_part as sub, expression as expr
            from information_schema.statistics where table_schema = database() and table_name <> '{history}' order by table_name, index_name, seq_in_index"
        ))
        .await?
    {
        let key = (r.text("t").to_string(), r.text("i").to_string());
        if r.text("nonunique") == "0" {
            unique.insert(key.clone());
        }
        let part = if r.get("expr").is_some() || r.get("sub").is_some() { String::new() } else { r.text("col").to_string() };
        parts.entry(key).or_default().push(part);
    }

    // Foreign keys: their columns, the columns they reference, and their types.
    struct Fk {
        cols: Vec<String>,
        ref_schema: String,
        ref_table: String,
        refs: Vec<String>,
    }
    let mut fks: BTreeMap<(String, String), Fk> = BTreeMap::new();
    for r in db
        .rows(&format!(
            "select table_name as t, constraint_name as n, column_name as col, referenced_table_schema as rs, referenced_table_name as rt, referenced_column_name as rc
            from information_schema.key_column_usage where table_schema = database() and referenced_table_name is not null and table_name <> '{history}'
            order by table_name, constraint_name, ordinal_position"
        ))
        .await?
    {
        let fk = fks.entry((r.text("t").to_string(), r.text("n").to_string())).or_insert_with(|| Fk { cols: vec![], ref_schema: r.text("rs").into(), ref_table: r.text("rt").into(), refs: vec![] });
        fk.cols.push(r.text("col").into());
        fk.refs.push(r.text("rc").into());
    }
    // A foreign key's own index: named as the key, on exactly its columns. Adding the key makes it
    // and dropping the key leaves it, so it goes with the key rather than the table's body.
    let fk_index = |t: &str, n: &str, fk: &Fk| -> bool {
        let key = (t.to_string(), n.to_string());
        parts.get(&key).is_some_and(|p| *p == fk.cols) && !unique.contains(&key) && n != "PRIMARY"
    };
    // A key between tables that reach each other through keys: neither table can be dropped while
    // the other's key stands, so such a key is dropped by a statement of its own, before either,
    // where every other key goes with its table.
    let mut refers: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for ((t, _), fk) in &fks {
        if fk.ref_schema == ctx.db && fk.ref_table != *t {
            refers.entry(t.as_str()).or_default().insert(fk.ref_table.as_str());
        }
    }
    let reaches = |from: &str, to: &str| -> bool {
        let mut seen = BTreeSet::new();
        let mut queue = vec![from];
        while let Some(at) = queue.pop() {
            if at == to {
                return true;
            }
            if seen.insert(at) {
                queue.extend(refers.get(at).into_iter().flatten().copied());
            }
        }
        false
    };
    let in_cycle: BTreeSet<(String, String)> = fks.iter().filter(|((t, _), fk)| fk.ref_schema == ctx.db && fk.ref_table != *t && reaches(&fk.ref_table, t)).map(|(k, _)| k.clone()).collect();

    let tables = db.rows(&format!("select table_name as name, table_type as type from information_schema.tables where table_schema = database() and table_name <> '{history}' order by table_name")).await?;
    let views: BTreeSet<String> = tables.iter().filter(|r| r.text("type") == "VIEW").map(|r| r.text("name").to_string()).collect();
    for r in tables.iter().filter(|r| r.text("type") != "VIEW") {
        let name = r.text("name").to_string();
        let q = ctx.ident(&name);
        let text = first(db.rows(&format!("show create table {}", backtick(&name))).await?, "Create Table");
        let mut body = parse_table(&ctx, &text, &name, &infos, &checks);
        body.q = q.clone();
        let owned: BTreeSet<String> = fks.iter().filter(|((t, n), fk)| *t == name && fk_index(t, n, fk)).map(|((_, n), _)| n.clone()).collect();
        body.indexes.retain(|(i, _, _)| !owned.contains(i));
        let id = format!("table {name}");
        let def = body.to_json();
        // Every column is an object of its own, which carries no statement: the table's own
        // statement makes and changes it. Each carries its table's body, which a rename writes.
        for c in &body.columns {
            let mut deps = BTreeSet::new();
            if let Some(info) = infos.get(&(name.clone(), c.name.clone())) {
                for t in tokenize(info.text("gen"), Dialect::Mysql) {
                    if let Some(other) = name_of(&t).filter(|_| t.kind == TokenKind::Quoted) {
                        if other != c.name && body.column(&other).is_some() {
                            deps.insert(format!("column {name}.{other}"));
                        }
                    }
                }
            }
            s.add(Object {
                id: format!("column {name}.{}", c.name),
                kind: Some(Kind::Column),
                owner: Some(id.clone()),
                def: c.def(),
                inline: true,
                deps,
                change: Change::Alter { blocks: false },
                extra: vec![def.clone()],
                ..Object::default()
            });
        }
        s.add(Object { id: id.clone(), kind: Some(Kind::Table), create: vec![create_table(&ctx, &body)], drop: vec![format!("drop table {q}")], def, change: Change::Alter { blocks: false }, ..Object::default() });

        // Foreign keys, from the server's rendering of each.
        for element in elements(&text) {
            let tokens = tokenize(&element, Dialect::Mysql);
            if !(tokens.first().is_some_and(|t| t.upper() == "CONSTRAINT") && tokens.iter().any(|t| t.upper() == "FOREIGN")) {
                continue;
            }
            let Some(fk_name) = tokens.get(1).and_then(name_of) else { continue };
            let Some(fk) = fks.get(&(name.clone(), fk_name.clone())) else { continue };
            let clause = ctx.tidy(&element, TABLE_TEXT);
            let same_db = fk.ref_schema == ctx.db;
            let type_of = |t: &str, c: &str| infos.get(&(t.to_string(), c.to_string())).map(|r| r.text("type").to_string()).unwrap_or_default();
            // The columns' types are part of it: a key cannot stay while either side's type
            // changes, so it is dropped before the change and added after.
            let types = fk.cols.iter().map(|c| type_of(&name, c)).chain(fk.refs.iter().map(|c| if same_db { type_of(&fk.ref_table, c) } else { String::new() })).collect::<Vec<_>>().join(", ");
            let mut deps: BTreeSet<String> = fk.cols.iter().map(|c| format!("column {name}.{c}")).collect();
            let cycle = in_cycle.contains(&(name.clone(), fk_name.clone()));
            if cycle {
                deps.insert(id.clone());
            }
            if same_db {
                deps.insert(format!("table {}", fk.ref_table));
                deps.extend(fk.refs.iter().map(|c| format!("column {}.{c}", fk.ref_table)));
            }
            let fq = ctx.ident(&fk_name);
            let mut drop = format!("alter table {q} drop foreign key {fq}");
            if owned.contains(&fk_name) {
                drop += &format!(", drop index {fq}");
            }
            s.add(Object {
                id: format!("foreign key {name}.{fk_name}"),
                kind: Some(Kind::Constraint),
                owner: (!cycle).then(|| id.clone()),
                def: format!("{clause} ({types})"),
                create: vec![format!("alter table {q} add {clause}")],
                drop: vec![drop],
                deps,
                ..Object::default()
            });
        }
    }

    // Views, from the server's rendering, and what each reads.
    let mut reads: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for r in db.rows("select view_name as v, table_schema as s, table_name as t from information_schema.view_table_usage where view_schema = database()").await? {
        if r.text("s") == ctx.db {
            let t = r.text("t");
            reads.entry(r.text("v").to_string()).or_default().insert(if views.contains(t) { format!("view {t}") } else { format!("table {t}") });
        }
    }
    for r in db.rows("select table_name as v, specific_schema as s, specific_name as f from information_schema.view_routine_usage where table_schema = database()").await? {
        if r.text("s") == ctx.db {
            reads.entry(r.text("v").to_string()).or_default().insert(format!("function {}", r.text("f")));
        }
    }
    for name in &views {
        let text = first(db.rows(&format!("show create view {}", backtick(name))).await?, "Create View");
        let Some(view) = parse_view(&ctx, &text) else { continue };
        let q = ctx.ident(name);
        let create = format!("create {}view {q}{} as {}{}", view.head, view.columns, view.body, view.check);
        s.add(Object {
            id: format!("view {name}"),
            kind: Some(Kind::View),
            def: format!("{}{}{} as {}", view.def_head, view.columns, view.check, view.body),
            create: vec![create],
            drop: vec![format!("drop view {q}")],
            deps: reads.remove(name).unwrap_or_default().into_iter().filter(|d| *d != format!("view {name}")).collect(),
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
    }

    // Routines: functions and procedures, each in a namespace of its own.
    for r in db.rows("select routine_name as n, routine_type as type, routine_definition as body, definer from information_schema.routines where routine_schema = database() order by routine_name").await? {
        let (name, word) = (r.text("n"), r.text("type").to_ascii_lowercase());
        let q = ctx.ident(name);
        let column = if word == "function" { "Create Function" } else { "Create Procedure" };
        let text = first(db.rows(&format!("show create {word} {}", backtick(name))).await?, column);
        let (head, body) = split_body(&ctx, &text, r.text("body"));
        let head = head.replacen("create ", &format!("create {}", ctx.definer(r.text("definer"))), 1);
        // A body of several lines starts on a line of its own.
        let create = format!("{head}{}{body}", if body.contains('\n') { "\n" } else { " " });
        s.add(Object {
            id: format!("{word} {name}"),
            kind: Some(Kind::Function),
            def: format!("{head}\n{body}\n-- definer {}", r.text("definer")),
            create: vec![create],
            drop: vec![format!("drop {word} {q}")],
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
    }

    // Triggers, in their order on their table: each after the one before it, which it follows.
    let triggers = db
        .rows(&format!(
            "select trigger_name as n, event_manipulation as ev, event_object_table as t, action_statement as body, action_timing as timing, action_order as ord, definer
            from information_schema.triggers where trigger_schema = database() and event_object_table <> '{history}' order by event_object_table, action_timing, event_manipulation, action_order"
        ))
        .await?;
    for (k, r) in triggers.iter().enumerate() {
        let name = r.text("n");
        let q = ctx.ident(name);
        let before = k.checked_sub(1).map(|j| &triggers[j]).filter(|p| p.text("t") == r.text("t") && p.text("timing") == r.text("timing") && p.text("ev") == r.text("ev"));
        let follows = before.map(|p| format!("follows {} ", ctx.ident(p.text("n")))).unwrap_or_default();
        let body = ctx.tidy(r.text("body"), BODY_TEXT);
        let head = format!("{} {} on {} for each row {follows}", r.text("timing").to_ascii_lowercase(), r.text("ev").to_ascii_lowercase(), ctx.ident(r.text("t")));
        s.add(Object {
            id: format!("trigger {name}"),
            kind: Some(Kind::Trigger),
            owner: Some(format!("table {}", r.text("t"))),
            def: format!("{head}{body}\n-- definer {}", r.text("definer")),
            create: vec![format!("create {}trigger {q} {head}{body}", ctx.definer(r.text("definer")))],
            drop: vec![format!("drop trigger {q}")],
            deps: before.map(|p| format!("trigger {}", p.text("n"))).into_iter().collect(),
            ..Object::default()
        });
    }

    // Events. When one starts is a time the server fixes as the event is made, so two databases
    // made apart differ there however they were written: it is not compared, and it is written only
    // where the schema named a time of its own, not the moment the event was made. Nor is when a
    // one-time event runs, for the same reason.
    for r in db
        .rows(
            "select event_name as n, definer, event_definition as body, event_type as type, interval_value as iv, interval_field as field, ends, status, on_completion as oc, event_comment as comment,
              starts = created as now
            from information_schema.events where event_schema = database() order by event_name",
        )
        .await?
    {
        let name = r.text("n");
        let q = ctx.ident(name);
        let text = first(db.rows(&format!("show create event {}", backtick(name))).await?, "Create Event");
        let (head, body) = split_body(&ctx, &text, r.text("body"));
        let head = without_defaults(&head, r.flag("now"));
        let def = format!(
            "{} every {} {} ends {} {} {} comment {}\n{body}\n-- definer {}",
            r.text("type"),
            r.text("iv"),
            r.text("field"),
            r.text("ends"),
            r.text("status"),
            r.text("oc"),
            r.text("comment"),
            r.text("definer")
        );
        s.add(Object {
            id: format!("event {name}"),
            kind: Some(Kind::Trigger),
            def,
            create: vec![format!("{} {body}", head.replacen("create ", &format!("create {}", ctx.definer(r.text("definer"))), 1))],
            drop: vec![format!("drop event {q}")],
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
    }
    Ok(s)
}

/// An event's head with what it would be anyway left out: `on completion not preserve`, `enable`,
/// and, where `now` says the server chose it, when it starts.
fn without_defaults(head: &str, now: bool) -> String {
    let tokens = tokenize(head, Dialect::Mysql);
    let words: Vec<String> = tokens.iter().map(Token::upper).collect();
    let mut ranges = Vec::new();
    let mut k = 0;
    while k < tokens.len() {
        let at = |w: &[&str]| words.len() >= k + w.len() && words[k..k + w.len()].iter().zip(w).all(|(a, b)| a == b);
        let width = if at(&["ON", "COMPLETION", "NOT", "PRESERVE"]) {
            4
        } else if at(&["ENABLE"]) {
            1
        } else if now && at(&["STARTS"]) && tokens.get(k + 1).is_some_and(|t| t.kind == TokenKind::Str) {
            2
        } else {
            0
        };
        if width > 0 {
            ranges.push((tokens[k].start, tokens[k + width - 1].end));
            k += width;
        } else {
            k += 1;
        }
    }
    cut(head, &ranges)
}

/// The elements of a `create table` the server rendered: each column, index and constraint, as
/// written between its parentheses.
fn elements(text: &str) -> Vec<String> {
    let text = open_version_comments(text);
    let tokens = tokenize(&text, Dialect::Mysql);
    let Some(open) = tokens.iter().position(|t| t.text == "(") else { return vec![] };
    let mut out = Vec::new();
    let mut depth = 0;
    let mut start = None;
    for t in &tokens[open..] {
        match t.text.as_str() {
            "(" => {
                depth += 1;
                if depth == 1 {
                    continue;
                }
            }
            ")" => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        out.push(text[s..t.start].trim().to_string());
                    }
                    break;
                }
            }
            "," if depth == 1 => {
                if let Some(s) = start.take() {
                    out.push(text[s..t.start].trim().to_string());
                }
                continue;
            }
            _ => {}
        }
        if start.is_none() {
            start = Some(t.start);
        }
    }
    out
}

/// An expression as information_schema stores it (a check's clause, a generated column's
/// expression): escaped as a string's contents, and its bytes of UTF-8 read one character each.
/// `show create table` renders the same expressions with every character beyond three bytes of
/// UTF-8 replaced by `?`, so these are read from here and decoded.
fn stored_text(text: &str) -> String {
    let mut unescaped = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                unescaped.push(next);
            }
            continue;
        }
        unescaped.push(c);
    }
    if unescaped.chars().all(|c| (c as u32) < 0x100) && unescaped.chars().any(|c| (c as u32) >= 0x80) {
        if let Ok(decoded) = String::from_utf8(unescaped.chars().map(|c| c as u8).collect()) {
            return decoded;
        }
    }
    unescaped
}

/// A column's attributes with its generation expression put back as `expression` says.
fn generated_from(attributes: &str, expression: &str) -> String {
    let tokens = tokenize(attributes, Dialect::Mysql);
    let Some(k) = tokens.windows(2).position(|w| w[0].upper() == "AS" && w[1].text == "(") else { return attributes.to_string() };
    let mut depth = 0;
    for t in &tokens[k + 1..] {
        match t.text.as_str() {
            "(" => depth += 1,
            ")" => {
                depth -= 1;
                if depth == 0 {
                    return format!("{}({expression}){}", &attributes[..tokens[k + 1].start], &attributes[t.end..]);
                }
            }
            _ => {}
        }
    }
    attributes.to_string()
}

/// What follows the elements of a `create table` the server rendered: its options and partitioning.
fn after_elements(text: &str) -> String {
    let text = open_version_comments(text);
    let mut depth = 0;
    for t in tokenize(&text, Dialect::Mysql) {
        match t.text.as_str() {
            "(" => depth += 1,
            ")" => {
                depth -= 1;
                if depth == 0 {
                    return text[t.end..].to_string();
                }
            }
            _ => {}
        }
    }
    String::new()
}

/// The table's body, from `show create table`'s text and the columns' information_schema rows.
fn parse_table(ctx: &Ctx, text: &str, table: &str, infos: &BTreeMap<(String, String), Row>, checks: &BTreeMap<(String, String), String>) -> Body {
    let mut body = Body::default();
    for element in elements(text) {
        let tokens = tokenize(&element, Dialect::Mysql);
        let Some(head) = tokens.first() else { continue };
        if head.kind == TokenKind::Quoted {
            let name = name_of(head).unwrap_or_default();
            let info = infos.get(&(table.to_string(), name.clone()));
            let rest = element[head.end..].trim();
            let ty = info.map(|i| i.text("type").to_string()).unwrap_or_default();
            let after = if !ty.is_empty() && rest.len() >= ty.len() && rest[..ty.len()].eq_ignore_ascii_case(&ty) { &rest[ty.len()..] } else { rest };
            let ty = if after.len() == rest.len() { String::new() } else { ty };
            let after = match info.map(|i| i.text("gen")).filter(|g| !g.is_empty()) {
                Some(gen) => generated_from(after, &stored_text(gen)),
                None => after.to_string(),
            };
            body.columns.push(Column {
                q: ctx.ident(&name),
                name,
                ty: ctx.tidy(&ty, Tidy { collapse: false, ..TABLE_TEXT }),
                collation: info.and_then(|i| i.get("collation")).unwrap_or_default().to_string(),
                rest: column_rest(&ctx.tidy(&after, TABLE_TEXT)),
            });
            continue;
        }
        let tidy = ctx.tidy(&element, TABLE_TEXT);
        match head.upper().as_str() {
            "PRIMARY" => body.indexes.push(("PRIMARY".into(), String::new(), tidy)),
            "CONSTRAINT" => {
                if tokens.iter().any(|t| t.upper() == "FOREIGN") {
                    continue;
                }
                let name = tokens.get(1).and_then(name_of).unwrap_or_default();
                let tidy = checks.get(&(table.to_string(), name.clone())).map(|c| ctx.tidy(c, TABLE_TEXT)).unwrap_or(tidy);
                body.checks.push((name.clone(), ctx.ident(&name), tidy));
            }
            _ => {
                let Some(name) = tokens.iter().find(|t| t.kind == TokenKind::Quoted).and_then(name_of) else { continue };
                body.indexes.push((name.clone(), ctx.ident(&name), tidy));
            }
        }
    }
    // In name order, the primary key first, so that two tables of the same indexes have one body
    // whatever order they were made in.
    body.indexes.sort_by(|a, b| (a.0 != "PRIMARY", &a.0).cmp(&(b.0 != "PRIMARY", &b.0)));
    body.checks.sort();
    let close = after_elements(text);
    let tokens = tokenize(&close, Dialect::Mysql);
    let mut k = 0;
    while k < tokens.len() {
        if tokens[k].upper() == "PARTITION" && tokens.get(k + 1).is_some_and(|t| t.upper() == "BY") {
            body.partition = ctx.tidy(&close[tokens[k].start..], TABLE_TEXT);
            break;
        }
        // `KEY=VALUE`, where the key may be two words (`DEFAULT CHARSET`) and the value a list.
        let mut key = Vec::new();
        while k < tokens.len() && tokens[k].kind == TokenKind::Word && key.len() < 3 {
            key.push(tokens[k].upper());
            k += 1;
        }
        if tokens.get(k).is_some_and(|t| t.text == "=") {
            k += 1;
        }
        let Some(value) = tokens.get(k) else { break };
        let (start, mut end) = (value.start, value.end);
        if value.text == "(" {
            let mut depth = 0;
            while k < tokens.len() {
                match tokens[k].text.as_str() {
                    "(" => depth += 1,
                    ")" => depth -= 1,
                    _ => {}
                }
                end = tokens[k].end;
                if depth == 0 {
                    break;
                }
                k += 1;
            }
        }
        k += 1;
        let key = key.into_iter().filter(|w| w != "DEFAULT").collect::<Vec<_>>().join(" ").to_ascii_lowercase();
        let key = if key == "character set" { "charset".to_string() } else { key };
        if !key.is_empty() && key != "auto_increment" {
            body.options.push((key, close[start..end].to_string()));
        }
    }
    body.options.sort();
    body
}

/// A column's attributes with what the definition carries elsewhere taken out: its character set
/// and collation (the column's collation is always part of the definition, since a table's
/// default can change under it), and `default null`, which a nullable column has unsaid.
fn column_rest(text: &str) -> String {
    let tokens = tokenize(text, Dialect::Mysql);
    let mut ranges = Vec::new();
    let mut k = 0;
    while k < tokens.len() {
        let w = tokens[k].upper();
        let width = match (w.as_str(), tokens.get(k + 1).map(Token::upper).as_deref()) {
            ("CHARACTER", Some("SET")) => 3,
            ("CHARSET" | "COLLATE", _) => 2,
            ("DEFAULT", Some("NULL")) => 2,
            _ => 0,
        };
        if width > 0 && k + width <= tokens.len() {
            ranges.push((tokens[k].start, tokens[k + width - 1].end));
            k += width;
        } else {
            k += 1;
        }
    }
    cut(text, &ranges)
}

/// `create table` for the body, with what the database would choose anyway left unsaid.
fn create_table(ctx: &Ctx, body: &Body) -> String {
    let collation = body.option("collate").unwrap_or_default();
    let mut lines: Vec<String> = body.columns.iter().map(|c| c.written(Some(collation))).collect();
    lines.extend(body.indexes.iter().map(|(_, _, d)| d.clone()));
    lines.extend(body.checks.iter().map(|(_, _, d)| d.clone()));
    let mut out = format!("create table {} (\n  {}\n)", body.q, lines.join(",\n  "));
    for (key, value) in &body.options {
        let skip = match key.as_str() {
            "engine" => value.eq_ignore_ascii_case(&ctx.engine),
            "charset" => true,
            "collate" => *value == ctx.collation,
            _ => false,
        };
        if !skip {
            out += &format!(" {key}={value}");
        }
    }
    if !body.partition.is_empty() {
        out += &format!("\n{}", body.partition);
    }
    out
}

/// A view as the server rendered it, in parts.
struct View {
    /// What `create` writes before `view`: the algorithm, definer and security that are not the defaults.
    head: String,
    /// The same, every part said, for the comparison.
    def_head: String,
    columns: String,
    body: String,
    check: String,
}

fn parse_view(ctx: &Ctx, text: &str) -> Option<View> {
    let tokens = tokenize(text, Dialect::Mysql);
    let at = tokens.iter().position(|t| t.upper() == "VIEW")?;
    let word_after = |w: &str| tokens[..at].iter().position(|t| t.upper() == w).and_then(|k| tokens[k + 1..at].iter().find(|t| t.text != "=")).map(|t| t.text.to_ascii_uppercase()).unwrap_or_default();
    let algorithm = word_after("ALGORITHM");
    let security = word_after("SECURITY");
    let definer = tokens[..at].iter().position(|t| t.upper() == "DEFINER").map(|k| tokens[k + 1..at].iter().skip_while(|t| t.text == "=").take_while(|t| t.upper() != "SQL").filter_map(|t| name_of(t).or_else(|| (t.text == "@").then(|| "@".to_string()))).collect::<String>()).unwrap_or_default();
    let mut k = at + 2;
    let mut columns = String::new();
    if tokens.get(k).is_some_and(|t| t.text == "(") {
        let end = tokens[k..].iter().position(|t| t.text == ")")? + k;
        columns = format!(" {}", ctx.tidy(&text[tokens[k].start..tokens[end].end], SERVER_TEXT));
        k = end + 1;
    }
    if tokens.get(k).map(Token::upper).as_deref() != Some("AS") {
        return None;
    }
    let mut end = text.len();
    let mut check = String::new();
    let n = tokens.len();
    if n >= 4 && tokens[n - 4].upper() == "WITH" && tokens[n - 2].upper() == "CHECK" && tokens[n - 1].upper() == "OPTION" {
        end = tokens[n - 4].start;
        check = format!(" with {} check option", tokens[n - 3].text.to_ascii_lowercase());
    }
    let body = ctx.tidy(&text[tokens[k].end..end], SERVER_TEXT);
    let mut head = String::new();
    if algorithm != "UNDEFINED" && !algorithm.is_empty() {
        head += &format!("algorithm = {} ", algorithm.to_ascii_lowercase());
    }
    head += &ctx.definer(&definer);
    if security == "INVOKER" {
        head += "sql security invoker ";
    }
    Some(View { def_head: format!("algorithm={algorithm} definer={definer} security={security}"), head, columns, body, check })
}

/// A routine's or an event's `create` as the server rendered it, split into its head (tidied, its
/// definer left out) and its body (as written, the database's own name taken out).
fn split_body(ctx: &Ctx, text: &str, body: &str) -> (String, String) {
    let (head, body) = match text.strip_suffix(body).filter(|_| !body.is_empty()) {
        Some(head) => (head, body),
        None => (text, ""),
    };
    let tokens = tokenize(head, Dialect::Mysql);
    let mut ranges = Vec::new();
    // DEFINER = `user`@`host`
    if let Some(k) = tokens.iter().position(|t| t.upper() == "DEFINER") {
        let mut j = k + 1;
        if tokens.get(j).is_some_and(|t| t.text == "=") {
            j += 1;
        }
        if tokens.get(j + 1).is_some_and(|t| t.text == "@") {
            j += 2;
        }
        if let Some(end) = tokens.get(j) {
            ranges.push((tokens[k].start, end.end));
        }
    }
    let head = cut(head, &ranges);
    // The routine's own name is the one name before a parenthesis in its head.
    (ctx.tidy(&head, TABLE_TEXT), ctx.tidy(body, BODY_TEXT).trim().to_string())
}

/// The columns renamed between two bodies, `(old, new)`, by the planner's rule: a column that
/// goes while one of the same definition comes, alone in its table with that definition.
fn renames(from: &Body, to: &Body) -> Vec<(String, String)> {
    let gone: Vec<&Column> = from.columns.iter().filter(|c| to.column(&c.name).is_none()).collect();
    let come: Vec<&Column> = to.columns.iter().filter(|c| from.column(&c.name).is_none()).collect();
    let mut out = Vec::new();
    for g in &gone {
        let same: Vec<&&Column> = come.iter().filter(|c| c.def() == g.def()).collect();
        let rivals = gone.iter().filter(|c| c.def() == g.def()).count();
        if same.len() == 1 && rivals == 1 {
            out.push((g.name.clone(), same[0].name.clone()));
        }
    }
    out
}

/// `text` with the names `map` renames, outside its first parenthesis' head: an index's key parts.
fn renamed_parts(text: &str, map: &BTreeMap<String, String>) -> String {
    let tokens = tokenize(text, Dialect::Mysql);
    let Some(open) = tokens.iter().position(|t| t.text == "(") else { return text.to_string() };
    let mut out = String::new();
    let mut at = 0;
    for t in &tokens[open..] {
        if let Some(new) = name_of(t).and_then(|n| map.get(&n)) {
            out.push_str(&text[at..t.start]);
            out.push_str(new);
            at = t.end;
        }
    }
    out.push_str(&text[at..]);
    out
}

/// The statements that take a table from one body to the other: one `alter table` with a clause
/// per change, and the change of its partitioning, which MySQL takes only as a statement of its own.
fn alter_table(from: &Body, to: &Body, pairs: &[(String, String)]) -> Vec<String> {
    let pairs = pairs.to_vec();
    let map: BTreeMap<String, String> = pairs.iter().map(|(old, new)| (old.clone(), to.column(new).map(|c| c.q.clone()).unwrap_or_default())).collect();
    let old_of: BTreeMap<&str, &str> = pairs.iter().map(|(o, n)| (n.as_str(), o.as_str())).collect();
    let mut clauses = Vec::new();

    // A check that reads a renamed column is dropped and added again in the same statement, which
    // MySQL requires; an index on one follows the column by itself.
    for (name, q, def) in &from.checks {
        if to.checks.iter().find(|(n, _, _)| n == name).is_none_or(|(_, _, d)| d != def) {
            clauses.push(format!("drop check {q}"));
        }
    }
    let plain = |def: &str| !def.contains("((");
    let index_now = |def: &str| if plain(def) { renamed_parts(def, &map) } else { def.to_string() };
    for (name, q, def) in &from.indexes {
        if to.indexes.iter().find(|(n, _, _)| n == name).is_none_or(|(_, _, d)| *d != index_now(def)) {
            clauses.push(if name == "PRIMARY" { "drop primary key".to_string() } else { format!("drop index {q}") });
        }
    }
    for c in &from.columns {
        if to.column(&c.name).is_none() && !map.contains_key(&c.name) {
            clauses.push(format!("drop column {}", c.q));
        }
    }
    // A column is written without its collation where it takes the table's default, unless the
    // default changes in this statement, which MySQL applies before the columns.
    let collation = (from.option("collate") == to.option("collate")).then(|| to.option("collate").unwrap_or_default());
    // A renamed column whose definition changes too is renamed and redefined by one `change column`:
    // a later clause of the same statement cannot name it by its new name.
    for (old, new) in &pairs {
        let (was, now) = (from.column(old), to.column(new));
        match (was, now) {
            (Some(was), Some(now)) if was.def() != now.def() => clauses.push(format!("change column {} {}", was.q, now.written(collation))),
            _ => clauses.push(format!("rename column {} to {}", was.map(|c| c.q.as_str()).unwrap_or(old), now.map(|c| c.q.as_str()).unwrap_or(new))),
        }
    }
    for c in &to.columns {
        match from.column(old_of.get(c.name.as_str()).copied().unwrap_or(&c.name)) {
            Some(was) if old_of.contains_key(c.name.as_str()) || was.def() == c.def() => {}
            Some(_) => clauses.push(format!("modify column {}", c.written(collation))),
            None => clauses.push(format!("add column {}", c.written(collation))),
        }
    }
    for (name, _, def) in &to.indexes {
        if from.indexes.iter().find(|(n, _, _)| n == name).is_none_or(|(_, _, d)| index_now(d) != *def) {
            clauses.push(format!("add {def}"));
        }
    }
    for (name, _, def) in &to.checks {
        if from.checks.iter().find(|(n, _, _)| n == name).is_none_or(|(_, _, d)| d != def) {
            clauses.push(format!("add {def}"));
        }
    }
    for (key, value) in &to.options {
        if key != "charset" && from.option(key) != Some(value.as_str()) {
            clauses.push(format!("{key}={value}"));
        }
    }
    for (key, _) in &from.options {
        if to.option(key).is_none() {
            clauses.push(match key.as_str() {
                "comment" => "comment=''".to_string(),
                "key_block_size" => "key_block_size=0".to_string(),
                _ => format!("{key}=default"),
            });
        }
    }
    let mut out = Vec::new();
    match clauses.len() {
        0 => {}
        1 => out.push(format!("alter table {} {}", to.q, clauses[0])),
        _ => out.push(format!("alter table {}\n  {}", to.q, clauses.join(",\n  "))),
    }
    if from.partition != to.partition {
        out.push(if to.partition.is_empty() { format!("alter table {} remove partitioning", to.q) } else { format!("alter table {} {}", to.q, to.partition) });
    }
    out
}

#[derive(Default)]
pub struct MysqlAlterer {
    decided: super::Decided,
}

impl MysqlAlterer {
    /// The columns of a table renamed between two of its bodies: the planner's decision where it
    /// made one, else its rule.
    fn pairs(&self, table: &str, from: &Body, to: &Body) -> Vec<(String, String)> {
        self.decided.of(table).unwrap_or_else(|| renames(from, to))
    }
}

/// A MySQL column type, as information_schema writes it, for a change that may lose values.
fn type_class(def: &str) -> super::TypeClass {
    use super::TypeClass;
    let lower = def.trim().to_lowercase();
    let Some(c) = regex::Regex::new(r"^([a-z]+)(?:\(([0-9]+)(?:,([0-9]+))?\))?").unwrap().captures(&lower) else { return TypeClass::Other };
    let (p, s) = (c.get(2).and_then(|m| m.as_str().parse::<u32>().ok()), c.get(3).and_then(|m| m.as_str().parse::<u32>().ok()));
    match &c[1] {
        "tinyint" => TypeClass::Int(8),
        "smallint" => TypeClass::Int(16),
        "mediumint" => TypeClass::Int(24),
        "int" | "integer" => TypeClass::Int(32),
        "bigint" => TypeClass::Int(64),
        // A float with a scale (`double(10,2)`) keeps that many decimal places.
        "float" | "double" | "real" if s.is_some() => TypeClass::Numeric(s),
        "float" => TypeClass::Float(6),
        "double" | "real" => TypeClass::Float(15),
        "decimal" | "numeric" => TypeClass::Numeric(Some(s.unwrap_or(0))),
        "date" => TypeClass::Date,
        // A timestamp is a moment, kept in UTC; a datetime is a wall-clock time.
        "datetime" => TypeClass::Timestamp { zone: false, precision: p.unwrap_or(0) },
        "timestamp" => TypeClass::Timestamp { zone: true, precision: p.unwrap_or(0) },
        "time" => TypeClass::Time { precision: p.unwrap_or(0) },
        "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" => TypeClass::Text,
        "json" => TypeClass::Jsonb,
        _ => TypeClass::Other,
    }
}

/// The type a column's definition opens with: `decimal(10,2) unsigned` of `decimal(10,2) unsigned not null`.
fn type_of(def: &str) -> String {
    let words: Vec<&str> = def.split_whitespace().collect();
    let n = 1 + words.iter().skip(1).take_while(|w| matches!(**w, "unsigned" | "zerofill" | "signed")).count();
    words[..n.min(words.len())].join(" ")
}

impl Alterer for MysqlAlterer {
    fn narrows(&self, from: &Object, to: &Object) -> Option<String> {
        if from.kind != Some(Kind::Column) {
            return None;
        }
        let (a, b) = (type_of(&from.def), type_of(&to.def));
        let why = super::narrowing(&type_class(&a), &type_class(&b))?;
        Some(format!("changes from {a} to {b}, which {why}"))
    }

    fn decide(&self, renames: &[(String, String)]) {
        self.decided.set(renames);
    }

    /// A column renamed: the table's whole statement, with the rename among its clauses, which
    /// keeps every other change after the rename in one statement. Where a table renames several
    /// columns, the last rename carries it (the planner takes them in the order of their names),
    /// and the others none.
    fn rename(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        let (a, b) = (Body::from_json(from.extra.first()?)?, Body::from_json(to.extra.first()?)?);
        let table = to.owner.as_deref()?.strip_prefix("table ")?;
        let new = to.id.strip_prefix(&format!("column {table}."))?;
        let pairs = self.pairs(table, &a, &b);
        if !pairs.iter().any(|(_, n)| n == new) {
            return None;
        }
        let last = pairs.iter().map(|(_, n)| n.as_str()).max()?;
        Some(if new == last { alter_table(&a, &b, &pairs) } else { Vec::new() })
    }

    fn alter(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        match to.kind? {
            Kind::Table => {
                let (a, b) = (Body::from_json(&from.def)?, Body::from_json(&to.def)?);
                let pairs = self.pairs(to.id.strip_prefix("table ")?, &a, &b);
                // Where a column is renamed, the rename writes the table's statement.
                Some(if pairs.is_empty() { alter_table(&a, &b, &pairs) } else { Vec::new() })
            }
            // The table's statement changes the column.
            Kind::Column => Some(Vec::new()),
            Kind::View => Some(vec![to.create.first()?.replacen("create ", "create or replace ", 1)]),
            // A routine or an event: MySQL has no create or replace for either, and nothing holds
            // one, so it is dropped and made again in place.
            Kind::Function | Kind::Trigger if to.owner.is_none() => Some(from.drop.iter().chain(&to.create).cloned().collect()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx {
        Ctx { db: "app".into(), reserved: ["SELECT", "KEY", "ORDER"].iter().map(|s| s.to_string()).collect(), user: "root@localhost".into(), engine: "InnoDB".into(), collation: "utf8mb4_0900_ai_ci".into() }
    }

    #[test]
    fn tidies_the_servers_rendering() {
        let c = ctx();
        assert_eq!(c.tidy("select `app`.`users`.`id` AS `id`,`app`.`users`.`order` AS `order` from `app`.`users`", SERVER_TEXT), "select users.id as id, users.`order` as `order` from users");
        assert_eq!(c.tidy("`n` int NOT NULL DEFAULT '0' /*!80023 INVISIBLE */", SERVER_TEXT), "n int not null default '0' invisible");
        // A quoted name before a parenthesis is a stored function's, which a bare name is not.
        assert_eq!(c.tidy("`count`(`x`) + b'101'", SERVER_TEXT), "`count`(x) + b'101'");
        assert_eq!(c.tidy("insert into app.log values ('app.x')", BODY_TEXT), "insert into log values ('app.x')");
        assert_eq!(stored_text(r#"json_schema_valid(_utf8mb4\'{"enum":["it\\\'s","cafÃ©"]}\',`w`)"#), r#"json_schema_valid(_utf8mb4'{"enum":["it\'s","café"]}',`w`)"#);
        assert_eq!(generated_from("int GENERATED ALWAYS AS ((`n` * 2)) VIRTUAL", "(`n` * 3)"), "int GENERATED ALWAYS AS ((`n` * 3)) VIRTUAL");
        assert_eq!(column_rest("character set latin1 collate latin1_bin default null comment 'x'"), "comment 'x'");
    }

    #[test]
    fn reads_a_table_body() {
        let c = ctx();
        let text = "CREATE TABLE `t` (\n  `id` int NOT NULL,\n  `b` varchar(20) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin DEFAULT 'x',\n  PRIMARY KEY (`id`),\n  KEY `k` (`b`(4) DESC),\n  CONSTRAINT `c` CHECK ((`id` > 0))\n) ENGINE=InnoDB AUTO_INCREMENT=3 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci COMMENT='People'\n/*!50100 PARTITION BY HASH (`id`)\nPARTITIONS 2 */";
        let body = parse_table(&c, text, "t", &BTreeMap::new(), &BTreeMap::new());
        assert_eq!(body.columns.len(), 2);
        assert_eq!(body.indexes, [("PRIMARY".to_string(), String::new(), "primary key (id)".to_string()), ("k".to_string(), "k".to_string(), "key k (b(4) desc)".to_string())]);
        assert_eq!(body.checks, [("c".to_string(), "c".to_string(), "constraint c check ((id > 0))".to_string())]);
        assert_eq!(body.option("comment"), Some("'People'"));
        assert_eq!(body.option("auto_increment"), None);
        assert_eq!(body.partition, "partition by hash (id) partitions 2");
    }
}
