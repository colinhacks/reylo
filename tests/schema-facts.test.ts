import { test } from "node:test";
import assert from "node:assert/strict";
import { querySchemas, typeSchema } from "../src/schema.ts";
import type { EngineType } from "../src/engine-type.ts";

const int4 = (facts: object): EngineType => ({ kind: "integer", bits: 32, name: "int4", ...facts }) as EngineType;
const varchar = (facts: object): EngineType => ({ kind: "text", name: "varchar", ...facts }) as EngineType;
const output = (t: EngineType, inJson = false) => typeSchema(t, "postgres", false, inJson);

test("a text's size in code points and a LIKE pattern are written, and nothing else of a string", () => {
  assert.deepEqual(output(varchar({ size: { max: 20 } })), { type: "string", maxLength: 20 });
  assert.deepEqual(output(varchar({ size: { exact: 5 } })), { type: "string", minLength: 5, maxLength: 5 });
  assert.deepEqual(typeSchema(varchar({ size: { exact: 5 } }), "postgres", true), { type: "string", maxLength: 5 });
  assert.deepEqual(output(varchar({ size: { unit: "byte", encoding: "UTF-8", max: 63 } })), { type: "string" });
  assert.deepEqual(output(varchar({ pattern: { dialect: "postgres", source: "^a" } })), { type: "string" });
  assert.deepEqual(output(varchar({ pattern: { dialect: "ilike", source: "a%" } })), { type: "string" });
});

test("a LIKE pattern reads as the regular expression matching the same whole values", () => {
  const regex = (source: string) => new RegExp((output(varchar({ pattern: { dialect: "like", source } })) as { pattern: string }).pattern, "u");
  assert.equal(regex("a%").source, "^a[\\s\\S]*$");
  assert.ok(regex("%@%").test("a\n@b"));
  assert.ok(!regex("%@%").test("ab"));
  assert.ok(regex("a_c").test("a🙂c"));
  assert.ok(!regex("a_c").test("ac"));
  assert.ok(regex("50\\%").test("50%"));
  assert.ok(!regex("50\\%").test("500"));
  assert.ok(regex("a.c").test("a.c"));
  assert.ok(!regex("a.c").test("abc"));
  assert.ok(regex("(x)[y]").test("(x)[y]"));
});

test("a number's facts are written where its form here is a JSON number and they fit one exactly", () => {
  assert.deepEqual(output(int4({ min: "0", max: "150" })), { type: "integer", format: "int32", minimum: 0, maximum: 150 });
  assert.deepEqual(output(int4({ multiple_of: "5" })), { type: "integer", format: "int32", multipleOf: 5 });
  assert.deepEqual(output(int4({ enum: ["1", "2"] })), { type: "integer", format: "int32", enum: [1, 2] });
  const int8 = { kind: "integer", bits: 64, name: "int8" };
  assert.deepEqual(output({ ...int8, min: "1", max: "9223372036854775806" } as EngineType), { type: "integer", format: "int64", minimum: 1 });
  const float8 = { kind: "float", exponent_bits: 11, mantissa_bits: 52, name: "float8" };
  assert.deepEqual(output({ ...float8, exclusive_min: "0", max: "1.5" } as EngineType), { type: "number", format: "double", exclusiveMinimum: 0, maximum: 1.5 });
  // A decimal `.run()` gives as its text takes no bounds, and inside a document it is a number.
  const numeric = { kind: "decimal", name: "numeric", min: "0", exclusive_max: "1e131072" } as EngineType;
  assert.deepEqual(output(numeric), { type: "string", format: "decimal" });
  assert.equal((output(numeric, true) as { minimum?: number }).minimum, 0);
  assert.equal((output(numeric, true) as { exclusiveMaximum?: number }).exclusiveMaximum, undefined);
});

test("an array's size is written on an array", () => {
  const vector = { kind: "array", name: "vector", element: { type: { kind: "float", exponent_bits: 8, mantissa_bits: 23 } }, size: { exact: 3 } } as EngineType;
  const schema = output(vector) as { minItems?: number; maxItems?: number };
  assert.deepEqual([schema.minItems, schema.maxItems], [3, 3]);
});

test("a nullable position keeps its facts beside null, and comments describe what they are on", () => {
  const schemas = querySchemas(
    {
      comment: "One user",
      params: [
        { name: "age", type: int4({ min: "0" }), nullable: true, comment: "Their age" },
        { name: "doc", type: { kind: "json", name: "jsonb" } as EngineType, schema: { type: "object", description: "As written" }, comment: "Column comment" },
      ],
      columns: [{ name: "name", type: varchar({ size: { max: 20 } }), nullable: true, comment: "Shown on the profile page" }],
      cardinality: "one",
    },
    "postgres",
  ) as { input: { description?: string; properties: Record<string, unknown> }; output: { description?: string; properties: Record<string, unknown> } };
  assert.equal(schemas.input.description, "One user");
  assert.equal(schemas.output.description, "One user");
  assert.deepEqual(schemas.input.properties.age, { description: "Their age", anyOf: [{ type: "integer", format: "int32", minimum: 0 }, { type: "null" }] });
  assert.deepEqual(schemas.input.properties.doc, { description: "As written", type: "object" });
  assert.deepEqual(schemas.output.properties.name, { description: "Shown on the profile page", anyOf: [{ type: "string", maxLength: 20 }, { type: "null" }] });
});
