// A shared /play address, `/play?q=<sql>` or `/play?example=<slug>`, and its link preview: what the share functions (lib/live/share-image.ts, lib/live/share-page.ts) read from the address, and the page's meta tags rewritten to name the card of that query. Plain functions, with no import, so that both functions and the card (lib/live/share-card.tsx) take them as they are.

/** The card's size, which the page's meta tags state. */
export const cardSize = { width: 1200, height: 630 };

/**
 * The longest query text a card reads, and the longest an image's address carries. Far more than a card ever shows (at its smallest size it holds 14 rows of 80-odd columns), so a query cut here still runs past the card's last row and gets its ellipsis.
 */
export const CARD_TEXT_BUDGET = 2400;

/** The address's query: its `q`, else its example's statement, as the playground opens it (lib/live/app.tsx). */
export function sharedQuery(params: URLSearchParams, examples: ReadonlyMap<string, string>): { sql: string; param: 'q' | 'example'; value: string } | undefined {
  const q = params.get('q');
  if (q !== null && q.trim()) return { sql: q, param: 'q', value: q.slice(0, CARD_TEXT_BUDGET) };
  const slug = params.get('example');
  const sql = slug === null ? undefined : examples.get(slug);
  if (slug !== null && sql !== undefined) return { sql, param: 'example', value: slug };
  return undefined;
}

/** The card's description, for og:image:alt: the query on one line, without its comments, cut to a sentence's length. */
export function cardAlt(sql: string): string {
  const flat = sql.replace(/--[^\n]*/g, '').replace(/\s+/g, ' ').replace(/\( /g, '(').replace(/ \)/g, ')').trim();
  return `A SQL query in the Reylo playground: ${flat.length > 240 ? `${flat.slice(0, 239)}…` : flat}`;
}

const ENTITIES: Record<string, string> = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

export function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (ch) => ENTITIES[ch]!);
}

/** The meta tags a shared address's page names its own card in, by their `property` or `name`. */
export type ShareTags = Record<'og:url' | 'og:image' | 'og:image:alt' | 'twitter:image' | 'twitter:image:alt', string>;

/**
 * The link preview's tags for a shared address on `origin`, or nothing when it names no query: the address itself, and the card of its query, /play/og.png with the same query and the card's `version`. The share function writes them into the prerendered page (lib/live/share-page.ts), and the page's `meta` gives the same ones in the reader's tab (app/routes/play.tsx), so that the head hydrates as it was served.
 */
export function shareTags(params: URLSearchParams, examples: ReadonlyMap<string, string>, origin: string, version: string | undefined): ShareTags | undefined {
  const shared = sharedQuery(params, examples);
  if (!shared) return undefined;
  const image = new URLSearchParams({ [shared.param]: shared.value });
  if (version) image.set('v', version);
  const imageUrl = `${origin}/play/og.png?${image}`;
  const alt = cardAlt(shared.sql);
  return {
    'og:url': `${origin}/play?${new URLSearchParams({ [shared.param]: shared.param === 'q' ? shared.sql : shared.value })}`,
    'og:image': imageUrl,
    'og:image:alt': alt,
    'twitter:image': imageUrl,
    'twitter:image:alt': alt,
  };
}

const META = /<meta (property|name)="([^"]+)" content="([^"]*)"\/?>/g;

/** The `content` of the page's meta tag `key`, as the page wrote it (HTML-escaped). */
export function metaContent(html: string, key: string): string | undefined {
  for (const [, , name, content] of html.matchAll(META)) if (name === key) return content;
  return undefined;
}

/**
 * The page with each tag of `tags` set to its value, escaped: in place where the page has the tag, so that the head hydrates as the page renders it, else added at the end of the head.
 */
export function rewriteMeta(html: string, tags: Partial<ShareTags>): string {
  const left = new Map(Object.entries(tags));
  const out = html.replace(META, (tag, attr: string, name: string) => {
    const value = left.get(name);
    if (value === undefined) return tag;
    left.delete(name);
    return `<meta ${attr}="${name}" content="${escapeHtml(value)}"/>`;
  });
  if (!left.size) return out;
  const added = [...left].map(([name, value]) => `<meta ${name.startsWith('og:') ? 'property' : 'name'}="${name}" content="${escapeHtml(value)}"/>`).join('');
  return out.replace('</head>', `${added}</head>`);
}
