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
// wall-clock text with `T` between the date and the time (a year of either outside 0000–9999 in
// ISO 8601's extended form, `+012345` or `-000001` for 2 BC); `timestamptz` the instant's ISO 8601 text
// in UTC, with `Z` and the server's fraction; `interval` an ISO 8601 duration with its sign in
// front; `json` the parsed value; `bytes` a Uint8Array (a Buffer is one); `vector` pgvector's
// `vector` or `halfvec` as an array of its numbers; `hstore` an hstore's keys as an object, each
// value a string or null; `text` the server's text, which every other type is (time, uuid, an
// enum's label, a row value whose fields are not known); `unknown` whatever the driver gives (a
// SQLite column with no declared type); `geometry` a MySQL GEOMETRY's bytes, which a driver may parse unless asked for them; `(…)` a Postgres row value of known fields as an object
// of them, each field listed as `[name, kind]` (`(["id","int8"],["name","text"])`); `point`,
// `line`, `lseg`, `box`, `path`, `polygon` and `circle` Postgres's geometric types as objects of
// their fields, as the type descriptor gives them; and `range:k` and `multirange:k` a range of `k`
// as an object of its bounds and a multirange as an array of them. A DuckDB STRUCT, MAP or UNION
// is decoded by the result's own types
// (src/decode-duckdb.ts), each of its values in its own kind's form. `k[]` is an array of `k`, a NULL element `null`, and `a|b` either (a MySQL
// expression whose type the bound value decides). No kind is a JavaScript Date. An array's text
// separates its elements with a comma, or with its element type's own delimiter (`:` for PostGIS's
// `geometry`), which a column carries beside its kind.
//
// This module runs in every generated module's process, so it names no Node type: a Buffer is
// reached through `globalThis` where one exists.
import { bareName, type EngineType } from "./engine-type.ts";

export type Dialect = "postgres" | "mysql" | "sqlite" | "duckdb";

/** A scalar value's representation. */
export type ScalarKind = "bool" | "int" | "float" | "int8" | "decimal" | "text" | "date" | "timestamp" | "timestamptz" | "interval" | "json" | "bytes" | "vector" | "hstore" | Geometric | "enum" | "set" | "geometry" | "unknown";
/** Postgres's built-in geometric types, each an object of its fields (`point` is `{ x, y }`). */
export type Geometric = "point" | "line" | "lseg" | "box" | "path" | "polygon" | "circle";
/** A column's kind as a module carries it: a scalar kind, `k[]` for an array of `k`, `a|b` for either, `(…)` for a Postgres row value of the fields it lists, `range:k` and `multirange:k` for a range of `k` and a multirange. */
export type Kind = string;
/** The output columns a module carries, in the statement's order; an array's `delimiter` where it is not a comma. */
export type Columns = readonly (readonly [name: string, kind: Kind, delimiter?: string])[];

/**
 * Where a JSON value holds a float or a decimal that may be NaN or an infinity, which Postgres
 * writes inside a document as text (src/render.ts, `jsonNumbers`): `true` for such a value,
 * `items` for an array's elements, through nested arrays where `deep`, and `keys` for an object's
 * fields.
 */
export type JsonNumbers = true | { readonly items: JsonNumbers; readonly deep?: true } | { readonly keys: readonly (readonly [key: string, at: JsonNumbers])[] };

/**
 * The JSON columns of a row whose non-finite numbers `.run()` gives as numbers: each name's last
 * column, the one a row keeps (src/row.ts), where its mask, by position beside `columns`, has any.
 */
export function rowMasks(columns: Columns, nonFinite: readonly (JsonNumbers | null)[]): (readonly [name: string, at: JsonNumbers])[] {
  const last = new Map<string, JsonNumbers | null>();
  columns.forEach(([name], i) => last.set(name, nonFinite[i] ?? null));
  return [...last].flatMap(([name, at]) => (at === null ? [] : [[name, at] as const]));
}

/**
 * A parsed JSON value with the text Postgres writes for each non-finite float or decimal at
 * `at`, `"NaN"`, `"Infinity"` or `"-Infinity"`, as that number, in place: JSON has no number for
 * one, and a float outside a document is one already.
 */
export function numbersInJson(v: unknown, at: JsonNumbers): unknown {
  if (at === true) return v === "NaN" ? NaN : v === "Infinity" ? Infinity : v === "-Infinity" ? -Infinity : v;
  if (v === null || typeof v !== "object") return v;
  if ("items" in at) {
    if (!Array.isArray(v)) return v;
    // A Postgres array nests to any depth, each level an array; where its element is a JSON array
    // as well, an array is read as the element, since the two cannot be told apart.
    const nested = at.deep === true && !(typeof at.items === "object" && "items" in at.items);
    for (let i = 0; i < v.length; i++) v[i] = nested && Array.isArray(v[i]) ? numbersInJson(v[i], at) : numbersInJson(v[i], at.items);
    return v;
  }
  const o = v as Record<string, unknown>;
  for (const [key, inner] of at.keys) {
    if (!Object.prototype.hasOwnProperty.call(o, key)) continue;
    const value = numbersInJson(o[key], inner);
    // `__proto__` is an own key JSON.parse made, which an assignment would not reach.
    if (key === "__proto__") Object.defineProperty(o, key, { value, writable: true, enumerable: true, configurable: true });
    else o[key] = value;
  }
  return v;
}

const POINT = "{ x: number; y: number }";

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
  interval: "string",
  // Any JSON value, the type `reylo` exports (src/render.ts).
  json: "Json",
  bytes: "Uint8Array",
  vector: "number[]",
  hstore: "Record<string, string | null>",
  // The geometric types' fields, as the type descriptor names them (engine/postgres pgtypes.rs).
  point: POINT,
  line: "{ a: number; b: number; c: number }",
  lseg: `${POINT}[]`,
  box: `{ high: ${POINT}; low: ${POINT} }`,
  path: `{ closed: boolean; points: ${POINT}[] }`,
  polygon: `${POINT}[]`,
  circle: `{ center: ${POINT}; radius: number }`,
  // A MySQL ENUM's label, whose union its descriptor says (src/render.ts).
  enum: "string",
  // A MySQL SET's members, which MySQL keeps once each, in the order the column declares them.
  set: "Set<string>",
  // A GEOMETRY's bytes, `unknown` as the type of a value its WKB alone describes.
  geometry: "unknown",
  unknown: "unknown",
};

