//! What the translated deparser (`generated/deparse.rs`, from libpg_query's `postgres_deparse.c`)
//! calls: its output machinery, with the default options, and the C string and list functions.

use super::generated_names::{f, t};
use super::nodes::{R, V};
use super::scan::keyword;
use std::cell::RefCell;

const INDENT_SIZE: i64 = 4;
const MAX_LINE_LENGTH: i64 = 80;
const NO_INDENT: i64 = 0;
const INDENT_AND_MERGE: i64 = 2;
const CONTEXT_IDENTIFIER: i64 = 12;
const CONTEXT_CONSTANT: i64 = 13;

#[derive(Default)]
struct Part {
    text: Vec<u8>,
    indent: i64,
    mergeable: bool,
}

struct Group {
    keyword: Option<Vec<u8>>,
    parts: Vec<Part>,
    indent_mode: i64,
}

#[derive(Default)]
struct Level {
    groups: Vec<Group>,
    base_indent: i64,
}

#[derive(Default)]
struct State {
    levels: Vec<Level>,
    result: Vec<Part>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<T>(run: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|s| run(&mut s.borrow_mut()))
}

fn new_part(level: &Level, indent_mode: i64) -> Part {
    let indent = level.base_indent + if indent_mode != NO_INDENT { INDENT_SIZE } else { 0 };
    Part { text: Vec::new(), indent, mergeable: indent_mode == INDENT_AND_MERGE }
}

impl State {
    fn level(&mut self) -> &mut Level {
        self.levels.last_mut().expect("the deparser is inside a nesting level")
    }

    fn group(&mut self) -> &mut Group {
        let level = self.level();
        if level.groups.is_empty() {
            let part = new_part(level, NO_INDENT);
            level.groups.push(Group { keyword: None, parts: vec![part], indent_mode: NO_INDENT });
        }
        level.groups.last_mut().expect("just made")
    }

    fn part(&mut self) -> &mut Part {
        self.group().parts.last_mut().expect("a part group holds a part")
    }

    fn append_part(&mut self, deduplicate: bool) {
        self.group();
        if deduplicate {
            self.remove_trailing_empty_part();
        }
        let level = self.levels.last_mut().expect("in a level");
        let mode = level.groups.last().expect("in a group").indent_mode;
        let part = new_part(level, mode);
        level.groups.last_mut().expect("in a group").parts.push(part);
    }

    fn remove_trailing_empty_part(&mut self) {
        let group = self.group();
        if group.parts.last().is_some_and(|p| p.text.is_empty()) {
            group.parts.pop();
        }
    }
}

fn trim_space(text: &mut Vec<u8>) {
    if text.last() == Some(&b' ') {
        text.pop();
    }
}

fn bytes(v: &V) -> Vec<u8> {
    match v {
        V::Str(s) => s.to_vec(),
        _ => Vec::new(),
    }
}

/// A C format with `%s`, `%d`, `%ld`, `%u` and `%%`.
pub fn message(format: &str, args: &[V]) -> String {
    String::from_utf8_lossy(&formatted(format.as_bytes(), args)).into_owned()
}

fn formatted(format: &[u8], args: &[V]) -> Vec<u8> {
    let (mut out, mut i, mut arg) = (Vec::with_capacity(format.len() + 8), 0, args.iter());
    while i < format.len() {
        let c = format[i];
        i += 1;
        if c != b'%' {
            out.push(c);
            continue;
        }
        if format.get(i) == Some(&b'l') {
            i += 1;
        }
        let spec = format.get(i).copied().unwrap_or(b'%');
        i += 1;
        match spec {
            b'%' => out.push(b'%'),
            b's' => out.extend(arg.next().map(bytes).unwrap_or_default()),
            b'd' | b'u' => out.extend(arg.next().map_or(0, V::int).to_string().bytes()),
            other => {
                out.push(b'%');
                out.push(other);
            }
        }
    }
    out
}

// The output machinery.

