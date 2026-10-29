// The run time on DuckDB, over @duckdb/node-api's DuckDBConnection: `runAndReadAll(sql, values)`
// runs the statement, and each value is given its type's one representation (src/values.ts) by
// the result's own column types, which hold every field of a STRUCT and every element of a LIST:
// a BIGINT or HUGEINT is a BigInt, a DECIMAL its exact text, a DATE `YYYY-MM-DD`, a TIMESTAMP its
// wall-clock text with `T`, a TIMESTAMPTZ its ISO 8601 instant in UTC, an INTERVAL an ISO 8601
// duration, a BLOB a Uint8Array, a JSON its value, a LIST an array, a STRUCT an object of its
// fields, a MAP its entries as `{ key, value }`, and a UNION `{ tag, value }`. A year outside 0000
// to 9999 is written as ISO 8601 extends it (`+010000`, `-000044`), and an infinite date or
// timestamp as `infinity` or `-infinity`.
//
// node-api binds a string, a number, a BigInt and a boolean as it is, and DuckDB casts each to its
// parameter's type, checking its range. A list, a struct, a map or binary data it binds only as
// a value of its own classes with its type said, so the run time builds those with node-api's own
// constructors, loaded from the app beside the connection: each element a type node-api takes
// from JavaScript (an integer a HUGEINT, a decimal or a date its text), which DuckDB casts in turn.
import { DecodeError, duration, ParameterError } from "./values.ts";
import type { EngineType } from "./engine-type.ts";
import type { Columns } from "./values.ts";

/** @duckdb/node-api's DuckDBConnection, a connection of a DuckDBInstance: `run` takes it. */
export type DuckdbClient = {
  runAndReadAll(sql: string, values?: any, types?: any): Promise<unknown>;
  prepare(sql: string): Promise<unknown>;
};

type Reader = { columnNames(): string[]; columnTypes(): DuckType[]; getRows(): unknown[][] };
type DuckType = {
  typeId: number;
  alias?: string;
  valueType?: DuckType;
  keyType?: DuckType;
  entryNames?: string[];
  entryTypes?: DuckType[];
  memberTags?: string[];
  memberTypes?: DuckType[];
};

/** The slice of @duckdb/node-api that builds a nested or binary value with its type. */
type Api = {
  DuckDBConnection: unknown;
  listValue(items: unknown[]): unknown;
  structValue(entries: Record<string, unknown>): unknown;
  mapValue(entries: { key: unknown; value: unknown }[]): unknown;
  unionValue(tag: string, value: unknown): unknown;
  blobValue(bytes: Uint8Array): unknown;
  LIST(of: unknown): unknown;
  STRUCT(fields: Record<string, unknown>): unknown;
  MAP(key: unknown, value: unknown): unknown;
  UNION(members: Record<string, unknown>): unknown;
  BOOLEAN: unknown;
  HUGEINT: unknown;
  DOUBLE: unknown;
  VARCHAR: unknown;
  BLOB: unknown;
};

// Named in a variable, so that a bundler leaves the import to the run time: only a DuckDB app has
// the package, and it imports it itself.
const NODE_API = "@duckdb/node-api";
/** The module path of node-api's entry, as Node's CommonJS cache keys it. */
const NODE_API_ENTRY = /[\\/]@duckdb[\\/]node-api[\\/]lib[\\/]index\.js$/;
const apis = new WeakMap<object, Promise<Api>>();

type ModuleCache = Record<string, { exports?: unknown } | undefined>;
type NodeProcess = { cwd(): string; getBuiltinModule?(id: string): { createRequire(from: string): { cache?: ModuleCache } } | undefined };

/**
 * The @duckdb/node-api the connection `c` came from. node-api takes a nested or binary value only
 * as an instance of its own classes, built over its own native binding, so a second copy of the
 * package (another version, or the same one installed twice, as a workspace or a linked package
 * can have it) builds values the connection refuses. node-api is CommonJS, so the copy that made
 * `c` is in Node's module cache however the app imported it: the one whose DuckDBConnection `c`
 * is an instance of, a subclass's included. For a connection that only wraps one, the copy is
 * the one loaded where there is one, else the copy reylo's install imports, which is refused only
 * when `c` is another copy's DuckDBConnection.
 */
function nodeApiOf(c: DuckdbClient): Promise<Api> {
  // A connection's class decides its copy; an object that only wraps one is its own.
  const proto = Object.getPrototypeOf(c) as object | null;
  const own = proto === null || proto === Object.prototype ? c : proto;
  let found = apis.get(own);
  if (found === undefined) {
    found = loadNodeApi(c);
    apis.set(own, found);
    found.catch(() => apis.delete(own));
  }
  return found;
}

