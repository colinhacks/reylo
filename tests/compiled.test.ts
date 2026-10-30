import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { sqlModule } from "../src/loader.ts";
import { defineQuery } from "../src/index.ts";
import { declarations, DRIVERS, pgLive, mysqlLive, INDEX, postgresDatabase, closeSessions, mysqlDatabase } from "./helpers.ts";

// A query's `sql` is the statement as Reylo sends it to the database, the engine's compiled text,
// and `raw` the file as written. Both are the module's exports too,
// and the content mapper declares both as string literals. Postgres and MySQL run on throwaway
// servers (REYLO_TEST_PG, REYLO_TEST_MYSQL), each test in a database of its own.

function project(url: string, driver?: string): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-compiled-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  if (driver) {
    mkdirSync(join(root, "node_modules"));
    symlinkSync(join(DRIVERS, driver), join(root, "node_modules", driver));
  }
  return root;
}

/** The module a build step writes for `raw`, imported: its default export and its `sql` and `raw` exports. */
async function built(root: string, name: string, raw: string) {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, raw);
  const { code } = await sqlModule(file, raw);
  const out = join(root, `${name}.mjs`);
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  return (await import(pathToFileURL(out).href)) as { default: { sql: string; raw: string; "~standard": { query: { text: string } }; run(client: unknown, params?: unknown): Promise<unknown> }; sql: string; raw: string };
}

/** The declaration's lines that carry the two texts, their doc comments left out. */
function texts(declaration: string): string[] {
  return declaration.split("\n").filter((line) => /^export declare const (sql|raw):|^ {2}readonly (sql|raw|source):/.test(line) || line.startsWith("declare const query:")).map((line) => line.replace(/^declare const query: Query<.*> & /, "declare const query: Query<…> & "));
}

const GET_USER = "select id, name, email from users where id = :id\n";

test("a query defined by hand: `raw` is its statement as written and `sql` the one the run time compiles from it", () => {
  const raw = "select :id as id, :name as name, :id as again";
  const pg = defineQuery({ dialect: "postgres", sql: raw, params: ["id", "name"] });
  assert.equal(pg.raw, raw);
  assert.equal(pg.sql, "select $1 as id, $2 as name, $1 as again");
  for (const dialect of ["mysql", "sqlite"] as const) {
    const q = defineQuery({ dialect, sql: raw, params: ["id", "name"] });
    assert.equal(q.raw, raw);
    assert.equal(q.sql, "select ? as id, ? as name, ? as again");
  }
  // The compiled text a build step writes wins over the run time's own lexer.
  assert.equal(defineQuery({ dialect: "postgres", sql: "select :id", params: ["id"], text: "select $1::int", order: ["id"] }).sql, "select $1::int");
  // A placeholder its params miss fails where `sql` is read, as `run` does.
  const missing = defineQuery({ dialect: "postgres", sql: "select :id, :other", params: ["id"] });
  assert.equal(missing.raw, "select :id, :other");
  assert.throws(() => missing.sql, /The statement's placeholder :other is parameter "other", which the query's params \(\["id"\]\) do not list/);
});

