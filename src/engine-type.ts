// The engine's types: the type descriptor format (spec/index.ts), which the engine answers
// every parameter and column with. Apart from the engine's host so that the run time and the
// modules a build writes can name it without Node's types: `src/execute.ts` reads a parameter's
// type from a module's `inputs`.
import type { Type } from "../spec/index.ts";

export type { Alternative, ArrayType, EnumType, Field, IntersectionType, JsonType, Slot, StructType, TableType, TaggedUnionType, TupleType, Type, UnionType } from "../spec/index.ts";

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
  const name = "label" in t ? (t.label ?? "") : "";
  const last = /(?:^|\.)("(?:[^"]|"")*"|[^."]*)$/.exec(name)?.[1] ?? name;
  return last.startsWith('"') ? last.slice(1, -1).replaceAll('""', '"') : last;
}

/** Whether a type is a MySQL `SET`: an array of its members, each at most once, which `.run()` returns as a `Set`. */
export function isMysqlSet(t: Type): boolean {
  return t.kind === "array" && t.distinct === true && t.label === "set";
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
  const named = "label" in t && t.label !== undefined && domains[t.label] !== undefined ? ({ ...t, label: domains[t.label] } as Type) : t;
  const slot = <S extends { type: Type }>(s: S): S => ({ ...s, type: withBases(s.type, domains) });
  switch (named.kind) {
    case "enum": return { ...named, type: withBases(named.type, domains) };
    case "array": return { ...named, element: slot(named.element) };
    case "struct": return { ...named, fields: named.fields.map(slot), ...(named.extra && { extra: slot(named.extra) }) };
    case "table": return { ...named, columns: named.columns.map(slot) };
    case "tuple": return { ...named, elements: named.elements.map(slot), ...(named.optional_elements && { optional_elements: named.optional_elements.map(slot) }), ...(named.rest && { rest: slot(named.rest) }) };
    case "json": return named.content === undefined ? named : { ...named, content: slot(named.content) };
    case "union": case "intersection": return { ...named, members: named.members.map((m) => withBases(m, domains)) };
    case "tagged_union": return { ...named, alternatives: named.alternatives.map(slot) };
    case "map": return { ...named, key: slot(named.key), value: slot(named.value) };
    case "range": case "multirange": return { ...named, subtype: withBases(named.subtype, domains) as typeof named.subtype };
    default: return named;
  }
}

/** An analysis's parameters and columns with each domain its types name written as its base type ([`withBases`]). */
export function analysisWithBases<A extends { params: readonly { type: Type }[]; columns: readonly { type: Type }[]; domains?: Readonly<Record<string, string>> }>(analysis: A): A {
  if (analysis.domains === undefined) return analysis;
  return { ...analysis, params: analysis.params.map((p) => ({ ...p, type: withBases(p.type, analysis.domains) })), columns: analysis.columns.map((c) => ({ ...c, type: withBases(c.type, analysis.domains) })) };
}

/** The mark an array takes under `allow_nested_arrays: true` ([`analysisWithNestedArrays`]), which is no part of the format. */
const NESTS = Symbol("nests");

/** Whether an array is one the project lets hold arrays of its elements at any depth. */
export function nestsAnyDeep(t: Type): boolean {
  return (t as { [NESTS]?: true })[NESTS] === true;
}

/**
 * A Postgres array type, by its name (`_int4`, `public._mood`), at its innermost level: the one a
 * column holds, which Postgres lets hold any number of dimensions, where the format writes it flat.
 */
function postgresArray(t: Type): boolean {
  return t.kind === "array" && bareName(t).startsWith("_") && t.element.type.kind !== "array";
}

function markNesting(t: Type): Type {
  switch (t.kind) {
    case "array": {
      const marked = { ...t, element: { ...t.element, type: markNesting(t.element.type) } };
      return postgresArray(t) ? Object.assign(marked, { [NESTS]: true }) : marked;
    }
    case "struct": return { ...t, fields: t.fields.map((f) => ({ ...f, type: markNesting(f.type) })) };
    // A document a statement builds holds its arrays as the statement's columns do.
    case "json": return t.content === undefined ? t : { ...t, content: { ...t.content, type: markNesting(t.content.type) } };
    default: return t;
  }
}

/** The analysis with each Postgres array typed at any depth in its result, as `allow_nested_arrays: true` asks: the format writes arrays flat, and Postgres lets a column's hold any number of dimensions. */
export function analysisWithNestedArrays<A extends { columns: readonly { type: Type }[] }>(analysis: A): A {
  return { ...analysis, columns: analysis.columns.map((c) => ({ ...c, type: markNesting(c.type) })) };
}
