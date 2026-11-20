import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { nativeBinary } from "../src/native.ts";
import { embeddedModule, lockChecksum, projectMigrations } from "../src/embedded-module.ts";

// What Reylo's bundler plugins put in place of reylo's placeholder module: the migrations `reylo
// migrate deploy` runs, in its order, which each Durable Object holds its history against when it
// starts (tests/migrate.test.ts).
// The project is made with reylo migrate itself, on SQLite, so no server is needed.

/** `reylo migrate <args>` in `root`, outside CI, where `new` and `dev` refuse to run. */
function migrate(root: string, ...args: string[]) {
  const env = { ...process.env };
  delete env.CI;
  const r = spawnSync(nativeBinary(), ["migrate", ...args], { cwd: root, encoding: "utf8", env });
  assert.equal(r.status, 0, r.stdout + r.stderr);
}

function project() {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-project-module-")));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".reylo.yaml"), "direct_url: file:./dev.sqlite\n");
  mkdirSync(join(root, "reylo"));
  return root;
}

/** The migrations the bundle carries, by name. */
async function names(root: string) {
  const { list, error } = await projectMigrations(root);
  assert.equal(error, undefined);
  return list!.map((m) => m.name);
}

/** Writes schema.sql, then the migration it needs, a second later so names sort in the order written. */
async function change(root: string, name: string, schema: string, run = true) {
  await new Promise((r) => setTimeout(r, 1100));
  writeFileSync(join(root, "reylo", "schema.sql"), schema);
  migrate(root, "new", name);
  if (run) migrate(root, "dev");
}