test("SQLite: `sql` is the compiled statement and `raw` the file, at run time and in the mapper's declaration", async () => {
  const root = project("file:./app.db");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null, email text)");
  db.exec("insert into users (name, email) values ('Ada', 'ada@example.com')");
  const doc = `select json_extract(:doc::json, '$.a') as a, :tags::json('{ "type": "array", "items": { "type": "string" } }') as tags\n`;
  try {
    const getUser = await built(root, "get_user", GET_USER);
    assert.equal(getUser.default.sql, "select id, name, email from users where id = ?");
    // The statement Reylo sends ends with the comment naming the file, which `sql` leaves out.
    assert.equal(getUser.default["~standard"].query.text, "select id, name, email from users where id = ? /*file='get_user.sql'*/");
    assert.equal(getUser.default.raw, GET_USER);
    assert.equal(getUser.sql, getUser.default.sql);
    assert.equal(getUser.raw, GET_USER);
    assert.deepEqual({ ...((await getUser.default.run(db, { id: 1 })) as object) }, { id: 1, name: "Ada", email: "ada@example.com" });
    const json = await built(root, "doc", doc);
    assert.equal(json.default.sql, "select json_extract(json(?), '$.a') as a, ? as tags");
    assert.equal(json.default.raw, doc);
    assert.deepEqual({ ...((await json.default.run(db, { doc: { a: 5 }, tags: ["x"] })) as object) }, { a: 5, tags: '["x"]' });
    await closeSessions();
    const declared = await declarations(root, { "get_user.sql": GET_USER, "doc.sql": doc });
    assert.deepEqual(texts(declared["get_user.sql"]!), [
      `export declare const sql: "select id, name, email from users where id = ?";`,
      `export declare const raw: "select id, name, email from users where id = :id\\n";`,
      `declare const query: Query<…> & {`,
      `  readonly sql: typeof sql;`,
      `  readonly raw: typeof raw;`,
      `  readonly source: string;`,
    ]);
    // The query's doc comment is the statement as written; `sql` and `raw` carry none, so a hover
    // over either shows its text once, as its type.
    assert.ok(declared["get_user.sql"]!.includes("/**\n * ```sql\n * select id, name, email from users where id = :id\n * ```\n */\ndeclare const query:"), declared["get_user.sql"]!);
    assert.equal(declared["get_user.sql"]!.split("/**").length, 2, declared["get_user.sql"]!);
    // The parameters are one object type, with no tuple beside it.
    const lines = declared["get_user.sql"]!.split("\n");
    assert.ok(lines.includes("export type Params = { id: number };"), declared["get_user.sql"]!);
    assert.ok(!lines.some((line) => line.startsWith("export type Values")), declared["get_user.sql"]!);
    assert.match(lines.find((line) => line.startsWith("declare const query:"))!, /^declare const query: Query<\{ id: number \}, \{ id: number; name: string; email: string \| null \}, "maybeOne", "sqlite", QueryError> & /);
    assert.deepEqual(texts(declared["doc.sql"]!).slice(0, 2), [`export declare const sql: ${JSON.stringify(json.default.sql)};`, `export declare const raw: ${JSON.stringify(doc)};`]);
  } finally {
    db.close();
    await closeSessions();
  }
});

test("SQLite: a statement that fires no trigger declares its QueryError narrowed to the keys and columns it writes through, and one that fires one the whole of it", async () => {
  const root = project("file:./app.db");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec(`
    create table users (id integer primary key, email text unique not null);
    create table audited (id integer primary key, n integer);
    create trigger audited_audit after insert on audited begin update audited set n = 1 where id = new.id; end;
  `);
  db.close();
  const insert = "insert into users (email) values (:email)\n";
  const audited = "insert into audited (id) values (:id)\n";
  try {
    const declared = await declarations(root, { "insert_user.sql": insert, "insert_audited.sql": audited });
    const queryError = (file: string) => /^export type QueryError = ([\s\S]*?>);$/m.exec(declared[file]!)![1];
    assert.equal(queryError("insert_user.sql"), [
      `import("reylo").SqliteQueryError<{`,
      `  "SQLITE_CONSTRAINT": { columns: readonly ["email"] | readonly ["id"] | readonly ["rowid"]; table: "users" };`,
      `  "SQLITE_CONSTRAINT_NOTNULL": { columns: readonly ["email"] | readonly ["id"]; table: "users" };`,
      `  "SQLITE_CONSTRAINT_PRIMARYKEY": { columns: readonly ["id"]; table: "users" };`,
      `  "SQLITE_CONSTRAINT_ROWID": { columns: readonly ["rowid"]; table: "users" };`,
      `  "SQLITE_CONSTRAINT_UNIQUE": { columns: readonly ["email"]; table: "users" };`,
      `}>`,
    ].join("\n"));
    assert.equal(queryError("insert_audited.sql"), `import("reylo").QueryError<"sqlite">`);
  } finally {
    await closeSessions();
  }
});

