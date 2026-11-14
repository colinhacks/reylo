//! DuckDB's schema, as the comparator's lines (`catalog`) and the diff engine's objects. DuckDB
//! writes each definition again from what its catalog holds, whatever it was written as: a table's
//! columns (`duckdb_columns()`), its constraints (`duckdb_constraints()`), an index's, a view's and
//! a sequence's statement. Those renderings are the definitions. A macro's parameters' defaults are
//! in no catalog function, only in what `EXPORT DATABASE` writes, so they are read from the export
//! of a copy of the schema where one can be made, and otherwise left out of its definition.
//!
//! DuckDB changes a table in place only so far: an explicit index on it, or another table's foreign
//! key referencing it, refuses a column's drop, rename, change of type or of nullability; a column in
//! a constraint keeps its type; no check or foreign key is added, and no constraint dropped. So a
//! table changes by `alter table` where DuckDB takes it (its own indexes dropped first and made again
//! after, where they would refuse it), and otherwise by a rebuild: its rows copied to a temporary
//! table, the table dropped and made again as the target has it, the rows copied back, and its
//! indexes and comments made again. A table other tables reference cannot be dropped while they do,
//! so they are rebuilt with it, at any depth, each dropped before and made after what it references.

use super::{Alterer, Change, Kind, Object, Schema};
use crate::catalog::HISTORY_TABLE;
use crate::db::Db;
use crate::sql::{tokenize, Token, TokenKind};
use crate::{Dialect, Error};
use std::collections::{BTreeMap, BTreeSet};

/// Separates the fields of an `extra` line, a character no SQL text holds.
const SEP: char = '\u{1f}';

/// DuckDB's keywords but its unreserved ones (`duckdb_keywords()`): a name that is one is quoted.
const KEYWORDS: &[&str] = &[
    "all", "analyse", "analyze", "and", "anti", "any", "array", "as", "asc", "asof", "asymmetric", "at", "authorization", "between", "bigint", "binary", "bit", "boolean", "both", "by", "case", "cast", "char",
    "character", "check", "coalesce", "collate", "collation", "column", "columns", "concurrently", "constraint", "create", "cross", "dec", "decimal", "default", "deferrable", "desc", "describe", "distinct",
    "do", "else", "end", "except", "exists", "extract", "false", "fetch", "float", "for", "foreign", "freeze", "from", "full", "generated", "glob", "group", "grouping", "grouping_id", "having", "ilike", "in",
    "initially", "inner", "inout", "int", "integer", "intersect", "interval", "into", "is", "isnull", "join", "lambda", "lateral", "leading", "left", "like", "limit", "map", "national", "natural", "nchar",
    "none", "not", "notnull", "null", "nullif", "numeric", "offset", "on", "only", "or", "order", "out", "outer", "overlaps", "overlay", "pivot", "pivot_longer", "pivot_wider", "placing", "position",
    "positional", "precision", "primary", "qualify", "real", "references", "returning", "right", "row", "select", "semi", "setof", "show", "similar", "smallint", "some", "struct", "substring",
    "summarize", "symmetric", "table", "tablesample", "then", "time", "timestamp", "to", "trailing", "treat", "trim", "true", "try_cast", "tuple", "union", "unique", "unpack", "unpivot", "using", "values",
    "varchar", "variadic", "verbose", "when", "where", "window", "with", "xmlattributes", "xmlconcat", "xmlelement", "xmlexists", "xmlforest", "xmlnamespaces", "xmlparse", "xmlpi", "xmlroot",
    "xmlserialize", "xmltable",
];

/// Every one of DuckDB's keywords (`duckdb_keywords()`), which DuckDB quotes wherever it writes one
/// as a name: written bare and uppercase in its renderings, each is a keyword, which a statement of
/// the plan writes in lowercase.
const ALL_KEYWORDS: &[&str] = &[
    "abort", "absolute", "access", "action", "add", "admin", "after", "aggregate", "all", "also", "alter", "always", "analyse", "analyze", "and", "anti", "any", "approx", "array", "as", "asc",
    "asof", "assertion", "assignment", "asymmetric", "at", "attach", "attribute", "authorization", "backward", "before", "begin", "between", "bigint", "binary", "bit", "boolean", "both", "by",
    "cache", "call", "called", "cascade", "cascaded", "case", "cast", "catalog", "centuries", "century", "chain", "char", "character", "characteristics", "check", "checkpoint", "class", "close",
    "cluster", "coalesce", "collate", "collation", "column", "columns", "comment", "comments", "commit", "committed", "compression", "concurrently", "configuration", "conflict", "connect",
    "connection", "constraint", "constraints", "content", "continue", "conversion", "copy", "cost", "create", "cross", "csv", "cube", "current", "cursor", "cycle", "data", "database", "day", "days",
    "deallocate", "dec", "decade", "decades", "decimal", "declare", "default", "defaults", "deferrable", "deferred", "definer", "delete", "delimiter", "delimiters", "depends", "desc", "describe",
    "destroy", "detach", "dictionary", "disable", "discard", "disconnect", "distance", "distinct", "do", "document", "domain", "double", "drop", "each", "else", "enable", "encoding", "encrypted",
    "end", "enum", "error", "escape", "event", "exact", "except", "exclude", "excluding", "exclusive", "execute", "exists", "explain", "export", "export_state", "extension", "extensions", "external",
    "extract", "false", "family", "fetch", "filter", "first", "float", "following", "for", "force", "foreign", "forward", "freeze", "from", "full", "function", "functions", "generated", "glob",
    "global", "grant", "granted", "group", "grouping", "grouping_id", "groups", "handler", "having", "header", "hold", "hour", "hours", "identity", "if", "ignore", "ilike", "immediate", "immutable",
    "implicit", "import", "in", "include", "including", "increment", "index", "indexes", "inherit", "inherits", "initially", "inline", "inner", "inout", "input", "insensitive", "insert", "install",
    "instead", "int", "integer", "intersect", "interval", "into", "invoker", "is", "isnull", "isolation", "join", "json", "key", "label", "lambda", "language", "large", "last", "lateral", "leading",
    "leakproof", "left", "level", "like", "limit", "listen", "load", "local", "location", "lock", "locked", "logged", "macro", "map", "mapping", "match", "matched", "materialized", "maxvalue",
    "merge", "method", "microsecond", "microseconds", "millennia", "millennium", "millisecond", "milliseconds", "minute", "minutes", "minvalue", "mode", "month", "months", "move", "name", "names",
    "national", "natural", "nchar", "nearest", "new", "next", "no", "none", "not", "nothing", "notify", "notnull", "nowait", "null", "nullif", "nulls", "numeric", "object", "of", "off", "offset",
    "oids", "old", "on", "only", "operator", "option", "options", "or", "order", "ordinality", "others", "out", "outer", "over", "overlaps", "overlay", "overriding", "owned", "owner", "parallel",
    "parser", "partial", "partition", "partitioned", "passing", "password", "percent", "persistent", "pivot", "pivot_longer", "pivot_wider", "placing", "plans", "policy", "position", "positional",
    "pragma", "preceding", "precision", "prepare", "prepared", "preserve", "primary", "prior", "privileges", "procedural", "procedure", "program", "publication", "qualify", "quarter", "quarters",
    "quote", "range", "read", "real", "reassign", "recheck", "recursive", "ref", "references", "referencing", "refresh", "register", "reindex", "relative", "release", "rename", "repeatable",
    "replace", "replica", "reset", "resource", "resources", "respect", "restart", "restrict", "returning", "returns", "revoke", "right", "role", "rollback", "rollup", "row", "rows", "rule", "sample",
    "savepoint", "schema", "schemas", "scope", "scroll", "search", "second", "seconds", "secret", "secure", "security", "select", "semi", "sequence", "sequences", "serializable", "server", "session",
    "set", "setof", "sets", "share", "show", "similar", "similarity", "simple", "skip", "smallint", "snapshot", "some", "sorted", "source", "sql", "stable", "standalone", "start", "statement",
    "statistics", "stdin", "stdout", "storage", "stored", "strict", "strip", "struct", "subscription", "substring", "summarize", "symmetric", "sysid", "system", "table", "tables", "tablesample",
    "tablespace", "target", "temp", "template", "temporary", "text", "then", "ties", "time", "timestamp", "to", "trailing", "transaction", "transform", "treat", "trigger", "trim", "true", "truncate",
    "trusted", "try_cast", "tuple", "type", "types", "unbounded", "uncommitted", "unencrypted", "union", "unique", "unknown", "unlisten", "unlogged", "unpack", "unpivot", "until", "update", "use",
    "user", "using", "vacuum", "valid", "validate", "validator", "value", "values", "varchar", "variable", "variadic", "varying", "verbose", "version", "view", "views", "virtual", "volatile", "week",
    "weeks", "when", "where", "whitespace", "window", "with", "within", "without", "work", "wrapper", "write", "xml", "xmlattributes", "xmlconcat", "xmlelement", "xmlexists", "xmlforest",
    "xmlnamespaces", "xmlparse", "xmlpi", "xmlroot", "xmlserialize", "xmltable", "year", "years", "yes", "zone",
];

