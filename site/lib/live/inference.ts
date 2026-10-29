// The playground's examples as text: the main page's Inference subsections, each by its heading and its first Postgres statement, and each further Postgres statement that names its own example with a `play="<title>"` meta attribute. A plain function of the section's source, so that the build (lib/live/play-data.ts) and the share functions (lib/live/share-examples.ts) read the same examples.

/** The example /play opens with: nested JSON, a nullable object and a cardinality. */
export const START = 'nested-fetching';

export type InferenceStatement = { title: string; slug: string; sql: string };

/**
 * The Inference section's subsections (content/sections/03-inference.mdx), each by its heading and its first Postgres statement (one written for every dialect, or for Postgres), then each statement in it marked `play="<title>"`, an example of its own under that title.
 */
export function inferenceStatements(source: string): InferenceStatement[] {
  const out: InferenceStatement[] = [];
  for (const part of source.split(/\n### /).slice(1)) {
    const heading = part.slice(0, part.indexOf('\n'));
    const title = heading.replace(/\s*\[#[^\]]*\]\s*$/, '').replace(/`/g, '').trim();
    const anchor = /\[#([^\]]+)\]/.exec(heading)?.[1];
    const slug = anchor ?? slugOf(title);
    const blocks = [...part.matchAll(/```sql ([^\n]*)\n([\s\S]*?)```/g)].filter(([, meta]) => /file="/.test(meta!) && /dialect="(?:postgres|all)"/.test(meta!));
    const own = (meta: string) => /(?:^|\s)play="([^"]*)"/.exec(meta)?.[1];
    const block = blocks.find(([, meta]) => own(meta!) === undefined);
    if (block) out.push({ title, slug, sql: block[2]!.replace(/\n$/, '') });
    for (const [, meta, sql] of blocks) {
      const name = own(meta!);
      if (name !== undefined) out.push({ title: name, slug: slugOf(name), sql: sql!.replace(/\n$/, '') });
    }
  }
  return out;
}

const slugOf = (title: string) => title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
