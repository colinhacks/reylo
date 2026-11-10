import { test } from "node:test";
import assert from "node:assert/strict";
import { validate, withDefaults } from "../src/json-schema.ts";

// The run time's JSON Schema 2020-12 validator (src/json-schema.ts). The official JSON Schema Test
// Suite runs over it in lab/json-schema-suite; these are the rules Reylo adds or decides.

test("a default fills a key the value leaves out, never a null one, and never inside a union member", () => {
  const schema = {
    type: "object",
    properties: { done: { type: "boolean", default: false }, tags: { type: "array", default: [] }, note: { type: ["string", "null"], default: "x" } },
    anyOf: [{ properties: { kind: { default: "a" } } }],
  };
  const value = { note: null };
  assert.deepEqual(withDefaults(schema, value), { done: false, tags: [], note: null });
  // The caller's value is left as it was, and each default is a copy.
  assert.deepEqual(value, { note: null });
  const a = withDefaults(schema, {}) as { tags: unknown[] };
  a.tags.push(1);
  assert.deepEqual((withDefaults(schema, {}) as { tags: unknown[] }).tags, []);
});

test("defaults follow the value into nested objects, arrays, allOf, and the branch `if` picks", () => {
  const schema = {
    type: "array",
    items: { type: "object", properties: { n: { default: 1 } }, if: { required: ["big"] }, then: { properties: { size: { default: "L" } } }, else: { properties: { size: { default: "S" } } } },
    allOf: [{ prefixItems: [{ default: { first: true } }] }],
  };
  assert.deepEqual(withDefaults(schema, [{ big: true }, {}]), [{ big: true, n: 1, size: "L" }, { n: 1, size: "S" }]);
  assert.deepEqual(withDefaults({ prefixItems: [{ default: 1 }, { default: 2 }] }, []), [1, 2]);
});

test("a failure names its place as a JSON Pointer", () => {
  const schema = { type: "object", properties: { "a/b": { type: "array", items: { type: "integer", minimum: 0 } } }, unevaluatedProperties: false };
  assert.deepEqual(validate(schema, { "a/b": [1, -1] }), { path: "/a~1b/1", message: "must be at least 0" });
  assert.deepEqual(validate(schema, { c: 1 }), { path: "/c", message: "is a key the schema does not allow" });
  assert.equal(validate(schema, { "a/b": [] }), undefined);
  // A format is asserted.
  assert.deepEqual(validate({ type: "string", format: "date-time" }, "2026-13-01T00:00:00Z"), { path: "", message: "must be a date-time" });
  assert.equal(validate({ type: "string", format: "date-time" }, "2026-10-20T10:00:00.5+02:00"), undefined);
});

// JSON.stringify writes the value the statement stores: it leaves out a key whose value is
// undefined and writes such an item of an array as null.
test("a value is checked as the JSON it is stored as: a key whose value is undefined is a key left out, and such an item is null", () => {
  const click = { type: "object", properties: { x: { type: "integer" }, button: { type: "string", enum: ["left", "right"], default: "left" } }, required: ["x"], additionalProperties: false };
  // The declared type admits `button?: …`, which a caller may spell as undefined.
  assert.equal(validate(click, { x: 1, button: undefined }), undefined);
  assert.equal(validate(click, { x: 1, extra: undefined }), undefined);
  assert.equal(validate({ type: "object", properties: { x: { type: "integer" } }, unevaluatedProperties: false }, { x: 1, extra: undefined }), undefined);
  assert.match(validate({ type: "object", properties: { x: { type: "integer" } }, unevaluatedProperties: false }, { x: 1, extra: null })!.message, /is a key the schema does not allow/);
  assert.deepEqual(withDefaults(click, { x: 1, button: undefined }), { x: 1, button: "left" });
  // What JSON leaves out is not there for `required`, `minProperties` or `dependentRequired` either.
  assert.match(validate(click, { x: undefined })!.message, /must have the key "x"/);
  assert.match(validate({ type: "object", minProperties: 1 }, { a: undefined })!.message, /at least 1 key/);
  assert.equal(validate({ type: "object", maxProperties: 1 }, { a: 1, b: undefined }), undefined);
  assert.match(validate({ dependentRequired: { a: ["b"] } }, { a: 1, b: undefined })!.message, /must have the key "b" beside "a"/);
  assert.equal(validate({ dependentRequired: { a: ["b"] } }, { a: undefined }), undefined);
  // An array's undefined item, and a hole, are stored as null.
  assert.match(validate({ type: "array", items: { type: "integer" } }, [1, undefined])!.message, /must be integer, got null/);
  assert.match(validate({ type: "array", items: { type: "integer" } }, [1, , 3])!.message, /must be integer, got null/);
  assert.equal(validate({ type: "array", items: { type: ["integer", "null"] } }, [1, undefined]), undefined);
  assert.match(validate({ uniqueItems: true }, [null, undefined])!.message, /no two equal items/);
  assert.match(validate({ uniqueItems: true }, [{ a: 1 }, { a: 1, b: undefined }])!.message, /no two equal items/);
  assert.equal(validate({ const: { a: 1 } }, { a: 1, b: undefined }), undefined);
});

test("multipleOf holds a value to a real multiple, within the division's rounding error alone", () => {
  for (const [value, of] of [[4.35, 0.01], [0.3, 0.1], [19.99, 0.01], [12391239123, 1e-8], [0, 5], [-15, 5], [1e308, 0.5], [0.075, 0.025]] as const) assert.equal(validate({ multipleOf: of }, value), undefined, `${value} / ${of}`);
  for (const [value, of] of [[4e-9, 5], [1.0000000001, 1], [0.0100000000001, 0.01], [7, 5], [0.0075, 0.01]] as const) assert.match(validate({ multipleOf: of }, value)?.message ?? "", /must be a multiple of/, `${value} / ${of}`);
});

test("a pattern Unicode mode refuses and the plain mode reads is read plainly, and one neither reads still throws", () => {
  // The character class of a widely copied email pattern: `\.` after a range is an error under `u`.
  const email = { type: "string", pattern: "^[\\w-\\.]+@([\\w-]+\\.)+[\\w-]{2,4}$" };
  assert.equal(validate(email, "a.b@example.co"), undefined);
  assert.match(validate(email, "not an email")!.message, /must match the pattern/);
  assert.equal(validate({ patternProperties: { "^x\\-": { type: "integer" } } }, { "x-a": 1 }), undefined);
  // Unicode mode where it reads the pattern: an astral character is one character.
  assert.equal(validate({ type: "string", pattern: "^.$" }, "😀"), undefined);
  assert.throws(() => validate({ type: "string", pattern: "(" }, "a"), SyntaxError);
});
