//! The SQL Server analysis. The server describes the statement itself, in two catalog queries a
//! round apart: `sp_describe_undeclared_parameters` types each parameter from where it stands, and
//! `sys.dm_exec_describe_first_result_set`, given those types, describes each result column with
//! its type and whether it may be NULL. That nullability is the server's own analysis: it nulls
//! the columns an outer join null-extends and an aggregate over no rows, and calls most computed
//! columns nullable, `count(*)` among them. The engine takes it as it is and narrows it where a
//! rule of its own holds ([`crate::nulls`]); it reads a `sqlparser` tree of the statement for how
//! many rows it returns ([`crate::cardinality`]) and which parameters may be null
//! ([`crate::params`]).
//!
//! A file is a batch: one `SELECT`, `INSERT`, `UPDATE`, `DELETE` or `MERGE`, its rows the
//! `SELECT`'s or the `OUTPUT` clause's, or several statements with variables, `IF`, `WHILE` and
//! `EXEC`, whose result is the one statement that makes rows ([`crate::batch`]). The server
//! describes a batch's first result set across every path through it, and the engine's rules read
//! the statement that makes it as they read a statement alone.

use crate::api::{Analysis, Column, Diagnostic, Param, Scope};
use crate::dialect::Dialect;
use crate::host::{as_str, Host, Lazy};
use crate::lex::{self, Lexed, Style, Target};
use crate::types::Described;
use reylo_engine_core::analysis::ResultTarget;
use reylo_engine_core::fresh::Tables;
use reylo_engine_core::host::as_bool;
use reylo_engine_core::types::{Cardinality, Slot, StatementKind, Type};
use serde_json::{json, Value};
use sqlparser::ast::{Expr, FromTable, ObjectName, OutputClause, Query, SelectItem, SetExpr, Statement, TableFactor, TableObject, TableWithJoins};

/// The parameters' types, deduced from where each stands: one row per parameter, its name in
/// column 1 and its type in columns 3 to 6 (`suggested_system_type_name`, `suggested_max_length`,
/// `suggested_precision`, `suggested_scale`). An error where a parameter's type cannot be deduced.
pub const UNDECLARED: &str = "exec sys.sp_describe_undeclared_parameters @tsql = @P1";

/// The database's default collation, which a parameter of a character type takes.
pub const DATABASE_COLLATION: &str = "select convert(nvarchar(128), databasepropertyex(db_name(), 'Collation'))";

/// The first result set's columns, given the parameters' declarations (`@P2`, NULL for none), in
/// browse mode, which names each column's source and adds the source's hidden key columns. A
/// statement the server refuses is one row with its error in the last two columns, not an error.
pub const FIRST_RESULT_SET: &str = "select is_hidden, column_ordinal, name, is_nullable, system_type_name, max_length, precision, scale, collation_name, \
    source_schema, source_table, source_column, is_part_of_unique_key, error_number, error_message \
    from sys.dm_exec_describe_first_result_set(@P1, @P2, 1) order by column_ordinal";

/// One column of the first result set, as [`FIRST_RESULT_SET`] reads it.
#[derive(Debug, Clone)]
pub struct Described1 {
    pub name: String,
    pub nullable: bool,
    pub ty: Described,
}

/// The parameters each of a statement's are deduced as, in binding order; `None` where the server
/// named none, which the deduction does for a parameter it has no context for.
type Deduced = Vec<Option<Described>>;

/// The analysis of a statement, its structured parameters refused first ([`crate::structured`]).
pub fn analyze_text(sql: &str, host: &Host) -> Lazy<Analysis> {
    reylo_engine_core::structured::analyze(sql, Dialect::Sqlserver, host, &|sql, _| analyze(sql, host), &|diagnostics| crate::analysis::bare_of(sql, Dialect::Sqlserver, diagnostics))
}

fn whole(sql: &str, message: String, code: u32, server_code: Option<String>) -> Diagnostic {
    Diagnostic { message, start: 0, length: sql.trim_end().len().max(1), code, server_code }
}

fn no_lexed() -> Lexed {
    Lexed { params: vec![], placeholders: vec![], style: Style::None, opaque: vec![] }
}

