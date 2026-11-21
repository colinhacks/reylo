import { Variant, VariantSwitch, VariantTabs } from 'docs-kit/components';
import type { ReactNode } from 'react';
import { DIALECTS, type DialectName } from '@/lib/dialects';

/*
 * One choice of dialect for the whole page: docs-kit's variant tabs with the group `dialect`, the
 * same group as Fumadocs' code tabs marked `tab-group="dialect"`, so a choice made in any of them,
 * a code-tab strip included, switches all of them and holds across reloads.
 */

/**
 * `<DialectTabs>` shows a strip Postgres | MySQL | SQLite above the panel of the chosen dialect.
 * `<DialectTabs follow>` shows no strip, only the chosen dialect's panel: it follows the choice
 * made elsewhere on the page, as the TypeScript under an example's SQL does (lib/remark-mapper.ts
 * writes those).
 */
export function DialectTabs({ follow = false, children }: { follow?: boolean; children: ReactNode }) {
  return (
    <VariantTabs group="dialect" values={DIALECTS} follow={follow}>
      {children}
    </VariantTabs>
  );
}

/** One dialect's panel inside `<DialectTabs>`. */
export function Dialect({ value, children }: { value: DialectName; children: ReactNode }) {
  return <Variant value={value}>{children}</Variant>;
}

/**
 * Postgres | MySQL | SQLite as a small switch at the right end of a code-tab strip: lib/remark-mapper.ts
 * puts one, marked `aside`, in each example whose statement differs per dialect, beside the tabs
 * `main.ts | <name>.sql`, whose panels follow the choice.
 */
export function DialectSwitch({ dialects }: { aside?: boolean; dialects?: string }) {
  // `dialects` names the group's, comma-separated, when it shows fewer than three.
  const values = dialects ? DIALECTS.filter((d) => dialects.split(',').includes(d)) : DIALECTS;
  // One database is no choice: a light gray label says which, as "Postgres only".
  if (values.length === 1) return <span className="reylo-dialect-only">{values[0]} only</span>;
  return <VariantSwitch group="dialect" values={values} label="Database" />;
}
