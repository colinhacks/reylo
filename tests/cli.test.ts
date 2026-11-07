// The `reylo` command, src/cli.ts: the native binary run with the command's arguments, standard
// streams and exit code, and `reylo play`, which runs in JavaScript.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
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
  const typed = reylo(root, ["analyze", "-"], { input: "select name from users where id = :id" });
  assert.equal(typed.status, 0, typed.err);
  const [line] = typed.out.trim().split("\n").map((l) => JSON.parse(l) as { result: { columns: { name: string }[]; cardinality: string } });
  assert.deepEqual([line!.result.columns.map((c) => c.name), line!.result.cardinality], [["name"], "at_most_one"]);
  // A statement with a diagnostic exits with 1, as the binary does.
  assert.equal(reylo(root, ["analyze", "--sql", "select nope from users"]).status, 1);
  const unknown = reylo(root, ["frobnicate"]);
  assert.equal(unknown.status, 2);
  assert.match(unknown.err, /^reylo: unknown command frobnicate\n\nUsage: reylo <command>/);
});

test("the migration commands and reylo run are the binary's, with their output and exit codes", () => {
  const root = project();
  const help = reylo(root, ["--help"]);
  assert.equal(help.status, 0, help.err);
  for (const usage of ["  migration <command>", "  migrate <command>", "  schema <command>", "  db <command>", "  run \"<sql>\""]) assert.ok(help.out.includes(usage), usage);
  const migrate = reylo(root, ["migrate", "--help"]);
  assert.equal(migrate.status, 0, migrate.err);
  assert.match(migrate.out, /^Usage: reylo <group> <command> \[flags\]\n/);
  for (const group of ["migration", "schema", "db"]) assert.equal(reylo(root, [group, "--help"]).out, migrate.out, group);
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
  // A command of another group is named, the words being so near each other.
  const slip = reylo(root, ["migrate", "create", "add_bio"]);
  assert.equal(slip.status, 2);
  assert.match(slip.err, /^reylo migrate has no create: it's reylo migration create\n/);
  assert.match(reylo(root, ["migration", "push"]).err, /^reylo migration has no push: it's reylo migration history push, or reylo schema push\n/);
  for (const [args, said] of [[["schema", "push", "--rename", "users.a=b"], /push doesn't take --rename/], [["migrate", "dev", "--drop", "users.a"], /dev doesn't take --drop/], [["migrate", "prod", "--schema", "x"], /prod doesn't take --schema/], [["migrate", "prod", "-f", "x.sql"], /prod doesn't take --file/], [["migrate", "prod", "dev"], /prod doesn't take dev/], [["migration", "create", "a", "b"], /create doesn't take b/], [["run", "--bogus", "select 1"], /run doesn't take --bogus/], [["migrate", "dev", "--dir", "db"], /dev doesn't take --dir/], [["migration", "lint", "--offline"], /lint doesn't take --offline/], [["migrate", "dev", "--dry-run"], /dev doesn't take --dry-run/], [["migration", "history", "push", "--watch"], /history push doesn't take --watch/]] as const) {
    const r = reylo(root, [...args]);
    assert.equal(r.status, 1, args.join(" "));
    assert.match(r.err, said);
  }
  assert.match(reylo(root, ["migrate", "prod", "--help"]).out, /^Usage: reylo <group> <command> \[flags\]\n/);
});

test("reylo migration init starts a project, and needs nothing in the project file", () => {
  const root = project();
  const refused = reylo(root, ["migrate", "prod"]);
  assert.equal(refused.status, 1);
  assert.match(refused.err, /no reylo here: check dir in \.reylo\.yaml, or start with reylo migration init/);
  const init = reylo(root, ["migration", "init"]);
  assert.equal(init.status, 0, init.err + init.out);
  assert.match(init.out, /^✓ Wrote reylo\/schema\.sql, reylo\/migrations\/init\.sql and reylo\/reylo-lock\.yaml\n/);
  assert.equal(existsSync(join(root, ".reylo.yaml")), false);
  // With no more than that, every command runs on it.
  for (const args of [["migration", "lint"], ["migrate", "prod", "--dry-run"], ["migrate", "prod"], ["migration", "history", "push"], ["migration", "history", "pull"], ["schema", "pull"]]) {
    const r = reylo(root, args);
    assert.equal(r.status, 0, `${args.join(" ")}: ${r.err}${r.out}`);
  }
  // A key that has gone is refused, as any key the file can't have.
  writeFileSync(join(root, ".reylo.yaml"), "migrate:\n  lock: true\n");
  assert.notEqual(reylo(root, ["migration", "lint"]).status, 0);
  // Started once, it isn't started again.
  rmSync(join(root, ".reylo.yaml"));
  const again = reylo(root, ["migration", "init"]);
  assert.equal(again.status, 1);
  assert.match(again.err, /schema\.sql exists already/);
});

test("reylo generate is gone, and says what fills reylo/.cache/ instead", () => {
  const gone = reylo(project(), ["generate"]);
  assert.equal(gone.status, 2);
  assert.equal(gone.err, "reylo generate was removed in 0.8.0: the type check and the build steps fill reylo/.cache/ where the database can be reached, and reylo analyze prints each statement's types.\n");
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