/** The classes on `c`'s prototype chain. */
function classesOf(c: object): unknown[] {
  const out: unknown[] = [];
  for (let p = Object.getPrototypeOf(c) as { constructor?: unknown } | null; p !== null && p !== Object.prototype; p = Object.getPrototypeOf(p)) out.push(p.constructor);
  return out;
}

async function loadNodeApi(c: DuckdbClient): Promise<Api> {
  const classes = classesOf(c);
  const proc = (globalThis as { process?: NodeProcess }).process;
  let cache: ModuleCache | undefined;
  try {
    cache = proc?.getBuiltinModule?.("node:module")?.createRequire(`${proc.cwd()}/`).cache;
  } catch {}
  const loaded: Api[] = [];
  for (const [path, m] of Object.entries(cache ?? {})) {
    const exports = NODE_API_ENTRY.test(path) ? (m?.exports as Api | undefined) : undefined;
    if (exports?.DuckDBConnection === undefined) continue;
    if (classes.includes(exports.DuckDBConnection)) return exports;
    loaded.push(exports);
  }
  if (loaded.length === 1) return loaded[0]!;
  let imported: Api;
  try {
    imported = (await import(/* @vite-ignore */ NODE_API)) as Api;
  } catch (e) {
    throw new TypeError(`A list, a struct, a map or binary data is bound with @duckdb/node-api's own values, and the copy the connection came from cannot be found, nor one loaded from reylo's install: ${(e as Error).message}`);
  }
  const another = classes.some((k) => k !== imported.DuckDBConnection && (k as { name?: unknown })?.name === "DuckDBConnection");
  if (loaded.length > 1 || another) {
    const why = another ? "the copy reylo's install loads is another" : "the app has loaded several, and the connection is none of their DuckDBConnection";
    throw new TypeError(`A list, a struct, a map or binary data is bound with @duckdb/node-api's own values, which must come from the copy of the package the connection came from, and ${why}: install a single @duckdb/node-api.`);
  }
  return imported;
}

/** Whether a client is @duckdb/node-api's DuckDBConnection. */
export function isDuckdb(c: { [key: string]: unknown } | null | undefined): boolean {
  return c != null && typeof c.runAndReadAll === "function" && typeof c.prepare === "function" && typeof c.run === "function";
}

// DuckDB's type ids (duckdb_type), which node-api's types carry.
const T = {
  BOOLEAN: 1, TINYINT: 2, SMALLINT: 3, INTEGER: 4, BIGINT: 5, UTINYINT: 6, USMALLINT: 7, UINTEGER: 8, UBIGINT: 9, FLOAT: 10, DOUBLE: 11, TIMESTAMP: 12, DATE: 13, TIME: 14, INTERVAL: 15,
  HUGEINT: 16, VARCHAR: 17, BLOB: 18, DECIMAL: 19, TIMESTAMP_S: 20, TIMESTAMP_MS: 21, TIMESTAMP_NS: 22, ENUM: 23, LIST: 24, STRUCT: 25, MAP: 26, UUID: 27, UNION: 28, BIT: 29, TIME_TZ: 30,
  TIMESTAMP_TZ: 31, UHUGEINT: 32, ARRAY: 33, BIGNUM: 35, TIME_NS: 39, TIMESTAMP_TZ_NS: 42,
} as const;

/**
 * Runs `text` with `values`, each bound by `types` (the engine's type of each parameter), and
 * returns its rows, each value given its kind. `columns` is empty for a statement that returns no
 * rows, whose count DuckDB returns as a row.
 */
export async function runDuckdb(c: DuckdbClient, text: string, values: unknown[], types: (EngineType | undefined)[], columns: Columns | undefined): Promise<unknown[]> {
  let bound = values;
  let bindTypes: unknown[] | undefined;
  if (values.some((v) => v !== null && typeof v === "object")) {
    const a = await nodeApiOf(c);
    bindTypes = values.map((v, i) => (v !== null && typeof v === "object" ? bindType(a, types[i], v) : undefined));
    bound = values.map((v, i) => (v !== null && typeof v === "object" ? bindValue(a, types[i], v) : v));
  }
  const reader = (await c.runAndReadAll(text, bound, bindTypes)) as Reader;
  if (columns !== undefined && columns.length === 0) return [];
  const names = reader.columnNames();
  const kinds = reader.columnTypes();
  return reader.getRows().map((row) => {
    const out: Record<string, unknown> = {};
    for (let i = 0; i < names.length; i++) out[names[i]!] = decode(row[i], kinds[i]!, names[i]!);
    return out;
  });
}

