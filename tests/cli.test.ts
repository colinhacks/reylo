import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, renameSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";

const CLI = join(import.meta.dirname, "..", "src", "cli.ts");
const REGISTER = join(import.meta.dirname, "..", "src", "register.ts");

function reylo(cwd: string, ...args: string[]) {
  const r = spawnSync(process.execPath, [CLI, ...args], { cwd, encoding: "utf8", env: { PATH: process.env.PATH, DATABASE_URL: "file:./app.db" } });
  // The time varies from run to run; everything else is the readout.
  return { status: r.status, out: r.stdout.replace(/\(\d+\.\d s\)/, "(_ s)"), err: r.stderr };
}

test("reylo generate reads a .env database URL with an inline comment", (t) => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-cli-env-")));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), 'DATABASE_URL="file:./app.db" # local development\n');
  writeFileSync(join(root, "query.sql"), "select id from users");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key)");
  db.close();

  const result = spawnSync(process.execPath, [CLI, "generate"], { cwd: root, encoding: "utf8", env: { PATH: process.env.PATH } });

  assert.equal(result.status, 0, result.stderr || result.stdout);
});

test("reylo generate analyzes every .sql file of the project, and prints each file with a mark and every error in full", () => {
  // The CLI sees its working directory as the real path (/private/var on macOS).
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-cli-")));
  writeFileSync(join(root, "package.json"), "{}");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null)");
  db.close();
  for (const dir of ["src/queries", "migrations", "node_modules/dep", ".next", "generated", "app/.server/queries"]) mkdirSync(join(root, dir), { recursive: true });
  writeFileSync(join(root, "src", "queries", "user.sql"), "select id, name from users where id = :id");
  writeFileSync(join(root, "src", "queries", "count.sql"), "select count(*) as n from users");
  writeFileSync(join(root, "src", "queries", "bad.sql"), "select id\nfrom nope");
  writeFileSync(join(root, "src", "queries", "typo.sql"), "selec id from users");
  // A file of one command is typed like a query, and one the database refuses is an error like any
  // other: the database cannot tell a migration from a command someone imports.
  writeFileSync(join(root, "src", "queries", "index.sql"), "create index users_name on users (name)");
  writeFileSync(join(root, "src", "queries", "stale.sql"), "create index users_email on users (email)");
  // React Router's server-only directory is the one dot directory searched.
  writeFileSync(join(root, "app", ".server", "queries", "names.sql"), "select name from users");
  // A migration of several statements is skipped: it is not one query.
  writeFileSync(join(root, "migrations", "0001.sql"), "create table posts (id integer primary key);\ncreate index posts_id on posts (id);");
  writeFileSync(join(root, "migrations", "0002.sql"), "alter table posts add column title text;\nalter table posts add column body text;");
  // Dependencies, build output and what git ignores are not the project's queries.
  writeFileSync(join(root, "node_modules", "dep", "x.sql"), "select 1");
  writeFileSync(join(root, ".next", "y.sql"), "select 1");
  writeFileSync(join(root, "generated", "z.sql"), "select 1");
  writeFileSync(join(root, ".gitignore"), "generated/\n");
  assert.equal(spawnSync("git", ["init", "-q"], { cwd: root }).status, 0);

  const first = reylo(root, "generate");
  assert.equal(first.status, 1, first.err);
  // Loading the engine prints no ExperimentalWarning for node:wasi (src/engine.ts).
  assert.equal(first.err, "");
  assert.equal(first.out, [
    `sqlite at ${join(root, "app.db")} (DATABASE_URL in the environment)`,
    `  app/.server/queries/names.sql  ✓`,
    `  src/queries/bad.sql            ✗`,
    `  src/queries/count.sql          ✓`,
    `  src/queries/index.sql          ✓`,
    `  src/queries/stale.sql          ✗`,
    `  src/queries/typo.sql           ✗`,
    `  src/queries/user.sql           ✓`,
    ``,
    // SQLite names no place for this error, so no line of the statement is shown.
    `src/queries/bad.sql:1:1: no such table: nope`,
    ``,
    // A command the database refuses fails, and its message says how to leave a migration out.
    `src/queries/stale.sql:1:36: no such column: email, so if it is a migration or a schema file, leave it out of the type check with "exclude" in tsconfig.json, as in "exclude": ["db/**"].`,
    `  1 | create index users_email on users (email)`,
    `    |                                    ^^^^^`,
    ``,
    // A misspelt keyword is an error, not a statement that is not a query.
    `src/queries/typo.sql:1:1: near "selec": syntax error`,
    `  1 | selec id from users`,
    `    | ^^^^^`,
    ``,
    `Skipped 2 files that are not one query (a migration?): migrations/0001.sql, migrations/0002.sql`,
    ``,
    `✗ 3 of 7 queries failed (_ s)`,
    ``,
  ].join("\n"));

  // A run that finds no query fails, so that a CI job does not carry an empty .reylo/ to a build.
  const none = reylo(root, "generate", "migrations");
  assert.equal(none.status, 1, none.err);
  assert.match(none.out, /^Skipped 2 files that are not one query \(a migration\?\): .*\n\nNo query under migrations: name the directory that holds them, or set cli\.include in \.reylo\.yaml\.\n$/m);

  // .reylo.yaml narrows the files; a file named on the command line is analyzed as named.
  writeFileSync(join(root, ".reylo.yaml"), `cli:\n  include: ["src/**/*.sql"]\n  exclude: ["**/bad.sql", "**/stale.sql", "**/typo.sql"]\n`);
  const configured = reylo(root, "generate");
  assert.equal(configured.status, 0, configured.err);
  assert.match(configured.out, /^  src\/queries\/count\.sql  ✓\n  src\/queries\/index\.sql  ✓\n  src\/queries\/user\.sql   ✓\n\n✓ 3 queries typed \(_ s\)\n$/m);
  assert.equal(reylo(root, "generate", "src/queries/bad.sql").status, 1);

  // Without the database the cache still answers, and --force cannot.
  renameSync(join(root, "app.db"), join(root, "gone.db"));
  const offline = reylo(root, "generate", "src/queries/user.sql");
  assert.equal(offline.status, 0, offline.err);
  assert.match(offline.out, /^Cannot open the database .*app\.db \(DATABASE_URL in the environment\): .*; the files are read from \.reylo\/ as they are\.$/m);
  assert.match(offline.out, /^  src\/queries\/user\.sql  ✓$/m);
  const forced = reylo(root, "generate", "--force", "src/queries/user.sql");
  assert.equal(forced.status, 1);
  assert.match(forced.out, /^src\/queries\/user\.sql: --force needs the database, which cannot be reached$/m);

  assert.equal(reylo(root, "generate", "--watch").status, 2);
  assert.equal(reylo(root, "generate", "package.json").status, 2);
  assert.equal(reylo(root, "frobnicate").status, 2);
});

