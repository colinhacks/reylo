import { test } from "node:test";
import assert from "node:assert/strict";
import type { EngineType } from "../src/engine.ts";
import { renderParamType, renderType } from "../src/render.ts";

const scalar = (name: string): EngineType => ({ kind: "scalar", name });
const field = (name: string, type: EngineType, nullable: boolean) => ({ name, type, nullable });

// A table's row type as the engine renders it once the catalog gave its columns.
const users: EngineType = {
  kind: "composite",
  name: "users",
  fields: [
    field("id", scalar("int8"), false),
    field("seen", scalar("timestamptz"), true),
    field("tags", { kind: "array", of: scalar("text") }, true),
    field("mood", { kind: "enum", name: "mood", labels: ["ok", "sad"] }, false),
  ],
};
// `json_build_object('author', u, 'posts', json_agg(p), 'n', count(*))`: a composite inside a document.
const doc: EngineType = {
  kind: "object",
  fields: [
    field("author", users, false),
    field("posts", { kind: "list", of: users, nullable_items: true }, true),
    field("n", scalar("int8"), false),
  ],
};

const inJson = `{ id: number; seen: string | null; tags: (string | null)[] | null; mood: "ok" | "sad" }`;

test("a composite is its row's text on the wire", () => {
  assert.equal(renderType(users, false, "postgres"), "string");
  assert.equal(renderType({ kind: "array", of: users }, false, "postgres"), "string");
  assert.equal(renderParamType(users, "postgres"), "string");
});

test("inside a JSON document a composite is an object of its JSON-encoded fields", () => {
  assert.equal(renderType(users, true, "postgres"), inJson);
  // The renderer parenthesizes an element whose text holds a `|` anywhere, as it does for an object.
  assert.equal(renderType({ kind: "array", of: users }, true, "postgres"), `(${inJson} | null)[]`);
  // A constructor's fields are inside the document, at the top level too.
  const expected = `{ author: ${inJson}; posts: (${inJson} | null)[] | null; n: number }`;
  assert.equal(renderType(doc, false, "postgres"), expected);
  assert.equal(renderType(doc, true, "postgres"), expected);
});

test("one type per SQL type, on every driver: what the run time returns (src/values.ts)", () => {
  const wire = (name: string, dialect: "postgres" | "mysql" = "postgres") => renderType(scalar(name), false, dialect);
  assert.deepEqual(["bool", "int2", "int4", "oid", "float4", "float8"].map((n) => wire(n)), ["boolean", "number", "number", "number", "number", "number"]);
  assert.equal(wire("int8"), "bigint");
  assert.deepEqual(["numeric", "money", "date", "timestamp", "timestamptz", "time", "timetz", "interval", "uuid", "text"].map((n) => wire(n)), Array(10).fill("string"));
  assert.equal(wire("bytea"), "Uint8Array");
  assert.equal(renderType({ kind: "json" }), "unknown");
  // Every other type is its text: Reylo owns every oid, so no driver parses one.
  assert.deepEqual(["point", "xid", "int4range", "void", "record", "tsvector", "citext"].map((n) => wire(n)), Array(7).fill("string"));
  // An array of each, an enum's labels, a row value's array its text.
  assert.equal(renderType({ kind: "array", of: scalar("timestamptz") }), "(string | null)[]");
  assert.equal(renderType({ kind: "array", of: scalar("int8") }), "(bigint | null)[]");
  assert.equal(renderType({ kind: "array", of: { kind: "enum", name: "mood", labels: ["ok", "sad"] } }), `("ok" | "sad" | null)[]`);
  // MySQL's types are the same names: a BIGINT a BigInt, a DECIMAL its text, a DATETIME its text, a
  // BIT its bytes, and a truth value (a TINYINT(1), a BIT(1), a comparison) a boolean.
  assert.equal(wire("int8", "mysql"), "bigint");
  assert.deepEqual(["numeric", "timestamp", "date", "time", "text"].map((n) => wire(n, "mysql")), Array(5).fill("string"));
  assert.equal(wire("bytea", "mysql"), "Uint8Array");
  assert.equal(wire("bool", "mysql"), "boolean");
});

