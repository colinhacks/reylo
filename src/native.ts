// Where the `reylo` executable is: the one native program that is the analysis engine of every
// database (`reylo engine <database>`, src/engine.ts). npm installs it as one of reylo's optional dependencies, the package of this
// platform alone (`@reylosql/engine-<os>-<arch>`, each declaring its `os`, `cpu` and `libc`), so it
// is found from reylo's own install, never the project's: the run time never reaches this file, and
// a bundler or a host's file tracing never pulls the binary into a function.
import { execSync } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** The platform packages, by `<platform>-<arch>`, with `-musl` on a musl Linux. */
export const PLATFORM_PACKAGES: Record<string, string> = {
  "darwin-arm64": "@reylosql/engine-darwin-arm64",
  "darwin-x64": "@reylosql/engine-darwin-x64",
  "linux-x64": "@reylosql/engine-linux-x64",
  "linux-x64-musl": "@reylosql/engine-linux-x64-musl",
  "linux-arm64": "@reylosql/engine-linux-arm64",
  "linux-arm64-musl": "@reylosql/engine-linux-arm64-musl",
  "win32-x64": "@reylosql/engine-win32-x64",
  "win32-arm64": "@reylosql/engine-win32-arm64",
};

/**
 * Whether this is a musl Linux (Alpine), whose `-musl` package is the one npm installs. Node's own
 * report names the glibc it runs on, and has no such field on musl; where there is no report,
 * `ldd --version` says `musl`, on musl to its standard error and with a nonzero exit, so both
 * streams are read.
 */
export function isMusl(): boolean {
  if (process.platform !== "linux") return false;
  try {
    const report = process.report.getReport() as unknown as string | { header?: Record<string, unknown> };
    const header = (typeof report === "string" ? JSON.parse(report) : report).header;
    if (header && "glibcVersionRuntime" in header) return !header.glibcVersionRuntime;
  } catch {}
  try {
    return execSync("ldd --version 2>&1", { encoding: "utf8" }).includes("musl");
  } catch (e) {
    const { stdout, stderr } = e as { stdout?: string; stderr?: string };
    return `${stdout ?? ""}${stderr ?? ""}`.includes("musl");
  }
}

/** This platform's key and its package, which is undefined where reylo ships no binary. */
export function platformPackage(): { key: string; pkg: string | undefined } {
  const key = `${process.platform}-${process.arch}${isMusl() ? "-musl" : ""}`;
  return { key, pkg: PLATFORM_PACKAGES[key] };
}

let found: string | undefined;

/**
 * The path of the `reylo` executable: `REYLO_ENGINE_PATH`; else this platform's package, from
 * reylo's own install; else, in a checkout of reylo's repository, its cargo build
 * (`target/release/reylo`, then `target/debug/reylo`). Throws, saying how to get one, where there
 * is none.
 */
export function nativeBinary(): string {
  const named = process.env.REYLO_ENGINE_PATH;
  if (named) {
    if (!existsSync(named)) throw new Error(`REYLO_ENGINE_PATH names ${named}, which does not exist: point it at a reylo executable, or unset it to use the one reylo installs`);
    return named;
  }
  if (found && existsSync(found)) return found;
  const exe = process.platform === "win32" ? "reylo.exe" : "reylo";
  const { key, pkg } = platformPackage();
  if (pkg) {
    const require = createRequire(import.meta.url);
    for (const resolve of [() => join(dirname(require.resolve(`${pkg}/package.json`)), "bin", exe), () => require.resolve(`${pkg}/bin/${exe}`)]) {
      try {
        const path = resolve();
        if (existsSync(path)) return (found = path);
      } catch {}
    }
  }
  const checkout = fileURLToPath(new URL("../target/", import.meta.url));
  for (const profile of ["release", "debug"]) {
    const path = join(checkout, profile, exe);
    if (existsSync(path)) return (found = path);
  }
  const why = pkg ? `${pkg} is not installed (an install with --no-optional, or --omit=optional, leaves it out)` : `reylo ships no binary for ${key}`;
  throw new Error(`reylo's engine, the reylo executable, is missing: ${why}. Install reylo again on a supported platform (${Object.keys(PLATFORM_PACKAGES).join(", ")}), or set REYLO_ENGINE_PATH to a reylo executable; in a checkout of reylo's repository, cargo build --release -p reylo builds one.`);
}
