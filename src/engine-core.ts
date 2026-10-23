// The engine's half that needs no host: a call into the WASI reactor's exports, the lexer, and the
// replay loop that hands each round's needs to an `EngineIo` until the analysis is done. It imports
// nothing from Node, so a browser runs it beside the same `engine.wasm` that src/engine.ts loads
// with `node:wasi`.
import { Unreachable } from "./connection.ts";
import type { EngineType } from "./engine-type.ts";

export type { EngineType } from "./engine-type.ts";
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

export type Response =
  | { status: "ok"; version: string; fingerprint: string; database?: Dialect }
  | { status: "lexed"; lexed: Lexed }
  | { status: "need"; needs: Need[] }
  | { status: "analysis"; analysis: Analysis }
  | { status: "config"; config: unknown }
  | { status: "configError"; error: { message: string; start: number; end: number } }
  | { status: "error"; message: string };

/** A WebAssembly memory and an instance's exports, typed here rather than through the DOM lib. */
export type Memory = { buffer: ArrayBuffer };
export type Instance = { exports: Record<string, unknown> };

/** An instantiated engine: `instance` is `engine.wasm` with its WASI imports, initialized as a reactor. */
export class EngineCore {
  #memory: Memory;
  #alloc: (len: number) => number;
  #free: (ptr: number, len: number) => void;
  #call: (ptr: number, len: number) => number;

  constructor(instance: Instance) {
    const e = instance.exports as Record<string, any>;
    this.#memory = e.memory;
    this.#alloc = e.reylo_alloc;
    this.#free = e.reylo_free;
    this.#call = e.reylo_call;
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
