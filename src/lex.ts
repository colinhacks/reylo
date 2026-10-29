// Placeholder lexing for the runtime loader: finds `:name` (and SQLite's `@name` and `$name`), and
// the dialect's own positional form (`$1` on Postgres, `?` on MySQL, `?` and `?NNN` on SQLite, and
// all three on DuckDB),
// outside strings and comments, and
// returns the binding order. A name may be dotted
// (`:auth.user_id`); each segment starts like a name, so `arr[1:2]` is a slice. MySQL text is read
// as a session in the default `sql_mode` reads it: a backslash in a string escapes the character
// after it, `"` quotes a string, `#` starts a line comment, and `--` does only before a space or
// a control character. This is the provisional JavaScript version of the engine's lexer; the Rust
// engine owns the authoritative one (dollar quoting, E'' strings, nested comments, array slices,
// MySQL's other modes).
export type Placeholder = { name: string; start: number; end: number };

// A name drops its prefix (`:id` is `id`); a positional placeholder is named by its number
// (`$2` and `?2` are `2`), and a bare `?` takes the number after the largest so far.
export function lexPlaceholders(sql: string, dialect: "postgres" | "mysql" | "sqlite" | "duckdb"): { params: string[]; placeholders: Placeholder[] } {
  const placeholders: Placeholder[] = [];
  const params: string[] = [];
  let largest = 0;
  let i = 0;
  const push = (name: string, start: number, end: number) => {
    placeholders.push({ name, start, end });
    if (!params.includes(name)) params.push(name);
  };
  const position = (digits: string) => {
    const n = digits.replace(/^0+(?=\d)/, "");
    largest = Math.max(largest, Number(n));
    return n;
  };
  const mysql = dialect === "mysql";
  while (i < sql.length) {
    const ch = sql[i]!;
    const next = sql[i + 1];
    if (ch === "'" || ch === '"') { i = skipQuoted(sql, i, ch, mysql); continue; }
    if (ch === "`") { i = skipQuoted(sql, i, ch, false); continue; }
    // MySQL's `--` is two minus signs before anything but a space or a control character.
    const comment = ch === "#" ? mysql : ch === "-" && next === "-" && !(mysql && i + 2 < sql.length && sql.charCodeAt(i + 2) > 32 && sql.charCodeAt(i + 2) !== 127);
    if (comment) { i = sql.indexOf("\n", i); if (i === -1) break; continue; }
    if (ch === "/" && next === "*") { const end = sql.indexOf("*/", i + 2); i = end === -1 ? sql.length : end + 2; continue; }
    if (ch === ":" && next === ":") { i += 2; continue; }
    // `arr[lo:hi]` is a slice: a name character before the colon means it opens nothing. SQLite's
    // own `@name` and `$name` are named placeholders there too.
    if ((ch === ":" || (dialect === "sqlite" && (ch === "@" || ch === "$"))) && next !== undefined && /[A-Za-z_]/.test(next) && !(i > 0 && /[A-Za-z0-9_]/.test(sql[i - 1]!))) {
      const m = /^[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*/.exec(sql.slice(i + 1))!;
      push(m[0], i, i + 1 + m[0].length); i += 1 + m[0].length; continue;
    }
    if (ch === "$" && (dialect === "postgres" || dialect === "duckdb") && next !== undefined && /[0-9]/.test(next)) {
      const m = /^[0-9]+/.exec(sql.slice(i + 1))!;
      push(position(m[0]), i, i + 1 + m[0].length); i += 1 + m[0].length; continue;
    }
    if (ch === "?" && (dialect === "sqlite" || dialect === "duckdb") && next !== undefined && /[0-9]/.test(next)) {
      // SQLite's and DuckDB's `?NNN` names the same position a `$NNN` does.
      const m = /^[0-9]+/.exec(sql.slice(i + 1))!;
      push(position(m[0]), i, i + 1 + m[0].length); i += 1 + m[0].length; continue;
    }
    if (ch === "?" && dialect !== "postgres") { largest += 1; push(String(largest), i, i + 1); i += 1; continue; }
    i += 1;
  }
  // Positional parameters bind by number, whatever order they appear in.
  if (params.length && params.every(isPositional)) params.sort((a, b) => Number(a) - Number(b));
  return { params, placeholders };
}

/** Whether a parameter name is a position: a number, where a name starts with a letter or `_`. */
export function isPositional(name: string): boolean {
  return /^\d+$/.test(name);
}

// A doubled quote is one character, and so, where `escapes`, is a backslash and what follows it.
function skipQuoted(sql: string, start: number, quote: string, escapes: boolean): number {
  let i = start + 1;
  while (i < sql.length) {
    if (escapes && sql[i] === "\\") { i += 2; continue; }
    if (sql[i] === quote) { if (sql[i + 1] === quote) { i += 2; continue; } return i + 1; }
    i += 1;
  }
  return i;
}
