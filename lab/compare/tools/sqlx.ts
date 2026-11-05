// sqlx: its query! macros read each statement's description from the database at compile time. A small program asks for the same description, and one crate compiled with query! settles what the macro accepts.
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import type { Kind } from "../probes.ts";
import { failed, gitDelta, gitInit, rewrite, sh, workdir, type Answer, type Tool, type TypedValue } from "./lib.ts";

const here = import.meta.dirname;
const target = resolve(here, "../.out/target");
// Homebrew's own cargo comes first on a Mac unless rustup's is named.
const PATH = existsSync("/opt/homebrew/opt/rustup/bin") ? `/opt/homebrew/opt/rustup/bin:${process.env.PATH}` : process.env.PATH!;
const version = "0.9.0";

function build() {
  const r = sh("cargo", ["build", "--release", "--manifest-path", join(here, "sqlx-describe/Cargo.toml")], { env: { PATH, CARGO_TARGET_DIR: target }, timeout: 1_800_000 });
  if (r.status !== 0) throw new Error(`sqlx-describe failed to build:\n${r.all}`);
}
function kindOf(t: string): Kind {
  const n = t.toUpperCase();
  if (/INT|FLOAT|DOUBLE|REAL|YEAR/.test(n)) return "num";
  if (/NUMERIC|DECIMAL/.test(n)) return "num";
  if (/BOOL/.test(n)) return "bool";
  if (/\[\]/.test(n)) return "json";
  if (/JSON/.test(n)) return "any";
  if (/TIME|DATE/.test(n)) return "time";
  if (/TEXT|CHAR|UUID|ENUM/.test(n)) return "text";
  return "any";
}
// Does query! take a null where the column is a key? A compile against the live database decides, once.
let optionAccepted: boolean | null = null;
function checkOption(url: string): { accepted: boolean; wrote: string[] } {
  const dir = workdir("sqlx-check");
  cpSync(join(here, "sqlx-check"), dir, { recursive: true });
  const lock = join(here, "sqlx-describe/Cargo.lock");
  if (existsSync(lock)) copyFileSync(lock, join(dir, "Cargo.lock"));
  sh("cargo", ["generate-lockfile"], { cwd: dir, env: { PATH } });
  gitInit(dir);
  const r = sh("cargo", ["check"], { cwd: dir, env: { PATH, CARGO_TARGET_DIR: target, DATABASE_URL: url, SQLX_OFFLINE: "false" }, timeout: 1_800_000 });
  return { accepted: r.status === 0, wrote: gitDelta(dir) };
}

export const sqlx: Tool = {
  id: "sqlx",
  name: "sqlx",
  version,
  dbs: ["postgres", "mysql", "sqlite"],
  noDuckdb: 'sqlx has no DuckDB driver.',
  cardinalityNote: "You pick it on each call: `fetch_one`, `fetch_optional` or `fetch_all`.",
  async analyze(db, statements, urls) {
    build();
    const url = db === "postgres" ? urls.postgres : db === "mysql" ? urls.mysql : `sqlite://${urls.sqlite}`;
    const input = JSON.stringify(statements.map((s) => rewrite(s.sql, db === "postgres" ? (n) => `$${n}` : () => "?")));
    const { spawnSync } = await import("node:child_process");
    const run = spawnSync(join(target, "release/sqlx-describe"), [db, url], { input, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
    const answers: Record<string, Answer> = {};
    if (run.status !== 0) { for (const s of statements) answers[s.key] = failed(run.stderr || "it could not connect"); return { answers, wrote: [] }; }
    let wrote: string[] = [];
    if (optionAccepted === null && db === "postgres") { const c = checkOption(urls.postgres); optionAccepted = c.accepted; wrote = c.wrote; }
    const described = JSON.parse(run.stdout);
    statements.forEach((s, i) => {
      const d = described[i];
      if (!d.ok) { answers[s.key] = failed(d.error); return; }
      const count = new Set([...s.sql.matchAll(/\$(\d+)/g)].map((m) => m[1])).size;
      // Where the database names no types, the macro checks only how many arguments there are. Every argument may be T or Option<T>.
      const params: TypedValue[] = Array.isArray(d.params) ? d.params.map((t: string) => ({ type: `${t} or Option<${t}>`, kind: kindOf(t), nullable: optionAccepted !== false })) : Array.from({ length: count }, () => ({ type: "any value (it counts arguments only)", kind: "any" as Kind, nullable: true }));
      const columns = d.columns.map((c: any) => { const nullable = c.nullable !== false; return { name: c.name, type: nullable ? `Option<${c.type}>` : c.type, kind: kindOf(c.type), nullable }; });
      answers[s.key] = { ok: true, params, columns, cardinality: null };
    });
    return { answers, wrote };
  },
};
