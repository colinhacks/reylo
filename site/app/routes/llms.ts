import { apiMarkdown, REYLO_API } from '@/lib/live/api-docs';
import { SHOW_RUST } from '@/lib/languages';
import { markdownOf } from '@/lib/markdown';

// Every page as Markdown, for a coding agent or a language model (https://llmstxt.org), in the
// sidebar's order, each as its own Markdown file holds it
// (lib/markdown.ts), then the playground's API (/play/api.md). Written to build/client/llms.txt at
// build time. The Rust page only while the docs show Rust (lib/languages.ts).
const PAGES = ['', 'typescript', ...(SHOW_RUST ? ['rust'] : []), 'inference', 'migrate', 'compare', 'cli', 'faq', 'd1', 'durable-objects', 'hyperdrive', 'nextjs', 'neon', 'supabase', 'planetscale', 'turso', 'deployment', 'prisma', 'configuration', 'postgres', 'mysql', 'sqlite', 'sqlserver'];

export async function loader() {
  const { playData } = await import('@/lib/live/play-data');
  const api = apiMarkdown(REYLO_API, (await playData()).apiAnswer);
  const text = [...(await Promise.all(PAGES.map(markdownOf))), api].join('\n');
  return new Response(text, { headers: { 'Content-Type': 'text/markdown; charset=utf-8' } });
}
