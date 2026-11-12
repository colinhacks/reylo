import { test } from "node:test";
import type { EngineType } from "../src/engine-type.ts";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { Session, type Outcome } from "../src/analyze.ts";
import { sqlModule } from "../src/loader.ts";
import { defineQuery } from "../src/index.ts";
import { duckdbInterval } from "../src/decode-duckdb.ts";
import { dialectOf, duckdbPath } from "../src/project.ts";
import { declarations, DRIVERS, INDEX, closeSessions } from "./helpers.ts";

// DuckDB runs inside the app, so a project's statements are typed on its own @duckdb/node-api,
// the labs' (lab/drivers), linked into each project as an install would put it.
const skip = existsSync(join(DRIVERS, "@duckdb", "node-api")) ? false : "lab/drivers is not installed (no @duckdb/node-api)";

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

/** A type as DuckDB names it, lowercased: an array its element's and `[]`, a struct its fields, an enum its labels. */
function shown(t: EngineType): string {
  if (t.kind === "array") return `${shown(t.element.type)}[]`;
  if (t.kind === "record") return `struct(${t.fields.map((f) => `${f.name} ${shown(f.type)}`).join(", ")})`;
  if (t.kind === "text" && t.enum !== undefined) return `enum(${t.enum.join(", ")})`;
  return "label" in t && t.label !== undefined ? t.label : t.kind;
}

/** Each column as `name: type`, with `?` where it may be NULL. */
const columns = (a: { columns: { name: string; type: EngineType; nullable?: boolean }[] }) => a.columns.map((c) => `${c.name}: ${shown(c.type)}${c.nullable ? "?" : ""}`);

test("DuckDB: each column is typed by the prepared statement, and may be NULL unless the engine shows it is not", { skip }, async () => {
  const root = await project();
  const one = await analyze(root, "select id, msg, level, tags, meta, m from logs where id = :id");
  assert.deepEqual(columns(one), ["id: integer", "msg: varchar", "level: varchar?", "tags: varchar[]?", "meta: struct(host varchar, pid integer)?", "m: enum(happy, sad)?"]);
  assert.equal(one.cardinality, "at_most_one");
  assert.equal(one.kind, "select");
  assert.equal(one.columns[1]!.comment, "What happened");
  assert.deepEqual(one.columns[4]!.type, { kind: "record", fields: [{ name: "host", type: { kind: "text", label: "varchar" }, nullable: true }, { name: "pid", type: { kind: "integer", bits: 32, label: "integer" }, nullable: true }] });
  assert.deepEqual(one.columns[5]!.type, { kind: "text", enum: ["happy", "sad"], label: "enum" });
  // An outer join pads its other side with NULL, unless a filter shows the row was matched.
  assert.deepEqual(columns(await analyze(root, "select l.id, t.name from logs l left join teams t on t.id = l.team_id")), ["id: integer", "name: varchar?"]);
  assert.deepEqual(columns(await analyze(root, "select l.id, t.name from logs l left join teams t on t.id = l.team_id where t.id is not null")), ["id: integer", "name: varchar"]);
  assert.deepEqual(columns(await analyze(root, "select l.id, t.name from teams t right join logs l on t.id = l.team_id")), ["id: integer", "name: varchar?"]);
  // A count is never NULL; another aggregate is, over no rows, but not over a group of them.
  const totals = await analyze(root, "select count(*) as n, max(id) as top from logs");
  assert.deepEqual([columns(totals), totals.cardinality], [["n: bigint", "top: integer?"], "one"]);
  const grouped = await analyze(root, "select level, count(*) as n, max(id) as top from logs group by level");
  assert.deepEqual([columns(grouped), grouped.cardinality], [["level: varchar?", "n: bigint", "top: integer"], "many"]);
  assert.deepEqual(columns(await analyze(root, "select level, count(*) as n from logs group by rollup (level)")), ["level: varchar?", "n: bigint"]);
  // DuckDB's own syntax: FROM first, EXCLUDE, QUALIFY.
  assert.deepEqual(columns(await analyze(root, "from logs select msg, level")), ["msg: varchar", "level: varchar?"]);
  assert.deepEqual(columns(await analyze(root, "select * exclude (tags, meta, m, seen) from logs qualify row_number() over (partition by level order by id) = 1")), ["id: integer", "msg: varchar", "level: varchar?", "team_id: integer?"]);
  // A division by zero is NULL where the session says so (`error_on_division_by_zero`).
  assert.deepEqual(columns(await analyze(root, "select id + 1 as next, id / 2 as half, coalesce(level, 'none') as level, msg || '!' as loud from logs")), ["next: integer", "half: double?", "level: varchar", "loud: varchar"]);
  assert.deepEqual(columns(await analyze(root, "with recent as (select id, msg from logs where id > :n) select * from recent")), ["id: integer", "msg: varchar"]);
  assert.deepEqual(columns(await analyze(root, "select id from logs union all select id from teams")), ["id: integer"]);
  // A statement the engine's parser does not read is typed from the describe alone.
  const summary = await analyze(root, "summarize logs");
  assert.ok(summary.columns.length > 0 && summary.columns.every((c) => c.nullable));
  assert.deepEqual([summary.cardinality, summary.cardinalityReason], ["many", "the engine does not read this statement's syntax, so nothing bounds its rows"]);
  // A PIVOT's columns are the values it reads, which DuckDB does not describe before it runs.
  assert.match((await analyze(root, "pivot logs on level using count(*)")).diagnostics[0]!.message, /^DuckDB does not describe the columns of this statement before it runs/);
});

