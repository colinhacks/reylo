//! What running a MySQL statement may do ([`Capabilities`]), an upper bound: the tables of the
//! session's database it reaches, through views and foreign keys' actions as its errors are read
//! ([`crate::errors`]), and what its kind and the server's functions it calls do beyond their rows.
//! A trigger or a stored function, whose code the engine does not read, may touch any table, and a
//! statement that sets one off has its tables unknown.

use crate::errors::{list, VIEWS_QUERY};
use crate::host::Host;
use reylo_engine_core::api::{Analysis, Scope};
use reylo_engine_core::dialect::Dialect;
use reylo_engine_core::errors::{called, has_word};
use reylo_engine_core::host::Lazy;
use reylo_engine_core::lex;
use reylo_engine_core::types::{Access, Capabilities, StatementKind};
use serde_json::Value;

/// `{written}` and `{read}` are lists of string literals, the tables of the session's database the
/// statement writes and reads; `{cascades}` whether a row it writes may change a key another
/// table's foreign key refers to. One row `(name, access)` for each table it reaches, qualified by
/// its database, with the most it may do to its rows (2 writes, 1 locks, 0 reads). A write reaches
/// the written tables, a view's tables, and, where `{cascades}`, the tables a foreign key's action
/// writes in turn; it locks the rows a foreign key checks, in the table the key refers to and,
/// where `{cascades}`, in the tables that refer to it. A read reaches the read tables and a view's.
const TABLES_QUERY: &str = "with recursive w(t) as ( \
      select table_name from information_schema.tables where table_schema = database() and table_name in ({written}) \
      union select u.table_name from information_schema.view_table_usage u join w on u.view_name = w.t where u.view_schema = database() and u.table_schema = database() \
      union select r.table_name from information_schema.referential_constraints r join w on r.referenced_table_name = w.t \
        where r.constraint_schema = database() and {cascades} and (r.delete_rule in ('CASCADE', 'SET NULL', 'SET DEFAULT') or r.update_rule in ('CASCADE', 'SET NULL', 'SET DEFAULT'))), \
    r(t) as (select table_name from information_schema.tables where table_schema = database() and table_name in ({read}) \
      union select u.table_name from information_schema.view_table_usage u join r on u.view_name = r.t where u.view_schema = database() and u.table_schema = database()), \
    reached(t, access) as ( \
      select t, 2 from w \
      union all select c.referenced_table_name, 1 from information_schema.referential_constraints c join w on c.table_name = w.t where c.constraint_schema = database() \
      union all select c.table_name, 1 from information_schema.referential_constraints c join w on c.referenced_table_name = w.t where c.constraint_schema = database() and {cascades} \
      union all select t, 0 from r) \
    select cast(concat(database(), '.', t) as char), max(access) from reached group by t";

/// The server's functions that leave state on the session: a named lock, the last insert id.
const SESSION: [&str; 4] = ["get_lock", "release_lock", "release_all_locks", "last_insert_id"];

/// What the statement may do, or `None` where the engine says nothing: a statement it could not
/// analyze.
pub fn capabilities(analysis: &Analysis, host: &Host) -> Lazy<Option<Capabilities>> {
    if !analysis.diagnostics.is_empty() {
        return Some(None);
    }
    let sql = &analysis.rewritten.sql;
    // A table named by its schema, which the catalog's walk does not reach.
    if analysis.capabilities.as_ref().is_some_and(Capabilities::tables_unknown) {
        return Some(analysis.capabilities.clone());
    }
    let mut capabilities = Capabilities::default();
    match analysis.kind {
        Some(StatementKind::Select | StatementKind::Insert | StatementKind::Update | StatementKind::Delete) => reached(analysis, sql, host, &mut capabilities)?,
        _ => command(sql, &mut capabilities),
    }
    Some(Some(capabilities))
}

