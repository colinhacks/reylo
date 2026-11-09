// Runtime execution over a caller-supplied client, typed structurally so that
// reylo has no dependency (not even a type-only one) on any driver package.
// Each type is the smallest slice of the real driver that execution needs, and
// the query's dialect selects which clients are accepted at compile time. A
// transaction, savepoint, reserved or checked-out connection has the same shape
// as the client it came from, so it is accepted wherever that client is.

import { bunRows, neonCallable, postgresJs, queryPostgres, slonik } from "./decode.ts";
import { mariadb, mysql2, planetscale } from "./decode-mysql.ts";
import { libsqlRows, normalize, runDurableObject, runSyncSqlite, runTurso } from "./decode-sqlite.ts";
import { encodeDuckdb, isDuckdb, runDuckdb, type DuckdbClient } from "./decode-duckdb.ts";
import { columnKind, describe, given, hstoreText, isArrayValued, isMultirangeKind, isRangeKind, isRowKind, numbersInJson, ParameterError, postgresInterval, postgresLiteral, postgresYear, rowMasks, withinDimensions, zoned, type Columns, type JsonNumbers } from "./values.ts";
import { bareName, schemaOf, type EngineType } from "./engine-type.ts";
import { lexPlaceholders } from "./lex.ts";
import { withSqlcommenter } from "./sqlcommenter.ts";
import type { JsonSchema } from "./engine-type.ts";
import type { Checks } from "./checks.ts";
import type { Members, Structured } from "./rows.ts";
import type { RowBuilder } from "./row.ts";
import { compared, due, passed, staleError, warnDrifted, type Fresh } from "./fresh.ts";
import { decodeCodecs, encodePlan, type Plan } from "./codecs.ts";

export type Dialect = "postgres" | "mysql" | "sqlite" | "duckdb";

/**
 * pg's Pool, Client and PoolClient; PGlite and its transactions; Neon's Pool and Client;
 * pg-promise's database, tasks and transactions; @vercel/postgres: query(text, values) → { rows }
 * (pg-promise resolves to the rows themselves, and its types say any).
 */
export type PgQueryClient = {
  query(text: string, values?: any[]): PromiseLike<{ rows: unknown[] }>;
  /** mariadb's query() resolves to any as well; its escapeId() tells it apart. */
  escapeId?: never;
};
/**
 * Neon's neon() function from 1.0: query(text, values) → rows. Its transaction() tells it from
 * mysql2/promise, whose query(text, values) resolves to a [rows, fields] array as well.
 */
export type NeonHttpClient = {
  query(text: string, values?: any[]): PromiseLike<Record<string, unknown>[]>;
  transaction(...args: any[]): unknown;
};
/** postgres.js `sql`, its begin and savepoint transactions and reserved connections; Bun.sql, likewise: unsafe(text, params) → rows. */
export type UnsafeClient = {
  unsafe(text: string, params?: any[]): PromiseLike<unknown[]>;
};
/** Bun.sql on MySQL or SQLite; commitDistributed() tells it from postgres.js, which has unsafe() too. */
export type BunSqlClient = UnsafeClient & { commitDistributed(name: string): Promise<unknown> };
/** Neon's neon() function before 1.0, which is called with (text, params). */
export type CallableClient = (text: string, params?: any[]) => PromiseLike<unknown[] | { rows: unknown[] }>;
/** Slonik's pool, connections, transactions and savepoints, whose query() takes only its sql tag's queries. */
export type SlonikClient = { query(sql: any): PromiseLike<unknown>; maybeOneFirst(...args: any[]): unknown; anyFirst(...args: any[]): unknown };
/**
 * mysql2/promise Pool, Connection and PoolConnection (→ [rows, fields]); PlanetScale's Connection,
 * Client and transaction from 1.x (→ { rows }). PlanetScale 2.0 binds no parameters, so its
 * execute(query, options) is not one of these.
 */
export type MysqlExecuteClient = {
  execute(text: string, values?: any[]): PromiseLike<[unknown, unknown] | { rows: unknown[]; statement: string }>;
};
/** mysql2's callback Pool, Connection and PoolConnection, run through their promise() wrapper. */
export type Mysql2CallbackClient = { promise(): MysqlExecuteClient };
/** mariadb's Pool, Connection and PoolConnection: query(text, values) → rows, or an OK packet for a write. */
export type MariadbClient = {
  query(text: string, values?: any): Promise<unknown>;
  batch(text: string, values?: any): Promise<unknown>;
};
/** The same from mariadb's callback API (`mariadb/callback`): query(text, values, callback). */
export type MariadbCallbackClient = {
  query(text: string, values: any, callback: (error: Error | null, rows?: any) => void): void;
  batch(text: string, values: any, callback: (error: Error | null, result?: any) => void): void;
};
/**
 * better-sqlite3, node:sqlite's DatabaseSync, bun:sqlite's Database and the Database of Turso's
 * `@tursodatabase/database/compat`, which answer at once: `runSync` takes them and returns the
 * result itself, so it works inside their `db.transaction(fn)`, whose function must not await.
 */
export type SyncSqliteClient = {
  prepare(sql: string): { all(...params: any[]): unknown[] };
};
/**
 * A SQLite-backed Durable Object's SQL storage, `ctx.storage.sql`: exec(sql, ...values) answers at
 * once with a cursor, so `runSync` takes it, inside `ctx.storage.transactionSync(fn)` too.
 */
export type DurableObjectSql = {
  exec(query: string, ...bindings: any[]): { toArray(): unknown[] };
  readonly databaseSize: number;
};
/** A Durable Object's storage, `ctx.storage`, which `drizzle-orm/durable-sqlite` keeps: `run` takes its `sql`. */
export type DurableObjectStorage = { readonly sql: DurableObjectSql };
/** Cloudflare D1's database and sessions: prepare(sql).bind(...values).all() → { results }. */
export type D1Client = {
  prepare(sql: string): { bind(...values: any[]): { all(): Promise<{ results: unknown[] }> } };
};
/** @libsql/client's Client and Transaction: execute({ sql, args }) → { rows }. */
export type LibsqlClient = {
  execute(statement: { sql: string; args: any[] }): Promise<{ rows: unknown[] }>;
  /** mariadb's execute() takes an object with sql and resolves to any; executeMultiple() tells libSQL apart. */
  executeMultiple(sql: string): Promise<unknown>;
};
/**
 * Turso's new engine, `@tursodatabase/database`: the Database `connect()` resolves to and the
 * Transaction `transactionAsync()` hands its callback, whose statements answer through a promise:
 * prepare(sql).all(...values) → rows. Its types say prepare() returns a promise of the statement
 * from 0.6, and the statement itself before; it returns the statement itself in every release.
 */
export type TursoClient = {
  prepare(sql: string): TursoStatement | PromiseLike<TursoStatement>;
  /** postgres.js's transaction has a prepare(name) of any type too; exec() tells Turso's apart. */
  exec(sql: string): Promise<unknown>;
};
type TursoStatement = { all(...params: any[]): Promise<unknown[]> };
/** The sqlite3 package's callback Database: all(sql, params, callback). */
export type Sqlite3Client = {
  all(sql: string, params: any[], callback: (error: Error | null, rows: any[]) => void): unknown;
};
/** The sqlite package's promise Database: all(sql, params) → rows. */
export type SqlitePromiseClient = {
  all(sql: string, params?: any): Promise<unknown[]>;
  getDatabaseInstance(): unknown;
};

