export const githubUrl = 'https://github.com/colinhacks/reylo';

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
