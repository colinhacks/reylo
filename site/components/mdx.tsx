import { AgentPrompt, docsMdxComponents } from 'docs-kit/components';
import type { MDXComponents } from 'mdx/types';
import type { ReactNode } from 'react';
import { CompareTable } from './compare-table';
import { Dialect, DialectSwitch, DialectTabs } from './dialect-tabs';
import { HeroActions, HeroDemo } from './hero';
import { HowItWorksDiagram } from './how-it-works-diagram';
import { InferenceCard, InferenceCardPlay, InferenceCardRow, InferenceCardTypes } from './inference-card';
import { Pairs, Rules } from './value-pairs';

/**
 * docs-kit's components (headings with the link icon and the copy-as-Markdown button, accordions,
 * tabs, `<Details>`, code tabs with the file name in the strip, Twoslash's popups) and Reylo's own.
 */
export function getMDXComponents(components?: MDXComponents) {
  return docsMdxComponents({
    // The setup instructions for a coding agent at the top of Setup.
    AgentPrompt: ({ children }: { children: ReactNode }) => <AgentPrompt title="Set up Reylo with an agent">{children}</AgentPrompt>,
    // The figure under How it works (components/how-it-works-diagram.tsx).
    HowItWorksDiagram,
    // The badges and "Copy agent prompt" under the main page's subheader (components/hero.tsx).
    HeroActions,
    // The looping editor recording under them (components/hero.tsx).
    HeroDemo,
    // One dialect choice for the page: <DialectTabs> with a strip, <DialectTabs follow> without,
    // each holding <Dialect value="Postgres|MySQL|SQLite"> panels, in step with the code tabs
    // marked tab-group="dialect" (components/dialect-tabs.tsx).
    DialectTabs,
    Dialect,
    // The switch at the right end of an example's strip (lib/remark-mapper.ts writes it).
    DialectSwitch,
    // The inference cards: a statement and the types Reylo infers for it, in labelled
    // rows (components/inference-card.tsx; lib/remark-mapper.ts fills them).
    InferenceCard,
    InferenceCardRow,
    InferenceCardTypes,
    InferenceCardPlay,
    // The marks on /compare (components/compare-table.tsx).
    CompareTable,
    // A type descriptor format card, the same card's rows: Types, Schema, Query, Descriptor and Holds
    // (lib/remark-typedefs.ts fills them).
    TypeDefCard: ({ children }: { children: ReactNode }) => (
      <div className="typedef-card">
        <InferenceCard layout="rows" bare>
          {children}
        </InferenceCard>
      </div>
    ),
    // A rule list whose rules show SQL values and what `.run()` returns for each, the dialect
    // pages' (components/value-pairs.tsx).
    Rules,
    Pairs,
    ...components,
  });
}

export const useMDXComponents = getMDXComponents;

declare global {
  type MDXProvidedComponents = ReturnType<typeof getMDXComponents>;
}
