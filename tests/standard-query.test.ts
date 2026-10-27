import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdtempSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath, pathToFileURL } from "node:url";
import { Session } from "../src/analyze.ts";
import { defineQuery } from "../src/index.ts";
import { sqlModule } from "../src/loader.ts";
import { ParameterError } from "../src/values.ts";

// Every import is a Standard Query: `~standard.query` holds the statement, its database, and the
// two mappings a driver calls, `encode` (the input to the values it binds, each in the database's
// raw form) and `decode` (the rows it read raw to the output), with `.run()`'s input and output.

const PG = process.env.REYLO_TEST_PG;
const DRIVERS = fileURLToPath(new URL("../lab/drivers/node_modules", import.meta.url));
const INDEX = pathToFileURL(fileURLToPath(new URL("../src/index.ts", import.meta.url))).href;
const pgLive = !PG ? "REYLO_TEST_PG names no server" : !existsSync(join(DRIVERS, "pg")) ? "lab/drivers is not installed" : false;

const users = defineQuery<{ id: number; tags: number[]; doc: unknown; bytes: Uint8Array; flag: boolean }, { id: number; at: string; big: bigint }, "many", "postgres">({
  dialect: "postgres",
  sql: "select id, at, big from users where id = :id and tags @> :tags and doc @> :doc and bytes = :bytes and flag = :flag",
  params: ["id", "tags", "doc", "bytes", "flag"],
  text: "select id, at, big from users where id = $1 and tags @> $2 and doc @> $3 and bytes = $4 and flag = $5",
  order: ["id", "tags", "doc", "bytes", "flag"],
  inputs: {
    id: { type: { kind: "scalar", name: "int4" }, nullable: false },
    tags: { type: { kind: "array", of: { kind: "scalar", name: "int4" } } },
    doc: { type: { kind: "json" } },
    bytes: { type: { kind: "scalar", name: "bytea" } },
    flag: { type: { kind: "scalar", name: "bool" } },
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
    { id: 2, at: "2026-10-22T00:00:00Z", big: null },
  ]);
  assert.throws(() => q.decode([["1", "x"]]), /selects 3 columns, and a row has 2/);
  const one = defineQuery<{}, { n: bigint }, "maybeOne", "postgres">({ dialect: "postgres", sql: "select count(*) as n from users", params: [], columns: [["n", "int8"]], cardinality: "maybeOne" });
  assert.deepEqual(one["~standard"].query.decode([["3"]]), { n: 3n });
  assert.equal(one["~standard"].query.decode([]), null);
  assert.throws(() => one["~standard"].query.decode([["1"], ["2"]]), /Expected at most one row, got 2/);
  const none = defineQuery<{}, never, "none", "postgres">({ dialect: "postgres", sql: "delete from users", params: [], columns: [], cardinality: "none" });
  assert.equal(none["~standard"].query.decode([]), undefined);
});

test("mysql and sqlite: each value as a driver binds it, and decode takes every form a driver reads", () => {
  const my = defineQuery<{ id: bigint; doc: unknown }, { id: bigint; at: string; amount: string; ok: boolean }, "maybeOne", "mysql">({
    dialect: "mysql",
    sql: "select id, at, amount, ok from t where id = :id and json_contains(doc, :doc)",
    params: ["id", "doc"],
    text: "select id, at, amount, ok from t where id = ? and json_contains(doc, ?)",
    order: ["id", "doc"],
    inputs: { id: { type: { kind: "scalar", name: "int8" } }, doc: { type: { kind: "json" } } },
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
  const lite = defineQuery<{ ok: boolean }, { id: bigint; ok: boolean }, "many", "sqlite">({
    dialect: "sqlite",
    sql: "select id, ok from t where ok = :ok",
    params: ["ok"],
    text: "select id, ok from t where ok = ?",
    order: ["ok"],
    inputs: { ok: { type: { kind: "scalar", name: "bool" } } },
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
  } finally {
    pg.types.setTypeParser(20, (x: string) => x);
    pg.types.setTypeParser(1184, pg.types.getTypeParser(1114));
    for (const session of Session.opened()) await session.close();
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
