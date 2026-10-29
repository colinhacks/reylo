import { docs } from 'collections/server';
import { loader } from 'fumadocs-core/source';

// The site's pages are content/docs: the main page, index.mdx, at /, and a reference page per
// dialect, postgres.mdx, mysql.mdx, sqlite.mdx and duckdb.mdx, at /postgres, /mysql, /sqlite and
// /duckdb. The main page
// goes on in content/sections/, a file per h2 section (lib/page-files.ts). Read at build time only,
// by the routes' loaders; the browser loads each page's files itself (lib/content.ts).
export const source = loader({
  baseUrl: '/',
  source: docs.toFumadocsSource(),
});
