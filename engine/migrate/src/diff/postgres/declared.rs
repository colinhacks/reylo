//! The text `schema.sql` declared each object with. The server's rendering of an expression is not
//! always one its parser reads back to the same expression (a check that keeps a varchar column to a
//! list renders a cast to text and an array of varchar, which a new constraint stores as another
//! tree), so a plan that creates objects from the rendering can never reach the schema it read.
//! Postgres's own parser reads `schema.sql`, each object it declares is matched to the object the
//! reader read from the database `schema.sql` built, and the plan creates the object from its
//! declaration: the statement itself where one statement declares the object whole (a view, an
//! index, a function, a trigger, a policy, a domain, a rule), and the expression alone for a
//! column's default or generation and a table's check constraint, which the plan writes into
//! statements of its own. What is compared stays the server's rendering. An object the file
//! declares in a way this does not follow (renamed, or altered after it is declared, or under a
//! search_path of its own) keeps the server's rendering, which the shadow check still covers.

use super::{bare, parse_column, render_column, AROUND};
use crate::diff::{Kind, Schema};
use pg_query::protobuf::{self, node::Node as N, AlterTableType as At, ConstrType, ObjectType};
use pg_query::Node;
use std::collections::{BTreeMap, BTreeSet};

/// An identifier as Postgres's `quote_ident` writes it: bare when it is lowercase and no keyword
/// but an unreserved one.
pub(super) fn quote_ident(name: &str) -> String {
    let bare = name.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '$')
        && !pg_query::scan(name).ok().and_then(|r| r.tokens.first().map(|t| t.keyword_kind)).is_some_and(|k| k > protobuf::KeywordKind::UnreservedKeyword as i32);
    if bare {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

fn qualified(schema: &str, name: &str) -> String {
    format!("{}.{}", quote_ident(schema), quote_ident(name))
}

/// The strings of a list of name nodes (`schema.name`).
fn names(list: &[Node]) -> Vec<String> {
    list.iter().filter_map(|n| match &n.node {
        Some(N::String(s)) => Some(s.sval.clone()),
        _ => None,
    }).collect()
}

/// An expression as the parser's deparser writes it, which the parser reads back to the same tree.
fn expression(node: &Node) -> Option<String> {
    let target = Node { node: Some(N::ResTarget(Box::new(protobuf::ResTarget { val: Some(Box::new(node.clone())), ..Default::default() }))) };
    let select = Node { node: Some(N::SelectStmt(Box::new(protobuf::SelectStmt { target_list: vec![target], op: protobuf::SetOperation::SetopNone as i32, limit_option: protobuf::LimitOption::Default as i32, ..Default::default() }))) };
    select.deparse(pg_query::DeparseOptions::default()).ok()?.strip_prefix("SELECT ").map(String::from)
}

/// Every node of a kind anywhere in a tree, as JSON, which every node of the parser's serializes to.
fn find(node: &impl serde::Serialize, kind: &str) -> Vec<serde_json::Value> {
    fn walk(v: &serde_json::Value, kind: &str, out: &mut Vec<serde_json::Value>) {
        match v {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    if k == kind {
                        out.push(v.clone());
                    }
                    walk(v, kind, out);
                }
            }
            serde_json::Value::Array(items) => items.iter().for_each(|v| walk(v, kind, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Ok(v) = serde_json::to_value(node) {
        walk(&v, kind, &mut out);
    }
    out
}

/// The columns an expression names, each once: the last field of each column reference.
fn columns_named(node: &Node) -> BTreeSet<String> {
    find(node, "ColumnRef").iter().filter_map(|r| r["fields"].as_array()?.last()?.pointer("/node/String/sval")?.as_str().map(String::from)).collect()
}

/// A name Postgres makes from parts (`makeObjectName`): `name1_name2_label`, the longer part
/// shortened first until it fits in 63 bytes.
fn object_name(name1: &str, name2: Option<&str>, label: &str) -> String {
    let overhead = label.len() + 1 + usize::from(name2.is_some());
    let avail = 63usize.saturating_sub(overhead);
    let (mut a, mut b) = (name1.len(), name2.map_or(0, str::len));
    while a + b > avail {
        if a > b {
            a -= 1
        } else {
            b -= 1
        }
    }
    let clip = |s: &str, n: usize| -> String {
        let mut end = n.min(s.len());
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s[..end].to_string()
    };
    match name2 {
        Some(n2) => format!("{}_{}_{label}", clip(name1, a), clip(n2, b)),
        None => format!("{}_{label}", clip(name1, a)),
    }
}

/// The names Postgres tries in turn for an unnamed object (`ChooseConstraintName`,
/// `ChooseRelationName`): the label, then the label numbered from 1.
fn candidates(name1: &str, name2: Option<&str>, label: &str) -> impl Iterator<Item = String> {
    let (name1, name2, label) = (name1.to_string(), name2.map(String::from), label.to_string());
    (0..100).map(move |pass| object_name(&name1, name2.as_deref(), &if pass == 0 { label.clone() } else { format!("{label}{pass}") }))
}

/// A type as `format_type` writes a function's argument (no modifier), for telling overloads apart.
fn type_text(t: &protobuf::TypeName, resolve: &dyn Fn(&[String], &str) -> Option<String>) -> String {
    let parts = names(&t.names);
    let base = match parts.as_slice() {
        [schema, name] if schema == "pg_catalog" => builtin(name).to_string(),
        [name] if builtin(name) != name.as_str() => builtin(name).to_string(),
        _ => resolve(&parts, "type").unwrap_or_else(|| parts.iter().map(|p| quote_ident(p)).collect::<Vec<_>>().join(".")),
    };
    format!("{base}{}", "[]".repeat(t.array_bounds.len()))
}

fn builtin(name: &str) -> &str {
    match name {
        "int4" => "integer",
        "int8" => "bigint",
        "int2" => "smallint",
        "float4" => "real",
        "float8" => "double precision",
        "bool" => "boolean",
        "varchar" => "character varying",
        "bpchar" => "character",
        "timestamp" => "timestamp without time zone",
        "timestamptz" => "timestamp with time zone",
        "time" => "time without time zone",
        "timetz" => "time with time zone",
        "varbit" => "bit varying",
        _ => name,
    }
}

/// The arguments of a function id (`function public.f(integer,text)`), as `regprocedure` lists them.
fn id_arguments(id: &str) -> Vec<String> {
    let Some(inside) = id.split_once('(').map(|(_, r)| r.trim_end_matches(')')) else { return vec![] };
    let (mut out, mut depth, mut quoted, mut at) = (Vec::new(), 0, false, 0);
    for (k, c) in inside.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                out.push(inside[at..k].to_string());
                at = k + 1;
            }
            _ => {}
        }
    }
    if !inside.is_empty() {
        out.push(inside[at..].to_string());
    }
    out
}

