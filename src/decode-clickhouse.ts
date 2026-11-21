// Reylo runs its own ClickHouse statements on the app's `@clickhouse/client` (or
// `@clickhouse/client-web`), and decodes every value itself (the representation is src/values.ts).
//
// A statement runs through the client's `exec()`, which hands back the response's bytes, and asks
// for the result as `RowBinaryWithNamesAndTypes`: each column's name and type, then each row's
// values in their own binary form. Every value is exact there, where the JSON a client parses by
// default is not (a `UInt64` past 2^53, a `Decimal`, NaN and the infinities), and the format is
// named with `default_format`, which a user whose profile is read-only may still set.
//
// - an integer of up to 32 bits is a number, and a wider one, to `UInt256`, a BigInt;
// - a `Decimal` is its exact text as ClickHouse writes it, without the zeros its scale pads with;
// - a `Float32` is the number it holds, exactly, as a `Float32Array` gives one: 0.1 stored in one
//   reads 0.10000000149011612, where ClickHouse prints the shortest text that reads back, `0.1`;
// - a `Date` or `Date32` is `YYYY-MM-DD`, and a `DateTime` or `DateTime64` the instant in UTC with
//   `Z`, to its type's digits (`2026-01-02T03:04:05.123Z`): the zone in a type's name is how the
//   column prints, and no part of a value. A year outside 0000 to 9999 is written with its sign and
//   six digits, as ISO 8601 writes one;
// - a `String` or `FixedString` is its bytes read as UTF-8, a `UUID` its text, an `IPv4` or `IPv6`
//   its address as ClickHouse writes it, and an `Enum8` or `Enum16` its label;
// - an `Array` is an array, a `Tuple` with names an object and one without an array, a `Map` its
//   pairs in order, each `{ key, value }`, since a ClickHouse map may hold a key twice, and a
//   `Variant` the value of whichever type it holds, or null.
//
// The result's types come with its rows, so each result is held against the types the type check
// read: a setting of the session that runs the statement (`join_use_nulls`) or a changed table
// gives a column another type, which is refused before a value of it is returned as the old one.
//
// A parameter is written `{name:Type}`, and its value goes as the text that type reads, which
// Reylo writes itself from the type in the placeholder, as a `param_` field of the request: a
// tuple in parentheses, a map in braces, an instant as its seconds since 1970, which no zone
// changes, and NULL as `\N`. ClickHouse wraps an integer outside its type's range without an
// error (300 as a `UInt8` is 44), so each is held to its range first, and so is every other value
// the server would change and say nothing: a decimal's digits, a float past a `Float32`, a date
// that is none of the calendar's, a string that is no text.
import { DecodeError, describe, ParameterError, type Columns, type Kind } from "./values.ts";
import { canonical, decimalOf, integerOf, parseType, plain, typeName, type Named } from "./clickhouse-types.ts";
import { rowOf } from "./row.ts";
import { readClickhouseError } from "./query-error.ts";
import type { AnyClient as Client, Bound, Marks, QueryModule, Runtime } from "./execute.ts";

type AnyClient = Record<string, any>;

/** The format a statement's result is asked for in. */
const FORMAT = "RowBinaryWithNamesAndTypes";

// ---------------------------------------------------------------------------------------------
// Reading a result
// ---------------------------------------------------------------------------------------------

/** The result ended inside a value: a statement that failed after its first rows ends so. */
class Truncated extends Error {}

// TextDecoder and TextEncoder are globals in every runtime Reylo supports; they are typed here rather
// than through the DOM lib or Node's, which a project that type-checks Reylo's sources need not have.
// Nothing is made as the module loads, so a bundle of another dialect's queries drops it whole.
type Coders = { TextDecoder: new (label?: string, options?: { ignoreBOM?: boolean }) => { decode(bytes: Uint8Array): string }; TextEncoder: new () => { encode(text: string): Uint8Array } };
const coders = (): Coders => globalThis as unknown as Coders;
let decoder: { decode(bytes: Uint8Array): string } | undefined;
/** Text as its UTF-8 bytes say, a byte that is none of it read as U+FFFD, and a byte order mark kept. */
const utf8 = (bytes: Uint8Array): string => (decoder ??= new (coders().TextDecoder)("utf-8", { ignoreBOM: true })).decode(bytes);
/** A text's UTF-8 bytes. */
const utf8Bytes = (text: string): Uint8Array => new (coders().TextEncoder)().encode(text);

