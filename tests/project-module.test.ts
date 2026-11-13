import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { nativeBinary } from "../src/native.ts";
import { projectModule } from "../src/project-module.ts";

// The module an import of the project file builds to: the migrations `reylo migrate deploy` runs, in
// its order, which each object holds its history against when it starts (tests/migrate.test.ts).
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

/** The migrations a module carries, by name. */
async function names(root: string) {
  const { code } = await projectModule(join(root, ".reylo.yaml"));
  const list = JSON.parse(/migrations\((.*)\);\n$/.exec(code)![1]!) as { name: string; sql: string }[];
  return list.map((m) => m.name);
}

/** Writes schema.sql, then the migration it needs, a second later so names sort in the order written. */
async function change(root: string, name: string, schema: string, run = true) {
  await new Promise((r) => setTimeout(r, 1100));
  writeFileSync(join(root, "reylo", "schema.sql"), schema);
  migrate(root, "new", name);
  if (run) migrate(root, "dev");
}

test("an import of the project file carries the locked migrations in the lock's order, and leaves a draft out", async () => {
  const root = project();
  await change(root, "users", "create table users (id integer primary key, name text not null);\n");
  migrate(root, "lock");
  await change(root, "bio", "create table users (id integer primary key, name text not null, bio text);\n");
  const [users, bio] = readdirSync(join(root, "reylo", "migrations")).sort();
  assert.deepEqual(await names(root), [users, bio]);
  const { code, watched } = await projectModule(join(root, ".reylo.yaml"));
  assert.match(code, /^import \{ migrations \} from "reylo";\nexport const \{ migrate \} = migrations\(/);
  assert.ok(code.includes("alter table users add column bio text"));
  // Nothing of the project file itself, whose direct_url can hold a password.
  assert.ok(!code.includes("dev.sqlite"));
  assert.ok(watched.includes(join(root, "reylo", "reylo-lock.yaml")));
  // A draft, which the lock doesn't list yet, is left out, as a deploy leaves it.
  await change(root, "age", "create table users (id integer primary key, name text not null, bio text, age int);\n", false);
  assert.deepEqual(await names(root), [users, bio]);
});

test("a lock with git's conflict markers, or one that lists a file the folder lacks, fails the build with what to do", async () => {
  const root = project();
  await change(root, "users", "create table users (id integer primary key, name text not null);\n");
  migrate(root, "lock");
  const [users] = readdirSync(join(root, "reylo", "migrations"));
  const lock = join(root, "reylo", "reylo-lock.yaml");
  const text = readFileSync(lock, "utf8");
  writeFileSync(lock, text.replace(/^migrations:\n/m, "migrations:\n<<<<<<< HEAD\n  - 20261110000000_a.sql 0123abcd\n=======\n  - 20261110000000_b.sql 4567cdef\n>>>>>>> branch\n"));
  await assert.rejects(projectModule(join(root, ".reylo.yaml")), /reylo\/reylo-lock\.yaml has git's conflict markers: keep main's lines, then run reylo migrate dev/);
  writeFileSync(lock, text);
  renameSync(join(root, "reylo", "migrations", users!), join(root, "aside.sql"));
  await assert.rejects(projectModule(join(root, ".reylo.yaml")), new RegExp(`reylo/reylo-lock\\.yaml lists ${users!.replace(".", "\\.")}, which reylo/migrations doesn't have: write it back from a database that ran it, with reylo migrate history pull`));
});

test("with no lock, the migrations run in name order, init.sql first", async () => {
  const root = project();
  mkdirSync(join(root, "reylo", "migrations"));
  writeFileSync(join(root, "reylo", "schema.sql"), "create table users (id integer primary key, name text not null, bio text);\n");
  writeFileSync(join(root, "reylo", "migrations", "20261106120000_bio.sql"), "alter table users add column bio text;\n");
  writeFileSync(join(root, "reylo", "migrations", "init.sql"), "create table users (id integer primary key, name text not null);\n");
  assert.deepEqual(await names(root), ["init.sql", "20261106120000_bio.sql"]);
});

test("an import of another project file, or of a project with no migrations, is refused with what to do", async () => {
  const root = project();
  await assert.rejects(projectModule(join(root, ".reylo.yaml")), /reylo\/migrations doesn't exist: write the first with reylo migrate new <name>/);
  writeFileSync(join(root, "staging.reylo.yaml"), "direct_url: file:./staging.sqlite\n");
  await assert.rejects(projectModule(join(root, "staging.reylo.yaml")), /isn't this project's file/);
  rmSync(root, { recursive: true, force: true });
});
