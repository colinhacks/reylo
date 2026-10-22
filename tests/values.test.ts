import { test } from "node:test";
import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { execute, type QueryModule } from "../src/execute.ts";
import { renderParamType, renderType } from "../src/render.ts";
import { typeSchema } from "../src/schema.ts";
import { coerce, columnKind, DecodeError, instant, kindType, scalarKind, textDecoder } from "../src/values.ts";
import type { EngineType } from "../src/engine-type.ts";
import type { Columns } from "../src/values.ts";

// The one table (src/values.ts): the kind of each SQL type, the TypeScript type the module declares
// for it, the JSON Schema of its JSON form, and the value the run time gives it, which must agree.

const scalar = (name: string): EngineType => ({ kind: "scalar", name });
/** The TypeScript type a value is, as the declared type names it. */
function typeOf(v: unknown): string {
  if (v === null) return "null";
  // No value Reylo returns is one.
  if (v instanceof Date) return "Date";
  if (v instanceof Uint8Array) return "Uint8Array";
  if (Array.isArray(v)) return `${typeOf(v.find((x) => x !== null))}[]`;
  return typeof v;
}

// A Postgres value's text, as the server writes it, for every type whose kind is not its text.
const TEXTS: Record<string, string> = {
  bool: "t", int2: "2", int4: "4", oid: "26", float4: "1.1", float8: "2.2", int8: "9007199254740993", numeric: "1.100000000000000001",
  money: "$1.50", date: "2026-10-20", timestamp: "2026-10-20 10:00:00.123456", timestamptz: "2026-10-20 10:00:00.123456+02",
  time: "10:00:00", timetz: "10:00:00+02", interval: "1 day", json: '{"a":1}', jsonb: "[1,2]", bytea: "\\x0102ff", uuid: "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
  text: "hi", point: "(1,2)", int4range: "[1,5)", xid: "7", void: "",
};

test("every Postgres type's decoded value is the type the module declares for it, and its JSON form is its schema's", () => {
  for (const [name, text] of Object.entries(TEXTS)) {
    const t = scalar(name);
    const kind = columnKind(t, "postgres");
    const value = textDecoder(kind)(text);
    const declared = renderType(t, false, "postgres");
    if (declared !== "unknown") assert.equal(typeOf(value), declared, name);
    // The array of it, NULL elements null.
    const array: EngineType = { kind: "array", of: t };
    const values = textDecoder(columnKind(array, "postgres"))(`{${JSON.stringify(text)},NULL}`) as unknown[];
    assert.equal(values[1], null, name);
    if (declared !== "unknown") assert.equal(`${typeOf(values[0])}[]`, renderType(array, false, "postgres"), name);
    // The value written as JSON, a BigInt as the integer it is, has the schema's JSON type.
    const schema = typeSchema(t, "postgres") as { type?: string | string[] };
    if (schema.type !== undefined) {
      const written = JSON.stringify(value instanceof Uint8Array ? Buffer.from(value).toString("base64") : value, (_, x) => (typeof x === "bigint" ? (JSON as unknown as { rawJSON(text: string): unknown }).rawJSON(String(x)) : x));
      assert.equal(written, typeof value === "bigint" ? String(value) : written, name);
      const json = JSON.parse(written);
      const jsonType = typeof json === "number" ? (Number.isInteger(json) ? ["integer", "number"] : ["number"]) : [typeof json];
      assert.ok([schema.type].flat().some((t) => jsonType.includes(t)), `${name}: ${JSON.stringify(json)} against ${JSON.stringify(schema)}`);
    }
  }
});

test("the representation, value by value", () => {
  const d = (name: string, text: string) => textDecoder(scalarKind(name, "postgres"))(text);
  assert.equal(d("int8", "9007199254740993"), 9007199254740993n);
  assert.equal(d("int8", "-9223372036854775808"), -9223372036854775808n);
  assert.deepEqual(textDecoder(scalarKind("int8", "postgres") + "[]")("{1,NULL,9223372036854775807}"), [1n, null, 9223372036854775807n]);
  assert.equal(d("timestamptz", "2026-10-20 10:00:00.123456+02"), "2026-10-20T08:00:00.123456Z");
  assert.equal(d("numeric", "12345678901234567890.123456789"), "12345678901234567890.123456789");
  assert.equal(d("date", "2026-10-20"), "2026-10-20");
  assert.equal(d("timestamp", "2026-10-20 10:00:00.123456"), "2026-10-20T10:00:00.123456");
  assert.equal(d("timestamp", "0044-03-15 12:00:00 BC"), "0044-03-15T12:00:00 BC");
  assert.equal(d("timestamp", "infinity"), "infinity");
  assert.equal(d("interval", "1 year 2 mons"), "1 year 2 mons");
  assert.deepEqual(d("bytea", "\\x0102ff"), Buffer.from([1, 2, 255]));
  assert.deepEqual([...(d("bytea", "\\001\\\\x") as Uint8Array)], [1, 92, 120], "the escape format too");
  assert.deepEqual(d("jsonb", '{"big":9007199254740993}'), { big: 9007199254740992 });
  assert.equal(d("bool", "f"), false);
  assert.ok(Number.isNaN(d("float8", "NaN") as number));
  assert.equal(d("float8", "-Infinity"), -Infinity);
});

