'use client';

import { useState, type ComponentPropsWithoutRef, type ReactNode } from 'react';
import { CheckIcon, CopyIcon } from './copy-icons';

type Props = ComponentPropsWithoutRef<'h2'> & { as: 'h2' | 'h3' | 'h4'; 'data-section'?: string };

const ICON = { viewBox: '0 0 24 24', width: 16, height: 16, fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round', 'aria-hidden': true } as const;

const linkIcon = (
  <svg {...ICON}>
    <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
    <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
  </svg>
);

/** A button that copies `text()` to the clipboard and shows a check for a moment. */
function CopyButton({ className, label, icon, text }: { className: string; label: string; icon: ReactNode; text: () => string }) {
  const [copied, setCopied] = useState(false);
  const copy = () =>
    void navigator.clipboard.writeText(text()).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  return (
    <button type="button" className={`${className} not-prose`} aria-label={label} title={label} onClick={copy}>
      {copied ? <CheckIcon /> : icon}
    </button>
  );
}

/**
 * A section heading. A link icon sits in the margin to its left, in a light gray, shown while the
 * heading is hovered or the icon has keyboard focus, and always on a device without hover (a
 * phone): clicking it copies the section's URL and puts its anchor in the address bar. An h2 and an h3
 * also carry, at the right end of the line, a button that copies the section as Markdown
 * (`remarkSections` in ../mdx/remark-sections.ts), to paste into an agent. The heading text is
 * plain text, not a link, so it carries none of the link underline.
 */
export function Heading({ as: As, id, children, className, 'data-section': section, ...props }: Props) {
  if (!id) return <As className={className} {...props}>{children}</As>;
  const url = () => {
    const url = new URL(window.location.href);
    url.hash = id;
    history.replaceState(null, '', url);
    return url.href;
  };
  return (
    <As id={id} className={['dk-heading', className].filter(Boolean).join(' ')} {...props}>
      <CopyButton className="dk-anchor" label="Copy a link to this section" icon={linkIcon} text={url} />
      <span className="dk-heading-text">{children}</span>
      {section && <CopyButton className="dk-copy-section" label="Copy this section as Markdown" icon={<CopyIcon />} text={() => section} />}
    </As>
  );
}
