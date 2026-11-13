// A child process that compiles statements on Cloudflare D1 as the project's wrangler runs it
// locally, for src/sqlite-drivers.ts. It runs apart from the type check because wrangler and
// workerd write to stdout, which is the content mapper's channel to TypeScript; its own stdout and
// stderr go nowhere, and it answers over IPC.
//
// `getPlatformProxy` starts workerd with the bindings of the project's wrangler config. The first D1
// binding, held in memory (`persist: false`) and never the remote database (`remoteBindings:
// false`: a binding marked `remote` would otherwise be the production database, and the schema
// replayed into it), is given the type check's schema, so a statement compiles against the tables
// the analysis read; then each statement is compiled with EXPLAIN, which runs D1's checks (its
// allowlist of functions is an authorizer SQLite calls while it compiles) without running the
// statement.
//
// A project with SQLite-backed Durable Objects and no D1 database is asked too: each such object's
// storage is Cloudflare's SQLite, and wrangler runs a local D1 database as a Durable Object's storage (its
// files are under `miniflare-D1DatabaseObject`), so both refuse the same functions. Such a project
// gets a D1 database of a config of Reylo's own, in a directory of its own, in memory as well.
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

type D1Statement = { bind(...values: unknown[]): D1Statement; all(): Promise<{ results: unknown[] }>; run(): Promise<unknown> };
type D1 = { prepare(sql: string): D1Statement; batch(statements: D1Statement[]): Promise<unknown> };
type Message = { kind: "schema"; ddl: string[] } | { kind: "explain"; id: number; sql: string; params: number } | { kind: "close" };

const send = (message: object) => process.send?.(message);
const [wrangler, dir] = process.argv.slice(2);
process.chdir(dir!);
const { getPlatformProxy } = (await import(pathToFileURL(wrangler!).href)) as {
  getPlatformProxy(options: { configPath?: string; persist: boolean; remoteBindings: boolean }): Promise<{ env: Record<string, unknown>; dispose(): Promise<void> }>;
};
const isD1 = (v: unknown): v is D1 => typeof (v as D1 | undefined)?.prepare === "function" && typeof (v as D1).batch === "function";
let proxy = await getPlatformProxy({ persist: false, remoteBindings: false });
let db = Object.values(proxy.env).find(isD1);
let scratch: string | undefined;
const config = ["wrangler.json", "wrangler.jsonc", "wrangler.toml"].filter((name) => existsSync(name)).map((name) => readFileSync(name, "utf8")).join("\n");
// Only a SQLite-backed class has SQL storage: one its migrations declare under `new_sqlite_classes`,
// or one `exports` declares with `storage` "sqlite". A key-value backed class (`new_classes`,
// `storage` "legacy-kv") has none, and a binding to another Worker's class (`script_name`) leaves
// that Worker's storage to that Worker.
if (!db && /\bnew_sqlite_classes\b|\bstorage"?\s*[:=]\s*"sqlite"/.test(config)) {
  await proxy.dispose().catch(() => {});
  scratch = mkdtempSync(join(tmpdir(), "reylo-d1-"));
  const date = /compatibility_date"?\s*[:=]\s*"(\d{4}-\d{2}-\d{2})"/.exec(config)?.[1];
  const own = { name: "reylo-sqlite-check", ...(date && { compatibility_date: date }), d1_databases: [{ binding: "DB", database_name: "reylo-sqlite-check", database_id: "00000000-0000-0000-0000-000000000000" }] };
  writeFileSync(join(scratch, "wrangler.json"), JSON.stringify(own));
  proxy = await getPlatformProxy({ configPath: join(scratch, "wrangler.json"), persist: false, remoteBindings: false });
  db = Object.values(proxy.env).find(isD1);
}
send({ kind: "ready", d1: db !== undefined, storage: scratch ? "durable-object" : "d1" });

/** Ends the proxy, and the directory of Reylo's own config where there is one. */
async function end() {
  await proxy.dispose().catch(() => {});
  if (scratch) rmSync(scratch, { recursive: true, force: true });
}

/**
 * Makes D1's schema the one given: what an earlier schema made is dropped (views first, then
 * tables, which take their indexes and triggers with them; a virtual table's shadow tables go
 * with it), then each statement runs, one at a time: a table D1 cannot make (a module its build
 * lacks) is left out, and a statement that reads it fails as it would on D1.
 */
async function replay(d1: D1, ddl: string[]) {
  const { results } = await d1
    .prepare("select type, name from sqlite_schema where type in ('view', 'table') and substr(name, 1, 7) <> 'sqlite_' and substr(name, 1, 4) <> '_cf_' order by type = 'table', rowid")
    .all();
  for (const { type, name } of results as { type: string; name: string }[]) await d1.prepare(`drop ${type} if exists "${name.replaceAll('"', '""')}"`).run().catch(() => {});
  for (const statement of ddl) await d1.prepare(statement).run().catch(() => {});
}

async function handle(message: Message) {
  if (message.kind === "close" || !db) {
    await end();
    process.exit(0);
  }
  if (message.kind === "schema") {
    await replay(db, message.ddl).catch(() => {});
  } else if (message.kind === "explain") {
    try {
      await db.prepare(`explain ${message.sql}`).bind(...Array.from({ length: message.params }, () => null)).all();
      send({ kind: "explain", id: message.id });
    } catch (e) {
      send({ kind: "explain", id: message.id, error: (e as Error).message });
    }
  }
}

// One message at a time, in the order they came: a new schema is in place before the statements
// sent after it compile.
let queue = Promise.resolve();
process.on("message", (message: Message) => {
  queue = queue.then(() => handle(message));
});
process.on("disconnect", () => {
  void end().finally(() => process.exit(0));
});
