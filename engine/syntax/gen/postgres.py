#!/usr/bin/env python3
"""Generates the Postgres parser's tables and token list from Postgres's own grammar.

    python3 engine/syntax/gen/postgres.py --postgres <checkout> --libpg-query <checkout> --pg-query <checkout of pg_query.rs>

Takes `gram.y` and `kwlist.h` at the tag below, applies the parser patches libpg_query carries, so
that the grammar is the one that tree was defined by, removes the C actions and runs Bison over
what is left. Needs Bison 3.8.2 (BISON names it) and writes `src/postgres/generated/`.
"""
import argparse, hashlib, json, os, re, struct, subprocess, sys, tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.dont_write_bytecode = True
import postgres_actions

TAG = "REL_18_6"
LIBPG_QUERY = "c1546e7e97edc93fe8474d7a59881e91fbf48cfc"
PG_QUERY = "ad0c87ed59c84ec6b42c970bb42d7985b7b5878b"
PATCHES = ["01_parser_additional_param_ref_support", "03_lexer_track_yyllocend", "04_lexer_comments_as_tokens", "09_allow_param_junk", "14_avoid_quadratic_memory_dotted_names", "15_node_string_constant_locations"]
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "src", "postgres", "generated")


def git(repo, *args):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True).stdout


def grammar_patch(text):
    """The hunks of a patch that touch gram.y, whichever header style the patch uses."""
    keep, out = False, []
    for part in re.split(r"(?m)^(?=(?:diff |--- ))", text):
        if part.startswith(("diff ", "--- ")):
            keep = "src/backend/parser/gram.y" in part.split("\n", 1)[0]
        if keep:
            out.append(part)
    return "".join(out)


def patched_grammar(args):
    with tempfile.TemporaryDirectory() as tmp:
        path = os.path.join(tmp, "src/backend/parser")
        os.makedirs(path)
        with open(os.path.join(path, "gram.y"), "wb") as f:
            f.write(git(args.postgres, "show", f"{TAG}:src/backend/parser/gram.y"))
        for name in PATCHES:
            text = git(args.libpg_query, "show", f"{LIBPG_QUERY}:patches/{name}.patch").decode("utf8", "replace")
            hunks = grammar_patch(text)
            if hunks:
                subprocess.run(["patch", "-s", "-p1", "--no-backup-if-mismatch"], cwd=tmp, input=hunks.encode(), check=True)
        with open(os.path.join(path, "gram.y"), encoding="utf8", errors="replace") as f:
            return f.read()


def without_actions(source):
    head, rules, *_ = re.split(r"^%%\s*$", source, flags=re.M)
    out, i, depth = [], 0, 0
    while i < len(rules):
        c = rules[i]
        if rules.startswith("/*", i):
            end = rules.find("*/", i + 2) + 2
            if not depth:
                out.append(" ")
            i = end
            continue
        if not depth and c == "'":
            end = rules.find("'", i + 1) + 1
            out.append(rules[i:end])
            i = end
            continue
        if depth and c in "\"'":
            j = i + 1
            while rules[j] != c:
                j += 2 if rules[j] == "\\" else 1
            i = j + 1
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
        elif not depth:
            out.append(c)
        i += 1
    head = re.sub(r"%\{.*?%\}", "", head, flags=re.S)
    head = re.sub(r"/\*.*?\*/", " ", head, flags=re.S)
    head = re.sub(r"%union\s*\{.*?\}\s*", "", head, count=1, flags=re.S)
    head = re.sub(r"^%(parse-param|lex-param|name-prefix|locations|pure-parser|define).*$", "", head, flags=re.M)
    head = re.sub(r"%type\s*<\w+>", "%nterm", head)
    head = re.sub(r"%token\s*<\w+>", "%token", head)
    return head + "\n%%\n" + "".join(out) + "\n%%\n"


def tables(c_source):
    def array(name):
        m = re.search(r"static const [\w ]+?\b" + name + r"\[\]\s*=\s*\{(.*?)\};", c_source, re.S)
        return [int(x) for x in re.findall(r"-?\d+", m.group(1))]

    def define(name):
        return int(re.search(r"#define " + name + r"\s+\(?(-?\d+)\)?", c_source).group(1))

    names = re.search(r"yytname\[\]\s*=\s*\{(.*?)\};", c_source, re.S).group(1)
    names = [bytes(x, "utf8").decode("unicode_escape") for x in re.findall(r'"((?:[^"\\]|\\.)*)"', names)]
    return {k: array("yy" + k) for k in "pact pgoto defact defgoto table check r1 r2".split()}, {
        "pact_ninf": define("YYPACT_NINF"),
        "table_ninf": define("YYTABLE_NINF"),
        "final": define("YYFINAL"),
        "last": define("YYLAST"),
        "ntokens": define("YYNTOKENS"),
    }, names


def rust_name(symbol):
    return re.sub(r"\W", "_", symbol)


