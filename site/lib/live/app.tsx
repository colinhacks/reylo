import { Fragment, useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import type { Catalog } from '../../../src/play-catalog.ts';
import type { PlayResult } from '../../../src/play-result.ts';
import { Wordmark } from '../../components/wordmark.tsx';
import { ApiDialog, type ApiDocs } from './api-dialog.tsx';
import type { Editor } from './editor.ts';
import type { Token } from './highlight.ts';

/*
 * The playground, on the docs' /play (components/play.tsx) and in `reylo play` (lib/live/standalone.tsx):
 * a bar with the database's state, the query on the left with the examples at the top right of its
 * pane, and on the right the types Reylo infers for it, Params and Returns, over which "View schema"
 * slides the schema it runs against, as a card closed by its × or Escape. The page opens drawn with
 * what it was given, an example's types included; then it connects to what analyzes each statement
 * (the docs' worker, lib/live/worker.ts, or `reylo play`'s server) and loads the editor
 * (lib/live/editor.ts), and every edit is analyzed as it is typed. The address names the example,
 * `?example=`, or holds an edited query, `?q=`, to share.
 */

export type LiveResult = PlayResult;

/** One of the docs' Inference examples, with its types as the build drew them. */
export type Example = {
  /** The subsection's heading. */
  title: string;
  /** Its anchor on the main page, which the address names: `?example=nested-fetching`. */
  slug: string;
  sql: string;
  sqlTokens: Token[][];
  result: LiveResult;
  params?: Token[][];
  returns: Token[][];
};

export type PlayData = {
  /** The schema, highlighted, or its text, highlighted once the highlighter loads. */
  schema: Token[][] | string;
  examples: Example[];
  /** The slug of the example the page opens with; with no examples, the query it opens with is `query`. */
  start?: string;
  query?: string;
  catalog: Catalog;
};

/** What analyzes the statements: the docs' worker, or `reylo play`'s server, behind a worker's interface. */
export type Channel = {
  postMessage(message: { sql: string }): void;
  onmessage: ((event: MessageEvent<FromChannel>) => void) | null;
  onerror: ((event: ErrorEvent) => void) | null;
  terminate(): void;
};

export type FromChannel = { kind: 'ready'; label: string } | { kind: 'result'; result: LiveResult } | { kind: 'failed'; message: string };

export type Labels = {
  /** While it connects: "Starting Postgres 18…". */
  loading: string;
  /** Before the reason it failed: "Postgres could not start in this browser". */
  failed: string;
};

type Status = { kind: 'loading' } | { kind: 'ready'; label: string; ms?: number } | { kind: 'failed'; message: string };

// What a statement that takes no parameters takes, as the types are written.
const voidLines: Token[][] = [[{ content: 'void', style: '--shiki-light:#D73A49;--shiki-dark:#F97583' }]];

const SHIKI_VARS = { '--shiki-light': '#24292e', '--shiki-dark': '#e1e4e8', '--shiki-light-bg': '#fff', '--shiki-dark-bg': '#24292e' } as CSSProperties;

function styleObject(style: string): CSSProperties | undefined {
  if (!style) return undefined;
  return Object.fromEntries(style.split(';').filter(Boolean).map((d) => d.split(':') as [string, string])) as CSSProperties;
}

/** Highlighted lines as the docs draw a code block, each line's own indent as `--indent` for its hanging wrap. */
function Lines({ lines }: { lines: Token[][] }) {
  return (
    <pre>
      <code>
        {lines.map((line, n) => {
          const indent = /^ */.exec(line.map((t) => t.content).join(''))![0].length;
          return (
            <Fragment key={n}>
              <span className="line" style={indent ? ({ '--indent': `${indent}ch` } as CSSProperties) : undefined}>
                {line.map((t, k) => (
                  <span key={k} style={styleObject(t.style)}>
                    {t.content}
                  </span>
                ))}
              </span>
              {n < lines.length - 1 ? '\n' : null}
            </Fragment>
          );
        })}
      </code>
    </pre>
  );
}

function Code({ lines, className, children }: { lines?: Token[][]; className?: string; children?: ReactNode }) {
  return (
    <figure dir="ltr" className={`shiki shiki-themes github-light github-dark not-prose ${className ?? ''}`} style={SHIKI_VARS}>
      {lines ? <Lines lines={lines} /> : children}
    </figure>
  );
}

const Icon = ({ d, size = 16, stroke = 1.75 }: { d: string; size?: number; stroke?: number }) => (
  <svg viewBox="0 0 24 24" width={size} height={size} fill="none" stroke="currentColor" strokeWidth={stroke} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" dangerouslySetInnerHTML={{ __html: d }} />
);
const DOWN = '<path d="m6 9 6 6 6-6"/>';
const CLOSE = '<path d="M18 6 6 18M6 6l12 12"/>';
const TABLE = '<ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5"/><path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"/>';
const CHECK = '<path d="M20 6 9 17l-5-5"/>';
const SUN = '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/>';
const MOON = '<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/>';
const GITHUB = '<path d="M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.4 5.4 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4"/><path d="M9 18c-4.51 2-5-2-7-2"/>';

function StatusLine({ status, labels }: { status: Status; labels: Labels }) {
  return (
    <span className="play-status" data-kind={status.kind} role="status">
      <span className="play-dot" aria-hidden="true" />
      {status.kind === 'loading' && labels.loading}
      {status.kind === 'ready' && status.label}
      {status.kind === 'failed' && `${labels.failed}: ${status.message}`}
    </span>
  );
}

/** The page's theme and how to change it: the docs' theme provider, or `reylo play`'s own. */
export type Theme = { resolved: string | undefined; set(theme: 'light' | 'dark'): void };

/** The light and dark switch: one icon button, the theme in use, drawn like the GitHub link beside it. */
function ThemeButton({ theme }: { theme: Theme }) {
  const { resolved: resolvedTheme, set: setTheme } = theme;
  const next = resolvedTheme === 'light' ? 'dark' : 'light';
  return (
    <button type="button" className="play-icon-button play-theme" aria-label={`Switch to the ${next} theme`} title={`Switch to the ${next} theme`} onClick={() => setTheme(next)}>
      {/* Both drawn, one shown by the page's theme class, so nothing changes as the page hydrates. */}
      <span className="play-theme-dark">
        <Icon d={MOON} />
      </span>
      <span className="play-theme-light">
        <Icon d={SUN} />
      </span>
    </button>
  );
}

/** The examples' groups in the menu, by each example's anchor on the main page; an example in none is under More. */
const GROUPS: [string, string[]][] = [
  ['Reading', ['column-types', 'left-joins', 'computed-columns', 'unique-keys', 'aggregates', 'subqueries', 'unions']],
  ['Parameters', ['naming', 'casts', 'nullable-parameters', 'dynamic-sorting', 'dynamic-filtering', 'filtering-by-a-list']],
  ['JSON', ['json', 'nested-fetching', 'typed-json']],
  ['Writing', ['returning-inserts', 'bulk-inserts', 'nested-inserts']],
];

function grouped(examples: Example[]): [string, Example[]][] {
  const listed = new Set(GROUPS.flatMap(([, slugs]) => slugs));
  const groups = GROUPS.map(([name, slugs]) => [name, slugs.map((slug) => examples.find((e) => e.slug === slug)).filter((e): e is Example => !!e)] as [string, Example[]]);
  const rest = examples.filter((e) => !listed.has(e.slug));
  return [...groups, ...(rest.length ? [['More', rest] as [string, Example[]]] : [])].filter(([, items]) => items.length);
}

function ExamplePicker({ examples, current, onPick }: { examples: Example[]; current?: string; onPick(example: Example): void }) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const groups = grouped(examples);
  const order = groups.flatMap(([, items]) => items);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => !root.current?.contains(e.target as Node) && setOpen(false);
    document.addEventListener('pointerdown', away);
    // The current example in view, and the keys on the list.
    setActive(Math.max(0, order.findIndex((e) => e.slug === current)));
    requestAnimationFrame(() => {
      list.current?.focus({ preventScroll: true });
      list.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' });
    });
    return () => document.removeEventListener('pointerdown', away);
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (open) list.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [active, open]);
  const choose = (e: Example) => {
    setOpen(false);
    onPick(e);
  };
  const title = examples.find((e) => e.slug === current)?.title;
  return (
    <div className="play-picker" ref={root}>
      <button type="button" className="play-button" aria-haspopup="listbox" aria-expanded={open} data-open={open || undefined} onClick={() => setOpen(!open)}>
        <span className="play-picker-label" data-custom={title ? undefined : true}>
          {title ?? 'Pick an example query'}
        </span>
        <Icon d={DOWN} />
      </button>
      {open && (
        <div
          ref={list}
          className="play-menu"
          role="listbox"
          aria-label="Examples"
          tabIndex={-1}
          aria-activedescendant={`play-example-${active}`}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown') setActive((i) => Math.min(order.length - 1, i + 1));
            else if (e.key === 'ArrowUp') setActive((i) => Math.max(0, i - 1));
            else if (e.key === 'Home') setActive(0);
            else if (e.key === 'End') setActive(order.length - 1);
            else if (e.key === 'Enter') choose(order[active]!);
            else if (e.key === 'Escape') setOpen(false);
            else return;
            e.preventDefault();
          }}
        >
          {groups.map(([name, items]) => (
            <div key={name} role="group" aria-label={name}>
              <div className="play-menu-group" aria-hidden="true">
                {name}
              </div>
              {items.map((e) => {
                const index = order.indexOf(e);
                return (
                  <div
                    key={e.slug}
                    id={`play-example-${index}`}
                    data-index={index}
                    role="option"
                    aria-selected={e.slug === current}
                    data-active={index === active || undefined}
                    className="play-menu-item"
                    onPointerMove={() => setActive(index)}
                    onClick={() => choose(e)}
                  >
                    <span>{e.title}</span>
                    {e.slug === current && <Icon d={CHECK} size={14} />}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** The query it opens with when there are no examples: its types empty, shown as `…`, until its first answer comes. */
const nothing = (sql: string): LiveResult => ({ sql, params: '', returns: '', diagnostics: [], ms: 0 });

export function PlayApp({ data, connect, theme, labels, home, github, api }: { data: PlayData; connect(): Channel; theme: Theme; labels: Labels; home: string; github: string; api: ApiDocs }) {
  const [apiOpen, setApiOpen] = useState(false);
  // One object per page's data: the effects below depend on it.
  const first = useMemo(() => data.examples.find((e) => e.slug === data.start) ?? { sql: data.query ?? '', result: nothing(data.query ?? ''), slug: undefined }, [data]);
  const [sql, setSql] = useState(first.sql);
  // The example the reader picked, or the address names: none at first, though the editor opens on one.
  const [example, setExample] = useState<string | undefined>(undefined);
  const [result, setResult] = useState<LiveResult>(first.result);
  // The last answer without errors, whose types stay up, dimmed, while the query has an error.
  const [good, setGood] = useState<LiveResult>(first.result);
  const [status, setStatus] = useState<Status>({ kind: 'loading' });
  const [schemaOpen, setSchemaOpen] = useState(false);
  const viewSchema = useRef<HTMLButtonElement>(null);
  const closeSchema = useRef<HTMLButtonElement>(null);
  const [highlight, setHighlight] = useState<{ fn: (text: string, lang: 'sql' | 'ts') => Token[][] }>();
  const [editing, setEditing] = useState(false);
  const editorElement = useRef<HTMLDivElement>(null);
  const editor = useRef<Editor>(undefined);
  const worker = useRef<Channel>(undefined);
  const latest = useRef(first.sql);
  // When the reader last typed, and an answer with errors waiting for them to pause.
  const lastEdit = useRef(0);
  const pending = useRef<ReturnType<typeof setTimeout>>(undefined);

  // The address follows the query: the example's name while it is one, else the text itself.
  const remember = useCallback(
    (text: string) => {
      const url = new URL(location.href);
      const named = data.examples.find((e) => e.sql === text);
      url.searchParams.delete('q');
      url.searchParams.delete('example');
      if (named && named.slug !== data.start) url.searchParams.set('example', named.slug);
      else if (!named) url.searchParams.set('q', text);
      history.replaceState(history.state, '', url);
      setExample(named?.slug);
    },
    [data],
  );

  useEffect(() => {
    const params = new URLSearchParams(location.search);
    const named = data.examples.find((e) => e.slug === params.get('example'));
    const start = params.get('q') ?? named?.sql ?? first.sql;
    latest.current = start;
    if (start !== first.sql) {
      setSql(start);
      setExample(data.examples.find((e) => e.sql === start)?.slug);
      const built = data.examples.find((e) => e.sql === start);
      if (built) {
        setResult(built.result);
        setGood(built.result);
      }
    }

    const w = connect();
    worker.current = w;
    w.onmessage = (event: MessageEvent<FromChannel>) => {
      const m = event.data;
      if (m.kind === 'ready') setStatus({ kind: 'ready', label: m.label });
      else if (m.kind === 'failed') setStatus({ kind: 'failed', message: m.message });
      else if (m.result.sql === latest.current) {
        const answer = m.result;
        clearTimeout(pending.current);
        const apply = () => {
          if (answer.sql !== latest.current) return;
          // Not over the completions: the errors wait until the list closes.
          if (answer.diagnostics.length && editor.current?.completing()) {
            pending.current = setTimeout(apply, 250);
            return;
          }
          setResult(answer);
          if (!answer.diagnostics.length) setGood(answer);
          setStatus((s) => (s.kind === 'ready' ? { ...s, ms: answer.ms } : s));
          editor.current?.setDiagnostics(answer.diagnostics);
        };
        // A statement half typed is an error for a moment: its errors show once the reader pauses,
        // and a clean answer shows at once.
        const wait = answer.diagnostics.length ? 600 - (performance.now() - lastEdit.current) : 0;
        if (wait > 0) pending.current = setTimeout(apply, wait);
        else apply();
      }
    };
    w.onerror = (e) => setStatus({ kind: 'failed', message: e.message || 'the worker stopped' });
    w.postMessage({ sql: start });

    let cancelled = false;
    Promise.all([import('./editor.ts'), import('./highlight.ts')]).then(async ([e, h]) => {
      const core = await h.loadHighlighter();
      if (cancelled || !editorElement.current) return;
      setHighlight({ fn: (text, lang) => h.tokens(core, text, lang) });
      editor.current = e.createEditor(editorElement.current, start, core, data.catalog, (text) => {
        latest.current = text;
        lastEdit.current = performance.now();
        setSql(text);
        // An example's types are known from the build until the tab's own answer comes.
        const built = data.examples.find((x) => x.sql === text);
        if (built) {
          setResult(built.result);
          setGood(built.result);
        }
        w.postMessage({ sql: text });
        remember(text);
      });
      setEditing(true);
      const view = editor.current.view;
      requestAnimationFrame(() => {
        view.requestMeasure();
        view.focus();
        view.dispatch({ selection: { anchor: view.state.doc.length } });
      });
    });
    return () => {
      cancelled = true;
      clearTimeout(pending.current);
      w.terminate();
      editor.current?.view.destroy();
      editor.current = undefined;
    };
  }, [data, first, remember, connect]);

  useEffect(() => {
    if (!schemaOpen) return;
    const escape = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      setSchemaOpen(false);
      viewSchema.current?.focus({ focusVisible: false } as FocusOptions);
    };
    document.addEventListener('keydown', escape);
    return () => document.removeEventListener('keydown', escape);
  }, [schemaOpen]);

  const pick = (e: Example) => {
    if (editor.current) {
      editor.current.setText(e.sql);
      editor.current.view.focus();
    } else {
      latest.current = e.sql;
      setSql(e.sql);
      setResult(e.result);
      setGood(e.result);
      setExample(e.slug);
      worker.current?.postMessage({ sql: e.sql });
      remember(e.sql);
    }
  };

  // The types shown: the statement's own, or while it has an error the last good ones, dimmed. An
  // example's come from the build until the tab's own answer replaces them, and any other's are
  // highlighted once the highlighter has loaded.
  const failing = result.diagnostics.length > 0;
  const shown = failing ? good : result;
  const built = data.examples.find((e) => e.result === shown);
  const typeLines = (text: string | undefined, fromBuild: Token[][] | undefined) => (built ? fromBuild : text && highlight ? highlight.fn(text, 'ts') : undefined);
  const params = typeLines(shown.params, built?.params);
  const returns = typeLines(shown.returns, built?.returns);
  const initialLines = data.examples.find((e) => e.sql === sql)?.sqlTokens ?? sql.split('\n').map((l) => [{ content: l, style: '' }]);
  const schemaLines = typeof data.schema !== 'string' ? data.schema : highlight ? highlight.fn(data.schema, 'sql') : data.schema.split('\n').map((l) => [{ content: l, style: '' }]);

  return (
    <div id="nd-page" className="play">
      <header className="play-bar">
        <a href={home} className="play-brand" aria-label="Reylo docs">
          <Wordmark className="play-wordmark" />
        </a>
        <StatusLine status={status} labels={labels} />
        <span className="play-grow" />
        <nav className="play-nav">
          <button type="button" className="play-nav-link" aria-haspopup="dialog" onClick={() => setApiOpen(true)}>
            API
          </button>
          <a href={home} className="play-nav-link">
            Docs
          </a>
          <a href={github} aria-label="GitHub" title="GitHub" className="play-icon-button">
            <Icon d={GITHUB} />
          </a>
          <ThemeButton theme={theme} />
        </nav>
      </header>
      <ApiDialog api={api} open={apiOpen} onClose={() => setApiOpen(false)} />
      <main className="play-main">
        <section className="play-pane play-query" aria-label="Query">
          <div className="play-pane-head">
            <span className="play-label">Query</span>
            <span className="play-grow" />
            {data.examples.length > 0 && <ExamplePicker examples={data.examples} current={example} onPick={pick} />}
          </div>
          <div
            className="play-scroll play-editor"
            onPointerDown={(e) => {
              // Below the last line, or beside the code: the editor takes the click, at the end.
              const view = editor.current?.view;
              if (!view || view.contentDOM.contains(e.target as Node)) return;
              e.preventDefault();
              view.focus();
              const at = view.posAtCoords({ x: e.clientX, y: e.clientY });
              view.dispatch({ selection: { anchor: at ?? view.state.doc.length } });
            }}
          >
            <Code className="play-code">
              {/* The query as the build drew it, until the editor takes its place in a host React never fills. */}
              {!editing && <Lines lines={initialLines} />}
              <div ref={editorElement} className="play-editor-host" hidden={!editing} />
            </Code>
          </div>
        </section>
        <aside className="play-pane play-side" aria-label="Inferred types and schema">
          {/* At the pane's top right, on the line of the Params label, taking no row of its own. */}
          <div className="play-side-actions">
            <button
              type="button"
              ref={viewSchema}
              className="play-button"
              aria-expanded={schemaOpen}
              aria-controls="play-schema"
              onClick={() => {
                setSchemaOpen(true);
                // Keys go to the card (Escape, Tab to its ×), with no focus ring drawn for a click.
                requestAnimationFrame(() => closeSchema.current?.focus({ preventScroll: true, focusVisible: false } as FocusOptions));
              }}
            >
              <Icon d={TABLE} />
              View schema
            </button>
          </div>
          <div className="play-scroll play-types" data-stale={failing || undefined}>
            <div className="play-row">
              <span className="play-label">Params</span>
              {shown.params === undefined ? <Code lines={voidLines} /> : params ? <Code lines={params} /> : <p className="play-none">…</p>}
            </div>
            <div className="play-row">
              <span className="play-label">Returns</span>
              {returns ? <Code lines={returns} /> : <p className="play-none">…</p>}
            </div>
            {failing && (
              <p className="play-types-error">
                <span className="play-dot" aria-hidden="true" />
                Types update when the error is fixed
              </p>
            )}
          </div>
          {/* The schema: a card that slides in over the whole pane, closed by its × or Escape. */}
          <section id="play-schema" className="play-schema" data-open={schemaOpen || undefined} aria-label="Schema" inert={!schemaOpen}>
            <div className="play-schema-head">
              <button
                type="button"
                ref={closeSchema}
                className="play-x"
                aria-label="Close the schema"
                onClick={() => {
                  setSchemaOpen(false);
                  viewSchema.current?.focus({ focusVisible: false } as FocusOptions);
                }}
              >
                <Icon d={CLOSE} size={18} stroke={1.5} />
              </button>
              <span className="play-label">Schema</span>
            </div>
            <div className="play-scroll">
              <Code lines={schemaLines} />
            </div>
          </section>
        </aside>
      </main>
    </div>
  );
}
