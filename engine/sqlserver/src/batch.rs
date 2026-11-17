//! A `.sql` file's batch: its statements, as T-SQL takes them with or without `;` between them, and
//! the one whose rows are the result. SQL Server's own describe types a batch's first result set,
//! following every path through it (`if`, `while`, `try`); the engine finds the statement that makes
//! that result set, so that its rules (the columns that are never NULL, the cardinality) read that
//! statement as they read a statement alone:
//!
//! - a statement makes a result set where it is a `select` that neither assigns variables
//!   (`select @n = count(*) …`) nor fills a table (`select … into`), a write with an `output` that
//!   fills no table, or an `exec`, whose procedure's result the engine has no statement of;
//! - the result always comes where its statement is at the batch's top level, after no `return`;
//!   elsewhere, in a branch or a loop, it may not come, so a row count of one is at most one;
//! - its rules read it where no `set` before it turns off an option they rest on (`ansi_warnings`,
//!   `arithabort`, `concat_null_yields_null`, `ansi_nulls`), with every `declare` before it, which
//!   the probe of its columns needs ([`crate::nulls`]).
//!
//! A batch the parser does not read, or one with a statement the engine does not follow, is typed by
//! the server's describe alone.

use std::any::TypeId;

use sqlparser::ast::{ConditionalStatements, DataType, GranteesType, OutputClause, Query, SelectItem, SetExpr, Statement};
use sqlparser::dialect::{Dialect, MsSqlDialect};
use sqlparser::keywords::Keyword;
use sqlparser::parser::{Parser, ParserError, ParserOptions};
use sqlparser::tokenizer::{Token, TokenWithSpan, Tokenizer};

/// The options whose `set … off` makes an expression NULL where the rules say it is not.
const NARROWING_OPTIONS: &[&str] = &["ansi_warnings", "arithabort", "concat_null_yields_null", "ansi_nulls"];

/// The words a T-SQL statement starts with, where one follows a `return`.
const STATEMENT_WORDS: &[Keyword] = &[
    Keyword::SELECT, Keyword::INSERT, Keyword::UPDATE, Keyword::DELETE, Keyword::MERGE, Keyword::WITH, Keyword::IF, Keyword::ELSE, Keyword::WHILE,
    Keyword::BEGIN, Keyword::END, Keyword::DECLARE, Keyword::SET, Keyword::EXEC, Keyword::EXECUTE, Keyword::PRINT, Keyword::THROW, Keyword::RAISERROR,
    Keyword::RETURN, Keyword::COMMIT, Keyword::ROLLBACK, Keyword::TRUNCATE, Keyword::WAITFOR,
];

/// T-SQL's reserved words that start a statement and that sqlparser's SQL Server dialect still
/// takes as an alias with no `as`, so that a statement after a select item or a table on its own
/// line (`select @n = 1` then `set @m = 2`) reads as the alias of what precedes it. T-SQL takes no
/// reserved word as an alias unless it is bracketed.
const RESERVED_STATEMENT_WORDS: &[Keyword] = &[
    Keyword::SET, Keyword::BEGIN, Keyword::OPEN, Keyword::CLOSE, Keyword::DEALLOCATE, Keyword::FETCH, Keyword::USE, Keyword::GRANT, Keyword::DENY,
    Keyword::REVOKE, Keyword::WAITFOR, Keyword::KILL, Keyword::COMMIT, Keyword::ROLLBACK, Keyword::CONTINUE, Keyword::BACKUP,
];

/// sqlparser's SQL Server dialect, with T-SQL's reserved statement words never an alias
/// ([`RESERVED_STATEMENT_WORDS`]). It reports the SQL Server dialect's type, so every check the
/// parser makes for that dialect holds, and answers every other question as that dialect does.
#[derive(Debug)]
struct TSql;

