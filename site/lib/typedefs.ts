// The cards of the type format's page (content/docs/typedefs.mdx) and of the analyze page
// (content/docs/analyze.mdx).
//
// A `<TypeDef>` card is one kind: its examples, written by hand as one jsonc block of types, each
// under a comment of its own, which the build holds to the format's JSON Schema; the values the kind
// holds (`holds`); and each database's types of the kind (`types`). lib/remark-typedefs.ts renders
// it as rows: Types, Holds and Databases.
//
// An `<Analyzed>` card is a `.sql` file and the schema it reads: `nub run capture-typedefs` runs
// `reylo analyze` on it, as YAML and as JSON, and writes both outputs to lib/typedefs.json, which the
// build renders the card from, so the page shows the command's real output.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import type { MdNode } from './remark-mapper.ts';

export const TYPEDEF = 'TypeDef';
export const ANALYZED = 'Analyzed';

/** The databases a card lists types for, in the order the docs name them. */
export const DATABASES = ['postgres', 'mysql', 'sqlite', 'duckdb'] as const;
export type Database = (typeof DATABASES)[number];
export const DATABASE_NAMES: Record<Database, string> = { postgres: 'Postgres', mysql: 'MySQL', sqlite: 'SQLite', duckdb: 'DuckDB' };

export type CardDialect = 'postgres' | 'mysql' | 'sqlite';
const DIALECTS: CardDialect[] = ['postgres', 'mysql', 'sqlite'];

export type TypeCard = {
  node: MdNode;
  line: number;
  /** The jsonc block of example types, as written. */
  examples: MdNode;
  /** Values the kind holds. */
  holds: string[];
  /** Each database's types of this kind: a type's name, or a phrase ("a composite type"). */
  types: Partial<Record<Database, string[]>>;
};

export type AnalyzedCard = {
  node: MdNode;
  line: number;
  /** The file's path in the project, as the command names it. */
  file: string;
  dialect: CardDialect;
  /** The schema the statement reads, built before it is analyzed. */
  schema?: string;
  query: string;
  /** Which output the card shows. */
  format: 'yaml' | 'json';
  /** How many lines of the output to show, a `# …` after them: all where absent. */
  lines?: number;
};

const attr = (node: MdNode, name: string) => node.attributes?.find((a) => a.type === 'mdxJsxAttribute' && a.name === name)?.value;

/** An attribute written as an expression of JSON (`holds={["true"]}`), or as a string. */
function json<T>(node: MdNode, name: string, empty: T, tag: string): T {
  const value = attr(node, name);
  if (value === undefined || value === null) return empty;
  const source = typeof value === 'string' ? value : String((value as { value?: unknown }).value);
  try {
    return JSON.parse(source) as T;
  } catch {
    throw new Error(`line ${node.position?.start.line ?? '?'}: <${tag} ${name}={…}> is no JSON: ${source}`);
  }
}

function each(tree: MdNode, name: string): MdNode[] {
  const out: MdNode[] = [];
  const walk = (node: MdNode) => {
    for (const child of node.children ?? []) {
      if (child.type === 'mdxJsxFlowElement' && child.name === name) out.push(child);
      else walk(child);
    }
  };
  walk(tree);
  return out;
}

/** Every `<TypeDef>` card of a page's tree. */
export function typeCards(tree: MdNode): TypeCard[] {
  return each(tree, TYPEDEF).map((node) => {
    const line = node.position?.start.line ?? 0;
    const blocks = (node.children ?? []).filter((c) => c.type === 'code');
    if (blocks.length !== 1 || blocks[0]!.lang !== 'jsonc') throw new Error(`line ${line}: a <${TYPEDEF}> card holds one jsonc block, its example types`);
    const types = json<Partial<Record<string, string[]>>>(node, 'types', {}, TYPEDEF);
    const unknown = Object.keys(types).find((d) => !(DATABASES as readonly string[]).includes(d));
    if (unknown) throw new Error(`line ${line}: <${TYPEDEF} types={…}> names ${unknown}; name ${DATABASES.join(', ')}`);
    return { node, line, examples: blocks[0]!, holds: json(node, 'holds', [], TYPEDEF), types: types as TypeCard['types'] };
  });
}

