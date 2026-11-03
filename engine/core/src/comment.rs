//! The prose written in comments before a statement: its summary, as an editor's hover shows it
//! (the format's `comment`, `spec/type-descriptor.ts`).

use crate::dialect::Dialect;

/// The comments before a statement, as prose, and the byte offset where they end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leading {
    pub prose: String,
    pub end: usize,
}

/// The prose of the comments before a statement, where it has any: an analysis's `comment`, which
/// every host's analysis carries ([`crate::api::call`], [`crate::replay`]).
pub fn prose(sql: &str, dialect: Dialect) -> Option<String> {
    let prose = leading(sql, dialect).prose;
    (!prose.is_empty()).then_some(prose)
}

/// The `--` lines and `/* … */` blocks before the statement's first token, markers stripped and
/// lines kept: a blank line between two comments, or a change from one form to the other, starts a
/// paragraph. Read as the dialect reads comments: Postgres and DuckDB nest block comments, MySQL's
/// `--` is one only before a space or a control character and `#` starts one too, and MySQL's
/// `/*!` and `/*+` are code to the server, so they end the prose.
pub fn leading(sql: &str, dialect: Dialect) -> Leading {
    let bytes = sql.as_bytes();
    let mut paragraphs: Vec<Vec<String>> = Vec::new();
    let mut last: Option<bool> = None; // Some(true) after a line comment, Some(false) after a block
    let mut i = 0;
    let mut end = 0;
    loop {
        let ws = sql[i..].len() - sql[i..].trim_start().len();
        let blank_before = sql[i..i + ws].matches('\n').count() > 1;
        let at = i + ws;
        let (ch, next) = (bytes.get(at).copied(), bytes.get(at + 1).copied());
        let mysql = dialect == Dialect::Mysql;
        let dash = ch == Some(b'-') && next == Some(b'-') && !(mysql && bytes.get(at + 2).is_some_and(|&c| c > 32 && c != 127));
        if dash || (ch == Some(b'#') && mysql) {
            let stop = sql[at..].find('\n').map_or(sql.len(), |n| at + n);
            let text = &sql[at + if ch == Some(b'#') { 1 } else { 2 }..stop];
            if last != Some(true) || blank_before || paragraphs.is_empty() {
                paragraphs.push(Vec::new());
            }
            paragraphs.last_mut().expect("a paragraph").push(text.to_string());
            last = Some(true);
            i = stop;
            end = stop;
            continue;
        }
        if ch == Some(b'/') && next == Some(b'*') && !(mysql && matches!(bytes.get(at + 2), Some(b'!' | b'+'))) {
            let nests = matches!(dialect, Dialect::Postgres | Dialect::Duckdb);
            let (mut depth, mut j) = (1, at + 2);
            while j < bytes.len() && depth > 0 {
                if bytes[j..].starts_with(b"*/") {
                    depth -= 1;
                    j += 2;
                } else if nests && bytes[j..].starts_with(b"/*") {
                    depth += 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            let text = &sql[at + 2..if depth == 0 { j - 2 } else { sql.len() }];
            paragraphs.push(block(text));
            last = Some(false);
            i = j;
            end = j;
            continue;
        }
        break;
    }
    let prose = paragraphs.into_iter().map(dedent).filter(|p| !p.is_empty()).map(|p| p.join("\n")).collect::<Vec<_>>().join("\n\n");
    Leading { prose, end }
}

/// A block comment's lines, without a `/**`'s extra stars or the `*` that starts each line of one
/// written as JSDoc is.
fn block(text: &str) -> Vec<String> {
    let text = text.trim_start_matches('*');
    let lines: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let rest: Vec<&&str> = lines[1..].iter().filter(|l| !l.trim().is_empty()).collect();
    let starred = |l: &str| l.trim_start().starts_with('*');
    if !rest.is_empty() && rest.iter().all(|l| starred(l)) {
        let mut out = vec![lines[0].to_string()];
        for l in &lines[1..] {
            let trimmed = l.trim_start();
            out.push(match trimmed.strip_prefix('*') {
                Some(after) if !after.starts_with('/') => after.strip_prefix(' ').unwrap_or(after).to_string(),
                _ => l.to_string(),
            });
        }
        return out;
    }
    lines.into_iter().map(str::to_string).collect()
}

/// Lines without their trailing space, the leading and trailing blank ones, and the indentation
/// they share.
fn dedent(lines: Vec<String>) -> Vec<String> {
    let mut trimmed: Vec<String> = lines.iter().map(|l| l.trim_end().to_string()).collect();
    while trimmed.first().is_some_and(String::is_empty) {
        trimmed.remove(0);
    }
    while trimmed.last().is_some_and(String::is_empty) {
        trimmed.pop();
    }
    let indent_of = |l: &str| l.len() - l.trim_start_matches([' ', '\t']).len();
    let indent = trimmed.iter().filter(|l| !l.is_empty()).map(|l| indent_of(l)).min().unwrap_or(0);
    trimmed.iter().map(|l| l[indent.min(indent_of(l))..].to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prose(sql: &str, dialect: Dialect) -> String {
        leading(sql, dialect).prose
    }

    #[test]
    fn line_and_block_comments_become_paragraphs() {
        assert_eq!(prose("-- One user,\n--   by id.\nselect 1", Dialect::Postgres), "One user,\n  by id.");
        assert_eq!(prose("-- a\n\n-- b\nselect 1", Dialect::Postgres), "a\n\nb");
        assert_eq!(prose("/**\n * Lists users.\n *\n * Newest first.\n */\nselect 1", Dialect::Postgres), "Lists users.\n\nNewest first.");
        assert_eq!(prose("-- a\n/* b */ select 1", Dialect::Postgres), "a\n\nb");
        assert_eq!(prose("select 1 -- not before", Dialect::Postgres), "");
        assert_eq!(leading("-- a\nselect 1", Dialect::Postgres).end, 4);
    }

    #[test]
    fn each_dialect_reads_its_own_comments() {
        assert_eq!(prose("/* outer /* inner */ still */ select 1", Dialect::Postgres), "outer /* inner */ still");
        assert_eq!(prose("/* outer /* inner */ select 1", Dialect::Sqlite), "outer /* inner");
        assert_eq!(prose("# hash\n--x\nselect 1", Dialect::Mysql), "hash");
        assert_eq!(prose("-- dash\n/*! code */ select 1", Dialect::Mysql), "dash");
        assert_eq!(prose("# not a comment\nselect 1", Dialect::Postgres), "");
    }
}
