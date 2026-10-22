import { test } from "node:test";
import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { arrayLiteral, execute, executeSync, type QueryModule } from "../src/execute.ts";
import { defineQuery } from "../src/index.ts";

// Each fake client records what execute handed it and answers the way its driver does; the
// driver behaviour each one copies was checked against the real package (lab/driver-matrix).
type Call = { method: string; args: unknown[] };
const pg: QueryModule<"postgres"> = { dialect: "postgres", sql: "select :id::int as id, :name as name, :id as again", params: ["id", "name"] };
const my: QueryModule<"mysql"> = { dialect: "mysql", sql: "select :id as id, :name as name, :id as again", params: ["id", "name"] };
const lite: QueryModule<"sqlite"> = { dialect: "sqlite", sql: "select :id as id, :name as name, :id as again", params: ["id", "name"] };
const row = { id: 1, name: "ada", again: 1 };

test("postgres: query(text, values) → { rows }, as on pg, PGlite and Neon's Pool, with $n numbered once per name", async () => {
  const calls: Call[] = [];
  const client = { query: async (...args: unknown[]) => (calls.push({ method: "query", args }), { rows: [row], rowCount: 1 }) };
  assert.deepEqual(await execute(client, pg, { id: 1, name: "ada" }), [row]);
  assert.deepEqual(calls, [{ method: "query", args: ["select $1::int as id, $2 as name, $1 as again", [1, "ada"]] }]);
});

test("postgres: a query() that resolves to the rows, as Neon's neon() from 1.0 and pg-promise do", async () => {
  const neon = Object.assign(() => Promise.reject(new Error("not a template")), {
    query: async () => [row],
    // Neon's unsafe() builds a fragment for a template; it must never be taken for a query.
    unsafe: () => ({ sql: "fragment" }),
  });
  assert.deepEqual(await execute(neon as never, pg, { id: 1, name: "ada" }), [row]);
});

test("postgres: pg-promise gets a { text, values } object, so it binds instead of formatting $1 into the text", async () => {
  const calls: Call[] = [];
  // pg-promise types query() as resolving to any.
  const db = { manyOrNone: async () => [], query: async (...args: unknown[]): Promise<any> => (calls.push({ method: "query", args }), [row]) };
  const q: QueryModule<"postgres"> = { dialect: "postgres", sql: "select '$2' as literal, :id::int as id", params: ["id"] };
  assert.deepEqual(await execute(db, q, { id: 1 }), [row]);
  // With Reylo's decoder table, which answers every oid itself.
  const { types, ...bound } = calls[0]!.args[0] as { text: string; values: unknown[]; types: { getTypeParser(oid: number): (text: string) => unknown } };
  assert.deepEqual(bound, { text: "select '$2' as literal, $1::int as id", values: [1] });
  assert.equal(types.getTypeParser(20)("9007199254740993"), 9007199254740993n);
});

test("postgres: unsafe(text, values) → rows, as on postgres.js and Bun.sql and their transactions", async () => {
  const calls: Call[] = [];
  const tx = { unsafe: (...args: unknown[]) => (calls.push({ method: "unsafe", args }), Promise.resolve([row])) };
  assert.deepEqual(await execute(tx, pg, { id: 1, name: "ada" }), [row]);
  assert.deepEqual(calls, [{ method: "unsafe", args: ["select $1::int as id, $2 as name, $1 as again", [1, "ada"]] }]);
});

// The module's `inputs`: :ids is an int4[] parameter, :data a jsonb one, :name text.
const typed: QueryModule<"postgres"> = {
  dialect: "postgres",
  sql: "select id from users where id = any(:ids) and name = :name and tags @> :data::jsonb",
  params: ["ids", "name", "data"],
  text: "select id from users where id = any($1) and name = $2 and tags @> $3::jsonb",
  order: ["ids", "name", "data"],
  inputs: { ids: { type: { kind: "array", of: { kind: "scalar", name: "int4" } } }, name: { type: { kind: "scalar", name: "text" } }, data: { type: { kind: "json" } } },
};
const typedValues = { ids: [1, 3], name: "ada", data: [1, { a: "b" }] };

test("postgres: every client gets an array for a SQL array parameter as its literal text; Bun.sql gets a JSON parameter as an object it serializes to its JSON text, postgres.js as its JSON text declared unknown", async () => {
  const seen: unknown[][] = [];
  const unsafe = async (_: string, values?: unknown[]) => (seen.push(values!), []);
  const bun = { unsafe, commitDistributed: async () => {} };
  await execute(bun, typed, typedValues);
  await execute({ unsafe }, typed, typedValues);
  assert.equal(seen.length, 2);
  for (const [ids, name, data] of seen) {
    assert.deepEqual([ids, name], ['{"1","3"}', "ada"]);
    // Not a string, which they would store as a JSON string, and not the array, whose type they would take from its first element.
    assert.equal(typeof data, "object");
    assert.ok(!Array.isArray(data));
    assert.equal(JSON.stringify(data), '[1,{"a":"b"}]');
  }
  // postgres.js (typed() beside unsafe()), whose JSON serializer Drizzle replaces with one that passes the value through.
  const postgresJs = { unsafe, typed: (value: unknown, type: number) => ({ value, type }) };
  await execute(postgresJs, typed, typedValues);
  assert.deepEqual(seen.at(-1), ['{"1","3"}', "ada", { value: '[1,{"a":"b"}]', type: 705 }]);
});

// Every kind of JSON value, null, and a jsonb[] parameter, whose elements are JSON values.
const jsonQuery: QueryModule<"postgres"> = {
  dialect: "postgres",
  sql: "select :s::jsonb, :n::jsonb, :b::jsonb, :t::jsonb, :z::jsonb, :o::jsonb, :e::jsonb, :l::jsonb[]",
  params: ["s", "n", "b", "t", "z", "o", "e", "l"],
  inputs: { ...Object.fromEntries([..."snbtzoe"].map((k) => [k, { type: { kind: "json" as const } }])), l: { type: { kind: "array", of: { kind: "json" } } } },
};
const jsonValues = { s: "abc", n: 5, b: true, t: '{"a":1}', z: null, o: { d: new Date("2026-01-02T03:04:05Z") }, e: [], l: ["x", { a: 1 }, [true]] };
const jsonTexts = ['"abc"', "5", "true", '"{\\"a\\":1}"', null, '{"d":"2026-01-02T03:04:05.000Z"}', "[]", '{"\\"x\\"","{\\"a\\":1}","[true]"}'];

test("postgres: a JSON parameter's value is a JavaScript value, which every driver gets as its JSON text, a string as a JSON string, null as NULL", async () => {
  const seen: unknown[][] = [];
  await execute({ query: async (_: string, values?: unknown[]) => (seen.push(values!), { rows: [] }) }, jsonQuery, jsonValues);
  const pgp = { manyOrNone: async () => [], query: async (q: any): Promise<any> => (seen.push(q.values), []) };
  await execute(pgp, jsonQuery, jsonValues);
  await execute((_: string, values?: unknown[]) => (seen.push(values!), Promise.resolve([])), jsonQuery, jsonValues);
  const unsafe = async (_: string, values?: unknown[]) => (seen.push(values!.map((v) => (v !== null && typeof v === "object" ? JSON.stringify(v) : v))), []);
  const bun = { unsafe, commitDistributed: async () => {} };
  await execute({ unsafe }, jsonQuery, jsonValues);
  await execute(bun, jsonQuery, jsonValues);
  // postgres.js gets each JSON value as its text declared unknown (705), and null as null.
  const unwrap = (v: unknown) => (v !== null && typeof v === "object" ? (assert.equal((v as { type: number }).type, 705), (v as { value: unknown }).value) : v);
  const postgresJs = { unsafe: async (_: string, values?: unknown[]) => (seen.push(values!.map(unwrap)), []), typed: (value: unknown, type: number) => ({ value, type }) };
  await execute(postgresJs, jsonQuery, jsonValues);
  assert.equal(seen.length, 6);
  for (const values of seen) assert.deepEqual(values, jsonTexts);
});

