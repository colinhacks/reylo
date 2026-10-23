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
import { Unreachable } from "./connection.ts";
import type { EngineType } from "./engine-type.ts";

export type { EngineType } from "./engine-type.ts";
export { reason, Unreachable } from "./connection.ts";
export type Dialect = "postgres" | "sqlite" | "mysql";

export type Diagnostic = { message: string; start: number; length: number; code: number };

export type Analysis = {
  kind: string;
  /** `nullable`: the statement lets the caller pass null (it passes the parameter first to coalesce, or writes it whole and bare, with no cast, to a column that may hold null). */
  /** `cast`: a `::type` written on MySQL or SQLite, which the run time checks a value against (src/rows.ts). */
  /** `comment`: the comment the database keeps on the column a parameter is written to (or, on MySQL, compared with), and on the table column a result column reads as it is; the declaration prints it as the field's doc comment. */
  params: { name: string; type: EngineType; type_name: string; nullable: boolean; schema?: import("./engine-type.ts").JsonSchema; cast?: string; comment?: string }[];
  columns: { name: string; type: EngineType; nullable: boolean; type_name: string; comment?: string }[];
  cardinality: "one" | "maybeOne" | "many" | "none";
  cardinalityReason?: string;
  rewritten: { sql: string; order: string[]; spans: [number, number, number, number][] };
  style: "none" | "named" | "positional" | "mixed";
  diagnostics: Diagnostic[];
  unresolved: string[];
  /** Each structured parameter in a MySQL insert, with the lists of `rewritten.sql` (UTF-8 byte offsets) the run time writes again for a call that leaves keys out; a Postgres or SQLite insert copies its defaults into the statement instead. */
  structured?: { name: string; lists: { start: number; end: number; entries: string[] }[] }[];
  /** The catalog facts the analysis relied on, and the query that reads them (engine/core/src/fresh.rs). */
  fresh?: { query: string; rows: string[][]; inserts: string[]; defaults?: string[][] };
};

export type Lexed = {
  params: string[];
  placeholders: { name: string; start: number; end: number }[];
  style: "none" | "named" | "positional" | "mixed";
  /** The spans the lexer read as text (strings, quoted identifiers, comments): `[start, end]` byte offsets, in order. */
  opaque: [number, number][];
};

export type Need =
  | { kind: "describe"; id: string; sql: string }
  | { kind: "query"; id: string; sql: string; params: unknown[]; scope: "session" | "statement" };

export type Described = { params: { oid: number }[]; columns: { name: string; oid: number; table: number; attnum: number }[] };
/** MySQL's prepare metadata: the protocol type code, the flags (bit 1 is NOT NULL), the charset (63 is binary), the display length and the table the column is one of (empty for an expression). */
export type MysqlDescribed = { kind: "mysql"; columns: { name: string; type: number; flags: number; charset: number; length: number; org_table: string; org_name: string }[]; params: { type: number; flags: number; charset: number }[] };
export type DbError = { message: string; code?: string; position?: number };
export type Answer =
  | { describe: { params: number[]; columns: Described["columns"] } }
  | { mysql: { columns: MysqlDescribed["columns"]; params: MysqlDescribed["params"] } }
  | { rows: unknown[][] }
  | { error: DbError };

/** What the host does for the engine: describe a statement, run a catalog query. */
export interface EngineIo {
  /** Postgres: Parse/Describe. MySQL: a server prepare. SQLite needs none: the engine prepares inside its own SQLite. */
  describe(sql: string): Promise<Described | MysqlDescribed>;
  /** Rows as arrays of values, in column order. */
  query(sql: string, params: unknown[]): Promise<unknown[][]>;
}

type Response =
  | { status: "ok"; version: string; fingerprint: string; database?: Dialect }
  | { status: "lexed"; lexed: Lexed }
  | { status: "need"; needs: Need[] }
  | { status: "analysis"; analysis: Analysis }
  | { status: "config"; config: unknown }
  | { status: "configError"; error: { message: string; start: number; end: number } }
  | { status: "error"; message: string };

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
type Memory = { buffer: ArrayBuffer };
type Instance = { exports: Record<string, unknown> };
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

