// The examples of the type descriptor format (spec/examples.ts) held to the engine: each example of a database Reylo runs has its setup run on an empty database of its own and its statement analyzed there by the root module's `analyze`, which returns what `reylo analyze` prints, and every field the example shows must be what the engine writes, all but the server's version, which is the test server's own. Reylo runs no SQL Server or Oracle, so their examples are written by hand.
import { test, type TestContext } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { Ajv2020 } from "ajv/dist/2020.js";
import { analyze } from "../src/index.ts";
import * as examples from "../spec/examples.ts";
import type { Example } from "../spec/examples.ts";
import type { StatementDescriptor } from "../spec/index.ts";
import { DRIVERS, driver, mysqlDatabase, mysqlLive, pgLive, postgresDatabase } from "./helpers.ts";

const schema = JSON.parse(readFileSync(new URL("../spec/typedefs.schema.json", import.meta.url), "utf8"));
const ajv = new Ajv2020({ strict: false, allErrors: true });
const valid = ajv.compile(schema);

const all = Object.entries(examples as Record<string, Example>);
const of = (database: Example["database"]) => all.filter(([, e]) => e.database === database);

/** What the engine wrote for an example's statement, held to the example: a valid descriptor with no diagnostic, and each field the example shows, the dialect by its name alone. */
function held(name: string, example: Example, found: StatementDescriptor): void {
  assert.ok(valid(found), `${name}: ${ajv.errorsText(valid.errors)}`);
  assert.deepEqual(found.diagnostics, [], name);
  for (const [key, want] of Object.entries(example.descriptor)) {
    if (key === "dialect") assert.equal(found.dialect.name, example.database, `${name}: dialect`);
    else assert.deepEqual(found[key as keyof StatementDescriptor], want, `${name}: ${key}`);
  }
}

test("every example is of a database held to the engine below, or of SQL Server or Oracle, written by hand", () => {
  assert.ok(all.length > 0);
  for (const [name, example] of all) assert.ok(["postgres", "mysql", "sqlite", "duckdb", "sqlserver", "oracle"].includes(example.database), `${name}: ${example.database}`);
});

test("the Postgres examples are what the engine writes", { skip: pgLive }, async (t) => {
  for (const [name, example] of of("postgres")) {
    await t.test(name, async (t: TestContext) => {
      const db = await postgresDatabase(`reylo_spec_${name}`);
      try {
        // An extension the setup creates is one the server may not have installed.
        for (const [, extension] of example.setup?.matchAll(/create extension (\w+)/gi) ?? []) {
          const [found] = await db.sql`select 1 from pg_available_extensions where name = ${extension!}`;
          if (!found) return t.skip(`the test server has no ${extension}`);
        }
        if (example.setup) await db.sql.unsafe(example.setup).simple();
        held(name, example, await analyze({ url: db.url, sql: example.statement }));
      } finally {
        await db.drop();
      }
    });
  }
});

test("the MySQL examples are what the engine writes", { skip: mysqlLive }, async (t) => {
  for (const [name, example] of of("mysql")) {
    await t.test(name, async () => {
      const db = await mysqlDatabase(`reylo_spec_${name}`, example.setup);
      try {
        held(name, example, await analyze({ url: db.url, sql: example.statement }));
      } finally {
        await db.drop();
      }
    });
  }
});

test("the SQLite examples are what the engine writes", async (t) => {
  for (const [name, example] of of("sqlite")) {
    await t.test(name, async () => {
      const file = join(mkdtempSync(join(tmpdir(), "reylo-spec-")), "app.db");
      const db = new DatabaseSync(file);
      if (example.setup) db.exec(example.setup);
      db.close();
      held(name, example, await analyze({ url: `file:${file}`, sql: example.statement }));
    });
  }
});

test("the DuckDB examples are what the engine writes", { skip: existsSync(join(DRIVERS, "@duckdb", "node-api")) ? false : "lab/drivers is not installed (no @duckdb/node-api)" }, async (t) => {
  const { DuckDBInstance } = driver("@duckdb/node-api");
  for (const [name, example] of of("duckdb")) {
    await t.test(name, async () => {
      // A project of its own, whose @duckdb/node-api answers the engine's needs, as an app's does.
      const root = mkdtempSync(join(tmpdir(), "reylo-spec-duckdb-"));
      writeFileSync(join(root, "package.json"), "{}");
      mkdirSync(join(root, "node_modules", "@duckdb"), { recursive: true });
      symlinkSync(join(DRIVERS, "@duckdb", "node-api"), join(root, "node_modules", "@duckdb", "node-api"));
      const file = join(root, "app.duckdb");
      const instance = await DuckDBInstance.create(file);
      const connection = await instance.connect();
      if (example.setup) await connection.run(example.setup);
      connection.closeSync();
      instance.closeSync();
      const cwd = process.cwd();
      process.chdir(root);
      try {
        held(name, example, await analyze({ url: `duckdb:${file}`, sql: example.statement }));
      } finally {
        process.chdir(cwd);
      }
    });
  }
});