class Reader {
  at = 0;
  readonly bytes: Uint8Array;
  readonly view: DataView;
  constructor(bytes: Uint8Array) {
    this.bytes = bytes;
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  /** Where `n` bytes start, once they are known to be there. */
  take(n: number): number {
    const start = this.at;
    if (start + n > this.bytes.length) throw new Truncated();
    this.at = start + n;
    return start;
  }

  u8(): number {
    return this.bytes[this.take(1)]!;
  }

  /** A length: seven bits a byte, the low ones first. */
  varint(): number {
    let value = 0;
    let scale = 1;
    for (;;) {
      const byte = this.u8();
      value += (byte & 0x7f) * scale;
      if (byte < 0x80) return value;
      scale *= 128;
    }
  }

  text(length: number): string {
    const start = this.take(length);
    return utf8(this.bytes.subarray(start, start + length));
  }
}

type Read = (r: Reader) => unknown;

/** An integer of `bytes` bytes, the low ones first, as a BigInt. */
function wide(r: Reader, bytes: number, unsigned: boolean): bigint {
  const start = r.take(bytes);
  let value = unsigned ? r.view.getBigUint64(start + bytes - 8, true) : r.view.getBigInt64(start + bytes - 8, true);
  for (let at = start + bytes - 16; at >= start; at -= 8) value = (value << 64n) | r.view.getBigUint64(at, true);
  return value;
}

function integerReader(bits: number, unsigned: boolean): Read {
  switch (bits) {
    case 8:
      return unsigned ? (r) => r.u8() : (r) => r.view.getInt8(r.take(1));
    case 16:
      return unsigned ? (r) => r.view.getUint16(r.take(2), true) : (r) => r.view.getInt16(r.take(2), true);
    case 32:
      return unsigned ? (r) => r.view.getUint32(r.take(4), true) : (r) => r.view.getInt32(r.take(4), true);
    default:
      return (r) => wide(r, bits / 8, unsigned);
  }
}

/** A decimal's text from its digits and its scale, as ClickHouse writes one: no zeros after its last digit. */
function decimalText(units: bigint, scale: number): string {
  const negative = units < 0n;
  let digits = (negative ? -units : units).toString();
  if (scale > 0) {
    digits = digits.padStart(scale + 1, "0");
    const fraction = digits.slice(-scale).replace(/0+$/, "");
    digits = fraction === "" ? digits.slice(0, -scale) : `${digits.slice(0, -scale)}.${fraction}`;
  }
  return negative && digits !== "0" ? `-${digits}` : digits;
}

const hex = (bytes: Uint8Array, from: number, to: number, step: 1 | -1): string => {
  let out = "";
  for (let i = from; i !== to; i += step) out += bytes[i]!.toString(16).padStart(2, "0");
  return out;
};

/**
 * An IPv6 address as ClickHouse writes it: its first longest run of zero groups as `::`, and its
 * last four bytes in dots where it holds an IPv4 address, mapped (`::ffff:1.2.3.4`) or with
 * nothing before them but a non-zero seventh group (`::1.2.3.4`, and `::2` as it is).
 */
function ipv6Text(bytes: Uint8Array, start: number): string {
  const groups: number[] = [];
  for (let i = 0; i < 16; i += 2) groups.push((bytes[start + i]! << 8) | bytes[start + i + 1]!);
  const dotted = groups.slice(0, 5).every((g) => g === 0) && (groups[5] === 0xffff || (groups[5] === 0 && groups[6] !== 0));
  const shown = dotted ? 6 : 8;
  let best = { at: -1, length: 0 };
  for (let i = 0; i < shown; ) {
    if (groups[i] !== 0) {
      i += 1;
      continue;
    }
    let end = i;
    while (end < shown && groups[end] === 0) end += 1;
    if (end - i > best.length) best = { at: i, length: end - i };
    i = end;
  }
  if (best.length < 2) best = { at: -1, length: 0 };
  let out = "";
  for (let i = 0; i < shown; i++) {
    if (i === best.at) {
      out += "::";
      i += best.length - 1;
      continue;
    }
    if (out !== "" && !out.endsWith(":")) out += ":";
    out += groups[i]!.toString(16);
  }
  if (!dotted) return out;
  return `${out}${out.endsWith(":") ? "" : ":"}${bytes[start + 12]}.${bytes[start + 13]}.${bytes[start + 14]}.${bytes[start + 15]}`;
}

const two = (n: number): string => (n < 10 ? `0${n}` : String(n));

/**
 * The date `days` after 1970-01-01, by the calendar's own arithmetic where JavaScript's Date holds
 * no such day or writes its year otherwise: a year outside 0000 to 9999 takes its sign and six
 * digits or more, as ISO 8601 extends a year.
 */
function dateText(days: number): string {
  // 0000-01-01 to 9999-12-31, which a Date writes as four digits.
  if (days >= -719528 && days <= 2932896) return new Date(days * 86_400_000).toISOString().slice(0, 10);
  // The day's place in its 400 years, which begin on a 1 March.
  const z = days + 719468;
  const era = Math.floor(z / 146097);
  const dayOfEra = z - era * 146097;
  const yearOfEra = Math.floor((dayOfEra - Math.floor(dayOfEra / 1460) + Math.floor(dayOfEra / 36524) - Math.floor(dayOfEra / 146096)) / 365);
  const dayOfYear = dayOfEra - (365 * yearOfEra + Math.floor(yearOfEra / 4) - Math.floor(yearOfEra / 100));
  const fromMarch = Math.floor((5 * dayOfYear + 2) / 153);
  const day = dayOfYear - Math.floor((153 * fromMarch + 2) / 5) + 1;
  const month = fromMarch < 10 ? fromMarch + 3 : fromMarch - 9;
  const year = yearOfEra + era * 400 + (month <= 2 ? 1 : 0);
  return `${year < 0 ? "-" : "+"}${String(Math.abs(year)).padStart(6, "0")}-${two(month)}-${two(day)}`;
}

/** The instant `seconds` after 1970-01-01T00:00:00Z, to the second: any a 64-bit count of seconds holds. */
function secondText(seconds: number | bigint): string {
  const s = BigInt(seconds);
  let days = s / 86400n;
  let rest = Number(s % 86400n);
  if (rest < 0) {
    days -= 1n;
    rest += 86400;
  }
  return `${dateText(Number(days))}T${two(Math.floor(rest / 3600))}:${two(Math.floor(rest / 60) % 60)}:${two(rest % 60)}`;
}

/** What reads a value of `type` from a result's bytes, for the column `column`. */
function readerOf(type: Named, column: string): Read {
  const t = plain(type);
  const inner = (at: number): Named => {
    const a = t.args[at];
    if (a === undefined || !("type" in a)) throw unread(type, column);
    return a.type;
  };
  const integer = integerOf(t.name);
  if (integer) return integerReader(integer.bits, integer.unsigned);
  const decimal = decimalOf(t);
  if (decimal) {
    const { precision, scale } = decimal;
    if (precision <= 9) return (r) => decimalText(BigInt(r.view.getInt32(r.take(4), true)), scale);
    const bytes = precision <= 18 ? 8 : precision <= 38 ? 16 : 32;
    return (r) => decimalText(wide(r, bytes, false), scale);
  }
  switch (t.name) {
    case "Nullable": {
      const read = readerOf(inner(0), column);
      return (r) => (r.u8() === 1 ? null : read(r));
    }
    // The type of a NULL alone, which a Nullable around it reads, and of an empty array's elements.
    case "Nothing":
      return () => null;
    case "Bool":
      return (r) => r.u8() !== 0;
    case "Float32":
      return (r) => r.view.getFloat32(r.take(4), true);
    case "Float64":
      return (r) => r.view.getFloat64(r.take(8), true);
    // The upper half of a 32-bit float.
    case "BFloat16":
      return (r) => {
        const scratch = new DataView(new ArrayBuffer(4));
        scratch.setUint32(0, r.view.getUint16(r.take(2), true) * 65536);
        return scratch.getFloat32(0);
      };
    case "String":
      return (r) => r.text(r.varint());
    case "FixedString": {
      const a = t.args[0];
      const length = a !== undefined && "number" in a ? Number(a.number) : NaN;
      if (!Number.isInteger(length)) throw unread(type, column);
      return (r) => r.text(length);
    }
    // Two 64-bit halves, each with its low byte first.
    case "UUID":
      return (r) => {
        const start = r.take(16);
        const h = hex(r.bytes, start + 7, start - 1, -1) + hex(r.bytes, start + 15, start + 7, -1);
        return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
      };
    case "IPv4":
      return (r) => {
        const start = r.take(4);
        return `${r.bytes[start + 3]}.${r.bytes[start + 2]}.${r.bytes[start + 1]}.${r.bytes[start]}`;
      };
    case "IPv6":
      return (r) => ipv6Text(r.bytes, r.take(16));
    case "Date":
      return (r) => dateText(r.view.getUint16(r.take(2), true));
    case "Date32":
      return (r) => dateText(r.view.getInt32(r.take(4), true));
    case "DateTime":
      return (r) => `${secondText(r.view.getUint32(r.take(4), true))}Z`;
    case "DateTime64": {
      const a = t.args[0];
      const digits = a !== undefined && "number" in a ? Number(a.number) : NaN;
      if (!Number.isInteger(digits) || digits < 0 || digits > 9) throw unread(type, column);
      const second = 10n ** BigInt(digits);
      return (r) => {
        const ticks = r.view.getBigInt64(r.take(8), true);
        // The second the instant falls in, toward the past, and the ticks past it.
        let seconds = ticks / second;
        let past = ticks % second;
        if (past < 0n) {
          seconds -= 1n;
          past += second;
        }
        return `${secondText(seconds)}${digits === 0 ? "" : `.${past.toString().padStart(digits, "0")}`}Z`;
      };
    }
    case "Enum8":
    case "Enum16": {
      const labels = new Map<number, string>();
      for (const a of t.args) if ("label" in a) labels.set(a.value, a.label);
      const wide16 = t.name === "Enum16";
      return (r) => {
        const value = wide16 ? r.view.getInt16(r.take(2), true) : r.view.getInt8(r.take(1));
        const label = labels.get(value);
        if (label === undefined) throw new DecodeError(`Column "${column}" holds the number ${value}, which its type ${typeName(type)} names no label for.`);
        return label;
      };
    }
    case "Array": {
      const read = readerOf(inner(0), column);
      return (r) => {
        const length = r.varint();
        const out = new Array<unknown>(length);
        for (let i = 0; i < length; i++) out[i] = read(r);
        return out;
      };
    }
    case "Tuple": {
      if (t.args.length > 0 && t.args.every((a) => "member" in a)) {
        const names = t.args.map((a) => ("member" in a ? a.member : ""));
        const reads = t.args.map((a) => readerOf("member" in a ? a.of : type, column));
        const build = rowOf(names);
        return (r) => build(reads.map((read) => read(r)));
      }
      const reads = t.args.map((_, i) => readerOf(inner(i), column));
      return (r) => reads.map((read) => read(r));
    }
    // A map's keys may repeat, so it is its pairs in order.
    case "Map": {
      const key = readerOf(inner(0), column);
      const value = readerOf(inner(1), column);
      return (r) => {
        const length = r.varint();
        const out = new Array<unknown>(length);
        for (let i = 0; i < length; i++) out[i] = { key: key(r), value: value(r) };
        return out;
      };
    }
    // A byte says which of the types the value is, in the order the type's name lists them; 255 is NULL.
    case "Variant": {
      const reads = t.args.map((_, i) => readerOf(inner(i), column));
      return (r) => {
        const which = r.u8();
        if (which === 255) return null;
        const read = reads[which];
        if (read === undefined) throw new DecodeError(`Column "${column}" holds a value of the type numbered ${which}, and its type ${typeName(type)} has ${reads.length}.`);
        return read(r);
      };
    }
    default:
      throw unread(type, column);
  }
}

function unread(type: Named, column: string): DecodeError {
  return new DecodeError(`Column "${column}" has the ClickHouse type ${typeName(type)}, which Reylo does not read: select it as a type it reads, as toString(${column}) does.`);
}

/** The error a statement raised after its result began, where the result's end carries one: `<message>\n<length> <tag>\r\n__exception__\r\n`, the tag the response's own (`X-ClickHouse-Exception-Tag`). */
function midStreamError(bytes: Uint8Array, tag: string | undefined): Error | undefined {
  // Each byte as the character of its number, so that a place in the text is a place in the bytes.
  let tail = "";
  for (let i = Math.max(0, bytes.length - 65536); i < bytes.length; i += 8192) tail += String.fromCharCode(...bytes.subarray(i, Math.min(bytes.length, i + 8192)));
  const close = tail.lastIndexOf("\r\n__exception__\r\n");
  if (close === -1) return undefined;
  const open = tag === undefined || tag === "" ? tail.indexOf("__exception__\r\n") : tail.indexOf(`__exception__\r\n${tag}\r\n`);
  if (open === -1 || open >= close) return undefined;
  const start = tail.indexOf("\r\n", open + "__exception__\r\n".length) + 2;
  const body = tail.slice(start, close).replace(/\n\d+ \S+$/, "");
  const raw = new Uint8Array(body.length);
  for (let i = 0; i < body.length; i++) raw[i] = body.charCodeAt(i);
  const text = utf8(raw).trim();
  const code = /^Code: (\d+)\./.exec(text)?.[1];
  const type = /\(([A-Z][A-Z0-9_]+)\)(?: \(version .*\))?\s*$/.exec(text)?.[1];
  // The message as the client's own error has it for a statement that fails at once: without the
  // code it opens with, and the name and version it ends with, which the error carries apart.
  const message = text.replace(/^Code: \d+\.\s*(?:DB::Exception:\s*)?/, "").replace(/\s*\(version .*\)\s*$/, "").replace(/\s*\([A-Z][A-Z0-9_]+\)\s*$/, "").trim();
  return Object.assign(new Error(message), { code, type });
}

/**
 * Every byte of a response's body, from Node's stream of the client or the web client's, and the
 * error the stream ended with, if it did: a statement that fails after its first rows closes the
 * connection early, with its error at the end of what was sent.
 */
async function bytesOf(stream: unknown): Promise<{ bytes: Uint8Array; error?: unknown }> {
  const chunks: Uint8Array[] = [];
  const push = (chunk: unknown) => chunks.push(typeof chunk === "string" ? utf8Bytes(chunk) : (chunk as Uint8Array));
  const web = stream as { getReader?: () => { read(): Promise<{ done: boolean; value?: Uint8Array }> } };
  let failure: { error: unknown } | undefined;
  try {
    if (typeof web?.getReader === "function") {
      const reader = web.getReader();
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        if (value !== undefined) push(value);
      }
    } else if (stream != null) {
      for await (const chunk of stream as AsyncIterable<unknown>) push(chunk);
    }
  } catch (error) {
    failure = { error };
  }
  let bytes = chunks[0] ?? new Uint8Array(0);
  if (chunks.length > 1) {
    bytes = new Uint8Array(chunks.reduce((n, c) => n + c.byteLength, 0));
    let at = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, at);
      at += chunk.byteLength;
    }
  }
  return failure ? { bytes, error: failure.error } : { bytes };
}

