import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Choice, Feature, Features, Way } from './feature-cards';
import { wordmark } from './wordmark';
import { githubUrl, migrateDocsUrl } from '@/lib/layout.shared';

/**
 * Reylo Migrate's landing page, at /migrate (app/routes/migrate.tsx): a split hero, its headline and
 * install command on the left and on the right a change to schema.sql that a terminal writes into a
 * migration and runs, then the feature cards, then a foot. The docs are a page of their own, at
 * /migrate/docs. Every line the terminal prints is the docs' own (content/docs/migrate.mdx).
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
        <div className="ml-copy">
          <h1 className="ml-h1">SQL-first database migrations for busy teams</h1>
          <p className="ml-lede">Keep your schema in one SQL file. Reylo Migrate writes each change as a migration of plain SQL, on Postgres, MySQL and SQLite.</p>
          <div className="ml-actions">
            <Install />
            <a className="ml-docs" href={migrateDocsUrl}>
              Read the docs
              <svg viewBox="0 0 16 16" aria-hidden="true">
                <path d="M2.5 8h11M9 3.5 13.5 8 9 12.5" />
              </svg>
            </a>
          </div>
        </div>
        <Stage />
      </header>

      <section className="ml-features">
        <Features>
          <Choice icon="file" title="SQL-first data modeling">
            <Way title="via `schema.sql`">
              <p>Keep the schema you want in one file, as plain SQL, and Reylo Migrate writes each migration from your changes to it.</p>
            </Way>
            <Way title="schemaless mode" or>
              <p>
                You or your agents write each migration by hand, and Reylo Migrate checks and runs them all the same. There's no <code>schema.sql</code> to keep.
              </p>
            </Way>
          </Choice>
          <Choice icon="branch" title="Linear or loose history">
            <Way title="linear, via `reylo.lock`">
              <p>A lockfile keeps one order, so two branches that each add a migration conflict in git, before anything deploys.</p>
            </Way>
            <Way title="loose, in name order" or>
              <p>
                Delete the lockfile, and Reylo Migrate runs migrations in lexicographic order, while refusing any changes that cause path-dependent drift. Nothing writes the lock again until <code>reylo migrate lock</code>.
              </p>
            </Way>
          </Choice>
          <Feature icon="zap" title="Fast iteration">
            <p>
              Try a change before you write its migration: <code>reylo db push --watch</code> takes your development database to <code>schema.sql</code> on every save.
            </p>
          </Feature>
          <Feature icon="split" title="Explicit drops">
            <p>
              Every destructive change needs <code>--drop</code>, in a migration and in a push. Where Reylo Migrate can't tell a rename from a drop, <code>--rename</code> settles it.
            </p>
          </Feature>
          <Feature icon="layers" title="Dry runs in shadow databases">
            <p>
              Reylo Migrate replays your migrations on a throwaway database, so <code>--dry-run</code> shows what a deploy would do, running nothing on yours.
            </p>
          </Feature>
          <Feature icon="hash" title="Integrity checks">
            <p>Chained hashes in the lock catch a migration edited, deleted or reordered, before anything runs. A fingerprint on each database catches a change made by hand, and shows it.</p>
          </Feature>
          <Feature icon="database" title="Dialect-agnostic">
            <p>Supports Postgres, MySQL and SQLite, with one set of commands.</p>
          </Feature>
        </Features>
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

function Mark() {
  return (
    <svg viewBox="0 0 400 235.47" fill="currentColor" aria-hidden="true">
      <path d={wordmark.mark} />
    </svg>
  );
}

const INSTALLS = { npm: 'npm install reylo', Homebrew: 'brew install reylosql/tap/reylo', curl: 'curl -fsSL https://reylo.dev/install.sh | sh' } as const;

/** The install command, by package manager, with a copy button. */
function Install() {
  const [kind, setKind] = useState<keyof typeof INSTALLS>('npm');
  const [copied, setCopied] = useState(false);
  const command = INSTALLS[kind];
  return (
    <div className="ml-install">
      <div className="ml-seg">
        {(Object.keys(INSTALLS) as (keyof typeof INSTALLS)[]).map((k) => (
          <button key={k} type="button" className={k === kind ? 'on' : undefined} onClick={() => setKind(k)}>
            {k}
          </button>
        ))}
      </div>
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
    </div>
  );
}

