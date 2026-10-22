// Reylo decodes every value of its own MySQL queries itself, on the app's own pool, connection or
// transaction (the representation is src/values.ts). Each driver but
// Bun.sql takes its decoding choices per query, laid over the app's without changing them
// (wiki/driver-decoding/spike/mysql.md):
//
// - mysql2 stays on `execute`, the binary protocol (its `query` writes the values into the text
//   with backslash escapes, which matched every row of a table under NO_BACKSLASH_ESCAPES). Its
//   per-query options make a BIGINT and a date the server's text, and Reylo's per-query `typeCast`
//   reads a DECIMAL and a JSON value as their text, which the app's `decimalNumbers` and
//   `jsonStrings` (read from the app's config only) would otherwise decide; it runs only where a
//   statement or the app's settings need it, since mysql2 calls it for every value. A BIGINT's
//   exact text becomes a BigInt, by the result's own column type, and a date's dropped zero
//   fraction comes back from the result's fields. `decimalNumbers` is never passed per query:
//   mysql2 keys its row-parser cache, shared by every pool in the process, by the per-query value
//   and builds the parser from the app's, so one query would poison another pool's parser.
// - mariadb takes every option per query, both ways: a BIGINT is its own BigInt (mariadb 2's too), and
//   `metaAsArray: false` keeps the rows an array of rows. A SET comes back an array, which no
//   option prevents: Reylo joins it.
// - PlanetScale takes a `cast` per query, which replaces the app's: Reylo's is the driver's own
//   default, with a BIGINT a BigInt and a DATETIME's `T`.
// - Bun.sql takes nothing per query: its values are converted (src/decode.ts, `bunRows`).
//
// MySQL has no boolean type: a TINYINT(1), a BIT(1) and a comparison are integers to every driver,
// and a column whose kind is `bool` (src/values.ts) is made a boolean after the driver, a value
// other than 0 or 1 refused (`truth`).
import { bigInteger, coerce, truth, utf8, type Columns, type Kind } from "./values.ts";

type AnyClient = Record<string, any>;

// ---------------------------------------------------------------------------------------------
// mysql2

/** Options mysql2 ORs with the app's (a BIGINT and a date as text) and the row's shape, which the query's own replace. */
const MYSQL2_OPTIONS = { supportBigNumbers: true, bigNumberStrings: true, dateStrings: true, rowsAsArray: false, nestTables: false };

type Mysql2Field = { type: string; string(encoding?: string): string | null; buffer(): Uint8Array | null };

/**
 * Reylo's typeCast: a DECIMAL as its text and a JSON value parsed from its bytes (both are
 * length-coded in the binary protocol too, so they read whole), JSON here because mysql2's own
 * parser turns an integer past 2^53 inside a document into a string under `supportBigNumbers`; a
 * GEOMETRY as its bytes; every other type mysql2's own reading with the options above.
 */
function mysql2Cast(f: Mysql2Field, next: () => unknown): unknown {
  switch (f.type) {
    case "NEWDECIMAL":
    case "DECIMAL":
      return f.string("ascii");
    case "JSON": {
      const bytes = f.buffer();
      return bytes === null ? null : JSON.parse(utf8(bytes));
    }
    case "GEOMETRY":
      return f.buffer();
    default:
      return next();
  }
}

/** A column's protocol type and fraction digits, as mysql2's fields give them. */
type Mysql2Column = { name: string; columnType: number; decimals: number };
const DATETIME = 12, TIMESTAMP = 7, TIME = 11;

/**
 * A DATETIME or TIMESTAMP with `T`, and the fraction digits the binary protocol drops when they are
 * all zero (mysql2 writes `09:31:00` for a DATETIME(3) of .000, and TIME's trailing zeros never),
 * from the column's own count.
 */
function wallClock(s: string, decimals: number, type: number): string {
  let text = s;
  if (decimals > 0) {
    const dot = text.indexOf(".");
    text = dot < 0 ? `${text}.${"0".repeat(decimals)}` : text.padEnd(dot + 1 + decimals, "0");
  }
  return type === TIME ? text : text.replace(" ", "T");
}

/** The app's settings, where mysql2 shows them: a promise Pool's, Connection's or PoolConnection's. */
type Mysql2Config = { typeCast?: unknown; decimalNumbers?: boolean; jsonStrings?: boolean };
const configOf = (m: AnyClient): Mysql2Config | undefined => m.pool?.config?.connectionConfig ?? m.connection?.config;

/**
 * Whether the statement needs Reylo's typeCast, which mysql2 calls for every value of the binary
 * protocol (about 70 ns each): for a JSON or GEOMETRY column, whenever the columns are not known,
 * and where the app's settings, which a query cannot turn off, would decide a DECIMAL or a JSON value
 * or replace mysql2's own reading (or cannot be read).
 */
