// The schema freshness check at run time (wiki/structured-params.md §6):
// any change fails. A module a build step writes carries the catalog facts its
// statement relied on, the columns it names with their types, whether each is not null and whether
// it has a default, for an insert's table the columns that need a value, and on Postgres and SQLite
// the text of each default a structured insert copied into its statement, beside the catalog query
// that read them (engine/core/src/fresh.rs). Before a query runs, the run time runs that query
// through the caller's own client and compares: any difference fails the call, naming the table
// and the column, rather than letting the query run against a schema its types were not made for.
//
// A query is checked on its first run in a process, and again when its last check is a minute old,
// so a long-running server notices a migration that lands while it runs. A catalog query that
// fails (a role that may not read the catalog) checks nothing, and the query runs; so does one
// whose answer is not shaped as its rows are, which no database gives: a client that answers every
// query with rows of its own, as a stub or a mock does, is no catalog to check against.

/**
 * What a module carries: the catalog query, which takes no parameter, and the facts as text, five
 * per column, or six on Postgres and SQLite, whose sixth is a default's text; `defaults` names the
 * `[table, column]` whose sixth is compared, each a default the statement copied.
 */
export type Fresh = {
  readonly query: string;
  readonly rows: readonly (readonly string[])[];
  readonly inserts: readonly string[];
  readonly defaults?: readonly (readonly string[])[];
};

/** How long a check holds, in milliseconds. */
export const FRESH_FOR = 60_000;

const checked = new WeakMap<object, number>();

// The clock is `performance.now()`, not `Date.now()`: an interval needs only a monotonic clock, and
// Next.js's Cache Components refuse `Date.now()` while a page prerenders, which failed `next build`
// on every page that ran a query (examples/nextjs, Next.js 16.4). It is reached through
// `globalThis`, typed here, since a project that type-checks Reylo's sources need not have the
// DOM's types or Node's, which declare it (the labs' type checks failed with "Cannot find name").
const clock = (globalThis as unknown as { performance: { now(): number } }).performance;

/** Whether `query` is due for a check now. */
export function due(query: object, now = clock.now()): boolean {
  const at = checked.get(query);
  return at === undefined || now - at >= FRESH_FOR;
}

/** Records a check of `query` that passed, or that could not run. */
export function passed(query: object, now = clock.now()): void {
  checked.set(query, now);
}

const yes = (v: string) => v === "true" || v === "1";
const describe = (r: readonly string[]) => `${r[2]}${yes(r[3]!) ? " not null" : ""}${yes(r[4]!) ? " with a default" : ""}`;
const table = (name: string) => name.replace(/^"|"$/g, "").replace(/"\."/g, ".");

/** The catalog query's columns, as engine/core/src/fresh.rs names them; the sixth on Postgres and SQLite. */
const COLUMNS = ["fresh_table", "fresh_column", "fresh_type", "fresh_not_null", "fresh_default", "fresh_expr"] as const;

/**
 * The differences between the facts the build saw and the rows the catalog query returns now, one
 * line each; none when the schema still holds what the query relied on, and `undefined` when the
 * rows are not the catalog query's: each is an object with its five or six columns, as many as
 * the module's facts have, or an array of as many values where a driver hands rows back as arrays.
 * A default the statement copied is compared by its text.
 *
 * A SQLite type is compared without its case, as SQLite reads a declared type: SQLite's
 * `pragma_table_xinfo` upper-cases the type names it knows (`integer` is `INTEGER`) and Turso's
 * engine gives them as declared, so a build on one and a run on the other saw the same schema. A
 * Postgres or MySQL type is compared exactly: a quoted Postgres type name keeps its case.
 */