/** Why a result's columns are not the ones the type check read, as an error that says what to do. */
function changed(query: QueryModule<"clickhouse">, what: string): Error {
  return new Error(
    `${query.source ?? "The statement"} returned ${what}. The table changed after the type check, or the session that runs the statement has a setting the type check's session lacks, as join_use_nulls: build it again against this database, and put such a setting in the statement's SETTINGS clause.`,
  );
}

/** The rows of a result in `RowBinaryWithNamesAndTypes`, each an object of its columns. */
function rowsOf(bytes: Uint8Array, query: QueryModule<"clickhouse">): unknown[] {
  if (bytes.length === 0) return [];
  const r = new Reader(bytes);
  const width = r.varint();
  const names: string[] = [];
  for (let i = 0; i < width; i++) names.push(r.text(r.varint()));
  const types: string[] = [];
  for (let i = 0; i < width; i++) types.push(r.text(r.varint()));
  const parsed = types.map((type, i) => {
    const read = parseType(type);
    // A session with output_format_binary_encode_types_in_binary_format writes each type as bytes, and no name.
    if (read === undefined) throw new DecodeError(`Column "${names[i]}" has the type ${JSON.stringify(type)}, whose name Reylo cannot read. Where the session's setting output_format_binary_encode_types_in_binary_format is 1, a type comes as bytes and no name: leave it at 0.`);
    return read;
  });
  const columns: Columns | undefined = query.columns;
  if (columns !== undefined) {
    if (columns.length !== width) throw changed(query, `${width} column${width === 1 ? "" : "s"} where the type check read ${columns.length}`);
    columns.forEach(([name, , described], i) => {
      // The one column of a statement that returns a value has no name the value goes by, and
      // ClickHouse's name for an expression holds each constant in it, a parameter's value too.
      if (name !== names[i] && !query.value) throw changed(query, `the column "${names[i]}" where the type check read "${name}"`);
      if (described !== undefined && canonical(described) !== canonical(types[i]!)) throw changed(query, `the column "${name}" as ${types[i]} where the type check read ${described}`);
    });
  }
  const reads = parsed.map((type, i) => readerOf(type, names[i]!));
  const build = query.row ?? rowOf(names);
  const rows: unknown[] = [];
  while (r.at < bytes.length) {
    const values = new Array<unknown>(width);
    for (let i = 0; i < width; i++) values[i] = reads[i]!(r);
    rows.push(build(values));
  }
  return rows;
}