test("timestamptz: the instant's ISO 8601 text in UTC, the server's fraction kept, which new Date(text) gets wrong for BC, year 1 and offsets with seconds", () => {
  const Temporal = (globalThis as unknown as { Temporal: { Instant: { from(text: string): { toString(): string; epochNanoseconds: bigint } } } }).Temporal;
  for (const [text, iso] of [
    ["2026-10-20 03:03:00.123456-07", "2026-10-20T10:03:00.123456Z"],
    ["2026-10-20 10:00:00+00", "2026-10-20T10:00:00Z"],
    ["2026-10-20 10:00:00.5+05:30", "2026-10-20T04:30:00.5Z"],
    ["2026-12-31 23:30:00-01", "2027-01-01T00:30:00Z"],
    ["0001-01-01 00:00:00+00", "0001-01-01T00:00:00Z"],
    ["0001-01-01 00:30:00+01", "0000-12-31T23:30:00Z"],
    ["1883-11-18 12:00:00-07:52:58", "1883-11-18T19:52:58Z"],
    ["1900-01-01 00:00:00+00:00:36", "1899-12-31T23:59:24Z"],
    ["0001-12-31 23:59:59.999999+00 BC", "0000-12-31T23:59:59.999999Z"],
    ["0002-06-01 12:00:00+00 BC", "-000001-06-01T12:00:00Z"],
    ["0044-03-15 12:00:00+00 BC", "-000043-03-15T12:00:00Z"],
    ["4713-11-24 00:00:00+00 BC", "-004712-11-24T00:00:00Z"],
    ["9999-12-31 23:59:59.999999-01", "+010000-01-01T00:59:59.999999Z"],
    ["12345-01-01 00:00:00+05:30", "+012344-12-31T18:30:00Z"],
    ["2000-02-29 12:00:00+00", "2000-02-29T12:00:00Z"],
    ["1900-03-01 00:00:00+01", "1900-02-28T23:00:00Z"],
  ] as const) {
    assert.equal(instant(text), iso, text);
    // Temporal reads each back to the same text: an ISO 8601 instant, the year extended outside 0000 to 9999.
    assert.equal(Temporal.Instant.from(iso).toString(), iso, iso);
  }
  // Each to the nanosecond against Temporal's own reading of the server's offset (its microseconds kept).
  assert.equal(Temporal.Instant.from(instant("2026-10-20 03:03:00.123456-07")).epochNanoseconds, Temporal.Instant.from("2026-10-20T03:03:00.123456-07:00").epochNanoseconds);
  // Past Temporal's range (±10^8 days), which Postgres's reaches (294276 AD): still ISO 8601 text.
  assert.equal(instant("294276-12-31 23:59:59.999999+00"), "+294276-12-31T23:59:59.999999Z");
  assert.throws(() => Temporal.Instant.from("+294276-12-31T23:59:59.999999Z"), RangeError);
  assert.equal(instant("infinity"), "infinity");
  assert.equal(instant("-infinity"), "-infinity");
  // The failing control: what postgres.js and PGlite use, and what a Date holds.
  assert.equal(new Date("0001-01-01 00:00:00+00").getUTCFullYear(), 2001);
  assert.equal(new Date("2026-10-20T03:03:00.123456-07:00").toISOString(), "2026-10-20T10:03:00.123Z");
});

test("SQLite: the kinds, and a NUMERIC column of either", () => {
  assert.deepEqual(["integer", "real", "text", "blob", "numeric", "bool", "mystery"].map((n) => scalarKind(n, "sqlite")), ["int", "float", "text", "bytes", "int|float|text", "bool", "unknown"]);
  assert.deepEqual(["int", "int|float|text", "unknown", "bytes"].map(kindType), ["number", "number | string", "unknown", "Uint8Array"]);
});

test("SQLite: a column declared BIGINT, INT8 or UNSIGNED BIG INT (the engine's int8) is a BigInt, as on Postgres and MySQL, and its parameter takes one back", () => {
  assert.equal(scalarKind("int8", "sqlite"), "int8");
  assert.equal(renderType(scalar("int8"), false, "sqlite"), "bigint");
  assert.equal(renderType(scalar("int8"), true, "sqlite"), "number", "inside a JSON document, the number SQLite writes");
  assert.equal(renderParamType(scalar("int8"), "sqlite"), "number | bigint");
  assert.deepEqual(typeSchema(scalar("int8"), "sqlite"), { type: "integer", format: "int64" }, "the BigInt, as the JSON integer it is");
  assert.deepEqual(typeSchema(scalar("int8"), "sqlite"), typeSchema(scalar("int8"), "postgres"));
  assert.deepEqual(typeSchema(scalar("int8"), "sqlite", true), { type: "integer", format: "int64" }, "a parameter");
});

