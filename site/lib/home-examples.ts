import path from 'node:path';
import { createHighlighter } from 'shiki';
import type { Plugin } from 'vite';
import { pageFiles } from './page-files.ts';
import { collect, collectFiles, DIALECT_NAME, type Dialect, type MdNode, INFERENCE_CARD, isStatement, moduleTypes, type Modules, parsePage, readModules, SCHEMA_FILE } from './remark-mapper.ts';
import { formatType, resultType } from '../../src/type-format.ts';

/*
 * What the homepage shows (components/home-landing.tsx), read from the docs and the types
 * captured for them (lib/sql-modules.json), never typed by hand: the Inference section's cards
 * (content/sections/03-inference.mdx), the first of each h2, each with its statement and types in
 * every dialect it has; the main page's schema in each dialect; and the intro's `get_author.sql`
 * with the type its import gives. Highlighted here, once, as lines of [text, colour] tokens, so the
 * page ships highlighted text and no highlighter. `virtual:home-examples` is all three.
 */

/** A line of code: its tokens, each its text and its colour by day and by night, as the docs' code has them (Shiki's github-light and github-dark), and for a table's name, the table, whose declaration a hover shows. */
export type Line = [text: string, light: string, dark: string, table?: string][];
/** A statement's parameters and result, as the docs' inference cards show them: `params` is undefined for none, `returns` is `void` for no rows. `tables` holds the declaration of each table it names. */
export type HomeVariant = { file: string; sql: Line[]; tables: Record<string, Line[]>; params?: Line[]; returns: Line[] };
export type HomeExample = { id: string; title: string; lede: string; variants: Partial<Record<Dialect, HomeVariant>> };
export type HomeData = {
  examples: HomeExample[];
  intro: { sql: Line[]; tables: Record<string, Line[]>; ts: Line[]; type: Line[] };
};

const VIRTUAL = 'virtual:home-examples';
const SECTION = 'content/sections/03-inference.mdx';
const INTRO = 'content/docs/index.mdx';
/** How many characters of a type fit on a line before it breaks: the docs' cards' width (lib/remark-mapper.ts). */
const WIDTH = 64;
const THEMES = { light: 'github-light', dark: 'github-dark' };

type Token = { content: string; htmlStyle?: Record<string, string> };

/** A heading's or a paragraph's text, inline code in backticks; a heading's `[#id]` comes off. */
function textOf(node: MdNode): string {
  if (node.type === 'text') return String(node.value ?? '');
  if (node.type === 'inlineCode') return `\`${String(node.value ?? '')}\``;
  return (node.children ?? []).map(textOf).join('');
}

const slug = (s: string) => s.toLowerCase().replace(/`/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');

/** A paragraph's first sentence, without the colon that hands off to its example. */
const firstSentence = (text: string) => (/^(.*?[.:])(\s|$)/.exec(text)?.[1] ?? text).replace(/:$/, '.');

/** Each `create table` of a schema, by the table's name, with the comment lines above it. */
function declarations(schema: string): Map<string, string> {
  const out = new Map<string, string>();
  for (const statement of statements(schema)) {
    const name = /^(?:\s*--[^\n]*\n)*\s*create\s+(?:temporary\s+)?table\s+(?:if\s+not\s+exists\s+)?[`"]?(\w+)/i.exec(statement)?.[1];
    if (name) out.set(name.toLowerCase(), `${statement.trim()};`);
  }
  return out;
}

/** A script's statements, split at each semicolon outside a string. */
function statements(script: string): string[] {
  const out: string[] = [];
  let start = 0;
  for (let i = 0; i < script.length; i++) {
    const c = script[i];
    if (c === "'") i = script.indexOf("'", i + 1) < 0 ? script.length : script.indexOf("'", i + 1);
    else if (c === '-' && script[i + 1] === '-') i = script.indexOf('\n', i) < 0 ? script.length : script.indexOf('\n', i);
    else if (c === ';') {
      out.push(script.slice(start, i));
      start = i + 1;
    }
  }
  if (script.slice(start).trim()) out.push(script.slice(start));
  return out;
}

