import { test } from "node:test";
import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { execute, type QueryModule } from "../src/execute.ts";
import { renderParamType, renderType } from "../src/render.ts";
import { typeSchema } from "../src/schema.ts";
import { coerce, columnKind, DecodeError, duration, hstore, hstoreText, instant, kindType, numbersInJson, parseRow, postgresInterval, postgresLiteral, postgresYear, rowMasks, scalarKind, textDecoder } from "../src/values.ts";
import type { EngineType } from "../src/engine-type.ts";
import { array, field, lite, my, oneOf, pg, record } from "./descriptors.ts";
import type { Columns } from "../src/values.ts";

// The one table (src/values.ts): the kind of each SQL type, the TypeScript type the module declares
// for it, the JSON Schema of its JSON form, and the value the run time gives it, which must agree.

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
  text: "hi", xid: "7", void: "",
};

test("every Postgres type's decoded value is the type the module declares for it, and its JSON form is its schema's", () => {
  for (const [name, text] of Object.entries(TEXTS)) {
    const t = pg(name);
    const kind = columnKind(t, "postgres");
    const value = textDecoder(kind)(text);
    const declared = renderType(t, false, "postgres");
    // A JSON value is any JSON, `Json`: what JSON.parse gives back.
    if (declared === "Json") assert.deepEqual(JSON.parse(JSON.stringify(value)), value, name);
    else if (declared !== "unknown") assert.equal(typeOf(value), declared, name);
    // The array of it, NULL elements null.
    const values = textDecoder(columnKind(array(t), "postgres"))(`{${JSON.stringify(text)},NULL}`) as unknown[];
    assert.equal(values[1], null, name);
    if (declared === "Json") assert.equal(renderType(array(t), false, "postgres"), "Json[]", name);
    else if (declared !== "unknown") assert.equal(`(${typeOf(values[0])} | null)[]`, renderType(array(t), false, "postgres"), name);
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

// Postgres's geometric types, each as the server writes it and as the object `.run()` gives it.
const SHAPES: [name: string, text: string, value: unknown][] = [
  ["point", "(1.5,-2)", { x: 1.5, y: -2 }],
  ["line", "{1,-1,0}", { a: 1, b: -1, c: 0 }],
  ["lseg", "[(0,0),(1,1e+300)]", [{ x: 0, y: 0 }, { x: 1, y: 1e300 }]],
  ["box", "(3,4),(1,2)", { high: { x: 3, y: 4 }, low: { x: 1, y: 2 } }],
  ["path", "[(0,0),(1,1),(2,0)]", { closed: false, points: [{ x: 0, y: 0 }, { x: 1, y: 1 }, { x: 2, y: 0 }] }],
  ["path", "((0,0),(1,1))", { closed: true, points: [{ x: 0, y: 0 }, { x: 1, y: 1 }] }],
  ["polygon", "((0,0),(1,1),(2,0))", [{ x: 0, y: 0 }, { x: 1, y: 1 }, { x: 2, y: 0 }]],
  ["circle", "<(1,2),Infinity>", { center: { x: 1, y: 2 }, radius: Infinity }],
];

test("a geometric type is an object of its fields, declared and decoded alike, and goes back as the text it came from", () => {
  for (const [name, text, value] of SHAPES) {
    const t = pg(name);
    const kind = columnKind(t, "postgres");
    assert.equal(kind, name);
    assert.deepEqual(textDecoder(kind)(text), value, name);
    assert.deepEqual(coerce(kind, text, "c"), value, `${name}: from the text a driver gave`);
    assert.equal(postgresLiteral(t, value, (m) => assert.fail(m)), text.replace("1e+300", "1e+300"), `${name}: back as its text`);
    // The type the module declares, the parameter's and the JSON Schema of the JSON form all say the same object.
    const declared = renderType(t, false, "postgres");
    assert.equal(renderParamType(t, "postgres"), declared, name);
    assert.equal(declared, kindType(kind), name);
    assert.equal((typeSchema(t, "postgres") as { type: string }).type, Array.isArray(value) ? "array" : "object", name);
  }
  assert.equal(renderType(pg("point"), false, "postgres"), "{ x: number; y: number }");
  assert.equal(renderType(pg("circle"), false, "postgres"), "{ center: { x: number; y: number }; radius: number }");
  assert.deepEqual(typeSchema(pg("point"), "postgres"), { type: "object", properties: { x: { type: "number", format: "double" }, y: { type: "number", format: "double" } }, required: ["x", "y"] });
  // Inside a document Postgres writes a geometric type as its text.
  assert.equal(renderType(pg("box"), true, "postgres"), "string");
  // An array of boxes is written with a semicolon between them, since a box's own text holds commas.
  assert.deepEqual(textDecoder("box[]", ";")("{(3,4),(1,2);(1,1),(0,0)}"), [{ high: { x: 3, y: 4 }, low: { x: 1, y: 2 } }, { high: { x: 1, y: 1 }, low: { x: 0, y: 0 } }]);
  assert.deepEqual(textDecoder(columnKind(array(pg("lseg")), "postgres"))('{"[(0,0),(1,1)]",NULL}'), [[{ x: 0, y: 0 }, { x: 1, y: 1 }], null]);
  // Anything but the type's own object is refused, naming what it takes.
  for (const [name, v] of [["point", "(1,2)"], ["point", { x: 1 }], ["point", { x: 1, y: "2" }], ["point", { x: 1, y: 2, z: 3 }], ["lseg", [{ x: 0, y: 0 }]], ["path", { closed: 1, points: [{ x: 0, y: 0 }] }], ["polygon", []]] as [string, unknown][]) {
    assert.throws(() => postgresLiteral(pg(name), v, (m) => { throw new TypeError(m); }), new RegExp(`^TypeError: a ${name}, `), JSON.stringify(v));
  }
});

test("a Postgres row value is an object of its fields, each decoded and bound as a column of its type is", () => {
  const address = record([field("street", pg("text"), true), field("zip", pg("int4"), true), field("tags", array(pg("text")), true), field("at", pg("timestamptz"), true), field("spot", pg("point"), true), field("boxes", array(pg("box")), true)], "public.address");
  const kind = columnKind(address, "postgres");
  assert.equal(kind, '(["street","text"],["zip","int"],["tags","text[]"],["at","timestamptz"],["spot","point"],["boxes","box[]",";"])');
  // record_out's quoting: a field with a comma, a quote, a parenthesis or a space is quoted, a quote inside doubled, an empty field NULL.
  const text = '("1 ""Main"", St",12345,"{a,""b c""}","2026-10-20 01:00:00-07","(1,2)",)';
  assert.deepEqual(parseRow(text, 6), ['1 "Main", St', "12345", '{a,"b c"}', "2026-10-20 01:00:00-07", "(1,2)", null]);
  const value = { street: '1 "Main", St', zip: 12345, tags: ["a", "b c"], at: "2026-10-20T08:00:00.000000Z", spot: { x: 1, y: 2 }, boxes: null };
  assert.deepEqual(textDecoder(kind)(text), value);
  assert.deepEqual(parseRow(String.raw`(,"","\\",a\,b,"x""y")`, 5), [null, "", "\\", "a,b", 'x"y']);
  assert.deepEqual(parseRow("()", 1), [null]);
  assert.deepEqual(textDecoder(`${kind}[]`)(`{${JSON.stringify(text)},NULL}`), [value, null]);
  assert.equal(renderType(address, false, "postgres"), "{ street: string | null; zip: number | null; tags: (string | null)[] | null; at: string | null; spot: { x: number; y: number } | null; boxes: ({ high: { x: number; y: number }; low: { x: number; y: number } } | null)[] | null }");
  // Back as the text record_in reads: every field quoted, NULL empty, and a field left out NULL.
  const literal = postgresLiteral(address, { ...value, tags: ["a", null], boxes: [{ high: { x: 3, y: 4 }, low: { x: 1, y: 2 } }] }, (m) => assert.fail(m));
  assert.equal(literal, String.raw`("1 \"Main\", St","12345","{\"a\",NULL}","2026-10-20T08:00:00.000000Z","(1,2)","{\"(3,4),(1,2)\"}")`);
  assert.equal(postgresLiteral(address, { zip: 1 }, (m) => assert.fail(m)), '(,"1",,,,)');
  const refuse = (v: unknown, expected: RegExp) => assert.throws(() => postgresLiteral(address, v, (m) => { throw new TypeError(m); }), expected);
  refuse("(x,1)", /^TypeError: public\.address, an object of its fields \(street, zip, tags, at, spot, boxes\); got "\(x,1\)"\.$/);
  refuse({ stret: "x" }, /^TypeError: public\.address, which has no field "stret"; its fields are street, zip, tags, at, spot, boxes\.$/);
  refuse({ spot: "(1,2)" }, /^TypeError: public\.address, whose field "spot" is a point, \{ x: number; y: number \}; got "\(1,2\)"\.$/);
});

test("a range is an object of its bounds and their brackets, each bound in its subtype's form, and goes back as the text it came from", () => {
  const cases: [name: string, text: string, value: unknown, literal: string][] = [
    ["int4range", "[1,5)", { lower: 1, upper: 5, bounds: "[)" }, '["1","5")'],
    ["numrange", "(1.5,5]", { lower: "1.5", upper: "5", bounds: "(]" }, '("1.5","5"]'],
    ["int8range", "[9007199254740993,)", { lower: 9007199254740993n, bounds: "[)" }, '["9007199254740993",)'],
    ["int4range", "(,)", { bounds: "()" }, "(,)"],
    ["int4range", "empty", { empty: true }, "empty"],
    ["tstzrange", '["2026-10-20 01:00:00-07",)', { lower: "2026-10-20T08:00:00.000000Z", bounds: "[)" }, '["2026-10-20T08:00:00.000000Z",)'],
    // An infinite bound is a bound, unlike an unbounded end.
    ["daterange", "[2026-10-20,infinity)", { lower: "2026-10-20", upper: "infinity", bounds: "[)" }, '["2026-10-20","infinity")'],
    ["int4multirange", "{[1,3),[5,7)}", [{ lower: 1, upper: 3, bounds: "[)" }, { lower: 5, upper: 7, bounds: "[)" }], '{["1","3"),["5","7")}'],
    ["int4multirange", "{}", [], "{}"],
  ];
  for (const [name, text, value, literal] of cases) {
    const t = pg(name);
    const kind = columnKind(t, "postgres");
    assert.deepEqual(textDecoder(kind)(text), value, `${name} ${text}`);
    assert.deepEqual(coerce(kind, text, "c"), value, `${name} ${text}: from the text a driver gave`);
    assert.equal(postgresLiteral(t, value, (m) => assert.fail(m)), literal, `${name} ${text}: back as text`);
  }
  // A parameter that leaves its brackets out takes Postgres's constructors' `[)`.
  assert.equal(postgresLiteral(pg("int4range"), { lower: 1, upper: 5 }, (m) => assert.fail(m)), '["1","5")');
  assert.deepEqual(textDecoder(columnKind(array(pg("tstzrange")), "postgres"))(String.raw`{"[\"2026-10-20 08:00:00+00\",)",empty}`), [{ lower: "2026-10-20T08:00:00.000000Z", bounds: "[)" }, { empty: true }]);
  const brackets = `"[)" | "[]" | "(]" | "()"`;
  assert.equal(renderType(pg("int4range"), false, "postgres"), `{ empty: true } | { lower?: number; upper?: number; bounds: ${brackets} }`);
  assert.equal(renderType(pg("int4multirange"), false, "postgres"), `({ empty: true } | { lower?: number; upper?: number; bounds: ${brackets} })[]`);
  assert.equal(renderParamType(pg("int8range"), "postgres"), `{ empty: true } | { lower?: number | bigint | string; upper?: number | bigint | string; bounds?: ${brackets} }`);
  // Inside a document Postgres writes a range as its text.
  assert.equal(renderType(pg("int4range"), true, "postgres"), "string");
  // A null bound, an unknown key, brackets of another shape and anything but an object are refused.
  for (const v of [{ lower: null }, { min: 1 }, { empty: true, lower: 1 }, { lower: 1, bounds: "[[" }, "[1,5)"]) {
    assert.throws(() => postgresLiteral(pg("int4range"), v, (m) => { throw new TypeError(m); }), /^TypeError: int4range, \{ empty: true \} or \{ lower, upper, bounds \}/, JSON.stringify(v));
  }
});

test("a MySQL ENUM of known labels is refused the '' MySQL stores for any other value, unless it lists one", () => {
  const labelled = (...labels: string[]) => ({ kind: "text", enum: labels, label: "enum" }) as EngineType;
  assert.equal(columnKind(labelled("a", "b"), "mysql"), "enum");
  assert.equal(renderType(labelled("a", "b"), false, "mysql"), '"a" | "b"');
  assert.equal(coerce("enum", "a", "kind"), "a");
  assert.throws(() => coerce("enum", "", "kind"), (e: Error) => e instanceof DecodeError && /^Column "kind" holds "", which is none of its ENUM's labels.*nullif\(kind, ''\)/.test(e.message));
  // One that lists '' holds it as any other label, and one whose labels are not known is its text.
  assert.equal(columnKind(labelled("", "a"), "mysql"), "text");
  assert.equal(columnKind({ kind: "text", label: "enum" }, "mysql"), "text");
  // Postgres enforces an enum's labels itself.
  assert.equal(columnKind({ kind: "text", enum: ["a"], label: "enum" }, "postgres"), "text");
});

test("a MySQL SET is a Set of its members, declared and decoded alike, and is its text inside JSON", () => {
  const t = my("set");
  assert.equal(columnKind(t, "mysql"), "set");
  assert.deepEqual(coerce("set", "a,c", "tags"), new Set(["a", "c"]));
  assert.deepEqual(coerce("set", "", "tags"), new Set());
  // mariadb gives an array of its members.
  assert.deepEqual(coerce("set", ["a", "c"], "tags"), new Set(["a", "c"]));
  assert.equal(renderType(t, false, "mysql"), "Set<string>");
  assert.equal(renderParamType(t, "mysql"), "Set<string>");
  assert.deepEqual(typeSchema(t, "mysql"), { type: "array", items: { type: "string" }, uniqueItems: true });
  // MySQL writes a SET inside a document as its text.
  assert.equal(renderType(t, true, "mysql"), "string");
  assert.deepEqual(typeSchema(t, "mysql", false, true), { type: "string" });
});

test("the representation, value by value", () => {
  const d = (name: string, text: string) => textDecoder(scalarKind(name))(text);
  assert.equal(d("int8", "9007199254740993"), 9007199254740993n);
  assert.equal(d("int8", "-9223372036854775808"), -9223372036854775808n);
  assert.deepEqual(textDecoder(scalarKind("int8") + "[]")("{1,NULL,9223372036854775807}"), [1n, null, 9223372036854775807n]);
  assert.equal(d("timestamptz", "2026-10-20 10:00:00.123456+02"), "2026-10-20T08:00:00.123456Z");
  assert.equal(d("numeric", "12345678901234567890.123456789"), "12345678901234567890.123456789");
  assert.equal(d("date", "2026-10-20"), "2026-10-20");
  assert.equal(d("timestamp", "2026-10-20 10:00:00.123456"), "2026-10-20T10:00:00.123456");
  assert.equal(d("timestamp", "0044-03-15 12:00:00 BC"), "-000043-03-15T12:00:00.000000");
  assert.equal(d("timestamp", "infinity"), "infinity");
  assert.equal(d("interval", "1 year 2 mons"), "P1Y2M");
  assert.deepEqual(d("bytea", "\\x0102ff"), Buffer.from([1, 2, 255]));
  assert.deepEqual([...(d("bytea", "\\001\\\\x") as Uint8Array)], [1, 92, 120], "the escape format too");
  assert.deepEqual(d("jsonb", '{"big":9007199254740993}'), { big: 9007199254740992 });
  assert.equal(d("bool", "f"), false);
  assert.ok(Number.isNaN(d("float8", "NaN") as number));
  assert.equal(d("float8", "-Infinity"), -Infinity);
});

test("interval: an ISO 8601 duration with its sign in front, which Temporal reads, and each part signed where the parts' signs differ", () => {
  const Temporal = (globalThis as unknown as { Temporal: { Duration: { from(text: string): { toString(): string } } } }).Temporal;
  // Postgres's text in IntervalStyle postgres, Reylo's value, and Postgres's own in iso_8601, as Postgres 18 writes them.
  for (const [text, iso, postgres] of [
    ["1 mon", "P1M", "P1M"],
    ["1 year 2 mons 3 days 04:05:06.789", "P1Y2M3DT4H5M6.789S", "P1Y2M3DT4H5M6.789S"],
    ["00:00:00", "PT0S", "PT0S"],
    ["48:00:00", "PT48H", "PT48H"],
    ["00:00:00.000001", "PT0.000001S", "PT0.000001S"],
    ["-1 days", "-P1D", "P-1D"],
    ["-1 years -2 mons", "-P1Y2M", "P-1Y-2M"],
    ["-1 mons -2 days -00:00:03", "-P1M2DT3S", "P-1M-2DT-3S"],
    ["-178956970 years -7 mons -2147483647 days -2562047788:00:54.775807", "-P178956970Y7M2147483647DT2562047788H54.775807S", "P-178956970Y-7M-2147483647DT-2562047788H-54.775807S"],
  ] as [string, string, string][]) {
    assert.equal(duration(text), iso, text);
    assert.equal(Temporal.Duration.from(iso).toString(), iso, `${text}: Temporal reads it`);
    assert.equal(postgresInterval(iso), postgres, `${text}: as a parameter, in the form Postgres reads`);
  }
  // Parts of different signs have no ISO 8601 form: Postgres's own, which Temporal refuses.
  for (const [text, iso] of [
    ["1 mon -1 days", "P1M-1D"],
    ["-1 days +01:00:00", "P-1DT1H"],
    ["1 day -01:30:00.5", "P1DT-1H-30M-0.5S"],
  ] as [string, string][]) {
    assert.equal(duration(text), iso, text);
    assert.equal(postgresInterval(iso), iso, `${text}: already the form Postgres reads`);
  }
  assert.equal(duration("infinity"), "infinity");
  assert.equal(duration("-infinity"), "-infinity");
  // Any other text, such as a parameter in Postgres's own words, goes as it is.
  assert.equal(postgresInterval("-1 days"), "-1 days");
  assert.equal(postgresInterval("+P1D"), "P1D");
  assert.deepEqual(textDecoder("interval[]")('{"1 day",NULL,-00:00:03}'), ["P1D", null, "-PT3S"]);
  assert.equal(coerce("interval", "-1 days", "span"), "-P1D", "a driver's text, as Bun.sql gives it");
});

test("date and timestamp: a year outside 0000 to 9999 in ISO 8601's extended form, as timestamptz's, which Temporal reads and a parameter takes back", () => {
  type PlainFrom = { from(text: string): { toString(options?: { fractionalSecondDigits?: number }): string } };
  const Temporal = (globalThis as unknown as { Temporal: { PlainDate: PlainFrom; PlainDateTime: PlainFrom } }).Temporal;
  const date = textDecoder("date");
  const timestamp = textDecoder("timestamp");
  // A server text as `.run()` gives it back for a parameter: a T, and six fraction digits.
  const padded = (t: string) => t.replace(" ", "T").replace(/(\d\d:\d\d:\d\d)(\.\d+)?/, (_, hms: string, f?: string) => `${hms}.${(f ?? ".").slice(1).padEnd(6, "0")}`);
  for (const [text, iso] of [
    ["2026-10-20", "2026-10-20"],
    ["0001-01-01", "0001-01-01"],
    ["9999-12-31", "9999-12-31"],
    ["0001-12-31 BC", "0000-12-31"],
    ["0002-06-01 BC", "-000001-06-01"],
    ["4714-11-24 BC", "-004713-11-24"],
    ["10000-01-01", "+010000-01-01"],
    ["12345-01-01", "+012345-01-01"],
    ["275760-09-13", "+275760-09-13"],
  ] as const) {
    assert.equal(date(text), iso, text);
    assert.equal(Temporal.PlainDate.from(iso).toString(), iso, iso);
    // The round-trip rule: the parameter is the text the server wrote, which it reads as the same date.
    assert.equal(postgresYear(iso), text, iso);
  }
  for (const [text, iso] of [
    ["2026-10-20 10:00:00.123456", "2026-10-20T10:00:00.123456"],
    ["0001-12-31 23:59:59.999999 BC", "0000-12-31T23:59:59.999999"],
    ["0002-06-01 12:00:00 BC", "-000001-06-01T12:00:00.000000"],
    ["4714-11-24 00:00:00 BC", "-004713-11-24T00:00:00.000000"],
    ["10000-01-01 00:00:00", "+010000-01-01T00:00:00.000000"],
    ["123456-01-01 00:00:00.5", "+123456-01-01T00:00:00.500000"],
    ["275760-09-13 00:00:00", "+275760-09-13T00:00:00.000000"],
  ] as const) {
    assert.equal(timestamp(text), iso, text);
    assert.equal(Temporal.PlainDateTime.from(iso).toString({ fractionalSecondDigits: 6 }), iso, iso);
    assert.equal(postgresYear(iso), padded(text), iso);
  }
  // A date past 999999 AD, which only Postgres's date reaches (5874897), keeps a sign before the
  // digits it needs; Temporal holds no such date, and the parameter still reads it back.
  assert.equal(date("5874897-12-31"), "+5874897-12-31");
  assert.equal(date("1000000-01-01"), "+1000000-01-01");
  assert.equal(postgresYear("+5874897-12-31"), "5874897-12-31");
  assert.throws(() => Temporal.PlainDate.from("+5874897-12-31"), RangeError);
  for (const word of ["infinity", "-infinity"]) {
    assert.equal(date(word), word);
    assert.equal(timestamp(word), word);
  }
  // Arrays: each element as a scalar's, a BC element quoted in Postgres's text.
  assert.deepEqual(textDecoder("date[]")('{"0002-06-01 BC",12345-01-01,2026-10-20,NULL,infinity}'), ["-000001-06-01", "+012345-01-01", "2026-10-20", null, "infinity"]);
  assert.deepEqual(textDecoder("timestamp[]")('{"0002-06-01 12:00:00 BC","10000-01-01 00:00:00",-infinity}'), ["-000001-06-01T12:00:00.000000", "+010000-01-01T00:00:00.000000", "-infinity"]);
  // A driver's Date (Bun.sql's, read as UTC) gives the same text as the server's.
  assert.equal(coerce("date", new Date("+012345-01-01T00:00:00Z"), "d"), "+012345-01-01");
  assert.equal(coerce("timestamp", new Date("+010000-01-01T00:00:00.250Z"), "t"), "+010000-01-01T00:00:00.25");
  assert.equal(coerce("timestamp", new Date("-000001-06-01T12:00:00Z"), "t"), "-000001-06-01T12:00:00");
});

test("timestamptz: the instant's ISO 8601 text in UTC, the fraction in six digits, which new Date(text) gets wrong for BC, year 1 and offsets with seconds", () => {
  const Temporal = (globalThis as unknown as { Temporal: { Instant: { from(text: string): { toString(options?: { fractionalSecondDigits?: number }): string; epochNanoseconds: bigint } } } }).Temporal;
  for (const [text, iso] of [
    ["2026-10-20 03:03:00.123456-07", "2026-10-20T10:03:00.123456Z"],
    ["2026-10-20 10:00:00+00", "2026-10-20T10:00:00.000000Z"],
    ["2026-10-20 10:00:00.5+05:30", "2026-10-20T04:30:00.500000Z"],
    ["2026-12-31 23:30:00-01", "2027-01-01T00:30:00.000000Z"],
    ["0001-01-01 00:00:00+00", "0001-01-01T00:00:00.000000Z"],
    ["0001-01-01 00:30:00+01", "0000-12-31T23:30:00.000000Z"],
    ["1883-11-18 12:00:00-07:52:58", "1883-11-18T19:52:58.000000Z"],
    ["1900-01-01 00:00:00+00:00:36", "1899-12-31T23:59:24.000000Z"],
    ["0001-12-31 23:59:59.999999+00 BC", "0000-12-31T23:59:59.999999Z"],
    ["0002-06-01 12:00:00+00 BC", "-000001-06-01T12:00:00.000000Z"],
    ["0044-03-15 12:00:00+00 BC", "-000043-03-15T12:00:00.000000Z"],
    ["4713-11-24 00:00:00+00 BC", "-004712-11-24T00:00:00.000000Z"],
    ["9999-12-31 23:59:59.999999-01", "+010000-01-01T00:59:59.999999Z"],
    ["12345-01-01 00:00:00+05:30", "+012344-12-31T18:30:00.000000Z"],
    ["2000-02-29 12:00:00+00", "2000-02-29T12:00:00.000000Z"],
    ["1900-03-01 00:00:00+01", "1900-02-28T23:00:00.000000Z"],
  ] as const) {
    assert.equal(instant(text), iso, text);
    // Temporal reads each back to the same text: an ISO 8601 instant, the year extended outside 0000 to 9999.
    assert.equal(Temporal.Instant.from(iso).toString({ fractionalSecondDigits: 6 }), iso, iso);
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

test("timestamptz: the text read by position is the pattern's, for every server text and for text that is not one", () => {
  // The reading by pattern alone, as `instant` was before it read the usual text by position.
  const TSTZ = /^(\d{4,})-(\d\d)-(\d\d) (\d\d):(\d\d):(\d\d)(\.\d{1,6})?([+-])(\d\d)(?::(\d\d))?(?::(\d\d))?( BC)?$/;
  const pad = (n: number, width = 2) => String(n).padStart(width, "0");
  const isoYear = (y: number) => (y >= 0 && y <= 9999 ? pad(y, 4) : `${y < 0 ? "-" : "+"}${pad(Math.abs(y), 6)}`);
  const daysFromCivil = (y: number, m: number, d: number) => {
    y -= m <= 2 ? 1 : 0;
    const era = Math.floor(y / 400);
    const yoe = y - era * 400;
    const doy = Math.floor((153 * (m + (m > 2 ? -3 : 9)) + 2) / 5) + d - 1;
    return era * 146097 + yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy - 719468;
  };
  const civilFromDays = (days: number): [number, number, number] => {
    const z = days + 719468;
    const era = Math.floor(z / 146097);
    const doe = z - era * 146097;
    const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
    const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
    const mp = Math.floor((5 * doy + 2) / 153);
    const d = doy - Math.floor((153 * mp + 2) / 5) + 1;
    const m = mp + (mp < 10 ? 3 : -9);
    return [era * 400 + yoe + (m <= 2 ? 1 : 0), m, d];
  };
  const byPattern = (x: string): string => {
    const m = TSTZ.exec(x);
    if (m === null) return x;
    const year = m[12] === undefined ? +m[1]! : 1 - +m[1]!;
    const offset = (m[8] === "-" ? -1 : 1) * (+m[9]! * 3600 + (m[10] === undefined ? 0 : +m[10] * 60) + (m[11] === undefined ? 0 : +m[11]));
    const seconds = daysFromCivil(year, +m[2]!, +m[3]!) * 86400 + +m[4]! * 3600 + +m[5]! * 60 + +m[6]! - offset;
    const days = Math.floor(seconds / 86400);
    const s = seconds - days * 86400;
    const [y, mo, d] = civilFromDays(days);
    return `${isoYear(y)}-${pad(mo)}-${pad(d)}T${pad(Math.floor(s / 3600))}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}${m[7] ? m[7].padEnd(7, "0") : ".000000"}Z`;
  };
  // A seeded generator, so that a failure names a text that fails again.
  let seed = 0x2545f491;
  const random = () => ((seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) | 0) >>> 0) / 2 ** 32;
  const int = (lo: number, hi: number) => lo + Math.floor(random() * (hi - lo + 1));
  const pick = <T,>(xs: readonly T[]): T => xs[int(0, xs.length - 1)]!;
  const DAYS = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  // What the server writes in DateStyle ISO: a year of four digits or more (` BC` before 1 AD), a
  // fraction of up to six digits, an offset in hours, minutes or seconds (a zone's local mean time).
  const serverText = () => {
    const kind = random();
    const year = kind < 0.6 ? int(1000, 9999) : kind < 0.75 ? int(1, 999) : kind < 0.9 ? int(10000, 294276) : int(1, 4713);
    const month = int(1, 12);
    const fraction = random() < 0.5 ? "" : `.${String(int(0, 999999)).padStart(6, "0").slice(0, int(1, 6))}`;
    const sign = random() < 0.5 ? "+" : "-";
    const zone = random();
    const offset = `${sign}${pad(int(0, 15))}${zone < 0.6 ? "" : `:${pad(int(0, 59))}${zone < 0.85 ? "" : `:${pad(int(0, 59))}`}`}`;
    return `${pad(year, 4)}-${pad(month)}-${pad(int(1, DAYS[month - 1]!))} ${pad(int(0, 23))}:${pad(int(0, 59))}:${pad(int(0, 59))}${fraction}${offset}${kind >= 0.9 ? " BC" : ""}`;
  };
  // A server text with one character changed, taken out or put in: text the pattern refuses, or reads otherwise.
  const CHARS = "0123456789-+:. BCTZx";
  const mutated = () => {
    const t = serverText();
    const at = int(0, t.length - 1);
    const op = random();
    return op < 0.5 ? t.slice(0, at) + pick([...CHARS]) + t.slice(at + 1) : op < 0.75 ? t.slice(0, at) + t.slice(at + 1) : t.slice(0, at) + pick([...CHARS]) + t.slice(at);
  };
  const texts = ["infinity", "-infinity", "", "2026-10-20 10:00:00+00", "2026-10-20 10:00:00.1234567+00", "2026-10-20 10:00:00.+00", "2026-10-20 10:00:00+0", "2026-10-20 10:00:00+00:0", "2026-10-20 10:00:00+00:00:00:00"];
  for (let i = 0; i < 300_000; i++) texts.push(serverText());
  for (let i = 0; i < 100_000; i++) texts.push(mutated());
  for (const text of texts) if (instant(text) !== byPattern(text)) assert.equal(instant(text), byPattern(text), JSON.stringify(text));
});

test("SQLite: the kinds, and a NUMERIC column of either", () => {
  assert.deepEqual(["integer", "real", "text", "blob", "numeric", "bool"].map((n) => columnKind(lite(n), "sqlite")), ["int", "float", "text", "bytes", "int|float|text", "bool"]);
  assert.equal(columnKind({ kind: "unknown" }, "sqlite"), "unknown");
  assert.deepEqual(["int", "int|float|text", "unknown", "bytes"].map(kindType), ["number", "number | string", "unknown", "Uint8Array"]);
});

test("SQLite: a column declared BIGINT, INT8 or UNSIGNED BIG INT (the engine's int8) is a BigInt, as on Postgres and MySQL, and its parameter takes one back", () => {
  assert.equal(columnKind(lite("int8"), "sqlite"), "int8");
  assert.equal(renderType(lite("int8"), false, "sqlite"), "bigint");
  assert.equal(renderType(lite("int8"), true, "sqlite"), "number", "inside a JSON document, the number SQLite writes");
  assert.equal(renderParamType(lite("int8"), "sqlite"), "number | bigint");
  assert.deepEqual(typeSchema(lite("int8"), "sqlite"), { type: "integer", format: "int64" }, "the BigInt, as the JSON integer it is");
  assert.deepEqual(typeSchema(lite("int8"), "sqlite"), typeSchema(pg("int8"), "postgres"));
  // Inside a document a non-finite float or decimal is the text Postgres writes, which the run time
  // gives as the number it is; MySQL stores none.
  assert.deepEqual(typeSchema(pg("float8"), "postgres", false, true), { type: "number", format: "double" });
  assert.deepEqual(typeSchema(my("decimal"), "mysql", false, true), { type: "number", format: "decimal" });
  assert.deepEqual(typeSchema(pg("interval"), "postgres", false, true), { type: "string", description: "pg type interval" }, "inside a document, Postgres's own text");
  assert.deepEqual(typeSchema(lite("int8"), "sqlite", true), { type: "integer", format: "int64" }, "a parameter");
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
  assert.equal(coerce("timestamptz", new Date("2026-10-20T10:00:00.500Z"), "c"), "2026-10-20T10:00:00.500000Z", "Bun.sql's Date, to its millisecond");
  assert.equal(coerce("timestamptz", new Date("2026-10-20T10:00:00Z"), "c"), "2026-10-20T10:00:00.000000Z");
  // A Postgres timestamp from a Date takes six digits too; a MySQL DATETIME keeps the driver's.
  assert.equal(coerce("timestamp", new Date("2026-10-20T10:00:00.500Z"), "c", ",", true), "2026-10-20T10:00:00.500000");
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
  const mysql = [my("bigint"), oneOf(my("bigint"), my("int")), oneOf(my("int"), my("bigint")), oneOf(my("bigint"), my("double"), my("decimal")), oneOf(my("int"), my("bigint"), my("double"), my("decimal")), { kind: "boolean" } as EngineType, my("timestamp"), my("date")];
  assert.deepEqual(mysql.map((t) => columnKind(t, "mysql")), ["int8", "int8", "int8", "int8|float|decimal", "int|int8|float|decimal", "bool", "timestamp", "date"]);
  assert.deepEqual(mysql.map((t) => kindType(columnKind(t, "mysql"))), ["bigint", "bigint", "bigint", "bigint | number | string", "number | bigint | string", "boolean", "string", "string"]);
  assert.equal(renderType(array(pg("int8"))), "(bigint | null)[]");
  assert.equal(renderType(pg("int8"), true), "number", "inside a JSON document, as the database writes it");
  assert.deepEqual(typeSchema(pg("int8"), "postgres"), { type: "integer", format: "int64" });
  assert.deepEqual(typeSchema(pg("int8"), "postgres", true), { type: ["integer", "string"], format: "int64" }, "a parameter, as every driver binds it");
  assert.deepEqual(typeSchema(pg("timestamptz"), "postgres"), { type: "string", format: "date-time" });
  assert.deepEqual(typeSchema({ kind: "boolean" }, "mysql"), { type: "boolean" });
  assert.deepEqual(typeSchema(lite("bool"), "sqlite"), { type: "boolean" });
  assert.deepEqual(typeSchema(lite("integer"), "sqlite"), { type: "integer", format: "int64" });
  assert.deepEqual(typeSchema(lite("numeric"), "sqlite"), { type: ["number", "string"] });
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

test("SQLite: a value its column's type does not allow, which a table that is not STRICT stores, refused naming the column; a NUMERIC's text, an untyped column's BLOB and an INTEGER column's REAL kept", async () => {
  const db = new DatabaseSync(":memory:");
  db.exec(`create table loose (id integer primary key, n integer, r real, s text, b blob, big bigint, z numeric, x);
    insert into loose values (1, 1.5, 2, 'hi', x'01', 9007199254740993, 'abc', x'02'), (2, 'abc', null, null, null, null, null, null), (3, x'0102', null, null, null, null, null, null),
      (4, null, 'xyz', null, null, null, null, null), (5, null, null, x'0203', null, null, null, null), (6, null, null, null, 'plain', null, null, null), (7, null, null, null, 5, null, null, null),
      (8, null, null, null, null, 1.5, null, null), (9, null, null, null, null, 'x', null, null), (10, null, null, null, null, null, x'01', null)`);
  const q: QueryModule<"sqlite"> = { dialect: "sqlite", sql: "select n, r, s, b, big, z, x from loose where id = :id", params: ["id"], columns: [["n", "int"], ["r", "float"], ["s", "text"], ["b", "bytes"], ["big", "int8"], ["z", "int|float|text"], ["x", "unknown"]] };
  assert.deepEqual(execute(db, q, { id: 1 }), [{ n: 1.5, r: 2, s: "hi", b: Uint8Array.of(1), big: 9007199254740993n, z: "abc", x: Uint8Array.of(2) }]);
  const refusals: [number, RegExp][] = [
    [2, /^TypeError: Column "n" is typed number and holds the text "abc": outside a STRICT table SQLite stores a value of any type in any column.* Replace the value, or read what the column holds with cast\(n as text\)\.$/],
    [3, /^TypeError: Column "n" is typed number and holds a BLOB of 2 bytes/],
    [4, /^TypeError: Column "r" is typed number and holds the text "xyz"/],
    [5, /^TypeError: Column "s" is typed string and holds a BLOB of 2 bytes/],
    [6, /^TypeError: Column "b" is typed Uint8Array and holds the text "plain"/],
    [7, /^TypeError: Column "b" is typed Uint8Array and holds the number 5/],
    [8, /^TypeError: Column "big" is typed bigint and holds the number 1.5: .* Replace the value, or read what the column holds with cast\(big as text\)\.$/],
    [9, /^TypeError: Column "big" is typed bigint and holds the text "x"/],
    [10, /^TypeError: Column "z" is typed number \| string and holds a BLOB of 1 byte:/],
  ];
  for (const [id, refused] of refusals) assert.throws(() => execute(db, q, { id }), (e: Error) => e instanceof DecodeError && refused.test(String(e)), `row ${id}`);
  // The drivers that read integers as numbers or strings: libSQL's digits are its INTEGER, any other text in a number's column is refused, and so is D1's BLOB in a string's.
  const libsql = (rows: unknown[]) => ({ execute: async () => ({ rows }), executeMultiple: async () => {} });
  const ints: QueryModule<"sqlite"> = { dialect: "sqlite", sql: "select n, big, b from loose", params: [], columns: [["n", "int"], ["big", "int8"], ["b", "bytes"]] };
  assert.deepEqual(await execute(libsql([{ n: "5", big: "9007199254740993", b: null }]), ints), [{ n: 5, big: 9007199254740993n, b: null }], "intMode: string");
  await assert.rejects(execute(libsql([{ n: "abc", big: null, b: null }]), ints) as Promise<unknown>, /Column "n" is typed number and holds the text "abc"/);
  await assert.rejects(execute(libsql([{ n: 5, big: 1.5, b: null }]), ints) as Promise<unknown>, /Column "big" is typed bigint and holds the number 1.5/);
  await assert.rejects(execute(libsql([{ n: Uint8Array.of(1).buffer, big: null, b: null }]), ints) as Promise<unknown>, /Column "n" is typed number and holds a BLOB of 1 byte/);
  await assert.rejects(execute(libsql([{ n: null, big: null, b: "5" }]), ints) as Promise<unknown>, /Column "b" is typed Uint8Array and holds the text "5"/);
  const d1 = { batch: async () => [], prepare: () => ({ first: async () => null, bind: () => ({ all: async () => ({ results: [{ s: [1, 2] }] }) }) }) };
  await assert.rejects(execute(d1, { dialect: "sqlite", sql: "select s from loose", params: [], columns: [["s", "text"]] }) as Promise<unknown>, /Column "s" is typed string and holds a BLOB of 2 bytes/);
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
  const vector: EngineType = { kind: "array", element: { type: { kind: "float", exponent_bits: 8, mantissa_bits: 23 } }, label: "vector" };
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

test("an hstore's text is an object of its keys, and an object goes back in as that text", () => {
  // As Postgres 17 writes hstore(array['a','q"uote','back\\slash','b c','x=>y','',','], array['1','v"','\\','sp ace',null,'empty','comma,']).
  const text = String.raw`""=>"empty", ","=>"comma,", "a"=>"1", "b c"=>"sp ace", "x=>y"=>NULL, "q\"uote"=>"v\"", "back\\slash"=>"\\"`;
  const value = { "": "empty", ",": "comma,", a: "1", "b c": "sp ace", "x=>y": null, 'q"uote': 'v"', "back\\slash": "\\" };
  assert.deepEqual(hstore(text), value);
  assert.deepEqual(textDecoder("hstore")(text), value);
  assert.deepEqual(hstore(""), {});
  // A key named __proto__ is a key, not the object's prototype.
  assert.deepEqual(Object.keys(hstore(String.raw`"__proto__"=>"x"`)), ["__proto__"]);
  // An array of hstores, as Postgres quotes each element.
  assert.deepEqual(textDecoder("hstore[]")(String.raw`{"\"a\"=>\"1\"","\"k\"=>NULL",NULL}`), [{ a: "1" }, { k: null }, null]);
  // Bun.sql hands the text over; an object a driver parsed stays as it is.
  assert.deepEqual(coerce("hstore", String.raw`"a"=>"1"`, "h"), { a: "1" });
  assert.deepEqual(coerce("hstore", { a: "1" }, "h"), { a: "1" });
  const fail = (message: string): never => {
    throw new TypeError(message);
  };
  assert.equal(hstoreText(value, fail), String.raw`""=>"empty", ","=>"comma,", "a"=>"1", "b c"=>"sp ace", "x=>y"=>NULL, "q\"uote"=>"v\"", "back\\slash"=>"\\"`);
  assert.deepEqual(hstore(hstoreText(value, fail)), value);
  assert.equal(hstoreText({ a: undefined, b: "2" }, fail), String.raw`"b"=>"2"`);
  assert.throws(() => hstoreText({ n: 1 }, fail), /its key "n" holds 1/);
  assert.throws(() => hstoreText(["a"], fail), /an object of its keys whose values are strings or null; got an array/);
  // Only a plain object's keys are an hstore's: a Map or a class instance is refused, not written empty.
  assert.throws(() => hstoreText(new Map([["a", "1"]]), fail), /got a Map\./);
  assert.throws(() => hstoreText(new (class Prefs {})(), fail), /got a Prefs\./);
  assert.equal(hstoreText(Object.assign(Object.create(null), { a: "1" }), fail), String.raw`"a"=>"1"`);
});

test("the text Postgres writes inside JSON for a non-finite float or decimal is that number, where the type says one may stand", () => {
  const at = { keys: [["n", true], ["xs", { items: true, deep: true }], ["rows", { items: { keys: [["v", true]] } }], ["__proto__", true]] } as const;
  const v = JSON.parse(`{"n": "NaN", "s": "NaN", "xs": [["Infinity", 1], ["-Infinity", "NaN"]], "rows": [{"v": "Infinity", "w": "NaN"}, {"v": 2.5}], "__proto__": "-Infinity"}`);
  assert.equal(numbersInJson(v, at), v, "in place");
  assert.deepEqual(v.n, NaN);
  assert.equal(v.s, "NaN", "a key the type names no number at keeps its text");
  assert.deepEqual(v.xs, [[Infinity, 1], [-Infinity, NaN]]);
  assert.deepEqual(v.rows, [{ v: Infinity, w: "NaN" }, { v: 2.5 }]);
  assert.equal(Object.getOwnPropertyDescriptor(v, "__proto__")?.value, -Infinity);
  // Any other text, a number, null and a value of another shape stay as they are.
  assert.equal(numbersInJson("nan", true), "nan");
  assert.equal(numbersInJson(null, at), null);
  assert.deepEqual(numbersInJson({ xs: "NaN" }, at), { xs: "NaN" });
  // A Postgres array of JSON arrays reads each array as an element.
  assert.deepEqual(numbersInJson([["NaN"]], { items: { items: true }, deep: true }), [[NaN]]);
});

test("a row converts each name's last column by that column's own mask, by position beside the columns", () => {
  const n = { keys: [["n", true]] } as const;
  const columns: Columns = [["x", "json"], ["x", "json"], ["y", "json"], ["y", "json"], ["constructor", "json"], ["__proto__", "json"], ["id", "int4"]];
  // The first `x` has a mask and the last none; the first `y` none and the last one.
  assert.deepEqual(rowMasks(columns, [n, null, null, n, null, n, null]), [["y", n], ["__proto__", n]]);
  assert.deepEqual(rowMasks(columns, []), []);
});

test("an array of arrays decodes its innermost elements by their kind, from text or a driver's arrays", () => {
  // The kind of array_agg over an int8[] column: two levels, whose text nests them itself.
  assert.deepEqual(textDecoder("int8[][]")("{{1,2},{3,NULL}}"), [[1n, 2n], [3n, null]]);
  assert.deepEqual(coerce("int8[][]", "{{1,2}}", "c"), [[1n, 2n]]);
  assert.deepEqual(coerce("int8[][]", [["1", "2"], ["3", null]], "c"), [[1n, 2n], [3n, null]]);
});

test("an array of rows reads as parseArray and parseRow read it, in every quoting record_out and array_out write", () => {
  const row = `(${JSON.stringify(["a", "int"])},${JSON.stringify(["b", "text"])},${JSON.stringify(["c", "text"])})`;
  const decode = textDecoder(row) as (text: string) => unknown;
  for (const text of ['(1,"he said ""hi"", (ok) \\\\ back",)', '(1,,"")', "(,,)", "(2,NULL,x)", '(3,"a b","c\\"d")']) {
    const [a, b, c] = parseRow(text, 3);
    assert.deepEqual(decode(text), { a: a === null ? null : Number(a), b, c }, text);
  }
  const rows = textDecoder(`${row}[]`) as (text: string) => unknown;
  const one = textDecoder(`(${JSON.stringify(["x", "int"])})[]`) as (text: string) => unknown;
  assert.deepEqual(rows("{}"), []);
  assert.deepEqual(rows("{NULL}"), [null]);
  assert.deepEqual(one("{(1),NULL,(2)}"), [{ x: 1 }, null, { x: 2 }]);
  assert.deepEqual(one("{()}"), [{ x: null }]);
  assert.deepEqual(rows('{"(1,x,)","(2,\\"y z\\",\\"\\")"}'), [{ a: 1, b: "x", c: null }, { a: 2, b: "y z", c: "" }]);
  // Another shape parseArray reads: lower bounds, and more than one dimension.
  assert.deepEqual(rows('[0:0]={"(1,x,y)"}'), [{ a: 1, b: "x", c: "y" }]);
  assert.deepEqual(rows('{{"(1,x,y)"},{"(2,a,b)"}}'), [[{ a: 1, b: "x", c: "y" }], [{ a: 2, b: "a", c: "b" }]]);
  // A field named __proto__ is a field, whatever an assignment would make of it.
  const proto = textDecoder(`(${JSON.stringify(["__proto__", "int"])})`) as (text: string) => object;
  assert.deepEqual(Object.keys(proto("(1)")), ["__proto__"]);
});
