// reylo's build-time half lives in the engine packages. Each one carries it as
// `@reylosql/<database>/tooling` (src/tooling.ts), so that reylo itself holds the run time and the
// entry points a project names: the build steps, the preload's worker, the content mapper and the
// CLI. An entry point loads it the first time it runs, from the project's install first and then
// from reylo's own, as the engine itself is found (src/engine.ts). Every engine package carries the
// same code at reylo's version, so the first one installed serves a project of any database.
//
// The path is resolved when it is needed and loaded with a `require` of it, which no bundler
// follows. A bundler that reads this file (Turbopack bundles the webpack loader; webpack parses it
// for its build dependencies) follows an `import()` of a package's name, and fails the build where
// that engine is not installed; an `import()` of a computed URL, which tsc wraps in a helper of its
// own, Turbopack reports as a module it cannot resolve. A `require` loads an ES module that has no
// top-level await, which scripts/build.ts checks of each `tooling.js`.
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";

/** What an engine package's build-time half offers reylo's entry points. */
export type Tooling = {
  /** The module a build step writes for a `.sql` file, and the `.reylo/` entry it was built from, which the build watches. */
  sqlModule(path: string, source: string): Promise<{ code: string; entry: string; watched: string[] }>;
  /** Closes every build step's connection, for a build that has ended. */
  releaseConnections(): Promise<void>;
  /** The methods of TypeScript's content mapper protocol, which src/mapper.ts serves over this process's stdin and stdout. */
  mapperHandlers(): Record<string, (params: unknown) => unknown | Promise<unknown>>;
  /** Runs the CLI's command, and answers with its exit code. */
  runCli(args: string[]): Promise<number>;
};

const PACKAGES = ["@reylosql/postgres", "@reylosql/mysql", "@reylosql/sqlite"];
const VERSION: string = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).version;

let loaded: Promise<Tooling> | undefined;

/** The build-time half, as the project of the directory `from` finds it, or reylo's own install. */
export function tooling(from: string = process.cwd()): Promise<Tooling> {
  if (!loaded) {
    const loading = load(from);
    // A failure is not kept: an engine may be installed while the process runs.
    loading.catch(() => {
      if (loaded === loading) loaded = undefined;
    });
    loaded = loading;
  }
  return loaded;
}

/** Closes the build steps' connections, where a build step has opened any. */
export async function releaseConnections(): Promise<void> {
  const loading = loaded;
  if (loading) await (await loading.catch(() => undefined))?.releaseConnections();
}

async function load(from: string): Promise<Tooling> {
  let other: { name: string; version: string | undefined; dir: string } | undefined;
  for (const base of [join(from, "package.json"), import.meta.url]) {
    const require = createRequire(base);
    for (const name of PACKAGES) {
      let path: string;
      try {
        path = require.resolve(`${name}/tooling`);
      } catch {
        // An engine package from before it carried this half has no such entry: its version says why.
        try {
          const manifest = require.resolve(`${name}/package.json`);
          other ??= { name, version: versionAt(manifest), dir: dirname(manifest) };
        } catch {}
        continue;
      }
      const version = versionAt(join(dirname(path), "package.json"));
      if (version !== VERSION) {
        other ??= { name, version, dir: dirname(path) };
        continue;
      }
      return (require(path) as { default: Tooling }).default;
    }
  }
  if (other) throw new Error(`reylo ${VERSION} runs with ${other.name} ${VERSION}, and the one installed is ${other.version ?? "of no version"} (${other.dir}): npm install -D ${other.name}@${VERSION}`);
  throw new Error(`reylo's build steps run in a database's engine, and none is installed: npm install -D @reylosql/postgres, @reylosql/mysql or @reylosql/sqlite, the one for your database`);
}

function versionAt(manifest: string): string | undefined {
  try {
    return JSON.parse(readFileSync(manifest, "utf8")).version;
  } catch {
    return undefined;
  }
}
