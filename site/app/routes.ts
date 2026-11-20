import { readdirSync } from 'node:fs';
import { index, route, type RouteConfig } from '@react-router/dev/routes';
import { builtPage } from '../lib/rust-content.ts';

// The homepage at /, the pages at /docs (the intro) and /<name> (content/docs), each page's Markdown,
// at /index.md and /<name>.md, the page a missing address gets, and the files beside them, each
// rendered to a file at build time (react-router.config.ts). The Rust page is one only while the docs
// show Rust (lib/languages.ts).
const markdown = readdirSync('content/docs')
  .filter(builtPage)
  .map((name) => `${name.slice(0, -'.mdx'.length)}.md`);

export default [
  // Reylo's homepage (components/home-landing.tsx), and the docs' intro (content/docs/index.mdx) under it.
  index('routes/home.tsx'),
  route('docs', 'routes/page.tsx', { id: 'docs' }),
  route(':slug', 'routes/page.tsx'),
  // Reylo Migrate: its landing page, and its docs (content/docs/migrate.mdx) under it.
  route('migrate', 'routes/migrate.tsx'),
  route('migrate/docs', 'routes/page.tsx', { id: 'migrate-docs' }),
  ...markdown.map((file) => route(file, 'routes/markdown.ts', { id: `markdown/${file}` })),
  route('404', 'routes/not-found.tsx'),
  // The playground, full screen (components/play.tsx).
  route('play', 'routes/play.tsx'),
  // Its link preview, the card of the query it opens with (a shared query's card is drawn on request).
  route('play/og.png', 'routes/play-og.tsx'),
  // Its API, as Markdown for an agent.
  route('play/api.md', 'routes/play-api-md.ts'),
  // The local playground (app/routes/playground.tsx), on the dev server only: it needs scratch databases.
  ...(process.env.NODE_ENV === 'production' ? [] : [route('playground', 'routes/playground.tsx')]),
  route('llms.txt', 'routes/llms.ts'),
  route('robots.txt', 'routes/robots.ts'),
  route('sitemap.xml', 'routes/sitemap.ts'),
  route('manifest.webmanifest', 'routes/manifest.ts'),
  route('opengraph-image.png', 'routes/opengraph-image.tsx'),
] satisfies RouteConfig;
