// reylo's build-time half, which each engine package carries as `@reylosql/<database>/tooling`
// (scripts/build.ts bundles it): the module a build step writes for a `.sql` file, the content
// mapper and the CLI, with the analysis, the `.reylo/` cache and the project file behind them.
// reylo's entry points load it the first time they run (src/engine-package.ts).
import type { Tooling } from "./engine-package.ts";
import { releaseConnections } from "./analyze.ts";
import { runCli } from "./commands.ts";
import { sqlModule } from "./loader.ts";
import { serveMapper } from "./mapper-server.ts";

export default { sqlModule, releaseConnections, serveMapper, runCli } satisfies Tooling;
