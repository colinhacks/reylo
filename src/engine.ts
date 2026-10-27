// The analysis engine: one WebAssembly component per database, in the `engine/` directory of the
// database's npm package (`@reylosql/postgres`, `@reylosql/sqlite`, `@reylosql/mysql`), which a
// project installs beside reylo and which is found from the project first, then from reylo's own
// install; each loads once per process. The engine connects to the database itself: the analysis
// of a statement is one request on a connection it opened (engine/native, built as a component by
// engine/build.sh), whose sockets jco's preview2-shim opens as Node's (src/component.ts). The
// requests that need no database (`ping`, `lex`, `config`) go to an instance in this thread, which
// answers at once; each connection's go to an instance in a worker of its own (src/engine-worker.ts),
// where a call may wait on the network without holding this thread or another connection.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { Worker } from "node:worker_threads";
import { guestPath, instantiate, type ComponentExports, type CoreModule, type Instantiate } from "./component.ts";
import { hostUrl, type HostUrl } from "./host-url.ts";
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

  private constructor(path: string, modules: Record<string, CoreModule>, here: ComponentExports) {
    super((request) => here.call(request));
    this.path = path;
    this.#modules = modules;
  }

  /** The component in `dir`: its core modules compiled once, here and for each connection's worker alike. */
  static async load(dir: string): Promise<Engine> {
    const modules: Record<string, CoreModule> = {};
    await Promise.all(readdirSync(dir).filter((n) => n.endsWith(".wasm")).map(async (n) => void (modules[n] = await WA.compile(readFileSync(join(dir, n))))));
    const { instantiate: run } = (await import(pathToFileURL(join(dir, "engine.js")).href)) as { instantiate: Instantiate };
    return new Engine(dir, modules, await instantiate(run, modules, false));
  }

  /**
   * A connection to the database `url` names (a SQLite database's path), opened by the engine in a
   * worker of its own, so that a server slow to answer holds up no other connection; an Unreachable
   * where it cannot be opened.
   */
  async open(url: string): Promise<Connection> {
    const target: HostUrl = url.includes("://") ? await hostUrl(url) : { url: url === ":memory:" ? url : guestPath(url), close: () => {} };
    const channel = new Channel(this.path, this.#modules);
    const ask = async (request: object) => settled(await channel.ask(request));
    let conn: number;
    try {
      conn = (await ask({ op: "open", url: target.url })).conn!;
    } catch (e) {
      channel.end();
      target.close();
      throw e;
    }
    return {
      analyze: async (sql) => (await ask({ op: "run", conn, sql })).analysis!,
      query: async (sql, params) => (await ask({ op: "query", conn, sql, params })).rows!,
      describe: async (sql) => {
        const d = await ask({ op: "describe", conn, sql });
        return { params: d.params!.map((oid) => ({ oid })), columns: d.columns! };
      },
      forget: async () => void (await ask({ op: "forget", conn })),
      close: async () => {
        await ask({ op: "close", conn }).catch(() => {});
        channel.end();
        target.close();
      },
    };
  }
}

/**
 * An answer as the session reads it. A refused login and a database that cannot be reached are an
 * Unreachable, its cause carrying the server's code, so that `refusedLogin` reads it
 * (src/connection.ts); a MariaDB server, which the type check refuses, is a NotMysql; any other
 * failure is an Error.
 */
function settled(answer: Answer): Answer {
  if (answer.status === "notMysql") throw new NotMysql(answer.server!);
  if (answer.status === "unreachable" || answer.status === "refused") throw new Unreachable(Object.assign(new Error(answer.message), answer.code && { code: answer.code }));
  if (answer.status === "error") throw Object.assign(new Error(answer.message), answer.code && { code: answer.code });
  return answer;
}

/**
 * One connection's worker: the component, its network on, answering that connection's requests in
 * turn. It keeps this process alive only while a request waits on it, so that a build ends when its
 * work does; a worker that fails or stops answers what waits on it, and everything after, as a
 * database that cannot be reached, and the session opens another.
 */
class Channel {
  readonly #worker: Worker;
  readonly #pending = new Map<number, (answer: Answer) => void>();
  #next = 1;
  #ended: string | undefined;

  constructor(path: string, modules: Record<string, CoreModule>) {
    // The worker's own file, beside this one: a `.ts` file in this repository, a `.js` one in an engine package.
    const extension = import.meta.url.slice(import.meta.url.lastIndexOf("."));
    // Without the app's own flags (a preload such as reylo/register), and without Node's watch-mode
    // report, which would arrive here as messages for each module the worker loads (src/component.ts).
    const env = { ...process.env };
    delete env.WATCH_REPORT_DEPENDENCIES;
    const worker = new Worker(new URL(`./engine-worker${extension}`, import.meta.url), { workerData: { js: pathToFileURL(join(path, "engine.js")).href, modules }, env, execArgv: [] });
    this.#worker = worker;
    worker.unref();
    worker.on("message", (line: unknown) => {
      if (typeof line !== "string") return;
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
      this.#ended ??= why;
      for (const resolve of this.#pending.values()) resolve({ status: "unreachable", message: this.#ended });
      this.#pending.clear();
    };
    worker.on("error", (e: Error) => ended(`the engine ${path} failed: ${e.message}`));
    worker.on("exit", (code) => ended(`the engine ${path} stopped (${code})`));
  }

  ask(request: object): Promise<Answer> {
    if (this.#ended) return Promise.resolve({ status: "unreachable", message: this.#ended });
    const id = this.#next++;
    return new Promise<Answer>((resolve) => {
      this.#pending.set(id, resolve);
      this.#worker.ref();
      this.#worker.postMessage(JSON.stringify({ ...request, id }));
    });
  }

  /** The worker stopped, with the connection it held. */
  end(): void {
    this.#ended ??= "the connection is closed";
    void this.#worker.terminate();
  }
}
