//! libpg_query's parse tree, read straight from its C structs into the `pg_query::protobuf` structs
//! the rest of the engine reads, and a tree built back into C structs for its deparser.
//! `pg_query::parse` has libpg_query encode the tree as protobuf in C and prost decode it again in
//! Rust, and `pg_query::deparse` the reverse; this reads each node the way that encoder writes it,
//! and makes each the way that decoder makes it (the converters `build.rs` generates from
//! libpg_query's own definitions of both), with none of the encoding, the decoding or their code
//! in the engine: the same tree comes back, and the same text.

use pg_query::protobuf as pb;
use pg_query::Error;
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::ffi::{c_char, c_int, c_void, CStr, CString};

#[allow(non_upper_case_globals, non_camel_case_types, non_snake_case, dead_code, clippy::all)]
mod b {
    include!(concat!(env!("OUT_DIR"), "/raw_bindings.rs"));
}

extern "C" {
    fn pg_query_raw_parse(input: *const c_char, parser_options: c_int) -> b::PgQueryInternalParsetreeAndError;
    fn pg_query_enter_memory_context() -> b::MemoryContext;
    fn pg_query_exit_memory_context(ctx: b::MemoryContext);
    fn pg_query_free_error(error: *mut b::PgQueryError);
    fn pg_query_free_deparse_result(result: b::PgQueryDeparseResult);
    fn reylo_deparse_nodes(stmts: *mut b::List) -> b::PgQueryDeparseResult;
    fn free(ptr: *mut c_void);
}

/// How deeply the reading has recursed, counted as libpg_query's protobuf counts it, so the same
/// trees are refused: past half of `PG_QUERY_PROTOBUF_MAX_DEPTH` generic nodes (`_outNode`,
/// `vendor/`), or a message that deep below the parse's (upb's encoder, where a generic node is
/// two messages, the `Node` and the node in it).
struct Depth {
    nodes: u32,
    messages: u32,
}

