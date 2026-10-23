// Structured parameters at run time (wiki/structured-params.md): the value of a `:events::rows(…)`
// or `:user::row(…)` parameter is checked against its column list before the statement runs, and
// written as the JSON the statement reads. A key a row leaves out takes its column's default: on
// Postgres and SQLite the statement holds each such default, copied from the catalog, so it is one
// text whatever keys the rows give; on MySQL an insert's column lists are
// written again without the keys a call leaves out, so every row of a call gives the same keys.
// Nothing here changes a value: a value the database would round, truncate or read in a zone
// nobody chose is refused.
import type { EngineType } from "./engine-type.ts";
import { validate, withDefaults } from "./json-schema.ts";
import { describe, hstoreText, ParameterError, postgresInterval, postgresYear, zoned } from "./values.ts";

export { describe, zoned } from "./values.ts";

type Dialect = "postgres" | "mysql" | "sqlite";
export type RowsType = Extract<EngineType, { kind: "rows" }>;
export type RowField = RowsType["fields"][number];

/** One comma-separated list of an insert's expansion, as UTF-16 offsets into the module's `text`: an entry per member, in the column list's order. */
export type StructuredList = { readonly start: number; readonly end: number; readonly entries: readonly string[] };
/** A structured parameter in insert position and the lists the run time writes again for the keys a call carries. */
export type Structured = { readonly name: string; readonly lists: readonly StructuredList[] };

// BigInt calls, not literals: a project that type-checks these sources for a target below ES2020
// refuses a literal (TS2737), as lab/nextjs's ES2017 does.
const TWO_63 = BigInt(2) ** BigInt(63);
const INTEGERS: Record<string, readonly [bigint, bigint]> = {
  int2: [BigInt(-32768), BigInt(32767)],
  int4: [BigInt(-2147483648), BigInt(2147483647)],
  int8: [-TWO_63, TWO_63 - BigInt(1)],
  // SQLite's INTEGER is a 64-bit integer.
  integer: [-TWO_63, TWO_63 - BigInt(1)],
};

/**
 * A 64-bit integer, which JSON carries as digits: a string on Postgres, which reads one exactly, and
 * a bare number elsewhere (MySQL's `CAST(… AS SIGNED)` wraps a string out of range;
 * wiki/structured-params/verify.md). A Postgres or MySQL `int8` takes a number, a BigInt or a string
 * of digits, as its parameter type says; SQLite's `integer` takes a number or a BigInt (src/render.ts).
 */
const WIDE = new Set(["int8", "integer"]);

/** Raw JSON text for a value the writer puts in as it is. */
class Raw {
  readonly text: string;
  constructor(text: string) {
    this.text = text;
  }
}

/**
 * Checks a structured parameter's value against its column list, and gives the value to bind:
 * on Postgres the JSON value, which the run time binds as it binds every JSON parameter, and on
 * MySQL and SQLite its JSON text. `inserted` is a parameter whose rows a MySQL insert writes, whose
 * lists the run time writes again, so its keys are the same in every row; `present` is the keys
 * they carry.
 */
