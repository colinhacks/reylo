// sqlc: `sqlc generate` writes Go; its structs and method signatures are what it inferred. Each statement runs alone, since one error stops a whole run.
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import type { Db, Kind } from "../probes.ts";
import { failed, gitDelta, gitInit, rewrite, sh, workdir, type Answer, type Tool, type TypedValue } from "./lib.ts";

const schemaDir = resolve(import.meta.dirname, "../schema");
const version = sh("sqlc", ["version"]).out.trim();
const ENGINE: Record<Db, string> = { postgres: "postgresql", mysql: "mysql", sqlite: "sqlite" };

function goType(type: string): TypedValue {
  const t = type.trim();
  const nullable = /^(sql\.Null|uuid\.NullUUID|pqtype\.Null|\*|interface\{\})/.test(t) || t === "any";
  let kind: Kind = "any";
  if (/^(sql\.Null(Int|Float)|u?int\d*$|float\d+$)/.test(t)) kind = "num";
  else if (/^(string$|sql\.NullString|uuid\.)/.test(t)) kind = "text";
  else if (/^(bool$|sql\.NullBool)/.test(t)) kind = "bool";
  else if (/^(time\.Time|sql\.NullTime)/.test(t)) kind = "time";
  else if (/^\[\](u?int|string)/.test(t)) kind = "json";
  return { type: t, kind, nullable };
}
function structFields(go: string, name: string): { name: string; type: string }[] | null {
  const m = go.match(new RegExp(`type ${name} struct \\{([\\s\\S]*?)\\n\\}`));
  if (!m) return null;
  return m[1].split("\n").map((l) => l.trim()).filter(Boolean).map((l) => { const [n, ...t] = l.replace(/`.*`/, "").trim().split(/\s+/); return { name: n, type: t.join(" ") }; });
}
function readQuery(go: string, wantColumns: string[] | undefined): Answer {
  const sig = go.match(/func \(q \*Queries\) Q\(ctx context\.Context(?:, arg (\w+))?\)\s*(?:\(([^)]*)\)|error)/);
  if (!sig) return failed("no method generated");
  const params = sig[1] ? (structFields(go, sig[1]) ?? []).map((f) => goType(f.type)) : [];
  let columns: (TypedValue & { name: string })[] = [];
  const ret = sig[2]?.split(",")[0]?.trim().replace(/^\[\]/, "");
  if (ret) {
    const fields = structFields(go, ret);
    if (fields) columns = fields.map((f, i) => ({ name: wantColumns?.find((c) => c.toLowerCase() === f.name.toLowerCase()) ?? wantColumns?.[i] ?? f.name, ...goType(f.type) }));
    else columns = [{ name: wantColumns?.[0] ?? "v", ...goType(ret) }];
  }
  return { ok: true, params, columns, cardinality: null };
}

export const sqlc: Tool = {
  id: "sqlc",
  name: "sqlc",
  version,
  dbs: ["postgres", "mysql", "sqlite"],
  noDuckdb: 'Not in v1.31.1, the latest release. A DuckDB engine is on its unreleased main branch.',
  cardinalityNote: "Declared by hand on each query: `:one`, `:many` or `:exec`. The annotation decides the method's return type, and sqlc accepts any of them.",
  async analyze(db, statements, urls) {
    const answers: Record<string, Answer> = {};
    let wrote: string[] = [];
    const database = process.env.COMPARE_SQLC_DATABASE === "1" && db === "postgres" ? `\n    database:\n      uri: ${urls.postgres}` : "";
    for (const s of statements) {
      const dir = workdir("sqlc");
      writeFileSync(join(dir, "schema.sql"), readFileSync(join(schemaDir, `${db}.sql`), "utf8"));
      const sql = rewrite(s.sql, db === "postgres" ? (n) => `$${n}` : () => "?");
      const returns = /^\s*select|returning/i.test(sql);
      writeFileSync(join(dir, "query.sql"), `-- name: Q ${returns ? ":many" : ":exec"}\n${sql};\n`);
      writeFileSync(join(dir, "sqlc.yaml"), `version: "2"\nsql:\n  - engine: ${ENGINE[db]}\n    schema: schema.sql\n    queries: query.sql${database}\n    gen:\n      go:\n        package: db\n        out: db\n        query_parameter_limit: 0\n`);
      gitInit(dir);
      const run = sh("sqlc", ["generate"], { cwd: dir });
      const out = join(dir, "db", "query.sql.go");
      if (run.status !== 0 || !existsSync(out)) { answers[s.key] = failed(run.all); continue; }
      answers[s.key] = readQuery(readFileSync(out, "utf8"), s.probe.columns?.map((c) => c.name));
      wrote = gitDelta(dir);
    }
    return { answers, wrote };
  },
};
