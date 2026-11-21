import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import { DocsBody, DocsPage, DocsTitle } from 'fumadocs-ui/layouts/docs/page';
import Link from 'fumadocs-core/link';
import type { TOCItemType } from 'fumadocs-core/toc';
import type { ComponentProps, CSSProperties, ReactNode } from 'react';
import { TocFooter, TocHeader, TocStar, TopRow } from './chrome';
import { PageListColumn, PageListMenu, pagesOf, type PageList } from './page-list';

/**
 * Fumadocs' docs grid with the table of contents moved to the left, where the sidebar tree would
 * be: the site is one page, so it has no tree. The areas and columns are Fumadocs' own
 * (fumadocs-ui/dist/layouts/docs/slots/container.js) with `toc` and `sidebar` swapped.
 */
const LEFT_TOC = {
  gridTemplate: `"toc toc header sidebar sidebar"
"toc toc toc-popover sidebar sidebar"
"toc toc main sidebar sidebar" 1fr / minmax(min-content, 1fr) var(--fd-toc-width) minmax(0, calc(var(--fd-layout-width) - var(--fd-sidebar-width) - var(--fd-toc-width))) var(--fd-sidebar-col) minmax(min-content, 1fr)`,
  // The table of contents (268px, 16.75rem) plus the page: one column (`--dk-column`, 40rem) and
  // Fumadocs' 2rem of padding on each side (`#nd-page` in style.css, "The reading measure").
  '--fd-layout-width': 'calc(var(--dk-column) + 20.75rem)',
} as CSSProperties;

/**
 * The table of contents without the h3s (and deeper) of the h2s whose anchors are in `fold`: a
 * site that wants the whole table on one screen (a window 900px tall, at Fumadocs' spacing, without
 * squeezing the entries closer) leaves out the h3s of its longest sections.
 * The headings stay on the page, with their anchors.
 */
export function foldToc(toc: TOCItemType[], fold: readonly string[] = []): TOCItemType[] {
  const folded = new Set(fold.map((anchor) => (anchor.startsWith('#') ? anchor : `#${anchor}`)));
  let inside = false;
  return toc.filter((item) => {
    if (item.depth === 2) inside = folded.has(item.url);
    return !(inside && item.depth > 2);
  });
}

type Tree = ComponentProps<typeof DocsLayout>['tree'];

/**
 * The previous and next pages, as two large links at the foot of the page, in the order the page
 * list gives them; pages that leave the docs (`external`) are passed over. A page the list doesn't
 * hold, or the first or last, shows the one link it has.
 */
function Pager({ list }: { list: PageList }) {
  const pages = pagesOf(list.groups).filter((p) => !p.external);
  const at = pages.findIndex((p) => p.href === list.current);
  if (at < 0) return null;
  const prev = pages[at - 1];
  const next = pages[at + 1];
  if (!prev && !next) return null;
  const name = (p: (typeof pages)[number]) => (p.code ? <code>{p.title}</code> : p.title);
  return (
    <nav className="dk-pager" aria-label="Previous and next pages">
      {prev ? (
        <Link href={prev.href} className="dk-pager-prev" rel="prev">
          <span className="dk-pager-label">
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M13.5 8h-11M7 3.5 2.5 8 7 12.5" />
            </svg>
            Previous
          </span>
          <span className="dk-pager-title">{name(prev)}</span>
        </Link>
      ) : null}
      {next ? (
        <Link href={next.href} className="dk-pager-next" rel="next">
          <span className="dk-pager-label">
            Next
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M2.5 8h11M9 3.5 13.5 8 9 12.5" />
            </svg>
          </span>
          <span className="dk-pager-title">{name(next)}</span>
        </Link>
      ) : null}
    </nav>
  );
}

/**
 * The whole page: no bar stays above it; the brand, the repository and the theme switch sit in the
 * left column around the table of contents, or in `TopRow` where Fumadocs hides that column. The
 * title is the page's h1; the frontmatter's description is the page's metadata only, so a page
 * that wants a subheader writes it as its first element (`<p className="dk-subheader">`), where it
 * can hold inline code.
 */
export type { PageList } from './page-list';

