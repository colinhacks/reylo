// One statement as the playground shows it (src/play-result.ts), analyzed in the reader's tab by the
// worker (lib/live/worker.ts) and at build time by the page's loader (lib/live/play-data.ts) for
// the examples, so the same function answers both.
import type { AnswerCache, EngineCore, EngineIo } from '../../../src/engine-core.ts';
import { playResult, type PlayResult } from '../../../src/play-result.ts';

export type LiveResult = PlayResult;
export type LiveDiagnostic = PlayResult['diagnostics'][number];

/** The width a type is broken at: what the playground's column of types holds. */
export const TYPE_WIDTH = 46;

export async function analyzeStatement(engine: EngineCore, io: EngineIo, cache: AnswerCache, sql: string): Promise<LiveResult> {
  const t = performance.now();
  let lexed;
  try {
    lexed = engine.lex(sql, 'postgres');
  } catch {}
  const options = { width: TYPE_WIDTH };
  try {
    const analysis = await engine.analyze(sql, 'postgres', io, cache);
    return playResult(sql, 'postgres', { analysis, lexed }, { ...options, ms: performance.now() - t });
  } catch (e) {
    return playResult(sql, 'postgres', { error: e as Error }, { ...options, ms: performance.now() - t });
  }
}
