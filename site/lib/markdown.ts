import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pageMarkdown } from 'docs-kit/remark-sections';
import { expandCompare } from '@/lib/compare';
import { siteUrl } from '@/lib/layout.shared';
import { expandInferred } from '@/lib/inferred-markdown';
import { PAGE_SECTIONS } from '@/lib/page-sections';
import { expandErrors } from '@/lib/sql-errors';

/** Each `<Step>` as a numbered list item, its name in bold and where it acts after it. */
function expandSteps(text: string): string {
  return text.replace(/<Steps>\n([\s\S]*?)<\/Steps>\n/g, (_, steps: string) => {
    let n = 0;
    return steps.replace(/<Step title="([^"]+)"(?: where="([^"]+)")?>\n+([\s\S]*?)\n+<\/Step>\n/g, (_m, title: string, where: string | undefined, body: string) => `${++n}. **${title}**${where ? ` (${where})` : ''}: ${body}\n`);
  });
}

/** Each `<Feature>` card as a list item, its name in bold, and each `<Choice>` card as one naming its two ways: what a card says, as an agent reads a list. */
function expandFeatures(text: string): string {
  const choice = (_: string, title: string, ways: string) => {
    const each = [...ways.matchAll(/<Way title="([^"]+)"( default)?>\n+([\s\S]*?)\n+<\/Way>\n/g)].map(([, name, isDefault, said], k) => `${k === 0 ? name[0]!.toUpperCase() + name.slice(1) : `Or ${name}`}${isDefault ? ' (the default)' : ''}: ${said}`);
    return `- **${title}.** ${each.join(' ')}\n`;
  };
  return text.replace(/<Features>\n([\s\S]*?)<\/Features>\n/g, (_, cards: string) =>
    cards.replace(/<Choice icon="\w+" title="([^"]+)">\n([\s\S]*?)<\/Choice>\n/g, choice).replace(/<Feature icon="\w+" title="([^"]+)">\n+([\s\S]*?)\n+<\/Feature>\n/g, '- **$1.** $2\n'),
  );
}

/**
 * A page of content/docs as Markdown, by its slug ('' for the main page): its files in order (a
 * page's sections are lib/page-sections.ts'), read as each heading's copy button reads its section, under
 * its title and description, with each link made absolute (docs-kit's pageMarkdown). Each page's
 * is written to build/client/<slug>.md (index.md for the main page) by app/routes/markdown.ts, which
 * vercel.json serves at the page's own address to an agent that asks for Markdown, and
 * /llms.txt is all of them (app/routes/llms.ts).
 */
export async function markdownOf(slug: string): Promise<string> {
  const own = `${slug || 'index'}.mdx`;
  const files = [path.resolve('content/docs', own), ...(PAGE_SECTIONS[own] ?? []).map((name) => path.resolve('content/sections', name))];
  // Each `<Inferred>` card with the types the page shows for it (lib/inferred-markdown.ts), and each
  // error example with its errors (lib/sql-errors.ts), and the comparison's marks as a table (lib/compare.ts).
  const sources = (await Promise.all(files.map((file) => readFile(file, 'utf8')))).map((text) => expandSteps(expandFeatures(expandCompare(expandErrors(expandInferred(text))))));
  return pageMarkdown(sources, { site: siteUrl, page: slug ? `/${slug}` : '/' });
}
