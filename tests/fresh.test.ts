import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { compared, sqliteDefault, type Fresh } from "../src/fresh.ts";
import { sqlModule } from "../src/loader.ts";
import { DRIVERS, pgLive, mysqlLive, INDEX, postgresDatabase, closeSessions, mysqlDatabase } from "./helpers.ts";

// The schema freshness check (src/fresh.ts, engine/core/src/fresh.rs). Postgres and MySQL run on throwaway servers named by
// REYLO_TEST_PG and REYLO_TEST_MYSQL, each test in a database of its own.
/** Each change a query survives, as the check says it. */
const drift = (fresh: Fresh, rows: readonly unknown[], dialect?: "postgres" | "mysql" | "sqlite" | "duckdb") => compared(fresh, rows, dialect)?.drifted.map((d) => `${d.name}: ${d.how}`);
/** Each change that fails the call. */
const stale = (fresh: Fresh, rows: readonly unknown[], dialect?: "postgres" | "mysql" | "sqlite" | "duckdb") => compared(fresh, rows, dialect)?.stale;

/** What `run` writes through `console.warn`. */
async function warnings(run: () => unknown): Promise<string[]> {
  const said: string[] = [];
  const warn = console.warn;
  console.warn = (message: string) => void said.push(message);
  try {
    await run();
  } finally {
    console.warn = warn;
  }
  return said;
}

const tursoLive = existsSync(join(DRIVERS, "@tursodatabase", "database")) ? false : "lab/drivers is not installed";

function project(driver?: string): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-fresh-"));
  writeFileSync(join(root, "package.json"), "{}");
  if (driver) {
    mkdirSync(join(root, "node_modules"));
    symlinkSync(join(DRIVERS, driver), join(root, "node_modules", driver));
  }
  return root;
}

/** A module built for `sql` and imported: each call gives a query object of its own, never checked yet. */
async function built(root: string, url: string, name: string, sql: string) {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, sql);
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  const { code } = await sqlModule(file, sql);
  const out = join(root, `${name}.mjs`);
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  return (await import(`${pathToFileURL(out).href}?${Math.random()}`)).default;
}

test("a change is said where the types may not hold: a column gone, retyped or made nullable; one made not null, given a default, or added, is not; rows of another shape check nothing", () => {
  const fresh: Fresh = {
    query: "…",
    rows: [
      ["users", "id", "integer", "true", "true"],
      ["users", "email", "text", "true", "false"],
    ],
    inserts: ["users"],
  };
  assert.deepEqual(drift(fresh, [["users", "id", "integer", "true", "true"], ["users", "email", "text", "true", "false"], ["users", "bio", "text", "false", "false"]]), []);
  assert.deepEqual(drift(fresh, [{ fresh_table: "users", fresh_column: "id", fresh_type: "integer", fresh_not_null: true, fresh_default: true }]), ["users.email: it was text not null, and no longer exists"]);
  assert.deepEqual(drift(fresh, [["users", "id", "bigint", "true", "true"], ["users", "email", "text", "false", "false"]]), ["users.id: it was integer not null with a default, and is now bigint not null with a default", "users.email: it was text not null, and is now text"]);
  // The types still hold: the database refuses a row the column refuses itself.
  assert.deepEqual(drift(fresh, [["users", "id", "integer", "true", "false"], ["users", "email", "text", "true", "true"]]), []);
  assert.deepEqual(drift(fresh, [...fresh.rows, ["users", "tenant", "integer", "true", "false"]]), []);
  assert.deepEqual(drift({ ...fresh, rows: [["users", "bio", "text", "false", "false"]] }, [["users", "bio", "text", "true", "false"]]), []);
  // A client that answers every query with a row of its own, as lab/nextjs's edge stub does, is no catalog.
  assert.equal(drift(fresh, [{ id: 1, name: "stub", email: null, role: "member", big: "7" }]), undefined);
  assert.equal(drift(fresh, [["users", "id", "integer"]]), undefined);
});