test("a scalar that names two types is either", () => {
  // What MySQL answers with by what is bound for a placeholder: a BIGINT is a BigInt, a DOUBLE a
  // number; an INT a temporary table may widen to a BIGINT is a BigInt either way.
  assert.equal(renderType(scalar("int8|float8"), false, "mysql"), "bigint | number");
  assert.equal(renderType(scalar("int8|int4"), false, "mysql"), "bigint");
  assert.equal(renderType(scalar("numeric|float8"), false, "mysql"), "string | number");
  assert.equal(renderType(scalar("date|timestamp"), false, "mysql"), "string");
  assert.equal(renderType({ kind: "array", of: scalar("numeric|float8") }, false, "mysql"), "(string | number | null)[]");
  // A truth value a document may hold as `true` or as 1.
  assert.equal(renderType(scalar("bool|int8"), true, "mysql"), "boolean | number");
  assert.equal(renderType({ kind: "list", of: scalar("bool|int8"), nullable_items: true }, false, "mysql"), "(boolean | number | null)[]");
  // A part no table spells is nothing known inside a document.
  assert.equal(renderType(scalar("int8|mystery"), true, "mysql"), "unknown");
});

test("a composite with no columns inside a document is an empty object", () => {
  assert.equal(renderType({ kind: "composite", name: "empty", fields: [] }, true), "Record<never, never>");
  assert.equal(renderType({ kind: "composite", name: "empty", fields: [] }, false), "string");
});

test("SQLite: an INTEGER is a number, exact or an error; a NUMERIC whichever the row stored; a column with no type unknown", () => {
  assert.equal(renderType(scalar("integer"), false, "sqlite"), "number");
  assert.equal(renderType(scalar("numeric"), false, "sqlite"), "number | string");
  assert.equal(renderType(scalar("real"), false, "sqlite"), "number");
  assert.equal(renderType(scalar("text"), false, "sqlite"), "string");
  assert.equal(renderType(scalar("blob"), false, "sqlite"), "Uint8Array");
  assert.equal(renderType(scalar("bool"), false, "sqlite"), "boolean");
  assert.equal(renderType({ kind: "unknown" }, false, "sqlite"), "unknown");
  // The driver binds a number for an INTEGER, and the run time a truth value as 1 or 0.
  assert.equal(renderParamType(scalar("integer"), "sqlite"), "number");
  assert.equal(renderParamType(scalar("numeric"), "sqlite"), "number | string | boolean");
});

// Each column type beside the type the engine gives a parameter compared with that column, as
// the engine reported them on 2026-10-20 against Postgres 18.6, MySQL 8.4.11 and SQLite for the
// driver matrix's `typed` fixture (`select c from typed` and `select id from typed where c = :v`).
// MySQL describes every integer context as a BIGINT and a BIT one as BIT, so the engine types a
// parameter by the column it is compared with (engine/mysql `column_types`), and keeps the
// prepare's (`int4`, `bool|bytea`) for an expression; SQLite types a parameter by the column's
// affinity, a BOOLEAN column's being NUMERIC.
const array = (of: EngineType): EngineType => ({ kind: "array", of });
const composite: EngineType = { kind: "composite", name: "user_in", fields: [{ name: "name", type: scalar("text"), nullable: true }] };
const ROUND_TRIPS: Record<"postgres" | "mysql" | "sqlite", [column: EngineType, param: EngineType][]> = {
  postgres: [
    ...["int2", "int4", "int8", "float4", "float8", "numeric", "bool", "text", "uuid", "interval", "time", "money", "date", "timestamp", "timestamptz", "bytea"].map((n): [EngineType, EngineType] => [scalar(n), scalar(n)]),
    [{ kind: "json" }, { kind: "json" }],
    [{ kind: "enum", name: "mood", labels: ["ok", "sad"] }, { kind: "enum", name: "mood", labels: ["ok", "sad"] }],
    ...["int4", "int8", "text", "bool", "timestamptz", "date", "bytea", "numeric"].map((n): [EngineType, EngineType] => [array(scalar(n)), array(scalar(n))]),
    [array({ kind: "enum", name: "mood", labels: ["ok", "sad"] }), array({ kind: "enum", name: "mood", labels: ["ok", "sad"] })],
    [array(composite), array(composite)],
    [array(scalar("record")), array(scalar("record"))],
    // An array the statement builds from values that may be NULL (`array_agg` over a left join)
    // goes back into a structured member alone: a top-level array parameter takes no NULL
    // element, tested below.
  ],
  mysql: [
    [scalar("int8"), scalar("int8")],
    [scalar("int4"), scalar("int4")],
    [scalar("int2"), scalar("int4")],
    [scalar("bool"), scalar("bool|int4")],
    [scalar("bool"), scalar("bool|bytea")],
    [scalar("bytea"), scalar("bool|bytea")],
    [scalar("bytea"), scalar("bytea")],
    ...["numeric", "float8", "text", "date", "timestamp", "time"].map((n): [EngineType, EngineType] => [scalar(n), scalar(n)]),
    [{ kind: "json" }, { kind: "json" }],
  ],
  sqlite: [
    ...["integer", "int8", "real", "text", "blob", "numeric"].map((n): [EngineType, EngineType] => [scalar(n), scalar(n)]),
    [scalar("bool"), scalar("numeric")],
    [{ kind: "unknown" }, { kind: "unknown" }],
  ],
};

