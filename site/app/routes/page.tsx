import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { use, useMemo } from 'react';
import { DocsShell } from 'docs-kit/shell';
import type { Root } from 'fumadocs-core/page-tree';
import type { Route } from './+types/page';
import { Brand } from '@/components/brand';
import { HeroTitle } from '@/components/hero';
import { getMDXComponents } from '@/components/mdx';
import { loadPage } from '@/lib/content';
import { githubUrl, markdownPath, pageTitle, siteTitle, siteUrl } from '@/lib/layout.shared';
import { ogAlt, ogSize } from '@/lib/og';
import { source } from '@/lib/source';

/**
 * The h2s whose h3s stay out of each page's table of contents, so that the whole table fits
 * on one screen (a window 900px tall, at Fumadocs' spacing); the headings stay on the page,
 * with their anchors. On the main page, every h3 shows: the query's members
 * under Usage and the keys of `.reylo.yaml`. On each dialect page (content/docs/<dialect>.mdx, one skeleton
 * for all three): the h3s of Parameters and of Drivers, one per driver, which leaves Data types'
 * h3s, the reason a reader lands there, in the table.
 */
const FOLD: Record<string, readonly string[]> = {
  '': [],
  postgres: ['#parameters', '#drivers'],
  mysql: ['#parameters', '#drivers'],
  sqlite: ['#parameters', '#drivers'],
};

/**
 * Run at build time only, when each page is rendered to a file (react-router.config.ts): the page
 * as the meta tags and the shell need it, without its content, which the component loads itself
 * (lib/content.ts). The link preview's URL carries a hash of the text of the image's route, so that
 * a change to the card changes the URL and the caches of X and Slack fetch the new card.
 */
export async function loader({ params }: Route.LoaderArgs) {
  const slug = params.slug ?? '';
  const page = source.getPage(slug ? [slug] : []);
  if (!page) throw new Response('Not found', { status: 404 });
  const card = await readFile('app/routes/opengraph-image.tsx');
  return {
    slug,
    path: page.path,
    url: page.url,
    title: page.data.title,
    description: page.data.description ?? '',
    tree: source.getPageTree(),
    og: createHash('sha256').update(card).digest('hex').slice(0, 16),
  };
}

/**
 * The main page's metadata: the title, the frontmatter's description, and the link previews, whose
 * image is app/routes/opengraph-image.tsx. Each dialect's page has its own title, description and
 * address, in the link previews too, and the same image.
 */
export function meta({ loaderData }: Route.MetaArgs): Route.MetaDescriptors {
  if (!loaderData) return [];
  const main = loaderData.slug === '';
  const title = main ? siteTitle : pageTitle(loaderData.title);
  const { description } = loaderData;
  const url = main ? siteUrl : `${siteUrl}${loaderData.url}`;
  const image = `${siteUrl}/opengraph-image.png?${loaderData.og}`;
  return [
    { title },
    { name: 'description', content: description },
    { name: 'application-name', content: 'Reylo' },
    { name: 'author', content: 'Colin McDonnell' },
    { name: 'creator', content: 'Colin McDonnell' },
    { name: 'robots', content: 'index, follow' },
    { name: 'googlebot', content: 'index, follow, max-image-preview:large, max-snippet:-1' },
    { tagName: 'link', rel: 'canonical', href: url },
    { tagName: 'link', rel: 'alternate', type: 'text/markdown', href: `${siteUrl}${markdownPath(loaderData.slug)}` },
    { property: 'og:title', content: title },
    { property: 'og:description', content: description },
    { property: 'og:url', content: url },
    { property: 'og:site_name', content: 'Reylo' },
    { property: 'og:locale', content: 'en_US' },
    { property: 'og:image', content: image },
    { property: 'og:image:type', content: 'image/png' },
    { property: 'og:image:width', content: String(ogSize.width) },
    { property: 'og:image:height', content: String(ogSize.height) },
    { property: 'og:image:alt', content: ogAlt },
    { property: 'og:type', content: main ? 'website' : 'article' },
    { name: 'twitter:card', content: 'summary_large_image' },
    { name: 'twitter:creator', content: '@colinhacks' },
    { name: 'twitter:title', content: title },
    { name: 'twitter:description', content: description },
    { name: 'twitter:image', content: image },
    { name: 'twitter:image:alt', content: ogAlt },
    { name: 'twitter:image:type', content: 'image/png' },
    { name: 'twitter:image:width', content: String(ogSize.width) },
    { name: 'twitter:image:height', content: String(ogSize.height) },
  ];
}