test("Postgres: `sql` numbers the parameters and keeps a typed JSON parameter's cast as `$1::jsonb`", { skip: pgLive }, async () => {
  const own = await postgresDatabase("reylo_compiled");
  const url = new URL(own.url);
  const sql = own.sql;
  const root = project(url.href, "postgres");
  const names = `select :names::jsonb('{ "type": "array", "items": { "type": "string" } }') as names\n`;
  try {
    await sql.unsafe(`create table users (id int generated always as identity primary key, name text not null, email text); insert into users (name, email) values ('Ada', 'ada@example.com')`).simple();
    const getUser = await built(root, "get_user", GET_USER);
    assert.equal(getUser.default.sql, "select id, name, email from users where id = $1");
    assert.equal(getUser.sql, getUser.default.sql);
    assert.equal(getUser.default.raw, GET_USER);
    assert.equal(getUser.raw, GET_USER);
    assert.deepEqual({ ...((await getUser.default.run(sql, { id: 1 })) as object) }, { id: 1, name: "Ada", email: "ada@example.com" });
    const json = await built(root, "names", names);
    assert.equal(json.default.sql, "select $1::jsonb as names");
    assert.equal(json.default.raw, names);
    assert.deepEqual({ ...((await json.default.run(sql, { names: ["a", "b"] })) as object) }, { names: ["a", "b"] });
    await closeSessions();
    const declared = await declarations(root, { "get_user.sql": GET_USER, "names.sql": names });
    assert.deepEqual(texts(declared["get_user.sql"]!), [
      `export declare const sql: "select id, name, email from users where id = $1";`,
      `export declare const raw: "select id, name, email from users where id = :id\\n";`,
      `declare const query: Query<…> & {`,
      `  readonly sql: typeof sql;`,
      `  readonly raw: typeof raw;`,
      `  readonly source: string;`,
    ]);
    assert.deepEqual(texts(declared["names.sql"]!).slice(0, 2), [`export declare const sql: "select $1::jsonb as names";`, `export declare const raw: ${JSON.stringify(names)};`]);
  } finally {
    await closeSessions();
    await own.drop();
  }
});

