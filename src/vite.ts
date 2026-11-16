// The Vite plugin, which is also a Rollup and a Rolldown plugin: one `load` hook that turns a
// `.sql` file into its module (`src/loader.ts`). `enforce: "pre"` runs it before Vite's own
// asset handling; Rollup and Rolldown ignore the field.
//
// It resolves a relative `.sql` import itself too, before any other plugin's resolveId (`order:
// "pre"`, wherever it is listed): the Cloudflare Vite plugin (`@cloudflare/vite-plugin` 1.64) makes
// every `.sql` import in a Worker an external text module in its own resolveId, so the import never
// reached `load`, and `.run` was undefined on the string it gave (examples/cloudflare-workers).
//
// Both hooks carry a filter, so a host that reads one (Vite from 6.3, Rollup from 4.38, Rolldown) calls them only for a `.sql` id, and Rolldown makes no call into JavaScript for any other module. Each handler checks the id again for an older host, which calls it for every module.
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { dirname, isAbsolute, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { releaseConnections, tooling } from "./tooling-loader.ts";

/** What Vitest hands a plugin's `configureVitest`, as far as it is used here. */
type VitestContext = {
  defineCacheKeyGenerator?(generate: (module: { id: string }) => false | undefined): void;
  experimental_defineCacheKeyGenerator?(generate: (module: { id: string }) => false | undefined): void;
};

// reylo's own placeholder for the migrations `migrate(ctx.storage)` runs, beside this file, which
// `load` fills with the project's (src/embedded-module.ts). Matched by its real path, so that no file
// of the project's by that name is touched.
const extension = import.meta.url.slice(import.meta.url.lastIndexOf("."));
const placeholderPath = fileURLToPath(new URL(`./embedded-migrations${extension}`, import.meta.url));
const PLACEHOLDER = existsSync(placeholderPath) ? realpathSync(placeholderPath) : placeholderPath;
const isPlaceholder = (id: string) => {
  const path = id.replace(/[?#].*$/, "");
  return path.endsWith(`embedded-migrations${extension}`) && existsSync(path) && realpathSync(path) === PLACEHOLDER;
};

export default function reylo() {
  // The project whose migrations the bundle carries: Vite's root, else the working directory.
  let root = process.cwd();
  return {
    name: "reylo",
    enforce: "pre" as const,
    configResolved(config: { root: string }) {
      root = config.root;
    },
    // Vite's dev server bundles a dependency before any plugin's `load` runs, which would leave the
    // placeholder empty: reylo stays out of that, in every environment (the Cloudflare plugin's
    // Worker is one of its own), and is served as its modules are.
    config() {
      return { optimizeDeps: { exclude: ["reylo"] } };
    },
    configEnvironment() {
      return { optimizeDeps: { exclude: ["reylo"] } };
    },
    resolveId: {
      order: "pre" as const,
      filter: { id: /\.sql$/ },
      handler(source: string, importer: string | undefined): string | null {
        if (!source.endsWith(".sql") || importer === undefined) return null;
        if (!source.startsWith("./") && !source.startsWith("../")) return null;
        // An importer may carry a query (`?v=…`) or be a virtual module, whose path is no directory.
        const from = importer.replace(/[?#].*$/, "");
        if (!isAbsolute(from)) return null;
        const path = resolve(dirname(from), source);
        return existsSync(path) ? path : null;
      },
    },
    load: {
      filter: { id: /\.sql$|embedded-migrations\.[jt]s(?:\?.*)?$/ },
      async handler(this: { addWatchFile(id: string): void }, id: string) {
        // reylo's placeholder takes the project's migrations, for `migrate(ctx.storage)`.
        if (isPlaceholder(id)) {
          const { code, watched } = await (await tooling(root)).embeddedModule(root);
          for (const file of watched) this.addWatchFile(file);
          return { code, map: null };
        }
        if (!id.endsWith(".sql")) return null;
        const { code, entry, watched } = await (await tooling(dirname(id))).sqlModule(id, readFileSync(id, "utf8"));
        // A new analysis of the statement (the type check's, after a migration) rebuilds the module,
        // and so does a change to the project file or a `.env` file, which the module's text reads.
        for (const file of [entry, ...watched]) this.addWatchFile(file);
        return { code, map: null };
      },
    },
    // Vitest keeps a transformed module across runs (`fsModuleCache`, on by default from Vitest
    // 5.0.5), keyed by its file's text, and in watch mode keeps its key until that file changes. A
    // `.sql` module changes with the analysis (a migration), the project file and `.env` as well,
    // and a key generator is synchronous where the module may wait on the database, so Vitest
    // keeps none: `load` makes each one again, from `reylo/.cache/` while the schema is unchanged.
    configureVitest(context: VitestContext) {
      const key = ({ id }: { id: string }) => (id.endsWith(".sql") ? false : undefined);
      // Vitest 4 has only the experimental name, which Vitest 5 keeps as a deprecated one.
      if (context.defineCacheKeyGenerator) context.defineCacheKeyGenerator(key);
      else context.experimental_defineCacheKeyGenerator?.(key);
    },
    async buildEnd() {
      await releaseConnections();
    },
  };
}