pub fn analyze(sql: &str, host: &Host) -> Lazy<Analysis> {
    let lexed = match lex::lex(sql, Dialect::Sqlserver) {
        Ok(lexed) => lexed,
        Err(e) => return Some(crate::analysis::bare(sql, &no_lexed(), Dialect::Sqlserver, vec![Diagnostic { message: e.message, start: e.offset, length: 1, code: 1, server_code: None }])),
    };
    let refuse = |d: Vec<Diagnostic>| Some(crate::analysis::bare(sql, &lexed, Dialect::Sqlserver, d));
    // A batch reads a value twice through a variable, which the parameter's one use declares.
    let repeated: Vec<Diagnostic> = crate::analysis::repeated(sql, &lexed)
        .into_iter()
        .map(|mut d| {
            let written = &sql[d.start..d.start + d.length];
            d.message = format!("The parameter {written} appears more than once; use each parameter once, and to read it twice, read it into a variable first, as in: declare @v <type> = {written}");
            d
        })
        .collect();
    if !repeated.is_empty() {
        return refuse(repeated);
    }
    if let Some(at) = go_line(sql, &lexed) {
        return refuse(vec![Diagnostic { message: GO.into(), start: at, length: 2, code: 10, server_code: None }]);
    }
    let rewritten = lex::rewrite(sql, &lexed, Target::AtNumbered);
    // The author's text for each `@p_n` the server's messages name.
    let authored = |message: &str| -> String {
        let mut out = message.to_string();
        for (i, name) in rewritten.order.iter().enumerate().rev() {
            if let Some(p) = lexed.placeholders.iter().find(|p| &p.name == name) {
                out = out.replace(&format!("@p_{}", i + 1), &sql[p.start..p.end]);
            }
        }
        out
    };
    // The batch's statements, and the one whose rows are the result ([`crate::batch`]).
    let statements = crate::batch::parse(&rewritten.sql);
    let read = statements.as_deref().map(crate::batch::read);
    if let Some(r) = read.as_ref().filter(|r| r.per_run > 1) {
        return refuse(vec![whole(sql, format!("{SEVERAL_RESULTS} This one may return {}.", if r.per_run > 2 { r.per_run.to_string() } else { "two or more".into() }), 10, None)]);
    }
    // The statement that makes the result set, where one alone does: where each branch of an `if`
    // makes one, the server's describe types them together, and the rules read neither.
    let found = read.as_ref().filter(|r| r.results.len() == 1).and_then(|r| r.results.first());
    // The statement the rules read: the result's, where they may.
    let parsed: Option<&Statement> = found.filter(|f| f.narrows).map(|f| f.statement);
    if let Some(message) = found.and_then(|f| refused(f.statement)) {
        return refuse(vec![whole(sql, message, 10, None)]);
    }
    let all: Vec<&Statement> = statements.as_deref().map(crate::batch::flatten).unwrap_or_default();
    // The first round: each parameter's type, and the collation one of a character type takes.
    let deduced: Deduced = if rewritten.order.is_empty() {
        Vec::new()
    } else {
        let collation = host.query(DATABASE_COLLATION, Vec::new(), Scope::Session);
        let undeclared = host.query(UNDECLARED, vec![json!(rewritten.sql)], Scope::Statement);
        let collation = collation?.ok().and_then(|rows| rows.first()?.first().and_then(as_str).map(String::from));
        match undeclared? {
            Ok(rows) => deduced(&rows, rewritten.order.len(), collation.as_deref()),
            Err(e) => {
                let message = authored(&e.message);
                // A parameter the server cannot type takes a cast, at the parameter.
                let at = cannot_deduce(&e.code).then(|| rewritten.order.iter().enumerate().find(|(i, _)| e.message.contains(&format!("'@p_{}'", i + 1)))).flatten();
                return match at.and_then(|(_, name)| lexed.placeholders.iter().find(|p| &p.name == name)) {
                    Some(p) => {
                        let written = &sql[p.start..p.end];
                        let message = format!("SQL Server cannot tell the type of {written} here ({}): give it one, as in cast({written} as int).", message.trim_end_matches('.'));
                        refuse(vec![Diagnostic { message, start: p.start, length: p.end - p.start, code: 6, server_code: e.code.clone() }])
                    }
                    None => refuse(vec![whole(sql, message, 6, e.code.clone())]),
                };
            }
        }
    };
    let declarations = declarations(&deduced);
    let declared = declarations.clone().map_or(Value::Null, Value::from);
    // The second round, asked at once: the statement's result, the probe of the columns its
    // computed columns read ([`crate::nulls`]), the unique keys of the tables its row count turns
    // on ([`crate::cardinality`]), and the columns of the table it writes ([`crate::params`]).
    let described = host.query(FIRST_RESULT_SET, vec![json!(rewritten.sql), declared.clone()], Scope::Statement);
    let mut plan = parsed.map(crate::nulls::plan).unwrap_or_default();
    if let (Some(probe), Some(f)) = (plan.probe.as_mut(), found) {
        probe.insert_str(0, &f.prelude);
    }
    let probe = plan.probe.as_ref().map(|probe| host.query(FIRST_RESULT_SET, vec![json!(probe), declared.clone()], Scope::Statement));
    // The JSON its columns build, and the probes that type its values ([`crate::json`]).
    let json_plan = parsed.map(crate::json::plan).unwrap_or_default();
    let prelude = found.map(|f| f.prelude.as_str()).unwrap_or_default();
    let json_probes: Vec<_> = json_plan.probes.iter().map(|p| host.query(FIRST_RESULT_SET, vec![json!(format!("{prelude}{p}")), declared.clone()], Scope::Statement)).collect();
    let keys = parsed.map(|s| crate::cardinality::keys(s, host));
    // A written table's columns are read where the engine follows every statement: one it does not,
    // as a `use`, may move the batch to another database's tables.
    let followed = read.as_ref().is_some_and(|r| r.understood);
    let written: Vec<_> = if followed { all.iter().map(|s| crate::params::columns(s, host)).collect() } else { Vec::new() };
    let described = described?;
    let probe = match probe {
        Some(p) => Some(p?),
        None => None,
    };
    // A probe the server refused types its values as nothing.
    let mut json_described: Vec<Option<Vec<Described1>>> = Vec::new();
    for p in json_probes {
        json_described.push(p?.ok().filter(|rows| rows.iter().all(|r| r.get(13).is_none_or(Value::is_null))).map(|rows| rows.iter().filter(|r| !r.first().and_then(as_bool).unwrap_or(false)).map(described_column).collect()));
    }
    let keys = match keys {
        Some(k) => Some(k?),
        None => None,
    };
    let mut columns_written = Vec::new();
    for w in written {
        columns_written.push(w?);
    }
    let rows = match described {
        Ok(rows) => rows,
        Err(e) => return refuse(vec![whole(sql, authored(&e.message), 6, e.code.clone())]),
    };
    if let Some(row) = rows.iter().find(|r| !r.get(13).is_none_or(Value::is_null)) {
        let code = row.get(13).map(|c| c.to_string().trim_matches('"').to_string());
        let message = authored(row.get(14).and_then(as_str).unwrap_or("the server could not describe the statement"));
        let message = match code.as_deref().and_then(undescribed) {
            Some(how) => format!("{} {how}", message.trim_end()),
            None => message,
        };
        return refuse(vec![whole(sql, message, 6, code)]);
    }
    let described: Vec<Described1> = rows.iter().filter(|r| !r.first().and_then(as_bool).unwrap_or(false)).map(described_column).collect();
    // A probe the server refused narrows nothing.
    let probed: Option<Vec<bool>> = probe.and_then(|p| p.ok()).filter(|rows| rows.iter().all(|r| r.get(13).is_none_or(Value::is_null))).map(|rows| rows.iter().filter(|r| !r.first().and_then(as_bool).unwrap_or(false)).map(|r| r.get(3).and_then(as_bool).unwrap_or(true)).collect());
    let never_null = crate::nulls::never_null(parsed, &plan, probed.as_deref(), described.len());
    let columns: Vec<Column> = described
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let nullable = c.nullable && !never_null.get(i).copied().unwrap_or(false);
            match json_plan.columns.get(i).cloned().flatten() {
                // A document the statement builds: what it holds, and NULL only where the shape may be.
                Some(shape) => {
                    let (content, _) = crate::json::resolve(&shape, &json_described);
                    Column::new(c.name.clone(), Type::json_of(Slot::new(content, false)), nullable && shape.nullable())
                }
                None => Column::new(c.name.clone(), crate::types::of(&c.ty), nullable),
            }
        })
        .collect();
    let nullable: std::collections::HashSet<String> = all.iter().enumerate().flat_map(|(i, s)| crate::params::nullable(s, &rewritten.order, columns_written.get(i))).collect();
    let params: Vec<Param> = lexed
        .params
        .iter()
        .map(|name| {
            let ty = rewritten.order.iter().position(|n| n == name).and_then(|i| deduced.get(i).cloned().flatten()).map_or_else(Type::unknown, |d| crate::types::of(&d));
            Param { nullable: nullable.contains(name), ..Param::new(name.clone(), ty) }
        })
        .collect();
    // A batch of one statement is that statement's kind; a batch of several is of none of them.
    let kind = match statements.as_deref() {
        Some([one]) => kind(one),
        Some(_) => Some(StatementKind::Other),
        None => None,
    };
    let (cardinality, reason) = match (parsed, found, &read) {
        (Some(statement), Some(f), _) => {
            let (cardinality, reason) = crate::cardinality::of(statement, keys.as_ref(), columns.len(), &f.variables);
            match cardinality {
                // A result that may not come is at most one row where its statement returns one.
                Cardinality::One if !f.sure => (Cardinality::AtMostOne, Some("the statement that returns the row may not run, as in a branch of an if".into())),
                _ => (cardinality, reason),
            }
        }
        _ if columns.is_empty() => (Cardinality::None, None),
        (None, Some(_), _) => (Cardinality::Many, Some("the rows come from a procedure, from a statement after a set that turns off ansi_warnings, or from one after a statement the engine does not follow, as a use, which nothing bounds".into())),
        (_, None, Some(r)) if r.understood => (Cardinality::Many, Some("each branch of an if returns rows of its own, which nothing bounds together".into())),
        (_, None, Some(_)) => (Cardinality::Many, Some("the batch holds a statement the engine does not follow, so nothing bounds its rows".into())),
        _ => (Cardinality::Many, Some("the engine could not parse the batch, so nothing bounds its rows".into())),
    };
    let targets = found.and_then(|f| result_targets(f.statement));
    let mut analysis = Analysis {
        kind,
        comment: None,
        params,
        columns,
        cardinality,
        cardinality_reason: reason,
        key: None,
        unique: Vec::new(),
        order: Vec::new(),
        ordered: false,
        value: false,
        rewritten,
        style: lexed.style,
        diagnostics: Vec::new(),
        unresolved: Vec::new(),
        structured: Vec::new(),
        fresh: None,
        errors: None,
        capabilities: None,
        read_only: statements.is_some() && all.iter().all(|s| read_only(s)),
        notes: Vec::new(),
        tables: statements.as_ref().map(|_| all_tables(&all)),
        domains: Default::default(),
    };
    crate::analysis::names(sql, Dialect::Sqlserver, &mut analysis, targets.as_deref(), &|_| None);
    Some(analysis)
}

