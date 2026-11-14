// The link preview reylo.dev shows on X, Slack, Discord, LinkedIn and the rest, written to
// build/client/opengraph-image.png at build time: the page links it as og:image, and as X's image
// too, with a hash of this file's text in its URL (app/routes/home.tsx). The card is drawn here
// rather than in a module this file imports, so that a change to the card changes the URL and the
// caches of X and Slack fetch the new card.
//
// The wordmark, the subheader's claim, and the opening example with the type of `author` in a
// popup, as the page shows it on hover. Drawn by @vercel/og's ImageResponse (Satori), in the site's
// dark theme: Geist and Geist Mono (assets/fonts, SIL Open Font License) on the dark page's
// colours, with the code in the page's GitHub Dark colours. Satori lays out flex boxes only, so
// every element with more than one child is a flex box. A card drawn entirely as a code block, with
// the type on five lines at the code's size, replaced this one for an hour on 2026-10-20, and
// this one came back.
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { CSSProperties, ReactNode } from 'react';
import { wordmark } from '@/components/wordmark';

import { ogSize } from '@/lib/og';

// The dark theme of app/global.css, and Shiki's GitHub Dark for the code.
const color = {
  page: '#0e0e0e',
  surface: '#161616',
  popup: '#1b1b1b',
  border: '#2b2b2b',
  headings: '#fafafa',
  muted: '#a1a1a1',
  yellow: '#ffd84a',
  pill: 'rgba(255, 216, 74, 0.14)',
  keyword: '#f97583',
  string: '#9ecbff',
  function: '#b392f0',
  constant: '#79b8ff',
  plain: '#e1e4e8',
  property: '#ffab70',
};

type Token = [text: string, color: keyof typeof color, hovered?: true];

const example: Token[][] = [
  [['import', 'keyword'], [' getAuthor ', 'plain'], ['from', 'keyword'], [' "./get_author.sql"', 'string'], [';', 'plain']],
  [['const', 'keyword'], [' ', 'plain'], ['author', 'constant', true], [' = ', 'keyword'], ['await', 'keyword'], [' getAuthor.', 'plain'], ['run', 'function'], ['(db, { id: ', 'plain'], ['1', 'constant'], [' });', 'plain']],
];

// The type the page's popup shows for `author` (site/lib/sql-modules.json), on one line.
const authorType: Token[][] = [
  [['const', 'keyword'], [' author', 'constant'], [': { ', 'plain'], ['id', 'property'], [': ', 'plain'], ['number', 'constant'], ['; ', 'plain'], ['name', 'property'], [': ', 'plain'], ['string', 'constant'], ['; ', 'plain'], ['post_count', 'property'], [': ', 'plain'], ['number', 'constant'], [' } | ', 'plain'], ['null', 'constant']],
];

// Sizes, in pixels of the 1200 × 630 image.
const PAD = 64;
const CODE = 27;
const POPUP = 22;
const HEADLINE = 66;

function Code({ lines, size }: { lines: Token[][]; size: number }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', fontFamily: 'Geist Mono', fontSize: size, lineHeight: 1.6 }}>
      {lines.map((line, i) => (
        <div key={i} style={{ display: 'flex', whiteSpace: 'pre' }}>
          {line.map(([text, ink, hovered], j) => (
            <span key={j} style={{ color: color[ink], ...(hovered ? { borderBottom: `3px solid ${color.yellow}` } : {}) }}>
              {text}
            </span>
          ))}
        </div>
      ))}
    </div>
  );
}

/** A headline line, with `.sql` set as the page sets inline code: mono, on a yellow tint. */
function HeadlineLine({ text }: { text: string }) {
  const [before, after] = text.split('.sql');
  const pill: CSSProperties = { fontFamily: 'Geist Mono', fontWeight: 500, fontSize: HEADLINE * 0.9, background: color.pill, borderRadius: HEADLINE * 0.16, padding: `${HEADLINE * 0.02}px ${HEADLINE * 0.14}px` };
  return (
    <div style={{ display: 'flex', alignItems: 'center', whiteSpace: 'pre' }}>
      <span>{before}</span>
      {after !== undefined ? <span style={pill}>.sql</span> : null}
      {after !== undefined ? <span>{after}</span> : null}
    </div>
  );
}

function Card(): ReactNode {
  const codeLeft = 28;
  const codeTop = 22;
  return (
    <div style={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column', padding: `60px ${PAD}px 0`, background: color.page, fontFamily: 'Geist' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <svg width={(44 * wordmark.width) / wordmark.height} height={44} viewBox={wordmark.viewBox}>
          <path fill={color.headings} d={wordmark.markOnDark} />
          <path fill={color.headings} transform={wordmark.wordTransform} d={wordmark.word} />
        </svg>
        <div style={{ display: 'flex', fontFamily: 'Geist Mono', fontSize: 23, color: color.muted }}>reylo.dev</div>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', marginTop: 62, fontSize: HEADLINE, fontWeight: 600, letterSpacing: -HEADLINE * 0.025, lineHeight: 1.12, color: color.headings }}>
        <HeadlineLine text="Import .sql files into TypeScript," />
        <HeadlineLine text="fully typed" />
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', position: 'relative', marginTop: 52 }}>
        <div style={{ display: 'flex', padding: `${codeTop}px ${codeLeft}px`, background: color.surface, border: `1.5px solid ${color.border}`, borderRadius: 16 }}>
          <Code lines={example} size={CODE} />
        </div>
        {/* Under `author`, which starts six characters into its line, as the page's popup opens. */}
        <div
          style={{
            position: 'absolute',
            left: codeLeft + CODE * 0.6 * 6 - 22,
            top: codeTop * 2 + CODE * 1.6 * 2 + 6,
            display: 'flex',
            padding: '12px 22px',
            background: color.popup,
            border: `1.5px solid ${color.border}`,
            borderRadius: 12,
            boxShadow: '0 12px 40px rgba(0, 0, 0, 0.45)',
          }}
        >
          <Code lines={authorType} size={POPUP} />
        </div>
      </div>
    </div>
  );
}

const font = async (file: string, name: string, weight: 400 | 500 | 600) => ({
  name,
  weight,
  style: 'normal' as const,
  data: await readFile(join(process.cwd(), 'assets/fonts', file)),
});

/** The preview as a PNG, built once, with the site. Its description is the page's og:image:alt (lib/og.ts). */
export async function loader() {
  const { ImageResponse } = await import('@vercel/og');
  const fonts = await Promise.all([font('Geist-600.ttf', 'Geist', 600), font('GeistMono-400.ttf', 'Geist Mono', 400), font('GeistMono-500.ttf', 'Geist Mono', 500)]);
  return new ImageResponse(<Card />, { ...ogSize, fonts });
}
