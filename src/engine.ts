// The analysis engine: one WASI reactor per database, `engine.wasm` of the database's own npm
// package (`@reylosql/postgres`, `@reylosql/sqlite`, `@reylosql/mysql`), which a project installs
// beside reylo and which is found from the project first, then from reylo's own install; each
// loads once per process. The engine is pure. `analyze` replays it: every round it either answers
// or names what it still needs — statements to describe, catalog queries to run — and this module
// fulfils those through the project's own driver and calls again.
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import type { WASI as WasiClass } from "node:wasi";

export type { EngineType } from "./engine-type.ts";
export { reason, Unreachable } from "./connection.ts";
export type { Analysis, Answer, DbError, Described, Diagnostic, Dialect, EngineIo, Lexed, MysqlDescribed, Need, Response } from "./engine-core.ts";
export { AnswerCache, EngineCore } from "./engine-core.ts";
import { EngineCore, type Dialect, type Instance, type Memory } from "./engine-core.ts";
/**
 * Each database's engine: an npm package that carries its artifact, `engine.wasm`, reylo's
 * build-time half (src/tooling.ts), and on Postgres and MySQL its describer with the driver inside
 * (`describer`). Each is an optional peer of reylo's, at reylo's own version, so a project installs
 * the ones for its databases. Every one carries the same core, which lexes and reads the project
 * file.
 */
export const ENGINE_PACKAGES: Record<Dialect, string> = { postgres: "@reylosql/postgres", sqlite: "@reylosql/sqlite", mysql: "@reylosql/mysql" };

/**
 * The version of reylo, which every engine package it loads must have: that of the package this
 * module ships in, which is reylo's in this repository and an engine package's once built
 * (src/tooling.ts). The two move together.
 */
const REYLO_VERSION: string = (() => {
  for (const at of ["./package.json", "../package.json"]) {
    try {
      return JSON.parse(readFileSync(new URL(at, import.meta.url), "utf8")).version;
    } catch {}
  }
  throw new Error("reylo's build steps cannot find the package.json of their own package");
})();

/** An engine reylo cannot load: none installed, another version than reylo's, or another database's. Reported as the project's error. */
export class MissingEngine extends Error {
  constructor(message: string) {
    super(message);
    this.name = "MissingEngine";
  }

  /** No engine of `dialect` where the project or reylo finds packages. */
  static of(dialect: Dialect): MissingEngine {
    const pkg = ENGINE_PACKAGES[dialect];
    return new MissingEngine(`${dialect} statements need ${pkg}: npm install -D ${pkg}`);
  }

  /** No engine at all, so not even the project file can be read. */
  static none(): MissingEngine {
    return new MissingEngine(`reylo reads the project file with a database's engine, and none is installed: npm install -D @reylosql/postgres, @reylosql/mysql or @reylosql/sqlite, the one for your database`);
  }
}

/**
 * Where `dialect`'s engine is: the project's own install first, as its drivers are found
 * (`describer`, below), then reylo's. `from` is where reylo looks after the project, its own module
 * unless a test says otherwise.
 */
export function enginePath(dialect: Dialect, projectDir: string, from: string = import.meta.url): string | undefined {
  for (const base of [join(projectDir, "package.json"), from]) {
    try {
      return createRequire(base).resolve(`${ENGINE_PACKAGES[dialect]}/engine.wasm`);
    } catch {}
  }
  return undefined;
}

/**
 * The half of `dialect`'s analysis that talks to its database, `@reylosql/<database>/describe`:
 * packages/postgres/describe.ts and packages/mysql/describe.ts, each with its driver inside.
 * Found as its engine is, and loaded once per process and file. SQLite has none: it prepares inside
 * the engine and reads the database file through `node:sqlite`.
 */
export function describer<T>(dialect: "postgres" | "mysql", projectDir: string, from: string = import.meta.url): Promise<T> {
  for (const base of [join(projectDir, "package.json"), from]) {
    let path: string;
    try {
      path = createRequire(base).resolve(`${ENGINE_PACKAGES[dialect]}/describe`);
    } catch {
      continue;
    }
    return import(pathToFileURL(path).href) as Promise<T>;
  }
  return Promise.reject(MissingEngine.of(dialect));
}

const engines = new Map<string, Promise<Engine>>();

/**
 * The engine of one database as `projectDir` finds it, loaded once per process and file. It
 * rejects with a MissingEngine where none is installed, where the package is another version than
 * reylo, and where its artifact reads another database's statements.
 */
