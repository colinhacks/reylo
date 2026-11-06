import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { Session } from "../src/analyze.ts";
import { sqlModule } from "../src/loader.ts";
import { DRIVERS, INDEX, MYSQL, driver, mysqlDatabase, mysqlLive, pgLive, postgresDatabase } from "./helpers.ts";

// A Postgres row value of known fields, and each built-in geometric type, is an object of its
// fields, and a range an object of its bounds (src/values.ts): the run time decodes each from the
// server's text, a parameter takes the same object back and binds the text the type's input reads,
// a structured parameter's member too, and a project's js.codecs reach its fields and bounds
// (src/codecs.ts). A MySQL SET is a Set of its members, read and bound.
const Temporal: any = (globalThis as any).Temporal;

const SCHEMA = `create type address as (street text, zip int4, tags text[], at timestamptz, spot point);
create table places (id int primary key, home address, homes address[], spot point, boxes box[], route path, shape polygon, ring circle, edge lseg, l line);
create table periods (id int primary key, seats int4range, at tstzrange, price numrange, days daterange, many int8multirange, spans int4range[])`;

const PERIOD = {
  id: 1,
  seats: { lower: 1, upper: 5, bounds: "[)" },
  at: { lower: "2026-10-20T08:00:00.000000Z", bounds: "[)" },
  price: { lower: "1.50", upper: "10", bounds: "(]" },
  days: { lower: "2026-10-20", upper: "infinity", bounds: "[)" },
  many: [{ lower: 1n, upper: 3n, bounds: "[)" }, { lower: 9007199254740993n, upper: 9007199254740995n, bounds: "[)" }],
  spans: [{ empty: true }, { bounds: "()" }, { upper: 0, bounds: "()" }],
};

const PLACE = {
  id: 1,
  home: { street: '1 "Main", St (rear)\\', zip: 12345, tags: ["a", "b c", null, ""], at: "2026-10-20T08:00:00.500000Z", spot: { x: 1.5, y: -2 } },
  homes: [{ street: "x", zip: null, tags: null, at: null, spot: null }, { street: "", zip: 0, tags: [], at: "0001-01-01T00:00:00.000000Z", spot: { x: 0, y: 0 } }],
  spot: { x: -0.5, y: 1e300 },
  boxes: [{ high: { x: 3, y: 4 }, low: { x: 1, y: 2 } }, { high: { x: 1, y: 1 }, low: { x: 0, y: 0 } }],
  route: { closed: false, points: [{ x: 0, y: 0 }, { x: 1, y: 1 }] },
  shape: [{ x: 0, y: 0 }, { x: 1, y: 1 }, { x: 2, y: 0 }],
  ring: { center: { x: 1, y: 2 }, radius: 3 },
  edge: [{ x: 0, y: 0 }, { x: 1, y: 1 }],
  l: { a: 1, b: -1, c: 0 },
};

/** A project on the database at `url`, with the drivers linked and the project file `yaml` where one is given. */
function project(url: string, yaml?: string, drivers = ["pg", "postgres"]): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-records-"));
  writeFileSync(join(root, "package.json"), "{}");
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  if (yaml !== undefined) writeFileSync(join(root, ".reylo.yaml"), yaml);
  mkdirSync(join(root, "node_modules"));
  for (const name of drivers) symlinkSync(join(DRIVERS, name), join(root, "node_modules", name));
  return root;
}

/** The module a build step writes for `sql`, imported. */
async function built(root: string, name: string, sql: string) {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, sql);
  const { code } = await sqlModule(file, sql);
  const out = join(root, `${name}.mjs`);
  writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
  return (await import(pathToFileURL(out).href)).default;
}

