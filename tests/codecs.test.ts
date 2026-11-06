import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { Session } from "../src/analyze.ts";
import { sqlModule } from "../src/loader.ts";
import { analysisWithBases } from "../src/engine-type.ts";
import { DRIVERS, pgLive, postgresDatabase } from "./helpers.ts";
import { array, field, lite, my, pg, record } from "./descriptors.ts";
import { decodeCodec, decodeCodecs, decodePlan, encodeCodec, encodePlan, leafCodec, planOf, type Codecs } from "../src/codecs.ts";
import { execute, executeSync, type QueryModule } from "../src/execute.ts";
import { defineQuery } from "../src/index.ts";
import { moduleTypes, standardColumnsType } from "../src/module-types.ts";
import { renderParamType, renderType } from "../src/render.ts";
import { querySchemas } from "../src/schema.ts";
import { DecodeError, ParameterError } from "../src/values.ts";
import type { EngineType } from "../src/engine-type.ts";

// The project file's `js.codecs` (src/codecs.ts): which value each family reaches on each database,
// at every position Reylo decodes, the type the module declares for it, and the value the run time
// gives it, which must agree. The tests run under nub, which gives Node 24 a global Temporal
// (tests/values.test.ts reads it too); the error a project meets on a runtime without one is
// checked by hiding it.
const Temporal: any = (globalThis as any).Temporal;

/** A SQLite column of NUMERIC affinity whose table declares it `declared`, as the engine writes it: whatever storage class a row stored, under the declared name. */
const declared = (name: string): EngineType => ({ ...(lite("numeric") as Extract<EngineType, { kind: "union" }>), name });
const duck = (name: string): EngineType => (({
  bigint: { kind: "integer", bits: 64 }, hugeint: { kind: "integer", bits: 128 }, decimal: { kind: "decimal", precision: 18, min_scale: 3, max_scale: 3 },
  timestamptz: { kind: "instant", tick: "1/1000000" }, timestamp_ns: { kind: "datetime", tick: "1/1000000000" }, date: { kind: "date" }, time: { kind: "time", tick: "1/1000000" },
  interval: { kind: "duration", max: "P2147483648M2147483648DT2562047788H54.775808S", tick: "1/1000000" }, blob: { kind: "bytes" }, varchar: { kind: "text" },
}) as Record<string, EngineType>)[name]!;
/** DuckDB's UNION, as the engine writes it: a tagged union of its alternatives, each value of which may be null. */
const tagged = (...alternatives: [string, EngineType][]): EngineType => ({ kind: "tagged_union", name: "union", alternatives: alternatives.map(([tag, type]) => ({ tag, type, nullable: true })) });
const every: Codecs = { decimal: "number", bytes: "string", date: "Temporal.PlainDate", time: "Temporal.PlainTime", datetime: "Temporal.PlainDateTime", instant: "Temporal.Instant", duration: "Temporal.Duration", set: "Array" };

