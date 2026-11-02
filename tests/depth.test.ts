import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { engine, type Dialect, type Engine, type EngineIo } from "../src/engine.ts";
import { Session } from "../src/analyze.ts";

// How deeply a statement may nest for the engine to read it: libpg_query hands over at most 1,000
// levels of a parse tree (engine/core/src/stack.rs), the same in every host of the engine, and the
// embedded SQLite reads an expression 350 deep, where SQLite's own bound is 1,000. These are the deepest statements of each shape that parse, as engine/core/src/stack.rs's
// tests have them.
function nested(shape: string, n: number): string {
  const fold = (start: string, f: (s: string, i: number) => string) => Array.from({ length: n }, (_, i) => i).reduce(f, start);
  switch (shape) {
    case "sum": return `select ${Array(n).fill("id").join(" + ")} as x from users`;
    case "concat": return `select ${Array(n).fill("name").join(" || ")} as x from users`;
    case "and_or": return `select id from users where ${fold("true", (s, i) => `id = ${i} ${i % 2 === 0 ? "or" : "and"} (${s})`)}`;
    case "case": return `select ${fold("0", (s, i) => `case when id = ${i} then ${i} else ${s} end`)} as x from users`;
    case "coalesce": return `select ${fold("nick", (s) => `coalesce(${s}, name)`)} as x from users`;
    case "json": return `select ${fold("id", (s) => `json_build_object('a', ${s})`)} as x from users`;
    case "subquery": return `select (${fold("select 1", (s) => `select (${s})`)}) as x from users`;
    case "or": return `select id from users where ${Array.from({ length: n }, (_, i) => `id = ${i}`).join(" or ")}`;
    case "parens": return `select ${"(".repeat(n)}id${")".repeat(n)} as x from users`;
  }
  throw new Error(`no shape ${shape}`);
}
const POSTGRES_BOUNDS: [string, number][] = [["sum", 496], ["concat", 496], ["and_or", 495], ["case", 493], ["coalesce", 495], ["json", 495], ["subquery", 164]];
const SQLITE_BOUNDS: [string, number][] = [["sum", 350], ["and_or", 348], ["case", 348], ["coalesce", 349], ["subquery", 23]];
// MySQL's statements are read by sqlparser with a fifth of the bound, 200 levels of its recursion,
// which a parenthesized operand (`id = 0 or (…)`) and a subquery each spend two of, and at most 500
// operators, as engine/mysql/src/mysql.rs's tests have them.
const MYSQL_BOUNDS: [string, number][] = [["sum", 501], ["and_or", 98], ["case", 195], ["coalesce", 196], ["subquery", 97]];

// A server that describes every statement as one int4 column and answers every catalog query with
// no rows: enough for the engine to parse the statement and run every pass over its tree.
const io: EngineIo = {
  async describe(sql) {
    const params = new Set(sql.match(/\$\d+/g) ?? []).size;
    return { params: Array.from({ length: params }, () => ({ oid: 23 })), columns: [{ name: "x", oid: 23, table: 0, attnum: 0 }] };
  },
  async query() {
    return [];
  },
};

/** The dialect's engine, as the analysis runs it. */
const loaded = (dialect: Dialect): Promise<Engine> => engine(dialect);

test("a Postgres statement nested to the bound is read whole, one level past it falls back to the describe, and the engine goes on", async () => {
  const engine = await loaded("postgres");
  for (const [shape, n] of POSTGRES_BOUNDS) {
    const at = await engine.analyze(nested(shape, n), "postgres", io);
    assert.equal(at.kind, "select", `${shape} at ${n}`);
    assert.notEqual(at.cardinalityReason, "the statement did not parse locally", `${shape} at ${n}`);
    const past = await engine.analyze(nested(shape, n + 1), "postgres", io);
    assert.deepEqual([past.kind, past.cardinality, past.cardinalityReason], ["", "many", "the statement did not parse locally"], `${shape} at ${n + 1}`);
    assert.ok(past.columns.every((c) => c.nullable) && past.diagnostics.length === 0, `${shape} at ${n + 1}`);
  }
  // Far past the bound, and long lists that do not nest at all.
  assert.equal((await engine.analyze(nested("sum", 100_000), "postgres", io)).kind, "");
  assert.equal((await engine.analyze(nested("parens", 20_000), "postgres", io)).kind, "");
  assert.equal((await engine.analyze(nested("or", 10_000), "postgres", io)).kind, "select");
  assert.equal((await engine.analyze(nested("parens", 5_000), "postgres", io)).kind, "select");
  // The same instance answers a plain statement after all of that.
  const plain = await engine.analyze("select id, name from users where id = :id", "postgres", io);
  assert.equal(plain.kind, "select");
});

test("a SQLite statement nested to the engine's bound is typed, and one level past it is SQLite's error at that bound", async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-depth-"));
  writeFileSync(join(root, "package.json"), "{}");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null, nick text)");
  db.close();
  const session = await Session.open(root, { DATABASE_URL: "file:./app.db" });
  try {
    for (const [shape, n] of SQLITE_BOUNDS) {
      const at = await session.analyze(nested(shape, n), { force: true });
      assert.ok(at.ok && at.analysis.kind === "select" && at.analysis.diagnostics.length === 0, `${shape} at ${n}: ${JSON.stringify(at).slice(0, 300)}`);
      const past = await session.analyze(nested(shape, n + 1), { force: true });
      assert.ok(past.ok, `${shape} at ${n + 1}`);
      assert.deepEqual(past.ok && past.analysis.diagnostics.map((d) => d.message), ["Expression tree is too large (maximum depth 350)"], `${shape} at ${n + 1}`);
    }
    const plain = await session.analyze("select id, name from users where id = :id", { force: true });
    assert.ok(plain.ok && plain.analysis.cardinality === "maybeOne");
  } finally {
    await session.close();
  }
});

test("a MySQL statement nested to the bound is analyzed, and one far past it falls back to the describe, without exhausting the stack", async () => {
  const engine = await loaded("mysql");
  const mysql: EngineIo = {
    async describe() {
      return { kind: "mysql", columns: [{ name: "x", type: 8, flags: 1, charset: 63, length: 20, org_table: "", org_name: "" }], params: [] };
    },
    async query() {
      return [];
    },
  };
  for (const [shape, n] of MYSQL_BOUNDS) {
    const at = await engine.analyze(nested(shape, n), "mysql", mysql);
    assert.equal(at.kind, "select", `${shape} at ${n}`);
  }
  for (const shape of ["sum", "or", "case", "and_or"]) {
    const past = await engine.analyze(nested(shape, 100_000), "mysql", mysql);
    assert.equal(past.cardinality, "many", `${shape} at 100000`);
  }
  const plain = await engine.analyze("select id as x from users where id = 1", "mysql", mysql);
  assert.equal(plain.columns.length, 1);
});
