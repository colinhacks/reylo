import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { transform, DRIVERS, PG, MYSQL, pgLive, mysqlLive } from "./helpers.ts";

// A parameter nothing in the statement types is error reylo12 in the type check unless the project
// file says `strict_params: false`. The content
// mapper runs as the type check runs it, through its own JSON-RPC process. Postgres and MySQL run
// on throwaway servers (REYLO_TEST_PG, REYLO_TEST_MYSQL), each in a database of its own.

function project(url: string, reylo: string | undefined, driver?: string, name = ".reylo.yaml"): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-strict-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  if (reylo !== undefined) writeFileSync(join(root, name), reylo);
  if (driver) {
    mkdirSync(join(root, "node_modules"));
    symlinkSync(join(DRIVERS, driver), join(root, "node_modules", driver));
  }
  return root;
}

const message = (written: string, file = ".reylo.yaml") => `Nothing in the statement gives the parameter ${written} a type; cast it, as in ${written}::text, or set strict_params: false in ${file}.`;

/** On MySQL every `::` cast types its parameter, through the server's describe of the cast. */
const CASTS = {
  "casts.sql": "select :a::int as a, :b::bigint unsigned as b, :c::numeric(10,2) as c, :d::double precision as d, :e::date as e, :f::time as f, :g::datetime as g, :h::varbinary(4) as h, :i::varchar(5) as i, :j::uuid as j from users limit :n::int\n",
  "where.sql": "select name from users where id = :id::bigint and name like concat(:prefix::text, '%')\n",
};
const CAST_TYPES = /export type Params = \{ a: number; b: number; c: string; d: number; e: string; f: string; g: string; h: Uint8Array; i: string; j: string; n: number \}/;

/** A JSON cast without a schema types its parameter as JSON, which is no `unknown`, so it is never flagged; the TypeScript type of any JSON value is `unknown` all the same. */
async function jsonCast(url: string, json: string, driver?: string) {
  const out = await transform(project(url, undefined, driver), { "json.sql": json, "cast.sql": "select :x::text as x\n" });
  assert.deepEqual(out["json.sql"]!.diagnostics, []);
  assert.deepEqual(out["cast.sql"]!.diagnostics, []);
}

test("SQLite: a parameter nothing types is an error by default, and strict_params: false keeps it unknown", async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-strict-db-"));
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key)");
  db.close();
  const url = `file:${join(root, "app.db")}`;
  // SQLite types no bare parameter in a select list: by default each is an error at its first use.
  const sql = { "bare.sql": "select :x as x, :y as y\n" };
  const strict = await transform(project(url, undefined), sql);
  assert.deepEqual(strict["bare.sql"]!.diagnostics, [
    { messageText: message(":x"), start: 7, length: 2, code: 12 },
    { messageText: message(":y"), start: 16, length: 2, code: 12 },
  ]);
  // A project file without the key takes the default too, and so does `strict_params: true`.
  for (const file of ["env: [.env]\n", "strict_params: true\n"]) {
    assert.deepEqual((await transform(project(url, file), sql))["bare.sql"]!.diagnostics.map((d) => d.code), [12, 12], file);
  }
  // A project whose file is `.reylo.yml` is told to set the key there, not in a `.reylo.yaml` it lacks.
  const yml = await transform(project(url, "strict_params: true\n", undefined, ".reylo.yml"), sql);
  assert.deepEqual(yml["bare.sql"]!.diagnostics.map((d) => d.messageText), [message(":x", ".reylo.yml"), message(":y", ".reylo.yml")]);
  // `strict_params: false` keeps such a parameter `unknown`, with no error.
  const loose = await transform(project(url, "strict_params: false\n"), sql);
  assert.deepEqual(loose["bare.sql"]!.diagnostics, []);
  assert.match(loose["bare.sql"]!.text, /export type Params = \{ x: unknown; y: unknown \}/);
  // A statement the engine refused keeps its own error alone: a parameter written twice is untyped
  // because the analysis stopped there.
  const repeated = await transform(project(url, undefined), { "repeated.sql": "select :q as a, :q as b\n" });
  assert.deepEqual(repeated["repeated.sql"]!.diagnostics.map((d) => d.code), [3]);
  // The cast the message suggests types it.
  assert.deepEqual((await transform(project(url, undefined), { "cast.sql": "select :x::text as x\n" }))["cast.sql"]!.diagnostics, []);
  await jsonCast(url, "select json_extract(:doc::json, '$.a') as a\n");
});

