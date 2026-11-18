import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pageMarkdown } from 'docs-kit/remark-sections';
import { expandCompare } from '@/lib/compare';
import { siteUrl } from '@/lib/layout.shared';
import { expandInferenceCards } from '@/lib/inference-card-markdown';
import { PAGE_SECTIONS } from '@/lib/page-sections';
import { rustContent } from '@/lib/rust-content';
import { expandErrors } from '@/lib/sql-errors';

/** Each `<Step>` as a numbered list item, its name in bold and where it acts after it. */
function expandSteps(text: string): string {
  return text.replace(/<Steps>\n([\s\S]*?)<\/Steps>\n/g, (_, steps: string) => {
    let n = 0;
    return steps.replace(/<Step title="([^"]+)"(?: where="([^"]+)")?>\n+([\s\S]*?)\n+<\/Step>\n/g, (_m, title: string, where: string | undefined, body: string) => `${++n}. **${title}**${where ? ` (${where})` : ''}: ${body}\n`);
  });
}

/**
 * A page of content/docs as Markdown, by its slug ('' for the main page): its files in order (a
 * page's sections are lib/page-sections.ts'), read as each heading's copy button reads its section, under
 * its title and description, with each link made absolute (docs-kit's pageMarkdown). Each page's
 * is written to build/client/<slug>.md (index.md for the main page) by app/routes/markdown.ts, which
 * vercel.json serves at the page's own address to an agent that asks for Markdown, and
 * /llms.txt is all of them (app/routes/llms.ts). Each file is read as the site builds it, its Rust
 * content left out while the docs leave Rust out (lib/rust-content.ts).
 */
export async function markdownOf(slug: string): Promise<string> {
  const own = `${slug || 'index'}.mdx`;
  const files = [path.resolve('content/docs', own), ...(PAGE_SECTIONS[own] ?? []).map((name) => path.resolve('content/sections', name))];
  // Each inference card with the types the page shows for it (lib/inference-card-markdown.ts), and each
  // error example with its errors (lib/sql-errors.ts), and the comparison's marks as a table (lib/compare.ts).
  const sources = (await Promise.all(files.map((file) => readFile(file, 'utf8')))).map((text) => expandSteps(expandCompare(expandErrors(expandInferenceCards(rustContent(text))))));
  return pageMarkdown(sources, { site: siteUrl, page: slug ? `/${slug}` : '/' });
}
