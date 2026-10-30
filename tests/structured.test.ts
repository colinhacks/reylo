import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { Session } from "../src/analyze.ts";
import { sqlModule } from "../src/loader.ts";
import { DRIVERS, PG, MYSQL, pgLive, mysqlLive, INDEX, postgresDatabase, closeSessions, plain, mysqlDatabase, driver } from "./helpers.ts";

// Structured parameters (wiki/structured-params.md): a parameter cast to `rows(…)` or `row(…)` is
// typed by its column list and written for its position; the run time checks each value, and a key
// a row leaves out takes its column's default: on Postgres and SQLite the insert holds each such
// default, copied from the catalog, and on MySQL the run time writes the insert's column lists for
// the keys a call carries. Postgres and MySQL run on throwaway servers, never one anyone uses: each test
// makes a database of its own there and drops it after.

function project(driver?: string): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-structured-"));
  writeFileSync(join(root, "package.json"), "{}");
  if (driver) {
    mkdirSync(join(root, "node_modules"));
    symlinkSync(join(DRIVERS, driver), join(root, "node_modules", driver));
  }
  return root;
}

/** The module a build step writes for `sql`, imported. */
async function built(root: string, url: string, name: string, sql: string) {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, sql);
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  const { code } = await sqlModule(file, sql);
  const out = join(root, `${name}.mjs`);
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  return (await import(pathToFileURL(out).href)).default;
}

async function analysis(root: string, url: string, sql: string) {
  const session = await Session.open(root, { DATABASE_URL: url });
  try {
    const outcome = await session.analyze(sql, { force: true });
    assert.ok(outcome.ok, outcome.ok ? "" : outcome.message);
    return outcome.analysis;
  } finally {
    await session.close();
  }
}

// The rows as plain objects: drivers return them with their own prototypes and extra properties.

test("SQLite: an insert's rows are checked and written in their order, a key left out takes its default, and the column list types the parameter", async () => {
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table events (id integer primary key, ts text not null, status text not null default 'new', amount real, meta text)");
  const url = "file:./app.db";
  const insert = `insert into events :events::rows(id integer not null, ts text not null, status? text not null, amount real, meta json('{"type": "object", "properties": {"source": {"type": "string"}}}')) returning id, status`;
  try {
    const a = await analysis(root, url, insert);
    assert.deepEqual(a.params.map((p) => [p.name, p.type_name]), [["events", "rows"]]);
    const events = a.params[0]!.type;
    assert.ok(events.kind === "rows");
    assert.deepEqual(events.fields.map((f) => [f.name, f.type.kind === "scalar" ? f.type.name : f.type.kind, f.nullable, f.optional]), [["id", "integer", false, false], ["ts", "text", false, false], ["status", "text", false, true], ["amount", "real", true, true], ["meta", "json", true, true]]);
    const q = await built(root, url, "insert_events", insert);
    assert.deepEqual(plain(await q.run(db, { events: [{ id: 2, ts: "2026-10-20", status: "done", amount: 1.5, meta: { source: "x" } }, { id: 1, ts: "2026-10-20", status: "late", amount: null, meta: null }] })), [{ id: 2, status: "done" }, { id: 1, status: "late" }]);
    assert.deepEqual(plain(await q.run(db, { events: [{ id: 3, ts: "2026-10-21" }] })), [{ id: 3, status: "new" }]);
    await assert.rejects(q.run(db, { events: [{ id: 4, ts: "x", status: null }] }), /"status" is text not null, and the row gives null/);
    await assert.rejects(q.run(db, { events: [{ id: "4", ts: "x" }] }), /"id" is integer, an integer; got "4"/);
    // An explicit null stores NULL, never the text null, and a document stays JSON text.
    assert.deepEqual(db.prepare("select id, status, amount, meta, typeof(meta) as t from events order by id").all().map((r) => ({ ...r })), [
      { id: 1, status: "late", amount: null, meta: null, t: "null" },
      { id: 2, status: "done", amount: 1.5, meta: `{"source":"x"}`, t: "text" },
      { id: 3, status: "new", amount: null, meta: null, t: "null" },
    ]);
    // Rows that leave out different keys go in one insert, each such row with the default.
    assert.deepEqual(plain(await q.run(db, { events: [{ id: 4, ts: "x" }, { id: 5, ts: "x", status: "s" }] })), [{ id: 4, status: "new" }, { id: 5, status: "s" }]);
    // An upsert reads ON CONFLICT as its own, since the expansion ends in WHERE.
    const upsert = await built(root, url, "upsert", "insert into events :events::rows(id integer not null, ts text not null) on conflict (id) do update set ts = excluded.ts returning id, ts");
    assert.deepEqual(plain(await upsert.run(db, { events: [{ id: 3, ts: "2026-10-22" }] })), [{ id: 3, ts: "2026-10-22" }]);
  } finally {
    db.close();
    await closeSessions();
  }
});

