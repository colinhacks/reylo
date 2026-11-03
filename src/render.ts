// The engine's type model as TypeScript text. Reylo gives every value it returns its type itself,
// on every driver (src/values.ts): one representation per SQL type, so
// a column's type is its kind's, from the same table the run time decodes with. Inside a JSON value
// the database has already serialized it, so the same type reads as JSON writes it there, and a
// value nothing types there, or a `json` or `jsonb` value nothing constrains, is any JSON value:
// `Json`, which the generated module imports from `reylo`.
import { bareName, schemaOf, type EngineType, type Field } from "./engine-type.ts";
import { columnKind, kindType as typeOfKind, type Dialect } from "./values.ts";

// What Postgres writes inside a JSON document for a type whose form there its kind does not say
// (the spec's facts): the text of a type the format structures (a bit string, a vector, a
// geometric type, a network address) or does not (an extension's), an object of its keys for an
// hstore, GeoJSON for a geometry, and an object of its columns for a row whose columns were not
// read. A type named nowhere here, which the engine knows no form of, is any JSON value.
const json: Record<string, string> = {
  bit: "string", varbit: "string", vector: "string", halfvec: "string", sparsevec: "string",
  point: "string", line: "string", lseg: "string", box: "string", path: "string", polygon: "string", circle: "string",
  inet: "string", cidr: "string",
  money: "string", tsvector: "string", tsquery: "string", regclass: "string", regtype: "string",
  ltree: "string", lquery: "string", ltxtquery: "string",
  // Types with no cast to JSON, which Postgres writes inside a document as their text: built-in
  // system types, and the extensions' on Neon and Supabase (PostGIS's others, cube, seg, isn,
  // intarray, h3, hll, ip4r, pgx_ulid, prefix, rdkit, roaringbitmap, semver, unit, VectorChord).
  pg_lsn: "string", jsonpath: "string", xid: "string", xid8: "string",
  regproc: "string", regprocedure: "string", regoper: "string", regoperator: "string", regnamespace: "string", regrole: "string", regcollation: "string", regconfig: "string", regdictionary: "string",
  geography: "string", box2d: "string", box3d: "string", spheroid: "string", raster: "string",
  cube: "string", seg: "string", ean13: "string", isbn: "string", isbn13: "string", ismn: "string", ismn13: "string", issn: "string", issn13: "string", upc: "string",
  query_int: "string", h3index: "string", hll: "string", hll_hashval: "string",
  ip4: "string", ip4r: "string", ip6: "string", ip6r: "string", ipaddress: "string", iprange: "string", ulid: "string", prefix_range: "string",
  mol: "string", qmol: "string", xqmol: "string", reaction: "string", bfp: "string", sfp: "string", roaringbitmap: "string", s3vec: "string",
  semver: "string", semverrange: "string", semvermultirange: "string", unit: "string", unitrange: "string", unitmultirange: "string", rabitq8: "string", rabitq4: "string",
  // An hstore is an object of its keys inside a document, each value text or null (`{"a": "1", "b": null}`).
  hstore: "Record<string, string | null>",
  // PostGIS writes a geometry through its cast to JSON: a GeoJSON geometry, `{"type":"Point","coordinates":[1,2]}`.
  geometry: "{ type: string; [key: string]: Json | undefined }",
  // A row whose columns were not read (a subquery's): an object of them inside a document.
  record: "Record<string, Json>",
};

/** Any JSON value, as a module names it: the type `reylo` exports. */
export const JSON_TYPE = "Json";

/** A type that may also be null. `Json` already admits null, which a NOT NULL `jsonb` column can hold, so it reads the same either way. */
export const orNull = (type: string) => (type === JSON_TYPE ? type : `${type} | null`);

export const ident = (name: string) => (/^[A-Za-z_$][\w$]*$/.test(name) ? name : JSON.stringify(name));

/**
 * The TypeScript of a value inside a document the database built, typed by the SQL value it was
 * written from, as the database writes that value in there (the spec's facts on what each database
 * writes inside its own JSON): a number for an integer (Postgres's `oid` but its text) and for a
 * float or a decimal, with the text of each infinity or NaN the type holds beside it where JSON has
 * no number for it (SQLite writes ±Infinity as a number), the text of a date or of any type the
 * table above names, an object for an hstore and a row, GeoJSON for a geometry, and any JSON value
 * for one nothing types.
 */