test("DuckDB: a UUID a call makes has its version, and a user's macro of a built-in's name is the macro", { skip }, async () => {
  const root = await project(`create table events (id uuid primary key default uuidv7());
create macro now() as null;`);
  const made = await analyze(root, "select uuid() as a, gen_random_uuid() as b, uuidv4() as c, uuidv7() as d, id, now() as at from events");
  assert.deepEqual(made.columns.slice(0, 5).map((c) => (c.type.kind === "uuid" ? (c.type.version ?? null) : c.type.kind)), [4, 4, 4, 7, null]);
  // `now()` is the macro, NULL, where the built-in never is.
  assert.equal(made.columns[5]!.nullable, true);
  const shadowed = await project("create macro uuidv7() as '00000000-0000-0000-0000-000000000000'::uuid;");
  const nil = await analyze(shadowed, "select uuidv7() as id");
  assert.deepEqual([nil.columns[0]!.type.kind, (nil.columns[0]!.type as { version?: number }).version], ["uuid", undefined]);
});

test("DuckDB: an array of a subquery's whole rows is never NULL, nor is its element, and each field is as its column is", { skip }, async () => {
  const root = await project("create table posts (id integer primary key); create table comments (id integer primary key, post_id integer not null, body varchar); create table replies (id integer primary key, comment_id integer not null, note varchar not null)");
  const fields = (t: EngineType): unknown => (t.kind === "array" && t.element.type.kind === "record" ? { element_nullable: t.element.nullable ?? false, fields: t.element.type.fields.map((f) => [f.name, f.nullable ?? false, fields(f.type)]) } : undefined);
  const column = async (sql: string) => {
    const [, c] = (await analyze(root, sql)).columns;
    return { nullable: c!.nullable ?? false, ...(fields(c!.type) as object) };
  };
  assert.deepEqual(await column("select p.id, array(select c from (select id, body from comments where post_id = p.id) c) as comments from posts p"), {
    nullable: false,
    element_nullable: false,
    fields: [["id", false, undefined], ["body", true, undefined]],
  });
  assert.deepEqual(await column("select p.id, array(select x from (select c.id, array(select r from (select note from replies where comment_id = c.id) r) as rs from comments c where c.post_id = p.id) x) as cs from posts p"), {
    nullable: false,
    element_nullable: false,
    fields: [["id", false, undefined], ["rs", false, { element_nullable: false, fields: [["note", false, undefined]] }]],
  });
  // An item an outer join pads may leave its row NULL, and its fields say nothing then.
  assert.deepEqual(await column("select 1 as one, array(select r from (select 1 as x) x left join replies r on r.id = x.x) as rs"), {
    nullable: false,
    element_nullable: true,
    fields: [["id", true, undefined], ["comment_id", true, undefined], ["note", true, undefined]],
  });
});

