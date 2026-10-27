import { test } from "node:test";
import assert from "node:assert/strict";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, renameSync, rmSync, symlinkSync, utimesSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { Session } from "../src/analyze.ts";
import { sqlModule } from "../src/loader.ts";
import { mapperHandlers } from "../src/mapper-server.ts";
import { mysqlStandIn } from "./mysql-stand-in.ts";
import { refusedLogin, Unreachable } from "../src/connection.ts";
import { defineQuery } from "../src/index.ts";

// A throwaway Postgres server for the tests that need one, never a database anyone uses: each such
// test makes a database of its own there and drops it after. The driver is the labs', never the
// package's own.
const PG = process.env.REYLO_TEST_PG;
const DRIVERS = fileURLToPath(new URL("../lab/drivers/node_modules", import.meta.url));
const live = !PG ? "REYLO_TEST_PG names no server" : !existsSync(join(DRIVERS, "postgres")) ? "lab/drivers is not installed" : false;

// A SQLite project: a package, its database, and DATABASE_URL naming it.
function project(): { root: string; env: Record<string, string> } {
  const root = mkdtempSync(join(tmpdir(), "reylo-analyze-"));
  writeFileSync(join(root, "package.json"), "{}");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null)");
  db.close();
  return { root, env: { DATABASE_URL: "file:./app.db" } };
}

// Each session stands for a process of its own: the type check, a build, reylo generate.
async function analyze(root: string, env: Record<string, string>, sql: string, options: { force?: boolean } = {}) {
  const session = await Session.open(root, env);
  try {
    return await session.analyze(sql, options);
  } finally {
    await session.close();
  }
}

const SELECT = "select id, name from users where id = :id";

test("a statement is analyzed against the database once, and read from .reylo/ by every process after", async () => {
  const { root, env } = project();
  const first = await analyze(root, env, SELECT);
  assert.ok(first.ok);
  assert.equal(first.source, "database");
  assert.equal(first.analysis.cardinality, "maybeOne");
  assert.ok(first.entry.startsWith(join(root, ".reylo", "queries")));
  assert.ok(existsSync(first.entry));
  // The directory keeps itself out of git.
  assert.match(readFileSync(join(root, ".reylo", ".gitignore"), "utf8"), /^\*$/m);
  const second = await analyze(root, env, SELECT);
  assert.ok(second.ok);
  assert.deepEqual([second.source, second.entry, second.analysis], ["cache", first.entry, first.analysis]);
});

test("a change to the schema makes the entry stale, and the statement is analyzed again", async () => {
  const { root, env } = project();
  const before = await analyze(root, env, "select * from users");
  assert.ok(before.ok);
  assert.deepEqual(before.analysis.columns.map((c) => c.name), ["id", "name"]);
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("alter table users add column email text");
  db.close();
  const after = await analyze(root, env, "select * from users");
  assert.ok(after.ok);
  assert.equal(after.source, "database");
  assert.deepEqual(after.analysis.columns.map((c) => c.name), ["id", "name", "email"]);
  assert.equal(after.entry, before.entry);
});

test("without the database, .reylo/ answers as it is, and a statement it lacks is an error that names reylo generate", async () => {
  const { root, env } = project();
  assert.ok((await analyze(root, env, SELECT)).ok);
  renameSync(join(root, "app.db"), join(root, "gone.db"));
  const cached = await analyze(root, env, SELECT);
  assert.ok(cached.ok);
  assert.equal(cached.source, "cache");
  const missing = await analyze(root, env, "select name from users");
  assert.ok(!missing.ok);
  assert.equal(missing.code, 5);
  assert.match(missing.message, /^Cannot open the database .*app\.db .*\.reylo has no analysis of this statement, so run reylo generate where the database can be reached\.$/);
  // --force asks the database even for a statement .reylo/ holds.
  const forced = await analyze(root, env, SELECT, { force: true });
  assert.ok(!forced.ok);
});