test("mysql and sqlite: every driver gets a JSON parameter as its JSON text, and null as NULL", async () => {
  const json = { type: { kind: "json" } } as const;
  const mq: QueryModule<"mysql"> = { dialect: "mysql", sql: "select cast(:a as json), cast(:b as json), :name, cast(:a as json), cast(:z as json)", params: ["a", "b", "name", "z"], inputs: { a: json, b: json, z: json, name: { type: { kind: "scalar", name: "text" } } } };
  const values = { a: [1, 2], b: "abc", name: "ada", z: null };
  const expected = ["[1,2]", '"abc"', "ada", "[1,2]", null];
  const seen: unknown[][] = [];
  await execute({ query: async () => [[], []] as [unknown, unknown], execute: async (_: unknown, v?: unknown[]) => (seen.push(v!), [[], []] as [unknown, unknown]) }, mq, values);
  await execute({ query: async (_: string, v: unknown) => (seen.push(v as unknown[]), []), batch: async () => [] }, mq, values);
  await execute({ unsafe: async (_: string, v?: unknown[]) => (seen.push(v!), []), commitDistributed: async () => {} }, mq, values);
  const lq: QueryModule<"sqlite"> = { ...mq, dialect: "sqlite", sql: "select json(:a) as a, json(:b) as b, :name as name, json(:a) as again, json(:z) as z" };
  await execute({ execute: async (s: { args: unknown[] }) => (seen.push(s.args), { rows: [] }), executeMultiple: async () => {} }, lq, values);
  assert.equal(seen.length, 4);
  // node:sqlite would read an array as its named parameters.
  assert.deepEqual((execute(new DatabaseSync(":memory:"), lq, values) as object[]).map((r) => ({ ...r })), [{ a: "[1,2]", b: '"abc"', name: "ada", again: "[1,2]", z: null }]);
  for (const v of seen) assert.deepEqual(v, expected);
});

test("postgres: pg's family, PGlite and Neon, which write any array as an array literal, get an array for a JSON parameter as its JSON text", async () => {
  const seen: unknown[][] = [];
  await execute({ query: async (_: string, values?: unknown[]) => (seen.push(values!), { rows: [] }) }, typed, typedValues);
  const pgp = { manyOrNone: async () => [], query: async (q: any): Promise<any> => (seen.push(q.values), []) };
  await execute(pgp, typed, typedValues);
  await execute((_: string, values?: unknown[]) => (seen.push(values!), Promise.resolve([])), typed, typedValues);
  assert.equal(seen.length, 3);
  for (const values of seen) assert.deepEqual(values, ['{"1","3"}', "ada", '[1,{"a":"b"}]']);
});

test("a query without inputs, defined by hand or built before them, binds every array as it is", async () => {
  const { inputs: _, ...bare } = typed;
  const seen: unknown[][] = [];
  const unsafe = async (_: string, values?: unknown[]) => (seen.push(values!), []);
  const bun = { unsafe, commitDistributed: async () => {} };
  await execute(bun, bare, typedValues);
  await execute({ unsafe }, bare, typedValues);
  await execute({ query: async (_: string, values?: unknown[]) => (seen.push(values!), { rows: [] }) }, bare, typedValues);
  for (const values of seen) assert.deepEqual(values, [[1, 3], "ada", [1, { a: "b" }]]);
  // Hand-written inputs work through the runtime's own lexer too, a repeated name included.
  const hand: QueryModule<"postgres"> = { dialect: "postgres", sql: "select :ids::int[] = :ids::int[]", params: ["ids"], inputs: { ids: { type: { kind: "array", of: { kind: "scalar", name: "int4" } } } } };
  seen.length = 0;
  await execute({ unsafe }, hand, { ids: [2] });
  assert.deepEqual(seen, [['{"2"}']]);
});

test("a Date for any date or time parameter, and a string without its zone for a timestamptz one, is refused before the statement runs", async () => {
  const time = (name: string) => ({ type: { kind: "scalar" as const, name } });
  const q: QueryModule<"postgres"> = {
    dialect: "postgres",
    sql: "select :at::timestamp, :day::date, :tz::timestamptz, :tzs::timestamptz[], :days::date[]",
    params: ["at", "day", "tz", "tzs", "days"],
    inputs: { at: time("timestamp"), day: time("date"), tz: time("timestamptz"), tzs: { type: { kind: "array", of: time("timestamptz").type } }, days: { type: { kind: "array", of: time("date").type } } },
  };
  const seen: unknown[][] = [];
  const client = { query: async (_: string, values?: unknown[]) => (seen.push(values!), { rows: [] }) };
  const ok = { at: "2026-10-12 19:06:48.282", day: "2026-10-12", tz: "1970-01-01T00:00:00Z", tzs: ["2026-10-12T19:06:48.282Z", "2026-10-12 19:06:48+09", "2026-10-12 19:06:48.5-07:30", "2026-10-12 19:06 America/Los_Angeles", "2026-10-12 19:06:48 UTC", "infinity", "-infinity", "0001-01-01 00:00:00+00 BC", "+010000-01-01T00:00:00Z"], days: [["2026-10-12"]] };
  await execute(client, q, ok);
  assert.equal(seen.length, 1);
  const refused = async (values: Record<string, unknown>, message: RegExp) => assert.rejects(Promise.resolve().then(() => execute(client, q, { ...ok, ...values })), (e: Error) => e instanceof TypeError && message.test(e.message));
  await refused({ at: new Date() }, /^Parameter "at" is a timestamp without a time zone, so it takes the wall-clock text/);
  await refused({ day: new Date() }, /^Parameter "day" is a date without a time zone/);
  await refused({ days: [[new Date()]] }, /^Parameter "days" is a date/);
  // A timestamptz takes the string `.run()` returns for one, and a Date nowhere.
  await refused({ tz: new Date(0) }, /^Parameter "tz" is a timestamptz, which takes the instant as a string that names its zone, as `\.run\(\)` returns one, not a Date: pass date\.toISOString\(\)/);
  await refused({ tzs: ["2026-10-12T00:00:00Z", new Date(0)] }, /^Parameter "tzs" is a timestamptz, .*not a Date/);
  for (const tz of ["2026-10-12 19:06:48", "2026-10-12T19:06:48.282", "2026-10-12"]) await refused({ tz }, /^Parameter "tz" is a timestamptz, an instant, so a string for it names its zone/);
  await refused({ tzs: ["2026-10-12T00:00:00Z", "2026-10-12 19:06"] }, /^Parameter "tzs" is a timestamptz/);
  // An array parameter takes no null element, as its type says, in any dimension.
  await refused({ tzs: ["2026-10-12T00:00:00Z", null] }, /^Parameter "tzs" is an array whose elements may not be null, and the element at \[1\] is/);
  await refused({ days: [["2026-10-12", undefined]] }, /^Parameter "days" is an array whose elements may not be null, and the element at \[0\]\[1\] is/);
  assert.equal(seen.length, 1);
  // MySQL's DATETIME and TIMESTAMP are the engine's timestamp, and a placeholder compared with both is either.
  const my: QueryModule<"mysql"> = { dialect: "mysql", sql: "select :at", params: ["at"], inputs: { at: time("date|timestamp") } };
  await assert.rejects(Promise.resolve().then(() => execute({ query: async () => [[], []] as [unknown, unknown], execute: async () => [[], []] as [unknown, unknown] }, my, { at: new Date() })), /^TypeError: Parameter "at" is a timestamp without a time zone/);
});

