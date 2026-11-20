import { useState, type CSSProperties, type ReactNode } from 'react';
import data from 'virtual:home-examples';
import { DATABASES, HOSTS } from './host-logos';
import { FileIcon, GitHubIcon, Mark, starCount, VariadicMark } from './migrate-landing';
import type { HomeExample, HomeVariant, Line } from '@/lib/home-examples';
import { githubUrl, introUrl } from '@/lib/layout.shared';
import './home-landing.css';

/*
 * Reylo's homepage, at / (app/routes/home.tsx), in the Migrate landing's language
 * (components/migrate-landing.tsx, the `.ml` styles): a nav whose GitHub pill carries a handwritten
 * "Leave a star!"; a hero split in half, the words on the left and on the right a query and, under
 * it, the main.ts that imports it; the Inference section's examples, each in the docs' inference card
 * (components/inference-card.tsx) with its schema behind "View schema", named in a list in the left
 * gutter; the features; and a closing call to action. Every statement and type is the docs' own,
 * captured (lib/home-examples.ts). Nothing moves by itself, and nothing reads the scroll position.
 */

type Dialect = 'postgres' | 'mysql' | 'sqlite' | 'sqlserver';

/** The examples the homepage shows, by the Inference section's anchors, in its order. */
const SHOWN = ['left-joins', 'computed-columns', 'unique-keys', 'aggregates', 'window-functions', 'unions', 'json', 'json-schema'];
const EXAMPLES = SHOWN.map((id) => data.examples.find((e) => e.id === id)).filter((e): e is HomeExample => !!e);

/** Lines a card holds: its statement, its parameters (`void` for none) and its result. */
const height = (v: HomeVariant) => v.sql.length + (v.params?.length ?? 1) + v.returns.length;
/** The tallest card's, in any database: every statement is padded with blank lines to it, so every card is as tall and nothing under the cards moves as the reader picks another. */
const TALLEST = Math.max(...EXAMPLES.flatMap((e) => Object.values(e.variants).map(height)));
const padded = (v: HomeVariant): Line[] => [...v.sql, ...Array.from({ length: TALLEST - height(v) }, (): Line => [])];

const DIALECT_NAMES: Record<Dialect, string> = { postgres: 'Postgres', mysql: 'MySQL', sqlite: 'SQLite', sqlserver: 'SQL Server' };

export function HomeLanding({ stars }: { stars?: number }) {
  return (
    <div className="ml rh">
      <Nav stars={stars} />
      <Hero />
      <Examples />
      <Features />
      <End />
      <Foot />
    </div>
  );
}

/* ── The nav ─────────────────────────────────────────────────────────────── */

function Nav({ stars }: { stars?: number }) {
  return (
    <nav className="ml-nav rh-nav">
      <a className="ml-brand" href="/">
        <Mark />
        Reylo
      </a>
      <div className="ml-links rh-links">
        <a href={introUrl}>Docs</a>
        <a href="/play">Playground</a>
        <a href="/migrate">Migrate</a>
        <span className="rh-star-wrap">
          <StarPill stars={stars} />
          <StarNote />
        </span>
      </div>
    </nav>
  );
}

/** GitHub's star button, unstarred: the mark, an outline star and the count, one link to the repository. */
function StarPill({ stars }: { stars?: number }) {
  return (
    <a className="rh-pill" href={githubUrl} aria-label={stars ? `Star Reylo on GitHub, ${stars} stars` : 'Star Reylo on GitHub'}>
      <GitHubIcon />
      <svg className="rh-pill-star" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
        <path d="M8 .25a.75.75 0 0 1 .673.418l1.882 3.815 4.21.612a.75.75 0 0 1 .416 1.279l-3.046 2.97.719 4.192a.751.751 0 0 1-1.088.791L8 12.347l-3.766 1.98a.75.75 0 0 1-1.088-.79l.72-4.194L.818 6.374a.75.75 0 0 1 .416-1.28l4.21-.611L7.327.668A.75.75 0 0 1 8 .25Zm0 2.445L6.615 5.5a.75.75 0 0 1-.564.41l-3.097.45 2.24 2.184a.75.75 0 0 1 .216.664l-.528 3.084 2.769-1.456a.75.75 0 0 1 .698 0l2.77 1.456-.53-3.084a.75.75 0 0 1 .216-.664l2.24-2.183-3.096-.45a.75.75 0 0 1-.564-.41L8 2.694Z" />
      </svg>
      <span>{stars ? starCount(stars) : 'Star'}</span>
    </a>
  );
}

