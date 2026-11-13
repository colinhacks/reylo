// The `reylo` command, src/cli.ts: the native binary run with the command's arguments, standard
// streams and exit code, and `reylo play`, which runs in JavaScript.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";

const CLI = join(import.meta.dirname, "..", "src", "cli.ts");
const VERSION = (JSON.parse(readFileSync(join(import.meta.dirname, "..", "package.json"), "utf8")) as { version: string }).version;
// A child's environment is PATH alone, and REYLO_ENGINE_PATH where it names the engine, as on CI; without it, the child takes this checkout's cargo build.
const ENV = { PATH: process.env.PATH, ...(process.env.REYLO_ENGINE_PATH ? { REYLO_ENGINE_PATH: process.env.REYLO_ENGINE_PATH } : {}) };

function reylo(cwd: string, args: string[], options: { env?: Record<string, string>; input?: string } = {}) {
  const r = spawnSync(process.execPath, [CLI, ...args], { cwd, encoding: "utf8", input: options.input, env: { ...ENV, ...options.env } });
  return { status: r.status, out: r.stdout, err: r.stderr };
}

/** A package whose `.env` names a SQLite database of one table. */
function project(): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-cli-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), "DATABASE_URL=file:./app.db\n");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null)");
  db.close();
  return root;
}

test("the reylo command runs the binary with its arguments, standard input and exit code", () => {
  const root = project();
  const version = reylo(root, ["--version"]);
  assert.deepEqual(version, { status: 0, out: `reylo ${VERSION}\n`, err: "" });
  const typed = reylo(root, ["analyze", "-", "--format", "json"], { input: "select name from users where id = :id" });
  assert.equal(typed.status, 0, typed.err);
  const [line] = typed.out.trim().split("\n").map((l) => JSON.parse(l) as { result: { columns: { name: string }[]; cardinality: string } });
  assert.deepEqual([line!.result.columns.map((c) => c.name), line!.result.cardinality], [["name"], "at_most_one"]);
  // A statement with a diagnostic exits with 1, as the binary does.
  assert.equal(reylo(root, ["analyze", "--sql", "select nope from users"]).status, 1);
  const unknown = reylo(root, ["frobnicate"]);
  assert.equal(unknown.status, 2);
  assert.match(unknown.err, /^reylo: unknown command frobnicate\n\nUsage: reylo <command>/);
});

test("reylo generate is gone, and says what fills .reylo/ instead", () => {
  const gone = reylo(project(), ["generate"]);
  assert.equal(gone.status, 2);
  assert.equal(gone.err, "reylo generate was removed in 0.8.0: the type check and the build steps fill .reylo/ where the database can be reached, and reylo analyze prints each statement's types.\n");
});

test("reylo play is the playground's server, in JavaScript", () => {
  const help = reylo(project(), ["play", "--help"]);
  assert.equal(help.status, 0, help.err);
  assert.match(help.out, /^Usage: reylo play \[--port <n>\] \[--no-open\]/);
});

test("the reylo command names a binary it cannot find", () => {
  const missing = reylo(project(), ["--version"], { env: { REYLO_ENGINE_PATH: join(tmpdir(), "no-such-reylo") } });
  assert.equal(missing.status, 1);
  assert.match(missing.err, /^reylo: REYLO_ENGINE_PATH names .*no-such-reylo, which does not exist/);
});

test("a signal that stops the reylo command stops the binary, and the command reports it as a shell does", { skip: process.platform === "win32" && "Windows has no signals to pass on" }, async () => {
  // The engine's program waits on its standard input, so it ends only when the signal reaches it.
  const child = spawn(process.execPath, [CLI, "engine", "sqlite"], { env: ENV, stdio: ["pipe", "pipe", "inherit"] });
  const lines = createInterface({ input: child.stdout! });
  const answered = new Promise<string>((done) => lines.once("line", done));
  child.stdin!.write(`${JSON.stringify({ op: "ping" })}\n`);
  assert.match(await answered, /"status":"ok"/);
  const ended = new Promise<number | null>((done) => child.once("exit", (code) => done(code)));
  child.kill("SIGTERM");
  assert.equal(await ended, 128 + 15);
});
