//! The MySQL analysis. The server's prepare metadata is the type source: every result column's
//! type and NOT NULL flag (the server's own analysis, expressions included) and, since 8.0.22, a
//! type per `?` from its context. The engine takes those as they are (see
//! wiki/mysql-sources.md). MySQL has no RETURNING and no server surface for the row count, so a
//! data-modifying statement returns no rows, and a SELECT's row count is the unique-key proof
//! over a `sqlparser` tree with the keys read from `information_schema.statistics`.
//!
//! Two things stand between that tree and the statement the server runs. MySQL reads some text by
//! the session's `sql_mode` (`||`, a double-quoted word, a backslash in a string, a space before a
//! call's parenthesis), so where the statement has such text the engine asks for the mode. The
//! placeholders and the tree are read as the session reads its strings and double quotes
//! ([`analyze_text`], [`SessionDialect`]), and where the rest reads otherwise than the parser, the
//! engine reads nothing off its own tree ([`misread`]). And an execution
//! answers with column types of its own, which are the prepare's except where a bound value or
//! the plan decides them: such a column is typed as either ([`settle`]).

use crate::api::{Analysis, Column, Diagnostic, MysqlDescribe, Param, Scope};
use crate::shape::Shape;
use crate::dialect::Dialect;
use crate::host::{as_str, Host, Lazy};
use crate::lex::{self, Lexed, MysqlText, Style, Target};
use crate::types::{Field, Type};
use serde_json::{json, Value};
use sqlparser::ast::{self, Expr, FunctionArg, FunctionArgExpr, GroupByExpr, LimitClause, Query, Select, SelectItem, SetExpr, Statement, TableFactor};
use sqlparser::dialect::MySqlDialect;
use sqlparser::parser::Parser;
use std::collections::{HashMap, HashSet};

/// The session's `sql_mode`, asked only for a statement whose reading turns on it.
const SQL_MODE_QUERY: &str = "select @@session.sql_mode";

/// Every view the session sees, in any schema. Asked once for a session, by a SELECT, which reads
/// each one materialized ([`crate::mysql_derived`]), and by a statement that returns an integer
/// column of a table or a view ([`settle`]).
const VIEWS_QUERY: &str = "select table_name from information_schema.views";

/// One row per column of every unique index of the named tables, in key order, with the column's
/// type. `?` is the comma-joined table names, because MySQL binds no arrays. A key part over an
/// expression (MySQL 8.0.13's functional indexes) has no column name.
const UNIQUE_KEYS_QUERY: &str = "select s.table_name, s.index_name, s.column_name, c.data_type from information_schema.statistics s \
    left join information_schema.columns c on c.table_schema = s.table_schema and c.table_name = s.table_name and c.column_name = s.column_name \
    where s.table_schema = database() and s.non_unique = 0 and find_in_set(s.table_name, ?) order by s.table_name, s.index_name, s.seq_in_index";

/// One row per column of the named tables and views of the current schema, in order, with its
/// type. `?` is the comma-joined table names.
const KEY_COLUMNS_QUERY: &str = "select c.table_name, c.column_name, c.data_type from information_schema.columns c \
    where c.table_schema = database() and find_in_set(c.table_name, ?) order by c.table_name, c.ordinal_position";

/// One row per view among the named tables of the current schema: its name, its defining SELECT
/// as the server stores it (every name qualified with the schema), and the schema. `?` is the
/// comma-joined names.
const VIEW_DEFINITIONS_QUERY: &str = "select v.table_name, v.view_definition, v.table_schema from information_schema.views v \
    where v.table_schema = database() and find_in_set(v.table_name, ?)";

/// One row per column of the named tables and views of the current schema. `?` is the
/// comma-joined table names.
const TABLE_COLUMNS_QUERY: &str = "select c.table_name, c.column_name from information_schema.columns c \
    where c.table_schema = database() and find_in_set(c.table_name, ?) order by c.table_name, c.ordinal_position";

/// One row per enforced CHECK of the named tables of the current database: the table, and the
/// CHECK as `information_schema` prints it ([`check_clause`]). `?` is the comma-joined table
/// names. A server before 8.0.16 has neither table, and its error is no CHECK.
const CHECKS_QUERY: &str = "select tc.table_name, cc.check_clause from information_schema.table_constraints tc \
    join information_schema.check_constraints cc on cc.constraint_schema = tc.constraint_schema and cc.constraint_name = tc.constraint_name \
    where tc.table_schema = database() and tc.constraint_type = 'CHECK' and tc.enforced = 'YES' and find_in_set(tc.table_name, ?) \
    order by tc.table_name, tc.constraint_name";

/// The statements the MySQL analysis reads; any other is a command ([`lex::Statement`]).
const KINDS: &[&str] = &["select", "with", "table", "values", "insert", "replace", "update", "delete"];

/// The commands that may return rows. Where the prepare describes no columns for one, it still
/// returns them when it runs: an EXPLAIN prepares with none and returns twelve.
const MAY_RETURN_ROWS: &[&str] = &["analyze", "cache", "check", "checksum", "desc", "describe", "execute", "explain", "handler", "help", "load", "optimize", "repair", "show"];

/// The analysis of a statement as the session reads its text. A backslash in a string escapes the
/// character after it unless the session's `sql_mode` has NO_BACKSLASH_ESCAPES, and `"` quotes a
/// string unless it has ANSI_QUOTES. Where the statement has either (`'C:\' and id = :id` has no
/// placeholder by default and one without escapes; `"email"` is a string or a column), the mode
/// decides, asked of the session, and both the placeholders and the parser's tree are read by it.
pub fn analyze_text(sql: &str, host: &Host) -> Lazy<Analysis> {
    crate::structured::analyze(sql, Dialect::Mysql, host, &|sql, marked| analyze_text_plain(sql, host, marked), &|diagnostics| crate::analysis::bare_of(sql, Dialect::Mysql, diagnostics))
}

/// The analysis of a statement without structured parameters; `marked` names the parameters cast
/// to `nullable(…)`.
fn analyze_text_plain(sql: &str, host: &Host, marked: &[String]) -> Lazy<Analysis> {
    let read = |text| lex::lex_mysql(sql, lex::Named::Dialect, text);
    let first = read(MysqlText::default());
    let alike = MysqlText::ALL.iter().all(|&text| read(text) == first);
    let quoted = first.as_ref().is_ok_and(|l| {
        l.opaque.iter().any(|&(start, end)| {
            let span = &sql[start..end];
            span.starts_with('"') || (span.starts_with('\'') && span.contains('\\'))
        })
    });
    let text = if alike && !quoted {
        Some(MysqlText::default())
    } else {
        let rows = host.query(SQL_MODE_QUERY, Vec::new(), Scope::Session)?;
        rows.ok().and_then(|rows| rows.first().and_then(|row| row.first()).and_then(as_str).map(MysqlText::of_mode))
    };
    let lexed = match text {
        Some(text) => read(text),
        None if alike => first,
        None => {
            let message = "the server did not say its sql_mode, which decides where this statement's strings end and so which of its placeholders are text".to_string();
            return Some(crate::analysis::bare(sql, &Lexed { params: vec![], placeholders: vec![], style: Style::None, opaque: vec![] }, Dialect::Mysql, vec![Diagnostic { message, start: 0, length: sql.trim_end().len().max(1), code: 1 }]));
        }
    };
    let lexed = match lexed {
        Ok(lexed) => lexed,
        Err(e) => return Some(crate::analysis::bare(sql, &Lexed { params: vec![], placeholders: vec![], style: Style::None, opaque: vec![] }, Dialect::Mysql, vec![Diagnostic { message: e.message, start: e.offset, length: 1, code: 1 }])),
    };
    if lexed.style == Style::Mixed {
        let first = lexed.placeholders.iter().find(|p| lex::is_positional(&p.name)).expect("a mixed statement has a positional placeholder");
        let d = Diagnostic { message: "Named and positional parameters cannot be mixed in one statement.".into(), start: first.start, length: (first.end - first.start).max(1), code: 2 };
        return Some(crate::analysis::bare(sql, &lexed, Dialect::Mysql, vec![d]));
    }
    let repeated = crate::analysis::repeated(sql, &lexed);
    if !repeated.is_empty() {
        return Some(crate::analysis::bare(sql, &lexed, Dialect::Mysql, repeated));
    }
    analyze(sql, &lexed, text, host, marked)
}

/// `sqlparser`'s MySQL dialect, reading strings and double quotes as a session of this reading
/// does: a backslash escapes only where it does there, and `"` quotes a name under ANSI_QUOTES.
/// Everything else is `MySqlDialect`'s, which it also says it is, so the parser's own checks for
/// MySQL hold for it.
#[derive(Debug)]
struct SessionDialect(MysqlText);

impl sqlparser::dialect::Dialect for SessionDialect {
    fn dialect(&self) -> std::any::TypeId {
        std::any::TypeId::of::<MySqlDialect>()
    }
    fn is_delimited_identifier_start(&self, ch: char) -> bool {
        ch == '`' || (ch == '"' && self.0.ansi_quotes)
    }
    fn supports_string_literal_backslash_escape(&self) -> bool {
        self.0.backslash_escapes
    }
    // The rest is `MySqlDialect`'s (sqlparser 0.63): every method it overrides, delegated.
    fn is_identifier_start(&self, ch: char) -> bool {
        MySqlDialect {}.is_identifier_start(ch)
    }
    fn is_identifier_part(&self, ch: char) -> bool {
        MySqlDialect {}.is_identifier_part(ch)
    }
    fn identifier_quote_style(&self, identifier: &str) -> Option<char> {
        MySqlDialect {}.identifier_quote_style(identifier)
    }
    fn supports_string_literal_concatenation(&self) -> bool {
        MySqlDialect {}.supports_string_literal_concatenation()
    }
    fn ignores_wildcard_escapes(&self) -> bool {
        MySqlDialect {}.ignores_wildcard_escapes()
    }
    fn supports_numeric_prefix(&self) -> bool {
        MySqlDialect {}.supports_numeric_prefix()
    }
    fn supports_bitwise_shift_operators(&self) -> bool {
        MySqlDialect {}.supports_bitwise_shift_operators()
    }
    fn supports_multiline_comment_hints(&self) -> bool {
        MySqlDialect {}.supports_multiline_comment_hints()
    }
    fn parse_infix(&self, parser: &mut Parser, expr: &Expr, precedence: u8) -> Option<Result<Expr, sqlparser::parser::ParserError>> {
        MySqlDialect {}.parse_infix(parser, expr, precedence)
    }
    fn parse_statement(&self, parser: &mut Parser) -> Option<Result<Statement, sqlparser::parser::ParserError>> {
        MySqlDialect {}.parse_statement(parser)
    }
    fn require_interval_qualifier(&self) -> bool {
        MySqlDialect {}.require_interval_qualifier()
    }
    fn supports_limit_comma(&self) -> bool {
        MySqlDialect {}.supports_limit_comma()
    }
    fn supports_create_table_select(&self) -> bool {
        MySqlDialect {}.supports_create_table_select()
    }
    fn supports_insert_set(&self) -> bool {
        MySqlDialect {}.supports_insert_set()
    }
    fn supports_user_host_grantee(&self) -> bool {
        MySqlDialect {}.supports_user_host_grantee()
    }
    fn is_table_factor_alias(&self, explicit: bool, kw: &sqlparser::keywords::Keyword, parser: &mut Parser) -> bool {
        MySqlDialect {}.is_table_factor_alias(explicit, kw, parser)
    }
    fn supports_table_hints(&self) -> bool {
        MySqlDialect {}.supports_table_hints()
    }
    fn requires_single_line_comment_whitespace(&self) -> bool {
        MySqlDialect {}.requires_single_line_comment_whitespace()
    }
    fn supports_match_against(&self) -> bool {
        MySqlDialect {}.supports_match_against()
    }
    fn supports_select_modifiers(&self) -> bool {
        MySqlDialect {}.supports_select_modifiers()
    }
    fn supports_set_names(&self) -> bool {
        MySqlDialect {}.supports_set_names()
    }
    fn supports_comma_separated_set_assignments(&self) -> bool {
        MySqlDialect {}.supports_comma_separated_set_assignments()
    }
    fn supports_update_order_by(&self) -> bool {
        MySqlDialect {}.supports_update_order_by()
    }
    fn supports_data_type_signed_suffix(&self) -> bool {
        MySqlDialect {}.supports_data_type_signed_suffix()
    }
    fn supports_cross_join_constraint(&self) -> bool {
        MySqlDialect {}.supports_cross_join_constraint()
    }
    fn supports_double_ampersand_operator(&self) -> bool {
        MySqlDialect {}.supports_double_ampersand_operator()
    }
    fn supports_binary_kw_as_cast(&self) -> bool {
        MySqlDialect {}.supports_binary_kw_as_cast()
    }
    fn supports_comment_optimizer_hint(&self) -> bool {
        MySqlDialect {}.supports_comment_optimizer_hint()
    }
    fn supports_constraint_keyword_without_name(&self) -> bool {
        MySqlDialect {}.supports_constraint_keyword_without_name()
    }
    fn supports_key_column_option(&self) -> bool {
        MySqlDialect {}.supports_key_column_option()
    }
    fn supports_group_by_with_modifier(&self) -> bool {
        MySqlDialect {}.supports_group_by_with_modifier()
    }
    fn supports_left_associative_joins_without_parens(&self) -> bool {
        MySqlDialect {}.supports_left_associative_joins_without_parens()
    }
}

