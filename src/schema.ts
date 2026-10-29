// The JSON Schema of a query's input and output: `query.inputSchema`, `query.outputSchema`, and
// Standard Schema's `jsonSchema` converter. A build step writes both into the module. The
// spellings are the engine's (engine/core/src/schema.rs, the framework's encoding table); what differs
// is where a value stands. The output is the result as JSON writes it (`Json<T>` in src/index.ts)
// of the value `run` returns, which is each type's one representation (src/values.ts, the same
// table the run time decodes with): a 64-bit integer, a BigInt, as the JSON integer it is (what a
// serializer that writes a BigInt exactly writes, `JSON.rawJSON(String(v))`; JSON.stringify itself
// refuses a BigInt), a decimal its text, a timestamptz its ISO text in UTC, a date and a timestamp
// the wall-clock text the server wrote. The input is the parameters the caller passes.
import type { EngineType, JsonSchema } from "./engine-type.ts";
import { scalarKind, type ScalarKind } from "./values.ts";

type Dialect = "postgres" | "mysql" | "sqlite" | "duckdb";
type Schema = { [key: string]: unknown };
type Analysis = {
  params: readonly { name: string; type: EngineType; nullable: boolean; schema?: JsonSchema }[];
  columns: readonly { name: string; type: EngineType; nullable: boolean }[];
  cardinality: "one" | "maybeOne" | "many" | "none";
};

/** The schema admitting null as well: a bare `type` gets `"null"` beside it, anything else an `anyOf` with null. */
function nullable(schema: Schema | JsonSchema): Schema | JsonSchema {
  if (schema === true || (typeof schema === "object" && Object.keys(schema).length === 0)) return schema;
  if (typeof schema === "object" && Object.keys(schema).length === 1 && typeof schema.type === "string") return { type: [schema.type, "null"] };
  return { anyOf: [schema, { type: "null" }] };
}

/** A PostGIS geometry inside a document, which its cast to JSON writes as GeoJSON. */
const GEOJSON: Schema = { type: "object", properties: { type: { type: "string" } }, required: ["type"], description: "a GeoJSON geometry" };

/** A base type by its catalog name, standing where `inJson` says: inside a JSON document, as a parameter the caller passes, or as `run` returns it. */
function scalar(name: string, dialect: Dialect, inJson: boolean, input: boolean): Schema {
  if (name.includes("|")) return { anyOf: name.split("|").map((part) => scalar(part, dialect, inJson, input)) };
  if (dialect === "sqlite") {
    // A SQLite INTEGER is a number as a parameter (src/render.ts), and its JSON spelling inside a
    // document; as `run` returns it, its kind's.
    if (inJson || input) {
      switch (name) {
        case "integer": case "int8": return { type: "integer", format: "int64" };
        case "real": return { type: "number", format: "double" };
        case "text": return { type: "string" };
        case "blob": return { type: "string", format: "byte", contentEncoding: "base64" };
        case "numeric": return { type: ["number", "string"] };
        case "bool": return { type: "boolean" };
        default: return {};
      }
    }
    const kinds = scalarKind(name, dialect).split("|");
    if (kinds.length > 1) {
      // A NUMERIC: whichever storage class the row stored, an integer being a number too.
      const types = [...new Set(kinds.map((k) => kindSchema(k as ScalarKind, name, dialect).type as string))];
      return { type: types.includes("number") ? types.filter((t) => t !== "integer") : types };
    }
    return kindSchema(kinds[0] as ScalarKind, name, dialect);
  }
  if (inJson) {
    // As the database writes the value inside a document.
    // JSON has no number for a non-finite value, so Postgres writes a float's or a decimal's as text;
    // MySQL stores none.
    const nonFinite = { enum: ["NaN", "Infinity", "-Infinity"] };
    switch (name) {
      case "int8": return { type: "integer", format: "int64" };
      case "float4": return dialect === "postgres" ? { anyOf: [{ type: "number", format: "float" }, nonFinite] } : { type: "number", format: "float" };
      case "float8": return dialect === "postgres" ? { anyOf: [{ type: "number", format: "double" }, nonFinite] } : { type: "number", format: "double" };
      case "numeric": return dialect === "postgres" ? { anyOf: [{ type: "number", format: "decimal" }, nonFinite] } : { type: "number", format: "decimal" };
      case "bytea": return { type: "string", format: "hex" };
      // Postgres's own interval text: the run time converts a column's interval, never one inside a document.
      case "interval": return { type: "string", description: "pg type interval" };
      // A row whose columns were not read (a subquery's): an object of them.
      case "record": return { type: "object" };
      // An hstore: an object of its keys, each value text or null.
      case "hstore": return { type: "object", additionalProperties: { type: ["string", "null"] } };
      // PostGIS's cast to JSON: a GeoJSON geometry.
      case "geometry": return GEOJSON;
      // An oid is not one of the numbers JSON has a number for: Postgres writes its text.
      case "oid": return { type: "string" };
      // pgvector has no cast to JSON, so a vector inside a document is its text.
      case "vector": case "halfvec": return { type: "string" };
    }
  } else if (input && dialect === "duckdb") {
    // What DuckDB binds: a 64-bit or wider integer as a number, a BigInt or its digits; a decimal as its digits.
    const kind = scalarKind(name, dialect);
    if (kind === "int8") return { type: ["integer", "string"], format: "int64" };
    if (kind === "decimal") return { type: "string", format: "decimal" };
    if (kind === "timestamp") return { type: "string", format: "date-time" };
  } else if (input) {
    // What every driver binds: a 64-bit integer as a number, a BigInt or its digits; a decimal as its digits.
    switch (name) {
      case "int8": return { type: ["integer", "string"], format: "int64" };
      case "numeric": return { type: "string", format: "decimal" };
      case "timestamp": return { type: "string", format: "date-time" };
    }
  }
  const kind = scalarKind(name, dialect);
  if (kind.includes("|")) return { anyOf: kind.split("|").map((k) => kindSchema(k as ScalarKind, name, dialect)) };
  const schema = kindSchema(kind as ScalarKind, name, dialect);
  if (kind !== "text") return schema;
  switch (name) {
    case "text": case "varchar": case "bpchar": case "name": case "char": case "citext": return { type: "string" };
    case "uuid": return { type: "string", format: "uuid" };
    case "time": return { type: "string", format: "time" };
    case "unknown": return {};
    default: return { type: "string", description: `${dialect === "mysql" ? "MySQL" : dialect === "duckdb" ? "DuckDB" : "pg"} type ${name}` };
  }
}

