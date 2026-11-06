//! What every database's analysis starts from: the statement's placeholders, and the bare analysis
//! a statement gets when nothing more can be said of it.

use crate::api::{Analysis, Diagnostic, Param};
use crate::dialect::Dialect;
use crate::lex::{self, Lexed, Style};
use crate::types::{Cardinality, Type};

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

/// How a statement's select list or `RETURNING` writes a result column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultTarget {
    /// A `*`, which stands for any number of columns, each named by what it reads.
    Star,
    /// An alias, or a column read as it is: the statement says the name.
    Named,
    /// An expression with no alias: the statement says no name, and the database makes one up. With
    /// the bytes it is written at, where the parse knows them.
    Unnamed(Option<(usize, usize)>),
}

/// The diagnostic code of an unnamed result column whose made-up name is no identifier.
pub const UNNAMED_COLUMN: u32 = 23;

/// Whether a made-up name reads as a name (`count`, `lower`) rather than as an expression's text
/// (`count(*)`, `?column?`, `1 + 1`).
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_') && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// The rules on the names of a statement's result columns, once its columns are known. `targets`
/// is its select list or `RETURNING` as its database's parse reads it, the first arm's of a set
/// operation, whose names are the result's; `None` where the parse read none. `at` gives the bytes
/// a column is written at (its index in `analysis.columns`) where the parse knows them.
///
/// The name of an expression with no alias is not the statement's to rely on: it differs by
/// database, and on some by the driver, the statement's spacing or the value bound (`id + ?` and
/// `id + 5` on MySQL). So a statement whose one column is such an expression gives its values, with
/// no name at all ([`Analysis::value`]), and among several columns such a name that is no
/// identifier is an error. Two columns of one name are an error too, since a driver builds each row
/// as an object keyed by name, which keeps the last of them, and nothing a type says of the first
/// is true of the row.
pub fn names(sql: &str, analysis: &mut Analysis, targets: Option<&[ResultTarget]>, at: &dyn Fn(usize) -> Option<(usize, usize)>) {
    // Only a list of this one expression: a `*` beside it may stand for no column today and for one
    // once the table has one.
    analysis.value = analysis.columns.len() == 1 && matches!(targets, Some([ResultTarget::Unnamed(_)]));
    let placed = placed_columns(analysis.columns.len(), targets);
    let mut found = Vec::new();
    let mut refused = vec![false; analysis.columns.len()];
    if !analysis.value {
        for (i, column) in analysis.columns.iter().enumerate() {
            let Some(ResultTarget::Unnamed(place)) = placed.get(i) else { continue };
            if is_identifier(&column.name) {
                continue;
            }
            refused[i] = true;
            // An expression's text is what MySQL's and SQLite's names are, so a parse that places
            // nothing can still find it, the second column of one name at its second place.
            let nth = analysis.columns[..i].iter().enumerate().filter(|(k, c)| refused[*k] && c.name == column.name).count();
            let (start, length) = place.or_else(|| at(i)).or_else(|| written_at(sql, &column.name, nth)).unwrap_or_else(|| first_word(sql));
            let message = format!("Column {} has no name: the database calls it \"{}\", which is no identifier and may differ on another driver or server. Give it one with `as`.", i + 1, column.name);
            found.push(diag(message, start, length, UNNAMED_COLUMN));
        }
    }
    for (i, column) in analysis.columns.iter().enumerate() {
        if refused[i] {
            continue;
        }
        if let Some(first) = analysis.columns[..i].iter().enumerate().position(|(k, c)| c.name == column.name && !refused[k]) {
            let (start, length) = at(i).unwrap_or_else(|| first_word(sql));
            let message = format!("Columns {} and {} are both named \"{}\", and a row keeps only the last of them. Give one of them another name with `as`.", first + 1, i + 1, column.name);
            found.push(diag(message, start, length, 14));
        }
    }
    analysis.diagnostics.extend(found);
}

