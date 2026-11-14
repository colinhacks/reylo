import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import loader from "../src/webpack.ts";
import { closeSessions } from "./helpers.ts";

const SQL = "select id, email from users where id = :id";

/** A project on a SQLite file holding `users` as `columns` declares it, and its one query. */
function project(columns: string): { root: string; migrate(columns: string): void } {
  const root = mkdtempSync(join(tmpdir(), "reylo-webpack-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".reylo.yaml"), 'direct_url: "file:./app.db"\n');
  writeFileSync(join(root, "q.sql"), SQL);
  const migrate = (to: string) => {
    const db = new DatabaseSync(join(root, "app.db"));
    db.exec(`drop table if exists users; create table users (${to})`);
    db.close();
  };
  migrate(columns);
  return { root, migrate };
}

/** The loader run as webpack runs it: the module's code, and what it told the host about caching. */
function run(root: string, host: { cacheable?: true } = { cacheable: true }): Promise<{ code: string; cacheable: boolean[] }> {
  const cacheable: boolean[] = [];
  return new Promise((done, fail) =>
    loader.call(
      {
        resourcePath: join(root, "q.sql"),
        addDependency() {},
        ...(host.cacheable ? { cacheable: (flag: boolean) => void cacheable.push(flag) } : {}),
        async: () => (error, code) => (error ? fail(error) : done({ code: code!, cacheable })),
      },
      SQL,
    ),
  );
}

// webpack keeps a loader's result while the files it depends on are unchanged, on disk across
// builds where its cache is. A migration changes no such file, so a kept module would decode the
// new schema's rows by the old one's columns.
test("reylo/webpack lets no host keep its module, so a build after a migration has the new schema's columns", async () => {
  const { root, migrate } = project("id text primary key, email integer");
  try {
    const before = await run(root);
    assert.deepEqual(before.cacheable, [false]);
    assert.match(before.code, /\[\["id","text"\],\["email","int"\]\]/);
    // A session trusts the schema it read for a few seconds: the next build is another process.
    await closeSessions();
    migrate("id integer primary key, email text not null");
    assert.match((await run(root)).code, /\[\["id","int"\],\["email","text"\]\]/);
    // A host with no cache to turn off, as a loader runner may be, still gets its module.
    assert.match((await run(root, {})).code, /\[\["id","int"\],\["email","text"\]\]/);
  } finally {
    await closeSessions();
  }
});
