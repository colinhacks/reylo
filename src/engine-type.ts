// The engine's types: the type descriptor format (spec/type-descriptor.ts), which the engine answers
// every parameter and column with. Apart from the engine's host so that the run time and the
// modules a build writes can name it without Node's types: `src/execute.ts` reads a parameter's
// type from a module's `inputs`.
import type { Type } from "../spec/type-descriptor.ts";

export type { ArrayType, Field, JsonType, Position, RecordType, Type, UnionType } from "../spec/type-descriptor.ts";

/** A parameter's or a column's type as the engine writes it. */
export type EngineType = Type;

/**
 * The engine's canonical JSON Schema of a JSON value: `true`, `false`, `{}`, `type` (one name or a
 * list), `enum`, `anyOf`, an array's `prefixItems`, `items` and `minItems`, and an object's
 * `properties`, `required` and `additionalProperties`, which it always states.
 */
export type JsonSchema = boolean | { readonly [key: string]: unknown };

/**
 * A JSON type's JSON Schema in the engine's canonical form, where it has one: without the
 * `$schema` the type carries it with, and `true` and `false` as themselves, which the engine
 * writes as `{}` and `{ "not": {} }` to carry it (`Type::json_schema_of` in engine/core).
 */
export function schemaOf(t: Type): JsonSchema | undefined {
  if (t.kind !== "json" || t.schema === undefined) return undefined;
  const { $schema: _, ...keywords } = t.schema;
  const keys = Object.keys(keywords);
  if (keys.length === 0) return true;
  if (keys.length === 1 && keys[0] === "not" && isEmptyObject(keywords.not)) return false;
  return keywords;
}

const isEmptyObject = (v: unknown) => v !== null && typeof v === "object" && !Array.isArray(v) && Object.keys(v).length === 0;

/** A type's name without the schema that qualifies it: `vector` of `public.vector`, `"Odd"` of `app."Odd"` unquoted. */
export function bareName(t: Type): string {
  const name = "name" in t ? (t.name ?? "") : "";
  const last = /(?:^|\.)("(?:[^"]|"")*"|[^."]*)$/.exec(name)?.[1] ?? name;
  return last.startsWith('"') ? last.slice(1, -1).replaceAll('""', '"') : last;
}

/** The members of a union, or the type alone. */
export function membersOf(t: Type): readonly Type[] {
  return t.kind === "union" ? t.members : [t];
}

/**
 * A type with each domain it names written as the base type the domain is made from (the
 * analysis's `domains`): what a client decodes a domain's values by, and so what Reylo's
 * representations read, where the format keeps the domain's own name.
 */
export function withBases(t: Type, domains: Readonly<Record<string, string>> | undefined): Type {
  if (domains === undefined || !Object.keys(domains).length) return t;
  const named = "name" in t && t.name !== undefined && domains[t.name] !== undefined ? ({ ...t, name: domains[t.name] } as Type) : t;
  switch (named.kind) {
    case "array": return { ...named, element: { ...named.element, type: withBases(named.element.type, domains) } };
    case "set": return { ...named, element: { ...named.element, type: withBases(named.element.type, domains) } };
    case "record": return { ...named, fields: named.fields.map((f) => ({ ...f, type: withBases(f.type, domains) })) };
    case "json": return named.content === undefined ? named : { ...named, content: { ...named.content, type: withBases(named.content.type, domains) } };
    case "union": return { ...named, members: named.members.map((m) => withBases(m, domains)) };
    case "map": return { ...named, key: withBases(named.key, domains), value: { ...named.value, type: withBases(named.value.type, domains) } };
    case "range": case "multirange": return { ...named, subtype: withBases(named.subtype, domains) as typeof named.subtype };
    default: return named;
  }
}

/** An analysis's parameters and columns with each domain its types name written as its base type ([`withBases`]). */
export function analysisWithBases<A extends { params: readonly { type: Type }[]; columns: readonly { type: Type }[]; domains?: Readonly<Record<string, string>> }>(analysis: A): A {
  if (analysis.domains === undefined) return analysis;
  return { ...analysis, params: analysis.params.map((p) => ({ ...p, type: withBases(p.type, analysis.domains) })), columns: analysis.columns.map((c) => ({ ...c, type: withBases(c.type, analysis.domains) })) };
}
