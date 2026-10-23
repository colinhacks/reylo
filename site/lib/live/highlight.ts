// The live card's highlighting: Shiki with the page's two themes and only the two languages the
// card shows, SQL and TypeScript, on Shiki's JavaScript regex engine, loaded with the editor when a
// reader first clicks into the card. Its tokens carry `--shiki-light` and `--shiki-dark` as the
// page's own blocks do, so the theme switch colours them the same way.
import { createHighlighterCore, type HighlighterCore } from 'shiki/core';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';

export type Token = { content: string; style: string };

let highlighter: Promise<HighlighterCore> | undefined;

export function loadHighlighter(): Promise<HighlighterCore> {
  return (highlighter ??= createHighlighterCore({
    themes: [import('shiki/themes/github-light.mjs'), import('shiki/themes/github-dark.mjs')],
    langs: [import('shiki/langs/sql.mjs'), import('shiki/langs/typescript.mjs')],
    engine: createJavaScriptRegexEngine(),
  }));
}

/** The text's lines as tokens, each with its two themes' colours as CSS variables. */
export function tokens(h: HighlighterCore, text: string, lang: 'sql' | 'ts'): Token[][] {
  const lines = h.codeToTokens(text, { lang, themes: { light: 'github-light', dark: 'github-dark' }, defaultColor: false }).tokens;
  return lines.map((line) =>
    line.map((t) => {
      const style = t.htmlStyle;
      return { content: t.content, style: typeof style === 'string' ? style : Object.entries(style ?? {}).map(([k, v]) => `${k}:${v}`).join(';') };
    }),
  );
}
