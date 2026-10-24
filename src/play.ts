// `reylo play`: the playground of reylo.dev/play on this machine, against the project's own
// database. It serves the page and an API on 127.0.0.1: each statement the page or a caller sends
// is analyzed as the type check analyzes a file (never run), and answered with its Params, Returns
// and diagnostics by line and column (src/play-result.ts). The database is the one the type check
// reads, and nothing else: with no database URL, PGlite, Postgres compiled to WebAssembly, stands in
// with a sample schema, so that the page opens on tables to query.
import { spawn } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { createRequire } from "node:module";
import { dirname, extname, join, sep } from "node:path";
import { pathToFileURL } from "node:url";
import { styleText } from "node:util";
import { Session } from "./analyze.ts";
import { AnswerCache, engine, type Dialect, type Engine } from "./engine.ts";
import { catalog, CATALOG_QUERIES, type Catalog } from "./play-catalog.ts";
import { playResult, type PlayResult } from "./play-result.ts";
import { SAMPLE_SCHEMA } from "./play-sample.ts";
import { pgliteIo } from "./pglite-io.ts";

export const PLAY_USAGE = `Usage: reylo play [--port <n>] [--no-open]

Serves the playground on http://127.0.0.1: a query editor whose Params, Returns and errors are
Reylo's analysis of the query against the project's database, at every keystroke. Nothing is run.
The database is the project's own, the one the type check reads. With no database URL, PGlite
(the npm package @electric-sql/pglite, installed in the project) stands in with a sample schema.
It opens the page in the browser; --no-open leaves it closed, for a script or an agent.

  http://127.0.0.1:<port>/?q=<sql>              the page, with a query in it
  GET  /api/analyze?q=<sql>                       a query's Params, Returns and diagnostics, as JSON
  POST /api/analyze  {"sql": "<sql>"}             the same
  GET  /api/schema                                the tables, their columns, functions and types`;

/** What answers the page: the dialect, a line saying which database, the schema it shows, and each statement's analysis. */
type Backend = {
  dialect: Dialect;
  label: string;
  /** What the project file sets that Reylo does not read (`url` alone), said once. */
  notice?: string;
  schema: string;
  catalog: Catalog;
  analyze(sql: string, width: number): Promise<PlayResult>;
  close(): Promise<void>;
};

const DEFAULT_PORT = 4983;

export async function play(args: string[]): Promise<number> {
  const options = parseArgs(args);
  if (typeof options === "string") {
    console.error(`reylo play: ${options}\n\n${PLAY_USAGE}`);
    return 2;
  }
  if (options.help) {
    console.log(PLAY_USAGE);
    return 0;
  }
  let running: Playground;
  try {
    running = await startPlay({ port: options.port });
  } catch (e) {
    console.error(`reylo play: ${(e as Error).message}`);
    return 1;
  }
  const { url, label, notice } = running;
  if (notice) console.log(styleText("yellow", notice));
  console.log(`${styleText("bold", "Reylo playground")} at ${styleText("cyan", url)}\n${styleText("dim", `${label}. The API: GET ${url}api/analyze?q=<sql>. Ctrl-C stops it.`)}`);
  if (options.open) openBrowser(url);
  await new Promise<void>((done) => {
    process.once("SIGINT", done);
    process.once("SIGTERM", done);
  });
  await running.close();
  return 0;
}

export type Playground = { url: string; label: string; notice?: string; close(): Promise<void> };

