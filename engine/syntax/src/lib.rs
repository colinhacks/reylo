//! Lossless SQL syntax trees. Every byte of a statement's text belongs to one token of its tree or
//! to the whitespace and comments before one, so a tree prints back the text it was read from, and
//! an invalid statement still has a tree, with what could not be read under error nodes.

pub mod lalr;
pub mod lemon;
pub mod postgres;
pub mod sqlite;
pub mod tolerant;
pub mod tree;

pub use tree::{Node, NodeId, SyntaxError, Tree, ERROR_KIND};

/// A database whose SQL this crate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dialect {
    Postgres,
    Sqlite,
    Mysql,
    ClickHouse,
    SqlServer,
}

/// Reads `text` into a lossless tree: every byte belongs to a token or the trivia before one, and text a dialect can't read stays in the tree, under an error node or with an error beside it. Postgres and SQLite follow the database's own grammar, so their errors are its syntax errors. MySQL, ClickHouse and SQL Server are read for their tokens, statements, clauses and parentheses, so their errors are only what doesn't balance or end.
pub fn parse(dialect: Dialect, text: &str) -> Tree<'_> {
    match dialect {
        Dialect::Postgres => postgres::parse_tree(text),
        Dialect::Sqlite => sqlite::parse_tree(text),
        Dialect::Mysql => tolerant::parse(tolerant::Flavor::MYSQL, text),
        Dialect::ClickHouse => tolerant::parse(tolerant::Flavor::ClickHouse, text),
        Dialect::SqlServer => tolerant::parse(tolerant::Flavor::SqlServer, text),
    }
}

/// Where each statement of `tree` starts and ends, its closing `;` left out.
pub fn statements(dialect: Dialect, tree: &Tree<'_>) -> Vec<(u32, u32)> {
    let name = match dialect {
        Dialect::Postgres => "toplevel_stmt",
        Dialect::Sqlite => "ecmd",
        Dialect::Mysql | Dialect::ClickHouse | Dialect::SqlServer => "statement",
    };
    let mut out = Vec::new();
    let mut stack = vec![tree.root];
    while let Some(id) = stack.pop() {
        let n = tree.node(id);
        if n.kind == ERROR_KIND || (!n.leaf && kind_name(dialect, n.kind) == name) {
            // Where its last token before any statement end stops.
            let is_end = |kind: u16| match dialect {
                Dialect::Postgres | Dialect::Sqlite => false,
                _ => kind == tolerant::SEMI,
            };
            let last = tree.leaves_of(id).map(|l| tree.node(l)).filter(|l| l.end > l.start && !is_end(l.kind)).last();
            if let Some(last) = last {
                let text = &tree.text[n.start as usize..last.end as usize];
                let kept = text.trim_end_matches(|c: char| c == ';' || c.is_whitespace()).len() as u32;
                if kept > 0 {
                    out.push((n.start, n.start + kept));
                }
            }
        } else {
            stack.extend(tree.children(id).iter().rev());
        }
    }
    out
}

/// The dialect's keywords, as its lexer spells them.
pub fn keywords(dialect: Dialect) -> Vec<&'static str> {
    match dialect {
        Dialect::Postgres => postgres::keywords().collect(),
        Dialect::Sqlite => sqlite::generated::KEYWORDS.iter().map(|k| k.0).collect(),
        Dialect::Mysql | Dialect::ClickHouse | Dialect::SqlServer => tolerant::KEYWORDS.to_vec(),
    }
}

/// The name of a node's kind in `dialect`'s trees.
pub fn kind_name(dialect: Dialect, kind: u16) -> &'static str {
    match dialect {
        Dialect::Postgres => postgres::kind_name(kind),
        Dialect::Sqlite => sqlite::kind_name(kind),
        Dialect::Mysql | Dialect::ClickHouse | Dialect::SqlServer => tolerant::kind_name(kind),
    }
}

/// The hash of this crate's sources (`build.rs`), which an engine's fingerprint combines with its own.
#[doc(hidden)]
pub const SOURCE_HASH: &str = env!("REYLO_ENGINE_SYNTAX_HASH");

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(dialect: Dialect, text: &str) -> Vec<&str> {
        statements(dialect, &parse(dialect, text)).into_iter().map(|(a, b)| &text[a as usize..b as usize]).collect()
    }

    #[test]
    fn a_statement_keeps_the_close_of_a_conditional_comment() {
        assert_eq!(spans(Dialect::Mysql, "SELECT 1 /*! + 2 */; select 3"), ["SELECT 1 /*! + 2 */", "select 3"]);
    }

    #[test]
    fn what_a_postgres_action_refuses_is_an_error_of_the_tree() {
        let text = "select 1 fetch first row with ties";
        let tree = parse(Dialect::Postgres, text);
        assert_eq!(tree.print(), text);
        assert_eq!(tree.errors[0].message, "WITH TIES cannot be specified without ORDER BY clause");
    }
}
