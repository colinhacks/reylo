// The project file's `js.codecs`: for a family of values, the TypeScript type `.run()` returns
// them as and a parameter of that family takes, in place of the one representation of
// src/values.ts. A family is a kind of the type descriptor format (spec/index.ts). A
// codec reaches every position Reylo decodes itself: a row's columns, an array's elements, a
// DuckDB struct's fields, a map's keys and values, a tagged union's value, a structured
// parameter's members. A value inside a JSON document the statement builds is the document's, and
// keeps the type JSON gives it. Every codec is exact or refuses a value it cannot hold: a number
// refuses a decimal a double does not hold, and a Temporal class
// refuses what no Temporal value is (`infinity`, MySQL's zero date, Postgres's `24:00:00`, an
// interval whose parts differ in sign). A 64-bit integer has no codec: a bigint is its one exact
// type, and a statement that wants a number or text casts it.
//
// The build decides each column's codecs from its type descriptor, so that the module's declared
// type and the value `run` returns agree. The renderer (src/render.ts) and the JSON Schema writer
// (src/schema.ts) apply a codec at each position they reach outside JSON, and the module carries,
// for each column and parameter that holds one, a plan: where in its value each codec stands
// (`codecs`, by column name; a parameter's in its `inputs`). The run time decodes every value as
// src/values.ts does, then follows each plan, in one pass over the rows.
//
// This module runs in every generated module's process, so it names no Node type, and it reaches
// Temporal through `globalThis`, where a runtime that has none (Cloudflare Workers) is told so.
import { bareName, type EngineType } from "./engine-type.ts";
import { columnKind, DecodeError, describe, isoYear, ParameterError, type Dialect } from "./values.ts";

/** The `js.codecs` section as the engine parses it (engine/core/src/config.rs). */
export type Codecs = {
  readonly decimal?: "string" | "number";
  readonly bytes?: "Uint8Array" | "string";
  readonly date?: "string" | "Temporal.PlainDate";
  readonly time?: "string" | "Temporal.PlainTime";
  readonly datetime?: "string" | "Temporal.PlainDateTime";
  readonly instant?: "string" | "Temporal.Instant";
  readonly duration?: "string" | "Temporal.Duration";
  readonly set?: "Set" | "Array";
};

type Family = keyof Codecs;

/** A codec at one position: its family, and the type it gives where the family has more than one besides its default. */
export type Codec = "decimal:number" | "bytes:string" | "date" | "time" | "datetime" | "instant" | "duration" | "set:array";

/**
 * Where a value's codecs stand: a codec for the value itself, or the parts that hold one, an
 * array's elements, a struct's or a structured parameter's fields by name, a map's keys and
 * values, a tagged union's value by its tag.
 */
export type Plan =
  | Codec
  | { readonly array: Plan }
  | { readonly fields: Readonly<Record<string, Plan>> }
  | { readonly entries: { readonly key?: Plan; readonly value?: Plan } }
  | { readonly tags: Readonly<Record<string, Plan>> };

/** The TypeScript type of a value under each codec, which a parameter of it takes too; a SET's array is of its members' type (src/render.ts). */
export const CODEC_TYPE: Readonly<Record<Codec, string>> = {
  "decimal:number": "number",
  "bytes:string": "string",
  date: "Temporal.PlainDate",
  time: "Temporal.PlainTime",
  datetime: "Temporal.PlainDateTime",
  instant: "Temporal.Instant",
  duration: "Temporal.Duration",
  "set:array": "string[]",
};

type TemporalName = "PlainDate" | "PlainTime" | "PlainDateTime" | "Instant" | "Duration";

/** The Temporal class of each codec that gives one, as `Object.prototype.toString` names its values. */
const TEMPORAL: Partial<Readonly<Record<Codec, TemporalName>>> = { date: "PlainDate", time: "PlainTime", datetime: "PlainDateTime", instant: "Instant", duration: "Duration" };