const GO: &str = "A typed .sql file is one batch, and a GO line separates two, which a driver does not send: leave it out, and the statements run as one batch.";
const SEVERAL_RESULTS: &str = "Reylo types a batch's first result set, and a typed .sql file returns one: a select that returns rows, or a write with an output, in each path through the batch.";

/// Why a statement is not one the analysis reads, by what its tree is.
fn refused(statement: &Statement) -> Option<String> {
    match statement {
        Statement::Query(q) if q.for_clause.as_ref().is_some_and(|f| !matches!(f, sqlparser::ast::ForClause::Browse)) => {
            Some("SQL Server sends a top-level FOR JSON's or FOR XML's text in pieces, which drivers join differently: select it as a column instead, as in select (select … for json path) as doc.".into())
        }
        _ => None,
    }
}

/// What to write instead of a batch the server's describe refused, by its error: a temporary table
/// (11525, 11526), branches whose result sets differ (11509 to 11512), SQL built at run time (11513,
/// 11514), a cursor (11527).
fn undescribed(code: &str) -> Option<&'static str> {
    Some(match code {
        "11525" | "11526" => "SQL Server describes no batch that fills a temporary table: use a table variable, as in declare @t table (id int).",
        "11509" | "11510" | "11511" | "11512" => "Return the same columns from every branch, or put each in a file of its own.",
        "11513" | "11514" => "Name what the SQL built at run time returns, as in exec (@sql) with result sets ((id int, name nvarchar(100))).",
        "11527" => "Read the rows with a select, into a table variable where the batch needs them in steps.",
        _ => return None,
    })
}

