// `reylo analyze`, the native binary's own command line: each statement's type descriptor
// (spec/index.ts) as one line of JSON, from the database the project names as the type
// check finds it.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { Ajv2020 } from "ajv/dist/2020.js";
import type { StatementDescriptor } from "../spec/index.ts";
import { nativeBinary } from "../src/native.ts";
import { mysqlDatabase, mysqlLive, pgLive, postgresDatabase } from "./helpers.ts";

type Line = StatementDescriptor & { file?: string };

// The format's JSON Schema (spec/typedefs.schema.json), whose `$id` is the URL every line printed
// names in `$schema`, and which every line printed is held to.
const schema = JSON.parse(readFileSync(new URL("../spec/typedefs.schema.json", import.meta.url), "utf8"));
const SCHEMA: string = schema.$id;
const ajv = new Ajv2020({ strict: false, allErrors: true });
const valid = ajv.compile(schema);

/** `reylo analyze` run in `cwd`, with no connection of the developer's own in its environment; each line it prints names the format's version and validates against its schema. */
function analyzeIn(cwd: string, args: string[], env: Record<string, string> = {}, input?: string) {
  const inherited = Object.fromEntries(Object.entries(process.env).filter(([name]) => name !== "DATABASE_URL" && name !== "REYLO_CONFIG"));
  const run = spawnSync(nativeBinary(), ["analyze", ...args], { cwd, env: { ...inherited, ...env }, input, encoding: "utf8" });
  const lines = run.stdout.split("\n").filter(Boolean).map((l) => JSON.parse(l) as Line);
  for (const line of lines) {
    assert.equal(line.$schema, SCHEMA);
    assert.ok(valid(line), `${JSON.stringify(line)}: ${ajv.errorsText(valid.errors)}`);
  }
  return { status: run.status, lines, stderr: run.stderr };
}

/** A package whose `.env` names a SQLite database of one table, with `files` written into it. */
function project(files: Record<string, string>): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-analyze-cli-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), "DATABASE_URL=file:./app.db\n");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null, email text)");
  db.close();
  for (const [name, text] of Object.entries(files)) {
    mkdirSync(join(root, name, ".."), { recursive: true });
    writeFileSync(join(root, name), text);
  }
  return root;
}

test("analyze prints each .sql file's descriptor under a directory, and exits 1 when one has a diagnostic", () => {
  const root = project({
    "q/get_user.sql": "-- One user, by id\nselect id, name from users where id = :id\n",
    "q/broken.sql": "select nope from users\n",
    "q/node_modules/dep/skip.sql": "select 1\n",
    "q/.hidden/skip.sql": "select 1\n",
  });
  const { status, lines, stderr } = analyzeIn(root, ["q"]);
  assert.equal(status, 1, stderr);
  assert.deepEqual(lines.map((l) => l.file), ["q/broken.sql", "q/get_user.sql"]);
  const [broken, user] = lines as [Line, Line];
  assert.equal(user.dialect.name, "sqlite");
  assert.match(user.dialect.version, /^3\.\d+\.\d+$/);
  assert.deepEqual({ kind: user.kind, comment: user.comment, params: user.params, cardinality: user.result.cardinality, diagnostics: user.diagnostics }, { kind: "select", comment: "One user, by id", params: [{ name: "id", type: { kind: "integer", bits: 64 } }], cardinality: "at_most_one", diagnostics: [] });
  assert.deepEqual(user.result.columns.map((c) => c.name), ["id", "name"]);
  // A diagnostic's place is in UTF-16 units of the statement as written.
  assert.deepEqual(broken.diagnostics.map((d) => [d.message, "select nope from users\n".slice(d.start, d.end)]), [["no such column: nope", "nope"]]);
});

test("analyze reads a statement from --sql or standard input, and a file from a subdirectory finds the package's project", () => {
  const root = project({ "q/get_user.sql": "select name from users where id = :id\n" });
  const sql = analyzeIn(root, ["--sql", "select count(*) as n from users"]);
  assert.equal(sql.status, 0, sql.stderr);
  assert.equal(sql.lines[0]!.file, undefined);
  assert.equal(sql.lines[0]!.result.cardinality, "one");
  const stdin = analyzeIn(root, ["-"], {}, "select email from users");
  assert.deepEqual(stdin.lines.map((l) => [l.result.columns[0]!.name, l.result.columns[0]!.nullable]), [["email", true]]);
  const nested = analyzeIn(join(root, "q"), ["get_user.sql"]);
  assert.equal(nested.status, 0, nested.stderr);
  assert.deepEqual(nested.lines.map((l) => [l.file, l.result.cardinality]), [["get_user.sql", "at_most_one"]]);
});

test("analyze holds a parameter nothing types to the project's allow_untyped_params, as the type check does", () => {
  const root = project({});
  const strict = analyzeIn(root, ["--sql", "select :x as x"]);
  assert.equal(strict.status, 1);
  assert.deepEqual(strict.lines[0]!.diagnostics.map((d) => [d.message, d.start, d.end]), [["Nothing in the statement gives the parameter :x a type; cast it, as in :x::text, or set allow_untyped_params: true in .reylo.yaml.", 7, 9]]);
  writeFileSync(join(root, ".reylo.yaml"), "allow_untyped_params: true\n");
  const lax = analyzeIn(root, ["--sql", "select :x as x"]);
  assert.equal(lax.status, 0, lax.stderr);
  assert.deepEqual(lax.lines[0]!.params, [{ name: "x", type: { kind: "unknown" } }]);
});

