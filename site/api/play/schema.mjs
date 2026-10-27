// /play/api/schema, which vercel.json routes here: the playground's tables, their columns,
// functions and types (lib/live/api.ts, bundled by vite.api.config.ts at build time).
import { playApi } from '../../.live/api/play-api.mjs';

const { handleSchema } = playApi(new URL('../../.live/', import.meta.url));

export const GET = handleSchema;
export const OPTIONS = handleSchema;