test("two databases the project uses in turn keep an entry each, so neither analyzes the other's statements again", async () => {
  const { root, env } = project();
  const db = new DatabaseSync(join(root, "test.db"));
  db.exec("create table users (id integer primary key, name text not null, email text)");
  db.close();
  const test = { DATABASE_URL: "file:./test.db" };
  const sources = [];
  for (const e of [env, test, env, test]) {
    const outcome = await analyze(root, e, "select * from users");
    assert.ok(outcome.ok);
    sources.push(`${outcome.source} ${outcome.analysis.columns.length}`);
  }
  assert.deepEqual(sources, ["database 2", "database 3", "cache 2", "cache 3"]);
  // Where neither can be reached, a URL with no entry of its own reads the one written last, by
  // the time it records, which a copy that resets the files' times (cp -r, fs.cpSync) keeps.
  for (const f of ["app.db", "test.db"]) renameSync(join(root, f), join(root, `${f}.gone`));
  const queries = join(root, ".reylo", "queries");
  const [older, newer] = readdirSync(queries).map((n) => join(queries, n)).sort((a, b) => JSON.parse(readFileSync(a, "utf8")).at - JSON.parse(readFileSync(b, "utf8")).at);
  utimesSync(newer!, new Date(2000, 0, 1), new Date(2000, 0, 1));
  utimesSync(older!, new Date(), new Date());
  const offline = await analyze(root, { DATABASE_URL: "file:./elsewhere.db" }, "select * from users");
  assert.ok(offline.ok);
  assert.deepEqual([offline.source, offline.entry, offline.analysis.columns.length], ["cache", newer, 3]);
});

test("a build that cannot reach the database reads .reylo/ with another URL, with none, and in a copy of the project elsewhere", async () => {
  const { root, env } = project();
  // The project installs its database's engine, which names the dialect where no URL does.
  mkdirSync(join(root, "node_modules", "@reylosql"), { recursive: true });
  symlinkSync(fileURLToPath(new URL("../packages/sqlite", import.meta.url)), join(root, "node_modules", "@reylosql", "sqlite"));
  const typed = await analyze(root, env, SELECT);
  assert.ok(typed.ok);
  assert.equal(typed.source, "database");
  // No URL: the entries are used as they are, and a statement they lack needs a URL.
  const bare = await analyze(root, {}, SELECT);
  assert.ok(bare.ok);
  assert.deepEqual([bare.source, bare.entry], ["cache", typed.entry]);
  const lacking = await analyze(root, {}, "select name from users");
  assert.ok(!lacking.ok);
  assert.deepEqual([lacking.code, lacking.message], [4, 'No database URL: set DATABASE_URL in the environment, .env.local or .env, or "direct_url" in .reylo.yaml.']);
  assert.ok(!(await analyze(root, {}, SELECT, { force: true })).ok);
  // Another URL, to a database the build cannot open, reads the same entry.
  const other = await analyze(root, { DATABASE_URL: "file:./elsewhere.db" }, SELECT);
  assert.ok(other.ok);
  assert.deepEqual([other.source, other.entry], ["cache", typed.entry]);
  // A copy of the project at another path, as a container build has it, without its database.
  const copy = mkdtempSync(join(tmpdir(), "reylo-analyze-copy-"));
  cpSync(root, copy, { recursive: true, verbatimSymlinks: true });
  rmSync(join(copy, "app.db"));
  const moved = await analyze(copy, env, SELECT);
  assert.ok(moved.ok);
  assert.equal(moved.source, "cache");
  assert.ok(moved.entry.startsWith(join(copy, ".reylo")));
});