/// Each result column's target, as far as the targets place it: from the left up to the first `*`,
/// and from the right back to the last; `Named` for what stands for a `*` or between two of them.
/// Empty where the targets cannot be the columns' (a list that expands some other way).
fn placed_columns(columns: usize, targets: Option<&[ResultTarget]>) -> Vec<ResultTarget> {
    let Some(targets) = targets else { return Vec::new() };
    let mut placed = vec![ResultTarget::Named; columns];
    match targets.iter().position(|t| *t == ResultTarget::Star) {
        None if targets.len() != columns => return Vec::new(),
        None => placed.copy_from_slice(targets),
        Some(_) if targets.len() - 1 > columns => return Vec::new(),
        Some(first) => {
            placed[..first].copy_from_slice(&targets[..first]);
            for (from_end, t) in targets.iter().rev().take_while(|t| **t != ResultTarget::Star).enumerate() {
                placed[columns - 1 - from_end] = *t;
            }
        }
    }
    placed
}

/// Where `name` stands in `sql` as code, the `nth` time: not in a string or a comment, and not
/// inside a longer word.
fn written_at(sql: &str, name: &str, nth: usize) -> Option<(usize, usize)> {
    let bytes = sql.as_bytes();
    let wanted = name.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80;
    let (first, last) = (*wanted.first()?, *wanted.last()?);
    let (mut i, mut seen) = (0, 0);
    while i < bytes.len() {
        if bytes[i..].starts_with(wanted) && (!word(first) || i == 0 || !word(bytes[i - 1])) && (!word(last) || !bytes.get(i + wanted.len()).copied().is_some_and(word)) {
            if seen == nth {
                return Some((i, wanted.len()));
            }
            seen += 1;
            i += wanted.len();
            continue;
        }
        match bytes[i] {
            quote @ (b'\'' | b'"' | b'`') => {
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                i += 1;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < bytes.len() && !bytes[i..].starts_with(b"*/") {
                    i += 1;
                }
                i += 2;
            }
            _ => i += 1,
        }
    }
    None
}

/// The bytes of the statement as written that a span of the rewritten statement stands for: lines
/// and columns from 1, as `sqlparser` counts them, line 0 for a span it does not know.
pub fn written_span(rewritten: &lex::Rewritten, start: (u64, u64), end: (u64, u64)) -> Option<(usize, usize)> {
    let at = |(line, column): (u64, u64)| -> Option<usize> {
        if line == 0 {
            return None;
        }
        let (mut l, mut c) = (1, 1);
        for (i, ch) in rewritten.sql.char_indices() {
            if (l, c) == (line, column) {
                return Some(i);
            }
            if ch == '\n' {
                l += 1;
                c = 1;
            } else {
                c += 1;
            }
        }
        ((l, c) == (line, column)).then_some(rewritten.sql.len())
    };
    let (from, to) = (rewritten.to_original(at(start)?), rewritten.to_original(at(end)?));
    (to > from).then_some((from, to - from))
}

/// The statement's first word, past any whitespace.
fn first_word(sql: &str) -> (usize, usize) {
    let start = sql.len() - sql.trim_start().len();
    let length = sql[start..].bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'_').count();
    (start, length.max(1))
}

/// The diagnostic code of a parameter that is a whole sort, group or partition key.
pub const CONSTANT_KEY: u32 = 17;

/// The clause a key stands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyClause {
    /// A statement's, a window's or an aggregate's `ORDER BY`.
    OrderBy,
    GroupBy,
    /// A window's `PARTITION BY`.
    PartitionBy,
    /// Postgres's `DISTINCT ON (…)`.
    DistinctOn,
}

/// The error of a parameter that is, whole or under casts, a key of `clause`: the placeholder
/// written at `start..end` of `sql`, in a clause that has no other key when `alone`. A parameter
/// is a value, the same for every row, so such a key sorts nothing, puts every row in one group or
/// partition, or keeps one row; a parameter chooses a column through an expression that holds it,
/// such as a `case`, which stays allowed.
pub fn constant_key(sql: &str, start: usize, end: usize, clause: KeyClause, alone: bool) -> Diagnostic {
    // On MySQL and SQLite the author's placeholder may take in the `::type` written on it.
    let written = sql.get(start..end).unwrap_or("").split("::").next().unwrap_or("").trim_end();
    let length = if written.is_empty() { end - start } else { written.len() };
    let written = if written.is_empty() { "the parameter" } else { written };
    let effect = match (clause, alone) {
        (KeyClause::OrderBy, _) => format!("order by {written} sorts every row by the same value"),
        (KeyClause::GroupBy, true) => format!("group by {written} puts every row in one group"),
        (KeyClause::GroupBy, false) => format!("group by {written} groups every row by the same value"),
        (KeyClause::PartitionBy, true) => format!("partition by {written} puts every row in one partition"),
        (KeyClause::PartitionBy, false) => format!("partition by {written} partitions every row by the same value"),
        (KeyClause::DistinctOn, true) => format!("distinct on ({written}) keeps a single row"),
        (KeyClause::DistinctOn, false) => format!("distinct on {written} compares every row by the same value"),
    };
    let message = format!("{effect}, since a parameter is a value and not a column's name; choose the column with case {written} when 'name' then name end");
    diag(message, start, length, CONSTANT_KEY)
}

