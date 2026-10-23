import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { withReylo } from "../src/next.ts";

// `next build` type-checks with the project's own tsc and no --runExternalCode, so withReylo turns
// that check off and runs it with the flag in compiler.runAfterProductionCompile (src/next.ts).
const REPO = join(import.meta.dirname, "..");

type Built = {
  typescript?: { ignoreBuildErrors?: boolean; tsconfigPath?: string };
  compiler?: { runAfterProductionCompile?: (metadata: { projectDir: string; distDir: string }) => Promise<void> };
};
/** The hook withReylo leaves in the config it returns, which next build runs after the compile. */
const afterCompile = (config: Built) => (withReylo(config) as Built).compiler!.runAfterProductionCompile!;

test("withReylo adds the loader on the server for Turbopack and webpack, and keeps the project's config", () => {
  const seen: string[] = [];
  const config = withReylo({
    reactStrictMode: true,
    turbopack: { root: "/app", rules: { "*.md": { loaders: ["md-loader"], as: "*.js" } } },
    webpack(webpackConfig: { module: { rules: unknown[] } }, context: { isServer: boolean }) {
      seen.push(context.isServer ? "server" : "client");
      return webpackConfig;
    },
  });
  assert.equal(config.reactStrictMode, true);
  assert.equal(config.turbopack.root, "/app");
  assert.deepEqual(config.turbopack.rules, {
    "*.sql": { condition: { not: "browser" }, loaders: ["reylo/turbopack"], as: "*.js" },
    "*.md": { loaders: ["md-loader"], as: "*.js" },
  });
  const server = config.webpack({ module: { rules: [] } }, { isServer: true });
  assert.deepEqual(server.module.rules, [{ test: /\.sql$/, use: "reylo/webpack" }]);
  const client = config.webpack({ module: { rules: [] } }, { isServer: false });
  assert.deepEqual(client.module.rules, []);
  assert.deepEqual(seen, ["server", "client"]);
  // A rule of the project's own for *.sql wins.
  const own = withReylo({ turbopack: { rules: { "*.sql": { loaders: ["own"] } } } });
  assert.deepEqual(own.turbopack.rules["*.sql"], { loaders: ["own"] });
});

test("withReylo takes a config of Next's own keys written in place, beside the ones it reads", () => {
  // A type check: each key here is one withReylo's own type does not name.
  const config = withReylo({ reactStrictMode: true, turbopack: { root: "/app" }, typescript: { tsconfigPath: "tsconfig.json" }, compiler: { removeConsole: true } });
  assert.equal(config.reactStrictMode, true);
  assert.equal(config.turbopack.root, "/app");
  assert.deepEqual(config.compiler.removeConsole, true);
});

test("withReylo turns Next's type check off for its own, and leaves both off where the project turned Next's off", () => {
  const config: Built = withReylo({ typescript: { tsconfigPath: "tsconfig.app.json" } });
  assert.deepEqual(config.typescript, { tsconfigPath: "tsconfig.app.json", ignoreBuildErrors: true });
  assert.equal(typeof config.compiler?.runAfterProductionCompile, "function");
  const off: Built = withReylo({ typescript: { ignoreBuildErrors: true } });
  assert.deepEqual(off.typescript, { ignoreBuildErrors: true });
  assert.equal(off.compiler, undefined);
});

/** A Next.js app's directory with a SQLite database, a query and the TypeScript 7.1 of this repository. */
function app(main: string): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-next-"));
  writeFileSync(join(root, "package.json"), `{ "private": true, "type": "module" }`);
  writeFileSync(join(root, ".env"), "DATABASE_URL=file:./app.db\n");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null)");
  db.close();
  writeFileSync(join(root, "get_user.sql"), "select id, name from users where id = :id\n");
  writeFileSync(join(root, "main.ts"), main);
  writeFileSync(join(root, "next-env.d.ts"), "");
  writeFileSync(
    join(root, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: { target: "esnext", module: "nodenext", types: [], strict: true, noEmit: true, skipLibCheck: true, allowImportingTsExtensions: true },
      contentMappers: [{ package: "reylo", extensions: [".sql"] }],
      include: ["*.ts"],
    }),
  );
  mkdirSync(join(root, "node_modules"));
  symlinkSync(REPO, join(root, "node_modules", "reylo"));
  symlinkSync(join(REPO, "node_modules", "typescript"), join(root, "node_modules", "typescript"));
  return root;
}

const ROW = `import getUser from "./get_user.sql";\ntype User = NonNullable<Awaited<ReturnType<typeof getUser.run>>>;\n`;

test("the type check runs the content mapper after the project's own hook, and fails on an error at its line and column", async (t) => {
  const printed = t.mock.method(console, "log", () => {});
  const order: string[] = [];
  const hook = afterCompile({
    compiler: {
      async runAfterProductionCompile() {
        order.push("project");
      },
    },
  });

  // Without --runExternalCode, `User` would be an error: the .sql import is TS2307.
  const good = app(`${ROW}export const name: User["name"] = "ada";\n`);
  await hook({ projectDir: good, distDir: join(good, ".next") });
  assert.deepEqual(order, ["project"]);
  assert.equal(printed.mock.callCount(), 0);

  const bad = app(`${ROW}export const id: User["id"] = "one";\n`);
  await assert.rejects(hook({ projectDir: bad, distDir: join(bad, ".next") }), /Failed to type check/);
  const output = printed.mock.calls.map((call) => String(call.arguments[0])).join("\n");
  assert.match(output, /main\.ts\(3,14\): error TS2322: Type 'string' is not assignable to type 'number'\./);
});

test("on a first build the type check runs next typegen, which writes next-env.d.ts", async (t) => {
  t.mock.method(console, "log", () => {});
  const root = app(`${ROW}export const name: User["name"] = "ada";\n`);
  const next = join(root, "node_modules", "next");
  mkdirSync(join(next, "dist", "bin"), { recursive: true });
  writeFileSync(join(next, "package.json"), JSON.stringify({ name: "next", bin: { next: "./dist/bin/next" } }));
  writeFileSync(
    join(next, "dist", "bin", "next"),
    `require("node:fs").writeFileSync(require("node:path").join(process.argv[3], "next-env.d.ts"), "// " + process.argv.slice(2).join(" ") + "\\n");\n`,
  );
  rmSync(join(root, "next-env.d.ts"));
  await afterCompile({})({ projectDir: root, distDir: join(root, ".next") });
  assert.equal(readFileSync(join(root, "next-env.d.ts"), "utf8"), `// typegen ${root}\n`);
});

test("the type check refuses a TypeScript older than 7.1, which has no content mappers", async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-next-"));
  writeFileSync(join(root, "package.json"), "{}");
  mkdirSync(join(root, "node_modules", "typescript"), { recursive: true });
  writeFileSync(join(root, "node_modules", "typescript", "package.json"), `{ "name": "typescript", "version": "6.0.3" }`);
  await assert.rejects(
    afterCompile({})({ projectDir: root, distDir: join(root, ".next") }),
    /needs TypeScript 7\.1 or later, whose content mappers type \.sql imports; the project has TypeScript 6\.0\.3/,
  );
  assert.equal(existsSync(join(root, "next-env.d.ts")), false);
});