test("Postgres: a statement that runs only the server's own code declares its QueryError narrowed to what it writes through, and one a trigger runs on the whole of it", { skip: pgLive }, async () => {
  const own = await postgresDatabase("reylo_query_error");
  const url = new URL(own.url);
  const sql = own.sql;
  const root = project(url.href, "postgres");
  const insert = "insert into users (email, age) values (:email::text, :age::int)\n";
  const audited = "insert into audited (id) values (:id::int)\n";
  const select = "select id, email from users\n";
  try {
    await sql.unsafe(`
      create table users (id int generated always as identity primary key, email text unique not null, age int constraint adult check (age >= 18));
      create table audited (id int primary key);
      create function audit() returns trigger language plpgsql as $$ begin return new; end $$;
      create trigger audited_audit before insert on audited for each row execute function audit();
    `).simple();
    const declared = await declarations(root, { "insert_user.sql": insert, "insert_audited.sql": audited, "list_users.sql": select });
    const queryError = (file: string) => /^export type QueryError = ([\s\S]*?>);$/m.exec(declared[file]!)![1];
    assert.equal(queryError("insert_user.sql"), [
      `import("reylo").PostgresQueryError<{`,
      `  "23502": { column: "email" | "id"; table: "users" };`,
      `  "23505": { constraint: "users_email_key" | "users_pkey"; table: "users" };`,
      `  "23514": { constraint: "adult"; table: "users" };`,
      `}>`,
    ].join("\n"));
    assert.equal(queryError("insert_audited.sql"), `import("reylo").QueryError<"postgres">`);
    assert.equal(queryError("list_users.sql"), `import("reylo").PostgresQueryError<{}>`);
    assert.match(declared["insert_user.sql"]!, /^declare const query: Query<.*, "none", "postgres", QueryError> & \{$/m);
  } finally {
    await closeSessions();
    await own.drop();
  }
});

test("MySQL: `sql` has `?` placeholders and writes a typed JSON parameter's cast as `cast(? as json)`", { skip: mysqlLive }, async () => {
  const own = await mysqlDatabase("reylo_compiled");
  const url = new URL(own.url);
  const db = own.db;
  const root = project(url.href, "mysql2");
  const names = `select :names::json('{ "type": "array", "items": { "type": "string" } }') as names\n`;
  try {
    await db.query("create table users (id int auto_increment primary key, name varchar(100) not null, email varchar(100))");
    await db.query("insert into users (name, email) values ('Ada', 'ada@example.com')");
    const getUser = await built(root, "get_user", GET_USER);
    assert.equal(getUser.default.sql, "select id, name, email from users where id = ?");
    assert.equal(getUser.sql, getUser.default.sql);
    assert.equal(getUser.default.raw, GET_USER);
    assert.equal(getUser.raw, GET_USER);
    assert.deepEqual({ ...((await getUser.default.run(db, { id: 1 })) as object) }, { id: 1, name: "Ada", email: "ada@example.com" });
    const json = await built(root, "names", names);
    assert.equal(json.default.sql, "select cast(? as json) as names");
    assert.equal(json.default.raw, names);
    assert.deepEqual({ ...((await json.default.run(db, { names: ["a", "b"] })) as object) }, { names: ["a", "b"] });
    await closeSessions();
    const declared = await declarations(root, { "get_user.sql": GET_USER, "names.sql": names });
    assert.deepEqual(texts(declared["get_user.sql"]!), [
      `export declare const sql: "select id, name, email from users where id = ?";`,
      `export declare const raw: "select id, name, email from users where id = :id\\n";`,
      `declare const query: Query<…> & {`,
      `  readonly sql: typeof sql;`,
      `  readonly raw: typeof raw;`,
      `  readonly source: string;`,
    ]);
    assert.deepEqual(texts(declared["names.sql"]!).slice(0, 2), [`export declare const sql: "select cast(? as json) as names";`, `export declare const raw: ${JSON.stringify(names)};`]);
  } finally {
    await closeSessions();
    await own.drop();
  }
});

test("the comment naming the file is sent and left out of sql, sqlcommenter: false leaves it off, and a running build sees the change", async () => {
  const root = project("file:./app.db");
  writeFileSync(join(root, ".reylo.yaml"), "sqlcommenter: false\n");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null, email text)");
  db.exec("insert into users (name, email) values ('Ada', 'ada@example.com')");
  try {
    const plain = await built(root, "get_user", GET_USER);
    assert.equal(plain.sql, "select id, name, email from users where id = ?");
    assert.equal(plain.default["~standard"].query.text, "select id, name, email from users where id = ?");
    assert.deepEqual({ ...((await plain.default.run(db, { id: 1 })) as object) }, { id: 1, name: "Ada", email: "ada@example.com" });
    const declared = await declarations(root, { "get_user.sql": GET_USER });
    assert.ok(texts(declared["get_user.sql"]!).includes(`export declare const sql: "select id, name, email from users where id = ?";`), declared["get_user.sql"]!);
    // The same process, after the project file turns it back on: the session it builds with is a
    // new one, and the statement it sends ends with the comment, which `sql` leaves out.
    writeFileSync(join(root, ".reylo.yaml"), "sqlcommenter: true\n");
    const tagged = await built(root, "get_user_again", GET_USER);
    assert.equal(tagged.sql, "select id, name, email from users where id = ?");
    assert.equal(tagged.default["~standard"].query.text, "select id, name, email from users where id = ? /*file='get_user_again.sql'*/");
    assert.deepEqual({ ...((await tagged.default.run(db, { id: 1 })) as object) }, { id: 1, name: "Ada", email: "ada@example.com" });
  } finally {
    db.close();
    await closeSessions();
  }
});
