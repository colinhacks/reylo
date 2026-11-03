// What a statement's module declares, read off its analysis: the diagnostics the type check places
// in the `.sql` file, the Params and Row types and the row count. The content mapper (src/mapper.ts)
// writes these into the module it hands TypeScript; the docs site's playground shows them as they
// are. It imports nothing from Node, so a browser can run it beside the engine.
import type { Analysis, Dialect, Lexed } from "./engine.ts";
import { docComment } from "./doc-comment.ts";
import { analysisWithBases } from "./engine-type.ts";
import { ident, lastOfEachName, orNull, renderParamType, renderType } from "./render.ts";

/** A diagnostic of the `.sql` file: its range in UTF-8 bytes, the encoding the content mapper chose. */
export type FileDiagnostic = { message: string; start: number; length: number; code: number };

export type ModuleTypes = {
  diagnostics: FileDiagnostic[];
  /** `{ id: number }`, `Record<never, never>` for none, or `unknown` where the statement failed. */
  params: string;
  /** `{ id: number; name: string }` (a field per line, after its doc comment, where a column's comment documents one), `never` for no columns, or `unknown` where the statement failed. */
  row: string;
  /** How many rows the statement returns, as a module's `Query` names it. */
  cardinality: "one" | "maybeOne" | "many" | "none";
  /** The parameters' names in order of first use: the module's `params` at run time. */
  names: string[];
  /** The type of the error `.try()` gives: the module's `QueryError`. */
  queryError: string;
  /** The Standard Query's `columns`, one entry per output column in order: `readonly [{ readonly name: "id"; decode(value: string): number }]`; undefined where the statement failed. */
  columns?: string;
};

/**
 * The type of the error `.try()` gives for a statement: where it runs only its database's own
 * code, narrowed to the names each code's fields may hold (engine/core/src/errors.rs), a SQLite
 * unique key's columns written as SQLite's message lists them, `a, b`; the dialect's whole error
 * type otherwise.
 */
export function queryErrorType(dialect: Dialect, errors: Analysis["errors"]): string {
  if (!errors) return `import("reylo").QueryError<${JSON.stringify(dialect)}>`;
  const name = { postgres: "PostgresQueryError", mysql: "MysqlQueryError", sqlite: "SqliteQueryError", duckdb: "DuckdbQueryError" }[dialect];
  const names = (field: string, list: string[]) => list.map((n) => (field === "columns" ? `readonly [${n.split(", ").map((c) => JSON.stringify(c)).join(", ")}]` : JSON.stringify(n))).join(" | ");
  const codes = Object.entries(errors).map(([code, fields]) => `  ${JSON.stringify(code)}: { ${Object.entries(fields).map(([field, list]) => `${field}: ${names(field, list)}`).join("; ")} };`);
  return codes.length ? `import("reylo").${name}<{\n${codes.join("\n")}\n}>` : `import("reylo").${name}<{}>`;
}

/** A parameter's TypeScript type: `unknown` when nothing in the statement types it (a CAST does), with `| null` where the statement lets the caller pass null. */
function paramType(q: Analysis["params"][number], rendered: string): string {
  if (q.type.kind === "unknown") return "unknown";
  return q.nullable ? orNull(rendered) : rendered;
}

/**
 * The module's import from `reylo`: `Query`, and `Json` where one of `types` names it. Comments,
 * string literal types and a key named `Json` are left out of the search, so that the module never
 * imports a name it does not use. Strings and comments are read in one pass, from the left, so a
 * comment's delimiter inside a quoted name (`"/*"`) starts no comment, nor a quote inside a comment
 * a string.
 */
export function reyloImport(...types: string[]): string {
  const text = types.join("\n").replace(/"(?:[^"\\]|\\.)*"|\/\*[\s\S]*?\*\//g, (m) => (m.startsWith('"') ? '""' : ""));
  return `import type { ${/(?<![\w$])Json(?![\w$]|\s*\??:)/.test(text) ? "Json, " : ""}Query } from "reylo";`;
}

/**
 * A column's comment as the doc comment of the field that reads it, for an editor's hover: on one
 * line where the comment has one, else a block of lines after `indent` (src/doc-comment.ts), and
 * nothing where there is none.
 */