test("SQLite: a BIGINT column on node:sqlite exact past 2^53 beside an INTEGER read as a number, whatever the database's setting; on the drivers that round, a BigInt within 2^53 and a rounded value refused", async () => {
  for (const readBigInts of [false, true]) {
    const db = new DatabaseSync(":memory:", { readBigInts });
    db.exec("create table t (id integer primary key, n integer, big bigint); insert into t values (1, 5, 9007199254740993), (2, 6, -9007199254740993), (3, 7, null)");
    const q: QueryModule<"sqlite"> = { dialect: "sqlite", sql: "select id, n, big from t order by id", params: [], columns: [["id", "int"], ["n", "int"], ["big", "int8"]] };
    assert.deepEqual(execute(db, q), [{ id: 1, n: 5, big: 9007199254740993n }, { id: 2, n: 6, big: -9007199254740993n }, { id: 3, n: 7, big: null }]);
    // The failing control: the app's own statement at the database's setting, which without
    // readBigInts refuses the value with an error that names no column.
    const own = () => ({ ...(db.prepare("select n, big from t where id = 1").get() as object) });
    if (readBigInts) assert.deepEqual(own(), { n: 5n, big: 9007199254740993n });
    else assert.throws(own, /^RangeError: Value is too large to be represented as a JavaScript number: 9007199254740993/);
    db.close();
  }
  const q: QueryModule<"sqlite"> = { dialect: "sqlite", sql: "select n, big from t", params: [], columns: [["n", "int"], ["big", "int8"]] };
  const d1 = (results: unknown[]) => ({ batch: async () => [], prepare: () => ({ first: async () => null, bind: () => ({ all: async () => ({ results }) }) }) });
  assert.deepEqual(await execute(d1([{ n: 5, big: 9007199254740991 }, { n: 6, big: -3 }]), q), [{ n: 5, big: 9007199254740991n }, { n: 6, big: -3n }]);
  await assert.rejects(execute(d1([{ n: 5, big: 9007199254740992 }]), q) as Promise<unknown>, (e: Error) => e instanceof DecodeError && /^Column "big" is a 64-bit integer, and the driver already rounded its value to 9007199254740992/.test(e.message));
  // libSQL under each intMode: a number, a BigInt or the digits.
  const libsql = (rows: unknown[]) => ({ execute: async () => ({ rows }), executeMultiple: async () => {} });
  for (const big of [7, 7n, "7"]) assert.deepEqual(await execute(libsql([{ n: 5, big }]), q), [{ n: 5, big: 7n }], typeof big);
  assert.deepEqual(await execute(libsql([{ n: 5, big: "9007199254740993" }]), q), [{ n: 5, big: 9007199254740993n }]);
  // Bun.sql's SQLite, with safeIntegers or without.
  const bun = (rows: unknown[]) => ({ unsafe: async () => rows, commitDistributed: async () => {} });
  assert.deepEqual(await execute(bun([{ n: 5, big: 9007199254740993n }]), q), [{ n: 5, big: 9007199254740993n }]);
  await assert.rejects(execute(bun([{ n: 5, big: 9007199254740992 }]), q) as Promise<unknown>, /Column "big" is a 64-bit integer, and the driver already rounded/);
});