/** Starts the playground for the project of `cwd`, on `port` (by default the first free one from 4983): what `reylo play` runs, and the tests. */
export async function startPlay(options: { port?: number | undefined; cwd?: string; env?: Record<string, string | undefined> } = {}): Promise<Playground> {
  const cwd = options.cwd ?? process.cwd();
  const page = pageDir(cwd);
  if (!page) throw new Error("the playground's page is not in this install of reylo; reinstall reylo.");
  const backend = await openBackend(cwd, options.env ?? process.env);
  const server = createServer((req, res) => {
    handle(req, res, backend, page, () => server.address()).catch((e: unknown) => send(res, 500, { error: (e as Error).message }));
  });
  const port = await listen(server, options.port);
  if (port === undefined) {
    await backend.close();
    throw new Error(`port ${options.port ?? DEFAULT_PORT} is in use; name another with --port.`);
  }
  return {
    url: `http://127.0.0.1:${port}/`,
    label: backend.label,
    ...(backend.notice ? { notice: backend.notice } : {}),
    async close() {
      server.closeAllConnections();
      await new Promise<void>((done) => server.close(() => done()));
      await backend.close();
    },
  };
}

function parseArgs(args: string[]): { port?: number; open: boolean; help: boolean } | string {
  const out: { port?: number; open: boolean; help: boolean } = { open: true, help: false };
  for (let i = 0; i < args.length; i++) {
    const a = args[i]!;
    const [flag, inline] = a.startsWith("--") && a.includes("=") ? [a.slice(0, a.indexOf("=")), a.slice(a.indexOf("=") + 1)] : [a, undefined];
    const value = () => inline ?? args[++i];
    if (flag === "--help" || flag === "-h") out.help = true;
    else if (flag === "--no-open") out.open = false;
    else if (flag === "--port") {
      const n = Number(value());
      if (!Number.isInteger(n) || n < 0 || n > 65535) return `--port takes a port number`;
      out.port = n;
    } else return `unknown argument ${a}`;
  }
  return out;
}

/** The built page: in an installed reylo, `play/` beside its package.json; in this repository, dist/reylo/play/. */
function pageDir(cwd: string): string | undefined {
  const candidates = [process.env.REYLO_PLAY_PAGE];
  // reylo exports no package.json: its entry is resolved, and its directory found above it.
  for (const from of [join(cwd, "noop.js"), import.meta.filename]) {
    try {
      let dir = dirname(createRequire(from).resolve("reylo"));
      while (dir !== dirname(dir) && !existsSync(join(dir, "package.json"))) dir = dirname(dir);
      candidates.push(join(dir, "play"));
    } catch {}
  }
  candidates.push(join(import.meta.dirname, "..", "dist", "reylo", "play"));
  return candidates.find((dir): dir is string => !!dir && existsSync(join(dir, "index.html")));
}

/** The project's database; PGlite with the sample schema only where the project names none. */
async function openBackend(cwd: string, env: Record<string, string | undefined>): Promise<Backend> {
  const session = await Session.open(cwd, env);
  const { project } = session;
  if (project.error) throw new Error(project.error);
  if (project.url) return databaseBackend(session);
  await session.close();
  const backend = await pgliteBackend(project.root);
  return project.notice ? { ...backend, notice: project.notice } : backend;
}

/** The project's database, through the type check's own session: its URL, its engine, its settings. */
async function databaseBackend(session: Session): Promise<Backend> {
  const { project } = session;
  const dialect = project.dialect;
  const schema = await session.schema();
  if (schema.value === null || schema.fatal) throw new Error(schema.fatal ? schema.error! : session.cannotConnect(schema.error));
  const queries = CATALOG_QUERIES[dialect];
  const rows = async (sql: string | undefined) => (sql ? await session.query(sql).catch(() => []) : []);
  const cat = catalog(dialect, await rows(queries.tables), await rows(queries.functions), await rows(queries.types));
  return {
    dialect,
    label: `${dialect === "postgres" ? "Postgres" : dialect === "mysql" ? "MySQL" : "SQLite"} at ${session.where()} (${project.urlSource})`,
    schema: schemaText(dialect, cat),
    catalog: cat,
    async analyze(sql, width) {
      const t = performance.now();
      const outcome = await session.analyze(sql, { store: false });
      const ms = performance.now() - t;
      if (!outcome.ok) return playResult(sql, dialect, { error: { message: outcome.message, code: outcome.code } }, { width, ms });
      let lexed;
      try {
        lexed = session.lex(sql);
      } catch {}
      return playResult(sql, dialect, { analysis: outcome.analysis, lexed }, { strictParams: project.strictParams, width, ms });
    },
    close: () => session.close(),
  };
}

