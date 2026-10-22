// One analysis path for every consumer of a `.sql` file — the type check, the bundler plugins,
// the runtime hook and `reylo generate` — and one cache they share: `.reylo/` beside the project
// file, one entry per statement. The cache is only for speed. An entry records the schema it was
// analyzed against; while the database answers, an entry for another schema is analyzed again,
// and while it does not, or no URL names it, the entries are used as they are, so a build that
// cannot reach the database runs on what `reylo generate` or an earlier type check wrote.
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { anyEngine, engine, reason, AnswerCache, MissingEngine, Unreachable, type Analysis, type Engine, type EngineIo } from "./engine.ts";
import { packageDir, resolveProject, type ResolvedProject } from "./project.ts";
import { openPostgresDescriber } from "./describe.ts";
import { NotMysql, openMysqlDescriber } from "./mysql-describe.ts";
import { openSqliteDescriber, sqlitePath } from "./sqlite-describe.ts";

/** What `.reylo/` holds for one statement. */
export type Entry = {
  version: 1;
  dialect: ResolvedProject["dialect"];
  schema: string;
  analysis: Analysis;
  /** When it was written, in milliseconds since 1970: a copy of `.reylo/` keeps it where it may not keep the file's time. */
  at: number;
};

export type Outcome =
  | { ok: true; analysis: Analysis; source: "cache" | "database"; entry: string }
  | { ok: false; message: string; code: number };

type Io = EngineIo & { close(): Promise<void> };

const ENTRY_VERSION = 1;
// How long a schema fingerprint is trusted, as long as a statement answer: a migration shows up
// in a running dev server or editor within seconds. Once the database is found unreachable, the
// session stays offline for a minute instead of paying the connect timeout per file.
const SCHEMA_TTL = 5_000;
const OFFLINE_TTL = 60_000;
// A build step's session closes its connection a moment after its last file, so that a build
// process ends when its work does: the plugins whose bundler says when a build ends close it then
// (`releaseConnections`), and a loader (webpack, Rspack, Turbopack) has no such signal. The type
// check's session keeps its connection.
const BUILD_IDLE = 250;

const sessions = new Map<string, Session>();

/**
 * The project a directory belongs to, its file read with whichever database's engine is installed,
 * and the engine of its own database, or why that cannot be loaded.
 */
async function openProject(dir: string, env: Record<string, string | undefined>): Promise<{ project: ResolvedProject; engine: Engine | MissingEngine }> {
  const project = resolveProject(await anyEngine(dir), dir, env);
  const own = await engine(project.dialect, project.root).catch((e: unknown) => (e instanceof MissingEngine ? e : new MissingEngine((e as Error).message)));
  return { project, engine: own };
}

export class Session {
  readonly project: ResolvedProject;
  /** The project's directory, which holds `.reylo/`. */
  readonly root: string;
  /** The engine of the project's database, or why there is none, which an analysis reports. */
  readonly #engine: Engine | MissingEngine;
  readonly #engineFingerprint: string;
  readonly #answers = new AnswerCache();
  readonly #idle: number;
  #io: Promise<Io> | undefined;
  #busy = 0;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #schema: { value: string | null; at: number; error?: string; fatal?: boolean } | undefined;

  private constructor(engine: Engine | MissingEngine, project: ResolvedProject, idle: number) {
    this.#engine = engine;
    this.project = project;
    this.root = project.root;
    this.#idle = idle;
    const ping = engine instanceof MissingEngine ? undefined : engine.call({ op: "ping" });
    this.#engineFingerprint = ping?.status === "ok" ? ping.fingerprint : "unknown";
  }

  /** The build steps' session for the project a file or directory belongs to: one per project (and connection) and process. */
  static async for(from: string, env: Record<string, string | undefined> = process.env): Promise<Session> {
    // From the package's directory, so that a message names a file as the project sees it.
    const { project, engine } = await openProject(packageDir(from), env);
    const key = JSON.stringify([project.root, project.url ?? null, project.error ?? null, engine instanceof MissingEngine ? engine.message : null]);
    let session = sessions.get(key);
    if (!session) sessions.set(key, (session = new Session(engine, project, BUILD_IDLE)));
    return session;
  }

