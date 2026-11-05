// `reylo/rtd`: Reylo Type Defs (RTD), experimental. `analyze` returns one statement's descriptor
// (spec/index.ts, `StatementDescriptor`), read from a live database by the engine, as
// plain data that serializes to JSON; `reylo analyze` prints the same descriptors on the command
// line (engine/cli/src/analyze.rs). It reads no project file and no `.reylo/` cache: the URL names
// the database, every call reads it afresh, and a parameter nothing types is an error, as under the
// project file's default, `strict_params: true`.
import type { StatementDescriptor } from "../spec/index.ts";
import { NotMysql, reason } from "./connection.ts";
import { duckdbDatabase } from "./duckdb.ts";
import { AnswerCache, engine, Unreachable, type Analysis, type Connection, type Engine } from "./engine.ts";
import { placeholdersOf, untypedParams } from "./module-types.ts";
import { dialectOf, duckdbPath, sqlitePath, type Dialect as Database } from "./project.ts";

export type * from "../spec/index.ts";

/** The format version `analyze` writes, as the URL of its JSON Schema (spec/typedefs.schema.json), which the type holds to the spec's. */
const SCHEMA: StatementDescriptor["$schema"] = "https://reylo.dev/typedefs/0.1.0.json";

/** What `analyze` reads: the database, and the statement. */
export interface AnalyzeInput {
  /** The database: `postgres://` or `postgresql://`, `mysql://`, a SQLite file as `file:` or `sqlite:`, or a DuckDB file as `duckdb:`. A relative file path is read from the working directory. */
  url: string;
  /** One statement, with `:name` placeholders on every database. */
  sql: string;
}

/**
 * The statement's type descriptor in Reylo Type Defs: its parameters and columns with their types
 * and nullability, its cardinality, its kind, and `sql` with the database's own placeholders. A
 * statement the database or the engine refuses comes back with its diagnostics, each at its place,
 * and no columns. It rejects only where it cannot analyze at all: a URL of no database it reads, no
 * engine to run, or a database it cannot reach.
 *
 * Calls with the same URL share one connection, which closes a moment after the last. Each reads
 * the catalog as it stands when it is made, so a migration run between two calls shows in the
 * second.
 *
 * @experimental Reylo Type Defs is at version 0.1.0, and until 1.0.0 a new version may change
 * anything in it. Each descriptor names its version in `$schema`.
 */
export async function analyze(input: AnalyzeInput): Promise<StatementDescriptor> {
  const { url, sql } = input;
  if (typeof url !== "string" || typeof sql !== "string") throw new TypeError("analyze takes { url, sql }, both strings");
  const dialect = dialectOf(url);
  if (!dialect) throw new Error("reylo/rtd cannot tell the database from the URL: it reads postgres://, postgresql://, mysql://, file:, sqlite: and duckdb: URLs.");
  if (dialect === "duckdb") return analyzeDuckdb(url, sql);
  const target = dialect === "sqlite" ? sqlitePath(url, process.cwd()) : url;
  const key = `${dialect}\0${target}`;
  const entry = acquire(key, dialect, target);
  try {
    let c: Connection;
    try {
      c = await entry.connection;
    } catch (e) {
      // Not kept: the next call tries to open it again.
      drop(key, entry);
      throw failure(e);
    }
    try {
      // The answers the engine keeps for the connection are about the catalog as it was: they are
      // dropped before each call, so that calls made together share them and a later call reads a
      // schema that changed since.
      await c.forget();
      const analysis = await c.analyze(sql);
      return descriptor(sql, dialect, c.version, analysis, await untyped(await engine(dialect), sql, dialect, analysis));
    } catch (e) {
      if (e instanceof Unreachable || e instanceof NotMysql) {
        drop(key, entry);
        throw failure(e);
      }
      // This statement alone could not be analyzed: one diagnostic over the whole of it says why,
      // as `reylo analyze` prints it.
      return refused(sql, dialect, c.version, (e as Error).message);
    }
  } finally {
    release(key, entry);
  }
}

/** How long a connection stays open after its last call, so that calls made one after another share it. */
const IDLE = 1_000;

type Shared = { connection: Promise<Connection>; calls: number; timer?: ReturnType<typeof setTimeout> };

/** The open connections, by database and URL. */
const connections = new Map<string, Shared>();

/** The connection to `target`, opened by the engine of its database where none is open. */
function acquire(key: string, dialect: Exclude<Database, "duckdb">, target: string): Shared {
  let entry = connections.get(key);
  if (!entry) {
    entry = { connection: engine(dialect).then((e) => e.open(target)), calls: 0 };
    connections.set(key, entry);
  }
  clearTimeout(entry.timer);
  entry.calls++;
  return entry;
}

