import { docs } from 'collections/server';
import { loader } from 'fumadocs-core/source';

// The site's pages are content/docs: index.mdx at /, and every other file at its name. Usage and
// Inference go on in content/sections/ (lib/page-sections.ts). Read at build time only,
// by the routes' loaders; the browser loads each page's files itself (lib/content.ts).
export const source = loader({
  baseUrl: '/',
  source: docs.toFumadocsSource(),
});
