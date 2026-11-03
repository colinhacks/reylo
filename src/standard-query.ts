// Standard Query: a SQL statement a driver can execute, as a typed function from an input to an
// output. A query is a Standard Typed (its `~standard.types` are the input and the output), and
// `~standard.query` holds the statement, the database it is written for, and two mappings: from
// the input to the values the driver binds, and from the rows the driver reads raw to the output.
// A driver that supports it runs the statement with `encode`'s values, reads every row as an
// array of raw values with its own conversions off, and returns what `decode` returns.
//
// Every Reylo query is one, with the same input and output as `.run()`: `encode` binds as
// `.run()` binds, its checks included, and `decode` gives each value its kind and the result its
// cardinality as `.run()` does. Two things `.run()` does need its client, and a Standard Query,
// which has none, does neither: it never checks that the schema still matches the one the
// statement was typed against (src/fresh.ts), and on MySQL it keeps a DATETIME's, TIMESTAMP's or
// TIME's fraction digits as the driver read them, which are the column's own over the text
// protocol, while mysql2's binary protocol drops trailing zeros (`.000`) that `.run()` writes back
// from the column's metadata.
import { normalize } from "./decode-sqlite.ts";
import { compile, rawStatement, sentText, type Dialect, type QueryModule } from "./execute.ts";
import type { StandardTypedV1 } from "./standard.ts";
import { rowOf, type RowBuilder } from "./row.ts";
import { coerce, DecodeError, textDecoder, type Decoder } from "./values.ts";

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
    /**
     * The result key: output columns, by name, that identify each row of the result, since no two
     * rows the statement returns agree on all of them (a null counting as equal to another null).
     * A driver comparing two results of the query tells by it that a row changed in place rather
     * than one row going and another coming. Never empty: absent where the statement returns one
     * row at most, which its cardinality says, and where no key is known.
     */
    readonly key?: readonly string[];
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

/** A column's raw value as its kind, on MySQL from whatever form the driver read. Postgres's are its text's decoders; SQLite's rows go through `normalize` whole. */
function decoder(name: string, kind: string, delim: string | undefined): (v: unknown) => unknown {
  // A MySQL DATETIME's text takes a T between the date and the time, as `.run()` writes it, a
  // column that may be one included (src/decode-mysql.ts, `mariadb`).
  if (kind.split("|").includes("timestamp")) return (v) => (typeof v === "string" ? v.replace(" ", "T") : coerce(kind, v, name, delim));
  return (v) => coerce(kind, v, name, delim);
}

const notText = (name: string): never => {
  throw new DecodeError(`Column "${name}" is not in Postgres's text form: a driver hands a Standard Query each value's text.`);
};

const columnCount = (columns: number, got: number) => new DecodeError(`The statement selects ${columns} columns, and a row has ${got}.`);

/**
 * The query members of a module: `encode` through `.run()`'s own binding, `decode` through its
 * decoders, then `materialize`, which applies the cardinality as `.run()` does.
 */
export function standardQuery<D extends Dialect, Output>(module: QueryModule<D>, materialize: (rows: Record<string, unknown>[]) => Output): StandardQueryV1.Query<Output, D> {
  let text: string | undefined;
  let decoders: ((v: unknown) => unknown)[] | undefined;
  let textDecoders: Decoder[] | undefined;
  let build: RowBuilder | undefined;
  // A closure, not `this`: a driver may take `encode` off the object.
  // The statement as Reylo sends it, the comment naming its file included, as `.run()` sends it.
  const statementText = () => (text ??= sentText(module, compile(module).text));
  return {
    dialect: module.dialect,
    ...(module.key === undefined ? {} : { key: module.key }),
    get text() {
      return statementText();
    },
    encode(input) {
      const bound = rawStatement(module, input === undefined ? {} : input);
      // A structured parameter in a MySQL insert writes its column lists again for the keys a call
      // carries, so its text depends on the input: such a statement runs through `.run()`.
      if (bound.text !== statementText()) throw new TypeError(`${module.source ? `${module.source}: ` : ""}This statement's text depends on its input (a structured parameter in an insert names the keys it carries), so it runs through .run(), not as a Standard Query.`);
      return bound.values as StandardQueryV1.RawValue<D>[];
    },
    decode(rows) {
      const columns = module.columns;
      if (columns === undefined) throw new TypeError("A query defined by hand without its columns cannot map rows to its output: a module a build step writes carries them.");
      const n = columns.length;
      // Each row's values, decoded into one array the builder copies from.
      const values = new Array<unknown>(n);
      build ??= module.row ?? rowOf(columns.map(([name]) => name));
      const out = new Array<Record<string, unknown>>(rows.length);
      if (module.dialect === "postgres") {
        // Postgres's text decoders, each value checked for text where the decoder takes it.
        const text = (textDecoders ??= columns.map(([, kind, delim]) => textDecoder(kind, delim)));
        for (let r = 0; r < rows.length; r++) {
          const raw = rows[r]!;
          if (raw.length !== n) throw columnCount(n, raw.length);
          for (let i = 0; i < n; i++) {
            const v = raw[i];
            values[i] = v === null || v === undefined ? null : typeof v === "string" ? text[i]!(v) : notText(columns[i]![0]);
          }
          out[r] = build(values);
        }
        return materialize(out);
      }
      // SQLite's values go through `.run()`'s own `normalize`, its refusals of a storage class the
      // column's type does not allow included. Its default reading suits a driver whose integer
      // mode is unknown: a BigInt is read exact, and a number past 2^53 in an INTEGER column is
      // refused as a rounded one.
      const sqlite = module.dialect === "sqlite";
      if (!sqlite) decoders ??= columns.map(([name, kind, delim]) => decoder(name, kind, delim));
      for (let r = 0; r < rows.length; r++) {
        const raw = rows[r]!;
        if (raw.length !== n) throw columnCount(n, raw.length);
        for (let i = 0; i < n; i++) {
          const v = raw[i];
          values[i] = v === null || v === undefined ? null : sqlite ? v : decoders![i]!(v);
        }
        out[r] = build(values);
      }
      return materialize(sqlite ? (normalize(out, columns) as Record<string, unknown>[]) : out);
    },
  };
}