test("postgres: postgres.js, which binds a date or time string through new Date(), gets one declared unknown; Bun.sql gets the string", async () => {
  const time = (name: string) => ({ type: { kind: "scalar" as const, name } });
  const q: QueryModule<"postgres"> = { dialect: "postgres", sql: "select :at::timestamp, :day::date, :tz::timestamptz, :d::timestamptz, :name::text", params: ["at", "day", "tz", "d", "name"], inputs: { at: time("timestamp"), day: time("date"), tz: time("timestamptz"), d: time("timestamptz"), name: time("text") } };
  const values = { at: "2026-01-02 03:04:05.006", day: "infinity", tz: "2026-01-02 03:04:05 America/New_York", d: "1970-01-01T00:00:00Z", name: "ada" };
  const seen: unknown[][] = [];
  const unsafe = async (_: string, v?: unknown[]) => (seen.push(v!), []);
  const typed = (value: unknown, type: number) => ({ value, type });
  const postgresJs = { unsafe, typed };
  await execute(postgresJs, q, values);
  const bun = { unsafe, typed, commitDistributed: async () => {} };
  await execute(bun, q, values);
  assert.deepEqual(seen, [
    [{ value: "2026-01-02 03:04:05.006", type: 705 }, { value: "infinity", type: 705 }, { value: "2026-01-02 03:04:05 America/New_York", type: 705 }, { value: "1970-01-01T00:00:00Z", type: 705 }, "ada"],
    ["2026-01-02 03:04:05.006", "infinity", "2026-01-02 03:04:05 America/New_York", "1970-01-01T00:00:00Z", "ada"],
  ]);
});

test("what run returns goes back in: a SQLite truth value as 1 or 0, a Postgres extended or BC year in Postgres's own form", async () => {
  const s = (name: string) => ({ type: { kind: "scalar" as const, name } });
  // SQLite: better-sqlite3 refuses a boolean and libSQL binds it as the REAL 1.0.
  const lq: QueryModule<"sqlite"> = { dialect: "sqlite", sql: "select :a as a, :b as b, :n as n", params: ["a", "b", "n"], inputs: { a: s("numeric"), b: s("integer"), n: s("integer") } };
  const seen: unknown[][] = [];
  await execute({ execute: async (st: { args: unknown[] }) => (seen.push(st.args), { rows: [] }), executeMultiple: async () => {} }, lq, { a: true, b: false, n: 7 });
  assert.deepEqual([...seen], [[1, 0, 7]]);
  // MySQL's drivers bind a boolean as 1 themselves.
  const mq: QueryModule<"mysql"> = { dialect: "mysql", sql: "select :f as f", params: ["f"], inputs: { f: s("int4") } };
  seen.length = 0;
  await execute({ query: async () => [[], []] as [unknown, unknown], execute: async (_: string, v?: unknown[]) => (seen.push(v!), [[], []] as [unknown, unknown]) }, mq, { f: true });
  assert.deepEqual([...seen], [[true]]);
  // Postgres refuses `+010000` and the year 0000, which run returns for years past 9999 and 1 BC.
  const pq: QueryModule<"postgres"> = {
    dialect: "postgres",
    sql: "select :far::timestamptz, :bc::timestamptz, :zero::date, :now::timestamptz, :days::date[]",
    params: ["far", "bc", "zero", "now", "days"],
    inputs: { far: s("timestamptz"), bc: s("timestamptz"), zero: s("date"), now: s("timestamptz"), days: { type: { kind: "array", of: { kind: "scalar", name: "date" } } } },
  };
  const values = { far: "+010000-01-01T00:00:00Z", bc: "-000001-06-01T12:00:00.5Z", zero: "0000-01-01", now: "2026-10-20T10:00:00Z", days: ["+010000-01-01", "2026-10-20"] };
  seen.length = 0;
  await execute({ query: async (_: string, v?: unknown[]) => (seen.push(v!), { rows: [] }) }, pq, values);
  assert.deepEqual([...seen], [["10000-01-01T00:00:00Z", "0002-06-01T12:00:00.5Z BC", "0001-01-01 BC", "2026-10-20T10:00:00Z", '{"10000-01-01","2026-10-20"}']]);
  // Without inputs, as defined by hand, every value goes as it is.
  const { inputs: _, ...bare } = lq;
  seen.length = 0;
  await execute({ execute: async (st: { args: unknown[] }) => (seen.push(st.args), { rows: [] }), executeMultiple: async () => {} }, bare, { a: true, b: false, n: 7 });
  assert.deepEqual([...seen], [[true, false, 7]]);
});

test("arrayLiteral writes a Postgres array literal as pg does: nesting, NULL, quotes, backslashes, binary, dates, the empty array", () => {
  assert.equal(arrayLiteral([]), "{}");
  assert.equal(arrayLiteral([1, 2n, 1.5, true]), '{"1","2","1.5","true"}');
  assert.equal(arrayLiteral([[1, 2], [3, null], []]), '{{"1","2"},{"3",NULL},{}}');
  assert.equal(arrayLiteral([null, undefined, "", "NULL"]), '{NULL,NULL,"","NULL"}');
  assert.equal(arrayLiteral(['say "hi"', "back\\slash", "a,b", "{x}", " pad "]), String.raw`{"say \"hi\"","back\\slash","a,b","{x}"," pad "}`);
  assert.equal(arrayLiteral([Uint8Array.of(1, 2, 255), Buffer.from([0, 16]), new Uint8Array([9, 171, 9]).subarray(1, 2)]), String.raw`{\\x0102ff,\\x0010,\\xab}`);
  assert.equal(arrayLiteral([{ a: 'q"' }]), String.raw`{"{\"a\":\"q\\\"\"}"}`);
  // pg's dateToString: the local time, then the offset.
  const d = new Date(2026, 0, 2, 3, 4, 5, 6);
  const offset = -d.getTimezoneOffset();
  const zone = `${offset < 0 ? "-" : "+"}${String(Math.floor(Math.abs(offset) / 60)).padStart(2, "0")}:${String(Math.abs(offset) % 60).padStart(2, "0")}`;
  assert.equal(arrayLiteral([d]), `{"2026-01-02T03:04:05.006${zone}"}`);
});

test("postgres: Neon's neon() before 1.0 is called with (text, values)", async () => {
  const calls: Call[] = [];
  const sql = (...args: unknown[]) => (calls.push({ method: "call", args }), Promise.resolve([row]));
  assert.deepEqual(await execute(sql, pg, { id: 1, name: "ada" }), [row]);
  assert.deepEqual(calls[0]!.args.slice(0, 2), ["select $1::int as id, $2 as name, $1 as again", [1, "ada"]]);
  // Its options take a decoder table, 0.9 too, and the fields with fullResults.
  assert.deepEqual(Object.keys(calls[0]!.args[2] as object), ["fullResults", "arrayMode", "types"]);
});

test("a parameter left out binds NULL, which postgres.js, mysql2, node:sqlite and libSQL would reject", async () => {
  const seen: unknown[][] = [];
  const client = { unsafe: async (_: string, values?: unknown[]) => (seen.push(values!), []) };
  await execute(client, pg, { id: 1 });
  await execute(client, { ...pg, text: "select $1, $2", order: ["name", "id"] }, { id: undefined });
  assert.deepEqual(seen, [[1, null], [null, null]]);
});

test("mysql: execute() → [rows, fields] on mysql2, a repeated name repeated in the values; a write's OK packet gives no rows", async () => {
  const calls: Call[] = [];
  const answers: unknown[] = [[[row], [{ name: "id" }]], [{ affectedRows: 1, insertId: 7 }, undefined]];
  const pool = { query: async () => [[], []] as [unknown, unknown], execute: async (...args: unknown[]) => (calls.push({ method: "execute", args }), answers.shift() as [unknown, unknown]) };
  assert.deepEqual(await execute(pool, my, { id: 1, name: "ada" }), [row]);
  // The statement with Reylo's per-query options and typeCast (src/decode-mysql.ts), never decimalNumbers.
  const [options, values] = calls[0]!.args as [Record<string, unknown>, unknown[]];
  assert.deepEqual(values, [1, "ada", 1]);
  assert.deepEqual({ ...options, typeCast: typeof options.typeCast }, { sql: "select ? as id, ? as name, ? as again", supportBigNumbers: true, bigNumberStrings: true, dateStrings: true, rowsAsArray: false, nestTables: false, typeCast: "function" });
  assert.deepEqual(await execute(pool, my, { id: 1, name: "ada" }), []);
});

