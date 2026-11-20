// What Reylo's Vite and esbuild plugins put in place of src/embedded-migrations.ts: the project's
// migrations, which `migrate(ctx.storage)` runs on a Durable Object (src/migrate.ts), and nothing
// else of the project file, whose `direct_url` can hold a password. The migrations are those
// `reylo migrate deploy` runs, in its order: the lock's, or with no lock the files' names,
// `init.sql` first. Each object checks its own history against them before it runs any
// (src/migrate.ts), as a deploy does with no shadow database. The files are held to what a deploy
// holds them to first (`check` in engine/migrate/src/commands.rs): a file in the migrations
// directory that isn't a migration, a migration the lock doesn't list yet, one edited after it was
// locked, lines merged out of their order, and a schema.sql the lock's checksum isn't of give
// `migrate` the deploy's refusal to throw rather than migrations to run.
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

/** Each part after its length, the first 4 bytes of the SHA-256 in hex, as `digest` writes a lock's checksums (engine/migrate/src/lock.rs). */
function digest(parts: string[]): string {
  const h = createHash("sha256");
  for (const part of parts) {
    const bytes = Buffer.from(part, "utf8");
    const length = Buffer.alloc(8);
    length.writeBigUInt64BE(BigInt(bytes.length));
    h.update(length).update(bytes);
  }
  return h.digest("hex").slice(0, 8);
}

/** A lock entry's checksum, as `lock::checksum` writes it. */
export function lockChecksum(prev: string, name: string, sql: string): string {
  return digest([prev, "\n", name, "\n", canonical(normalized(sql))]);
}

/** The lock's checksum of schema.sql's text after the entry `last`, as `lock::schema_sum` writes it. */
export function schemaChecksum(last: string, sql: string): string {
  return digest(["schema\n", last, "\n", canonical(sql)]);
}

/** A migration's file name, as `read_migrations` reads it (engine/migrate/src/commands.rs). */
const MIGRATION = /^(init\.sql|\d{14}_[a-z0-9_]+\.sql)$/;

/** The project's migrations in the order they run, or why they can't be read; undefined for a project with none. */
export async function projectMigrations(root: string): Promise<{ list?: Migration[]; error?: string; watched: string[] }> {
  const project = resolveProject(await anyEngine(), root);
  // The directory of Reylo's files, which `dir` names: the cache is `.cache/` inside it.
  const dir = dirname(project.cache);
  const shown = relative(project.root, dir) || ".";
  const migrationsDir = join(dir, "migrations");
  const lockPath = join(dir, "reylo.lock");
  // The lock's name before 0.8.0, refused as every migration command refuses it (`old_lock` in
  // engine/migrate/src/commands.rs): read as no lock, it would run the migrations in name order.
  const oldPath = join(dir, "reylo-lock.yaml");
  const schemaPath = join(dir, "schema.sql");
  const watched = [...(project.file ? [project.file] : []), lockPath, oldPath, migrationsDir, schemaPath];
  if (!existsSync(migrationsDir)) return { watched };
  if (existsSync(oldPath)) {
    const error = existsSync(lockPath) ? `Both ${shown}/reylo-lock.yaml and ${shown}/reylo.lock are here: ${shown}/reylo.lock is the lock, and ${shown}/reylo-lock.yaml its name before 0.8.0, so keep ${shown}/reylo.lock and delete the other, with git rm ${shown}/reylo-lock.yaml` : `The lock's old name is ${shown}/reylo-lock.yaml: rename it to ${shown}/reylo.lock, with git mv ${shown}/reylo-lock.yaml ${shown}/reylo.lock`;
    return { error, watched };
  }
  // Every other entry but a hidden one is refused, lock or not (`stray_problem`).
  const files = readdirSync(migrationsDir).filter((f) => !f.startsWith("."));
  const strays = files.filter((f) => !MIGRATION.test(f));
  if (strays.length) {
    return { error: `${strays.length === 1 ? "Not a migration" : "Not migrations"}: ${strays.join(", ")}, in ${shown}/migrations, where a file is init.sql or <YYYYMMDDHHMMSS>_<name>.sql, the name in a-z, 0-9 and _`, watched };
  }
  const migrations = files.sort((a, b) => (a === "init.sql" ? -1 : b === "init.sql" ? 1 : a < b ? -1 : a > b ? 1 : 0));
  let names: string[];
  if (existsSync(lockPath)) {
    const lock = readFileSync(lockPath, "utf8");
    // Two branches that each added a migration leave git's markers in the lock.
    if (/^(<{7}|={7}|>{7})( |$)/m.test(lock)) return { error: `Merge conflict markers in ${shown}/reylo.lock: keep main's lines, then run reylo migrate dev, which adds this branch's migrations back`, watched };
    const entries = [...lock.matchAll(/^\s*-\s+(\S+\.sql)\s+([0-9a-f]+)\s*$/gm)].map((m) => ({ name: m[1]!, sum: m[2]! }));
    names = entries.map((e) => e.name);
    const missing = names.filter((n) => !existsSync(join(migrationsDir, n)));
    if (missing.length) return { error: `The lock lists ${missing.join(", ")}, which ${shown}/migrations doesn't have: write it back from a database that ran it, with reylo migrate history pull`, watched };
    // Each entry's checksum chains its file's text to the entry before it (`links` in engine/migrate/src/commands.rs).
    for (const [k, e] of entries.entries()) {
      const sql = readFileSync(join(migrationsDir, e.name), "utf8");
      const prev = k === 0 ? "" : entries[k - 1]!.sum;
      if (lockChecksum(prev, e.name, sql) === e.sum) continue;
      const moved = ["", ...entries.slice(0, Math.max(k - 1, 0)).map((p) => p.sum)].some((sum) => sum !== prev && lockChecksum(sum, e.name, sql) === e.sum);
      const error = moved ? `The lock was merged out of order at ${e.name}: keep main's lines, in main's order, then run reylo migrate dev, which adds this branch's migrations back` : `Migration ${shown}/migrations/${e.name} changed after it was locked: a database that ran the old text refuses it, so put the change in a new migration, or, if no database ran it, lock it as it now reads with reylo migrate dev`;
      return { error, watched: [...watched, ...names.map((n) => join(migrationsDir, n))] };
    }
    // A migration the lock doesn't list yet is a draft, which only development runs (`lock_problems`).
    const drafts = migrations.filter((n) => !names.includes(n));
    if (drafts.length) {
      const error = drafts.length === 1 ? `The lock doesn't list ${shown}/migrations/${drafts[0]} yet: run it in development, which locks it, with reylo migrate dev` : `The lock doesn't list ${drafts.length} migrations yet, ${drafts.join(", ")}: run them in development, which locks them, with reylo migrate dev`;
      return { error, watched: [...watched, ...migrations.map((n) => join(migrationsDir, n))] };
    }
    // The checksum of schema.sql from when the migrations last reached it, which a deploy holds it to (`stamp`).
    const schemaSum = /^schema:\s*(\S+)\s*$/m.exec(lock)?.[1];
    if (schemaSum !== undefined && existsSync(schemaPath) && schemaChecksum(entries.at(-1)?.sum ?? "", readFileSync(schemaPath, "utf8")) !== schemaSum) {
      return { error: `Changes in ${shown}/schema.sql since the lock was written: if the schema changed, write its migration and run it with reylo migrate new <name> and reylo migrate dev; if only formatting or comments changed, check it again with reylo migrate dev`, watched: [...watched, ...names.map((n) => join(migrationsDir, n))] };
    }
  } else {
    names = migrations;
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
