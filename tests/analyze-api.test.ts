// The root module's `analyze`: each statement's descriptor in Reylo Type Defs, the same as
// `reylo analyze` prints for it, from the database the project names or the one the caller does.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { build } from "esbuild";
import { analyze, type TypeDefs } from "../src/index.ts";
import { nativeBinary } from "../src/native.ts";
import { DRIVERS, mysqlDatabase, mysqlLive, pgLive, postgresDatabase, ROOT } from "./helpers.ts";

type StatementDescriptor = TypeDefs.StatementDescriptor;

/** What `reylo analyze --url <url> --sql <sql>` prints for one statement, run from a directory of its own. */
function cli(url: string, sql: string): StatementDescriptor {
  const cwd = mkdtempSync(join(tmpdir(), "reylo-analyze-cli-"));
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
  const file = join(mkdtempSync(join(tmpdir(), "reylo-analyze-api-")), "app.db");
  const db = new DatabaseSync(file);
  db.exec(setup);
  db.close();
  return file;
}

test("analyze returns a SQLite statement's descriptor in Reylo Type Defs, as reylo analyze prints it", async () => {
  const url = `file:${sqliteFile("create table users (id integer primary key, name text not null, email text)")}`;
  const [user, count, broken, untyped] = await same(url, ["-- One user, by id\nselect id, name, email from users where id = :id", "select count(*) as n from users", "select 'é🙂' as x, nope from users", "select 'é🙂' as x, :v as v, :w as w"]);
  const { columns, ...rest } = user!;
  assert.deepEqual(rest, {
    $schema: "https://reylo.dev/typedefs/0.1.0.json",
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
  await assert.rejects(analyze({ url: "redis://localhost", sql: "select 1" }), /Cannot tell the database from the URL/);
  await assert.rejects(analyze({ url: "postgres://postgres@127.0.0.1:1/none", sql: "select 1" }), /^Error: Cannot reach the database/);
  await assert.rejects(analyze({ url: "file:app.db" } as never), TypeError);
});

test("analyze reads Postgres, its server's version, comments and the server's own error codes", { skip: pgLive }, async () => {
  const db = await postgresDatabase("reylo_analyze_api", "create table users (id int8 primary key, email varchar(255) not null, bio text); comment on column users.bio is 'Shown on the profile page'");
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
  const db = await postgresDatabase("reylo_analyze_api_fresh", "create table notes (id int primary key, body text)");
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
  const db = await mysqlDatabase("reylo_analyze_api", "create table users (id bigint primary key, email varchar(255) not null, bio text comment 'Shown on the profile page')");
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
  const root = mkdtempSync(join(tmpdir(), "reylo-analyze-api-duckdb-"));
  writeFileSync(join(root, "package.json"), "{}");
  mkdirSync(join(root, "node_modules", "@duckdb"), { recursive: true });
  symlinkSync(join(DRIVERS, "@duckdb", "node-api"), join(root, "node_modules", "@duckdb", "node-api"));
  const cwd = process.cwd();
  process.chdir(root);
  try {
    const td = await analyze({ url: "duckdb::memory:", sql: "select 42::integer as answer, :name::varchar as name" });
    assert.equal(td.$schema, "https://reylo.dev/typedefs/0.1.0.json");
    assert.equal(td.dialect.name, "duckdb");
    assert.match(td.dialect.version, /^\d+\.\d+\.\d+/);
    assert.deepEqual(td.columns.map((c) => [c.name, c.type.kind]), [["answer", "integer"], ["name", "text"]]);
    assert.deepEqual(td.params.map((p) => p.name), ["name"]);
  } finally {
    process.chdir(cwd);
  }
});

/** A package with `files` written into it, and the SQLite databases `databases` names, each holding one table of its name. */
function project(files: Record<string, string>, databases: string[] = []): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-analyze-api-project-"));
  writeFileSync(join(root, "package.json"), "{}");
  for (const [name, text] of Object.entries(files)) writeFileSync(join(root, name), text);
  for (const name of databases) {
    const db = new DatabaseSync(join(root, `${name}.db`));
    db.exec(`create table ${name} (id integer primary key)`);
    db.close();
  }
  return root;
}

/** `work`, run from `dir` with no connection of the developer's own in the environment, as a script in that project runs. */
async function inProject<T>(dir: string, work: () => Promise<T>): Promise<T> {
  const cwd = process.cwd();
  const saved = { DATABASE_URL: process.env.DATABASE_URL, REYLO_CONFIG: process.env.REYLO_CONFIG };
  delete process.env.DATABASE_URL;
  delete process.env.REYLO_CONFIG;
  process.chdir(dir);
  try {
    return await work();
  } finally {
    process.chdir(cwd);
    for (const [name, value] of Object.entries(saved)) if (value !== undefined) process.env[name] = value;
  }
}

/** What `reylo analyze --sql <sql>` prints from `cwd`, with no connection of the developer's own in its environment. */
function cliIn(cwd: string, sql: string, ...args: string[]): StatementDescriptor {
  const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => name !== "DATABASE_URL" && name !== "REYLO_CONFIG"));
  const run = spawnSync(nativeBinary(), ["analyze", "--sql", sql, ...args], { cwd, env, encoding: "utf8" });
  assert.ok(run.status === 0 || run.status === 1, run.stderr);
  return JSON.parse(run.stdout.trim().split("\n")[0]!);
}

