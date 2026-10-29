// The type check's DuckDB. DuckDB runs inside the app, so the engine's needs (a statement's
// describe, a catalog query) are answered by the project's own DuckDB, `@duckdb/node-api` loaded
// from the project, as its prepared statement reports a statement's types: the engine component
// carries no DuckDB (engine/native/src/duckdb.rs). The database file is opened read-only for each
// analysis and closed after it: DuckDB lets no other process open a file one process holds for
// writing, nor open it for writing while another process holds it at all, so a type check that kept
// it open would stop the app from starting. Where the app holds it, the type check reads a copy.
// A relative path in the statement (`read_parquet('logs/*.parquet')`) is read from the project's
// directory, where the app usually runs.
import { constants, copyFileSync, existsSync, mkdtempSync, rmSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { Unreachable } from "./connection.ts";
import type { DuckdbDescribed, DuckdbType, EngineIo } from "./engine-core.ts";

/** The slice of `@duckdb/node-api` the type check uses. */
type NodeApi = {
  DuckDBInstance: { create(path?: string, options?: Record<string, string>): Promise<Instance> };
  DuckDBTypeId: Record<number, string>;
  StatementType: Record<number, string>;
};
type Instance = { connect(): Promise<DbConnection>; closeSync(): void };
type DbConnection = {
  prepare(sql: string): Promise<Prepared>;
  runAndReadAll(sql: string, values?: unknown[]): Promise<{ getRowsJson(): unknown[][] }>;
  closeSync(): void;
};
type Prepared = {
  statementType: number;
  parameterCount: number;
  parameterName(i: number): string;
  parameterTypeId(i: number): number;
  parameterType(i: number): NodeType;
  columnCount: number;
  columnName(i: number): string;
  columnType(i: number): NodeType;
  destroySync?(): void;
};
type NodeType = {
  typeId: number;
  alias?: string;
  toString(): string;
  width?: number;
  scale?: number;
  valueType?: NodeType;
  length?: number;
  entryNames?: string[];
  entryTypes?: NodeType[];
  keyType?: NodeType;
  memberTags?: string[];
  memberTypes?: NodeType[];
  values?: string[];
};

/** Why the type check cannot read a DuckDB database: no `@duckdb/node-api` in the project. */
export class NoDuckdbDriver extends Error {
  constructor(root: string) {
    super(`DuckDB statements are typed on the project's own DuckDB, and ${root} has no @duckdb/node-api: npm install @duckdb/node-api`);
    this.name = "NoDuckdbDriver";
  }
}

const loaded = new Map<string, NodeApi>();

/** `@duckdb/node-api` as the project at `root` resolves it, loaded once per project. */
function nodeApi(root: string): NodeApi {
  let api = loaded.get(root);
  if (!api) {
    try {
      api = createRequire(join(root, "package.json"))("@duckdb/node-api") as NodeApi;
    } catch {
      throw new NoDuckdbDriver(root);
    }
    loaded.set(root, api);
  }
  return api;
}

/** The type as the engine reads it (engine/duckdb/src/api.rs). */
function logical(api: NodeApi, t: NodeType): DuckdbType {
  const out: DuckdbType = { id: api.DuckDBTypeId[t.typeId] ?? String(t.typeId), text: t.toString() };
  if (t.alias) out.alias = t.alias;
  if (t.width !== undefined && t.scale !== undefined) Object.assign(out, { width: t.width, scale: t.scale });
  if (t.keyType && t.valueType) return Object.assign(out, { key: logical(api, t.keyType), value: logical(api, t.valueType) });
  if (t.valueType) out.child = logical(api, t.valueType);
  if (t.length !== undefined && out.id === "ARRAY") out.size = t.length;
  if (t.entryNames && t.entryTypes) out.fields = t.entryNames.map((name, i) => ({ name, type: logical(api, t.entryTypes![i]!) }));
  if (t.memberTags && t.memberTypes) out.fields = t.memberTags.map((name, i) => ({ name, type: logical(api, t.memberTypes![i]!) }));
  if (t.values) out.values = [...t.values];
  return out;
}

/** The ids of a parameter whose type DuckDB could not tell from where it stands. */
const UNTYPED = new Set(["INVALID", "ANY", "SQLNULL"]);

/** What the prepared statement of `sql` reports, as the engine reads a describe. */
async function describe(api: NodeApi, c: DbConnection, sql: string): Promise<DuckdbDescribed> {
  const p = await c.prepare(sql);
  try {
    const params: DuckdbDescribed["params"] = [];
    for (let i = 1; i <= p.parameterCount; i++) {
      let type: DuckdbType | null = null;
      if (!UNTYPED.has(api.DuckDBTypeId[p.parameterTypeId(i)] ?? "")) {
        try {
          type = logical(api, p.parameterType(i));
        } catch {}
      }
      params.push({ name: p.parameterName(i), type });
    }
    const columns: DuckdbDescribed["columns"] = [];
    for (let i = 0; i < p.columnCount; i++) {
      // A column of a parameter DuckDB could not type (`select $1`) has no type node-api reads.
      let type: DuckdbType = { id: "INVALID", text: "" };
      try {
        type = logical(api, p.columnType(i));
      } catch {}
      columns.push({ name: p.columnName(i), type });
    }
    return { kind: "duckdb", statement: api.StatementType[p.statementType] ?? "", params, columns };
  } finally {
    p.destroySync?.();
  }
}

/** A database DuckDB cannot open (no such file, one it cannot read) is one that cannot be reached: a build runs on `.reylo/`. */
function opened(e: unknown): never {
  throw new Unreachable(e instanceof Error ? e : new Error(String(e)));
}

/** DuckDB's refusal of a file another process holds. */
const LOCKED = /Could not set lock on file/;

/**
 * DuckDB's refusal on Windows of a file another process holds, and that process. There DuckDB opens
 * a file it writes with no sharing at all, so no other process can read it, not even to copy it.
 */
const HELD_ON_WINDOWS = /File is already open in\s+(.*?\(PID \d+\))/;

/** The copy of each database file the type check reads while another process holds it, by the file's path. */
const copies = new Map<string, { key: string; dir: string; file: string }>();
let cleanup = false;

/**
 * A copy of the database file at `path` and its write-ahead log, as they stand: a clone where the
 * file system makes one (APFS, Btrfs, XFS), so that it takes no space until the app writes. It is
 * made again only when the file or its log has changed, or when `fresh`, and removed at exit.
 */
function copyOf(path: string, fresh = false): string {
  const wal = `${path}.wal`;
  const stamp = (p: string) => (existsSync(p) ? `${statSync(p).size}:${statSync(p).mtimeMs}` : "-");
  const key = `${stamp(path)}/${stamp(wal)}`;
  const held = copies.get(path);
  if (held && held.key === key && !fresh) return held.file;
  if (held) rmSync(held.dir, { recursive: true, force: true });
  const dir = mkdtempSync(join(tmpdir(), "reylo-duckdb-"));
  const file = join(dir, basename(path));
  copyFileSync(path, file, constants.COPYFILE_FICLONE);
  if (existsSync(wal)) copyFileSync(wal, `${file}.wal`, constants.COPYFILE_FICLONE);
  copies.set(path, { key, dir, file });
  if (!cleanup) {
    cleanup = true;
    process.once("exit", () => {
      for (const { dir } of copies.values()) rmSync(dir, { recursive: true, force: true });
    });
  }
  return file;
}

/** The copy of the database at `path`, opened: a copy made while the app writes the file can catch a write half done, so one more is made before the type check gives up. */
async function openCopy(api: NodeApi, path: string, options: Record<string, string>): Promise<Instance> {
  try {
    return await api.DuckDBInstance.create(copyOf(path), options);
  } catch {
    try {
      return await api.DuckDBInstance.create(copyOf(path, true), options);
    } catch (e) {
      opened(e);
    }
  }
}

/**
 * The DuckDB database at `path` (`:memory:` for an empty one), for the project at `root`: each
 * call opens it read-only, runs, and closes it, and where another process holds it for writing,
 * opens a copy of it instead ([`copyOf`]). A NoDuckdbDriver where the project has no
 * `@duckdb/node-api`.
 */
export function duckdbDatabase(path: string, root: string): { run<T>(work: (io: EngineIo) => Promise<T>): Promise<T> } {
  // A project without the driver can type nothing: its error is the project's, at once.
  const api = nodeApi(root);
  return {
    async run(work) {
      const options: Record<string, string> = { file_search_path: root };
      if (path !== ":memory:") options.access_mode = "READ_ONLY";
      const instance = await api.DuckDBInstance.create(path, options).catch((e: unknown) => {
        const message = e instanceof Error ? e.message : String(e);
        if (LOCKED.test(message)) return openCopy(api, path, options);
        const holder = HELD_ON_WINDOWS.exec(message)?.[1];
        if (holder) opened(new Error(`${holder} holds it for writing, and on Windows no other process can read it meanwhile, not even to copy it: type-check while that process is stopped`));
        return opened(e);
      });
      try {
        const c = await instance.connect().catch(opened);
        try {
          return await work({
            describe: (sql) => describe(api, c, sql),
            query: async (sql, params) => (await c.runAndReadAll(sql, params as never)).getRowsJson(),
          });
        } finally {
          c.closeSync();
        }
      } finally {
        instance.closeSync();
      }
    },
  };
}
