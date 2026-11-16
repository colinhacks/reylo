import type { ReactNode } from 'react';

/*
 * An editor's completion list over a code block whose language Twoslash does not run, as Rust: the
 * block is the page's, and the list opens at the end of its last line, `at` characters in, as
 * Twoslash's own `^|` list does over TypeScript. `items` is the members the editor offers, each
 * `name: type`, comma-separated; the page writes them from what the compiler checked.
 */
export function Completion({ items, at, children }: { items: string; at: string; children: ReactNode }) {
  const members = items.split(/,\s*(?![^<]*>)/).map((item) => {
    const i = item.indexOf(':');
    return { name: item.slice(0, i).trim(), type: item.slice(i + 1).trim() };
  });
  return (
    <div className="reylo-completion" style={{ ['--reylo-at' as string]: at }}>
      {children}
      <ul className="twoslash-completion-list reylo-completion-list" aria-label="Completions">
        {members.map((m) => (
          <li key={m.name}>
            <span className="reylo-completion-name">{m.name}</span>
            <span className="reylo-completion-type">{m.type}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
