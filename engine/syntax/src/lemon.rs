//! A table-driven LALR(1) parser over tables the Lemon parser generator wrote, building a [`Tree`]. It follows Lemon's own driver (`lempar.c`), with its token fallbacks and wildcard. A statement the grammar refuses still gets a tree: what was read of it, and the tokens up to the next statement, go under an error node, and parsing starts again there.

use crate::lalr::Lexeme;
use crate::tree::{span, Node, NodeId, SyntaxError, Tree, ERROR_KIND};

/// A grammar's tables and the numbers that mark out its kinds of action, as Lemon lays them out.
pub struct Grammar {
    pub action: &'static [u16],
    pub lookahead: &'static [u16],
    pub shift_ofst: &'static [u16],
    pub reduce_ofst: &'static [i16],
    pub default: &'static [u16],
    pub fallback: &'static [u16],
    pub rule_lhs: &'static [u16],
    pub rule_nrhs: &'static [i8],
    pub wildcard: u16,
    pub max_shift: u16,
    pub min_shiftreduce: u16,
    pub max_shiftreduce: u16,
    pub accept: u16,
    pub min_reduce: u16,
    /// A token the scanner could not read, which no state takes, the wildcard's included.
    pub illegal: u16,
    /// A statement's end, where parsing starts over after an error.
    pub separator: u16,
    /// The symbols that hold finished statements, which an error in a later one leaves as they are.
    pub complete: &'static [u16],
    /// Whether a rule's action refuses what the rule read, which the tables cannot say.
    pub refuse: fn(&Reduced<'_>) -> Option<SyntaxError>,
}

/// A rule just reduced, for [`Grammar::refuse`].
pub struct Reduced<'t> {
    pub rule: usize,
    /// The node the rule made.
    pub node: NodeId,
    pub nodes: &'t [Node],
    pub child_ids: &'t [NodeId],
    pub text: &'t str,
}

impl Reduced<'_> {
    /// The children of `id`.
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        let n = &self.nodes[id as usize];
        if n.leaf {
            return &[];
        }
        &self.child_ids[n.lead as usize..(n.lead + n.children) as usize]
    }
}

impl Grammar {
    #[inline]
    fn shift_action(&self, state: u16, mut look: u16) -> u16 {
        if state > self.max_shift {
            return state;
        }
        loop {
            let i = self.shift_ofst[state as usize] as usize + look as usize;
            if self.lookahead[i] == look {
                return self.action[i];
            }
            let fallback = self.fallback[look as usize];
            if fallback != 0 {
                look = fallback;
                continue;
            }
            let j = i - look as usize + self.wildcard as usize;
            if self.lookahead[j] == self.wildcard && look > 0 {
                return self.action[j];
            }
            return self.default[state as usize];
        }
    }

    #[inline]
    fn reduce_action(&self, state: u16, lhs: u16) -> u16 {
        self.action[(self.reduce_ofst[state as usize] as i32 + lhs as i32) as usize]
    }
}

/// Parses `tokens`, of `text`, into a tree. Each error's message is left for the dialect to write.
pub fn parse<'s>(g: &Grammar, text: &'s str, tokens: &[Lexeme], tail: u32) -> Tree<'s> {
    let mut nodes: Vec<Node> = Vec::with_capacity(tokens.len() * 3);
    let mut child_ids: Vec<NodeId> = Vec::with_capacity(tokens.len() * 3);
    // Each entry's state, and the node read there; the first entry has none.
    let mut stack: Vec<(u16, NodeId)> = Vec::with_capacity(64);
    stack.push((0, 0));
    let mut errors: Vec<SyntaxError> = Vec::new();
    let mut finished: Vec<NodeId> = Vec::new();
    let mut p = 0usize;
    'read: loop {
        let sym = tokens.get(p).map_or(0, |l| l.sym);
        let mut act = stack[stack.len() - 1].0;
        // What an action refused while this token was read. Reading stops once the token is taken, and a later refusal replaces an earlier one, as each replaces the parser's one error message.
        let mut refused: Option<SyntaxError> = None;
        loop {
            if sym == g.illegal {
                act = g.max_shiftreduce + 1;
            } else {
                act = g.shift_action(act, sym);
            }
            if act >= g.min_reduce {
                let rule = (act - g.min_reduce) as usize;
                let len = -(g.rule_nrhs[rule] as i32) as usize;
                let lhs = g.rule_lhs[rule];
                let at = stack.len() - len;
                let first = child_ids.len() as u32;
                child_ids.extend(stack[at..].iter().map(|e| e.1));
                let (start, end) = span(&nodes, &child_ids[first as usize..], tokens.get(p).map_or(tail, |l| l.lead));
                stack.truncate(at);
                nodes.push(Node { kind: lhs, start, end, lead: first, children: len as u32, leaf: false });
                let node = nodes.len() as NodeId - 1;
                if let Some(e) = (g.refuse)(&Reduced { rule, node, nodes: &nodes, child_ids: &child_ids, text }) {
                    refused = Some(e);
                }
                act = g.reduce_action(stack[stack.len() - 1].0, lhs);
                stack.push((act, node));
            } else if act <= g.max_shiftreduce && refused.is_none() {
                let Some(l) = tokens.get(p) else { break 'read };
                nodes.push(Node { kind: l.sym, start: l.start, end: l.end, lead: l.lead, children: 0, leaf: true });
                let state = if act > g.max_shift { act + g.min_reduce - g.min_shiftreduce } else { act };
                stack.push((state, nodes.len() as NodeId - 1));
                p += 1;
                continue 'read;
            } else if act == g.accept && refused.is_none() {
                finished.extend(stack.drain(1..).map(|e| e.1));
                break 'read;
            } else {
                // What was read of the statement, and the tokens to its end, under one error node.
                let end_of_text = text.len() as u32;
                let (from, to) = tokens.get(p).map_or((end_of_text, end_of_text), |l| (l.start, l.end));
                let taken = act <= g.max_shiftreduce || act == g.accept;
                errors.push(match refused.take() {
                    Some(e) if taken => e,
                    _ => SyntaxError { message: String::new(), start: from, end: to },
                });
                let mut held: Vec<NodeId> = stack.drain(1..).map(|e| e.1).collect();
                let done = held.iter().take_while(|id| g.complete.contains(&nodes[**id as usize].kind)).count();
                finished.extend(held.drain(..done));
                while let Some(l) = tokens.get(p) {
                    nodes.push(Node { kind: l.sym, start: l.start, end: l.end, lead: l.lead, children: 0, leaf: true });
                    held.push(nodes.len() as NodeId - 1);
                    p += 1;
                    if l.sym == g.separator {
                        break;
                    }
                }
                if !held.is_empty() {
                    let first_child = child_ids.len() as u32;
                    let (start, end) = span(&nodes, &held, from);
                    child_ids.extend_from_slice(&held);
                    nodes.push(Node { kind: ERROR_KIND, start, end, lead: first_child, children: held.len() as u32, leaf: false });
                    finished.push(nodes.len() as NodeId - 1);
                }
                if p >= tokens.len() {
                    break 'read;
                }
                continue 'read;
            }
        }
    }
    let first = child_ids.len() as u32;
    child_ids.extend_from_slice(&finished);
    let (start, end) = span(&nodes, &finished, tail);
    nodes.push(Node { kind: 0, start, end, lead: first, children: finished.len() as u32, leaf: false });
    let root = nodes.len() as NodeId - 1;
    Tree { text, nodes, child_ids, root, tail, errors }
}
