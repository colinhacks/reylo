//! A JSON_TABLE column of a character type that names no character set or collation takes the
//! connection's, with the coercibility of a table's column, so comparing it with a column of
//! another collation is an error ("Illegal mix of collations", 1267). mysql2 connects with
//! `utf8mb4_unicode_ci` and MySQL 8 makes its databases `utf8mb4_0900_ai_ci`, so such a column
//! compared with a table's text column fails as both are set up out of the box.
//!
//! The compiled statement gives each such column the database's collation in its definition, as
//! the expansion of a `rows(…)` parameter gives its own JSON_TABLE's (`structured.rs`), where the
//! database's character set is `utf8mb4`, JSON's own: a column of another character set would
//! convert the JSON's text into it, so there the column is left as written. The column keeps a
//! column's coercibility, so it compares with a table's column of the database's collation, and a
//! column of another collation decides as MySQL lets it, as before (a `_bin` one wins; one of the
//! connection's collation is refused). A type that names its own character set or collation says
//! what it wants and is left as written, and so is a JSON_TABLE the expansion wrote, whose names
//! start with `reylo.`. MariaDB compares its JSON_TABLE's text with any column already, and is
//! left alone.
//!
//! A CAST or a CONVERT to CHAR takes the connection's collation too, but no expression can be
//! given a collation without overriding a column's in a comparison, which would change what an
//! equality on a binary column matches: Reylo writes `:p::text` as the bare placeholder instead,
//! which takes the collation of the column it meets (`structured.rs`).

use crate::lex::Rewritten;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tok {
    Word,
    Open,
    Close,
    Comma,
    /// A string, a quoted name or a comment, as the lexer reads them.
    Opaque,
    Other,
}

#[derive(Debug, Clone, Copy)]
struct Token {
    tok: Tok,
    start: usize,
    end: usize,
}

fn tokens(sql: &str, opaque: &[(usize, usize)]) -> Vec<Token> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut next_opaque = opaque.iter().peekable();
    while i < b.len() {
        while next_opaque.peek().is_some_and(|&&(_, end)| end <= i) {
            next_opaque.next();
        }
        if let Some(&&(start, end)) = next_opaque.peek() {
            if start <= i {
                out.push(Token { tok: Tok::Opaque, start, end });
                i = end;
                continue;
            }
        }
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80;
        let (tok, end) = match b[i] {
            b'(' => (Tok::Open, i + 1),
            b')' => (Tok::Close, i + 1),
            b',' => (Tok::Comma, i + 1),
            c if c.is_ascii_whitespace() => {
                i += 1;
                continue;
            }
            c if word(c) => (Tok::Word, i + b[i..].iter().take_while(|c| word(**c)).count()),
            _ => (Tok::Other, i + 1),
        };
        out.push(Token { tok, start: i, end });
        i = end;
    }
    out
}

/// Where each JSON_TABLE column of a character type that names no character set or collation
/// ends its type, in text order: the offsets the database's are written after. `opaque` is the
/// lexer's strings, quoted names and comments.
pub fn sites(sql: &str, opaque: &[(usize, usize)]) -> Vec<usize> {
    let t = tokens(sql, opaque);
    let mut out = Vec::new();
    for k in 0..t.len() {
        if !(is(&t, sql, k, "json_table") && t.get(k + 1).is_some_and(|x| x.tok == Tok::Open)) {
            continue;
        }
        let Some(end) = close_of(&t, k + 1) else { continue };
        // The expansion of a `rows(…)` parameter reads its own columns through CONVERT …
        // COLLATE, and the run time may write its lists again by offset.
        if t[k + 1..end].iter().any(|x| x.tok == Tok::Opaque && sql[x.start..x.end].starts_with("`reylo.")) {
            continue;
        }
        let mut depth = 0usize;
        for j in k + 2..end {
            match t[j].tok {
                Tok::Open => depth += 1,
                Tok::Close => depth -= 1,
                _ => {}
            }
            if depth == 0 && is(&t, sql, j, "columns") && t.get(j + 1).is_some_and(|x| x.tok == Tok::Open) {
                columns(&t, sql, j + 1, &mut out);
                break;
            }
        }
    }
    out.sort_unstable();
    out
}

