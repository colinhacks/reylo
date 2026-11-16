// Bun.sql under Bun, with each of its three adapters:
//   --adapter postgres   the shared local Postgres
//   --adapter mysql      --url mysql://... (MySQL 8.4 or 8.0 container)
//   --adapter sqlite     --file <path> (a fresh database file the runner seeded)
import { args, notRun, runJob, type Dialect } from "../lib/harness.ts";
import { taggedShapes } from "../lib/tagged.ts";

const a = args();
const adapter = a.adapter as Dialect;
const Bun = (globalThis as any).Bun;
const target = adapter === "postgres" ? "Postgres 14 (local)" : adapter === "mysql" ? a.target! : "SQLite file";
const meta = { pkg: `Bun.sql (${adapter})`, version: Bun?.version ?? "?", target };

if (!Bun) {
  notRun(meta, "Bun.sql needs the bun runtime");
  process.exit(0);
}
const { SQL } = await import("bun");

await runJob(meta, async () => {
  const sql =
    adapter === "postgres"
      ? new SQL(process.env.DM_PG_URL ?? "postgres://localhost:5432/postgres", { max: 10 })
      : adapter === "mysql"
        ? // Since Bun 1.4 the MySQL adapter refuses to fetch the server's RSA key for
          // caching_sha2_password over a connection without TLS unless asked to, as mysql2 does
          // unasked; the containers serve no TLS.
          new SQL(a.url!, { max: 10, allowPublicKeyRetrieval: true })
        : new SQL({ adapter: "sqlite", filename: a.file! });
  const table = adapter === "sqlite" ? "scratch" : "driver_matrix.scratch";
  const ph = adapter === "postgres" ? "$1" : "?";
  return {
    ...meta,
    dialect: adapter,
    count: async (tag) => Number((await sql.unsafe(`select count(*) as n from ${table} where tag = ${ph}`, [tag]))[0].n),
    shapes: taggedShapes(sql, { reserve: adapter !== "sqlite" }),
    close: () => sql.close(),
  };
});