/**
 * A Drizzle database over one of these clients (`drizzle(pool)`, whose `$client` is the pool):
 * `run` runs on the driver client underneath, so the values are what Reylo decodes on that driver.
 */
export type DrizzleDatabase<C> = { readonly $client: C };
/**
 * A transaction or savepoint a Drizzle database hands out (`db.transaction(async (tx) => …)`),
 * which `run` runs on through the transaction's own connection. Each dialect's is known by what
 * only it has: `setTransaction()` on Postgres, `get()` and `all()` on SQLite, neither on MySQL.
 */
export type DrizzlePgTransaction = { rollback(): never; setTransaction(config: any): Promise<void> };
export type DrizzleMysqlTransaction = { rollback(): never; execute(query: any): unknown; setTransaction?: never; get?: never };
export type DrizzleSqliteTransaction = { rollback(): never; get(query: any): unknown; all(query: any): unknown };
/** A transaction of a synchronous SQLite driver's Drizzle database, whose `all()` returns the rows themselves. */
export type DrizzleSyncSqliteTransaction = { rollback(): never; get(query: any): unknown; all(query: any): unknown[] };
/** Every Postgres client `run` takes, or a transaction, savepoint or connection taken from one. */
export type PostgresClient = PgQueryClient | NeonHttpClient | UnsafeClient | CallableClient | SlonikClient | DrizzleDatabase<PgQueryClient | NeonHttpClient | UnsafeClient | CallableClient> | DrizzlePgTransaction;
/** Every MySQL client `run` takes, or a transaction or connection taken from one. */
export type MysqlClient = MysqlExecuteClient | Mysql2CallbackClient | MariadbClient | MariadbCallbackClient | BunSqlClient | DrizzleDatabase<MysqlExecuteClient | Mysql2CallbackClient | MariadbClient | BunSqlClient> | DrizzleMysqlTransaction;
/** Every SQLite client `run` takes, or a transaction taken from one. */
export type SqliteClient = SyncSqliteClient | DurableObjectSql | D1Client | LibsqlClient | TursoClient | Sqlite3Client | SqlitePromiseClient | BunSqlClient | DrizzleDatabase<SyncSqliteClient | DurableObjectStorage | D1Client | LibsqlClient | BunSqlClient> | DrizzleSqliteTransaction;

export type { DuckdbClient } from "./decode-duckdb.ts";

// Each dialect's clients are one named union, so a hover over `run` reads
// `run(client: PostgresClient, …)` rather than listing every client's shape.
export type ClientFor<D extends Dialect> = D extends "postgres" ? PostgresClient : D extends "mysql" ? MysqlClient : D extends "duckdb" ? DuckdbClient : SqliteClient;

/** The clients `runSync` takes: the synchronous SQLite drivers, and none for Postgres or MySQL. */
export type SyncClientFor<D extends Dialect> = D extends "sqlite" ? SyncSqliteClient | DurableObjectSql | DrizzleDatabase<SyncSqliteClient | DurableObjectStorage> | DrizzleSyncSqliteTransaction : never;

export type QueryModule<D extends Dialect = Dialect> = {
  readonly dialect: D;
  /** The statement as written, with Reylo's placeholders (`:id`) left in place: the query's `raw`. */
  readonly sql: string;
  /** Parameter names in placeholder order, so an object of params binds positionally. */
  readonly params: readonly string[];
  /** The statement as the engine compiled it for the database, when a build step ran: the query's `sql`. A query without it is compiled by `compile` below. */
  readonly text?: string | undefined;
  /** The binding order for `text`: one name per placeholder of the rewritten statement. */
  readonly order?: readonly string[] | undefined;
  /** What the analysis found of each parameter, by name (every build step puts it in the module). */
  readonly inputs?: Inputs | undefined;
  /** Each output column's name and kind (src/values.ts), in order: what the run time gives each value (every build step puts it in the module). */
  readonly columns?: Columns | undefined;
  /** Where each Postgres JSON column, by position beside `columns`, holds a float or a decimal that may be NaN or an infinity, which the run time gives as that number (src/values.ts, `numbersInJson`); `null` for a column that holds none. */
  readonly nonFinite?: readonly (JsonNumbers | null)[] | undefined;
  /** Each row as an object of the columns' names, one shape for every row (src/row.ts): every build step puts it in the module, beside `columns`. */
  readonly row?: RowBuilder | undefined;
  /** The result key: output columns, by name, that identify each row of the result, where the analysis proves one. */
  readonly key?: readonly string[] | undefined;
  /** Whether the statement's ORDER BY orders its rows totally, where the analysis proves it. */
  readonly ordered?: boolean | undefined;
  /** How many rows the statement returns, which a query's Standard Query says beside its `decode`. */
  readonly cardinality?: "many" | "one" | "maybeOne" | "none" | undefined;
  /** The columns that hold a family the project file's `js.codecs` gives another type, by name, each with where in its value the codecs stand (src/codecs.ts). */
  readonly codecs?: Readonly<Record<string, Plan>> | undefined;
  /** Each structured parameter in insert position, with the lists of `text` the run time writes again for the keys a call carries. */
  readonly structured?: readonly Structured[] | undefined;
  /** The catalog facts the statement relied on, checked before it runs (src/fresh.ts). */
  readonly fresh?: Fresh | undefined;
  /** The `.sql` file the module was built from, relative to the project's root, for messages. */
  readonly source?: string | undefined;
  /** Whether a Postgres array column may hold nested arrays, as `allow_nested_arrays: true` says: none is refused. */
  readonly nestedArrays?: boolean | undefined;
  /** Whether the statement Reylo sends carries a sqlcommenter comment naming `source` (src/sqlcommenter.ts); `text`, the query's `sql`, leaves it out. */
  readonly sqlcommenter?: boolean | undefined;
  /** The checks of a typed JSON, structured or cast parameter (src/checks.ts), which a build step passes where the statement has one. */
  readonly checks?: Checks | undefined;
};

/**
 * Each parameter by name: `type` is the engine's type for it. The run time reads it where a
 * driver binds a value of one type wrong (an array to a SQL array on postgres.js and Bun.sql, to
 * JSON on pg); a query without it, defined by hand or built before 2026-10-12, binds every value
 * as it is. A typed JSON parameter (wiki/typed-json-params.md) carries its schema beside it, and
 * a parameter cast with `::` on MySQL or SQLite the type as written (src/rows.ts, `checkCast`).
 * `nullable` is false where the statement takes no null for the parameter, which the run time
 * then refuses, given or left out, except that a JSON parameter's null there is JSON's null; a
 * query defined by hand without it binds what it is given.
 */
export type Inputs = Readonly<Record<string, { readonly type: EngineType; readonly schema?: JsonSchema; readonly cast?: string; readonly members?: Members; readonly delimiter?: string; readonly nullable?: boolean; readonly codec?: Plan }>>;

type AnyClient = Record<string, any> & ((...args: any[]) => any);

/**
 * The parameters: an object keyed by name, or by number for positional placeholders (`$1` and
 * `?` give `{ 1: ..., 2: ... }`, and `params` is `["1", "2"]`). An array is refused, since
 * 2026-10-20 (wiki/design.md §9).
 */
