// `nub run typedefs-schema` in site/, which `build` runs with `--check`: the JSON Schema of the type
// descriptor format, generated from spec/type-descriptor.ts, at the URL its `$schema` literal names,
// `https://reylo.dev/typedefs/<version>`. It is written to public/typedefs/<version>.json, which
// vercel.json serves at that URL. Each version's file is committed and kept once its version is
// released, so a descriptor written in an older version still validates against its own.
//
// The schema holds a document to its version exactly: no key the format does not define, but at a
// descriptor's top level, where a tool may write keys of its own beside it, as `reylo analyze`
// writes the file a statement was read from. Every example in spec/examples.ts is checked against
// it. With `--check`, nothing is written, and a file that differs from what the spec generates fails.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { Ajv } from 'ajv';
import { createGenerator } from 'ts-json-schema-generator';

const site = path.resolve(import.meta.dirname, '..');
const spec = path.join(path.dirname(site), 'spec', 'type-descriptor.ts');
const check = process.argv.includes('--check');

const literal = /\$schema: "(https:\/\/reylo\.dev\/typedefs\/(\d+\.\d+\.\d+))";/.exec(readFileSync(spec, 'utf8'));
if (!literal) throw new Error(`${spec} names no $schema of the form https://reylo.dev/typedefs/<version>`);
const [, url, version] = literal;

const generated = createGenerator({ path: spec, type: 'StatementDescriptor', expose: 'export', jsDoc: 'basic', additionalProperties: false, skipTypeCheck: true }).createSchema('StatementDescriptor');
const definitions = generated.definitions as Record<string, Record<string, unknown>>;
delete definitions.StatementDescriptor!.additionalProperties;
const schema = { $schema: generated.$schema, $id: url, title: `Reylo Type Defs ${version}`, description: 'What introspecting one SQL statement returns, and the type of each parameter and column in it.', $ref: generated.$ref, definitions };
const text = `${JSON.stringify(schema, null, 2)}\n`;

// Every example the spec gives validates as the type its declaration names.
const ajv = new Ajv({ strict: false, allErrors: true });
ajv.addSchema(schema, url);
const examplesFile = path.join(path.dirname(spec), 'examples.ts');
const examples: Record<string, unknown> = await import(examplesFile);
const failures: string[] = [];
for (const [, name, definition, list] of readFileSync(examplesFile, 'utf8').matchAll(/^export const (\w+): (\w+)(\[\])? =/gm)) {
  const validate = ajv.getSchema(`${url}#/definitions/${definition}`);
  if (!validate) throw new Error(`spec/examples.ts declares ${name} a ${definition}, which the schema does not define`);
  const values = list ? (examples[name!] as unknown[]) : [examples[name!]];
  values.forEach((v, i) => {
    if (!validate(v)) failures.push(`${name}${list ? `[${i}]` : ''} is no ${definition}: ${ajv.errorsText(validate.errors)}`);
  });
}
if (failures.length) throw new Error(`spec/examples.ts does not validate against the schema it generates:\n${failures.join('\n')}`);

const out = path.join(site, 'public', 'typedefs', `${version}.json`);
if (check) {
  if (!existsSync(out) || readFileSync(out, 'utf8') !== text) {
    console.error(`${path.relative(site, out)} is not the schema spec/type-descriptor.ts generates: run \`nub run typedefs-schema\` in site/.`);
    process.exit(1);
  }
} else {
  mkdirSync(path.dirname(out), { recursive: true });
  writeFileSync(out, text);
  console.log(`wrote ${path.relative(site, out)}`);
}
