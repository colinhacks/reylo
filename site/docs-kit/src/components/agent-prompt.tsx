'use client';

import { useState, type ReactNode } from 'react';
import { Callout } from './callout';

/** The selector of an agent card (its `data-agent-prompt`), by which `<CopyAgentPrompt>` finds the card it copies. */
export const AGENT_PROMPT_SELECTOR = '[data-agent-prompt]';

/**
 * The prompt an agent card holds, as plain text: its code block's lines, as Shiki writes them (one
 * `.line` element each). The card's Copy button and `<CopyAgentPrompt>` at the top of the page both
 * copy this, so the prompt is written once, in the page's code block.
 */
export function agentPromptText(card: Element): string {
  const lines = [...card.querySelectorAll('pre .line')].map((line) => line.textContent ?? '');
  return lines.join('\n').trimEnd() + '\n';
}

/**
 * The instructions a coding agent follows to set the product up in a project: a complete,
 * copyable instruction block that is visually obvious. A standard callout (./callout.tsx) titled for the product, a note,
 * then the prompt, an ordinary code block, copied with the code block's own copy icon or with
 * `<CopyAgentPrompt>` at the top of the page; the code block shows six lines and scrolls (style.css).
 * It was a card of its own until 2026-10-21. The wrapper carries `data-agent-prompt`, by which `<CopyAgentPrompt>` finds it.
 * A footer under the prompt holds the card's own "Copy instructions", in the accent, at its right.
 */
export function AgentPrompt({
  title,
  note = 'Paste these instructions into Claude Code, Codex, Cursor or any coding agent.',
  children,
}: {
  title: string;
  note?: string;
  children: ReactNode;
}) {
  return (
    <div className="dk-agent" aria-label="Setup instructions for a coding agent" data-agent-prompt="">
      <Callout title={title}>
        <p>{note}</p>
        {children}
        <AgentPromptCopy />
      </Callout>
    </div>
  );
}

/** The card's footer: "Copy instructions", which copies the prompt above it and says so for a moment. */
function AgentPromptCopy() {
  const [copied, setCopied] = useState(false);
  return (
    <div className="dk-agent-foot">
      <button
        type="button"
        className="dk-agent-copy"
        onClick={(e) => {
          const card = e.currentTarget.closest(AGENT_PROMPT_SELECTOR);
          if (!card) return;
          void navigator.clipboard.writeText(agentPromptText(card));
          setCopied(true);
          setTimeout(() => setCopied(false), 1400);
        }}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          {copied ? (
            <path d="M3 8.5l3.2 3.2L13 4.8" />
          ) : (
            <>
              <rect x="5.25" y="5.25" width="8.5" height="8.5" rx="1.75" />
              <path d="M10.75 5.25v-1.5a1.5 1.5 0 0 0-1.5-1.5h-5.5a1.5 1.5 0 0 0-1.5 1.5v5.5a1.5 1.5 0 0 0 1.5 1.5h1.5" />
            </>
          )}
        </svg>
        {copied ? 'Copied' : 'Copy instructions'}
      </button>
    </div>
  );
}