function needsCast(m: AnyClient, columns: Columns | undefined): boolean {
  if (!columns || columns.some(([, kind]) => kind === "unknown" || kind.split("|").some((k) => k === "json" || k.startsWith("json")))) return true;
  const config = configOf(m);
  return config === undefined || typeof config.typeCast === "function" || config.decimalNumbers === true || config.jsonStrings === true;
}

/** mysql2's protocol type of a BIGINT (MYSQL_TYPE_LONGLONG). */
const LONGLONG = 8;

/** What a column's values need after a driver: a truth value made a boolean, a BIGINT a BigInt. */
type Fix = (v: unknown, name: string) => unknown;
const toBool: Fix = truth;
// A BIGINT's text is always its digits: BigInt reads it straight.
const toBigInt: Fix = (v, name) => (typeof v === "string" ? BigInt(v) : typeof v === "number" || typeof v === "bigint" ? bigInteger(v, name) : v);

// A VECTOR is MySQL's one array: mysql2 decodes its floats, mariadb and PlanetScale give its bytes.
const toFloats: Fix = (v, name) => coerce("float[]", v, name);

/**
 * The fix a column of `kind` needs, if any: `bool` a boolean, `int8` (a BIGINT, or an integer a
 * temporary table may widen to one) a BigInt from whatever the driver gave, `float[]` (a VECTOR)
 * its floats.
 */
function byKind(kind: Kind | undefined): Fix | undefined {
  if (kind === "bool") return toBool;
  if (kind === "int8") return toBigInt;
  if (kind === "float[]") return toFloats;
  return undefined;
}

/** Applies each column's fix, in place: the drivers build new rows per call. */
function fixRows(rows: Record<string, unknown>[], fixes: (readonly [string, Fix])[]): void {
  if (!fixes.length) return;
  for (const row of rows) {
    for (const [name, fix] of fixes) {
      const v = row[name];
      if (v !== null && v !== undefined) row[name] = fix(v, name);
    }
  }
}

/** mysql2's execute(), resolving to [rows, fields]. */
export async function mysql2(m: AnyClient, text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  const [rows, fields] = await m.execute({ sql: text, ...MYSQL2_OPTIONS, typeCast: needsCast(m, columns) ? mysql2Cast : true }, values);
  if (!Array.isArray(rows)) return [];
  const described = (fields ?? []) as Mysql2Column[];
  // The module's kinds where its columns are the result's; a BIGINT by the result's own type, which
  // says it of an either too (`:n + 1`, a BIGINT or a DOUBLE by what is bound).
  const kinds = columns && columns.length === described.length ? columns : undefined;
  const fixes: (readonly [string, Fix])[] = [];
  described.forEach((f, i) => {
    const fix = byKind(kinds?.[i]![1]) ?? (f.columnType === LONGLONG ? toBigInt : undefined);
    if (fix) fixes.push([f.name, fix]);
  });
  // A DATETIME's, TIMESTAMP's and TIME's text, padded to the column's own fraction digits.
  for (const f of described) {
    if (f.columnType === DATETIME || f.columnType === TIMESTAMP || (f.columnType === TIME && f.decimals > 0)) {
      const decimals = f.decimals > 6 ? 0 : f.decimals, type = f.columnType;
      fixes.push([f.name, (v) => (typeof v === "string" ? wallClock(v, decimals, type) : v)]);
    }
  }
  fixRows(rows as Record<string, unknown>[], fixes);
  return rows;
}

// ---------------------------------------------------------------------------------------------
// mariadb

/**
 * Every decoding option, each laid over the app's for this query (mariadb's parser.js:
 * Object.assign(Object.create(connOpts), opts)). mariadb 3: with `bigIntAsNumber` and
 * `supportBigNumbers` off a BIGINT is its own BigInt, and with `decimalAsNumber` off too a DECIMAL
 * its text (`bigNumberStrings` matters only beside `supportBigNumbers`). mariadb 2, which has none
 * of the first two: `supportBigInt` makes a BIGINT a BigInt and `bigNumberStrings` a DECIMAL its
 * text, where it would be a lossy number.
 */
const MARIADB_OPTIONS = {
  bigIntAsNumber: false,
  decimalAsNumber: false,
  supportBigNumbers: false,
  bigNumberStrings: true,
  supportBigInt: true,
  checkNumberRange: false,
  dateStrings: true,
  jsonStrings: false,
  autoJsonMap: true,
  bitOneIsBoolean: false,
  rowsAsArray: false,
  nestTables: false,
  metaAsArray: false,
  typeCast: undefined,
};

/** A GEOMETRY's bytes, where mariadb would give GeoJSON; only for a statement whose columns hold one, since a typeCast sends every value through mariadb's slower path. */
const mariadbGeometry = (column: { type: string; buffer(): unknown }, next: () => unknown) => (column.type === "GEOMETRY" ? column.buffer() : next());