export function differences(fresh: Fresh, rows: readonly unknown[], dialect?: "postgres" | "mysql" | "sqlite" | "duckdb"): string[] | undefined {
  const now = new Map<string, string[]>();
  const width = fresh.rows[0]?.length === 6 ? 6 : 5;
  const columns = COLUMNS.slice(0, width);
  for (const row of rows) {
    if (row === null || typeof row !== "object") return undefined;
    const raw: unknown[] | undefined = Array.isArray(row) ? (row.length === width ? row : undefined) : Object.keys(row).length === width && columns.every((k) => k in row) ? columns.map((k) => (row as Record<string, unknown>)[k]) : undefined;
    if (raw === undefined) return undefined;
    const values = raw.map((v) => (v === null || v === undefined ? "" : String(v)));
    now.set(`${values[0]}\u0000${values[1]}`, values);
  }
  const sameType = dialect === "sqlite" ? (a: string, b: string) => a === b || asciiLower(a) === asciiLower(b) : (a: string, b: string) => a === b;
  const sameDefault = dialect === "sqlite" ? (a: string, b: string) => a === b || sqliteDefault(a) === sqliteDefault(b) : (a: string, b: string) => a === b;
  const out: string[] = [];
  const seen = new Set<string>();
  const copied = new Set((fresh.defaults ?? []).map((d) => `${d[0]}\u0000${d[1]}`));
  for (const was of fresh.rows) {
    const key = `${was[0]}\u0000${was[1]}`;
    seen.add(key);
    const is = now.get(key);
    if (!is) out.push(`${table(was[0]!)}.${was[1]} was ${describe(was)} and no longer exists`);
    else if (!sameType(is[2]!, was[2]!) || yes(is[3]!) !== yes(was[3]!) || yes(is[4]!) !== yes(was[4]!)) out.push(`${table(was[0]!)}.${was[1]} was ${describe(was)} and is now ${describe(is)}`);
    // The insert writes the default as the build read it, for a row that leaves the key out.
    else if (copied.has(key) && !sameDefault(is[5]!, was[5]!)) out.push(`${table(was[0]!)}.${was[1]} had ${was[5] ? `the default ${was[5]}` : "no default"}, which the insert writes for a row that leaves its key out, and now has ${is[5] ? `the default ${is[5]}` : "none"}`);
  }
  // A column an insert must now fill, which the build did not see.
  for (const [key, is] of now) {
    if (!seen.has(key) && fresh.inserts.includes(is[0]!) && yes(is[3]!) && !yes(is[4]!)) out.push(`${table(is[0]!)}.${is[1]} is now ${describe(is)}, which the insert does not write`);
  }
  return out;
}

/**
 * A SQLite default's text in one spelling, since engines that read the same schema write it
 * differently: `pragma_table_xinfo` gives the text as written on SQLite (`member`,
 * `lower(hex(randomblob(4)))`, `-7`, `current_timestamp`), and Turso's engine writes it again
 * (`'member'`, `(lower (hex (randomblob (4))))`, `- 7`, `CURRENT_TIMESTAMP`), which evaluates the
 * same (wiki/structured-params/default-extraction.md). Outside its strings the text loses its
 * spaces and its ASCII case and the parentheses around it all, and a lone identifier is the string
 * SQLite stores for it, as the build copies it (engine/core/src/structured.rs).
 */
export function sqliteDefault(text: string): string {
  const t = text.trim();
  const lone = /^(?:"((?:[^"]|"")*)"|`((?:[^`]|``)*)`|\[([^\]]*)\]|([A-Za-z_\u0080-\uffff][\w$\u0080-\uffff]*))$/.exec(t);
  if (lone && !(lone[4] !== undefined && /^(?:true|false|null|current_time|current_date|current_timestamp)$/i.test(lone[4]))) {
    const name = lone[1] !== undefined ? lone[1].replace(/""/g, '"') : lone[2] !== undefined ? lone[2].replace(/``/g, "`") : (lone[3] ?? lone[4])!;
    return `'${name.replace(/'/g, "''")}'`;
  }
  let out = "";
  for (let i = 0; i < t.length; ) {
    if (t[i] === "'") {
      let j = i + 1;
      while (j < t.length && !(t[j] === "'" && t[j + 1] !== "'")) j += t[j] === "'" ? 2 : 1;
      out += t.slice(i, j + 1);
      i = j + 1;
    } else {
      if (!/\s/.test(t[i]!)) out += asciiUpper(t[i]!);
      i++;
    }
  }
  // Parentheses around the whole text, as many as there are.
  while (out.startsWith("(") && out.endsWith(")") && closes(out) === out.length - 1) out = out.slice(1, -1);
  return out;
}

/** Where the parenthesis that opens `s` closes, its strings skipped. */
function closes(s: string): number {
  let depth = 0;
  for (let i = 0; i < s.length; i++) {
    if (s[i] === "'") {
      i++;
      while (i < s.length && !(s[i] === "'" && s[i + 1] !== "'")) i += s[i] === "'" ? 2 : 1;
    } else if (s[i] === "(") depth++;
    else if (s[i] === ")" && --depth === 0) return i;
  }
  return -1;
}

const asciiUpper = (s: string) => s.replace(/[a-z]+/g, (m) => m.toUpperCase());

/** SQLite folds the case of ASCII letters alone. */
const asciiLower = (s: string) => s.replace(/[A-Z]+/g, (m) => m.toLowerCase());

/** The error a call fails with when the schema changed under its query. */
export function staleError(source: string | undefined, lines: readonly string[]): Error {
  return new Error(`The database changed after ${source ? `${source} was built` : "this query was built"}: ${lines.join("; ")}. Build it again (the type check, or reylo generate) and deploy the new build.`);
}
