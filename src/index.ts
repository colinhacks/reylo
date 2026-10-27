// The runtime query object behind `import query from "./file.sql"`.
// Types come from the content mapper; values come from the runtime loader.
import { compile, execute, executeSync, type ClientFor, type Dialect, type Inputs, type QueryModule, type SyncClientFor } from "./execute.ts";
import type { StandardJSONSchemaV1, StandardTypedV1 } from "./standard.ts";
import { standardQuery, type StandardQueryV1 } from "./standard-query.ts";
import type { Structured } from "./rows.ts";
import type { Checks } from "./checks.ts";
import type { Fresh } from "./fresh.ts";
import { DecodeError, ParameterError, type Columns } from "./values.ts";
import { readQueryError, readQueryErrorSync, type QueryError } from "./query-error.ts";
import { listening, type QueryTrace } from "./trace.ts";

export type { ClientFor, Dialect, Inputs, MysqlClient, PostgresClient, SqliteClient, SyncClientFor } from "./execute.ts";
export type { QueryTrace } from "./trace.ts";
export type { StandardQueryV1 } from "./standard-query.ts";
export { checks } from "./checks.ts";
export type { MysqlCode, MysqlNarrowing, MysqlQueryError, PostgresCode, PostgresCondition, PostgresNarrowing, PostgresQueryError, QueryError, SqliteCode, SqliteNarrowing, SqliteQueryError } from "./query-error.ts";

/** How many rows a statement returns: `none` is a data-modifying statement without RETURNING. */
export type Cardinality = "many" | "one" | "maybeOne" | "none";

export type Materialize<Row, C extends Cardinality> = C extends "many" ? Row[] : C extends "one" ? Row : C extends "none" ? void : Row | null;

/**
 * The shape of a row value as JSON: a BigInt is the integer it is (JSON.stringify refuses one, so a
 * serializer writes it, as `JSON.rawJSON(String(v))` does), and a Temporal value or a byte array
 * becomes text.
 */
export type Json<T> = T extends bigint
  ? number
  : T extends Date | Uint8Array | { toJSON(): string }
  ? string
  : T extends readonly (infer U)[]
    ? Json<U>[]
    : T extends object
      ? { [K in keyof T]: Json<T[K]> }
      : T;

export type ReyloTypes<Params, Row, C extends Cardinality> = {
  readonly element: Row;
  readonly cardinality: C;
  readonly json: Materialize<Json<Row>, C>;
  readonly input: Params;
};

/**
 * What `try` returns: the value, or the error the database raised. The other field is undefined,
 * so either can be read first, as `if (result.error)`; the shape is Standard Schema's result, with one
 * error where it has a list of issues.
 */
export type TryResult<Value, E> = { readonly value: Value; readonly error?: undefined } | { readonly error: E; readonly value?: undefined };

/** A query: its statement, its parameters and result types (`Params`, `Row`), and the error its database may raise (`E`). */
export interface Query<Params, Row, C extends Cardinality, D extends Dialect, E = QueryError<D>>
  extends StandardTypedV1<Params, Materialize<Row, C>>, StandardJSONSchemaV1<Params, Materialize<Row, C>>, StandardQueryV1<Params, Materialize<Row, C>, D> {
  readonly dialect: D;
  // `sql` and `raw` carry no doc comment here: each module declares them again with its statement
  // and what they are (src/doc-comment.ts, `sqlNote` and `RAW_NOTE`), and TypeScript would show
  // this one first, run together with that one. `sql` is the base text: a structured parameter
  // (`::rows`, `::row`, src/rows.ts) in a MySQL insert has its column lists written again for the
  // keys each call carries. Said here and not in `sqlNote`, whose text the docs site shows in its
  // hovers, which leave structured parameters out.
  readonly sql: string;
  readonly raw: string;
  /** Parameter names in binding order. */
  readonly params: readonly string[];
  /**
   * The `.sql` file the query was built from, relative to the project's root: a name for logs and
   * traces. Reylo's own errors about the query start with it. A query defined by hand has none.
   */
  readonly source: string | undefined;
  readonly cardinality: C;
  /** The JSON Schema (2020-12) of the parameters, as the object `run` takes. */
  readonly inputSchema: Record<string, unknown>;
  /** The JSON Schema (2020-12) of the result as JSON: what `run` returns, written as JSON, a BigInt as the integer it is. */
  readonly outputSchema: Record<string, unknown>;
  /** Standard JSON Schema's `jsonSchema` and Standard Query's `query`: a driver that supports Standard Query runs the import with the same input and output as `.run()`. */
  readonly "~standard": StandardJSONSchemaV1.Props<Params, Materialize<Row, C>> & StandardQueryV1.Props<Params, Materialize<Row, C>, D>;
  readonly "~reylo": ReyloTypes<Params, Row, C>;
  /**
   * Runs the statement on the client, or on a transaction or connection taken from it. The
   * parameters are an object keyed by name (by number, `{ 1: … }`, for positional placeholders),
   * left out when the statement takes none. Always a promise, which rejects with the driver's own error.
   */
  run(client: ClientFor<D>, ...params: ParamsArg<Params>): Promise<Materialize<Row, C>>;
  /**
   * Runs the statement on a synchronous SQLite client (better-sqlite3, node:sqlite, bun:sqlite)
   * and returns the result itself, throwing the driver's error, so it works inside their
   * `db.transaction(fn)`, whose function cannot await.
   */
  runSync(client: SyncClientFor<D>, ...params: ParamsArg<Params>): Materialize<Row, C>;
  /**
   * Runs the statement as `run` does, and resolves to `{ value }`, or to `{ error }` where the
   * database raised an error: its code from the database's own list, and the constraint, table or
   * column it names. Every other error rejects, as from `run`: a refused connection, a client
   * Reylo cannot run on, a value Reylo refuses.
   */
  try(client: ClientFor<D>, ...params: ParamsArg<Params>): Promise<TryResult<Materialize<Row, C>, E>>;
  /** `try` on a synchronous SQLite client, as `runSync`: it returns the result itself. */
  trySync(client: SyncClientFor<D>, ...params: ParamsArg<Params>): TryResult<Materialize<Row, C>, E>;
  /** The statement bound to a client, such as a transaction: `const get = q.with(tx); await get({ id: 1 })`. */
  with(client: ClientFor<D>): (...params: ParamsArg<Params>) => Promise<Materialize<Row, C>>;
}

