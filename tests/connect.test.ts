import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createServer, type AddressInfo } from "node:net";
import { Session } from "../src/analyze.ts";
import { engine, reason, Unreachable } from "../src/engine.ts";

const ROOT = fileURLToPath(new URL("..", import.meta.url));

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
