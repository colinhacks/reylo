// Runs every driver job in its own process (Node 26.7 by default, Node 24.19 for the second
// node:sqlite row, Bun for Bun.sql and bun:sqlite), a few at a time, and writes results.json and
// RESULTS.md. Usage: node runner.ts [--only <substring>] [--jobs <n>]
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { seedSqliteFile } from "./lib/seed-sqlite.ts";
import type { Result } from "./lib/harness.ts";

const root = new URL(".", import.meta.url).pathname;
const repo = join(root, "../..");
const argv = process.argv.slice(2);
const only = argv.includes("--only") ? argv[argv.indexOf("--only") + 1] : undefined;
const parallel = argv.includes("--jobs") ? Number(argv[argv.indexOf("--jobs") + 1]) : Number(process.env.DM_JOBS ?? 4);
const JOB_TIMEOUT = Number(process.env.DM_JOB_TIMEOUT ?? 300000);

const NODE26 = process.env.DM_NODE26 ?? join(homedir(), ".nvm/versions/node/v26.7.0/bin/node");
const NODE24 = process.env.DM_NODE24 ?? join(homedir(), ".nvm/versions/node/v24.19.0/bin/node");
const BUN = process.env.DM_BUN ?? "bun";

type Spec = {
  id: string;
  bin: string;
  file: string;
  args: string[];
  /** Seed a fresh SQLite file and pass it as --file. */
  sqlite?: boolean;
  control?: boolean;
  /** For the rows the runner writes itself when the job cannot start. */
  meta: { pkg: string; version: string; target: string; runtime: string };
};

const n26 = "node 26.7.0";
const specs: Spec[] = [];
const add = (s: Spec) => specs.push(s);

// Postgres
for (const [alias, v] of [["pg-8", "8.23.0"], ["pg-7", "7.18.2"]]) add({ id: `pg@${v}`, bin: NODE26, file: "drivers/pg.ts", args: ["--pkg", alias, "--version", v], meta: { pkg: "pg", version: v, target: "Postgres 14 (local)", runtime: n26 } });
add({ id: "postgres@3.4.9", bin: NODE26, file: "drivers/postgresjs.ts", args: ["--pkg", "postgres-3", "--version", "3.4.9"], meta: { pkg: "postgres", version: "3.4.9", target: "Postgres 14 (local)", runtime: n26 } });
for (const [alias, v] of [["pglite-05", "0.5.8"], ["pglite-04", "0.4.6"], ["pglite-03", "0.3.16"], ["pglite-02", "0.2.17"]]) add({ id: `pglite@${v}`, bin: NODE26, file: "drivers/pglite.ts", args: ["--pkg", alias, "--version", v], meta: { pkg: "@electric-sql/pglite", version: v, target: "PGlite (in-process)", runtime: n26 } });
for (const [alias, v] of [["neon-1", "1.1.0"], ["neon-010", "0.10.4"], ["neon-09", "0.9.5"]]) add({ id: `neon@${v}`, bin: NODE26, file: "drivers/neon.ts", args: ["--pkg", alias, "--version", v], meta: { pkg: "@neondatabase/serverless", version: v, target: "Postgres 14 via local Neon proxy", runtime: n26 } });
add({ id: "pg-promise@12.7.1", bin: NODE26, file: "drivers/pgpromise.ts", args: ["--pkg", "pg-promise", "--version", "12.7.1"], meta: { pkg: "pg-promise", version: "12.7.1", target: "Postgres 14 (local)", runtime: n26 } });
add({ id: "bun-sql-postgres", bin: BUN, file: "drivers/bunsql.ts", args: ["--adapter", "postgres"], meta: { pkg: "Bun.sql (postgres)", version: "1.3.14", target: "Postgres 14 (local)", runtime: "bun" } });

