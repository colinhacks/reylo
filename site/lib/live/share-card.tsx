// The link preview of a query on /play: the query itself, highlighted, as the playground shows it, under the playground's bar. Drawn by @vercel/og's ImageResponse (Satori) at 1200 × 630, in the site's dark theme: Geist and Geist Mono (assets/fonts, SIL Open Font License) on the dark page's colours, the SQL in Shiki's GitHub Dark. The build draws /play's own card with it, of the example the page opens with (app/routes/play-og.tsx), and the share function draws one per shared query (lib/live/share-image.ts). It loads Shiki's core, the SQL grammar and one theme, and nothing of the engine or PGlite.
//
// The query is the hero: the largest size at which it fits without wrapping a line, else the largest at which it fits with its long lines wrapped (each continuation indented two columns past its line), else the smallest size, cut after the last row that fits, with an ellipsis on a row of its own. Satori lays out flex boxes only, so every element with more than one child is a flex box.
import { readFile } from 'node:fs/promises';
import type { ReactNode } from 'react';
import { createHighlighterCore, type HighlighterCore } from 'shiki/core';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';
import { wordmark } from '../../components/wordmark.tsx';
import { CARD_TEXT_BUDGET, cardSize } from './share.ts';

// The dark theme of app/global.css and lib/live/play.css.
const color = {
  page: '#0e0e0e',
  border: '#2b2b2b',
  headings: '#fafafa',
  muted: '#a1a1a1',
  gutter: '#4d4d4d',
  plain: '#e1e4e8',
  accent: '#fbc40e',
};

// Geometry, in pixels of the 1200 × 630 image.
const PAD_X = 64;
const BAR = 96;
const CODE_TOP = 44;
const CODE_BOTTOM = 48;
const LINE_HEIGHT = 1.5;
/** Geist Mono's advance, in ems. */
const ADVANCE = 0.6;
/** The sizes the query is drawn at: as large as fits, and never below the smallest, which a timeline's card still shows legibly. */
const LARGEST = 46;
const SMALLEST = 20;
/** Below this size, a query that fits only without wrapping is drawn larger with its lines wrapped. */
const UNWRAPPED_FLOOR = 22;
/** A cut query's last rows fade toward its ellipsis, by their distance from the end. */
const FADE = [1, 0.5, 0.75];

type Cell = { ch: string; color: string };
type Row = { number?: number; cells: Cell[] };
type Layout = { size: number; rows: Row[]; digits: number; cut: boolean };

let highlighter: Promise<HighlighterCore> | undefined;

function loadHighlighter(): Promise<HighlighterCore> {
  return (highlighter ??= createHighlighterCore({
    themes: [import('shiki/themes/github-dark.mjs')],
    langs: [import('shiki/langs/sql.mjs')],
    engine: createJavaScriptRegexEngine(),
  }));
}

/** East Asian wide characters and emoji take two columns of a monospace face. */
const WIDE = /[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹏＀-｠￠-￦\u{1F300}-\u{1FAFF}\u{20000}-\u{3FFFD}]/u;
const width = (ch: string) => (WIDE.test(ch) ? 2 : 1);
const cellsWidth = (cells: Cell[]) => cells.reduce((n, c) => n + width(c.ch), 0);

/** The query as the card reads it: tabs expanded, trailing space and blank edges dropped, cut to the budget. */
export function cardText(sql: string): string {
  const text = sql
    .slice(0, CARD_TEXT_BUDGET)
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .map((line) => {
      let out = '';
      for (const ch of line.trimEnd()) out += ch === '\t' ? ' '.repeat(4 - (out.length % 4)) : ch;
      return out;
    })
    .join('\n');
  return text.replace(/^\s*\n/, '').replace(/\n\s*$/, '');
}

/** Each source line's characters with their colours. */
async function highlight(text: string): Promise<Cell[][]> {
  const h = await loadHighlighter();
  const lines = h.codeToTokens(text, { lang: 'sql', theme: 'github-dark' }).tokens;
  return lines.map((line) => line.flatMap((t) => [...t.content].map((ch) => ({ ch, color: t.color ?? color.plain }))));
}

/** A line's rows at `cols` columns: the first as wide as it may be, each continuation indented past the line's own indent. */
function wrapLine(cells: Cell[], cols: number): Cell[][] {
  if (cellsWidth(cells) <= cols) return [cells];
  let indent = 0;
  while (indent < cells.length && cells[indent]!.ch === ' ') indent++;
  const hang = Math.min(indent + 2, Math.floor(cols / 2));
  const rows: Cell[][] = [];
  let rest = cells;
  let room = cols;
  while (rest.length) {
    if (cellsWidth(rest) <= room) {
      rows.push(rest);
      break;
    }
    // As many characters as the row holds, broken after the last space in its final 40% if it has one.
    let take = 0;
    let used = 0;
    while (take < rest.length && used + width(rest[take]!.ch) <= room) used += width(rest[take++]!.ch);
    let space = -1;
    for (let i = take - 1; i >= Math.floor(take * 0.6); i--) {
      if (rest[i]!.ch === ' ') {
        space = i;
        break;
      }
    }
    const end = space > 0 ? space + 1 : Math.max(take, 1);
    rows.push(rest.slice(0, end));
    rest = rest.slice(end);
    while (rest.length && rest[0]!.ch === ' ') rest = rest.slice(1);
    if (rows.length === 1) room = cols - hang;
  }
  return rows.map((row, i) => (i === 0 ? row : [...Array.from({ length: hang }, () => ({ ch: ' ', color: color.plain })), ...row]));
}

