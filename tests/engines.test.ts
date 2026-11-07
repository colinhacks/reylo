import { test } from "node:test";
import assert from "node:assert/strict";
import { chmodSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { anyEngine, engine, MissingEngine, Unreachable } from "../src/engine.ts";
import { nativeBinary, PLATFORM_PACKAGES, platformPackage } from "../src/native.ts";
import { resolveProject } from "../src/project.ts";
import { DRIVERS, PG, postgresDatabase } from "./helpers.ts";

// The engine is one executable for every database, `reylo engine <database>` (src/native.ts),
// which reylo finds from REYLO_ENGINE_PATH, its platform's package, or a checkout's cargo build,
// and refuses where it is another version than reylo.
const VERSION: string = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).version;
// A stand-in for the executable is a shell script, which Windows does not run.
const noShell = process.platform === "win32" && "a stand-in engine is a shell script";

/** A stand-in for the executable that answers every request line with `answer`, carrying its `id` back as the engine does. */
function standIn(answer: object): string {
  const path = join(mkdtempSync(join(tmpdir(), "reylo-engine-")), "reylo");
  const text = JSON.stringify(answer).slice(0, -1);
  writeFileSync(path, `#!/bin/sh\nwhile read -r line; do\n  id=$(printf '%s' "$line" | sed -n 's/.*"id":\\([0-9]*\\).*/\\1/p')\n  echo '${text},"id":'"$id"'}'\ndone\n`);
  chmodSync(path, 0o755);
  return path;
}

/** `run` with REYLO_ENGINE_PATH set to `path`, and set back after. */
async function withEngine<T>(path: string, run: () => Promise<T>): Promise<T> {
  const before = process.env.REYLO_ENGINE_PATH;
  process.env.REYLO_ENGINE_PATH = path;
  try {
    return await run();
  } finally {
    if (before === undefined) delete process.env.REYLO_ENGINE_PATH;
    else process.env.REYLO_ENGINE_PATH = before;
  }
}

test("this platform has a package of the executable, named as reylo's optional dependencies name it", () => {
  const { key, pkg } = platformPackage();
  assert.equal(pkg, PLATFORM_PACKAGES[key]);
  assert.match(key, /^(darwin|linux|win32)-(arm64|x64)(-musl)?$/);
  assert.equal(pkg, `@reylosql/engine-${key}`);
});

test("REYLO_ENGINE_PATH comes first, and one that names no file is refused, saying so", async () => {
  const path = standIn({ status: "ok", version: VERSION, fingerprint: "0123456789abcdef", database: "postgres" });
  await withEngine(path, async () => assert.equal(nativeBinary(), path));
  const missing = join(tmpdir(), "reylo-no-such-engine");
  await withEngine(missing, async () => {
    assert.throws(() => nativeBinary(), { message: `REYLO_ENGINE_PATH names ${missing}, which does not exist: point it at a reylo executable, or unset it to use the one reylo installs` });
    await assert.rejects(engine("postgres"), (e: Error) => e instanceof MissingEngine && e.message.startsWith("REYLO_ENGINE_PATH names"));
    // Not even the project file can be read without one.
    const root = mkdtempSync(join(tmpdir(), "reylo-engines-"));
    writeFileSync(join(root, "package.json"), "{}");
    writeFileSync(join(root, ".reylo.yaml"), "direct_url: postgres://localhost/app\n");
    const reader = await anyEngine();
    assert.ok(reader instanceof MissingEngine);
    assert.match(resolveProject(reader, root, {}).error!, /^\.reylo\.yaml: REYLO_ENGINE_PATH names /);
  });
});

test("an engine of another version than reylo is refused, naming both versions and how to get the right one", { skip: noShell }, async () => {
  const path = standIn({ status: "ok", version: "9.9.9", fingerprint: "0123456789abcdef", database: "postgres" });
  await withEngine(path, () =>
    assert.rejects(engine("postgres"), (e: Error) => {
      assert.ok(e instanceof MissingEngine);
      assert.equal(e.message, `reylo ${VERSION} runs with its engine at ${VERSION}, and ${path} is 9.9.9: install reylo again, point REYLO_ENGINE_PATH at the engine of reylo ${VERSION}, or in a checkout of reylo's repository build it with cargo build --release -p reylo`);
      return true;
    }),
  );
});

