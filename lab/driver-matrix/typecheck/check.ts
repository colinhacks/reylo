// The type side of the matrix: generates cases.ts, which passes every client and transaction
// object of every typed driver major to run() of a Postgres, a MySQL and a SQLite query (the
// ClientFor<D> parameter) and checks the type run() returns, then runs the repository's tsc
// (TypeScript 7.1) over it and maps each diagnostic back to its case. A case is accepted when its
// line has no error. Writes typecheck/results.json and typecheck/RESULTS.md.
//
// Usage: node typecheck/check.ts
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

type Dialect = "postgres" | "mysql" | "sqlite";
const ALL: Dialect[] = ["postgres", "mysql", "sqlite"];
type Entry = {
  pkg: string;
  shape: string;
  type: string;
  /** The dialects whose queries must accept it; every other dialect must reject it. */
  accepts: Dialect[];
  /** runSync() accepts it and is typed as the row (a synchronous SQLite client); run() is a promise on every client. */
  sync?: boolean;
  note?: string;
};

const here = new URL(".", import.meta.url).pathname;
const repo = join(here, "../../..");

const imports = `import type * as pg8 from "pg-8";
import type * as pg7 from "pg-7";
import type postgres from "postgres-3";
import type { PGlite as PGlite05, Transaction as PGliteTx05 } from "pglite-05";
import type { PGlite as PGlite04, Transaction as PGliteTx04 } from "pglite-04";
import type { PGlite as PGlite03, Transaction as PGliteTx03 } from "pglite-03";
import type { PGlite as PGlite02, Transaction as PGliteTx02 } from "pglite-02";
import type * as neon1 from "neon-1";
import type * as neon010 from "neon-010";
import type * as neon09 from "neon-09";
import type { IDatabase, ITask } from "pg-promise";
import type { SQL, TransactionSQL, SavepointSQL, ReservedSQL } from "bun";
import type * as my3p from "mysql2-3/promise";
import type * as my3c from "mysql2-3";
import type * as my315p from "mysql2-315/promise";
import type * as my315c from "mysql2-315";
import type * as my2p from "mysql2-2/promise";
import type * as my2c from "mysql2-2";
import type * as ma3 from "mariadb-3";
import type * as ma3cb from "mariadb-3/callback";
import type * as ma34 from "mariadb-34";
import type * as ma34cb from "mariadb-34/callback";
import type * as ma2 from "mariadb-2";
import type * as ps2 from "planetscale-2";
import type * as ps1 from "planetscale-1";
import type BetterSqlite3 from "better-sqlite3";
import type { DatabaseSync, SQLTagStore } from "node:sqlite";
import type { Database as BunDatabase } from "bun:sqlite";
import type * as ls017 from "libsql-017";
import type * as ls018 from "libsql-018";
import type * as ls015 from "libsql-015";
import type * as ls014 from "libsql-014";
import type * as tu08 from "turso-08";
import type * as tu08c from "turso-08/compat";
import type * as tu07 from "turso-07";
import type * as tu07c from "turso-07/compat";
import type * as tu06 from "turso-06";
import type * as tu06c from "turso-06/compat";
import type * as tu04 from "turso-04";
import type * as tu04c from "turso-04/compat";
import type * as s3v5 from "sqlite3-5";
import type * as s3v6 from "sqlite3-6";
import type { Database as SqliteDatabase } from "sqlite";
import type { D1Database, D1DatabaseSession } from "@cloudflare/workers-types";
import type { NodePgDatabase, NodePgQueryResultHKT } from "drizzle-orm/node-postgres";
import type { PostgresJsDatabase } from "drizzle-orm/postgres-js";
import type { MySql2Database } from "drizzle-orm/mysql2";
import type { BetterSQLite3Database } from "drizzle-orm/better-sqlite3";
import type { LibSQLDatabase } from "drizzle-orm/libsql";
import type { PgTransaction } from "drizzle-orm/pg-core";
import type { MySqlTransaction } from "drizzle-orm/mysql-core";
import type { SQLiteTransaction } from "drizzle-orm/sqlite-core";
import type { NeonDatabase } from "drizzle-orm/neon-serverless";
import type { NeonHttpDatabase } from "drizzle-orm/neon-http";
import type { PgliteDatabase } from "drizzle-orm/pglite";
import type { BunSQLDatabase } from "drizzle-orm/bun-sql";
import type { PlanetScaleDatabase } from "drizzle-orm/planetscale-serverless";
import type { DrizzleD1Database } from "drizzle-orm/d1";
import type { BunSQLiteDatabase } from "drizzle-orm/bun-sqlite";
import type { DrizzleSqliteDODatabase } from "drizzle-orm/durable-sqlite";
import type { DurableObjectStorage } from "@cloudflare/workers-types";`;

