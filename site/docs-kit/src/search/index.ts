// The search index of a site with several pages, built when the site is built and written to a
// file the search dialog loads the first time it opens (../components/search.tsx, README, "Search").
//
// A page's index is its source read as each heading's copy button reads it (`toAgentMarkdown`):
// the page's components as plain Markdown, so a callout, a tab's label and a collapsible's summary
// are searched as the text they show. The page is then cut at its headings, and each section into
// the things a reader would land on: a paragraph, a list item, a table's row, a code block.

import { toAgentMarkdown } from '../mdx/remark-sections.ts';
import type { SearchIndex, SearchSection } from './engine.ts';

export type { SearchIndex, SearchPage, SearchSection } from './engine.ts';

/** A page to index. */
export type SearchSource = {
  url: string;
  title: string;
  description?: string;
  /** The page's source file, or its files in the order the page shows them. Frontmatter is ignored. */
  source: string | readonly string[];
  /** The page's table of contents as Fumadocs builds it (`page.data.toc`): every heading's anchor, in order. */
  toc: readonly { url: string; depth: number }[];
};

const FRONTMATTER = /^---\n[\s\S]*?\n---\n/;

/**
 * Markdown's inline syntax taken off, except the backticks around code, which the dialog draws as
 * code: a link is its text, an image goes, emphasis loses its marks, an HTML tag goes.
 */
function inline(text: string): string {
  return text
    .split(/(`+[^`]*`+)/)
    .map((part, i) => {
      // A code span, with one backtick each side whatever the source wrote.
      if (i % 2 === 1) return `\`${part.replace(/^`+\s?|\s?`+$/g, '')}\``;
      return part
        .replace(/!\[[^\]]*\]\([^)]*\)/g, '')
        .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
        .replace(/<\/?[a-zA-Z][^>]*>/g, '')
        .replace(/(\*\*|__)(?=\S)(.+?)(?<=\S)\1/g, '$2')
        .replace(/(^|[^\w*])\*(?=\S)([^*]+?)(?<=\S)\*(?!\w)/g, '$1$2')
        .replace(/(^|[^\w_])_(?=\S)([^_]+?)(?<=\S)_(?!\w)/g, '$1$2')
        .replace(/\\([\\`*_{}[\]()#+\-.!|<>])/g, '$1')
        .replace(/&amp;/g, '&')
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')
        .replace(/&nbsp;/g, ' ');
    })
    .join('')
    .replace(/\s+/g, ' ')
    .trim();
}

/** A table row's cells, as one line: `Include draft PRs · Off · Review draft PRs`. */
function row(line: string): string {
  const cells = line
    .replace(/^\s*\||\|\s*$/g, '')
    .split(/(?<!\\)\|/)
    .map((cell) => inline(cell.replace(/\\\|/g, '|')))
    .filter(Boolean);
  return cells.join(' · ');
}

const LIST_ITEM = /^\s*(?:[-*+]|\d+[.)])\s+/;

/** One page's sections. Throws when the page's headings and its table of contents disagree. */
function sectionsOf(page: SearchSource): SearchSection[] {
  const files = typeof page.source === 'string' ? [page.source] : page.source;
  const markdown = files.map((text) => toAgentMarkdown(text.replace(FRONTMATTER, ''))).join('\n\n');
  const sections: SearchSection[] = [{ text: [] }];
  let section = sections[0]!;
  let headings = 0;
  let paragraph: string[] = [];
  let item: string[] | undefined;
  let table = 0;
  const flush = () => {
    for (const lines of [paragraph, item ?? []]) {
      const text = inline(lines.join(' '));
      if (text) section.text.push(text);
    }
    paragraph = [];
    item = undefined;
  };

  const lines = markdown.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;
    const fence = line.match(/^\s*(`{3,})/);
    if (fence) {
      flush();
      const body: string[] = [];
      for (i++; i < lines.length && !lines[i]!.trimStart().startsWith(fence[1]!); i++) body.push(lines[i]!);
      const code = body.join('\n').trim();
      if (code) (section.code ??= []).push(code);
      continue;
    }
    if (!line.startsWith('|')) table = 0;
    const heading = line.match(/^(#{1,6})\s+(.*?)\s*#*\s*$/);
    if (heading) {
      flush();
      const entry = page.toc[headings++];
      if (!entry) throw new Error(`docs-kit search: ${page.url} has more headings than its table of contents (at "${heading[2]}")`);
      section = { id: entry.url.replace(/^#/, ''), title: inline(heading[2]!), depth: heading[1]!.length, text: [] };
      sections.push(section);
    } else if (line.trim() === '') {
      flush();
    } else if (line.startsWith('|')) {
      flush();
      // A table's first row names its columns and its second rules them off; the rest are its rows.
      if (++table > 2) {
        const text = row(line);
        if (text) section.text.push(text);
      }
    } else if (LIST_ITEM.test(line)) {
      flush();
      item = [line.replace(LIST_ITEM, '')];
    } else if (item) {
      item.push(line.trim());
    } else {
      // A quote (a callout) reads as a paragraph.
      paragraph.push(line.replace(/^\s*>\s?/, ''));
    }
  }
  flush();
  if (headings !== page.toc.length) throw new Error(`docs-kit search: ${page.url} has ${headings} headings and its table of contents ${page.toc.length}`);
  return sections.filter((entry, i) => i === 0 || entry.title);
}

/** The site's search index, its pages in the order given: the order results fall back to. */
export function searchIndex(pages: readonly SearchSource[]): SearchIndex {
  return {
    version: 1,
    pages: pages.map((page) => ({
      url: page.url,
      title: page.title,
      ...(page.description ? { description: page.description } : {}),
      sections: sectionsOf(page),
    })),
  };
}
