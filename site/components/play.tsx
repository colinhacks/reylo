import { useTheme } from 'fumadocs-ui/provider/base';
import { useCallback, useMemo } from 'react';
import { githubUrl } from '@/lib/layout.shared';
import { REYLO_API } from '@/lib/live/api-docs';
import { PlayApp, type Channel, type PlayData } from '@/lib/live/app';

// The docs' /play (app/routes/play.tsx): the playground (lib/live/app.tsx) drawn at build time with
// the main page's Postgres schema and every example's types (lib/live/play-data.ts), then analyzing
// in a worker holding Reylo's Postgres engine and PGlite (lib/live/worker.ts). Nothing leaves the tab.
// The reader may replace the schema with their own, which the worker's PGlite then holds.
const labels = { loading: 'Starting Postgres 18…', failed: 'Postgres could not start in this browser' };

// Where the playground analyzes each query: here, all of it in the reader's tab.
const about = {
  runs: "Postgres 18 runs in this tab, compiled to WebAssembly (PGlite), holding a sample schema of users, posts and events, or your own: View schema shows it, and Edit replaces it. Reylo's engine analyzes each query against it, also in this tab: nothing is run, and nothing is sent anywhere.",
};

// The one line a reader arriving from a link reads first, centred in the bar.
const pitch = (
  <>
    A type inference engine for Postgres, SQLite, and MySQL. <a href={githubUrl}>Go to repo 👉</a>
  </>
);

export function Playground({ data }: { data: PlayData }) {
  // The API's example answer as the build analyzed it: the dialog shows it without asking a function that may be cold.
  const api = useMemo(() => ({ ...REYLO_API, ...(data.apiAnswer ? { answer: data.apiAnswer } : {}) }), [data]);
  const { resolvedTheme, setTheme } = useTheme();
  const connect = useCallback(() => new Worker(new URL('../lib/live/worker.ts', import.meta.url), { type: 'module' }) as unknown as Channel, []);
  return <PlayApp data={data} connect={connect} theme={{ resolved: resolvedTheme, set: setTheme }} labels={labels} home="/" github={githubUrl} api={api} about={about} pitch={pitch} schemaEditable />;
}
