// A TypeScript type as the docs' Inference cards (site/lib/remark-mapper.ts) and `reylo play` show
// it: the one-line type a statement's module declares (src/render.ts's form: object literals,
// unions, arrays, string literals and names, and a column's comment as the doc comment before its
// field, src/module-types.ts), broken over lines where it is wider than the column.

type Node =
  | { kind: "name"; text: string }
  | { kind: "object"; members: { key: string; type: Node; doc?: string }[] }
  | { kind: "union"; of: Node[] }
  | { kind: "array"; of: Node };

/** The type's text as a tree. Throws on text outside the forms src/render.ts writes. */
export function parseType(text: string): Node {
  // A doc comment is one token, the field's that follows it.
  const tokens = text.match(/\/\*\*[\s\S]*?\*\/|"(?:[^"\\]|\\.)*"|[A-Za-z_$][\w$]*(?:<[^<>]*>)?|\[\]|[{}()[\]|;:?,]/g) ?? [];
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
    return { kind: "name", text: take() };
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
const oneLineDoc = (doc: string) => doc.replace(/\n\s*\*\/$/, " */").replace(/\n\s*\*\s?/g, " ").replace(/^\/\*\*\s*/, "/** ");

/** Whether an object in the type has a field with a doc comment. */
const hasDoc = (node: Node): boolean =>
  node.kind === "object" ? node.members.some((m) => m.doc || hasDoc(m.type)) : node.kind === "union" ? node.of.some(hasDoc) : node.kind === "array" ? hasDoc(node.of) : false;

const flat = (node: Node): string => {
  switch (node.kind) {
    case "name":
      return node.text;
    case "object":
      return node.members.length ? `{ ${node.members.map((m) => `${m.doc ? `${oneLineDoc(m.doc)} ` : ""}${m.key}: ${flat(m.type)}`).join("; ")} }` : "{}";
    case "union":
      return node.of.map(flat).join(" | ");
    case "array":
      return node.of.kind === "union" ? `(${flat(node.of)})[]` : `${flat(node.of)}[]`;
  }
};

/**
 * The type on one line where it fits `width` columns after the `lead` columns already on its line
 * and before the `reserve` columns that must follow it there, else with each object's members on
 * lines of their own, indented two spaces past `indent`, the indentation of the line it starts on.
 */
function print(node: Node, indent: number, lead: number, width: number, reserve = 0): string {
  const one = flat(node);
  // A type with a doc comment is broken over lines at any finite width, each comment on the lines before its field, as TypeScript reads one.
  if (lead + one.length + reserve <= width && (width === Infinity || !hasDoc(node))) return one;
  switch (node.kind) {
    case "name":
      return one;
    case "object": {
      const inner = " ".repeat(indent + 2);
      const doc = (text: string | undefined) => (text ? `${text.split("\n").map((line) => inner + line).join("\n")}\n` : "");
      return `{\n${node.members.map((m) => `${doc(m.doc)}${inner}${m.key}: ${print(m.type, indent + 2, indent + 2 + m.key.length + 2, width, 1)};`).join("\n")}\n${" ".repeat(indent)}}`;
    }
    case "union": {
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
      return node.of.kind === "union" ? `(${print(node.of, indent, lead + 1, width, reserve + 3)})[]` : `${print(node.of, indent, lead, width, reserve + 2)}[]`;
  }
}

/** The type's text, its objects broken over lines where it is wider than `width`. */
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
