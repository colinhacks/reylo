import { apiMarkdown, REYLO_API } from '@/lib/live/api-docs';
import { markdownOf } from '@/lib/markdown';

// Every page as Markdown, for a coding agent or a language model (https://llmstxt.org), in the
// sidebar's order, then the guide to moving from Prisma, each as its own Markdown file holds it
// (lib/markdown.ts), then the playground's API (/play/api.md). Written to build/client/llms.txt at
// build time.
const PAGES = ['', 'usage', 'inference', 'configuration', 'observability', 'schema-changes', 'analyze', 'typedefs', 'compare', 'faq', 'postgres', 'mysql', 'sqlite', 'duckdb', 'prisma'];

export async function loader() {
  const { playData } = await import('@/lib/live/play-data');
  const api = apiMarkdown(REYLO_API, (await playData()).apiAnswer);
  const text = [...(await Promise.all(PAGES.map(markdownOf))), api].join('\n');
  return new Response(text, { headers: { 'Content-Type': 'text/markdown; charset=utf-8' } });
}
