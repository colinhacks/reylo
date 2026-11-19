// Placeholder lexing for the runtime loader: finds `:name` (and SQLite's `@name` and `$name`), and
// the dialect's own positional form (`$1` on Postgres, `?` on MySQL, `?` and `?NNN` on SQLite),
// outside strings and comments, and returns the binding order. A name may be dotted
// (`:auth.user_id`); each segment starts like a name, so `arr[1:2]` is a slice, and on Postgres a
// colon that opens a subscript is a slice with no lower bound (`tags[:n]`). MySQL text is read
// as a session in the default `sql_mode` reads it, unless a `MysqlText` says otherwise: a
// backslash in a string escapes the character after it, `"` quotes a string, `#` starts a line comment, and `--` does only before a space or
// a control character. ClickHouse writes a parameter with its type, `{name:Type}`, which is the
// only thing a `{` starts there, the placeholder being the whole of it; `:name` is one there only
// where a cast types it (`:name::Type`), since `?` is its conditional operator, whose `:` may
// stand before a name. Its text is read its way: a backslash escapes in a string and in a quoted
// name, `#` comments before a space or a `!`, a block comment nests, and `$tag$…$tag$` quotes.
// This is the provisional JavaScript version of the engine's lexer; the Rust
// engine owns the authoritative one (dollar quoting, E'' strings, nested comments, array slices,
// MySQL's other modes).
export type Placeholder = { name: string; start: number; end: number };

/** How a MySQL session reads text, by its `sql_mode` (the engine's `MysqlText`): the default mode reads a backslash in a string as an escape, and a double-quoted word as a string. */
export type MysqlText = { backslashEscapes: boolean; ansiQuotes: boolean };
const DEFAULT_MYSQL_TEXT: MysqlText = { backslashEscapes: true, ansiQuotes: false };