// MySQL family
const mysqlServers = [
  ["33406", "MySQL 8.4"],
  ["33380", "MySQL 8.0"],
];
for (const [alias, v] of [["mysql2-3", "3.24.5"], ["mysql2-315", "3.15.3"], ["mysql2-2", "2.3.3"]]) {
  for (const [port, target] of mysqlServers) add({ id: `mysql2@${v}/${target}`, bin: NODE26, file: "drivers/mysql2.ts", args: ["--pkg", alias, "--version", v, "--port", port!, "--target", target!], meta: { pkg: "mysql2", version: v, target: target!, runtime: n26 } });
}
for (const [alias, v] of [["mariadb-3", "3.5.4"], ["mariadb-34", "3.4.5"], ["mariadb-2", "2.5.6"]]) {
  for (const [port, target] of [["33406", "MySQL 8.4"]]) add({ id: `mariadb@${v}/${target}`, bin: NODE26, file: "drivers/mariadb.ts", args: ["--pkg", alias, "--version", v, "--port", port!, "--target", target!], meta: { pkg: "mariadb", version: v, target: target!, runtime: n26 } });
}
for (const [port, target] of mysqlServers) add({ id: `bun-sql-mysql/${target}`, bin: BUN, file: "drivers/bunsql.ts", args: ["--adapter", "mysql", "--url", `mysql://root:porg@127.0.0.1:${port}/driver_matrix`, "--target", target!], meta: { pkg: "Bun.sql (mysql)", version: "1.3.14", target: target!, runtime: "bun" } });
for (const [alias, v] of [["planetscale-2", "2.0.0"], ["planetscale-1", "1.20.2"]]) add({ id: `planetscale@${v}`, bin: NODE26, file: "drivers/planetscale.ts", args: ["--pkg", alias, "--version", v], meta: { pkg: "@planetscale/database", version: v, target: "MySQL 8.4 via ps-http-sim", runtime: n26 } });

// Drizzle over each of its drivers, which run on the driver packages installed under their own names.
const drizzleVersion = "0.45.4";
for (const driver of ["node-postgres", "postgres-js"]) add({ id: `drizzle-orm@${drizzleVersion}/${driver}`, bin: NODE26, file: "drivers/drizzle.ts", args: ["--pkg", "drizzle-orm", "--version", drizzleVersion, "--driver", driver], meta: { pkg: `drizzle-orm (${driver})`, version: drizzleVersion, target: "Postgres 14 (local)", runtime: n26 } });
add({ id: `drizzle-orm@${drizzleVersion}/mysql2`, bin: NODE26, file: "drivers/drizzle.ts", args: ["--pkg", "drizzle-orm", "--version", drizzleVersion, "--driver", "mysql2", "--port", "33406", "--target", "MySQL 8.4"], meta: { pkg: "drizzle-orm (mysql2)", version: drizzleVersion, target: "MySQL 8.4", runtime: n26 } });
add({ id: `drizzle-orm@${drizzleVersion}/better-sqlite3`, bin: NODE26, file: "drivers/drizzle.ts", args: ["--pkg", "drizzle-orm", "--version", drizzleVersion, "--driver", "better-sqlite3"], sqlite: true, meta: { pkg: "drizzle-orm (better-sqlite3)", version: drizzleVersion, target: "SQLite file", runtime: n26 } });
add({ id: `drizzle-orm@${drizzleVersion}/libsql`, bin: NODE26, file: "drivers/drizzle.ts", args: ["--pkg", "drizzle-orm", "--version", drizzleVersion, "--driver", "libsql"], sqlite: true, meta: { pkg: "drizzle-orm (libsql)", version: drizzleVersion, target: "SQLite file (file: URL)", runtime: n26 } });
const drizzleJob = (driver: string, target: string, opts: { bun?: boolean; sqlite?: boolean } = {}) =>
  add({ id: `drizzle-orm@${drizzleVersion}/${driver}`, bin: opts.bun ? BUN : NODE26, file: "drivers/drizzle.ts", args: ["--pkg", "drizzle-orm", "--version", drizzleVersion, "--driver", driver], sqlite: opts.sqlite, meta: { pkg: `drizzle-orm (${driver})`, version: drizzleVersion, target, runtime: opts.bun ? "bun" : n26 } });
drizzleJob("neon-serverless", "Postgres 14 via local Neon proxy");
drizzleJob("neon-http", "Postgres 14 via local Neon proxy");
drizzleJob("pglite", "PGlite (in-process)");
drizzleJob("bun-sql", "Postgres 14 (local)", { bun: true });
drizzleJob("planetscale-serverless", "MySQL 8.4 via ps-http-sim");
drizzleJob("bun-sqlite", "SQLite file", { bun: true, sqlite: true });
drizzleJob("d1", "D1 (Miniflare, local workerd)");