// ---------------------------------------------------------------------------------------------
// Writing a parameter
// ---------------------------------------------------------------------------------------------

/** The characters a backslash writes in ClickHouse's text, in a parameter's value and in a string inside one. */
const ESCAPED: Readonly<Record<string, string>> = { "\\": "\\\\", "\t": "\\t", "\n": "\\n", "\r": "\\r", "\0": "\\0", "\b": "\\b", "\f": "\\f" };

/** A string as a parameter's whole value, or in quotes inside an array, a tuple or a map. */
function stringText(s: string, top: boolean): string {
  const escaped = s.replace(/[\\\t\n\r\0\b\f]/g, (c) => ESCAPED[c]!);
  return top ? escaped : `'${escaped.replaceAll("'", "\\'")}'`;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
const INSTANT = /^([+-]?\d{4,6})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d+))?)?\s*(?:(Z)|([+-])(\d{2})(?::?(\d{2}))?(?::?(\d{2}))?)$/i;
const DECIMAL = /^[+-]?(?:\d+\.?\d*|\.\d+)$/;
/** Half of a surrogate pair with no other half, which no encoding of text holds. */
const LONE_SURROGATE = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?:[^\uD800-\uDBFF]|^)[\uDC00-\uDFFF]/;
/** A zone's name that is UTC's own, or one of its other names. */
const UTC = /^(?:Etc\/)?(?:UTC|UCT|GMT|Zulu|Universal|Greenwich)$/;

