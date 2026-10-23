// The doc comments of a statement's module: the comment written at the top of the `.sql` file as
// prose, then the statement in a `sql` code block, so an editor's hover over the import (or over
// its `.sql`) shows them as it shows any declaration's JSDoc. It imports nothing from Node.
import type { Dialect } from "./engine.ts";

/** The comments before a statement, as prose, and the offset where they end. */
export type Leading = { prose: string; end: number };

/**
 * The `--` lines and `/* … *\/` blocks before the statement's first token, markers stripped and
 * lines kept: a blank line between two comments, or a change from one form to the other, starts a
 * paragraph. Read as the dialect reads comments: Postgres nests block comments, MySQL's `--` is one
 * only before a space or a control character and `#` starts one too, and MySQL's `/*!` and `/*+`
 * are code to the server, so they end the prose. With no dialect (no project), `--` and `/* *\/`.
 */
export function leadingComment(sql: string, dialect?: Dialect): Leading {
  const paragraphs: string[][] = [];
  let lines: string[] | undefined;
  let last: "line" | "block" | undefined;
  let i = 0;
  let end = 0;
  for (;;) {
    const ws = /^\s*/.exec(sql.slice(i))![0];
    const at = i + ws.length;
    const ch = sql[at];
    const next = sql[at + 1];
    const blankBefore = (ws.match(/\n/g)?.length ?? 0) > 1;
    const lineComment = (ch === "-" && next === "-" && !(dialect === "mysql" && at + 2 < sql.length && sql.charCodeAt(at + 2) > 32 && sql.charCodeAt(at + 2) !== 127)) || (ch === "#" && dialect === "mysql");
    if (lineComment) {
      const stop = sql.indexOf("\n", at);
      const text = sql.slice(at + (ch === "#" ? 1 : 2), stop === -1 ? sql.length : stop);
      if (!lines || last !== "line" || blankBefore) paragraphs.push((lines = []));
      lines.push(text);
      last = "line";
      i = end = stop === -1 ? sql.length : stop;
      continue;
    }
    if (ch === "/" && next === "*" && !(dialect === "mysql" && (sql[at + 2] === "!" || sql[at + 2] === "+"))) {
      let depth = 1;
      let j = at + 2;
      while (j < sql.length && depth > 0) {
        if (sql.startsWith("*/", j)) { depth--; j += 2; }
        else if (dialect === "postgres" && sql.startsWith("/*", j)) { depth++; j += 2; }
        else j++;
      }
      const text = sql.slice(at + 2, depth === 0 ? j - 2 : sql.length);
      paragraphs.push((lines = block(text)));
      last = "block";
      i = end = j;
      continue;
    }
    break;
  }
  const prose = paragraphs.map(dedent).filter((p) => p.length).map((p) => p.join("\n")).join("\n\n");
  return { prose, end };
}

/** A block comment's lines, without a `/**`'s extra stars or the `*` that starts each line of one written as JSDoc is. */
function block(text: string): string[] {
  const lines = text.replace(/^\*+/, "").split(/\r?\n/);
  const rest = lines.slice(1).filter((l) => l.trim().length);
  if (rest.length && rest.every((l) => /^\s*\*/.test(l))) return [lines[0]!, ...lines.slice(1).map((l) => l.replace(/^\s*\*(?!\/) ?/, ""))];
  return lines;
}

/** Lines without their trailing space, the leading and trailing blank ones, and the indentation they share. */
function dedent(lines: string[]): string[] {
  const trimmed = lines.map((l) => l.replace(/\s+$/, ""));
  while (trimmed.length && !trimmed[0]) trimmed.shift();
  while (trimmed.length && !trimmed.at(-1)) trimmed.pop();
  const indent = Math.min(...trimmed.filter((l) => l).map((l) => /^[ \t]*/.exec(l)![0].length));
  return trimmed.map((l) => l.slice(Math.min(indent, /^[ \t]*/.exec(l)![0].length)));
}

/** The statement after its leading comments, its blank edges trimmed. */
export function statementAfter(sql: string, leading: Leading): string {
  return sql.slice(leading.end).trim();
}

/** What `sql` is, after the statement in its doc comment. */
export const sqlNote = (commented: boolean) =>
  `The statement as Reylo sends it to the database, as the module's \`sql\` export: \`:id\` is \`$1\` on Postgres and \`?\` on MySQL and SQLite, ${commented ? "each cast is the dialect's own, and a comment naming the file ends it" : "and each cast is the dialect's own"}.`;
/** The note where the project's statements end with their file's sqlcommenter comment, as they do unless it says `sqlcommenter: false`. */
export const SQL_NOTE = sqlNote(true);
/** What `raw` is, after the statement in its doc comment. */
export const RAW_NOTE = "The `.sql` file's contents, exactly as written, as the module's `raw` export.";

/**
 * A JSDoc comment, indented by `indent`: the prose, then `sql` in a fenced `sql` block, then
 * `note`, each a paragraph where it is given. A star before a slash would end the comment, so a
 * backslash goes between them; the fence is longer than any run of backticks in the statement, and
 * a fence the prose leaves open is closed before the block. Empty when there is nothing to say.
 */
export function docComment(prose: string, sql: string | undefined, note = "", indent = ""): string {
  const parts: string[] = [];
  if (prose) {
    const open = openFence(prose);
    parts.push(open ? `${prose}\n${open}` : prose);
  }
  if (sql) {
    const longest = Math.max(0, ...(sql.match(/`+/g) ?? []).map((run) => run.length));
    const fence = "`".repeat(Math.max(3, longest + 1));
    parts.push(`${fence}sql\n${sql}\n${fence}`);
  }
  if (note) parts.push(note);
  if (!parts.length) return "";
  const body = parts.join("\n\n").replaceAll("*/", "*\\/").split("\n");
  return [`${indent}/**`, ...body.map((l) => (l ? `${indent} * ${l}` : `${indent} *`)), `${indent} */`, ""].join("\n");
}

/** The fence a Markdown text opens and never closes, which would take in what follows it. */
function openFence(text: string): string | undefined {
  let open: string | undefined;
  for (const line of text.split("\n")) {
    const m = /^ {0,3}(`{3,}|~{3,})/.exec(line);
    if (!m) continue;
    if (!open) open = m[1];
    else if (m[1]![0] === open[0] && m[1]!.length >= open.length && !line.slice(m[0].length).trim()) open = undefined;
  }
  return open;
}
