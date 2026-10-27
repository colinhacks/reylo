// /play/og.png?q=<sql> (or ?example=<slug>), which vercel.json routes to api/play-og/image.mjs: the link preview of a shared query (lib/live/share-card.tsx), a 1200 × 630 PNG. The page of a shared address names it (lib/live/share-page.ts); /play/og.png with neither is the static card of the example the page opens with, drawn at build time (app/routes/play-og.tsx). Bundled by vite.share.config.ts into .share/play-og.mjs; it loads Shiki, Satori and the fonts, and neither the engine nor PGlite.
import { playCard } from './share-card.tsx';
import { examples } from './share-examples.ts';
import { sharedQuery } from './share.ts';

// The card depends on its address alone, and the address carries the card's version (`v`), so the CDN and the crawlers keep it.
const KEPT = 'public, max-age=86400, s-maxage=31536000';

/** The handler, with the fonts read from `fonts`, the site's assets/fonts directory. */
export function playOg(fonts: URL) {
  const handleImage = async (request: Request): Promise<Response> => {
    if (request.method !== 'GET' && request.method !== 'HEAD') return new Response('GET\n', { status: 405, headers: { allow: 'GET, HEAD', 'cache-control': 'no-store' } });
    const shared = sharedQuery(new URL(request.url).searchParams, examples);
    if (!shared) return new Response('name the query: ?q=<sql> or ?example=<slug>\n', { status: 404, headers: { 'content-type': 'text/plain; charset=utf-8', 'cache-control': 'no-store' } });
    try {
      const card = await playCard(shared.sql, fonts, { 'content-type': 'image/png', 'cache-control': KEPT, 'x-content-type-options': 'nosniff' });
      return request.method === 'HEAD' ? new Response(null, { status: card.status, headers: card.headers }) : card;
    } catch (error) {
      console.error(error);
      return new Response('the card could not be drawn\n', { status: 500, headers: { 'content-type': 'text/plain; charset=utf-8', 'cache-control': 'no-store' } });
    }
  };
  return { handleImage };
}