test("the module a build step emits carries the analysis and reads nothing at run time", async () => {
  const { root, env } = project();
  const file = join(root, "user.sql");
  writeFileSync(file, SELECT);
  // The build steps share one session per project; this one finds the database through .env.
  writeFileSync(join(root, ".env"), `DATABASE_URL=${env.DATABASE_URL}\n`);
  try {
    const { code, entry, watched } = await sqlModule(file, SELECT);
    assert.ok(existsSync(entry));
    // A bundler watches the files the module's text reads besides the statement: here the .env
    // file, the only one of the project's files that exists.
    assert.deepEqual(watched, [join(root, ".env")]);
    assert.equal(code, [
      `import { defineQuery } from "reylo";`,
      `export const sql = "select id, name from users where id = ? /*file='user.sql'*/";`,
      `export const raw = ${JSON.stringify(SELECT)};`,
      `export const params = ["id"];`,
      `export default defineQuery({ dialect: "sqlite", sql: raw, params, cardinality: "maybeOne", text: sql, order: ["id"], inputs: {"id":{"type":{"kind":"scalar","name":"integer"},"nullable":false}}, columns: [["id","int"],["name","text"]], fresh: {"query":"select j.value as fresh_table, p.name as fresh_column, p.type as fresh_type, cast(p.\\"notnull\\" as text) as fresh_not_null, cast(p.dflt_value is not null or (p.pk > 0 and lower(p.type) = 'integer' and (select count(*) from pragma_table_xinfo(j.value) k where k.pk > 0) = 1) as text) as fresh_default from (select 'users' as value) j, pragma_table_xinfo(j.value) p order by 1, 2","rows":[["users","id","INTEGER","0","1"],["users","name","TEXT","1","0"]],"inserts":[]}, source: "user.sql", jsonSchema: {"input":{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"id":{"type":"integer","format":"int64"}},"required":["id"],"additionalProperties":false},"output":{"$schema":"https://json-schema.org/draft/2020-12/schema","anyOf":[{"type":"object","properties":{"id":{"type":"integer","format":"int64"},"name":{"type":"string"}},"required":["id","name"]},{"type":"null"}]}} });`,
      ``,
    ].join("\n"));
    // A statement that relies on no catalog fact carries its file too, written with `/`.
    mkdirSync(join(root, "queries"));
    const one = join(root, "queries", "one.sql");
    writeFileSync(one, "select 1 as n");
    const { code: plain } = await sqlModule(one, "select 1 as n");
    assert.ok(!plain.includes("fresh:"));
    assert.ok(plain.includes(`, source: "queries/one.sql", `));
    // A build (a process of its own) that has neither the database nor the entry fails instead of guessing.
    for (const session of Session.opened()) await session.close();
    renameSync(join(root, "app.db"), join(root, "gone.db"));
    await assert.rejects(sqlModule(file, "select name from users"), /user\.sql: Cannot open the database .*run reylo generate/);
  } finally {
    for (const session of Session.opened()) await session.close();
  }
});

test("one statement of another kind is a command: DDL resolves to nothing and runs, and a PRAGMA returns its rows", async () => {
  const { root, env } = project();
  const ddl = "create index users_name on users (name)";
  const created = await analyze(root, env, ddl);
  assert.ok(created.ok);
  const { kind, cardinality, columns, params, diagnostics } = created.analysis;
  assert.deepEqual({ kind, cardinality, columns, params, diagnostics }, { kind: "create", cardinality: "none", columns: [], params: [], diagnostics: [] });
  const db = new DatabaseSync(join(root, "app.db"));
  try {
    assert.equal(await defineQuery({ dialect: "sqlite", sql: ddl, params: [], cardinality }).run(db), undefined);
    // A command that returns rows has the columns SQLite names, of no known type and nullable.
    const listed = await analyze(root, env, "pragma index_list(users)");
    assert.ok(listed.ok);
    assert.equal(listed.analysis.cardinality, "many");
    assert.deepEqual(listed.analysis.columns.map((c) => [c.name, c.type.kind, c.nullable]), ["seq", "name", "unique", "origin", "partial"].map((name) => [name, "unknown", true]));
    const rows = await defineQuery({ dialect: "sqlite", sql: "pragma index_list(users)", params: [], cardinality: "many" }).run(db);
    assert.deepEqual(rows.map((r) => (r as { name: string }).name), ["users_name"]);
  } finally {
    db.close();
  }
  // A CREATE of a table the database has is SQLite's error, with how to leave a migration out.
  const existing = await analyze(root, env, "create table users (id integer primary key)");
  assert.ok(existing.ok);
  assert.deepEqual(existing.analysis.diagnostics.map((d) => [d.code, d.message]), [[6, 'table users already exists, so if it is a migration or a schema file, leave it out of the type check with "exclude" in tsconfig.json, as in "exclude": ["db/**"].']]);
});

