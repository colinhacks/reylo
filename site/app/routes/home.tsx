import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import type { Route } from './+types/home';
import { HomeLanding } from '@/components/home-landing';
import { siteTitle, siteUrl } from '@/lib/layout.shared';
import { movedAnchorsScript } from '@/lib/moved-anchors';
import { ogAlt, ogSize } from '@/lib/og';
import { source } from '@/lib/source';
import { fetchStars } from '@/lib/stars';
import { structuredData } from '@/lib/structured-data';

/**
 * Reylo's homepage, at / (components/home-landing.tsx); the docs open at /docs. Run at build time,
 * when the page is rendered to a file: the intro's description, for the metadata; the hash of the
 * link preview's route, which its URL carries, so that a change to the card changes the URL and the
 * caches of X and Slack fetch the new card; and Reylo's star count, for the nav's GitHub pill, or none
 * when GitHub can't be reached.
 */
export async function loader() {
  const description = source.getPage([])?.data.description ?? '';
  const og = createHash('sha256').update(await readFile('app/routes/opengraph-image.tsx')).digest('hex').slice(0, 16);
  return { description, og, stars: await fetchStars() };
}

/** The site's metadata: its title, the intro's description, and the link previews, whose image is app/routes/opengraph-image.tsx. */
export function meta({ loaderData }: Route.MetaArgs): Route.MetaDescriptors {
  if (!loaderData) return [];
  const { description } = loaderData;
  const image = `${siteUrl}/opengraph-image.png?${loaderData.og}`;
  return [
    { title: siteTitle },
    { name: 'description', content: description },
    { name: 'application-name', content: 'Reylo' },
    { name: 'author', content: 'Colin McDonnell' },
    { name: 'creator', content: 'Colin McDonnell' },
    { name: 'robots', content: 'index, follow' },
    { name: 'googlebot', content: 'index, follow, max-image-preview:large, max-snippet:-1' },
    { tagName: 'link', rel: 'canonical', href: siteUrl },
    { property: 'og:title', content: siteTitle },
    { property: 'og:description', content: description },
    { property: 'og:url', content: siteUrl },
    { property: 'og:site_name', content: 'Reylo' },
    { property: 'og:locale', content: 'en_US' },
    { property: 'og:image', content: image },
    { property: 'og:image:type', content: 'image/png' },
    { property: 'og:image:width', content: String(ogSize.width) },
    { property: 'og:image:height', content: String(ogSize.height) },
    { property: 'og:image:alt', content: ogAlt },
    { property: 'og:type', content: 'website' },
    { name: 'twitter:card', content: 'summary_large_image' },
    { name: 'twitter:creator', content: '@colinhacks' },
    { name: 'twitter:title', content: siteTitle },
    { name: 'twitter:description', content: description },
    { name: 'twitter:image', content: image },
    { name: 'twitter:image:alt', content: ogAlt },
    { name: 'twitter:image:type', content: 'image/png' },
    { name: 'twitter:image:width', content: String(ogSize.width) },
    { name: 'twitter:image:height', content: String(ogSize.height) },
  ];
}

/**
 * The page, after two scripts: an old link to a section of the one-page docs, which lived here, finds
 * its page (lib/moved-anchors.ts), and the structured data describes the site and the project.
 */
export default function Home({ loaderData }: Route.ComponentProps) {
  return (
    <>
      <script dangerouslySetInnerHTML={{ __html: movedAnchorsScript }} />
      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: structuredData(loaderData.description) }} />
      <HomeLanding stars={loaderData.stars} />
    </>
  );
}