/// Whether the server's error says it could not deduce a parameter's type: in this context
/// (11502), from two untyped parameters in one expression (11503), or among several possible
/// types (11506).
fn cannot_deduce(code: &Option<String>) -> bool {
    matches!(code.as_deref(), Some("11502" | "11503" | "11506"))
}

/// Each parameter's deduced type, in binding order, by the name the rewrite gave it (`@p_1`).
/// A parameter of a character type takes the database's default collation, whose code page its
/// length in bytes is counted in.
fn deduced(rows: &[Vec<Value>], count: usize, collation: Option<&str>) -> Deduced {
    let mut out: Deduced = vec![None; count];
    for row in rows {
        let Some(name) = row.get(1).and_then(as_str) else { continue };
        let Some(n) = name.strip_prefix("@p_").and_then(|n| n.parse::<usize>().ok()) else { continue };
        let Some(ty) = row.get(3).and_then(as_str) else { continue };
        if n == 0 || n > count {
            continue;
        }
        out[n - 1] = Some(Described { name: ty.to_string(), max_length: int(row.get(4)), precision: int(row.get(5)) as u32, scale: int(row.get(6)) as u32, collation: collation.map(String::from) });
    }
    out
}

/// The parameters' declarations, as the describe of the result takes them: `@p_1 int, …`. A
/// parameter the server named no type for is declared as `sql_variant`, which takes any value.
fn declarations(deduced: &Deduced) -> Option<String> {
    (!deduced.is_empty()).then(|| deduced.iter().enumerate().map(|(i, d)| format!("@p_{} {}", i + 1, d.as_ref().map_or("sql_variant", |d| d.name.as_str()))).collect::<Vec<_>>().join(", "))
}

