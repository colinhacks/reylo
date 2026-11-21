//! SQLite's syntax: the tables Lemon generates from SQLite's own `parse.y` (`gen/sqlite.py`) and a port of its tokenizer, so a statement is accepted or refused by the same grammar SQLite reads it with.

pub mod generated;
pub mod scan;

use crate::lemon::{self, Grammar, Reduced};
use crate::tree::{NodeId, SyntaxError, Tree, ERROR_KIND};
use generated as g;

static GRAMMAR: Grammar = Grammar {
    action: &g::YY_ACTION,
    lookahead: &g::YY_LOOKAHEAD,
    shift_ofst: &g::YY_SHIFT_OFST,
    reduce_ofst: &g::YY_REDUCE_OFST,
    default: &g::YY_DEFAULT,
    fallback: &g::YY_FALLBACK,
    rule_lhs: &g::YY_RULE_LHS,
    rule_nrhs: &g::YY_RULE_NRHS,
    wildcard: g::YYWILDCARD,
    max_shift: g::YY_MAX_SHIFT,
    min_shiftreduce: g::YY_MIN_SHIFTREDUCE,
    max_shiftreduce: g::YY_MAX_SHIFTREDUCE,
    accept: g::YY_ACCEPT_ACTION,
    min_reduce: g::YY_MIN_REDUCE,
    illegal: g::TK_ILLEGAL,
    separator: g::TK_SEMI,
    complete: &[g::SYM_CMDLIST, g::SYM_ECMD],
    refuse,
};

/// What `parse.y`'s actions refuse of a statement its rules take, with the message each gives. The limits an action enforces, such as the number of terms in a compound, are the database's settings and not its syntax, and stay with it.
fn refuse(r: &Reduced<'_>) -> Option<SyntaxError> {
    let at = |id: NodeId, message: String| {
        let n = &r.nodes[id as usize];
        SyntaxError { message, start: n.start, end: n.end }
    };
    let token = |id: NodeId| &r.text[r.nodes[id as usize].start as usize..r.nodes[id as usize].end as usize];
    let kids = r.children(r.node);
    match r.rule {
        // `#1` names a register, in a statement SQLite writes for itself.
        g::RULE_EXPR_VARIABLE => {
            let t = token(kids[0]).as_bytes();
            (t[0] == b'#' && t.get(1).is_some_and(u8::is_ascii_digit)).then(|| at(kids[0], format!("near \"{}\": syntax error", token(kids[0]))))
        }
        g::RULE_TERM_QNUMBER => separator_refusal(token(kids[0])).map(|m| at(kids[0], m)),
        g::RULE_EIDLIST_MORE | g::RULE_EIDLIST_FIRST => {
            let [name, collate, order] = kids[kids.len() - 3..] else { return None };
            let read = |id: NodeId| !r.children(id).is_empty();
            (read(collate) || read(order)).then(|| at(r.node, format!("syntax error after column name \"{}\"", token(name))))
        }
        g::RULE_TRIDXBY_INDEXED => Some(at(r.node, "the INDEXED BY clause is not allowed on UPDATE or DELETE statements within triggers".to_string())),
        g::RULE_TRIDXBY_NOT_INDEXED => Some(at(r.node, "the NOT INDEXED clause is not allowed on UPDATE or DELETE statements within triggers".to_string())),
        g::RULE_SELECT | g::RULE_SELECT_WITH | g::RULE_SELECT_WITH_RECURSIVE => compound_refusal(r, kids[kids.len() - 1]).map(|m| at(r.node, m)),
        _ => None,
    }
}

/// SQLite's check of a number with `_` in it: each must sit between two digits. It rewrites the token in place as it goes, so a message shows the token as far as it had got.
fn separator_refusal(token: &str) -> Option<String> {
    let mut z = token.as_bytes().to_vec();
    z.push(0);
    let hex = z[0] == b'0' && (z[1] == b'x' || z[1] == b'X');
    let digit = |c: u8| if hex { c.is_ascii_hexdigit() } else { c.is_ascii_digit() };
    let (mut from, mut to, mut message) = (0, 0, None);
    loop {
        let c = z[from];
        if c != b'_' {
            z[to] = c;
            to += 1;
        } else if !digit(z[from - 1]) || !digit(z[from + 1]) {
            let end = z.iter().position(|b| *b == 0).unwrap_or(z.len());
            message = Some(format!("unrecognized token: \"{}\"", String::from_utf8_lossy(&z[..end])));
        }
        from += 1;
        if c == 0 {
            return message;
        }
    }
}

