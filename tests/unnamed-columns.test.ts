import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { sqlModule } from "../src/loader.ts";
import { defineQuery } from "../src/index.ts";
import { DRIVERS, INDEX, closeSessions, driver, mysqlDatabase, mysqlLive, pgLive, postgresDatabase, transform } from "./helpers.ts";

// A statement whose one column is an expression with no alias gives that column's values, since the
// only name it has is one the database makes up. Among several columns such a name is an error where
// it is no identifier. What reads rows by a name the driver reports must not depend on it.

function project(url: string, ...drivers: string[]): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-unnamed-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  writeFileSync(join(root, "tsconfig.json"), "{}");
  if (drivers.length) {
    mkdirSync(join(root, "node_modules"));
    for (const name of drivers) symlinkSync(join(DRIVERS, name), join(root, "node_modules", name));
  }
  return root;
}

/** The module a build step writes for `sql`, imported. */
async function built(root: string, name: string, sql: string) {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, sql);
  const { code } = await sqlModule(file, sql);
  const out = join(root, `${name}.mjs`);
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  return (await import(pathToFileURL(out).href)).default;
}

/** The `Row` each statement declares, and the codes of its diagnostics. */
async function declared(root: string, statements: Record<string, string>) {
  const out: Record<string, { row: string; codes: number[] }> = {};
  const files = Object.fromEntries(Object.entries(statements).map(([name, sql]) => [`${name}.sql`, sql]));
  for (const [file, { text, diagnostics }] of Object.entries(await transform(root, files))) {
    out[file.replace(/\.sql$/, "")] = { row: /^export type Row = (.*);$/m.exec(text)![1]!, codes: diagnostics.map((d) => d.code) };
  }
  return out;
}

const SQLITE_SETUP = "create table users (id integer primary key, name text not null, email text unique) strict; insert into users (name, email) values ('A', 'a@x'), ('B', null)";

function sqlite() {
  const root = project("file:./app.db");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec(SQLITE_SETUP);
  return { root, db };
}

test("the type of one unnamed column is its value's, and any other column keeps its row", async () => {
  const { root } = sqlite();
  const types = await declared(root, {
    count: "select count(*) from users",
    named: "select count(*) as total from users",
    bare: "select id from users",
    call: "select lower(name) from users",
    nullable: "select lower(email) from users where id = 1",
  });
  assert.deepEqual(types.count, { row: "number", codes: [] });
  assert.deepEqual(types.named, { row: "{ total: number }", codes: [] });
  assert.deepEqual(types.bare, { row: "{ id: number }", codes: [] });
  assert.deepEqual(types.call, { row: "string", codes: [] });
  assert.deepEqual(types.nullable, { row: "string | null", codes: [] });
  await closeSessions();
});

test("among several columns, a made-up name that is no identifier is an error at the column, and the statement is still typed", async () => {
  const { root } = sqlite();
  const sql = "select id, count(*) from users group by id";
  const { [`refused.sql`]: refused } = await transform(root, { "refused.sql": sql });
  assert.equal(refused!.diagnostics.length, 1);
  const [d] = refused!.diagnostics;
  assert.equal(d!.code, 23);
  assert.equal(sql.slice(d!.start, d!.start + d!.length), "count(*)");
  assert.match(d!.messageText, /^Column 2 has no name: the database calls it "count\(\*\)"/);
  assert.match(refused!.text, /export type Row = \{ id: number; "count\(\*\)": number \};/);
  const types = await declared(root, { fixed: "select id, count(*) as total from users group by id" });
  assert.deepEqual(types.fixed, { row: "{ id: number; total: number }", codes: [] });
  await closeSessions();
});

test("SQLite: every call gives the values, whatever name the driver reads", async () => {
  const { root, db } = sqlite();
  const count = await built(root, "count", "select count(*) from users");
  assert.equal(await count.run(db), 2);
  assert.equal(count.runSync(db), 2);
  assert.deepEqual(await count.try(db), { value: 2 });
  assert.deepEqual(count.trySync(db), { value: 2 });
  assert.equal(await count.with(db)(), 2);

  const names = await built(root, "names", "select lower(name) from users order by id");
  assert.deepEqual(await names.run(db), ["a", "b"]);

  // NULL and no row are one value here, and an alias tells them apart.
  const email = await built(root, "email", "select lower(email) from users where id = :id");
  assert.equal(await email.run(db, { id: 1 }), "a@x");
  assert.equal(await email.run(db, { id: 2 }), null);
  assert.equal(await email.run(db, { id: 9 }), null);
  const named = await built(root, "named", "select lower(email) as email from users where id = :id");
  assert.deepEqual({ ...(await named.run(db, { id: 2 })) }, { email: null });
  assert.equal(await named.run(db, { id: 9 }), null);

  // A write's RETURNING takes the rule too, and a bare column keeps its row.
  const added = await built(root, "added", "insert into users (name) values (:name::text) returning id + 1");
  assert.equal(await added.run(db, { name: "C" }), 4);
  const bare = await built(root, "bare", "insert into users (name) values (:name::text) returning id");
  assert.deepEqual({ ...(await bare.run(db, { name: "D" })) }, { id: 4 });

  // A row count still holds.
  const several = await built(root, "several", "select lower(name) from users limit 1");
  assert.equal(several.cardinality, "maybeOne");
  await closeSessions();
});