test("a default a structured insert copied is a fact by its text: changed, given or dropped, it fails the call; another column's default text is no fact", () => {
  const fresh: Fresh = {
    query: "…",
    rows: [
      ["accounts", "name", "text", "true", "false", ""],
      ["accounts", "role", "text", "true", "true", "'member'::text"],
      ["accounts", "email", "text", "false", "false", ""],
    ],
    inserts: ["accounts"],
    defaults: [["accounts", "role"], ["accounts", "email"]],
  };
  const now = (role: string, email: string, name = "") => [["accounts", "name", "text", "true", "false", name], ["accounts", "role", "text", "true", "true", role], ["accounts", "email", "text", "false", email ? "true" : "false", email]];
  assert.deepEqual(stale(fresh, now("'member'::text", "", "'x'::text")), []);
  assert.deepEqual(stale(fresh, now("'guest'::text", "")), ["accounts.role had the default 'member'::text, which the insert writes for a row that leaves its key out, and now has the default 'guest'::text"]);
  assert.deepEqual(stale(fresh, [{ fresh_table: "accounts", fresh_column: "name", fresh_type: "text", fresh_not_null: true, fresh_default: false, fresh_expr: null }, { fresh_table: "accounts", fresh_column: "role", fresh_type: "text", fresh_not_null: true, fresh_default: true, fresh_expr: "'member'::text" }, { fresh_table: "accounts", fresh_column: "email", fresh_type: "text", fresh_not_null: false, fresh_default: false, fresh_expr: null }]), []);
  // A default given to a column that had none: the insert would write NULL for a row that leaves the key out.
  assert.deepEqual(stale(fresh, now("'member'::text", "'a@x'::text")), ["accounts.email had no default, which the insert writes for a row that leaves its key out, and now has the default 'a@x'::text"]);
  // Rows of five values are not this module's catalog query.
  assert.equal(stale(fresh, [["accounts", "role", "text", "true", "true"]]), undefined);
});

test("a SQLite default is compared in one spelling, as node:sqlite and Turso's engine write the same one differently", () => {
  for (const [sqlite, turso] of [["member", "'member'"], ['"quoted"', "'quoted'"], ["lower(hex(randomblob(4)))", "(lower (hex (randomblob (4))))"], ["-7", "- 7"], ["current_timestamp", "CURRENT_TIMESTAMP"], ["true", "TRUE"], ["x'00ff'", "x'00ff'"], ["'it''s A b'", "'it''s A b'"]]) {
    assert.equal(sqliteDefault(sqlite!), sqliteDefault(turso!), `${sqlite} and ${turso}`);
  }
  for (const [a, b] of [["'member'", "'Member'"], ["'a b'", "'ab'"], ["7", "-7"], ["member", "'guest'"]]) assert.notEqual(sqliteDefault(a!), sqliteDefault(b!), `${a} and ${b}`);
  const fresh: Fresh = { query: "…", rows: [["a", "role", "TEXT", "1", "1", "lower(hex(randomblob(4)))"]], inserts: ["a"], defaults: [["a", "role"]] };
  assert.deepEqual(stale(fresh, [["a", "role", "text", "1", "1", "(lower (hex (randomblob (4))))"]], "sqlite"), []);
  assert.equal(stale(fresh, [["a", "role", "TEXT", "1", "1", "(lower (hex (randomblob (4))))"]], "postgres")?.length, 1);
});

