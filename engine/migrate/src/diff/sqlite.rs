//! SQLite's schema as the diff engine's objects. SQLite keeps the `create` text each object was
//! written with (`sqlite_schema`) and records no dependency between objects, so every definition is
//! that text rendered again token by token, as the comparator renders it, every object is created
//! again from the target's own text, and the dependencies are read from the text: a view or a
//! trigger depends on the tables and views it names, and on the columns of those tables it names;
//! an index on its table and the columns it names; a table on the tables its foreign keys
//! reference (`pragma_foreign_key_list`). A table changes in place where SQLite's `alter table` can
//! (`add column`, `drop column`, `rename column`), and otherwise by the rebuild SQLite documents:
//! the new table made under another name, the rows copied, the old one dropped, the new one renamed,
//! and the indexes, triggers and views that went with the old one made again. Dropping a table where
//! foreign keys are on deletes its rows first, which fires the actions of the tables that reference
//! it, so those tables are rebuilt with it, at any depth, referencing the new one: the file keeps
//! every row on whatever runs it, foreign keys on or off, with no pragma of its own.

use super::{Alterer, Change, Kind, Object, Schema};
use crate::catalog::HISTORY_TABLE;
use crate::db::{Db, Row};
use crate::sql::{tokenize, Token, TokenKind};
use crate::{Dialect, Error};
use std::collections::{BTreeMap, BTreeSet};

/// Separates the fields of an `extra` line, a character no SQL text holds.
const SEP: char = '\u{1f}';

/// SQLite's keywords: a name that is one is quoted where a statement writes it.
const KEYWORDS: &[&str] = &[
    "abort", "action", "add", "after", "all", "alter", "always", "analyze", "and", "as", "asc", "attach", "autoincrement", "before", "begin", "between", "by", "cascade", "case", "cast", "check", "collate",
    "column", "commit", "conflict", "constraint", "create", "cross", "current", "current_date", "current_time", "current_timestamp", "database", "default", "deferrable", "deferred", "delete", "desc",
    "detach", "distinct", "do", "drop", "each", "else", "end", "escape", "except", "exclude", "exclusive", "exists", "explain", "fail", "filter", "first", "following", "for", "foreign", "from", "full",
    "generated", "glob", "group", "groups", "having", "if", "ignore", "immediate", "in", "index", "indexed", "initially", "inner", "insert", "instead", "intersect", "into", "is", "isnull", "join", "key",
    "last", "left", "like", "limit", "match", "materialized", "natural", "no", "not", "nothing", "notnull", "null", "nulls", "of", "offset", "on", "or", "order", "others", "outer", "over", "partition",
    "plan", "pragma", "preceding", "primary", "query", "raise", "range", "recursive", "references", "regexp", "reindex", "release", "rename", "replace", "restrict", "returning", "right", "rollback",
    "row", "rows", "savepoint", "select", "set", "table", "temp", "temporary", "then", "ties", "to", "transaction", "trigger", "unbounded", "union", "unique", "update", "using", "vacuum", "values",
    "view", "virtual", "when", "where", "window", "with", "without",
];

/// The words that open a table constraint rather than a column, as the comparator reads them.
const TABLE_CONSTRAINT: &[&str] = &["constraint", "primary", "unique", "check", "foreign"];

