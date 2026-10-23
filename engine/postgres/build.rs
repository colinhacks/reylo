// The hash of this crate's sources, which its fingerprint combines with core's (`src/lib.rs`).
// `vendor/` is the patch to libpg_query, which sets how deeply a statement may nest, and `csrc/`
// the C that runs its deparser.
//
// And the reader of libpg_query's parse tree (`src/raw.rs`): bindings to the C node structs, and
// a converter from each to the `pg_query::protobuf` struct libpg_query would have encoded it as,
// generated from the very files libpg_query generates its protobuf encoder from
// (`src/include/pg_query_outfuncs_defs.c`, `pg_query_outfuncs_conds.c` and
// `pg_query_enum_defs.c`), so every node and field it encodes is read, the same way.
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    reylo_engine_build::emit("REYLO_ENGINE_POSTGRES_HASH", &["build.rs", "src", "csrc", "vendor"]);
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let crate_dir = root.join(".patched/pg_query");
    let lib = crate_dir.join("libpg_query");
    if !lib.exists() {
        panic!("{} is missing: run `engine/build.sh --prepare`", lib.display());
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let defs = Defs::read(&lib, &crate_dir.join("src/protobuf.rs"));
    bindings(&root, &lib, &defs, &out.join("raw_bindings.rs"));
    fs::write(out.join("raw_convert.rs"), defs.converter()).unwrap();
    fs::write(out.join("raw_build.rs"), defs.builder()).unwrap();
    // libpg_query's deparser over nodes, with its errors caught in C (`csrc/deparse.c`).
    cc::Build::new()
        .file(root.join("csrc/deparse.c"))
        .include(lib.join("src/postgres/include"))
        .include(lib.join("src/include"))
        .include(lib.join("src"))
        .include(&lib)
        .flag_if_supported("-fwrapv")
        .flag_if_supported("-fno-strict-aliasing")
        .warnings(false)
        .compile("reylo_deparse");
}

/// The C structs, and the constants and enums the converter reads, for the target being built.
fn bindings(root: &Path, lib: &Path, defs: &Defs, path: &Path) {
    let target = env::var("TARGET").unwrap();
    let header = "#include \"postgres.h\"\n#include \"nodes/parsenodes.h\"\n#include \"nodes/primnodes.h\"\n#include \"nodes/value.h\"\n#include \"nodes/pg_list.h\"\n#include \"nodes/bitmapset.h\"\n#include \"pg_query.h\"\n#include \"pg_query_internal.h\"\n";
    let mut builder = bindgen::Builder::default()
        .header_contents("raw.h", header)
        .clang_arg(format!("-I{}", lib.join("src/postgres/include").display()))
        .clang_arg(format!("-I{}", lib.join("src/include").display()))
        .clang_arg(format!("-I{}", lib.join("src").display()))
        .clang_arg(format!("-I{}", lib.display()))
        .default_enum_style(bindgen::EnumVariation::Consts)
        .layout_tests(false)
        .derive_debug(false)
        .generate_comments(false)
        .allowlist_type("Node|NodeTag|List|ListCell|Bitmapset|bitmapword|PgQueryInternalParsetreeAndError|PgQueryError|PgQueryDeparseResult|MemoryContext")
        .allowlist_var("PG_VERSION_NUM|PG_QUERY_PROTOBUF_MAX_DEPTH");
    for c in defs.c_types() {
        builder = builder.allowlist_type(c);
    }
    for e in defs.enums.keys() {
        builder = builder.allowlist_type(e);
    }
    if target.starts_with("wasm32") {
        // The C build's own flags (`engine/build.sh`): WASI's sysroot, the stubs for the headers
        // WASI lacks and the defines that stand in for what it lacks.
        let flags = env::var(format!("CFLAGS_{}", target.replace('-', "_"))).unwrap_or_default();
        for w in flags.split_whitespace() {
            builder = builder.clang_arg(w);
        }
        builder = builder.clang_arg(format!("--target={target}")).clang_arg(format!("-I{}", root.join("vendor/stubs").display()));
    }
    builder.generate().expect("the bindings to libpg_query's nodes").write_to_file(path).unwrap();
}

