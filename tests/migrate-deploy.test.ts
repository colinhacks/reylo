import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { nativeBinary } from "../src/native.ts";
import { migrate } from "../src/migrate.ts";
import { ROOT } from "./helpers.ts";

// `await migrate()` on a server runs `reylo migrate deploy` in the working directory, as the command
// line does there, on SQLite so that no server is needed.
test("await migrate() runs reylo migrate deploy, resolves to what it ran, and rejects with the deploy's problems", async () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-migrate-deploy-")));
  writeFileSync(join(root, "package.json"), `{ "type": "module" }`);
  writeFileSync(join(root, ".reylo.yaml"), "direct_url: file:./dev.sqlite\n");
  mkdirSync(join(root, "reylo"));
  mkdirSync(join(root, "node_modules"));
  symlinkSync(ROOT, join(root, "node_modules", "reylo"));
  writeFileSync(join(root, "reylo", "schema.sql"), "create table users (id integer primary key, name text not null);\n");
  const env = { ...process.env };
  delete env.CI;
  for (const args of [["new", "users"], ["dev"], ["lock"]]) {
    const r = spawnSync(nativeBinary(), ["migrate", ...args], { cwd: root, encoding: "utf8", env });
    assert.equal(r.status, 0, r.stdout + r.stderr);
  }
  const was = process.cwd();
  process.chdir(root);
  try {
    const ran = await migrate({ url: "file:./prod.sqlite" });
    assert.equal(ran.length, 1);
    assert.match(ran[0]!, /^\d{14}_users\.sql$/);
    assert.deepEqual(await migrate({ url: "file:./prod.sqlite" }), []);
    // The project's own database, which the dev run took to the migrations already.
    assert.deepEqual(await migrate(), []);
    // A draft refuses the deploy, with what to do about it.
    writeFileSync(join(root, "reylo", "migrations", "20991231000000_draft.sql"), "alter table users add column bio text;\n");
    await assert.rejects(migrate({ url: "file:./prod.sqlite" }), /^Error: reylo migrate deploy refused: The lock doesn't list 20991231000000_draft\.sql yet\n  Run it in development, which locks it: reylo migrate dev/);
  } finally {
    process.chdir(was);
  }
});
