// Reylo runs its own SQL Server queries on the app's `mssql` pool, transaction or request, and
// decodes every value itself (the representation is src/values.ts), reading each column's type from
// the result's own metadata:
//
// - a `bigint` comes from tedious as its digits, and becomes a BigInt;
// - a `decimal`, `numeric`, `money` or `smallmoney` comes as its exact text where the pool's
//   tedious has `options.decimalAsString` on, and is refused as a number otherwise, which can't
//   hold every value;
// - a `date`, `time`, `datetime2`, `datetime` and `smalldatetime` come as a Date read in UTC (the
//   pool's `options.useUTC`, on by default), with the digits past the millisecond in the Date's
//   `nanosecondsDelta`, and become their text: `YYYY-MM-DD`, `HH:MM:SS.fffffff`, and the two with
//   `T` between them, to the column's scale;
// - a `datetimeoffset` comes as the instant, with the offset it was written with in the Date's
//   `offsetMinutes`, and becomes its text with that offset (`2024-01-02T03:04:05.1230000+05:30`);
// - a `uniqueidentifier` becomes its text as SQL Server writes it, in capitals;
// - binary data is a Buffer, which is a Uint8Array.
//
// Each parameter is bound with the type the analysis deduced for it (`input(name, type, value)`),
// from the `TYPES` of the app's own `mssql`, which the module imports (src/loader.ts), so that a
// `varchar` column is compared with a `varchar` and a decimal parameter keeps its scale:
//
// - a text or binary value longer than the deduced length is declared at its own, so that the
//   server meets it as SQL Server would (a comparison matches nothing, a write fails with error
//   2628) rather than the protocol;
// - a decimal is written exactly or refused: a tedious before `options.decimalAsString` writes one
//   through a JavaScript number, so a value that doesn't survive that needs the version that has
//   it, which the pool proves once by reading a decimal back as its text;
// - a `datetimeoffset` is a Date of its instant whose `offsetMinutes`, and `getTimezoneOffset()`
//   for a tedious from before it reads that, give the offset it was written with.
import { DecodeError, describe, type Columns, type Kind } from "./values.ts";
import { bareName, type EngineType } from "./engine-type.ts";
import { readSqlserverError } from "./query-error.ts";
import type { AnyClient as Client, Bound, QueryModule, Runtime } from "./execute.ts";

type AnyClient = Record<string, any>;

/** The `TYPES` of `mssql`: each SQL type's factory, `Decimal(precision, scale)`. */
export type MssqlTypes = Readonly<Record<string, ((...args: any[]) => unknown) & { declaration?: string }>>;

/** A column of a result as `mssql` describes it in `arrayRowMode`. */
type MssqlColumn = { name: string; type?: { declaration?: string }; scale?: number; length?: number };

/** The request a statement runs on: a pool's or a transaction's new one, or a request itself. */
function requestOf(c: AnyClient): AnyClient | undefined {
  if (typeof c.input === "function" && typeof c.query === "function" && typeof c.batch === "function") return c;
  // A ConnectionPool (connect()) or a Transaction (begin()) of mssql's: request() makes a request on it.
  if (typeof c.request === "function" && (typeof c.connect === "function" || typeof c.begin === "function")) return c.request();
  return undefined;
}

/** The pool's settings, through a transaction's or a request's `parent`. */
function optionsOf(c: AnyClient): { useUTC?: boolean; decimalAsString?: boolean } | undefined {
  let at: AnyClient | undefined = c;
  for (let depth = 0; at && depth < 4; depth++) {
    if (at.config?.options) return at.config.options;
    at = at.parent;
  }
  return undefined;
}

/** A Uint8Array that is not a Buffer as a Buffer over the same memory: tedious takes a Buffer alone. */
function asBuffer(v: unknown): unknown {
  const NodeBuffer = (globalThis as { Buffer?: { isBuffer(v: unknown): boolean; from(buffer: ArrayBufferLike, offset: number, length: number): Uint8Array } }).Buffer;
  return v instanceof Uint8Array && NodeBuffer !== undefined && !NodeBuffer.isBuffer(v) ? NodeBuffer.from(v.buffer, v.byteOffset, v.byteLength) : v;
}