/** The family each codec belongs to, which its setting is named by. */
const FAMILY: Readonly<Record<Codec, Family>> = {
  "decimal:number": "decimal",
  "bytes:string": "bytes",
  date: "date",
  time: "time",
  datetime: "datetime",
  instant: "instant",
  duration: "duration",
  "set:array": "set",
};

/**
 * A SQLite column's declared type that names a date or a time (`DATETIME`, `DATE(10)`): SQLite has
 * no such type, so such a column holds whatever was written, and only its declaration, the
 * descriptor's name, says that the text it holds is a date.
 */
const SQLITE_DECLARED: Readonly<Record<string, Family>> = { DATE: "date", DATETIME: "datetime", TIMESTAMP: "datetime", TIME: "time" };

/**
 * The family of a value of type `t` that is no array, struct, map or tagged union, from the value
 * it is decoded to (src/values.ts, `columnKind`) and, where that does not say, its descriptor: a
 * duration (MySQL's `TIME` among them), a time of day with no zone (a `timetz` is none), and a
 * SQLite column whose declaration names a date. An instant that keeps an offset or a region per
 * value is none either, since a `Temporal.Instant` keeps the moment alone.
 */
function family(t: EngineType, dialect: Dialect): Family | undefined {
  if (t.kind === "duration") return "duration";
  const kind = columnKind(t, dialect);
  switch (kind) {
    case "decimal":
      return "decimal";
    case "bytes":
      return "bytes";
    case "date":
      return "date";
    case "timestamp":
      return "datetime";
    case "timestamptz":
      return t.kind === "instant" && t.zone !== undefined ? undefined : "instant";
    case "text":
      return t.kind === "time" && t.zone === undefined ? "time" : undefined;
    case "set":
      return "set";
    default:
      return dialect === "sqlite" && t.kind === "union" ? SQLITE_DECLARED[bareName(t).trim().replace(/\s*\(.*$/, "").toUpperCase()] : undefined;
  }
}

/** The codec a value of type `t` takes itself, where the project's codecs give its family one other than the default. */
export function leafCodec(t: EngineType, dialect: Dialect, codecs: Codecs): Codec | undefined {
  const f = family(t, dialect);
  const v = f === undefined ? undefined : codecs[f];
  switch (f) {
    case "set":
      return v === "Array" ? "set:array" : undefined;
    case "decimal":
      return v === "number" ? "decimal:number" : undefined;
    case "bytes":
      return v === "string" ? "bytes:string" : undefined;
    case undefined:
      return undefined;
    default:
      return v !== undefined && v !== "string" ? f : undefined;
  }
}

/**
 * Where the codecs of a value of type `t` stand, following the parts of it Reylo decodes itself
 * (`param`: a parameter's, whose structured members are fields too); undefined where none does.
 */
export function planOf(t: EngineType, dialect: Dialect, codecs: Codecs, param = false): Plan | undefined {
  if (!Object.keys(codecs).length) return undefined;
  const leaf = leafCodec(t, dialect, codecs);
  if (leaf !== undefined) return leaf;
  switch (t.kind) {
    case "array": {
      if (!columnKind(t, dialect).endsWith("[]") && !(param && t.element.type.kind === "record")) return undefined;
      const element = planOf(t.element.type, dialect, codecs, param);
      return element === undefined ? undefined : { array: element };
    }
    case "record": {
      // A DuckDB struct, a Postgres row value and a structured parameter are objects of their fields.
      if (dialect !== "duckdb" && dialect !== "postgres" && !param) return undefined;
      const fields = Object.fromEntries(t.fields.flatMap((f) => {
        const plan = planOf(f.type, dialect, codecs, param);
        return plan === undefined ? [] : [[f.name, plan]];
      }));
      return Object.keys(fields).length ? { fields } : undefined;
    }
    case "range":
    case "multirange": {
      // A Postgres range's two bounds.
      if (dialect !== "postgres") return undefined;
      const bound = planOf(t.subtype, dialect, codecs, param);
      if (bound === undefined) return undefined;
      const fields: Plan = { fields: { lower: bound, upper: bound } };
      return t.kind === "range" ? fields : { array: fields };
    }
    case "map": {
      if (dialect !== "duckdb") return undefined;
      const key = planOf(t.key, dialect, codecs, param);
      const value = planOf(t.value.type, dialect, codecs, param);
      return key === undefined && value === undefined ? undefined : { entries: { ...(key === undefined ? {} : { key }), ...(value === undefined ? {} : { value }) } };
    }
    case "tagged_union": {
      // A tagged union's value, `{ tag, value }`, by the alternative its tag names.
      const plans = Object.fromEntries(t.alternatives.flatMap((a) => {
        const plan = planOf(a.type, dialect, codecs, param);
        return plan === undefined ? [] : [[a.tag, plan]];
      }));
      return Object.keys(plans).length ? { tags: plans } : undefined;
    }
    default:
      return undefined;
  }
}

// ---------------------------------------------------------------------------------------------
// The run time.

type TemporalClass = { from(text: string): unknown };

/** The global Temporal's class for a codec: a runtime without one is told which setting needs it. */
function temporalClass(codec: Codec, name: string, what: "Column" | "Parameter"): TemporalClass {
  const cls = (globalThis as unknown as { Temporal?: Record<string, TemporalClass | undefined> }).Temporal?.[TEMPORAL[codec]!];
  if (cls === undefined) throw new (what === "Column" ? DecodeError : ParameterError)(`${what} "${name}" is a Temporal.${TEMPORAL[codec]} by js.codecs.${FAMILY[codec]} in the project file, and this JavaScript runtime has no global Temporal. Run on one that has it, or set js.codecs.${FAMILY[codec]} to string.`);
  return cls;
}

/**
 * The ISO 8601 text of a date, or a date and time, as src/values.ts writes Postgres's: a year past
 * 9999 in ISO's extended form, and one before 1 (` BC`, year 1 − n) as ISO's year, which
 * Temporal reads; MySQL's and SQLite's space before the time as it is.
 */
function isoDate(text: string): string {
  const m = /^(\d{4,})(-.*?)( BC)?$/.exec(text);
  if (m === null) return text;
  const year = m[3] === undefined ? +m[1]! : 1 - +m[1]!;
  return `${isoYear(year)}${m[2]}`;
}

/** MySQL's TIME, `-838:59:59.000000` to `838:59:59.000000`, as an ISO 8601 duration; any other text as it is. */
function isoDuration(text: string): string {
  const m = /^(-)?(\d+):(\d\d):(\d\d)(\.\d+)?$/.exec(text);
  if (m === null) return text;
  const fraction = (m[5] ?? "").replace(/\.?0+$/, "");
  return `${m[1] ?? ""}PT${+m[2]!}H${+m[3]!}M${+m[4]!}${fraction}S`;
}

/** Whether every digit of a decimal's text survives as a double: the number's own shortest text is the same decimal. */
function sameDecimal(text: string, n: number): boolean {
  const canonical = (s: string): string => {
    let sign = "";
    if (s[0] === "-" || s[0] === "+") (sign = s[0] === "-" ? "-" : ""), (s = s.slice(1));
    let [whole = "", frac = ""] = s.split(".");
    let exp = 0;
    const e = /[eE]([+-]?\d+)$/.exec(frac || whole);
    if (e) {
      exp = +e[1]!;
      if (frac) frac = frac.slice(0, e.index);
      else whole = whole.slice(0, e.index);
    }
    // The digits and where the point falls in them, then neither zero at either end.
    let digits = whole + frac;
    let point = whole.length + exp;
    const lead = digits.length - digits.replace(/^0+/, "").length;
    digits = digits.slice(lead).replace(/0+$/, "");
    point -= lead;
    if (digits === "") return "0";
    return `${sign}${digits}@${point}`;
  };
  return canonical(text) === canonical(String(n));
}

// The whole text each Temporal codec reads. Temporal's own parsers take more, and change it
// without an error (measured on Node 26.11: `23:59:60` is read as `23:59:59`, `08:00:00+02:00` as
// `08:00:00`, `2016-12-31T10:00` as the date `2016-12-31`, `20161231` as a date), which a SQLite
// column, holding whatever text was written, may hold; the text src/values.ts writes always fits.
const DAY = String.raw`(?:\d{4}|[+-]\d{6})-\d\d-\d\d`;
const CLOCK = String.raw`\d\d:\d\d(?::[0-5]\d(?:\.\d{1,9})?)?`;
const SHAPE: Partial<Record<Codec, RegExp>> = {
  date: new RegExp(`^${DAY}$`),
  time: new RegExp(`^${CLOCK}$`),
  datetime: new RegExp(`^${DAY}[T ]${CLOCK}$`),
  instant: new RegExp(`^${DAY}[T ]${CLOCK}(?:Z|[+-]\\d\\d(?::?\\d\\d)?)$`, "i"),
};

/** The value a position of `codec` holds, from the one src/values.ts decoded; null stays null, and an array is decoded element by element. */
export function decodeCodec(codec: Codec, v: unknown, column: string): unknown {
  if (v === null || v === undefined) return v;
  if (Array.isArray(v)) return v.map((e) => decodeCodec(codec, e, column));
  switch (codec) {
    case "set:array":
      // A SET's members, in the order the column declares them.
      return v instanceof Set ? [...v] : v;
    case "decimal:number": {
      if (typeof v !== "string") return v;
      const n = Number(v);
      // NaN and the infinities a `numeric` holds are numbers too; a finite value past a
      // double's range is not one of them.
      if (Number.isFinite(n) ? sameDecimal(v, n) : /^[+-]?(?:infinity|nan)$/i.test(v.trim())) return n;
      throw new DecodeError(`Column "${column}" holds ${v}, which a number cannot hold exactly (js.codecs.decimal is number in the project file), so Reylo refuses it rather than return a rounded value. Cast it to text in the statement (${column}::text) to read it exactly.`);
    }
    case "bytes:string":
      return v instanceof Uint8Array ? base64(v) : v;
    default: {
      const cls = TEMPORAL[codec]!;
      const refuse = (shown: string) =>
        new DecodeError(`Column "${column}" holds ${shown}, which no Temporal.${cls} holds (js.codecs.${FAMILY[codec]} is Temporal.${cls} in the project file). Map it in the statement (nullif(${column}, 'infinity')), or cast it to text (${column}::text) to read it as it is.`);
      // A SQLite column declared as a date holds whatever was written.
      if (typeof v !== "string") throw refuse(describe(v));
      const temporal = temporalClass(codec, column, "Column");
      const text = codec === "duration" ? isoDuration(v) : codec === "time" ? v : isoDate(v);
      if (SHAPE[codec]?.test(text) === false) throw refuse(JSON.stringify(v));
      try {
        return temporal.from(text);
      } catch (e) {
        if (e instanceof RangeError) throw refuse(JSON.stringify(v));
        throw e;
      }
    }
  }
}

/** A value with its plan's codecs applied, wherever they stand in it. */
export function decodePlan(plan: Plan, v: unknown, column: string): unknown {
  if (v === null || v === undefined) return v;
  if (typeof plan === "string") return decodeCodec(plan, v, column);
  if ("array" in plan) {
    // An element may be an array itself, in a Postgres array of any depth.
    const element = (e: unknown): unknown => (Array.isArray(e) && !(typeof plan.array === "object" && "array" in plan.array) ? e.map(element) : decodePlan(plan.array, e, column));
    return Array.isArray(v) ? v.map(element) : v;
  }
  if (typeof v !== "object") return v;
  if ("fields" in plan) {
    const o = v as Record<string, unknown>;
    for (const [name, p] of Object.entries(plan.fields)) if (name in o) o[name] = decodePlan(p, o[name], column);
    return o;
  }
  if ("entries" in plan) {
    if (!Array.isArray(v)) return v;
    for (const e of v as { key: unknown; value: unknown }[]) {
      if (plan.entries.key !== undefined) e.key = decodePlan(plan.entries.key, e.key, column);
      if (plan.entries.value !== undefined) e.value = decodePlan(plan.entries.value, e.value, column);
    }
    return v;
  }
  const u = v as { tag?: unknown; value: unknown };
  const p = typeof u.tag === "string" ? plan.tags[u.tag] : undefined;
  if (p !== undefined) u.value = decodePlan(p, u.value, column);
  return u;
}

/** Each column of every row a plan names given its codecs, in place. */
export function decodeCodecs(rows: unknown[], plans: Readonly<Record<string, Plan>>): unknown[] {
  const entries = Object.entries(plans);
  for (const row of rows) {
    if (row === null || typeof row !== "object") continue;
    const r = row as Record<string, unknown>;
    for (const [column, plan] of entries) if (column in r) r[column] = decodePlan(plan, r[column], column);
  }
  return rows;
}

const tag = (v: unknown) => Object.prototype.toString.call(v);

/** A Temporal.Duration as MySQL's TIME reads it, `[-]H:MM:SS[.fffffffff]`, which holds hours and below. */
function mysqlTime(v: { sign: number; years: number; months: number; weeks: number; days: number; hours: number; minutes: number; seconds: number; milliseconds: number; microseconds: number; nanoseconds: number }, refuse: () => ParameterError): string {
  if (v.years || v.months || v.weeks) throw refuse();
  // Temporal keeps each field as it was given (`{ minutes: 90 }`, `{ milliseconds: 1500 }`), so the
  // text is written from the total, in BigInts, since a field may be past 2^53 in nanoseconds.
  const part = (n: number, unit: number) => BigInt(Math.abs(n)) * BigInt(unit);
  const micros = part(v.days, 86400000000) + part(v.hours, 3600000000) + part(v.minutes, 60000000) + part(v.seconds, 1000000) + part(v.milliseconds, 1000) + part(v.microseconds, 1) + BigInt(Math.abs(v.nanoseconds)) / BigInt(1000);
  // MySQL clips a TIME past 838:59:59 without an error.
  if (micros > BigInt(838 * 3600 + 59 * 60 + 59) * BigInt(1000000)) throw refuse();
  const seconds = Number(micros / BigInt(1000000));
  const fraction = String(Number(micros % BigInt(1000000))).padStart(6, "0").replace(/0+$/, "");
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${v.sign < 0 ? "-" : ""}${Math.floor(seconds / 3600)}:${pad(Math.floor(seconds / 60) % 60)}:${pad(seconds % 60)}${fraction && `.${fraction}`}`;
}

/**
 * A value of `codec` as the driver binds it, the text or bytes its type reads: a Temporal value's
 * ISO 8601 text (a SQLite date and time with a space, as SQLite's own functions write it, and a
 * duration for MySQL's TIME as its hours), base64's bytes, a number's digits. Anything but the
 * codec's type is refused, since TypeScript holds the parameter to it.
 */
export function encodeCodec(codec: Codec, v: unknown, name: string, dialect: Dialect): unknown {
  if (v === null || v === undefined) return v;
  const refuse = (expected: string) => new ParameterError(`Parameter "${name}" takes ${expected}, as js.codecs.${FAMILY[codec]} in the project file says; got ${describe(v)}.`);
  // A SET's members as an array, which binds as the Set the run time writes as MySQL's text.
  if (codec === "set:array") {
    if (!Array.isArray(v) || !v.every((m) => typeof m === "string")) throw refuse("an array of the SET's members");
    return new Set(v);
  }
  if (Array.isArray(v)) return v.map((e) => encodeCodec(codec, e, name, dialect));
  switch (codec) {
    case "decimal:number":
      if (typeof v !== "number") throw refuse("a number");
      return String(v);
    case "bytes:string": {
      if (typeof v !== "string" || !/^[A-Za-z0-9+/]*={0,2}$/.test(v) || v.length % 4 === 1) throw refuse("binary data as its base64");
      return fromBase64(v);
    }
    default: {
      const cls = TEMPORAL[codec]!;
      if (tag(v) !== `[object Temporal.${cls}]`) throw refuse(`a Temporal.${cls}`);
      // Postgres and MySQL keep microseconds at most, and would round what is past them.
      if ((dialect === "postgres" || dialect === "mysql") && pastMicroseconds(codec, v)) {
        throw new ParameterError(`Parameter "${name}" holds ${String(v)}, a Temporal.${cls} past the microsecond, which ${dialect === "mysql" ? "MySQL" : "Postgres"} would round. Round it first: .round({ smallestUnit: "microsecond" }).`);
      }
      if (codec === "duration" && dialect === "mysql") return mysqlTime(v as Parameters<typeof mysqlTime>[0], () => refuse("a Temporal.Duration a MySQL TIME holds, of days and less and within ±838:59:59"));
      const text = String(v);
      return dialect === "sqlite" && codec === "datetime" ? text.replace("T", " ") : text;
    }
  }
}

/** Whether a Temporal value of `codec` holds a part of a second finer than a microsecond. */
function pastMicroseconds(codec: Codec, v: unknown): boolean {
  const t = v as { epochNanoseconds?: bigint; nanosecond?: number; nanoseconds?: number };
  if (codec === "instant") return t.epochNanoseconds! % BigInt(1000) !== BigInt(0);
  if (codec === "datetime" || codec === "time") return t.nanosecond !== 0;
  return codec === "duration" && t.nanoseconds! % 1000 !== 0;
}

/** A parameter's value with its plan's codecs written as the driver binds them, wherever they stand in it. */
export function encodePlan(plan: Plan, v: unknown, name: string, dialect: Dialect): unknown {
  if (v === null || v === undefined) return v;
  if (typeof plan === "string") return encodeCodec(plan, v, name, dialect);
  if ("array" in plan) {
    const element = (e: unknown): unknown => (Array.isArray(e) && !(typeof plan.array === "object" && "array" in plan.array) ? e.map(element) : encodePlan(plan.array, e, name, dialect));
    return Array.isArray(v) ? v.map(element) : v;
  }
  if (typeof v !== "object") return v;
  if ("fields" in plan) {
    const o = { ...(v as Record<string, unknown>) };
    for (const [field, p] of Object.entries(plan.fields)) if (field in o) o[field] = encodePlan(p, o[field], name, dialect);
    return o;
  }
  if ("entries" in plan) {
    if (!Array.isArray(v)) return v;
    return (v as { key: unknown; value: unknown }[]).map((e) => ({
      ...e,
      ...(plan.entries.key === undefined ? {} : { key: encodePlan(plan.entries.key, e.key, name, dialect) }),
      ...(plan.entries.value === undefined ? {} : { value: encodePlan(plan.entries.value, e.value, name, dialect) }),
    }));
  }
  const u = v as { tag?: unknown; value: unknown };
  const p = typeof u.tag === "string" ? plan.tags[u.tag] : undefined;
  return p === undefined ? u : { ...u, value: encodePlan(p, u.value, name, dialect) };
}

// Base64 through the runtime's own: Uint8Array's toBase64 and fromBase64 where it has them (Node
// 25 and later, Bun), Buffer where it is Node 24, and btoa and atob, which every runtime has.
type B64 = { prototype: { toBase64?: (this: Uint8Array) => string }; fromBase64?: (s: string) => Uint8Array };
type BufferClass = { from(data: Uint8Array | string, encoding?: string): Uint8Array & { toString(encoding: string): string } };
const U8 = Uint8Array as unknown as B64;
const NodeBuffer = (globalThis as { Buffer?: BufferClass }).Buffer;
const g = globalThis as unknown as { btoa(s: string): string; atob(s: string): string };

function base64(bytes: Uint8Array): string {
  if (U8.prototype.toBase64) return U8.prototype.toBase64.call(bytes);
  if (NodeBuffer) return NodeBuffer.from(bytes).toString("base64");
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return g.btoa(s);
}

function fromBase64(text: string): Uint8Array {
  if (U8.fromBase64) return U8.fromBase64(text);
  if (NodeBuffer) return new Uint8Array(NodeBuffer.from(text, "base64"));
  return Uint8Array.from(g.atob(text), (c) => c.charCodeAt(0));
}
