import type { ReactNode } from 'react';

const icons: Record<string, ReactNode> = {
  file: (
    <>
      <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7z" />
      <path d="M14 2v5a1 1 0 0 0 1 1h5" />
      <path d="M10 13h4M8 17h8" />
    </>
  ),
  branch: (
    <>
      <circle cx="6" cy="5" r="2.5" />
      <circle cx="6" cy="19" r="2.5" />
      <circle cx="18" cy="7" r="2.5" />
      <path d="M6 7.5v9" />
      <path d="M18 9.5c0 4-5 4.5-9.5 7" />
    </>
  ),
  database: (
    <>
      <ellipse cx="12" cy="5" rx="8" ry="3" />
      <path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5" />
      <path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3" />
    </>
  ),
  hash: <path d="M4 9h16M4 15h16M10 3 8 21M16 3l-2 18" />,
  zap: <path d="M13 2 4 14h7l-1 8 9-12h-7z" />,
  split: (
    <>
      <path d="M16 3h5v5" />
      <path d="M8 3H3v5" />
      <path d="M12 22v-8.3a4 4 0 0 0-1.172-2.872L3 3" />
      <path d="m15 9 6-6" />
    </>
  ),
  layers: (
    <>
      <path d="m12 2 9 5-9 5-9-5z" />
      <path d="m3 12 9 5 9-5" />
      <path d="m3 17 9 5 9-5" />
    </>
  ),
};

/** The cards that list a page's features under its title: two across, one on a phone. */
export function Features({ children }: { children: ReactNode }) {
  return <div className="reylo-features">{children}</div>;
}

/** A name with `code` in backticks, as the card shows it. */
function named(text: string): ReactNode {
  return text.split('`').map((part, k) => (k % 2 ? <code key={k}>{part}</code> : part));
}

/** A feature that is a choice between two ways: a card two columns wide, its name over the two ways, an "or" on the seam between them. lib/markdown.ts writes it as a list item. */
export function Choice({ icon, title, children }: { icon: keyof typeof icons; title: string; children: ReactNode }) {
  return (
    <div className="reylo-choice">
      <div className="reylo-feature-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          {icons[icon]}
        </svg>
        {title}
      </div>
      <div className="reylo-choice-ways">{children}</div>
    </div>
  );
}

/** One way of a choice: its name (`code` in backticks), and a sentence or two. The second way, `or`, carries the "or" on the seam. */
export function Way({ title, or, children }: { title: string; or?: boolean; children: ReactNode }) {
  return (
    <div className="reylo-choice-way">
      {or ? (
        <span className="reylo-choice-or" aria-hidden="true">
          or
        </span>
      ) : null}
      <div className="reylo-choice-way-title">{named(title)}</div>
      {children}
    </div>
  );
}

/** One feature: an icon and a name on one line, then a sentence or two; `wide` spans both columns. lib/markdown.ts writes it as a list item. */
export function Feature({ icon, title, wide, children }: { icon: keyof typeof icons; title: string; wide?: boolean; children: ReactNode }) {
  return (
    <div className={wide ? 'reylo-feature reylo-feature-wide' : 'reylo-feature'}>
      <div className="reylo-feature-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          {icons[icon]}
        </svg>
        {title}
      </div>
      {children}
    </div>
  );
}