test("SQLite: rows that leave out different keys go in one insert, each such row with its column's default copied from the catalog", async () => {
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec(`create table accounts (id integer primary key, name text not null, email text, role text not null default 'member',
    nick text default member, token text not null default (lower(hex(randomblob(8)))), n integer default 7, at text not null default current_timestamp)`);
  const url = "file:./app.db";
  const insert = "insert into accounts :a::rows(id? integer not null, name text not null, email text, role? text not null, nick text, token? text not null, n integer, at? text not null) returning id, role";
  try {
    const file = join(root, "insert_accounts.sql");
    writeFileSync(file, insert);
    writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
    // One statement for every call: no list for the run time to write again.
    const { code } = await sqlModule(file, insert);
    assert.ok(!code.includes("structured:"), code);
    const q = await built(root, url, "insert_accounts", insert);
    assert.match(q.sql, /CASE WHEN json_type\("reylo\.e"\.value, '\$\."role"'\) IS NOT NULL THEN "reylo\.e"\.value ->> '\$\."role"' ELSE \('member'\) END/);
    const rows = [{ name: "Ada" }, { id: 100, name: "Grace", role: "admin", nick: "g", token: "given", n: 1 }, { name: "Linus", email: null, nick: null, n: null }];
    assert.deepEqual(plain(await q.run(db, { a: rows })), [{ id: 1, role: "member" }, { id: 100, role: "admin" }, { id: 101, role: "member" }]);
    const stored = db.prepare("select id, name, email, role, nick, token, n, at, typeof(n) as tn from accounts order by id").all().map((r) => ({ ...r })) as Record<string, unknown>[];
    // A key left out takes the default; a key given as null is NULL; the bare identifier `member` is the string SQLite stores.
    assert.deepEqual(stored.map((r) => [r.id, r.email, r.role, r.nick, r.n, r.tn]), [[1, null, "member", "member", 7, "integer"], [100, null, "admin", "g", 1, "integer"], [101, null, "member", null, null, "null"]]);
    // An expression default runs per row, current_timestamp once per statement, as the database's own.
    assert.equal(stored[1]!.token, "given");
    assert.match(String(stored[0]!.token), /^[0-9a-f]{16}$/);
    assert.match(String(stored[2]!.token), /^[0-9a-f]{16}$/);
    assert.notEqual(stored[0]!.token, stored[2]!.token);
    assert.equal(stored[0]!.at, stored[2]!.at);
    assert.match(String(stored[0]!.at), /^\d{4}-\d\d-\d\d \d\d:\d\d:\d\d$/);
    // One row: the same.
    const one = await built(root, url, "insert_account", "insert into accounts :a::row(name text not null, role? text not null) returning role");
    assert.deepEqual(plain(await one.run(db, { a: { name: "Mary" } })), [{ role: "member" }]);
    assert.deepEqual(plain(await one.run(db, { a: { name: "Ken", role: "owner" } })), [{ role: "owner" }]);
  } finally {
    db.close();
    await closeSessions();
  }
});

test("SQLite: elsewhere a rows parameter is a table named by its alias or by the parameter, a not null key never null and the rest nullable", async () => {
  const root = project();
  new DatabaseSync(join(root, "app.db")).close();
  const url = "file:./app.db";
  const sql = "select e.id, e.note from :events::rows(id integer not null, note text) as e order by e.id";
  try {
    const a = await analysis(root, url, sql);
    assert.deepEqual(a.columns.map((c) => [c.name, c.nullable]), [["id", false], ["note", true]]);
    const q = await built(root, url, "select_events", sql);
    const db = new DatabaseSync(join(root, "app.db"));
    try {
      assert.deepEqual(plain(await q.run(db, { events: [{ id: 2 }, { id: 1, note: "a" }] })), [{ id: 1, note: "a" }, { id: 2, note: null }]);
    } finally {
      db.close();
    }
    const unaliased = await analysis(root, url, "select events.id from :events::rows(id integer not null)");
    assert.deepEqual(unaliased.columns.map((c) => [c.name, c.nullable]), [["id", false]]);
  } finally {
    await closeSessions();
  }
});

test("a column list that does not read is the type check's error at its place, and fails the build", async () => {
  const root = project();
  new DatabaseSync(join(root, "app.db")).close();
  const url = "file:./app.db";
  const cases: [string, string, RegExp][] = [
    ["insert into t :p::rows(a text not null default 'x')", "default", /never restates a default/],
    ["insert into t :p::rows(a? text)", "a?", /may be left out already/],
    ["select * from :p::rows(a? text not null)", "a?", /only where the rows are inserted/],
    ["insert into t :p::row(a text, posts rows(title text))", "rows(title text)", /nested row or rows member is not built yet/],
  ];
  try {
    for (const [sql, at, message] of cases) {
      const a = await analysis(root, url, sql);
      const d = a.diagnostics.find((d) => d.code === 20);
      assert.ok(d, sql);
      assert.match(d.message, message);
      assert.equal(sql.slice(d.start, d.start + d.length), at, sql);
      const file = join(root, "bad.sql");
      writeFileSync(file, sql);
      writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
      await assert.rejects(sqlModule(file, sql), message);
    }
  } finally {
    await closeSessions();
  }
});

