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

/// The I-Regexp matching the values a Postgres regular expression finds a match in, as `~` reads
/// it: an advanced regular expression, case-sensitive, whose `.` and bracket expressions match a
/// newline, and whose `^` and `$` match only at the value's ends. `~` matches anywhere in the value,
/// so a branch not anchored at an end is padded with any run of characters there. `None` for a
/// construct with no exact I-Regexp: a back reference, a lookaround, embedded options (`(?i)`,
/// `***=`), a class shorthand (`\d`, `\w`), a constraint escape (`\m`, `\y`, `\A`), a character
/// class, collating element or equivalence class in brackets (`[[:alpha:]]`, `[[.a.]]`), an anchor
/// inside a group, and any escape or bound this does not read. Brackets' ranges are by code point,
/// as Postgres's are.
pub fn from_postgres_regex(re: &str) -> Option<String> {
    let chars: Vec<char> = re.chars().collect();
    let mut parser = Regex { s: &chars, at: 0, depth: 0 };
    let mut branches = Vec::new();
    loop {
        let (start, body, end) = parser.branch(true)?;
        let pad = |anchored: bool| if anchored { String::new() } else { format!("{ANY}*") };
        branches.push(if body.is_empty() && !start && !end { pad(false) } else { format!("{}{body}{}", pad(start), pad(end)) });
        match parser.next() {
            None => break,
            Some('|') => {}
            Some(_) => return None,
        }
    }
    Some(branches.join("|"))
}

/// How deeply groups may nest before a regular expression is left unread.
const MAX_GROUPS: usize = 32;

/// The largest count a bound may write (`RE_DUP_MAX`).
const MAX_BOUND: u32 = 255;

/// A Postgres regular expression read from `at`, as [`from_postgres_regex`] reads one.
struct Regex<'a> {
    s: &'a [char],
    at: usize,
    depth: usize,
}

impl Regex<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.at).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += 1;
        Some(c)
    }

    /// One branch, up to a `|`, a `)` or the end, as I-Regexp; at the top, whether it is anchored
    /// at its start (`^`) and at its end (`$`).
    fn branch(&mut self, top: bool) -> Option<(bool, String, bool)> {
        let start = top && self.peek() == Some('^');
        if start {
            self.at += 1;
        }
        let mut out = String::new();
        loop {
            match self.peek() {
                None | Some('|') | Some(')') => return Some((start, out, false)),
                Some('$') if top && matches!(self.s.get(self.at + 1), None | Some('|')) => {
                    self.at += 1;
                    return Some((start, out, true));
                }
                _ => {
                    out.push_str(&self.atom()?);
                    out.push_str(&self.quantifier()?);
                }
            }
        }
    }

    /// One atom: a group, a bracket expression, `.`, an escape or a character.
    fn atom(&mut self) -> Option<String> {
        match self.next()? {
            '(' => {
                if self.peek() == Some('?') {
                    // Only a group that captures nothing; a lookaround or options have no I-Regexp.
                    if self.s.get(self.at + 1) != Some(&':') {
                        return None;
                    }
                    self.at += 2;
                }
                self.depth += 1;
                if self.depth > MAX_GROUPS {
                    return None;
                }
                let mut branches = Vec::new();
                loop {
                    let (_, body, _) = self.branch(false)?;
                    branches.push(body);
                    match self.next()? {
                        '|' => {}
                        ')' => break,
                        _ => return None,
                    }
                }
                self.depth -= 1;
                Some(format!("({})", branches.join("|")))
            }
            '[' => self.bracket(),
            '.' => Some(ANY.to_string()),
            '\\' => {
                let mut out = String::new();
                push_literal(&mut out, self.escaped()?);
                Some(out)
            }
            // A quantifier with nothing before it, an anchor inside a group or before the end, and a
            // closing brace or bracket with no opening one.
            '*' | '+' | '?' | '{' | '^' | '$' | '}' | ']' => None,
            c => {
                let mut out = String::new();
                push_literal(&mut out, c);
                Some(out)
            }
        }
    }

    /// The character an escape stands for: a line break, a carriage return, a tab, a form feed, a
    /// vertical tab, a bell or an escape, or any character but a letter or a digit as itself.
    fn escaped(&mut self) -> Option<char> {
        Some(match self.next()? {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'f' => '\u{c}',
            'v' => '\u{b}',
            'a' => '\u{7}',
            'e' => '\u{1b}',
            c if c.is_alphanumeric() => return None,
            c => c,
        })
    }

    /// What may follow an atom: `*`, `+`, `?` or a bound, each greedy or not, since whether a value
    /// holds a match does not depend on which match is preferred; nothing, or a second quantifier,
    /// which Postgres refuses.
    fn quantifier(&mut self) -> Option<String> {
        let out = match self.peek() {
            Some(c @ ('*' | '+' | '?')) => {
                self.at += 1;
                c.to_string()
            }
            Some('{') => {
                self.at += 1;
                let number = |p: &mut Self| -> Option<u32> {
                    let from = p.at;
                    while p.peek().is_some_and(|c| c.is_ascii_digit()) {
                        p.at += 1;
                    }
                    let n: u32 = p.s[from..p.at].iter().collect::<String>().parse().ok()?;
                    (n <= MAX_BOUND).then_some(n)
                };
                let min = number(self)?;
                match self.next()? {
                    '}' => format!("{{{min}}}"),
                    ',' if self.peek() == Some('}') => {
                        self.at += 1;
                        format!("{{{min},}}")
                    }
                    ',' => {
                        let max = number(self)?;
                        (self.next()? == '}' && min <= max).then_some(())?;
                        format!("{{{min},{max}}}")
                    }
                    _ => return None,
                }
            }
            _ => return Some(String::new()),
        };
        if self.peek() == Some('?') {
            self.at += 1;
        }
        match self.peek() {
            Some('*' | '+' | '?' | '{') => None,
            _ => Some(out),
        }
    }

    /// A bracket expression, after its `[`: characters, escapes and ranges, perhaps complemented.
    fn bracket(&mut self) -> Option<String> {
        let mut out = String::from("[");
        if self.peek() == Some('^') {
            self.at += 1;
            out.push('^');
        }
        let mut first = true;
        loop {
            let c = self.next()?;
            let low = match c {
                ']' if !first => break,
                '[' if matches!(self.peek(), Some(':' | '.' | '=')) => return None,
                '\\' => self.escaped()?,
                // A `-` not first, not last and not ending a range, which Postgres refuses.
                '-' if !first && self.peek() != Some(']') => return None,
                c => c,
            };
            first = false;
            push_class_char(&mut out, low);
            if self.peek() == Some('-') && self.s.get(self.at + 1).is_some_and(|c| *c != ']') {
                self.at += 1;
                let high = match self.next()? {
                    '\\' => self.escaped()?,
                    '[' => return None,
                    c => c,
                };
                if high < low {
                    return None;
                }
                out.push('-');
                push_class_char(&mut out, high);
            }
        }
        out.push(']');
        // Two `&` or two `~` in a row are an operator in some regular expressions' classes, which an
        // I-Regexp reader may translate to.
        (!out.contains("&&") && !out.contains("~~")).then_some(out)
    }
}

