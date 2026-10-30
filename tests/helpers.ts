// What the loader's tests share: the test databases `REYLO_TEST_PG` and `REYLO_TEST_MYSQL` name and a database of a test's own on each, the drivers `lab/drivers` installs, and the content mapper run over its JSON-RPC protocol, as the type check runs it.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const ROOT = join(import.meta.dirname, "..");
export const DRIVERS = join(ROOT, "lab", "drivers", "node_modules");
export const MAPPER = join(ROOT, "src", "mapper.ts");
export const INDEX = pathToFileURL(join(ROOT, "src", "index.ts")).href;
export const PG = process.env.REYLO_TEST_PG;
export const MYSQL = process.env.REYLO_TEST_MYSQL;
/** Why a test of the live Postgres skips, or `false`. */
export const pgLive = !PG ? "REYLO_TEST_PG names no server" : !existsSync(join(DRIVERS, "postgres")) ? "lab/drivers is not installed" : false;
/** Why a test of the live MySQL skips, or `false`. */
export const mysqlLive = !MYSQL ? "REYLO_TEST_MYSQL names no server" : !existsSync(join(DRIVERS, "mysql2")) ? "lab/drivers is not installed" : false;

/** A driver as `lab/drivers` installs it. */
export function driver(name: string): any {
  const pkg = name.startsWith("@") ? name.split("/").slice(0, 2).join("/") : name.split("/")[0]!;
  return createRequire(join(DRIVERS, pkg, "package.json"))(name);
}

/** Closes every session the tests opened, so that the next analysis connects afresh. */
export async function closeSessions(): Promise<void> {
  const { Session } = await import("../src/analyze.ts");
  for (const session of Session.opened()) await session.close();
}

/** Rows as plain objects, to compare with literals. */
export const plain = (rows: readonly object[]) => [...rows].map((r) => ({ ...r }));

/** A database of a test's own on the test Postgres, its URL, and a postgres.js client of it with `setup` run first. */
export async function postgresDatabase(prefix: string, setup?: string) {
  const postgres = driver("postgres");
  const admin = postgres(PG!, { max: 1, onnotice: () => {} });
  const name = `${prefix}_${process.pid}`;
  await admin.unsafe(`drop database if exists ${name} with (force)`);
  await admin.unsafe(`create database ${name}`);
  const url = new URL(PG!);
  url.pathname = `/${name}`;
  const sql = postgres(url.href, { max: 1, onnotice: () => {} });
  if (setup) await sql.unsafe(setup).simple();
  return {
    url: url.href,
    sql,
    async drop() {
      await sql.end();
      await admin.unsafe(`drop database if exists ${name} with (force)`);
      await admin.end();
    },
  };
}

/** A database of a test's own on the test MySQL, its URL, and a mysql2 connection to it as the driver connects by default, with `setup` run first. */
export async function mysqlDatabase(prefix: string, setup?: string) {
  const mysql = driver("mysql2/promise");
  const u = new URL(MYSQL!);
  const connect = (database?: string, multipleStatements = false) => mysql.createConnection({ host: u.hostname, port: Number(u.port), user: decodeURIComponent(u.username) || "root", password: decodeURIComponent(u.password), database, multipleStatements });
  const admin = await connect();
  const name = `${prefix}_${process.pid}`;
  await admin.query(`drop database if exists ${name}`);
  await admin.query(`create database ${name}`);
  const url = new URL(MYSQL!);
  url.pathname = `/${name}`;
  if (setup) {
    const setter = await connect(name, true);
    await setter.query(setup);
    await setter.end();
  }
  const db = await connect(name);
  return {
    url: url.href,
    db,
    rows: async (sql: string) => plain((await db.query(sql))[0]),
    async drop() {
      await db.end();
      await admin.query(`drop database if exists ${name}`);
      await admin.end();
    },
  };
}

/** A diagnostic the content mapper reports. */
export type Diagnostic = { messageText: string; start: number; length: number; code: number };

/** The declaration the content mapper writes for each file of `files` (name to content), which it must write with no diagnostic. */
export async function declarations(root: string, files: Record<string, string>): Promise<Record<string, string>> {
  const out: Record<string, string> = {};
  for (const [name, { text, diagnostics }] of Object.entries(await transform(root, files))) {
    if (diagnostics.length) throw new Error(`${name}: ${JSON.stringify(diagnostics)}`);
    out[name] = text;
  }
  return out;
}

