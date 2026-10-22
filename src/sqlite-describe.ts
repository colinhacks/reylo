// The SQLite side of the host: node:sqlite (built into Node ≥ 22.5, so no peer dependency)
// opens the project's database read-only and runs the two queries the engine asks for, the schema
// DDL from `sqlite_schema` and the functions this SQLite has from `pragma_function_list`. The
// engine replays that DDL into its embedded SQLite and prepares the statement there, so no
// describe goes through this file. The function list is that of the SQLite inside the Node that
// runs the type check; a project that runs on another driver runs on that driver's SQLite, which
// src/sqlite-drivers.ts prepares each statement on too.
import { fileURLToPath } from "node:url";
import { isAbsolute, resolve } from "node:path";

export interface SqliteDescriber {
  /** Rows as arrays of values, in column order: the engine's schema and function queries. */
  values(sql: string, params?: unknown[]): Promise<unknown[][]>;
  close(): Promise<void>;
}

/** `file:./dev.db`, `file:///abs/dev.db`, `sqlite:./dev.db`, `:memory:` → a path, relative ones resolved against baseDir. */
export function sqlitePath(url: string, baseDir: string): string {
  if (url === ":memory:" || url === "file::memory:") return ":memory:";
  const raw = url.startsWith("file://") ? fileURLToPath(url) : url.replace(/^(file|sqlite):(\/\/)?/, "");
  return isAbsolute(raw) ? raw : resolve(baseDir, raw);
}

export async function openSqliteDescriber(url: string, baseDir: string): Promise<SqliteDescriber> {
  const { DatabaseSync } = (await import("node:sqlite")) as typeof import("node:sqlite");
  const path = sqlitePath(url, baseDir);
  const db = new DatabaseSync(path, { readOnly: path !== ":memory:" });
  return {
    async values(sql, params = []) {
      const stmt = db.prepare(sql);
      const names = stmt.columns().map((c) => c.name);
      return (stmt.all(...(params as never[])) as Record<string, unknown>[]).map((row) => names.map((n) => row[n]));
    },
    async close() {
      db.close();
    },
  };
}

