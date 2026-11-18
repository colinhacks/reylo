import { useEffect, useState, type ReactNode } from 'react';
import { wordmark } from './wordmark';
import { githubUrl, migrateDocsUrl } from '@/lib/layout.shared';

/**
 * Reylo Migrate's landing page, at /migrate (app/routes/migrate.tsx): a type-led hero, the mark
 * behind it, then one row per feature, its words on one side and a terminal or an editor playing it
 * on the other, the sides alternating. Every line a terminal prints is the docs' own
 * (content/docs/migrate.mdx), and the docs are a page of their own, at /migrate/docs. The players
 * run on timers from the moment the page loads; nothing here reads the scroll position.
 */
export function MigrateLanding() {
  return (
    <div className="ml">
      <nav className="ml-nav">
        <a className="ml-brand" href="/migrate">
          <Mark />
          Reylo Migrate
        </a>
        <div className="ml-links">
          <a href={migrateDocsUrl}>Docs</a>
          <a href="/">Reylo</a>
          <a href={githubUrl}>GitHub</a>
        </div>
      </nav>

      <header className="ml-hero">
        <Mark className="ml-ghost" />
        <span className="ml-eyebrow">
          <Mark />
          Reylo Migrate
        </span>
        <h1 className="ml-h1">
          SQL-first database migrations <em>for busy teams</em>
        </h1>
        <p className="ml-lede">
          Keep your whole schema in one <code>schema.sql</code>, and Reylo Migrate writes each migration as plain SQL. One binary, for Postgres, MySQL and SQLite.
        </p>
        <Install />
      </header>

      <Row title="Change the schema, get the migration" visual={<SchemaToMigration />}>
        <p>
          Edit <code>schema.sql</code> and run one command. The migration is plain SQL, the change and nothing else, and yours to read and edit.
        </p>
      </Row>

      <Row title="Everything your database can hold" visual={<SchemaKinds />} flip>
        <p>Enums, domains, generated columns, exclusion constraints, partial indexes, materialized views, functions, triggers, row level security and grants, declared as you would on the database itself.</p>
      </Row>

      <Row title="Nothing drops until you say so" visual={<Term lines={DROPS} />}>
        <p>
          A change that loses data is refused until <code>--drop</code> names it. A rename keeps its rows, and where a rename and a drop look alike, Reylo Migrate asks.
        </p>
      </Row>

      <Row title="Try a change on every save" visual={<Term lines={PUSH} />} flip>
        <p>
          A push takes your development database to <code>schema.sql</code> as you edit, with <code>--watch</code> on every save, and the migration comes when the change is right.
        </p>
      </Row>

      <Row title="Dry runs before every deploy" visual={<Term lines={DRY_RUN} />}>
        <p>
          Your migrations replay on throwaway shadow databases: against production's schema, against <code>schema.sql</code>, and with what's pending tried on top. Nothing runs on yours.
        </p>
      </Row>

      <Row title="Hand changes stop the deploy" visual={<Term lines={HAND_CHANGE} />} flip>
        <p>Each migration stores a fingerprint of the schema it leaves. A column added in a dashboard no longer matches it, so the deploy refuses and shows the change, with the ways to keep it.</p>
      </Row>

      <section className="ml-end">
        <h2>Start from the database you have</h2>
        <p>
          Run <code>reylo migrate init</code>, and it writes your <code>schema.sql</code> and a first migration from the database.
        </p>
        <Install />
      </section>

      <footer className="ml-foot">
        <a href={migrateDocsUrl}>Docs</a>
        <a href="/">Reylo</a>
        <a href={githubUrl}>GitHub</a>
        <span>Released under the MIT License. Copyright © 2026 Colin McDonnell.</span>
      </footer>
    </div>
  );
}

function Mark({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 400 235.47" fill="currentColor" aria-hidden="true">
      <path d={wordmark.mark} />
    </svg>
  );
}

/** A feature: its name and a sentence or two, beside what it does. `flip` puts the words on the right. */
function Row({ title, visual, flip, children }: { title: string; visual: ReactNode; flip?: boolean; children: ReactNode }) {
  return (
    <section className={flip ? 'ml-row ml-flip' : 'ml-row'}>
      <div className="ml-row-text">
        <h2>{title}</h2>
        {children}
      </div>
      <div className="ml-row-visual">{visual}</div>
    </section>
  );
}

