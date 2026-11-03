import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const REGISTER = join(import.meta.dirname, "..", "src", "register.ts");
// A child's environment is PATH alone, and REYLO_ENGINE_PATH where it names the engine, as on CI; without it, the child takes this checkout's cargo build.
const ENV = { PATH: process.env.PATH, ...(process.env.REYLO_ENGINE_PATH ? { REYLO_ENGINE_PATH: process.env.REYLO_ENGINE_PATH } : {}) };

test("a program run under the reylo/register preload prints nothing on stderr", () => {
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
  // The engine is the preload's worker's child, whose stderr is the program's own.
  const r = spawnSync(process.execPath, ["--import", pathToFileURL(REGISTER).href, "main.ts"], { cwd: root, encoding: "utf8", env: { ...ENV, DATABASE_URL: "file:./app.db" } });
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, `{"id":1,"name":"ada"}\n`);
  assert.equal(r.stderr, "");
});