test("a row value, a geometric type and a range go back in as they came out, on pg and postgres.js, in a structured parameter too", { skip: pgLive, timeout: 60_000 }, async () => {
  const own = await postgresDatabase("reylo_records", SCHEMA);
  const root = project(own.url);
  const pg = driver("pg");
  const postgres = driver("postgres");
  const client = new pg.Client(own.url);
  await client.connect();
  const sql = postgres(own.url, { max: 1, onnotice: () => {} });
  try {
    const insert = await built(root, "insert_place", "insert into places values (:id, :home, :homes, :spot, :boxes, :route, :shape, :ring, :edge, :l)");
    const select = await built(root, "get_place", "select * from places where id = :id");
    const rows = await built(root, "insert_rows", "insert into places (id, home, spot, boxes) select id, home, spot, boxes from :places::rows(id int not null, home address, spot point, boxes box[])");
    await insert.run(client, PLACE);
    assert.deepEqual(await select.run(client, { id: 1 }), PLACE);
    assert.deepEqual(await select.run(sql, { id: 1 }), PLACE);
    // What the server holds is what the text says, not a JSON object's text.
    const held = (await client.query("select (home).street, (home).zip, (homes[1]).street as first, spot[1]::text as y, ring::text as ring from places")).rows;
    assert.deepEqual(held, [{ street: PLACE.home.street, zip: 12345, first: "x", y: "1e+300", ring: "<(1,2),3>" }]);
    await insert.run(sql, { ...PLACE, id: 2 });
    assert.deepEqual(await select.run(client, { id: 2 }), { ...PLACE, id: 2 });
    await rows.run(client, { places: [{ id: 3, home: PLACE.home, spot: PLACE.spot, boxes: PLACE.boxes }] });
    assert.deepEqual(await select.run(sql, { id: 3 }), { id: 3, home: PLACE.home, homes: null, spot: PLACE.spot, boxes: PLACE.boxes, route: null, shape: null, ring: null, edge: null, l: null });
    // A value of another shape is refused before the statement runs, naming the parameter and the field.
    await assert.rejects(insert.run(client, { ...PLACE, id: 4, spot: "(1,2)" }), /insert_place\.sql: Parameter "spot" is a point, \{ x: number; y: number \}; got "\(1,2\)"\.$/);
    await assert.rejects(insert.run(client, { ...PLACE, id: 4, home: { ...PLACE.home, zipp: 1 } }), /Parameter "home" is public\.address, which has no field "zipp"/);
    await assert.rejects(rows.run(client, { places: [{ id: 4, home: null, spot: { x: 1 }, boxes: null }] }), /a point, \{ x: number; y: number \}; got an object/);
    // A range, by its bounds; a discrete range comes back in Postgres's canonical form, [) on both.
    const period = await built(root, "insert_period", "insert into periods values (:id, :seats, :at, :price, :days, :many, :spans)");
    const getPeriod = await built(root, "get_period", "select * from periods where id = :id");
    await period.run(client, PERIOD);
    assert.deepEqual(await getPeriod.run(client, { id: 1 }), PERIOD);
    assert.deepEqual(await getPeriod.run(sql, { id: 1 }), PERIOD);
    await period.run(sql, { ...PERIOD, id: 2, seats: { lower: 1, upper: 5, bounds: "(]" } });
    assert.deepEqual((await getPeriod.run(client, { id: 2 })).seats, { lower: 2, upper: 6, bounds: "[)" });
    await period.run(client, { ...PERIOD, id: 3, seats: { lower: 1, upper: 5 } });
    assert.deepEqual((await getPeriod.run(client, { id: 3 })).seats, { lower: 1, upper: 5, bounds: "[)" });
    await assert.rejects(period.run(client, { ...PERIOD, id: 4, seats: "[1,5)" }), /Parameter "seats" is int4range, \{ empty: true \} or \{ lower, upper, bounds \}/);
  } finally {
    await client.end();
    await sql.end();
    for (const session of Session.opened()) await session.close();
    await own.drop();
  }
});

test("js.codecs reach a row value's fields and a range's bounds, read and bound", { skip: pgLive, timeout: 60_000 }, async () => {
  const own = await postgresDatabase("reylo_records_codecs", SCHEMA);
  const root = project(own.url, "js:\n  codecs:\n    instant: Temporal.Instant\n");
  const pg = driver("pg");
  const client = new pg.Client(own.url);
  await client.connect();
  try {
    const insert = await built(root, "insert_place", "insert into places (id, home, homes) values (:id, :home, :homes)");
    const select = await built(root, "get_place", "select home, homes from places where id = :id");
    const at = Temporal.Instant.from("2026-10-20T08:00:00.5Z");
    await insert.run(client, { id: 1, home: { ...PLACE.home, at }, homes: [{ ...PLACE.homes[0], at }] });
    const row = await select.run(client, { id: 1 });
    assert.ok(row.home.at instanceof Temporal.Instant);
    assert.ok(at.equals(row.home.at));
    assert.ok(at.equals(row.homes[0].at));
    // A range's bounds too.
    const period = await built(root, "insert_period", "insert into periods (id, at) values (:id, :at)");
    const getPeriod = await built(root, "get_period", "select at from periods where id = :id");
    await period.run(client, { id: 1, at: { lower: at, bounds: "[)" } });
    assert.ok(at.equals((await getPeriod.run(client, { id: 1 })).at.lower));
    // The codec's type alone: the default's text is refused.
    await assert.rejects(insert.run(client, { id: 2, home: { ...PLACE.home }, homes: null }), /Parameter "home" takes a Temporal.Instant/);
  } finally {
    await client.end();
    for (const session of Session.opened()) await session.close();
    await own.drop();
  }
});