// A name drops its prefix (`:id` is `id`); a positional placeholder is named by its number
// (`$2` and `?2` are `2`), and a bare `?` takes the number after the largest so far.
export function lexPlaceholders(sql: string, dialect: "postgres" | "mysql" | "sqlite" | "clickhouse", mysqlText: MysqlText = DEFAULT_MYSQL_TEXT): { params: string[]; placeholders: Placeholder[] } {
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
  const clickhouse = dialect === "clickhouse";
  const doubleQuotedString = mysql && !mysqlText.ansiQuotes;
  const escapes = (mysql && mysqlText.backslashEscapes) || clickhouse;
  // What was read last, past whitespace and comments: an operand (a name other than `array`, a
  // number, a quoted name, a placeholder, `)` or `]`), the `[` of a subscript after one, or other.
  let last: "other" | "operand" | "subscript" = "other";
  while (i < sql.length) {
    const ch = sql[i]!;
    const next = sql[i + 1];
    if (ch === "'" || (ch === '"' && doubleQuotedString)) { i = skipQuoted(sql, i, ch, escapes); last = "other"; continue; }
    if (ch === '"' || ch === "`") { i = skipQuoted(sql, i, ch, clickhouse); last = "operand"; continue; }
    // SQLite's `[name]` is a quoted identifier, which ends at the first `]`.
    if (ch === "[" && dialect === "sqlite") { const end = sql.indexOf("]", i); i = end === -1 ? sql.length : end + 1; last = "operand"; continue; }
    // MySQL's `--` is two minus signs before anything but a space or a control character.
    // ClickHouse's `#` starts a comment only before a space or a `!`.
    const comment = ch === "#" ? mysql || (clickhouse && (next === " " || next === "!")) : ch === "-" && next === "-" && !(mysql && i + 2 < sql.length && sql.charCodeAt(i + 2) > 32 && sql.charCodeAt(i + 2) !== 127);
    if (comment) { i = sql.indexOf("\n", i); if (i === -1) break; continue; }
    if (ch === "/" && next === "*" && clickhouse) {
      // A ClickHouse block comment nests.
      let depth = 1;
      i += 2;
      while (i < sql.length && depth > 0) {
        if (sql.startsWith("/*", i)) { depth += 1; i += 2; } else if (sql.startsWith("*/", i)) { depth -= 1; i += 2; } else i += 1;
      }
      continue;
    }
    if (ch === "/" && next === "*") { const end = sql.indexOf("*/", i + 2); i = end === -1 ? sql.length : end + 2; continue; }
    if (clickhouse && ch === "$") {
      // A heredoc, `$tag$…$tag$`.
      const tag = /^\$(?:[A-Za-z_][A-Za-z0-9_]*)?\$/.exec(sql.slice(i));
      if (tag) { const end = sql.indexOf(tag[0], i + tag[0].length); i = end === -1 ? sql.length : end + tag[0].length; last = "other"; continue; }
    }
    if (clickhouse && ch === "{") {
      // `{name:Type}`, to the `}` that closes it, past the parentheses and strings of its type.
      const head = /^\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*:(?!:)/.exec(sql.slice(i));
      const close = head ? bracesEnd(sql, i + head[0].length) : -1;
      if (head && close !== -1 && sql.slice(i + head[0].length, close).trim() !== "") { push(head[1]!, i, close + 1); i = close + 1; last = "operand"; continue; }
      i += 1; last = "other"; continue;
    }
    if (clickhouse && ch === ":" && next !== ":") {
      // A `:name` is a parameter only where a cast types it, and never after a `.`, as a typed JSON path writes one.
      const m = next !== undefined && /[A-Za-z_]/.test(next) && !(i > 0 && /[A-Za-z0-9_.]/.test(sql[i - 1]!)) ? /^[A-Za-z_][A-Za-z0-9_]*(?=::[A-Za-z_])/.exec(sql.slice(i + 1)) : null;
      if (m) { push(m[0], i, i + 1 + m[0].length); i += 1 + m[0].length; last = "operand"; continue; }
      i += 1; last = "other"; continue;
    }
    if (ch === ":" && next === ":") {
      // A `::` cast: the type name after it is not a parameter.
      const m = /^[A-Za-z_][A-Za-z0-9_]*/.exec(sql.slice(i + 2));
      i += 2 + (m ? m[0].length : 0); last = "operand"; continue;
    }
    // A colon that opens a Postgres subscript is a slice with no lower bound, since no expression
    // starts with a colon there; an array constructor's `[` follows `array` or another `[`.
    if (ch === ":" && last === "subscript" && dialect === "postgres") { i += 1; last = "other"; continue; }
    // `arr[lo:hi]` is a slice: a name character before the colon means it opens nothing. SQLite's
    // own `@name` and `$name` are named placeholders there too.
    if ((ch === ":" || (dialect === "sqlite" && (ch === "@" || ch === "$"))) && next !== undefined && /[A-Za-z_]/.test(next) && !(i > 0 && /[A-Za-z0-9_]/.test(sql[i - 1]!))) {
      const m = /^[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*/.exec(sql.slice(i + 1))!;
      push(m[0], i, i + 1 + m[0].length); i += 1 + m[0].length; last = "operand"; continue;
    }
    if (ch === "$" && dialect === "postgres" && next !== undefined && /[0-9]/.test(next)) {
      const m = /^[0-9]+/.exec(sql.slice(i + 1))!;
      push(position(m[0]), i, i + 1 + m[0].length); i += 1 + m[0].length; last = "operand"; continue;
    }
    if (ch === "?" && dialect === "sqlite" && next !== undefined && /[0-9]/.test(next)) {
      // SQLite's `?NNN` names the same position a `$NNN` does.
      const m = /^[0-9]+/.exec(sql.slice(i + 1))!;
      push(position(m[0]), i, i + 1 + m[0].length); i += 1 + m[0].length; last = "operand"; continue;
    }
    // ClickHouse's `?` is its conditional operator.
    if (ch === "?" && dialect !== "postgres" && !clickhouse) { largest += 1; push(String(largest), i, i + 1); i += 1; last = "operand"; continue; }
    if (/\s/.test(ch)) { i += 1; continue; }
    if (/[A-Za-z0-9_]/.test(ch)) {
      // A name or a number, whole: `array` opens a constructor, anything else is an operand.
      const m = /^[A-Za-z0-9_]+/.exec(sql.slice(i))!;
      last = m[0].toLowerCase() === "array" ? "other" : "operand"; i += m[0].length; continue;
    }
    last = ch === ")" || ch === "]" || (ch === "$" && i > 0 && /[A-Za-z0-9_]/.test(sql[i - 1]!)) ? "operand" : ch === "[" && last === "operand" ? "subscript" : "other";
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

/** The index of the `}` that closes a ClickHouse placeholder whose type starts at `from`, or -1. */
function bracesEnd(sql: string, from: number): number {
  let depth = 0;
  let i = from;
  while (i < sql.length) {
    const ch = sql[i]!;
    if (ch === "'" || ch === '"' || ch === "`") { const end = skipQuoted(sql, i, ch, true); if (end > sql.length || sql[end - 1] !== ch || end - 1 === i) return -1; i = end; continue; }
    if (ch === "(") depth += 1;
    else if (ch === ")") { if (depth === 0) return -1; depth -= 1; }
    else if (ch === "}" && depth === 0) return i;
    else if (ch === "{" || ch === "}" || ch === ";") return -1;
    i += 1;
  }
  return -1;
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
