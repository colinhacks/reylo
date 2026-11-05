// TypeSQL: `typesql compile` writes a TypeScript file per .sql file, whose function signature holds what it inferred, the row count included.
import { mkdirSync, readFileSync, symlinkSync, writeFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import ts from "typescript";
import type { Card, Db } from "../probes.ts";
import { failed, gitDelta, gitInit, readTsDeclarations, readTsType, rewrite, sh, workdir, type Answer, type Tool } from "./lib.ts";

const root = resolve(import.meta.dirname, "..");
const bin = join(root, "node_modules/.bin/typesql");
const version: string = JSON.parse(readFileSync(join(root, "node_modules/typesql-cli/package.json"), "utf8")).version;
const CLIENT: Record<Db, string> = { postgres: "pg", mysql: "mysql2", sqlite: "better-sqlite3" };

// The row count is in the return type: Row[] is many, Row | null is at most one, Row is exactly one, and a result of counts is none.
function cardinalityOf(path: string, result: unknown): Card | null {
  const src = ts.createSourceFile(path, readFileSync(path, "utf8"), ts.ScriptTarget.Latest, true);
  for (const s of src.statements) {
    if (!ts.isFunctionDeclaration(s) || !s.modifiers?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword) || !s.type) continue;
    let t: ts.TypeNode = s.type;
    if (ts.isTypeReferenceNode(t) && t.typeName.getText() === "Promise" && t.typeArguments) t = t.typeArguments[0];
    const text = t.getText().replace(/\s/g, "");
    if (Array.isArray(result) && result.some((f: any) => /^(rowCount|changes|affectedRows|lastInsertRowid|insertId)$/.test(f.name))) return "none";
    if (text.endsWith("[]")) return "many";
    if (text.endsWith("|null") || text.endsWith("|undefined")) return "maybeOne";
    return "one";
  }
  return null;
}

export const typesql: Tool = {
  id: "typesql",
  name: "TypeSQL",
  version,
  dbs: ["postgres", "mysql", "sqlite"],
  noDuckdb: "Its clients are Postgres, MySQL, SQLite, LibSQL and D1.",
  async analyze(db, statements, urls) {
    const dir = workdir("typesql");
    symlinkSync(join(root, "node_modules"), join(dir, "node_modules"));
    mkdirSync(join(dir, "sql"));
    writeFileSync(join(dir, "package.json"), '{"name":"p","private":true,"type":"module"}\n');
    writeFileSync(join(dir, "typesql.json"), JSON.stringify({ databaseUri: db === "postgres" ? urls.postgres : db === "mysql" ? urls.mysql : urls.sqlite, sqlDir: "./sql", client: CLIENT[db], includeCrudTables: [] }));
    statements.forEach((s, i) => writeFileSync(join(dir, `sql/q${i}.sql`), `${rewrite(s.sql, db === "postgres" ? (n) => `$${n}` : (n) => `:p${n}`)}\n`));
    gitInit(dir);
    const run = sh(bin, ["compile"], { cwd: dir });
    const answers: Record<string, Answer> = {};
    statements.forEach((s, i) => {
      const out = join(dir, `sql/q${i}.ts`);
      if (!existsSync(out)) { answers[s.key] = failed(run.all.split("\n").filter((l) => l.includes(`q${i}`)).join(" ") || run.all); return; }
      if (readFileSync(out, "utf8").startsWith("//Invalid SQL")) { answers[s.key] = failed("it marks this query as invalid SQL"); return; }
      const d = readTsDeclarations(out);
      const P = `Q${i}`;
      const result = d[`${P}Result`];
      // An UPDATE's SET values and its WHERE values are two objects, whose fields are numbered through the statement.
      const fields = [...(Array.isArray(d[`${P}Data`]) ? (d[`${P}Data`] as any[]) : []), ...(Array.isArray(d[`${P}Params`]) ? (d[`${P}Params`] as any[]) : [])];
      const number = (name: string) => Number(name.match(/(\d+)$/)?.[1] ?? 0);
      const params = [...fields].sort((a, b) => number(a.name) - number(b.name)).map((f) => { const t = readTsType(f.type, d); return { ...t, nullable: t.nullable || f.optional }; });
      const counts = Array.isArray(result) && result.some((f) => /^(rowCount|changes|affectedRows|lastInsertRowid|insertId)$/.test(f.name));
      answers[s.key] = { ok: true, params, columns: Array.isArray(result) && !counts ? result.map((f) => ({ name: f.name, ...readTsType(f.type, d) })) : [], cardinality: cardinalityOf(out, result) };
    });
    return { answers, wrote: gitDelta(dir) };
  },
};
