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

// An import of the project file builds to its migrations, for a Durable Object's `migrate`
// (src/project-module.ts); its load is the same hook's, so the filter admits it.
test("reylo/vite builds an import of the project file into its migrations", async () => {
  const root = mkdtempSync(join(tmpdir(), "reylo-vite-project-file-"));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".reylo.yaml"), "direct_url: file:./dev.sqlite\n");
  mkdirSync(join(root, "reylo", "migrations"), { recursive: true });
  writeFileSync(join(root, "reylo", "migrations", "init.sql"), "create table users (id integer primary key);\n");
  const plugin = reylo();
  for (const id of [join(root, ".reylo.yaml"), join(root, "config", "staging.reylo.yml")]) assert.ok(plugin.load.filter.id.test(id), id);
  assert.ok(!plugin.load.filter.id.test(join(root, "reylo.yaml.bak")));
  const watched: string[] = [];
  const out = (await plugin.load.handler.call({ addWatchFile: (f: string) => watched.push(f) }, join(root, ".reylo.yaml"))) as { code: string };
  assert.match(out.code, /export const \{ migrate \} = migrations\(\[\{"name":"init\.sql","sql":"create table users \(id integer primary key\);\\n"\}\]\);/);
  assert.ok(watched.includes(join(root, "reylo", "migrations", "init.sql")));
});
