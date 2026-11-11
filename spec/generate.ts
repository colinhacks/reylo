// `nub run typedefs` at the root: the TypeScript of the type descriptor format, spec/index.ts,
// generated from its JSON Schema, spec/typedefs.schema.json, which is the source of truth; the
// schema as reylo.dev publishes it, site/public/typedefs/<version>.json, the version its `$id` names;
// and spec/examples.json, the examples of spec/examples.ts as JSON, which the engine's own tests read
// (engine/core/src/types.rs). Each version's published file is kept once written, so a descriptor of
// an older version still validates against its own. With `--check`, nothing is written, and a file
// that differs from what its source gives fails.
//
// The schema keeps to a small idiom, which this reads and refuses to go beyond: each definition is an
// object (whose `allOf` names the definitions it extends), a union of definitions (`anyOf`), or a list
// of strings (`enum`); a property is a `$ref`, a `const`, an `enum`, a string, a number, a boolean,
// an array or an object of its own.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

type Schema = { [keyword: string]: unknown };

const root = path.resolve(import.meta.dirname, "..");
const source = path.join(root, "spec", "typedefs.schema.json");
const check = process.argv.includes("--check");
const schema = JSON.parse(readFileSync(source, "utf8")) as Schema;
const defs = schema.$defs as Record<string, Schema>;
const version = /^https:\/\/reylo\.dev\/typedefs\/(\d+\.\d+\.\d+)\.json$/.exec(String(schema.$id))?.[1];
if (!version) throw new Error(`${source}: its $id is no https://reylo.dev/typedefs/<version>.json`);

function comment(text: string | undefined, indent: string): string {
  if (!text) return "";
  const lines = text.split("\n");
  if (lines.length === 1) return `${indent}/** ${text} */\n`;
  return `${indent}/**\n${lines.map((l) => (l ? `${indent} * ${l}` : `${indent} *`)).join("\n")}\n${indent} */\n`;
}

function refName(s: Schema): string {
  const m = /^#\/\$defs\/(\w+)$/.exec(String(s.$ref));
  if (!m) throw new Error(`a $ref outside $defs: ${JSON.stringify(s)}`);
  return m[1]!;
}

function type(s: Schema, indent: string): string {
  if (s.$ref !== undefined) return refName(s);
  if ("const" in s) return JSON.stringify(s.const);
  if (Array.isArray(s.enum)) return s.enum.map((v) => JSON.stringify(v)).join(" | ");
  if (Array.isArray(s.anyOf)) return (s.anyOf as Schema[]).map((m) => type(m, indent)).join(" | ");
  switch (s.type) {
    case "string":
      return "string";
    case "number":
    case "integer":
      return "number";
    case "boolean":
      return "boolean";
    case "array": {
      const items = type(s.items as Schema, indent);
      return /[| ]/.test(items) ? `(${items})[]` : `${items}[]`;
    }
    case "object":
      return `{ ${members(s, indent).map((m) => m.trim()).join(" ")} }`;
  }
  if (Object.keys(s).filter((k) => k !== "description").length === 0) return "unknown";
  throw new Error(`a schema outside the idiom: ${JSON.stringify(s)}`);
}

/** An object's members, one per line, each after its description. */
function members(s: Schema, indent: string): string[] {
  const required = new Set((s.required as string[] | undefined) ?? []);
  const out: string[] = [];
  for (const [name, prop] of Object.entries((s.properties as Record<string, Schema> | undefined) ?? {})) {
    const key = /^[A-Za-z_$][\w$]*$/.test(name) ? name : JSON.stringify(name);
    out.push(`${comment(prop.description as string | undefined, indent)}${indent}${key}${required.has(name) ? "" : "?"}: ${type(prop, indent)};`);
  }
  if (s.additionalProperties !== undefined && s.additionalProperties !== false) out.push(`${indent}[keyword: string]: ${type(s.additionalProperties as Schema, indent)};`);
  return out;
}

let ts = `// Generated from typedefs.schema.json, the format's source of truth, by \`nub run typedefs\`: edit the\n// schema, not this file. The rules every descriptor follows are in README.md beside it.\n\n`;
for (const [name, def] of Object.entries(defs)) {
  ts += comment(def.description as string | undefined, "");
  if (def.type === "object") {
    const bases = ((def.allOf as Schema[] | undefined) ?? []).map(refName);
    ts += `export interface ${name}${bases.length ? ` extends ${bases.join(", ")}` : ""} {\n${members(def, "  ").join("\n")}\n}\n\n`;
  } else {
    const union = type(def, "");
    // A long union takes a line per member.
    ts += union.length > 80 && Array.isArray(def.anyOf) ? `export type ${name} =\n${(def.anyOf as Schema[]).map((m) => `  | ${type(m, "  ")}`).join("\n")};\n\n` : `export type ${name} = ${union};\n\n`;
  }
}
ts = ts.trimEnd() + "\n";

// The examples, by name, as one JSON document.
const examples = `${JSON.stringify({ ...(await import("./examples.ts")) }, null, 2)}\n`;

// Each file written, its text, and the file it is written from.
const outputs: [string, string, string][] = [
  [path.join(root, "spec", "index.ts"), ts, "spec/typedefs.schema.json"],
  [path.join(root, "site", "public", "typedefs", `${version}.json`), readFileSync(source, "utf8").replaceAll("\r\n", "\n"), "spec/typedefs.schema.json"],
  [path.join(root, "spec", "examples.json"), examples, "spec/examples.ts"],
];
// A checkout on Windows may hold any of these with CRLF line endings, which change nothing they say.
const lf = (text: string) => text.replaceAll("\r\n", "\n");
const stale = outputs.filter(([file, text]) => !existsSync(file) || lf(readFileSync(file, "utf8")) !== lf(text));
if (check) {
  for (const [file, , from] of stale) console.error(`${path.relative(root, file)} is not what ${from} gives: run \`nub run typedefs\`.`);
  if (stale.length) process.exit(1);
} else {
  for (const [file, text] of stale) {
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, text);
    console.log(`wrote ${path.relative(root, file)}`);
  }
}