test("a build fails on a statement the database refuses, at its line and column as the type check places it", async () => {
  const { root, env } = project();
  writeFileSync(join(root, ".env"), `DATABASE_URL=${env.DATABASE_URL}\n`);
  try {
    // The offset is in UTF-8 bytes; the column counts characters, past the two-byte é.
    const sql = "select 'é' as e,\n  nmae from users";
    const file = join(root, "bad.sql");
    writeFileSync(file, sql);
    await assert.rejects(sqlModule(file, sql), /bad\.sql:2:3: no such column: nmae$/);
  } finally {
    for (const session of Session.opened()) await session.close();
  }
});

test("a parameter written twice is an error at each later use, which the type check places and a build fails on; a with reads the value twice", async () => {
  const { root, env } = project();
  writeFileSync(join(root, ".env"), `DATABASE_URL=${env.DATABASE_URL}\n`);
  const twice = "select id, name from users\nwhere name like :q or name = :q or id = :q";
  try {
    const session = await Session.for(root);
    const outcome = await session.analyze(twice);
    assert.ok(outcome.ok);
    // Byte offsets, as the content mapper hands them to the type check: the second and third :q.
    const second = twice.indexOf(":q or id");
    assert.deepEqual(outcome.analysis.diagnostics.map((d) => [d.code, d.start, d.length]), [[3, second, 2], [3, twice.lastIndexOf(":q"), 2]]);
    assert.equal(outcome.analysis.diagnostics[0]!.message, "The parameter :q appears more than once; use each parameter once, and select it in a with to read it twice, cast to its type, as in: with input as (select :q::<type> as q) select … from input");
    const file = join(root, "search.sql");
    writeFileSync(file, twice);
    await assert.rejects(sqlModule(file, twice), /search\.sql: The parameter :q appears more than once/);
    // SQLite's own forms and its numbered placeholder are names too; a bare ? never repeats.
    for (const sql of ["select id from users where name = @q or name = @q", "select id from users where id = ?1 or id = ?1", "select id from users where id = ? or id = ?1"]) {
      const refused = await session.analyze(sql);
      assert.ok(refused.ok);
      assert.deepEqual(refused.analysis.diagnostics.map((d) => d.code), [3], sql);
    }
    const bare = await session.analyze("select id from users where id = ? or id = ?");
    assert.ok(bare.ok);
    assert.deepEqual(bare.analysis.diagnostics, []);
    // Cast to nullable(…) in the with, the parameter may be null, and a coalesce of its column reads
    // that null.
    const once = "with input as (select :q::nullable(text) as q) select id, name from users, input where name like coalesce(input.q, '%') or name = input.q";
    const read = await session.analyze(once);
    assert.ok(read.ok);
    assert.deepEqual(read.analysis.diagnostics, []);
    assert.deepEqual(read.analysis.params.map((p) => [p.name, p.nullable]), [["q", true]]);
    writeFileSync(file, once);
    const { code } = await sqlModule(file, once);
    assert.match(code, /export const params = \["q"\];/);
  } finally {
    for (const session of Session.opened()) await session.close();
  }
});

