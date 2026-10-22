// The SQLite the app runs on. The type check prepares each statement in Reylo's own SQLite, but
// the app runs it on its driver's, which may be older or another engine: libSQL carries SQLite
// 3.45.1, which has no `json_pretty` and takes `iif` with three arguments or more. So each statement
// is also prepared on every SQLite driver the project depends on that this Node can load, on a
// read-only connection to the same database, and a driver that refuses it fails the type check
// with that SQLite's own message. Preparing compiles the statement without running it.
//
// A driver is the project's when its package.json lists it. One this Node cannot load (a native
// module built for another Node) or that cannot open the database (Turso's engine refuses a
// schema with an FTS5 table) is left out, as are bun:sqlite and Cloudflare D1, which run in
// another runtime.
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { packageDir } from "./project.ts";

export interface SqliteDriver {
  /** The package the project lists, as a message names it. */
  readonly name: string;
  /** The version of SQLite it carries. */
  readonly version: string;
  /** SQLite's message where it refuses to prepare `sql`; undefined where it prepares it. */
  refuses(sql: string): Promise<string | undefined>;
  close(): Promise<void>;
}

type Require = NodeJS.Require;
type Open = (require: Require, path: string) => Promise<SqliteDriver>;

/** better-sqlite3's API, which the `libsql` package and Turso's compat module copy. */
interface SyncDatabase {
  prepare(sql: string): { get(): unknown };
  close(): void;
}

function sync(name: string, db: SyncDatabase): SqliteDriver {
  const row = db.prepare("select sqlite_version() as v").get() as { v: string };
  return {
    name,
    version: row.v,
    async refuses(sql) {
      try {
        db.prepare(sql);
        return undefined;
      } catch (e) {
        return (e as Error).message;
      }
    },
    async close() {
      db.close();
    },
  };
}

/** The drivers, in the order a message lists them. */
const DRIVERS: [string, Open][] = [
  [
    "better-sqlite3",
    async (require, path) => {
      const Database = require("better-sqlite3") as new (path: string, options: object) => SyncDatabase;
      return sync("better-sqlite3", new Database(path, { readonly: true, fileMustExist: true }));
    },
  ],
  [
    // `@libsql/client` opens a file through the `libsql` package, its own dependency.
    "@libsql/client",
    async (require, path) => {
      const Database = createRequire(require.resolve("@libsql/client"))("libsql") as new (path: string, options: object) => SyncDatabase;
      return sync("@libsql/client", new Database(path, { readonly: true }));
    },
  ],
  [
    "libsql",
    async (require, path) => {
      const Database = require("libsql") as new (path: string, options: object) => SyncDatabase;
      return sync("libsql", new Database(path, { readonly: true }));
    },
  ],
  [
    "sqlite3",
    async (require, path) => {
      type Db = {
        get(sql: string, cb: (e: Error | null, row: { v: string }) => void): void;
        prepare(sql: string, cb: (e: Error | null) => void): { finalize(): void };
        close(cb: () => void): void;
      };
      const sqlite3 = require("sqlite3") as { Database: new (path: string, mode: number, cb: (e: Error | null) => void) => Db; OPEN_READONLY: number };
      const db = await new Promise<Db>((done, fail) => {
        const d: Db = new sqlite3.Database(path, sqlite3.OPEN_READONLY, (e) => (e ? fail(e) : done(d)));
      });
      const version = await new Promise<string>((done, fail) => db.get("select sqlite_version() as v", (e, row) => (e ? fail(e) : done(row.v))));
      return {
        name: "sqlite3",
        version,
        refuses: (sql) =>
          new Promise((done) => {
            const statement = db.prepare(sql, (e) => {
              if (!e) statement.finalize();
              // node-sqlite3 writes the result code before SQLite's message.
              done(e ? e.message.replace(/^SQLITE_\w+: /, "") : undefined);
            });
          }),
        close: () => new Promise<void>((done) => db.close(() => done())),
      };
    },
  ],
  [
    "@tursodatabase/database",
    async (require, path) => {
      const { Database } = (await import(pathToFileURL(require.resolve("@tursodatabase/database/compat")).href)) as { Database: new (path: string, options: object) => SyncDatabase };
      return sync("@tursodatabase/database", new Database(path, { readonly: true }));
    },
  ],
];

/** The SQLite drivers the project's package.json lists, by package name. */
export function projectDrivers(root: string): string[] {
  const file = join(packageDir(root), "package.json");
  if (!existsSync(file)) return [];
  let listed: Set<string>;
  try {
    const pkg = JSON.parse(readFileSync(file, "utf8")) as Record<string, Record<string, string> | undefined>;
    listed = new Set(["dependencies", "devDependencies", "optionalDependencies"].flatMap((key) => Object.keys(pkg[key] ?? {})));
  } catch {
    return [];
  }
  return DRIVERS.map(([name]) => name).filter((name) => listed.has(name));
}

/** Opens each of the project's drivers on the database at `path`, read-only; one that cannot load or open it is left out. */
export async function openProjectDrivers(root: string, path: string): Promise<SqliteDriver[]> {
  if (path === ":memory:") return [];
  const names = projectDrivers(root);
  if (!names.length) return [];
  const require = createRequire(join(packageDir(root), "package.json"));
  const opened = await Promise.all(DRIVERS.filter(([name]) => names.includes(name)).map(([, open]) => open(require, path).catch(() => undefined)));
  return opened.filter((d): d is SqliteDriver => d !== undefined);
}

const utf8 = new TextEncoder();

/**
 * Where in the file a driver's refusal points: the call SQLite names ("no such function: x",
 * "wrong number of arguments to function x()"), else the text a syntax error is near, else the
 * statement's first word. In UTF-8 bytes, as the analysis's diagnostics are.
 */
export function placeRefusal(sql: string, message: string): { start: number; length: number } {
  const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const fn = /no such function: ([\w$]+)/i.exec(message)?.[1] ?? /arguments to function ([\w$]+)\(\)/i.exec(message)?.[1];
  const near = /near "([^"]+)"/.exec(message)?.[1];
  const found = fn ? new RegExp(`\\b${escape(fn)}\\s*\\(`, "i").exec(sql) : near ? new RegExp(escape(near), "i").exec(sql) : null;
  const at = found ? { index: found.index, text: fn ?? near! } : { index: sql.search(/\S/), text: /\S+/.exec(sql)?.[0] ?? "" };
  return { start: utf8.encode(sql.slice(0, Math.max(at.index, 0))).length, length: Math.max(utf8.encode(at.text).length, 1) };
}
