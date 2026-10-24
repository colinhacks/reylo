// The content mapper, which each engine package carries (src/tooling.ts) and src/mapper.ts starts:
// the type check's view of a `.sql` file. The analysis itself (the
// database, the engine, the `.reylo/` cache shared with the bundler plugins and `reylo generate`)
// is `src/analyze.ts`; this file renders it as types and places its diagnostics in the file.
import { createHash } from "node:crypto";
import { dirname, relative } from "node:path";
import { serve } from "./rpc.ts";
import { anyEngine, type Lexed } from "./engine.ts";
import { moduleTypes, queryErrorType, withinFile } from "./module-types.ts";
import { PROJECT_FILE, PROJECT_FILES, resolveProject, sourceName } from "./project.ts";
import { withSqlcommenter } from "./sqlcommenter.ts";
import { Session } from "./analyze.ts";
import { docComment, leadingComment, RAW_NOTE, sqlNote, statementAfter } from "./doc-comment.ts";

type Project = {
  strictParams: boolean;
  /** Whether each statement ends with a sqlcommenter comment naming its file. */
  sqlcommenter: boolean;
  /** The project file as its messages name it: relative to the project, or `.reylo.yaml` where there is none. */
  projectFile: string;
  session: Promise<Session>;
};
const projects = new Map<string, Project>();

/** A diagnostic of the file, its range kept inside the file's `size` bytes (`withinFile`). */
function diag(size: number, messageText: string, start: number, length: number, code = 1) {
  return { messageText, ...withinFile(start, length, size), code };
}

/**
 * The span map TypeScript reads as `mappings`: `[virtualStart, virtualLength, originalStart,
 * originalLength, kind, features]`, in UTF-8 bytes, the encoding `initialize` chose. One atom takes the
 * whole module to the statement, so go-to-definition on anything the module declares (the import's
 * path, the query, a column, a parameter) opens the `.sql` file at its statement. TypeScript maps a
 * definition only when its whole range lies in one segment, and the import's path goes to the range
 * of the module from its first token to its end, which a map by name would cut into pieces. No hover,
 * references or rename, which would land on the whole statement or write the TypeScript into it.
 */
function mapToStatement(text: string, sql: string): number[][] {
  const bytes = Buffer.from(sql, "utf8");
  let start = 0;
  let end = bytes.length;
  while (start < end && /\s/.test(String.fromCharCode(bytes[start]!))) start++;
  while (end > start && /\s/.test(String.fromCharCode(bytes[end - 1]!))) end--;
  const ATOM = 1;
  const DEFINITION = 8, TYPE_DEFINITION = 16, IMPLEMENTATION = 32;
  return [[0, Buffer.byteLength(text), start, end - start, ATOM, DEFINITION | TYPE_DEFINITION | IMPLEMENTATION]];
}

export type MapperHandlers = Record<string, (params: unknown) => unknown | Promise<unknown>>;

/** Answers TypeScript's content mapper protocol over this process's stdin and stdout. */
export function serveMapper(): void {
  serve(mapperHandlers());
}