// SQLite
for (const [alias, v] of [["bs3-13", "13.0.3"], ["bs3-12", "12.11.1"]]) add({ id: `better-sqlite3@${v}`, bin: NODE26, file: "drivers/sync-sqlite.ts", args: ["--lib", alias, "--version", v], sqlite: true, meta: { pkg: "better-sqlite3", version: v, target: "SQLite file", runtime: n26 } });
// These lines have no prebuilt binary for Node 24 or 26, and their source does not compile against Node 26's V8
// (11.10.0 calls Object::GetPrototype, Context::GetIsolate and PropertyCallbackInfo::This, which V8 removed), so
// they run on Node 24, the other Node reylo supports, from a source build in native-old/ against Node 24's headers.
for (const [alias, v] of [["bs3-11", "11.10.0"], ["bs3-9", "9.6.0"], ["bs3-8", "8.7.0"]]) add({ id: `better-sqlite3@${v}`, bin: NODE24, file: "drivers/sync-sqlite.ts", args: ["--lib", alias, "--version", v, "--from", "native-old"], sqlite: true, meta: { pkg: "better-sqlite3", version: v, target: "SQLite file", runtime: "node 24.19.0" } });
add({ id: "node:sqlite@26", bin: NODE26, file: "drivers/sync-sqlite.ts", args: ["--lib", "node:sqlite"], sqlite: true, meta: { pkg: "node:sqlite", version: "26.7.0", target: "SQLite file", runtime: n26 } });
add({ id: "node:sqlite@24", bin: NODE24, file: "drivers/sync-sqlite.ts", args: ["--lib", "node:sqlite"], sqlite: true, meta: { pkg: "node:sqlite", version: "24.19.0", target: "SQLite file", runtime: "node 24.19.0" } });
add({ id: "bun:sqlite", bin: BUN, file: "drivers/sync-sqlite.ts", args: ["--lib", "bun:sqlite"], sqlite: true, meta: { pkg: "bun:sqlite", version: "1.3.14", target: "SQLite file", runtime: "bun" } });
add({ id: "bun-sql-sqlite", bin: BUN, file: "drivers/bunsql.ts", args: ["--adapter", "sqlite"], sqlite: true, meta: { pkg: "Bun.sql (sqlite)", version: "1.3.14", target: "SQLite file", runtime: "bun" } });
for (const [alias, v] of [["libsql-017", "0.17.4"], ["libsql-018", "0.18.0"], ["libsql-015", "0.15.15"], ["libsql-014", "0.14.0"]]) add({ id: `libsql@${v}`, bin: NODE26, file: "drivers/libsql.ts", args: ["--pkg", alias, "--version", v], sqlite: true, meta: { pkg: "@libsql/client", version: v, target: "SQLite file (file: URL)", runtime: n26 } });
for (const [alias, v] of [["turso-08", "0.8.3"], ["turso-07", "0.7.2"], ["turso-06", "0.6.1"], ["turso-04", "0.4.4"]]) {
  add({ id: `turso@${v}`, bin: NODE26, file: "drivers/turso.ts", args: ["--pkg", alias, "--version", v, "--api", "promise"], sqlite: true, meta: { pkg: "@tursodatabase/database", version: v, target: "SQLite file (Turso's engine)", runtime: n26 } });
  add({ id: `turso-compat@${v}`, bin: NODE26, file: "drivers/turso.ts", args: ["--pkg", alias, "--version", v, "--api", "compat"], sqlite: true, meta: { pkg: "@tursodatabase/database/compat", version: v, target: "SQLite file (Turso's engine)", runtime: n26 } });
}
for (const [alias, v] of [["sqlite3-5", "5.1.7"], ["sqlite3-6", "6.0.1"]]) {
  add({ id: `sqlite3@${v}`, bin: NODE26, file: "drivers/sqlite3.ts", args: ["--pkg", alias, "--version", v], sqlite: true, meta: { pkg: "sqlite3", version: v, target: "SQLite file", runtime: n26 } });
  add({ id: `sqlite@5.1.1/sqlite3@${v}`, bin: NODE26, file: "drivers/sqlite3.ts", args: ["--wrapper", "sqlite", "--pkg", alias, "--version", "5.1.1", "--driverVersion", v], sqlite: true, meta: { pkg: "sqlite", version: "5.1.1", target: `SQLite file (over sqlite3 ${v})`, runtime: n26 } });
}
add({ id: "d1/miniflare@4.20260730.0", bin: NODE26, file: "drivers/d1.ts", args: ["--pkg", "miniflare-4", "--version", "4.20260730.0"], meta: { pkg: "miniflare D1", version: "4.20260730.0", target: "D1 (Miniflare, local workerd)", runtime: n26 } });

