'use client';
import { Popup, PopupContent, PopupTrigger } from 'fumadocs-twoslash/ui';
import { type CSSProperties, Fragment, type ReactNode } from 'react';
import { compareCell, compareResults, compareRows, compareTools, type CompareCell } from '@/lib/compare';
import cards from 'virtual:compare-cards';
import type { CompareCard } from '@/lib/compare-cards';
import { wordmark } from './wordmark';

/** A note's `code` as code. */
function Text({ children }: { children: string }) {
  return children.split(/(`[^`]+`)/).map((part, i) => (part.startsWith('`') ? <code key={i}>{part.slice(1, -1)}</code> : <Fragment key={i}>{part}</Fragment>));
}

const ICONS: Record<CompareCell['state'], ReactNode> = {
  y: <svg className="reylo-compare-y" viewBox="4.4 4.4 11.2 11.2" aria-hidden="true"><path d="M5.6 10.4l2.9 2.9 5.9-6.4" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" /></svg>,
  n: <svg className="reylo-compare-n" viewBox="4.4 4.4 11.2 11.2" aria-hidden="true"><path d="M7 7l6 6M13 7l-6 6" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></svg>,
  p: <svg className="reylo-compare-p" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="5.6" fill="none" stroke="currentColor" strokeWidth="1.5" /><path d="M8 2.4a5.6 5.6 0 0 1 0 11.2z" fill="currentColor" /></svg>,
};
const WORDS = { y: 'Yes', p: 'Partly', n: 'No' };

const html = (text: string) => ({ __html: text });

/** A query a tool got wrong, in the labelled rows of the docs' Inference cards, with what it infers and what the type should be. Its code is highlighted at build time (lib/compare-cards.ts). */
function Card({ card }: { card: CompareCard }) {
  return (
    <div className="reylo-compare-card">
      {card.schema ? (
        <div className="reylo-compare-card-schema">
          <div className="reylo-compare-card-label">Schema</div>
          <pre dangerouslySetInnerHTML={html(card.schema)} />
        </div>
      ) : null}
      <div className="reylo-compare-card-query">
        <div className="reylo-compare-card-head">
          <div className="reylo-compare-card-label">Query</div>
          {card.on ? <span className="reylo-compare-card-on">{card.on}</span> : null}
        </div>
        <pre dangerouslySetInnerHTML={html(card.query)} />
      </div>
      <div className="reylo-compare-card-types">
        {card.lines.map((line, i) => (
          <Fragment key={i}>
            <div className="reylo-compare-card-label">{line.label}</div>
            {line.right ? ICONS.y : ICONS.n}
            <code className={line.text ? 'reylo-compare-card-error' : undefined} dangerouslySetInnerHTML={html(line.html)} />
          </Fragment>
        ))}
      </div>
    </div>
  );
}

/** A cross or half circle: hovered, or tapped on a phone, the docs' popup shows a query the tool got wrong, or why it has the mark. */
function Mark({ tool, row, label, cell }: { tool: string; row: string; label: string; cell: CompareCell }) {
  if (cell.state === 'y') return <span className="reylo-compare-mark" role="img" aria-label={WORDS.y}>{ICONS.y}</span>;
  const card = cards[`${tool}:${row}`];
  const name = tool === 'reylo' ? 'Reylo' : tool === 'postgrestypedsql' ? 'Postgres Typed SQL' : compareResults.tools[tool]?.name ?? tool;
  return (
    <Popup delay={60}>
      <PopupTrigger className="reylo-compare-mark" aria-label={`${name}, ${label}: ${WORDS[cell.state]}`}>{ICONS[cell.state]}</PopupTrigger>
      <PopupContent side="top" sideOffset={6} className={card ? 'reylo-compare-pop reylo-compare-pop-card' : 'reylo-compare-pop'}>
        {card ? <Card card={card} /> : <p><Text>{cell.note ?? ''}</Text></p>}
      </PopupContent>
    </Popup>
  );
}

/** A row's label: hovered, it says what the row asks of a tool. */
function Label({ label, asks }: { label: string; asks?: string }) {
  if (!asks) return <>{label}</>;
  return (
    <Popup delay={60}>
      <PopupTrigger className="reylo-compare-label">{label}</PopupTrigger>
      <PopupContent side="top" align="start" sideOffset={6} className="reylo-compare-pop"><p><Text>{asks}</Text></p></PopupContent>
    </Popup>
  );
}

/**
 * The comparison on /compare: Reylo's column first, inside the saber frame, then each tool's marks,
 * from lib/compare-results.json. Each row's rule runs unbroken from edge to edge, under the frame.
 */
export function CompareTable() {
  const column = (i: number) => 5 + i;
  let row = 2;
  return (
    <div className="reylo-compare">
      <div className="reylo-compare-scroll">
        <div className="reylo-compare-grid not-prose" style={{ '--reylo-tools': compareTools.length } as CSSProperties}>
          <div className="reylo-compare-corner" style={{ gridRow: 1, gridColumn: 1 }} />
          <div className="reylo-compare-head" style={{ gridRow: 1, gridColumn: 3 }}>
            <span className="reylo-compare-name reylo-compare-brand">
              <svg viewBox={wordmark.viewBox} role="img" aria-label="Reylo"><path className="reylo-compare-brand-mark" d={wordmark.mark} /><path transform={wordmark.wordTransform} d={wordmark.word} /></svg>
            </span>
            <span className="reylo-compare-sub">TypeScript</span>
          </div>
          {compareTools.map((t, i) => (
            <div key={t.id} className="reylo-compare-head" style={{ gridRow: 1, gridColumn: column(i) }}>
              <a className="reylo-compare-name" href={t.href}>{t.name}</a>
              <span className="reylo-compare-sub">{t.sub}</span>
            </div>
          ))}
          {compareRows.map((g) => {
            const group = row++;
            return (
              <Fragment key={g.group}>
                <div className="reylo-compare-group" style={{ gridRow: group }}><span>{g.group}</span></div>
                {g.rows.map((r) => {
                  const at = row++;
                  return (
                    <Fragment key={r.id}>
                      <div className="reylo-compare-rule" style={{ gridRow: at }} />
                      <div className="reylo-compare-row-label" style={{ gridRow: at, gridColumn: 1 }}><Label label={r.label} asks={r.asks} /></div>
                      <div className="reylo-compare-cell" style={{ gridRow: at, gridColumn: 3 }}><Mark tool="reylo" row={r.id} label={r.label} cell={compareCell('reylo', r.id)} /></div>
                      {compareTools.map((t, i) => (
                        <div key={t.id} className="reylo-compare-cell" style={{ gridRow: at, gridColumn: column(i) }}>
                          <Mark tool={t.id} row={r.id} label={r.label} cell={compareCell(t.id, r.id)} />
                        </div>
                      ))}
                    </Fragment>
                  );
                })}
              </Fragment>
            );
          })}
          <div className="reylo-compare-frame" style={{ gridRow: `1 / ${row}`, gridColumn: 3 }} />
          <div className="reylo-compare-stick" style={{ gridRow: `1 / ${row}`, gridColumn: 1 }} />
        </div>
      </div>
    </div>
  );
}
