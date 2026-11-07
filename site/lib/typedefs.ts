// The `<TypeDef>` cards of the type descriptor format's page (content/docs/typedefs.mdx). A card is a
// schema and a statement on one dialect, whose statement descriptor `nub run capture-typedefs` reads
// with `reylo analyze` into lib/typedefs.json, or an example of spec/examples.ts, by its name, for a
// database that needs more than `reylo analyze` to run. lib/remark-typedefs.ts renders each card from what this file
// reads of it: its rows, Types (the card's `types`, the database types of its kind), Schema, Query,
// Descriptor (the part of the descriptor `show` names, commented by the card's `notes`) and Holds
// (the card's `holds`, values its type admits).
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import type { MdNode } from './remark-mapper.ts';

export const TYPEDEF = 'TypeDef';
export type CardDialect = 'postgres' | 'mysql' | 'sqlite';
const DIALECTS: CardDialect[] = ['postgres', 'mysql', 'sqlite'];

export type Card = {
  node: MdNode;
  line: number;
  /** A card the capture reads: its dialect, its schema (none for a statement that reads no table) and its statement. */
  statement?: { dialect: CardDialect; schema?: string; query: string };
  /** A card shown from spec/examples.ts: the example's name, whose descriptor `show` reads. */
  example?: string;
  /** The name `<TypeDefPart of="…">` shows a part of this card's descriptor by. */
  id?: string;
  /** The statement is one the database refuses, to show its diagnostics: a card without this fails the capture on any. */
  errors: boolean;
  /** The part of the descriptor shown ([`shown`]): the whole of it when absent. */
  show?: string;
  /** A comment for a key of what is shown, by its path from there (`type.precision`). */
  notes: Record<string, string>;
  /** Values the type admits, each as SQL writes it. */
  holds: string[];
  /** The database types of this kind, by database: each a type's name, or a phrase ("a composite type"). */
  types: Record<string, string[]>;
};

const attr = (node: MdNode, name: string) => node.attributes?.find((a) => a.type === 'mdxJsxAttribute' && a.name === name)?.value;

/** An attribute written as an expression of JSON (`notes={{ "type.bits": "…" }}`), or as a string. */
function json<T>(node: MdNode, name: string, empty: T): T {
  const value = attr(node, name);
  if (value === undefined || value === null) return empty;
  const source = typeof value === 'string' ? value : String((value as { value?: unknown }).value);
  try {
    return JSON.parse(source) as T;
  } catch {
    throw new Error(`line ${node.position?.start.line ?? '?'}: <${TYPEDEF} ${name}={…}> is no JSON: ${source}`);
  }
}

/** Every `<TypeDef>` card of a page's tree. */
export function cards(tree: MdNode): Card[] {
  const out: Card[] = [];
  const walk = (node: MdNode) => {
    for (const child of node.children ?? []) {
      if (child.type === 'mdxJsxFlowElement' && child.name === TYPEDEF) out.push(card(child));
      else walk(child);
    }
  };
  walk(tree);
  return out;
}

function card(node: MdNode): Card {
  const line = node.position?.start.line ?? 0;
  const where = `line ${line}: a <${TYPEDEF}> card`;
  const code = (node.children ?? []).filter((c) => c.type === 'code' && (c.lang ?? 'sql') === 'sql');
  const schema = code.find((c) => /(?:^|\s)schema(?:\s|$)/.test(c.meta ?? ''));
  const queries = code.filter((c) => c !== schema);
  const example = attr(node, 'example');
  const dialect = (attr(node, 'dialect') ?? 'postgres') as CardDialect;
  if (queries.length > 1 || (typeof example !== 'string' && queries.length !== 1)) throw new Error(`${where} needs one sql block, its statement (and a sql schema block where the statement reads a table), or an example="…" from spec/examples.ts`);
  if (!DIALECTS.includes(dialect)) throw new Error(`${where} names dialect="${dialect}"; write postgres, mysql or sqlite, or show an example of another`);
  const show = attr(node, 'show');
  return {
    node,
    line,
    // An example's schema and statement are shown, and only a statement of no example is analyzed.
    ...(typeof example !== 'string' && { statement: { dialect, ...(schema && { schema: schema.value }), query: queries[0]!.value! } }),
    ...(typeof example === 'string' && { example }),
    ...(typeof show === 'string' && { show }),
    ...(typeof attr(node, 'id') === 'string' && { id: attr(node, 'id') as string }),
    errors: (node.attributes ?? []).some((a) => a.type === 'mdxJsxAttribute' && a.name === 'errors'),
    notes: json(node, 'notes', {}),
    holds: json(node, 'holds', []),
    types: json(node, 'types', {}),
  };
}