/// The statement's text in the file, its leading and trailing comments left out.
fn source(sql: &str, raw: &protobuf::RawStmt) -> Option<String> {
    let start = raw.stmt_location.max(0) as usize;
    let end = if raw.stmt_len == 0 { sql.len() } else { start + raw.stmt_len as usize };
    let text = sql.get(start..end)?;
    let tokens = pg_query::scan(text).ok()?.tokens;
    let code: Vec<&protobuf::ScanToken> = tokens.iter().filter(|t| t.token != protobuf::Token::SqlComment as i32 && t.token != protobuf::Token::CComment as i32).collect();
    let (first, last) = (code.first()?, code.last()?);
    text.get(first.start as usize..last.end as usize).map(String::from)
}

/// What `schema.sql` declares, keyed by the reader's object ids.
#[derive(Default, Debug)]
pub(super) struct Declared {
    /// Whole statements: the object's create.
    pub statements: BTreeMap<String, String>,
    /// Expressions: `default <column id>`, `generated <column id>`, `check <constraint id>`,
    /// `default <domain id>` and `check <domain id> <name>`.
    pub expressions: BTreeMap<String, String>,
    /// Tables whose expressions are not followed: a column's type changed after its declaration,
    /// which casts what the server stores, or columns copied with `like`.
    pub untrusted: BTreeSet<String>,
}

struct Reader<'a> {
    schema: &'a Schema,
    /// The search_path in effect, the schemas that exist in it.
    path: Vec<String>,
    base: Vec<String>,
    out: Declared,
    /// Names the reader has given to unnamed objects.
    given: BTreeSet<String>,
}

