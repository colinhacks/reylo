// Runs every tool on every probe it can take and writes results.json and RESULTS.md. `--only <id>` runs one tool, and merges it into the existing results.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { ALL_PROBES, CARDINALITY, EXPRESSIONS, FROM_THE_DATABASE, JSON_SCHEMA, JSON_SHAPES, OUTER_JOINS, PARAMETER_NULLABILITY, PARAMETER_TYPES, SMOKE, sqlFor, type Probe } from "./probes.ts";
import { gradeTool } from "./grade.ts";
import type { Answer, Statement, Tool, Urls } from "./tools/lib.ts";

const here = import.meta.dirname;
const envFile = resolve(here, ".out/urls.env");
const env = existsSync(envFile) ? Object.fromEntries(readFileSync(envFile, "utf8").split("\n").filter(Boolean).map((l) => l.split(/=(.*)/s).slice(0, 2))) : {};
const urls: Urls = { postgres: env.COMPARE_PG, mysql: env.COMPARE_MYSQL, sqlite: env.COMPARE_SQLITE };
const dead: Urls = { postgres: "postgres://postgres@127.0.0.1:1/none", mysql: "mysql://root@127.0.0.1:1/none", sqlite: "/nonexistent/none.db" };

const GROUPS: [string, Probe[]][] = [["outer-joins", OUTER_JOINS], ["expressions", EXPRESSIONS], ["parameter-types", PARAMETER_TYPES], ["parameter-nullability", PARAMETER_NULLABILITY], ["cardinality", CARDINALITY], ["json-shapes", JSON_SHAPES], ["json-schema", JSON_SCHEMA], ["from-the-database", FROM_THE_DATABASE], ["smoke", [SMOKE]]];

const toolIds = (process.argv.includes("--only") ? process.argv[process.argv.indexOf("--only") + 1].split(",") : ["reylo", "sqlc", "sqlx", "pgtyped", "typedsql", "safeql", "sqlxts", "typesql", "postgrestypedsql"]);
const results: any = existsSync(resolve(here, "results.json")) ? JSON.parse(readFileSync(resolve(here, "results.json"), "utf8")) : { tools: {} };
// `--report` writes the reports from the results already held, and runs nothing.
const reportOnly = process.argv.includes("--report");
if (!reportOnly) results.at = new Date().toISOString();
for (const id of reportOnly ? [] : toolIds) {
  const tool: Tool = (await import(`./tools/${id}.ts`))[id];
  const answers: Record<string, Answer> = {};
  let wrote: string[] = [];
  const t0 = Date.now();
  for (const db of tool.dbs) {
    const statements: Statement[] = GROUPS.flatMap(([group, probes]) => probes.flatMap((probe) => { const sql = sqlFor(probe, db); return sql ? [{ key: `${db}:${group}:${probe.id}`, probe, sql }] : []; }));
    const r = await tool.analyze(db, statements, urls);
    Object.assign(answers, r.answers);
    if (db === tool.dbs[0]) wrote = r.wrote;
  }
  // Does it need a database? The smoke statement again, pointed at nothing.
  const smokeSql = sqlFor(SMOKE, tool.dbs[0])!;
  const offline = await tool.analyze(tool.dbs[0], [{ key: "dead", probe: SMOKE, sql: smokeSql }], dead);
  const needsDatabase = !offline.answers.dead?.ok;
  // A tool that needs none may still have a setting that asks the database: it counts if it answers with a server and fails with none, and its answers are the ones graded for this row.
  let connects: "yes" | "opt-in" | "no" = needsDatabase ? "yes" : "no";
  if (!needsDatabase && tool.withDatabaseSetting) {
    const smoke = [{ key: "smoke", probe: SMOKE, sql: smokeSql }];
    const live = await tool.withDatabaseSetting(smoke, urls);
    const none = await tool.withDatabaseSetting(smoke, dead);
    if (live.smoke?.ok && !none.smoke?.ok) {
      connects = "opt-in";
      const statements = FROM_THE_DATABASE.flatMap((probe) => { const sql = sqlFor(probe, "postgres"); return sql ? [{ key: `postgres:from-the-database:${probe.id}`, probe, sql }] : []; });
      Object.assign(answers, await tool.withDatabaseSetting(statements, urls));
    }
  }
  const cells = gradeTool(answers, tool.dbs, { wrote, connects, cardinalityNote: tool.cardinalityNote, noConnectNote: tool.noConnectNote, optInNote: tool.optInNote, withoutGenerating: await tool.withoutGenerating?.() });
  // DuckDB takes no probes but this one: of these tools, only Reylo can point at it, and an adapter without `duckdb` has no way to.
  const duck = await tool.duckdb?.(SMOKE);
  cells.duckdb = duck?.ok ? { state: "y", right: 1, total: 1, results: [] } : { state: "n", right: 0, total: 1, note: duck?.error ?? tool.noDuckdb ?? `${tool.name} has no support for DuckDB.`, results: [] };
  results.tools[id] = { name: tool.name, version: tool.version, dbs: tool.dbs, connects, wrote, cells };
  writeFileSync(resolve(here, "results.json"), `${JSON.stringify(results, null, 1)}\n`);
  console.log(`\n${tool.name} ${tool.version} (${((Date.now() - t0) / 1000).toFixed(0)}s, connects: ${connects})`);
  for (const [row, c] of Object.entries(cells)) console.log(`  ${c.state}  ${row.padEnd(24)} ${c.right}/${c.total}${c.note ? `  ${c.note}` : ""}`);
}

