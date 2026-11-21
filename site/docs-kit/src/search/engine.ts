// The search itself: what the dialog (../components/search.tsx) runs in the browser over the index
// a site builds (./index.ts). No server, and nothing here touches the page, so the ranking can be
// tried from Node.
//
// A docs site is small (tens of pages) and its readers type what a heading says, so the order is a
// reader's: a page whose title is the query comes first, then a section whose heading is, then
// text. A word matches the word typed, the same word in another form (`key`, `keys`), a word it
// begins (`instal`, as the reader types), a compound written apart or together (`automerge`,
// `auto-merge`), and, when nothing else does, a word one slip of the keys away (`bedrok`).

/** A heading's section: the text under it, up to the next heading. The first section of a page has no heading. */
export type SearchSection = {
  /** The heading's anchor, without the `#`. */
  id?: string;
  /** The heading as it reads, its code in backticks. */
  title?: string;
  depth?: number;
  /** Paragraphs, list items and table rows, in order, each as it reads, its code in backticks. */
  text: string[];
  /** Code blocks, in order. */
  code?: string[];
};

export type SearchPage = {
  /** The page's address on the site, without the site's base path: `/`, `/install`. */
  url: string;
  title: string;
  description?: string;
  sections: SearchSection[];
};

export type SearchIndex = { version: 1; pages: SearchPage[] };

type Range = [start: number, end: number];

/** Text as a result shows it: the parts that are code, and the parts that matched the query. */
export type Marked = { text: string; code: Range[]; marks: Range[] };

/** A section of a page that matched: its heading, the heading it sits under, and the text that matched or opens it. */
export type SectionResult = { href: string; trail?: string; title: Marked; excerpt?: Marked };

/** A page that matched, or that holds sections that did. */
export type PageResult = {
  href: string;
  title: Marked;
  excerpt?: Marked;
  /** Its sections that matched, best first. */
  sections: SectionResult[];
  /** Whether the page's best match is its first section here, not the page itself. */
  sectionLeads: boolean;
};

type Kind = 'title' | 'heading' | 'description' | 'text' | 'code';
type Token = { word: string; start: number; end: number; joined?: boolean };
type Unit = {
  page: number;
  section: number;
  kind: Kind;
  text: string;
  code: Range[];
  /** Its words in order, each between spaces, for a query of several words found side by side. */
  phrase: string;
  words: number;
  /** Where each word is in the text, worked out when a result first shows the unit. */
  tokens?: Token[];
};
type Section = { page: number; id?: string; depth: number; heading?: number; parent?: number; units: number[] };

export type Prepared = {
  index: SearchIndex;
  units: Unit[];
  sections: Section[];
  /** Each page's sections, as indexes into `sections`; the first is the page's own (its title, description and opening text). */
  pages: number[][];
  postings: Map<string, number[]>;
  stems: Map<string, string[]>;
  vocabulary: string[];
};

const WEIGHT: Record<Kind, number> = { title: 10, heading: 7, description: 3.5, text: 2.2, code: 1.2 };

/** Words a question is asked with, which a section need not hold when the query has others. */
const STOP = new Set('a an and are as at be by can do does for from how i in is it my of on or the to what when where which why with'.split(' '));

/**
 * A string in lower case without its accents, character for character, so a place in the result is
 * the same place in the text.
 */
function fold(text: string): string {
  // Nearly every text is plain ASCII, which lower case leaves the same length.
  if (!/[^\x00-\x7f]/.test(text)) return text.toLowerCase();
  let out = '';
  for (let i = 0; i < text.length; i++) {
    const char = text[i]!;
    if (char < '\u0080') {
      out += char.toLowerCase();
      continue;
    }
    const base = char.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();
    out += base.length === 1 ? base : char;
  }
  return out;
}

/** The characters that join the parts of one name: `auto-merge`, `PULLFROG_MODEL`, `pullfrog.yml`. */
const JOINER = new Set(['-', '_', '.']);

/** The most parts of a name that are also read as one word: `re-review` as `rereview`. */
const JOINED = 4;

/**
 * A text's words, and each run of words joined into one name also as the name written together, so
 * `automerge` finds `auto-merge`. Each with its place in the text, to mark what a query matched.
 */
