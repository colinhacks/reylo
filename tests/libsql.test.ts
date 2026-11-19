import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { Session } from "../src/analyze.ts";
import { sqlModule } from "../src/loader.ts";
import { closeSessions, driver, DRIVERS, INDEX } from "./helpers.ts";

// libSQL's own SQL, on a database file libSQL made: a vector column, its vector index (which adds
// libSQL's own tables to the schema) and the functions and table-valued vector_top_k over them. The
// type check reads the file with its own SQLite, which has none of them, and the project's drivers
// decide whether its SQLite runs them.
const missing = ["@libsql/client", "better-sqlite3"].find((d) => !existsSync(join(DRIVERS, d)));
const skip = missing ? `lab/drivers is not installed (no ${missing})` : false;

const SCHEMA = `
  create table docs (id integer primary key, title text not null, emb F32_BLOB(3));
  create index docs_emb on docs (libsql_vector_idx(emb));
  insert into docs (title, emb) values ('a', vector('[1,2,3]')), ('b', vector('[3,2,1]')), ('c', null);
`;

async function project(drivers: string[]): Promise<string> {
  const root = mkdtempSync(join(tmpdir(), "reylo-libsql-"));
  writeFileSync(join(root, "package.json"), JSON.stringify({ dependencies: Object.fromEntries(drivers.map((d) => [d, "*"])) }));
  writeFileSync(join(root, ".env"), "DATABASE_URL=file:./app.db\n");
  for (const name of drivers) {
    const to = join(root, "node_modules", name);
    mkdirSync(dirname(to), { recursive: true });
    symlinkSync(join(DRIVERS, name), to);
  }
  const db = driver("@libsql/client").createClient({ url: `file:${join(root, "app.db")}` });
  await db.executeMultiple(SCHEMA);
  db.close();
  return root;
}

const TOP_K = "select id from vector_top_k('docs_emb', vector(:q), :k)";
const NEAREST = "select id, title, vector_distance_cos(emb, vector(:q)) as distance from docs where emb is not null order by distance limit 2";
const INSERT = "insert into docs (title, emb) values (:title, vector(:v)) returning id, emb";
const READ = "select id, emb, vector_extract(emb) as text from docs where id = :id";

test("libSQL's vector functions and vector_top_k type on a libSQL file, and @libsql/client prepares them", { skip }, async () => {
  const root = await project(["@libsql/client"]);
  const session = await Session.open(root, { DATABASE_URL: "file:./app.db" });
  try {
    const typed = async (sql: string) => {
      const outcome = await session.analyze(sql);
      assert.ok(outcome.ok, !outcome.ok ? outcome.message : "");
      assert.deepEqual(outcome.analysis.diagnostics, [], sql);
      return outcome.analysis;
    };
    const kinds = (list: readonly { name: string; type: { kind: string }; nullable?: boolean }[]) => Object.fromEntries(list.map((c) => [c.name, `${c.type.kind}${c.nullable ? "?" : ""}`]));
    const top = await typed(TOP_K);
    assert.deepEqual([kinds(top.params), kinds(top.columns)], [{ q: "text", k: "integer" }, { id: "integer" }]);
    const nearest = await typed(NEAREST);
    assert.deepEqual([kinds(nearest.params), kinds(nearest.columns)], [{ q: "text" }, { id: "integer", title: "union", distance: "float?" }]);
    assert.equal(kinds((await typed(INSERT)).params).v, "text");
    // A vector column outside a STRICT table, where libSQL allows no vector column, holds what its BLOB affinity stores.
    assert.deepEqual(kinds((await typed(READ)).columns), { id: "integer", emb: "union?", text: "text" });
    // libSQL's tables for the index are the schema's like any other.
    assert.deepEqual(Object.keys(kinds((await typed("select name, metadata from libsql_vector_meta_shadow")).columns)), ["name", "metadata"]);
    await typed("alter table docs alter column title to title text not null");
  } finally {
    await session.close();
  }
});

test("a project whose SQLite has no vectors is refused libSQL's functions by its driver", { skip }, async () => {
  const root = await project(["better-sqlite3"]);
  const session = await Session.open(root, { DATABASE_URL: "file:./app.db" });
  try {
    const outcome = await session.analyze(NEAREST);
    assert.ok(outcome.ok, !outcome.ok ? outcome.message : "");
    assert.match(outcome.analysis.diagnostics[0]?.message ?? "", /^better-sqlite3 runs SQLite [\d.]+, which refuses this statement: /);
  } finally {
    await session.close();
  }
});

test("on @libsql/client, .run() returns a vector as a Uint8Array, its distance as a number and vector_top_k's id as the row's", { skip }, async () => {
  const root = await project(["@libsql/client"]);
  let files = 0;
  const built = async (raw: string) => {
    const file = join(root, `q${files}.sql`);
    writeFileSync(file, raw);
    const { code } = await sqlModule(file, raw);
    const out = join(root, `q${files++}.mjs`);
    writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
    return (await import(pathToFileURL(out).href)).default;
  };
  const db = driver("@libsql/client").createClient({ url: `file:${join(root, "app.db")}` });
  try {
    const top = await built(TOP_K);
    assert.deepEqual(await top.run(db, { q: "[1,2,3]", k: 2 }), [{ id: 1 }, { id: 2 }]);
    const nearest = await built(NEAREST);
    const rows = await nearest.run(db, { q: "[1,2,3]" });
    assert.deepEqual(rows.map((r: { id: number; title: string }) => [r.id, r.title]), [[1, "a"], [2, "b"]]);
    assert.equal(rows[0].distance, 0);
    assert.ok(typeof rows[1].distance === "number" && rows[1].distance > 0);
    const f32 = (...values: number[]) => new Uint8Array(new Float32Array(values).buffer);
    const insert = await built(INSERT);
    assert.deepEqual(await insert.run(db, { title: "d", v: "[0.5,1,2]" }), { id: 4, emb: f32(0.5, 1, 2) });
    const read = await built(READ);
    const row = await read.run(db, { id: 1 });
    assert.ok(row.emb instanceof Uint8Array, "a vector is a Uint8Array, where @libsql/client hands over an ArrayBuffer");
    assert.deepEqual(row, { id: 1, emb: f32(1, 2, 3), text: "[1,2,3]" });
  } finally {
    db.close();
    await closeSessions();
  }
});
