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

test("a refusal is placed at the call or the text it names, in UTF-8 bytes", () => {
  const sql = "-- café\nselect n.id from notes n right join notes m on m.id = n.id";
  const bytes = (s: string) => new TextEncoder().encode(s).length;
  assert.deepEqual(placeRefusal(sql, 'near "right": syntax error'), { start: bytes(sql.slice(0, sql.indexOf("right"))), length: 5 });
  assert.deepEqual(placeRefusal("select JSON_PRETTY (x) from t", "no such function: json_pretty"), { start: 7, length: 11 });
  // A message that names nothing in the statement points at its first word.
  assert.deepEqual(placeRefusal("  select 1", "unrecognized token"), { start: 2, length: 6 });
});