function tokenize(text: string): { tokens: Token[]; phrase: string; words: number } {
  const folded = fold(text);
  const raw: Token[] = [];
  let start = -1;
  for (let i = 0; i <= folded.length; i++) {
    const code = i < folded.length ? folded.charCodeAt(i) : 0;
    const inWord = (code > 96 && code < 123) || (code > 47 && code < 58);
    if (inWord && start === -1) start = i;
    else if (!inWord && start !== -1) {
      raw.push({ word: folded.slice(start, i), start, end: i });
      start = -1;
    }
  }
  const tokens = raw.filter((token) => token.word.length > 1);
  let phrase = ' ';
  for (let i = 0; i < raw.length; i++) {
    phrase += `${raw[i]!.word} `;
    let word = raw[i]!.word;
    for (let j = i + 1; j < raw.length && j < i + JOINED && raw[j]!.start === raw[j - 1]!.end + 1 && JOINER.has(folded[raw[j - 1]!.end]!); j++) {
      word += raw[j]!.word;
      tokens.push({ word, start: raw[i]!.start, end: raw[j]!.end, joined: true });
    }
  }
  return { tokens, phrase, words: raw.length };
}

/** A name and the words around it: a run of words with one joiner between each two. */
const NAMES = /[a-z0-9]+(?:[-_.][a-z0-9]+)*/g;

/**
 * The same words as `tokenize` finds, without their places: what the index is built from, when the
 * index arrives, where the browser's own matcher does the reading. `add` gets each word to index.
 */
function words(text: string, add: (word: string) => void): { phrase: string; words: number } {
  const names = fold(text).match(NAMES);
  if (!names) return { phrase: ' ', words: 0 };
  const all: string[] = [];
  for (const name of names) {
    if (!/[-_.]/.test(name)) {
      all.push(name);
      if (name.length > 1) add(name);
      continue;
    }
    const parts = name.split(/[-_.]/);
    for (let i = 0; i < parts.length; i++) {
      all.push(parts[i]!);
      if (parts[i]!.length > 1) add(parts[i]!);
      let word = parts[i]!;
      for (let j = i + 1; j < parts.length && j < i + JOINED; j++) add((word += parts[j]!));
    }
  }
  return { phrase: ` ${all.join(' ')} `, words: all.length };
}

/** A unit's words with their places. */
const tokensOf = (unit: Unit): Token[] => (unit.tokens ??= tokenize(unit.text).tokens);

/** A word's stem, roughly: enough that `review`, `reviews`, `reviewed` and `reviewing` share one. */
function stem(word: string): string {
  let out = word;
  if (out.length > 4 && out.endsWith('ies')) out = `${out.slice(0, -3)}y`;
  else if (out.length > 4 && /(?:ss|ch|sh|x|z)es$/.test(out)) out = out.slice(0, -2);
  else if (out.length > 3 && out.endsWith('s') && !/(?:ss|us|is)$/.test(out)) out = out.slice(0, -1);
  if (out.length > 5 && out.endsWith('ing')) out = out.slice(0, -3);
  else if (out.length > 4 && out.endsWith('ed')) out = out.slice(0, -2);
  if (out.length > 3 && out.at(-1) === out.at(-2) && !/[aeiou]$/.test(out)) out = out.slice(0, -1);
  return out.length > 3 && out.endsWith('e') ? out.slice(0, -1) : out;
}

/** Text with its backticks taken out, and where the code they marked now is. */
function unmark(source: string): { text: string; code: Range[] } {
  let text = '';
  const code: Range[] = [];
  source.split('`').forEach((part, i) => {
    if (i % 2 === 1 && part) code.push([text.length, text.length + part.length]);
    text += part;
  });
  return { text, code };
}

