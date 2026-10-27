// What /play is drawn with at build time (app/routes/play.tsx's loader): the main page's Postgres
// schema, highlighted; the examples, one per subsection of the main page's Inference section, each
// with its types; and what the editor completes, its tables, columns, functions and types. The types come from the same
// engine and the same PGlite snapshot the reader's tab loads (lib/live/plugin.ts writes it before
// the build reads it), through the same function (lib/live/analyze.ts), so the page is complete,
// for every example, before anything loads.
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { AnswerCache } from '../../../src/engine-core.ts';
import { pageFiles } from '../page-files.ts';
import { collectFiles, moduleTypes, pagePostgresSchema, readModules } from '../remark-mapper.ts';
import { formatType, resultType } from '../../../src/type-format.ts';
import { answerJson, REYLO_API } from './api-docs.ts';
import { analyzeStatement, type LiveResult } from './analyze.ts';
import { catalog, CATALOG_QUERIES, type Catalog } from '../../../src/play-catalog.ts';
import { instantiateEngine } from './engine.ts';
import type { Example, PlayData } from './app.tsx';
import { loadHighlighter, tokens } from './highlight.ts';
import { inferenceStatements, START } from './inference.ts';
import { pgliteIo } from './pg-io.ts';

export type { Example, PlayData } from './app.tsx';

let computed: Promise<PlayData> | undefined;

/** The playground's data, computed once per build: the prerender asks for it twice, for the page and for its data. */
export function playData(): Promise<PlayData> {
  return (computed ??= compute());
}

async function compute(): Promise<PlayData> {
  const site = process.cwd();
  const { PGlite } = await import('@electric-sql/pglite');
  const pg = await PGlite.create({ loadDataDir: new Blob([readFileSync(path.join(site, '.live', 'postgres.tar'))]) });
  try {
    const dir = path.join(site, '..', 'packages', 'postgres', 'engine');
    const cores = Object.fromEntries(await Promise.all(readdirSync(dir).filter((n) => n.endsWith('.wasm')).map(async (n) => [n, await WebAssembly.compile(readFileSync(path.join(dir, n)))] as const)));
    const engine = instantiateEngine(cores);
    const io = pgliteIo(pg);
    const h = await loadHighlighter();
    const examples: Example[] = [];
    const cards = collectFiles(pageFiles(site), site).filter((b) => b.inferred && b.dialect === 'postgres');
    const modules = readModules(site);
    for (const { title, slug, sql } of inferenceStatements(readFileSync(path.join(site, 'content', 'sections', '03-inference.mdx'), 'utf8'))) {
      const result = await analyzeStatement(engine, io, new AnswerCache(), sql);
      if (result.diagnostics.length) {
        console.warn(`lib/live/play-data.ts: the Inference example "${title}" is left out: against the main page's Postgres schema, ${result.diagnostics[0]!.message}`);
        continue;
      }
      // A card that shows a schema of its own is an example here only where the main page's schema
      // gives its statement the types the card shows.
      const card = cards.find((b) => b.node.value.replace(/\n$/, '') === sql);
      if (card?.schema !== undefined) {
        const module = modules.postgres?.[card.name!]?.[card.hash];
        const shown = module && moduleTypes(module);
        const flat = (t: string | undefined) => (t === undefined ? '' : formatType(t, Infinity).replace(/\s+/g, ' '));
        if (!shown || flat(shown.params) !== flat(result.params) || flat(resultType(shown.row, shown.cardinality)) !== flat(result.returns)) continue;
      }
      examples.push({
        title,
        slug,
        sql,
        sqlTokens: tokens(h, sql, 'sql'),
        result,
        params: result.params === undefined ? undefined : tokens(h, result.params, 'ts'),
        returns: tokens(h, result.returns, 'ts'),
      });
    }
    if (!examples.some((e) => e.slug === START)) throw new Error(`lib/live/play-data.ts: no Inference example "${START}" to open with`);
    // The schema's comments follow their code by two spaces, where the docs line them up in a
    // column: the playground's pane is narrower than that column.
    const schema = pagePostgresSchema().replace(/(\S)[ \t]{2,}--/g, '$1  --');
    const apiAnswer = answerJson(await analyzeStatement(engine, io, new AnswerCache(), REYLO_API.example));
    return { schema: tokens(h, schema, 'sql'), examples, start: START, catalog: await readCatalog(io), apiAnswer };
  } finally {
    await pg.close();
  }
}

/** The playground's Postgres's catalog, as `reylo play` reads a database's. */
async function readCatalog(io: { query(sql: string, params: unknown[]): Promise<unknown[][]> }): Promise<Catalog> {
  const q = CATALOG_QUERIES.postgres;
  return catalog('postgres', await io.query(q.tables, []), await io.query(q.functions!, []), await io.query(q.types!, []));
}
