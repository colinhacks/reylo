'use client';

import { useSearchContext } from 'fumadocs-ui/contexts/search';
import { createContext, lazy, use, useEffect } from 'react';
import { flushSync } from 'react-dom';

// What opens the search of a site with several pages (README, "Search"): the field under the name
// in the left column, the icon in the top row and the field at the head of the list on a phone, and
// the `/` key. The dialog itself (./search.tsx) and the search it runs (../search/engine.ts) load
// apart from the page, and the index only once a reader goes to search.

const dialog = () => import('./search');

/** The dialog, for Fumadocs' search provider, which opens it on ⌘K and Ctrl+K. */
export const SearchDialog = lazy(dialog);

/** The address of the site's search index, or nothing on a site without search. */
export const SearchIndexUrl = createContext<string | undefined>(undefined);

export function SearchIcon({ size = 16 }: { size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="11" cy="11" r="7.5" />
      <path d="m20.5 20.5-4.2-4.2" />
    </svg>
  );
}

/**
 * How a control opens the search: at once, inside the press, so a phone raises its keyboard for
 * the input the dialog focuses; and the index is fetched as soon as a pointer or the keyboard
 * reaches the control, before the press.
 */
function useOpenSearch(before?: () => void) {
  const { setOpenSearch } = useSearchContext();
  const index = use(SearchIndexUrl);
  const warm = () => {
    if (index) void dialog().then((module) => module.loadIndex(index));
  };
  return {
    onPointerEnter: warm,
    onFocus: warm,
    onTouchStart: warm,
    onClick: () =>
      flushSync(() => {
        before?.();
        setOpenSearch(true);
      }),
  };
}

/** The search field's look, as a button: an icon, "Search", and the keys that open it. `onOpen` runs first, to close what holds the field. */
export function SearchField({ onOpen }: { onOpen?: () => void }) {
  const { enabled, hotKey } = useSearchContext();
  const open = useOpenSearch(onOpen);
  if (!enabled) return null;
  return (
    <button type="button" className="dk-search-field" aria-haspopup="dialog" aria-keyshortcuts="Meta+K Control+K /" {...open}>
      <SearchIcon size={15} />
      <span>Search</span>
      <kbd aria-hidden="true">
        {hotKey.map((key, i) => (
          <span key={i}>{key.display}</span>
        ))}
      </kbd>
    </button>
  );
}

/** The search as an icon, for a row of icons. */
export function SearchButton({ className }: { className?: string }) {
  const { enabled } = useSearchContext();
  const open = useOpenSearch();
  if (!enabled) return null;
  return (
    <button type="button" className={className} aria-label="Search" aria-haspopup="dialog" {...open}>
      <SearchIcon size={18} />
    </button>
  );
}

const typing = (target: EventTarget | null) => target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));

/** The `/` key opens the search from anywhere on the page but a field being typed in. */
export function SearchKeys() {
  const { setOpenSearch } = useSearchContext();
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== '/' || event.metaKey || event.ctrlKey || event.altKey || event.defaultPrevented || typing(event.target)) return;
      event.preventDefault();
      setOpenSearch(true);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [setOpenSearch]);
  return null;
}
