import { test } from "node:test";
import assert from "node:assert/strict";
import type { EngineType } from "../src/engine.ts";
import { renderParamType } from "../src/render.ts";
import { prepareRows, variant, type RowsType } from "../src/rows.ts";

// What the engine gives for `:events::rows(id int8 not null, ts timestamptz not null, status? text
// not null, amount numeric(12,2), tags text[], meta jsonb('{"type": "object", …}'))`.
const scalar = (name: string): EngineType => ({ kind: "scalar", name });
const events: RowsType = {
  kind: "rows",
  one: false,
  fields: [
    { name: "id", type: scalar("int8"), written: "int8", nullable: false, optional: false },
    { name: "ts", type: scalar("timestamptz"), written: "timestamptz", nullable: false, optional: false },
    { name: "status", type: scalar("text"), written: "text", nullable: false, optional: true },
    { name: "amount", type: scalar("numeric"), written: "numeric(12,2)", nullable: true, optional: true },
    { name: "tags", type: { kind: "array", of: scalar("text") }, written: "text[]", nullable: true, optional: true },
    { name: "meta", type: { kind: "json", schema: { type: "object", properties: { source: { type: "string" } }, required: ["source"], additionalProperties: true } }, written: "jsonb", nullable: true, optional: true, schema: { type: "object", properties: { source: { type: "string" } }, required: ["source"] } },
  ],
};

test("the caller's type is the column list's: each key as its parameter type takes it, ? where it may be left out, | null where it may be null", () => {
  assert.equal(
    renderParamType(events, "postgres"),
    "readonly { id: number | bigint | string; ts: string; status?: string; amount?: string | null; tags?: (string | null)[] | null; meta?: { source: string } | null }[]",
  );
  assert.equal(renderParamType({ ...events, one: true, fields: events.fields.slice(0, 1) }, "postgres"), "{ id: number | bigint | string }");
});

