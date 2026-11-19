// A ClickHouse type's name, read into its parts and written back on one line: the run time's copy
// of the engine's reader (engine/clickhouse/src/types.rs), which the two change together. A
// result carries each column's type by name beside its rows, and a parameter's placeholder its
// own, `{name:Type}`, so the run time reads both: a row's values by the types the result names
// (src/decode-clickhouse.ts), and a parameter's value as the text its type takes.

/** A type as ClickHouse names it: its name, and the arguments in its parentheses. */
export type Named = { readonly name: string; readonly args: readonly Arg[] };

/** One argument of a type's name. */
export type Arg =
  /** A type: the `String` of `Array(String)`. */
  | { readonly type: Named }
  /** A member and its type: the `x Float64` of `Tuple(x Float64)`. */
  | { readonly member: string; readonly of: Named }
  /** A number: the `18` of `Decimal(18, 4)`. */
  | { readonly number: string }
  /** A string, unescaped: the `UTC` of `DateTime('UTC')`. */
  | { readonly text: string }
  /** An enum's label, unescaped, and its number: `'admin' = 1`. */
  | { readonly label: string; readonly value: number }
  /** Anything else, as written: a JSON type's `max_dynamic_paths = 8`. */
  | { readonly other: string };

const ESCAPES: Readonly<Record<string, string>> = { n: "\n", t: "\t", r: "\r", "0": "\0", b: "\b", f: "\f", a: "\x07", v: "\v", e: "\x1b" };

class Parser {
  i = 0;
  readonly text: string;
  constructor(text: string) {
    this.text = text;
  }

  space(): void {
    while (this.i < this.text.length && /\s/.test(this.text[this.i]!)) this.i += 1;
  }

  peek(): string | undefined {
    this.space();
    return this.text[this.i];
  }

  /** A name: letters, digits and `_`, or one quoted with backquotes or double quotes. */
  name(): string | undefined {
    const c = this.peek();
    if (c === "`" || c === '"') return this.quoted(c);
    const m = /^[A-Za-z_][A-Za-z0-9_]*/.exec(this.text.slice(this.i));
    if (!m) return undefined;
    this.i += m[0].length;
    return m[0];
  }

  /** A string or a quoted name from its opening quote, unescaped. */
  quoted(quote: string): string | undefined {
    let out = "";
    this.i += 1;
    for (;;) {
      const c = this.text[this.i];
      if (c === undefined) return undefined;
      if (c === "\\") {
        const e = this.text[this.i + 1];
        if (e === undefined) return undefined;
        if (e === "x") {
          const code = Number.parseInt(this.text.slice(this.i + 2, this.i + 4), 16);
          if (Number.isNaN(code)) return undefined;
          out += String.fromCharCode(code);
          this.i += 4;
        } else {
          out += ESCAPES[e] ?? e;
          this.i += 2;
        }
      } else if (c === quote && this.text[this.i + 1] === quote) {
        out += quote;
        this.i += 2;
      } else if (c === quote) {
        this.i += 1;
        return out;
      } else {
        out += c;
        this.i += 1;
      }
    }
  }

  number(): string | undefined {
    const m = /^-?[0-9.]+/.exec(this.text.slice(this.i));
    if (!m) return undefined;
    this.i += m[0].length;
    return m[0];
  }

  named(): Named | undefined {
    const name = this.name();
    return name === undefined ? undefined : this.arguments(name);
  }

  /** The type of `name`, with the arguments that follow it in parentheses. */
  arguments(name: string): Named | undefined {
    const args: Arg[] = [];
    if (this.peek() !== "(") return { name, args };
    this.i += 1;
    if (this.peek() === ")") {
      this.i += 1;
      return { name, args };
    }
    for (;;) {
      const arg = this.arg();
      if (arg === undefined) return undefined;
      args.push(arg);
      const next = this.peek();
      if (next === ",") this.i += 1;
      else if (next === ")") {
        this.i += 1;
        return { name, args };
      } else return undefined;
    }
  }

  /** One argument: read as what it is where it ends at a `,` or a `)`, else kept as written. */
  arg(): Arg | undefined {
    this.space();
    const start = this.i;
    const known = this.known();
    if (known !== undefined) {
      const next = this.peek();
      if (next === "," || next === ")") return known;
    }
    this.i = start;
    const raw = this.raw();
    return raw === undefined ? undefined : { other: raw };
  }

