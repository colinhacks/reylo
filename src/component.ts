// What this thread and the engine's worker (src/engine-worker.ts) share about an engine component:
// its exports, how jco's transpiled module instantiates it, and the WASI it is given. The component
// is engine/wasip2, transpiled by jco into its engine package's `engine/` (engine/build.sh), whose
// imports jco's preview2-shim answers: files through `node:fs`, sockets through `node:net`.
import { WASIShim } from "@bytecodealliance/preview2-shim/instantiation";

/** A compiled core module of the component, opaque here: the WebAssembly namespace is not typed through the DOM lib. */
export type CoreModule = unknown;

/** The two exports of an engine component (engine/wasip2/src/lib.rs). */
export interface ComponentExports {
  /** The core's answer to a request that needs no database: `ping`, `lex`, `config`, and `analyze` with the host's answers. */
  call(request: string): string;
  /** The answer to a request on a connection the engine opens itself: `open`, `run`, `query`, `forget`, `close`. */
  serve(request: string): string;
}

/** The transpiled component's `instantiate`, as `jco transpile --instantiation sync` writes it. */
export type Instantiate = (getCoreModule: (name: string) => CoreModule, imports: object) => ComponentExports;

/**
 * The component instantiated over its compiled core modules. The engine reaches the host's files
 * as its own process would (a SQLite database, a root certificate file) and, where `network` is
 * set, opens connections.
 */
export function instantiate(run: Instantiate, modules: Record<string, CoreModule>, network: boolean): ComponentExports {
  const env = Object.entries(process.env).filter((e): e is [string, string] => e[1] !== undefined);
  return run((name) => modules[name]!, new WASIShim({ sandbox: { preopens: { "/": "/" }, env: Object.fromEntries(env), enableNetwork: network } }).getImportObject());
}
