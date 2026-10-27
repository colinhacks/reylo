import path from 'node:path';
import { docsKit } from 'docs-kit/vite';
import { defineConfig } from 'vite';
import { live } from './lib/live/plugin.ts';
import { playground } from './lib/playground-plugin.ts';
import { mapperDependents } from './lib/remark-mapper.ts';
import * as mdxConfig from './source.config.ts';

// docs-kit's plugins (docs-kit/src/vite.ts): fumadocs-mdx over source.config.ts, Tailwind 4, React
// Router, and what keeps `vite dev` cheap on a large page (one compile per edit, a small module, a
// section re-rendered alone), and the mapper's own (a section compiled again when a statement it
// imports from another section changes, and only then), and the local playground's endpoint on the
// dev server (lib/playground-plugin.ts), and the live card's files (lib/live/plugin.ts). `@/` is the site's root; `collections` is fumadocs-mdx's
// generated .source/ (lib/source.ts).
export default defineConfig({
  plugins: [...docsKit(mdxConfig), mapperDependents(), playground(), live()],
  resolve: {
    alias: {
      '@': import.meta.dirname,
      collections: path.join(import.meta.dirname, '.source'),
    },
  },
  server: { port: 3311 },
  // The live card's worker (lib/live/worker.ts) loads PGlite as a chunk of its own, which a module
  // worker can; PGlite finds its files by the URLs it is handed, so Vite leaves it unbundled in dev.
  // PGlite's data file, which its own code and the worker both name, ships as a `.tar`: the CDN
  // compresses a tar (6.3 MB to about 1.1 MB) and leaves `application/octet-stream` as it is.
  worker: {
    format: 'es',
    rollupOptions: { output: { assetFileNames: (asset) => (asset.names?.[0]?.endsWith('.data') ? 'assets/[name]-[hash].tar' : 'assets/[name]-[hash][extname]') } },
  },
  optimizeDeps: { exclude: ['@electric-sql/pglite'] },
  // @vercel/og (the link preview, app/routes/opengraph-image.tsx) loads its WebAssembly with a
  // `require` Vite's module runner cannot run, so Node loads it.
  // jco's preview2-shim, which the playground's build-time data runs the engine over, starts its I/O worker from a
  // file beside its own module: it runs from node_modules, where that file is, rather than bundled.
  ssr: { external: ['@vercel/og', '@bytecodealliance/preview2-shim'] },
});
