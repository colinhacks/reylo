// The JSON Schema of a query's input and output: `query.inputSchema`, `query.outputSchema`, and
// Standard Schema's `jsonSchema` converter. A build step writes both into the module. The
// spellings are the engine's (engine/core/src/schema.rs); what differs
// is where a value stands. The output is the result as JSON writes it (`AsJson<T>` in src/index.ts)
// of the value `run` returns, which is each type's one representation (src/values.ts, the same
// table the run time decodes with): a 64-bit integer, a BigInt, as the JSON integer it is (what a
// serializer that writes a BigInt exactly writes, `JSON.rawJSON(String(v))`; JSON.stringify itself
// refuses a BigInt), a decimal its text, a timestamptz its ISO text in UTC, a date and a timestamp
// the wall-clock text the server wrote. The input is the parameters the caller passes.
import { bareName, schemaOf, type EngineType, type JsonSchema } from "./engine-type.ts";
import { columnKind, sqliteAffinity, sqliteWide, type ScalarKind } from "./values.ts";

type Dialect = "postgres" | "mysql" | "sqlite" | "duckdb";
type Schema = { [key: string]: unknown };
type Analysis = {
  params: readonly { name: string; type: EngineType; nullable?: boolean; optional?: boolean; schema?: JsonSchema; members?: readonly { written: string; schema?: JsonSchema }[] }[];
  columns: readonly { name: string; type: EngineType; nullable?: boolean }[];
  cardinality: "one" | "at_most_one" | "many" | "none";
};

/** The schema admitting null as well: a bare `type` gets `"null"` beside it, anything else an `anyOf` with null. */
function nullable(schema: Schema | JsonSchema): Schema | JsonSchema {
  if (schema === true || (typeof schema === "object" && Object.keys(schema).length === 0)) return schema;
  if (typeof schema === "object" && Object.keys(schema).length === 1 && typeof schema.type === "string") return { type: [schema.type, "null"] };
  return { anyOf: [schema, { type: "null" }] };
}

/** A PostGIS geometry inside a document, which its cast to JSON writes as GeoJSON. */
const GEOJSON: Schema = { type: "object", properties: { type: { type: "string" } }, required: ["type"], description: "a GeoJSON geometry" };

/** A SQLite value's storage class, as the affinity of the declared type the engine traced it to names it, or of the value it computes. */
function sqliteClass(t: EngineType): string {
  const name = "name" in t ? (t.name ?? "") : "";
  switch (t.kind) {
    case "boolean": return "bool";
    case "integer": return sqliteWide(name) ? "int8" : "integer";
    case "float": return "real";
    case "text": return "text";
    case "bytes": return "blob";
    case "union": {
      if (/^bool(ean)?$/i.test(name.trim())) return "bool";
      const affinity = sqliteAffinity(name);
      return affinity === "integer" && sqliteWide(name) ? "int8" : affinity;
    }
    default: return "";
  }
}

