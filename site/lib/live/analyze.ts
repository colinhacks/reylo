// One statement as the playground shows it: the diagnostics the content mapper places in the file,
// by line and column, and its Params and Returns as the Inference cards print them. The worker
// (lib/live/worker.ts) runs it in the reader's tab, and the page's loader (lib/live/play-data.ts)
// at build time for the statement the page opens with, so the same function answers both.
import type { AnswerCache, EngineCore, EngineIo } from '../../../src/engine-core.ts';
import { moduleTypes } from '../../../src/module-types.ts';
import { formatType, resultType } from '../type-format.ts';
import { placer } from './place.ts';

export type LiveDiagnostic = { message: string; code: number; line: number; column: number; endLine: number; endColumn: number };

export type LiveResult = {
  sql: string;
  /** `{ id: number }`; absent where the statement takes no parameters, as on the cards. */
  params?: string;
  /** What `.run()` resolves to. */
  returns: string;
  diagnostics: LiveDiagnostic[];
  ms: number;
};

/** The width a type is broken at: what the playground's column of types holds. */
const TYPE_WIDTH = 46;

export async function analyzeStatement(engine: EngineCore, io: EngineIo, cache: AnswerCache, sql: string): Promise<LiveResult> {
  const t = performance.now();
  const place = placer(sql);
  let lexed;
  try {
    lexed = engine.lex(sql, 'postgres');
  } catch {}
  let params: string | undefined;
  let returns: string;
  let diagnostics;
  try {
    const analysis = await engine.analyze(sql, 'postgres', io, cache);
    const types = moduleTypes(sql, analysis, 'postgres', lexed, true);
    diagnostics = types.diagnostics;
    params = types.params === 'Record<never, never>' ? undefined : formatType(types.params, TYPE_WIDTH);
    returns = formatType(types.row === 'unknown' ? 'unknown' : resultType(types.row, types.cardinality), TYPE_WIDTH);
  } catch (e) {
    // As the content mapper places an analysis that fails: one diagnostic at the file's start.
    diagnostics = [{ message: (e as Error).message, start: 0, length: 1, code: 5 }];
    params = 'unknown';
    returns = 'unknown';
  }
  return { sql, params, returns, diagnostics: diagnostics.map((d) => ({ message: d.message, code: d.code, ...place(d.start, d.length) })), ms: performance.now() - t };
}
