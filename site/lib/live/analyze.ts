// One statement as the playground shows it (src/play-result.ts), analyzed in the reader's tab by the
// worker (lib/live/worker.ts), at build time by the page's loader (lib/live/play-data.ts) for the
// examples, and on the server by the playground's API (lib/live/api.ts), so one function answers all three.
import type { AnswerCache, EngineCore, EngineIo } from '../../../src/engine-core.ts';
import { playResult, type PlayResult } from '../../../src/play-result.ts';

export type LiveResult = PlayResult;
export type LiveDiagnostic = PlayResult['diagnostics'][number];

/** The width a type is broken at: what the playground's column of types holds. */
export const TYPE_WIDTH = 46;

export async function analyzeStatement(engine: EngineCore, io: EngineIo, cache: AnswerCache, sql: string, width = TYPE_WIDTH): Promise<LiveResult> {
  const t = performance.now();
  let lexed;
  try {
    lexed = engine.lex(sql, 'postgres');
  } catch {}
  const options = { width };
  try {
    const analysis = await engine.analyze(sql, 'postgres', io, cache);
    return playResult(sql, 'postgres', { analysis, lexed }, { ...options, ms: performance.now() - t });
  } catch (e) {
    return playResult(sql, 'postgres', { error: e as Error }, { ...options, ms: performance.now() - t });
  }
}
