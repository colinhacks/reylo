import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { DatabaseSync } from "node:sqlite";
import { Session } from "../src/analyze.ts";
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
    create policy guard on guarded using (writer() = 1);
    create view next_ids as select nextval('s') as id;
    create view next_ids_again as select id from next_ids;
    create view random_view as select random() as r, id from t;
    create table sequenced (id int primary key);
    alter table sequenced enable row level security;
    create policy sequenced_guard on sequenced using (nextval('s') > 0);
    create table guarded_by_view (id int primary key);
    alter table guarded_by_view enable row level security;
    create policy view_guard on guarded_by_view using (exists (select 1 from next_ids));
    create function step(int, int) returns int volatile language sql as 'select nextval(''s'')::int';
    create aggregate writing_sum(int) (sfunc = step, stype = int);
    create function pure_step(int, int) returns int immutable language sql as 'select $1 + $2';
    create aggregate pure_sum(int) (sfunc = pure_step, stype = int);
    create view aggregate_view as select writing_sum(id) as total from t;
    create function public.timeofday() returns text volatile language sql as 'select nextval(''s'')::text';`);
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
      ["select id from next_ids", false],
      ["select id from next_ids_again", false],
      ["select r from random_view", true],
      ["select id from sequenced", false],
      ["select id from guarded_by_view", false],
      ["select writing_sum(id) from t", false],
      ["select total from aggregate_view", false],
      ["select pure_sum(id), count(*) from t", true],
      ["select public.timeofday()", false],
    ];
    for (const [sql, expected] of cases) assert.equal(await readOnly(db.url, sql), expected, sql);
  } finally {
    await db.drop();
    await closeSessions();
  }
});

test("on Postgres, a table a policy reads is read by a write, not written", { skip: pgLive }, async () => {
  const db = await postgresDatabase("read_only_policy", `create table members (id int primary key, note text not null unique);
    create function touch() returns trigger language plpgsql as $$ begin return new; end $$;
    create trigger touched before insert on members for each row execute function touch();
    create table docs (id int primary key);
    alter table docs enable row level security;
    create policy member on docs using (exists (select 1 from members));`);
  const root = mkdtempSync(join(tmpdir(), "reylo-read-only-"));
  writeFileSync(join(root, "package.json"), "{}");
  const session = await Session.open(root, { DATABASE_URL: db.url });
  try {
    const outcome = await session.analyze("insert into docs values (1) returning id", { store: false });
    assert.ok(outcome.ok, !outcome.ok ? outcome.message : "");
    const errors = JSON.stringify(outcome.analysis.errors);
    assert.ok(errors?.includes("docs_pkey"), `the write's own errors are named: ${errors}`);
    assert.ok(!errors.includes("members"), `a table the policy only reads is not written: ${errors}`);
  } finally {
    await session.close();
    await db.drop();
    await closeSessions();
  }
});

test("on MySQL, only a SELECT with no lock, no INTO, no variable and no stored function is read-only", { skip: mysqlLive }, async () => {
  const db = await mysqlDatabase("read_only", "create table t (id int primary key, name text); create function writer() returns int deterministic return 1; create view locking_view as select get_lock('a', 0) as l, id from t; create view locking_again as select id from locking_view; create view plain_view as select id from t;");
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
      ["select id from plain_view", true],
      ["select id from locking_view", false],
      ["select id from locking_again", false],
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
