// reylo's build-time half: the module a build step writes for a `.sql` file and the content mapper,
// with the analysis, the cache in `reylo/.cache/`, the project file and the engine behind them. reylo's
// entry points load it the first time they run (src/tooling-loader.ts), so that the run time a
// project imports never reaches it.
import type { Tooling } from "./tooling-loader.ts";
import { releaseConnections } from "./analyze.ts";
import { sqlModule } from "./loader.ts";
import { mapperHandlers } from "./mapper-server.ts";
import { compile } from "./compile.ts";

export default { compile, sqlModule, releaseConnections, mapperHandlers } satisfies Tooling;
