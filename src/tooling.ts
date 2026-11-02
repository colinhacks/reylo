// reylo's build-time half: the module a build step writes for a `.sql` file, the content mapper and
// the CLI's TypeScript commands, with the analysis, the `.reylo/` cache, the project file and the
// engine behind them. reylo's entry points load it the first time they run (src/tooling-loader.ts),
// so that the run time a project imports never reaches it.
import type { Tooling } from "./tooling-loader.ts";
import { releaseConnections } from "./analyze.ts";
import { runCli } from "./commands.ts";
import { sqlModule } from "./loader.ts";
import { mapperHandlers } from "./mapper-server.ts";

export default { sqlModule, releaseConnections, mapperHandlers, runCli } satisfies Tooling;
