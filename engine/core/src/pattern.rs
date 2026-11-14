//! A text's `pattern` is an I-Regexp (RFC 9485), which matches a whole value; each database's
//! engine reads the patterns its CHECKs hold a column to and writes the I-Regexp that matches
//! exactly the same values, or none where it cannot.

/// What matches any one character, a newline and a carriage return included: I-Regexp's `.` leaves
/// out the two (RFC 9485 §5.2).
const ANY: &str = "(.|\\n|\\r)";

/// The I-Regexp matching the values a SQL `LIKE` pattern matches, case for case: `%` any run of
/// characters, `_` any one, and `escape` making the character after it literal. `None` for a
/// pattern that ends in its escape, which the database refuses.
pub fn from_like(like: &str, escape: Option<char>) -> Option<String> {
    let mut out = String::with_capacity(like.len() * 2);
    let mut chars = like.chars();
    while let Some(c) = chars.next() {
        match c {
            _ if Some(c) == escape => push_literal(&mut out, chars.next()?),
            '%' => {
                out.push_str(ANY);
                out.push('*');
            }
            '_' => out.push_str(ANY),
            _ => push_literal(&mut out, c),
        }
    }
    Some(out)
}

/// A character as I-Regexp matches it literally: a metacharacter escaped (RFC 9485's
/// `SingleCharEsc`), a newline and a carriage return and a tab written as their escapes.
fn push_literal(out: &mut String, c: char) {
    match c {
        '.' | '\\' | '?' | '*' | '+' | '{' | '}' | '(' | ')' | '[' | ']' | '|' | '-' | '^' => {
            out.push('\\');
            out.push(c);
        }
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        _ => out.push(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_like_pattern_is_the_i_regexp_of_the_same_values() {
        assert_eq!(from_like("a%", Some('\\')).as_deref(), Some("a(.|\\n|\\r)*"));
        assert_eq!(from_like("%@%", Some('\\')).as_deref(), Some("(.|\\n|\\r)*@(.|\\n|\\r)*"));
        assert_eq!(from_like("a_c", Some('\\')).as_deref(), Some("a(.|\\n|\\r)c"));
        // A metacharacter is literal, and so is what the escape marks.
        assert_eq!(from_like("1.5 (x)", Some('\\')).as_deref(), Some("1\\.5 \\(x\\)"));
        assert_eq!(from_like("100\\%", Some('\\')).as_deref(), Some("100%"));
        assert_eq!(from_like("a\\_b", Some('\\')).as_deref(), Some("a_b"));
        assert_eq!(from_like("a!%", Some('!')).as_deref(), Some("a%"));
        assert_eq!(from_like("a\\%", None).as_deref(), Some("a\\\\(.|\\n|\\r)*"));
        // A pattern ending in its escape is no pattern.
        assert_eq!(from_like("a\\", Some('\\')), None);
    }
}