test("mysql: PlanetScale's execute() → { rows }", async () => {
  const conn = { execute: async () => ({ rows: [row], statement: "select", headers: ["id"], insertId: "0" }) };
  assert.deepEqual(await execute(conn, my, { id: 1, name: "ada" }), [row]);
});

test("mysql: mariadb runs through query(), not its own execute(); a write's OK packet gives no rows", async () => {
  const calls: string[] = [];
  const answers: unknown[] = [Object.assign([row], { meta: [] }), { affectedRows: 1, insertId: 7n, warningStatus: 0 }];
  const conn = {
    query: async () => (calls.push("query"), answers.shift()),
    batch: async () => (calls.push("batch"), {}),
    execute: async () => (calls.push("execute"), [row]),
  };
  assert.deepEqual([...((await execute(conn, my, { id: 1, name: "ada" })) as unknown[])], [row]);
  assert.deepEqual(await execute(conn, my, { id: 1, name: "ada" }), []);
  assert.deepEqual(calls, ["query", "query"]);
});

test("mysql: mariadb's callback API answers through the callback, and its errors reject", async () => {
  const failure = Object.assign(new Error("Duplicate entry 'a' for key 'email'"), { code: "ER_DUP_ENTRY" });
  const conn = {
    batch: () => ({}),
    // The callback query() returns a command object, not a promise.
    query: (options: { sql: string }, values: unknown[], done: (e: Error | null, rows?: unknown) => void) => (setImmediate(() => (options.sql.startsWith("insert") ? done(failure) : done(null, [{ values }]))), { on() {} }),
  };
  assert.deepEqual(await execute(conn as never, my, { id: 1, name: "ada" }), [{ values: [1, "ada", 1] }]);
  await assert.rejects(() => execute(conn as never, { dialect: "mysql", sql: "insert into t values (:a)", params: ["a"] }, { a: 1 }) as Promise<unknown>, (e) => e === failure);
});

// A plain Uint8Array (TextEncoder's, getRandomValues') and a view into a larger buffer, neither a Buffer.
const binary = () => ({ plain: Uint8Array.of(1, 2, 255), view: new Uint8Array([9, 1, 2, 255, 9]).subarray(1, 4) });
const bin: QueryModule<"mysql"> = { dialect: "mysql", sql: "select hex(:plain), hex(:view), hex(:buf), :n", params: ["plain", "view", "buf", "n"] };

test("mysql: mariadb, which writes a value that is not a Buffer as JSON, gets a binary value as a Buffer over the same memory", async () => {
  const { plain, view } = binary();
  const buf = Buffer.from([1, 2, 255]);
  for (const api of ["promise", "callback"]) {
    const seen: unknown[][] = [];
    const conn =
      api === "promise"
        ? { batch: async () => ({}), query: async (_: string, values: unknown[]) => (seen.push(values), []) }
        : { batch: () => ({}), query: (_: string, values: unknown[], done: (e: Error | null, rows?: unknown) => void) => (seen.push(values), setImmediate(() => done(null, [])), { on() {} }) };
    await execute(conn as never, bin, { plain, view, buf, n: 1 });
    const [a, b, c, n] = seen[0]!;
    assert.ok(Buffer.isBuffer(a) && Buffer.isBuffer(b), api);
    assert.deepEqual([...(a as Buffer)], [1, 2, 255]);
    assert.deepEqual([...(b as Buffer)], [1, 2, 255]);
    // Views, not copies: the same memory at the same offset.
    assert.ok((a as Buffer).buffer === plain.buffer && (b as Buffer).buffer === view.buffer && (b as Buffer).byteOffset === 1);
    assert.equal(c, buf);
    assert.equal(n, 1);
  }
});

test("postgres: Neon's neon() before 0.10, which sends a Buffer as JSON, gets a binary value in Postgres's hex form", async () => {
  const { plain, view } = binary();
  const seen: unknown[][] = [];
  const sql = (_: string, values?: unknown[]) => (seen.push(values!), Promise.resolve([]));
  const q: QueryModule<"postgres"> = { dialect: "postgres", sql: "select :plain::bytea, :view::bytea, :buf::bytea, :empty::bytea, :n", params: ["plain", "view", "buf", "empty", "n"] };
  await execute(sql, q, { plain, view, buf: Buffer.from([0, 16, 171]), empty: new Uint8Array(0), n: 1 });
  assert.deepEqual(seen[0], ["\\x0102ff", "\\x0102ff", "\\x0010ab", "\\x", 1]);
});

test("every other driver gets a binary value as it is, pg-promise included, since it binds a { text, values } object", async () => {
  const { plain } = binary();
  const seen: unknown[] = [];
  const pgq: QueryModule<"postgres"> = { dialect: "postgres", sql: "select :b::bytea", params: ["b"] };
  const liteq: QueryModule<"sqlite"> = { dialect: "sqlite", sql: "select :b", params: ["b"] };
  const pgp = { manyOrNone: async () => [], query: async (q: any): Promise<any> => (seen.push(q.values[0]), []) };
  await execute(pgp, pgq, { b: plain });
  await execute({ query: async (_: string, values?: unknown[]) => (seen.push(values![0]), { rows: [] }) }, pgq, { b: plain });
  await execute({ unsafe: async (_: string, values?: unknown[]) => (seen.push(values![0]), []) }, pgq, { b: plain });
  await execute({ query: async () => [[], []] as [unknown, unknown], execute: async (_: unknown, values?: unknown[]) => (seen.push(values![0]), [[], []] as [unknown, unknown]) }, { ...bin, sql: "select hex(:b)", params: ["b"] }, { b: plain });
  await execute({ prepare: () => ({ all: (...values: unknown[]) => (seen.push(values[0]), []) }) }, liteq, { b: plain });
  assert.equal(seen.length, 5);
  for (const v of seen) assert.equal(v, plain);
});

test("mysql: PlanetScale 2.0, which binds no parameters, runs a statement without any and refuses one with some", async () => {
  const calls: unknown[][] = [];
  // 2.0's execute(query, options) refuses anything past them, or a second argument that is not its options.
  const conn = {
    execute: async (...args: unknown[]) => {
      calls.push(args);
      if (args.length > 2 || (args[1] != null && (typeof args[1] !== "object" || Array.isArray(args[1])))) throw new Error("Query parameters are not supported: pass a complete SQL string and options to execute(query, options)");
      return { rows: [row], statement: String(args[0]), headers: [] };
    },
  };
  assert.deepEqual(await execute(conn, { dialect: "mysql", sql: "select 1", params: [] }), [row]);
  await assert.rejects(() => execute(conn, my, { id: 1, name: "ada" }) as Promise<unknown>, (e: TypeError) => e instanceof TypeError && /2\.0 binds no parameters/.test(e.message) && /not supported/.test((e.cause as Error).message));
  // 1.x's form first, then 2.0's, with Reylo's cast either way.
  assert.deepEqual(calls[0]!.slice(0, 2), ["select 1", null]);
  assert.deepEqual([calls[1]![0], Object.keys(calls[1]![1] as object)], ["select 1", ["as", "cast"]]);
});

test("mysql: a mysql2 callback pool or connection runs through its promise() wrapper", async () => {
  const callback = {
    // The callback execute() returns a Query object, not a promise.
    execute: () => ({ on() {} }),
    promise: () => ({ query: async () => [[], []] as [unknown, unknown], execute: async () => [[row], []] as [unknown, unknown] }),
  };
  assert.deepEqual(await execute(callback, my, { id: 1, name: "ada" }), [row]);
});

test("mysql: Bun.sql's unsafe()", async () => {
  assert.deepEqual(await execute({ unsafe: async () => [row], commitDistributed: async () => {} }, my, { id: 1, name: "ada" }), [row]);
});

test("sqlite: node:sqlite answers at once, with one ? per placeholder; a write gives no rows", () => {
  const db = new DatabaseSync(":memory:");
  db.exec("create table t (a int)");
  const rows = execute(db, lite, { id: 1, name: "ada" });
  assert.ok(Array.isArray(rows), "a synchronous client's rows are not a promise");
  assert.deepEqual(rows.map((r) => ({ ...(r as object) })), [row]);
  assert.deepEqual(execute(db, { dialect: "sqlite", sql: "insert into t values (:a)", params: ["a"] }, { a: 2 }), []);
  assert.deepEqual(db.prepare("select a from t").all().map((r) => ({ ...r })), [{ a: 2 }]);
});

