// The type format's page (content/docs/typedefs.mdx) and the analyze page (content/docs/analyze.mdx):
// each `<TypeDef>` card (lib/typedefs.ts) becomes the docs' card of labelled rows (`TypeDefCard`,
// components/mdx.tsx), Types, Holds and Databases, its example types first held to the format's
// JSON Schema, so that the build fails on one the format refuses; each `<Analyzed>` card the command
// and the output `nub run capture-typedefs` captured for it; `<TypeDefsVersion />` the badge under the
// title, the format's stage and version from its JSON Schema, and the button to that schema; and
// `<TypeDefsSpec />` its TypeScript, spec/index.ts, without its prose, so that the page restates
// none of them by hand.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { Ajv2020 } from 'ajv/dist/2020.js';
import type { MdNode } from './remark-mapper.ts';
import { analyzedCards, CAPTURES, DATABASE_NAMES, DATABASES, keyOf, readCaptures, typeCards, values, type AnalyzedCard, type TypeCard } from './typedefs.ts';

const SPEC = path.resolve(import.meta.dirname, '..', '..', 'spec');

const code = (lang: string, value: string, meta: string | null = null): MdNode => ({ type: 'code', lang, meta, value });
const jsx = (name: string, attributes: [string, string | null][], children: MdNode[]): MdNode => ({
  type: 'mdxJsxFlowElement',
  name,
  attributes: attributes.map(([name, value]) => ({ type: 'mdxJsxAttribute', name, value })),
  children,
});
const row = (label: string, children: MdNode[]) => jsx('InferredRow', [['label', label]], children);
const span = (className: string, value: string): MdNode => ({ type: 'mdxJsxTextElement', name: 'span', attributes: [{ type: 'mdxJsxAttribute', name: 'className', value: className }], children: [{ type: 'text', value }] });
const link = (url: string, className: string, value: string) => ({ type: 'mdxJsxTextElement', name: 'a', attributes: [{ type: 'mdxJsxAttribute', name: 'href', value: url }, { type: 'mdxJsxAttribute', name: 'className', value: className }], children: [{ type: 'text', value }] }) as MdNode;

/** The format's JSON Schema, and a validator of one type against it. */
function typeValidator() {
  const schema = JSON.parse(readFileSync(path.join(SPEC, 'typedefs.schema.json'), 'utf8'));
  const ajv = new Ajv2020({ strict: false, allErrors: true });
  ajv.addSchema(schema);
  const validate = ajv.getSchema(`${schema.$id}#/$defs/Type`)!;
  return (value: unknown) => (validate(value) ? undefined : ajv.errorsText(validate.errors));
}

/** One database's types of a kind: each a type's name as code, a phrase ("a composite type") as text, or an em dash where it has none. */
function typesRow(card: TypeCard): MdNode {
  const line = (database: (typeof DATABASES)[number]): MdNode => {
    const types = card.types[database] ?? [];
    const shown = types.length ? types.map((t): MdNode => (/^(?:an?|every|any) /.test(t) ? span('typedef-phrase', t) : { type: 'inlineCode', value: t })) : [span('typedef-none', '—')];
    return { type: 'paragraph', children: [span('typedef-database', DATABASE_NAMES[database]), jsx('span', [['className', 'typedef-names']], shown)] } as MdNode;
  };
  return jsx('div', [['className', 'typedef-types']], DATABASES.map(line));
}

function renderType(card: TypeCard, valid: (value: unknown) => string | undefined, where: string) {
  const found = values(card.examples.value ?? '');
  if (!found.length) throw new Error(`${where}: a <TypeDef> card's jsonc block holds no type`);
  found.forEach((value, i) => {
    const why = valid(value);
    if (why) throw new Error(`${where}: example ${i + 1} of this <TypeDef> card is no type of the format: ${why}\n${JSON.stringify(value)}`);
  });
  const rows: MdNode[] = [row('Types', [card.examples])];
  if (card.holds.length) {
    const held: MdNode = { type: 'paragraph', children: card.holds.map((value) => ({ type: 'inlineCode', value })) };
    rows.push(row('Holds', [jsx('div', [['className', 'typedef-values']], [held])]));
  }
  rows.push(row('Databases', [typesRow(card)]));
  card.node.name = 'TypeDefCard';
  card.node.attributes = [];
  card.node.children = rows;
}

