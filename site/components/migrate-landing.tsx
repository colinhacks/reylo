import { useEffect, useState, type ReactNode } from 'react';
import { DATABASES, HOSTS, type Logo } from './host-logos';
import { VARIADIC_SQUARE, VARIADIC_SQUARES } from './variadic-mark';
import { wordmark } from './wordmark';
import { githubUrl, migrateDocsUrl } from '@/lib/layout.shared';

/**
 * Reylo Migrate's landing page, at /migrate (app/routes/migrate.tsx): a hero split in half, the
 * words on the left and the first edit playing on the right, then one row per feature, its words on one side and a terminal or an editor playing it
 * on the other, the sides alternating. Every line a terminal prints is the docs' own
 * (content/docs/migrate.mdx), and the docs are a page of their own, at /migrate/docs. The players
 * run on timers from the moment the page loads; nothing here reads the scroll position.
 */
export function MigrateLanding({ stars }: { stars?: number }) {
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
          <a href={githubUrl} className="ml-gh">
            <GitHubIcon />
            <span className="ml-gh-label">GitHub</span>
            {stars ? <span className="ml-stars">{starCount(stars)}</span> : null}
          </a>
        </div>
      </nav>

      <header className="ml-hero">
        <div className="ml-hero-words">
          <h1 className="ml-h1">
            <span>SQL-first</span> <em>schema and migrations.</em>
          </h1>
          <p className="ml-lede">Model your schema in <code>schema.sql</code>. Generate migrations, or write your own. Works on Postgres, MySQL, SQLite and SQL Server.</p>
          <Install />
        </div>
        <div className="ml-hero-visual">
          <SchemaToMigration />
        </div>
      </header>

      <Row n={1} title="The most advanced cross-dialect diff engine" visual={<DialectMigrations />}>
        <p>Edit <code>schema.sql</code>, then generate a migration with one command. Editable as needed.</p>
      </Row>

      <Row n={2} title="Use the full power of your database" visual={<SchemaKinds />} flip>
        <p>No lossy or limited DSLs. Declare views, functions, triggers, policies, checks, computed columns, and more.</p>
      </Row>

      <Row n={3} title="Explicit disambiguation" visual={<Term lines={RENAMES} />}>
        <p>When schema changes are ambiguous, Reylo asks you for clarification. Use <code>--drop</code> or <code>--rename</code> to disambiguate.</p>
      </Row>

      <Row n={4} title="Fast iteration loops" visual={<Term lines={PUSH} />} flip>
        <p>Push <code>schema.sql</code> to your development database on every save. Then generate a migration once you're done iterating on schema.</p>
      </Row>

      <Row n={5} title="Schemaless mode" visual={<Schemaless />}>
        <p>For a DDL-first experience, just delete <code>schema.sql</code>. You (or your agents) can write raw DDL migrations.</p>
      </Row>

      <Row n={6} title="Introspect anytime" visual={<Term lines={INTROSPECT} sql />} flip>
        <p>Introspect the current database schema anytime in readable SQL.</p>
      </Row>

      <Row n={7} title={<>Linear history via <code>reylo.lock</code></>} visual={<LockFile />}>
        <p>Enforce linear migration history in Git with the <code>reylo.lock</code> lockfile.</p>
      </Row>

      <Row n={8} title="Soundness guaranteed" visual={<Term lines={HAND_CHANGE} />} flip>
        <p>Checksums in the lockfile prevent edited or reordered migrations. Untracked schema changes are detected and flagged.</p>
      </Row>

      <Row n={9} title="Lock-free mode" visual={<Term lines={LOCK_FREE} />}>
        <p>For faster iteration, delete the lockfile. Unapplied migrations are run in lexicographic order. Reylo automatically prevents any ordering-dependent schema drift.</p>
      </Row>

      <Row n={10} title="Works with your database" visual={<Hosts />} flip>
        <p>Works with Postgres, MySQL, SQLite, SQL Server—and all major database hosts and backends.</p>
      </Row>

      <Row n={11} title="Switch easily" visual={<Term lines={INIT} />}>
        <p>One command introspects your current schema, generates <code>schema.sql</code>, and creates your initial <code>migrations/init.sql</code>.</p>
      </Row>

      <section className="ml-end">
        <h2>Ship faster with Reylo</h2>
        <Install centered />
      </section>

      <footer className="ml-foot">
        <a className="ml-variadic" href="https://variadic.sh">
          <VariadicMark />A Variadic project
        </a>
        <div className="ml-foot-links">
          <a href={migrateDocsUrl}>Docs</a>
          <a href={`${migrateDocsUrl}#cli-reylo-migrate`}>CLI</a>
          <a href="/">Reylo</a>
          <a href={githubUrl}>GitHub</a>
          <a href="https://www.npmjs.com/package/reylo">npm</a>
        </div>
        <span className="ml-license">Released under the MIT License.</span>
      </footer>
    </div>
  );
}

