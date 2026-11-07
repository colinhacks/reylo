import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { pathToFileURL } from "node:url";
import { Session } from "../src/analyze.ts";
import { defineQuery, type Query } from "../src/index.ts";
import { sqlModule } from "../src/loader.ts";
import { rowOf, rowSource, type RowBuilder } from "../src/row.ts";
import { ParameterError } from "../src/values.ts";
import type { StandardQueryV1 } from "../src/standard-query.ts";
import type { Dialect } from "../src/execute.ts";
import { closeSessions, declarations, DRIVERS, PG, pgLive, INDEX, transform } from "./helpers.ts";

// Every import is a Standard Query: `~standard.query` holds the statement, its database, and the
// two mappings a driver calls, `encode` (the input to the values it binds, each in the database's
// raw form) and `decode` (the rows it read raw to the output), with `.run()`'s input and output.


const users = defineQuery<{ id: number; tags: number[]; doc: unknown; bytes: Uint8Array; flag: boolean }, { id: number; at: string; big: bigint }, "many", "postgres">({
  dialect: "postgres",
  sql: "select id, at, big from users where id = :id and tags @> :tags and doc @> :doc and bytes = :bytes and flag = :flag",
  params: ["id", "tags", "doc", "bytes", "flag"],
  text: "select id, at, big from users where id = $1 and tags @> $2 and doc @> $3 and bytes = $4 and flag = $5",
  order: ["id", "tags", "doc", "bytes", "flag"],
  inputs: {
    id: { type: { kind: "integer", bits: 32, label: "int4" }, nullable: false },
    tags: { type: { kind: "array", element: { type: { kind: "integer", bits: 32, label: "int4" } }, label: "_int4" } },
    doc: { type: { kind: "json", label: "jsonb" } },
    bytes: { type: { kind: "bytes", label: "bytea" } },
    flag: { type: { kind: "boolean", label: "bool" } },
  },
  columns: [
    ["id", "int"],
    ["at", "timestamptz"],
    ["big", "int8"],
  ],
  cardinality: "many",
});

test("postgres: the statement, its database, and each value as the text the server reads", () => {
  const q = users["~standard"].query;
  assert.equal(q.dialect, "postgres");
  assert.equal(q.text, users.sql);
  assert.deepEqual(q.encode({ id: 7, tags: [1, 2], doc: { a: [1] }, bytes: new Uint8Array([0, 255]), flag: true }), ["7", '{"1","2"}', '{"a":[1]}', "\\x00ff", "t"]);
  // A key left out binds NULL, as `.run()` binds it; one the statement takes no null for is refused.
  assert.deepEqual(users["~standard"].query.encode({ id: 7 }), ["7", null, null, null, null]);
  assert.throws(() => q.encode({ tags: [1] }), (e) => e instanceof ParameterError && /"id" is missing/.test(e.message));
  // The input is an object, as `.run()` takes it.
  assert.throws(() => q.encode([7]), ParameterError);
  // A driver may take `encode` off the object.
  const { encode } = q;
  assert.deepEqual(encode({ id: 7 }), ["7", null, null, null, null]);
});

test("postgres: decode gives each value its kind, and the output its cardinality, as .run() does", () => {
  const q = users["~standard"].query;
  assert.deepEqual(q.decode([["1", "2026-10-22 14:46:00.123456+02", "9007199254740993"], ["2", "2026-10-22 00:00:00+00", null]]), [
    { id: 1, at: "2026-10-22T12:46:00.123456Z", big: 9007199254740993n },
    { id: 2, at: "2026-10-22T00:00:00.000000Z", big: null },
  ]);
  assert.throws(() => q.decode([["1", "x"]]), /selects 3 columns, and a row has 2/);
  const one = defineQuery<{}, { n: bigint }, "maybeOne", "postgres">({ dialect: "postgres", sql: "select count(*) as n from users", params: [], columns: [["n", "int8"]], cardinality: "maybeOne" });
  assert.deepEqual(one["~standard"].query.decode([["3"]]), { n: 3n });
  assert.equal(one["~standard"].query.decode([]), null);
  assert.throws(() => one["~standard"].query.decode([["1"], ["2"]]), /Expected at most one row, got 2/);
  const none = defineQuery<{}, never, "none", "postgres">({ dialect: "postgres", sql: "delete from users", params: [], columns: [], cardinality: "none" });
  assert.equal(none["~standard"].query.decode([]), undefined);
});