/// One `WRITE_*` line of a converter: the macro's kind, and its arguments.
struct Field {
    kind: String,
    args: Vec<String>,
}

/// A generated `_out` function: the proto message it writes, the C struct it reads, its fields.
struct OutFn {
    message: String,
    c_type: String,
    fields: Vec<Field>,
}

/// A generated `_read` function: the proto message it reads, the C struct it makes, its fields.
struct ReadFn {
    message: String,
    c_type: String,
    fields: Vec<Field>,
}

struct Defs {
    /// By the function's name, `_out<name>`.
    outs: BTreeMap<String, OutFn>,
    /// By the function's name, `_read<name>`.
    reads: BTreeMap<String, ReadFn>,
    /// `READ_COND(message, function, …)`.
    read_conds: Vec<(String, String)>,
    /// Each enum's proto numbers and the C constant each reads as, and the constant for any other.
    int_enums: BTreeMap<String, (Vec<(i64, String)>, String)>,
    /// `case T_<tag>: OUT_NODE(message, function, …, c_type, …)`.
    conds: Vec<(String, String, String, String)>,
    /// Each enum's C constants and the proto number each writes as.
    enums: BTreeMap<String, Vec<(String, i64)>>,
    /// The Rust struct of each prost message, by its name with no underscores, lowercased; and
    /// each struct's fields, and the type of each.
    rust_structs: HashMap<String, String>,
    fields: HashMap<String, HashMap<String, (String, String)>>,
    /// The `Node` oneof's variants: the Rust struct each holds, and whether boxed.
    variants: HashMap<String, (String, bool)>,
}

/// The functions libpg_query writes by hand in `pg_query_outfuncs_protobuf.c`, which
/// `src/raw.rs` writes by hand too.
const HAND: &[&str] = &["List", "IntList", "OidList", "Integer", "Float", "Boolean", "String", "BitString", "AConst"];

