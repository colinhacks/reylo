import { createHash } from 'node:crypto';
import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { createProcessor } from '@mdx-js/mdx';
import { remarkCodeTab } from 'fumadocs-core/mdx-plugins';
import type { Plugin } from 'vite';
import { pageFiles } from './page-files.ts';
import { isErrorExample, renderErrors } from './sql-errors.ts';
import { formatType, resultType } from './type-format.ts';

export { pageFiles };

// Twoslash runs TypeScript's JavaScript API (6.0, from fumadocs-twoslash), which cannot run a
// content mapper, and Reylo's mapper needs a database. So `nub run capture` (scripts/
// capture-types.ts) runs the loader's own analysis on every `.sql` block of the pages, with the
// scratch databases, and writes the module the mapper returns for each into lib/sql-modules.json.
// This plugin reads that file: each `ts twoslash` block that imports `./<name>.sql` gets, above its
// `// ---cut---`, the module for the block it imports (`<name>.d.sql.ts`, which the import resolves
// to under allowArbitraryExtensions) and `db` declared as that dialect's client. A block written
// once under an example's SQL tabs becomes one copy per dialect, in <DialectTabs follow>. The
// build never needs a database, and fails when a block's module was not captured.
//
// The conventions (the contract between the page and this file):
// - A `sql` block's file name is its `title`, else its `file` meta attribute, which names the file
//   without showing it and which this plugin removes, else its `tab` when that is not a dialect
//   name; a block whose name ends in `.sql` is a statement, except `schema.sql`, the page's schema.
// - A `sql` block with `dialect="all"` is the same statement in every dialect: it is shown once,
//   analyzed in each, and a `ts twoslash` block that imports it follows the page's dialect choice,
//   as under a code-tab group of dialects.
// - A block's dialect is its `tab` when that is Postgres, MySQL or SQLite; else the enclosing
//   <Dialect value> (or <Tab value>) panel's; else its `dialect="postgres|mysql|sqlite"` meta
//   attribute's, which this plugin removes; else, for a `ts twoslash` block, the dialect of the
//   `.sql` blocks it imports; else Postgres.
// - The page is several files, shown in order: content/docs/index.mdx (the title, the opening and the
//   schema), then one file per h2 section, content/sections/*.mdx, by name (`pageFiles`). An import
//   resolves across all of them: to the nearest block above with that file name and the importing
//   block's dialect, else to the first one below, as if the files were one.
// - A `ts twoslash` block with no dialect of its own whose import resolves to a code-tab group of
//   dialects becomes <DialectTabs follow> with a <Dialect value> panel per dialect of that group. A
//   group of dialects is code tabs named for their dialect (`tab="MySQL"`), or file tabs that each say
//   theirs (`tab="users.sql (MySQL)" file="users.sql" dialect="mysql"`), the way every example shows a
//   statement that differs per dialect beside its `main.ts` tab.
// - Such file tabs, once the TypeScript beside them follows, become one tab named for the file
//   (`users.sql`), whose panel follows the page's dialect choice as well, and the group gets a
//   <DialectSwitch aside>, which docs-kit draws at the right end of its strip: `main.ts | users.sql`
//   on the left, Postgres | MySQL | SQLite on the right (`mergeDialectFiles`).
// - An `<Inferred>` element is a card of the Inference section: its ```sql file="<name>.sql"``` block
//   (with `dialect="all"`, or one per dialect) is a statement whose types the card shows, and its
//   ```sql schema dialect="…"``` blocks, where it has any, are the schema that statement is analyzed
//   against in place of the page's, its module keyed by both texts (`renderInferred`).
// - A `sql` block whose meta says `errors` is an error example, a statement that fails, shown with
//   the errors the content mapper gives it, captured into lib/sql-errors.json (lib/sql-errors.ts); it
//   is no statement an import resolves to.
// - `db` is declared unless the block declares or imports it; a block that names `Bun.` or imports
//   `bun` or `bun:sqlite` gets Bun's types, and one that names `D1Database` without importing it
//   gets Cloudflare's Workers types.
//
// scripts/capture-types.ts parses the pages as Fumadocs does and calls `plan` below, so the
// capture and the build always agree on which statement each import needs.

