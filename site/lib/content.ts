import browserCollections from 'collections/browser';
import type { CompiledMDXProperties } from 'fumadocs-mdx';
import { PAGE_SECTIONS } from '@/lib/page-sections';

/** One file of a page as the browser loads it: its content (`default`) and its table of contents. */
export type PageFile = Pick<CompiledMDXProperties, 'default' | 'toc'>;

const sections = browserCollections.sections.raw;
const loaded = new Map<string, Promise<PageFile[]>>();

/**
 * A page's files, in the order it shows them: the page's own file, then the sections it holds
 * (lib/page-sections.ts). Every file is a module of its own, fetched in parallel and once, so a
 * reader of /postgres downloads that page alone. The route renders them with `use()`: while they
 * load during hydration, React keeps the prerendered page.
 */
export function loadPage(path: string): Promise<PageFile[]> {
  let files = loaded.get(path);
  if (!files) {
    const own = browserCollections.docs.raw[`./${path}`];
    if (!own) throw new Error(`content/docs/${path} is not a page`);
    files = Promise.all<PageFile>([own(), ...(PAGE_SECTIONS[path] ?? []).map((name) => sections[`./${name}`]!())]);
    loaded.set(path, files);
  }
  return files;
}