test("a SQLite type is compared without its case, as SQLite reads one; a Postgres or MySQL type exactly", () => {
  const fresh: Fresh = { query: "…", rows: [["t", "id", "INTEGER", "0", "1"], ["t", "n", "bigInt", "1", "0"]], inserts: [] };
  // SQLite's pragma_table_xinfo upper-cases a type name it knows; Turso's engine gives it as declared.
  const turso = [["t", "id", "integer", "0", "1"], ["t", "n", "bigint", "1", "0"]];
  assert.deepEqual(drift(fresh, turso, "sqlite"), []);
  assert.deepEqual(drift(fresh, [["t", "id", "integer", "1", "1"], ["t", "n", "text", "1", "0"]], "sqlite"), ["t.n: it was bigInt not null, and is now text not null"]);
  assert.deepEqual(drift(fresh, turso, "postgres"), ["t.id: it was INTEGER with a default, and is now integer with a default", "t.n: it was bigInt not null, and is now bigint not null"]);
  assert.equal(drift(fresh, turso, "mysql")?.length, 2);
  // SQLite folds ASCII letters alone.
  assert.deepEqual(drift({ query: "…", rows: [["t", "x", "É", "0", "0"]], inserts: [] }, [["t", "x", "é", "0", "0"]], "sqlite"), ["t.x: it was É, and is now é"]);
});

test("SQLite on Turso's engine: a query built on node:sqlite runs, whose types Turso spells in lowercase, and a column it names, renamed, warns and fails on the statement", { skip: tursoLive }, async () => {
  const require = createRequire(join(DRIVERS, "pg", "package.json"));
  const { connect } = await import(pathToFileURL(require.resolve("@tursodatabase/database")).href);
  const root = project();
  const file = join(root, "app.db");
  const setup = new DatabaseSync(file);
  setup.exec("create table users (id integer primary key, email text not null, n bigInt, ok boolean)");
  setup.exec("insert into users (email, n, ok) values ('a@x', 7, 1)");
  setup.exec("create table accounts (id integer primary key, name text not null, role text not null default member, tok text not null default (lower(hex(randomblob(4)))), n integer default -7)");
  setup.close();
  const url = "file:./app.db";
  const db = await connect(file);
  try {
    const read = await built(root, url, "read", "select id, email, n, ok from users order by id");
    const facts = JSON.parse(/"rows":(\[\[.*?\]\]),"inserts"/.exec(readFileSync(join(root, "read.mjs"), "utf8"))![1]!) as string[][];
    assert.deepEqual(facts.map((r) => r[2]), ["TEXT", "INTEGER", "bigInt", "boolean"], "the facts node:sqlite gave the build, the names it knows upper-cased");
    assert.deepEqual((await db.prepare("select type from pragma_table_xinfo('users')").all()).map((r: { type: string }) => r.type), ["integer", "text", "bigInt", "boolean"], "the control: Turso's engine gives each type as declared");
    // A column declared BIGINT, in any case, is a BigInt.
    assert.deepEqual(await read.run(db), [{ id: 1, email: "a@x", n: 7n, ok: true }]);
    // A structured insert built on node:sqlite copies the defaults node:sqlite reads, which Turso
    // writes again in its own spelling and evaluates the same, rows that leave out different keys included.
    const insert = await built(root, url, "insert", "insert into accounts :a::rows(name text not null, role? text not null, tok? text not null, n integer) returning id, role, n");
    assert.deepEqual(await insert.run(db, { a: [{ name: "Ada" }, { name: "Grace", role: "admin", tok: "t", n: null }] }), [{ id: 1, role: "member", n: -7 }, { id: 2, role: "admin", n: null }]);
    const read2 = await built(root, url, "read2", "select id, email from users order by id");
    await db.exec("alter table users rename column email to mail");
    assert.deepEqual(await warnings(() => assert.rejects(read2.run(db))), ["reylo: users.email has changed since read2.sql was built (it was TEXT not null, and no longer exists), so its types may be out of date until the next build."]);
  } finally {
    await db.close();
    await closeSessions();
  }
});

