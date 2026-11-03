// A TypeScript type as the docs' Inference cards (site/lib/remark-mapper.ts) and `reylo play` show
// it: the one-line type a statement's module declares (src/render.ts's form: object literals,
// unions, arrays, tuples, `readonly`, string and number literals and names, and a column's comment
// as the doc comment before its field, src/module-types.ts), broken over lines where it is wider
// than the column.

type Node =
  | { kind: "name"; text: string }
  | { kind: "object"; members: { key: string; type: Node; doc?: string }[] }
  | { kind: "union"; of: Node[] }
  | { kind: "array"; of: Node }
  | { kind: "tuple"; items: { type: Node; optional: boolean; rest: boolean }[] }
  | { kind: "readonly"; of: Node };

// A doc comment is one token, the field's that follows it.
const TOKEN = /\/\*\*[\s\S]*?\*\/|"(?:[^"\\]|\\.)*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|[A-Za-z_$][\w$]*(?:<[^<>]*>)?|\.\.\.|\[\]|[{}()[\]|;:?,]/g;

/** The type's text as a tree. Throws on text outside the forms src/render.ts writes. */
export function parseType(text: string): Node {
  const tokens = text.match(TOKEN) ?? [];
  // Text no token matches would be dropped without a word, and the type printed wrong.
  const unread = text.replace(TOKEN, "").trim();
  if (unread) throw new Error(`type-format: no form reads "${unread}" in ${text}`);
  let at = 0;
  const peek = () => tokens[at];
  const take = (expected?: string) => {
    const token = tokens[at++];
    if (token === undefined || (expected !== undefined && token !== expected)) throw new Error(`type-format: expected ${expected ?? "a token"} at ${at - 1} in ${text}`);
    return token;
  };
  const union = (): Node => {
    const of = [postfix()];
    while (peek() === "|") {
      take("|");
      of.push(postfix());
    }
    return of.length === 1 ? of[0]! : { kind: "union", of };
  };
  const postfix = (): Node => {
    // `readonly` binds looser than `[]`: `readonly string[]` is a read-only array of strings.
    if (peek() === "readonly") {
      take("readonly");
      return { kind: "readonly", of: postfix() };
    }
    let node = primary();
    while (peek() === "[]") {
      take("[]");
      node = { kind: "array", of: node };
    }
    return node;
  };
  const primary = (): Node => {
    const token = peek();
    if (token === "(") {
      take("(");
      const inner = union();
      take(")");
      return inner;
    }
    if (token === "{") return object();
    // `[]` here is the empty tuple; after a type it is an array, which `postfix` reads.
    if (token === "[]") {
      take("[]");
      return { kind: "tuple", items: [] };
    }
    if (token === "[") return tuple();
    return { kind: "name", text: take() };
  };
  const tuple = (): Node => {
    take("[");
    const items: { type: Node; optional: boolean; rest: boolean }[] = [];
    while (peek() !== "]") {
      const rest = peek() === "...";
      if (rest) take("...");
      const type = union();
      const optional = peek() === "?";
      if (optional) take("?");
      items.push({ type, optional, rest });
      if (peek() === ",") take(",");
      else if (peek() !== "]") take("]");
    }
    take("]");
    return { kind: "tuple", items };
  };
  const object = (): Node => {
    take("{");
    const members: { key: string; type: Node }[] = [];
    while (peek() !== "}") {
      // Its text, one line per line of the comment, without the comment's own indentation.
      const doc = peek()?.startsWith("/**") ? take().split("\n").map((line, i) => (i ? ` ${line.trim()}` : line.trim())).join("\n") : undefined;
      let key: string;
      if (peek() === "[") {
        // An index signature, `[key: string]: unknown`.
        take("[");
        const name = take();
        take(":");
        const of = take();
        take("]");
        key = `[${name}: ${of}]`;
      } else key = take();
      if (peek() === "?") key += take("?");
      take(":");
      members.push({ key, type: union(), ...(doc ? { doc } : {}) });
      if (peek() === ";") take(";");
    }
    take("}");
    return { kind: "object", members };
  };
  const node = union();
  if (at !== tokens.length) throw new Error(`type-format: unread text after ${tokens.slice(0, at).join(" ")} in ${text}`);
  return node;
}

/** A doc comment on one line: `/** line one line two *\/`. */
const oneLineDoc = (doc: string) => doc.replace(/\n\s*\*\/$/, " */").replace(/(?:\n[ \t]*\*[ \t]*)+/g, " ").replace(/^\/\*\*\s*/, "/** ");

/** Whether an object in the type has a field with a doc comment. */
const hasDoc = (node: Node): boolean => {
  switch (node.kind) {
    case "name":
      return false;
    case "object":
      return node.members.some((m) => m.doc || hasDoc(m.type));
    case "union":
      return node.of.some(hasDoc);
    case "tuple":
      return node.items.some((item) => hasDoc(item.type));
    case "array":
    case "readonly":
      return hasDoc(node.of);
  }
};

/** Whether the type needs parentheses as an array's element or an optional tuple element. */
const loose = (node: Node) => node.kind === "union" || node.kind === "readonly";

/** A tuple element's text around its type: `...` before a rest element, `?` after an optional one, and the parentheses a loose type needs there. */
const around = (item: { type: Node; optional: boolean; rest: boolean }) => {
  const wrap = item.optional && loose(item.type);
  return { before: `${item.rest ? "..." : ""}${wrap ? "(" : ""}`, after: `${wrap ? ")" : ""}${item.optional ? "?" : ""}` };
};