/// A name as a statement writes it: bare where DuckDB reads it as written, else in double quotes.
pub(crate) fn quote(name: &str) -> String {
    let bare = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if bare && !KEYWORDS.contains(&name.to_ascii_lowercase().as_str()) {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

/// An object's name with its schema, but in `main`, which every unqualified name means.
pub(crate) fn qualified(schema: &str, name: &str) -> String {
    if schema == "main" {
        quote(name)
    } else {
        format!("{}.{}", quote(schema), quote(name))
    }
}

pub(crate) fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// DuckDB's rendering written in lowercase, as `schema.sql` is: its keywords, and the words of a
/// type where one stands (after `AS` in a cast), where they are uppercase and outside strings and
/// quoted names. A name DuckDB writes bare keeps its case: DuckDB quotes every keyword it writes as a
/// name, but not a type's name (`DATE`, `UUID`) nor a struct's field.
pub(crate) fn styled(sql: &str) -> String {
    lowercased(sql, false)
}

/// A type DuckDB wrote (`DECIMAL(10,2)`, `STRUCT(DATE DATE)`), in lowercase but for its fields' names.
pub(crate) fn styled_type(t: &str) -> String {
    lowercased(t, true)
}

fn lowercased(sql: &str, whole_type: bool) -> String {
    let tokens = tokenize(sql, Dialect::Duckdb);
    let mut out = String::with_capacity(sql.len());
    let mut at = 0;
    // The depths of the parentheses that hold a struct's or a union's fields, those of a cast's, and
    // whether the next word is a field's name or the words to the cast's close a type.
    let (mut fields, mut casts): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
    let mut typed: Vec<usize> = Vec::new();
    let mut depth = 0usize;
    let mut field_next = false;
    for (k, t) in tokens.iter().enumerate() {
        match t.text.as_str() {
            "(" => {
                depth += 1;
                let before = if k > 0 { tokens[k - 1].upper() } else { String::new() };
                let opens_fields = matches!(before.as_str(), "STRUCT" | "UNION" | "ROW");
                if opens_fields {
                    fields.push(depth);
                }
                if matches!(before.as_str(), "CAST" | "TRY_CAST") {
                    casts.push(depth);
                }
                field_next = opens_fields;
                continue;
            }
            ")" => {
                if fields.last() == Some(&depth) {
                    fields.pop();
                }
                if casts.last() == Some(&depth) {
                    casts.pop();
                }
                if typed.last() == Some(&depth) {
                    typed.pop();
                }
                depth = depth.saturating_sub(1);
                field_next = false;
                continue;
            }
            "," => {
                field_next = fields.last() == Some(&depth);
                continue;
            }
            _ => {}
        }
        if std::mem::take(&mut field_next) {
            continue;
        }
        if t.upper() == "AS" && casts.last() == Some(&depth) {
            typed.push(depth);
        }
        let in_type = whole_type || !typed.is_empty();
        if t.kind == TokenKind::Word && t.text.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') && t.text.chars().any(|c| c.is_ascii_uppercase()) {
            let lower = t.text.to_ascii_lowercase();
            if in_type || ALL_KEYWORDS.binary_search(&lower.as_str()).is_ok() {
                out.push_str(&sql[at..t.start]);
                out.push_str(&lower);
                at = t.end;
            }
        }
    }
    out.push_str(&sql[at..]);
    out
}

// ---------------------------------------------------------------------------------------------
// The catalog, read once for the comparator's lines and the diff engine's objects.

/// The database's own objects: in the database the connection uses, neither internal nor temporary.
const HERE: &str = "database_name = current_database()";

#[derive(Clone, Debug, Default)]
pub(crate) struct TableRow {
    pub schema: String,
    pub name: String,
    pub comment: Option<String>,
    /// Each column's collation, as the table's own text writes it, which no catalog function gives.
    pub collations: BTreeMap<String, String>,
    /// Each generated column's expression, as the table's text writes it: `duckdb_columns()` gives
    /// it cast to the column's type, which a statement made from it would cast again.
    pub generated: BTreeMap<String, String>,
    /// The table's text, as DuckDB writes it.
    pub sql: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ColumnRow {
    pub schema: String,
    pub table: String,
    pub name: String,
    pub position: i64,
    pub data_type: String,
    pub not_null: bool,
    pub default: Option<String>,
    pub generated: Option<String>,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ConstraintRow {
    pub schema: String,
    pub table: String,
    /// `primary key`, `unique`, `check` or `foreign key`.
    pub kind: String,
    pub columns: Vec<String>,
    /// A check's expression.
    pub expression: String,
    /// A foreign key's table and columns.
    pub references: Option<(String, Vec<String>)>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Named {
    pub schema: String,
    pub name: String,
    /// The table an index is on.
    pub table: String,
    /// The statement DuckDB writes for it, or a type's definition.
    pub sql: String,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SequenceRow {
    pub schema: String,
    pub name: String,
    pub start: String,
    /// The value it gave last, where it gave one since its database was opened.
    pub last: Option<String>,
    pub increment: String,
    pub min: String,
    pub max: String,
    pub cycle: bool,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MacroRow {
    pub schema: String,
    pub name: String,
    pub table: bool,
    /// Each overload's parameters, each its name and any type it is given, and body.
    pub overloads: Vec<(Vec<(String, Option<String>)>, String)>,
    pub comment: Option<String>,
    /// The whole statement, as `EXPORT DATABASE` writes it with its parameters' defaults, where it
    /// could be read and agrees with the rest.
    pub exported: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Read {
    pub schemas: Vec<(String, Option<String>)>,
    pub tables: Vec<TableRow>,
    pub columns: Vec<ColumnRow>,
    pub constraints: Vec<ConstraintRow>,
    pub indexes: Vec<Named>,
    pub views: Vec<Named>,
    pub sequences: Vec<SequenceRow>,
    pub types: Vec<Named>,
    pub macros: Vec<MacroRow>,
    /// The comments on views' columns: each view's schema and name, the column and the comment.
    pub view_comments: Vec<(String, String, String, String)>,
    /// What DuckDB records each object depends on, which it makes nothing before and drops nothing
    /// under: a table on the sequences, types and macros its expressions reach, through other
    /// macros too, an index on those of its expression, a type on the types it was written with.
    /// Each the dependent's id and the id of what it depends on.
    pub depends: Vec<(String, String)>,
}

fn json_list(text: &str) -> Vec<String> {
    serde_json::from_str(text).unwrap_or_default()
}

/// What a table's own text says of each generated column: `n INTEGER GENERATED ALWAYS AS((a + 1))`.
fn generated(sql: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for part in parts(sql) {
        let Some(first) = part.first() else { continue };
        let Some(k) = part.windows(3).position(|w| w[0].upper() == "GENERATED" && w[1].upper() == "ALWAYS" && w[2].upper() == "AS") else { continue };
        let open = k + 3;
        if part.get(open).map(|t| t.text.as_str()) != Some("(") {
            continue;
        }
        let mut depth = 0;
        for (j, t) in part.iter().enumerate().skip(open) {
            match t.text.as_str() {
                "(" => depth += 1,
                ")" => {
                    depth -= 1;
                    if depth == 0 {
                        let (start, end) = (part[open + 1].start, part[j - 1].end);
                        out.insert(unquote(first), sql[start..end].to_string());
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// What a table's own text says of each column's collation: `name VARCHAR COLLATE nocase`.
fn collations(sql: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for part in parts(sql) {
        let Some(first) = part.first() else { continue };
        if first.kind == TokenKind::Word && ["CONSTRAINT", "PRIMARY", "UNIQUE", "CHECK", "FOREIGN"].contains(&first.upper().as_str()) {
            continue;
        }
        let mut depth = 0;
        for (k, t) in part.iter().enumerate() {
            match t.text.as_str() {
                "(" => depth += 1,
                ")" => depth -= 1,
                _ if depth == 0 && t.upper() == "COLLATE" => {
                    // A collation of several parts, `nocase.noaccent`, is all of them.
                    let mut name = String::new();
                    let mut j = k + 1;
                    while let Some(c) = part.get(j).filter(|c| matches!(c.kind, TokenKind::Word | TokenKind::Quoted)) {
                        name.push_str(&unquote(c));
                        if part.get(j + 1).is_some_and(|d| d.text == ".") {
                            name.push('.');
                            j += 2;
                        } else {
                            break;
                        }
                    }
                    if !name.is_empty() {
                        out.insert(unquote(first), name);
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The parts of a `create table` text's column list, at its top level.
fn parts(sql: &str) -> Vec<Vec<Token>> {
    let tokens = tokenize(sql, Dialect::Duckdb);
    let open = tokens.iter().position(|t| t.text == "(").map_or(tokens.len(), |at| at + 1);
    let mut depth = 0;
    let mut part = Vec::new();
    let mut out = Vec::new();
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
            out.push(std::mem::take(&mut part));
            continue;
        }
        part.push(t.clone());
    }
    if !part.is_empty() {
        out.push(part);
    }
    out
}

/// A quoted name without its quotes, anything else as written.
fn unquote(t: &Token) -> String {
    if t.kind == TokenKind::Quoted && t.text.len() >= 2 {
        t.text[1..t.text.len() - 1].replace("\"\"", "\"")
    } else {
        t.text.clone()
    }
}

/// A table's constraints, read from its text as DuckDB writes it: a key or a unique constraint of
/// one column after the column's type, the rest after the columns, a check's expression as
/// `duckdb_constraints()` gives it.
fn constraints_of(schema: &str, table: &str, sql: &str) -> Vec<ConstraintRow> {
    let row = |kind: &str, columns: Vec<String>| ConstraintRow { schema: schema.into(), table: table.into(), kind: kind.into(), columns, ..ConstraintRow::default() };
    let names = |part: &[Token], open: usize| -> Vec<String> {
        part.iter().skip(open + 1).take_while(|t| t.text != ")").filter(|t| t.text != ",").map(unquote).collect()
    };
    let mut out = Vec::new();
    for part in parts(sql) {
        let Some(first) = part.first() else { continue };
        let at = |k: usize| part.get(k).map(Token::upper).unwrap_or_default();
        match first.upper().as_str() {
            "CHECK" if part.len() > 3 => out.push(ConstraintRow { expression: sql[part[2].start..part[part.len() - 2].end].to_string(), ..row("check", vec![]) }),
            "PRIMARY" if at(1) == "KEY" => out.push(row("primary key", names(&part, 2))),
            "UNIQUE" => out.push(row("unique", names(&part, 1))),
            "FOREIGN" => {
                let text = &sql[first.start..part[part.len() - 1].end];
                if let Some((columns, to, referenced)) = foreign_key(text) {
                    out.push(ConstraintRow { references: Some((to, referenced)), ..row("foreign key", columns) });
                }
            }
            _ => {
                // A column's own: after its type, outside any parentheses.
                let mut depth = 0;
                for (k, t) in part.iter().enumerate().skip(1) {
                    match t.text.as_str() {
                        "(" => depth += 1,
                        ")" => depth -= 1,
                        _ if depth == 0 && t.upper() == "PRIMARY" && at(k + 1) == "KEY" => out.push(row("primary key", vec![unquote(first)])),
                        _ if depth == 0 && t.upper() == "UNIQUE" => out.push(row("unique", vec![unquote(first)])),
                        _ => {}
                    }
                }
            }
        }
    }
    out
}

/// DuckDB keeps a constraint's names as its statement wrote them (`references Users (ID)` of a table
/// `users (id)`), and matches names without regard to case: each is read as its table and columns
/// are named.
fn normalize_constraints(read: &mut Read) {
    for k in read.constraints.iter_mut() {
        let column_of = |schema: &str, table: &str, name: &str| read.columns.iter().find(|c| c.schema == schema && c.table == table && c.name.eq_ignore_ascii_case(name)).map(|c| c.name.clone()).unwrap_or_else(|| name.to_string());
        k.columns = k.columns.iter().map(|c| column_of(&k.schema, &k.table, c)).collect();
        if let Some((to, columns)) = &k.references {
            let table = [k.schema.as_str(), "main"].iter().find_map(|s| read.tables.iter().find(|t| t.schema == *s && t.name.eq_ignore_ascii_case(to))).map(|t| (t.schema.clone(), t.name.clone()));
            k.references = Some(match table {
                Some((schema, name)) => (name.clone(), columns.iter().map(|c| column_of(&schema, &name, c)).collect()),
                None => (to.clone(), columns.clone()),
            });
        }
    }
}

/// The database's schema, as its catalog functions give it.
pub(crate) async fn read_catalog(db: &mut Db) -> Result<Read, Error> {
    let history = format!("not (schema_name = 'main' and table_name = '{HISTORY_TABLE}')");
    let mut read = Read::default();
    for r in db.rows(&format!("select schema_name as s, comment from duckdb_schemas() where {HERE} and not internal order by 1")).await? {
        read.schemas.push((r.text("s").to_string(), r.get("comment").map(String::from)));
    }
    for r in db.rows(&format!("select schema_name as s, table_name as t, comment, sql from duckdb_tables() where {HERE} and not internal and not temporary and {history} order by 1, 2")).await? {
        read.tables.push(TableRow { schema: r.text("s").into(), name: r.text("t").into(), comment: r.get("comment").map(String::from), collations: collations(r.text("sql")), generated: generated(r.text("sql")), sql: r.text("sql").to_string() });
    }
    for r in db
        .rows(&format!(
            "select c.schema_name as s, c.table_name as t, c.column_name as c, c.column_index as i, c.data_type as type, c.is_nullable as nullable,
              c.column_default as def, c.is_generated as gen, c.generation_expression as expr, c.comment as comment
            from duckdb_columns() c join duckdb_tables() t on t.table_oid = c.table_oid
            where c.{HERE} and not t.internal and not t.temporary and not (t.schema_name = 'main' and t.table_name = '{HISTORY_TABLE}')
            order by 1, 2, c.column_index"
        ))
        .await?
    {
        let generated = r.flag("gen");
        let written = read.tables.iter().find(|t| t.schema == r.text("s") && t.name == r.text("t")).and_then(|t| t.generated.get(r.text("c")).cloned());
        read.columns.push(ColumnRow {
            schema: r.text("s").into(),
            table: r.text("t").into(),
            name: r.text("c").into(),
            position: r.text("i").parse().unwrap_or(0),
            data_type: r.text("type").into(),
            not_null: !r.flag("nullable"),
            default: if generated { None } else { r.get("def").map(String::from) },
            generated: if generated { Some(written.unwrap_or_else(|| r.get("expr").or(r.get("def")).unwrap_or_default().to_string())) } else { None },
            comment: r.get("comment").map(String::from),
        });
    }
    let constraints = db
        .rows(&format!(
            "select schema_name as s, table_name as t, constraint_type as kind, expression as expr, to_json(constraint_column_names)::varchar as cols,
              referenced_table as ref, to_json(referenced_column_names)::varchar as refcols
            from duckdb_constraints() where {HERE} and constraint_type in ('PRIMARY KEY', 'UNIQUE', 'CHECK', 'FOREIGN KEY') and {history}
            order by 1, 2, constraint_index"
        ))
        .await;
    match constraints {
        Ok(rows) => {
            for r in rows {
                let kind = r.text("kind").to_lowercase();
                read.constraints.push(ConstraintRow {
                    schema: r.text("s").into(),
                    table: r.text("t").into(),
                    references: (kind == "foreign key").then(|| (r.text("ref").to_string(), json_list(r.text("refcols")))),
                    kind,
                    columns: json_list(r.text("cols")),
                    expression: r.text("expr").to_string(),
                });
            }
        }
        // DuckDB's `duckdb_constraints()` fails for the whole database where one check reads a
        // struct's field with a dot (`meta.a > 0`), which it binds as a table's column: each
        // table's constraints are read from its own text instead, which writes them alike.
        Err(_) => {
            for t in &read.tables {
                read.constraints.extend(constraints_of(&t.schema, &t.name, &t.sql));
            }
        }
    }
    normalize_constraints(&mut read);
    for r in db.rows(&format!("select schema_name as s, index_name as n, table_name as t, sql, comment from duckdb_indexes() where {HERE} order by 1, 2")).await? {
        read.indexes.push(Named { schema: r.text("s").into(), name: r.text("n").into(), table: r.text("t").into(), sql: trimmed(r.text("sql")), comment: r.get("comment").map(String::from) });
    }
    for r in db.rows(&format!("select schema_name as s, view_name as n, sql, comment from duckdb_views() where {HERE} and not internal and not temporary order by 1, 2")).await? {
        read.views.push(Named { schema: r.text("s").into(), name: r.text("n").into(), sql: trimmed(r.text("sql")), comment: r.get("comment").map(String::from), ..Named::default() });
    }
    for r in db
        .rows(&format!(
            "select c.schema_name as s, c.table_name as v, c.column_name as c, c.comment as comment
            from duckdb_columns() c join duckdb_views() v on v.view_oid = c.table_oid
            where c.{HERE} and not v.internal and not v.temporary and c.comment is not null order by 1, 2, c.column_index"
        ))
        .await?
    {
        read.view_comments.push((r.text("s").into(), r.text("v").into(), r.text("c").into(), r.text("comment").into()));
    }
    for r in db
        .rows(&format!(
            "select schema_name as s, sequence_name as n, start_value as start, last_value as last, increment_by as inc, min_value as min, max_value as max, cycle, comment
            from duckdb_sequences() where {HERE} and not temporary order by 1, 2"
        ))
        .await?
    {
        read.sequences.push(SequenceRow {
            schema: r.text("s").into(),
            name: r.text("n").into(),
            start: r.text("start").into(),
            last: r.get("last").map(String::from),
            increment: r.text("inc").into(),
            min: r.text("min").into(),
            max: r.text("max").into(),
            cycle: r.flag("cycle"),
            comment: r.get("comment").map(String::from),
        });
    }
    let types = db.rows(&format!("select schema_name as s, type_name as n, comment from duckdb_types() where {HERE} and not internal order by 1, 2")).await?;
    for r in types {
        let (schema, name) = (r.text("s").to_string(), r.text("n").to_string());
        let def = db.rows(&format!("select typeof(cast(null as {})) as t", qualified(&schema, &name))).await?.first().map(|t| t.text("t").to_string()).unwrap_or_default();
        read.types.push(Named { schema, name, sql: def, comment: r.get("comment").map(String::from), ..Named::default() });
    }
    let macros = db
        .rows(&format!(
            "select schema_name as s, function_name as n, function_type as kind, to_json(parameters)::varchar as params, to_json(parameter_types)::varchar as types, macro_definition as body, comment
            from duckdb_functions() where {HERE} and not internal and function_type in ('macro', 'table_macro') order by 1, 2"
        ))
        .await?;
    for r in macros {
        let (schema, name) = (r.text("s").to_string(), r.text("n").to_string());
        let types: Vec<Option<String>> = serde_json::from_str(r.text("types")).unwrap_or_default();
        let params = json_list(r.text("params")).into_iter().enumerate().map(|(k, p)| (p, types.get(k).cloned().flatten())).collect();
        let overload = (params, r.text("body").to_string());
        // A scalar macro and a table macro may share a name: two objects.
        let table = r.text("kind") == "table_macro";
        match read.macros.iter_mut().find(|m| m.schema == schema && m.name == name && m.table == table) {
            Some(m) => m.overloads.push(overload),
            None => read.macros.push(MacroRow { schema, name, table, overloads: vec![overload], comment: r.get("comment").map(String::from), exported: None }),
        }
    }
    // Every object of the database by its oid, as the diff engine names it.
    for r in db
        .rows(&format!(
            "with o as (
              select table_oid as oid, 'table' as kind, schema_name as s, table_name as n from duckdb_tables() where {HERE}
              union all select sequence_oid, 'sequence', schema_name, sequence_name from duckdb_sequences() where {HERE}
              union all select type_oid, 'type', schema_name, type_name from duckdb_types() where {HERE} and not internal
              union all select function_oid, function_type, schema_name, function_name from duckdb_functions() where {HERE} and not internal and function_type in ('macro', 'table_macro')
              union all select index_oid, 'index', schema_name, index_name from duckdb_indexes() where {HERE}
            )
            select distinct a.kind as rk, a.s as rs, a.n as rn, b.kind as dk, b.s as ds, b.n as dn
            from duckdb_dependencies() d join o a on a.oid = d.objid join o b on b.oid = d.refobjid
            where d.deptype = 'n' order by 1, 2, 3, 4, 5, 6"
        ))
        .await?
    {
        let id = |kind: &str, schema: &str, name: &str| match kind {
            "macro" => macro_id(schema, name, false),
            "table_macro" => macro_id(schema, name, true),
            _ => format!("{kind} {}", qualified(schema, name)),
        };
        read.depends.push((id(r.text("dk"), r.text("ds"), r.text("dn")), id(r.text("rk"), r.text("rs"), r.text("rn"))));
    }
    if !read.macros.is_empty() {
        if let Ok(Some(text)) = exported(db).await {
            for statement in crate::sql::split_statements(&text, Dialect::Duckdb) {
                let words: Vec<String> = statement.tokens.iter().take(3).map(Token::upper).collect();
                if words.first().map(String::as_str) != Some("CREATE") || !words.iter().any(|w| w == "MACRO") {
                    continue;
                }
                let at = statement.tokens.iter().position(|t| t.upper() == "MACRO").map_or(0, |k| k + 1);
                let (schema, name) = name_at(&statement.tokens, at);
                let table = makes_table_macro(&statement.tokens);
                if let Some(m) = read.macros.iter_mut().find(|m| m.schema == schema && m.name == name && m.table == table) {
                    m.exported = Some(statement.text.clone());
                }
            }
        }
    }
    Ok(read)
}

/// Whether a `create macro` statement makes a table macro: one whose body follows `AS TABLE`.
fn makes_table_macro(tokens: &[Token]) -> bool {
    tokens.windows(2).any(|w| w[0].upper() == "AS" && w[1].upper() == "TABLE")
}

/// A macro's id: a table macro's apart from a scalar one's of the same name.
fn macro_id(schema: &str, name: &str, table: bool) -> String {
    format!("macro {}{}", if table { "table " } else { "" }, qualified(schema, name))
}

/// The schema and name of the object whose name starts at token `at`, `main` where unqualified.
fn name_at(tokens: &[Token], at: usize) -> (String, String) {
    let first = tokens.get(at).map(unquote).unwrap_or_default();
    if tokens.get(at + 1).is_some_and(|t| t.text == ".") {
        (first, tokens.get(at + 2).map(unquote).unwrap_or_default())
    } else {
        ("main".into(), first)
    }
}

fn trimmed(sql: &str) -> String {
    sql.trim().trim_end_matches(';').trim_end().to_string()
}

/// DuckDB's own text of the database's schema, as `EXPORT DATABASE` writes it: of a copy of the
/// schema without rows, made in a database of its own, on a connection of its own, so that the
/// connection's transaction, if it has one, is left as it is. None where no copy could be made.
pub(crate) async fn exported(db: &mut Db) -> Result<Option<String>, Error> {
    let Db::Duckdb(conn) = db else { return Ok(None) };
    let catalog = conn.query("select current_database() as n")?.1.first().and_then(|r| r[0].clone()).unwrap_or_default();
    let mut side = conn.sibling()?;
    let copy = crate::db::random_name().replace(crate::db::SHADOW_PREFIX, "reylo_schema_");
    let dir = std::env::temp_dir().join(format!("{copy}-{}", std::process::id()));
    let made = (|| -> Result<String, Error> {
        side.query(&format!("attach ':memory:' as {copy}"))?;
        let written = (|| {
            side.query(&format!("copy from database {} to {copy} (schema)", quote(&catalog)))?;
            side.query(&format!("drop table if exists {copy}.main.{HISTORY_TABLE}"))?;
            side.query(&format!("export database {copy} to {}", literal(&dir.to_string_lossy())))?;
            std::fs::read_to_string(dir.join("schema.sql")).map_err(|e| Error(e.to_string()))
        })();
        let _ = side.query(&format!("detach {copy}"));
        written
    })();
    let _ = std::fs::remove_dir_all(&dir);
    Ok(made.ok())
}

/// The statements of an export, each after those that make what it names: `EXPORT DATABASE` writes
/// each kind of object together, views in name order and tables before the macros their checks call,
/// which a fresh database refuses. Where no order is found, the rest keep the export's.
pub(crate) fn in_order(text: &str) -> Vec<String> {
    let statements: Vec<crate::sql::Statement> = crate::sql::split_statements(text, Dialect::Duckdb);
    // What each makes: the name after `CREATE [OR REPLACE] [UNIQUE] <kind> [IF NOT EXISTS]`.
    let made: Vec<Option<String>> = statements
        .iter()
        .map(|s| {
            let t = &s.tokens;
            if t.first().map(Token::upper).as_deref() != Some("CREATE") {
                return None;
            }
            let k = t.iter().position(|x| matches!(x.upper().as_str(), "TABLE" | "VIEW" | "MACRO" | "FUNCTION" | "SEQUENCE" | "TYPE" | "SCHEMA" | "INDEX"))?;
            let mut at = k + 1;
            if t.get(at).map(Token::upper).as_deref() == Some("IF") {
                at += 3;
            }
            if t.get(k).map(Token::upper).as_deref() == Some("MACRO") && t.get(at).map(Token::upper).as_deref() == Some("TABLE") {
                at += 1;
            }
            let (_, name) = name_at(t, at);
            Some(name.to_lowercase())
        })
        .collect();
    // What each names, but a macro's parameters, which name nothing whatever they share a name with.
    let named: Vec<BTreeSet<String>> = statements
        .iter()
        .map(|s| {
            let params = macro_params(&s.tokens);
            s.tokens.iter().filter(|t| matches!(t.kind, TokenKind::Word | TokenKind::Quoted)).map(|t| unquote(t).to_lowercase()).filter(|n| !params.contains(n)).collect()
        })
        .collect();
    let mut left: Vec<usize> = (0..statements.len()).collect();
    let mut out = Vec::new();
    while !left.is_empty() {
        let ready = left.iter().position(|&i| !left.iter().any(|&j| j != i && made[j].as_ref().is_some_and(|m| named[i].contains(m) && made[i].as_ref() != Some(m)))).unwrap_or(0);
        out.push(statements[left.remove(ready)].text.clone());
    }
    out
}

/// The names a `create macro` statement gives its parameters, in each of its overloads: the first
/// word of each item of a list in parentheses that follows the macro's name or a comma, and is
/// followed by `AS`.
fn macro_params(tokens: &[Token]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if tokens.first().map(Token::upper).as_deref() != Some("CREATE") {
        return out;
    }
    let Some(m) = tokens.iter().position(|t| t.upper() == "MACRO") else { return out };
    let mut depth = 0;
    let mut k = m + 1;
    while k < tokens.len() {
        let t = &tokens[k];
        if t.text == "(" && depth == 0 && (tokens[k - 1].text == "," || matches!(tokens[k - 1].kind, TokenKind::Word | TokenKind::Quoted)) {
            let mut j = k + 1;
            let mut inner = 0;
            let mut item_start = true;
            let mut names = Vec::new();
            while j < tokens.len() && !(tokens[j].text == ")" && inner == 0) {
                match tokens[j].text.as_str() {
                    "(" => inner += 1,
                    ")" => inner -= 1,
                    "," if inner == 0 => {
                        item_start = true;
                        j += 1;
                        continue;
                    }
                    _ => {}
                }
                if item_start && matches!(tokens[j].kind, TokenKind::Word | TokenKind::Quoted) {
                    names.push(unquote(&tokens[j]).to_lowercase());
                }
                item_start = false;
                j += 1;
            }
            if tokens.get(j + 1).is_some_and(|a| a.upper() == "AS") {
                out.extend(names);
            }
            k = j + 1;
            continue;
        }
        match t.text.as_str() {
            "(" => depth += 1,
            ")" => depth -= 1,
            _ => {}
        }
        k += 1;
    }
    out
}

/// What `EXPORT DATABASE` leaves out of the schema it writes: the comments, as the statements that
/// make them.
pub(crate) async fn comment_statements(db: &mut Db) -> Result<Vec<String>, Error> {
    let schema = read(db).await?;
    Ok(schema.objects.values().filter(|o| o.kind == Some(Kind::Comment)).flat_map(|o| o.create.clone()).collect())
}

/// A macro's definition as the catalog functions give it: each overload's parameters and body.
fn macro_def(m: &MacroRow) -> String {
    let overloads: Vec<String> = m.overloads.iter().map(|(params, body)| format!("({}) AS {}{body}", parameters(params), if m.table { "TABLE " } else { "" })).collect();
    overloads.join(", ")
}

/// A macro's parameters as its definition writes them: each name, and its type where it has one.
fn parameters(params: &[(String, Option<String>)]) -> String {
    params.iter().map(|(p, t)| match t {
        Some(t) => format!("{} {t}", quote(p)),
        None => quote(p),
    }).collect::<Vec<_>>().join(", ")
}

/// The comparator's lines of a DuckDB database: each object, written from the catalog's renderings,
/// with names as DuckDB keeps them, in the database the connection uses.
pub(crate) async fn lines(db: &mut Db) -> Result<Vec<String>, Error> {
    let read = read_catalog(db).await?;
    let json = |text: &str| serde_json::to_string(text).unwrap_or_default();
    let mut lines = Vec::new();
    for (s, comment) in &read.schemas {
        lines.push(format!("schema {}", quote(s)));
        if let Some(c) = comment {
            lines.push(format!("comment schema {} {}", quote(s), json(c)));
        }
    }
    for t in &read.tables {
        let name = qualified(&t.schema, &t.name);
        lines.push(format!("table {name}"));
        if let Some(c) = &t.comment {
            lines.push(format!("comment table {name} {}", json(c)));
        }
    }
    let collation = |c: &ColumnRow| read.tables.iter().find(|t| t.schema == c.schema && t.name == c.table).and_then(|t| t.collations.get(&c.name).cloned());
    for c in &read.columns {
        let table = qualified(&c.schema, &c.table);
        lines.push(format!("column {table}.{} {}", quote(&c.name), column_def(c, collation(c).as_deref())));
        if let Some(comment) = &c.comment {
            lines.push(format!("comment column {table}.{} {}", quote(&c.name), json(comment)));
        }
    }
    for k in &read.constraints {
        lines.push(constraint_line(k));
    }
    for i in &read.indexes {
        lines.push(format!("index {}", i.sql));
        if let Some(c) = &i.comment {
            lines.push(format!("comment index {} {}", qualified(&i.schema, &i.name), json(c)));
        }
    }
    for v in &read.views {
        lines.push(format!("view {} {}", qualified(&v.schema, &v.name), v.sql));
        if let Some(c) = &v.comment {
            lines.push(format!("comment view {} {}", qualified(&v.schema, &v.name), json(c)));
        }
    }
    for (schema, view, column, text) in &read.view_comments {
        lines.push(format!("comment view column {}.{} {}", qualified(schema, view), quote(column), json(text)));
    }
    for s in &read.sequences {
        lines.push(format!("sequence {} {}", qualified(&s.schema, &s.name), sequence_def(s)));
        if let Some(c) = &s.comment {
            lines.push(format!("comment sequence {} {}", qualified(&s.schema, &s.name), json(c)));
        }
    }
    for t in &read.types {
        lines.push(format!("type {} {}", qualified(&t.schema, &t.name), t.sql));
        if let Some(c) = &t.comment {
            lines.push(format!("comment type {} {}", qualified(&t.schema, &t.name), json(c)));
        }
    }
    for m in &read.macros {
        let name = qualified(&m.schema, &m.name);
        for (params, body) in &m.overloads {
            lines.push(format!("{}macro {name}({}) {body}", if m.table { "table " } else { "" }, parameters(params)));
        }
        if let Some(c) = &m.comment {
            lines.push(format!("comment {}macro {name} {}", if m.table { "table " } else { "" }, json(c)));
        }
    }
    Ok(lines)
}

/// A column's definition: its type, as DuckDB names it (a type of the database's own written out),
/// its collation, whether it takes nulls, and its default or what generates it.
fn column_def(c: &ColumnRow, collation: Option<&str>) -> String {
    let mut out = c.data_type.clone();
    if let Some(collation) = collation {
        out.push_str(&format!(" COLLATE {collation}"));
    }
    if c.not_null {
        out.push_str(" NOT NULL");
    }
    match (&c.generated, &c.default) {
        (Some(expr), _) => out.push_str(&format!(" GENERATED ALWAYS AS ({expr}) VIRTUAL")),
        (None, Some(def)) => out.push_str(&format!(" DEFAULT {def}")),
        _ => {}
    }
    out
}

fn columns_list(columns: &[String]) -> String {
    columns.iter().map(|c| quote(c)).collect::<Vec<_>>().join(", ")
}

/// A constraint's line: what it is and on what, without the name DuckDB gives it, which is its own
/// whatever a statement names it.
fn constraint_line(k: &ConstraintRow) -> String {
    let table = qualified(&k.schema, &k.table);
    match (k.kind.as_str(), &k.references) {
        ("check", _) => format!("check {table} {}", k.expression),
        ("foreign key", Some((to, columns))) => format!("foreign key {table} ({}) references {} ({})", columns_list(&k.columns), quote(to), columns_list(columns)),
        (kind, _) => format!("{kind} {table} ({})", columns_list(&k.columns)),
    }
}

/// A constraint as a table's definition writes it.
fn constraint_sql(k: &ConstraintRow) -> String {
    match (k.kind.as_str(), &k.references) {
        ("check", _) => format!("check ({})", styled(&k.expression)),
        ("foreign key", Some((to, columns))) => format!("foreign key ({}) references {} ({})", columns_list(&k.columns), quote(to), columns_list(columns)),
        (kind, _) => format!("{kind} ({})", columns_list(&k.columns)),
    }
}

/// A sequence's definition, without where it starts: DuckDB stores a sequence with its current
/// value as its start, so that after a database is opened again, the start is the rows' doing.
fn sequence_def(s: &SequenceRow) -> String {
    format!("increment {} min {} max {}{}", s.increment, s.min, s.max, if s.cycle { " cycle" } else { "" })
}

/// What the database holds that the comparison reads only in part: the macros whose parameters have
/// defaults, which no catalog function gives, so that a change to one is not seen.
pub async fn unmodeled(db: &mut Db) -> Result<Vec<String>, Error> {
    let read = read_catalog(db).await?;
    Ok(read.macros.iter().filter(|m| m.exported.as_deref().is_some_and(|t| t.contains(":="))).map(|m| format!("macro {}: its parameters' defaults are not read", qualified(&m.schema, &m.name))).collect())
}

// ---------------------------------------------------------------------------------------------
// The diff engine's objects.

/// The sequences a default names, `nextval('s')`, as the schema-qualified names of `read`.
fn sequences_in(text: &str, read: &Read) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for s in &read.sequences {
        let quoted = |n: &str| format!("\"{}\"", n.replace('"', "\"\""));
        let named = [
            format!("'{}'", s.name),
            format!("'{}.{}'", s.schema, s.name),
            format!("'{}'", qualified(&s.schema, &s.name)),
            format!("'{}'", quoted(&s.name)),
            format!("'{}.{}'", s.schema, quoted(&s.name)),
            format!("'{}.{}'", quoted(&s.schema), quoted(&s.name)),
        ];
        if named.iter().any(|n| text.to_lowercase().contains(&n.to_lowercase())) {
            out.insert(format!("sequence {}", qualified(&s.schema, &s.name)));
        }
    }
    out
}

/// The names a text mentions, folded: what a view or a macro depends on, read from its words.
fn mentions(sql: &str) -> BTreeSet<String> {
    tokenize(sql, Dialect::Duckdb).iter().filter(|t| matches!(t.kind, TokenKind::Word | TokenKind::Quoted)).map(|t| unquote(t).to_lowercase()).collect()
}

/// The schema of the database `db` is connected to, as the diff engine's objects.
pub async fn read(db: &mut Db) -> Result<Schema, Error> {
    let read = read_catalog(db).await?;
    // DuckDB reads `x.t` as the table `t` of the database `x` or of the schema `x`, and refuses it as
    // ambiguous where both are there, so no statement of a plan can name such a schema's objects.
    let database = db.rows("select current_database() as n").await?.first().map(|r| r.text("n").to_string()).unwrap_or_default();
    if let Some((schema, _)) = read.schemas.iter().find(|(s, _)| s.eq_ignore_ascii_case(&database)) {
        return Err(Error(format!("The schema {} has the name of the database it is in, so DuckDB reads every name written {schema}.<name> as ambiguous: rename the schema or the database file", quote(schema))));
    }
    // So does every other database DuckDB has open, `system` and `temp`.
    let open: Vec<String> = db.rows("select database_name as n from duckdb_databases()").await?.iter().map(|r| r.text("n").to_string()).collect();
    if let Some((schema, _)) = read.schemas.iter().find(|(s, _)| open.iter().any(|d| d.eq_ignore_ascii_case(s))) {
        return Err(Error(format!("The schema {} has the name of DuckDB's database {schema}, so DuckDB reads every name written {schema}.<name> as ambiguous: rename the schema", quote(schema))));
    }
    let mut s = Schema::default();
    // A type of the database's own, by the definition DuckDB writes in its place in a column's type,
    // where no other type has that definition: written by its name.
    let mut aliases: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for t in &read.types {
        if t.sql.contains('(') {
            aliases.entry(t.sql.clone()).or_default().push(qualified(&t.schema, &t.name));
        }
    }
    let alias_of = |data_type: &str| -> (String, Option<String>) {
        match aliases.get(data_type) {
            Some(names) if names.len() == 1 => (names[0].clone(), Some(format!("type {}", names[0]))),
            _ => (styled_type(data_type), None),
        }
    };

    for (name, comment) in &read.schemas {
        let id = format!("schema {}", quote(name));
        s.add(Object { id: id.clone(), kind: Some(Kind::Schema), def: name.clone(), create: vec![format!("create schema {}", quote(name))], drop: vec![format!("drop schema {}", quote(name))], ..Object::default() });
        if let Some(c) = comment {
            add_comment(&mut s, &id, "schema", &quote(name), c);
        }
    }
    let schema_dep = |schema: &str| -> BTreeSet<String> { if schema == "main" { BTreeSet::new() } else { BTreeSet::from([format!("schema {}", quote(schema))]) } };
    // The macros and types an expression names (a default, a check, a generated column's, an
    // index's), which DuckDB needs there as what holds it is made, and keeps from being dropped while
    // it stands.
    // and what the macros it calls reach in turn, as DuckDB records it: their bodies' macros, types
    // and sequences, but for their parameters' names.
    let expression_deps = |texts: &[&str]| -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut queue: Vec<(String, BTreeSet<String>)> = texts.iter().map(|t| (t.to_string(), BTreeSet::new())).collect();
        while let Some((text, params)) = queue.pop() {
            let named: BTreeSet<String> = mentions(&text).difference(&params).cloned().collect();
            out.extend(sequences_in(&text, &read));
            out.extend(read.types.iter().filter(|t| named.contains(&t.name.to_lowercase())).map(|t| format!("type {}", qualified(&t.schema, &t.name))));
            for m in read.macros.iter().filter(|m| !m.table && named.contains(&m.name.to_lowercase())) {
                if out.insert(macro_id(&m.schema, &m.name, false)) {
                    queue.extend(m.overloads.iter().map(|(p, body)| (body.clone(), p.iter().map(|(n, _)| n.to_lowercase()).collect())));
                }
            }
        }
        out
    };
    for t in &read.types {
        let name = qualified(&t.schema, &t.name);
        let id = format!("type {name}");
        // A column holds its type written out, so a type is dropped and made again under its columns,
        // which change with their tables: changed, it is no replacement of what uses it.
        s.add(Object {
            id: id.clone(),
            kind: Some(Kind::Type),
            def: t.sql.clone(),
            create: vec![format!("create type {name} as {}", styled_type(&t.sql))],
            drop: vec![format!("drop type {name}")],
            deps: schema_dep(&t.schema),
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
        if let Some(c) = &t.comment {
            add_comment(&mut s, &id, "type", &name, c);
        }
    }
    for q in &read.sequences {
        let name = qualified(&q.schema, &q.name);
        let id = format!("sequence {name}");
        let create = format!("create sequence {name} start with {} increment by {} minvalue {} maxvalue {}{}", q.start, q.increment, q.min, q.max, if q.cycle { " cycle" } else { "" });
        // Where it starts again: its first value, since the start it was made with isn't kept.
        let first = if q.increment.starts_with('-') { &q.max } else { &q.min };
        let restart = format!("create sequence {name} start with {first} increment by {} minvalue {} maxvalue {}{}", q.increment, q.min, q.max, if q.cycle { " cycle" } else { "" });
        // The value it gives next, where a plan made from this database starts it again.
        let next = match (q.last.as_deref().and_then(|l| l.parse::<i128>().ok()), q.increment.parse::<i128>().ok()) {
            (Some(last), Some(inc)) => (last + inc).to_string(),
            _ => q.start.clone(),
        };
        s.add(Object {
            id: id.clone(),
            kind: Some(Kind::Sequence),
            def: sequence_def(q),
            create: vec![create],
            drop: vec![format!("drop sequence {name}")],
            deps: schema_dep(&q.schema),
            extra: vec![format!("restart{SEP}{restart}"), format!("start{SEP}{}", q.start), format!("next{SEP}{next}")],
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
        if let Some(c) = &q.comment {
            add_comment(&mut s, &id, "sequence", &name, c);
        }
    }
    for t in &read.tables {
        let name = qualified(&t.schema, &t.name);
        let id = format!("table {name}");
        let columns: Vec<&ColumnRow> = read.columns.iter().filter(|c| c.schema == t.schema && c.table == t.name).collect();
        let constraints: Vec<&ConstraintRow> = read.constraints.iter().filter(|k| k.schema == t.schema && k.table == t.name).collect();
        let mut deps = schema_dep(&t.schema);
        let mut parts = Vec::new();
        let mut lines = Vec::new();
        let mut info = vec![format!("name{SEP}{name}")];
        for c in &columns {
            let collation = t.collations.get(&c.name).map(String::as_str);
            let def = column_def(c, collation);
            lines.push(format!("{} {def}", quote(&c.name)));
            let (written_type, type_dep) = alias_of(&c.data_type);
            let mut written = written_type;
            if let Some(collation) = collation {
                written.push_str(&format!(" collate {collation}"));
            }
            if c.not_null && !constraints.iter().any(|k| k.kind == "primary key" && k.columns.contains(&c.name)) {
                written.push_str(" not null");
            }
            match (&c.generated, &c.default) {
                (Some(expr), _) => written.push_str(&format!(" generated always as ({}) virtual", styled(expr))),
                (None, Some(def)) => written.push_str(&format!(" default {}", styled(def))),
                _ => {}
            }
            parts.push(format!("{} {written}", quote(&c.name)));
            let mut column_deps: BTreeSet<String> = c.default.iter().chain(c.generated.iter()).flat_map(|text| sequences_in(text, &read)).collect();
            column_deps.extend(type_dep);
            column_deps.extend(expression_deps(&c.default.iter().chain(c.generated.iter()).map(String::as_str).collect::<Vec<_>>()));
            deps.extend(column_deps.iter().cloned());
            let mut flags = String::new();
            if c.generated.is_some() {
                flags.push('g');
            }
            if c.not_null && c.default.is_none() && c.generated.is_none() {
                flags.push('n');
            }
            for k in &constraints {
                if k.columns.contains(&c.name) || (k.kind == "check" && mentions(&k.expression).contains(&c.name.to_lowercase())) {
                    flags.push(match k.kind.as_str() {
                        "primary key" | "unique" => 'k',
                        "check" => 'c',
                        _ => 'f',
                    });
                }
            }
            s.add(Object {
                id: format!("column {name}.{}", quote(&c.name)),
                kind: Some(Kind::Column),
                owner: Some(id.clone()),
                def,
                inline: true,
                change: Change::Alter { blocks: false },
                deps: column_deps,
                extra: vec![format!("name{SEP}{}", quote(&c.name)), format!("position{SEP}{}", c.position), format!("flags{SEP}{flags}"), format!("written{SEP}{}", parts.last().cloned().unwrap_or_default())],
                ..Object::default()
            });
            if let Some(comment) = &c.comment {
                add_comment(&mut s, &format!("column {name}.{}", quote(&c.name)), "column", &format!("{name}.{}", quote(&c.name)), comment);
            }
        }
        // The macros and types its defaults, generated columns and checks name.
        let texts: Vec<&str> = columns.iter().flat_map(|c| c.default.iter().chain(c.generated.iter())).chain(constraints.iter().filter(|k| k.kind == "check").map(|k| &k.expression)).map(String::as_str).collect();
        deps.extend(expression_deps(&texts));
        for k in &constraints {
            lines.push(constraint_line(k));
            parts.push(constraint_sql(k));
            if let Some((to, referenced)) = &k.references {
                // A foreign key names its table without a schema: the table's own schema's, else main's.
                let target = [qualified(&t.schema, to), quote(to)].into_iter().find(|n| read.tables.iter().any(|x| qualified(&x.schema, &x.name) == *n)).unwrap_or_else(|| quote(to));
                let target_id = format!("table {target}");
                if target_id != id {
                    deps.insert(target_id.clone());
                } else {
                    // A key of the table's own, each column with the one it references.
                    for (c, r) in k.columns.iter().zip(referenced) {
                        info.push(format!("selfref{SEP}{}{SEP}{}", quote(c), quote(r)));
                    }
                }
                info.push(format!("references{SEP}{target_id}"));
            }
            info.push(format!("constraint{SEP}{}", constraint_sql(k)));
        }
        lines.sort();
        let def = format!("table\n{}", lines.join("\n"));
        let create = format!("create table {name} ({})", parts.join(", "));
        info.push(format!("create{SEP}{create}"));
        s.add(Object { id: id.clone(), kind: Some(Kind::Table), def, create: vec![create], drop: vec![format!("drop table {name}")], deps, change: Change::Alter { blocks: false }, extra: info, ..Object::default() });
        if let Some(c) = &t.comment {
            add_comment(&mut s, &id, "table", &name, c);
        }
    }
    // What views and macros name: the tables, views and macros of the database, and those tables'
    // columns, which DuckDB keeps no record of, read from their words.
    let mut relations: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, id) in read
        .tables
        .iter()
        .map(|t| (t.name.to_lowercase(), format!("table {}", qualified(&t.schema, &t.name))))
        .chain(read.views.iter().map(|v| (v.name.to_lowercase(), format!("view {}", qualified(&v.schema, &v.name)))))
        .chain(read.macros.iter().map(|m| (m.name.to_lowercase(), macro_id(&m.schema, &m.name, m.table))))
    {
        relations.entry(name).or_default().push(id);
    }
    let deps_of = |text: &str, own: &str, params: &BTreeSet<String>| -> BTreeSet<String> {
        let named: BTreeSet<String> = mentions(text).difference(params).cloned().collect();
        let mut out = BTreeSet::new();
        for id in named.iter().filter_map(|n| relations.get(n)).flatten() {
            if id == own {
                continue;
            }
            out.insert(id.clone());
            if let Some(table) = id.strip_prefix("table ") {
                for c in read.columns.iter().filter(|c| qualified(&c.schema, &c.table) == table && named.contains(&c.name.to_lowercase())) {
                    out.insert(format!("column {table}.{}", quote(&c.name)));
                }
            }
        }
        out
    };
    for i in &read.indexes {
        let name = qualified(&i.schema, &i.name);
        let id = format!("index {name}");
        let table = format!("table {}", qualified(&i.schema, &i.table));
        let deps = expression_deps(&[i.sql.as_str()]);
        s.add(Object { id: id.clone(), kind: Some(Kind::Index), owner: Some(table), def: i.sql.clone(), create: vec![styled_named(&i.sql)], drop: vec![format!("drop index {name}")], deps, ..Object::default() });
        if let Some(c) = &i.comment {
            add_comment(&mut s, &id, "index", &name, c);
        }
    }
    for v in &read.views {
        let name = qualified(&v.schema, &v.name);
        let id = format!("view {name}");
        let mut deps = deps_of(&v.sql, &id, &BTreeSet::new());
        deps.extend(schema_dep(&v.schema));
        s.add(Object { id: id.clone(), kind: Some(Kind::View), def: v.sql.clone(), create: vec![styled_named(&v.sql)], drop: vec![format!("drop view {name}")], deps, ..Object::default() });
        if let Some(c) = &v.comment {
            add_comment(&mut s, &id, "view", &name, c);
        }
    }
    for m in &read.macros {
        let name = qualified(&m.schema, &m.name);
        let id = macro_id(&m.schema, &m.name, m.table);
        let def = macro_def(m);
        let create = m.exported.clone().unwrap_or_else(|| format!("create macro {name} {def}"));
        // A parameter's name is no table's, whatever it shares a name with.
        let params: BTreeSet<String> = m.overloads.iter().flat_map(|(p, _)| p.iter().map(|(n, _)| n.to_lowercase())).collect();
        let mut deps = deps_of(&def, &id, &params);
        deps.extend(schema_dep(&m.schema));
        s.add(Object {
            id: id.clone(),
            kind: Some(Kind::Function),
            def,
            create: vec![styled_named(&create)],
            drop: vec![format!("drop macro {}{name}", if m.table { "table " } else { "" })],
            deps,
            // Made again by its alter, around what DuckDB records as using it.
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
        if let Some(c) = &m.comment {
            add_comment(&mut s, &id, if m.table { "macro table" } else { "macro" }, &name, c);
        }
    }
    for (schema, view, column, text) in &read.view_comments {
        let view_id = format!("view {}", qualified(schema, view));
        let name = format!("{}.{}", qualified(schema, view), quote(column));
        s.add(Object {
            id: format!("comment view column {name}"),
            kind: Some(Kind::Comment),
            owner: Some(view_id),
            def: text.clone(),
            create: vec![format!("comment on column {name} is {}", literal(text))],
            drop: vec![format!("comment on column {name} is null")],
            change: Change::Alter { blocks: false },
            ..Object::default()
        });
    }
    // What DuckDB records, which the names read above may miss.
    // Kept apart too, as what DuckDB refuses to drop under it.
    for (dependent, on) in &read.depends {
        if dependent != on && s.objects.contains_key(on) {
            if let Some(o) = s.objects.get_mut(dependent) {
                o.deps.insert(on.clone());
                o.extra.push(format!("recorded{SEP}{on}"));
            }
        }
    }
    // A macro or a type a table's expressions name is made again around the table: its indexes and
    // comments, and theirs, wait for that, so that one of them changed is not made before it.
    let around: Vec<(String, BTreeSet<String>)> = s
        .objects
        .values()
        .filter(|o| o.kind == Some(Kind::Table))
        .map(|o| (o.id.clone(), o.deps.iter().filter(|d| d.starts_with("macro ") || d.starts_with("type ")).cloned().collect::<BTreeSet<_>>()))
        .filter(|(_, d)| !d.is_empty())
        .collect();
    for (table, deps) in around {
        let on: Vec<String> = s
            .objects
            .values()
            .filter(|o| matches!(o.kind, Some(Kind::Index | Kind::Comment)))
            .filter(|o| o.owner.as_deref() == Some(table.as_str()) || o.owner.as_deref().and_then(|w| s.objects.get(w)).is_some_and(|w| matches!(w.kind, Some(Kind::Column | Kind::Index)) && w.owner.as_deref() == Some(table.as_str())))
            .map(|o| o.id.clone())
            .collect();
        for id in on {
            if let Some(o) = s.objects.get_mut(&id) {
                o.deps.extend(deps.iter().cloned());
            }
        }
    }
    Ok(s)
}

/// A statement DuckDB wrote, in lowercase but for the name it makes: `CREATE INDEX Ix ON t(ID)`.
fn styled_named(sql: &str) -> String {
    let tokens = tokenize(sql, Dialect::Duckdb);
    let at = tokens.iter().take(5).position(|t| matches!(t.upper().as_str(), "INDEX" | "VIEW" | "MACRO" | "FUNCTION" | "TABLE" | "SEQUENCE" | "TYPE")).map_or(0, |k| k + 1);
    let name_end = if tokens.get(at + 1).is_some_and(|t| t.text == ".") { at + 3 } else { at + 1 };
    let (Some(start), Some(end)) = (tokens.get(at).map(|t| t.start), tokens.get(name_end.saturating_sub(1)).map(|t| t.end)) else { return styled(sql) };
    format!("{}{}{}", styled(&sql[..start]), &sql[start..end], styled(&sql[end..]))
}

fn add_comment(s: &mut Schema, owner: &str, kind: &str, name: &str, text: &str) {
    s.add(Object {
        id: format!("comment {owner}"),
        kind: Some(Kind::Comment),
        owner: Some(owner.to_string()),
        def: text.to_string(),
        create: vec![format!("comment on {kind} {name} is {}", literal(text))],
        drop: vec![format!("comment on {kind} {name} is null")],
        change: Change::Alter { blocks: false },
        ..Object::default()
    });
}

/// Rewrites the create statements of `schema`, read from the database `schema.sql` built, from the
/// text the file declared each macro with, which holds what no catalog function gives: its
/// parameters' defaults.
pub fn declare(schema: &mut Schema, sql: &str) {
    for statement in crate::sql::split_statements(sql, Dialect::Duckdb) {
        let words: Vec<String> = statement.tokens.iter().take(5).map(Token::upper).collect();
        if words.first().map(String::as_str) != Some("CREATE") {
            continue;
        }
        let Some(k) = words.iter().position(|w| w == "MACRO" || w == "FUNCTION") else { continue };
        if words[1..k].iter().any(|w| !matches!(w.as_str(), "OR" | "REPLACE")) {
            continue;
        }
        let (schema_name, name) = name_at(&statement.tokens, k + 1);
        let id = macro_id(&schema_name, &name, makes_table_macro(&statement.tokens));
        if let Some(o) = schema.objects.get_mut(&id) {
            o.create = vec![statement.text.clone()];
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The alterer.

#[derive(Default)]
pub struct DuckdbAlterer {
    decided: super::Decided,
    schemas: std::cell::RefCell<Option<(Schema, Schema)>>,
}

/// A table's facts, from its object and its columns'.
struct Table<'a> {
    o: &'a Object,
    name: String,
    columns: Vec<&'a Object>,
}

fn field<'a>(o: &'a Object, key: &str) -> Option<&'a str> {
    o.extra.iter().find_map(|l| l.strip_prefix(key).and_then(|rest| rest.strip_prefix(SEP)))
}

fn fields<'a>(o: &'a Object, key: &str) -> Vec<&'a str> {
    o.extra.iter().filter_map(|l| l.strip_prefix(key).and_then(|rest| rest.strip_prefix(SEP))).collect()
}

impl<'a> Table<'a> {
    fn of(schema: &'a Schema, o: &'a Object) -> Table<'a> {
        let mut columns: Vec<&Object> = schema.objects.values().filter(|c| c.kind == Some(Kind::Column) && c.owner.as_deref() == Some(o.id.as_str())).collect();
        columns.sort_by_key(|c| field(c, "position").and_then(|p| p.parse::<i64>().ok()).unwrap_or(0));
        Table { o, name: field(o, "name").unwrap_or_default().to_string(), columns }
    }
    fn column(&self, name: &str) -> Option<&'a Object> {
        self.columns.iter().copied().find(|c| field(c, "name") == Some(name))
    }
    fn names(&self) -> Vec<String> {
        self.columns.iter().map(|c| field(c, "name").unwrap_or_default().to_string()).collect()
    }
    fn constraints(&self) -> BTreeSet<String> {
        fields(self.o, "constraint").into_iter().map(String::from).collect()
    }
}

fn flag(c: &Object, f: char) -> bool {
    field(c, "flags").is_some_and(|x| x.contains(f))
}

/// The tables of `from` whose foreign keys reference the table `id`, at any depth, that the plan
/// keeps: each after every one of them it references.
fn children_of<'a>(from: &'a Schema, to: &Schema, id: &str) -> Vec<&'a Object> {
    // One that keeps its key on the table: one that moves it away changes on its own.
    let references = |o: &Object, parent: &str| fields(o, "references").contains(&parent) && to.objects.get(&o.id).is_some_and(|t| fields(t, "references").contains(&parent));
    let mut seen: Vec<String> = vec![id.to_string()];
    let mut family: Vec<&Object> = Vec::new();
    let mut k = 0;
    while k < seen.len() {
        let parent = seen[k].clone();
        k += 1;
        for o in from.objects.values().filter(|o| o.kind == Some(Kind::Table) && to.objects.contains_key(&o.id) && references(o, &parent)) {
            if !seen.contains(&o.id) {
                seen.push(o.id.clone());
                family.push(o);
            }
        }
    }
    let mut placed: BTreeSet<String> = BTreeSet::from([id.to_string()]);
    let mut ordered = Vec::new();
    while !family.is_empty() {
        let at = family.iter().position(|o| fields(o, "references").iter().all(|r| !seen.contains(&r.to_string()) || placed.contains(*r))).unwrap_or(0);
        let o = family.remove(at);
        placed.insert(o.id.clone());
        ordered.push(o);
    }
    ordered
}

/// How a table changes: in place, with its own indexes dropped around the change where they would
/// refuse it, or by a rebuild.
#[derive(Debug, PartialEq, Eq)]
enum Mode {
    InPlace { around_indexes: bool },
    Rebuild,
}

impl DuckdbAlterer {
    fn schemas(&self) -> std::cell::Ref<'_, Option<(Schema, Schema)>> {
        self.schemas.borrow()
    }

    /// The renames of the table's columns the planner decided, new name to old, by written name.
    fn renames_of(&self, table_id: &str, to: &Schema) -> BTreeMap<String, String> {
        let table = table_id.strip_prefix("table ").unwrap_or(table_id);
        self.decided
            .of(table)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(old, new)| {
                let new_name = field(to.objects.get(&format!("column {table}.{new}"))?, "name")?.to_string();
                Some((new_name, old))
            })
            .collect()
    }

    fn mode(&self, from: &Object, to: &Object) -> Mode {
        let schemas = self.schemas();
        let Some((fs, ts)) = schemas.as_ref() else { return Mode::Rebuild };
        let (a, b) = (Table::of(fs, from), Table::of(ts, to));
        let renamed = self.renames_of(&to.id, ts);
        let old_of = |new: &str| renamed.get(new).cloned().unwrap_or_else(|| new.to_string());
        // The constraints as they read before the renames, which DuckDB carries through a rename.
        let renamed_back = |text: &str| renamed.iter().fold(text.to_string(), |t, (new, old)| super::swap_name(&t, new, old));
        let target_constraints: BTreeSet<String> = b.constraints().iter().map(|c| renamed_back(c)).collect();
        if a.constraints() != target_constraints {
            return Mode::Rebuild;
        }
        let referenced = fs.objects.values().any(|o| o.kind == Some(Kind::Table) && o.id != from.id && ts.objects.contains_key(&o.id) && fields(o, "references").contains(&from.id.as_str()));
        // A column an index covers DuckDB changes in no way while the index stands, nor once it is
        // dropped earlier in the same transaction: such a change is a rebuild.
        let mut indexed: BTreeSet<String> = fs.objects.values().filter(|o| o.kind == Some(Kind::Index) && o.owner.as_deref() == Some(from.id.as_str())).flat_map(|o| mentions(&o.def)).collect();
        // Nor one a generated column reads.
        indexed.extend(a.columns.iter().filter_map(|c| column_change(&c.def).generated).flat_map(|e| mentions(&e)));
        let mut blocked = false;
        let new_names: BTreeSet<String> = b.names().iter().map(|n| old_of(n)).collect();
        for c in &a.columns {
            let name = field(c, "name").unwrap_or_default();
            let new_name = renamed.iter().find(|(_, old)| old.as_str() == name).map(|(new, _)| new.clone()).unwrap_or_else(|| name.to_string());
            match b.column(&new_name) {
                None if !new_names.contains(name) => {
                    // A drop: refused where a key covers a column after it, or where something names it.
                    let at = field(c, "position").and_then(|p| p.parse::<i64>().ok()).unwrap_or(0);
                    // DuckDB drops no column before one a key, a foreign key or an index covers.
                    let later_key = a.columns.iter().any(|x| (flag(x, 'k') || flag(x, 'f') || indexed.contains(&field(x, "name").unwrap_or_default().to_lowercase())) && field(x, "position").and_then(|p| p.parse::<i64>().ok()).unwrap_or(0) > at);
                    if later_key || flag(c, 'k') || flag(c, 'c') || flag(c, 'f') || flag(c, 'g') || indexed.contains(&name.to_lowercase()) {
                        return Mode::Rebuild;
                    }
                    blocked = true;
                }
                None => {}
                Some(n) => {
                    let covered = indexed.contains(&name.to_lowercase());
                    if new_name != name {
                        // Nor renames a column a foreign key of its table's names.
                        if covered || flag(c, 'f') {
                            return Mode::Rebuild;
                        }
                        blocked = true;
                    }
                    let (x, y) = (column_change(&c.def), column_change(&n.def));
                    if x.kind != y.kind || x.collation != y.collation || x.generated != y.generated {
                        if x.generated.is_some() || y.generated.is_some() || x.collation != y.collation || flag(c, 'k') || flag(c, 'c') || flag(c, 'f') {
                            return Mode::Rebuild;
                        }
                        if covered {
                            return Mode::Rebuild;
                        }
                        blocked = true;
                    }
                    if x.not_null != y.not_null {
                        if covered {
                            return Mode::Rebuild;
                        }
                        blocked = true;
                    }
                }
            }
        }
        for n in &b.columns {
            let name = field(n, "name").unwrap_or_default();
            if a.column(&old_of(name)).is_none() {
                if flag(n, 'g') {
                    return Mode::Rebuild;
                }
                // An index or a foreign key on the table refuses a column added that takes no null.
                if column_change(&n.def).not_null {
                    blocked = true;
                }
            }
        }
        if blocked && referenced {
            return Mode::Rebuild;
        }
        let kept_indexes = !kept_on(fs, ts, &from.id, Kind::Index).is_empty();
        Mode::InPlace { around_indexes: blocked && kept_indexes }
    }
}

/// A column's definition, read apart: its type, collation, nullability, default and generation.
#[derive(Debug, Default, PartialEq, Eq)]
struct ColumnDef {
    kind: String,
    collation: Option<String>,
    not_null: bool,
    default: Option<String>,
    generated: Option<String>,
}

fn column_change(def: &str) -> ColumnDef {
    let mut out = ColumnDef::default();
    let mut rest = def.to_string();
    if let Some(at) = rest.find(" GENERATED ALWAYS AS (") {
        let expr = &rest[at + 22..];
        let expr = expr.strip_suffix(" VIRTUAL").unwrap_or(expr);
        out.generated = Some(expr.strip_suffix(')').unwrap_or(expr).to_string());
        rest.truncate(at);
    } else if let Some(at) = rest.find(" DEFAULT ") {
        out.default = Some(rest[at + 9..].to_string());
        rest.truncate(at);
    }
    if let Some(stripped) = rest.strip_suffix(" NOT NULL") {
        out.not_null = true;
        rest = stripped.to_string();
    }
    if let Some(at) = rest.rfind(" COLLATE ") {
        out.collation = Some(rest[at + 9..].to_string());
        rest.truncate(at);
    }
    out.kind = rest;
    out
}

/// What is on the table `id` of kind `kind` (its indexes, or its comments and its columns'), the
/// same on both sides: a rebuild drops them with the table and makes them again itself.
fn kept_on<'a>(from: &'a Schema, to: &Schema, id: &str, kind: Kind) -> Vec<&'a Object> {
    let on = |o: &Object| -> bool {
        match o.owner.as_deref() {
            Some(owner) if owner == id => true,
            // A comment on one of its columns or indexes.
            Some(owner) => kind == Kind::Comment && from.objects.get(owner).is_some_and(|c| matches!(c.kind, Some(Kind::Column | Kind::Index)) && c.owner.as_deref() == Some(id)),
            None => false,
        }
    };
    let same = |o: &Object| to.objects.get(&o.id).is_some_and(|t| t.def == o.def);
    // A comment on an index the plan makes again is made by the plan after it.
    let owner_kept = |o: &Object| o.owner.as_deref().and_then(|w| from.objects.get(w)).is_none_or(|w| w.kind != Some(Kind::Index) || same(w));
    let mut out: Vec<&Object> = from.objects.values().filter(|o| o.kind == Some(kind) && on(o) && same(o) && owner_kept(o)).collect();
    out.sort_by(|x, y| x.id.cmp(&y.id));
    out
}

impl Alterer for DuckdbAlterer {
    fn schemas(&self, from: &Schema, to: &Schema) {
        *self.schemas.borrow_mut() = Some((from.clone(), to.clone()));
    }

    fn decide(&self, renames: &[(String, String)]) {
        self.decided.set(renames);
    }

    fn waits_for(&self, id: &str) -> Vec<String> {
        let schemas = self.schemas();
        let Some((fs, ts)) = schemas.as_ref() else { return vec![] };
        let changed = changed_under_tables(fs, ts);
        let mut out = Vec::new();
        // A macro, type or sequence made again around tables after what the tables it makes as the
        // target has them need.
        if changed.iter().any(|x| x.id == id) {
            let a = self.around_of(fs, ts, id);
            for m in &a.members {
                if let Some(t) = ts.objects.get(&m.id).filter(|t| t.def != m.def) {
                    out.extend(needs_of(ts, t).into_iter().filter(|d| d != id && !a.members.iter().any(|x| x.id == *d)));
                }
            }
            return out;
        }
        let Some(table) = table_of(ts, id).or_else(|| table_of(fs, id)) else { return out };
        // A table that one is made again around, and what is on it, after that.
        for x in &changed {
            if self.around_of(fs, ts, &x.id).touches(&table) {
                out.push(x.id.clone());
            }
        }
        // A rebuild after what the tables it makes as the target has them need.
        if id == table {
            if let (Some(a), Some(b)) = (fs.objects.get(id), ts.objects.get(id)) {
                if a.def != b.def && self.mode(a, b) == Mode::Rebuild {
                    for c in children_of(fs, ts, id).iter().filter(|c| self.absorbed(&c.id)) {
                        if let Some(t) = ts.objects.get(&c.id) {
                            out.extend(needs_of(ts, t).into_iter().filter(|d| d != id && *d != c.id));
                        }
                    }
                }
            }
        }
        out
    }

    fn narrows(&self, from: &Object, to: &Object) -> Option<String> {
        if from.kind != Some(Kind::Column) {
            return None;
        }
        let (a, b) = (column_change(&from.def).kind, column_change(&to.def).kind);
        let why = loss(&parse_type(&a), &parse_type(&b))?;
        Some(format!("changes from {} to {}, which {why}", styled_type(&a), styled_type(&b)))
    }

    fn rename(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        let (table_from, table_to) = (from.owner.as_deref()?, to.owner.as_deref()?);
        let schemas = self.schemas();
        let (fs, ts) = schemas.as_ref()?;
        let (tf, tt) = (fs.objects.get(table_from)?, ts.objects.get(table_to)?);
        let mode = if tf.def == tt.def { Mode::InPlace { around_indexes: false } } else { self.mode(tf, tt) };
        // The table's own statements rename it, where they rebuild it or drop indexes around it, and
        // so does the rebuild of a table it references, which makes it again as the target has it.
        if mode != (Mode::InPlace { around_indexes: false }) || self.absorbed(&tf.id) || self.around_absorbed(&tf.id) {
            return Some(vec![]);
        }
        let table = field(tf, "name")?;
        Some(vec![format!("alter table {table} rename column {} to {}", field(from, "name")?, field(to, "name")?)])
    }

    fn alter(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        match to.kind? {
            // A column changes with its table, whose alter sees the whole of both.
            Kind::Column => Some(vec![]),
            Kind::Comment => Some(to.create.clone()),
            // DuckDB alters no macro, type or sequence: each is made again, around what DuckDB records
            // as using it.
            Kind::Type | Kind::Function | Kind::Sequence => {
                let schemas = self.schemas();
                let (fs, ts) = schemas.as_ref()?;
                let mut out = Vec::new();
                let mut create = to.create.clone();
                if to.kind == Some(Kind::Sequence) {
                    // On the database a plan is made from, a sequence goes on from the value it gives
                    // next; in a migration, which runs on databases whose values no plan can know, it
                    // starts where the target's starts, which the file says.
                    let (start, next) = (field(to, "start").unwrap_or_default(), field(from, "next").unwrap_or_default());
                    if fs.live {
                        create = vec![to.create[0].replacen(&format!(" start with {start} "), &format!(" start with {next} "), 1)];
                    } else {
                        out.push(format!("-- DuckDB makes {} again, so it starts again at {start}: where rows hold its values, start it past them", from.id.trim_start_matches("sequence ")));
                    }
                }
                let between: Vec<String> = from.drop.iter().cloned().chain(create).chain(unchanged_comment(fs, ts, &to.id)).collect();
                out.extend(self.around(fs, ts, &from.id, between));
                Some(out)
            }
            Kind::Table => self.alter_table(from, to),
            _ => None,
        }
    }
}

impl DuckdbAlterer {
    /// The column of `to` that the column `c` of `from` becomes: by its id, or by the rename decided.
    fn target_column<'a>(&self, ts: &'a Schema, c: &Object) -> Option<&'a Object> {
        let table = c.owner.as_deref()?;
        let name = field(c, "name")?;
        let new = self.renames_of(table, ts).into_iter().find(|(_, old)| old == name).map(|(new, _)| new).unwrap_or_else(|| name.to_string());
        let table_name = table.strip_prefix("table ").unwrap_or(table);
        ts.objects.get(&format!("column {table_name}.{new}"))
    }

    /// The defaults `taken` off their columns set back as the target has them, by the columns' names
    /// as they are, where the target keeps them as they were: one that changes is set by its
    /// table's own statements, which run after.
    fn defaults_back(&self, fs: &Schema, ts: &Schema, taken: &[&Object]) -> Vec<String> {
        taken
            .iter()
            .filter_map(|c| {
                let target = self.target_column(ts, c)?;
                let default = column_change(&target.def).default?;
                (Some(default) == column_change(&c.def).default).then(|| format!("alter table {} alter column {} set default {}", table_name(fs, c), field(c, "name").unwrap_or_default(), written_default(target)))
            })
            .collect()
    }

    /// What DuckDB records as using the macro, type or sequence `id`, which it is made again around:
    /// the tables whose checks or generated columns reach it (or that it records as using it for
    /// another reason), with every table that references them and every table rebuilt in the plan
    /// that one of those references; the columns of other tables whose defaults reach it; the
    /// indexes of other tables that reach it; and the types written with it.
    fn around_of<'a>(&self, fs: &'a Schema, ts: &'a Schema, id: &str) -> Around<'a> {
        let mut out = Around::default();
        let Some(x) = fs.objects.get(id) else { return out };
        let reaches = |text: &str| reaches(fs, text, x);
        let in_both = |o: &&Object| ts.objects.contains_key(&o.id);
        let columns_of = |t: &str| -> Vec<&'a Object> { fs.objects.values().filter(|c| c.kind == Some(Kind::Column) && c.owner.as_deref() == Some(t)).collect() };
        let mut members: Vec<&Object> = fs
            .objects
            .values()
            .filter(|t| t.kind == Some(Kind::Table))
            .filter(in_both)
            .filter(|t| {
                let checked = fields(t, "constraint").iter().any(|k| k.starts_with("check ") && reaches(k)) || columns_of(&t.id).iter().any(|c| column_change(&c.def).generated.is_some_and(|g| reaches(&g)));
                let defaulted = columns_of(&t.id).iter().any(|c| column_change(&c.def).default.is_some_and(|d| reaches(&d)));
                checked || (fields(t, "recorded").contains(&id) && !defaulted)
            })
            .collect();
        // With every table that references one, and every table rebuilt in the plan that one of those
        // references, which would make it again otherwise.
        loop {
            let before = members.len();
            for k in 0..members.len() {
                for child in children_of(fs, ts, &members[k].id) {
                    if !members.iter().any(|m| m.id == child.id) {
                        members.push(child);
                    }
                }
            }
            let parents: Vec<&Object> = fs
                .objects
                .values()
                .filter(|p| p.kind == Some(Kind::Table) && !members.iter().any(|m| m.id == p.id))
                .filter(|p| ts.objects.get(&p.id).is_some_and(|t| t.def != p.def && children_of(fs, ts, &p.id).iter().any(|c| members.iter().any(|m| m.id == c.id)) && self.mode(p, t) == Mode::Rebuild))
                .collect();
            members.extend(parents);
            if members.len() == before {
                break;
            }
        }
        let member = |table: &str| members.iter().any(|m| m.id == table);
        out.taken = fs
            .objects
            .values()
            .filter(|c| c.kind == Some(Kind::Column) && c.owner.as_deref().is_some_and(|t| ts.objects.contains_key(t) && !member(t)))
            .filter(|c| column_change(&c.def).default.is_some_and(|d| reaches(&d)))
            .collect();
        out.indexes = fs
            .objects
            .values()
            .filter(|i| i.kind == Some(Kind::Index) && (fields(i, "recorded").contains(&id) || reaches(&i.def)) && !member(i.owner.as_deref().unwrap_or_default()) && ts.objects.get(&i.id).is_some_and(|t| t.def == i.def))
            .collect();
        out.types = fs.objects.values().filter(|t| t.kind == Some(Kind::Type) && t.id != id && fields(t, "recorded").contains(&id)).filter(in_both).collect();
        // Each after the tables it references, as it is and as it will be.
        let mut left = members;
        while !left.is_empty() {
            let refs = |o: &Object| -> Vec<String> { fields(o, "references").into_iter().chain(ts.objects.get(&o.id).map(|t| fields(t, "references")).unwrap_or_default()).map(String::from).collect() };
            let at = left.iter().position(|o| refs(o).iter().all(|r| *r == o.id || !left.iter().any(|x| x.id == *r))).unwrap_or(0);
            out.members.push(left.remove(at));
        }
        out
    }

    /// Whether the table `id` is made again as the target has it around a macro, type or sequence
    /// made again, so that its own statements are none.
    fn around_absorbed(&self, id: &str) -> bool {
        let schemas = self.schemas();
        let Some((fs, ts)) = schemas.as_ref() else { return false };
        let (Some(a), Some(b)) = (fs.objects.get(id), ts.objects.get(id)) else { return false };
        a.def != b.def && changed_under_tables(fs, ts).iter().any(|x| self.around_of(fs, ts, &x.id).members.iter().any(|m| m.id == id))
    }

    /// The statements that make the macro, type or sequence `id` again, `between`, around what
    /// DuckDB records as using it: a default that reaches it is taken off its column and set back
    /// after, an index that does is dropped and made again after, a type written with it is dropped
    /// and made again as the target writes it, and a table whose check or generated column reaches
    /// it is rebuilt around it, with its family, each that changes made as the target has it.
    fn around(&self, fs: &Schema, ts: &Schema, id: &str, between: Vec<String>) -> Vec<String> {
        let a = self.around_of(fs, ts, id);
        let mut out: Vec<String> = a.taken.iter().map(|c| format!("alter table {} alter column {} drop default", table_name(fs, c), field(c, "name").unwrap_or_default())).collect();
        out.extend(a.indexes.iter().map(|i| i.drop[0].clone()));
        let mut between_all: Vec<String> = a.types.iter().map(|t| t.drop[0].clone()).collect();
        between_all.extend(between);
        for t in &a.types {
            between_all.extend(ts.objects.get(&t.id).map(|x| x.create[0].clone()));
            between_all.extend(unchanged_comment(fs, ts, &t.id));
        }
        if a.members.is_empty() {
            out.extend(between_all);
        } else {
            let members = self.members_of(fs, ts, &a.members, &|m: &Object| ts.objects.get(&m.id).is_some_and(|t| t.def != m.def));
            let mut after = Vec::new();
            for m in &members {
                after.extend(kept_on(fs, ts, &m.from.id, Kind::Index).iter().map(|i| i.create[0].clone()));
                after.extend(kept_on(fs, ts, &m.from.id, Kind::Comment).iter().map(|c| c.create[0].clone()));
            }
            out.extend(rebuild_family(&members, between_all, after));
        }
        out.extend(self.defaults_back(fs, ts, &a.taken));
        for i in &a.indexes {
            out.push(i.create[0].clone());
            out.extend(unchanged_comment(fs, ts, &i.id));
        }
        out
    }

    /// The tables of a family made again: each `targeted` takes as the target has it, the rest as
    /// they are, but for their keys on a column of one made as the target has it, which follow its
    /// rename and take its type.
    fn members_of<'a>(&self, fs: &'a Schema, ts: &'a Schema, family: &[&'a Object], targeted: &dyn Fn(&Object) -> bool) -> Vec<Member<'a>> {
        // Each made as the target has it: its name as its keys are written, and its columns' new names.
        let made: Vec<(String, &Object, &Object, BTreeMap<String, String>)> = family
            .iter()
            .filter(|x| targeted(x))
            .filter_map(|x| {
                let t = ts.objects.get(&x.id)?;
                let new_of_old = self.renames_of(&x.id, ts).into_iter().map(|(n, o)| (o, n)).collect();
                Some((super::last_written(field(x, "name").unwrap_or_default()), *x, t, new_of_old))
            })
            .collect();
        family
            .iter()
            .map(|x| {
                if let Some(t) = ts.objects.get(&x.id).filter(|_| targeted(x)) {
                    return Member { from: x, create: field(t, "create").unwrap_or_default().to_string(), name: field(t, "name").unwrap_or_default().to_string(), pairs: pairs_of(fs, ts, x, t, &self.renames_of(&x.id, ts)) };
                }
                let mut retype: BTreeMap<String, String> = BTreeMap::new();
                for (bare, f, t, new_of_old) in &made {
                    let (a, b) = (Table::of(fs, f), Table::of(ts, t));
                    for (columns, table, referenced) in fields(x, "constraint").iter().filter_map(|k| foreign_key(k)) {
                        if !table.eq_ignore_ascii_case(bare.trim_matches('"')) {
                            continue;
                        }
                        for (c, r) in columns.iter().zip(&referenced) {
                            let (Some(old), Some(new)) = (a.column(&quote(r)), b.column(new_of_old.get(&quote(r)).map_or(&quote(r), |n| n))) else { continue };
                            if column_change(&old.def).kind == column_change(&new.def).kind {
                                continue;
                            }
                            let Some(column) = Table::of(fs, x).column(&quote(c)) else { continue };
                            retype.insert(quote(c), self.target_column(ts, column).map_or_else(|| written_type(new), written_type));
                        }
                    }
                }
                let mut create = create_retyped(fs, x, &retype);
                for (bare, _, _, new_of_old) in &made {
                    create = swap_referenced_columns(&create, bare, new_of_old);
                }
                Member { from: x, create, name: field(x, "name").unwrap_or_default().to_string(), pairs: copied(fs, x) }
            })
            .collect()
    }
}

/// What a macro, type or sequence is made again around (`DuckdbAlterer::around_of`).
#[derive(Default)]
struct Around<'a> {
    members: Vec<&'a Object>,
    taken: Vec<&'a Object>,
    indexes: Vec<&'a Object>,
    types: Vec<&'a Object>,
}

impl Around<'_> {
    /// Whether the table `id` is among what this changes.
    fn touches(&self, id: &str) -> bool {
        self.members.iter().any(|m| m.id == id) || self.taken.iter().chain(&self.indexes).any(|o| o.owner.as_deref() == Some(id))
    }
}

/// The scalar macros, types and sequences the plan makes again, which what uses them waits for.
fn changed_under_tables<'a>(fs: &'a Schema, ts: &Schema) -> Vec<&'a Object> {
    fs.objects
        .values()
        .filter(|x| matches!(x.kind, Some(Kind::Type | Kind::Sequence)) || (x.kind == Some(Kind::Function) && !x.id.starts_with("macro table ")))
        .filter(|x| ts.objects.get(&x.id).is_some_and(|t| t.def != x.def))
        .collect()
}