class Constants:
    """Every C constant the actions name: an enumerator as the protobuf numbers it, or a `#define`."""

    def __init__(self, enum_defs, headers):
        self.proto, self.enums = {}, {}
        for name, body in re.findall(r"\n_enumToInt(\w+)\(\w+ value\) \{(.*?)\n\}", enum_defs, re.S):
            values = re.findall(r"case (\w+): return (\d+);", body)
            self.enums[name] = values
            for c, number in values:
                self.proto[c] = int(number)
        self.defines, self.plain = {}, {}
        for text in headers:
            text = re.sub(r"//[^\n]*", "", re.sub(r"/\*.*?\*/", " ", text, flags=re.S)).replace("\\\n", " ")
            for name, value in re.findall(r"^#\s*define\s+(\w+)[ \t]+(.+?)\s*$", text, flags=re.M):
                self.defines.setdefault(name, value)
            for body in re.findall(r"\benum\s*\w*\s*\{(.*?)\}", text, re.S):
                number = 0
                for item in body.split(","):
                    item = item.strip()
                    if not item or item.startswith("#"):
                        continue
                    m = re.match(r"(\w+)\s*(?:=\s*(.+))?$", item, re.S)
                    if not m:
                        continue
                    if m.group(2) is not None:
                        try:
                            number = self.evaluate(m.group(2).strip())
                        except Exception:
                            number = None
                    if number is not None:
                        self.plain.setdefault(m.group(1), number)
                        number += 1
                    else:
                        break

    def evaluate(self, expression):
        expression = expression.strip()
        expression = re.sub(r"'(\\?.)'", lambda m: str(ord(m.group(1)[-1])), expression)
        expression = re.sub(r"\b(0[xX][0-9a-fA-F]+|\d+)[uUlL]+\b", r"\1", expression)
        expression = re.sub(r"\([a-z_A-Z0-9]+\)\s*(?=\d|\()", "", expression)

        def name(m):
            return str(self(m.group(0)))

        expression = re.sub(r"\b[A-Za-z_]\w*\b", name, expression)
        return int(eval(expression, {"__builtins__": {}}))

    # What a header generated at build time defines, or a type's bound.
    KNOWN = {"PG_INT32_MAX": 2147483647, "InvalidOid": 0, "InvalidRelFileNumber": 0, "PG_INT16_MAX": 32767, "TEXTOID": 25, "FETCH_ALL": (1 << 63) - 1}

    def __call__(self, name):
        if name in self.KNOWN:
            return self.KNOWN[name]
        if name in self.plain:
            return self.plain[name]
        if name in self.defines:
            return self.evaluate(self.defines[name])
        raise KeyError(name)

    def proto_arms(self, enum):
        """Each enumerator's C value and protobuf number."""
        return [(self(c), int(number)) for c, number in self.enums[enum]]


# The constants `helpers.rs` names.
HELPER_CONSTANTS = ["AEXPR_OP", "AND_EXPR", "OR_EXPR", "NOT_EXPR", "DEFELEM_UNSPEC", "RELPERSISTENCE_PERMANENT", "FUNC_PARAM_OUT", "FUNC_PARAM_TABLE", "FUNC_PARAM_VARIADIC", "FUNC_PARAM_DEFAULT", "FUNC_PARAM_IN", "LIMIT_OPTION_WITH_TIES", "LockWaitSkip", "PARTITION_STRATEGY_LIST", "PARTITION_STRATEGY_RANGE", "PARTITION_STRATEGY_HASH", "PUBLICATIONOBJ_CONTINUATION", "PUBLICATIONOBJ_TABLE", "PUBLICATIONOBJ_TABLES_IN_SCHEMA", "PUBLICATIONOBJ_TABLES_IN_CUR_SCHEMA", "CTEMaterializeDefault", "CAS_DEFERRABLE", "CAS_INITIALLY_DEFERRED", "CAS_NOT_VALID", "CAS_NO_INHERIT", "CAS_NOT_ENFORCED", "CAS_ENFORCED"]
# The fields `helpers.rs` names that no action or schema entry does.
HELPER_FIELDS = ["parsetree", "val", "fval", "sval", "ival", "boolval", "bsval", "isnull"]


