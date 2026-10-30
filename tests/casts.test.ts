import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { sqlModule } from "../src/loader.ts";
import { DRIVERS, mysqlLive, INDEX, closeSessions, plain, mysqlDatabase } from "./helpers.ts";

// A parameter's `::type` on MySQL and SQLite, which have no `::`: the build writes the dialect's
// own cast, and the run time checks the value against the type, since both databases' casts turn
// a value they cannot read into 0 or NULL (`::` everywhere,
// converted by the compiler). MySQL runs on a throwaway server named by REYLO_TEST_MYSQL, in a database of
// its own.

function project(driver?: string): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-casts-"));
  writeFileSync(join(root, "package.json"), "{}");
  if (driver) {
    mkdirSync(join(root, "node_modules"));
    symlinkSync(join(DRIVERS, driver), join(root, "node_modules", driver));
  }
  return root;
}

/** The module a build step writes for `sql`, imported, and its compiled statement (the query's `sql`), which Reylo sends with its file's comment after it. */
async function built(root: string, url: string, name: string, sql: string) {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, sql);
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  const { code } = await sqlModule(file, sql);
  const out = join(root, `${name}.mjs`);
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  const module = await import(pathToFileURL(out).href);
  const text = module.default.sql as string;
  assert.equal(module.default["~standard"].query.text, `${text} /*file='${name}.sql'*/`);
  return { query: module.default, text, code };
}