/// SQLite compares names without regard to ASCII case, and only ASCII case.
fn fold(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// A name as a statement writes it: bare where it can be, else in double quotes.
fn quote(name: &str) -> String {
    let bare = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if bare && !KEYWORDS.contains(&fold(name).as_str()) {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// A quoted name or a string without its quotes, anything else as written.
fn unquote(t: &Token) -> String {
    if !matches!(t.kind, TokenKind::Quoted | TokenKind::Str) || t.text.len() < 2 {
        return t.text.clone();
    }
    let inner = &t.text[1..t.text.len() - 1];
    match t.text.as_bytes()[0] {
        b'"' => inner.replace("\"\"", "\""),
        b'`' => inner.replace("``", "`"),
        b'\'' => inner.replace("''", "'"),
        _ => inner.to_string(),
    }
}

/// The name a word or a quoted name says, folded; None for any other token.
fn name_of(t: &Token) -> Option<String> {
    matches!(t.kind, TokenKind::Word | TokenKind::Quoted).then(|| fold(&unquote(t)))
}

/// The text rendered token by token as the comparator renders it: names unquoted and lowercased,
/// keywords lowercased, one space between tokens, literals exact.
fn canonical(tokens: &[Token]) -> String {
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

/// `sql` with the tokens `with` answers for replaced.
fn rewrite(sql: &str, tokens: &[Token], mut with: impl FnMut(usize, &Token) -> Option<String>) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut at = 0;
    for (k, t) in tokens.iter().enumerate() {
        if let Some(text) = with(k, t) {
            out.push_str(&sql[at..t.start]);
            out.push_str(&text);
            at = t.end;
        }
    }
    out.push_str(&sql[at..]);
    out
}

/// The stored text as a statement of the plan: SQLite writes `CREATE TABLE` (or `INDEX`, `VIEW`,
/// `TRIGGER`) itself before the name, uppercase whatever the text was written in, so those words
/// are lowercased where the rest of the text is.
fn written(sql: &str) -> String {
    let tokens = tokenize(sql, Dialect::Sqlite);
    let Some(name) = name_at(&tokens) else { return sql.to_string() };
    let words = tokens[name..].iter().filter(|t| t.kind == TokenKind::Word && t.text.chars().any(|c| c.is_ascii_alphabetic()));
    let (lower, upper) = words.fold((0, 0), |(l, u), t| if t.text.chars().any(|c| c.is_ascii_lowercase()) { (l + 1, u) } else { (l, u + 1) });
    if lower < upper {
        return sql.to_string();
    }
    rewrite(sql, &tokens, |k, t| (k < name).then(|| t.text.to_lowercase()))
}

/// Where the object's name is in its stored text: after `CREATE [UNIQUE | VIRTUAL] TABLE` (or
/// `INDEX`, `VIEW`, `TRIGGER`), which is all SQLite keeps before it.
fn name_at(tokens: &[Token]) -> Option<usize> {
    let k = tokens.iter().take(4).position(|t| matches!(t.upper().as_str(), "TABLE" | "INDEX" | "VIEW" | "TRIGGER"))?;
    (k + 1 < tokens.len()).then_some(k + 1)
}

/// One part of a table's column list: a column or a table constraint.
#[derive(Clone, Debug)]
struct Part {
    tokens: Vec<Token>,
    /// As written.
    raw: String,
    /// The column's name, folded, or None for a table constraint.
    column: Option<String>,
}

impl Part {
    fn canonical(&self) -> String {
        canonical(&self.tokens)
    }
    /// The column's definition without its name.
    fn definition(&self) -> String {
        canonical(&self.tokens[1..])
    }
    /// The words outside parentheses, uppercased.
    fn words(&self) -> Vec<String> {
        let mut depth = 0;
        let mut out = Vec::new();
        for t in &self.tokens {
            match t.text.as_str() {
                "(" => depth += 1,
                ")" => depth -= 1,
                _ if depth == 0 && t.kind == TokenKind::Word => out.push(t.upper()),
                _ => {}
            }
        }
        out
    }
}

/// The parts of a table's `create` text, split at the commas of its column list's top level as the
/// comparator splits them.
fn parts(sql: &str) -> Vec<Part> {
    let tokens = tokenize(sql, Dialect::Sqlite);
    let open = tokens.iter().position(|t| t.text == "(").map_or(tokens.len(), |at| at + 1);
    let mut depth = 0;
    let mut part: Vec<Token> = Vec::new();
    let mut groups: Vec<Vec<Token>> = Vec::new();
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
            groups.push(std::mem::take(&mut part));
            continue;
        }
        part.push(t.clone());
    }
    if !part.is_empty() {
        groups.push(part);
    }
    groups
        .into_iter()
        .map(|tokens| {
            let raw = sql[tokens[0].start..tokens[tokens.len() - 1].end].to_string();
            let first = &tokens[0];
            let constraint = first.kind == TokenKind::Word && TABLE_CONSTRAINT.contains(&fold(&first.text).as_str());
            let column = (!constraint).then(|| fold(&unquote(first)));
            Part { tokens, raw, column }
        })
        .collect()
}

/// The names a text mentions, folded.
fn mentions(tokens: &[Token]) -> BTreeSet<String> {
    tokens.iter().filter_map(name_of).collect()
}

/// A column's default, read from its words: None where it has none, else whether it is a constant
/// (`add column` takes no other) and whether it is null.
fn default_of(part: &Part) -> Option<(bool, bool)> {
    let mut depth = 0;
    for (k, t) in part.tokens.iter().enumerate() {
        match t.text.as_str() {
            "(" => depth += 1,
            ")" => depth -= 1,
            _ if depth == 0 && t.upper() == "DEFAULT" => {
                let rest = &part.tokens[k + 1..];
                let value: &[Token] = match rest.first().map(|t| t.text.as_str()) {
                    Some("(") => {
                        let close = rest.iter().position(|t| t.text == ")").unwrap_or(rest.len());
                        &rest[1..close]
                    }
                    _ => {
                        let signed = rest.first().is_some_and(|t| matches!(t.text.as_str(), "+" | "-"));
                        let blob = rest.first().is_some_and(|t| t.upper() == "X") && rest.get(1).is_some_and(|t| t.kind == TokenKind::Str && t.start == rest[0].end);
                        &rest[..rest.len().min(if signed || blob { 2 } else { 1 })]
                    }
                };
                let constant = match value {
                    [t] => matches!(t.kind, TokenKind::Number | TokenKind::Str) || matches!(t.upper().as_str(), "NULL" | "TRUE" | "FALSE"),
                    [sign, n] => (matches!(sign.text.as_str(), "+" | "-") && n.kind == TokenKind::Number) || (sign.upper() == "X" && n.kind == TokenKind::Str),
                    _ => false,
                };
                return Some((constant, value.len() == 1 && value[0].upper() == "NULL"));
            }
            _ => {}
        }
    }
    None
}

/// What a table's alterer needs to know beyond its text, kept in the table's `extra`.
#[derive(Debug, Default)]
struct Info {
    /// The table's name as SQLite stores it.
    name: String,
    virtual_table: bool,
    autoincrement: bool,
    /// Whether its rows have a rowid no column of it names (no INTEGER PRIMARY KEY, not WITHOUT ROWID).
    bare_rowid: bool,
    /// Each column's flags: `a` added in place (`add column` takes it on a table with rows), `d`
    /// dropped in place, `g` generated (no insert writes it), `n` not null with nothing to fill it.
    flags: BTreeMap<String, String>,
    /// What its rebuild takes down and makes again, in the order it was made: id, what decides
    /// whether the planner replaces it itself, its drop (none where it goes with what it is on),
    /// its create, and where it stands among the schema's objects.
    goes: Vec<(String, String, String, String, usize)>,
    /// The names of its schema a rebuild's new table could take.
    taken: BTreeSet<String>,
}

impl Info {
    fn lines(&self) -> Vec<String> {
        let mut out = vec![format!("name{SEP}{}", self.name)];
        for (flag, on) in [("virtual", self.virtual_table), ("autoincrement", self.autoincrement), ("rowid", self.bare_rowid)] {
            if on {
                out.push(flag.to_string());
            }
        }
        out.extend(self.flags.iter().map(|(c, f)| format!("column{SEP}{c}{SEP}{f}")));
        out.extend(self.goes.iter().map(|(id, fingerprint, drop, create, rank)| format!("goes{SEP}{id}{SEP}{fingerprint}{SEP}{drop}{SEP}{create}{SEP}{rank}")));
        out.extend(self.taken.iter().map(|t| format!("taken{SEP}{t}")));
        out
    }

    fn of(o: &Object) -> Info {
        let mut info = Info::default();
        for line in &o.extra {
            let f: Vec<&str> = line.split(SEP).collect();
            match f[..] {
                ["name", name] => info.name = name.to_string(),
                ["virtual"] => info.virtual_table = true,
                ["autoincrement"] => info.autoincrement = true,
                ["rowid"] => info.bare_rowid = true,
                ["column", c, flags] => {
                    info.flags.insert(c.to_string(), flags.to_string());
                }
                ["goes", id, fingerprint, drop, create, rank] => info.goes.push((id.to_string(), fingerprint.to_string(), drop.to_string(), create.to_string(), rank.parse().unwrap_or(0))),
                ["taken", t] => {
                    info.taken.insert(t.to_string());
                }
                _ => {}
            }
        }
        info
    }

    fn flag(&self, column: &str, flag: char) -> bool {
        self.flags.get(column).is_some_and(|f| f.contains(flag))
    }
}

/// The schema of the database `db` is connected to.
pub async fn read(db: &mut Db) -> Result<Schema, Error> {
    let list: BTreeMap<String, Row> = db.rows("select name, type, strict, wr from pragma_table_list where schema = 'main'").await?.into_iter().map(|r| (fold(r.text("name")), r)).collect();
    let shadow = |name: &str| list.get(&fold(name)).is_some_and(|r| r.text("type") == "shadow");
    let rows: Vec<Row> = db
        .rows(&format!("select type, name, tbl_name as tbl, sql from sqlite_schema where name not like 'sqlite_%' and tbl_name <> '{HISTORY_TABLE}' and sql is not null order by rowid"))
        .await?
        .into_iter()
        .filter(|r| !shadow(r.text("tbl")))
        .collect();

    // Every table's and view's id by its folded name, and each table's columns.
    let mut relations: BTreeMap<String, String> = BTreeMap::new();
    let mut columns: BTreeMap<String, Vec<Part>> = BTreeMap::new();
    for r in &rows {
        let name = fold(r.text("name"));
        match r.text("type") {
            "table" => {
                relations.insert(name.clone(), format!("table {name}"));
                if !r.text("sql").to_ascii_uppercase().starts_with("CREATE VIRTUAL") {
                    columns.insert(name, parts(r.text("sql")).into_iter().filter(|p| p.column.is_some()).collect());
                }
            }
            "view" => {
                relations.insert(name.clone(), format!("view {name}"));
            }
            _ => {}
        }
    }
    let names: BTreeSet<String> = rows.iter().map(|r| fold(r.text("name"))).collect();
    // What a text depends on: the tables and views it names, and those tables' columns it names.
    let deps_of = |tokens: &[Token], own: &str| -> BTreeSet<String> {
        let named = mentions(tokens);
        let mut out = BTreeSet::new();
        for n in &named {
            let Some(id) = relations.get(n) else { continue };
            if id == own {
                continue;
            }
            out.insert(id.clone());
            for c in columns.get(n).into_iter().flatten() {
                let c = c.column.as_deref().unwrap_or_default();
                if named.contains(c) {
                    out.insert(format!("column {n}.{c}"));
                }
            }
        }
        out
    };

    let mut s = Schema::default();
    let mut infos: BTreeMap<String, Info> = BTreeMap::new();
    for r in &rows {
        let (name, sql) = (r.text("name"), r.text("sql"));
        let folded = fold(name);
        let tokens = tokenize(sql, Dialect::Sqlite);
        let body = name_at(&tokens).map_or(&tokens[..], |at| &tokens[at + 1..]);
        match r.text("type") {
            "table" if sql.to_ascii_uppercase().starts_with("CREATE VIRTUAL") => {
                let id = format!("table {folded}");
                infos.insert(id.clone(), Info { name: name.to_string(), virtual_table: true, ..Info::default() });
                s.add(Object { id, kind: Some(Kind::Table), def: format!("virtual {}", canonical(&tokens)), create: vec![written(sql)], drop: vec![format!("drop table {}", quote(name))], ..Object::default() });
            }
            "table" => {
                let id = format!("table {folded}");
                let table_parts = parts(sql);
                let option = list.get(&folded);
                let options = [option.is_some_and(|o| o.text("strict") == "1").then_some("strict"), option.is_some_and(|o| o.text("wr") == "1").then_some("without rowid")]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(", ");
                let lines: BTreeSet<String> = table_parts.iter().map(Part::canonical).collect();
                let def = format!("table {options}\n{}", lines.into_iter().collect::<Vec<_>>().join("\n"));
                let xinfo = db.rows(&format!("select name, type, pk, hidden from pragma_table_xinfo({})", literal(name))).await?;
                let hidden: BTreeMap<String, String> = xinfo.iter().map(|c| (fold(c.text("name")), c.text("hidden").to_string())).collect();
                let keys: Vec<&Row> = xinfo.iter().filter(|c| c.text("pk") != "0").collect();
                let wr = option.is_some_and(|o| o.text("wr") == "1");
                // A lone INTEGER PRIMARY KEY is the rowid itself.
                let alias = !wr && keys.len() == 1 && keys[0].text("type").eq_ignore_ascii_case("integer");
                let mut info = Info {
                    name: name.to_string(),
                    autoincrement: tokens.iter().any(|t| t.upper() == "AUTOINCREMENT"),
                    bare_rowid: !wr && !alias,
                    taken: names.iter().filter(|n| n.starts_with("new_")).cloned().collect(),
                    ..Info::default()
                };
                for (k, p) in table_parts.iter().enumerate() {
                    let Some(c) = &p.column else { continue };
                    let words = p.words();
                    let has = |w: &str| words.iter().any(|x| x == w);
                    let generated = matches!(hidden.get(c).map(String::as_str), Some("2" | "3"));
                    let stored = hidden.get(c).map(String::as_str) == Some("3");
                    let not_null = words.windows(2).any(|w| w[0] == "NOT" && w[1] == "NULL");
                    let default = default_of(p);
                    let filled = generated || default.is_some_and(|(_, null)| !null);
                    let addable = !has("PRIMARY") && !has("UNIQUE") && !stored && default.is_none_or(|(constant, _)| constant) && (!not_null || filled) && !(has("REFERENCES") && default.is_some_and(|(_, null)| !null));
                    // Another part that names it (a check, a key, a generated column) keeps it.
                    let named = table_parts.iter().enumerate().any(|(j, q)| j != k && mentions(&q.tokens).contains(c));
                    let droppable = !has("PRIMARY") && !has("UNIQUE") && !has("REFERENCES") && !named;
                    let mut flags = String::new();
                    for (flag, on) in [('a', addable), ('d', droppable), ('g', generated), ('n', not_null && !filled && !(alias && has("PRIMARY")))] {
                        if on {
                            flags.push(flag);
                        }
                    }
                    info.flags.insert(c.clone(), flags);
                    s.add(Object {
                        id: format!("column {folded}.{c}"),
                        kind: Some(Kind::Column),
                        owner: Some(id.clone()),
                        def: p.definition(),
                        inline: true,
                        change: Change::Alter { blocks: false },
                        extra: vec![quote(name), p.tokens[0].text.clone()],
                        ..Object::default()
                    });
                }
                let mut deps = BTreeSet::new();
                for f in db.rows(&format!("select \"table\" as t from pragma_foreign_key_list({})", literal(name))).await? {
                    if let Some(target) = relations.get(&fold(f.text("t"))).filter(|t| **t != id) {
                        deps.insert(target.clone());
                    }
                }
                infos.insert(id.clone(), info);
                s.add(Object { id, kind: Some(Kind::Table), def, create: vec![written(sql)], drop: vec![format!("drop table {}", quote(name))], deps, change: Change::Alter { blocks: false }, ..Object::default() });
            }
            kind @ ("index" | "view" | "trigger") => {
                let id = format!("{kind} {folded}");
                let on = fold(r.text("tbl"));
                let owner = match kind {
                    "view" => None,
                    _ => relations.get(&on).cloned(),
                };
                let mut deps = deps_of(body, &id);
                if let Some(owner) = &owner {
                    deps.remove(owner);
                }
                s.add(Object {
                    id,
                    kind: Some(match kind {
                        "index" => Kind::Index,
                        "view" => Kind::View,
                        _ => Kind::Trigger,
                    }),
                    owner,
                    def: canonical(&tokens),
                    create: vec![written(sql)],
                    drop: vec![format!("drop {kind} {}", quote(name))],
                    deps,
                    ..Object::default()
                });
            }
            _ => {}
        }
    }

    // What a rebuild of each table takes down and makes again. Its drop takes its own indexes and
    // triggers with it; and SQLite refuses to rename the new table into place while any view or
    // trigger names a table or view that is not there, so every view and trigger that names it, or
    // names a view taken down with it, goes too. The planner replaces some of these itself (one that
    // changes, and one that reaches a view or a virtual table that changes), so each carries its
    // definition and those of what it reaches, and the rebuild leaves the ones that differ to it.
    let reach = |id: &str| -> String {
        let o = &s.objects[id];
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut queue: Vec<String> = o.deps.iter().chain(o.owner.iter()).cloned().collect();
        while let Some(next) = queue.pop() {
            let Some(o) = s.objects.get(&next) else { continue };
            let replaced = o.kind == Some(Kind::View) || (o.change == Change::Replace && o.kind == Some(Kind::Table));
            if replaced && seen.insert(format!("{next}={}", o.def)) {
                queue.extend(o.deps.iter().chain(o.owner.iter()).cloned());
            }
        }
        format!("{}\n{}", o.def, seen.into_iter().collect::<Vec<_>>().join("\n"))
    };
    let ordered: Vec<String> = rows.iter().map(|r| format!("{} {}", r.text("type"), fold(r.text("name")))).filter(|id| s.objects.get(id).is_some_and(|o| matches!(o.kind, Some(Kind::Index | Kind::View | Kind::Trigger)))).collect();
    for (table, info) in infos.iter_mut() {
        if info.virtual_table {
            continue;
        }
        // The views it reaches, and what goes with it, each with whether it needs a drop of its own.
        let mut down: BTreeSet<String> = BTreeSet::from([table.clone()]);
        let mut goes: BTreeMap<String, bool> = BTreeMap::new();
        loop {
            let before = goes.len();
            for id in &ordered {
                let o = &s.objects[id];
                let with_owner = o.owner.as_ref().is_some_and(|owner| down.contains(owner));
                if with_owner || (matches!(o.kind, Some(Kind::View | Kind::Trigger)) && o.deps.iter().any(|d| down.contains(d))) {
                    goes.entry(id.clone()).or_insert(!with_owner);
                    if o.kind == Some(Kind::View) {
                        down.insert(id.clone());
                    }
                }
            }
            if goes.len() == before {
                break;
            }
        }
        for (rank, id) in ordered.iter().enumerate().filter(|(_, id)| goes.contains_key(*id)) {
            let o = &s.objects[id];
            let drop = if goes[id] { o.drop[0].clone() } else { String::new() };
            info.goes.push((id.clone(), reach(id), drop, o.create[0].clone(), rank));
        }
    }
    for (id, info) in infos {
        if let Some(o) = s.objects.get_mut(&id) {
            o.extra = info.lines();
        }
    }
    Ok(s)
}

#[derive(Default)]
pub struct SqliteAlterer {
    decided: super::Decided,
    schemas: std::cell::RefCell<Option<(Schema, Schema)>>,
}

/// The affinity of a SQLite column's declared type, by SQLite's rules (datatype3.html, 3.1), as far as
/// a change between two affinities can lose values.
fn affinity(def: &str) -> super::TypeClass {
    use super::TypeClass;
    const ENDS: [&str; 11] = ["CONSTRAINT", "PRIMARY", "NOT", "NULL", "UNIQUE", "CHECK", "DEFAULT", "COLLATE", "REFERENCES", "GENERATED", "AS"];
    let declared: Vec<&str> = def.split_whitespace().take_while(|w| !ENDS.contains(&w.to_ascii_uppercase().as_str())).collect();
    let t = declared.join(" ").to_ascii_uppercase();
    if t.contains("INT") {
        TypeClass::Int(64)
    } else if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") || t.is_empty() || t.contains("BLOB") {
        TypeClass::Text
    } else if t.contains("REAL") || t.contains("FLOA") || t.contains("DOUB") {
        TypeClass::Float(15)
    } else {
        TypeClass::Numeric(None)
    }
}

