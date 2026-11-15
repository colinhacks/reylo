import { Variant, VariantSwitch, VariantTabs } from 'docs-kit/components';
import type { ReactNode } from 'react';
import { DIALECTS } from '@/lib/dialects';
import { LANGUAGES, type LanguageName } from '@/lib/languages';
import { DialectSwitch } from './dialect-tabs';

/*
 * The inference cards: a statement with the types Reylo infers for it, in one card of
 * labelled rows, Schema (where the example needs one), Query, Params and Returns, every row
 * visible at once, scannable without a click on a tab. Each switch sits on the line of what it
 * changes: the database on the Query row's, the language (TypeScript, Rust or the type descriptor,
 * Type Defs) on the Params and Returns band's. The page writes `<InferenceCard>` around a schema
 * block and a statement; lib/remark-mapper.ts fills it with a panel per dialect, the rows read off
 * the statement's captured module, which follow the page's dialect and language choices.
 */

/** The card. `layout` is `side` (the parameters and the result beside the query) or `rows` (under it). */
export function InferenceCard({ layout = 'side', children }: { layout?: 'side' | 'rows'; children: ReactNode }) {
  return (
    <div className="reylo-inference-card" data-layout={layout}>
      {children}
    </div>
  );
}

/** One labelled row of the card: its label, then its code. `dialects` puts the database switch on the label's line, for the Query row of a card whose statement differs by database. */
export function InferenceCardRow({ label, dialects, children }: { label: string; dialects?: string; children: ReactNode }) {
  const shown = dialects ? DIALECTS.filter((d) => dialects.split(',').includes(d)) : [];
  return (
    <div className="reylo-inference-card-row" data-row={label.toLowerCase()}>
      <div className="reylo-inference-card-label">{label}</div>
      {dialects && (
        <div className="reylo-inference-card-switch">
          <DialectSwitch dialects={shown.length < DIALECTS.length ? shown.join(',') : undefined} />
        </div>
      )}
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

/** The Params and Returns rows in each language, following the page's language choice, with the language switch on the band's first line (lib/remark-mapper.ts writes them). */
export function InferenceLanguages({ children }: { children: ReactNode }) {
  return (
    <div className="reylo-inference-languages">
      <div className="reylo-inference-card-switch">
        <VariantSwitch group="language" values={LANGUAGES} label="Language" />
      </div>
      <VariantTabs group="language" values={LANGUAGES} follow>
        {children}
      </VariantTabs>
    </div>
  );
}

/** One language's rows inside `<InferenceLanguages>`. */
export function InferenceLanguage({ value, children }: { value: LanguageName; children: ReactNode }) {
  return <Variant value={value}>{children}</Variant>;
}

/** TypeScript | Rust | Type Defs over a block whose panels follow the page's language choice, as Inference's Primitives. */
export function LanguageSwitch() {
  return (
    <div className="reylo-language-switch">
      <VariantSwitch group="language" values={LANGUAGES} label="Language" />
    </div>
  );
}