/** A string's length in UTF-8, which no code page's encoding of it exceeds. */
function utf8Length(s: string): number {
  let n = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    n += c < 0x80 ? 1 : c < 0x800 ? 2 : c >= 0xd800 && c < 0xdc00 ? (i++, 4) : 3;
  }
  return n;
}

/** The SQL type of a parameter's engine type, by its label and size, as `mssql`'s factory makes it, for the value `v` it binds. */
function mssqlType(types: MssqlTypes, t: EngineType, v: unknown): unknown {
  const name = bareName(t);
  const size = (t as { size?: { max?: number; exact?: number } }).size;
  const length = size?.exact ?? size?.max;
  const max = (types.MAX as unknown as number) ?? 65535;
  // A value's length, in UTF-16 code units for nchar and nvarchar, in bytes for the rest.
  const units = typeof v === "string" ? v.length : 0;
  const bytes = typeof v === "string" ? utf8Length(v) : v instanceof Uint8Array ? v.byteLength : 0;
  // The declared length: the deduced one, or the value's where that is longer, up to `limit`.
  const fit = (measured: number, limit: number) => (length === undefined ? max : measured <= length ? length : measured <= limit ? measured : max);
  // A time's scale, from its tick: `1/1000` is 3 digits past the second.
  const tick = (t as { tick?: string }).tick;
  const scale = tick === "1" ? 0 : tick?.match(/^1\/1(0+)$/)?.[1]?.length;
  switch (name) {
    case "bit":
      return types.Bit?.();
    case "tinyint":
      return types.TinyInt?.();
    case "smallint":
      return types.SmallInt?.();
    case "int":
      return types.Int?.();
    case "bigint":
      return types.BigInt?.();
    case "decimal":
    case "numeric": {
      const d = t as { precision?: number; max_scale?: number };
      return (name === "decimal" ? types.Decimal : types.Numeric)?.(d.precision ?? 18, d.max_scale ?? 0);
    }
    case "money":
      return types.Money?.();
    case "smallmoney":
      return types.SmallMoney?.();
    case "float":
      return types.Float?.();
    case "real":
      return types.Real?.();
    case "char":
      return bytes > (length ?? 1) ? types.VarChar?.(fit(bytes, 8000)) : types.Char?.(length ?? 1);
    case "varchar":
      return types.VarChar?.(fit(bytes, 8000));
    case "nchar":
      return units > (length ?? 1) ? types.NVarChar?.(fit(units, 4000)) : types.NChar?.(length ?? 1);
    case "nvarchar":
    case "sysname":
      return types.NVarChar?.(fit(units, 4000));
    case "text":
      return types.Text?.();
    case "ntext":
      return types.NText?.();
    case "binary":
      return bytes > (length ?? 1) ? types.VarBinary?.(fit(bytes, 8000)) : types.Binary?.(length ?? 1);
    case "varbinary":
      return types.VarBinary?.(fit(bytes, 8000));
    case "image":
      return types.Image?.();
    case "uniqueidentifier":
      return types.UniqueIdentifier?.();
    case "date":
      return types.Date?.();
    case "time":
      return types.Time?.(scale ?? 7);
    case "datetime2":
      return types.DateTime2?.(scale ?? 7);
    case "datetime":
      return types.DateTime?.();
    case "smalldatetime":
      return types.SmallDateTime?.();
    case "datetimeoffset":
      return types.DateTimeOffset?.(scale ?? 7);
    case "xml":
      return types.Xml?.();
    // A JSON parameter is its text, which the statement reads as an nvarchar(max).
    case "json":
    case "jsonb":
      return types.NVarChar?.(max);
    default:
      return undefined;
  }
}