// Failing controls
for (const c of ["pg", "sqlite"]) add({ id: `control/${c}`, bin: NODE26, file: "drivers/control.ts", args: ["--case", c], control: true, meta: { pkg: "control", version: c, target: "", runtime: n26 } });

// ---------------------------------------------------------------------------------------------

const outDir = join(root, ".out");
const dataDir = join(root, ".data");
mkdirSync(outDir, { recursive: true });
mkdirSync(dataDir, { recursive: true });
const slug = (id: string) => id.replace(/[^A-Za-z0-9.@_-]+/g, "_");

type JobRun = { id: string; exit: number | null; signal: string | null; ms: number; timedOut: boolean; stderr: string; control: boolean };

function nativeOldMissing(spec: Spec): string | null {
  const i = spec.args.indexOf("--from");
  if (i === -1) return null;
  const alias = spec.args[spec.args.indexOf("--lib") + 1]!;
  const binary = join(root, spec.args[i + 1]!, "node_modules", alias, "build/Release/better_sqlite3.node");
  if (existsSync(binary)) return null;
  let abi = "?";
  try {
    abi = execFileSync(spec.bin, ["-p", "process.versions.modules"], { encoding: "utf8" }).trim();
  } catch {}
  return `no prebuilt binary for Node ${spec.meta.runtime.replace("node ", "")} (ABI ${abi}), and native-old/ has no source build of it against that Node's headers (see README)`;
}

function runOne(spec: Spec): Promise<JobRun> {
  const out = join(outDir, `${slug(spec.id)}.jsonl`);
  rmSync(out, { force: true });
  const missing = nativeOldMissing(spec);
  if (missing) {
    writeFileSync(out, JSON.stringify({ ...spec.meta, shape: "(job)", scenario: "setup", status: "not run", live: true, error: missing } satisfies Result) + "\n");
    return Promise.resolve({ id: spec.id, exit: 0, signal: null, ms: 0, timedOut: false, stderr: "", control: !!spec.control });
  }
  const args = [spec.file, ...spec.args];
  if (spec.sqlite) {
    const file = join(dataDir, `${slug(spec.id)}.db`);
    seedSqliteFile(file);
    args.push("--file", file);
  }
  const started = Date.now();
  return new Promise((resolve) => {
    let stderr = "";
    let stdout = "";
    const child = spawn(spec.bin, args, { cwd: root, env: { ...process.env, DM_OUT: out }, stdio: ["ignore", "pipe", "pipe"] });
    child.stdout.on("data", (d) => (stdout += d));
    child.stderr.on("data", (d) => (stderr += d));
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGKILL");
    }, JOB_TIMEOUT);
    child.on("error", (e) => {
      stderr += String(e);
    });
    child.on("close", (exit, signal) => {
      clearTimeout(timer);
      writeFileSync(join(outDir, `${slug(spec.id)}.log`), stdout + (stderr ? `\n--- stderr ---\n${stderr}` : ""));
      const ms = Date.now() - started;
      const fails = (stdout.match(/^FAIL /gm) ?? []).length;
      console.log(`${fails ? "FAIL" : "done"} ${spec.id} (${(ms / 1000).toFixed(1)} s${fails ? `, ${fails} failing` : ""}${timedOut ? ", killed after timeout" : ""}${exit ? `, exit ${exit}` : ""})`);
      resolve({ id: spec.id, exit, signal, ms, timedOut, stderr: stderr.slice(-2000), control: !!spec.control });
    });
  });
}

async function pool<T, R>(items: T[], n: number, fn: (t: T) => Promise<R>): Promise<R[]> {
  const results: R[] = new Array(items.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: Math.min(n, items.length) }, async () => {
      while (next < items.length) {
        const i = next++;
        results[i] = await fn(items[i]!);
      }
    }),
  );
  return results;
}

const selected = only ? specs.filter((s) => s.id.includes(only)) : specs;
console.log(`running ${selected.length} jobs, ${parallel} at a time`);
const runs = await pool(selected, parallel, runOne);

