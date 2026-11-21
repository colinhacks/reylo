import { Variant, VariantSwitch, VariantTabs } from 'docs-kit/components';
import type { ReactNode } from 'react';
import { DIALECTS, type DialectName } from '@/lib/dialects';

/*
 * One choice of dialect for the whole page: docs-kit's variant tabs with the group `dialect`, the
 * same group as Fumadocs' code tabs marked `tab-group="dialect"`, so a choice made in any of them,
 * a code-tab strip included, switches all of them and holds across reloads.
 */

/**
 * `<DialectTabs>` shows a strip Postgres | MySQL | SQLite | ClickHouse above the panel of the chosen dialect.
 * `<DialectTabs follow>` shows no strip, only the chosen dialect's panel: it follows the choice
 * made elsewhere on the page, as the TypeScript under an example's SQL does (lib/remark-mapper.ts
 * writes those).
 */
export function DialectTabs({ follow = false, dialects, children }: { follow?: boolean; dialects?: string; children: ReactNode }) {
  // `dialects` names the strip's, comma-separated, where a panel exists for fewer than all of them.
  const values = dialects ? DIALECTS.filter((d) => dialects.split(',').includes(d)) : DIALECTS;
  return (
    <VariantTabs group="dialect" values={values} follow={follow}>
      {children}
    </VariantTabs>
  );
}

/** One dialect's panel inside `<DialectTabs>`, or prose several dialects share: `value="Postgres,MySQL,SQLite"` is that panel for each of them. */
export function Dialect({ value, children }: { value: DialectName | (string & {}); children: ReactNode }) {
  return value.split(',').map((name) => (
    <Variant key={name} value={name.trim()}>
      {children}
    </Variant>
  ));
}

/**
 * Postgres | MySQL | SQLite | ClickHouse as a small switch at the right end of a code-tab strip: lib/remark-mapper.ts
 * puts one, marked `aside`, in each example whose statement differs per dialect, beside the tabs
 * `main.ts | <name>.sql`, whose panels follow the choice.
 */
export function DialectSwitch({ dialects }: { aside?: boolean; dialects?: string }) {
  // `dialects` names the group's, comma-separated, when it shows fewer than all of them.
  const values = dialects ? DIALECTS.filter((d) => dialects.split(',').includes(d)) : DIALECTS;
  // One database is no choice: a light gray label says which, as "Postgres only".
  if (values.length === 1) return <span className="reylo-dialect-only">{values[0]} only</span>;
  return <VariantSwitch group="dialect" values={values} label="Database" />;
}
