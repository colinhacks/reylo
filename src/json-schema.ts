// A JSON Schema 2020-12 validator for the run time, which has no dependencies: it checks a value
// against the schema a statement writes in a `json('…')` member of a column list or, where it is
// built, a JSON parameter's cast (wiki/typed-json-params.md: JSON
// Schema in its entirety; checks TypeScript cannot say are Reylo's to run). The engine refuses what
// the profile leaves out before a module is written (references, `unevaluatedItems`, a schema in
// `unevaluatedProperties`, the dynamic keywords), so none of those reach this file. A format is
// asserted, as the decision of 2026-10-19 says; a format this file has no rule for checks nothing.

type Schema = boolean | { readonly [key: string]: unknown };
type Object_ = { readonly [key: string]: unknown };

/** Why a value fails a schema: the place in the value, as a JSON Pointer, and what it breaks. */
export type Failure = { path: string; message: string };

/**
 * A fresh copy of a JSON value, such as a schema's `default`, so that filling one in never shares
 * it between calls. A JSON round trip rather than `structuredClone`, which the run time cannot name
 * where a project's tsconfig loads no Node or DOM types (the labs set `"types": []`), and which
 * gives the same copy for a JSON value.
 */
export function cloneJson<T>(value: T): T {
  return value === undefined ? value : JSON.parse(JSON.stringify(value));
}

/**
 * The value with the schema's defaults filled in, as ajv's `useDefaults` fills them (a
 * JSON parameter's default is its schema's `default`): a key `properties`
 * names and the value lacks takes its `default`, and so does an item `prefixItems` names past the
 * array's end. A key that is null keeps its null. Defaults are filled wherever the schema applies
 * to the value but inside `anyOf`, `oneOf`, `not`, `if`, `contains` and `propertyNames`, whose
 * matches are unknown until the checks run. The value given is not changed: a copy comes back.
 */
export function withDefaults(schema: Schema, value: unknown): unknown {
  if (schema === null || typeof schema !== "object" || Array.isArray(schema)) return value;
  const s = schema as Object_;
  let out = value;
  if (value !== null && typeof value === "object" && !Array.isArray(value)) {
    const o: Record<string, unknown> = { ...(value as Object_) };
    const properties = (s.properties ?? {}) as Record<string, Schema>;
    for (const [key, sub] of Object.entries(properties)) {
      if (!holds(o, key) && sub !== null && typeof sub === "object" && "default" in sub) o[key] = cloneJson((sub as Object_).default);
    }
    const patterns = Object.entries((s.patternProperties ?? {}) as Record<string, Schema>).map(([p, sub]) => [regex(p), sub] as const);
    for (const key of Object.keys(o)) {
      if (!written(o[key])) continue;
      let named = false;
      if (Object.prototype.hasOwnProperty.call(properties, key)) {
        named = true;
        o[key] = withDefaults(properties[key]!, o[key]);
      }
      for (const [re, sub] of patterns) {
        if (re.test(key)) {
          named = true;
          o[key] = withDefaults(sub, o[key]);
        }
      }
      if (!named && s.additionalProperties !== undefined) o[key] = withDefaults(s.additionalProperties as Schema, o[key]);
    }
    if (s.dependentSchemas && typeof s.dependentSchemas === "object") {
      for (const [key, sub] of Object.entries(s.dependentSchemas as Record<string, Schema>)) if (holds(o, key)) Object.assign(o, withDefaults(sub, o) as object);
    }
    out = o;
  } else if (Array.isArray(value)) {
    const a = [...value];
    const prefix = Array.isArray(s.prefixItems) ? (s.prefixItems as Schema[]) : [];
    prefix.forEach((sub, i) => {
      if (i >= a.length && sub !== null && typeof sub === "object" && "default" in sub && a.length === i) a.push(cloneJson((sub as Object_).default));
    });
    for (let i = 0; i < a.length; i++) a[i] = withDefaults(i < prefix.length ? prefix[i]! : (s.items as Schema) ?? true, a[i]);
    out = a;
  }
  if (Array.isArray(s.allOf)) for (const sub of s.allOf as Schema[]) out = withDefaults(sub, out);
  // Only the branch the value takes: `if` decides it as the checks will.
  if (s.if !== undefined) {
    const branch = validate(s.if as Schema, out) === undefined ? s.then : s.else;
    if (branch !== undefined) out = withDefaults(branch as Schema, out);
  }
  return out;
}