test("each family reaches the values that hold its kind, on every database, and nothing else", () => {
  const onPg = (t: EngineType) => leafCodec(t, "postgres", every);
  assert.deepEqual(["numeric", "bytea", "date", "time", "timestamp", "timestamptz", "interval"].map((n) => onPg(pg(n))), ["decimal:number", "bytes:string", "date", "time", "datetime", "instant", "duration"]);
  // A decimal is a number whatever its scale: 1.5 and 1.50 are one value.
  assert.equal(onPg({ ...pg("numeric"), precision: 10, min_scale: 2, max_scale: 2 } as EngineType), "decimal:number");
  // What no codec reaches: a 64-bit integer, whose one exact type is a bigint, a time with its
  // offset, text, the narrower integers, JSON, a MAC address (bytes Reylo gives as their text);
  // and an array is its elements' (planOf, below).
  for (const name of ["int8", "timetz", "text", "uuid", "int4", "float8", "bool", "jsonb", "money", "macaddr"]) assert.equal(onPg(pg(name)), undefined, name);
  assert.equal(onPg(array(pg("timestamptz"))), undefined);
  // An instant that keeps an offset per value would lose it in a Temporal.Instant.
  assert.equal(onPg({ ...pg("timestamptz"), zone: "offset" } as EngineType), undefined);
  // MySQL's TIMESTAMP reaches the client as the session's wall-clock time; its TIME is a duration,
  // which runs past 24 hours and below zero; its BOOLEAN is a tinyint(1); its SET is a Set.
  const onMy = (t: EngineType) => leafCodec(t, "mysql", every);
  assert.deepEqual(["bigint", "decimal", "datetime", "timestamp", "date", "blob", "time", "set", "tinyint(1)", "int"].map((n) => onMy(my(n))), [undefined, "decimal:number", "datetime", "datetime", "date", "bytes:string", "duration", "set:array", undefined, undefined]);
  // SQLite has no date type: a column's declaration says what its text is.
  const onLite = (t: EngineType) => leafCodec(t, "sqlite", every);
  assert.deepEqual(["DATE", "datetime", "TIMESTAMP", "TIME", "DATETIME(6)"].map((n) => onLite(declared(n))), ["date", "datetime", "datetime", "time", "datetime"]);
  assert.deepEqual([onLite(lite("numeric")), onLite(lite("bool")), onLite(lite("int8")), onLite(lite("blob")), onLite(lite("integer"))], [undefined, undefined, undefined, "bytes:string", undefined]);
  const onDuck = (n: string) => leafCodec(duck(n), "duckdb", every);
  assert.deepEqual(["bigint", "hugeint", "decimal", "timestamptz", "timestamp_ns", "date", "time", "interval", "blob"].map(onDuck), [undefined, undefined, "decimal:number", "instant", "datetime", "date", "time", "duration", "bytes:string"]);
  // A family left out, or set to its default, gives no codec.
  assert.equal(leafCodec(pg("bytea"), "postgres", {}), undefined);
  assert.equal(leafCodec(pg("timestamptz"), "postgres", { instant: "string" }), undefined);
  assert.equal(leafCodec(pg("bytea"), "postgres", { bytes: "Uint8Array" }), undefined);
  assert.equal(leafCodec(my("set"), "mysql", { set: "Set" }), undefined);
  assert.equal(leafCodec(pg("numeric"), "postgres", { decimal: "number" }), "decimal:number");
});

test("a plan follows every part of a value Reylo decodes, and stops at JSON", () => {
  assert.deepEqual(planOf(array(pg("timestamptz")), "postgres", every), { array: "instant" });
  assert.deepEqual(planOf(array(array(pg("numeric"))), "postgres", every), { array: { array: "decimal:number" } });
  // A DuckDB struct's fields, a map's keys and values, a tagged union's value.
  const struct = record([field("at", duck("timestamptz")), field("name", duck("varchar")), field("n", duck("decimal"))]);
  assert.deepEqual(planOf(struct, "duckdb", every), { fields: { at: "instant", n: "decimal:number" } });
  assert.deepEqual(planOf({ kind: "map", key: duck("date"), value: { type: duck("varchar"), nullable: true } }, "duckdb", every), { entries: { key: "date" } });
  assert.deepEqual(planOf(tagged(["at", duck("timestamptz")], ["s", duck("varchar")]), "duckdb", every), { tags: { at: "instant" } });
  assert.deepEqual(planOf(array(struct), "duckdb", every), { array: { fields: { at: "instant", n: "decimal:number" } } });
  // A structured parameter's members are fields too.
  assert.deepEqual(planOf(array(record([field("at", pg("timestamptz")), field("name", pg("text"))])), "postgres", every, true), { array: { fields: { at: "instant" } } });
  // A Postgres row value's fields, as a DuckDB struct's.
  assert.deepEqual(planOf(array(record([field("at", pg("timestamptz"), true), field("n", pg("int4"))], "public.event")), "postgres", every), { array: { fields: { at: "instant" } } });
  // JSON is the database's, and a value with no codec in it has no plan.
  assert.equal(planOf({ kind: "json", content: { type: pg("timestamptz") } }, "postgres", every), undefined);
  assert.equal(planOf(pg("point"), "postgres", every), undefined);
  assert.equal(planOf(struct, "duckdb", {}), undefined);
  assert.equal(planOf(array(pg("text")), "postgres", every), undefined);
});

