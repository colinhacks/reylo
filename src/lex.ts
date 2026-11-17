// Placeholder lexing for the runtime loader: finds `:name` (and SQLite's `@name` and `$name`, and
// SQL Server's `@name`), and the dialect's own positional form (`$1` on Postgres, `?` on MySQL, `?` and `?NNN` on SQLite),
// outside strings and comments, and returns the binding order. A name may be dotted
// (`:auth.user_id`); each segment starts like a name, so `arr[1:2]` is a slice, and on Postgres a
// colon that opens a subscript is a slice with no lower bound (`tags[:n]`). MySQL text is read
// as a session in the default `sql_mode` reads it: a backslash in a string escapes the character
// after it, `"` quotes a string, `#` starts a line comment, and `--` does only before a space or
// a control character. SQL Server's `[name]` is a quoted name, in which `]]` is one `]`, its `@name`
// a name as T-SQL writes one (no dots; `@`, `#` and `$` may follow its first letter), its `@@name` a
// function, and it has no `?`. This is the provisional JavaScript version of the engine's lexer; the Rust
// engine owns the authoritative one (dollar quoting, E'' strings, nested comments, array slices,
// MySQL's other modes).
export type Placeholder = { name: string; start: number; end: number };

// A name drops its prefix (`:id` is `id`); a positional placeholder is named by its number
// (`$2` and `?2` are `2`), and a bare `?` takes the number after the largest so far.
export function lexPlaceholders(sql: string, dialect: "postgres" | "mysql" | "sqlite" | "sqlserver"): { params: string[]; placeholders: Placeholder[] } {
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
  // What was read last, past whitespace and comments: an operand (a name other than `array`, a
  // number, a quoted name, a placeholder, `)` or `]`), the `[` of a subscript after one, or other.
  let last: "other" | "operand" | "subscript" = "other";
  // A SQL Server batch's own variables and an `exec`'s argument names, read once where an `@name` is met.
  let tsql: { declared: Set<string>; args: Set<number> } | undefined;
  while (i < sql.length) {
    const ch = sql[i]!;
    const next = sql[i + 1];
    if (ch === "'" || (ch === '"' && mysql)) { i = skipQuoted(sql, i, ch, mysql); last = "other"; continue; }
    if (ch === '"' || ch === "`") { i = skipQuoted(sql, i, ch, false); last = "operand"; continue; }
    // SQLite's `[name]` is a quoted identifier, which ends at the first `]`.
    if (ch === "[" && dialect === "sqlite") { const end = sql.indexOf("]", i); i = end === -1 ? sql.length : end + 1; last = "operand"; continue; }
    // SQL Server's `[name]`, in which `]]` is one `]`.
    if (ch === "[" && dialect === "sqlserver") { i = skipQuoted(sql, i, "]", false); last = "operand"; continue; }
    // MySQL's `--` is two minus signs before anything but a space or a control character.
    const comment = ch === "#" ? mysql : ch === "-" && next === "-" && !(mysql && i + 2 < sql.length && sql.charCodeAt(i + 2) > 32 && sql.charCodeAt(i + 2) !== 127);
    if (comment) { i = sql.indexOf("\n", i); if (i === -1) break; continue; }
    if (ch === "/" && next === "*") { const end = sql.indexOf("*/", i + 2); i = end === -1 ? sql.length : end + 2; continue; }
    if (ch === ":" && next === ":") {
      // A `::` cast: the type name after it is not a parameter.
      const m = /^[A-Za-z_][A-Za-z0-9_]*/.exec(sql.slice(i + 2));
      i += 2 + (m ? m[0].length : 0); last = "operand"; continue;
    }
    // A colon that opens a Postgres subscript is a slice with no lower bound, since no expression
    // starts with a colon there; an array constructor's `[` follows `array` or another `[`.
    if (ch === ":" && last === "subscript" && dialect === "postgres") { i += 1; last = "other"; continue; }
    if (ch === "@" && dialect === "sqlserver") {
      // SQL Server's `@@rowcount` and its like are functions.
      if (next === "@") { i += 2 + /^[A-Za-z0-9_]*/.exec(sql.slice(i + 2))![0].length; last = "operand"; continue; }
      if (next !== undefined && /[A-Za-z_]/.test(next) && !(i > 0 && /[A-Za-z0-9_]/.test(sql[i - 1]!))) {
        const m = /^[A-Za-z_][A-Za-z0-9_@#$]*/.exec(sql.slice(i + 1))!;
        // A variable the batch declares, or an argument's name in an `exec`, is T-SQL's own.
        tsql ??= tsqlVariables(sql);
        if (!tsql.declared.has(m[0].toLowerCase()) && !tsql.args.has(i)) push(m[0], i, i + 1 + m[0].length);
        i += 1 + m[0].length; last = "operand"; continue;
      }
    }
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
    if (ch === "?" && (dialect === "mysql" || dialect === "sqlite")) { largest += 1; push(String(largest), i, i + 1); i += 1; last = "operand"; continue; }
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
/** The words that start a T-SQL statement, which end a `declare`'s list or an `exec`'s arguments where no `;` does. */
const TSQL_STARTS = new Set(["select", "insert", "update", "delete", "merge", "set", "if", "else", "while", "exec", "execute", "begin", "end", "declare", "return", "print", "throw", "raiserror", "with", "open", "fetch", "close", "deallocate", "waitfor", "goto", "break", "continue", "truncate", "create", "alter", "drop", "commit", "rollback", "save", "use"]);

/**
 * The names a T-SQL batch gives `@` that are no placeholder (the engine's `tsql_variables` in
 * engine/core/src/lex.rs): the variables it declares, lowercased, and the offsets of the argument
 * names an `exec` passes by name.
 */
function tsqlVariables(sql: string): { declared: Set<string>; args: Set<number> } {
  const declared = new Set<string>();
  const args = new Set<number>();
  let mode: "none" | "declare" | "exec" = "none";
  let expect = false;
  let depth = 0;
  // The `case` expressions open at the top level, whose `else` and `end` end no statement.
  let cases = 0;
  let i = 0;
  while (i < sql.length) {
    const ch = sql[i]!;
    if (ch === "'" || ch === '"' || ch === "[") {
      const close = ch === "[" ? "]" : ch;
      i += 1;
      while (i < sql.length) {
        if (sql[i] === close) { if (sql[i + 1] === close) { i += 2; continue; } break; }
        i += 1;
      }
      i += 1; continue;
    }
    if (ch === "-" && sql[i + 1] === "-") { const end = sql.indexOf("\n", i); i = end === -1 ? sql.length : end; continue; }
    if (ch === "/" && sql[i + 1] === "*") {
      let nest = 0;
      while (i < sql.length) {
        if (sql[i] === "/" && sql[i + 1] === "*") { nest += 1; i += 2; }
        else if (sql[i] === "*" && sql[i + 1] === "/") { nest -= 1; i += 2; if (nest === 0) break; }
        else i += 1;
      }
      continue;
    }
    if (ch === "(") { depth += 1; i += 1; continue; }
    if (ch === ")") { depth = Math.max(0, depth - 1); i += 1; continue; }
    if (ch === ";" && depth === 0) { mode = "none"; cases = 0; i += 1; continue; }
    if (ch === "," && depth === 0) { if (mode === "declare") expect = true; i += 1; continue; }
    if (ch === "@" && sql[i + 1] === "@") { i += 2; continue; }
    if (ch === "@" && /[A-Za-z_]/.test(sql[i + 1] ?? "")) {
      const name = /^[A-Za-z_][A-Za-z0-9_@#$]*/.exec(sql.slice(i + 1))![0];
      const end = i + 1 + name.length;
      if (depth === 0 && mode === "declare" && expect) { declared.add(name.toLowerCase()); expect = false; }
      else if (depth === 0 && mode === "exec" && /^\s*=(?!=)/.test(sql.slice(end))) args.add(i);
      i = end; continue;
    }
    if (/[A-Za-z_]/.test(ch) && !(i > 0 && /[A-Za-z0-9_]/.test(sql[i - 1]!))) {
      const word = /^[A-Za-z_][A-Za-z0-9_]*/.exec(sql.slice(i))![0];
      const lower = word.toLowerCase();
      if (depth === 0) {
        if (lower === "case") cases += 1;
        else if (lower === "end" && cases > 0) cases -= 1;
        else if (cases === 0 && TSQL_STARTS.has(lower)) {
          mode = lower === "declare" ? "declare" : lower === "exec" || lower === "execute" ? "exec" : "none";
          expect = mode === "declare";
        }
      }
      i += word.length; continue;
    }
    i += 1;
  }
  return { declared, args };
}

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
