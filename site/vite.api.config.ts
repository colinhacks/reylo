import path from 'node:path';
import { defineConfig } from 'vite';

// The site's functions' code as plain modules in .live/api/, which the functions in api/ import:
// the site's build runs this after `react-router build`, whose live plugin writes the files the
// playground's API reads beside it in .live/, and which writes the /play page the share page
// carries. Packages stay imports, which each function carries from node_modules.
// - play-api.mjs: the playground's API (lib/live/api.ts), for api/play/.
// - play-og.mjs and play-page.mjs: a shared query's link preview and the page that names it
//   (lib/live/share-image.ts, lib/live/share-page.ts), for api/play-og/. Neither loads the engine
//   or PGlite.
const live = path.join(import.meta.dirname, 'lib', 'live');

export default defineConfig({
  publicDir: false,
  build: {
    ssr: true,
    outDir: path.join(import.meta.dirname, '.live', 'api'),
    emptyOutDir: true,
    minify: false,
    rollupOptions: {
      input: { 'play-api': path.join(live, 'api.ts'), 'play-og': path.join(live, 'share-image.ts'), 'play-page': path.join(live, 'share-page.ts') },
      output: { entryFileNames: '[name].mjs', chunkFileNames: 'chunks/[name]-[hash].mjs', format: 'es' },
    },
  },
});
