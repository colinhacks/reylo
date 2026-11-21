// A docs site's Vite plugins: fumadocs-mdx over the site's source.config.ts, Tailwind 4, React
// Router's framework mode, and what makes a large MDX page cheap to edit in `vite dev` (measured on
// Reylo's page on 2026-10-20). `vite.config.ts` is one line:
//
//   import * as mdxConfig from './source.config';
//   export default defineConfig({ plugins: docsKit(mdxConfig) });
//
// The site's react-router.config.ts sets `ssr: false` and the paths to prerender, so `react-router
// build` writes static files only (build/client).
import { createHash } from 'node:crypto';
import v8 from 'node:v8';
import vm from 'node:vm';
import { reactRouter } from '@react-router/dev/vite';
import tailwindcss from '@tailwindcss/vite';
import mdx from 'fumadocs-mdx/vite';
import type { Plugin, PluginOption, UserConfig } from 'vite';

type MdxConfig = Parameters<typeof mdx>[0];

const MDX = /\.mdx?(\?|$)/;
const JSX_RUNTIME = /^import \{([^}]*)\} from "react\/jsx(-dev)?-runtime";$/m;

/**
 * The compiled page made small, in `vite dev` only. Without it Vite gives the server's copy of the
 * page an inline source map of the whole page: Reylo's page module was 91.8 MB, 74 MB of it the map,
 * and the dev server reached 4.4–5.0 GB. The map is replaced by an empty one, the JSX runtime is
 * imported once into local constants (each call otherwise names the module), and characters above
 * U+00FF become `\u` escapes, so V8 keeps the source at one byte a character.
 */
function slim(): Plugin {
  return {
    name: 'docs-kit:slim',
    apply: 'serve',
    transform: {
      filter: { id: MDX },
      handler(code, id) {
        if (id.includes('only=frontmatter')) return null;
        const out = code
          .replace(JSX_RUNTIME, (_, names: string, dev?: string) => {
            const constants = names.split(',').map((name) => {
              const [imported, local = imported] = name.trim().split(/\s+as\s+/);
              return `${local} = _jsxRuntime.${imported}`;
            });
            return `import * as _jsxRuntime from "react/jsx${dev ?? ''}-runtime"; const ${constants.join(', ')};`;
          })
          .replace(/[Ā-￿]/g, (c) => `\\u${c.charCodeAt(0).toString(16).padStart(4, '0')}`);
        return { code: out, map: { version: 3, sources: [id.split('?')[0]], names: [], mappings: '' } };
      },
    },
  };
}

let gc: (() => void) | undefined;
let collections: ReturnType<typeof setTimeout>[] = [];

/** Collects the garbage 1, 5, 15 and 30 seconds after the latest compile, so V8 hands the compile's transient memory back. */
function collectAfterCompile() {
  try {
    if (!gc) {
      v8.setFlagsFromString('--expose-gc');
      gc = vm.runInNewContext('gc') as () => void;
    }
  } catch {
    return;
  }
  for (const timer of collections) clearTimeout(timer);
  collections = [1000, 5000, 15000, 30000].map((ms) => setTimeout(() => gc!(), ms).unref());
}

type WatchingContext = { addWatchFile(file: string): void };

/**
 * One MDX compile per edit in `vite dev`, and one per page in the build. The browser's copy of a page
 * and the server's are two Vite environments, and each ran fumadocs-mdx's transform, so every edit
 * compiled the page twice on one thread, and the build compiled every page twice, once for the
 * client bundle and again for the server's (9.8 s of Reylo's 28.9 s build, 2026-11-20). The
 * transform is memoized by the file and its source, so the environment that asks second takes the
 * first one's output (both compile with the same options, so the output is the same), and the files
 * the compile watched (a remark plugin's dependencies) are watched for it too, so an edit to one of
 * them reaches the page in both environments.
 */
