// Standard Query: a SQL statement a driver can execute, as a typed function from an input to an
// output. A query is a Standard Typed (its `~standard.types` are the input and the output), and
// `~standard.query` holds the statement, the database it is written for, and two mappings: from
// the input to the values the driver binds, and from the rows the driver reads raw to the output.
// A driver that supports it runs the statement with `encode`'s values, reads every row as an
// array of raw values with its own conversions off, and returns what `decode` returns.
//
// Every Reylo query is one, with the same input and output as `.run()`: `encode` binds as
// `.run()` binds, its checks included, and `decode` gives each value its kind and the result its
// cardinality as `.run()` does.
import { rawStatement, type Dialect, type QueryModule } from "./execute.ts";
import type { StandardTypedV1 } from "./standard.ts";
import { coerce, DecodeError, textDecoder } from "./values.ts";

/** The Standard Query interface: a statement a driver can execute, as a typed function from an input to an output. */
export interface StandardQueryV1<Input = unknown, Output = Input, D extends StandardQueryV1.Dialect = StandardQueryV1.Dialect>
  extends StandardTypedV1<Input, Output> {
  /** The Standard properties. */
  readonly "~standard": StandardQueryV1.Props<Input, Output, D>;
}

export declare namespace StandardQueryV1 {
  /** The Standard Query properties interface. */
  export interface Props<Input = unknown, Output = Input, D extends StandardQueryV1.Dialect = StandardQueryV1.Dialect>
    extends StandardTypedV1.Props<Input, Output> {
    /** The statement and its two mappings. */
    readonly query: Query<Output, D>;
  }

  /** The Standard Query query interface. */
  export interface Query<Output = unknown, D extends StandardQueryV1.Dialect = StandardQueryV1.Dialect> {
    /** The database the statement is written for. A driver refuses a query for another one. */
    readonly dialect: D;
    /** The statement, with the database's positional placeholders: `$1`, `$2` on Postgres, `?` on MySQL and SQLite. */
    readonly text: string;
    /**
     * Maps the input to the values to bind, one per placeholder, in order, each in its database's
     * raw form. The driver passes the input as the caller gave it, `undefined` when it was left
     * out. Throws a `TypeError` for an input it cannot bind.
     */
    encode(input: unknown): readonly RawValue<D>[];
    /**
     * Maps the raw result to the output: every row the statement returned, each an array of raw
     * values in the order the statement selects them. Throws for a result that does not fit the
     * output (a second row where at most one is expected, say).
     */
    decode(rows: readonly (readonly RawValue<D>[])[]): Output;
  }

  /** The database a statement is written for. */
  export type Dialect =
    | "postgres"
    | "mysql"
    | "sqlite"
    // Accepts any string: allows other databases while preserving autocomplete
    | ({} & string);

  /**
   * A value in its raw form: what a driver of the database reads before converting anything, and
   * binds without converting anything. On Postgres, the value's text as the server sends it in
   * the text format. NULL is `null`.
   */
  export type RawValue<D extends Dialect = Dialect> =
    | null
    | (D extends "postgres" ? string : D extends "mysql" | "sqlite" ? string | number | bigint | Uint8Array : unknown);

  /** The arguments a driver takes after the query: optional when the input can be empty. */
  export type InferArgs<Q extends StandardTypedV1> = {} extends StandardTypedV1.InferInput<Q>
    ? [input?: StandardTypedV1.InferInput<Q>]
    : [input: StandardTypedV1.InferInput<Q>];
}

/** A column's raw value as its kind: on Postgres from its text, on MySQL and SQLite from whatever form the driver read. */
function decoder(dialect: Dialect, name: string, kind: string, delim: string | undefined): (v: unknown) => unknown {
  if (dialect === "postgres") {
    const fromText = textDecoder(kind, delim);
    return (v) => {
      if (typeof v !== "string") throw new DecodeError(`Column "${name}" is not in Postgres's text form: a driver hands a Standard Query each value's text.`);
      return fromText(v);
    };
  }
  // A MySQL DATETIME's text takes a T between the date and the time, as `.run()` writes it.
  if (dialect === "mysql" && kind === "timestamp") return (v) => (typeof v === "string" ? v.replace(" ", "T") : coerce(kind, v, name, delim));
  return (v) => coerce(kind, v, name, delim);
}

/**
 * The query members of a module: `encode` through `.run()`'s own binding, `decode` through its
 * decoders, then `materialize`, which applies the cardinality as `.run()` does.
 */
export function standardQuery<D extends Dialect, Output>(module: QueryModule<D>, materialize: (rows: Record<string, unknown>[]) => Output): StandardQueryV1.Query<Output, D> {
  let text: string | undefined;
  let decoders: ((v: unknown) => unknown)[] | undefined;
  return {
    dialect: module.dialect,
    get text() {
      return (text ??= rawStatement(module, {}, false).text);
    },
    encode(input) {
      const bound = rawStatement(module, input === undefined ? {} : input, true);
      // A structured parameter in a MySQL insert writes its column lists again for the keys a call
      // carries, so its text depends on the input: such a statement runs through `.run()`.
      if (bound.text !== this.text) throw new TypeError(`${module.source ? `${module.source}: ` : ""}This statement's text depends on its input (a structured parameter in an insert names the keys it carries), so it runs through .run(), not as a Standard Query.`);
      return bound.values as StandardQueryV1.RawValue<D>[];
    },
    decode(rows) {
      const columns = module.columns;
      if (columns === undefined) throw new TypeError("A query defined by hand without its columns cannot map rows to its output: a module a build step writes carries them.");
      decoders ??= columns.map(([name, kind, delim]) => decoder(module.dialect, name, kind, delim));
      return materialize(
        rows.map((raw) => {
          if (raw.length !== columns.length) throw new DecodeError(`The statement selects ${columns.length} columns, and a row has ${raw.length}.`);
          const row: Record<string, unknown> = {};
          for (let i = 0; i < raw.length; i++) row[columns[i]![0]] = raw[i] === null || raw[i] === undefined ? null : decoders![i]!(raw[i]);
          return row;
        }),
      );
    },
  };
}