impl Reader<'_> {
    /// Whether text declared now resolves its names as the plan's statements do: under the
    /// search_path the file started with, or under an empty one, where every name is qualified.
    fn trusted(&self) -> bool {
        self.path == self.base || self.path.is_empty()
    }

    fn exists(&self, id: &str) -> bool {
        self.schema.objects.contains_key(id)
    }

    /// The schema an unqualified object is created in: the first of the path.
    fn creation(&self, schema: &str) -> Option<String> {
        if schema.is_empty() {
            self.path.first().cloned()
        } else {
            Some(schema.to_string())
        }
    }

    /// The qualified name an existing object of one of `kinds` is found by.
    fn resolve(&self, parts: &[String], kinds: &[&str]) -> Option<String> {
        match parts {
            [schema, name] => Some(qualified(schema, name)),
            [name] => self.path.iter().map(|s| qualified(s, name)).find(|q| kinds.iter().any(|k| self.exists(&format!("{k} {q}")))).or_else(|| self.path.first().map(|s| qualified(s, name))),
            [_, schema, name] => Some(qualified(schema, name)),
            _ => None,
        }
    }

    /// The qualified name of an existing object of `kind` the path finds, if any.
    fn existing(&self, parts: &[String], kind: &str) -> Option<String> {
        match parts {
            [name] => self.path.iter().map(|s| qualified(s, name)).find(|q| self.exists(&format!("{kind} {q}"))),
            _ => self.resolve(parts, &[kind]),
        }
    }

    fn relation(&self, r: &protobuf::RangeVar) -> Option<(String, String)> {
        let parts: Vec<String> = [r.schemaname.clone(), r.relname.clone()].into_iter().filter(|s| !s.is_empty()).collect();
        let q = self.resolve(&parts, &["table", "view", "index", "sequence"])?;
        Some((q, r.relname.clone()))
    }

    fn statement(&mut self, id: String, text: Option<String>) {
        match text {
            Some(text) if self.trusted() && self.exists(&id) => {
                self.out.statements.insert(id, text);
            }
            _ => {
                self.out.statements.remove(&id);
            }
        }
    }

    fn expression(&mut self, key: String, node: Option<&Node>) {
        match node.and_then(expression) {
            Some(text) if self.trusted() => {
                self.out.expressions.insert(key, text);
            }
            _ => {
                self.out.expressions.remove(&key);
            }
        }
    }

    /// The first name Postgres would give an unnamed object that the schema has.
    fn choose(&mut self, names: impl Iterator<Item = String>, id: impl Fn(&str) -> String, has: impl Fn(&str) -> bool) -> Option<String> {
        for name in names {
            let full = id(&name);
            if self.given.contains(&full) {
                continue;
            }
            if has(&full) {
                self.given.insert(full);
                return Some(name);
            }
            return None;
        }
        None
    }

    /// A check constraint of a table, named or named as Postgres names it.
    fn check(&mut self, table: &str, relname: &str, c: &protobuf::Constraint) {
        let Some(expr) = c.raw_expr.as_deref() else { return };
        let name = if c.conname.is_empty() {
            let named = columns_named(expr);
            let column = if named.len() == 1 { named.iter().next().cloned() } else { None };
            let schema = self.schema;
            let id = |n: &str| format!("constraint {table}.{}", quote_ident(n));
            match self.choose(candidates(relname, column.as_deref(), "check"), id, |full| schema.objects.get(full).is_some_and(|o| o.def.starts_with("CHECK ("))) {
                Some(name) => name,
                None => return,
            }
        } else {
            c.conname.clone()
        };
        let id = format!("constraint {table}.{}", quote_ident(&name));
        self.given.insert(id.clone());
        self.expression(format!("check {id}"), Some(expr));
    }

    fn column(&mut self, table: &str, relname: &str, def: &protobuf::ColumnDef) {
        let column = format!("column {table}.{}", quote_ident(&def.colname));
        for c in &def.constraints {
            let Some(N::Constraint(c)) = &c.node else { continue };
            match ConstrType::try_from(c.contype) {
                Ok(ConstrType::ConstrDefault) => self.expression(format!("default {column}"), c.raw_expr.as_deref()),
                Ok(ConstrType::ConstrGenerated) => self.expression(format!("generated {column}"), c.raw_expr.as_deref()),
                Ok(ConstrType::ConstrCheck) => self.check(table, relname, c),
                _ => {}
            }
        }
    }

    /// Forgets what is declared of a table: its columns' and constraints' expressions.
    fn forget_table(&mut self, table: &str) {
        let column = format!(" column {table}.");
        let constraint = format!(" constraint {table}.");
        self.out.expressions.retain(|k, _| !k.contains(&column) && !k.contains(&constraint));
        for kind in ["view", "index", "table"] {
            self.out.statements.remove(&format!("{kind} {table}"));
        }
        let owned = |k: &String| k.split_once(' ').is_some_and(|(_, rest)| rest.strip_prefix(table).is_some_and(|r| r.starts_with('.')));
        self.out.statements.retain(|k, _| !(k.starts_with("trigger ") || k.starts_with("policy ") || k.starts_with("rule ")) || !owned(k));
    }

    fn function(&mut self, parts: &[String], parameters: &[Node], text: Option<String>) {
        let Some(schema) = (if parts.len() >= 2 { Some(parts[parts.len() - 2].clone()) } else { self.creation("") }) else { return };
        let name = parts.last().cloned().unwrap_or_default();
        let prefix = format!("function {}(", qualified(&schema, &name));
        let inputs: Vec<&protobuf::FunctionParameter> = parameters
            .iter()
            .filter_map(|p| match &p.node {
                Some(N::FunctionParameter(p)) => Some(p.as_ref()),
                _ => None,
            })
            .filter(|p| !matches!(protobuf::FunctionParameterMode::try_from(p.mode), Ok(protobuf::FunctionParameterMode::FuncParamOut | protobuf::FunctionParameterMode::FuncParamTable)))
            .collect();
        let same: Vec<&String> = self.schema.objects.keys().filter(|id| id.starts_with(&prefix) && id_arguments(id).len() == inputs.len()).collect();
        let id = if same.len() == 1 {
            Some(same[0].clone())
        } else {
            let resolve = |parts: &[String], kind: &str| self.existing(parts, kind);
            let wanted: Vec<String> = inputs.iter().map(|p| p.arg_type.as_ref().map(|t| type_text(t, &resolve)).unwrap_or_default()).collect();
            let matching: Vec<&&String> = same.iter().filter(|id| id_arguments(id) == wanted).collect();
            (matching.len() == 1).then(|| matching[0].to_string())
        };
        if let Some(id) = id {
            self.statement(id, text);
        } else {
            // An overload this cannot tell apart: none of the name keeps a declaration.
            self.out.statements.retain(|k, _| !k.starts_with(&prefix));
        }
    }

    /// The name Postgres gives an unnamed index (`ChooseIndexName`): the table's name, its
    /// columns' (`expr` for an expression), and `idx`.
    fn index_name(&mut self, table: &str, schema: &str, relname: &str, params: &[Node]) -> Option<String> {
        let mut columns: Vec<String> = Vec::new();
        for e in params {
            let Some(N::IndexElem(e)) = &e.node else { continue };
            let base = if !e.indexcolname.is_empty() {
                e.indexcolname.clone()
            } else if !e.name.is_empty() {
                e.name.clone()
            } else {
                "expr".to_string()
            };
            let mut name = base.clone();
            let mut n = 1;
            while columns.contains(&name) {
                name = format!("{base}{n}");
                n += 1;
            }
            columns.push(name);
        }
        let mut addition = String::new();
        for c in &columns {
            if !addition.is_empty() {
                addition.push('_');
            }
            addition.push_str(c);
            if addition.len() >= 64 {
                break;
            }
        }
        let objects = self.schema;
        let owners = [format!("table {table}"), format!("view {table}")];
        let id = |n: &str| format!("index {schema}.{}", quote_ident(n));
        let has = |full: &str| objects.objects.get(full).is_some_and(|o| o.owner.as_ref().is_some_and(|w| owners.contains(w)));
        self.choose(candidates(relname, Some(&addition), "idx"), id, has)
    }

    fn domain_check(&mut self, domain: &str, bare: &str, c: &protobuf::Constraint) {
        let name = if c.conname.is_empty() {
            let schema = self.schema;
            let id = |n: &str| format!("check {domain} {}", quote_ident(n));
            let has = |full: &str| {
                let (_, name) = full.rsplit_once(' ').unwrap_or_default();
                schema.objects.get(domain).is_some_and(|o| o.extra.iter().any(|l| l.starts_with(&format!("check {name} "))))
            };
            match self.choose(candidates(bare, None, "check"), id, has) {
                Some(name) => name,
                None => return,
            }
        } else {
            c.conname.clone()
        };
        let key = format!("check {domain} {}", quote_ident(&name));
        self.given.insert(key.clone());
        self.expression(key, c.raw_expr.as_deref());
    }

    fn read(&mut self, sql: &str, raw: &protobuf::RawStmt) {
        let Some(node) = raw.stmt.as_deref().and_then(|n| n.node.as_ref()) else { return };
        let text = || source(sql, raw);
        match node {
            N::VariableSetStmt(v) if v.name == "search_path" => {
                self.path = match protobuf::VariableSetKind::try_from(v.kind) {
                    Ok(protobuf::VariableSetKind::VarSetValue) => v
                        .args
                        .iter()
                        .filter_map(|a| match &a.node {
                            Some(N::AConst(protobuf::AConst { val: Some(protobuf::a_const::Val::Sval(s)), .. })) => Some(s.sval.clone()),
                            _ => None,
                        })
                        .collect(),
                    _ => self.base.clone(),
                };
                self.keep_existing();
            }
            // pg_dump's `select pg_catalog.set_config('search_path', '', false)`.
            N::SelectStmt(s) => {
                for call in find(s.as_ref(), "FuncCall") {
                    let name = call["funcname"].as_array().and_then(|f| f.last()).and_then(|f| f.pointer("/node/String/sval")).and_then(|v| v.as_str());
                    let args = call["args"].as_array().cloned().unwrap_or_default();
                    let arg = |k: usize| args.get(k).and_then(|a| a.pointer("/node/AConst/val/Sval/sval")).and_then(|v| v.as_str()).map(String::from);
                    if name == Some("set_config") && arg(0).as_deref() == Some("search_path") {
                        self.path = arg(1).unwrap_or_default().split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect();
                        self.keep_existing();
                    }
                }
            }
            N::CreateStmt(c) => {
                let Some(r) = &c.relation else { return };
                if r.relpersistence == "t" || c.partbound.is_some() || c.of_typename.is_some() {
                    return;
                }
                let Some(schema) = self.creation(&r.schemaname) else { return };
                let table = qualified(&schema, &r.relname);
                self.forget_table(&table);
                self.out.untrusted.remove(&format!("table {table}"));
                if c.table_elts.iter().any(|e| matches!(e.node, Some(N::TableLikeClause(_)))) {
                    self.out.untrusted.insert(format!("table {table}"));
                }
                for e in &c.table_elts {
                    match &e.node {
                        Some(N::ColumnDef(d)) => self.column(&table, &r.relname, d),
                        Some(N::Constraint(k)) if k.contype == ConstrType::ConstrCheck as i32 => self.check(&table, &r.relname, k),
                        _ => {}
                    }
                }
            }
            N::AlterTableStmt(a) => {
                let Some((table, relname)) = a.relation.as_ref().and_then(|r| self.relation(r)) else { return };
                if a.objtype != ObjectType::ObjectTable as i32 {
                    self.forget_table(&table);
                    return;
                }
                for cmd in &a.cmds {
                    let Some(N::AlterTableCmd(cmd)) = &cmd.node else { continue };
                    let column = format!("column {table}.{}", quote_ident(&cmd.name));
                    let def = cmd.def.as_deref();
                    match At::try_from(cmd.subtype) {
                        Ok(At::AtAddColumn) => {
                            if let Some(N::ColumnDef(d)) = def.and_then(|d| d.node.as_ref()) {
                                self.column(&table, &relname, d);
                            }
                        }
                        Ok(At::AtColumnDefault) => self.expression(format!("default {column}"), def),
                        Ok(At::AtSetExpression) => self.expression(format!("generated {column}"), def),
                        Ok(At::AtDropExpression) => self.expression(format!("generated {column}"), None),
                        Ok(At::AtAddConstraint) => {
                            if let Some(N::Constraint(k)) = def.and_then(|d| d.node.as_ref()) {
                                if k.contype == ConstrType::ConstrCheck as i32 {
                                    self.check(&table, &relname, k);
                                }
                            }
                        }
                        Ok(At::AtDropConstraint) => self.expression(format!("check constraint {table}.{}", quote_ident(&cmd.name)), None),
                        Ok(At::AtDropColumn) => {
                            self.expression(format!("default {column}"), None);
                            self.expression(format!("generated {column}"), None);
                        }
                        Ok(At::AtAlterColumnType) => {
                            self.out.untrusted.insert(format!("table {table}"));
                        }
                        _ => {}
                    }
                }
            }
            N::IndexStmt(i) => {
                let Some((table, relname)) = i.relation.as_ref().and_then(|r| self.relation(r)) else { return };
                let schema = table.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or_default();
                let name = if i.idxname.is_empty() {
                    let Some(name) = self.index_name(&table, &schema, &relname, &i.index_params) else { return };
                    name
                } else {
                    i.idxname.clone()
                };
                let id = format!("index {schema}.{}", quote_ident(&name));
                self.given.insert(id.clone());
                // Written as declared, unless it has no name to write or builds concurrently,
                // which no transaction may: then as the parser writes it, named and built at once.
                let text = if i.idxname.is_empty() || i.concurrent {
                    let mut named = i.as_ref().clone();
                    named.idxname = name;
                    named.concurrent = false;
                    Node { node: Some(N::IndexStmt(Box::new(named))) }.deparse(pg_query::DeparseOptions::default()).ok()
                } else {
                    text()
                };
                self.statement(id, text);
            }
            N::ViewStmt(v) => {
                let Some(r) = &v.view else { return };
                let Some(schema) = self.creation(&r.schemaname) else { return };
                // A `*` is expanded when the view is made, to the columns its tables have then.
                let starred = !find(v.as_ref(), "AStar").is_empty();
                self.statement(format!("view {}", qualified(&schema, &r.relname)), if starred { None } else { text() });
            }
            N::CreateTableAsStmt(c) if c.objtype == ObjectType::ObjectMatview as i32 => {
                let Some(r) = c.into.as_ref().and_then(|i| i.rel.as_ref()) else { return };
                let Some(schema) = self.creation(&r.schemaname) else { return };
                let starred = !find(c.as_ref(), "AStar").is_empty();
                self.statement(format!("view {}", qualified(&schema, &r.relname)), if starred { None } else { text() });
            }
            N::CreateFunctionStmt(f) => {
                let parts = names(&f.funcname);
                self.function(&parts, &f.parameters, text());
            }
            N::AlterFunctionStmt(f) => {
                let parts = f.func.as_ref().map(|f| names(&f.objname)).unwrap_or_default();
                if let Some(q) = self.resolve(&parts, &["function"]) {
                    self.out.statements.retain(|k, _| !k.starts_with(&format!("function {q}(")));
                }
            }
            N::CreateTrigStmt(t) => {
                let Some((table, _)) = t.relation.as_ref().and_then(|r| self.relation(r)) else { return };
                self.statement(format!("trigger {table}.{}", quote_ident(&t.trigname)), text());
            }
            N::CreatePolicyStmt(p) => {
                let Some((table, _)) = p.table.as_ref().and_then(|r| self.relation(r)) else { return };
                self.statement(format!("policy {table}.{}", quote_ident(&p.policy_name)), text());
            }
            N::AlterPolicyStmt(p) => {
                let Some((table, _)) = p.table.as_ref().and_then(|r| self.relation(r)) else { return };
                self.statement(format!("policy {table}.{}", quote_ident(&p.policy_name)), None);
            }
            N::RuleStmt(r) => {
                let Some((table, _)) = r.relation.as_ref().and_then(|v| self.relation(v)) else { return };
                self.statement(format!("rule {table}.{}", quote_ident(&r.rulename)), text());
            }
            N::CreatePublicationStmt(p) => self.statement(format!("publication {}", quote_ident(&p.pubname)), text()),
            N::AlterPublicationStmt(p) => self.statement(format!("publication {}", quote_ident(&p.pubname)), None),
            N::CreateDomainStmt(d) => {
                let parts = names(&d.domainname);
                let Some(bare) = parts.last().cloned() else { return };
                let Some(schema) = (if parts.len() >= 2 { Some(parts[parts.len() - 2].clone()) } else { self.creation("") }) else { return };
                let domain = format!("type {}", qualified(&schema, &bare));
                self.out.expressions.retain(|k, _| !k.starts_with(&format!("check {domain} ")) && *k != format!("default {domain}"));
                self.statement(domain.clone(), text());
                for c in &d.constraints {
                    let Some(N::Constraint(c)) = &c.node else { continue };
                    match ConstrType::try_from(c.contype) {
                        Ok(ConstrType::ConstrDefault) => self.expression(format!("default {domain}"), c.raw_expr.as_deref()),
                        Ok(ConstrType::ConstrCheck) => self.domain_check(&domain, &bare, c),
                        _ => {}
                    }
                }
            }
            N::AlterDomainStmt(d) => {
                let parts = names(&d.type_name);
                let Some(q) = self.resolve(&parts, &["type"]) else { return };
                let domain = format!("type {q}");
                let bare = parts.last().cloned().unwrap_or_default();
                // The domain's statement no longer declares it whole.
                self.out.statements.remove(&domain);
                let def = d.def.as_deref();
                match d.subtype.as_str() {
                    "T" => self.expression(format!("default {domain}"), def),
                    "C" => {
                        if let Some(N::Constraint(c)) = def.and_then(|d| d.node.as_ref()) {
                            if c.contype == ConstrType::ConstrCheck as i32 {
                                self.domain_check(&domain, &bare, c);
                            }
                        }
                    }
                    "X" => self.expression(format!("check {domain} {}", quote_ident(&d.name)), None),
                    _ => {}
                }
            }
            N::DropStmt(d) => {
                for o in &d.objects {
                    let parts = match &o.node {
                        Some(N::List(l)) => names(&l.items),
                        Some(N::TypeName(t)) => names(&t.names),
                        Some(N::ObjectWithArgs(f)) => names(&f.objname),
                        Some(N::String(s)) => vec![s.sval.clone()],
                        _ => continue,
                    };
                    match ObjectType::try_from(d.remove_type) {
                        Ok(ObjectType::ObjectTable | ObjectType::ObjectView | ObjectType::ObjectMatview | ObjectType::ObjectIndex | ObjectType::ObjectForeignTable) => {
                            if let Some(q) = self.resolve(&parts, &["table", "view", "index"]) {
                                self.forget_table(&q);
                            }
                        }
                        Ok(ObjectType::ObjectFunction | ObjectType::ObjectProcedure | ObjectType::ObjectRoutine) => {
                            if let Some(q) = self.resolve(&parts, &["function"]) {
                                self.out.statements.retain(|k, _| !k.starts_with(&format!("function {q}(")));
                            }
                        }
                        Ok(ObjectType::ObjectDomain | ObjectType::ObjectType) => {
                            if let Some(q) = self.resolve(&parts, &["type"]) {
                                self.out.statements.remove(&format!("type {q}"));
                            }
                        }
                        Ok(ObjectType::ObjectTrigger | ObjectType::ObjectPolicy | ObjectType::ObjectRule) => {
                            let Some((name, table)) = parts.split_last() else { continue };
                            let Some(q) = self.resolve(table, &["table", "view"]) else { continue };
                            for kind in ["trigger", "policy", "rule"] {
                                self.out.statements.remove(&format!("{kind} {q}.{}", quote_ident(name)));
                            }
                        }
                        _ => {}
                    }
                }
            }
            // A rename, or a move to another schema, leaves the text declared before it naming
            // what is no longer there: only what is declared after it is followed.
            N::RenameStmt(_) | N::AlterObjectSchemaStmt(_) => {
                self.out.statements.clear();
                self.out.expressions.clear();
            }
            _ => {}
        }
    }

    /// The path as the server searches it: only the schemas that exist.
    fn keep_existing(&mut self) {
        let schema = self.schema;
        self.path.retain(|s| schema.objects.contains_key(&format!("schema {}", quote_ident(s))));
    }
}

