// The esbuild plugin, which is also a Bun plugin: `Bun.build({ plugins })` and `Bun.plugin` take
// the same `setup(build)` object, with the same `onLoad` filter and the same `{ contents, loader }`
// result. Bun's runtime plugin answers `import` through it; `require()` needs a synchronous
// answer, which `reylo/register` gives.
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { releaseConnections, tooling } from "./tooling-loader.ts";

// reylo's own placeholder for the migrations `migrate(ctx.storage)` runs, beside this file, which
// `onLoad` fills with the project's (src/embedded-module.ts), matched by its real path.
const extension = import.meta.url.slice(import.meta.url.lastIndexOf("."));
const placeholderPath = fileURLToPath(new URL(`./embedded-migrations${extension}`, import.meta.url));
const PLACEHOLDER = existsSync(placeholderPath) ? realpathSync(placeholderPath) : placeholderPath;

type Build = {
  /** esbuild's; the project whose migrations the bundle carries is its working directory. */
  initialOptions?: { absWorkingDir?: string };
  onLoad(options: { filter: RegExp }, callback: (args: { path: string }) => Promise<{ contents: string; loader: "js"; watchFiles?: string[] } | undefined>): void;
  /** esbuild's and Bun.build's; Bun's runtime plugin has none. */
  onEnd?(callback: () => Promise<void>): void;
};

export default function reylo() {
  return {
    name: "reylo",
    setup(build: Build) {
      build.onLoad({ filter: /\.sql$/ }, async (args) => {
        const { code, entry, watched } = await (await tooling(dirname(args.path))).sqlModule(args.path, readFileSync(args.path, "utf8"));
        // In watch mode a new analysis of the statement (the type check's, after a migration)
        // rebuilds the module, and so does a change to the project file or a `.env` file.
        return { contents: code, loader: "js", watchFiles: [entry, ...watched] };
      });
      // reylo's placeholder takes the project's migrations, for `migrate(ctx.storage)`.
      build.onLoad({ filter: /embedded-migrations\.[jt]s$/ }, async (args) => {
        if (!existsSync(args.path) || realpathSync(args.path) !== PLACEHOLDER) return undefined;
        const root = build.initialOptions?.absWorkingDir ?? process.cwd();
        const { code, watched } = await (await tooling(root)).embeddedModule(root);
        return { contents: code, loader: "js", watchFiles: watched };
      });
      build.onEnd?.(releaseConnections);
    },
  };
}
