import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Session } from "../src/analyze.ts";
import { placeRefusal, projectDrivers } from "../src/sqlite-drivers.ts";

// The drivers are the labs' (lab/drivers), linked into each project as an install would put them.
// libSQL carries SQLite 3.45.1, better-sqlite3 3.53 and sqlite3 3.52: `json_pretty` came in 3.46
// and `iif` with two arguments in 3.48.
const DRIVERS = fileURLToPath(new URL("../lab/drivers/node_modules", import.meta.url));
const missing = ["better-sqlite3", "@libsql/client", "sqlite3"].find((d) => !existsSync(join(DRIVERS, d)));
const skip = missing ? `lab/drivers is not installed (no ${missing})` : false;

function project(drivers: string[]): { root: string; env: Record<string, string> } {
  const root = mkdtempSync(join(tmpdir(), "reylo-drivers-"));
  writeFileSync(join(root, "package.json"), JSON.stringify({ dependencies: Object.fromEntries(drivers.map((d) => [d, "*"])) }));
  for (const d of drivers) link(root, d);
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table notes (id integer primary key, body text not null, meta text)");
  db.close();
  return { root, env: { DATABASE_URL: "file:./app.db" } };
}

function link(root: string, driver: string) {
  const to = join(root, "node_modules", driver);
  mkdirSync(dirname(to), { recursive: true });
  symlinkSync(join(DRIVERS, driver), to);
}

async function analyze(root: string, env: Record<string, string>, sql: string) {
  const session = await Session.open(root, env);
  try {
    const outcome = await session.analyze(sql);
    assert.ok(outcome.ok, !outcome.ok ? outcome.message : "");
    return outcome;
  } finally {
    await session.close();
  }
}

const PRETTY = "select id, json_pretty(meta) as meta from notes where id = :id";
const IIF = "select id, iif(id > 1, 'big') as size from notes";

test("a statement the app's SQLite refuses fails the type check at the call, with that SQLite's message", { skip }, async () => {
  const { root, env } = project(["@libsql/client"]);
  const pretty = await analyze(root, env, PRETTY);
  assert.deepEqual(pretty.analysis.diagnostics, [
    { message: "@libsql/client runs SQLite 3.45.1, which refuses this statement: no such function: json_pretty", start: PRETTY.indexOf("json_pretty"), length: "json_pretty".length, code: 6 },
  ]);
  const iif = await analyze(root, env, IIF);
  assert.deepEqual(iif.analysis.diagnostics, [
    { message: "@libsql/client runs SQLite 3.45.1, which refuses this statement: wrong number of arguments to function iif()", start: IIF.indexOf("iif"), length: 3, code: 6 },
  ]);
  // What every SQLite takes is no error.
  assert.deepEqual((await analyze(root, env, "select id, iif(id > 1, 'big', 'small') as size from notes")).analysis.diagnostics, []);
});

test("drivers whose SQLite has the function prepare it, and a project that lists no driver is not checked", { skip }, async () => {
  for (const drivers of [["better-sqlite3", "sqlite3"], []]) {
    const { root, env } = project(drivers);
    assert.deepEqual(projectDrivers(root), drivers);
    for (const sql of [PRETTY, IIF]) assert.deepEqual((await analyze(root, env, sql)).analysis.diagnostics, [], `${drivers.join(", ") || "no driver"}: ${sql}`);
  }
});

test("an analysis read from .reylo/ is checked on the drivers too", { skip }, async () => {
  const { root, env } = project([]);
  const first = await analyze(root, env, PRETTY);
  assert.deepEqual([first.source, first.analysis.diagnostics], ["database", []]);
  // The project takes libSQL: the entry still matches the schema, and the driver refuses it.
  writeFileSync(join(root, "package.json"), JSON.stringify({ dependencies: { "@libsql/client": "*" } }));
  link(root, "@libsql/client");
  const second = await analyze(root, env, PRETTY);
  assert.equal(second.source, "cache");
  assert.match(second.analysis.diagnostics[0]?.message ?? "", /^@libsql\/client runs SQLite 3\.45\.1, which refuses this statement: no such function: json_pretty$/);
});