/// What `sql` declares of `schema`, under the search_path `base` (the schemas that exist in it).
pub(super) fn read(schema: &Schema, sql: &str, base: &[String]) -> Declared {
    let mut reader = Reader { schema, path: base.to_vec(), base: base.to_vec(), out: Declared::default(), given: BTreeSet::new() };
    reader.keep_existing();
    reader.base = reader.path.clone();
    if let Ok(parsed) = pg_query::parse(sql, 0) {
        for raw in &parsed.protobuf.stmts {
            reader.read(sql, raw);
        }
    }
    reader.out
}

/// The index of the parenthesis that closes the one at `open`, past quoted text.
pub(super) fn closing(text: &str, open: usize) -> Option<usize> {
    let (mut depth, mut quote) = (0, None);
    for (k, c) in text.char_indices().skip_while(|(k, _)| *k < open) {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}

/// Rewrites the create statements of `schema`'s objects from what `declared` holds.
pub(super) fn apply(schema: &mut Schema, declared: &Declared) {
    for (id, text) in &declared.statements {
        if let Some(o) = schema.objects.get_mut(id) {
            if let Some(first) = o.create.first_mut() {
                *first = text.clone();
                schema.verbatim.insert(text.clone());
            }
        }
    }
    // Column definitions and check constraints, then the tables that write them inline.
    let mut inline: Vec<(String, String, String)> = Vec::new();
    for o in schema.objects.values_mut() {
        let Some(owner) = o.owner.clone() else { continue };
        if declared.untrusted.contains(&owner) || !owner.starts_with("table ") {
            continue;
        }
        match o.kind {
            Some(Kind::Column) => {
                let old = bare(&o.def).to_string();
                let mut c = parse_column(&old);
                let mut extra = Vec::new();
                if let (Some(_), Some(text)) = (&c.default, declared.expressions.get(&format!("default {}", o.id))) {
                    c.default = Some(text.clone());
                    extra.push(format!("default {text}"));
                }
                if let (Some(g), Some(text)) = (c.generated.clone(), declared.expressions.get(&format!("generated {}", o.id))) {
                    let kind = g.rsplit(' ').next().unwrap_or("stored");
                    c.generated = Some(format!("generated always as ({text}) {kind}"));
                    extra.push(format!("generated {text}"));
                }
                if extra.is_empty() {
                    continue;
                }
                let def = render_column(&c);
                let name = o.id.rsplit('.').next().unwrap_or_default().to_string();
                if let Some(first) = o.create.first_mut() {
                    if let Some(head) = first.strip_suffix(old.as_str()) {
                        *first = format!("{head}{def}");
                    }
                }
                inline.push((owner, format!("{name} {old}"), format!("{name} {def}")));
                o.extra.extend(extra);
            }
            Some(Kind::Constraint) => {
                let Some(text) = declared.expressions.get(&format!("check {}", o.id)) else { continue };
                let old = bare(&o.def).to_string();
                let Some(close) = old.strip_prefix("CHECK ").and_then(|_| closing(&old, 6)) else { continue };
                let def = format!("CHECK ({text}){}", &old[close + 1..]);
                let name = o.id.rsplit('.').next().unwrap_or_default().to_string();
                if let Some(first) = o.create.first_mut() {
                    if let Some(head) = first.strip_suffix(old.as_str()) {
                        *first = format!("{head}{def}");
                    }
                }
                if o.inline {
                    inline.push((owner, format!("constraint {name} {old}"), format!("constraint {name} {def}")));
                }
            }
            _ => {}
        }
    }
    for (table, old, new) in inline {
        let Some(t) = schema.objects.get_mut(&table) else { continue };
        if let Some(first) = t.create.first_mut() {
            for end in [",", "\n"] {
                let from = format!("\n  {old}{end}");
                if first.contains(&from) {
                    *first = first.replacen(&from, &format!("\n  {new}{end}"), 1);
                    break;
                }
            }
        }
    }
    // The statements a column's or an enum's change makes again, as now written.
    let creates: BTreeMap<String, String> = schema.objects.values().map(|o| (o.id.clone(), o.create.join("; "))).collect();
    for o in schema.objects.values_mut() {
        for line in o.extra.iter_mut().filter(|l| l.starts_with(AROUND)) {
            let mut parts: Vec<String> = line.splitn(4, '\t').map(String::from).collect();
            if let (Some(id), Some(create)) = (parts.first().and_then(|p| p.strip_prefix(AROUND)), parts.get(3)) {
                if let Some(now) = creates.get(id).filter(|c| *c != create) {
                    parts[3] = now.clone();
                    *line = parts.join("\t");
                }
            }
        }
    }
    // A domain's default and checks, which its alterer writes.
    for o in schema.objects.values_mut().filter(|o| o.kind == Some(Kind::Type) && o.def.starts_with("domain ")) {
        if let Some(text) = declared.expressions.get(&format!("default {}", o.id)) {
            o.extra.push(format!("declared default {text}"));
        }
        let prefix = format!("check {} ", o.id);
        for (key, text) in declared.expressions.range(prefix.clone()..) {
            let Some(name) = key.strip_prefix(&prefix) else { break };
            o.extra.push(format!("declared check {name} {text}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_as_postgres_names_them() {
        assert_eq!(quote_ident("users"), "users");
        assert_eq!(quote_ident("user"), "\"user\"");
        assert_eq!(quote_ident("Users"), "\"Users\"");
        assert_eq!(quote_ident("name"), "name");
        assert_eq!(object_name("accounts", Some("tier"), "check"), "accounts_tier_check");
        let long = "a".repeat(60);
        assert_eq!(object_name(&long, Some("column"), "check").len(), 63);
    }

    #[test]
    fn deparses_an_expression_as_declared() {
        let parsed = pg_query::parse("create table t (tier varchar(10) check (tier in ('free', 'pro')))", 0).unwrap();
        let checks = find(&parsed.protobuf, "Constraint");
        assert_eq!(checks.len(), 1);
        let N::CreateStmt(c) = parsed.protobuf.stmts[0].stmt.as_ref().unwrap().node.as_ref().unwrap() else { panic!() };
        let Some(N::ColumnDef(d)) = &c.table_elts[0].node else { panic!() };
        let Some(N::Constraint(k)) = &d.constraints[0].node else { panic!() };
        assert_eq!(expression(k.raw_expr.as_deref().unwrap()).unwrap(), "tier IN ('free', 'pro')");
        assert_eq!(columns_named(k.raw_expr.as_deref().unwrap()).into_iter().collect::<Vec<_>>(), ["tier"]);
    }

    #[test]
    fn matches_declarations_to_the_objects_they_made() {
        let mut schema = Schema::default();
        let add = |schema: &mut Schema, id: &str, def: &str, owner: Option<&str>| schema.add(crate::diff::Object { id: id.into(), def: def.into(), owner: owner.map(String::from), create: vec!["rendered".into()], ..Default::default() });
        add(&mut schema, "schema public", "", None);
        add(&mut schema, "schema other", "", None);
        add(&mut schema, "table public.t", "", None);
        add(&mut schema, "constraint public.t.t_x_check", "CHECK ((x > 0))", Some("table public.t"));
        add(&mut schema, "constraint public.t.t_x_check1", "CHECK ((x < 9))", Some("table public.t"));
        add(&mut schema, "index public.t_x_idx", "", Some("table public.t"));
        add(&mut schema, "function public.f(integer)", "", None);
        add(&mut schema, "function public.f(text)", "", None);
        add(&mut schema, "view public.v", "", None);
        add(&mut schema, "view other.w", "", None);
        let sql = "create table t (x integer check (x > 0), check (x < 9));
            create index on t (x);
            create function f(a int) returns int language sql as 'select a';
            create function f(a text) returns int language sql as 'select 1';
            create view v as select x from t;
            set search_path = other, public;
            create view w as select x from t;";
        let d = read(&schema, sql, &["public".to_string()]);
        assert_eq!(d.expressions.get("check constraint public.t.t_x_check").map(String::as_str), Some("x > 0"));
        assert_eq!(d.expressions.get("check constraint public.t.t_x_check1").map(String::as_str), Some("x < 9"));
        assert_eq!(d.statements.get("index public.t_x_idx").map(String::as_str), Some("CREATE INDEX t_x_idx ON t USING btree (x)"));
        assert!(d.statements["function public.f(integer)"].contains("'select a'"));
        assert!(d.statements["function public.f(text)"].contains("'select 1'"));
        assert_eq!(d.statements.get("view public.v").map(String::as_str), Some("create view v as select x from t"));
        // Under a search_path of its own, `t` may be another table: the rendering stays.
        assert!(!d.statements.contains_key("view other.w"));
    }

    #[test]
    fn keeps_a_statement_without_its_comments() {
        let sql = "-- the users\ncreate view v as select 1; /* after */ -- tail\n create index i on t (x) -- why\n;";
        let parsed = pg_query::parse(sql, 0).unwrap();
        assert_eq!(source(sql, &parsed.protobuf.stmts[0]).unwrap(), "create view v as select 1");
        assert_eq!(source(sql, &parsed.protobuf.stmts[1]).unwrap(), "create index i on t (x)");
        assert_eq!(id_arguments("function public.f(integer,\"a,b\",numeric)"), ["integer", "\"a,b\"", "numeric"]);
    }
}
