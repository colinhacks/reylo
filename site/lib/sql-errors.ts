import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { diagnosticDirective, type Diagnostic } from 'docs-kit/diagnostics';
import { toAgentMarkdown } from 'docs-kit/remark-sections';
import { hashOf } from './remark-mapper.ts';

// The page's error examples: a `sql` block marked `errors` (```sql tab="get_user.sql" dialect="all"
// errors```) is a statement that fails, shown with the errors Reylo's content mapper gives it, the
// database's own or Reylo's, each a squiggle under its range and its message in a card pinned under
// the line (docs-kit's mdx/diagnostics.ts draws them). `nub run capture` (scripts/capture-types.ts)
// sends each such block to the mapper itself, against the page's schema in each of its dialects, and
// writes what comes back to lib/sql-errors.json; `renderErrors` below writes them into the block as
// docs-kit's `// @diagnostic:` lines. An error example is no statement a `ts twoslash` block can
// import, and the capture refuses one that has no error.

/** lib/sql-errors.json: dialect, then file name, then the hash of the block's text, then its errors as the editor places them. */
export type Errors = Partial<Record<'postgres' | 'mysql' | 'sqlite' | 'duckdb', Record<string, Record<string, Diagnostic[]>>>>;

/** A `sql` block whose meta says `errors`: an example of a statement that fails. */
export const isErrorExample = (node: { lang?: string | null; meta?: string | null }) => (node.lang ?? 'sql') === 'sql' && /(?:^|\s)errors(?:\s|$)/.test(node.meta ?? '');

/** lib/sql-errors.json as the build reads it, again for every page, so a new capture shows on the next render. */
export function readErrors(site = process.cwd()): Errors {
  const file = path.join(site, 'lib', 'sql-errors.json');
  return existsSync(file) ? (JSON.parse(readFileSync(file, 'utf8')) as Errors) : {};
}

/** The block's text with its errors written above it as docs-kit's directives. */
export const withErrors = (text: string, errors: Diagnostic[]) => [...errors.map(diagnosticDirective), text].join('\n');

/** The few pieces of lib/remark-mapper.ts's tree this file reads and writes. */
type Node = { type: string; name?: string | null; lang?: string | null; meta?: string | null; value?: string; attributes?: { type: string; name?: string; value?: unknown }[]; data?: Record<string, unknown>; children?: Node[] };
type Example = { node: Node & { value: string }; parent: Node; name: string | undefined; dialect: keyof Errors; hash: string; line: number };

const DIALECT_NAME = { postgres: 'Postgres', mysql: 'MySQL', sqlite: 'SQLite', duckdb: 'DuckDB' } as const;

const jsx = (name: string, attributes: [string, string | null][], children: Node[]): Node => ({
  type: 'mdxJsxFlowElement',
  name,
  attributes: attributes.map(([name, value]) => ({ type: 'mdxJsxAttribute', name, value })),
  children,
});

/** The element whose children hold `child`, anywhere under `node`. */
function parentOf(node: Node, child: Node): Node | undefined {
  for (const each of node.children ?? []) {
    if (each === child) return node;
    const found = parentOf(each, child);
    if (found) return found;
  }
  return undefined;
}

/**
 * Each error example of the file (`blocks`, as lib/remark-mapper.ts collects them, one per dialect
 * the example is shown in) gets its captured errors. Where they read the same in every dialect, the
 * block is shown once with them; where they differ (each database words its own errors), the block
 * becomes a panel per dialect in <DialectTabs follow>, and the code-tab group it is a tab of gets the
 * Postgres | MySQL | SQLite switch at the right end of its strip, as an example whose statement
 * differs per dialect does.
 */