/**
 * "Leave a star!" in a hand, under the pill, and an arrow drawn from the end of it, sweeping right and
 * then up into the middle of the pill's underside. Drawn from the pill's own box (`.rh-star-note`), so
 * it lands on the pill at every width; the nav scrolls away with the page, and the note with it.
 */
function StarNote() {
  return (
    <svg className="rh-star-note" viewBox="0 0 170 84" fill="none" aria-hidden="true">
      <text x="62" y="62" textAnchor="end" transform="rotate(-4 62 62)">
        Leave a star!
      </text>
      <path d="M 72 57 C 110 63, 144 36, 150 3" />
      <path d="M 142 15 L 150 3 L 158 14" />
    </svg>
  );
}

/* ── The install ────────────────────────────────────────────────────────── */

const INSTALLS = { npm: 'npm install reylo', pnpm: 'pnpm add reylo', bun: 'bun add reylo', nub: 'nub add reylo' } as const;

function CopyIcon({ done }: { done: boolean }) {
  return done ? (
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <path d="M3 8.5l3.2 3.2L13 4.8" />
    </svg>
  ) : (
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <rect x="5.25" y="5.25" width="8.5" height="8.5" rx="1.75" />
      <path d="M10.75 5.25v-1.5a1.5 1.5 0 0 0-1.5-1.5h-5.5a1.5 1.5 0 0 0-1.5 1.5v5.5a1.5 1.5 0 0 0 1.5 1.5h1.5" />
    </svg>
  );
}

/** The install command by package manager, with "Get started", which opens the docs at their top. `centered` puts the two side by side, centred. */
function Install({ centered }: { centered?: boolean }) {
  const [kind, setKind] = useState<keyof typeof INSTALLS>('npm');
  const [copied, setCopied] = useState(false);
  const command = INSTALLS[kind];
  return (
    <div className={`ml-install rh-install${centered ? ' ml-centered' : ''}`}>
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
              <CopyIcon done={copied} />
            </button>
          </div>
        </div>
        <a className="ml-docs ml-docs-primary" href={introUrl}>
          Get started
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M2.5 8h11M9 3.5 13.5 8 9 12.5" />
          </svg>
        </a>
      </div>
    </div>
  );
}

/* ── Code ───────────────────────────────────────────────────────────────── */

/**
 * Highlighted lines, as lib/home-examples.ts tokenized them, each token in its colour by day and by
 * night (`.rh-tok` picks one; a window, dark on either theme, takes the night's).
 */
function Code({ lines }: { lines: Line[] }) {
  return lines.map((line, i) => (
    <span key={i} className="rh-line">
      {line.map(([text, light, dark], j) => (
        <span key={j} className="rh-tok" style={{ '--l': light, '--d': dark } as CSSProperties}>
          {text}
        </span>
      ))}
      {'\n'}
    </span>
  ));
}

/* ── The inference card ─────────────────────────────────────────────────── */

/**
 * The docs' inference card (components/inference-card.tsx, `.reylo-inference-card` in
 * app/global.css), whose styles are scoped to the docs' page: Query, with the database switch at the
 * right of its label's line where the example has more than one database, then Params and Returns in
 * a band under it.
 */
function Card({ variant, dialects, dialect, onDialect }: { variant: HomeVariant; dialects: Dialect[]; dialect: Dialect; onDialect: (d: Dialect) => void }) {
  return (
    <div className="rh-card">
      <div className="rh-card-row">
        <div className="rh-card-label">Query</div>
        {dialects.length > 1 && (
          <div className="rh-switch" role="tablist" aria-label="Database">
            {dialects.map((d) => (
              <button key={d} type="button" role="tab" aria-selected={d === dialect} data-state={d === dialect ? 'active' : undefined} onClick={() => onDialect(d)}>
                {DIALECT_NAMES[d]}
              </button>
            ))}
          </div>
        )}
        <pre className="rh-code">
          <Code lines={padded(variant)} />
        </pre>
      </div>
      <div className="rh-card-types">
        <div className="rh-card-row">
          <div className="rh-card-label">Params</div>
          <pre className="rh-code">
            <Code lines={variant.params ?? [[['void', '#D73A49', '#F97583']]]} />
          </pre>
        </div>
        <div className="rh-card-row">
          <div className="rh-card-label">Returns</div>
          <pre className="rh-code">
            <Code lines={variant.returns} />
          </pre>
        </div>
      </div>
    </div>
  );
}

