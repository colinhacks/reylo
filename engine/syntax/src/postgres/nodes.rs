//! The values Postgres's grammar actions work with, kept in the shape of its C: a node is a shared
//! cell whose fields are read and written by name, a list is shared and grows in place, and a
//! pointer to a field is a value. The actions are translated from C (`gen/postgres_actions.py`).

use super::generated_names::t;
use std::cell::RefCell;
use std::rc::Rc;

/// An action's result: its value, or Postgres's words for refusing the statement.
pub type R = Result<V, String>;

/// A rule's action, over its right-hand side's values and locations, and its own location.
pub type Action = fn(&mut [V], &[i32], i32) -> R;

/// A node: its tag and the fields set on it.
pub struct Obj {
    pub tag: u16,
    fields: RefCell<Vec<(u16, V)>>,
}

impl Obj {
    /// A field's value; one never set is NULL, as palloc0 leaves it.
    pub fn value(&self, field: u16) -> V {
        self.fields.borrow().iter().find(|(f, _)| *f == field).map_or(V::Null, |(_, v)| v.clone())
    }

    pub fn set(&self, field: u16, value: V) {
        let mut fields = self.fields.borrow_mut();
        match fields.iter_mut().find(|(f, _)| *f == field) {
            Some(slot) => slot.1 = value,
            None => fields.push((field, value)),
        }
    }

    /// An integer, boolean, character or enum field.
    pub fn int(&self, field: u16) -> i64 {
        self.value(field).int()
    }

    /// A string field: empty where it is NULL.
    pub fn string(&self, field: u16) -> String {
        match self.value(field) {
            V::Str(s) => String::from_utf8_lossy(&s).into_owned(),
            _ => String::new(),
        }
    }

    /// A deep copy, as `copyObject` makes.
    fn copied(&self) -> Obj {
        let fields = self.fields.borrow().iter().map(|(f, v)| (*f, v.copied())).collect();
        Obj { tag: self.tag, fields: RefCell::new(fields) }
    }
}

/// A tree of any depth goes without recursing: a statement may nest deeper than the stack.
impl Drop for Obj {
    fn drop(&mut self) {
        let mut pending: Vec<V> = self.fields.get_mut().drain(..).map(|(_, v)| v).collect();
        while let Some(v) = pending.pop() {
            match v {
                V::Node(rc) => {
                    if let Some(obj) = Rc::into_inner(rc) {
                        let mut obj = obj;
                        pending.extend(obj.fields.get_mut().drain(..).map(|(_, v)| v));
                    }
                }
                V::List(rc) | V::Cell(rc, _) => {
                    if let Some(items) = Rc::into_inner(rc) {
                        pending.extend(items.into_inner());
                    }
                }
                V::Place(rc, _) => pending.push(V::Node(rc)),
                _ => {}
            }
        }
    }
}

/// A value of the grammar: what a C `YYSTYPE`, a local or a field holds.
#[derive(Default)]
pub enum V {
    #[default]
    Null,
    Int(i64),
    /// A C string, which an escape may have left outside UTF-8.
    Str(Rc<[u8]>),
    Node(Rc<Obj>),
    List(Rc<RefCell<Vec<V>>>),
    /// A `ListCell`: the list and its place in it.
    Cell(Rc<RefCell<Vec<V>>>, usize),
    /// A pointer to a node's field.
    Place(Rc<Obj>, u16),
}

/// Out of line: the generated actions and deparser clone values at thousands of places.
impl Clone for V {
    #[inline(never)]
    fn clone(&self) -> V {
        match self {
            V::Null => V::Null,
            V::Int(i) => V::Int(*i),
            V::Str(s) => V::Str(s.clone()),
            V::Node(o) => V::Node(o.clone()),
            V::List(l) => V::List(l.clone()),
            V::Cell(l, at) => V::Cell(l.clone(), *at),
            V::Place(o, field) => V::Place(o.clone(), *field),
        }
    }
}

impl V {
    pub fn s(text: &str) -> V {
        V::Str(Rc::from(text.as_bytes()))
    }

    pub fn bytes(text: &[u8]) -> V {
        V::Str(Rc::from(text))
    }

    pub fn b(value: bool) -> V {
        V::Int(value as i64)
    }

    /// `makeNode`: every field zero.
    pub fn node(tag: u16) -> V {
        V::Node(Rc::new(Obj { tag, fields: RefCell::new(Vec::new()) }))
    }

    pub fn list(items: Vec<V>) -> V {
        V::List(Rc::new(RefCell::new(items)))
    }

    pub fn place(of: &V, field: u16) -> V {
        match of {
            V::Node(n) => V::Place(n.clone(), field),
            _ => V::Null,
        }
    }