test("SQLite: the column list is checked against the table its insert writes, and a disagreement fails the build", async () => {
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table events (id integer primary key, ts text not null, status text not null default 'new', amount real, total real generated always as (amount * 2))");
  db.close();
  const url = "file:./app.db";
  const cases: [string, string, RegExp][] = [
    ["insert into events :e::rows(id integer not null, ts text, status? text not null)", "ts text", /events\.ts is not null, and `ts` lets a row give null or leave the key out: write `ts text not null`/],
    ["insert into events :e::rows(id integer not null, ts text not null, status text)", "status text", /write `status\? text not null`, which lets a row leave the key out so that the default applies/],
    ["insert into events :e::rows(id integer not null, ts text not null, amount? real not null)", "amount", /`amount\?` lets a row leave the key out, but events\.amount has no default/],
    ["insert into events :e::rows(id integer not null, status? text not null)", ":e::rows", /events\.ts is not null and has no default, and the column list leaves it out/],
    ["insert into events :e::rows(id text not null, ts text not null)", "text", /events\.id is INTEGER, and the column list says text: their affinities differ \(TEXT and INTEGER\)/],
    ["insert into events :e::rows(ts text not null, total real)", "total", /events\.total is a generated column/],
  ];
  try {
    for (const [sql, at, message] of cases) {
      const a = await analysis(root, url, sql);
      const d = a.diagnostics.filter((d) => d.code === 21);
      assert.equal(d.length, 1, `${sql}: ${JSON.stringify(a.diagnostics)}`);
      assert.match(d[0]!.message, message);
      assert.equal(sql.slice(d[0]!.start, d[0]!.start + d[0]!.length), at, sql);
      const file = join(root, "bad.sql");
      writeFileSync(file, sql);
      writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
      await assert.rejects(sqlModule(file, sql), message);
    }
    // The rowid is filled when it is left out, so an INTEGER PRIMARY KEY needs no `?`.
    assert.deepEqual((await analysis(root, url, "insert into events :e::rows(ts text not null)")).diagnostics, []);
  } finally {
    await closeSessions();
  }
});

test("Postgres: a rows insert keeps the array's order and exact 64-bit ids, a key left out takes its default, and every driver binds the rows", { skip: pgLive }, async () => {
  const own = await postgresDatabase("reylo_structured");
  const url = new URL(own.url);
  const sql = own.sql;
  const root = project("postgres");
  try {
    await sql.unsafe(`create table events (id int8 primary key, ts timestamptz not null, status text not null default 'new', amount numeric(12,2), meta jsonb);
      create table users (id int8 generated always as identity primary key, email text not null, name text not null)`);
    const insert = `insert into events :events::rows(id int8 not null, ts timestamptz not null, status? text not null, amount numeric(12,2), meta jsonb('{"type": "object"}')) returning id, status, amount`;
    const a = await analysis(root, url.href, insert);
    const events = a.params[0]!.type;
    assert.ok(events.kind === "rows");
    assert.deepEqual(events.fields.map((f) => [f.name, f.type.kind === "scalar" ? f.type.name : f.type.kind, f.written]), [["id", "int8", "int8"], ["ts", "timestamptz", "timestamptz"], ["status", "text", "text"], ["amount", "numeric", "numeric(12,2)"], ["meta", "json", "jsonb"]]);
    assert.deepEqual(a.columns.map((c) => [c.name, c.nullable]), [["id", false], ["status", false], ["amount", true]]);
    const q = await built(root, url.href, "insert_events", insert);
    // postgres.js serializes a JSON parameter itself; pg takes its text.
    assert.deepEqual(plain(await q.run(sql, { events: [{ id: 9007199254740993n, ts: "2026-10-20T10:00:00Z", status: "done", amount: "1.50", meta: { a: 1 } }] })), [{ id: 9007199254740993n, status: "done", amount: "1.50" }]);
    const pg = driver("pg");
    const client = new pg.Client(url.href);
    await client.connect();
    try {
      assert.deepEqual(plain(await q.run(client, { events: [{ id: 2, ts: "2026-10-20 12:00:00+02" }] })), [{ id: 2n, status: "new", amount: null }]);
      await assert.rejects(q.run(client, { events: [{ id: 3, ts: "2026-10-20T10:00:00Z", amount: "1.234" }] }), /would be rounded/);
    } finally {
      await client.end();
    }
    // The array's order is the insert's, and RETURNING follows it.
    const users = await built(root, url.href, "insert_users", "insert into users :users::rows(email text not null, name text not null) returning id, email");
    const many = Array.from({ length: 1000 }, (_, i) => ({ email: `u${i}@x`, name: `n${1000 - i}` }));
    const returned = await users.run(sql, { users: many });
    assert.deepEqual(returned.map((r: { email: string }) => r.email), many.map((u) => u.email));
    assert.deepEqual(returned.map((r: { id: bigint }) => r.id), many.map((_, i) => BigInt(i + 1)));
    // One row.
    const user = await built(root, url.href, "insert_user", "insert into users :user::row(email text not null, name text not null) returning id");
    assert.deepEqual(plain(await user.run(sql, { user: { email: "a@x", name: "Ada" } })), [{ id: 1001n }]);
    // Elsewhere, a table whose not null keys are never null.
    const read = "select e.id, e.amount from :events::rows(id int8 not null, amount numeric) as e order by e.id";
    assert.deepEqual((await analysis(root, url.href, read)).columns.map((c) => [c.name, c.nullable]), [["id", false], ["amount", true]]);
    const r = await built(root, url.href, "read_events", read);
    assert.deepEqual(plain(await r.run(sql, { events: [{ id: 2 }, { id: 1, amount: 3 }] })), [{ id: 1n, amount: "3" }, { id: 2n, amount: null }]);
    // A typed JSON parameter: Postgres receives `$1::jsonb`, and the run time fills its defaults and checks it.
    const typed = await built(root, url.href, "typed_users", `insert into users (email, name)
select x.email, x.name from jsonb_to_recordset(:users::jsonb('{"type": "array", "items": {"type": "object", "properties": {"email": {"type": "string", "format": "email"}, "name": {"type": "string", "default": "anon"}}, "required": ["email", "name"]}}')) as x(email text, name text)
returning email, name`);
    assert.deepEqual(plain(await typed.run(sql, { users: [{ email: "b@x" }, { email: "c@x", name: "Cy" }] })), [{ email: "b@x", name: "anon" }, { email: "c@x", name: "Cy" }]);
    await assert.rejects(typed.run(sql, { users: [{ email: "nope" }] }), /Parameter "users" at \/0\/email must be a email/);
    // Binary data: Postgres's reader takes bytea from its own \\x text.
    await sql.unsafe("create table files (id int8 primary key, data bytea not null)");
    const files = await built(root, url.href, "insert_files", "insert into files :files::rows(id int8 not null, data bytea not null) returning data");
    const [file] = await files.run(sql, { files: [{ id: 1, data: new Uint8Array([0, 1, 255]) }] });
    assert.deepEqual([...file.data], [0, 1, 255]);
    // A type the database does not know is an error at the member's type.
    const bad = "insert into events :events::rows(id int8 not null, ts timestamptz not null, amount numerc)";
    const d = (await analysis(root, url.href, bad)).diagnostics;
    assert.deepEqual(d.map((x) => [x.message, bad.slice(x.start, x.start + x.length)]), [[`type "numerc" does not exist (42704)`, "numerc"]]);
    // A member's type is its column's, modifiers included; an identity GENERATED ALWAYS is the database's to number.
    const checks: [string, string, RegExp][] = [
      ["insert into events :e::rows(id numeric not null, ts timestamptz not null)", "numeric", /events\.id is bigint, and the column list says numeric/],
      ["insert into events :e::rows(id int8 not null, ts timestamptz not null, amount numeric)", "numeric", /events\.amount is numeric\(12,2\), and the column list says numeric/],
      ["insert into events :e::rows(id int8 not null, ts timestamp not null)", "timestamp", /events\.ts is timestamp with time zone, and the column list says timestamp/],
      ["insert into users :u::rows(id int8 not null, email text not null, name text not null)", "id", /users\.id is GENERATED ALWAYS AS IDENTITY/],
    ];
    for (const [sql, at, message] of checks) {
      const found = (await analysis(root, url.href, sql)).diagnostics.filter((x) => x.code === 21);
      assert.equal(found.length, 1, `${sql}: ${JSON.stringify(found)}`);
      assert.match(found[0]!.message, message);
      assert.equal(sql.slice(found[0]!.start, found[0]!.start + found[0]!.length), at, sql);
    }
    // Aliases of one type are the type: bigint is int8, timestamp with time zone is timestamptz.
    assert.deepEqual((await analysis(root, url.href, "insert into events :e::rows(id bigint not null, ts timestamp with time zone not null, amount numeric(12, 2))")).diagnostics, []);
  } finally {
    await closeSessions();
    await own.drop();
  }
});

