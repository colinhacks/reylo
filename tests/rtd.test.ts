// `reylo/rtd`: each statement's descriptor in Reylo Type Defs, from `analyze({ url, sql })`, the
// same as `reylo analyze` prints for it.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { analyze, RTD_VERSION, type StatementDescriptor } from "../src/rtd.ts";
import { nativeBinary } from "../src/native.ts";
import { DRIVERS, mysqlDatabase, mysqlLive, pgLive, postgresDatabase } from "./helpers.ts";

/** What `reylo analyze --url <url> --sql <sql>` prints for one statement, run from a directory of its own. */
function cli(url: string, sql: string): StatementDescriptor {
  const cwd = mkdtempSync(join(tmpdir(), "reylo-rtd-cli-"));
  writeFileSync(join(cwd, "package.json"), "{}");
  const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => name !== "DATABASE_URL" && name !== "REYLO_CONFIG"));
  const run = spawnSync(nativeBinary(), ["analyze", "--url", url, "--sql", sql], { cwd, env, encoding: "utf8" });
  assert.ok(run.status === 0 || run.status === 1, run.stderr);
  const { file: _, ...descriptor } = JSON.parse(run.stdout.trim().split("\n")[0]!);
  return descriptor;
}

/** Each statement's descriptor from the API, checked against the command line's for it. */
async function same(url: string, statements: string[]): Promise<StatementDescriptor[]> {
  const found = await Promise.all(statements.map((sql) => analyze({ url, sql })));
  for (const [i, sql] of statements.entries()) assert.deepEqual(found[i], cli(url, sql), sql);
  return found;
}

function sqliteFile(setup: string): string {
  const file = join(mkdtempSync(join(tmpdir(), "reylo-rtd-")), "app.db");
  const db = new DatabaseSync(file);
  db.exec(setup);
  db.close();
  return file;
}

test("analyze returns a SQLite statement's descriptor in Reylo Type Defs, as reylo analyze prints it", async () => {
  const url = `file:${sqliteFile("create table users (id integer primary key, name text not null, email text)")}`;
  const [user, count, broken, untyped] = await same(url, ["-- One user, by id\nselect id, name, email from users where id = :id", "select count(*) as n from users", "select 'é🙂' as x, nope from users", "select 'é🙂' as x, :v as v, :w as w"]);
  assert.equal(RTD_VERSION, "0.1");
  const { columns, ...rest } = user!;
  assert.deepEqual(rest, {
    rtd: "0.1",
    dialect: { name: "sqlite", version: user!.dialect.version },
    sql: "-- One user, by id\nselect id, name, email from users where id = ?",
    kind: "select",
    comment: "One user, by id",
    params: [{ name: "id", type: { kind: "integer", bits: 64 } }],
    cardinality: "at_most_one",
    diagnostics: [],
  });
  assert.deepEqual(columns.map((c) => [c.name, c.nullable ?? false]), [["id", false], ["name", false], ["email", true]]);
  assert.match(user!.dialect.version, /^3\.\d+\.\d+$/);
  assert.equal(count!.cardinality, "one");
  // A diagnostic's place counts UTF-16 units of the statement as written, past characters of two
  // to four bytes.
  assert.deepEqual(broken!.diagnostics.map((d) => [d.message, "select 'é🙂' as x, nope from users".slice(d.start, d.end)]), [["no such column: nope", "nope"]]);
  assert.deepEqual(broken!.columns, []);
  // A parameter nothing types is an error at its first use, as under the project file's default,
  // `strict_params: true`.
  assert.deepEqual(untyped!.diagnostics.map((d) => [d.message, "select 'é🙂' as x, :v as v, :w as w".slice(d.start, d.end)]), ["v", "w"].map((n) => [`Nothing in the statement gives the parameter :${n} a type; cast it, as in :${n}::text, or set strict_params: false in .reylo.yaml.`, `:${n}`]));
  // Plain data: JSON carries all of it.
  assert.deepEqual(JSON.parse(JSON.stringify(user)), user);
});