/** Where a statement names one of the tables, as [start, end, table]: an identifier outside strings and comments, not after a dot. */
function tableNames(sql: string, tables: Map<string, string>): [number, number, string][] {
  const out: [number, number, string][] = [];
  for (let i = 0; i < sql.length; i++) {
    const c = sql[i];
    if (c === "'") i = sql.indexOf("'", i + 1) < 0 ? sql.length : sql.indexOf("'", i + 1);
    else if (c === '-' && sql[i + 1] === '-') i = sql.indexOf('\n', i) < 0 ? sql.length : sql.indexOf('\n', i);
    else if (/[A-Za-z_]/.test(c)) {
      let end = i;
      while (end < sql.length && /\w/.test(sql[end])) end++;
      const word = sql.slice(i, end).toLowerCase();
      if (tables.has(word) && !/\.\s*$/.test(sql.slice(0, i)) && sql[i - 1] !== ':') out.push([i, end, word]);
      i = end - 1;
    }
  }
  return out;
}

/** Highlighted lines with each table's name a token of its own, marked with the table. */
function markTables(code: string, highlighted: Line[], tables: Map<string, string>): Line[] {
  const spans = tableNames(code, tables);
  if (!spans.length) return highlighted;
  let offset = 0;
  return highlighted.map((line) => {
    const out: Line = [];
    for (const [text, light, dark] of line) {
      const start = offset;
      offset += text.length;
      let at = start;
      for (const [s, e, table] of spans) {
        if (e <= at || s >= offset) continue;
        if (s > at) out.push([text.slice(at - start, s - start), light, dark]);
        out.push([text.slice(Math.max(s, at) - start, Math.min(e, offset) - start), light, dark, table]);
        at = Math.min(e, offset);
      }
      if (at < offset) out.push([text.slice(at - start), light, dark]);
    }
    offset += 1;
    return out;
  });
}

