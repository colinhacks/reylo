import { test } from "node:test";
import assert from "node:assert/strict";
import type { EngineType } from "../src/engine.ts";
import { renderParamType } from "../src/render.ts";
import { prepareRows, rowsOf, variant, type Members, type RowsType } from "../src/rows.ts";
import { array, checked, field, pg, record } from "./descriptors.ts";

/** A structured parameter as the engine gives it: its type (one record, or an array of them) and its column list as written. */
function structured(one: boolean, members: [name: string, type: EngineType, written: string, nullable: boolean, optional: boolean, schema?: object][]): { type: EngineType; members: Members; rows: RowsType } {
  const row = record(members.map(([name, type, , nullable, optional]) => field(name, type, nullable, optional)));
  const type: EngineType = one ? row : { kind: "array", element: { type: row } };
  const written: Members = members.map(([, , w, , , schema]) => (schema === undefined ? { written: w } : { written: w, schema: schema as { [key: string]: unknown } }));
  return { type, members: written, rows: rowsOf(type, written) };
}

// What the engine gives for `:events::rows(id int8 not null, ts timestamptz not null, status? text
// not null, amount numeric(12,2), tags text[], meta jsonb('{"type": "object", …}'))`.
const meta = { type: "object", properties: { source: { type: "string" } }, required: ["source"] };
const eventMembers: Parameters<typeof structured>[1] = [
  ["id", pg("int8"), "int8", false, false],
  ["ts", pg("timestamptz"), "timestamptz", false, false],
  ["status", pg("text"), "text", false, true],
  ["amount", pg("numeric"), "numeric(12,2)", true, true],
  ["tags", array(pg("text")), "text[]", true, true],
  ["meta", checked({ ...meta, additionalProperties: true }), "jsonb", true, true, meta],
];
const eventsOf = structured(false, eventMembers);
const events = eventsOf.rows;

test("the caller's type is the column list's: each key as its parameter type takes it, ? where it may be left out, | null where it may be null", () => {
  assert.equal(
    renderParamType(eventsOf.type, "postgres", false, eventsOf.members),
    "readonly { id: number | bigint | string; ts: string; status?: string; amount?: string | null; tags?: (string | null)[] | null; meta?: { source: string } | null }[]",
  );
  const one = structured(true, eventMembers.slice(0, 1));
  assert.equal(renderParamType(one.type, "postgres", false, one.members), "{ id: number | bigint | string }");
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
  assert.throws(() => prepareRows("p", { one: true, fields: [{ ...events.fields[2]! }] }, {}, "postgres", true), /every key is left out/);
});

test("a pgvector vector member takes an array of finite numbers, written as the text pgvector reads", () => {
  const itemsOf = structured(false, [
    ["embedding", pg("vector"), "vector(3)", false, false],
    ["history", array(pg("halfvec")), "halfvec[]", true, true],
  ]);
  const items = itemsOf.rows;
  assert.equal(renderParamType(itemsOf.type, "postgres", false, itemsOf.members), "readonly { embedding: number[]; history?: (number[] | null)[] | null }[]");
  assert.deepEqual(prepareRows("items", items, [{ embedding: [1, 2.5, 3], history: [[0.5], null] }], "postgres", true).value, [{ embedding: "[1,2.5,3]", history: ["[0.5]", null] }]);
  for (const embedding of [[], [1, NaN], ["1"], "x".length]) assert.throws(() => prepareRows("items", items, [{ embedding }], "postgres", true), /embedding.* is vector\(3\), an array of finite numbers with one at least/);
});

test("an hstore member takes an object of its keys, written as the text hstore reads", () => {
  const settingsOf = structured(true, [["prefs", pg("hstore"), "hstore", false, false]]);
  const settings = settingsOf.rows;
  assert.equal(renderParamType(settingsOf.type, "postgres", false, settingsOf.members), "{ prefs: Record<string, string | null> }");
  assert.deepEqual(prepareRows("settings", settings, { prefs: { theme: "dark", gone: null } }, "postgres", true).value, { prefs: '"theme"=>"dark", "gone"=>NULL' });
  assert.throws(() => prepareRows("settings", settings, { prefs: { n: 1 } }, "postgres", true), /prefs.* is hstore, whose values are strings or null, and its key "n" holds 1/);
  assert.throws(() => prepareRows("settings", settings, { prefs: 1 }, "postgres", true), /prefs.* is hstore, an object of its keys/);
});