/** PGlite in this process, from the project's install or reylo's own, with the sample schema. */
async function pgliteBackend(root: string): Promise<Backend> {
  let PGlite: typeof import("@electric-sql/pglite").PGlite | undefined;
  for (const from of [join(root, "noop.js"), import.meta.filename]) {
    try {
      ({ PGlite } = (await import(pathToFileURL(createRequire(from).resolve("@electric-sql/pglite")).href)) as typeof import("@electric-sql/pglite"));
      break;
    } catch {}
  }
  if (!PGlite) {
    throw new Error("no database URL is set, and PGlite is not installed to stand in for one: set DATABASE_URL (or url in .reylo.yaml), or install @electric-sql/pglite in the project (npm install -D @electric-sql/pglite).");
  }
  const schema = SAMPLE_SCHEMA;
  const pg = await PGlite.create();
  try {
    await pg.exec(schema);
  } catch (e) {
    await pg.close();
    throw new Error(`the sample schema does not load into PGlite: ${(e as Error).message}`);
  }
  const io = pgliteIo(pg);
  const eng: Engine = await engine("postgres", root);
  const cache = new AnswerCache();
  const queries = CATALOG_QUERIES.postgres;
  const cat = catalog("postgres", await io.query(queries.tables, []), await io.query(queries.functions!, []), await io.query(queries.types!, []));
  const version = String((await io.query("select current_setting('server_version')", []))[0]![0]).split(" ")[0];
  return {
    dialect: "postgres",
    label: `Postgres ${version} via PGlite, with a sample schema (no database URL is set)`,
    schema,
    catalog: cat,
    async analyze(sql, width) {
      const t = performance.now();
      let lexed;
      try {
        lexed = eng.lex(sql, "postgres");
      } catch {}
      try {
        const analysis = await eng.analyze(sql, "postgres", io, cache);
        return playResult(sql, "postgres", { analysis, lexed }, { width, ms: performance.now() - t });
      } catch (e) {
        return playResult(sql, "postgres", { error: e as Error }, { width, ms: performance.now() - t });
      }
    },
    close: () => pg.close(),
  };
}

/** The schema as the page shows it for a database it did not create: each table's columns, as a CREATE TABLE writes them. */
function schemaText(dialect: Dialect, cat: Catalog): string {
  const q = (name: string) => (/^[a-z_][a-z0-9_]*$/.test(name) ? name : dialect === "mysql" ? `\`${name}\`` : `"${name.replace(/"/g, '""')}"`);
  return cat.tables
    .map((t) => {
      const width = Math.max(...t.columns.map((c) => q(c.name).length));
      return `create table ${q(t.name)} (\n${t.columns.map((c) => `  ${q(c.name).padEnd(width)} ${c.type}${c.nullable ? "" : " not null"}`).join(",\n")}\n);`;
    })
    .join("\n\n");
}

function listen(server: ReturnType<typeof createServer>, port: number | undefined): Promise<number | undefined> {
  // A port the caller names is that port or nothing; the default moves up past ports in use.
  const tries = port === undefined ? Array.from({ length: 20 }, (_, i) => DEFAULT_PORT + i) : [port];
  return new Promise((done) => {
    const attempt = (i: number) => {
      if (i >= tries.length) return done(undefined);
      server.once("error", (e: NodeJS.ErrnoException) => (e.code === "EADDRINUSE" ? attempt(i + 1) : done(undefined)));
      server.listen(tries[i], "127.0.0.1", () => {
        server.removeAllListeners("error");
        const address = server.address();
        done(typeof address === "object" && address ? address.port : undefined);
      });
    };
    attempt(0);
  });
}

