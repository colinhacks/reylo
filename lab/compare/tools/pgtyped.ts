// PgTyped: its CLI writes a .queries.ts file beside each .sql file, whose interfaces are what it inferred.
import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { failed, gitDelta, gitInit, readTsDeclarations, readTsType, rewrite, sh, workdir, type Answer, type Tool } from "./lib.ts";

const root = resolve(import.meta.dirname, "..");
const bin = join(root, "node_modules/.bin/pgtyped");
const version: string = JSON.parse(readFileSync(join(root, "node_modules/@pgtyped/cli/package.json"), "utf8")).version;

export const pgtyped: Tool = {
  id: "pgtyped",
  name: "PgTyped",
  version,
  dbs: ["postgres"],
  noDuckdb: 'PgTyped is Postgres only.',
  cardinalityNote: "A query's `.run()` always returns an array, whatever the statement.",
  async analyze(_db, statements, urls) {
    const dir = workdir("pgtyped");
    mkdirSync(join(dir, "src"));
    const u = new URL(urls.postgres);
    writeFileSync(join(dir, "config.json"), JSON.stringify({ transforms: [{ mode: "sql", include: "**/*.sql", emitTemplate: "{{dir}}/{{name}}.queries.ts" }], srcDir: "./src/", failOnError: false, camelCaseColumnNames: false, db: { host: u.hostname, port: Number(u.port), user: u.username, dbName: u.pathname.slice(1), password: "" } }));
    writeFileSync(join(dir, "src/q.sql"), statements.map((s, i) => `/* @name Q${i} */\n${rewrite(s.sql, (n) => `:p${n}`)};\n`).join("\n"));
    gitInit(dir);
    const run = sh(bin, ["-c", "config.json"], { cwd: dir });
    const out = join(dir, "src/q.queries.ts");
    const answers: Record<string, Answer> = {};
    if (!existsSync(out)) { for (const s of statements) answers[s.key] = failed(run.all); return { answers, wrote: gitDelta(dir) }; }
    const decls = readTsDeclarations(out);
    // PgTyped names its unions and arrays (`NumberOrString`, `numberArray`) at the top of the file.
    const alias = (t: string) => t.replace(/\b[A-Za-z]\w*\b/g, (w) => (typeof decls[w] === "string" ? `(${decls[w]})` : w));
    statements.forEach((s, i) => {
      const p = decls[`IQ${i}Params`];
      const r = decls[`IQ${i}Result`];
      if (p === undefined && r === undefined) { answers[s.key] = failed(run.all.split("\n").filter((l) => l.includes(`Q${i}`)).join(" ") || "no types generated"); return; }
      const order = (s.sql.match(/\$\d+/g) ?? []).map((m) => `p${m.slice(1)}`);
      const params = Array.isArray(p) ? order.map((name) => { const f = p.find((x) => x.name === name); const t = readTsType(alias(f?.type ?? "any")); return { ...t, nullable: t.nullable || !!f?.optional, type: `${f?.optional ? "optional " : ""}${f?.type}` }; }) : [];
      const columns = Array.isArray(r) ? r.map((f) => ({ name: f.name, ...readTsType(alias(f.type)), type: f.type })) : [];
      answers[s.key] = { ok: true, params, columns, cardinality: null };
    });
    return { answers, wrote: gitDelta(dir) };
  },
};
