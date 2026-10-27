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
  /** What a query is analyzed against, ending the sentence that opens the dialog. */
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

  const get = `curl ${shell(api.analyze)} --get --data-urlencode ${shell(`q=${api.example}`)}`;
  const post = `curl ${shell(api.analyze)} -H 'content-type: application/json' -d ${shell(JSON.stringify({ sql: api.example }))}`;

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
        <p>The playground's analysis as JSON, for scripts and agents. A query is analyzed, never run, against {api.against}.</p>

        <h3>Analyze a query</h3>
        <p>
          <code>GET {api.analyze}?q=&lt;sql&gt;</code>
        </p>
        <CodeBlock text={get} copy />
        {answer !== '' && <CodeBlock text={answer ?? '…'} wrap={false} />}
        <dl>
          <dt>params</dt>
          <dd>The type of its parameters; absent when it takes none.</dd>
          <dt>returns</dt>
          <dd>
            What <code>.run()</code> resolves to.
          </dd>
          <dt>element</dt>
          <dd>
            One row's type: what <code>returns</code> holds one or many of, by <code>cardinality</code>.
          </dd>
          <dt>cardinality</dt>
          <dd>
            How many rows it returns: <code>many</code>, <code>one</code>, <code>maybeOne</code> or <code>none</code>.
          </dd>
          <dt>diagnostics</dt>
          <dd>
            Each error's <code>message</code> and <code>code</code>, and where it is: <code>line</code>, <code>column</code>, <code>endLine</code>, <code>endColumn</code>.
          </dd>
        </dl>
        <p>
          A POST takes the query in a JSON body. Add <code>width</code> to break the types over lines wider than that many characters.
        </p>
        <CodeBlock text={post} copy />

        <h3>List the schema</h3>
        <p>
          <code>GET {api.schema}</code>
        </p>
        <p>The tables with their columns and types, and the functions and type names the editor completes.</p>

        {api.page && (
          <>
            <h3>Share a link</h3>
            <p>
              The page's address holds the query as you type, as <code>?q=</code>. Asked for JSON, that same address answers like <code>/analyze</code>, so a link you share works for an agent too:
            </p>
            <CodeBlock text={`curl -H 'accept: application/json' ${shell(`${api.page}?q=${encodeURIComponent(api.example)}`)}`} copy />
          </>
        )}
      </div>
    </dialog>
  );
}