/** An array type's element type, `(a | b)[]` as `a | b`; undefined for any other type. */
const element = (ts: string) => (ts.endsWith("[]") ? ts.slice(0, -2).replace(/^\((.*)\)$/, "$1") : undefined);

/**
 * Whether every value of the column's type is one the parameter's type takes, member by member of
 * the union, element by element of an array. An array's NULL elements are the one exception: a
 * column's may hold them, and a parameter's elements are strict, so an array goes back in without
 * them.
 */
function admits(column: EngineType, param: EngineType, dialect: "postgres" | "mysql" | "sqlite"): boolean {
  const returned = renderType(column, false, dialect);
  const taken = renderParamType(param, dialect);
  const members = (ts: string) => ts.split(" | ");
  const [returnedElement, takenElement] = [element(returned), element(taken)];
  if (returnedElement !== undefined && takenElement !== undefined) return members(returnedElement).filter((m) => m !== "null").every((m) => members(takenElement).includes(m) || members(takenElement).includes("unknown"));
  return members(taken).includes("unknown") || members(returned).every((member) => members(taken).includes(member));
}

test("a parameter takes every value run returns for the column it is compared with, on every dialect: the round-trip rule", () => {
  for (const [dialect, pairs] of Object.entries(ROUND_TRIPS) as ["postgres" | "mysql" | "sqlite", [EngineType, EngineType][]][]) {
    for (const [column, param] of pairs) {
      assert.ok(admits(column, param, dialect), `${dialect}: a column of ${JSON.stringify(column)} returns ${renderType(column, false, dialect)}, which a parameter of ${JSON.stringify(param)}, typed ${renderParamType(param, dialect)}, does not take`);
    }
  }
});

test("parameters take what every driver binds, whatever Reylo returns", () => {
  assert.equal(renderParamType(scalar("numeric"), "postgres"), "string");
  assert.equal(renderParamType(scalar("int8"), "postgres"), "number | bigint | string");
  assert.equal(renderParamType(scalar("blob"), "sqlite"), "Uint8Array");
  assert.equal(renderParamType(scalar("bytea"), "mysql"), "Uint8Array");
  // A JSON value, built by the statement or read from a column, is parsed.
  const built: EngineType = { kind: "object", fields: [{ name: "id", type: scalar("int4"), nullable: false }] };
  assert.equal(renderType(built, false, "mysql"), "{ id: number }");
  // Inside a JSON value the database wrote the bytes and the number already.
  const inner: EngineType = { kind: "object", fields: [{ name: "b", type: scalar("bytea"), nullable: false }, { name: "n", type: scalar("numeric"), nullable: false }] };
  assert.equal(renderType(inner, false, "postgres"), "{ b: string; n: number }");
});

test("every date and time parameter takes a string: a timestamptz one the text that names its zone, which run returns, and no Date", () => {
  assert.equal(renderParamType(scalar("timestamptz"), "postgres"), "string");
  assert.equal(renderParamType({ kind: "array", of: scalar("timestamptz") }, "postgres"), "string[]");
  for (const name of ["timestamp", "date"]) {
    assert.equal(renderParamType(scalar(name), "postgres"), "string");
    assert.equal(renderParamType(scalar(name), "mysql"), "string");
    assert.equal(renderParamType({ kind: "array", of: scalar(name) }, "postgres"), "string[]");
  }
  // What MySQL answers by what is bound for a placeholder compared with a DATE and a DATETIME.
  assert.equal(renderParamType(scalar("date|timestamp"), "mysql"), "string");
});

