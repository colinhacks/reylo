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