function jsonImage(t: EngineType, dialect: Dialect): string {
  const name = bareName(t);
  switch (t.kind) {
    case "boolean": return "boolean";
    case "integer":
      if (dialect === "postgres" && name === "oid") return "string";
      return "number";
    case "float":
    case "decimal": {
      if (dialect === "sqlite") return "number";
      const text = [...(t.nan ? [`"NaN"`] : []), ...(t.infinity ? [`"Infinity"`, `"-Infinity"`] : [])];
      return ["number", ...text].join(" | ");
    }
    case "text":
      return t.enum?.length ? t.enum.map((l) => JSON.stringify(l)).join(" | ") : "string";
    case "union": {
      // SQLite: a value of a column a NUMERIC declares is a number or the text it holds.
      if (dialect === "sqlite") return /^bool(ean)?$/i.test(name) ? "boolean" : name.toUpperCase() === "NUMERIC" ? "number | string" : JSON_TYPE;
      if (dialect === "postgres" && json[name]) return json[name]!;
      const parts = t.members.map((m) => jsonImage(m, dialect));
      return parts.includes(JSON_TYPE) ? JSON_TYPE : [...new Set(parts.flatMap((part) => part.split(" | ")))].join(" | ");
    }
    case "bytes":
      return dialect === "sqlite" ? JSON_TYPE : "string";
    case "uuid": case "date": case "time": case "datetime": case "instant": case "duration": case "set": case "range": case "multirange": case "xml":
      return dialect === "sqlite" ? JSON_TYPE : "string";
    default:
      return (dialect === "postgres" ? json[name] : undefined) ?? JSON_TYPE;
  }
}

export function renderType(t: EngineType, inJson = false, dialect: Dialect = "postgres"): string {
  const name = bareName(t);
  switch (t.kind) {
    case "array": {
      // A type Postgres structures as an array and writes as its text (a bit string, a vector), or
      // an array of row values, which no driver parses.
      if (inJson && dialect === "postgres" && json[name] !== undefined) return json[name]!;
      // MySQL writes a BIT inside a document as its bytes in base64.
      if (inJson && dialect === "mysql" && name === "bit") return "string";
      if (!inJson) {
        const kind = columnKind(t, dialect);
        if (!kind.endsWith("[]")) return typeOfKind(kind);
      }
      // Postgres lets any array hold NULL elements, whatever a column's NOT NULL says, so a result's
      // elements may be null unless the statement builds the array from values it shows are not
      // (`array[1, 2]`, `array_agg` over a NOT NULL column). A parameter's are strict
      // (`renderParamType`).
      const item = renderType(t.element.type, inJson, dialect);
      const inner = t.element.nullable ? orNull(item) : item;
      return /[|&]/.test(inner) ? `(${inner})[]` : `${inner}[]`;
    }
    case "record":
      // A row inside a document is an object of its fields; a built-in geometric type, its text.
      if (inJson) return dialect === "postgres" && name !== "record" && json[name] !== undefined ? json[name]! : renderFields(t.fields, dialect);
      // A DuckDB struct's fields stand as a row's values do.
      if (dialect === "duckdb") {
        return t.fields.length ? `{ ${lastOfEachName(t.fields).map((f) => `${ident(f.name)}: ${orNull(renderType(f.type, false, dialect))}`).join("; ")} }` : "Record<never, never>";
      }
      // On the wire a Postgres row value is the text the server prints, which no driver parses.
      return "string";
    // A JSON value: what the statement built, the values a schema admits, or any JSON.
    case "json":
      if (t.content !== undefined) {
        const content = renderType(t.content.type, true, dialect);
        return t.content.nullable ? orNull(content) : content;
      }
      return t.schema === undefined ? JSON_TYPE : schemaType(schemaOf(t));
    case "map":
      // A DuckDB map's entries stand as a row's values do.
      if (dialect === "duckdb") return `{ key: ${renderType(t.key, false, dialect)}; value: ${orNull(renderType(t.value.type, false, dialect))} }[]`;
      break;
    case "tagged_union":
      // A tagged union is one alternative, `{ tag, value }`, as DuckDB's client returns it; inside
      // a document, whose form of it the engine does not read, any JSON.
      if (inJson) break;
      return t.alternatives.length ? t.alternatives.map((a) => `{ tag: ${JSON.stringify(a.tag)}; value: ${a.nullable ? orNull(renderType(a.type, false, dialect)) : renderType(a.type, false, dialect)} }`).join(" | ") : "never";
    case "text":
      // An enum's labels, or the strings a CHECK keeps a column to.
      if (t.enum !== undefined) return t.enum.length ? t.enum.map((l) => JSON.stringify(l)).join(" | ") : "string";
      break;
    // Inside a document the database built, a value nothing types is still JSON; anywhere else
    // (SQLite's `json_extract`, a column of no declared type) it may be any value.
    case "unknown":
      return inJson ? JSON_TYPE : "unknown";
  }
  return inJson ? jsonImage(t, dialect) : typeOfKind(columnKind(t, dialect));
}