// Collect every record, and a row for each job that ended without finishing.
const results: (Result & { control?: boolean; job: string })[] = [];
for (const [i, spec] of selected.entries()) {
  const run = runs[i]!;
  const file = join(outDir, `${slug(spec.id)}.jsonl`);
  const lines = existsSync(file) ? readFileSync(file, "utf8").trim().split("\n").filter(Boolean) : [];
  let done = false;
  for (const line of lines) {
    const r = JSON.parse(line);
    if (r.done) done = true;
    else results.push({ ...r, job: spec.id, ...(spec.control ? { control: true } : {}) });
  }
  const reported = lines.some((l) => l.includes('"scenario":"setup"'));
  if (!done && !reported) {
    const why = run.timedOut ? `the process was killed after ${JOB_TIMEOUT / 1000} s` : `the process exited with ${run.signal ?? `code ${run.exit}`} before finishing`;
    const tail = run.stderr.trim().split("\n").slice(-3).join(" / ");
    results.push({ ...spec.meta, job: spec.id, shape: "(job)", scenario: "process", status: "fail", live: true, error: `${why}${tail ? `: ${tail}` : ""}` });
  }
}

// ---------------------------------------------------------------------------------------------
// results.json

const hash = (p: string) => createHash("sha256").update(readFileSync(join(repo, p))).digest("hex").slice(0, 12);
let head = "?";
let dirty = "?";
try {
  head = execFileSync("git", ["-C", repo, "rev-parse", "--short", "HEAD"], { encoding: "utf8" }).trim();
  dirty = execFileSync("git", ["-C", repo, "status", "--porcelain", "--", "src"], { encoding: "utf8" }).trim() ? "yes" : "no";
} catch {}
const typecheckFile = join(root, "typecheck/results.json");
const typecheck = existsSync(typecheckFile) ? JSON.parse(readFileSync(typecheckFile, "utf8")) : null;
const meta = {
  generated: new Date().toISOString(),
  src: { head, uncommittedChangesInSrc: dirty, "execute.ts": hash("src/execute.ts"), "index.ts": hash("src/index.ts") },
  runtimes: { node26: NODE26, node24: NODE24, bun: BUN },
};
if (!only) writeFileSync(join(root, "results.json"), JSON.stringify({ ...meta, jobs: runs, results, typecheck }, null, 2) + "\n");

// ---------------------------------------------------------------------------------------------
// RESULTS.md

const cell = (s: unknown) => String(s ?? "").replace(/\|/g, "\\|").replace(/\s+/g, " ").trim();
const real = results.filter((r) => !r.control);
const controls = results.filter((r) => r.control);
const count = (rs: typeof results, s: string) => rs.filter((r) => r.status === s).length;
const md: string[] = [];
md.push("# Driver matrix results", "");
md.push(`Generated ${meta.generated} by \`./run.sh\` against the working tree at ${head} (uncommitted changes in src/: ${dirty}; src/execute.ts ${meta.src["execute.ts"]}, src/index.ts ${meta.src["index.ts"]}). The report and the diagnoses are in [README.md](README.md).`, "");
md.push(`${real.length} rows: ${count(real, "pass")} pass, ${count(real, "fail")} fail, ${count(real, "unsupported")} unsupported (reylo refuses them with its own TypeError, as its contract says), ${count(real, "skip")} skip, ${count(real, "not run")} not run. Failing controls: ${count(controls, "fail")} of ${controls.length} control rows fail, as designed.`, "");

md.push("## Per job", "");
md.push("| Package@version | Runtime | Target | Pass | Fail | Unsupported | Skip | Not run |", "| --- | --- | --- | --- | --- | --- | --- | --- |");
const jobs = [...new Set(real.map((r) => r.job))];
for (const j of jobs) {
  const rs = real.filter((r) => r.job === j);
  const r0 = rs[0]!;
  md.push(`| ${cell(`${r0.pkg}@${r0.version}`)} | ${cell(r0.runtime)} | ${cell(r0.target)} | ${count(rs, "pass")} | ${count(rs, "fail")} | ${count(rs, "unsupported")} | ${count(rs, "skip")} | ${count(rs, "not run")} |`);
}
md.push("");