/** What the content mapper answers for each file of `files` (name to content) in the project at `root`, through its own JSON-RPC process, as the type check runs it: the declaration and its diagnostics. */
export async function transform(root: string, files: Record<string, string>): Promise<Record<string, { text: string; diagnostics: Diagnostic[] }>> {
  const child = spawn(process.execPath, [MAPPER], { cwd: root, stdio: ["pipe", "pipe", "inherit"] });
  const pending = new Map<number, (result: unknown) => void>();
  let buffer = Buffer.alloc(0);
  child.stdout.on("data", (chunk: Buffer) => {
    buffer = Buffer.concat([buffer, chunk]);
    for (;;) {
      const end = buffer.indexOf("\r\n\r\n");
      if (end === -1) return;
      const length = Number(/Content-Length:\s*(\d+)/i.exec(buffer.subarray(0, end).toString("ascii"))![1]);
      if (buffer.length < end + 4 + length) return;
      const message = JSON.parse(buffer.subarray(end + 4, end + 4 + length).toString("utf8"));
      buffer = buffer.subarray(end + 4 + length);
      pending.get(message.id)?.(message.error ? Promise.reject(new Error(message.error.message)) : message.result);
    }
  });
  let id = 0;
  const call = (method: string, params: unknown) =>
    new Promise<any>((resolve) => {
      pending.set(++id, resolve);
      const body = Buffer.from(JSON.stringify({ jsonrpc: "2.0", id, method, params }), "utf8");
      child.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
      child.stdin.write(body);
    });
  try {
    await call("initialize", { protocolVersion: 1 });
    await call("openProject", { projectHandle: "p", configFileName: join(root, "tsconfig.json"), options: {} });
    const out: Record<string, { text: string; diagnostics: Diagnostic[] }> = {};
    for (const [name, content] of Object.entries(files)) {
      const result = await call("transform", { fileName: join(root, name), content, projectHandle: "p" });
      out[name] = { text: result.text, diagnostics: result.diagnostics };
    }
    await call("closeProject", { projectHandle: "p" });
    return out;
  } finally {
    child.stdin.end();
    await new Promise((resolve) => child.once("exit", resolve));
  }
}

/** TypeScript's language server for the project at `root`, over stdio with LSP framing, as an editor talks to it. */
export function languageServer(root: string) {
  const child = spawn(process.execPath, [join(ROOT, "node_modules", "typescript", "bin", "tsc"), "--lsp", "--stdio"], { cwd: root, stdio: ["pipe", "pipe", "ignore"] });
  const pending = new Map<number, (result: any) => void>();
  let buffer = Buffer.alloc(0);
  const write = (message: object) => {
    const body = Buffer.from(JSON.stringify({ jsonrpc: "2.0", ...message }), "utf8");
    child.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
    child.stdin.write(body);
  };
  child.stdout.on("data", (chunk: Buffer) => {
    buffer = Buffer.concat([buffer, chunk]);
    for (;;) {
      const end = buffer.indexOf("\r\n\r\n");
      if (end === -1) return;
      const length = Number(/Content-Length:\s*(\d+)/i.exec(buffer.subarray(0, end).toString("ascii"))![1]);
      if (buffer.length < end + 4 + length) return;
      const message = JSON.parse(buffer.subarray(end + 4, end + 4 + length).toString("utf8"));
      buffer = buffer.subarray(end + 4 + length);
      // A request from the server (registerCapability, workspace/configuration) is answered as a client that has nothing to add.
      if (message.method && message.id !== undefined) write({ id: message.id, result: message.method === "workspace/configuration" ? message.params.items.map(() => null) : null });
      else if (message.id !== undefined) pending.get(message.id)?.(message.result);
    }
  });
  let id = 0;
  const request = (method: string, params: unknown) =>
    new Promise<any>((resolve) => {
      pending.set(++id, resolve);
      write({ id, method, params });
    });
  const notify = (method: string, params: unknown) => write({ method, params });
  const close = async () => {
    await request("shutdown", undefined);
    notify("exit", undefined);
    await new Promise((resolve) => child.once("exit", resolve));
  };
  return { request, notify, close };
}