test("a parameter as a whole sort key is an error the type check places, and a build still runs the statement it typed", async () => {
  const { root, env } = project();
  writeFileSync(join(root, ".env"), `DATABASE_URL=${env.DATABASE_URL}\n`);
  const sql = "select id, name from users\norder by :sort";
  try {
    const session = await Session.for(root);
    const outcome = await session.analyze(sql);
    assert.ok(outcome.ok);
    assert.deepEqual(outcome.analysis.diagnostics.map((d) => [d.code, d.start, d.length]), [[17, sql.indexOf(":sort"), 5]]);
    assert.deepEqual(outcome.analysis.columns.map((c) => c.name), ["id", "name"]);
    // As an ON CONFLICT target no key matches, the error is the type check's: the module is built.
    const file = join(root, "sorted.sql");
    writeFileSync(file, sql);
    const { code } = await sqlModule(file, sql);
    assert.match(code, /export const params = \["sort"\];/);
  } finally {
    for (const session of Session.opened()) await session.close();
  }
});

test("the Postgres fingerprint runs on the server, holds while nothing changes, and moves with each catalog the analysis reads, under a search path that puts a user's `||` first", { skip: live }, async () => {
  const postgres = createRequire(join(DRIVERS, "postgres", "package.json"))("postgres");
  const admin = postgres(PG, { max: 1, onnotice: () => {} });
  const name = `reylo_fingerprint_${process.pid}`;
  await admin.unsafe(`drop database if exists ${name}`);
  await admin.unsafe(`create database ${name}`);
  const url = new URL(PG!);
  url.pathname = `/${name}`;
  const db = postgres(url.href, { max: 1, onnotice: () => {} });
  const root = mkdtempSync(join(tmpdir(), "reylo-fingerprint-"));
  writeFileSync(join(root, "package.json"), "{}");
  mkdirSync(join(root, "node_modules"));
  symlinkSync(join(DRIVERS, "postgres"), join(root, "node_modules", "postgres"));
  // A session per call, as a process of its own, so that none answers from a fingerprint it took.
  const fingerprint = async () => {
    const session = await Session.open(root, { DATABASE_URL: url.href });
    try {
      return (await session.schema()).value;
    } finally {
      await session.close();
    }
  };
  try {
    await db.unsafe(`create table a (x int, s text); create table b (x int, s text);
      create function skip() returns trigger language plpgsql as 'begin return null; end';
      create function to_one(boolean) returns int8 language sql as 'select 1::int8';
      create function plus(int, int) returns int language sql as 'select $1 + $2'`);
    // A query the server refuses gives "unverified <pid> <time>", a value no entry ever matches.
    const first = await fingerprint();
    assert.match(first ?? "", /^[0-9a-f]{16}$/);
    assert.equal(await fingerprint(), first);
    // A user's `||` that answers NULL, which a search path naming pg_catalog after public puts
    // before the built-in: the query builds its text with pg_catalog's own, so every row still
    // reads, and the fingerprint moves with each change below.
    await db.unsafe(`create function cat_null(text, text) returns text language sql immutable as 'select null::pg_catalog.text';
      create operator public.|| (leftarg = text, rightarg = text, function = cat_null)`);
    await admin.unsafe(`alter database ${name} set search_path = public, pg_catalog`);
    const hostile = await fingerprint();
    assert.notEqual(hostile, first);
    assert.equal(await fingerprint(), hostile);
    // Each change touches only a catalog the row count or the trust check reads: rows of b are read
    // by a scan of a, a BEFORE ROW trigger can skip an INSERT's row and a rule can replace it, a
    // key holds only under the index's collation, and a user's operator, cast, default argument,
    // volatility or strictness decides which call the server makes and what it may answer.
    let last = hostile;
    for (const change of [
      "alter table b inherit a",
      "create trigger skip before insert on a for each row execute function skip()",
      "alter table a disable trigger skip",
      "create rule nothing as on insert to b do instead nothing",
      `alter table a alter column s type text collate "C"`,
      "create operator === (leftarg = int, rightarg = int, function = int4eq)",
      "create cast (boolean as int8) with function to_one(boolean)",
      "drop cast (boolean as int8); create cast (boolean as int8) with function to_one(boolean) as implicit",
      "create or replace function plus(int, int default 1) returns int language sql as 'select $1 + $2'",
      "alter function plus(int, int) immutable",
      "alter function plus(int, int) strict",
    ]) {
      await db.unsafe(change);
      const next = await fingerprint();
      assert.notEqual(next, last, change);
      assert.equal(await fingerprint(), next, change);
      last = next;
    }
  } finally {
    await db.end();
    await admin.unsafe(`drop database if exists ${name}`);
    await admin.end();
  }
});