test("SQLite: a query whose columns changed warns, naming them, and a statement that no longer runs fails on its own; one whose table gained a column it does not name says nothing", async () => {
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, email text not null, name text)");
  db.exec("insert into users (email, name) values ('a@x', 'Ada')");
  const url = "file:./app.db";
  try {
    const read = await built(root, url, "read", "select id, email from users order by id");
    assert.deepEqual((await read.run(db)).map((r: object) => ({ ...r })), [{ id: 1, email: "a@x" }]);
    // A column the statement does not name: no fact, so the query runs on.
    db.exec("alter table users add column bio text");
    const read2 = await built(root, url, "read2", "select id, email from users order by id");
    db.exec("alter table users add column age integer");
    assert.equal((await read2.run(db)).length, 1);
    // A column it names, renamed: the check says so, and the statement fails on its own, synchronously too.
    const read3 = await built(root, url, "read3", "select id, email from users order by id");
    const star = await built(root, url, "star", "select * from users order by id");
    db.exec("alter table users rename column email to mail");
    assert.deepEqual(await warnings(() => assert.rejects(read3.run(db), /no such column: email/)), ["reylo: users.email has changed since read3.sql was built (it was TEXT not null, and no longer exists), so its types may be out of date until the next build."]);
    // After a `*` the statement still runs, and the row has another key.
    assert.deepEqual(await warnings(async () => assert.equal(((await star.run(db)) as { mail: string }[])[0]!.mail, "a@x")), ["reylo: users.email has changed since star.sql was built (it was TEXT not null, and no longer exists), so its types may be out of date until the next build."]);
    const read4 = await built(root, url, "read4", "select id, name from users order by id");
    db.exec("alter table users rename column name to full_name");
    assert.deepEqual(await warnings(() => assert.throws(() => read4.runSync(db), /no such column: name/)), ["reylo: users.name has changed since read4.sql was built (it was TEXT, and no longer exists), so its types may be out of date until the next build."]);
    // An insert's table that gains a column it must fill.
    const insert = await built(root, url, "insert", "insert into users (mail) values (:mail)");
    db.exec("alter table users add column tenant integer not null default 0");
    await insert.run(db, { mail: "b@x" });
  } finally {
    db.close();
    await closeSessions();
  }
});

test("SQLite: a default a structured insert copied, changed after the build, fails the call naming the table and the column", async () => {
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table accounts (id integer primary key, name text not null, role text not null default 'member')");
  const url = "file:./app.db";
  const insert = "insert into accounts :a::rows(name text not null, role? text not null) returning role";
  try {
    const q = await built(root, url, "insert_accounts", insert);
    assert.deepEqual((await q.run(db, { a: [{ name: "Ada" }, { name: "Grace", role: "admin" }] })).map((r: object) => ({ ...r })), [{ role: "member" }, { role: "admin" }]);
    // A table rebuilt with another default: the module would still write 'member'.
    const stale = await built(root, url, "insert_accounts2", insert);
    db.exec(`create table accounts_new (id integer primary key, name text not null, role text not null default 'guest');
      insert into accounts_new select * from accounts; drop table accounts; alter table accounts_new rename to accounts`);
    await assert.rejects(stale.run(db, { a: [{ name: "Linus" }] }), /The database changed after insert_accounts2\.sql was built: accounts\.role had the default 'member', which the insert writes for a row that leaves its key out, and now has the default 'guest'\. Build it again/);
    assert.equal((db.prepare("select count(*) as n from accounts").get() as { n: number }).n, 2);
    // A query that does not copy the default runs on.
    const read = await built(root, url, "read", "select name, role from accounts order by id");
    assert.equal((await read.run(db)).length, 2);
  } finally {
    db.close();
    await closeSessions();
  }
});

