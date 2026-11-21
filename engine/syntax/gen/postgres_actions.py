#!/usr/bin/env python3
"""Translates `gram.y`'s C actions to Rust, over the dynamic nodes of `src/postgres/nodes.rs`.

The actions build Postgres's parse tree through pointers, so the Rust keeps that shape: a node is
a shared cell, a field is read and written by name, and every value is one `V`. Called by
`postgres.py`, which hands it the patched grammar and the source trees it reads constants from.
"""
import re


class Unsupported(Exception):
    pass


# ---------------------------------------------------------------- the grammar's rules


def parse_rules(source):
    """Each alternative of the rules section, in Bison's numbering order: (lhs, symbols, action)."""
    _, text, *_ = re.split(r"^%%\s*$", source, flags=re.M)
    i, n, rules = 0, len(text), []
    lhs, symbols, action, prec_next = None, [], None, False

    def skip_block(j):
        depth = 0
        while True:
            c = text[j]
            if text.startswith("/*", j):
                j = text.index("*/", j + 2) + 2
                continue
            if c in "\"'":
                k = j + 1
                while text[k] != c:
                    k += 2 if text[k] == "\\" else 1
                j = k + 1
                continue
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    return j + 1
            j += 1

    def flush():
        nonlocal symbols, action
        rules.append((lhs, symbols, action))
        symbols, action = [], None

    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
        elif text.startswith("/*", i):
            i = text.index("*/", i + 2) + 2
        elif c == "'":
            j = text.index("'", i + 1)
            if action is not None:
                raise Unsupported("a mid-rule action in " + str(lhs))
            if prec_next:
                prec_next = False
            else:
                symbols.append(text[i : j + 1])
            i = j + 1
        elif c == "{":
            j = skip_block(i)
            if action is not None:
                raise Unsupported("two actions in " + str(lhs))
            action = text[i + 1 : j - 1]
            i = j
        elif c == "|":
            flush()
            i += 1
        elif c == ";":
            flush()
            lhs = None
            i += 1
        elif text.startswith("%prec", i):
            prec_next = True
            i += 5
        elif text.startswith("%empty", i):
            i += 6
        else:
            m = re.compile(r"[A-Za-z_][\w.]*").match(text, i)
            if not m:
                raise Unsupported("rules: " + text[i : i + 30])
            word, i = m.group(0), m.end()
            j = i
            while j < n and text[j].isspace():
                j += 1
            if lhs is None or (j < n and text[j] == ":" and not prec_next):
                if lhs is not None:
                    raise Unsupported("rule without ';' before " + word)
                lhs = word
                i = j + 1
            elif prec_next:
                prec_next = False
            else:
                if action is not None:
                    raise Unsupported("a mid-rule action in " + str(lhs))
                symbols.append(word)
    return rules


# ---------------------------------------------------------------- C, tokenized

TOKEN = re.compile(
    r"""
    (?P<space>\s+|/\*.*?\*/|//[^\n]*)
  | (?P<dollar>\$\$|\$\d+)
  | (?P<at>@\$|@\d+)
  | (?P<num>0[xX][0-9a-fA-F]+[uUlL]*|\d+[uUlL]*)
  | (?P<id>[A-Za-z_]\w*)
  | (?P<str>"(?:[^"\\]|\\.)*")
  | (?P<chr>'(?:[^'\\]|\\.)+')
  | (?P<op><<=|>>=|->|\+\+|--|<<|>>|<=|>=|==|!=|&&|\|\||\+=|-=|\*=|/=|\|=|&=|\^=|[-+*/%&|^~!<>=?:;,.(){}\[\]])
    """,
    re.X | re.S,
)


def tokenize(code):
    out, i = [], 0
    while i < len(code):
        m = TOKEN.match(code, i)
        if not m:
            raise Unsupported("cannot read C at: " + code[i : i + 40])
        if m.lastgroup != "space":
            out.append((m.lastgroup, m.group(0)))
        i = m.end()
    return out


def c_string(literal):
    body, out, i = literal[1:-1], [], 0
    simple = {"n": "\n", "t": "\t", "\\": "\\", '"': '"', "'": "'", "r": "\r", "0": "\0"}
    while i < len(body):
        if body[i] == "\\":
            out.append(simple[body[i + 1]])
            i += 2
        else:
            out.append(body[i])
            i += 1
    return "".join(out)


def rust_string(value):
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t").replace("\r", "\\r").replace("\0", "\\0") + '"'