/** The index as the search reads it. Built once, when the index arrives. */
export function prepare(index: SearchIndex): Prepared {
  const units: Unit[] = [];
  const sections: Section[] = [];
  const pages: number[][] = [];
  const postings = new Map<string, number[]>();
  const add = (page: number, section: number, kind: Kind, source: string): number => {
    const { text, code } = kind === 'code' ? { text: source, code: [[0, source.length] as Range] } : unmark(source);
    const id = units.length;
    const read = words(text, (word) => {
      const list = postings.get(word);
      if (!list) postings.set(word, [id]);
      else if (list[list.length - 1] !== id) list.push(id);
    });
    units.push({ page, section, kind, text, code, ...read });
    sections[section]!.units.push(id);
    return id;
  };
  index.pages.forEach((page, p) => {
    const own: number[] = [];
    // The headings a section sits under, by depth, to find its parent.
    const open: number[] = [];
    page.sections.forEach((entry, i) => {
      const at = sections.length;
      const depth = entry.depth ?? 1;
      sections.push({ page: p, id: entry.id, depth, units: [] });
      own.push(at);
      if (i === 0) {
        sections[at]!.heading = add(p, at, 'title', page.title);
        if (page.description) add(p, at, 'description', page.description);
      } else {
        while (open.length && sections[open.at(-1)!]!.depth >= depth) open.pop();
        sections[at]!.parent = open.at(-1);
        open.push(at);
        sections[at]!.heading = add(p, at, 'heading', entry.title ?? '');
      }
      for (const text of entry.text) add(p, at, 'text', text);
      for (const code of entry.code ?? []) add(p, at, 'code', code);
    });
    pages.push(own);
  });

  const vocabulary = [...postings.keys()].sort();
  const stems = new Map<string, string[]>();
  for (const word of vocabulary) {
    const key = stem(word);
    const list = stems.get(key);
    if (list) list.push(word);
    else stems.set(key, [word]);
  }
  return { index, units, sections, pages, postings, stems, vocabulary };
}

/** Whether two words are one slip of the keys apart, or two for `limit` 2: a letter wrong, missing, added, or two swapped. */
function near(a: string, b: string, limit: number): number {
  if (Math.abs(a.length - b.length) > limit) return limit + 1;
  let previous: number[] = [];
  let current = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    const before = previous;
    previous = current;
    current = [i];
    let best = i;
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      let value = Math.min(previous[j]! + 1, current[j - 1]! + 1, previous[j - 1]! + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) value = Math.min(value, before[j - 2]! + 1);
      current.push(value);
      if (value < best) best = value;
    }
    if (best > limit) return limit + 1;
  }
  return current[b.length]!;
}

type Match = { quality: number; prefix?: number };

/** The words of the index a typed word matches, each with how well. `last` is the word still being typed. */
function candidates(prepared: Prepared, term: string, last: boolean): Map<string, Match> {
  const found = new Map<string, Match>();
  const offer = (word: string, match: Match) => {
    if ((found.get(word)?.quality ?? 0) < match.quality) found.set(word, match);
  };
  if (prepared.postings.has(term)) offer(term, { quality: 1 });
  for (const word of prepared.stems.get(stem(term)) ?? []) offer(word, { quality: 0.85 });
  // The word still being typed is the start of a word; a word typed whole is, only when it is no word itself.
  if (last ? term.length > 1 : found.size === 0 && term.length > 2) {
    // The first word of the sorted vocabulary that is not before the term.
    let low = 0;
    let high = prepared.vocabulary.length;
    while (low < high) {
      const middle = (low + high) >> 1;
      if (prepared.vocabulary[middle]! < term) low = middle + 1;
      else high = middle;
    }
    for (let i = low; i < prepared.vocabulary.length && prepared.vocabulary[i]!.startsWith(term); i++) {
      const word = prepared.vocabulary[i]!;
      if (word !== term) offer(word, { quality: (last ? 0.8 : 0.6) * (0.75 + (0.25 * term.length) / word.length), prefix: term.length });
    }
  }
  if (found.size === 0 && term.length > 3) {
    const limit = term.length > 7 ? 2 : 1;
    for (const word of prepared.vocabulary) {
      if (word[0] !== term[0]) continue;
      const distance = near(term, word, limit);
      if (distance > limit) continue;
      // The word's other forms with it: `subscripton` finds `subscriptions` as well as `subscription`.
      for (const form of prepared.stems.get(stem(word)) ?? [word]) offer(form, { quality: distance === 1 ? 0.5 : 0.35 });
    }
  }
  return found;
}

/** A unit's text with the parts of it the query matched. */
function marked(unit: Unit, terms: Map<string, Match>[]): Marked {
  const marks: Range[] = [];
  for (const token of tokensOf(unit)) {
    for (const term of terms) {
      const match = term.get(token.word);
      if (match) marks.push([token.start, match.prefix && !token.joined ? token.start + match.prefix : token.end]);
    }
  }
  marks.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const merged: Range[] = [];
  for (const mark of marks) {
    const before = merged.at(-1);
    // Two matches with one space or joiner between them are one mark: `auto-merge`, `API key`.
    if (before && (mark[0] <= before[1] || (mark[0] === before[1] + 1 && /[^a-z0-9]/i.test(unit.text[before[1]]!)))) before[1] = Math.max(before[1], mark[1]);
    else merged.push([mark[0], mark[1]]);
  }
  return { text: unit.text, code: unit.code, marks: merged };
}

