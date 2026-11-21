//! What a BEFORE row trigger written in PL/pgSQL does with its row, read off the body's tokens.
//!
//! One question only: whether the body can do anything to a row but keep it. A body this reading
//! cannot place answers no, which claims nothing.

use super::generated::tok;
use super::scan::{self, Token, Value};

/// Whether the token is the name `word`, in any case the scanner folds.
fn is_word(token: &Token, word: &str) -> bool {
    match &token.value {
        Value::Keyword(k) => *k == word,
        Value::Bytes(b) if token.kind == tok::IDENT => b.as_slice() == word.as_bytes(),
        _ => false,
    }
}

/// The words this reading finds a block's structure by. A quoted identifier that spells one is a
/// name the reading would take for the word, so a body with one answers no.
const STRUCTURE: [&str; 21] = ["alias", "begin", "call", "case", "declare", "else", "elseif", "elsif", "end", "exception", "for", "foreach", "if", "into", "loop", "return", "strict", "then", "when", "while", "execute"];

fn spells_structure(body: &str, token: &Token) -> bool {
    let text = &body[token.start as usize..token.end as usize];
    let quoted = text.starts_with('"') || text.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("u&"));
    quoted && matches!(&token.value, Value::Bytes(b) if STRUCTURE.iter().any(|w| w.as_bytes() == b.as_slice()))
}

/// A name the outermost DECLARE made for NEW (`true`) or OLD (`false`).
type Aliases = Vec<(Vec<u8>, bool)>;

/// The name a token spells, as the scanner folded it.
fn name_of<'t>(token: &'t Token) -> Option<&'t [u8]> {
    match &token.value {
        Value::Keyword(k) => Some(k.as_bytes()),
        Value::Bytes(b) if token.kind == tok::IDENT => Some(b),
        _ => None,
    }
}

/// Which record a token names: NEW (`true`) or OLD (`false`), itself or by an alias.
fn record_of(token: &Token, aliases: &Aliases) -> Option<bool> {
    if is_word(token, "new") {
        return Some(true);
    }
    if is_word(token, "old") {
        return Some(false);
    }
    let name = name_of(token)?;
    aliases.iter().find(|(alias, _)| alias == name).map(|(_, is_new)| *is_new)
}

fn is_name(token: &Token) -> bool {
    token.kind == tok::IDENT || matches!(token.value, Value::Keyword(_))
}

fn is_char(token: &Token, c: u8) -> bool {
    token.kind == super::generated::CHAR_TOKENS[c as usize]
}

/// PL/pgSQL's reserved words (`pl_reserved_kwlist.h`). After a dot one is no field of a record to
/// PL/pgSQL's scanner, which then takes the record itself as what the statement names.
const RESERVED: [&str; 24] = ["all", "begin", "by", "case", "declare", "else", "end", "execute", "for", "foreach", "from", "if", "in", "into", "loop", "not", "null", "or", "strict", "then", "to", "using", "when", "while"];

fn is_reserved(token: &Token) -> bool {
    RESERVED.iter().any(|w| is_word(token, w))
}

/// A dotted name at `at`: where it ends, and whether it names NEW or OLD as a whole, which a last
/// part of `new` or `old` does (`new`, or a block's label before it), and a name with a reserved
/// word after a dot may.
fn target(words: &[&Token], at: usize, aliases: &Aliases) -> (usize, bool) {
    let mut end = at;
    let mut reserved = false;
    while words.get(end + 1).is_some_and(|t| is_char(t, b'.')) && words.get(end + 2).is_some_and(|t| is_name(t)) {
        end += 2;
        reserved |= is_reserved(words[end]);
    }
    (end + 1, reserved || words.get(end).is_some_and(|t| record_of(t, aliases).is_some()))
}

/// A list of targets at `at`: whether any names NEW or OLD as a whole.
fn any_whole_record(words: &[&Token], mut at: usize, aliases: &Aliases) -> bool {
    loop {
        if !words.get(at).is_some_and(|t| is_name(t)) {
            return false;
        }
        let (end, whole) = target(words, at, aliases);
        if whole {
            return true;
        }
        if !words.get(end).is_some_and(|t| is_char(t, b',')) {
            return false;
        }
        at = end + 1;
    }
}