test("reylo generate's hint for a run that finds no query names a .reylo.yml project's own file", () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-cli-yml-")));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".reylo.yml"), "strict_params: true\n");
  // No .sql file opens a project here, so the hint reads the project file of the directory searched.
  const none = reylo(root, "generate");
  assert.equal(none.status, 1, none.err);
  assert.equal(none.out, "No query under .: name the directory that holds them, or set cli.include in .reylo.yml.\n");
});

test("reylo generate's globs: a directory named like a .sql file is searched, an include may name a directory, and one starting with ! excludes", () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-cli-globs-")));
  writeFileSync(join(root, "package.json"), "{}");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null)");
  db.close();
  for (const dir of ["src/queries/x.sql", "src/legacy"]) mkdirSync(join(root, dir), { recursive: true });
  writeFileSync(join(root, "src", "queries", "x.sql", "ids.sql"), "select id from users");
  writeFileSync(join(root, "src", "queries", "names.sql"), "select name from users");
  writeFileSync(join(root, "src", "legacy", "old.sql"), "select 1 as one");
  const files = (out: string) => [...out.matchAll(/^  (\S+)\s+✓$/gm)].map((m) => m[1]);

  const all = reylo(root, "generate");
  assert.equal(all.status, 0, all.err);
  assert.deepEqual(files(all.out), ["src/legacy/old.sql", "src/queries/names.sql", "src/queries/x.sql/ids.sql"]);

  writeFileSync(join(root, ".reylo.yaml"), `cli:\n  include: ["src", "!src/legacy/**"]\n`);
  const configured = reylo(root, "generate");
  assert.equal(configured.status, 0, configured.err);
  assert.deepEqual(files(configured.out), ["src/queries/names.sql", "src/queries/x.sql/ids.sql"]);
});