/** The JSON form of a value of `kind` as `run` returns it, once JSON: each kind's one representation (src/values.ts). */
function kindSchema(kind: ScalarKind, name: string, dialect: Dialect): Schema {
  switch (kind) {
    case "bool": return { type: "boolean" };
    // A SQLite INTEGER is 64 bits wide, and so is an oid's range past int4's.
    case "int": return name === "oid" || dialect === "sqlite" || name === "uinteger" ? { type: "integer", format: "int64" } : { type: "integer", format: "int32" };
    case "float": return name === "float4" || (dialect === "duckdb" && name === "float") ? { type: "number", format: "float" } : { type: "number", format: "double" };
    // A BigInt, as the JSON integer it is.
    case "int8": return { type: "integer", format: "int64" };
    case "decimal": return { type: "string", format: "decimal" };
    case "timestamptz": return { type: "string", format: "date-time" };
    // RFC 3339's duration, which JSON Schema's `duration` format names, has no sign and no fraction.
    case "interval": return { type: "string", description: "an ISO 8601 duration with its sign in front (P1Y2M3DT4H5M6.789S, -P1D); one whose parts differ in sign signs each part (P1M-1D)" };
    case "date": return { type: "string", format: "date" };
    // A date and a time with no zone, which RFC 3339's date-time does not admit.
    case "timestamp": return { type: "string", description: "a date and time without a time zone, as the database wrote it (2026-10-20T10:00:00.123456)" };
    case "bytes": return { type: "string", format: "byte", contentEncoding: "base64" };
    case "vector": return { type: "array", items: { type: "number", format: "float" } };
    case "hstore": return { type: "object", additionalProperties: { type: ["string", "null"] } };
    case "text": return { type: "string" };
    case "json": return {};
    case "unknown": return {};
  }
}

