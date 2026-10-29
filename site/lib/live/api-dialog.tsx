// The playground's API, explained in a dialog the bar's "API" link opens (lib/live/app.tsx): each
// endpoint, a request to copy, and the answer to that request, from the page's build where it has it, else fetched from the API when the dialog
// opens, so what it shows is what the API answers. The docs' /play and `reylo play` pass their own
// addresses.
import { useEffect, useRef, useState } from 'react';
import { CodeCopyButton } from '../../docs-kit/src/components/code-copy.tsx';
import { answerJson, apiMarkdown, apiSections, type ApiDocs } from './api-docs.ts';

export type { ApiDocs };


/** A code block with docs-kit's copy icon, as every code block of the docs has; it copies the block's `<pre>`. */
function CodeBlock({ text, copy, wrap = true }: { text: string; copy?: boolean; wrap?: boolean }) {
  return (
    <figure className="play-api-code">
      <pre data-nowrap={wrap ? undefined : true}>{text}</pre>
      {copy && <CodeCopyButton />}
    </figure>
  );
}

/** The head's button: the whole dialog as Markdown, for an agent's prompt or a note. */
function CopyAll({ text }: { text: () => string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      className="play-api-copy-all"
      onClick={() => {
        void navigator.clipboard.writeText(text()).then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 1200);
        });
      }}
    >
      {copied ? 'Copied' : 'Copy all'}
    </button>
  );
}


export function ApiDialog({ api, open, onClose }: { api: ApiDocs; open: boolean; onClose(): void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [answer, setAnswer] = useState<string | undefined>(api.answer);
  const pressedOutside = useRef(false);

  useEffect(() => {
    const d = dialog.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);

  // The example's answer, asked of the API itself, without the fields that echo the request or time it.
  useEffect(() => {
    if (!open || answer !== undefined) return;
    fetch(`${api.analyze}?q=${encodeURIComponent(api.example)}`)
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error(r.statusText))))
      .then((result) => setAnswer(answerJson(result)))
      .catch(() => setAnswer(''));
  }, [open, answer, api]);

  // The endpoints, drawn below and copied whole as Markdown by the head's button (lib/live/api-docs.ts).
  const sections = apiSections(api, answer === '' ? undefined : (answer ?? '…'));
  const markdown = () => apiMarkdown(api, answer || undefined);

  return (
    <dialog
      ref={dialog}
      className="play-api"
      aria-labelledby="play-api-title"
      onClose={onClose}
      onPointerDown={(e) => (pressedOutside.current = e.target === e.currentTarget)}
      onClick={(e) => {
        // A press and its release both on the backdrop, outside the dialog's box, close it; a text
        // selection dragged from inside out to the backdrop does not.
        if (pressedOutside.current && e.target === e.currentTarget) onClose();
      }}
    >
      <div className="play-api-head">
        <h2 id="play-api-title">API</h2>
        <span className="play-grow" />
        <CopyAll text={markdown} />
        <button type="button" className="play-x" aria-label="Close" onClick={onClose}>
          <svg viewBox="0 0 24 24" width={18} height={18} fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinecap="round" aria-hidden="true">
            <path d="M18 6 6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div className="play-api-body">
        {sections.map((section) => (
          <section key={section.title}>
            <h3>{section.title}</h3>
            <p>{section.text}</p>
            <CodeBlock text={section.request} copy />
            {section.answer && <CodeBlock text={section.answer} wrap={false} />}
          </section>
        ))}
      </div>
    </dialog>
  );
}
