// What Reylo's Vite and esbuild plugins put in place of src/embedded-migrations.ts: the project's
// migrations, which `migrate(ctx.storage)` runs on a Durable Object (src/migrate.ts), and nothing
// else of the project file, whose `direct_url` can hold a password. The migrations are those
// `reylo migrate deploy` runs, in its order: the lock's, or with no lock the files' names,
// `init.sql` first. A draft the lock doesn't list yet is left out, as a deploy leaves it. Each
// object checks its own history against them before it runs any (src/migrate.ts), as a deploy does
// with no shadow database. The files are held against the lock's checksums first, as a deploy holds
// them, so that a migration edited after it was locked, or lines merged out of their order, give
// `migrate` the deploy's refusal to throw rather than the edited text to run.
//
// Every bundle that imports reylo holds the placeholder, whether it calls `migrate` or not, so a
// project with no migrations gets none, and a lock that can't be read gives `migrate` its error to
// throw rather than failing a build that may never call it.
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { anyEngine } from "./engine.ts";
import { resolveProject } from "./project.ts";
import type { Migration } from "./migrate.ts";
import { canonical, normalized } from "./migration-text.ts";

/** A lock entry's checksum, as `lock::checksum` writes it (engine/migrate/src/lock.rs): each part after its length, the first 4 bytes of the SHA-256 in hex. */
export function lockChecksum(prev: string, name: string, sql: string): string {
  const h = createHash("sha256");
  for (const part of [prev, "\n", name, "\n", canonical(normalized(sql))]) {
    const bytes = Buffer.from(part, "utf8");
    const length = Buffer.alloc(8);
    length.writeBigUInt64BE(BigInt(bytes.length));
    h.update(length).update(bytes);
  }
  return h.digest("hex").slice(0, 8);
}

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
    const entries = [...lock.matchAll(/^\s*-\s+(\S+\.sql)\s+([0-9a-f]+)\s*$/gm)].map((m) => ({ name: m[1]!, sum: m[2]! }));
    names = entries.map((e) => e.name);
    const missing = names.filter((n) => !existsSync(join(migrationsDir, n)));
    if (missing.length) return { error: `${shown}/reylo-lock.yaml lists ${missing.join(", ")}, which ${shown}/migrations doesn't have: write it back from a database that ran it, with reylo migrate history pull`, watched };
    // Each entry's checksum chains its file's text to the entry before it (`links` in engine/migrate/src/commands.rs).
    for (const [k, e] of entries.entries()) {
      const sql = readFileSync(join(migrationsDir, e.name), "utf8");
      const prev = k === 0 ? "" : entries[k - 1]!.sum;
      if (lockChecksum(prev, e.name, sql) === e.sum) continue;
      const moved = ["", ...entries.slice(0, Math.max(k - 1, 0)).map((p) => p.sum)].some((sum) => sum !== prev && lockChecksum(sum, e.name, sql) === e.sum);
      const error = moved ? `${shown}/reylo-lock.yaml was merged out of order at ${e.name}: keep main's lines, in main's order, then run reylo migrate dev, which adds this branch's migrations back` : `${shown}/migrations/${e.name} changed after it was locked: a database that ran the old text refuses it, so put the change in a new migration, or, if no database ran it, lock it as it now reads with reylo migrate dev`;
      return { error, watched: [...watched, ...names.map((n) => join(migrationsDir, n))] };
    }
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