const entries: Entry[] = [];
const pg = (pkg: string, shape: string, type: string, note?: string) => entries.push({ pkg, shape, type, accepts: ["postgres"], ...(note ? { note } : {}) });
const my = (pkg: string, shape: string, type: string, note?: string) => entries.push({ pkg, shape, type, accepts: ["mysql"], ...(note ? { note } : {}) });
const sq = (pkg: string, shape: string, type: string, sync = false, note?: string) => entries.push({ pkg, shape, type, accepts: ["sqlite"], sync, ...(note ? { note } : {}) });

for (const [ns, v] of [["pg8", "pg@8 (@types/pg 8.23.1)"], ["pg7", "pg@7 (@types/pg 7.14.11)"]] as const) {
  pg(v, "Pool", `${ns}.Pool`);
  pg(v, "Client", `${ns}.Client`);
  pg(v, "PoolClient", `${ns}.PoolClient`);
}
pg("postgres@3.4.9", "Sql", "postgres.Sql<{}>");
pg("postgres@3.4.9", "TransactionSql (begin, savepoint)", "postgres.TransactionSql<{}>");
pg("postgres@3.4.9", "ReservedSql (reserve)", "postgres.ReservedSql<{}>");
for (const [s, v] of [["05", "0.5.8"], ["04", "0.4.6"], ["03", "0.3.16"], ["02", "0.2.17"]]) {
  pg(`@electric-sql/pglite@${v}`, "PGlite", `PGlite${s}`);
  pg(`@electric-sql/pglite@${v}`, "Transaction", `PGliteTx${s}`);
}
for (const [ns, v] of [["neon1", "1.1.0"], ["neon010", "0.10.4"], ["neon09", "0.9.5"]]) {
  pg(`@neondatabase/serverless@${v}`, "Pool", `${ns}.Pool`);
  pg(`@neondatabase/serverless@${v}`, "Client", `${ns}.Client`);
  pg(`@neondatabase/serverless@${v}`, "PoolClient", `${ns}.PoolClient`);
  pg(`@neondatabase/serverless@${v}`, "neon()", `${ns}.NeonQueryFunction<false, false>`);
  pg(`@neondatabase/serverless@${v}`, "neon(url, { fullResults: true })", `${ns}.NeonQueryFunction<false, true>`);
}
pg("pg-promise@12.7.1", "IDatabase (db)", "IDatabase<{}>");
pg("pg-promise@12.7.1", "ITask (task, tx)", "ITask<{}>");
for (const [shape, type] of [["SQL", "SQL"], ["TransactionSQL (begin)", "TransactionSQL"], ["SavepointSQL (savepoint)", "SavepointSQL"], ["ReservedSQL (reserve)", "ReservedSQL"]]) {
  entries.push({ pkg: "Bun.sql@1.3.14 (bun-types)", shape: shape!, type: type!, accepts: ALL, note: "one class for the three adapters" });
}
for (const [p, c, v] of [["my3p", "my3c", "3.24.5"], ["my315p", "my315c", "3.15.3"], ["my2p", "my2c", "2.3.3"]]) {
  my(`mysql2@${v}`, "promise Pool", `${p}.Pool`);
  my(`mysql2@${v}`, "promise Connection", `${p}.Connection`);
  my(`mysql2@${v}`, "promise PoolConnection", `${p}.PoolConnection`);
  my(`mysql2@${v}`, "callback Pool", `${c}.Pool`);
  my(`mysql2@${v}`, "callback Connection", `${c}.Connection`);
  my(`mysql2@${v}`, "callback PoolConnection", `${c}.PoolConnection`);
}
for (const [ns, cb, v] of [["ma3", "ma3cb", "3.5.4"], ["ma34", "ma34cb", "3.4.5"], ["ma2", "", "2.5.6"]]) {
  my(`mariadb@${v}`, "Pool", `${ns}.Pool`);
  my(`mariadb@${v}`, "Connection", `${ns}.Connection`);
  my(`mariadb@${v}`, "PoolConnection", `${ns}.PoolConnection`);
  if (cb) {
    my(`mariadb@${v}`, "callback Pool (mariadb/callback)", `${cb}.Pool`);
    my(`mariadb@${v}`, "callback Connection (mariadb/callback)", `${cb}.Connection`);
    my(`mariadb@${v}`, "callback PoolConnection (mariadb/callback)", `${cb}.PoolConnection`);
  }
}
for (const [ns, v] of [["ps2", "2.0.0"], ["ps1", "1.20.2"]]) {
  const note = v === "2.0.0" ? "2.0 execute() takes no parameters, so run() cannot use it: every dialect should reject it" : undefined;
  for (const [shape, type] of [["Connection (connect())", `${ns}.Connection`], ["Client", `${ns}.Client`], ["Transaction", `${ns}.Transaction`]]) {
    entries.push({ pkg: `@planetscale/database@${v}`, shape: shape!, type: type!, accepts: v === "2.0.0" ? [] : ["mysql"], ...(note ? { note } : {}) });
  }
}
sq("better-sqlite3 (@types/better-sqlite3 9.6.0)", "Database", "BetterSqlite3.Database", true);
sq("node:sqlite (@types/node 26.6.1)", "DatabaseSync", "DatabaseSync", true);
entries.push({ pkg: "node:sqlite (@types/node 26.6.1)", shape: "SQLTagStore (createTagStore())", type: "SQLTagStore", accepts: [], note: "run() cannot use it (Node throws on a string), so every dialect should reject it" });
sq("bun:sqlite (bun-types 1.3.14)", "Database", "BunDatabase", true);
// Drizzle: each database by its $client, each transaction by what only its dialect's has.
pg("drizzle-orm@0.45.4 (node-postgres)", "drizzle(pool)", "NodePgDatabase & { $client: pg8.Pool }");
pg("drizzle-orm@0.45.4 (postgres-js)", "drizzle(sql)", "PostgresJsDatabase & { $client: postgres.Sql }");
pg("drizzle-orm@0.45.4", "PgTransaction (db.transaction, tx.transaction)", "PgTransaction<NodePgQueryResultHKT, Record<string, never>, Record<string, never>>");
my("drizzle-orm@0.45.4 (mysql2)", "drizzle(pool)", "MySql2Database & { $client: my3p.Pool }");
my("drizzle-orm@0.45.4", "MySqlTransaction (db.transaction, tx.transaction)", "MySqlTransaction<any, any, any, any>");
sq("drizzle-orm@0.45.4 (better-sqlite3)", "drizzle(database)", "BetterSQLite3Database & { $client: BetterSqlite3.Database }", true);
sq("drizzle-orm@0.45.4", "SQLiteTransaction (db.transaction, over better-sqlite3)", "SQLiteTransaction<'sync', any, any, any>", true);
sq("drizzle-orm@0.45.4 (libsql)", "drizzle(client)", "LibSQLDatabase & { $client: ls018.Client }");
sq("drizzle-orm@0.45.4", "SQLiteTransaction (db.transaction, over libSQL)", "SQLiteTransaction<'async', any, any, any>");
pg("drizzle-orm@0.45.4 (neon-serverless)", "drizzle(pool)", "NeonDatabase & { $client: neon1.Pool }");
pg("drizzle-orm@0.45.4 (neon-http)", "drizzle(neon())", "NeonHttpDatabase & { $client: neon1.NeonQueryFunction<false, false> }");
pg("drizzle-orm@0.45.4 (pglite)", "drizzle(pglite)", "PgliteDatabase & { $client: PGlite05 }");
entries.push({ pkg: "drizzle-orm@0.45.4 (bun-sql)", shape: "drizzle(sql)", type: "BunSQLDatabase & { $client: SQL }", accepts: ALL, note: "its client is Bun.sql's one class for the three adapters" });
my("drizzle-orm@0.45.4 (planetscale-serverless)", "drizzle(client)", "PlanetScaleDatabase & { $client: ps1.Client }");
sq("drizzle-orm@0.45.4 (d1)", "drizzle(d1)", "DrizzleD1Database & { $client: D1Database }");
sq("drizzle-orm@0.45.4 (bun-sqlite)", "drizzle(database)", "BunSQLiteDatabase & { $client: BunDatabase }", true);
sq("drizzle-orm@0.45.4 (durable-sqlite)", "drizzle(ctx.storage)", "DrizzleSqliteDODatabase & { $client: DurableObjectStorage }", true);
for (const [ns, v] of [["ls017", "0.17.4"], ["ls018", "0.18.0"], ["ls015", "0.15.15"], ["ls014", "0.14.0"]]) {
  sq(`@libsql/client@${v}`, "Client", `${ns}.Client`);
  sq(`@libsql/client@${v}`, "Transaction", `${ns}.Transaction`);
}
for (const [ns, v] of [["tu08", "0.8.3"], ["tu07", "0.7.2"], ["tu06", "0.6.1"], ["tu04", "0.4.4"]]) {
  sq(`@tursodatabase/database@${v}`, "Database (connect())", `${ns}.Database`);
  if (ns === "tu08" || ns === "tu07") sq(`@tursodatabase/database@${v}`, "Transaction (transactionAsync())", `${ns}.Transaction`);
  sq(`@tursodatabase/database@${v}`, "compat Database", `${ns}c.Database`, true);
}
sq("sqlite3@5.1.7", "Database", "s3v5.Database");
sq("sqlite3@6.0.1", "Database", "s3v6.Database");
sq("sqlite@5.1.1", "Database (open())", "SqliteDatabase");
sq("D1 (@cloudflare/workers-types 5.20260929.1)", "D1Database", "D1Database");
sq("D1 (@cloudflare/workers-types 5.20260929.1)", "D1DatabaseSession (withSession())", "D1DatabaseSession");