/// The table the object `id` is, or is on (a column, an index, a comment on any of them).
fn table_of(schema: &Schema, id: &str) -> Option<String> {
    let mut at = schema.objects.get(id)?;
    for _ in 0..4 {
        if at.kind == Some(Kind::Table) {
            return Some(at.id.clone());
        }
        at = schema.objects.get(at.owner.as_deref()?)?;
    }
    None
}

/// What a table needs there before it is made: what it and its columns depend on.
fn needs_of(schema: &Schema, t: &Object) -> BTreeSet<String> {
    let mut out = t.deps.clone();
    for c in schema.objects.values().filter(|c| c.kind == Some(Kind::Column) && c.owner.as_deref() == Some(t.id.as_str())) {
        out.extend(c.deps.iter().cloned());
    }
    out
}

/// Whether an expression reaches the macro, type or sequence `x`: names it, or calls a macro whose
/// body reaches it, as DuckDB records a table's dependencies.
fn reaches(schema: &Schema, text: &str, x: &Object) -> bool {
    let qualified_name = x.id.trim_start_matches("macro ").trim_start_matches("type ").trim_start_matches("sequence ");
    let bare = super::last_written(qualified_name).trim_matches('"').replace("\"\"", "\"").to_lowercase();
    // A sequence by the text that names it, `nextval('s')`, in any of the ways it may be written.
    let tokens = tokenize(qualified_name, Dialect::Duckdb);
    let in_schema = if tokens.len() == 3 && tokens[1].text == "." { unquote(&tokens[0]).to_lowercase() } else { "main".to_string() };
    let quoted = |n: &str| format!("\"{}\"", n.replace('"', "\"\""));
    let mut forms = vec![format!("{in_schema}.{bare}"), format!("{in_schema}.{}", quoted(&bare)), format!("{}.{}", quoted(&in_schema), quoted(&bare))];
    if in_schema == "main" {
        forms.extend([bare.clone(), quoted(&bare)]);
    }
    let names_it = |t: &str| -> bool {
        if x.kind == Some(Kind::Sequence) {
            let lower = t.to_lowercase();
            forms.iter().any(|f| lower.contains(&format!("'{f}'")))
        } else {
            mentions(t).contains(&bare)
        }
    };
    let mut queue = vec![text.to_string()];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    while let Some(t) = queue.pop() {
        if names_it(&t) {
            return true;
        }
        let named = mentions(&t);
        for m in schema.objects.values().filter(|m| m.kind == Some(Kind::Function) && !m.id.starts_with("macro table ")) {
            let name = super::last_written(m.id.trim_start_matches("macro ")).trim_matches('"').to_lowercase();
            if named.contains(&name) && seen.insert(m.id.clone()) {
                queue.push(m.def.clone());
            }
        }
    }
    false
}