test("coerce: a value a driver decoded, given its kind; an integer a number cannot hold is refused, naming the column", () => {
  assert.equal(coerce("int8", 9007199254740993n, "c"), 9007199254740993n);
  assert.equal(coerce("int8", "9007199254740993", "c"), 9007199254740993n, "a driver's exact text");
  assert.equal(coerce("int8", 42, "c"), 42n);
  assert.throws(() => coerce("int8", 9007199254740992, "big_id"), (e: Error) => e instanceof DecodeError && /^Column "big_id" is a 64-bit integer, and the driver already rounded its value to 9007199254740992/.test(e.message));
  assert.deepEqual(coerce("int8[]", ["1", 2n], "c"), [1n, 2n]);
  assert.equal(coerce("decimal", 1.5, "c"), "1.5");
  assert.equal(coerce("date", new Date("2026-10-20T00:00:00Z"), "c"), "2026-10-20");
  // Bun.sql decodes a date or timestamp before 1 AD as an Invalid Date (its infinities are ±Infinity):
  // refused, never read as infinity.
  for (const kind of ["date", "timestamp", "timestamptz", "timestamp|timestamptz"]) {
    assert.throws(() => coerce(kind, new Date(Number.NaN), "born"), (e: Error) => e instanceof DecodeError && /^Column "born": the driver returned an Invalid Date/.test(e.message), kind);
  }
  assert.equal(coerce("timestamptz", Infinity, "c"), "infinity");
  assert.equal(coerce("timestamptz", -Infinity, "c"), "-infinity");
  assert.equal(coerce("timestamp", new Date("2026-10-20T10:00:00.500Z"), "c"), "2026-10-20T10:00:00.5");
  assert.equal(coerce("timestamptz", new Date("2026-10-20T10:00:00.500Z"), "c"), "2026-10-20T10:00:00.5Z", "Bun.sql's Date, to its millisecond");
  assert.equal(coerce("timestamptz", new Date("2026-10-20T10:00:00Z"), "c"), "2026-10-20T10:00:00Z");
  assert.equal(coerce("timestamptz", "2026-10-20 10:00:00.123456+02", "c"), "2026-10-20T08:00:00.123456Z");
  assert.equal(coerce("timestamptz", -Infinity, "c"), "-infinity");
  assert.deepEqual(coerce("int[]", Int32Array.of(1, 2), "c"), [1, 2]);
  assert.deepEqual(coerce("text[]", "{a,NULL}", "c"), ["a", null]);
  assert.equal(coerce("text", ["x", "z"], "c"), "x,z", "mariadb's SET");
  assert.deepEqual(coerce("bytes", true, "c"), Uint8Array.of(1), "Bun.sql's and mariadb's BIT(1) where it is no boolean (a view's)");
  assert.deepEqual(coerce("bytes", [1, 2], "c"), Uint8Array.of(1, 2), "D1's BLOB");
  assert.deepEqual(coerce("bytes", Uint8Array.of(3).buffer, "c"), Uint8Array.of(3), "libSQL's BLOB");
  assert.deepEqual(coerce("json", '{"a":1}', "c"), { a: 1 });
  assert.equal(coerce("int", 9007199254740991n, "c"), 9007199254740991);
  // The bounds, exactly: ±(2^53 - 1) is a number, ±2^53 refused.
  assert.deepEqual([9007199254740991n, -9007199254740991n, 0n].map((v) => coerce("int", v, "n")), [9007199254740991, -9007199254740991, 0]);
  for (const v of [9007199254740992n, -9007199254740992n, 2n ** 63n - 1n, -(2n ** 63n)]) assert.throws(() => coerce("int", v, "n"), DecodeError, String(v));
  assert.throws(() => coerce("int", 9007199254740993n, "big_id"), (e: Error) => e instanceof DecodeError && /^Column "big_id" holds 9007199254740993, an integer past ±\(2\^53 - 1\)/.test(e.message));
  assert.throws(() => coerce("int", 9007199254740992, "big_id"), /Column "big_id" holds an integer past ±\(2\^53 - 1\), which the driver already rounded to 9007199254740992/);
  // A NUMERIC may hold a REAL past 2^53, which is no rounded integer.
  assert.equal(coerce("int|float|text", 1e20, "c"), 1e20);
  // A MySQL either: a BIGINT a BigInt, a DOUBLE a number, a DECIMAL its text.
  assert.equal(coerce("int8|float|decimal", 1n, "c"), 1n);
  assert.equal(coerce("int8|float|decimal", 2.5, "c"), 2.5);
  assert.equal(coerce("int8|float|decimal", "1.10", "c"), "1.10");
});

test("a truth value: 0 or 1 in every form a driver gives it, and any other value refused, naming the column", () => {
  for (const v of [true, 1, 1n, "1", "t", "true", Uint8Array.of(1)]) assert.equal(coerce("bool", v, "c"), true, String(v));
  for (const v of [false, 0, 0n, "0", "f", "false", Uint8Array.of(0)]) assert.equal(coerce("bool", v, "c"), false, String(v));
  // The failing control: a MySQL TINYINT(1) or a SQLite BOOLEAN holds any integer the database lets in.
  for (const [v, shown] of [[2, "2"], [-1, "-1"], [255n, "255"], ["yes", '"yes"'], [0.5, "0.5"], [Uint8Array.of(2), "the bytes 0x02"]] as const) {
    assert.throws(() => coerce("bool", v, "active"), (e: Error) => e instanceof DecodeError && e.message.startsWith(`Column "active" is a boolean, stored as 0 or 1, and holds ${shown}, which is neither`), String(v));
  }
});

test("the kinds: a BIGINT is a BigInt, and an integer a temporary table may widen to one is too; a timestamptz a string; no kind a Date", () => {
  assert.deepEqual(["int8", "int8|int4", "int4|int8", "int8|float8|numeric", "int4|int8|float8|numeric", "bool", "timestamptz"].map((n) => scalarKind(n, "mysql")), ["int8", "int8", "int8", "int8|float|decimal", "int|int8|float|decimal", "bool", "timestamptz"]);
  assert.deepEqual(["int8", "int8|int4", "int8|float8|numeric", "bool", "timestamptz", "date"].map((n) => kindType(scalarKind(n, "mysql"))), ["bigint", "bigint", "bigint | number | string", "boolean", "string", "string"]);
  assert.equal(renderType({ kind: "array", of: scalar("int8") }), "bigint[]");
  assert.equal(renderType(scalar("int8"), true), "number", "inside a JSON document, as the database writes it");
  assert.deepEqual(typeSchema(scalar("int8"), "postgres"), { type: "integer", format: "int64" });
  assert.deepEqual(typeSchema(scalar("int8"), "postgres", true), { type: ["integer", "string"], format: "int64" }, "a parameter, as every driver binds it");
  assert.deepEqual(typeSchema(scalar("timestamptz"), "postgres"), { type: "string", format: "date-time" });
  assert.deepEqual(typeSchema(scalar("bool"), "mysql"), { type: "boolean" });
  assert.deepEqual(typeSchema(scalar("bool"), "sqlite"), { type: "boolean" });
  assert.deepEqual(typeSchema(scalar("integer"), "sqlite"), { type: "integer", format: "int64" });
  assert.deepEqual(typeSchema(scalar("numeric"), "sqlite"), { type: ["number", "string"] });
  for (const kind of ["bool", "int", "float", "int8", "decimal", "text", "date", "timestamp", "timestamptz", "json", "bytes", "unknown"]) assert.notEqual(kindType(kind), "Date", kind);
});