// A project on MySQL whose server is a stand-in (tests/mysql-stand-in.ts), named in .env so that a
// build step, which reads the process's environment, finds it too.
function standInProject(port: number): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-not-mysql-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=mysql://root@127.0.0.1:${port}/app\n`);
  return root;
}

test("a MariaDB server is refused by the type check, once per connection, with an error that says to type-check against MySQL", async () => {
  const server = await mysqlStandIn({ version: "11.8.2-MariaDB-ubu2404" });
  const root = standInProject(server.port);
  const session = await Session.open(root, {});
  try {
    for (const sql of ["select 1 as one", "select 2 as two"]) {
      const outcome = await session.analyze(sql);
      assert.ok(!outcome.ok);
      assert.equal(outcome.code, 5);
      assert.equal(
        outcome.message,
        `Cannot type-check against the database at 127.0.0.1:${server.port}/app (DATABASE_URL in .env): it is MariaDB 11.8.2, and Reylo type-checks MySQL statements against MySQL. Point the type check's URL at a MySQL server, 8.4 or later, with the same schema.`,
      );
    }
    // One connection asked once, then closed; nothing else ran on it.
    assert.deepEqual(server.queries, ["select version()"]);
    assert.ok(server.ended());
    assert.ok((await session.schema()).fatal);
  } finally {
    await session.close();
  }
  // A build step, which reads the process's environment, fails at the file with the same error.
  const saved = process.env.DATABASE_URL;
  delete process.env.DATABASE_URL;
  try {
    await assert.rejects(sqlModule(join(root, "q.sql"), "select 1"), /q\.sql: Cannot type-check against .* it is MariaDB 11\.8\.2/);
  } finally {
    if (saved !== undefined) process.env.DATABASE_URL = saved;
    for (const session of Session.opened()) await session.close();
    await server.close();
  }
});

test("a MySQL server is not refused: the type check goes on to read its schema", async () => {
  const server = await mysqlStandIn({ version: "8.4.6" });
  const session = await Session.open(standInProject(server.port), {});
  try {
    const schema = await session.schema();
    assert.ok(!schema.fatal);
    assert.match(schema.value ?? "", /^[0-9a-f]{16}$/);
    assert.equal(server.queries[0], "select version()");
    assert.equal(server.queries.length, 2);
  } finally {
    await session.close();
    await server.close();
  }
});

// A MySQL stand-in that takes one password, and refuses any other at the login as MySQL does (error 1045).
const TOKENS = ["token-1", "token-2", "token-3"];