/** The first failure of `value` against `schema`, or `undefined` when it passes. */
export function validate(schema: Schema, value: unknown): Failure | undefined {
  const failures: Failure[] = [];
  check(schema, value, "", failures);
  return failures[0];
}

/** What a passing check evaluated of an object's keys, for `unevaluatedProperties`. */
type Evaluated = Set<string>;

/** Checks `value`; on failure pushes why and returns `undefined`, on success the keys it evaluated. */
function check(schema: Schema, value: unknown, path: string, out: Failure[]): Evaluated | undefined {
  const evaluated: Evaluated = new Set();
  if (schema === true) return evaluated;
  if (schema === false) return fail(out, path, "no value is allowed here");
  if (schema === null || typeof schema !== "object" || Array.isArray(schema)) return evaluated;
  const s = schema as Object_;

  if (s.type !== undefined) {
    const types = Array.isArray(s.type) ? (s.type as string[]) : [s.type as string];
    if (!types.some((t) => is(t, value))) return fail(out, path, `must be ${types.join(" or ")}, got ${kind(value)}`);
  }
  if (Array.isArray(s.enum) && !s.enum.some((e) => equal(e, value))) return fail(out, path, `must be one of ${s.enum.map((e) => JSON.stringify(e)).join(", ")}`);
  if ("const" in s && !equal(s.const, value)) return fail(out, path, `must be ${JSON.stringify(s.const)}`);

  if (typeof value === "number") {
    if (typeof s.multipleOf === "number" && !multiple(value, s.multipleOf)) return fail(out, path, `must be a multiple of ${s.multipleOf}`);
    if (typeof s.maximum === "number" && value > s.maximum) return fail(out, path, `must be at most ${s.maximum}`);
    if (typeof s.exclusiveMaximum === "number" && value >= s.exclusiveMaximum) return fail(out, path, `must be less than ${s.exclusiveMaximum}`);
    if (typeof s.minimum === "number" && value < s.minimum) return fail(out, path, `must be at least ${s.minimum}`);
    if (typeof s.exclusiveMinimum === "number" && value <= s.exclusiveMinimum) return fail(out, path, `must be more than ${s.exclusiveMinimum}`);
  }
  if (typeof value === "string") {
    // A length counts code points, as JSON Schema says.
    const length = [...value].length;
    if (typeof s.maxLength === "number" && length > s.maxLength) return fail(out, path, `must be at most ${s.maxLength} characters long`);
    if (typeof s.minLength === "number" && length < s.minLength) return fail(out, path, `must be at least ${s.minLength} characters long`);
    if (typeof s.pattern === "string" && !regex(s.pattern).test(value)) return fail(out, path, `must match the pattern ${s.pattern}`);
    if (typeof s.format === "string" && !format(s.format, value)) return fail(out, path, `must be a ${s.format}`);
  }
  if (Array.isArray(value)) {
    if (typeof s.maxItems === "number" && value.length > s.maxItems) return fail(out, path, `must have at most ${s.maxItems} items`);
    if (typeof s.minItems === "number" && value.length < s.minItems) return fail(out, path, `must have at least ${s.minItems} items`);
    if (s.uniqueItems === true) {
      for (let i = 0; i < value.length; i++) for (let j = i + 1; j < value.length; j++) if (equal(item(value[i]), item(value[j]))) return fail(out, path, `must have no two equal items (${i} and ${j})`);
    }
    const prefix = Array.isArray(s.prefixItems) ? (s.prefixItems as Schema[]) : [];
    for (let i = 0; i < Math.min(prefix.length, value.length); i++) if (!check(prefix[i]!, item(value[i]), `${path}/${i}`, out)) return undefined;
    if (s.items !== undefined) for (let i = prefix.length; i < value.length; i++) if (!check(s.items as Schema, item(value[i]), `${path}/${i}`, out)) return undefined;
    if (s.contains !== undefined) {
      let matched = 0;
      for (let i = 0; i < value.length; i++) if (check(s.contains as Schema, item(value[i]), path, [])) matched++;
      const min = typeof s.minContains === "number" ? s.minContains : 1;
      if (matched < min) return fail(out, path, min === 1 ? "must contain an item that matches its `contains` schema" : `must contain at least ${min} items that match its \`contains\` schema`);
      if (typeof s.maxContains === "number" && matched > s.maxContains) return fail(out, path, `must contain at most ${s.maxContains} items that match its \`contains\` schema`);
    }
  }
  if (value !== null && typeof value === "object" && !Array.isArray(value)) {
    const o = value as Object_;
    const keys = Object.keys(o).filter((key) => written(o[key]));
    if (typeof s.maxProperties === "number" && keys.length > s.maxProperties) return fail(out, path, `must have at most ${s.maxProperties} keys`);
    if (typeof s.minProperties === "number" && keys.length < s.minProperties) return fail(out, path, `must have at least ${s.minProperties} keys`);
    // A key is the object's own: `toString` and `__proto__` are keys only where the value has them.
    const has = (key: string) => holds(o, key);
    if (Array.isArray(s.required)) for (const key of s.required as string[]) if (!has(key)) return fail(out, path, `must have the key "${key}"`);
    if (s.dependentRequired && typeof s.dependentRequired === "object") {
      for (const [key, needs] of Object.entries(s.dependentRequired as Record<string, string[]>)) {
        if (has(key)) for (const need of needs) if (!has(need)) return fail(out, path, `must have the key "${need}" beside "${key}"`);
      }
    }
    const properties = (s.properties ?? {}) as Record<string, Schema>;
    const patterns = Object.entries((s.patternProperties ?? {}) as Record<string, Schema>).map(([p, sub]) => [regex(p), sub] as const);
    for (const key of keys) {
      const at = `${path}/${pointer(key)}`;
      let named = false;
      if (Object.prototype.hasOwnProperty.call(properties, key)) {
        named = true;
        if (!check(properties[key]!, o[key], at, out)) return undefined;
      }
      for (const [re, sub] of patterns) {
        if (re.test(key)) {
          named = true;
          if (!check(sub, o[key], at, out)) return undefined;
        }
      }
      if (named) evaluated.add(key);
      else if (s.additionalProperties !== undefined) {
        if (!check(s.additionalProperties as Schema, o[key], at, out)) return s.additionalProperties === false ? fail(out, at, `is a key the schema does not allow`, true) : undefined;
        evaluated.add(key);
      }
      if (s.propertyNames !== undefined && !check(s.propertyNames as Schema, key, at, [])) return fail(out, at, `is a key whose name the schema does not allow`);
    }
    if (s.dependentSchemas && typeof s.dependentSchemas === "object") {
      for (const [key, sub] of Object.entries(s.dependentSchemas as Record<string, Schema>)) {
        if (!has(key)) continue;
        const e = check(sub, value, path, out);
        if (!e) return undefined;
        e.forEach((k) => evaluated.add(k));
      }
    }
  }

  // The applicators: a subschema's evaluated keys count only where it passed.
  if (Array.isArray(s.allOf)) {
    for (const sub of s.allOf as Schema[]) {
      const e = check(sub, value, path, out);
      if (!e) return undefined;
      e.forEach((k) => evaluated.add(k));
    }
  }
  if (Array.isArray(s.anyOf)) {
    let any = false;
    for (const sub of s.anyOf as Schema[]) {
      const e = check(sub, value, path, []);
      if (e) {
        any = true;
        e.forEach((k) => evaluated.add(k));
      }
    }
    if (!any) return fail(out, path, "must match one of its `anyOf` schemas");
  }
  if (Array.isArray(s.oneOf)) {
    let matched = 0;
    for (const sub of s.oneOf as Schema[]) {
      const e = check(sub, value, path, []);
      if (e) {
        matched++;
        e.forEach((k) => evaluated.add(k));
      }
    }
    if (matched !== 1) return fail(out, path, matched === 0 ? "must match one of its `oneOf` schemas" : "must match only one of its `oneOf` schemas, and matches more");
  }
  if (s.not !== undefined && check(s.not as Schema, value, path, [])) return fail(out, path, "must not match its `not` schema");
  if (s.if !== undefined) {
    const e = check(s.if as Schema, value, path, []);
    const branch = e ? s.then : s.else;
    if (e) e.forEach((k) => evaluated.add(k));
    if (branch !== undefined) {
      const b = check(branch as Schema, value, path, out);
      if (!b) return undefined;
      b.forEach((k) => evaluated.add(k));
    }
  }
  if (value !== null && typeof value === "object" && !Array.isArray(value)) {
    const o = value as Object_;
    if (s.unevaluatedProperties === false) {
      for (const key of Object.keys(o)) if (written(o[key]) && !evaluated.has(key)) return fail(out, `${path}/${pointer(key)}`, "is a key the schema does not allow");
    }
    // `true` evaluates every key left, for an enclosing `unevaluatedProperties: false`.
    if (s.unevaluatedProperties === true) for (const key of Object.keys(o)) if (written(o[key])) evaluated.add(key);
  }
  return evaluated;
}