test("SQLite: Reylo's own statement per database and text, with integers read as BigInts whatever the database's setting, and the app's statements unchanged", () => {
  const db = new DatabaseSync(":memory:", { readBigInts: true, returnArrays: true });
  db.exec("create table t (id integer primary key, n integer, r real, b blob, doc text, flag integer, x); insert into t values (1, 5, 1.5, x'0102', '{\"a\":1}', 1, 7), (2, 9007199254740993, null, null, null, 0, null), (3, 1, null, null, null, 2, null)");
  const q: QueryModule<"sqlite"> = { dialect: "sqlite", sql: "select id, n, r, b, doc, flag, x from t where id = :id", params: ["id"], columns: [["id", "int"], ["n", "int"], ["r", "float"], ["b", "bytes"], ["doc", "json"], ["flag", "bool"], ["x", "unknown"]] };
  const [row] = execute(db, q, { id: 1 }) as Record<string, unknown>[];
  // A plain object, as on every other driver: node:sqlite's own rows have a null prototype.
  assert.deepEqual(row, { id: 1, n: 5, r: 1.5, b: Uint8Array.of(1, 2), doc: { a: 1 }, flag: true, x: 7 });
  // The failing control: the app's own statement keeps its database's settings.
  assert.deepEqual(db.prepare("select n from t where id = 1").get(), [5n]);
  assert.throws(() => execute(db, q, { id: 2 }), (e: Error) => e instanceof DecodeError && e instanceof TypeError && /^Column "n" holds 9007199254740993/.test(e.message));
  // A boolean column holding 2, which SQLite lets in: refused, naming it.
  assert.throws(() => execute(db, q, { id: 3 }), /^TypeError: Column "flag" is a boolean, stored as 0 or 1, and holds 2/);
  // A query defined by hand: integers exact, nothing else known.
  const [bare] = execute(db, { dialect: "sqlite", sql: "select n, doc from t where id = 1", params: [] }) as Record<string, unknown>[];
  assert.deepEqual(bare, { n: 5, doc: '{"a":1}' });
});

test("SQLite: the asynchronous drivers' rows normalized: libSQL's BigInt, string and ArrayBuffer, D1's array, a rounded integer refused", async () => {
  const q: QueryModule<"sqlite"> = { dialect: "sqlite", sql: "select n, b, doc from t", params: [], columns: [["n", "int"], ["b", "bytes"], ["doc", "json"]] };
  const libsql = (rows: unknown[]) => ({ execute: async () => ({ rows }), executeMultiple: async () => {} });
  assert.deepEqual(await execute(libsql([{ n: 5n, b: Uint8Array.of(1).buffer, doc: "[1]" }]), q), [{ n: 5, b: Uint8Array.of(1), doc: [1] }]);
  assert.deepEqual(await execute(libsql([{ n: "5", b: null, doc: null }]), q), [{ n: 5, b: null, doc: null }], "intMode: string");
  const d1 = { batch: async () => [], prepare: () => ({ first: async () => null, bind: () => ({ all: async () => ({ results: [{ n: 9007199254740992, b: [1, 2], doc: "{}" }] }) }) }) };
  await assert.rejects(execute(d1, q) as Promise<unknown>, /Column "n" holds an integer past .* already rounded/);
  const ok = { ...d1, prepare: () => ({ first: async () => null, bind: () => ({ all: async () => ({ results: [{ n: 1, b: [1, 2], doc: "{}" }] }) }) }) };
  assert.deepEqual(await execute(ok, q), [{ n: 1, b: Uint8Array.of(1, 2), doc: {} }]);
});

test("mysql2: per-query options and Reylo's typeCast, never decimalNumbers; a DECIMAL and JSON as their text, a date's dropped fraction and its T, TIME's digits", async () => {
  let sent: Record<string, any> = {};
  const field = (type: string, text: string | null) => ({ type, string: () => text, buffer: () => (text === null ? null : Buffer.from(text)) });
  const pool = {
    query: async () => [[], []],
    // mysql2 calls the typeCast once per value with the field and its own reading, and resolves to [rows, fields].
    execute: async (options: Record<string, any>) => {
      sent = options;
      const cast = options.typeCast as (f: unknown, next: () => unknown) => unknown;
      const row = {
        amount: cast(field("NEWDECIMAL", "1.100000000000000001"), () => 1.1),
        doc: cast(field("JSON", '{"big":9007199254740993}'), () => ({ big: "9007199254740993" })),
        at: cast(field("DATETIME", null), () => "2026-03-08 09:31:00"),
        at6: cast(field("DATETIME", null), () => "2026-03-08 09:31:00.123456"),
        latest: cast(field("DATETIME", null), () => "2026-03-08 09:31:00"),
        t: cast(field("TIME", null), () => "10:00:00.5"),
        geo: cast(field("GEOMETRY", "wkb"), () => ({ x: 1, y: 2 })),
        big: cast(field("LONGLONG", null), () => "9007199254740993"),
      };
      const fields = [["amount", 246, 18], ["doc", 245, 0], ["at", 12, 3], ["at6", 12, 6], ["latest", 12, 0], ["t", 11, 2], ["geo", 255, 0], ["big", 8, 0]].map(([name, columnType, decimals]) => ({ name, columnType, decimals }));
      return [[row], fields];
    },
  };
  const [row] = (await execute(pool as never, { dialect: "mysql", sql: "select 1", params: [] })) as Record<string, unknown>[];
  // A BIGINT's exact text a BigInt, by the result's own column type (LONGLONG).
  assert.deepEqual(row, { amount: "1.100000000000000001", doc: { big: 9007199254740992 }, at: "2026-03-08T09:31:00.000", at6: "2026-03-08T09:31:00.123456", latest: "2026-03-08T09:31:00", t: "10:00:00.50", geo: Buffer.from("wkb"), big: 9007199254740993n });
  assert.equal("decimalNumbers" in sent, false, "a per-query decimalNumbers poisons mysql2's parser cache for every pool");
  assert.equal("jsonStrings" in sent, false);
});