test("Postgres: rows that leave out different keys go in one insert, each such row with its column's default copied from the catalog, a sequence advancing only for those rows", { skip: pgLive }, async () => {
  const require = createRequire(join(DRIVERS, "postgres", "package.json"));
  const postgres = require("postgres");
  const admin = postgres(PG, { max: 1, onnotice: () => {} });
  const name = `reylo_structured_mixed_${process.pid}`;
  await admin.unsafe(`drop database if exists ${name}`);
  await admin.unsafe(`create database ${name}`);
  const url = new URL(PG!);
  url.pathname = `/${name}`;
  // A zone with a daylight-saving gap, where a timestamp read through timestamptz moves.
  const sql = postgres(url.href, { max: 1, onnotice: () => {}, connection: { TimeZone: "America/New_York" } });
  const root = project("postgres");
  try {
    await sql.unsafe(`create sequence ext_seq;
      create domain dom_t as text default 'from domain';
      create table accounts (
        id int8 generated by default as identity primary key,
        n int8 not null default nextval('ext_seq'),
        name text not null,
        email text,
        role text not null default 'member',
        owner text not null default current_user,
        tag text not null default concat('t-', gen_random_uuid()),
        created timestamp not null default now(),
        dom dom_t)`);
    const insert = "insert into accounts :a::rows(id? int8 not null, n? int8 not null, name text not null, email text, role? text not null, owner? text not null, tag? text not null, created? timestamp not null, dom text) returning id, role";
    const file = join(root, "insert_accounts.sql");
    writeFileSync(file, insert);
    writeFileSync(join(root, ".env"), `DATABASE_URL=${url.href}\n`);
    const { code } = await sqlModule(file, insert);
    assert.ok(!code.includes("structured:"), code);
    const q = await built(root, url.href, "insert_accounts", insert);
    // Each default is cast to its column's type, so the CASE keeps a given value as it is.
    assert.match(q.sql, /ELSE \(CURRENT_USER\)::text END/);
    assert.match(q.sql, /ELSE \(now\(\)\)::timestamp without time zone END/);
    const rows = [
      { name: "Ada" },
      { id: 100, n: 500, name: "Grace", role: "admin", owner: "o".repeat(100), tag: "given", created: "2026-03-08 02:30:00", dom: "given" },
      { name: "Linus", email: null, dom: null },
    ];
    assert.deepEqual(plain(await q.run(sql, { a: rows })), [{ id: 1n, role: "member" }, { id: 100n, role: "admin" }, { id: 2n, role: "member" }]);
    const stored = await sql.unsafe("select id::text, n::text, email, role, owner, length(owner) as owner_length, tag, created::text as created, dom, current_user::text as me from accounts order by accounts.id");
    const me = stored[0].me;
    assert.deepEqual(
      stored.map((r: Record<string, unknown>) => [r.id, r.n, r.email, r.role, r.owner_length, r.dom]),
      [["1", "1", null, "member", me.length, "from domain"], ["2", "2", null, "member", me.length, null], ["100", "500", null, "admin", 100, "given"]],
    );
    assert.equal(stored[0].owner, me);
    // The sequences moved only for the rows that left their keys out.
    assert.deepEqual(plain(await sql.unsafe("select last_value::text from ext_seq")), [{ last_value: "2" }]);
    // An expression default runs per row; the given timestamp is stored as given, in a gap of the session's zone.
    assert.match(stored[0].tag, /^t-[0-9a-f-]{36}$/);
    assert.match(stored[1].tag, /^t-[0-9a-f-]{36}$/);
    assert.notEqual(stored[0].tag, stored[1].tag);
    assert.equal(stored[2].tag, "given");
    assert.equal(stored[2].created, "2026-03-08 02:30:00");
    // node-postgres binds the same statement.
    const pg = require("pg");
    const client = new pg.Client(url.href);
    await client.connect();
    try {
      assert.deepEqual(plain(await q.run(client, { a: [{ name: "Mary", role: "owner" }, { name: "Ken" }] })), [{ id: 3n, role: "owner" }, { id: 4n, role: "member" }]);
    } finally {
      await client.end();
    }
    // One row reads the object itself for its key tests.
    const one = await built(root, url.href, "insert_account", "insert into accounts :a::row(name text not null, role? text not null) returning role");
    assert.deepEqual(plain(await one.run(sql, { a: { name: "Joan" } })), [{ role: "member" }]);
    assert.deepEqual(plain(await one.run(sql, { a: { name: "Alan", role: "admin" } })), [{ role: "admin" }]);
    // A view's column with no default of its own would take its table's, which is not copied.
    await sql.unsafe("create view accounts_v as select id, name, email from accounts");
    const view = "insert into accounts_v :a::rows(name text not null, email text)";
    const found = (await analysis(root, url.href, view)).diagnostics.filter((x) => x.code === 21);
    assert.deepEqual(found.map((x) => [x.message, view.slice(x.start, x.start + x.length)]), [["accounts_v.email is a column of a view with no default of its own, so a row that leaves `email` out would take the default of the view's table, which Reylo does not copy into the insert: give the view's column a default (ALTER VIEW accounts_v ALTER COLUMN email SET DEFAULT …), or write the member without `?` and with `not null`.", "email"]]);
  } finally {
    await closeSessions();
    await sql.end();
    await admin.unsafe(`drop database if exists ${name}`);
    await admin.end();
  }
});

