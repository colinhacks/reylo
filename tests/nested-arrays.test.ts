// `allow_nested_arrays: true` in the project file: a Postgres array column typed at any depth.
import { test } from "node:test";
import assert from "node:assert/strict";
import { moduleTypes, reyloImport } from "../src/module-types.ts";
import { querySchemas } from "../src/schema.ts";
import { playResult } from "../src/play-result.ts";
import { analysisWithNestedArrays } from "../src/engine-type.ts";
import type { Analysis } from "../src/engine.ts";

const analysis = {
  params: [],
  columns: [
    { name: "cells", nullable: false, type: { kind: "array", any_depth: true, name: "_int4", element: { nullable: true, type: { kind: "integer", bits: 32, name: "int4" } } } },
    { name: "stacked", nullable: false, type: { kind: "array", name: "_int4", element: { type: { kind: "array", any_depth: true, name: "_int4", element: { nullable: true, type: { kind: "integer", bits: 32, name: "int4" } } } } } },
  ],
  cardinality: "many",
  kind: "select",
  diagnostics: [],
  rewritten: { sql: "select cells, array_agg(cells) as stacked from grids", spans: [], order: [] },
} as unknown as Analysis;

test("a Postgres array column is typed with one dimension by default, and at any depth under allow_nested_arrays", () => {
  const flat = moduleTypes("", analysis, "postgres", undefined, true);
  assert.match(flat.row, /cells: \(number \| null\)\[\]/);
  assert.match(flat.row, /stacked: \(number \| null\)\[\]\[\]/);
  assert.equal(reyloImport(flat.row), `import type { Query } from "reylo";`);
  const nested = moduleTypes("", analysis, "postgres", undefined, true, ".reylo.yaml", {}, true);
  assert.match(nested.row, /cells: NestedArray<number \| null>/);
  // An array the statement builds of arrays keeps its levels, over the arrays it collects.
  assert.match(nested.row, /stacked: NestedArray<number \| null>\[\]/);
  assert.equal(reyloImport(nested.row), `import type { NestedArray, Query } from "reylo";`);
});

test("reylo play types an array column as the type check does under allow_nested_arrays", () => {
  assert.match(playResult("", "postgres", { analysis }).element ?? "", /cells: \(number \| null\)\[\]/);
  assert.match(playResult("", "postgres", { analysis }, { allowNestedArrays: true }).element ?? "", /cells: NestedArray<number \| null>/);
});

test("a query's output schema lets an array nest under allow_nested_arrays", () => {
  const items = (a: Analysis) => (querySchemas(a, "postgres").output as { items: { properties: { cells: { items: unknown } } } }).items.properties.cells.items;
  assert.deepEqual(items(analysisWithNestedArrays(analysis)), { anyOf: [items(analysis), { type: "array" }] });
});

test("an array column inside JSON a statement builds is typed at any depth under allow_nested_arrays", () => {
  const built = { ...analysis, columns: [{ name: "doc", nullable: false, type: { kind: "json", name: "jsonb", content: { type: { kind: "record", fields: [{ name: "cells", type: analysis.columns[0]!.type }] } } } }] } as unknown as Analysis;
  assert.match(moduleTypes("", built, "postgres", undefined, true).row, /cells: \(number \| null\)\[\]/);
  const nested = moduleTypes("", built, "postgres", undefined, true, ".reylo.yaml", {}, true);
  assert.match(nested.row, /cells: NestedArray<number \| null>/);
  assert.equal(reyloImport(nested.row), `import type { NestedArray, Query } from "reylo";`);
});