/// The name of the table that holds the column `c`, as a statement writes it.
fn table_name(schema: &Schema, c: &Object) -> String {
    c.owner.as_deref().and_then(|t| schema.objects.get(t)).and_then(|t| field(t, "name")).unwrap_or_default().to_string()
}

/// A DuckDB type read apart: a list's or an array's element, a struct's fields, a map's key and
/// value, or a type of no parts.
#[derive(Debug, PartialEq)]
enum Ty {
    Scalar(String),
    List(Box<Ty>),
    Struct(Vec<(String, Ty)>),
    /// A union's members, each its tag and type.
    Union(Vec<(String, Ty)>),
    Map(Box<Ty>, Box<Ty>),
}

fn parse_type(t: &str) -> Ty {
    let t = t.trim();
    // A list or an array of a type: `INTEGER[]`, `INTEGER[3]`.
    if t.ends_with(']') {
        let mut depth = 0;
        for (k, c) in t.char_indices().rev() {
            match c {
                ']' => depth += 1,
                '[' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ty::List(Box::new(parse_type(&t[..k])));
                    }
                }
                _ => {}
            }
        }
    }
    let upper = t.to_ascii_uppercase();
    let inner = |prefix: &str| -> Option<Vec<String>> {
        let rest = upper.strip_prefix(prefix)?.trim_start();
        let rest = rest.strip_prefix('(')?.strip_suffix(')')?;
        let start = t.len() - rest.len() - 1;
        Some(split_top(&t[start..start + rest.len()]))
    };
    let fields_of = |fields: Vec<String>| -> Vec<(String, Ty)> {
        fields
            .iter()
            .filter_map(|f| {
                let tokens = tokenize(f, Dialect::Duckdb);
                let name = unquote(tokens.first()?);
                Some((name, parse_type(f[tokens.first()?.end..].trim())))
            })
            .collect()
    };
    if let Some(fields) = inner("STRUCT") {
        return Ty::Struct(fields_of(fields));
    }
    if let Some(members) = inner("UNION") {
        return Ty::Union(fields_of(members));
    }
    if let Some(parts) = inner("MAP") {
        if let [k, v] = &parts[..] {
            return Ty::Map(Box::new(parse_type(k)), Box::new(parse_type(v)));
        }
    }
    Ty::Scalar(t.to_string())
}