export type Attribute = { type: string; name?: string; value?: unknown };
export type MdNode = {
  type: string;
  name?: string | null;
  lang?: string | null;
  meta?: string | null;
  value?: string;
  attributes?: Attribute[];
  data?: { tab?: string; tabGroup?: string; [key: string]: unknown };
  children?: MdNode[];
  position?: { start: { line: number; offset?: number } };
};
type Code = MdNode & { type: 'code'; value: string };

export const DIALECTS = ['Postgres', 'MySQL', 'SQLite'] as const;
export type DialectName = (typeof DIALECTS)[number];
export type Dialect = 'postgres' | 'mysql' | 'sqlite';
const DIALECT_OF: Record<string, Dialect> = { postgres: 'postgres', mysql: 'mysql', sqlite: 'sqlite' };
export const DIALECT_NAME: Record<Dialect, DialectName> = { postgres: 'Postgres', mysql: 'MySQL', sqlite: 'SQLite' };
/** The client an example calls `db`: the one Basic usage builds in each dialect. */
const CLIENT: Record<Dialect, string> = {
  postgres: 'import("postgres").Sql',
  mysql: 'import("mysql2/promise").Pool',
  sqlite: 'import("better-sqlite3").Database',
};
/** A tab or panel value or a meta attribute naming a dialect, in any case. */
const dialectOf = (value: unknown): Dialect | undefined => (typeof value === 'string' ? DIALECT_OF[value.toLowerCase()] : undefined);

/** The file whose blocks hold each dialect's schema, in the accordion near the top of a page. */
export const SCHEMA_FILE = 'schema.sql';

export type Block = {
  /** The page file the block is in, relative to the site, when the page is several files. */
  file?: string;
  node: Code;
  /** The node's parent, where an expansion replaces it. */
  parent: MdNode;
  /** Its position among every code block of the page. */
  order: number;
  name: string | undefined;
  dialect: Dialect;
  /** Where the dialect came from; `default` is Postgres for want of anything else. */
  from: 'tab' | 'panel' | 'meta' | 'default';
  /** The code-tab group (Fumadocs' CodeBlockTabs) the block is a tab of, when its tab is a dialect. */
  group: MdNode | undefined;
  line: number;
  hash: string;
  /** Inside an `<Inferred>` card: the card's schema for the block's dialect, when it shows one, which the statement is analyzed against in place of the page's. */
  schema?: string;
  /** Inside an `<Inferred>` card, whose statements the page shows with their types. */
  inferred?: boolean;
};

/** The key a statement's module has in lib/sql-modules.json, under its dialect and file name. */
export const hashOf = (text: string) => createHash('sha256').update(text).digest('hex').slice(0, 16);

/** A Fumadocs meta attribute, such as `title="main.ts"`. */
const attribute = (meta: string | null | undefined, name: string) => new RegExp(`(?:^|\\s)${name}=(?:"([^"]*)"|'([^']*)'|(\\S+))`).exec(meta ?? '')?.slice(1).find((v) => v !== undefined);

export const isTwoslash = (node: MdNode) => (node.lang === 'ts' || node.lang === 'tsx') && /(?:^|\s)twoslash(?:\s|$)/.test(node.meta ?? '');

/**
 * Every code block of the page, in order, with its file name and dialect. Fumadocs' remarkCodeTab
 * has already run: a group of code tabs is a CodeBlockTabs element, and each block's `tab` and
 * `tab-group` have moved from its meta into `data`.
 */
