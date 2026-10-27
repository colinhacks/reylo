// The playground's API, explained in a dialog the bar's "API" link opens (lib/live/app.tsx): each
// endpoint, a request to copy, and the answer to that request, fetched from the API when the dialog
// opens, so what it shows is what the API answers. The docs' /play and `reylo play` pass their own
// addresses.
import { useEffect, useRef, useState } from 'react';
import { CodeCopyButton } from '../../docs-kit/src/components/code-copy.tsx';

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

  // The endpoints, drawn below and copied whole as Markdown by the head's button.
  const sections: { title: string; text: string; request: string; answer?: string }[] = [
    ...(api.page ? [{ title: `GET ${path(api.page)}?q=<sql>`, text: "The playground's own link, asked for JSON, answers with the query's types, so a link you share works for an agent too.", request: `curl -H 'accept: application/json' ${shell(api.page + q)}` }] : []),
    {
      title: `GET ${path(api.analyze)}?q=<sql>`,
      text: `A query's params, return type, row type, cardinality and errors, analyzed against ${api.against} and never run.`,
      request: `curl ${shell(api.analyze + q)}`,
      ...(answer !== '' ? { answer: answer ?? '…' } : {}),
    },
    { title: `GET ${path(api.schema)}`, text: 'The tables with their columns and types, and the functions and type names the editor completes.', request: `curl ${shell(api.schema)}` },
  ];
  const markdown = () =>
    `# ${new URL(api.analyze).origin.replace(/^https?:\/\//, '')} playground API\n\n` +
    sections.map((x) => [`## ${x.title}`, x.text, '```sh\n' + x.request + '\n```', ...(x.answer && x.answer !== '…' ? ['```json\n' + x.answer + '\n```'] : [])].join('\n\n')).join('\n\n') +
    '\n';

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
