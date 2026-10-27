import { useTheme } from 'fumadocs-ui/provider/base';
import { useCallback } from 'react';
import { githubUrl } from '@/lib/layout.shared';
import { PlayApp, type Channel, type PlayData } from '@/lib/live/app';

// The docs' /play (app/routes/play.tsx): the playground (lib/live/app.tsx) drawn at build time with
// the main page's Postgres schema and every example's types (lib/live/play-data.ts), then analyzing
// in a worker holding Reylo's Postgres engine and PGlite (lib/live/worker.ts). Nothing leaves the tab.
const labels = { loading: 'Starting Postgres 18…', failed: 'Postgres could not start in this browser' };

// The API runs on reylo.dev (lib/live/api.ts), which a local copy of the site does not serve.
const api = {
  analyze: 'https://reylo.dev/play/api/analyze',
  schema: 'https://reylo.dev/play/api/schema',
  page: 'https://reylo.dev/play',
  against: "the page's sample schema",
  example: 'select name from users where id = :id',
};

export function Playground({ data }: { data: PlayData }) {
  const { resolvedTheme, setTheme } = useTheme();
  const connect = useCallback(() => new Worker(new URL('../lib/live/worker.ts', import.meta.url), { type: 'module' }) as unknown as Channel, []);
  return <PlayApp data={data} connect={connect} theme={{ resolved: resolvedTheme, set: setTheme }} labels={labels} home="/" github={githubUrl} api={api} />;
}