test("analyze reads the database the project names when no url is given, as the type check and reylo analyze do", async () => {
  // The project file's direct_url, from the project's root, over DATABASE_URL in its .env.
  const named = project({ ".reylo.yaml": "direct_url: file:./named.db\n", ".env": "DATABASE_URL=file:./other.db\n" }, ["named", "other"]);
  const fromFile = await inProject(named, () => analyze({ sql: "select id from named" }));
  assert.deepEqual(fromFile.diagnostics, []);
  assert.deepEqual(fromFile, cliIn(named, "select id from named"));
  // DATABASE_URL from a .env file, with no project file.
  const dotenv = project({ ".env": "DATABASE_URL=file:./other.db\n" }, ["other"]);
  const fromEnv = await inProject(dotenv, () => analyze({ sql: "select id from other" }));
  assert.deepEqual(fromEnv.diagnostics, []);
  assert.deepEqual(fromEnv, cliIn(dotenv, "select id from other"));
  // A url given wins, and its relative path is read from the working directory.
  const given = await inProject(named, () => analyze({ sql: "select id from other", url: "file:./other.db" }));
  assert.deepEqual(given.diagnostics, []);
  assert.deepEqual(given, cliIn(named, "select id from other", "--url", "file:./other.db"));
});

test("analyze with a url leaves the project's connection unread, so a command in it never runs", async () => {
  const root = project({ ".reylo.yaml": "direct_url: $(exit 7)\n" }, ["other"]);
  await inProject(root, async () => {
    await assert.rejects(analyze({ sql: "select id from other" }), /direct_url/);
    assert.deepEqual((await analyze({ sql: "select id from other", url: "file:./other.db" })).diagnostics, []);
  });
});

test("analyze says where to set a database when none is named, and what is wrong with a project file it cannot read", async () => {
  await inProject(project({}), async () => {
    await assert.rejects(analyze({ sql: "select 1" }), /^Error: No database URL: set DATABASE_URL in the environment, \.env\.local or \.env, or "direct_url" in \.reylo\.yaml\.$/);
  });
  await inProject(project({ ".reylo.yaml": "nope: 1\n" }, ["other"]), async () => {
    await assert.rejects(analyze({ sql: "select 1", url: "file:./other.db" }), /\.reylo\.yaml:1:1/);
  });
});

test("analyze holds an untyped parameter to the project file's strict_params, with a url given too", async () => {
  const strict = project({ ".env": "DATABASE_URL=file:./other.db\n" }, ["other"]);
  const refused = await inProject(strict, () => analyze({ sql: "select :x as x" }));
  assert.deepEqual(refused.diagnostics.map((d) => d.message), ["Nothing in the statement gives the parameter :x a type; cast it, as in :x::text, or set strict_params: false in .reylo.yaml."]);
  assert.deepEqual(refused, cliIn(strict, "select :x as x"));
  const lax = project({ ".reylo.yaml": "strict_params: false\n", ".env": "DATABASE_URL=file:./other.db\n" }, ["other"]);
  for (const input of [{ sql: "select :x as x" }, { sql: "select :x as x", url: "file:./other.db" }]) {
    const found = await inProject(lax, () => analyze(input));
    assert.deepEqual(found.diagnostics, []);
    assert.deepEqual(found.params, [{ name: "x", type: { kind: "unknown" } }]);
  }
  assert.deepEqual(await inProject(lax, () => analyze({ sql: "select :x as x" })), cliIn(lax, "select :x as x"));
});

test("an app's bundle of the root module holds none of the build-time half, and calling analyze where there is no Node says so", async () => {
  const { outputFiles, warnings, metafile } = await build({ entryPoints: [join(ROOT, "src", "index.ts")], bundle: true, platform: "browser", format: "esm", write: false, metafile: true, logLevel: "silent" });
  assert.deepEqual(warnings, []);
  const inputs = Object.keys(metafile.inputs);
  for (const part of ["src/tooling.ts", "src/analyze-api.ts", "src/engine.ts", "src/analyze.ts", "src/duckdb.ts", "src/project.ts"]) assert.ok(!inputs.includes(part), `${part} is in the bundle`);
  assert.ok(inputs.includes("src/build-time.ts"));
  assert.doesNotMatch(outputFiles[0]!.text, /child_process|import\.meta/);
  // A browser, or a Worker without nodejs_compat: no `process` to reach Node through.
  const saved = Object.getOwnPropertyDescriptor(globalThis, "process")!;
  Object.defineProperty(globalThis, "process", { value: undefined, configurable: true });
  try {
    await assert.rejects(analyze({ sql: "select 1" }), /needs Node 24 or later, or Bun/);
  } finally {
    Object.defineProperty(globalThis, "process", saved);
  }
});