test("DuckDB: how many rows a statement returns", { skip }, async () => {
  const root = await project();
  const card = async (sql: string) => (await analyze(root, sql)).cardinality;
  assert.equal(await card("select 1 as one"), "one");
  assert.equal(await card("select 1 as one where :b::boolean"), "at_most_one");
  assert.equal(await card("select * from logs limit 1"), "at_most_one");
  assert.equal(await card("select * from logs where id = :id and level = 'x'"), "at_most_one");
  assert.equal(await card("select * from logs where team_id = :id"), "many");
  assert.equal(await card("select unnest(tags) as tag from logs where id = :id"), "many");
  assert.equal(await card("insert into teams (id, name) values (:id, :name) returning id"), "one");
  assert.equal(await card("insert into teams (id, name) values (:id, :name) on conflict do nothing returning id"), "at_most_one");
  assert.equal(await card("insert into teams (id, name) values (1, 'a'), (2, 'b') returning id"), "many");
  assert.equal(await card("insert into teams (id, name) values (:id, :name)"), "none");
  assert.equal(await card("update logs set msg = :msg where id = :id returning msg"), "at_most_one");
  assert.equal(await card("update logs set msg = :msg where level = :level"), "none");
  assert.equal(await card("delete from logs where level = :level returning id"), "many");
  // A LIMIT that may be 0 bounds the rows to at most one, not exactly one.
  assert.equal(await card("select count(*) as n from logs limit :n"), "at_most_one");
  // A write WITH queries feed is that write.
  const fed = await analyze(root, "with x as (select 9 as id, 'x' as name) insert into teams select * from x");
  assert.deepEqual([fed.kind, fed.cardinality, fed.columns, fed.fresh?.inserts], ["insert", "none", [], ["teams"]]);
  const returned = await analyze(root, "with x as (select 9 as id, 'x' as name) insert into teams select * from x returning id, name");
  assert.deepEqual([returned.kind, returned.cardinality, columns(returned)], ["insert", "many", ["id: integer", "name: varchar"]]);
  const create = await analyze(root, "create table other (a integer)");
  assert.deepEqual([create.kind, create.cardinality, create.columns], ["other", "none", []]);
  // What a write's RETURNING returns is the row it wrote.
  assert.deepEqual(columns(await analyze(root, "insert into logs (id, msg) values (:id, :msg) returning id, msg, level, seen")), ["id: integer", "msg: varchar", "level: varchar?", "seen: timestamptz"]);
});

test("DuckDB: each parameter is typed where it stands, and may be NULL where the statement lets it", { skip }, async () => {
  const root = await project();
  const params = (a: { params: { name: string; type: EngineType; nullable?: boolean }[] }) => a.params.map((p) => `${p.name}: ${shown(p.type)}${p.nullable ? "?" : ""}`);
  assert.deepEqual(params(await analyze(root, "select id from logs where level = :level and id > :after")), ["level: varchar", "after: integer"]);
  assert.deepEqual(params(await analyze(root, "select :n::bigint * 2 as twice")), ["n: bigint"]);
  // A parameter the statement tests for NULL, coalesces, or writes bare to a column that may hold
  // NULL, and one cast to nullable(…), takes NULL.
  assert.deepEqual(params(await analyze(root, "select id from logs where coalesce(:level, level) = level")), ["level: varchar?"]);
  assert.deepEqual(params(await analyze(root, "select id from logs where level is not distinct from :level")), ["level: varchar?"]);
  assert.deepEqual(params(await analyze(root, "insert into logs (id, msg, level) values (:id, :msg, :level)")), ["id: integer", "msg: varchar", "level: varchar?"]);
  assert.deepEqual(params(await analyze(root, "update logs set level = :level where id = :id")), ["level: varchar?", "id: integer"]);
  const nullable = await analyze(root, "select :v::nullable(varchar) as v");
  assert.deepEqual([params(nullable), columns(nullable)], [["v: varchar?"], ["v: varchar?"]]);
  // A parameter that takes no NULL is never NULL where it is selected.
  assert.deepEqual(columns(await analyze(root, "select :v::varchar as v")), ["v: varchar"]);
  // DuckDB's own positional placeholders, and its `$name`, which Reylo does not bind.
  assert.deepEqual(params(await analyze(root, "select id from logs where level = ? and id > ?")), ["1: varchar", "2: integer"]);
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
  // A struct whose fields have no names, as (1, 'a') and row(…) build, is refused where a column
  // or a parameter holds one, inside a list too: a value's fields are read by their names.
  const tuples = await analyze(root, "select (id, msg) as pair, [row(id)] as singles, {'id': id} as named from logs where (id, msg) = :p");
  assert.deepEqual(tuples.diagnostics.map((d) => d.code), [22, 22, 22]);
  assert.equal(tuples.diagnostics[0]!.message, `Column "pair" holds a struct whose fields have no names, as (1, 'a') and row(…) build, and a value's fields are read by their names: name them, as in {'a': 1, 'b': 'x'} or struct_pack(a := 1, b := 'x')`);
  assert.equal(tuples.diagnostics[2]!.message, ":p takes a struct whose fields have no names, and a value's fields are read by their names: cast it to a struct that names them, as in :p::struct(a integer, b varchar)");
  assert.deepEqual(columns(tuples), ["pair: unknown", "singles: unknown[]", "named: struct(id integer)"]);
});

