import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import type { Route } from './+types/play';
import { Playground } from '@/components/play';
import { siteUrl } from '@/lib/layout.shared';
import { cardAlt, cardSize, shareTags } from '@/lib/live/share';

// The playground, at /play (components/play.tsx): drawn at build time with the main page's
// Postgres schema and the types of the statement it opens with (lib/live/play-data.ts), then
// live in the reader's tab. Its link preview is the card of that statement (app/routes/play-og.tsx),
// whose address carries a hash of the card's module, so that a change to the card is a new address
// to the caches of X and Slack. A shared address, /play?q=<sql>, is served by a function that names
// the card of its own query in these same tags (lib/live/share-page.ts).
export async function loader() {
  const { playData } = await import('@/lib/live/play-data');
  const card = await readFile('lib/live/share-card.tsx');
  return { ...(await playData()), og: createHash('sha256').update(card).digest('hex').slice(0, 16) };
}

const description = 'Edit a SQL query against a sample Postgres schema and see the types Reylo infers for it, with Postgres running in your browser.';

export function meta({ loaderData, location }: Route.MetaArgs): Route.MetaDescriptors {
  const title = 'Reylo playground: a query and its types, live';
  const examples = new Map(loaderData?.examples.map((e) => [e.slug, e.sql]));
  // A shared query's own card, on the address the reader opened, as its page was served; else the
  // card of the example the page opens with.
  const origin = typeof window === 'undefined' ? siteUrl : window.location.origin;
  const tags = shareTags(new URLSearchParams(location.search), examples, origin, loaderData?.og) ?? {
    'og:url': `${siteUrl}/play`,
    'og:image': `${siteUrl}/play/og.png?v=${loaderData?.og ?? ''}`,
    'og:image:alt': cardAlt(examples.get(loaderData?.start ?? '') ?? ''),
  };
  const image = tags['og:image'];
  const alt = tags['og:image:alt'];
  return [
    { title },
    { name: 'description', content: description },
    { tagName: 'link', rel: 'canonical', href: `${siteUrl}/play` },
    { property: 'og:title', content: title },
    { property: 'og:description', content: description },
    { property: 'og:url', content: tags['og:url'] },
    { property: 'og:site_name', content: 'Reylo' },
    { property: 'og:type', content: 'website' },
    { property: 'og:image', content: image },
    { property: 'og:image:type', content: 'image/png' },
    { property: 'og:image:width', content: String(cardSize.width) },
    { property: 'og:image:height', content: String(cardSize.height) },
    { property: 'og:image:alt', content: alt },
    { name: 'twitter:card', content: 'summary_large_image' },
    { name: 'twitter:title', content: title },
    { name: 'twitter:description', content: description },
    { name: 'twitter:image', content: image },
    { name: 'twitter:image:alt', content: alt },
  ];
}

export default function Play({ loaderData }: Route.ComponentProps) {
  return <Playground data={loaderData} />;
}
