import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pageMarkdown } from 'docs-kit/remark-sections';
import { expandCompare } from '@/lib/compare';
import { siteUrl } from '@/lib/layout.shared';
import { expandInferred } from '@/lib/inferred-markdown';
import { pageFiles } from '@/lib/page-files';
import { expandErrors } from '@/lib/sql-errors';

/**
 * A page of content/docs as Markdown, by its slug ('' for the main page): its files in order (the
 * main page's are lib/page-files.ts'), read as each heading's copy button reads its section, under
 * its title and description, with each link made absolute (docs-kit's pageMarkdown). Each page's
 * is written to build/client/<slug>.md (index.md for the main page) by app/routes/markdown.ts, which
 * vercel.json serves at the page's own address to an agent that asks for Markdown, and
 * /llms.txt is all of them (app/routes/llms.ts).
 */
export async function markdownOf(slug: string): Promise<string> {
  const files = slug ? [path.resolve('content/docs', `${slug}.mdx`)] : pageFiles();
  // Each `<Inferred>` card with the types the page shows for it (lib/inferred-markdown.ts), and each
  // error example with its errors (lib/sql-errors.ts), and the comparison's marks as a table (lib/compare.ts).
  const sources = (await Promise.all(files.map((file) => readFile(file, 'utf8')))).map((text) => expandCompare(expandErrors(expandInferred(text))));
  return pageMarkdown(sources, { site: siteUrl, page: slug ? `/${slug}` : '/' });
}