const TIME = /^(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?$/;
const DATE = /^([+-]?\d{4,6})-(\d{2})-(\d{2})$/;
const DATETIME = /^([+-]?\d{4,6})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?$/;
const ZONED = /^([+-]?\d{4,6})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?\s*(?:Z|([+-])(\d{2}):?(\d{2}))$/i;

/** A Date of the UTC fields given, with the digits past the millisecond in tedious's `nanosecondDelta`, which it writes. */
function utcDate(year: number, month: number, day: number, h: number, m: number, s: number, fraction: string | undefined): Date {
  const digits = (fraction ?? "").padEnd(7, "0").slice(0, 7);
  const d = new Date(0);
  d.setUTCFullYear(year, month - 1, day);
  d.setUTCHours(h, m, s, Number(digits.slice(0, 3)));
  Object.defineProperty(d, "nanosecondDelta", { value: Number(digits.slice(3)) / 1e7, enumerable: false });
  return d;
}

/** A parameter's value as tedious binds its type: a date or a time as a Date of its fields, read in UTC as the pool reads them. */
function bindValue(t: EngineType, v: unknown, name: string): unknown {
  if (v === null || v === undefined || typeof v !== "string") return asBuffer(v);
  const label = bareName(t);
  let m: RegExpExecArray | null;
  switch (label) {
    case "date":
      if ((m = DATE.exec(v))) return utcDate(num(m[1]), num(m[2]), num(m[3]), 0, 0, 0, undefined);
      break;
    case "time":
      if ((m = TIME.exec(v))) return utcDate(1970, 1, 1, num(m[1]), num(m[2]), num(m[3]), m[4]);
      break;
    case "datetime2":
    case "datetime":
    case "smalldatetime":
      if ((m = DATETIME.exec(v))) return utcDate(num(m[1]), num(m[2]), num(m[3]), num(m[4]), num(m[5]), num(m[6]), m[7]);
      break;
    case "datetimeoffset":
      if ((m = ZONED.exec(v))) {
        const offset = m[8] === undefined ? 0 : (m[8] === "-" ? -1 : 1) * (num(m[9]) * 60 + num(m[10]));
        const local = utcDate(num(m[1]), num(m[2]), num(m[3]), num(m[4]), num(m[5]), num(m[6]), m[7]);
        const instant = new Date(local.getTime() - offset * 60000);
        Object.defineProperty(instant, "nanosecondDelta", { value: (local as Date & { nanosecondDelta: number }).nanosecondDelta, enumerable: false });
        Object.defineProperty(instant, "offsetMinutes", { value: offset, enumerable: false });
        // A tedious from before it reads offsetMinutes writes the offset getTimezoneOffset() gives.
        Object.defineProperty(instant, "getTimezoneOffset", { value: () => -offset, enumerable: false });
        return instant;
      }
      break;
    default:
      return v;
  }
  throw new TypeError(`Parameter "${name}" is a SQL Server ${label}, and ${JSON.stringify(v)} is not its text.`);
}

const pad = (n: number, width = 2) => String(n).padStart(width, "0");

/** A matched group's number; a group the pattern requires is always there. */
const num = (group: string | undefined) => Number(group);

/** A year as ISO 8601 writes it: four digits from 0000 to 9999. SQL Server's run from 0001 to 9999. */
const year = (y: number) => pad(y, 4);

/** The digits past the second of a Date tedious read, to `scale` places: its milliseconds and the `nanosecondsDelta` beside them. */
function fraction(d: Date, scale: number): string {
  if (scale <= 0) return "";
  const ns = (d as Date & { nanosecondsDelta?: number }).nanosecondsDelta ?? 0;
  const ticks = d.getUTCMilliseconds() * 10000 + Math.round(ns * 1e7);
  return "." + String(ticks).padStart(7, "0").slice(0, scale);
}

/** A column's value as its representation, by the SQL type its metadata names. */
function decodeValue(v: unknown, column: MssqlColumn, kind: Kind | undefined, name: string): unknown {
  if (v === null || v === undefined) return null;
  const type = column.type?.declaration?.toLowerCase();
  const scale = column.scale ?? 7;
  switch (type) {
    case "bigint":
      return typeof v === "string" || typeof v === "number" ? BigInt(v) : v;
    case "decimal":
    case "numeric":
    case "money":
    case "smallmoney":
      if (typeof v !== "string") throw new DecodeError(`Column "${name}" is a SQL Server ${type}, which tedious gave as ${describe(v)}: Reylo returns its exact text, which tedious gives with the pool's options.decimalAsString set to true.`);
      return v;
    case "date":
      if (v instanceof Date) return `${year(v.getUTCFullYear())}-${pad(v.getUTCMonth() + 1)}-${pad(v.getUTCDate())}`;
      break;
    case "time":
      if (v instanceof Date) return `${pad(v.getUTCHours())}:${pad(v.getUTCMinutes())}:${pad(v.getUTCSeconds())}${fraction(v, scale)}`;
      break;
    case "datetime2":
    case "datetime":
    case "smalldatetime": {
      if (!(v instanceof Date)) break;
      const digits = type === "datetime" ? 3 : type === "smalldatetime" ? 0 : scale;
      return `${year(v.getUTCFullYear())}-${pad(v.getUTCMonth() + 1)}-${pad(v.getUTCDate())}T${pad(v.getUTCHours())}:${pad(v.getUTCMinutes())}:${pad(v.getUTCSeconds())}${fraction(v, digits)}`;
    }
    case "datetimeoffset": {
      if (!(v instanceof Date)) break;
      const offset = (v as Date & { offsetMinutes?: number }).offsetMinutes;
      if (offset === undefined) throw new DecodeError(`Column "${name}" is a SQL Server datetimeoffset, and the pool's tedious gave its instant without the offset it was written with: Reylo returns both, which tedious gives on a Date's offsetMinutes from the version that adds it.`);
      const local = new Date(v.getTime() + offset * 60000);
      Object.defineProperty(local, "nanosecondsDelta", { value: (v as Date & { nanosecondsDelta?: number }).nanosecondsDelta ?? 0 });
      const sign = offset < 0 ? "-" : "+";
      const abs = Math.abs(offset);
      return `${year(local.getUTCFullYear())}-${pad(local.getUTCMonth() + 1)}-${pad(local.getUTCDate())}T${pad(local.getUTCHours())}:${pad(local.getUTCMinutes())}:${pad(local.getUTCSeconds())}${fraction(local, scale)}${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}`;
    }
    case "uniqueidentifier":
      return typeof v === "string" ? v.toUpperCase() : v;
    default:
      return v;
  }
  throw new DecodeError(`Column "${name}" is a SQL Server ${type}, which tedious gave as ${describe(v)}${kind ? ` where Reylo reads a ${kind}` : ""}.`);
}

/** A decimal's text in units of its last place at `scale`, rounded half away from zero as SQL Server rounds it, or undefined for text that is no plain decimal. */
function decimalUnits(text: string, scale: number): bigint | undefined {
  const m = /^\s*([+-]?)(\d*)(?:\.(\d*))?\s*$/.exec(text);
  if (!m || (m[2] === "" && !m[3])) return undefined;
  const digits = m[3] ?? "";
  const kept = BigInt((m[2] || "0") + digits.slice(0, scale).padEnd(scale, "0")) + (digits.length > scale && digits[scale]! >= "5" ? 1n : 0n);
  return m[1] === "-" ? -kept : kept;
}

/** Whether a tedious that reads a decimal parameter with parseFloat, as before options.decimalAsString, writes `text` at `scale` exactly: it writes the integer `value * 10 ** scale` is, money's as it is and a decimal's rounded half away from zero, which is exact where that double is the exact value. */
function floatKeeps(text: string, scale: number, money: boolean): boolean {
  const exact = decimalUnits(text, scale);
  // Text that is no plain decimal, such as `1e5`, goes through parseFloat on every tedious.
  if (exact === undefined) return true;
  const value = parseFloat(text);
  const scaled = value * Math.pow(10, scale);
  const written = money ? scaled : Math.sign(value) * Math.round(Math.abs(scaled));
  return Number.isInteger(written) && BigInt(written) === exact;
}

/** Whether each pool's tedious writes a decimal exactly, as the version that reads one as its text does, by pool. */
const exactDecimals = new WeakMap<object, Promise<boolean>>();

/** Whether the tedious of the pool `c` runs on reads a decimal as its text, which the version that writes one exactly also adds: asked once a pool, on the pool or the transaction `c` is or belongs to. */
function writesExactDecimals(c: AnyClient): Promise<boolean> {
  const on: AnyClient | undefined = typeof c.request === "function" ? c : c.parent;
  if (!on || typeof on.request !== "function") return Promise.resolve(false);
  let pool = on;
  while (pool.parent) pool = pool.parent;
  let known = exactDecimals.get(pool);
  if (!known) {
    known = Promise.resolve()
      .then(() => on.request().query("select cast(1.5 as decimal(2, 1)) as d"))
      .then(
        (result) => typeof result?.recordset?.[0]?.d === "string",
        () => {
          exactDecimals.delete(pool);
          return false;
        },
      );
    exactDecimals.set(pool, known);
  }
  return known;
}

/** A decimal parameter's scale and whether it is money, or undefined for a parameter of any other type. */
function decimalScale(t: EngineType): { scale: number; money: boolean } | undefined {
  switch (bareName(t)) {
    case "decimal":
    case "numeric":
      return { scale: (t as { max_scale?: number }).max_scale ?? 0, money: false };
    case "money":
    case "smallmoney":
      return { scale: 4, money: true };
    default:
      return undefined;
  }
}

/** A SQL Server statement's rows on an `mssql` pool, transaction or request, or undefined on any other client. */
async function runSqlserver(c: Client, query: QueryModule<"sqlserver">, { text, values, names }: Bound): Promise<unknown[] | undefined> {
  const request = requestOf(c);
  if (!request) return undefined;
  const options = optionsOf(request);
  if (options?.useUTC === false) throw new TypeError(`reylo reads SQL Server's dates and times in UTC, and this pool reads them in the process's time zone: create it with options.useUTC left on, its default.`);
  const types = query.libraries?.mssql as MssqlTypes | undefined;
  for (const [i, name] of names.entries()) {
    const t = query.inputs?.[name]?.type;
    const v = values[i];
    const decimal = t !== undefined ? decimalScale(t) : undefined;
    if (decimal && typeof v === "string" && !floatKeeps(v, decimal.scale, decimal.money) && !(await writesExactDecimals(c))) {
      throw new TypeError(`Parameter "${name}" is a SQL Server ${bareName(t!)}, which tedious would write as ${parseFloat(v)}: Reylo binds its exact text, which tedious writes with the pool's options.decimalAsString set to true.`);
    }
  }
  names.forEach((name, i) => {
    const t = query.inputs?.[name]?.type;
    const sqlType = types !== undefined && t !== undefined ? mssqlType(types, t, values[i]) : undefined;
    const value = t !== undefined ? bindValue(t, values[i], name) : asBuffer(values[i]);
    if (sqlType === undefined) request.input(`p_${i + 1}`, value);
    else request.input(`p_${i + 1}`, sqlType, value);
  });
  request.arrayRowMode = true;
  const result = await request.query(text);
  const rows: unknown[][] = result?.recordset ?? [];
  const meta: MssqlColumn[] = result?.columns?.[0] ?? [];
  const columns: Columns = query.columns ?? meta.map((m) => [m.name, "unknown"] as const);
  return rows.map((row) => {
    const out: Record<string, unknown> = {};
    columns.forEach(([name, kind], i) => {
      out[name] = decodeValue(row[i], meta[i] ?? { name }, kind, name);
    });
    return out;
  });
}

/** A column's raw value, as a Standard Query's `decode` takes it (src/standard-query.ts): `.run()`'s, which the raw value already is. */
function sqlserverDecoder(name: string, kind: Kind): (v: unknown) => unknown {
  return (v) => {
    if (v === null || v === undefined) return null;
    if (kind === "int8" && (typeof v === "string" || typeof v === "number")) return BigInt(v);
    if (kind === "decimal" && typeof v !== "string") throw new DecodeError(`Column "${name}" holds ${describe(v)}, and a SQL Server decimal's raw value is its exact text, which tedious gives with options.decimalAsString.`);
    return v;
  };
}

/** SQL Server's half of the run time (src/execute.ts, `Runtime`), which `defineSqlserverQuery` passes. */
export const sqlserver: Runtime<"sqlserver"> = { dialect: "sqlserver", run: runSqlserver, readError: readSqlserverError, decoder: sqlserverDecoder };
