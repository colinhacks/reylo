// The analysis engine: the `reylo` executable (src/native.ts), which holds every database's engine
// and serves one database's as `reylo engine <database>`: one JSON request per line on its standard
// input, one answer per line on its standard output, each carrying its request's `id` back
// (engine/native/src/lib.rs). The engine connects to the database itself. Each connection is a
// child process of its own, which answers its requests in turn, so that a server slow to answer
// holds up no other connection; the requests that need no database go to one child per database
// (the ping, the lexer, and DuckDB's analysis, whose needs this process answers), and those that
// must answer before this thread can go on (the project file's `config`) to `reylo engine
// <database> call`, which answers the one request on its standard input and exits. A child keeps
// this process alive only while a request waits on it, as esbuild's service does, so that a build
// ends when its work does: the child and its pipes are unreferenced once, when it starts, and a
// timer holds the process while a request waits. Bun keeps a process alive for good once a child
// it unreferenced is referenced again in a Bun.build plugin's hook, which is where the esbuild
// plugin asks the engine, so the child is never referenced again; a timer is held and let go alike
// in every runtime.
import { spawn, spawnSync, type ChildProcessWithoutNullStreams } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { hostUrl, type HostUrl } from "./host-url.ts";
import { NotMysql, Unreachable } from "./connection.ts";
import { EngineCore, type Analysis, type Described, type Dialect, type Lexed, type Response } from "./engine-core.ts";
import { nativeBinary } from "./native.ts";

export type { EngineType } from "./engine-type.ts";
export { reason, Unreachable } from "./connection.ts";
export type { Analysis, Answer, DbError, Described, Diagnostic, Dialect, DuckdbDescribed, DuckdbType, EngineIo, Lexed, MysqlDescribed, Need, Response } from "./engine-core.ts";
export { AnswerCache, EngineCore } from "./engine-core.ts";

/** The version of reylo, which the engine it runs must have: that of the package this module ships in. */
export const REYLO_VERSION: string = (() => {
  for (const at of ["../package.json", "./package.json"]) {
    try {
      return JSON.parse(readFileSync(new URL(at, import.meta.url), "utf8")).version;
    } catch {}
  }
  throw new Error("reylo cannot find the package.json of its own package");
})();

/** An engine reylo cannot run: none installed, or one of another version than reylo's. Reported as the project's error. */
export class MissingEngine extends Error {
  constructor(message: string) {
    super(message);
    this.name = "MissingEngine";
  }
}

const engines = new Map<string, Promise<Engine>>();

/** How long the engine has to answer its first request, a ping: it answers in milliseconds, so a binary that does not answer at all is not the engine. */
const PING_TIMEOUT = 10_000;

/**
 * The engine of one database, loaded once per process and binary. It rejects with a MissingEngine
 * where there is no binary, where it is another version than reylo, and where it does not answer
 * as one database's engine.
 */
export function engine(dialect: Dialect): Promise<Engine> {
  let binary: string;
  try {
    binary = nativeBinary();
  } catch (e) {
    return Promise.reject(new MissingEngine((e as Error).message));
  }
  const key = `${binary}\0${dialect}`;
  let loading = engines.get(key);
  if (!loading) {
    loading = (async () => {
      // The child that answers this database's requests that need no database, the ping first.
      const core = new Channel(binary, dialect);
      let timer: ReturnType<typeof setTimeout> | undefined;
      const late = new Promise<Answer>((done) => (timer = setTimeout(() => done({ status: "error", message: `it did not answer a ping in ${PING_TIMEOUT / 1000} s` }), PING_TIMEOUT)));
      const answer = await Promise.race([core.ask({ op: "ping" }), late]);
      clearTimeout(timer);
      const ping = answer as unknown as Response;
      const refuse = (message: string) => {
        core.kill();
        return new MissingEngine(message);
      };
      if (ping.status !== "ok") throw refuse(`the engine ${binary} does not answer: ${answer.message ?? answer.status}`);
      if (ping.version !== REYLO_VERSION) {
        throw refuse(`reylo ${REYLO_VERSION} runs with its engine at ${REYLO_VERSION}, and ${binary} is ${ping.version}: install reylo again, point REYLO_ENGINE_PATH at the engine of reylo ${REYLO_VERSION}, or in a checkout of reylo's repository build it with cargo build --release -p reylo`);
      }
      if (ping.database !== dialect) throw refuse(`${binary} serves ${ping.database ?? "no one database"}'s engine when asked for ${dialect}'s`);
      return new Engine(binary, dialect, ping.fingerprint, core);
    })();
    // A failure is not kept: the binary may be installed or rebuilt while the process runs.
    loading.catch(() => engines.delete(key));
    engines.set(key, loading);
  }
  return loading;
}

