// Where the playground's engine is: site/.playground/, which no commit holds. It is Postgres's
// engine core as one WebAssembly component, transpiled by jco into `engine.js` and the
// `engine.core*.wasm` modules that file asks for by name. scripts/playground-engine.ts fills it
// before the dev server or the build starts, from the checkout's own build (target/playground/
// postgres/, which engine/build.sh writes) where there is one, else from the GitHub release of the
// root package.json's version. lib/live/engine.ts and lib/live/worker.ts import it by that path,
// since an import's path is fixed when the file is written; what reads its files names it here.
import path from 'node:path';

/** The directory scripts/playground-engine.ts fills, under the site's root. */
export function engineDir(site: string): string {
  return path.join(site, '.playground');
}