def plain_protobuf(text):
    """pg_query's prost-generated structs, without prost: plain derives, and the accessors prost's derive adds for enum fields."""
    out, accessors, current, pending, i = [], {}, None, None, 0
    start = text.rindex("#[derive(serde::Serialize)]", 0, text.index("pub struct SummaryResult"))
    module = text.index("pub mod summary_result {")
    depth, j = 0, text.index("{", module)
    while True:
        depth += {"{": 1, "}": -1}.get(text[j], 0)
        j += 1
        if depth == 0:
            break
    text = text[:start] + text[j:]
    lines = text.split("\n")
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        if stripped.startswith("#[prost("):
            attr = stripped
            while not attr.rstrip().endswith(")]"):
                i += 1
                attr += lines[i].strip()
            m = re.search(r'enumeration = "(\w+)"', attr)
            pending = m.group(1) if m else None
            i += 1
            continue
        m = re.match(r"\s*pub struct (\w+)", line)
        if m:
            current = m.group(1)
        m = re.match(r"\s*pub (r#)?(\w+): ", line)
        if m and pending and current:
            accessors.setdefault(current, []).append(((m.group(1) or "") + m.group(2), m.group(2), pending))
        if m:
            pending = None
        line = line.replace(", ::prost::Message)]", ", Debug, Default)]")
        line = line.replace("#[derive(Clone, PartialEq, ::prost::Oneof)]", "#[derive(Clone, PartialEq, Debug)]")
        line = line.replace(", ::prost::Enumeration)]", ")]")
        line = line.replace("::prost::alloc::vec::Vec", "Vec").replace("::prost::alloc::boxed::Box", "Box").replace("::prost::alloc::string::String", "::std::string::String").replace("::core::option::Option", "Option")
        out.append(line)
        i += 1
    body = re.sub(r",\s*::prost::Enumeration,?\s*\)\]", ")]", "\n".join(out))
    for name in re.findall(r"#\[repr\(i32\)\]\s*pub enum (\w+)", body):
        first = re.search(r"pub enum " + name + r" \{\s*(\w+) = 0", body).group(1)
        body += f"""
impl Default for {name} {{
    fn default() -> Self {{
        Self::{first}
    }}
}}
impl From<{name}> for i32 {{
    fn from(value: {name}) -> i32 {{
        value as i32
    }}
}}
impl TryFrom<i32> for {name} {{
    type Error = i32;
    fn try_from(value: i32) -> Result<Self, i32> {{
        ENUM_VALUES_{name}.iter().copied().find(|v| *v as i32 == value).ok_or(value)
    }}
}}
"""
        block = re.search(r"pub enum " + name + r" \{(.*?)\n\}", body, re.S).group(1)
        variants = re.findall(r"^\s*(\w+) = -?\d+,", block, flags=re.M)
        body += f"#[allow(non_upper_case_globals)]\nstatic ENUM_VALUES_{name}: [{name}; {len(variants)}] = [" + ", ".join(f"{name}::{v}" for v in variants) + "];\n"
    for struct, fields in accessors.items():
        body += f"impl {struct} {{\n"
        for raw, plain, enum in fields:
            body += f"    pub fn {raw}(&self) -> {enum} {{\n        {enum}::try_from(self.{raw}).unwrap_or_default()\n    }}\n"
            body += f"    pub fn set_{plain}(&mut self, value: {enum}) {{\n        self.{raw} = value as i32;\n    }}\n"
        body += "}\n"
    return "// Generated by engine/syntax/gen/postgres.py from pg_query's protobuf definitions: the tree's types, with no wire format.\n#![allow(clippy::all, non_camel_case_types)]\n" + body