/// `text` is how the session reads its strings, `None` where it would not say and the statement
/// turns on it; `marked` names the parameters cast to `nullable(…)`.
pub fn analyze(sql: &str, lexed: &Lexed, text: Option<MysqlText>, host: &Host, marked: &[String]) -> Lazy<Analysis> {
    let dialect = SessionDialect(text.unwrap_or_default());
    // What the engine prints of the statement, for the probes it describes, writes its strings
    // as the session reads them back.
    sqlparser::ast::set_backslash_escapes(dialect.0.backslash_escapes);
    let statement = match lex::one_statement(sql, lexed, KINDS) {
        Ok(statement) => statement,
        Err(message) => return Some(crate::analysis::bare(sql, lexed, Dialect::Mysql, vec![Diagnostic { message, start: 0, length: sql.trim_end().len().max(1), code: 10 }])),
    };
    // A CALL prepares with no columns and returns whatever result sets the procedure makes when
    // it runs, which a command typed from its prepare would drop.
    if statement == lex::Statement::Command("call".into()) {
        let message = "A CALL returns the result sets its procedure makes, which MySQL does not describe before the procedure runs, so its rows cannot be typed: select from a function or a view instead".to_string();
        return Some(crate::analysis::bare(sql, lexed, Dialect::Mysql, vec![Diagnostic { message, start: 0, length: sql.trim_end().len().max(1), code: 10 }]));
    }
    // The server runs the text of a `/*! … */` comment where its version allows, and the lexer
    // took it for a comment: what the statement is, and whether it is one of the typed kinds, is
    // then not what was checked above.
    if let Some(&(start, end)) = lexed.opaque.iter().find(|&&(start, end)| sql[start..end].starts_with("/*!")) {
        let message = "MySQL runs the text of a /*! … */ comment as SQL where the server's version allows it, so the statement is not the one written outside it: write it as plain SQL".to_string();
        return Some(crate::analysis::bare(sql, lexed, Dialect::Mysql, vec![Diagnostic { message, start, length: end - start, code: 10 }]));
    }
    let rewritten = lex::rewrite(sql, lexed, Target::Question);
    // Text MySQL would make in the connection's collation takes the database's, asked once a
    // session (`mysql_collation`), before the prepare checks the statement as it will run.
    let sites = crate::mysql_collation::sites(sql, &lexed.opaque);
    let rewritten = if sites.is_empty() {
        rewritten
    } else {
        let server = match host.query(crate::structured::MYSQL_SERVER_QUERY, Vec::new(), Scope::Session)? {
            Ok(rows) => crate::structured::Server::of(&rows),
            Err(_) => crate::structured::Server::default(),
        };
        match &server.collation {
            Some((charset, collation)) if !server.mariadb && charset.eq_ignore_ascii_case("utf8mb4") => crate::mysql_collation::written(rewritten, &sites, collation),
            _ => rewritten,
        }
    };
    // Asked in one round: the prepare, the mode where it decides the reading, and the columns of
    // the tables a parameter is written to.
    let raw = host.describe(&rewritten.sql);
    let misread = match text {
        Some(_) => misread(sql, lexed, host),
        None => Some(Some("the server did not say its sql_mode, which decides how it reads this statement's strings and double quotes, so nothing bounds its rows".into())),
    };
    // A tree the server would read otherwise still says what kind of statement it is, and nothing more.
    let read = parse(&dialect, &rewritten.sql).ok().and_then(|mut v| if v.len() == 1 { v.pop() } else { None });
    let names = crate::mysql_params::names(&rewritten);
    if let Some(statement) = &read {
        crate::mysql_params::prefetch(statement, &names, host);
        crate::errors::prefetch(kind(Some(statement)), &crate::mysql_params::tables(statement), &rewritten.sql, host);
    }
    let (raw, misread) = (raw?, misread?);
    let raw = match raw {
        Ok(r) => r,
        Err(e) => {
            let message = match &e.code {
                Some(code) => format!("{} ({code})", e.message),
                None => e.message.clone(),
            };
            let message = if matches!(statement, lex::Statement::Command(_)) { lex::refused_command(&message) } else { message };
            return Some(crate::analysis::bare(sql, lexed, Dialect::Mysql, vec![Diagnostic { message, start: 0, length: sql.trim_end().len().max(1), code: 6 }]));
        }
    };
    // Where the engine's lexer and the server part text from code differently, a placeholder of
    // the one is text to the other: the statement would be typed with a parameter the server
    // does not take, and fail when it runs.
    if raw.params.len() != rewritten.order.len() {
        let message = format!(
            "the server finds {} placeholder(s) in this statement and the engine {}, so the engine does not read its strings and comments as the server does",
            raw.params.len(),
            rewritten.order.len()
        );
        return Some(crate::analysis::bare(sql, lexed, Dialect::Mysql, vec![Diagnostic { message, start: 0, length: sql.trim_end().len().max(1), code: 1 }]));
    }
    // A command (DDL, a session setting, SHOW, EXPLAIN) has no tree to read: the prepare says what
    // it takes and returns, and nothing says where a column comes from, so each may be NULL. One
    // that returns no rows runs as a write does.
    if let lex::Statement::Command(kind) = statement {
        if raw.columns.is_empty() && MAY_RETURN_ROWS.contains(&kind.as_str()) {
            let message = format!("MySQL does not describe the rows of {} before it runs, so they cannot be typed", kind.to_ascii_uppercase());
            return Some(crate::analysis::bare(sql, lexed, Dialect::Mysql, vec![Diagnostic { message, start: 0, length: sql.trim_end().len().max(1), code: 10 }]));
        }
        let columns: Vec<Column> = raw
            .columns
            .iter()
            .map(|c| {
                let (ty, name) = column_type(c.r#type, c.charset);
                Column { name: c.name.clone(), ty, nullable: true, type_name: name.to_string(), comment: None }
            })
            .collect();
        let params = lexed
            .params
            .iter()
            .map(|name| {
                let (ty, type_name) = match rewritten.order.iter().position(|n| n == name).and_then(|i| raw.params.get(i)) {
                    Some(p) => param_type(p.r#type, p.charset),
                    None => (Type::Unknown, ""),
                };
                Param { name: name.clone(), ty, type_name: type_name.to_string(), nullable: false, schema: None, cast: None, comment: None }
            })
            .collect();
        let cardinality = if columns.is_empty() { "none" } else { "many" };
        return Some(Analysis {
            kind,
            params,
            columns,
            cardinality: cardinality.into(),
            cardinality_reason: None,
            rewritten: vectors_as_text(rewritten, &raw),
            style: lexed.style,
            diagnostics: Vec::new(),
            unresolved: Vec::new(),
            structured: Vec::new(),
            fresh: None,
            errors: None,
            tables: None,
        });
    }
    let parsed = if misread.is_some() { None } else { read.clone() };
    // A parameter as a whole sort or group key is an error; the rest is typed all the same.
    let mut diagnostics = parsed.as_ref().map(|statement| constant_keys(sql, statement, &rewritten)).unwrap_or_default();
    // The parameters the caller may pass null for, which the prepare does not say: each one cast
    // to `nullable(…)`, and the rest by the rules of Postgres and SQLite, read off the tree
    // (`mysql_params`).
    let nullable = match &parsed {
        Some(statement) => crate::mysql_params::nullable(statement, &names, host, marked)?,
        None => crate::mysql_params::Nullable { params: marked.iter().cloned().collect(), refused: Vec::new() },
    };
    // A parameter cast to `nullable(…)` and written whole to a column that refuses null would be
    // refused at run time for the null its type allows: an error at the parameter.
    for (name, table, column) in &nullable.refused {
        let Some(at) = lexed.placeholders.iter().find(|p| &p.name == name) else { continue };
        let written = &sql[at.start..at.end];
        let message = format!("The parameter {written} is cast to nullable(…), but it is written to {table}.{column}, which refuses null: cast it to its type alone.");
        diagnostics.push(Diagnostic { message, start: at.start, length: at.end - at.start, code: crate::structured::CODE });
    }
    let nullable = nullable.params;
    // A parameter's type from the column it is written to or compared with, where the prepare's
    // says less, and the comments of those columns and of the ones the result columns read
    // (`mysql_params::typed`).
    let typed = match &parsed {
        Some(statement) => crate::mysql_params::typed(statement, &names, host)?,
        None => Default::default(),
    };
    // A result column's comment is its origin's, where the prepare names the column itself.
    let traced = parsed.as_ref().and_then(crate::mysql_params::traced_from);
    let comment_of = |c: &crate::api::MysqlColumn| -> Option<String> {
        let (table, column) = (c.org_table.as_deref()?, c.org_name.as_deref()?);
        if column.is_empty() || !traced.as_ref()?.iter().any(|t| t.eq_ignore_ascii_case(table)) {
            return None;
        }
        typed.columns.get(&(table.to_ascii_lowercase(), column.to_ascii_lowercase())).cloned()
    };
    // What the engine knows of each column being NULL: the server's flags, read where merging
    // could make them wrong with every derived table materialized, and what the engine proves
    // beyond them.
    let proven = match &parsed {
        Some(Statement::Query(q)) => {
            // The views of every schema, which the copy reads through derived tables of their
            // own; asked once for a session.
            let views: HashSet<String> = host.query(VIEWS_QUERY, Vec::new(), Scope::Session)?.ok().map(|rows| rows.iter().filter_map(|row| row.first().and_then(as_str)).map(str::to_ascii_lowercase).collect()).unwrap_or_default();
            prove(q, Some(&raw), &views, host, &dialect, 0)?
        }
        _ => Proven { flags: raw.columns.iter().map(|c| c.flags & 1 != 0).collect(), ..Proven::default() },
    };
    let never_null = proven.never_null(&raw.columns);
    // A column of a view may be an expression, which the prepare does not tell from a table's.
    let views: Option<HashSet<String>> = match raw.columns.iter().any(|c| (other_width(c).is_some() || flag_width(c)) && c.org_table.as_ref().is_some_and(|t| !t.is_empty())) {
        true => host.query(VIEWS_QUERY, Vec::new(), Scope::Session)?.ok().map(|rows| rows.iter().filter_map(|row| row.first().and_then(as_str)).map(str::to_ascii_lowercase).collect()),
        false => Some(HashSet::new()),
    };
    let settled = settle(sql, lexed, &rewritten, parsed.as_ref(), &raw, views.as_ref());
    // A JSON column a CHECK holds to a JSON Schema: the values the schema lets in.
    let documents = documents(parsed.as_ref(), &raw, host)?;
    // The truth values: what the statement computes as 1 or 0, by the engine's reading of it, and
    // a table's TINYINT(1) or BIT(1) column, by the prepare's.
    let truths = match &parsed {
        Some(Statement::Query(q)) => crate::mysql_shape::truths(q, raw.columns.len()),
        _ => Vec::new(),
    };
    let columns = raw
        .columns
        .iter()
        .enumerate()
        .map(|(position, c)| {
            let (ty, name) = column_type(c.r#type, c.charset);
            let ty = settled.get(position).cloned().flatten().unwrap_or(ty);
            // A computed one is named `boolean`, which the oracle holds to 0 and 1; a column keeps
            // its type's name, since it holds whatever was stored in it and the run time refuses
            // any other value.
            let computed = truths.get(position).copied().unwrap_or(false);
            let truth = computed || flag_column(c, views.as_ref());
            let ty = if truth && !matches!(ty, Type::Json { .. }) { Type::Scalar { name: "bool".into() } } else { ty };
            let name = if computed { "boolean" } else { name };
            let (not_null, shape) = never_null[position];
            match shape.map(|i| &proven.shaped[i]) {
                // A JSON column takes the shape's type; any other keeps the server's.
                Some((_, shaped_ty, _)) => {
                    let ty = if c.r#type == 245 { shaped_ty.clone() } else { ty };
                    Column { name: c.name.clone(), ty, nullable: !not_null, type_name: name.to_string(), comment: comment_of(c) }
                }
                None => {
                    let ty = match (ty, documents.get(position).cloned().flatten()) {
                        (Type::Json { schema: None }, Some(schema)) => Type::Json { schema: Some(schema) },
                        (ty, _) => ty,
                    };
                    Column { name: c.name.clone(), ty, nullable: !not_null, type_name: name.to_string(), comment: comment_of(c) }
                }
            }
        })
        .collect();
    // The first `?` a name binds carries the server's type for it.
    let params = lexed
        .params
        .iter()
        .map(|name| {
            let index = rewritten.order.iter().position(|n| n == name);
            let (ty, type_name) = match index.and_then(|i| raw.params.get(i)) {
                Some(p) => param_type(p.r#type, p.charset),
                None => (Type::Unknown, ""),
            };
            // Only an integer or a BIT context is narrowed: those are the ones the prepare describes alike for every column.
            let ty = match (&ty, typed.types.get(name)) {
                (Type::Scalar { name: server }, Some(column)) if server == "int4" || server == "bool|bytea" => Type::Scalar { name: (*column).to_string() },
                _ => ty,
            };
            Param { name: name.clone(), ty, type_name: type_name.to_string(), nullable: nullable.contains(name), schema: None, cast: None, comment: typed.comments.get(name).cloned() }
        })
        .collect();
    let kind = kind(read.as_ref());
    let (cardinality, reason) = match &parsed {
        // Its rows go into variables or a file: it returns none.
        Some(Statement::Query(q)) if assigns(q) => ("none".to_string(), None),
        Some(Statement::Query(q)) => {
            let keys = catalog(host, q)?;
            cardinality(q, &keys)
        }
        Some(Statement::Insert(_)) | Some(Statement::Update(_)) | Some(Statement::Delete(_)) => ("none".to_string(), None),
        Some(_) => ("many".to_string(), Some("only a SELECT is proven".into())),
        // MySQL has no RETURNING: a write returns no rows however the server reads it.
        None if matches!(read, Some(Statement::Insert(_) | Statement::Update(_) | Statement::Delete(_))) => ("none".to_string(), None),
        // A prepare with no columns is a statement with no result set: a write the engine's
        // parser does not read (`delete t1, t2 from …`, `update t1, t2 set …`) or a SELECT whose
        // rows go into variables or a file (`select … into @n`), and it returns no rows. Not a
        // `CALL`, which prepares with no columns and returns whatever result sets the procedure
        // makes: only a statement that opens with a write or a SELECT.
        None if raw.columns.is_empty() && opens_without_result_sets(&rewritten.sql) => ("none".to_string(), None),
        None => ("many".to_string(), Some(misread.unwrap_or_else(|| "the engine could not parse the statement, so nothing bounds its rows".into()))),
    };
    let rewritten = vectors_as_text(rewritten, &raw);
    let mut analysis = Analysis {
        kind: kind.to_string(),
        params,
        columns,
        cardinality,
        cardinality_reason: reason,
        rewritten,
        style: lexed.style,
        diagnostics,
        unresolved: Vec::new(),
        structured: Vec::new(),
        fresh: None,
        errors: None,
        // From the tree as the session reads the statement, also where it reads some part of it
        // otherwise than the parser (`||`, a spaced `count (`): which tables a FROM names does not
        // turn on that.
        tables: read.as_ref().map(crate::mysql_params::tables),
    };
    crate::analysis::clashes(sql, &mut analysis, &|_| None);
    Some(analysis)
}

/// How deep in derived tables the engine reads a derived table's columns: each level is a round
/// of prepares after the one inside it.
const MAX_DERIVED_DEPTH: usize = 3;

/// How many derived tables and WITH queries of one query the engine reads the columns of; past
/// it, none.
const MAX_DERIVED: usize = 8;

/// What the engine knows of a query's result columns being NULL ([`prove`]).
#[derive(Debug, Default)]
struct Proven {
    /// The prepare the flags are read from.
    columns: Vec<crate::api::MysqlColumn>,
    /// Each column's NOT NULL flag, by position.
    flags: Vec<bool>,
    /// The columns the engine proves never NULL beyond the flags, by position.
    not_null: Vec<usize>,
    /// The JSON shapes, by result column name: the type, and whether never NULL.
    shaped: Vec<(String, Type, bool)>,
}

impl Proven {
    /// Per column: whether it is never NULL, and its shape, the first of its name that no column
    /// before it took. A constructor never returns SQL NULL, whatever the server's flag says; an
    /// aggregate over no rows does, unless guarded.
    fn never_null(&self, columns: &[crate::api::MysqlColumn]) -> Vec<(bool, Option<usize>)> {
        let mut used = vec![false; self.shaped.len()];
        columns
            .iter()
            .enumerate()
            .map(|(position, c)| {
                let not_null = self.flags.get(position).copied().unwrap_or(false) || self.not_null.contains(&position);
                match self.shaped.iter().enumerate().position(|(i, (name, _, _))| name == &c.name && !used[i]) {
                    Some(i) => {
                        used[i] = true;
                        (not_null || self.shaped[i].2, Some(i))
                    }
                    None => (not_null, None),
                }
            })
            .collect()
    }
}

/// What the engine knows of a query's result columns being NULL: the server's flags, from `raw`
/// (or its own prepare where there is none) or, where merging could make them wrong, from the
/// copy that materializes what the server would merge ([`crate::mysql_derived`]); and what it
/// proves beyond them: what the WHERE proves (`mysql_nulls`), the JSON shapes and the scalar
/// subqueries (`mysql_shape`), and its derived tables' and WITH queries' own columns
/// ([`through_derived`]). `depth` is how many derived tables the query is inside.
fn prove(q: &Query, raw: Option<&MysqlDescribe>, views: &HashSet<String>, host: &Host, dialect: &SessionDialect, depth: usize) -> Lazy<Proven> {
    let mut pending = false;
    let mut materialized = crate::mysql_derived::materialized(q, views);
    let mut outcome = materialized.as_ref().map(|m| host.describe(&m.to_string()));
    // Reading a view through a derived table can break the copy, as where a column is named
    // through the view's schema (`app.v.x`): the copy without that still materializes the derived
    // tables and WITH queries.
    if matches!(outcome, Some(Some(Err(_)))) && !views.is_empty() {
        let without = crate::mysql_derived::materialized(q, &HashSet::new());
        if without.as_ref().map(Query::to_string) != materialized.as_ref().map(Query::to_string) {
            outcome = without.as_ref().map(|m| host.describe(&m.to_string()));
            materialized = without;
        }
    }
    // A copy the server does not prepare, as a server before SET_VAR would not, leaves the
    // statement's own flags.
    let described: Option<Option<MysqlDescribe>> = match outcome {
        Some(None) => None,
        Some(Some(Ok(d))) if raw.is_none_or(|raw| raw.columns.len() == d.columns.len()) => Some(Some(d)),
        _ => match raw {
            Some(raw) => Some(Some(raw.clone())),
            None => host.describe(&q.to_string()).map(Result::ok),
        },
    };
    let described = match described {
        Some(Some(d)) => Some(d),
        // A query the server does not prepare alone proves nothing.
        Some(None) => return Some(Proven::default()),
        None => {
            pending = true;
            None
        }
    };
    let count = described.as_ref().map(|d| d.columns.len());
    let q_read = materialized.as_ref().unwrap_or(q);
    // What the WHERE proves (`mysql_nulls`): the columns it tests, and the statement with the outer
    // joins inner whose null-extended rows it rejects, which every probe below reads in its place.
    let mut not_null: Vec<usize> = crate::mysql_nulls::tested(q);
    let reduced: Option<Query> = crate::mysql_nulls::reduce(q_read);
    let q: &Query = reduced.as_ref().unwrap_or(q_read);
    // JSON shapes: the constructors' value expressions, prepared as columns of their own statements.
    // Whose an aggregate in a scalar subquery is can turn on the columns of the subquery's
    // tables, which the plan asks for by naming them; the probes are the same either way.
    let mut plan = crate::mysql_shape::plan(q, &Default::default());
    if !plan.wanted.is_empty() {
        match table_columns(host, &plan.wanted) {
            Some(columns) => plan = crate::mysql_shape::plan(q, &columns),
            None => pending = true,
        }
    }
    let mut outcomes: Vec<Option<MysqlDescribe>> = Vec::with_capacity(plan.probes.len());
    // Per probe, the JSON Schema a CHECK holds each JSON column it reads to.
    let mut documents: Vec<Vec<Option<Value>>> = Vec::with_capacity(plan.probes.len());
    for probe in &plan.probes {
        let mut described = &probe.sql;
        let mut outcome = host.describe(&probe.sql);
        if let (Some(Err(_)), Some(wider)) = (&outcome, &probe.wider) {
            described = wider;
            outcome = host.describe(wider);
        }
        match outcome {
            Some(Ok(d)) => {
                let read = parse(dialect, described).ok().and_then(|mut v| if v.len() == 1 { v.pop() } else { None });
                match self::documents(read.as_ref(), &d, host) {
                    Some(found) => documents.push(found),
                    None => {
                        pending = true;
                        documents.push(Vec::new());
                    }
                }
                outcomes.push(Some(d));
            }
            Some(Err(_)) => {
                outcomes.push(None);
                documents.push(Vec::new());
            }
            None => {
                pending = true;
                outcomes.push(None);
                documents.push(Vec::new());
            }
        }
    }
    // The flags of the statement with its outer joins reduced, by position.
    if let Some(reduced) = &reduced {
        match host.describe(&reduced.to_string()) {
            Some(Ok(d)) if Some(d.columns.len()) == count => not_null.extend(d.columns.iter().enumerate().filter(|(_, c)| c.flags & 1 != 0).map(|(i, _)| i)),
            Some(_) => {}
            None => pending = true,
        }
    }
    // An aggregate under a plain GROUP BY: the argument's flag over the rows it aggregates.
    match grouped_flags(q, host) {
        Some(found) => not_null.extend(found),
        None => pending = true,
    }
    // A window aggregate over a frame that holds the current row: the argument's flag there.
    match window_flags(q, count, host) {
        Some(found) => not_null.extend(found),
        None => pending = true,
    }
    // The outer columns' flags without the subquery items' pessimism, by projection position.
    match outer_flags(q, host) {
        Some(found) => not_null.extend(found),
        None => pending = true,
    }
    match through_derived(q, count, views, host, dialect, depth) {
        Some(found) => not_null.extend(found),
        None => pending = true,
    }
    if pending {
        return None;
    }
    let described = described.expect("a query described when nothing is pending");
    // What may reach each leaf: a placeholder in a document is the value it is given too.
    let flows: Vec<Vec<u8>> = plan.probes.iter().zip(&outcomes).map(|(probe, outcome)| outcome.as_ref().map(|d| probe_flow(&probe.sql, d, dialect)).unwrap_or_default()).collect();
    let shaped = plan
        .columns
        .iter()
        .map(|(name, shape)| {
            let (ty, non_null) = resolve(shape, &outcomes, &plan, &flows, &documents);
            (name.clone(), ty, non_null)
        })
        .collect();
    Some(Proven { flags: described.columns.iter().map(|c| c.flags & 1 != 0).collect(), columns: described.columns, not_null, shaped })
}

/// The positions of the columns that are an aggregate called as a window function over a frame
/// that always holds the current row ([`crate::mysql_nulls::windowed`]) and are never NULL: a JSON
/// aggregate, and one that follows its argument where the copy of the statement that selects the
/// argument in its place flags it NOT NULL or the WHERE proves it. The server flags every such
/// call nullable but the bit functions. `count` is how many columns the query has.
fn window_flags(q: &Query, count: Option<usize>, host: &Host) -> Lazy<Vec<usize>> {
    let Some((probe, follows, mut found)) = crate::mysql_nulls::window_probe(q) else { return Some(Vec::new()) };
    if !follows.is_empty() {
        if let Ok(d) = host.describe(&probe.to_string())? {
            if Some(d.columns.len()) == count {
                let tested = crate::mysql_nulls::tested(&probe);
                // An argument that is an aggregate of a plain GROUP BY (`sum(max(x)) over ()`)
                // is as NULL as that aggregate's own argument.
                let grouped = grouped_flags(&probe, host)?;
                found.extend(follows.into_iter().filter(|&i| d.columns.get(i).is_some_and(|c| c.flags & 1 != 0) || tested.contains(&i) || grouped.contains(&i)));
            }
        }
    }
    Some(found)
}

/// The positions of the columns that are an aggregate under a plain GROUP BY
/// ([`crate::mysql_nulls::grouped_aggregate`]) and are never NULL: a JSON aggregate, and one that
/// follows its argument where the statement that selects the arguments from the same FROM and
/// WHERE flags that argument NOT NULL or the WHERE proves it. The server flags every such call
/// nullable but the bit functions and `count`.
fn grouped_flags(q: &Query, host: &Host) -> Lazy<Vec<usize>> {
    let Some((probe, follows, mut found)) = crate::mysql_nulls::grouped_probe(q) else { return Some(Vec::new()) };
    if !follows.is_empty() {
        if let Ok(d) = host.describe(&probe.to_string())? {
            if d.columns.len() == follows.len() {
                let tested = crate::mysql_nulls::tested(&probe);
                found.extend(follows.into_iter().enumerate().filter(|&(j, _)| d.columns[j].flags & 1 != 0 || tested.contains(&j)).map(|(_, i)| i));
            }
        }
    }
    Some(found)
}

/// The positions of the outer columns the statement without its subquery items flags NOT NULL
/// ([`crate::mysql_shape::outer_probe`]).
fn outer_flags(q: &Query, host: &Host) -> Lazy<Vec<usize>> {
    let Some((sql, kept)) = crate::mysql_shape::outer_probe(q) else { return Some(Vec::new()) };
    Some(match host.describe(&sql)? {
        Ok(d) => kept.iter().enumerate().filter(|(k, _)| d.columns.get(*k).is_some_and(|c| c.flags & 1 != 0)).map(|(_, &i)| i).collect(),
        Err(_) => Vec::new(),
    })
}

/// The positions of the columns of a materialized query ([`crate::mysql_derived::materialized`])
/// that the server flags NOT NULL once each column of its derived tables and WITH queries that
/// the engine proves never NULL, by analyzing the table's query alone, is written so the server
/// sees it too ([`crate::mysql_derived::wrapper`]). The server then carries the proof through
/// whatever reads the table, which an outer join, a ROLLUP or an aggregate over no rows may still
/// make NULL. `count` is how many columns the query has.
fn through_derived(q: &Query, count: Option<usize>, views: &HashSet<String>, host: &Host, dialect: &SessionDialect, depth: usize) -> Lazy<Vec<usize>> {
    use crate::mysql_derived::{each_target, filler, standalone, wrapper};
    if depth >= MAX_DERIVED_DEPTH || !crate::mysql_derived::is_materialized(q) {
        return Some(Vec::new());
    }
    // A table read whole, as the copy reads each view, proves nothing of its own: it is no target
    // and takes no share of the bound.
    let (mut alone, mut whole): (Vec<Option<Query>>, usize) = (Vec::new(), 0);
    each_target(&mut q.clone(), &mut |t| {
        if crate::mysql_derived::reads_one_table(t.query) {
            whole += 1;
            alone.push(None);
        } else {
            alone.push(standalone(t.query, t.scope.as_ref()));
        }
    });
    if alone.len() == whole || alone.len() - whole > MAX_DERIVED {
        return Some(Vec::new());
    }
    let mut pending = false;
    let proofs: Vec<Proven> = alone
        .iter()
        .map(|q| match q {
            Some(q) => prove(q, None, views, host, dialect, depth + 1).unwrap_or_else(|| {
                pending = true;
                Proven::default()
            }),
            None => Proven::default(),
        })
        .collect();
    if pending {
        return None;
    }
    let mut copy = q.clone();
    let (mut next, mut changed) = (0, false);
    each_target(&mut copy, &mut |t| {
        let proven = &proofs[next];
        next += 1;
        let columns: Vec<(String, Option<&str>)> = proven
            .never_null(&proven.columns)
            .iter()
            .zip(&proven.columns)
            .zip(&proven.flags)
            .map(|(((never, _), c), flag)| (c.name.clone(), if *never && !flag { filler(c) } else { None }))
            .collect();
        if columns.iter().all(|(_, filler)| filler.is_none()) {
            return;
        }
        if let Ok(mut parsed) = parse(dialect, &wrapper(t.query, &columns, t.listed)) {
            if let (Some(Statement::Query(wrapped)), None) = (parsed.pop(), parsed.pop()) {
                *t.query = *wrapped;
                changed = true;
            }
        }
    });
    if !changed {
        return Some(Vec::new());
    }
    let mut not_null = Vec::new();
    match host.describe(&copy.to_string()) {
        Some(Ok(d)) if Some(d.columns.len()) == count => not_null.extend(d.columns.iter().enumerate().filter(|(_, c)| c.flags & 1 != 0).map(|(i, _)| i)),
        Some(_) => {}
        None => pending = true,
    }
    match outer_flags(&copy, host) {
        Some(found) => not_null.extend(found),
        None => pending = true,
    }
    if pending {
        None
    } else {
        Some(not_null)
    }
}

/// The functions MySQL reads as built-ins only where the parenthesis follows the name at once
/// (unless `sql_mode` has IGNORE_SPACE): after a space the name is a stored function's.
const SPACE_SENSITIVE: &[&str] = &[
    "adddate", "bit_and", "bit_or", "bit_xor", "cast", "count", "curdate", "curtime", "date_add", "date_sub", "extract", "group_concat", "max", "mid", "min", "now",
    "position", "session_user", "std", "stddev", "stddev_pop", "stddev_samp", "subdate", "substr", "substring", "sum", "sysdate", "system_user", "trim", "variance",
    "var_pop", "var_samp",
];

/// Why the engine's parser does not read the statement as this server does, if it does not.
/// `sqlparser` reads `||` as a concatenation that binds tighter than `=`, `count (x)` as the
/// aggregate and NOT as MySQL ranks it by default; the server reads each by the session's
/// `sql_mode`, where `a = b || c` is `(a = b) OR c` unless PIPES_AS_CONCAT is set, `count (x)`
/// calls a stored function `count` unless IGNORE_SPACE is set, and HIGH_NOT_PRECEDENCE moves NOT.
/// (Strings and double quotes the parser reads by the mode itself: [`SessionDialect`].) The mode
/// is asked for only when the text has one of these; an optimizer hint that sets a variable for
/// the statement leaves no mode to ask for.
fn misread(sql: &str, lexed: &Lexed, host: &Host) -> Lazy<Option<String>> {
    let text = |&(start, end): &(usize, usize)| &sql[start..end];
    let is_comment = |span: &str| span.starts_with("--") || span.starts_with("/*") || span.starts_with('#');
    if lexed.opaque.iter().map(text).any(|span| span.starts_with("/*+") && span.to_ascii_lowercase().contains("set_var")) {
        return Some(Some("an optimizer hint sets a variable for this statement (SET_VAR), which may change how the server reads it, so nothing bounds its rows".into()));
    }
    // The text outside strings, quoted names and comments, a comment as `\u{2}` and any other
    // of those as `\u{1}`.
    let mut code = String::with_capacity(sql.len());
    let mut at = 0;
    for &(start, end) in &lexed.opaque {
        code.push_str(&sql[at..start]);
        code.push_str(if is_comment(&sql[start..end]) { "\u{2}" } else { "\u{1}" });
        at = end;
    }
    code.push_str(&sql[at..]);
    let lower = code.to_ascii_lowercase();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    // Each word of the code with what stands before and after it.
    let mut words: Vec<(&str, Option<char>, &str)> = Vec::new();
    let mut i = 0;
    while let Some(c) = lower[i..].chars().next() {
        if !word(c) {
            i += c.len_utf8();
            continue;
        }
        let end = lower[i..].find(|c: char| !word(c)).map_or(lower.len(), |n| i + n);
        words.push((&lower[i..end], lower[..i].chars().next_back(), &lower[end..]));
        i = end;
    }
    let mut readings: Vec<(&str, bool, &str)> = Vec::new();
    if code.contains("||") {
        readings.push(("PIPES_AS_CONCAT", false, "`||` is OR, and binds looser than a comparison"));
    }
    // A built-in's name, then a space or a comment, then its parenthesis.
    let parted = |commented: bool| {
        words.iter().any(|(name, before, after)| {
            let gap: String = after.chars().take_while(|c| c.is_whitespace() || *c == '\u{2}').collect();
            SPACE_SENSITIVE.contains(name) && *before != Some('.') && !gap.is_empty() && gap.contains('\u{2}') == commented && after[gap.len()..].starts_with('(')
        })
    };
    // Under IGNORE_SPACE too (measured on 8.4): the mode skips spaces, and a comment is none.
    if parted(true) {
        return Some(Some("a comment between a built-in's name and its parenthesis makes the name a stored function's, which is not how the engine's parser reads the statement, so nothing bounds its rows".into()));
    }
    if parted(false) {
        readings.push(("IGNORE_SPACE", false, "a name with a space before its parenthesis is a stored function's, not the built-in's"));
    }
    // `is not` and `not in`, `not like`, `not between`, `not regexp` are operators of their own.
    let negates = |i: usize| {
        let follows = words.get(i + 1).map(|w| w.0);
        words[i].0 == "not" && !(i > 0 && words[i - 1].0 == "is") && !matches!(follows, Some("in" | "like" | "between" | "regexp" | "rlike"))
    };
    if (0..words.len()).any(negates) || code.replace("!=", "").contains('!') {
        readings.push(("HIGH_NOT_PRECEDENCE", true, "NOT binds tighter than a comparison"));
    }
    if readings.is_empty() {
        return Some(None);
    }
    let rows = host.query(SQL_MODE_QUERY, Vec::new(), Scope::Session)?;
    let mode = rows.ok().and_then(|rows| rows.first().and_then(|row| row.first()).and_then(as_str).map(str::to_ascii_uppercase));
    let Some(mode) = mode else {
        return Some(Some("the server did not say its sql_mode, which decides how it reads this statement, so nothing bounds its rows".into()));
    };
    let set = |name: &str| mode.split(',').any(|m| m.trim() == name);
    Some(readings.iter().find(|(name, when_set, _)| set(name) == *when_set).map(|(name, when_set, what)| {
        format!("under this server's sql_mode ({} {name}) {what}, which is not how the engine's parser reads the statement, so nothing bounds its rows", if *when_set { "with" } else { "without" })
    }))
}

// ---- the types an execution may answer with ---------------------------------------------------

/// What a placeholder's value may change in a column's type, by the kind of placeholder. The
/// server types a `?` at the prepare (an integer beside an integer column), and the statement
/// that runs has the type of the value in its place, which each driver gives it its own way.
///
/// A driver that binds (mysql2's `execute()`) sends a number that is no integer as a DOUBLE, which
/// makes `? + 1` a DOUBLE, and a `Date` as a DATETIME, which makes `? + interval 1 day` one where
/// the prepare said DATE. A driver that binds nothing (mariadb's `query()`, PlanetScale's
/// `execute()`) writes the value into the text, and the server types the statement by the
/// literal: `1.5 + 1` is a DECIMAL, `-5` a BIGINT where the prepare typed `-?` a DOUBLE,
/// `price * '2'` a DOUBLE where the prepare said DECIMAL, `'2026-01-01' + interval 1 day` a
/// string, and `coalesce('2', price)` a string.
///
/// A placeholder the server typed as an integer or as a float takes a number.
const INTEGER: u8 = 1;
const FLOAT: u8 = 2;
/// One typed as a DECIMAL takes a string of digits.
const DECIMAL: u8 = 4;
/// One typed as a date or a time takes a `Date` or a string.
const TEMPORAL: u8 = 8;
/// A string, bytes or JSON: `coalesce(doc, ?)` is JSON at the prepare and a string of bytes at
/// the execution.
const OTHER: u8 = 16;
/// Anything: a user variable (`@x`) holds whatever the session gave it, a bit operation is a
/// number or a string of bytes by what it is given, and an expression the walk does not read
/// may be either of those.
const ANY: u8 = 32;
/// The value may be the column's value as it is (a bare placeholder, an argument of COALESCE or
/// IF, a CASE result), where arithmetic and any other function compute a value from it.
const PASSED: u8 = 64;
/// Every kind of placeholder.
const PLACEHOLDERS: u8 = INTEGER | FLOAT | DECIMAL | TEMPORAL | OTHER;

/// The error of each parameter that is, bare or under casts and collations, a whole key of an
/// ORDER BY (a statement's, a window's, an aggregate's, an UPDATE's or a DELETE's), a GROUP BY or a
/// window's PARTITION BY, in the statement, its WITH queries, its derived tables and the subqueries
/// of its expressions, at the placeholder as the author wrote it. A key that only holds a parameter
/// (`case :sort when 'name' then name end`) is none.
fn constant_keys(sql: &str, statement: &Statement, rewritten: &lex::Rewritten) -> Vec<Diagnostic> {
    use crate::analysis::KeyClause;
    type Found = Vec<((u64, u64), KeyClause, bool)>;
    fn param(e: &Expr) -> Option<(u64, u64)> {
        match e {
            Expr::Value(v) if matches!(&v.value, ast::Value::Placeholder(p) if p == "?") => Some((v.span.start.line, v.span.start.column)),
            Expr::Nested(inner) | Expr::Cast { expr: inner, .. } | Expr::Convert { expr: inner, .. } | Expr::Collate { expr: inner, .. } => param(inner),
            _ => None,
        }
    }
    fn keys<'e>(list: impl ExactSizeIterator<Item = &'e Expr>, clause: KeyClause, out: &mut Found) {
        let alone = list.len() == 1;
        out.extend(list.filter_map(param).map(|p| (p, clause, alone)));
    }
    fn order(list: &[ast::OrderByExpr], out: &mut Found) {
        keys(list.iter().map(|k| &k.expr), KeyClause::OrderBy, out);
        list.iter().for_each(|k| expr(&k.expr, out));
    }
    fn window(spec: &ast::WindowSpec, out: &mut Found) {
        keys(spec.partition_by.iter(), KeyClause::PartitionBy, out);
        spec.partition_by.iter().for_each(|e| expr(e, out));
        order(&spec.order_by, out);
    }
    fn query(q: &Query, out: &mut Found) {
        for cte in q.with.iter().flat_map(|w| &w.cte_tables) {
            query(&cte.query, out);
        }
        set(&q.body, out);
        if let Some(ast::OrderBy { kind: ast::OrderByKind::Expressions(list), .. }) = &q.order_by {
            order(list, out);
        }
    }
    fn set(body: &SetExpr, out: &mut Found) {
        match body {
            SetExpr::Select(s) => select(s, out),
            SetExpr::Query(q) => query(q, out),
            SetExpr::SetOperation { left, right, .. } => {
                set(left, out);
                set(right, out);
            }
            _ => {}
        }
    }
    fn select(s: &Select, out: &mut Found) {
        for item in &s.projection {
            if let SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } = item {
                expr(e, out);
            }
        }
        s.from.iter().for_each(|t| from(t, out));
        s.selection.iter().chain(&s.having).for_each(|e| expr(e, out));
        if let GroupByExpr::Expressions(list, _) = &s.group_by {
            keys(list.iter(), KeyClause::GroupBy, out);
            list.iter().for_each(|e| expr(e, out));
        }
        for ast::NamedWindowDefinition(_, w) in &s.named_window {
            if let ast::NamedWindowExpr::WindowSpec(spec) = w {
                window(spec, out);
            }
        }
    }
    fn from(t: &ast::TableWithJoins, out: &mut Found) {
        factor(&t.relation, out);
        for join in &t.joins {
            factor(&join.relation, out);
            use ast::JoinOperator::*;
            if let Join(c) | Inner(c) | Left(c) | LeftOuter(c) | Right(c) | RightOuter(c) | CrossJoin(c) | StraightJoin(c) = &join.join_operator {
                if let ast::JoinConstraint::On(e) = c {
                    expr(e, out);
                }
            }
        }
    }
    fn factor(f: &TableFactor, out: &mut Found) {
        match f {
            TableFactor::Derived { subquery, .. } => query(subquery, out),
            TableFactor::NestedJoin { table_with_joins, .. } => from(table_with_joins, out),
            _ => {}
        }
    }
    fn expr(e: &Expr, out: &mut Found) {
        match e {
            Expr::Subquery(q) => query(q, out),
            Expr::Exists { subquery, .. } => query(subquery, out),
            Expr::InSubquery { expr: left, subquery, .. } => {
                expr(left, out);
                query(subquery, out);
            }
            Expr::Function(f) => {
                if let ast::FunctionArguments::List(list) = &f.args {
                    for arg in &list.args {
                        if let Some(e) = crate::mysql_params::arg_expr(arg) {
                            expr(e, out);
                        }
                    }
                    for clause in &list.clauses {
                        if let ast::FunctionArgumentClause::OrderBy(keys) = clause {
                            order(keys, out);
                        }
                    }
                }
                if let ast::FunctionArguments::Subquery(q) = &f.args {
                    query(q, out);
                }
                order(&f.within_group, out);
                if let Some(ast::WindowType::WindowSpec(spec)) = &f.over {
                    window(spec, out);
                }
            }
            Expr::Case { operand, conditions, else_result, .. } => {
                for e in operand.iter().chain(else_result).map(|b| b.as_ref()).chain(conditions.iter().flat_map(|w| [&w.condition, &w.result])) {
                    expr(e, out);
                }
            }
            Expr::BinaryOp { left, right, .. } => {
                expr(left, out);
                expr(right, out);
            }
            Expr::InList { expr: left, list, .. } => std::iter::once(left.as_ref()).chain(list).for_each(|e| expr(e, out)),
            Expr::Between { expr: e, low, high, .. } => [e, low, high].into_iter().for_each(|e| expr(e, out)),
            Expr::Like { expr: e, pattern, .. } | Expr::RLike { expr: e, pattern, .. } => [e, pattern].into_iter().for_each(|e| expr(e, out)),
            Expr::Tuple(items) => items.iter().for_each(|e| expr(e, out)),
            Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } | Expr::Cast { expr: inner, .. } | Expr::Convert { expr: inner, .. } | Expr::Collate { expr: inner, .. } | Expr::IsNull(inner) | Expr::IsNotNull(inner) => expr(inner, out),
            _ => {}
        }
    }
    let mut found = Found::new();
    match statement {
        Statement::Query(q) => query(q, &mut found),
        Statement::Insert(insert) => insert.source.iter().for_each(|q| query(q, &mut found)),
        Statement::Update(update) => {
            from(&update.table, &mut found);
            update.assignments.iter().map(|a| &a.value).chain(&update.selection).for_each(|e| expr(e, &mut found));
            order(&update.order_by, &mut found);
        }
        Statement::Delete(delete) => {
            delete.selection.iter().for_each(|e| expr(e, &mut found));
            order(&delete.order_by, &mut found);
        }
        _ => {}
    }
    // Each `?` of the rewritten text by where the parser finds it (lines and characters from 1),
    // with its place in the author's text.
    let mut at: HashMap<(u64, u64), (usize, usize)> = HashMap::new();
    for &(start, end, rs, _) in &rewritten.spans {
        let before = &rewritten.sql[..rs.min(rewritten.sql.len())];
        let line = before.matches('\n').count() as u64 + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) as u64 + 1;
        at.insert((line, column), (start, end));
    }
    let mut out: Vec<Diagnostic> = found.into_iter().filter_map(|(p, clause, alone)| at.get(&p).map(|&(start, end)| crate::analysis::constant_key(sql, start, end, clause, alone))).collect();
    out.sort_by_key(|d| d.start);
    out.dedup_by_key(|d| d.start);
    out
}

