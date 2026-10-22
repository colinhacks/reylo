// The SQLite the app runs on. The type check prepares each statement in Reylo's own SQLite, but
// the app runs it on its driver's, which may be older or another engine: libSQL carries SQLite
// 3.45.1, which has no `json_pretty` and takes `iif` with three arguments or more. So each statement
// is also prepared on every SQLite driver the project depends on that this Node can load, on a
// read-only connection to the same database, and a driver that refuses it fails the type check
// with that SQLite's own message. Preparing compiles the statement without running it.
//
// A driver is the project's when its package.json lists it. One this Node cannot load (a native
// module built for another Node) or that cannot open the database (Turso's engine refuses a
// schema with an FTS5 table) is left out, as is bun:sqlite, which runs in another runtime.
//
// Cloudflare D1 is asked through the project's wrangler, which runs D1 locally: D1 refuses
// functions its SQLite has (an allowlist it applies as SQLite compiles a statement, which nothing
// lists: it refuses `pragma_function_list` too) and lacks others (FTS4, Geopoly), so the only way
// to know is to compile the statement there (src/d1-proxy.ts).
import { fork } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath, pathToFileURL } from "node:url";
import { packageDir } from "./project.ts";

export interface SqliteDriver {
  /** The package the project lists. */
  readonly name: string;
  /** SQLite's message where it refuses to prepare `sql`, whose placeholders number `params`; undefined where it prepares it. */
  refuses(sql: string, params: number): Promise<string | undefined>;
  /** The error a refusal is: "@libsql/client runs SQLite 3.45.1, which refuses this statement: …". */
  says(refusal: string): string;
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
    says: (refusal) => `${name} runs SQLite ${row.v}, which refuses this statement: ${refusal}`,
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
        says: (refusal) => `sqlite3 runs SQLite ${version}, which refuses this statement: ${refusal}`,
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
  ["wrangler", openD1],
];

/** How long workerd may take to start before D1 is left out. */
const D1_START = 30_000;

/**
 * D1 as the project's wrangler runs it locally, in a child process (src/d1-proxy.ts), given the
 * schema of the database the type check reads. A project whose wrangler config binds no D1
 * database is not asked.
 */
async function openD1(require: Require, path: string): Promise<SqliteDriver> {
  const entry = require.resolve("wrangler");
  const version = wranglerVersion(entry);
  // The schema as SQLite keeps it, in the order it was made, without SQLite's own tables, a
  // virtual table's shadow tables (its module makes them) and D1's `_cf_` tables (D1 refuses
  // them).
  const schema = new DatabaseSync(path, { readOnly: true });
  let ddl: string[];
  try {
    ddl = (
      schema
        .prepare(
          `select s.sql from sqlite_schema s where s.sql is not null and substr(s.name, 1, 7) <> 'sqlite_' and substr(s.name, 1, 4) <> '_cf_'
           and not exists (select 1 from pragma_table_list p where p.name = s.name and p.type = 'shadow') order by s.rowid`,
        )
        .all() as { sql: string }[]
    ).map((r) => r.sql);
  } finally {
    schema.close();
  }
  const extension = import.meta.url.slice(import.meta.url.lastIndexOf("."));
  const child = fork(fileURLToPath(new URL(`./d1-proxy${extension}`, import.meta.url)), [entry, dirname(require.resolve("./package.json"))], {
    stdio: ["ignore", "ignore", "ignore", "ipc"],
    env: { ...process.env, WRANGLER_SEND_METRICS: "false" },
  });
  child.unref();
  type Reply = { kind: "ready"; d1: boolean } | { kind: "schema" } | { kind: "explain"; id: number; error?: string };
  const pending = new Map<number, (error: string | undefined) => void>();
  let next = 0;
  const gone = new Promise<never>((_, fail) => child.once("exit", () => fail(new Error("D1's process ended"))));
  gone.catch(() => {});
  const reply = <K extends Reply["kind"]>(kind: K) =>
    Promise.race([gone, new Promise<Extract<Reply, { kind: K }>>((done) => {
      const on = (m: Reply) => {
        if (m.kind !== kind) return;
        child.off("message", on);
        done(m as Extract<Reply, { kind: K }>);
      };
      child.on("message", on);
    })]);
  // The channel keeps this process alive only while a statement waits on its answer, so a type
  // check or a build ends when its own work does.
  child.on("message", (m: Reply) => {
    if (m.kind !== "explain") return;
    pending.get(m.id)?.(m.error);
    pending.delete(m.id);
    if (!pending.size) child.channel?.unref();
  });
  child.once("exit", () => {
    for (const settle of pending.values()) settle(undefined);
    pending.clear();
  });
  const stop = () => {
    if (child.connected) child.send({ kind: "close" });
    setTimeout(() => child.kill(), 2_000).unref();
  };
  try {
    const timeout = new Promise<never>((_, fail) => setTimeout(() => fail(new Error("workerd did not start")), D1_START).unref());
    const ready = await Promise.race([reply("ready"), timeout]);
    if (!ready.d1) throw new Error("the wrangler config binds no D1 database");
    child.send({ kind: "schema", ddl });
    await Promise.race([reply("schema"), timeout]);
  } catch (e) {
    stop();
    throw e;
  }
  child.channel?.unref();
  return {
    name: "wrangler",
    says: (refusal) => `Cloudflare D1, as wrangler ${version} runs it locally, refuses this statement: ${refusal}`,
    refuses: (sql, params) =>
      new Promise((done) => {
        if (!child.connected) return done(undefined);
        const id = next++;
        pending.set(id, (error) => {
          // D1 wraps SQLite's message: "D1_ERROR: no such function: x at offset 7: SQLITE_ERROR".
          const message = error?.replace(/^D1_ERROR: /, "").replace(/ at offset \d+/, "").replace(/: SQLITE_\w+$/, "");
          // A count of bindings D1 reads otherwise than the analysis is no refusal of the statement.
          done(message && !/^Wrong number of parameter bindings/.test(message) ? message : undefined);
        });
        child.channel?.ref();
        child.send({ kind: "explain", id, sql, params });
      }),
    async close() {
      stop();
    },
  };
}

/** The installed wrangler's version, from the package.json above its entry point. */
function wranglerVersion(entry: string): string {
  for (let dir = dirname(entry); dirname(dir) !== dir; dir = dirname(dir)) {
    const file = join(dir, "package.json");
    if (!existsSync(file)) continue;
    const pkg = JSON.parse(readFileSync(file, "utf8")) as { name?: string; version?: string };
    if (pkg.name === "wrangler") return pkg.version ?? "";
  }
  return "";
}

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
 * "wrong number of arguments to function x()", D1's "not authorized to use function: x"), else the text a syntax error is near, else the
 * statement's first word. In UTF-8 bytes, as the analysis's diagnostics are.
 */
export function placeRefusal(sql: string, message: string): { start: number; length: number } {
  const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const fn = /(?:no such function|not authorized to use function): ([\w$]+)/i.exec(message)?.[1] ?? /arguments to function ([\w$]+)\(\)/i.exec(message)?.[1];
  const near = /near "([^"]+)"/.exec(message)?.[1];
  const found = fn ? new RegExp(`\\b${escape(fn)}\\s*\\(`, "i").exec(sql) : near ? new RegExp(escape(near), "i").exec(sql) : null;
  const at = found ? { index: found.index, text: fn ?? near! } : { index: sql.search(/\S/), text: /\S+/.exec(sql)?.[0] ?? "" };
  return { start: utf8.encode(sql.slice(0, Math.max(at.index, 0))).length, length: Math.max(utf8.encode(at.text).length, 1) };
}
