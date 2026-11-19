import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { moduleCode, type ModuleInput } from "../src/loader.ts";

// Statements' analyses as the engine gave them, each with the project's settings, and the module a
// build step wrote for it, held character for character: Postgres with a decimal codec and its
// library, a JSON column that may hold NaN, a lone value, a structured insert, and a total order
// with nested arrays and no sqlcommenter comment; MySQL's structured insert with the lists it
// writes again, and a cast beside a typed JSON parameter; SQLite.
const FIXTURES: { name: string; input: ModuleInput; code: string }[] = JSON.parse(readFileSync(new URL("./module-code.json", import.meta.url), "utf8"));

test("a module is its imports, its exports and its query's definition, each field written as it was", () => {
  assert.equal(FIXTURES.length, 8);
  for (const { name, input, code } of FIXTURES) assert.equal(moduleCode(input), code, name);
});