/// A type's parts, split at the commas of its parentheses' top level.
fn split_top(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, 0);
    let mut quoted = false;
    for (k, c) in text.char_indices() {
        match c {
            '\'' | '"' => quoted = !quoted,
            '(' | '[' if !quoted => depth += 1,
            ')' | ']' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                out.push(text[start..k].trim().to_string());
                start = k + 1;
            }
            _ => {}
        }
    }
    out.push(text[start..].trim().to_string());
    out
}

/// What a change of a value's type from `a` to `b` loses without failing, as the end of a
/// sentence; a struct's field, a list's element and a map's key or value each lose as their type does.
fn loss(a: &Ty, b: &Ty) -> Option<String> {
    match (a, b) {
        (Ty::Scalar(x), Ty::Scalar(y)) => cast_loss(x, y).or_else(|| super::narrowing(&type_class(x), &type_class(y))),
        (Ty::List(x), Ty::List(y)) => loss(x, y).map(|w| format!("{w}, in each element")),
        (Ty::Map(ka, va), Ty::Map(kb, vb)) => loss(ka, kb).map(|w| format!("{w}, in each key")).or_else(|| loss(va, vb).map(|w| format!("{w}, in each value"))),
        (Ty::Struct(fa), Ty::Struct(fb)) => parts_loss(fa, fb, "field", "fields"),
        (Ty::Union(fa), Ty::Union(fb)) => parts_loss(fa, fb, "member", "members"),
        _ => None,
    }
}

