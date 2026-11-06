import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { sqlModule } from "../src/loader.ts";
import { defineQuery } from "../src/index.ts";
import { closeSessions, driver, DRIVERS, INDEX, mysqlDatabase, mysqlLive, pgLive, postgresDatabase } from "./helpers.ts";

// A row's keys are the columns its type declares, whatever a driver or the database makes of a column with no alias. Such a column takes its name from the text it runs as on SQLite and MySQL, so the name depends on what Reylo sends and on who writes it.

function project(url: string, drivers: string[] = []): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-names-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  for (const name of drivers) {
    const to = join(root, "node_modules", name);
    mkdirSync(dirname(to), { recursive: true });
    symlinkSync(join(DRIVERS, name), to);
  }
  return root;
}

let files = 0;
/** The module a build step writes for `raw`, imported, and the columns it declares. */
async function built(root: string, raw: string) {
  const name = `q${files++}`;
  const file = join(root, `${name}.sql`);
  writeFileSync(file, raw);
  const { code } = await sqlModule(file, raw);
  const out = join(root, `${name}.mjs`);
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  const query = (await import(pathToFileURL(out).href)).default;
  return { query, columns: query["~standard"].query.columns.map((c: { name: string }) => c.name) as string[] };
}

/** One row as a list of its values under the columns declared, for a statement `.run()` gives rows of. */
function row(value: unknown): Record<string, unknown> {
  return { ...(Array.isArray(value) ? value[0] : value) };
}

const sorted = (names: string[]) => [...names].sort();

/** Statements whose every column is named by its text, each with its parameters, the names the type must declare and the row. */
const SQLITE: [string, object | undefined, string[], Record<string, unknown>][] = [
  ["select 1 + 1", undefined, ["1 + 1"], { "1 + 1": 2 }],
  ["select 1 + 1;\n", undefined, ["1 + 1"], { "1 + 1": 2 }],
  ["select a, b || 'y' from t", undefined, ["a", "b || 'y'"], { a: 1, "b || 'y'": "xy" }],
  ["select (select max(a) from t)", undefined, ["(select max(a) from t)"], { "(select max(a) from t)": 1 }],
  ["select :x::int + 1", { x: 5 }, ["CAST(? AS INTEGER) + 1"], { "CAST(? AS INTEGER) + 1": 6 }],
  ["select a + :x::int from t", { x: 5 }, ["a + CAST(? AS INTEGER)"], { "a + CAST(? AS INTEGER)": 6 }],
  ["select * from (select a + :x::int from t)", { x: 5 }, ["a + CAST(? AS INTEGER)"], { "a + CAST(? AS INTEGER)": 6 }],
  ["update t set a = a returning upper(b)", undefined, ["upper(b)"], { "upper(b)": "X" }],
  ["update t set a = a returning a + :x::int", { x: 5 }, ["a + CAST(? AS INTEGER)"], { "a + CAST(? AS INTEGER)": 6 }],
  // A column named by a number, which a libSQL row also holds another column's value under.
  ["select 1, 2 + 3", undefined, ["1", "2 + 3"], { "1": 1, "2 + 3": 5 }],
];

test("SQLite: a row holds the columns its type declares, on every client, with the comment on", async () => {
  const root = project("file:./app.db");
  const seed = "create table t (a integer, b text); insert into t values (1, 'x');";
  const file = join(root, "app.db");
  const own = new DatabaseSync(file);
  own.exec(seed);
  const better = new (driver("better-sqlite3"))(":memory:");
  better.exec(seed);
  const libsql = driver("@libsql/client").createClient({ url: `file:${join(root, "libsql.db")}` });
  await libsql.executeMultiple(seed);
  const turso = driver("@tursodatabase/database");
  const tursoDb = await turso.connect(":memory:");
  await tursoDb.exec(seed);
  const tursoCompat = new (driver("@tursodatabase/database/compat").Database)(":memory:");
  tursoCompat.exec(seed);
  const sqlite3 = driver("sqlite3");
  const callbacks = new sqlite3.Database(":memory:");
  await new Promise<void>((resolve, reject) => callbacks.exec(seed, (e: Error | null) => (e ? reject(e) : resolve())));
  const wrapped = await driver("sqlite").open({ filename: ":memory:", driver: sqlite3.Database });
  await wrapped.exec(seed);
  const clients: [string, unknown, boolean][] = [
    ["node:sqlite", own, true],
    ["better-sqlite3", better, true],
    ["libSQL", libsql, false],
    ["Turso", tursoDb, false],
    ["Turso's compat Database", tursoCompat, true],
    ["sqlite3", callbacks, false],
    ["sqlite", wrapped, false],
  ];
  try {
    for (const [raw, params, names, expected] of SQLITE) {
      const { query, columns } = await built(root, raw);
      assert.deepEqual(columns, names, raw);
      for (const [name, client, sync] of clients) {
        assert.deepEqual(row(await query.run(client, params)), expected, `${name}: ${raw}`);
        if (sync) assert.deepEqual(row(query.runSync(client, params)), expected, `${name}, runSync: ${raw}`);
      }
    }
    // A column named `length`, which a libSQL row also holds its width under.
    const { query } = await built(root, "select 7 as length, 8 as b");
    assert.deepEqual(row(await query.run(libsql)), { length: 7, b: 8 });
  } finally {
    own.close();
    better.close();
    libsql.close();
    tursoDb.close();
    tursoCompat.close();
    callbacks.close();
    await wrapped.close();
    await closeSessions();
    rmSync(root, { recursive: true, force: true });
  }
});