/// Whether an expression is a user variable. `sqlparser` reads `@x` as a name and `` @`x` `` as a
/// placeholder; `@@x` is a system variable, of one type.
pub(crate) fn is_variable(e: &Expr) -> bool {
    match e {
        Expr::Identifier(i) => i.quote_style.is_none() && i.value.starts_with('@') && !i.value.starts_with("@@"),
        Expr::Value(v) => matches!(&v.value, ast::Value::Placeholder(p) if p.starts_with('@')),
        _ => false,
    }
}

/// The select-list items of a query, in order: what may reach each one's type, and for a
/// wildcard what may reach any of its columns.
enum Item {
    One(u8),
    Star(u8),
}

struct Flow<'a> {
    /// The kind of each placeholder by where its `?` stands: line and column, as `sqlparser` counts.
    kinds: &'a HashMap<(u64, u64), u8>,
    /// Every kind the statement has, for a placeholder or an expression the walk cannot place.
    all: u8,
}

impl Flow<'_> {
    /// What may reach each item of a query's select list. A column of a derived table or of a
    /// WITH query carries what reached that query's own items, so every column a query reads
    /// carries `inherited`, plus whatever its own WITH queries and derived tables carry.
    fn query(&self, q: &Query, inherited: u8) -> Vec<Item> {
        let any = |items: Vec<Item>| items.iter().fold(0, |all, item| all | match item { Item::One(k) | Item::Star(k) => *k });
        let mut carried = inherited;
        for cte in q.with.iter().flat_map(|w| &w.cte_tables) {
            carried |= any(self.query(&cte.query, carried));
        }
        self.body(&q.body, carried)
    }

    fn body(&self, body: &SetExpr, mut carried: u8) -> Vec<Item> {
        let any = |items: &[Item]| items.iter().fold(0, |all, item| all | match item { Item::One(k) | Item::Star(k) => *k });
        match body {
            SetExpr::Select(select) => {
                for factor in select.from.iter().flat_map(|t| std::iter::once(&t.relation).chain(t.joins.iter().map(|j| &j.relation))) {
                    carried |= self.factor(factor, carried);
                }
                select
                    .projection
                    .iter()
                    .map(|item| match item {
                        SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => Item::One(self.expr(e, carried)),
                        _ => Item::Star(carried),
                    })
                    .collect()
            }
            SetExpr::Query(q) => self.query(q, carried),
            // A column of a set operation has the type its arms agree on.
            SetExpr::SetOperation { left, right, .. } => {
                let (left, right) = (self.body(left, carried), self.body(right, carried));
                let star = left.iter().chain(&right).any(|item| matches!(item, Item::Star(_)));
                if star || left.len() != right.len() {
                    let all = any(&left) | any(&right);
                    return left.iter().map(|_| Item::Star(all)).collect();
                }
                left.iter().zip(&right).map(|(l, r)| Item::One(any(std::slice::from_ref(l)) | any(std::slice::from_ref(r)))).collect()
            }
            SetExpr::Values(values) => {
                let width = values.rows.iter().map(|row| row.content.len()).max().unwrap_or(0);
                (0..width).map(|i| Item::One(values.rows.iter().filter_map(|row| row.content.get(i)).fold(0, |all, e| all | self.expr(e, carried)))).collect()
            }
            // TABLE reads a table's own columns; anything else is not walked.
            SetExpr::Table(_) => vec![Item::Star(0)],
            _ => vec![Item::Star(self.all)],
        }
    }

    /// What the columns of one FROM item carry.
    fn factor(&self, factor: &TableFactor, carried: u8) -> u8 {
        let any = |items: Vec<Item>| items.iter().fold(0, |all, item| all | match item { Item::One(k) | Item::Star(k) => *k });
        match factor {
            TableFactor::Table { .. } => 0,
            TableFactor::Derived { subquery, .. } => any(self.query(subquery, carried)),
            TableFactor::NestedJoin { table_with_joins, .. } => {
                std::iter::once(&table_with_joins.relation).chain(table_with_joins.joins.iter().map(|j| &j.relation)).fold(0, |all, f| all | self.factor(f, carried))
            }
            // A table function's columns are typed by its own declaration (JSON_TABLE's COLUMNS).
            TableFactor::JsonTable { .. } => 0,
            _ => self.all,
        }
    }

    /// What may reach an expression's type: a placeholder it computes with, a column that carries
    /// one. A comparison is a truth value whatever it compares, and a CAST is the type it names.
    fn expr(&self, e: &Expr, carried: u8) -> u8 {
        use ast::BinaryOperator as B;
        let arg = |a: &FunctionArg| match a {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(x)) | FunctionArg::Named { arg: FunctionArgExpr::Expr(x), .. } | FunctionArg::ExprNamed { arg: FunctionArgExpr::Expr(x), .. } => self.expr(x, carried),
            _ => 0,
        };
        match e {
            _ if is_variable(e) => ANY,
            Expr::Value(v) => match &v.value {
                ast::Value::Placeholder(_) => self.kinds.get(&(v.span.start.line, v.span.start.column)).copied().unwrap_or(self.all) | PASSED,
                _ => 0,
            },
            Expr::Identifier(i) if i.value.starts_with("@@") => 0,
            Expr::Identifier(_) | Expr::CompoundIdentifier(_) => carried,
            Expr::Nested(inner) => self.expr(inner, carried),
            // A unary plus is not there for the server.
            Expr::UnaryOp { op: ast::UnaryOperator::Plus, expr } => self.expr(expr, carried),
            Expr::UnaryOp { op: ast::UnaryOperator::Minus, expr } => self.expr(expr, carried) & !PASSED,
            Expr::UnaryOp { op: ast::UnaryOperator::Not, .. } => 0,
            Expr::BinaryOp { left, op, right } => match op {
                B::Eq | B::NotEq | B::Lt | B::LtEq | B::Gt | B::GtEq | B::Spaceship | B::And | B::Or | B::Xor => 0,
                B::Plus | B::Minus | B::Multiply | B::Divide | B::Modulo | B::MyIntegerDivide => (self.expr(left, carried) | self.expr(right, carried)) & !PASSED,
                // A bit operation, and any operator the walk does not know.
                _ => match self.expr(left, carried) | self.expr(right, carried) {
                    0 => 0,
                    reached => reached | ANY,
                },
            },
            Expr::Cast { .. } | Expr::Convert { .. } | Expr::TypedString { .. } | Expr::Interval(_) => 0,
            // An integer and a string whatever they are given.
            Expr::Position { .. } | Expr::Extract { .. } | Expr::Substring { .. } | Expr::Trim { .. } | Expr::Overlay { .. } | Expr::MatchAgainst { .. } => 0,
            Expr::Ceil { expr, .. } | Expr::Floor { expr, .. } => self.expr(expr, carried) & !PASSED,
            Expr::Collate { expr, .. } => self.expr(expr, carried),
            Expr::IsNull(_) | Expr::IsNotNull(_) | Expr::IsTrue(_) | Expr::IsNotTrue(_) | Expr::IsFalse(_) | Expr::IsNotFalse(_) | Expr::IsUnknown(_) | Expr::IsNotUnknown(_) => 0,
            Expr::InList { .. } | Expr::InSubquery { .. } | Expr::Between { .. } | Expr::Like { .. } | Expr::RLike { .. } | Expr::Exists { .. } => 0,
            // A JSON function's type is its own, whatever it is given.
            Expr::Function(f) if builtin(f).starts_with("json_") => 0,
            Expr::Function(f) => {
                let reached = match &f.args {
                    ast::FunctionArguments::None => 0,
                    ast::FunctionArguments::List(list) => list.args.iter().fold(0, |all, a| all | arg(a)),
                    ast::FunctionArguments::Subquery(q) => self.query(q, carried).iter().fold(0, |all, item| all | match item { Item::One(k) | Item::Star(k) => *k }),
                };
                // These hand one of their arguments back as it is; any other built-in computes a
                // value from them.
                const PASSING: &[&str] = &["coalesce", "ifnull", "if", "nullif", "greatest", "least", "any_value", "min", "max", "lag", "lead", "first_value", "last_value", "nth_value", "name_const"];
                // These are an integer whatever they are given (`unix_timestamp` is not: a
                // BIGINT for a date and a DECIMAL for a time with a fraction).
                const INTEGERS: &[&str] = &[
                    "length", "char_length", "character_length", "octet_length", "bit_length", "ascii", "ord", "locate", "instr", "find_in_set", "field", "strcmp", "year",
                    "month", "day", "dayofmonth", "dayofweek", "dayofyear", "quarter", "week", "weekday", "yearweek", "hour", "minute", "second", "microsecond", "datediff",
                    "to_days", "to_seconds", "crc32", "count", "sign", "isnull", "period_diff", "timestampdiff",
                ];
                // A bit function is a number or a string of bytes by what it is given.
                const BITS: &[&str] = &["bit_and", "bit_or", "bit_xor", "bit_count"];
                match builtin(f).as_str() {
                    // A stored function, by its schema: it returns the type it declares.
                    "" => 0,
                    name if INTEGERS.contains(&name) => 0,
                    name if PASSING.contains(&name) => reached,
                    name if BITS.contains(&name) && reached != 0 => reached | ANY,
                    _ => reached & !PASSED,
                }
            }
            Expr::Case { conditions, else_result, .. } => conditions.iter().fold(else_result.as_deref().map_or(0, |e| self.expr(e, carried)), |all, when| all | self.expr(&when.result, carried)),
            Expr::Subquery(q) => self.query(q, carried).iter().fold(0, |all, item| all | match item { Item::One(k) | Item::Star(k) => *k }),
            // An operator or a form the walk does not read: any placeholder may be in it.
            _ => match self.all | carried {
                0 => 0,
                reached => reached | ANY,
            },
        }
    }
}