test("analyze rejects a URL of no database it reads, and a database it cannot reach", async () => {
  await assert.rejects(analyze({ url: "redis://localhost", sql: "select 1" }), /cannot tell the database from the URL/);
  await assert.rejects(analyze({ url: "postgres://postgres@127.0.0.1:1/none", sql: "select 1" }), /^Error: Cannot reach the database/);
  await assert.rejects(analyze({ sql: "select 1" } as never), TypeError);
});

test("analyze reads Postgres, its server's version, comments and the server's own error codes", { skip: pgLive }, async () => {
  const db = await postgresDatabase("reylo_rtd", "create table users (id int8 primary key, email varchar(255) not null, bio text); comment on column users.bio is 'Shown on the profile page'");
  try {
    const [user, missing] = await same(db.url, ["select id, email, bio from users where id = :id", "select emial from users"]);
    const [{ server_version }] = await db.sql`show server_version`;
    assert.deepEqual(user!.dialect, { name: "postgres", version: server_version.split(" ")[0] });
    assert.equal(user!.sql, "select id, email, bio from users where id = $1");
    assert.deepEqual(user!.columns.map((c) => [c.name, c.type.kind, c.nullable ?? false, c.comment]), [["id", "integer", false, undefined], ["email", "text", false, undefined], ["bio", "text", true, "Shown on the profile page"]]);
    assert.deepEqual(missing!.diagnostics.map((d) => [d.code, "select emial from users".slice(d.start, d.end)]), [["42703", "emial"]]);
  } finally {
    await db.drop();
  }
});

test("analyze reads the catalog as it stands at each call, though calls share a connection", { skip: pgLive }, async () => {
  const db = await postgresDatabase("reylo_rtd_fresh", "create table notes (id int primary key, body text)");
  try {
    const before = await analyze({ url: db.url, sql: "select body from notes" });
    assert.equal(before.columns[0]!.nullable, true);
    await db.sql`alter table notes alter column body set not null`;
    const after = await analyze({ url: db.url, sql: "select body from notes" });
    assert.equal(after.columns[0]!.nullable, undefined);
  } finally {
    await db.drop();
  }
});

test("analyze reads MySQL, its server's version and the server's own error codes", { skip: mysqlLive }, async () => {
  const db = await mysqlDatabase("reylo_rtd", "create table users (id bigint primary key, email varchar(255) not null, bio text comment 'Shown on the profile page')");
  try {
    const [user, missing] = await same(db.url, ["select id, email, bio from users where id = :id", "select emial from users"]);
    const [{ v }] = (await db.rows("select version() as v")) as [{ v: string }];
    assert.deepEqual(user!.dialect, { name: "mysql", version: v });
    assert.equal(user!.sql, "select id, email, bio from users where id = ?");
    assert.equal(user!.cardinality, "at_most_one");
    assert.deepEqual(user!.columns.map((c) => [c.name, c.nullable ?? false, c.comment]), [["id", false, undefined], ["email", false, undefined], ["bio", true, "Shown on the profile page"]]);
    assert.deepEqual(missing!.diagnostics.map((d) => d.code), ["1054"]);
  } finally {
    await db.drop();
  }
});

test("analyze reads DuckDB through the project's own @duckdb/node-api", { skip: existsSync(join(DRIVERS, "@duckdb", "node-api")) ? false : "lab/drivers is not installed (no @duckdb/node-api)" }, async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-rtd-duckdb-"));
  writeFileSync(join(root, "package.json"), "{}");
  mkdirSync(join(root, "node_modules", "@duckdb"), { recursive: true });
  symlinkSync(join(DRIVERS, "@duckdb", "node-api"), join(root, "node_modules", "@duckdb", "node-api"));
  const cwd = process.cwd();
  process.chdir(root);
  try {
    const td = await analyze({ url: "duckdb::memory:", sql: "select 42::integer as answer, :name::varchar as name" });
    assert.equal(td.rtd, "0.1");
    assert.equal(td.dialect.name, "duckdb");
    assert.match(td.dialect.version, /^\d+\.\d+\.\d+/);
    assert.deepEqual(td.columns.map((c) => [c.name, c.type.kind]), [["answer", "integer"], ["name", "text"]]);
    assert.deepEqual(td.params.map((p) => p.name), ["name"]);
  } finally {
    process.chdir(cwd);
  }
});