/**
 * The 32-bit float nearest a number, as the shortest text that reads back as it, which is how
 * ClickHouse writes one: a server reads the seventeen digits of the double beside it as another
 * float on some releases.
 */
function shortest32(v: number): string {
  const float = Math.fround(v);
  for (let digits = 1; digits < 9; digits++) {
    const text = float.toPrecision(digits);
    if (Math.fround(Number(text)) === float) return String(Number(text));
  }
  return String(Number(float.toPrecision(9)));
}

/** The days from 1970-01-01 to a date, or undefined where its text is no date of the calendar. */
function daysOf(text: string): number | undefined {
  const m = DATE.exec(text);
  if (!m) return undefined;
  const ms = Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return dateText(ms / 86_400_000) === text ? ms / 86_400_000 : undefined;
}

/**
 * A parameter's value as the text its type reads. `path` says where in the parameter the value
 * stands (`[1]`, `.name`), and is empty for the parameter's whole value, which is written without
 * the quotes a string inside an array, a tuple or a map takes.
 */
function valueText(type: Named, v: unknown, name: string, path = ""): string {
  const t = plain(type);
  const top = path === "";
  const fail = (message: string): never => {
    throw new ParameterError(`Parameter "${name}"${top ? "" : ` at ${path}`} has the ClickHouse type ${typeName(type)}, ${message}; got ${describe(v)}.`);
  };
  const inner = (at: number): Named => {
    const a = t.args[at];
    return a !== undefined && "type" in a ? a.type : fail("which Reylo does not write");
  };
  if (t.name === "Nullable") return v == null ? (top ? "\\N" : "NULL") : valueText(inner(0), v, name, path);
  if (v == null) fail("which takes no null");
  const quote = (text: string) => (top ? text : `'${text}'`);
  const integer = integerOf(t.name);
  if (integer) {
    if (typeof v !== "bigint" && !(typeof v === "number" && Number.isInteger(v))) fail("which takes a whole number or a BigInt");
    const n = BigInt(v as number | bigint);
    const min = integer.unsigned ? 0n : -(2n ** BigInt(integer.bits - 1));
    const max = integer.unsigned ? 2n ** BigInt(integer.bits) - 1n : 2n ** BigInt(integer.bits - 1) - 1n;
    // ClickHouse wraps a number outside the range into it, without an error.
    if (n < min || n > max) fail(`which holds ${min} to ${max}`);
    // A number past 2^53 - 1 is the nearest one a double holds, and not the digits written.
    if (typeof v === "number" && !Number.isSafeInteger(v)) fail("which takes a number to 2^53 - 1, where one stops being exact, and a BigInt past it");
    return n.toString();
  }
  const decimal = decimalOf(t);
  if (decimal) {
    const text = typeof v === "bigint" ? v.toString() : typeof v === "number" && Number.isFinite(v) ? String(v) : typeof v === "string" ? v.trim() : "";
    if (!DECIMAL.test(text)) fail("which takes a decimal's text (1234.56)");
    const [whole = "", fraction = ""] = text.replace(/^[+-]/, "").split(".");
    // ClickHouse drops the digits past the scale, without an error.
    if (fraction.replace(/0+$/, "").length > decimal.scale) fail(`which keeps ${decimal.scale} digit${decimal.scale === 1 ? "" : "s"} after the point`);
    if (whole.replace(/^0+/, "").length > decimal.precision - decimal.scale) fail(`which keeps ${decimal.precision - decimal.scale} digit${decimal.precision - decimal.scale === 1 ? "" : "s"} before the point`);
    return text;
  }
  switch (t.name) {
    case "Bool":
      if (typeof v !== "boolean") fail("which takes true or false");
      return v ? "true" : "false";
    case "Float32":
    case "Float64":
    case "BFloat16":
      if (typeof v !== "number") fail("which takes a number");
      // ClickHouse reads a number past a 32-bit float as an infinity, without an error.
      if (t.name !== "Float64" && Number.isFinite(v) && !Number.isFinite(Math.fround(v as number))) fail("which holds -3.4028235e38 to 3.4028235e38");
      if (Number.isNaN(v)) return "nan";
      if (!Number.isFinite(v)) return v === Infinity ? "inf" : "-inf";
      if (Object.is(v, -0)) return "-0";
      return t.name === "Float64" ? String(v) : shortest32(v as number);
    case "String":
      if (typeof v !== "string") fail("which takes a string");
      if (LONE_SURROGATE.test(v as string)) fail("which takes text, and half of a surrogate pair is none: it would go as U+FFFD");
      return stringText(v as string, top);
    case "FixedString": {
      if (typeof v !== "string") fail("which takes a string");
      if (LONE_SURROGATE.test(v as string)) fail("which takes text, and half of a surrogate pair is none: it would go as U+FFFD");
      const a = t.args[0];
      const length = a !== undefined && "number" in a ? Number(a.number) : 0;
      if (utf8Bytes(v as string).length > length) fail(`which holds ${length} byte${length === 1 ? "" : "s"}`);
      return stringText(v as string, top);
    }
    case "UUID":
      if (typeof v !== "string" || !UUID.test(v)) fail("which takes a UUID's text");
      return quote(v as string);
    case "IPv4":
    case "IPv6":
      if (typeof v !== "string" || !/^[0-9a-f.:]+$/i.test(v)) fail("which takes an address's text");
      return quote(v as string);
    case "Enum8":
    case "Enum16":
      if (typeof v !== "string" || !t.args.some((a) => "label" in a && a.label === v)) fail(`which is one of ${t.args.map((a) => ("label" in a ? JSON.stringify(a.label) : "")).join(", ")}`);
      return stringText(v as string, top);
    case "Date":
    case "Date32": {
      const days = typeof v === "string" ? daysOf(v) : undefined;
      if (days === undefined) fail("which takes a date's text (2026-01-02)");
      const [min, max] = t.name === "Date" ? [0, 65535] : [-25567, 120529];
      // ClickHouse reads a date outside the range as another date, without an error.
      if (days! < min || days! > max) fail(`which holds ${dateText(min)} to ${dateText(max)}`);
      return quote(v as string);
    }
    case "DateTime":
    case "DateTime64": {
      const a = t.args[0];
      const digits = t.name === "DateTime64" && a !== undefined && "number" in a ? Number(a.number) : 0;
      const m = typeof v === "string" ? INSTANT.exec(v.trim()) : null;
      if (!m) fail('which takes the instant as a string with its offset or Z ("2026-01-02T03:04:05.123Z")');
      const [, year, month, day, hour, minute, second = "0", fraction = "", , sign, offsetHours = "0", offsetMinutes = "0", offsetSeconds = "0"] = m!;
      const local = new Date(0);
      local.setUTCFullYear(Number(year), Number(month) - 1, Number(day));
      local.setUTCHours(Number(hour), Number(minute), Number(second), 0);
      // A date that is none of the calendar's (30 February, hour 24, second 60) rolls over to another, without an error.
      const calendar = local.getUTCFullYear() === Number(year) && local.getUTCMonth() === Number(month) - 1 && local.getUTCDate() === Number(day) && local.getUTCHours() === Number(hour) && local.getUTCMinutes() === Number(minute) && local.getUTCSeconds() === Number(second);
      if (!calendar || Number(offsetHours) > 23 || Number(offsetMinutes) > 59 || Number(offsetSeconds) > 59) fail('which takes an instant of the calendar ("2026-01-02T03:04:05.123Z")');
      const offset = (sign === "-" ? -1 : 1) * (Number(offsetHours) * 3600 + Number(offsetMinutes) * 60 + Number(offsetSeconds));
      // The digits past the type's own would be dropped, without an error.
      if (fraction.slice(digits).replace(/0+$/, "") !== "") fail(`which keeps ${digits} digit${digits === 1 ? "" : "s"} after the second`);
      const second64 = 10n ** BigInt(digits);
      const ticks = (BigInt(local.getTime() / 1000) - BigInt(offset)) * second64 + BigInt(fraction.slice(0, digits).padEnd(digits, "0") || "0");
      // Its seconds since 1970, which no zone changes, in five digits or more: ClickHouse reads a
      // shorter number as a year, and refuses it.
      if (t.name === "DateTime") {
        if (ticks < 0n || ticks > 4294967295n) fail("which holds 1970-01-01T00:00:00Z to 2106-02-07T06:28:15Z");
        return ticks.toString().padStart(5, "0");
      }
      // The years ClickHouse says a DateTime64 holds: some releases read one past them as the
      // last instant of 9999, without an error.
      if (ticks < -2208988800n * second64 || ticks >= 10413792000n * second64) fail("which holds 1900-01-01T00:00:00Z to 2299-12-31T23:59:59Z");
      if (ticks < -(2n ** 63n) || ticks >= 2n ** 63n) fail("whose 64 bits hold no instant so far from 1970");
      const negative = ticks < 0n;
      const magnitude = negative ? -ticks : ticks;
      const past = (of: bigint) => (digits === 0 ? "" : `.${of.toString().padStart(digits, "0")}`);
      if (negative && magnitude < second64) {
        // ClickHouse reads `-0.5` as half a second after 1970: the sign of a number whose whole
        // part is zero is lost. The second before 1970 is written as its date and time, which a
        // type reads in its own zone, so only a type that names UTC reads the instant meant.
        const zone = t.args[1];
        if (!(zone !== undefined && "text" in zone && UTC.test(zone.text))) fail(`which ClickHouse reads an instant within the second before 1970 for only where the type names UTC, as DateTime64(${digits}, 'UTC') does`);
        return quote(`1969-12-31 23:59:59${past(second64 - magnitude)}`);
      }
      return quote(`${negative ? "-" : ""}${(magnitude / second64).toString().padStart(5, "0")}${past(magnitude % second64)}`);
    }
    case "Array":
      if (!Array.isArray(v)) fail("which takes an array");
      return `[${(v as unknown[]).map((e, i) => valueText(inner(0), e, name, `${path}[${i}]`)).join(",")}]`;
    case "Tuple": {
      const named = t.args.length > 0 && t.args.every((a) => "member" in a);
      if (named && !Array.isArray(v) && typeof v === "object") {
        // A member under another name would go as NULL where the member takes one, and one too many as nothing.
        const members = t.args.map((a) => ("member" in a ? a.member : ""));
        const missing = members.find((member) => !Object.hasOwn(v as object, member) || (v as Record<string, unknown>)[member] === undefined);
        if (missing !== undefined) fail(`which has the member ${JSON.stringify(missing)}, and the value has none`);
        const unknown = Object.keys(v as object).find((key) => !members.includes(key));
        if (unknown !== undefined) fail(`which has no member ${JSON.stringify(unknown)}`);
      }
      const values = Array.isArray(v) ? v : named && typeof v === "object" ? t.args.map((a) => (v as Record<string, unknown>)["member" in a ? a.member : ""]) : fail(named ? "which takes an object of its members" : "which takes an array of its elements");
      if (values.length !== t.args.length) fail(`which has ${t.args.length} element${t.args.length === 1 ? "" : "s"}`);
      return `(${t.args.map((a, i) => valueText("member" in a ? a.of : inner(i), values[i], name, "member" in a ? `${path}.${a.member}` : `${path}[${i}]`)).join(",")})`;
    }
    case "Map": {
      if (!Array.isArray(v) || !v.every((e) => e !== null && typeof e === "object" && "key" in e && "value" in e && Object.keys(e).length === 2)) fail("which takes an array of its pairs, each { key, value } and no more");
      return `{${(v as { key: unknown; value: unknown }[]).map((e, i) => `${valueText(inner(0), e.key, name, `${path}[${i}].key`)}:${valueText(inner(1), e.value, name, `${path}[${i}].value`)}`).join(",")}}`;
    }
    default:
      return fail("which Reylo does not write");
  }
}