function fail(out: Failure[], path: string, message: string, replace = false): undefined {
  if (replace) out.length = 0;
  out.push({ path, message });
  return undefined;
}

function is(type: string, v: unknown): boolean {
  switch (type) {
    case "null": return v === null;
    case "boolean": return typeof v === "boolean";
    case "number": return typeof v === "number" && Number.isFinite(v);
    case "integer": return typeof v === "number" && Number.isInteger(v);
    case "string": return typeof v === "string";
    case "array": return Array.isArray(v);
    case "object": return v !== null && typeof v === "object" && !Array.isArray(v);
    default: return false;
  }
}

function kind(v: unknown): string {
  if (v === null) return "null";
  if (Array.isArray(v)) return "an array";
  if (typeof v === "number") return Number.isInteger(v) ? "an integer" : "a number";
  if (typeof v === "object") return "an object";
  return `a ${typeof v}`;
}

/** JSON equality: numbers by value (1 and 1.0 alike), objects by their keys whatever the order. */
function equal(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== typeof b || a === null || b === null || typeof a !== "object") return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  if (Array.isArray(a)) {
    if (a.length !== (b as unknown[]).length) return false;
    for (let i = 0; i < a.length; i++) if (!equal(item(a[i]), item((b as unknown[])[i]))) return false;
    return true;
  }
  const ka = Object.keys(a as object).filter((k) => written((a as Object_)[k]));
  const kb = Object.keys(b as object).filter((k) => written((b as Object_)[k]));
  return ka.length === kb.length && ka.every((k) => holds(b as Object_, k) && equal((a as Object_)[k], (b as Object_)[k]));
}