/** The longest an excerpt runs before the dialog's own line clamp cuts it. */
const EXCERPT = 240;

/** A part of a text, with its ranges moved to match and, unless it is `bare`, an ellipsis at each end that was cut. */
function cut(text: Marked, start: number, end: number, bare = false): Marked {
  const lead = start > 0 && !bare ? '…' : '';
  const tail = end < text.text.length && !bare ? '…' : '';
  const move = (ranges: Range[]) =>
    ranges
      .filter(([from, to]) => to > start && from < end)
      .map(([from, to]): Range => [Math.max(from, start) - start + lead.length, Math.min(to, end) - start + lead.length]);
  return { text: `${lead}${text.text.slice(start, end)}${tail}`, code: move(text.code), marks: move(text.marks) };
}

/**
 * The part of a text a result shows: from its start when the match is near it, and otherwise from a
 * few words before the match. Code is the line that matched, or its first line.
 */
function excerpt(unit: Unit, text: Marked): Marked {
  const first = text.marks[0]?.[0] ?? 0;
  if (unit.kind === 'code') {
    let start = text.text.lastIndexOf('\n', first - 1) + 1;
    const stop = text.text.indexOf('\n', first);
    const end = Math.min(stop === -1 ? text.text.length : stop, start + EXCERPT);
    while (start < end && (text.text[start] === ' ' || text.text[start] === '\t')) start++;
    return cut(text, start, end, true);
  }
  let start = 0;
  if (first > 80) {
    start = text.text.lastIndexOf(' ', first - 50) + 1;
    // A sentence that starts just before reads better than a cut in the middle of the last one.
    const sentence = text.text.lastIndexOf('. ', first);
    if (sentence !== -1 && first - sentence < 110) start = sentence + 2;
  }
  return cut(text, start, Math.min(text.text.length, start + EXCERPT));
}

const plain = (unit: Unit): Marked => ({ text: unit.text, code: unit.code, marks: [] });

/**
 * The pages that match a query, best first, each with the sections of it that matched. A query
 * with no word of two characters or more matches nothing.
 */