/**
 * An engine to read the project file with, before the URL in it names the database: the core,
 * which parses the file, is every database's. A MissingEngine where there is none to run.
 */
export async function anyEngine(): Promise<Engine | MissingEngine> {
  return engine("postgres").catch((e: unknown) => (e instanceof MissingEngine ? e : new MissingEngine((e as Error).message)));
}

type Answer = { status: string; id?: number; message?: string; code?: string; conn?: number; version?: string; analysis?: Analysis; rows?: unknown[][]; server?: string; params?: number[]; columns?: Described["columns"] };

/** What a session asks of a connection the engine holds. */
export interface Connection {
  /** The server's version, as the engine read it when the connection opened: `18.1`, `8.4.11`, and on SQLite the engine's own SQLite's, `3.50.2`. */
  readonly version: string;
  analyze(sql: string): Promise<Analysis>;
  /** A query, its parameters bound and its rows read as the engine's catalog queries are. */
  query(sql: string, params: unknown[]): Promise<unknown[][]>;
  /** A statement described as the engine's own describe reads it, for a host that runs the engine's analysis itself (EngineCore's `analyze`); Postgres only. */
  describe(sql: string): Promise<Described>;
  /** The schema changed: the answers the engine keeps for this connection are stale. */
  forget(): Promise<void>;
  close(): Promise<void>;
}

/** The key a request's answer is kept by, for the requests whose answer is the same for the same request: `config` and `lex`. */
function keptKey(request: object): string | undefined {
  const op = (request as { op?: unknown }).op;
  return op === "config" || op === "lex" ? JSON.stringify(request) : undefined;
}

/** How many answers to `config` and `lex` are kept: each is the same for the same request, and a type check asks again for each file it transforms. */
const KEPT_ANSWERS = 512;

/** One database's engine, as the `reylo` executable serves it. */
export class Engine extends EngineCore {
  readonly binary: string;
  readonly dialect: Dialect;
  /** The engine's fingerprint, core's sources and its database's, which names the `reylo/.cache/` entries it writes. */
  readonly fingerprint: string;
  /** The child that answers this database's requests that need no database, started again where it stopped. */
  #core: Channel;
  readonly #kept = new Map<string, Response>();

  constructor(binary: string, dialect: Dialect, fingerprint: string, core: Channel) {
    super((request) => callSync(binary, dialect, request));
    this.binary = binary;
    this.dialect = dialect;
    this.fingerprint = fingerprint;
    this.#core = core;
  }