test("sqlite: better-sqlite3's statement that returns no rows is run(), since its all() throws", () => {
  const calls: Call[] = [];
  const db = {
    prepare: (sql: string) => ({
      reader: !sql.startsWith("insert"),
      all: (...args: unknown[]) => {
        if (sql.startsWith("insert")) throw new TypeError("This statement does not return data. Use run() instead");
        return calls.push({ method: "all", args }), [row];
      },
      run: (...args: unknown[]) => (calls.push({ method: "run", args }), { changes: 1 }),
    }),
  };
  assert.deepEqual(execute(db, { dialect: "sqlite", sql: "insert into t values (:a)", params: ["a"] }, { a: 2 }), []);
  assert.deepEqual(execute(db, lite, { id: 1, name: "ada" }), [row]);
  assert.deepEqual(calls, [{ method: "run", args: [2] }, { method: "all", args: [1, "ada", 1] }]);
});

test("sqlite: Cloudflare D1's prepare(sql).bind(...values).all() → { results }", async () => {
  const bound: unknown[][] = [];
  const d1 = {
    batch: async () => [],
    prepare: () => ({ first: async () => null, bind: (...values: unknown[]) => (bound.push(values), { all: async () => ({ results: [row], success: true, meta: {} }) }) }),
  };
  const rows = execute(d1, lite, { id: 1, name: "ada" });
  assert.ok(rows instanceof Promise);
  assert.deepEqual(await rows, [row]);
  assert.deepEqual(bound, [[1, "ada", 1]]);
});

test("sqlite: @libsql/client and its transactions take execute({ sql, args }) → { rows }", async () => {
  const calls: unknown[] = [];
  const tx = { execute: async (stmt: { sql: string; args: unknown[] }) => (calls.push(stmt), { rows: [row], columns: ["id"], rowsAffected: 0 }), executeMultiple: async () => {}, commit: async () => {} };
  assert.deepEqual(await execute(tx, lite, { id: 1, name: "ada" }), [row]);
  assert.deepEqual(calls, [{ sql: "select ? as id, ? as name, ? as again", args: [1, "ada", 1] }]);
});

test("sqlite: Turso's @tursodatabase/database, its Database in every release and its Transaction, takes prepare(sql).all(...values) → a promise of rows; its compat Database answers at once", async () => {
  const calls: unknown[] = [];
  // Its statement, as every release from 0.4 has it: safeIntegers(), a promise from all(), close().
  const statement = (sql: string) => {
    const st = {
      safe: undefined as boolean | undefined,
      closed: false,
      safeIntegers: (on: boolean) => ((st.safe = on), st),
      all: async (...values: unknown[]) => (calls.push({ sql, values, safe: st.safe }), [row]),
      close: () => void (st.closed = true),
      get reader(): boolean {
        throw new Error("not implemented"); // 0.4's
      },
    };
    return st;
  };
  const statements: ReturnType<typeof statement>[] = [];
  const prepare = (sql: string) => (statements.push(statement(sql)), statements.at(-1)!);
  const common = { connect: async () => {}, maxWriteReplicationIndex: () => {}, exec: async () => {}, prepare };
  // 0.4: no all() on the database, so it looked synchronous; 0.6 and later: all() and exec(), so it looked like sqlite3's.
  const v04 = { ...common };
  const v08 = { ...common, all: async () => [], batch: async () => [], transactionAsync: () => {} };
  const tx = { prepare, exec: async () => {}, all: async () => [], batch: async () => [], run: async () => ({}) };
  for (const client of [v04, v08, tx]) {
    const pending = execute(client as never, lite, { id: 1, name: "ada" });
    assert.ok(pending instanceof Promise);
    assert.deepEqual(await pending, [row]);
  }
  assert.deepEqual(calls, Array(3).fill({ sql: "select ? as id, ? as name, ? as again", values: [1, "ada", 1], safe: true }), "a statement whose columns are not typed reads integers exact");
  assert.ok(statements.every((st) => st.closed), "each statement is closed after its run");
  // A module whose every column is typed reads numbers, as better-sqlite3's statement does.
  await execute(v08 as never, { ...lite, columns: [["id", "int"], ["name", "text"], ["again", "int"]] }, { id: 1, name: "ada" });
  assert.equal((calls.at(-1) as { safe: boolean }).safe, false);
  await assert.rejects(Promise.resolve().then(() => defineQuery<{}, unknown, "many", "sqlite">({ dialect: "sqlite", sql: "select 1", params: [] }).runSync(v08 as never)), /runSync takes a synchronous SQLite client .*answers through a promise/);
  // The compat Database (@tursodatabase/database/compat) has batch() as D1 does, and answers at once.
  // It gets a statement per call, closed after it (0.4's, run a second time, lost a subquery's rows).
  let prepared = 0;
  let closed = 0;
  const compat = {
    prepare: (sql: string) => (prepared++, { reader: true, safeIntegers: () => {}, all: (...values: unknown[]) => (calls.push({ sql, values }), [row]), close: () => void closed++ }),
    batch: () => [],
    exec: () => {},
    maxWriteReplicationIndex: () => {},
  };
  const q = defineQuery<{ id: number; name: string }, unknown, "many", "sqlite">({ dialect: "sqlite", sql: lite.sql, params: lite.params });
  assert.deepEqual(q.runSync(compat, { id: 2, name: "bo" }), [row]);
  assert.deepEqual(q.runSync(compat, { id: 2, name: "bo" }), [row]);
  assert.deepEqual(calls.at(-1), { sql: "select ? as id, ? as name, ? as again", values: [2, "bo", 2] });
  assert.deepEqual([prepared, closed], [2, 2]);
});

test("sqlite: sqlite3 and the sqlite package, which bind a BigInt as NULL, get one a number holds as that number, a larger one its digits where the parameter is a BIGINT column's, and is refused elsewhere", async () => {
  const seen: unknown[][] = [];
  const sqlite3 = { all: (_: string, v: unknown[], cb: (e: Error | null, rows: unknown[]) => void) => (seen.push(v), cb(null, [])), exec: () => {} };
  const sqlite = { all: async (_: string, v?: unknown[]) => (seen.push(v!), []), getDatabaseInstance: () => ({}) };
  await execute(sqlite3, lite, { id: BigInt(7), name: "ada" });
  await execute(sqlite, lite, { id: BigInt(-7), name: "ada" });
  assert.deepEqual([...seen], [[7, "ada", 7], [-7, "ada", -7]]);
  for (const client of [sqlite3, sqlite]) await assert.rejects(Promise.resolve().then(() => execute(client, lite, { id: BigInt("9007199254740993"), name: "ada" })), /^TypeError: sqlite3 binds a BigInt as NULL, and 9007199254740993 is past 2\^53 - 1/);
  assert.equal(seen.length, 2);
  // A BIGINT column's parameter (the engine's int8), whose INTEGER affinity reads the digits as that integer.
  const big: QueryModule<"sqlite"> = { ...lite, inputs: { id: { type: { kind: "scalar", name: "int8" } }, name: { type: { kind: "scalar", name: "text" } } } };
  await execute(sqlite3, big, { id: BigInt("9007199254740993"), name: "ada" });
  await execute(sqlite, big, { id: BigInt("-9007199254740993"), name: "ada" });
  assert.deepEqual([...seen].slice(2), [["9007199254740993", "ada", "9007199254740993"], ["-9007199254740993", "ada", "-9007199254740993"]]);
});

