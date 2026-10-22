import { test } from "node:test";
import assert from "node:assert/strict";
import { docComment, leadingComment, statementAfter } from "../src/doc-comment.ts";
import type { Dialect } from "../src/engine.ts";

const read = (sql: string, dialect?: Dialect) => {
  const leading = leadingComment(sql, dialect);
  return [leading.prose, statementAfter(sql, leading)];
};

test("the comment before the statement is its prose, markers stripped and lines kept", () => {
  assert.deepEqual(read("-- The signed-in user's profile.\n-- Null when the id no longer exists.\nselect 1\n"), ["The signed-in user's profile.\nNull when the id no longer exists.", "select 1"]);
  // Blank lines around it, indentation the lines share, and the indentation of one line beyond it.
  assert.deepEqual(read("\n\n  --  One.\n  --    Two.\n\nselect 1"), ["One.\n  Two.", "select 1"]);
  // A blank line, or a change of form, starts a paragraph.
  assert.deepEqual(read("-- One.\n\n-- Two.\n/* Three. */ select 1"), ["One.\n\nTwo.\n\nThree.", "select 1"]);
  // A block written as JSDoc is, its stars and its tags left as they are.
  assert.deepEqual(read("/**\n * One.\n *   Two.\n * @see x\n */\nselect 1"), ["One.\n  Two.\n@see x", "select 1"]);
  assert.deepEqual(read("/*\n    One.\n    Two.\n*/\nselect 1"), ["One.\nTwo.", "select 1"]);
  // A comment after the first token is the statement's.
  assert.deepEqual(read("select 1 -- one\n-- two\n"), ["", "select 1 -- one\n-- two"]);
  assert.deepEqual(read("select 1"), ["", "select 1"]);
  assert.deepEqual(read("-- only a comment\n"), ["only a comment", ""]);
});

test("comments are read as the dialect reads them", () => {
  // Postgres nests block comments; MySQL and SQLite end one at the first `*/`.
  assert.deepEqual(read("/* a /* b */ c */ select 1", "postgres"), ["a /* b */ c", "select 1"]);
  assert.deepEqual(read("/* a /* b */ c */ select 1", "sqlite"), ["a /* b", "c */ select 1"]);
  // On MySQL `#` starts a comment, `--` starts one only before a space, and `/*!` and `/*+` are code.
  assert.deepEqual(read("# One.\n-- Two.\nselect 1", "mysql"), ["One.\nTwo.", "select 1"]);
  assert.deepEqual(read("--1\nselect 1", "mysql"), ["", "--1\nselect 1"]);
  assert.deepEqual(read("/*!40101 set names utf8 */", "mysql"), ["", "/*!40101 set names utf8 */"]);
  assert.deepEqual(read("# One.\nselect 1", "postgres"), ["", "# One.\nselect 1"]);
});

test("the doc comment holds the prose, the statement in a sql block and the note, and nothing ends it early", () => {
  assert.equal(docComment("One.\nTwo.", "select 1\nfrom t", "What it is."), ["/**", " * One.", " * Two.", " *", " * ```sql", " * select 1", " * from t", " * ```", " *", " * What it is.", " */", ""].join("\n"));
  assert.equal(docComment("", "select 1", "", "  "), ["  /**", "   * ```sql", "   * select 1", "   * ```", "   */", ""].join("\n"));
  assert.equal(docComment("", undefined), "");
  // A star before a slash, in the prose or the statement, would end the comment.
  const ends = docComment("a */ b", "select 1 /* c */");
  assert.equal(ends.match(/\*\//g)!.length, 1);
  assert.match(ends, /a \*\\\/ b/);
  assert.match(ends, /select 1 \/\* c \*\\\//);
  // The fence is longer than any run of backticks in the statement, and a fence the prose opens is closed.
  assert.match(docComment("", "select '```' as a, '````' as b"), /^ \* `````sql$[\s\S]*^ \* `````$/m);
  assert.equal(docComment("See:\n```ts\nrun()", "select 1"), ["/**", " * See:", " * ```ts", " * run()", " * ```", " *", " * ```sql", " * select 1", " * ```", " */", ""].join("\n"));
});
