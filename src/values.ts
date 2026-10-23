// How Reylo represents a SQL value in JavaScript: one table, which the type renderer
// (src/render.ts), the JSON Schema writer (src/schema.ts) and the run time's decoders
// (src/decode.ts, src/decode-mysql.ts, src/decode-sqlite.ts) all read, so that the type a module
// declares and the value `run` returns cannot drift apart. Reylo sets
// every value's type itself, on every driver, whatever the app configured its driver to return:
// one representation per SQL type.
//
// A value's representation is its kind, which the build writes into the module for each output
// column (`columns`, as `[name, kind]`): `bool` is a boolean; `int` and `float` a number; `int8` a
// BigInt; `decimal` the exact decimal text; `date` the text `YYYY-MM-DD`; `timestamp` the server's
// wall-clock text with `T` between the date and the time; `timestamptz` the instant's ISO 8601 text
// in UTC, with `Z` and the server's fraction; `json` the parsed value; `bytes` a Uint8Array (a
// Buffer is one); `text` the server's text, which every other type is (time, interval, uuid, an
// enum's label, a range, a row value); `unknown` whatever the driver gives (a SQLite column with no
// declared type). `k[]` is an array of `k`, a NULL element `null`, and `a|b` either (a MySQL
// expression whose type the bound value decides). No kind is a JavaScript Date.
//
// This module runs in every generated module's process, so it names no Node type: a Buffer is
// reached through `globalThis` where one exists.
import type { EngineType } from "./engine-type.ts";

export type Dialect = "postgres" | "mysql" | "sqlite";

/** A scalar value's representation. */
export type ScalarKind = "bool" | "int" | "float" | "int8" | "decimal" | "text" | "date" | "timestamp" | "timestamptz" | "json" | "bytes" | "unknown";
/** A column's kind as a module carries it: a scalar kind, `k[]` for an array of `k`, `a|b` for either. */
export type Kind = string;
/** The output columns a module carries, in the statement's order. */
export type Columns = readonly (readonly [name: string, kind: Kind])[];

/** The TypeScript type of each scalar kind: what a value of that kind is at run time. */
export const TS_TYPE: Readonly<Record<ScalarKind, string>> = {
  bool: "boolean",
  int: "number",
  float: "number",
  int8: "bigint",
  decimal: "string",
  text: "string",
  date: "string",
  timestamp: "string",
  timestamptz: "string",
  json: "unknown",
  bytes: "Uint8Array",
  unknown: "unknown",
};

/**
 * The kind of a Postgres or MySQL base type, by the engine's name for it (MySQL's are the Postgres
 * names: a DATETIME is `timestamp`, a BIT `bytea`, a YEAR `int2`). Every type not named here is
 * its text: Reylo owns every oid, so the driver never parses one (an enum's label, a range, `time`,
 * `interval`, `uuid`, `money`, `point`, `xml`, a row value).
 */
const SQL_KINDS: Readonly<Record<string, ScalarKind>> = {
  bool: "bool",
  int2: "int",
  int4: "int",
  oid: "int",
  float4: "float",
  float8: "float",
  int8: "int8",
  numeric: "decimal",
  date: "date",
  timestamp: "timestamp",
  timestamptz: "timestamptz",
  json: "json",
  jsonb: "json",
  bytea: "bytes",
};

/**
 * SQLite's: the engine names a column by its affinity, and a column declared BIGINT, INT8 or
 * UNSIGNED BIG INT `int8`, since the declared width decides on every database.
 * An INTEGER is a number, exact or an error; an `int8` a BigInt on every SQLite
 * driver (src/decode-sqlite.ts reads either); a NUMERIC holds whichever storage class the row
 * stored; `bool` is a truth value or a column declared BOOLEAN; a column with no declared type,
 * or ANY in a STRICT table, is whatever the driver gives.
 */