/** Every `<Analyzed>` card of a page's tree. */
export function analyzedCards(tree: MdNode): AnalyzedCard[] {
  return each(tree, ANALYZED).map((node) => {
    const line = node.position?.start.line ?? 0;
    const where = `line ${line}: an <${ANALYZED}> card`;
    const code = (node.children ?? []).filter((c) => c.type === 'code' && c.lang === 'sql');
    const schema = code.find((c) => /(?:^|\s)schema(?:\s|$)/.test(c.meta ?? ''));
    const queries = code.filter((c) => c !== schema);
    if (queries.length !== 1) throw new Error(`${where} holds one sql block, the file's statement, and a sql schema block where it reads a table`);
    const file = attr(node, 'file');
    if (typeof file !== 'string' || !file.endsWith('.sql')) throw new Error(`${where} names its file="…sql"`);
    const dialect = (attr(node, 'dialect') ?? 'postgres') as CardDialect;
    if (!DIALECTS.includes(dialect)) throw new Error(`${where} names dialect="${dialect}"; write postgres, mysql or sqlite`);
    const format = (attr(node, 'format') ?? 'yaml') as AnalyzedCard['format'];
    if (format !== 'yaml' && format !== 'json') throw new Error(`${where} names format="${format}"; write yaml or json`);
    const lines = attr(node, 'lines');
    return { node, line, file, dialect, ...(schema && { schema: schema.value! }), query: queries[0]!.value!, format, ...(typeof lines === 'string' && { lines: Number(lines) }) };
  });
}

/** The key of a card's captured outputs in lib/typedefs.json: one per file, schema and statement. */
export const keyOf = (c: Pick<AnalyzedCard, 'dialect' | 'schema' | 'query' | 'file'>) =>
  createHash('sha256').update(JSON.stringify([c.dialect, c.schema ?? '', c.query, c.file])).digest('hex').slice(0, 16);

export type Captures = Record<string, { dialect: CardDialect; file: string; query: string; yaml: string; json: string }>;
export const CAPTURES = 'typedefs.json';
export const readCaptures = (site = process.cwd()): Captures => JSON.parse(readFileSync(path.join(site, 'lib', CAPTURES), 'utf8'));

/** The JSON values a jsonc block holds, in order: each top-level value, its comments dropped. */
export function values(text: string): unknown[] {
  // Line comments, outside strings.
  let plain = '';
  for (let i = 0, quoted = false; i < text.length; i++) {
    const c = text[i]!;
    if (quoted) {
      plain += c;
      if (c === '\\') plain += text[++i] ?? '';
      else if (c === '"') quoted = false;
    } else if (c === '"') {
      quoted = true;
      plain += c;
    } else if (c === '/' && text[i + 1] === '/') {
      while (i < text.length && text[i] !== '\n') i++;
      plain += '\n';
    } else plain += c;
  }
  // Each value from its opening bracket to the one that closes it.
  const out: unknown[] = [];
  let i = 0;
  while (i < plain.length) {
    while (i < plain.length && /\s/.test(plain[i]!)) i++;
    if (i >= plain.length) break;
    let depth = 0;
    let quoted = false;
    const start = i;
    for (; i < plain.length; i++) {
      const c = plain[i]!;
      if (quoted) {
        if (c === '\\') i++;
        else if (c === '"') quoted = false;
      } else if (c === '"') quoted = true;
      else if (c === '{' || c === '[') depth++;
      else if ((c === '}' || c === ']') && --depth === 0) {
        i++;
        break;
      }
    }
    out.push(JSON.parse(plain.slice(start, i)));
  }
  return out;
}