test("sqlite: sqlite3's callback all(), its errors, and the sqlite package's promise all()", async () => {
  const failure = Object.assign(new Error("SQLITE_ERROR: near \"selec\": syntax error"), { code: "SQLITE_ERROR" });
  const sqlite3 = {
    prepare: () => ({}),
    exec: () => {},
    all: (sql: string, values: unknown[], done: (e: Error | null, rows: unknown[]) => void) => setImmediate(() => (sql.includes("selec ") ? done(failure, []) : done(null, [{ values }]))),
  };
  assert.deepEqual(await execute(sqlite3, lite, { id: 1, name: "ada" }), [{ values: [1, "ada", 1] }]);
  await assert.rejects(() => execute(sqlite3, { dialect: "sqlite", sql: "selec 1", params: [] }) as Promise<unknown>, (e) => e === failure);
  const wrapper = { getDatabaseInstance: () => sqlite3, prepare: async () => ({}), all: async (_: string, values: unknown[]) => [{ values }] };
  assert.deepEqual(await execute(wrapper, lite, { id: 2, name: "bo" }), [{ values: [2, "bo", 2] }]);
});

test("sqlite: Bun.sql's SQLite adapter, which has unsafe() and no prepare()", async () => {
  assert.deepEqual(await execute({ unsafe: async () => [row], commitDistributed: async () => {} }, lite, { id: 1, name: "ada" }), [row]);
});

test("the driver's own error comes through unchanged", async () => {
  const duplicate = Object.assign(new Error("duplicate key value violates unique constraint"), { code: "23505" });
  await assert.rejects(() => execute({ query: async () => Promise.reject(duplicate) }, pg, { id: 1 }) as Promise<unknown>, (e) => e === duplicate);
  const db = new DatabaseSync(":memory:");
  assert.throws(() => execute(db, { dialect: "sqlite", sql: "selec 1", params: [] }), /syntax error/);
});