const INSTALLS = { brew: 'brew install reylosql/tap/reylo', npm: 'npm install reylo', curl: 'curl -fsSL https://reylo.dev/install.sh | sh' } as const;

/** The install command, by package manager, with a copy button, and the docs beside it. */
function Install() {
  const [kind, setKind] = useState<keyof typeof INSTALLS>('brew');
  const [copied, setCopied] = useState(false);
  const command = INSTALLS[kind];
  return (
    <div className="ml-install">
      <div className="ml-big">
        <div className="ml-cmd">
          <span className="ml-dollar">$</span>
          <span>{command}</span>
          <button
            type="button"
            className="ml-copybtn"
            aria-label="Copy the command"
            onClick={() => {
              void navigator.clipboard.writeText(command);
              setCopied(true);
              setTimeout(() => setCopied(false), 1400);
            }}
          >
            {copied ? (
              <svg viewBox="0 0 16 16" aria-hidden="true">
                <path d="M3 8.5l3.2 3.2L13 4.8" />
              </svg>
            ) : (
              <svg viewBox="0 0 16 16" aria-hidden="true">
                <rect x="5.25" y="5.25" width="8.5" height="8.5" rx="1.75" />
                <path d="M10.75 5.25v-1.5a1.5 1.5 0 0 0-1.5-1.5h-5.5a1.5 1.5 0 0 0-1.5 1.5v5.5a1.5 1.5 0 0 0 1.5 1.5h1.5" />
              </svg>
            )}
          </button>
        </div>
        <a className="ml-docs" href={migrateDocsUrl}>
          Read the docs
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M2.5 8h11M9 3.5 13.5 8 9 12.5" />
          </svg>
        </a>
      </div>
      <div className="ml-alt">
        or
        {(Object.keys(INSTALLS) as (keyof typeof INSTALLS)[]).map((k) => (
          <button key={k} type="button" className={k === kind ? 'on' : undefined} onClick={() => setKind(k)}>
            {k}
          </button>
        ))}
      </div>
    </div>
  );
}