test("Standard Query: `decode` and `output` give the values, each column keeps its database's name, and the cardinality tells a list from one value", async () => {
  const { root, db } = sqlite();
  const count = await built(root, "count", "select count(*) from users");
  const query = count["~standard"].query;
  assert.equal(query.cardinality, "one");
  assert.deepEqual(query.columns.map((c: { name: string }) => c.name), ["count(*)"]);
  assert.equal(query.decode([[2n]]), 2);
  assert.equal(query.output([{ "count(*)": 2 }]), 2);
  assert.throws(() => query.decode([[1n], [2n]]), /Expected exactly one row, got 2/);

  // A list of rows is not told from an array by its output alone.
  const list = await built(root, "list", "select json_group_array(id) from users");
  assert.equal(list["~standard"].query.cardinality, "one");
  assert.deepEqual((await list.run(db)).sort(), [1, 2]);
  const names = await built(root, "names", "select lower(name) from users order by id");
  assert.equal(names["~standard"].query.cardinality, "many");
  assert.deepEqual(names["~standard"].query.decode([["a"], ["b"]]), ["a", "b"]);
  await closeSessions();
});

test("a query's type carries its cardinality in its Standard Query, which an output of an array can't say", () => {
  const query = defineQuery<Record<never, never>, number[], "one", "sqlite">({ dialect: "sqlite", sql: "select 1", params: [], cardinality: "one" });
  const cardinality: "one" = query["~standard"].query.cardinality;
  assert.equal(cardinality, "one");
});

test("the output's JSON Schema is the value's", async () => {
  const { root } = sqlite();
  const count = await built(root, "count", "select count(*) from users");
  assert.equal(count.outputSchema.type, "integer");
  const names = await built(root, "names", "select lower(name) from users");
  assert.equal(names.outputSchema.type, "array");
  assert.equal(names.outputSchema.items.type, "string");
  // One null stands for NULL and for no row.
  const email = await built(root, "email", "select lower(email) from users where id = 1");
  assert.deepEqual(email.outputSchema.type, ["string", "null"]);
  const bare = await built(root, "bare", "select name from users where id = 1");
  assert.deepEqual(bare.outputSchema.anyOf[1], { type: "null" });
  await closeSessions();
});

test("Postgres: the values on each driver, and the names only identifiers pass", { skip: pgLive }, async () => {
  const database = await postgresDatabase("reylo_unnamed", "create table users (id serial primary key, name text not null, email text unique); insert into users (name, email) values ('A', 'a@x'), ('B', null)");
  try {
    const root = project(database.url, "pg", "postgres");
    const count = await built(root, "count", "select count(*)::int from users");
    const expression = await built(root, "expression", "select id + :n::int from users where id = 1");
    const list = await built(root, "list", "select array_agg(id order by id) from users");
    const json = await built(root, "json", "select jsonb_build_object('id', id) from users where id = 1");
    const pg = driver("pg");
    const pool = new pg.Pool({ connectionString: database.url });
    const postgres = driver("postgres")(database.url);
    try {
      for (const client of [pool, postgres]) {
        assert.equal(await count.run(client), 2);
        assert.equal(await expression.run(client, { n: 5 }), 6);
        assert.deepEqual(await list.run(client), [1, 2]);
        assert.deepEqual(await json.run(client), { id: 1 });
      }
    } finally {
      await pool.end();
      await postgres.end();
    }
    const types = await declared(root, {
      aggregate: "select id, count(*) from users group by id",
      call: "select id, lower(name) from users",
      expression: "select id, id + 1 from users",
      two: "select 1 + 1, 2 + 2",
      one: "select 1 + 1",
    });
    assert.deepEqual(types.aggregate, { row: "{ id: number; count: bigint }", codes: [] });
    assert.deepEqual(types.call, { row: "{ id: number; lower: string }", codes: [] });
    assert.deepEqual(types.expression!.codes, [23]);
    assert.deepEqual(types.two!.codes, [23, 23]);
    assert.deepEqual(types.one, { row: "number", codes: [] });
  } finally {
    await closeSessions();
    await database.drop();
  }
});

test("MySQL: the values on each driver, whatever the driver names the column", { skip: mysqlLive }, async () => {
  const database = await mysqlDatabase("reylo_unnamed", "create table users (id int auto_increment primary key, name varchar(50) not null, email varchar(255) unique); insert into users (name, email) values ('A', 'a@x'), ('B', null)");
  try {
    const root = project(database.url, "mysql2", "mariadb");
    // mariadb writes each value into the statement's text, so MySQL names this column after the value.
    const expression = await built(root, "expression", "select id + :n::int from users where id = 1");
    const count = await built(root, "count", "select cast(count(*) as double) from users");
    const mysql2 = await driver("mysql2/promise").createConnection(database.url);
    const u = new URL(database.url);
    const mariadb = await driver("mariadb").createConnection({ host: u.hostname, port: Number(u.port), user: decodeURIComponent(u.username), database: u.pathname.slice(1) });
    try {
      for (const client of [mysql2, mariadb]) {
        assert.equal(await expression.run(client, { n: 5 }), 6n);
        assert.equal(await expression.run(client, { n: 6 }), 7n);
        assert.equal(await count.run(client), 2);
      }
    } finally {
      await mysql2.end();
      await mariadb.end();
    }
    const types = await declared(root, {
      expression: "select id, id + 1 from users",
      aggregate: "select id, count(*) from users group by id",
      named: "select id, count(*) as total from users group by id",
    });
    assert.deepEqual(types.expression!.codes, [23]);
    assert.deepEqual(types.aggregate!.codes, [23]);
    assert.deepEqual(types.named, { row: "{ id: number; total: bigint }", codes: [] });
  } finally {
    await closeSessions();
    await database.drop();
  }
});