export async function buildHomeData(site = process.cwd(), modules: Modules = readModules(site)): Promise<HomeData> {
  const highlighter = await createHighlighter({ themes: Object.values(THEMES), langs: ['sql', 'ts'] });
  const lines = (code: string, lang: 'sql' | 'ts'): Line[] =>
    (highlighter.codeToTokens(code.replace(/\n$/, ''), { lang, themes: THEMES, defaultColor: false }).tokens as Token[][]).map((line) =>
      line.map((t): Line[number] => [t.content, t.htmlStyle?.['--shiki-light'] ?? '#24292e', t.htmlStyle?.['--shiki-dark'] ?? '#e1e4e8']),
    );

  // The Inference section's cards, by the h2 each sits under, and the paragraph before it.
  const tree = parsePage(path.join(site, SECTION));
  const cards: { card: MdNode; title: string; id: string; lede: string }[] = [];
  let title = '';
  let id = '';
  let lede = '';
  const walk = (node: MdNode) => {
    for (const child of node.children ?? []) {
      if (child.type === 'heading' && (child as MdNode & { depth?: number }).depth === 2) {
        const text = textOf(child);
        title = text.replace(/\s*\[#[\w-]+\]\s*$/, '');
        id = /\[#([\w-]+)\]\s*$/.exec(text)?.[1] ?? slug(title);
        lede = '';
      } else if (child.type === 'paragraph' && node.type === 'root') {
        lede = firstSentence(textOf(child));
      } else if (child.type === 'mdxJsxFlowElement' && child.name === INFERENCE_CARD) {
        if (!cards.some((c) => c.id === id)) cards.push({ card: child, title, id, lede });
      } else walk(child);
    }
  };
  walk(tree);
  const blocks = collect(tree);

  // The schema every card without its own is analyzed against: the main page's, per dialect.
  const page = collectFiles(pageFiles(site), site);
  const pageTables: Partial<Record<Dialect, Map<string, string>>> = {};
  for (const dialect of Object.keys(DIALECT_NAME) as Dialect[]) {
    const parts = page.filter((b) => b.name === SCHEMA_FILE && b.dialect === dialect).map((b) => b.node.value.replace(/\n$/, ''));
    if (parts.length) pageTables[dialect] = declarations(parts.join('\n\n'));
  }
  /** A statement highlighted, its tables marked, and the declarations of those it names. */
  const statement = (sql: string, tables: Map<string, string>) => {
    const marked = markTables(sql.replace(/\n$/, ''), lines(sql, 'sql'), tables);
    const named = new Set(marked.flat().flatMap((t) => (t[3] ? [t[3]] : [])));
    return { sql: marked, tables: Object.fromEntries([...named].map((t) => [t, lines(tables.get(t)!, 'sql')])) };
  };

  const examples: HomeExample[] = [];
  for (const { card, title, id, lede } of cards) {
    const statements = blocks.filter((b) => b.parent === card && isStatement(b));
    const variants: HomeExample['variants'] = {};
    for (const dialect of Object.keys(DIALECT_NAME) as Dialect[]) {
      const block = statements.find((b) => b.dialect === dialect);
      const module = block && modules[dialect]?.[block.name!]?.[block.hash];
      if (!block || module === undefined) continue;
      const { params, row, cardinality } = moduleTypes(module);
      const types = (width: number) => ({
        ...(params && { params: lines(formatType(params, width), 'ts') }),
        returns: lines(formatType(resultType(row, cardinality), width), 'ts'),
      });
      const tables = block.schema !== undefined ? declarations(block.schema) : (pageTables[dialect] ?? new Map<string, string>());
      variants[dialect] = {
        file: block.name!.replace(/^inferred_/, ''),
        ...statement(block.node.value, tables),
        ...types(WIDTH),
      };
    }
    if (Object.keys(variants).length) examples.push({ id, title, lede, variants });
  }

  // The intro's statement, and the type of what its import's `.run()` resolves to.
  const intro = collect(parsePage(path.join(site, INTRO))).find((b) => b.name === 'get_author.sql' && b.dialect === 'postgres');
  const introModule = intro && modules.postgres?.['get_author.sql']?.[intro.hash];
  if (!intro || introModule === undefined) throw new Error(`lib/home-examples.ts: no captured module for get_author.sql in ${INTRO}; run \`nub run capture\` in site/.`);
  const { row, cardinality } = moduleTypes(introModule);
  // The intro's main.ts, without its Twoslash directives and the completion it opens.
  const main = collect(parsePage(path.join(site, INTRO))).find((b) => b.name === 'main.ts')?.node.value;
  if (main === undefined) throw new Error(`lib/home-examples.ts: no main.ts block in ${INTRO}.`);
  const ts = main.split('\n').filter((l) => !l.startsWith('// @')).join('\n').replace(/\n+author\?\.[\s\S]*$/, '');
  const data: HomeData = {
    examples,
    intro: { ...statement(intro.node.value, pageTables.postgres ?? new Map()), ts: lines(ts, 'ts'), type: lines(`const author: ${formatType(resultType(row, cardinality), 44)}`, 'ts') },
  };
  highlighter.dispose();
  return data;
}

/** The Vite plugin behind `virtual:home-examples`. */
export function homeExamples(): Plugin {
  return {
    name: 'reylo-home-examples',
    resolveId: (id) => (id === VIRTUAL ? `\0${VIRTUAL}` : undefined),
    async load(id) {
      if (id !== `\0${VIRTUAL}`) return;
      const site = process.cwd();
      for (const file of [SECTION, INTRO, 'lib/sql-modules.json']) this.addWatchFile(path.join(site, file));
      return `export default ${JSON.stringify(await buildHomeData(site))};`;
    },
  };
}
