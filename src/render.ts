// The engine's type model as TypeScript text. Reylo gives every value it returns its type itself,
// on every driver (src/values.ts): one representation per SQL type, so
// a column's type is its kind's, from the same table the run time decodes with. Inside a JSON value
// the database has already serialized it, so the same type reads as JSON writes it there.
import type { EngineType } from "./engine.ts";
import { kindType as typeOfKind, scalarKind, type Dialect } from "./values.ts";

// What a database writes inside a JSON document (`json_build_object`, `json_agg`, a composite
// inside one): a number for the integers and decimals, the text of a date, hexadecimal for bytes.
// JSON has no number for a non-finite value, so Postgres writes a float's or a decimal's as text.
const nonFinite = `number | "NaN" | "Infinity" | "-Infinity"`;
const json: Record<string, string> = {
  bool: "boolean",
  // An oid is not one of the numbers JSON has a number for: Postgres writes its text.
  int2: "number", int4: "number", oid: "string", float4: nonFinite, float8: nonFinite,
  int8: "number", numeric: nonFinite,
  text: "string", varchar: "string", bpchar: "string", name: "string", char: "string", citext: "string",
  uuid: "string", time: "string", timetz: "string", interval: "string",
  date: "string", timestamp: "string", timestamptz: "string",
  bytea: "string",
  money: "string", inet: "string", cidr: "string", macaddr: "string", macaddr8: "string", xml: "string",
  bit: "string", varbit: "string", tsvector: "string", tsquery: "string", regclass: "string", regtype: "string",
  int4range: "string", int8range: "string", numrange: "string", tsrange: "string", tstzrange: "string", daterange: "string",
  int4multirange: "string", int8multirange: "string", nummultirange: "string", tsmultirange: "string", tstzmultirange: "string", datemultirange: "string",
  ltree: "string", lquery: "string", ltxtquery: "string",
  // Types with no cast to JSON, which Postgres writes inside a document as their text: built-in
  // geometric and system types, and the extensions' on Neon and Supabase (pgvector, PostGIS's
  // others, cube, seg, isn, intarray, h3, hll, ip4r, pgx_ulid, prefix, rdkit, roaringbitmap,
  // semver, unit, VectorChord).
  point: "string", line: "string", lseg: "string", box: "string", path: "string", polygon: "string", circle: "string",
  pg_lsn: "string", jsonpath: "string", xid: "string", xid8: "string",
  regproc: "string", regprocedure: "string", regoper: "string", regoperator: "string", regnamespace: "string", regrole: "string", regcollation: "string", regconfig: "string", regdictionary: "string",
  vector: "string", halfvec: "string", sparsevec: "string", geography: "string", box2d: "string", box3d: "string", spheroid: "string", raster: "string",
  cube: "string", seg: "string", ean13: "string", isbn: "string", isbn13: "string", ismn: "string", ismn13: "string", issn: "string", issn13: "string", upc: "string",
  query_int: "string", h3index: "string", hll: "string", hll_hashval: "string",
  ip4: "string", ip4r: "string", ip6: "string", ip6r: "string", ipaddress: "string", iprange: "string", ulid: "string", prefix_range: "string",
  mol: "string", qmol: "string", xqmol: "string", reaction: "string", bfp: "string", sfp: "string", roaringbitmap: "string", s3vec: "string",
  semver: "string", semverrange: "string", semvermultirange: "string", unit: "string", unitrange: "string", unitmultirange: "string", rabitq8: "string", rabitq4: "string",
  // An hstore is an object of its keys inside a document, each value text or null (`{"a": "1", "b": null}`).
  hstore: "Record<string, string | null>",
  // PostGIS writes a geometry through its cast to JSON: a GeoJSON geometry, `{"type":"Point","coordinates":[1,2]}`.
  geometry: "{ type: string; [key: string]: unknown }",
  // A row whose columns were not read (a subquery's): an object of them inside a document.
  record: "Record<string, unknown>",
};
// MySQL stores no infinity and no NaN, so its numbers inside a document are numbers.
const mysqlJson: Record<string, string> = { ...json, float4: "number", float8: "number", numeric: "number" };
const sqliteJson: Record<string, string> = { integer: "number", int8: "number", real: "number", numeric: "number | string", text: "string", bool: "boolean" };

