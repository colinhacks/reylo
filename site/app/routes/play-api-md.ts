import { apiMarkdown, REYLO_API } from '@/lib/live/api-docs';

// /play/api.md: the playground's API as Markdown, for an agent (lib/live/api-docs.ts), its example's answer the one the build analyzed (lib/live/play-data.ts). Written to build/client at build time; vercel.json answers /play asked for Markdown with it, and /llms.txt ends with it.
export async function loader() {
  const { playData } = await import('@/lib/live/play-data');
  const { apiAnswer } = await playData();
  return new Response(apiMarkdown(REYLO_API, apiAnswer), { headers: { 'Content-Type': 'text/markdown; charset=utf-8' } });
}