function sharedCompile(): Plugin {
  const cache = new Map<string, { result: Promise<unknown>; watched: string[] }>();
  return {
    name: 'docs-kit:shared-compile',
    // One instance for every environment of the build, as the dev server has.
    sharedDuringBuild: true,
    // A page's output also depends on files other than its own (a statement another section
    // defines, captured types): any change Vite sees empties the cache, so the cache only ever
    // shares one compile between the two environments that ask for the same version.
    watchChange() {
      cache.clear();
    },
    configResolved(config) {
      const target = config.plugins.find((plugin) => plugin.name === 'fumadocs-mdx:mdx') as (Plugin & { transform: { handler: (...args: unknown[]) => unknown } }) | undefined;
      if (!target?.transform || typeof target.transform !== 'object') return;
      const handler = target.transform.handler;
      target.transform.handler = async function (this: WatchingContext, code: unknown, id: unknown, ...rest: unknown[]) {
        const key = `${id}\0${createHash('sha1').update(String(code)).digest('hex')}`;
        const hit = cache.get(key);
        if (hit) {
          const result = await hit.result;
          for (const file of hit.watched) this.addWatchFile(file);
          return result;
        }
        for (const k of cache.keys()) if (k.startsWith(`${id}\0`)) cache.delete(k);
        // The context is this transform's own, so the files the compile watches are recorded on it
        // while the compile runs.
        const watched: string[] = [];
        const addWatchFile = this.addWatchFile;
        this.addWatchFile = (file) => {
          watched.push(file);
          addWatchFile.call(this, file);
        };
        const result = Promise.resolve(handler.call(this, code, id, ...rest)).finally(() => {
          this.addWatchFile = addWatchFile;
          collectAfterCompile();
        });
        cache.set(key, { result, watched });
        return result;
      };
    },
  };
}

/** React Router's Fast Refresh footer on a client module (@react-router/dev, REACT_REFRESH_FOOTER). */
const REFRESH_FOOTER =
  /RefreshRuntime\.__hmr_import\(import\.meta\.url\)\.then\(\(currentExports\) => \{\s*RefreshRuntime\.registerExportsForReactRefresh\(("(?:[^"\\]|\\.)*"), currentExports\);\s*import\.meta\.hot\.accept\(\(nextExports\) => \{[\s\S]*?\n\s*\}\);\s*\}\);/;

/**
 * Each MDX file is its own Fast Refresh boundary, in `vite dev`, so an edit re-renders only that
 * file's content. React Router's refresh wrapper refuses an MDX module's update because its `toc`,
 * `frontmatter` and `structuredData` exports are new objects after every edit, and the update climbed
 * to the route and re-rendered the whole page (about 3 s on Reylo's page). This accepts `toc` and
 * `frontmatter` when they are equal as JSON (otherwise the update climbs as before, so a new heading
 * still reaches the table of contents) and `structuredData`, the search index, always: a section
 * edit then reloads in under a second.
 *
 * It also registers the module's `accept` handler at once. React Router's wrapper registered it
 * only after an asynchronous import of the module resolved, and Vite clears a module's handlers when
 * a new version of it runs, so an update arriving in between (an edit makes two or three in a row:
 * the file, fumadocs-mdx's dependencies, Tailwind's rescan) found no handler and was dropped
 * silently: 6 of 20 edits to a section never reached the browser. The handler now waits for the old
 * exports inside itself instead.
 */
function refreshBoundary(): Plugin {
  return {
    name: 'docs-kit:refresh-boundary',
    apply: 'serve',
    enforce: 'post',
    transform: {
      filter: { id: /\.mdx(\?|$)/ },
      handler(code) {
        if (this.environment.name !== 'client') return null;
        const footer = REFRESH_FOOTER.exec(code);
        if (!footer) return null;
        const source = footer[1];
        const replacement = `const __dkCurrentExports = RefreshRuntime.__hmr_import(import.meta.url);
  __dkCurrentExports.then((currentExports) => RefreshRuntime.registerExportsForReactRefresh(${source}, currentExports));
  import.meta.hot.accept(async (nextExports) => {
    if (!nextExports) return;
    const currentExports = await __dkCurrentExports;
    const accepted = Object.keys(nextExports).filter((k) => k !== "default" && (k === "structuredData" || JSON.stringify(currentExports[k]) === JSON.stringify(nextExports[k])));
    const invalidateMessage = RefreshRuntime.validateRefreshBoundaryAndEnqueueUpdate(currentExports, nextExports, accepted);
    if (invalidateMessage) import.meta.hot.invalidate(invalidateMessage);
  });`;
        return { code: code.slice(0, footer.index) + replacement + code.slice(footer.index + footer[0].length), map: null };
      },
    },
  };
}

/** Twoslash and TypeScript load at build time on the server side, never bundled. */
function serverExternals(): Plugin {
  return {
    name: 'docs-kit:server-externals',
    config: (): UserConfig => ({ ssr: { external: ['typescript', 'twoslash'] } }),
  };
}

/**
 * The plugins of a docs site: fumadocs-mdx over `mdxConfig` (the site's source.config.ts, imported
 * as a namespace), Tailwind 4, React Router, and docs-kit's own for `vite dev`.
 */
export function docsKit(mdxConfig: MdxConfig): PluginOption[] {
  return [mdx(mdxConfig), slim(), sharedCompile(), tailwindcss(), reactRouter(), refreshBoundary(), serverExternals()];
}