/// Whether the body of a BEFORE row trigger keeps every row: each RETURN returns NEW, where
/// `may_return_new`, or OLD, where `may_return_old`; nothing assigns either as a whole, or reads
/// a row into it; nothing shadows them, and a name for either comes only from the outermost
/// DECLARE's `name ALIAS FOR new`; and nothing CALLs a procedure, which can write
/// either back through an INOUT parameter.
pub fn keeps_the_row(body: &str, may_return_new: bool, may_return_old: bool) -> bool {
    let Ok(tokens) = scan::scan(body) else { return false };
    let words: Vec<&Token> = tokens.iter().filter(|t| t.kind != tok::SQL_COMMENT && t.kind != tok::C_COMMENT).collect();
    // Under `standard_conforming_strings = off`, which a function can set for itself, a backslash
    // before a quote keeps the string open where this reading sees it close.
    if !is_block(body, &words) || body.contains("\\'") || words.iter().any(|t| spells_structure(body, t)) {
        return false;
    }
    let mut i = 0;
    let mut starts = true;
    let mut declaring = false;
    let mut aliases: Aliases = Vec::new();
    // Whether the outermost block's BEGIN has been read: an alias inside it has a scope of its own.
    let mut begun = false;
    while i < words.len() {
        let word = words[i];
        if declaring {
            if is_word(word, "begin") {
                // Anywhere but where a declaration would start, it is a name in an expression.
                if !starts {
                    return false;
                }
                declaring = false;
                begun = true;
                starts = true;
                i += 1;
                continue;
            }
            if starts && is_word(word, "declare") {
                i += 1;
                continue;
            }
            if starts && record_of(word, &aliases).is_some() {
                return false;
            }
            if starts && words.get(i + 1).is_some_and(|t| is_word(t, "alias")) {
                let of = words.get(i + 3).and_then(|t| record_of(t, &aliases));
                let plain = words.get(i + 2).is_some_and(|t| is_word(t, "for")) && words.get(i + 4).is_some_and(|t| is_char(t, b';'));
                let (Some(name), Some(is_new), true, false) = (name_of(word), of, plain, begun) else { return false };
                aliases.push((name.to_vec(), is_new));
                i += 4;
                continue;
            }
            starts = is_char(word, b';');
            i += 1;
            continue;
        }
        if is_word(word, "begin") {
            begun = true;
        }
        // A nested block's declarations have scopes this reading doesn't track, and the word may be a
        // column's name in a statement.
        if is_word(word, "declare") && begun {
            return false;
        }
        if starts && is_word(word, "declare") {
            declaring = true;
            starts = true;
            i += 1;
            continue;
        }
        if is_word(word, "into") {
            let mut at = i + 1;
            if words.get(at).is_some_and(|t| is_word(t, "strict")) {
                at += 1;
            }
            if any_whole_record(&words, at, &aliases) {
                return false;
            }
        }
        if starts {
            if is_word(word, "return") {
                let Some(value) = words.get(i + 1) else { return false };
                let allowed = record_of(value, &aliases).is_some_and(|is_new| if is_new { may_return_new } else { may_return_old });
                if !allowed || !words.get(i + 2).is_some_and(|t| is_char(t, b';')) {
                    return false;
                }
            } else if is_word(word, "call") {
                return false;
            } else if is_word(word, "for") || is_word(word, "foreach") {
                if any_whole_record(&words, i + 1, &aliases) {
                    return false;
                }
            } else if is_name(word) {
                let (end, whole) = target(&words, i, &aliases);
                // `new := …`, and `new[…] := …` and `new.* := …`, which write the record itself.
                if whole && words.get(end).is_some_and(|t| t.kind == tok::COLON_EQUALS || is_char(t, b'=') || is_char(t, b'[') || is_char(t, b'.')) {
                    return false;
                }
            }
        }
        starts = is_char(word, b';') || word.kind == tok::Op && word_text(body, word) == ">>" || ["begin", "then", "else", "loop", "exception"].iter().any(|k| is_word(word, k));
        i += 1;
    }
    // A declaration section that never reaches BEGIN is no block this reading can place.
    !declaring
}

/// Whether the text has a block's outline: `[<<label>>] [DECLARE …] BEGIN … END [label] [;]`.
fn is_block(body: &str, words: &[&Token]) -> bool {
    let first = if words.first().is_some_and(|t| word_text(body, t) == "<<") {
        if !words.get(2).is_some_and(|t| word_text(body, t) == ">>") {
            return false;
        }
        3
    } else {
        0
    };
    if !words.get(first).is_some_and(|t| is_word(t, "begin") || is_word(t, "declare")) {
        return false;
    }
    let mut last = words.len();
    if last > 0 && is_char(words[last - 1], b';') {
        last -= 1;
    }
    last > first && (is_word(words[last - 1], "end") || (last >= first + 2 && is_name(words[last - 1]) && is_word(words[last - 2], "end")))
}

fn word_text<'b>(body: &'b str, token: &Token) -> &'b str {
    &body[token.start as usize..token.end as usize]
}

#[cfg(test)]
mod tests {
    use super::keeps_the_row;

    #[test]
    fn answers_no_where_a_word_of_the_block_is_a_name() {
        for body in [
            "declare \"begin\" int; new t; begin return new; end",
            "declare x int := (select begin from (select 1 as begin) s); new t; begin return new; end",
            "declare \"begin\" alias for new; begin \"begin\" := null; return new; end",
            "declare \"declare\" int; begin \"declare\" := 1; return null; begin null; end; end",
            "begin perform case when true then declare else 1 end from (select 1 as declare) s; return null; begin null; end; end",
            "declare \"for\" alias for new; begin \"for\" := null; return new; end",
            "declare \"foreach\" alias for new; begin \"foreach\" := null; return new; end",
            "declare \"strict\" alias for new; begin select 1 into \"strict\"; return new; end",
            "begin perform 'a\\'' ; return null; --';\nreturn new; end",
        ] {
            assert!(!keeps_the_row(body, true, true), "{body}");
        }
    }

    #[test]
    fn keeps_what_it_can_read() {
        for body in ["begin new.a := regexp_replace(new.a, '\\s+', ' '); return new; end", "declare n alias for new; begin n.a := 1; return n; end", "<<outer>> begin return new; end outer;"] {
            assert!(keeps_the_row(body, true, true), "{body}");
        }
    }
}