test("a returned array's elements may be NULL unless the statement shows they are not; a parameter's are strict", () => {
  // Postgres lets any array hold NULL elements, whatever a column's NOT NULL says: a table's array
  // column is `(T | null)[]`, and so is `array_agg(u.email)` over a LEFT JOIN, `{NULL}` for a team
  // with no users. An array the statement builds from values it shows are not NULL is `T[]`.
  const texts = (nullable_items?: boolean): EngineType => (nullable_items === undefined ? { kind: "array", of: scalar("text") } : { kind: "array", of: scalar("text"), nullable_items });
  assert.equal(renderType(texts()), "(string | null)[]");
  assert.equal(renderType(texts(false)), "string[]");
  assert.equal(renderType(texts(true)), "(string | null)[]");
  assert.equal(renderType(texts(true), true), "(string | null)[]");
  assert.equal(renderType({ kind: "array", of: { kind: "enum", name: "mood", labels: ["ok", "sad"] }, nullable_items: true }), `("ok" | "sad" | null)[]`);
  // A top-level array parameter's elements take no NULL, as the parameter itself takes NULL only
  // as the first argument of `coalesce`: a NULL in `= any(:ids)` matches nothing. An array inside a
  // structured parameter's member keeps NULL elements, so a built array goes back into a row.
  assert.equal(renderParamType(texts()), "string[]");
  assert.equal(renderParamType({ kind: "array", of: scalar("int8") }), "(number | bigint | string)[]");
  assert.equal(renderParamType({ kind: "array", of: scalar("int4") }), "number[]");
  const member: EngineType = { kind: "rows", one: true, fields: [{ name: "tags", type: texts(), written: "text[]", nullable: false, optional: false }] };
  assert.equal(renderParamType(member), "{ tags: (string | null)[] }");
});

test("a JSON column a CHECK holds to a JSON Schema is the type of the values the schema admits", () => {
  const json = (schema: unknown): EngineType => ({ kind: "json", schema: schema as never });
  const open = { type: "object", properties: { name: { type: "string" }, age: { type: ["integer", "null"] } }, required: ["name"], additionalProperties: true };
  assert.equal(renderType(json(open)), "{ name: string; age?: number | null; [key: string]: unknown }");
  assert.equal(renderType(json({ ...open, additionalProperties: false })), "{ name: string; age?: number | null }");
  assert.equal(renderType(json({ type: "object", additionalProperties: { type: "string" } })), "{ [key: string]: string }");
  assert.equal(renderType(json({ type: "object", additionalProperties: false })), "Record<never, never>");
  assert.equal(renderType(json({ type: ["object", "null"] })), "Record<string, unknown> | null");
  // A discriminated union, a literal object, a tuple with an optional element and a rest.
  const tagged = { anyOf: [
    { type: "object", properties: { kind: { enum: ["a"] }, x: { type: "number" } }, required: ["kind", "x"], additionalProperties: false },
    { type: "object", properties: { kind: { enum: ["b"] } }, required: ["kind"], additionalProperties: false },
  ] };
  assert.equal(renderType(json(tagged)), `{ kind: "a"; x: number } | { kind: "b" }`);
  assert.equal(renderType(json({ enum: [{ a: 1, "b-c": [true, null] }, "x", 2, null] })), `{ a: 1; "b-c": [true, null] } | "x" | 2 | null`);
  assert.equal(renderType(json({ type: "array", prefixItems: [{ type: "string" }, { type: ["number", "null"] }], items: { type: "boolean" }, minItems: 1 })), "[string, (number | null)?, ...boolean[]]");
  assert.equal(renderType(json({ type: "array", prefixItems: [{ type: "string" }], items: false, minItems: 1 })), "[string]");
  assert.equal(renderType(json({ type: "array", items: { type: ["string", "integer"] } })), "(string | number)[]");
  assert.equal(renderType(json({ type: "array", items: false })), "[]");
  assert.equal(renderType(json(false)), "never");
  assert.equal(renderType(json({})), "unknown");
  // Reylo parses JSON on every driver; inside a document, and as a parameter, the schema's type stands.
  assert.equal(renderType(json(open), true, "postgres"), "{ name: string; age?: number | null; [key: string]: unknown }");
  assert.equal(renderParamType(json(open), "postgres"), "{ name: string; age?: number | null; [key: string]: unknown }");
  assert.equal(renderType({ kind: "json" }), "unknown");
});