// RESULTS.md: the grid, then every query a tool got wrong and what it answered.
const MARK = { y: "yes", p: "partly", n: "no" };
const ROW_LABELS: Record<string, string> = { "no-codegen": "No codegen", "from-the-database": "Live database", "outer-joins": "Nullable left joins", "expression-nullability": "Nullable expressions", "parameter-types": "Parameter types", "parameter-nullability": "Nullable parameters", cardinality: "Cardinality", "json-shapes": "Nested JSON", "json-schema": "JSON Schema types", postgres: "Postgres", mysql: "MySQL", sqlite: "SQLite", duckdb: "DuckDB" };
const ids = Object.keys(results.tools);
const lines = [`# Results`, ``, `Written by \`./run.sh\` on ${results.at.slice(0, 10)}. Each mark comes from what the tool answered; [probes.ts](probes.ts) holds the queries and the right answers, and [grade.ts](grade.ts) the rules.`, ``, `| | ${ids.map((i) => results.tools[i].name).join(" | ")} |`, `| --- | ${ids.map(() => "---").join(" | ")} |`, `| Version | ${ids.map((i) => results.tools[i].version).join(" | ")} |`];
for (const [row, label] of Object.entries(ROW_LABELS)) lines.push(`| ${label} | ${ids.map((i) => { const c = results.tools[i].cells[row]; return c ? `${MARK[c.state as "y"]}${c.total > 1 ? ` (${c.right}/${c.total})` : ""}` : "no"; }).join(" | ")} |`);
for (const i of ids) {
  const t = results.tools[i];
  lines.push(``, `## ${t.name} ${t.version}`);
  for (const [row, c] of Object.entries<any>(t.cells)) {
    const bad = c.results.filter((r: any) => !r.ok);
    if (!bad.length && !c.note) continue;
    lines.push(``, `**${ROW_LABELS[row] ?? row}**: ${MARK[c.state as "y"]}${c.total > 1 ? `, ${c.right} of ${c.total}` : ""}${c.note ? `. ${String(c.note).replace(/\n/g, " ")}` : ""}`);
    if (bad.length) lines.push(``);
    for (const b of bad) lines.push(`- ${b.db}: \`${b.sql}\`: ${b.problems.join("; ").replace(/\n/g, " ").slice(0, 300)}`);
  }
}
const DB_NAME: Record<string, string> = { postgres: "Postgres", mysql: "MySQL", sqlite: "SQLite" };
const CARD = { one: "exactly one row", maybeOne: "at most one row", many: "many rows", none: "no rows" };
lines.push(``, `## The queries`, ``, `Every tool gets each query on each database it supports. [probes.ts](probes.ts) holds them with their right answers.`);
for (const [group, probes] of GROUPS) {
  if (group === "smoke") continue;
  lines.push(``, `**${group}**`, ``);
  for (const p of probes) {
    const byDb = new Map<string, string[]>();
    for (const db of ["postgres", "mysql", "sqlite"] as const) {
      const sql = sqlFor(p, db);
      if (sql) byDb.set(sql, [...(byDb.get(sql) ?? []), DB_NAME[db]]);
    }
    for (const [sql, names] of byDb) lines.push(`- \`${sql}\`${names.length === 3 ? "" : ` (${names.join(" and ")})`}${p.cardinality ? `: ${CARD[p.cardinality]}` : ""}`);
  }
}
writeFileSync(resolve(here, "RESULTS.md"), `${lines.join("\n")}\n`);

// What the docs' table shows of each mark: the tool's note, or the queries it got wrong and what it answered.
const site: any = { at: results.at, tools: {} };
for (const i of ids) {
  const t = results.tools[i];
  const cells: any = {};
  for (const [row, c] of Object.entries<any>(t.cells)) {
    const wrong = c.results.filter((r: any) => !r.ok && !r.problems.every((p: string) => p === "it does not infer one"));
    const groups = new Map<string, { sql: string; problem: string; dbs: string[] }>();
    for (const r of wrong) {
      const problem = r.problems.map((p: string) => (p.startsWith("it reports an error") ? (/invalid SQL/.test(p) ? "It marks this query as invalid SQL." : "It stops with an error on this query.") : `${p[0].toUpperCase()}${p.slice(1)}.`)).join(" ");
      const g = groups.get(r.sql + problem) ?? { sql: r.sql, problem, dbs: [] };
      g.dbs.push(DB_NAME[r.db]);
      groups.set(r.sql + problem, g);
    }
    const dbsRun = new Set(c.results.map((r: any) => r.db)).size;
    cells[row] = { state: c.state, ...(c.note ? { note: String(c.note).replace(/\n/g, " ").slice(0, 300) } : {}), wrong: wrong.length, examples: [...groups.values()].slice(0, 2).map((g) => ({ sql: g.sql, problem: g.problem, ...(g.dbs.length < dbsRun ? { on: g.dbs.join(" and ") } : {}) })) };
  }
  site.tools[i] = { name: t.name, version: t.version, cells };
}
writeFileSync(resolve(here, "../../site/lib/compare-results.json"), `${JSON.stringify(site, null, 1)}\n`);