test("the bundle carries the locked migrations in the lock's order, and a draft gives migrate the deploy's refusal", async () => {
  const root = project();
  await change(root, "users", "create table users (id integer primary key, name text not null);\n");
  migrate(root, "lock");
  await change(root, "bio", "create table users (id integer primary key, name text not null, bio text);\n");
  const [users, bio] = readdirSync(join(root, "reylo", "migrations")).sort();
  assert.deepEqual(await names(root), [users, bio]);
  const { code, watched } = await embeddedModule(root);
  assert.match(code, /^export const embedded = \[\{"name":/);
  assert.ok(code.includes("alter table users add column bio text"));
  // Nothing of the project file itself, whose direct_url can hold a password.
  assert.ok(!code.includes("dev.sqlite"));
  assert.ok(watched.includes(join(root, "reylo", "reylo.lock")));
  // A draft, which the lock doesn't list yet, is refused, as a deploy refuses it.
  await change(root, "age", "create table users (id integer primary key, name text not null, bio text, age int);\n", false);
  const [, , age] = readdirSync(join(root, "reylo", "migrations")).sort();
  assert.equal((await projectMigrations(root)).error, `The lock doesn't list reylo/migrations/${age} yet: run it in development, which locks it, with reylo migrate dev`);
  migrate(root, "dev");
  assert.deepEqual(await names(root), [users, bio, age]);
});

test("a file that isn't a migration, or a schema.sql the lock's checksum isn't of, gives migrate the deploy's refusal", async () => {
  const root = project();
  await change(root, "users", "create table users (id integer primary key, name text not null);\n");
  migrate(root, "lock");
  const [users] = readdirSync(join(root, "reylo", "migrations"));
  const schema = join(root, "reylo", "schema.sql");
  const text = readFileSync(schema, "utf8");
  // A comment or spacing is no change, as the lock reads schema.sql.
  writeFileSync(schema, `-- the users\n${text.replace(/ /g, "  ")}`);
  assert.deepEqual(await names(root), [users]);
  writeFileSync(schema, text.replace("name text not null", "name text not null, bio text"));
  assert.match((await projectMigrations(root)).error ?? "", /^Changes in reylo\/schema\.sql since the lock was written: if the schema changed, write its migration and run it/);
  writeFileSync(schema, text);
  writeFileSync(join(root, "reylo", "migrations", "notes.sql"), "select 1;\n");
  const stray = "Not a migration: notes.sql, in reylo/migrations, where a file is init.sql or <YYYYMMDDHHMMSS>_<name>.sql, the name in a-z, 0-9 and _";
  assert.equal((await projectMigrations(root)).error, stray);
  // With no lock, a stray is refused too, never run as a migration.
  rmSync(join(root, "reylo", "reylo.lock"));
  assert.equal((await projectMigrations(root)).error, stray);
  rmSync(join(root, "reylo", "migrations", "notes.sql"));
  assert.deepEqual(await names(root), [users]);
});

test("a lock with git's conflict markers, or one that lists a file the folder lacks, gives migrate the error to throw", async () => {
  const root = project();
  await change(root, "users", "create table users (id integer primary key, name text not null);\n");
  migrate(root, "lock");
  const [users] = readdirSync(join(root, "reylo", "migrations"));
  const lock = join(root, "reylo", "reylo.lock");
  const text = readFileSync(lock, "utf8");
  writeFileSync(lock, text.replace(/^migrations:\n/m, "migrations:\n<<<<<<< HEAD\n  - 20261110000000_a.sql 0123abcd\n=======\n  - 20261110000000_b.sql 4567cdef\n>>>>>>> branch\n"));
  assert.match((await projectMigrations(root)).error ?? "", /^Merge conflict markers in reylo\/reylo\.lock: keep main's lines, then run reylo migrate dev/);
  assert.match((await embeddedModule(root)).code, /^export const embedded = \{"error":"Merge conflict markers in reylo\/reylo\.lock/);
  writeFileSync(lock, text);
  renameSync(join(root, "reylo", "migrations", users!), join(root, "aside.sql"));
  assert.match((await projectMigrations(root)).error ?? "", new RegExp(`^The lock lists ${users!.replace(".", "\\.")}, which reylo/migrations doesn't have: write it back from a database that ran it, with reylo migrate history pull`));
});

test("a migration edited after it was locked, or lines merged out of order, give migrate the deploy's refusal", async () => {
  const root = project();
  await change(root, "users", "create table users (id integer primary key, name text not null);\n");
  await change(root, "bio", "create table users (id integer primary key, name text not null, bio text);\n");
  migrate(root, "lock");
  const [users, bio] = readdirSync(join(root, "reylo", "migrations")).sort();
  const file = join(root, "reylo", "migrations", bio!);
  const text = readFileSync(file, "utf8");
  // A comment, spacing or CRLF line endings are no edit, as the lock reads them.
  writeFileSync(file, `-- the profile's text\r\n${text.replace(/\n/g, "\r\n").replace(/ /g, "  ")}\r\n`);
  assert.deepEqual(await names(root), [users, bio]);
  writeFileSync(file, text.replace("bio text", "bio text not null default ''"));
  assert.match((await projectMigrations(root)).error ?? "", new RegExp(`^Migration reylo/migrations/${bio!.replace(".", "\\.")} changed after it was locked: a database that ran the old text refuses it`));
  writeFileSync(file, text);
  // Two branches that each locked a migration after users, merged as "accept both": the second
  // line is chained after users, not after the line before it.
  const lock = join(root, "reylo", "reylo.lock");
  const lines = readFileSync(lock, "utf8");
  const [first] = lines.match(/^  - .*$/gm)!;
  const age = "20990101000000_age.sql";
  writeFileSync(join(root, "reylo", "migrations", age), "alter table users add column age int;\n");
  const sum = first!.split(" ").pop()!;
  writeFileSync(lock, lines.replace(/^  - .*\n(?![\s\S]*^  - )/m, (bioLine) => `  - ${age} ${lockChecksum(sum, age, "alter table users add column age int;")}\n${bioLine}`));
  assert.match((await projectMigrations(root)).error ?? "", new RegExp(`^The lock was merged out of order at ${bio!.replace(".", "\\.")}`));
});

test("with no lock, the migrations run in name order, init.sql first", async () => {
  const root = project();
  mkdirSync(join(root, "reylo", "migrations"));
  writeFileSync(join(root, "reylo", "schema.sql"), "create table users (id integer primary key, name text not null, bio text);\n");
  writeFileSync(join(root, "reylo", "migrations", "20261106120000_bio.sql"), "alter table users add column bio text;\n");
  writeFileSync(join(root, "reylo", "migrations", "init.sql"), "create table users (id integer primary key, name text not null);\n");
  assert.deepEqual(await names(root), ["init.sql", "20261106120000_bio.sql"]);
});

test("the lock under its old name, alone or beside the lock, gives migrate the error to throw, never the migrations in name order", async () => {
  const root = project();
  await change(root, "users", "create table users (id integer primary key, name text not null);\n");
  migrate(root, "lock");
  const lock = join(root, "reylo", "reylo.lock");
  const old = join(root, "reylo", "reylo-lock.yaml");
  const text = readFileSync(lock, "utf8");
  renameSync(lock, old);
  assert.equal((await projectMigrations(root)).error, "The lock's old name is reylo/reylo-lock.yaml: rename it to reylo/reylo.lock, with git mv reylo/reylo-lock.yaml reylo/reylo.lock");
  assert.match((await embeddedModule(root)).code, /^export const embedded = \{"error":"The lock's old name is reylo\/reylo-lock\.yaml/);
  assert.ok((await embeddedModule(root)).watched.includes(old));
  writeFileSync(lock, text);
  assert.match((await projectMigrations(root)).error ?? "", /^Both reylo\/reylo-lock\.yaml and reylo\/reylo\.lock are here: .* with git rm reylo\/reylo-lock\.yaml$/);
  rmSync(old);
  assert.equal((await projectMigrations(root)).error, undefined);
});

test("a project with no migrations gives the bundle none, and builds as before", async () => {
  const root = project();
  assert.equal((await embeddedModule(root)).code, "export const embedded = undefined;\n");
  rmSync(root, { recursive: true, force: true });
});
