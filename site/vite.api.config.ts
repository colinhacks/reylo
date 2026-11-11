import path from 'node:path';
import { defineConfig } from 'vite';

// The playground's API (lib/live/api.ts) as one plain module, .live/api/play-api.mjs, which the
// functions in api/play/ import: the site's build runs this after `react-router build`, whose live
// plugin writes the files the API reads beside it in .live/. PGlite stays a package import, which
// the function carries from node_modules. jco's preview2-shim is bundled in its browser build, as
// the reader's tab runs it: the engine's `call` reads and opens nothing, and the shim's Node build
// starts an I/O worker from a file beside it, which the function would not carry.
export default defineConfig({
  publicDir: false,
  ssr: { noExternal: ['@bytecodealliance/preview2-shim'], resolve: { conditions: ['module', 'browser'], externalConditions: ['node'] } },
  build: {
    ssr: path.join(import.meta.dirname, 'lib', 'live', 'api.ts'),
    outDir: path.join(import.meta.dirname, '.live', 'api'),
    emptyOutDir: true,
    minify: false,
    rollupOptions: { output: { entryFileNames: 'play-api.mjs', format: 'es' } },
  },
});