/**
 * The type each parameter of a statement declares, by name, read at its placeholders' places in the
 * text Reylo sends: the marks a build step writes, each the placeholder whole, `{name:Type}`
 * (src/loader.ts, `marksOf`). The run time reads no SQL of its own, and a query's types are read once.
 */
const declared = new WeakMap<QueryModule, ReadonlyMap<string, Named>>();

function typesOf(query: QueryModule, text: string, marks: Marks): ReadonlyMap<string, Named> {
  let types = declared.get(query);
  if (types === undefined) {
    const found = new Map<string, Named>();
    for (const [start, end] of marks) {
      const written = text.slice(start, end);
      const colon = written.indexOf(":");
      const type = written.startsWith("{") && written.endsWith("}") && colon !== -1 ? parseType(written.slice(colon + 1, -1)) : undefined;
      if (type !== undefined) found.set(written.slice(1, colon).trim(), type);
    }
    declared.set(query, (types = found));
  }
  return types;
}

// ---------------------------------------------------------------------------------------------
// Running a statement
// ---------------------------------------------------------------------------------------------

/** `@clickhouse/client`'s and `@clickhouse/client-web`'s client: exec() hands back the response's bytes. */
function isClient(c: AnyClient): boolean {
  return typeof c.exec === "function" && typeof c.query === "function" && typeof c.insert === "function" && typeof c.command === "function";
}

