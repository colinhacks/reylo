// sqlx-ts: `sqlx-ts <dir> -g` writes a .queries.ts file beside each source file, whose Params and Result types are what it inferred.
import { resolve, join } from "node:path";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { failed, gitDelta, gitInit, readTsDeclarations, readTsTuple, readTsType, rewrite, sh, workdir, type Answer, type Tool } from "./lib.ts";

const bin = process.env.SQLX_TS_BIN ?? resolve(import.meta.dirname, "../.out/sqlx-ts/bin/sqlx-ts");
// Built from the tag v0.49.0, whose binary still reports 0.48.0.
const version = "0.49.0";
const TYPE = { postgres: "postgres", mysql: "mysql", sqlite: "sqlite" } as const;

export const sqlxts: Tool = {
  id: "sqlxts",
  name: "sqlx-ts",
  version,
  dbs: ["postgres", "mysql", "sqlite"],
  noDuckdb: 'Its `--db-type` takes Postgres, MySQL and SQLite.',
  cardinalityNote: "It types the row only, and how many rows come back is left to you.",
  async analyze(db, statements, urls) {
    const url = db === "postgres" ? urls.postgres.replace("localhost", "127.0.0.1") : db === "mysql" ? urls.mysql : urls.sqlite;
    const config = JSON.stringify({ generate_types: { enabled: true }, connections: { default: db === "sqlite" ? { DB_TYPE: TYPE[db], DB_NAME: url } : { DB_TYPE: TYPE[db], DB_URL: url } } });
    const answers: Record<string, Answer> = {};
    let wrote: string[] = [];
    // One statement it cannot read stops a whole run, so each runs alone.
    for (const s of statements) {
      const dir = workdir("sqlxts");
      writeFileSync(join(dir, ".sqlxrc.json"), config);
      writeFileSync(join(dir, "q.ts"), `import { sql } from 'sqlx-ts'\nexport const q = sql\`${rewrite(s.sql, db === "postgres" ? (n) => `$${n}` : () => "?")}\`\n`);
      gitInit(dir);
      const run = sh(bin, [".", "--ext=ts", "-g"], { cwd: dir });
      const out = join(dir, "q.queries.ts");
      if (run.status !== 0 || !existsSync(out)) { answers[s.key] = failed(run.all.split("\n").filter((l) => !/^\[(INFO|WARN)\]/.test(l)).join(" ").trim()); continue; }
      wrote = gitDelta(dir);
      const d = readTsDeclarations(out);
      const params = d.QParams;
      const result = d.IQResult;
      // It prefixes a column read through a join with its table's name, and sorts the columns.
      const named = (name: string) => s.probe.columns?.find((c) => c.name === name || name.endsWith(`_${c.name}`))?.name ?? name;
      // On MySQL and SQLite an INSERT's parameters are nested per row.
      const flat = (t: string): string[] => readTsTuple(t).flatMap((x) => (x.startsWith("[") ? flat(x) : [x]));
      answers[s.key] = {
        ok: true,
        params: typeof params === "string" ? flat(params).map(readTsType) : [],
        columns: Array.isArray(result) ? result.map((f) => ({ name: named(f.name), ...readTsType(f.type) })) : [],
        cardinality: null,
      };
    }
    return { answers, wrote };
  },
};