/** Variadic's mark: its squares, in their own colours. */
function VariadicMark() {
  return (
    <svg viewBox="90 90 332 332" aria-hidden="true">
      {VARIADIC_SQUARES.map((q) => (
        <rect key={`${q.x},${q.y}`} x={q.x} y={q.y} width={VARIADIC_SQUARE.size} height={VARIADIC_SQUARE.size} rx={VARIADIC_SQUARE.rx} fill={q.fill} />
      ))}
    </svg>
  );
}

function Mark({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 400 235.47" fill="currentColor" aria-hidden="true">
      <path d={wordmark.mark} />
    </svg>
  );
}

/** A star count as GitHub writes it: 1.6k from a thousand on. */
function starCount(n: number): string {
  return n < 1000 ? String(n) : `${(n / 1000).toFixed(1).replace(/\.0$/, '')}k`;
}

function GitHubIcon() {
  return (
    <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z" />
    </svg>
  );
}

/** A feature: its number, in the accent, its name and a sentence or two, beside what it does. `flip` puts the words on the right. */
function Row({ n, title, visual, flip, children }: { n: number; title: ReactNode; visual: ReactNode; flip?: boolean; children: ReactNode }) {
  return (
    <section className={flip ? 'ml-row ml-flip' : 'ml-row'}>
      <div className="ml-row-text">
        <span className="ml-num">{String(n).padStart(2, '0')}</span>
        <h2>{title}</h2>
        {children}
      </div>
      <div className="ml-row-visual">{visual}</div>
    </section>
  );
}

const INSTALLS = { brew: 'brew install reylosql/tap/reylo', npm: 'npm install reylo', curl: 'curl -fsSL https://reylo.dev/install.sh | sh' } as const;