export function DocsShell({
  tree,
  title,
  toc,
  fold,
  centered,
  pages,
  pageTitle,
  brand,
  github,
  star = true,
  before,
  titleAside,
  footer,
  children,
}: {
  /** The page tree (`source.getPageTree()`). */
  tree: Tree;
  title: ReactNode;
  /** The page's table of contents (`page.data.toc`). */
  toc: TOCItemType[];
  /** The h2 anchors whose h3s stay out of the table of contents. */
  fold?: readonly string[];
  /**
   * A page with no table of contents: the width of its one column, which sits in the middle of the
   * window under the top row, on Fumadocs' own grid, whose outer columns share what the window leaves.
   */
  centered?: string;
  /**
   * A site of several pages: the pages in the left column and the table of contents in the right,
   * on Fumadocs' own grid, with no bar above them. Each column gives way to the sticky strip above
   * the page as the window narrows (style.css, "The page list"). `centered` then keeps the left
   * column and gives the page the right column's room.
   */
  pages?: PageList;
  /** With `pages`, the current page's name on the strip that opens the list on a phone. */
  pageTitle?: string;
  /** The site's mark and name, a link to the top of the page. */
  brand: ReactNode;
  /** The repository's URL. */
  github: string;
  /** Whether the table of contents ends with the repository's link and its "Leave a star!" (`TocStar`); off on a page that asks for a star elsewhere, so no page asks twice. */
  star?: boolean;
  /** Rendered before the title, outside the page's prose: a script of structured data, say. */
  before?: ReactNode;
  /** Rendered at the right end of the title's row, above its rule: `<CopyAgentPrompt />`, say. */
  titleAside?: ReactNode;
  /** Rendered at the foot of the column, under a hairline, in small muted text: the licence and the copyright, say. */
  footer?: ReactNode;
  /** The page's MDX, rendered. */
  children: ReactNode;
}) {
  const body = (
    <>
      {before}
      {titleAside ? (
        <div className="dk-title-row">
          <DocsTitle>{title}</DocsTitle>
          {titleAside}
        </div>
      ) : (
        <DocsTitle>{title}</DocsTitle>
      )}
      <DocsBody>{children}</DocsBody>
      {pages ? <Pager list={pages} /> : null}
      {footer ? <footer className="dk-footer">{footer}</footer> : null}
    </>
  );

  if (pages) {
    return (
      <DocsLayout
        tree={tree}
        sidebar={{ enabled: false }}
        nav={{ enabled: false }}
        containerProps={{
          'data-dk-pages': '',
          'data-dk-wide': centered ? '' : undefined,
          style: centered ? ({ '--dk-column': centered } as CSSProperties) : undefined,
        } as ComponentProps<typeof DocsLayout>['containerProps']}
      >
        <PageListColumn list={pages} brand={brand} github={github} />
        <TopRow brand={brand} github={github} paged />
        <PageListMenu list={pages} brand={brand} github={github} title={pageTitle ?? ''} />
        <DocsPage
          toc={centered ? [] : foldToc(toc, fold)}
          tableOfContent={centered ? { enabled: false } : { style: 'clerk', container: { className: 'ps-4 pt-8' }, footer: star && toc.length ? <TocStar github={github} /> : undefined }}
          tableOfContentPopover={centered ? { enabled: false } : undefined}
          breadcrumb={{ enabled: false }}
          footer={{ enabled: false }}
        >
          {body}
        </DocsPage>
      </DocsLayout>
    );
  }

  return (
    <DocsLayout
      tree={tree}
      sidebar={{ enabled: false }}
      nav={{ enabled: false }}
      containerProps={{ style: centered ? ({ '--dk-column': centered, '--fd-layout-width': 'calc(var(--dk-column) + 4rem)' } as CSSProperties) : LEFT_TOC }}
    >
      <TopRow brand={brand} github={github} centered={centered !== undefined} />
      <DocsPage
        toc={centered ? [] : foldToc(toc, fold)}
        tableOfContent={
          centered
            ? { enabled: false }
            : {
                style: 'clerk',
                header: <TocHeader brand={brand} />,
                footer: <TocFooter github={github} />,
                container: { className: 'justify-self-end ps-4 pt-8' },
              }
        }
        tableOfContentPopover={centered ? { enabled: false } : undefined}
        breadcrumb={{ enabled: false }}
        footer={{ enabled: false }}
      >
        {body}
      </DocsPage>
    </DocsLayout>
  );
}
