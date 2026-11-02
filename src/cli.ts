#!/usr/bin/env node
// The `reylo` command. Its commands are the build-time half's (src/commands.ts).
import { tooling } from "./tooling-loader.ts";

try {
  process.exitCode = await (await tooling()).runCli(process.argv.slice(2));
} catch (e) {
  console.error(`reylo: ${(e as Error).message}`);
  process.exitCode = 1;
}
