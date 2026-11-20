import { SHOW_RUST } from './languages.ts';

export const githubUrl = 'https://github.com/reylosql/reylo';

/** Reylo Migrate's docs: content/docs/migrate.mdx, at an address of its own under its landing page, /migrate (components/migrate-landing.tsx). */
export const migrateDocsUrl = '/migrate/docs';

/** The docs' intro, content/docs/index.mdx, at an address of its own under the homepage, / (components/home-landing.tsx). */
export const introUrl = '/docs';

/** A page's address: its file's name, but the intro and Reylo Migrate's docs, which sit under their landing pages. */
export const docsUrl = (url: string) => (url === '/' ? introUrl : url === '/migrate' ? migrateDocsUrl : url);

/** The docs site's address: reylo.dev, which every other Reylo domain redirects to (wiki/docs-site.md). */
export const siteUrl = 'https://reylo.dev';

/**
 * The address of a page's Markdown, by its slug: /index.md for the main page, /<slug>.md for
 * another (lib/markdown.ts writes them). Here, not there, since the page's `meta`, which links it,
 * runs in the browser too, and lib/markdown.ts reads the disk.
 */
export const markdownPath = (slug: string) => `/${slug || 'index'}.md`;

/**
 * The title search results and link previews show: the name and what it does, in under 60
 * characters so that Google does not cut it. The page's h1 stays the name alone.
 */
export const siteTitle = 'Reylo — Import .sql files into TypeScript, fully typed';

/**
 * A dialect's reference page (content/docs/<dialect>.mdx) in search results and link previews:
 * the name, the database and what the page holds, under 60 characters. The page's h1 stays the
 * database's name.
 */
export const pageTitle = (database: string) => `Reylo on ${database}: types, nulls, row counts, drivers`;

/** The comparison's title in search results and link previews. */
export const compareTitle = 'Reylo compared with sqlc, sqlx, PgTyped and others';

/** A page's title in search results and link previews, where the title is not a database's. */
export const docsTitle = (title: string) => `${title} — Reylo`;

/**
 * The list of pages in the left column, by group, and the links away from the docs at its foot:
 * first what holds for every language, then each language's own pages, then the reference. The Rust
 * page only while the docs show Rust (lib/languages.ts).
 */
export const pageGroups = [
  {
    label: 'Docs',
    items: [
      { title: 'Intro', href: introUrl },
      { title: SHOW_RUST ? 'Usage with TypeScript' : 'Usage', href: '/typescript' },
      ...(SHOW_RUST ? [{ title: 'Usage with Rust', href: '/rust' }] : []),
      { title: 'Inference', href: '/inference' },
      { title: 'Reylo Migrate', href: '/migrate', external: true },
      { title: 'Comparison', href: '/compare' },
      { title: 'FAQ', href: '/faq' },
    ],
  },
  {
    label: 'Deployment',
    items: [
      { title: 'Cloudflare D1', href: '/d1' },
      { title: 'Durable Objects', href: '/durable-objects' },
      { title: 'Cloudflare Hyperdrive', href: '/hyperdrive' },
      { title: 'Neon', href: '/neon' },
      { title: 'Supabase', href: '/supabase' },
      { title: 'PlanetScale', href: '/planetscale' },
      { title: 'Turso', href: '/turso' },
      { title: 'Next.js', href: '/nextjs' },
      { title: 'CI', href: '/deployment' },
    ],
  },
  {
    label: 'Reference',
    items: [
      { title: '.reylo.yaml', href: '/configuration', code: true },
      { title: 'CLI', href: '/cli' },
      { title: 'Postgres', href: '/postgres' },
      { title: 'MySQL', href: '/mysql' },
      { title: 'SQLite', href: '/sqlite' },
    ],
  },
];

export const pageLinks = [
  { title: 'Playground', href: '/play' },
  { title: 'npm', href: 'https://www.npmjs.com/package/reylo' },
];