impl Depth {
    /// Into one message.
    fn enter(&mut self) -> Result<(), Deep> {
        self.messages += 1;
        if self.messages >= b::PG_QUERY_PROTOBUF_MAX_DEPTH {
            return Err(Deep);
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.messages -= 1;
    }
}

/// The tree was deeper than that.
struct Deep;

/// The refusal of a tree past the bound, which [`crate::stack::too_deep`] knows.
pub const TOO_DEEP: &str = "parse tree is nested too deeply to read";

/// A parse: the tree, which is all the engine reads of one (`pg_query::ParseResult` also lists
/// the tables and functions it names, which takes a walk over the whole tree).
#[derive(Clone)]
pub struct Parsed {
    pub protobuf: pb::ParseResult,
}

impl Parsed {
    /// The tree with the tables and functions it names, which `pg_query` lists by walking it.
    pub fn listed(self) -> pg_query::ParseResult {
        pg_query::ParseResult::new(self.protobuf, String::new())
    }
}

/// Parse `sql` with libpg_query, as `pg_query::parse(sql, 0)` does, into the same tree.
pub fn parse(sql: &str) -> pg_query::Result<Parsed> {
    let input = CString::new(sql)?;
    unsafe {
        let ctx = pg_query_enter_memory_context();
        let result = pg_query_raw_parse(input.as_ptr(), 0);
        let parsed = if !result.error.is_null() {
            Err(Error::Parse(CStr::from_ptr((*result.error).message).to_string_lossy().into_owned()))
        } else {
            match stmts(result.tree) {
                Ok(stmts) => Ok(Parsed { protobuf: pb::ParseResult { version: b::PG_VERSION_NUM as i32, stmts } }),
                Err(Deep) => Err(Error::Parse(TOO_DEEP.to_string())),
            }
        };
        // The tree is in the memory context; the error and stderr are malloc'd and outlive it.
        pg_query_exit_memory_context(ctx);
        if !result.error.is_null() {
            pg_query_free_error(result.error);
        }
        if !result.stderr_buffer.is_null() {
            free(result.stderr_buffer as *mut c_void);
        }
        parsed
    }
}

/// The statements of a parse: a List of RawStmt, each written as `_outRawStmt` writes it.
unsafe fn stmts(list: *const b::List) -> Result<Vec<pb::RawStmt>, Deep> {
    let mut d = Depth { nodes: 0, messages: 0 };
    cells(list).iter().filter(|c| !c.ptr_value.is_null()).map(|c| out_RawStmt(&*(c.ptr_value as *const b::RawStmt), &mut d)).collect()
}

/// A List's cells (none for NIL).
unsafe fn cells<'a>(list: *const b::List) -> &'a [b::ListCell] {
    if list.is_null() || (*list).length == 0 {
        &[]
    } else {
        std::slice::from_raw_parts((*list).elements, (*list).length as usize)
    }
}

/// A generic node, as `_outNode` writes it: counted against the bound.
unsafe fn node(p: *const b::Node, d: &mut Depth) -> Result<Option<pb::node::Node>, Deep> {
    d.nodes += 1;
    if d.nodes > b::PG_QUERY_PROTOBUF_MAX_DEPTH / 2 {
        return Err(Deep);
    }
    d.enter()?;
    let n = node_at(p, d)?;
    d.leave();
    d.nodes -= 1;
    Ok(n)
}

/// A list field, each cell a generic node (`WRITE_LIST_FIELD`).
unsafe fn list(list: *const b::List, d: &mut Depth) -> Result<Vec<pb::Node>, Deep> {
    cells(list).iter().map(|c| Ok(pb::Node { node: node(c.ptr_value as *const b::Node, d)? })).collect()
}

/// A string field: absent where the C string is NULL (`WRITE_STRING_FIELD`).
unsafe fn string(s: *const c_char) -> String {
    if s.is_null() {
        String::new()
    } else {
        CStr::from_ptr(s).to_string_lossy().into_owned()
    }
}

/// A char field: the one character, absent where it is 0 (`WRITE_CHAR_FIELD`).
fn chr(c: c_char) -> String {
    if c == 0 {
        String::new()
    } else {
        (c as u8 as char).to_string()
    }
}

/// A Bitmapset's members, in order (`WRITE_BITMAPSET_FIELD`).
unsafe fn bitmapset(set: *const b::Bitmapset) -> Vec<u64> {
    if set.is_null() {
        return Vec::new();
    }
    let bits = 8 * std::mem::size_of::<b::bitmapword>() as u64;
    let words = (*set).words.as_slice((*set).nwords as usize);
    let mut members = Vec::new();
    for (i, &w) in words.iter().enumerate() {
        for bit in 0..bits {
            if w & (1 << bit) != 0 {
                members.push(i as u64 * bits + bit);
            }
        }
    }
    members
}

// The nodes libpg_query writes by hand (`pg_query_outfuncs_protobuf.c`).

#[allow(non_snake_case)]
unsafe fn out_List(n: &b::List, d: &mut Depth) -> Result<pb::List, Deep> {
    d.enter()?;
    let message = pb::List { items: list(n, d)? };
    d.leave();
    Ok(message)
}

/// An integer list's cells hold no nodes; libpg_query's encoder reads them as though they did, and
/// a raw parse tree holds none, so its numbers are read as Integer nodes here.
#[allow(non_snake_case)]
unsafe fn out_IntList(n: &b::List, _: &mut Depth) -> Result<pb::IntList, Deep> {
    Ok(pb::IntList { items: cells(n).iter().map(|c| pb::Node { node: Some(pb::node::Node::Integer(pb::Integer { ival: c.int_value })) }).collect() })
}

#[allow(non_snake_case)]
unsafe fn out_OidList(n: &b::List, _: &mut Depth) -> Result<pb::OidList, Deep> {
    Ok(pb::OidList { items: cells(n).iter().map(|c| pb::Node { node: Some(pb::node::Node::Integer(pb::Integer { ival: c.oid_value as i32 })) }).collect() })
}

#[allow(non_snake_case)]
unsafe fn out_Integer(n: &b::Integer, d: &mut Depth) -> Result<pb::Integer, Deep> {
    d.enter()?;
    let message = pb::Integer { ival: n.ival };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
unsafe fn out_Float(n: &b::Float, d: &mut Depth) -> Result<pb::Float, Deep> {
    d.enter()?;
    let message = pb::Float { fval: string(n.fval) };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
unsafe fn out_Boolean(n: &b::Boolean, d: &mut Depth) -> Result<pb::Boolean, Deep> {
    d.enter()?;
    let message = pb::Boolean { boolval: n.boolval };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
unsafe fn out_String(n: &b::String, d: &mut Depth) -> Result<pb::String, Deep> {
    d.enter()?;
    let message = pb::String { sval: string(n.sval) };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
unsafe fn out_BitString(n: &b::BitString, d: &mut Depth) -> Result<pb::BitString, Deep> {
    d.enter()?;
    let message = pb::BitString { bsval: string(n.bsval) };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
unsafe fn out_AConst(n: &b::A_Const, d: &mut Depth) -> Result<pb::AConst, Deep> {
    use pb::a_const::Val;
    d.enter()?;
    let val = if n.isnull {
        None
    } else {
        match n.val.node.type_ {
            b::NodeTag_T_Integer => Some(Val::Ival(out_Integer(&n.val.ival, d)?)),
            b::NodeTag_T_Float => Some(Val::Fval(out_Float(&n.val.fval, d)?)),
            b::NodeTag_T_Boolean => Some(Val::Boolval(out_Boolean(&n.val.boolval, d)?)),
            b::NodeTag_T_String => Some(Val::Sval(out_String(&n.val.sval, d)?)),
            b::NodeTag_T_BitString => Some(Val::Bsval(out_BitString(&n.val.bsval, d)?)),
            _ => None,
        }
    };
    d.leave();
    Ok(pb::AConst { isnull: n.isnull, location: n.location, val })
}

include!(concat!(env!("OUT_DIR"), "/raw_convert.rs"));

/// Print a tree back to SQL with libpg_query's deparser, as `pg_query::deparse(tree,
/// Default::default())` does.
pub fn deparse(tree: &pb::ParseResult) -> pg_query::Result<String> {
    let mut a = Arena::default();
    unsafe {
        let stmts: Vec<*mut b::Node> = tree.stmts.iter().map(|s| in_RawStmt(s, &mut a) as *mut b::Node).collect();
        if a.unsupported {
            return Err(Error::Parse("unsupported protobuf node type".to_string()));
        }
        let list = a.list(stmts);
        let result = reylo_deparse_nodes(list);
        let text = if !result.error.is_null() {
            Err(Error::Parse(CStr::from_ptr((*result.error).message).to_string_lossy().into_owned()))
        } else {
            Ok(CStr::from_ptr(result.query).to_string_lossy().into_owned())
        };
        pg_query_free_deparse_result(result);
        text
    }
}

/// The C nodes of one deparse, which the deparser only reads: zeroed as `makeNode` zeroes them,
/// and freed together once it is done.
#[derive(Default)]
struct Arena {
    blocks: Vec<(*mut u8, Layout)>,
    strings: Vec<CString>,
    /// A node libpg_query's reader refuses ("unsupported protobuf node type").
    unsupported: bool,
}

impl Arena {
    unsafe fn alloc<T>(&mut self, layout: Layout) -> *mut T {
        let p = alloc_zeroed(layout);
        assert!(!p.is_null(), "out of memory");
        self.blocks.push((p, layout));
        p as *mut T
    }

    /// A node of type `T`, with its tag.
    unsafe fn make<T>(&mut self, tag: b::NodeTag) -> *mut T {
        let p: *mut T = self.alloc(Layout::new::<T>());
        *(p as *mut b::NodeTag) = tag;
        p
    }

    /// A C string, which lives as long as the arena.
    fn string(&mut self, s: &str) -> *mut c_char {
        let c = CString::new(s.replace('\0', "")).unwrap();
        let p = c.as_ptr() as *mut c_char;
        self.strings.push(c);
        p
    }

    /// A List of the nodes (NIL for none), as `lappend` builds it.
    unsafe fn list(&mut self, items: Vec<*mut b::Node>) -> *mut b::List {
        if items.is_empty() {
            return std::ptr::null_mut();
        }
        let list: *mut b::List = self.make(b::NodeTag_T_List);
        let cells: *mut b::ListCell = self.alloc(Layout::array::<b::ListCell>(items.len()).unwrap());
        for (i, item) in items.iter().enumerate() {
            (*cells.add(i)).ptr_value = *item as *mut c_void;
        }
        (*list).length = items.len() as c_int;
        (*list).max_length = items.len() as c_int;
        (*list).elements = cells;
        list
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        for &(p, layout) in &self.blocks {
            unsafe { dealloc(p, layout) };
        }
    }
}

// The nodes libpg_query's reader makes by hand (`pg_query_readfuncs_protobuf.c`).

#[allow(non_snake_case)]
unsafe fn in_Integer(m: &pb::Integer, a: &mut Arena) -> *mut b::Integer {
    let n = a.make::<b::Integer>(b::NodeTag_T_Integer);
    (*n).ival = m.ival;
    n
}

#[allow(non_snake_case)]
unsafe fn in_Float(m: &pb::Float, a: &mut Arena) -> *mut b::Float {
    let n = a.make::<b::Float>(b::NodeTag_T_Float);
    (*n).fval = a.string(&m.fval);
    n
}

#[allow(non_snake_case)]
unsafe fn in_Boolean(m: &pb::Boolean, a: &mut Arena) -> *mut b::Boolean {
    let n = a.make::<b::Boolean>(b::NodeTag_T_Boolean);
    (*n).boolval = m.boolval;
    n
}

#[allow(non_snake_case)]
unsafe fn in_String(m: &pb::String, a: &mut Arena) -> *mut b::String {
    let n = a.make::<b::String>(b::NodeTag_T_String);
    (*n).sval = a.string(&m.sval);
    n
}

#[allow(non_snake_case)]
unsafe fn in_BitString(m: &pb::BitString, a: &mut Arena) -> *mut b::BitString {
    let n = a.make::<b::BitString>(b::NodeTag_T_BitString);
    (*n).bsval = a.string(&m.bsval);
    n
}

#[allow(non_snake_case)]
unsafe fn in_List(m: &pb::List, a: &mut Arena) -> *mut b::List {
    let items = m.items.iter().map(|x| in_node(x, a)).collect();
    a.list(items)
}

#[allow(non_snake_case)]
unsafe fn in_AConst(m: &pb::AConst, a: &mut Arena) -> *mut b::A_Const {
    use pb::a_const::Val;
    let n = a.make::<b::A_Const>(b::NodeTag_T_A_Const);
    (*n).location = m.location;
    if m.isnull {
        (*n).isnull = true;
    } else {
        match &m.val {
            Some(Val::Ival(v)) => (*n).val.ival = *in_Integer(v, a),
            Some(Val::Fval(v)) => (*n).val.fval = *in_Float(v, a),
            Some(Val::Boolval(v)) => (*n).val.boolval = *in_Boolean(v, a),
            Some(Val::Sval(v)) => (*n).val.sval = *in_String(v, a),
            Some(Val::Bsval(v)) => (*n).val.bsval = *in_BitString(v, a),
            None => {}
        }
    }
    n
}

include!(concat!(env!("OUT_DIR"), "/raw_build.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    /// The reading and libpg_query's own protobuf give the same tree, or the same refusal; and
    /// the tree deparses to the same text both ways.
    fn same(sql: &str) -> Result<(), String> {
        match (pg_query::parse(sql, 0), parse(sql)) {
            (Ok(a), Ok(b)) if a.protobuf == b.protobuf => match (pg_query::deparse(&a.protobuf, Default::default()), deparse(&b.protobuf)) {
                (Ok(x), Ok(y)) if x == y => Ok(()),
                (Err(x), Err(y)) if x.to_string() == y.to_string() => Ok(()),
                (x, y) => Err(format!("deparsed {x:?} against {y:?}: {sql}")),
            },
            (Ok(_), Ok(_)) => Err(format!("a different tree: {sql}")),
            (Err(a), Err(b)) if a.to_string() == b.to_string() || (crate::stack::too_deep(&a.to_string()) && crate::stack::too_deep(&b.to_string())) => Ok(()),
            (a, b) => Err(format!("{:?} against {:?}: {sql}", a.map(|_| ()), b.map(|_| ()))),
        }
    }

    #[test]
    fn every_postgres_statement_of_the_oracle_reads_as_libpg_querys_tree() {
        let cases = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/cases");
        let mut read = 0;
        let mut wrong = Vec::new();
        for case in std::fs::read_dir(&cases).unwrap().flatten() {
            if std::fs::read_to_string(case.path().join("dialect")).is_ok_and(|d| d.trim() != "postgres") {
                continue;
            }
            for file in std::fs::read_dir(case.path()).unwrap().flatten() {
                let path = file.path();
                if path.extension().is_some_and(|e| e == "sql") {
                    let sql = std::fs::read_to_string(&path).unwrap();
                    read += 1;
                    if let Err(e) = crate::stack::run(|| same(&sql)) {
                        wrong.push(e);
                    }
                }
            }
        }
        assert!(read > 500, "{read} statements");
        assert!(wrong.is_empty(), "{} of {read}: {:#?}", wrong.len(), &wrong[..wrong.len().min(5)]);
    }

    /// The time each takes over a corpus (`REYLO_PG_CORPUS`), printed: `cargo test --release …
    /// reading_time -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn reading_time() {
        let corpus = std::env::var("REYLO_PG_CORPUS").expect("REYLO_PG_CORPUS");
        let lines: Vec<String> = std::fs::read_to_string(corpus).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        crate::stack::run(|| {
            for round in 0..3 {
                let t = std::time::Instant::now();
                lines.iter().for_each(|sql| drop(pg_query::parse(sql, 0)));
                let protobuf = t.elapsed();
                let t = std::time::Instant::now();
                lines.iter().for_each(|sql| drop(parse(sql)));
                let direct = t.elapsed();
                println!("round {round}: {} statements, protobuf {protobuf:?}, direct {direct:?}", lines.len());
            }
        });
    }

    /// Every statement of a corpus, one JSON string per line, as `REYLO_PG_CORPUS` names it (such
    /// as Postgres's regression suite, split into statements).
    #[test]
    #[ignore]
    fn every_statement_of_a_corpus_reads_as_libpg_querys_tree() {
        let corpus = std::env::var("REYLO_PG_CORPUS").expect("REYLO_PG_CORPUS");
        let lines: Vec<String> = std::fs::read_to_string(corpus).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        let wrong: Vec<String> = lines.iter().filter_map(|sql| crate::stack::run(|| same(sql)).err()).collect();
        assert!(wrong.is_empty(), "{} of {}: {:#?}", wrong.len(), lines.len(), &wrong[..wrong.len().min(10)]);
    }
}
