#!/usr/bin/env node
// The `reylo` command. Its commands are the engine package's (src/commands.ts).
import { tooling } from "./engine-package.ts";

try {
  process.exitCode = await (await tooling()).runCli(process.argv.slice(2));
} catch (e) {
  console.error(`reylo: ${(e as Error).message}`);
  process.exitCode = 1;
}
