import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { connect, createServer, type AddressInfo, type Server } from "node:net";
import { Session } from "../src/analyze.ts";
import { guestPath } from "../src/component.ts";
import { engine, reason, Unreachable } from "../src/engine.ts";
import { hostUrl } from "../src/host-url.ts";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const PG = process.env.REYLO_TEST_PG;
const MYSQL = process.env.REYLO_TEST_MYSQL;
const noPg = !PG && "REYLO_TEST_PG names no server";
const noMysql = !MYSQL && "REYLO_TEST_MYSQL names no server";

/** A Unix socket in a directory of its own that pipes each connection to the TCP server `url` names, as a server listening on its socket. */
async function socketFor(url: string, name: string): Promise<{ dir: string; path: string; server: Server }> {
  const { hostname, port } = new URL(url);
  const dir = mkdtempSync(join(tmpdir(), "reylo-sock-"));
  const path = join(dir, name);
  const server = createServer((client) => {
    const upstream = connect(Number(port), hostname);
    client.on("error", () => upstream.destroy());
    upstream.on("error", () => client.destroy());
    client.pipe(upstream).pipe(client);
  });
  await new Promise<void>((done) => server.listen(path, done));
  return { dir, path, server };
}

test("reason reads a refused connection, an AggregateError with no message of its own", () => {
  const refused = Object.assign(new AggregateError([new Error("connect ECONNREFUSED ::1:1"), new Error("connect ECONNREFUSED 127.0.0.1:1")], ""), { code: "ECONNREFUSED" });
  assert.equal(reason(refused), "connect ECONNREFUSED ::1:1; connect ECONNREFUSED 127.0.0.1:1");
  assert.equal(reason(Object.assign(new Error(""), { code: "ETIMEDOUT" })), "ETIMEDOUT");
});

test("the engine reports a connection the server refuses as Unreachable, on Postgres and on MySQL", async () => {
  for (const [dialect, url] of [["postgres", "postgres://127.0.0.1:1/x"], ["mysql", "mysql://root@127.0.0.1:1/x"]] as const) {
    const [r] = await Promise.allSettled([(await engine(dialect, ROOT)).open(url)]);
    assert.equal(r!.status, "rejected", dialect);
    const e = (r as PromiseRejectedResult).reason;
    assert.ok(e instanceof Unreachable, `${dialect}: expected Unreachable, got ${e}`);
    assert.match(e.message, /refused/i, dialect);
  }
});

test("analyses waiting on a server that never answers fail together, at the one connect timeout", { timeout: 30_000 }, async () => {
  // A server that accepts the socket and never says anything: the engine gives up after its connect
  // timeout, and every analysis that waited on that one connection fails with it.
  const silent = createServer((socket) => socket.on("error", () => {}));
  await new Promise<void>((resolve) => silent.listen(0, "127.0.0.1", resolve));
  const root = mkdtempSync(join(tmpdir(), "reylo-silent-"));
  writeFileSync(join(root, "package.json"), "{}");
  const session = await Session.open(root, { DATABASE_URL: `postgres://postgres@127.0.0.1:${(silent.address() as AddressInfo).port}/x` });
  try {
    const t0 = Date.now();
    const results = await Promise.all([session.analyze("select 1"), session.analyze("select 2"), session.analyze("select 3")]);
    for (const r of results) {
      assert.ok(!r.ok);
      assert.equal(r.code, 5);
      assert.match(r.message, /^Cannot connect to the database at 127\.0\.0\.1:\d+\/x/);
    }
    assert.ok(Date.now() - t0 < 15_000, `took ${Date.now() - t0} ms`);
  } finally {
    await session.close();
    silent.close();
  }
});

test("the files a Postgres URL names are resolved from the working directory and mapped to the engine's paths", async () => {
  const { url } = await hostUrl("postgres://u@h/d?sslmode=verify-ca&sslrootcert=certs/root.pem&sslcert=%2Fabs%2Fc.pem&sslkey=k.pem&application_name=a+b");
  const params = new URLSearchParams(url.slice(url.indexOf("?") + 1));
  assert.equal(params.get("sslrootcert"), resolve("certs/root.pem"));
  assert.equal(params.get("sslcert"), "/abs/c.pem");
  assert.equal(params.get("sslkey"), resolve("k.pem"));
  // Every other parameter is passed on as written.
  assert.match(url, /&application_name=a\+b$/);
  assert.equal((await hostUrl("postgres://u@h/d?sslrootcert=system")).url, "postgres://u@h/d?sslrootcert=system");
  assert.equal((await hostUrl("mysql://root@h/d?ssl=true")).url, "mysql://root@h/d?ssl=true");
  assert.equal(guestPath("C:\\app\\dev.db", "win32"), "/C:/app/dev.db");
  assert.equal(guestPath("\\\\host\\share\\dev.db", "win32"), "/host/share/dev.db");
  assert.equal(guestPath("/app/dev.db", "linux"), "/app/dev.db");
});

