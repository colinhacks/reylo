// SafeQL: an ESLint rule over tagged templates. `eslint --fix` writes the row type into the source, and a value of the wrong type fails the lint, which is how it types parameters: from each call's value, not from the query.
import { readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import ts from "typescript";
import type { Kind } from "../probes.ts";
import { failed, gitDelta, gitInit, readTsType, rewrite, sh, workdir, type Answer, type Statement, type Tool, type TypedValue } from "./lib.ts";

const root = resolve(import.meta.dirname, "..");
const bin = join(root, "node_modules/.bin/eslint");
const version: string = JSON.parse(readFileSync(join(root, "node_modules/@ts-safeql/eslint-plugin/package.json"), "utf8")).version;

const kindToValue = (k?: Kind) => (k === "num" ? "number" : k === "time" ? "Date" : "string");
const wrongValue = (v: string) => (v === "number" ? "boolean" : "boolean");
const valueOf = (s: Statement, i: number) => s.probe.params?.[i]?.value ?? kindToValue(s.probe.params?.[i]?.kinds?.[0]);
const count = (sql: string) => new Set([...sql.matchAll(/\$(\d+)/g)].map((m) => m[1])).size;

// One file of tagged templates, each with its parameters declared with the given value types.
function source(items: { name: string; sql: string; values: string[] }[]) {
  const decls = items.flatMap((it) => it.values.map((v, i) => `declare const ${it.name}_p${i + 1}: ${v};`));
  return `import postgres from "postgres";\nconst sql = postgres();\n${decls.join("\n")}\n${items.map((it) => `export const ${it.name} = sql\`${rewrite(it.sql, (n) => `\${${it.name}_p${n}}`)}\`;`).join("\n")}\n`;
}
// Lints the file, and returns each template's messages by its const's name.
function lint(dir: string, file: string, fix: boolean) {
  const run = sh(bin, [file, "--format", "json", ...(fix ? ["--fix"] : [])], { cwd: dir, timeout: 300_000 });
  const text = readFileSync(join(dir, file), "utf8");
  const lines = text.split("\n");
  const byName: Record<string, { messageId?: string; message: string }[]> = {};
  try {
    for (const m of JSON.parse(run.out)[0].messages) {
      for (let l = m.line - 1; l >= 0; l--) { const found = lines[l]?.match(/^export const (\w+) = /); if (found) { (byName[found[1]] ??= []).push(m); break; } }
    }
  } catch { return { byName, fatal: run.all }; }
  return { byName, fatal: "" };
}
// The row type `--fix` wrote: sql<{ a: string }[]>`…`.
function rowTypes(path: string): Record<string, string> {
  const src = ts.createSourceFile(path, readFileSync(path, "utf8"), ts.ScriptTarget.Latest, true);
  const out: Record<string, string> = {};
  for (const s of src.statements) {
    if (!ts.isVariableStatement(s)) continue;
    for (const d of s.declarationList.declarations) {
      if (d.initializer && ts.isTaggedTemplateExpression(d.initializer) && d.initializer.typeArguments?.[0]) out[d.name.getText()] = d.initializer.typeArguments[0].getText();
    }
  }
  return out;
}
function columnsOf(row: string): (TypedValue & { name: string })[] {
  const src = ts.createSourceFile("t.ts", `type T = ${row};`, ts.ScriptTarget.Latest, true);
  let node = (src.statements[0] as ts.TypeAliasDeclaration).type;
  if (ts.isArrayTypeNode(node)) node = node.elementType;
  if (!ts.isTypeLiteralNode(node)) return [];
  return node.members.filter(ts.isPropertySignature).map((m) => ({ name: m.name.getText().replace(/['"]/g, ""), ...readTsType(m.type?.getText() ?? "any") }));
}
const blocking = (msgs?: { messageId?: string }[]) => (msgs ?? []).filter((m) => m.messageId !== "missingTypeAnnotations");

export const safeql: Tool = {
  id: "safeql",
  name: "SafeQL",
  version,
  dbs: ["postgres"],
  noDuckdb: 'SafeQL is Postgres only.',
  cardinalityNote: "Every query is typed as an array, unless your own transform says otherwise.",
  async analyze(_db, statements, urls) {
    const dir = workdir("safeql");
    symlinkSync(join(root, "node_modules"), join(dir, "node_modules"));
    writeFileSync(join(dir, "package.json"), '{"name":"p","private":true,"type":"module"}\n');
    writeFileSync(join(dir, "tsconfig.json"), '{"compilerOptions":{"target":"es2022","module":"esnext","moduleResolution":"bundler","strict":true,"skipLibCheck":true,"noEmit":true},"include":["*.ts"]}\n');
    writeFileSync(join(dir, "eslint.config.js"), `import safeql from "@ts-safeql/eslint-plugin/config";\nimport tseslint from "typescript-eslint";\nexport default tseslint.config({ files: ["**/*.ts"], languageOptions: { parser: tseslint.parser, parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname } } }, safeql.configs.connections({ databaseUrl: ${JSON.stringify(urls.postgres)}, targets: [{ tag: "sql", transform: "{type}[]" }] }));\n`);
    const items = statements.map((s, i) => ({ s, name: `q${i}`, n: count(s.sql) }));
    writeFileSync(join(dir, "right.ts"), source(items.map(({ s, name, n }) => ({ name, sql: s.sql, values: Array.from({ length: n }, (_, k) => valueOf(s, k)) }))));
    gitInit(dir);
    const right = lint(dir, "right.ts", true);
    const wrote = gitDelta(dir);
    const rows = rowTypes(join(dir, "right.ts"));
    // For each parameter: is a value of the wrong type refused, and is a null refused?
    const perParam = items.flatMap(({ s, name, n }) => Array.from({ length: n }, (_, k) => ({ s, name, k })));
    const variant = (make: (v: string) => string) => source(perParam.map(({ s, name, k }) => ({ name: `${name}_${k}`, sql: s.sql, values: Array.from({ length: count(s.sql) }, (_, j) => (j === k ? make(valueOf(s, j)) : valueOf(s, j))) })));
    writeFileSync(join(dir, "wrong.ts"), variant(wrongValue));
    writeFileSync(join(dir, "nulls.ts"), variant((v) => `${v} | null`));
    const wrong = lint(dir, "wrong.ts", false);
    const nulls = lint(dir, "nulls.ts", false);
    const answers: Record<string, Answer> = {};
    for (const { s, name, n } of items) {
      const errs = blocking(right.byName[name]);
      if (right.fatal || errs.length) { answers[s.key] = failed(right.fatal || errs.map((e) => (e as any).message).join("; ")); continue; }
      const params: TypedValue[] = Array.from({ length: n }, (_, k) => {
        const refused = blocking(wrong.byName[`${name}_${k}`]).length > 0;
        const takesNull = blocking(nulls.byName[`${name}_${k}`]).length === 0;
        const want = s.probe.params?.[k];
        const value = valueOf(s, k);
        return { type: value, kind: refused ? (want?.kinds?.[0] ?? (value === "number" ? "num" : "text")) : "any", nullable: takesNull, checksValues: true, wrongValuePasses: refused ? undefined : "boolean" };
      });
      answers[s.key] = { ok: true, params, columns: rows[name] ? columnsOf(rows[name]) : [], cardinality: null };
    }
    return { answers, wrote };
  },
};