/** Whether the reader asked for less motion: then every player shows its last frame and stays. */
function stillPlease(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/** A terminal session, line by line: a `$ ` line is typed, any other printed, and `#edit` is the moment the editor beside it changes. */
type Script = readonly string[];

type Line = { prompt: boolean; text: string; cursor?: boolean };

/** Every line of a session at once: what the page renders first, and what a reader who asked for less motion keeps. */
const finished = (script: Script): Line[] => script.filter((l) => l !== '#edit').map((l) => (l.startsWith('$ ') ? { prompt: true, text: l.slice(2) } : { prompt: false, text: l }));

/** Plays a script on timers: each command typed, its output a line at a time, a pause, and again. */
function usePlayer(script: Script, onEdit?: (edited: boolean) => void): Line[] {
  const [lines, setLines] = useState<Line[]>(() => finished(script));
  useEffect(() => {
    if (stillPlease()) return;
    let alive = true;
    const sleep = (ms: number) => new Promise<void>((resolve, reject) => setTimeout(() => (alive ? resolve() : reject(new Error('stopped'))), ms));
    (async () => {
      for (;;) {
        onEdit?.(false);
        setLines([]);
        await sleep(700);
        for (const l of script) {
          if (l === '#edit') {
            await sleep(500);
            onEdit?.(true);
          } else if (!l.startsWith('$ ')) {
            await sleep(l === '' ? 250 : 110);
            setLines((ls) => [...ls, { prompt: false, text: l }]);
          } else {
            const text = l.slice(2);
            await sleep(450);
            setLines((ls) => [...ls, { prompt: true, text: '', cursor: true }]);
            for (let i = 1; i <= text.length; i++) {
              await sleep(24 + Math.random() * 40);
              const typed = text.slice(0, i);
              setLines((ls) => [...ls.slice(0, -1), { prompt: true, text: typed, cursor: true }]);
            }
            await sleep(350);
            setLines((ls) => [...ls.slice(0, -1), { prompt: true, text }]);
          }
        }
        setLines((ls) => [...ls, { prompt: true, text: '', cursor: true }]);
        await sleep(5000);
      }
    })().catch(() => {});
    return () => {
      alive = false;
    };
    // The script and its callback are module constants and a state setter, the same on every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return lines;
}

/** A printed line's marks in their colours: ✓ and + green, ✗ red, ! and · yellow; a command it names bright. */
function paint(line: string): ReactNode {
  const m = line.match(/^(\s*)(✓|✗|!|·|\+)(\s.*)?$/);
  if (m) {
    const cls = m[2] === '✓' || m[2] === '+' ? 'ml-ok' : m[2] === '✗' ? 'ml-er' : 'ml-wn';
    return (
      <>
        {m[1]}
        <span className={cls}>{m[2]}</span>
        {m[3]}
      </>
    );
  }
  if (/^\s+reylo /.test(line)) return <span className="ml-c">{line}</span>;
  if (line === 'Nothing was changed.') return <span className="ml-mu">{line}</span>;
  return line;
}

/** A terminal: the session so far, over the whole session drawn invisibly, which holds the window at its final height as it plays. */
function TermWindow({ lines, script, className }: { lines: Line[]; script: Script; className?: string }) {
  const draw = (ls: Line[]) =>
    ls.map((l, i) => (
      <div key={i} className="ml-ln">
        {l.prompt ? <span className="ml-ps">$ </span> : null}
        {l.prompt ? <span className="ml-c">{l.text}</span> : paint(l.text)}
        {l.cursor ? <span className="ml-cur" /> : null}
      </div>
    ));
  return (
    <div className={className ? `ml-win ml-terminal ${className}` : 'ml-win ml-terminal'} aria-hidden="true">
      <div className="ml-bar">
        <span className="ml-dots">
          <i />
          <i />
          <i />
        </span>
        ~/app
      </div>
      <pre className="ml-term">
        <div className="ml-hold">{draw([...finished(script), { prompt: true, text: '' }])}</div>
        <div className="ml-live">{draw(lines)}</div>
      </pre>
    </div>
  );
}

function Term({ lines }: { lines: Script }) {
  return <TermWindow lines={usePlayer(lines)} script={lines} />;
}

const DROPS: Script = [
  '$ reylo migrate new drop_bio',
  '✗ This migration drops the column users.bio and its data',
  '',
  '  To drop it:',
  '    reylo migrate new drop_bio --drop users.bio',
  '',
  'Nothing was changed.',
  '',
  '$ reylo migrate new about',
  '✓ Wrote reylo/migrations/20261104150000_about.sql',
  '  ! users.bio → about is written as a rename, keeping its rows',
  "    If they're different columns, delete the file, then:",
  '      reylo migrate new about --drop users.bio',
];

const PUSH: Script = [
  '$ reylo db push',
  'Pushing reylo/schema.sql to postgres://localhost:5432/app',
  '  + alter table users add column nick text;',
  '✓ This database matches reylo/schema.sql',
  '! Not in a migration yet: reylo migrate new <name> writes it',
];

const DRY_RUN: Script = [
  '$ reylo migrate deploy --dry-run',
  '✓ This database is as its migrations left it',
  '✓ The migrations match reylo/schema.sql',
  '✓ 1 pending, each tried on a shadow database:',
  '    · 20261104140347_add_bio.sql',
];

const HAND_CHANGE: Script = [
  '$ reylo migrate deploy',
  '✗ This database was changed outside its migrations',
  '',
  '  + column users.hotfix bigint',
  '',
  '  If a migration makes this change, record it here, running nothing:',
  '    reylo migrate history push',
  '  If none does, write one and run it in development, then record it here:',
  '    reylo migrate new <name>',
  '    reylo migrate dev',
  '    reylo migrate history push',
  '',
  'Nothing was changed.',
];

const MIGRATE: Script = [
  '#edit',
  '$ reylo migrate new add_bio',
  '✓ Wrote reylo/migrations/20261104140347_add_bio.sql',
  '',
  '$ reylo migrate dev',
  'Applying to postgres://localhost:5432/app',
  '  ✓ 20261104140347_add_bio.sql  applied · 4 ms',
  '✓ Wrote reylo/reylo.lock',
];

const kw = (s: string) => <span className="ml-kw">{s}</span>;
const ty = (s: string) => <span className="ml-ty">{s}</span>;

function FileIcon() {
  return (
    <svg viewBox="0 0 16 16">
      <path d="M4.5 1.75h4.75l3.25 3.25v8.5a.75.75 0 0 1-.75.75h-7.25a.75.75 0 0 1-.75-.75v-11a.75.75 0 0 1 .75-.75z M9.25 1.75v3.25h3.25" />
    </svg>
  );
}

/** schema.sql gains a column, and the terminal under it writes and runs its migration. */
function SchemaToMigration() {
  const [edited, setEdited] = useState(true);
  const lines = usePlayer(MIGRATE, setEdited);
  return (
    <div className="ml-pair">
      <div className="ml-win ml-editor" aria-hidden="true">
        <div className="ml-bar">
          <FileIcon />
          reylo/schema.sql
        </div>
        <pre className="ml-code">
          <span className="ml-l" data-g=" ">
            {kw('create table')} users (
          </span>
          <span className="ml-l" data-g=" ">
            {'  id '}
            {ty('integer')} {kw('primary key')},
          </span>
          <span className={edited ? 'ml-l ml-del' : 'ml-l'} data-g={edited ? '-' : ' '}>
            {'  email '}
            {ty('text')} {kw('not null')}
          </span>
          <span className={edited ? 'ml-l ml-add' : 'ml-l ml-pending'} data-g="+">
            {'  email '}
            {ty('text')} {kw('not null')},
          </span>
          <span className={edited ? 'ml-l ml-add' : 'ml-l ml-pending'} data-g="+">
            {'  bio '}
            {ty('text')}
          </span>
          <span className="ml-l" data-g=" ">
            );
          </span>
        </pre>
      </div>
      <TermWindow lines={lines} script={MIGRATE} className="ml-under" />
    </div>
  );
}

/** What schema.sql can declare, one kind at a time, from the docs' examples. */
const KINDS: { name: string; sql: ReactNode }[] = [
  {
    name: 'Enums and domains',
    sql: (
      <>
        {kw('create type')} mood {kw('as enum')} ({ty("'sad'")}, {ty("'ok'")}, {ty("'happy'")});{'\n'}
        {kw('create domain')} email {kw('as')} {ty('text')} {kw('check')} (value {kw('like')} {ty("'%@%'")});
      </>
    ),
  },
  {
    name: 'Exclusion constraints',
    sql: (
      <>
        {kw('create table')} bookings ({'\n'}
        {'  '}room {ty('integer')} {kw('not null')},{'\n'}
        {'  '}during {ty('tstzrange')} {kw('not null')},{'\n'}
        {'  '}
        {kw('exclude using')} gist (room {kw('with')} =, during {kw('with')} &&){'\n'}
        );
      </>
    ),
  },
  {
    name: 'Partial indexes',
    sql: (
      <>
        {kw('create index')} users_unverified {kw('on')} users (created_at){'\n'}
        {'  '}
        {kw('where')} verified_at {kw('is null')};
      </>
    ),
  },
  {
    name: 'Triggers',
    sql: (
      <>
        {kw('create function')} touch() {kw('returns trigger language')} plpgsql {kw('as')} $${'\n'}
        {kw('begin')} new.updated_at := now(); {kw('return')} new; {kw('end')} $$;{'\n'}
        {'\n'}
        {kw('create trigger')} users_touch {kw('before update on')} users{'\n'}
        {'  '}
        {kw('for each row execute function')} touch();
      </>
    ),
  },
  {
    name: 'Row level security',
    sql: (
      <>
        {kw('alter table')} documents {kw('enable row level security')};{'\n'}
        {kw('create policy')} documents_own {kw('on')} documents{'\n'}
        {'  '}
        {kw('using')} (owner_id = current_setting({ty("'app.user_id'")})::{ty('integer')});
      </>
    ),
  },
];

/** The kinds as tabs over an editor, one after another every few seconds until the reader picks one. */
function SchemaKinds() {
  const [at, setAt] = useState(0);
  const [picked, setPicked] = useState(false);
  useEffect(() => {
    if (picked || stillPlease()) return;
    const timer = setInterval(() => setAt((i) => (i + 1) % KINDS.length), 3500);
    return () => clearInterval(timer);
  }, [picked]);
  return (
    <div className="ml-win ml-kinds">
      <div className="ml-bar">
        <FileIcon />
        reylo/schema.sql
      </div>
      <div className="ml-tabs" role="tablist">
        {KINDS.map((k, i) => (
          <button
            key={k.name}
            type="button"
            role="tab"
            aria-selected={i === at}
            className={i === at ? 'on' : undefined}
            onClick={() => {
              setAt(i);
              setPicked(true);
            }}
          >
            {k.name}
          </button>
        ))}
      </div>
      <pre className="ml-sql">{KINDS[at].sql}</pre>
    </div>
  );
}
