//! What every database's analysis starts from: the statement's placeholders, and the bare analysis
//! a statement gets when nothing more can be said of it.

use crate::api::{Analysis, Diagnostic, Param};
use crate::dialect::Dialect;
use crate::lex::{self, Lexed, Style};
use crate::types::Type;

/// The statement's placeholders, or the bare analysis that reports why they cannot be read: a
/// lex error, named and positional parameters mixed, or a parameter used more than once.
pub fn lexed(sql: &str, dialect: Dialect) -> Result<Lexed, Analysis> {
    let lexed = match lex::lex(sql, dialect) {
        Ok(l) => l,
        Err(e) => return Err(bare(sql, &none(), dialect, vec![diag(e.message, e.offset, 1, 1)])),
    };
    if lexed.style == Style::Mixed {
        let first = lexed.placeholders.iter().find(|p| lex::is_positional(&p.name)).expect("a mixed statement has a positional placeholder");
        let d = diag("Named and positional parameters cannot be mixed in one statement.", first.start, first.end - first.start, 2);
        return Err(bare(sql, &lexed, dialect, vec![d]));
    }
    let repeated = repeated(sql, &lexed);
    if !repeated.is_empty() {
        return Err(bare(sql, &lexed, dialect, repeated));
    }
    Ok(lexed)
}

/// Each later use of a parameter, as the diagnostic at its place ([`lex::repeated`]).
pub fn repeated(sql: &str, lexed: &Lexed) -> Vec<Diagnostic> {
    lex::repeated(sql, lexed).into_iter().map(|r| diag(r.message, r.start, r.end - r.start, 3)).collect()
}

/// Two result columns of one name. A driver builds each row as an object keyed by name, so it
/// keeps the last of them, and nothing a type says of the first is true of the row. One diagnostic
/// at each later column of a name: at the bytes `at` gives for that column (its index in
/// `analysis.columns`), which the database's own parse knows, else on the statement's first word.
pub fn clashes(sql: &str, analysis: &mut Analysis, at: &dyn Fn(usize) -> Option<(usize, usize)>) {
    let mut found = Vec::new();
    for (i, column) in analysis.columns.iter().enumerate() {
        if let Some(first) = analysis.columns[..i].iter().position(|c| c.name == column.name) {
            let (start, length) = at(i).unwrap_or_else(|| first_word(sql));
            let message = format!("Columns {} and {} are both named \"{}\", and a row keeps only the last of them. Give one of them another name with `as`.", first + 1, i + 1, column.name);
            found.push(diag(message, start, length, 14));
        }
    }
    analysis.diagnostics.extend(found);
}

/// The statement's first word, past any whitespace.
fn first_word(sql: &str) -> (usize, usize) {
    let start = sql.len() - sql.trim_start().len();
    let length = sql[start..].bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
    (start, length.max(1))
}

/// A diagnostic at least one byte long.
pub fn diag(message: impl Into<String>, start: usize, length: usize, code: u32) -> Diagnostic {
    Diagnostic { message: message.into(), start, length: length.max(1), code }
}

/// A statement with no placeholders read.
pub fn none() -> Lexed {
    Lexed { params: vec![], placeholders: vec![], style: Style::None, opaque: vec![] }
}

/// What the engine answers when it cannot describe: every parameter unknown, no columns, and
/// `many` rows.
pub fn bare(sql: &str, lexed: &Lexed, dialect: Dialect, diagnostics: Vec<Diagnostic>) -> Analysis {
    let rewritten = lex::rewrite(sql, lexed, dialect.binds());
    Analysis {
        kind: String::new(),
        params: lexed.params.iter().map(|n| Param { name: n.clone(), ty: Type::Unknown, type_name: String::new(), nullable: false, schema: None, cast: None }).collect(),
        columns: Vec::new(),
        cardinality: "many".to_string(),
        cardinality_reason: None,
        rewritten,
        style: lexed.style,
        diagnostics,
        unresolved: Vec::new(),
        structured: Vec::new(),
        fresh: None,
        tables: None,
    }
}

/// The bare analysis of `sql` with `diagnostics`, its parameters as far as the lexer reads them.
pub fn bare_of(sql: &str, dialect: Dialect, diagnostics: Vec<Diagnostic>) -> Analysis {
    let lexed = lex::lex(sql, dialect).unwrap_or_else(|_| none());
    bare(sql, &lexed, dialect, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Column;

    #[test]
    fn two_columns_of_one_name_are_an_error_at_the_later() {
        let mut a = bare_of("select a.x, b.x, a.x from a, b", Dialect::Postgres, Vec::new());
        a.columns = ["x", "x", "x"].iter().map(|n| Column { name: n.to_string(), ty: Type::Unknown, nullable: true, type_name: String::new() }).collect();
        clashes("  select a.x, b.x, a.x from a, b", &mut a, &|i| (i == 1).then_some((14, 1)));
        assert_eq!(a.diagnostics.iter().map(|d| (d.code, d.start, d.length)).collect::<Vec<_>>(), [(14, 14, 1), (14, 2, 6)]);
        assert_eq!(a.diagnostics[0].message, "Columns 1 and 2 are both named \"x\", and a row keeps only the last of them. Give one of them another name with `as`.");
        assert!(a.diagnostics[1].message.starts_with("Columns 1 and 3 are both named"));
        let mut b = bare_of("select 1 as x, 2 as y", Dialect::Postgres, Vec::new());
        b.columns = ["x", "y"].iter().map(|n| Column { name: n.to_string(), ty: Type::Unknown, nullable: true, type_name: String::new() }).collect();
        clashes("select 1 as x, 2 as y", &mut b, &|_| None);
        assert!(b.diagnostics.is_empty());
    }
}
