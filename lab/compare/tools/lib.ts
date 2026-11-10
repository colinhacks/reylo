// What an adapter returns for a statement, and the helpers adapters share.
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import ts from "typescript";
import type { Card, Db, Kind, Probe } from "../probes.ts";

// `checksValues`: the tool types no parameter, and checks the value at each call instead; `wrongValuePasses` names a wrong value it let through.
export interface TypedValue { type: string; kind: Kind; nullable: boolean; fields?: string[]; checksValues?: boolean; wrongValuePasses?: string }
// `shown`: the parameters and row as the tool writes them in TypeScript, for a tool whose types the docs show whole.
export interface Answer { ok: boolean; error?: string; params: TypedValue[] | null; columns: (TypedValue & { name: string })[] | null; cardinality: Card | null; shown?: { params: string; row: string } }
export interface Statement { key: string; probe: Probe; sql: string }
export interface Urls { postgres: string; mysql: string; sqlite: string }
export interface Tool {
  id: string;
  name: string;
  version: string;
  dbs: Db[];
  // How a user gets a row count from this tool, when the tool does not infer one.
  cardinalityNote?: string;
  // Types one statement on a DuckDB file, for the tools that can.
  duckdb?(probe: Probe): Promise<Answer>;
  noDuckdb?: string;
  // A second way to use the tool with nothing generated, run for real: the note to show where it works.
  withoutGenerating?(): Promise<string | null>;
  // Why it never connects, for a tool that reads its types elsewhere.
  noConnectNote?: string;
  // A setting that makes a tool that needs no database ask one, on Postgres, and what it says of it.
  withDatabaseSetting?(statements: Statement[], urls: Urls): Promise<Record<string, Answer>>;
  optInNote?: string;
  analyze(db: Db, statements: Statement[], urls: Urls): Promise<{ answers: Record<string, Answer>; wrote: string[] }>;
}

export const failed = (error: string): Answer => ({ ok: false, error: error.slice(0, 400), params: null, columns: null, cardinality: null });

export function workdir(prefix: string): string {
  const dir = mkdtempSync(join(tmpdir(), `reylo-compare-${prefix}-`));
  return dir;
}

export function sh(cmd: string, args: string[], opts: { cwd?: string; env?: Record<string, string>; timeout?: number } = {}) {
  const env = { ...process.env, ...opts.env } as Record<string, string>;
  delete env.NODE_COMPAT;
  const r = spawnSync(cmd, args, { cwd: opts.cwd, env, encoding: "utf8", timeout: opts.timeout ?? 180_000, maxBuffer: 64 * 1024 * 1024 });
  return { status: r.status, out: r.stdout ?? "", err: r.stderr ?? "", all: `${r.stdout ?? ""}${r.stderr ?? ""}` };
}

// Starts a project as a git repository, so that a run's new and changed files can be read back as a user's `git status` would show them.
export function gitInit(dir: string, ignore: string[] = ["node_modules", "target"]) {
  writeFileSync(join(dir, ".gitignore"), `${ignore.join("\n")}\n`);
  sh("git", ["init", "-q"], { cwd: dir });
  sh("git", ["add", "-A"], { cwd: dir });
  sh("git", ["-c", "user.name=lab", "-c", "user.email=lab@example.com", "-c", "commit.gpgsign=false", "commit", "-qm", "before"], { cwd: dir });
}
export function gitDelta(dir: string): string[] {
  return sh("git", ["status", "--porcelain", "--untracked-files=all"], { cwd: dir }).out.split("\n").filter(Boolean).map((l) => l.trim());
}

// Rewrites $1, $2, … with what a tool writes for its nth parameter.
export const rewrite = (sql: string, f: (n: number) => string) => sql.replace(/\$(\d+)/g, (_, n) => f(Number(n)));
export const paramCount = (sql: string) => new Set([...sql.matchAll(/\$(\d+)/g)].map((m) => m[1])).size;

// What a generated file declares, by name: an object's members, or a type's text.
export type Declarations = Record<string, { name: string; optional: boolean; type: string }[] | string>;