export function collect(tree: MdNode): Block[] {
  const blocks: Block[] = [];
  const walk = (node: MdNode, ancestors: MdNode[]) => {
    for (const child of node.children ?? []) {
      if (child.type === 'code') {
        const code = child as Code;
        // A card's schema is no statement of its own: the card's statements are analyzed against it.
        if (isSchemaBlock(code) && node.name === INFERRED) continue;
        const tab = code.data?.tab;
        const tabDialect = dialectOf(tab);
        const panel = ancestors
          .concat(node)
          .findLast((a) => a.type === 'mdxJsxFlowElement' && (a.name === 'Dialect' || a.name === 'Tab') && dialectOf(attr(a, 'value')));
        const panelDialect = panel && dialectOf(attr(panel, 'value'));
        const metaDialect = attribute(code.meta, 'dialect');
        const every = metaDialect === 'all';
        if (metaDialect !== undefined && !every && !dialectOf(metaDialect)) throw new Error(`${where(code)}: dialect="${metaDialect}" names no dialect; write postgres, mysql, sqlite or all.`);
        const dialect = tabDialect ?? panelDialect ?? dialectOf(metaDialect) ?? 'postgres';
        const name = attribute(code.meta, 'title') ?? attribute(code.meta, 'file') ?? (tab && !tabDialect ? tab : undefined);
        const card = node.name === INFERRED ? node : undefined;
        const shared = { node: code, parent: node, name, line: code.position?.start.line ?? 0, hash: hashOf(code.value), ...(card && { inferred: true }) };
        // In a card, a dialect's statement is analyzed against the card's schema for that dialect, and
        // its module is keyed by both texts.
        const local = (d: Dialect) => {
          const schema = card && cardSchema(card, d);
          return schema === undefined ? {} : { schema, hash: hashOf(`${schema}\n\0\n${code.value}`) };
        };
        // The same statement in every dialect: one block per dialect, grouped as a code-tab group
        // of dialects is, so that an import of it expands into a panel per dialect.
        if (every)
          for (const each of DIALECTS) {
            const d = DIALECT_OF[each.toLowerCase()]!;
            blocks.push({ ...shared, ...local(d), order: blocks.length, dialect: d, from: 'meta', group: code });
          }
        else
          blocks.push({
            ...shared,
            ...local(dialect),
            order: blocks.length,
            dialect,
            from: tabDialect ? 'tab' : panelDialect ? 'panel' : metaDialect ? 'meta' : 'default',
            // A tab named for its dialect, or a file's tab that says its dialect (`tab="users.sql (MySQL)" file="users.sql" dialect="mysql"`).
            group: tabDialect || (tab && dialectOf(metaDialect)) ? ancestors.concat(node).findLast((a) => a.type === 'mdxJsxFlowElement' && a.name === 'CodeBlockTabs') : undefined,
          });
      } else walk(child, ancestors.concat(node));
    }
  };
  walk(tree, []);
  return blocks;
}

function attr(node: MdNode, name: string): unknown {
  return node.attributes?.find((a) => a.type === 'mdxJsxAttribute' && a.name === name)?.value;
}
const where = (node: MdNode) => `line ${node.position?.start.line ?? '?'}`;

/** A database's own page, whose every example is that database's. */
const DIALECT_PAGE = /(^|\/)(postgres|mysql|sqlite)\.mdx$/;

/** The main page's Postgres schema, its `schema.sql` blocks: what the playground's PGlite holds (lib/live/plugin.ts). */
let postgresSchema: string | undefined;
export function pagePostgresSchema(): string {
  if (postgresSchema === undefined) {
    const blocks = collectFiles(pageFiles(process.cwd())).filter((b) => b.name === SCHEMA_FILE && b.dialect === 'postgres');
    if (!blocks.length) throw new Error("the main page has no Postgres schema.sql block, which the live card's database holds");
    postgresSchema = blocks.map((b) => b.node.value).join('\n\n');
  }
  return postgresSchema;
}

/** The element that shows its statements with the types Reylo infers for them (`renderInferred`). */
export const INFERRED = 'Inferred';
/** A card's schema block: ```sql schema dialect="postgres"```. */
const isSchemaBlock = (code: MdNode) => (code.lang ?? 'sql') === 'sql' && /(?:^|\s)schema(?:\s|$)/.test(code.meta ?? '');
/** The card's schema for a dialect: its schema block of that dialect, or of every dialect. */
function cardSchema(card: MdNode, dialect: Dialect): string | undefined {
  const schemas = (card.children ?? []).filter((c) => c.type === 'code' && isSchemaBlock(c)) as Code[];
  const of = (c: Code) => attribute(c.meta, 'dialect') ?? 'all';
  return (schemas.find((c) => dialectOf(of(c)) === dialect) ?? schemas.find((c) => of(c) === 'all'))?.value;
}

/** A `.sql` block that a page analyzes: every SQL block named `*.sql` but the schema and the error examples (a TypeScript tab named Bun.sql is not one). */
export const isStatement = (block: Block) => (block.node.lang ?? 'sql') === 'sql' && !!block.name?.endsWith('.sql') && block.name !== SCHEMA_FILE && !isErrorExample(block.node);

/** `import getUser from "./get_user.sql"`. */
const IMPORT = /\b(?:from|import)\s+["']\.\/([^"']+\.sql)["']/g;