test("MySQL: a rows insert reads each column with ERROR ON ERROR, keeps a 64-bit id exact and gives a key left out its default", { skip: mysqlLive }, async () => {
  const require = createRequire(join(DRIVERS, "mysql2", "package.json"));
  const mysql = require("mysql2/promise");
  const server = new URL(MYSQL!);
  const admin = await mysql.createConnection({ host: server.hostname, port: Number(server.port), user: decodeURIComponent(server.username) || "root", password: decodeURIComponent(server.password) });
  const name = `reylo_structured_${process.pid}`;
  await admin.query(`drop database if exists ${name}`);
  await admin.query(`create database ${name}`);
  const url = new URL(MYSQL!);
  url.pathname = `/${name}`;
  const db = await mysql.createConnection({ host: server.hostname, port: Number(server.port), user: decodeURIComponent(server.username) || "root", password: decodeURIComponent(server.password), database: name });
  const root = project("mysql2");
  try {
    await db.query("create table events (id bigint primary key, ts datetime(6) not null, status varchar(32) not null default 'new', amount decimal(12,2), meta json)");
    const insert = `insert into events :events::rows(id bigint not null, ts datetime(6) not null, status? varchar(32) not null, amount decimal(12,2), meta json('{"type": "object"}'))`;
    const a = await analysis(root, url.href, insert);
    const events = a.params[0]!.type;
    assert.ok(events.kind === "rows");
    assert.deepEqual(events.fields.map((f) => [f.name, f.type.kind === "scalar" ? f.type.name : f.type.kind]), [["id", "int8"], ["ts", "timestamp"], ["status", "text"], ["amount", "numeric"], ["meta", "json"]]);
    const q = await built(root, url.href, "insert_events", insert);
    await q.run(db, { events: [{ id: 9223372036854775807n, ts: "2026-10-20 10:00:00", amount: "1.50", meta: { a: 1 } }, { id: 1, ts: "2026-10-20 11:00:00", amount: null, meta: null }] });
    await assert.rejects(q.run(db, { events: [{ id: 2, ts: "2026-10-20 10:00:00", status: "x".repeat(33) }] }), /at most 32 characters/);
    // MySQL copies no default: the run time writes the lists for the keys a call carries, the same in every row.
    await assert.rejects(q.run(db, { events: [{ id: 3, ts: "2026-10-20 10:00:00", status: "done" }, { id: 4, ts: "2026-10-20 10:00:00" }] }), /row 2 leaves out "status", which row 1 gives\. Every row of one call gives the same keys/);
    const mismatch = "insert into events :e::rows(id bigint not null, ts datetime not null)";
    const found = (await analysis(root, url.href, mismatch)).diagnostics.filter((x) => x.code === 21);
    assert.deepEqual(found.map((x) => [x.message, mismatch.slice(x.start, x.start + x.length)]), [["events.ts is datetime(6), and the column list says datetime: a column list writes each column's own type, so that the checks the run time makes are the column's.", "datetime"]]);
    assert.deepEqual((await analysis(root, url.href, "insert into events :e::rows(id bigint not null, ts datetime(6) not null)")).diagnostics, []);
    // A typed JSON parameter: MySQL receives `cast(? as json)`.
    const typed = await built(root, url.href, "typed_events", `select j.id from json_table(:ids::json('{"type": "array", "items": {"type": "integer", "minimum": 1}}'), '$[*]' columns (id bigint path '$' error on error)) as j order by j.id`);
    // A BIGINT is a BigInt (src/values.ts).
    assert.deepEqual(plain(await typed.run(db, { ids: [3, 1] })), [{ id: 1n }, { id: 3n }]);
    await assert.rejects(typed.run(db, { ids: [0] }), /Parameter "ids" at \/0 must be at least 1/);
    const [rows] = await db.query("select cast(id as char) as id, status, amount, meta from events order by id");
    assert.deepEqual(plain(rows), [
      { id: "1", status: "new", amount: null, meta: null },
      { id: "9223372036854775807", status: "new", amount: "1.50", meta: { a: 1 } },
    ]);
  } finally {
    await closeSessions();
    await db.end();
    await admin.query(`drop database if exists ${name}`);
    await admin.end();
  }
});

