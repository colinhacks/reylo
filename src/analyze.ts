// One analysis path for every consumer of a `.sql` file — the type check, the bundler plugins,
// the runtime hook and `reylo generate` — and one cache they share: `.reylo/` beside the project
// file, one entry per statement. The cache is only for speed. An entry records the schema it was
// analyzed against; while the database answers, an entry for another schema is analyzed again,
// and while it does not, or no URL names it, the entries are used as they are, so a build that
// cannot reach the database runs on what `reylo generate` or an earlier type check wrote.
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { anyEngine, engine, reason, MissingEngine, Unreachable, type Analysis, type Engine, type Lexed } from "./engine.ts";
import { packageDir, resolveProject, sqlitePath, type ResolvedProject } from "./project.ts";
import { NotMysql, refusedLogin } from "./connection.ts";
import { openProjectDrivers, placeRefusal, type SqliteDriver } from "./sqlite-drivers.ts";

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

/**
 * A connection the engine holds (src/engine.ts): `analyze` runs a statement's whole analysis over
 * it, `query` runs the schema's fingerprint, and `forget` drops the answers the engine keeps once
 * the schema changes. On SQLite, `drivers` are the app's own, which each statement is prepared on
 * too (src/sqlite-drivers.ts).
 */
type Io = { analyze(sql: string): Promise<Analysis>; query(sql: string, params: unknown[]): Promise<unknown[][]>; forget(): Promise<void>; close(): Promise<void>; drivers?: SqliteDriver[] };

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
  /** The project as resolved when the session opened, or again once a failed `direct_url` command succeeds (`#reread`). */
  project!: ResolvedProject;
  /** The project's directory, which holds `.reylo/`. */
  readonly root: string;
  /** The engine of the project's database, or why there is none, which an analysis reports. */
  #engine!: Engine | MissingEngine;
  #engineFingerprint!: string;
  readonly #idle: number;
  /** Where the project was resolved from, and with what environment, for `#reread`. */
  readonly #dir: string;
  readonly #env: Record<string, string | undefined>;
  #io: Promise<Io> | undefined;
  /** The URL the connection opens: the project's, until a command in `direct_url` gives a fresh one. */
  #url: string | undefined;
  #busy = 0;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #schema: { value: string | null; at: number; error?: string; fatal?: boolean } | undefined;

  private constructor(engine: Engine | MissingEngine, project: ResolvedProject, idle: number, dir: string, env: Record<string, string | undefined>) {
    this.root = project.root;
    this.#idle = idle;
    this.#dir = dir;
    this.#env = env;
    this.#adopt(engine, project);
  }

  #adopt(engine: Engine | MissingEngine, project: ResolvedProject): void {
    this.#engine = engine;
    this.project = project;
    this.#url = project.url;
    const ping = engine instanceof MissingEngine ? undefined : engine.call({ op: "ping" });
    this.#engineFingerprint = ping?.status === "ok" ? ping.fingerprint : "unknown";
  }

  /**
   * Reads the project again while a `direct_url` command's failure is its error, as when a
   * password manager was locked: the command runs again once its failure has stood a minute
   * (src/project.ts), so that a session kept open, as the type check's is, recovers without a
   * restart.
   */
  async #reread(): Promise<void> {
    if (!this.project.error || !this.project.refreshUrl) return;
    const { project, engine } = await openProject(this.#dir, this.#env);
    if (project.error) return;
    this.#adopt(engine, project);
    this.#schema = undefined;
  }

  /** The build steps' session for the project a file or directory belongs to: one per project (and connection) and process. */
  static async for(from: string, env: Record<string, string | undefined> = process.env): Promise<Session> {
    // From the package's directory, so that a message names a file as the project sees it.
    const dir = packageDir(from);
    const { project, engine } = await openProject(dir, env);
    // A setting the build step reads (`sqlcommenter`) is part of the key, so a change to it in the
    // project file reaches a running watch process.
    const key = JSON.stringify([project.root, project.url ?? null, project.error ?? null, project.sqlcommenter, engine instanceof MissingEngine ? engine.message : null]);
    let session = sessions.get(key);
    if (!session) sessions.set(key, (session = new Session(engine, project, BUILD_IDLE, dir, env)));
    return session;
  }

  /** A session of its own, for a caller that must not share one (the type check, a test); it keeps its connection until closed. */
  static async open(from: string, env: Record<string, string | undefined> = process.env, idle = Infinity): Promise<Session> {
    const { project, engine } = await openProject(from, env);
    return new Session(engine, project, idle, from, env);
  }

  /** Every build-step session this process opened, for a caller that ends them (`reylo generate`). */
  static opened(): Session[] {
    return [...sessions.values()];
  }

  lex(sql: string): Lexed {
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

  /**
   * The statement's analysis: from `.reylo/` when the entry matches the schema, else from the
   * database. `store: false` neither reads nor writes `.reylo/`, for a statement that is no file of
   * the project (`reylo play`'s, analyzed at each keystroke).
   */
  async analyze(sql: string, options: { force?: boolean; store?: boolean } = {}): Promise<Outcome> {
    clearTimeout(this.#timer);
    this.#busy++;
    try {
      await this.#reread();
      const outcome = await this.#analyze(sql, options);
      return outcome.ok && this.project.dialect === "sqlite" ? await this.#onDrivers(sql, outcome) : outcome;
    } finally {
      if (--this.#busy === 0 && this.#io && Number.isFinite(this.#idle)) this.#timer = setTimeout(() => this.#release(), this.#idle).unref();
    }
  }

  /** Rows of a catalog query on the session's connection, as arrays of values: what `reylo play` completes. */
  async query(sql: string, params: unknown[] = []): Promise<unknown[][]> {
    return (await this.#connection()).query(sql, params);
  }

  /**
   * A SQLite statement prepared on the app's own drivers too, whose SQLite may be older than the
   * engine's: the first that refuses it is an error at the place SQLite names. Not kept in
   * `.reylo/`, since the drivers are the project's and not the database's; a session that cannot
   * open the database checks nothing. One whose connection was released while its schema is still
   * trusted opens it again, so a `.reylo/` hit is checked however long the session sat idle.
   */
  async #onDrivers(sql: string, outcome: Extract<Outcome, { ok: true }>): Promise<Outcome> {
    const { analysis } = outcome;
    if (analysis.diagnostics.length || this.#schema?.value == null) return outcome;
    const io = await this.#connection().catch(() => undefined);
    for (const driver of io?.drivers ?? []) {
      const refusal = await driver.refuses(analysis.rewritten.sql, analysis.rewritten.order.length);
      if (refusal === undefined) continue;
      const { start, length } = placeRefusal(sql, refusal);
      const message = driver.says(refusal);
      return { ...outcome, analysis: { ...analysis, diagnostics: [{ message, start, length, code: 6 }] } };
    }
    return outcome;
  }

  async #analyze(sql: string, options: { force?: boolean; store?: boolean }): Promise<Outcome> {
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
    const entry = options.force || options.store === false ? undefined : readEntry(file);
    if (entry && entry.schema === schema.value) return { ok: true, analysis: entry.analysis, source: "cache", entry: file };
    let analysis: Analysis;
    const io = this.#connection();
    try {
      analysis = await (await io).analyze(sql);
    } catch (e) {
      // A dead connection is reopened by the next call; a server error is reported.
      if (e instanceof Unreachable) {
        this.#drop(io);
        this.#schema = { value: null, at: Date.now(), error: reason(e) };
        return { ok: false, message: this.cannotConnect(reason(e)), code: 5 };
      }
      if (e instanceof NotMysql) {
        this.#drop(io);
        this.#schema = { value: null, at: Date.now(), error: this.notMysql(e), fatal: true };
        return { ok: false, message: this.#schema.error!, code: 5 };
      }
      return { ok: false, message: (e as Error).message, code: 5 };
    }
    if (options.store !== false) writeEntry(this.root, file, { version: ENTRY_VERSION, dialect: this.project.dialect, schema: schema.value, analysis, at: Date.now() });
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
    const pending = this.#connection();
    let io: Io;
    try {
      io = await pending;
    } catch (e) {
      this.#drop(pending);
      this.#schema = e instanceof Unreachable ? { value: null, at: now, error: reason(e) } : { value: null, at: now, error: e instanceof NotMysql ? this.notMysql(e) : (e as Error).message, fatal: true };
      return this.#schema;
    }
    try {
      const rows = await io.query(FINGERPRINT[this.project.dialect], []);
      const value = createHash("sha256").update(JSON.stringify(rows)).digest("hex").slice(0, 16);
      // The engine's answers describe the old schema too.
      if (this.#schema?.value && this.#schema.value !== value) await io.forget().catch(() => {});
      this.#schema = { value, at: now };
    } catch (e) {
      if (e instanceof Unreachable) {
        this.#drop(pending);
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
    this.#io ??= (async (): Promise<Io> => {
      let first: Io;
      try {
        first = await this.#open(this.#url!);
      } catch (e) {
        const fresh = this.#freshUrl(e);
        if (fresh === undefined) throw e;
        first = await this.#open(fresh);
      }
      if (!this.project.refreshUrl) return first;
      // A refused login can surface at any call, where a token expired while the session was open:
      // the call is asked once more on a connection with the command's fresh output. The calls refused
      // together, as every call waiting when a token expires is, share one reopen, so that the
      // command runs once and one connection replaces the old; a call that fails on a connection
      // already replaced is asked again on the one that replaced it.
      let current: Promise<Io> = Promise.resolve(first);
      const again = async <T>(call: (on: Io) => Promise<T>): Promise<T> => {
        const used = current;
        const on = await used;
        try {
          return await call(on);
        } catch (e) {
          if (current === used) {
            if (!refusedLogin(e)) throw e;
            current = (async () => {
              await on.close().catch(() => {});
              const fresh = this.#freshUrl(e);
              if (fresh === undefined) throw e;
              return this.#open(fresh);
            })();
            current.catch(() => {});
          }
          return call(await current);
        }
      };
      return {
        analyze: (sql) => again((on) => on.analyze(sql)),
        query: (text, params) => again((on) => on.query(text, params)),
        forget: () => again((on) => on.forget()),
        close: () => current.then((on) => on.close(), () => {}),
        ...(first.drivers && { drivers: first.drivers }),
      };
    })();
    const io = this.#io;
    io.catch(() => {
      if (this.#io === io) this.#io = undefined;
    });
    return io;
  }

  /**
   * Forgets a connection a call found dead, and closes it, since one opened in its place with a
   * command's fresh output may be live. Only the connection that call ran on: a call that fails
   * late never ends one opened since.
   */
  #drop(io: Promise<Io>): void {
    if (this.#io !== io) return;
    this.#io = undefined;
    io.then((i) => i.close()).catch(() => {});
  }

  /**
   * A URL to try instead, when the server refused the login and `direct_url`'s command now prints
   * something else, as when a short-lived password has expired; else undefined, so that nothing
   * is tried twice on the same password.
   */
  #freshUrl(e: unknown): string | undefined {
    if (!refusedLogin(e)) return undefined;
    const fresh = this.project.refreshUrl?.();
    if (fresh === undefined || fresh === this.#url) return undefined;
    this.#url = fresh;
    return fresh;
  }

  async #open(url: string): Promise<Io> {
    const { dialect, root } = this.project;
    // The URL names a database whose engine the project lacks: its error, as the import's.
    if (this.#engine instanceof MissingEngine) throw this.#engine;
    // The engine opens the database itself: on SQLite the file the project names, and the statement
    // is prepared on the app's own SQLite drivers too, which are the app's, not the engine's.
    const target = dialect === "sqlite" ? sqlitePath(url, root) : url;
    const c = await this.#engine.open(target);
    let drivers: SqliteDriver[] | undefined;
    try {
      drivers = dialect === "sqlite" ? await openProjectDrivers(root, target) : undefined;
    } catch (e) {
      await c.close();
      throw e;
    }
    return {
      analyze: (sql) => c.analyze(sql),
      query: (text, params) => c.query(text, params),
      forget: () => c.forget(),
      close: async () => {
        await Promise.all((drivers ?? []).map((d) => d.close().catch(() => {})));
        await c.close();
      },
      ...(drivers && { drivers }),
    };
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
 * count read them), enum labels, domains, functions with their defaults, volatility, strictness,
 * kind and variadic argument, user operators and casts (the trust check reads them), view
 * definitions and column comments (a field's doc comment), and the server settings that change a statement's types;
 * and what the errors a statement may raise are read from: defaults (a domain's too), generated columns, policies and
 * operator classes on Postgres, and triggers, foreign keys, checks and every schema's stored functions on MySQL. SQLite's whole schema is
 * its `sqlite_schema` table, and the functions of the SQLite that opened it are part of what an
 * analysis reads: another Node has another SQLite. A `"char"` column is cast to text:
 * `text || "char"` is ambiguous, and a query the server refuses leaves every entry unverified.
 */
// The Postgres query names every operator, function, type and table as pg_catalog's, and converts a
// value to text only by a built-in cast (a name, a "char", a boolean) or by its type's output
// function (`textin(oidout(…))`), as the engine's catalog queries do (engine/postgres/src/postgres.rs):
// under a search path that puts a user's schema before pg_catalog, a user's `||` that answers NULL
// would leave every row NULL, and the fingerprint would never move. Each row is the text the plain
// `||` of each field makes. The server's own `pg_get_viewdef` and `pg_get_ruledef` read a rule
// through a query of bare operators, which a user's `=` of oids answers too: a view's or a rule's
// row is then NULL, and left out.
const NS = `not (n.nspname operator(pg_catalog.=) any ('{pg_catalog,information_schema}'::pg_catalog.name[])) and n.nspname operator(pg_catalog.!~) '^pg_(toast|temp)'::pg_catalog.text`;
const cat = (...parts: string[]) => parts.join(" operator(pg_catalog.||) ");
const lit = (s: string) => `'${s}'::pg_catalog.text`;
const line = (tag: string, ...fields: string[]) => cat(lit(`${tag} `), ...fields.flatMap((f, i) => (i ? [lit(" "), f] : [f])));
const out = (type: string, value: string) => `pg_catalog.textin(pg_catalog.${type}out(${value}))`;
const txt = (value: string) => `${value}::pg_catalog.text`;
const md5 = (value: string) => `pg_catalog.md5(${value})`;
const or = (value: string) => `coalesce(${value}, ''::pg_catalog.text)`;
const eq = (a: string, b: string) => `${a} operator(pg_catalog.=) ${b}`;
const rel = (alias: string, column: string) => `join pg_catalog.pg_class c on ${eq("c.oid", `${alias}.${column}`)} join pg_catalog.pg_namespace n on ${eq("n.oid", "c.relnamespace")}`;
const PG_CLASS = "'pg_catalog.pg_class'::pg_catalog.regclass::pg_catalog.oid";
const USER = "'16384'::pg_catalog.oid";
const POSTGRES_FINGERPRINT = `select pg_catalog.md5(coalesce(pg_catalog.string_agg(t.x, E'\\n'::pg_catalog.text order by t.x), ''::pg_catalog.text)) from (
    select ${line("v", "pg_catalog.current_setting('server_version_num'::pg_catalog.text)", "pg_catalog.current_setting('search_path'::pg_catalog.text)")} as x
    union all select ${line("r", out("oid", "c.oid"), cat(txt("n.nspname"), lit("."), txt("c.relname")), txt("c.relkind"))} from pg_catalog.pg_class c join pg_catalog.pg_namespace n on ${eq("n.oid", "c.relnamespace")} where ${NS}
    union all select ${line("a", out("oid", "a.attrelid"), out("int2", "a.attnum"), txt("a.attname"), out("oid", "a.atttypid"), txt("a.attnotnull"), out("oid", "a.attcollation"))} from pg_catalog.pg_attribute a ${rel("a", "attrelid")} where a.attnum operator(pg_catalog.>) 0 and not a.attisdropped and ${NS}
    union all select ${line("h", out("oid", "h.inhrelid"), out("oid", "h.inhparent"), out("int4", "h.inhseqno"))} from pg_catalog.pg_inherits h ${rel("h", "inhrelid")} where ${NS}
    union all select ${line("c", out("oid", "o.conrelid"), out("oid", "o.contypid"), txt("o.conname"), "pg_catalog.pg_get_constraintdef(o.oid)")} from pg_catalog.pg_constraint o join pg_catalog.pg_namespace n on ${eq("n.oid", "o.connamespace")} where ${NS}
    union all select ${line("i", out("oid", "i.indexrelid"), txt("i.indisvalid"), "pg_catalog.pg_get_indexdef(i.indexrelid)")} from pg_catalog.pg_index i ${rel("i", "indrelid")} where ${NS}
    union all select ${line("g", out("oid", "g.tgrelid"), txt("g.tgname"), out("int2", "g.tgtype"), txt("g.tgenabled"))} from pg_catalog.pg_trigger g ${rel("g", "tgrelid")} where not g.tgisinternal and ${NS}
    union all select ${line("u", out("oid", "w.ev_class"), txt("w.rulename"), txt("w.ev_type"), txt("w.ev_enabled"), md5("pg_catalog.pg_get_ruledef(w.oid)"))} from pg_catalog.pg_rewrite w ${rel("w", "ev_class")} where w.rulename operator(pg_catalog.<>) '_RETURN'::pg_catalog.name and ${NS}
    union all select ${line("o", out("oid", "o.oid"), txt("o.oprname"), out("oid", "o.oprleft"), out("oid", "o.oprright"), out("oid", "o.oprcode::pg_catalog.oid"))} from pg_catalog.pg_operator o where o.oid operator(pg_catalog.>=) ${USER}
    union all select ${line("e", out("oid", "e.enumtypid"), out("float4", "e.enumsortorder"), txt("e.enumlabel"))} from pg_catalog.pg_enum e
    union all select ${line("t", out("oid", "t.oid"), txt("t.typname"), txt("t.typtype"), out("oid", "t.typbasetype"), txt("t.typnotnull"), md5(or("t.typdefault")))} from pg_catalog.pg_type t join pg_catalog.pg_namespace n on ${eq("n.oid", "t.typnamespace")} where ${NS}
    union all select ${line("f", out("oid", "p.oid"), txt("p.proname"), out("oid", "p.prorettype"), txt("p.proretset"), out("oidvector", "p.proargtypes"), md5(or("p.prosrc")), txt("p.prokind"), txt("p.provolatile"), txt("p.proisstrict"), out("int2", "p.pronargdefaults"), out("oid", "p.provariadic"))} from pg_catalog.pg_proc p join pg_catalog.pg_namespace n on ${eq("n.oid", "p.pronamespace")} where ${NS}
    union all select ${line("k", out("oid", "k.oid"), out("oid", "k.castsource"), out("oid", "k.casttarget"), out("oid", "k.castfunc"), txt("k.castcontext"), txt("k.castmethod"))} from pg_catalog.pg_cast k where k.oid operator(pg_catalog.>=) ${USER}
    union all select ${line("w", out("oid", "c.oid"), md5("pg_catalog.pg_get_viewdef(c.oid)"))} from pg_catalog.pg_class c join pg_catalog.pg_namespace n on ${eq("n.oid", "c.relnamespace")} where c.relkind operator(pg_catalog.=) any ('{v,m}'::pg_catalog."char"[]) and ${NS}
    union all select ${line("x", out("oid", "d.adrelid"), out("int2", "d.adnum"), md5("pg_catalog.pg_get_expr(d.adbin, d.adrelid)"))} from pg_catalog.pg_attrdef d ${rel("d", "adrelid")} where ${NS}
    union all select ${line("p", out("oid", "p.polrelid"), txt("p.polname"), txt("p.polcmd"), txt("p.polpermissive"), md5(cat(or("pg_catalog.pg_get_expr(p.polqual, p.polrelid)"), lit(" "), or("pg_catalog.pg_get_expr(p.polwithcheck, p.polrelid)"))))} from pg_catalog.pg_policy p ${rel("p", "polrelid")} where ${NS}
    union all select ${line("q", out("oid", "q.oid"), txt("q.opcname"), out("oid", "q.opcmethod"), out("oid", "q.opcintype"))} from pg_catalog.pg_opclass q where q.oid operator(pg_catalog.>=) ${USER}
    union all select ${line("d", out("oid", "d.objoid"), out("int4", "d.objsubid"), md5("d.description"))} from pg_catalog.pg_description d ${rel("d", "objoid")} where ${eq("d.classoid", PG_CLASS)} and d.objsubid operator(pg_catalog.>) 0 and ${NS}
  ) t`;
const FINGERPRINT: Record<ResolvedProject["dialect"], string> = {
  postgres: POSTGRES_FINGERPRINT,
  mysql: `select concat_ws(' ', @@version, @@sql_mode, count(*), sum(crc32(x)), bit_xor(crc32(x))) from (
    select concat_ws(' ', 'c', table_name, column_name, ordinal_position, column_type, is_nullable, column_key, extra, coalesce(generation_expression, ''), md5(column_comment)) as x from information_schema.columns where table_schema = database()
    union all select concat_ws(' ', 's', table_name, index_name, non_unique, seq_in_index, column_name) from information_schema.statistics where table_schema = database()
    union all select concat_ws(' ', 'v', table_name, md5(view_definition)) from information_schema.views where table_schema = database()
    union all select concat_ws(' ', 'r', routine_name, routine_type, coalesce(dtd_identifier, ''), md5(coalesce(routine_definition, ''))) from information_schema.routines where routine_schema = database()
    union all select concat_ws(' ', 'p', routine_schema, routine_name) from information_schema.routines where routine_schema <> database() and routine_type = 'FUNCTION'
    union all select concat_ws(' ', 'g', event_object_table, trigger_name, event_manipulation, action_timing) from information_schema.triggers where event_object_schema = database()
    union all select concat_ws(' ', 'f', table_name, constraint_name, referenced_table_name, update_rule, delete_rule) from information_schema.referential_constraints where constraint_schema = database()
    union all select concat_ws(' ', 'k', table_name, constraint_name, constraint_type) from information_schema.table_constraints where table_schema = database()
  ) t`,
  sqlite: `select type, name, tbl_name, coalesce(sql, '') from sqlite_schema
    union all select 'function', name, type, narg from pragma_function_list order by 1, 2, 3, 4`,
};
