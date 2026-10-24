// The playground's worker (app/routes/play.tsx): Reylo's Postgres engine and PGlite, Postgres 18
// compiled to WebAssembly, holding the main page's schema, both in the reader's tab and nowhere
// else. It loads when the page opens, then analyzes each statement it is sent
// (lib/live/analyze.ts). A statement that arrives while another is analyzed replaces any still
// waiting, so the latest text wins and typing never queues work.
import type { PGlite } from '@electric-sql/pglite';
import { AnswerCache, type EngineCore, type EngineIo } from '../../../src/engine-core.ts';
import engineUrl from '../../../packages/postgres/engine.wasm?url';
import pgliteWasmUrl from '../../.live/pglite.wasm?url';
import pgliteDataUrl from '../../.live/pglite-data.tar?url';
import snapshotUrl from '../../.live/postgres.tar?url';
import { analyzeStatement } from './analyze.ts';
import type { FromChannel } from './app.tsx';
import { instantiateEngine } from './engine.ts';
import { pgliteIo } from '../../../src/pglite-io.ts';

export type FromWorker = FromChannel;

// The smallest valid module: PGlite fetches initdb.wasm unless it is handed one, and a data
// directory loaded from the snapshot never runs initdb.
const EMPTY_MODULE = new Uint8Array([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]);

type Loaded = { engine: EngineCore; io: EngineIo; cache: AnswerCache; pg: PGlite };
let loading: Promise<Loaded> | undefined;

function load(): Promise<Loaded> {
  return (loading ??= (async () => {
    const { PGlite } = await import('@electric-sql/pglite');
    const [engineModule, pgliteWasmModule, fsBundle, loadDataDir, initdbWasmModule] = await Promise.all([
      WebAssembly.compileStreaming(fetch(engineUrl)),
      WebAssembly.compileStreaming(fetch(pgliteWasmUrl)),
      fetch(pgliteDataUrl).then((r) => r.blob()),
      fetch(snapshotUrl).then((r) => r.blob()),
      WebAssembly.compile(EMPTY_MODULE),
    ]);
    // 16 MB of shared buffers, where the default 128 MB grows PGlite's memory to 189 MB; the
    // schema's catalog fits in far less.
    const pg = await PGlite.create({ pgliteWasmModule, fsBundle, loadDataDir, initdbWasmModule, postgresqlconf: ['shared_buffers = 16MB'] } as never);
    return { engine: await instantiateEngine(engineModule), io: pgliteIo(pg), cache: new AnswerCache(), pg };
  })());
}

let running = false;
let waiting: string | undefined;

async function pump() {
  if (running) return;
  running = true;
  // A turn of the event loop first, so that statements already posted arrive and replace each
  // other: an analysis on PGlite settles in microtasks, so without it none would ever be dropped.
  await new Promise((r) => setTimeout(r, 0));
  while (waiting !== undefined) {
    const sql = waiting;
    waiting = undefined;
    try {
      const { engine, io, cache } = await load();
      postMessage({ kind: 'result', result: await analyzeStatement(engine, io, cache, sql) } satisfies FromWorker);
    } catch (e) {
      postMessage({ kind: 'failed', message: String((e as Error)?.message ?? e) } satisfies FromWorker);
    }
    await new Promise((r) => setTimeout(r, 0));
  }
  running = false;
}

load().then(
  async ({ pg }) => {
    const version = (await pg.query<{ v: string }>("select current_setting('server_version') as v")).rows[0]!.v;
    postMessage({ kind: 'ready', label: `Postgres ${version.split('.')[0]} via PGlite` } satisfies FromWorker);
  },
  (e) => postMessage({ kind: 'failed', message: String((e as Error)?.message ?? e) } satisfies FromWorker),
);

self.onmessage = (event: MessageEvent<{ sql: string }>) => {
  waiting = event.data.sql;
  pump();
};