class Schema:
    """libpg_query's description of every parse node: its fields, in `pg_query_outfuncs_defs.c`."""

    HAND = {"List", "IntList", "OidList", "Integer", "Float", "Boolean", "String", "BitString", "AConst"}

    def __init__(self, defs, conds, pb):
        self.outs = {}
        for name, message, c_type, body in re.findall(r"\n_out(\w+)\(OUT_TYPE\((\w+), \w+\) out, const (\w+) \*node\)\s*\{(.*?)\n\}", defs, re.S):
            fields = []
            for kind, args in re.findall(r"WRITE_(\w+?)(?:_FIELD)?\((.*?)\);", body):
                fields.append((kind, [a.strip() for a in args.split(",")]))
            self.outs[name] = (message, c_type, fields)
        self.conds = []
        tag = None
        for line in conds.split("\n"):
            line = line.strip()
            if line.startswith("case T_"):
                tag = line[7:].rstrip(":")
            elif line.startswith("OUT_NODE("):
                parts = [p.strip() for p in line[len("OUT_NODE(") :].rstrip(");").split(",")]
                self.conds.append((tag, parts[0], parts[1], parts[4]))
        self.struct_fields = {}
        for name, block in re.findall(r"pub struct (\w+) \{([^{}]*)\}", pb):
            flat = re.sub(r"\s*\n\s*", " ", block)
            self.struct_fields[name] = {}
            for raw, plain, ty in re.findall(r"pub (r#)?(\w+): (.*?),(?= pub | *$)", flat):
                self.struct_fields[name][plain] = (raw + plain, ty)
        self.structs = {n.replace("_", "").lower(): n for n in self.struct_fields}
        self.variants = {}
        node = pb[pb.index("pub enum Node {") :]
        node = node[node.index("{") + 1 : node.index("\n    }")]
        for variant, payload in re.findall(r"(\w+)\(\s*(.*?)\s*\),", node, re.S):
            self.variants[payload.rsplit("super::", 1)[-1].rstrip(">, \n")] = (variant, "Box<" in payload)

    def rust(self, message):
        return self.structs[message.replace("_", "").lower()]

    def field_names(self):
        names = set()
        for _, _, fields in self.outs.values():
            for kind, args in fields:
                names.add(self.parts(kind, args)[1].split(".")[-1])
        return names

    @staticmethod
    def parts(kind, args):
        if kind == "ENUM":
            return args[2], args[4]
        if kind in ("SPECIFIC_NODE", "SPECIFIC_NODE_PTR"):
            return args[3], args[5]
        return args[1], args[3]

    def reader(self, constants, int_enums):
        """From the tree's structs back to nodes, for the deparser."""
        out = ["// Generated by engine/syntax/gen/postgres.py from libpg_query's description of each node.", ""]
        for enum, (arms, default) in sorted(int_enums.items()):
            rendered = " ".join(f"{number} => {constants(c)}," for number, c in arms)
            out.append(f"#[allow(non_snake_case, unreachable_patterns)]\nfn c_{enum}(v: i32) -> i64 {{\n    match v {{ {rendered} _ => {constants(default)} }}\n}}\n")
        for name, (message, c_type, fields) in sorted(self.outs.items()):
            if name in self.HAND:
                continue
            rust = self.rust(message)
            types = self.struct_fields[rust]
            out.append(f"#[allow(non_snake_case, unused_variables)]\nfn r_{name}(m: &pb::{rust}) -> V {{\n    let n = V::node(t::T_{c_type});")
            for kind, args in fields:
                outname, fld = self.parts(kind, args)
                rname, rtype = types[outname]
                c, x = f"f::F_{fld}", f"m.{rname}"
                if kind in ("INT", "UINT", "UINT64", "LONG", "FLOAT"):
                    value = f"V::Int({x} as i64)"
                elif kind == "BOOL":
                    value = f"V::b({x})"
                elif kind == "CHAR":
                    value = f"ch(&{x})"
                elif kind == "STRING":
                    value = f"s(&{x})"
                elif kind == "ENUM":
                    value = f"V::Int(c_{args[1]}({x}))"
                elif kind == "LIST":
                    value = f"l(&{x})"
                elif kind == "BITMAPSET":
                    continue
                elif kind == "NODE_PTR":
                    value = f"match &{x} {{ Some(v) => r_node(v), None => V::Null }}"
                elif kind in ("SPECIFIC_NODE_PTR", "SPECIFIC_NODE"):
                    value = f"match &{x} {{ Some(v) => r_{args[1]}(v), None => V::Null }}"
                else:
                    raise SystemExit("a new field kind: " + kind)
                out.append(f"    n.set({c}, {value});")
            out.append("    n\n}\n")
        out.append(READ_HAND)
        out.append("/// One node, by its variant.\n#[allow(non_snake_case)]\nfn r_node(m: &pb::Node) -> V {\n    use pb::node::Node as N;\n    match &m.node {")
        for tag, message, function, c_type in self.conds:
            if tag in ("IntList", "OidList"):
                continue
            rust = self.rust(message)
            variant, _ = self.variants[rust]
            if tag == "List":
                out.append(f"        Some(N::{variant}(x)) => V::list(x.items.iter().map(r_node).collect()),")
            else:
                out.append(f"        Some(N::{variant}(x)) => r_{function}(x),")
        out.append("        _ => V::Null,\n    }\n}\n")
        return "\n".join(out)

    def converter(self, constants, headers_text):
        out = ["// Generated by engine/syntax/gen/postgres.py from libpg_query's description of each node. Included where `pb` names the tree's types.", ""]
        for enum in sorted(constants.enums):
            arms = " ".join(f"{value} => {number}," for value, number in constants.proto_arms(enum))
            out.append(f"#[allow(non_snake_case, unreachable_patterns)]\nfn enum_{enum}(v: i64) -> i32 {{\n    match v {{ {arms} _ => -1 }}\n}}\n")
        for name, (message, c_type, fields) in sorted(self.outs.items()):
            if name in self.HAND:
                continue
            rust = self.rust(message)
            types = self.struct_fields[rust]
            out.append(f"#[allow(non_snake_case, unused_variables, clippy::needless_update)]\nfn m_{name}(n: &Obj, d: &mut Depth) -> Result<pb::{rust}, Deep> {{\n    d.enter()?;\n    let message = pb::{rust} {{")
            for kind, args in fields:
                outname, fld = self.parts(kind, args)
                if "." in fld:
                    raise SystemExit(f"a nested field: {name}.{fld}")
                rname, rtype = types[outname]
                c = f"f::F_{fld}"
                boxed = "Box<" in rtype
                if kind in ("INT", "UINT", "UINT64", "LONG"):
                    value = f"n.int({c}) as {rtype}"
                elif kind == "FLOAT":
                    value = f"n.int({c}) as {rtype}"
                elif kind == "BOOL":
                    value = f"n.int({c}) != 0"
                elif kind == "CHAR":
                    value = f"chr(n.int({c}))"
                elif kind == "STRING":
                    value = f"n.string({c})"
                elif kind == "ENUM":
                    value = f"enum_{args[1]}(n.int({c}))"
                elif kind == "LIST":
                    value = f"list(&n.value({c}), d)?"
                elif kind == "BITMAPSET":
                    value = "Vec::new()"
                elif kind == "NODE_PTR":
                    wrapped = f"pb::Node {{ node: node(&v, d)? }}"
                    value = f"{{ let v = n.value({c}); if !v.truthy() {{ None }} else {{ Some({'Box::new(' + wrapped + ')' if boxed else wrapped}) }} }}"
                elif kind == "SPECIFIC_NODE_PTR":
                    call = f"m_{args[1]}(o, d)?"
                    value = f"match n.value({c}) {{ V::Node(o) => {{ let o = &*o; Some({'Box::new(' + call + ')' if boxed else call}) }} _ => None }}"
                elif kind == "SPECIFIC_NODE":
                    call = f"m_{args[1]}(o, d)?"
                    value = f"match n.value({c}) {{ V::Node(o) => {{ let o = &*o; Some({'Box::new(' + call + ')' if boxed else call}) }} _ => None }}"
                else:
                    raise SystemExit("a new field kind: " + kind)
                out.append(f"        {rname}: {value},")
            out.append("        ..Default::default()\n    };\n    d.leave();\n    Ok(message)\n}\n")
        out.append(HAND_CONVERTERS)
        arms = []
        for tag, message, function, c_type in self.conds:
            if tag in ("List", "IntList", "OidList"):
                continue
            rust = self.rust(message)
            variant, boxed = self.variants[rust]
            call = f"m_{function}(o, d)?"
            out.append(f"#[inline(never)]\n#[allow(non_snake_case)]\nfn n_{tag}(o: &Obj, d: &mut Depth) -> Result<Option<pb::node::Node>, Deep> {{\n    Ok(Some(pb::node::Node::{variant}({'Box::new(' + call + ')' if boxed else call})))\n}}\n")
            arms.append(f"        t::T_{tag} => n_{tag}(o, d),")
        # Each node's conversion has a function of its own, so that this frame, which every level of a tree spends, holds none of their temporaries.
        out.append("/// One node, by its tag.\n#[allow(non_snake_case)]\nfn node_at(o: &Obj, d: &mut Depth) -> Result<Option<pb::node::Node>, Deep> {\n    match o.tag {")
        out.extend(arms)
        out.append("        _ => Ok(None),\n    }\n}\n")
        return "\n".join(out)


