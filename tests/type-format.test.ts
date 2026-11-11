import { test } from "node:test";
import assert from "node:assert/strict";
import { formatType } from "../src/type-format.ts";

// A row type as src/module-types.ts writes one where columns have comments: a field per line, each
// after its doc comment, the escaped `*\/` of a comment that holds one included.
const commented = "{\n  id: number;\n  /** the name, as typed *\\/ */\n  name: string;\n  /**\n   * line one\n   * line two\n   */\n  email: string | null;\n}";

test("formatType keeps a column's doc comment on the lines before its field", () => {
  assert.equal(formatType(`${commented}[]`, 80), "{\n  id: number;\n  /** the name, as typed *\\/ */\n  name: string;\n  /**\n   * line one\n   * line two\n   */\n  email: string | null;\n}[]");
  // Inside a union, indented with the object it documents.
  assert.equal(formatType(`{ post: ${commented.replaceAll("\n", "\n  ")} } | null`, 80), "{\n  post: {\n    id: number;\n    /** the name, as typed *\\/ */\n    name: string;\n    /**\n     * line one\n     * line two\n     */\n    email: string | null;\n  };\n} | null");
});

test("formatType puts doc comments on one line where the type has no width", () => {
  assert.equal(formatType(commented, Infinity), "{ id: number; /** the name, as typed *\\/ */ name: string; /** line one line two */ email: string | null }");
  // A blank line between paragraphs, which the comment writes as a bare ` *`, is one space too.
  assert.equal(formatType("{\n  /**\n   * para one\n   *\n   * para two\n   */\n  x: string;\n}", Infinity), "{ /** para one para two */ x: string }");
});

test("formatType leaves a type with no comment on one line where it fits", () => {
  assert.equal(formatType("{ id: number; name: string } | null", 40), "{ id: number; name: string } | null");
  assert.equal(formatType("{ id: number; name: string }[]", 20), "{\n  id: number;\n  name: string;\n}[]");
});

test("formatType reads Json, the named type of any JSON value, wherever src/render.ts writes it", () => {
  assert.equal(formatType("{ stage: Json }", 80), "{ stage: Json }");
  const row = "{ doc: Json; tags: Json[]; meta: Record<string, Json>; g: { type: string; [key: string]: Json | undefined } }";
  assert.equal(formatType(row, Infinity), row);
  assert.equal(formatType(row, 40), "{\n  doc: Json;\n  tags: Json[];\n  meta: Record<string, Json>;\n  g: {\n    type: string;\n    [key: string]: Json | undefined;\n  };\n}");
});

test("formatType keeps number literals, as a JSON Schema's enum or const writes them", () => {
  const row = '{ columns: "auto" | 1 | 2 | -0.5 | 1e+21; version: 2 }';
  assert.equal(formatType(row, Infinity), row);
  assert.equal(formatType(row, 44), '{\n  columns: "auto" | 1 | 2 | -0.5 | 1e+21;\n  version: 2;\n}');
});

test("formatType reads tuples, as a JSON Schema's prefixItems writes them", () => {
  for (const t of ["{ s: [string, number][] }", "{ p: [string, number?, ...Json[]] }", "{ e: [] }", "{ o: [(string | null)?, ...(number | boolean)[]] }", "{ n: [[number, number], { x: number }] }"]) {
    assert.equal(formatType(t, Infinity), t);
  }
  assert.equal(formatType("{ id: number; samples: [string, number][] } | null", 64), "{ id: number; samples: [string, number][] } | null");
  assert.equal(formatType("{ samples: [string, { at: string; value: number }?][] }", 30), "{\n  samples: [\n    string,\n    {\n      at: string;\n      value: number;\n    }?,\n  ][];\n}");
});

test("formatType reads readonly arrays and tuples, as a parameter's type writes them", () => {
  const params = "{ ids: readonly number[]; pair: readonly [string, number]; pairs: readonly { key: string; value: number | null }[] }";
  assert.equal(formatType(params, Infinity), params);
  assert.equal(formatType("{ pair: readonly [string, number] }", 20), "{\n  pair: readonly [\n    string,\n    number,\n  ];\n}");
});

test("formatType refuses text it cannot read rather than printing the type without it", () => {
  assert.throws(() => formatType("{ a: string & { b: number } }", Infinity), /no form reads "&"/);
});

test("formatType breaks a union too wide for its line with a member per line, as Prettier does", () => {
  const shape = '{ kind: "circle"; radius: number } | { kind: "square"; size: number }';
  assert.equal(formatType(`{ id: number; shape: ${shape} } | null`, 64), '{\n  id: number;\n  shape:\n    | { kind: "circle"; radius: number }\n    | { kind: "square"; size: number };\n} | null');
  // A union that fits once it is on a line of its own still takes a line per member there.
  assert.equal(formatType('{ id: number; shape: { kind: "circle"; r: number } | { kind: "rect"; w: number } } | null', 64), '{\n  id: number;\n  shape:\n    | { kind: "circle"; r: number }\n    | { kind: "rect"; w: number };\n} | null');
  // As an array's element, inside its parentheses.
  assert.equal(formatType(`{ shapes: (${shape})[] }`, 64), '{\n  shapes: (\n    | { kind: "circle"; radius: number }\n    | { kind: "square"; size: number }\n  )[];\n}');
  // A member too wide for its own line breaks under its `|`.
  assert.equal(formatType(`{ shape: ${shape} }`, 30), '{\n  shape:\n    | {\n        kind: "circle";\n        radius: number;\n      }\n    | {\n        kind: "square";\n        size: number;\n      };\n}');
  // One type with null keeps its place on the line, and its object breaks.
  assert.equal(formatType("{ post: { id: number; title: string } | null }", 30), "{\n  post: {\n    id: number;\n    title: string;\n  } | null;\n}");
});