fn is(t: &[Token], sql: &str, k: usize, w: &str) -> bool {
    t.get(k).is_some_and(|x| x.tok == Tok::Word && sql[x.start..x.end].eq_ignore_ascii_case(w))
}

/// The token that closes the parenthesis opening at `k`.
fn close_of(t: &[Token], k: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (j, x) in t.iter().enumerate().skip(k) {
        match x.tok {
            Tok::Open => depth += 1,
            Tok::Close => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

/// A COLUMNS list from its parenthesis at `k`: each column, and each NESTED one's.
fn columns(t: &[Token], sql: &str, k: usize, out: &mut Vec<usize>) {
    let Some(end) = close_of(t, k) else { return };
    let mut items = Vec::new();
    let (mut from, mut depth) = (k + 1, 0usize);
    for j in k + 1..end {
        match t[j].tok {
            Tok::Open => depth += 1,
            Tok::Close => depth -= 1,
            Tok::Comma if depth == 0 => {
                items.push((from, j));
                from = j + 1;
            }
            _ => {}
        }
    }
    items.push((from, end));
    for (from, to) in items {
        if from >= to {
            continue;
        }
        if is(t, sql, from, "nested") {
            if let Some(j) = (from..to).find(|&j| is(t, sql, j, "columns") && t.get(j + 1).is_some_and(|x| x.tok == Tok::Open)) {
                columns(t, sql, j + 1, out);
            }
            continue;
        }
        // `name type [EXISTS] PATH '…' …`; `name FOR ORDINALITY` has no PATH.
        let mut depth = 0usize;
        let Some(path) = (from + 1..to).find(|&j| {
            match t[j].tok {
                Tok::Open => depth += 1,
                Tok::Close => depth = depth.saturating_sub(1),
                _ => {}
            }
            depth == 0 && (is(t, sql, j, "path") || is(t, sql, j, "exists"))
        }) else {
            continue;
        };
        let words: Vec<String> = t[from + 1..path].iter().filter(|x| x.tok == Tok::Word).map(|x| sql[x.start..x.end].to_ascii_lowercase()).collect();
        let text = matches!(words.first().map(String::as_str), Some("char" | "character" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext"));
        let own = words.iter().skip(1).any(|w| matches!(w.as_str(), "character" | "charset" | "collate" | "binary" | "ascii" | "unicode" | "byte"));
        if text && !own {
            out.push(t[path - 1].end);
        }
    }
}

/// `rewritten` with ` COLLATE <collation>` written at each site, which gives the column the
/// collation's character set too: its placeholders keep their places, and each text it adds is
/// listed in `inserted`. `sites` are offsets of the text `rewritten` was made from.
pub fn written(rewritten: Rewritten, sites: &[usize], collation: &str) -> Rewritten {
    if sites.is_empty() {
        return rewritten;
    }
    let clause = format!(" COLLATE {collation}");
    // The rewritten offset of an original one: every placeholder before it changed its length.
    let at = |p: usize| {
        let delta: isize = rewritten.spans.iter().filter(|&&(_, oe, _, _)| oe <= p).map(|&(os, oe, rs, re)| (re - rs) as isize - (oe - os) as isize).sum();
        (p as isize + delta) as usize
    };
    let mut sql = String::with_capacity(rewritten.sql.len() + sites.len() * clause.len());
    let mut inserted = Vec::with_capacity(sites.len());
    let mut from = 0;
    for &original in sites {
        let to = at(original);
        sql.push_str(&rewritten.sql[from..to]);
        inserted.push((original, sql.len(), sql.len() + clause.len()));
        sql.push_str(&clause);
        from = to;
    }
    sql.push_str(&rewritten.sql[from..]);
    // Each placeholder after an added text moved by its length.
    let spans = rewritten.spans.iter().map(|&(os, oe, rs, re)| {
        let shift = sites.iter().filter(|&&original| original <= os).count() * clause.len();
        (os, oe, rs + shift, re + shift)
    });
    let spans = spans.collect();
    let mut all = rewritten.inserted;
    all.extend(inserted);
    all.sort_by_key(|&(_, rs, _)| rs);
    Rewritten { sql, order: rewritten.order, spans, inserted: all }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::Dialect;
    use crate::lex::{self, Target};

    fn compiled(sql: &str) -> String {
        let lexed = lex::lex(sql, Dialect::Mysql).unwrap();
        written(lex::rewrite(sql, &lexed, Target::Question), &sites(sql, &lexed.opaque), "utf8mb4_0900_ai_ci").sql
    }

    #[test]
    fn a_json_table_text_column_takes_the_databases_collation() {
        assert_eq!(
            compiled("select u.id from users u join json_table(:names, '$[*]' columns (n varchar(40) path '$', k int path '$.k', t text exists path '$.t')) j on u.name = j.n"),
            "select u.id from users u join json_table(?, '$[*]' columns (n varchar(40) COLLATE utf8mb4_0900_ai_ci path '$', k int path '$.k', t text COLLATE utf8mb4_0900_ai_ci exists path '$.t')) j on u.name = j.n"
        );
        // A NESTED path's columns too; an ordinality column, and a number, take nothing.
        assert_eq!(
            compiled("select * from json_table(:d, '$' columns (id for ordinality, nested path '$.tags[*]' columns (tag char(10) path '$'))) t"),
            "select * from json_table(?, '$' columns (id for ordinality, nested path '$.tags[*]' columns (tag char(10) COLLATE utf8mb4_0900_ai_ci path '$'))) t"
        );
    }

    #[test]
    fn what_names_its_own_character_set_or_collation_and_what_is_no_json_table_column_is_left_as_written() {
        for sql in [
            "select * from json_table(:d, '$' columns (n varchar(40) collate utf8mb4_bin path '$')) t",
            "select * from json_table(:d, '$' columns (n varchar(40) character set latin1 path '$')) t",
            "select * from json_table(:d, '$' columns (n varchar(40) charset ascii path '$')) t",
            "select * from json_table(:d, '$' columns (n char(4) binary path '$')) t",
            "select * from json_table(:d, '$[*]' columns (`reylo.n` for ordinality, `x` varchar(9) path '$.x')) as `reylo.j`",
            "select cast(:e as char), convert(:e, char(20))",
            "select 'json_table(x, ''$'' columns (n text path ''$''))'",
        ] {
            let lexed = lex::lex(sql, Dialect::Mysql).unwrap();
            assert!(sites(sql, &lexed.opaque).is_empty(), "{sql}");
        }
    }

    #[test]
    fn the_placeholders_and_the_added_text_map_back_to_the_authors_text() {
        let sql = "select j.n from json_table(:doc, '$' columns (n text path '$.n')) j where j.n <> :n";
        let lexed = lex::lex(sql, Dialect::Mysql).unwrap();
        let r = written(lex::rewrite(sql, &lexed, Target::Question), &sites(sql, &lexed.opaque), "utf8mb4_0900_ai_ci");
        for &(os, oe, rs, re) in &r.spans {
            assert_eq!(&r.sql[rs..re], "?");
            assert!(sql[os..oe].starts_with(':'));
        }
        assert_eq!(r.inserted.len(), 1);
        let (at, start, end) = r.inserted[0];
        assert_eq!(&r.sql[start..end], " COLLATE utf8mb4_0900_ai_ci");
        assert_eq!(&sql[at..at + 7], " path '");
        // An offset inside the added text is the place it was added at; one after it is where it
        // was in the author's text.
        assert_eq!(r.to_original(start + 4), at);
        assert_eq!(r.to_original(r.sql.find("<> ?").unwrap()), sql.find("<> :n").unwrap());
    }

    #[test]
    fn the_engines_parser_reads_a_json_table_columns_character_set_and_collation_and_prints_them() {
        use sqlparser::dialect::MySqlDialect;
        use sqlparser::parser::Parser;
        // The written statement, and one that names a character set too, keep their tree, which
        // the row count, the JSON shapes and the probes are read from.
        for sql in [
            compiled("select j.n from json_table(:d, '$[*]' columns (n text path '$')) j"),
            "select j.n from json_table(?, '$[*]' columns (n varchar(40) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin EXISTS PATH '$')) AS j".to_string(),
        ] {
            let parsed = Parser::parse_sql(&MySqlDialect {}, &sql).unwrap_or_else(|e| panic!("{sql}: {e}"));
            let printed = parsed[0].to_string();
            assert!(printed.contains("COLLATE utf8mb4_"), "{printed}");
            assert_eq!(Parser::parse_sql(&MySqlDialect {}, &printed).unwrap()[0].to_string(), printed);
        }
    }
}
