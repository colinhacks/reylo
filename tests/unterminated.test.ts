import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { withinFile } from "../src/module-types.ts";
import { DRIVERS, PG, pgLive, ROOT } from "./helpers.ts";

// A statement that ends inside a string, a quoted name or a comment is an error at what it leaves
// open, and its module is still typed. The error once sat at the end of the text, one byte past
// the end of the file, and TypeScript refused the whole transform ("The content mapper returned an
// invalid transform response"), which made the import `any`. The type check runs as a project runs
// it, `tsc` with runExternalCode: over SQLite, and over Postgres where REYLO_TEST_PG names a server.
// TypeScript's own launcher, run by this Node: node_modules/.bin holds a shell script, which Windows cannot start.
const TSC = join(ROOT, "node_modules", "typescript", "bin", "tsc");

const FILES = {
  "string.sql": "select 'a",
  "quoted.sql": "select `a\n",
  "comment.sql": "select 1 /* x",
};

function check(url: string, engine: "sqlite" | "postgres", setup: (root: string) => void): string {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-unterminated-")));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  setup(root);
  mkdirSync(join(root, "node_modules", "@reylosql"), { recursive: true });
  symlinkSync(ROOT, join(root, "node_modules", "reylo"));
  symlinkSync(join(ROOT, "packages", engine), join(root, "node_modules", "@reylosql", engine));
  const compilerOptions = { target: "esnext", module: "nodenext", lib: ["esnext"], types: [], strict: true, noEmit: true, skipLibCheck: true, allowImportingTsExtensions: true };
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ compilerOptions, contentMappers: [{ package: "reylo", extensions: [".sql"] }], include: ["main.ts"] }));
  for (const [name, text] of Object.entries(FILES)) writeFileSync(join(root, name), text);
  // Each module is typed: an import that became `any` would leave its expected error unused.
  writeFileSync(join(root, "main.ts"), Object.keys(FILES).map((name, i) => `import q${i} from "./${name}";\n// @ts-expect-error a typed query has no such property\nq${i}.nope;\n`).join(""));
  const r = spawnSync(process.execPath, [TSC, "-p", "tsconfig.json", "--runExternalCode", "--pretty", "false"], { cwd: root, encoding: "utf8", env: { ...process.env, DATABASE_URL: url } });
  return (r.stdout + r.stderr).trim();
}

const EXPECTED = [
  "comment.sql(1,10): error reylo1: unterminated block comment",
  "quoted.sql(1,8): error reylo1: unterminated quoted identifier",
  "string.sql(1,8): error reylo1: unterminated string literal",
];

test("SQLite: an unterminated string, quoted name or comment is an error at its start, in a typed module", () => {
  const out = check("file:./app.db", "sqlite", (root) => new DatabaseSync(join(root, "app.db")).close());
  assert.deepEqual(out.split("\n").sort(), EXPECTED, out);
});

test("Postgres: an unterminated string, quoted name or comment is an error at its start, in a typed module", { skip: pgLive }, () => {
  const out = check(PG!, "postgres", (root) => {
    mkdirSync(join(root, "node_modules"), { recursive: true });
    symlinkSync(join(DRIVERS, "postgres"), join(root, "node_modules", "postgres"));
  });
  assert.deepEqual(out.split("\n").sort(), EXPECTED, out);
});

test("a diagnostic's range is cut at the end of the file, never past it", () => {
  assert.deepEqual(withinFile(7, 1, 9), { start: 7, length: 1 });
  assert.deepEqual(withinFile(9, 1, 9), { start: 9, length: 0 });
  assert.deepEqual(withinFile(12, 3, 9), { start: 9, length: 0 });
  assert.deepEqual(withinFile(5, 10, 9), { start: 5, length: 4 });
  assert.deepEqual(withinFile(0, 1, 0), { start: 0, length: 0 });
});
