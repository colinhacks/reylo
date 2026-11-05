// Reylo: `reylo generate` analyzes each .sql file into .reylo/, whose entries hold the analysis the type check reads.
import { mkdirSync, readdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import type { Kind } from "../probes.ts";
import { failed, gitDelta, gitInit, rewrite, sh, workdir, type Answer, type Tool, type TypedValue } from "./lib.ts";

const bin = resolve(import.meta.dirname, "../node_modules/.bin/reylo");
const version: string = JSON.parse(readFileSync(resolve(import.meta.dirname, "../node_modules/reylo/package.json"), "utf8")).version;

function kindOf(t: any): Kind {
  if (t.kind === "list" || t.kind === "object" || t.kind === "array" || t.kind === "tuple") return "json";
  if (t.kind === "enum") return "text";
  const name = String(t.name ?? "").toLowerCase();
  if (/^(int|smallint|mediumint|bigint|tinyint|float|double|real|integer|year)|^(int[248]|float[48]|oid)$/.test(name)) return "num";
  if (/^(numeric|decimal)/.test(name)) return "text";
  if (/^(bool)/.test(name)) return "bool";
  if (/^(json|jsonb)$/.test(name)) return "any";
  if (/^(timestamp|timestamptz|date|time|datetime)/.test(name)) return "time";
  if (/^(any|unknown|null|blob)$/.test(name) || !name) return "any";
  return "text";
}
function fieldsOf(t: any): string[] | undefined {
  if (t.schema?.properties) return Object.keys(t.schema.properties);
  if (t.kind === "object") return (t.fields ?? []).map((f: any) => f.name);
  if (t.of) return fieldsOf(t.of);
  if (t.items) return fieldsOf(t.items);
  return undefined;
}
const show = (t: any): string => (t.kind === "json" ? (t.schema ? `json(${JSON.stringify(t.schema)})` : "Json") : t.kind === "scalar" ? t.name : t.kind === "enum" ? (t.labels ?? []).map((l: string) => JSON.stringify(l)).join(" | ") : t.kind === "object" ? `{ ${(t.fields ?? []).map((f: any) => `${f.name}: ${show(f.type)}${f.nullable ? " | null" : ""}`).join("; ")} }` : t.of ? `${show(t.of)}[]` : JSON.stringify(t));
const value = (c: any): TypedValue => ({ type: `${show(c.type)}${c.nullable ? " | null" : ""}`, kind: kindOf(c.type), nullable: !!c.nullable, fields: fieldsOf(c.type) });
const normal = (sql: string) => sql.replace(/\$\d+|\?\d*|(?<!:):p\d+\b/g, "?").replace(/\s+/g, " ").trim().replace(/;$/, "");

export const reylo: Tool = {
  id: "reylo",
  name: "Reylo",
  version,
  dbs: ["postgres", "mysql", "sqlite"],
  async duckdb(probe) {
    const dir = workdir("reylo-duckdb");
    mkdirSync(join(dir, "q"));
    writeFileSync(join(dir, "package.json"), '{"name":"p","private":true,"type":"module"}\n');
    symlinkSync(resolve(import.meta.dirname, "../node_modules"), join(dir, "node_modules"));
    const { DuckDBInstance } = await import("@duckdb/node-api");
    const db = await DuckDBInstance.create(join(dir, "app.duckdb"));
    const c = await db.connect();
    await c.run("create table users (id integer primary key, name varchar not null)");
    c.closeSync();
    db.closeSync();
    writeFileSync(join(dir, "q/smoke.sql"), `${probe.sql}\n`);
    const run = sh(bin, ["generate", "q"], { cwd: dir, env: { DATABASE_URL: "duckdb:./app.duckdb" } });
    return run.status === 0 ? { ok: true, params: [], columns: [], cardinality: null } : failed(run.all);
  },
  async analyze(db, statements, urls) {
    const dir = workdir("reylo");
    mkdirSync(join(dir, "q"));
    writeFileSync(join(dir, "package.json"), '{"name":"p","private":true,"type":"module"}\n');
    statements.forEach((s, i) => writeFileSync(join(dir, "q", `s${i}.sql`), `${rewrite(s.sql, (n) => `:p${n}`)}\n`));
    gitInit(dir);
    const url = db === "postgres" ? urls.postgres : db === "mysql" ? urls.mysql : `file:${urls.sqlite}`;
    const run = sh(bin, ["generate", "q"], { cwd: dir, env: { DATABASE_URL: url } });
    const entries = new Map<string, any>();
    try {
      for (const f of readdirSync(join(dir, ".reylo", "queries"))) {
        const e = JSON.parse(readFileSync(join(dir, ".reylo", "queries", f), "utf8")).analysis;
        entries.set(normal(e.rewritten.sql), e);
      }
    } catch {}
    const answers: Record<string, Answer> = {};
    for (const s of statements) {
      const a = entries.get(normal(s.sql));
      answers[s.key] = a ? { ok: true, params: a.params.map(value), columns: a.columns.map((c: any) => ({ name: c.name, ...value(c) })), cardinality: a.cardinality } : failed(run.all.split("\n").filter((l) => l.includes(`s${statements.indexOf(s)}.sql`)).join(" ") || run.all);
    }
    return { answers, wrote: gitDelta(dir) };
  },
};
