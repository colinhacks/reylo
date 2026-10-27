// The playground's API, explained in a dialog the bar's "API" link opens (lib/live/app.tsx): each
// endpoint, a request to copy, and the answer to that request, fetched from the API when the dialog
// opens, so what it shows is what the API answers. The docs' /play and `reylo play` pass their own
// addresses.
import { useEffect, useRef, useState } from 'react';

export type ApiDocs = {
  /** The analyze endpoint's address. */
  analyze: string;
  /** The schema endpoint's address. */
  schema: string;
  /** The page's own address, where it answers `?q=` with JSON when asked for JSON. */
  page?: string;
  /** What a query is analyzed against: "the page's sample schema". */
  against: string;
  /** The query of the examples. */
  example: string;
};

function CodeBlock({ text, copy, wrap = true }: { text: string; copy?: boolean; wrap?: boolean }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="play-api-code">
      <pre data-nowrap={wrap ? undefined : true}>{text}</pre>
      {copy && (
        <button
          type="button"
          className="play-api-copy"
          onClick={() => {
            void navigator.clipboard.writeText(text).then(() => {
              setCopied(true);
              setTimeout(() => setCopied(false), 1200);
            });
          }}
        >
          {copied ? 'Copied' : 'Copy'}
        </button>
      )}
    </div>
  );
}

const shell = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;

export function ApiDialog({ api, open, onClose }: { api: ApiDocs; open: boolean; onClose(): void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [answer, setAnswer] = useState<string>();
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
      .then(({ sql: _sql, ms: _ms, ...rest }) => setAnswer(JSON.stringify(rest, null, 2)))
      .catch(() => setAnswer(''));
  }, [open, answer, api]);

  const path = (url: string) => new URL(url).pathname;
  // Encoded as a browser writes a form, a space as `+`, which reads more easily than `%20`.
  const q = `?${new URLSearchParams({ q: api.example })}`;

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
        <button type="button" className="play-x" aria-label="Close" onClick={onClose}>
          <svg viewBox="0 0 24 24" width={18} height={18} fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinecap="round" aria-hidden="true">
            <path d="M18 6 6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div className="play-api-body">
        {api.page && (
          <section>
            <h3>
              GET {path(api.page)}?q=&lt;sql&gt;
            </h3>
            <p>The playground's own link, asked for JSON, answers with the query's types, so a link you share works for an agent too.</p>
            <CodeBlock text={`curl -H 'accept: application/json' ${shell(api.page + q)}`} copy />
          </section>
        )}
        <section>
          <h3>
            GET {path(api.analyze)}?q=&lt;sql&gt;
          </h3>
          <p>A query's params, return type, row type, cardinality and errors, analyzed against {api.against} and never run.</p>
          <CodeBlock text={`curl ${shell(api.analyze + q)}`} copy />
          {answer !== '' && <CodeBlock text={answer ?? '…'} wrap={false} />}
        </section>
        <section>
          <h3>GET {path(api.schema)}</h3>
          <p>The tables with their columns and types, and the functions and type names the editor completes.</p>
          <CodeBlock text={`curl ${shell(api.schema)}`} copy />
        </section>
      </div>
    </dialog>
  );
}
