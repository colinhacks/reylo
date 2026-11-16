// Makes the databases from reylo/migrations, starting over: the development database the type check
// reads (reylo migrate deploy), and the local D1 database wrangler keeps (wrangler d1 migrations
// apply), each with the tables and rows of the migration.
import { rmSync } from "node:fs";
import { spawnSync } from "node:child_process";

rmSync("dev.sqlite", { force: true });
rmSync(".wrangler/state/v3/d1", { recursive: true, force: true });
for (const command of ["reylo migrate deploy", "wrangler d1 migrations apply DB --local"]) {
  const run = spawnSync(command, { shell: true, encoding: "utf8" });
  if (run.status !== 0) {
    process.stderr.write(run.stdout + run.stderr);
    process.exit(run.status ?? 1);
  }
}
console.log("created users and posts");