export type ParamsInput = Record<string, unknown>;

/**
 * Runs the statement and returns its rows (an empty array for a statement that returns none).
 * On a synchronous SQLite client the rows come back at once and an error is thrown; on every
 * other client they come back as a promise, which rejects with the driver's own error.
 */
export function execute<D extends Dialect>(
  client: ClientFor<D>,
  query: QueryModule<D>,
  params: ParamsInput = {},
): unknown[] | Promise<unknown[]> {
  const c = underneath(client as AnyClient);
  if (query.dialect === "sqlite" && (isSyncSqlite(c) || isDurableObjectSql(c))) return withCodecs(query, executeSyncSqlite(c, query, params));
  const nonFinite = query.dialect === "postgres" && query.nonFinite !== undefined && query.columns !== undefined ? rowMasks(query.columns, query.nonFinite) : [];
  // A Postgres array column holds the dimensions its type declares and no more (src/values.ts, `withinDimensions`), unless the project lets it nest.
  const arrays = query.dialect === "postgres" && !query.nestedArrays ? (query.columns ?? []).filter(([, kind]) => kind.endsWith("[]")) : [];
  if (!nonFinite.length && !arrays.length) return withCodecs(query, executeAsync(c, query, params));
  return withCodecs(query, executeAsync(c, query, params).then((rows) => {
    for (const row of rows as Record<string, unknown>[]) {
      // Postgres writes a non-finite float or decimal inside JSON as text, whichever driver reads it.
      for (const [name, at] of nonFinite) row[name] = numbersInJson(row[name], at);
      for (const [name, kind] of arrays) withinDimensions(row[name], kind, name);
    }
    return rows;
  }));
}

/** The rows with the codecs of the module's columns applied where they stand (src/codecs.ts): one pass, where a column holds one. */
function withCodecs(query: QueryModule, rows: unknown[] | Promise<unknown[]>): unknown[] | Promise<unknown[]> {
  const codecs = query.codecs;
  if (codecs === undefined || Object.keys(codecs).length === 0) return rows;
  return Array.isArray(rows) ? decodeCodecs(rows, codecs) : rows.then((r) => decodeCodecs(r, codecs));
}

/**
 * The driver client a Drizzle database, transaction or savepoint runs on, which its session
 * keeps on `session.client` (the pool for a database, the transaction's own connection inside
 * one), or on `session.tx` inside a libSQL transaction, whose `client` stays the base client;
 * any other client as it is.
 */
function underneath(c: AnyClient): AnyClient {
  const o = c as unknown as { select?: unknown; session?: { client?: unknown; tx?: unknown; dialect?: unknown } } | null | undefined;
  const session = o != null && typeof o === "object" && typeof o.select === "function" ? o.session : undefined;
  if (session == null || typeof session !== "object" || session.dialect == null) return c;
  const client = ((session.tx ?? session.client) as AnyClient | undefined) ?? c;
  // drizzle-orm/durable-sqlite keeps the Durable Object's storage, whose SQL storage is its sql.
  const sql = (client as { sql?: AnyClient }).sql;
  return isDurableObjectSql(sql) ? sql! : client;
}

/** The synchronous path alone, for `runSync`: any other client is refused before anything runs. */
export function executeSync<D extends Dialect>(client: SyncClientFor<D>, query: QueryModule<D>, params: ParamsInput = {}): unknown[] {
  const c = underneath(client as unknown as AnyClient);
  const refuse = (why: string) => new TypeError(`runSync takes a synchronous SQLite client (better-sqlite3, node:sqlite, bun:sqlite, the Database of @tursodatabase/database/compat or a Durable Object's ctx.storage.sql); ${why}. run takes every client.`);
  if (query.dialect !== "sqlite") throw refuse(`a ${query.dialect} statement has none`);
  if (!isSyncSqlite(c) && !isDurableObjectSql(c)) throw refuse("this client answers through a promise");
  const rows = executeSyncSqlite(c, query, params);
  if (Array.isArray(rows)) return withCodecs(query, rows) as unknown[];
  rows.catch(() => {});
  throw refuse("this client's statement answered with a promise");
}

function executeSyncSqlite(c: AnyClient, query: QueryModule, params: ParamsInput): unknown[] | Promise<unknown[]> {
  checkParams(query, params);
  if (query.fresh && due(query)) {
    let rows: unknown[] | undefined;
    try {
      rows = isDurableObjectSql(c) ? c.exec(query.fresh.query).toArray() : c.prepare(query.fresh.query).all();
    } catch {}
    const found = rows && compared(query.fresh, rows, query.dialect);
    if (found?.stale.length) throw staleError(query.source, found.stale);
    if (found) warnDrifted(query.source, found.drifted);
    passed(query);
  }
  const { text, values, names } = statement(query, params);
  if (isDurableObjectSql(c)) return runDurableObject(c, text, durableObjectValues(query, names, values), query.columns);
  return runSyncSqlite(c, text, values, query.columns);
}

// A SQLite-backed Durable Object's `ctx.storage.sql`: exec() and databaseSize, and no prepare().
// sqlite3's Database has exec() too, beside all().
function isDurableObjectSql(c: AnyClient | null | undefined): boolean {
  return c != null && typeof c.exec === "function" && typeof c.prepare !== "function" && typeof c.all !== "function" && "databaseSize" in c;
}

// A Durable Object's exec() refuses a BigInt ("Cannot convert a BigInt value to a number").
function durableObjectValues(query: QueryModule, names: readonly string[], values: unknown[]): unknown[] {
  return values.map((v, i) => numberOrDigits(v, query.inputs?.[names[i]!]?.type, "durable-object"));
}

// A SQLite client with prepare() whose statements answer at once. D1 (batch()), sqlite3 and
// sqlite (all() on the database), libSQL (execute()), Bun.sql (unsafe()) and Turso's promise API
// are asynchronous. Turso's compat Database has batch() as D1 does, and answers at once; its
// maxWriteReplicationIndex(), which every Database of Turso's has, tells it from D1's. A DuckDB
// connection has prepare() too, answering through a promise, and runAndReadAll() beside it.
function isSyncSqlite(c: AnyClient | null | undefined): boolean {
  return (
    c != null &&
    typeof c.prepare === "function" &&
    typeof c.runAndReadAll !== "function" &&
    (typeof c.batch !== "function" || typeof c.maxWriteReplicationIndex === "function") &&
    typeof c.all !== "function" &&
    typeof c.execute !== "function" &&
    typeof c.unsafe !== "function" &&
    !isTurso(c)
  );
}

// Turso's promise API (@tursodatabase/database): a Database has connect() and
// maxWriteReplicationIndex() in every release from 0.4 (0.4 has no all(), so it looked synchronous,
// and from 0.6 its all() took it for sqlite3's, whose callback it never calls); the Transaction
// that transactionAsync() hands its callback (0.7 and later) has all() beside batch(), which
// sqlite3's database lacks. The compat Database has no connect().
function isTurso(c: AnyClient): boolean {
  return typeof c.prepare === "function" && ((typeof c.connect === "function" && typeof c.maxWriteReplicationIndex === "function") || (typeof c.all === "function" && typeof c.batch === "function"));
}