/// What a change of a struct's fields or a union's members loses: those it drops, else what one of
/// them loses as its type changes.
fn parts_loss(fa: &[(String, Ty)], fb: &[(String, Ty)], one: &str, many: &str) -> Option<String> {
    let gone: Vec<&str> = fa.iter().filter(|(n, _)| !fb.iter().any(|(m, _)| m.eq_ignore_ascii_case(n))).map(|(n, _)| n.as_str()).collect();
    if !gone.is_empty() {
        return Some(format!("drops its {} {}", if gone.len() == 1 { one } else { many }, gone.join(", ")));
    }
    fa.iter().find_map(|(n, x)| {
        let (_, y) = fb.iter().find(|(m, _)| m.eq_ignore_ascii_case(n))?;
        loss(x, y).map(|w| format!("{w}, in its {one} {n}"))
    })
}

/// What DuckDB's own casts lose without failing, where other databases refuse the change or keep
/// the values: a time's offset, a number's value as a boolean, text's escapes as a blob's bytes.
fn cast_loss(a: &str, b: &str) -> Option<String> {
    let (a, b) = (a.to_ascii_uppercase(), b.to_ascii_uppercase());
    let first = |t: &str| t.split(['(', ' ']).next().unwrap_or_default().to_string();
    let (x, y) = (first(&a), first(&b));
    let zoned = x == "TIMETZ" || (x == "TIME" && a.contains("WITH TIME ZONE"));
    let number = matches!(x.as_str(), "TINYINT" | "UTINYINT" | "SMALLINT" | "USMALLINT" | "INTEGER" | "UINTEGER" | "BIGINT" | "UBIGINT" | "HUGEINT" | "UHUGEINT" | "FLOAT" | "DOUBLE" | "DECIMAL" | "VARINT" | "BIGNUM");
    match () {
        _ if zoned && y == "TIME" && !b.contains("WITH TIME ZONE") => Some("keeps the time of day, not its offset".into()),
        _ if number && y == "BOOLEAN" => Some("keeps only whether each number is zero (5 becomes true)".into()),
        _ if x == "VARCHAR" && y == "BLOB" => Some("reads each backslash escape as a byte ('\\x41' becomes A)".into()),
        _ => None,
    }
}

