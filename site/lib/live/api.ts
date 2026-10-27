// The playground's API on reylo.dev, /play/api/analyze and /play/api/schema: what `reylo play`
// answers on a project's own database (src/play.ts), here on the playground's Postgres, PGlite
// holding the main page's schema, run on the server so that an agent or a script can try a query
// without a database of its own. The engine, the snapshot and the analysis are the ones /play runs
// in the reader's tab (lib/live/worker.ts), so both give the same types. Nothing a caller sends is
// run: a statement is only described, as the type check describes it.
//
// vite.api.config.ts bundles this file into .live/api/play-api.mjs, one plain module that the
// functions in api/play/ import, since a function's own compiler leaves this code's `.ts` imports
// unresolved. The caller names the directory of the live plugin's files (lib/live/plugin.ts).
import { readFile } from 'node:fs/promises';
import type { PGlite } from '@electric-sql/pglite';
import { AnswerCache, type EngineCore, type EngineIo } from '../../../src/engine-core.ts';
import { catalog, CATALOG_QUERIES, type Catalog } from '../../../src/play-catalog.ts';
import { analyzeStatement, type LiveResult } from './analyze.ts';
import { instantiateEngine } from './engine.ts';
import { pgliteIo } from './pg-io.ts';

// The smallest valid module: PGlite compiles initdb.wasm unless it is handed one, and a data
// directory loaded from the snapshot never runs initdb.
const EMPTY_MODULE = new Uint8Array([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0]);

/** The longest statement the API takes: far past any query, short of a request meant to tie up the function. */
const MAX_SQL = 100_000;

const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-methods': 'GET, POST, OPTIONS', 'access-control-allow-headers': 'content-type' };

// A GET's answer depends on its address alone and on the deployment, which a new one replaces, so
// the CDN keeps it; a POST, and any error, is answered fresh.
const KEPT = 'public, max-age=300, s-maxage=86400';

function json(status: number, body: unknown, cache: string): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': cache, 'x-content-type-options': 'nosniff', ...CORS } });
}

type Loaded = { engine: EngineCore; io: EngineIo; label: string; catalog: Catalog };

/**
 * The API over the live plugin's files in `live`: its engine and PGlite are made on the first
 * request and kept while the function's instance stays warm.
 */
export function playApi(live: URL) {
  let loading: Promise<Loaded> | undefined;

  const load = (): Promise<Loaded> => {
    loading ??= (async () => {
      const { PGlite } = await import('@electric-sql/pglite');
      const [engineBytes, pgliteWasm, fsBundle, snapshot] = await Promise.all(['engine.wasm', 'pglite.wasm', 'pglite-data.tar', 'postgres.tar'].map((name) => readFile(new URL(name, live))));
      const pg: PGlite = await PGlite.create({
        pgliteWasmModule: await WebAssembly.compile(pgliteWasm!),
        fsBundle: new Blob([fsBundle!]),
        loadDataDir: new Blob([snapshot!]),
        initdbWasmModule: await WebAssembly.compile(EMPTY_MODULE),
        // 16 MB of shared buffers, as in the reader's tab: the schema's catalog fits in far less.
        postgresqlconf: ['shared_buffers = 16MB'],
      } as never);
      const io = pgliteIo(pg);
      const version = (await pg.query<{ v: string }>("select current_setting('server_version') as v")).rows[0]!.v;
      const q = CATALOG_QUERIES.postgres;
      const tables = catalog('postgres', await io.query(q.tables, []), await io.query(q.functions!, []), await io.query(q.types!, []));
      return { engine: await instantiateEngine(await WebAssembly.compile(engineBytes!)), io, label: `Postgres ${version.split('.')[0]} via PGlite`, catalog: tables };
    })();
    // A load that failed is tried again by the next request, rather than failing every one after it.
    loading.catch(() => (loading = undefined));
    return loading;
  };

  /** A statement's Params, Returns and diagnostics, its types broken over lines wider than `width`. */
  const analyze = async (sql: string, width = Infinity): Promise<LiveResult> => {
    const { engine, io } = await load();
    // A cache of its own per statement, so a warm instance holds nothing a caller sent.
    return analyzeStatement(engine, io, new AnswerCache(), sql, width);
  };

  /** The playground's tables, their columns, functions and types, as `reylo play` gives its database's. */
  const schema = async (): Promise<{ dialect: 'postgres'; database: string } & Catalog> => {
    const { label, catalog } = await load();
    return { dialect: 'postgres', database: label, ...catalog };
  };

  /** /play/api/analyze: `?q=<sql>` (or `?sql=`), or a JSON body `{"sql": "<sql>"}`, and an optional `width`, as in `reylo play`. */
  const handleAnalyze = async (request: Request): Promise<Response> => {
    if (request.method === 'OPTIONS') return new Response(null, { status: 204, headers: CORS });
    const url = new URL(request.url);
    let sql: string | null = url.searchParams.get('q') ?? url.searchParams.get('sql');
    let width = Number(url.searchParams.get('width') ?? Infinity);
    if (request.method === 'POST') {
      let body: { sql?: unknown; q?: unknown; width?: unknown };
      try {
        body = JSON.parse((await request.text()) || '{}');
      } catch {
        return json(400, { error: 'the body is not JSON' }, 'no-store');
      }
      sql = typeof body.sql === 'string' ? body.sql : typeof body.q === 'string' ? body.q : sql;
      if (typeof body.width === 'number') width = body.width;
    } else if (request.method !== 'GET') return json(405, { error: 'GET or POST' }, 'no-store');
    if (sql === null) return json(400, { error: 'name the query: ?q=<sql>, or a JSON body {"sql": "<sql>"}' }, 'no-store');
    if (sql.length > MAX_SQL) return json(413, { error: `the query is longer than ${MAX_SQL} characters` }, 'no-store');
    const result = await analyze(sql, Number.isFinite(width) && width > 0 ? width : Infinity);
    return json(200, result, request.method === 'GET' ? KEPT : 'no-store');
  };

  /** /play/api/schema. */
  const handleSchema = async (request: Request): Promise<Response> => {
    if (request.method === 'OPTIONS') return new Response(null, { status: 204, headers: CORS });
    if (request.method !== 'GET') return json(405, { error: 'GET' }, 'no-store');
    return json(200, await schema(), KEPT);
  };

  return { analyze, schema, handleAnalyze, handleSchema };
}
