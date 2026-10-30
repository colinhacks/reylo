import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { engine } from "../src/engine.ts";
import { findProjectFile, resolveProject } from "../src/project.ts";
import { Session } from "../src/analyze.ts";
import { ROOT } from "./helpers.ts";

// Any engine reads the project file; the repository's own install has every one.

// A project tree: `files` maps a path under a fresh directory to its text.
function tree(files: Record<string, string>): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-project-"));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(join(root, path, ".."), { recursive: true });
    writeFileSync(join(root, path), text);
  }
  return root;
}

// A project file's command runs in the system's shell, cmd.exe on Windows, and these tests write theirs for a POSIX shell.
const posixShell = process.platform === "win32" && "its command is written for a POSIX shell, and Windows runs it in cmd.exe";

test("without a project file, DATABASE_URL comes from the environment, then .env.local, then .env", async () => {
  const e = await engine("postgres", ROOT);
  const root = tree({ "app/package.json": "{}", "app/.env": "DATABASE_URL=postgres://from-env-file/db\n", "app/.env.local": "DATABASE_URL=postgres://from-local/db\n" });
  const dir = join(root, "app");
  assert.equal(findProjectFile(dir, {}), undefined);
  const local = resolveProject(e, dir, {});
  assert.deepEqual([local.url, local.urlSource, local.dialect, local.error], ["postgres://from-local/db", "DATABASE_URL in .env.local", "postgres", undefined]);
  const set = resolveProject(e, dir, { DATABASE_URL: "file:./x.db" });
  assert.deepEqual([set.url, set.urlSource, set.dialect], ["file:./x.db", "DATABASE_URL in the environment", "sqlite"]);
  // A parameter nothing types is an error unless the project file says otherwise (since 2026-10-20).
  assert.equal(local.strictParams, true);
  const loose = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: file:./x.db\nstrict_params: false\n" });
  assert.equal(resolveProject(e, loose, {}).strictParams, false);
  assert.equal(resolveProject(e, tree({ "package.json": "{}", ".reylo.yaml": "direct_url: file:./x.db\n" }), {}).strictParams, true);
  // The sqlcommenter comment is on unless the project file turns it off.
  assert.equal(local.sqlcommenter, true);
  assert.equal(resolveProject(e, tree({ "package.json": "{}", ".reylo.yaml": "direct_url: file:./x.db\nsqlcommenter: false\n" }), {}).sqlcommenter, false);
});

test("the project file sits beside package.json, and its direct_url, env files and strict_params apply", async () => {
  const e = await engine("postgres", ROOT);
  const root = tree({
    "package.json": "{}",
    ".reylo.yaml": "url: postgres://pooler/app\ndirect_url: mysql://root:${DB_PASSWORD}@127.0.0.1:3306/app\nenv: [secrets.env]\nstrict_params: true\napi:\n  routes: api\n",
    "secrets.env": "DB_PASSWORD=hunter2\n",
    "src/db/tsconfig.json": "{}",
  });
  // The tsconfig sits below the package; the project file is the package's.
  const r = resolveProject(e, join(root, "src/db"), {});
  assert.equal(r.file, join(root, ".reylo.yaml"));
  assert.equal(r.root, root);
  // The type check takes the direct connection, and the dialect is its scheme's.
  assert.equal(r.url, "mysql://root:hunter2@127.0.0.1:3306/app");
  assert.equal(r.urlSource, '"direct_url" in ../../.reylo.yaml, DB_PASSWORD in ../../secrets.env');
  assert.equal(r.dialect, "mysql");
  assert.equal(r.strictParams, true);
  assert.equal(r.error, undefined);
  // A file that sets both has named Reylo's connection: nothing to say.
  assert.equal(r.notice, undefined);
  assert.ok(r.watched.includes(join(root, "secrets.env")));
  // A package of its own below does not see the file above it.
  const nested = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: file:./a.db\n", "packages/web/package.json": "{}" });
  assert.equal(findProjectFile(join(nested, "packages/web"), {}), undefined);
  // REYLO_CONFIG names the file instead.
  const other = tree({ "elsewhere.yaml": "direct_url: file:./dev.db\n" });
  assert.equal(resolveProject(e, join(root, "src/db"), { REYLO_CONFIG: join(other, "elsewhere.yaml") }).dialect, "sqlite");
});