test("a codec position is declared its codec's type, through every part Reylo decodes, and a parameter takes it alone", () => {
  assert.equal(renderType(pg("timestamptz"), false, "postgres", every), "Temporal.Instant");
  assert.equal(renderType(array(pg("timestamptz")), false, "postgres", every), "(Temporal.Instant | null)[]");
  assert.equal(renderType(array(pg("numeric"), false), false, "postgres", every), "number[]");
  assert.equal(renderType(record([field("at", duck("timestamptz")), field("n", duck("decimal"))]), false, "duckdb", every), "{ at: Temporal.Instant | null; n: number | null }");
  // A SET's array is of its members' type.
  assert.equal(renderType(my("set"), false, "mysql", every), "string[]");
  assert.equal(renderType({ ...my("set"), element: { type: { kind: "text", enum: ["a", "b"] } } } as EngineType, false, "mysql", every), '("a" | "b")[]');
  assert.equal(renderType(tagged(["at", duck("timestamptz")], ["s", duck("varchar")]), false, "duckdb", every), '{ tag: "at"; value: Temporal.Instant | null } | { tag: "s"; value: string | null }');
  // Inside JSON the database writes the value, which keeps JSON's type.
  assert.equal(renderType({ kind: "json", content: { type: pg("timestamptz") } }, false, "postgres", every), "string");
  assert.equal(renderParamType(my("set"), "mysql", false, undefined, every), "string[]");
  assert.equal(renderParamType(array(pg("date")), "postgres", false, undefined, every), "Temporal.PlainDate[]");
  assert.equal(renderParamType(pg("numeric"), "postgres", false, undefined, every), "number");
  assert.equal(renderParamType(my("time"), "mysql", false, undefined, every), "Temporal.Duration");
  // The module's declared row and parameters.
  const analysis = {
    kind: "select",
    params: [{ name: "since", type: pg("timestamptz") }],
    columns: [
      { name: "id", type: pg("int8") },
      { name: "price", type: { ...pg("numeric"), precision: 10, min_scale: 2, max_scale: 2 } },
      { name: "total", type: pg("numeric"), nullable: true },
      { name: "at", type: pg("timestamptz") },
      { name: "took", type: pg("interval") },
    ],
    cardinality: "many",
    diagnostics: [],
    rewritten: { sql: "", order: ["since"], spans: [] },
  } as any;
  const types = moduleTypes("select …", analysis, "postgres", { params: ["since"], placeholders: [] }, true, ".reylo.yaml", every);
  assert.match(types.row, /id: bigint;[\s\S]*price: number;[\s\S]*total: number \| null;[\s\S]*at: Temporal\.Instant;[\s\S]*took: Temporal\.Duration/);
  assert.match(types.params, /since: Temporal\.Instant/);
  const plain = moduleTypes("select …", analysis, "postgres", { params: ["since"], placeholders: [] }, true);
  assert.match(plain.row, /id: bigint;[\s\S]*price: string;[\s\S]*at: string/);
  // The JSON Schema names the JSON form the codec's value writes.
  const schemas = querySchemas(analysis, "postgres", every);
  const row = (schemas.output as any).items.properties;
  assert.deepEqual([row.id, row.price, row.total, row.took], [{ type: "integer", format: "int64" }, { type: "number" }, { type: ["number", "null"] }, { type: "string", description: "an ISO 8601 duration with its sign in front (P1Y2M3DT4H5M6.789S, -P1D)" }]);
});