impl Dialect for TSql {
    fn dialect(&self) -> TypeId {
        TypeId::of::<MsSqlDialect>()
    }
    fn is_select_item_alias(&self, explicit: bool, kw: &Keyword, parser: &mut Parser) -> bool {
        !RESERVED_STATEMENT_WORDS.contains(kw) && MsSqlDialect {}.is_select_item_alias(explicit, kw, parser)
    }
    fn is_table_factor_alias(&self, explicit: bool, kw: &Keyword, parser: &mut Parser) -> bool {
        !RESERVED_STATEMENT_WORDS.contains(kw) && MsSqlDialect {}.is_table_factor_alias(explicit, kw, parser)
    }
    // Every other method the SQL Server dialect has of its own, as it answers it.
    fn is_delimited_identifier_start(&self, ch: char) -> bool {
        MsSqlDialect {}.is_delimited_identifier_start(ch)
    }
    fn is_identifier_start(&self, ch: char) -> bool {
        MsSqlDialect {}.is_identifier_start(ch)
    }
    fn is_identifier_part(&self, ch: char) -> bool {
        MsSqlDialect {}.is_identifier_part(ch)
    }
    fn identifier_quote_style(&self, identifier: &str) -> Option<char> {
        MsSqlDialect {}.identifier_quote_style(identifier)
    }
    fn convert_type_before_value(&self) -> bool {
        MsSqlDialect {}.convert_type_before_value()
    }
    fn supports_outer_join_operator(&self) -> bool {
        MsSqlDialect {}.supports_outer_join_operator()
    }
    fn supports_dollar_as_money_prefix(&self) -> bool {
        MsSqlDialect {}.supports_dollar_as_money_prefix()
    }
    fn supports_connect_by(&self) -> bool {
        MsSqlDialect {}.supports_connect_by()
    }
    fn supports_eq_alias_assignment(&self) -> bool {
        MsSqlDialect {}.supports_eq_alias_assignment()
    }
    fn supports_try_convert(&self) -> bool {
        MsSqlDialect {}.supports_try_convert()
    }
    fn supports_boolean_literals(&self) -> bool {
        MsSqlDialect {}.supports_boolean_literals()
    }
    fn supports_named_fn_args_with_colon_operator(&self) -> bool {
        MsSqlDialect {}.supports_named_fn_args_with_colon_operator()
    }
    fn supports_named_fn_args_with_expr_name(&self) -> bool {
        MsSqlDialect {}.supports_named_fn_args_with_expr_name()
    }
    fn supports_named_fn_args_with_rarrow_operator(&self) -> bool {
        MsSqlDialect {}.supports_named_fn_args_with_rarrow_operator()
    }
    fn supports_start_transaction_modifier(&self) -> bool {
        MsSqlDialect {}.supports_start_transaction_modifier()
    }
    fn supports_end_transaction_modifier(&self) -> bool {
        MsSqlDialect {}.supports_end_transaction_modifier()
    }
    fn supports_set_stmt_without_operator(&self) -> bool {
        MsSqlDialect {}.supports_set_stmt_without_operator()
    }
    fn supports_table_versioning(&self) -> bool {
        MsSqlDialect {}.supports_table_versioning()
    }
    fn supports_nested_comments(&self) -> bool {
        MsSqlDialect {}.supports_nested_comments()
    }
    fn supports_object_name_double_dot_notation(&self) -> bool {
        MsSqlDialect {}.supports_object_name_double_dot_notation()
    }
    fn get_reserved_grantees_types(&self) -> &[GranteesType] {
        MsSqlDialect {}.get_reserved_grantees_types()
    }
    fn parse_statement(&self, parser: &mut Parser) -> Option<Result<Statement, ParserError>> {
        MsSqlDialect {}.parse_statement(parser)
    }
    fn get_next_precedence(&self, parser: &Parser) -> Option<Result<u8, ParserError>> {
        MsSqlDialect {}.get_next_precedence(parser)
    }
}

/// The batch's statements, as T-SQL takes them, with or without a `;` after each. sqlparser 0.63
/// reads that at a batch's top level alone: it stops for a `;` after each statement of a block
/// (`begin … end`, a `while`'s body), and reads a `return` with no value as returning the statement
/// after it. T-SQL wants neither, so the engine puts a `;` after such a `return`, and where the
/// parser stops for one after a statement it has read whole.
pub fn parse(sql: &str) -> Option<Vec<Statement>> {
    let dialect = TSql;
    let mut tokens = Tokenizer::new(&dialect, sql).tokenize_with_location().ok()?;
    end_bare_returns(&mut tokens);
    // Each try that stops for a `;` adds one, before a token that had none, so the tries are bounded.
    for _ in 0..=tokens.len() {
        let mut options = ParserOptions::new();
        options.require_semicolon_stmt_delimiter = false;
        let mut parser = Parser::new(&dialect).with_options(options).with_tokens_with_locations(tokens.clone());
        let error = match parser.parse_statements() {
            Ok(statements) => return Some(statements),
            Err(e) => e.to_string(),
        };
        let next = parser.peek_token_ref();
        if !error.ends_with(&format!("Expected: ;, found: {}{}", next.token, next.span.start)) {
            return None;
        }
        let at = (parser.index()..tokens.len()).find(|&i| !matches!(tokens[i].token, Token::Whitespace(_))).unwrap_or(tokens.len());
        tokens.insert(at, TokenWithSpan::wrap(Token::SemiColon));
    }
    None
}

