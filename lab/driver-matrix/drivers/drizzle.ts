// Drizzle over each of its drivers (--driver): the database `drizzle(client)` gives, a
// transaction it hands out, and a savepoint inside that transaction, wherever the driver has them.
// reylo runs on the driver client each one keeps on `session.client`, so the scenarios are the
// driver's own:
//   --driver node-postgres           over pg, against the shared local Postgres
//   --driver postgres-js             over postgres.js, the same server
//   --driver neon-serverless         over Neon's Pool, through the local Neon proxy (WebSocket)
//   --driver neon-http               over Neon's neon() function, through the same proxy (HTTP)
//   --driver pglite                  over PGlite, in-process
//   --driver bun-sql                 over Bun.sql, against the shared local Postgres, under Bun
//   --driver mysql2                  over mysql2's promise pool, against a MySQL container (--port)
//   --driver planetscale-serverless  over @planetscale/database's Client, through ps-http-sim
//   --driver better-sqlite3          over better-sqlite3, on the database file the runner seeded (--file)
//   --driver bun-sqlite              over bun:sqlite, on that file, under Bun
//   --driver libsql                  over @libsql/client, on that file through a file: URL
//   --driver d1                      over Cloudflare D1, through Miniflare
// Drizzle imports Neon's, PlanetScale's and PGlite's packages by their own names, so these jobs
// load the same copies under those names.
import ws from "ws";
import { args, callbackTx, notRun, requireFrom, runJob, seedSql, statements, type Shape } from "../lib/harness.ts";

const a = args();
const driver = a.driver!;
const target =
  {
    "node-postgres": "Postgres 14 (local)",
    "postgres-js": "Postgres 14 (local)",
    "neon-serverless": "Postgres 14 via local Neon proxy",
    "neon-http": "Postgres 14 via local Neon proxy",
    pglite: "PGlite (in-process)",
    "bun-sql": "Postgres 14 (local)",
    mysql2: a.target ?? "MySQL 8.4",
    "planetscale-serverless": "MySQL 8.4 via ps-http-sim",
    "better-sqlite3": "SQLite file",
    "bun-sqlite": "SQLite file",
    libsql: "SQLite file (file: URL)",
    d1: "D1 (Miniflare, local workerd)",
  }[driver] ?? driver;
const meta = { pkg: `drizzle-orm (${driver})`, version: a.version!, target };
const pgUrl = process.env.DM_PG_URL ?? "postgres://localhost:5432/postgres";
const libName: Record<string, string> = {
  "node-postgres": "pg",
  "postgres-js": "postgres",
  "neon-serverless": "@neondatabase/serverless",
  "neon-http": "@neondatabase/serverless",
  pglite: "@electric-sql/pglite",
  "bun-sql": "bun",
  mysql2: "mysql2/promise",
  "planetscale-serverless": "@planetscale/database",
  "better-sqlite3": "better-sqlite3",
  "bun-sqlite": "bun:sqlite",
  libsql: "@libsql/client",
  d1: "miniflare-4",
};

let drizzle: (client: unknown) => any;
let lib: any;
try {
  if ((driver === "bun-sql" || driver === "bun-sqlite") && !(globalThis as any).Bun) throw new Error(`drizzle-orm/${driver} needs the bun runtime`);
  drizzle = requireFrom(`${a.pkg!}/${driver}`).drizzle;
  lib = driver === "bun-sql" || driver === "bun-sqlite" || driver === "pglite" ? await import(driver === "pglite" ? requireFrom.resolve(libName[driver]!) : libName[driver]!) : requireFrom(libName[driver]!);
} catch (e) {
  notRun(meta, `cannot load drizzle-orm/${driver}: ${(e as Error).message.split("\n")[0]}`);
  process.exit(0);
}

// A transaction and a savepoint inside it, as Drizzle hands them out.
const transactions = (db: any) => {
  const begin = (fn: (tx: any) => Promise<unknown>) => db.transaction(fn);
  return callbackTx(begin, (tx, fn) => tx.transaction(fn));
};
// The database, a transaction and a savepoint inside it.
const all = (name: string, db: any): Shape[] => [
  { name, kind: "async", scope: (fn) => fn(db) },
  { name: "db.transaction(tx)", kind: "async", inTransaction: true, scope: (fn) => db.transaction((tx: any) => fn(tx)), tx: transactions(db) },
  { name: "tx.transaction(sp) (savepoint)", kind: "async", inTransaction: true, scope: (fn) => db.transaction((tx: any) => tx.transaction((sp: any) => fn(sp))) },
];

