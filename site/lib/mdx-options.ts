import type { applyMdxPreset } from 'fumadocs-mdx/config';

type Environment = Parameters<ReturnType<typeof applyMdxPreset>>[0];

/**
 * docs-kit's MDX options (each section's Markdown on its heading, Twoslash with its cache and the
 * `// @hover:` directive, the hanging indent of wrapped code, each distinct popup built once), with
 * Reylo's own: the modules Reylo's mapper produced for the page's `.sql` blocks, put above each
 * importing block (lib/remark-mapper.ts), the type descriptor format's cards, rules and TypeScript
 * on its own page (lib/remark-typedefs.ts), Twoslash's options (lib/twoslash.ts), the Inference
 * cards' types and the error examples' errors in each heading's section Markdown
 * (lib/inferred-markdown.ts, lib/sql-errors.ts), and the ```terminal blocks' language (lib/terminal.ts).
 */
export async function mdxOptions(environment: Environment) {
  const { docsMdxOptions } = await import('docs-kit/mdx');
  const { remarkMapper } = await import('./remark-mapper.ts');
  const { remarkTypedefs } = await import('./remark-typedefs.ts');
  const { reyloTwoslash } = await import('./twoslash.ts');
  const { rehypeInferredSections } = await import('./inferred-markdown.ts');
  const { rehypeErrorSections } = await import('./sql-errors.ts');
  const { terminal } = await import('./terminal.ts');
  const options = await docsMdxOptions(environment, { remarkPlugins: [remarkTypedefs, remarkMapper], twoslash: reyloTwoslash(), langs: ['shellscript', 'sql', terminal] });
  // After docs-kit's rehypeSections: each `<Inferred>` card's section Markdown carries its types, and
  // each error example its errors.
  return { ...options, rehypePlugins: [...(options.rehypePlugins ?? []), rehypeInferredSections, rehypeErrorSections] };
}