type Step = { kind: 'type'; text: string } | { kind: 'out'; line: ReactNode; wait: number } | { kind: 'edit' };

const kw = (s: string) => <span className="ml-kw">{s}</span>;
const ty = (s: string) => <span className="ml-ty">{s}</span>;
const ok = <span className="ml-ok">✓</span>;
const mu = (s: string) => <span className="ml-mu">{s}</span>;

const STEPS: Step[] = [
  { kind: 'edit' },
  { kind: 'type', text: 'reylo migrate new add_bio' },
  { kind: 'out', line: <>{ok} Wrote reylo/migrations/20261104140347_add_bio.sql</>, wait: 300 },
  { kind: 'out', line: '', wait: 500 },
  { kind: 'type', text: 'cat reylo/migrations/20261104140347_add_bio.sql' },
  { kind: 'out', line: <>{kw('alter table')} users {kw('add column')} bio {ty('text')};</>, wait: 200 },
  { kind: 'out', line: '', wait: 500 },
  { kind: 'type', text: 'reylo migrate dev' },
  { kind: 'out', line: mu('Applying to postgres://localhost:5432/app'), wait: 350 },
  { kind: 'out', line: <>  {ok} 20261104140347_add_bio.sql  {mu('applied · 4 ms')}</>, wait: 450 },
  { kind: 'out', line: <>{ok} Wrote reylo/reylo.lock</>, wait: 250 },
];

type Line = { prompt: boolean; text: ReactNode; cursor?: boolean };

/** Every line of the run at once, as a reader who asked for less motion sees it, and as the page renders before it plays. */
function finished(): Line[] {
  return STEPS.flatMap((s): Line[] => (s.kind === 'type' ? [{ prompt: true, text: s.text }] : s.kind === 'out' ? [{ prompt: false, text: s.line }] : []));
}

/** schema.sql gains a column, then the terminal types the commands that write and run its migration, and plays again. */
function Stage() {
  const [lines, setLines] = useState<Line[]>(finished);
  const [edited, setEdited] = useState(true);
  const alive = useRef(true);

  useEffect(() => {
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    alive.current = true;
    const sleep = (ms: number) =>
      new Promise<void>((resolve, reject) => setTimeout(() => (alive.current ? resolve() : reject(new Error('stopped'))), ms));
    (async () => {
      for (;;) {
        setLines([]);
        setEdited(false);
        await sleep(600);
        for (const step of STEPS) {
          if (step.kind === 'edit') {
            await sleep(500);
            setEdited(true);
          } else if (step.kind === 'out') {
            await sleep(step.wait);
            setLines((ls) => [...ls, { prompt: false, text: step.line }]);
          } else {
            await sleep(500);
            setLines((ls) => [...ls, { prompt: true, text: '', cursor: true }]);
            for (let i = 1; i <= step.text.length; i++) {
              await sleep(26 + Math.random() * 42);
              const typed = step.text.slice(0, i);
              setLines((ls) => [...ls.slice(0, -1), { prompt: true, text: typed, cursor: true }]);
            }
            await sleep(380);
            setLines((ls) => [...ls.slice(0, -1), { prompt: true, text: step.text }]);
          }
        }
        setLines((ls) => [...ls, { prompt: true, text: '', cursor: true }]);
        await sleep(4500);
      }
    })().catch(() => {});
    return () => {
      alive.current = false;
    };
  }, []);

  return (
    <div className="ml-stage" aria-hidden="true">
      <div className="ml-win ml-editor">
        <div className="ml-bar">
          <svg viewBox="0 0 16 16">
            <path d="M4.5 1.75h4.75l3.25 3.25v8.5a.75.75 0 0 1-.75.75h-7.25a.75.75 0 0 1-.75-.75v-11a.75.75 0 0 1 .75-.75z M9.25 1.75v3.25h3.25" />
          </svg>
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
      <div className="ml-win ml-terminal">
        <div className="ml-bar">
          <span className="ml-dots">
            <i />
            <i />
            <i />
          </span>
          ~/app
        </div>
        <pre className="ml-term">
          {lines.map((l, i) => (
            <div key={i} className="ml-ln">
              {l.prompt ? <span className="ml-ps">$ </span> : null}
              {l.prompt ? <span className="ml-c">{l.text}</span> : l.text}
              {l.cursor ? <span className="ml-cur" /> : null}
            </div>
          ))}
        </pre>
      </div>
    </div>
  );
}