/** A value of a type with no structure of its own here, standing where `inJson` says: inside a JSON document, as a parameter the caller passes, or as `run` returns it. */
function scalar(t: EngineType, dialect: Dialect, inJson: boolean, input: boolean): Schema {
  const name = bareName(t);
  if (dialect === "sqlite") {
    // A SQLite INTEGER is a number as a parameter (src/render.ts), and its JSON spelling inside a
    // document; as `run` returns it, its kind's.
    if (inJson || input) {
      switch (sqliteClass(t)) {
        case "integer": case "int8": return { type: "integer", format: "int64" };
        case "real": return { type: "number", format: "double" };
        case "text": return { type: "string" };
        case "blob": return { type: "string", format: "byte", contentEncoding: "base64" };
        case "numeric": return { type: ["number", "string"] };
        case "bool": return { type: "boolean" };
        default: return {};
      }
    }
    const kinds = columnKind(t, dialect).split("|");
    if (kinds.length > 1) {
      // A NUMERIC: whichever storage class the row stored, an integer being a number too.
      const types = [...new Set(kinds.map((k) => kindSchema(k as ScalarKind, t, dialect).type as string))];
      return { type: types.includes("number") ? types.filter((type) => type !== "integer") : types };
    }
    return kindSchema(kinds[0] as ScalarKind, t, dialect);
  }
  if (t.kind === "union" && !(dialect === "postgres" && (name === "inet" || name === "cidr"))) return { anyOf: t.members.map((m) => scalar(m, dialect, inJson, input)) };
  const kind = columnKind(t, dialect);
  if (inJson) {
    // As the database writes the value inside a document.
    // JSON has no number for a non-finite value, so Postgres writes a float's or a decimal's as text;
    // MySQL stores none.
    if (t.kind === "integer" && name === "oid" && dialect === "postgres") return { type: "string" };
    // An integer is a JSON number in there, a MySQL `tinyint(1)` too.
    if (t.kind === "integer") return { type: "integer", format: t.bits > 32 || (t.bits === 32 && t.unsigned) ? "int64" : "int32" };
    if (t.kind === "float" || t.kind === "decimal") {
      const number = { type: "number", format: t.kind === "decimal" ? "decimal" : t.exponent_bits <= 8 ? "float" : "double" };
      const text = [...(t.nan ? ["NaN"] : []), ...(t.infinity ? ["Infinity", "-Infinity"] : [])];
      return dialect === "postgres" && text.length ? { anyOf: [number, { enum: text }] } : number;
    }
    if (kind === "bytes") return { type: "string", format: "hex" };
    // Postgres's own interval text: the run time converts a column's interval, never one inside a document.
    if (t.kind === "duration" && dialect === "postgres") return { type: "string", description: "pg type interval" };
    // A row whose columns were not read (a subquery's): an object of them.
    if (t.kind === "opaque" && name === "record") return { type: "object" };
    // An hstore: an object of its keys, each value text or null.
    if (kind === "hstore") return { type: "object", additionalProperties: { type: ["string", "null"] } };
    // PostGIS's cast to JSON: a GeoJSON geometry.
    if (t.kind === "geometry" && name === "geometry" && dialect === "postgres") return GEOJSON;
    // pgvector has no cast to JSON, so a vector inside a document is its text.
    if (kind === "vector") return { type: "string" };
  } else if (input && dialect === "duckdb") {
    // What DuckDB binds: a 64-bit or wider integer as a number, a BigInt or its digits; a decimal as its digits.
    if (kind === "int8") return { type: ["integer", "string"], format: "int64" };
    if (kind === "decimal") return { type: "string", format: "decimal" };
    if (kind === "timestamp") return { type: "string", format: "date-time" };
  } else if (input) {
    // A MySQL TINYINT(1) or BIT(1) column's parameter takes the truth value its rows return, or the integer it stores.
    if (dialect === "mysql" && kind === "bool" && t.kind !== "boolean") return { anyOf: [{ type: "boolean" }, { type: "integer", format: "int32" }] };
    // What every driver binds: a 64-bit integer as a number, a BigInt or its digits; a decimal as its digits.
    if (kind === "int8") return { type: ["integer", "string"], format: "int64" };
    if (kind === "decimal") return { type: "string", format: "decimal" };
    if (kind === "timestamp") return { type: "string", format: "date-time" };
  }
  if (kind.includes("|")) return { anyOf: kind.split("|").map((k) => kindSchema(k as ScalarKind, t, dialect)) };
  const schema = kindSchema(kind as ScalarKind, t, dialect);
  if (kind !== "text") return schema;
  switch (t.kind) {
    case "text": case "set": return { type: "string" };
    case "uuid": return { type: "string", format: "uuid" };
    case "time": return t.zone === undefined ? { type: "string", format: "time" } : { type: "string", description: `pg type ${name}` };
    case "duration": return { type: "string", format: "time" };
    default: return { type: "string", description: `${dialect === "mysql" ? "MySQL" : dialect === "duckdb" ? "DuckDB" : "pg"} type ${name || t.kind}` };
  }
}

