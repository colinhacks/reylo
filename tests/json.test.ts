import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { sqlModule } from "../src/loader.ts";
import { reyloImport } from "../src/module-types.ts";
import { closeSessions, declarations, DRIVERS, INDEX, mysqlDatabase, mysqlLive, pgLive, postgresDatabase, ROOT } from "./helpers.ts";

// A `json` or `jsonb` value that nothing in the schema or the statement gives a shape is any JSON
// value, `Json`, which `reylo` exports and a module imports where one of its types names it; so is
// a value Reylo cannot type inside JSON a statement builds. A JSON parameter's null is JSON's null
// where the statement takes no SQL NULL for it, and SQL NULL where it does.

// TypeScript's own launcher, run by this Node: node_modules/.bin holds a shell script, which Windows cannot start.
const TSC = join(ROOT, "node_modules", "typescript", "bin", "tsc");

const QUERY_ONLY = `import type { Query } from "reylo";`;
const WITH_JSON = `import type { Json, Query } from "reylo";`;

test("a module imports Json from reylo only where one of its types names it", () => {
  assert.equal(reyloImport("{ stage: Json }", "{ id: number }"), WITH_JSON);
  assert.equal(reyloImport("Record<never, never>", "{ tags: Json[]; doc: Record<string, Json>; g: { type: string; [key: string]: Json | undefined } }"), WITH_JSON);
  // A key of that name, a string literal type and a column's comment name no type.
  assert.equal(reyloImport("{ Json: number; Jsonb?: string }", `{ kind: "Json" | "a Json b" }`), QUERY_ONLY);
  assert.equal(reyloImport("Record<never, never>", "{\n  /** the Json payload */\n  id: number;\n}"), QUERY_ONLY);
  // A comment's delimiter inside a quoted name starts no comment, and a quote inside a comment no string.
  assert.equal(reyloImport("Record<never, never>", `{ "/*": number; data: Json; "*/": number }`), WITH_JSON);
  assert.equal(reyloImport("Record<never, never>", `{\n  /** a " quote */\n  data: Json;\n  /** another " */\n  id: number;\n}`), WITH_JSON);
  assert.equal(reyloImport("Record<never, never>", `{\n  /** a " quote */\n  id: number;\n  /** Json" */\n  n: number;\n}`), QUERY_ONLY);
});

test("SQLite: a value nothing types inside JSON the statement builds is Json, which the type check reads as any JSON value", async () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-json-")));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".env"), "DATABASE_URL=file:./app.db\n");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table docs (id integer primary key, doc text not null)");
  db.close();
  mkdirSync(join(root, "node_modules", "@reylosql"), { recursive: true });
  symlinkSync(ROOT, join(root, "node_modules", "reylo"));
  symlinkSync(join(ROOT, "packages", "sqlite"), join(root, "node_modules", "@reylosql", "sqlite"));
  // noUnusedLocals: a module that names no Json imports none.
  const compilerOptions = { target: "esnext", module: "nodenext", lib: ["esnext"], types: [], strict: true, noUnusedLocals: true, noEmit: true, skipLibCheck: true, allowImportingTsExtensions: true };
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ compilerOptions, contentMappers: [{ package: "reylo", extensions: [".sql"] }], include: ["main.ts"] }));
  const files = {
    "built.sql": "select json_object('k', json_extract(doc, '$.a'), 'n', id) as f from docs\n",
    "plain.sql": "select json_object('Json', id) as Json from docs\n",
  };
  for (const [name, text] of Object.entries(files)) writeFileSync(join(root, name), text);
  const declared = await declarations(root, files);
  assert.deepEqual(declared["built.sql"]!.split("\n").slice(0, 3), [WITH_JSON, "export type Params = Record<never, never>;", "export type Row = { f: { k: Json; n: number } };"]);
  assert.deepEqual(declared["plain.sql"]!.split("\n").slice(0, 3), [QUERY_ONLY, "export type Params = Record<never, never>;", "export type Row = { Json: { Json: number } };"]);
  writeFileSync(
    join(root, "main.ts"),
    [
      `import type { Json } from "reylo";`,
      `import type { Row } from "./built.sql";`,
      `import type { Row as Plain } from "./plain.sql";`,
      `declare const row: Row;`,
      `declare const plain: Plain;`,
      `export const k: Json = row.f.k;`,
      `export const n: number = row.f.n;`,
      `// @ts-expect-error a JSON value is no string until it is narrowed`,
      `export const s: string = row.f.k;`,
      `export const narrowed: string | undefined = typeof row.f.k === "string" ? row.f.k : undefined;`,
      `export const key: number = plain.Json.Json;`,
      ``,
    ].join("\n"),
  );
  const r = spawnSync(process.execPath, [TSC, "-p", "tsconfig.json", "--runExternalCode", "--pretty", "false"], { cwd: root, encoding: "utf8" });
  assert.equal(r.stdout + r.stderr, "");
  assert.equal(r.status, 0);
});

