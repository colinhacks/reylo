import { apiMarkdown, REYLO_API } from '@/lib/live/api-docs';
import { markdownOf } from '@/lib/markdown';

// Every page as Markdown, for a coding agent or a language model (https://llmstxt.org): the main
// page, then each dialect's reference, in the order a reader takes them, each as its own Markdown
// file holds it (lib/markdown.ts), then the playground's API (/play/api.md). Written to
// build/client/llms.txt at build time.
const PAGES = ['', 'postgres', 'mysql', 'sqlite', 'duckdb'];

export async function loader() {
  const { playData } = await import('@/lib/live/play-data');
  const api = apiMarkdown(REYLO_API, (await playData()).apiAnswer);
  const text = [...(await Promise.all(PAGES.map(markdownOf))), api].join('\n');
  return new Response(text, { headers: { 'Content-Type': 'text/markdown; charset=utf-8' } });
}
