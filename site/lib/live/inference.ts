// The playground's examples as text: the main page's Inference subsections, each by its heading and its first Postgres statement. A plain function of the section's source, so that the build (lib/live/play-data.ts) and the share functions (lib/live/share-examples.ts) read the same examples.

/** The example /play opens with: nested JSON, a nullable object and a cardinality. */
export const START = 'nested-fetching';

export type InferenceStatement = { title: string; slug: string; sql: string };

/**
 * The Inference section's subsections (content/sections/03-inference.mdx), each by its heading and its first Postgres statement (one written for every dialect, or for Postgres).
 */
export function inferenceStatements(source: string): InferenceStatement[] {
  const out: InferenceStatement[] = [];
  for (const part of source.split(/\n### /).slice(1)) {
    const heading = part.slice(0, part.indexOf('\n'));
    const title = heading.replace(/\s*\[#[^\]]*\]\s*$/, '').replace(/`/g, '').trim();
    const anchor = /\[#([^\]]+)\]/.exec(heading)?.[1];
    const slug = anchor ?? title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
    const blocks = [...part.matchAll(/```sql ([^\n]*)\n([\s\S]*?)```/g)];
    const block = blocks.find(([, meta]) => /file="/.test(meta!) && /dialect="(?:postgres|all)"/.test(meta!));
    if (block) out.push({ title, slug, sql: block[2]!.replace(/\n$/, '') });
  }
  return out;
}
