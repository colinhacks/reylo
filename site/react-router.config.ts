import { copyFileSync, readdirSync } from 'node:fs';
import type { Config } from '@react-router/dev/config';

// The site builds to static files only (build/client), which Vercel serves as they are: no server
// renders at run time (`ssr: false`), and every path the site has is rendered at build time: each
// page of content/docs (index.mdx at /, every other file at its name), each page's Markdown
// (/index.md, /<name>.md), the files beside them, the playground (/play) and its link preview (/play/og.png), and the page for a missing address, which
// Vercel serves from 404.html.
const pages = readdirSync('content/docs')
  .filter((name) => name.endsWith('.mdx'))
  .map((name) => (name === 'index.mdx' ? '/' : `/${name.slice(0, -'.mdx'.length)}`));

export default {
  ssr: false,
  prerender: [...pages, ...pages.map((page) => `${page === '/' ? '/index' : page}.md`), '/404', '/play', '/play/og.png', '/llms.txt', '/robots.txt', '/sitemap.xml', '/manifest.webmanifest', '/opengraph-image.png'],
  buildEnd() {
    copyFileSync('build/client/404/index.html', 'build/client/404.html');
  },
} satisfies Config;