test("MySQL: a row holds the columns its type declares, where the driver writes each value into the statement", { skip: mysqlLive }, async () => {
  const own = await mysqlDatabase("column_names", "create table t (a int, b text); insert into t values (1, 'x');");
  const root = project(own.url, ["mysql2", "mariadb"]);
  const u = new URL(own.url);
  const maria = await driver("mariadb").createConnection({ host: u.hostname, port: Number(u.port), user: decodeURIComponent(u.username) || "root", password: decodeURIComponent(u.password), database: u.pathname.slice(1), allowPublicKeyRetrieval: true });
  try {
    const cases: [string, object | undefined, string[], unknown[]][] = [
      ["select 1 + 1", undefined, ["1 + 1"], [2n]],
      ["select upper(b) from t", undefined, ["upper(b)"], ["X"]],
      // mariadb's text names these `CAST(5 AS SIGNED) + 1` and `concat('hey', 'a')`.
      ["select :x::int + 1", { x: 5 }, ["CAST(? AS SIGNED) + 1"], [6n]],
      ["select concat(:s::text, 'a')", { s: "hey" }, ["concat(?, 'a')"], ["heya"]],
      // Written into the text, the value would name its column `hey`, the first column's name, and leave one key where the type declares two.
      ["select 'hey', :s::text", { s: "hey" }, ["hey", "?"], ["hey", "hey"]],
      // The name a value would give its column, on the object's prototype.
      ["select :s::text", { s: "__proto__" }, ["?"], ["__proto__"]],
    ];
    for (const [raw, params, names, values] of cases) {
      const { query, columns } = await built(root, raw);
      assert.deepEqual(columns, names, raw);
      for (const [name, client] of [["mysql2", own.db], ["mariadb", maria]] as const) {
        const got = row(await query.run(client, params));
        assert.deepEqual(sorted(Object.keys(got)), sorted(names), `${name}: ${raw}`);
        assert.deepEqual(names.map((n) => got[n]), values, `${name}: ${raw}`);
      }
    }
  } finally {
    await maria.end();
    await closeSessions();
    await own.drop();
    rmSync(root, { recursive: true, force: true });
  }
});

test("PlanetScale: rows are read by position under the names declared, and a query defined by hand keeps the driver's", async () => {
  let as: unknown;
  // As the driver answers: an array or an object per row, named as the values were written into the text.
  const conn = {
    execute: async (_: string, __: unknown, options: { as: string }) => {
      as = options.as;
      return { rows: options.as === "array" ? [[6n]] : [{ "CAST(5 AS SIGNED) + 1": 6n }], fields: [{ name: "CAST(5 AS SIGNED) + 1", type: "INT64" }], statement: "" };
    },
  };
  const declared = defineQuery({ dialect: "mysql", sql: "select CAST(:x AS SIGNED) + 1", params: ["x"], cardinality: "many", columns: [["CAST(? AS SIGNED) + 1", "int8"]] } as never) as any;
  assert.deepEqual(await declared.run(conn, { x: 5 }), [{ "CAST(? AS SIGNED) + 1": 6n }]);
  assert.equal(as, "array");
  const byHand = defineQuery({ dialect: "mysql", sql: "select CAST(:x AS SIGNED) + 1", params: ["x"], cardinality: "many" } as never) as any;
  assert.deepEqual(await byHand.run(conn, { x: 5 }), [{ "CAST(5 AS SIGNED) + 1": 6n }]);
  assert.equal(as, "object");
});

test("Postgres: a column with no alias is named by the expression's form, with no text of the statement in it", { skip: pgLive }, async () => {
  const own = await postgresDatabase("column_names", "create table t (a int, b text); insert into t values (1, 'x');");
  const root = project(own.url, ["postgres"]);
  try {
    for (const [raw, params, names, expected] of [
      ["select 1 + 1", undefined, ["?column?"], { "?column?": 2 }],
      ["select :x::int + 1", { x: 5 }, ["?column?"], { "?column?": 6 }],
      ["update t set a = a returning upper(b)", undefined, ["upper"], { upper: "X" }],
    ] as const) {
      const { query, columns } = await built(root, raw);
      assert.deepEqual(columns, names, raw);
      assert.deepEqual(row(await query.run(own.sql, params)), expected, raw);
    }
  } finally {
    await closeSessions();
    await own.drop();
    rmSync(root, { recursive: true, force: true });
  }
});

test("DuckDB: a column with no alias is named by the statement as DuckDB reads it, with no comment in it", async () => {
  const root = project("duckdb:./app.duckdb", ["@duckdb/node-api"]);
  const { DuckDBInstance } = driver("@duckdb/node-api");
  const instance = await DuckDBInstance.create(join(root, "app.duckdb"));
  const connection = await instance.connect();
  await connection.run("create table t (a int, b text); insert into t values (1, 'x')");
  try {
    for (const [raw, params, names, expected] of [
      ["select 1 + 1", undefined, ["(1 + 1)"], { "(1 + 1)": 2 }],
      ["select :x::int + 1", { x: 5 }, ["(CAST($1 AS INTEGER) + 1)"], { "(CAST($1 AS INTEGER) + 1)": 6 }],
      ["update t set a = a returning upper(b)", undefined, ["upper(b)"], { "upper(b)": "X" }],
    ] as const) {
      const { query, columns } = await built(root, raw);
      assert.deepEqual(columns, names, raw);
      assert.deepEqual(row(await query.run(connection, params)), expected, raw);
    }
  } finally {
    connection.closeSync();
    instance.closeSync();
    await closeSessions();
    rmSync(root, { recursive: true, force: true });
  }
});
