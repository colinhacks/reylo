//! Where a statement's byte stands in the Rust source that holds it, for an error that cannot
//! underline it: no error can point inside a string literal on stable Rust (`Literal::subspan`,
//! rust-lang/rust#54725), so the macro's message names the line and column instead.

/// The line and column of byte `offset` of a string literal's value, each from 1, the column in
/// characters as rustc counts them. `literal` is the literal as written, `"…"` with its escapes or
/// `r#"…"#`, and `line` and `column` are where it starts. An offset inside an escape stands at the
/// escape, and one past the value at the closing quote.
pub fn in_literal(literal: &str, line: usize, column: usize, offset: usize) -> (usize, usize) {
    // `r` and its hashes before the quote, and the hashes after the closing one: a byte each.
    let hashes = literal.strip_prefix('r').map(|rest| rest.len() - rest.trim_start_matches('#').len());
    let open = hashes.map_or(1, |h| h + 2);
    let close = hashes.map_or(1, |h| h + 1);
    let body = literal.get(open..literal.len().saturating_sub(close)).unwrap_or("");
    let (mut line, mut column) = (line, column + open);
    let mut step = |c: char| {
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    };
    let mut chars = body.chars().peekable();
    let mut value = 0;
    loop {
        // A line continuation ahead adds nothing to the value, so the offset stands past it.
        let mut ahead = chars.clone();
        let continues = hashes.is_none() && ahead.next() == Some('\\') && ahead.next() == Some('\n');
        if value >= offset && !continues {
            break;
        }
        let Some(c) = chars.next() else { break };
        step(c);
        if hashes.is_some() || c != '\\' {
            value += c.len_utf8();
            continue;
        }
        match chars.next() {
            // A backslash at a line's end: the line break and the whitespace after it are no part of
            // the value.
            Some('\n') => {
                step('\n');
                while let Some(&w) = chars.peek().filter(|w| matches!(w, ' ' | '\t' | '\n' | '\r')) {
                    chars.next();
                    step(w);
                }
            }
            Some('x') => {
                step('x');
                for d in chars.by_ref().take(2) {
                    step(d);
                }
                value += 1;
            }
            Some('u') => {
                step('u');
                let mut digits = String::new();
                for d in chars.by_ref() {
                    step(d);
                    if d == '}' {
                        break;
                    }
                    if d != '{' && d != '_' {
                        digits.push(d);
                    }
                }
                value += u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32).map_or(1, char::len_utf8);
            }
            Some(e) => {
                step(e);
                value += 1;
            }
            None => break,
        }
    }
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::in_literal;

    #[test]
    fn a_plain_string_counts_from_its_quote() {
        // `#[reylo::query("select id, nme from users")]`, the quote at column 16.
        assert_eq!(in_literal(r#""select id, nme from users""#, 1, 16, 11), (1, 28));
        assert_eq!(in_literal(r#""select""#, 4, 1, 0), (4, 2));
    }

    #[test]
    fn a_statement_over_several_lines_counts_its_lines() {
        let literal = "\"\n    select id, nme\n    from users\n\"";
        assert_eq!(in_literal(literal, 1, 16, "\n    select id, ".len()), (2, 16));
        assert_eq!(in_literal(literal, 1, 16, "\n    select id, nme\n    from ".len()), (3, 10));
    }

    #[test]
    fn a_raw_string_takes_its_hashes_and_no_escapes() {
        let literal = "r#\"\n  select \"a\\b\", nme\n\"#";
        assert_eq!(in_literal(literal, 1, 16, "\n  select \"a\\b\", ".len()), (2, 17));
        assert_eq!(in_literal("r\"select nme\"", 1, 16, 7), (1, 25));
    }

    #[test]
    fn an_escape_is_one_character_of_the_value() {
        // The value `select "a", nme`: `\"` is two characters of the source and one of the value.
        assert_eq!(in_literal(r#""select \"a\", nme""#, 1, 16, 12), (1, 31));
        assert_eq!(in_literal(r#""select '\x41', nme""#, 1, 1, 12), (1, 17));
        // `\u{e9}` is é, two bytes of the value and six characters of the source.
        assert_eq!(in_literal(r#""select '\u{e9}', nme""#, 1, 1, 13), (1, 19));
    }

    #[test]
    fn a_line_continuation_is_no_part_of_the_value() {
        // The value `select id, nme`: the backslash, the line break and the indent are dropped.
        assert_eq!(in_literal("\"select id, \\\n      nme\"", 1, 16, 11), (2, 7));
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        assert_eq!(in_literal("\"select 'é', nme\"", 1, 1, "select 'é', ".len()), (1, 14));
    }

    #[test]
    fn an_offset_past_the_value_stands_at_the_closing_quote() {
        assert_eq!(in_literal(r#""select""#, 1, 1, 99), (1, 8));
    }
}