const SQLITE_KINDS: Readonly<Record<string, Kind>> = {
  integer: "int",
  int8: "int8",
  real: "float",
  text: "text",
  blob: "bytes",
  numeric: "int|float|text",
  bool: "bool",
};

/**
 * The kind of a base type by its engine name, `a|b` naming either. An integer that may be a BIGINT
 * or a narrower integer (a MySQL expression a temporary table may widen, `int8|int4`) is a BIGINT's,
 * a BigInt either way, where no float shares the column: there a number may be either.
 */
export function scalarKind(name: string, dialect: Dialect): Kind {
  const kinds = [...new Set(name.split("|").flatMap((part) => (dialect === "sqlite" ? (SQLITE_KINDS[part] ?? "unknown") : (SQL_KINDS[part] ?? "text")).split("|")))];
  const wide = kinds.includes("int8") && !kinds.includes("float") ? kinds.filter((k) => k !== "int") : kinds;
  return wide.join("|");
}

/** The kind of a value of the engine's type `t`, standing in a result row (not inside a JSON document). */
export function columnKind(t: EngineType, dialect: Dialect): Kind {
  switch (t.kind) {
    case "scalar":
      return scalarKind(t.name, dialect);
    case "enum":
      return "text";
    case "array":
      // A row value's array is the text Postgres prints, as a row value is.
      if ((t.of.kind === "scalar" && t.of.name === "record") || t.of.kind === "composite") return "text";
      return `${columnKind(t.of, dialect)}[]`;
    case "object":
    case "list":
    case "json":
    case "rows":
      return "json";
    case "composite":
      return "text";
    case "unknown":
      return "unknown";
  }
}

/** The TypeScript type of a scalar kind, `a|b` as the union of both; `unknown` where any part is. */
export function kindType(kind: Kind): string {
  const parts = kind.split("|").map((k) => TS_TYPE[k as ScalarKind] ?? "unknown");
  return parts.includes("unknown") ? "unknown" : [...new Set(parts)].join(" | ");
}

// ---------------------------------------------------------------------------------------------
// The value of each kind from the server's text, as every Postgres driver but Bun.sql hands it
// over (pg, pg-promise, Neon, PGlite, and postgres.js through `.raw()`), at the server's default
// DateStyle (ISO), IntervalStyle and bytea_output, which the drivers' own parsers read too.

export type Decoder = (text: string) => unknown;

const identity: Decoder = (x) => x;
const number: Decoder = (x) => +x;

/** `2026-10-20 10:00:00.123456` → `2026-10-20T10:00:00.123456`; `infinity` and a BC year's suffix as written. */
const timestamp: Decoder = (x) => x.replace(" ", "T");

// timestamptz in DateStyle ISO: `2026-10-20 03:03:00.123456-07`, offsets with seconds before
// standard time (`-07:52:58`), years past 9999, ` BC`. `new Date(text)`, which postgres.js and
// PGlite use, returns Invalid Date for the last three and reads year 0001 as 2001
// (wiki/driver-decoding/spike/postgres.md); and a Date holds milliseconds, not the server's
// microseconds. So Reylo moves the wall-clock time to UTC itself, in whole seconds on a day count,
// and keeps the fraction as the server wrote it.
const TSTZ = /^(\d{4,})-(\d\d)-(\d\d) (\d\d):(\d\d):(\d\d)(\.\d{1,6})?([+-])(\d\d)(?::(\d\d))?(?::(\d\d))?( BC)?$/;