test("a session whose connection was released checks a statement it reads from .reylo/ all the same", { skip }, async () => {
  const { root, env } = project(["@libsql/client"]);
  const session = await Session.open(root, env);
  try {
    const first = await session.analyze(PRETTY);
    assert.ok(first.ok && first.analysis.diagnostics.length === 1);
    // A build step's session releases its connection once idle, while the schema it read is still trusted.
    await session.release();
    const second = await session.analyze(PRETTY);
    assert.ok(second.ok, !second.ok ? second.message : "");
    assert.equal(second.source, "cache");
    assert.match(second.analysis.diagnostics[0]?.message ?? "", /^@libsql\/client runs SQLite 3\.45\.1, which refuses this statement: no such function: json_pretty$/);
  } finally {
    await session.close();
  }
});

test("a refusal is placed at the call or the text it names, in UTF-8 bytes", () => {
  const sql = "-- café\nselect n.id from notes n right join notes m on m.id = n.id";
  const bytes = (s: string) => new TextEncoder().encode(s).length;
  assert.deepEqual(placeRefusal(sql, 'near "right": syntax error'), { start: bytes(sql.slice(0, sql.indexOf("right"))), length: 5 });
  assert.deepEqual(placeRefusal("select JSON_PRETTY (x) from t", "no such function: json_pretty"), { start: 7, length: 11 });
  // A message that names nothing in the statement points at its first word.
  assert.deepEqual(placeRefusal("  select 1", "unrecognized token"), { start: 2, length: 6 });
});

// Cloudflare D1 through the project's wrangler, which runs it locally in workerd: D1 refuses
// functions its SQLite has (`json_pretty`, the alias `ceiling` where `ceil` is allowed), and says so
// only when a statement compiles.
const d1 = skip || (!existsSync(join(DRIVERS, "wrangler")) && "lab/drivers has no wrangler installed");

function worker(root: string, d1Binding: boolean) {
  const config = { name: "reylo-test", main: "worker.js", compatibility_date: "2026-10-01", ...(d1Binding && { d1_databases: [{ binding: "DB", database_name: "reylo-test", database_id: "00000000-0000-0000-0000-000000000000" }] }) };
  writeFileSync(join(root, "wrangler.jsonc"), JSON.stringify(config));
  writeFileSync(join(root, "worker.js"), "export default { fetch() { return new Response(''); } };\n");
}

test("a statement Cloudflare D1 refuses fails the type check at the call, with D1's message", { skip: d1, timeout: 120_000 }, async () => {
  const { root, env } = project(["wrangler"]);
  worker(root, true);
  const session = await Session.open(root, env);
  try {
    const CEILING = "select id, ceiling(id / 2.0) as half from notes";
    for (const [sql, fn] of [[PRETTY, "json_pretty"], [CEILING, "ceiling"]] as const) {
      const outcome = await session.analyze(sql);
      assert.ok(outcome.ok);
      const [diagnostic, ...rest] = outcome.analysis.diagnostics;
      assert.deepEqual(rest, []);
      assert.match(diagnostic?.message ?? "", new RegExp(`^Cloudflare D1, as wrangler \\d+\\.\\d+\\.\\d+ runs it locally, refuses this statement: not authorized to use function: ${fn}$`));
      assert.deepEqual([diagnostic?.start, diagnostic?.length, diagnostic?.code], [sql.indexOf(fn), fn.length, 6]);
    }
    // What D1 allows compiles against the schema the type check read, its parameters bound.
    for (const sql of ["select id, ceil(id / 2.0) as half from notes where id = :id", "insert into notes (body, meta) values (:body, :meta) returning id", IIF]) {
      const outcome = await session.analyze(sql);
      assert.ok(outcome.ok);
      assert.deepEqual(outcome.analysis.diagnostics, [], sql);
    }
  } finally {
    await session.close();
  }
});

test("a project whose wrangler config binds no D1 database is not asked", { skip: d1, timeout: 120_000 }, async () => {
  const { root, env } = project(["wrangler"]);
  worker(root, false);
  assert.deepEqual((await analyze(root, env, PRETTY)).analysis.diagnostics, []);
});