function renderAnalyzed(card: AnalyzedCard, output: string) {
  const command = `npx reylo analyze ${card.file}${card.format === 'json' ? ' --format json' : ''}`;
  let shown = output.trimEnd();
  if (card.lines !== undefined && shown.split('\n').length > card.lines) shown = `${shown.split('\n').slice(0, card.lines).join('\n')}\n# …`;
  card.node.name = 'div';
  card.node.attributes = [{ type: 'mdxJsxAttribute', name: 'className', value: 'typedef-analyzed' }];
  card.node.children = [code('sh', command), code(card.format, shown, 'title="Output"')];
}

/** The format's version, from its JSON Schema's `$id`, and the path reylo.dev serves that schema at. */
function version(): { version: string; href: string } {
  const id = String(JSON.parse(readFileSync(path.join(SPEC, 'typedefs.schema.json'), 'utf8')).$id);
  const m = /^https:\/\/reylo\.dev(\/typedefs\/(\d+\.\d+\.\d+)\.json)$/.exec(id);
  if (!m) throw new Error(`spec/typedefs.schema.json: its $id, ${id}, is no https://reylo.dev/typedefs/<version>.json`);
  return { version: m[2]!, href: m[1]! };
}

/** spec/index.ts without its doc comments and its banner: every type and field, as TypeScript. */
function spec(): string {
  return readFileSync(path.join(SPEC, 'index.ts'), 'utf8')
    .replace(/^\/\/.*\n/gm, '')
    .replace(/^[ \t]*\/\*\*[\s\S]*?\*\/\n/gm, '')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

export function remarkTypedefs() {
  return (tree: MdNode, file: { path?: string; data?: Record<string, unknown> }) => {
    const page = file.path ? path.relative(process.cwd(), file.path) : 'the page';
    const replace = (name: string, nodes: (node: MdNode) => MdNode[]) => {
      const walk = (node: MdNode) => {
        const children = node.children ?? [];
        for (let i = 0; i < children.length; i++) {
          const child = children[i]!;
          if (child.type === 'mdxJsxFlowElement' && child.name === name) children.splice(i, 1, ...nodes(child));
          else walk(child);
        }
      };
      walk(tree);
    };
    const compiler = file.data?._compiler as { addDependency(file: string): void } | undefined;
    compiler?.addDependency(path.join(SPEC, 'index.ts'));
    compiler?.addDependency(path.join(SPEC, 'typedefs.schema.json'));
    const types = typeCards(tree);
    if (types.length) {
      const valid = typeValidator();
      for (const card of types) renderType(card, valid, `${page}:${card.line}`);
    }
    const analyzed = analyzedCards(tree);
    if (analyzed.length) {
      compiler?.addDependency(path.join(process.cwd(), 'lib', CAPTURES));
      const captures = readCaptures();
      for (const card of analyzed) {
        const captured = captures[keyOf(card)];
        if (captured === undefined) throw new Error(`${page}:${card.line}: lib/${CAPTURES} holds no output for this <Analyzed> card; run \`nub run capture-typedefs\` in site/ (with the scratch databases running), then build again.`);
        renderAnalyzed(card, captured[card.format]);
      }
    }
    replace('TypeDefsVersion', () => {
      const { version: at, href } = version();
      const [major, minor] = at.split('.');
      const badge = jsx('span', [['className', 'typedef-badge']], [span('typedef-badge-stage', 'preview'), span('typedef-badge-version', `v${major}.${minor}`)]);
      return [jsx('p', [['className', 'typedef-version not-prose']], [badge, link(href, 'typedef-schema-button', 'View JSON Schema')])];
    });
    replace('TypeDefsSpec', () => {
      const intro: MdNode = { type: 'paragraph', children: [{ type: 'text', value: "Every type and field, as TypeScript, generated from the format's JSON Schema:" }] };
      return [intro, jsx('div', [['className', 'typedef-spec']], [code('ts', spec(), 'title="spec/index.ts"')])];
    });
  };
}
