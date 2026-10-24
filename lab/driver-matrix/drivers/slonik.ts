// Slonik against the shared local Postgres, with its own type parsers left on: the pool, a
// connection from pool.connect(), a transaction, and a transaction inside it (a savepoint).
import { args, callbackTx, notRun, requireFrom, runJob } from "../lib/harness.ts";

const a = args();
const meta = { pkg: "slonik", version: a.version!, target: "Postgres 14 (local)" };
const url = process.env.DM_PG_URL ?? "postgres://localhost:5432/postgres";

let slonik: any;
try {
  slonik = await import(requireFrom.resolve(a.pkg!));
} catch (e) {
  notRun(meta, `cannot load ${a.pkg}: ${(e as Error).message}`);
  process.exit(0);
}

await runJob(meta, async () => {
  const { createPool, sql } = slonik;
  const pool = await createPool(url, { maximumPoolSize: 10 });
  const begin = (fn: (t: any) => Promise<unknown>) => pool.transaction(fn);
  return {
    ...meta,
    dialect: "postgres",
    // Slonik's sql tag binds each use of a parameter on its own, so reylo refuses a statement that
    // uses one twice, which only a query defined by hand can (tests/execute.test.ts checks it).
    skip: { S5a: "a parameter used twice is refused on Slonik", S5b: "a parameter used twice is refused on Slonik" },
    count: async (tag) => pool.oneFirst(sql.unsafe`select count(*)::int as n from driver_matrix.scratch where tag = ${tag}`),
    shapes: [
      { name: "pool", kind: "async", scope: (fn) => fn(pool) },
      { name: "pool.connect(c)", kind: "async", scope: (fn) => pool.connect((c: any) => fn(c)) },
      {
        name: "pool.transaction(tx)",
        kind: "async",
        inTransaction: true,
        scope: (fn) => begin((t) => fn(t)),
        tx: callbackTx(begin, (t, fn) => t.transaction(fn)),
      },
      {
        name: "tx.transaction(sp) (savepoint)",
        kind: "async",
        inTransaction: true,
        scope: (fn) => begin((t) => t.transaction((sp: any) => fn(sp))),
        tx: callbackTx(
          (fn) => begin((t) => t.transaction(fn)),
          (sp, fn) => sp.transaction(fn),
        ),
      },
    ],
    close: () => pool.end(),
  };
});
