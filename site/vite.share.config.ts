import { readFileSync } from 'node:fs';
import path from 'node:path';
import { defineConfig } from 'vite';

// A shared query's link preview and the page that names it (lib/live/share-image.ts, lib/live/share-page.ts) as plain modules in .share/, which the functions in api/play-og/ import: the site's build runs this after `react-router build`, which writes the /play page the share page carries inside it. Packages stay imports, which each function carries from node_modules. The modules sit apart from .live/, whose whole directory the playground's API functions carry, so that neither function carries the other's code: these load neither the engine nor PGlite.
const live = path.join(import.meta.dirname, 'lib', 'live');

export default defineConfig({
  publicDir: false,
  // The examples /play shows, which the page's loader wrote while `react-router build` ran (lib/live/share-shown.ts).
  define: { __PLAY_EXAMPLES__: readFileSync(path.join(import.meta.dirname, '.live', 'play-examples.json'), 'utf8') },
  build: {
    ssr: true,
    outDir: path.join(import.meta.dirname, '.share'),
    emptyOutDir: true,
    minify: false,
    rollupOptions: {
      input: { 'play-og': path.join(live, 'share-image.ts'), 'play-page': path.join(live, 'share-page.ts') },
      output: { entryFileNames: '[name].mjs', chunkFileNames: '[name]-[hash].mjs', format: 'es' },
    },
  },
});