/** "View schema" under the card's right corner, the schema in a popover on hover, focus or a click: the docs' (components/schema-query.tsx). */
function ViewSchema({ lines }: { lines: Line[] }) {
  return (
    <div className="schema-query-foot rh-schema-foot">
      <span className="schema-query-link" tabIndex={0}>
        View schema
        <span className="schema-query-pop" role="dialog" aria-label="Schema">
          <span className="schema-query-card not-prose">
            <pre className="rh-code rh-schema">
              <Code lines={lines} />
            </pre>
          </span>
        </span>
      </span>
    </div>
  );
}

/* ── The examples ───────────────────────────────────────────────────────── */

/**
 * The Inference section's examples, one inference card at a time, centred between the names in the
 * left gutter and a right gutter as wide. The reader picks one; none turns by itself.
 */
function Examples() {
  const [at, setAt] = useState(0);
  const [dialect, setDialect] = useState<Dialect>('postgres');
  const shownDialect = (e: HomeExample): Dialect => (e.variants[dialect] ? dialect : (Object.keys(e.variants)[0] as Dialect));
  return (
    <section className="rh-examples">
      <header className="rh-section-head">
        <h2>What Reylo infers</h2>
        <p>Parameters, columns, nullability, cardinality and nested JSON shapes, from your database's own analysis of each query.</p>
      </header>
      <div className="rh-ex">
        <div className="rh-ex-list" role="tablist" aria-label="Examples">
          {EXAMPLES.map((e, i) => (
            <button key={e.id} type="button" role="tab" aria-selected={i === at} className={i === at ? 'on' : undefined} onClick={() => setAt(i)}>
              {e.title}
            </button>
          ))}
        </div>
        <div className="rh-ex-cards">
          {EXAMPLES.map((e, i) => {
            const d = shownDialect(e);
            const variant = e.variants[d]!;
            return (
              <div key={e.id} className="rh-ex-card" data-on={i === at || undefined}>
                <Card variant={variant} dialects={Object.keys(e.variants) as Dialect[]} dialect={d} onDialect={setDialect} />
                <ViewSchema lines={variant.schema ?? data.schemas[d] ?? []} />
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

/* ── The hero ───────────────────────────────────────────────────────────── */

/**
 * The words on the left; on the right the intro's statement, and under it, indented, the main.ts that
 * imports it, with the type of `author` in a hover, as the Migrate landing lays its editor over its
 * terminal.
 */
function Hero() {
  return (
    <header className="ml-hero rh-hero-split">
      <div className="ml-hero-words">
        <h1 className="ml-h1 rh-h1">
          <span>Import SQL</span> <span>into TypeScript,</span> <em>fully typed</em>
        </h1>
        <p className="ml-lede">Reylo infers type information from SQL queries, against a live database.</p>
        <Install />
      </div>
      <div className="ml-hero-visual rh-import">
        <div className="ml-pair">
          <div className="ml-win ml-editor" aria-hidden="true">
            <div className="ml-bar">
              <FileIcon />
              get_author.sql
            </div>
            <pre className="rh-code">
              <Code lines={data.intro.sql} />
            </pre>
          </div>
          <div className="ml-win ml-under rh-import-ts" aria-hidden="true">
            <div className="ml-bar">
              <FileIcon />
              main.ts
            </div>
            <pre className="rh-code">
              <Code lines={data.intro.ts} />
            </pre>
            <div className="rh-hover">
              <pre className="rh-code">
                <Code lines={data.intro.type} />
              </pre>
            </div>
          </div>
        </div>
      </div>
    </header>
  );
}

/* ── Features, the call to action and the foot ──────────────────────────── */

const FEATURES: { title: string; body: ReactNode; icon: ReactNode }[] = [
  {
    title: 'The full power of SQL',
    body: (
      <>
        Any syntax, function or operator your database supports, from window functions to JSON aggregation and <code>returning</code> clauses.
      </>
    ),
    icon: <path d="M4 7h16M4 12h10M4 17h13" />,
  },
  {
    title: 'No hand-written types',
    body: "Parameters, columns, nullability and cardinality come from the database's own analysis of each query. There is nothing to declare by hand.",
    icon: <path d="M8 4H6a2 2 0 0 0-2 2v4l-2 2 2 2v4a2 2 0 0 0 2 2h2M16 4h2a2 2 0 0 1 2 2v4l2 2-2 2v4a2 2 0 0 1-2 2h-2" />,
  },
  {
    title: 'Checked against your database',
    body: (
      <>
        A misspelled column is a type error at its line and column, and so is a wrong argument to <code>.run()</code>.
      </>
    ),
    icon: <path d="M5 12.5l4.5 4.5L19 7.5" />,
  },
  {
    title: 'Agent-friendly',
    body: 'A query and its types are one file. An agent edits the SQL, and the types follow, with nothing else to update.',
    icon: (
      <>
        <rect x="4" y="7" width="16" height="12" rx="3" />
        <path d="M12 3v4M9 12h.01M15 12h.01M9.5 15.5h5" />
      </>
    ),
  },
  {
    title: 'Postgres, MySQL, SQLite and SQL Server',
    body: 'Each in its own dialect, on Neon, Supabase, PlanetScale, Cloudflare D1, Turso and more.',
    icon: (
      <>
        <ellipse cx="12" cy="6" rx="7" ry="2.75" />
        <path d="M5 6v12c0 1.5 3.1 2.75 7 2.75s7-1.25 7-2.75V6M5 12c0 1.5 3.1 2.75 7 2.75s7-1.25 7-2.75" />
      </>
    ),
  },
  {
    title: 'Your own driver',
    body: (
      <>
        Runs on <code>pg</code>, postgres.js, Bun.sql, Neon, <code>mysql2</code>, <code>mariadb</code>, PlanetScale, <code>better-sqlite3</code>, <code>node:sqlite</code>, libSQL, Cloudflare D1, <code>mssql</code> and more, or through Drizzle and Knex.
      </>
    ),
    icon: <path d="M9 3v5M15 3v5M6 8h12v3a6 6 0 0 1-12 0V8ZM12 17v4" />,
  },
];

/** The databases and hosts in two rows, as even as their count allows, each centred. */
const LOGOS = [...DATABASES, ...HOSTS];
const LOGO_ROWS = [LOGOS.slice(0, Math.ceil(LOGOS.length / 2)), LOGOS.slice(Math.ceil(LOGOS.length / 2))];

function Features() {
  return (
    <section className="rh-features">
      <header className="rh-section-head">
        <h2>Why Reylo</h2>
      </header>
      <div className="rh-grid">
        {FEATURES.map((f) => (
          <div key={f.title} className="rh-feature">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              {f.icon}
            </svg>
            <h3>{f.title}</h3>
            <p>{f.body}</p>
          </div>
        ))}
      </div>
      <div className="rh-logos">
        {LOGO_ROWS.map((row, i) => (
          <div key={i} className="rh-logos-row">
            {row.map((t) => (
              <span key={t.name} title={t.name}>
                <svg viewBox={t.viewBox} fill="currentColor" aria-hidden="true">
                  {t.paths.map((d) => (
                    <path key={d} d={d} />
                  ))}
                </svg>
                {t.name.replace(/ and Aurora$/, '')}
              </span>
            ))}
          </div>
        ))}
      </div>
    </section>
  );
}

function End() {
  return (
    <section className="ml-end rh-end">
      <h2>Ship faster with Reylo</h2>
      <Install centered />
      <a className="rh-star-link" href={githubUrl}>
        Leave a star
      </a>
    </section>
  );
}

function Foot() {
  return (
    <footer className="ml-foot">
      <a className="ml-variadic" href="https://variadic.sh">
        <VariadicMark />A Variadic project
      </a>
      <div className="ml-foot-links">
        <a href={introUrl}>Docs</a>
        <a href="/inference">Inference</a>
        <a href="/play">Playground</a>
        <a href="/migrate">Migrate</a>
        <a href={githubUrl}>GitHub</a>
        <a href="https://www.npmjs.com/package/reylo">npm</a>
      </div>
      <span className="ml-license">Released under the MIT License.</span>
    </footer>
  );
}
