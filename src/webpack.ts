// A webpack-style loader, for webpack 5, Rspack and Next.js with Turbopack, which take loaders
// (not plugins) for a file type. Turbopack takes the same function as `reylo/turbopack`
// (`src/turbopack.ts`), so that each host's config names its own host:
//   webpack / Rspack:  module.rules: [{ test: /\.sql$/, use: "reylo/webpack" }]
//   Turbopack:         turbopack: { rules: { "*.sql": { loaders: ["reylo/turbopack"], as: "*.js" } } }
import { dirname } from "node:path";
import { tooling } from "./engine-package.ts";

type LoaderContext = {
  resourcePath: string;
  addDependency(file: string): void;
  async(): (error: Error | null, code?: string) => void;
};

export default function reyloSqlLoader(this: LoaderContext, source: string): void {
  const done = this.async();
  tooling(dirname(this.resourcePath)).then((t) => t.sqlModule(this.resourcePath, source)).then(
    ({ code, entry }) => {
      // A new analysis of the statement (the type check's, after a migration) rebuilds the module.
      this.addDependency(entry);
      done(null, code);
    },
    (error: Error) => done(error),
  );
}