test("SQLite: a parameter as a whole sort or group key is error reylo17 at the parameter, and the statement is typed all the same", async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-strict-db-"));
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null)");
  db.close();
  const url = `file:${join(root, "app.db")}`;
  const sorted = "select id, name from users order by :sort::text\n";
  const grouped = "select count(*) as n from users group by :g\n";
  const chosen = "select id, name from users order by case :sort::text when 'name' then name end, id\n";
  const out = await transform(project(url, undefined), { "sorted.sql": sorted, "grouped.sql": grouped, "chosen.sql": chosen });
  const why = "since a parameter is a value and not a column's name; choose the column with";
  assert.deepEqual(out["sorted.sql"]!.diagnostics, [{ messageText: `order by :sort sorts every row by the same value, ${why} case :sort when 'name' then name end`, start: sorted.indexOf(":sort"), length: ":sort::text".length, code: 17 }]);
  assert.match(out["sorted.sql"]!.text, /export type Params = \{ sort: string \}/);
  assert.match(out["sorted.sql"]!.text, /export type Row = \{ id: number; name: string \}/);
  // A parameter nothing else types stays unknown with no second error: the key is the one to fix.
  assert.deepEqual(out["grouped.sql"]!.diagnostics.map((d) => [d.code, d.start, d.messageText]), [[17, grouped.indexOf(":g"), `group by :g puts every row in one group, ${why} case :g when 'name' then name end`]]);
  assert.match(out["grouped.sql"]!.text, /export type Row = \{ n: number \}/);
  // The case the message suggests chooses the column with the parameter.
  assert.deepEqual(out["chosen.sql"]!.diagnostics, []);
});

test("Postgres: a JSON cast without a schema is no untyped parameter", { skip: pgLive }, async () => {
  const require = createRequire(join(DRIVERS, "postgres", "package.json"));
  const postgres = require("postgres");
  const admin = postgres(PG!, { max: 1, onnotice: () => {} });
  const name = `reylo_strict_${process.pid}`;
  await admin.unsafe(`drop database if exists ${name}`);
  await admin.unsafe(`create database ${name}`);
  const url = new URL(PG!);
  url.pathname = `/${name}`;
  try {
    await jsonCast(url.href, "select :doc::jsonb ->> 'a' as a, :plain::json as plain\n", "postgres");
  } finally {
    await admin.unsafe(`drop database if exists ${name} with (force)`);
    await admin.end();
  }
});

test("MySQL: a JSON cast without a schema is no untyped parameter, and every cast types its parameter", { skip: mysqlLive }, async () => {
  const require = createRequire(join(DRIVERS, "mysql2", "package.json"));
  const mysql = require("mysql2/promise");
  const server = new URL(MYSQL!);
  const admin = await mysql.createConnection({ host: server.hostname, port: Number(server.port), user: decodeURIComponent(server.username) || "root", password: decodeURIComponent(server.password) });
  const name = `reylo_strict_${process.pid}`;
  await admin.query(`drop database if exists ${name}`);
  await admin.query(`create database ${name}`);
  const url = new URL(MYSQL!);
  url.pathname = `/${name}`;
  try {
    await jsonCast(url.href, "select json_extract(:doc::json, '$.a') as a\n", "mysql2");
    await admin.query(`create table ${name}.users (id bigint primary key, name varchar(50) not null)`);
    const casts = await transform(project(url.href, undefined, "mysql2"), CASTS);
    for (const file of Object.keys(CASTS)) assert.deepEqual(casts[file]!.diagnostics, [], file);
    assert.match(casts["casts.sql"]!.text, CAST_TYPES);
  } finally {
    await admin.query(`drop database if exists ${name}`);
    await admin.end();
  }
});
