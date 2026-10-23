// The content mapper TypeScript starts (package.json, `typescript.contentMapper`): the engine
// package's (src/mapper-server.ts). Where no engine of reylo's version is installed, one that
// answers every file with the reason, as the type check reports a missing engine everywhere else.
import { tooling } from "./engine-package.ts";
import { serve } from "./rpc.ts";

try {
  (await tooling()).serveMapper();
} catch (e) {
  const message = (e as Error).message;
  const text = `import type { Query } from "reylo";\nexport type Params = unknown;\nexport type Row = unknown;\ndeclare const query: Query<unknown, unknown, "many", "postgres">;\nexport default query;\n`;
  serve({
    initialize: (params) => ({ protocolVersion: (params as { protocolVersion?: number } | undefined)?.protocolVersion ?? 1, positionEncoding: "utf-8", diagnosticSource: "reylo" }),
    openProject: () => ({ configIdentity: message, watchedFiles: [], optionDiagnostics: [] }),
    transform(params) {
      const p = params as { fileName: string; content: string };
      // A project file is no query, and its module declares nothing.
      if (p.fileName.endsWith(".reylo.yaml") || p.fileName.endsWith(".reylo.yml")) return { extension: ".ts", diagnostics: [], text: "export {};\n", mappings: [] };
      return { extension: ".ts", diagnostics: [{ messageText: message, start: 0, length: Math.min(1, Buffer.byteLength(p.content)), code: 11 }], text, mappings: [] };
    },
    closeProject: () => ({}),
  });
}