test("an unsupported client rejects, naming what it has and what reylo takes", async () => {
  await assert.rejects(() => execute({ connect() {}, end() {} } as never, pg) as Promise<unknown>, (e: TypeError) => e instanceof TypeError && /postgres statement on an object with connect, end\. It takes a client with query\(text, values\)/.test(e.message));
  // An object with Drizzle's select() but no session over a client is no Drizzle database.
  await assert.rejects(() => execute({ $client: {}, select() {} } as never, pg) as Promise<unknown>, /object with select\. .*or a Drizzle database over one of them\./);
  // Kysely and Prisma decode their values themselves, so each is told to pass the driver's client.
  class Kysely { selectFrom() {} executeQuery() {} transaction() {} }
  await assert.rejects(() => execute(new Kysely() as never, pg) as Promise<unknown>, /object with selectFrom, executeQuery, transaction\. .* Kysely decodes the values of its queries itself: pass the pool or database its dialect was made with\.$/);
  await assert.rejects(() => execute({ $queryRawUnsafe() {}, $transaction() {} } as never, my) as Promise<unknown>, / Prisma decodes the values of its queries itself: pass a client of your database's driver, such as the pool its driver adapter was made with\.$/);
  // Inside their transactions, neither hands out the connection.
  await assert.rejects(() => execute(Object.assign(new Kysely(), { isTransaction: true }) as never, pg) as Promise<unknown>, / Kysely decodes the values of its queries itself and does not hand out its transaction's connection: take the transaction from the driver's client instead\.$/);
  await assert.rejects(() => execute({ $queryRawUnsafe() {} } as never, lite) as Promise<unknown>, / Prisma decodes the values of its queries itself and does not hand out its transaction's connection/);
  // node:sqlite's tag store has all() for templates; it is not sqlite3's database.
  const store = new DatabaseSync(":memory:").createTagStore();
  await assert.rejects(() => execute(store as never, lite, { id: 1, name: "ada" }) as Promise<unknown>, /sqlite statement on an object with .*pass its database, store\.db/);
  await assert.rejects(() => execute(null as never, my) as Promise<unknown>, /mysql statement on null/);
  // A pg pool handed to a SQLite statement matches no SQLite shape.
  await assert.rejects(() => execute({ query: async () => ({ rows: [] }) } as never, lite) as Promise<unknown>, /sqlite statement on an object with query/);
});

test("a Drizzle database or transaction runs on the driver client its session keeps", async () => {
  // Drizzle's database and transaction objects: select() beside a session holding the dialect and the client.
  const drizzle = <C>(client: C) => ({ $client: client, select() {}, execute() {}, session: { dialect: {}, client } });
  const seen: unknown[][] = [];
  const pgClient = { query: async (_: string, values: unknown[]) => (seen.push(values), { rows: [row] }) };
  assert.deepEqual(await execute(drizzle(pgClient) as never, pg, { id: 1, name: "ada" }), [row]);
  const mysqlPool = { query: async () => [[], []], execute: async (_: unknown, values: unknown[]) => (seen.push(values), [[row], []]) };
  assert.deepEqual(await execute(drizzle(mysqlPool) as never, my, { id: 1, name: "ada" }), [row]);
  assert.deepEqual(seen, [[1, "ada"], [1, "ada", 1]]);
  const db = new DatabaseSync(":memory:");
  assert.deepEqual(executeSync(drizzle(db) as never, lite, { id: 1, name: "ada" }), [row]);
  // Inside a libSQL transaction the session keeps the base client on client and the transaction on tx.
  const libsql = (name: string) => ({ execute: async () => (seen.push([name]), { rows: [row], columns: ["id", "name", "again"], columnTypes: [] }), batch: async () => [], transaction: async () => {} });
  const tx = { select() {}, session: { dialect: {}, client: libsql("base"), tx: { ...libsql("tx"), commit: async () => {}, rollback: async () => {} } } };
  await execute(tx as never, lite, { id: 1, name: "ada" });
  assert.deepEqual(seen.at(-1), ["tx"]);
  // A MySQL client whose execute() answers without PlanetScale's rows, as TiDB Cloud's transaction inside
  // drizzle-orm/tidb-serverless answers with the rows themselves, is refused rather than read as no rows.
  const tidb = { $client: {}, select() {}, rollback() {}, session: { dialect: {}, client: { execute: async () => [row] } } };
  await assert.rejects(() => execute(tidb as never, my, { id: 1, name: "ada" }) as Promise<unknown>, /^TypeError: reylo ran this statement through execute\(\), as on PlanetScale's @planetscale\/database, and the client answered with an array rather than a result with rows\./);
});

test("positional placeholders take an object keyed by number, bound by number on every dialect", async () => {
  const seen: [string, unknown[]][] = [];
  const pgClient = { query: async (text: string, values: unknown[]) => (seen.push([text, values]), { rows: [] }) };
  const mysqlPool = { query: async () => [[], []] as [unknown, unknown], execute: async (options: { sql: string }, values: unknown[]) => (seen.push([options.sql, values]), [[], []] as [unknown, unknown]) };
  const d1 = { batch: async () => [], prepare: (text: string) => ({ first: async () => null, bind: (...values: unknown[]) => (seen.push([text, values]), { all: async () => ({ results: [] }) }) }) };
  const positional: Record<string, QueryModule> = {
    postgres: { dialect: "postgres", sql: "select $2::text as name, $1::int as id", params: ["1", "2"] },
    mysql: { dialect: "mysql", sql: "select ? as id, ? as name", params: ["1", "2"] },
    sqlite: { dialect: "sqlite", sql: "select ?1 as id, ?2 as name, ?1 as again", params: ["1", "2"] },
  };
  for (const [client, dialect] of [[pgClient, "postgres"], [mysqlPool, "mysql"], [d1, "sqlite"]] as const) await execute(client as never, positional[dialect]!, { 1: 1, 2: "ada" });
  assert.deepEqual(seen, [
    ["select $2::text as name, $1::int as id", [1, "ada"]],
    ["select ? as id, ? as name", [1, "ada"]],
    ["select ? as id, ? as name, ? as again", [1, "ada", 1]],
  ]);
  // The mapper's rewritten statement binds by `order`, which names params.
  const unsafe: unknown[][] = [];
  await execute({ unsafe: async (_: string, values: unknown[]) => (unsafe.push(values), []) }, { ...pg, text: "select $1, $2, $1", order: ["name", "id", "name"] }, { id: 1, name: "ada" });
  assert.deepEqual(unsafe, [["ada", 1, "ada"]]);
});

test("an array of parameters is refused before anything runs, with a TypeError that says to pass an object", async () => {
  const calls: unknown[] = [];
  const client = { unsafe: async (...args: unknown[]) => (calls.push(args), []) };
  const named = /^TypeError: The parameters must be an object keyed by parameter name, or by number for positional placeholders \(\{ 1: \.\.\., 2: \.\.\. \}\); got an array\. This statement takes \{ id: \.\.\., name: \.\.\. \}\.$/;
  const refused = (re: RegExp) => (e: Error) => e instanceof TypeError && re.test(String(e));
  // A freshness check would be a round trip: the array is refused before it too.
  await assert.rejects(() => execute(client, { ...pg, fresh: { query: "select 1", rows: [], inserts: [] } }, [1, "ada"] as never) as Promise<unknown>, refused(named));
  const positional: QueryModule<"postgres"> = { dialect: "postgres", sql: "select $1::int, $2::text", params: ["1", "2"] };
  await assert.rejects(() => execute(client, positional, [1, "ada"] as never) as Promise<unknown>, refused(/got an array\. This statement takes \{ 1: \.\.\., 2: \.\.\. \}\.$/));
  await assert.rejects(() => execute(client, pg, "ada" as never) as Promise<unknown>, refused(/positional placeholders \(\{ 1: \.\.\., 2: \.\.\. \}\); got ada\. This statement takes \{ id: \.\.\., name: \.\.\. \}\.$/));
  await assert.rejects(() => execute(client, pg, null as never) as Promise<unknown>, refused(/got null\./));
  assert.deepEqual(calls, []);
  // On a synchronous SQLite client the TypeError is thrown.
  const db = new DatabaseSync(":memory:");
  assert.throws(() => execute(db, lite, [1, "ada"] as never), refused(named));
  assert.throws(() => execute(db, { dialect: "sqlite", sql: "select 1", params: [] }, [] as never), refused(/got an array\. This statement takes none: leave them out\.$/));
  // Through the query object: run and with reject, runSync throws, and the types refuse it too.
  const q = defineQuery<{ id: number; name: string }, { id: number; name: string; again: number }, "one", "sqlite">({ dialect: "sqlite", sql: lite.sql, params: lite.params, cardinality: "one" });
  // @ts-expect-error the parameters are an object, never an array
  await assert.rejects(() => q.run(db, [1, "ada"]), refused(named));
  // @ts-expect-error the parameters are an object, never an array
  await assert.rejects(() => q.with(db)([1, "ada"]), refused(named));
  // @ts-expect-error the parameters are an object, never an array
  assert.throws(() => q.runSync(db, [1, "ada"]), refused(named));
  assert.deepEqual({ ...(q.runSync(db, { id: 1, name: "ada" }) as object) }, row);
  const p = defineQuery<{ 1: number; 2: string }, { id: number }, "one", "postgres">({ dialect: "postgres", sql: positional.sql, params: positional.params, cardinality: "one" });
  // @ts-expect-error positional placeholders take an object keyed by number
  await assert.rejects(() => p.run(client, [1, "ada"]), refused(/This statement takes \{ 1: \.\.\., 2: \.\.\. \}\./));
  db.close();
});

test("runSync: on a synchronous client the value comes back at once and an error is thrown, so db.transaction(fn) rolls back", () => {
  const db = new DatabaseSync(":memory:");
  db.exec("create table users (id integer primary key, email text unique)");
  const insert = defineQuery<{ email: string }, { id: number }, "one", "sqlite">({ dialect: "sqlite", sql: "insert into users (email) values (:email) returning id", params: ["email"], cardinality: "one" });
  const count = defineQuery<{}, { n: number }, "one", "sqlite">({ dialect: "sqlite", sql: "select count(*) as n from users", params: [], cardinality: "one" });
  // better-sqlite3's and bun:sqlite's transaction(fn): BEGIN, the function, then COMMIT, or ROLLBACK if it throws.
  const transaction = (fn: () => void) => {
    db.exec("begin");
    try {
      fn();
      db.exec("commit");
    } catch (e) {
      db.exec("rollback");
      throw e;
    }
  };
  const first = insert.runSync(db, { email: "ada@example.com" });
  assert.equal(first.id, 1);
  assert.throws(() => transaction(() => {
    insert.runSync(db, { email: "bo@example.com" });
    insert.runSync(db, { email: "ada@example.com" }); // the unique constraint fails here
  }), /UNIQUE constraint failed/);
  assert.equal(count.runSync(db).n, 1, "the transaction's first insert was rolled back");
  assert.throws(() => defineQuery<{}, unknown, "one", "sqlite">({ dialect: "sqlite", sql: "select 1 where 0", params: [], cardinality: "one" }).runSync(db), /Expected exactly one row, got 0/);
});

test("run: on a synchronous client the statement runs at once, and its rows and its error still come through the promise", async () => {
  const db = new DatabaseSync(":memory:");
  db.exec("create table users (id integer primary key, email text unique)");
  const insert = defineQuery<{ email: string }, { id: number }, "one", "sqlite">({ dialect: "sqlite", sql: "insert into users (email) values (:email) returning id", params: ["email"], cardinality: "one" });
  const pending = insert.run(db, { email: "ada@example.com" });
  assert.ok(pending instanceof Promise);
  assert.equal((db.prepare("select count(*) as n from users").get() as { n: number }).n, 1, "the insert ran before run returned");
  assert.equal((await pending).id, 1);
  let duplicate: unknown;
  assert.doesNotThrow(() => void (duplicate = insert.run(db, { email: "ada@example.com" })));
  await assert.rejects(duplicate as Promise<unknown>, /UNIQUE constraint failed/);
});

test("runSync: a client that answers through a promise, or a Postgres or MySQL statement, is refused before anything runs", () => {
  let called = false;
  const libsql = { execute: async () => ((called = true), { rows: [] }), executeMultiple: async () => {} };
  const lite = defineQuery<{}, unknown, "many", "sqlite">({ dialect: "sqlite", sql: "select 1", params: [] });
  assert.throws(() => lite.runSync(libsql as never), /runSync takes a synchronous SQLite client .*answers through a promise\. run takes every client/);
  const pgq = defineQuery<{}, unknown, "many", "postgres">({ dialect: "postgres", sql: "select 1", params: [] });
  assert.throws(() => pgq.runSync({ query: async () => ((called = true), { rows: [] }) } as never), /a postgres statement has none/);
  assert.equal(called, false);
});

test("run: on an asynchronous client every failure is a rejection, never a throw", async () => {
  const q = defineQuery<{ id: number }, { id: number }, "maybeOne", "postgres">({ dialect: "postgres", sql: "select :id::int as id", params: ["id"], cardinality: "maybeOne" });
  const two = { query: async () => ({ rows: [{ id: 1 }, { id: 2 }] }) };
  let returned: unknown;
  assert.doesNotThrow(() => void (returned = q.run(two, { id: 1 })));
  await assert.rejects(returned as Promise<unknown>, /Expected at most one row, got 2/);
  await assert.rejects(q.run({} as never, { id: 1 }), TypeError);
  assert.deepEqual(await q.with({ query: async () => ({ rows: [] }) })({ id: 1 }), null);
});

test("a query's source names its file, and Reylo's own errors about the query start with it", async () => {
  const db = new DatabaseSync(":memory:");
  db.exec("create table users (id integer primary key, email text unique)");
  const one = defineQuery<{}, unknown, "one", "sqlite">({ dialect: "sqlite", sql: "select 1 where 0", params: [], cardinality: "one", source: "queries/none.sql" });
  assert.equal(one.source, "queries/none.sql");
  assert.throws(() => one.runSync(db), /^Error: queries\/none\.sql: Expected exactly one row, got 0$/m);
  await assert.rejects(one.run(db), { message: "queries/none.sql: Expected exactly one row, got 0" });
  // A parameter's value: thrown in runSync, rejected by run, the stack's first line named too.
  const at = defineQuery<{ at: string }, unknown, "many", "postgres">({ dialect: "postgres", sql: "select :at::timestamptz as at", params: ["at"], inputs: { at: { type: { kind: "scalar", name: "timestamptz" } } }, source: "at.sql" });
  const rejected = await at.run({ query: async () => ({ rows: [] }) }, { at: "2026-10-12 19:06:48" }).then(() => undefined, (e: Error) => e);
  assert.ok(rejected instanceof TypeError);
  assert.match(rejected.message, /^at\.sql: Parameter "at" is a timestamptz/);
  assert.match(rejected.stack!, /^TypeError: at\.sql: Parameter "at"/);
  const viaWith = await at.with({ query: async () => ({ rows: [] }) })({ at: new Date() as never }).then(() => undefined, (e: Error) => e);
  assert.match(viaWith!.stack!, /^TypeError: at\.sql: Parameter "at" is a timestamptz, which takes the instant as a string/);
  // A column's value Reylo cannot give its kind.
  const truth = defineQuery<{}, unknown, "many", "sqlite">({ dialect: "sqlite", sql: "select 2 as b", params: [], columns: [["b", "bool"]], source: "truth.sql" });
  assert.throws(() => truth.runSync(db), (e: Error) => e instanceof TypeError && /^truth\.sql: Column "b" is a boolean/.test(e.message));
  await assert.rejects(truth.run(db), (e: Error) => /^truth\.sql: Column "b" is a boolean/.test(e.message));
  // The driver's own error passes through as it is.
  const insert = defineQuery<{ email: string }, unknown, "none", "sqlite">({ dialect: "sqlite", sql: "insert into users (email) values (:email)", params: ["email"], cardinality: "none", source: "insert.sql" });
  insert.runSync(db, { email: "ada@example.com" });
  assert.throws(() => insert.runSync(db, { email: "ada@example.com" }), (e: Error) => /^UNIQUE constraint failed/.test(e.message));
  await assert.rejects(insert.run(db, { email: "ada@example.com" }), (e: Error) => /^UNIQUE constraint failed/.test(e.message));
  // A query defined by hand, with no file, keeps its messages as they are.
  const bare = defineQuery<{}, unknown, "one", "sqlite">({ dialect: "sqlite", sql: "select 1 where 0", params: [], cardinality: "one" });
  assert.equal(bare.source, undefined);
  assert.throws(() => bare.runSync(db), { message: "Expected exactly one row, got 0" });
});

// A SQLite-backed Durable Object's ctx.storage.sql, as workerd's answers
// (lab/driver-matrix/probes/durable-object.mjs): exec() runs at once and returns a cursor; a BigInt
// binding throws; an INTEGER comes back as a number, rounded past 2^53; a BLOB as an ArrayBuffer.
function durableObjectSql(db: DatabaseSync) {
  return {
    exec(sql: string, ...bindings: unknown[]) {
      if (bindings.some((b) => typeof b === "bigint")) throw new TypeError("Cannot convert a BigInt value to a number");
      const st = db.prepare(sql);
      st.setReadBigInts(true);
      const rows = st.all(...(bindings as never[])).map((row) =>
        Object.fromEntries(Object.entries(row).map(([k, v]) => [k, typeof v === "bigint" ? Number(v) : v instanceof Uint8Array ? v.slice().buffer : v])),
      );
      return { toArray: () => rows, columnNames: rows.length ? Object.keys(rows[0]!) : [] };
    },
    get databaseSize() {
      return 4096;
    },
  };
}

test("a Durable Object's ctx.storage.sql: runSync and run take it, its values given their kinds", async () => {
  const db = new DatabaseSync(":memory:");
  db.exec("create table items (id integer primary key, n integer, big bigint, data blob, flag boolean, doc json)");
  const sql = durableObjectSql(db);
  const insert = defineQuery<{ n: bigint; big: bigint; data: Uint8Array; flag: boolean; doc: unknown }, { id: number }, "one", "sqlite">({
    dialect: "sqlite",
    sql: "insert into items (n, big, data, flag, doc) values (:n, :big, :data, :flag, :doc) returning id",
    params: ["n", "big", "data", "flag", "doc"],
    cardinality: "one",
    inputs: { big: { type: { kind: "scalar", name: "int8" } } },
    columns: [["id", "int"]],
  });
  const select = defineQuery<{ id: number }, unknown, "one", "sqlite">({
    dialect: "sqlite",
    sql: "select n, big, data, flag, doc from items where id = :id",
    params: ["id"],
    cardinality: "one",
    columns: [["n", "int"], ["big", "int8"], ["data", "bytes"], ["flag", "bool"], ["doc", "json"]],
  });
  // A BigInt a number holds goes as that number; one past 2^53 to a BIGINT column as its digits.
  const { id } = insert.runSync(sql, { n: 5n, big: 2n ** 53n + 1n, data: new Uint8Array([1, 2]), flag: true, doc: '{"a":1}' });
  assert.equal(id, 1);
  assert.deepEqual({ ...db.prepare("select n, cast(big as text) as big, typeof(big) as t, flag from items").get() }, { n: 5, big: "9007199254740993", t: "integer", flag: 1 });
  // Read back, that BIGINT comes as a rounded number, which Reylo refuses rather than return.
  assert.throws(() => select.runSync(sql, { id }), /Column "big" is a 64-bit integer, and the driver already rounded its value/);
  const second = await insert.run(sql, { n: 6n, big: 7n, data: new Uint8Array([3]), flag: false, doc: "[]" });
  assert.deepEqual(await select.run(sql, second), { n: 6, big: 7n, data: new Uint8Array([3]), flag: false, doc: [] });
  assert.deepEqual(select.runSync(sql, second), { n: 6, big: 7n, data: new Uint8Array([3]), flag: false, doc: [] });
  // A BigInt past 2^53 anywhere else is refused before it reaches exec().
  assert.throws(() => insert.runSync(sql, { n: 2n ** 53n, big: 1n, data: new Uint8Array(), flag: true, doc: "1" }), /^TypeError: A Durable Object's SQL storage binds no BigInt, and 9007199254740992 is past 2\^53 - 1/);
  // A write without returning gives no rows.
  const touch = defineQuery<{}, unknown, "none", "sqlite">({ dialect: "sqlite", sql: "update items set n = n + 1", params: [], cardinality: "none" });
  assert.equal(touch.runSync(sql), undefined);
  assert.equal((db.prepare("select sum(n) as s from items").get() as { s: number }).s, 13);
});

test("a Durable Object's ctx.storage.sql: the catalog facts are read again through exec()", async () => {
  const db = new DatabaseSync(":memory:");
  db.exec("create table users (id integer primary key, email text not null)");
  const sql = durableObjectSql(db);
  const query = "select j.value as fresh_table, p.name as fresh_column, p.type as fresh_type, cast(p.\"notnull\" as text) as fresh_not_null, cast(p.dflt_value is not null or (p.pk > 0 and lower(p.type) = 'integer' and (select count(*) from pragma_table_xinfo(j.value) k where k.pk > 0) = 1) as text) as fresh_default from (select 'users' as value) j, pragma_table_xinfo(j.value) p order by 1, 2";
  const built = (email: string[]) =>
    defineQuery<{}, unknown, "many", "sqlite">({ dialect: "sqlite", sql: "select id, email from users", params: [], fresh: { query, rows: [email, ["users", "id", "INTEGER", "0", "1"]], inserts: [] }, source: "users.sql" });
  assert.deepEqual(built(["users", "email", "TEXT", "1", "0"]).runSync(sql), []);
  assert.throws(() => built(["users", "email", "TEXT", "0", "0"]).runSync(sql), /^Error: The database changed after users\.sql was built: users\.email was/);
  // run takes the same synchronous path: the check and the statement run before it returns.
  const ran: string[] = [];
  const counted = { ...sql, exec: (text: string, ...bindings: unknown[]) => (ran.push(text), sql.exec(text, ...bindings)), databaseSize: 4096 };
  const pending = built(["users", "email", "TEXT", "1", "0"]).run(counted);
  assert.deepEqual(ran, [query, "select id, email from users"]);
  assert.deepEqual(await pending, []);
});