// Slonik's pool, connections, transactions and savepoints, which have query() as pg's do but take
// only the queries its sql tag builds (src/decode.ts, `slonik`).
function isSlonik(c: AnyClient): boolean {
  return typeof c.maybeOneFirst === "function" && typeof c.anyFirst === "function";
}

// A value as Slonik's sql tag takes it, which is a string, a number, a boolean, a BigInt or null:
// the rest as pg would write them, binary data as `\x` and its hex, an array as its literal, a
// Date as its instant, and an object as its JSON. An omitted parameter binds NULL, as on pg.
function slonikParam(v: unknown): unknown {
  if (v === undefined || v === null) return null;
  if (v instanceof Uint8Array) return asByteaHex(v);
  if (Array.isArray(v)) return arrayLiteral(v);
  if (v instanceof Date) return v.toISOString();
  return typeof v === "object" ? JSON.stringify(v) : v;
}

async function executeAsync(c: AnyClient | null | undefined, query: QueryModule, params: ParamsInput): Promise<unknown[]> {
  if (c == null || (typeof c !== "object" && typeof c !== "function")) throw unsupported(c, query.dialect);
  // A DuckDB connection has prepare() as SQLite's drivers do, and runs no other database's statement.
  if (query.dialect !== "duckdb" && isDuckdb(c)) throw unsupported(c, query.dialect);
  checkParams(query, params);
  // The catalog facts the statement relied on, read again through the same client.
  if (query.fresh && due(query)) {
    const fresh = query.fresh;
    let rows: unknown[] | undefined;
    try {
      rows = await executeAsync(c, { dialect: query.dialect, sql: fresh.query, params: [], text: fresh.query, order: [] }, {});
    } catch {}
    const found = rows && compared(fresh, rows, query.dialect);
    if (found?.stale.length) throw staleError(query.source, found.stale);
    if (found) warnDrifted(query.source, found.drifted);
    passed(query);
  }
  const { text, values, names } = statement(query, params);
  const type = (i: number) => query.inputs?.[names[i]!]?.type;
  const isJsonParam = (v: unknown, i: number) => isJsonValue(query, names[i]!, v);
  switch (query.dialect) {
    case "postgres": {
      // An array for a SQL array parameter goes as its literal text, which the server reads as
      // the parameter's type, written as pg writes it: Bun.sql binds no JavaScript array
      // (`any($1)` failed with "insufficient data left in message"), postgres.js types an
      // array by its first element ("cannot cast type boolean to boolean[]" for `$1::bool[]`),
      // and the copy of pg inside Neon's Pool and Client threw on a Uint8Array element and
      // left the connection out of step. Each element of a `jsonb[]` is a JSON value.
      const bound = values.map((v, i) => {
        const t = type(i);
        return Array.isArray(v) && t?.kind === "array" && !isJsonParam(v, i) ? arrayLiteral(t.element.type.kind === "json" ? v.map((e) => (e == null ? e : JSON.stringify(e))) : v, query.inputs?.[names[i]!]?.delimiter) : v;
      });
      // A JSON parameter goes to pg's family, PGlite and Neon as its JSON text, and to postgres.js
      // as its JSON text declared `unknown` (below). Bun.sql serializes a value for a JSON
      // parameter itself, so JSON text would be stored as a JSON string, and it types a boolean,
      // a number or an array of either by its JavaScript type ("cannot cast type boolean to
      // jsonb"): it gets an object whose toJSON() returns the value, which it binds as JSON and
      // serializes to exactly its JSON text. So does an unsafe() client without typed().
      const json = () => bound.map((v, i) => (isJsonParam(v, i) ? jsonText(v) : v));
      // Slonik before every other query(): its query() takes only its sql tag's queries.
      if (isSlonik(c)) return await slonik(c, text, json().map(slonikParam), query.columns);
      // query() before unsafe(): Neon's neon() has both, and its unsafe() builds a fragment
      // for a template instead of running anything. Reylo decodes every value itself (decode.ts).
      if (typeof c.query === "function") return await queryPostgres(c, text, json(), query.columns);
      if (typeof c.unsafe === "function") {
        // postgres.js reads a string for a date or time parameter through `new Date(x)` and binds
        // its toISOString(), so a timestamp's wall-clock time moved by the app's offset, and
        // `infinity` or a zone name failed with "Invalid time value". Declared `unknown` (705), it
        // is sent as the text, which the server reads as the parameter's type, as it would an
        // undeclared one. Bun.sql (commitDistributed()) sends the text and has no typed().
        // A JSON parameter goes to postgres.js as its JSON text declared `unknown` too: its own
        // serializer would write a value as JSON, but a client Drizzle wraps has that serializer
        // replaced by one that passes the value through (drizzle-orm/postgres-js), and a
        // transaction shows neither. Bun.sql serializes the value itself.
        const bun = typeof c.commitDistributed === "function";
        const asText = typeof c.typed === "function" && !bun;
        const mapped = bound.map((v, i) => (isJsonParam(v, i) ? (asText ? c.typed(jsonText(v), 705) : { toJSON: () => jsonValue(v) }) : asText && typeof v === "string" && isTime(type(i), query.dialect) ? c.typed(v, 705) : v));
        return bun ? bunRows(await c.unsafe(text, mapped), query.columns, true) : await postgresJs(c, text, mapped, query.columns, query.row);
      }
      if (typeof c === "function") return await neonCallable(c, text, json().map(asByteaHex), query.columns);
      break;
    }
    case "mysql": {
      if (typeof c.unsafe === "function") return bunRows(await c.unsafe(text, values), query.columns);
      // mariadb, checked before execute() because it has one too (returning rows, not a pair).
      // Its callback API (mariadb/callback) answers through the callback and returns a command
      // object; its promise API returns a promise and ignores the callback.
      if (typeof c.batch === "function" && typeof c.query === "function") return await mariadb(c, text, values.map(asBuffer), query.columns);
      // mysql2's callback API: the promise() wrapper runs on the same connection.
      const m = typeof c.promise === "function" ? c.promise() : c;
      if (typeof m.execute === "function") {
        // mysql2 has query() beside execute(); PlanetScale's Connection, Client and transaction do not.
        return typeof m.query === "function" ? await mysql2(m, text, values, query.columns) : await planetscale(m, text, values, query.columns);
      }
      break;
    }
    case "duckdb": {
      // @duckdb/node-api's DuckDBConnection: each value given its kind by the result's own types (decode-duckdb.ts).
      if (isDuckdb(c)) return await runDuckdb(c as unknown as DuckdbClient, text, values, names.map((_, i) => type(i)), query.columns);
      break;
    }
    case "sqlite": {
      // The driver's rows, each value given its kind (decode-sqlite.ts).
      const columns = query.columns;
      if (typeof c.unsafe === "function") return normalize(await c.unsafe(text, values), columns);
      if (typeof c.execute === "function") {
        const result = await c.execute({ sql: text, args: values });
        return normalize(libsqlRows(result.rows, columns, Array.isArray(result.columns) ? result.columns : undefined), columns);
      }
      if (isTurso(c)) return await runTurso(c, text, values, columns);
      // The sqlite package wraps sqlite3, and sqlite3 binds a BigInt as NULL.
      if (typeof c.getDatabaseInstance === "function") return normalize(await c.all(text, values.map((v, i) => numberOrDigits(v, type(i), "sqlite3"))), columns);
      if (typeof c.all === "function" && typeof c.exec === "function") {
        const bound = values.map((v, i) => numberOrDigits(v, type(i), "sqlite3"));
        return normalize(
          await new Promise<unknown[]>((resolve, reject) => c.all(text, bound, (error: Error | null, rows: unknown[]) => (error ? reject(error) : resolve(rows)))),
          columns,
        );
      }
      if (typeof c.prepare === "function") {
        const prepared = c.prepare(text);
        // D1 refuses a BigInt, which `.run()` returns for a BIGINT column.
        if (typeof prepared.first === "function") return normalize((await prepared.bind(...values.map((v, i) => numberOrDigits(v, type(i), "d1"))).all()).results, columns, { d1: true });
        return normalize(await prepared.all(...values), columns);
      }
      break;
    }
  }
  throw unsupported(c, query.dialect);
}