/** The parameters argument, always an object: required when the statement takes any, optional when it takes none. */
export type ParamsArg<Params> = {} extends Params ? [params?: Params] : [params: Params];

export type QueryDefinition = {
  dialect: Dialect;
  /** The statement as written, Reylo's placeholders in place: the query's `raw`. */
  sql: string;
  params: readonly string[];
  /**
   * The statement as the engine compiled it for the database, the query's `sql`, and its binding
   * order: every build step puts them in the module. A query defined by hand without them is
   * compiled from `sql` by the run time's own lexer.
   */
  text?: string;
  order?: readonly string[];
  /** Each parameter's type, by name, as the engine found it: every build step puts it in the module, and a driver that binds some values wrong gets them converted by it. */
  inputs?: Inputs;
  /** Each output column's name and kind (src/values.ts), in order: every build step puts them in the module, and the run time gives each value its kind by them. */
  columns?: Columns;
  /** Each structured parameter in a MySQL insert, with the lists of `text` the run time writes again for the keys a call carries (a Postgres or SQLite insert copies its defaults instead). */
  structured?: readonly Structured[];
  /** The catalog facts the statement relied on, which the run time checks before it runs (src/fresh.ts). */
  fresh?: Fresh;
  /** The `.sql` file, relative to the project's root and written with `/`: the query's `source`. Every build step puts it in the module. */
  source?: string;
  cardinality?: Cardinality;
  jsonSchema?: { input: Record<string, unknown>; output: Record<string, unknown> };
  /** The checks of a typed JSON, structured or cast parameter, `checks` from "reylo": a build step passes them where the statement has one, so that an app none of whose statements has one bundles none of them. */
  checks?: Checks;
};