  known(): Arg | undefined {
    const c = this.peek();
    if (c === undefined) return undefined;
    if (c === "'") {
      const text = this.quoted("'");
      if (text === undefined) return undefined;
      if (this.peek() !== "=") return { text };
      this.i += 1;
      this.space();
      const number = this.number();
      return number === undefined || !/^-?\d+$/.test(number) ? undefined : { label: text, value: Number(number) };
    }
    if (/[0-9-]/.test(c)) {
      const number = this.number();
      return number === undefined ? undefined : { number };
    }
    let name = this.name();
    if (name === undefined) return undefined;
    // A JSON type's path, which only ever names a member.
    let path = false;
    while (this.text[this.i] === ".") {
      this.i += 1;
      const part = this.name();
      if (part === undefined) return undefined;
      name = `${name}.${part}`;
      path = true;
    }
    const next = this.peek();
    if (next !== undefined && /[A-Za-z_`"]/.test(next)) {
      const of = this.named();
      return of === undefined ? undefined : { member: name, of };
    }
    if (path) return undefined;
    const type = this.arguments(name);
    return type === undefined ? undefined : { type };
  }

  /** The text up to the `,` or `)` that ends an argument, past its own parentheses and strings. */
  raw(): string | undefined {
    const start = this.i;
    let depth = 0;
    for (;;) {
      const c = this.text[this.i];
      if (c === undefined) return undefined;
      if (c === "'" || c === "`" || c === '"') {
        if (this.quoted(c) === undefined) return undefined;
      } else if (c === "(") {
        depth += 1;
        this.i += 1;
      } else if (c === ")" && depth > 0) {
        depth -= 1;
        this.i += 1;
      } else if ((c === "," || c === ")") && depth === 0) break;
      else this.i += 1;
    }
    const text = this.text.slice(start, this.i).trim();
    return text === "" ? undefined : text;
  }
}

/** The type `text` names, or undefined where it is no type's name. */
export function parseType(text: string): Named | undefined {
  const p = new Parser(text);
  const named = p.named();
  p.space();
  return named !== undefined && p.i === text.length ? named : undefined;
}

/** A string as ClickHouse writes one in a type's name: in single quotes, a quote and a backslash escaped. */
function quotedText(text: string): string {
  return `'${text.replaceAll("\\", "\\\\").replaceAll("'", "\\'")}'`;
}

function memberName(name: string): string {
  return /^[A-Za-z_][A-Za-z0-9_.]*$/.test(name) ? name : `\`${name.replaceAll("\\", "\\\\").replaceAll("`", "\\`")}\``;
}

/** The type on one line, as ClickHouse names it beside a result's rows: `DateTime64(3, 'UTC')`. */
export function typeName(t: Named): string {
  if (t.args.length === 0) return t.name === "Tuple" ? "Tuple()" : t.name;
  const args = t.args.map((a) => {
    if ("type" in a) return typeName(a.type);
    if ("member" in a) return `${memberName(a.member)} ${typeName(a.of)}`;
    if ("number" in a) return a.number;
    if ("text" in a) return quotedText(a.text);
    if ("label" in a) return `${quotedText(a.label)} = ${a.value}`;
    return a.other;
  });
  return `${t.name}(${args.join(", ")})`;
}

/** A type's name on one line, however it is spaced, or the name as it is where it does not read as one. */
export function canonical(name: string): string {
  const parsed = parseType(name);
  return parsed === undefined ? name.trim() : typeName(parsed);
}

/** What the geometric types are sugar for, each by its name. */
const GEOMETRY: Readonly<Record<string, string>> = {
  Point: "Tuple(Float64, Float64)",
  Ring: "Array(Tuple(Float64, Float64))",
  LineString: "Array(Tuple(Float64, Float64))",
  Polygon: "Array(Array(Tuple(Float64, Float64)))",
  MultiLineString: "Array(Array(Tuple(Float64, Float64)))",
  MultiPolygon: "Array(Array(Array(Tuple(Float64, Float64))))",
};

/**
 * The type a value's bytes and text are read by: without the wrappers that change neither
 * (`LowCardinality`, and a `SimpleAggregateFunction`'s function), a geometric type as what it is
 * sugar for, and a `Nested` as the array of tuples it is.
 */
export function plain(t: Named): Named {
  if (t.name === "LowCardinality" && t.args.length === 1 && "type" in t.args[0]!) return plain(t.args[0].type);
  if (t.name === "SimpleAggregateFunction" && t.args.length === 2 && "type" in t.args[1]!) return plain(t.args[1].type);
  if (t.name === "Nested") return { name: "Array", args: [{ type: { name: "Tuple", args: t.args } }] };
  const sugar = GEOMETRY[t.name];
  return sugar === undefined ? t : parseType(sugar)!;
}

/** An integer type's width and whether it is unsigned, by its name. */
export function integerOf(name: string): { bits: number; unsigned: boolean } | undefined {
  const m = /^(U?)Int(8|16|32|64|128|256)$/.exec(name);
  return m ? { bits: Number(m[2]), unsigned: m[1] === "U" } : undefined;
}

/** A decimal type's precision and scale, by its name and arguments. */
export function decimalOf(t: Named): { precision: number; scale: number } | undefined {
  const number = (at: number) => {
    const a = t.args[at];
    return a !== undefined && "number" in a ? Number(a.number) : undefined;
  };
  const sized: Readonly<Record<string, number>> = { Decimal32: 9, Decimal64: 18, Decimal128: 38, Decimal256: 76 };
  if (t.name === "Decimal") {
    const precision = number(0);
    return precision === undefined ? undefined : { precision, scale: number(1) ?? 0 };
  }
  const precision = sized[t.name];
  const scale = number(0);
  return precision === undefined || scale === undefined ? undefined : { precision, scale };
}