// ---------------------------------------------------------------------------------------------
// Generate cases.ts

type Case = { id: number; line: number; entry: Entry; check: string; expect: string };
const cases: Case[] = [];
const lines: string[] = [
  "// Generated by typecheck/check.ts; do not edit. Each /*@N*/ line is one case in typecheck/RESULTS.md.",
  ...imports.split("\n"),
  'import { defineQuery } from "../../../src/index.ts";',
  "",
  "type Params = { id: number };",
  "type Row = { id: number; name: string };",
  'const pgQ = defineQuery<Params, Row, "one", "postgres">({ dialect: "postgres", sql: "select id, name from t where id = :id", params: ["id"] });',
  'const myQ = defineQuery<Params, Row, "one", "mysql">({ dialect: "mysql", sql: "select id, name from t where id = :id", params: ["id"] });',
  'const sqQ = defineQuery<Params, Row, "one", "sqlite">({ dialect: "sqlite", sql: "select id, name from t where id = :id", params: ["id"] });',
  "/** The kind of what run() or runSync() is typed to return: a promise, or the row itself. */",
  'declare function kind<T>(v: T): [T] extends [PromiseLike<unknown>] ? "promise" : [T] extends [Row] ? "value" : "other";',
  "",
];
const qFor: Record<Dialect, string> = { postgres: "pgQ", mysql: "myQ", sqlite: "sqQ" };
entries.forEach((e, i) => {
  const v = `c${i}`;
  lines.push(`declare const ${v}: ${e.type}; // ${e.pkg} ${e.shape}`);
  for (const d of ALL) {
    const id = cases.length;
    lines.push(`/*@${id}*/ ${qFor[d]}.run(${v}, { id: 1 });`);
    cases.push({ id, line: lines.length, entry: e, check: `run on a ${d} query`, expect: e.accepts.includes(d) ? "accepted" : "rejected" });
  }
  const own = e.accepts.length === 1 ? e.accepts[0]! : e.accepts.length ? "postgres" : null;
  if (own) {
    let id = cases.length;
    lines.push(`/*@${id}*/ { const k: "promise" = kind(${qFor[own]}.run(${v}, { id: 1 })); void k; }`);
    cases.push({ id, line: lines.length, entry: e, check: "run(...) is typed as a promise", expect: "accepted" });
    id = cases.length;
    lines.push(`/*@${id}*/ { const k: "promise" = kind(${qFor[own]}.with(${v})({ id: 1 })); void k; }`);
    cases.push({ id, line: lines.length, entry: e, check: "with(...)(...) is typed as a promise", expect: "accepted" });
    id = cases.length;
    if (e.sync) {
      lines.push(`/*@${id}*/ { const k: "value" = kind(${qFor[own]}.runSync(${v}, { id: 1 })); void k; }`);
      cases.push({ id, line: lines.length, entry: e, check: "runSync(...) is typed as the row", expect: "accepted" });
    } else {
      lines.push(`/*@${id}*/ ${qFor[own]}.runSync(${v}, { id: 1 });`);
      cases.push({ id, line: lines.length, entry: e, check: "runSync(...) is refused", expect: "rejected" });
    }
  }
});
lines.push("");
writeFileSync(join(here, "cases.ts"), lines.join("\n"));