/// A diagnostic at least one byte long.
pub fn diag(message: impl Into<String>, start: usize, length: usize, code: u32) -> Diagnostic {
    Diagnostic { message: message.into(), start, length: length.max(1), code, server_code: None }
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
        kind: None,
        comment: None,
        params: lexed.params.iter().map(|n| Param::new(n.clone(), Type::unknown())).collect(),
        columns: Vec::new(),
        cardinality: Cardinality::Many,
        cardinality_reason: None,
        key: None,
        unique: Vec::new(),
        order: Vec::new(),
        ordered: false,
        value: false,
        rewritten,
        style: lexed.style,
        diagnostics,
        unresolved: Vec::new(),
        structured: Vec::new(),
        fresh: None,
        errors: None,
        read_only: false,
        notes: Vec::new(),
        tables: None,
        domains: Default::default(),
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

    #[test]
    fn two_columns_of_one_name_are_an_error_at_the_later() {
        let mut a = bare_of("select a.x, b.x, a.x from a, b", Dialect::Postgres, Vec::new());
        a.columns = ["x", "x", "x"].iter().map(|n| Column::new(*n, Type::unknown(), true)).collect();
        names("  select a.x, b.x, a.x from a, b", &mut a, None, &|i| (i == 1).then_some((14, 1)));
        assert_eq!(a.diagnostics.iter().map(|d| (d.code, d.start, d.length)).collect::<Vec<_>>(), [(14, 14, 1), (14, 2, 6)]);
        assert_eq!(a.diagnostics[0].message, "Columns 1 and 2 are both named \"x\", and a row keeps only the last of them. Give one of them another name with `as`.");
        assert!(a.diagnostics[1].message.starts_with("Columns 1 and 3 are both named"));
        let mut b = bare_of("select 1 as x, 2 as y", Dialect::Postgres, Vec::new());
        b.columns = ["x", "y"].iter().map(|n| Column::new(*n, Type::unknown(), true)).collect();
        names("select 1 as x, 2 as y", &mut b, None, &|_| None);
        assert!(b.diagnostics.is_empty());
    }

    /// The analysis of `sql` once its columns are named `columns`, with `targets` as its parse read them.
    fn named(sql: &str, columns: &[&str], targets: Option<&[ResultTarget]>) -> Analysis {
        let mut a = bare_of(sql, Dialect::Postgres, Vec::new());
        a.columns = columns.iter().map(|n| Column::new(*n, Type::unknown(), true)).collect();
        names(sql, &mut a, targets, &|_| None);
        a
    }

    #[test]
    fn one_unnamed_column_is_the_result_and_any_other_stays_a_row() {
        use ResultTarget::*;
        const UNNAMED: ResultTarget = Unnamed(None);
        for name in ["count", "count(*)", "?column?", "1 + 1"] {
            let a = named("select x", &[name], Some(&[UNNAMED]));
            assert!(a.value, "{name}");
            assert!(a.diagnostics.is_empty(), "{name}");
        }
        assert!(!named("select x as x", &["x"], Some(&[Named])).value);
        assert!(!named("select * from t", &["x"], Some(&[Star])).value);
        assert!(!named("select x, y", &["x", "y"], Some(&[UNNAMED, UNNAMED])).value);
        // A `*` that stands for no column today beside one expression stays rows.
        assert!(!named("select *, 1 + 1 from t", &["x"], Some(&[Star, UNNAMED])).value);
        // A statement no parse read, and a list that is not the columns' (DuckDB's `columns(…)`).
        assert!(!named("select x", &["x"], None).value);
        assert!(!named("select x, y", &["x"], Some(&[UNNAMED, UNNAMED])).value);
    }

    #[test]
    fn among_several_columns_a_made_up_name_that_is_no_identifier_is_an_error() {
        use ResultTarget::*;
        const UNNAMED: ResultTarget = Unnamed(None);
        let codes = |a: &Analysis| a.diagnostics.iter().map(|d| (d.code, d.start, d.length)).collect::<Vec<_>>();
        // Placed where the text holds the name, else on the first word.
        assert_eq!(codes(&named("select id, count(*) from t", &["id", "count(*)"], Some(&[Named, UNNAMED]))), [(UNNAMED_COLUMN, 11, 8)]);
        assert_eq!(codes(&named("select id, 1 + 1", &["id", "?column?"], Some(&[Named, UNNAMED]))), [(UNNAMED_COLUMN, 0, 6)]);
        assert_eq!(named("select id, 1 + 1", &["id", "?column?"], Some(&[Named, UNNAMED])).diagnostics[0].message, "Column 2 has no name: the database calls it \"?column?\", which is no identifier and may differ on another driver or server. Give it one with `as`.");
        // A name the database derives that reads as one, and a name the statement wrote.
        assert!(named("select id, count(*) from t", &["id", "count"], Some(&[Named, UNNAMED])).diagnostics.is_empty());
        assert!(named("select id, lower(n) from t", &["id", "größe_2$"], Some(&[Named, UNNAMED])).diagnostics.is_empty());
        assert!(named("select 1 as \"a b\", \"c d\" from t", &["a b", "c d"], Some(&[Named, Named])).diagnostics.is_empty());
        // No parse, no rule; a star's columns are named by what they read.
        assert!(named("select id, 1 + 1", &["id", "?column?"], None).diagnostics.is_empty());
        assert!(named("select * from t", &["a b", "c d"], Some(&[Star])).diagnostics.is_empty());
        // Either side of a star is placed from its own end, and what stands between two is not.
        assert_eq!(codes(&named("select 1 + 1, *, 2 + 2 from t", &["1 + 1", "a", "b", "2 + 2"], Some(&[UNNAMED, Star, UNNAMED]))).len(), 2);
        assert_eq!(named("select 1 + 1, *, 2 + 2 from t", &["a", "b", "2 + 2"], Some(&[Named, Star, UNNAMED])).diagnostics[0].message.split(':').next(), Some("Column 3 has no name"));
        assert!(named("select *, 1 + 1, * from t", &["a", "1 + 1", "b"], Some(&[Star, UNNAMED, Star])).diagnostics.is_empty());
        // Two refused columns of one name take that one error each, and no clash on top.
        let two = named("select 1 + 1, 2 + 2", &["?column?", "?column?"], Some(&[UNNAMED, UNNAMED]));
        assert_eq!(two.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [UNNAMED_COLUMN, UNNAMED_COLUMN]);
        // Two names that read as names still clash.
        let clash = named("select lower(a), lower(b)", &["lower", "lower"], Some(&[UNNAMED, UNNAMED]));
        assert_eq!(clash.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [14]);
        // The parse's place comes first, and a name's text is looked for as code.
        assert_eq!(codes(&named("select id, x", &["id", "a + b"], Some(&[Named, Unnamed(Some((11, 1)))]))), [(UNNAMED_COLUMN, 11, 1)]);
        assert_eq!(codes(&named("select 11, 1", &["a", "1"], Some(&[Named, UNNAMED]))), [] as [(u32, usize, usize); 0]);
        assert_eq!(codes(&named("select 'count(*)', count(*) from t", &["count(*)", "b"], Some(&[UNNAMED, Named]))), [(UNNAMED_COLUMN, 18, 8)]);
        assert_eq!(codes(&named("select /* count(*) */ id, count(*) from t", &["id", "count(*)"], Some(&[Named, UNNAMED]))), [(UNNAMED_COLUMN, 26, 8)]);
        assert_eq!(codes(&named("select a + 1, a + 1 from t", &["a + 1", "a + 1"], Some(&[UNNAMED, UNNAMED]))), [(UNNAMED_COLUMN, 7, 5), (UNNAMED_COLUMN, 14, 5)]);
        // Not inside a longer word.
        assert_eq!(written_at("select 11, 1", "1", 0), Some((11, 1)));
    }
}
