//! The tree every dialect's parser builds: one arena of nodes, a leaf per token.

/// A node's place in its tree.
pub type NodeId = u32;

/// The kind of a node holding what a dialect's parser could not read, in every dialect.
pub const ERROR_KIND: u16 = u16::MAX;

/// The text `children` cover, leaving out those that read nothing: an empty optional rule has no
/// text of its own, so it moves neither end. With no text at all, the place `at`.
pub(crate) fn span(nodes: &[Node], children: &[NodeId], at: u32) -> (u32, u32) {
    let read = |id: &&NodeId| {
        let n = &nodes[**id as usize];
        n.end > n.start
    };
    match (children.iter().find(read), children.iter().rev().find(read)) {
        (Some(a), Some(b)) => (nodes[*a as usize].start, nodes[*b as usize].end),
        _ => (at, at),
    }
}

/// A node: a leaf holds one token and the trivia before it, an inner node a run of children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    /// The dialect's symbol for this token or grammar rule.
    pub kind: u16,
    /// Where the text this node covers starts, its leading trivia excluded.
    pub start: u32,
    /// Where that text ends.
    pub end: u32,
    /// For a leaf, where its leading trivia starts. For an inner node, its first child in the
    /// tree's child list.
    pub lead: u32,
    /// How many children it has. A leaf has none.
    pub children: u32,
    /// Whether it is a token.
    pub leaf: bool,
}

/// A statement's text and its tree.
#[derive(Clone, Debug)]
pub struct Tree<'s> {
    pub text: &'s str,
    pub nodes: Vec<Node>,
    pub child_ids: Vec<NodeId>,
    pub root: NodeId,
    /// Where the trivia after the last token starts.
    pub tail: u32,
    /// Each stretch of text that could not be read, with what was wrong there.
    pub errors: Vec<SyntaxError>,
}

/// A place the text does not follow the grammar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
    pub start: u32,
    pub end: u32,
}

impl<'s> Tree<'s> {
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        let n = self.node(id);
        if n.leaf {
            return &[];
        }
        &self.child_ids[n.lead as usize..(n.lead + n.children) as usize]
    }

    /// The text this node covers, without the trivia before its first token.
    pub fn slice(&self, id: NodeId) -> &'s str {
        let n = self.node(id);
        &self.text[n.start as usize..n.end as usize]
    }

    /// The leaves in text order, on an explicit stack: a statement may nest deeper than the
    /// thread's own stack would take.
    pub fn leaves(&self) -> Leaves<'_, 's> {
        self.leaves_of(self.root)
    }

    /// The leaves under `id`, in text order.
    pub fn leaves_of(&self, id: NodeId) -> Leaves<'_, 's> {
        Leaves { tree: self, stack: vec![id] }
    }

    /// The tree printed from its leaves, which is the text it was read from.
    pub fn print(&self) -> String {
        let mut out = String::with_capacity(self.text.len());
        for id in self.leaves() {
            let n = self.node(id);
            out.push_str(&self.text[n.lead as usize..n.end as usize]);
        }
        out.push_str(&self.text[self.tail as usize..]);
        out
    }

    /// The deepest leaf whose text, trivia included, holds the byte `at`.
    pub fn leaf_at(&self, at: u32) -> Option<NodeId> {
        self.leaves().find(|id| {
            let n = self.node(*id);
            n.lead <= at && at < n.end.max(n.lead + 1)
        })
    }
}

/// See [`Tree::leaves`].
pub struct Leaves<'t, 's> {
    tree: &'t Tree<'s>,
    stack: Vec<NodeId>,
}

impl Iterator for Leaves<'_, '_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        while let Some(id) = self.stack.pop() {
            if self.tree.node(id).leaf {
                return Some(id);
            }
            self.stack.extend(self.tree.children(id).iter().rev());
        }
        None
    }
}