// --- Binding ---------------------------------------------------------------------------------

/** The type node-api builds a value of `v` with, standing where the engine's type `t` does: every integer a HUGEINT, a float a DOUBLE, every text a VARCHAR, which DuckDB then casts. */
function bindType(a: Api, t: EngineType | undefined, v: unknown): unknown {
  if (v instanceof Uint8Array) return a.BLOB;
  if (Array.isArray(v)) {
    if (t?.kind === "map") return a.MAP(bindType(a, t.key, firstOf(v, "key")), bindType(a, t.value, firstOf(v, "value")));
    const of = t?.kind === "array" ? t.of : undefined;
    return a.LIST(bindType(a, of, v.find((e) => e != null)));
  }
  if (t?.kind === "struct" && v !== null && typeof v === "object") {
    return a.STRUCT(Object.fromEntries(t.fields.map((f) => [f.name, bindType(a, f.type, (v as Record<string, unknown>)[f.name])])));
  }
  if (t?.kind === "union" && v !== null && typeof v === "object") {
    return a.UNION(Object.fromEntries(t.members.map((m) => [m.name, bindType(a, m.type, (v as { tag?: unknown; value?: unknown }).tag === m.name ? (v as { value?: unknown }).value : undefined)])));
  }
  if (t?.kind === "scalar") {
    const name = t.name;
    if (name === "boolean") return a.BOOLEAN;
    if (INTEGERS.has(name)) return a.HUGEINT;
    if (name === "float" || name === "double") return a.DOUBLE;
    if (name === "blob") return a.BLOB;
    return a.VARCHAR;
  }
  if (t?.kind === "json" || t?.kind === "enum") return a.VARCHAR;
  // No type to go by (a query defined by hand): the value's own.
  switch (typeof v) {
    case "boolean": return a.BOOLEAN;
    case "number": return Number.isInteger(v) ? a.HUGEINT : a.DOUBLE;
    case "bigint": return a.HUGEINT;
    default: return a.VARCHAR;
  }
}

function firstOf(entries: unknown[], key: "key" | "value"): unknown {
  for (const e of entries) if (e !== null && typeof e === "object" && (e as Record<string, unknown>)[key] != null) return (e as Record<string, unknown>)[key];
  return undefined;
}

const INTEGERS = new Set(["tinyint", "smallint", "integer", "bigint", "hugeint", "utinyint", "usmallint", "uinteger", "ubigint", "uhugeint", "bignum"]);

/** `v` as node-api builds it for `bindType`'s type. */
function bindValue(a: Api, t: EngineType | undefined, v: unknown): unknown {
  if (v === null || v === undefined) return null;
  if (v instanceof Uint8Array) return a.blobValue(v);
  if (Array.isArray(v)) {
    if (t?.kind === "map") return a.mapValue(v.map((e) => ({ key: bindValue(a, t.key, (e as { key?: unknown } | null)?.key), value: bindValue(a, t.value, (e as { value?: unknown } | null)?.value) })));
    return a.listValue(v.map((e) => bindValue(a, t?.kind === "array" ? t.of : undefined, e)));
  }
  if (typeof v === "object") {
    if (t?.kind === "struct") return a.structValue(Object.fromEntries(t.fields.map((f) => [f.name, bindValue(a, f.type, (v as Record<string, unknown>)[f.name])])));
    if (t?.kind === "union") {
      const { tag, value } = v as { tag?: unknown; value?: unknown };
      return a.unionValue(String(tag), bindValue(a, t.members.find((m) => m.name === tag)?.type, value));
    }
    return JSON.stringify(v);
  }
  // A scalar, as `bindType` typed it: an integer a HUGEINT, a float a DOUBLE, a truth value a
  // BOOLEAN, and anything else its text.
  if (t?.kind === "scalar") {
    if (INTEGERS.has(t.name)) return typeof v === "bigint" ? v : BigInt(v as number | string);
    if (t.name === "float" || t.name === "double") return Number(v);
    if (t.name === "boolean") return v;
    return t.name === "interval" ? duckdbInterval(String(v)) : String(v);
  }
  if (t !== undefined) return typeof v === "string" ? v : t.kind === "json" ? JSON.stringify(v) : String(v);
  return typeof v === "number" && Number.isInteger(v) ? BigInt(v) : v;
}

