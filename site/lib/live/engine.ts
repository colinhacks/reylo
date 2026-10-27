// The Postgres engine (packages/postgres/engine, the component jco transpiled) over jco's
// preview2-shim, in the worker and at build time alike: the playground answers the engine's needs
// from PGlite itself (lib/live/pg-io.ts), so only the component's `call` runs, and the shim, its
// browser build in the reader's tab, opens nothing.
import { WASIShim } from '@bytecodealliance/preview2-shim/instantiation';
import { EngineCore, type Instantiate } from '../../../src/engine-core.ts';
// jco's output carries no declarations: src/engine-core.ts types it.
import * as component from '../../../packages/postgres/engine/engine.js';

/** The engine over its compiled core modules, by the names jco gave them (engine.core.wasm, engine.core2.wasm, …). */
export function instantiateEngine(modules: Record<string, WebAssembly.Module>): EngineCore {
  const exports = (component as { instantiate: Instantiate }).instantiate((name) => modules[name], new WASIShim().getImportObject());
  return new EngineCore((request) => exports.call(request));
}