export function engine(dialect: Dialect, projectDir: string, from?: string): Promise<Engine> {
  const path = enginePath(dialect, projectDir, from);
  if (!path) return Promise.reject(MissingEngine.of(dialect));
  let loading = engines.get(path);
  if (!loading) {
    loading = (async () => {
      const pkg = ENGINE_PACKAGES[dialect];
      let version: string | undefined;
      try {
        version = JSON.parse(readFileSync(join(dirname(path), "package.json"), "utf8")).version;
      } catch {}
      if (version !== REYLO_VERSION) {
        throw new MissingEngine(`reylo ${REYLO_VERSION} runs with ${pkg} ${REYLO_VERSION}, and the one installed is ${version ?? "of no version"} (${dirname(path)}): npm install -D ${pkg}@${REYLO_VERSION}`);
      }
      const loaded = await Engine.load(path);
      const ping = loaded.call({ op: "ping" });
      if (ping.status !== "ok" || ping.database !== dialect) {
        const reads = ping.status === "ok" && ping.database ? `${ping.database} statements` : "no one database's statements";
        throw new MissingEngine(`${pkg} (${dirname(path)}) carries an engine that reads ${reads}, not ${dialect} ones: npm install -D ${pkg}@${REYLO_VERSION}`);
      }
      return loaded;
    })();
    // A failure is not kept: the package may be installed or replaced while the process runs.
    loading.catch(() => engines.delete(path));
    engines.set(path, loading);
  }
  return loading;
}

/**
 * An engine to read the project file with, before the URL in it names the database: every
 * database's engine carries the same core, which parses the file, so the first installed will do.
 * A MissingEngine where none is installed, or where none that is installed loads.
 */
export async function anyEngine(projectDir: string, from?: string): Promise<Engine | MissingEngine> {
  let failed: MissingEngine | undefined;
  for (const dialect of ["postgres", "mysql", "sqlite"] as const) {
    if (!enginePath(dialect, projectDir, from)) continue;
    try {
      return await engine(dialect, projectDir, from);
    } catch (e) {
      failed ??= e instanceof MissingEngine ? e : new MissingEngine((e as Error).message);
    }
  }
  return failed ?? MissingEngine.none();
}

// The WebAssembly namespace is a global in every runtime reylo supports; it is typed here
// rather than through the DOM lib.
const WA = (globalThis as unknown as { WebAssembly: { compile(bytes: Uint8Array): Promise<unknown>; instantiate(module: unknown, imports: object): Promise<Instance> } }).WebAssembly;

// Node warns "ExperimentalWarning: WASI is an experimental feature" when `node:wasi` first loads
// in a thread, not when a WASI is made, so a static import printed that line before anything else
// in every program run under `reylo/register`. The module is loaded here instead, when the engine
// is, while a filter on `process.emitWarning`, which Node's own warning goes through, swallows that
// one warning and passes every other through. Node 24 and Bun both have `getBuiltinModule`.
const WASI_WARNING = "WASI is an experimental feature and might change at any time";
function loadWasi(): { WASI: typeof WasiClass } {
  const emitWarning = process.emitWarning;
  process.emitWarning = function (this: unknown, warning: string | Error, ...rest: unknown[]) {
    const message = typeof warning === "string" ? warning : warning.message;
    const type = typeof warning === "string" ? (typeof rest[0] === "string" ? rest[0] : (rest[0] as { type?: string } | undefined)?.type) : warning.name;
    if (message === WASI_WARNING && type === "ExperimentalWarning") return;
    return (emitWarning as (...args: unknown[]) => void).call(this, warning, ...rest);
  } as typeof process.emitWarning;
  try {
    return process.getBuiltinModule("node:wasi");
  } finally {
    process.emitWarning = emitWarning;
  }
}

/** The engine loaded from a file with Node's (or Bun's) `node:wasi`. */
export class Engine extends EngineCore {
  static async load(path: URL | string): Promise<Engine> {
    const module = await WA.compile(readFileSync(path));
    const { WASI } = loadWasi();
    const wasi = new WASI({ version: "preview1", args: ["reylo-engine"], env: {} });
    // Bun's node:wasi has neither getImportObject nor initialize: it takes the imports from
    // wasiImport, and its start(instance) only sets the memory of a reactor, which has no _start.
    // Its random_get returns the length instead of 0 before Bun 1.4 (oven-sh/bun#35438), which
    // the engine's first hash map reads as a failure, so it is replaced.
    const bun = typeof (wasi as { getImportObject?: unknown }).getImportObject !== "function";
    let instance: Instance;
    const random_get = (ptr: number, len: number) => {
      const memory = instance.exports.memory as Memory;
      for (let at = 0; at < len; at += 65536) crypto.getRandomValues(new Uint8Array(memory.buffer, ptr + at, Math.min(65536, len - at)));
      return 0;
    };
    instance = await WA.instantiate(module, bun ? { wasi_snapshot_preview1: { ...wasi.wasiImport, random_get } } : wasi.getImportObject());
    if (bun) wasi.start(instance as never);
    else wasi.initialize(instance as never);
    return new Engine(instance);
  }

}
