// What Reylo's Vite and esbuild plugins put in place of src/embedded-migrations.ts: the project's
// migrations, which `migrate(ctx.storage)` runs on a Durable Object (src/migrate.ts), and nothing
// else of the project file, whose `direct_url` can hold a password. The migrations are those
// `reylo migrate deploy` runs, in its order: the lock's, or with no lock the files' names,
// `init.sql` first. A draft the lock doesn't list yet is left out, as a deploy leaves it. Each
// object checks its own history against them before it runs any (src/migrate.ts), as a deploy does
// with no shadow database, so this only reads the lock for the order.
//
// Every bundle that imports reylo holds the placeholder, whether it calls `migrate` or not, so a
// project with no migrations gets none, and a lock that can't be read gives `migrate` its error to
// throw rather than failing a build that may never call it.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { anyEngine } from "./engine.ts";
import { resolveProject } from "./project.ts";
import type { Migration } from "./migrate.ts";

/** The project's migrations in the order they run, or why they can't be read; undefined for a project with none. */
export async function projectMigrations(root: string): Promise<{ list?: Migration[]; error?: string; watched: string[] }> {
  const project = resolveProject(await anyEngine(), root);
  // The directory of Reylo's files, which `dir` names: the cache is `.cache/` inside it.
  const dir = dirname(project.cache);
  const shown = relative(project.root, dir) || ".";
  const migrationsDir = join(dir, "migrations");
  const lockPath = join(dir, "reylo-lock.yaml");
  const watched = [...(project.file ? [project.file] : []), lockPath, migrationsDir];
  if (!existsSync(migrationsDir)) return { watched };
  let names: string[];
  if (existsSync(lockPath)) {
    const lock = readFileSync(lockPath, "utf8");
    // Two branches that each added a migration leave git's markers in the lock.
    if (/^(<{7}|={7}|>{7})( |$)/m.test(lock)) return { error: `${shown}/reylo-lock.yaml has git's conflict markers: keep main's lines, then run reylo migrate dev, which adds this branch's migrations back`, watched };
    names = [...lock.matchAll(/^\s*-\s+(\S+\.sql)\s+[0-9a-f]+\s*$/gm)].map((m) => m[1]!);
    const missing = names.filter((n) => !existsSync(join(migrationsDir, n)));
    if (missing.length) return { error: `${shown}/reylo-lock.yaml lists ${missing.join(", ")}, which ${shown}/migrations doesn't have: write it back from a database that ran it, with reylo migrate history pull`, watched };
  } else {
    names = readdirSync(migrationsDir)
      .filter((f) => f.endsWith(".sql"))
      .sort((a, b) => (a === "init.sql" ? -1 : b === "init.sql" ? 1 : a < b ? -1 : a > b ? 1 : 0));
  }
  const list = names.map((name) => ({ name, sql: readFileSync(join(migrationsDir, name), "utf8") }));
  return { list, watched: [...watched, ...names.map((n) => join(migrationsDir, n))] };
}

/** The module that replaces src/embedded-migrations.ts in a bundle of the project at `root`, and what the build watches. */
export async function embeddedModule(root: string): Promise<{ code: string; watched: string[] }> {
  const { list, error, watched } = await projectMigrations(root);
  const value = error !== undefined ? { error } : list;
  return { code: `export const embedded = ${value === undefined ? "undefined" : JSON.stringify(value)};\n`, watched };
}
