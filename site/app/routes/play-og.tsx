// /play's own link preview, written to build/client/play/og.png at build time: the card of the example the page opens with (lib/live/share-card.tsx), which app/routes/play.tsx names as og:image with the card's version in its address. A shared query's card, /play/og.png?q=<sql>, is drawn on request by the same module (lib/live/share-image.ts).
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';
import { playCard } from '@/lib/live/share-card';
import { startSql } from '@/lib/live/share-examples';

export async function loader() {
  if (startSql === undefined) throw new Error('app/routes/play-og.tsx: no example for /play to open with');
  return playCard(startSql, pathToFileURL(join(process.cwd(), 'assets/fonts/')));
}
