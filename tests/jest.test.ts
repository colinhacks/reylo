import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { fileURLToPath } from "node:url";

// Jest and Vitest are lab/jest's dependencies, never the package's own.
const lab = fileURLToPath(new URL("../lab/jest", import.meta.url));
const root = fileURLToPath(new URL("..", import.meta.url));
const skip = !existsSync(`${lab}/node_modules/jest`) && "lab/jest is not installed";

// NO_COLOR: Vitest colours its summary even into a pipe in some environments (one with no
// variables but HOME and PATH, for one), and the assertions read the plain text.
const env = (nodeOptions = "") => ({ ...process.env, NO_COLOR: "1", NODE_OPTIONS: `--no-warnings ${nodeOptions}` });

function run(bin: string, args: string[], nodeOptions = "", cwd = lab) {
  const r = spawnSync(process.execPath, [`${lab}/node_modules/${bin}`, ...args], { cwd, encoding: "utf8", env: env(nodeOptions) });
  return { status: r.status, output: r.stdout + r.stderr };
}

const cache = mkdtempSync(join(tmpdir(), "reylo-jest-"));
const jest = (args: string[], nodeOptions?: string) => run("jest/bin/jest.js", ["--ci", `--cacheDirectory=${cache}`, ...args], nodeOptions);

test("setup", { skip }, () => {
  assert.equal(spawnSync(process.execPath, ["setup.mjs"], { cwd: lab }).status, 0);
});

test("Jest's runtime never asks Node's loader: the reylo/register preload leaves a .sql file to Jest, which reads it as JavaScript", { skip }, () => {
  const r = jest(["--config", JSON.stringify({ watchman: false, testPathIgnorePatterns: ["/node_modules/", "/probe/"], extensionsToTreatAsEsm: [".sql"] })], "--experimental-vm-modules --import reylo/register");
  assert.equal(r.status, 1, r.output);
  assert.match(r.output, /SyntaxError: Unexpected identifier 'id'/);
});

test("reylo/jest: a CommonJS run, with reylo's runtime compiled to CommonJS by babel-jest", { skip }, () => {
  const r = jest(["-c", "jest.cjs.config.cjs", "query.test.cjs"]);
  assert.equal(r.status, 0, r.output);
  assert.match(r.output, /Tests:\s+1 passed, 1 total/);
});

test("reylo/jest: an ESM run (--experimental-vm-modules), a CommonJS and an ESM test", { skip }, () => {
  const r = jest(["-c", "jest.esm.config.cjs"], "--experimental-vm-modules");
  assert.equal(r.status, 0, r.output);
  assert.match(r.output, /Tests:\s+2 passed, 2 total/);
});

test("Vitest needs nothing beyond reylo/vite", { skip }, () => {
  const r = run("vitest/vitest.mjs", ["run"]);
  assert.equal(r.status, 0, r.output);
  assert.match(r.output, /Tests\s+1 passed/);
});

// A project outside the repository on lab/jest's Vitest, with Vitest's module cache on (its default
// from 5.0.5; lab/jest's 5.0.3 has it off) and a statement a unique index changes from `many` to
// `maybeOne`. Each run of its test adds the cardinality its module carries to `seen.txt`.
function cachedProject() {
  const dir = mkdtempSync(join(tmpdir(), "reylo-vitest-"));
  mkdirSync(`${dir}/node_modules/@reylosql`, { recursive: true });
  symlinkSync(`${lab}/node_modules/vitest`, `${dir}/node_modules/vitest`);
  symlinkSync(root, `${dir}/node_modules/reylo`);
  symlinkSync(`${root}/packages/sqlite`, `${dir}/node_modules/@reylosql/sqlite`);
  writeFileSync(`${dir}/package.json`, `{ "private": true }\n`);
  writeFileSync(`${dir}/.reylo.yaml`, "direct_url: file:./app.db\n");
  writeFileSync(`${dir}/vitest.config.mjs`, `import reylo from "reylo/vite";\nexport default { plugins: [reylo()], test: { globals: true, fsModuleCache: true } };\n`);
  writeFileSync(`${dir}/by-email.sql`, "select id, name from users where email = :email\n");
  writeFileSync(`${dir}/by-email.test.mjs`, `import { appendFileSync } from "node:fs";\nimport query from "./by-email.sql";\ntest("by-email", () => appendFileSync(new URL("./seen.txt", import.meta.url), query.cardinality + "\\n"));\n`);
  const exec = (sql: string) => {
    const db = new DatabaseSync(`${dir}/app.db`);
    db.exec(sql);
    db.close();
  };
  exec("create table users (id integer primary key, name text not null, email text)");
  const seen = () => (existsSync(`${dir}/seen.txt`) ? readFileSync(`${dir}/seen.txt`, "utf8").trim().split("\n") : []);
  return { dir, exec, seen };
}

test("Vitest's module cache makes a .sql module again after a migration", { skip }, () => {
  const { dir, exec, seen } = cachedProject();
  const vitest = () => assert.equal(run("vitest/vitest.mjs", ["run"], "", dir).status, 0);
  vitest();
  exec("create unique index users_email on users(email)");
  vitest();
  vitest();
  assert.deepEqual(seen(), ["many", "maybeOne", "maybeOne"]);
});

test("Vitest's watch mode makes a .sql module again when the type check analyzes it anew", { skip }, async () => {
  const { dir, exec, seen } = cachedProject();
  // The type check (here `reylo generate`) writes the statement's entry to `.reylo/`, which the
  // module is built from; it is there before Vitest starts, so that its first run writes none.
  const generate = () => assert.equal(spawnSync(process.execPath, [`${root}/src/cli.ts`, "generate"], { cwd: dir, env: env() }).status, 0);
  generate();
  const watch = spawn(process.execPath, [`${lab}/node_modules/vitest/vitest.mjs`, "--watch"], { cwd: dir, env: env(), stdio: ["ignore", "pipe", "pipe"] });
  let output = "";
  watch.stdout.on("data", (chunk) => (output += chunk));
  watch.stderr.on("data", (chunk) => (output += chunk));
  const until = async (lines: number) => {
    for (const deadline = Date.now() + 30_000; seen().length < lines; await sleep(100)) if (Date.now() > deadline) assert.fail(`no run ${lines} of the test:\n${output}`);
  };
  try {
    await until(1);
    exec("create unique index users_email on users(email)");
    generate();
    await until(2);
    assert.deepEqual(seen(), ["many", "maybeOne"]);
  } finally {
    watch.kill();
  }
});