await runJob(meta, async () => {
  if (driver === "node-postgres") {
    const pool = new lib.Pool({ connectionString: pgUrl, max: 10 });
    const db = drizzle(pool);
    return {
      ...meta,
      dialect: "postgres",
      count: async (tag) => (await pool.query("select count(*)::int as n from driver_matrix.scratch where tag = $1", [tag])).rows[0].n,
      shapes: [
        { name: "drizzle(pool)", kind: "async", scope: (fn) => fn(db) },
        { name: "db.transaction(tx)", kind: "async", inTransaction: true, scope: (fn) => db.transaction((tx: any) => fn(tx)), tx: transactions(db) },
        { name: "tx.transaction(sp) (savepoint)", kind: "async", inTransaction: true, scope: (fn) => db.transaction((tx: any) => tx.transaction((sp: any) => fn(sp))) },
      ] satisfies Shape[],
      close: () => pool.end(),
    };
  }
  if (driver === "postgres-js") {
    const sql = lib(pgUrl, { max: 10, onnotice: () => {} });
    const db = drizzle(sql);
    return {
      ...meta,
      dialect: "postgres",
      count: async (tag) => Number((await sql.unsafe("select count(*)::int as n from driver_matrix.scratch where tag = $1", [tag]))[0].n),
      shapes: [
        { name: "drizzle(sql)", kind: "async", scope: (fn) => fn(db) },
        { name: "db.transaction(tx)", kind: "async", inTransaction: true, scope: (fn) => db.transaction((tx: any) => fn(tx)), tx: transactions(db) },
        { name: "tx.transaction(sp) (savepoint)", kind: "async", inTransaction: true, scope: (fn) => db.transaction((tx: any) => tx.transaction((sp: any) => fn(sp))) },
      ] satisfies Shape[],
      close: () => sql.end({ timeout: 2 }),
    };
  }
  if (driver === "mysql2") {
    const url = process.env.DM_MYSQL_URL ?? `mysql://root:porg@127.0.0.1:${a.port ?? "33406"}/driver_matrix`;
    const pool = lib.createPool({ uri: url, connectionLimit: 10 });
    const db = drizzle(pool);
    return {
      ...meta,
      dialect: "mysql",
      count: async (tag) => (await pool.query("select count(*) as n from scratch where tag = ?", [tag]))[0][0].n,
      shapes: [
        { name: "drizzle(pool)", kind: "async", scope: (fn) => fn(db) },
        { name: "db.transaction(tx)", kind: "async", inTransaction: true, scope: (fn) => db.transaction((tx: any) => fn(tx)), tx: transactions(db) },
        { name: "tx.transaction(sp) (savepoint)", kind: "async", inTransaction: true, scope: (fn) => db.transaction((tx: any) => tx.transaction((sp: any) => fn(sp))) },
      ] satisfies Shape[],
      close: () => pool.end(),
    };
  }
  if (driver === "neon-serverless" || driver === "neon-http") {
    // The proxy knows the endpoint by this host name; fetchEndpoint and wsProxy send every request
    // to the proxy's local port.
    const url = "postgres://postgres:postgres@db.localtest.me:5432/main";
    const proxy = process.env.DM_NEON_PROXY ?? "127.0.0.1:54444";
    lib.neonConfig.fetchEndpoint = () => `http://${proxy}/sql`;
    lib.neonConfig.useSecureWebSocket = false;
    lib.neonConfig.wsProxy = () => `${proxy}/v1`;
    lib.neonConfig.webSocketConstructor = ws;
    const pool = new lib.Pool({ connectionString: url, max: 10 });
    const count = async (tag: string) => (await pool.query("select count(*)::int as n from driver_matrix.scratch where tag = $1", [tag])).rows[0].n;
    if (driver === "neon-serverless") return { ...meta, dialect: "postgres", count, shapes: all("drizzle(pool)", drizzle(pool)), close: () => pool.end() };
    const db = drizzle(lib.neon(url));
    const noTx = "drizzle-orm/neon-http has no transaction: neon()'s transaction() takes an array of queries";
    return { ...meta, dialect: "postgres", count, shapes: [{ name: "drizzle(neon())", kind: "async", scope: (fn) => fn(db), skip: { S7: noTx } }] satisfies Shape[], close: () => pool.end() };
  }
  if (driver === "pglite") {
    const client = new lib.PGlite();
    await client.exec(seedSql("postgres"));
    return {
      ...meta,
      dialect: "postgres",
      count: async (tag) => (await client.query("select count(*)::int as n from driver_matrix.scratch where tag = $1", [tag])).rows[0].n,
      shapes: all("drizzle(pglite)", drizzle(client)),
      close: () => client.close(),
    };
  }
  if (driver === "bun-sql") {
    const sql = new lib.SQL(pgUrl, { max: 10 });
    return {
      ...meta,
      dialect: "postgres",
      count: async (tag) => Number((await sql.unsafe("select count(*)::int as n from driver_matrix.scratch where tag = $1", [tag]))[0].n),
      shapes: all("drizzle(sql)", drizzle(sql)),
      // Bun.sql decodes a timestamptz to a Date itself, and a year before 1 comes back an Invalid Date.
      skip: { S11b: "Bun.sql 1.4.3 decodes a timestamptz before 1 AD as an Invalid Date, which Reylo refuses (44ac8d4), so the instant never comes back to pass in" },
      close: () => sql.close(),
    };
  }
  if (driver === "planetscale-serverless") {
    const client = new lib.Client({ url: `http://root:porg@${process.env.DM_PS_SIM ?? "127.0.0.1:38080"}` });
    const literal = (s: string) => `'${s.replace(/'/g, "''")}'`;
    return {
      ...meta,
      dialect: "mysql",
      count: async (tag) => Number((await client.execute(`select count(*) as n from scratch where tag = ${literal(tag)}`)).rows[0].n),
      shapes: all("drizzle(client)", drizzle(client)),
    };
  }
  if (driver === "d1") {
    const mf = new lib.Miniflare({ modules: true, script: "export default {}", d1Databases: ["DB"] });
    const d1 = await mf.getD1Database("DB");
    await d1.batch(statements(seedSql("sqlite")).map((s: string) => d1.prepare(s)));
    const noTx = "D1 has no interactive transaction: batch() takes prepared statements, not a client";
    return {
      ...meta,
      dialect: "sqlite",
      count: async (tag) => (await d1.prepare("select count(*) as n from scratch where tag = ?").bind(tag).first()).n,
      shapes: [{ name: "drizzle(d1)", kind: "async", scope: (fn) => fn(drizzle(d1)), skip: { S7: noTx } }] satisfies Shape[],
      close: () => mf.dispose(),
    };
  }
  if (driver === "bun-sqlite") {
    // Drizzle's transaction is synchronous here, as bun:sqlite's is, so its function cannot await a
    // scenario; the database is the shape.
    const file = new lib.Database(a.file!);
    const counter = file.prepare("select count(*) as n from scratch where tag = ?");
    return {
      ...meta,
      dialect: "sqlite",
      count: async (tag) => counter.get(tag).n,
      shapes: [{ name: "drizzle(database)", kind: "sync", scope: (fn) => fn(drizzle(file)) }] satisfies Shape[],
      close: async () => file.close(),
    };
  }
  if (driver === "libsql") {
    // Inside a transaction Drizzle's libSQL session keeps the base client on client and the
    // transaction on tx, so S7 checks that reylo runs on the transaction.
    const client = lib.createClient({ url: `file:${a.file}` });
    const db = drizzle(client);
    return {
      ...meta,
      dialect: "sqlite",
      count: async (tag) => Number((await client.execute({ sql: "select count(*) as n from scratch where tag = ?", args: [tag] })).rows[0].n),
      shapes: [
        { name: "drizzle(client)", kind: "async", scope: (fn) => fn(db) },
        { name: "db.transaction(tx)", kind: "async", inTransaction: true, scope: (fn) => db.transaction((tx: any) => fn(tx)), tx: transactions(db) },
        { name: "tx.transaction(sp) (savepoint)", kind: "async", inTransaction: true, scope: (fn) => db.transaction((tx: any) => tx.transaction((sp: any) => fn(sp))) },
      ] satisfies Shape[],
      close: async () => client.close(),
    };
  }
  // better-sqlite3: Drizzle's transaction is synchronous there, as the driver's is, so its
  // function cannot await a scenario; the database is the shape.
  const file = new lib(a.file!);
  const db = drizzle(file);
  const counter = file.prepare("select count(*) as n from scratch where tag = ?");
  return {
    ...meta,
    dialect: "sqlite",
    count: async (tag) => counter.get(tag).n,
    shapes: [{ name: "drizzle(database)", kind: "sync", scope: (fn) => fn(db) }] satisfies Shape[],
    close: async () => file.close(),
  };
});
