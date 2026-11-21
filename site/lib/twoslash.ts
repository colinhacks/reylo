import path from 'node:path';
import type { DocsTwoslashOptions } from 'docs-kit/mdx';

// Twoslash runs TypeScript's JavaScript API (6.0, from fumadocs-twoslash), which cannot run a
// content mapper. So a `ts twoslash` block carries, as a hidden file, the module reylo's mapper
// returns for the `.sql` block it imports (`get_user.d.sql.ts`, which `./get_user.sql` resolves to
// under allowArbitraryExtensions; lib/remark-mapper.ts puts it there from lib/sql-modules.json),
// and the `reylo` package that module imports is mounted from the repository's own source: the
// hovers show the real `Query` type and its docs. docs-kit (docs-kit/src/mdx/twoslash.ts) adds
// the cache on disk, the `// @hover:` directive and the trimmed documentation.

/** Twoslash's options for Reylo's page; scripts and tests reach Twoslash through these too. */
export function reyloTwoslash(): DocsTwoslashOptions {
  // The build runs in site/; the reylo package is the repository root above it.
  const repo = path.resolve(process.cwd(), '..');
  return {
    packages: [{ name: 'reylo', root: repo, entries: ['src/index.ts'] }],
    compilerOptions: {
      // `process.env` and node:sqlite in the examples; @types/node is a dev dependency of the
      // site. A block that uses Bun or Cloudflare D1 references their types itself
      // (lib/remark-mapper.ts adds the reference), so no other block sees their globals.
      types: ['node'],
      // `import getUser from "./get_user.sql"` resolves to the hidden `get_user.d.sql.ts`.
      allowArbitraryExtensions: true,
      // The drivers' declarations are not the page's to check: Bun's and Cloudflare's globals
      // overlap Node's and the DOM's.
      skipLibCheck: true,
      // The examples of a Drizzle database are checked on its latest release. Its own Postgres
      // driver is only in the 1.0 release candidates, installed beside it as `drizzle-orm-1`.
      paths: { 'drizzle-orm/postgres/client': [path.join(process.cwd(), 'node_modules/drizzle-orm-1/postgres/client/dist/index.d.ts')] },
    },
    // `Query<…the whole query type…>.run(…)` reads as `run(…)`: the query's own hover shows the
    // type, and repeating it made the popup of every call scroll.
    processHoverInfo: (info) => info.replace(/^Query<[\s\S]*?>\.(?=(?:run|runSync|with)\b)/, ''),
  };
}
