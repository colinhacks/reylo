// How the root module reaches the build-time half (src/tooling.ts) when a project calls one of its
// build-time functions, as `analyze`. The root is in every app's bundle, on every runtime, so nothing
// here may reach a bundler or fail on load. It imports no `node:` module and touches `process` only
// when called. It never writes `import.meta`, which is a syntax error in Jest's CommonJS mode and the
// build machine's path in a webpack bundle. And it loads the half with a `require` of a computed
// path, which no bundler follows, where Turbopack fails and Rspack warns on an `import()` of one. A
// `require` loads an ES module that has no top-level await.
import type { StatementDescriptor } from "../spec/index.ts";

/** What `analyze` reads: the statement, and the database where it is not the project's. */
export interface AnalyzeInput {
  /** One statement, with `:name` placeholders on every database. */
  sql: string;
  /**
   * The database, where it is not the project's: `postgres://` or `postgresql://`, `mysql://`, a SQLite
   * file as `file:` or `sqlite:`, or a DuckDB file as `duckdb:`, a relative path read from the working
   * directory. Without it, the database the type check reads: `direct_url` in the project file, else
   * `DATABASE_URL` from the environment or the project's `.env` files.
   */
  url?: string | undefined;
}

export type { StatementDescriptor };

/** The part of the build-time half the root module calls, named here so that a project's type check, which reads the root, never reads the half's Node imports. */
export type BuildTime = {
  /** One statement's descriptor in Reylo Type Defs (src/analyze-api.ts). */
  analyze(input: AnalyzeInput): Promise<StatementDescriptor>;
};

type NodeRequire = ((id: string) => unknown) & { resolve?(id: string): string };
type Host = { getBuiltinModule?(id: string): unknown; cwd?(): string; argv?: string[] };

const CANNOT_RUN = "reylo cannot analyze a statement in this runtime: the engine is a native program, which needs Node 24 or later, or Bun.";

/** The build-time half, from the reylo the project resolves: from the working directory, then from the program's own script (a server started from another directory). */
export function buildTime(): BuildTime {
  const host = (globalThis as { process?: Host }).process;
  const builtin = host?.getBuiltinModule?.bind(host);
  if (!builtin || !host?.cwd) throw new Error(CANNOT_RUN);
  const { createRequire } = builtin("node:module") as { createRequire(from: string): NodeRequire };
  const { dirname, join } = builtin("node:path") as { dirname(path: string): string; join(...parts: string[]): string };
  const bases = [join(host.cwd(), "package.json"), ...(host.argv?.[1] ? [host.argv[1]] : [])];
  for (const base of bases) {
    const require = createRequire(base);
    // Cloudflare's `node:module` gives a `require` with no `.resolve`: this runtime cannot load the
    // engine at all, which is not the same as reylo missing from a project.
    if (typeof require.resolve !== "function") throw new Error(CANNOT_RUN);
    let entry: string;
    try {
      entry = require.resolve("reylo");
    } catch (e) {
      const code = (e as { code?: string }).code;
      if (code === "MODULE_NOT_FOUND" || code === "ERR_PACKAGE_PATH_NOT_EXPORTED") continue;
      throw e;
    }
    // The half is the entry's sibling, compiled (`.js`) or not (`.ts`).
    const extension = entry.slice(entry.lastIndexOf("."));
    return (require(join(dirname(entry), `tooling${extension}`)) as { default: BuildTime }).default;
  }
  throw new Error(`reylo cannot find itself from ${bases.join(" or ")}: install reylo in the project.`);
}