/** The file TypeScript looks up for `./name.sql` under allowArbitraryExtensions. */
export const declarationName = (file: string) => `${file.slice(0, -'.sql'.length)}.d.sql.ts`;

export type Need = { name: string; block: Block };
export type Variant = { dialect: Dialect; needs: Need[] };
export type Plan = { block: Block; variants: Variant[]; expand: boolean };

/**
 * What each `ts twoslash` block needs: per dialect it is shown in, the `.sql` block each of its
 * imports resolves to. A block that writes `// @filename: get_user.d.sql.ts` itself supplies that
 * module, and needs nothing for it.
 */
export function plan(blocks: Block[], targets?: Set<Block>): Plan[] {
  const statements = blocks.filter(isStatement);
  const resolve = (name: string, at: Block, dialect: Dialect | undefined) => {
    const matches = (b: Block) => b.name === name && (dialect === undefined || b.dialect === dialect);
    return statements.findLast((b) => b.order < at.order && matches(b)) ?? statements.find((b) => b.order > at.order && matches(b));
  };
  const plans: Plan[] = [];
  for (const block of blocks) {
    if (targets && !targets.has(block)) continue;
    if (!isTwoslash(block.node)) continue;
    const code = block.node.value;
    const names = [...new Set([...code.matchAll(IMPORT)].map((m) => m[1]!))].filter((name) => !code.includes(`@filename: ${declarationName(name)}`));
    if (!names.length) {
      plans.push({ block, variants: [{ dialect: block.dialect, needs: [] }], expand: false });
      continue;
    }
    const needs = (dialect: Dialect): Need[] =>
      names.map((name) => {
        const found = resolve(name, block, dialect);
        if (!found) throw new Error(`${where(block.node)}: the ts twoslash block imports ./${name}, and the page has no ${DIALECT_NAME[dialect]} block with that file name (a code block titled ${name}, or a code tab of that name).`);
        return { name, block: found };
      });
    if (block.from !== 'default') {
      plans.push({ block, variants: [{ dialect: block.dialect, needs: needs(block.dialect) }], expand: false });
      continue;
    }
    // No dialect of its own: the block follows what it imports. A group of dialect tabs gives it
    // a panel per dialect; a lone block gives it that block's dialect.
    const nearest = names.map((name) => resolve(name, block, undefined)!).filter(Boolean);
    const grouped = nearest.filter((b) => b.group);
    if (grouped.length) {
      const dialects = new Set(grouped.flatMap((b) => statements.filter((s) => s.group === b.group && s.name === b.name).map((s) => s.dialect)));
      const variants = (Object.keys(DIALECT_NAME) as Dialect[]).filter((d) => dialects.has(d)).map((dialect) => ({ dialect, needs: needs(dialect) }));
      // A group of one dialect, such as a dialect page's `main.ts | pg_events.sql`, needs no panel to follow.
      plans.push({ block, variants, expand: variants.length > 1 });
      continue;
    }
    const dialect = nearest.length && nearest.every((b) => b.dialect === nearest[0]!.dialect) ? nearest[0]!.dialect : block.dialect;
    plans.push({ block, variants: [{ dialect, needs: needs(dialect) }], expand: false });
  }
  return plans;
}

/** lib/sql-modules.json: dialect, then file name, then the hash of the block's text, then the module. */
export type Modules = Partial<Record<Dialect, Record<string, Record<string, string>>>>;

/** Whether the block's own code declares or imports `db`. */
const declaresDb = (code: string) =>
  /\b(?:const|let|var|using|function|class)\s+(?:db\b|\{[^}]*\bdb\b[^}]*\}|\[[^\]]*\bdb\b[^\]]*\])/.test(code) ||
  /\bimport\s+(?:type\s+)?(?:db\b|\*\s+as\s+db\b|[^;]*\{[^}]*\bdb\b[^}]*\})/.test(code);

