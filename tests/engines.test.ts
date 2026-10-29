import { test } from "node:test";
import assert from "node:assert/strict";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { anyEngine, engine, enginePath, MissingEngine } from "../src/engine.ts";
import { resolveProject } from "../src/project.ts";

// Each database's engine is a package of its own (packages/<database>), which reylo finds from the
// project first, then from its own install. Here reylo's own install is a directory with no
// package in it, so that only what each project installs counts.
const PACKAGES = fileURLToPath(new URL("../packages", import.meta.url));
const nowhere = pathToFileURL(join(mkdtempSync(join(tmpdir(), "reylo-no-engines-")), "reylo.js")).href;
const VERSION: string = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).version;

/** A project whose node_modules holds `installed`: each engine package by name, as a directory to link or a version to copy at. */
function project(installed: Record<string, string | { from: string; version: string }> = {}, files: Record<string, string> = {}): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-engines-"));
  writeFileSync(join(root, "package.json"), "{}");
  for (const [path, text] of Object.entries(files)) writeFileSync(join(root, path), text);
  mkdirSync(join(root, "node_modules/@reylosql"), { recursive: true });
  for (const [name, source] of Object.entries(installed)) {
    const at = join(root, "node_modules/@reylosql", name);
    if (typeof source === "string") symlinkSync(source, at);
    else {
      cpSync(source.from, at, { recursive: true });
      writeFileSync(join(at, "package.json"), JSON.stringify({ name: `@reylosql/${name}`, version: source.version, type: "module", exports: { "./engine/*": "./engine/*", "./package.json": "./package.json" } }));
    }
  }
  return root;
}

test("a project with no engine installed is told which package its database needs, and how to install it", async () => {
  const root = project();
  assert.equal(enginePath("postgres", root, nowhere), undefined);
  await assert.rejects(engine("postgres", root, nowhere), { name: "MissingEngine", message: "postgres statements need @reylosql/postgres: npm install -D @reylosql/postgres" });
  await assert.rejects(engine("mysql", root, nowhere), { message: "mysql statements need @reylosql/mysql: npm install -D @reylosql/mysql" });
  await assert.rejects(engine("duckdb", root, nowhere), { message: "duckdb statements need @reylosql/duckdb: npm install -D @reylosql/duckdb" });
  // Not even the project file can be read without one.
  const file = project({}, { ".reylo.yaml": "direct_url: postgres://localhost/app\n" });
  const reader = await anyEngine(file, nowhere);
  assert.ok(reader instanceof MissingEngine);
  assert.match(resolveProject(reader, file, {}).error!, /^\.reylo\.yaml: reylo reads the project file with a database's engine, and none is installed: npm install -D @reylosql\/postgres, @reylosql\/mysql, @reylosql\/sqlite or @reylosql\/duckdb/);
});

test("with only SQLite's engine installed, a Postgres URL needs Postgres's, and SQLite's reads the project file", async () => {
  const root = project({ sqlite: join(PACKAGES, "sqlite") }, { ".reylo.yaml": "direct_url: postgres://localhost/app\n" });
  const reader = await anyEngine(root, nowhere);
  assert.ok(!(reader instanceof MissingEngine));
  assert.equal(reader.call({ op: "ping" }).status, "ok");
  const resolved = resolveProject(reader, root, {});
  assert.deepEqual([resolved.dialect, resolved.error], ["postgres", undefined]);
  await assert.rejects(engine(resolved.dialect, resolved.root, nowhere), { message: "postgres statements need @reylosql/postgres: npm install -D @reylosql/postgres" });
  const sqlite = await engine("sqlite", root, nowhere);
  assert.equal((sqlite.call({ op: "ping" }) as { database?: string }).database, "sqlite");
});

test("an engine of another version than reylo is refused, naming both versions and the one to install", async () => {
  const root = project({ postgres: { from: join(PACKAGES, "postgres"), version: "9.9.9" } });
  await assert.rejects(engine("postgres", root, nowhere), (e: Error) => {
    assert.ok(e instanceof MissingEngine);
    assert.ok(e.message.startsWith(`reylo ${VERSION} runs with @reylosql/postgres ${VERSION}, and the one installed is 9.9.9 (`), e.message);
    assert.ok(e.message.endsWith(`): npm install -D @reylosql/postgres@${VERSION}`), e.message);
    return true;
  });
});

test("a package whose artifact reads another database's statements is refused by name", async () => {
  const root = project({ postgres: { from: join(PACKAGES, "sqlite"), version: VERSION } });
  await assert.rejects(engine("postgres", root, nowhere), /^MissingEngine: @reylosql\/postgres \(.*\) carries an engine that reads sqlite statements, not postgres ones/);
});

test("the project's own engine comes before reylo's, and each database's artifact names its database", async () => {
  const root = project({ postgres: join(PACKAGES, "postgres"), mysql: join(PACKAGES, "mysql") });
  assert.equal(enginePath("postgres", root, nowhere), join(PACKAGES, "postgres", "engine"));
  for (const dialect of ["postgres", "mysql"] as const) {
    const e = await engine(dialect, root, nowhere);
    const ping = e.call({ op: "ping" }) as { database?: string; fingerprint: string };
    assert.equal(ping.database, dialect);
    assert.match(ping.fingerprint, /^[0-9a-f]{16}$/);
    // Another database's statement is refused by name.
    const other = dialect === "postgres" ? "mysql" : "postgres";
    assert.deepEqual(e.call({ op: "analyze", sql: "select 1", dialect: other, answers: {} }), { status: "error", message: `this engine reads ${dialect} statements, not ${other} ones` });
  }
});