/// A DuckDB type, as far as a change between two of them can lose values.
fn type_class(t: &str) -> super::TypeClass {
    use super::TypeClass;
    let upper = t.to_ascii_uppercase();
    let first = upper.split(['(', ' ']).next().unwrap_or_default();
    match first {
        "TINYINT" | "UTINYINT" => TypeClass::Int(8),
        "SMALLINT" | "USMALLINT" => TypeClass::Int(16),
        "INTEGER" | "UINTEGER" => TypeClass::Int(32),
        "BIGINT" | "UBIGINT" => TypeClass::Int(64),
        "HUGEINT" | "UHUGEINT" => TypeClass::Int(128),
        "FLOAT" => TypeClass::Float(6),
        "DOUBLE" => TypeClass::Float(15),
        "DECIMAL" => TypeClass::Numeric(upper.split_once(',').and_then(|(_, s)| s.trim_end_matches(')').trim().parse().ok()).or(Some(3))),
        "DATE" => TypeClass::Date,
        "TIMESTAMP" if upper.contains("WITH TIME ZONE") => TypeClass::Timestamp { zone: true, precision: 6 },
        "TIMESTAMP_S" => TypeClass::Timestamp { zone: false, precision: 0 },
        "TIMESTAMP_MS" => TypeClass::Timestamp { zone: false, precision: 3 },
        "TIMESTAMP_NS" => TypeClass::Timestamp { zone: false, precision: 9 },
        "TIMESTAMP" => TypeClass::Timestamp { zone: false, precision: 6 },
        "TIME_NS" => TypeClass::Time { precision: 9 },
        "TIME" => TypeClass::Time { precision: 6 },
        // An integer of any size.
        "BIGNUM" | "VARINT" => TypeClass::Numeric(Some(0)),
        "INTERVAL" => TypeClass::Interval(None),
        "VARCHAR" => TypeClass::Text,
        "JSON" => TypeClass::Json,
        _ => TypeClass::Other,
    }
}

impl DuckdbAlterer {
    /// The statements that take table `from` to `to`: `alter table` where DuckDB takes them, else a
    /// rebuild. A column renamed in place is renamed after these, by `rename`, so these write it by
    /// its old name.
    fn alter_table(&self, from: &Object, to: &Object) -> Option<Vec<String>> {
        // Made again as the target has it by the rebuild of a table it references, or around a
        // macro, type or sequence made again.
        if self.absorbed(&from.id) || self.around_absorbed(&from.id) {
            return Some(vec![]);
        }
        let mode = self.mode(from, to);
        let schemas = self.schemas();
        let (fs, ts) = schemas.as_ref()?;
        let (a, b) = (Table::of(fs, from), Table::of(ts, to));
        let renamed = self.renames_of(&to.id, ts);
        let table = a.name.clone();
        if let Mode::InPlace { around_indexes } = mode {
            let mut out = Vec::new();
            let indexes = if around_indexes { kept_on(fs, ts, &from.id, Kind::Index) } else { vec![] };
            out.extend(indexes.iter().map(|i| i.drop[0].clone()));
            let old_of = |new: &str| renamed.get(new).cloned().unwrap_or_else(|| new.to_string());
            let kept: BTreeSet<String> = b.names().iter().map(|n| old_of(n)).collect();
            for c in &a.columns {
                let name = field(c, "name").unwrap_or_default();
                if !kept.contains(name) {
                    out.push(format!("alter table {table} drop column {name}"));
                }
            }
            for n in &b.columns {
                let new_name = field(n, "name").unwrap_or_default();
                let old_name = old_of(new_name);
                let Some(c) = a.column(&old_name) else {
                    let written = field(n, "written").unwrap_or_default();
                    out.push(format!("alter table {table} add column {written}"));
                    continue;
                };
                // A column renamed in place by `rename` is changed here by its old name; one the
                // table's statements rename, where they drop its indexes around it, by its new one.
                let (x, y) = (column_change(&c.def), column_change(&n.def));
                let column = if around_indexes && old_name != new_name {
                    out.push(format!("alter table {table} rename column {old_name} to {new_name}"));
                    new_name.to_string()
                } else {
                    old_name.clone()
                };
                if x.kind != y.kind {
                    out.push(format!("alter table {table} alter column {column} set data type {}", written_type(n)));
                }
                if x.not_null && !y.not_null {
                    out.push(format!("alter table {table} alter column {column} drop not null"));
                }
                if x.default != y.default {
                    match &y.default {
                        Some(_) => out.push(format!("alter table {table} alter column {column} set default {}", written_default(n))),
                        None => out.push(format!("alter table {table} alter column {column} drop default")),
                    }
                }
                if !x.not_null && y.not_null {
                    out.push(format!("alter table {table} alter column {column} set not null"));
                }
            }
            for i in &indexes {
                out.push(i.create[0].clone());
                out.extend(unchanged_comment(fs, ts, &i.id));
            }
            return Some(out);
        }
        Some(self.rebuild(fs, ts, from))
    }

    /// Whether the table `id` is made again as the target has it by the rebuild of a table its keys
    /// reference, at any depth: where every table the target's keys reference is there before the
    /// plan, so that it can be made with them.
    fn absorbed(&self, id: &str) -> bool {
        let schemas = self.schemas();
        let Some((fs, ts)) = schemas.as_ref() else { return false };
        let Some(target) = ts.objects.get(id) else { return false };
        if !fields(target, "references").iter().all(|r| fs.objects.contains_key(*r)) {
            return false;
        }
        // Only by one: a table two rebuilds take is made again as it is by each, and changes after.
        fs.objects
            .values()
            .filter(|o| o.kind == Some(Kind::Table) && o.id != id && ts.objects.get(&o.id).is_some_and(|t| t.def != o.def))
            .filter(|o| children_of(fs, ts, &o.id).iter().any(|c| c.id == id) && self.mode(o, &ts.objects[&o.id]) == Mode::Rebuild)
            .count()
            == 1
    }
}

/// The rows a rebuilt table takes back: the columns both have, by their new names from their old
/// ones, but those the new table generates.
fn pairs_of(fs: &Schema, ts: &Schema, from: &Object, to: &Object, renamed: &BTreeMap<String, String>) -> Vec<(String, String)> {
    let (a, b) = (Table::of(fs, from), Table::of(ts, to));
    let old_of = |new: &str| renamed.get(new).cloned().unwrap_or_else(|| new.to_string());
    b.columns
        .iter()
        .filter(|n| !flag(n, 'g'))
        .filter_map(|n| {
            let new = field(n, "name")?.to_string();
            let old = old_of(&new);
            a.column(&old).filter(|c| !flag(c, 'g')).map(|_| (new, old))
        })
        .collect()
}

/// A column's type as its definition writes it, without what follows.
fn written_type(c: &Object) -> String {
    let written = field(c, "written").unwrap_or_default();
    let name = field(c, "name").unwrap_or_default();
    let rest = written.strip_prefix(name).unwrap_or(written).trim_start();
    let ends = [" collate ", " not null", " default ", " generated always as "];
    let end = ends.iter().filter_map(|e| rest.find(e)).min().unwrap_or(rest.len());
    rest[..end].to_string()
}

fn written_default(c: &Object) -> String {
    let written = field(c, "written").unwrap_or_default();
    written.split_once(" default ").map(|(_, d)| d.to_string()).unwrap_or_default()
}

/// One table of a rebuild: as it is, the statement that makes it again, its name then, and the
/// columns whose rows come back, each its name then and its name in the copy.
struct Member<'a> {
    from: &'a Object,
    create: String,
    name: String,
    pairs: Vec<(String, String)>,
}

/// The columns of a table that each copy of it fills: all of them but those it generates.
fn copied(schema: &Schema, o: &Object) -> Vec<(String, String)> {
    Table::of(schema, o).columns.iter().filter(|c| !flag(c, 'g')).filter_map(|c| field(c, "name").map(|n| (n.to_string(), n.to_string()))).collect()
}