export class Engine {
  #memory: Memory;
  #alloc: (len: number) => number;
  #free: (ptr: number, len: number) => void;
  #call: (ptr: number, len: number) => number;

  private constructor(instance: Instance) {
    const e = instance.exports as Record<string, any>;
    this.#memory = e.memory;
    this.#alloc = e.reylo_alloc;
    this.#free = e.reylo_free;
    this.#call = e.reylo_call;
  }

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

  call(request: object): Response {
    const bytes = new TextEncoder().encode(JSON.stringify(request));
    const ptr = this.#alloc(bytes.length);
    new Uint8Array(this.#memory.buffer, ptr, bytes.length).set(bytes);
    const out = this.#call(ptr, bytes.length);
    this.#free(ptr, bytes.length);
    const len = new DataView(this.#memory.buffer).getUint32(out, true);
    const text = new TextDecoder().decode(new Uint8Array(this.#memory.buffer, out + 4, len));
    this.#free(out, len + 4);
    return JSON.parse(text);
  }

  lex(sql: string, dialect: Dialect): Lexed {
    const r = this.call({ op: "lex", sql, dialect });
    if (r.status !== "lexed") throw new Error(r.status === "error" ? r.message : `unexpected ${r.status}`);
    return r.lexed;
  }

  /** Analyze one statement, fulfilling the engine's needs through `io` until it answers. */
  async analyze(sql: string, dialect: Dialect, io: EngineIo, cache = new AnswerCache()): Promise<Analysis> {
    const answers: Record<string, Answer> = {};
    for (let round = 0; round < 16; round++) {
      const r = this.call({ op: "analyze", sql, dialect, answers });
      if (r.status === "analysis") return r.analysis;
      if (r.status !== "need") throw new Error(r.status === "error" ? r.message : `unexpected ${r.status}`);
      await Promise.all(r.needs.map(async (need) => { answers[need.id] = await cache.get(need, () => fulfil(need, io)); }));
    }
    throw new Error("the analysis engine did not converge");
  }
}

async function fulfil(need: Need, io: EngineIo): Promise<Answer> {
  try {
    if (need.kind === "describe") {
      const d = await io.describe(need.sql);
      if ("kind" in d) return { mysql: { columns: d.columns, params: d.params } };
      return { describe: { params: d.params.map((p) => p.oid), columns: d.columns } };
    }
    return { rows: await io.query(need.sql, need.params) };
  } catch (e) {
    if (e instanceof Unreachable) throw e;
    const err = e as Error & { code?: string; position?: string | number };
    const error: DbError = { message: err.message };
    if (err.code) error.code = err.code;
    if (err.position !== undefined) error.position = Number(err.position);
    return { error };
  }
}

/**
 * Answers the host may reuse. A session answer is about the database as a whole (which
 * operators return sets, which built-ins are shadowed); a statement answer is about a
 * statement's relations and types. Both go stale when the schema changes, so both expire:
 * a migration shows up within seconds, and a transform of the same text a moment later
 * costs no round trip.
 */
export class AnswerCache {
  #entries = new Map<string, { at: number; answer: Answer }>();
  #ttl: { session: number; statement: number };
  constructor(ttl = { session: 60_000, statement: 5_000 }) {
    this.#ttl = ttl;
  }

  async get(need: Need, fetch: () => Promise<Answer>): Promise<Answer> {
    const ttl = need.kind === "query" && need.scope === "session" ? this.#ttl.session : this.#ttl.statement;
    const hit = this.#entries.get(need.id);
    if (hit && Date.now() - hit.at < ttl) return hit.answer;
    const answer = await fetch();
    // A failure is not knowledge: the next transform asks again.
    if (!("error" in answer)) this.#entries.set(need.id, { at: Date.now(), answer });
    return answer;
  }

  clear() {
    this.#entries.clear();
  }
}
