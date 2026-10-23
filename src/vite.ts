// The Vite plugin, which is also a Rollup and a Rolldown plugin: one `load` hook that turns a
// `.sql` file into its module (`src/loader.ts`). `enforce: "pre"` runs it before Vite's own
// asset handling; Rollup and Rolldown ignore the field.
//
// It resolves a relative `.sql` import itself too, before any other plugin's resolveId (`order:
// "pre"`, wherever it is listed): the Cloudflare Vite plugin (`@cloudflare/vite-plugin` 1.64) makes
// every `.sql` import in a Worker an external text module in its own resolveId, so the import never
// reached `load`, and `.run` was undefined on the string it gave (examples/cloudflare-workers).
import { existsSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, resolve } from "node:path";
import { releaseConnections, tooling } from "./engine-package.ts";

export default function reylo() {
  return {
    name: "reylo",
    enforce: "pre" as const,
    resolveId: {
      order: "pre" as const,
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
    async load(this: { addWatchFile(id: string): void }, id: string) {
      if (!id.endsWith(".sql")) return null;
      const { code, entry, watched } = await (await tooling(dirname(id))).sqlModule(id, readFileSync(id, "utf8"));
      // A new analysis of the statement (the type check's, after a migration) rebuilds the module,
      // and so does a change to the project file or a `.env` file, which the module's text reads.
      for (const file of [entry, ...watched]) this.addWatchFile(file);
      return { code, map: null };
    },
    async buildEnd() {
      await releaseConnections();
    },
  };
}
