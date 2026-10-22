// A child process that compiles statements on Cloudflare D1 as the project's wrangler runs it
// locally, for src/sqlite-drivers.ts. It runs apart from the type check because wrangler and
// workerd write to stdout, which is the content mapper's channel to TypeScript; its own stdout and
// stderr go nowhere, and it answers over IPC.
//
// `getPlatformProxy` starts workerd with the bindings of the project's wrangler config. The first D1
// binding, held in memory (`persist: false`), is given the type check's schema, so a statement
// compiles against the tables the analysis read; then each statement is compiled with EXPLAIN,
// which runs D1's checks (its allowlist of functions is an authorizer SQLite calls while it
// compiles) without running the statement.
import { pathToFileURL } from "node:url";

type D1Statement = { bind(...values: unknown[]): D1Statement; all(): Promise<unknown>; run(): Promise<unknown> };
type D1 = { prepare(sql: string): D1Statement; batch(statements: D1Statement[]): Promise<unknown> };
type Message = { kind: "schema"; ddl: string[] } | { kind: "explain"; id: number; sql: string; params: number } | { kind: "close" };

const send = (message: object) => process.send?.(message);
const [wrangler, dir] = process.argv.slice(2);
process.chdir(dir!);
const { getPlatformProxy } = (await import(pathToFileURL(wrangler!).href)) as {
  getPlatformProxy(options: { persist: boolean }): Promise<{ env: Record<string, unknown>; dispose(): Promise<void> }>;
};
const proxy = await getPlatformProxy({ persist: false });
const isD1 = (v: unknown): v is D1 => typeof (v as D1 | undefined)?.prepare === "function" && typeof (v as D1).batch === "function";
const db = Object.values(proxy.env).find(isD1);
send({ kind: "ready", d1: db !== undefined });

process.on("message", async (message: Message) => {
  if (message.kind === "close" || !db) {
    await proxy.dispose().catch(() => {});
    process.exit(0);
  }
  if (message.kind === "schema") {
    // One statement at a time: a table D1 cannot make (a module its build lacks) is left out, and
    // a statement that reads it fails as it would on D1.
    for (const ddl of message.ddl) await db.prepare(ddl).run().catch(() => {});
    send({ kind: "schema" });
  } else if (message.kind === "explain") {
    try {
      await db.prepare(`explain ${message.sql}`).bind(...Array.from({ length: message.params }, () => null)).all();
      send({ kind: "explain", id: message.id });
    } catch (e) {
      send({ kind: "explain", id: message.id, error: (e as Error).message });
    }
  }
});
process.on("disconnect", () => {
  void proxy.dispose().finally(() => process.exit(0));
});
