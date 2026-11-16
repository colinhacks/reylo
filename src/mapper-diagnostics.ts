// How the content mapper reports a `.sql` file's diagnostics to TypeScript. A transform's own
// `diagnostics` are syntax errors of the file, and `tsc` reports a program's type errors only when
// it has no syntax error, so one error in a `.sql` file would hide every type error of the project
// on the command line. Each diagnostic goes instead as an expect directive over an empty range of
// the module: no TypeScript diagnostic can land in an empty range, so the directive is never used,
// and TypeScript reports its message, under its code, at its place in the file, among the type
// errors. The language server shows it either way. It imports nothing, so src/mapper.ts can load it
// without the build-time half.

/** A diagnostic of the `.sql` file: its range in the file, in UTF-8 bytes, and its code (`reylo<code>`). */
export type MapperDiagnostic = { messageText: string; start: number; length: number; code: number };

/** The content mapper protocol's `diagnosticDirectives` tuple: `[originalStart, originalLength, virtualStart, virtualEnd, policy, unusedExpectDirectiveIndex]`. */
type Directive = [number, number, number, number, number, number];

const EXPECT = 1;

/** What a transform answers for its file's diagnostics: no `diagnostics`, and a directive per diagnostic. */
export function reportDiagnostics(diagnostics: readonly MapperDiagnostic[]): { diagnosticDirectives?: { unusedExpectDirectiveDiagnostics: { code: number; messageText: string }[]; directives: Directive[] } } {
  if (diagnostics.length === 0) return {};
  return {
    diagnosticDirectives: {
      unusedExpectDirectiveDiagnostics: diagnostics.map(({ code, messageText }) => ({ code, messageText })),
      directives: diagnostics.map((d, i) => [d.start, d.length, 0, 0, EXPECT, i]),
    },
  };
}

/** The part of a transform's answer that reports diagnostics, as a client reads it. */
export type Reported = { diagnostics?: readonly MapperDiagnostic[]; diagnosticDirectives?: { unusedExpectDirectiveDiagnostics: { code: number; messageText: string }[]; directives: number[][] } };

/** The diagnostics a transform's answer reports, read back from its directives, for a client that asks the mapper itself. */
export function reportedDiagnostics(result: Reported): MapperDiagnostic[] {
  const { unusedExpectDirectiveDiagnostics: messages = [], directives = [] } = result.diagnosticDirectives ?? {};
  const expected = directives.filter((d) => d[4] === EXPECT).map(([start, length, , , , index]) => ({ ...messages[index ?? 0]!, start: start!, length: length! }));
  return [...(result.diagnostics ?? []), ...expected];
}
