import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, realpathSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { request } from "node:http";
import { join } from "node:path";
import { startPlay } from "../src/play.ts";

// `reylo play` (src/play.ts) serves the page it finds in the install; these tests give it one, so
// that they need no build of it.
const page = mkdtempSync(join(tmpdir(), "reylo-play-page-"));
writeFileSync(join(page, "index.html"), "<!doctype html><title>the page</title>");
process.env.REYLO_PLAY_PAGE = page;

function project(files: Record<string, string>): string {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "reylo-play-")));
  writeFileSync(join(root, "package.json"), "{}");
  for (const [name, text] of Object.entries(files)) {
    mkdirSync(join(root, name, ".."), { recursive: true });
    writeFileSync(join(root, name), text);
  }
  return root;
}

const get = async (url: string, headers: Record<string, string> = {}) => {
  const res = await fetch(url, { headers });
  return { status: res.status, body: res.headers.get("content-type")?.startsWith("application/json") ? await res.json() : await res.text() };
};

test("reylo play analyzes each statement against the project's database, without running it or writing .reylo/", async () => {
  const root = project({ ".reylo.yaml": 'direct_url: "file:./app.db"\n' });
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null, email text)");
  db.close();
  const play = await startPlay({ cwd: root, port: 0, env: {} });
  try {
    assert.match(play.label, /^SQLite at .*app\.db \("direct_url" in \.reylo\.yaml\)$/);
    const q = (sql: string) => `${play.url}api/analyze?q=${encodeURIComponent(sql)}`;

    const user = await get(q("select id, name, email from users where id = :id"));
    assert.equal(user.status, 200);
    assert.deepEqual({ ...user.body, ms: 0 }, { sql: "select id, name, email from users where id = :id", params: "{ id: number }", returns: "{ id: number; name: string; email: string | null } | null", cardinality: "maybeOne", diagnostics: [], ms: 0 });

    // An error is placed as an editor counts: by line and column.
    const bad = await get(q("select id,\n  nope\nfrom users"));
    assert.equal(bad.body.diagnostics.length, 1);
    assert.match(bad.body.diagnostics[0].message, /no such column: nope/);
    assert.deepEqual({ line: bad.body.diagnostics[0].line, column: bad.body.diagnostics[0].column }, { line: 2, column: 3 });

    // The same as JSON in a POST, with the types broken over lines past `width`.
    const res = await fetch(`${play.url}api/analyze`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ sql: "select id, name, email from users", width: 20 }) });
    assert.equal((await res.json()).returns, "{\n  id: number;\n  name: string;\n  email: string | null;\n}[]");

    const schema = await get(`${play.url}api/schema`);
    assert.equal(schema.body.dialect, "sqlite");
    assert.deepEqual(schema.body.tables, [{ name: "users", columns: [{ name: "id", type: "INTEGER", nullable: true }, { name: "name", type: "TEXT", nullable: false }, { name: "email", type: "TEXT", nullable: true }] }]);

    // The page at any path but the API's, which reads its query from the address.
    assert.equal((await get(`${play.url}?q=select%201`)).body, "<!doctype html><title>the page</title>");
    assert.equal((await get(`${play.url}somewhere/else`)).body, "<!doctype html><title>the page</title>");
    assert.equal((await get(q("select 1"), {})).status, 200);
    assert.equal((await get(`${play.url}api/analyze`)).status, 400);
    assert.equal(existsSync(join(root, ".reylo")), false);
  } finally {
    await play.close();
  }
});

test("reylo play answers this machine's own pages and tools, and refuses another site's page or a rebound name", async () => {
  const root = project({ ".reylo.yaml": 'direct_url: "file:./app.db"\n' });
  new DatabaseSync(join(root, "app.db")).close();
  const play = await startPlay({ cwd: root, port: 0, env: {} });
  try {
    const schema = `${play.url}api/schema`;
    assert.equal((await get(schema)).status, 200);
    assert.equal((await get(schema, { origin: play.url.slice(0, -1) })).status, 200);
    assert.equal((await get(schema, { origin: "https://example.com" })).status, 403);
    // fetch() sends its own Host header; a name rebound to 127.0.0.1 sends the name.
    const rebound = await new Promise<number>((done, fail) => request(schema, { headers: { host: `example.com:${new URL(play.url).port}` } }, (res) => (res.resume(), done(res.statusCode!))).on("error", fail).end());
    assert.equal(rebound, 403);
  } finally {
    await play.close();
  }
});

test("reylo play stands PGlite in for a project with no database URL, with its schema file's tables or a sample's", async () => {
  const root = project({ "db/schema.sql": "create table notes (id serial primary key, body text not null, pinned boolean);" });
  const withFile = await startPlay({ cwd: root, port: 0, env: {}, schema: "db/schema.sql" });
  try {
    assert.match(withFile.label, /^Postgres \d+\.\d+ via PGlite, with db\/schema\.sql$/);
    const notes = await get(`${withFile.url}api/analyze?q=${encodeURIComponent("select body, pinned from notes where id = :id")}`);
    assert.equal(notes.body.returns, "{ body: string; pinned: boolean | null } | null");
    assert.equal(notes.body.params, "{ id: number }");
  } finally {
    await withFile.close();
  }
  const sample = await startPlay({ cwd: root, port: 0, env: {} });
  try {
    assert.match(sample.label, /with a sample schema$/);
    const posts = await get(`${sample.url}api/analyze?q=${encodeURIComponent("select title from posts")}`);
    assert.equal(posts.body.returns, "{ title: string }[]");
  } finally {
    await sample.close();
  }
  await assert.rejects(startPlay({ cwd: root, port: 0, env: {}, schema: "db/missing.sql" }), /--schema: .*missing\.sql does not exist/);
});