/**
 * The error of a request the server refused for its size, with the parameter that made it so: a
 * parameter goes as a field of the request, and the server takes 128 KiB of one unless its
 * `http_max_field_value_size` says otherwise. The server's own text names neither.
 */
function tooLong(error: unknown, names: readonly string[], settings: Readonly<Record<string, string>>): Error | undefined {
  const message = error instanceof Error ? error.message : "";
  if (!/Field value too long|URI (?:invalid or )?too long/i.test(message)) return undefined;
  let longest: { name: string; bytes: number } | undefined;
  for (const name of names) {
    const bytes = utf8Bytes(settings[`param_${name}`] ?? "").length;
    if (longest === undefined || bytes > longest.bytes) longest = { name, bytes };
  }
  if (longest === undefined) return undefined;
  const code = /Code: (\d+)/.exec(message)?.[1];
  const own = message.replace(/^Poco::Exception\.\s*Code: \d+,\s*(?:e\.code\(\) = \d+,\s*)?/, "").replace(/\s*\(version [^()]*(?:\([^()]*\))?\)\s*$/, "").trim();
  return Object.assign(new Error(`${own}: the parameter "${longest.name}" is ${longest.bytes} bytes as ClickHouse's text, and the server takes 131072 of one unless its http_max_field_value_size is raised. Send fewer values at a time.`, { cause: error }), code === undefined ? {} : { code });
}