md.push("## Failures, grouped", "", "One row per package, shape and set of failing scenarios, across the versions, runtimes and targets that fail the same way. The error is the one of the first failing scenario; every row is in the last table.", "");
md.push("| Package | Versions | Runtimes | Targets | Shape | Failing scenarios | First error |", "| --- | --- | --- | --- | --- | --- | --- |");
const byJobShape = new Map<string, Result[]>();
for (const r of real.filter((r) => r.status === "fail" || r.status === "not run")) {
  const key = [r.job, r.shape].join("\u0000");
  byJobShape.set(key, [...(byJobShape.get(key) ?? []), r]);
}
const merged = new Map<string, { r: Result; scenarios: string; versions: Set<string>; runtimes: Set<string>; targets: Set<string> }>();
for (const rs of byJobShape.values()) {
  const r = rs[0]!;
  const scenarios = rs.map((x) => x.scenario.split(" ")[0]!).join(", ");
  const key = [r.pkg, r.shape, scenarios, r.status].join("\u0000");
  const g = merged.get(key) ?? { r, scenarios, versions: new Set(), runtimes: new Set(), targets: new Set() };
  g.versions.add(r.version);
  g.runtimes.add(r.runtime);
  g.targets.add(r.target);
  merged.set(key, g);
}
const list = (s: Set<string>) => [...s].join(", ");
for (const g of merged.values()) md.push(`| ${cell(g.r.pkg)} | ${cell(list(g.versions))} | ${cell(list(g.runtimes))} | ${cell(list(g.targets))} | ${cell(g.r.shape)} | ${cell(g.scenarios)} | ${cell(g.r.status === "not run" ? `not run: ${g.r.error}` : g.r.error)} |`);
md.push("");

md.push("## Unsupported by design", "", "Rows where reylo refuses the client or the statement with its own TypeError, which is its contract for that shape: the row checks the refusal (its message, and for PlanetScale 2.0 that its cause is the driver's error).", "");
md.push("| Package | Versions | Runtimes | Shape | Scenarios | reylo's refusal |", "| --- | --- | --- | --- | --- | --- |");
const unsup = new Map<string, { r: Result; scenarios: Set<string>; versions: Set<string>; runtimes: Set<string> }>();
for (const r of real.filter((r) => r.status === "unsupported")) {
  const key = [r.pkg, r.shape].join("\u0000");
  const g = unsup.get(key) ?? { r, scenarios: new Set(), versions: new Set(), runtimes: new Set() };
  g.scenarios.add(r.scenario.split(" ")[0]!);
  g.versions.add(r.version);
  g.runtimes.add(r.runtime);
  unsup.set(key, g);
}
for (const g of unsup.values()) md.push(`| ${cell(g.r.pkg)} | ${cell([...g.versions].join(", "))} | ${cell([...g.runtimes].join(", "))} | ${cell(g.r.shape)} | ${cell([...g.scenarios].join(", "))} | ${cell(g.r.detail)} |`);
md.push("");

md.push("## Failing controls", "", "Deliberately broken clients (drivers/control.ts). Every row marked FAIL here is a scenario that caught the breakage; a pass is a scenario the breakage does not touch.", "");
md.push("| Control | Shape | Scenario | Result | Error |", "| --- | --- | --- | --- | --- |");
for (const r of controls) md.push(`| ${cell(`${r.pkg}/${r.version}`)} | ${cell(r.shape)} | ${cell(r.scenario)} | ${r.status.toUpperCase()} | ${cell(r.error)} |`);
md.push("");

if (typecheck) {
  md.push("## Type check", "", `\`tsc\` ${cell(typecheck.tsc)} over typecheck/cases.ts: every client and transaction object passed to a query's run (ClientFor<D>), and the type run returns. Details in [typecheck/RESULTS.md](typecheck/RESULTS.md).`, "");
  md.push(`${typecheck.summary}`, "");
}

md.push("## Every row", "");
md.push("| Package@version | Runtime | Target | Shape | Scenario | Result | Error or detail |", "| --- | --- | --- | --- | --- | --- | --- |");
for (const r of real) md.push(`| ${cell(`${r.pkg}@${r.version}`)} | ${cell(r.runtime)} | ${cell(r.target)} | ${cell(r.shape)} | ${cell(r.scenario)} | ${r.status === "pass" ? "pass" : r.status.toUpperCase()} | ${cell(r.error ?? r.detail ?? "")} |`);
md.push("");
if (!only) writeFileSync(join(root, "RESULTS.md"), md.join("\n"));

console.log(`\n${real.length} rows: ${count(real, "pass")} pass, ${count(real, "fail")} fail, ${count(real, "unsupported")} unsupported, ${count(real, "skip")} skip, ${count(real, "not run")} not run; controls ${count(controls, "fail")}/${controls.length} failing as designed`);
if (only) console.log("(--only: results.json and RESULTS.md were not rewritten)");