// ---------------------------------------------------------------------------------------------
// Run tsc and map its diagnostics to cases

const tscBin = join(repo, "node_modules/.bin/tsc");
const version = spawnSync(tscBin, ["--version"], { encoding: "utf8" }).stdout.trim();
const run = spawnSync(tscBin, ["-p", join(here, "tsconfig.json"), "--pretty", "false"], { encoding: "utf8", cwd: here });
const output = (run.stdout ?? "") + (run.stderr ?? "");
mkdirSync(join(here, "../.out"), { recursive: true });
writeFileSync(join(here, "../.out/tsc.log"), output);
const diags: { file: string; line: number; message: string }[] = [];
for (const raw of output.split("\n")) {
  const m = /^(.*?)\((\d+),(\d+)\): error (TS\d+: .*)$/.exec(raw);
  if (m) diags.push({ file: m[1]!, line: Number(m[2]), message: m[4]! });
  else if (raw.startsWith("  ") && diags.length) diags[diags.length - 1]!.message += " " + raw.trim();
}
const byLine = new Map<number, string[]>();
const stray: string[] = [];
for (const d of diags) {
  if (d.file.endsWith("cases.ts") && cases.some((c) => c.line === d.line)) byLine.set(d.line, [...(byLine.get(d.line) ?? []), d.message]);
  else stray.push(`${d.file}:${d.line}: ${d.message}`);
}

