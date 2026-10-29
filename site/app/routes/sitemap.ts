import { siteUrl } from '@/lib/layout.shared';
import { source } from '@/lib/source';

// The site's pages (the main page at /, and each dialect's reference at /postgres, /mysql,
// /sqlite and /duckdb), dated by the build, which runs on every push that changes the site. Written to
// build/client/sitemap.xml.
export function loader() {
  const lastmod = new Date().toISOString();
  const urls = source.getPages().map((page) => `<url>
<loc>${new URL(page.url, siteUrl).href}</loc>
<lastmod>${lastmod}</lastmod>
<changefreq>weekly</changefreq>
<priority>1</priority>
</url>
`);
  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls.join('')}</urlset>
`;
  return new Response(xml, { headers: { 'Content-Type': 'application/xml; charset=utf-8' } });
}