/**
 * The TypeScript type of a JSON value the engine's canonical JSON Schema describes: the values a
 * CHECK's schema admits (engine/core/src/schema_type.rs), or a typed JSON parameter's or a
 * `json('…')` member's, where a schema that says nothing more is any JSON value, `Json`. An object
 * whose other keys may hold anything is typed by the keys it names alone: a TypeScript object type
 * admits other keys already, and one with no index signature flags a misspelled key, in a read or
 * in an object literal. One whose schema types its other keys, or that names none, says so with an
 * index signature, `Json | undefined` beside named keys, since TypeScript requires every named key's
 * type, `undefined` included for an optional one, to fit the signature. So does an open object that
 * is one of a union's objects (`besideObjects`): `"k" in v` narrows a union to the members that name
 * `k` or have a signature, and an open member that names no `k` may still hold one of any type.
 */
export function schemaType(schema: unknown, besideObjects = false): string {
  if (schema === false) return "never";
  if (schema === null || typeof schema !== "object" || Array.isArray(schema)) return JSON_TYPE;
  const m = schema as Record<string, unknown>;
  if (Array.isArray(m.anyOf)) {
    const shared = besideObjects || objectAlternatives(m) > 1;
    return union(m.anyOf.map((s) => schemaType(s, shared)));
  }
  if (Array.isArray(m.enum)) return union(m.enum.map(literalType));
  if (m.not !== undefined) return "never";
  const kinds = kindsOf(m);
  return kinds.length ? union(kinds.map((kind) => kindType(kind, m, besideObjects))) : JSON_TYPE;
}

function kindsOf(m: Record<string, unknown>): string[] {
  return typeof m.type === "string" ? [m.type] : Array.isArray(m.type) ? m.type.filter((k): k is string => typeof k === "string") : [];
}

/** How many object types a schema's union holds: its object members, nested unions' and literal objects included. */
function objectAlternatives(schema: unknown): number {
  if (schema === null || typeof schema !== "object" || Array.isArray(schema)) return 0;
  const m = schema as Record<string, unknown>;
  if (Array.isArray(m.anyOf)) return m.anyOf.reduce((n: number, s) => n + objectAlternatives(s), 0);
  if (Array.isArray(m.enum)) return m.enum.filter((v) => v !== null && typeof v === "object" && !Array.isArray(v)).length;
  return kindsOf(m).includes("object") ? 1 : 0;
}