test("the project file may be named .reylo.yml, and both names beside one package.json are an error", async () => {
  const e = await engine("postgres", ROOT);
  const yml = tree({ "package.json": "{}", ".reylo.yml": "direct_url: file:./x.db\n" });
  const r = resolveProject(e, yml, {});
  assert.deepEqual([r.file, r.dialect, r.error], [join(yml, ".reylo.yml"), "sqlite", undefined]);
  // Creating the other name would make the project an error, so the type check watches it.
  assert.ok(r.watched.includes(join(yml, ".reylo.yaml")));
  const none = resolveProject(e, tree({ "package.json": "{}" }), {});
  assert.equal(none.file, undefined);
  assert.equal(none.watched.filter((path) => /\.reylo\.ya?ml$/.test(path)).length, 2);
  const both = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: file:./a.db\n", ".reylo.yml": "direct_url: file:./b.db\n" });
  assert.equal(resolveProject(e, both, {}).error, ".reylo.yaml and .reylo.yml are both project files; keep one.");
  // REYLO_CONFIG names the one file, whatever sits beside it.
  assert.equal(resolveProject(e, both, { REYLO_CONFIG: join(both, ".reylo.yml") }).error, undefined);
});

test("a types section, removed on 2026-10-20, is refused where it stands, whatever it holds; package.json decides no type", async () => {
  const e = await engine("postgres", ROOT);
  for (const types of ["types:\n  dates: temporal\n", "types: {}\n", "types:\n  datez: Date\n"]) {
    const root = tree({ "package.json": JSON.stringify({ dependencies: { mariadb: "^3", "@planetscale/database": "^1" } }), ".reylo.yaml": `direct_url: mysql://root@127.0.0.1:3306/app\n\n${types}` });
    const r = resolveProject(e, root, {});
    assert.match(r.error!, /^\.reylo\.yaml:3:1: types: the types section was removed on 2026-10-20\. Reylo now gives every value one type on every driver, whatever the driver's settings: .* Delete the section\.$/, types);
    assert.equal("repr" in r || "driver" in r, false);
    assert.ok(!r.watched.includes(join(root, "package.json")));
  }
  // A key that merely starts with `types` is the engine's to refuse, and a nested `types:` is another section's.
  const nested = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: postgres://x/db\napi:\n  types: whatever\n" });
  assert.equal(resolveProject(e, nested, {}).error, undefined);
});

test("a key the library does not know, and a variable nothing sets, are errors that say where", async () => {
  const e = await engine("postgres", ROOT);
  const typo = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: postgres://x/db\n\nstirct: true\n" });
  assert.match(resolveProject(e, typo, {}).error!, /^\.reylo\.yaml:3:1: unknown field `stirct`/);
  // `strict` was renamed `strict_params` on 2026-10-20, with no alias: the old key is unknown, and its error names the new one.
  const renamed = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: postgres://x/db\nstrict: true\n" });
  assert.equal(resolveProject(e, renamed, {}).error, ".reylo.yaml:2:1: unknown field `strict`: the key is `strict_params` since 2026-10-20");
  const unset = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: ${NOPE}\n" });
  const r = resolveProject(e, unset, {});
  assert.equal(r.url, undefined);
  assert.match(r.error!, /^\.reylo\.yaml: "direct_url" names \$\{NOPE\}, which neither the environment nor/);
  // A scheme reylo does not read names no dialect.
  const odd = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: ./dev.db\n" });
  assert.match(resolveProject(e, odd, {}).error!, /^Cannot tell the database from the URL \("direct_url" in \.reylo\.yaml\)/);
});

test("a $(command) in direct_url is replaced by what it prints, once per process, percent-encoded in the user or password", { skip: posixShell }, async () => {
  const e = await engine("postgres", ROOT);
  // The command counts its runs in a file beside it, and prints a password a URL cannot hold as it is.
  const root = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: postgres://app:$(echo run >> runs.txt; printf 'p@ss/w?rd&x')@db.internal:5432/app\n" });
  const runs = () => readFileSync(join(root, "runs.txt"), "utf8").split("\n").filter(Boolean).length;
  const r = resolveProject(e, root, {});
  assert.equal(r.error, undefined);
  assert.equal(r.url, "postgres://app:p%40ss%2Fw%3Frd%26x@db.internal:5432/app");
  assert.equal(decodeURIComponent(new URL(r.url!).password), "p@ss/w?rd&x");
  // Where the URL came from, never what the command printed.
  assert.equal(r.urlSource, '"direct_url" in .reylo.yaml');
  assert.equal(r.dialect, "postgres");
  // Resolved again, as every build step and the type check do, the command does not run again.
  assert.equal(resolveProject(e, root, {}).url, r.url);
  assert.equal(runs(), 1);
  // A refused login runs it again.
  assert.equal(r.refreshUrl!(), r.url);
  assert.equal(runs(), 2);
  // A URL with no command has nothing to refresh.
  assert.equal(resolveProject(e, tree({ "package.json": "{}", ".reylo.yaml": "direct_url: postgres://x/db\n" }), {}).refreshUrl, undefined);
  // A command that is the whole value prints a URL, taken as printed.
  const whole = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: $(printf 'mysql://root:p@ss@127.0.0.1:3306/app')\n" });
  assert.deepEqual([resolveProject(e, whole, {}).url, resolveProject(e, whole, {}).dialect], ["mysql://root:p@ss@127.0.0.1:3306/app", "mysql"]);
});

test("a command's variables are its shell's, from the environment and the env files, and no value is read as shell syntax", { skip: posixShell }, async () => {
  const e = await engine("postgres", ROOT);
  const root = tree({
    "package.json": "{}",
    ".reylo.yaml": "direct_url: postgres://app:$(printf %s \"$DB_PASSWORD\")@$(printf %s \"$DB_HOST\")/app\n",
    ".env": "DB_PASSWORD=$(touch pwned)\nDB_HOST=from-env-file\n",
  });
  const r = resolveProject(e, root, { DB_HOST: "from-environment" });
  assert.equal(r.url, `postgres://app:${encodeURIComponent("$(touch pwned)")}@from-environment/app`);
  assert.equal(existsSync(join(root, "pwned")), false);
  // Reylo's own ${VAR}, outside a command, puts its value in as it is, never as a command.
  const text = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: postgres://app:${DB_PASSWORD}@h/app\n", ".env": "DB_PASSWORD=$(touch pwned)\n" });
  assert.equal(resolveProject(e, text, {}).url, "postgres://app:$(touch pwned)@h/app");
  assert.equal(existsSync(join(text, "pwned")), false);
});

test("a command that fails, prints nothing or is left open is an error that names it, and runs nothing more", { skip: posixShell }, async () => {
  const e = await engine("postgres", ROOT);
  const failing = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: postgres://app:$(echo 'no item matches' >&2; exit 3)@h/app\n" });
  const r = resolveProject(e, failing, {});
  assert.equal(r.url, undefined);
  assert.equal(r.error, `.reylo.yaml: "direct_url" runs \`echo 'no item matches' >&2; exit 3\`, which exited with code 3: no item matches.`);
  const empty = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: postgres://app:$(true)@h/app\n" });
  assert.equal(resolveProject(e, empty, {}).error, '.reylo.yaml: "direct_url" runs `true`, which printed nothing.');
  const open = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: postgres://app:$(printf x@h/app\n" });
  assert.equal(resolveProject(e, open, {}).error, '.reylo.yaml: "direct_url" opens $( at column 16 and never closes it.');
  // A variable nothing sets is said first, and the command, which may ask for a password, never runs.
  const unset = tree({ "package.json": "{}", ".reylo.yaml": "direct_url: postgres://${NOPE}:$(touch ran; printf x)@h/app\n" });
  assert.match(resolveProject(e, unset, {}).error!, /names \$\{NOPE\}/);
  assert.equal(existsSync(join(unset, "ran")), false);
});

// The type check never reads url, and a file that sets it alone is told to set direct_url.
const NOTICE = ".reylo.yaml: Reylo does not read url; set direct_url to the dev database Reylo reads your types from (it uses DATABASE_URL meanwhile)";

test("url is never the type check's connection: alone, it draws a notice and DATABASE_URL is used; direct_url wins; both draw nothing", async () => {
  const e = await engine("postgres", ROOT);
  // url alone: DATABASE_URL, from the environment or an env file, and one notice that names direct_url.
  const alone = tree({ "package.json": "{}", ".reylo.yaml": "url: postgres://prod-pooler/app\n", ".env": "DATABASE_URL=postgres://dev/app\n" });
  const fromFile = resolveProject(e, alone, {});
  assert.deepEqual([fromFile.url, fromFile.urlSource, fromFile.error, fromFile.notice], ["postgres://dev/app", "DATABASE_URL in .env", undefined, NOTICE]);
  const fromEnv = resolveProject(e, alone, { DATABASE_URL: "file:./dev.db" });
  assert.deepEqual([fromEnv.url, fromEnv.urlSource, fromEnv.dialect, fromEnv.notice], ["file:./dev.db", "DATABASE_URL in the environment", "sqlite", NOTICE]);
  // Nothing in url is resolved or checked: an unset variable or an odd scheme there is the framework's to report.
  const unread = tree({ "package.json": "{}", ".reylo.yaml": "url: ${NOPE}/x\n" });
  assert.deepEqual([resolveProject(e, unread, { DATABASE_URL: "postgres://dev/app" }).url, resolveProject(e, unread, { DATABASE_URL: "postgres://dev/app" }).error], ["postgres://dev/app", undefined]);
  // With no DATABASE_URL either, there is no connection, and the error says why url is not one.
  const none = resolveProject(e, tree({ "package.json": "{}", ".reylo.yaml": "url: postgres://prod-pooler/app\n" }), {});
  assert.equal(none.url, undefined);
  assert.equal(none.error, undefined);
  assert.equal(none.missingUrl, "No database URL: .reylo.yaml sets url, which Reylo does not read; set direct_url there, or DATABASE_URL in the environment, .env.local or .env.");
  // direct_url wins over DATABASE_URL, and with url beside it nothing is said.
  const both = resolveProject(e, tree({ "package.json": "{}", ".reylo.yaml": "url: postgres://prod-pooler/app\ndirect_url: postgres://dev/app\n" }), { DATABASE_URL: "postgres://elsewhere/app" });
  assert.deepEqual([both.url, both.urlSource, both.notice], ["postgres://dev/app", '"direct_url" in .reylo.yaml', undefined]);
  // Without a project file, or without url in it, nothing is said either.
  assert.equal(resolveProject(e, tree({ "package.json": "{}" }), {}).notice, undefined);
  assert.equal(resolveProject(e, tree({ "package.json": "{}", ".reylo.yaml": "strict_params: false\n" }), {}).notice, undefined);
  assert.equal(resolveProject(e, tree({ "package.json": "{}" }), {}).missingUrl, 'No database URL: set DATABASE_URL in the environment, .env.local or .env, or "direct_url" in .reylo.yaml.');
});

test(".reylo/ names an entry by its statement, then its server: never by url or a password", async () => {
  const entry = async (yaml: string, env: Record<string, string>) => {
    const session = await Session.open(tree({ "package.json": "{}", ".reylo.yaml": yaml }), env);
    await session.close();
    return basename(session.entryFile("select 1")).replace(/\.json$/, "").split("-");
  };
  const [statement, dev] = await entry("strict_params: true\n", { DATABASE_URL: "postgres://dev/app" });
  // url alone names it like no url at all, and like a direct_url that names the same database;
  // the password is not part of it.
  assert.deepEqual(await entry("url: postgres://prod-pooler/app\n", { DATABASE_URL: "postgres://dev/app" }), [statement, dev]);
  assert.deepEqual(await entry("direct_url: postgres://:secret@dev/app\n", {}), [statement, dev]);
  // Another database of the dialect: the same statement, another server.
  const [same, other] = await entry("strict_params: true\n", { DATABASE_URL: "postgres://ci@127.0.0.1:5432/test" });
  assert.equal(same, statement);
  assert.notEqual(other, dev);
  // With no URL and every engine installed, as reylo's own install has them, the dialect stays
  // Postgres: the same statement, and no server.
  assert.equal((await entry("strict_params: true\n", {}))[0], statement);
  // Another dialect is another statement.
  assert.notEqual((await entry("strict_params: true\n", { DATABASE_URL: "mysql://root@127.0.0.1:3306/app" }))[0], statement);
});
