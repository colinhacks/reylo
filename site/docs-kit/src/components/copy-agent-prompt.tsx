'use client';

import { useEffect, useRef, useState } from 'react';
import { AGENT_PROMPT_SELECTOR, agentPromptText } from './agent-prompt';
import { CheckIcon } from './copy-icons';

/** Lucide's sparkles, the button's mark: the prompt is for a coding agent. */
function SparklesIcon() {
  return (
    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z" />
      <path d="M20 3v4" />
      <path d="M22 5h-4" />
      <path d="M4 17v2" />
      <path d="M5 18H3" />
    </svg>
  );
}

/**
 * "Copy agent prompt" at the top of the page, on the title's row (`titleAside` of `DocsShell`): the
 * call to action at the very top of the page, a plain button. It copies what the page's
 * agent card (`<AgentPrompt>`) copies, found in the page by its `data-agent-prompt`, so the prompt
 * is written once, in the card's code block. A page without a card shows no button.
 */
export function CopyAgentPrompt({ label = 'Copy agent prompt' }: { label?: string }) {
  const [missing, setMissing] = useState(false);
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Rendered on the server, so a page with a card shows the button from the first paint; one
  // without a card drops it once it has hydrated.
  useEffect(() => {
    setMissing(!document.querySelector(AGENT_PROMPT_SELECTOR));
    return () => clearTimeout(timer.current);
  }, []);

  async function copy() {
    const card = document.querySelector(AGENT_PROMPT_SELECTOR);
    if (!card) return;
    await navigator.clipboard.writeText(agentPromptText(card));
    setCopied(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopied(false), 1600);
  }

  if (missing) return null;
  // Both labels share one grid cell, the other one hidden, as on the card's Copy button, so
  // "Copied" keeps the button's width and moves nothing.
  return (
    <button type="button" className="dk-cta" onClick={copy}>
      <span data-shown={!copied || undefined}>
        <SparklesIcon />
        {label}
      </span>
      <span data-shown={copied || undefined} aria-live="polite">
        <CheckIcon />
        Copied
      </span>
    </button>
  );
}