// The parameters are an object, refused before anything reaches the database otherwise: an
// array, which `run` took in the order of `params` from 2026-10-03, was dropped on 2026-10-20
// (wiki/design.md §9), so the call says which key each value binds.
function checkParams(query: QueryModule, input: unknown): asserts input is Record<string, unknown> {
  if (input !== null && typeof input === "object" && !Array.isArray(input)) return;
  const keys = query.params.map((name) => `${/^(?:[A-Za-z_$][\w$]*|\d+)$/.test(name) ? name : JSON.stringify(name)}: ...`);
  const takes = keys.length ? `This statement takes { ${keys.join(", ")} }.` : "This statement takes none: leave them out.";
  throw new ParameterError(`The parameters must be an object keyed by parameter name, or by number for positional placeholders ({ 1: ..., 2: ... }); got ${Array.isArray(input) ? "an array" : String(input)}. ${takes}`);
}

// The rows are bound positionally in every dialect. A value left out binds NULL, as pg,
// PGlite, better-sqlite3 and Bun.sql already do; postgres.js, mysql2, node:sqlite and libSQL
// would reject it instead.
function statement(query: QueryModule, params: ParamsInput) {
  // The build step already compiled the statement; only the values need ordering.
  const compiled = compile(query);
  const bound: { text: string; values: unknown[]; names: readonly string[] } = { text: compiled.text, values: compiled.order.map((name) => value(params, name)), names: compiled.order };
  bound.names.forEach((name, i) => {
    const input = query.inputs?.[name];
    const type = input?.type;
    // A typed JSON parameter (`:p::jsonb('…')`): a value left out takes the schema's default, a
    // key under `properties` too, and the value is checked against the schema before the
    // statement runs (wiki/typed-json-params.md). Null is SQL NULL where the statement takes SQL
    // NULL for the parameter, and elsewhere JSON's null, where the schema admits it; a value left
    // out there is refused.
    const schema = input?.schema;
    if (schema !== undefined) {
      let v = given(params, name);
      const { cloneJson, validate, withDefaults } = checksFor(query, name);
      if (v === undefined && schema !== null && typeof schema === "object" && "default" in schema) v = cloneJson(schema.default);
      if (v !== undefined && v !== null) {
        v = withDefaults(schema, v);
        const failure = validate(schema, v);
        if (failure) throw new ParameterError(`Parameter "${name}"${failure.path ? ` at ${failure.path}` : ""} ${failure.message}.`);
      } else if (input?.nullable === false) {
        if (v === undefined || validate(schema, null)) refuseNull(name, v);
        v = JSON_NULL;
      }
      bound.values[i] = v === undefined ? null : v;
      return;
    }
    // A value of a family the project file's `js.codecs` gives another type takes that type alone,
    // wherever it stands in the parameter, a structured parameter's members among them, and is
    // bound as the text or the bytes its type reads (src/codecs.ts).
    if (input?.codec !== undefined) bound.values[i] = encodePlan(input.codec, bound.values[i], name, query.dialect);
    // A structured parameter is checked against its column list and written as the JSON the
    // statement reads; in an insert, its lists name only the keys the call carries. It refuses
    // null itself, saying what to pass.
    if (type !== undefined && input?.members !== undefined) {
      const structured = query.structured?.find((s) => s.name === name);
      const { prepareRows, rowsOf, variant } = checksFor(query, name);
      const rows = rowsOf(type, input.members);
      const prepared = prepareRows(name, rows, bound.values[i], query.dialect, structured !== undefined);
      bound.values[i] = prepared.value;
      if (structured) bound.text = variant(bound.text, structured, rows.fields, prepared.present);
      return;
    }
    // A parameter the statement takes no null for, which its types rely on (a column it alone
    // fills, an expression of it typed non-null), refuses null and refuses to be left out, which
    // would bind NULL: exactly where its declared type takes no null (src/render.ts,
    // `renderParamType`). One typed `unknown` is declared `unknown` and takes anything. A JSON one
    // takes JSON's null there instead, unless the schema its type carries refuses null, and is
    // refused only when left out.
    let jsonNull = false;
    if (input?.nullable === false && bound.values[i] === null) {
      if (type?.kind === "json" && given(params, name) !== undefined && !refusesNull(query, type)) jsonNull = true;
      else if (type?.kind === "json" || refusesNull(query, type)) refuseNull(name, given(params, name));
    }
    // An array parameter takes no NULL element, as its type says (`:ids::int[]` is `number[]`):
    // a NULL in `= any(:ids)` matches nothing. An array inside a
    // structured parameter's member keeps its NULL elements (src/rows.ts).
    if (type?.kind === "array" && Array.isArray(bound.values[i])) {
      const at = nullElement(bound.values[i] as unknown[]);
      if (at !== undefined) throw new ParameterError(`Parameter "${name}" is an array whose elements may not be null, and the element at ${at} is. Leave it out of the array.`);
    }
    // A `::type` written on MySQL or SQLite, whose cast turns a value it cannot read into 0 or
    // NULL: the value is checked against the type as written.
    const cast = query.inputs?.[name]?.cast;
    if (cast !== undefined && type !== undefined) checksFor(query, name).checkCast(name, cast, type, bound.values[i], query.dialect);
    checkTime(type, name, bound.values[i], query.dialect);
    checkBinary(type, name, bound.values[i], query.dialect);
    bound.values[i] = jsonNull ? JSON_NULL : encode(type, bound.values[i], query.dialect, name);
  });
  // Every MySQL and SQLite driver binds JSON text as text, which the database reads as JSON (a
  // structured parameter's is written already).
  if (query.dialect !== "postgres") bound.values = bound.values.map((v, i) => (isJsonValue(query, bound.names[i]!, v) && query.inputs?.[bound.names[i]!]?.members === undefined ? jsonText(v) : v));
  // The comment naming the file goes on last, after a structured parameter's lists are written.
  bound.text = sentText(query, bound.text);
  return bound;
}

/** `text` as Reylo sends it: with the comment naming the query's file, where the module asks for one (src/sqlcommenter.ts). */
export function sentText(query: QueryModule, text: string): string {
  return query.sqlcommenter && query.source ? withSqlcommenter(text, query.source, query.dialect) : text;
}