/** A project at a new directory on `url`, with `driver` from lab/drivers installed. */
function project(url: string, driver: string): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-json-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  mkdirSync(join(root, "node_modules"));
  symlinkSync(join(DRIVERS, driver), join(root, "node_modules", driver));
  return root;
}

type Built = { run(client: unknown, params?: unknown): Promise<unknown> };

/** The module a build step writes for `raw`, imported: its default export. */
async function built(root: string, name: string, raw: string): Promise<Built> {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, raw);
  const { code } = await sqlModule(file, raw);
  const out = join(root, `${name}.mjs`);
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  return ((await import(pathToFileURL(out).href)) as { default: Built }).default;
}

/** A module's Params and Row lines. */
const types = (declaration: string) => declaration.split("\n").filter((line) => /^export type (Params|Row) = /.test(line) || line.startsWith("import type"));

test("Postgres: a jsonb value nothing constrains is Json, and its parameter's null is JSON's null where the statement takes no SQL NULL", { skip: pgLive }, async () => {
  const own = await postgresDatabase("reylo_json", "create table events (id int generated always as identity primary key, payload jsonb not null, meta jsonb)");
  const sql = own.sql;
  const root = project(own.url, "postgres");
  const stage = `with args as (select :stage::jsonb as stage) select stage, stage->'a' as a from args\n`;
  const insert = "insert into events (payload, meta) values (:payload, :meta) returning id\n";
  try {
    const declared = await declarations(root, { "stage.sql": stage, "insert.sql": insert });
    assert.deepEqual(types(declared["stage.sql"]!), [WITH_JSON, "export type Params = { stage: Json };", "export type Row = { stage: Json; a: Json };"]);
    assert.deepEqual(types(declared["insert.sql"]!), [WITH_JSON, "export type Params = { payload: Json; meta?: Json };", "export type Row = { id: number };"]);
    await closeSessions();
    const write = await built(root, "insert", insert);
    // payload, a NOT NULL column, takes JSON's null; meta, which takes SQL NULL, takes it for null.
    const { id } = (await write.run(sql, { payload: null, meta: null })) as { id: number };
    const { id: other } = (await write.run(sql, { payload: { a: [1, "x"] } })) as { id: number };
    await assert.rejects(async () => write.run(sql, { meta: {} }), { name: "TypeError", message: /Parameter "payload" is missing, and the statement takes no null for it: pass a value\.$/ });
    const stored = await sql.unsafe(`select id, payload is null as payload_null, payload = 'null'::jsonb as json_null, payload::text as payload, meta is null as meta_null from events order by id`);
    assert.deepEqual(stored.map((row: Record<string, unknown>) => ({ ...row })), [
      { id, payload_null: false, json_null: true, payload: "null", meta_null: true },
      { id: other, payload_null: false, json_null: false, payload: `{"a": [1, "x"]}`, meta_null: true },
    ]);
    const read = await built(root, "stage", stage);
    assert.deepEqual({ ...((await read.run(sql, { stage: { a: "draft" } })) as object) }, { stage: { a: "draft" }, a: "draft" });
    assert.deepEqual({ ...((await read.run(sql, { stage: null })) as object) }, { stage: null, a: null });
  } finally {
    await closeSessions();
    await own.drop();
  }
});

test("MySQL: a JSON value nothing constrains is Json, and its parameter's null is JSON's null where the statement takes no SQL NULL", { skip: mysqlLive }, async () => {
  const own = await mysqlDatabase("reylo_json", "create table events (id int auto_increment primary key, payload json not null, meta json)");
  const root = project(own.url, "mysql2");
  const insert = "insert into events (payload, meta) values (:payload, :meta)\n";
  const read = "select payload, meta, payload->'$.a' as a from events order by id\n";
  try {
    const declared = await declarations(root, { "insert.sql": insert, "read.sql": read });
    assert.deepEqual(types(declared["insert.sql"]!), [WITH_JSON, "export type Params = { payload: Json; meta?: Json };", "export type Row = never;"]);
    assert.deepEqual(types(declared["read.sql"]!), [WITH_JSON, "export type Params = Record<never, never>;", "export type Row = { payload: Json; meta: Json; a: Json };"]);
    await closeSessions();
    const write = await built(root, "insert", insert);
    await write.run(own.db, { payload: null, meta: null });
    await write.run(own.db, { payload: { a: [1, "x"] } });
    assert.deepEqual(await own.rows("select payload is null as payload_null, json_type(payload) as kind, meta is null as meta_null from events order by id"), [
      { payload_null: 0, kind: "NULL", meta_null: 1 },
      { payload_null: 0, kind: "OBJECT", meta_null: 1 },
    ]);
    assert.deepEqual(((await (await built(root, "read", read)).run(own.db)) as object[]).map((row) => ({ ...row })), [
      { payload: null, meta: null, a: null },
      { payload: { a: [1, "x"] }, meta: null, a: [1, "x"] },
    ]);
  } finally {
    await closeSessions();
    await own.drop();
  }
});