export const ident = (name: string) => (/^[A-Za-z_$][\w$]*$/.test(name) ? name : JSON.stringify(name));

export function renderType(t: EngineType, inJson = false, dialect: Dialect = "postgres"): string {
  switch (t.kind) {
    case "scalar": {
      // `a|b` is either: a MySQL column whose type the execution decides (`:n + 1` is a BIGINT
      // or a DOUBLE by what is bound), or a truth value a document holds as `true` or as 1.
      if (!inJson) return typeOfKind(scalarKind(t.name, dialect));
      const parts = t.name.split("|").map((name) => (dialect === "sqlite" ? sqliteJson : dialect === "mysql" ? mysqlJson : json)[name] ?? "unknown");
      return parts.includes("unknown") ? "unknown" : [...new Set(parts.flatMap((part) => part.split(" | ")))].join(" | ");
    }
    case "enum": return t.labels.length ? t.labels.map((l) => JSON.stringify(l)).join(" | ") : "string";
    case "array": {
      if (((t.of.kind === "scalar" && t.of.name === "record") || t.of.kind === "composite") && !inJson) return "string";
      // Postgres lets any array hold NULL elements, whatever a column's NOT NULL says, so a result's
      // elements may be null unless the statement builds the array from values it shows are not
      // (`array[1, 2]`, `array_agg` over a NOT NULL column). A parameter's are strict
      // (`renderParamType`).
      const inner = renderType(t.of, inJson, dialect) + (t.nullable_items !== false ? " | null" : "");
      return /[|&]/.test(inner) ? `(${inner})[]` : `${inner}[]`;
    }
    // A JSON value whose shape the statement builds.
    case "object": return renderFields(t.fields, dialect);
    // A composite whose columns the catalog gave: on the wire it is a row value like `record`,
    // which no driver parses; inside a JSON document the database writes it as an object.
    case "composite": return inJson ? renderFields(t.fields, dialect) : "string";
    case "list": {
      const inner = renderType(t.of, true, dialect) + (t.nullable_items ? " | null" : "");
      return /[|&]/.test(inner) ? `(${inner})[]` : `${inner}[]`;
    }
    case "json": return t.schema === undefined ? "unknown" : schemaType(t.schema);
    // Only a parameter is one; no column is.
    case "rows": return renderParamType(t, dialect);
    case "unknown": return "unknown";
  }
}

/**
 * The TypeScript type of a JSON value the engine's canonical JSON Schema describes: the values a
 * CHECK's schema admits (engine/core/src/schema_type.rs). An object that may hold keys besides the ones
 * it names says so with an index signature, `unknown` beside named keys, since TypeScript requires
 * every named key's type, `undefined` included for an optional one, to fit the signature.
 */
export function schemaType(schema: unknown, input = false): string {
  if (schema === false) return "never";
  if (schema === null || typeof schema !== "object" || Array.isArray(schema)) return "unknown";
  const m = schema as Record<string, unknown>;
  const sub = (s: unknown) => schemaType(s, input);
  if (Array.isArray(m.anyOf)) return union(m.anyOf.map(sub));
  if (Array.isArray(m.enum)) return union(m.enum.map(literalType));
  if (m.not !== undefined) return "never";
  const kinds = typeof m.type === "string" ? [m.type] : Array.isArray(m.type) ? m.type.filter((k): k is string => typeof k === "string") : [];
  return kinds.length ? union(kinds.map((kind) => kindType(kind, m, input))) : "unknown";
}

/**
 * `input`: the value is one the caller passes, a typed JSON parameter's or a `json('…')` member's,
 * whose keys the schema does not name are ignored unless it closes the object; its type names the
 * keys alone, so that TypeScript still flags a misspelled one in an object literal.
 */
