// One preload for every runtime: `node --import reylo/register`, `nub --import reylo/register`,
// `bun --preload reylo/register` (or bunfig.toml). It is the build step for code that runs
// unbundled: each `.sql` file becomes its module (`src/loader.ts`) as it loads, from `reylo/.cache/` or
// the database. Node's load hook (module.registerHooks) and Bun's plugin under `require()` must
// answer synchronously, while the analysis waits on the database, so they ask `src/bridge.ts`,
// which blocks on a worker thread.
import { workerData } from "node:worker_threads";
import { sqlModuleSync } from "./bridge.ts";

declare const Bun: undefined | { plugin(p: { name: string; setup(build: { onLoad(o: { filter: RegExp }, cb: (args: { path: string }) => { loader: string; contents: string }): void }): void }): void };

// The worker inherits the preload; it loads no `.sql` file itself.
if (!(workerData as { reyloWorker?: boolean } | null)?.reyloWorker) {
  if (typeof Bun !== "undefined") {
    Bun.plugin({
      name: "reylo-sql",
      setup(build) {
        build.onLoad({ filter: /\.sql$/ }, (args) => ({ loader: "js", contents: sqlModuleSync(args.path) }));
      },
    });
  } else {
    const { registerHooks } = await import("node:module");
    const { fileURLToPath } = await import("node:url");
    registerHooks({
      load(url, context, nextLoad) {
        if (!url.endsWith(".sql")) return nextLoad(url, context);
        return { format: "module", source: sqlModuleSync(fileURLToPath(url)), shortCircuit: true };
      },
    });
  }
}