test("mariadb: every option per query, metaAsArray off, a SET joined and a DATETIME's T; a GEOMETRY read as bytes only where a column may hold one", async () => {
  let sent: Record<string, any> = {};
  const meta = [{ name: () => "tags", type: "SET", isSet: () => true }, { name: () => "at", type: "DATETIME", isSet: () => false }, { name: () => "id", type: "INT", isSet: () => false }];
  const conn = { batch: async () => ({}), query: async (options: Record<string, any>) => ((sent = options), Object.assign([{ tags: ["x", "z"], at: "2026-03-08 09:31:00.000", id: 1 }], { meta })) };
  assert.deepEqual([...((await execute(conn as never, { dialect: "mysql", sql: "select 1", params: [] })) as unknown[])], [{ tags: "x,z", at: "2026-03-08T09:31:00.000", id: 1 }]);
  assert.equal(sent.metaAsArray, false);
  // A BIGINT mariadb's own BigInt, a DECIMAL its text, on mariadb 3 and mariadb 2 (supportBigInt, bigNumberStrings).
  assert.deepEqual([sent.bigIntAsNumber, sent.supportBigNumbers, sent.decimalAsNumber, sent.supportBigInt, sent.bigNumberStrings], [false, false, false, true, true]);
  assert.equal(sent.typeCast, undefined, "an app's typeCast is switched off");
  await execute(conn as never, { dialect: "mysql", sql: "select 1", params: [], columns: [["shape", "unknown"]] });
  assert.equal(typeof sent.typeCast, "function");
});

test("PlanetScale: Reylo's cast per query, the driver's own default with a DATETIME's T, over the app's", async () => {
  let cast: (field: { type: string; charset?: number }, value: string | null) => unknown = () => null;
  const conn = { execute: async (_: string, __?: unknown, options?: { cast: typeof cast }) => ((cast = options!.cast), { rows: [], statement: "" }) };
  await execute(conn, { dialect: "mysql", sql: "select 1", params: [] });
  const bytes = (s: string) => String.fromCharCode(...Buffer.from(s, "utf8"));
  assert.deepEqual(
    [cast({ type: "INT64" }, "9007199254740993"), cast({ type: "DATETIME" }, "2026-03-08 09:31:00.000"), cast({ type: "DECIMAL" }, "1.10"), cast({ type: "JSON" }, bytes('{"é":1}')), cast({ type: "INT32" }, "7"), cast({ type: "FLOAT64" }, "2.2"), cast({ type: "VARBINARY", charset: 63 }, "\u0001ÿ"), cast({ type: "VARCHAR", charset: 255 }, bytes("é")), cast({ type: "INT64" }, null)],
    [9007199254740993n, "2026-03-08T09:31:00.000", "1.10", { é: 1 }, 7, 2.2, Uint8Array.of(1, 255), "é", null],
  );
});

test("mariadb's callback API, whose rows carry no meta, and mariadb 2, whose columns have no isSet(): the module's kinds join a SET and give a DATETIME its T", async () => {
  const conn = {
    batch: () => ({}),
    query: (_: unknown, __: unknown, done: (e: Error | null, rows?: unknown, meta?: unknown) => void) => (setImmediate(() => done(null, [{ tags: ["x", "z"], at: "2026-03-08 09:31:00.000", id: 1 }], [{ name: () => "tags", type: "SET" }])), { on() {} }),
  };
  const q: QueryModule<"mysql"> = { dialect: "mysql", sql: "select 1", params: [], columns: [["tags", "text"], ["at", "timestamp"], ["id", "int"]] };
  assert.deepEqual(await execute(conn as never, q), [{ tags: "x,z", at: "2026-03-08T09:31:00.000", id: 1 }]);
  // A query defined by hand: the callback's own meta.
  assert.deepEqual(await execute(conn as never, { ...q, columns: undefined }), [{ tags: "x,z", at: "2026-03-08 09:31:00.000", id: 1 }]);
});