/// The statements that make a family of tables again, the first and then those whose keys
/// reference it, at any depth, each after what it references: each one's rows copied to a
/// temporary table, the tables dropped, each one that references another first, then `between` (a
/// macro or a type made again), the tables made again, and the rows copied back, then `after` (their
/// indexes and comments).
fn rebuild_family(members: &[Member<'_>], between: Vec<String>, after: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    // DuckDB checks a key a table has on itself against the rows the table held when the
    // transaction began, so no copy, in any order, gives back a row whose key names another row.
    for m in members {
        let bare = super::last_written(&m.name);
        let tokens = tokenize(&m.create, Dialect::Duckdb);
        if tokens.windows(2).any(|w| w[0].upper() == "REFERENCES" && unquote(&w[1]).eq_ignore_ascii_case(bare.trim_matches('"'))) {
            out.push(format!("-- {} references itself, and DuckDB checks such a key against the rows a table held when its transaction began: where a row's key names another row, this copy fails, and the migration with it, changing nothing", m.name));
        }
    }
    let mut copies: Vec<String> = Vec::new();
    for m in members {
        let current = field(m.from, "name").unwrap_or_default();
        let bare = super::last_written(current).trim_matches('"').to_string();
        let copy = (1..).map(|n| if n == 1 { format!("reylo_copy_{bare}") } else { format!("reylo_copy_{bare}_{n}") }).find(|c| !copies.contains(c)).unwrap_or_default();
        out.push(format!("create temporary table {} as select * from {current}", quote(&copy)));
        copies.push(copy);
    }
    for m in members.iter().rev() {
        out.push(format!("drop table {}", field(m.from, "name").unwrap_or_default()));
    }
    out.extend(between);
    out.extend(members.iter().map(|m| m.create.clone()));
    let temp = |copy: &str| format!("temp.main.{}", quote(copy));
    for (m, copy) in members.iter().zip(&copies) {
        if !m.pairs.is_empty() {
            out.push(format!(
                "insert into {} ({}) select {} from {}",
                m.name,
                m.pairs.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "),
                m.pairs.iter().map(|(_, o)| o.as_str()).collect::<Vec<_>>().join(", "),
                temp(copy)
            ));
        }
    }
    for copy in &copies {
        out.push(format!("drop table {}", temp(copy)));
    }
    out.extend(after);
    out
}

impl DuckdbAlterer {
    /// The statements that rebuild the table `from` as the target has it, with the tables that
    /// reference it: each made again as the target has it where it is absorbed, and else as it is,
    /// but for its keys on what those change; and what the plan keeps of them made again after.
    fn rebuild(&self, fs: &Schema, ts: &Schema, from: &Object) -> Vec<String> {
        let family: Vec<&Object> = std::iter::once(from).chain(children_of(fs, ts, &from.id)).collect();
        let members = self.members_of(fs, ts, &family, &|x: &Object| x.id == from.id || self.absorbed(&x.id));
        let mut after = Vec::new();
        for m in &members {
            after.extend(kept_on(fs, ts, &m.from.id, Kind::Index).iter().map(|i| i.create[0].clone()));
            after.extend(kept_on(fs, ts, &m.from.id, Kind::Comment).iter().map(|c| c.create[0].clone()));
        }
        rebuild_family(&members, vec![], after)
    }
}

/// A foreign key as a table's definition writes it, read apart: its columns, the table it
/// references, and the columns there.
fn foreign_key(sql: &str) -> Option<(Vec<String>, String, Vec<String>)> {
    let tokens = tokenize(sql, Dialect::Duckdb);
    if tokens.first()?.upper() != "FOREIGN" {
        return None;
    }
    let list = |at: usize| -> (Vec<String>, usize) {
        let mut out = Vec::new();
        let mut k = at + 1;
        while k < tokens.len() && tokens[k].text != ")" {
            if tokens[k].text != "," {
                out.push(unquote(&tokens[k]));
            }
            k += 1;
        }
        (out, k + 1)
    };
    let open = tokens.iter().position(|t| t.text == "(")?;
    let (columns, end) = list(open);
    let mut at = tokens.iter().skip(end).position(|t| t.upper() == "REFERENCES")? + end + 1;
    // The table by its last name: DuckDB writes a key to a table of another schema with its schema.
    while tokens.get(at + 1).is_some_and(|t| t.text == ".") {
        at += 2;
    }
    let table = unquote(tokens.get(at)?);
    let (referenced, _) = if tokens.get(at + 1).is_some_and(|t| t.text == "(") { list(at + 1) } else { (vec![], at) };
    Some((columns, table, referenced))
}

/// The table `o`'s `create table` statement, with the columns `retype` names of the types it gives.
fn create_retyped(schema: &Schema, o: &Object, retype: &BTreeMap<String, String>) -> String {
    let create = field(o, "create").unwrap_or_default().to_string();
    if retype.is_empty() {
        return create;
    }
    let mut parts: Vec<String> = Table::of(schema, o)
        .columns
        .iter()
        .map(|c| {
            let written = field(c, "written").unwrap_or_default();
            let name = field(c, "name").unwrap_or_default();
            match retype.get(name) {
                Some(t) => {
                    let rest = &written[(name.len() + 1 + written_type(c).len()).min(written.len())..];
                    format!("{name} {t}{rest}")
                }
                None => written.to_string(),
            }
        })
        .collect();
    parts.extend(fields(o, "constraint").into_iter().map(String::from));
    format!("create table {} ({})", field(o, "name").unwrap_or_default(), parts.join(", "))
}

/// The comment on `id` where it is the same on both sides, as the statement that makes it: for what a
/// plan drops and makes again itself, which the planner leaves be.
fn unchanged_comment(fs: &Schema, ts: &Schema, id: &str) -> Option<String> {
    let comment = format!("comment {id}");
    let (a, b) = (fs.objects.get(&comment)?, ts.objects.get(&comment)?);
    (a.def == b.def).then(|| b.create[0].clone())
}

/// The statements that rename the tables `renamed` names, each its old id in `current` and its new
/// id: `alter table … rename to`, which DuckDB refuses while the table has an index, so its indexes
/// are dropped first and made again after. A rename leaves a foreign key that references the table
/// naming what is no longer there, so a table other tables reference, or that references itself, is
/// made again under its new name instead, with those tables, at any depth, referencing it there.
pub fn rename_statements(current: &Schema, renamed: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    for (old_id, new_id) in renamed {
        let Some(t) = current.objects.get(old_id) else { continue };
        let from = field(t, "name").unwrap_or_default().to_string();
        let new_name = super::last_written(new_id.strip_prefix("table ").unwrap_or(new_id));
        let old_bare = super::last_written(&from);
        // In its own schema: `billing.invoices` renamed is `billing.<new name>`.
        let tokens = tokenize(&from, Dialect::Duckdb);
        let renamed_name = if tokens.len() == 3 && tokens[1].text == "." { format!("{}.{new_name}", tokens[0].text) } else { new_name.clone() };
        let on = |id: &str, kind: Kind| -> Vec<&Object> { current.objects.values().filter(|o| o.kind == Some(kind) && o.owner.as_deref() == Some(id)).collect() };
        let comments_of = |id: &str| -> Vec<&Object> {
            current.objects.values().filter(|o| o.kind == Some(Kind::Comment) && o.owner.as_deref().is_some_and(|owner| owner == id || current.objects.get(owner).is_some_and(|c| matches!(c.kind, Some(Kind::Column | Kind::Index)) && c.owner.as_deref() == Some(id)))).collect()
        };
        let children = children_of(current, current, old_id);
        // A table with a foreign key of its own: renamed, DuckDB keeps the old name in the table it
        // references, whose drop it then refuses for good.
        if children.is_empty() && fields(t, "references").is_empty() {
            let indexes = on(old_id, Kind::Index);
            out.extend(indexes.iter().map(|i| i.drop[0].clone()));
            out.push(format!("alter table {from} rename to {new_name}"));
            for i in &indexes {
                out.push(swap_table(&i.create[0], &old_bare, &new_name));
                out.extend(comments_of(old_id).iter().filter(|c| c.owner.as_deref() == Some(i.id.as_str())).map(|c| c.create[0].clone()));
            }
            continue;
        }
        let create = swap_references(&swap_created(field(t, "create").unwrap_or_default(), &new_name), &old_bare, &new_name);
        let mut members = vec![Member { from: t, create, name: renamed_name.clone(), pairs: copied(current, t) }];
        for x in &children {
            members.push(Member { from: x, create: swap_references(field(x, "create").unwrap_or_default(), &old_bare, &new_name), name: field(x, "name").unwrap_or_default().to_string(), pairs: copied(current, x) });
        }
        let mut after = Vec::new();
        for m in &members {
            let renaming = m.from.id == *old_id;
            for i in on(&m.from.id, Kind::Index) {
                after.push(if renaming { swap_table(&i.create[0], &old_bare, &new_name) } else { i.create[0].clone() });
            }
            for c in comments_of(&m.from.id) {
                after.push(if renaming { c.create[0].replacen(&format!(" on table {from} is "), &format!(" on table {renamed_name} is "), 1).replacen(&format!(" on column {from}."), &format!(" on column {renamed_name}."), 1) } else { c.create[0].clone() });
            }
        }
        out.extend(rebuild_family(&members, vec![], after));
    }
    out
}

/// A `create table` statement of the plan's, making the table under the name `new`.
fn swap_created(sql: &str, new: &str) -> String {
    let tokens = tokenize(sql, Dialect::Duckdb);
    let Some(at) = tokens.iter().position(|t| t.upper() == "TABLE").map(|k| k + 1) else { return sql.to_string() };
    let end = if tokens.get(at + 1).is_some_and(|t| t.text == ".") { at + 2 } else { at };
    match (tokens.get(at), tokens.get(end)) {
        (Some(first), Some(last)) => {
            let schema = if end > at { &sql[first.start..tokens[at + 1].end] } else { "" };
            format!("{}{schema}{new}{}", &sql[..first.start], &sql[last.end..])
        }
        _ => sql.to_string(),
    }
}

/// A table's statement with the columns its keys reference in the table `table` written by their
/// names after a rename: `references a (id)` as `references a (aid)`.
fn swap_referenced_columns(sql: &str, table: &str, new_of_old: &BTreeMap<String, String>) -> String {
    if new_of_old.is_empty() {
        return sql.to_string();
    }
    let tokens = tokenize(sql, Dialect::Duckdb);
    let mut out = String::new();
    let mut at = 0;
    let mut k = 0;
    while k < tokens.len() {
        if tokens[k].upper() == "REFERENCES" && tokens.get(k + 1).is_some_and(|t| unquote(t).eq_ignore_ascii_case(table.trim_matches('"'))) && tokens.get(k + 2).is_some_and(|t| t.text == "(") {
            let mut j = k + 3;
            while j < tokens.len() && tokens[j].text != ")" {
                if let Some(new) = new_of_old.get(&quote(&unquote(&tokens[j]))) {
                    out.push_str(&sql[at..tokens[j].start]);
                    out.push_str(new);
                    at = tokens[j].end;
                }
                j += 1;
            }
            k = j;
        }
        k += 1;
    }
    out.push_str(&sql[at..]);
    out
}

/// The statements that empty every table of `schema` and start its sequences again, as `db wipe`
/// runs them in one transaction: each table dropped, each after those that reference it, and made
/// again with its indexes and comments, since DuckDB's keys don't see rows deleted earlier in the
/// same transaction, and a table that references itself has no order of deletes at all.
pub fn emptied(schema: &Schema) -> (usize, Vec<String>) {
    let mut left: Vec<&Object> = schema.objects.values().filter(|o| o.kind == Some(Kind::Table)).collect();
    let mut ordered: Vec<&Object> = Vec::new();
    while !left.is_empty() {
        let at = left.iter().position(|o| !o.deps.iter().any(|d| left.iter().any(|x| x.id == *d && x.id != o.id))).unwrap_or(0);
        ordered.push(left.remove(at));
    }
    let mut out: Vec<String> = ordered.iter().rev().map(|o| format!("drop table {}", field(o, "name").unwrap_or_default())).collect();
    for s in schema.objects.values().filter(|o| o.kind == Some(Kind::Sequence)) {
        out.extend(s.drop.iter().cloned());
        out.extend(field(s, "restart").map(String::from));
        out.extend(schema.objects.get(&format!("comment {}", s.id)).map(|c| c.create[0].clone()));
    }
    out.extend(ordered.iter().map(|o| field(o, "create").unwrap_or_default().to_string()));
    for o in &ordered {
        out.extend(schema.objects.values().filter(|i| i.kind == Some(Kind::Index) && i.owner.as_deref() == Some(o.id.as_str())).map(|i| i.create[0].clone()));
        let owned = |c: &&Object| c.owner.as_deref().is_some_and(|owner| owner == o.id || schema.objects.get(owner).is_some_and(|x| matches!(x.kind, Some(Kind::Column | Kind::Index)) && x.owner.as_deref() == Some(o.id.as_str())));
        out.extend(schema.objects.values().filter(|c| c.kind == Some(Kind::Comment)).filter(owned).map(|c| c.create[0].clone()));
    }
    (ordered.len(), out)
}

/// An index's statement on the table renamed from `old` to `new`.
fn swap_table(sql: &str, old: &str, new: &str) -> String {
    let tokens = tokenize(sql, Dialect::Duckdb);
    let Some(on) = tokens.iter().position(|t| t.upper() == "ON") else { return sql.to_string() };
    let at = if tokens.get(on + 2).is_some_and(|t| t.text == ".") { on + 3 } else { on + 1 };
    match tokens.get(at) {
        Some(t) if unquote(t).eq_ignore_ascii_case(old.trim_matches('"')) => format!("{}{new}{}", &sql[..t.start], &sql[t.end..]),
        _ => sql.to_string(),
    }
}

/// A table's statement with its foreign keys to the table renamed from `old` referencing `new`.
fn swap_references(sql: &str, old: &str, new: &str) -> String {
    let tokens = tokenize(sql, Dialect::Duckdb);
    let mut out = String::new();
    let mut at = 0;
    for (k, t) in tokens.iter().enumerate() {
        if k > 0 && tokens[k - 1].upper() == "REFERENCES" && unquote(t).eq_ignore_ascii_case(old.trim_matches('"')) {
            out.push_str(&sql[at..t.start]);
            out.push_str(new);
            at = t.end;
        }
    }
    out.push_str(&sql[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_what_duckdb_would_read_otherwise() {
        assert_eq!(quote("users"), "users");
        assert_eq!(quote("Users"), "Users");
        assert_eq!(quote("order"), "\"order\"");
        assert_eq!(quote("my table"), "\"my table\"");
        assert_eq!(qualified("main", "t"), "t");
        assert_eq!(qualified("billing", "select"), "billing.\"select\"");
    }

    #[test]
    fn writes_duckdbs_renderings_in_lowercase_but_its_names() {
        assert_eq!(styled("CREATE TABLE T(ID INTEGER DEFAULT('X') NOT NULL)"), "create table T(ID integer default('X') not null)");
        assert_eq!(styled_named("CREATE INDEX IX ON T(ID)"), "create index IX on T(ID)");
        assert_eq!(styled_named("CREATE VIEW V AS SELECT 1 AS \"SELECT\""), "create view V as select 1 as \"SELECT\"");
        // A name DuckDB writes bare that is no keyword keeps its case, as a type's name; a cast's
        // type and a column's type are written in lowercase, but for a struct's fields.
        assert_eq!(styled_named("CREATE INDEX i ON t(DATE, UUID)"), "create index i on t(DATE, UUID)");
        assert_eq!(styled("CAST(DATE AS DOUBLE) + S"), "cast(DATE as double) + S");
        assert_eq!(styled_type("STRUCT(DATE DATE, n STRUCT(US UUID))[]"), "struct(DATE date, n struct(US uuid))[]");
        assert_eq!(styled_type("ENUM('A', 'b')"), "enum('A', 'b')");
    }

    #[test]
    fn nanoseconds_are_finer_than_a_timestamp() {
        assert_eq!(super::super::narrowing(&type_class("TIMESTAMP_NS"), &type_class("TIMESTAMP")).as_deref(), Some("rounds to 6 decimal places of a second"));
        assert_eq!(super::super::narrowing(&type_class("TIMESTAMP"), &type_class("TIMESTAMP_NS")), None);
    }

    #[test]
    fn names_what_duckdbs_casts_lose_in_any_part_of_a_type() {
        let lost = |a: &str, b: &str| loss(&parse_type(a), &parse_type(b));
        assert_eq!(lost("TIME WITH TIME ZONE", "TIME").as_deref(), Some("keeps the time of day, not its offset"));
        assert_eq!(lost("INTEGER", "BOOLEAN").as_deref(), Some("keeps only whether each number is zero (5 becomes true)"));
        assert_eq!(lost("VARCHAR", "BLOB").as_deref(), Some("reads each backslash escape as a byte ('\\x41' becomes A)"));
        assert_eq!(lost("DOUBLE[]", "INTEGER[]").as_deref(), Some("rounds to whole numbers, in each element"));
        assert_eq!(lost("MAP(VARCHAR, DOUBLE)", "MAP(VARCHAR, INTEGER)").as_deref(), Some("rounds to whole numbers, in each value"));
        assert_eq!(lost("STRUCT(x DOUBLE)", "STRUCT(x INTEGER)").as_deref(), Some("rounds to whole numbers, in its field x"));
        assert_eq!(lost("STRUCT(host VARCHAR, port INTEGER)", "STRUCT(host VARCHAR)").as_deref(), Some("drops its field port"));
        assert_eq!(lost("UNION(n DOUBLE, s VARCHAR)", "UNION(n INTEGER, s VARCHAR)").as_deref(), Some("rounds to whole numbers, in its member n"));
        assert_eq!(lost("UNION(n DOUBLE, s VARCHAR)", "UNION(n DOUBLE)").as_deref(), Some("drops its member s"));
        assert_eq!(lost("BIGNUM", "DOUBLE").as_deref(), Some("keeps about 15 significant digits"));
        assert_eq!(lost("TIME_NS", "TIME").as_deref(), Some("rounds to 6 decimal places of a second"));
        for (a, b) in [("BOOLEAN", "INTEGER"), ("TIME", "TIME WITH TIME ZONE"), ("BLOB", "VARCHAR"), ("INTEGER[]", "BIGINT[]"), ("STRUCT(x INTEGER)", "STRUCT(x INTEGER, y INTEGER)")] {
            assert_eq!(lost(a, b), None, "{a} to {b}");
        }
    }

    /// A table's constraints as its text writes them are those `duckdb_constraints()` gives, on
    /// every schema of the corpus and the oracle's cases: so that a database read from its tables'
    /// text compares with one read the other way.
    #[tokio::test(flavor = "current_thread")]
    async fn reads_constraints_from_a_tables_text_as_duckdb_gives_them() {
        if std::env::var_os("REYLO_DUCKDB_LIBRARY").is_none() {
            return;
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        for e in std::fs::read_dir(root.join("tests/diff/duckdb")).unwrap() {
            let dir = e.unwrap().path();
            files.extend(["v1.sql", "v2.sql"].iter().map(|f| dir.join(f)));
        }
        for e in std::fs::read_dir(root.join("../../oracle/cases")).unwrap() {
            let dir = e.unwrap().path();
            if std::fs::read_to_string(dir.join("dialect")).unwrap_or_default().trim() == "duckdb" {
                files.push(dir.join("setup.sql"));
            }
        }
        let mut differ = Vec::new();
        let mut read_any = 0;
        for file in files {
            let sql = std::fs::read_to_string(&file).unwrap_or_default();
            let mut db = Db::connect("duckdb::memory:", &std::env::temp_dir()).await.unwrap();
            if sql.trim().is_empty() || db.exec(&sql).await.is_err() {
                continue;
            }
            let read = read_catalog(&mut db).await.unwrap();
            read_any += read.constraints.len();
            let mut text = read.clone();
            text.constraints = read.tables.iter().flat_map(|t| constraints_of(&t.schema, &t.name, &t.sql)).collect();
            normalize_constraints(&mut text);
            let lines = |r: &Read| -> BTreeSet<String> { r.constraints.iter().map(constraint_line).collect() };
            if lines(&read) != lines(&text) {
                differ.push(format!("{}:\n  catalog {:?}\n  text    {:?}", file.display(), lines(&read), lines(&text)));
            }
        }
        assert!(read_any > 100, "read only {read_any} constraints");
        assert!(differ.is_empty(), "{}", differ.join("\n"));
    }

    #[test]
    fn reads_a_foreign_key_apart() {
        assert_eq!(foreign_key("foreign key (a_id, \"B\") references \"Parent\" (id, b)"), Some((vec!["a_id".into(), "B".into()], "Parent".into(), vec!["id".into(), "b".into()])));
        assert_eq!(foreign_key("check (n > 0)"), None);
    }

    #[test]
    fn reads_a_columns_definition_apart() {
        let d = column_change("VARCHAR COLLATE nocase NOT NULL DEFAULT 'x'");
        assert_eq!((d.kind.as_str(), d.collation.as_deref(), d.not_null, d.default.as_deref()), ("VARCHAR", Some("nocase"), true, Some("'x'")));
        let g = column_change("INTEGER GENERATED ALWAYS AS ((id + 1)) VIRTUAL");
        assert_eq!(g.generated.as_deref(), Some("(id + 1)"));
        assert_eq!(collations("CREATE TABLE t(a VARCHAR COLLATE nocase, b ENUM('x', 'y'), CHECK((a <> '')))").get("a").map(String::as_str), Some("nocase"));
    }

    #[test]
    fn renames_a_table_in_what_names_it() {
        assert_eq!(swap_table("create index i on users(email)", "users", "accounts"), "create index i on accounts(email)");
        assert_eq!(swap_references("create table posts (user_id integer, foreign key (user_id) references users (id))", "users", "accounts"), "create table posts (user_id integer, foreign key (user_id) references accounts (id))");
    }
}
