// `reylo play`: the playground of reylo.dev/play on this machine, against the project's own
// database. It serves the page and an API on 127.0.0.1: each statement the page or a caller sends
// is analyzed as the type check analyzes a file (never run), and answered with its Params, Returns
// and diagnostics by line and column (src/play-result.ts). The database is the one the type check
// reads, and nothing else: a project that names none is told so, and pointed at reylo.dev/play.
import { spawn } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { createRequire } from "node:module";
import { dirname, extname, join, sep } from "node:path";
import { styleText } from "node:util";
import { Session } from "./analyze.ts";
import type { Dialect } from "./engine.ts";
import { catalog, CATALOG_QUERIES, type Catalog } from "./play-catalog.ts";
import { playResult, type PlayResult } from "./play-result.ts";

export const PLAY_USAGE = `Usage: reylo play [--port <n>] [--no-open]

Serves the playground on http://127.0.0.1: a query editor whose Params, Returns and errors are
Reylo's analysis of the query against the project's database, at every keystroke. Nothing is run.
The database is the project's own, the one the type check reads; with none, it says how to set it.
It opens the page in the browser; --no-open leaves it closed, for a script or an agent.

  http://127.0.0.1:<port>/?q=<sql>              the page, with a query in it
  GET  /api/analyze?q=<sql>                       a query's Params, Returns and diagnostics, as JSON
  POST /api/analyze  {"sql": "<sql>"}             the same
  GET  /api/schema                                the tables, their columns, functions and types`;

/** What answers the page: the dialect, a line saying which database, the schema it shows, and each statement's analysis. */
type Backend = {
  dialect: Dialect;
  label: string;
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
  const { url, label } = running;
  console.log(`${styleText("bold", "Reylo playground")} at ${styleText("cyan", url)}\n${styleText("dim", `${label}. The API: GET ${url}api/analyze?q=<sql>. Ctrl-C stops it.`)}`);
  if (options.open) openBrowser(url);
  await new Promise<void>((done) => {
    process.once("SIGINT", done);
    process.once("SIGTERM", done);
  });
  await running.close();
  return 0;
}

export type Playground = { url: string; label: string; close(): Promise<void> };

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

/** The project's database, the one its type check reads; a project that names none is refused. */
async function openBackend(cwd: string, env: Record<string, string | undefined>): Promise<Backend> {
  const session = await Session.open(cwd, env);
  const { project } = session;
  if (project.error || !project.url) {
    await session.close();
    throw new Error(project.error ?? `${project.missingUrl} reylo play reads the project's own database; to try Reylo without one, open https://reylo.dev/play.`);
  }
  return databaseBackend(session);
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
    schema: (await schemaDdl(dialect, (sql) => session.query(sql))) ?? schemaText(dialect, cat),
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

/**
 * The database's schema as statements that would make it, for the page's schema card: SQLite's own
 * stored statements, MySQL's \`SHOW CREATE\`, and on Postgres, which keeps no statement, one written
 * from its catalog (types, tables with their defaults and constraints, views, indexes) for the
 * schemas on the search path. Undefined where the database refuses, and the columns stand in.
 */
async function schemaDdl(dialect: Dialect, query: (sql: string) => Promise<unknown[][]>): Promise<string | undefined> {
  try {
    if (dialect === "sqlite") {
      const rows = await query(`select sql as ddl from sqlite_schema where sql is not null and name not like 'sqlite_%' order by case type when 'table' then 0 when 'view' then 1 when 'index' then 2 else 3 end, name`);
      return rows.map(([ddl]) => `${String(ddl).trim()};`).join("\n\n");
    }
    if (dialect === "mysql") {
      const tables = await query("select table_name, table_type from information_schema.tables where table_schema = database() order by table_type = 'VIEW', table_name");
      const out: string[] = [];
      for (const [name, type] of tables as [string, string][]) {
        const [row] = await query(`show create ${type === "VIEW" ? "view" : "table"} \`${name.replace(/`/g, "``")}\``);
        // A table's next AUTO_INCREMENT value is its data's, not its schema's.
        out.push(`${String(row?.[1] ?? "").replace(/ AUTO_INCREMENT=\d+/, "")};`);
      }
      return out.join("\n\n");
    }
    const rows = await query(POSTGRES_SCHEMA);
    return rows.map(([ddl]) => String(ddl)).join("\n\n");
  } catch {
    return undefined;
  }
}

