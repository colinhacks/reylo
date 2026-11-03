// Type descriptors (spec/type-descriptor.ts) as each database's engine writes them, for the tests that
// render, check and bind values of a type without running an engine.
import type { EngineType, Field } from "../src/engine-type.ts";

const float = (exponent_bits: number, mantissa_bits: number, special = true): EngineType => ({ kind: "float", exponent_bits, mantissa_bits, ...(special ? { infinity: true, nan: true } : {}) });
const float8 = { ...float(11, 52), name: "float8" } as EngineType;
const point: EngineType = { kind: "record", name: "point", fields: [{ name: "x", type: float8 }, { name: "y", type: float8 }] };

/** A Postgres type by its catalog name, as the engine's registry writes it (engine/postgres `pgtypes.rs`); an extension's under the schema `public`. */
export function pg(name: string): EngineType {
  const known: Record<string, EngineType> = {
    bool: { kind: "boolean" },
    int2: { kind: "integer", bits: 16 },
    int4: { kind: "integer", bits: 32 },
    int8: { kind: "integer", bits: 64 },
    oid: { kind: "integer", bits: 32, unsigned: true },
    float4: float(8, 23),
    float8: float(11, 52),
    numeric: { kind: "decimal", min_scale: 0, max_scale: 16383, exclusive_min: "-1e131072", exclusive_max: "1e131072", infinity: true, nan: true },
    text: { kind: "text" }, varchar: { kind: "text" }, bpchar: { kind: "text" }, name: { kind: "text" },
    bytea: { kind: "bytes" },
    uuid: { kind: "uuid" },
    date: { kind: "date", min: "-4713-11-24", max: "+5874897-12-31", infinity: true },
    timestamp: { kind: "datetime", tick: "1/1000000", min: "-4713-11-24T00:00:00", max: "+294276-12-31T23:59:59.999999", infinity: true },
    timestamptz: { kind: "instant", tick: "1/1000000", min: "-4713-11-24T00:00:00Z", max: "+294276-12-31T23:59:59.999999Z", infinity: true },
    time: { kind: "time", tick: "1/1000000", max: "24:00:00" },
    timetz: { kind: "time", tick: "1/1000000", zone: "offset", max: "24:00:00" },
    interval: { kind: "duration", months: { max: "P2147483648M" }, days: { max: "P2147483648D" }, time: { tick: "1/1000000", max: "PT2562047788H54.775808S" }, infinity: true },
    json: { kind: "json" }, jsonb: { kind: "json" },
    xml: { kind: "xml", fragment: true },
    point,
    int4range: { kind: "range", subtype: { kind: "integer", bits: 32, name: "int4" } },
    bit: { kind: "array", element: { type: { kind: "boolean" } } },
    macaddr: { kind: "bytes", size: { exact: 6 } },
  };
  const extensions: Record<string, EngineType> = {
    vector: { kind: "array", element: { type: float(8, 23, false) } },
    halfvec: { kind: "array", element: { type: float(5, 10, false) } },
    sparsevec: { kind: "array", element: { type: float(8, 23, false) } },
    hstore: { kind: "map", key: { kind: "text" }, value: { nullable: true, type: { kind: "text" } } },
    geometry: { kind: "geometry", model: "planar" },
    geography: { kind: "geometry", model: "geodetic" },
    citext: { kind: "text" },
  };
  if (known[name]) return { ...known[name], name } as EngineType;
  if (extensions[name]) return { ...extensions[name], name: `public.${name}` } as EngineType;
  // `record` and every other type with no structure of its own here.
  return { kind: "opaque", name: ["record", "money", "xid", "void", "tsvector"].includes(name) ? name : `public.${name}` };
}

