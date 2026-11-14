import { AgentPrompt, docsMdxComponents } from 'docs-kit/components';
import type { MDXComponents } from 'mdx/types';
import type { ReactNode } from 'react';
import { CompareTable } from './compare-table';
import { Dialect, DialectSwitch, DialectTabs } from './dialect-tabs';
import { Choice, Feature, Features, Way } from './feature-cards';
import { Step, Steps } from './steps';
import { HeroActions, HeroDemo } from './hero';
import { HowItWorksDiagram } from './how-it-works-diagram';
import { MigrationsDiagram } from './migrations-diagram';
import { Inferred, InferredRow, InferredTypes } from './inferred';
import { PlaygroundLink } from './playground-link';
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
    // The records and commands of Reylo Migrate, at the top of the Migrations page (components/migrations-diagram.tsx).
    MigrationsDiagram,
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
    // The Inference section's cards: a statement and the types Reylo infers for it, in labelled
    // rows (components/inferred.tsx; lib/remark-mapper.ts fills them).
    Inferred,
    InferredRow,
    InferredTypes,
    // The cards under /migrations' title (components/feature-cards.tsx).
    Features,
    Feature,
    Choice,
    Way,
    // The vertical stepper on /migrations (components/steps.tsx).
    Steps,
    Step,
    // The marks on /compare (components/compare-table.tsx).
    CompareTable,
    // A type descriptor format card, the same card's rows: Types, Schema, Query, Descriptor and Holds
    // (lib/remark-typedefs.ts fills them).
    TypeDefCard: ({ children }: { children: ReactNode }) => (
      <div className="typedef-card">
        <Inferred layout="rows" bare>
          {children}
        </Inferred>
      </div>
    ),
    // The card that links the playground, /play (components/playground-link.tsx).
    PlaygroundLink,
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