fn int(v: Option<&Value>) -> i64 {
    v.and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0)
}

fn text(v: Option<&Value>) -> Option<String> {
    v.and_then(as_str).map(String::from)
}

fn described_column(row: &Vec<Value>) -> Described1 {
    Described1 {
        name: text(row.get(2)).unwrap_or_default(),
        nullable: row.get(3).and_then(as_bool).unwrap_or(true),
        ty: Described { name: text(row.get(4)).unwrap_or_default(), max_length: int(row.get(5)), precision: int(row.get(6)) as u32, scale: int(row.get(7)) as u32, collation: text(row.get(8)) },
    }
}

/// The offset of a line that holds `GO` alone, outside text, as tools that run scripts read one.
fn go_line(sql: &str, lexed: &Lexed) -> Option<usize> {
    let mut at = 0;
    for line in sql.split_inclusive('\n') {
        let word = line.trim();
        let start = at + (line.len() - line.trim_start().len());
        at += line.len();
        let in_text = lexed.opaque.iter().any(|&(s, e)| s <= start && start < e);
        if !in_text && word.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("go")) && word.split_whitespace().nth(1).is_none_or(|w| w.bytes().all(|c| c.is_ascii_digit())) {
            return Some(start);
        }
    }
    None
}

/// The tables every statement of the batch reads and writes.
fn all_tables(statements: &[&Statement]) -> Tables {
    let mut out = Tables::default();
    for statement in statements {
        let t = tables(statement);
        for name in t.read {
            if !out.read.contains(&name) {
                out.read.push(name);
            }
        }
        for name in t.write {
            if !out.write.contains(&name) {
                out.write.push(name);
            }
        }
    }
    out
}

/// The kind of statement the tree is.
fn kind(statement: &Statement) -> Option<StatementKind> {
    Some(match statement {
        Statement::Query(_) => StatementKind::Select,
        Statement::Insert(_) => StatementKind::Insert,
        Statement::Update(_) => StatementKind::Update,
        Statement::Delete(_) => StatementKind::Delete,
        Statement::Merge(_) => StatementKind::Merge,
        _ => return None,
    })
}

/// A query that makes no table of its rows (`SELECT … INTO`) writes nothing.
fn read_only(statement: &Statement) -> bool {
    match statement {
        Statement::Query(q) => !selects_into(q),
        // A batch's declarations, its own settings and its messages write nothing.
        Statement::Declare { .. } | Statement::Set(_) | Statement::Print(_) | Statement::Return(_) | Statement::Throw(_) | Statement::RaisError { .. } => true,
        _ => false,
    }
}