type MariadbColumn = { type?: string; columnType?: number; name?: string | (() => string); isSet?: () => boolean };

/** mariadb's query(), on its promise or its callback API, with Reylo's options. */
export async function mariadb(c: AnyClient, text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  const options = { sql: text, ...MARIADB_OPTIONS, ...(columns?.some(([, kind]) => kind === "unknown") ? { typeCast: mariadbGeometry } : {}) };
  // The promise API resolves to the rows with their `meta`; the callback API hands the meta over as a third argument.
  let callbackMeta: MariadbColumn[] | undefined;
  const result = await new Promise((resolve, reject) => {
    const returned = c.query(options, values, (error: Error | null, rows: unknown, meta?: MariadbColumn[]) => (error ? reject(error) : ((callbackMeta = meta), resolve(rows))));
    if (typeof returned?.then === "function") returned.then(resolve, reject);
  });
  if (!Array.isArray(result)) return [];
  // A DATETIME and a TIMESTAMP with `T`, and a SET's members joined as MySQL writes them: by the
  // module's kinds, else by the columns' metadata (mariadb 2 has no isSet()).
  const work: { name: string; date: boolean }[] = columns
    ? columns.filter(([, kind]) => kind === "text" || kind.split("|").includes("timestamp")).map(([name, kind]) => ({ name, date: kind !== "text" }))
    : (((result as { meta?: MariadbColumn[] }).meta ?? callbackMeta ?? []) as MariadbColumn[])
        .map((m) => ({ name: typeof m.name === "function" ? m.name() : m.name, type: m.type, set: (typeof m.isSet === "function" && m.isSet()) || m.type === "SET" }))
        .filter((m): m is { name: string; type: string | undefined; set: boolean } => m.name !== undefined && (m.set || m.type === "DATETIME" || m.type === "TIMESTAMP"))
        .map((m) => ({ name: m.name, date: !m.set }));
  // A truth value a boolean; an INT a temporary table may widen to a BIGINT a BigInt, as the BIGINT is.
  fixRows(result as Record<string, unknown>[], columnFixes(columns));
  if (!work.length) return result;
  for (const row of result as Record<string, unknown>[]) {
    for (const { name, date } of work) {
      const v = row[name];
      if (v === null || v === undefined) continue;
      if (Array.isArray(v)) row[name] = coerce("text", v, name);
      else if (date && typeof v === "string") row[name] = v.replace(" ", "T");
    }
  }
  return result;
}

/** The fixes the module's kinds call for, for a driver that tells each type by its value. */
function columnFixes(columns: Columns | undefined): (readonly [string, Fix])[] {
  const fixes: (readonly [string, Fix])[] = [];
  for (const [name, kind] of columns ?? []) {
    const fix = byKind(kind);
    if (fix) fixes.push([name, fix]);
  }
  return fixes;
}

// ---------------------------------------------------------------------------------------------
// PlanetScale 1.x

type PlanetscaleField = { type: string; charset?: number };
/** PlanetScale hands each value over as a string of bytes, one character each. */
const bytesOf = (s: string) => {
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
};

/** PlanetScale's own default cast (cast.js), with a BIGINT a BigInt and a DATETIME's and a TIMESTAMP's `T`: it replaces any cast of the app's for this query. */
function planetscaleCast(field: PlanetscaleField, value: string | null): unknown {
  if (value === null || value === undefined) return value;
  switch (field.type) {
    case "INT64":
    case "UINT64":
      return BigInt(value);
    case "DATE":
    case "TIME":
    case "DECIMAL":
      return value;
    case "DATETIME":
    case "TIMESTAMP":
      return value.replace(" ", "T");
    case "JSON":
      return JSON.parse(utf8(bytesOf(value)));
    case "INT8":
    case "INT16":
    case "INT24":
    case "INT32":
    case "UINT8":
    case "UINT16":
    case "UINT24":
    case "UINT32":
    case "YEAR":
      return parseInt(value, 10);
    case "FLOAT32":
    case "FLOAT64":
      return parseFloat(value);
  }
  if (field.charset === 63) return bytesOf(value);
  return value ? utf8(bytesOf(value)) : "";
}

/** PlanetScale's execute(): 1.x binds parameters; 2.0 takes none, and its options come second. */
export async function planetscale(m: AnyClient, text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  const options = { as: "object", cast: planetscaleCast };
  let result;
  try {
    result = await m.execute(text, values.length ? values : null, options);
  } catch (error) {
    if (!/^Query parameters are not supported/.test((error as Error)?.message)) throw error;
    if (values.length) throw new TypeError(`@planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint.`, { cause: error });
    result = await m.execute(text, options);
  }
  if (!Array.isArray(result?.rows)) return [];
  fixRows(result.rows, columnFixes(columns));
  return result.rows;
}
