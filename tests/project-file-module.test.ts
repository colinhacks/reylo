import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { ROOT } from "./helpers.ts";

// The setup lists `.reylo.yaml` and `.reylo.yml` among the content mapper's extensions beside `.sql`,
// so the project file is the mapper's to type rather than a YAML loader's. Its module declares
// `migrate`, for a Durable Object (src/project-module-types.ts), and neither an import of it nor a
// `<name>.reylo.yaml` that `include` takes in is read as a query. The type check runs as a project runs it, over SQLite, so no server is needed.
// TypeScript's own launcher, run by this Node: node_modules/.bin holds a shell script, which Windows cannot start.
const TSC = join(ROOT, "node_modules", "typescript", "bin", "tsc");

test("a project file in the mapper's extensions is a module that declares migrate, never a query", () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-project-file-")));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".env"), "DATABASE_URL=file:./app.db\n");
  writeFileSync(join(root, ".reylo.yaml"), "# the project file\nallow_untyped_params: false\n");
  mkdirSync(join(root, "config"));
  writeFileSync(join(root, "config", "staging.reylo.yaml"), "allow_untyped_params: false\n");
  writeFileSync(join(root, "config", "local.reylo.yml"), "allow_untyped_params: false\n");
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table posts (id integer primary key, title text not null)");
  db.close();
  mkdirSync(join(root, "node_modules"), { recursive: true });
  symlinkSync(ROOT, join(root, "node_modules", "reylo"));
  const compilerOptions = { target: "esnext", module: "nodenext", lib: ["esnext"], types: [], strict: true, noEmit: true, skipLibCheck: true, allowImportingTsExtensions: true };
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ compilerOptions, contentMappers: [{ package: "reylo", extensions: [".sql", ".reylo.yaml", ".reylo.yml"] }], include: ["main.ts", "config"] }));
  writeFileSync(join(root, "posts.sql"), "select id, title from posts where id > :after::integer\n");
  writeFileSync(
    join(root, "main.ts"),
    [
      `import posts from "./posts.sql";`,
      `import type * as project from "./.reylo.yaml";`,
      `import type * as local from "./config/local.reylo.yml";`,
      `// @ts-expect-error the project file's module has no default export`,
      `import config from "./.reylo.yaml";`,
      `import { migrate } from "./.reylo.yaml";`,
      `// A Durable Object's storage, as Cloudflare's types give it: exec is generic over its row.`,
      `declare const storage: { sql: { exec<T extends Record<string, unknown>>(query: string, ...bindings: any[]): { toArray(): T[]; one(): T } }; transactionSync<T>(closure: () => T): T; getAlarm(): Promise<number | null> };`,
      `export const ran: string[] = migrate(storage);`,
      `// @ts-expect-error migrate takes a Durable Object's storage, not its SQL alone`,
      `migrate(storage.sql);`,
      `export const query = posts;`,
      `export type Project = typeof project | typeof local;`,
      `export const unused = config;`,
      ``,
    ].join("\n"),
  );
  const r = spawnSync(process.execPath, [TSC, "-p", "tsconfig.json", "--runExternalCode", "--pretty", "false"], { cwd: root, encoding: "utf8" });
  assert.equal(r.stdout + r.stderr, "");
  assert.equal(r.status, 0);
});