test("reylo generate says once that the type check does not read url, and names the connection it used", () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-cli-url-")));
  writeFileSync(join(root, "package.json"), "{}");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null)");
  db.close();
  writeFileSync(join(root, "a.sql"), "select id from users");
  writeFileSync(join(root, "b.sql"), "select name from users");
  // Reylo does not read url: the run reads DATABASE_URL, and says so once, not per file.
  writeFileSync(join(root, ".reylo.yaml"), "url: postgres://prod-pooler/app\n");
  const alone = reylo(root, "generate");
  assert.equal(alone.status, 0, alone.err);
  assert.equal(alone.out, [
    `sqlite at ${join(root, "app.db")} (DATABASE_URL in the environment)`,
    `.reylo.yaml: Reylo does not read url; set direct_url to the dev database Reylo reads your types from (it uses DATABASE_URL meanwhile)`,
    `  a.sql  ✓`,
    `  b.sql  ✓`,
    ``,
    `✓ 2 queries typed (_ s)`,
    ``,
  ].join("\n"));
  // Without DATABASE_URL there is no connection, and the one error says why url is not one.
  const none = spawnSync(process.execPath, [CLI, "generate", "--force"], { cwd: root, encoding: "utf8", env: { PATH: process.env.PATH } });
  assert.equal(none.status, 1, none.stderr);
  assert.match(none.stdout, /^No database URL: \.reylo\.yaml sets url, which Reylo does not read; set direct_url there, or DATABASE_URL in the environment, \.env\.local or \.env\.$/m);
  assert.doesNotMatch(none.stdout, /it uses DATABASE_URL meanwhile/);
  // direct_url beside url names Reylo's connection: the run reads direct_url and says nothing more.
  writeFileSync(join(root, ".reylo.yaml"), "url: postgres://prod-pooler/app\ndirect_url: file:./app.db\n");
  const both = reylo(root, "generate");
  assert.equal(both.status, 0, both.err);
  assert.match(both.out, /^sqlite at .*app\.db \("direct_url" in \.reylo\.yaml\)\n  a\.sql  ✓\n/);
  assert.doesNotMatch(both.out, /does not read url/);
});

test("a program run under the reylo/register preload prints nothing on stderr: node:wasi's ExperimentalWarning is swallowed", () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-register-")));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  // The module a `.sql` file becomes imports reylo, as a lab reaches it: through a link to this repository.
  mkdirSync(join(root, "node_modules"));
  symlinkSync(join(import.meta.dirname, ".."), join(root, "node_modules", "reylo"));
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null); insert into users (name) values ('ada')");
  db.close();
  writeFileSync(join(root, "user.sql"), "select id, name from users where id = :id");
  writeFileSync(join(root, "main.ts"), `import { DatabaseSync } from "node:sqlite";\nimport user from "./user.sql";\nconsole.log(JSON.stringify(await user.run(new DatabaseSync("app.db"), { id: 1 })));\n`);
  // The engine loads in the preload's worker thread, which reports its warnings through the main thread's stderr.
  const r = spawnSync(process.execPath, ["--import", REGISTER, "main.ts"], { cwd: root, encoding: "utf8", env: { PATH: process.env.PATH, DATABASE_URL: "file:./app.db" } });
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, `{"id":1,"name":"ada"}\n`);
  assert.doesNotMatch(r.stderr, /ExperimentalWarning/);
  assert.equal(r.stderr, "");
});
