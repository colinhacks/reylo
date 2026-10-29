// What the playground is, in a dialog the bar's "About" link opens (lib/live/app.tsx), drawn as the API dialog is (lib/live/api-dialog.tsx). The docs' /play and `reylo play` say where the query is analyzed, which is theirs alone.
import { useEffect, useRef, type ReactNode } from 'react';

export type About = {
  /** Where each query is analyzed, and what leaves the machine: the paragraph only this page can say. */
  runs: ReactNode;
};

export function AboutDialog({ about, open, onClose }: { about: About; open: boolean; onClose(): void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const pressedOutside = useRef(false);

  useEffect(() => {
    const d = dialog.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);

  return (
    <dialog
      ref={dialog}
      className="play-api play-about"
      aria-labelledby="play-about-title"
      onClose={onClose}
      onPointerDown={(e) => (pressedOutside.current = e.target === e.currentTarget)}
      onClick={(e) => {
        // A press and its release both on the backdrop close it, as the API dialog does.
        if (pressedOutside.current && e.target === e.currentTarget) onClose();
      }}
    >
      <div className="play-api-head">
        <h2 id="play-about-title">About</h2>
        <span className="play-grow" />
        <button type="button" className="play-x" aria-label="Close" onClick={onClose}>
          <svg viewBox="0 0 24 24" width={18} height={18} fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinecap="round" aria-hidden="true">
            <path d="M18 6 6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div className="play-api-body">
        <section>
          <h3>Reylo</h3>
          <p>Reylo is a general-purpose SQL type inference engine. Give it a query and a database, and it infers the query's parameters, its columns and their nullability, nested JSON shapes and cardinality, from the database's own analysis, with nothing declared by hand.</p>
          <p>It is language-agnostic: TypeScript is one rendering of what it infers, JSON Schema another, and its API answers any language.</p>
        </section>
        <section>
          <h3>This playground</h3>
          <p>Edit the query, and the Params and Returns Reylo infers update as you type, with each error underlined where it is.</p>
          <p>{about.runs}</p>
        </section>
        <section>
          <h3>Next</h3>
          <p>
            Use it from TypeScript with the <a href="https://reylo.dev/#setup">setup guide</a>, or from any language, script or agent through <a href="https://reylo.dev/play/api.md">the API</a>. <a href="https://reylo.dev">The docs</a> cover the rest.
          </p>
        </section>
      </div>
    </dialog>
  );
}