const POSTGRES_SCHEMA = `with q as (
  select n.oid, case when n.nspname = 'public' then '' else quote_ident(n.nspname) || '.' end as p
  from pg_namespace n where n.nspname = any(current_schemas(false)) and n.nspname <> 'pg_catalog'
)
select ddl from (
  select 0 as k, t.typname::text as name, format('create type %s%s as enum (%s);', q.p, quote_ident(t.typname),
    (select string_agg(quote_literal(e.enumlabel), ', ' order by e.enumsortorder) from pg_enum e where e.enumtypid = t.oid)) as ddl
  from pg_type t join q on q.oid = t.typnamespace where t.typtype = 'e'
  union all
  select 0, t.typname::text, format('create domain %s%s as %s%s%s;', q.p, quote_ident(t.typname), format_type(t.typbasetype, t.typtypmod),
    case when t.typnotnull then ' not null' else '' end,
    coalesce((select string_agg(' ' || pg_get_constraintdef(con.oid), '' order by con.conname) from pg_constraint con where con.contypid = t.oid and con.contype = 'c'), ''))
  from pg_type t join q on q.oid = t.typnamespace where t.typtype = 'd'
  union all
  select 1, c.relname::text, format(E'create table %s%s (\n%s\n);', q.p, quote_ident(c.relname), (
    select string_agg(line, E',\n' order by ord) from (
      select a.attnum::int as ord, '  ' || quote_ident(a.attname) || ' ' || format_type(a.atttypid, a.atttypmod)
        || case when a.attidentity in ('a', 'd') then ' generated ' || case a.attidentity when 'a' then 'always' else 'by default' end || ' as identity' else '' end
        || case when a.attgenerated <> '' then ' generated always as (' || pg_get_expr(d.adbin, d.adrelid) || ')' || case a.attgenerated when 's' then ' stored' else ' virtual' end
                when d.adbin is not null and a.attidentity = '' then ' default ' || pg_get_expr(d.adbin, d.adrelid) else '' end
        || case when a.attnotnull then ' not null' else '' end as line
      from pg_attribute a left join pg_attrdef d on d.adrelid = a.attrelid and d.adnum = a.attnum
      where a.attrelid = c.oid and a.attnum > 0 and not a.attisdropped
      union all
      select 100000 + row_number() over (order by con.contype <> 'p', con.contype, con.conname)::int, '  constraint ' || quote_ident(con.conname) || ' ' || pg_get_constraintdef(con.oid)
      from pg_constraint con where con.conrelid = c.oid and con.contype in ('p', 'u', 'f', 'c', 'x')
    ) lines))
  from pg_class c join q on q.oid = c.relnamespace where c.relkind in ('r', 'p', 'f')
  union all
  select 2, c.relname::text, format(E'create %sview %s%s as\n%s;', case c.relkind when 'm' then 'materialized ' else '' end, q.p, quote_ident(c.relname), rtrim(rtrim(pg_get_viewdef(c.oid, true)), ';'))
  from pg_class c join q on q.oid = c.relnamespace where c.relkind in ('v', 'm')
  union all
  select 3, i.relname::text, pg_get_indexdef(i.oid) || ';'
  from pg_index x join pg_class i on i.oid = x.indexrelid join pg_class c on c.oid = x.indrelid join q on q.oid = c.relnamespace
  where not exists (select 1 from pg_constraint con where con.conindid = x.indexrelid and con.contype in ('p', 'u', 'x'))
) s order by k, name`;

/** The schema as the page shows it where the database refuses to give it: each table's columns, as a CREATE TABLE writes them. */
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

