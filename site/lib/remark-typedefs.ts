// The type descriptor format's page (content/docs/typedefs.mdx): each `<TypeDef>` card (lib/typedefs.ts)
// becomes the docs' card of labelled rows (`TypeDefCard`, components/mdx.tsx), Types, Schema, Query,
// Descriptor and Holds; `<TypeDefPart of="id" show="sql" />` a part of a card's descriptor;
// `<TypeDefsVersion />` the chips under the title, the format's version from its JSON Schema, its
// stage and a link to that schema; and `<TypeDefsSpec />` its
// TypeScript, spec/index.ts, without its prose, so that the page restates none of them by hand.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import type { MdNode } from './remark-mapper.ts';
import { annotate, cards, CAPTURES, keyOf, readCaptures, shown, TYPEDEF, type Card } from './typedefs.ts';
import * as examples from '../../spec/examples.ts';

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
const link = (url: string, value: string) => ({ type: 'link', url, children: [{ type: 'text', value }] }) as MdNode;

/** The descriptor of an example of spec/examples.ts, by its name. */
function example(name: string, where: string): unknown {
  const found = (examples as unknown as Record<string, examples.Example | undefined>)[name];
  if (!found?.descriptor) throw new Error(`${where}: example="${name}" names no example spec/examples.ts exports`);
  return found.descriptor;
}

/** A database's types of a kind: each a type's name as code, or a phrase ("a composite type") as text. */
const typeLine = ([database, types]: [string, string[]]): MdNode => ({
  type: 'paragraph',
  children: [span('typedef-database', database), ...types.map((t): MdNode => (/^(?:an?|every) /.test(t) ? span('typedef-phrase', t) : { type: 'inlineCode', value: t }))],
});

function render(card: Card, descriptor: unknown) {
  const rows: MdNode[] = [];
  const databases = Object.entries(card.types);
  if (databases.length) rows.push(row('Types', [jsx('div', [['className', 'typedef-types']], databases.map(typeLine))]));
  const schema = (card.node.children ?? []).find((c) => c.type === 'code' && /(?:^|\s)schema(?:\s|$)/.test(c.meta ?? ''));
  const query = (card.node.children ?? []).find((c) => c.type === 'code' && c !== schema);
  if (schema) rows.push(row('Schema', [code('sql', schema.value!)]));
  if (query) rows.push(row('Query', [code('sql', query.value!)]));
  rows.push(row('Descriptor', [code('jsonc', annotate(shown(descriptor, card.show), card.notes))]));
  if (card.holds.length) {
    const values: MdNode = { type: 'paragraph', children: card.holds.map((value) => ({ type: 'inlineCode', value })) };
    rows.push(row('Holds', [jsx('div', [['className', 'typedef-values']], [values])]));
  }
  card.node.name = 'TypeDefCard';
  card.node.attributes = [];
  card.node.children = rows;
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
    const found = cards(tree);
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
    const parts = new Map<string, unknown>();
    if (found.length) {
      compiler?.addDependency(path.join(process.cwd(), 'lib', CAPTURES));
      compiler?.addDependency(path.join(SPEC, 'examples.ts'));
      const captures = found.some((c) => c.statement) ? readCaptures() : {};
      for (const card of found) {
        const where = `${page}:${card.line}`;
        const descriptor = card.statement ? captures[keyOf(card.statement)]?.descriptor : example(card.example!, where);
        if (descriptor === undefined) throw new Error(`${where}: lib/${CAPTURES} holds no descriptor for this <${TYPEDEF}> card; run \`nub run capture-typedefs\` in site/ (with the scratch databases running), then build again.`);
        if (card.id) parts.set(card.id, descriptor);
        render(card, descriptor);
      }
    }
    compiler?.addDependency(path.join(SPEC, 'index.ts'));
    compiler?.addDependency(path.join(SPEC, 'typedefs.schema.json'));
    const attribute = (node: MdNode, name: string) => String(node.attributes?.find((a) => a.type === 'mdxJsxAttribute' && a.name === name)?.value);
    replace('TypeDefPart', (node) => {
      const descriptor = parts.get(attribute(node, 'of'));
      if (descriptor === undefined) throw new Error(`${page}:${node.position?.start.line ?? '?'}: <TypeDefPart of="${attribute(node, 'of')}"> names no card's id`);
      return [code('jsonc', annotate(shown(descriptor, attribute(node, 'show')), {}))];
    });
    replace('TypeDefsVersion', () => {
      const { version: at, href } = version();
      return [jsx('p', [['className', 'typedef-version not-prose']], [span('', `v${at}`), span('', 'Preview'), link(href, 'JSON Schema')])];
    });
    replace('TypeDefsSpec', () => {
      const intro: MdNode = { type: 'paragraph', children: [{ type: 'text', value: "Every type and field, as TypeScript, generated from the format's " }, link(version().href, 'JSON Schema'), { type: 'text', value: ':' }] };
      return [intro, jsx('div', [['className', 'typedef-spec']], [code('ts', spec(), 'title="spec/index.ts"')])];
    });
  };
}
