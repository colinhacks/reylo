import { docsUrl, siteUrl } from '@/lib/layout.shared';
import { source } from '@/lib/source';
import { unlistedPage } from '@/lib/rust-content';

// The site's pages, dated by the build, which runs on every push that changes the site. Written to
// build/client/sitemap.xml.
export function loader() {
  const lastmod = new Date().toISOString();
  // The homepage, /, and Reylo Migrate's landing page, /migrate, are pages too, each over its docs.
  // An unlisted page (lib/rust-content.ts) is left out.
  const pages = source.getPages().filter((page) => !unlistedPage(page.slugs.join('/')));
  const urls = ['/', ...pages.map((page) => docsUrl(page.url)), '/migrate'].map((url) => `<url>
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