/**
 * A value bound to a DuckDB parameter, checked and written as DuckDB reads it (the round-trip
 * rule): an integer takes no fraction, which DuckDB's cast would round away, and an ISO 8601
 * duration, which `.run()` returns for an INTERVAL and DuckDB does not read, goes as DuckDB's own
 * text. An array's elements and a struct's fields are checked by their own types.
 */
export function encodeDuckdb(type: EngineType | undefined, v: unknown, name: string): unknown {
  if (type === undefined || v == null) return v;
  if (type.kind === "array" && Array.isArray(v)) return v.map((e) => encodeDuckdb(type.of, e, name));
  if (type.kind === "struct" && typeof v === "object" && !Array.isArray(v)) {
    const out: Record<string, unknown> = { ...(v as Record<string, unknown>) };
    for (const f of type.fields) out[f.name] = encodeDuckdb(f.type, out[f.name], name);
    return out;
  }
  if (type.kind !== "scalar") return v;
  if (INTEGERS.has(type.name)) {
    if (typeof v === "number" && !Number.isInteger(v)) throw new ParameterError(`Parameter "${name}" is an integer, and ${v} has a fraction, which DuckDB would round away.`);
    if (typeof v === "string" && !/^\s*[+-]?\d+\s*$/.test(v)) throw new ParameterError(`Parameter "${name}" is an integer, so a string for it holds the digits alone; got ${JSON.stringify(v)}.`);
  }
  if (type.name === "interval" && typeof v === "string") return duckdbInterval(v);
  return v;
}

/**
 * An ISO 8601 duration (`P1Y2M3DT4H5M6.789S`, `-P1D`, `P1M-1D`) as DuckDB's interval text,
 * each part with its sign; any other text as it is, which DuckDB reads as its own (`1 day`).
 */
export function duckdbInterval(text: string): string {
  const n = "([+-]?\\d+(?:[.,]\\d+)?)";
  const m = new RegExp(`^\\s*([+-])?P(?:${n}Y)?(?:${n}M)?(?:${n}W)?(?:${n}D)?(?:T(?:${n}H)?(?:${n}M)?(?:${n}S)?)?\\s*$`, "i").exec(text);
  if (m === null || !/\d/.test(text)) return text;
  const negate = m[1] === "-";
  const units = ["years", "months", "weeks", "days", "hours", "minutes", "seconds"];
  const parts: string[] = [];
  units.forEach((unit, i) => {
    const value = m[i + 2];
    if (value === undefined) return;
    const number = value.replace(",", ".");
    parts.push(`${negate ? (number.startsWith("-") ? number.slice(1) : `-${number.replace(/^\+/, "")}`) : number} ${unit}`);
  });
  return parts.length ? parts.join(" ") : "0 seconds";
}

// --- Decoding --------------------------------------------------------------------------------

const pad = (n: number | bigint, width = 2) => String(n).padStart(width, "0");

/** An ISO 8601 year: four digits from 0000 to 9999, and a sign and six digits outside them. */
const isoYear = (y: number) => (y >= 0 && y <= 9999 ? pad(y, 4) : `${y < 0 ? "-" : "+"}${pad(Math.abs(y), 6)}`);

/** The date `days` after 1970-01-01 in the proleptic Gregorian calendar, year 0 being 1 BC. */
function civil(days: number): string {
  const z = days + 719468;
  const era = Math.floor(z / 146097);
  const doe = z - era * 146097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const d = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const m = mp + (mp < 10 ? 3 : -9);
  return `${isoYear(era * 400 + yoe + (m <= 2 ? 1 : 0))}-${pad(m)}-${pad(d)}`;
}

/** A count of `unit`ths of a second since 1970 as `YYYY-MM-DDTHH:MM:SS[.fraction]`, the fraction's trailing zeros dropped. */
function wallClock(count: bigint, perSecond: bigint, digits: number): string {
  const perDay = 86400n * perSecond;
  let days = count / perDay;
  let rest = count % perDay;
  if (rest < 0n) {
    rest += perDay;
    days -= 1n;
  }
  const seconds = Number(rest / perSecond);
  const fraction = (rest % perSecond).toString().padStart(digits, "0").replace(/0+$/, "");
  return `${civil(Number(days))}T${pad(Math.floor(seconds / 3600))}:${pad(Math.floor(seconds / 60) % 60)}:${pad(seconds % 60)}${fraction && `.${fraction}`}`;
}

// DuckDB's infinite date and timestamps.
const DATE_INFINITY = 2147483647;
const TS_INFINITY = 9223372036854775807n;