test("runSync refuses a client whose prepared statement answers through a promise, which run awaits", async () => {
  const db = { prepare: () => ({ all: async () => [{ n: 1 }] }) };
  const q: QueryModule<"sqlite"> = { dialect: "sqlite", sql: "select 1 as n", params: [], columns: [["n", "int"]] };
  assert.deepEqual(await execute(db as never, q), [{ n: 1 }]);
  const { executeSync } = await import("../src/execute.ts");
  assert.throws(() => executeSync(db as never, { ...q, sql: "select 2 as n" }), /answered with a promise/);
});

test("mysql2: Reylo's typeCast only where it is needed, since mysql2 calls it for every value: a JSON or unknown column, unknown columns, or the app's typeCast, decimalNumbers or jsonStrings", async () => {
  const sent: unknown[] = [];
  const pool = (connectionConfig: Record<string, unknown> | undefined) => ({ pool: connectionConfig && { config: { connectionConfig } }, query: async () => [[], []], execute: async (options: { typeCast: unknown }) => (sent.push(typeof options.typeCast === "function" ? "cast" : options.typeCast), [[], []]) });
  const q = (columns?: [string, string][]): QueryModule<"mysql"> => ({ dialect: "mysql", sql: "select 1", params: [], ...(columns ? { columns } : {}) });
  await execute(pool({}) as never, q([["n", "int8"], ["amount", "decimal"], ["at", "timestamp"]]));
  await execute(pool({}) as never, q([["doc", "json"]]));
  await execute(pool({}) as never, q([["shape", "unknown"]]));
  await execute(pool({}) as never, q());
  await execute(pool({ decimalNumbers: true }) as never, q([["amount", "decimal"]]));
  await execute(pool({ jsonStrings: true }) as never, q([["n", "int"]]));
  await execute(pool({ typeCast: () => null }) as never, q([["n", "int"]]));
  await execute(pool(undefined) as never, q([["n", "int"]]));
  assert.deepEqual(sent, [true, "cast", "cast", "cast", "cast", "cast", "cast", "cast"]);
});

test("MySQL: a truth value is a boolean on every driver, whatever form it gives it in, and a BIGINT a BigInt; a TINYINT(1) holding 2 refused", async () => {
  const columns: Columns = [["done", "bool"], ["same", "bool"], ["one_bit", "bool"], ["n", "int8"], ["widened", "int8"], ["either", "int8|float|decimal"]];
  const q: QueryModule<"mysql"> = { dialect: "mysql", sql: "select 1", params: [], columns };
  const expected = [{ done: true, same: false, one_bit: true, n: 9007199254740993n, widened: 7n, either: 2.5 }];
  // mysql2: a TINYINT(1) a number, a comparison a BIGINT's text (bigNumberStrings), a BIT(1) a Buffer, an INT a temporary table may widen a number.
  const mysql2Row = (done: unknown) => ({ done, same: "0", one_bit: Buffer.of(1), n: "9007199254740993", widened: 7, either: 2.5 });
  const fields = [["done", 1], ["same", 8], ["one_bit", 16], ["n", 8], ["widened", 3], ["either", 5]].map(([name, columnType]) => ({ name, columnType, decimals: 0 }));
  const mysql2 = (done: unknown) => ({ query: async () => [[], []], execute: async () => [[mysql2Row(done)], fields] });
  assert.deepEqual(await execute(mysql2(1) as never, q), expected);
  await assert.rejects(execute(mysql2(2) as never, q) as Promise<unknown>, /^TypeError: Column "done" is a boolean, stored as 0 or 1, and holds 2, which is neither/);
  // A query defined by hand: a BIGINT by the result's own type, a TINYINT(1) as it is.
  assert.deepEqual(await execute(mysql2(2) as never, { ...q, columns: undefined }), [{ ...mysql2Row(2), same: 0n, n: 9007199254740993n }]);
  // mariadb: a BIGINT its own BigInt, a BIT(1) a Buffer (bitOneIsBoolean off).
  const mariadb = (done: unknown) => ({ batch: async () => ({}), query: async () => [{ done, same: 0n, one_bit: Buffer.of(1), n: 9007199254740993n, widened: 7, either: 2.5 }] });
  assert.deepEqual([...((await execute(mariadb(1) as never, q)) as unknown[])], expected);
  await assert.rejects(execute(mariadb(-1) as never, q) as Promise<unknown>, /Column "done" is a boolean, stored as 0 or 1, and holds -1/);
  // PlanetScale: Reylo's cast, then the kinds.
  const planetscale = (done: string) => ({
    execute: async (_: string, __: unknown, options: { cast: (f: { type: string; charset?: number }, v: string | null) => unknown }) => ({
      statement: "",
      rows: [{ done: options.cast({ type: "INT8" }, done), same: options.cast({ type: "INT64" }, "0"), one_bit: options.cast({ type: "BIT", charset: 63 }, "\u0001"), n: options.cast({ type: "INT64" }, "9007199254740993"), widened: options.cast({ type: "INT32" }, "7"), either: options.cast({ type: "FLOAT64" }, "2.5") }],
    }),
  });
  assert.deepEqual(await execute(planetscale("1") as never, q), expected);
  await assert.rejects(execute(planetscale("2") as never, q) as Promise<unknown>, /Column "done" is a boolean/);
  // Bun.sql: a BIT(1) a boolean, a BIGINT its digits.
  const bun = (done: unknown) => ({ unsafe: async () => [{ done, same: "0", one_bit: true, n: "9007199254740993", widened: 7, either: 2.5 }], commitDistributed: async () => {} });
  assert.deepEqual(await execute(bun(1) as never, q), expected);
  await assert.rejects(execute(bun(5) as never, q) as Promise<unknown>, /Column "done" is a boolean, stored as 0 or 1, and holds 5/);
});

