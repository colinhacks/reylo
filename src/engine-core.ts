// The engine's half that needs no host: a request to the engine, the lexer, and the replay loop
// that hands each round's needs to an `EngineIo` until the analysis is done, for a host that answers
// them over a connection of its own. It imports nothing from Node, so a browser runs it beside a
// WebAssembly build of the engine's core, as the docs site's playground does; src/engine.ts runs it
// over the native engine.
import { Unreachable } from "./connection.ts";
import type { EngineType } from "./engine-type.ts";

export type { EngineType } from "./engine-type.ts";
export type Dialect = "postgres" | "sqlite" | "mysql" | "duckdb";

/** A diagnostic as Reylo's own tools read it: its place in UTF-8 bytes, Reylo's code, and the database's own code where the database raised it (`server_code`). */
export type Diagnostic = { message: string; start: number; length: number; code: number; server_code?: string };

/**
 * What the engine answers for a statement: the format's statement descriptor (spec/index.ts,
 * `StatementDescriptor`) less its dialect and with Reylo's diagnostics, and what Reylo's own tools
 * read beside it.
 */
export type Analysis = {
  /** Absent when the statement did not parse; `other` for a statement none of the five. */
  kind?: "select" | "insert" | "update" | "delete" | "merge" | "other";
  /** The prose written in comments before the statement. */
  comment?: string;
  /** What is worth saying of the statement but fails nothing, for a hover. */
  notes?: string[];
  /** `nullable`: the statement lets the caller pass null. `optional`: the caller may leave the parameter out (a typed JSON parameter whose schema has a `default`). */
  /** `cast`: a `::type` written on MySQL or SQLite, which the run time checks a value against (src/rows.ts). `members`: a structured parameter's column list as written, by field. `delimiter`: what separates an array's elements in its text, where it is not a comma. */
  /** `comment`: the comment the database keeps on the column a parameter is written to (or, on MySQL, compared with), and on the table column a result column reads as it is; the declaration prints it as the field's doc comment. */
  params: { name: string; type: EngineType; nullable?: boolean; optional?: boolean; comment?: string; schema?: import("./engine-type.ts").JsonSchema; cast?: string; delimiter?: string; members?: { written: string; schema?: import("./engine-type.ts").JsonSchema }[] }[];
  columns: { name: string; type: EngineType; nullable?: boolean; comment?: string; delimiter?: string }[];
  cardinality: "one" | "at_most_one" | "many" | "none";
  /** The result is the one column's values rather than rows that hold them: the statement gives the column no name, so its only name is one the database makes up. */
  value?: true;
  /** Each domain a type names, with the base type it is made from, which a client decodes its values by (src/engine-type.ts, `withBases`). */
  domains?: Record<string, string>;
  cardinalityReason?: string;
  /** The result key: output columns, by name, no two rows of the result agree on all of (a NULL equal to another NULL). Only for a result that can hold more than one row: absent where the cardinality is `one` or `maybeOne`, and where no key is proven (engine/core/src/api.rs). */
  key?: string[];
  /** Every set of output columns no two rows agree on all of, as `key` is, a table's primary key first where the result holds it. */
  unique?: string[][];
  /** The columns of the result its ORDER BY sorts on, as far as its keys are such columns. */
  order?: { column: string; desc?: true; nulls: "first" | "last" }[];
  /** Whether the statement's ORDER BY orders its rows totally: no two rows of a result tie on every sort key, so the order is the same on every run. Only for a result that can hold more than one row, as `key` is (engine/core/src/api.rs). */
  ordered?: true;
  /** `inserted`: text the engine added that the statement as written does not hold, as `[original offset, start, end]` (the collation MySQL's own text needs). */
  rewritten: { sql: string; order: string[]; spans: [number, number, number, number][]; inserted?: [number, number, number][] };
  style: "none" | "named" | "positional" | "mixed";
  diagnostics: Diagnostic[];
  unresolved: string[];
  /** Each structured parameter in a MySQL insert, with the lists of `rewritten.sql` (UTF-8 byte offsets) the run time writes again for a call that leaves keys out; a Postgres or SQLite insert copies its defaults into the statement instead. */
  structured?: { name: string; lists: { start: number; end: number; entries: string[] }[] }[];
  /** The catalog facts the analysis relied on, and the query that reads them (engine/core/src/fresh.rs). */
  fresh?: { query: string; rows: string[][]; inserts: string[]; defaults?: string[][] };
  /**
   * By each error's code, the names each of its fields may hold, where the statement runs only its
   * database's own code (engine/core/src/errors.rs): `.try()`'s error, narrowed. Absent where
   * the statement may run a user's code, which may raise any error.
   */
  errors?: Record<string, Record<string, string[]>>;
  /** Whether the statement is certain to write nothing. */
  readOnly?: boolean;
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

export type Described = { params: { oid: number }[]; columns: { name: string; oid: number; table: number; attnum: number; typmod?: number }[] };
/** MySQL's prepare metadata: the protocol type code, the flags (bit 1 is NOT NULL), the charset (63 is binary), the length, the decimals (a DECIMAL's scale, a time's fractional digits) and the schema and table the column is one of (empty for an expression). */
export type MysqlDescribed = { kind: "mysql"; columns: { name: string; type: number; flags: number; charset: number; length: number; decimals?: number; schema?: string; org_table: string; org_name: string }[]; params: { type: number; flags: number; charset: number }[] };
/** A DuckDB logical type as the engine reads it (engine/duckdb/src/api.rs): its id, its text, and what the id says more of. */
export type DuckdbType = { id: string; text: string; alias?: string; width?: number; scale?: number; child?: DuckdbType; size?: number; fields?: { name: string; type: DuckdbType }[]; key?: DuckdbType; value?: DuckdbType; values?: string[] };
/** What DuckDB's prepared statement reports: its statement type, each parameter's type (null where DuckDB cannot tell it), each result column's name and type. */
export type DuckdbDescribed = { kind: "duckdb"; statement: string; params: { name: string; type: DuckdbType | null }[]; columns: { name: string; type: DuckdbType }[] };
export type DbError = { message: string; code?: string; position?: number };
export type Answer =
  | { describe: { params: number[]; columns: Described["columns"] } }
  | { mysql: { columns: MysqlDescribed["columns"]; params: MysqlDescribed["params"] } }
  | { describe: { statement: string; params: DuckdbDescribed["params"]; columns: DuckdbDescribed["columns"] } }
  | { rows: unknown[][] }
  | { error: DbError };

/** What the host does for the engine: describe a statement, run a catalog query. */
export interface EngineIo {
  /** Postgres: Parse/Describe. MySQL: a server prepare. DuckDB: a prepared statement. SQLite needs none: the engine prepares inside its own SQLite. */
  describe(sql: string): Promise<Described | MysqlDescribed | DuckdbDescribed>;
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

/** An engine's request function: one JSON request in, its answer out (a WebAssembly build's `call` export, or the native engine's one-shot form). */
export type EngineCall = (request: string) => string;


/** A compiled core module of a WebAssembly build of the engine, opaque here: the WebAssembly namespace is not typed through the DOM lib. */
export type CoreModule = unknown;

/** The exports of a WebAssembly build of the engine (engine/wasip2/src/lib.rs), which the docs site's playground runs. */
export interface ComponentExports {
  /** The core's answer to a request that needs no database: `ping`, `lex`, `config`, and `analyze` with the host's answers. */
  call(request: string): string;
  /** The answer to a request on a connection the engine opens itself: `open`, `run`, `query`, `describe`, `forget`, `close`. */
  serve(request: string): string;
}

/** The transpiled component's `instantiate`, as `jco transpile --instantiation sync` writes it. */
export type Instantiate = (getCoreModule: (name: string) => CoreModule, imports: object) => ComponentExports;

/**
 * An engine's core: its requests that need no database, and the analysis of a statement whose
 * needs a host answers itself (`analyze` with an `EngineIo`), as a host that holds its own
 * connection does (the docs site's playground, on PGlite). A Node host lets the engine connect
 * itself instead (src/engine.ts).
 */
export class EngineCore {
  #call: EngineCall;

  constructor(call: EngineCall) {
    this.#call = call;
  }

  call(request: object): Response {
    return JSON.parse(this.#call(JSON.stringify(request)));
  }

  /** A request whose answer may wait: each round of `analyze`. Here the call itself; a host whose engine answers over a channel sends it there. */
  protected async ask(request: object): Promise<Response> {
    return this.call(request);
  }

  lex(sql: string, dialect: Dialect): Lexed {
    const r = this.call({ op: "lex", sql, dialect });
    if (r.status !== "lexed") throw new Error(r.status === "error" ? r.message : `unexpected ${r.status}`);
    return r.lexed;
  }

  /** Analyze one statement, fulfilling the engine's needs through `io` until it answers. */
  async analyze(sql: string, dialect: Dialect, io: EngineIo, cache = new AnswerCache()): Promise<Analysis> {
    const answers: Record<string, Answer> = {};
    for (let round = 0; round < 32; round++) {
      const r = await this.ask({ op: "analyze", sql, dialect, answers });
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
      if ("kind" in d && d.kind === "duckdb") return { describe: { statement: d.statement, params: d.params, columns: d.columns } };
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