/// The type of each result column an execution may answer with another type than the prepare's,
/// by position; `None` where the prepare's stands. Such a column is typed as either: a scalar
/// whose name lists both (`int8|float8`), which the renderer writes as a union.
///
/// A column a placeholder may reach ([`Flow`]): a number may come back a BIGINT, a DOUBLE or a
/// DECIMAL, a date as a DATE, a DATETIME, a TIME or a string, and a TIME or a JSON value is
/// unknown, as is any column a user variable or a bit operation over a placeholder may reach.
///
/// And an integer the statement computes, where the plan passes it through a temporary table (a
/// GROUP BY or an ORDER BY the server has no index for, DISTINCT, a window, SQL_BUFFER_RESULT):
/// the table's column is an INT where the expression's display length is under ten digits and a
/// BIGINT from there on, whatever the prepare typed it as. Measured on 8.4, `quarter(day)` and
/// `n > 1` prepare as BIGINT and answer INT, and `coalesce(n, 0)` and `lag(n) over (…)` prepare
/// as INT and answer BIGINT. Only a table's own column keeps its type: the prepare names the
/// table, and `views` says which names are no table's (none known: every one may be). A column
/// under WITH ROLLUP names no table.
fn settle(sql: &str, lexed: &Lexed, rewritten: &lex::Rewritten, parsed: Option<&Statement>, raw: &MysqlDescribe, views: Option<&HashSet<String>>) -> Vec<Option<Type>> {
    let kind = placeholder_kind;
    // The text outside strings, names and comments.
    let mut code = String::with_capacity(sql.len());
    let mut at = 0;
    for &(start, end) in &lexed.opaque {
        code.push_str(&sql[at..start]);
        code.push(' ');
        at = end;
    }
    code.push_str(&sql[at..]);
    let variables = code.replace("@@", "").contains('@');
    let all = raw.params.iter().fold(if variables { ANY } else { 0 }, |all, p| all | kind(p.r#type));
    // Where each `?` of the rewritten text stands, as the parser counts: lines and characters from 1.
    let mut kinds: HashMap<(u64, u64), u8> = HashMap::new();
    for (i, &(_, _, start, _)) in rewritten.spans.iter().enumerate() {
        let before = &rewritten.sql[..start.min(rewritten.sql.len())];
        let line = before.matches('\n').count() as u64 + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) as u64 + 1;
        kinds.insert((line, column), raw.params.get(i).map_or(all, |p| kind(p.r#type)));
    }
    let flow = Flow { kinds: &kinds, all };
    // What may reach each column, by position: the items before a wildcard count from the
    // start, those after the last one from the end, and the rest share what is between.
    let width = raw.columns.len();
    let reached: Vec<u8> = match (all, parsed) {
        (0, _) => vec![0; width],
        (_, Some(Statement::Query(q))) => {
            let items = flow.query(q, 0);
            let one = |item: &Item| match item {
                Item::One(k) | Item::Star(k) => *k,
            };
            let head: Vec<u8> = items.iter().take_while(|i| matches!(i, Item::One(_))).map(one).collect();
            let tail: Vec<u8> = if head.len() == items.len() { Vec::new() } else { items.iter().rev().take_while(|i| matches!(i, Item::One(_))).map(one).collect::<Vec<_>>().into_iter().rev().collect() };
            let middle = items[head.len()..items.len() - tail.len()].iter().fold(0, |all, i| all | one(i));
            if head.len() + tail.len() > width || (head.len() == items.len() && head.len() != width) {
                vec![all; width]
            } else {
                (0..width).map(|i| if i < head.len() { head[i] } else if i >= width - tail.len() { tail[i - (width - tail.len())] } else { middle }).collect()
            }
        }
        // A statement the engine does not read: any placeholder may reach any column.
        _ => vec![all; width],
    };
    let own = |c: &crate::api::MysqlColumn| c.org_table.as_ref().is_some_and(|table| !table.is_empty() && views.is_some_and(|views| !views.contains(&table.to_ascii_lowercase())));
    raw.columns
        .iter()
        .zip(&reached)
        .map(|(c, reached)| {
            // A table's own column is of its type whatever is bound and whatever the plan.
            if own(c) {
                return None;
            }
            let ty = column_type(c.r#type, c.charset).0;
            match (widened(&ty, *reached), other_width(c)) {
                (Some(Type::Scalar { name }), Some(other)) if !name.split('|').any(|n| n == other) => Some(Type::Scalar { name: format!("{name}|{other}") }),
                (None, Some(other)) => match ty {
                    Type::Scalar { name } => Some(Type::Scalar { name: format!("{name}|{other}") }),
                    _ => None,
                },
                (widened, _) => widened,
            }
        })
        .collect()
}

/// The type a value of type `ty` may have where `reached` may reach it ([`Flow`]), if another.
/// A value the statement computes from a placeholder is of its family, and which member is the
/// value's to say: a number is a BIGINT, a DOUBLE or a DECIMAL, and a date a DATE, a DATETIME, a
/// TIME or, from a string written into the text, a string. Where a placeholder that takes a
/// string may be handed back as it is, so is a number.
fn widened(ty: &Type, reached: u8) -> Option<Type> {
    if reached & ANY != 0 {
        return Some(Type::Unknown);
    }
    let placeholders = reached & PLACEHOLDERS != 0;
    let name = match ty {
        Type::Scalar { name } => name,
        Type::Json { .. } if placeholders => return Some(Type::Unknown),
        _ => return None,
    };
    let string = reached & PASSED != 0 && reached & (DECIMAL | TEMPORAL | OTHER) != 0;
    let mut names: Vec<&str> = vec![name.as_str()];
    match name.as_str() {
        "time" if placeholders => return Some(Type::Unknown),
        "date" | "timestamp" if placeholders => names.extend(["date", "timestamp", "time", "text"]),
        "int2" | "int4" | "int8" | "numeric" | "float4" | "float8" if placeholders => {
            names.extend(["int8", "float8", "numeric"]);
            if string {
                names.push("text");
            }
        }
        _ => {}
    }
    let mut distinct: Vec<&str> = Vec::new();
    for name in names {
        if !distinct.contains(&name) {
            distinct.push(name);
        }
    }
    (distinct.len() > 1).then(|| Type::Scalar { name: distinct.join("|") })
}

/// Whether a statement's first word makes it a write or a SELECT, which return no result set when
/// the prepare gives them no columns (a `CALL` does not say what its procedure returns).
fn opens_without_result_sets(sql: &str) -> bool {
    use sqlparser::keywords::Keyword;
    let Ok(tokens) = sqlparser::tokenizer::Tokenizer::new(&MySqlDialect {}, sql).tokenize() else { return false };
    let first = tokens.iter().find_map(|t| match t {
        sqlparser::tokenizer::Token::Whitespace(_) | sqlparser::tokenizer::Token::LParen => None,
        sqlparser::tokenizer::Token::Word(w) => Some(Some(w.keyword)),
        _ => Some(None),
    });
    matches!(first, Some(Some(Keyword::DELETE | Keyword::UPDATE | Keyword::INSERT | Keyword::REPLACE | Keyword::SELECT | Keyword::WITH)))
}

/// Whether a query's rows go into variables or a file (`select … into @n`), so it returns none.
fn assigns(q: &ast::Query) -> bool {
    match q.body.as_ref() {
        SetExpr::Select(s) => s.into.is_some(),
        SetExpr::Query(q) => assigns(q),
        _ => false,
    }
}

/// The kind of statement the analysis says it is.
fn kind(read: Option<&Statement>) -> &'static str {
    match read {
        Some(Statement::Query(_)) => "select",
        Some(Statement::Insert(_)) => "insert",
        Some(Statement::Update(_)) => "update",
        Some(Statement::Delete(_)) => "delete",
        _ => "",
    }
}

/// Parse with a depth bound of a fifth of [`crate::stack::MAX_DEPTH`] in place of sqlparser's
/// default of 50: 200 levels of its recursion, which are 196 nested function calls (`coalesce`,
/// `abs`, `json_object`), 195 nested `CASE`s, or 97 nested subqueries, which spend two levels each.
/// It is less than the Postgres parser's 500 levels because in the WebAssembly engine sqlparser's
/// recursion runs on the JavaScript engine's own stack, which a host shares (`core/src/stack.rs`),
/// at about twice libpg_query's cost a level: a nested function call is eight frames of the parser,
/// some 2 kB of Node's stack once V8 has optimized them, so under the Postgres bound 496 nested
/// `coalesce`s needed two thirds of Node's stack before V8 optimized the engine and, on Node 24,
/// more than all of it after. At 200 the deepest statement of every shape needs at most 42% of it,
/// as on Postgres, whatever V8 has compiled the engine to (measured on Node 24 and 26, 2026-10-21).
/// sqlparser's bound counts only its own recursion, and it reads a chain of operators (`a + b + …`,
/// `x = 1 or x = 2 or …`) in a loop into a tree as deep as the chain, which every walk of the
/// engine and the tree's drop then recurse through: so a statement with more operators than half of
/// [`crate::stack::MAX_DEPTH`] is refused before it is read, a bound of its own, since those walks
/// cost a fraction of the parser's recursion (501 terms need 15% of the stack). That counts
/// operators in separate expressions together, and only ever leaves a statement to the describe
/// that a deeper bound would have read.
///
/// MySQL's compound interval units (`interval '1:30' minute_second`, `extract(day_hour from d)`)
/// are not sqlparser's (0.63 refuses the statement), so each is read as its smallest part, the
/// unit that decides what MySQL computes: `day_hour` as `hour` keeps `date_add` of a DATE a
/// DATETIME, `year_month` as `month` keeps it a DATE, and EXTRACT is a BIGINT either way. Each of
/// them is a reserved word in MySQL, so a bare one is always the unit. The token keeps its place
/// in the text; only a probe printed from the tree (a JSON shape's) names the smaller unit, which
/// the server types alike.
fn parse(dialect: &dyn sqlparser::dialect::Dialect, sql: &str) -> Result<Vec<Statement>, sqlparser::parser::ParserError> {
    use sqlparser::keywords::Keyword;
    use sqlparser::tokenizer::{Token, Word};
    let mut tokens = sqlparser::tokenizer::Tokenizer::new(dialect, sql).tokenize_with_location()?;
    for t in tokens.iter_mut() {
        let Token::Word(w) = &t.token else { continue };
        if w.quote_style.is_some() {
            continue;
        }
        let unit = match w.value.to_ascii_uppercase().as_str() {
            "SECOND_MICROSECOND" | "MINUTE_MICROSECOND" | "HOUR_MICROSECOND" | "DAY_MICROSECOND" => (Keyword::MICROSECOND, "MICROSECOND"),
            "MINUTE_SECOND" | "HOUR_SECOND" | "DAY_SECOND" => (Keyword::SECOND, "SECOND"),
            "HOUR_MINUTE" | "DAY_MINUTE" => (Keyword::MINUTE, "MINUTE"),
            "DAY_HOUR" => (Keyword::HOUR, "HOUR"),
            "YEAR_MONTH" => (Keyword::MONTH, "MONTH"),
            _ => continue,
        };
        t.token = Token::Word(Word { value: unit.1.to_string(), quote_style: None, keyword: unit.0 });
    }
    let operators = tokens
        .iter()
        .filter(|t| match &t.token {
            Token::Word(w) => matches!(
                w.keyword,
                Keyword::AND | Keyword::OR | Keyword::XOR | Keyword::DIV | Keyword::MOD | Keyword::NOT | Keyword::IS | Keyword::IN | Keyword::LIKE | Keyword::RLIKE | Keyword::REGEXP | Keyword::BETWEEN | Keyword::COLLATE | Keyword::MEMBER | Keyword::ESCAPE
            ) && w.quote_style.is_none(),
            Token::Whitespace(_) | Token::Comma | Token::LParen | Token::RParen | Token::Period | Token::SemiColon | Token::Placeholder(_) | Token::Number(..) | Token::EOF => false,
            t => !matches!(t.to_string().chars().next(), Some('\'' | '"' | '`' | 'X' | 'x' | 'N' | 'B' | 'b')),
        })
        .count();
    if operators > crate::stack::MAX_DEPTH / 2 {
        return Err(sqlparser::parser::ParserError::RecursionLimitExceeded);
    }
    Parser::new(dialect).with_recursion_limit(crate::stack::MAX_DEPTH / 5).with_tokens_with_locations(tokens).parse_statements()
}

/// What may reach each item of a probe's select list. A probe is the engine's own text, where a
/// `?` is not where the statement's was, so any of the probe's placeholders may stand anywhere.
fn probe_flow(sql: &str, described: &MysqlDescribe, dialect: &SessionDialect) -> Vec<u8> {
    let all = described.params.iter().fold(0, |all, p| all | placeholder_kind(p.r#type));
    let unread = vec![if all == 0 { 0 } else { all | ANY }; described.columns.len()];
    let Ok(mut statements) = parse(dialect, sql) else { return unread };
    let (Some(Statement::Query(q)), None) = (statements.pop(), statements.pop()) else { return unread };
    let kinds = HashMap::new();
    let flow = Flow { kinds: &kinds, all };
    flow.query(&q, 0).iter().map(|item| match item { Item::One(k) | Item::Star(k) => *k }).collect()
}

/// The kind of a placeholder the server typed with this protocol type.
fn placeholder_kind(code: u32) -> u8 {
    match code {
        1 | 2 | 3 | 8 | 9 | 13 => INTEGER,
        4 | 5 => FLOAT,
        0 | 246 => DECIMAL,
        7 | 10 | 11 | 12 | 14 => TEMPORAL,
        _ => OTHER,
    }
}

/// The integer type a temporary table gives an expression the prepare typed as the other one:
/// an INT under ten digits of display length, a BIGINT from ten on. A length the host did not
/// send may be either.
fn other_width(c: &crate::api::MysqlColumn) -> Option<&'static str> {
    match c.r#type {
        1 | 2 | 3 | 9 | 13 if c.length.is_none_or(|l| l > 9) => Some("int8"),
        8 if c.length.is_none_or(|l| l <= 9) => Some("int4"),
        _ => None,
    }
}

/// Whether a column's prepare says it is one digit or one bit wide: a TINYINT(1), which MySQL's
/// BOOLEAN is, or a BIT(1). MySQL 8 reports a TINYINT's display width only where it is 1 (every
/// other TINYINT reports 4), and a BIT's width is its own (measured on 8.4.11 and 9.7.2,
/// wiki/driver-decoding.md).
fn flag_width(c: &crate::api::MysqlColumn) -> bool {
    matches!((c.r#type, c.length), (1 | 16, Some(1)))
}

/// Whether a result column is a table's own TINYINT(1) or BIT(1) column, read straight from it
/// (through a derived table or a WITH query too, which the prepare traces to the table): what an
/// application stores a truth value in, and which the run time reads as a boolean, refusing a value
/// other than 0 or 1. An expression names no table, and a view's column may be an expression whose
/// width says nothing, so neither is one; nor is any column where the views are not known.
fn flag_column(c: &crate::api::MysqlColumn, views: Option<&HashSet<String>>) -> bool {
    let named = |s: &Option<String>| s.as_ref().is_some_and(|s| !s.is_empty());
    flag_width(c) && named(&c.org_name) && c.org_table.as_ref().is_some_and(|table| !table.is_empty() && views.is_some_and(|views| !views.contains(&table.to_ascii_lowercase())))
}

/// A result column's protocol type code as the type model, with the MySQL name for messages.
/// The names are the Postgres ones, which the renderer and the runtime conversion already read.
fn column_type(code: u32, charset: u32) -> (Type, &'static str) {
    let scalar = |name: &str| Type::Scalar { name: name.to_string() };
    match code {
        1 => (scalar("int2"), "tinyint"),
        2 => (scalar("int2"), "smallint"),
        3 => (scalar("int4"), "int"),
        9 => (scalar("int4"), "mediumint"),
        13 => (scalar("int2"), "year"),
        8 => (scalar("int8"), "bigint"),
        4 => (scalar("float4"), "float"),
        5 => (scalar("float8"), "double"),
        0 | 246 => (scalar("numeric"), "decimal"),
        7 => (scalar("timestamp"), "timestamp"),
        12 => (scalar("timestamp"), "datetime"),
        10 | 14 => (scalar("date"), "date"),
        11 => (scalar("time"), "time"),
        245 => (Type::Json { schema: None }, "json"),
        // MySQL 9's VECTOR, a column and the context of a `?` alike: 32-bit floats, none of them
        // NULL, whose value comes as their little-endian bytes (measured on 9.7.2). A parameter
        // binds their text instead (`vectors_as_text`).
        242 => (Type::Array { of: Box::new(scalar("float4")), nullable_items: Some(false), delimiter: None }, "vector"),
        16 => (scalar("bytea"), "bit"),
        249..=254 | 15 => {
            if charset == 63 { (scalar("bytea"), "binary") } else { (scalar("text"), "varchar") }
        }
        6 => (Type::Unknown, "null"),
        _ => (Type::Unknown, "unknown"),
    }
}

/// Each `?` the prepare typed as a VECTOR wrapped in `to_vector(…)`, so that it binds the text of
/// its floats (`[1,2.5]`), which the server reads as a VECTOR wherever it stands. A VECTOR's bytes
/// would not do on every driver: mariadb and PlanetScale write every value into the statement's
/// text, where bytes are a binary string, which MySQL takes for a VECTOR column or a function's
/// argument but refuses beside `=` ("Incorrect arguments to =", 9.7.2). `to_vector` is
/// `string_to_vector`'s other name.
fn vectors_as_text(rewritten: lex::Rewritten, raw: &MysqlDescribe) -> lex::Rewritten {
    if !raw.params.iter().any(|p| p.r#type == 242) {
        return rewritten;
    }
    let mut sql = String::with_capacity(rewritten.sql.len() + 11 * raw.params.len());
    let mut spans = Vec::with_capacity(rewritten.spans.len());
    let mut from = 0;
    for (i, &(os, oe, rs, re)) in rewritten.spans.iter().enumerate() {
        sql.push_str(&rewritten.sql[from..rs]);
        let start = sql.len();
        let vector = raw.params.get(i).is_some_and(|p| p.r#type == 242);
        if vector {
            sql.push_str("to_vector(");
        }
        sql.push_str(&rewritten.sql[rs..re]);
        if vector {
            sql.push(')');
        }
        spans.push((os, oe, start, sql.len()));
        from = re;
    }
    sql.push_str(&rewritten.sql[from..]);
    // Each text the rewrite added (`mysql_collation`) moved with the wraps before it.
    let moved = |p: usize| rewritten.spans.iter().zip(&spans).filter(|((_, _, _, re), _)| *re <= p).last().map_or(p, |((_, _, _, re), (_, _, _, new_re))| p - re + new_re);
    let inserted = rewritten.inserted.iter().map(|&(at, rs, re)| (at, moved(rs), moved(re))).collect();
    lex::Rewritten { sql, order: rewritten.order, spans, inserted }
}

/// A parameter's type from the server's context guess. Every integer context comes back as
/// LONGLONG, whatever the column's width, so an integer parameter is a plain number (the run
/// time's parameter types widen it to the BIGINT and TINYINT(1) values a row returns). A BIT
/// context comes back as BIT with the length 64, whatever the column's, and a BIT(1) column is a
/// truth value where a row holds it, so the parameter is either.
fn param_type(code: u32, charset: u32) -> (Type, &'static str) {
    match code {
        1 | 2 | 3 | 8 | 9 | 13 => (Type::Scalar { name: "int4".into() }, "int"),
        16 => (Type::Scalar { name: "bool|bytea".into() }, "bit"),
        _ => column_type(code, charset),
    }
}

/// One shape plus its probes' prepares as the column's type and whether it is never NULL.
fn resolve(shape: &Shape, outcomes: &[Option<MysqlDescribe>], plan: &crate::mysql_shape::Plan, flows: &[Vec<u8>], documents: &[Vec<Option<Value>>]) -> (Type, bool) {
    match shape {
        Shape::Object { fields, non_null } => {
            let fields = fields
                .iter()
                .map(|(k, s)| {
                    let (ty, nn) = resolve(s, outcomes, plan, flows, documents);
                    Field { name: k.clone(), ty, nullable: !nn }
                })
                .collect();
            (Type::Object { fields }, *non_null)
        }
        Shape::Array { items, non_null } => {
            let (ty, nn) = resolve(items, outcomes, plan, flows, documents);
            (Type::List { of: Box::new(ty), nullable_items: !nn }, *non_null)
        }
        Shape::Column { probe, index } => match outcomes.get(*probe).and_then(|o| o.as_ref()).and_then(|o| o.columns.get(*index)) {
            Some(c) if plan.booleans.contains(&(*probe, *index)) => (Type::Scalar { name: "bool".into() }, c.flags & 1 != 0),
            // A truth value is an integer to the server, so no other type is in doubt.
            Some(c) => {
                let ty = column_type(c.r#type, c.charset).0;
                let ty = flows.get(*probe).and_then(|f| f.get(*index)).and_then(|reached| widened(&ty, *reached)).unwrap_or(ty);
                // A JSON column a CHECK holds to a JSON Schema: the values it admits.
                let ty = match (ty, documents.get(*probe).and_then(|d| d.get(*index)).cloned().flatten()) {
                    (Type::Json { schema: None }, Some(schema)) => Type::Json { schema: Some(schema) },
                    (ty, _) => ty,
                };
                match ty {
                    Type::Scalar { name } if plan.either.contains(&(*probe, *index)) && name.split('|').any(|n| matches!(n, "int2" | "int4" | "int8")) => (Type::Scalar { name: format!("bool|{name}") }, c.flags & 1 != 0),
                    ty => (ty, c.flags & 1 != 0),
                }
            }
            None => (Type::Unknown, false),
        },
        _ => (Type::Unknown, false),
    }
}

// ---- the row count ------------------------------------------------------------------------------

/// The unique keys of the tables a statement reads.
#[derive(Debug, Default)]
pub(crate) struct Keys {
    /// Per table, the column lists of its unique keys.
    of: HashMap<String, Vec<Vec<String>>>,
    /// Per table, what each key column's type compares as.
    class: HashMap<String, HashMap<String, Class>>,
    /// Per table and view, its columns in order, each with what it compares as.
    columns: HashMap<String, Vec<(String, Class)>>,
    /// Per view, its defining SELECT, which names its tables with the view's schema.
    views: HashMap<String, Query>,
    /// The schema the views are in, which their statements qualify each name with.
    pub(crate) schema: Option<String>,
}

/// What a value compares as. MySQL compares a string with a number as two doubles, so `'01'`,
/// `'1'` and `'1x'` all equal `1`, and a key holds only where both sides are of one class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Class {
    Text,
    /// An integer or a decimal, which compare exactly.
    Number,
    /// A FLOAT or a DOUBLE column. An exact number compared with one compares as a double, where
    /// two integers past 2^53 are one value, so it pins no exact key. A constant of the kind is
    /// converted to the key's own type first, and is a number here.
    Real,
    /// A date or time, which a constant is converted to, or anything else.
    Other,
    /// A TIMESTAMP, which a session shows as its own zone's wall-clock time. Where the clocks go
    /// back an hour two instants show as one time, and the server compares a constant with the
    /// column by that time unless it looks the constant up in the index (it does not under
    /// IGNORE INDEX, or for an invisible index): no comparison is known to bound the rows.
    Instant,
}

/// The columns of `tables`, each table by its name as given: a table the server has under
/// another case answers to none of them.
fn table_columns(host: &Host, tables: &[String]) -> Lazy<crate::mysql_shape::Columns> {
    let rows = host.query(TABLE_COLUMNS_QUERY, vec![json!(tables.join(","))], Scope::Statement)?;
    let mut columns = crate::mysql_shape::Columns::new();
    for row in rows.unwrap_or_default() {
        if let (Some(table), Some(column)) = (row.first().and_then(as_str), row.get(1).and_then(as_str)) {
            columns.entry(table.to_string()).or_default().insert(column.to_ascii_lowercase());
        }
    }
    Some(columns)
}

/// Per result column, the canonical schema of the values a CHECK's `json_schema_valid` lets into
/// the JSON column it reads ([`crate::schema_type`]), where the prepare's origin of the column is
/// the column itself ([`traced_tables`]).
fn documents(statement: Option<&Statement>, raw: &MysqlDescribe, host: &Host) -> Lazy<Vec<Option<Value>>> {
    let none = || Some(vec![None; raw.columns.len()]);
    let Some(Statement::Query(q)) = statement else { return none() };
    let Some(tables) = traced_tables(q) else { return none() };
    let wanted: Vec<(usize, String, String)> = raw
        .columns
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let (table, column) = (c.org_table.as_deref()?, c.org_name.as_deref()?);
            let read = c.r#type == 245 && !column.is_empty() && tables.iter().any(|t| t.eq_ignore_ascii_case(table));
            read.then(|| (i, table.to_ascii_lowercase(), column.to_ascii_lowercase()))
        })
        .collect();
    if wanted.is_empty() {
        return none();
    }
    let mut names: Vec<String> = wanted.iter().map(|(_, table, _)| table.clone()).collect();
    names.sort();
    names.dedup();
    let rows = host.query(CHECKS_QUERY, vec![json!(names.join(","))], Scope::Statement)?;
    let mut schemas: HashMap<(String, String), Vec<(String, &[&crate::schema_type::Validator])>> = HashMap::new();
    for row in rows.unwrap_or_default() {
        let (Some(table), Some(clause)) = (row.first().and_then(as_str), row.get(1).and_then(as_str)) else { continue };
        for (column, schema) in check_documents(clause) {
            schemas.entry((table.to_ascii_lowercase(), column.to_ascii_lowercase())).or_default().push((schema, &[&crate::schema_type::MYSQL]));
        }
    }
    let mut out = vec![None; raw.columns.len()];
    for (i, table, column) in wanted {
        if let Some(texts) = schemas.get(&(table, column)) {
            out[i] = crate::schema_type::checked_type(texts);
        }
    }
    Some(out)
}

/// The tables of a SELECT whose columns' origins in the prepare are the columns themselves: one
/// with no WITH and no set operation, every item a column or a `*`, over tables named without a
/// schema. Through a derived table or a WITH query MySQL gives the base table and the derived
/// column's own name as a column's origin (`select doc from (select other as doc from o) s` reads
/// `o.doc`), and an expression's origin is not its value's; a view's columns name the view, which
/// has no CHECK.
fn traced_tables(q: &Query) -> Option<Vec<String>> {
    fn column(e: &Expr) -> bool {
        match e {
            Expr::Identifier(_) | Expr::CompoundIdentifier(_) => true,
            Expr::Nested(inner) => column(inner),
            _ => false,
        }
    }
    if q.with.is_some() {
        return None;
    }
    let SetExpr::Select(select) = q.body.as_ref() else { return None };
    let items = select.projection.iter().all(|item| match item {
        SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => column(e),
        SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..) => true,
        _ => false,
    });
    if !items {
        return None;
    }
    let mut tables = Vec::new();
    for from in &select.from {
        for factor in std::iter::once(&from.relation).chain(from.joins.iter().map(|j| &j.relation)) {
            match factor {
                TableFactor::Table { name, args: None, .. } if name.0.len() == 1 => tables.push(name.0[0].as_ident()?.value.clone()),
                _ => return None,
            }
        }
    }
    Some(tables)
}

/// `CHECK_CLAUSE` as the CHECK was written. `information_schema` hands its text back double
/// encoded, each byte of its UTF-8 as the character of that number, and with one more level of
/// backslash escapes than the server's own reprint. `None` where that does not read back, or
/// where a string holds `\a` or `\f`, which `sqlparser` reads as control characters and MySQL as
/// the letters.
fn check_clause(text: &str) -> Option<String> {
    let text = if text.chars().all(|c| (c as u32) <= 0xFF) { String::from_utf8(text.chars().map(|c| c as u8).collect()).ok()? } else { text.to_string() };
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        out.push(if c == '\\' { chars.next()? } else { c });
    }
    let mut chars = out.chars();
    while let Some(c) = chars.next() {
        if c == '\\' && matches!(chars.next(), Some('a' | 'f') | None) {
            return None;
        }
    }
    Some(out)
}

/// The JSON Schema text each `json_schema_valid('…', column)` of a CHECK holds its column to, as
/// `(column, schema)`: the call alone, a conjunct of an AND (each one holds), or beside `column
/// is null` under an OR, which only lets NULL through as well. The function is NULL for a NULL
/// document and true or false for any other, so a row the CHECK lets in holds a value the schema
/// admits. `clause` is `CHECK_CLAUSE` ([`check_clause`]).
fn check_documents(clause: &str) -> Vec<(String, String)> {
    fn column(e: &Expr) -> Option<String> {
        match e {
            Expr::Identifier(i) => Some(i.value.clone()),
            Expr::Nested(inner) => column(inner),
            _ => None,
        }
    }
    fn string(e: &Expr) -> Option<String> {
        match e {
            Expr::Value(v) => match &v.value {
                ast::Value::SingleQuotedString(s) => Some(s.clone()),
                _ => None,
            },
            Expr::Prefixed { value, .. } => string(value),
            Expr::Nested(inner) => string(inner),
            Expr::Cast { expr, data_type: ast::DataType::JSON, .. } => string(expr),
            _ => None,
        }
    }
    fn validates(e: &Expr) -> Option<(String, String)> {
        let Expr::Function(f) = e else { return None };
        let name = f.name.0.last()?.as_ident()?;
        if !name.value.eq_ignore_ascii_case("json_schema_valid") {
            return None;
        }
        let ast::FunctionArguments::List(list) = &f.args else { return None };
        let [FunctionArg::Unnamed(FunctionArgExpr::Expr(schema)), FunctionArg::Unnamed(FunctionArgExpr::Expr(document))] = list.args.as_slice() else { return None };
        Some((column(document)?, string(schema)?))
    }
    fn conjuncts(e: &Expr, only: Option<&str>, out: &mut Vec<(String, String)>) {
        let mine = |column: &str| only.is_none_or(|o| o.eq_ignore_ascii_case(column));
        match e {
            Expr::Nested(inner) => conjuncts(inner, only, out),
            Expr::BinaryOp { left, op: ast::BinaryOperator::And, right } => {
                conjuncts(left, only, out);
                conjuncts(right, only, out);
            }
            Expr::BinaryOp { left, op: ast::BinaryOperator::Or, right } => {
                fn bare(e: &Expr) -> &Expr {
                    match e {
                        Expr::Nested(inner) => bare(inner),
                        other => other,
                    }
                }
                for (null, other) in [(left, right), (right, left)] {
                    if let Expr::IsNull(tested) = bare(null) {
                        if let Some(c) = column(tested).filter(|c| mine(c)) {
                            conjuncts(other, Some(&c), out);
                        }
                    }
                }
            }
            _ => {
                if let Some((c, schema)) = validates(e).filter(|(c, _)| mine(c)) {
                    out.push((c, schema));
                }
            }
        }
    }
    let Some(text) = check_clause(clause) else { return Vec::new() };
    let Ok(expr) = Parser::new(&MySqlDialect {}).try_with_sql(&text).and_then(|mut p| p.parse_expr()) else { return Vec::new() };
    let mut out = Vec::new();
    conjuncts(&expr, None, &mut out);
    out
}

/// The unique keys, columns and views of every table a query reads, and of every table those
/// views read in turn: a round for each level of views.
fn catalog(host: &Host, q: &Query) -> Lazy<Keys> {
    let mut keys = Keys::default();
    let mut names = crate::mysql_keys::tables_anywhere(q, None);
    let mut seen: HashSet<String> = names.iter().map(|n| n.to_ascii_lowercase()).collect();
    while !names.is_empty() {
        let joined = json!(names.join(","));
        let found = unique_keys(host, &names);
        let columns = host.query(KEY_COLUMNS_QUERY, vec![joined.clone()], Scope::Statement);
        let views = host.query(VIEW_DEFINITIONS_QUERY, vec![joined], Scope::Statement);
        let (Some(found), Some(columns), Some(views)) = (found, columns, views) else { return None };
        keys.of.extend(found.of);
        keys.class.extend(found.class);
        for row in columns.unwrap_or_default() {
            if let (Some(table), Some(column), Some(ty)) = (row.first().and_then(as_str), row.get(1).and_then(as_str), row.get(2).and_then(as_str)) {
                keys.columns.entry(table.to_string()).or_default().push((column.to_ascii_lowercase(), class_of(ty)));
            }
        }
        let mut next = Vec::new();
        for row in views.unwrap_or_default() {
            let (Some(name), Some(definition), Some(schema)) = (row.first().and_then(as_str), row.get(1).and_then(as_str), row.get(2).and_then(as_str)) else { continue };
            // A definition the session may not read is empty, and reads as nothing.
            let Ok(mut parsed) = Parser::parse_sql(&MySqlDialect {}, definition) else { continue };
            let Some(Statement::Query(view)) = parsed.pop() else { continue };
            keys.schema = Some(schema.to_string());
            for table in crate::mysql_keys::tables_anywhere(&view, Some(schema)) {
                if seen.insert(table.to_ascii_lowercase()) {
                    next.push(table);
                }
            }
            keys.views.insert(name.to_string(), *view);
        }
        names = next;
    }
    Some(keys)
}

fn unique_keys(host: &Host, tables: &[String]) -> Lazy<Keys> {
    let rows = host.query(UNIQUE_KEYS_QUERY, vec![json!(tables.join(","))], Scope::Statement)?;
    Some(keys_of(rows.unwrap_or_default()))
}

/// The keys in [`UNIQUE_KEYS_QUERY`]'s rows. An index with a key part over an expression is no
/// key of its columns alone, and is left out whole.
fn keys_of(rows: Vec<Vec<Value>>) -> Keys {
    let mut keys = Keys::default();
    let mut by_index: Vec<(String, String, Option<Vec<String>>)> = Vec::new();
    for row in rows {
        let (Some(table), Some(index)) = (row.first().and_then(as_str), row.get(1).and_then(as_str)) else { continue };
        let column = row.get(2).and_then(as_str).map(str::to_ascii_lowercase);
        if let (Some(column), Some(ty)) = (&column, row.get(3).and_then(as_str)) {
            keys.class.entry(table.to_string()).or_default().insert(column.clone(), class_of(ty));
        }
        match by_index.last_mut() {
            Some((t, i, cols)) if t == table && i.eq_ignore_ascii_case(index) => match (cols.as_mut(), column) {
                (Some(cols), Some(column)) => cols.push(column),
                _ => *cols = None,
            },
            _ => by_index.push((table.to_string(), index.to_string(), column.map(|c| vec![c]))),
        }
    }
    for (table, _, cols) in by_index {
        // A table with no key left still answers to its name.
        let of = keys.of.entry(table).or_default();
        if let Some(cols) = cols {
            of.push(cols);
        }
    }
    keys
}

impl Keys {
    /// The server's name for the table a statement names. The catalog query compares names as
    /// the server does, so a row under another case than the statement's came back because the
    /// server holds the two for one table (`lower_case_table_names` 1 or 2); where it tells them
    /// apart (0) only the exact name comes back, and `Users` never takes the keys of `users`.
    fn name(&self, table: &str) -> Option<&str> {
        if let Some((name, _)) = self.of.get_key_value(table) {
            return Some(name);
        }
        let mut same = self.of.keys().filter(|name| name.eq_ignore_ascii_case(table));
        match (same.next(), same.next()) {
            (Some(name), None) => Some(name),
            _ => None,
        }
    }

    pub(crate) fn keys(&self, table: &str) -> Option<&Vec<Vec<String>>> {
        self.of.get(self.name(table)?).filter(|keys| !keys.is_empty())
    }

    /// The columns of a table or a view, in lower case, matched by name as [`Keys::name`] does.
    pub(crate) fn columns(&self, table: &str) -> Option<&Vec<(String, Class)>> {
        same_name(&self.columns, table)
    }

    /// What a column of a table compares as.
    pub(crate) fn class_of(&self, table: &str, column: &str) -> Option<Class> {
        if let Some(class) = self.name(table).and_then(|name| self.class.get(name)).and_then(|c| c.get(column)) {
            return Some(*class);
        }
        self.columns(table)?.iter().find(|(c, _)| c == column).map(|(_, class)| *class)
    }

    /// A view's defining SELECT.
    pub(crate) fn view(&self, name: &str) -> Option<&Query> {
        same_name(&self.views, name)
    }
}

/// The entry of `map` under `name`, or under the one name that differs from it in case alone.
fn same_name<'m, T>(map: &'m HashMap<String, T>, name: &str) -> Option<&'m T> {
    if let Some(found) = map.get(name) {
        return Some(found);
    }
    let mut same = map.iter().filter(|(n, _)| n.eq_ignore_ascii_case(name));
    match (same.next(), same.next()) {
        (Some((_, found)), None) => Some(found),
        _ => None,
    }
}

/// What a column of this `information_schema` data type compares as.
fn class_of(data_type: &str) -> Class {
    match data_type.to_ascii_lowercase().as_str() {
        "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" | "enum" | "set" | "binary" | "varbinary" | "tinyblob" | "blob" | "mediumblob" | "longblob" => Class::Text,
        "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint" | "decimal" | "numeric" | "bit" | "year" => Class::Number,
        "float" | "double" | "real" => Class::Real,
        "timestamp" => Class::Instant,
        _ => Class::Other,
    }
}

fn cardinality(q: &Query, keys: &Keys) -> (String, Option<String>) {
    match exactly_one(q) {
        Ok(()) => ("one".into(), None),
        Err(one_why) => match crate::mysql_keys::at_most_one(q, keys) {
            Ok(()) => ("maybeOne".into(), Some(one_why)),
            Err(why) => ("many".into(), Some(why)),
        },
    }
}

pub(crate) fn select_of(q: &Query) -> Result<&Select, String> {
    match q.body.as_ref() {
        SetExpr::Select(s) => Ok(s),
        SetExpr::SetOperation { .. } => Err("a set operation (UNION, INTERSECT, EXCEPT) returns any number of rows".into()),
        SetExpr::Values(_) => Err("a VALUES list returns a row per row it lists".into()),
        _ => Err("only a plain SELECT is proven".into()),
    }
}

pub(crate) fn const_limit(q: &Query) -> Option<Option<i64>> {
    let limit = match &q.limit_clause {
        Some(LimitClause::LimitOffset { limit: Some(l), .. }) => l,
        Some(LimitClause::OffsetCommaLimit { limit, .. }) => limit,
        Some(LimitClause::LimitOffset { limit: None, .. }) | None => return None,
    };
    Some(match limit {
        Expr::Value(v) => match &v.value {
            ast::Value::Number(n, _) => n.parse::<i64>().ok(),
            _ => None,
        },
        _ => None,
    })
}

pub(crate) fn exactly_one(q: &Query) -> Result<(), String> {
    // A constant LIMIT of a row or more with no OFFSET keeps the one row.
    let keeps = match &q.limit_clause {
        None => true,
        Some(LimitClause::LimitOffset { offset: None, limit_by, .. }) if limit_by.is_empty() => const_limit(q).is_none_or(|n| n.is_some_and(|n| n >= 1)),
        Some(_) => false,
    };
    if !keeps {
        return Err("a LIMIT or an OFFSET can remove the row (LIMIT 0, OFFSET 1)".into());
    }
    one_before_limit(q)
}

/// Exactly one row before the statement's LIMIT and OFFSET apply.
pub(crate) fn one_before_limit(q: &Query) -> Result<(), String> {
    if let SetExpr::Values(v) = q.body.as_ref() {
        return if v.rows.len() == 1 { Ok(()) } else { Err("a VALUES list returns a row per row it lists".into()) };
    }
    let s = select_of(q)?;
    if !matches!(s.group_by, GroupByExpr::Expressions(ref e, ref modifiers) if e.is_empty() && modifiers.is_empty()) {
        return Err("GROUP BY returns one row per group, and none when nothing matches".into());
    }
    if s.having.is_some() {
        return Err("HAVING can remove the row".into());
    }
    if s.projection.iter().any(|item| match item {
        SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => has_aggregate(e),
        _ => false,
    }) {
        return Ok(());
    }
    if s.from.is_empty() && s.selection.is_none() {
        return Ok(());
    }
    Err("a FROM returns a row per row it matches, and only an aggregate with no GROUP BY collapses them to one".into())
}

/// Whether a key column of this class compared with a value of this one compares the column as
/// it is: a placeholder the caller binds with the column's own type, or a value of the column's
/// class. A date or time column converts a constant to its own type. A FLOAT or DOUBLE key compared
/// with an exact number converts the number, and its own values stay apart.
pub(crate) fn compares_as_is(key: Option<Class>, value: Option<Class>) -> bool {
    match (key, value) {
        (Some(Class::Instant), _) => false,
        (_, None) | (None | Some(Class::Other), _) => true,
        (Some(Class::Real), Some(Class::Number)) => true,
        (Some(k), Some(v)) => k == v,
    }
}

/// What a compared value compares as, where the build can tell; `None` for a placeholder.
pub(crate) fn value_class(e: &Expr) -> Option<Class> {
    // `md5` and `sha1` are not here: MySQL 9.7 and 26.7 have neither, and there a stored function
    // of the name answers the call. The same holds in `deterministic`.
    const TEXT: &[&str] = &[
        "lower", "upper", "lcase", "ucase", "trim", "ltrim", "rtrim", "concat", "concat_ws", "substring", "substr", "replace", "left", "right", "lpad", "rpad",
        "sha2", "hex", "to_base64", "from_base64", "date_format", "bin_to_uuid",
    ];
    const NUMBER: &[&str] = &["abs", "round", "floor", "ceil", "ceiling", "length", "char_length", "character_length"];
    Some(match e {
        Expr::Value(v) => match &v.value {
            ast::Value::Placeholder(_) => return None,
            ast::Value::Number(..) | ast::Value::Boolean(_) => Class::Number,
            ast::Value::SingleQuotedString(_) | ast::Value::DoubleQuotedString(_) | ast::Value::NationalStringLiteral(_) | ast::Value::HexStringLiteral(_) => Class::Text,
            _ => Class::Other,
        },
        Expr::Nested(inner) => return value_class(inner),
        // `-x`, `+x`, `NOT x` and `~x` are numbers.
        Expr::UnaryOp { .. } => Class::Number,
        Expr::BinaryOp { op: ast::BinaryOperator::Plus | ast::BinaryOperator::Minus | ast::BinaryOperator::Multiply | ast::BinaryOperator::Divide | ast::BinaryOperator::Modulo | ast::BinaryOperator::MyIntegerDivide, .. } => Class::Number,
        // `a || b` where it joins strings (PIPES_AS_CONCAT; elsewhere the tree is not read).
        Expr::BinaryOp { op: ast::BinaryOperator::StringConcat, .. } => Class::Text,
        Expr::Function(f) => {
            let name = builtin(f);
            if TEXT.contains(&name.as_str()) {
                Class::Text
            } else if NUMBER.contains(&name.as_str()) {
                Class::Number
            } else {
                Class::Other
            }
        }
        Expr::Cast { data_type, .. } => {
            let target = data_type.to_string().to_ascii_lowercase();
            let starts = |prefixes: &[&str]| prefixes.iter().any(|p| target.starts_with(p));
            if starts(&["char", "nchar", "varchar", "binary", "text"]) {
                Class::Text
            } else if starts(&["signed", "unsigned", "int", "bigint", "decimal", "double", "float", "real", "numeric"]) {
                Class::Number
            } else {
                Class::Other
            }
        }
        _ => Class::Other,
    })
}

/// Whether an expression gives every row the same value: a literal, or built-ins known to answer
/// the same for the same arguments. A stored function may be NOT DETERMINISTIC, and so may any
/// name not listed, which counts as changing.
pub(crate) fn deterministic(e: &Expr) -> bool {
    const KNOWN: &[&str] = &[
        "lower", "upper", "lcase", "ucase", "trim", "ltrim", "rtrim", "concat", "concat_ws", "abs", "round", "floor", "ceil", "ceiling", "length",
        "char_length", "character_length", "substring", "substr", "replace", "left", "right", "lpad", "rpad", "coalesce", "ifnull", "nullif", "if",
        "sha2", "hex", "unhex", "to_base64", "from_base64", "date_format", "str_to_date", "from_unixtime", "uuid_to_bin", "bin_to_uuid",
    ];
    match e {
        Expr::Value(_) | Expr::TypedString { .. } => true,
        Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } | Expr::Cast { expr: inner, .. } => deterministic(inner),
        Expr::BinaryOp { left, right, .. } => deterministic(left) && deterministic(right),
        Expr::Function(f) => {
            let name = builtin(f);
            KNOWN.contains(&name.as_str())
                && match &f.args {
                    ast::FunctionArguments::None => true,
                    ast::FunctionArguments::List(list) => list.args.iter().all(|a| match a {
                        ast::FunctionArg::Unnamed(ast::FunctionArgExpr::Expr(x)) | ast::FunctionArg::Named { arg: ast::FunctionArgExpr::Expr(x), .. } | ast::FunctionArg::ExprNamed { arg: ast::FunctionArgExpr::Expr(x), .. } => deterministic(x),
                        _ => false,
                    }),
                    ast::FunctionArguments::Subquery(_) => false,
                }
        }
        _ => false,
    }
}

