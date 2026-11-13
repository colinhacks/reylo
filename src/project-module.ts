// The module a build step writes for an import of the project file (`import { migrate } from
// "./.reylo.yaml"`): the project's migrations, which a Durable Object runs on its own database with
// `migrate` (src/migrate.ts), and nothing else of the file, whose `direct_url` can hold a password.
// The migrations are those `reylo migrate deploy` runs, in its order: the lock's, or with no lock
// the files' names, `init.sql` first. `reylo migrate lint` checks them first, as a deploy would, so
// that a migration edited after it was locked, or one the lock lists that the folder lacks, fails
// the build; a draft the lock doesn't list yet is left out, as a deploy leaves it.
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { anyEngine } from "./engine.ts";
import { nativeBinary } from "./native.ts";
import { resolveProject } from "./project.ts";
import type { Migration } from "./migrate.ts";

/** The problems `reylo migrate lint` reports that a deploy runs past: a draft, which it leaves out, and a schema.sql whose migration isn't written yet. */
const DEPLOYABLE = new Set(["migration-draft", "schema-sql-changed"]);

type Lint = { ok: boolean; error?: string; problems?: { code: string; title: string; fixes?: { what: string; commands: string[] }[] }[] };

/** The module for an import of `file`, the project's file, and what the build watches. */
export async function projectModule(file: string): Promise<{ code: string; watched: string[] }> {
  const project = resolveProject(await anyEngine(), dirname(file));
  if (project.error) throw new Error(project.error);
  if (!project.file || resolve(project.file) !== resolve(file)) {
    throw new Error(`${file} isn't this project's file${project.file ? ` (${project.file} is)` : ""}: import the project file the type check reads, beside package.json`);
  }
  // The directory of Reylo's files, which `dir` names: the cache is `.cache/` inside it.
  const dir = dirname(project.cache);
  const shown = relative(project.root, dir) || ".";
  const migrationsDir = join(dir, "migrations");
  if (!existsSync(migrationsDir)) throw new Error(`${file} gives migrate the project's migrations, and ${shown}/migrations doesn't exist: write the first with reylo migrate new <name>`);

  const run = spawnSync(nativeBinary(), ["migrate", "lint", "--json"], { cwd: project.root, encoding: "utf8", windowsHide: true });
  let lint: Lint;
  try {
    lint = JSON.parse(run.stdout) as Lint;
  } catch {
    throw new Error(`reylo migrate lint failed: ${(run.stderr || run.stdout || String(run.error)).trim()}`);
  }
  if (lint.error) throw new Error(`reylo migrate lint: ${lint.error}`);
  const refused = (lint.problems ?? []).filter((p) => !DEPLOYABLE.has(p.code));
  if (refused.length) {
    const lines = refused.flatMap((p) => [p.title, ...(p.fixes ?? []).map((f) => `  ${f.what}: ${f.commands.join(", then ")}`)]);
    throw new Error(`The migrations ${file} gives migrate don't pass reylo migrate lint:\n${lines.join("\n")}`);
  }

  const lockPath = join(dir, "reylo-lock.yaml");
  const names = existsSync(lockPath)
    ? [...readFileSync(lockPath, "utf8").matchAll(/^\s*-\s+(\S+\.sql)\s+[0-9a-f]+\s*$/gm)].map((m) => m[1]!)
    : readdirSync(migrationsDir)
        .filter((f) => f.endsWith(".sql"))
        .sort((a, b) => (a === "init.sql" ? -1 : b === "init.sql" ? 1 : a < b ? -1 : a > b ? 1 : 0));
  const list: Migration[] = names.map((name) => ({ name, sql: readFileSync(join(migrationsDir, name), "utf8") }));
  const code = `import { migrations } from "reylo";\nexport const { migrate } = migrations(${JSON.stringify(list)});\n`;
  return { code, watched: [file, lockPath, migrationsDir, ...names.map((n) => join(migrationsDir, n))] };
}
