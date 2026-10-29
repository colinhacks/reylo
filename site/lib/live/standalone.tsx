// `reylo play`'s page (src/play.ts serves it, built by vite.play.config.ts): the playground
// (lib/live/app.tsx) on the project's own database, which the server reads at `/api/page` and
// analyzes each statement against at `/api/analyze`. It opens on the address's `?q=`, or on a
// query of the schema's first table.
import '../../play-page/fonts.css';
import '../../app/global.css';
import { useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import type { Catalog } from '../../../src/play-catalog.ts';
import { TYPE_WIDTH } from './analyze.ts';
import { PlayApp, type Channel, type FromChannel, type Theme } from './app.tsx';

type Page = { dialect: string; label: string; schema: string; catalog: Catalog };

// Where `reylo play` analyzes each query: the project's own database, from this machine.
const about = {
  runs: "Each query is analyzed against your project's database, the one its type check reads, by reylo play on this machine. Nothing is run.",
};

const labels = { loading: 'Connecting…', failed: 'The playground stopped answering' };

/** The server behind a worker's interface: one statement at a time, the latest text replacing any still waiting. */
function serverChannel(label: string): Channel {
  let running = false;
  let waiting: string | undefined;
  let stopped = false;
  const channel: Channel = {
    onmessage: null,
    onerror: null,
    // A statement; the schema is the project's own, never replaced from the page.
    postMessage(message) {
      if (!('sql' in message)) return;
      waiting = message.sql;
      void pump();
    },
    terminate() {
      stopped = true;
    },
  };
  const emit = (data: FromChannel) => !stopped && channel.onmessage?.({ data } as MessageEvent<FromChannel>);
  async function pump() {
    if (running) return;
    running = true;
    while (waiting !== undefined && !stopped) {
      const sql = waiting;
      waiting = undefined;
      try {
        const res = await fetch('/api/analyze', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sql, width: TYPE_WIDTH }) });
        const body = await res.json();
        if (!res.ok) throw new Error(body.error ?? res.statusText);
        emit({ kind: 'result', result: body });
      } catch (e) {
        emit({ kind: 'failed', message: (e as Error).message });
      }
    }
    running = false;
  }
  setTimeout(() => emit({ kind: 'ready', label }), 0);
  return channel;
}

function Play({ page }: { page: Page }) {
  const [resolved, setResolved] = useState(document.documentElement.classList.contains('light') ? 'light' : 'dark');
  const theme: Theme = {
    resolved,
    set(next) {
      document.documentElement.classList.replace(resolved, next);
      localStorage.setItem('reylo-play-theme', next);
      setResolved(next);
    },
  };
  const data = useMemo(() => {
    const table = page.catalog.tables[0]?.name;
    const query = new URLSearchParams(location.search).get('q') ?? (table ? `select * from ${table}` : 'select 1');
    return { schema: page.schema, examples: [], query, catalog: page.catalog };
  }, [page]);
  const connect = useMemo(() => () => serverChannel(page.label), [page]);
  const api = useMemo(() => {
    const table = page.catalog.tables[0]?.name;
    return { analyze: `${location.origin}/api/analyze`, schema: `${location.origin}/api/schema`, against: "your project's database", example: table ? `select * from ${table}` : 'select 1' };
  }, [page]);
  return <PlayApp data={data} connect={connect} theme={theme} labels={labels} home="https://reylo.dev" github="https://github.com/colinhacks/reylo" api={api} about={about} />;
}

const root = createRoot(document.getElementById('root')!);
fetch('/api/page')
  .then((r) => r.json() as Promise<Page>)
  .then(
    (page) => root.render(<Play page={page} />),
    (e: Error) => root.render(<p style={{ padding: 20 }}>The playground's server cannot be reached: {e.message}</p>),
  );
