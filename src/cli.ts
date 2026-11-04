#!/usr/bin/env node
// The `reylo` command: the native `reylo` binary (src/native.ts finds it), run with this process's
// arguments, standard streams and exit code, but for `reylo play`, the playground's server
// (src/play.ts), which renders each statement's types as TypeScript and so runs here.
//
// npm's `bin` must name a file in the package itself, so this file stands between the command and
// the binary. It waits for the binary rather than replacing itself, which Node cannot do: a signal
// that ends this process (Ctrl-C, a container's stop) is passed on to the binary, and the binary's
// own end is this process's, a signal's as 128 and its number, as a shell reports it.
import { spawn } from "node:child_process";
import { constants } from "node:os";
import { nativeBinary } from "./native.ts";

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
  const child = spawn(binary, args, { stdio: "inherit", windowsHide: true });
  const forward = (signal: NodeJS.Signals) => {
    if (child.exitCode === null && child.signalCode === null) child.kill(signal);
  };
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"] as const) process.on(signal, forward);
  child.on("error", (e) => {
    console.error(`reylo: cannot start ${binary}: ${e.message}`);
    process.exit(1);
  });
  child.on("exit", (code, signal) => process.exit(signal ? 128 + (constants.signals[signal] ?? 0) : (code ?? 1)));
}