/// What a query or a write reaches.
fn reached(analysis: &Analysis, sql: &str, host: &Host, capabilities: &mut Capabilities) -> Lazy<()> {
    // A trigger or a stored function runs, which the errors' reading of the catalog found, or the
    // statement names a table of another database, which it does not read.
    let Some(tables) = analysis.tables.as_ref().filter(|_| analysis.errors.is_some()) else {
        unread(capabilities);
        return Some(());
    };
    if tables.read.iter().chain(&tables.write).any(|t| t.contains('.') || t.contains('\\')) {
        unread(capabilities);
        return Some(());
    }
    // An INSERT changes no key but through ON DUPLICATE KEY UPDATE or as a REPLACE, which deletes.
    let cascades = !tables.write.is_empty() && (analysis.kind != Some(StatementKind::Insert) || has_word(sql, Dialect::Mysql, &["update", "replace"]));
    let query = TABLES_QUERY
        .replace("{written}", &list(tables.write.iter().map(String::as_str)))
        .replace("{read}", &list(tables.read.iter().map(String::as_str)))
        .replace("{cascades}", if cascades { "true" } else { "false" });
    let Ok(rows) = host.query(&query, Vec::new(), Scope::Statement)? else {
        unread(capabilities);
        return Some(());
    };
    // A locking read (`FOR UPDATE`, `FOR SHARE`, `LOCK IN SHARE MODE`) locks the rows it reads, and
    // so does a write under REPEATABLE READ, InnoDB's default, of the rows its SELECT or subquery reads.
    let locks = analysis.kind != Some(StatementKind::Select) || has_word(sql, Dialect::Mysql, &["for", "lock"]);
    for row in &rows {
        let Some(name) = row.first().and_then(Value::as_str) else { continue };
        let access = match row.get(1).and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok())) {
            Some(2) => Access::Write,
            Some(1) => Access::Readlock,
            _ if locks => Access::Readlock,
            _ => Access::Read,
        };
        capabilities.touch(name, access);
    }
    let mut functions = called(sql, Dialect::Mysql);
    // A view calls the server's functions too: each definition it reaches is read as the
    // statement's own text is, as `read_only` reads them, and one the session cannot read is unread.
    if !tables.read.is_empty() {
        let Ok(views) = host.query(&VIEWS_QUERY.replace("{read}", &list(tables.read.iter().map(String::as_str))), Vec::new(), Scope::Statement)? else {
            unread(capabilities);
            return Some(());
        };
        for view in &views {
            match view.first().and_then(Value::as_str) {
                Some(definition) if !definition.is_empty() => functions.extend(called(definition, Dialect::Mysql)),
                _ => {
                    unread(capabilities);
                    return Some(());
                }
            }
        }
    }
    if functions.iter().any(|f| SESSION.contains(&f.as_str())) || sql.contains(":=") {
        capabilities.session = true;
    }
    // `SELECT … INTO` a variable sets it; `INTO OUTFILE` or `DUMPFILE` writes a file on the server.
    if has_word(sql, Dialect::Mysql, &["outfile", "dumpfile"]) || functions.iter().any(|f| f == "load_file") {
        capabilities.external = true;
    } else if has_word(sql, Dialect::Mysql, &["into"]) && analysis.kind == Some(StatementKind::Select) {
        capabilities.session = true;
    }
    Some(())
}

/// What a statement that is no query or write does, by its first keywords.
fn command(sql: &str, capabilities: &mut Capabilities) {
    let lexed = lex::lex(sql, Dialect::Mysql).ok();
    let word = lexed.as_ref().map(|l| lex::first_word(sql, l)).unwrap_or_default();
    let has = |words: &[&str]| has_word(sql, Dialect::Mysql, words);
    match word.as_str() {
        "start" | "begin" | "commit" | "rollback" | "savepoint" | "release" | "xa" => capabilities.transaction = true,
        // A value worked out by a call or a subquery runs what it calls or reads.
        "set" if sql.contains('(') => {
            unread(capabilities);
            capabilities.transaction = has(&["autocommit"]);
        }
        // A global or persisted setting is the server's; any other the session's, a transaction's characteristics included.
        "set" if has(&["global", "persist", "persist_only"]) || sql.contains("@@global") || sql.contains("@@persist") => capabilities.external = true,
        // An account's password or default role is the server's to keep, as a GRANT is.
        "set" if has(&["password"]) || has(&["default"]) && has(&["role"]) => capabilities.ddl = true,
        // Turning autocommit on commits the open transaction.
        "set" if has(&["autocommit"]) => {
            capabilities.session = true;
            capabilities.transaction = true;
        }
        "set" | "use" | "prepare" | "deallocate" => capabilities.session = true,
        // A plan, and what the server shows of its catalog and state: no table's rows. With ANALYZE, the statement runs.
        "show" | "help" => {}
        "explain" | "describe" | "desc" if !has(&["analyze"]) => {}
        // LOCK TABLES commits the open transaction, and the session holds the locks.
        "lock" | "unlock" => {
            capabilities.unknown_tables();
            capabilities.transaction = true;
            capabilities.session = true;
        }
        // DDL commits the open transaction, and may rewrite a table's rows.
        "create" | "alter" | "drop" | "rename" | "truncate" | "grant" | "revoke" | "analyze" | "optimize" | "repair" | "check" | "checksum" | "install" | "uninstall" => {
            capabilities.ddl = true;
            capabilities.transaction = true;
            capabilities.unknown_tables();
            // A temporary table is the session's.
            capabilities.session = has(&["temporary"]);
        }
        "kill" | "flush" | "shutdown" | "restart" | "reset" | "purge" | "change" | "binlog" | "clone" => capabilities.external = true,
        // A procedure, a prepared statement, LOAD DATA and EXPLAIN ANALYZE run what the engine does not read; a procedure may commit.
        _ => {
            unread(capabilities);
            capabilities.transaction = true;
            if word == "load" {
                capabilities.external = true;
            }
        }
    }
}

/// What a statement that reads a table named by its schema may do, as far as the engine says: any
/// table, since the catalog's walk keeps to the session's database.
pub fn by_schema() -> Capabilities {
    let mut capabilities = Capabilities::default();
    unread(&mut capabilities);
    capabilities
}

/// Code the engine does not read, which may touch any table, change what the database defines or
/// the session, and reach past it.
fn unread(capabilities: &mut Capabilities) {
    capabilities.unknown_tables();
    capabilities.ddl = true;
    capabilities.session = true;
    capabilities.external = true;
}
