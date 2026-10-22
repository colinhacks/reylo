// Drizzle over each of its drivers (--driver): the database `drizzle(client)` gives, a transaction
// it hands out, and a savepoint inside that transaction. reylo runs on the driver client each one
// keeps on `session.client`, so the scenarios are the driver's own:
//   --driver node-postgres   over pg, against the shared local Postgres
//   --driver postgres-js     over postgres.js, the same server
//   --driver mysql2          over mysql2's promise pool, against a MySQL container (--port)
//   --driver better-sqlite3  over better-sqlite3, on the database file the runner seeded (--file)
//   --driver libsql          over @libsql/client, on that file through a file: URL
import { args, callbackTx, notRun, requireFrom, runJob, type Shape } from "../lib/harness.ts";

const a = args();
const driver = a.driver!;
const target = { "node-postgres": "Postgres 14 (local)", "postgres-js": "Postgres 14 (local)", mysql2: a.target ?? "MySQL 8.4", "better-sqlite3": "SQLite file", libsql: "SQLite file (file: URL)" }[driver] ?? driver;
const meta = { pkg: `drizzle-orm (${driver})`, version: a.version!, target };
const pgUrl = process.env.DM_PG_URL ?? "postgres://localhost:5432/postgres";

let drizzle: (client: unknown) => any;
let lib: any;
try {
  drizzle = requireFrom(`${a.pkg!}/${driver}`).drizzle;
  lib = requireFrom({ "node-postgres": "pg", "postgres-js": "postgres", mysql2: "mysql2/promise", "better-sqlite3": "better-sqlite3", libsql: "@libsql/client" }[driver]!);
} catch (e) {
  notRun(meta, `cannot load drizzle-orm/${driver}: ${(e as Error).message.split("\n")[0]}`);
  process.exit(0);
}

// A transaction and a savepoint inside it, as Drizzle hands them out.
const transactions = (db: any) => {
  const begin = (fn: (tx: any) => Promise<unknown>) => db.transaction(fn);
  return callbackTx(begin, (tx, fn) => tx.transaction(fn));
};

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
