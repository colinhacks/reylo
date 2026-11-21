// Drizzle's own Postgres driver (`drizzle-orm/postgres/client`): its Pool, a Connection from
// connect(), one from pool.acquire(), the connection pool.transaction() hands its callback with a
// savepoint inside it, and drizzle-orm/postgres's database over the pool with its transaction:
//   --transport tcp      against the shared local Postgres
//   --transport neon-ws  through the local Neon proxy's WebSocket, from `postgres/client/neon-ws`
// Its HTTP clients name no column's type, which reylo refuses (S9 alone).
import ws from "ws";
import { args, callbackTx, lazy, notRun, requireFrom, runJob, type Shape } from "../lib/harness.ts";

const a = args();
const transport = a.transport ?? "tcp";
const neon = transport === "neon-ws";
const meta = { pkg: `drizzle-orm/postgres/client (${transport})`, version: a.version!, target: neon ? "Postgres 14 via local Neon proxy" : "Postgres 14 (local)" };
const proxy = process.env.DM_NEON_PROXY ?? "127.0.0.1:54444";
const config = neon
  ? { url: "postgres://postgres:postgres@db.localtest.me:5432/main", wsProxy: () => `${proxy}/v1`, secure: false, webSocketConstructor: ws }
  : { url: process.env.DM_PG_URL ?? "postgres://localhost:5432/postgres" };

let client: any;
let drizzle: (config: { client: unknown }) => any;
let neonHttp: any;
try {
  client = requireFrom(`${a.pkg!}/postgres/client${neon ? "/neon-ws" : ""}`);
  drizzle = requireFrom(`${a.pkg!}/postgres`).drizzle;
  neonHttp = requireFrom(`${a.pkg!}/postgres/client/neon-http`);
} catch (e) {
  notRun(meta, `cannot load ${a.pkg}/postgres/client: ${(e as Error).message.split("\n")[0]}`);
  process.exit(0);
}

await runJob(meta, async () => {
  const pool = client.createPool({ ...config, max: 10 });
  const connection = lazy(() => client.connect(config));
  const db = drizzle({ client: pool });
  const begin = (fn: (tx: any) => Promise<unknown>) => pool.transaction(fn);
  const dbBegin = (fn: (tx: any) => Promise<unknown>) => db.transaction(fn);
  const savepoint = (tx: any, fn: (sp: any) => Promise<unknown>) => tx.transaction(fn);
  return {
    ...meta,
    dialect: "postgres",
    // Outside any transaction of the pool's, whose guard refuses the pool inside its own callback.
    count: (tag) => pool.outsideTransaction(async () => (await pool.query("select count(*)::int as n from driver_matrix.scratch where tag = $1", [tag], { mode: "object" })).rows[0].n),
    shapes: [
      { name: "Pool", kind: "async", scope: (fn) => fn(pool) },
      { name: "Connection (connect())", kind: "async", scope: async (fn) => fn(await connection()) },
      {
        name: "Connection (pool.acquire())",
        kind: "async",
        scope: async (fn) => {
          const c = await pool.acquire();
          try {
            await fn(c);
          } finally {
            pool.release(c);
          }
        },
      },
      { name: "pool.transaction(tx)", kind: "async", inTransaction: true, scope: (fn) => begin((tx) => fn(tx)), tx: callbackTx(begin, savepoint) },
      { name: "tx.transaction(sp) (savepoint)", kind: "async", inTransaction: true, scope: (fn) => begin((tx) => tx.transaction((sp: any) => fn(sp))) },
      { name: "drizzle({ client: pool })", kind: "async", scope: (fn) => fn(db) },
      { name: "db.transaction(tx)", kind: "async", inTransaction: true, scope: (fn) => dbBegin((tx) => fn(tx)), tx: callbackTx(dbBegin, savepoint) },
      { name: "neon-http createPool()", kind: "async", refused: /Drizzle's HTTP clients/, scope: (fn) => fn(neonHttp.createPool(config.url)) },
    ] satisfies Shape[],
    close: async () => {
      await pool.end();
      await (await connection()).end();
    },
  };
});