function timestamp(count: bigint, perSecond: bigint, digits: number, zone: string): string {
  if (count === TS_INFINITY) return "infinity";
  if (count === -TS_INFINITY) return "-infinity";
  return wallClock(count, perSecond, digits) + zone;
}

/** An interval's months, days and microseconds as an ISO 8601 duration, as `.run()` returns a Postgres one (src/values.ts). */
function interval(months: number, days: number, micros: bigint): string {
  const years = Math.trunc(months / 12);
  const mons = months - years * 12;
  const sign = micros < 0n ? "-" : "";
  const abs = micros < 0n ? -micros : micros;
  const seconds = abs / 1000000n;
  const fraction = (abs % 1000000n).toString().padStart(6, "0");
  return duration(`${years} years ${mons} mons ${days} days ${sign}${seconds / 3600n}:${pad((seconds / 60n) % 60n)}:${pad(seconds % 60n)}.${fraction}`);
}

/** A DECIMAL's unscaled value as its exact text. */
function decimal(value: bigint, scale: number): string {
  if (scale === 0) return value.toString();
  const negative = value < 0n;
  const digits = (negative ? -value : value).toString().padStart(scale + 1, "0");
  return `${negative ? "-" : ""}${digits.slice(0, -scale)}.${digits.slice(-scale)}`;
}

/** A value of DuckDB's type `t`, as node-api reads it, in its kind's form; `column` names it in an error. */
function decode(v: unknown, t: DuckType, column: string): unknown {
  if (v === null || v === undefined) return null;
  const o = v as Record<string, any>;
  switch (t.typeId) {
    case T.BOOLEAN: case T.TINYINT: case T.SMALLINT: case T.INTEGER: case T.UTINYINT: case T.USMALLINT: case T.UINTEGER: case T.FLOAT: case T.DOUBLE:
      return v;
    case T.BIGINT: case T.UBIGINT: case T.HUGEINT: case T.UHUGEINT: case T.BIGNUM:
      return typeof v === "bigint" ? v : BigInt(v as number);
    case T.VARCHAR:
      if (t.alias?.toUpperCase() === "JSON" && typeof v === "string") {
        try {
          return JSON.parse(v);
        } catch {
          throw new DecodeError(`Column "${column}" is JSON, and holds text that is not: ${JSON.stringify(v.slice(0, 60))}.`);
        }
      }
      return v;
    case T.ENUM: case T.UUID: case T.BIT: case T.TIME: case T.TIME_NS: case T.TIME_TZ:
      return String(v);
    case T.BLOB:
      return o.bytes instanceof Uint8Array ? o.bytes : v;
    case T.DECIMAL:
      return typeof o.value === "bigint" && typeof o.scale === "number" ? decimal(o.value, o.scale) : String(v);
    case T.DATE:
      if (o.days === DATE_INFINITY) return "infinity";
      if (o.days === -DATE_INFINITY) return "-infinity";
      return typeof o.days === "number" ? civil(o.days) : String(v);
    case T.TIMESTAMP: return timestamp(o.micros, 1000000n, 6, "");
    case T.TIMESTAMP_S: return timestamp(o.seconds, 1n, 0, "");
    case T.TIMESTAMP_MS: return timestamp(o.millis, 1000n, 3, "");
    case T.TIMESTAMP_NS: return timestamp(o.nanos, 1000000000n, 9, "");
    case T.TIMESTAMP_TZ: return timestamp(o.micros, 1000000n, 6, "Z");
    case T.TIMESTAMP_TZ_NS: return timestamp(o.nanos, 1000000000n, 9, "Z");
    case T.INTERVAL: return interval(o.months, o.days, o.micros);
    case T.LIST: case T.ARRAY:
      return (o.items as unknown[]).map((e) => decode(e, t.valueType!, column));
    case T.STRUCT: {
      const out: Record<string, unknown> = {};
      t.entryNames!.forEach((name, i) => (out[name] = decode(o.entries[name], t.entryTypes![i]!, column)));
      return out;
    }
    case T.MAP:
      return (o.entries as { key: unknown; value: unknown }[]).map((e) => ({ key: decode(e.key, t.keyType!, column), value: decode(e.value, t.valueType!, column) }));
    case T.UNION: {
      const at = t.memberTags!.indexOf(o.tag);
      return { tag: o.tag, value: at < 0 ? o.value : decode(o.value, t.memberTypes![at]!, column) };
    }
    default:
      // A type an extension adds (GEOMETRY, VARIANT): its text, where node-api gives a value of its own.
      if (v instanceof Uint8Array) return v;
      return typeof v === "object" ? String(v) : v;
  }
}