test("analyze takes the project's direct_url and --url, and says what it cannot analyze", { skip: process.platform === "win32" && "its command is written for a POSIX shell" }, () => {
  const root = project({});
  new DatabaseSync(join(root, "other.db")).exec("create table things (label text not null)");
  writeFileSync(join(root, ".reylo.yaml"), 'direct_url: "$(echo file:./other.db)"\n');
  const own = analyzeIn(root, ["--sql", "select label from things"]);
  assert.equal(own.status, 0, own.stderr);
  assert.deepEqual(own.lines[0]!.result.columns.map((c) => c.name), ["label"]);
  const given = analyzeIn(root, ["--sql", "select name from users", "--url", "file:./app.db"]);
  assert.equal(given.status, 0, given.stderr);
  const duckdb = analyzeIn(root, ["--sql", "select 1", "--url", "duckdb:./dev.duckdb"]);
  assert.equal(duckdb.status, 2);
  assert.match(duckdb.stderr, /^reylo analyze: A DuckDB database runs inside the app/);
  const bare = mkdtempSync(join(tmpdir(), "reylo-analyze-cli-"));
  writeFileSync(join(bare, "package.json"), "{}");
  const none = analyzeIn(bare, ["--sql", "select 1"]);
  assert.equal(none.status, 2);
  assert.match(none.stderr, /No database URL: set DATABASE_URL in the environment, \.env\.local or \.env, or "direct_url" in \.reylo\.yaml\./);
  assert.match(analyzeIn(root, []).stderr, /name a \.sql file, a directory, - for standard input, or --sql/);
});

test("analyze prints a statement the engine cannot analyze as its line with one diagnostic, and goes on to the next file", () => {
  const broken = "select value from tally\n";
  const root = project({ "q/a.sql": "select id from users\n", "q/b.sql": broken, "q/c.sql": "select name from users\n" });
  // A view over a table-valued function the database's own SQLite lacks: the engine reads the
  // view, and the catalog query that reads its columns fails on the database.
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create view tally as select value from carray(1, 2, 'int32')");
  db.close();
  const { status, lines, stderr } = analyzeIn(root, ["q"]);
  assert.equal(status, 1, stderr);
  assert.deepEqual(lines.map((l) => [l.file, l.diagnostics.length]), [["q/a.sql", 0], ["q/b.sql", 1], ["q/c.sql", 0]]);
  const line = lines[1]!;
  assert.deepEqual({ sql: line.sql, kind: line.kind, params: line.params, columns: line.result.columns, cardinality: line.result.cardinality }, { sql: broken, kind: undefined, params: [], columns: [], cardinality: "many" });
  const [diagnostic] = line.diagnostics as [Line["diagnostics"][number]];
  assert.match(diagnostic.message, /carray/);
  assert.deepEqual([diagnostic.start, diagnostic.end, diagnostic.code], [0, broken.length, undefined]);
});

test("analyze reads a relative SQLite path in --url from the working directory, and the project's from its root", () => {
  const root = project({});
  mkdirSync(join(root, "sub"));
  new DatabaseSync(join(root, "sub", "x.db")).exec("create table things (label text not null)");
  const given = analyzeIn(join(root, "sub"), ["--sql", "select label from things", "--url", "file:./x.db"]);
  assert.equal(given.status, 0, given.stderr);
  assert.deepEqual(given.lines[0]!.result.columns.map((c) => c.name), ["label"]);
  // The project's DATABASE_URL, file:./app.db, is the root's app.db from the subdirectory too.
  const own = analyzeIn(join(root, "sub"), ["--sql", "select name from users"]);
  assert.equal(own.status, 0, own.stderr);
  assert.deepEqual(own.lines[0]!.result.columns.map((c) => c.name), ["name"]);
});

test("analyze reads a Postgres statement over its own connection, the server's version beside it", { skip: pgLive }, async () => {
  const own = await postgresDatabase("reylo_analyze_cli", "create table users (id int4 primary key, name text not null)");
  try {
    const root = project({});
    const { status, lines, stderr } = analyzeIn(root, ["--url", own.url, "--sql", "select id, name from users where id = :id"]);
    assert.equal(status, 0, stderr);
    const [line] = lines as [Line];
    assert.equal(line.dialect.name, "postgres");
    assert.match(line.dialect.version, /^\d+(\.\d+)?$/);
    assert.equal(line.sql, "select id, name from users where id = $1");
    assert.deepEqual(line.params, [{ name: "id", type: { kind: "integer", bits: 32, name: "int4" } }]);
    assert.deepEqual(line.result.columns.map((c) => [c.name, c.type.kind, c.nullable ?? false]), [["id", "integer", false], ["name", "text", false]]);
    assert.equal(line.result.cardinality, "at_most_one");
  } finally {
    await own.drop();
  }
});

test("analyze reads a MySQL statement over its own connection, the server's version beside it", { skip: mysqlLive }, async () => {
  const own = await mysqlDatabase("reylo_analyze_cli", "create table users (id int primary key, name varchar(40) not null)");
  try {
    const root = project({});
    const { status, lines, stderr } = analyzeIn(root, ["--url", own.url, "--sql", "select id, name from users where id = :id"]);
    assert.equal(status, 0, stderr);
    const [line] = lines as [Line];
    assert.equal(line.dialect.name, "mysql");
    assert.match(line.dialect.version, /^\d+\.\d+\.\d+/);
    assert.equal(line.sql, "select id, name from users where id = ?");
    assert.deepEqual(line.result.columns.map((c) => [c.name, c.type.kind, c.nullable ?? false]), [["id", "integer", false], ["name", "text", false]]);
    assert.equal(line.result.cardinality, "at_most_one");
  } finally {
    await own.drop();
  }
});
