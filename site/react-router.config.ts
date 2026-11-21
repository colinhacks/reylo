import { copyFileSync, readdirSync } from 'node:fs';
import type { Config } from '@react-router/dev/config';
import { builtPage } from './lib/rust-content.ts';

// The site builds to static files only (build/client), which Vercel serves as they are: no server
// renders at run time (`ssr: false`), and every path the site has is rendered at build time: the
// homepage (/), each page of content/docs (index.mdx at /docs, every other file at its name), each
// page's Markdown (/index.md, /<name>.md), the files beside them, the playground (/play) and its link
// preview (/play/og.png), and the page for a missing address, which Vercel serves from 404.html. The
// Rust page is one too, unlisted while the docs leave Rust out (lib/rust-content.ts).
const names = readdirSync('content/docs')
  .filter(builtPage)
  .map((name) => name.slice(0, -'.mdx'.length));
// The intro is index.mdx, at /docs, under the homepage at /; Reylo Migrate's docs are migrate.mdx, at
// /migrate/docs, under its landing page at /migrate.
const routes = ['/', '/migrate', ...names.map((name) => (name === 'index' ? '/docs' : name === 'migrate' ? '/migrate/docs' : `/${name}`))];

export default {
  ssr: false,
  prerender: [...routes, ...names.map((name) => `/${name}.md`), '/404', '/play', '/play/og.png', '/play/api.md', '/llms.txt', '/robots.txt', '/sitemap.xml', '/manifest.webmanifest', '/opengraph-image.png'],
  buildEnd() {
    copyFileSync('build/client/404/index.html', 'build/client/404.html');
  },
} satisfies Config;
