import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import reylo from "../src/vite.ts";

// The Cloudflare Vite plugin makes every `.sql` import in a Worker an external text module in its
// own resolveId (`@cloudflare/vite-plugin` 1.64), so the import never reached Reylo's load hook and
// `.run` was undefined. Reylo's plugin now resolves a relative `.sql` import itself, in a resolveId
// ordered "pre", which runs before every other plugin's wherever each is listed.
test("reylo/vite resolves a relative .sql import to its file before any other plugin can claim it", () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-vite-"));
  mkdirSync(join(root, "src"));
  mkdirSync(join(root, "queries"));
  writeFileSync(join(root, "queries", "get_user.sql"), "select 1 as one");
  const importer = join(root, "src", "worker.ts");
  const plugin = reylo();
  // A host that reads hook filters calls a handler only for the ids its filter admits, so every import resolved here must pass resolveId's filter, and the file it resolves to load's.
  const resolveId = (source: string, from: string | undefined) => {
    const path = plugin.resolveId.handler(source, from);
    if (path !== null) assert.ok(plugin.resolveId.filter.id.test(source) && plugin.load.filter.id.test(path), source);
    return path;
  };
  assert.equal(resolveId("../queries/get_user.sql", importer), join(root, "queries", "get_user.sql"));
  assert.equal(resolveId("./get_user.sql", join(root, "queries", "index.ts")), join(root, "queries", "get_user.sql"));
  // Vite's importer may carry a query.
  assert.equal(resolveId("../queries/get_user.sql", `${importer}?v=1a2b`), join(root, "queries", "get_user.sql"));
  // Everything else is left to Vite: a missing file (Vite's own error), a bare or aliased
  // specifier, another extension, an entry with no importer and a virtual importer.
  assert.equal(resolveId("../queries/missing.sql", importer), null);
  assert.equal(resolveId("queries/get_user.sql", importer), null);
  assert.equal(resolveId("~/queries/get_user.sql", importer), null);
  assert.equal(resolveId("../queries/get_user.txt", importer), null);
  assert.equal(resolveId("../queries/get_user.sql", undefined), null);
  assert.equal(resolveId("../queries/get_user.sql", "\0virtual:entry"), null);
});

test("reylo/vite's resolveId runs before the other plugins' hooks", () => {
  assert.equal(reylo().resolveId.order, "pre");
});

// reylo's placeholder for the migrations `migrate(ctx.storage)` runs takes the project's, by its real
// path alone (src/embedded-module.ts), and the dev server leaves reylo to the plugin.
test("reylo/vite fills reylo's migrations placeholder with the project's, and keeps reylo out of the dev server's bundling", async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-vite-migrations-"));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".reylo.yaml"), "direct_url: file:./dev.sqlite\n");
  mkdirSync(join(root, "reylo", "migrations"), { recursive: true });
  writeFileSync(join(root, "reylo", "migrations", "init.sql"), "create table users (id integer primary key);\n");
  const plugin = reylo();
  plugin.configResolved({ root });
  assert.deepEqual(plugin.config().optimizeDeps.exclude, ["reylo"]);
  assert.deepEqual(plugin.configEnvironment().optimizeDeps.exclude, ["reylo"]);
  const placeholder = join(import.meta.dirname, "..", "src", "embedded-migrations.ts");
  const watched: string[] = [];
  const load = (id: string) => plugin.load.handler.call({ addWatchFile: (f: string) => watched.push(f) }, id) as Promise<{ code: string } | null>;
  for (const id of [placeholder, `${placeholder}?v=1a2b3c`]) {
    assert.ok(plugin.load.filter.id.test(id), id);
    assert.equal((await load(id))?.code, `export const embedded = [{"name":"init.sql","sql":"create table users (id integer primary key);\\n"}];\n`);
  }
  assert.ok(watched.includes(join(root, "reylo", "migrations", "init.sql")));
  // A project's own file by that name is the project's.
  writeFileSync(join(root, "embedded-migrations.ts"), "export const mine = 1;\n");
  assert.equal(await load(join(root, "embedded-migrations.ts")), null);
});