test("an imported query carries its input's and its output's JSON Schema, a structured parameter's keys included", async () => {
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table events (id integer primary key, ts text not null, status text not null default 'new', amount real)");
  db.close();
  try {
    const q = await built(root, "file:./app.db", "insert_events", "insert into events :events::rows(id integer not null, ts text not null, status? text not null, amount real) returning id, status");
    assert.deepEqual(q.inputSchema, {
      $schema: "https://json-schema.org/draft/2020-12/schema",
      type: "object",
      properties: {
        events: {
          type: "array",
          items: {
            type: "object",
            properties: { id: { type: "integer", format: "int64" }, ts: { type: "string" }, status: { type: "string" }, amount: { anyOf: [{ type: "number", format: "double" }, { type: "null" }] } },
            required: ["id", "ts"],
            additionalProperties: false,
          },
        },
      },
      required: ["events"],
      additionalProperties: false,
    });
    assert.deepEqual(q.outputSchema, {
      $schema: "https://json-schema.org/draft/2020-12/schema",
      type: "array",
      items: { type: "object", properties: { id: { type: "integer", format: "int64" }, status: { type: "string" } }, required: ["id", "status"] },
    });
    // Standard Schema's converter gives the same.
    assert.deepEqual(q["~standard"].jsonSchema.input({ target: "draft-2020-12" }), q.inputSchema);
  } finally {
    await closeSessions();
  }
});

