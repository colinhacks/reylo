// Postgres Typed SQL: `postgres-typed-sql generate` loads the schema file into an embedded Postgres and writes a TypeScript file per .typed.sql file, whose types and row bounds are what it inferred.
import { mkdirSync, readFileSync, symlinkSync, writeFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import type { Card } from "../probes.ts";
import { failed, gitDelta, gitInit, readTsDeclarations, readTsType, rewrite, sh, workdir, type Answer, type Tool } from "./lib.ts";

const root = resolve(import.meta.dirname, "..");
const bin = join(root, "node_modules/.bin/postgres-typed-sql");
const version: string = JSON.parse(readFileSync(join(root, "node_modules/postgres-typed-sql/package.json"), "utf8")).version;
// The embedded Postgres has no pg_jsonschema, so the schema file loads without that extension and its CHECK.
const schema = readFileSync(join(root, "schema/postgres.sql"), "utf8").replace(/create extension[^\n]*\n/, "").replace(/ check \(jsonb_matches_schema\(.*?, payload\)\)/, "");

function bounds(text: string): Card | null {
  const m = text.match(/rowBounds:\s*\{\s*min:\s*(\d+),\s*max:\s*(\d+|null)/);
  if (!m) return null;
  if (m[2] === "null") return "many";
  if (m[2] === "0") return "none";
  if (m[2] === "1") return m[1] === "1" ? "one" : "maybeOne";
  return "many";
}

export const postgrestypedsql: Tool = {
  id: "postgrestypedsql",
  name: "Postgres Typed SQL",
  version,
  dbs: ["postgres"],
  noDuckdb: "It is Postgres only.",
  noConnectNote: "It loads your schema file into an embedded Postgres, and never connects to your database.",
  // Its experimental import workflow: TypeScript 7.1 types a .typed.sql import through its content mapper. A wrong assignment must fail with the query's type, with nothing left to commit.
  async withoutGenerating() {
    const dir = workdir("pts-direct");
    symlinkSync(join(root, "node_modules"), join(dir, "node_modules"));
    mkdirSync(join(dir, "src"));
    writeFileSync(join(dir, "package.json"), '{"name":"p","private":true,"type":"module"}\n');
    writeFileSync(join(dir, "schema.sql"), schema);
    writeFileSync(join(dir, "postgres-typed-sql.config.mjs"), "export default { schema: ['schema.sql'], codecProfile: 'node-postgres', imports: { runtime: 'postgres-typed-sql/adapters/node-postgres', scalars: 'postgres-typed-sql/scalars' } }\n");
    writeFileSync(join(dir, "tsconfig.json"), JSON.stringify({ compilerOptions: { strict: true, noEmit: true, module: "nodenext", target: "es2023", skipLibCheck: true, types: [] }, include: ["src"], contentMappers: [{ package: "postgres-typed-sql", extensions: [".typed.sql"], options: { config: "./postgres-typed-sql.config.mjs" } }] }));
    writeFileSync(join(dir, "src/q.typed.sql"), "select id, name from users where id = :id\n");
    writeFileSync(join(dir, "src/main.ts"), 'import q from "./q.typed.sql";\nconst wrong: number = q;\nexport { wrong };\n');
    gitInit(dir);
    const run = sh("node", [join(root, "node_modules/typescript-7/bin/tsc"), "--noEmit", "--runExternalCode"], { cwd: dir });
    const typed = /TS2322/.test(run.all) && /main\.ts/.test(run.all);
    return typed && gitDelta(dir).length === 0 ? "By default it writes files to commit. An experimental workflow on TypeScript 7.1 types the import with nothing generated." : null;
  },
  async analyze(_db, statements, urls) {
    const answers: Record<string, Answer> = {};
    // It reads the database's address from nowhere: a run with no server still answers, which run.ts measures. The URL is unused.
    void urls;
    const dir = workdir("pts");
    symlinkSync(join(root, "node_modules"), join(dir, "node_modules"));
    mkdirSync(join(dir, "db"));
    mkdirSync(join(dir, "src/q"), { recursive: true });
    writeFileSync(join(dir, "package.json"), '{"name":"p","private":true,"type":"module"}\n');
    writeFileSync(join(dir, "db/schema.sql"), schema);
    writeFileSync(join(dir, "postgres-typed-sql.config.mjs"), "import { defineConfig } from 'postgres-typed-sql'\nexport default defineConfig({ schema: './db/schema.sql', include: ['./src'], imports: { runtime: 'postgres-typed-sql/adapters/node-postgres', scalars: 'postgres-typed-sql/scalars' }, codecProfile: 'node-postgres' })\n");
    statements.forEach((s, i) => writeFileSync(join(dir, `src/q/q${i}.typed.sql`), `${rewrite(s.sql, (n) => `:p${n}`)}\n`));
    gitInit(dir);
    const run = sh(bin, ["generate"], { cwd: dir, timeout: 600_000 });
    statements.forEach((s, i) => {
      const out = join(dir, `src/q/q${i}.typed-sql.ts`);
      if (!existsSync(out)) { answers[s.key] = failed(run.all.split("\n").filter((l) => l.includes(`q${i}`)).slice(0, 3).join(" ") || run.all); return; }
      const d = readTsDeclarations(out);
      const params = d[`Q${i}Params`];
      const row = d[`Q${i}Row`];
      const order = (s.sql.match(/\$\d+/g) ?? []).map((m) => `p${m.slice(1)}`);
      answers[s.key] = {
        ok: true,
        params: Array.isArray(params) ? order.map((name) => { const f = params.find((x) => x.name === name); const t = readTsType(f?.type ?? "any", d); return { ...t, nullable: t.nullable || !!f?.optional }; }) : [],
        columns: Array.isArray(row) ? row.map((f) => ({ name: f.name, ...readTsType(f.type, d) })) : [],
        cardinality: bounds(readFileSync(out, "utf8")),
      };
    });
    return { answers, wrote: gitDelta(dir) };
  },
};
