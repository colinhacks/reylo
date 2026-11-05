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
  const typed = reylo(root, ["compile", "-"], { input: "select name from users where id = :id" });
  assert.equal(typed.status, 0, typed.err);
  const [line] = typed.out.trim().split("\n").map((l) => JSON.parse(l) as { columns: { name: string }[]; cardinality: string });
  assert.deepEqual([line!.columns.map((c) => c.name), line!.cardinality], [["name"], "at_most_one"]);
  // A statement with a diagnostic exits with 1, as the binary does.
  assert.equal(reylo(root, ["compile", "--sql", "select nope from users"]).status, 1);
  const unknown = reylo(root, ["frobnicate"]);
  assert.equal(unknown.status, 2);
  assert.match(unknown.err, /^reylo: unknown command frobnicate\n\nUsage: reylo <command>/);
});

test("reylo migrate and reylo run are the binary's, with their output and exit codes", () => {
  const root = project();
  const help = reylo(root, ["--help"]);
  assert.equal(help.status, 0, help.err);
  for (const usage of ["  migrate <command>", "  run \"<sql>\""]) assert.ok(help.out.includes(usage), usage);
  const migrate = reylo(root, ["migrate", "--help"]);
  assert.equal(migrate.status, 0, migrate.err);
  assert.match(migrate.out, /^Usage: reylo migrate <command> \[flags\]\n/);
  // The docs show the help as it is.
  assert.ok(readFileSync(join(import.meta.dirname, "..", "site", "content", "docs", "migrate.mdx"), "utf8").includes(`\`\`\`terminal\n${migrate.out}\`\`\``));
  const run = reylo(root, ["run", "insert into users (name) values ('ada')", "insert into users (name) values ('grace')"]);
  assert.equal(run.status, 0, run.err);
  const db = new DatabaseSync(join(root, "app.db"));
  assert.deepEqual({ ...db.prepare("select count(*) as n from users").get() }, { n: 2 });
  db.close();
  // A flag's value after an `=` is its value, so that this runs on the database it names.
  const other = join(root, "other.db");
  assert.equal(reylo(root, ["run", `--url=sqlite:${other}`, "create table here (id integer)"]).status, 0);
  const elsewhere = new DatabaseSync(other);
  assert.deepEqual({ ...elsewhere.prepare("select count(*) as n from sqlite_schema where name = 'here'").get() }, { n: 1 });
  elsewhere.close();
  const missing = reylo(root, ["run", "--file", "nope.sql"]);
  assert.equal(missing.status, 1);
  assert.match(missing.err, /^reylo run: cannot read nope\.sql: /);
  assert.notEqual(reylo(root, ["migrate", "sideways"]).status, 0);
  // A flag a command doesn't take is refused, never passed over: this must not run prod.
  const flag = reylo(root, ["migrate", "prod", "--dev"]);
  assert.equal(flag.status, 1);
  assert.match(flag.err, /^reylo migrate: prod doesn't take --dev/);
  for (const [args, said] of [[["migrate", "sync", "--rename", "users.a=b"], /sync doesn't take --rename/], [["migrate", "dev", "--drop", "users.a"], /dev doesn't take --drop/], [["migrate", "prod", "--schema", "x"], /prod doesn't take --schema/], [["migrate", "prod", "-f", "x.sql"], /prod doesn't take --file/], [["migrate", "prod", "dev"], /prod doesn't take dev/], [["migrate", "create", "a", "b"], /create doesn't take b/], [["migrate", "dev", "--dir", "--watch"], /--dir needs a value/], [["run", "--bogus", "select 1"], /run doesn't take --bogus/]] as const) {
    const r = reylo(root, [...args]);
    assert.equal(r.status, 1, args.join(" "));
    assert.match(r.err, said);
  }
  assert.match(reylo(root, ["migrate", "prod", "--help"]).out, /^Usage: reylo migrate <command> \[flags\]\n/);
});

test("reylo migrate init starts a project: it adds the migrate section the other commands need", () => {
  const root = project();
  const refused = reylo(root, ["migrate", "check"]);
  assert.equal(refused.status, 1);
  assert.match(refused.err, /no migrate section in \.reylo\.yaml: start with reylo migrate init/);
  const init = reylo(root, ["migrate", "init"]);
  assert.equal(init.status, 0, init.err + init.out);
  assert.match(init.out, /✓ Wrote reylo\/schema\.sql, reylo\/migrations\/init\.sql and reylo\/reylo-lock\.yaml\n[\s\S]*✓ Added migrate to \.reylo\.yaml\n$/);
  assert.equal(readFileSync(join(root, ".reylo.yaml"), "utf8"), "migrate:\n  lock: true\n");
  // Beside what the file holds; the section the file has already is left as it is.
  writeFileSync(join(root, ".reylo.yaml"), "strict_params: true\n");
  const next = project();
  writeFileSync(join(next, ".reylo.yaml"), "strict_params: true");
  assert.equal(reylo(next, ["migrate", "init"]).status, 0);
  assert.equal(readFileSync(join(next, ".reylo.yaml"), "utf8"), "strict_params: true\nmigrate:\n  lock: true\n");
  // unlock and lock say so in the file.
  assert.equal(reylo(next, ["migrate", "unlock"]).status, 0);
  assert.equal(readFileSync(join(next, ".reylo.yaml"), "utf8"), "strict_params: true\nmigrate:\n  lock: false\n");
  assert.equal(reylo(next, ["migrate", "lock"]).status, 0);
  assert.equal(readFileSync(join(next, ".reylo.yaml"), "utf8"), "strict_params: true\nmigrate:\n  lock: true\n");
});

test("reylo generate is gone, and says what fills reylo/.cache/ instead", () => {
  const gone = reylo(project(), ["generate"]);
  assert.equal(gone.status, 2);
  assert.equal(gone.err, "reylo generate was removed in 0.8.0: the type check and the build steps fill reylo/.cache/ where the database can be reached, and reylo compile prints each statement's types.\n");
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
