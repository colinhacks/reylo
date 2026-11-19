// The type descriptor format: its JSON Schema, spec/typedefs.schema.json, is the source of truth, and
// spec/index.ts and the copy reylo.dev publishes are generated from it (spec/generate.ts), as
// spec/examples.json is from spec/examples.ts. tests/spec-examples.test.ts holds the examples to the engine.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { Ajv2020 } from "ajv/dist/2020.js";
import * as examples from "../spec/examples.ts";
import type { Example } from "../spec/examples.ts";

const SPEC = new URL("../spec/", import.meta.url);
const schema = JSON.parse(readFileSync(new URL("typedefs.schema.json", SPEC), "utf8"));

test("the TypeScript of the format and its published schema are what its JSON Schema gives, and examples.json what examples.ts gives", () => {
  const run = spawnSync(process.execPath, [fileURLToPath(new URL("generate.ts", SPEC)), "--check"], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
});

test("a CRLF checkout of the generated files is still current, as a Windows checkout may be", () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-typedefs-"));
  mkdirSync(join(root, "spec"));
  for (const name of ["generate.ts", "typedefs.schema.json", "index.ts", "examples.ts", "examples.json"]) writeFileSync(join(root, "spec", name), readFileSync(new URL(name, SPEC), "utf8").replace(/\r?\n/g, "\r\n"));
  const published = join(root, "site", "public", "typedefs");
  mkdirSync(published, { recursive: true });
  const file = schema.$id.split("/").pop();
  writeFileSync(join(published, file), readFileSync(new URL(`../site/public/typedefs/${file}`, SPEC), "utf8").replace(/\r?\n/g, "\r\n"));
  const run = spawnSync(process.execPath, [join(root, "spec", "generate.ts"), "--check"], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
});

test("a schema that would change a version the newest release tag publishes is refused, and one no tag carries is free to change", () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-typedefs-"));
  mkdirSync(join(root, "spec"));
  for (const name of ["generate.ts", "index.ts", "examples.ts", "examples.json"]) writeFileSync(join(root, "spec", name), readFileSync(new URL(name, SPEC), "utf8"));
  const published = join(root, "site", "public", "typedefs");
  mkdirSync(published, { recursive: true });
  // 0.1.0 as v0.8.0-beta.0 published it, the checkout's copy of that file being the same.
  const released = readFileSync(new URL("../site/public/typedefs/0.1.0.json", SPEC), "utf8");
  writeFileSync(join(published, "0.1.0.json"), released);
  // A repository of its own, so the user's git configuration (signing, hooks) takes no part.
  const env = { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1", GIT_AUTHOR_NAME: "test", GIT_AUTHOR_EMAIL: "test@example.com", GIT_COMMITTER_NAME: "test", GIT_COMMITTER_EMAIL: "test@example.com" };
  const git = (...args: string[]) => assert.equal(spawnSync("git", ["-C", root, ...args], { encoding: "utf8", env }).status, 0, args.join(" "));
  git("init", "-q");
  git("add", ".");
  git("commit", "-q", "-m", "release");
  git("tag", "v0.8.0-beta.0");
  const generate = (schemaText: string, ...args: string[]) => {
    writeFileSync(join(root, "spec", "typedefs.schema.json"), schemaText);
    return spawnSync(process.execPath, [join(root, "spec", "generate.ts"), ...args], { encoding: "utf8" });
  };
  const at = (version: string, text: string) => {
    const changed = { ...schema, $id: `https://reylo.dev/typedefs/${version}.json`, title: `Reylo Type Defs ${version}`, description: text };
    changed.$defs = { ...schema.$defs, StatementDescriptor: { ...schema.$defs.StatementDescriptor, properties: { ...schema.$defs.StatementDescriptor.properties, $schema: { const: changed.$id } } } };
    return `${JSON.stringify(changed, null, 2)}\n`;
  };
  // The schema changed under the released version, writing and checking alike: the published file stays.
  for (const args of [[], ["--check"]]) {
    const run = generate(at("0.1.0", "Changed."), ...args);
    assert.equal(run.status, 1);
    assert.match(run.stderr, /site\/public\/typedefs\/0\.1\.0\.json is published by v0\.8\.0-beta\.0 .* give spec\/typedefs\.schema\.json a new version/);
    assert.equal(readFileSync(join(published, "0.1.0.json"), "utf8"), released);
  }
  // The released version's own text, and a version no tag carries, are written as they are.
  assert.equal(generate(released).status, 0);
  for (const text of ["Unreleased.", "Unreleased, and changed."]) {
    const run = generate(at("9.9.9", text));
    assert.equal(run.status, 0, run.stderr);
    assert.equal(readFileSync(join(published, "9.9.9.json"), "utf8"), at("9.9.9", text));
  }
});

test("the format's JSON Schema is a valid 2020-12 schema, whose $id names its version's published file", () => {
  // Strict mode refuses an unknown keyword and a schema that contradicts itself.
  const ajv = new Ajv2020({ strict: true, allErrors: true });
  ajv.compile(schema);
  assert.match(schema.$id, /^https:\/\/reylo\.dev\/typedefs\/\d+\.\d+\.\d+\.json$/);
  assert.equal(schema.$defs.StatementDescriptor.properties.$schema.const, schema.$id);
});

test("the shape refuses an integer's exclusive bound, a UUID version outside 1 to 8, a union of one member, an enum of none and a count that is no whole number", () => {
  const ajv = new Ajv2020({ strict: false, allErrors: true });
  ajv.addSchema(schema);
  const type = ajv.getSchema(`${schema.$id}#/$defs/Type`)!;
  for (const t of [
    { kind: "integer", bits: 32, min: "1" },
    { kind: "float", exponent_bits: 11, mantissa_bits: 52, exclusive_min: "0" },
    { kind: "decimal", exclusive_max: "1e126" },
    { kind: "uuid", versions: [4, 7] },
    { kind: "enum", type: { kind: "text" }, values: ["a"] },
    { kind: "union", members: [{ kind: "text" }, { kind: "bytes" }] },
  ]) assert.ok(type(t), `${JSON.stringify(t)}: ${ajv.errorsText(type.errors)}`);
  for (const t of [
    { kind: "integer", bits: 32, exclusive_min: "0" },
    { kind: "integer", bits: 32, exclusive_max: "10" },
    { kind: "uuid", versions: [0] },
    { kind: "uuid", versions: [9] },
    { kind: "uuid", versions: [4.5] },
    { kind: "uuid", versions: [] },
    { kind: "uuid", versions: [7, 7] },
    { kind: "enum", type: { kind: "text" }, values: [] },
    { kind: "enum", type: { kind: "text" }, values: ["a", "a"] },
    { kind: "union", members: [{ kind: "text" }] },
    { kind: "intersection", members: [] },
    // A count is a whole number, as the Rust types read it: never a fraction, and never below zero where it is unsigned.
    { kind: "integer", bits: 32.5 },
    { kind: "integer", bits: -8 },
    { kind: "float", exponent_bits: 8, mantissa_bits: 23.5 },
    { kind: "decimal", precision: 10, min_scale: 0.5 },
    { kind: "text", size: { max: 2.5 } },
    { kind: "text", code_points: [[1, 1114111.5]] },
    { kind: "geometry", srid: 4326.5 },
  ]) assert.ok(!type(t), JSON.stringify(t));
});

test("every example in spec/examples.ts is a descriptor of the format, whole or in part", () => {
  const ajv = new Ajv2020({ strict: false, allErrors: true });
  ajv.addSchema(schema);
  // A part of a descriptor: the same schema, with nothing required of the descriptor itself.
  const partial = structuredClone(schema);
  partial.$id = schema.$id.replace(/\.json$/, "-part.json");
  partial.$defs.StatementDescriptor.required = [];
  ajv.addSchema(partial);
  const whole = ajv.getSchema(`${schema.$id}#/$defs/StatementDescriptor`)!;
  const part = ajv.getSchema(`${partial.$id}#/$defs/StatementDescriptor`)!;
  const all = Object.entries(examples);
  assert.ok(all.length > 0);
  for (const [name, example] of all) {
    const { descriptor } = example as Example;
    const validate = descriptor.$schema === undefined ? part : whole;
    assert.ok(validate(descriptor), `${name}: ${ajv.errorsText(validate.errors)}`);
    if (descriptor.dialect) assert.equal(descriptor.dialect.name, (example as Example).database, name);
  }
});
