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

/// Each call of one of `names` written with an odd number of arguments: the constructors of a JSON
/// object take keys and values in pairs, and a database prepares a call with an odd count and
/// refuses it only when it runs. Read from the text outside the lexer's strings and comments
/// ([`Lexed::opaque`]); a name qualified with a schema is somebody else's function, and a
/// `VARIADIC` argument is an array whose length the text does not say.
pub fn odd_pairs(sql: &str, lexed: &Lexed, names: &[&str]) -> Vec<Diagnostic> {
    let bytes = sql.as_bytes();
    let opaque_at = |i: usize| lexed.opaque.iter().find(|(s, e)| *s <= i && i < *e).map(|(_, e)| *e);
    // Postgres and SQLite read every byte past ASCII as part of a name (`café`, `éjson_build_object`).
    // The scan steps byte by byte and compares bytes, so it never slices inside a character.
    let is_name = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80;
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(end) = opaque_at(i) {
            i = end;
            continue;
        }
        let starts_word = is_name(bytes[i]) && (i == 0 || !(is_name(bytes[i - 1]) || bytes[i - 1] == b'.'));
        let Some(name) = names.iter().find(|n| starts_word && bytes.get(i..i + n.len()).is_some_and(|w| w.eq_ignore_ascii_case(n.as_bytes())) && !bytes.get(i + n.len()).is_some_and(|b| is_name(*b))) else {
            i += 1;
            continue;
        };
        let mut j = i + name.len();
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if bytes.get(j) != Some(&b'(') {
            i += name.len();
            continue;
        }
        // The arguments: the commas at the call's own depth, past every string and comment.
        let (mut depth, mut commas, mut empty, mut variadic, mut k) = (0usize, 0usize, true, false, j);
        while k < bytes.len() {
            if let Some(end) = opaque_at(k) {
                if depth == 1 {
                    empty = false;
                }
                k = end;
                continue;
            }
            match bytes[k] {
                b'(' | b'[' => {
                    if depth == 1 {
                        empty = false;
                    }
                    depth += 1;
                }
                b')' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b',' if depth == 1 => commas += 1,
                b if depth == 1 && !b.is_ascii_whitespace() => {
                    empty = false;
                    if !is_name(bytes[k - 1]) && bytes.get(k..k + 8).is_some_and(|w| w.eq_ignore_ascii_case(b"variadic")) && !bytes.get(k + 8).is_some_and(|b| is_name(*b)) {
                        variadic = true;
                    }
                }
                _ => {}
            }
            k += 1;
        }
        let count = if empty { 0 } else { commas + 1 };
        if k < bytes.len() && !variadic && count % 2 == 1 {
            let written = &sql[i..i + name.len()];
            found.push(diag(format!("{written} takes keys and values in pairs, and this call has {count} argument{}, which the database refuses when it runs.", if count == 1 { "" } else { "s" }), i, name.len(), 15));
        }
        i = j + 1;
    }
    found
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

    #[test]
    fn a_json_object_of_an_odd_count_of_arguments_is_an_error_at_its_name() {
        let names = ["json_build_object", "jsonb_build_object"];
        let at = |sql: &str| {
            let lexed = lex::lex(sql, Dialect::Postgres).unwrap();
            odd_pairs(sql, &lexed, &names).iter().map(|d| (sql[d.start..d.start + d.length].to_string(), d.message.clone(), d.code)).collect::<Vec<_>>()
        };
        let found = at("select json_build_object('a', 1, 'b'), JSONB_BUILD_OBJECT('k')");
        assert_eq!(found.iter().map(|(w, _, code)| (w.as_str(), *code)).collect::<Vec<_>>(), [("json_build_object", 15), ("JSONB_BUILD_OBJECT", 15)]);
        assert_eq!(found[0].1, "json_build_object takes keys and values in pairs, and this call has 3 arguments, which the database refuses when it runs.");
        assert!(found[1].1.contains("this call has 1 argument,"));
        // Pairs, nested calls, commas inside strings, arrays and comments, no arguments at all,
        // a VARIADIC array, another schema's function and a name inside a string are all fine.
        for sql in [
            "select json_build_object('a', f(1, 2), 'b,c', array[1, 2])",
            "select json_build_object()",
            "select json_build_object( )",
            "select jsonb_build_object(variadic array['a', 'b'])",
            "select app.json_build_object('a')",
            "select 'json_build_object(1)' as t",
            "select json_build_object('a', /* , */ 1)",
            "select json_build_object('a', café, 'b', 1)",
            "select éjson_build_object('a')",
            "select json_build_objecté('a')",
        ] {
            assert!(at(sql).is_empty(), "{sql}");
        }
        // A name past ASCII among the arguments is one argument, and the scan never splits it.
        assert_eq!(at("select json_build_object('a', café, 'b')").len(), 1);
        assert_eq!(at("select json_build_object(variadicé, 'a', 1)").len(), 1);
    }
}