export function prepareRows(name: string, type: RowsType, value: unknown, dialect: Dialect, inserted: boolean): { value: unknown; present: ReadonlySet<string> } {
  const fail = (message: string): never => {
    throw new ParameterError(`Parameter "${name}": ${message}`);
  };
  if (value === undefined || value === null) fail(type.one ? "pass one object, whose keys the column list names." : "pass an array of objects, whose keys the column list names; an empty array inserts nothing.");
  const rows = type.one ? [value] : Array.isArray(value) ? value : fail(`pass an array of objects, one per row; got ${describe(value)}.`);
  const known = new Map(type.fields.map((f) => [f.name, f]));
  let present: Set<string> | undefined;
  let first = -1;
  const out: Record<string, unknown>[] = [];
  rows.forEach((row: unknown, i: number) => {
    const where = type.one ? "" : `row ${i + 1}: `;
    if (row === null || typeof row !== "object" || Array.isArray(row) || row instanceof Date) fail(`${where}each row is an object whose keys the column list names; got ${describe(row)}.`);
    const r = row as Record<string, unknown>;
    for (const key of Object.keys(r)) {
      if (!known.has(key) && r[key] !== undefined) fail(`${where}the column list names no key "${key}" (it names ${type.fields.map((f) => `"${f.name}"`).join(", ")}).`);
    }
    const keys = new Set<string>();
    const written: Record<string, unknown> = {};
    for (const f of type.fields) {
      const v = r[f.name];
      if (v === undefined) {
        if (!f.optional) fail(`${where}"${f.name}" is ${f.written} not null, and the row leaves it out.`);
        continue;
      }
      keys.add(f.name);
      if (v === null) {
        if (!f.nullable) fail(`${where}"${f.name}" is ${f.written} not null, and the row gives null${f.optional ? `; leave the key out for the column's default` : ""}.`);
        written[f.name] = null;
        continue;
      }
      const checked = check(f.type, f.written, v, dialect, (message) => fail(`${where}"${f.name}" ${message}`));
      // A JSON member's schema fills its defaults and checks the value (wiki/typed-json-params.md).
      if (f.schema !== undefined) {
        const filled = withDefaults(f.schema, checked);
        const failure = validate(f.schema, filled);
        if (failure) fail(`${where}"${f.name}"${failure.path ? ` at ${failure.path}` : ""} ${failure.message}.`);
        written[f.name] = filled;
      } else written[f.name] = checked;
    }
    // One statement inserts every row of a call, so on MySQL, which copies no default, its column
    // list is the same for each: a key one row leaves out and another gives cannot have its
    // default in one and its value in the other.
    if (inserted) {
      if (present === undefined) {
        present = keys;
        first = i;
      } else {
        const differ = type.fields.find((f) => present!.has(f.name) !== keys.has(f.name));
        if (differ) fail(`row ${i + 1} ${keys.has(differ.name) ? "gives" : "leaves out"} "${differ.name}", which row ${first + 1} ${keys.has(differ.name) ? "leaves out" : "gives"}. Every row of one call gives the same keys, since one insert writes them all and a key left out takes its column's default: give "${differ.name}" in every row, or in none.`);
      }
    }
    out.push(written);
  });
  const keys = present ?? new Set(type.fields.map((f) => f.name));
  if (inserted && !type.fields.some((f) => keys.has(f.name))) fail("every key is left out, so the insert would name no column; give one at least.");
  const shaped = type.one ? out[0] : out;
  return { value: dialect === "postgres" ? shaped : write(shaped), present: keys };
}

/** One value checked against its type, as the JSON the database reads; `fail` says why not. */
function check(type: EngineType, written: string, v: unknown, dialect: Dialect, fail: (message: string) => never): unknown {
  switch (type.kind) {
    case "json":
      // JSON.stringify throws on a BigInt and skips a function; a JSON member holds JSON.
      try {
        if (JSON.stringify(v) === undefined) fail(`is ${written}, which holds a JSON value; got ${describe(v)}.`);
      } catch (e) {
        fail(`is ${written}, which holds a JSON value: ${(e as Error).message}.`);
      }
      return v;
    case "enum":
      if (typeof v === "string" && (type.labels.length === 0 || type.labels.includes(v))) return v;
      return fail(`is the enum ${type.name}, one of ${type.labels.map((l) => JSON.stringify(l)).join(", ")}; got ${describe(v)}.`);
    case "array": {
      if (!Array.isArray(v)) return fail(`is ${written}, an array; got ${describe(v)}.`);
      const element = written.replace(/\s*\[\s*\]\s*$/, "");
      return v.map((e, i) => (e === null ? null : check(type.of, element, e, dialect, (message) => fail(`element ${i + 1} ${message}`))));
    }
    case "scalar":
      return scalar(type.name, written, v, dialect, fail);
    default:
      return v;
  }
}

function scalar(name: string, written: string, v: unknown, dialect: Dialect, fail: (message: string) => never): unknown {
  // `a|b` is either, where the database decides by the value.
  if (name.includes("|")) {
    for (const part of name.split("|")) {
      try {
        return scalar(part, written, v, dialect, (message) => {
          throw new TypeError(message);
        });
      } catch {}
    }
    return fail(`is ${written}; got ${describe(v)}.`);
  }
  const range = INTEGERS[name];
  if (range) {
    const n = integer(range, name === "int8", written, v, fail, dialect !== "postgres");
    if (!WIDE.has(name)) return Number(n);
    return dialect === "postgres" ? n.toString() : new Raw(n.toString());
  }
  switch (name) {
    case "float4":
    case "float8":
    case "real":
      if (typeof v === "number" && Number.isFinite(v)) return v;
      return fail(`is ${written}, a finite number; got ${describe(v)}.`);
    case "numeric":
      return decimal(written, v, fail);
    case "bool":
      if (typeof v === "boolean") return v;
      return fail(`is ${written}, true or false; got ${describe(v)}.`);
    case "text":
    case "varchar":
    case "bpchar":
    case "char":
    case "name":
    case "citext": {
      if (typeof v !== "string") return fail(`is ${written}, a string; got ${describe(v)}.`);
      const n = /\(\s*(\d+)\s*\)/.exec(written)?.[1];
      // A length counts characters (code points), as Postgres and MySQL count them.
      if (n !== undefined && [...v].length > Number(n)) fail(`is ${written}, at most ${n} characters; got ${[...v].length}, which the database would refuse or cut.`);
      return v;
    }
    case "vector":
    case "halfvec":
      // pgvector's vector, as `.run()` returns it, written as the text its input reads.
      if (Array.isArray(v) && v.length > 0 && v.every((e) => typeof e === "number" && Number.isFinite(e))) return JSON.stringify(v);
      if (typeof v === "string") return v;
      return fail(`is ${written}, an array of finite numbers with one at least; got ${describe(v)}.`);
    case "hstore":
      // An hstore, as `.run()` returns it, written as the text its input reads.
      if (v !== null && typeof v === "object") return hstoreText(v, (message) => fail(message.replace(/^is an hstore/, `is ${written}`)));
      if (typeof v === "string") return v;
      return fail(`is ${written}, an object of its keys whose values are strings or null; got ${describe(v)}.`);
    case "uuid":
      if (typeof v === "string" && /^\{?[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}\}?$/i.test(v)) return v;
      return fail(`is ${written}, a UUID in hexadecimal; got ${describe(v)}.`);
    case "timestamptz":
      // A string that names its zone, as `.run()` returns one; no Date.
      if (v instanceof Date) return fail(`is ${written}, which takes the instant as a string that names its zone, not a Date: pass date.toISOString() ("2026-10-12T19:06:48.282Z").`);
      if (typeof v === "string" && zoned.test(v.trim())) return dialect === "postgres" ? postgresYear(v) : v;
      return fail(`is ${written}, an instant: a string that names its zone ("2026-10-12T19:06:48.282Z", "2026-10-12 19:06:48+09"); got ${describe(v)}, which the server would read in the session's time zone.`);
    case "timestamp":
    case "date":
      if (typeof v === "string") return dialect === "postgres" ? postgresYear(v) : v;
      return fail(`is ${written}, a wall-clock reading with no zone, which takes its text ("2026-10-12 19:06:48.282", "2026-10-12"), not a Date: drivers write a Date's time in different zones.${v instanceof Date ? "" : ` Got ${describe(v)}.`}`);
    case "bytea":
    case "blob": {
      // Binary data travels as hexadecimal text: Postgres's own `\x` form, and bare digits that
      // MySQL's and SQLite's readers decode with unhex().
      if (!(v instanceof Uint8Array)) return fail(`is ${written}, binary data, a Uint8Array; got ${describe(v)}.`);
      const n = /\(\s*(\d+)\s*\)/.exec(written)?.[1];
      if (n !== undefined && v.length > Number(n)) fail(`is ${written}, at most ${n} bytes; got ${v.length}, which the database would refuse or cut.`);
      let hex = dialect === "postgres" ? "\\x" : "";
      for (const byte of v) hex += (byte < 16 ? "0" : "") + byte.toString(16);
      return hex;
    }
    case "interval":
      // `-P1D`, as `.run()` returns a negative interval, in the form Postgres reads.
      if (typeof v === "string") return postgresInterval(v);
      if (typeof v === "number") return v;
      return fail(`is ${written}; got ${describe(v)}.`);
    default:
      // A type the run time has no rule for (time, inet, …) takes the text the database reads, and
      // a number or a truth value as JSON writes them.
      if (typeof v === "string" || typeof v === "number" || typeof v === "boolean") return v;
      return fail(`is ${written}; got ${describe(v)}.`);
  }
}

/** An integer in `range`, as a number, a BigInt or, where `digits`, a string of its digits. */
/**
 * `truth`: MySQL and SQLite, where a truth value is an integer, and a TINYINT(1), BIT(1) or BOOLEAN
 * column returns a boolean, which goes back in as 1 or 0 (the round-trip rule, src/render.ts).
 */
function integer(range: readonly [bigint, bigint], digits: boolean, written: string, v: unknown, fail: (message: string) => never, truth = false): bigint {
  let n: bigint;
  if (typeof v === "bigint") n = v;
  else if (typeof v === "boolean" && truth) n = BigInt(v ? 1 : 0);
  else if (typeof v === "number" && Number.isInteger(v)) {
    if (!Number.isSafeInteger(v)) fail(`is ${written}; ${v} is past 2^53 - 1, where a number stops being exact, so pass it as a BigInt${digits ? " or a string of digits" : ""}.`);
    n = BigInt(v);
  } else if (typeof v === "string" && /^[+-]?\d+$/.test(v.trim()) && digits) n = BigInt(v.trim());
  else return fail(`is ${written}, an integer; got ${describe(v)}.`);
  if (n < range[0] || n > range[1]) fail(`is ${written}, from ${range[0]} to ${range[1]}; got ${n}.`);
  return n;
}

/** MySQL's integer types as a `::` cast names them, each with its range; `unsigned` doubles it from 0. */
const MYSQL_INTEGERS: Record<string, readonly [bigint, bigint]> = {
  tinyint: [BigInt(-128), BigInt(127)],
  smallint: INTEGERS.int2!,
  int2: INTEGERS.int2!,
  mediumint: [BigInt(-8388608), BigInt(8388607)],
  int: INTEGERS.int4!,
  integer: INTEGERS.int4!,
  int4: INTEGERS.int4!,
  bigint: INTEGERS.int8!,
  int8: INTEGERS.int8!,
  signed: INTEGERS.int8!,
};

/**
 * Checks the value of a parameter written `:p::type` on MySQL or SQLite, whose cast turns a value
 * it cannot read into 0 or NULL instead of failing (`CAST('abc' AS SIGNED)` is 0, and SQLite's
 * `CAST('1.9' AS INTEGER)` is 1), against the type as written, before the statement runs; the
 * value is bound as it is. `type` is the engine's type for the parameter. On SQLite every integer
 * is 64 bits, whatever the cast calls it.
 */
export function checkCast(name: string, written: string, type: EngineType, v: unknown, dialect: Dialect): void {
  if (v === null || v === undefined) return;
  const fail = (message: string): never => {
    throw new ParameterError(`Parameter "${name}" ${message}`);
  };
  const base = written.replace(/\s*\(.*$/, "");
  const signed = dialect === "sqlite" ? (/int/i.test(base) ? INTEGERS.integer : undefined) : MYSQL_INTEGERS[base.replace(/\s*unsigned$/, "") || "bigint"];
  if (signed) {
    const range: readonly [bigint, bigint] = dialect === "mysql" && base.endsWith("unsigned") ? [BigInt(0), signed[1] * BigInt(2) + BigInt(1)] : signed;
    integer(range, dialect === "mysql" && range[1] >= INTEGERS.int8![1], written, v, fail, true);
    return;
  }
  if (base === "uuid") {
    scalar("uuid", written, v, dialect, fail);
    return;
  }
  if (dialect === "mysql" && (base === "date" || base === "time" || base === "datetime" || base === "timestamp")) {
    wallClock(base, written, v, fail);
    return;
  }
  check(type, written, v, dialect, fail);
}

/**
 * A date, a time of day or both, as text MySQL's CAST reads without changing it; it turns other
 * text into NULL, an impossible date included, and rounds a fraction finer than the type keeps
 * (`::time` keeps none, and `::datetime` is written as DATETIME(6)).
 */
function wallClock(base: string, written: string, v: unknown, fail: (message: string) => never): void {
  const example = base === "date" ? `"2026-10-12"` : base === "time" ? `"19:06:48"` : `"2026-10-12 19:06:48.282"`;
  if (typeof v !== "string") return void fail(`is ${written}, which takes its text (${example}), not ${describe(v)}${v instanceof Date ? ": drivers write a Date's time in different zones" : ""}.`);
  const s = v.trim();
  const keeps = Number(/\(\s*(\d+)\s*\)/.exec(written)?.[1] ?? (base === "time" ? 0 : 6));
  const m =
    base === "time"
      ? /^(-?)(\d{1,3}):(\d{2})(?::(\d{2})(?:\.(\d+))?)?$/.exec(s)
      : /^(\d{4})-(\d{2})-(\d{2})(?:(?:[T ])(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d+))?)?)?$/.exec(s);
  const ok = (() => {
    if (!m) return false;
    if (base === "time") return Number(m[2]) <= 838 && Number(m[3]) < 60 && Number(m[4] ?? 0) < 60 && (m[5] ?? "").length <= keeps;
    const [year, month, day] = [Number(m[1]), Number(m[2]), Number(m[3])];
    const days = new Date(Date.UTC(year, month, 0)).getUTCDate();
    if (month < 1 || month > 12 || day < 1 || day > days) return false;
    if (base === "date") return m[4] === undefined;
    return Number(m[4] ?? 0) < 24 && Number(m[5] ?? 0) < 60 && Number(m[6] ?? 0) < 60 && (m[7] ?? "").length <= keeps;
  })();
  if (!ok) fail(`is ${written}, as text like ${example}${keeps > 0 && base !== "date" ? ` with at most ${keeps} digits after the second's point` : base === "time" ? " with no fraction of a second" : ""}; got ${describe(v)}, which MySQL's CAST would make NULL or change.`);
}

/**
 * A decimal, as a number or as its digits. `numeric(p,s)` keeps `s` digits after the point and
 * `p` in all: a value with more decimals would be rounded, so it is refused, and so is one with
 * more digits before the point than the column holds.
 */
function decimal(written: string, v: unknown, fail: (message: string) => never): unknown {
  let text: string;
  if (typeof v === "number") {
    if (!Number.isFinite(v)) return fail(`is ${written}, a finite decimal; got ${v}.`);
    text = String(v);
  } else if (typeof v === "string") text = v.trim();
  else if (typeof v === "bigint") text = v.toString();
  else return fail(`is ${written}, a decimal: a number or a string of its digits; got ${describe(v)}.`);
  const m = /^([+-]?)(\d*)(?:\.(\d*))?(?:e([+-]?\d+))?$/i.exec(text);
  if (!m || (m[2] === "" && (m[3] ?? "") === "")) return fail(`is ${written}, a decimal: a number or a string of its digits; got ${describe(v)}.`);
  const mod = /\(\s*(\d+)\s*(?:,\s*(-?\d+)\s*)?\)/.exec(written);
  if (mod) {
    const precision = Number(mod[1]);
    const scale = Number(mod[2] ?? 0);
    const exponent = Number(m[4] ?? 0);
    let whole = (m[2] ?? "").replace(/^0+/, "");
    let fraction = (m[3] ?? "").replace(/0+$/, "");
    // Shift the point by the exponent.
    if (exponent > 0) {
      const moved = fraction.slice(0, exponent).padEnd(exponent, "0");
      whole = (whole + moved).replace(/^0+/, "");
      fraction = fraction.slice(exponent);
    } else if (exponent < 0) {
      const digits = whole.padStart(-exponent, "0");
      fraction = (digits.slice(digits.length + exponent) + fraction).replace(/0+$/, "");
      whole = digits.slice(0, digits.length + exponent).replace(/^0+/, "");
    }
    if (fraction.length > scale) fail(`is ${written}, which keeps ${scale} digit${scale === 1 ? "" : "s"} after the point, so ${text} would be rounded.`);
    if (whole.length > precision - scale) fail(`is ${written}, which holds at most ${precision - scale} digit${precision - scale === 1 ? "" : "s"} before the point; got ${text}.`);
  }
  return typeof v === "bigint" ? v.toString() : v;
}

/** The checked rows as JSON text, a 64-bit integer as its digits. */
function write(v: unknown): string {
  if (v instanceof Raw) return v.text;
  if (Array.isArray(v)) return `[${v.map(write).join(",")}]`;
  if (v !== null && typeof v === "object") {
    return `{${Object.entries(v as Record<string, unknown>)
      .map(([k, x]) => `${JSON.stringify(k)}:${write(x)}`)
      .join(",")}}`;
  }
  return JSON.stringify(v);
}

/** The statement with each list of a structured parameter written again for the keys present (MySQL): an entry stays when its key is required or present. */
export function variant(text: string, structured: Structured, fields: readonly RowField[], present: ReadonlySet<string>): string {
  const lists = [...structured.lists].sort((a, b) => b.start - a.start);
  let out = text;
  for (const list of lists) {
    const kept = list.entries.filter((_, k) => {
      const f = fields[k];
      return f === undefined || !f.optional || present.has(f.name);
    });
    out = out.slice(0, list.start) + kept.join(", ") + out.slice(list.end);
  }
  return out;
}
