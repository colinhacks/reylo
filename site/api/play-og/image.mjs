// /play/og.png?q=<sql> (or ?example=<slug>), which vercel.json routes here: a shared query's link preview (lib/live/share-image.ts, bundled by vite.share.config.ts at build time), drawn with the site's fonts, which vercel.json includes.
import { playOg } from '../../.share/play-og.mjs';

const { handleImage } = playOg(new URL('../../assets/fonts/', import.meta.url));

export const GET = handleImage;
export const HEAD = handleImage;
