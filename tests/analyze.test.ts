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
import { notMysql } from "../src/mysql-describe.ts";

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
    const { code, entry } = await sqlModule(file, SELECT);
    assert.ok(existsSync(entry));
    assert.equal(code, [
      `import { defineQuery } from "reylo";`,
      `export const sql = "select id, name from users where id = ?";`,
      `export const raw = ${JSON.stringify(SELECT)};`,
      `export const params = ["id"];`,
      `export default defineQuery({ dialect: "sqlite", sql: raw, params, cardinality: "maybeOne", text: sql, order: ["id"], inputs: {"id":{"type":{"kind":"scalar","name":"integer"}}}, columns: [["id","int"],["name","text"]], fresh: {"query":"select j.value as fresh_table, p.name as fresh_column, p.type as fresh_type, cast(p.\\"notnull\\" as text) as fresh_not_null, cast(p.dflt_value is not null or (p.pk > 0 and lower(p.type) = 'integer' and (select count(*) from pragma_table_xinfo(j.value) k where k.pk > 0) = 1) as text) as fresh_default from (select 'users' as value) j, pragma_table_xinfo(j.value) p order by 1, 2","rows":[["users","id","INTEGER","0","1"],["users","name","TEXT","1","0"]],"inserts":[]}, source: "user.sql", jsonSchema: {"input":{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"id":{"type":"integer","format":"int64"}},"required":["id"],"additionalProperties":false},"output":{"$schema":"https://json-schema.org/draft/2020-12/schema","anyOf":[{"type":"object","properties":{"id":{"type":"integer","format":"int64"},"name":{"type":"string"}},"required":["id","name"]},{"type":"null"}]}} });`,
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
    // Coalesced through the with, the parameter may be null; a test of it for null alone would not
    // make it so.
    const once = "with input as (select cast(:q as text) as q) select id, name from users, input where name like coalesce(input.q, '%') or name = input.q";
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

test("the Postgres fingerprint runs on the server, holds while nothing changes, and moves with each catalog the analysis reads", { skip: live }, async () => {
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
      create function skip() returns trigger language plpgsql as 'begin return null; end'`);
    // A query the server refuses gives "unverified <pid> <time>", a value no entry ever matches.
    const first = await fingerprint();
    assert.match(first ?? "", /^[0-9a-f]{16}$/);
    assert.equal(await fingerprint(), first);
    // Each change touches only a catalog the row count or the trust check reads: rows of b are read
    // by a scan of a, a BEFORE ROW trigger can skip an INSERT's row and a rule can replace it, a
    // key holds only under the index's collation, and a user's operator is not the built-in one.
    let last = first;
    for (const change of [
      "alter table b inherit a",
      "create trigger skip before insert on a for each row execute function skip()",
      "alter table a disable trigger skip",
      "create rule nothing as on insert to b do instead nothing",
      `alter table a alter column s type text collate "C"`,
      "create operator === (leftarg = int, rightarg = int, function = int4eq)",
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

// A project whose mysql2 is a stand-in: its server answers version() with what the test sets, and
// every other query with one row.
function fakeMysql(): { root: string; serve(version: string): { queries: string[]; ended: () => boolean } } {
  const root = mkdtempSync(join(tmpdir(), "reylo-not-mysql-"));
  writeFileSync(join(root, "package.json"), "{}");
  // In .env, so that a build step, which reads the process's environment, finds it too.
  writeFileSync(join(root, ".env"), "DATABASE_URL=mysql://root@db.internal:3307/app\n");
  mkdirSync(join(root, "node_modules", "mysql2"), { recursive: true });
  writeFileSync(join(root, "node_modules", "mysql2", "package.json"), JSON.stringify({ name: "mysql2", version: "3.24.4" }));
  writeFileSync(
    join(root, "node_modules", "mysql2", "promise.js"),
    `exports.createConnection = async () => {
      const server = globalThis.reyloFakeMysql;
      return {
        on() {},
        async query(q) {
          const sql = typeof q === "string" ? q : q.sql;
          server.queries.push(sql);
          return [sql === "select version()" ? [[server.version]] : [["x"]], []];
        },
        async prepare() { throw Object.assign(new Error("the stand-in prepares nothing"), { fatal: true }); },
        async end() { server.ended = true; },
      };
    };`,
  );
  return {
    root,
    serve(version) {
      const server = { version, queries: [] as string[], ended: false };
      (globalThis as any).reyloFakeMysql = server;
      return { queries: server.queries, ended: () => server.ended };
    },
  };
}

test("a MariaDB server is refused by the type check, once per connection, with an error that says to type-check against MySQL", async () => {
  const { root, serve } = fakeMysql();
  const server = serve("11.8.2-MariaDB-ubu2404");
  const session = await Session.open(root, {});
  try {
    for (const sql of ["select 1 as one", "select 2 as two"]) {
      const outcome = await session.analyze(sql);
      assert.ok(!outcome.ok);
      assert.equal(outcome.code, 5);
      assert.equal(
        outcome.message,
        `Cannot type-check against the database at db.internal:3307/app (DATABASE_URL in .env): it is MariaDB 11.8.2, and Reylo type-checks MySQL statements against MySQL. Point the type check's URL at a MySQL server, 8.4 or later, with the same schema.`,
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
  }
});

test("a MySQL server is not refused: the type check goes on to read its schema", async () => {
  const { root, serve } = fakeMysql();
  const server = serve("8.4.6");
  const session = await Session.open(root, {});
  try {
    const schema = await session.schema();
    assert.ok(!schema.fatal);
    assert.match(schema.value ?? "", /^[0-9a-f]{16}$/);
    assert.equal(server.queries[0], "select version()");
    assert.equal(server.queries.length, 2);
  } finally {
    await session.close();
  }
});

test("the server's version names MariaDB in either form it is given, and no MySQL", () => {
  assert.equal(notMysql("11.8.2-MariaDB-ubu2404"), "MariaDB 11.8.2");
  assert.equal(notMysql("5.5.5-10.11.6-MariaDB-0+deb12u1"), "MariaDB 10.11.6");
  assert.equal(notMysql("10.6.21-MariaDB"), "MariaDB 10.6.21");
  for (const mysql of ["8.4.6", "9.7.0", "8.0.36-PlanetScale", "8.0.40-0ubuntu0.24.04.1", undefined, null]) assert.equal(notMysql(mysql), undefined);
});
