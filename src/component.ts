// What this thread and the engine's worker (src/engine-worker.ts) share about an engine component:
// the WASI it is given. The component is engine/wasip2, transpiled by jco into its engine package's
// `engine/` (engine/build.sh), whose imports jco's preview2-shim answers: files through `node:fs`,
// sockets through `node:net`. Its exports are typed in src/engine-core.ts, which imports nothing,
// so that the docs site types them without the shim's Node build.
import { resolve } from "node:path";
import type { ComponentExports, CoreModule, Instantiate } from "./engine-core.ts";

export type { ComponentExports, CoreModule, Instantiate } from "./engine-core.ts";

type Shim = typeof import("@bytecodealliance/preview2-shim/instantiation");
let shim: Promise<Shim> | undefined;

/**
 * jco's preview2-shim, imported once per thread. Importing it starts its I/O worker, which must not
 * inherit Node's watch-mode report (`node --watch` sets WATCH_REPORT_DEPENDENCIES in the process it
 * runs): a worker under it reports each module it loads to the thread that started it as a message,
 * and the shim fails on a message that is not its own. The variable is hidden while the shim loads.
 */
function loadShim(): Promise<Shim> {
  return (shim ??= (async () => {
    const report = process.env.WATCH_REPORT_DEPENDENCIES;
    delete process.env.WATCH_REPORT_DEPENDENCIES;
    try {
      return await import("@bytecodealliance/preview2-shim/instantiation");
    } finally {
      if (report !== undefined) process.env.WATCH_REPORT_DEPENDENCIES = report;
    }
  })());
}

/**
 * A path on this machine as the component reaches it through the preopen `/`: the same path, or on
 * Windows the path with forward slashes under `/` (`C:\app\dev.db` is `/C:/app/dev.db`, a share's
 * `\\host\share\db` is `/host/share/db`), which the shim maps back to the drive or the share.
 */
export function guestPath(path: string, platform: NodeJS.Platform = process.platform): string {
  return platform === "win32" ? `/${path.replace(/\\/g, "/").replace(/^\/+/, "")}` : path;
}

/**
 * The component instantiated over its compiled core modules. The engine reaches the host's files
 * as its own process would (a SQLite database, a root certificate file), with `NODE_EXTRA_CA_CERTS`
 * read from where Node reads it, and, where `network` is set, opens connections.
 */
export async function instantiate(run: Instantiate, modules: Record<string, CoreModule>, network: boolean): Promise<ComponentExports> {
  const { WASIShim } = await loadShim();
  const env = Object.fromEntries(Object.entries(process.env).filter((e): e is [string, string] => e[1] !== undefined));
  if (env.NODE_EXTRA_CA_CERTS) env.NODE_EXTRA_CA_CERTS = guestPath(resolve(env.NODE_EXTRA_CA_CERTS));
  return run((name) => modules[name]!, new WASIShim({ sandbox: { preopens: { "/": "/" }, env, enableNetwork: network } }).getImportObject());
}
