// The type descriptor format: its JSON Schema, spec/typedefs.schema.json, is the source of truth, and
// spec/index.ts and the copy reylo.dev publishes are generated from it (spec/generate.ts).
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { Ajv2020 } from "ajv/dist/2020.js";
import * as examples from "../spec/examples.ts";

const SPEC = new URL("../spec/", import.meta.url);
const schema = JSON.parse(readFileSync(new URL("typedefs.schema.json", SPEC), "utf8"));

test("the TypeScript of the format and its published schema are what its JSON Schema gives", () => {
  const run = spawnSync(process.execPath, [fileURLToPath(new URL("generate.ts", SPEC)), "--check"], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
});

test("a CRLF checkout of the generated files is still current, as a Windows checkout may be", () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-typedefs-"));
  mkdirSync(join(root, "spec"));
  for (const name of ["generate.ts", "typedefs.schema.json", "index.ts"]) writeFileSync(join(root, "spec", name), readFileSync(new URL(name, SPEC), "utf8").replaceAll("\n", "\r\n"));
  const published = join(root, "site", "public", "typedefs");
  mkdirSync(published, { recursive: true });
  const file = schema.$id.split("/").pop();
  writeFileSync(join(published, file), readFileSync(new URL(`../site/public/typedefs/${file}`, SPEC), "utf8").replaceAll("\n", "\r\n"));
  const run = spawnSync(process.execPath, [join(root, "spec", "generate.ts"), "--check"], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
});

test("the format's JSON Schema is a valid 2020-12 schema, whose $id names its version's published file", () => {
  // Strict mode refuses an unknown keyword and a schema that contradicts itself.
  const ajv = new Ajv2020({ strict: true, allErrors: true });
  ajv.compile(schema);
  assert.match(schema.$id, /^https:\/\/reylo\.dev\/typedefs\/\d+\.\d+\.\d+\.json$/);
  assert.equal(schema.$defs.StatementDescriptor.properties.$schema.const, schema.$id);
});

test("every example in spec/examples.ts is a value of the type its declaration names", () => {
  const ajv = new Ajv2020({ strict: false, allErrors: true });
  ajv.addSchema(schema);
  const declared = [...readFileSync(new URL("examples.ts", SPEC), "utf8").matchAll(/^export const (\w+): (\w+)(\[\])? =/gm)];
  assert.ok(declared.length > 0);
  for (const [, name, definition, list] of declared) {
    const validate = ajv.getSchema(`${schema.$id}#/$defs/${definition}`);
    assert.ok(validate, `${definition} is no definition of the schema`);
    const value = (examples as Record<string, unknown>)[name!];
    for (const [i, v] of (list ? (value as unknown[]) : [value]).entries()) {
      assert.ok(validate(v), `${name}${list ? `[${i}]` : ""}: ${ajv.errorsText(validate.errors)}`);
    }
  }
});