const TYPES: Record<string, string> = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".css": "text/css; charset=utf-8", ".woff2": "font/woff2", ".svg": "image/svg+xml", ".wasm": "application/wasm", ".json": "application/json" };

async function handle(req: IncomingMessage, res: ServerResponse, backend: Backend, page: string, address: () => ReturnType<ReturnType<typeof createServer>["address"]>): Promise<void> {
  const a = address();
  const port = typeof a === "object" && a ? a.port : 0;
  // Only this machine's own pages and tools: a page on another site that sends a request here, or
  // a DNS name rebound to 127.0.0.1, names another host or origin, and is refused, so no web page
  // reads the schema through the playground.
  const local = (host: string | undefined) => !!host && [`127.0.0.1:${port}`, `localhost:${port}`, `[::1]:${port}`].includes(host);
  const origin = req.headers.origin;
  if (!local(req.headers.host) || (origin !== undefined && !local(origin.replace(/^http:\/\//, "")))) return send(res, 403, { error: "the playground answers this machine's own pages and tools only" });
  const url = new URL(req.url ?? "/", `http://${req.headers.host}`);

  if (url.pathname === "/api/analyze") {
    let sql: string | null = url.searchParams.get("q") ?? url.searchParams.get("sql");
    let width = Number(url.searchParams.get("width") ?? Infinity);
    if (req.method === "POST") {
      let body: { sql?: unknown; q?: unknown; width?: unknown };
      try {
        body = JSON.parse((await readBody(req)) || "{}");
      } catch {
        return send(res, 400, { error: "the body is not JSON" });
      }
      sql = typeof body.sql === "string" ? body.sql : typeof body.q === "string" ? body.q : sql;
      if (typeof body.width === "number") width = body.width;
    } else if (req.method !== "GET") return send(res, 405, { error: "GET or POST" });
    if (sql === null) return send(res, 400, { error: "name the query: ?q=<sql>, or a JSON body {\"sql\": \"<sql>\"}" });
    return send(res, 200, await backend.analyze(sql, Number.isFinite(width) && width > 0 ? width : Infinity));
  }
  if (url.pathname === "/api/schema") return send(res, 200, { dialect: backend.dialect, database: backend.label, ...backend.catalog });
  if (url.pathname === "/api/page") return send(res, 200, { dialect: backend.dialect, label: backend.label, schema: backend.schema, catalog: backend.catalog });

  // The page's files; any other path is the page, which reads its query from the address.
  const file = url.pathname === "/" ? join(page, "index.html") : join(page, decodeURIComponent(url.pathname));
  const inside = file.startsWith(page + sep) && existsSync(file) && statSync(file).isFile();
  const path = inside ? file : join(page, "index.html");
  res.writeHead(200, { "content-type": TYPES[extname(path)] ?? "application/octet-stream", "cache-control": "no-cache", "x-content-type-options": "nosniff" });
  res.end(readFileSync(path));
}

function send(res: ServerResponse, status: number, body: unknown): void {
  res.writeHead(status, { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", "x-content-type-options": "nosniff" });
  res.end(JSON.stringify(body));
}

function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((done, fail) => {
    let text = "";
    req.setEncoding("utf8");
    req.on("data", (chunk: string) => {
      text += chunk;
      if (text.length > 1_000_000) req.destroy(new Error("the body is over a megabyte"));
    });
    req.on("end", () => done(text));
    req.on("error", fail);
  });
}

function openBrowser(url: string): void {
  const [command, args] = process.platform === "darwin" ? ["open", [url]] : process.platform === "win32" ? ["cmd", ["/c", "start", "", url]] : ["xdg-open", [url]];
  spawn(command, args, { stdio: "ignore", detached: true }).on("error", () => {}).unref();
}