// The value is checked as the JSON it is stored as. JSON.stringify leaves out an object's key
// whose value is undefined, a function or a symbol, and writes such an item of an array, a hole
// too, as null: a key left out that way is no key of the object, and such an item is null.
const written = (v: unknown) => v !== undefined && typeof v !== "function" && typeof v !== "symbol";
const holds = (o: Object_, key: string) => Object.prototype.hasOwnProperty.call(o, key) && written(o[key]);
const item = (v: unknown) => (written(v) ? v : null);

function multiple(value: number, of: number): boolean {
  const q = value / of;
  // Within the division's own rounding error of a whole number, as a decimal multiple like 0.01
  // needs; past the largest double, the remainder says it (1e308 is a multiple of 0.5).
  if (!Number.isFinite(q)) return value % of === 0;
  // Zero is the multiple of everything; no other value is a multiple by rounding to it.
  const whole = Math.round(q);
  if (whole === 0) return value === 0;
  // The error a division of two doubles leaves grows with the quotient, a few units in its last place.
  return Math.abs(q - whole) <= Number.EPSILON * 8 * Math.abs(q);
}

const regexes = new Map<string, RegExp>();
/**
 * A pattern as ECMA-262 reads it, with Unicode on, as JSON Schema says. One that Unicode mode
 * refuses and the plain mode reads (`[\w-\.]`, `\-`, in patterns copied from elsewhere) is read
 * plainly, as the databases' own validators read it.
 */
function regex(pattern: string): RegExp {
  let re = regexes.get(pattern);
  if (re) return re;
  try {
    re = new RegExp(pattern, "u");
  } catch (unicode) {
    try {
      re = new RegExp(pattern);
    } catch {
      throw unicode;
    }
  }
  regexes.set(pattern, re);
  return re;
}

