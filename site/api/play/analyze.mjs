// /play/api/analyze, which vercel.json routes here, and /play?q= asked for JSON: a statement's
// Params, Returns and diagnostics (lib/live/api.ts, bundled by vite.api.config.ts at build time).
import { playApi } from '../../.live/api/play-api.mjs';

const { handleAnalyze } = playApi(new URL('../../.live/', import.meta.url));

export const GET = handleAnalyze;
export const POST = handleAnalyze;
export const OPTIONS = handleAnalyze;