/** An object of `fields`, each spelled by `field`, every key required unless `optional` says otherwise. */
function object(fields: readonly { name: string }[], field: (f: number) => Schema | JsonSchema, optional: (f: number) => boolean = () => false, closed = false): Schema {
  const properties: Record<string, unknown> = {};
  const required: string[] = [];
  // A later field of a name replaces an earlier one, as a row object and `JSON.parse` keep the last.
  fields.forEach((f, i) => {
    properties[f.name] = field(i);
    const at = required.indexOf(f.name);
    if (at >= 0) required.splice(at, 1);
    if (!optional(i)) required.push(f.name);
  });
  return { type: "object", properties, required, ...(closed ? { additionalProperties: false } : {}) };
}

/** The JSON Schema of a value of `t`, as a result column (`input` false) or a parameter the caller passes (`input` true), or a structured parameter's member (`inRow`). */
export function typeSchema(t: EngineType, dialect: Dialect, input = false, inJson = false, inRow = false): Schema | JsonSchema {
  switch (t.kind) {
    case "scalar": return scalar(t.name, dialect, inJson, input);
    case "enum": return { type: "string", enum: t.labels };
    case "array": {
      // A result's elements may be null unless the statement shows they are not; a parameter's
      // are strict, but a structured parameter's member keeps them (src/render.ts).
      const items = typeSchema(t.of, dialect, input, inJson, inRow);
      return { type: "array", items: (input ? inRow || t.nullable_items : t.nullable_items !== false) ? nullable(items) : items };
    }
    case "composite": return inJson ? object(t.fields, (i) => (t.fields[i]!.nullable ? nullable : (s: Schema | JsonSchema) => s)(typeSchema(t.fields[i]!.type, dialect, input, true))) : { type: "string", description: `the row value of ${t.name} as Postgres prints it` };
    case "object": return object(t.fields, (i) => (t.fields[i]!.nullable ? nullable : (s: Schema | JsonSchema) => s)(typeSchema(t.fields[i]!.type, dialect, input, true)));
    case "list": {
      const items = typeSchema(t.of, dialect, input, true);
      return { type: "array", items: t.nullable_items ? nullable(items) : items };
    }
    case "json": return t.schema ?? {};
    // What the caller passes: each key as a parameter of its type takes it.
    case "rows": {
      // A JSON member's schema as written, with its defaults and formats.
      const row = object(t.fields, (i) => (t.fields[i]!.nullable ? nullable : (s: Schema | JsonSchema) => s)(t.fields[i]!.schema ?? typeSchema(t.fields[i]!.type, dialect, true, false, true)), (i) => t.fields[i]!.optional, true);
      return t.one ? row : { type: "array", items: row };
    }
    // A DuckDB struct's fields, map's entries and union's value stand as a row's values do, each may be null.
    case "struct": return object(t.fields, (i) => nullable(typeSchema(t.fields[i]!.type, dialect, input)));
    case "map": return { type: "array", items: object([{ name: "key" }, { name: "value" }], (i) => (i === 0 ? typeSchema(t.key, dialect, input) : nullable(typeSchema(t.value, dialect, input)))) };
    case "union": return { anyOf: t.members.map((m) => object([{ name: "tag" }, { name: "value" }], (i) => (i === 0 ? { const: m.name } : nullable(typeSchema(m.type, dialect, input))))) };
    case "unknown": return {};
  }
}

/** The query's input and output schemas: the parameters as an object, and the result as `run` returns it once JSON. */
export function querySchemas(analysis: Analysis, dialect: Dialect): { input: Schema; output: Schema } {
  // A typed JSON parameter's schema as written, with its defaults; one with a `default` at its root may be left out, as may one that takes null.
  const input = object(analysis.params, (i) => {
    const p = analysis.params[i]!;
    const s = p.schema ?? typeSchema(p.type, dialect, true);
    return p.nullable ? nullable(s) : s;
  }, (i) => {
    const p = analysis.params[i]!;
    const s = p.schema;
    return p.nullable || (s !== null && typeof s === "object" && s !== undefined && "default" in s);
  }, true);
  const row = object(analysis.columns, (i) => {
    const c = analysis.columns[i]!;
    const s = typeSchema(c.type, dialect);
    return c.nullable ? nullable(s) : s;
  });
  const output: Schema = analysis.cardinality === "many" ? { type: "array", items: row } : analysis.cardinality === "one" ? row : analysis.cardinality === "maybeOne" ? { anyOf: [row, { type: "null" }] } : { type: "null" };
  return { input: { $schema: "https://json-schema.org/draft/2020-12/schema", ...input }, output: { $schema: "https://json-schema.org/draft/2020-12/schema", ...output } };
}
