// What every build step emits for a `.sql` file: a module that carries everything the query needs
// at run time (the dialect, the statement as the engine rewrote it for the database, the binding
// order, each parameter's type, the row count the analysis proved), so that the running code reads no file and asks no
// database. The analysis comes from `src/analyze.ts`: `.reylo/` when it holds the statement for the
// current schema, else the database. A build that has neither fails, because a module built
// without the analysis would not do what its types say.
import { dirname } from "node:path";
import { Session } from "./analyze.ts";
import { sourceName } from "./project.ts";
import { withSqlcommenter } from "./sqlcommenter.ts";
import { querySchemas } from "./schema.ts";
import { columnKind } from "./values.ts";

export type SqlModule = {
  code: string;
  /** The `.reylo/` entry the module was built from. A bundler watches it, so that the module is built again when the type check analyzes the statement anew (after a migration). */
  entry: string;
};

export async function sqlModule(path: string, sql: string): Promise<SqlModule> {
  const session = await Session.for(dirname(path));
  let outcome = await session.analyze(sql);
  // A statement the database refuses may have been analyzed against a schema read a few seconds
  // before a migration (a session reads it again at most every 5 seconds, and SQLite's analysis
  // replays it): it is analyzed again when the schema has changed since.
  if (outcome.ok && outcome.analysis.diagnostics.some((d) => d.code === 6) && (await session.recheck())) outcome = await session.analyze(sql);
  if (!outcome.ok) throw new Error(`${path}: ${outcome.message}`);
  const { analysis } = outcome;
  // A statement whose placeholders break a rule fails the build: one the engine could not read
  // (1), named and positional mixed (2), or a parameter written more than once (3), since each
  // parameter appears once so that one place types it.
  const unread = analysis.diagnostics.find((d) => d.code === 1 || d.code === 2 || d.code === 3);
  if (unread) throw new Error(`${path}: ${unread.message}`);
  let params: string[];
  if (session.project.dialect === "mysql") {
    // A MySQL session reads its strings by its sql_mode, which the analysis asked for: its
    // placeholders are the ones the server finds.
    params = analysis.params.map((p) => p.name);
  } else {
    try {
      params = session.lex(sql).params;
    } catch (e) {
      throw new Error(`${path}: ${(e as Error).message}`);
    }
  }
  // A column list that does not read, or that disagrees with the table its insert writes, fails
  // the build too.
  const descriptor = analysis.diagnostics.find((d) => d.code === 20 || d.code === 21);
  if (descriptor) throw new Error(`${path}: ${descriptor.message}`);
  // So does a statement the database refuses (6: a misspelled column, a dropped table), at its line
  // and column, as the type check reports it: its module would run a statement nothing typed, with
  // no columns and any number of rows.
  const refused = analysis.diagnostics.find((d) => d.code === 6);
  if (refused) throw new Error(`${path}:${place(sql, refused.start)}: ${refused.message}`);
  // Each parameter's type, for the drivers that bind an array or a JSON value wrong, and whether it
  // takes null, which the run time refuses for one that does not (src/execute.ts).
  const inputs = Object.fromEntries(analysis.params.map((p) => [p.name, { type: p.type, ...(p.schema === undefined ? {} : { schema: p.schema }), ...(p.cast === undefined ? {} : { cast: p.cast }), nullable: p.nullable }]));
  // Each output column's name and kind, from the table the declared types come from: what the run
  // time gives each value (src/values.ts).
  const columns = analysis.columns.map((c) => [c.name, columnKind(c.type, session.project.dialect)]);
  // A structured parameter's lists, at JavaScript's offsets into the statement (the engine's are
  // UTF-8 bytes).
  const source = sourceName(path);
  const text = session.project.sqlcommenter ? withSqlcommenter(analysis.rewritten.sql, source) : analysis.rewritten.sql;
  const structured = (analysis.structured ?? []).map((s) => ({ name: s.name, lists: s.lists.map((l) => ({ start: utf16(text, l.start), end: utf16(text, l.end), entries: l.entries })) }));
  // A typed JSON, structured or cast parameter is checked with reylo's `checks`, imported only where
  // the statement has one, so that an app none of whose statements has one bundles none of them
  // (src/checks.ts). A non-null JSON parameter whose type alone carries a schema needs them too,
  // to learn whether that schema allows null.
  const checked = analysis.params.some((p) => p.schema !== undefined || p.cast !== undefined || p.type.kind === "rows" || (p.type.kind === "json" && p.type.schema !== undefined && !p.nullable));
  const code = [
    `import { defineQuery${checked ? ", checks" : ""} } from "reylo";`,
    // `sql` is the statement as Reylo sends it to the database, `raw` the file as written; the
    // definition's own `sql` is the statement as written and its `text` the compiled one.
    `export const sql = ${JSON.stringify(text)};`,
    `export const raw = ${JSON.stringify(sql)};`,
    `export const params = ${JSON.stringify(params)};`,
    `export default defineQuery({ dialect: ${JSON.stringify(session.project.dialect)}, sql: raw, params, cardinality: ${JSON.stringify(analysis.cardinality)}, text: sql, order: ${JSON.stringify(analysis.rewritten.order)}, inputs: ${JSON.stringify(inputs)}, columns: ${JSON.stringify(columns)}${structured.length ? `, structured: ${JSON.stringify(structured)}` : ""}${analysis.fresh ? `, fresh: ${JSON.stringify(analysis.fresh)}` : ""}, source: ${JSON.stringify(source)}, jsonSchema: ${JSON.stringify(querySchemas(analysis, session.project.dialect))}${checked ? ", checks" : ""} });`,
    ``,
  ].join("\n");
  return { code, entry: outcome.entry };
}

/** The line and column, from 1, of a UTF-8 byte offset into `text`. */
function place(text: string, bytes: number): string {
  const before = text.slice(0, utf16(text, bytes));
  return `${before.split("\n").length}:${before.length - before.lastIndexOf("\n")}`;
}

/** The JavaScript offset into `text` of a UTF-8 byte offset. */
function utf16(text: string, bytes: number): number {
  return Buffer.from(text, "utf8").subarray(0, bytes).toString("utf8").length;
}
