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
import { mysqlStandIn } from "./mysql-stand-in.ts";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const PG = process.env.REYLO_TEST_PG;
const MYSQL = process.env.REYLO_TEST_MYSQL;
const noPg = !PG && "REYLO_TEST_PG names no server";
const noMysql = !MYSQL && "REYLO_TEST_MYSQL names no server";
// Node listens on no Unix socket on Windows, where a path names a named pipe.
const noUnixSockets = process.platform === "win32" && "Node opens no Unix socket on Windows";

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
  assert.equal(params.get("sslrootcert"), guestPath(resolve("certs/root.pem")));
  assert.equal(params.get("sslcert"), guestPath(resolve("/abs/c.pem")));
  assert.equal(params.get("sslkey"), guestPath(resolve("k.pem")));
  // Every other parameter is passed on as written.
  assert.match(url, /&application_name=a\+b$/);
  assert.equal((await hostUrl("postgres://u@h/d?sslrootcert=system")).url, "postgres://u@h/d?sslrootcert=system");
  assert.equal((await hostUrl("mysql://root@h/d?ssl=true")).url, "mysql://root@h/d?ssl=true");
  assert.equal(guestPath("C:\\app\\dev.db", "win32"), "/C:/app/dev.db");
  assert.equal(guestPath("\\\\host\\share\\dev.db", "win32"), "/host/share/dev.db");
  assert.equal(guestPath("/app/dev.db", "linux"), "/app/dev.db");
});

test("a URL that names a Unix socket is opened through a bridge, on Postgres and on MySQL", { skip: noUnixSockets || noPg || noMysql }, async () => {
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
      // The bridge's token never reaches the server, which shows a session's attributes to its accounts.
      if (dialect === "mysql") assert.deepEqual(await c.query("select count(*) from performance_schema.session_connect_attrs where attr_name = 'reylo_bridge'", []), [[0]]);
      await c.close();
    }
  } finally {
    pg.server.close();
    my.server.close();
  }
});

test("a json or jsonb column of a Postgres query reads as its value, as the server-side extension's answers do", { skip: noPg }, async () => {
  const c = await (await engine("postgres", ROOT)).open(PG!);
  try {
    const rows = await c.query(`select '{"a": [1, null]}'::pg_catalog.jsonb as b, '[true]'::pg_catalog.json as j, null::pg_catalog.jsonb as n`, []);
    assert.deepEqual(rows, [[{ a: [1, null] }, [true], null]]);
  } finally {
    await c.close();
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
    const healthy = (async () => {
      const c = await e.open(PG!);
      try {
        return (await c.analyze("select 3 as z")).columns.map((col) => col.name);
      } finally {
        await c.close();
      }
    })();
    // The healthy connection's analysis ends while the other still waits out its connect timeout.
    assert.equal(await Promise.race([healthy.then(() => "healthy"), waiting.then(() => "silent")]), "healthy");
    assert.deepEqual(await healthy, ["z"]);
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

/** A connection to `url`'s host and port that writes `bytes`, and resolves with what it read once the bridge closed it. */
async function knock(url: string, bytes: Buffer): Promise<Buffer> {
  const { hostname, port } = new URL(url.replace(/^[a-z]+:/, "http:"));
  const socket = connect(Number(port), hostname);
  const read: Buffer[] = [];
  socket.on("data", (chunk: Buffer) => read.push(chunk));
  socket.on("error", () => {});
  socket.write(bytes);
  await new Promise((done) => socket.on("close", done));
  return Buffer.concat(read);
}

test("the bridge to a Unix socket closes a connection that does not carry its token, before the server reads it", { skip: noUnixSockets }, async () => {
  // Postgres: the socket is not even opened for a connection that does not open with the token.
  const dir = mkdtempSync(join(tmpdir(), "reylo-sock-"));
  let reached = 0;
  const pgSocket = createServer((socket) => {
    reached++;
    socket.destroy();
  });
  await new Promise<void>((done) => pgSocket.listen(join(dir, ".s.PGSQL.5432"), done));
  // MySQL: the server speaks first, so the stand-in is reached, but no login reaches it.
  const standIn = await mysqlStandIn({ version: "8.4.6" });
  const my = await socketFor(`mysql://root@127.0.0.1:${standIn.port}`, "mysql.sock");
  try {
    const pg = await hostUrl(`postgres://postgres@/app?host=${dir}`);
    assert.ok(pg.bridge && pg.url.startsWith("postgres://postgres@127.0.0.1:"), pg.url);
    // A startup message for user postgres, as another local process would send it.
    const startup = Buffer.from("\0\0\0\x24\0\x03\0\0user\0postgres\0database\0app\0\0", "latin1");
    assert.equal((await knock(pg.url, Buffer.concat([startup, startup]))).length, 0);
    assert.equal(reached, 0);
    pg.close();
    const mysql = await hostUrl(`mysql://root@localhost/app?socketPath=${encodeURIComponent(my.path)}`);
    assert.ok(mysql.bridge && mysql.url.startsWith("mysql://root@127.0.0.1:"), mysql.url);
    // A login with no connection attributes, after the greeting the bridge passes on.
    const login = Buffer.alloc(36);
    login.writeUIntLE(32, 0, 3);
    login[3] = 1;
    login.writeUInt32LE(0x200 | 0x8000, 4);
    const greeting = await knock(mysql.url, login);
    assert.ok(greeting.length > 4 && greeting[4] === 10, "the greeting reached the client");
    // The greeting's capabilities offer no TLS, so the login that answers it is read in plain text.
    // After the protocol byte and the version, the connection id (4), the scramble's first part (8) and a filler byte.
    const capabilities = greeting.readUInt16LE(greeting.indexOf(0, 5) + 1 + 4 + 8 + 1);
    assert.equal(capabilities & 0x0800, 0);
    assert.deepEqual(standIn.tried, []);
    mysql.close();
  } finally {
    pgSocket.close();
    my.server.close();
    await standIn.close();
  }
});

test("PGHOST names the socket's directory where the URL names no host, as libpq reads it", async () => {
  const bridged = await hostUrl("postgres://u@/d", { PGHOST: "/var/run/postgresql", PGPORT: "5433" });
  assert.ok(bridged.bridge);
  assert.match(bridged.url, /^postgres:\/\/u@127\.0\.0\.1:\d+\/d$/);
  bridged.close();
  // A host in the URL, or a TCP host in PGHOST, is no socket.
  assert.equal((await hostUrl("postgres://u@db/d", { PGHOST: "/var/run/postgresql" })).bridge, undefined);
  assert.equal((await hostUrl("postgres://u@/d", { PGHOST: "db.internal" })).bridge, undefined);
});