fn selects_into(q: &Query) -> bool {
    match q.body.as_ref() {
        SetExpr::Select(s) => s.into.is_some(),
        SetExpr::Query(q) => selects_into(q),
        SetExpr::SetOperation { left, .. } => matches!(left.as_ref(), SetExpr::Select(s) if s.into.is_some()),
        _ => false,
    }
}

/// The select list, or the `OUTPUT` list, the result's columns are written by.
pub(crate) fn result_items(statement: &Statement) -> Option<&[SelectItem]> {
    fn output(o: &Option<OutputClause>) -> Option<&[SelectItem]> {
        match o {
            Some(OutputClause::Output { select_items, into_table: None, .. }) => Some(select_items.as_slice()),
            _ => None,
        }
    }
    match statement {
        Statement::Query(q) => first_select(q).map(|s| s.projection.as_slice()),
        Statement::Insert(i) => output(&i.output),
        Statement::Update(u) => output(&u.output),
        Statement::Delete(d) => output(&d.output),
        Statement::Merge(m) => output(&m.output),
        _ => None,
    }
}

/// The SELECT whose list names a query's columns: the query's own, or its first arm's.
pub(crate) fn first_select(q: &Query) -> Option<&sqlparser::ast::Select> {
    let mut body = q.body.as_ref();
    loop {
        match body {
            SetExpr::Select(s) => return Some(s),
            SetExpr::Query(q) => body = q.body.as_ref(),
            SetExpr::SetOperation { left, .. } => body = left.as_ref(),
            _ => return None,
        }
    }
}

/// How each item of the result list writes its column, for the rules on names
/// ([`crate::analysis::names`]): an alias (`as n`, or T-SQL's `n = …`) or a column read as it is
/// names it; any other expression does not.
fn result_targets(statement: &Statement) -> Option<Vec<ResultTarget>> {
    let items = result_items(statement)?;
    Some(
        items
            .iter()
            .map(|item| match item {
                SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..) => ResultTarget::Star,
                SelectItem::ExprWithAlias { .. } | SelectItem::ExprWithAliases { .. } => ResultTarget::Named,
                SelectItem::UnnamedExpr(Expr::Identifier(_) | Expr::CompoundIdentifier(_)) => ResultTarget::Named,
                _ => ResultTarget::Unnamed(None),
            })
            .collect(),
    )
}

/// The tables the statement reads and writes, each as it writes it, which the freshness facts
/// are read for ([`reylo_engine_core::fresh`]); a WITH query's name is none.
fn tables(statement: &Statement) -> Tables {
    let mut t = Tables::default();
    let mut ctes: Vec<String> = Vec::new();
    match statement {
        Statement::Query(q) => query_tables(q, &mut t.read, &mut ctes),
        Statement::Insert(i) => {
            if let TableObject::TableName(name) = &i.table {
                t.write.push(name.to_string());
            }
            if let Some(source) = &i.source {
                query_tables(source, &mut t.read, &mut ctes);
            }
        }
        Statement::Update(u) => {
            factor_tables(&u.table.relation, &mut t.write, &mut t.read, &mut ctes);
            let mut inner = Vec::new();
            for j in &u.table.joins {
                factor_tables(&j.relation, &mut t.read, &mut inner, &mut ctes);
            }
            t.read.extend(inner);
            if let Some(sqlparser::ast::UpdateTableFromKind::AfterSet(from) | sqlparser::ast::UpdateTableFromKind::BeforeSet(from)) = &u.from {
                twj_tables(from, &mut t.read, &mut ctes);
            }
        }
        Statement::Delete(d) => {
            let from = match &d.from {
                FromTable::WithFromKeyword(f) | FromTable::WithoutKeyword(f) => f,
            };
            if d.tables.is_empty() {
                for twj in from.iter().take(1) {
                    let mut read = Vec::new();
                    factor_tables(&twj.relation, &mut t.write, &mut read, &mut ctes);
                    t.read.extend(read);
                }
            } else {
                t.write.extend(d.tables.iter().map(ObjectName::to_string));
                twj_tables(from, &mut t.read, &mut ctes);
            }
        }
        Statement::Merge(m) => {
            let mut read = Vec::new();
            factor_tables(&m.table, &mut t.write, &mut read, &mut ctes);
            factor_tables(&m.source, &mut t.read, &mut read, &mut ctes);
            t.read.extend(read);
        }
        _ => {}
    }
    t.read.retain(|n| !ctes.iter().any(|c| c.eq_ignore_ascii_case(n)));
    t.read.sort();
    t.read.dedup();
    t.write.sort();
    t.write.dedup();
    t
}