type Outcome = { pkg: string; shape: string; check: string; expect: string; actual: string; verdict: "ok" | "wrongly accepted" | "wrongly rejected"; message?: string; note?: string };
const outcomes: Outcome[] = cases.map((c) => {
  const errs = byLine.get(c.line);
  const actual = errs ? "rejected" : "accepted";
  const verdict = actual === c.expect ? "ok" : actual === "accepted" ? "wrongly accepted" : "wrongly rejected";
  return { pkg: c.entry.pkg, shape: c.entry.shape, check: c.check, expect: c.expect, actual, verdict, ...(errs ? { message: errs.join(" / ").slice(0, 400) } : {}), ...(c.entry.note ? { note: c.entry.note } : {}) };
});
const wrong = outcomes.filter((o) => o.verdict !== "ok");
const summary = `${outcomes.length} cases over ${entries.length} client types: ${outcomes.filter((o) => o.verdict === "ok").length} as expected, ${outcomes.filter((o) => o.verdict === "wrongly accepted").length} wrongly accepted, ${outcomes.filter((o) => o.verdict === "wrongly rejected").length} wrongly rejected${stray.length ? `; ${stray.length} diagnostics outside the case lines (the harness itself is broken, see .out/tsc.log)` : ""}.`;
writeFileSync(join(here, "results.json"), JSON.stringify({ tsc: version, summary, stray, outcomes }, null, 2) + "\n");

const cell = (s: unknown) => String(s ?? "").replace(/\|/g, "\\|").replace(/\s+/g, " ").trim();
const md: string[] = ["# Type check results", "", `Generated by \`node typecheck/check.ts\` with the repository's tsc (${version}) over [cases.ts](cases.ts). ${summary}`, ""];
if (stray.length) md.push("## Diagnostics outside the cases", "", ...stray.map((s) => `- ${cell(s)}`), "");
md.push("## Wrong verdicts", "");
if (wrong.length) {
  md.push("| Package | Client | Check | Expected | Actual | Note | tsc |", "| --- | --- | --- | --- | --- | --- | --- |");
  for (const o of wrong) md.push(`| ${cell(o.pkg)} | ${cell(o.shape)} | ${cell(o.check)} | ${o.expect} | ${o.actual} (${o.verdict}) | ${cell(o.note)} | ${cell(o.message)} |`);
} else md.push("None.");
md.push("", "## Every case", "", "| Package | Client | Check | Expected | Actual | Verdict |", "| --- | --- | --- | --- | --- | --- |");
for (const o of outcomes) md.push(`| ${cell(o.pkg)} | ${cell(o.shape)} | ${cell(o.check)} | ${o.expect} | ${o.actual} | ${o.verdict === "ok" ? "ok" : o.verdict.toUpperCase()} |`);
md.push("");
writeFileSync(join(here, "RESULTS.md"), md.join("\n"));
console.log(summary);
for (const o of wrong) console.log(`${o.verdict.toUpperCase()}: ${o.pkg} ${o.shape} -- ${o.check}${o.message ? ` (${o.message.slice(0, 160)})` : ""}`);
for (const s of stray.slice(0, 20)) console.log(`STRAY: ${s.slice(0, 300)}`);
process.exit(stray.length ? 1 : 0);