/** A key as one JSON Pointer segment. */
function pointer(key: string): string {
  return key.replace(/~/g, "~0").replace(/\//g, "~1");
}

const DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
const TIME = /^(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:z|([+-])(\d{2}):(\d{2}))$/i;

function date(v: string): boolean {
  const m = DATE.exec(v);
  if (!m) return false;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const days = [31, y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return mo >= 1 && mo <= 12 && d >= 1 && d <= days[mo - 1]!;
}

function time(v: string): boolean {
  const m = TIME.exec(v);
  if (!m) return false;
  const [h, mi, s] = [Number(m[1]), Number(m[2]), Number(m[3])];
  if (h > 23 || mi > 59 || s > 60) return false;
  if (m[4] && (Number(m[5]) > 23 || Number(m[6]) > 59)) return false;
  // A leap second is 23:59:60 in UTC.
  if (s === 60) {
    const offset = m[4] ? (m[4] === "+" ? 1 : -1) * (Number(m[5]) * 60 + Number(m[6])) : 0;
    const utc = (((h * 60 + mi - offset) % 1440) + 1440) % 1440;
    return utc === 23 * 60 + 59;
  }
  return true;
}

/** The formats this file asserts (RFC 3339, RFC 4122, RFC 5321's address form loosely, RFC 3986's scheme): any other passes. */
function format(name: string, v: string): boolean {
  switch (name) {
    case "date": return date(v);
    case "time": return time(v);
    case "date-time": {
      const t = v.search(/[tT ]/);
      return t === 10 && date(v.slice(0, 10)) && time(v.slice(11));
    }
    case "uuid": return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v);
    case "email": return email(v);
    case "ipv4": return /^(?:(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)\.){3}(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)$/.test(v);
    case "ipv6": return ipv6(v);
    case "uri": return uri(v);
    case "hostname": return hostname(v);
    case "regex":
      try {
        new RegExp(v, "u");
        return true;
      } catch {
        return false;
      }
    default: return true;
  }
}

// RFC 5321's mailbox, in ASCII: a dot-string or a quoted string, `@`, and a domain of letter,
// digit and hyphen labels or an address literal.
const ATOM = "[A-Za-z0-9!#$%&'*+\\-/=?^_`{|}~]+";
const DOT_STRING = new RegExp(`^${ATOM}(?:\\.${ATOM})*$`);
const QUOTED = /^"(?:[\x20\x21\x23-\x5b\x5d-\x7e]|\\[\x20-\x7e])*"$/;
const LABEL = /^[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?$/;

function email(v: string): boolean {
  const at = v.lastIndexOf("@");
  if (at < 1) return false;
  const local = v.slice(0, at);
  const domain = v.slice(at + 1);
  if (!(DOT_STRING.test(local) || QUOTED.test(local))) return false;
  if (domain.startsWith("[") && domain.endsWith("]")) {
    const literal = domain.slice(1, -1);
    return /^ipv6:/i.test(literal) ? ipv6(literal.slice(5)) : format("ipv4", literal);
  }
  return domain.length <= 255 && domain.split(".").every((label) => label.length <= 63 && LABEL.test(label));
}

// RFC 3986's URI: a scheme, `:`, an authority after `//` or a path, a query and a fragment, each
// of the characters its part allows, a `%` always followed by two hexadecimal digits.
const UNRESERVED = "A-Za-z0-9\\-._~";
const SUB_DELIMS = "!$&'()*+,;=";
const PCT = "%[0-9A-Fa-f]{2}";
const PCHAR = `(?:[${UNRESERVED}${SUB_DELIMS}:@]|${PCT})`;
const URI = new RegExp(`^([A-Za-z][A-Za-z0-9+.-]*):(?://([^/?#]*))?((?:/|${PCHAR})*)(?:\\?((?:${PCHAR}|[/?])*))?(?:#((?:${PCHAR}|[/?])*))?$`);
const USERINFO = new RegExp(`^(?:[${UNRESERVED}${SUB_DELIMS}:]|${PCT})*$`);
const REG_NAME = new RegExp(`^(?:[${UNRESERVED}${SUB_DELIMS}]|${PCT})*$`);

function uri(v: string): boolean {
  const m = URI.exec(v);
  if (!m) return false;
  const authority = m[2];
  // Without an authority a path may not start with `//`.
  if (authority === undefined) return !(m[3] ?? "").startsWith("//");
  const at = authority.lastIndexOf("@");
  if (at >= 0 && !USERINFO.test(authority.slice(0, at))) return false;
  const hostport = authority.slice(at + 1);
  if (hostport.startsWith("[")) {
    const close = hostport.indexOf("]");
    if (close < 0) return false;
    const literal = hostport.slice(1, close);
    const ok = /^v[0-9a-f]+\.[A-Za-z0-9\-._~!$&'()*+,;=:]+$/i.test(literal) || ipv6(literal);
    return ok && /^(?::\d*)?$/.test(hostport.slice(close + 1));
  }
  const colon = hostport.lastIndexOf(":");
  const host = colon >= 0 ? hostport.slice(0, colon) : hostport;
  if (colon >= 0 && !/^\d*$/.test(hostport.slice(colon + 1))) return false;
  return REG_NAME.test(host);
}

// RFC 1123's host name: labels of letters, digits and hyphens, at most 63 characters each and 253
// in all. A label with `--` in its third and fourth places is an A-label (`xn--`), whose Punycode
// must decode (RFC 3492) to a U-label that IDNA 2008's contextual rules allow (RFC 5892,
// appendix A): this checks the ones for MIDDLE DOT, Greek KERAIA, Hebrew GERESH and GERSHAYIM,
// KATAKANA MIDDLE DOT, the two sets of Arabic-Indic digits, the joiners after a virama, a combining
// mark first, and the code points RFC 5892 lists as DISALLOWED exceptions. A ZERO WIDTH NON-JOINER
// that follows no virama is taken where it joins two letters of a joining script, short of the
// Unicode joining types the rule names.
function hostname(v: string): boolean {
  if (v.length === 0 || v.length > 253) return false;
  return v.split(".").every((label) => {
    if (label.length === 0 || label.length > 63 || !/^[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?$/.test(label)) return false;
    if (label.slice(2, 4) !== "--") return true;
    if (label.slice(0, 2).toLowerCase() !== "xn") return false;
    const u = punycode(label.slice(4).toLowerCase());
    return u !== undefined && uLabel(u);
  });
}

const VIRAMA = new Set([0x094d, 0x09cd, 0x0a4d, 0x0acd, 0x0b4d, 0x0bcd, 0x0c4d, 0x0ccd, 0x0d3b, 0x0d3c, 0x0d4d, 0x0dca, 0x0e3a, 0x0eba, 0x0f84, 0x1039, 0x103a, 0x1714, 0x1715, 0x1734, 0x17d2, 0x1a60, 0x1b44, 0x1baa, 0x1bab, 0x1bf2, 0x1bf3, 0x2d7f, 0xa806, 0xa82c, 0xa8c4, 0xa953, 0xa9c0, 0xaaf6, 0xabed, 0x10a3f, 0x11046, 0x11070, 0x1107f, 0x110b9, 0x11133, 0x11134, 0x111c0, 0x11235, 0x112ea, 0x1134d, 0x11442, 0x114c2, 0x115bf, 0x1163f, 0x116b6, 0x1172b, 0x11839, 0x1193d, 0x1193e, 0x119e0, 0x11a34, 0x11a47, 0x11a99, 0x11c3f, 0x11d44, 0x11d45, 0x11d97, 0x11f41, 0x11f42]);
const DISALLOWED = new Set([0x0640, 0x07fa, 0x302e, 0x302f, 0x3031, 0x3032, 0x3033, 0x3034, 0x3035, 0x303b]);
const JOINING = /[\p{Script=Arabic}\p{Script=Syriac}\p{Script=Nko}\p{Script=Mongolian}\p{Script=Mandaic}\p{Script=Manichaean}\p{Script=Adlam}\p{Script=Hanifi_Rohingya}\p{Script=Sogdian}]/u;

function uLabel(label: string): boolean {
  const cps = [...label];
  if (cps.length === 0 || /^\p{M}/u.test(label) || label.slice(2, 4) === "--") return false;
  const at = (i: number) => cps[i] ?? "";
  for (let i = 0; i < cps.length; i++) {
    const c = cps[i]!.codePointAt(0)!;
    if (DISALLOWED.has(c)) return false;
    switch (c) {
      case 0x00b7: if (at(i - 1) !== "l" || at(i + 1) !== "l") return false; break;
      case 0x0375: if (!/\p{Script=Greek}/u.test(at(i + 1))) return false; break;
      case 0x05f3: case 0x05f4: if (!/\p{Script=Hebrew}/u.test(at(i - 1))) return false; break;
      case 0x30fb: if (!cps.some((x) => x !== "・" && /[\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Han}]/u.test(x))) return false; break;
      case 0x200d: if (!VIRAMA.has(at(i - 1).codePointAt(0) ?? -1)) return false; break;
      case 0x200c: {
        if (VIRAMA.has(at(i - 1).codePointAt(0) ?? -1)) break;
        let before = i - 1;
        while (before >= 0 && /\p{Mn}/u.test(at(before))) before--;
        let after = i + 1;
        while (after < cps.length && /\p{Mn}/u.test(at(after))) after++;
        if (!(JOINING.test(at(before)) && JOINING.test(at(after)))) return false;
        break;
      }
    }
  }
  const arabic = cps.some((x) => /[٠-٩]/.test(x));
  const extended = cps.some((x) => /[۰-۹]/.test(x));
  return !(arabic && extended);
}

/** RFC 3492's decoding of a Punycode string, or `undefined` where it is not one. */
function punycode(input: string): string | undefined {
  const base = 36, tMin = 1, tMax = 26, skew = 38, damp = 700;
  const delimiter = input.lastIndexOf("-");
  const output = delimiter > 0 ? [...input.slice(0, delimiter)].map((c) => c.codePointAt(0)!) : [];
  if (output.some((c) => c >= 0x80)) return undefined;
  let n = 128, i = 0, bias = 72;
  const adapt = (delta: number, points: number, first: boolean) => {
    delta = first ? Math.floor(delta / damp) : delta >> 1;
    delta += Math.floor(delta / points);
    let k = 0;
    while (delta > ((base - tMin) * tMax) >> 1) {
      delta = Math.floor(delta / (base - tMin));
      k += base;
    }
    return k + Math.floor(((base - tMin + 1) * delta) / (delta + skew));
  };
  const digit = (c: string) => (/[a-z]/.test(c) ? c.charCodeAt(0) - 97 : /[0-9]/.test(c) ? c.charCodeAt(0) - 22 : -1);
  let pos = delimiter > 0 ? delimiter + 1 : 0;
  if (pos >= input.length && delimiter <= 0) return undefined;
  while (pos < input.length) {
    const old = i;
    let w = 1;
    for (let k = base; ; k += base) {
      if (pos >= input.length) return undefined;
      const d = digit(input[pos++]!);
      if (d < 0) return undefined;
      i += d * w;
      const t = k <= bias ? tMin : k >= bias + tMax ? tMax : k - bias;
      if (d < t) break;
      w *= base - t;
      if (!Number.isSafeInteger(i) || !Number.isSafeInteger(w)) return undefined;
    }
    bias = adapt(i - old, output.length + 1, old === 0);
    n += Math.floor(i / (output.length + 1));
    i %= output.length + 1;
    if (n > 0x10ffff) return undefined;
    output.splice(i++, 0, n);
  }
  return String.fromCodePoint(...output);
}

function ipv6(v: string): boolean {
  if (!/^[0-9a-f:.]+$/i.test(v) || v.includes(":::")) return false;
  const halves = v.split("::");
  if (halves.length > 2) return false;
  const groups = (h: string) => (h === "" ? [] : h.split(":"));
  const parts = halves.flatMap(groups);
  const last = parts[parts.length - 1] ?? "";
  const v4 = last.includes(".");
  if (v4 && !format("ipv4", last)) return false;
  const words = parts.length + (v4 ? 1 : 0);
  if (parts.slice(0, v4 ? -1 : undefined).some((g) => !/^[0-9a-f]{1,4}$/i.test(g))) return false;
  return halves.length === 2 ? words < 8 : words === 8;
}