test("Postgres: a default a structured insert copied, changed after the build, fails the call naming the table and the column", { skip: pgLive }, async () => {
  const own = await postgresDatabase("reylo_fresh_defaults");
  const url = new URL(own.url);
  const sql = own.sql;
  const root = project("postgres");
  try {
    await sql.unsafe(`create table accounts (id int8 generated by default as identity primary key, name text not null, role text not null default 'member', note text)`);
    const insert = "insert into accounts :a::rows(name text not null, role? text not null, note text) returning role";
    const q = await built(root, url.href, "insert_accounts", insert);
    assert.deepEqual((await q.run(sql, { a: [{ name: "Ada" }, { name: "Grace", role: "admin" }] })).map((r: object) => ({ ...r })), [{ role: "member" }, { role: "admin" }]);
    const stale = await built(root, url.href, "insert_accounts2", insert);
    await sql.unsafe(`alter table accounts alter column role set default 'guest'`);
    await assert.rejects(stale.run(sql, { a: [{ name: "Linus" }] }), /accounts\.role had the default 'member'::text, which the insert writes for a row that leaves its key out, and now has the default 'guest'::text/);
    // A default given to a nullable column whose missing key wrote NULL.
    const stale2 = await built(root, url.href, "insert_accounts3", insert);
    await sql.unsafe(`alter table accounts alter column note set default 'n/a'`);
    await assert.rejects(stale2.run(sql, { a: [{ name: "Linus" }] }), /accounts\.note had no default, which the insert writes for a row that leaves its key out, and now has the default 'n\/a'::text/);
    assert.deepEqual([...(await sql.unsafe("select count(*)::int as n from accounts"))].map((r) => ({ ...r })), [{ n: 2 }]);
  } finally {
    await closeSessions();
    await own.drop();
  }
});

test("Postgres: a retyped column warns once and runs, a new nullable one says nothing, and a new required one is the database's to refuse", { skip: pgLive }, async () => {
  const own = await postgresDatabase("reylo_fresh");
  const url = new URL(own.url);
  const sql = own.sql;
  const root = project("postgres");
  try {
    await sql.unsafe(`create table "Users" (id int8 generated always as identity primary key, email text not null, name text)`);
    const insert = await built(root, url.href, "insert_user", `insert into "Users" (email) values (:email) returning id`);
    await insert.run(sql, { email: "a@x" });
    const read = await built(root, url.href, "read_users", `select id, email from "Users" order by id`);
    await sql.unsafe(`alter table "Users" add column bio text`);
    assert.equal((await read.run(sql)).length, 1);
    const read2 = await built(root, url.href, "read_users2", `select id, email from "Users" order by id`);
    await sql.unsafe(`alter table "Users" alter column email type varchar(100)`);
    assert.deepEqual(await warnings(async () => assert.equal((await read2.run(sql)).length, 1)), ["reylo: Users.email has changed since read_users2.sql was built (it was text not null, and is now character varying(100) not null), so its types may be out of date until the next build."]);
    // Checked again a minute later, it says nothing more.
    assert.deepEqual(await warnings(async () => assert.equal((await read2.run(sql)).length, 1)), []);
    // Built again on the new schema, by a session of its own.
    await closeSessions();
    const insert2 = await built(root, url.href, "insert_user2", `insert into "Users" (email) values (:email) returning id`);
    // Rows exist, so the column takes a default to be added, which then goes.
    await sql.unsafe(`alter table "Users" add column tenant int not null default 0; alter table "Users" alter column tenant drop default`);
    await assert.rejects(insert2.run(sql, { email: "b@x" }), /null value in column "tenant"/);
  } finally {
    await closeSessions();
    await own.drop();
  }
});

