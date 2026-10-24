import path from 'node:path';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

// `reylo play`'s page (play-page/index.html, lib/live/standalone.tsx): the docs' playground with the
// docs' own stylesheet, built as a static page that src/play.ts serves. The root's `nub run build`
// writes it into the reylo package (scripts/build.ts); REYLO_PLAY_OUT names another directory.
export default defineConfig({
  root: path.join(import.meta.dirname, 'play-page'),
  base: './',
  plugins: [tailwindcss()],
  resolve: { alias: { '@': import.meta.dirname } },
  build: {
    outDir: process.env.REYLO_PLAY_OUT ?? path.join(import.meta.dirname, '..', 'dist', 'reylo', 'play'),
    emptyOutDir: true,
    chunkSizeWarningLimit: 2000,
  },
});
