import { toAgentMarkdown } from 'docs-kit/remark-sections';
import { hashOf, INFERRED, readModules, type Dialect, type Modules } from './remark-mapper.ts';
import { DIALECTS } from './dialects.ts';
import { formatType, resultType } from '../../src/type-format.ts';

// The page's Markdown (lib/markdown.ts: /index.md and /llms.txt) and each heading's copy button
// read a section from its source, where an `<Inferred>` card is its schema and statement blocks
// alone: the types the card shows come from lib/sql-modules.json at build time
// (lib/remark-mapper.ts). Here each card's source becomes Markdown that carries them: per group of
// dialects that read the same, the schema, the query, and its params and result as TypeScript.

const CARD = new RegExp(`<${INFERRED}>([\\s\\S]*?)</${INFERRED}>`, 'g');
const FENCE = /^```sql([^\n]*)\n([\s\S]*?)\n```$/gm;
const metaOf = (meta: string, name: string) => new RegExp(`(?:^|\\s)${name}="([^"]*)"`).exec(meta)?.[1];
const ALL = DIALECTS.map((d) => d.toLowerCase() as Dialect);

/** A card's source as Markdown with its types, or the source as it is where a module is missing. */
function cardMarkdown(body: string, modules: Modules): string {
  const schemas = new Map<string, string>();
  const statements = new Map<Dialect, { name: string; text: string }>();
  for (const [, meta = '', text = ''] of body.matchAll(FENCE)) {
    const dialect = metaOf(meta, 'dialect') ?? 'all';
    const dialects = dialect === 'all' ? ALL : [dialect as Dialect];
    if (/(?:^|\s)schema(?:\s|$)/.test(meta)) for (const d of dialects) schemas.set(d, text);
    else {
      const name = metaOf(meta, 'file');
      if (name) for (const d of dialects) statements.set(d, { name, text });
    }
  }
  const name = (d: Dialect) => DIALECTS.find((n) => n.toLowerCase() === d)!;
  const label = (dialects: Dialect[]) => dialects.map(name).join(', ').replace(/, ([^,]+)$/, ' and $1');
  // Each dialect's schema, then its query with its types; dialects that read the same share one.
  const shown = ALL.filter((d) => statements.has(d));
  const parts: { dialects: Dialect[]; text: string }[] = [];
  for (const dialect of shown) {
    const statement = statements.get(dialect)!;
    const schema = schemas.get(dialect);
    const hash = hashOf(schema === undefined ? statement.text : `${schema}\n\0\n${statement.text}`);
    const module = modules[dialect]?.[statement.name]?.[hash];
    if (module === undefined) return body;
    const params = /^export type Params = (.*);$/m.exec(module)?.[1];
    const row = /^export type Row = (.*);$/m.exec(module)?.[1] ?? 'never';
    const cardinality = /, "(one|maybeOne|many|none)", "(?:postgres|mysql|sqlite)"> & \{/.exec(module)?.[1] ?? 'many';
    const types = [
      `// Params\n${params && params !== 'Record<never, never>' ? formatType(params, 64) : 'void'}`,
      `// Returns\n${formatType(resultType(row, cardinality), 64)}`,
    ].join('\n');
    const text = `\`\`\`sql\n${statement.text}\n\`\`\`\n\n\`\`\`ts\n${types}\n\`\`\``;
    const same = parts.find((p) => p.text === text);
    if (same) same.dialects.push(dialect);
    else parts.push({ dialects: [dialect], text });
  }
  const withSchema = shown.filter((d) => schemas.has(d));
  const oneSchema = new Set(withSchema.map((d) => schemas.get(d))).size === 1;
  const schemaText = withSchema.map((d) => `\`\`\`sql\n-- Schema${oneSchema && withSchema.length === shown.length ? '' : ` (${name(d)})`}\n${schemas.get(d)}\n\`\`\``).slice(0, oneSchema ? 1 : undefined);
  const every = parts.length === 1 && parts[0]!.dialects.length === ALL.length;
  const typed = parts.map((p) => (every ? p.text : `${label(p.dialects)}:\n\n${p.text}`));
  return [...schemaText, ...typed].join('\n\n');
}

/** The source with each `<Inferred>` card written out as Markdown with its types. */
export function expandInferred(source: string, modules: Modules = readModules()): string {
  return source.replace(CARD, (_, body: string) => cardMarkdown(body, modules));
}

type Element = { properties?: Record<string, unknown>; children?: Element[] };

/**
 * After docs-kit's rehypeSections has put each heading's section Markdown on it (`data-section`),
 * each card's text there is replaced by the card with its types, as the page's Markdown has it.
 */
export function rehypeInferredSections() {
  return (tree: Element, file: { value?: unknown }) => {
    const source = String(file.value ?? '');
    if (!source.includes(`<${INFERRED}>`)) return;
    const modules = readModules();
    const swaps = [...source.matchAll(CARD)].map(([card]) => [toAgentMarkdown(card).trim(), toAgentMarkdown(expandInferred(card, modules)).trim()] as const);
    const visit = (node: Element) => {
      const section = node.properties?.['data-section'];
      if (typeof section === 'string') node.properties!['data-section'] = swaps.reduce((text, [raw, typed]) => text.replace(raw, typed), section);
      node.children?.forEach(visit);
    };
    visit(tree);
  };
}
