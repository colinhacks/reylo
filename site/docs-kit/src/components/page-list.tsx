'use client';

import Link from 'fumadocs-core/link';
import { useEffect, useState, type ReactNode } from 'react';
import { GitHubIcon, GitHubStar, ThemeRow } from './chrome';

/** A page of the list. */
export type PageListItem = { title: string; href: string };

/** A group of pages, under a label or, with none, the list's main run. */
export type PageListGroup = { label?: string; items: PageListItem[] };

/** The page list a site with several pages shows in its left column. */
export type PageList = {
  groups: PageListGroup[];
  /** Links away from the docs, under a hairline at the foot of the list. */
  links?: PageListItem[];
  /** The label over the links, as a group's label. */
  linksLabel?: string;
  /** The address of the page being read, which the list marks. */
  current: string;
};

function Groups({ list, onPick }: { list: PageList; onPick?: () => void }) {
  return (
    <nav className="dk-pages" aria-label="Pages">
      {list.groups.map((group, i) => (
        <div key={group.label ?? i} className="dk-pages-group">
          {group.label ? <div className="dk-pages-label">{group.label}</div> : null}
          {group.items.map((item) => (
            <Link key={item.href} href={item.href} className="dk-pages-item" aria-current={item.href === list.current ? 'page' : undefined} onClick={onPick}>
              {item.title}
            </Link>
          ))}
        </div>
      ))}
    </nav>
  );
}

function Foot({ list, github, star }: { list: PageList; github: string; star: boolean }) {
  return (
    <div className="dk-pages-foot">
      {list.linksLabel && list.links?.length ? <div className="dk-pages-label">{list.linksLabel}</div> : null}
      {list.links?.map((link) => (
        <a key={link.href} href={link.href} className="dk-pages-link">
          {link.title}
          <span aria-hidden className="dk-pages-out">
            ↗
          </span>
        </a>
      ))}
      <ThemeRow>{star ? <GitHubStar github={github} /> : <a href={github} rel="noreferrer noopener" target="_blank" className="dk-pages-link"><GitHubIcon className="size-3.5 shrink-0" />GitHub</a>}</ThemeRow>
    </div>
  );
}

/** The left column: the name, the pages, and at its foot the links away from the docs, the theme switch and the note under the repository's link. */
export function PageListColumn({ list, brand, github }: { list: PageList; brand: ReactNode; github: string }) {
  return (
    <aside id="dk-page-list">
      <div className="dk-pages-brand">{brand}</div>
      <div className="dk-pages-scroll">
        <Groups list={list} />
      </div>
      <Foot list={list} github={github} star />
    </aside>
  );
}

function MenuIcon({ open }: { open?: boolean }) {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
      {open ? <path d="M18 6 6 18M6 6l12 12" /> : <path d="M4 6h16M4 12h16M4 18h16" />}
    </svg>
  );
}

/**
 * Below 64rem, where the left column is gone: the left end of the sticky strip above the page, the
 * current page's name, which opens the same list full screen. The strip's other half is the
 * headings' popover, whose trigger leaves room for this button (style.css).
 */
export function PageListMenu({ list, brand, github, title }: { list: PageList; brand: ReactNode; github: string; title: string }) {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!open) return;
    const { overflow } = document.body.style;
    document.body.style.overflow = 'hidden';
    return () => {
      document.body.style.overflow = overflow;
    };
  }, [open]);
  return (
    <>
      <button type="button" className="dk-pages-menu" aria-expanded={open} aria-label="Open the list of pages" onClick={() => setOpen(true)}>
        <MenuIcon />
        <span>{title}</span>
      </button>
      {open ? (
        <div className="dk-pages-sheet" role="dialog" aria-modal="true" aria-label="Pages">
          <div className="dk-pages-sheet-head">
            {brand}
            <button type="button" aria-label="Close the list of pages" onClick={() => setOpen(false)}>
              <MenuIcon open />
            </button>
          </div>
          <div className="dk-pages-scroll">
            <Groups list={list} onPick={() => setOpen(false)} />
          </div>
          <Foot list={list} github={github} star={false} />
        </div>
      ) : null}
    </>
  );
}
