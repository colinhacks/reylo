// Gives each h2 and h3 its section as Markdown, for the heading's copy button
// (../components/heading.tsx): the text from the heading to the next heading of the same or a
// higher level, taken from the page's source, with what only the page needs removed, so the section
// pastes cleanly into an agent. It runs before a site's own remark plugins (the first of
// `docsMdxOptions`' remark plugins), so a block a site expands later, such as one copy per variant,
// is copied once, as it is written.
//
// A Twoslash query (`//    ^?`) keeps what the page shows for it, the type, written after its caret
// as Twoslash writes a query in plain text. Twoslash runs after these plugins, in the compile's
// Shiki transformer (./twoslash.ts), so `remarkQueryBlocks` names each block that has a query, the
// transformer records the block's results under that name, in memory and on disk, and
// `rehypeSections` writes each heading's Markdown again once they are known. A route that builds a
// page's Markdown later, `pageMarkdown`, reads them from the disk.

import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

type Node = {
  type: string;
  depth?: number;
  value?: string;
  position?: { start: { offset?: number }; end: { offset?: number } };
  data?: { hProperties?: Record<string, unknown> };
  properties?: Record<string, unknown>;
  children?: Node[];
};
type File = { value: unknown; data: Record<string, unknown> };

/**
 * A fenced code block, whole, from its opening fence to its closing one, and the indent both fences
 * share: a block inside a list item is indented to the item's text.
 */
const FENCE = /^([ \t]*)```[^\n]*\n[\s\S]*?^\1```$/gm;

/** A fence's meta attribute, such as `tab="Postgres"`. */
const attribute = (meta: string, name: string) => meta.match(new RegExp(`(?:^|\\s)${name}="([^"]*)"`))?.[1];

/** A Twoslash query alone on its line, `//    ^?`, with what comes before its caret. */
const QUERY = /^([ \t]*\/\/[ \t]*)\^\?[ \t]*$/;

/** A source file's frontmatter and its body. */
const FRONTMATTER = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/;

/** Where the compile leaves what each block's queries show, for a route that reads them later. */
const RESULTS = path.resolve('node_modules/.cache/docs-kit/queries');

/** What each block's queries show, by the block's id, and under it by variant ('' for a block written once). */
type Results = Record<string, string[]>;
const recorded = new Map<string, Results>();

/**
 * A code block's id: its file's text without the frontmatter (which the compile does not see), the
 * block's own text, and how many identical blocks come before it in the file. A route that reads the
 * file finds the same id, and an edit to the file gives each of its blocks a new one, so no result
 * outlives the text it was made from.
 */
export function blockId(file: string, offset: number, block: string): string {
  let before = 0;
  for (let at = file.indexOf(block); at !== -1 && at < offset; at = file.indexOf(block, at + 1)) before++;
  const body = file.replace(FRONTMATTER, '$2').trim();
  return createHash('sha256').update(body).update('\0').update(block).update('\0').update(String(before)).digest('hex').slice(0, 16);
}

/** Records what one variant of a block shows for each of its queries, in order (./twoslash.ts). */
export function recordQueries(id: string, variant: string, texts: string[]): void {
  const results = { ...recorded.get(id), [variant]: texts };
  recorded.set(id, results);
  mkdirSync(RESULTS, { recursive: true });
  writeFileSync(path.join(RESULTS, `${id}.json`), JSON.stringify(results));
}

function resultsOf(id: string): Results | undefined {
  const hit = recorded.get(id);
  if (hit) return hit;
  const file = path.join(RESULTS, `${id}.json`);
  return existsSync(file) ? (JSON.parse(readFileSync(file, 'utf8')) as Results) : undefined;
}

/** A block that Twoslash checks and that holds a query. */
const isQueried = (block: string) => /^```tsx?\b[^\n]*\btwoslash\b/.test(block) && block.split('\n').some((line) => QUERY.test(line));

/**
 * A query's line with what it shows after the caret and the result's further lines under it, as
 * Twoslash prints a query in plain text. When a block's variants (one copy per variant, each named by
 * a `// @variant:` line) show different results, each is written, under the names that show it.
 */
function answered(lead: string, shown: [variant: string, text: string][]): string {
  const indent = lead.match(/^[ \t]*/)![0];
  const under = `${indent}//${' '.repeat(lead.length - indent.length + 1)}`;
  const lines = shown.flatMap(([variant, text]) => (shown.length > 1 && variant ? `${variant}: ${text}` : text).split('\n'));
  return [`${lead}^? ${lines[0]}`, ...lines.slice(1).map((line) => `${under}${line}`)].join('\n');
}

/**
 * One fenced block as an agent should read it: the language alone on the fence, a label above it
 * for its tab and file name, and none of Twoslash's setup, directives and completions or the diff
 * markers; a query keeps what it shows when the compile recorded it under the block's `id`.
 */
