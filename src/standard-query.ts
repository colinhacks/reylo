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
import { sqliteDecoder } from "./decode-sqlite.ts";
import { compile, rawStatement, sentText, type Dialect, type QueryModule } from "./execute.ts";
import type { StandardTypedV1 } from "./standard.ts";
import { rowOf, type RowBuilder } from "./row.ts";
import { coerce, DecodeError, numbersInJson, textDecoder, withinDimensions, type JsonNumbers } from "./values.ts";

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
    /**
     * Each output column, in the order the statement selects them: its name, and `decode`, which
     * maps one raw value of the column, never NULL, to the value `decode` puts in the row. With
     * `output`, it lets a driver that reads a result value by value decode each value once, as it
     * reads it, rather than hand `decode` the raw rows. NULL is null in the row, and a driver
     * never calls a column's `decode` for it. Absent where the columns are not known.
     */
    readonly columns?: readonly Column<D>[];
    /**
     * Maps rows already decoded through `columns`, each an object keyed by the columns' names, to
     * the output, as `decode` does once it has decoded them: `decode(rows)` equals `output` of the
     * same rows with each value through its column's `decode`. Throws as `decode` does for a result
     * that does not fit the output (a second row where at most one is expected, say). Present
     * wherever `columns` is.
     */
    output?(rows: readonly Record<string, unknown>[]): Output;
    /**
     * Whether the order of the rows is total: the statement's ORDER BY sorts by a key of the rows
     * it sorts, so no two rows of a result tie and the order is the same on every run. A driver
     * comparing two results of the query by position can rely on it. `true` where the order is
     * total, and absent otherwise, a statement of one row at most included; a query's declared
     * type carries the literal `true`, so a driver's signature can require it.
     */
    readonly ordered?: true;
  }

  /** One output column of a query: its name, and the decoder of its raw values. */
  export interface Column<D extends StandardQueryV1.Dialect = StandardQueryV1.Dialect> {
    /** The column's name, as the output's rows are keyed by it. */
    readonly name: string;
    /** Maps one raw value of the column, never NULL, to its value in the output's rows. */
    decode(value: Exclude<RawValue<D>, null>): unknown;
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

/** A column's raw value as its kind, on MySQL from whatever form the driver read. Postgres's are its text's decoders, SQLite's `.run()`'s own (`sqliteDecoder`). */
function decoder(name: string, kind: string, delim: string | undefined): (v: unknown) => unknown {
  // A MySQL DATETIME's text takes a T between the date and the time, as `.run()` writes it, a
  // column that may be one included (src/decode-mysql.ts, `mariadb`).
  if (kind.split("|").includes("timestamp")) return (v) => (typeof v === "string" ? v.replace(" ", "T") : coerce(kind, v, name, delim));
  return (v) => coerce(kind, v, name, delim);
}

/** A Postgres column's text as its kind, with the non-finite numbers inside its JSON as numbers and an array held to its dimensions, as `.run()` gives them (src/execute.ts). */
function postgresDecoder(name: string, kind: string, delim: string | undefined, nonFinite: JsonNumbers | undefined): (v: string) => unknown {
  const decode = textDecoder(kind, delim);
  const numbers = nonFinite === undefined ? decode : (v: string) => numbersInJson(decode(v), nonFinite);
  return kind.endsWith("[]") ? (v) => withinDimensions(numbers(v), kind, name) : numbers;
}

const notText = (name: string): never => {
  throw new DecodeError(`Column "${name}" is not in Postgres's text form: a driver hands a Standard Query each value's text.`);
};

const columnCount = (columns: number, got: number) => new DecodeError(`The statement selects ${columns} columns, and a row has ${got}.`);

/**
 * The query members of a module: `encode` through `.run()`'s own binding, `decode` through its
 * decoders, then `materialize`, which applies the cardinality as `.run()` does.
 */
export function standardQuery<D extends Dialect, Output>(module: QueryModule<D>, materialize: (rows: Record<string, unknown>[]) => Output): StandardQueryV1.Query<Output, D> & { output(rows: readonly Record<string, unknown>[]): Output } {
  let text: string | undefined;
  let build: RowBuilder | undefined;
  const dialect: Dialect = module.dialect;
  // Each column's decoder, which `decode` runs on every value and `columns` hands a driver. SQLite's
  // is `.run()`'s own reading, its refusals of a storage class the column's type does not allow
  // included. Its default suits a driver whose integer mode is unknown: a BigInt is read exact, and
  // a number past 2^53 in an INTEGER column is refused as a rounded one.
  const decoders = module.columns?.map(([name, kind, delim], i): ((v: never) => unknown) => (dialect === "postgres" ? postgresDecoder(name, kind, delim, module.nonFinite?.[i] ?? undefined) : dialect === "sqlite" ? sqliteDecoder(name, kind) : decoder(name, kind, delim)));
  const columns = module.columns?.map(([name], i) => Object.freeze({ name, decode: decoders![i]! }) as StandardQueryV1.Column<D>);
  // A closure, not `this`: a driver may take `encode` off the object.
  // The statement as Reylo sends it, the comment naming its file included, as `.run()` sends it.
  const statementText = () => (text ??= sentText(module, compile(module).text));
  return {
    dialect: module.dialect,
    ...(module.key === undefined ? {} : { key: module.key }),
    ...(columns === undefined ? {} : { columns: Object.freeze(columns) }),
    output: (rows) => materialize(rows as Record<string, unknown>[]),
    ...(module.ordered ? { ordered: true as const } : {}),
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
      const names = module.columns;
      if (names === undefined || decoders === undefined) throw new TypeError("A query defined by hand without its columns cannot map rows to its output: a module a build step writes carries them.");
      const n = names.length;
      const fns = decoders as ((v: unknown) => unknown)[];
      // Each row's values, decoded into one array the builder copies from.
      const values = new Array<unknown>(n);
      build ??= module.row ?? rowOf(names.map(([name]) => name));
      const out = new Array<Record<string, unknown>>(rows.length);
      for (let r = 0; r < rows.length; r++) {
        const raw = rows[r]!;
        if (raw.length !== n) throw columnCount(n, raw.length);
        // Postgres's values are text, each checked where its decoder takes it.
        if (dialect === "postgres")
          for (let i = 0; i < n; i++) {
            const v = raw[i];
            values[i] = v === null || v === undefined ? null : typeof v === "string" ? fns[i]!(v) : notText(names[i]![0]);
          }
        else
          for (let i = 0; i < n; i++) {
            const v = raw[i];
            values[i] = v === null || v === undefined ? null : fns[i]!(v);
          }
        out[r] = build(values);
      }
      return materialize(out);
    },
  };
}
