import type { Plugin } from 'vite';
import { SHOW_RUST } from './languages.ts';

/** The page that is Rust's alone, which the site builds only while SHOW_RUST is on. */
export const RUST_PAGE = 'rust.mdx';

/** Whether a file of content/docs is a page the site builds. */
export const builtPage = (name: string) => name.endsWith('.mdx') && (SHOW_RUST || name !== RUST_PAGE);

/** What `<tag>…</tag>` wraps: whole lines, the tags on lines of their own, or a phrase inside a line. */
const wrapped = (tag: string) => [new RegExp(`^<${tag}>\\n[\\s\\S]*?\\n</${tag}>\\n`, 'gm'), new RegExp(`<${tag}>.*?</${tag}>`, 'g')] as const;

/** The tags alone, a tag on a line of its own with its line. */
const tags = (tag: string) => [new RegExp(`^</?${tag}>\\n`, 'gm'), new RegExp(`</?${tag}>`, 'g')] as const;

/**
 * A file of content/ as the site builds it. `<Rust>…</Rust>` wraps what the docs say only while
 * they show Rust, and `<NoRust>…</NoRust>` what they say only while they don't (lib/languages.ts),
 * whether a phrase inside a line, as in the frontmatter, or whole blocks between tags on lines of
 * their own. The one left out goes with its tags, and the other keeps its content without them, so
 * that MDX never sees either tag, and a code tab inside one joins the tabs around it. While the docs
 * leave Rust out, each `<LanguagePanel value="Rust">` goes too.
 */
export function rustContent(source: string): string {
  const [shown, hidden] = SHOW_RUST ? ['Rust', 'NoRust'] : ['NoRust', 'Rust'];
  let out = source;
  for (const pattern of wrapped(hidden)) out = out.replace(pattern, '');
  for (const pattern of tags(shown)) out = out.replace(pattern, '');
  if (!SHOW_RUST) out = out.replace(/^<LanguagePanel value="Rust">\n[\s\S]*?\n<\/LanguagePanel>\n/gm, '');
  return out;
}

/**
 * Every file of content/ through rustContent before fumadocs-mdx compiles it, which reads the code
 * Vite hands it: its own transform is ordered `pre` too, so this plugin goes first in the list.
 */
export function rustContentPlugin(): Plugin {
  return {
    name: 'reylo:rust-content',
    enforce: 'pre',
    transform: {
      order: 'pre',
      filter: { id: /\/content\/.*\.mdx(\?|$)/ },
      handler(code) {
        const out = rustContent(code);
        return out === code ? null : { code: out, map: null };
      },
    },
  };
}