test("MySQL: a VECTOR is an array of its floats on every driver that reads one, and its parameter binds their text", async () => {
  const vector: EngineType = { kind: "array", of: scalar("float4"), nullable_items: false };
  assert.equal(columnKind(vector, "mysql"), "float[]");
  assert.equal(renderType(vector), "number[]");
  assert.equal(renderParamType(vector, "mysql"), "number[]");
  // The server sends a VECTOR as its floats' bytes, 32-bit and little-endian (MySQL 9.7.2).
  const floats = [1, 2.5, -3, 0.10000000149011612];
  const bytes = Buffer.alloc(16);
  floats.forEach((f, i) => bytes.writeFloatLE(f, i * 4));
  const columns: Columns = [["v", "float[]"], ["vn", "float[]"]];
  const q: QueryModule<"mysql"> = { dialect: "mysql", sql: "select 1", params: [], columns };
  const expected = [{ v: floats, vn: null }];
  // mysql2 3.x decodes the floats itself; mariadb and PlanetScale give the bytes.
  const fields = [{ name: "v", columnType: 242, decimals: 0 }, { name: "vn", columnType: 242, decimals: 0 }];
  assert.deepEqual(await execute({ query: async () => [[], []], execute: async () => [[{ v: floats, vn: null }], fields] } as never, q), expected);
  assert.deepEqual(await execute({ query: async () => [[], []], execute: async () => [[{ v: Buffer.from(bytes), vn: null }], fields] } as never, q), expected);
  assert.deepEqual([...((await execute({ batch: async () => ({}), query: async () => [{ v: Buffer.from(bytes), vn: null }] } as never, q)) as unknown[])], expected);
  const planetscale = {
    execute: async (_: string, __: unknown, options: { cast: (f: { type: string; charset?: number }, v: string | null) => unknown }) => ({
      rows: [{ v: options.cast({ type: "VECTOR", charset: 63 }, String.fromCharCode(...bytes)), vn: options.cast({ type: "VECTOR", charset: 63 }, null) }],
    }),
  };
  assert.deepEqual(await execute(planetscale as never, q), expected);
  // A slice of a larger buffer, as a driver's row may be, reads from its own offset.
  assert.deepEqual(coerce("float[]", Buffer.concat([Buffer.of(9, 9, 9), bytes]).subarray(3), "v"), floats);
  assert.throws(() => coerce("float[]", bytes.subarray(0, 5), "v"), (e: Error) => e instanceof DecodeError && /Column "v" is a VECTOR, whose value is 4 bytes per 32-bit float, and holds 5 bytes/.test(e.message));
  // A Postgres float4[] from Bun.sql is a Float32Array, which is not read as bytes.
  assert.deepEqual(coerce("float[]", new Float32Array([1, 2.5]), "v"), [1, 2.5]);

  // A parameter binds the text of its floats, which the engine's `to_vector(?)` reads, on every driver.
  const w: QueryModule<"mysql"> = { dialect: "mysql", sql: "insert into items (v) values (to_vector(:v))", params: ["v"], inputs: { v: { type: vector } } };
  const seen: unknown[][] = [];
  await execute({ query: async () => [[], []] as [unknown, unknown], execute: async (_: unknown, v?: unknown[]) => (seen.push(v!), [[], []] as [unknown, unknown]) } as never, w, { v: floats });
  await execute({ query: async (_: string, v: unknown) => (seen.push(v as unknown[]), []), batch: async () => [] } as never, w, { v: floats });
  await execute({ unsafe: async (_: string, v?: unknown[]) => (seen.push(v!), []), commitDistributed: async () => {} } as never, w, { v: floats });
  await execute({ execute: async (_: string, v: unknown[]) => (seen.push(v), { rows: [] }) } as never, w, { v: floats });
  await execute({ batch: async () => ({}), query: async (_: string, v: unknown) => (seen.push(v as unknown[]), []) } as never, w, { v: null });
  assert.deepEqual(seen, [...Array(4).fill(["[1,2.5,-3,0.10000000149011612]"]), [null]]);
  const fake = { query: async () => [[], []], execute: async () => [[], []] } as never;
  await assert.rejects(execute(fake, w, { v: [1, "2"] }) as Promise<unknown>, /^TypeError: Parameter "v" is a VECTOR, an array of finite numbers, and the element at \[1\] is "2"\.$/);
  await assert.rejects(execute(fake, w, { v: [1, NaN] }) as Promise<unknown>, /the element at \[1\] is NaN/);
  await assert.rejects(execute(fake, w, { v: [Infinity] }) as Promise<unknown>, /the element at \[0\] is Infinity/);
  await assert.rejects(execute(fake, w, { v: [1, null] }) as Promise<unknown>, /^TypeError: Parameter "v" is an array whose elements may not be null, and the element at \[1\] is/);
});