READ_HAND = """
fn s(v: &::std::string::String) -> V {
    if v.is_empty() { V::Null } else { V::bytes(v.as_bytes()) }
}

fn ch(v: &::std::string::String) -> V {
    V::Int(v.as_bytes().first().copied().unwrap_or(0) as i64)
}

fn l(items: &[pb::Node]) -> V {
    if items.is_empty() { V::Null } else { V::list(items.iter().map(r_node).collect()) }
}

fn value(tag: u16, field: u16, v: V) -> V {
    let n = V::node(tag);
    n.set(field, v);
    n
}

#[allow(non_snake_case)]
fn r_Integer(m: &pb::Integer) -> V { value(t::T_Integer, f::F_ival, V::Int(m.ival as i64)) }
#[allow(non_snake_case)]
fn r_Float(m: &pb::Float) -> V { value(t::T_Float, f::F_fval, V::bytes(m.fval.as_bytes())) }
#[allow(non_snake_case)]
fn r_Boolean(m: &pb::Boolean) -> V { value(t::T_Boolean, f::F_boolval, V::b(m.boolval)) }
#[allow(non_snake_case)]
fn r_String(m: &pb::String) -> V { value(t::T_String, f::F_sval, V::bytes(m.sval.as_bytes())) }
#[allow(non_snake_case)]
fn r_BitString(m: &pb::BitString) -> V { value(t::T_BitString, f::F_bsval, V::bytes(m.bsval.as_bytes())) }

#[allow(non_snake_case)]
fn r_AConst(m: &pb::AConst) -> V {
    use pb::a_const::Val;
    let n = V::node(t::T_A_Const);
    n.set(f::F_isnull, V::b(m.isnull));
    n.set(f::F_location, V::Int(m.location as i64));
    match &m.val {
        Some(Val::Ival(x)) => n.set(f::F_val, r_Integer(x)),
        Some(Val::Fval(x)) => n.set(f::F_val, r_Float(x)),
        Some(Val::Boolval(x)) => n.set(f::F_val, r_Boolean(x)),
        Some(Val::Sval(x)) => n.set(f::F_val, r_String(x)),
        Some(Val::Bsval(x)) => n.set(f::F_val, r_BitString(x)),
        None => {}
    }
    n
}

/// A statement's tree as the nodes the deparser reads.
pub fn raw_stmt(m: &pb::RawStmt) -> V {
    r_RawStmt(m)
}
"""