impl Defs {
    fn read(lib: &Path, protobuf: &Path) -> Defs {
        let defs = fs::read_to_string(lib.join("src/include/pg_query_outfuncs_defs.c")).unwrap();
        let conds = fs::read_to_string(lib.join("src/include/pg_query_outfuncs_conds.c")).unwrap();
        let read_defs = fs::read_to_string(lib.join("src/include/pg_query_readfuncs_defs.c")).unwrap();
        let read_conds_c = fs::read_to_string(lib.join("src/include/pg_query_readfuncs_conds.c")).unwrap();
        let enums = fs::read_to_string(lib.join("src/include/pg_query_enum_defs.c")).unwrap();
        let pb = fs::read_to_string(protobuf).unwrap();
        for p in [lib.join("src/include/pg_query_outfuncs_defs.c"), lib.join("src/include/pg_query_outfuncs_conds.c"), lib.join("src/include/pg_query_readfuncs_defs.c"), lib.join("src/include/pg_query_readfuncs_conds.c"), lib.join("src/include/pg_query_enum_defs.c"), protobuf.to_path_buf()] {
            println!("cargo:rerun-if-changed={}", p.display());
        }

        let mut outs = BTreeMap::new();
        let mut rest = defs.as_str();
        while let Some(at) = rest.find("\n_out") {
            rest = &rest[at + 5..];
            let name = &rest[..rest.find('(').unwrap()];
            let sig_end = rest.find(")\n{").unwrap();
            let sig = &rest[..sig_end];
            let ot = &sig[sig.find("OUT_TYPE(").unwrap() + 9..];
            let mut parts = ot[..ot.find(')').unwrap()].split(", ");
            let message = parts.next().unwrap().to_string();
            let c_type = sig[sig.find("const ").unwrap() + 6..sig.find(" *node").unwrap()].to_string();
            let body = &rest[sig_end + 3..rest.find("\n}").unwrap()];
            let fields = body
                .lines()
                .filter_map(|l| {
                    let l = l.trim();
                    let l = l.strip_prefix("WRITE_")?;
                    let open = l.find('(')?;
                    let kind = l[..open].trim_end_matches("_FIELD").to_string();
                    let args = l[open + 1..l.rfind(')')?].split(',').map(|a| a.trim().to_string()).collect();
                    Some(Field { kind, args })
                })
                .collect();
            outs.insert(name.to_string(), OutFn { message, c_type, fields });
        }

        let mut cases = Vec::new();
        let mut tag = None;
        for line in conds.lines().map(str::trim) {
            if let Some(t) = line.strip_prefix("case T_") {
                tag = Some(t.trim_end_matches(':').to_string());
            } else if let Some(args) = line.strip_prefix("OUT_NODE(") {
                let a: Vec<String> = args.trim_end_matches(");").split(',').map(|s| s.trim().to_string()).collect();
                cases.push((tag.take().unwrap(), a[0].clone(), a[1].clone(), a[4].clone()));
            }
        }

        let mut reads = BTreeMap::new();
        let mut rest = read_defs.as_str();
        while let Some(at) = rest.find("\n_read") {
            rest = &rest[at + 6..];
            let name = &rest[..rest.find('(').unwrap()];
            let sig_end = rest.find(")\n{").unwrap();
            let sig = &rest[..sig_end];
            let ot = &sig[sig.find("OUT_TYPE(").unwrap() + 9..];
            let message = ot[..ot.find(',').unwrap()].to_string();
            let body = &rest[sig_end + 3..rest.find("\n}").unwrap()];
            let make = &body[body.find("makeNode(").unwrap() + 9..];
            let c_type = make[..make.find(')').unwrap()].to_string();
            let fields = body
                .lines()
                .filter_map(|l| {
                    // A struct that embeds another as its first member copies that one's tag over
                    // its own, and sets its own again after.
                    if let Some(tag) = l.trim().strip_prefix("NodeSetTag(node, T_") {
                        return Some(Field { kind: "SET_TAG".to_string(), args: vec![tag.trim_end_matches(");").to_string()] });
                    }
                    let l = l.trim().strip_prefix("READ_")?;
                    let open = l.find('(')?;
                    let kind = l[..open].trim_end_matches("_FIELD").to_string();
                    let args = l[open + 1..l.rfind(')')?].split(',').map(|a| a.trim().to_string()).collect();
                    Some(Field { kind, args })
                })
                .collect();
            reads.insert(name.to_string(), ReadFn { message, c_type, fields });
        }
        let read_conds = read_conds_c
            .lines()
            .filter_map(|l| {
                let a: Vec<String> = l.trim().strip_prefix("READ_COND(")?.trim_end_matches(");").split(',').map(|s| s.trim().to_string()).collect();
                Some((a[0].clone(), a[1].clone()))
            })
            .collect();
        let mut int_enums = BTreeMap::new();
        let mut rest = enums.as_str();
        while let Some(at) = rest.find("\n_intToEnum") {
            rest = &rest[at + "\n_intToEnum".len()..];
            let name = rest[..rest.find('(').unwrap()].to_string();
            let body = &rest[..rest.find("\n}").unwrap()];
            let values = body
                .lines()
                .filter_map(|l| {
                    let (n, c) = l.trim().strip_prefix("case ")?.split_once(": return ")?;
                    Some((n.parse().ok()?, c.trim_end_matches(';').to_string()))
                })
                .collect();
            let default = body.lines().rev().find_map(|l| l.trim().strip_prefix("return ")).unwrap().trim_end_matches(';').to_string();
            int_enums.insert(name, (values, default));
        }

        let mut enum_map = BTreeMap::new();
        let mut rest = enums.as_str();
        while let Some(at) = rest.find("\n_enumToInt") {
            rest = &rest[at + 11..];
            let name = rest[..rest.find('(').unwrap()].to_string();
            let body = &rest[..rest.find("\n}").unwrap()];
            let values = body
                .lines()
                .filter_map(|l| {
                    let l = l.trim().strip_prefix("case ")?;
                    let (c, v) = l.split_once(": return ")?;
                    Some((c.to_string(), v.trim_end_matches(';').parse().ok()?))
                })
                .collect();
            enum_map.insert(name, values);
        }

        // prost's structs: `pub struct Name { … }` with `pub field: Type,` lines.
        let mut rust_structs = HashMap::new();
        let mut fields = HashMap::new();
        let mut rest = pb.as_str();
        while let Some(at) = rest.find("pub struct ") {
            rest = &rest[at + 11..];
            let name = rest[..rest.find(' ').unwrap()].to_string();
            let body = &rest[..rest.find("\n}").unwrap()];
            // A field is a `pub name: Type,` line and the lines rustfmt wraps a long type onto.
            let mut fs_ = HashMap::new();
            let mut current: Option<(String, String)> = None;
            let mut flush = |current: &mut Option<(String, String)>| {
                if let Some((n, t)) = current.take() {
                    fs_.insert(n.trim_start_matches("r#").to_string(), (n, t.trim_end_matches(',').to_string()));
                }
            };
            for l in body.lines().map(str::trim) {
                if let Some(f) = l.strip_prefix("pub ") {
                    flush(&mut current);
                    if let Some((n, t)) = f.split_once(": ") {
                        current = Some((n.to_string(), t.to_string()));
                    }
                } else if l.starts_with("#[") || l.starts_with("///") || l.is_empty() {
                    flush(&mut current);
                } else if let Some((_, t)) = current.as_mut() {
                    t.push_str(l);
                }
            }
            flush(&mut current);
            rust_structs.insert(name.replace('_', "").to_lowercase(), name.clone());
            fields.insert(name, fs_);
        }
        let mut variants = HashMap::new();
        let node_enum = &pb[pb.find("pub enum Node {").unwrap()..];
        let node_enum = &node_enum[node_enum.find('{').unwrap() + 1..node_enum.find("\n    }").unwrap()];
        // Without its attributes and whitespace (rustfmt wraps a long variant): `Name(Type),…`.
        let mut flat = String::new();
        let mut chars = node_enum.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '#' && chars.peek() == Some(&'[') {
                let mut depth = 0;
                for c in chars.by_ref() {
                    match c {
                        '[' => depth += 1,
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            } else if !c.is_whitespace() {
                flat.push(c);
            }
        }
        for item in flat.split("),").filter(|i| !i.is_empty()) {
            let open = item.find('(').unwrap_or_else(|| panic!("a Node variant without a payload: {item}"));
            let variant = item[..open].to_string();
            let payload = item[open + 1..].trim_end_matches(')').trim_end_matches(',');
            let boxed = payload.contains("Box<");
            let ty = payload.rsplit("super::").next().unwrap().trim_end_matches('>').to_string();
            variants.insert(ty, (variant, boxed));
        }
        Defs { outs, reads, read_conds, int_enums, conds: cases, enums: enum_map, rust_structs, fields, variants }
    }

    fn c_types(&self) -> Vec<String> {
        let mut v: Vec<String> = self.outs.values().map(|o| o.c_type.clone()).chain(self.conds.iter().map(|c| c.3.clone())).chain(self.reads.values().map(|r| r.c_type.clone())).collect();
        v.sort();
        v.dedup();
        v
    }

    fn rust(&self, message: &str) -> &str {
        self.rust_structs.get(&message.replace('_', "").to_lowercase()).unwrap_or_else(|| panic!("no prost struct for {message}"))
    }

    fn converter(&self) -> String {
        let mut s = String::from("// Generated by engine/postgres/build.rs from libpg_query's outfuncs, conds and enum defs.\n\n");
        for (name, values) in &self.enums {
            writeln!(s, "#[allow(non_snake_case, unreachable_patterns, dead_code)]\nfn enum_{name}(v: b::{name}) -> i32 {{\n    match v {{").unwrap();
            for (c, n) in values {
                writeln!(s, "        b::{name}_{c} => {n},").unwrap();
            }
            writeln!(s, "        _ => -1,\n    }}\n}}\n").unwrap();
        }
        for (name, f) in &self.outs {
            if HAND.contains(&name.as_str()) {
                continue;
            }
            let rust = self.rust(&f.message);
            let fields = &self.fields[rust];
            writeln!(s, "#[allow(non_snake_case, unused_variables, clippy::needless_update)]\nunsafe fn out_{name}(n: &b::{}, d: &mut Depth) -> Result<pb::{rust}, Deep> {{\n    d.enter()?;\n    let message = pb::{rust} {{", f.c_type).unwrap();
            for field in &f.fields {
                let (outname, fld) = match field.kind.as_str() {
                    "ENUM" => (&field.args[2], &field.args[4]),
                    "SPECIFIC_NODE" | "SPECIFIC_NODE_PTR" => (&field.args[3], &field.args[5]),
                    _ => (&field.args[1], &field.args[3]),
                };
                let (rname, rtype) = fields.get(outname.as_str()).unwrap_or_else(|| panic!("{rust} has no field {outname}"));
                let c = format!("n.{}", c_field(fld));
                let boxed = rtype.contains("Box<");
                let value = match field.kind.as_str() {
                    "INT" | "UINT" | "UINT64" | "LONG" | "FLOAT" => format!("{c} as {}", rtype),
                    "BOOL" => c,
                    "CHAR" => format!("chr({c})"),
                    "STRING" => format!("string({c})"),
                    "ENUM" => format!("enum_{}({c})", field.args[1]),
                    "LIST" => format!("list({c}, d)?"),
                    "BITMAPSET" => format!("bitmapset({c})"),
                    "NODE_PTR" => {
                        let wrapped = format!("pb::Node {{ node: node({c} as *const b::Node, d)? }}");
                        format!("if {c}.is_null() {{ None }} else {{ Some({}) }}", if boxed { format!("Box::new({wrapped})") } else { wrapped })
                    }
                    "SPECIFIC_NODE_PTR" => {
                        let call = format!("out_{}(&*{c}, d)?", field.args[1]);
                        format!("if {c}.is_null() {{ None }} else {{ Some({}) }}", if boxed { format!("Box::new({call})") } else { call })
                    }
                    "SPECIFIC_NODE" => {
                        let call = format!("out_{}(&{c}, d)?", field.args[1]);
                        format!("Some({})", if boxed { format!("Box::new({call})") } else { call })
                    }
                    k => panic!("WRITE_{k}_FIELD is new: teach engine/postgres/build.rs to read it"),
                };
                writeln!(s, "        {rname}: {value},").unwrap();
            }
            writeln!(s, "        ..Default::default()\n    }};\n    d.leave();\n    Ok(message)\n}}\n").unwrap();
        }
        writeln!(s, "/// One node, by its tag, as `_outNodeAt` writes it; a tag it does not know is no node.\nunsafe fn node_at(p: *const b::Node, d: &mut Depth) -> Result<Option<pb::node::Node>, Deep> {{\n    if p.is_null() {{\n        return Ok(None);\n    }}\n    Ok(Some(match (*p).type_ {{").unwrap();
        for (tag, message, function, c_type) in &self.conds {
            let rust = self.rust(message);
            let (variant, boxed) = self.variants.get(rust).unwrap_or_else(|| panic!("no Node variant holds {rust}"));
            let call = format!("out_{function}(&*(p as *const b::{c_type}), d)?");
            writeln!(s, "        b::NodeTag_T_{tag} => pb::node::Node::{variant}({}),", if *boxed { format!("Box::new({call})") } else { call }).unwrap();
        }
        writeln!(s, "        _ => return Ok(None),\n    }}))\n}}").unwrap();
        s
    }
}

impl Defs {
    /// From the engine's tree back to libpg_query's nodes, as its protobuf reader makes them
    /// (`pg_query_readfuncs_defs.c`, `pg_query_readfuncs_conds.c`), for its deparser.
    fn builder(&self) -> String {
        let mut s = String::from("// Generated by engine/postgres/build.rs from libpg_query's readfuncs and enum defs.\n\n");
        for (name, (values, default)) in &self.int_enums {
            writeln!(s, "#[allow(non_snake_case, dead_code)]\nfn int_to_enum_{name}(v: i32) -> b::{name} {{\n    match v {{").unwrap();
            for (n, c) in values {
                writeln!(s, "        {n} => b::{name}_{c},").unwrap();
            }
            writeln!(s, "        _ => b::{name}_{default},\n    }}\n}}\n").unwrap();
        }
        for (name, f) in &self.reads {
            if HAND.contains(&name.as_str()) || name == "A_Const" {
                continue;
            }
            let rust = self.rust(&f.message);
            let fields = &self.fields[rust];
            writeln!(s, "#[allow(non_snake_case, unused_variables, unused_mut)]\nunsafe fn in_{name}(m: &pb::{rust}, a: &mut Arena) -> *mut b::{c} {{\n    let node = a.make::<b::{c}>(b::NodeTag_T_{c});\n    let n = &mut *node;", c = f.c_type).unwrap();
            for field in &f.fields {
                if field.kind == "SET_TAG" {
                    writeln!(s, "    *(node as *mut b::NodeTag) = b::NodeTag_T_{};", field.args[0]).unwrap();
                    continue;
                }
                let (outname, fld) = match field.kind.as_str() {
                    "ENUM" => (&field.args[2], &field.args[4]),
                    "SPECIFIC_NODE" | "SPECIFIC_NODE_PTR" => (&field.args[3], &field.args[5]),
                    _ => (&field.args[1], &field.args[3]),
                };
                let (rname, _) = fields.get(outname.as_str()).unwrap_or_else(|| panic!("{rust} has no field {outname}"));
                let c = format!("n.{}", c_field(fld));
                let m = format!("m.{rname}");
                let line = match field.kind.as_str() {
                    "INT" | "UINT" | "UINT64" | "LONG" | "FLOAT" => format!("{c} = {m} as _;"),
                    "BOOL" => format!("{c} = {m};"),
                    "CHAR" => format!("if let Some(&ch) = {m}.as_bytes().first() {{ {c} = ch as _; }}"),
                    "STRING" => format!("if !{m}.is_empty() {{ {c} = a.string(&{m}); }}"),
                    "ENUM" => format!("{c} = int_to_enum_{}({m});", field.args[1]),
                    "LIST" => format!("let items: Vec<*mut b::Node> = {m}.iter().map(|x| in_node(x, a)).collect();\n    {c} = a.list(items);"),
                    "BITMAPSET" => "// READ_BITMAPSET_FIELD reads nothing in libpg_query either.".to_string(),
                    "NODE_PTR" | "ABSTRACT_PTR" | "VALUE_PTR" => format!("if let Some(x) = &{m} {{ {c} = in_node(x, a) as _; }}"),
                    "SPECIFIC_NODE" => format!("if let Some(x) = &{m} {{ {c} = *in_{}(x, a); }}", field.args[1]),
                    "SPECIFIC_NODE_PTR" => format!("if let Some(x) = &{m} {{ {c} = in_{}(x, a); }}", field.args[1]),
                    k => panic!("READ_{k}_FIELD is new: teach engine/postgres/build.rs to read it"),
                };
                writeln!(s, "    {line}").unwrap();
            }
            writeln!(s, "    node\n}}\n").unwrap();
        }
        writeln!(s, "/// One node, by its variant, as `_readNode` makes it.\n#[allow(unreachable_patterns)]\nunsafe fn in_node(m: &pb::Node, a: &mut Arena) -> *mut b::Node {{\n    use pb::node::Node as N;\n    match &m.node {{\n        None => std::ptr::null_mut(),").unwrap();
        for (message, function) in &self.read_conds {
            let rust = self.rust(message);
            let (variant, _) = self.variants.get(rust).unwrap_or_else(|| panic!("no Node variant holds {rust}"));
            writeln!(s, "        Some(N::{variant}(x)) => in_{function}(x, a) as *mut b::Node,").unwrap();
        }
        writeln!(s, "        Some(N::Integer(x)) => in_Integer(x, a) as *mut b::Node,\n        Some(N::Float(x)) => in_Float(x, a) as *mut b::Node,\n        Some(N::Boolean(x)) => in_Boolean(x, a) as *mut b::Node,\n        Some(N::String(x)) => in_String(x, a) as *mut b::Node,\n        Some(N::BitString(x)) => in_BitString(x, a) as *mut b::Node,\n        Some(N::AConst(x)) => in_AConst(x, a) as *mut b::Node,\n        Some(N::List(x)) => in_List(x, a) as *mut b::Node,\n        Some(_) => {{\n            a.unsupported = true;\n            std::ptr::null_mut()\n        }}\n    }}\n}}").unwrap();
        s
    }
}

/// The Rust name bindgen gives a C field: a Rust keyword takes a trailing underscore.
fn c_field(name: &str) -> String {
    const KEYWORDS: &[&str] = &["as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try"];
    name.split('.').map(|p| if KEYWORDS.contains(&p) { format!("{p}_") } else { p.to_string() }).collect::<Vec<_>>().join(".")
}