/// A `;` after each `return` that a statement follows, which returns no value.
fn end_bare_returns(tokens: &mut Vec<TokenWithSpan>) {
    let word = |t: &Token, of: &[Keyword]| matches!(t, Token::Word(w) if w.quote_style.is_none() && of.contains(&w.keyword));
    let mut i = 0;
    while i < tokens.len() {
        if word(&tokens[i].token, &[Keyword::RETURN]) {
            let next = tokens[i + 1..].iter().find(|t| !matches!(t.token, Token::Whitespace(_)));
            if next.is_some_and(|t| word(&t.token, STATEMENT_WORDS)) {
                tokens.insert(i + 1, TokenWithSpan::wrap(Token::SemiColon));
            }
        }
        i += 1;
    }
}

/// The statement that makes a batch's result set, and how its rules may read it.
#[derive(Debug)]
pub struct Found<'a> {
    pub statement: &'a Statement,
    /// Whether the result comes whenever the batch runs.
    pub sure: bool,
    /// Whether the rules read the statement: a `select` or a write, after no `set` that turns off an
    /// option they rest on.
    pub narrows: bool,
    /// The batch's `declare`s before the statement, each with its `;`, which a probe of its
    /// columns starts with.
    pub prelude: String,
    /// The variables declared before the statement, each with its type's name (`nvarchar`), but
    /// table variables.
    pub variables: Vec<(String, String)>,
}

/// What the engine reads of a batch.
#[derive(Debug, Default)]
pub struct Read<'a> {
    /// The statements that make a result set, in the order they stand: one, or one in each branch
    /// of an `if`.
    pub results: Vec<Found<'a>>,
    /// The most result sets a run of the batch may return: one path's statements add up, an `if`'s
    /// branches take the most of theirs, and a loop that makes rows may make them again.
    pub per_run: usize,
    /// Whether every statement is one the engine follows.
    pub understood: bool,
}

#[derive(Default)]
struct Walk<'a> {
    results: Vec<Found<'a>>,
    prelude: String,
    variables: Vec<(String, String)>,
    returned: bool,
    options_off: bool,
    unknown: bool,
}

/// The batch's result statements, as the module's rules read them.
pub fn read(statements: &[Statement]) -> Read<'_> {
    let mut walk = Walk::default();
    let per_run = visit(statements, false, &mut walk);
    Read { results: walk.results, per_run, understood: !walk.unknown }
}

/// The statements' result statements, into `w`, and the most result sets a run of them may return.
fn visit<'a>(statements: &'a [Statement], conditional: bool, w: &mut Walk<'a>) -> usize {
    let mut per_run = 0;
    for statement in statements {
        let before = w.results.len();
        match statement {
            Statement::If(i) => {
                let mut most = 0;
                for block in std::iter::once(&i.if_block).chain(&i.elseif_blocks).chain(&i.else_block) {
                    most = most.max(visit(block_statements(&block.conditional_statements), true, w));
                }
                per_run += most;
            }
            Statement::While(l) => {
                let (body, after) = loop_body(&l.while_block.conditional_statements);
                if visit(body, true, w) > 0 {
                    // A loop that makes rows makes them again on its next turn.
                    per_run += 2;
                }
                per_run += visit(after, conditional, w);
            }
            // `begin … end` with no `transaction` is a block; `begin transaction` starts one.
            Statement::StartTransaction { statements, transaction: None, .. } => per_run += visit(statements, conditional, w),
            Statement::StartTransaction { .. } | Statement::Commit { .. } | Statement::Rollback { .. } => {}
            // sqlparser prints `declare @a int, @b int` as `DECLARE @a INT; @b INT`, so each is
            // written as a `declare` of its own.
            Statement::Declare { stmts } => {
                for declare in stmts {
                    w.prelude.push_str(&format!("DECLARE {declare};\n"));
                    if let (Some(name), Some(ty)) = (declare.names.first(), &declare.data_type) {
                        if !matches!(ty, DataType::Table(_) | DataType::NamedTable { .. }) {
                            let ty = ty.to_string();
                            w.variables.push((name.value.clone(), ty.split('(').next().unwrap_or_default().trim().to_ascii_lowercase()));
                        }
                    }
                }
            }
            Statement::Set(_) => {
                let text = statement.to_string().to_ascii_lowercase();
                if NARROWING_OPTIONS.iter().any(|o| text.contains(o)) && text.contains("off") {
                    w.options_off = true;
                }
            }
            Statement::Return(_) => w.returned = true,
            Statement::Print(_) | Statement::Throw(_) | Statement::RaisError { .. } => {}
            Statement::Query(q) => {
                if makes_rows(q) {
                    found(statement, conditional, true, w);
                }
            }
            Statement::Insert(i) if returns(&i.output) => found(statement, conditional, true, w),
            Statement::Update(u) if returns(&u.output) => found(statement, conditional, true, w),
            Statement::Delete(d) if returns(&d.output) => found(statement, conditional, true, w),
            Statement::Merge(m) if returns(&m.output) => found(statement, conditional, true, w),
            Statement::Insert(_) | Statement::Update(_) | Statement::Delete(_) | Statement::Merge(_) => {}
            Statement::Execute { .. } => found(statement, conditional, false, w),
            _ => w.unknown = true,
        }
        // A statement that makes rows, met here rather than in a block, is one result set more.
        if matches!(statement, Statement::Query(_) | Statement::Insert(_) | Statement::Update(_) | Statement::Delete(_) | Statement::Merge(_) | Statement::Execute { .. }) {
            per_run += w.results.len() - before;
        }
    }
    per_run
}

