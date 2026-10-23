// A diagnostic's place in the statement as an editor counts it. The engine and the content mapper
// give ranges in UTF-8 bytes; an editor counts UTF-16 units, by 1-based line and column.

/** For a statement, a function from a UTF-8 byte range to its 1-based line and column range in UTF-16 units. */
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