test("SQLite: a typed JSON parameter is typed by its schema, takes its defaults, and is checked before the statement runs", async () => {
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table todos (id integer primary key, title text not null, done integer not null default 0)");
  const url = "file:./app.db";
  const insert = `insert into todos (title, done)
select value ->> '$.title', value ->> '$.done' from json_each(:todos::json('{
  "type": "array",
  "maxItems": 2,
  "items": {
    "type": "object",
    "properties": { "title": { "type": "string", "minLength": 1 }, "done": { "type": "boolean", "default": false } },
    "required": ["title", "done"],
    "additionalProperties": false
  }
}')) returning id, title, done`;
  try {
    const a = await analysis(root, url, insert);
    const p = a.params[0]!;
    assert.deepEqual([p.name, p.type_name, p.type.kind], ["todos", "json", "json"]);
    // `done` has a default, so the caller may leave it out; the run time fills it.
    assert.deepEqual(p.type.kind === "json" && p.type.schema && (p.type.schema as { items: { required: string[] } }).items.required, ["title"]);
    const q = await built(root, url, "insert_todos", insert);
    assert.deepEqual(plain(await q.run(db, { todos: [{ title: "a" }, { title: "b", done: true }] })), [{ id: 1, title: "a", done: 0 }, { id: 2, title: "b", done: 1 }]);
    await assert.rejects(q.run(db, { todos: [{ title: "" }] }), /Parameter "todos" at \/0\/title must be at least 1 characters long/);
    await assert.rejects(q.run(db, { todos: [{ title: "a", extra: 1 }] }), /at \/0\/extra is a key the schema does not allow/);
    await assert.rejects(q.run(db, { todos: [{ title: "a" }, { title: "b" }, { title: "c" }] }), /must have at most 2 items/);
    // A parameter whose schema has a default at its root may be left out.
    const limited = await built(root, url, "limited", `select id from todos order by id limit :n::json('{ "type": "integer", "minimum": 1, "maximum": 200, "default": 1 }')`);
    assert.deepEqual(plain(await limited.run(db, {})), [{ id: 1 }]);
    assert.deepEqual(plain(await limited.run(db, { n: 2 })), [{ id: 1 }, { id: 2 }]);
    await assert.rejects(limited.run(db, { n: 0 }), /Parameter "n" must be at least 1/);
    assert.deepEqual(limited.inputSchema.required, []);
    // What the profile refuses is the type check's error at the schema, and fails the build.
    const bad = `select * from json_each(:p::json('{ "$ref": "#/$defs/x" }'))`;
    const d = (await analysis(root, url, bad)).diagnostics.find((x) => x.code === 20);
    assert.ok(d);
    assert.match(d.message, /breaks the schema's locality/);
    assert.equal(bad.slice(d.start, d.start + d.length), `'{ "$ref": "#/$defs/x" }'`);
  } finally {
    db.close();
    await closeSessions();
  }
});

test("SQLite: a JSON member of a column list is checked against its schema, its nested defaults filled", async () => {
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table events (id integer primary key, meta text)");
  try {
    const q = await built(root, "file:./app.db", "insert_events", `insert into events :events::rows(id integer not null, meta json('{"type": "object", "properties": {"source": {"type": "string", "default": "web"}, "n": {"type": "integer", "minimum": 0}}, "required": ["source"]}')) returning id, meta`);
    assert.deepEqual(plain(await q.run(db, { events: [{ id: 1, meta: {} }, { id: 2, meta: { source: "app", n: 3 } }] })), [{ id: 1, meta: `{"source":"web"}` }, { id: 2, meta: `{"source":"app","n":3}` }]);
    await assert.rejects(q.run(db, { events: [{ id: 3, meta: { n: -1 } }] }), /row 1: "meta" at \/n must be at least 0/);
  } finally {
    db.close();
    await closeSessions();
  }
});

test("binary data in a column list travels as hexadecimal text, which each reader decodes", async () => {
  const bytes = new Uint8Array([0, 1, 127, 128, 255]);
  const same = (got: unknown) => assert.deepEqual([...new Uint8Array(got as ArrayLike<number>)], [...bytes]);
  // SQLite decodes it with unhex().
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table files (id integer primary key, data blob not null)");
  try {
    const insert = await built(root, "file:./app.db", "insert_files", "insert into files :files::rows(id integer not null, data blob not null) returning id, data");
    const [row] = await insert.run(db, { files: [{ id: 1, data: bytes }] });
    same(row.data);
    const read = await built(root, "file:./app.db", "read_files", "select f.data from :files::rows(data blob not null) as f");
    same((await read.run(db, { files: [{ data: bytes }] }))[0].data);
    await assert.rejects(insert.run(db, { files: [{ id: 2, data: "00ff" }] }), /"data" is blob, binary data, a Uint8Array; got "00ff"/);
  } finally {
    db.close();
    await closeSessions();
  }
  // MySQL decodes it with UNHEX().
  if (mysqlLive) return;
  const own = await mysqlDatabase("reylo_binary");
  const url = new URL(own.url);
  const my = own.db;
  const mroot = project("mysql2");
  try {
    await my.query("create table files (id bigint primary key, data varbinary(16) not null)");
    const insert = await built(mroot, url.href, "insert_files", "insert into files :files::rows(id bigint not null, data varbinary(16) not null)");
    await insert.run(my, { files: [{ id: 1, data: bytes }] });
    const [rows] = await my.query("select data from files");
    same((rows as { data: Uint8Array }[])[0]!.data);
    const read = await built(mroot, url.href, "read_files", "select f.data from :files::rows(data varbinary(16) not null) as f");
    same((await read.run(my, { files: [{ data: bytes }] }))[0].data);
  } finally {
    await closeSessions();
    await own.drop();
  }
});
test("MySQL: a rows table's text compares with a column of any collation, a literal and a parameter, and FOR UPDATE and LATERAL take it", { skip: mysqlLive }, async () => {
  const my = await mysqlDatabase("reylo_positions");
  const root = project("mysql2");
  try {
    // t has the database's collation, u another one; mysql2 connects with utf8mb4_unicode_ci,
    // which a JSON_TABLE's own text column takes (docs/structured-params.md, Positions).
    await my.db.query("create table t (id bigint primary key, x varchar(32) not null)");
    await my.db.query("create table u (id bigint primary key, x varchar(32) collate utf8mb4_general_ci not null)");
    await my.db.query("insert into t values (1, 'a'), (2, 'b'), (3, 'c')");
    await my.db.query("insert into u values (1, 'a'), (2, 'b')");
    const rows = "id bigint not null, x varchar(32) not null";
    const p = [{ id: 9, x: "b" }, { id: 8, x: "c" }];
    const cases: [string, Record<string, unknown>, object[]][] = [
      [`select t.id from t join :p::rows(${rows}) r on r.x = t.x order by t.id`, { p }, [{ id: 2n }, { id: 3n }]],
      [`select u.id from u join :p::rows(${rows}) r on r.x = u.x order by u.id`, { p }, [{ id: 2n }]],
      [`select t.id from t where t.x in (select x from :p::rows(x varchar(32) not null)) order by t.id`, { p: [{ x: "c" }] }, [{ id: 3n }]],
      [`select r.id from :p::rows(${rows}) r where r.x = 'b'`, { p }, [{ id: 9n }]],
      [`select r.id from :p::rows(${rows}) r where r.x = :q`, { p, q: "b" }, [{ id: 9n }]],
      [`select x from t union select x from :p::rows(x varchar(32) not null) order by x`, { p: [{ x: "z" }] }, [{ x: "a" }, { x: "b" }, { x: "c" }, { x: "z" }]],
      [`with r as (select * from :p::rows(${rows})) select t.id from r a join r b on b.x = a.x join t on t.x = a.x order by t.id`, { p }, [{ id: 2n }, { id: 3n }]],
      [`select t.id from t join :p::rows(id bigint not null) r on r.id = t.id order by t.id for update`, { p: [{ id: 2 }] }, [{ id: 2n }]],
      [`select t.id, r.x from t cross join lateral :p::rows(${rows}) r where r.id = t.id order by t.id`, { p: [{ id: 3, x: "C" }] }, [{ id: 3n, x: "C" }]],
    ];
    let n = 0;
    for (const [sql, params, want] of cases) {
      const q = await built(root, my.url, `text_${n++}`, sql);
      assert.deepEqual(plain(await q.run(my.db, params)), want, sql);
    }
  } finally {
    await closeSessions();
    await my.drop();
  }
});

test("MySQL: an insert's rows take the row alias written after the column list, which ON DUPLICATE KEY UPDATE reads", { skip: mysqlLive }, async () => {
  const my = await mysqlDatabase("reylo_row_alias");
  const root = project("mysql2");
  try {
    await my.db.query("create table events (id bigint primary key, x varchar(32) not null, n int)");
    await my.db.query("insert into events values (1, 'a', 10), (2, 'b', 20)");
    const upsert = await built(root, my.url, "upsert", "insert into events :events::rows(id bigint not null, x varchar(32) not null) as e on duplicate key update x = e.x");
    await upsert.run(my.db, { events: [{ id: 2, x: "B" }, { id: 4, x: "d" }] });
    const one = await built(root, my.url, "upsert_one", "insert into events :e::row(id bigint not null, x varchar(32) not null) as new on duplicate key update x = new.x");
    await one.run(my.db, { e: { id: 1, x: "A" } });
    assert.deepEqual(await my.rows("select id, x, n from events order by id"), [{ id: 1, x: "A", n: 10 }, { id: 2, x: "B", n: 20 }, { id: 4, x: "d", n: null }]);
  } finally {
    await closeSessions();
    await my.drop();
  }
});

test("an insert takes the table's alias before its rows, and refuses a column list of its own at the author's text", async () => {
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table t (id integer primary key, x text not null)");
  db.exec("insert into t values (1, 'a'), (2, 'b')");
  const url = "file:./app.db";
  try {
    const upsert = await built(root, url, "aliased", "insert into t as tt :p::rows(id integer not null, x text not null) on conflict (id) do update set x = excluded.x where tt.x <> excluded.x returning id, x");
    assert.deepEqual(plain(await upsert.run(db, { p: [{ id: 2, x: "B" }, { id: 3, x: "c" }] })), [{ id: 2, x: "B" }, { id: 3, x: "c" }]);
    const own = "insert into t (id, x) :p::rows(id integer not null, x text not null)";
    const d = (await analysis(root, url, own)).diagnostics.find((x) => x.code === 20);
    assert.ok(d);
    assert.match(d.message, /writes the insert's column list itself/);
    assert.equal(own.slice(d.start, d.start + d.length), "(id, x)");
    writeFileSync(join(root, "own.sql"), own);
    await assert.rejects(sqlModule(join(root, "own.sql"), own), /writes the insert's column list itself/);
  } finally {
    db.close();
    await closeSessions();
  }
});

test("Postgres: an insert takes the table's alias before its rows", { skip: pgLive }, async () => {
  const own = await postgresDatabase("reylo_insert_alias");
  const url = new URL(own.url);
  const sql = own.sql;
  const root = project("postgres");
  try {
    await sql.unsafe("create table t (id int8 primary key, x text not null); insert into t values (1, 'a'), (2, 'b')");
    const upsert = await built(root, url.href, "aliased", "insert into t as tt :p::rows(id int8 not null, x text not null) on conflict (id) do update set x = excluded.x where tt.x <> excluded.x returning id, x");
    assert.deepEqual(plain(await upsert.run(sql, { p: [{ id: 2, x: "B" }, { id: 3, x: "c" }] })), [{ id: 2n, x: "B" }, { id: 3n, x: "c" }]);
  } finally {
    await closeSessions();
    await own.drop();
  }
});