impl Alterer for SqliteAlterer {
    fn narrows(&self, from: &Object, to: &Object) -> Option<String> {
        if from.kind != Some(Kind::Column) {
            return None;
        }
        // Only text turns into numbers: an INTEGER column keeps 1.5 as it is, and so on.
        let from_affinity = affinity(&from.def);
        if from_affinity != super::TypeClass::Text {
            return None;
        }
        let why = super::narrowing(&from_affinity, &affinity(&to.def))?;
        let declared = |def: &str| def.split_whitespace().next().unwrap_or("none").to_lowercase();
        Some(format!("changes from {} to {}, which {why}", declared(&from.def), declared(&to.def)))
    }

    fn schemas(&self, from: &Schema, to: &Schema) {
        *self.schemas.borrow_mut() = Some((from.clone(), to.clone()));
    }

    fn decide(&self, renames: &[(String, String)]) {
        self.decided.set(renames);
    }

    fn rename(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        let (table, old, new) = (from.extra.first()?, from.extra.get(1)?, to.extra.get(1)?);
        Some(vec![format!("alter table {table} rename column {old} to {new}")])
    }

    fn alter(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        match to.kind? {
            // A column changes with its table, whose alter sees the whole of both.
            Kind::Column => Some(vec![]),
            Kind::Table => alter_table(from, to, self.decided.of(to.id.strip_prefix("table ")?), self.schemas.borrow().as_ref()),
            _ => None,
        }
    }
}

