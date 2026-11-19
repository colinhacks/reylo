import { docsUrl, siteUrl } from '@/lib/layout.shared';
import { source } from '@/lib/source';

// The site's pages, dated by the build, which runs on every push that changes the site. Written to
// build/client/sitemap.xml.
export function loader() {
  const lastmod = new Date().toISOString();
  // Reylo Migrate's docs sit under its landing page, /migrate, which is a page too.
  const urls = [...source.getPages().map((page) => docsUrl(page.url)), '/migrate'].map((url) => `<url>
<loc>${new URL(url, siteUrl).href}</loc>
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
