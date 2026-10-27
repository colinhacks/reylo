// The worker an engine component serves its connections in (src/engine.ts): each message is one
// request line, answered by the component's `serve`, which returns when the answer is whole. The
// shim's sockets block the thread that calls into the component until the network answers, which in
// a worker leaves the thread that sent the request free.
import { parentPort, workerData } from "node:worker_threads";
import { instantiate, type CoreModule, type Instantiate } from "./component.ts";

const { js, modules } = workerData as { js: string; modules: Record<string, CoreModule> };
const engine = instantiate(((await import(js)) as { instantiate: Instantiate }).instantiate, modules, true);
parentPort!.on("message", (line: string) => parentPort!.postMessage(engine.serve(line)));