export function fieldComment(comment: string | undefined, indent = ""): string {
  const text = (comment ?? "").replace(/\r\n?/g, "\n").replace(/[ \t]+$/gm, "").trim();
  if (!text) return "";
  if (!text.includes("\n")) return `/** ${text.replaceAll("*/", "*\\/")} */`;
  return docComment(text, undefined, "", indent).trim();
}

/**
 * An object type of `fields`, each a field's text and its column's comment: on one line, or a field
 * per line, each after its doc comment, where any has one. TypeScript reads a doc comment only where
 * a line starts with it.
 */
function objectType(fields: (readonly [text: string, comment: string | undefined])[]): string {
  const docs = fields.map(([, comment]) => fieldComment(comment, "  "));
  if (!docs.some(Boolean)) return `{ ${fields.map(([text]) => text).join("; ")} }`;
  return `{\n${fields.map(([text], i) => `${docs[i] ? `  ${docs[i]}\n` : ""}  ${text};`).join("\n")}\n}`;
}

const utf8 = new TextEncoder();
const fromUtf8 = new TextDecoder();

/**
 * A diagnostic's range kept inside a text of `size` UTF-8 bytes. TypeScript refuses a content
 * mapper's whole transform when a diagnostic runs past the end of the file ("returned an invalid
 * transform response"), and the module becomes `any`; a range that does is cut at the end.
 */
export function withinFile(start: number, length: number, size: number): { start: number; length: number } {
  const from = Math.max(0, Math.min(start, size));
  return { start: from, length: Math.max(0, Math.min(length, size - from)) };
}

/**
 * The module's types for a statement the engine analyzed. `lexed` is the engine's lexer's reading of
 * the statement, which names the parameters (absent where the lexer refused it); `strictParams` is
 * the project file's `strict_params`, and `projectFile` the file the error about it names: the
 * project's own file, which may be `.reylo.yml` or another path REYLO_CONFIG names.
 */
export function moduleTypes(sql: string, written: Analysis, dialect: Dialect, lexed: Pick<Lexed, "params" | "placeholders"> | undefined, strictParams: boolean, projectFile = ".reylo.yaml"): ModuleTypes {
  const analysis = analysisWithBases(written);
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
  const cardinality = analysis.cardinality === "at_most_one" ? "maybeOne" : analysis.cardinality;
  if (analysis.diagnostics.some((d) => d.code === 6) || (dialect === "postgres" && analysis.kind === undefined)) return { diagnostics, params: "unknown", row: "unknown", cardinality, names, queryError: queryErrorType(dialect, undefined) };
  const columns = lastOfEachName(analysis.columns).map((c) => [`${ident(c.name)}: ${c.nullable ? orNull(renderType(c.type, false, dialect)) : renderType(c.type, false, dialect)}`, c.comment] as const);
  // A parameter is typed from its context (a column it is compared with or written to, a CAST);
  // one no context types is `unknown`.
  const typed = analysis.params.map((q) => [q.name, paramType(q, renderParamType(q.type, dialect, false, q.members)), q.comment] as const);
  // A parameter that takes null may be left out, which binds NULL; so may one the engine says the
  // caller may leave out, a typed JSON parameter whose schema has a `default`.
  const optional = new Set(analysis.params.filter((q) => q.nullable || q.optional).map((q) => q.name));
  const paramTypes = typed.map(([name, type, comment]) => [`${ident(name)}${optional.has(name) ? "?" : ""}: ${type}`, comment] as const);
  return {
    diagnostics,
    params: paramTypes.length ? objectType(paramTypes) : "Record<never, never>",
    row: columns.length ? objectType(columns) : "never",
    cardinality,
    names,
    queryError: queryErrorType(dialect, analysis.errors),
    columns: standardColumnsType(analysis.columns, dialect),
  };
}

/**
 * The type of a module's Standard Query `columns`: one entry per output column, in order, whose
 * decoder takes the column's raw value, never NULL, and gives the column's type without null.
 */
export function standardColumnsType(columns: Analysis["columns"], dialect: Dialect): string {
  const raw = dialect === "postgres" ? "string" : dialect === "duckdb" ? "unknown" : "string | number | bigint | Uint8Array";
  return `readonly [${columns.map((c) => `{ readonly name: ${JSON.stringify(c.name)}; decode(value: ${raw}): ${renderType(c.type, false, dialect)} }`).join(", ")}]`;
}