test("a row has the columns' names: one named __proto__ is an own property, and of two with one name the last value wins at the first's place", () => {
  const names = ["id", "__proto__", "id", "x y", "1"];
  // The builder a module carries, as its source reads, and the loop a query defined by hand builds rows with.
  const generated = new Function(`return ${rowSource(names)};`)() as RowBuilder;
  for (const build of [generated, rowOf(names)]) {
    const row = build([1, { leaked: true }, 2, "s", "n"]);
    assert.deepEqual(Object.keys(row), ["1", "id", "__proto__", "x y"]);
    assert.equal(Object.getPrototypeOf(row), Object.prototype);
    assert.deepEqual(Object.getOwnPropertyDescriptor(row, "__proto__"), { value: { leaked: true }, writable: true, enumerable: true, configurable: true });
    assert.equal((row as { leaked?: boolean }).leaked, undefined);
    assert.equal(row.id, 2);
    // A new object every row, which keeps nothing of the values array it copies.
    assert.notEqual(build([1, null, 2, "s", "n"]), row);
  }
  // decode builds the same rows with the module's builder and without it.
  const columns = [["id", "int"], ["__proto__", "json"], ["at", "timestamptz"]] as const;
  const def = { dialect: "postgres", sql: "select 1", params: [], columns } as const;
  const rows = [["1", '{"leaked":true}', "2026-10-22 14:46:00.5+02"], ["2", null, null]];
  const expected = [Object.defineProperty({ id: 1, at: "2026-10-22T12:46:00.500000Z" }, "__proto__", { value: { leaked: true }, writable: true, enumerable: true, configurable: true }), Object.defineProperty({ id: 2, at: null }, "__proto__", { value: null, writable: true, enumerable: true, configurable: true })];
  for (const q of [defineQuery(def), defineQuery({ ...def, row: new Function(`return ${rowSource(columns.map(([n]) => n))};`)() as RowBuilder })]) {
    const out = q["~standard"].query.decode(rows) as Record<string, unknown>[];
    assert.deepStrictEqual(out, expected);
    assert.deepEqual(out.map((r) => Object.keys(r)), [["id", "__proto__", "at"], ["id", "__proto__", "at"]]);
  }
  // A driver that hands a value in another form than text is refused, naming the column.
  assert.throws(() => defineQuery(def)["~standard"].query.decode([[1 as unknown as string, null, null]]), /Column "id" is not in Postgres's text form/);
});

/** Each raw row through the query's `columns`, as a driver that decodes each value once as it reads it builds it: NULL as null, never decoded. */
function throughColumns(sq: { columns?: readonly { name: string; decode(v: never): unknown }[] }, raw: readonly (readonly unknown[])[]): Record<string, unknown>[] {
  return raw.map((r) => {
    const row: Record<string, unknown> = {};
    sq.columns!.forEach((c, i) => {
      const v = r[i];
      Object.defineProperty(row, c.name, { value: v === null || v === undefined ? null : c.decode(v as never), writable: true, enumerable: true, configurable: true });
    });
    return row;
  });
}
/** What a call gives: its value, or its error's message. */
function outcome(f: () => unknown): { value?: unknown; error?: string } {
  try {
    return { value: f() };
  } catch (e) {
    return { error: `${(e as Error).constructor.name}: ${(e as Error).message}` };
  }
}

// Every kind and its array, each with a raw value of its database's form, and a row of NULLs.
const KINDS = {
  postgres: [
    ["bool", "t"], ["int", "42"], ["float", "-1.5e-7"], ["int8", "9007199254740993"], ["decimal", "12345678901234567890.50"], ["text", "héllo"], ["date", "0002-06-01 BC"],
    ["timestamp", "12345-01-01 00:00:00.5"], ["timestamptz", "2026-10-22 14:46:00.123456-07:52:58"], ["interval", "-1 years -2 mons +3 days 04:05:06.5"], ["json", '{"a":[1,null,"x"]}'],
    ["bytes", "\\x00ff"], ["vector", "[1,2.5,-3,1e-07]"], ["hstore", String.raw`"a"=>"1", "b c"=>NULL, "q\""=>"\\"`], ["unknown", "(1,x)"],
    ["bool[]", "{t,f,NULL}"], ["int[]", "{1,NULL,-3}"], ["float[]", "{1.5,NaN,Infinity}"], ["int8[]", "{9007199254740993,NULL}"], ["decimal[]", "{1.50,NaN}"], ["text[]", '{a,"b,c",NULL,"NULL",""}'],
    ["date[]", '{2026-10-22,"0002-06-01 BC"}'], ["timestamp[]", '{"2026-10-22 14:46:00",infinity}'], ["timestamptz[]", '{"2026-10-22 14:46:00+00","1883-11-18 12:00:00-07:52:58"}'],
    ["interval[]", '{"1 day","-02:00:00"}'], ["json[]", String.raw`{"{\"a\":1}",NULL}`], ["bytes[]", String.raw`{"\\x00ff",NULL}`], ["vector[]", '{"[1,2]",NULL,"[3]"}'],
    ["hstore[]", String.raw`{"\"a\"=>\"1\"",NULL,""}`], ["text[]:", "{0101:NULL:0102}"], ["text[];", "{(1,1),(0,0);(2,2),(1,1)}"],
  ],
  mysql: [
    ["int8", "9007199254740993"], ["int8", 9007199254740993n], ["timestamp", "2026-10-22 14:46:00.000000"], ["date", "2026-10-22"], ["decimal", "1.50"], ["bool", 1], ["bool", "0"], ["json", '{"a":1}'],
    ["json", { a: 1 }], ["timestamp|date", "2026-10-22 14:46:00"], ["bytes", new Uint8Array([0, 255])], ["int", 7], ["float", 1.5], ["text", "x"], ["int8|int", "5"],
  ],
  sqlite: [
    ["int", 7], ["int", 7n], ["int8", 9007199254740993n], ["int8", "9007199254740993"], ["float", 1.5], ["text", "x"], ["text", 5n], ["bytes", new Uint8Array([1])], ["bool", 1n], ["bool", 0], ["json", '{"a":[1]}'],
    ["int|float|text", 2.5], ["int|float|text", "abc"], ["unknown", 3n], ["unknown", "x"],
  ],
  duckdb: [["int8", 9007199254740993n], ["int", 7], ["decimal", "1.50"], ["bool", true], ["text", "x"], ["timestamptz", "2026-10-22T14:46:00Z"]],
} as const;

test("columns and output: decode(raw) is output(raw through columns), for every kind and array, every cardinality and every refusal", () => {
  for (const [dialect, kinds] of Object.entries(KINDS) as [Dialect, readonly (readonly [string, unknown])[]][]) {
    // A delimiter after the brackets (`text[]:`) is the array's own; each column a name of its own.
    const columns = kinds.map(([kind], i) => {
      const delim = /\[\]([:;])$/.exec(kind)?.[1];
      return delim ? ([`c${i}`, kind.slice(0, -1), delim] as const) : ([`c${i}`, kind] as const);
    });
    const full = kinds.map(([, v]) => v);
    const nulls = kinds.map(() => null);
    // Two rows of every value and one of NULLs, then each cardinality's edges: no row, one, two.
    for (const cardinality of ["many", "one", "maybeOne", "none"] as const) {
      const q = defineQuery({ dialect, sql: "select 1", params: [], columns, cardinality } as never) as { "~standard": { query: StandardQueryV1.Query<unknown, Dialect> & { output(rows: readonly Record<string, unknown>[]): unknown } } };
      const sq = q["~standard"].query;
      assert.deepEqual(sq.columns!.map((c) => c.name), columns.map(([name]) => name));
      for (const raw of [[], [full], [nulls], [full, nulls], [full, full, nulls]]) {
        const decoded = outcome(() => sq.decode(raw as never));
        assert.deepStrictEqual(outcome(() => sq.output(throughColumns(sq, raw))), decoded, `${dialect} ${cardinality} ${raw.length} rows`);
      }
      // Each value alone: the column's decoder gives what decode puts in the row, a refusal included.
      if (cardinality === "many")
        kinds.forEach(([kind, v], i) => {
          const row = outcome(() => (sq.decode([kinds.map((_, j) => (j === i ? v : null))] as never) as Record<string, unknown>[])[0]![`c${i}`]);
          assert.deepStrictEqual(outcome(() => sq.columns![i]!.decode(v as never)), row, `${dialect} ${kind}`);
        });
    }
  }
  // A value its column's type does not allow is refused alike both ways: SQLite's text in an INTEGER column.
  const loose = defineQuery({ dialect: "sqlite", sql: "select n from t", params: [], columns: [["n", "int"]] })["~standard"].query;
  const refused = outcome(() => loose.decode([["abc"]]));
  assert.match(refused.error!, /DecodeError: .*"n"/);
  assert.deepStrictEqual(outcome(() => loose.output(throughColumns(loose, [["abc"]]))), refused);
  // A query defined by hand without its columns has no `columns`; its `output` applies the cardinality alone.
  const bare = defineQuery({ dialect: "postgres", sql: "select 1", params: [], cardinality: "maybeOne" })["~standard"].query;
  assert.equal("columns" in bare, false);
  assert.deepEqual(bare.output([{ a: 1 }]), { a: 1 });
  assert.throws(() => bare.output([{ a: 1 }, { a: 2 }]), /Expected at most one row, got 2/);
});

test("postgres: a float or a decimal inside JSON that Postgres writes as text is the number it is, through decode and through its column's decoder, as .run() gives it", () => {
  const q = defineQuery({ dialect: "postgres", sql: "select doc, raw from t", params: [], columns: [["doc", "json"], ["raw", "json"]], nonFinite: [{ keys: [["n", true]] }, null] })["~standard"].query;
  const text = '{"n": "NaN", "s": "Infinity"}';
  assert.deepStrictEqual(q.decode([[text, text]]), [{ doc: { n: NaN, s: "Infinity" }, raw: { n: "NaN", s: "Infinity" } }]);
  assert.deepStrictEqual(q.columns![0]!.decode(text as never), { n: NaN, s: "Infinity" });
  assert.equal(q.columns![0]!.decode(null as never), null);
  // Each column converts by its own position: of two columns of one name, the row keeps the last,
  // and a name `Object.prototype` holds is no mask.
  const twice = defineQuery({ dialect: "postgres", sql: "select", params: [], columns: [["x", "json"], ["x", "json"], ["constructor", "json"], ["__proto__", "json"]], nonFinite: [{ keys: [["n", true]] }, null, null, { keys: [["n", true]] }] })["~standard"].query;
  const doc = '{"n": "NaN"}';
  const [row] = twice.decode([[doc, doc, doc, doc]]) as Record<string, unknown>[];
  assert.deepStrictEqual({ ...row }, { x: { n: "NaN" }, constructor: { n: "NaN" }, ["__proto__"]: { n: NaN } });
  assert.deepStrictEqual(twice.columns!.map((c) => c.decode(doc as never)), [{ n: NaN }, { n: "NaN" }, { n: "NaN" }, { n: NaN }]);
});

test("mysql and sqlite: each value as a driver binds it, and decode takes every form a driver reads", () => {
  const my = defineQuery<{ id: bigint; doc: unknown }, { id: bigint; at: string; amount: string; ok: boolean }, "maybeOne", "mysql">({
    dialect: "mysql",
    sql: "select id, at, amount, ok from t where id = :id and json_contains(doc, :doc)",
    params: ["id", "doc"],
    text: "select id, at, amount, ok from t where id = ? and json_contains(doc, ?)",
    order: ["id", "doc"],
    inputs: { id: { type: { kind: "integer", bits: 64, label: "bigint" } }, doc: { type: { kind: "json", label: "json" } } },
    columns: [
      ["id", "int8"],
      ["at", "timestamp"],
      ["amount", "decimal"],
      ["ok", "bool"],
    ],
    cardinality: "maybeOne",
  });
  assert.deepEqual(my["~standard"].query.encode({ id: 5n, doc: { a: 1 } }), [5n, '{"a":1}']);
  // mysql2's binary protocol gives a BIGINT as its digits, mariadb's text protocol as a BigInt.
  assert.deepEqual(my["~standard"].query.decode([["9007199254740993", "2026-10-22 14:46:00.000000", "1.50", 1]]), { id: 9007199254740993n, at: "2026-10-22T14:46:00.000000", amount: "1.50", ok: true });
  assert.deepEqual(my["~standard"].query.decode([[9007199254740993n, "2026-10-22 14:46:00", "1.50", "1"]]), { id: 9007199254740993n, at: "2026-10-22T14:46:00", amount: "1.50", ok: true });
  // A column that may be a DATETIME takes the T too, as `.run()` gives it.
  const either = defineQuery<{}, { at: string }, "many", "mysql">({ dialect: "mysql", sql: "select coalesce(at, d) as at from t", params: [], columns: [["at", "timestamp|date"]] });
  assert.deepEqual(either["~standard"].query.decode([["2026-10-22 14:46:00"]]), [{ at: "2026-10-22T14:46:00" }]);

  const lite = defineQuery<{ ok: boolean }, { id: bigint; ok: boolean }, "many", "sqlite">({
    dialect: "sqlite",
    sql: "select id, ok from t where ok = :ok",
    params: ["ok"],
    text: "select id, ok from t where ok = ?",
    order: ["ok"],
    inputs: { ok: { type: { kind: "integer", bits: 8, label: "tinyint(1)" } } },
    columns: [
      ["id", "int8"],
      ["ok", "bool"],
    ],
  });
  assert.deepEqual(lite["~standard"].query.encode({ ok: true }), [1]);
  assert.deepEqual(lite["~standard"].query.decode([[1n, 0n]]), [{ id: 1n, ok: false }]);

  // SQLite decodes through `.run()`'s own normalizing: a value of a storage class the column's type
  // does not allow, which a table that is not STRICT stores, is refused, and an integer in a column
  // with no type is a number however the driver read it.
  const loose = defineQuery<{}, { n: number; s: string; u: unknown }, "many", "sqlite">({
    dialect: "sqlite",
    sql: "select n, s, u from loose",
    params: [],
    columns: [
      ["n", "int"],
      ["s", "text"],
      ["u", "unknown"],
    ],
  });
  const lq = loose["~standard"].query;
  assert.deepEqual(lq.decode([[1n, "a", 2n]]), [{ n: 1, s: "a", u: 2 }]);
  assert.throws(() => lq.decode([["abc", "a", null]]), /Column "n" is typed number and holds the text "abc"/);
  assert.throws(() => lq.decode([[1, new Uint8Array([1]), null]]), /Column "s" is typed string and holds a BLOB of 1 byte/);
  assert.throws(() => lq.decode([[2n ** 60n, "a", null]]), /"n"/);
});

test("the result key a build step writes is the Standard Query's key", () => {
  const keyed = defineQuery({ dialect: "postgres", sql: "select id, name from users", params: [], columns: [["id", "int"], ["name", "text"]], key: ["id"] });
  assert.deepEqual(keyed["~standard"].query.key, ["id"]);
  assert.equal("key" in users["~standard"].query, false);
});

test("a total order a build step writes is the Standard Query's ordered, which a driver's signature can require", () => {
  const listed = defineQuery({ dialect: "postgres", sql: "select id from users order by id", params: [], columns: [["id", "int"]], key: ["id"], ordered: true });
  assert.equal(listed["~standard"].query.ordered, true);
  assert.equal("ordered" in users["~standard"].query, false);
});

/** What a driver that compares results by position asks of a query, and what a module of a total order declares. */
type Positional = { readonly "~standard": { readonly query: { readonly ordered: true } } };
type DeclaredOrdered = Query<{}, { id: number }, "many", "postgres"> & Positional;
// Checked by the type check, never run: a query declared ordered passes, and one declared without it does not.
export function positionalSignature(ordered: DeclaredOrdered, plain: Query<{}, { id: number }, "many", "postgres">) {
  const takes = (q: Positional) => q;
  takes(ordered);
  // @ts-expect-error A query whose order is not declared total.
  takes(plain);
}

test("a query defined by hand without its columns cannot decode, and says so", () => {
  const bare = defineQuery<{ id: number }, { id: number }, "many", "postgres">({ dialect: "postgres", sql: "select :id::int as id", params: ["id"] });
  assert.equal(bare["~standard"].query.text, "select $1::int as id");
  assert.deepEqual(bare["~standard"].query.encode({ id: 1 }), ["1"]);
  assert.throws(() => bare["~standard"].query.decode([["1"]]), /without its columns/);
});

test("sqlite: node:sqlite runs a built import as a Standard Query, with its own conversions off, to .run()'s output", async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-standard-"));
  const db = join(root, "app.db");
  const setup = new DatabaseSync(db);
  setup.exec("create table t (id integer primary key, name text not null, ok boolean not null, big bigint, data blob)");
  setup.exec("insert into t values (1, 'ada', 1, 9007199254740993, x'00ff'), (2, 'bob', 0, null, null)");
  setup.close();
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=file:${db}\n`);
  try {
    const query = await built(root, "list", "select id, name, ok, big, data from t where ok = :ok or id > :min order by id");
    const conn = new DatabaseSync(db);
    const sq = query["~standard"].query;
    const statement = conn.prepare(sq.text);
    statement.setReturnArrays(true);
    statement.setReadBigInts(true);
    const input = { ok: true, min: 1 };
    const output = sq.decode(statement.all(...sq.encode(input)) as unknown as unknown[][]);
    assert.deepEqual(output, await query.run(conn, input));
    assert.deepEqual(output, [
      { id: 1, name: "ada", ok: true, big: 9007199254740993n, data: new Uint8Array([0, 255]) },
      { id: 2, name: "bob", ok: false, big: null, data: null },
    ]);
    // SQLite's analysis proves no result key yet.
    assert.equal("key" in sq, false);
    conn.close();
  } finally {
    for (const session of Session.opened()) await session.close();
  }
});

test("postgres: pg runs a built import as a Standard Query, with every parser off, to .run()'s output", { skip: pgLive }, async () => {
  const require = createRequire(join(DRIVERS, "pg", "package.json"));
  const pg = require("pg");
  const admin = new pg.Client({ connectionString: PG });
  await admin.connect();
  const name = `reylo_standard_${process.pid}`;
  await admin.query(`drop database if exists ${name}`);
  await admin.query(`create database ${name}`);
  const url = new URL(PG!);
  url.pathname = `/${name}`;
  const pool = new pg.Pool({ connectionString: url.href });
  // The app's own parsers, which a Standard Query's rows must never see.
  pg.types.setTypeParser(20, (x: string) => `app:${x}`);
  pg.types.setTypeParser(1184, (x: string) => `app:${x}`);
  const root = mkdtempSync(join(tmpdir(), "reylo-standard-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url.href}\n`);
  try {
    await pool.query("create table events (id int8 primary key, at timestamptz not null, tags int4[] not null, doc jsonb, amount numeric)");
    await pool.query("insert into events values (9007199254740993, '2026-10-22 14:46:00.123456+00', '{1,2}', '{\"a\":1}', 12345678901234567890.5)");
    const query = await built(root, "events", "select id, at, tags, doc, amount from events where tags @> :tags and at >= :since");
    const input = { tags: [1], since: "2026-01-01T00:00:00Z" };
    const sq = query["~standard"].query;
    const raw = await pool.query({ text: sq.text, values: sq.encode(input), rowMode: "array", types: { getTypeParser: () => (x: string) => x } });
    const output = sq.decode(raw.rows);
    assert.deepEqual(output, await query.run(pool, input));
    assert.deepEqual(output, [{ id: 9007199254740993n, at: "2026-10-22T14:46:00.123456Z", tags: [1, 2], doc: { a: 1 }, amount: "12345678901234567890.5" }]);
    // The result key: the primary key identifies each row; a statement of one row at most, which its
    // cardinality says, and one whose rows nothing identifies have none.
    assert.deepEqual(sq.key, ["id"]);
    assert.equal("key" in (await built(root, "count", "select count(*) as n from events"))["~standard"].query, false);
    assert.equal("key" in (await built(root, "amounts", "select amount from events"))["~standard"].query, false);
  } finally {
    pg.types.setTypeParser(20, (x: string) => x);
    pg.types.setTypeParser(1184, pg.types.getTypeParser(1114));
    for (const session of Session.opened()) await session.close();
    await pool.end();
    await admin.query(`drop database if exists ${name}`);
    await admin.end();
  }
});