test("a codec gives each value its type exactly, or refuses it naming the column and the value", () => {
  // A SET's members as an array, in the order the column declares them.
  assert.deepEqual(decodeCodec("set:array", new Set(["a", "c"]), "tags"), ["a", "c"]);
  // A decimal is a number where its digits survive as one, NaN and the infinities included.
  assert.equal(decodeCodec("decimal:number", "19.50", "price"), 19.5);
  assert.equal(decodeCodec("decimal:number", "-0.0000001", "price"), -1e-7);
  assert.ok(Number.isNaN(decodeCodec("decimal:number", "NaN", "price") as number));
  assert.equal(decodeCodec("decimal:number", "-Infinity", "price"), -Infinity);
  for (const v of ["12345678901234567890.12", "0.1000000000000000000001", "1e400"]) assert.throws(() => decodeCodec("decimal:number", v, "price"), (e: Error) => e instanceof DecodeError && e.message.startsWith(`Column "price" holds ${v}, which a number cannot hold exactly`), v);
  // The Temporal classes, from the text src/values.ts writes; a year past 9999 or before 1 AD too.
  const instant = decodeCodec("instant", "2026-10-20T08:00:00.123456Z", "at");
  assert.ok(instant instanceof Temporal.Instant);
  assert.equal(String(instant), "2026-10-20T08:00:00.123456Z");
  assert.equal(String(decodeCodec("datetime", "2026-10-20T10:00:00.5", "at")), "2026-10-20T10:00:00.5");
  assert.equal(String(decodeCodec("datetime", "2026-10-20 10:00:00", "at")), "2026-10-20T10:00:00");
  assert.equal(String(decodeCodec("datetime", "0044-03-15T12:00:00 BC", "at")), "-000043-03-15T12:00:00");
  assert.equal(String(decodeCodec("date", "10000-01-01", "day")), "+010000-01-01");
  assert.equal(String(decodeCodec("time", "23:59:59.999999", "t")), "23:59:59.999999");
  // A duration from Postgres's and DuckDB's ISO 8601 text, and from MySQL's TIME.
  assert.equal(String(decodeCodec("duration", "P1Y2M3DT4H5M6.789S", "took")), "P1Y2M3DT4H5M6.789S");
  assert.equal(String(decodeCodec("duration", "-P1D", "took")), "-P1D");
  assert.equal(String(decodeCodec("duration", "838:59:59", "took")), "PT838H59M59S");
  assert.equal(String(decodeCodec("duration", "-12:00:00.500000", "took")), "-PT12H0.5S");
  assert.equal(decodeCodec("date", ["2026-10-20", null], "days")!.toString(), "2026-10-20,");
  // What no Temporal value is: Postgres's infinities, 24:00:00 and an interval whose parts differ
  // in sign, MySQL's zero date, a SQLite date column holding a number.
  const refused = (codec: Parameters<typeof decodeCodec>[0], v: unknown, shown: string, cls: string) =>
    assert.throws(() => decodeCodec(codec, v, "c"), (e: Error) => e instanceof DecodeError && e.message.startsWith(`Column "c" holds ${shown}, which no Temporal.${cls} holds`), shown);
  refused("instant", "infinity", `"infinity"`, "Instant");
  refused("date", "-infinity", `"-infinity"`, "PlainDate");
  refused("time", "24:00:00", `"24:00:00"`, "PlainTime");
  refused("datetime", "0000-00-00 00:00:00", `"0000-00-00 00:00:00"`, "PlainDateTime");
  refused("duration", "P1M-1D", `"P1M-1D"`, "Duration");
  refused("date", 1729425600, "1729425600", "PlainDate");
  // Temporal's own parsers take more than each codec's text and change it without an error (a leap
  // second as :59, an offset dropped, a date and time as its date), which a SQLite column can hold.
  for (const [codec, v, cls] of [["time", "23:59:60", "PlainTime"], ["time", "08:00:00+02:00", "PlainTime"], ["datetime", "2016-12-31T23:59:60", "PlainDateTime"], ["datetime", "2016-12-31", "PlainDateTime"], ["datetime", "2016-12-31T10:00[Europe/Paris]", "PlainDateTime"], ["date", "2016-12-31T10:00", "PlainDate"], ["date", "20161231", "PlainDate"], ["instant", "2016-12-31T23:59:60Z", "Instant"]] as const) refused(codec, v, JSON.stringify(v), cls);
  assert.equal(String(decodeCodec("datetime", "2016-12-31 10:00", "c")), "2016-12-31T10:00:00");
  assert.equal(String(decodeCodec("instant", "12345-01-01T00:00:00Z", "c")), "+012345-01-01T00:00:00Z");
  // Binary data as its base64.
  assert.equal(decodeCodec("bytes:string", Uint8Array.of(1, 2, 255), "data"), "AQL/");
  assert.equal(decodeCodec("instant", null, "at"), null);
  // A runtime with no Temporal is told which setting needs it.
  const saved = (globalThis as any).Temporal;
  delete (globalThis as any).Temporal;
  try {
    assert.throws(() => decodeCodec("instant", "2026-10-20T08:00:00Z", "at"), /Column "at" is a Temporal.Instant by js.codecs.instant in the project file, and this JavaScript runtime has no global Temporal/);
  } finally {
    (globalThis as any).Temporal = saved;
  }
});