function cleanBlock(block: string, id?: string): string {
  const newline = block.indexOf('\n');
  const [, lang = '', meta = ''] = block.slice(0, newline).match(/^```(\S*)\s*(.*)$/) ?? [];
  let body = block.slice(newline + 1, block.lastIndexOf('```'));
  // The setup a Twoslash block hides: everything above `---cut---` (or `---cut-before---`) and
  // everything below `---cut-after---`.
  body = body.replace(/^[\s\S]*?^\/\/ ---cut(?:-before)?---\n/m, '').replace(/^\/\/ ---cut-after---\n[\s\S]*$/m, '');
  // Each query's result, when every variant recorded one for each query of the block.
  const results = id ? Object.entries(resultsOf(id) ?? {}) : [];
  const queries = body.split('\n').filter((line) => QUERY.test(line)).length;
  if (results.length && results.every(([, texts]) => texts.length === queries)) {
    let index = 0;
    body = body
      .split('\n')
      .map((line) => {
        const lead = line.match(QUERY)?.[1];
        if (lead === undefined) return line;
        const i = index++;
        const shown = new Map<string, string[]>();
        for (const [variant, texts] of results) shown.set(texts[i]!, [...(shown.get(texts[i]!) ?? []), variant]);
        return answered(lead, [...shown].map(([text, variants]) => [variants.filter(Boolean).join(', '), text]));
      })
      .join('\n');
  }
  body = body
    // Twoslash's directives (`// @noErrors`, `// @hover: …`), queries it has no result for
    // (`//    ^?`) and completions (`^|`).
    .replace(/^[ \t]*\/\/ @\w+.*\n/gm, '')
    .replace(/^[ \t]*\/\/\s*\^[?|][ \t]*\n/gm, '')
    // Shiki's diff and highlight markers.
    .replace(/[ \t]*\/\/ \[!code [^\]]*\]/g, '');
  // A tab that names a variant is bold; a file name, as a tab or a title, is code.
  const tab = attribute(meta, 'tab');
  const title = attribute(meta, 'title');
  const name = (text: string) => (/\.\w+$/.test(text) ? `\`${text}\`` : `**${text}**`);
  const parts = [tab, title].filter((part, i, all): part is string => !!part && all.indexOf(part) === i);
  const label = parts.length ? `${parts.map(name).join(', ')}:\n\n` : '';
  return `${label}\`\`\`${lang}\n${body}\`\`\``;
}

/** The prose between code blocks, with the page's components as plain Markdown. */
function cleanProse(text: string): string {
  return (
    text
      // MDX comments.
      .replace(/\{\/\*[\s\S]*?\*\/\}\n?/g, '')
      // A heading's custom id (`### Name [#id]`), which only the HTML anchor reads.
      .replace(/^(#{1,6}[ \t].*?)[ \t]+\[#[^\]\s]+\][ \t]*$/gm, '$1')
      // A callout as a quote, its title in bold.
      .replace(/<Callout[^>]*?title="([^"]*)"[^>]*>\s*([\s\S]*?)\s*<\/Callout>/g, (_, title: string, body: string) => `> **${title}.** ${body.replace(/\n/g, '\n> ')}`)
      .replace(/<Callout[^>]*>\s*([\s\S]*?)\s*<\/Callout>/g, (_, body: string) => `> ${body.replace(/\n/g, '\n> ')}`)
      // A component's opening tag alone on its line that names a value, a title or a summary (a tab,
      // a variant's panel, an accordion item, a `<Details>`) becomes that label in bold; any other
      // line holding only a component's tag (a tab group, a figure, a card, or an empty panel such
      // as `<Variant value="…" />`, which holds nothing) goes.
      .replace(/^[ \t]*<[A-Z]\w*\b[^>]*?\s(?:value|title|summary)="([^"]*)"[^>]*(?<!\/)>[ \t]*$/gm, '**$1**')
      .replace(/^[ \t]*<\/?[A-Z]\w*\b[^>]*>[ \t]*\n?/gm, '')
      // HTML written for its class: a `<div>` line goes, a `<p>` is its text.
      .replace(/^[ \t]*<\/?div\b[^>]*>[ \t]*\n?/gm, '')
      .replace(/<p\b[^>]*>([\s\S]*?)<\/p>/g, '$1')
  );
}

/**
 * A page's source, or a part of one, as Markdown for an agent. Given the whole file the part comes
 * from and where in it the part starts, each query keeps the result the compile recorded for it.
 */
