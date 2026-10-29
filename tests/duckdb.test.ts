import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { Session, type Outcome } from "../src/analyze.ts";
import { sqlModule } from "../src/loader.ts";
import { defineQuery } from "../src/index.ts";
import { duckdbInterval } from "../src/decode-duckdb.ts";
import { dialectOf, duckdbPath } from "../src/project.ts";

// DuckDB runs inside the app, so a project's statements are typed on its own @duckdb/node-api,
// the labs' (lab/drivers), linked into each project as an install would put it.
const DRIVERS = fileURLToPath(new URL("../lab/drivers/node_modules", import.meta.url));
const skip = existsSync(join(DRIVERS, "@duckdb", "node-api")) ? false : "lab/drivers is not installed (no @duckdb/node-api)";
const INDEX = pathToFileURL(fileURLToPath(new URL("../src/index.ts", import.meta.url))).href;
const MAPPER = fileURLToPath(new URL("../src/mapper.ts", import.meta.url));

type Api = {
  DuckDBInstance: { create(path: string): Promise<{ connect(): Promise<Connection>; closeSync(): void }> };
};
type Connection = { run(sql: string): Promise<unknown>; runAndReadAll(sql: string, values?: unknown[]): Promise<unknown>; prepare(sql: string): Promise<unknown>; closeSync(): void };
const api = (): Api => createRequire(join(DRIVERS, "package.json"))("@duckdb/node-api") as Api;

const SCHEMA = `create type mood as enum ('happy', 'sad');
create table teams (id integer primary key, name varchar not null);
create table logs (id integer primary key, msg varchar not null, level varchar, tags varchar[], meta struct(host varchar, pid integer), m mood, team_id integer, seen timestamptz not null default now());
comment on column logs.msg is 'What happened';`;

