// The analysis engine: one WebAssembly component per database, in the `engine/` directory of the
// database's npm package (`@reylosql/postgres`, `@reylosql/sqlite`, `@reylosql/mysql`), which a
// project installs beside reylo and which is found from the project first, then from reylo's own
// install; each loads once per process. The engine connects to the database itself: the analysis
// of a statement is one request on a connection it opened (engine/native, built as a component by
// engine/build.sh), whose sockets jco's preview2-shim opens as Node's (src/component.ts). The
// requests that need no database (`ping`, `lex`, `config`) go to an instance in this thread, which
// answers at once; the rest go to an instance in a worker (src/engine-worker.ts), where a call may
// wait on the network without holding this thread.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { Worker } from "node:worker_threads";
import { instantiate, type ComponentExports, type CoreModule, type Instantiate } from "./component.ts";
import { NotMysql, Unreachable } from "./connection.ts";
import { EngineCore, type Analysis, type Described, type Dialect } from "./engine-core.ts";

export type { EngineType } from "./engine-type.ts";
export { reason, Unreachable } from "./connection.ts";
export type { Analysis, Answer, DbError, Described, Diagnostic, Dialect, EngineIo, Lexed, MysqlDescribed, Need, Response } from "./engine-core.ts";
export { AnswerCache, EngineCore } from "./engine-core.ts";

/**
 * Each database's engine: an npm package that carries its component, transpiled into `engine/`,
 * and reylo's build-time half (src/tooling.ts). Each is an optional peer of reylo's, at reylo's own
 * version, so a project installs the ones for its databases. Every one carries the same core, which
 * lexes and reads the project file.
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
 * Where `dialect`'s engine is, the `engine/` directory of its package: the project's own install
 * first, then reylo's. `from` is where reylo looks after the project, its own module unless a test
 * says otherwise.
 */
export function enginePath(dialect: Dialect, projectDir: string, from: string = import.meta.url): string | undefined {
  for (const base of [join(projectDir, "package.json"), from]) {
    try {
      const dir = join(dirname(createRequire(base).resolve(`${ENGINE_PACKAGES[dialect]}/package.json`)), "engine");
      if (existsSync(join(dir, "engine.js"))) return dir;
    } catch {}
  }
  return undefined;
}

const engines = new Map<string, Promise<Engine>>();

/**
 * The engine of one database as `projectDir` finds it, loaded once per process and directory. It
 * rejects with a MissingEngine where none is installed, where the package is another version than
 * reylo, and where its component reads another database's statements.
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
const WA = (globalThis as unknown as { WebAssembly: { compile(bytes: Uint8Array): Promise<CoreModule> } }).WebAssembly;

type Answer = { status: string; message?: string; code?: string; conn?: number; analysis?: Analysis; rows?: unknown[][]; server?: string; params?: number[]; columns?: Described["columns"] };

/** What a session asks of a connection the engine holds. */
export interface Connection {
  analyze(sql: string): Promise<Analysis>;
  /** A query, its parameters bound and its rows read as the engine's catalog queries are. */
  query(sql: string, params: unknown[]): Promise<unknown[][]>;
  /** A statement described as the engine's own describe reads it, for a host that runs the engine's analysis itself (EngineCore's `analyze`); Postgres only. */
  describe(sql: string): Promise<Described>;
  /** The schema changed: the answers the engine keeps for this connection are stale. */
  forget(): Promise<void>;
  close(): Promise<void>;
}

/** A database's engine component, loaded from its package's `engine/`. */
export class Engine extends EngineCore {
  readonly path: string;
  readonly #modules: Record<string, CoreModule>;
  #worker: Worker | undefined;
  #pending = new Map<number, (answer: Answer) => void>();
  #next = 1;

  private constructor(path: string, modules: Record<string, CoreModule>, here: ComponentExports) {
    super((request) => here.call(request));
    this.path = path;
    this.#modules = modules;
  }

  /** The component in `dir`: its core modules compiled once, here and for the worker alike. */
  static async load(dir: string): Promise<Engine> {
    const modules: Record<string, CoreModule> = {};
    await Promise.all(readdirSync(dir).filter((n) => n.endsWith(".wasm")).map(async (n) => void (modules[n] = await WA.compile(readFileSync(join(dir, n))))));
    const { instantiate: run } = (await import(pathToFileURL(join(dir, "engine.js")).href)) as { instantiate: Instantiate };
    return new Engine(dir, modules, instantiate(run, modules, false));
  }

  /** A connection to the database `url` names (a SQLite database's path), opened by the engine; an Unreachable where it cannot be. */
  async open(url: string): Promise<Connection> {
    const conn = (await this.#ask({ op: "open", url })).conn!;
    return {
      analyze: async (sql) => (await this.#ask({ op: "run", conn, sql })).analysis!,
      query: async (sql, params) => (await this.#ask({ op: "query", conn, sql, params })).rows!,
      describe: async (sql) => {
        const d = await this.#ask({ op: "describe", conn, sql });
        return { params: d.params!.map((oid) => ({ oid })), columns: d.columns! };
      },
      forget: async () => void (await this.#ask({ op: "forget", conn })),
      close: async () => void (await this.#ask({ op: "close", conn }).catch(() => {})),
    };
  }

  /**
   * A request to the engine's worker. A refused login and a database that cannot be reached are an
   * Unreachable, its cause carrying the server's code, so that `refusedLogin` reads it
   * (src/connection.ts); a MariaDB server, which the type check refuses, is a NotMysql; any other
   * failure is an Error.
   */
  async #ask(request: object): Promise<Answer> {
    const worker = this.#start();
    const id = this.#next++;
    const answer = await new Promise<Answer>((resolve) => {
      this.#pending.set(id, resolve);
      worker.ref();
      worker.postMessage(JSON.stringify({ ...request, id }));
    });
    if (answer.status === "notMysql") throw new NotMysql(answer.server!);
    if (answer.status === "unreachable" || answer.status === "refused") throw new Unreachable(Object.assign(new Error(answer.message), answer.code && { code: answer.code }));
    if (answer.status === "error") throw Object.assign(new Error(answer.message), answer.code && { code: answer.code });
    return answer;
  }

  /** The worker, started on the first request: it keeps this process alive only while a request waits on it, so that a build ends when its work does. */
  #start(): Worker {
    if (this.#worker) return this.#worker;
    // The worker's own file, beside this one: a `.ts` file in this repository, a `.js` one in an engine package.
    const extension = import.meta.url.slice(import.meta.url.lastIndexOf("."));
    const worker = new Worker(new URL(`./engine-worker${extension}`, import.meta.url), { workerData: { js: pathToFileURL(join(this.path, "engine.js")).href, modules: this.#modules } });
    this.#worker = worker;
    worker.on("message", (line: string) => {
      let answer: Answer & { id?: number };
      try {
        answer = JSON.parse(line);
      } catch {
        return;
      }
      const resolve = this.#pending.get(answer.id!);
      if (!resolve) return;
      this.#pending.delete(answer.id!);
      resolve(answer);
      if (this.#pending.size === 0) worker.unref();
    });
    const ended = (why: string) => {
      if (this.#worker !== worker) return;
      this.#worker = undefined;
      for (const resolve of this.#pending.values()) resolve({ status: "unreachable", message: why });
      this.#pending.clear();
    };
    worker.on("error", (e: Error) => ended(`the engine ${this.path} failed: ${e.message}`));
    worker.on("exit", (code) => ended(`the engine ${this.path} stopped (${code})`));
    return worker;
  }
}