test("a URL that names a Unix socket is opened through a bridge, on Postgres and on MySQL", { skip: noPg || noMysql }, async () => {
  const pg = await socketFor(PG!, ".s.PGSQL.5432");
  const my = await socketFor(MYSQL!, "mysql.sock");
  const { pathname: database, username } = new URL(PG!);
  try {
    for (const [dialect, url] of [
      ["postgres", `postgres://${username}@/${database.slice(1)}?host=${pg.dir}`],
      ["postgres", `postgres://${username}@${encodeURIComponent(pg.dir)}/${database.slice(1)}`],
      ["mysql", `mysql://root@localhost/?socketPath=${encodeURIComponent(my.path)}`],
    ] as const) {
      const c = await (await engine(dialect, ROOT)).open(url);
      assert.deepEqual((await c.analyze("select 1 as x")).columns.map((col) => col.name), ["x"], url);
      await c.close();
    }
  } finally {
    pg.server.close();
    my.server.close();
  }
});

test("a connection the server closed while it sat idle is opened again for the next request, on Postgres and on MySQL", { skip: noPg || noMysql }, async () => {
  for (const [dialect, url, id, kill] of [
    ["postgres", PG!, "select pg_backend_pid()", (n: unknown) => `select pg_terminate_backend(${n})`],
    ["mysql", MYSQL!, "select connection_id()", (n: unknown) => `kill ${n}`],
  ] as const) {
    const e = await engine(dialect, ROOT);
    const [c, other] = [await e.open(url), await e.open(url)];
    try {
      const [[backend]] = (await c.query(id, [])) as [[unknown]];
      await other.query(kill(backend), []).catch(() => {});
      await new Promise((done) => setTimeout(done, 300));
      assert.deepEqual((await c.analyze("select 2 as y")).columns.map((col) => col.name), ["y"], dialect);
      const [[again]] = (await c.query(id, [])) as [[unknown]];
      assert.notEqual(again, backend, dialect);
    } finally {
      await c.close();
      await other.close();
    }
  }
});

test("a connection waiting on a server that never answers holds up no other connection", { skip: noPg }, async () => {
  const silent = createServer((socket) => socket.on("error", () => {}));
  await new Promise<void>((done) => silent.listen(0, "127.0.0.1", done));
  try {
    const e = await engine("postgres", ROOT);
    const waiting = e.open(`postgres://postgres@127.0.0.1:${(silent.address() as AddressInfo).port}/x`).catch((err: unknown) => err);
    const t0 = Date.now();
    const c = await e.open(PG!);
    assert.deepEqual((await c.analyze("select 3 as z")).columns.map((col) => col.name), ["z"]);
    assert.ok(Date.now() - t0 < 5_000, `took ${Date.now() - t0} ms`);
    await c.close();
    assert.ok((await waiting) instanceof Unreachable);
  } finally {
    silent.close();
  }
});

test("the engine runs under Node's watch mode, whose report its workers do not inherit", () => {
  // `node --watch` runs its process with WATCH_REPORT_DEPENDENCIES set; a worker that inherits it
  // reports each module it loads to its parent as a message.
  const script = `const { engine } = await import(${JSON.stringify(new URL("../src/engine.ts", import.meta.url).href)});
const c = await (await engine("sqlite", ${JSON.stringify(ROOT)})).open(":memory:");
console.log((await c.analyze("select 1 as x")).columns[0].name);
await c.close();`;
  const run = spawnSync(process.execPath, ["--input-type=module", "-e", script], { env: { ...process.env, WATCH_REPORT_DEPENDENCIES: "1" }, encoding: "utf8", timeout: 30_000 });
  assert.equal(run.stderr, "");
  assert.equal(run.stdout.trim(), "x");
});
