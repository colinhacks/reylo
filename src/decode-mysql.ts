// Reylo decodes every value of its own MySQL queries itself, on the app's own pool, connection or
// transaction (the representation is src/values.ts). Each driver but
// Bun.sql takes its decoding choices per query, laid over the app's without changing them
// (wiki/driver-decoding/spike/mysql.md):
//
// - mysql2 stays on `execute`, the binary protocol, unless the server refuses prepared statements:
//   then Reylo writes the values into the text itself and runs `query` (below, `mysql2Query`). Its
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
import { lastOfEach, namesInResult, rowOf } from "./row.ts";
import { bigInteger, coerce, DecodeError, describe, truth, utf8, withFraction, type Columns, type Kind } from "./values.ts";
import { bunRows } from "./decode.ts";
import { readMysqlError } from "./query-error.ts";
import type { AnyClient as Client, Bound, Marks, QueryModule, Runtime } from "./execute.ts";
import type { SqlMode } from "./engine-core.ts";

type AnyClient = Record<string, any>;

// ---------------------------------------------------------------------------------------------
// mysql2

/**
 * Options mysql2 ORs with the app's (a BIGINT and a date as text) and the row's shape, which the query's own replace. Rows are read by position: mysql2's key is the name the executed statement's metadata gives, with a trailing space where MySQL keeps one (`select true` is `true `, as is `current_date` and an unaliased `case … end`), and building an object under the name `__proto__` fails in the driver and closes the connection.
 */
const MYSQL2_OPTIONS = { supportBigNumbers: true, bigNumberStrings: true, dateStrings: true, rowsAsArray: true, nestTables: false };

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

/**
 * Reylo's typeCast for the text protocol, where every value is its text: a DOUBLE and a FLOAT read
 * by Number(), since mysql2 reads a DOUBLE's text digit by digit and misses the nearest double for
 * some of them, and a VECTOR as its bytes, since mysql2's text parser reads a NULL VECTOR as [].
 */
function mysql2TextCast(f: Mysql2Field, next: () => unknown): unknown {
  if (f.type === "DOUBLE" || f.type === "FLOAT") {
    const s = f.string("ascii");
    return s === null ? null : Number(s);
  }
  if (f.type === "VECTOR") return f.buffer();
  return mysql2Cast(f, next);
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
  const text = withFraction(s, decimals);
  return type === TIME ? text : text.replace(" ", "T");
}

/** The app's settings, where mysql2 shows them: a promise Pool's, Connection's or PoolConnection's. */
type Mysql2Config = { typeCast?: unknown; decimalNumbers?: boolean; jsonStrings?: boolean; queryFormat?: unknown; charsetNumber?: number };
const configOf = (m: AnyClient): Mysql2Config | undefined => m.pool?.config?.connectionConfig ?? m.connection?.config;

/**
 * Whether the statement needs Reylo's typeCast, which mysql2 calls for every value of the binary
 * protocol (about 70 ns each): for a JSON or GEOMETRY column, whenever the columns are not known,
 * and where the app's settings, which a query cannot turn off, would decide a DECIMAL or a JSON value
 * or replace mysql2's own reading (or cannot be read).
 */
