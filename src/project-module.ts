// The module a build step writes for an import of the project file (`import { migrate } from
// "./.reylo.yaml"`): the project's migrations, which a Durable Object runs on its own database with
// `migrate` (src/migrate.ts), and nothing else of the file, whose `direct_url` can hold a password.
// The migrations are those `reylo migrate deploy` runs, in its order: the lock's, or with no lock
// the files' names, `init.sql` first. A draft the lock doesn't list yet is left out, as a deploy
// leaves it. Each object checks its own history against them before it runs any (src/migrate.ts),
// as a deploy does with no shadow database, so the build only reads the lock for the order.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { anyEngine } from "./engine.ts";
import { resolveProject } from "./project.ts";
import type { Migration } from "./migrate.ts";

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

  const lockPath = join(dir, "reylo-lock.yaml");
  let names: string[];
  if (existsSync(lockPath)) {
    const lock = readFileSync(lockPath, "utf8");
    // Two branches that each added a migration leave git's markers in the lock.
    if (/^(<{7}|={7}|>{7})( |$)/m.test(lock)) throw new Error(`${shown}/reylo-lock.yaml has git's conflict markers: keep main's lines, then run reylo migrate dev, which adds this branch's migrations back`);
    names = [...lock.matchAll(/^\s*-\s+(\S+\.sql)\s+[0-9a-f]+\s*$/gm)].map((m) => m[1]!);
    const missing = names.filter((n) => !existsSync(join(migrationsDir, n)));
    if (missing.length) throw new Error(`${shown}/reylo-lock.yaml lists ${missing.join(", ")}, which ${shown}/migrations doesn't have: write it back from a database that ran it, with reylo migrate history pull`);
  } else {
    names = readdirSync(migrationsDir)
      .filter((f) => f.endsWith(".sql"))
      .sort((a, b) => (a === "init.sql" ? -1 : b === "init.sql" ? 1 : a < b ? -1 : a > b ? 1 : 0));
  }
  const list: Migration[] = names.map((name) => ({ name, sql: readFileSync(join(migrationsDir, name), "utf8") }));
  const code = `import { migrations } from "reylo";\nexport const { migrate } = migrations(${JSON.stringify(list)});\n`;
  return { code, watched: [file, lockPath, migrationsDir, ...names.map((n) => join(migrationsDir, n))] };
}