/** The key of a statement's descriptor in lib/typedefs.json. */
export const keyOf = (s: NonNullable<Card['statement']>) => createHash('sha256').update(JSON.stringify([s.dialect, s.schema ?? '', s.query])).digest('hex').slice(0, 16);

export type Captures = Record<string, { dialect: CardDialect; query: string; descriptor: unknown }>;
export const CAPTURES = 'typedefs.json';
export const readCaptures = (site = process.cwd()): Captures => JSON.parse(readFileSync(path.join(site, 'lib', CAPTURES), 'utf8'));

/** The part of a descriptor `show` names: a value at a path of keys and indexes, under its key where it is not an object with keys of its own to show (`sql`, `params`, `result.columns`). */
export function shown(descriptor: unknown, show: string | undefined): unknown {
  if (!show) return descriptor;
  const keys = show.split('.');
  const at = (v: unknown, k: string) => (Array.isArray(v) ? v[Number(k)] : (v as Record<string, unknown> | undefined)?.[k]);
  const value = keys.reduce(at, descriptor);
  if (value === undefined) throw new Error(`show="${show}" names nothing in the descriptor`);
  const last = keys[keys.length - 1]!;
  return keys.length === 1 || Array.isArray(value) || typeof value !== 'object' ? { [last]: value } : value;
}

const compact = (v: unknown): string => {
  if (Array.isArray(v)) return `[${v.map(compact).join(', ')}]`;
  if (v && typeof v === 'object') {
    const entries = Object.entries(v);
    return entries.length ? `{ ${entries.map(([k, w]) => `${JSON.stringify(k)}: ${compact(w)}`).join(', ')} }` : '{}';
  }
  return JSON.stringify(v);
};

/**
 * JSON with comments: every key on a line of its own where a note sits on it or under it, or where
 * its value would not fit on one line, and the top level always; each note beside its line where it
 * fits in `width`, and above it where it doesn't. A list breaks over lines where it would not fit.
 */
export function annotate(value: unknown, notes: Record<string, string>, width = 74): string {
  const lines: [string, string | undefined][] = [];
  const under = (p: string) => Object.keys(notes).some((k) => k.startsWith(`${p}.`));
  const write = (v: unknown, indent: number, p: string, prefix: string, comma: string, note: string | undefined) => {
    const pad = ' '.repeat(indent);
    const one = compact(v);
    const fits = indent + prefix.length + one.length + comma.length <= width;
    if (v && typeof v === 'object' && !Array.isArray(v) && Object.keys(v).length && (indent === 0 || !fits || under(p))) {
      if (note) lines.push([`${pad}// ${note}`, undefined]);
      lines.push([`${pad}${prefix}{`, undefined]);
      const entries = Object.entries(v);
      entries.forEach(([k, w], i) => {
        const q = p ? `${p}.${k}` : k;
        write(w, indent + 2, q, `${JSON.stringify(k)}: `, i < entries.length - 1 ? ',' : '', notes[q]);
      });
      lines.push([`${pad}}${comma}`, undefined]);
    } else if (Array.isArray(v) && v.length && (!fits || v.some((_, i) => under(`${p}.${i}`)))) {
      if (note) lines.push([`${pad}// ${note}`, undefined]);
      lines.push([`${pad}${prefix}[`, undefined]);
      v.forEach((item, i) => write(item, indent + 2, `${p}.${i}`, '', i < v.length - 1 ? ',' : '', notes[`${p}.${i}`]));
      lines.push([`${pad}]${comma}`, undefined]);
    } else lines.push([`${pad}${prefix}${one}${comma}`, note]);
  };
  write(value, 0, '', '', '', undefined);
  const beside = (l: string, n: string) => l.length + 4 + n.length <= width + 2;
  const column = Math.max(0, ...lines.filter(([l, n]) => n && beside(l, n)).map(([l]) => l.length)) + 2;
  return lines
    .map(([l, n]) => {
      if (!n) return l;
      if (beside(l, n)) return `${l.padEnd(column)}// ${n}`;
      return `${' '.repeat(l.length - l.trimStart().length)}// ${n}\n${l}`;
    })
    .join('\n');
}
