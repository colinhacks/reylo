import { readFileSync } from 'node:fs';
import { createHighlighter } from 'shiki';
import type { Plugin } from 'vite';
import { formatType } from '../../src/type-format.ts';

/*
 * The popover cards of /compare: for a mark with a query behind it, the query, what the tool infers for it and what the type should be, as lib/compare-results.json holds them. Built here, once, so that the page ships highlighted text and no highlighter. `virtual:compare-cards` is the cards by `<tool>:<row>`.
 */

/** One line of the card's types: `html` is highlighted code, or `text`, a sentence, for a tool that stops on the query. */
export type CompareCard = { schema?: string; query: string; on?: string; lines: { label: string; html: string; right?: boolean; text?: boolean }[] };

/** `says`: what a tool that checks each value instead of typing the parameter lets through, as "A null passes for $1". */
type Member = { type: string; wrong?: boolean; name?: string; says?: string };
type Example = { sql: string; schema?: string; on?: string; error?: string; params: Member[]; columns: Member[]; rows?: string; reylo: { params: string; row: string; cardinality: string; rows: string } };
type Results = { tools: Record<string, { cells: Record<string, { example?: Example }> }> };
type Token = { content: string; htmlStyle?: string | Record<string, string> };
type Mark = { start: number; end: number };

const VIRTUAL = 'virtual:compare-cards';
/** How many characters of a type fit on a line of the card before it breaks, as the docs' cards break one. */
const WIDTH = 44;
const THEMES = { light: 'github-light', dark: 'github-dark' };
const TYPE_PREFIX = 'type T = ';

const escape = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const style = (t: Token) => (typeof t.htmlStyle === 'string' ? t.htmlStyle : Object.entries(t.htmlStyle ?? {}).map(([k, v]) => `${k}:${v}`).join(';'));
const span = (t: Token, text: string) => `<span style="${style(t)}">${escape(text)}</span>`;

/** The query's top-level clauses on lines of their own, as the docs' cards set a query. */
function clauses(sql: string) {
  const words = [' from ', ' left join ', ' right join ', ' full join ', ' join ', ' where ', ' group by ', ' order by ', ' union all ', ' values ', ' returning ', ' limit ', ' set '];
  let depth = 0;
  let out = '';
  for (let i = 0; i < sql.length; i++) {
    if (sql[i] === '(') depth++;
    if (sql[i] === ')') depth--;
    const word = depth === 0 ? words.find((w) => sql.startsWith(w, i)) : undefined;
    if (word) {
      out += `\n${word.trim()} `;
      i += word.length - 1;
    } else out += sql[i];
  }
  return out;
}

/** A `create table`, one column to a line. */
function statements(sql: string) {
  return sql.split('\n').map((statement) => {
    const m = /^(create table \w+ \()(.*)\);$/.exec(statement);
    if (!m) return statement;
    const columns = [''];
    let depth = 0;
    for (const c of m[2]!) {
      if (c === '(') depth++;
      if (c === ')') depth--;
      if (c === ',' && depth === 0) columns.push('');
      else columns[columns.length - 1] += c;
    }
    // A check's call on lines of its own, its arguments indented under it.
    const lines = columns.map((c) => {
      const check = /^(.*? check) \((\w+)\((.*), (\w+)\)\)$/.exec(c.trim());
      return check ? `  ${check[1]} (\n    ${check[2]}(\n      ${check[3]},\n      ${check[4]}\n    )\n  )` : `  ${c.trim()}`;
    });
    return `${m[1]}\n${lines.join(',\n')}\n);`;
  }).join('\n');
}

/** A tool's parameters or row as one object, laid out as the docs lay out a type, with the range of each wrong member. */
function objectOf(members: { text: string; wrong?: boolean }[]) {
  const multiline = `{ ${members.map((m) => m.text).join('; ')} }`.length > WIDTH;
  let text = multiline ? '{\n' : '{ ';
  const marks: Mark[] = [];
  members.forEach((m, i) => {
    text += multiline ? '  ' : i ? '; ' : '';
    if (m.wrong) marks.push({ start: text.length, end: text.length + m.text.length });
    text += m.text + (multiline ? ';\n' : '');
  });
  return { text: multiline ? `${text}}` : `${text} }`, marks };
}

