import { readdirSync } from 'node:fs';
import { defineCollections, defineConfig, defineDocs } from 'fumadocs-mdx/config';
import { SHOW_RUST } from './lib/languages.ts';
import { builtPage } from './lib/rust-content.ts';

// The pages are content/docs. Usage and Inference go on in content/sections/ (lib/page-sections.ts), so that an edit
// compiles one section. Both take lib/mdx-options.ts, imported inside the options so that only the
// build loads it (and TypeScript with it). While the docs leave Rust out (lib/languages.ts), its
// page is no page at all: the pages are listed by name, every one of content/docs but rust.mdx.
const mdxOptions = async (environment: Parameters<typeof import('./lib/mdx-options.ts').mdxOptions>[0]) => (await import('./lib/mdx-options.ts')).mdxOptions(environment);

export const docs = defineDocs({ dir: 'content/docs', docs: { mdxOptions, ...(SHOW_RUST ? {} : { files: readdirSync('content/docs').filter(builtPage) }) } });
export const sections = defineCollections({ type: 'doc', dir: 'content/sections', mdxOptions });

export default defineConfig();