/** Days from 1970-01-01 to a date of the proleptic Gregorian calendar, year 0 being 1 BC (Howard Hinnant's days_from_civil). */
function daysFromCivil(y: number, m: number, d: number): number {
  y -= m <= 2 ? 1 : 0;
  const era = Math.floor(y / 400);
  const yoe = y - era * 400;
  const doy = Math.floor((153 * (m + (m > 2 ? -3 : 9)) + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe - 719468;
}

/** The date `days` after 1970-01-01, the inverse of `daysFromCivil`. */
function civilFromDays(days: number): [number, number, number] {
  const z = days + 719468;
  const era = Math.floor(z / 146097);
  const doe = z - era * 146097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const d = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const m = mp + (mp < 10 ? 3 : -9);
  return [era * 400 + yoe + (m <= 2 ? 1 : 0), m, d];
}

const pad = (n: number, width = 2) => String(n).padStart(width, "0");

/**
 * An ISO 8601 year: four digits from 0000 (1 BC) to 9999, and the extended form, a sign and six
 * digits, outside them (`+010000`, `-000001` for 2 BC), as `Temporal.Instant` reads and writes it.
 */
const isoYear = (y: number) => (y >= 0 && y <= 9999 ? pad(y, 4) : `${y < 0 ? "-" : "+"}${pad(Math.abs(y), 6)}`);

/**
 * A date or time parameter's text as Postgres reads it, the inverse of `isoYear` for what `run`
 * returns (the round-trip rule): Postgres 18 refuses an ISO 8601 extended year (`+010000-01-01`,
 * "time zone displacement out of range") and the year 0000 ("date/time field value out of range"),
 * and reads `10000-01-01` and a year before 1 as its own ` BC` form, year 1 − n, so `-000001` is
 * `0002 … BC` and `0000` or `+000000` is `0001 … BC`. Any other text is left as it is.
 */
export function postgresYear(text: string): string {
  const m = /^([+-]\d{6}|0000)(-.*)$/.exec(text);
  if (m === null || /\sbc$/i.test(text)) return text;
  const year = +m[1]!;
  return year > 0 ? `${pad(year, 4)}${m[2]}` : `${pad(1 - year, 4)}${m[2]} BC`;
}

/**
 * A timestamptz's text as the instant's ISO 8601 text in UTC: `2026-10-20 10:00:00.123456+02` is
 * `2026-10-20T08:00:00.123456Z`, the fraction as the server wrote it (trailing zeros dropped, none
 * for a whole second), which `Temporal.Instant.from` reads. `infinity` and `-infinity` stay those
 * words.
 */
export function instant(x: string): string {
  const m = TSTZ.exec(x);
  if (m === null) return x;
  const year = m[12] === undefined ? +m[1]! : 1 - +m[1]!;
  const offset = (m[8] === "-" ? -1 : 1) * (+m[9]! * 3600 + (m[10] === undefined ? 0 : +m[10] * 60) + (m[11] === undefined ? 0 : +m[11]));
  const seconds = daysFromCivil(year, +m[2]!, +m[3]!) * 86400 + +m[4]! * 3600 + +m[5]! * 60 + +m[6]! - offset;
  const days = Math.floor(seconds / 86400);
  const s = seconds - days * 86400;
  const [y, mo, d] = civilFromDays(days);
  return `${isoYear(y)}-${pad(mo)}-${pad(d)}T${pad(Math.floor(s / 3600))}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}${m[7] ?? ""}Z`;
}

/**
 * The instant a driver already decoded to a Date (Bun.sql's timestamptz), as the same text: to the
 * millisecond, which is all a Date holds, with its fraction's trailing zeros dropped as the server
 * drops them, so that a value the driver did not cut reads alike on every driver.
 */
export function instantOfDate(d: Date, column: string): string {
  validDate(d, column);
  return d.toISOString().replace(/\.?0+Z$/, "Z");
}

type BufferClass = { from(data: string, encoding: string): Uint8Array; isBuffer(v: unknown): boolean };
const NodeBuffer = (globalThis as { Buffer?: BufferClass }).Buffer;
// TextDecoder is a global in every runtime Reylo supports; it is typed here rather than through
// the DOM lib or Node's, which a project that type-checks Reylo's sources need not have.
const Utf8 = (globalThis as unknown as { TextDecoder: new (label?: string) => { decode(bytes: Uint8Array): string } }).TextDecoder;
const utf8Decoder = new Utf8("utf-8");

/** UTF-8 bytes as text: a Buffer's own toString where the bytes are one. */
export function utf8(bytes: Uint8Array): string {
  return NodeBuffer?.isBuffer(bytes) ? (bytes as Uint8Array & { toString(encoding: string): string }).toString("utf8") : utf8Decoder.decode(bytes);
}
const fromHex = (Uint8Array as unknown as { fromHex?: (hex: string) => Uint8Array }).fromHex;

/** bytea's text: hex (`\x0102ff`, the server's default) or the escape format (`bytea_output = escape`). */
export function bytea(x: string): Uint8Array {
  if (x.charCodeAt(0) === 92 && x.charCodeAt(1) === 120) {
    const hex = x.slice(2);
    if (NodeBuffer) return NodeBuffer.from(hex, "hex");
    if (fromHex) return fromHex(hex);
    const out = new Uint8Array(hex.length / 2);
    for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(2 * i, 2 * i + 2), 16);
    return out;
  }
  const out: number[] = [];
  for (let i = 0; i < x.length; i++) {
    if (x[i] !== "\\") out.push(x.charCodeAt(i));
    else if (x[i + 1] === "\\") out.push(92), i++;
    else out.push(parseInt(x.slice(i + 1, i + 4), 8)), (i += 3);
  }
  return Uint8Array.from(out);
}

const TEXT_DECODERS: Readonly<Record<ScalarKind, Decoder>> = {
  bool: (x) => x === "t",
  int: number,
  float: number,
  int8: (x) => BigInt(x),
  decimal: identity,
  text: identity,
  date: identity,
  timestamp,
  timestamptz: instant,
  json: (x) => JSON.parse(x),
  bytes: bytea,
  unknown: identity,
};

/**
 * Postgres's array output: `{a,"b,c",NULL,{d}}`, optionally after its bounds (`[0:1]={...}`).
 * An element is quoted when it holds the delimiter, a brace, a quote, a backslash or white space,
 * or is empty or the word NULL; inside quotes a backslash escapes the next character.
 */
export function parseArray(text: string, element: (text: string) => unknown, delim = ","): unknown[] {
  const n = text.length;
  let i = text[0] === "[" ? text.indexOf("=") + 1 : 0;
  const level = (): unknown[] => {
    const out: unknown[] = [];
    i++;
    if (text[i] === "}") return i++, out;
    while (i < n) {
      if (text[i] === "{") out.push(level());
      else if (text[i] === '"') {
        let s = "";
        let from = ++i;
        while (i < n && text[i] !== '"') {
          if (text[i] === "\\") (s += text.slice(from, i)), (from = ++i);
          i++;
        }
        out.push(element(s + text.slice(from, i++)));
      } else {
        const from = i;
        while (i < n && text[i] !== delim && text[i] !== "}") i++;
        const s = text.slice(from, i);
        out.push(s === "NULL" ? null : element(s));
      }
      if (text[i++] === "}") return out;
    }
    return out;
  };
  return level();
}

/** The decoder of a Postgres value of `kind` from its text; `delim` is an array's (`;` for `box[]`). */
export function textDecoder(kind: Kind, delim = ","): Decoder {
  if (kind.endsWith("[]")) {
    const element = textDecoder(kind.slice(0, -2), delim);
    return (x) => parseArray(x, element, delim);
  }
  return TEXT_DECODERS[kind as ScalarKind] ?? identity;
}

// ---------------------------------------------------------------------------------------------
// The value of each kind from whatever a driver decoded it to: for the drivers that take no
// decoder (Bun.sql; the SQLite drivers, whose JSON is text and whose truth values are integers),
// and for the rare value a driver decoded before Reylo could name its type. Each converts the
// forms a driver is measured to give (wiki/driver-decoding/spike/) and leaves a value already in
// Reylo's form as it is.

/** Why a value cannot be given its kind: Reylo's error, naming the column. */
export class DecodeError extends TypeError {}

/** Why a parameter's value cannot be bound: Reylo's error, naming the parameter. */
export class ParameterError extends TypeError {}

/**
 * A string for a `timestamptz` names its zone: without one the server reads it in the session's
 * TimeZone, which the driver or the server sets.
 */
export const zoned = /(?:\d:\d\d(?::\d\d(?:\.\d+)?)?(?:\s*(?:z|[+-]\d\d?(?::?\d\d){0,2})|\s+[a-z][\w/+-]*)|^-?infinity|^epoch|^now)(?:\s+bc)?$/i;

/** A value as a message about a parameter names it: its kind, or a short form of itself. */
export function describe(v: unknown): string {
  if (v === null) return "null";
  if (v instanceof Date) return "a Date";
  if (Array.isArray(v)) return "an array";
  if (v instanceof Uint8Array) return "binary data";
  if (typeof v === "bigint") return `${v}n`;
  if (typeof v === "string") return JSON.stringify(v.length > 60 ? `${v.slice(0, 57)}...` : v);
  if (typeof v === "object") return "an object";
  return String(v);
}

const MAX_SAFE = Number.MAX_SAFE_INTEGER;

/** An integer as a number, refused where a number cannot hold it exactly. */
export function exactInteger(v: number | bigint, column: string): number {
  if (typeof v === "bigint") {
    // Exact: a BigInt past ±(2^53 - 1) converts to a number at least 2^53 in magnitude, since
    // rounding keeps the order and 2^53 is a number. Comparing BigInts instead took a five-column
    // row with a SQLite BIGINT, beside which better-sqlite3 reads every integer as a BigInt, from
    // 3.3 to 5.2 ms per 10,000 rows.
    const n = Number(v);
    if (n > MAX_SAFE || n < -MAX_SAFE) throw outOfRange(String(v), column);
    return n;
  }
  if (v > MAX_SAFE || v < -MAX_SAFE) throw outOfRange(String(v), column, true);
  return v;
}

function outOfRange(value: string, column: string, rounded = false): DecodeError {
  return new DecodeError(
    rounded
      ? `Column "${column}" holds an integer past ±(2^53 - 1), which the driver already rounded to ${value}: a number cannot hold it exactly, so Reylo refuses it rather than return a rounded value. Cast it to text in the statement (cast(${column} as text)) to read it exactly.`
      : `Column "${column}" holds ${value}, an integer past ±(2^53 - 1), which a number cannot hold exactly, so Reylo refuses it rather than return a rounded value. Cast it to text in the statement (cast(${column} as text)) to read it exactly.`,
  );
}

const INTEGER = /^[+-]?\d+$/;

/**
 * A 64-bit integer as a BigInt, from the driver's BigInt, its digits, or a number: a number past
 * ±(2^53 - 1) is one the driver already rounded, which no BigInt makes exact again, so it is
 * refused. Anything else (a SQLite REAL a column of INTEGER affinity kept) is left as it is.
 */
export function bigInteger(v: number | bigint | string, column: string): unknown {
  if (typeof v === "bigint") return v;
  if (typeof v === "string") return INTEGER.test(v) ? BigInt(v) : v;
  if (Number.isSafeInteger(v)) return BigInt(v);
  if (Number.isInteger(v)) throw new DecodeError(`Column "${column}" is a 64-bit integer, and the driver already rounded its value to ${v}, past ±(2^53 - 1): Reylo refuses it rather than return a BigInt of a rounded value.`);
  return v;
}

/**
 * A truth value as a boolean: true, false, 1, 0, `1n`, `0n`, `t`, `f`, `true`, `false`, `1`, `0`,
 * and a BIT(1)'s one byte. MySQL and SQLite have no boolean type and store one as an integer (a
 * TINYINT(1), a BIT(1), a column SQLite declares BOOLEAN), which neither database holds to 0 and 1:
 * any other value is refused, naming the column, rather than read as true.
 */
export function truth(v: unknown, column: string): boolean {
  switch (typeof v) {
    case "boolean":
      return v;
    case "number":
      if (v === 0 || v === 1) return v === 1;
      break;
    case "bigint":
      if (v === BigInt(0) || v === BigInt(1)) return v === BigInt(1);
      break;
    case "string":
      if (v === "t" || v === "true" || v === "1") return true;
      if (v === "f" || v === "false" || v === "0") return false;
      break;
    case "object":
      if (v instanceof Uint8Array && v.length === 1 && v[0]! <= 1) return v[0] === 1;
      break;
  }
  const shown = v instanceof Uint8Array ? `the bytes 0x${Array.from(v, (b) => b.toString(16).padStart(2, "0")).join("")}` : typeof v === "string" ? JSON.stringify(v) : String(v);
  return refuseTruth(column, shown);
}

/** The error for a value of a boolean column that is not 0 or 1, `shown` as the message names it. */
export function refuseTruth(column: string, shown: string): never {
  throw new DecodeError(
    `Column "${column}" is a boolean, stored as 0 or 1, and holds ${shown}, which is neither: the database does not hold the column to 0 and 1, so Reylo refuses the value rather than guess its truth. Store 0 or 1 in it, or read the stored number with ${column} + 0 in the statement.`,
  );
}

/**
 * A Date a driver handed back must hold a time. Bun.sql 1.4 decodes a date, timestamp or timestamptz
 * before 1 AD as an Invalid Date (its infinities come as the numbers ±Infinity), so an Invalid Date
 * is a value the driver lost, and Reylo refuses it rather than return a wrong one.
 */
function validDate(d: Date, column: string): void {
  if (Number.isNaN(d.getTime())) throw new DecodeError(`Column "${column}": the driver returned an Invalid Date, which is how Bun.sql decodes a date or timestamp before 1 AD. Reylo refuses it rather than return a wrong value.`);
}

/**
 * A Date a driver read as UTC (Bun.sql's date and timestamp) back to the wall-clock text the server
 * wrote: `YYYY-MM-DD`, or with `T` and the fraction the driver kept, and ` BC` for a year before 1.
 */
function utcText(d: Date, kind: "date" | "timestamp", column: string): string {
  validDate(d, column);
  const year = d.getUTCFullYear();
  const date = `${pad(year > 0 ? year : 1 - year, 4)}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}`;
  const bc = year > 0 ? "" : " BC";
  if (kind === "date") return date + bc;
  const ms = d.getUTCMilliseconds();
  return `${date}T${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())}${ms ? `.${pad(ms, 3).replace(/0+$/, "")}` : ""}${bc}`;
}

const isBytes = (v: unknown): v is Uint8Array => v instanceof Uint8Array;

/**
 * A MySQL VECTOR's floats from its bytes, 32-bit and little-endian, which mariadb and PlanetScale
 * hand over as they are (mysql2 decodes them itself).
 */
export function float32s(bytes: Uint8Array, column: string): number[] {
  if (bytes.byteLength % 4 !== 0) throw new DecodeError(`Column "${column}" is a VECTOR, whose value is 4 bytes per 32-bit float, and holds ${bytes.byteLength} bytes.`);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  return Array.from({ length: bytes.byteLength / 4 }, (_, i) => view.getFloat32(i * 4, true));
}

/**
 * `v` as a value of `kind`, from the form a driver decoded it to; `column` names it in an error. A
 * number past ±(2^53 - 1) in an integer column is one the driver already rounded, and is refused.
 */
export function coerce(kind: Kind, v: unknown, column: string): unknown {
  if (v === null || v === undefined) return null;
  if (kind.endsWith("[]")) {
    const element = kind.slice(0, -2);
    if (typeof v === "string") return parseArray(v, (s) => coerce(element, s, column));
    if (Array.isArray(v)) return v.map((e) => (Array.isArray(e) ? coerce(kind, e, column) : coerce(element, e, column)));
    // A MySQL VECTOR's bytes: no driver gives any other array of floats as bytes.
    if (element === "float" && isBytes(v)) return float32s(v, column);
    // Bun.sql's int4[] and float4[] are typed arrays.
    if (ArrayBuffer.isView(v) && !(v instanceof DataView)) return Array.from(v as unknown as ArrayLike<unknown>, (e) => coerce(element, e, column));
    return v;
  }
  if (kind.includes("|")) return coerceEither(kind, v, column);
  switch (kind as ScalarKind) {
    case "bool":
      return truth(v, column);
    case "int":
    case "float":
      if (typeof v === "number") return kind === "int" ? exactInteger(v, column) : v;
      if (typeof v === "bigint") return exactInteger(v, column);
      if (typeof v === "string" && v !== "" && !Number.isNaN(+v)) return kind === "int" && /^[+-]?\d+$/.test(v) ? exactInteger(BigInt(v), column) : +v;
      return v;
    case "int8":
      if (typeof v === "bigint" || typeof v === "number" || typeof v === "string") return bigInteger(v, column);
      return v;
    case "decimal":
      if (typeof v === "bigint" || typeof v === "number") return String(v);
      return v;
    case "date":
    case "timestamp":
      if (v instanceof Date) return utcText(v, kind as "date" | "timestamp", column);
      // Bun.sql's `infinity`.
      if (v === Infinity || v === -Infinity) return v > 0 ? "infinity" : "-infinity";
      return v;
    case "timestamptz":
      if (typeof v === "string") return instant(v);
      if (v instanceof Date) return instantOfDate(v, column);
      // Bun.sql's `infinity`.
      if (v === Infinity || v === -Infinity) return v > 0 ? "infinity" : "-infinity";
      return v;
    case "json":
      return typeof v === "string" ? JSON.parse(v) : v;
    case "bytes":
      if (isBytes(v)) return v;
      if (v instanceof ArrayBuffer) return new Uint8Array(v);
      // D1's BLOB is an array of numbers; mariadb's and Bun.sql's BIT(1) a truth value.
      if (Array.isArray(v)) return Uint8Array.from(v as number[]);
      if (typeof v === "boolean") return Uint8Array.of(v ? 1 : 0);
      if (typeof v === "string") return bytea(v);
      return v;
    case "text":
      if (typeof v === "string") return v;
      // mariadb's SET is an array of its members.
      if (Array.isArray(v)) return v.join(",");
      if (typeof v === "number" || typeof v === "bigint") return String(v);
      return v;
    default:
      return v;
  }
}

/**
 * A value of `a|b`: by what the driver gave, since only the value says which it is. A string is a
 * DECIMAL's or a text's, never a BIGINT's: each MySQL driver's BIGINT reaches here as a BigInt
 * already (src/decode-mysql.ts reads it by the result's own column type, and mariadb gives one).
 */
function coerceEither(kind: Kind, v: unknown, column: string): unknown {
  const kinds = kind.split("|");
  // A MySQL BIGINT is a BigInt; a SQLite INTEGER a number, exact or refused.
  if (typeof v === "bigint") return kinds.includes("int8") ? v : exactInteger(v, column);
  if (typeof v === "number") {
    // Where the column may hold a float, a number past 2^53 may be one.
    if (kinds.includes("float") || !Number.isInteger(v)) return v;
    if (kinds.includes("int")) return exactInteger(v, column);
    return kinds.includes("int8") ? bigInteger(v, column) : v;
  }
  if (v instanceof Date) return kinds.includes("timestamp") ? utcText(v, "timestamp", column) : kinds.includes("date") ? utcText(v, "date", column) : kinds.includes("timestamptz") ? instantOfDate(v, column) : v;
  if (v instanceof ArrayBuffer || (Array.isArray(v) && kinds.includes("bytes"))) return coerce("bytes", v, column);
  return v;
}
