import type { ReactNode } from 'react';

/** A vertical stepper: each step numbered, joined to the next by a line. */
export function Steps({ children }: { children: ReactNode }) {
  return <ol className="reylo-steps">{children}</ol>;
}

/** One step: its name, where it acts, and a sentence or two. lib/markdown.ts writes it as a numbered list item. */
export function Step({ title, where, children }: { title: string; where?: string; children: ReactNode }) {
  return (
    <li className="reylo-step" data-shadow={where?.includes('shadow') ? '' : undefined}>
      <div className="reylo-step-head">
        <span className="reylo-step-title">{title}</span>
        {where ? <span className="reylo-step-where">{where}</span> : null}
      </div>
      <div className="reylo-step-body">{children}</div>
    </li>
  );
}