export function defineQuery<Params, Row, C extends Cardinality, D extends Dialect>(
  def: QueryDefinition & { dialect: D; cardinality?: C },
): Query<Params, Row, C, D> {
  const cardinality = (def.cardinality ?? "many") as C;
  const missingSchema = () => {
    throw new Error(`No JSON Schema is available for this query: a module a build step writes carries it, and a query defined by hand has none.`);
  };
  // Reylo's own errors about the query start with its file.
  const where = def.source ? `${def.source}: ` : "";
  // What the run time reads, kept off the query object, whose `sql` is the compiled statement.
  const module: QueryModule<D> = {
    dialect: def.dialect,
    sql: def.sql,
    params: def.params,
    text: def.text,
    order: def.order,
    inputs: def.inputs,
    columns: def.columns,
    structured: def.structured,
    fresh: def.fresh,
    source: def.source,
    checks: def.checks,
  };
  let compiled: string | undefined;
  const query: Query<Params, Row, C, D> = {
    dialect: def.dialect,
    // A query defined by hand is compiled when its `sql` is first read, so a placeholder its
    // `params` misses fails there and in `run`, not where it is defined.
    get sql() {
      return (compiled ??= compile(module).text);
    },
    raw: def.sql,
    params: def.params,
    source: def.source,
    cardinality,
    get inputSchema() {
      return def.jsonSchema?.input ?? missingSchema();
    },
    get outputSchema() {
      return def.jsonSchema?.output ?? missingSchema();
    },
    "~standard": {
      version: 1,
      vendor: "reylo",
      jsonSchema: {
        input: () => def.jsonSchema?.input ?? missingSchema(),
        output: () => def.jsonSchema?.output ?? missingSchema(),
      },
      query: standardQuery(module, (rows) => materialize(rows as Row[])),
    },
    "~reylo": undefined as unknown as ReyloTypes<Params, Row, C>,
    run: ((client: ClientFor<D>, params?: Params) => run(client, params ?? ({} as Params))) as Query<Params, Row, C, D>["run"],
    runSync: ((client: SyncClientFor<D>, params?: Params) => {
      const values = (params ?? {}) as Record<string, unknown>;
      const runOnce = () => {
        try {
          return materialize(executeSync(client, module, values) as Row[]);
        } catch (error) {
          throw named(error);
        }
      };
      const tracing = listening();
      return tracing ? tracing.traceSync(runOnce, trace(client, values, true)) : runOnce();
    }) as Query<Params, Row, C, D>["runSync"],
    try: ((client: ClientFor<D>, params?: Params) =>
      run(client, params ?? ({} as Params)).then(
        (value) => ({ value }),
        async (error) => {
          const read = await readQueryError(def.dialect, error);
          if (read) return { error: read };
          throw error;
        },
      )) as Query<Params, Row, C, D>["try"],
    trySync: ((client: SyncClientFor<D>, params?: Params) => {
      try {
        return { value: query.runSync(client, ...([params] as ParamsArg<Params>)) };
      } catch (error) {
        const read = readQueryErrorSync(def.dialect, error);
        if (read) return { error: read };
        throw error;
      }
    }) as Query<Params, Row, C, D>["trySync"],
    with: ((client: ClientFor<D>) => (params?: Params) => run(client, params ?? ({} as Params))) as Query<Params, Row, C, D>["with"],
  };
  // `.run()`, `.try()` and the function `.with()` returns, published on the tracing channel as one
  // call each (src/trace.ts).
  function run(client: ClientFor<D>, params: Params): Promise<Materialize<Row, C>> {
    const tracing = listening();
    return tracing ? tracing.tracePromise(() => runOnce(client, params), trace(client, params as Record<string, unknown>, false)) : runOnce(client, params);
  }
  function trace(client: unknown, params: Record<string, unknown>, sync: boolean): QueryTrace {
    return { query: query as unknown as QueryTrace["query"], client, params, sync };
  }
  // A synchronous client answers before run returns, but its rows and its error still come
  // back through the promise, so run means the same on every client.
  function runOnce(client: ClientFor<D>, params: Params): Promise<Materialize<Row, C>> {
    try {
      const rows = execute(client, module, params as Record<string, unknown>);
      return Array.isArray(rows)
        ? Promise.resolve(materialize(rows as Row[]))
        : rows.then(
            (rows) => materialize(rows as Row[]),
            (error) => Promise.reject(named(error)),
          );
    } catch (error) {
      return Promise.reject(named(error));
    }
  }
  // Reylo's errors about a parameter's value or a column's value take the file, as the row counts'
  // below do. The driver's own errors, and Reylo's about the client, pass through as they are.
  function named(error: unknown): unknown {
    if (where && (error instanceof ParameterError || error instanceof DecodeError) && !error.message.startsWith(where)) {
      const message = error.message;
      error.message = where + message;
      // V8 writes the stack when it is first read, with the message as it is then; a stack written
      // earlier (read before, or on Bun) still has the old message in its first line.
      if (typeof error.stack === "string" && !error.stack.includes(error.message)) error.stack = error.stack.replace(message, () => error.message);
    }
    return error;
  }
  function materialize(rows: Row[]): Materialize<Row, C> {
    switch (cardinality) {
      case "none":
        return undefined as Materialize<Row, C>;
      case "many":
        return rows as Materialize<Row, C>;
      case "one":
        if (rows.length !== 1) throw new Error(`${where}Expected exactly one row, got ${rows.length}`);
        return rows[0] as Materialize<Row, C>;
      default:
        if (rows.length > 1) throw new Error(`${where}Expected at most one row, got ${rows.length}`);
        return (rows[0] ?? null) as Materialize<Row, C>;
    }
  }
  return query;
}