test("a MySQL SET is a Set of its members on mysql2 and mariadb, and a parameter written to or compared with one takes a Set", { skip: mysqlLive, timeout: 60_000 }, async () => {
  const own = await mysqlDatabase("reylo_records_set", "create table tagged (id int primary key, tags set('a','b','c') not null)");
  const root = project(own.url, undefined, ["mysql2", "mariadb"]);
  const mysql2 = await driver("mysql2/promise").createConnection(own.url);
  const u = new URL(MYSQL!);
  const mariadb = await driver("mariadb").createConnection({ host: u.hostname, port: Number(u.port), user: decodeURIComponent(u.username) || "root", database: own.url.split("/").pop() });
  try {
    const insert = await built(root, "insert_tagged", "insert into tagged (id, tags) values (:id, :tags)");
    const select = await built(root, "get_tagged", "select id, tags from tagged where tags = :tags");
    // MySQL keeps each member once, in the order the column declares them.
    await insert.run(mysql2, { id: 1, tags: new Set(["c", "a"]) });
    await insert.run(mariadb, { id: 2, tags: new Set() });
    for (const client of [mysql2, mariadb]) {
      assert.deepEqual(await select.run(client, { tags: new Set(["a", "c"]) }), [{ id: 1, tags: new Set(["a", "c"]) }]);
      // MySQL compares a SET with the text as it is written, so a parameter is written in the column's order.
      assert.deepEqual(await select.run(client, { tags: new Set(["c", "a"]) }), [{ id: 1, tags: new Set(["a", "c"]) }]);
      assert.deepEqual(await select.run(client, { tags: new Set() }), [{ id: 2, tags: new Set() }]);
    }
  } finally {
    await mysql2.end();
    await mariadb.end();
    for (const session of Session.opened()) await session.close();
    await own.drop();
  }
});

test("a row value of no type the catalog has, and a built-in catalog table's row, are objects of their fields on every run, on pg, postgres.js, pg-promise and PGlite", { skip: pgLive, timeout: 60_000 }, async () => {
  const schema = "create table items (id int primary key, big int8, price numeric); insert into items values (1, 9007199254740993, 1.5), (2, 2, null)";
  const own = await postgresDatabase("reylo_records_anonymous", schema);
  const root = project(own.url, "js:\n  codecs:\n    decimal: number\n");
  const pg = driver("pg");
  const client = new pg.Client({ connectionString: own.url });
  await client.connect();
  const pool = new pg.Pool({ connectionString: own.url });
  const postgres = driver("postgres")(own.url);
  const pgp = driver("pg-promise")();
  const promised = pgp(own.url);
  const { PGlite } = driver("@electric-sql/pglite");
  const lite = new PGlite();
  await lite.exec(schema);
  const rows = [{ id: 1, big: 9007199254740993n, price: 1.5 }, { id: 2, big: 2n, price: null }];
  try {
    const each = await built(root, "each_row", "select s, 'plain' as note from (select id, big, price from items) s order by (s).id");
    const all = await built(root, "all_rows", "select array_agg(s order by (s).id) as many from (select id, big, price from items) s");
    const catalog = await built(root, "catalog_row", "select t from pg_catalog.pg_type t where t.typname = 'int4'");
    for (const db of [client, pool, postgres, promised, lite]) {
      // Twice: PGlite names a statement's oids from its earlier runs.
      for (let run = 0; run < 2; run++) {
        assert.deepEqual(await each.run(db), rows.map((s) => ({ s, note: "plain" })));
        assert.deepEqual(await all.run(db), { many: rows });
        assert.equal(((await catalog.run(db)) as any)[0].t.typname, "int4");
      }
    }
  } finally {
    await client.end();
    await pool.end();
    await postgres.end();
    pgp.end();
    await lite.close();
    for (const session of Session.opened()) await session.close();
    await own.drop();
  }
});

test("a MySQL ENUM column is the union of its labels, and the '' MySQL stores for any other value is refused, naming the column", { skip: mysqlLive, timeout: 60_000 }, async () => {
  const own = await mysqlDatabase("reylo_records_enum", "create table moods (id int primary key, mood enum('ok','sad') not null); insert into moods values (1, 'sad'); insert ignore into moods values (2, 'nope')");
  const root = project(own.url, undefined, ["mysql2", "mariadb"]);
  const mysql2 = await driver("mysql2/promise").createConnection(own.url);
  const u = new URL(MYSQL!);
  const mariadb = await driver("mariadb").createConnection({ host: u.hostname, port: Number(u.port), user: decodeURIComponent(u.username) || "root", database: own.url.split("/").pop() });
  try {
    const select = await built(root, "get_mood", "select mood from moods where id = :id");
    const cleared = await built(root, "get_cleared", "select nullif(mood, '') as mood from moods where id = :id");
    for (const client of [mysql2, mariadb]) {
      assert.deepEqual(await select.run(client, { id: 1 }), { mood: "sad" });
      await assert.rejects(select.run(client, { id: 2 }), /Column "mood" holds "", which is none of its ENUM's labels/);
      assert.deepEqual(await cleared.run(client, { id: 2 }), { mood: null });
    }
  } finally {
    await mysql2.end();
    await mariadb.end();
    for (const session of Session.opened()) await session.close();
    await own.drop();
  }
});
