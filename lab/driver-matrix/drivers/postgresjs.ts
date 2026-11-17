// porsager/postgres against the shared local Postgres: sql, sql.begin, tx.savepoint, sql.reserve.
import { args, notRun, requireFrom, runJob } from "../lib/harness.ts";
import { taggedShapes } from "../lib/tagged.ts";

const a = args();
const meta = { pkg: "postgres", version: a.version!, target: "Postgres 14 (local)" };
const url = process.env.DM_PG_URL ?? "postgres://localhost:5432/postgres";

let postgres: any;
try {
  // 4.0 and later are ES modules only, which require() gives as their namespace.
  const m = requireFrom(a.pkg!);
  postgres = m.default ?? m;
} catch (e) {
  notRun(meta, `cannot load ${a.pkg}: ${(e as Error).message}`);
  process.exit(0);
}

await runJob(meta, async () => {
  const sql = postgres(url, { max: 10, onnotice: () => {} });
  return {
    ...meta,
    dialect: "postgres",
    count: async (tag) => Number((await sql.unsafe("select count(*)::int as n from driver_matrix.scratch where tag = $1", [tag]))[0].n),
    shapes: taggedShapes(sql, { reserve: true }),
    close: () => sql.end({ timeout: 2 }),
  };
});