TYPE_WORDS = {"int", "char", "bool", "const", "unsigned", "signed", "long", "short", "struct", "enum", "union", "void", "float", "double", "static", "volatile"}
KEYWORDS = {"if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "return", "foreach", "for_each_from", "for_each_cell", "goto", "sizeof"}
BINARY = {
    "||": 1, "&&": 2, "|": 3, "^": 4, "&": 5, "==": 6, "!=": 6, "<": 7, ">": 7, "<=": 7, ">=": 7, "<<": 8, ">>": 8, "+": 9, "-": 9, "*": 10, "/": 10, "%": 10,
}
ASSIGN = {"=", "|=", "&=", "+=", "-=", "*=", "/=", "^=", "<<=", ">>="}


# The structs a parse node holds inside itself, by the field that holds them.
EMBEDDED = {"base": "CreateStmt"}


class Parser:
    """C statements and expressions as tuples."""

    def __init__(self, tokens):
        self.t, self.i = tokens, 0

    def peek(self, k=0):
        return self.t[self.i + k] if self.i + k < len(self.t) else ("eof", "")

    def text(self, k=0):
        return self.peek(k)[1]

    def next(self):
        tok = self.peek()
        self.i += 1
        return tok

    def eat(self, s):
        if self.text() == s:
            self.i += 1
            return True
        return False

    def expect(self, s):
        if not self.eat(s):
            raise Unsupported(f"expected {s!r}, found {self.text()!r} near {' '.join(x[1] for x in self.t[max(0, self.i - 6) : self.i + 4])}")

    def statements(self):
        out = []
        while self.peek()[0] != "eof" and self.text() != "}":
            out.append(self.statement())
        return out

    def is_type_start(self):
        """Whether a declaration starts here: a type, `*`s, and a name."""
        j = 0
        if self.peek(j)[0] != "id":
            return False
        word = self.text(j)
        if word in KEYWORDS:
            return False
        if word in TYPE_WORDS:
            return True
        j += 1
        while self.text(j) == "*":
            j += 1
        return self.peek(j)[0] == "id" and self.text(j) not in KEYWORDS and self.text(j + 1) in ("=", ";", ",", "[")

    def declaration(self):
        while self.peek()[0] == "id" and (self.text() in TYPE_WORDS):
            self.next()
            if self.peek()[0] == "id" and self.text(1) in ("*",) or (self.peek()[0] == "id" and self.peek(1)[0] == "id"):
                if self.text() not in TYPE_WORDS:
                    self.next()
                    break
        else:
            if self.peek()[0] == "id" and (self.text(1) == "*" or self.peek(1)[0] == "id"):
                self.next()
        out = []
        while True:
            while self.eat("*"):
                pass
            name = self.next()[1]
            init = None
            if self.eat("="):
                init = self.expression(assign=False)
            out.append(("decl", name, init))
            if not self.eat(","):
                break
        self.expect(";")
        return ("block_flat", out)

    def statement(self):
        word = self.text()
        if word == "{":
            self.next()
            body = self.statements()
            self.expect("}")
            return ("block", body)
        if word == ";":
            self.next()
            return ("block", [])
        if word == "if":
            self.next()
            self.expect("(")
            cond = self.expression()
            while self.eat(","):  # the comma operator: the last operand is the value
                cond = self.expression()
            self.expect(")")
            then = self.statement()
            other = self.statement() if self.eat("else") else None
            return ("if", cond, then, other)
        if word in ("foreach", "for_each_from", "for_each_cell"):
            self.next()
            self.expect("(")
            cell = self.next()[1]
            self.expect(",")
            items = self.expression()
            start = None
            if word != "foreach":
                self.expect(",")
                start = self.expression()
                if word == "for_each_cell":
                    start = ("cellindex", start)
            self.expect(")")
            return ("foreach", cell, items, self.statement(), start)
        if word == "return":
            self.next()
            value = None if self.text() == ";" else self.expression()
            self.expect(";")
            return ("return", value)
        if word in ("break", "continue"):
            self.next()
            self.expect(";")
            return (word,)
        if word == "for":
            self.next()
            self.expect("(")
            init = None
            if not self.eat(";"):
                if self.is_type_start():
                    init = self.declaration()
                else:
                    init = ("expr", self.expression())
                    self.expect(";")
            cond = None if self.text() == ";" else self.expression()
            self.expect(";")
            step = None if self.text() == ")" else self.expression()
            self.expect(")")
            return ("for", init, cond, step, self.statement())
        if word == "while":
            self.next()
            self.expect("(")
            cond = self.expression()
            self.expect(")")
            return ("for", None, cond, None, self.statement())
        if word == "switch":
            self.next()
            self.expect("(")
            subject = self.expression()
            self.expect(")")
            self.expect("{")
            cases = []
            while not self.eat("}"):
                labels = []
                while self.text() in ("case", "default"):
                    if self.next()[1] == "default":
                        labels.append(None)
                    else:
                        labels.append(self.ternary())
                    self.expect(":")
                body = []
                while self.text() not in ("case", "default", "}"):
                    body.append(self.statement())
                cases.append((labels, body))
            return ("switch", subject, cases)
        if word in ("do", "goto"):
            raise Unsupported("C statement: " + word)
        if self.is_type_start():
            return self.declaration()
        value = self.expression()
        self.expect(";")
        return ("expr", value)

    def expression(self, assign=True):
        left = self.ternary()
        if assign and self.text() in ASSIGN:
            op = self.next()[1]
            return ("assign", op, left, self.expression())
        if not assign and self.text() in ASSIGN:
            op = self.next()[1]
            return ("assign", op, left, self.expression(assign=False))
        return left

    def ternary(self):
        cond = self.binary(1)
        if self.eat("?"):
            yes = self.expression()
            self.expect(":")
            return ("cond", cond, yes, self.ternary())
        return cond

    def binary(self, level):
        left = self.unary()
        while self.text() in BINARY and BINARY[self.text()] >= level:
            op = self.next()[1]
            left = ("bin", op, left, self.binary(BINARY[op] + 1))
        return left

    def is_cast(self):
        """`(` a type name `*`s `)` before an operand."""
        if self.text() != "(" or self.peek(1)[0] != "id":
            return False
        j = 1
        while self.text(j) in TYPE_WORDS:
            j += 1
        if self.peek(j)[0] != "id" and j == 1:
            return False
        if self.peek(j)[0] == "id":
            if self.text(j) in KEYWORDS:
                return False
            j += 1
        stars = 0
        while self.text(j) == "*":
            stars += 1
            j += 1
        if self.text(j) != ")":
            return False
        if stars == 0 and self.text(1) not in TYPE_WORDS and not re.fullmatch(r"(int32|int64|uint32|uint8|int16|Oid|ParseLoc|Node|List|[A-Z]\w+)", self.text(1)):
            return False
        after = self.peek(j + 1)
        return after[0] in ("id", "num", "str", "chr", "dollar", "at") or after[1] in ("(", "-", "!", "&", "*", "~")

    def unary(self):
        word = self.text()
        if self.is_cast():
            while self.text() != ")":
                self.next()
            self.next()
            return self.unary()
        if word in ("!", "-", "~", "&", "*", "+"):
            self.next()
            return ("un", word, self.unary())
        if word == "sizeof":
            self.next()
            self.expect("(")
            name = []
            while self.text() != ")":
                name.append(self.next()[1])
            self.next()
            return ("sizeof", name[-1] if name[-1] != "*" else name[0])
        return self.postfix(self.primary())

    def primary(self):
        kind, text = self.next()
        if text == "(":
            value = self.expression()
            if self.text() == ",":
                items = [value]
                while self.eat(","):
                    items.append(self.expression())
                value = ("tuple", items)
            self.expect(")")
            return value
        if kind == "num":
            return ("num", int(re.sub(r"[uUlL]+$", "", text), 0))
        if kind == "str":
            value = c_string(text)
            while self.peek()[0] == "str":
                value += c_string(self.next()[1])
            return ("str", value)
        if kind == "chr":
            return ("num", ord(c_string(text)))
        if kind == "dollar":
            return ("dollar", 0 if text == "$$" else int(text[1:]))
        if kind == "at":
            return ("at", 0 if text == "@$" else int(text[1:]))
        if kind == "id":
            return ("id", text)
        raise Unsupported(f"expression at {text!r}")

    def postfix(self, value):
        while True:
            if self.eat("("):
                args = []
                if self.text() != ")":
                    while True:
                        args.append(self.type_or_expression())
                        if not self.eat(","):
                            break
                self.expect(")")
                if value[0] != "id":
                    raise Unsupported("a call through a pointer")
                value = ("call", value[1], args)
            elif self.text() in ("->", "."):
                dot = self.next()[1] == "."
                field = self.next()[1]
                if dot and value[0] == "get" and value[2] == "val" and field in ("sval", "ival", "fval", "boolval", "bsval", "node"):
                    # `A_Const.val`, a union of the value nodes: the node it holds.
                    value = ("union", value[1])
                    continue
                if dot and value[0] == "get" and value[2] == field and field in ("sval", "ival", "fval", "boolval", "bsval"):
                    # `v->sval.sval` on a `union ValUnion *`: the value node's own field.
                    continue
                if dot and value[0] == "union":
                    value = ("tagof", value) if field == "type" else ("get", value, field)
                    continue
                if dot:
                    if value[0] != "get" or value[2] not in EMBEDDED:
                        raise Unsupported("a member of an embedded struct: " + field)
                    value = ("embedded", value[1], value[2])
                value = ("get", value, field)
            elif self.text() in ("++", "--"):
                op = self.next()[1]
                value = ("assign", "+=" if op == "++" else "-=", value, ("num", 1))
            elif self.text() == "[":
                raise Unsupported("an index")
            else:
                return value

    def type_or_expression(self):
        """A macro argument, which may be a bare type name (`makeNode(SelectStmt)`, `castNode(T, x)`)."""
        if self.peek()[0] == "id" and self.text(1) in (",", ")"):
            return ("id", self.next()[1])
        return self.expression()


def collect(node, found):
    """Every identifier, field and callee an AST names."""
    if isinstance(node, (list, tuple)):
        if node and node[0] == "get" and len(node) == 3:
            found["fields"].add(node[2])
            collect(node[1], found)
            return
        if node and node[0] == "call":
            found["calls"].add(node[1])
            for a in node[2]:
                collect(a, found)
            return
        if node and node[0] == "id" and len(node) == 2:
            found["ids"].add(node[1])
            return
        if node and node[0] == "decl":
            found["decls"].add(node[1])
        for x in node:
            collect(x, found)


# ---------------------------------------------------------------- Rust

PASS_THROUGH = {"pstrdup", "castNode_", "linitial_node_"}
TYPE_ARGUMENT_FIRST = {"castNode", "lfirst_node", "linitial_node", "lsecond_node", "llast_node"}
ERROR_CALLS = {"ereport", "elog"}
DROPPED_CALLS = {"Assert", "StaticAssertStmt"}


class Emitter:
    def __init__(self, constant, tag, field, in_function=False, own=frozenset()):
        self.constant, self.tag, self.field = constant, tag, field
        self.scopes = [set()]
        self.targets = []
        self.labels = 0
        self.in_function = in_function
        self.own = own

    def label(self, prefix):
        self.labels += 1
        return f"'{prefix}{self.labels}"

    def is_local(self, name):
        return any(name in s for s in self.scopes)

    def expr(self, e):
        kind = e[0]
        if kind == "num":
            return f"V::Int({e[1]})"
        if kind == "str":
            return f"V::s({rust_string(e[1])})"
        if kind == "dollar":
            return "out.clone()" if e[1] == 0 else f"y[{e[1] - 1}].clone()"
        if kind == "at":
            return "V::Int(here as i64)" if e[1] == 0 else f"V::Int(l[{e[1] - 1}] as i64)"
        if kind == "id":
            name = e[1]
            if self.is_local(name):
                return f"v_{name}.clone()"
            if name in ("NULL", "NIL", "yyscanner", "yynerrs"):
                return "V::Null"
            if name == "true":
                return "V::Int(1)"
            if name == "false":
                return "V::Int(0)"
            if name == "DEFAULT_INDEX_TYPE":
                return 'V::s("btree")'
            if name.startswith("T_"):
                return f"V::Int(t::{name} as i64)"
            return f"V::Int({self.constant(name)})"
        if kind == "get":
            return f"{self.expr(e[1])}.get({self.field(e[2])})"
        if kind == "cellindex":
            return f"V::Int({self.expr(e[1])}.cell_index())"
        if kind == "union":
            return f"{self.expr(e[1])}.get({self.field('val')})"
        if kind == "tagof":
            return f"V::Int({self.expr(e[1])}.tag().map_or(0, |tag| tag as i64))"
        if kind == "embedded":
            return f"{self.expr(e[1])}.embedded({self.field(e[2])}, t::T_{EMBEDDED[e[2]]})"
        if kind == "cond":
            return f"(if {self.expr(e[1])}.truthy() {{ {self.expr(e[2])} }} else {{ {self.expr(e[3])} }})"
        if kind == "un":
            op, operand = e[1], e[2]
            if op == "!":
                return f"V::b(!{self.expr(operand)}.truthy())"
            if op == "-":
                return f"{self.expr(operand)}.neg()"
            if op == "&":
                if operand[0] == "get" and (operand[2] in EMBEDDED or operand[2] == "val"):
                    return self.expr(operand)  # a struct or union held inside the node
                if operand[0] != "get":
                    raise Unsupported("& of something other than a field")
                return f"V::place(&{self.expr(operand[1])}, {self.field(operand[2])})"
            if op == "*":
                return f"{self.expr(operand)}.deref()"
            if op == "+":
                return self.expr(operand)
            if op == "~":
                return f"{self.expr(operand)}.not()"
            raise Unsupported("unary " + op)
        if kind == "bin":
            op, a, b = e[1], e[2], e[3]
            if op == "&&":
                return f"V::b({self.expr(a)}.truthy() && {self.expr(b)}.truthy())"
            if op == "||":
                return f"V::b({self.expr(a)}.truthy() || {self.expr(b)}.truthy())"
            names = {"==": "eq", "!=": "ne", "<": "lt", ">": "gt", "<=": "le", ">=": "ge", "+": "add", "-": "sub", "*": "mul", "|": "or", "&": "and", "<<": "shl"}
            if op not in names:
                raise Unsupported("binary " + op)
            return f"{self.expr(a)}.{names[op]}(&{self.expr(b)})"
        if kind == "assign":
            raise Unsupported("an assignment inside an expression")
        if kind == "sizeof":
            return "V::Int(0)"
        if kind == "call":
            return self.call(e[1], e[2])
        raise Unsupported("expression " + kind)

    def call(self, name, args):
        if self.in_function:
            if name in self.own:
                rendered = ", ".join("&" + self.expr(a) for a in args)
                return f"{name}({rendered})?"
            if name == "castNode" or name == "lfirst_node":
                inner = self.expr(args[1])
                inner = f"h::lfirst(&{inner})?" if name == "lfirst_node" else inner
                return f"h::cast(t::T_{args[0][1]}, &{inner})?"
            if name in ("linitial_node", "lsecond_node", "lthird_node", "llast_node"):
                return f"h::cast(t::T_{args[0][1]}, &h::{name[: -len('_node')]}(&{self.expr(args[1])})?)?"
            if name == "IsA":
                return f"V::b({self.expr(args[0])}.is_a(t::T_{args[1][1]}))"
            if name == "nodeTag":
                return f"V::Int({self.expr(args[0])}.tag_number())"
            if name == "makeNode":
                return f"V::node(t::T_{args[0][1]})"
            if name == "deparseAppendStringInfo":
                return f"h::deparseAppendStringInfo(&[{', '.join(self.expr(a) for a in args)}])?"
            if name in ("pfree", "list_free", "check_stack_depth", "bms_free", "Assert"):
                return "V::Null"
            rendered = ", ".join("&" + self.expr(a) for a in args)
            return f"h::{name}({rendered})?"
        if name == "makeNode":
            return f"V::node(t::T_{args[0][1]})"
        if name in ("palloc", "palloc0") and args and args[0][0] == "sizeof":
            return f"V::node(t::T_{args[0][1]})"
        if name == "IsA":
            return f"V::b({self.expr(args[0])}.is_a(t::T_{args[1][1]}))"
        if name in TYPE_ARGUMENT_FIRST:
            inner = self.expr(args[1])
            return inner if name == "castNode" else f"h::{name[: -len('_node')]}(&{inner})"
        if name == "pstrdup":
            return self.expr(args[0])
        if name == "psprintf":
            return f"h::psprintf(&[{', '.join(self.expr(a) for a in args)}])?"
        if name in ("linitial", "lsecond", "llast", "lfirst"):
            return f"h::{name}(&{self.expr(args[0])})"
        rendered = ", ".join("&" + self.expr(a) for a in args)
        return f"h::{name}({rendered})?"

    def error(self, args):
        """An `ereport(ERROR, …)`: returns the message; another level writes nothing."""
        level = args[0]
        if level != ("id", "ERROR"):
            return ""
        message = None

        def find(node):
            nonlocal message
            if isinstance(node, tuple) and node and node[0] == "call":
                if node[1] in ("errmsg", "errmsg_internal"):
                    message = node[2]
                for a in node[2]:
                    find(a)
            elif isinstance(node, (list, tuple)):
                for x in node:
                    find(x)

        find(args[1:])
        if message is None:
            if len(args) > 1 and args[1][0] == "str":
                message = args[1:]
            else:
                return 'return Err("syntax error".to_string());'
        rendered = ", ".join(self.expr(a) for a in message[1:])
        return f"return Err(h::message({rust_string(message[0][1])}, &[{rendered}]));"

    def statement(self, s):
        kind = s[0]
        if kind == "block":
            self.scopes.append(set())
            body = "\n".join(self.statement(x) for x in s[1])
            self.scopes.pop()
            return "{\n" + body + "\n}"
        if kind == "block_flat":
            return "\n".join(self.statement(x) for x in s[1])
        if kind == "decl":
            init = self.expr(s[2]) if s[2] is not None else "V::Null"
            self.scopes[-1].add(s[1])
            return f"let mut v_{s[1]} = {init};"
        if kind == "if":
            out = f"if {self.expr(s[1])}.truthy() {self.braced(s[2])}"
            if s[3] is not None:
                out += f" else {self.braced(s[3])}"
            return out
        if kind == "foreach":
            self.scopes.append({s[1]})
            label = self.label("l")
            self.targets.append(("loop", label))
            body = self.braced(s[3])
            self.targets.pop()
            self.scopes.pop()
            from_index = f".skip({self.expr(s[4])}.int() as usize)" if len(s) > 4 and s[4] is not None else ""
            return f"{label}: for v_{s[1]} in {self.expr(s[2])}.cells().into_iter(){from_index} {body}"
        if kind == "return":
            if not self.in_function:
                raise Unsupported("return in an action")
            return "return Ok(V::Null);" if s[1] is None else f"return Ok({self.expr(s[1])});"
        if kind == "break":
            return f"break {self.targets[-1][1]};"
        if kind == "continue":
            for target_kind, label in reversed(self.targets):
                if target_kind == "loop":
                    return f"continue {label};"
                if target_kind == "for":
                    return f"break {label}_body;"
            raise Unsupported("continue outside a loop")
        if kind == "for":
            self.scopes.append(set())
            init = self.statement(s[1]) if s[1] is not None else ""
            label = self.label("l")
            self.targets.append(("for", label))
            body = self.braced(s[4])
            self.targets.pop()
            cond = f"if !{self.expr(s[2])}.truthy() {{ break; }}" if s[2] is not None else ""
            step = self.effect(s[3]) if s[3] is not None else ""
            self.scopes.pop()
            return f"{{\n{init}\n{label}: loop {{\n{cond}\n{label}_body: {body}\n{step}\n}}\n}}"
        if kind == "switch":
            subject = self.expr(s[1])
            label = self.label("s")
            self.targets.append(("switch", label))
            picks, bodies, default = [], [], None
            for index, (labels, body) in enumerate(s[2]):
                conditions = [f"on.eq(&{self.expr(l)}).truthy()" for l in labels if l is not None]
                if None in labels:
                    default = index
                if conditions:
                    picks.append(f"if {' || '.join(conditions)} {{ {index} }}")
                self.scopes.append(set())
                code = "\n".join(self.statement(x) for x in body)
                self.scopes.pop()
                bodies.append(f"if at <= {index} {{\n{code}\n}}")
            self.targets.pop()
            fallback = default if default is not None else len(s[2])
            chain = " else ".join(picks + [f"{{ {fallback} }}"]) if picks else str(fallback)
            return f"{label}: {{\nlet on = {subject};\nlet at: usize = {chain};\n" + "\n".join(bodies) + "\n}"
        if kind == "expr":
            return self.effect(s[1])
        raise Unsupported("statement " + kind)

    def braced(self, s):
        text = self.statement(s)
        return text if s[0] == "block" else "{\n" + text + "\n}"

    def effect(self, e):
        if e[0] == "assign":
            op, left, right = e[1], e[2], e[3]
            if right[0] == "assign":
                return self.effect(right) + "\n" + self.effect(("assign", op, left, right[2]))
            value = self.expr(right)
            if op != "=":
                names = {"|=": "or", "&=": "and", "+=": "add", "-=": "sub"}
                value = f"{self.expr(left)}.{names[op]}(&{value})"
            if left[0] == "dollar":
                return f"out = {value};" if left[1] == 0 else f"y[{left[1] - 1}] = {value};"
            if left[0] == "id":
                if not self.is_local(left[1]):
                    raise Unsupported("assignment to " + left[1])
                return f"v_{left[1]} = {value};"
            if left[0] == "get":
                return f"{self.expr(left[1])}.set({self.field(left[2])}, {value});"
            if left[0] == "call" and left[1] == "linitial":
                return f"h::set_{left[1]}(&{self.expr(left[2][0])}, {value});"
            if left[0] == "un" and left[1] == "*":
                return f"{self.expr(left[2])}.store({value});"
            raise Unsupported("assignment target " + left[0])
        if e[0] == "call":
            if self.in_function and e[1] == "elog":
                message = e[2][1] if len(e[2]) > 1 else ("str", "deparse error")
                rendered = ", ".join(self.expr(a) for a in e[2][2:])
                return f"return Err(h::message({rust_string(message[1])}, &[{rendered}]));"
            if e[1] in ERROR_CALLS:
                return self.error(e[2])
            if e[1] in DROPPED_CALLS:
                return ""
            if e[1] == "parser_yyerror":
                return f"return Err(h::syntax({self.expr(e[2][0])}, here));"
        return f"let _ = {self.expr(e)};"


# A value read through a reference needs no count taken on it. Taking one at each use is most of the generated code's size, once the count and its release are inlined.
_LOCAL = r"(v_\w+|y\[\d+\])"
_READS = "get|eq|ne|lt|le|gt|ge|add|sub|mul|or|and|shl|neg|not|truthy|is_a|tag_number|tag|int|cell_index|cells|items|len|is_empty|deref|store|set|embedded|copied"


def without_clones(code):
    code = re.sub(r"&" + _LOCAL + r"\.clone\(\)", r"&\1", code)
    return re.sub(r"\b" + _LOCAL + r"\.clone\(\)\.(" + _READS + r")\(", r"\1.\2(", code)


def translate(source, rules_check, constant):
    """The generated `actions.rs`, and what it names."""
    rules = parse_rules(source)
    rules_check(rules)
    parsed, found = [], {"fields": set(), "calls": set(), "ids": set(), "decls": set()}
    for index, (lhs, symbols, action) in enumerate(rules):
        if action is None:
            parsed.append(None)
            continue
        try:
            p = Parser(tokenize(action))
            body = p.statements()
            if p.peek()[0] != "eof":
                raise Unsupported("trailing C: " + p.text())
        except Unsupported as e:
            raise Unsupported(f"rule {index + 2} ({lhs}): {e}\n{action}") from e
        collect(body, found)
        parsed.append(body)
    tags, fields = set(), set()

    def tag_of(name):
        tags.add(name)
        return name

    def field_of(name):
        fields.add(name)
        return f"f::F_{name}"

    emitter = Emitter(constant, tag_of, field_of)
    out = [
        "// Generated by engine/syntax/gen/postgres.py from the C actions of Postgres's gram.y.",
        "#![allow(unused_mut, unused_variables, unused_assignments, unused_labels, unused_parens, unreachable_code, non_snake_case, clippy::all)]",
        "use super::generated_names::{f, t};",
        "use super::helpers as h;",
        "use super::nodes::{Action, V, R};",
        "",
    ]
    table = []
    # Rules whose actions read alike share one function.
    written = {}
    for index, body in enumerate(parsed):
        rule = index + 2
        if body is None:
            table.append("default_action")
            continue
        emitter.scopes = [set()]
        try:
            code = without_clones("\n".join(emitter.statement(s) for s in body))
        except Unsupported as e:
            raise Unsupported(f"rule {rule} ({rules[index][0]}): {e}\n{rules[index][2]}") from e
        # `$$` starts as `$1` for an action that reads it; one that sets it first has no use for the copy.
        first, _, rest = code.partition("\n")
        set_first = re.fullmatch(r"out = (.*);", first)
        if set_first and not re.search(r"\bout\b", set_first.group(1)):
            code = f"let mut out = {set_first.group(1)};\n{rest}"
        else:
            code = f"let mut out = y.first().cloned().unwrap_or(V::Null);\n{code}"
        # A value read once is handed on, with no count taken on it.
        for at in set(re.findall(r"y\[(\d+)\]", code)):
            if len(re.findall(r"y\[" + at + r"\]", code)) == 1 and not code.startswith("let mut out = y.first()"):
                code = code.replace(f"y[{at}].clone()", f"std::mem::take(&mut y[{at}])")
        if code not in written:
            written[code] = f"a{rule}"
            out.append(f"fn a{rule}(y: &mut [V], l: &[i32], here: i32) -> R {{\n{code}\nOk(out)\n}}")
        table.append(written[code])
    out.append("fn default_action(y: &mut [V], _: &[i32], _: i32) -> R {\nOk(y.first().cloned().unwrap_or(V::Null))\n}")
    out.append(f"pub static ACTIONS: [Action; {len(table) + 2}] = [default_action, default_action, " + ", ".join(table) + "];")
    return "\n".join(out) + "\n", found, tags, fields, rules


# ---------------------------------------------------------------- the deparser

# Written by hand in `deparse_rt.rs`: the output machinery, and what reads a C string byte by byte.
DEPARSE_BY_HAND = {
    "removeTrailingSpaceFromStr", "isReservedKeyword", "isOp", "makeDeparseStatePart", "freeDeparseStatePart", "deparseGetCurrentPartGroup", "deparseGetCurrentPart",
    "deparseMarkCurrentPartNonMergable", "deparseGetCurrentStringInfo", "deparseAppendStringInfo", "deparseAppendStringInfoString", "deparseAppendStringInfoChar", "removeTrailingSpace",
    "deparseRemoveTrailingEmptyPart", "deparseAppendPart", "deparseAppendCommaAndPart", "deparseAppendPartGroup", "deparseAppendCommentsIfNeeded", "deparseEmit", "deparseStateIncreaseNestingLevel",
    "deparseStateDecreaseNestingLevel", "deparseStringLiteral", "deparseGenericDefElemName", "deparseNumericOnly", "deparseValue", "deparseAConst", "optBooleanValue", "deparseRawStmt", "deparseRawStmtOpts",
    "pg_query_strict_cast_node", "pg_query_require_value_node", "pg_query_checked_list_nth", "pg_query_checked_list_last",
}


def c_functions(source):
    """Each function a C file defines: its name, parameter names and body tokens."""
    source = re.sub(r"/\*.*?\*/", " ", source, flags=re.S).replace("\\\n", " ")
    source = re.sub(r"^\s*#.*$", "", source, flags=re.M)
    tokens = tokenize(source)
    out, i, n = [], 0, len(tokens)
    while i < n:
        if tokens[i][1] != "{":
            i += 1
            continue
        depth, j = 0, i
        while True:
            if tokens[j][1] == "{":
                depth += 1
            elif tokens[j][1] == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        k = i - 1
        if tokens[k][1] == ")":
            parens = 0
            while True:
                if tokens[k][1] == ")":
                    parens += 1
                elif tokens[k][1] == "(":
                    parens -= 1
                    if parens == 0:
                        break
                k -= 1
            params, current, depth2 = [], [], 0
            for t in tokens[k + 1 : i - 1]:
                if t[1] == "(":
                    depth2 += 1
                elif t[1] == ")":
                    depth2 -= 1
                if t[1] == "," and depth2 == 0:
                    params.append(current)
                    current = []
                else:
                    current.append(t)
            if current:
                params.append(current)
            names = [p[-1][1] for p in params if p and p[-1][0] == "id" and not (len(p) == 1 and p[0][1] == "void")]
            out.append((tokens[k - 1][1], names, tokens[i + 1 : j]))
        i = j + 1
    return out


def translate_deparser(source, constant, field_of):
    functions = [f for f in c_functions(source) if f[0] not in DEPARSE_BY_HAND]
    own = frozenset(f[0] for f in functions)
    emitter = Emitter(constant, lambda n: n, field_of, in_function=True, own=own)
    out = [
        "// Generated by engine/syntax/gen/postgres.py from libpg_query's postgres_deparse.c.",
        "#![allow(unused_mut, unused_variables, unused_assignments, unused_labels, unused_parens, unreachable_code, non_snake_case, clippy::all)]",
        "use super::deparse_rt as h;",
        "use super::generated_names::{f, t};",
        "use super::nodes::{V, R};",
        "",
    ]
    calls = set()
    for name, params, body_tokens in functions:
        p = Parser(body_tokens)
        try:
            body = p.statements()
            if p.peek()[0] != "eof":
                raise Unsupported("trailing C: " + p.text())
        except Unsupported as e:
            raise Unsupported(f"{name}: {e}") from e
        found = {"fields": set(), "calls": set(), "ids": set(), "decls": set()}
        collect(body, found)
        calls |= found["calls"]
        emitter.scopes = [set(params)]
        emitter.targets = []
        try:
            code = without_clones("\n".join(emitter.statement(x) for x in body))
        except Unsupported as e:
            raise Unsupported(f"{name}: {e}") from e
        signature = ", ".join(f"p_{x}: &V" for x in params)
        # A parameter nothing assigns is read through the caller's reference.
        bind = "\n".join(f"let mut v_{x} = p_{x}.clone();" if re.search(r"\bv_" + x + r" = ", code) else f"let v_{x} = p_{x};" for x in params)
        out.append(f"pub fn {name}({signature}) -> R {{\n{bind}\n{code}\nOk(V::Null)\n}}")
    return "\n".join(out) + "\n", sorted(calls - own)