/// Every statement of the batch, those in its blocks (`if`, `while`, `begin … end`) included, in
/// the order they stand.
pub fn flatten(statements: &[Statement]) -> Vec<&Statement> {
    let mut out = Vec::new();
    for statement in statements {
        match statement {
            Statement::If(i) => {
                for block in std::iter::once(&i.if_block).chain(&i.elseif_blocks).chain(&i.else_block) {
                    out.extend(flatten(block_statements(&block.conditional_statements)));
                }
            }
            Statement::While(l) => out.extend(flatten(block_statements(&l.while_block.conditional_statements))),
            Statement::StartTransaction { statements, transaction: None, .. } => out.extend(flatten(statements)),
            other => out.push(other),
        }
    }
    out
}

fn found<'a>(statement: &'a Statement, conditional: bool, readable: bool, w: &mut Walk<'a>) {
    w.results.push(Found { statement, sure: !conditional && !w.returned, narrows: readable && !w.options_off, prelude: w.prelude.clone(), variables: w.variables.clone() });
}

fn block_statements(c: &ConditionalStatements) -> &[Statement] {
    match c {
        ConditionalStatements::Sequence { statements } => statements,
        ConditionalStatements::BeginEnd(b) => &b.statements,
    }
}

/// A `while`'s body, and the statements after the loop: without `begin … end` its body is one
/// statement, where sqlparser reads every statement up to the block's end into it.
fn loop_body(c: &ConditionalStatements) -> (&[Statement], &[Statement]) {
    match c {
        ConditionalStatements::Sequence { statements } => statements.split_at(statements.len().min(1)),
        ConditionalStatements::BeginEnd(b) => (&b.statements, &[]),
    }
}

/// Whether a write's `output` sends rows, rather than filling a table.
fn returns(o: &Option<OutputClause>) -> bool {
    matches!(o, Some(OutputClause::Output { into_table: None, .. }))
}