/** A call is done with the connection: the last one closes it after a moment, a timer that does not hold the process open. */
function release(key: string, entry: Shared): void {
  if (--entry.calls > 0 || connections.get(key) !== entry) return;
  entry.timer = setTimeout(() => drop(key, entry), IDLE);
  entry.timer.unref?.();
}

/** Closes a connection and forgets it, so that the next call opens another. */
function drop(key: string, entry: Shared): void {
  clearTimeout(entry.timer);
  if (connections.get(key) === entry) connections.delete(key);
  entry.connection.then((c) => c.close(), () => {});
}

/** DuckDB runs inside the app: the engine's needs are answered by the project's own `@duckdb/node-api`, which opens the file for this call alone (src/duckdb.ts). */
async function analyzeDuckdb(url: string, sql: string): Promise<StatementDescriptor> {
  const root = process.cwd();
  const duckdb = await engine("duckdb");
  const db = duckdbDatabase(duckdbPath(url, root), root);
  return db.run(async (io) => {
    const [[version]] = (await io.query("select version()", [])) as [[string]];
    try {
      const analysis = await duckdb.analyze(sql, "duckdb", io, new AnswerCache());
      return descriptor(sql, "duckdb", version.replace(/^v/, ""), analysis, await untyped(duckdb, sql, "duckdb", analysis));
    } catch (e) {
      if (e instanceof Unreachable) throw failure(e);
      return refused(sql, "duckdb", version.replace(/^v/, ""), (e as Error).message);
    }
  });
}

/** Why the database could not be read, as `reylo analyze` words it. */
function failure(e: unknown): Error {
  if (e instanceof NotMysql) return new Error(`The URL names a MySQL database, and the server is ${e.server}, which Reylo does not read.`, { cause: e });
  if (e instanceof Unreachable) return new Error(`Cannot reach the database: ${reason(e)}`, { cause: e });
  return e as Error;
}

/**
 * An error at the first use of each parameter nothing in the statement types, as `reylo analyze`
 * and the type check give it under the project file's default, `strict_params: true`, in UTF-8
 * bytes (`untypedParams` in src/module-types.ts). The engine's own lexer finds the placeholders; a
 * statement it cannot read has none, and each error is then at the statement's start.
 */
async function untyped(e: Engine, sql: string, dialect: Database, analysis: Analysis): Promise<Analysis["diagnostics"]> {
  if (analysis.diagnostics.length || !analysis.params.some((p) => p.type.kind === "unknown")) return [];
  const lexed = dialect === "mysql" ? undefined : await e.lexAsync(sql, dialect).catch(() => undefined);
  return untypedParams(sql, analysis, placeholdersOf(analysis, dialect, lexed));
}

/**
 * The engine's analysis as the format writes it: the format's fields alone, a diagnostic's place
 * in UTF-16 units of the statement as written, and the database's own code (`Analysis::descriptor`
 * in engine/core/src/api.rs, which `reylo analyze` prints).
 */
function descriptor(sql: string, dialect: Database, version: string, a: Analysis, untyped: Analysis["diagnostics"] = []): StatementDescriptor {
  return {
    $schema: SCHEMA,
    dialect: { name: dialect, version },
    sql: a.rewritten.sql,
    ...(a.kind !== undefined && { kind: a.kind }),
    ...(a.comment !== undefined && { comment: a.comment }),
    params: a.params.map((p) => ({ name: p.name, type: p.type, ...(p.nullable && { nullable: true }), ...(p.optional && { optional: true }), ...(p.comment !== undefined && { comment: p.comment }) })),
    columns: a.columns.map((c) => ({ name: c.name, type: c.type, ...(c.nullable && { nullable: true }), ...(c.comment !== undefined && { comment: c.comment }) })),
    cardinality: a.cardinality,
    diagnostics: [...a.diagnostics, ...untyped].map((d) => {
      const start = utf16(sql, d.start);
      return { message: d.message, start, end: Math.max(utf16(sql, d.start + d.length), start), ...(d.server_code !== undefined && { code: d.server_code }) };
    }),
  };
}

/** A statement the engine could not analyze: no parameters or columns, and one diagnostic over the whole of it. */
function refused(sql: string, dialect: Database, version: string, message: string): StatementDescriptor {
  return { $schema: SCHEMA, dialect: { name: dialect, version }, sql, params: [], columns: [], cardinality: "many", diagnostics: [{ message, start: 0, end: sql.length }] };
}

/** The UTF-16 offset of the UTF-8 byte offset `byte` of `text`: a byte inside a character counts from the character's start, and one past the end at the end (`utf16_offset` in engine/core/src/api.rs). */
function utf16(text: string, byte: number): number {
  let bytes = 0;
  let units = 0;
  for (const ch of text) {
    const cp = ch.codePointAt(0)!;
    bytes += cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
    if (bytes > byte) return units;
    units += ch.length;
  }
  return units;
}
