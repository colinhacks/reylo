// One statement as the playground shows it, in `reylo play` and on the docs site's /play: the
// diagnostics the content mapper would place in its file, by line and column, and its Params and
// Returns as the docs print them. Free of Node, since the docs run it in the reader's tab.
import type { Analysis, Dialect, Lexed } from "./engine-core.ts";
import { moduleTypes } from "./module-types.ts";
import { formatType, resultType } from "./type-format.ts";

export type PlayDiagnostic = { message: string; code: number; line: number; column: number; endLine: number; endColumn: number };

export type PlayResult = {
  sql: string;
  /** `{ id: number }`; absent where the statement takes no parameters. */
  params?: string;
  /** What `.run()` resolves to. */
  returns: string;
  /** One row's type, as the query's `element`: what `returns` holds one or many of, by `cardinality`. */
  element?: string;
  /** How many rows it returns: `many`, `one`, `maybeOne` or `none`; absent where it cannot be analyzed. */
  cardinality?: string;
  diagnostics: PlayDiagnostic[];
  /** How long the analysis took, in milliseconds. */
  ms: number;
};

/**
 * The statement's result from its analysis, or from the error that stopped one, which is placed as
 * the content mapper places it: at the file's start. Types are broken over lines wider than `width`.
 */
export function playResult(
  sql: string,
  dialect: Dialect,
  outcome: { analysis: Analysis; lexed?: Pick<Lexed, "params" | "placeholders"> | undefined } | { error: { message: string; code?: number } },
  options: { strictParams?: boolean; width?: number; ms?: number } = {},
): PlayResult {
  const place = placer(sql);
  const width = options.width ?? Infinity;
  const ms = options.ms ?? 0;
  const placed = (d: { message: string; start: number; length: number; code: number }) => ({ message: d.message, code: d.code, ...place(d.start, d.length) });
  if ("error" in outcome) return { sql, params: "unknown", returns: "unknown", diagnostics: [placed({ message: outcome.error.message, start: 0, length: 1, code: outcome.error.code ?? 5 })], ms };
  try {
    const types = moduleTypes(sql, outcome.analysis, dialect, outcome.lexed, options.strictParams ?? true);
    return {
      sql,
      ...(types.params === "Record<never, never>" ? {} : { params: formatType(types.params, width) }),
      returns: formatType(types.row === "unknown" ? "unknown" : resultType(types.row, types.cardinality), width),
      element: formatType(types.row, width),
      cardinality: types.cardinality,
      diagnostics: types.diagnostics.map(placed),
      ms,
    };
  } catch (e) {
    return { sql, params: "unknown", returns: "unknown", diagnostics: [placed({ message: (e as Error).message, start: 0, length: 1, code: 5 })], ms };
  }
}

/**
 * For a statement, a function from a UTF-8 byte range, as the engine and the content mapper give
 * ranges, to its 1-based line and column range in UTF-16 units, as an editor counts.
 */
export function placer(sql: string) {
  const bytes = new TextEncoder().encode(sql);
  // index[b] is the UTF-16 index of the character byte b begins or sits inside.
  const index = new Uint32Array(bytes.length + 1);
  let u = 0;
  for (let b = 0; b < bytes.length; ) {
    const lead = bytes[b]!;
    const n = lead < 0x80 ? 1 : lead < 0xe0 ? 2 : lead < 0xf0 ? 3 : 4;
    for (let k = 0; k < n && b + k < bytes.length; k++) index[b + k] = u;
    b += n;
    u += n === 4 ? 2 : 1;
  }
  index[bytes.length] = sql.length;
  const lineStarts = [0];
  for (let i = 0; i < sql.length; i++) if (sql.charCodeAt(i) === 10) lineStarts.push(i + 1);
  const at = (i: number) => {
    let lo = 0;
    let hi = lineStarts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (lineStarts[mid]! <= i) lo = mid;
      else hi = mid - 1;
    }
    return { line: lo + 1, column: i - lineStarts[lo]! + 1 };
  };
  const clamp = (byte: number) => index[Math.max(0, Math.min(byte, bytes.length))]!;
  return (start: number, length: number) => {
    const a = at(clamp(start));
    const b = at(clamp(start + length));
    return { line: a.line, column: a.column, endLine: b.line, endColumn: b.column };
  };
}
