import type { Route } from './+types/play';
import { Playground } from '@/components/play';
import { siteUrl } from '@/lib/layout.shared';

// The playground, at /play (components/play.tsx): drawn at build time with the main page's
// Postgres schema and the types of the statement it opens with (lib/live/play-data.ts), then
// live in the reader's tab.
export async function loader() {
  const { playData } = await import('@/lib/live/play-data');
  return playData();
}

const description = 'Edit a SQL query against a sample Postgres schema and see the types Reylo infers for it, with Postgres running in your browser.';

export function meta(): Route.MetaDescriptors {
  const title = 'Reylo playground: a query and its types, live';
  return [
    { title },
    { name: 'description', content: description },
    { tagName: 'link', rel: 'canonical', href: `${siteUrl}/play` },
    { property: 'og:title', content: title },
    { property: 'og:description', content: description },
    { property: 'og:url', content: `${siteUrl}/play` },
  ];
}

export default function Play({ loaderData }: Route.ComponentProps) {
  return <Playground data={loaderData} />;
}