/// A compound's ORDER BY or LIMIT goes on its last term, and the error names the operator after the rightmost term that broke that.
fn compound_refusal(r: &Reduced<'_>, select: NodeId) -> Option<String> {
    let mut node = select;
    loop {
        if r.nodes[node as usize].kind != g::SYM_SELECTNOWITH {
            return None;
        }
        let &[left, op, _] = r.children(node) else { return None };
        // Lemon reduces no rule that only renames one symbol, so a compound's first term is its `oneselect` itself.
        let prior = if r.nodes[left as usize].kind == g::SYM_SELECTNOWITH { *r.children(left).last()? } else { left };
        let has = |kind: u16| r.children(prior).iter().any(|c| r.nodes[*c as usize].kind == kind && !r.children(*c).is_empty());
        let clause = if has(g::SYM_ORDERBY_OPT) {
            "ORDER BY"
        } else if has(g::SYM_LIMIT_OPT) {
            "LIMIT"
        } else {
            node = left;
            continue;
        };
        let words = r.children(op);
        let name = match (words.len(), r.nodes[words[0] as usize].kind) {
            (2, _) => "UNION ALL",
            (_, g::TK_UNION) => "UNION",
            (_, g::TK_EXCEPT) => "EXCEPT",
            _ => "INTERSECT",
        };
        return Some(format!("{clause} clause should come after {name} not before"));
    }
}

/// The name of a token or grammar rule in this dialect's trees.
pub fn kind_name(kind: u16) -> &'static str {
    if kind == ERROR_KIND {
        return "error";
    }
    g::NAMES.get(kind as usize).copied().unwrap_or("?")
}

/// Reads `text` into a lossless tree. An error's message is the one SQLite gives for it.
pub fn parse_tree(text: &str) -> Tree<'_> {
    let (tokens, tail) = scan::lexemes(text);
    let mut tree = lemon::parse(&GRAMMAR, text, &tokens, tail);
    for e in tree.errors.iter_mut().filter(|e| e.message.is_empty()) {
        let token = &text[e.start as usize..e.end as usize];
        e.message = if token.is_empty() {
            "incomplete input".to_string()
        } else if tokens.get(tokens.partition_point(|l| l.start < e.start)).is_some_and(|l| l.start == e.start && l.sym == g::TK_ILLEGAL) {
            format!("unrecognized token: \"{token}\"")
        } else {
            format!("near \"{token}\": syntax error")
        };
    }
    tree
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_what_it_read() {
        for text in ["", " -- only a comment\n", "select 1", "select a, b from t where x = 1; -- done\n", "select (", "select 1; select from; select 2", "create table t(a int) strict;", "\u{feff}select [a b], `c`, \"d\" from t", "select 'unterminated", "x'zz'", "select 1\0 and the rest"] {
            assert_eq!(parse_tree(text).print(), text);
        }
    }

    #[test]
    fn accepts_and_refuses_as_the_grammar_says() {
        for text in ["select 1", "select 1;", ";", "", "select sum(x) over (partition by y) from t", "select x over from t", "select 1 window, 2", "with a as (select 1) select * from a", "create virtual table v using fts5(anything, 'at', all (1, 2))", "insert into t values (1) on conflict do nothing returning *", "select a ->> '$.b' from t"] {
            assert!(parse_tree(text).errors.is_empty(), "refused {text}");
        }
        for (text, message) in [("select (", "incomplete input"), ("select from", "near \"from\": syntax error"),("selec 1", "near \"selec\": syntax error"), ("select 1 2", "near \"2\": syntax error"), ("select 'a", "unrecognized token: \"'a\"")] {
            let tree = parse_tree(text);
            assert_eq!(tree.errors.first().map(|e| e.message.as_str()), Some(message), "{text}");
        }
    }

    #[test]
    fn reads_on_after_an_error() {
        let tree = parse_tree("select from; select 1; select (; select 2");
        assert_eq!(tree.errors.len(), 2);
        let text = "select 1; select from; select 2";
        let spans: Vec<&str> = crate::statements(crate::Dialect::Sqlite, &parse_tree(text)).iter().map(|(a, b)| &text[*a as usize..*b as usize]).collect();
        assert_eq!(spans, ["select 1", "select from", "select 2"]);
    }
}