function kindType(kind: string, m: Record<string, unknown>, besideObjects: boolean): string {
  switch (kind) {
    case "string": return "string";
    case "integer": case "number": return "number";
    case "boolean": return "boolean";
    case "null": return "null";
    case "array": {
      const prefix = Array.isArray(m.prefixItems) ? m.prefixItems.map((s) => schemaType(s)) : [];
      const rest = m.items === undefined ? JSON_TYPE : schemaType(m.items);
      if (!prefix.length) return rest === "never" ? "[]" : `${grouped(rest)}[]`;
      const min = typeof m.minItems === "number" ? m.minItems : 0;
      const items = prefix.map((item, i) => (i < min ? item : `${grouped(item)}?`));
      if (rest !== "never") items.push(`...${grouped(rest)}[]`);
      return `[${items.join(", ")}]`;
    }
    case "object": {
      const properties = m.properties !== null && typeof m.properties === "object" && !Array.isArray(m.properties) ? (m.properties as Record<string, unknown>) : {};
      const required = Array.isArray(m.required) ? m.required : [];
      const fields = Object.entries(properties).map(([key, value]) => `${ident(key)}${required.includes(key) ? "" : "?"}: ${schemaType(value)}`);
      const rest = m.additionalProperties === undefined || m.additionalProperties === false ? "never" : schemaType(m.additionalProperties);
      const open = m.additionalProperties === undefined || rest === JSON_TYPE;
      const signature = fields.length ? (rest !== "never" && rest !== JSON_TYPE) || (open && besideObjects) : rest !== "never";
      if (signature) fields.push(`[key: string]: ${fields.length ? `${JSON_TYPE} | undefined` : rest}`);
      if (fields.length) return `{ ${fields.join("; ")} }`;
      return m.additionalProperties === undefined ? `Record<string, ${JSON_TYPE}>` : "Record<never, never>";
    }
    default: return JSON_TYPE;
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

/** A union of JSON types: `Json` absorbs the rest, and `never` adds nothing. */
function union(parts: string[]): string {
  const kept = [...new Set(parts)];
  if (kept.includes(JSON_TYPE)) return JSON_TYPE;
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
function renderFields(fields: readonly Field[], dialect: Dialect): string {
  fields = lastOfEachName(fields);
  return fields.length ? `{ ${fields.map((f) => `${ident(f.name)}: ${f.nullable ? orNull(renderType(f.type, true, dialect)) : renderType(f.type, true, dialect)}`).join("; ")} }` : "Record<never, never>";
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
export function renderParamType(t: EngineType, dialect: Dialect = "postgres", inRow = false, members?: readonly { readonly written: string }[]): string {
  const kind = columnKind(t, dialect);
  // DuckDB binds a number, a BigInt or the digits for a 64-bit or wider integer, the exact text for
  // a DECIMAL, the text for a date or a time, and bytes for a BLOB; a struct takes an object of
  // its fields and a map its entries.
  if (dialect === "duckdb") {
    if (kind === "int8") return "number | bigint | string";
    if (kind === "decimal" || kind === "date" || kind === "timestamp" || kind === "timestamptz" || kind === "interval") return "string";
    if (t.kind === "record" && t.fields.length && t.fields.every((f) => f.name === "")) return `readonly [${t.fields.map((f) => orNull(renderParamType(f.type, dialect, true))).join(", ")}]`;
    if (t.kind === "record" && members === undefined) return t.fields.length ? `{ ${t.fields.map((f) => `${ident(f.name)}: ${orNull(renderParamType(f.type, dialect, true))}`).join("; ")} }` : "Record<never, never>";
    if (t.kind === "map") return `readonly { key: ${renderParamType(t.key, dialect, true)}; value: ${orNull(renderParamType(t.value.type, dialect, true))} }[]`;
  }
  // A JSON parameter takes the JSON a result of its type holds, which `.run()` writes with
  // `JSON.stringify`; its null is JSON's null where the statement takes no SQL NULL
  // (src/execute.ts), so `Json` admits it either way.
  if (t.kind === "json") return t.schema === undefined ? JSON_TYPE : schemaType(schemaOf(t));
  // A structured parameter: one row, or an array of rows, each key as a parameter of its type
  // takes it, `?` where the caller may leave it out, `| null` where it may be null.
  if (members !== undefined) {
    const record = t.kind === "record" ? t : t.kind === "array" && t.element.type.kind === "record" ? t.element.type : undefined;
    if (record !== undefined) {
      const fields = record.fields.map((f) => {
        const type = renderParamType(f.type, dialect, true);
        return `${ident(f.name)}${f.optional ? "?" : ""}: ${f.nullable ? orNull(type) : type}`;
      });
      const row = fields.length ? `{ ${fields.join("; ")} }` : "Record<never, never>";
      return t.kind === "record" ? row : `readonly ${row}[]`;
    }
  }
  if (dialect !== "sqlite") {
    if (kind === "int8") return "number | bigint | string";
    if (kind.split("|").every((part) => part === "timestamp" || part === "timestamptz" || part === "date")) return "string";
  }
  // A SQLite driver binds a number for an INTEGER. A BOOLEAN column has NUMERIC affinity, so a
  // parameter written to or compared with one is NUMERIC, and takes the truth value the column
  // returns, which the run time binds as 1 or 0.
  // A BIGINT, INT8 or UNSIGNED BIG INT column comes back as a bigint, which its parameter takes.
  if (dialect === "sqlite") {
    if (kind === "int") return "number";
    if (kind === "int8") return "number | bigint";
    if (kind === "int|float|text") return "number | string | boolean";
    if (kind === "bool") return "boolean | number";
  }
  // A MySQL TINYINT(1) or BIT(1) column returns a truth value, which its parameter takes beside the integer it stores.
  if (dialect === "mysql" && kind === "bool" && t.kind !== "boolean") return "boolean | number";
  // Every driver binds a Uint8Array (a Buffer is one) for a binary value, mariadb and Neon's
  // neon() before 0.10 once `execute` has converted it, and a digit string for a decimal.
  if (kind === "bytes") return "Uint8Array";
  if (kind === "decimal" && dialect !== "sqlite") return "string";
  if (t.kind === "array") {
    // An array of row values comes back as the text Postgres prints, which the parameter takes back
    // beside an array of each row's own text.
    if (dialect === "postgres" && kind === "text" && ((t.element.type.kind === "record" && json[bareName(t.element.type)] === undefined) || bareName(t.element.type) === "record")) return "string | string[]";
    if (kind.endsWith("[]")) {
      const inner = inRow ? orNull(renderParamType(t.element.type, dialect, inRow)) : renderParamType(t.element.type, dialect);
      return /[|&]/.test(inner) ? `(${inner})[]` : `${inner}[]`;
    }
  }
  return renderType(t, false, dialect);
}