// Reads a TypeScript type as text: its kind, whether it admits null, and the field names of the object it holds. A name the file declares (`decls`) is followed to its declaration.
export function readTsType(text: string, decls: Declarations = {}): TypedValue {
  const src = ts.createSourceFile("t.ts", `type T = ${text};`, ts.ScriptTarget.Latest, true);
  const fields = new Set<string>();
  let nullable = false;
  const kinds = new Set<Kind>();
  const visitText = (t: string, top: boolean, seen: Set<string>) => {
    const s = ts.createSourceFile("t.ts", `type T = ${t};`, ts.ScriptTarget.Latest, true);
    visit((s.statements[0] as ts.TypeAliasDeclaration).type, top, seen);
  };
  const visit = (n: ts.TypeNode, top: boolean, seen: Set<string>): void => {
    if (ts.isParenthesizedTypeNode(n)) return visit(n.type, top, seen);
    if (ts.isTypeOperatorNode(n)) return visit(n.type, top, seen);
    if (ts.isUnionTypeNode(n)) return n.types.forEach((t) => visit(t, top, seen));
    if (ts.isArrayTypeNode(n)) { kinds.add("json"); return visit(n.elementType, false, seen); }
    if (ts.isTupleTypeNode(n)) { kinds.add("json"); return; }
    if (ts.isTypeReferenceNode(n)) {
      const name = n.typeName.getText();
      if (/^(Readonly)?Array$|^PgArray$/.test(name) && n.typeArguments) { kinds.add("json"); return visit(n.typeArguments[0], false, seen); }
      if (name === "Date") return void kinds.add("time");
      if (/(^|\.)Decimal$/.test(name)) return void kinds.add("num");
      if (/^Pg\w*String$/.test(name)) return void kinds.add("text");
      const d = decls[name];
      if (d !== undefined && !seen.has(name)) {
        const next = new Set(seen).add(name);
        if (typeof d === "string") return visitText(d, top, next);
        kinds.add("json");
        for (const m of d) fields.add(m.name);
        return;
      }
      return void kinds.add("any");
    }
    if (ts.isTypeLiteralNode(n)) {
      kinds.add("json");
      for (const m of n.members) if (m.name) fields.add(m.name.getText().replace(/['"]/g, ""));
      return;
    }
    if (ts.isLiteralTypeNode(n)) {
      if (n.literal.kind === ts.SyntaxKind.NullKeyword) { if (top) nullable = true; return; }
      if (ts.isStringLiteral(n.literal)) return void kinds.add("text");
      if (ts.isNumericLiteral(n.literal)) return void kinds.add("num");
      return void kinds.add("bool");
    }
    switch (n.kind) {
      case ts.SyntaxKind.NumberKeyword: case ts.SyntaxKind.BigIntKeyword: return void kinds.add("num");
      case ts.SyntaxKind.StringKeyword: return void kinds.add("text");
      case ts.SyntaxKind.BooleanKeyword: return void kinds.add("bool");
      case ts.SyntaxKind.UndefinedKeyword: case ts.SyntaxKind.VoidKeyword: if (top) nullable = true; return;
      // Any value at all, null among them.
      case ts.SyntaxKind.AnyKeyword: case ts.SyntaxKind.UnknownKeyword: if (top) nullable = true; return void kinds.add("any");
      default: return void kinds.add("any");
    }
  };
  visit((src.statements[0] as ts.TypeAliasDeclaration).type, true, new Set());
  const kind: Kind = kinds.has("any") ? "any" : kinds.has("json") ? "json" : kinds.values().next().value ?? "any";
  return { type: text.replace(/\s+/g, " ").trim(), kind, nullable, fields: fields.size ? [...fields] : undefined };
}

// The members of every interface and object type alias in a TypeScript file, by declared name.
export function readTsDeclarations(path: string): Declarations {
  const text = readFileSync(path, "utf8");
  const src = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true);
  const out: Declarations = {};
  const members = (ms: ts.NodeArray<ts.TypeElement>) => ms.filter(ts.isPropertySignature).map((m) => ({ name: m.name.getText().replace(/['"]/g, ""), optional: !!m.questionToken, type: m.type?.getText() ?? "any" }));
  const walk = (statements: ts.NodeArray<ts.Statement>, prefix: string) => {
    for (const s of statements) {
      if (ts.isInterfaceDeclaration(s)) out[prefix + s.name.text] = members(s.members);
      if (ts.isTypeAliasDeclaration(s)) out[prefix + s.name.text] = ts.isTypeLiteralNode(s.type) ? members(s.type.members) : s.type.getText();
      if (ts.isModuleDeclaration(s) && s.body && ts.isModuleBlock(s.body)) walk(s.body.statements, `${s.name.getText()}.`);
    }
  };
  walk(src.statements, "");
  return out;
}
// The elements of a TypeScript tuple type, as type text: `[a: number, b?: string]`.
export function readTsTuple(text: string): string[] {
  const src = ts.createSourceFile("t.ts", `type T = ${text};`, ts.ScriptTarget.Latest, true);
  const node = (src.statements[0] as ts.TypeAliasDeclaration).type;
  if (!ts.isTupleTypeNode(node)) return [];
  return node.elements.map((e) => (ts.isNamedTupleMember(e) ? e.type.getText() : e.getText()));
}
export { existsSync, readFileSync, writeFileSync, join };