/** The code area's width and height for text of `size`, past the gutter of line numbers `digits` wide. */
function room(size: number, digits: number) {
  const gutter = digits * ADVANCE * size + gutterGap(size, digits);
  return {
    cols: Math.floor((cardSize.width - 2 * PAD_X - gutter) / (ADVANCE * size)),
    rows: Math.floor((cardSize.height - BAR - CODE_TOP - CODE_BOTTOM) / (LINE_HEIGHT * size)),
  };
}

const gutterGap = (size: number, digits: number) => (digits ? Math.round(size * 1.1) : 0);

/** The size the query is drawn at and its rows: see the top of this file. */
export function layout(lines: Cell[][]): Layout {
  // A query of one line has no line numbers: a lone 1 says nothing.
  const digits = lines.length > 1 ? String(lines.length).length : 0;
  const rowsAt = (size: number) => {
    const { cols } = room(size, digits);
    return lines.flatMap((cells, i) => wrapLine(cells, cols).map((row, j): Row => ({ number: j === 0 ? i + 1 : undefined, cells: row })));
  };
  for (let size = LARGEST; size >= UNWRAPPED_FLOOR; size--) {
    const fit = room(size, digits);
    if (lines.length <= fit.rows && lines.every((cells) => cellsWidth(cells) <= fit.cols)) return { size, rows: rowsAt(size), digits, cut: false };
  }
  for (let size = LARGEST; size >= SMALLEST; size--) {
    const rows = rowsAt(size);
    if (rows.length <= room(size, digits).rows) return { size, rows, digits, cut: false };
  }
  // Too long at the smallest size: the rows that fit, less one for the ellipsis.
  const all = rowsAt(SMALLEST);
  const keep = room(SMALLEST, digits).rows - 1;
  const rows = all.slice(0, keep);
  // The ellipsis at the indent of the row it stands for.
  const next = all[keep]!;
  let indent = 0;
  while (indent < next.cells.length && next.cells[indent]!.ch === ' ') indent++;
  rows.push({ cells: [...Array.from({ length: indent }, () => ({ ch: ' ', color: color.plain })), { ch: '…', color: color.muted }] });
  return { size: SMALLEST, rows, digits, cut: true };
}

/** Consecutive characters of one colour, as one span each. */
function runs(cells: Cell[]): { text: string; color: string }[] {
  const out: { text: string; color: string }[] = [];
  for (const { ch, color } of cells) {
    const last = out[out.length - 1];
    if (last && (last.color === color || ch === ' ')) last.text += ch;
    else out.push({ text: ch, color });
  }
  return out;
}

function Card({ layout: { size, rows, digits, cut } }: { layout: Layout }): ReactNode {
  const lineHeight = Math.round(size * LINE_HEIGHT);
  const wordmarkHeight = 30;
  return (
    <div style={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column', background: color.page, fontFamily: 'Geist' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', height: BAR, padding: `0 ${PAD_X}px`, borderBottom: `2px solid ${color.border}` }}>
        <div style={{ display: 'flex', alignItems: 'center' }}>
          <svg width={(wordmarkHeight * wordmark.width) / wordmark.height} height={wordmarkHeight} viewBox={wordmark.viewBox}>
            <path fill={color.headings} d={wordmark.mark} />
            <path fill={color.headings} transform={wordmark.wordTransform} d={wordmark.word} />
          </svg>
          <div style={{ display: 'flex', width: 2, height: 30, background: color.border, margin: '0 24px' }} />
          <div style={{ display: 'flex', fontSize: 27, fontWeight: 600, letterSpacing: -0.3, color: color.muted }}>Playground</div>
        </div>
        <div style={{ display: 'flex', fontFamily: 'Geist Mono', fontSize: 23, color: color.muted }}>reylo.dev/play</div>
      </div>
      <div style={{ display: 'flex', flexGrow: 1, flexDirection: 'column', justifyContent: 'center', padding: `${CODE_TOP}px ${PAD_X}px ${CODE_BOTTOM}px` }}>
        {rows.map((row, i) => (
          <div key={i} style={{ display: 'flex', fontFamily: 'Geist Mono', fontSize: size, lineHeight: `${lineHeight}px`, height: lineHeight, whiteSpace: 'pre', opacity: cut ? FADE[rows.length - 1 - i] ?? 1 : 1 }}>
            {digits ? <div style={{ display: 'flex', justifyContent: 'flex-end', width: digits * ADVANCE * size, marginRight: gutterGap(size, digits), color: color.gutter }}>{row.number === undefined ? '' : String(row.number)}</div> : null}
            {runs(row.cells).map((run, j) => (
              <span key={j} style={{ color: run.color }}>
                {run.text}
              </span>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

const fontCache = new Map<string, Promise<Buffer>>();

/** The card for `sql`, as a PNG response, with the fonts read from `fonts`, the site's assets/fonts. */
export async function playCard(sql: string, fonts: URL, headers?: Record<string, string>): Promise<Response> {
  const { ImageResponse } = await import('@vercel/og');
  const font = (file: string, name: string, weight: 400 | 600) => {
    let data = fontCache.get(file);
    if (!data) fontCache.set(file, (data = readFile(new URL(file, fonts))));
    return data.then((data) => ({ name, weight, style: 'normal' as const, data }));
  };
  const [lines, ...loaded] = await Promise.all([highlight(cardText(sql)), font('Geist-600.ttf', 'Geist', 600), font('GeistMono-400.ttf', 'Geist Mono', 400)]);
  return new ImageResponse(<Card layout={layout(lines)} />, { ...cardSize, fonts: loaded, headers });
}
