// The editor's completions, offered by where the cursor stands in the statement: tables after FROM
// and JOIN, types after `::`, the columns of the tables the query reads in an expression, an
// alias's columns after `u.`, and only the keywords that can come next. The statement is read as
// tokens, not parsed: a half-typed statement is the usual case, and its clauses are enough to know
// what can follow.
import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import type { Catalog, Table } from '../../../src/play-catalog.ts';

type Token = { text: string; lower: string; kind: 'word' | 'quoted' | 'string' | 'number' | 'param' | 'op' | 'punct' | 'comment'; from: number; to: number; depth: number };

/** The statement's tokens, each with how many parentheses it sits inside. */
export function tokenize(text: string): Token[] {
  const out: Token[] = [];
  let depth = 0;
  let i = 0;
  const push = (kind: Token['kind'], from: number, to: number) => {
    const t = text.slice(from, to);
    if (t === ')') depth = Math.max(0, depth - 1);
    out.push({ text: t, lower: t.toLowerCase(), kind, from, to, depth });
    if (t === '(') depth++;
  };
  while (i < text.length) {
    const c = text[i]!;
    const rest = text.slice(i);
    let m: RegExpExecArray | null;
    if (/\s/.test(c)) i++;
    else if (rest.startsWith('--')) {
      const end = text.indexOf('\n', i);
      push('comment', i, (i = end < 0 ? text.length : end));
    } else if (rest.startsWith('/*')) {
      const end = text.indexOf('*/', i + 2);
      push('comment', i, (i = end < 0 ? text.length : end + 2));
    } else if ((m = /^[eE]?'(?:[^']|'')*(?:'|$)/.exec(rest)) || (m = /^(\$[A-Za-z_]*\$)[\s\S]*?(?:\1|$)/.exec(rest))) push('string', i, (i += m[0].length));
    else if ((m = /^"(?:[^"]|"")*(?:"|$)/.exec(rest))) push('quoted', i, (i += m[0].length));
    else if ((m = /^::/.exec(rest))) push('punct', i, (i += 2));
    else if ((m = /^:[A-Za-z_][\w]*(?:\.[A-Za-z_][\w]*)*|^\$\d+/.exec(rest))) push('param', i, (i += m[0].length));
    else if ((m = /^\d+(?:\.\d*)?(?:[eE][+-]?\d+)?/.exec(rest))) push('number', i, (i += m[0].length));
    else if ((m = /^[A-Za-z_][\w$]*/.exec(rest))) push('word', i, (i += m[0].length));
    else if ((m = /^[+\-*/<>=~!@#%^&|`?]+/.exec(rest))) push('op', i, (i += m[0].length));
    else push('punct', i, ++i);
  }
  return out;
}

// Words that start a clause: the last of them at the cursor's level says what can come next.
const CLAUSES = new Set(['select', 'from', 'join', 'on', 'where', 'group', 'order', 'having', 'limit', 'offset', 'set', 'returning', 'values', 'into', 'update', 'delete', 'insert', 'using', 'with', 'window', 'partition', 'union', 'intersect', 'except', 'fetch']);

// Words that never end an expression (`where u.id is` still waits for one): every keyword the
// lists below offer, and the rest of what Postgres reserves.
const KEYWORDS = new Set(
  (
    'all and any array as asc between both by case cast collate constraint create cross current_date current_time current_timestamp current_user default delete desc distinct do else end except exists false fetch filter first for foreign from full group having ilike in inner insert intersect interval into is join last lateral leading left like limit natural not null nulls offset on only or order outer over partition recursive returning right select set similar some table then to trailing true union unique update using values when where window with'
  ).split(' '),
);
// Words after which an expression is complete.
const ENDS_EXPRESSION = new Set(['null', 'true', 'false', 'end', 'current_date', 'current_time', 'current_timestamp', 'current_user']);

// SQL's own forms that read as calls: offered with the functions.
const FORMS = ['coalesce', 'nullif', 'greatest', 'least', 'cast'];

type Kind = 'column' | 'table' | 'function' | 'aggregate' | 'keyword' | 'type';
type Scope = { name: string; table?: Table; target?: boolean }[];

/** The tables the statement reads and writes, by alias where it has one, and its WITH queries. */
function scopeOf(tokens: Token[], catalog: Catalog): Scope {
  const scope: Scope = [];
  const word = (t: Token | undefined) => t && (t.kind === 'word' || t.kind === 'quoted') && !KEYWORDS.has(t.lower);
  const name = (t: Token) => (t.kind === 'quoted' ? t.text.slice(1, -1).replace(/""/g, '"') : t.lower);
  const fromList = new Set<number>();
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i]!;
    if (t.lower === 'with' || (t.text === ',' && tokens[i + 2]?.lower === 'as' && tokens[i + 3]?.text === '(')) {
      // `with recent as (`, and each later `, other as (`.
      let j = i + 1;
      if (tokens[j]?.lower === 'recursive') j++;
      if (word(tokens[j]) && tokens[j + 1]?.lower === 'as') scope.push({ name: name(tokens[j]!) });
      continue;
    }
    if (CLAUSES.has(t.lower) && !['from', 'join', 'update', 'into', 'using'].includes(t.lower)) fromList.delete(t.depth);
    const starts = ['from', 'join', 'update', 'into', 'using'].includes(t.lower) || (t.text === ',' && fromList.has(t.depth));
    if (!starts) continue;
    if (t.lower === 'from' || t.lower === 'using') fromList.add(t.depth);
    let j = i + 1;
    while (['only', 'lateral'].includes(tokens[j]?.lower ?? '')) j++;
    let table: Table | undefined;
    let itemName: string | undefined;
    if (tokens[j]?.text === '(') {
      // A subquery: skip to its close, where only its alias names it.
      let depth = 0;
      for (; j < tokens.length; j++) {
        if (tokens[j]!.text === '(') depth++;
        else if (tokens[j]!.text === ')' && --depth === 0) break;
      }
      j++;
    } else if (word(tokens[j])) {
      if (tokens[j + 1]?.text === '.' && word(tokens[j + 2])) j += 2;
      itemName = name(tokens[j]!);
      table = catalog.tables.find((x) => x.name === itemName) ?? undefined;
      j++;
      if (tokens[j]?.text === '(') {
        // `insert into users (name)`: the target, before its columns; or a function in FROM.
        if (t.lower === 'into' && table) scope.push({ name: itemName, table, target: true });
        continue;
      }
    } else continue;
    if (tokens[j]?.lower === 'as') j++;
    const alias = word(tokens[j]) ? name(tokens[j]!) : undefined;
    const target = t.lower === 'update' || t.lower === 'into';
    if (alias) scope.push({ name: alias, table, target });
    else if (itemName) scope.push({ name: itemName, table, target });
  }
  return scope;
}

export function sqlCompletions(catalog: Catalog) {
  const option = (label: string, type: Kind, boost: number, detail?: string): Completion => ({ label, type, boost, ...(detail ? { detail } : {}) });
  const keywords = (words: string[]) => words.map((w) => option(w, 'keyword', -1));
  const types = catalog.types.map((t) => option(t, 'type', 0));
  const functions = [...catalog.functions.map((f) => option(f.name, f.kind, 0, f.returns)), ...FORMS.map((f) => option(f, 'function', 0))];
  const rowFunctions = catalog.functions.filter((f) => f.rows).map((f) => option(f.name, f.kind, -0.5, f.returns));
  const columnsOf = (table: Table | undefined) => (table?.columns ?? []).map((c) => option(c.name, 'column', 2, c.type));
  const tables = (scope: Scope) => [
    ...catalog.tables.map((t) => option(t.name, 'table', 1)),
    ...scope.filter((s) => !s.table && !catalog.tables.some((t) => t.name === s.name)).map((s) => option(s.name, 'table', 1)),
  ];

  return (context: CompletionContext): CompletionResult | null => {
    // Nothing while a parameter's name is typed (`:id`, `:auth.user_id`, `$1`): the name is the reader's own.
    if (context.matchBefore(/(?:^|[^:]):[\w.]*$|\$\d*$/)) return null;
    const typed = context.matchBefore(/[\w$]*$/)!;
    const doc = context.state.doc.toString();
    const all = tokenize(doc);
    // Not inside a string, a quoted name or a comment, an unclosed one included.
    const closed = (t: Token) => (t.kind === 'comment' ? t.text.startsWith('--') || t.text.endsWith('*/') : t.text.length > 1 && t.text.endsWith(t.kind === 'quoted' ? '"' : t.text.startsWith('$') ? (/^\$[A-Za-z_]*\$/.exec(t.text)?.[0] ?? '$') : "'"));
    const inside = (t: Token) => t.from < context.pos && (context.pos < t.to || (context.pos === t.to && (!closed(t) || t.text.startsWith('--'))));
    if (all.some((t) => (t.kind === 'string' || t.kind === 'quoted' || t.kind === 'comment') && inside(t))) return null;
    // The statement around the cursor.
    const start = all.findLastIndex((t) => t.text === ';' && t.to <= typed.from) + 1;
    let end = all.findIndex((t, i) => i >= start && t.text === ';');
    if (end < 0) end = all.length;
    const tokens = all.slice(start, end).filter((t) => t.kind !== 'comment');
    const before = tokens.filter((t) => t.to <= typed.from);
    // The scope without the word being typed, which is no table yet.
    const scope = scopeOf(tokens.filter((t) => !(t.from === typed.from && t.kind === 'word')), catalog);
    const word = typed.text;

    const result = (options: Completion[], minimum = 0): CompletionResult | null => {
      if (!options.length) return null;
      if (!context.explicit && word.length < minimum) return null;
      if (!context.explicit && word.length === 0) return null;
      return { from: typed.from, options };
    };

    const prev = before.at(-1);
    const prev2 = before.at(-2);

    // After a qualifier: an alias's or a table's columns, or a schema's tables.
    if (prev?.text === '.' && prev.to === typed.from && prev2 && (prev2.kind === 'word' || prev2.kind === 'quoted')) {
      const q = prev2.kind === 'quoted' ? prev2.text.slice(1, -1) : prev2.lower;
      if (q === 'public') return { from: typed.from, options: catalog.tables.map((t) => option(t.name, 'table', 1)) };
      const table = scope.find((s) => s.name === q)?.table ?? catalog.tables.find((t) => t.name === q);
      return table ? { from: typed.from, options: columnsOf(table) } : null;
    }
    // After a cast's `::`, or `cast(x as`: a type.
    if (prev?.text === '::') return { from: typed.from, options: types };
    const level = prev ? (prev.text === '(' ? prev.depth + 1 : prev.depth) : 0;
    const opener = (() => {
      for (let i = before.length - 1, depth = 0; i >= 0; i--) {
        const t = before[i]!;
        if (t.text === ')') depth++;
        else if (t.text === '(' && depth-- === 0) return i;
      }
      return -1;
    })();
    const atLevel = before.slice(opener + 1).filter((t) => t.depth === level);
    const clause = atLevel.findLast((t) => CLAUSES.has(t.lower))?.lower;
    const beforeOpener = opener >= 0 ? before[opener - 1] : undefined;
    // `insert into users (`: the target's columns.
    if (opener >= 0 && before[opener - 2]?.lower === 'into' && (prev?.text === '(' || prev?.text === ',')) {
      const columns = columnsOf(scope.find((s) => s.target)?.table);
      return columns.length ? { from: typed.from, options: columns } : null;
    }
    if (prev?.lower === 'as') {
      if (beforeOpener?.lower === 'cast' && clause === undefined) return result(types);
      return null; // naming an alias
    }

    // The start of a statement, or of a subquery.
    const statement = keywords(['select', 'with', 'insert', 'update', 'delete', 'values']);
    if (!prev || prev.text === ';') return result(statement);
    if (prev.text === '(' && (prev2?.lower === 'as' || prev2?.lower === 'from' || prev2?.lower === 'join' || prev2?.lower === 'lateral')) return result(keywords(['select', 'with', 'values']));

    const ends = (t: Token | undefined): boolean =>
      !!t && (['quoted', 'string', 'number', 'param'].includes(t.kind) || t.text === ')' || (t.kind === 'word' && (!KEYWORDS.has(t.lower) || ENDS_EXPRESSION.has(t.lower))) || (t.text === '*' && (!prev2 || prev2.lower === 'select' || prev2.text === ',' || prev2.text === '.')));

    // Two-word keywords and their second words.
    const second: Record<string, string[]> = { group: ['by'], order: ['by'], partition: ['by'], nulls: ['first', 'last'], left: ['join', 'outer'], right: ['join', 'outer'], full: ['join', 'outer'], outer: ['join'], inner: ['join'], cross: ['join'], natural: ['join', 'left', 'inner'], insert: ['into'], delete: ['from'], is: ['null', 'not', 'true', 'false', 'distinct'], union: ['all', 'select'] };
    if (prev.kind === 'word' && second[prev.lower] && !(prev.lower === 'is')) return result(keywords(second[prev.lower]!));

    // Where a table goes.
    const fromItem = ['from', 'join', 'using'].includes(clause ?? '');
    if (fromItem || clause === 'update' || clause === 'into') {
      const awaitsTable = prev.lower === clause || prev.text === ',' || prev.lower === 'only' || prev.lower === 'lateral';
      if (awaitsTable && prev.text !== ',' && clause === 'into') return result(tables(scope).filter((o) => catalog.tables.some((t) => t.name === o.label)));
      if (awaitsTable && clause === 'update') return result(tables(scope).filter((o) => catalog.tables.some((t) => t.name === o.label)));
      // `delete from` takes a table alone.
      if (awaitsTable && clause === 'from' && prev2?.lower === 'delete') return result(tables(scope).filter((o) => catalog.tables.some((t) => t.name === o.label)));
      if (awaitsTable && fromItem) return result([...tables(scope), ...rowFunctions, ...keywords(['lateral'])], 1);
      if (clause === 'into') return result(keywords(prev.text === ')' ? ['values', 'select', 'default'] : ['values', 'select', 'default', 'as']));
      if (clause === 'update') return result(keywords(['set', 'as']));
      const joins = ['join', 'left', 'right', 'inner', 'full', 'cross', 'natural'];
      return result(keywords([...(ends(prev) && prev2 && (prev2.lower === clause || prev2.text === ',') ? ['as'] : []), ...(clause === 'join' ? ['on', 'using'] : []), 'where', ...joins, 'group', 'order', 'limit', 'offset', 'having', 'window', 'union', 'intersect', 'except', 'returning', 'for']));
    }
    if (clause === 'delete') return result(keywords(['from']));
    if (clause === 'insert') return result(keywords(['into']));
    if (clause === 'limit' || clause === 'offset' || clause === 'fetch') return ends(prev) ? result(keywords(clause === 'limit' ? ['offset'] : ['limit', 'rows', 'fetch'])) : null;
    if (clause === 'with') return ends(prev) ? result(keywords(['as'])) : prev.lower === 'with' ? result(keywords(['recursive'])) : null;
    if (clause === 'set' && (prev.lower === 'set' || prev.text === ',')) return result(columnsOf(scope.find((s) => s.target)?.table));

    // An expression: inside a CASE, its words come next.
    const cases = atLevel.filter((t) => t.lower === 'case').length - atLevel.filter((t) => t.lower === 'end').length;
    const inCase = cases > 0 ? ['when', 'then', 'else', 'end'] : [];
    if (ends(prev) || prev.lower === 'asc' || prev.lower === 'desc' || prev.lower === 'first' || prev.lower === 'last') {
      const operators = ['and', 'or', 'is', 'not', 'in', 'like', 'ilike', 'between'];
      const next: Record<string, string[]> = {
        select: ['as', 'from', ...operators, 'over', 'filter', 'union'],
        where: [...operators, 'group', 'order', 'limit', 'offset', 'returning', 'union', 'for'],
        on: [...operators, 'where', 'join', 'left', 'right', 'inner', 'full', 'cross', 'group', 'order', 'limit'],
        having: [...operators, 'order', 'limit', 'offset', 'union'],
        group: ['having', 'order', 'limit', 'offset', 'union'],
        order: ['asc', 'desc', 'nulls', 'limit', 'offset'],
        partition: ['order'],
        returning: ['as'],
        set: [...operators, 'where', 'from', 'returning'],
        values: opener < 0 ? ['returning', 'on'] : [],
        window: ['as'],
      };
      if (prev.lower === 'asc' || prev.lower === 'desc') return result(keywords(['nulls', 'limit', 'offset']));
      if (prev.lower === 'first' || prev.lower === 'last') return result(keywords(['limit', 'offset']));
      // Inside a CASE, only its own words and the operators: the clause's words wait for its END.
      return result(keywords(inCase.length ? [...inCase, ...operators] : clause && next[clause] ? next[clause]! : operators));
    }

    // The start of an expression: the columns in scope first, then the tables to qualify them with,
    // then the functions, then the few keywords that start one.
    const reads = scope.filter((s) => s.table || !catalog.tables.some((t) => t.name === s.name));
    const columns = new Map<string, Completion>();
    for (const s of reads.length ? reads : catalog.tables.map((t) => ({ name: t.name, table: t }))) for (const c of columnsOf(s.table)) if (!columns.has(c.label)) columns.set(c.label, c);
    const qualifiers = scope.map((s) => option(s.name, 'table', 1));
    const starts = ['case', 'not', 'exists', 'null', 'true', 'false', 'array', 'interval', 'current_date', 'current_timestamp'];
    const extra = [...(prev.lower === 'select' ? ['distinct'] : []), ...(prev.text === '(' ? ['select'] : []), ...(prev.lower === 'not' ? ['in', 'like', 'ilike', 'between'] : []), ...(prev.lower === 'is' ? ['null', 'not', 'true', 'false', 'distinct'] : [])];
    if (prev.lower === 'is') return result(keywords(['null', 'not', 'true', 'false', 'distinct']));
    const pool = [...columns.values(), ...qualifiers, ...keywords([...extra, ...starts, ...inCase])];
    // A word of one letter matches too much of Postgres's thousand functions: they join from the second.
    return result(word.length >= 2 || context.explicit ? [...pool, ...functions] : pool, 1);
  };
}