  /**
   * The answer to a request that needs no database, while this thread waits: `reylo engine
   * <database> call`, a process of its own, for a caller that cannot wait (the project file's
   * `config`). The answers to `config` and `lex`, the same for the same request, are kept.
   */
  override call(request: object): Response {
    const key = keptKey(request);
    return (key && this.#kept.get(key)) || this.#remember(key, super.call(request));
  }

  /** The lexer's reading of `sql`, through the long-lived child, for a caller that can wait: a round trip, where the one-shot form starts a process. */
  async lexAsync(sql: string, dialect: Dialect): Promise<Lexed> {
    const request = { op: "lex", sql, dialect };
    const key = keptKey(request);
    const r = (key && this.#kept.get(key)) || this.#remember(key, await this.ask(request));
    if (r.status !== "lexed") throw new Error(r.status === "error" ? r.message : `unexpected ${r.status}`);
    return r.lexed;
  }

  #remember(key: string | undefined, answer: Response): Response {
    if (key === undefined || answer.status === "error") return answer;
    if (this.#kept.size >= KEPT_ANSWERS) this.#kept.delete(this.#kept.keys().next().value!);
    this.#kept.set(key, answer);
    return answer;
  }

  /** A request that needs no database, through this database's long-lived child: the rounds of an analysis whose needs this process answers, and the lexer for a caller that can wait. */
  protected override async ask(request: object): Promise<Response> {
    if (this.#core.ended) this.#core = new Channel(this.binary, this.dialect);
    const answer = await this.#core.ask(request);
    if (answer.status === "unreachable") throw new Error(answer.message);
    return answer as unknown as Response;
  }

  /**
   * A connection to the database `url` names (a SQLite database's path), opened by the engine in a
   * child of its own, so that a server slow to answer holds up no other connection; an Unreachable
   * where it cannot be opened.
   */
  async open(url: string): Promise<Connection> {
    const target: HostUrl = url.includes("://") ? await hostUrl(url) : { url, close: () => {} };
    const channel = new Channel(this.binary, this.dialect);
    const ask = async (request: object) => settled(await channel.ask(request));
    let conn: number;
    let version: string;
    try {
      const opened = await ask({ op: "open", url: target.url, ...(target.bridge && { bridge: target.bridge }) });
      conn = opened.conn!;
      version = opened.version ?? "";
    } catch (e) {
      channel.end();
      target.close();
      // A socket the bridge could not open: its reason, rather than the connection it closed.
      const failure = target.failure?.();
      throw failure ? new Unreachable(failure) : e;
    }
    return {
      version,
      analyze: async (sql) => (await ask({ op: "run", conn, sql })).analysis!,
      query: async (sql, params) => (await ask({ op: "query", conn, sql, params })).rows!,
      describe: async (sql) => {
        const d = await ask({ op: "describe", conn, sql });
        return { params: d.params!.map((oid) => ({ oid })), columns: d.columns! };
      },
      forget: async () => void (await ask({ op: "forget", conn })),
      close: async () => {
        await ask({ op: "close", conn }).catch(() => {});
        channel.end();
        target.close();
      },
    };
  }
}

/**
 * An answer as the session reads it. A refused login and a database that cannot be reached are an
 * Unreachable, its cause carrying the server's code, so that `refusedLogin` reads it
 * (src/connection.ts); a MariaDB server, which the type check refuses, is a NotMysql; any other
 * failure is an Error.
 */
function settled(answer: Answer): Answer {
  if (answer.status === "notMysql") throw new NotMysql(answer.server!);
  if (answer.status === "unreachable" || answer.status === "refused") throw new Unreachable(Object.assign(new Error(answer.message), answer.code && { code: answer.code }));
  if (answer.status === "error") throw Object.assign(new Error(answer.message), answer.code && { code: answer.code });
  return answer;
}

/** The variables that name a file the engine reads: Node's extra roots, and libpq's TLS files. */
const FILE_VARIABLES = ["NODE_EXTRA_CA_CERTS", "PGSSLROOTCERT", "PGSSLCERT", "PGSSLKEY"];

/**
 * The engine's environment: this process's, with each variable that names a file resolved from
 * this process's working directory as Node and libpq resolve it, now, so that a later `chdir` here
 * changes nothing for a child already running.
 */
function engineEnv(): NodeJS.ProcessEnv {
  const env = { ...process.env };
  for (const name of FILE_VARIABLES) if (env[name] && env[name] !== "system") env[name] = resolve(env[name]);
  return env;
}

/** The answer of `reylo engine <database> call` to `request`, or why there is none, as the engine words a failure. */
function oneShot(binary: string, run: { error?: Error; status: number | null; signal: NodeJS.Signals | null; stdout: string; stderr: string }): string {
  if (run.error) return JSON.stringify({ status: "error", message: `the engine ${binary} could not start: ${run.error.message}` });
  const line = run.stdout.trim();
  if (run.status === 0 && line) return line;
  const said = run.stderr.trim().split("\n").slice(-3).join("\n");
  return JSON.stringify({ status: "error", message: `the engine ${binary} ${run.status === null ? `ended on ${run.signal}` : `exited with code ${run.status}`}${said ? `: ${said}` : ""}` });
}

/** One request that needs no database, answered by the one-shot form while this thread waits. */
function callSync(binary: string, dialect: Dialect, request: string): string {
  const run = spawnSync(binary, ["engine", dialect, "call"], { input: request, encoding: "utf8", env: engineEnv(), maxBuffer: 1 << 30, windowsHide: true });
  return oneShot(binary, run);
}

/** The period of the timer that holds the process while a request waits: long, since it only has to exist. */
const HOLD_INTERVAL = 1 << 30;

/** How much of what the engine writes to its standard error is kept, for the message of a child that stopped. */
const STDERR_KEPT = 4096;

/**
 * One child, `reylo engine <database>`, answering its requests in turn. It keeps this process alive
 * only while a request waits on it; a child that fails to start or stops answers what waits on it,
 * and everything after, as a database that cannot be reached, and the session opens another.
 */
class Channel {
  readonly #child: ChildProcessWithoutNullStreams;
  readonly #pending = new Map<number, (answer: Answer) => void>();
  #next = 1;
  #buffered = "";
  #stderr = "";
  #ended: string | undefined;
  /** What holds this process while a request waits: a timer, which never fires. */
  #waiting: ReturnType<typeof setInterval> | undefined;

  constructor(binary: string, dialect: Dialect) {
    const child = spawn(binary, ["engine", dialect], { env: engineEnv(), windowsHide: true });
    this.#child = child;
    child.stdout.setEncoding("utf8").on("data", (chunk: string) => {
      this.#buffered += chunk;
      let end: number;
      while ((end = this.#buffered.indexOf("\n")) !== -1) {
        const line = this.#buffered.slice(0, end);
        this.#buffered = this.#buffered.slice(end + 1);
        this.#answer(line);
      }
    });
    child.stderr.setEncoding("utf8").on("data", (chunk: string) => (this.#stderr = (this.#stderr + chunk).slice(-STDERR_KEPT)));
    // A child that stopped closes its input under a request being written: the exit says why.
    child.stdin.on("error", () => {});
    child.on("error", (e: Error) => this.#end(`the engine ${binary} could not start: ${e.message}`));
    // `close`, after the child's output has been read to its end, rather than `exit`.
    child.on("close", (code, signal) => {
      const said = this.#stderr.trim().split("\n").slice(-3).join("\n");
      this.#end(`the engine ${binary} stopped (${code ?? signal})${said ? `: ${said}` : ""}`);
    });
    // Neither the child nor its pipes keep this process alive, from the start (`#hold`).
    child.unref();
    for (const stream of [child.stdin, child.stdout, child.stderr]) (stream as unknown as { unref?(): void }).unref?.();
  }

  get ended(): boolean {
    return this.#ended !== undefined;
  }

  ask(request: object): Promise<Answer> {
    if (this.#ended) return Promise.resolve({ status: "unreachable", message: this.#ended });
    const id = this.#next++;
    return new Promise<Answer>((resolve) => {
      this.#pending.set(id, resolve);
      this.#hold(true);
      this.#child.stdin.write(`${JSON.stringify({ ...request, id })}\n`);
    });
  }

  /** The child stopped, with what it held: its input closed, at which it exits. */
  end(): void {
    this.#ended ??= "the connection is closed";
    this.#child.stdin.end();
    this.#hold(false);
  }

  /** The child stopped at once, for one that is no engine to talk to. */
  kill(): void {
    this.end();
    this.#child.kill();
  }

  #answer(line: string): void {
    let answer: Answer;
    try {
      answer = JSON.parse(line);
    } catch {
      return;
    }
    // A line the engine could not read as a request is answered without its `id`: the child answers
    // in turn, so the answer is the oldest request's.
    const id = answer.id ?? this.#pending.keys().next().value;
    const resolve = id === undefined ? undefined : this.#pending.get(id);
    if (!resolve) return;
    this.#pending.delete(id!);
    resolve(answer);
    if (this.#pending.size === 0) this.#hold(false);
  }

  #end(why: string): void {
    this.#ended ??= why;
    for (const resolve of this.#pending.values()) resolve({ status: "unreachable", message: this.#ended });
    this.#pending.clear();
    this.#hold(false);
  }

  /** Whether this process is held for the requests waiting on the child: by a timer, since the child itself is never referenced again once it starts. */
  #hold(held: boolean): void {
    if (held) this.#waiting ??= setInterval(() => {}, HOLD_INTERVAL);
    else if (this.#waiting !== undefined) {
      clearInterval(this.#waiting);
      this.#waiting = undefined;
    }
  }
}