/// The statements that take table `from` to `to`: `alter table` where SQLite can, else a rebuild.
/// A column the planner renames is renamed after this, so the statements write it by its old name.
fn alter_table(from: &Object, to: &Object, decided: Option<Vec<(String, String)>>, schemas: Option<&(Schema, Schema)>) -> Option<Vec<String>> {
    let (a, b) = (Info::of(from), Info::of(to));
    if a.virtual_table || b.virtual_table {
        return None;
    }
    let (old_sql, new_sql) = (from.create.first()?, to.create.first()?);
    let old_parts = parts(old_sql);
    let columns = |ps: &[Part]| -> Vec<(String, String)> { ps.iter().filter_map(|p| Some((p.column.clone()?, p.definition()))).collect() };
    let (old_columns, new_columns) = (columns(&old_parts), columns(&parts(new_sql)));

    // The renames, found as the planner finds them: a column that goes while one of its definition
    // comes, alone in its table.
    let gone: Vec<&(String, String)> = old_columns.iter().filter(|(c, _)| !new_columns.iter().any(|(n, _)| n == c)).collect();
    let come: Vec<&(String, String)> = new_columns.iter().filter(|(c, _)| !old_columns.iter().any(|(n, _)| n == c)).collect();
    let mut renamed: BTreeMap<String, String> = BTreeMap::new();
    match decided {
        // The planner's decision, which may rename what its rule alone would not.
        Some(pairs) => renamed.extend(pairs.into_iter().map(|(old, new)| (new, old))),
        None => {
            for (old, def) in &gone {
                let same: Vec<&String> = come.iter().filter(|(_, d)| d == def).map(|(c, _)| c).collect();
                if same.len() == 1 && gone.iter().filter(|(_, d)| d == def).count() == 1 {
                    renamed.insert(same[0].clone(), old.clone());
                }
            }
        }
    }
    let written_as: BTreeMap<String, String> = old_parts.iter().filter_map(|p| Some((p.column.clone()?, p.tokens[0].text.clone()))).collect();
    let target = before_renames(new_sql, &renamed, &written_as);
    let new_parts = parts(&target);
    let new_flags: BTreeMap<String, String> = b.flags.iter().map(|(c, f)| (renamed.get(c).cloned().unwrap_or_else(|| c.clone()), f.clone())).collect();
    let flag = |c: &str, f: char| new_flags.get(c).is_some_and(|x| x.contains(f));

    let constraints = |ps: &[Part]| -> BTreeSet<String> { ps.iter().filter(|p| p.column.is_none()).map(Part::canonical).collect() };
    let options = |def: &str| def.lines().next().unwrap_or_default().to_string();
    let find = |ps: &'_ [Part], c: &str| ps.iter().find(|p| p.column.as_deref() == Some(c)).cloned();
    let mut rebuild = options(&from.def) != options(&to.def) || constraints(&old_parts) != constraints(&new_parts);
    let mut drops = Vec::new();
    let mut adds = Vec::new();
    for p in old_parts.iter().filter(|p| p.column.is_some()) {
        let c = p.column.as_deref().unwrap_or_default();
        match find(&new_parts, c) {
            Some(q) if q.canonical() != p.canonical() => rebuild = true,
            Some(_) => {}
            None if a.flag(c, 'd') => drops.push(p.tokens[0].text.clone()),
            None => rebuild = true,
        }
    }
    for p in new_parts.iter().filter(|p| p.column.is_some()) {
        let c = p.column.as_deref().unwrap_or_default();
        if find(&old_parts, c).is_none() {
            if flag(c, 'a') {
                adds.push(p.raw.clone());
            } else {
                rebuild = true;
            }
        }
    }
    let table = quote(&a.name);
    if !rebuild {
        let mut out: Vec<String> = drops.iter().map(|c| format!("alter table {table} drop column {c}")).collect();
        out.extend(adds.iter().map(|p| format!("alter table {table} add column {p}")));
        return Some(out);
    }

    // The tables that reference it, at any depth, as they are on both sides, parents first.
    let children: Vec<(&Object, &Object)> = schemas.map_or_else(Vec::new, |(f, t)| children_of(f, t, &from.id));
    let family: Vec<(Info, Info)> = std::iter::once((Info::of(from), Info::of(to))).chain(children.iter().map(|(x, y)| (Info::of(x), Info::of(y)))).collect();

    // What goes with them and the planner leaves be: the same on both sides.
    let kept = |x: &(String, String, String, String, usize), other: &Info| other.goes.iter().any(|y| y.0 == x.0 && y.1 == x.1);
    let mut going: BTreeMap<String, &(String, String, String, String, usize)> = BTreeMap::new();
    let mut coming: BTreeMap<String, &(String, String, String, String, usize)> = BTreeMap::new();
    for (x, y) in &family {
        for item in x.goes.iter().filter(|i| !i.2.is_empty() && kept(i, y)) {
            going.entry(item.0.clone()).or_insert(item);
        }
        for item in y.goes.iter().filter(|i| kept(i, x)) {
            coming.entry(item.0.clone()).or_insert(item);
        }
    }
    let by_rank = |items: BTreeMap<String, &(String, String, String, String, usize)>| {
        let mut items: Vec<(usize, String)> = items.values().map(|i| (i.4, i.3.clone())).collect();
        items.sort();
        items
    };
    let mut going: Vec<(usize, String)> = going.values().map(|i| (i.4, i.2.clone())).collect();
    going.sort();
    let mut out: Vec<String> = going.into_iter().rev().map(|(_, drop)| drop).collect();
    for p in &new_parts {
        let Some(c) = p.column.as_deref() else { continue };
        if find(&old_parts, c).is_none() && flag(c, 'n') {
            out.push(format!("-- {} is not null with no default, so the copy fails on a table that has rows: give it a default, or add it nullable, fill it, then make it not null", p.tokens[0].text));
        }
    }
    // Each table's new name, and the tables that reference one of them referencing that.
    let mut scratches: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (x, y) in &family {
        let name = (1..)
            .map(|n| if n == 1 { format!("new_{}", x.name) } else { format!("new_{}_{n}", x.name) })
            .find(|n| !x.taken.contains(&fold(n)) && !y.taken.contains(&fold(n)) && !scratches.values().any(|(s, _)| fold(s) == fold(n)))
            .unwrap_or_default();
        let quoted = quote(&name);
        scratches.insert(fold(&x.name), (name, quoted));
    }
    let scratch_of = |name: &str| scratches.get(&fold(name)).map(|(_, q)| q.clone()).unwrap_or_default();
    let renamed_text = |sql: &str, own: &str| -> Option<String> {
        let tokens = tokenize(sql, Dialect::Sqlite);
        let at = name_at(&tokens)?;
        let references: BTreeSet<usize> = tokens.iter().enumerate().filter(|(_, t)| t.upper() == "REFERENCES").map(|(k, _)| k + 1).collect();
        Some(rewrite(sql, &tokens, |k, t| {
            if k == at {
                return Some(scratch_of(own));
            }
            if references.contains(&k) {
                return scratches.get(&name_of(t)?).map(|(_, q)| q.clone());
            }
            None
        }))
    };
    out.push(renamed_text(&target, &a.name)?);
    for ((x, _), (child, _)) in family[1..].iter().zip(&children) {
        out.push(renamed_text(child.create.first()?, &x.name)?);
    }
    // The columns both have, but those the new table generates; and the rowid where no column is it.
    let copied: Vec<String> = new_parts
        .iter()
        .filter(|p| p.column.as_deref().is_some_and(|c| find(&old_parts, c).is_some() && !flag(c, 'g')))
        .map(|p| p.tokens[0].text.clone())
        .collect();
    let named: Vec<String> = old_columns.iter().chain(new_columns.iter()).map(|(c, _)| c.clone()).collect();
    copy_rows(&mut out, &a, &b, &scratches[&fold(&a.name)].0, copied, &named);
    for ((x, y), (child, _)) in family[1..].iter().zip(&children) {
        let child_parts = parts(child.create.first()?);
        let copied: Vec<String> = child_parts.iter().filter(|p| p.column.as_deref().is_some_and(|c| !x.flag(c, 'g'))).map(|p| p.tokens[0].text.clone()).collect();
        let named: Vec<String> = child_parts.iter().filter_map(|p| p.column.clone()).collect();
        copy_rows(&mut out, x, y, &scratches[&fold(&x.name)].0, copied, &named);
    }
    // Children first, so that no drop has a row left to fire an action on.
    for (x, _) in family.iter().rev() {
        out.push(format!("drop table {}", quote(&x.name)));
    }
    for (x, _) in &family {
        out.push(format!("alter table {} rename to {}", scratch_of(&x.name), quote(&x.name)));
    }
    out.extend(by_rank(coming).into_iter().map(|(_, create)| create));
    Some(out)
}