const parameter = (p: Member, i: number) => ({ text: p.type.startsWith('optional ') ? `$${i + 1}?: ${p.type.slice(9)}` : `$${i + 1}: ${p.type}`, wrong: p.wrong });
/** The lab writes `$1` as `:p1` for Reylo, so its answer names the parameter `p1`; the card's query says `$1`. */
const asPlaceholders = (type: string) => type.replace(/\bp(\d+)(\??):/g, '$$$1$2:');

export async function buildCompareCards(results: Results): Promise<Record<string, CompareCard>> {
  const highlighter = await createHighlighter({ themes: Object.values(THEMES), langs: ['sql', 'ts'] });
  const tokens = (code: string, lang: 'sql' | 'ts') => highlighter.codeToTokens(code, { lang, themes: THEMES, defaultColor: false }).tokens as Token[][];
  const sql = (code: string) => tokens(code, 'sql').map((line) => line.map((t) => span(t, t.content)).join('')).join('\n');
  // Reylo's row type alone, as the tool's line is one row's type; a wrong row count has a line of its own.
  // A type, its wrong members underlined: each token is cut where a mark begins or ends.
  const type = (text: string, marks: Mark[] = []) => {
    let at = -TYPE_PREFIX.length;
    return tokens(TYPE_PREFIX + text, 'ts').map((line, n) => {
      if (n) at++;
      return line.map((t) => {
        const from = at;
        at += t.content.length;
        const cuts = [...new Set([from, at, ...marks.flatMap((m) => [m.start, m.end]).filter((c) => c > from && c < at)])].sort((a, b) => a - b);
        return cuts.slice(0, -1).map((start, i) => {
          const end = cuts[i + 1]!;
          if (end <= 0) return '';
          const piece = span(t, t.content.slice(Math.max(start, 0) - from, end - from));
          return marks.some((m) => start >= m.start && end <= m.end) ? `<span class="reylo-compare-card-wrong">${piece}</span>` : piece;
        }).join('');
      }).join('');
    }).join('\n');
  };

  const cards: Record<string, CompareCard> = {};
  for (const [tool, { cells }] of Object.entries(results.tools)) {
    for (const [row, { example: e }] of Object.entries(cells)) {
      if (!e) continue;
      const shouldBe = (label: string) => ({ label: 'Should be', right: true, html: type(formatType(label === 'Params' ? asPlaceholders(e.reylo.params) : e.reylo.row, WIDTH)) });
      const lines: CompareCard['lines'] = [];
      if (e.error) lines.push({ label: 'Returns', text: true, html: escape(e.error.replace(/\.$/, '')) }, shouldBe('Returns'));
      for (const [label, members] of [['Params', e.params.map(parameter)], ['Returns', e.columns.map((c) => ({ text: `${c.name}: ${c.type}`, wrong: c.wrong }))]] as const) {
        if (!members.some((m) => m.wrong)) continue;
        const { text, marks } = objectOf([...members]);
        const says = label === 'Params' ? e.params.flatMap((p) => (p.wrong && p.says ? [p.says] : [])) : [];
        lines.push(says.length ? { label, text: true, html: escape(says.join('. ')) } : { label, html: type(text, marks) }, shouldBe(label));
      }
      if (e.rows) lines.push({ label: 'Rows', html: escape(e.rows) }, { label: 'Should be', right: true, html: escape(e.reylo.rows) });
      cards[`${tool}:${row}`] = { ...(e.schema ? { schema: sql(statements(e.schema)) } : {}), query: sql(clauses(e.sql)), ...(e.on ? { on: e.on } : {}), lines };
    }
  }
  highlighter.dispose();
  return cards;
}

/** The Vite plugin behind `virtual:compare-cards`. */
export function compareCards(): Plugin {
  const file = `${import.meta.dirname}/compare-results.json`;
  return {
    name: 'reylo-compare-cards',
    resolveId: (id) => (id === VIRTUAL ? `\0${VIRTUAL}` : undefined),
    async load(id) {
      if (id !== `\0${VIRTUAL}`) return;
      this.addWatchFile(file);
      return `export default ${JSON.stringify(await buildCompareCards(JSON.parse(readFileSync(file, 'utf8'))))};`;
    },
  };
}
