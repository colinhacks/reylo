// What a statement's module declares, read off its analysis: the diagnostics the type check places
// in the `.sql` file, the Params and Row types and the row count. The content mapper (src/mapper.ts)
// writes these into the module it hands TypeScript; the docs site's playground shows them as they
// are. It imports nothing from Node, so a browser can run it beside the engine.
import type { Analysis, Dialect, Lexed } from "./engine.ts";
import { declaredType, ident, lastOfEachName, renderParamType, renderType } from "./render.ts";

/** A diagnostic of the `.sql` file: its range in UTF-8 bytes, the encoding the content mapper chose. */
export type FileDiagnostic = { message: string; start: number; length: number; code: number };

export type ModuleTypes = {
  diagnostics: FileDiagnostic[];
  /** `{ id: number }`, `Record<never, never>` for none, or `unknown` where the statement failed. */
  params: string;
  /** `{ id: number; name: string }`, `never` for no columns, or `unknown` where the statement failed. */
  row: string;
  cardinality: Analysis["cardinality"];
  /** The parameters' names in order of first use: the module's `params` at run time. */
  names: string[];
};

/** A parameter's TypeScript type: `unknown` when nothing in the statement types it (a CAST does), with `| null` where the statement lets the caller pass null. */
function paramType(q: Analysis["params"][number], rendered: string): string {
  if (q.type.kind === "unknown") return "unknown";
  return q.nullable ? `${rendered} | null` : rendered;
}

const utf8 = new TextEncoder();
const fromUtf8 = new TextDecoder();

/**
 * The module's types for a statement the engine analyzed. `lexed` is the engine's lexer's reading of
 * the statement, which names the parameters (absent where the lexer refused it); `strictParams` is
 * the project file's `strict_params`, and `projectFile` the file the error about it names: the
 * project's own file, which may be `.reylo.yml` or another path REYLO_CONFIG names.
 */
export function moduleTypes(sql: string, analysis: Analysis, dialect: Dialect, lexed: Pick<Lexed, "params" | "placeholders"> | undefined, strictParams: boolean, projectFile = ".reylo.yaml"): ModuleTypes {
  let names = lexed?.params ?? [];
  let placeholders = lexed?.placeholders ?? [];
  // A MySQL session reads its strings by its sql_mode, which the analysis asked for and the
  // lexer alone does not know: the placeholders are the ones the analysis read.
  if (dialect === "mysql") {
    names = analysis.params.map((q) => q.name);
    placeholders = analysis.rewritten.spans.map(([start, end], k) => ({ name: analysis.rewritten.order[k]!, start, end }));
  }
  const diagnostics: FileDiagnostic[] = analysis.diagnostics.map((d) => ({ message: d.message, start: d.start, length: d.length, code: d.code }));
  // A parameter nothing in the statement types is an error at its first use, unless the project
  // file says `strict_params: false` (the default is true since 2026-10-20). A statement the engine
  // already refused (a parameter written twice, an error from the server) was not analysed to its
  // end, so its parameters are untyped for that reason: its own error is the one to fix first.
  if (strictParams && !analysis.diagnostics.length) {
    const bytes = utf8.encode(sql);
    for (const q of analysis.params) {
      if (q.type.kind !== "unknown") continue;
      const at = placeholders.find((h) => h.name === q.name);
      // The engine's offsets are UTF-8 bytes, as the diagnostics' are. A MySQL placeholder's span
      // takes in the `::type` the engine rewrote, which is no part of its name.
      const span = at ? fromUtf8.decode(bytes.subarray(at.start, at.end)) : `:${q.name}`;
      const written = span.replace(/\s*::[\s\S]*$/, "");
      const length = at ? utf8.encode(written).length : 1;
      const message = `Nothing in the statement gives the parameter ${written} a type; cast it, as in ${written}::text, or set strict_params: false in ${projectFile}.`;
      diagnostics.push({ message, start: at?.start ?? 0, length, code: 12 });
    }
  }
  const cardinality = analysis.cardinality;
  if (analysis.diagnostics.some((d) => d.code === 6) || (dialect === "postgres" && analysis.kind === "")) return { diagnostics, params: "unknown", row: "unknown", cardinality, names };
  const columns = lastOfEachName(analysis.columns).map((c) => `${ident(c.name)}: ${renderType(c.type, false, dialect)}${c.nullable ? " | null" : ""}`);
  // A parameter is typed from its context (a column it is compared with or written to, a CAST);
  // one no context types is `unknown`.
  const typed = analysis.params.map((q) => [q.name, paramType(q, declaredType(q.type, q.schema) ?? renderParamType(q.type, dialect))] as const);
  // A typed JSON parameter whose schema has a `default` may be left out (wiki/typed-json-params.md).
  const optional = new Set(analysis.params.filter((q) => q.schema !== null && typeof q.schema === "object" && "default" in q.schema).map((q) => q.name));
  const paramTypes = typed.map(([name, type]) => `${ident(name)}${optional.has(name) ? "?" : ""}: ${type}`);
  return {
    diagnostics,
    params: paramTypes.length ? `{ ${paramTypes.join("; ")} }` : "Record<never, never>",
    row: columns.length ? `{ ${columns.join("; ")} }` : "never",
    cardinality,
    names,
  };
}