/**
 * The statement and its values in the database's raw form, which a Standard Query's `encode`
 * returns (src/standard-query.ts), bound as `.run()` binds them, its checks included: on Postgres
 * each value's text, which a driver binds with no declared type, so the server reads it as the
 * parameter's type; on MySQL and SQLite each value as a driver binds it.
 */
export function rawStatement(query: QueryModule, params: unknown): { text: string; values: unknown[] } {
  checkParams(query, params);
  const { text, values, names } = statement(query, params);
  if (query.dialect !== "postgres") return { text, values };
  return { text, values: values.map((v, i) => postgresText(query, names[i]!, v)) };
}

/** A bound value as the text Postgres's input function reads, as `.run()` sends it through pg. */
function postgresText(query: QueryModule, name: string, v: unknown): string | null {
  if (v == null) return null;
  const t = query.inputs?.[name]?.type;
  if (isJsonValue(query, name, v)) return jsonText(v);
  if (Array.isArray(v) && t?.kind === "array") return arrayLiteral(t.element.type.kind === "json" ? v.map((e) => (e == null ? e : JSON.stringify(e))) : v, query.inputs?.[name]?.delimiter);
  if (Array.isArray(v)) return arrayLiteral(v);
  if (v instanceof Uint8Array) return asByteaHex(v) as string;
  if (typeof v === "boolean") return v ? "t" : "f";
  if (v instanceof Date) return dateText(v);
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

/** The checks a parameter needs (src/checks.ts): every build step passes them where its statement has such a parameter. */
function checksFor(query: QueryModule, name: string): Checks {
  if (query.checks) return query.checks;
  throw new ParameterError(`Parameter "${name}" is a typed JSON, structured or cast parameter, which the run time checks with Reylo's checks; a query defined by hand passes them: defineQuery({ …, checks }) with checks from "reylo".`);
}

/** Where an array, or an array inside it (a multidimensional one), holds null or undefined: `[2]`, `[1][0]`. */
function nullElement(array: readonly unknown[], path = ""): string | undefined {
  for (let i = 0; i < array.length; i++) {
    const e = array[i];
    if (e == null) return `${path}[${i}]`;
    if (Array.isArray(e)) {
      const inner = nullElement(e, `${path}[${i}]`);
      if (inner !== undefined) return inner;
    }
  }
  return undefined;
}

/** Whether a parameter the statement takes no null for is declared with a type that takes no null: for a JSON one, whether the schema its type carries refuses JSON's null too. */
function refusesNull(query: QueryModule, type: EngineType | undefined): boolean {
  if (type === undefined || type.kind === "unknown") return false;
  if (type.kind !== "json") return true;
  const schema = schemaOf(type);
  return schema !== undefined && query.checks !== undefined && query.checks.validate(schema, null) !== undefined;
}

/** A parameter the statement takes no null for, given null or left out. */
function refuseNull(name: string, v: unknown): never {
  throw new ParameterError(`Parameter "${name}" ${v === undefined ? "is missing" : "is null"}, and the statement takes no null for it: pass a value.`);
}

function value(params: Record<string, unknown>, name: string) {
  const v = given(params, name);
  return v === undefined ? null : v;
}

// A binary parameter is typed Uint8Array, and every driver binds one as it is (pg-promise too,
// since it gets { text, values }) but two (lab/driver-matrix, S1c). mariadb writes any value
// that is not a Buffer as JSON, so a plain Uint8Array (TextEncoder's, getRandomValues') arrived
// as the text {"0":1,"1":2,"2":255}: it gets a Buffer over the same memory, where a Buffer
// exists (not in every Worker). Neon's neon() before 0.10 sends a Buffer as JSON as well, so it
// gets Postgres's hex form, `\x0102ff`, which 0.10 and later send for a Buffer themselves.
function asBuffer(v: unknown) {
  // Named structurally, so that a project without Node's types can type-check the module.
  const NodeBuffer = (globalThis as { Buffer?: { isBuffer(v: unknown): boolean; from(buffer: ArrayBufferLike, offset: number, length: number): Uint8Array } }).Buffer;
  return v instanceof Uint8Array && NodeBuffer !== undefined && !NodeBuffer.isBuffer(v) ? NodeBuffer.from(v.buffer, v.byteOffset, v.byteLength) : v;
}


// A JSON parameter's value is a JavaScript value, which reaches the database as its JSON text
// (JSON.stringify), so a string is a JSON string on every driver; null is SQL NULL where the
// statement takes SQL NULL for the parameter, and JSON's null elsewhere (`JSON_NULL`). Each driver
// left to itself did something else (wiki/driver-decoding.md, 2026-10-12): pg wrote an array as
// an array literal and a string as raw JSON text, mariadb expanded an array into a list, the
// SQLite drivers refused an object, and mysql2 bound 5 as 5.0.
function isJsonValue(query: QueryModule, name: string, v: unknown): boolean {
  const input = query.inputs?.[name];
  return v != null && (input?.type.kind === "json" || input?.members !== undefined);
}

/**
 * JSON's null, which a JSON parameter's null stands for where the statement takes no SQL NULL for
 * it, as the engine reports it: a NOT NULL `jsonb` column holds JSON's null and refuses SQL NULL.
 * A `coalesce` or a null test of the parameter takes no SQL NULL either, as for every other type,
 * so `coalesce(:p::jsonb, col)` given null keeps JSON's null rather than `col`; a cast to
 * `nullable(jsonb)` is what passes SQL NULL there. It stays apart from SQL NULL until each driver's
 * binding writes it as the JSON text `null`.
 */
const JSON_NULL: unique symbol = Symbol("JSON null");

/** A JSON parameter's value as JSON: `JSON_NULL` as null. */
const jsonValue = (v: unknown) => (v === JSON_NULL ? null : v);

/** A JSON parameter's value as the JSON text a driver binds. */
const jsonText = (v: unknown) => JSON.stringify(jsonValue(v));

// A date or time value's type says whether it has a zone (src/render.ts),
// and a value that would be read in a zone nobody chose is refused before the
// statement runs. A `timestamp` or a `date` (MySQL's DATETIME and TIMESTAMP are the engine's
// `timestamp`) takes no Date: pg and mysql2 wrote its wall-clock time in the app's zone,
// postgres.js in the server's, PGlite and Bun.sql in UTC, and Bun.sql's `date` failed in every
// zone. A `timestamptz` takes a string that names its zone, which is what `run` returns for one:
// without a zone the server reads the text in the session's TimeZone, which the driver or the
// server sets, and a Date is refused too, as every other type's value is but a string,
// with the call that writes it as one.

const isTime = (type: EngineType | undefined, dialect: Dialect) => type !== undefined && columnKind(type, dialect).split("|").every((part) => part === "timestamp" || part === "timestamptz" || part === "date");

function checkTime(type: EngineType | undefined, name: string, v: unknown, dialect: Dialect): void {
  if (type === undefined || v == null) return;
  if (type.kind === "array") {
    if (Array.isArray(v)) for (const e of v) checkTime(Array.isArray(e) ? type : type.element.type, name, e, dialect);
    return;
  }
  const kind = columnKind(type, dialect);
  if (kind === "timestamptz") {
    if (v instanceof Date) {
      throw new ParameterError(`Parameter "${name}" is a timestamptz, which takes the instant as a string that names its zone, as \`.run()\` returns one, not a Date: pass date.toISOString() ("2026-10-12T19:06:48.282Z").`);
    }
    if (typeof v === "string" && !zoned.test(v.trim())) {
      throw new ParameterError(`Parameter "${name}" is a timestamptz, an instant, so a string for it names its zone ("2026-10-12T19:06:48.282Z", "2026-10-12 19:06:48+09", "2026-10-12 19:06:48 America/Los_Angeles"); got ${JSON.stringify(v)}, which the server would read in the session's time zone.`);
    }
  } else if (v instanceof Date && kind.split("|").every((part) => part === "timestamp" || part === "date")) {
    throw new ParameterError(`Parameter "${name}" is a ${kind === "date" ? "date" : "timestamp"} without a time zone, so it takes the wall-clock text ("2026-10-12 19:06:48.282", "2026-10-12"), not a Date: drivers write a Date's time in different zones (the app's, the server's or UTC).`);
  }
}

// A binary parameter takes a Uint8Array (a Buffer is one), as `.run()` returns binary data and
// as a structured parameter's binary member does (src/rows.ts). Anything else would reach the
// driver as it is, and a string means something different on each database: Postgres reads it
// as bytea's hex or escape text, MySQL stores the bytes of its UTF-8, and SQLite stores it as
// TEXT, which comes back a string. So base64 text was stored as its own letters.
function checkBinary(type: EngineType | undefined, name: string, v: unknown, dialect: Dialect): void {
  if (type === undefined || v == null) return;
  if (type.kind === "array" && columnKind(type, dialect).endsWith("[]")) {
    if (Array.isArray(v)) for (const e of v) checkBinary(Array.isArray(e) ? type : type.element.type, name, e, dialect);
    return;
  }
  if (columnKind(type, dialect) !== "bytes" || v instanceof Uint8Array) return;
  throw new ParameterError(`Parameter "${name}" is binary data, so it takes a Uint8Array (a Buffer is one), as \`.run()\` returns one; got ${describe(v)}. ${v instanceof ArrayBuffer ? "Wrap an ArrayBuffer in one: new Uint8Array(buffer)." : "Decode base64 or hex text into one first."}`);
}

// What `run` returns goes back in as the database reads it (the round-trip rule, src/render.ts):
// on Postgres a date or time with an extended or BC year in Postgres's own form, which refuses
// `+010000` and `0000`, and a negative interval with each part signed (`-P1D` as `P-1D`), since
// Postgres refuses a sign before the `P`; on SQLite a truth value, which a BOOLEAN column returns, as 1 or 0, since
// better-sqlite3 refuses a boolean ("SQLite3 can only bind numbers, strings, bigints, buffers, and
// null") and libSQL binds `true` as the REAL 1.0. A query without `inputs` gets neither. On MySQL
// a VECTOR, MySQL's one array, as the text of its floats, `[1,2.5]`, which the engine's
// `to_vector(?)` reads: drivers left to themselves sent an array as the text "1,2,3" (mysql2's
// execute()) or expanded it into a list of values (mysql2's query(), mariadb). On Postgres a
// pgvector vector the same way, as the text its input reads, and an array of vectors as an array
// of those texts.
function encode(type: EngineType | undefined, v: unknown, dialect: Dialect, name: string): unknown {
  if (type === undefined || v == null) return v;
  if (dialect === "duckdb") return encodeDuckdb(type, v, name);
  if (dialect === "sqlite") return typeof v === "boolean" ? (v ? 1 : 0) : v;
  if (dialect === "mysql") {
    // A SET goes as its members joined by commas, in the order its column declares them: MySQL
    // stores a SET in that order, and compares one with a parameter's text as it is written.
    if (type.kind === "set" && v instanceof Set) {
      const labels = type.element.type.kind === "text" ? type.element.type.enum : undefined;
      return (labels === undefined ? [...v] : [...v].sort((a, b) => labels.indexOf(a as string) - labels.indexOf(b as string))).join(",");
    }
    return type.kind === "array" && bareName(type) === "vector" && Array.isArray(v) ? vectorText(v, name, "a VECTOR") : v;
  }
  const kind = columnKind(type, dialect);
  // A row value, a geometric type and a range go as the text Postgres reads, from the object
  // `.run()` returns for them, and an array of them as an array of those texts (below).
  if (!kind.endsWith("[]") && (isRowKind(kind) || GEOMETRIC_KINDS.has(kind) || isRangeKind(kind) || isMultirangeKind(kind))) return postgresLiteral(type, v, (message) => {
    throw new ParameterError(`Parameter "${name}" is ${message}`);
  });
  if (type.kind === "array" && kind.endsWith("[]")) return Array.isArray(v) ? v.map((e) => encode(Array.isArray(e) && !isArrayValued(columnKind(type.element.type, dialect)) ? type : type.element.type, e, dialect, name)) : v;
  if (kind === "vector" && Array.isArray(v)) {
    if (!v.length) throw new ParameterError(`Parameter "${name}" is a pgvector ${bareName(type)}, which has one dimension at least; got an empty array.`);
    return vectorText(v, name, `a pgvector ${bareName(type)}`);
  }
  if (kind === "hstore" && typeof v === "object") return hstoreText(v, (message) => {
    throw new ParameterError(`Parameter "${name}" ${message}`);
  });
  if (typeof v !== "string") return v;
  if (isTime(type, dialect)) return postgresYear(v);
  return kind === "interval" ? postgresInterval(v) : v;
}

/** Postgres's geometric types' kinds (src/values.ts). */
const GEOMETRIC_KINDS: ReadonlySet<string> = new Set(["point", "line", "lseg", "box", "path", "polygon", "circle"]);

/** A vector's floats as the text a VECTOR's `to_vector` and pgvector's input read; JSON's, which would write NaN and ±Infinity as null, so those are refused. */
function vectorText(v: readonly unknown[], name: string, what: string): string {
  v.forEach((e, i) => {
    if (typeof e !== "number" || !Number.isFinite(e)) {
      throw new ParameterError(`Parameter "${name}" is ${what}, an array of finite numbers, and the element at [${i}] is ${typeof e === "string" ? JSON.stringify(e) : typeof e === "number" ? e : typeof e}.`);
    }
  });
  return JSON.stringify(v);
}

// sqlite3 6.0.1 binds a BigInt as NULL, with no error (`select cast(? as text)` with
// 9007199254740993n gave null), and a Durable Object's exec() refuses one, where every other
// SQLite driver binds it exactly. A BigInt a number holds exactly goes as that number. A larger
// one goes as its digits to a parameter of a BIGINT column (the engine's int8), whose INTEGER
// affinity reads the text as that integer in an insert and a comparison alike, and is refused
// anywhere else, where the text would stay text.
function numberOrDigits(v: unknown, type: EngineType | undefined, driver: "sqlite3" | "durable-object" | "d1"): unknown {
  if (typeof v !== "bigint") return v;
  if (v >= BigInt(Number.MIN_SAFE_INTEGER) && v <= BigInt(Number.MAX_SAFE_INTEGER)) return Number(v);
  if (type !== undefined && columnKind(type, "sqlite") === "int8") return String(v);
  const digits = "pass it as a string of digits, which SQLite stores as that integer in an INTEGER column";
  if (driver === "durable-object") throw new ParameterError(`A Durable Object's SQL storage binds no BigInt, and ${v} is past 2^53 - 1, where a number stops being exact: ${digits}.`);
  if (driver === "d1") throw new ParameterError(`Cloudflare D1 binds no BigInt, and ${v} is past 2^53 - 1, where a number stops being exact: ${digits}.`);
  throw new ParameterError(`sqlite3 binds a BigInt as NULL, and ${v} is past 2^53 - 1, where a number stops being exact: ${digits}, or run the statement on better-sqlite3, node:sqlite or libSQL, which bind a BigInt.`);
}

function asByteaHex(v: unknown) {
  if (!(v instanceof Uint8Array)) return v;
  let hex = "\\x";
  for (const byte of v) hex += (byte < 16 ? "0" : "") + byte.toString(16);
  return hex;
}

/**
 * A Postgres array literal, written as pg writes one (`arrayString` in pg/lib/utils.js): a nested
 * array in braces, null and undefined as NULL, binary data as `\\x` and its hex, and every other
 * element quoted, with `"` and `\` escaped: a Date as its local time and offset, an object as
 * its JSON, anything else as its string. The elements are separated by `delim`, the element type's
 * delimiter: the server reads `{"a","b"}` as one element of a `geometry[]`, whose delimiter is `:`.
 */
export function arrayLiteral(array: readonly unknown[], delim = ","): string {
  let out = "{";
  for (let i = 0; i < array.length; i++) {
    const v = array[i];
    if (i) out += delim;
    if (v == null) out += "NULL";
    else if (Array.isArray(v)) out += arrayLiteral(v, delim);
    else if (ArrayBuffer.isView(v)) out += "\\" + asByteaHex(new Uint8Array(v.buffer, v.byteOffset, v.byteLength));
    else out += `"${(v instanceof Date ? dateText(v) : typeof v === "object" ? JSON.stringify(v) : String(v)).replace(/[\\"]/g, "\\$&")}"`;
  }
  return out + "}";
}

/** pg's `dateToString`: the local date and time to the millisecond, then the offset; BC for a year before 1. */
function dateText(d: Date): string {
  const pad = (n: number, width = 2) => String(n).padStart(width, "0");
  const year = d.getFullYear();
  const offset = -d.getTimezoneOffset();
  const abs = Math.abs(offset);
  return `${pad(year < 1 ? 1 - year : year, 4)}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}${offset < 0 ? "-" : "+"}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}${year < 1 ? " BC" : ""}`;
}

/**
 * The statement as Reylo sends it to the database, and the parameter each placeholder binds: the
 * engine's compiled text where a build step wrote one into the module. Without it (a query
 * defined by hand) the run time lexes the placeholders itself (`lex.ts`); the engine's lexer is
 * the authoritative one, and the two agree on every statement the fallback is meant for.
 * Postgres numbers each distinct parameter once; MySQL has only `?`, and better-sqlite3 reads
 * SQLite's `?NNN` as a named parameter, so both get one `?` per placeholder and a repeated name is
 * repeated in the binding order.
 */
export function compile(query: QueryModule): { text: string; order: readonly string[] } {
  if (query.text && query.order) return { text: query.text, order: query.order };
  const { placeholders } = lexPlaceholders(query.sql, query.dialect);
  // Postgres and DuckDB number each distinct parameter once.
  const numbered = query.dialect === "postgres" || query.dialect === "duckdb";
  let text = "";
  let last = 0;
  const order: string[] = [];
  for (const p of placeholders) {
    const index = indexOf(query, p);
    text += query.sql.slice(last, p.start) + (numbered ? `$${index + 1}` : "?");
    order.push(p.name);
    last = p.end;
  }
  return { text: text + query.sql.slice(last), order: numbered ? query.params : order };
}

function indexOf(query: QueryModule, p: { name: string; start: number; end: number }) {
  const index = query.params.indexOf(p.name);
  // A hand-built query whose `params` misses a placeholder would bind `$0`.
  if (index === -1) throw new Error(`The statement's placeholder ${query.sql.slice(p.start, p.end)} is parameter "${p.name}", which the query's params (${JSON.stringify(query.params)}) do not list.`);
  return index;
}

const accepted: Record<Dialect, string> = {
  postgres: "query(text, values) as on pg, PGlite, Neon and pg-promise; unsafe(text, values) as on postgres.js and Bun.sql; Neon's neon() function; a Slonik pool, connection or transaction; or a Drizzle database over one of them",
  mysql: "execute(text, values) as on mysql2 and PlanetScale; query(text, values) as on mariadb's pool or connection, from its promise or callback API; unsafe(text, values) as on Bun.sql; a mysql2 callback pool or connection; or a Drizzle database over one of them",
  sqlite: "prepare(sql) as on better-sqlite3, node:sqlite, bun:sqlite, Turso's @tursodatabase/database and Cloudflare D1; exec(sql, ...values) as on a Durable Object's ctx.storage.sql; execute({ sql, args }) as on @libsql/client; all(sql, params) as on sqlite3 and sqlite; unsafe(text, values) as on Bun.sql; or a Drizzle database over one of them",
  duckdb: "runAndReadAll(sql, values) as on @duckdb/node-api's DuckDBConnection",
};

function unsupported(c: unknown, dialect: Dialect): TypeError {
  let seen: string;
  if (c === null || (typeof c !== "object" && typeof c !== "function")) seen = String(c);
  else {
    const names = new Set<string>();
    for (let o: object | null = c; o && o !== Object.prototype && o !== Function.prototype; o = Object.getPrototypeOf(o)) {
      for (const key of Object.getOwnPropertyNames(o)) {
        const d = Object.getOwnPropertyDescriptor(o, key);
        if (key !== "constructor" && typeof d?.value === "function") names.add(key);
      }
    }
    seen = names.size ? `an object with ${[...names].slice(0, 12).join(", ")}${names.size > 12 ? ", ..." : ""}` : "an object with no methods";
  }
  // node:sqlite's tag store keeps its database on db. Kysely and Prisma decode every value of a
  // raw query themselves, by their own rules or the driver's parsers, before it returns.
  // Neither hands out a transaction's connection.
  const o = c as { db?: { prepare?: unknown }; selectFrom?: unknown; executeQuery?: unknown; isTransaction?: unknown; $queryRawUnsafe?: unknown; $transaction?: unknown } | null;
  const inTx = " and does not hand out its transaction's connection: take the transaction from the driver's client instead.";
  const hint =
    dialect !== "duckdb" && isDuckdb(c as AnyClient)
      ? ` It is a DuckDB connection, and the statement is typed for ${dialect}: set the project's URL to the DuckDB database (duckdb:./app.duckdb).`
      : typeof o?.db?.prepare === "function"
      ? " A tag store is not a client: pass its database, store.db."
      : typeof o?.selectFrom === "function" && typeof o.executeQuery === "function"
        ? ` Kysely decodes the values of its queries itself${o.isTransaction === true ? inTx : ": pass the pool or database its dialect was made with."}`
        : typeof o?.$queryRawUnsafe === "function"
          ? ` Prisma decodes the values of its queries itself${typeof o.$transaction !== "function" ? inTx : ": pass a client of your database's driver, such as the pool its driver adapter was made with."}`
          : "";
  return new TypeError(`reylo cannot run a ${dialect} statement on ${seen}. It takes a client with ${accepted[dialect]}.${hint}`);
}
