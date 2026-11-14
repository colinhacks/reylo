// How reylo's entry points (the build steps, the preload's worker and the content mapper) load
// the build-time half (src/tooling.ts) the first time they run: from beside this file, else,
// where a bundler has moved this file into a bundle of its own (Turbopack bundles the webpack
// loader), from reylo's install as the project resolves it.
//
// The path is computed when it is needed and loaded with a `require` from
// `process.getBuiltinModule`, which no bundler follows. A bundler that reads this file (Turbopack
// bundles the webpack loader; webpack parses it for its build dependencies) would otherwise bundle
// the whole build-time half with it, the engine's child processes, the project's drivers and its
// DuckDB included. Turbopack traces a `require` made by an imported `createRequire`, and an
// `import()` of a computed URL, which tsc wraps in a helper of its own, and reports either as a
// module it cannot resolve, which Next.js prints wherever reylo sits outside node_modules, as a
// linked package does. A `require` loads an ES module that has no top-level await.
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BuildTime } from "./build-time.ts";

/** What the build-time half offers reylo's entry points. */
export type Tooling = BuildTime & {
  /** The module a build step writes for a `.sql` file, and what the build watches: the `.reylo/` entry it was built from, and the project's files its text reads. */
  sqlModule(path: string, source: string): Promise<{ code: string; entry: string; watched: string[] }>;
  /** Closes every build step's connection, for a build that has ended. */
  releaseConnections(): Promise<void>;
  /** The methods of TypeScript's content mapper protocol, which src/mapper.ts serves over this process's stdin and stdout. */
  mapperHandlers(): Record<string, (params: unknown) => unknown | Promise<unknown>>;
};

let loaded: Tooling | undefined;

/** Node's `createRequire`, reached where no bundler sees it. */
function createRequire(from: string): NodeJS.Require {
  return process.getBuiltinModule("node:module").createRequire(from);
}

/** The build-time half; `from` is the directory of the file being built, whose project resolves reylo where this file is in a bundle. */
export async function tooling(from: string = process.cwd()): Promise<Tooling> {
  if (loaded) return loaded;
  // The half is this file's sibling, compiled (`.js`) or not (`.ts`).
  const extension = import.meta.url.slice(import.meta.url.lastIndexOf("."));
  const beside = join(dirname(fileURLToPath(import.meta.url)), `tooling${extension}`);
  let path = beside;
  if (!existsSync(path)) {
    try {
      path = join(dirname(createRequire(join(from, "package.json")).resolve("reylo")), "tooling.js");
    } catch {
      throw new Error(`reylo's build steps are not beside ${fileURLToPath(import.meta.url)}, and the project at ${from} does not resolve reylo: install reylo in the project`);
    }
  }
  return (loaded = (createRequire(import.meta.url)(path) as { default: Tooling }).default);
}

/** Closes the build steps' connections, where a build step has opened any. */
export async function releaseConnections(): Promise<void> {
  if (loaded) await loaded.releaseConnections();
}
