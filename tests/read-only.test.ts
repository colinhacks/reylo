import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { DatabaseSync } from "node:sqlite";
import { sqlModule } from "../src/loader.ts";
import { pgLive, mysqlLive, INDEX, postgresDatabase, mysqlDatabase, closeSessions } from "./helpers.ts";

// A query's `.readOnly`: true only where the engine is certain the statement writes nothing. Each
// statement is built into a module, as a build step builds it.

async function readOnly(url: string, sql: string): Promise<boolean> {
  const root = mkdtempSync(join(tmpdir(), "reylo-read-only-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  const file = join(root, "q.sql");
  writeFileSync(file, sql);
  const { code } = await sqlModule(file, sql);
  const out = join(root, "q.mjs");
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  return (await import(pathToFileURL(out).href)).default.readOnly;
}

test("on Postgres, only a SELECT that sets off nothing volatile is read-only", { skip: pgLive }, async () => {
  const db = await postgresDatabase("read_only", `create table t (id int primary key, name text);
    create sequence s;
    create function reader() returns int stable language sql as 'select 1';
    create function writer() returns int language plpgsql as $$ begin insert into t values (1, 'x'); return 1; end $$;
    create function same(text, text) returns boolean language sql immutable as 'select $1 = $2';
    create operator === (leftarg = text, rightarg = text, function = same);
    create function same_now(text, text) returns boolean language sql volatile as 'select $1 = $2';
    create operator ==== (leftarg = text, rightarg = text, function = same_now);
    create view plain_view as select id from t;
    create view writing_view as select id, writer() as w from t;
    create table guarded (id int primary key);
    alter table guarded enable row level security;
    create policy guard on guarded using (writer() = 1);`);
  try {
    const cases: [string, boolean][] = [
      ["select id from t", true],
      ["select 1", true],
      ["select id, now(), lower(name) from t where id = 1", true],
      ["select random(), gen_random_uuid()", true],
      ["with a as (select id from t) select id from a", true],
      ["select id from t where name === 'x'", true],
      ["select id from t where name ==== 'x'", false],
      ["select id from plain_view", true],
      ["select id from writing_view", false],
      ["select id from guarded", false],
      ["insert into t values (1, 'x') returning id", false],
      ["update t set name = 'y'", false],
      ["delete from t", false],
      ["with a as (delete from t returning id) select id from a", false],
      ["select id from t for update", false],
      ["select nextval('s')", false],
      ["select set_config('a.b', 'c', false)", false],
      ["select pg_advisory_lock(1)", false],
      ["select writer()", false],
      ["select reader()", true],
      ["select * into t2 from t", false],
    ];
    for (const [sql, expected] of cases) assert.equal(await readOnly(db.url, sql), expected, sql);
  } finally {
    await db.drop();
    await closeSessions();
  }
});

test("on MySQL, only a SELECT with no lock, no INTO, no variable and no stored function is read-only", { skip: mysqlLive }, async () => {
  const db = await mysqlDatabase("read_only", "create table t (id int primary key, name text); create function writer() returns int deterministic return 1;");
  try {
    const cases: [string, boolean][] = [
      ["select id from t", true],
      ["select id, length(name) from t where id = 1", true],
      ["insert into t values (1, 'x')", false],
      ["select id from t for update", false],
      ["select id from t lock in share mode", false],
      ["select @a := 1", false],
      ["select get_lock('a', 0)", false],
      ["select writer()", false],
    ];
    for (const [sql, expected] of cases) assert.equal(await readOnly(db.url, sql), expected, sql);
  } finally {
    await db.drop();
    await closeSessions();
  }
});

test("on SQLite, a SELECT is read-only and a write is not", async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-read-only-"));
  const path = join(root, "app.db");
  const db = new DatabaseSync(path);
  db.exec("create table t (id integer primary key, name text)");
  db.close();
  assert.equal(await readOnly(`file:${path}`, "select id from t"), true);
  assert.equal(await readOnly(`file:${path}`, "insert into t (name) values ('x') returning id"), false);
  await closeSessions();
});
