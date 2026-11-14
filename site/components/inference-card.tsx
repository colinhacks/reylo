import type { ReactNode } from 'react';
import { DIALECTS } from '@/lib/dialects';
import { DialectSwitch } from './dialect-tabs';

/*
 * The inference cards: a statement with the types Reylo infers for it, in one card of
 * labelled rows, Schema (where the example needs one), Query, Params and Returns, every row
 * visible at once, scannable without a click on a tab. The page writes `<InferenceCard>` around a schema block
 * and a statement; lib/remark-mapper.ts fills it with a panel per dialect, the rows read off the
 * statement's captured module, which follow the page's dialect choice, and the switch here sets it.
 */

/** The card. `layout` is `side` (the parameters and the result beside the query) or `rows` (under it); `bare` drops the dialect switch. */
export function InferenceCard({ dialects, layout = 'side', bare, children }: { dialects?: string; layout?: 'side' | 'rows'; bare?: boolean; children: ReactNode }) {
  const shown = dialects ? DIALECTS.filter((d) => dialects.split(',').includes(d)) : DIALECTS;
  return (
    <div className="reylo-inference-card" data-layout={layout}>
      {/* On a database's own page, `bare`: no switch, and no label naming the one database. */}
      {!bare && (
        <div className="reylo-inference-card-switch">
          <DialectSwitch dialects={shown.length < DIALECTS.length ? shown.join(',') : undefined} />
        </div>
      )}
      {children}
    </div>
  );
}

/** One labelled row of the card: its label, then its code. */
export function InferenceCardRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="reylo-inference-card-row" data-row={label.toLowerCase()}>
      <div className="reylo-inference-card-label">{label}</div>
      <div className="reylo-inference-card-body">{children}</div>
    </div>
  );
}

/** A quiet link under the card, at its right, that opens its Postgres statement in the playground (/play). */
export function InferenceCardPlay({ href }: { href: string }) {
  // Inside `not-prose`, so the prose's link underline stays off it.
  return (
    <div className="reylo-inference-card-play not-prose">
      <a href={href}>
        <svg viewBox="0 0 10 12" aria-hidden="true">
          <path d="M1 1.2v9.6a.6.6 0 0 0 .92.5l7.4-4.8a.6.6 0 0 0 0-1L1.92.7A.6.6 0 0 0 1 1.2Z" />
        </svg>
        Open in playground
      </a>
    </div>
  );
}

/** The Params and Returns rows together: a column beside the query, or a band under it. */
export function InferenceCardTypes({ children }: { children: ReactNode }) {
  return <div className="reylo-inference-card-types">{children}</div>;
}