export function search(prepared: Prepared, query: string, { pages: pageLimit = 8, sections: sectionLimit = 3 } = {}): PageResult[] {
  const typed = tokenize(query);
  const all = [...new Set(typed.tokens.filter((token) => !token.joined).map((token) => token.word))];
  const kept = all.filter((word) => !STOP.has(word));
  const words = kept.length ? kept : all;
  if (words.length === 0) return [];
  const stillTyping = /[a-z0-9]$/i.test(query);
  const terms = words.map((word, i) => candidates(prepared, word, stillTyping && i === words.length - 1));
  const phrase = words.length > 1 ? ` ${words.join(' ')}` : undefined;
  // A name typed whole (`PULLFROG_MODEL`, `fetch-depth`): the units that hold it as written.
  const named = new Set<number>();
  for (const token of typed.tokens) if (token.joined) for (const unit of prepared.postings.get(token.word) ?? []) named.add(unit);

  // How well each unit holds each word.
  const quality = terms.map((term) => {
    const of = new Map<number, number>();
    for (const [word, match] of term) {
      for (const unit of prepared.postings.get(word) ?? []) if ((of.get(unit) ?? 0) < match.quality) of.set(unit, match.quality);
    }
    return of;
  });
  const hit = new Set<number>();
  for (const of of quality) for (const unit of of.keys()) hit.add(unit);

  const need = words.length - (words.length > 2 ? 1 : 0);
  const score = new Map<number, number>();
  for (const id of hit) {
    const unit = prepared.units[id]!;
    const section = prepared.sections[unit.section]!;
    const title = prepared.sections[prepared.pages[unit.page]![0]!]!.heading;
    const parent = section.parent === undefined ? undefined : prepared.sections[section.parent]!.heading;
    let own = 0;
    let held = 0;
    let lent = 0;
    let around = 0;
    quality.forEach((of) => {
      const here = of.get(id);
      if (here) {
        own += here;
        held++;
        return;
      }
      // A word the text lacks may be in what it sits under: its heading, that heading's, the page's title.
      const from = Math.max(
        section.heading === undefined || section.heading === id ? 0 : (of.get(section.heading) ?? 0) * 0.5,
        parent === undefined ? 0 : (of.get(parent) ?? 0) * 0.35,
        title === undefined || title === id ? 0 : (of.get(title) ?? 0) * 0.35,
      );
      if (from) {
        lent += from;
        around++;
      }
    });
    if (held + around < need) continue;
    let value = (WEIGHT[unit.kind] * (own + lent)) / words.length;
    // A word lent by a heading counts for half a word held.
    value *= ((held + around / 2) / words.length) ** 2;
    if (held === words.length) value *= 1.25;
    if (phrase && unit.phrase.includes(phrase)) value *= 1.8;
    if (named.has(id)) value *= 1.6;
    if (unit.kind === 'title' || unit.kind === 'heading') {
      const said = ` ${words.join(' ')}`;
      if (unit.phrase === `${said} `) value *= 1.6;
      else if (unit.phrase.startsWith(said)) value *= 1.3;
      value /= 1 + 0.08 * Math.max(0, unit.words - words.length);
      if (unit.kind === 'heading' && section.depth > 2) value *= 0.9;
    } else {
      value /= 1 + 0.003 * unit.words;
    }
    score.set(id, value);
  }

  // Each section's score is its best unit's, and a little of its second's.
  type Scored = { section: number; value: number; best?: number };
  const bySection = new Map<number, number[]>();
  for (const id of score.keys()) {
    const section = prepared.units[id]!.section;
    const list = bySection.get(section);
    if (list) list.push(id);
    else bySection.set(section, [id]);
  }
  const scored = new Map<number, Scored>();
  for (const [section, ids] of bySection) {
    ids.sort((a, b) => score.get(b)! - score.get(a)!);
    const body = ids.find((id) => prepared.units[id]!.kind !== 'title' && prepared.units[id]!.kind !== 'heading');
    scored.set(section, { section, value: score.get(ids[0]!)! + 0.25 * (ids[1] === undefined ? 0 : score.get(ids[1])!), best: body });
  }

  const results: { value: number; order: number; result: PageResult }[] = [];
  prepared.pages.forEach((own, p) => {
    const found = own.map((section) => scored.get(section)).filter((entry): entry is Scored => entry !== undefined);
    if (found.length === 0) return;
    const ranked = [...found].sort((a, b) => b.value - a.value);
    const value = ranked[0]!.value + 0.2 * ((ranked[1]?.value ?? 0) + (ranked[2]?.value ?? 0));
    const page = prepared.index.pages[p]!;
    const top = prepared.sections[own[0]!]!;
    const lead = scored.get(own[0]!);
    const description = top.units.map((id) => prepared.units[id]!).find((unit) => unit.kind === 'description');
    const text = (id: number | undefined) => (id === undefined ? undefined : excerpt(prepared.units[id]!, marked(prepared.units[id]!, terms)));
    const sections = ranked
      .filter((entry) => entry.section !== own[0] && entry.value >= 0.25 * ranked[0]!.value)
      .slice(0, sectionLimit)
      .map((entry): SectionResult => {
        const section = prepared.sections[entry.section]!;
        const opening = section.units.map((id) => prepared.units[id]!).find((unit) => unit.kind === 'text');
        return {
          href: `${page.url}#${section.id}`,
          trail: section.parent === undefined ? undefined : prepared.units[prepared.sections[section.parent]!.heading!]!.text,
          title: marked(prepared.units[section.heading!]!, terms),
          excerpt: text(entry.best) ?? (opening ? excerpt(opening, plain(opening)) : undefined),
        };
      });
    results.push({
      value,
      order: p,
      result: {
        href: page.url,
        title: marked(prepared.units[top.heading!]!, terms),
        excerpt: text(lead?.best) ?? (description ? plain(description) : undefined),
        sections,
        sectionLeads: sections.length > 0 && ranked[0]!.section !== own[0],
      },
    });
  });
  results.sort((a, b) => b.value - a.value || a.order - b.order);
  const floor = (results[0]?.value ?? 0) * 0.08;
  return results
    .filter((entry) => entry.value >= floor)
    .slice(0, pageLimit)
    .map((entry) => entry.result);
}