/// Whether an expression reads a column anywhere; anything the walk does not know counts as reading one.
pub(crate) fn reads_column(e: &Expr) -> bool {
    match e {
        // A user variable is no value the build knows the type of, and may change from row to row.
        _ if is_variable(e) => true,
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => true,
        Expr::Value(_) | Expr::TypedString { .. } => false,
        Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } | Expr::Cast { expr: inner, .. } | Expr::IsNull(inner) | Expr::IsNotNull(inner) => reads_column(inner),
        Expr::BinaryOp { left, right, .. } => reads_column(left) || reads_column(right),
        Expr::Function(f) => match &f.args {
            ast::FunctionArguments::None => false,
            ast::FunctionArguments::List(list) => list.args.iter().any(|a| match a {
                ast::FunctionArg::Unnamed(ast::FunctionArgExpr::Expr(x)) | ast::FunctionArg::Named { arg: ast::FunctionArgExpr::Expr(x), .. } | ast::FunctionArg::ExprNamed { arg: ast::FunctionArgExpr::Expr(x), .. } => reads_column(x),
                _ => true,
            }),
            ast::FunctionArguments::Subquery(_) => true,
        },
        _ => true,
    }
}

pub(crate) const AGGREGATES: &[&str] = &["count", "sum", "avg", "min", "max", "group_concat", "json_arrayagg", "json_objectagg", "bit_and", "bit_or", "bit_xor", "std", "stddev", "stddev_pop", "stddev_samp", "var_pop", "var_samp", "variance"];