test("postgres: a built import's columns decode the types the database created as decode does, and its declaration types each column", { skip: pgLive }, async () => {
  const require = createRequire(join(DRIVERS, "pg", "package.json"));
  const pg = require("pg");
  const admin = new pg.Client({ connectionString: PG });
  await admin.connect();
  const name = `reylo_columns_${process.pid}`;
  await admin.query(`drop database if exists ${name}`);
  await admin.query(`create database ${name}`);
  const url = new URL(PG!);
  url.pathname = `/${name}`;
  const pool = new pg.Pool({ connectionString: url.href });
  const root = mkdtempSync(join(tmpdir(), "reylo-columns-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url.href}\n`);
  try {
    await pool.query("create extension hstore");
    await pool.query("create type mood as enum ('happy', 'sad')");
    await pool.query("create table notes (id int4 primary key, mood mood not null, moods mood[], prefs hstore, history hstore[], at timestamptz not null, big int8)");
    await pool.query(String.raw`insert into notes values (1, 'happy', '{happy,NULL,sad}', '"a"=>"1", "b c"=>NULL', array['"x"=>"y"'::hstore, NULL], '2026-10-22 14:46:00.5+02', 9007199254740993), (2, 'sad', NULL, NULL, NULL, '1883-11-18 12:00:00-07:52:58', NULL)`);
    const sql = "select id, mood, moods, prefs, history, at, big from notes order by id";
    const query = await built(root, "notes", sql);
    const sq = query["~standard"].query;
    assert.deepEqual(sq.columns.map((c: { name: string }) => c.name), ["id", "mood", "moods", "prefs", "history", "at", "big"]);
    // The server's own text, every parser off: decode, and the same rows through each column's decoder.
    const raw = (await pool.query({ text: sq.text, values: sq.encode({}), rowMode: "array", types: { getTypeParser: () => (x: string) => x } })).rows;
    const decoded = sq.decode(raw);
    assert.deepStrictEqual(sq.output(throughColumns(sq, raw)), decoded);
    assert.deepEqual(decoded, await query.run(pool));
    assert.deepEqual(decoded, [
      { id: 1, mood: "happy", moods: ["happy", null, "sad"], prefs: { a: "1", "b c": null }, history: [{ x: "y" }, null], at: "2026-10-22T12:46:00.500000Z", big: 9007199254740993n },
      { id: 2, mood: "sad", moods: null, prefs: null, history: null, at: "1883-11-18T19:52:58.000000Z", big: null },
    ]);
    // The declaration types each column's decoder by the column's type, without null, and the
    // statement's total order (`order by id`) as the literal `true`.
    await closeSessions();
    const declared = (await declarations(root, { "notes.sql": sql }))["notes.sql"]!;
    const line = declared.split("\n").find((l) => l.includes(`readonly "~standard"`));
    assert.equal(
      line,
      `  readonly "~standard": { readonly query: { readonly columns: readonly [{ readonly name: "id"; decode(value: string): number }, { readonly name: "mood"; decode(value: string): "happy" | "sad" }, { readonly name: "moods"; decode(value: string): ("happy" | "sad" | null)[] }, { readonly name: "prefs"; decode(value: string): Record<string, string | null> }, { readonly name: "history"; decode(value: string): (Record<string, string | null> | null)[] }, { readonly name: "at"; decode(value: string): string }, { readonly name: "big"; decode(value: string): bigint }]; readonly ordered: true } };`,
    );
    // Of two columns with one name the row keeps the last, while `columns` lists both: a JSON column
    // only `columns` names still has its type imported.
    // The mapper warns of the repeated name (14), and still declares the module.
    const { text: twice, diagnostics } = (await transform(root, { "twice.sql": "select '{}'::jsonb as x, 1 as x" }))["twice.sql"]!;
    assert.deepEqual(diagnostics.map((d: { code: number }) => d.code), [14]);
    assert.ok(twice.startsWith(`import type { Json, Query } from "reylo";`), twice);
    assert.ok(twice.includes(`readonly columns: readonly [{ readonly name: "x"; decode(value: string): Json }, { readonly name: "x"; decode(value: string): number }]`), twice);
  } finally {
    await closeSessions();
    await pool.end();
    await admin.query(`drop database if exists ${name}`);
    await admin.end();
  }
});

/** A module built for `sql` in `root` and imported. */
async function built(root: string, name: string, sql: string) {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, sql);
  const { code } = await sqlModule(file, sql);
  const out = join(root, `${name}.mjs`);
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  return (await import(`${pathToFileURL(out).href}?${Math.random()}`)).default;
}

test("a Standard Query holds a Postgres array column to one dimension, as .run() does", () => {
  const q = defineQuery<{}, { tags: (number | null)[] | null }, "many", "postgres">({ dialect: "postgres", sql: "select tags from posts", params: [], columns: [["tags", "int[]"]], cardinality: "many" })["~standard"].query;
  assert.deepEqual(q.decode([["{1,NULL}"], [null]]), [{ tags: [1, null] }, { tags: null }]);
  assert.throws(() => q.decode([["{{1,2},{3,4}}"]]), /^TypeError: Column "tags" holds an array of 2 dimensions, and its type has one:/);
  assert.throws(() => q.columns![0]!.decode("{{1}}"), /Column "tags" holds an array of 2 dimensions/);
  const nesting = defineQuery<{}, { tags: (number | null)[] | null }, "many", "postgres">({ dialect: "postgres", sql: "select tags from posts", params: [], columns: [["tags", "int[]"]], cardinality: "many", nestedArrays: true })["~standard"].query;
  assert.deepEqual(nesting.decode([["{{1,2},{3,4}}"]]), [{ tags: [[1, 2], [3, 4]] }]);
});