test("Postgres: a function whose body the types were read from, replaced, or a column its body reads, changed, warns once and runs; a body that settled nothing is no fact", { skip: pgLive }, async () => {
  const own = await postgresDatabase("reylo_fresh_bodies");
  const url = new URL(own.url);
  const sql = own.sql;
  const root = project("postgres");
  const reader = "create function comments_for(p int8) returns table (id int8, body text) language sql stable as $$ select id, body from comments where post_id = p $$";
  try {
    await sql.unsafe("create table comments (id int8 primary key, post_id int8 not null, body text not null, note text)");
    await sql.unsafe(reader);
    await sql.unsafe("create function atomic_comments(p int8) returns table (id int8, body text) language sql stable begin atomic select id, body from comments where post_id = p; end");
    await sql.unsafe("create function in_plpgsql(p int8) returns table (id int8) language plpgsql stable as $$ begin return query select c.id from comments c where c.post_id = p; end $$");
    await sql.unsafe("insert into comments values (1, 1, 'a', null)");
    const statement = "select id, body from comments_for(:p::int8)";
    const q = await built(root, url.href, "comments_for", statement);
    assert.deepEqual(await warnings(async () => assert.equal((await q.run(sql, { p: 1 })).length, 1)), []);
    // The body changes and the signature does not.
    const replaced = await built(root, url.href, "comments_for2", statement);
    await sql.unsafe("create or replace function comments_for(p int8) returns table (id int8, body text) language sql stable as $$ select id, note from comments where post_id = p $$");
    assert.deepEqual(await warnings(async () => assert.equal((await replaced.run(sql, { p: 1 })).length, 1)), ["reylo: comments_for(bigint) has changed since comments_for2.sql was built, so its types may be out of date until the next build."]);
    await closeSessions();
    // The same for a body stored parsed, whose source text is empty.
    const atomic = await built(root, url.href, "atomic_comments", "select id, body from atomic_comments(:p::int8)");
    await sql.unsafe("create or replace function atomic_comments(p int8) returns table (id int8, body text) language sql stable begin atomic select id, note from comments where post_id = p; end");
    assert.deepEqual(await warnings(async () => assert.equal((await atomic.run(sql, { p: 1 })).length, 1)), ["reylo: atomic_comments(bigint) has changed since atomic_comments.sql was built, so its types may be out of date until the next build."]);
    await closeSessions();
    // A column the body reads, loosened with the function untouched.
    await sql.unsafe("drop function comments_for(int8)");
    await sql.unsafe(reader);
    const read = await built(root, url.href, "comments_for3", statement);
    await sql.unsafe("alter table comments alter column body drop not null");
    assert.deepEqual(await warnings(async () => assert.equal((await read.run(sql, { p: 1 })).length, 1)), ["reylo: comments.body has changed since comments_for3.sql was built (it was text not null, and is now text), so its types may be out of date until the next build."]);
    await closeSessions();
    // A body that settled nothing is no fact: a function in plpgsql keeps today's result.
    const opaque = await built(root, url.href, "in_plpgsql", "select id from in_plpgsql(:p::int8)");
    await sql.unsafe("create or replace function in_plpgsql(p int8) returns table (id int8) language plpgsql stable as $$ begin return query select c.post_id from comments c; end $$");
    assert.deepEqual(await warnings(async () => assert.equal((await opaque.run(sql, { p: 1 })).length, 1)), []);
  } finally {
    await closeSessions();
    await own.drop();
  }
});

test("MySQL: a retyped column warns and runs, and a new nullable one says nothing", { skip: mysqlLive }, async () => {
  const own = await mysqlDatabase("reylo_fresh");
  const url = new URL(own.url);
  const db = own.db;
  const root = project("mysql2");
  try {
    await db.query("create table users (id bigint auto_increment primary key, email varchar(100) not null)");
    await db.query("insert into users (email) values ('a@x')");
    const read = await built(root, url.href, "read_users", "select id, email from users order by id");
    await db.query("alter table users add column bio text");
    assert.equal((await read.run(db)).length, 1);
    const read2 = await built(root, url.href, "read_users2", "select id, email from users order by id");
    await db.query("alter table users modify email varchar(200) not null");
    assert.deepEqual(await warnings(async () => assert.equal((await read2.run(db)).length, 1)), ["reylo: users.email has changed since read_users2.sql was built (it was varchar(100) not null, and is now varchar(200) not null), so its types may be out of date until the next build."]);
  } finally {
    await closeSessions();
    await own.drop();
  }
});
