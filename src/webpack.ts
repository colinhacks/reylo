// A webpack-style loader, for webpack 5, Rspack and Next.js with Turbopack, which take loaders
// (not plugins) for a file type. Turbopack takes the same function as `reylo/turbopack`
// (`src/turbopack.ts`), so that each host's config names its own host:
//   webpack / Rspack:  module.rules: [{ test: /\.sql$/, use: "reylo/webpack" }]
//   Turbopack:         turbopack: { rules: { "*.sql": { loaders: ["reylo/turbopack"], as: "*.js" } } }
import { dirname } from "node:path";
import { tooling } from "./tooling-loader.ts";

type LoaderContext = {
  resourcePath: string;
  addDependency(file: string): void;
  /** Absent on a host that keeps no result of a loader. */
  cacheable?(flag: boolean): void;
  async(): (error: Error | null, code?: string) => void;
};

export default function reyloSqlLoader(this: LoaderContext, source: string): void {
  // The module changes with the database's schema, which is no file a bundler can watch: a cached
  // result, webpack's on disk above all, would outlive a migration. So the loader runs in every
  // build, and `.reylo/` answers it while the schema is unchanged.
  this.cacheable?.(false);
  const done = this.async();
  tooling(dirname(this.resourcePath)).then((t) => t.sqlModule(this.resourcePath, source)).then(
    ({ code, entry, watched }) => {
      // A new analysis of the statement (the type check's, after a migration) rebuilds the module,
      // and so does a change to the project file or a `.env` file, which the module's text reads.
      for (const file of [entry, ...watched]) this.addDependency(file);
      done(null, code);
    },
    (error: Error) => done(error),
  );
}