/// A character as an I-Regexp bracket expression holds it: `-`, `[`, `\`, `]` and `^` escaped, and
/// a line break, a carriage return and a tab as their escapes.
fn push_class_char(out: &mut String, c: char) {
    match c {
        '-' | '[' | '\\' | ']' | '^' => {
            out.push('\\');
            out.push(c);
        }
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        _ => out.push(c),
    }
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

    #[test]
    fn a_postgres_regex_is_the_i_regexp_of_the_values_it_finds_a_match_in() {
        let any = "(.|\\n|\\r)";
        // Anchored at both ends, the expression itself; unanchored, padded at each open end.
        assert_eq!(from_postgres_regex("^[a-z]+$").as_deref(), Some("[a-z]+"));
        assert_eq!(from_postgres_regex("^ab").as_deref(), Some(format!("ab{any}*")).as_deref());
        assert_eq!(from_postgres_regex("@").as_deref(), Some(format!("{any}*@{any}*")).as_deref());
        assert_eq!(from_postgres_regex("").as_deref(), Some(format!("{any}*")).as_deref());
        assert_eq!(from_postgres_regex("^a$|^b").as_deref(), Some(format!("a|b{any}*")).as_deref());
        // `.` matches a line break, and a quantifier's preference changes nothing.
        assert_eq!(from_postgres_regex("^a.c$").as_deref(), Some(format!("a{any}c")).as_deref());
        assert_eq!(from_postgres_regex("^a+?b{2,3}?(?:c|d)*$").as_deref(), Some("a+b{2,3}(c|d)*"));
        assert_eq!(from_postgres_regex("^[^-a\\]]-x{3,}$").as_deref(), Some("[^\\-a\\]]\\-x{3,}"));
        assert_eq!(from_postgres_regex("^\\.\\$\\n[\\t ]$").as_deref(), Some("\\.$\\n[\\t ]"));
        assert_eq!(from_postgres_regex("^[]a]$").as_deref(), Some("[\\]a]"));
        // What has no exact I-Regexp.
        for refused in [
            "^(a)\\1$", "^(?=a)", "(?i)^a$", "***=a", "^\\d+$", "^\\w$", "\\ma", "\\Aa", "^[[:alpha:]]$", "^[[.a.]]$", "^[[=a=]]$", "(^a)", "a^b", "a$b", "*a", "a**", "a{2}{3}", "a{,3}", "a{3,2}", "a{256}", "^[a-c-e]$", "^[z-a]$", "a\\", "(a", "a)", "]", "}", "^[a&&b]$", "\\x41", "\\u0041",
        ] {
            assert_eq!(from_postgres_regex(refused), None, "{refused}");
        }
    }
}