/** The JSON form of a value of `kind` as `run` returns it, once JSON: each kind's one representation (src/values.ts). */
function kindSchema(kind: ScalarKind, t: EngineType, dialect: Dialect): Schema {
  switch (kind) {
    case "bool": return { type: "boolean" };
    // A SQLite INTEGER is 64 bits wide, and so is an unsigned 32-bit integer's range past int4's.
    case "int": return t.kind === "integer" && t.bits <= 32 && !(t.bits === 32 && t.unsigned) && dialect !== "sqlite" ? { type: "integer", format: "int32" } : { type: "integer", format: "int64" };
    case "float": return t.kind === "float" && t.exponent_bits <= 8 ? { type: "number", format: "float" } : { type: "number", format: "double" };
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

/** The JSON Schema of a value of `t`, as a result column (`input` false) or a parameter the caller passes (`input` true), or a structured parameter's member (`inRow`); `members`, a structured parameter's column list as written. */
export function typeSchema(t: EngineType, dialect: Dialect, input = false, inJson = false, inRow = false, members?: readonly { written: string; schema?: JsonSchema }[]): Schema | JsonSchema {
  const name = bareName(t);
  const kind = columnKind(t, dialect);
  // What the caller passes for a structured parameter: each key as a parameter of its type takes it.
  if (members !== undefined) {
    const record = t.kind === "record" ? t : t.kind === "array" && t.element.type.kind === "record" ? t.element.type : undefined;
    if (record !== undefined) {
      // A JSON member's schema as written, with its defaults and formats.
      const row = object(record.fields, (i) => (record.fields[i]!.nullable ? nullable : (s: Schema | JsonSchema) => s)(members[i]?.schema ?? typeSchema(record.fields[i]!.type, dialect, true, false, true)), (i) => record.fields[i]!.optional === true, true);
      return t.kind === "record" ? row : { type: "array", items: row };
    }
  }
  switch (t.kind) {
    case "text":
      if (t.enum !== undefined) return { type: "string", enum: t.enum };
      break;
    case "array": {
      // A type Postgres writes as its text inside a document, or a client gets as its own (a
      // bit string, a vector), is no array here.
      if (inJson ? (dialect === "postgres" && ["bit", "varbit", "vector", "halfvec", "sparsevec", "lseg", "polygon"].includes(name)) || (dialect === "mysql" && name === "bit") : !kind.endsWith("[]")) break;
      // A result's elements may be null unless the statement shows they are not; a parameter's
      // are strict, but a structured parameter's member keeps them (src/render.ts).
      const items = typeSchema(t.element.type, dialect, input, inJson, inRow);
      return { type: "array", items: (input ? inRow : t.element.nullable) ? nullable(items) : items };
    }
    case "record": {
      // A DuckDB struct's fields stand as a row's values do, each may be null.
      if (dialect === "duckdb" && !inJson) {
        return object(t.fields, (i) => nullable(typeSchema(t.fields[i]!.type, dialect, input)));
      }
      // A built-in geometric type is its text, inside a document too.
      if (dialect === "postgres" && ["point", "line", "box", "path", "circle"].includes(name)) break;
      return inJson ? object(t.fields, (i) => (t.fields[i]!.nullable ? nullable : (s: Schema | JsonSchema) => s)(typeSchema(t.fields[i]!.type, dialect, input, true))) : { type: "string", description: `the row value of ${name && name !== "record" ? name : "a record"} as Postgres prints it` };
    }
    case "json":
      // What a statement built: its keys and items are inside the document.
      if (t.content !== undefined) {
        const content = typeSchema(t.content.type, dialect, input, true);
        return t.content.nullable ? nullable(content) : content;
      }
      return schemaOf(t) ?? {};
    case "map":
      // A DuckDB map as its entries, each a key and its value, in order.
      if (dialect === "duckdb") return { type: "array", items: object([{ name: "key" }, { name: "value" }], (i) => (i === 0 ? typeSchema(t.key, dialect, input) : nullable(typeSchema(t.value.type, dialect, input)))) };
      break;
    case "tagged_union":
      // A tagged union as its tag and its value; inside a document, whose form of it the engine
      // does not read, any JSON.
      if (inJson) return {};
      return { anyOf: t.alternatives.map((a) => object([{ name: "tag" }, { name: "value" }], (i) => (i === 0 ? { const: a.tag } : (a.nullable ? nullable : (s: Schema | JsonSchema) => s)(typeSchema(a.type, dialect, input))))) };
    case "unknown":
      return {};
  }
  return scalar(t, dialect, inJson, input);
}

/** The query's input and output schemas: the parameters as an object, and the result as `run` returns it once JSON. */
export function querySchemas(analysis: Analysis, dialect: Dialect): { input: Schema; output: Schema } {
  // A typed JSON parameter's schema as written, with its defaults; one with a `default` at its root may be left out, as may one that takes null.
  const input = object(analysis.params, (i) => {
    const p = analysis.params[i]!;
    const s = p.schema ?? typeSchema(p.type, dialect, true, false, false, p.members);
    return p.nullable ? nullable(s) : s;
  }, (i) => {
    const p = analysis.params[i]!;
    const s = p.schema;
    return p.nullable === true || p.optional === true || (s !== null && typeof s === "object" && s !== undefined && "default" in s);
  }, true);
  const row = object(analysis.columns, (i) => {
    const c = analysis.columns[i]!;
    const s = typeSchema(c.type, dialect);
    return c.nullable ? nullable(s) : s;
  });
  const output: Schema = analysis.cardinality === "many" ? { type: "array", items: row } : analysis.cardinality === "one" ? row : analysis.cardinality === "at_most_one" ? { anyOf: [row, { type: "null" }] } : { type: "null" };
  return { input: { $schema: "https://json-schema.org/draft/2020-12/schema", ...input }, output: { $schema: "https://json-schema.org/draft/2020-12/schema", ...output } };
}
