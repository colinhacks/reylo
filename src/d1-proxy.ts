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
import { pathToFileURL } from "node:url";

type D1Statement = { bind(...values: unknown[]): D1Statement; all(): Promise<{ results: unknown[] }>; run(): Promise<unknown> };
type D1 = { prepare(sql: string): D1Statement; batch(statements: D1Statement[]): Promise<unknown> };
type Message = { kind: "schema"; ddl: string[] } | { kind: "explain"; id: number; sql: string; params: number } | { kind: "close" };

const send = (message: object) => process.send?.(message);
const [wrangler, dir] = process.argv.slice(2);
process.chdir(dir!);
const { getPlatformProxy } = (await import(pathToFileURL(wrangler!).href)) as {
  getPlatformProxy(options: { persist: boolean; remoteBindings: boolean }): Promise<{ env: Record<string, unknown>; dispose(): Promise<void> }>;
};
const proxy = await getPlatformProxy({ persist: false, remoteBindings: false });
const isD1 = (v: unknown): v is D1 => typeof (v as D1 | undefined)?.prepare === "function" && typeof (v as D1).batch === "function";
const db = Object.values(proxy.env).find(isD1);
send({ kind: "ready", d1: db !== undefined });

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
    await proxy.dispose().catch(() => {});
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
  void proxy.dispose().finally(() => process.exit(0));
});