test("an executable that answers as another database's engine, or not at all, is refused", { skip: noShell }, async () => {
  const other = standIn({ status: "ok", version: VERSION, fingerprint: "0123456789abcdef", database: "sqlite" });
  await withEngine(other, () => assert.rejects(engine("postgres"), { message: `${other} serves sqlite's engine when asked for postgres's` }));
  const broken = join(mkdtempSync(join(tmpdir(), "reylo-engine-")), "reylo");
  writeFileSync(broken, "#!/bin/sh\necho 'no such thing' >&2\nexit 3\n");
  chmodSync(broken, 0o755);
  await withEngine(broken, () => assert.rejects(engine("postgres"), { message: `the engine ${broken} does not answer: the engine ${broken} stopped (3): no such thing` }));
  // One that starts and never answers is refused once the ping has waited its while.
  const silent = join(mkdtempSync(join(tmpdir(), "reylo-engine-")), "reylo");
  writeFileSync(silent, "#!/bin/sh\nexec sleep 60\n");
  chmodSync(silent, 0o755);
  await withEngine(silent, () => assert.rejects(engine("postgres"), { message: `the engine ${silent} does not answer: it did not answer a ping in 10 s` }));
});

test("each database's engine names its database, and refuses another database's statement by name", async () => {
  for (const dialect of ["postgres", "mysql", "sqlite", "duckdb"] as const) {
    const e = await engine(dialect);
    const ping = e.call({ op: "ping" }) as { database?: string; fingerprint: string; version: string };
    assert.deepEqual([ping.database, ping.version], [dialect, VERSION]);
    assert.match(ping.fingerprint, /^[0-9a-f]{16}$/);
    assert.equal(e.fingerprint, ping.fingerprint);
    const other = dialect === "postgres" ? "mysql" : "postgres";
    assert.deepEqual(e.call({ op: "analyze", sql: "select 1", dialect: other, answers: {} }), { status: "error", message: `this engine reads ${dialect} statements, not ${other} ones` });
  }
});

test("a closed connection's child has ended, its requests refused as a lost connection's are, and the next connection starts another", async () => {
  const e = await engine("sqlite");
  const c = await e.open(":memory:");
  assert.deepEqual((await c.analyze("select 1 as x")).columns.map((col) => col.name), ["x"]);
  await c.close();
  await assert.rejects(c.analyze("select 1"), (err: Error) => err instanceof Unreachable && err.message === "the connection is closed");
  const again = await e.open(":memory:");
  assert.deepEqual((await again.analyze("select 2 as y")).columns.map((col) => col.name), ["y"]);
  await again.close();
});

// A host that runs the engine's analysis itself, over the connection the engine holds, answers a
// describe the server refuses with the server's code and the character its message points at: the
// engine reads a SQL function's body by where the server says a name in it is no column.
const livePostgres = !PG ? "REYLO_TEST_PG names no server" : !existsSync(join(DRIVERS, "postgres")) ? "lab/drivers is not installed" : false;

test("an analysis run over the engine's own connection reads a SQL function's body, by where the server's refusal points", { skip: livePostgres }, async () => {
  const own = await postgresDatabase(
    "reylo_engine_bodies",
    "create table comments (id int8 primary key, post_id int8 not null, body text not null); create function one_comment(x int8) returns table (id int8, body text) language sql stable as $$ select id, body from comments where id = x $$;",
  );
  const e = await engine("postgres");
  const c = await e.open(own.url);
  try {
    await assert.rejects(c.describe("select id, body from comments where id = x"), (err: Error & { code?: string; position?: number }) => {
      assert.deepEqual([err.message, err.code, err.position], ['column "x" does not exist', "42703", 42]);
      return true;
    });
    const analysis = await e.analyze("select id, body from one_comment(:x::int8)", "postgres", { describe: (sql) => c.describe(sql), query: (sql, params) => c.query(sql, params) });
    assert.equal(analysis.cardinality, "at_most_one");
    assert.deepEqual(analysis.columns.map((col) => [col.name, col.nullable ?? false]), [["id", false], ["body", false]]);
  } finally {
    await c.close();
    await own.drop();
  }
});
