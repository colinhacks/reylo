import type { RehypeCodeOptions } from 'fumadocs-core/mdx-plugins';
import type { DefaultMDXOptions, applyMdxPreset } from 'fumadocs-mdx/config';
import type { DocsTwoslashOptions } from './twoslash.ts';

type Environment = Parameters<ReturnType<typeof applyMdxPreset>>[0];
type Langs = NonNullable<RehypeCodeOptions['langs']>;
/** A Shiki theme: a bundled theme's name, or a theme object. */
type Theme = NonNullable<Extract<RehypeCodeOptions, { themes: unknown }>['themes'][string]>;
/** A list of remark plugins, as Fumadocs' preset takes them. */
type RemarkPlugins = Extract<DefaultMDXOptions['remarkPlugins'], unknown[]>;

export type DocsMdxOptions = {
  /** The site's remark plugins, run after `remarkSections` and `remarkQueryBlocks`. */
  remarkPlugins?: RemarkPlugins;
  /** Twoslash on `ts twoslash` blocks (./twoslash.ts); `false` turns it off. */
  twoslash?: DocsTwoslashOptions | false;
  /**
   * The languages to load up front, besides `js`, `jsx`, `ts` and `tsx`: Shiki cannot load a
   * language lazily inside a Twoslash popup.
   */
  langs?: Langs;
  /**
   * Shiki's light and dark themes, in place of Fumadocs' `github-light` and `github-dark`, by name
   * or as theme objects: a site's own code colours. Twoslash's popups take them too.
   */
  themes?: { light: Theme; dark: Theme };
  /** Shiki's colour replacements, per theme: a site's own palette for the GitHub themes. */
  colorReplacements?: RehypeCodeOptions['colorReplacements'];
};

/**
 * The MDX options of a docs page: Fumadocs' defaults plus each section's Markdown on its heading,
 * written again after Twoslash with what each query shows (./remark-sections.ts), the site's remark
 * plugins, Twoslash with its cache and the `@hover`
 * directive (./twoslash.ts), a block's errors under their lines (./diagnostics.ts), the hanging
 * indent of wrapped code lines (./hanging-indent.ts), and each distinct Twoslash popup built once
 * (./recma-shared-popups.ts).
 *
 * Call it from a `mdxOptions` of `fumadocs-mdx/macro`'s `defineDocs`, through a dynamic import, so
 * the app never bundles it or TypeScript:
 *
 *   mdxOptions: async (environment) => (await import('./mdx-options')).mdxOptions(environment)
 */
export async function docsMdxOptions(environment: Environment, options: DocsMdxOptions = {}) {
  const { applyMdxPreset } = await import('fumadocs-mdx/config');
  const { rehypeCodeDefaultOptions } = await import('fumadocs-core/mdx-plugins');
  const { rehypeSections, remarkQueryBlocks, remarkSections } = await import('./remark-sections.ts');
  const { recmaSharedPopups } = await import('./recma-shared-popups.ts');
  const { hangingIndent } = await import('./hanging-indent.ts');
  const { diagnosticsTransformer } = await import('./diagnostics.ts');
  const langs: Langs = ['js', 'jsx', 'ts', 'tsx'];
  for (const lang of options.langs ?? []) if (!langs.includes(lang)) langs.push(lang);
  const twoslash = options.twoslash === false ? [] : [(await import('./twoslash.ts')).docsTwoslash(options.twoslash)];
  return applyMdxPreset({
    remarkPlugins: [remarkSections, ...(twoslash.length ? [remarkQueryBlocks] : []), ...(options.remarkPlugins ?? [])],
    rehypePlugins: [rehypeSections],
    recmaPlugins: [recmaSharedPopups],
    rehypeCodeOptions: {
      ...rehypeCodeDefaultOptions,
      transformers: [...(rehypeCodeDefaultOptions.transformers ?? []), diagnosticsTransformer, ...twoslash, hangingIndent],
      langs,
      ...(options.themes && { themes: options.themes }),
      ...(options.colorReplacements && { colorReplacements: options.colorReplacements }),
    },
  })(environment);
}