/**
 * A link to a heading of the page (`#examples`, in the table of contents or the text) is a
 * navigation to React Router, which renders the route again on the `popstate` the browser fires:
 * the whole page, every section and code block, held the main thread for 270ms in the built site
 * and 1.1s under the dev server before the browser could paint the jump. The page is the same, so
 * the rendered tree is kept for as long as the route's data is, and the jump is the browser's own.
 */
export default function Page({ loaderData }: Route.ComponentProps) {
  return useMemo(() => <PageContent loaderData={loaderData} />, [loaderData]);
}

function PageContent({ loaderData }: Pick<Route.ComponentProps, 'loaderData'>) {
  const { slug, path, title, description, tree } = loaderData;
  const files = use(loadPage(path));
  const components = getMDXComponents();
  const main = slug === '';

  // docs-kit's page (docs-kit/src/components/shell.tsx): the table of contents on the left, with
  // the brand at its top (a link to the main page) and the GitHub link, the "Leave a star!" note
  // and the theme switch at its foot. The main page is content/docs/index.mdx and then each section
  // of content/sections/, each a module of its own, and its table of contents is theirs in order.
  // The subheader under the title is the page's first element (`.dk-subheader`), since it may set
  // code; the frontmatter's description, in plain text, is the page's metadata (above). On the main
  // page only, the title is centred, the mark over the name (components/hero.tsx), and "Copy agent
  // prompt" sits under the subheader with the badges (content/docs/index.mdx), copying the agent
  // card at the top of Setup; and the structured data describes the site and the project. Every page's column ends on the licence.
  return (
    <DocsShell
      // The tree's names are the pages' titles, plain strings, so it reaches the browser as it is
      // (Fumadocs' serializer would wrap each name in a span of its own).
      tree={tree as Root}
      title={main ? <HeroTitle>{title}</HeroTitle> : title}
      // An h4 (Usage's database under `.run()`) stays on the page but out of the table.
      toc={files.flatMap((file) => file.toc).filter((item) => item.depth <= 3)}
      fold={FOLD[slug] ?? []}
      brand={<Brand />}
      github={githubUrl}
      before={main ? <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: structuredData(description) }} /> : undefined}
      footer={<>Released under the MIT License. Copyright © 2026 Colin McDonnell.</>}
    >
      {files.map((file, i) => (
        <file.default key={i} components={components} />
      ))}
    </DocsShell>
  );
}

/**
 * The site and the project as schema.org data: the WebSite gives search results the site's name,
 * and the SoftwareSourceCode says what the page is about and where its code lives.
 */
function structuredData(description: string): string {
  const author = { '@type': 'Person', name: 'Colin McDonnell', url: 'https://github.com/colinhacks' };
  const data = {
    '@context': 'https://schema.org',
    '@graph': [
      { '@type': 'WebSite', '@id': `${siteUrl}/#website`, url: `${siteUrl}/`, name: 'Reylo', description, inLanguage: 'en' },
      {
        '@type': 'SoftwareSourceCode',
        '@id': `${siteUrl}/#software`,
        name: 'Reylo',
        description,
        url: `${siteUrl}/`,
        codeRepository: githubUrl,
        programmingLanguage: ['TypeScript', 'SQL'],
        runtimePlatform: 'Node.js',
        license: 'https://opensource.org/licenses/MIT',
        author,
        isPartOf: { '@id': `${siteUrl}/#website` },
      },
    ],
  };
  // `<` is escaped so that no string in the data can close the script element.
  return JSON.stringify(data).replace(/</g, '\\u003c');
}