/// Whether a `select` sends rows: one that fills a table (`into`) or assigns variables does not. The
/// parser reads `select @n = count(*)` as a column named `@n`.
fn makes_rows(q: &Query) -> bool {
    match q.body.as_ref() {
        SetExpr::Select(s) => s.into.is_none() && !s.projection.iter().any(|item| matches!(item, SelectItem::ExprWithAlias { alias, .. } if alias.value.starts_with('@'))),
        SetExpr::Query(q) => makes_rows(q),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn results(sql: &str) -> Vec<(String, bool, bool)> {
        let statements = parse(sql).unwrap();
        read(&statements).results.iter().map(|f| (f.statement.to_string(), f.sure, f.narrows)).collect()
    }

    #[test]
    fn finds_the_result_statement() {
        // Declarations, a variable's assignment and a write with no output make no rows.
        let sql = "declare @since datetime2 = dateadd(day, -7, sysutcdatetime())\nselect @n = count(*) from orders\ninsert into log (n) values (1)\nselect id from orders where placed_at > @since";
        assert_eq!(results(sql), [("SELECT id FROM orders WHERE placed_at > @since".to_string(), true, true)]);
        let statements = parse(sql).unwrap();
        assert!(read(&statements).results[0].prelude.starts_with("DECLARE @since"));
        // A branch's result may not come; a `return` before one makes it so too.
        assert_eq!(results("if @f = 1 select id from orders")[0].1, false);
        assert_eq!(results("if @f = 1 return; select id from orders")[0].1, false);
        // An option the rules rest on, turned off, stops them reading the statement.
        assert_eq!(results("set ansi_warnings off; select n + 1 as m from t")[0].2, false);
        // A write with an output, and a procedure.
        assert_eq!(results("update t set a = 1 output inserted.a where id = @p_1").len(), 1);
        assert_eq!(results("exec dbo.find @id = @p_1")[0].2, false);
        // Two statements that make rows, with or without a `;` between them, are two result sets a
        // run; one in each branch of an `if` is one; one in a loop may be many.
        let per_run = |sql: &str| read(&parse(sql).unwrap()).per_run;
        assert_eq!(per_run("select 1 as a select 2 as b"), 2);
        assert_eq!(per_run("if @f = 1 select 1 as a else select 2 as a"), 1);
        assert_eq!(per_run("declare @i int = 0; while @i < 3 begin set @i = @i + 1; select @i as i end"), 2);
        assert_eq!(per_run("insert into t (a) values (1); update t set a = 2"), 0);
    }

    #[test]
    fn reads_t_sql_without_semicolons() {
        // A block's statements, with no `;` between them or before its `end`.
        assert_eq!(results("if @f = 1\nbegin\n  declare @n int = 1\n  select id from orders where id = @n\nend"), [("SELECT id FROM orders WHERE id = @n".to_string(), false, true)]);
        // A `return` with no value, then a statement.
        assert_eq!(results("if @f = 1 return\nselect id from orders"), [("SELECT id FROM orders".to_string(), false, true)]);
        // A `while` without `begin` loops over one statement; the next is after the loop.
        let statements = parse("declare @n int = 0\nwhile @n < @upto set @n = @n + 1\nselect @n as n").unwrap();
        let looped = read(&statements);
        assert_eq!((looped.per_run, looped.results[0].sure), (1, true));
        // Each variable of a `declare` list is declared on its own, as T-SQL reads the prelude.
        let statements = parse("declare @a int = 1, @b nvarchar(10) = N'x'\ndeclare @t table (id int)\nselect @a as a, @b as b").unwrap();
        assert_eq!(read_prelude(&statements), "DECLARE @a INT = 1;\nDECLARE @b NVARCHAR(10) = N'x';\nDECLARE @t TABLE(id INT);\n");
        assert_eq!(read(&statements).results[0].variables, [("@a".to_string(), "int".to_string()), ("@b".to_string(), "nvarchar".to_string())]);
        // A word that only looks like `return`, and a `case … end` in a block.
        assert!(parse("select [return] from t").is_some());
        assert_eq!(per_run_of("while @i < 3 begin set @i = case when @i > 1 then 3 else @i + 1 end select @i as i end"), 2);
    }

    #[test]
    fn reads_a_statement_after_a_select_item_or_a_table_on_its_own_line() {
        let kinds = |sql: &str| parse(sql).unwrap().iter().map(|s| s.to_string().split(' ').next().unwrap_or_default().to_string()).collect::<Vec<_>>();
        assert_eq!(kinds("select @a = 1\nset @b = 2"), ["SELECT", "SET"]);
        assert_eq!(kinds("select 1 as a\nbegin tran\ncommit"), ["SELECT", "BEGIN", "COMMIT"]);
        assert_eq!(kinds("select * from t\nbegin transaction\nrollback"), ["SELECT", "BEGIN", "ROLLBACK"]);
        // An UPDATE's SET on its own line is still its own.
        assert_eq!(kinds("update t\nset a = 1"), ["UPDATE"]);
        // Everything else reads as the SQL Server dialect reads it.
        for sql in [
            "select top (5) [id], t.[name] n from dbo.t with (nolock) where x = N'a' and y = $5 order by id offset 1 rows",
            "if @f = 1 begin select 1 as a; end else select 2 as b",
            "declare @t table (id int not null primary key); insert into @t (id) output inserted.id select id from t",
            "merge t using s on t.id = s.id when matched then update set a = s.a output inserted.id;",
            "exec dbo.p @a = 1, @b = N'x'",
        ] {
            let ours = format!("{:?}", parse(sql));
            let mut options = ParserOptions::new();
            options.require_semicolon_stmt_delimiter = false;
            let theirs = format!("{:?}", Parser::new(&MsSqlDialect {}).with_options(options).try_with_sql(sql).unwrap().parse_statements().ok());
            assert_eq!(ours, theirs, "{sql}");
        }
    }

    fn read_prelude(statements: &[Statement]) -> String {
        read(statements).results[0].prelude.clone()
    }

    fn per_run_of(sql: &str) -> usize {
        read(&parse(sql).unwrap()).per_run
    }
}
