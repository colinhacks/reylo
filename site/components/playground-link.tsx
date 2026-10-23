/**
 * A one-line card that links the playground (/play): it opens the page and leaves the reading where
 * it was. A tag alone on its line, which the page's Markdown leaves out.
 */
export function PlaygroundLink() {
  // Inside `not-prose`, so the prose's link underline stays off the card.
  return (
    <div className="not-prose">
      <a href="/play" className="reylo-play-link">
        <span className="reylo-play-link-title">Playground</span>
        <span className="reylo-play-link-text">Edit a query against the page's schema and see what Reylo infers, live.</span>
        <span className="reylo-play-link-arrow" aria-hidden="true">
          →
        </span>
      </a>
    </div>
  );
}
