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
  shield: (
    <>
      <path d="M12 22s8-3.5 8-10V5l-8-3-8 3v7c0 6.5 8 10 8 10z" />
      <path d="m9 12 2 2 4-4" />
    </>
  ),
  hash: <path d="M4 9h16M4 15h16M10 3 8 21M16 3l-2 18" />,
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

/** One feature: an icon and a name on one line, then a sentence or two. lib/markdown.ts writes it as a list item. */
export function Feature({ icon, title, children }: { icon: keyof typeof icons; title: string; children: ReactNode }) {
  return (
    <div className="reylo-feature">
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