/**
 * The kind of a Postgres base type by its catalog name, for a value whose type a driver names
 * (src/decode.ts; a value the engine typed takes its kind from its descriptor, `columnKind`). Every
 * type not named here is its text: Reylo owns every oid, so the driver never parses one (an enum's label, `time`,
 * `uuid`, `money`, `xml`); a range and a row value take their kinds from their oids or their
 * descriptors (src/decode.ts). pgvector's `vector` and `halfvec`, and an
 * `hstore`, are extensions' types, named by their names as every created type is.
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
  interval: "interval",
  json: "json",
  jsonb: "json",
  bytea: "bytes",
  vector: "vector",
  halfvec: "vector",
  hstore: "hstore",
  point: "point",
  line: "line",
  lseg: "lseg",
  box: "box",
  path: "path",
  polygon: "polygon",
  circle: "circle",
};

/**
 * The kind of a Postgres type by its catalog name, which the run time decodes an array's elements
 * by where a driver names their type (src/decode.ts): every type not named above is its text.
 */
export function scalarKind(name: string): Kind {
  return SQL_KINDS[name] ?? "text";
}

/** Whether a SQLite declared type names a 64-bit integer: `BIGINT`, `INT8` or `UNSIGNED BIG INT`, a size after it ignored (engine/sqlite `is_int8_declared`). */
export function sqliteWide(declared: string): boolean {
  const base = declared.split("(")[0]!.trim().toLowerCase().split(/\s+/).join(" ");
  return base === "bigint" || base === "int8" || base === "unsigned big int";
}

/** SQLite's affinity of a declared type, by its five rules (engine/sqlite `affinity`). */
export function sqliteAffinity(declared: string): "integer" | "text" | "blob" | "real" | "numeric" {
  const d = declared.toUpperCase();
  if (d.includes("INT")) return "integer";
  if (d.includes("CHAR") || d.includes("CLOB") || d.includes("TEXT")) return "text";
  if (d.includes("BLOB") || d === "") return "blob";
  if (d.includes("REAL") || d.includes("FLOA") || d.includes("DOUB")) return "real";
  return "numeric";
}

/**
 * The kind of a SQLite value the engine traced to a column's declared type, by that type's name:
 * a BOOLEAN's a truth value, an integer's a number or, declared 64 bits wide, a BigInt, and a
 * NUMERIC's whichever storage class the row stored. A client reads SQLite's declared type, which
 * the store does not enforce, and refuses a value it does not admit (src/decode-sqlite.ts).
 */
function sqliteDeclaredKind(declared: string): Kind {
  const d = declared.trim();
  if (/^bool(ean)?$/i.test(d)) return "bool";
  switch (sqliteAffinity(d)) {
    case "integer": return sqliteWide(d) ? "int8" : "int";
    case "real": return "float";
    case "text": return "text";
    case "blob": return "bytes";
    case "numeric": return "int|float|text";
  }
}

/** Postgres's built-in geometric types, which the type descriptor structures as records (`lseg` and `polygon` as arrays of points) and names with no schema, as it names every built-in type. */
const GEOMETRIC: ReadonlySet<string> = new Set<Geometric>(["point", "line", "lseg", "box", "path", "polygon", "circle"]);
/** The geometric type a descriptor is, by its whole name: a created type of the same name is named with its schema. */
const geometric = (t: EngineType): Geometric | undefined => ("label" in t && t.label !== undefined && GEOMETRIC.has(t.label) ? (t.label as Geometric) : undefined);

/** A Postgres row value's kind: `(…)`, each field `[name, kind]`, and an array field's delimiter third where it is not a comma. */
export const isRowKind = (kind: Kind): boolean => kind.startsWith("(");
const rowFieldsCache = new Map<Kind, Columns>();
/** The fields a row value's kind lists, in order. */
export function rowFields(kind: Kind): Columns {
  let fields = rowFieldsCache.get(kind);
  if (fields === undefined) rowFieldsCache.set(kind, (fields = JSON.parse(`[${kind.slice(1, -1)}]`) as Columns));
  return fields;
}

/** What separates the elements of an array of `element` in Postgres's text: its type's `typdelim`, `;` for a box and `:` for PostGIS's types. */
function delimiterOf(element: EngineType): string | undefined {
  if (geometric(element) === "box") return ";";
  return element.kind === "geometry" && (bareName(element) === "geometry" || bareName(element) === "geography") ? ":" : undefined;
}

/** Kinds that may each be the value's, one `a|b` kind: a BigInt absorbs a narrower integer, a BigInt either way, where no float shares the column. */
function either(kinds: Kind[]): Kind {
  const parts = [...new Set(kinds.flatMap((k) => k.split("|")))];
  const wide = parts.includes("int8") && !parts.includes("float") ? parts.filter((k) => k !== "int") : parts;
  return wide.join("|");
}

/**
 * The kind of a value of the engine's type `t`, standing in a result row (not inside a JSON
 * document): its one representation (the table above). An integer is a number up to 32 bits and a
 * BigInt past them, a time of day, an enum's label, a range and every type the run time does not
 * convert the server's text, and a structured type a driver hands over as text (a Postgres row
 * value, a bit string, an `inet`) that text. A name decides where the type is a client's
 * convention: MySQL's `tinyint(1)` and a SQLite column declared BOOLEAN are truth values, a SQLite
 * column declared BIGINT a BigInt, and pgvector's vectors and an hstore their own kinds.
 */
