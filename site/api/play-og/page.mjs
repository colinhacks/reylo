// /play?q=<sql> (or ?example=<slug>) asked for a page, which vercel.json routes here: /play with
// its link preview naming that query's card (lib/live/share-page.ts, bundled by vite.api.config.ts
// at build time, the page's HTML inside it).
import { handlePage } from '../../.live/api/play-page.mjs';

export const GET = handlePage;
export const HEAD = handlePage;