/** The install command, by package manager: the managers as tabs over its upper left, a copy button at its end, and the docs button, filled with the yellow. `centered` is the closing call to action's, which centres the two side by side. */
function Install({ centered }: { centered?: boolean }) {
  const [kind, setKind] = useState<keyof typeof INSTALLS>('brew');
  const [copied, setCopied] = useState(false);
  const command = INSTALLS[kind];
  return (
    <div className={centered ? 'ml-install ml-centered' : 'ml-install'}>
      <div className="ml-big">
        <div className="ml-box">
          <div className="ml-seg" role="tablist">
            {(Object.keys(INSTALLS) as (keyof typeof INSTALLS)[]).map((k) => (
              <button key={k} type="button" role="tab" aria-selected={k === kind} className={k === kind ? 'on' : undefined} onClick={() => setKind(k)}>
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
        <a className="ml-docs ml-docs-primary" href={migrateDocsUrl}>
          Read the docs
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M2.5 8h11M9 3.5 13.5 8 9 12.5" />
          </svg>
        </a>
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

/** A script's directive: `#edit` changes the editor beside the terminal, `#spin <text>` shows a spinner over the output that follows it. */
const directive = (l: string) => l === '#edit' || l.startsWith('#spin ');

/** Every line of a session at once: what the page renders first, and what a reader who asked for less motion keeps. */
const finished = (script: Script): Line[] => script.filter((l) => !directive(l)).map((l) => (l.startsWith('$ ') ? { prompt: true, text: l.slice(2) } : { prompt: false, text: l }));

const SPINNER = '⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏';

/**
 * Plays a script on timers, as a terminal shows it: the prompt and its cursor waiting, a command typed
 * at it, a moment, the command's whole output at once (after a spinner where the script asks for one),
 * and the next prompt with it; a pause, and again.
 */
function usePlayer(script: Script, onEdit?: (edited: boolean) => void): Line[] {
  const [lines, setLines] = useState<Line[]>(() => finished(script));
  useEffect(() => {
    if (stillPlease()) return;
    let alive = true;
    const sleep = (ms: number) => new Promise<void>((resolve, reject) => setTimeout(() => (alive ? resolve() : reject(new Error('stopped'))), ms));
    const prompt: Line = { prompt: true, text: '', cursor: true };
    // The output after command line k, up to the next command, edit or the end, printed in one go
    // with the next prompt. Returns the last line used.
    const output = async (k: number): Promise<number> => {
      let j = k + 1;
      const spin = script[j]?.startsWith('#spin ') ? script[j++].slice(6) : null;
      const start = j;
      while (j < script.length && !script[j].startsWith('$ ') && !directive(script[j])) j++;
      if (spin) {
        for (let f = 0; f < 18; f++) {
          const frame = `${SPINNER[f % SPINNER.length]} ${spin}`;
          setLines((ls) => [...(f === 0 ? ls : ls.slice(0, -1)), { prompt: false, text: frame }]);
          await sleep(80);
        }
        setLines((ls) => ls.slice(0, -1));
      } else {
        await sleep(start < j ? 260 : 120);
      }
      const printed = script.slice(start, j).map((text): Line => ({ prompt: false, text }));
      setLines((ls) => [...ls, ...printed, prompt]);
      return j - 1;
    };
    (async () => {
      for (;;) {
        onEdit?.(false);
        setLines([prompt]);
        await sleep(700);
        for (let k = 0; k < script.length; k++) {
          const l = script[k];
          if (l === '#edit') {
            await sleep(500);
            onEdit?.(true);
          } else if (l.startsWith('$ ')) {
            const text = l.slice(2);
            await sleep(450);
            for (let i = 1; i <= text.length; i++) {
              await sleep(24 + Math.random() * 40);
              const typed = text.slice(0, i);
              setLines((ls) => [...ls.slice(0, -1), { prompt: true, text: typed, cursor: true }]);
            }
            await sleep(350);
            setLines((ls) => [...ls.slice(0, -1), { prompt: true, text }]);
            k = await output(k);
          }
        }
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

/** A printed line in the CLI's colours: ✓ and + green, ✗ red, ! and ↺ yellow; a » note and the lead over a command (an indented line from a capital to a colon) dim whole; the command on its own line cyan. */
function paint(line: string): ReactNode {
  if (/^\s*» /.test(line) || /^\s+[A-Z].*:$/.test(line)) return <span className="ml-mu">{line}</span>;
  const m = line.match(/^(\s*)(✓|✗|!|\+|↺)(\s.*)?$/);
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
  if (/^\s+reylo /.test(line)) return <span className="ml-cy">{line}</span>;
  if (line === 'Nothing was changed.') return <span className="ml-mu">{line}</span>;
  return line;
}

/** A terminal: the session so far, over the whole session drawn invisibly, which holds the window at its final height as it plays. */
/** The SQL words `reylo db introspect` colours in a terminal (reylosql/reylo#309): keywords cyan, types yellow, names plain. */
const SQL_KEYWORDS = new Set(['create', 'table', 'not', 'null', 'constraint', 'primary', 'key', 'alter', 'add', 'column', 'unique', 'references']);
const SQL_TYPES = new Set(['integer', 'bigint', 'text', 'boolean']);

/** A line of SQL a command printed, coloured as the CLI colours it. */
function paintSql(line: string): ReactNode {
  return line.split(/(\b[a-z_]+\b)/).map((t, i) => (SQL_KEYWORDS.has(t) ? <span key={i} className="ml-cy">{t}</span> : SQL_TYPES.has(t) ? <span key={i} className="ml-wn">{t}</span> : t));
}

/** A spinner's frame: the braille in the CLI's cyan, before what it says. */
const paintSpin = (line: string): ReactNode => (
  <>
    <span className="ml-cy">{line[0]}</span>
    {line.slice(1)}
  </>
);

function TermWindow({ lines, script, className, sql }: { lines: Line[]; script: Script; className?: string; sql?: boolean }) {
  const out = (text: string) => (SPINNER.includes(text[0] ?? ' ') ? paintSpin(text) : sql ? paintSql(text) : paint(text));
  const draw = (ls: Line[]) =>
    ls.map((l, i) => (
      <div key={i} className="ml-ln">
        {l.prompt ? <span className="ml-ps">$ </span> : null}
        {l.prompt ? <span className="ml-c">{l.text}</span> : out(l.text)}
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

/** A terminal playing a script; `sql` colours what it prints as SQL. */
function Term({ lines, sql }: { lines: Script; sql?: boolean }) {
  return <TermWindow lines={usePlayer(lines)} script={lines} sql={sql} />;
}

/** As reylo 0.8.0 printed it: the migration asked for with no flags, then again with the renames named. */
const RENAMES: Script = [
  '$ reylo migrate new tidy_columns',
  '✗ These 2 look like renames, but each changes more than its name',
  '',
  '    posts.title → heading  text → varchar(100)',
  '    users.bio   → about    text → varchar(200)',
  '',
  '  If they are renames, keeping their rows:',
  '    reylo migrate new tidy_columns \\',
  '      --rename posts.title=heading \\',
  '      --rename users.bio=about',
  '  If they are new, dropping the old ones and their data:',
  '    reylo migrate new tidy_columns --drop posts.title,users.bio',
  '',
  'Nothing was changed.',
  '$ reylo migrate new tidy_columns --rename posts.title=heading --rename users.bio=about',
  '✓ Wrote reylo/migrations/20261104150000_tidy_columns.sql',
];

/** With no schema.sql: DDL run on the database, the migration written from what it changed (`#edit` fills the editor above), and the migration recorded as run. */
const SCHEMALESS: Script = [
  "$ reylo run 'alter table users add column bio text'",
  '✓ alter table users add column bio text',
  '$ reylo migrate new add_bio',
  '✓ Wrote reylo/migrations/20261104140347_add_bio.sql',
  '#edit',
  '$ reylo migrate dev',
  'Applying to postgres://localhost:5432/app',
  '  + 20261104140347_add_bio.sql  recorded: already in place',
  '✓ Wrote reylo/reylo.lock',
];

/** What reylo printed for one Postgres table, after the spinner an interactive terminal shows while it reads (reylosql/reylo#309). */
const INTROSPECT: Script = [
  '$ reylo db introspect',
  '#spin Introspecting postgres://localhost:5432/app',
  'create table users (',
  '  id integer not null,',
  '  email text not null,',
  '  bio text,',
  '  constraint users_pkey primary key (id)',
  ');',
];

/** Lock-free: a migration that sorts before one the database ran is refused, then runs once it's given a later name. */
const LOCK_FREE: Script = [
  '$ reylo migrate deploy',
  '✗ Migration 20261105080000_no_nick.sql sorts before 20261105090000_nick.sql, which this database already ran, and the two orders end differently',
  '  + a fresh database: column users.nick text',
  '  Give 20261105080000_no_nick.sql a later name, so that every database runs it after 20261105090000_nick.sql',
  '',
  'Nothing was changed.',
  '$ mv reylo/migrations/20261105080000_no_nick.sql reylo/migrations/20261105100000_no_nick.sql',
  '$ reylo migrate deploy',
  'Applying to postgres://db.example.com/app',
  '  ✓ 20261105100000_no_nick.sql  applied · 9 ms',
  '✓ Applied 1 migration',
];

const PUSH: Script = [
  '$ reylo db push',
  'Pushing reylo/schema.sql to postgres://localhost:5432/app',
  '  + alter table users add column nick text;',
  '✓ This database matches reylo/schema.sql',
  '$ reylo migrate new add_nick',
  '✓ Wrote reylo/migrations/20261104150000_add_nick.sql',
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

const INIT: Script = [
  '$ reylo migrate init',
  '✓ Started reylo/ from postgres://localhost:5432/app',
  '  reylo/',
  "  ├── schema.sql          this database's schema",
  '  ├── migrations/',
  '  │   └── init.sql        the same, recorded as run here',
  '  └── reylo.lock          the order the migrations run in',
];

const MIGRATE: Script = [
  '#edit',
  '$ reylo migrate new add_bio',
  '✓ Wrote reylo/migrations/20261104140347_add_bio.sql',
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

/**
 * One edit to schema.sql, and the migration `reylo migrate new email_required` writes for it on each
 * database, as reylo 0.8.0 wrote them, and SQL Server's as the release that adds it writes it: Postgres,
 * MySQL and SQL Server alter the column, SQLite rebuilds the table.
 */
const DIALECTS: { name: string; sql: ReactNode }[] = [
  {
    name: 'Postgres',
    sql: (
      <>
        {kw('alter table')} users {kw('alter column')} email {kw('set not null')};
      </>
    ),
  },
  {
    name: 'MySQL',
    sql: (
      <>
        {kw('alter table')} users {kw('modify column')} email {ty('text')} {kw('not null')};
      </>
    ),
  },
  {
    name: 'SQLite',
    sql: (
      <>
        {kw('create table')} new_users (id {ty('integer')} {kw('primary key')}, email {ty('text')} {kw('not null')}, name {ty('text')} {kw('not null')}) {kw('strict')};{'\n'}
        {kw('insert into')} new_users (id, email, name) {kw('select')} id, email, name {kw('from')} users;{'\n'}
        {kw('drop table')} users;{'\n'}
        {kw('alter table')} new_users {kw('rename to')} users;
      </>
    ),
  },
  {
    name: 'SQL Server',
    sql: (
      <>
        {kw('alter table')} users {kw('alter column')} email {ty('text')} {kw('not null')};
      </>
    ),
  },
];

/** schema.sql with `email` made required, and under it the migration for each database, as tabs that turn by themselves until one is picked. */
function DialectMigrations() {
  const [at, setAt] = useState(0);
  const [picked, setPicked] = useState(false);
  useEffect(() => {
    if (picked || stillPlease()) return;
    const timer = setInterval(() => setAt((i) => (i + 1) % DIALECTS.length), 3500);
    return () => clearInterval(timer);
  }, [picked]);
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
          <span className="ml-l ml-del" data-g="-">
            {'  email '}
            {ty('text')},
          </span>
          <span className="ml-l ml-add" data-g="+">
            {'  email '}
            {ty('text')} {kw('not null')},
          </span>
          <span className="ml-l" data-g=" ">
            {'  name '}
            {ty('text')} {kw('not null')}
          </span>
          <span className="ml-l" data-g=" ">
            );
          </span>
        </pre>
      </div>
      <div className="ml-win ml-under ml-dialects">
        <div className="ml-bar">
          <FileIcon />
          reylo/migrations/20261104140347_email_required.sql
        </div>
        <div className="ml-tabs" role="tablist">
          {DIALECTS.map((d, i) => (
            <button
              key={d.name}
              type="button"
              role="tab"
              aria-selected={i === at}
              className={i === at ? 'on' : undefined}
              onClick={() => {
                setAt(i);
                setPicked(true);
              }}
            >
              {d.name}
            </button>
          ))}
        </div>
        <div className="ml-kinds-body">
          {DIALECTS.map((d, i) => (
            <pre key={d.name} className="ml-sql" data-on={i === at || undefined}>
              {d.sql}
            </pre>
          ))}
        </div>
      </div>
    </div>
  );
}

/** Schemaless: the migration file, empty until `reylo migrate new` writes it, over the terminal that changes the database and makes it. */
function Schemaless() {
  const [edited, setEdited] = useState(true);
  const lines = usePlayer(SCHEMALESS, setEdited);
  return (
    <div className="ml-pair">
      <div className="ml-win ml-editor" aria-hidden="true">
        <div className="ml-bar">
          <FileIcon />
          reylo/migrations/20261104140347_add_bio.sql
        </div>
        <pre className="ml-code">
          <span className={edited ? 'ml-l ml-add' : 'ml-l ml-pending'} data-g="+">
            {kw('alter table')} users {kw('add column')} bio {ty('text')};
          </span>
          {/* Room under the line, so the terminal's corner doesn't cover it. */}
          <span className="ml-l" data-g=" ">
            {' '}
          </span>
          <span className="ml-l" data-g=" ">
            {' '}
          </span>
        </pre>
      </div>
      <TermWindow lines={lines} script={SCHEMALESS} className="ml-under" />
    </div>
  );
}

/** Where Reylo Migrate runs, as each host's logo over its name. */
function Hosts() {
  // A tile says "Tested" only once every tile can: a grid where some do reads as if the rest failed.
  const chips = [...DATABASES, ...HOSTS].every((t) => t.tested);
  const tile = (t: Logo) => (
    <span key={t.name}>
      <svg viewBox={t.viewBox} fill="currentColor" aria-hidden="true">
        {t.paths.map((d) => (
          <path key={d} d={d} />
        ))}
      </svg>
      {t.name}
      {chips ? <em className="ml-tested">Tested</em> : null}
    </span>
  );
  return (
    <div className="ml-hosts">{[...DATABASES, ...HOSTS].map(tile)}</div>
  );
}

/** reylo/reylo.lock after two branches each added a migration: git stops the merge on the lock, before anything deploys. */
function LockFile() {
  const line = (name: string, sum: string) => (
    <>
      {'  - '}
      {name} {ty(sum)}
      {'\n'}
    </>
  );
  return (
    <div className="ml-win" aria-hidden="true">
      <div className="ml-bar">
        <FileIcon />
        reylo/reylo.lock
      </div>
      <pre className="ml-sql ml-lock">
        {kw('schema:')} {ty('c7bf681b')}
        {'\n'}
        {kw('migrations:')}
        {'\n'}
        {line('init.sql', '1d42e07a')}
        {line('20261104120000_users.sql', 'c86e9de4')}
        {line('20261104140347_add_bio.sql', '78ad08c0')}
        {line('20261105090000_tags.sql', '3f1a9b27')}
        <span className="ml-er">{'<<<<<<< main'}</span>
        {'\n'}
        {line('20261106100000_orders.sql', '9c04e1d2')}
        <span className="ml-mu">{'======='}</span>
        {'\n'}
        {line('20261106093000_nick.sql', 'e51b7a38')}
        <span className="ml-er">{'>>>>>>> add-nick'}</span>
      </pre>
    </div>
  );
}

/** What schema.sql can declare, one kind at a time, from the docs' examples. */
const KINDS: { name: string; sql: ReactNode }[] = [
  {
    name: 'Views',
    sql: (
      <>
        {kw('create view')} active_users {kw('as')}{'\n'}
        {'  '}
        {kw('select')} id, email {kw('from')} users {kw('where')} deleted_at {kw('is null')};{'\n'}
        {'\n'}
        {kw('create materialized view')} daily_signups {kw('as')}{'\n'}
        {'  '}
        {kw('select')} date_trunc({ty("'day'")}, created_at) {kw('as')} day, count(*){'\n'}
        {'  '}
        {kw('from')} users {kw('group by')} 1;
      </>
    ),
  },
  {
    name: 'Functions',
    sql: (
      <>
        {kw('create function')} slugify(t {ty('text')}) {kw('returns')} {ty('text')}{'\n'}
        {'  '}
        {kw('language')} sql {kw('immutable')} {kw('as')} {'$$'}{'\n'}
        {'    '}
        {kw('select')} lower(regexp_replace(t, {ty("'[^a-z0-9]+'")}, {ty("'-'")}, {ty("'gi'")})){'\n'}
        {'  '}
        {'$$'};
      </>
    ),
  },
  {
    name: 'Triggers',
    sql: (
      <>
        {kw('create function')} touch() {kw('returns trigger language')} plpgsql {kw('as')} {'$$'}{'\n'}
        {kw('begin')} new.updated_at := now(); {kw('return')} new; {kw('end')} {'$$'};{'\n'}
        {'\n'}
        {kw('create trigger')} users_touch {kw('before update on')} users{'\n'}
        {'  '}
        {kw('for each row execute function')} touch();
      </>
    ),
  },
  {
    name: 'Indexes',
    sql: (
      <>
        {kw('create index')} users_unverified {kw('on')} users (created_at){'\n'}
        {'  '}
        {kw('where')} verified_at {kw('is null')};{'\n'}
        {'\n'}
        {kw('create unique index')} users_email {kw('on')} users (lower(email));
      </>
    ),
  },
  {
    name: 'Policies',
    sql: (
      <>
        {kw('alter table')} documents {kw('enable row level security')};{'\n'}
        {kw('create policy')} documents_own {kw('on')} documents{'\n'}
        {'  '}
        {kw('using')} (owner_id = current_setting({ty("'app.user_id'")})::{ty('integer')});
      </>
    ),
  },
  {
    name: 'Checks',
    sql: (
      <>
        {kw('create table')} bookings ({'\n'}
        {'  '}seats {ty('integer')} {kw('not null check')} (seats {'>'} 0),{'\n'}
        {'  '}starts_at {ty('timestamptz')} {kw('not null')},{'\n'}
        {'  '}ends_at {ty('timestamptz')} {kw('not null')},{'\n'}
        {'  '}
        {kw('check')} (ends_at {'>'} starts_at){'\n'}
        );
      </>
    ),
  },
  {
    name: 'Computed',
    sql: (
      <>
        {kw('create table')} line_items ({'\n'}
        {'  '}price {ty('numeric')} {kw('not null')},{'\n'}
        {'  '}qty {ty('integer')} {kw('not null')},{'\n'}
        {'  '}total {ty('numeric')} {kw('generated always as')} (price * qty) {kw('stored')}{'\n'}
        );
      </>
    ),
  },
  {
    name: 'JSON Schema',
    sql: (
      <>
        {kw('create table')} events ({'\n'}
        {'  '}payload {ty('jsonb')} {kw('not null check')} (jsonb_matches_schema({ty("'{")}{'\n'}
        {'    '}
        {ty('"type": "object",')}
        {'\n'}
        {'    '}
        {ty('"properties": { "x": { "type": "integer" } },')}
        {'\n'}
        {'    '}
        {ty('"required": ["x"]')}
        {'\n'}
        {'  '}
        {ty("}'")}, payload)){'\n'}
        );
      </>
    ),
  },
  {
    name: 'And more',
    sql: (
      <>
        {kw('create extension if not exists')} btree_gist;{'\n'}
        {kw('create type')} mood {kw('as enum')} ({ty("'sad'")}, {ty("'ok'")}, {ty("'happy'")});{'\n'}
        {kw('create domain')} email {kw('as')} {ty('text')} {kw('check')} (value {kw('like')} {ty("'%@%'")});{'\n'}
        {kw('create type')} floatrange {kw('as range')} (subtype = {ty('float8')});{'\n'}
        {kw('create table')} rooms (room {ty('int')}, during {ty('tstzrange')},{'\n'}
        {'  '}
        {kw('exclude using')} gist (room {kw('with')} =, during {kw('with')} &&));{'\n'}
        {kw('grant select on')} active_users {kw('to')} reporting;{'\n'}
        {kw('comment on table')} users {kw('is')} {ty("'The people who sign in'")};
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
      <div className="ml-kinds-body">
        {KINDS.map((k, i) => (
          <pre key={k.name} className="ml-sql" data-on={i === at || undefined}>
            {k.sql}
          </pre>
        ))}
      </div>
    </div>
  );
}