function needsCast(m: AnyClient, columns: Columns | undefined): boolean {
  if (!columns || columns.some(([, kind]) => kind === "unknown" || kind.split("|").some((k) => k === "geometry" || k === "json" || k.startsWith("json")))) return true;
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
// A SET is a Set of its members: mysql2 and PlanetScale give its text, mariadb an array.
const toSet: Fix = (v, name) => coerce("set", v, name);
// An ENUM holds one of its labels, or the `''` MySQL stores for any other value, which is refused.
const toEnum: Fix = (v, name) => coerce("enum", v, name);

/**
 * The fix a column of `kind` needs, if any: `bool` a boolean, `int8` (a BIGINT, or an integer a
 * temporary table may widen to one) a BigInt from whatever the driver gave, `float[]` (a VECTOR)
 * its floats, `set` a Set of its members, `enum` its label.
 */
function byKind(kind: Kind | undefined): Fix | undefined {
  if (kind === "bool") return toBool;
  if (kind === "int8") return toBigInt;
  if (kind === "float[]") return toFloats;
  if (kind === "set") return toSet;
  if (kind === "enum") return toEnum;
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

// ---------------------------------------------------------------------------------------------
// mysql2 through query(), where the server refuses prepared statements (Cloudflare Hyperdrive:
// "Hyperdrive does not currently support MySQL COM_STMT_PREPARE messages"). Reylo writes each
// value into the statement itself, at the module's marks, the places the engine found its
// placeholders, and sends the text with no values, so mysql2's escaping and placeholder scan
// decide nothing. A place is a placeholder only for a session that reads the text's strings as the
// build did: under NO_BACKSLASH_ESCAPES a backslash escapes nothing, and under ANSI_QUOTES a
// double-quoted word is a name, so where the statement's reading turns on either, the module
// carries the build's mode, and a session whose own differs is refused rather than given a value
// at a place it reads as inside a string. A string value is quoted with each quote doubled, which
// every sql_mode reads the same way, and a backslash in it is escaped as the session reads it.
// The session's mode is read on the connection the statement then runs on, rather than from the
// server's NO_BACKSLASH_ESCAPES status flag, which can be stale after a stored routine changes
// sql_mode. A value holding a character outside ASCII is refused on a connection whose charset is
// not UTF-8, and so is the app's queryFormat, which would rewrite the statement.

/** The pools and connections whose server refused a prepared statement: their statements go through query() from then on. */
const refusesPrepares = new WeakSet<object>();

/** The client a refusal is remembered on: the pool or connection under a promise wrapper, which mysql2's callback API makes anew for every call. */
const preparesKey = (m: AnyClient): object => m.pool ?? m.connection ?? m;

const isPrepareRefusal = (e: unknown) => e instanceof Error && /COM_STMT_PREPARE/.test(e.message);

const hexOf = (bytes: Uint8Array) => {
  let hex = "";
  for (const byte of bytes) hex += (byte < 16 ? "0" : "") + byte.toString(16);
  return hex;
};

/**
 * The collations of utf8mb4 and utf8mb3 that every mysql2 from 2.3 on encodes as UTF-8
 * (charset_encodings.js). UTF-8 writes a character outside ASCII with no ASCII byte; another
 * encoding can write one with a quote's or a backslash's byte (SJIS writes `¥` as 0x5c, and
 * Node's latin1 and ascii keep a character's low byte, so `Ŝ` is 0x5c and `ħ` is 0x27), which the
 * server would read as that character after Reylo had escaped the value. Every client charset
 * writes ASCII the same way.
 */
const UTF8_COLLATIONS: readonly (readonly [number, number])[] = [[33, 33], [45, 46], [76, 76], [83, 83], [192, 215], [223, 247], [255, 271], [273, 275], [277, 294], [296, 298], [300, 300], [303, 305]];
const isUtf8 = (charsetNumber: number) => UTF8_COLLATIONS.some(([from, to]) => charsetNumber >= from && charsetNumber <= to);

/** A string literal for a session that reads a backslash as an escape (`escapes`) or as itself. */
function stringLiteral(s: string, escapes: boolean): string {
  const quoted = s.replace(/'/g, "''");
  return `'${escapes ? quoted.replace(/\\/g, "\\\\") : quoted}'`;
}

/** The text a value is written as a string literal with, where it is one. */
const textOf = (v: unknown): string | undefined => (typeof v === "string" ? v : v !== null && typeof v === "object" && !(v instanceof Uint8Array) && !(v instanceof Date) ? JSON.stringify(v) : undefined);

/** A value as the literal the server reads as what execute() binds for it: a number that is no safe integer as a DOUBLE (`1.5e0`), never the DECIMAL `1.5` is. */
function mysqlLiteral(v: unknown, m: AnyClient, escapes: boolean): string {
  if (v === null || v === undefined) return "NULL";
  switch (typeof v) {
    case "boolean":
      return v ? "TRUE" : "FALSE";
    case "bigint":
      return String(v);
    case "number": {
      if (!Number.isFinite(v)) throw new TypeError(`reylo cannot write ${v} into a MySQL statement, which it runs through mysql2's query() on a server that refuses prepared statements: MySQL has no such number.`);
      if (Object.is(v, -0)) return "-0e0";
      if (Number.isSafeInteger(v)) return String(v);
      const s = String(v);
      return s.includes("e") ? s : `${s}e0`;
    }
    case "string":
      return stringLiteral(v, escapes);
  }
  // `_binary`, since MySQL reads two bare hex literals in a bitwise operator as numbers.
  if (v instanceof Uint8Array) return `_binary X'${hexOf(v)}'`;
  // A Date in the connection's timezone, as execute() binds one; its text holds no quote or backslash.
  if (v instanceof Date) return m.escape(v);
  return stringLiteral(JSON.stringify(v), escapes);
}

/** `text` with the placeholder at each mark replaced by its value's literal, for a session that reads a backslash in a string as an escape (`escapes`) or as itself. */
function inlineMysql(text: string, marks: Marks, values: unknown[], m: AnyClient, escapes: boolean): string {
  if (marks.length !== values.length) throw new TypeError(`reylo has ${marks.length} placeholders for ${values.length} values in this MySQL statement.`);
  let out = "";
  let at = 0;
  marks.forEach(([start, end], i) => {
    if (text.slice(start, end) !== "?") throw new TypeError(`reylo cannot write a value into this MySQL statement at ${start}, which holds ${JSON.stringify(text.slice(start, end))}, not a placeholder ?.`);
    out += text.slice(at, start) + mysqlLiteral(values[i], m, escapes);
    at = end;
  });
  return out + text.slice(at);
}

/** A session's sql_mode as the two flags that decide how it reads text. */
function readsText(sqlMode: string): SqlMode {
  const flags = sqlMode.split(",");
  return { backslashEscapes: !flags.includes("NO_BACKSLASH_ESCAPES"), ansiQuotes: flags.includes("ANSI_QUOTES") };
}

/** What a sql_mode has of the two flags that decide how it reads text. */
function modeWords(mode: SqlMode): string {
  const set = [...(mode.backslashEscapes ? [] : ["NO_BACKSLASH_ESCAPES"]), ...(mode.ansiQuotes ? ["ANSI_QUOTES"] : [])];
  const unset = [...(mode.backslashEscapes ? ["NO_BACKSLASH_ESCAPES"] : []), ...(mode.ansiQuotes ? [] : ["ANSI_QUOTES"])];
  return set.length === 2 ? `has ${set.join(" and ")}` : set.length === 0 ? `has neither ${unset.join(" nor ")}` : `has ${set[0]} and not ${unset[0]}`;
}

/** Field names mysql2's compiled parsers refuse (helpers.js, privateObjectProps), which a string written into the text names an unaliased column. */
const PRIVATE_NAMES = new Set(["__proto__", "__defineGetter__", "__defineSetter__", "__lookupGetter__", "__lookupSetter__"]);

/**
 * The statement through query(), its values written in at its marks. Where the module carries the
 * sql_mode it was built under, or a value holds a backslash, the session's sql_mode is read first,
 * on the connection the statement then runs on.
 */
async function mysql2Query(m: AnyClient, text: string, marks: Marks | undefined, built: SqlMode | undefined, values: unknown[], columns: Columns | undefined): Promise<[unknown, unknown]> {
  if (marks === undefined) throw new TypeError("reylo cannot run this mysql statement through mysql2 here: this server, as Cloudflare Hyperdrive does, refuses prepared statements, so Reylo writes the values into the statement, and a query defined by hand needs `marks` there, the place of each placeholder in its text, which a build step writes.");
  const config = configOf(m);
  if (typeof config?.queryFormat === "function") throw new Error("reylo cannot run a mysql statement through mysql2 here: this server, as Cloudflare Hyperdrive does, refuses prepared statements, so Reylo runs mysql2's query(), which passes every statement through the app's queryFormat. Connect without a queryFormat, or with the mariadb driver.");
  if (config?.charsetNumber !== undefined && !isUtf8(config.charsetNumber) && values.some((v) => /[^\x00-\x7f]/.test(textOf(v) ?? ""))) throw new Error("reylo cannot run a mysql statement through mysql2 here: this server, as Cloudflare Hyperdrive does, refuses prepared statements, so Reylo writes the values into the statement, and this connection's charset is not UTF-8, so a character outside ASCII in a value could reach the server as a quote or a backslash (as SJIS writes ¥). Connect with utf8mb4, or with the mariadb driver.");
  const proto = columns?.some(([name]) => name === "__proto__") || values.some((v) => typeof v === "string" && PRIVATE_NAMES.has(v)) ? { disableEval: true } : {};
  const options = { ...MYSQL2_OPTIONS, ...proto, namedPlaceholders: false, typeCast: mysql2TextCast };
  // Without a backslash in a value, every session writes the values alike.
  if (built === undefined && !values.some((v) => textOf(v)?.includes("\\"))) return await m.query({ sql: inlineMysql(text, marks, values, m, true), ...options });
  // A promise Pool hands out a PoolConnection, so that the probe and the statement share one session.
  const pooled = typeof m.getConnection === "function" && typeof m.release !== "function" ? await m.getConnection() : undefined;
  const conn = pooled ?? m;
  try {
    const [rows] = await conn.query({ sql: "select @@session.sql_mode", rowsAsArray: true, nestTables: false, namedPlaceholders: false, typeCast: true });
    const mode = readsText(String(rows[0][0]));
    if (built !== undefined && (mode.backslashEscapes !== built.backslashEscapes || mode.ansiQuotes !== built.ansiQuotes)) {
      throw new Error(`reylo cannot run this mysql statement through mysql2 here: this server, as Cloudflare Hyperdrive does, refuses prepared statements, so Reylo writes the values into the statement at its placeholders, which the build found as a session reads them whose sql_mode ${modeWords(built)}, and this session's sql_mode ${modeWords(mode)}, so it reads the statement's strings otherwise. Set the session's sql_mode as the build's was, or build against a server whose sql_mode is this one's.`);
    }
    return await conn.query({ sql: inlineMysql(text, marks, values, m, mode.backslashEscapes), ...options });
  } finally {
    pooled?.release();
  }
}

/** mysql2's execute(), resolving to [rows, fields], or its query() where the server refuses prepared statements. */
export async function mysql2(m: AnyClient, text: string, values: unknown[], columns: Columns | undefined, marks?: Marks, built?: SqlMode): Promise<unknown[]> {
  // mysql2's compiled parser refuses a field named `__proto__` even for an array row, and its interpreted one (`disableEval`, from 3.15) reads it.
  const proto = columns?.some(([name]) => name === "__proto__") ? { disableEval: true } : {};
  let answer: [unknown, unknown];
  if (refusesPrepares.has(preparesKey(m))) {
    answer = await mysql2Query(m, text, marks, built, values, columns);
  } else {
    try {
      answer = await m.execute({ sql: text, ...MYSQL2_OPTIONS, ...proto, typeCast: needsCast(m, columns) ? mysql2Cast : true }, values);
    } catch (e) {
      if (!isPrepareRefusal(e)) throw e;
      refusesPrepares.add(preparesKey(m));
      answer = await mysql2Query(m, text, marks, built, values, columns);
    }
  }
  const [result, fields] = answer;
  if (!Array.isArray(result)) return [];
  if (!result.length) return result;
  const described = (fields ?? []) as Mysql2Column[];
  const cols = resultColumns(columns, described.map((f) => f.name), result[0]);
  const rows = asObjects(result, cols);
  // A row keeps the last value of each name, so that column's fixes alone apply to it.
  const kept = new Set(lastOfEach(cols.map(([name], i) => [name, i] as const)).map(([, i]) => i));
  const fixes: (readonly [string, Fix])[] = [];
  // The module's kind; a BIGINT by the result's own type, which says it of an either too (`:n + 1`,
  // a BIGINT or a DOUBLE by what is bound).
  described.forEach((f, i) => {
    const fix = byKind(cols[i]![1]) ?? (f.columnType === LONGLONG ? toBigInt : undefined);
    if (fix && kept.has(i)) fixes.push([cols[i]![0], fix]);
  });
  // A DATETIME's, TIMESTAMP's and TIME's text, padded to the column's own fraction digits.
  described.forEach((f, i) => {
    if (kept.has(i) && (f.columnType === DATETIME || f.columnType === TIMESTAMP || (f.columnType === TIME && f.decimals > 0))) {
      const decimals = f.decimals > 6 ? 0 : f.decimals, type = f.columnType;
      fixes.push([cols[i]![0], (v) => (typeof v === "string" ? wallClock(v, decimals, type) : v)]);
    }
  });
  fixRows(rows, fixes);
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
  // By position where the module declares the columns: mariadb writes each value into the statement's text, so MySQL names a column with no alias after it (`select :x::int + 1` is `CAST(5 AS SIGNED) + 1`), and two of them may share a name.
  const options = { sql: text, ...MARIADB_OPTIONS, rowsAsArray: Boolean(columns?.length), ...(columns?.some(([, kind]) => kind === "unknown" || kind.split("|").includes("geometry")) ? { typeCast: mariadbGeometry } : {}) };
  // The promise API resolves to the rows with their `meta`; the callback API hands the meta over as a third argument.
  let callbackMeta: MariadbColumn[] | undefined;
  const result = await new Promise((resolve, reject) => {
    const returned = c.query(options, values, (error: Error | null, rows: unknown, meta?: MariadbColumn[]) => (error ? reject(error) : ((callbackMeta = meta), resolve(rows))));
    if (typeof returned?.then === "function") returned.then(resolve, reject);
  });
  if (!Array.isArray(result)) return [];
  if (!result.length) return result;
  const meta = () => ((result as { meta?: MariadbColumn[] }).meta ?? callbackMeta ?? []) as MariadbColumn[];
  // The module's columns as the result holds them, the last of each name: a row keeps that one's value.
  const cols = columns && Array.isArray(result[0]) ? resultColumns(columns, meta().map((m) => (typeof m.name === "function" ? m.name() : m.name) as string), result[0]) : (columns ?? []).map(([name, kind]) => [name, kind] as const);
  const rows = asObjects(result, cols);
  const own = lastOfEach(cols);
  // A DATETIME and a TIMESTAMP with `T`, and a SET's members joined as MySQL writes them: by the
  // module's kinds, else by the columns' metadata (mariadb 2 has no isSet()).
  const work: { name: string; date: boolean }[] = columns
    ? own.filter(([, kind]) => kind === "text" || kind?.split("|").includes("timestamp")).map(([name, kind]) => ({ name, date: kind !== "text" }))
    : meta()
        .map((m) => ({ name: typeof m.name === "function" ? m.name() : m.name, type: m.type, set: (typeof m.isSet === "function" && m.isSet()) || m.type === "SET" }))
        .filter((m): m is { name: string; type: string | undefined; set: boolean } => m.name !== undefined && (m.set || m.type === "DATETIME" || m.type === "TIMESTAMP"))
        .map((m) => ({ name: m.name, date: !m.set }));
  // A truth value a boolean; an INT a temporary table may widen to a BIGINT a BigInt, as the BIGINT is.
  fixRows(rows, kindFixes(own));
  if (!work.length) return rows;
  for (const row of rows) {
    for (const { name, date } of work) {
      const v = row[name];
      if (v === null || v === undefined) continue;
      if (Array.isArray(v)) row[name] = coerce("text", v, name);
      else if (date && typeof v === "string") row[name] = v.replace(" ", "T");
    }
  }
  return rows;
}

/** A result's column as the module declares it: its name, and its kind where the module has one of that column. */
type Declared = readonly [name: string, kind: Kind | undefined];

/**
 * A result's columns as the module declares them, in the result's order (`namesInResult`): the
 * module's own by position where the result is still theirs, else each of the result's by its
 * name, with the kind of the module's last column of that name, which is the one a row keeps.
 * `first` is the result's first row: where it is an array the result's names do not account for
 * (mariadb's callback API hands some rows over with no metadata), the module's stand by position.
 */
function resultColumns(columns: Columns | undefined, result: string[], first: unknown): Declared[] {
  if (!columns) return result.map((name) => [name, undefined] as const);
  const declared = columns.map(([name]) => name);
  const width = Array.isArray(first) ? first.length : result.length;
  if (columns.length === width && (result.length !== width || namesInResult(declared, result) === declared)) return columns.map(([name, kind]) => [name, kind] as const);
  const kinds = new Map(columns.map(([name, kind]) => [name, kind] as const));
  return result.map((name) => [name, kinds.get(name)] as const);
}

/** Rows a driver gave as arrays, as objects under the columns' names; a client that took no such option gives objects already, kept as they are. */
function asObjects(rows: unknown[], cols: readonly Declared[]): Record<string, unknown>[] {
  if (!Array.isArray(rows[0])) return rows as Record<string, unknown>[];
  return (rows as unknown[][]).map(rowOf(cols.map(([name]) => name)));
}

/** The fixes the columns' kinds call for, for a driver that tells each type by its value. */
function kindFixes(cols: readonly Declared[]): (readonly [string, Fix])[] {
  const fixes: (readonly [string, Fix])[] = [];
  for (const [name, kind] of cols) {
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
  // By position where the module declares the columns: the driver writes each value into the statement's text, so MySQL names a column with no alias after it, as on mariadb, and Vitess in front of a PlanetScale database writes such a name its own way.
  const options = { as: columns?.length ? "array" : "object", cast: planetscaleCast };
  let result;
  try {
    result = await m.execute(text, values.length ? values : null, options);
  } catch (error) {
    if (!/^Query parameters are not supported/.test((error as Error)?.message)) throw error;
    if (values.length) throw new TypeError(`@planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint.`, { cause: error });
    result = await m.execute(text, options);
  }
  // PlanetScale's result always carries its rows, an empty array for a statement that returns
  // none. Another driver's execute() (TiDB Cloud's answers with the rows themselves) would read
  // as no rows at all.
  if (!Array.isArray(result?.rows)) throw new TypeError(`reylo ran this statement through execute(), as on PlanetScale's @planetscale/database, and the client answered with ${Array.isArray(result) ? "an array" : result === null || typeof result !== "object" ? String(result) : "an object"} rather than a result with rows. It takes execute(text, values) as on mysql2 and PlanetScale, or a Drizzle database over one of them.`);
  if (!columns?.length || !result.rows.length) return result.rows;
  const cols = Array.isArray(result.rows[0]) ? resultColumns(columns, ((result.fields ?? []) as { name: string }[]).map((f) => f.name), result.rows[0]) : columns.map(([name, kind]) => [name, kind] as const);
  const rows = asObjects(result.rows, cols);
  fixRows(rows, kindFixes(lastOfEach(cols)));
  return rows;
}

// A binary parameter is typed Uint8Array, and every driver binds one as it is (pg-promise too,
// since it gets { text, values }) but two (lab/driver-matrix, S1c). mariadb writes any value
// that is not a Buffer as JSON, so a plain Uint8Array (TextEncoder's, getRandomValues') arrived
// as the text {"0":1,"1":2,"2":255}: it gets a Buffer over the same memory, where a Buffer
// exists (not in every Worker). Neon's neon() before 0.10 is the other (src/execute.ts, `asByteaHex`).
function asBuffer(v: unknown) {
  // Named structurally, so that a project without Node's types can type-check the module.
  const NodeBuffer = (globalThis as { Buffer?: { isBuffer(v: unknown): boolean; from(buffer: ArrayBufferLike, offset: number, length: number): Uint8Array } }).Buffer;
  return v instanceof Uint8Array && NodeBuffer !== undefined && !NodeBuffer.isBuffer(v) ? NodeBuffer.from(v.buffer, v.byteOffset, v.byteLength) : v;
}

/** A MySQL statement's rows on each MySQL driver's client, or undefined on any other client. */
async function runMysql(c: Client, query: QueryModule<"mysql">, { text, values, marks }: Bound): Promise<unknown[] | undefined> {
  if (typeof c.unsafe === "function") return bunRows(await c.unsafe(text, values), query.columns);
  // mariadb, checked before execute() because it has one too (returning rows, not a pair).
  // Its callback API (mariadb/callback) answers through the callback and returns a command
  // object; its promise API returns a promise and ignores the callback.
  if (typeof c.batch === "function" && typeof c.query === "function") return await mariadb(c, text, values.map(asBuffer), query.columns);
  // mysql2's callback API: the promise() wrapper runs on the same connection.
  const m = typeof c.promise === "function" ? c.promise() : c;
  if (typeof m.execute === "function") {
    // mysql2 has query() beside execute(); PlanetScale's Connection, Client and transaction do not.
    return typeof m.query === "function" ? await mysql2(m, text, values, query.columns, marks, query.sqlMode) : await planetscale(m, text, values, query.columns);
  }
  return undefined;
}

/** A column's raw value as its kind, from whatever form the driver read, as a Standard Query's `decode` takes it (src/standard-query.ts). */
function mysqlDecoder(name: string, kind: Kind, delim: string | undefined): (v: unknown) => unknown {
  // A MySQL DATETIME's text takes a T between the date and the time, as `.run()` writes it, a
  // column that may be one included (`mariadb`).
  if (kind.split("|").includes("timestamp")) return (v) => (typeof v === "string" ? v.replace(" ", "T") : coerce(kind, v, name, delim));
  // A MySQL GEOMETRY's raw value is its bytes, as `.run()` gives it. An object is a driver's own
  // reading of one (mysql2's `{ x, y }`, mariadb's GeoJSON), which the bytes cannot be had back from.
  if (kind.split("|").includes("geometry"))
    return (v) => {
      if (typeof v === "object" && v !== null && !(v instanceof Uint8Array)) throw new DecodeError(`Column "${name}" holds ${describe(v)}, which is no raw MySQL value: a GEOMETRY's raw value is its bytes, which a driver parses into an object unless its conversions are off (mysql2's typeCast returning \`field.buffer()\`).`);
      return coerce(kind, v, name, delim);
    };
  return (v) => coerce(kind, v, name, delim);
}

/** MySQL's half of the run time (src/execute.ts, `Runtime`), which `defineMysqlQuery` passes. */
export const mysql: Runtime<"mysql"> = { dialect: "mysql", run: runMysql, readError: readMysqlError, decoder: mysqlDecoder };
