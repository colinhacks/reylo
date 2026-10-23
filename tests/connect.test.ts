import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer, type AddressInfo } from "node:net";
import { reason, Unreachable } from "../src/engine.ts";
import { forPostgresJs, openPostgresDescriber } from "../packages/postgres/describe.ts";
import { openMysqlDescriber } from "../packages/mysql/describe.ts";

const unreachable = (r: PromiseSettledResult<unknown>, pattern: RegExp) => {
  assert.equal(r.status, "rejected");
  const e = (r as PromiseRejectedResult).reason;
  assert.ok(e instanceof Unreachable, `expected Unreachable, got ${e}`);
  assert.match(e.message, pattern);
};

test("reason reads a refused connection, an AggregateError with no message of its own", () => {
  const refused = Object.assign(new AggregateError([new Error("connect ECONNREFUSED ::1:1"), new Error("connect ECONNREFUSED 127.0.0.1:1")], ""), { code: "ECONNREFUSED" });
  assert.equal(reason(refused), "connect ECONNREFUSED ::1:1; connect ECONNREFUSED 127.0.0.1:1");
  assert.equal(reason(Object.assign(new Error(""), { code: "ETIMEDOUT" })), "ETIMEDOUT");
});

test("postgres.js: a refused connection fails every call as Unreachable", async () => {
  const d = await openPostgresDescriber("postgres://localhost:1/x");
  const results = await Promise.allSettled([d.values("select 1"), d.describe("select 2"), d.query("select 3")]);
  for (const r of results) unreachable(r, /ECONNREFUSED/);
  await d.close();
});

test("postgres.js: calls queued behind a server that never answers fail together at the first timeout", { timeout: 30_000 }, async () => {
  // A server that accepts the socket and never says anything: the connection times out, and
  // postgres.js would connect again for each queued call, one timeout after another. The client
  // resets the socket when it gives up, which the server's side hears as ECONNRESET.
  const silent = createServer((socket) => socket.on("error", () => {}));
  await new Promise<void>((resolve) => silent.listen(0, "127.0.0.1", resolve));
  try {
    const d = await openPostgresDescriber(`postgres://127.0.0.1:${(silent.address() as AddressInfo).port}/x`);
    const t0 = Date.now();
    const results = await Promise.allSettled([d.values("select 1"), d.values("select 2"), d.values("select 3")]);
    for (const r of results) unreachable(r, /CONNECT_TIMEOUT/);
    assert.ok(Date.now() - t0 < 15_000, `took ${Date.now() - t0} ms`);
    await d.close();
  } finally {
    silent.close();
  }
});

test("mysql2: a refused connection is Unreachable", async () => {
  const [r] = await Promise.allSettled([openMysqlDescriber("mysql://root@localhost:1/x")]);
  unreachable(r!, /ECONNREFUSED/);
});

test("postgres.js gets no channel_binding parameter, and one that requires binding gets a verified certificate", () => {
  assert.deepEqual(forPostgresJs("postgresql://u:p%40ss@ep-x-pooler.aws.neon.tech/neondb?sslmode=require&channel_binding=require"), { url: "postgresql://u:p%40ss@ep-x-pooler.aws.neon.tech/neondb?sslmode=require", ssl: "verify-full" });
  assert.deepEqual(forPostgresJs("postgres://localhost/db?channel_binding=prefer"), { url: "postgres://localhost/db" });
  assert.deepEqual(forPostgresJs("postgres://localhost/db?channel_binding=disable&application_name=x"), { url: "postgres://localhost/db?application_name=x" });
  assert.deepEqual(forPostgresJs("postgres://localhost/db?sslmode=disable"), { url: "postgres://localhost/db?sslmode=disable" });
  assert.deepEqual(forPostgresJs("postgres://localhost/db"), { url: "postgres://localhost/db" });
});

test("postgres.js gets libpq's TLS files as TLS options, not as server settings", () => {
  const dir = mkdtempSync(join(tmpdir(), "reylo-tls-"));
  writeFileSync(join(dir, "ca.pem"), "CA");
  writeFileSync(join(dir, "c.pem"), "CERT");
  writeFileSync(join(dir, "k.pem"), "KEY");
  try {
    const full = forPostgresJs(`postgres://h/db?sslmode=verify-full&sslrootcert=${dir}/ca.pem&sslcert=${dir}/c.pem&sslkey=${dir}/k.pem&sslpassword=pw&application_name=x`);
    assert.equal(full.url, "postgres://h/db?sslmode=verify-full&application_name=x");
    const ssl = full.ssl as import("node:tls").ConnectionOptions;
    assert.equal(String(ssl.ca), "CA");
    assert.equal(String(ssl.cert), "CERT");
    assert.equal(String(ssl.key), "KEY");
    assert.equal(ssl.passphrase, "pw");
    assert.equal(ssl.rejectUnauthorized, true);
    assert.equal(ssl.checkServerIdentity, undefined);
    const ca = forPostgresJs(`postgres://h/db?sslmode=verify-ca&sslrootcert=${dir}/ca.pem`).ssl as import("node:tls").ConnectionOptions;
    assert.equal(ca.rejectUnauthorized, true);
    assert.equal(typeof ca.checkServerIdentity, "function");
    const required = forPostgresJs(`postgres://h/db?sslmode=require&sslrootcert=${dir}/ca.pem`).ssl as import("node:tls").ConnectionOptions;
    assert.equal(required.rejectUnauthorized, true);
    assert.equal(typeof required.checkServerIdentity, "function");
    assert.equal((forPostgresJs(`postgres://h/db?sslmode=require&sslcert=${dir}/c.pem&sslkey=${dir}/k.pem`).ssl as import("node:tls").ConnectionOptions).rejectUnauthorized, false);
    const system = forPostgresJs(`postgres://h/db?sslmode=verify-full&sslrootcert=system&sslcert=${dir}/c.pem&sslkey=${dir}/k.pem`).ssl as import("node:tls").ConnectionOptions;
    assert.equal(system.ca, undefined);
    assert.equal(String(system.cert), "CERT");
    assert.equal(system.rejectUnauthorized, true);
    assert.equal(system.checkServerIdentity, undefined);
    assert.deepEqual(forPostgresJs(`postgres://h/db?sslmode=disable&sslrootcert=${dir}/ca.pem`), { url: "postgres://h/db?sslmode=disable" });
    assert.deepEqual(forPostgresJs("postgres://h/db?sslmode=verify-full&sslrootcert=system"), { url: "postgres://h/db?sslmode=verify-full&sslrootcert=system" });
  } finally {
    rmSync(dir, { recursive: true });
  }
});