test("SQLite: a cast is SQLite's own, a date's to TEXT, JSON's through json(), and a value the cast would change is refused", async () => {
  const root = project();
  const db = new DatabaseSync(join(root, "app.db"));
  db.exec("create table users (id integer primary key, name text not null, born date, score real)");
  db.exec("insert into users (name, born, score) values ('Ada', '1815-12-10', 1.5), ('Alan', '1912-06-23', 2.5), ('Grace', '1906-12-09', 3.5)");
  const url = "file:./app.db";
  try {
    const page = await built(root, url, "page", "select id from users order by id limit :n::int");
    assert.equal(page.text, "select id from users order by id limit CAST(? AS INTEGER)");
    assert.deepEqual(plain(await page.query.run(db, { n: 2 })), [{ id: 1 }, { id: 2 }]);
    // SQLite's CAST reads 'abc' as 0 and '1.9' as 1.
    assert.deepEqual({ ...(db.prepare("select cast('abc' as integer) as a, cast('1.9' as integer) as b").get() as object) }, { a: 0, b: 1 });
    await assert.rejects(page.query.run(db, { n: "abc" }), /Parameter "n" is int, an integer; got "abc"/);
    await assert.rejects(page.query.run(db, { n: 1.9 }), /Parameter "n" is int, an integer; got 1\.9/);
    // Its CAST to `date` reads a date as the number before its first dash, so a date is cast to TEXT.
    assert.deepEqual({ ...(db.prepare("select cast('1900-01-01' as date) as d").get() as object) }, { d: 1900 });
    const born = await built(root, url, "born", "select name from users where born > :since::date order by born");
    assert.equal(born.text, "select name from users where born > CAST(? AS TEXT) order by born");
    assert.deepEqual(plain(await born.query.run(db, { since: "1900-01-01" })), [{ name: "Grace" }, { name: "Alan" }]);
    // json() makes the text a JSON value inside SQLite's JSON functions, where text alone is a
    // string; the object json_object builds comes back parsed.
    const doc = await built(root, url, "doc", "select json_extract(:doc::json, '$.a') as a, json_object('d', :inner::json, 's', :text) as d");
    assert.equal(doc.text, "select json_extract(json(?), '$.a') as a, json_object('d', json(?), 's', ?) as d");
    assert.match(doc.code, /"doc":\{"type":\{"kind":"json"/);
    assert.deepEqual({ ...(await doc.query.run(db, { doc: { a: 5 }, inner: [1], text: "[1]" })) }, { a: 5, d: { d: [1], s: "[1]" } });
    const short = await built(root, url, "short", "select :s::varchar(3) as s, :x::real * 2 as x");
    await assert.rejects(short.query.run(db, { s: "abcd", x: 1 }), /Parameter "s" is varchar\(3\), at most 3 characters; got 4/);
    await assert.rejects(short.query.run(db, { s: "abc", x: "2" }), /Parameter "x" is real, a finite number; got "2"/);
    assert.deepEqual({ ...(await short.query.run(db, { s: "abc", x: 1.25 })) }, { s: "abc", x: 2.5 });
    await assert.rejects(built(root, url, "flag", "select :b::boolean as b"), /SQLite has no boolean type/);
  } finally {
    db.close();
    await closeSessions();
  }
});

test("MySQL: a cast is MySQL's own, LIMIT and a text cast take the bare placeholder, and a value the cast would change is refused", { skip: mysqlLive }, async () => {
  const own = await mysqlDatabase("reylo_casts");
  const url = new URL(own.url);
  const db = own.db;
  const root = project("mysql2");
  try {
    await db.query("create table events (id bigint auto_increment primary key, amount decimal(10,2) not null, at datetime(6) not null)");
    await db.query("insert into events (amount, at) values (1.50, '2026-10-01 10:00:00'), (2.50, '2026-10-02 10:00:00'), (3.50, '2026-10-03 10:00:00')");
    const big = await built(root, url.href, "big", "select id from events where amount > :min::numeric(10,2) order by id limit :n::int offset :skip::int");
    assert.equal(big.text, "select id from events where amount > CAST(? AS DECIMAL(10,2)) order by id limit ? offset ?");
    // A BIGINT is a BigInt (src/values.ts).
    assert.deepEqual(plain(await big.query.run(db, { min: "2.00", n: 5, skip: 0 })), [{ id: 2n }, { id: 3n }]);
    assert.deepEqual(plain(await big.query.run(db, { min: 1, n: 1, skip: 1 })), [{ id: 2n }]);
    // MySQL's CAST reads 'abc' as 0 and rounds 1.239 to 1.24.
    const [[raw]] = await db.query("select cast('abc' as signed) as a, cast(1.239 as decimal(10,2)) as b");
    assert.deepEqual({ ...raw }, { a: 0, b: "1.24" });
    await assert.rejects(big.query.run(db, { min: "abc", n: 1, skip: 0 }), /Parameter "min" is numeric\(10,2\), a decimal/);
    await assert.rejects(big.query.run(db, { min: 1.239, n: 1, skip: 0 }), /keeps 2 digits after the point, so 1\.239 would be rounded/);
    await assert.rejects(big.query.run(db, { min: 1, n: "5", skip: 0 }), /Parameter "n" is int, an integer; got "5"/);
    await assert.rejects(big.query.run(db, { min: 1, n: 2 ** 31, skip: 0 }), /Parameter "n" is int, from -2147483648 to 2147483647/);
    const before = await built(root, url.href, "before", "select id from events where at < :t::datetime order by id");
    assert.equal(before.text, "select id from events where at < CAST(? AS DATETIME(6)) order by id");
    assert.deepEqual(plain(await before.query.run(db, { t: "2026-10-02 00:00:00" })), [{ id: 1n }]);
    await assert.rejects(before.query.run(db, { t: "2026-02-30 00:00:00" }), /Parameter "t" is datetime, as text like "2026-10-12 19:06:48\.282"/);
    await assert.rejects(before.query.run(db, { t: new Date() }), /takes its text .*not a Date/);
    const doc = await built(root, url.href, "doc", "select json_extract(:doc::json, '$.a') as a, :id::bigint unsigned as id");
    assert.equal(doc.text, "select json_extract(CAST(? AS JSON), '$.a') as a, CAST(? AS UNSIGNED) as id");
    assert.deepEqual({ ...(await doc.query.run(db, { doc: { a: [1, 2] }, id: 7 })) }, { a: [1, 2], id: 7n });
    await assert.rejects(doc.query.run(db, { doc: {}, id: -1 }), /Parameter "id" is bigint unsigned, from 0 to 18446744073709551615; got -1/);
    // A text cast is the bare placeholder: a CAST to CHAR would take the connection's collation
    // (utf8mb4_unicode_ci under mysql2), which a utf8mb4_0900_ai_ci column refuses to compare with
    // ("Illegal mix of collations"), while a bare placeholder takes the column's.
    await db.query("create table people (id bigint primary key, name varchar(50) collate utf8mb4_0900_ai_ci not null, code char(3) collate utf8mb4_0900_ai_ci not null, uid char(36) collate utf8mb4_0900_ai_ci not null)");
    await db.query("insert into people values (1, 'Ada', 'ADA', '6f1c1a8e-7a43-4c39-9d0c-3c1f8e3b8a01'), (2, 'Alan', 'ALN', '0b7e3f52-1d2c-4e8a-9f0e-5a6b7c8d9e02'), (3, 'Grace', 'GRC', 'c3d4e5f6-a7b8-4c9d-8e0f-1a2b3c4d5e03')");
    await assert.rejects(db.execute("select name from people where name like concat(cast(? as char), '%')", ["A"]), /Illegal mix of collations/);
    const prefix = await built(root, url.href, "prefix", "select name from people where name like concat(:prefix::text, '%') and code <> :code::char(3) and uid <> :uid::uuid order by id");
    assert.equal(prefix.text, "select name from people where name like concat(?, '%') and code <> ? and uid <> ? order by id");
    // MySQL types the bare placeholder as a string, and the parameter keeps its cast.
    assert.match(prefix.code, /"prefix":\{"type":\{"kind":"scalar","name":"text"\},"cast":"text","nullable":false\}/);
    const alone = await built(root, url.href, "alone", "select :p::varchar(5) as p");
    assert.equal(alone.text, "select ? as p");
    assert.match(alone.code, /"p":\{"type":\{"kind":"scalar","name":"text"\},"cast":"varchar\(5\)","nullable":false\}/);
    assert.deepEqual({ ...(await alone.query.run(db, { p: "abc" })) }, { p: "abc" });
    assert.deepEqual(plain(await prefix.query.run(db, { prefix: "A", code: "ALN", uid: "c3d4e5f6-a7b8-4c9d-8e0f-1a2b3c4d5e03" })), [{ name: "Ada" }]);
    // The run time still checks the value against the cast.
    await assert.rejects(prefix.query.run(db, { prefix: 1, code: "ALN", uid: "c3d4e5f6-a7b8-4c9d-8e0f-1a2b3c4d5e03" }), /Parameter "prefix" is text, a string; got 1/);
    await assert.rejects(prefix.query.run(db, { prefix: "A", code: "ALAN", uid: "c3d4e5f6-a7b8-4c9d-8e0f-1a2b3c4d5e03" }), /Parameter "code" is char\(3\), at most 3 characters; got 4/);
    await assert.rejects(prefix.query.run(db, { prefix: "A", code: "ALN", uid: "not-a-uuid" }), /Parameter "uid" is uuid, a UUID in hexadecimal/);
    await assert.rejects(built(root, url.href, "flag", "select :b::bool as b"), /MySQL has no boolean type/);
    await assert.rejects(built(root, url.href, "zone", "select :t::timestamptz as t"), /drops the zone/);
  } finally {
    await closeSessions();
    await own.drop();
  }
});

test("MySQL: a window frame's bound, NTILE's argument and the offset of LAG and LEAD take the bare placeholder, and the value is checked", { skip: mysqlLive }, async () => {
  const own = await mysqlDatabase("reylo_window_casts");
  const url = new URL(own.url);
  const db = own.db;
  const root = project("mysql2");
  try {
    await db.query("create table t (id int primary key, n int not null)");
    await db.query("insert into t values (1, 10), (2, 20), (3, 30)");
    // MySQL takes only a literal or `?` in each, and refused `CAST(? AS SIGNED)` with a syntax error.
    const w = await built(root, url.href, "window", "select id, sum(n) over (order by id rows between :back::int preceding and current row) as s, ntile(:tiles::int) over (order by id) as b, lag(n, :lag::int) over (order by id) as p, lead(n, :lead::int) over (order by id) as q from t order by id");
    assert.equal(w.text, "select id, sum(n) over (order by id rows between ? preceding and current row) as s, ntile(?) over (order by id) as b, lag(n, ?) over (order by id) as p, lead(n, ?) over (order by id) as q from t order by id");
    assert.deepEqual(plain(await w.query.run(db, { back: 1, tiles: 2, lag: 1, lead: 1 })).map((r) => Object.values(r).map(String)), [["1", "10", "1", "null", "20"], ["2", "30", "1", "10", "30"], ["3", "50", "2", "20", "null"]]);
    await assert.rejects(w.query.run(db, { back: "1", tiles: 2, lag: 1, lead: 1 }), /Parameter "back" is int, an integer; got "1"/);
    // A frame over a RANGE takes a number of any kind.
    const range = await built(root, url.href, "range", "select id, count(*) over (order by n range between :k::numeric preceding and current row) as c from t order by id");
    assert.equal(range.text, "select id, count(*) over (order by n range between ? preceding and current row) as c from t order by id");
    assert.deepEqual(plain(await range.query.run(db, { k: 10 })).map((r) => Object.values(r).map(String)), [["1", "1"], ["2", "2"], ["3", "2"]]);
    await assert.rejects(built(root, url.href, "tiles", "select ntile(:k::text) over (order by id) from t"), /NTILE's argument and the offset of LAG and LEAD take an integer/);
  } finally {
    await closeSessions();
    await own.drop();
  }
});