    pub fn truthy(&self) -> bool {
        match self {
            V::Null => false,
            V::Int(n) => *n != 0,
            V::List(l) => !l.borrow().is_empty(),
            _ => true,
        }
    }

    pub fn int(&self) -> i64 {
        match self {
            V::Int(n) => *n,
            _ => 0,
        }
    }

    pub fn is_a(&self, tag: u16) -> bool {
        match self {
            V::Node(n) => n.tag == tag,
            V::List(_) => tag == t::T_List,
            V::Cell(..) => self.deref().is_a(tag),
            _ => false,
        }
    }

    /// `nodeTag`, for a node or a list.
    pub fn tag_number(&self) -> i64 {
        match self {
            V::Node(n) => n.tag as i64,
            V::List(_) => t::T_List as i64,
            _ => -1,
        }
    }

    /// A cell's place in its list; past the end for anything else.
    pub fn cell_index(&self) -> i64 {
        match self {
            V::Cell(_, i) => *i as i64,
            _ => i64::MAX,
        }
    }

    pub fn tag(&self) -> Option<u16> {
        match self {
            V::Node(n) => Some(n.tag),
            _ => None,
        }
    }

    pub fn get(&self, field: u16) -> V {
        match self {
            V::Node(n) => n.value(field),
            _ => V::Null,
        }
    }

    pub fn set(&self, field: u16, value: V) {
        if let V::Node(n) = self {
            n.set(field, value);
        }
    }

    /// A struct a node holds inside itself, made on first use.
    pub fn embedded(&self, field: u16, tag: u16) -> V {
        let held = self.get(field);
        if held.truthy() {
            return held;
        }
        let made = V::node(tag);
        self.set(field, made.clone());
        made
    }

    /// `*p`, or a cell's value.
    pub fn deref(&self) -> V {
        match self {
            V::Place(n, f) => n.value(*f),
            V::Cell(l, i) => l.borrow().get(*i).cloned().unwrap_or(V::Null),
            other => other.clone(),
        }
    }

    /// `*p = value`.
    pub fn store(&self, value: V) {
        if let V::Place(n, f) = self {
            n.set(*f, value);
        }
    }

    /// The cells of a list, as `foreach` walks them.
    pub fn cells(&self) -> Vec<V> {
        match self {
            V::List(l) => (0..l.borrow().len()).map(|i| V::Cell(l.clone(), i)).collect(),
            _ => Vec::new(),
        }
    }

    pub fn items(&self) -> Vec<V> {
        match self {
            V::List(l) => l.borrow().clone(),
            _ => Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            V::List(l) => l.borrow().len(),
            _ => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `copyObject`.
    pub fn copied(&self) -> V {
        match self {
            V::Node(n) => V::Node(Rc::new(n.copied())),
            V::List(l) => V::list(l.borrow().iter().map(V::copied).collect()),
            other => other.clone(),
        }
    }

    fn same(&self, other: &V) -> bool {
        match (self, other) {
            (V::Null, V::Null) => true,
            (V::Null, V::Int(0)) | (V::Int(0), V::Null) => true,
            (V::Int(a), V::Int(b)) => a == b,
            (V::Str(a), V::Str(b)) => Rc::ptr_eq(a, b),
            (V::Node(a), V::Node(b)) => Rc::ptr_eq(a, b),
            (V::List(a), V::List(b)) => Rc::ptr_eq(a, b),
            (V::Null, V::List(l)) | (V::List(l), V::Null) => l.borrow().is_empty(),
            _ => false,
        }
    }

    pub fn eq(&self, other: &V) -> V {
        V::b(self.same(other))
    }
    pub fn ne(&self, other: &V) -> V {
        V::b(!self.same(other))
    }
    pub fn lt(&self, other: &V) -> V {
        V::b(self.int() < other.int())
    }
    pub fn le(&self, other: &V) -> V {
        V::b(self.int() <= other.int())
    }
    pub fn gt(&self, other: &V) -> V {
        V::b(self.int() > other.int())
    }
    pub fn ge(&self, other: &V) -> V {
        V::b(self.int() >= other.int())
    }
    pub fn add(&self, other: &V) -> V {
        V::Int(self.int() + other.int())
    }
    pub fn sub(&self, other: &V) -> V {
        V::Int(self.int() - other.int())
    }
    pub fn mul(&self, other: &V) -> V {
        V::Int(self.int() * other.int())
    }
    pub fn or(&self, other: &V) -> V {
        V::Int(self.int() | other.int())
    }
    pub fn and(&self, other: &V) -> V {
        V::Int(self.int() & other.int())
    }
    pub fn shl(&self, other: &V) -> V {
        V::Int(self.int() << other.int())
    }
    pub fn neg(&self) -> V {
        V::Int(-self.int())
    }
    pub fn not(&self) -> V {
        V::Int(!self.int())
    }
}