test("a plan gives a codec's type inside a struct, a map and a tagged union, and leaves the rest", () => {
  const value = { s: { at: "2026-10-20T08:00:00Z", name: "a" }, m: [{ key: "2026-10-20", value: "x" }], u: { tag: "at", value: "2026-10-20T08:00:00Z" }, v: { tag: "s", value: "y" } };
  decodePlan({ fields: { at: "instant" } }, value.s, "s");
  assert.ok(value.s.at as unknown instanceof Temporal.Instant);
  assert.equal(value.s.name, "a");
  decodePlan({ entries: { key: "date" } }, value.m, "m");
  assert.ok(value.m[0]!.key as unknown instanceof Temporal.PlainDate);
  decodePlan({ tags: { at: "instant" } }, value.u, "u");
  assert.ok(value.u.value as unknown instanceof Temporal.Instant);
  decodePlan({ tags: { at: "instant" } }, value.v, "v");
  assert.equal(value.v.value, "y");
  // A Postgres array of any depth: the plan of its elements reaches every level.
  assert.deepEqual(decodePlan({ array: "decimal:number" }, [["1.5", "2"], ["3", null]], "grid"), [[1.5, 2], [3, null]]);
  // A parameter's: the codec's type written as the driver binds it, the value itself untouched.
  const at = Temporal.Instant.from("2026-10-20T08:00:00Z");
  const row = { at, name: "a" };
  assert.deepEqual(encodePlan({ array: { fields: { at: "instant" } } }, [row], "rows", "postgres"), [{ at: "2026-10-20T08:00:00Z", name: "a" }]);
  assert.equal(row.at, at);
  assert.deepEqual(encodePlan({ tags: { at: "instant" } }, { tag: "at", value: at }, "u", "duckdb"), { tag: "at", value: "2026-10-20T08:00:00Z" });
  assert.deepEqual(encodePlan({ entries: { value: "date" } }, [{ key: "k", value: Temporal.PlainDate.from("2026-10-20") }], "m", "duckdb"), [{ key: "k", value: "2026-10-20" }]);
});