#[allow(non_snake_case)]
pub fn deparseAppendStringInfo(args: &[V]) -> R {
    let text = formatted(&bytes(&args[1]), &args[2..]);
    with(|s| s.part().text.extend(text));
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn deparseAppendStringInfoString(_: &V, text: &V) -> R {
    let text = bytes(text);
    with(|s| s.part().text.extend(text));
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn deparseAppendStringInfoChar(_: &V, c: &V) -> R {
    let c = c.int() as u8;
    with(|s| s.part().text.push(c));
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn removeTrailingSpace(_: &V) -> R {
    with(|s| trim_space(&mut s.part().text));
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn deparseMarkCurrentPartNonMergable(_: &V) -> R {
    with(|s| s.part().mergeable = false);
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn deparseAppendPart(_: &V, deduplicate: &V) -> R {
    with(|s| s.append_part(deduplicate.truthy()));
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn deparseAppendCommaAndPart(_: &V) -> R {
    with(|s| {
        s.part().text.push(b',');
        s.append_part(true);
    });
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn deparseAppendPartGroup(_: &V, keyword: &V, indent_mode: &V) -> R {
    let keyword = match keyword {
        V::Str(k) => Some(k.to_vec()),
        _ => None,
    };
    let mode = indent_mode.int();
    with(|s| {
        if !s.level().groups.is_empty() {
            trim_space(&mut s.part().text);
        }
        let level = s.level();
        let part = new_part(level, mode);
        level.groups.push(Group { keyword, parts: vec![part], indent_mode: mode });
    });
    Ok(V::Null)
}

/// No comments are ever passed in.
#[allow(non_snake_case)]
pub fn deparseAppendCommentsIfNeeded(_: &V, _: &V) -> R {
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn deparseStateIncreaseNestingLevel(_: &V) -> R {
    Ok(with(|s| {
        let mut level = Level::default();
        let parent = if s.levels.is_empty() {
            V::Null
        } else {
            let mode = s.group().indent_mode;
            let base = s.level().base_indent;
            level.base_indent = base + INDENT_SIZE + if mode != NO_INDENT { INDENT_SIZE } else { 0 };
            s.part().mergeable = false;
            V::Int(1)
        };
        s.levels.push(level);
        parent
    }))
}

#[allow(non_snake_case)]
pub fn deparseStateDecreaseNestingLevel(_: &V, parent: &V) -> R {
    let has_parent = parent.truthy();
    with(|s| {
        let level = s.levels.pop().expect("a level to leave");
        let base = Level { groups: Vec::new(), base_indent: level.base_indent };
        for mut group in level.groups {
            if group.indent_mode == INDENT_AND_MERGE && group.parts.len() > 1 {
                let mut parts = std::mem::take(&mut group.parts).into_iter();
                let mut merged = vec![parts.next().expect("more than one part")];
                for part in parts {
                    let target = merged.last_mut().expect("holds the first part");
                    trim_space(&mut target.text);
                    if target.mergeable && part.mergeable && target.indent + target.text.len() as i64 + 1 + part.text.len() as i64 <= MAX_LINE_LENGTH {
                        if !target.text.is_empty() && target.text.last() != Some(&b'(') {
                            target.text.push(b' ');
                        }
                        target.text.extend(part.text);
                    } else {
                        merged.push(part);
                    }
                }
                group.parts = merged;
            }
            if let Some(keyword) = group.keyword.take() {
                let mut target = new_part(&base, NO_INDENT);
                target.text = keyword;
                group.parts.insert(0, target);
                if group.parts.len() == 2 || group.indent_mode == NO_INDENT {
                    let part = group.parts.remove(1);
                    if !part.text.is_empty() {
                        group.parts[0].text.push(b' ');
                        group.parts[0].text.extend(part.text);
                    }
                }
            }
            if has_parent {
                let holder = s.levels.last_mut().expect("the parent level").groups.last_mut().expect("the parent's group");
                holder.parts.append(&mut group.parts);
            } else {
                s.result.append(&mut group.parts);
            }
        }
        if has_parent {
            s.append_part(true);
            s.part().mergeable = false;
        }
    });
    Ok(V::Null)
}

/// The parts of one statement, joined as `deparseEmit` joins them without pretty-printing.
fn emit(out: &mut Vec<u8>) {
    with(|s| {
        if s.result.last().is_some_and(|p| p.text.is_empty()) {
            s.result.pop();
        }
        let count = s.result.len();
        for (i, part) in s.result.drain(..).enumerate() {
            if matches!(part.text.first(), Some(b')' | b';')) {
                trim_space(out);
            }
            out.extend(part.text);
            trim_space(out);
            if i + 1 != count && out.last() != Some(&b'(') {
                out.push(b' ');
            }
        }
    });
}

// C strings and identifiers.

#[allow(non_snake_case)]
pub fn deparseStringLiteral(_: &V, value: &V) -> R {
    let text = bytes(value);
    with(|s| {
        let out = &mut s.part().text;
        if text.contains(&b'\\') {
            out.push(b'E');
        }
        out.push(b'\'');
        for &c in &text {
            if c == b'\'' || c == b'\\' {
                out.push(c);
            }
            out.push(c);
        }
        out.push(b'\'');
    });
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn deparseGenericDefElemName(state: &V, name: &V) -> R {
    let upper: Vec<u8> = bytes(name).iter().map(u8::to_ascii_uppercase).collect();
    deparseAppendStringInfoString(state, &V::bytes(&upper))
}

pub fn quote_identifier(name: &V) -> R {
    let text = bytes(name);
    let mut safe = matches!(text.first(), Some(b'a'..=b'z' | b'_'));
    safe &= text.iter().all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'_'));
    if safe && keyword(&text).is_some_and(|k| k.2 != 0) {
        safe = false;
    }
    if safe {
        return Ok(name.clone());
    }
    let mut quoted = Vec::with_capacity(text.len() + 2);
    quoted.push(b'"');
    for &c in &text {
        if c == b'"' {
            quoted.push(b'"');
        }
        quoted.push(c);
    }
    quoted.push(b'"');
    Ok(V::bytes(&quoted))
}

#[allow(non_snake_case)]
pub fn isReservedKeyword(value: &V) -> R {
    let text = bytes(value);
    let lower = text.iter().all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'_'));
    Ok(V::b(lower && keyword(&text).is_some_and(|k| k.2 == 3)))
}

#[allow(non_snake_case)]
pub fn isOp(value: &V) -> R {
    Ok(V::b(bytes(value).iter().all(|c| matches!(c, b'~' | b'!' | b'@' | b'#' | b'^' | b'&' | b'|' | b'`' | b'?' | b'+' | b'-' | b'*' | b'/' | b'%' | b'<' | b'>' | b'='))))
}

pub fn strcmp(a: &V, b: &V) -> R {
    Ok(V::Int(match bytes(a).cmp(&bytes(b)) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }))
}

pub fn strlen(s: &V) -> R {
    Ok(V::Int(bytes(s).len() as i64))
}

/// Whether `needle` is in `hay`: its callers only compare the pointer with NULL.
pub fn strstr(hay: &V, needle: &V) -> R {
    let (hay, needle) = (bytes(hay), bytes(needle));
    Ok(V::b(hay.windows(needle.len().max(1)).any(|w| w == needle)))
}

#[allow(non_snake_case)]
pub fn Min(a: &V, b: &V) -> R {
    Ok(V::Int(a.int().min(b.int())))
}

#[allow(non_snake_case)]
pub fn INTERVAL_MASK(b: &V) -> R {
    Ok(V::Int(1 << b.int()))
}

#[allow(non_snake_case)]
pub fn TRIGGER_FOR_INSERT(kind: &V) -> R {
    Ok(V::Int(kind.int() & (1 << 2)))
}
#[allow(non_snake_case)]
pub fn TRIGGER_FOR_DELETE(kind: &V) -> R {
    Ok(V::Int(kind.int() & (1 << 3)))
}
#[allow(non_snake_case)]
pub fn TRIGGER_FOR_UPDATE(kind: &V) -> R {
    Ok(V::Int(kind.int() & (1 << 4)))
}
#[allow(non_snake_case)]
pub fn TRIGGER_FOR_TRUNCATE(kind: &V) -> R {
    Ok(V::Int(kind.int() & (1 << 5)))
}

// Nodes and lists, checked as the deparser's strict macros check them.

/// `castNode`: an error where a node is of another type.
pub fn cast(tag: u16, value: &V) -> R {
    if value.truthy() && value.tag_number() != tag as i64 {
        return Err(format!("deparse: unexpected node type {}, expected {}", value.tag_number(), tag));
    }
    Ok(value.clone())
}

fn value_of(value: &V, tag: u16, field: u16) -> R {
    if !value.truthy() {
        return Err("deparse: value extraction from NULL node".to_string());
    }
    Ok(cast(tag, value)?.get(field))
}

#[allow(non_snake_case)]
pub fn strVal(v: &V) -> R {
    value_of(v, t::T_String, f::F_sval)
}
#[allow(non_snake_case)]
pub fn intVal(v: &V) -> R {
    value_of(v, t::T_Integer, f::F_ival)
}
#[allow(non_snake_case)]
pub fn boolVal(v: &V) -> R {
    value_of(v, t::T_Boolean, f::F_boolval)
}

fn nth(list: &V, n: usize) -> R {
    let len = list.len();
    if n >= len {
        return Err(format!("deparse: list index {n} out of range for length {len}"));
    }
    Ok(list.items()[n].clone())
}

pub fn linitial(list: &V) -> R {
    nth(list, 0)
}
pub fn lsecond(list: &V) -> R {
    nth(list, 1)
}
pub fn lthird(list: &V) -> R {
    nth(list, 2)
}
pub fn lfourth(list: &V) -> R {
    nth(list, 3)
}
pub fn llast(list: &V) -> R {
    if list.is_empty() {
        return Err("deparse: llast() on empty list".to_string());
    }
    nth(list, list.len() - 1)
}

pub fn lfirst(cell: &V) -> R {
    Ok(cell.deref())
}

pub fn list_length(list: &V) -> R {
    Ok(V::Int(list.len() as i64))
}

/// The cell after `cell`, or NULL.
pub fn lnext(_: &V, cell: &V) -> R {
    Ok(match cell {
        V::Cell(l, i) if i + 1 < l.borrow().len() => V::Cell(l.clone(), i + 1),
        _ => V::Null,
    })
}

pub fn foreach_current_index(cell: &V) -> R {
    Ok(V::Int(cell.cell_index()))
}

// Values.

#[allow(non_snake_case)]
pub fn deparseNumericOnly(state: &V, value: &V) -> R {
    if !value.truthy() {
        return Err("deparse: unexpected NULL value in deparseNumericOnly".to_string());
    }
    if value.is_a(t::T_Integer) {
        let text = value.get(f::F_ival).int().to_string();
        return deparseAppendStringInfoString(state, &V::s(&text));
    }
    if value.is_a(t::T_Float) {
        return deparseAppendStringInfoString(state, &value.get(f::F_fval));
    }
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn deparseValue(state: &V, value: &V, context: &V) -> R {
    if !value.truthy() {
        return deparseAppendStringInfoString(state, &V::s("NULL"));
    }
    match value.tag() {
        Some(t::T_Integer | t::T_Float) => deparseNumericOnly(state, value),
        Some(t::T_Boolean) => deparseAppendStringInfoString(state, &V::s(if value.get(f::F_boolval).truthy() { "true" } else { "false" })),
        Some(t::T_String) => {
            let text = value.get(f::F_sval);
            match context.int() {
                CONTEXT_IDENTIFIER => deparseAppendStringInfoString(state, &quote_identifier(&text)?),
                CONTEXT_CONSTANT => deparseStringLiteral(state, &text),
                _ => deparseAppendStringInfoString(state, &text),
            }
        }
        Some(t::T_BitString) => {
            let text = bytes(&value.get(f::F_bsval));
            if let Some((b'x' | b'b', rest)) = text.split_first().map(|(c, r)| (*c, r)) {
                deparseAppendStringInfoChar(state, &V::Int(text[0] as i64))?;
                deparseStringLiteral(state, &V::bytes(rest))?;
            }
            Ok(V::Null)
        }
        other => Err(format!("deparse: unrecognized value node type: {}", other.unwrap_or(0))),
    }
}

#[allow(non_snake_case)]
pub fn deparseAConst(state: &V, constant: &V) -> R {
    let value = if constant.get(f::F_isnull).truthy() { V::Null } else { constant.get(f::F_val) };
    deparseValue(state, &value, &V::Int(CONTEXT_CONSTANT))
}

#[allow(non_snake_case)]
pub fn optBooleanValue(node: &V) -> R {
    if !node.truthy() {
        return Ok(V::Int(1));
    }
    Ok(match node.tag() {
        Some(t::T_String) => {
            let text = bytes(&strVal(node)?);
            V::b(text.get(..3.min(text.len())) == Some(b"on".as_slice()) && text.len() == 2)
        }
        Some(t::T_Integer) => V::b(intVal(node)?.int() != 0),
        Some(t::T_Boolean) => boolVal(node)?,
        _ => V::Int(0),
    })
}

/// One `RawStmt`, written to `out`, as `deparseRawStmtOpts` writes it with the default options.
pub fn raw_stmt(raw: &V, statement: fn(&V, &V) -> R, out: &mut Vec<u8>) -> Result<(), String> {
    let stmt = raw.get(f::F_stmt);
    if !stmt.truthy() {
        return Err("deparse error in deparseRawStmt: RawStmt with empty Stmt".to_string());
    }
    with(|s| *s = State::default());
    let state = V::Null;
    let wrote = statement(&state, &stmt);
    if wrote.is_ok() {
        emit(out);
    }
    with(|s| *s = State::default());
    wrote.map(|_| ())
}