test("DuckDB: an error DuckDB raises is placed where its caret points", { skip }, async () => {
  const root = await project();
  const sql = "select id,\n  nope from logs where id = :id";
  const [d] = (await analyze(root, sql)).diagnostics;
  assert.equal(d!.code, 6);
  assert.match(d!.message, /^Binder Error: Referenced column "nope" not found in FROM clause!/);
  assert.equal(sql.slice(d!.start, d!.start + d!.length), "nope");
});

test("DuckDB: a SELECT is read-only unless a WITH query writes or it may move a sequence, itself or through a view or a macro", { skip }, async () => {
  const root = await project(`create sequence s;
    create table t (id integer);
    create view plain_view as select id from t;
    create view moving_view as select nextval('s') as id;
    create view moving_again as select id from moving_view;
    create macro moving_macro() as nextval('s');
    create macro moving_table() as table select nextval('s') as id;
    create macro plain_macro(a) as a + 1;`);
  const cases: [string, boolean][] = [
    ["select id from t", true],
    ["select id from plain_view", true],
    ["select plain_macro(id) from t", true],
    ["insert into t values (1)", false],
    // A WITH query that writes runs with the SELECT that reads it.
    ["with gone as (delete from t where id = 1 returning id) select count(*) as n from gone", false],
    ["with made as (insert into t values (9) returning id) select id from made", false],
    ["with moved as (update t set id = id + 1 returning id), kept as (select id from t) select kept.id from kept, moved", false],
    ["select nextval('s')", false],
    ["select id from moving_view", false],
    ["select id from moving_again", false],
    ["select moving_macro()", false],
    ["select id from moving_table()", false],
    ["select * from query('select nextval(''s'')')", false],
  ];
  for (const [sql, expected] of cases) assert.equal((await analyze(root, sql)).readOnly ?? false, expected, sql);
  await closeSessions();
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

test("DuckDB: a database file another process holds for writing is typed from a copy of it, which Windows refuses", { skip }, async () => {
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
    if (process.platform !== "win32") {
      assert.deepEqual(columns(await analyze(root, "select id, note from later")), ["id: integer", "note: varchar"]);
    } else {
      // On Windows, DuckDB shares no file it writes, so there is no copy to read: the refusal names the holder.
      const session = await Session.open(root, { DATABASE_URL: "duckdb:./app.duckdb" });
      try {
        const outcome = await session.analyze("select id, note from later", { store: false });
        assert.ok(!outcome.ok);
        assert.match(outcome.message, /\(PID \d+\) holds it for writing, and on Windows no other process can read it meanwhile, not even to copy it: type-check while that process is stopped/);
      } finally {
        await session.close();
      }
    }
  } finally {
    child.stdin.end();
    await new Promise((resolve) => child.once("exit", resolve));
  }
});

test("DuckDB: the URL names the database file, relative to the project", () => {
  assert.equal(dialectOf("duckdb:./app.duckdb"), "duckdb");
  const base = resolve("/srv/app");
  assert.equal(duckdbPath("duckdb:./app.duckdb", base), join(base, "app.duckdb"));
  assert.equal(duckdbPath("duckdb:///data/app.duckdb", base), "/data/app.duckdb");
  // On Windows the URL's path names a drive, as a file URL's does.
  if (process.platform === "win32") assert.equal(duckdbPath("duckdb:///C:/data/app.duckdb", base), "C:/data/app.duckdb");
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

const VALUES = `create table vals (id integer primary key, tags varchar[], meta struct(host varchar, pid integer), h hugeint, d decimal(38,2), ts timestamptz, t timestamp, dt date, i interval, b blob, j json, m map(varchar, integer), u union(num integer, str varchar), big bigint, f double, ok boolean, uid uuid, tm time, arr float[3], m2 mood);`;

test("DuckDB: .run() returns each type's one representation, and a row goes back in as it came out", { skip }, async () => {
  const root = await project(`${SCHEMA}\n${VALUES}`);
  const insert = await built(root, "insert", "insert into vals values (:id, :tags, :meta, :h, :d, :ts, :t, :dt, :i, :b, :j, :m, :u, :big, :f, :ok, :uid, :tm, :arr, :m2) returning *");
  const get = await built(root, "get", "select * from vals where id = :id");
  await closeSessions();
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
  // A struct whose fields have no names, the `tuple` column, is refused (tested above).
  const root = await project("create table t as select row_number() over () as k, * exclude (\"tuple\") from test_all_types()");
  const db0 = await api().DuckDBInstance.create(join(root, "app.duckdb"));
  const c0 = await db0.connect();
  const names = ((await c0.runAndReadAll("select column_name from duckdb_columns() where table_name = 't' and column_name <> 'k' order by column_index")) as { getRowsJson(): string[][] }).getRowsJson().map((r) => r[0]!);
  const q = (n: string) => `"${n}"`;
  for (const n of names) await c0.run(`create table ${q(`w_${n}`)} as select k, ${q(n)} from t limit 0`);
  c0.closeSync();
  db0.closeSync();
  const read = await built(root, "read", "select * from t order by k");
  const writes = await Promise.all(names.map((n, i) => built(root, `write_${i}`, `insert into ${q(`w_${n}`)} (k, ${q(n)}) values (:k, :v)`)));
  await closeSessions();
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

test("DuckDB: a JSON parameter is bound as the JSON of its value, a number a number, alone and inside a list, a struct and a map", { skip }, async () => {
  const root = await project("create table jt (id integer primary key, j json not null, l json[], s struct(j json), m map(varchar, json))");
  const one = await built(root, "json_one", "insert into jt (id, j) values (:id, :v) returning j::varchar as text, json_type(j) as kind");
  const nested = await built(root, "json_nested", "insert into jt (id, j, l, s, m) values (:id, '1', :l, :s, :m) returning l, s, m, l::varchar as text");
  await closeSessions();
  const db = await api().DuckDBInstance.create(join(root, "app.duckdb"));
  const c = await db.connect();
  try {
    const stored = async (id: number, v: unknown) => ({ ...(await one.run(c, { id, v })) });
    assert.deepEqual(await stored(1, 5), { text: "5", kind: "UBIGINT" });
    assert.deepEqual(await stored(2, 1.5), { text: "1.5", kind: "DOUBLE" });
    assert.deepEqual(await stored(3, "5"), { text: `"5"`, kind: "VARCHAR" });
    assert.deepEqual(await stored(4, { a: [1, "x"] }), { text: `{"a":[1,"x"]}`, kind: "OBJECT" });
    // A string inside a list, a struct or a map is a JSON string, not JSON to parse, and an array is one JSON array.
    const l = ["5", 5, "abc", { a: 1 }, [1, 2]];
    const row = await nested.run(c, { id: 5, l, s: { j: "5" }, m: [{ key: "k", value: [1] }, { key: "k2", value: "null" }] });
    assert.deepEqual({ ...row }, { l, s: { j: "5" }, m: [{ key: "k", value: [1] }, { key: "k2", value: "null" }], text: `["5", 5, "abc", {"a":1}, [1,2]]` });
  } finally {
    c.closeSync();
    db.closeSync();
  }
});

test("DuckDB: the schema's fingerprint moves with a macro's parameters and an enum's labels, which an analysis reads", { skip }, async () => {
  const root = await project("create macro pick(a, b) as a; create macro typed(a integer) as a; create type mood as enum ('happy', 'sad')");
  // A session per call, as a process of its own, so that none answers from a fingerprint it took.
  const fingerprint = async () => {
    const session = await Session.open(root, { DATABASE_URL: "duckdb:./app.duckdb" });
    try {
      return (await session.schema()).value;
    } finally {
      await session.close();
    }
  };
  let last = await fingerprint();
  assert.equal(await fingerprint(), last);
  for (const change of ["create or replace macro pick(b, a) as a", "create or replace macro typed(a varchar) as a", "drop type mood; create type mood as enum ('happy', 'sad', 'new')"]) {
    const db = await api().DuckDBInstance.create(join(root, "app.duckdb"));
    const c = await db.connect();
    await c.run(change);
    c.closeSync();
    db.closeSync();
    const next = await fingerprint();
    assert.notEqual(next, last, change);
    last = next;
  }
});

test("DuckDB: .run() refuses a fraction for an integer, and .try() reads DuckDB's error", { skip }, async () => {
  const root = await project();
  const get = await built(root, "get", "select id, msg from logs where id = :id");
  const insert = await built(root, "insert", "insert into teams (id, name) values (:id, :name)");
  await closeSessions();
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

test("DuckDB: a Standard Query's decode of node-api's getRows() is .run()'s result, for every type and every value nested in another", { skip }, async () => {
  const root = await project(`${SCHEMA}\ncreate table t as select * from test_all_types();`);
  const nested = `select [[1, 2], [3]] as grid, [date '2026-10-20', null] as days, [timestamptz '2026-10-20 10:00:00+02'] as instants, {'d': date '2026-10-20', 'n': 1.50::decimal(10, 2), 'at': timestamp '2026-10-20 10:00:00.5', 'zoned': timestamptz '2026-10-20 10:00:00+02', 'big': 9007199254740993::bigint, 'tags': ['a', null], 'blob': '\\x01'::blob} as s, map {'k': interval 1 day} as m, union_value(num := 2)::union(num integer, str varchar) as u, (1, 'x') as tup, 42::variant as v, '{"a": [1]}'::json as j, ['{"b": 2}'::json] as js, timestamp_s '2026-10-20 10:00:00' as secs, timestamp_ms '2026-10-20 10:00:00.123' as millis, timestamp_ns '2026-10-20 10:00:00.123456789' as nanos, time '12:34:56.5' as tm, uuid 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11' as id`;
  const queries = [await built(root, "all", "select * from t"), await built(root, "nested", nested)];
  await closeSessions();
  const db = await api().DuckDBInstance.create(join(root, "app.duckdb"));
  const c = await db.connect();
  try {
    for (const q of queries) {
      const sq = (q as unknown as { "~standard": { query: { text: string; decode(rows: unknown[][]): unknown; columns: { name: string; decode(v: unknown): unknown }[]; output(rows: Record<string, unknown>[]): unknown } } })["~standard"].query;
      const read = async () => ((await c.runAndReadAll(sq.text)) as { getRows(): unknown[][] }).getRows();
      const run = await q.run(c);
      assert.deepStrictEqual(sq.decode(await read()), run);
      // Each column's decoder gives the same values, which `output` makes the same result.
      const rows = (await read()).map((r) => Object.fromEntries(sq.columns.map((col, i) => [col.name, r[i] === null ? null : col.decode(r[i])])));
      assert.deepStrictEqual(sq.output(rows), run);
    }
  } finally {
    c.closeSync();
    db.closeSync();
  }
});

test("DuckDB: a column named __proto__ is an own property of the row, never its prototype", { skip }, async () => {
  const db = await api().DuckDBInstance.create(":memory:");
  const c = await db.connect();
  try {
    const q = defineQuery({ dialect: "duckdb", sql: `select 1 as id, {'leaked': true} as "__proto__"`, params: [], cardinality: "one" });
    const row = (await q.run(c as never)) as Record<string, unknown>;
    assert.equal(Object.getPrototypeOf(row), Object.prototype);
    assert.deepEqual(Object.keys(row), ["id", "__proto__"]);
    assert.deepEqual(Object.getOwnPropertyDescriptor(row, "__proto__")?.value, { leaked: true });
    assert.equal((row as { leaked?: boolean }).leaked, undefined);
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

test("DuckDB: a UNION of one member is a union, declared, returned and bound as its tag and value", { skip }, async () => {
  const root = await project("create table single (id integer primary key, u union(k integer) not null);");
  const sql = "select u from single where id = :id\n";
  const declared = (await declarations(root, { "one.sql": sql }))["one.sql"]!;
  const line = declared.split("\n").find((l) => l.startsWith("declare const query:"))!;
  assert.match(line, /^declare const query: Query<\{ id: number \}, \{ u: \{ tag: "k"; value: number \| null \} \}, "maybeOne", "duckdb", QueryError> & /, line);
  const insert = await built(root, "insert", "insert into single values (:id, :u) returning u");
  const get = await built(root, "get", sql);
  await closeSessions();
  const db = await api().DuckDBInstance.create(join(root, "app.duckdb"));
  const c = await db.connect();
  try {
    assert.deepEqual({ ...(await insert.run(c, { id: 1, u: { tag: "k", value: 5 } })) }, { u: { tag: "k", value: 5 } });
    assert.deepEqual({ ...(await get.run(c, { id: 1 })) }, { u: { tag: "k", value: 5 } });
  } finally {
    c.closeSync();
    db.closeSync();
  }
});

test("DuckDB: a GEOMETRY is its WKB bytes, declared unknown as a MySQL GEOMETRY is, and binds from its bytes or its WKT", { skip }, async () => {
  const root = await project("create table shapes (id integer primary key, g geometry not null, gs geometry[]);");
  const sql = "select g, gs from shapes where id = :id\n";
  const declared = (await declarations(root, { "one.sql": sql }))["one.sql"]!;
  const line = declared.split("\n").find((l) => l.startsWith("declare const query:"))!;
  assert.match(line, /^declare const query: Query<\{ id: number \}, \{ g: unknown; gs: \(unknown \| null\)\[\] \| null \}, "maybeOne", "duckdb", QueryError> & /, line);
  const insert = await built(root, "insert", "insert into shapes values (:id, :g, :gs) returning g, gs, g::varchar as wkt");
  const get = await built(root, "get", sql);
  await closeSessions();
  const db = await api().DuckDBInstance.create(join(root, "app.duckdb"));
  const c = await db.connect();
  try {
    // POINT(1 2) as WKB: little-endian, type 1, then x and y as doubles.
    const point = Uint8Array.from(Buffer.from("0101000000000000000000f03f0000000000000040", "hex"));
    const hex = (v: unknown) => (assert.ok(v instanceof Uint8Array, `${v} is not bytes`), Buffer.from(v).toString("hex"));
    const inserted = await insert.run(c, { id: 1, g: point, gs: [point, point] });
    assert.deepEqual([hex(inserted.g), inserted.gs.map(hex), inserted.wkt], [hex(point), [hex(point), hex(point)], "POINT (1 2)"]);
    // Its WKT binds too, and what `.run()` returns binds again as it is.
    assert.equal(hex((await insert.run(c, { id: 2, g: "POINT(1 2)", gs: null })).g), hex(point));
    const row = await get.run(c, { id: 1 });
    assert.equal(hex((await insert.run(c, { ...row, id: 3 })).g), hex(point));
    const plain = await defineQuery({ dialect: "duckdb", sql: "select 'LINESTRING(0 0, 1 1)'::geometry as g", params: [], cardinality: "one" }).run(c as never);
    assert.equal(hex((plain as { g: unknown }).g), "01020000000200000000000000000000000000000000000000000000000000f03f000000000000f03f");
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
  assert.match(line, /^declare const query: Query<\{ id: number \}, \{ id: number; tags: \(string \| null\)\[\] \| null; meta: \{ host: string \| null; pid: number \| null \} \| null; h: bigint \| null; d: string \| null; ts: string \| null; b: Uint8Array \| null; j: Json; m: \{ key: string; value: number \| null \}\[\] \| null; u: \{ tag: "num"; value: number \| null \} \| \{ tag: "str"; value: string \| null \} \| null; m2: "happy" \| "sad" \| null \}, "maybeOne", "duckdb", QueryError> & /, line);
});
