// Control for the type check's "wrongly accepted" cases: at run time, a client of one database
// passed to another dialect's query sends that dialect's placeholders to the wrong server.
// Run with ./setup.sh done: node probes/cross-dialect.ts
import { createRequire } from "node:module";
import { defineQuery } from "../../../src/index.ts";
import { handWritten } from "../lib/hand-written.ts";

const require = createRequire(new URL("../package.json", import.meta.url));
const mariadb = require("mariadb-3");
const postgres = require("postgres-3");
const pgQ = defineQuery<any, any, "one", "postgres">({ dialect: "postgres", ...handWritten("select id, name from driver_matrix.people where id = :id", "postgres"), cardinality: "one" });
const myQ = defineQuery<any, any, "one", "mysql">({ dialect: "mysql", ...handWritten("select id, name from driver_matrix.people where id = :id", "mysql"), cardinality: "one" });
const sqQ = defineQuery<any, any, "one", "sqlite">({ dialect: "sqlite", ...handWritten("select id, name from people where id = :id", "sqlite"), cardinality: "one" });

const pool = mariadb.createPool({ host: "127.0.0.1", port: 33406, user: "root", password: "porg", database: "driver_matrix", allowPublicKeyRetrieval: true });
const sql = postgres(process.env.DM_PG_URL ?? "postgres://localhost:5432/postgres", { max: 1 });
for (const [label, p] of [
  ["mariadb Pool on a postgres query", () => pgQ.run(pool, { id: 1 })],
  ["mariadb Pool on a sqlite query", () => sqQ.run(pool, { id: 1 })],
  ["postgres.js sql on a mysql query", () => myQ.run(sql, { id: 1 })],
  ["postgres.js sql on a sqlite query", () => sqQ.run(sql, { id: 1 })],
] as const) {
  try {
    console.log(label, "->", JSON.stringify(await p()));
  } catch (e) {
    console.log(label, "->", (e as Error).constructor.name, (e as Error).message.split("\n")[0]);
  }
}
await pool.end();
await sql.end();