/** The content mapper's methods, for an entry point that serves them (src/mapper.ts). */
export function mapperHandlers(): MapperHandlers {
  return {
    initialize(params) {
      // Builds from 2026-09-18 on send `protocolVersion: 1` and refuse a mapper that does not
      // answer with the version it speaks; the npm nightly of the same day does not send it yet.
      const { protocolVersion } = (params ?? {}) as { protocolVersion?: number };
      return { protocolVersion: protocolVersion ?? 1, positionEncoding: "utf-8", diagnosticSource: "reylo" };
    },
    async openProject(params) {
      const p = params as { projectHandle: string; options?: Record<string, unknown>; configFileName: string };
      const projectDir = dirname(p.configFileName);
      // Every setting lives in the project file, which PorgREST and Reylo Framework read too.
      const resolved = resolveProject(await anyEngine(projectDir), projectDir);
      const projectFile = resolved.file ? relative(projectDir, resolved.file) || resolved.file : PROJECT_FILE;
      const optionDiagnostics = Object.keys(p.options ?? {}).map((key) => ({ path: [key], messageText: `reylo reads its settings from ${projectFile}, not from the content mapper's options; move "${key}" there.`, code: 104 }));
      projects.set(p.projectHandle, { strictParams: resolved.strictParams, sqlcommenter: resolved.sqlcommenter, projectFile, session: Session.open(projectDir) });
      const configIdentity = createHash("sha1").update(JSON.stringify([resolved.file ?? null, resolved.dialect, resolved.strictParams, resolved.sqlcommenter, resolved.url ?? null, resolved.error ?? null, p.options ?? null])).digest("hex");
      return { configIdentity, watchedFiles: resolved.watched, optionDiagnostics };
    },
    async transform(params) {
      const p = params as { fileName: string; content: string; projectHandle: string };
      const sql = p.content;
      const size = Buffer.byteLength(sql);
      const diagnostics: ReturnType<typeof diag>[] = [];
      // A project file (`.reylo.yaml` or `.reylo.yml`, or a `<name>.reylo.yaml` that `include` takes
      // in) is no query, and its module declares nothing.
      if (PROJECT_FILES.some((name) => p.fileName.endsWith(name))) return { extension: ".ts" as const, diagnostics, text: "export {};\n", mappings: [] };
      const project = projects.get(p.projectHandle);
      if (!project) {
        // A transform for a project this process never opened: answer, never throw, so the file
        // keeps its default export instead of becoming an empty module.
        diagnostics.push(diag(size, `The content mapper has no project for this file; restart the TypeScript server.`, 0, 1, 9));
        const leading = leadingComment(sql);
        const doc = docComment(leading.prose, statementAfter(sql, leading));
        const text = `import type { Query } from "reylo";\nexport type Params = unknown;\nexport type Row = unknown;\nexport type QueryError = import("reylo").QueryError;\n${doc}declare const query: Query<unknown, unknown, "many", "postgres">;\nexport default query;\n`;
        return { extension: ".ts" as const, diagnostics, text, mappings: mapToStatement(text, sql) };
      }
      // The analysis first, and the dialect from the session after it: a session whose `direct_url`
      // command has just succeeded takes the project the command's URL resolves to (src/analyze.ts).
      const session = await project.session;
      const outcome = await session.analyze(sql);
      const { dialect } = session.project;
      // The engine's lexer names the parameters. A statement it cannot read (a placeholder form the
      // dialect would misread, named and positional mixed) comes back from the analysis as an error
      // at its place; until then nothing is proved, so the row count is `many`.
      let lexed: Lexed | undefined;
      try {
        lexed = session.lex(sql);
      } catch {}
      // `compiled` is the statement as Reylo sends it to the database (the engine's rewritten text),
      // which an analysis that failed has not given: then `sql` is any string. The comment at the top
      // of the file and the statement go into doc comments, so a hover over the import shows the
      // prose and the statement as written, and one over its `sql` the statement as sent. The
      // rewrite changes placeholders alone, never a comment, so the compiled text starts with the
      // same comments.
      const leading = leadingComment(sql, dialect);
      // `shown` is the statement the `sql` doc comment shows: the compiled text without its file's
      // comment, whose `*/` a doc comment cannot hold. The literal type carries it whole.
      const emit = (compiled: string | undefined, paramsType: string, rowType: string, cardinality: string, names: string[], shown = compiled, queryError = queryErrorType(dialect, undefined)) => {
        const written = statementAfter(sql, leading);
        const sent = shown === undefined ? undefined : statementAfter(shown, leadingComment(shown, dialect));
        const sqlDoc = (indent = "") => docComment(leading.prose, sent, sqlNote(project.sqlcommenter), indent);
        const rawDoc = (indent = "") => docComment(leading.prose, written, RAW_NOTE, indent);
        const text = [
          `import type { Query } from "reylo";`,
          `export type Params = ${paramsType};`,
          `export type Row = ${rowType};`,
          `export type QueryError = ${queryError};`,
          `${sqlDoc()}export declare const sql: ${compiled === undefined ? "string" : JSON.stringify(compiled)};`,
          `${rawDoc()}export declare const raw: ${JSON.stringify(sql)};`,
          `export declare const params: readonly [${names.map((n) => JSON.stringify(n)).join(", ")}];`,
          // The literal types go in the Query type directly rather than through the aliases, so a
          // hover shows the row's shape (`{ id: number; ... } | null`) and not `Row | null`. The two
          // texts go in by `typeof`, so a hover over the query names them and one over its `sql`
          // shows the statement, with the doc comment of the property here (Query's own `sql` and
          // `raw` carry none, since TypeScript would show it first and run the two together).
          `${docComment(leading.prose, written)}declare const query: Query<${paramsType}, ${rowType}, ${JSON.stringify(cardinality)}, ${JSON.stringify(dialect)}, QueryError> & {`,
          `${sqlDoc("  ")}  readonly sql: typeof sql;`,
          `${rawDoc("  ")}  readonly raw: typeof raw;`,
          // Every build step writes the file's path into the module, so an import's source is never
          // missing. Its text is not given: the build reads the root from the file's own project,
          // which may not be the project the type check opened.
          `  readonly source: string;`,
          `};`,
          `export default query;`,
          ``,
        ].join("\n");
        return { extension: ".ts" as const, diagnostics, text, mappings: mapToStatement(text, sql) };
      };

      if (!outcome.ok) {
        diagnostics.push(diag(size, outcome.message, 0, 1, outcome.code));
        return emit(undefined, "unknown", "unknown", "many", lexed?.params ?? []);
      }
      const { analysis } = outcome;
      // The types, the diagnostics the analysis gives and the untyped parameters' are one function
      // the docs site's playground shares (src/module-types.ts).
      const types = moduleTypes(sql, analysis, dialect, lexed, project.strictParams, project.projectFile);
      diagnostics.push(...types.diagnostics.map((d) => diag(size, d.message, d.start, d.length, d.code)));
      // The statement as the build sends it, its file's comment included (src/sqlcommenter.ts).
      const compiled = project.sqlcommenter ? withSqlcommenter(analysis.rewritten.sql, sourceName(p.fileName)) : analysis.rewritten.sql;
      return emit(compiled, types.params, types.row, types.cardinality, types.names, analysis.rewritten.sql, types.queryError);
    },
    async closeProject(params) {
      const handle = (params as { projectHandle: string }).projectHandle;
      const project = projects.get(handle);
      projects.delete(handle);
      if (project) await (await project.session).close();
      return {};
    },
  };
}