/** A project whose database is a DuckDB file holding `schema`, with @duckdb/node-api installed unless `driver` is false. */
async function project(schema = SCHEMA, driver = true): Promise<string> {
  const root = mkdtempSync(join(tmpdir(), "reylo-duckdb-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), "DATABASE_URL=duckdb:./app.duckdb\n");
  if (driver) {
    mkdirSync(join(root, "node_modules", "@duckdb"), { recursive: true });
    symlinkSync(join(DRIVERS, "@duckdb", "node-api"), join(root, "node_modules", "@duckdb", "node-api"));
  }
  const db = await api().DuckDBInstance.create(join(root, "app.duckdb"));
  const c = await db.connect();
  await c.run(schema);
  c.closeSync();
  db.closeSync();
  return root;
}

async function analyze(root: string, sql: string): Promise<Extract<Outcome, { ok: true }>["analysis"]> {
  const session = await Session.open(root, { DATABASE_URL: "duckdb:./app.duckdb" });
  try {
    const outcome = await session.analyze(sql, { store: false });
    assert.ok(outcome.ok, !outcome.ok ? outcome.message : "");
    return outcome.analysis;
  } finally {
    await session.close();
  }
}

/** Each column as `name: TYPE`, with `?` where it may be NULL. */
const columns = (a: { columns: { name: string; type_name: string; nullable: boolean }[] }) => a.columns.map((c) => `${c.name}: ${c.type_name}${c.nullable ? "?" : ""}`);

test("DuckDB: each column is typed by the prepared statement, and may be NULL unless the engine shows it is not", { skip }, async () => {
  const root = await project();
  const one = await analyze(root, "select id, msg, level, tags, meta, m from logs where id = :id");
  assert.deepEqual(columns(one), ["id: INTEGER", "msg: VARCHAR", "level: VARCHAR?", "tags: VARCHAR[]?", 'meta: STRUCT("host" VARCHAR, "pid" INTEGER)?', "m: ENUM('happy', 'sad')?"]);
  assert.equal(one.cardinality, "maybeOne");
  assert.equal(one.kind, "select");
  assert.equal(one.columns[1]!.comment, "What happened");
  assert.deepEqual(one.columns[4]!.type, { kind: "struct", fields: [{ name: "host", type: { kind: "scalar", name: "varchar" }, nullable: true }, { name: "pid", type: { kind: "scalar", name: "integer" }, nullable: true }] });
  assert.deepEqual(one.columns[5]!.type, { kind: "enum", name: "ENUM('happy', 'sad')", labels: ["happy", "sad"] });
  // An outer join pads its other side with NULL, unless a filter shows the row was matched.
  assert.deepEqual(columns(await analyze(root, "select l.id, t.name from logs l left join teams t on t.id = l.team_id")), ["id: INTEGER", "name: VARCHAR?"]);
  assert.deepEqual(columns(await analyze(root, "select l.id, t.name from logs l left join teams t on t.id = l.team_id where t.id is not null")), ["id: INTEGER", "name: VARCHAR"]);
  assert.deepEqual(columns(await analyze(root, "select l.id, t.name from teams t right join logs l on t.id = l.team_id")), ["id: INTEGER", "name: VARCHAR?"]);
  // A count is never NULL; another aggregate is, over no rows, but not over a group of them.
  const totals = await analyze(root, "select count(*) as n, max(id) as top from logs");
  assert.deepEqual([columns(totals), totals.cardinality], [["n: BIGINT", "top: INTEGER?"], "one"]);
  const grouped = await analyze(root, "select level, count(*) as n, max(id) as top from logs group by level");
  assert.deepEqual([columns(grouped), grouped.cardinality], [["level: VARCHAR?", "n: BIGINT", "top: INTEGER"], "many"]);
  assert.deepEqual(columns(await analyze(root, "select level, count(*) as n from logs group by rollup (level)")), ["level: VARCHAR?", "n: BIGINT"]);
  // DuckDB's own syntax: FROM first, EXCLUDE, QUALIFY.
  assert.deepEqual(columns(await analyze(root, "from logs select msg, level")), ["msg: VARCHAR", "level: VARCHAR?"]);
  assert.deepEqual(columns(await analyze(root, "select * exclude (tags, meta, m, seen) from logs qualify row_number() over (partition by level order by id) = 1")), ["id: INTEGER", "msg: VARCHAR", "level: VARCHAR?", "team_id: INTEGER?"]);
  // A division by zero is NULL where the session says so (`error_on_division_by_zero`).
  assert.deepEqual(columns(await analyze(root, "select id + 1 as next, id / 2 as half, coalesce(level, 'none') as level, msg || '!' as loud from logs")), ["next: INTEGER", "half: DOUBLE?", "level: VARCHAR", "loud: VARCHAR"]);
  assert.deepEqual(columns(await analyze(root, "with recent as (select id, msg from logs where id > :n) select * from recent")), ["id: INTEGER", "msg: VARCHAR"]);
  assert.deepEqual(columns(await analyze(root, "select id from logs union all select id from teams")), ["id: INTEGER"]);
  // A statement the engine's parser does not read is typed from the describe alone.
  const summary = await analyze(root, "summarize logs");
  assert.ok(summary.columns.length > 0 && summary.columns.every((c) => c.nullable));
  assert.deepEqual([summary.cardinality, summary.cardinalityReason], ["many", "the engine does not read this statement's syntax, so nothing bounds its rows"]);
  // A PIVOT's columns are the values it reads, which DuckDB does not describe before it runs.
  assert.match((await analyze(root, "pivot logs on level using count(*)")).diagnostics[0]!.message, /^DuckDB does not describe the columns of this statement before it runs/);
});

test("DuckDB: how many rows a statement returns", { skip }, async () => {
  const root = await project();
  const card = async (sql: string) => (await analyze(root, sql)).cardinality;
  assert.equal(await card("select 1 as one"), "one");
  assert.equal(await card("select 1 as one where :b::boolean"), "maybeOne");
  assert.equal(await card("select * from logs limit 1"), "maybeOne");
  assert.equal(await card("select * from logs where id = :id and level = 'x'"), "maybeOne");
  assert.equal(await card("select * from logs where team_id = :id"), "many");
  assert.equal(await card("select unnest(tags) as tag from logs where id = :id"), "many");
  assert.equal(await card("insert into teams (id, name) values (:id, :name) returning id"), "one");
  assert.equal(await card("insert into teams (id, name) values (:id, :name) on conflict do nothing returning id"), "maybeOne");
  assert.equal(await card("insert into teams (id, name) values (1, 'a'), (2, 'b') returning id"), "many");
  assert.equal(await card("insert into teams (id, name) values (:id, :name)"), "none");
  assert.equal(await card("update logs set msg = :msg where id = :id returning msg"), "maybeOne");
  assert.equal(await card("update logs set msg = :msg where level = :level"), "none");
  assert.equal(await card("delete from logs where level = :level returning id"), "many");
  // A LIMIT that may be 0 bounds the rows to at most one, not exactly one.
  assert.equal(await card("select count(*) as n from logs limit :n"), "maybeOne");
  // A write WITH queries feed is that write.
  const fed = await analyze(root, "with x as (select 9 as id, 'x' as name) insert into teams select * from x");
  assert.deepEqual([fed.kind, fed.cardinality, fed.columns, fed.fresh?.inserts], ["insert", "none", [], ["teams"]]);
  const returned = await analyze(root, "with x as (select 9 as id, 'x' as name) insert into teams select * from x returning id, name");
  assert.deepEqual([returned.kind, returned.cardinality, columns(returned)], ["insert", "many", ["id: INTEGER", "name: VARCHAR"]]);
  const create = await analyze(root, "create table other (a integer)");
  assert.deepEqual([create.kind, create.cardinality, create.columns], ["create", "none", []]);
  // What a write's RETURNING returns is the row it wrote.
  assert.deepEqual(columns(await analyze(root, "insert into logs (id, msg) values (:id, :msg) returning id, msg, level, seen")), ["id: INTEGER", "msg: VARCHAR", "level: VARCHAR?", "seen: TIMESTAMP WITH TIME ZONE"]);
});

test("DuckDB: each parameter is typed where it stands, and may be NULL where the statement lets it", { skip }, async () => {
  const root = await project();
  const params = (a: { params: { name: string; type_name: string; nullable: boolean }[] }) => a.params.map((p) => `${p.name}: ${p.type_name}${p.nullable ? "?" : ""}`);
  assert.deepEqual(params(await analyze(root, "select id from logs where level = :level and id > :after")), ["level: VARCHAR", "after: INTEGER"]);
  assert.deepEqual(params(await analyze(root, "select :n::bigint * 2 as twice")), ["n: BIGINT"]);
  // A parameter the statement tests for NULL, coalesces, or writes bare to a column that may hold
  // NULL, and one cast to nullable(…), takes NULL.
  assert.deepEqual(params(await analyze(root, "select id from logs where coalesce(:level, level) = level")), ["level: VARCHAR?"]);
  assert.deepEqual(params(await analyze(root, "select id from logs where level is not distinct from :level")), ["level: VARCHAR?"]);
  assert.deepEqual(params(await analyze(root, "insert into logs (id, msg, level) values (:id, :msg, :level)")), ["id: INTEGER", "msg: VARCHAR", "level: VARCHAR?"]);
  assert.deepEqual(params(await analyze(root, "update logs set level = :level where id = :id")), ["level: VARCHAR?", "id: INTEGER"]);
  const nullable = await analyze(root, "select :v::nullable(varchar) as v");
  assert.deepEqual([params(nullable), columns(nullable)], [["v: VARCHAR?"], ["v: VARCHAR?"]]);
  // A parameter that takes no NULL is never NULL where it is selected.
  assert.deepEqual(columns(await analyze(root, "select :v::varchar as v")), ["v: VARCHAR"]);
  // DuckDB's own positional placeholders, and its `$name`, which Reylo does not bind.
  assert.deepEqual(params(await analyze(root, "select id from logs where level = ? and id > ?")), ["1: VARCHAR", "2: INTEGER"]);
  assert.equal((await analyze(root, "select id from logs where id = $1")).rewritten.sql, "select id from logs where id = $1");
  assert.deepEqual((await analyze(root, "select id from logs where id = $id")).diagnostics.map((d) => d.message), ["DuckDB reads $id as a parameter that Reylo does not bind; write :id"]);
  const gap = "select id from logs where id = $1 or team_id = $3";
  assert.deepEqual((await analyze(root, gap)).diagnostics.map((d) => [d.message, gap.slice(d.start, d.start + d.length)]), [["DuckDB numbers parameters from 1 without a gap, and this statement has no $2: number them in order", "$3"]]);
  // A parameter DuckDB cannot type leaves it describing none of the statement's columns, which is
  // an error at that parameter, saying to cast it.
  for (const sql of ["select :x as x", "select id, msg || :s as m from logs where id = :id", "select id + :n::integer + :m as x from logs"]) {
    const untyped = await analyze(root, sql);
    const named = sql.includes(":s") ? ":s" : sql.includes(":m") ? ":m" : ":x";
    assert.deepEqual(untyped.diagnostics.map((d) => [d.message, sql.slice(d.start, d.start + d.length)]), [[`DuckDB cannot tell the type of ${named} from where it stands, and describes none of the statement's columns until it can: cast it, as in ${named}::integer`, named]]);
  }
  // A rows(…) parameter is Postgres's, SQLite's and MySQL's for now.
  assert.match((await analyze(root, "insert into teams :rows::rows(id integer not null, name varchar not null)")).diagnostics[0]!.message, /DuckDB statements take no rows\(…\) or row\(…\) parameter yet/);
});

test("DuckDB: an error DuckDB raises is placed where its caret points", { skip }, async () => {
  const root = await project();
  const sql = "select id,\n  nope from logs where id = :id";
  const [d] = (await analyze(root, sql)).diagnostics;
  assert.equal(d!.code, 6);
  assert.match(d!.message, /^Binder Error: Referenced column "nope" not found in FROM clause!/);
  assert.equal(sql.slice(d!.start, d!.start + d!.length), "nope");
});

test("DuckDB: a project without @duckdb/node-api is told to install it, and a missing file cannot be reached", { skip }, async () => {
  const root = await project(SCHEMA, false);
  const session = await Session.open(root, { DATABASE_URL: "duckdb:./app.duckdb" });
  try {
    const outcome = await session.analyze("select 1 as one", { store: false });
    assert.ok(!outcome.ok);
    assert.match(outcome.message, /DuckDB statements are typed on the project's own DuckDB, and .* has no @duckdb\/node-api: npm install @duckdb\/node-api/);
  } finally {
    await session.close();
  }
  const other = await project();
  const missing = await Session.open(other, { DATABASE_URL: "duckdb:./nowhere.duckdb" });
  try {
    const outcome = await missing.analyze("select 1 as one", { store: false });
    assert.ok(!outcome.ok);
    assert.match(outcome.message, /^Cannot open the database .*nowhere\.duckdb \(DATABASE_URL in the environment\): IO Error: Cannot open database .* database does not exist/);
  } finally {
    await missing.close();
  }
});

test("DuckDB: a database file another process holds for writing is typed from a copy of it", { skip }, async () => {
  const root = await project();
  // A second process opens the file read-write, as a running app does, writes a table it has not
  // checkpointed, and holds the file until its stdin closes.
  const holder = `const { DuckDBInstance } = require(${JSON.stringify(join(DRIVERS, "@duckdb", "node-api"))});
(async () => {
  const c = await (await DuckDBInstance.create(${JSON.stringify(join(root, "app.duckdb"))})).connect();
  await c.run("create table later (id integer primary key, note varchar not null)");
  process.stdout.write("held\\n");
  process.stdin.resume();
  process.stdin.on("end", () => process.exit(0));
})();`;
  const child = spawn(process.execPath, ["-e", holder], { stdio: ["pipe", "pipe", "inherit"] });
  try {
    await new Promise<void>((resolve, reject) => {
      child.stdout.once("data", () => resolve());
      child.once("exit", (code) => reject(new Error(`the holder exited with ${code}`)));
    });
    assert.deepEqual(columns(await analyze(root, "select id, note from later")), ["id: INTEGER", "note: VARCHAR"]);
  } finally {
    child.stdin.end();
    await new Promise((resolve) => child.once("exit", resolve));
  }
});

test("DuckDB: the URL names the database file, relative to the project", () => {
  assert.equal(dialectOf("duckdb:./app.duckdb"), "duckdb");
  assert.equal(duckdbPath("duckdb:./app.duckdb", "/srv/app"), "/srv/app/app.duckdb");
  assert.equal(duckdbPath("duckdb:///data/app.duckdb", "/srv/app"), "/data/app.duckdb");
  assert.equal(duckdbPath("duckdb::memory:", "/srv/app"), ":memory:");
  assert.equal(duckdbPath("duckdb:", "/srv/app"), ":memory:");
});

/** The module a build step writes for `raw`, imported. */
async function built(root: string, name: string, raw: string) {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, raw);
  const { code } = await sqlModule(file, raw);
  const out = join(root, `${name}.mjs`);
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  return (await import(pathToFileURL(out).href)).default as { run(client: unknown, params?: unknown): Promise<any>; try(client: unknown, params?: unknown): Promise<any> };
}

const closeAll = async () => {
  for (const session of Session.opened()) await session.close();
};

const VALUES = `create table vals (id integer primary key, tags varchar[], meta struct(host varchar, pid integer), h hugeint, d decimal(38,2), ts timestamptz, t timestamp, dt date, i interval, b blob, j json, m map(varchar, integer), u union(num integer, str varchar), big bigint, f double, ok boolean, uid uuid, tm time, arr float[3], m2 mood);`;

test("DuckDB: .run() returns each type's one representation, and a row goes back in as it came out", { skip }, async () => {
  const root = await project(`${SCHEMA}\n${VALUES}`);
  const insert = await built(root, "insert", "insert into vals values (:id, :tags, :meta, :h, :d, :ts, :t, :dt, :i, :b, :j, :m, :u, :big, :f, :ok, :uid, :tm, :arr, :m2) returning *");
  const get = await built(root, "get", "select * from vals where id = :id");
  await closeAll();
  const db = await api().DuckDBInstance.create(join(root, "app.duckdb"));
  const c = await db.connect();
  try {
    const row = {
      id: 1,
      tags: ["a", "b"],
      meta: { host: "h", pid: 7 },
      h: 170141183460469231731687303715884105727n,
      d: "12345678901234567.89",
      ts: "2026-10-20T10:00:00.123456Z",
      t: "2026-10-20T10:00:00.5",
      dt: "2026-10-20",
      i: "P1Y2M3DT4H5M6.789S",
      b: new Uint8Array([1, 2, 255]),
      j: { a: [1, "x"] },
      m: [{ key: "k", value: 1 }],
      u: { tag: "str", value: "s" },
      big: 9007199254740993n,
      f: 1.5,
      ok: true,
      uid: "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
      tm: "12:34:56.789",
      arr: [1, 2, 3.5],
      m2: "happy",
    };
    const inserted = await insert.run(c, row);
    assert.deepEqual({ ...inserted, b: [...inserted.b] }, { ...row, b: [1, 2, 255] });
    assert.ok(inserted.b instanceof Uint8Array);
    // What `.run()` returns binds again as it is.
    const back = await insert.run(c, { ...(await get.run(c, { id: 1 })), id: 2 });
    assert.deepEqual({ ...back, b: [...back.b] }, { ...row, id: 2, b: [1, 2, 255] });
    assert.equal(await get.run(c, { id: 3 }), null);
    // A UNION takes a tag of one of its members, and a value for it.
    await assert.rejects(insert.run(c, { ...row, id: 4, u: { tag: "nope", value: 5 } }), /Parameter "u" is a UNION, so it takes \{ tag, value \} with the tag one of "num", "str"; got \{"tag":"nope","value":5\}/);
    await assert.rejects(insert.run(c, { ...row, id: 4, u: { tag: "num", value: null } }), /Parameter "u" is a UNION, and @duckdb\/node-api cannot bind one whose "num" member is null/);
    // A year past 9999, a time zone's offset and an infinite date read as ISO 8601 writes them.
    const edges = await defineQuery({ dialect: "duckdb", sql: "select '10000-01-01'::date as far, '2026-10-20 10:00:00+02'::timestamptz as zoned, 'infinity'::date as forever, interval '-1 day' as back, 1.5::decimal(10, 3) as d", params: [], cardinality: "one" }).run(c as never);
    assert.deepEqual({ ...(edges as object) }, { far: "+010000-01-01", zoned: "2026-10-20T08:00:00Z", forever: "infinity", back: "-P1D", d: "1.500" });
    // A VARIANT is the value it holds, in the form of that value's own type.
    const variant = await defineQuery({ dialect: "duckdb", sql: "select 42::variant as n, 'hi'::variant as s, {'x': 1, 'y': [true, null]}::variant as o, 9007199254740993::bigint::variant as big, '2026-10-20'::date::variant as day, null::variant as none", params: [], cardinality: "one" }).run(c as never);
    assert.deepEqual({ ...(variant as object) }, { n: 42, s: "hi", o: { x: 1, y: [true, null] }, big: 9007199254740993n, day: "2026-10-20", none: null });
  } finally {
    c.closeSync();
    db.closeSync();
  }
});

test("DuckDB: every type's least, greatest and NULL value round-trips through .run() exactly", { skip }, async () => {
  // DuckDB's own test_all_types(): each type's extremes, which .run() decodes and a parameter
  // writes back, each column into a table of its own; DuckDB then compares each with the original.
  // An array that holds NULL is not written: an array parameter takes no NULL element.
  const root = await project("create table t as select row_number() over () as k, * exclude (geometry) from test_all_types()");
  const db0 = await api().DuckDBInstance.create(join(root, "app.duckdb"));
  const c0 = await db0.connect();
  const names = ((await c0.runAndReadAll("select column_name from duckdb_columns() where table_name = 't' and column_name <> 'k' order by column_index")) as { getRowsJson(): string[][] }).getRowsJson().map((r) => r[0]!);
  const q = (n: string) => `"${n}"`;
  for (const n of names) await c0.run(`create table ${q(`w_${n}`)} as select k, ${q(n)} from t limit 0`);
  c0.closeSync();
  db0.closeSync();
  const read = await built(root, "read", "select * from t order by k");
  const writes = await Promise.all(names.map((n, i) => built(root, `write_${i}`, `insert into ${q(`w_${n}`)} (k, ${q(n)}) values (:k, :v)`)));
  await closeAll();
  const db = await api().DuckDBInstance.create(join(root, "app.duckdb"));
  const c = await db.connect();
  try {
    const rows = (await read.run(c)) as Record<string, unknown>[];
    assert.equal(rows.length, 3);
    const nullElement = (v: unknown): boolean => Array.isArray(v) && v.some((e) => e === null || nullElement(e));
    for (const [i, n] of names.entries()) {
      for (const row of rows) {
        if (nullElement(row[n])) continue;
        await writes[i]!.run(c, { k: row.k, v: row[n] });
      }
      const differ = ((await c.runAndReadAll(`select t.k from t join ${q(`w_${n}`)} w using (k) where t.${q(n)} is distinct from w.${q(n)}`)) as { getRowsJson(): unknown[][] }).getRowsJson();
      assert.deepEqual(differ, [], `${n} differs after a round trip`);
    }
  } finally {
    c.closeSync();
    db.closeSync();
  }
});

test("DuckDB: .run() refuses a fraction for an integer, and .try() reads DuckDB's error", { skip }, async () => {
  const root = await project();
  const get = await built(root, "get", "select id, msg from logs where id = :id");
  const insert = await built(root, "insert", "insert into teams (id, name) values (:id, :name)");
  await closeAll();
  const db = await api().DuckDBInstance.create(join(root, "app.duckdb"));
  const c = await db.connect();
  try {
    await assert.rejects(get.run(c, { id: 1.5 }), /Parameter "id" is an integer, and 1\.5 has a fraction, which DuckDB would round away/);
    await assert.rejects(get.run(c, { id: "1.5" }), /Parameter "id" is an integer, so a string for it holds the digits alone/);
    assert.equal(await insert.run(c, { id: 1, name: "a" }), undefined);
    const tried = await insert.try(c, { id: 1, name: "b" });
    assert.equal(tried.error.name, "QueryError");
    assert.equal(tried.error.code, "CONSTRAINT");
    assert.match(tried.error.message, /^Constraint Error: Duplicate key "id: 1" violates primary key constraint/);
  } finally {
    c.closeSync();
    db.closeSync();
  }
});

test("DuckDB: a DuckDB connection is never taken for better-sqlite3", { skip }, async () => {
  const db = await api().DuckDBInstance.create(":memory:");
  const c = await db.connect();
  try {
    const sqlite = defineQuery({ dialect: "sqlite", sql: "select 1 as one", params: [], cardinality: "one" });
    await assert.rejects(sqlite.run(c as never), /It is a DuckDB connection, and the statement is typed for sqlite: set the project's URL to the DuckDB database/);
  } finally {
    c.closeSync();
    db.closeSync();
  }
});

test("DuckDB: a list binds through a subclass of DuckDBConnection, and through an object that wraps one", { skip }, async () => {
  const node = createRequire(join(DRIVERS, "package.json"))("@duckdb/node-api") as Api & { DuckDBConnection: new (...args: never[]) => Connection };
  const db = await node.DuckDBInstance.create(":memory:");
  const c = await db.connect();
  try {
    class Logged extends node.DuckDBConnection {}
    Object.setPrototypeOf(c, Logged.prototype);
    const wrapped = { runAndReadAll: c.runAndReadAll.bind(c), prepare: c.prepare.bind(c), run: c.run.bind(c) };
    const tags = defineQuery({ dialect: "duckdb", sql: "select :tags::varchar[] as tags", params: ["tags"], text: "select $1::varchar[] as tags", order: ["tags"], cardinality: "one" });
    for (const client of [c, wrapped]) assert.deepEqual({ ...((await tags.run(client as never, { tags: ["a", "b"] })) as object) }, { tags: ["a", "b"] });
  } finally {
    c.closeSync();
    db.closeSync();
  }
});

test("DuckDB: an ISO 8601 duration is bound as DuckDB's own interval text", () => {
  assert.equal(duckdbInterval("P1Y2M3DT4H5M6.789S"), "1 years 2 months 3 days 4 hours 5 minutes 6.789 seconds");
  assert.equal(duckdbInterval("-P1DT2H"), "-1 days -2 hours");
  assert.equal(duckdbInterval("P1M-1D"), "1 months -1 days");
  assert.equal(duckdbInterval("1 day"), "1 day");
});

test("DuckDB: the module declares the types .run() returns", { skip }, async () => {
  const root = await project(`${SCHEMA}\n${VALUES}`);
  const sql = "select id, tags, meta, h, d, ts, b, j, m, u, m2 from vals where id = :id\n";
  const declared = (await declarations(root, { "get.sql": sql }))["get.sql"]!;
  const line = declared.split("\n").find((l) => l.startsWith("declare const query:"))!;
  assert.match(line, /^declare const query: Query<\{ id: number \}, \{ id: number; tags: \(string \| null\)\[\] \| null; meta: \{ host: string \| null; pid: number \| null \} \| null; h: bigint \| null; d: string \| null; ts: string \| null; b: Uint8Array \| null; j: unknown \| null; m: \{ key: string; value: number \| null \}\[\] \| null; u: \{ tag: "num"; value: number \| null \} \| \{ tag: "str"; value: string \| null \} \| null; m2: "happy" \| "sad" \| null \}, "maybeOne", "duckdb", QueryError> & /, line);
});

/** The declaration the content mapper writes for each file, through its own JSON-RPC process, as the type check runs it. */
async function declarations(root: string, files: Record<string, string>): Promise<Record<string, string>> {
  const child = spawn(process.execPath, [MAPPER], { cwd: root, stdio: ["pipe", "pipe", "inherit"] });
  const pending = new Map<number, (result: unknown) => void>();
  let buffer = Buffer.alloc(0);
  child.stdout.on("data", (chunk: Buffer) => {
    buffer = Buffer.concat([buffer, chunk]);
    for (;;) {
      const end = buffer.indexOf("\r\n\r\n");
      if (end === -1) return;
      const length = Number(/Content-Length:\s*(\d+)/i.exec(buffer.subarray(0, end).toString("ascii"))![1]);
      if (buffer.length < end + 4 + length) return;
      const message = JSON.parse(buffer.subarray(end + 4, end + 4 + length).toString("utf8"));
      buffer = buffer.subarray(end + 4 + length);
      pending.get(message.id)?.(message.error ? Promise.reject(new Error(message.error.message)) : message.result);
    }
  });
  let id = 0;
  const call = (method: string, params: unknown) =>
    new Promise<any>((resolve) => {
      pending.set(++id, resolve);
      const body = Buffer.from(JSON.stringify({ jsonrpc: "2.0", id, method, params }), "utf8");
      child.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
      child.stdin.write(body);
    });
  try {
    await call("initialize", { protocolVersion: 1 });
    await call("openProject", { projectHandle: "p", configFileName: join(root, "tsconfig.json"), options: {} });
    const out: Record<string, string> = {};
    for (const [name, content] of Object.entries(files)) {
      const result = await call("transform", { fileName: join(root, name), content, projectHandle: "p" });
      assert.deepEqual(result.diagnostics, [], `${name}: ${JSON.stringify(result.diagnostics)}`);
      out[name] = result.text;
    }
    await call("closeProject", { projectHandle: "p" });
    return out;
  } finally {
    child.stdin.end();
    await new Promise((resolve) => child.once("exit", resolve));
  }
}