function kindType(kind: string, m: Record<string, unknown>, input = false): string {
  const schemaType_ = (s: unknown) => schemaType(s, input);
  switch (kind) {
    case "string": return "string";
    case "integer": case "number": return "number";
    case "boolean": return "boolean";
    case "null": return "null";
    case "array": {
      const prefix = Array.isArray(m.prefixItems) ? m.prefixItems.map(schemaType_) : [];
      const rest = m.items === undefined ? "unknown" : schemaType_(m.items);
      if (!prefix.length) return rest === "never" ? "[]" : `${grouped(rest)}[]`;
      const min = typeof m.minItems === "number" ? m.minItems : 0;
      const items = prefix.map((item, i) => (i < min ? item : `${grouped(item)}?`));
      if (rest !== "never") items.push(`...${grouped(rest)}[]`);
      return `[${items.join(", ")}]`;
    }
    case "object": {
      const properties = m.properties !== null && typeof m.properties === "object" && !Array.isArray(m.properties) ? (m.properties as Record<string, unknown>) : {};
      const required = Array.isArray(m.required) ? m.required : [];
      const fields = Object.entries(properties).map(([key, value]) => `${ident(key)}${required.includes(key) ? "" : "?"}: ${schemaType_(value)}`);
      const open = m.additionalProperties === undefined || m.additionalProperties === true;
      const rest = m.additionalProperties === undefined || m.additionalProperties === false || (input && open && Object.keys(properties).length > 0) ? "never" : schemaType_(m.additionalProperties);
      if (rest !== "never") fields.push(`[key: string]: ${fields.length ? "unknown" : rest}`);
      if (fields.length) return `{ ${fields.join("; ")} }`;
      return m.additionalProperties === undefined ? "Record<string, unknown>" : "Record<never, never>";
    }
    default: return "unknown";
  }
}

/** The TypeScript literal type of one JSON value. */
function literalType(v: unknown): string {
  if (v === null) return "null";
  if (typeof v === "string") return JSON.stringify(v);
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  if (Array.isArray(v)) return `[${v.map(literalType).join(", ")}]`;
  if (typeof v === "object") {
    const entries = Object.entries(v as Record<string, unknown>);
    return entries.length ? `{ ${entries.map(([key, x]) => `${ident(key)}: ${literalType(x)}`).join("; ")} }` : "Record<never, never>";
  }
  return "unknown";
}

/** A union of types: `unknown` absorbs the rest, and `never` adds nothing. */
function union(parts: string[]): string {
  const kept = [...new Set(parts)];
  if (kept.includes("unknown")) return "unknown";
  const real = kept.filter((p) => p !== "never");
  return real.length ? real.join(" | ") : "never";
}

/** A type in parentheses where it is a union at its own top level, as an array's element or an optional tuple element must be. */
function grouped(ts: string): string {
  let depth = 0;
  let quoted = false;
  for (let i = 0; i < ts.length; i++) {
    const c = ts[i]!;
    if (quoted) {
      if (c === "\\") i++;
      else if (c === '"') quoted = false;
    } else if (c === '"') quoted = true;
    else if ("{([<".includes(c)) depth++;
    else if ("})]>".includes(c)) depth--;
    else if (c === "|" && depth === 0) return `(${ts})`;
  }
  return ts;
}

/**
 * The last of each name: a row a driver builds is an object, which keeps the last of two columns
 * of one name, and so does `JSON.parse` of two keys of one name. The engine reports the columns.
 */
export function lastOfEachName<T extends { name: string }>(items: readonly T[]): T[] {
  return items.filter((item, i) => !items.slice(i + 1).some((later) => later.name === item.name));
}

/** An object type of JSON-encoded fields, each nullable as the engine says. */
function renderFields(fields: { name: string; type: EngineType; nullable: boolean }[], dialect: Dialect): string {
  fields = lastOfEachName(fields);
  return fields.length ? `{ ${fields.map((f) => `${ident(f.name)}: ${renderType(f.type, true, dialect)}${f.nullable ? " | null" : ""}`).join("; ")} }` : "Record<never, never>";
}

/**
 * The type of a value whose JSON Schema the statement writes, a typed JSON parameter's
 * (`:p::jsonb('…')`) or a `json('…')` member's: the caller's, so a key its schema does not name is
 * left out of the type (src/render.ts, `schemaType`'s `input`); `undefined` for any other.
 */
export function declaredType(t: EngineType, declared: unknown): string | undefined {
  return declared !== undefined && t.kind === "json" && t.schema !== undefined ? schemaType(t.schema, true) : undefined;
}