  /** A session of its own, for a caller that must not share one (the type check, a test); it keeps its connection until closed. */
  static async open(from: string, env: Record<string, string | undefined> = process.env, idle = Infinity): Promise<Session> {
    const { project, engine } = await openProject(from, env);
    return new Session(engine, project, idle);
  }

  /** Every build-step session this process opened, for a caller that ends them (`reylo generate`). */
  static opened(): Session[] {
    return [...sessions.values()];
  }

  lex(sql: string) {
    if (this.#engine instanceof MissingEngine) throw this.#engine;
    return this.#engine.lex(sql, this.project.dialect);
  }

  /**
   * Where this session's entry for a statement lives, whether or not it exists yet: named by the
   * statement, then by the server. Two databases keep an entry each, since a Postgres fingerprint
   * holds object ids, so a dev and a test database built by the same migrations still differ. A
   * session that cannot reach its database, or has no URL, reads another server's entry for the
   * statement where it has none of its own (`#offlineEntry`).
   */
  entryFile(sql: string): string {
    return join(this.root, ".reylo", "queries", `${this.#statementKey(sql)}-${sha256(this.#server()).slice(0, 16)}.json`);
  }

  #statementKey(sql: string): string {
    return sha256(JSON.stringify([ENTRY_VERSION, this.#engineFingerprint, this.project.dialect, sql])).slice(0, 32);
  }

  /** The statement's entry from this session's server, else the newest from any other: for a session that cannot reach its database, or has no URL. */
  #offlineEntry(sql: string): { file: string; entry: Entry } | undefined {
    const own = this.entryFile(sql);
    const entry = readEntry(own);
    if (entry) return { file: own, entry };
    const dir = join(this.root, ".reylo", "queries");
    const prefix = `${this.#statementKey(sql)}-`;
    let names: string[];
    try {
      names = readdirSync(dir).filter((n) => n.startsWith(prefix) && n.endsWith(".json"));
    } catch {
      return undefined;
    }
    // Newest by the time the entry records, since a copy (`cp -r`, `fs.cpSync`, a CI artifact) may
    // reset the file's.
    let newest: { file: string; entry: Entry } | undefined;
    for (const file of names.map((n) => join(dir, n))) {
      const other = readEntry(file);
      if (other && (!newest || (other.at ?? -Infinity) > (newest.entry.at ?? -Infinity))) newest = { file, entry: other };
    }
    return newest;
  }

  /** The statement's analysis: from `.reylo/` when the entry matches the schema, else from the database. */
  async analyze(sql: string, options: { force?: boolean } = {}): Promise<Outcome> {
    clearTimeout(this.#timer);
    this.#busy++;
    try {
      return await this.#analyze(sql, options);
    } finally {
      if (--this.#busy === 0 && this.#io && Number.isFinite(this.#idle)) this.#timer = setTimeout(() => this.#release(), this.#idle).unref();
    }
  }

  async #analyze(sql: string, options: { force?: boolean }): Promise<Outcome> {
    if (this.project.error) return { ok: false, message: this.project.error, code: 11 };
    // With no URL, `.reylo/` answers as it does where the database cannot be reached (the dialect
    // is the installed engine's), and a statement it lacks needs a URL.
    if (!this.project.url) {
      const hit = this.#engine instanceof MissingEngine || options.force ? undefined : this.#offlineEntry(sql);
      return hit ? { ok: true, analysis: hit.entry.analysis, source: "cache", entry: hit.file } : { ok: false, message: this.project.missingUrl, code: 4 };
    }
    // The URL names the database, whose engine package the project has to have installed.
    const engine = this.#engine;
    if (engine instanceof MissingEngine) return { ok: false, message: engine.message, code: 11 };
    const file = this.entryFile(sql);
    const schema = await this.schema();
    if (schema.fatal) return { ok: false, message: schema.error!, code: 5 };
    if (schema.value === null) {
      const hit = options.force ? undefined : this.#offlineEntry(sql);
      if (hit) return { ok: true, analysis: hit.entry.analysis, source: "cache", entry: hit.file };
      return { ok: false, message: `${this.cannotConnect(schema.error)}; .reylo has no analysis of this statement, so run reylo generate where the database can be reached.`, code: 5 };
    }
    const entry = options.force ? undefined : readEntry(file);
    if (entry && entry.schema === schema.value) return { ok: true, analysis: entry.analysis, source: "cache", entry: file };
    let analysis: Analysis;
    try {
      analysis = await engine.analyze(sql, this.project.dialect, await this.#connection(), this.#answers);
    } catch (e) {
      // A dead connection is reopened by the next call; a server error is reported.
      if (e instanceof Unreachable) {
        this.#io = undefined;
        this.#schema = { value: null, at: Date.now(), error: reason(e) };
        return { ok: false, message: this.cannotConnect(reason(e)), code: 5 };
      }
      if (e instanceof NotMysql) {
        this.#io = undefined;
        this.#schema = { value: null, at: Date.now(), error: this.notMysql(e), fatal: true };
        return { ok: false, message: this.#schema.error!, code: 5 };
      }
      return { ok: false, message: (e as Error).message, code: 5 };
    }
    writeEntry(this.root, file, { version: ENTRY_VERSION, dialect: this.project.dialect, schema: schema.value, analysis, at: Date.now() });
    return { ok: true, analysis, source: "database", entry: file };
  }

  /**
   * The fingerprint of the database's schema; null when the database cannot be reached, and
   * `fatal` when no connection can be made at all (no driver installed) or the server is one the type
   * check refuses (MariaDB, `NotMysql`). A server that refuses the
   * fingerprint query gets a value no entry matches, so every statement is analyzed live.
   */
  async schema(): Promise<{ value: string | null; error?: string; fatal?: boolean }> {
    const now = Date.now();
    if (this.#schema && now - this.#schema.at < (this.#schema.value === null ? OFFLINE_TTL : SCHEMA_TTL)) return this.#schema;
    let io: Io;
    try {
      io = await this.#connection();
    } catch (e) {
      this.#io = undefined;
      this.#schema = e instanceof Unreachable ? { value: null, at: now, error: reason(e) } : { value: null, at: now, error: e instanceof NotMysql ? this.notMysql(e) : (e as Error).message, fatal: true };
      return this.#schema;
    }
    try {
      const rows = await io.query(FINGERPRINT[this.project.dialect], []);
      const value = createHash("sha256").update(JSON.stringify(rows)).digest("hex").slice(0, 16);
      // The engine's answers describe the old schema too.
      if (this.#schema?.value && this.#schema.value !== value) this.#answers.clear();
      this.#schema = { value, at: now };
    } catch (e) {
      if (e instanceof Unreachable) {
        this.#io = undefined;
        this.#schema = { value: null, at: now, error: reason(e) };
      } else {
        this.#schema = { value: `unverified ${process.pid} ${now}`, at: now };
      }
    }
    return this.#schema;
  }

  /** Reads the schema's fingerprint again now, however recent the last read; whether it changed. */
  async recheck(): Promise<boolean> {
    const before = this.#schema?.value;
    if (this.#schema) this.#schema = { ...this.#schema, at: -Infinity };
    return (await this.schema()).value !== before;
  }

  /** Where the database was looked for, and where that came from; never the credentials. */
  cannotConnect(why?: string): string {
    const { dialect, url, root, urlSource } = this.project;
    if (dialect === "sqlite") return `Cannot open the database ${sqlitePath(url ?? "", root)} (${urlSource})${why ? `: ${why}` : ""}`;
    return `Cannot connect to the database at ${this.where()} (${urlSource})${why ? `: ${why}` : ""}`;
  }

  /** Why a server the type check cannot read is refused, and what to point it at instead. */
  notMysql(e: NotMysql): string {
    return `Cannot type-check against the database at ${this.where()} (${this.project.urlSource}): it is ${e.server}, and Reylo type-checks MySQL statements against MySQL. Point the type check's URL at a MySQL server, 8.4 or later, with the same schema.`;
  }

  /** The server, without credentials, that names this session's entries: none with no URL, which writes none. */
  #server(): string {
    const { dialect, url, root } = this.project;
    if (!url) return "";
    if (dialect === "sqlite") return sqlitePath(url, root);
    try {
      const u = new URL(url);
      return `${u.protocol}//${u.username}@${u.host}${u.pathname}`;
    } catch {
      return url;
    }
  }

  /** The database as a person names it: host, port and database, or the SQLite file. */
  where(): string {
    const { dialect, url, root } = this.project;
    if (dialect === "sqlite") return sqlitePath(url ?? "", root);
    try {
      const u = new URL(url ?? "");
      return `${u.hostname || u.searchParams.get("host") || "localhost"}${u.port ? `:${u.port}` : ""}${u.pathname}`;
    } catch {
      return "the database";
    }
  }

  async close(): Promise<void> {
    for (const [key, session] of sessions) if (session === this) sessions.delete(key);
    await this.release();
  }

  /** Closes the connection now, for a caller that knows no file is coming; the next analysis opens another. */
  async release(): Promise<void> {
    clearTimeout(this.#timer);
    await this.#release();
  }

  /** Closes the connection; the next analysis opens another. */
  async #release(): Promise<void> {
    const io = this.#io;
    this.#io = undefined;
    if (io) await io.then((i) => i.close()).catch(() => {});
  }

  #connection(): Promise<Io> {
    const { dialect, url, root } = this.project;
    this.#io ??= (async (): Promise<Io> => {
      if (dialect === "mysql") {
        const m = await openMysqlDescriber(root, url!);
        return { describe: (text) => m.describe(text), query: (text, params) => m.values(text, params), close: () => m.close() };
      }
      if (dialect === "sqlite") {
        // A file that cannot be opened is a database that cannot be reached: a build without it
        // runs on `.reylo/`.
        const s = await openSqliteDescriber(url!, root).catch((e) => Promise.reject(new Unreachable(e)));
        // The engine prepares SQLite statements inside its own SQLite; it never asks to describe.
        return { describe: () => Promise.reject(new Error("SQLite is described inside the engine")), query: (text, params) => s.values(text, params), close: () => s.close() };
      }
      const d = await openPostgresDescriber(root, url!);
      return { describe: (text) => d.describe(text), query: (text, params) => d.values(text, params), close: () => d.close() };
    })();
    const io = this.#io;
    io.catch(() => {
      if (this.#io === io) this.#io = undefined;
    });
    return io;
  }
}

/** Closes every build-step session's connection, for a build step whose build has ended: the process then ends without waiting out the idle time. */
export async function releaseConnections(): Promise<void> {
  await Promise.all([...sessions.values()].map((session) => session.release()));
}

const sha256 = (text: string) => createHash("sha256").update(text).digest("hex");

function readEntry(file: string): Entry | undefined {
  if (!existsSync(file)) return undefined;
  try {
    const entry = JSON.parse(readFileSync(file, "utf8")) as Entry;
    return entry.version === ENTRY_VERSION ? entry : undefined;
  } catch {
    return undefined;
  }
}

// Several processes write `.reylo/` at once (an editor's type check, a dev server, a build), so an
// entry is written to a file of its own and renamed into place: a reader sees a whole entry or
// none. `.reylo/.gitignore` keeps the directory out of git, as pytest's and ruff's caches do. The
// cache is only for speed, so a directory that cannot be written (a read-only checkout) is not an
// error.
function writeEntry(root: string, file: string, entry: Entry): void {
  const dir = join(root, ".reylo");
  const tmp = `${file}.${process.pid}.${Math.random().toString(36).slice(2)}.tmp`;
  try {
    mkdirSync(join(dir, "queries"), { recursive: true });
    const ignore = join(dir, ".gitignore");
    if (!existsSync(ignore)) writeFileSync(ignore, "# reylo's cache: written by the type check, the build steps and reylo generate; never committed.\n*\n");
    writeFileSync(tmp, JSON.stringify(entry) + "\n");
    renameSync(tmp, file);
  } catch {
    rmSync(tmp, { force: true });
  }
}

/**
 * One query per dialect whose rows change when anything an analysis reads changes: relations and
 * their columns, inheritance, constraints, indexes, row triggers and rules (nullability and the row
 * count read them), enum labels, domains, functions, user operators (the trust check reads them) and view
 * definitions, and the server settings that change a statement's types. SQLite's whole schema is
 * its `sqlite_schema` table, and the functions of the SQLite that opened it are part of what an
 * analysis reads: another Node has another SQLite. A `"char"` column is cast to text:
 * `text || "char"` is ambiguous, and a query the server refuses leaves every entry unverified.
 */
const NS = `not in ('pg_catalog', 'information_schema') and n.nspname !~ '^pg_(toast|temp)'`;
const FINGERPRINT: Record<ResolvedProject["dialect"], string> = {
  postgres: `select md5(coalesce(string_agg(x, E'\\n' order by x), '')) from (
    select 'v ' || current_setting('server_version_num') || ' ' || current_setting('search_path') as x
    union all select 'r ' || c.oid || ' ' || n.nspname || '.' || c.relname || ' ' || c.relkind::text from pg_class c join pg_namespace n on n.oid = c.relnamespace where n.nspname ${NS}
    union all select 'a ' || a.attrelid || ' ' || a.attnum || ' ' || a.attname || ' ' || a.atttypid || ' ' || a.attnotnull || ' ' || a.attcollation from pg_attribute a join pg_class c on c.oid = a.attrelid join pg_namespace n on n.oid = c.relnamespace where a.attnum > 0 and not a.attisdropped and n.nspname ${NS}
    union all select 'h ' || h.inhrelid || ' ' || h.inhparent || ' ' || h.inhseqno from pg_inherits h join pg_class c on c.oid = h.inhrelid join pg_namespace n on n.oid = c.relnamespace where n.nspname ${NS}
    union all select 'c ' || o.conrelid || ' ' || o.contypid || ' ' || o.conname || ' ' || pg_get_constraintdef(o.oid) from pg_constraint o join pg_namespace n on n.oid = o.connamespace where n.nspname ${NS}
    union all select 'i ' || i.indexrelid || ' ' || i.indisvalid || ' ' || pg_get_indexdef(i.indexrelid) from pg_index i join pg_class c on c.oid = i.indrelid join pg_namespace n on n.oid = c.relnamespace where n.nspname ${NS}
    union all select 'g ' || g.tgrelid || ' ' || g.tgname || ' ' || g.tgtype || ' ' || g.tgenabled::text from pg_trigger g join pg_class c on c.oid = g.tgrelid join pg_namespace n on n.oid = c.relnamespace where not g.tgisinternal and n.nspname ${NS}
    union all select 'u ' || w.ev_class || ' ' || w.rulename || ' ' || w.ev_type::text || ' ' || w.ev_enabled::text || ' ' || md5(pg_get_ruledef(w.oid)) from pg_rewrite w join pg_class c on c.oid = w.ev_class join pg_namespace n on n.oid = c.relnamespace where w.rulename <> '_RETURN' and n.nspname ${NS}
    union all select 'o ' || o.oid || ' ' || o.oprname || ' ' || o.oprleft || ' ' || o.oprright || ' ' || o.oprcode::oid from pg_operator o where o.oid >= 16384
    union all select 'e ' || e.enumtypid || ' ' || e.enumsortorder || ' ' || e.enumlabel from pg_enum e
    union all select 't ' || t.oid || ' ' || t.typname || ' ' || t.typtype::text || ' ' || t.typbasetype || ' ' || t.typnotnull from pg_type t join pg_namespace n on n.oid = t.typnamespace where n.nspname ${NS}
    union all select 'f ' || p.oid || ' ' || p.proname || ' ' || p.prorettype || ' ' || p.proretset || ' ' || p.proargtypes::text || ' ' || md5(coalesce(p.prosrc, '')) from pg_proc p join pg_namespace n on n.oid = p.pronamespace where n.nspname ${NS}
    union all select 'w ' || c.oid || ' ' || md5(pg_get_viewdef(c.oid)) from pg_class c join pg_namespace n on n.oid = c.relnamespace where c.relkind in ('v', 'm') and n.nspname ${NS}
  ) t`,
  mysql: `select concat_ws(' ', @@version, @@sql_mode, count(*), sum(crc32(x)), bit_xor(crc32(x))) from (
    select concat_ws(' ', 'c', table_name, column_name, ordinal_position, column_type, is_nullable, column_key, extra, coalesce(generation_expression, '')) as x from information_schema.columns where table_schema = database()
    union all select concat_ws(' ', 's', table_name, index_name, non_unique, seq_in_index, column_name) from information_schema.statistics where table_schema = database()
    union all select concat_ws(' ', 'v', table_name, md5(view_definition)) from information_schema.views where table_schema = database()
    union all select concat_ws(' ', 'r', routine_name, routine_type, coalesce(dtd_identifier, ''), md5(coalesce(routine_definition, ''))) from information_schema.routines where routine_schema = database()
  ) t`,
  sqlite: `select type, name, tbl_name, coalesce(sql, '') from sqlite_schema
    union all select 'function', name, type, narg from pragma_function_list order by 1, 2, 3, 4`,
};
