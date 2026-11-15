#!/usr/bin/env node
// The `reylo` command: the native `reylo` binary (src/native.ts finds it), run with this process's
// arguments, standard streams and exit code, but for `reylo play`, the playground's server
// (src/play.ts), which renders each statement's types as TypeScript and so runs here.
//
// npm's `bin` must name a file in the package itself, so this file stands between the command and
// the binary. It waits for the binary rather than replacing itself, which Node cannot do: a signal
// that ends this process (Ctrl-C, a container's stop) is passed on to the binary, and the binary's
// own end is this process's, a signal's as 128 and its number, as a shell reports it.
//
// The migrations run on a DuckDB database through the project's own DuckDB, the shared library
// `@duckdb/node-api`'s platform package installs: this file finds it as Node resolves the package,
// from where the command runs, and names it to the binary in `REYLO_DUCKDB_LIBRARY`
// (engine/migrate/src/duckdb.rs).
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { constants } from "node:os";
import { dirname, join } from "node:path";
import { isMusl, nativeBinary } from "./native.ts";

/** The DuckDB library of the project's `@duckdb/node-api`, or undefined where it has none. */
function duckdbLibrary(): string | undefined {
  try {
    const api = createRequire(join(process.cwd(), "package.json")).resolve("@duckdb/node-api");
    const bindings = createRequire(api).resolve("@duckdb/node-bindings");
    const platform = `${process.platform}-${process.arch}${isMusl() ? "-musl" : ""}`;
    const pkg = createRequire(bindings).resolve(`@duckdb/node-bindings-${platform}/package.json`);
    const file = process.platform === "darwin" ? "libduckdb.dylib" : process.platform === "win32" ? "duckdb.dll" : "libduckdb.so";
    const library = join(dirname(pkg), file);
    return existsSync(library) ? library : undefined;
  } catch {
    return undefined;
  }
}

const args = process.argv.slice(2);

if (args[0] === "play") {
  const { play } = await import("./play.ts");
  process.exitCode = await play(args.slice(1));
} else {
  let binary: string;
  try {
    binary = nativeBinary();
  } catch (e) {
    console.error(`reylo: ${(e as Error).message}`);
    process.exit(1);
  }
  // The handlers go in before the binary starts: once it can answer, a signal has to reach it, rather
  // than end this process and leave it running. A handler runs on a later turn of the event loop, by
  // which time `child` is set.
  let child: ChildProcess;
  const forward = (signal: NodeJS.Signals) => {
    if (child.exitCode === null && child.signalCode === null) child.kill(signal);
  };
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"] as const) process.on(signal, forward);
  const env = { ...process.env };
  if (["migrate", "migration", "db", "run"].includes(args[0] ?? "") && !env.REYLO_DUCKDB_LIBRARY) {
    const library = duckdbLibrary();
    if (library) env.REYLO_DUCKDB_LIBRARY = library;
  }
  child = spawn(binary, args, { stdio: "inherit", windowsHide: true, env });
  child.on("error", (e) => {
    console.error(`reylo: cannot start ${binary}: ${e.message}`);
    process.exit(1);
  });
  child.on("exit", (code, signal) => process.exit(signal ? 128 + (constants.signals[signal] ?? 0) : (code ?? 1)));
}