test("a codec parameter takes its codec's type alone, bound as the text or bytes its type reads", () => {
  // A SET's array binds as the Set the run time writes as MySQL's text.
  assert.deepEqual(encodeCodec("set:array", ["a", "c"], "tags", "mysql"), new Set(["a", "c"]));
  assert.equal(encodeCodec("decimal:number", 19.5, "price", "postgres"), "19.5");
  assert.equal(encodeCodec("instant", Temporal.Instant.from("2026-10-20T08:00:00.123456Z"), "at", "postgres"), "2026-10-20T08:00:00.123456Z");
  assert.equal(encodeCodec("datetime", Temporal.PlainDateTime.from("2026-10-20T10:00:00"), "at", "postgres"), "2026-10-20T10:00:00");
  // SQLite's date and time functions write a space before the time, and its text compares as text.
  assert.equal(encodeCodec("datetime", Temporal.PlainDateTime.from("2026-10-20T10:00:00"), "at", "sqlite"), "2026-10-20 10:00:00");
  assert.deepEqual(encodeCodec("date", [Temporal.PlainDate.from("2026-10-20")], "days", "postgres"), ["2026-10-20"]);
  assert.deepEqual(encodeCodec("bytes:string", "AQL/", "data", "postgres"), Uint8Array.of(1, 2, 255));
  // A duration as ISO 8601, which Postgres and DuckDB read, and as MySQL's TIME, in hours.
  assert.equal(encodeCodec("duration", Temporal.Duration.from("P1DT2H"), "took", "postgres"), "P1DT2H");
  assert.equal(encodeCodec("duration", Temporal.Duration.from("P1DT2H3M4.5S"), "took", "mysql"), "26:03:04.5");
  assert.equal(encodeCodec("duration", Temporal.Duration.from("-PT12H"), "took", "mysql"), "-12:00:00");
  // Temporal keeps each field as it was given, and MySQL reads the total.
  assert.equal(encodeCodec("duration", Temporal.Duration.from({ minutes: 90 }), "took", "mysql"), "1:30:00");
  assert.equal(encodeCodec("duration", Temporal.Duration.from({ milliseconds: 1500 }), "took", "mysql"), "0:00:01.5");
  assert.equal(encodeCodec("duration", Temporal.Duration.from({ seconds: 1, microseconds: 5 }), "took", "mysql"), "0:00:01.000005");
  assert.equal(encodeCodec("duration", Temporal.Duration.from({ hours: 838, minutes: 59, seconds: 59 }), "took", "mysql"), "838:59:59");
  const refused = (codec: Parameters<typeof encodeCodec>[0], v: unknown, expected: RegExp, dialect: "postgres" | "mysql" = "postgres") => assert.throws(() => encodeCodec(codec, v, "p", dialect), (e: Error) => e instanceof ParameterError && expected.test(e.message));
  refused("instant", "2026-10-20T08:00:00Z", /^Parameter "p" takes a Temporal.Instant, as js.codecs.instant in the project file says; got "2026-10-20T08:00:00Z"\.$/);
  refused("date", Temporal.PlainDateTime.from("2026-10-20T10:00:00"), /takes a Temporal.PlainDate/);
  refused("set:array", new Set(["a"]), /^Parameter "p" takes an array of the SET's members, as js.codecs.set in the project file says; got an object\.$/, "mysql");
  refused("set:array", [1], /takes an array of the SET's members/, "mysql");
  refused("decimal:number", "19.50", /takes a number, as js.codecs.decimal in the project file says/);
  refused("bytes:string", "not base64!", /takes binary data as its base64/);
  refused("duration", Temporal.Duration.from("P1M"), /takes a Temporal.Duration a MySQL TIME holds, of days and less and within ±838:59:59/, "mysql");
  // MySQL clips a TIME past its range without an error.
  refused("duration", Temporal.Duration.from({ hours: 839 }), /within ±838:59:59/, "mysql");
  refused("duration", Temporal.Duration.from({ hours: -838, minutes: -59, seconds: -59, milliseconds: -1 }), /within ±838:59:59/, "mysql");
  // A part of a second Postgres and MySQL would round away, which DuckDB's nanosecond types keep.
  const fine = Temporal.Instant.from("2026-10-20T08:00:00.123456789Z");
  refused("instant", fine, /^Parameter "p" holds 2026-10-20T08:00:00\.123456789Z, a Temporal\.Instant past the microsecond, which Postgres would round\. Round it first/);
  refused("datetime", Temporal.PlainDateTime.from("2026-10-20T10:00:00.0000001"), /past the microsecond, which MySQL would round/, "mysql");
  refused("duration", Temporal.Duration.from("PT0.0000005S"), /a Temporal\.Duration past the microsecond/);
  assert.equal(encodeCodec("instant", fine, "at", "duckdb"), "2026-10-20T08:00:00.123456789Z");
  assert.equal(encodeCodec("time", Temporal.PlainTime.from("10:00:00.123456"), "t", "postgres"), "10:00:00.123456");
});

test("run gives a codec's columns their type and binds its parameters, on a real driver", () => {
  const db = new DatabaseSync(":memory:");
  db.exec("create table events (id bigint primary key, at datetime not null, data blob)");
  db.exec("insert into events values (9007199254740993, '2026-10-20 10:00:00', x'0102ff')");
  const get: QueryModule<"sqlite"> = {
    dialect: "sqlite",
    sql: "select id, at, data from events where id = :id",
    params: ["id"],
    inputs: { id: { type: lite("int8"), nullable: false } },
    columns: [["id", "int8"], ["at", "int|float|text"], ["data", "bytes"]],
    codecs: { at: "datetime", data: "bytes:string" },
  };
  const rows = executeSync(db, get, { id: 9007199254740993n }) as any[];
  assert.equal(rows.length, 1);
  assert.equal(rows[0].id, 9007199254740993n);
  assert.ok(rows[0].at instanceof Temporal.PlainDateTime);
  assert.equal(String(rows[0].at), "2026-10-20T10:00:00");
  assert.equal(rows[0].data, "AQL/");
  // A row goes back in as it came out.
  const put: QueryModule<"sqlite"> = {
    dialect: "sqlite",
    sql: "insert into events (id, at, data) values (:id, :at, :data)",
    params: ["id", "at", "data"],
    inputs: { id: { type: lite("int8") }, at: { type: declared("DATETIME"), codec: "datetime" }, data: { type: lite("blob"), codec: "bytes:string" } },
    columns: [],
  };
  executeSync(db, put, { id: 1n, at: rows[0].at, data: rows[0].data });
  assert.deepEqual({ ...(db.prepare("select at, hex(data) as h from events where id = 1").get() as object) }, { at: "2026-10-20 10:00:00", h: "0102FF" });
  // A column declared as a date that holds anything but its text is refused.
  db.exec("insert into events values (2, 1729425600, null)");
  assert.throws(() => executeSync(db, get, { id: 2n }), (e: Error) => e instanceof DecodeError && /^Column "at" holds 1729425600, which no Temporal.PlainDateTime holds/.test(e.message));
  // run's path, which answers with a promise on any other client, gives the same values.
  assert.equal(String((execute(db, get, { id: 9007199254740993n }) as any[])[0].at), "2026-10-20T10:00:00");
});

test("a Standard Query gives a codec's columns the same type as run, by decode and by each column's decoder", () => {
  const q = defineQuery<{}, { at: unknown; n: unknown }, "many", "postgres">({ dialect: "postgres", sql: "select at, n from t", params: [], cardinality: "many", columns: [["at", "timestamptz"], ["n", "decimal"]], codecs: { at: "instant", n: "decimal:number" } })["~standard"].query;
  const [row] = q.decode([["2026-10-20 10:00:00+02", "19.50"]]) as { at: unknown; n: unknown }[];
  assert.ok(row!.at instanceof Temporal.Instant);
  assert.equal(String(row!.at), "2026-10-20T08:00:00Z");
  assert.equal(row!.n, 19.5);
  assert.ok(q.columns![0]!.decode("2026-10-20 10:00:00+02") instanceof Temporal.Instant);
  assert.equal(q.columns![1]!.decode("19.50"), 19.5);
  assert.equal(standardColumnsType([{ name: "at", type: pg("timestamptz"), nullable: false }] as any, "postgres", every), `readonly [{ readonly name: "at"; decode(value: string): Temporal.Instant }]`);
});

test("decodeCodecs leaves the columns it names no plan for as they are", () => {
  const rows = [{ n: "1.5", name: "a" }, { n: null, name: "b" }];
  decodeCodecs(rows, { n: "decimal:number" });
  assert.deepEqual(rows, [{ n: 1.5, name: "a" }, { n: null, name: "b" }]);
});

test("a project's js.codecs reach the module a build writes and the types it declares", async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-codecs-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".reylo.yaml"), "direct_url: file:./app.db\njs:\n  codecs:\n    datetime: Temporal.PlainDateTime\n    bytes: string\n");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table events (id bigint primary key, at datetime not null, note text, data blob)");
  db.close();
  const sql = "select id, at, note, data from events where id = :id";
  const file = join(root, "get_event.sql");
  writeFileSync(file, sql);
  try {
    const session = await Session.for(root);
    assert.deepEqual(session.project.codecs, { datetime: "Temporal.PlainDateTime", bytes: "string" });
    const outcome = await session.analyze(sql);
    assert.ok(outcome.ok);
    // A column declared as a date is one by its declaration, which the engine writes as its name.
    assert.deepEqual(outcome.analysis.columns.map((c) => planOf(c.type, "sqlite", session.project.codecs) ?? null), [null, "datetime", null, "bytes:string"]);
    const types = moduleTypes(sql, outcome.analysis, "sqlite", await session.lex(sql), true, ".reylo.yaml", session.project.codecs);
    assert.match(types.row, /id: bigint;[\s\S]*at: Temporal\.PlainDateTime;[\s\S]*note: string \| null;[\s\S]*data: string \| null/);
    assert.match(types.params, /id: number \| bigint/);
    const { code } = await sqlModule(file, sql);
    assert.match(code, /, codecs: \{"at":"datetime","data":"bytes:string"\}/);
    // The JSON Schema names the JSON form a codec's value writes.
    assert.match(code, /"at":\{"type":"string","description":"a date and time without a time zone/);
  } finally {
    for (const session of Session.opened()) await session.close();
  }
});

test("a Postgres analysis gives each column of a family its codec, and none to a time with its offset", { skip: pgLive }, async () => {
  const own = await postgresDatabase("reylo_codecs", "create table items (id int8 primary key, price numeric(10,2) not null, ratio numeric, at timestamptz, t time, tz timetz, took interval, data bytea, tags int8[]); create domain moment as timestamptz; create table logs (at moment);");
  const root = mkdtempSync(join(tmpdir(), "reylo-codecs-pg-"));
  writeFileSync(join(root, "package.json"), "{}");
  mkdirSync(join(root, "node_modules"));
  symlinkSync(join(DRIVERS, "postgres"), join(root, "node_modules", "postgres"));
  const session = await Session.open(root, { DATABASE_URL: own.url });
  const plans = async (sql: string) => {
    const outcome = await session.analyze(sql);
    assert.ok(outcome.ok, sql);
    return analysisWithBases(outcome.analysis).columns.map((c) => planOf(c.type, "postgres", every) ?? null);
  };
  try {
    assert.deepEqual(await plans("select id, price, ratio, at, t, tz, took, data, tags, price * 2 as doubled from items"), [null, "decimal:number", "decimal:number", "instant", "time", null, "duration", "bytes:string", null, "decimal:number"]);
    // A domain is decoded as the type it is made from.
    assert.deepEqual(await plans("select at from logs"), ["instant"]);
  } finally {
    await session.close();
    await own.drop();
  }
});
