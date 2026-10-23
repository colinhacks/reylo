// The content mapper TypeScript starts (package.json, `typescript.contentMapper`): the engine
// package's (src/mapper-server.ts). Where no engine of reylo's version is installed, one that
// answers every file with the reason, as the type check reports a missing engine everywhere else.
// An engine installed while the process runs takes over: each call looks for it again, and the
// projects opened meanwhile are opened again on it.
import { tooling } from "./engine-package.ts";
import { serve } from "./rpc.ts";

type Handler = (params: unknown) => unknown | Promise<unknown>;
type Handlers = Record<string, Handler>;

const NO_ENGINE_TEXT = `import type { Query } from "reylo";\nexport type Params = unknown;\nexport type Row = unknown;\nexport type QueryError = import("reylo").QueryError;\ndeclare const query: Query<unknown, unknown, "many", "postgres">;\nexport default query;\n`;

let real: Handlers | undefined;
let message = "";
const opened = new Map<string, unknown>();

/** The engine's methods, once an engine of reylo's version is installed. */
async function engine(): Promise<Handlers | undefined> {
  if (real) return real;
  try {
    const handlers = (await tooling()).mapperHandlers();
    for (const params of opened.values()) await handlers.openProject!(params);
    return (real = handlers);
  } catch (e) {
    message = (e as Error).message;
    return undefined;
  }
}

const handle = (method: string, fallback: Handler): Handler => async (params) => {
  const handlers = await engine();
  return handlers ? handlers[method]!(params) : fallback(params);
};

serve({
  initialize: handle("initialize", (params) => ({ protocolVersion: (params as { protocolVersion?: number } | undefined)?.protocolVersion ?? 1, positionEncoding: "utf-8", diagnosticSource: "reylo" })),
  openProject: handle("openProject", (params) => {
    const p = params as { projectHandle: string };
    opened.set(p.projectHandle, params);
    return { configIdentity: message, watchedFiles: [], optionDiagnostics: [] };
  }),
  transform: handle("transform", (params) => {
    const p = params as { fileName: string; content: string };
    // A project file is no query, and its module declares nothing.
    if (p.fileName.endsWith(".reylo.yaml") || p.fileName.endsWith(".reylo.yml")) return { extension: ".ts", diagnostics: [], text: "export {};\n", mappings: [] };
    return { extension: ".ts", diagnostics: [{ messageText: message, start: 0, length: Math.min(1, Buffer.byteLength(p.content)), code: 11 }], text: NO_ENGINE_TEXT, mappings: [] };
  }),
  closeProject: handle("closeProject", (params) => {
    opened.delete((params as { projectHandle: string }).projectHandle);
    return {};
  }),
});
