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

/** A line of code: its tokens, each its text and its colour by day and by night, as the docs' code has them (Shiki's github-light and github-dark). */
export type Line = [text: string, light: string, dark: string][];
/** A statement's parameters and result, as the docs' inference cards show them: `params` is undefined for none, `returns` is `void` for no rows. */
export type HomeVariant = { file: string; schema?: Line[]; sql: Line[]; params?: Line[]; returns: Line[] };
export type HomeExample = { id: string; title: string; lede: string; variants: Partial<Record<Dialect, HomeVariant>> };
export type HomeData = {
  examples: HomeExample[];
  schemas: Partial<Record<Dialect, Line[]>>;
  intro: { sql: Line[]; ts: Line[]; type: Line[] };
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
      variants[dialect] = {
        file: block.name!.replace(/^inferred_/, ''),
        ...(block.schema !== undefined && { schema: lines(block.schema, 'sql') }),
        sql: lines(block.node.value, 'sql'),
        ...types(WIDTH),
      };
    }
    if (Object.keys(variants).length) examples.push({ id, title, lede, variants });
  }

  // The schema every card without its own is analyzed against: the main page's, per dialect.
  const page = collectFiles(pageFiles(site), site);
  const schemas: HomeData['schemas'] = {};
  for (const dialect of Object.keys(DIALECT_NAME) as Dialect[]) {
    const parts = page.filter((b) => b.name === SCHEMA_FILE && b.dialect === dialect).map((b) => b.node.value.replace(/\n$/, ''));
    if (parts.length) schemas[dialect] = lines(parts.join('\n\n'), 'sql');
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
    schemas,
    intro: { sql: lines(intro.node.value, 'sql'), ts: lines(ts, 'ts'), type: lines(`const author: ${formatType(resultType(row, cardinality), 44)}`, 'ts') },
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