HAND_CONVERTERS = """
fn chr(c: i64) -> ::std::string::String {
    if c == 0 { ::std::string::String::new() } else { (c as u8 as char).to_string() }
}

fn node(v: &V, d: &mut Depth) -> Result<Option<pb::node::Node>, Deep> {
    d.nodes += 1;
    if d.nodes > d.max / 2 {
        return Err(Deep);
    }
    d.enter()?;
    let out = match v {
        V::Node(o) => node_at(o, d)?,
        V::List(_) => Some(pb::node::Node::List(m_List(v, d)?)),
        _ => None,
    };
    d.leave();
    d.nodes -= 1;
    Ok(out)
}

fn list(v: &V, d: &mut Depth) -> Result<Vec<pb::Node>, Deep> {
    v.items().iter().map(|x| Ok(pb::Node { node: node(x, d)? })).collect()
}

#[allow(non_snake_case)]
fn m_List(v: &V, d: &mut Depth) -> Result<pb::List, Deep> {
    d.enter()?;
    let message = pb::List { items: list(v, d)? };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
fn m_Integer(n: &Obj, d: &mut Depth) -> Result<pb::Integer, Deep> {
    d.enter()?;
    let message = pb::Integer { ival: n.int(f::F_ival) as i32 };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
fn m_Float(n: &Obj, d: &mut Depth) -> Result<pb::Float, Deep> {
    d.enter()?;
    let message = pb::Float { fval: n.string(f::F_fval) };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
fn m_Boolean(n: &Obj, d: &mut Depth) -> Result<pb::Boolean, Deep> {
    d.enter()?;
    let message = pb::Boolean { boolval: n.int(f::F_boolval) != 0 };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
fn m_String(n: &Obj, d: &mut Depth) -> Result<pb::String, Deep> {
    d.enter()?;
    let message = pb::String { sval: n.string(f::F_sval) };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
fn m_BitString(n: &Obj, d: &mut Depth) -> Result<pb::BitString, Deep> {
    d.enter()?;
    let message = pb::BitString { bsval: n.string(f::F_bsval) };
    d.leave();
    Ok(message)
}

#[allow(non_snake_case)]
fn m_AConst(n: &Obj, d: &mut Depth) -> Result<pb::AConst, Deep> {
    use pb::a_const::Val;
    d.enter()?;
    let isnull = n.int(f::F_isnull) != 0;
    let val = if isnull {
        None
    } else {
        match n.value(f::F_val) {
            V::Node(o) => match o.tag {
                t::T_Integer => Some(Val::Ival(m_Integer(&o, d)?)),
                t::T_Float => Some(Val::Fval(m_Float(&o, d)?)),
                t::T_Boolean => Some(Val::Boolval(m_Boolean(&o, d)?)),
                t::T_String => Some(Val::Sval(m_String(&o, d)?)),
                t::T_BitString => Some(Val::Bsval(m_BitString(&o, d)?)),
                _ => None,
            },
            _ => None,
        }
    };
    d.leave();
    Ok(pb::AConst { isnull, location: n.int(f::F_location) as i32, val })
}

/// The statements of a parse, as `_outRawStmt` writes each.
pub fn statements(list: &V, max: u32) -> Result<Vec<pb::RawStmt>, Deep> {
    let mut d = Depth::new(max);
    list.items().iter().filter_map(|v| match v { V::Node(o) => Some(m_RawStmt(o, &mut d)), _ => None }).collect()
}
"""


def braced(text, start):
    """The text from the `{` at or after `start` through its matching `}`."""
    i, depth = text.index("{", start), 0
    j = i
    while True:
        depth += {"{": 1, "}": -1}.get(text[j], 0)
        j += 1
        if depth == 0:
            return text[i:j]