/**
 * The type a parameter accepts: what `run` returns for its SQL type, so that a row goes back in as
 * it came out (the round-trip rule), and what else every driver binds
 * the same way. Every Postgres and MySQL driver binds a number, a BigInt or a digit string for a
 * 64-bit integer, so `limit :n` takes a number. On MySQL the engine types a parameter by the
 * column it is written to or compared with (engine/mysql `column_types`), since the prepare
 * describes every integer context as a BIGINT and every BIT one alike: a BIGINT column's is
 * `int8`, a TINYINT(1)'s or a BIT(1)'s `bool|int4`, which takes the truth value the row returns;
 * a SQLite BOOLEAN column's is NUMERIC, which takes it too, and `src/execute.ts` binds it as 1 or 0. A `timestamptz` is
 * an instant, which `run` returns as its ISO 8601 text in UTC, and takes that text, or any that
 * names its zone (`src/execute.ts`), and no `Date`, which `date.toISOString()` writes as such a
 * string. A `timestamp` or a `date` (MySQL's DATETIME and TIMESTAMP are the engine's
 * `timestamp`) is a wall-clock reading with no zone, and the drivers write a `Date`'s in different
 * zones, the app's (pg, mysql2), the server's (postgres.js) or UTC (PGlite, Bun.sql), so it takes
 * the text alone. An array parameter's elements take no NULL, as a parameter with a cast is its
 * cast's type: a NULL element
 * of `= any(:ids)` matches nothing. An array inside a structured
 * parameter's member (`inRow`) keeps NULL elements, which `src/rows.ts` binds, so an array a
 * statement builds with NULL elements (`array_agg` over a left join is `(string | null)[]`) goes
 * back into a row.
 */
export function renderParamType(t: EngineType, dialect: Dialect = "postgres", inRow = false): string {
  if (t.kind === "scalar" && dialect !== "sqlite") {
    if (t.name === "int8") return "number | bigint | string";
    if (t.name.split("|").every((name) => name === "timestamp" || name === "timestamptz" || name === "date")) return "string";
  }
  // A SQLite driver binds a number for an INTEGER. A BOOLEAN column has NUMERIC affinity, so a
  // parameter written to or compared with one is NUMERIC, and takes the truth value the column
  // returns, which the run time binds as 1 or 0.
  // A column declared BIGINT's parameter keeps an INTEGER's type for now.
  if (t.kind === "scalar" && dialect === "sqlite" && t.name === "integer") return "number";
  // A BIGINT, INT8 or UNSIGNED BIG INT column comes back as a bigint, which its parameter takes.
  if (t.kind === "scalar" && dialect === "sqlite" && t.name === "int8") return "number | bigint";
  if (t.kind === "scalar" && dialect === "sqlite" && t.name === "numeric") return "number | string | boolean";
  if (t.kind === "scalar" && dialect === "sqlite" && t.name === "bool") return "boolean | number";
  // Every driver binds a Uint8Array (a Buffer is one) for a binary value, mariadb and Neon's
  // neon() before 0.10 once `execute` has converted it, a digit string for a decimal, and any
  // JSON value for a JSON parameter.
  if (t.kind === "scalar" && (t.name === "bytea" || t.name === "blob")) return "Uint8Array";
  if (t.kind === "scalar" && t.name === "numeric" && dialect !== "sqlite") return "string";
  if (t.kind === "json") return t.schema === undefined ? "unknown" : schemaType(t.schema);
  // A structured parameter: each key as a parameter of its type takes it, `?` where the caller
  // may leave it out, `| null` where it may be null.
  if (t.kind === "rows") {
    const fields = t.fields.map((f) => `${ident(f.name)}${f.optional ? "?" : ""}: ${declaredType(f.type, f.schema) ?? renderParamType(f.type, dialect, true)}${f.nullable ? " | null" : ""}`);
    const row = fields.length ? `{ ${fields.join("; ")} }` : "Record<never, never>";
    return t.one ? row : `readonly ${row}[]`;
  }
  if (t.kind === "array") {
    // An array of row values comes back as the text Postgres prints, which the parameter takes back
    // beside an array of each row's own text.
    if ((t.of.kind === "scalar" && t.of.name === "record") || t.of.kind === "composite") return "string | string[]";
    if (inRow) return `(${renderParamType(t.of, dialect, inRow)} | null)[]`;
    const inner = renderParamType(t.of, dialect);
    return /[|&]/.test(inner) ? `(${inner})[]` : `${inner}[]`;
  }
  return renderType(t, false, dialect);
}
