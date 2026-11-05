// Prisma TypedSQL: `prisma generate --sql` writes a TypeScript file per .sql file, whose Parameters and Result types are what it inferred. One bad statement stops a run, so those it names are set aside and the run repeated.
import { mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import type { Db } from "../probes.ts";
import { failed, gitDelta, gitInit, readTsDeclarations, readTsTuple, readTsType, rewrite, sh, workdir, type Answer, type Tool } from "./lib.ts";

const root = resolve(import.meta.dirname, "..");
const bin = join(root, "node_modules/.bin/prisma");
const version: string = JSON.parse(readFileSync(join(root, "node_modules/prisma/package.json"), "utf8")).version;
const PROVIDER: Record<Db, string> = { postgres: "postgresql", mysql: "mysql", sqlite: "sqlite" };

export const typedsql: Tool = {
  id: "typedsql",
  name: "TypedSQL",
  version,
  dbs: ["postgres", "mysql", "sqlite"],
  noDuckdb: "DuckDB isn't a Prisma datasource.",
  cardinalityNote: "Every query returns an array, whatever the statement.",
  async analyze(db, statements, urls) {
    const dir = workdir("typedsql");
    symlinkSync(join(root, "node_modules"), join(dir, "node_modules"));
    mkdirSync(join(dir, "prisma/sql"), { recursive: true });
    writeFileSync(join(dir, "package.json"), '{"name":"p","private":true,"type":"module"}\n');
    writeFileSync(join(dir, "prisma.config.ts"), 'import { defineConfig } from "prisma/config";\nexport default defineConfig({ schema: "prisma/schema.prisma", datasource: { url: process.env.DATABASE_URL! } });\n');
    writeFileSync(join(dir, "prisma/schema.prisma"), `generator client {\n  provider = "prisma-client"\n  output = "../generated/prisma"\n  previewFeatures = ["typedSql"]\n}\ndatasource db {\n  provider = "${PROVIDER[db]}"\n}\n`);
    statements.forEach((s, i) => writeFileSync(join(dir, `prisma/sql/q${i}.sql`), `${rewrite(s.sql, db === "postgres" ? (n) => `$${n}` : () => "?")}\n`));
    const env = { DATABASE_URL: db === "postgres" ? urls.postgres : db === "mysql" ? urls.mysql : `file:${urls.sqlite}` };
    const pull = sh(bin, ["db", "pull"], { cwd: dir, env });
    const answers: Record<string, Answer> = {};
    if (pull.status !== 0) { for (const s of statements) answers[s.key] = failed(pull.all); return { answers, wrote: [] }; }
    gitInit(dir);
    const bad = new Map<number, string>();
    let run = sh(bin, ["generate", "--sql"], { cwd: dir, env });
    for (let tries = 0; run.status !== 0 && tries < 20; tries++) {
      const named = [...new Set([...run.all.matchAll(/q(\d+)\.sql/g)].map((m) => Number(m[1])))];
      if (!named.length) break;
      for (const i of named) { bad.set(i, run.all); rmSync(join(dir, `prisma/sql/q${i}.sql`), { force: true }); }
      run = sh(bin, ["generate", "--sql"], { cwd: dir, env });
    }
    statements.forEach((s, i) => {
      const out = join(dir, `generated/prisma/sql/q${i}.ts`);
      if (!existsSync(out)) { answers[s.key] = failed(bad.get(i) ?? run.all); return; }
      const d = readTsDeclarations(out);
      const tuple = d[`q${i}.Parameters`];
      const result = d[`q${i}.Result`];
      answers[s.key] = {
        ok: true,
        params: typeof tuple === "string" ? readTsTuple(tuple).map(readTsType) : [],
        columns: Array.isArray(result) ? result.map((f) => ({ name: f.name, ...readTsType(f.type) })) : [],
        cardinality: null,
      };
    });
    return { answers, wrote: gitDelta(dir) };
  },
};