/** A ClickHouse statement's rows on an `@clickhouse/client` client, or undefined on any other client. */
async function runClickhouse(c: Client, query: QueryModule<"clickhouse">, { text, values, names, marks }: Bound): Promise<unknown[] | undefined> {
  if (!isClient(c)) return undefined;
  if (marks === undefined && names.length > 0) throw new TypeError("Reylo cannot bind this ClickHouse statement's parameters: each goes as its declared type's text, which Reylo reads at its placeholder, and a query defined by hand needs `marks` there, the place of each placeholder in its text, which a build step writes.");
  const types = typesOf(query, text, marks ?? []);
  const settings: Record<string, string> = { default_format: FORMAT };
  names.forEach((name, i) => {
    const type = types.get(name);
    if (type === undefined) throw new ParameterError(`Parameter "${name}" has no type in the statement: a ClickHouse parameter is written with one, as {${name}:UInt64}.`);
    settings[`param_${name}`] = valueText(type, values[i], name);
  });
  let response: { stream?: unknown; response_headers?: unknown } | undefined;
  try {
    response = await c.exec({ query: text, clickhouse_settings: settings });
  } catch (error) {
    throw tooLong(error, names, settings) ?? error;
  }
  const headers = response?.response_headers as Record<string, unknown> | undefined;
  const tag = headers?.["x-clickhouse-exception-tag"];
  // A statement's own SETTINGS clause outranks the request's, and a format it names is the result's.
  const format = headers?.["x-clickhouse-format"];
  if (typeof format === "string" && format !== "" && format !== FORMAT) {
    await bytesOf(response?.stream);
    throw new DecodeError(`${query.source ?? "The statement"} returned its result in the format ${format}, and Reylo reads ${FORMAT}: take the format out of the statement's SETTINGS (default_format).`);
  }
  const { bytes, error } = await bytesOf(response?.stream);
  const failed = midStreamError(bytes, typeof tag === "string" ? tag : undefined);
  if (failed) throw failed;
  if (error !== undefined) throw error;
  try {
    return rowsOf(bytes, query);
  } catch (error) {
    if (error instanceof Truncated) throw new DecodeError(`${query.source ?? "The statement"}'s result ended inside a row: the connection closed, or the statement failed, after its first rows.`);
    throw error;
  }
}

/** A column's raw value as `.run()` gives it, as a Standard Query's `decode` takes it (src/standard-query.ts): `.run()`'s own, since the raw value of a ClickHouse result is the one Reylo reads from its bytes. */
function clickhouseDecoder(_name: string, _kind: Kind): (v: unknown) => unknown {
  return (v) => (v === undefined ? null : v);
}

/** ClickHouse's half of the run time (src/execute.ts, `Runtime`), which `defineClickhouseQuery` passes. */
export const clickhouse: Runtime<"clickhouse"> = { dialect: "clickhouse", run: runClickhouse, readError: readClickhouseError, decoder: clickhouseDecoder };

/** The parts of this module its tests hold to the server's own text and bytes. */
export const internals = { rowsOf, valueText, typesOf, midStreamError, ipv6Text, decimalText };