/// The name of the built-in function a call names, in lower case, or nothing for a name qualified
/// with a schema or written in backticks: the first is a stored function, whatever built-in shares
/// its name, and so is the second where the built-in is one the grammar names (`` `count`(x) ``,
/// `` `coalesce`(x) ``). A plain unqualified name is the built-in's wherever there is one.
pub(crate) fn builtin(f: &ast::Function) -> String {
    match f.name.0.as_slice() {
        [ast::ObjectNamePart::Identifier(only)] if only.quote_style.is_none() => only.value.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// Whether an expression calls an aggregate at this level: not under OVER, not inside a subquery.
pub(crate) fn has_aggregate(e: &Expr) -> bool {
    aggregate_where(e, &mut |_| true)
}

/// Whether an expression calls, at this level, an aggregate that `counts`.
pub(crate) fn aggregate_where(e: &Expr, counts: &mut dyn FnMut(&ast::Function) -> bool) -> bool {
    match e {
        Expr::Function(f) => {
            let name = builtin(f);
            if f.over.is_none() && AGGREGATES.contains(&name.as_str()) && counts(f) {
                return true;
            }
            match &f.args {
                ast::FunctionArguments::List(list) => list.args.iter().any(|a| match a {
                    ast::FunctionArg::Unnamed(ast::FunctionArgExpr::Expr(x)) | ast::FunctionArg::Named { arg: ast::FunctionArgExpr::Expr(x), .. } | ast::FunctionArg::ExprNamed { arg: ast::FunctionArgExpr::Expr(x), .. } => aggregate_where(x, counts),
                    _ => false,
                }),
                _ => false,
            }
        }
        Expr::Nested(inner) | Expr::UnaryOp { expr: inner, .. } | Expr::Cast { expr: inner, .. } | Expr::IsNull(inner) | Expr::IsNotNull(inner) => aggregate_where(inner, counts),
        Expr::BinaryOp { left, right, .. } => aggregate_where(left, counts) || aggregate_where(right, counts),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_json_schema_check_as_information_schema_prints_it() {
        // `CHECK_CLAUSE` as MySQL 8.4.11 printed the CHECKs of wiki/json-schema-checks/mysql/catalog.ts.
        let schema = r#"{"type":"object","properties":{"a":{"type":"string"}},"required":["a"]}"#;
        let at = |clause: &str| check_documents(clause).into_iter().map(|(c, s)| (c, serde_json::from_str::<Value>(&s).unwrap())).collect::<Vec<_>>();
        let doc = vec![("doc".to_string(), serde_json::from_str::<Value>(schema).unwrap())];
        assert_eq!(at(&format!(r"json_schema_valid(_utf8mb4\'{schema}\',`doc`)")), doc);
        assert_eq!(at(&format!(r"((`doc` is null) or json_schema_valid(_utf8mb4\'{schema}\',`doc`))")), doc);
        assert_eq!(at(&format!(r"(json_schema_valid(_utf8mb4\'{schema}\',`doc`) and (json_length(`doc`) < 5))")), doc);
        assert_eq!(at(&format!(r"json_schema_valid(cast(_utf8mb4\'{schema}\' as json),`doc`)")), doc);
        // A schema the CHECK builds, a call compared with a value, and an OR beside another
        // column's null test say nothing.
        assert_eq!(at(r"json_schema_valid(json_object(_utf8mb4\'type\',_utf8mb4\'object\'),`doc`)"), vec![]);
        assert_eq!(at(&format!(r"((`other` is null) or json_schema_valid(_utf8mb4\'{schema}\',`doc`))")), vec![]);
        assert_eq!(at(&format!(r"((`doc` = 1) or json_schema_valid(_utf8mb4\'{schema}\',`doc`))")), vec![]);
    }

    #[test]
    fn a_check_clause_reads_back_through_its_escapes_and_its_double_encoding() {
        // Quotes, backslashes, a JSON escape, non-ASCII and four-byte text, as 8.4.11 printed them.
        let catalog = include_str!("../../../wiki/json-schema-checks/mysql/out/catalog-mysql-8.4.txt");
        let line = catalog.lines().find_map(|l| l.strip_prefix("c_esc_default ")).unwrap();
        let clause: String = serde_json::from_str(line).unwrap();
        let found = check_documents(&clause);
        assert_eq!(found.len(), 1);
        let schema: Value = serde_json::from_str(&found[0].1).unwrap();
        assert_eq!(schema, json!({"enum": ["it's", "a\\b", "\u{e9}", "\u{e9}", "\"q\"", "\u{1f600}", "\u{20ac}", "t\tx", "%_"]}));
        // A string `sqlparser` would read otherwise than MySQL is not read.
        assert_eq!(check_clause(r"json_schema_valid(_utf8mb4\'\\f\',`doc`)"), None);
    }

    use super::*;

    fn keys() -> Keys {
        keys_of(
            [("users", "PRIMARY", "id", "int"), ("users", "email", "email", "varchar"), ("posts", "PRIMARY", "id", "bigint"), ("events", "at", "at", "datetime"), ("stamps", "at", "at", "timestamp")]
                .into_iter()
                .map(|(t, i, c, ty)| vec![json!(t), json!(i), json!(c), json!(ty)])
                .collect(),
        )
    }

    fn card(sql: &str) -> String {
        let mut v = Parser::parse_sql(&MySqlDialect {}, sql).unwrap();
        let Statement::Query(q) = v.pop().unwrap() else { panic!("not a query") };
        cardinality(&q, &keys()).0
    }

    #[test]
    fn joins_derived_tables_and_with_queries_are_pinned_by_keys() {
        let mut k = keys();
        for (table, columns) in [("users", &[("id", "int"), ("email", "varchar"), ("name", "varchar")][..]), ("posts", &[("id", "bigint"), ("user_id", "bigint"), ("title", "text")][..])] {
            k.columns.insert(table.into(), columns.iter().map(|(c, ty)| (c.to_string(), class_of(ty))).collect());
        }
        let rows = |sql: &str| {
            let mut v = Parser::parse_sql(&MySqlDialect {}, sql).unwrap();
            let Statement::Query(q) = v.pop().unwrap() else { panic!("not a query") };
            cardinality(&q, &k).0
        };
        for sql in [
            "select p.title, u.name from posts p join users u on u.id = p.user_id where p.id = ?",
            "select p.title, u.name from posts p left join users u on u.id = p.user_id where p.id = ?",
            "select p.title, u.name from posts p, users u where u.id = p.user_id and p.id = ?",
            "select * from (select id, title from posts where id = ?) s",
            "select * from (select id, title from posts) s where s.id = ?",
            "with s as (select id, title from posts) select * from s where s.id = ?",
            "select * from (select id as post_id from posts) s where s.post_id = ?",
            "select * from (select p.id, u.name from posts p join users u on u.id = p.user_id) s where s.id = ?",
            "select * from (select user_id, count(*) as n from posts group by user_id) s where s.user_id = ?",
            "select * from (select distinct user_id from posts) s where s.user_id = ?",
        ] {
            assert_eq!(rows(sql), "maybeOne", "{sql}");
        }
        for sql in [
            "select p.title, u.name from posts p join users u on u.id = p.user_id where u.id = ?",
            "select p.title from posts p join users u on u.email = p.title where p.id = ?",
            "select * from (select title from posts) s where s.title = ?",
            "select * from (select id + 0 as id from posts) s where s.id = ?",
            "select * from (select id from posts union all select id from posts) s where s.id = ?",
            "select * from (select p.id from posts p cross join users u) s where s.id = ?",
            "select * from (select user_id, count(*) as n from posts group by user_id with rollup) s where s.user_id = ?",
            "select p.id from posts p full outer join users u on u.id = p.user_id where p.id = ?",
        ] {
            assert_eq!(rows(sql), "many", "{sql}");
        }
    }

    #[test]
    fn placeholders_parse_and_keys_prove() {
        // A name qualified with a schema is that schema's: a stored function is no aggregate,
        // and a table there has keys of its own.
        assert_eq!(card("select app.count(id) as n from users"), "many");
        assert_eq!(card("select id from app.users where id = ?"), "many");
        assert_eq!(card("select id from users where id = app.lower(?)"), "many");
        assert_eq!(card("select id, name from users where id = ? and name <> ?"), "maybeOne");
        assert_eq!(card("select id from users where email = ?"), "maybeOne");
        assert_eq!(card("select id from users where name = ?"), "many");
        assert_eq!(card("select count(*) as n, max(at) as latest from users where name like ?"), "one");
        assert_eq!(card("select u.id, count(p.id) as n from users u left join posts p on p.user_id = u.id where u.id = ? group by u.id"), "maybeOne");
        assert_eq!(card("select id from users limit 1"), "maybeOne");
        assert_eq!(card("select id from users limit 5, 1"), "maybeOne");
        // A LIMIT above one, one from a parameter and an OFFSET remove rows and add none.
        assert_eq!(card("select id from users where id = ? limit 5"), "maybeOne");
        assert_eq!(card("select id from users where id = ? limit ? offset ?"), "maybeOne");
        assert_eq!(card("select id from users where name = ? limit 5"), "many");
        assert_eq!(card("select count(*) as n from users limit 5"), "one");
        assert_eq!(card("select count(*) as n from users limit 0"), "maybeOne");
        assert_eq!(card("select count(*) as n from users limit ?"), "maybeOne");
        assert_eq!(card("select count(*) as n from users limit 5 offset 1"), "maybeOne");
        assert_eq!(card("select count(*) as n from users limit 1, 5"), "maybeOne");
        assert_eq!(card("select id from users union all select user_id from posts"), "many");
        assert_eq!(card("select id from users where id = ? + 1"), "maybeOne");
        assert_eq!(card("select id from users u join posts p on p.user_id = u.id where u.id = p.user_id"), "many");
        // `RAND()` and `UUID()` are evaluated for each row, and a stored function may be too.
        assert_eq!(card("select id from users where id = floor(rand() * 3)"), "many");
        assert_eq!(card("select id from users where email = uuid()"), "many");
        assert_eq!(card("select id from users where email = my_function(?)"), "many");
        // A date or time key converts a constant to its own type, so only the determinism check
        // stops `SYSDATE()`, another value in each row.
        assert_eq!(card("select id from events where at = '2026-10-04 00:00:00'"), "maybeOne");
        assert_eq!(card("select id from events where at = sysdate()"), "many");
        assert_eq!(card("select id from users where email = lower(?)"), "maybeOne");
    }

    #[test]
    fn a_key_compared_across_string_and_number_bounds_nothing() {
        // MySQL compares a string with a number as two doubles: '01', '1' and '1x' all equal 1.
        assert_eq!(card("select id from users where email = 1"), "many");
        assert_eq!(card("select id from users where id = '1'"), "many");
        assert_eq!(card("select id from users where email = abs(?)"), "many");
        assert_eq!(card("select id from users where email = '1'"), "maybeOne");
        assert_eq!(card("select id from users where id = 1"), "maybeOne");
        assert_eq!(card("select id from users where id = cast(? as signed)"), "maybeOne");
        // A key part over an expression has no column: the index is no key of its columns.
        let functional = keys_of(vec![vec![json!("users"), json!("u"), json!("email"), json!("varchar")], vec![json!("users"), json!("u"), Value::Null, Value::Null]]);
        assert!(functional.keys("users").is_none());
    }

    #[test]
    fn a_key_bounds_nothing_where_the_server_may_not_use_it() {
        // A TIMESTAMP shows two instants as one wall-clock time where the clocks go back, and a
        // scan compares by that time.
        assert_eq!(card("select id from stamps where at = ?"), "many");
        assert_eq!(card("select id from stamps where at = '2026-11-01 01:30:00'"), "many");
        // WITH ROLLUP adds a row to the one group, and to the one row of an aggregate.
        assert_eq!(card("select id, count(*) as n from users where id = ? group by id with rollup"), "many");
        assert_eq!(card("select count(*) as n from users group by id with rollup limit 1"), "maybeOne");
        // A WITH query hides the table of its name, and has no key.
        assert_eq!(card("with users as (select 1 as id union all select 1) select id from users where id = 1"), "many");
        // A name in backticks is a stored function's where the grammar names the built-in.
        assert_eq!(card("select `count`(id) as n from users"), "many");
        assert_eq!(card("select id from users where email = `lower`(?)"), "many");
        // A user variable holds a value of any type: a number beside a string key compares as two doubles.
        assert_eq!(card("select id from users where email = @n"), "many");
        assert_eq!(card("select id from users where email = @`n`"), "many");
        assert_eq!(card("select id from users where email = concat(@`n`, '')"), "many");
    }

    #[test]
    fn a_table_takes_the_keys_the_server_holds_under_its_name() {
        let row = |t: &str, c: &str| vec![json!(t), json!("PRIMARY"), json!(c), json!("int")];
        // The server holds `Users` and `users` for two tables (lower_case_table_names 0) and
        // answers with the one named; where it holds them for one it answers with its own name.
        let exact = keys_of(vec![row("Users", "id")]);
        assert_eq!(exact.keys("Users"), Some(&vec![vec!["id".to_string()]]));
        assert_eq!(exact.keys("users"), Some(&vec![vec!["id".to_string()]]));
        let both = keys_of(vec![row("Users", "id"), row("users", "other")]);
        assert_eq!(both.keys("users"), Some(&vec![vec!["other".to_string()]]));
        assert_eq!(both.keys("Users"), Some(&vec![vec!["id".to_string()]]));
        assert!(both.keys("USERS").is_none());
    }

    /// Analyzes with a host that answers every prepare with `described`, the mode query with
    /// `mode` and any other query with no rows; says whether the mode was asked for.
    fn analyzed(sql: &str, described: Value, mode: &str) -> (Analysis, bool) {
        let mut answers: HashMap<String, crate::api::Answer> = HashMap::new();
        let mut asked = false;
        for _ in 0..8 {
            let host = Host::new(&answers);
            if let Some(analysis) = analyze_text(sql, &host) {
                return (analysis, asked);
            }
            for need in host.needs() {
                let (id, answer) = match need {
                    crate::api::Need::Describe { id, .. } => (id, json!({ "mysql": described })),
                    crate::api::Need::Query { id, sql, .. } if sql == SQL_MODE_QUERY => {
                        asked = true;
                        (id, json!({ "rows": [[mode]] }))
                    }
                    crate::api::Need::Query { id, sql, .. } if sql == VIEWS_QUERY || sql == VIEW_DEFINITIONS_QUERY => (id, json!({ "rows": [] })),
                    crate::api::Need::Query { id, sql, .. } if sql == KEY_COLUMNS_QUERY => (id, json!({ "rows": [["users", "id", "int"], ["users", "email", "varchar"], ["users", "name", "varchar"]] })),
                    crate::api::Need::Query { id, .. } => (id, json!({ "rows": [["users", "PRIMARY", "id", "int"], ["users", "email", "email", "varchar"]] })),
                };
                answers.insert(id, serde_json::from_value(answer).unwrap());
            }
        }
        panic!("the analysis never settled")
    }

    #[test]
    fn a_statement_the_server_reads_by_its_mode_is_read_by_it() {
        let one = json!({ "columns": [{ "name": "id", "type": 3, "flags": 1 }], "params": [] });
        let rows = |sql: &str, mode: &str| analyzed(sql, one.clone(), mode).0.cardinality;
        let default = "ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION";
        // `||` is OR unless PIPES_AS_CONCAT: `id = 1 || 1` is every row.
        assert_eq!(rows("select id from users where id = 1 || 1", default), "many");
        assert_eq!(rows("select id from users where id = 1 and name = 'a' || 'b'", default), "many");
        assert_eq!(rows("select id from users where id = 1 and name = 'a' || 'b'", "PIPES_AS_CONCAT"), "maybeOne");
        // A space before the parenthesis calls a stored function unless IGNORE_SPACE.
        assert_eq!(rows("select count (id) as id from users", default), "many");
        assert_eq!(rows("select count/* all */(id) as id from users", default), "many");
        assert_eq!(rows("select count /* all */ (id) as id from users", "IGNORE_SPACE"), "many");
        assert_eq!(rows("select count\n(id) as id from users", "IGNORE_SPACE"), "one");
        assert_eq!(rows("select count (id) as id from users", "ignore_space"), "one");
        assert_eq!(rows("select app.count (id) as id from users", default), "many");
        // A double-quoted word is a column under ANSI_QUOTES, a backslash a character under
        // NO_BACKSLASH_ESCAPES, and the parser reads both as the session does.
        assert_eq!(rows("select id from users where email = \"email\"", default), "maybeOne");
        assert_eq!(rows("select id from users where email = \"email\"", "ANSI_QUOTES"), "many");
        assert_eq!(rows("select \"id\" from \"users\" where \"id\" = 1", "ANSI_QUOTES"), "maybeOne");
        assert_eq!(rows("select id from users where email = 'a\\'' or 1 = 1 -- '", default), "many");
        // There the whole tail is one string: `'a\'' or 1 = 1 -- '`.
        assert_eq!(rows("select id from users where email = 'a\\'' or 1 = 1 -- '", "NO_BACKSLASH_ESCAPES"), "maybeOne");
        assert_eq!(rows("select id from users where email = 'a\\\\'", default), "maybeOne");
        assert_eq!(rows("select id from users where email = 'a\\\\'", "NO_BACKSLASH_ESCAPES"), "maybeOne");
        assert_eq!(rows("select id from users where email = 'C:\\' and id = 1", "NO_BACKSLASH_ESCAPES"), "maybeOne");
        // NOT binds tighter than a comparison under HIGH_NOT_PRECEDENCE.
        assert_eq!(rows("select id from users where id = 1 and not name = 'x'", default), "maybeOne");
        assert_eq!(rows("select id from users where id = 1 and not name = 'x'", "HIGH_NOT_PRECEDENCE"), "many");
        // A write returns no rows however it is read.
        let (write, _) = analyzed("update users set name = 'a' || 'b' where id = 1", json!({ "columns": [], "params": [] }), default);
        assert_eq!((write.kind.as_str(), write.cardinality.as_str()), ("update", "none"));
        assert_eq!(analyzed("select id from users where id = 1 || 1", one.clone(), default).0.kind, "select");
        // A hint may set the mode for the statement alone.
        assert_eq!(rows("select /*+ SET_VAR(sql_mode = 'PIPES_AS_CONCAT') */ id from users where id = 1", default), "many");
        // The mode is asked for only where the text turns on it.
        for sql in ["select id from users where id = 1 and name is not null", "select id from users where id = 1 and id not in (2) and name not like '||' and id != 3", "select count(id) as id from users"] {
            let (analysis, asked) = analyzed(sql, one.clone(), "HIGH_NOT_PRECEDENCE,ANSI_QUOTES");
            assert!(!asked, "{sql}");
            assert_ne!(analysis.cardinality, "many", "{sql}");
        }
        // A placeholder count the server does not share: the engine read the text otherwise.
        let miscounted = analyzed("select id from users where id = :id", json!({ "columns": [{ "name": "id", "type": 3, "flags": 1 }], "params": [{ "type": 8 }, { "type": 8 }] }), default).0;
        assert_eq!(miscounted.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), vec![1]);
        // The server runs what a version comment holds.
        let (analysis, _) = analyzed("select id from users /*!80000 where id = 1 */", one.clone(), default);
        assert_eq!(analysis.diagnostics.iter().map(|d| (d.code, d.start, d.length)).collect::<Vec<_>>(), vec![(10, 21, 24)]);
    }

    #[test]
    fn a_parameter_as_a_whole_sort_or_group_key_is_an_error_at_its_place() {
        let default = "ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION";
        let full = |sql: &str| {
            let n = lex::lex(sql, Dialect::Mysql).unwrap().placeholders.len();
            analyzed(sql, json!({ "columns": [{ "name": "id", "type": 3, "flags": 1 }], "params": vec![json!({ "type": 253 }); n] }), default).0
        };
        let keys = |sql: &str| full(sql).diagnostics.iter().filter(|d| d.code == 17).map(|d| (d.start, d.length, d.message.clone())).collect::<Vec<_>>();
        let at = |sql: &str, written: &str| sql.find(written).unwrap();
        // A `::type` cast is part of the parameter's place, as on every diagnostic at a cast one.
        for (sql, length) in [
            ("select id from users order by :sort", 5),
            ("select id from users order by :sort::text desc", 11),
            ("select id from users order by :sort::int", 10),
            ("select id from users order by cast(:sort as char)", 5),
            ("select id from users order by convert(:sort, char)", 5),
            ("select id from users order by :sort collate utf8mb4_bin", 5),
        ] {
            let found = keys(sql);
            assert_eq!(found.iter().map(|(s, l, _)| (*s, *l)).collect::<Vec<_>>(), [(at(sql, ":sort"), length)], "{sql}");
            assert_eq!(found[0].2, "order by :sort sorts every row by the same value, since a parameter is a value and not a column's name; choose the column with case :sort when 'name' then name end");
        }
        let sql = "select id from users order by name, ?";
        assert_eq!(keys(sql).iter().map(|(s, l, _)| (*s, *l)).collect::<Vec<_>>(), [(at(sql, "?"), 1)]);
        assert!(keys(sql)[0].2.starts_with("order by ? sorts every row"));
        // GROUP BY, alone and beside another key.
        assert_eq!(keys("select count(*) as n from users group by :g")[0].2, "group by :g puts every row in one group, since a parameter is a value and not a column's name; choose the column with case :g when 'name' then name end");
        assert!(keys("select count(*) as n from users group by name, :g")[0].2.starts_with("group by :g groups every row by the same value"));
        // A window's keys, a named window's, an aggregate's ORDER BY, and a write's.
        let sql = "select row_number() over (partition by :p order by :o) as n from users";
        let found = keys(sql);
        assert_eq!(found.iter().map(|(s, _, m)| (*s, m.split(',').next().unwrap().to_string())).collect::<Vec<_>>(), [(at(sql, ":p"), "partition by :p puts every row in one partition".to_string()), (at(sql, ":o"), "order by :o sorts every row by the same value".to_string())]);
        assert_eq!(keys("select row_number() over w as n from users window w as (partition by :p)").len(), 1);
        assert_eq!(keys("select group_concat(name order by :o separator ',') as names from users").len(), 1);
        assert_eq!(keys("delete from users order by :o limit 10").len(), 1);
        // In a subquery, a WITH query and a set operation's ORDER BY.
        assert_eq!(keys("select id from (select id from users order by :s limit 3) t").len(), 1);
        assert_eq!(keys("with t as (select name from users group by :g, name) select name from t").len(), 1);
        assert_eq!(keys("select id from users where id in (select id from (select id from users order by :s limit 3) x)").len(), 1);
        assert_eq!(keys("select id from users union select id from users order by :s").len(), 1);
        // A key that holds a parameter chooses a column with it; a position and a LIMIT are none.
        for sql in [
            "select id, name from users order by case :sort when 'name' then name end",
            "select id from users order by case when :desc then id end desc, id",
            "select id from users order by 1 limit :n",
            "select id, :tag as tag from users order by 2",
            "select id, :tag as tag from users order by tag",
            "select id from users order by concat(lower(:x), name)",
        ] {
            assert_eq!(keys(sql), [], "{sql}");
        }
        // The rest of the statement is typed all the same.
        assert_eq!(full("select id from users order by :sort").columns.len(), 1);
    }

    #[test]
    fn statements_nest_to_the_engines_bound() {
        // sqlparser's own bound is 50; the engine's is 200 levels of its recursion, which a
        // parenthesized operand (`id = 0 or (…)`) and a subquery each spend two of, and 500
        // operators in all. A statement past either is an error to parse, left to the server's
        // describe, never an overflow of the stack.
        let deepest = |shape: &str| crate::stack::run(|| (1..1_000).take_while(|&n| parse(&MySqlDialect {}, &crate::stack::nested(shape, n)).is_ok()).last().unwrap_or(0));
        let found: Vec<(&str, usize)> = ["sum", "and_or", "case", "coalesce", "subquery"].into_iter().map(|shape| (shape, deepest(shape))).collect();
        assert_eq!(found, vec![("sum", 501), ("and_or", 98), ("case", 195), ("coalesce", 196), ("subquery", 97)]);
        for shape in ["sum", "case", "or", "and_or"] {
            assert!(crate::stack::run(|| parse(&MySqlDialect {}, &crate::stack::nested(shape, 100_000)).is_err()), "{shape}");
        }
        // A whole analysis at the bound and far past it, which used to overflow the stack.
        let described = json!({ "columns": [{ "name": "x", "type": 8, "flags": 1 }], "params": [] });
        for n in [501, 100_000] {
            let (analysis, _) = crate::stack::run(|| analyzed(&crate::stack::nested("sum", n), described.clone(), "STRICT_TRANS_TABLES"));
            assert_eq!(analysis.columns.len(), 1, "{n}");
        }
    }

    #[test]
    fn compound_interval_units_are_read() {
        // Every compound unit of MySQL, in an INTERVAL and in EXTRACT, read as its smallest part.
        let units = [
            ("second_microsecond", "MICROSECOND"),
            ("minute_microsecond", "MICROSECOND"),
            ("minute_second", "SECOND"),
            ("hour_microsecond", "MICROSECOND"),
            ("hour_second", "SECOND"),
            ("hour_minute", "MINUTE"),
            ("day_microsecond", "MICROSECOND"),
            ("day_second", "SECOND"),
            ("day_minute", "MINUTE"),
            ("day_hour", "HOUR"),
            ("YEAR_MONTH", "MONTH"),
        ];
        for (unit, read) in units {
            let sql = format!("select date_add(d, interval '1' {unit}), extract({unit} from d) from t where id = ?");
            let tree = parse(&MySqlDialect {}, &sql).unwrap_or_else(|e| panic!("{sql}: {e}")).pop().unwrap().to_string();
            assert_eq!(tree, format!("SELECT date_add(d, INTERVAL '1' {read}), EXTRACT({read} FROM d) FROM t WHERE id = ?"));
        }
        // A quoted name is a name.
        assert_eq!(parse(&MySqlDialect {}, "select `day_hour` from t").unwrap().pop().unwrap().to_string(), "SELECT `day_hour` FROM t");
        // Read, the statement is proven: one row without FROM, at most one by the key.
        let default = "ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION";
        let text = json!({ "columns": [{ "name": "x", "type": 253, "flags": 0 }], "params": [] });
        assert_eq!(analyzed("select date_add('2100-12-31 23:59:59', interval '1:1' minute_second)", text, default).0.cardinality, "one");
        let at = json!({ "columns": [{ "name": "x", "type": 12, "flags": 0 }], "params": [{ "type": 3 }] });
        assert_eq!(analyzed("select now() - interval '1 1' day_hour as x from users where id = :id", at, default).0.cardinality, "maybeOne");
    }

    #[test]
    fn a_table_may_be_named_local_or_directory() {
        // Hive's `INSERT OVERWRITE [LOCAL] DIRECTORY` took both words in upstream's sqlparser,
        // which refused these; the engine's copy reads them as MySQL does (vendor/sqlparser).
        for sql in ["insert into local values (1)", "insert into directory (id) values (1)"] {
            assert!(parse(&MySqlDialect {}, sql).is_ok(), "{sql}");
        }
    }

    #[test]
    fn a_chain_of_joins_is_read_in_one_pass() {
        // A join with no ON or USING of its own is the left operand of the joins after it, and
        // nests them only where an ON or USING is left over for it.
        let printed = |sql: &str| parse(&MySqlDialect {}, sql).unwrap_or_else(|e| panic!("{sql}: {e}")).pop().unwrap().to_string();
        assert_eq!(printed("SELECT * FROM a JOIN b RIGHT JOIN c ON x"), "SELECT * FROM a JOIN b RIGHT JOIN c ON x");
        assert_eq!(printed("SELECT * FROM a JOIN b JOIN c ON x ON y"), "SELECT * FROM a JOIN (b JOIN c ON x) ON y");
        // The joins read after a bare one are kept, never read again, which made a chain of bare
        // joins exponential to parse: 20 of them took a second. 61 tables are MySQL's limit.
        let bare = |n: usize| format!("SELECT * FROM t0{}", (1..n).map(|i| format!(" JOIN t{i}")).collect::<String>());
        let started = std::time::Instant::now();
        assert_eq!(printed(&bare(21)), bare(21));
        assert!(started.elapsed() < std::time::Duration::from_secs(1), "{:?}", started.elapsed());
        for sql in [bare(61), format!("{} ON t0.id = t60.id", bare(61))] {
            assert_eq!(crate::stack::run(|| printed(&sql)), sql);
        }
        // Every ON at the end: each join nests the rest.
        let ons = (1..61).rev().map(|i| format!(" ON t{}.id = t{i}.id", i - 1)).collect::<String>();
        let nested = (1..60).rev().fold("t60".to_string(), |inner, i| format!("(t{i} JOIN {inner} ON t{i}.id = t{}.id)", i + 1));
        assert_eq!(crate::stack::run(|| printed(&format!("{}{ons}", bare(61)))), format!("SELECT * FROM t0 JOIN {nested} ON t0.id = t1.id"));
    }

    #[test]
    fn bang_binds_tighter_than_any_binary_operator_but_the_arrows() {
        // `->` and `->>` are part of `!`'s operand, as in MySQL, and every other binary operator
        // takes `!x` as its operand.
        let item = |sql: &str| match parse(&MySqlDialect {}, sql).unwrap_or_else(|e| panic!("{sql}: {e}")).pop() {
            Some(Statement::Query(q)) => match *q.body {
                SetExpr::Select(s) => match s.projection.into_iter().next() {
                    Some(SelectItem::UnnamedExpr(e)) => e,
                    other => panic!("{sql}: {other:?}"),
                },
                other => panic!("{sql}: {other:?}"),
            },
            other => panic!("{sql}: {other:?}"),
        };
        for sql in ["select !j->'$.a' from t", "select !j->>'$.a' from t"] {
            assert!(matches!(item(sql), Expr::UnaryOp { op: ast::UnaryOperator::BangNot, expr } if matches!(*expr, Expr::BinaryOp { .. })), "{sql}");
        }
        for sql in ["select !0 = 1", "select !a + 1 from t", "select !a ^ 1 from t"] {
            assert!(matches!(item(sql), Expr::BinaryOp { left, .. } if matches!(*left, Expr::UnaryOp { op: ast::UnaryOperator::BangNot, .. })), "{sql}");
        }
    }

    #[test]
    fn a_write_or_a_select_into_returns_no_rows() {
        // MySQL's multi-table DELETE and UPDATE return no rows, and neither does a SELECT whose
        // rows go into variables, its INTO after the columns or after the whole statement.
        let default = "ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION";
        let none = json!({ "columns": [], "params": [{ "type": 3 }] });
        for sql in [
            "delete u.*, e.* from users u join emails e on e.user_id = u.id where u.id = :id",
            "update users, emails set users.name = 'a', emails.email = 'b' where users.id = :id",
            "select id from users where id = :id into @found",
            "select id into @found from users where id = :id",
            "select id from users where id = :id for update into @found",
        ] {
            assert!(parse(&MySqlDialect {}, sql).is_ok(), "{sql}");
            let analysis = analyzed(sql, none.clone(), default).0;
            assert_eq!((analysis.cardinality.as_str(), analysis.columns.len(), analysis.params.len()), ("none", 0, 1), "{sql}");
        }
        // A CALL the parser does not read returns whatever its procedure makes.
        let call = format!("call report(:id{})", " + 1".repeat(600));
        assert!(parse(&MySqlDialect {}, &call).is_err());
        assert_eq!(analyzed(&call, none.clone(), default).0.cardinality, "many");
    }

    #[test]
    fn the_session_dialect_is_mysqls_but_for_strings_and_double_quotes() {
        // In the default reading the tree is `MySqlDialect`'s, statement for statement.
        for sql in [
            "select a.id, count(*) div 2, x << 1 from t a straight_join u b on a.id = b.id where a.n && 1 group by a.id with rollup limit 1, 2",
            "select 'it\\'s', \"say \\\"hi\\\"\", `n`, binary name, _utf8mb4'x', 0x1f, x'1f', b'1', date '2026-01-01' + interval 1 day from t use index (k) for update",
            "insert into t set a = 1, b = 'x' on duplicate key update b = values(b)",
            "update t set a = a + 1 where id = ? order by id limit 1",
            "select /*+ max_execution_time(10) */ id from t where match (name) against ('x' in boolean mode) -- c",
            "select json_object('k', json_arrayagg(id)) from t # c",
        ] {
            let mysql = Parser::parse_sql(&MySqlDialect {}, sql).map(|v| format!("{v:?}"));
            assert_eq!(Parser::parse_sql(&SessionDialect(MysqlText::default()), sql).map(|v| format!("{v:?}")), mysql, "{sql}");
        }
        let one = |sql: &str, text: MysqlText| {
            sqlparser::ast::set_backslash_escapes(text.backslash_escapes);
            Parser::parse_sql(&SessionDialect(text), sql).unwrap().pop().unwrap().to_string()
        };
        let plain = MysqlText { backslash_escapes: false, ansi_quotes: false };
        let ansi = MysqlText { backslash_escapes: true, ansi_quotes: true };
        assert_eq!(one(r"select 'C:\' as p, id from t where id = ?", plain), r"SELECT 'C:\' AS p, id FROM t WHERE id = ?");
        assert!(Parser::parse_sql(&SessionDialect(MysqlText::default()), r"select 'C:\' as p, id from t where id = ?").is_err());
        assert_eq!(one(r#"select "id" from "t" where "id" = 1"#, ansi), r#"SELECT "id" FROM "t" WHERE "id" = 1"#);
    }

    #[test]
    fn a_bit_context_parameter_is_a_truth_value_or_bytes() {
        // MySQL 8.4 describes `bit1 = ?` as BIT (16) of length 64, a BIT(1) column's and a BIT(8)'s alike.
        let described = json!({ "columns": [{ "name": "id", "type": 3, "flags": 1 }], "params": [{ "type": 16 }, { "type": 253, "charset": 63 }, { "type": 8 }] });
        let (analysis, _) = analyzed("select id from users where bit1 = :b and raw = :r and big = :n", described, "STRICT_TRANS_TABLES");
        let types: Vec<String> = analysis.params.iter().map(|p| format!("{}={:?}", p.name, p.ty)).collect();
        assert_eq!(types, ["b=Scalar { name: \"bool|bytea\" }", "r=Scalar { name: \"bytea\" }", "n=Scalar { name: \"int4\" }"]);
    }

    #[test]
    fn a_vector_is_an_array_of_floats() {
        // MySQL 9.7.2 describes a VECTOR(3) column, a NULL one, and a `?` compared with one, as type 242.
        let described = json!({ "columns": [{ "name": "v", "type": 242, "flags": 4241, "charset": 63, "length": 12 }, { "name": "vn", "type": 242, "flags": 144, "charset": 63, "length": 8 }], "params": [{ "type": 242, "flags": 128, "charset": 63 }] });
        let (analysis, _) = analyzed("select v, vn from items where v = :v", described, "STRICT_TRANS_TABLES");
        let floats = Type::Array { of: Box::new(Type::Scalar { name: "float4".into() }), nullable_items: Some(false), delimiter: None };
        let columns: Vec<_> = analysis.columns.iter().map(|c| (c.ty.clone(), c.nullable, c.type_name.as_str())).collect();
        assert_eq!(columns, [(floats.clone(), false, "vector"), (floats.clone(), true, "vector")]);
        assert_eq!(analysis.params[0].ty, floats);
        // The parameter binds its floats' text, wrapped where the server reads it as a VECTOR; the
        // map back to the author's text keeps the placeholder's own place.
        assert_eq!(analysis.rewritten.sql, "select v, vn from items where v = to_vector(?)");
        assert_eq!(analysis.rewritten.spans, [(34, 36, 34, 46)]);
        let described = json!({ "columns": [], "params": [{ "type": 242 }, { "type": 242 }, { "type": 3 }, { "type": 3 }] });
        let (analysis, _) = analyzed("update items set v = :v, vn = :w where id = :id and n = :n", described, "STRICT_TRANS_TABLES");
        assert_eq!(analysis.rewritten.sql, "update items set v = to_vector(?), vn = to_vector(?) where id = ? and n = ?");
        assert_eq!(analysis.rewritten.to_original(analysis.rewritten.sql.find("id = ?").unwrap() + 5), "update items set v = :v, vn = :w where id = :id and n = :n".find(":id").unwrap());
    }

    #[test]
    fn placeholders_are_read_as_the_session_reads_strings() {
        let by_id = json!({ "columns": [{ "name": "id", "type": 3, "flags": 1 }], "params": [{ "type": 3 }] });
        let default = "ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION";
        let read = |sql: &str, mode: &str| {
            let (analysis, asked) = analyzed(sql, by_id.clone(), mode);
            (analysis.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>(), analysis.diagnostics.iter().map(|d| (d.code, d.message.clone())).collect::<Vec<_>>(), asked)
        };
        // An escaped quote leaves the placeholder after it in the string; `'\\'` ends where it seems to.
        assert_eq!(read(r"select id from users where name = 'it\'s :not' and id = :id", default), (vec!["id".to_string()], vec![], true));
        assert_eq!(read(r"select id from users where name = '\\' and id = :id", default), (vec!["id".to_string()], vec![], true));
        // Without escapes `'C:\'` ends at its second quote; with them it runs on to the end.
        assert_eq!(read(r"select id from users where name = 'C:\' and id = :id", "STRICT_TRANS_TABLES,NO_BACKSLASH_ESCAPES"), (vec!["id".to_string()], vec![], true));
        assert_eq!(read(r"select id from users where name = 'C:\' and id = :id", default), (vec![], vec![(1, "unterminated string literal".to_string())], true));
        // Text every mode reads alike asks for no mode: a `#` comment, `--` before a non-space.
        assert_eq!(read("select id from users where id = :id # and :no", default), (vec!["id".to_string()], vec![], false));
        assert_eq!(read("select id from users where id = 1 --:id", default), (vec!["id".to_string()], vec![], false));
        // A mode the server does not say leaves the placeholders unread.
        let mut answers: HashMap<String, crate::api::Answer> = HashMap::new();
        let sql = r"select id from users where name = 'C:\' and id = :id";
        let host = Host::new(&answers);
        assert!(analyze_text(sql, &host).is_none());
        for need in host.needs() {
            if let crate::api::Need::Query { id, .. } = need {
                answers.insert(id, serde_json::from_value(json!({ "rows": [] })).unwrap());
            }
        }
        let analysis = analyze_text(sql, &Host::new(&answers)).expect("the mode was answered");
        assert_eq!(analysis.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn a_column_an_execution_may_type_otherwise_is_typed_as_either() {
        // A BIGINT expression of twelve digits, and an INT column of the table `users`.
        let column = |ty: u32| json!({ "name": "c", "type": ty, "flags": 0, "length": 12, "org_table": if ty == 3 { "users" } else { "" } });
        let types = |sql: &str, columns: &[u32], params: &[u32]| {
            let described = json!({ "columns": columns.iter().map(|&c| column(c)).collect::<Vec<_>>(), "params": params.iter().map(|&p| json!({ "type": p })).collect::<Vec<_>>() });
            let shown = |ty: &Type| match ty {
                Type::Scalar { name } => name.clone(),
                other => format!("{other:?}"),
            };
            analyzed(sql, described, "").0.columns.iter().map(|c| shown(&c.ty)).collect::<Vec<_>>().join(" ")
        };
        // A number for a `?` the server typed as an integer may be no integer: a DOUBLE where it
        // is bound, a DECIMAL where it is written into the text.
        assert_eq!(types("select :n + 1 as c, id, name from users where id = :id", &[8, 3, 253], &[8, 8]), "int8|float8|numeric int4 text");
        assert_eq!(types("select id,\n  coalesce(:n, id) as c from users", &[3, 8], &[8]), "int4 int8|float8|numeric");
        assert_eq!(types("select u.*, :n * 2 as c from users u", &[3, 253, 8], &[8]), "int4 text int8|float8|numeric");
        assert_eq!(types("select :n * 2 as c, u.* from users u", &[8, 3, 253], &[8]), "int8|float8|numeric int4 text");
        // `-5` is a BIGINT where the prepare typed `-?` a DOUBLE.
        assert_eq!(types("select -:x as c, abs(:y) as d", &[5, 5], &[5, 5]), "float8|int8|numeric float8|int8|numeric");
        // A string for a DECIMAL makes the product a DOUBLE where it is written into the text,
        // and is the value itself where nothing computes with it.
        assert_eq!(types("select id * :rate as c, coalesce(:fallback, price) as d, +:plus as e, round(:rounded) as f from users", &[246, 246, 246, 246], &[246, 246, 246, 246]), "numeric|int8|float8 numeric|int8|float8|text numeric|int8|float8|text numeric|int8|float8");
        assert_eq!(types("select length(:s) as c, year(:d) as d, app.pick(:t) as e", &[8, 8, 8], &[253, 10, 253]), "int8 int8 int8");
        // A comparison and a cast are their own types whatever the value is, and so is a column
        // beside the placeholder: a comparison is a truth value.
        assert_eq!(types("select id = :n as c, cast(:m as signed) as d, id from users", &[8, 8, 3], &[8, 8]), "bool int8 int4");
        // A column of a derived table or a WITH query carries what reached it.
        assert_eq!(types("select d.c from (select :n + 1 as c from users) d", &[8], &[8]), "int8|float8|numeric");
        assert_eq!(types("with d as (select :n + 1 as c) select sum(c) as c from d", &[246], &[8]), "numeric|int8|float8");
        assert_eq!(types("select (select max(:n + p.id) from posts p) as c from users", &[8], &[8]), "int8|float8|numeric");
        assert_eq!(types("select id from users where id = :n union all select :m + 1", &[8], &[8, 8]), "int8|float8|numeric");
        // A `Date` bound for a DATE makes the sum a DATETIME, and a string written into the text a string.
        assert_eq!(types("select :day + interval 1 day as c, addtime(:t, '1') as d", &[10, 11], &[10, 11]), "date|timestamp|time|text Unknown");
        assert_eq!(types("select created + interval :n day as c from users", &[10], &[8]), "date");
        assert_eq!(types("select adddate(created, :n) as c, addtime(:t, '1') as d from users", &[10, 11], &[8, 11]), "date|timestamp|time|text Unknown");
        // A bit operation is a number or a string of bytes by what it is given, and a form the
        // walk does not read may be anything.
        assert_eq!(types("select ~:a as c, :b | 1 as d, bit_count(:c) as e, id | 1 as f from users", &[253, 8, 8, 8], &[253, 8, 253]), "Unknown Unknown Unknown int8");
        assert_eq!(types("select :a member of ('[1]') as c, substring(name, 1, :n) as d, ceil(:x) as e from users", &[8, 253, 8], &[8, 8, 5]), "Unknown text int8|float8|numeric");
        // A user variable holds a value of any type.
        assert_eq!(types("select @n := coalesce(@n, 0) + 1 as c, id, @@max_join_size as d, @`q` as e from users", &[8, 3, 8, 251], &[]), "Unknown int4 int8 Unknown");
        assert_eq!(types("select d.c, 'a@b' as e from (select @n as c) d", &[8, 253], &[]), "Unknown text");
        // A string bound beside a JSON value makes the result a string of bytes.
        assert_eq!(types("select coalesce(doc, :fallback) as c, json_extract(doc, :path) as d from users", &[245, 245], &[245, 253]), "Unknown Json { schema: None }");
    }

    #[test]
    fn a_truth_value_is_a_boolean_by_the_statement_and_a_flag_column_by_the_prepare() {
        // Each column as the prepare on 8.4.11 and 9.7.2 reports it (wiki/driver-decoding.md).
        let column = |ty: u32, length: u32, table: &str, name: &str| json!({ "name": "c", "type": ty, "flags": 0, "length": length, "org_table": table, "org_name": name });
        let types = |sql: &str, columns: Vec<Value>| {
            let described = json!({ "columns": columns, "params": [] });
            analyzed(sql, described, "").0.columns.iter().map(|c| match &c.ty { Type::Scalar { name } => name.clone(), other => format!("{other:?}") }).collect::<Vec<_>>().join(" ")
        };
        let truth = || column(8, 1, "", "");
        // Comparisons, tests and logical operators, and what passes one through.
        let sql = "select a = b as c, a is null as c, exists (select 1) as c, a in (1, 2) as c, not a as c, a and b as c, a <=> b as c, a between 1 and 2 as c, 'x' like 'y' as c, true as c, (a = b) as c, if(a, a = b, null) as c, coalesce(a = b, false) as c, case when a then a < b end as c, nullif(a = b, 1) as c, (select a = b from users limit 1) as c from users";
        assert_eq!(types(sql, (0..16).map(|_| truth()).collect()), ["bool"; 16].join(" "));
        // A literal number is no truth value, whatever its width, nor is what computes with one.
        let sql = "select 1 as c, 5 as c, case when a then 1 else 0 end as c, if(a, 1, a = b) as c, (a = b) + 0 as c, max(a = b) as c from users";
        assert_eq!(types(sql, vec![column(8, 2, "", ""), column(8, 2, "", ""), column(8, 2, "", ""), column(8, 2, "", ""), column(8, 3, "", ""), column(8, 1, "", "")]), ["int8|int4"; 6].join(" "));
        // Every arm of a union, a NULL among them; a wildcard leaves every position unknown.
        assert_eq!(types("select a = b as c from users union select null", vec![truth()]), "bool");
        assert_eq!(types("select a = b as c from users union select 5", vec![column(8, 2, "", "")]), "int8|int4");
        assert_eq!(types("select users.*, a = b as c from users", vec![column(3, 11, "users", "id"), truth()]), "int4 bool");
        assert_eq!(types("select a = b as c, users.*, a < b as d from users", vec![truth(), column(3, 11, "users", "id"), column(8, 1, "", "e"), truth()]), "bool int4 int8|int4 bool");
        assert_eq!(types("select users.* from users union select a = b from users", vec![column(8, 1, "", "")]), "int8|int4");
        // A table's TINYINT(1) or BIT(1), through a derived table too; not a TINYINT(4), a BIT(8),
        // an expression or a view's column.
        let flags = vec![column(1, 1, "users", "active"), column(16, 1, "users", "b1"), column(1, 4, "users", "small"), column(16, 8, "users", "b8"), column(1, 1, "users", "x"), column(1, 4, "", ""), column(1, 1, "", "")];
        assert_eq!(types("select active, b1, small, b8, d.x, max(active) as m, coalesce(active, active) as k from users, (select active as x from users) d", flags), "bool bool int2 bytea bool int2 int2");
        let mut answers: HashMap<String, crate::api::Answer> = HashMap::new();
        let sql = "select active from recent";
        let described = json!({ "columns": [column(1, 1, "recent", "active")], "params": [] });
        for _ in 0..8 {
            let host = Host::new(&answers);
            if let Some(analysis) = analyze_text(sql, &host) {
                assert_eq!(analysis.columns[0].ty, Type::Scalar { name: "int2".into() }, "a view's column");
                return;
            }
            for need in host.needs() {
                let (id, answer) = match need {
                    crate::api::Need::Describe { id, .. } => (id, json!({ "mysql": described })),
                    crate::api::Need::Query { id, sql, .. } if sql == VIEWS_QUERY => (id, json!({ "rows": [["recent"]] })),
                    crate::api::Need::Query { id, .. } => (id, json!({ "rows": [] })),
                };
                answers.insert(id, serde_json::from_value(answer).unwrap());
            }
        }
        panic!("the analysis never settled")
    }

    #[test]
    fn an_integer_the_statement_computes_is_as_wide_as_the_plan_makes_it() {
        let types = |columns: Value, views: &[&str]| {
            let lexed = lex::lex("select 1", Dialect::Mysql).unwrap();
            let raw: MysqlDescribe = serde_json::from_value(json!({ "columns": columns, "params": [] })).unwrap();
            let views: HashSet<String> = views.iter().map(|v| v.to_string()).collect();
            let settled = settle("select 1", &lexed, &lex::rewrite("select 1", &lexed, Target::Question), None, &raw, Some(&views));
            settled.iter().map(|ty| match ty { Some(Type::Scalar { name }) => name.clone(), other => format!("{other:?}") }).collect::<Vec<_>>().join(" ")
        };
        let column = |ty: u32, length: u32, table: &str| json!({ "name": "c", "type": ty, "flags": 0, "length": length, "org_table": table });
        // A table's own column keeps its type; a view's may be an expression.
        assert_eq!(types(json!([column(3, 11, "users"), column(8, 20, "users"), column(1, 4, "users")]), &[]), "None None None");
        assert_eq!(types(json!([column(3, 11, "Recent"), column(8, 2, "recent")]), &["recent"]), "int4|int8 int8|int4");
        // An expression: under ten digits an INT, from ten on a BIGINT.
        assert_eq!(types(json!([column(8, 1, ""), column(8, 9, ""), column(8, 10, ""), column(8, 21, "")]), &[]), "int8|int4 int8|int4 None None");
        assert_eq!(types(json!([column(3, 11, ""), column(3, 9, ""), column(1, 4, ""), column(13, 4, ""), column(9, 10, "")]), &[]), "int4|int8 None None None int4|int8");
        // A host that sends neither the length nor the table.
        assert_eq!(types(json!([{ "name": "c", "type": 3, "flags": 0 }, { "name": "c", "type": 8, "flags": 0 }, { "name": "c", "type": 253, "flags": 0 }]), &[]), "int4|int8 int8|int4 None");
        // Without the views, every name may be one.
        let raw: MysqlDescribe = serde_json::from_value(json!({ "columns": [column(3, 11, "users")], "params": [] })).unwrap();
        let lexed = lex::lex("select 1", Dialect::Mysql).unwrap();
        assert_eq!(settle("select 1", &lexed, &lex::rewrite("select 1", &lexed, Target::Question), None, &raw, None), vec![Some(Type::Scalar { name: "int4|int8".into() })]);
    }

    /// Analyzes with a host that describes each statement as `describe` says, answers the mode
    /// query with the default mode and any other query with no rows.
    fn analyzed_by(sql: &str, describe: &dyn Fn(&str) -> Value) -> Analysis {
        let mut answers: HashMap<String, crate::api::Answer> = HashMap::new();
        for _ in 0..16 {
            let host = Host::new(&answers);
            if let Some(analysis) = analyze_text(sql, &host) {
                return analysis;
            }
            for need in host.needs() {
                let (id, answer) = match need {
                    crate::api::Need::Describe { id, sql } => (id, json!({ "mysql": describe(&sql) })),
                    crate::api::Need::Query { id, sql, .. } if sql == SQL_MODE_QUERY => (id, json!({ "rows": [["ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION"]] })),
                    crate::api::Need::Query { id, .. } => (id, json!({ "rows": [] })),
                };
                answers.insert(id, serde_json::from_value(answer).unwrap());
            }
        }
        panic!("the analysis never settled")
    }

    #[test]
    fn a_strict_where_proves_the_joined_row_and_the_column_it_tests() {
        // As 8.4 describes it: `title` is NOT NULL only where the join is inner, and `published_at`
        // is a nullable column either way.
        let describe = |sql: &str| {
            let title = if sql.contains("INNER JOIN") { 1 } else { 0 };
            json!({ "columns": [{ "name": "name", "type": 253, "flags": 1 }, { "name": "title", "type": 253, "flags": title }, { "name": "published_at", "type": 12, "flags": 0 }], "params": [] })
        };
        let nullable = |sql: &str| analyzed_by(sql, &describe).columns.iter().map(|c| c.nullable).collect::<Vec<_>>();
        let select = "select u.name, p.title, p.published_at from users u left join posts p on p.user_id = u.id where";
        assert_eq!(nullable(&format!("{select} p.published_at is not null")), [false, false, false]);
        assert_eq!(nullable(&format!("{select} p.id > 0")), [false, false, true]);
        // A test NULL can pass proves nothing.
        assert_eq!(nullable(&format!("{select} p.published_at is not null or u.email is not null")), [false, true, true]);
        assert_eq!(nullable(&format!("{select} coalesce(p.published_at, now()) > '2000-01-01'")), [false, true, true]);
        assert_eq!(nullable(&format!("{select} p.id is null")), [false, true, true]);
    }

    #[test]
    fn json_arrayagg_under_a_plain_group_by_is_never_null() {
        let describe = |sql: &str| {
            let column = if sql.starts_with("SELECT title") { json!({ "name": "title", "type": 253, "flags": 1 }) } else { json!({ "name": "titles", "type": 245, "flags": 0 }) };
            json!({ "columns": [column], "params": [] })
        };
        let nullable = |sql: &str| analyzed_by(sql, &describe).columns[0].nullable;
        assert!(!nullable("select json_arrayagg(title) as titles from posts group by user_id"));
        assert!(nullable("select json_arrayagg(title) as titles from posts"));
        assert!(nullable("select json_arrayagg(title) as titles from posts group by user_id with rollup"));
        assert!(nullable("select json_arrayagg(title) over (rows between 1 following and 1 following) as titles from posts group by title"));
    }

    #[test]
    fn a_derived_tables_column_is_as_null_as_its_query_alone_finds_it() {
        // As 8.4 describes it: a scalar subquery item is nullable, its count is not, and a
        // derived table's column carries the flag of the item it selects, a coalesce's NOT NULL
        // too, except on the null-extended side of an outer join.
        let describe = |sql: &str| {
            let column = |name: &str, not_null: bool| json!({ "name": name, "type": 8, "flags": if not_null { 1 } else { 0 } });
            let columns = if sql.starts_with("SELECT count(*)") {
                vec![column("count(*)", true)]
            } else if sql == "SELECT id FROM users" {
                vec![column("id", true)]
            } else {
                vec![column("id", true), column("n", sql.contains("coalesce(reylo_derived.reylo_1, 0)") && !sql.contains("LEFT JOIN"))]
            };
            json!({ "columns": columns, "params": [] })
        };
        let nullable = |sql: &str| analyzed_by(sql, &describe).columns.iter().map(|c| c.nullable).collect::<Vec<_>>();
        let derived = "(select id, (select count(*) from posts) as n from users)";
        for sql in [
            format!("select d.id, d.n from {derived} d"),
            format!("select id, n from {derived} d"),
            format!("select * from {derived} d"),
            format!("select d.id, d.n from (select e.id, e.n from {derived} e) d"),
            format!("with d as {derived} select d.id, d.n from d"),
            format!("select d.a, d.b from {derived} d (a, b)"),
        ] {
            assert_eq!(nullable(&sql), [false, false], "{sql}");
        }
        // The null-extended side of an outer join, and a LATERAL derived table, which reads the
        // tables before it and is no query of its own.
        assert_eq!(nullable(&format!("select d.id, d.n from users u left join {derived} d on d.id = u.id")), [false, true]);
        assert_eq!(nullable("select d.id, d.n from users u, lateral (select u.id, (select count(*) from posts) as n) d"), [false, true]);
    }

    #[test]
    fn a_derived_tables_expression_on_the_null_extended_side_is_nullable() {
        // As 8.4 describes it: merged into the statement, the derived table's expression keeps its
        // own NOT NULL flag under the outer join; materialized, it is nullable there.
        let describe = |sql: &str| {
            let materialized = sql.contains("derived_merge=off");
            json!({ "columns": [{ "name": "id", "type": 3, "flags": 1 }, { "name": "next", "type": 8, "flags": if materialized { 0 } else { 1 } }], "params": [] })
        };
        let nullable = |sql: &str| analyzed_by(sql, &describe).columns.iter().map(|c| c.nullable).collect::<Vec<_>>();
        assert_eq!(nullable("select u.id, d.next from users u left join (select user_id, id + 1 as next from posts) d on d.user_id = u.id"), [false, true]);
        assert_eq!(nullable("with d as (select user_id, id + 1 as next from posts) select u.id, d.next from users u left join d on d.user_id = u.id"), [false, true]);
        // Nothing merged: the statement's own flags.
        assert_eq!(nullable("select id, id + 1 as next from users"), [false, false]);
    }

    #[test]
    fn a_view_read_through_a_derived_table_neither_breaks_the_copy_nor_takes_a_target() {
        // As 8.4 describes it over a view `v`: a column named through the view's schema does not
        // resolve against the derived table the copy reads the view through, and the derived
        // table's expression is nullable on the null-extended side once materialized.
        let described = std::cell::RefCell::new(Vec::<String>::new());
        let describe = |sql: &str| -> Value {
            described.borrow_mut().push(sql.to_string());
            if sql.contains("app.v.x") && sql.contains("(SELECT * FROM app.v) AS v") {
                return json!({ "error": { "message": "Unknown column 'app.v.x' in 'field list'" } });
            }
            let materialized = sql.contains("derived_merge=off");
            json!({ "mysql": { "columns": [{ "name": "x", "type": 3, "flags": 1 }, { "name": "next", "type": 8, "flags": if materialized { 0 } else { 1 } }], "params": [] } })
        };
        let nullable = |sql: &str| {
            let mut answers: HashMap<String, crate::api::Answer> = HashMap::new();
            for _ in 0..16 {
                let host = Host::new(&answers);
                if let Some(analysis) = analyze_text(sql, &host) {
                    return analysis.columns.iter().map(|c| c.nullable).collect::<Vec<_>>();
                }
                for need in host.needs() {
                    let (id, answer) = match need {
                        crate::api::Need::Describe { id, sql } => (id, describe(&sql)),
                        crate::api::Need::Query { id, sql, .. } if sql == SQL_MODE_QUERY => (id, json!({ "rows": [["ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION"]] })),
                        crate::api::Need::Query { id, sql, .. } if sql == VIEWS_QUERY => (id, json!({ "rows": [["v"]] })),
                        crate::api::Need::Query { id, .. } => (id, json!({ "rows": [] })),
                    };
                    answers.insert(id, serde_json::from_value(answer).unwrap());
                }
            }
            panic!("the analysis never settled")
        };
        let derived = "left join (select user_id, id + 1 as next from posts) d on d.user_id = u.id";
        assert_eq!(nullable(&format!("select app.v.x, d.next from users u join app.v on app.v.id = u.id {derived}")), [false, true]);
        // The view's derived table is no query of its own to prove.
        described.borrow_mut().clear();
        assert_eq!(nullable(&format!("select v.x, d.next from users u join v on v.id = u.id {derived}")), [false, true]);
        assert!(!described.borrow().iter().any(|sql| sql.starts_with("SELECT * FROM v")), "{:?}", described.borrow());
    }

    #[test]
    fn a_column_read_as_it_is_carries_its_comment() {
        // As 8.4 prepares them over `users (id int primary key comment 'The id', age int comment
        // 'Age (in years)', email varchar(100))` and `o (other int comment 'Other', doc int comment
        // 'Doc')`: an expression has no origin, an alias keeps its column's, and through a derived
        // table the origin is the base table beside the derived column's own name.
        let column = |name: &str, table: &str, org: &str| json!({ "name": name, "type": 3, "flags": 0, "org_table": table, "org_name": org });
        let describe = |sql: &str| {
            let columns = match sql {
                "select id, age as years, lower(email) from users where id = ?" => vec![column("id", "users", "id"), column("years", "users", "age"), column("lower(email)", "", "")],
                "select doc from (select other as doc from o) s" => vec![column("doc", "o", "doc")],
                _ => vec![],
            };
            let params = sql.matches('?').map(|_| json!({ "type": 8 })).collect::<Vec<_>>();
            json!({ "columns": columns, "params": params })
        };
        let comments = [("users", "id", "The id"), ("users", "age", "Age (in years)"), ("users", "email", ""), ("o", "other", "Other"), ("o", "doc", "Doc")];
        let analyzed = |sql: &str| {
            let mut answers: HashMap<String, crate::api::Answer> = HashMap::new();
            for _ in 0..16 {
                let host = Host::new(&answers);
                if let Some(analysis) = analyze_text(sql, &host) {
                    return analysis;
                }
                for need in host.needs() {
                    let (id, answer) = match need {
                        crate::api::Need::Describe { id, sql } => (id, json!({ "mysql": describe(&sql) })),
                        crate::api::Need::Query { id, sql, .. } if sql == SQL_MODE_QUERY => (id, json!({ "rows": [["ONLY_FULL_GROUP_BY,STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION"]] })),
                        crate::api::Need::Query { id, sql, params, .. } if sql == crate::mysql_params::TYPES_QUERY => {
                            let tables = params[0].as_str().unwrap().split(',').map(String::from).collect::<Vec<_>>();
                            let rows = comments.iter().filter(|(t, _, _)| tables.iter().any(|n| n == t)).map(|(t, c, comment)| json!([t, c, "int", "int", "", comment])).collect::<Vec<_>>();
                            (id, json!({ "rows": rows }))
                        }
                        crate::api::Need::Query { id, .. } => (id, json!({ "rows": [] })),
                    };
                    answers.insert(id, serde_json::from_value(answer).unwrap());
                }
            }
            panic!("the analysis never settled")
        };
        let some = |c: &str| Some(c.to_string());
        let a = analyzed("select id, age as years, lower(email) from users where id = :id");
        assert_eq!(a.columns.iter().map(|c| c.comment.clone()).collect::<Vec<_>>(), [some("The id"), some("Age (in years)"), None]);
        // A parameter compared with a column carries its comment.
        assert_eq!(a.params.iter().map(|p| p.comment.clone()).collect::<Vec<_>>(), [some("The id")]);
        // `o.doc` is not the column `doc` reads.
        let a = analyzed("select doc from (select other as doc from o) s");
        assert_eq!(a.columns.iter().map(|c| c.comment.clone()).collect::<Vec<_>>(), [None]);
    }
}
