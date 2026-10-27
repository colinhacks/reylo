import { readdirSync } from 'node:fs';
import { route, type RouteConfig } from '@react-router/dev/routes';

// The pages, at / and /<dialect> (content/docs), each page's Markdown, at /index.md and
// /<dialect>.md, the page a missing address gets, and the files beside them, each rendered to a
// file at build time (react-router.config.ts).
const markdown = readdirSync('content/docs')
  .filter((name) => name.endsWith('.mdx'))
  .map((name) => `${name.slice(0, -'.mdx'.length)}.md`);

export default [
  route(':slug?', 'routes/page.tsx'),
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
