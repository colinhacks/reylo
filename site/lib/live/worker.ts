// The playground's worker (app/routes/play.tsx): Reylo's Postgres engine and PGlite, Postgres 18
// compiled to WebAssembly, holding the main page's schema, both in the reader's tab and nowhere
// else. It loads when the page opens, then analyzes each statement it is sent
// (lib/live/analyze.ts). A statement that arrives while another is analyzed replaces any still
// waiting, so the latest text wins and typing never queues work. A schema it is sent replaces the
// one it holds, in one transaction, so DDL that fails leaves the previous schema as it was; then it
// sends what the editor completes from the new one and analyzes the last statement again.
import type { Extension, PGlite } from '@electric-sql/pglite';
import { AnswerCache, type EngineCore, type EngineIo } from '../../../src/engine-core.ts';
import pgliteWasmUrl from '../../.live/pglite.wasm?url';
import pgliteDataUrl from '../../.live/pglite-data.tar?url';
import snapshotUrl from '../../.live/postgres.tar?url';
import { analyzeStatement } from './analyze.ts';
import type { FromChannel, ToChannel } from './app.tsx';
import { instantiateEngine } from './engine.ts';
import { pgliteIo, readCatalog } from './pg-io.ts';
import { pgJsonschemaStub } from './pg-stub.ts';

export type FromWorker = FromChannel;

// The smallest valid module: PGlite fetches initdb.wasm unless it is handed one, and a data
// directory loaded from the snapshot never runs initdb.
const EMPTY_MODULE = new Uint8Array([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]);

type Loaded = { engine: EngineCore; io: EngineIo; cache: AnswerCache; pg: PGlite };

// The engine component's core modules, by their file names, which jco's output asks for by name.
const coreUrls = Object.fromEntries(Object.entries(import.meta.glob<string>('../../../packages/postgres/engine/*.wasm', { query: '?url', import: 'default', eager: true })).map(([path, url]) => [path.slice(path.lastIndexOf('/') + 1), url]));
let loading: Promise<Loaded> | undefined;

function load(): Promise<Loaded> {
  return (loading ??= (async () => {
    const { PGlite } = await import('@electric-sql/pglite');
    const [engineModules, pgliteWasmModule, fsBundle, loadDataDir, initdbWasmModule] = await Promise.all([
      // Compiled from their bytes: the smallest are inlined as data URLs, which compileStreaming refuses for their type.
      Promise.all(Object.entries(coreUrls).map(async ([name, url]) => [name, await WebAssembly.compile(await (await fetch(url)).arrayBuffer())] as const)).then(Object.fromEntries),
      WebAssembly.compileStreaming(fetch(pgliteWasmUrl)),
      fetch(pgliteDataUrl).then((r) => r.blob()),
      fetch(snapshotUrl).then((r) => r.blob()),
      WebAssembly.compile(EMPTY_MODULE),
    ]);
    // 16 MB of shared buffers, where the default 128 MB grows PGlite's memory to 189 MB; the
    // schema's catalog fits in far less.
    // The pg_jsonschema stub's files (lib/live/pg-stub.ts), which the snapshot's data directory does
    // not hold, so that a schema of the reader's own can create the extension again.
    const extensions = { pgJsonschemaStub: pgJsonschemaStub as unknown as Extension };
    const pg = await PGlite.create({ pgliteWasmModule, fsBundle, loadDataDir, initdbWasmModule, extensions, postgresqlconf: ['shared_buffers = 16MB'] } as never);
    return { engine: instantiateEngine(engineModules), io: pgliteIo(pg), cache: new AnswerCache(), pg };
  })());
}

// Every schema but Postgres's own goes, with all it holds, and the settings a schema's DDL may have
// set, such as its search_path; then an empty public schema, as a new database has.
const EMPTY = `reset all;
do $$ declare s name; begin
  for s in select nspname from pg_namespace where nspname !~ '^pg_' and nspname <> 'information_schema' loop
    execute format('drop schema %I cascade', s);
  end loop;
end $$;
create schema public;`;

// DDL that creates pg_jsonschema itself; any other has the stub created first, as the sample's
// first line does, so a CHECK may call jsonb_matches_schema either way.
const CREATES_JSONSCHEMA = /\bcreate\s+extension\s+(?:if\s+not\s+exists\s+)?"?pg_jsonschema\b/i;

/** The line and column of a character offset in `text`, as Postgres reports an error's position: 1-based, in characters. */
function lineColumn(text: string, position: number): { line: number; column: number } {
  const before = [...text].slice(0, Math.max(0, position - 1)).join('');
  const lines = before.split('\n');
  return { line: lines.length, column: lines.at(-1)!.length + 1 };
}

async function replaceSchema(ddl: string, id: number): Promise<FromWorker> {
  const { pg, io, cache } = await load();
  try {
    await pg.transaction(async (tx) => {
      await tx.exec(EMPTY);
      if (!CREATES_JSONSCHEMA.test(ddl)) await tx.exec('create extension pg_jsonschema');
      await tx.exec(ddl);
    });
  } catch (e) {
    const error = e as Error & { position?: string; code?: string };
    const position = Number(error.position);
    return { kind: 'schema', id, ok: false, message: String(error.message ?? e), ...(error.code ? { code: error.code } : {}), ...(position > 0 ? lineColumn(ddl, position) : {}) };
  }
  cache.clear();
  return { kind: 'schema', id, ok: true, catalog: await readCatalog(io) };
}

let running = false;
let waiting: string | undefined;
let schema: { ddl: string; id: number } | undefined;
// The statement analyzed last, analyzed again against a new schema.
let last: string | undefined;

async function pump() {
  if (running) return;
  running = true;
  // A turn of the event loop first, so that statements already posted arrive and replace each
  // other: an analysis on PGlite settles in microtasks, so without it none would ever be dropped.
  await new Promise((r) => setTimeout(r, 0));
  while (schema !== undefined || waiting !== undefined) {
    if (schema !== undefined) {
      const { ddl, id } = schema;
      schema = undefined;
      try {
        const reply = await replaceSchema(ddl, id);
        postMessage(reply);
        if (reply.kind === 'schema' && reply.ok) waiting ??= last;
      } catch (e) {
        postMessage({ kind: 'schema', id, ok: false, message: String((e as Error)?.message ?? e) } satisfies FromWorker);
      }
      continue;
    }
    const sql = waiting!;
    waiting = undefined;
    last = sql;
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

self.onmessage = (event: MessageEvent<ToChannel>) => {
  const m = event.data;
  if ('schema' in m) schema = { ddl: m.schema, id: m.id };
  else waiting = m.sql;
  pump();
};