test("a login the server refuses runs direct_url's command again, once, so that an expired token is replaced", async () => {
  let current = "token-1";
  const server = await mysqlStandIn({ version: "8.4.6", accepts: () => current, passwords: TOKENS });
  const root = mkdtempSync(join(tmpdir(), "reylo-token-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, "token.txt"), "token-1");
  writeFileSync(join(root, ".reylo.yaml"), `direct_url: mysql://root:$(cat token.txt)@127.0.0.1:${server.port}/app\n`);
  const session = await Session.open(root, {});
  try {
    // The token the session read at its start has expired, and the command now prints the next.
    current = "token-2";
    writeFileSync(join(root, "token.txt"), "token-2");
    const schema = await session.schema();
    assert.equal(schema.error, undefined);
    assert.match(schema.value ?? "", /^[0-9a-f]{16}$/);
    assert.deepEqual(server.tried, ["token-1", "token-2"]);
    // A command whose fresh output is refused too is asked once, and the refusal is the error.
    current = "token-3";
    await session.release();
    assert.ok(await session.recheck());
    const refused = await session.schema();
    assert.equal(refused.value, null);
    assert.match(refused.error ?? "", /Access denied for user 'root'/);
    assert.deepEqual(server.tried, ["token-1", "token-2", "token-2"]);
  } finally {
    await session.close();
    await server.close();
  }
});

// The same on Postgres against the live server, where a role that does not exist is a refused login
// (SQLSTATE 28000) as an expired password is.
test("on Postgres, a login the server refuses runs direct_url's command again, once", { skip: live }, async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-pg-token-"));
  writeFileSync(join(root, "package.json"), "{}");
  const server = new URL(PG!);
  writeFileSync(join(root, "role.txt"), "reylo_no_such_role");
  writeFileSync(join(root, ".reylo.yaml"), `direct_url: postgres://$(cat role.txt)@${server.host}${server.pathname}\n`);
  const session = await Session.open(root, {});
  try {
    // The role the session read at its start is refused, and the command now prints one that logs in.
    writeFileSync(join(root, "role.txt"), server.username || "postgres");
    const schema = await session.schema();
    assert.equal(schema.error, undefined);
    assert.match(schema.value ?? "", /^[0-9a-f]{16}$/);
    // The connection the fresh URL opened is the one the analysis runs on.
    const outcome = await session.analyze("select 1 as one", { force: true });
    assert.deepEqual(outcome.ok && outcome.analysis.columns.map((c) => c.name), ["one"]);
  } finally {
    await session.close();
  }
});

test("calls refused together when a token expires share one run of the command and one new connection", async () => {
  let current = "token-1";
  const server = await mysqlStandIn({ version: "8.4.6", accepts: () => current, passwords: TOKENS });
  const root = mkdtempSync(join(tmpdir(), "reylo-token-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, "token.txt"), "token-1");
  writeFileSync(join(root, ".reylo.yaml"), `direct_url: mysql://root:$(echo run >> runs.txt; cat token.txt)@127.0.0.1:${server.port}/app\n`);
  const runs = () => readFileSync(join(root, "runs.txt"), "utf8").split("\n").filter(Boolean).length;
  const session = await Session.open(root, {});
  try {
    assert.match((await session.schema()).value ?? "", /^[0-9a-f]{16}$/);
    const before = runs();
    // The token expires while the session's connection is closed, and two reads of the schema then
    // ask for one at once: the refused login runs the command once, for one new connection.
    current = "token-2";
    writeFileSync(join(root, "token.txt"), "token-2");
    await session.release();
    const [changed, schema] = await Promise.all([session.recheck(), session.schema()]);
    assert.equal(changed, false);
    assert.match(schema.value ?? "", /^[0-9a-f]{16}$/);
    assert.equal(runs() - before, 1);
    assert.deepEqual(server.tried, ["token-1", "token-1", "token-2"]);
    assert.equal(server.open(), 1);
  } finally {
    await session.close();
  }
  // The engine's goodbye reaches the server before the session's close returns.
  await new Promise((resolve) => setTimeout(resolve, 50));
  assert.equal(server.open(), 0);
  await server.close();
});

test("a session kept open runs a failed direct_url command again once its failure has stood a minute", async (t) => {
  t.mock.timers.enable({ apis: ["Date"], now: Date.now() });
  const server = await mysqlStandIn({ version: "8.4.6", accepts: () => "token-1", passwords: TOKENS });
  const root = mkdtempSync(join(tmpdir(), "reylo-token-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".reylo.yaml"), `direct_url: mysql://root:$(echo run >> runs.txt; cat token.txt)@127.0.0.1:${server.port}/app\n`);
  const runs = () => readFileSync(join(root, "runs.txt"), "utf8").split("\n").filter(Boolean).length;
  // The password manager is locked when the session opens, as the type check's may be at the editor's start.
  const session = await Session.open(root, {});
  try {
    const failed = await session.analyze("select 1");
    assert.equal(failed.ok, false);
    assert.equal(!failed.ok && failed.code, 11);
    assert.match(!failed.ok ? failed.message : "", /runs `echo run >> runs.txt; cat token.txt`, which exited with code 1/);
    // Unlocked, the command is not run again within the minute, so that a prompt is not asked again per file.
    writeFileSync(join(root, "token.txt"), "token-1");
    const soon = await session.analyze("select 1");
    assert.equal(!soon.ok && soon.code, 11);
    assert.equal(runs(), 1);
    // After it, the next analysis runs the command, and the session logs in with what it prints.
    t.mock.timers.tick(60_000);
    const later = await session.analyze("select 1");
    // The stand-in prepares nothing, so the analysis itself still fails, past the project.
    assert.equal(!later.ok && later.code, 5);
    assert.equal(session.project.error, undefined);
    assert.equal(runs(), 2);
    // The connection the stand-in drops at the prepare is opened once more for the same analysis.
    assert.deepEqual(server.tried, ["token-1", "token-1"]);
  } finally {
    await session.close();
    await server.close();
  }
});

test("the type check names the dialect of the project a kept session recovers to", async (t) => {
  t.mock.timers.enable({ apis: ["Date"], now: Date.now() });
  const server = await mysqlStandIn({ version: "8.4.6", accepts: () => "token-1", passwords: TOKENS });
  const root = mkdtempSync(join(tmpdir(), "reylo-token-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".reylo.yaml"), `direct_url: mysql://root:$(cat token.txt)@127.0.0.1:${server.port}/app\n`);
  const mapper = mapperHandlers();
  const projectHandle = `recovers-${root}`;
  await mapper.openProject!({ projectHandle, configFileName: join(root, ".reylo.yaml") });
  const transform = async () => (await mapper.transform!({ fileName: join(root, "q.sql"), content: "select 1", projectHandle })) as { text: string; diagnostics: { code: number }[] };
  try {
    // With no URL, and no engine installed in the project, the dialect is Postgres's.
    const before = await transform();
    assert.deepEqual(before.diagnostics.map((d) => d.code), [11]);
    assert.match(before.text, /QueryError<"postgres">/);
    writeFileSync(join(root, "token.txt"), "token-1");
    t.mock.timers.tick(60_000);
    // The stand-in prepares nothing, so the analysis fails past the project, in MySQL's terms.
    const after = await transform();
    assert.deepEqual(after.diagnostics.map((d) => d.code), [5]);
    assert.match(after.text, /QueryError<"mysql">/);
    assert.match(after.text, /"many", "mysql", QueryError>/);
  } finally {
    await mapper.closeProject!({ projectHandle });
    await server.close();
  }
});

test("a refused login is told apart from a server that cannot be reached, on Postgres and MySQL", () => {
  const pg = (code: string) => new Unreachable(Object.assign(new Error("password authentication failed"), { code, severity: "FATAL" }));
  assert.ok(refusedLogin(pg("28P01")));
  assert.ok(refusedLogin(pg("28000")));
  assert.ok(refusedLogin(new Unreachable(Object.assign(new Error("Access denied"), { code: "ER_ACCESS_DENIED_ERROR", errno: 1045 }))));
  assert.ok(!refusedLogin(pg("3D000")));
  assert.ok(!refusedLogin(new Unreachable(Object.assign(new Error("connect ECONNREFUSED"), { code: "ECONNREFUSED" }))));
});