/// The tables of `from` that reference the table `id`, at any depth, parents first, each with itself
/// in `to`: one the plan drops is no part of a rebuild, since the plan drops it first.
fn children_of<'a>(from: &'a Schema, to: &'a Schema, id: &str) -> Vec<(&'a Object, &'a Object)> {
    let mut seen: Vec<String> = vec![id.to_string()];
    let mut out = Vec::new();
    let mut k = 0;
    while k < seen.len() {
        let parent = seen[k].clone();
        k += 1;
        for o in from.objects.values().filter(|o| o.kind == Some(Kind::Table) && o.deps.contains(&parent) && !Info::of(o).virtual_table) {
            let Some(there) = to.objects.get(&o.id) else { continue };
            if !seen.contains(&o.id) {
                seen.push(o.id.clone());
                out.push((o, there));
            }
        }
    }
    out
}

/// What copies a table's rows into its rebuild, named `scratch`: the columns `copied` names, its
/// rowid where no column is it, and AUTOINCREMENT's counter, which can be past every row's key.
fn copy_rows(out: &mut Vec<String>, a: &Info, b: &Info, scratch: &str, mut copied: Vec<String>, named: &[String]) {
    let (table, quoted) = (quote(&a.name), quote(scratch));
    if a.bare_rowid && b.bare_rowid {
        if let Some(rowid) = ["rowid", "_rowid_", "oid"].into_iter().find(|n| !named.iter().any(|c| c == n)) {
            copied.insert(0, rowid.to_string());
        }
    }
    if !copied.is_empty() {
        let list = copied.join(", ");
        out.push(format!("insert into {quoted} ({list}) select {list} from {table}"));
    }
    if a.autoincrement && b.autoincrement {
        out.push(format!("delete from sqlite_sequence where name = {}", literal(scratch)));
        out.push(format!("insert into sqlite_sequence (name, seq) select {}, seq from sqlite_sequence where name = {}", literal(scratch), literal(&a.name)));
    }
}