fn query_tables(q: &Query, out: &mut Vec<String>, ctes: &mut Vec<String>) {
    if let Some(with) = &q.with {
        for cte in &with.cte_tables {
            ctes.push(cte.alias.name.value.clone());
            query_tables(&cte.query, out, ctes);
        }
    }
    set_tables(&q.body, out, ctes);
}

fn set_tables(body: &SetExpr, out: &mut Vec<String>, ctes: &mut Vec<String>) {
    match body {
        SetExpr::Select(s) => twj_tables(&s.from, out, ctes),
        SetExpr::Query(q) => query_tables(q, out, ctes),
        SetExpr::SetOperation { left, right, .. } => {
            set_tables(left, out, ctes);
            set_tables(right, out, ctes);
        }
        _ => {}
    }
}

fn twj_tables(from: &[TableWithJoins], out: &mut Vec<String>, ctes: &mut Vec<String>) {
    for twj in from {
        let mut more = Vec::new();
        factor_tables(&twj.relation, out, &mut more, ctes);
        for j in &twj.joins {
            factor_tables(&j.relation, out, &mut more, ctes);
        }
        out.extend(more);
    }
}

/// A table factor's table into `named`, and what a derived table inside it reads into `inner`.
fn factor_tables(f: &TableFactor, named: &mut Vec<String>, inner: &mut Vec<String>, ctes: &mut Vec<String>) {
    match f {
        TableFactor::Table { name, args: None, .. } => named.push(name.to_string()),
        TableFactor::Derived { subquery, .. } => query_tables(subquery, inner, ctes),
        TableFactor::NestedJoin { table_with_joins, .. } => twj_tables(std::slice::from_ref(table_with_joins.as_ref()), inner, ctes),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::MsSqlDialect;
    use sqlparser::parser::Parser;
    use reylo_engine_core::api::Answers;
    use reylo_engine_core::host::Host as CoreHost;

    /// The analysis of `sql` with the answers `answer` gives each need, by its query and params.
    fn analyzed(sql: &str, answer: &dyn Fn(&str, &[Value]) -> Result<Vec<Vec<Value>>, crate::api::DbError>) -> Analysis {
        let mut answers: Answers<crate::api::NoDescribe> = Answers::new();
        for _ in 0..4 {
            let host = CoreHost::new(&answers);
            if let Some(a) = analyze(sql, &host) {
                return a;
            }
            for need in host.needs() {
                if let crate::api::Need::Query { id, sql, params, .. } = need {
                    let a = match answer(&sql, &params) {
                        Ok(rows) => crate::api::Answer::Rows { rows },
                        Err(error) => crate::api::Answer::Error { error },
                    };
                    answers.insert(id, a);
                }
            }
        }
        panic!("no analysis after four rounds");
    }

    fn column(name: &str, nullable: bool, ty: &str, max_length: i64, precision: u32, scale: u32) -> Vec<Value> {
        vec![json!(false), json!(1), json!(name), json!(nullable), json!(ty), json!(max_length), json!(precision), json!(scale), Value::Null, Value::Null, Value::Null, Value::Null, json!(false), Value::Null, Value::Null]
    }

    #[test]
    fn types_a_select_from_the_server_s_describe() {
        let a = analyzed("select id, name from users where id = :id", &|sql, params| {
            if sql == UNDECLARED {
                assert_eq!(params, [json!("select id, name from users where id = @p_1")]);
                return Ok(vec![vec![json!(1), json!("@p_1"), json!(56), json!("int"), json!(4), json!(10), json!(0)]]);
            }
            if sql == FIRST_RESULT_SET {
                assert_eq!(params[1], json!("@p_1 int"));
                return Ok(vec![column("id", false, "int", 4, 10, 0), column("name", true, "nvarchar(50)", 100, 0, 0)]);
            }
            Ok(vec![])
        });
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert_eq!(a.rewritten.sql, "select id, name from users where id = @p_1");
        assert_eq!(a.params[0].ty.label.as_deref(), Some("int"));
        assert_eq!((a.columns[0].nullable, a.columns[1].nullable), (false, true));
        assert_eq!(a.kind, Some(StatementKind::Select));
        assert!(a.read_only);
    }

    #[test]
    fn a_parameter_the_server_cannot_type_asks_for_a_cast() {
        let a = analyzed("select :p as p", &|sql, _| {
            if sql == DATABASE_COLLATION {
                return Ok(vec![vec![json!("SQL_Latin1_General_CP1_CI_AS")]]);
            }
            assert_eq!(sql, UNDECLARED);
            Err(crate::api::DbError { message: "The parameter type for '@p_1' cannot be uniquely deduced; two possibilities are 'sql_variant' and 'xml'.".into(), code: Some("11506".into()), position: None })
        });
        assert_eq!(a.diagnostics.len(), 1);
        assert_eq!((a.diagnostics[0].start, a.diagnostics[0].length), (7, 2));
        assert!(a.diagnostics[0].message.starts_with("SQL Server cannot tell the type of :p here (The parameter type for ':p' cannot be uniquely deduced"), "{}", a.diagnostics[0].message);
    }

    #[test]
    fn a_batch_returns_one_result_set_and_has_no_go_line() {
        let none = |_: &str, _: &[Value]| -> Result<Vec<Vec<Value>>, crate::api::DbError> { panic!("nothing is asked") };
        assert!(analyzed("select 1 as a select 2 as b", &none).diagnostics[0].message.starts_with("Reylo types a batch's first result set, and a typed .sql file returns one"));
        assert!(analyzed("select 1 as a; if @f = 1 select 2 as b", &none).diagnostics[0].message.ends_with("This one may return two or more."));
        let go = analyzed("create table t (a int)\ngo\nselect a from t", &none);
        assert_eq!((go.diagnostics[0].start, go.diagnostics[0].message.as_str()), (23, GO));
        assert_eq!(go_line("select 'a\ngo\n' as x", &lex::lex("select 'a\ngo\n' as x", Dialect::Sqlserver).unwrap()), None);
    }

    #[test]
    fn a_result_after_a_use_keeps_the_server_s_types() {
        // After a `use`, the batch reads another database's tables, so nothing is asked of the
        // connection's own: no probe of the columns, no key and no written table's columns.
        let asked = std::cell::RefCell::new(Vec::new());
        let a = analyzed("use otherdb; insert into dbo.log (id) values (@id); select n + 1 as n from dbo.t where id = @id2", &|sql, _| {
            asked.borrow_mut().push(sql.to_string());
            if sql == UNDECLARED {
                return Ok(vec![vec![json!(1), json!("@p_1"), json!(56), json!("int"), json!(4), json!(10), json!(0)], vec![json!(2), json!("@p_2"), json!(56), json!("int"), json!(4), json!(10), json!(0)]]);
            }
            if sql == FIRST_RESULT_SET {
                return Ok(vec![column("n", true, "int", 4, 10, 0)]);
            }
            Ok(vec![])
        });
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert!(a.columns[0].nullable);
        assert_eq!(a.cardinality, Cardinality::Many);
        let asked = asked.borrow();
        assert_eq!(asked.iter().filter(|q| *q == FIRST_RESULT_SET).count(), 1, "{asked:?}");
        assert!(!asked.iter().any(|q| q == crate::cardinality::UNIQUE_KEYS || q == crate::params::TABLE_COLUMNS), "{asked:?}");
    }

    #[test]
    fn the_tables_a_statement_reads_and_writes() {
        let parse = |sql: &str| Parser::parse_sql(&MsSqlDialect {}, sql).unwrap().pop().unwrap();
        let t = tables(&parse("with c as (select id from a) select x from c join dbo.b on b.id = c.id left join (select id from d) e on e.id = b.id"));
        assert_eq!((t.read, t.write), (vec!["a".to_string(), "d".into(), "dbo.b".into()], vec![]));
        let t = tables(&parse("insert into t (a) output inserted.id select a from s"));
        assert_eq!((t.read, t.write), (vec!["s".to_string()], vec!["t".to_string()]));
    }
}