export function renderErrors(tree: Node, blocks: Example[], page: string) {
  const examples = new Map<Node, Example[]>();
  for (const block of blocks) if (isErrorExample(block.node)) examples.set(block.node, [...(examples.get(block.node) ?? []), block]);
  if (!examples.size) return;
  const captured = readErrors();
  for (const [node, list] of examples) {
    node.meta = (node.meta ?? '').replace(/(?:^|\s)errors(?=\s|$)/, '').trim();
    const found = list.map((block) => {
      const errors = block.name && captured[block.dialect]?.[block.name]?.[block.hash];
      if (!errors) throw new Error(`${page}:${block.line}: lib/sql-errors.json holds no errors for the ${DIALECT_NAME[block.dialect]} error example ${block.name ?? '(it has no file name: give it tab="<name>.sql")'}; run \`nub run capture\` in site/ (with the scratch databases running), then build again.`);
      return { block, errors };
    });
    const text = list[0]!.node.value;
    if (found.every(({ errors }) => JSON.stringify(errors) === JSON.stringify(found[0]!.errors))) {
      node.value = withErrors(text, found[0]!.errors);
      continue;
    }
    const parent = list[0]!.parent;
    const panels = found.map(({ block, errors }) => jsx('Dialect', [['value', DIALECT_NAME[block.dialect]]], [{ ...node, data: { ...node.data }, value: withErrors(text, errors) }]));
    parent.children!.splice(parent.children!.indexOf(node), 1, jsx('DialectTabs', [['follow', null]], panels));
    const group = parent.name === 'CodeBlockTab' ? parentOf(tree, parent) : undefined;
    if (group?.name === 'CodeBlockTabs' && !group.children!.some((child) => child.name === 'DialectSwitch')) {
      const offered = found.map(({ block }) => DIALECT_NAME[block.dialect]);
      group.children!.push(jsx('DialectSwitch', [['aside', null], ...(offered.length < 3 ? [['dialects', offered.join(',')] as [string, string]] : [])], []));
    }
  }
}

// The page's Markdown (lib/markdown.ts: /index.md and /llms.txt) and each heading's copy button read
// a section from its source, where an error example is its statement alone: its errors come from
// lib/sql-errors.json at build time. Here each example's source gains them, under the block, as `tsc`
// prints them (`get_user.sql:1:12 - error reylo6: …`), each dialect's where they differ.

const EXAMPLE = /^```sql([^\n]*)\n([\s\S]*?)\n```$/gm;
const metaOf = (meta: string, name: string) => new RegExp(`(?:^|\\s)${name}="([^"]*)"`).exec(meta)?.[1];

/** One error example's fence followed by its errors, or the fence as it is where none were captured. */
function exampleMarkdown(fence: string, meta: string, text: string, captured: Errors): string {
  const name = metaOf(meta, 'tab') ?? metaOf(meta, 'file') ?? metaOf(meta, 'title');
  const dialect = metaOf(meta, 'dialect') ?? 'postgres';
  const dialects = (dialect === 'all' ? Object.keys(DIALECT_NAME) : [dialect]) as (keyof Errors)[];
  const hash = hashOf(text);
  const lines: { dialects: string[]; text: string }[] = [];
  for (const d of dialects) {
    const errors = name && captured[d]?.[name]?.[hash];
    if (!errors) return fence;
    const printed = errors.map((e) => `${name}:${e.start.line}:${e.start.column} - error ${e.source.replace(/^(\w+)\((\d+)\)$/, '$1$2')}: ${e.message}`).join('\n');
    const same = lines.find((l) => l.text === printed);
    if (same) same.dialects.push(DIALECT_NAME[d]);
    else lines.push({ dialects: [DIALECT_NAME[d]], text: printed });
  }
  const shown = lines.length === 1 ? lines[0]!.text : lines.map((l) => l.text.replace(/^/gm, `${l.dialects.join(', ')}: `)).join('\n');
  return `${fence}\n\n\`\`\`text\n${shown}\n\`\`\``;
}

/** The source with each error example followed by the errors the page shows for it. */
export function expandErrors(source: string, captured: Errors = readErrors()): string {
  return source.replace(EXAMPLE, (fence, meta: string, text: string) => (isErrorExample({ lang: 'sql', meta }) ? exampleMarkdown(fence, meta, text, captured) : fence));
}

type Element = { properties?: Record<string, unknown>; children?: Element[] };

/**
 * After docs-kit's rehypeSections has put each heading's section Markdown on it (`data-section`),
 * each error example there is followed by its errors, as the page's Markdown has them.
 */
export function rehypeErrorSections() {
  return (tree: Element, file: { value?: unknown }) => {
    const source = String(file.value ?? '');
    if (!/^```sql[^\n]*\serrors\b/m.test(source)) return;
    const captured = readErrors();
    const swaps = [...source.matchAll(EXAMPLE)].filter(([, meta = '']) => isErrorExample({ lang: 'sql', meta })).map(([fence]) => [toAgentMarkdown(fence).trim(), toAgentMarkdown(expandErrors(fence, captured)).trim()] as const);
    const visit = (node: Element) => {
      const section = node.properties?.['data-section'];
      if (typeof section === 'string') node.properties!['data-section'] = swaps.reduce((text, [raw, shown]) => text.replace(raw, shown), section);
      node.children?.forEach(visit);
    };
    visit(tree);
  };
}