/// A table's text with each column the planner renames written by its old name, as the table is
/// until the rename runs: in the column list, but not where a foreign key names another table's
/// columns, nor a function.
fn before_renames(sql: &str, renamed: &BTreeMap<String, String>, written_as: &BTreeMap<String, String>) -> String {
    if renamed.is_empty() {
        return sql.to_string();
    }
    let tokens = tokenize(sql, Dialect::Sqlite);
    let open = tokens.iter().position(|t| t.text == "(").unwrap_or(tokens.len());
    let mut skip = BTreeSet::new();
    for (k, t) in tokens.iter().enumerate() {
        if t.upper() == "REFERENCES" {
            let mut j = k + 2;
            if tokens.get(j).is_some_and(|t| t.text == "(") {
                while j < tokens.len() && tokens[j].text != ")" {
                    skip.insert(j);
                    j += 1;
                }
            }
        }
    }
    rewrite(sql, &tokens, |k, t| {
        if k <= open || skip.contains(&k) || tokens.get(k + 1).is_some_and(|n| n.text == "(") || tokens.get(k.wrapping_sub(1)).is_some_and(|p| p.text == ".") {
            return None;
        }
        let old = renamed.get(&name_of(t)?)?;
        Some(written_as.get(old).cloned().unwrap_or_else(|| quote(old)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_as_the_comparator_does() {
        assert_eq!(canonical(&tokenize("CREATE INDEX \"I\" ON t ( a , b )", Dialect::Sqlite)), "create index i on t (a, b)");
    }

    #[test]
    fn splits_a_table_into_columns_and_constraints() {
        let p = parts("CREATE TABLE t (id integer primary key, \"Full Name\" text check (length(\"Full Name\") > 0), constraint k unique (id, \"Full Name\"))");
        assert_eq!(p.iter().map(|p| p.column.clone()).collect::<Vec<_>>(), [Some("id".into()), Some("full name".into()), None]);
        assert_eq!(p[1].definition(), "text check (length (full name) > 0)");
    }

    #[test]
    fn reads_whether_a_default_is_a_constant() {
        let d = |text: &str| default_of(&parts(&format!("create table t ({text})"))[0]);
        assert_eq!(d("a int"), None);
        assert_eq!(d("a int default 1"), Some((true, false)));
        assert_eq!(d("a int default -1 not null"), Some((true, false)));
        assert_eq!(d("a blob default x'00'"), Some((true, false)));
        assert_eq!(d("a int default null"), Some((true, true)));
        assert_eq!(d("a int default (1)"), Some((true, false)));
        assert_eq!(d("a text default current_timestamp"), Some((false, false)));
        assert_eq!(d("a text default ('a' || 'b')"), Some((false, false)));
    }

    #[test]
    fn writes_a_table_by_its_columns_old_names() {
        let renamed = BTreeMap::from([("name".to_string(), "full_name".to_string())]);
        let written_as = BTreeMap::from([("full_name".to_string(), "full_name".to_string())]);
        let sql = "CREATE TABLE name (id integer primary key, name text check (length(name) > 0), boss text references people (name))";
        assert_eq!(before_renames(sql, &renamed, &written_as), "CREATE TABLE name (id integer primary key, full_name text check (length(full_name) > 0), boss text references people (name))");
    }

    #[test]
    fn lowercases_what_sqlite_writes_before_the_name() {
        assert_eq!(written("CREATE TABLE t (a int)"), "create table t (a int)");
        assert_eq!(written("CREATE UNIQUE INDEX I ON T (A)"), "CREATE UNIQUE INDEX I ON T (A)");
        assert_eq!(quote("my table"), "\"my table\"");
        assert_eq!(quote("order"), "\"order\"");
        assert_eq!(quote("users"), "users");
    }
}
