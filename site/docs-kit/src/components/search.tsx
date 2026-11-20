'use client';

import Link from 'fumadocs-core/link';
import { use, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { prepare, search, type Marked, type PageResult, type Prepared, type SearchIndex } from '../search/engine';
import { SearchIcon, SearchIndexUrl } from './search-trigger';

// The search dialog of a site with several pages (README, "Search"): one field over the pages that
// match what is typed, each with the sections of it that matched, as the reader types. The index
// is one file the site builds (../search/index.ts) and the search runs here (../search/engine.ts),
// so there is no server and a keystroke costs about a millisecond.
//
// The field keeps the focus the whole time. The arrow keys move a mark through the results, Enter
// follows the marked one (in a new tab with ⌘ or Ctrl), and Escape closes, as does a press outside.
// The results are links, so a click, a middle click and "copy link address" work as on any link.

const indexes = new Map<string, Promise<Prepared>>();

/** The site's index, fetched and read once. A fetch that failed is made again the next time. */
export function loadIndex(url: string): Promise<Prepared> {
  let index = indexes.get(url);
  if (!index) {
    index = fetch(url)
      .then((response) => {
        if (!response.ok) throw new Error(`docs-kit search: ${url} answered ${response.status}`);
        return response.json() as Promise<SearchIndex>;
      })
      .then(prepare);
    index.catch(() => indexes.delete(url));
    indexes.set(url, index);
  }
  return index;
}

const ICON = { viewBox: '0 0 24 24', width: 16, height: 16, fill: 'none', stroke: 'currentColor', strokeWidth: 1.75, strokeLinecap: 'round', strokeLinejoin: 'round', 'aria-hidden': true } as const;

const pageIcon = (
  <svg {...ICON}>
    <path d="M14.5 2.5H7a2 2 0 0 0-2 2v15a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7Z" />
    <path d="M14.5 2.5V7H19M9 12.5h6M9 16.5h6" />
  </svg>
);

const sectionIcon = (
  <svg {...ICON}>
    <path d="M4.5 9h15M4.5 15h15M10 3.5l-2 17M16 3.5l-2 17" />
  </svg>
);

/** Text with its code as code and the parts the query matched marked. */
function Text({ value }: { value: Marked }) {
  const plain = (from: number, to: number) => {
    const out: ReactNode[] = [];
    let at = from;
    for (const [start, end] of value.marks) {
      if (end <= from || start >= to) continue;
      const first = Math.max(start, from);
      if (first > at) out.push(value.text.slice(at, first));
      at = Math.min(end, to);
      out.push(<mark key={first}>{value.text.slice(first, at)}</mark>);
    }
    if (at < to) out.push(value.text.slice(at, to));
    return out;
  };
  const out: ReactNode[] = [];
  let at = 0;
  for (const [start, end] of value.code) {
    out.push(...plain(at, start), <code key={`code${start}`}>{plain(start, end)}</code>);
    at = end;
  }
  out.push(...plain(at, value.text.length));
  return out;
}

/** Whether a text is one piece of code from end to end: a line of a code block. */
const allCode = (value: Marked) => value.code.length === 1 && value.code[0]![0] === 0 && value.code[0]![1] === value.text.length;

type Row = { id: string; at: number; href: string; section: boolean; trail?: string; title: Marked; excerpt?: Marked };

/**
 * The results as the rows the dialog draws, a page's row and then its sections'. A row shows the
 * text under its title when the query matched in that text; a page with no section to show always
 * does, so a page found by its title says what it is.
 */
function rowsOf(results: PageResult[], id: string): { groups: Row[][]; rows: Row[] } {
  const rows: Row[] = [];
  const row = (entry: Omit<Row, 'id' | 'at'>) => {
    const added = { ...entry, id: `${id}row${rows.length}`, at: rows.length };
    rows.push(added);
    return added;
  };
  const matched = (text?: Marked) => (text?.marks.length ? text : undefined);
  const groups = results.map((page) => [
    row({ href: page.href, section: false, title: page.title, excerpt: page.sections.length === 0 ? page.excerpt : matched(page.excerpt) }),
    ...page.sections.map((section) => row({ href: section.href, section: true, trail: section.trail, title: section.title, excerpt: matched(section.excerpt) })),
  ]);
  return { groups, rows };
}

export default function Search({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const index = use(SearchIndexUrl);
  const id = useId();
  const dialog = useRef<HTMLDialogElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState('');
  const [data, setData] = useState<Prepared | 'failed'>();
  /** The row the reader moved the mark to, and the results it was among: new results put the mark back on the best. */
  const [picked, setPicked] = useState<{ among: PageResult[]; at: number }>();
  const byKey = useRef(false);
  const pressedOutside = useRef(false);

  // Opened in the same task as the press that asked, so a phone raises its keyboard for the field.
  useLayoutEffect(() => {
    const element = dialog.current!;
    if (open && !element.open) {
      element.showModal();
      input.current!.focus();
      input.current!.select();
    } else if (!open && element.open) element.close();
  }, [open]);

  // A fetch that failed is made again as the reader goes on typing.
  const retry = data === 'failed' ? query : '';
  useEffect(() => {
    if (!open || !index) return;
    let live = true;
    loadIndex(index).then(
      (prepared) => live && setData(prepared),
      () => live && setData('failed'),
    );
    return () => {
      live = false;
    };
  }, [open, index, retry]);

  const asked = query.trim().length > 1;
  const results = useMemo(() => (asked && typeof data === 'object' ? search(data, query) : []), [asked, data, query]);
  const { groups, rows } = useMemo(() => rowsOf(results, id), [results, id]);
  const active = picked?.among === results ? picked.at : results[0]?.sectionLeads ? 1 : 0;

  useLayoutEffect(() => {
    if (list.current) list.current.scrollTop = 0;
  }, [results]);
  useEffect(() => {
    if (byKey.current) document.getElementById(rows[active]?.id ?? '')?.scrollIntoView({ block: 'nearest' });
    byKey.current = false;
  }, [active, rows]);

  let note: ReactNode = null;
  if (asked && data === 'failed') note = <p className="dk-search-note">The search could not load. Check the connection.</p>;
  else if (asked && !data) note = <p className="dk-search-note dk-search-wait">Loading…</p>;
  else if (asked && rows.length === 0) note = <p className="dk-search-note">No results for “{query.trim()}”</p>;

  return (
    // The role is the element's own; written out, it is how Fumadocs' theme key (`d`) knows to stay out of a dialog.
    <dialog
      ref={dialog}
      role="dialog"
      className="dk-search"
      aria-label="Search"
      onClose={() => onOpenChange(false)}
      onPointerDown={(event) => {
        pressedOutside.current = event.target === event.currentTarget;
      }}
      onClick={(event) => {
        // A press on the backdrop is a press on the dialog itself; a selection dragged out of the field ends there too, and is not one.
        if (pressedOutside.current && event.target === event.currentTarget) onOpenChange(false);
      }}
    >
      <div className="dk-search-head">
        <SearchIcon size={18} />
        <input
          ref={input}
          value={query}
          placeholder="Search the docs"
          type="text"
          inputMode="search"
          enterKeyHint="go"
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          spellCheck={false}
          role="combobox"
          aria-label="Search the docs"
          aria-autocomplete="list"
          aria-expanded={rows.length > 0}
          aria-controls={`${id}list`}
          aria-activedescendant={rows[active]?.id}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing) return;
            if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
              event.preventDefault();
              if (rows.length === 0) return;
              byKey.current = true;
              setPicked({ among: results, at: (active + (event.key === 'ArrowDown' ? 1 : rows.length - 1)) % rows.length });
            } else if (event.key === 'Enter') {
              const row = document.getElementById(rows[active]?.id ?? '');
              if (!(row instanceof HTMLAnchorElement)) return;
              event.preventDefault();
              if (event.metaKey || event.ctrlKey) window.open(row.href, '_blank', 'noopener');
              else row.click();
            }
          }}
        />
        <button type="button" className="dk-search-close" aria-label="Close the search" onClick={() => onOpenChange(false)}>
          <kbd>esc</kbd>
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
            <path d="M18 6 6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      {note}
      <div
        ref={list}
        id={`${id}list`}
        role="listbox"
        aria-label="Results"
        className="dk-search-list"
        hidden={rows.length === 0}
        // A press in the list leaves the focus in the field; on a phone, dragging the list puts the keyboard away.
        onMouseDown={(event) => event.preventDefault()}
        onTouchMove={() => input.current?.blur()}
      >
        {groups.map((group) => (
          <div key={group[0]!.href} role="group" className="dk-search-group">
            {group.map((row) => (
              <Link
                key={row.id}
                id={row.id}
                href={row.href}
                role="option"
                aria-selected={row.at === active}
                tabIndex={-1}
                className="dk-search-row"
                data-section={row.section ? '' : undefined}
                onPointerMove={() => {
                  if (row.at !== active) setPicked({ among: results, at: row.at });
                }}
                onClick={(event) => {
                  // A click that opens the result elsewhere (a new tab, a new window) leaves the dialog open for the next.
                  if (!(event.metaKey || event.ctrlKey || event.shiftKey)) onOpenChange(false);
                }}
              >
                {row.section ? sectionIcon : pageIcon}
                <span className="dk-search-title">
                  {row.trail ? <span className="dk-search-trail">{row.trail}</span> : null}
                  <Text value={row.title} />
                </span>
                {row.excerpt ? (
                  <span className="dk-search-excerpt" data-code={allCode(row.excerpt) ? '' : undefined}>
                    <Text value={row.excerpt} />
                  </span>
                ) : null}
              </Link>
            ))}
          </div>
        ))}
      </div>
      <p role="status" className="sr-only">
        {asked && typeof data === 'object' ? (rows.length === 0 ? 'No results' : `${results.length} ${results.length === 1 ? 'page' : 'pages'} found`) : ''}
      </p>
    </dialog>
  );
}