test("a value is checked against its column list, and written as the JSON the statement reads", () => {
  const row = { id: 9007199254740993n, ts: "2026-10-20T10:00:00.000Z", status: "done", amount: "1.50", tags: ["a"], meta: { source: "x" } };
  // Postgres reads a 64-bit integer from its digits exactly, and the run time binds the JSON value.
  assert.deepEqual(prepareRows("events", events, [row], "postgres", true).value, [{ id: "9007199254740993", ts: "2026-10-20T10:00:00.000Z", status: "done", amount: "1.50", tags: ["a"], meta: { source: "x" } }]);
  // MySQL and SQLite take the JSON text, with the integer a bare number.
  assert.equal(prepareRows("events", events, [row], "mysql", true).value, `[{"id":9007199254740993,"ts":"2026-10-20T10:00:00.000Z","status":"done","amount":"1.50","tags":["a"],"meta":{"source":"x"}}]`);
  const refused: [unknown, RegExp][] = [
    [{ ts: "2026-10-20T10:00:00Z" }, /row 1: "id" is int8 not null, and the row leaves it out/],
    [{ id: 1, ts: "2026-10-20T10:00:00Z", status: null }, /"status" is text not null, and the row gives null; leave the key out for the column's default/],
    [{ id: 1, ts: "2026-10-20T10:00:00Z", stauts: "x" }, /the column list names no key "stauts"/],
    [{ id: 2 ** 53, ts: "2026-10-20T10:00:00Z" }, /past 2\^53 - 1/],
    [{ id: 2n ** 63n, ts: "2026-10-20T10:00:00Z" }, /from -9223372036854775808 to 9223372036854775807/],
    [{ id: 1.5, ts: "2026-10-20T10:00:00Z" }, /"id" is int8, an integer; got 1.5/],
    [{ id: 1, ts: "2026-10-20 10:00" }, /names its zone/],
    [{ id: 1, ts: new Date() }, /not a Date: pass date.toISOString\(\)/],
    [{ id: 1, ts: "2026-10-20T10:00:00Z", amount: "1.234" }, /keeps 2 digits after the point, so 1.234 would be rounded/],
    [{ id: 1, ts: "2026-10-20T10:00:00Z", amount: "12345678901" }, /at most 10 digits before the point/],
    [{ id: 1, ts: "2026-10-20T10:00:00Z", tags: ["a", 1] }, /"tags" element 2 is text, a string; got 1/],
    [{ id: 1, ts: "2026-10-20T10:00:00Z", meta: { n: 1n } }, /holds a JSON value/],
  ];
  for (const [value, message] of refused) assert.throws(() => prepareRows("events", events, [value], "postgres", true), message, JSON.stringify(value, (_, v) => (typeof v === "bigint" ? `${v}n` : v)));
  assert.throws(() => prepareRows("events", events, { id: 1 }, "postgres", true), /pass an array of objects, one per row; got an object/);
  assert.throws(() => prepareRows("user", { ...events, one: true }, [], "postgres", true), /each row is an object/);
});

test("the rows an insert writes carry the same keys, and its lists name only those", () => {
  const a = { id: 1, ts: "2026-10-20T10:00:00Z" };
  assert.throws(() => prepareRows("events", events, [a, { ...a, status: "x" }], "postgres", true), /row 2 gives "status", which row 1 leaves out/);
  // Elsewhere a key left out reads as NULL, so the rows may differ.
  assert.doesNotThrow(() => prepareRows("events", { ...events, fields: events.fields.filter((f) => f.name !== "status") }, [a, { ...a, amount: 1 }], "postgres", false));
  const { present } = prepareRows("events", events, [a, a], "postgres", true);
  const text = `insert into events ("id", "ts", "status", "amount", "tags", "meta") SELECT e."id", e."ts", e."status", e."amount", e."tags", e."meta" FROM x`;
  const entries = (prefix: string) => ["id", "ts", "status", "amount", "tags", "meta"].map((n) => `${prefix}"${n}"`);
  const list = (entries: string[]) => {
    const start = text.indexOf(entries.join(", "));
    return { start, end: start + entries.join(", ").length, entries };
  };
  const structured = { name: "events", lists: [list(entries("")), list(entries("e."))] };
  assert.equal(variant(text, structured, events.fields, present), `insert into events ("id", "ts") SELECT e."id", e."ts" FROM x`);
  assert.throws(() => prepareRows("p", { kind: "rows", one: true, fields: [{ ...events.fields[2]! }] }, {}, "postgres", true), /every key is left out/);
});

test("a pgvector vector member takes an array of finite numbers, written as the text pgvector reads", () => {
  const items: RowsType = {
    kind: "rows",
    one: false,
    fields: [
      { name: "embedding", type: scalar("vector"), written: "vector(3)", nullable: false, optional: false },
      { name: "history", type: { kind: "array", of: scalar("halfvec") }, written: "halfvec[]", nullable: true, optional: true },
    ],
  };
  assert.equal(renderParamType(items), "readonly { embedding: number[]; history?: (number[] | null)[] | null }[]");
  assert.deepEqual(prepareRows("items", items, [{ embedding: [1, 2.5, 3], history: [[0.5], null] }], "postgres", true).value, [{ embedding: "[1,2.5,3]", history: ["[0.5]", null] }]);
  for (const embedding of [[], [1, NaN], ["1"], "x".length]) assert.throws(() => prepareRows("items", items, [{ embedding }], "postgres", true), /embedding.* is vector\(3\), an array of finite numbers with one at least/);
});

test("an hstore member takes an object of its keys, written as the text hstore reads", () => {
  const settings: RowsType = {
    kind: "rows",
    one: true,
    fields: [{ name: "prefs", type: scalar("hstore"), written: "hstore", nullable: false, optional: false }],
  };
  assert.equal(renderParamType(settings), "{ prefs: Record<string, string | null> }");
  assert.deepEqual(prepareRows("settings", settings, { prefs: { theme: "dark", gone: null } }, "postgres", true).value, { prefs: '"theme"=>"dark", "gone"=>NULL' });
  assert.throws(() => prepareRows("settings", settings, { prefs: { n: 1 } }, "postgres", true), /prefs.* is hstore, whose values are strings or null, and its key "n" holds 1/);
  assert.throws(() => prepareRows("settings", settings, { prefs: 1 }, "postgres", true), /prefs.* is hstore, an object of its keys/);
});