/** A MySQL type by its name, as the engine's prepare reading writes it (engine/mysql `mysql_types.rs`). */
export function my(name: string, length?: number): EngineType {
  const known: Record<string, EngineType> = {
    tinyint: { kind: "integer", bits: 8 }, "tinyint(1)": { kind: "integer", bits: 8 }, smallint: { kind: "integer", bits: 16 }, int: { kind: "integer", bits: 32 }, bigint: { kind: "integer", bits: 64 },
    float: float(8, 23, false), double: float(11, 52, false), decimal: { kind: "decimal" },
    varchar: { kind: "text" }, char: { kind: "text" }, text: { kind: "text" },
    varbinary: { kind: "bytes" }, blob: { kind: "bytes" },
    bit: { kind: "array", ...(length === undefined ? {} : { size: { exact: length } }), element: { type: { kind: "boolean" } } },
    date: { kind: "date" },
    datetime: { kind: "datetime", tick: "1/1000000" },
    timestamp: { kind: "instant", tick: "1/1000000", min: "1970-01-01T00:00:01Z", max: "2038-01-19T03:14:07.999999Z" },
    time: { kind: "duration", time: { tick: "1/1000000", max: "PT838H59M59S" } },
    json: { kind: "json" },
  };
  return { ...known[name]!, name } as EngineType;
}

/** A SQLite value by the analysis's affinity name for it, as the engine hands it on (engine/sqlite `descriptor`): a computed storage class, a column declared BIGINT's or BOOLEAN's under that name, a NUMERIC's union under its own. */
export function lite(name: string): EngineType {
  const text: EngineType = { kind: "text", ill_formed: "utf8" };
  const stored = (n: string): EngineType => ({ kind: "union", per: "value", name: n, members: [{ kind: "integer", bits: 64 }, { kind: "float", exponent_bits: 11, mantissa_bits: 52, infinity: true }, text, { kind: "bytes" }] });
  switch (name) {
    case "integer": return { kind: "integer", bits: 64 };
    case "int8": return { kind: "integer", bits: 64, name: "BIGINT" };
    case "real": return { kind: "float", exponent_bits: 11, mantissa_bits: 52, infinity: true };
    case "text": return text;
    case "blob": return { kind: "bytes" };
    case "numeric": return stored("NUMERIC");
    case "bool": return stored("BOOLEAN");
    default: throw new Error(`no SQLite type ${name}`);
  }
}

/** An array of `element`, whose elements may be NULL unless `nullable` is false; a Postgres array of any depth, as a column's is. */
export const array = (element: EngineType, nullable = true, name?: string): EngineType => ({ kind: "array", any_depth: true, element: { type: element, ...(nullable ? { nullable: true } : {}) }, ...(name === undefined ? {} : { name }) });

/** A text type whose values are `labels`: an enum, or a column a CHECK keeps to them. */
export const enumOf = (name: string, labels: string[]): EngineType => ({ kind: "text", enum: labels, name });

export const field = (name: string, type: EngineType, nullable = false, optional = false): Field => ({ name, type, ...(nullable ? { nullable: true } : {}), ...(optional ? { optional: true } : {}) });

/** A record of `fields`: a composite under its name, a row, an object a statement builds. */
export const record = (fields: Field[], name?: string): EngineType => ({ kind: "record", fields, ...(name === undefined ? {} : { name }) });

/** JSON a statement built, of `content`. */
export const doc = (content: EngineType, nullable = false): EngineType => ({ kind: "json", content: { type: content, ...(nullable ? { nullable: true } : {}) } });

/** JSON every value of which the engine's canonical JSON Schema `source` admits. */
export const checked = (source: unknown): EngineType => ({ kind: "json", schema: { dialect: "https://json-schema.org/draft/2020-12/schema", source: source as object } });

/** One of several types the execution chooses among (`per: "execution"`), or each value one of (`per: "value"`). */
export const either = (...members: EngineType[]): EngineType => ({ kind: "union", per: "execution", members });
export const oneOf = (...members: EngineType[]): EngineType => ({ kind: "union", per: "value", members });