const flat = (node: Node): string => {
  switch (node.kind) {
    case "name":
      return node.text;
    case "object":
      return node.members.length ? `{ ${node.members.map((m) => `${m.doc ? `${oneLineDoc(m.doc)} ` : ""}${m.key}: ${flat(m.type)}`).join("; ")} }` : "{}";
    case "union":
      return node.of.map(flat).join(" | ");
    case "array":
      return loose(node.of) ? `(${flat(node.of)})[]` : `${flat(node.of)}[]`;
    case "tuple":
      return `[${node.items.map((item) => `${around(item).before}${flat(item.type)}${around(item).after}`).join(", ")}]`;
    case "readonly":
      return `readonly ${flat(node.of)}`;
  }
};

// A type with a doc comment is broken over lines at any finite width, each comment on the lines before its field, as TypeScript reads one.
const fits = (node: Node, lead: number, width: number, reserve: number) => lead + flat(node).length + reserve <= width && (width === Infinity || !hasDoc(node));

/** Whether the union is one type with `null` or `undefined`, which keeps its place on the line, as Prettier keeps it. */
const hugs = (node: Node) => node.kind === "union" && node.of.filter((n) => !(n.kind === "name" && (n.text === "null" || n.text === "undefined"))).length === 1;

type Union = Node & { kind: "union" };

/** Whether the type is a union too wide for its line that takes a line per member, each after a leading `|`, as Prettier breaks one; it starts on a line of its own. */
const piped = (node: Node, lead: number, width: number, reserve: number): node is Union => node.kind === "union" && !hugs(node) && !fits(node, lead, width, reserve);

/** A union on lines of its own, starting at `indent`: a member per line after a leading `|`, the last followed by `reserve` columns. */
const pipes = (node: Union, indent: number, width: number, reserve = 0) =>
  node.of.map((n, i) => `| ${print(n, indent + 2, indent + 2, width, i === node.of.length - 1 ? reserve : 0)}`).join(`\n${" ".repeat(indent)}`);

/**
 * The type on one line where it fits `width` columns after the `lead` columns already on its line
 * and before the `reserve` columns that must follow it there, else with each object's members on
 * lines of their own, indented two spaces past `indent`, the indentation of the line it starts on.
 */
function print(node: Node, indent: number, lead: number, width: number, reserve = 0): string {
  const one = flat(node);
  if (fits(node, lead, width, reserve)) return one;
  const pad = " ".repeat(indent);
  switch (node.kind) {
    case "name":
      return one;
    case "object": {
      const inner = " ".repeat(indent + 2);
      const doc = (text: string | undefined) => (text ? `${text.split("\n").map((line) => inner + line).join("\n")}\n` : "");
      const member = (m: { key: string; type: Node }) => {
        const lead = indent + 2 + m.key.length + 2;
        if (!piped(m.type, lead, width, 1)) return `${m.key}: ${print(m.type, indent + 2, lead, width, 1)};`;
        return `${m.key}:\n${inner}  ${pipes(m.type, indent + 4, width, 1)};`;
      };
      return `{\n${node.members.map((m) => `${doc(m.doc)}${inner}${member(m)}`).join("\n")}\n${pad}}`;
    }
    case "union": {
      if (!hugs(node) && lead === indent) return pipes(node, indent, width, reserve);
      let line = lead;
      return node.of
        .map((n, i) => {
          const tail = i < node.of.length - 1 ? 3 + flat({ kind: "union", of: node.of.slice(i + 1) }).length + reserve : reserve;
          const text = print(n, indent, line, width, tail);
          const last = text.split("\n").at(-1)!;
          line = (text.includes("\n") ? last.length : line + last.length) + 3;
          return text;
        })
        .join(" | ");
    }
    case "array":
      if (piped(node.of, lead + 1, width, reserve + 3)) return `(\n${pad}  ${pipes(node.of, indent + 2, width)}\n${pad})[]`;
      return loose(node.of) ? `(${print(node.of, indent, lead + 1, width, reserve + 3)})[]` : `${print(node.of, indent, lead, width, reserve + 2)}[]`;
    case "tuple": {
      if (!node.items.length) return one;
      // Each element on a line of its own, as an object's members are.
      const inner = " ".repeat(indent + 2);
      const items = node.items.map((item) => {
        const { before, after } = around(item);
        if (before === "(" && piped(item.type, indent + 3, width, after.length + 1)) return `${inner}(\n${inner}  ${pipes(item.type, indent + 4, width)}\n${inner}${after},`;
        return `${inner}${before}${print(item.type, indent + 2, indent + 2 + before.length, width, after.length + 1)}${after},`;
      });
      return `[\n${items.join("\n")}\n${pad}]`;
    }
    case "readonly":
      return `readonly ${print(node.of, indent, lead + "readonly ".length, width, reserve)}`;
  }
}

/** The type's text, its objects, tuples and unions broken over lines where it is wider than `width`. */
export function formatType(text: string, width: number): string {
  return print(parseType(text), 0, 0, width);
}

/** What `.run()` resolves to for a statement's row type and cardinality, as the module's `Query` reads them. */
export function resultType(row: string, cardinality: string): string {
  if (cardinality === "none" || row === "never") return "void";
  const wrapped = /\|/.test(row.replace(/\{[^{}]*\}/g, "")) ? `(${row})` : row;
  if (cardinality === "one") return row;
  if (cardinality === "maybeOne") return `${row} | null`;
  return `${wrapped}[]`;
}