export function toAgentMarkdown(source: string, file?: { text: string; offset: number }): string {
  let out = '';
  let last = 0;
  for (const match of source.matchAll(FENCE)) {
    // An indented block is cleaned without its indent and written back under it. Its id is taken
    // from its opening backticks, where the block's node starts for `remarkQueryBlocks`.
    const indent = match[1]!;
    const block = match[0].slice(indent.length);
    const start = match.index + indent.length;
    const id = file && isQueried(block) ? blockId(file.text, file.offset + start, block) : undefined;
    const lines = cleanBlock(block.replaceAll(`\n${indent}`, '\n'), id).split('\n');
    out += cleanProse(source.slice(last, match.index)) + lines.map((line) => (line ? indent + line : line)).join('\n');
    last = match.index + match[0].length;
  }
  out += cleanProse(source.slice(last));
  return (
    out
      // A terminal block's colours are escape codes, which an agent reads as noise.
      .replace(/\x1b\[[0-9;]*m/g, '')
      .replace(/\n{3,}/g, '\n\n')
      .trim() + '\n'
  );
}

/** Each h2 and h3 with its section's Markdown, `data-section`, and the section's place for `rehypeSections`. */
export function remarkSections() {
  return (tree: Node, file: File) => {
    const source = String(file.value);
    const headings = (tree.children ?? []).filter((node) => node.type === 'heading' && node.position?.start.offset !== undefined);
    const sections: [start: number, end: number][] = [];
    headings.forEach((heading, i) => {
      if (heading.depth !== 2 && heading.depth !== 3) return;
      const next = headings.slice(i + 1).find((other) => other.depth! <= heading.depth!);
      const start = heading.position!.start.offset!;
      const end = next?.position!.start.offset ?? source.length;
      heading.data ??= {};
      heading.data.hProperties = {
        ...heading.data.hProperties,
        'data-section': toAgentMarkdown(source.slice(start, end), { text: source, offset: start }),
        'data-section-index': sections.push([start, end]) - 1,
      };
    });
    file.data.docsKitSections = sections;
  };
}

/** The code nodes of a tree, in order. */
function codeNodes(node: Node, out: Node[] = []): Node[] {
  if (node.type === 'code') out.push(node);
  for (const child of node.children ?? []) codeNodes(child, out);
  return out;
}

/**
 * Names each `ts twoslash` block that has a query with a first line, `// @docs-block: <id>`
 * (`blockId`), which docs-kit's Twoslash transformer takes out before Twoslash runs and under which
 * it records what each query shows. It runs before a site's own remark plugins, so a block a site
 * copies, once per variant, keeps the name of the block as it is written.
 */
export function remarkQueryBlocks() {
  return (tree: Node, file: File) => {
    const source = String(file.value);
    for (const node of codeNodes(tree)) {
      const start = node.position?.start.offset;
      const end = node.position?.end.offset;
      if (start === undefined || end === undefined) continue;
      const block = source.slice(start, end);
      if (isQueried(block)) node.value = `// @docs-block: ${blockId(source, start, block)}\n${node.value}`;
    }
  };
}

/** The elements of a tree that carry a property, in order. */
function withProperty(node: Node, name: string, out: Node[] = []): Node[] {
  if (node.properties && name in node.properties) out.push(node);
  for (const child of node.children ?? []) withProperty(child, name, out);
  return out;
}

/**
 * Each heading's section Markdown again, after Twoslash has run in Shiki and recorded what the
 * section's queries show, so the copy button pastes each type the page shows; it runs after
 * Fumadocs' rehypeCode.
 */
export function rehypeSections() {
  return (tree: Node, file: File) => {
    const source = String(file.value);
    const sections = (file.data.docsKitSections ?? []) as [number, number][];
    for (const heading of withProperty(tree, 'data-section-index')) {
      const [start, end] = sections[Number(heading.properties!['data-section-index'])] ?? [];
      if (start !== undefined) heading.properties!['data-section'] = toAgentMarkdown(source.slice(start, end), { text: source, offset: start });
      delete heading.properties!['data-section-index'];
    }
  };
}

/**
 * A page as Markdown, from its source files in the order the page shows them (the first holds the
 * frontmatter): its title as the h1, its description as a quote, then every file read as each
 * heading's copy button reads its section, each query with what the page shows for it, with each
 * link made absolute, a link to a section (`#…`) to the page's address and a link to another page
 * (`/…`) to the site's. A site serves it as the page's own Markdown, to an agent that asks for
 * Markdown (README, "Markdown for agents"), and joins its pages' for `/llms.txt`. The query results
 * are the ones the site's last compile recorded, so the route runs after the page compiles, as a
 * route rendered at build time does.
 */
export function pageMarkdown(sources: readonly string[], { site, page = '/' }: { site: string; page?: string }): string {
  const [, frontmatter = ''] = (sources[0] ?? '').match(FRONTMATTER) ?? [];
  const field = (name: string) => frontmatter.match(new RegExp(`^${name}: (.*)$`, 'm'))?.[1] ?? '';
  const parts = sources.map((text) => {
    const body = text.match(FRONTMATTER)?.[2] ?? text;
    return toAgentMarkdown(body, { text, offset: text.length - body.length }).trim();
  });
  const address = `${site}${page}`;
  const text = parts.filter(Boolean).join('\n\n').replaceAll('](#', `](${address}#`).replaceAll('](/', `](${site}/`);
  const description = field('description');
  return `# ${field('title')}\n\n${description ? `> ${description}\n\n` : ''}${text.trim()}\n`;
}