export function columnKind(t: EngineType, dialect: Dialect): Kind {
  const name = bareName(t);
  switch (t.kind) {
    case "boolean":
      return "bool";
    case "integer":
      if (dialect === "sqlite") return sqliteWide(name) ? "int8" : "int";
      if (dialect === "mysql" && name === "tinyint(1)") return "bool";
      return t.bits > 32 ? "int8" : "int";
    case "float":
      return "float";
    case "decimal":
      // DuckDB's BIGNUM, an integer of no fixed width, is a BigInt.
      return dialect === "duckdb" && name === "bignum" ? "int8" : "decimal";
    case "bytes":
      return dialect === "postgres" && (name === "macaddr" || name === "macaddr8") ? "text" : "bytes";
    case "array": {
      if (dialect === "postgres") {
        if (name === "vector" || name === "halfvec") return "vector";
        if (name === "bit" || name === "varbit" || name === "sparsevec") return "text";
        const shape = geometric(t);
        if (shape !== undefined) return shape;
        // An array of row values whose fields are not known is the text Postgres prints.
        const element = t.element.type;
        if (element.kind === "opaque" && bareName(element) === "record") return "text";
      }
      if (dialect === "mysql" && name === "bit") return t.size?.exact === 1 ? "bool" : "bytes";
      if (dialect === "duckdb" && name === "bit") return "text";
      return `${columnKind(t.element.type, dialect)}[]`;
    }
    case "json":
      return "json";
    case "date":
      return "date";
    case "datetime":
      return "timestamp";
    case "instant":
      // A MySQL TIMESTAMP reaches the client as the session's wall-clock time.
      return dialect === "mysql" ? "timestamp" : "timestamptz";
    case "duration":
      // MySQL's TIME is its text.
      return dialect === "mysql" ? "text" : "interval";
    case "map":
      return dialect === "postgres" && name === "hstore" ? "hstore" : "unknown";
    case "union":
      if (dialect === "sqlite") return "label" in t && t.label !== undefined ? sqliteDeclaredKind(t.label) : "unknown";
      if (dialect === "postgres" && (name === "inet" || name === "cidr")) return "text";
      if (dialect === "duckdb") return "unknown";
      return either(t.members.map((m) => columnKind(m, dialect)));
    case "tagged_union":
      // A DuckDB union is decoded by the result's own types (src/decode-duckdb.ts).
      return "unknown";
    case "record": {
      // A DuckDB struct is decoded by the result's own types (src/decode-duckdb.ts).
      if (dialect !== "postgres") return "unknown";
      // A Postgres row value is an object of its fields, each decoded from its text as a column of
      // its type is.
      const shape = geometric(t);
      if (shape !== undefined) return shape;
      return `(${t.fields.map((f) => {
        const kind = columnKind(f.type, dialect);
        const delim = f.type.kind === "array" && kind.endsWith("[]") ? delimiterOf(f.type.element.type) : undefined;
        return JSON.stringify(delim === undefined ? [f.name, kind] : [f.name, kind, delim]);
      }).join(",")})`;
    }
    case "text":
      // A MySQL ENUM of known labels holds one of them or `''`, which the run time refuses.
      return dialect === "mysql" && name === "enum" && t.enum !== undefined && !t.enum.includes("") ? "enum" : "text";
    case "set":
      // A MySQL SET is a Set of its members.
      return dialect === "mysql" ? "set" : "text";
    case "range":
    case "multirange":
      // A Postgres range is an object of its bounds, each decoded from its text as its subtype is.
      return dialect === "postgres" ? `${t.kind}:${columnKind(t.subtype, dialect)}` : "text";
    case "geometry":
      // A MySQL GEOMETRY is its bytes, which `.run()` asks each driver for.
      return dialect === "postgres" ? "text" : dialect === "mysql" ? "geometry" : "unknown";
    case "opaque":
      return dialect === "postgres" ? "text" : "unknown";
    case "unknown":
    case "rows":
      return "unknown";
    default:
      return "text";
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

/**
 * A date's or timestamp's text with its year as `isoYear` writes it: Postgres writes a year past
 * 9999 with more digits (`12345-01-01`) and one before 1 AD in its own form (`0002-06-01 BC`), which
 * `Temporal.PlainDate.from` refuses, so `12345-01-01` is `+012345-01-01` and `0002-06-01 BC` is
 * `-000001-06-01`. A four-digit year after 1 BC, `infinity` and `-infinity` are left as they are.
 */
function isoDate(x: string): string {
  const dash = x.indexOf("-");
  const bc = x.endsWith(" BC");
  if (dash < 4 || (dash === 4 && !bc)) return x;
  const digits = x.slice(0, dash);
  if (!/^\d+$/.test(digits)) return x;
  return isoYear(bc ? 1 - +digits : +digits) + x.slice(dash, bc ? -3 : undefined);
}

/** `2026-10-20` as it is; `0002-06-01 BC` → `-000001-06-01` and `12345-01-01` → `+012345-01-01` (`isoDate`). */
const date: Decoder = isoDate;

/** `2026-10-20 10:00:00.5` → `2026-10-20T10:00:00.500000`, its year as a date's (`isoDate`) and its fraction six digits (`sixDigits`); `infinity` as written. */
const timestamp: Decoder = (x) => sixDigits(isoDate(x.replace(" ", "T")));

/**
 * A Postgres `timestamp`'s ISO 8601 text with a fraction of six digits, the microseconds Postgres
 * keeps, where the server writes as many as the value needs: so two of them sort as strings in the
 * order of their times. Any other text as it is.
 */
export function sixDigits(x: string): string {
  const m = /^(.*T\d\d:\d\d:\d\d)(?:\.(\d{1,6}))?$/.exec(x);
  return m === null ? x : `${m[1]}.${(m[2] ?? "").padEnd(6, "0")}`;
}

/** A fraction of a second as the server wrote it (`.5`, or none), as six digits (`.500000`). */
const micros = (fraction: string) => (fraction === "" ? ".000000" : fraction.padEnd(7, "0"));

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
 * digits, outside them (`+010000`, `-000001` for 2 BC), as `Temporal` reads and writes it. A year
 * past 999999, which only a Postgres `date` reaches (up to 5874897), takes the digits it needs after
 * the sign (`+5874897`): ISO 8601 leaves the number of extended digits to the parties that exchange
 * them, Temporal holds no such date in any form, and a sign keeps the rule that every year outside
 * 0000–9999 has one, which `postgresYear` reads back.
 */
export const isoYear = (y: number) => (y >= 0 && y <= 9999 ? pad(y, 4) : `${y < 0 ? "-" : "+"}${pad(Math.abs(y), 6)}`);

/**
 * A date or time parameter's text as Postgres reads it, the inverse of `isoYear` for what `run`
 * returns (the round-trip rule): Postgres 18 refuses an ISO 8601 extended year (`+010000-01-01`,
 * "time zone displacement out of range") and the year 0000 ("date/time field value out of range"),
 * and reads `10000-01-01` and a year before 1 as its own ` BC` form, year 1 − n, so `-000001` is
 * `0002 … BC` and `0000` or `+000000` is `0001 … BC`. Any other text is left as it is.
 */
export function postgresYear(text: string): string {
  const m = /^([+-]\d{6,}|0000)(-.*)$/.exec(text);
  if (m === null || /\sbc$/i.test(text)) return text;
  const year = +m[1]!;
  return year > 0 ? `${pad(year, 4)}${m[2]}` : `${pad(1 - year, 4)}${m[2]} BC`;
}

/**
 * A timestamptz's text as the instant's ISO 8601 text in UTC: `2026-10-20 10:00:00.5+02` is
 * `2026-10-20T08:00:00.500000Z`, the fraction six digits where the server writes as many as the
 * value needs, so that two instants sort as strings in the order of their times. `Temporal.Instant.from`
 * reads it. `infinity` and `-infinity` stay those words.
 */
export function instant(x: string): string {
  // The server's usual text, `YYYY-MM-DD HH:MM:SS[.f]±HH[:MM[:SS]]` with a year from 1000 to 9999,
  // is read by position, every character checked, and moved to UTC in integer arithmetic: a
  // third of the pattern's cost. Any other text (BC, a year outside those, infinity) is the
  // pattern's, and so is an instant in UTC outside those years, so that both give the same text.
  const n = x.length;
  if (n < 22 || !digits(x, 0, 4) || x.charCodeAt(0) === 48 || x.charCodeAt(4) !== 45 || !digits(x, 5, 7) || x.charCodeAt(7) !== 45 || !digits(x, 8, 10) || x.charCodeAt(10) !== 32) return instantByPattern(x);
  if (!digits(x, 11, 13) || x.charCodeAt(13) !== 58 || !digits(x, 14, 16) || x.charCodeAt(16) !== 58 || !digits(x, 17, 19)) return instantByPattern(x);
  let i = 19;
  if (x.charCodeAt(19) === 46) {
    i = 20;
    while (i < n && i < 26 && isDigit(x, i)) i++;
    if (i === 20) return instantByPattern(x);
  }
  const sign = x.charCodeAt(i);
  if (sign !== 43 && sign !== 45) return instantByPattern(x);
  const zone = n - i - 1;
  if (!(zone === 2 || (zone === 5 && x.charCodeAt(i + 3) === 58) || (zone === 8 && x.charCodeAt(i + 3) === 58 && x.charCodeAt(i + 6) === 58))) return instantByPattern(x);
  if (!digits(x, i + 1, i + 3) || (zone >= 5 && !digits(x, i + 4, i + 6)) || (zone === 8 && !digits(x, i + 7, i + 9))) return instantByPattern(x);
  const fraction = i > 19 ? x.slice(19, i) : "";
  const offset = (sign === 45 ? -1 : 1) * (two(x, i + 1) * 3600 + (zone >= 5 ? two(x, i + 4) * 60 : 0) + (zone === 8 ? two(x, i + 7) : 0));
  const seconds = daysFromCivil(two(x, 0) * 100 + two(x, 2), two(x, 5), two(x, 8)) * 86400 + two(x, 11) * 3600 + two(x, 14) * 60 + two(x, 17) - offset;
  const days = Math.floor(seconds / 86400);
  const s = seconds - days * 86400;
  // `civilFromDays`, inline, without its array.
  const z = days + 719468;
  const era = Math.floor(z / 146097);
  const doe = z - era * 146097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const d = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const mo = mp + (mp < 10 ? 3 : -9);
  const y = era * 400 + yoe + (mo <= 2 ? 1 : 0);
  if (y < 1000 || y > 9999) return instantByPattern(x);
  return `${y}-${TWO[mo]}-${TWO[d]}T${TWO[Math.floor(s / 3600)]}:${TWO[Math.floor(s / 60) % 60]}:${TWO[s % 60]}${micros(fraction)}Z`;
}

/** `00` to `99`, by number. */
const TWO = Array.from({ length: 100 }, (_, i) => pad(i));
const isDigit = (x: string, i: number) => {
  const c = x.charCodeAt(i);
  return c >= 48 && c <= 57;
};
/** Whether every character of `x` from `start` to `end` is a digit. */
function digits(x: string, start: number, end: number): boolean {
  for (let i = start; i < end; i++) if (!isDigit(x, i)) return false;
  return true;
}
/** The two digits at `i` as a number. */
const two = (x: string, i: number) => (x.charCodeAt(i) - 48) * 10 + x.charCodeAt(i + 1) - 48;

/** `instant` by the pattern, for every text: BC, any year, infinity, and text that is not a timestamptz's, returned as it is. */
function instantByPattern(x: string): string {
  const m = TSTZ.exec(x);
  if (m === null) return x;
  const year = m[12] === undefined ? +m[1]! : 1 - +m[1]!;
  const offset = (m[8] === "-" ? -1 : 1) * (+m[9]! * 3600 + (m[10] === undefined ? 0 : +m[10] * 60) + (m[11] === undefined ? 0 : +m[11]));
  const seconds = daysFromCivil(year, +m[2]!, +m[3]!) * 86400 + +m[4]! * 3600 + +m[5]! * 60 + +m[6]! - offset;
  const days = Math.floor(seconds / 86400);
  const s = seconds - days * 86400;
  const [y, mo, d] = civilFromDays(days);
  return `${isoYear(y)}-${pad(mo)}-${pad(d)}T${pad(Math.floor(s / 3600))}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}${micros(m[7] ?? "")}Z`;
}

/**
 * The instant a driver already decoded to a Date (Bun.sql's timestamptz), as the same text: to the
 * millisecond, which is all a Date holds, in six digits as `instant` writes them, so that a value
 * the driver did not cut reads alike on every driver.
 */
export function instantOfDate(d: Date, column: string): string {
  validDate(d, column);
  return d.toISOString().replace(/Z$/, "000Z");
}

// interval in IntervalStyle postgres: `1 year 2 mons 3 days 04:05:06.789`. Postgres keeps three
// parts, each signed on its own: months (written as years and months), days, and a time that may
// pass 24 hours; a part after a negative one carries `+` (`-1 days +01:00:00`).
const INTERVAL = /^(?:([+-]?\d+) years? ?)?(?:([+-]?\d+) mons? ?)?(?:([+-]?\d+) days? ?)?(?:([+-]?)(\d+):(\d\d):(\d\d)(?:\.(\d+))?)?$/;

/**
 * An interval's text as an ISO 8601 duration, which `Temporal.Duration.from` reads: `1 year 2 mons
 * 3 days 04:05:06.789` is `P1Y2M3DT4H5M6.789S` and zero is `PT0S`, as Postgres writes them in
 * IntervalStyle iso_8601, and a negative interval has one sign in front, `-1 days` being `-P1D`
 * (Postgres writes `P-1D`, which ISO 8601 and Temporal refuse). An interval whose parts differ in
 * sign has no ISO 8601 form, so it keeps Postgres's, each part signed on its own: `1 mon -1 days`
 * is `P1M-1D`, which Postgres reads back and Temporal refuses. `infinity` and `-infinity` stay
 * those words.
 */
export function duration(x: string): string {
  const m = INTERVAL.exec(x);
  if (m === null || x === "") return x;
  const time = m[4] === "-" ? -1 : 1;
  // Each designator Postgres writes, its magnitude and its sign; a zero is left out.
  const parts: [designator: string, magnitude: string, sign: number][] = [];
  for (const [designator, digits] of [["Y", m[1]], ["M", m[2]], ["D", m[3]]] as const) {
    if (digits !== undefined && +digits !== 0) parts.push([designator, String(Math.abs(+digits)), digits.startsWith("-") ? -1 : 1]);
  }
  const date = parts.length;
  if (+(m[5] ?? 0) !== 0) parts.push(["H", String(+m[5]!), time]);
  if (+(m[6] ?? 0) !== 0) parts.push(["M", String(+m[6]!), time]);
  const fraction = (m[8] ?? "").replace(/0+$/, "");
  if (+(m[7] ?? 0) !== 0 || fraction !== "") parts.push(["S", `${+(m[7] ?? 0)}${fraction && `.${fraction}`}`, time]);
  if (parts.length === 0) return "PT0S";
  const negative = parts.every(([, , sign]) => sign < 0);
  const write = ([designator, magnitude, sign]: [string, string, number]) => `${sign < 0 && !negative ? "-" : ""}${magnitude}${designator}`;
  const clock = parts.slice(date).map(write).join("");
  return `${negative ? "-" : ""}P${parts.slice(0, date).map(write).join("")}${clock && `T${clock}`}`;
}

/**
 * An interval parameter's text as Postgres reads it, the inverse of `duration` for a negative
 * interval (the round-trip rule): Postgres refuses a sign before the `P` (`-P1DT2H`, as `.run()`
 * returns it and Temporal writes it), and reads each part signed instead (`P-1DT-2H`). Any other
 * text is left as it is.
 */
export function postgresInterval(text: string): string {
  const m = /^\s*([+-])(P[^+-]*)$/i.exec(text);
  if (m === null) return text;
  return m[1] === "+" ? m[2]! : m[2]!.replace(/(\d+(?:[.,]\d+)?)([YMWDHS])/gi, "-$1$2");
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

/**
 * An hstore's text as an object of its keys: `"a"=>"1", "b c"=>NULL` is `{ a: "1", "b c": null }`.
 * The server quotes every key and value, escapes `"` and `\` with a backslash, writes a null value
 * as a bare NULL and separates the pairs with `, `. A key such as `__proto__` is a key like any other.
 */
export function hstore(x: string): Record<string, string | null> {
  const pairs: [string, string | null][] = [];
  const n = x.length;
  let i = 0;
  const quoted = (): string => {
    let s = "";
    let from = ++i;
    while (i < n && x[i] !== '"') {
      if (x[i] === "\\") (s += x.slice(from, i)), (from = ++i);
      i++;
    }
    s += x.slice(from, i++);
    return s;
  };
  while (i < n) {
    while (x[i] === " " || x[i] === ",") i++;
    if (i >= n || x[i] !== '"') break;
    const key = quoted();
    i += 2;
    if (x[i] === '"') pairs.push([key, quoted()]);
    else (pairs.push([key, null]), (i += 4));
  }
  return Object.fromEntries(pairs);
}

/**
 * An hstore, as `.run()` returns it, in the text its input reads: each key and value quoted, `"`
 * and `\` escaped, a null value a bare NULL. A key whose value is undefined is left out, as JSON
 * leaves it out.
 */
export function hstoreText(v: object, fail: (message: string) => never): string {
  // Only a plain object's own keys are its pairs: a Map, a Set or a class instance would be written
  // as an empty hstore, so anything else is refused, its constructor named.
  const proto = Object.getPrototypeOf(v);
  if (proto !== Object.prototype && proto !== null) {
    const name = Array.isArray(v) || v instanceof Date || ArrayBuffer.isView(v) ? undefined : (v.constructor as { name?: unknown } | undefined)?.name;
    return fail(`is an hstore, an object of its keys whose values are strings or null; got ${typeof name === "string" && name ? `a ${name}` : describe(v)}.`);
  }
  const quote = (s: string) => `"${s.replace(/[\\"]/g, "\\$&")}"`;
  const pairs: string[] = [];
  for (const [key, value] of Object.entries(v)) {
    if (value === undefined) continue;
    if (value !== null && typeof value !== "string") fail(`is an hstore, whose values are strings or null, and its key ${JSON.stringify(key)} holds ${describe(value)}.`);
    pairs.push(`${quote(key)}=>${value === null ? "NULL" : quote(value)}`);
  }
  return pairs.join(", ");
}

// Postgres's geometric types from their text (geo_ops.c): a point `(x,y)`, a line `{a,b,c}`, a
// segment `[(x1,y1),(x2,y2)]`, a box `(x1,y1),(x2,y2)` with its upper right corner first, a path
// `[…]` when open and `(…)` when closed, a polygon `(…)`, a circle `<(x,y),r>`; each number as
// float8 writes it (`1.5`, `-2`, `1e+300`, `Infinity`, `NaN`), which `+` reads.
type Point = { x: number; y: number };
const POINTS = /\(([^,()]+),([^,()]+)\)/g;
const points = (x: string): Point[] => Array.from(x.matchAll(POINTS), (m) => ({ x: +m[1]!, y: +m[2]! }));
const GEOMETRIC_DECODERS: Readonly<Record<Geometric, Decoder>> = {
  point: (x) => points(x)[0],
  line: (x) => {
    const [a, b, c] = x.slice(1, -1).split(",").map(Number);
    return { a, b, c };
  },
  lseg: points,
  box: (x) => {
    const [high, low] = points(x);
    return { high, low };
  },
  path: (x) => ({ closed: x[0] === "(", points: points(x) }),
  polygon: points,
  circle: (x) => ({ center: points(x)[0], radius: +x.slice(x.lastIndexOf(",") + 1, -1) }),
};

/**
 * The text of each field of a Postgres row value, `(1,"a b",,"x""y")` (record_out), null where a
 * field is empty: a field is quoted when it holds a comma, a parenthesis, a quote, a backslash or
 * white space, or is empty, and a quote inside quotes is doubled; a backslash escapes the next
 * character anywhere. `count` is the row's number of fields, since one NULL field and none both
 * print `()`.
 */
export function parseRow(text: string, count: number, ends = ",)"): (string | null)[] {
  const out: (string | null)[] = [];
  let i = 1;
  for (let f = 0; f < count; f++) {
    if (f) i++;
    if (ends.includes(text[i]!)) {
      out.push(null);
      continue;
    }
    let s = "";
    let quoted = false;
    let from = i;
    while (i < text.length) {
      const c = text[i];
      if (c === "\\") {
        s += text.slice(from, i) + (text[i + 1] ?? "");
        from = i += 2;
      } else if (c === '"') {
        s += text.slice(from, i);
        if (quoted && text[i + 1] === '"') (s += '"'), (i += 2);
        else (quoted = !quoted), i++;
        from = i;
      } else if (!quoted && ends.includes(c!)) break;
      else i++;
    }
    out.push(s + text.slice(from, i));
  }
  return out;
}

// A Postgres range: `empty`, or its two bounds between `[` or `(` and `]` or `)`, each quoted as a
// row value's field is and absent where that end is unbounded (range_out); a multirange its ranges
// in braces, `{[1,3),[5,7)}`. As an object, a range is `{ empty: true }`, or its `lower` and `upper`
// bounds, each in its subtype's own form and left out where that end is unbounded, and `bounds`,
// the brackets Postgres writes around them, which its range constructors take (`int4range(1, 5,
// '[)')`): an unbounded end is always exclusive.
export const RANGE_BOUNDS = ["[)", "[]", "(]", "()"] as const;

/** A range's kind, `range:<subtype's kind>`, and a multirange's, `multirange:<subtype's kind>`. */
export const isRangeKind = (kind: Kind): boolean => kind.startsWith("range:");
export const isMultirangeKind = (kind: Kind): boolean => kind.startsWith("multirange:");
const subtypeKind = (kind: Kind): Kind => kind.slice(kind.indexOf(":") + 1);

/** A range from its text, each bound by `bound`. */
function parseRange(text: string, bound: Decoder): Record<string, unknown> {
  if (text === "empty") return { empty: true };
  const [lower, upper] = parseRow(text, 2, ",)]");
  const range: Record<string, unknown> = {};
  if (lower !== null && lower !== undefined) range.lower = bound(lower);
  if (upper !== null && upper !== undefined) range.upper = bound(upper);
  range.bounds = text[0]! + text[text.length - 1]!;
  return range;
}

/** Each range's text in a multirange's, `{[1,3),[5,7)}`: a range ends at the first `]` or `)` outside quotes. */
function multirangeParts(text: string): string[] {
  const parts: string[] = [];
  let i = 1;
  while (i < text.length && text[i] !== "}") {
    const from = i;
    let quoted = false;
    for (i++; i < text.length; i++) {
      const c = text[i];
      if (c === "\\") i++;
      else if (c === '"') quoted = !quoted;
      else if (!quoted && (c === "]" || c === ")")) break;
    }
    parts.push(text.slice(from, ++i));
    if (text[i] === ",") i++;
  }
  return parts;
}

const rowDecoders = new Map<Kind, Decoder>();
/** A row value of `kind` from its text, as an object of its fields, each from its text as a column of its kind. */
function rowDecoder(kind: Kind): Decoder {
  let d = rowDecoders.get(kind);
  if (d !== undefined) return d;
  const fields = rowFields(kind);
  const decoders = fields.map(([, k, delim]) => textDecoder(k, delim));
  d = (x) => {
    const texts = parseRow(x, fields.length);
    const row: Record<string, unknown> = {};
    fields.forEach(([name], i) => {
      const text = texts[i];
      setKey(row, name, text === null || text === undefined ? null : decoders[i]!(text));
    });
    return row;
  };
  rowDecoders.set(kind, d);
  return d;
}

/** `row[name] = v`, a field named `__proto__` among them, which an assignment would take for the object's prototype. */
function setKey(row: Record<string, unknown>, name: string, v: unknown): void {
  if (name === "__proto__") Object.defineProperty(row, name, { value: v, enumerable: true, writable: true, configurable: true });
  else row[name] = v;
}

// The text Postgres's input functions read, for a value Reylo writes as text itself: a row value,
// a geometric type and a range, from the object `.run()` returns for them.

/** A number as float8's input reads it: `Infinity`, `-Infinity` and `NaN` as JavaScript writes them, and -0 with its sign. */
const floatText = (n: number): string => (Object.is(n, -0) ? "-0" : String(n));
const isObject = (v: unknown): v is Record<string, unknown> => v !== null && typeof v === "object" && !Array.isArray(v) && !(v instanceof Uint8Array);
/** Whether `v` is an object of exactly the keys `keys`. */
const hasKeys = (v: unknown, keys: readonly string[]): v is Record<string, unknown> => isObject(v) && Object.keys(v).length === keys.length && keys.every((k) => k in v);

/** A geometric type's text from its object, or undefined where `v` is not one. */
function geometricText(shape: Geometric, v: unknown): string | undefined {
  const num = (n: unknown) => (typeof n === "number" ? floatText(n) : undefined);
  const pt = (p: unknown) => {
    if (!hasKeys(p, ["x", "y"])) return undefined;
    const x = num(p.x);
    const y = num(p.y);
    return x === undefined || y === undefined ? undefined : `(${x},${y})`;
  };
  const pts = (a: unknown, count?: number) => {
    if (!Array.isArray(a) || !a.length || (count !== undefined && a.length !== count)) return undefined;
    const each = a.map(pt);
    return each.includes(undefined) ? undefined : each.join(",");
  };
  const all = (...parts: (string | undefined)[]) => (parts.includes(undefined) ? undefined : parts.join(","));
  switch (shape) {
    case "point":
      return pt(v);
    case "line": {
      if (!hasKeys(v, ["a", "b", "c"])) return undefined;
      const abc = all(num(v.a), num(v.b), num(v.c));
      return abc === undefined ? undefined : `{${abc}}`;
    }
    case "lseg": {
      const ends = pts(v, 2);
      return ends === undefined ? undefined : `[${ends}]`;
    }
    case "box":
      return hasKeys(v, ["high", "low"]) ? all(pt(v.high), pt(v.low)) : undefined;
    case "path": {
      if (!hasKeys(v, ["closed", "points"]) || typeof v.closed !== "boolean") return undefined;
      const each = pts(v.points);
      return each === undefined ? undefined : v.closed ? `(${each})` : `[${each}]`;
    }
    case "polygon": {
      const each = pts(v);
      return each === undefined ? undefined : `(${each})`;
    }
    case "circle": {
      if (!hasKeys(v, ["center", "radius"])) return undefined;
      const parts = all(pt(v.center), num(v.radius));
      return parts === undefined ? undefined : `<${parts}>`;
    }
  }
}

/** A range's text from its object: `empty`, or its bounds, each quoted, an unbounded end empty. */
function rangeText(t: Extract<EngineType, { kind: "range" | "multirange" }>, v: unknown, fail: (message: string) => never): string {
  const what = `${bareName(t) || "a range"}, { empty: true } or { lower, upper, bounds } (each bound left out where that end is unbounded, bounds one of ${RANGE_BOUNDS.map((b) => JSON.stringify(b)).join(", ")})`;
  if (!isObject(v)) return fail(`${what}; got ${describe(v)}.`);
  const keys = Object.keys(v).filter((k) => v[k] !== undefined);
  if (keys.length === 1 && v.empty === true) return "empty";
  if (keys.some((k) => k !== "lower" && k !== "upper" && k !== "bounds")) return fail(`${what}; got { ${keys.join(", ")} }.`);
  // Bounds left out are `[)`, as Postgres's range constructors take them.
  const bounds = v.bounds ?? "[)";
  if (!(RANGE_BOUNDS as readonly unknown[]).includes(bounds)) return fail(`${what}; got bounds ${describe(bounds)}.`);
  const bound = (key: "lower" | "upper") => (v[key] === undefined ? "" : v[key] === null ? fail(`${what}, whose ${key} is a bound, never null; leave it out for an unbounded end.`) : quoted(postgresLiteral(t.subtype, v[key], (message) => fail(`${what}, whose ${key} is ${message}`))));
  return `${(bounds as string)[0]}${bound("lower")},${bound("upper")}${(bounds as string)[1]}`;
}

/** A value inside an array's or a row value's text: quoted, with each quote and backslash escaped. */
const quoted = (text: string) => `"${text.replace(/["\\]/g, "\\$&")}"`;

/** Whether a value of `t` is itself a JavaScript array: a vector, a segment, a polygon. */
const arrayValued = (t: EngineType) => ["vector", "lseg", "polygon"].includes(columnKind(t, "postgres"));

/**
 * A value of the Postgres type `t` as the text its input function reads: a row value from an
 * object of its fields, a geometric type from its object, and each part of either as a parameter
 * of its type takes it (binary data as `\x` and its hex, a date or a timestamp with its year as
 * Postgres writes it, an interval from its ISO 8601 text, JSON from its value). `fail` says why
 * `v` is not such a value, after "is".
 */
export function postgresLiteral(t: EngineType, v: unknown, fail: (message: string) => never): string {
  const shape = geometric(t);
  if (shape !== undefined) return geometricText(shape, v) ?? fail(`a ${shape}, ${TS_TYPE[shape]}; got ${describe(v)}.`);
  const kind = columnKind(t, "postgres");
  if (t.kind === "record" && isRowKind(kind)) {
    const name = t.label ?? "a row value";
    if (!isObject(v)) return fail(`${name}, an object of its fields (${t.fields.map((f) => f.name).join(", ")}); got ${describe(v)}.`);
    const names = new Set(t.fields.map((f) => f.name));
    const extra = Object.keys(v).find((key) => !names.has(key));
    if (extra !== undefined) fail(`${name}, which has no field ${JSON.stringify(extra)}; its fields are ${t.fields.map((f) => f.name).join(", ")}.`);
    // A field left out, or undefined, is NULL, as an empty field is in the text.
    return `(${t.fields.map((f) => {
      const field = Object.prototype.hasOwnProperty.call(v, f.name) ? v[f.name] : undefined;
      return field === null || field === undefined ? "" : quoted(postgresLiteral(f.type, field, (message) => fail(`${name}, whose field ${JSON.stringify(f.name)} is ${message}`)));
    }).join(",")})`;
  }
  if (t.kind === "range") return rangeText(t, v, fail);
  if (t.kind === "multirange") {
    if (!Array.isArray(v)) return fail(`${bareName(t) || "a multirange"}, an array of ranges; got ${describe(v)}.`);
    return `{${v.map((r) => rangeText(t, r, fail)).join(",")}}`;
  }
  if (t.kind === "array" && kind.endsWith("[]")) {
    const element = t.element.type;
    const delim = delimiterOf(element) ?? ",";
    const write = (a: unknown): string => {
      if (!Array.isArray(a)) return fail(`an array; got ${describe(a)}.`);
      return `{${a.map((e) => (e === null || e === undefined ? "NULL" : Array.isArray(e) && !arrayValued(element) ? write(e) : quoted(postgresLiteral(element, e, fail)))).join(delim)}}`;
    };
    return write(v);
  }
  if (t.kind === "json") return JSON.stringify(v);
  if (kind === "hstore" && isObject(v)) return hstoreText(v, (message) => fail(message.replace(/^is /, "")));
  if (kind === "vector" && Array.isArray(v) && v.every((n) => typeof n === "number" && Number.isFinite(n))) return JSON.stringify(v);
  if (v instanceof Uint8Array) {
    let hex = "\\x";
    for (const byte of v) hex += (byte < 16 ? "0" : "") + byte.toString(16);
    return hex;
  }
  switch (typeof v) {
    case "boolean":
      return v ? "t" : "f";
    case "number":
      return floatText(v);
    case "bigint":
      return String(v);
    case "string":
      if (kind === "date" || kind === "timestamp" || kind === "timestamptz") return postgresYear(v);
      return kind === "interval" ? postgresInterval(v) : v;
  }
  return fail(`${bareName(t) || t.kind}; got ${describe(v)}.`);
}

/** A MySQL SET from its text, its members joined by commas (`a,c`), none for an empty one. */
const members = (x: string): Set<string> => new Set(x === "" ? [] : x.split(","));

const TEXT_DECODERS: Readonly<Record<ScalarKind, Decoder>> = {
  bool: (x) => x === "t",
  int: number,
  float: number,
  int8: (x) => BigInt(x),
  decimal: identity,
  text: identity,
  date,
  timestamp,
  timestamptz: instant,
  interval: duration,
  json: (x) => JSON.parse(x),
  bytes: bytea,
  // pgvector prints the shortest text of each float, `[1,2.5,1e-07]`, which JSON reads exactly.
  vector: (x) => JSON.parse(x),
  hstore,
  ...GEOMETRIC_DECODERS,
  enum: identity,
  set: (x) => members(x),
  geometry: identity,
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

/**
 * A Postgres array held to the dimensions its kind declares, one per `[]`. Postgres lets an array
 * column hold any number of dimensions, whatever it declares (one `int4[]` column holds `{1,2}` and
 * `{{1,2},{3,4}}`), and Reylo types a column's value as one dimension, which nearly every array
 * column holds, so a value of more is refused, naming the column. An array the statement builds
 * of arrays (`array_agg` over an array column) is declared with a `[]` per level, and holds them.
 * Postgres's arrays are rectangular, so the depth of the first element at each level is every one's.
 * A vector element is an array itself, one level more; a JSON element may be one at any depth,
 * which its type holds whatever the dimensions around it.
 */
export function withinDimensions(v: unknown, kind: Kind, column: string): unknown {
  const element = elementKind(kind);
  if (!Array.isArray(v) || element === "json") return v;
  const declared = (kind.length - element.length) / 2;
  let depth = element === "vector" ? -1 : 0;
  for (let level: unknown = v; Array.isArray(level); level = level.find((e) => e !== null)) depth++;
  if (depth <= declared) return v;
  // An array built of arrays is too deep only where an array column it collects is.
  throw new DecodeError(
    declared === 1
      ? `Column "${column}" holds an array of ${depth} dimensions, and its type has one: Reylo types a Postgres array column with one dimension, which Postgres doesn't enforce. Select to_jsonb(${column}) to read it as JSON, or set allow_nested_arrays: true in .reylo.yaml to type it at any depth.`
      : `Column "${column}" holds an array of ${depth} dimensions, and its type has ${declared}: an array column it collects holds more than one dimension, which Postgres doesn't enforce. Collect to_jsonb(<that column>) instead to read it as JSON.`,
  );
}

/**
 * The kind of an array's innermost elements, `int` for `int[][]`: Postgres's text of an array
 * nests its levels itself, so every element it spells is one of these.
 */
export function elementKind(kind: Kind): Kind {
  return kind.replace(/(?:\[\])+$/, "") as Kind;
}

/** The decoder of a Postgres value of `kind` from its text; `delim` is an array's (`;` for `box[]`). */
export function textDecoder(kind: Kind, delim = ","): Decoder {
  if (kind.endsWith("[]")) {
    const element = textDecoder(elementKind(kind), delim);
    return (x) => parseArray(x, element, delim);
  }
  if (isRowKind(kind)) return rowDecoder(kind);
  if (isRangeKind(kind)) {
    const bound = textDecoder(subtypeKind(kind));
    return (x) => parseRange(x, bound);
  }
  if (isMultirangeKind(kind)) {
    const bound = textDecoder(subtypeKind(kind));
    return (x) => multirangeParts(x).map((r) => parseRange(r, bound));
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
 * A Date a driver read as UTC (Bun.sql's date and timestamp) back to the text the server's would
 * decode to: `YYYY-MM-DD`, or with `T` and the fraction the driver kept, the year as `isoYear`
 * writes it.
 */
function utcText(d: Date, kind: "date" | "timestamp", column: string, postgres: boolean): string {
  validDate(d, column);
  const date = `${isoYear(d.getUTCFullYear())}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}`;
  if (kind === "date") return date;
  const ms = d.getUTCMilliseconds();
  // A Postgres `timestamp`'s six digits, as `sixDigits` writes them; a MySQL DATETIME's as the driver kept them.
  const fraction = postgres ? `.${pad(ms, 3)}000` : ms ? `.${pad(ms, 3).replace(/0+$/, "")}` : "";
  return `${date}T${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())}${fraction}`;
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
 * `delim` is an array's, where its text is what the driver gave; `postgres` says the value is a
 * Postgres one, whose `timestamp` takes six fraction digits.
 */
export function coerce(kind: Kind, v: unknown, column: string, delim = ",", postgres = false): unknown {
  if (v === null || v === undefined) return null;
  if (kind.endsWith("[]")) {
    const element = elementKind(kind);
    if (typeof v === "string") return parseArray(v, (s) => coerce(element, s, column, ",", postgres), delim);
    if (Array.isArray(v)) return v.map((e) => (Array.isArray(e) ? coerce(kind, e, column, delim, postgres) : coerce(element, e, column, ",", postgres)));
    // A MySQL VECTOR's bytes: no driver gives any other array of floats as bytes.
    if (element === "float" && isBytes(v)) return float32s(v, column);
    // Bun.sql's int4[] and float4[] are typed arrays.
    if (ArrayBuffer.isView(v) && !(v instanceof DataView)) return Array.from(v as unknown as ArrayLike<unknown>, (e) => coerce(element, e, column));
    return v;
  }
  // A row value, and a geometric type, are their text where no driver parses them.
  if (isRowKind(kind) || isRangeKind(kind) || isMultirangeKind(kind)) return typeof v === "string" ? textDecoder(kind)(v) : v;
  if (GEOMETRIC.has(kind)) return typeof v === "string" ? GEOMETRIC_DECODERS[kind as Geometric](v) : v;
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
      if (v instanceof Date) return utcText(v, kind as "date" | "timestamp", column, postgres);
      // Bun.sql's `infinity`.
      if (v === Infinity || v === -Infinity) return v > 0 ? "infinity" : "-infinity";
      return v;
    case "timestamptz":
      if (typeof v === "string") return instant(v);
      if (v instanceof Date) return instantOfDate(v, column);
      // Bun.sql's `infinity`.
      if (v === Infinity || v === -Infinity) return v > 0 ? "infinity" : "-infinity";
      return v;
    case "interval":
      return typeof v === "string" ? duration(v) : v;
    case "json":
    case "vector":
      return typeof v === "string" ? JSON.parse(v) : v;
    case "hstore":
      return typeof v === "string" ? hstore(v) : v;
    case "bytes":
      if (isBytes(v)) return v;
      if (v instanceof ArrayBuffer) return new Uint8Array(v);
      // D1's BLOB is an array of numbers; mariadb's and Bun.sql's BIT(1) a truth value.
      if (Array.isArray(v)) return Uint8Array.from(v as number[]);
      if (typeof v === "boolean") return Uint8Array.of(v ? 1 : 0);
      if (typeof v === "string") return bytea(v);
      return v;
    case "enum":
      // MySQL stores `''` for a value an ENUM does not list, under INSERT IGNORE or outside strict
      // mode, which the column's union does not admit.
      if (v === "") throw new DecodeError(`Column "${column}" holds "", which is none of its ENUM's labels: MySQL stores it for a value the column does not list, under INSERT IGNORE or outside strict mode. Select nullif(${column}, '') to read such a row.`);
      return v;
    case "set":
      // mysql2's SET is its text, mariadb's an array of its members.
      if (typeof v === "string") return members(v);
      if (Array.isArray(v)) return new Set(v as unknown[]);
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
  if (v instanceof Date) return kinds.includes("timestamp") ? utcText(v, "timestamp", column, false) : kinds.includes("date") ? utcText(v, "date", column, false) : kinds.includes("timestamptz") ? instantOfDate(v, column) : v;
  if (v instanceof ArrayBuffer || (Array.isArray(v) && kinds.includes("bytes"))) return coerce("bytes", v, column);
  return v;
}
