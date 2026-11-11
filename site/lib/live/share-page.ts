// /play?q=<sql> (or ?example=<slug>) asked for a page, which vercel.json routes to api/play-og/page.mjs: /play's prerendered page, its link preview naming the card of that query (/play/og.png with the same query, lib/live/share-image.ts) rather than the page's own, so that a shared query shows itself on X, Slack and the rest. Only the preview's meta tags change, in place, to the ones the page's `meta` gives in the reader's tab, so the head hydrates as served. Bundled by vite.share.config.ts into .share/play-page.mjs, with the page's HTML inside it.
import page from '../../build/client/play/index.html?raw';
import { examples } from './share-shown.ts';
import { metaContent, rewriteMeta, shareTags } from './share.ts';

// The page depends on its address alone, and a deployment replaces the CDN's copy.
const KEPT = 'public, max-age=0, s-maxage=86400';

/** The card's version, as the page's own card names it (`/play/og.png?v=<hash>`, app/routes/play.tsx). */
const version = /[?&]v=([\w-]+)/.exec(metaContent(page, 'og:image') ?? '')?.[1];

/** The page for a shared address on `origin`, or the page as it is when the address names no query. */
export function sharePage(url: URL, origin: string): string {
  const tags = shareTags(url.searchParams, examples, origin, version);
  return tags ? rewriteMeta(page, tags) : page;
}

/** The address's origin, as the reader asked for it: https, but on this machine. */
function originOf(request: Request): string {
  const url = new URL(request.url);
  const host = request.headers.get('x-forwarded-host') ?? url.host;
  return /^(localhost|127\.0\.0\.1)(:\d+)?$/.test(host) ? `${url.protocol}//${host}` : `https://${host}`;
}

export async function handlePage(request: Request): Promise<Response> {
  if (request.method !== 'GET' && request.method !== 'HEAD') return new Response('GET\n', { status: 405, headers: { allow: 'GET, HEAD', 'cache-control': 'no-store' } });
  const html = sharePage(new URL(request.url), originOf(request));
  const headers = { 'content-type': 'text/html; charset=utf-8', 'cache-control': KEPT, vary: 'Accept' };
  return new Response(request.method === 'HEAD' ? null : html, { headers });
}