def walker(crate_file):
    """pg_query's walk over a tree, with the references it hands out, under its MIT license."""
    enum_rs, ref_rs, notice = crate_file("src/node_enum.rs"), crate_file("src/node_ref.rs"), crate_file("LICENSE")
    variants = re.findall(r"^    (\w+)\(&'a protobuf::(\w+)\),", ref_rs, flags=re.M)
    nodes = braced(enum_rs, enum_rs.index("pub fn nodes(&self)"))
    to_ref = braced(enum_rs, enum_rs.index("pub fn to_ref(&self)"))
    out = ["// Generated by engine/syntax/gen/postgres.py from the pg_query crate's node walker, under its license:"]
    out += ["// " + line if line else "//" for line in notice.strip().split("\n")]
    out += ["", "#![allow(clippy::all)]", "use super::pb as protobuf;", "use super::pb::node::Node as NodeEnum;", "use std::collections::VecDeque;", ""]
    out.append("#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, serde::Serialize)]\npub enum Context {\n    None,\n    Select,\n    DML,\n    DDL,\n    Call,\n}\n")
    out.append("#[derive(Debug, Copy, Clone)]\npub enum NodeRef<'a> {")
    out += [f"    {v}(&'a protobuf::{t})," for v, t in variants]
    out.append("}\n")
    for v, t in variants:
        out.append(f"impl protobuf::{t} {{\n    pub fn to_ref(&self) -> NodeRef<'_> {{\n        NodeRef::{v}(self)\n    }}\n}}")
    out.append("\nimpl NodeEnum {\n    pub fn nodes(&self) -> Vec<(NodeRef<'_>, i32, Context, bool)> " + nodes + "\n\n    pub fn to_ref(&self) -> NodeRef<'_> " + to_ref + "\n}\n")
    out.append("impl protobuf::ParseResult {\n    /// The nodes of every statement, as pg_query walks them.\n    pub fn nodes(&self) -> Vec<(NodeRef<'_>, i32, Context, bool)> {\n        self.stmts.iter().filter_map(|s| s.stmt.as_ref().and_then(|s| s.node.as_ref()).map(|n| n.nodes())).flatten().collect()\n    }\n}")
    return "\n".join(out) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--postgres", required=True)
    parser.add_argument("--libpg-query", required=True)
    parser.add_argument("--pg-query", required=True)
    args = parser.parse_args()
    source = patched_grammar(args)
    stripped = without_actions(source)
    bison = os.environ.get("BISON", "/opt/homebrew/opt/bison/bin/bison")
    with tempfile.TemporaryDirectory() as tmp:
        y, c = os.path.join(tmp, "gram.y"), os.path.join(tmp, "gram.c")
        open(y, "w", encoding="utf8").write(stripped)
        subprocess.run([bison, "-Wno-other", "-o", c, y], check=True)
        arrays, constants, names = tables(open(c, encoding="utf8").read())
    order = ["pact", "pgoto", "defact", "defgoto", "table", "check", "r1", "r2"]
    os.makedirs(OUT, exist_ok=True)

    # The actions, in Rust, over the tree's types.
    tree = "src/include"
    files = git(args.postgres, "ls-tree", "-r", "--name-only", TAG, tree).decode().split()
    headers = [git(args.postgres, "show", f"{TAG}:{p}").decode("utf8", "replace") for p in files if p.endswith(".h")]
    prologue = source[: source.index("%}")]
    limit_patch = git(args.libpg_query, "show", f"{LIBPG_QUERY}:patches/05_limit_option_enum_value_default.patch").decode("utf8", "replace")
    nodes_h = next(h for p, h in zip([p for p in files if p.endswith(".h")], headers) if p.endswith("nodes/nodes.h"))
    if "LIMIT_OPTION_DEFAULT" in limit_patch:
        nodes_h = nodes_h.replace("typedef enum LimitOption\n{\n", "typedef enum LimitOption\n{\n\tLIMIT_OPTION_DEFAULT,\n")
    headers.append(nodes_h)
    headers.append(prologue)
    lib = lambda p: git(args.libpg_query, "show", f"{LIBPG_QUERY}:{p}").decode("utf8", "replace")
    deparser = lib("src/postgres_deparse.c")
    c_constants = Constants(lib("src/include/pg_query_enum_defs.c"), [nodes_h] + headers + [prologue, deparser])
    crate_file = lambda p: git(args.pg_query, "show", f"{PG_QUERY}:{p}").decode("utf8")
    pb_source = crate_file("src/protobuf.rs")
    pb = plain_protobuf(pb_source)
    open(os.path.join(OUT, "pb.rs"), "w").write(pb)
    open(os.path.join(OUT, "walk.rs"), "w").write(walker(crate_file))
    schema = Schema(lib("src/include/pg_query_outfuncs_defs.c"), lib("src/include/pg_query_outfuncs_conds.c"), pb)

    def check(rules):
        if len(rules) + 1 != len(arrays["r1"]) - 1:
            raise SystemExit(f"{len(rules)} rules read, Bison has {len(arrays['r1']) - 2}")
        for index, (lhs, symbols, _) in enumerate(rules):
            rule = index + 2
            if names[arrays["r1"][rule]] != lhs or arrays["r2"][rule] != len(symbols):
                raise SystemExit(f"rule {rule}: read {lhs}/{len(symbols)}, Bison has {names[arrays['r1'][rule]]}/{arrays['r2'][rule]}")

    missing = set()

    def constant(name):
        try:
            return c_constants(name)
        except Exception:
            missing.add(name)
            return 0

    actions, found, tags, action_fields, rules = postgres_actions.translate(source, check, constant)
    if missing:
        raise SystemExit("constants no header defines: " + ", ".join(sorted(missing)))
    open(os.path.join(OUT, "actions.rs"), "w").write(actions)

    deparse_fields = set()

    def deparse_field(name):
        deparse_fields.add(name)
        return f"f::F_{name}"

    deparse_rs, deparse_calls = postgres_actions.translate_deparser(deparser, constant, deparse_field)
    if missing:
        raise SystemExit("constants no header defines (deparser): " + ", ".join(sorted(missing)))
    open(os.path.join(OUT, "deparse.rs"), "w").write(deparse_rs)
    action_fields |= deparse_fields
    print("deparser calls:", " ".join(deparse_calls))

    pseudo = re.findall(r"typedef struct (\w+)\s*\{(.*?)\}", prologue, re.S)
    all_tags = sorted({c[0] for c in schema.conds} | {n for n, _ in pseudo} | set(re.findall(r"\bt::T_(\w+)", actions + deparse_rs)) | {"A_Star", "List"})
    all_fields = sorted(schema.field_names() | action_fields | set(HELPER_FIELDS))
    file_headers = ""
    N = ["// Generated by engine/syntax/gen/postgres.py: the parse nodes' tags and field names.", "#![allow(non_upper_case_globals, dead_code)]", "pub mod t {"]
    N.extend(f"    pub const T_{n}: u16 = {i};" for i, n in enumerate(all_tags))
    N.append("}\npub mod f {")
    N.extend(f"    pub const F_{n}: u16 = {i};" for i, n in enumerate(all_fields))
    N.append("}\npub mod c {")
    N.extend(f"    pub const {n}: i64 = {c_constants(n)};" for n in HELPER_CONSTANTS)
    N.append("}")
    open(os.path.join(OUT, "names.rs"), "w").write("\n".join(N) + "\n")
    open(os.path.join(OUT, "convert.rs"), "w").write(schema.converter(c_constants, file_headers))
    int_enums = {}
    for name, body in re.findall(r"\n_intToEnum(\w+)\(int value\) \{(.*?)\n\}", lib("src/include/pg_query_enum_defs.c"), re.S):
        arms = [(int(n), c) for n, c in re.findall(r"case (\d+): return (\w+);", body)]
        default = re.findall(r"^\s*return (\w+);", body, flags=re.M)[-1]
        int_enums[name] = (arms, default)
    open(os.path.join(OUT, "read.rs"), "w").write(schema.reader(c_constants, int_enums))
    wide = {k for k in order if max(map(abs, arrays[k])) > 32767}
    blob = b"".join(struct.pack(f"<{len(arrays[k])}{'i' if k in wide else 'h'}", *arrays[k]) for k in order)
    open(os.path.join(OUT, "tables.bin"), "wb").write(blob)
    kw = git(args.postgres, "show", f"{TAG}:src/include/parser/kwlist.h").decode()
    keywords = re.findall(r'PG_KEYWORD\("([^"]+)", (\w+), (\w+), (\w+)\)', kw)
    categories = {"UNRESERVED_KEYWORD": 0, "COL_NAME_KEYWORD": 1, "TYPE_FUNC_NAME_KEYWORD": 2, "RESERVED_KEYWORD": 3}
    ntokens = constants["ntokens"]
    index = {n: i for i, n in enumerate(names)}
    L = [f"// Generated by engine/syntax/gen/postgres.py from Postgres {TAG}'s gram.y and kwlist.h, with libpg_query {LIBPG_QUERY}'s parser patches.", f"// gram.y sha256 {hashlib.sha256(source.encode()).hexdigest()}", ""]
    for k, v in constants.items():
        L.append(f"pub const {k.upper()}: i32 = {v};")
    major, minor = re.fullmatch(r"REL_(\d+)_(\d+)", TAG).groups()
    L.append(f"/// The version of Postgres this grammar is, as `server_version_num` writes it.\npub const PG_VERSION_NUM: i32 = {int(major) * 10000 + int(minor)};")
    offset = 0
    L.append("/// Each table's start, length and integer width in bytes in `tables.bin`, little-endian.")
    segments = []
    for k in order:
        width = 4 if k in wide else 2
        segments.append(f"({offset}, {len(arrays[k])}, {width})")
        offset += width * len(arrays[k])
    L.append("pub const SEGMENTS: [(usize, usize, usize); 8] = [" + ", ".join(segments) + "];")
    L.append("")
    L.append("#[allow(non_upper_case_globals, dead_code)]\npub mod tok {")
    for i, n in enumerate(names[:ntokens]):
        if re.fullmatch(r"[A-Za-z_]\w*", n) and n not in ("error",):
            L.append(f"    pub const {n}: u16 = {i};")
    L.append("}\n")
    L.append("/// The grammar's symbol of each ASCII character token, 0 where it names none.")
    chars = [0] * 128
    for i, n in enumerate(names[:ntokens]):
        if len(n) == 3 and n[0] == "'" and n[2] == "'" and ord(n[1]) < 128:
            chars[ord(n[1])] = i
    L.append("pub static CHAR_TOKENS: [u16; 128] = [" + ", ".join(map(str, chars)) + "];\n")
    L.append("/// Every keyword, sorted by name: its name, grammar symbol and category (0 unreserved, 1 column name, 2 type or function name, 3 reserved).")
    L.append(f"pub static KEYWORDS: [(&str, u16, u8); {len(keywords)}] = [")
    for name, token, category, _ in sorted(keywords):
        L.append(f'    ("{name}", {index[token]}, {categories[category]}),')
    L.append("];\n")
    L.append(f"pub static NAMES: [&str; {len(names)}] = [")
    L.extend("    " + json.dumps(n) + "," for n in names)
    L.append("];")
    open(os.path.join(OUT, "tokens.rs"), "w").write("\n".join(L) + "\n")
    print(f"{constants['ntokens']} tokens, {len(arrays['r1']) - 1} rules, {len(arrays['pact'])} states, {len(blob)} table bytes, {len(keywords)} keywords")


if __name__ == "__main__":
    main()