/** The block's code with its hidden setup above `// ---cut---`, for one dialect. */
export function withSetup(block: Block, variant: Variant, modules: Modules, file = 'the page'): string {
  const code = block.node.value;
  const files: string[] = [];
  for (const { name, block: sql } of variant.needs) {
    const module = modules[variant.dialect]?.[name]?.[sql.hash];
    if (module === undefined) {
      throw new Error(`${file}:${block.line}: the ts twoslash block imports ./${name}, and lib/sql-modules.json holds no module for the ${DIALECT_NAME[variant.dialect]} statement at ${sql.file ? `${sql.file}:` : 'line '}${sql.line}; run \`nub run capture\` in site/ (with the scratch databases running), then build again.`);
    }
    files.push(`// @filename: ${declarationName(name)}\n${module}`);
  }
  const globals: string[] = [];
  if (/\bBun\.|\bfrom\s+["']bun(?::sqlite)?["']/.test(code)) globals.push('/// <reference types="bun" />');
  if (/\bD1Database\b/.test(code) && !/@cloudflare\/workers-types/.test(code)) globals.push('/// <reference types="@cloudflare/workers-types" />');
  if (variant.needs.length && !declaresDb(code)) globals.push(`declare const db: ${CLIENT[variant.dialect]};`);
  // The examples read `process.env.DATABASE_URL` with no `!`, as a project that declares it on
  // `process.env` does.
  if (/\bprocess\.env\.DATABASE_URL\b/.test(code)) globals.push('declare namespace NodeJS { interface ProcessEnv { DATABASE_URL: string } }');
  if (globals.length) files.push(`// @filename: globals.d.ts\n${globals.join('\n')}\n`);
  if (!files.length) return code;
  const title = attribute(block.node.meta, 'title');
  const own = code.includes('---cut---') ? code : `// ---cut---\n${code}`;
  const visible = code.includes('@filename') ? code : `// @filename: ${title && /\.[cm]?tsx?$/.test(title) ? title : 'main.ts'}\n${own}`;
  return files.join('') + visible;
}

const jsx = (name: string, attributes: [string, string | null][], children: MdNode[]): MdNode => ({
  type: 'mdxJsxFlowElement',
  name,
  attributes: attributes.map(([name, value]) => ({ type: 'mdxJsxAttribute', name, value })),
  children,
});

/** A file's tree as Fumadocs has it when this plugin runs: MDX parsed, then code tabs grouped. */
export function parsePage(file: string): MdNode {
  // fumadocs-mdx takes the frontmatter off before it parses; blank lines keep the line numbers.
  const text = readFileSync(file, 'utf8').replace(/^---\n[\s\S]*?\n---\n/, (m) => '\n'.repeat(m.split('\n').length - 1));
  const processor = createProcessor();
  const tree = processor.parse(text) as unknown as MdNode;
  (remarkCodeTab as unknown as (this: unknown) => (tree: MdNode) => void).call(processor)(tree);
  return tree;
}

const parsed = new Map<string, { stamp: string; blocks: Block[] }>();

/** The blocks of one of the page's other files, parsed again only when the file changes. */
function blocksOf(file: string, site: string): Block[] {
  const stat = statSync(file);
  const stamp = `${stat.mtimeMs}:${stat.size}`;
  const hit = parsed.get(file);
  if (hit?.stamp === stamp) return hit.blocks;
  const shown = path.relative(site, file);
  const blocks = collect(parsePage(file)).map((block) => ({ ...block, file: shown }));
  parsed.set(file, { stamp, blocks });
  return blocks;
}

/** Every code block of the given files, in their order and numbered across them, each marked with its file. */
export function collectFiles(files: string[], site = process.cwd()): Block[] {
  const blocks = files.flatMap((file) => collect(parsePage(file)).map((block) => ({ ...block, file: path.relative(site, file) })));
  blocks.forEach((block, i) => (block.order = i));
  return blocks;
}

/**
 * A file's own blocks among those of the page's other files, in page order and numbered across
 * them, so that an import resolves to a statement another section shows. A file that is not one of
 * the main page's (a dialect page) is a page by itself.
 */
function inPage(file: string | undefined, own: Block[], site = process.cwd()): Block[] {
  const files = pageFiles(site);
  const at = file ? files.indexOf(path.resolve(file)) : -1;
  const all = at < 0 ? own : [...files.slice(0, at).flatMap((f) => blocksOf(f, site)), ...own, ...files.slice(at + 1).flatMap((f) => blocksOf(f, site))];
  all.forEach((block, i) => (block.order = i));
  return all;
}

/** The statements a file's blocks import from the page's other files, by dialect, name, file and text: what its output depends on beyond its own text and lib/sql-modules.json. */
function imports(plans: Plan[]): string {
  const needs = plans.flatMap((p) => p.variants.flatMap((v) => v.needs.filter((n) => n.block.file).map((n) => `${v.dialect} ${n.name} ${n.block.file} ${n.block.hash}`)));
  return [...new Set(needs)].sort().join('\n');
}

/**
 * What each of the page's files imported from the others when it last compiled. On `globalThis`,
 * since the dev server's plugin (`mapperDependents`) and the compile may load this module apart.
 */
const compiled = ((globalThis as { __reyloMapperImports?: Map<string, string> }).__reyloMapperImports ??= new Map());

/** The page's files whose output an edit to `changed` made stale: each whose imports from the others now resolve to another statement or another text. */
function staleAfter(changed: string, site: string): string[] {
  const stale: string[] = [];
  for (const [file, before] of compiled) {
    if (file === changed || !existsSync(file)) continue;
    try {
      const own = blocksOf(file, site).map((block) => ({ ...block, file: undefined }));
      if (imports(plan(inPage(file, own, site), new Set(own))) !== before) stale.push(file);
    } catch {
      stale.push(file);
    }
  }
  return stale;
}

/**
 * The dev server's half of the page's imports across files: when a file of the page changes, each
 * other file whose imports it changed is compiled again with it, and no other. A section that
 * imports a statement another section shows used to depend on that whole file, so every edit to
 * Usage compiled How it works, Supported drivers and Examples again (3–5 s for an edit to show,
 * measured 2026-10-20), though none of them changes unless the statement does.
 */
export function mapperDependents(): Plugin {
  const decided = new Map<string, string[]>();
  return {
    name: 'reylo:mapper-dependents',
    apply: 'serve',
    hotUpdate({ file, modules, timestamp }) {
      if (!file.endsWith('.mdx')) return;
      // Decided once per edit, for the browser's environment and the server's alike.
      const key = `${file}\0${timestamp}`;
      let stale = decided.get(key);
      if (!stale) {
        stale = staleAfter(path.resolve(file), process.cwd());
        decided.clear();
        decided.set(key, stale);
      }
      const graph = this.environment.moduleGraph;
      const extra = stale.flatMap((other) => [...(graph.getModulesByFile(other) ?? [])]);
      return extra.length ? [...modules, ...extra] : undefined;
    },
  };
}

/** lib/sql-modules.json as the build reads it: the site's own copy, read again for every page so a new capture shows on the next render. */
export function readModules(site = process.cwd()): Modules {
  const file = path.join(site, 'lib', 'sql-modules.json');
  return existsSync(file) ? (JSON.parse(readFileSync(file, 'utf8')) as Modules) : {};
}

/**
 * Each code-tab group's file tabs that each say their dialect (`users.sql (Postgres)`,
 * `users.sql (MySQL)`, `users.sql (SQLite)`) become one tab, `users.sql`, holding <DialectTabs follow>
 * with a <Dialect value> panel per block, and the group gets a <DialectSwitch aside>, which docs-kit
 * draws at the right end of the strip (its code-tabs.tsx). A tab per file and dialect made a strip of
 * four tabs that ran wider than the column on nine of the pages' examples, and on a phone on every
 * one; the strip is now a tab per file and the switch. Runs after the TypeScript beside them has
 * become <DialectTabs follow>, which reads these blocks as a group of dialects.
 */
function mergeDialectFiles(blocks: Block[]) {
  const groups = new Map<MdNode, Map<string, Block[]>>();
  for (const block of blocks) {
    // The file is the tab's label without its dialect: `users.sql` of `users.sql (MySQL)`, and
    // `main.ts` of `main.ts (MySQL)` where the TypeScript differs per dialect too.
    const label = /^(.+) \((Postgres|MySQL|SQLite)\)$/.exec(block.node.data?.tab ?? '');
    if (!label || !block.group || block.group.name !== 'CodeBlockTabs' || block.from !== 'meta' || dialectOf(label[2]) !== block.dialect) continue;
    const files = groups.get(block.group) ?? new Map<string, Block[]>();
    files.set(label[1]!, [...(files.get(label[1]!) ?? []), block]);
    groups.set(block.group, files);
  }
  for (const [group, files] of groups) {
    const strip = group.children!.find((child) => child.name === 'CodeBlockTabsList');
    const dialects = new Set<Dialect>();
    for (const [name, list] of files) {
      if (list.length < 2 || !strip) continue;
      list.sort((a, b) => DIALECTS.indexOf(DIALECT_NAME[a.dialect]) - DIALECTS.indexOf(DIALECT_NAME[b.dialect]));
      const tabs = list.map((block) => block.parent);
      const values = tabs.map((tab) => attr(tab, 'value'));
      const panels = list.map((block) => jsx('Dialect', [['value', DIALECT_NAME[block.dialect]]], block.parent.children!));
      const at = Math.min(...tabs.map((tab) => group.children!.indexOf(tab)));
      group.children = group.children!.filter((child) => !tabs.includes(child));
      group.children.splice(at, 0, jsx('CodeBlockTab', [['value', name]], [jsx('DialectTabs', [['follow', null]], panels)]));
      const first = strip.children!.findIndex((trigger) => values.includes(attr(trigger, 'value')));
      strip.children = strip.children!.filter((trigger) => !values.includes(attr(trigger, 'value')));
      strip.children.splice(first, 0, jsx('CodeBlockTabsTrigger', [['value', name]], [{ type: 'text', value: name }]));
      const chosen = group.attributes?.find((a) => a.name === 'defaultValue');
      if (chosen && values.includes(chosen.value)) chosen.value = name;
      for (const block of list) dialects.add(block.dialect);
    }
    // The switch offers the dialects the group shows; with the page's choice elsewhere, the group
    // shows its first (docs-kit's style.css).
    const offered = DIALECTS.filter((d) => dialects.has(DIALECT_OF[d.toLowerCase()]!));
    if (offered.length) group.children!.push(jsx('DialectSwitch', [['aside', null], ...(offered.length < DIALECTS.length ? [['dialects', offered.join(',')] as [string, string]] : [])], []));
  }
}

/**
 * How an `<Inferred>` card lays out its rows (components/inferred.tsx): `side` puts the parameters
 * and the result in a column beside the query, `rows` under it. The types are broken over lines to
 * the width they get: about 30 characters in the side column, 64 under the query.
 */
export const INFERRED_LAYOUT: 'side' | 'rows' = 'rows';
const TYPE_WIDTH = { side: 30, rows: 64 } as const;

/** A statement's module (as `render` in scripts/capture-types.ts writes it): its parameters, row and cardinality. */
export function moduleTypes(module: string): { params: string | undefined; row: string; cardinality: string } {
  const params = /^export type Params = (.*);$/m.exec(module)?.[1];
  const row = /^export type Row = (.*);$/m.exec(module)?.[1];
  const cardinality = /declare const query: Query<[\s\S]*, "(one|maybeOne|many|none)", "(?:postgres|mysql|sqlite)", QueryError> & \{/.exec(module)?.[1];
  if (params === undefined || row === undefined || cardinality === undefined) throw new Error(`a module lib/remark-mapper.ts cannot read:\n${module}`);
  return { params: params === 'Record<never, never>' ? undefined : params, row, cardinality };
}

const codeNode = (lang: string, value: string): MdNode => ({ type: 'code', lang, meta: null, value });

/**
 * Each `<Inferred>` card of the file: its schema blocks (```sql schema```, one per dialect or one for
 * every dialect) and its statement (```sql file="name.sql" dialect="…"```, once for every dialect or
 * once per dialect) become, per dialect, labelled rows, Schema (where the card has one), Query, Params
 * (where the statement has any) and Returns, holding what `.run()` takes and resolves to, read off
 * the statement's captured module, in a <DialectTabs follow> that follows the page's dialect choice.
 */
function renderInferred(tree: MdNode, blocks: Block[], modules: Modules, page: string) {
  const width = TYPE_WIDTH[INFERRED_LAYOUT];
  const cards: MdNode[] = [];
  const find = (node: MdNode) => {
    for (const child of node.children ?? []) {
      if (child.type === 'mdxJsxFlowElement' && child.name === INFERRED) cards.push(child);
      else find(child);
    }
  };
  find(tree);
  for (const card of cards) {
    const statements = blocks.filter((b) => b.parent === card && isStatement(b));
    if (!statements.length) throw new Error(`${page}:${where(card)}: an <Inferred> card shows no statement; give it a \`sql\` block with file="<name>.sql".`);
    const dialects = (Object.keys(DIALECT_NAME) as Dialect[]).filter((d) => statements.some((b) => b.dialect === d));
    const panels = dialects.map((dialect) => {
      const block = statements.find((b) => b.dialect === dialect)!;
      const module = modules[dialect]?.[block.name!]?.[block.hash];
      if (module === undefined) throw new Error(`${page}:${block.line}: lib/sql-modules.json holds no module for the ${DIALECT_NAME[dialect]} statement ${block.name} of this <Inferred> card; run \`nub run capture\` in site/ (with the scratch databases running), then build again.`);
      const { params, row, cardinality } = moduleTypes(module);
      const rows: MdNode[] = [];
      if (block.schema !== undefined) rows.push(jsx('InferredRow', [['label', 'Schema']], [codeNode('sql', block.schema)]));
      rows.push(jsx('InferredRow', [['label', 'Query']], [codeNode('sql', block.node.value)]));
      const types: MdNode[] = [];
      if (params) types.push(jsx('InferredRow', [['label', 'Params']], [codeNode('ts', formatType(params, width))]));
      types.push(jsx('InferredRow', [['label', 'Returns']], [codeNode('ts', formatType(resultType(row, cardinality), width))]));
      rows.push(jsx('InferredTypes', [], types));
      return jsx('Dialect', [['value', DIALECT_NAME[dialect]]], rows);
    });
    card.attributes = [
      { type: 'mdxJsxAttribute', name: 'dialects', value: dialects.map((d) => DIALECT_NAME[d]).join(',') },
      { type: 'mdxJsxAttribute', name: 'layout', value: INFERRED_LAYOUT },
      // A database's own page is that database's throughout: its cards need no switch or label.
      ...(DIALECT_PAGE.test(page) ? [{ type: 'mdxJsxAttribute', name: 'bare', value: null }] : []),
    ];
    card.children = [jsx('DialectTabs', [['follow', null]], panels)];
  }
}

export function remarkMapper() {
  return (tree: MdNode, file: { path?: string; data?: Record<string, unknown> }) => {
    // fumadocs-mdx hands remark plugins the loader's compiler (its own remark-include uses it):
    // with the captured modules as a dependency of the page, the dev server compiles the page again
    // after a capture, instead of keeping the error it showed while the capture was missing.
    const compiler = file.data?._compiler as { addDependency(file: string): void } | undefined;
    compiler?.addDependency(path.join(process.cwd(), 'lib', 'sql-modules.json'));
    compiler?.addDependency(path.join(process.cwd(), 'lib', 'sql-errors.json'));
    const page = !file.path ? 'the page' : path.relative(process.cwd(), file.path).startsWith('..') ? file.path : path.relative(process.cwd(), file.path);
    let blocks: Block[];
    let plans: Plan[];
    try {
      blocks = collect(tree);
      plans = plan(inPage(file.path, blocks), new Set(blocks));
      if (file.path) compiled.set(path.resolve(file.path), imports(plans));
    } catch (e) {
      throw new Error(`${page}: ${(e as Error).message}`);
    }
    const modules = plans.some((p) => p.variants.some((v) => v.needs.length)) || blocks.some((b) => b.inferred) ? readModules() : {};
    for (const { block, variants, expand } of plans) {
      if (!expand) {
        block.node.value = withSetup(block, variants[0]!, modules, page);
        continue;
      }
      // Each copy names its dialect, `// @variant:`, so the page's Markdown, which shows the block
      // once, says which dialect each type is (docs-kit's twoslash.ts).
      const panels = variants.map((variant) => jsx('Dialect', [['value', DIALECT_NAME[variant.dialect]]], [{ ...block.node, data: { ...block.node.data }, value: `// @variant: ${DIALECT_NAME[variant.dialect]}\n${withSetup(block, variant, modules, page)}` }]));
      const index = block.parent.children!.indexOf(block.node);
      block.parent.children!.splice(index, 1, jsx('DialectTabs', [['follow', null]], panels));
    }
    mergeDialectFiles(blocks);
    renderErrors(tree, blocks, page);
    try {
      renderInferred(tree, blocks, modules, page);
    } catch (e) {
      throw new Error((e as Error).message.startsWith(page) ? (e as Error).message : `${page}: ${(e as Error).message}`);
    }
    // The dialect and the file name are read; Shiki and Twoslash do not know the attributes.
    for (const { node } of blocks) if (node.meta) node.meta = node.meta.replace(/(?:^|\s)(?:dialect|file|strict)=(?:"[^"]*"|'[^']*'|\S+)/g, '').trim();
  };
}
