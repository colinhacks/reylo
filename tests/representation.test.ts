import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { DatabaseSync } from "node:sqlite";
import { Session } from "../src/analyze.ts";
import { sqlModule } from "../src/loader.ts";
import { renderType } from "../src/render.ts";

// Reylo's one representation per SQL type (src/values.ts), on the real drivers, each set up by the
// app against it: the module a build step writes, run on each, must give the same rows, and each
// app's own query keeps the app's settings (the failing control). Postgres and MySQL run on
// throwaway servers (REYLO_TEST_PG, REYLO_TEST_MYSQL), each test in a database of its own; the
// drivers are lab/drivers'.
const PG = process.env.REYLO_TEST_PG;
const MYSQL = process.env.REYLO_TEST_MYSQL;
const DRIVERS = fileURLToPath(new URL("../lab/drivers/node_modules", import.meta.url));
const INDEX = pathToFileURL(fileURLToPath(new URL("../src/index.ts", import.meta.url))).href;
const drivers = existsSync(join(DRIVERS, "pg")) || "lab/drivers is not installed";
const pgLive = !PG ? "REYLO_TEST_PG names no server" : drivers === true ? false : drivers;
const mysqlLive = !MYSQL ? "REYLO_TEST_MYSQL names no server" : drivers === true ? false : drivers;
const require = createRequire(join(DRIVERS, "pg", "package.json"));

function project(...linked: string[]): string {
  const root = mkdtempSync(join(tmpdir(), "reylo-representation-"));
  writeFileSync(join(root, "package.json"), "{}");
  mkdirSync(join(root, "node_modules"));
  for (const driver of linked) symlinkSync(join(DRIVERS, driver), join(root, "node_modules", driver));
  return root;
}

/** The module a build step writes for `sql`, imported. */
async function built(root: string, url: string, name: string, sql: string) {
  const file = join(root, `${name}.sql`);
  writeFileSync(file, sql);
  writeFileSync(join(root, ".env"), `DATABASE_URL=${url}\n`);
  try {
    const { code } = await sqlModule(file, sql);
    const out = join(root, `${name}.mjs`);
    writeFileSync(out, code.replace(`from "reylo"`, `from ${JSON.stringify(INDEX)}`));
    return (await import(pathToFileURL(out).href)).default;
  } finally {
    for (const session of Session.opened()) await session.close();
  }
}

/** Rows compared by value: a driver's own row prototype and a Buffer's are not Reylo's to set. No value Reylo returns is a Date. */
const plain = (v: unknown): unknown => {
  if (v instanceof Date) throw new assert.AssertionError({ message: `a Date in Reylo's rows: ${v.toISOString()}` });
  return v instanceof Uint8Array ? { bytes: [...v] } : Array.isArray(v) ? v.map(plain) : v !== null && typeof v === "object" ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, plain(x)])) : v;
};

const PG_SCHEMA = `create type mood as enum ('ok', 'sad');
create domain posint as int check (value > 0);
create type pair as (a int, b text);
create table t (
  id int primary key, created_by int8, b bool, i2 int2, f4 float4, f8 float8, n numeric, d date, ts timestamp(6), tstz timestamptz, tm time, iv interval,
  j json, jb jsonb, by bytea, e mood, u uuid, p pair, r int4range,
  a_i4 int4[], a_i8 int8[], a_n numeric[], a_tx text[], a_e mood[], a_dom posint[], a_p pair[], a_ts timestamp[], a_tstz timestamptz[], a_by bytea[], a_iv interval[]
);
insert into t values (1, 9007199254740993, true, 2, 1.5, 2.2, 12345678901234567890.123456789, '2026-10-20', '2026-10-20 10:00:00.123456', '2026-10-20 10:00:00.123456+02', '10:00:00',
  '1 year 2 mons 3 days 04:05:06.789', '{"a":1}', '{"big":1}', '\\x0102ff', 'sad', 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11', '(1,"x,y")', '[1,5)',
  '{1,NULL,3}', '{9007199254740993}', '{1.5,12345678901234567890.1}', '{a,NULL,"NULL","b,c"}', '{ok,sad}', '{1,2}', array['(1,x)'::pair], '{"2026-10-20 10:00:00.5"}', '{"2026-10-20 10:00:00+00"}', '{"\\\\x01"}', '{"1 day"}'),
  (2, null, null, null, null, null, null, 'infinity', '0044-03-15 12:00:00 BC', '0001-01-01 00:00:00+00', null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null)`;
const PG_SELECT = "select * from t order by id";
const PG_ROWS = [
  {
    id: 1, created_by: 9007199254740993n, b: true, i2: 2, f4: 1.5, f8: 2.2, n: "12345678901234567890.123456789", d: "2026-10-20", ts: "2026-10-20T10:00:00.123456",
    tstz: "2026-10-20T08:00:00.123456Z", tm: "10:00:00", iv: "P1Y2M3DT4H5M6.789S", j: { a: 1 }, jb: { big: 1 }, by: { bytes: [1, 2, 255] }, e: "sad",
    u: "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11", p: '(1,"x,y")', r: "[1,5)", a_i4: [1, null, 3], a_i8: [9007199254740993n], a_n: ["1.5", "12345678901234567890.1"],
    a_tx: ["a", null, "NULL", "b,c"], a_e: ["ok", "sad"], a_dom: [1, 2], a_p: '{"(1,x)"}', a_ts: ["2026-10-20T10:00:00.5"], a_tstz: ["2026-10-20T10:00:00Z"], a_by: [{ bytes: [1] }], a_iv: ["P1D"],
  },
  {
    id: 2, created_by: null, b: null, i2: null, f4: null, f8: null, n: null, d: "infinity", ts: "0044-03-15T12:00:00 BC", tstz: "0001-01-01T00:00:00Z", tm: null, iv: null,
    j: null, jb: null, by: null, e: null, u: null, p: null, r: null, a_i4: null, a_i8: null, a_n: null, a_tx: null, a_e: null, a_dom: null, a_p: null, a_ts: null, a_tstz: null, a_by: null, a_iv: null,
  },
];

test("Postgres: one row on every driver, however the app set its driver up; the app's own queries keep its settings", { skip: pgLive, timeout: 120_000 }, async () => {
  const pg = require("pg");
  const postgres = require("postgres");
  const pgPromise = require("pg-promise");
  const { PGlite } = require("@electric-sql/pglite");
  const admin = new pg.Client(PG);
  await admin.connect();
  const name = `reylo_representation_${process.pid}`;
  await admin.query(`drop database if exists ${name}`);
  await admin.query(`create database ${name}`);
  const url = new URL(PG!);
  url.pathname = `/${name}`;
  const root = project("pg", "postgres");
  const parsers = { 20: pg.types.getTypeParser(20), 1700: pg.types.getTypeParser(1700) };
  try {
    const setup = new pg.Client(url.href);
    await setup.connect();
    await setup.query(PG_SCHEMA);
    await setup.end();
    const q = await built(root, url.href, "all_types", PG_SELECT);
    // pg, with a global int8 parser that returns a string and numeric to a float, a pool table and a connect listener.
    pg.types.setTypeParser(20, (x: string) => `${x}`);
    pg.types.setTypeParser(1700, parseFloat);
    const client = new pg.Client(url.href);
    await client.connect();
    assert.deepEqual(plain(await q.run(client)), PG_ROWS, "pg Client with a global int8 parser");
    const control = (await client.query("select created_by, tstz from t where id = 1")).rows[0];
    assert.equal(control.created_by, "9007199254740993", "the control: the app's own query, an int8 its text");
    assert.ok(control.tstz instanceof Date, "the control: pg's own timestamptz, a Date");
    await client.end();
    const pool = new pg.Pool({ connectionString: url.href, max: 2, types: { getTypeParser: (oid: number, f?: string) => (oid === 1184 ? (x: string) => `app:${x}` : pg.types.getTypeParser(oid, f)) } });
    pool.on("connect", (c: any) => c.setTypeParser(1082, (x: string) => `connect:${x}`));
    assert.deepEqual(plain(await q.run(pool)), PG_ROWS, "pg Pool");
    const tx = await pool.connect();
    await tx.query("begin");
    assert.deepEqual(plain(await q.run(tx)), PG_ROWS, "pg PoolClient in a transaction");
    await tx.query("rollback");
    tx.release();
    await pool.end();
    // binary: true, which pg 8 reads wrong, is refused.
    const binary = new pg.Client({ connectionString: url.href, binary: true });
    await binary.connect();
    await assert.rejects(q.run(binary), /binary: true/);
    await binary.end();
    // pg-promise: the database and a transaction; its receive event is refused.
    const pgp = pgPromise();
    const db = pgp(url.href);
    assert.deepEqual(plain(await q.run(db)), PG_ROWS, "pg-promise database");
    assert.deepEqual(plain(await db.tx((t: unknown) => q.run(t))), PG_ROWS, "pg-promise transaction");
    pgp.end();
    const camel = pgPromise({ receive: ({ data }: { data: Record<string, unknown>[] }) => data.forEach((r) => ((r.createdBy = r.created_by), delete r.created_by)) });
    const cdb = camel(url.href);
    await assert.rejects(q.run(cdb), /receive event rewrites every result/);
    camel.end();
    // postgres.js with BigInt and camelCase: Reylo's rows keep created_by, and its values.
    const sql = postgres(url.href, { types: { bigint: postgres.BigInt }, transform: postgres.camel, onnotice: () => {} });
    assert.deepEqual(plain(await q.run(sql)), PG_ROWS, "postgres.js with transform camel and types bigint");
    assert.deepEqual(plain(await sql.begin((t: unknown) => q.run(t))), PG_ROWS, "postgres.js transaction");
    const [own] = await sql`select created_by from t where id = 1`;
    assert.deepEqual(own, { createdBy: 9007199254740993n }, "the control: the app's own query");
    await sql.end();
    const rowTransform = postgres(url.href, { transform: { row: (r: unknown) => ({ wrapped: r }) }, onnotice: () => {} });
    await assert.rejects(q.run(rowTransform), /transform\.row rewrites every row/);
    await rowTransform.end();
    // PGlite, with the same schema and the app's own parsers on the instance.
    const lite = await PGlite.create({ parsers: { 20: (x: string) => Number(x), 1184: (x: string) => new Date(x) } });
    await lite.exec(PG_SCHEMA);
    assert.deepEqual(plain(await q.run(lite)), PG_ROWS, "PGlite");
    assert.deepEqual(plain(await q.run(lite)), PG_ROWS, "PGlite, a later run");
    assert.deepEqual(plain(await lite.transaction((t: unknown) => q.run(t))), PG_ROWS, "PGlite transaction");
    const liteControl = (await lite.query("select created_by, tstz from t where id = 1")).rows[0];
    assert.deepEqual([typeof liteControl.created_by, liteControl.tstz instanceof Date], ["number", true], "the control: the app's own query");
    await lite.close();
  } finally {
    pg.types.setTypeParser(20, parsers[20]);
    pg.types.setTypeParser(1700, parsers[1700]);
    await admin.query(`drop database if exists ${name} with (force)`);
    await admin.end();
  }
});

const MYSQL_SCHEMA = [
  "create table t (id int primary key, a datetime(3), a0 datetime, c date, d bigint unsigned, d2 bigint, e decimal(30,18), f json, g bit(1), h bit(8), i set('x','y','z'), j enum('a','b'), k tinyint(1), l year, n double, o varbinary(4), q time(2), s text)",
  "insert into t values (1, '2026-03-08 09:31:00.000', '2026-03-08 09:31:00', '2026-03-08', 18446744073709551615, -9007199254740993, 1.100000000000000001, '{\"k\": [1, 2]}', b'1', b'10000001', 'x,z', 'b', 1, 2026, 2.2, x'0102ff', '10:00:00.5', 'hi'), (2, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null)",
  // A TINYINT(1) holds any TINYINT: the failing control of a boolean column.
  "create table odd (id int primary key, ok boolean)",
  "insert into odd values (1, 2)",
];
const MYSQL_ROWS = [
  { id: 1, a: "2026-03-08T09:31:00.000", a0: "2026-03-08T09:31:00", c: "2026-03-08", d: 18446744073709551615n, d2: -9007199254740993n, e: "1.100000000000000001", f: { k: [1, 2] }, g: true, h: { bytes: [129] }, i: "x,z", j: "b", k: true, l: 2026, n: 2.2, o: { bytes: [1, 2, 255] }, q: "10:00:00.50", s: "hi", n2: 2n, first: true },
  { id: 2, a: null, a0: null, c: null, d: null, d2: null, e: null, f: null, g: null, h: null, i: null, j: null, k: null, l: null, n: null, o: null, q: null, s: null, n2: 2n, first: false },
];

test("MySQL: one row on mysql2 and mariadb, however the app set them up; mysql2's decimalNumbers gives Reylo the exact DECIMAL", { skip: mysqlLive, timeout: 120_000 }, async () => {
  const mysql = require("mysql2/promise");
  const mariadb = require("mariadb");
  const admin = await mysql.createConnection(MYSQL);
  const name = `reylo_representation_${process.pid}`;
  await admin.query(`drop database if exists ${name}`);
  await admin.query(`create database ${name}`);
  const url = new URL(MYSQL!);
  url.pathname = `/${name}`;
  const root = project("mysql2");
  try {
    const setup = await mysql.createConnection(url.href);
    for (const statement of MYSQL_SCHEMA) await setup.query(statement);
    await setup.end();
    const q = await built(root, url.href, "all_types", "select t.*, count(*) over () as n2, t.id = 1 as first from t order by id");
    const odd = await built(root, url.href, "odd", "select ok from odd");
    const refused = /Column "ok" is a boolean, stored as 0 or 1, and holds 2/;
    const hostile = mysql.createPool({ uri: url.href, decimalNumbers: true, jsonStrings: true, rowsAsArray: true, supportBigNumbers: false, dateStrings: ["DATE"], typeCast: (f: { type: string }, next: () => unknown) => (f.type === "TINY" ? next() === 1 : next()) });
    assert.deepEqual(plain(await q.run(hostile)), MYSQL_ROWS, "mysql2 pool with decimalNumbers, jsonStrings, rowsAsArray, supportBigNumbers off and a typeCast");
    const [[own]] = await hostile.query("select e, d2 from t where id = 1");
    assert.deepEqual(own, [1.1, -9007199254740992], "the control: the app's own query, lossy");
    await assert.rejects(odd.run(hostile), refused);
    assert.deepEqual((await hostile.query("select ok from odd"))[0], [[false]], "the control: the app's own typeCast reads the 2 as false");
    const conn = await hostile.getConnection();
    await conn.beginTransaction();
    assert.deepEqual(plain(await q.run(conn)), MYSQL_ROWS, "mysql2 PoolConnection in a transaction");
    await conn.rollback();
    conn.release();
    await hostile.end();
    const pool = mariadb.createPool({ host: url.hostname, port: Number(url.port), user: url.username || "root", password: decodeURIComponent(url.password), database: name, bigIntAsNumber: true, decimalAsNumber: true, jsonStrings: true, rowsAsArray: true, metaAsArray: true, connectionLimit: 2 });
    assert.deepEqual(plain(await q.run(pool)), MYSQL_ROWS, "mariadb pool with bigIntAsNumber, decimalAsNumber, jsonStrings, rowsAsArray and metaAsArray");
    await assert.rejects(odd.run(pool), refused);
    await pool.end();
  } finally {
    await admin.query(`drop database if exists ${name}`);
    await admin.end();
  }
});

test("SQLite: one row on every driver, whatever its integer setting; an integer past 2^53, a BOOLEAN column's 2 and a value its column's type does not allow an error naming its column", { skip: drivers === true ? false : drivers, timeout: 60_000 }, async () => {
  const Database = require("better-sqlite3");
  const { createClient } = require("@libsql/client");
  const sqlite3 = require("sqlite3");
  const { open } = require("sqlite");
  const root = project();
  const file = join(root, "app.db");
  const setup = new DatabaseSync(file);
  setup.exec(`create table t (id integer primary key, n integer not null, r real, s text, b blob, flag boolean, x, z numeric);
    insert into t values (1, 5, 1.5, 'hi', x'0102ff', 1, 7, 2.5), (2, 6, null, null, null, 0, x'01', 'abc');
    create table big (id integer primary key, v integer); insert into big values (1, 9007199254740993);
    create table odd (id integer primary key, ok boolean); insert into odd values (1, 2);
    create table loose (id integer primary key, n integer, s text); insert into loose values (1, 'abc', x'0102');
    create table tight (id integer primary key, n integer not null) strict; insert into tight values (1, 5)`);
  setup.close();
  // A column declared BOOLEAN and a comparison are booleans.
  const q = await built(root, "file:./app.db", "all_types", "select *, n > 5 as more from t order by id");
  const big = await built(root, "file:./app.db", "big", "select id, v from big");
  const odd = await built(root, "file:./app.db", "odd", "select ok from odd");
  // A table that is not STRICT stores the text 'abc' in an INTEGER column and a BLOB in a TEXT one.
  const looseN = await built(root, "file:./app.db", "loose_n", "select n from loose");
  const looseS = await built(root, "file:./app.db", "loose_s", "select s from loose");
  const textInN = /Column "n" is typed number and holds the text "abc"/;
  const blobInS = /Column "s" is typed string and holds a BLOB of 2 bytes/;
  // A value that is one of several kinds is typed unknown, so nothing is refused over a STRICT table.
  const mixed = await built(root, "file:./app.db", "mixed", "select n as v from tight union all select 'x'");
  // libSQL's intMode "string" hands the 2 over as its digits.
  const refused = /Column "ok" is a boolean, stored as 0 or 1, and holds "?2"?, which is neither/;
  const rows = [
    { id: 1, n: 5, r: 1.5, s: "hi", b: { bytes: [1, 2, 255] }, flag: true, x: 7, z: 2.5, more: false },
    { id: 2, n: 6, r: null, s: null, b: null, flag: false, x: { bytes: [1] }, z: "abc", more: true },
  ];
  const bigError = /Column "v" holds .*9007199254740992|Column "v" holds 9007199254740993/;
  // better-sqlite3 with every integer a BigInt: Reylo's statement reads numbers, the app's keep its setting.
  const db = new Database(file);
  db.defaultSafeIntegers(true);
  assert.deepEqual(plain(q.runSync(db)), rows, "better-sqlite3 with defaultSafeIntegers(true)");
  assert.deepEqual(plain(db.transaction(() => q.runSync(db))()), rows, "better-sqlite3 in db.transaction");
  assert.equal(typeof db.prepare("select n from t").get().n, "bigint", "the control: the app's own statement");
  assert.throws(() => big.runSync(db), bigError);
  assert.throws(() => odd.runSync(db), refused);
  assert.equal(db.prepare("select ok from odd").get().ok, 2n, "the control: SQLite lets the 2 in");
  assert.throws(() => looseN.runSync(db), textInN);
  assert.throws(() => looseS.runSync(db), blobInS);
  assert.equal(db.prepare("select n from loose").get().n, "abc", "the control: SQLite lets the text in");
  assert.deepEqual(plain(mixed.runSync(db)), [{ v: 5 }, { v: "x" }], "better-sqlite3: a STRICT table's union of a number and text");
  db.close();
  const node = new DatabaseSync(file, { readBigInts: true });
  assert.deepEqual(plain(await q.run(node)), rows, "node:sqlite with readBigInts");
  await assert.rejects(big.run(node), bigError);
  await assert.rejects(odd.run(node), refused);
  await assert.rejects(looseN.run(node), textInN);
  await assert.rejects(looseS.run(node), blobInS);
  assert.deepEqual(plain(await mixed.run(node)), [{ v: 5 }, { v: "x" }], "node:sqlite: a STRICT table's union of a number and text");
  node.close();
  for (const intMode of ["number", "bigint", "string"]) {
    const client = createClient({ url: `file:${file}`, intMode });
    // Under intMode "string" an integer in a column with no type is its digits (wiki/driver-decoding.md).
    if (intMode !== "string") assert.deepEqual(plain(await q.run(client)), rows, `libSQL with intMode ${intMode}`);
    await assert.rejects(odd.run(client), refused);
    await assert.rejects(looseN.run(client), textInN);
    await assert.rejects(looseS.run(client), blobInS);
    client.close();
  }
  // Turso's new engine on the same file: its promise API through run, in transactionAsync too, and
  // its compat Database through runSync. At its defaults it rounds an integer past 2^53 silently.
  const turso = await import(pathToFileURL(require.resolve("@tursodatabase/database")).href);
  const { Database: TursoCompat } = await import(pathToFileURL(require.resolve("@tursodatabase/database/compat")).href);
  for (const safe of [false, true]) {
    const tdb = await turso.connect(file);
    tdb.defaultSafeIntegers(safe);
    assert.deepEqual(plain(await q.run(tdb)), rows, `Turso's engine with defaultSafeIntegers(${safe})`);
    assert.deepEqual(plain(await tdb.transactionAsync((tx: unknown) => q.run(tx))()), rows, "Turso's engine in transactionAsync");
    await assert.rejects(big.run(tdb), bigError);
    await assert.rejects(odd.run(tdb), refused);
    await assert.rejects(looseN.run(tdb), textInN);
    await assert.rejects(looseS.run(tdb), blobInS);
    assert.equal((await tdb.prepare("select v from big").get()).v, safe ? 9007199254740993n : 9007199254740992, "the control: the app's own statement");
    await tdb.close();
  }
  const compat = new TursoCompat(file);
  assert.deepEqual(plain(q.runSync(compat)), rows, "Turso's compat Database");
  assert.deepEqual(plain(compat.transaction(() => q.runSync(compat))()), rows, "Turso's compat Database in db.transaction");
  assert.throws(() => big.runSync(compat), bigError);
  assert.throws(() => odd.runSync(compat), refused);
  assert.throws(() => looseN.runSync(compat), textInN);
  assert.throws(() => looseS.runSync(compat), blobInS);
  compat.close();
  const s3 = new sqlite3.Database(file);
  assert.deepEqual(plain(await q.run(s3)), rows, "sqlite3");
  await assert.rejects(big.run(s3), /Column "v" holds an integer past .* already rounded to 9007199254740992/);
  await assert.rejects(odd.run(s3), refused);
  await assert.rejects(looseN.run(s3), textInN);
  await assert.rejects(looseS.run(s3), blobInS);
  await new Promise((resolve) => s3.close(resolve));
  const wrapper = await open({ filename: file, driver: sqlite3.Database });
  assert.deepEqual(plain(await q.run(wrapper)), rows, "sqlite");
  await assert.rejects(looseN.run(wrapper), textInN);
  await assert.rejects(looseS.run(wrapper), blobInS);
  await wrapper.close();
});

test("SQLite: libSQL's intMode \"string\" changes no value Reylo types a number or a NUMERIC's: an INTEGER is its number, exact up to 2^53 - 1 and refused past it; a column with no type keeps libSQL's digits", { skip: drivers === true ? false : drivers, timeout: 60_000 }, async () => {
  const { createClient } = require("@libsql/client");
  const root = project();
  const file = join(root, "app.db");
  const setup = new DatabaseSync(file);
  // `ok + 0` is a REAL's expression on a column declared BOOLEAN, and what SQLite computes from the
  // stored 2 is the INTEGER 2; `z` is a NUMERIC column, which stores 7 and 9007199254740991 as
  // INTEGERs and 2.5 and 'abc' as they are; `x` has no type.
  setup.exec(`create table t (id integer primary key, ok boolean, z numeric, x);
    insert into t values (1, 2, 7, 7), (2, 2, 2.5, 'abc'), (3, 2, 'abc', '7'), (4, 2, 9007199254740991, null);
    create table past (id integer primary key, z numeric); insert into past values (1, 9007199254740992)`);
  setup.close();
  const q = await built(root, "file:./app.db", "computed", "select id, ok + 0 as n, ok + 9007199254740989 as edge, z, x from t order by id");
  const over = await built(root, "file:./app.db", "computed_over", "select ok + 9007199254740990 as edge from t where id = 1");
  const past = await built(root, "file:./app.db", "numeric_past", "select z from past");
  for (const intMode of ["number", "bigint", "string"]) {
    const client = createClient({ url: `file:${file}`, intMode });
    const digits = intMode === "string";
    assert.deepEqual(
      plain(await q.run(client)),
      [
        // A column with no type cannot tell libSQL's digits from a TEXT's: it keeps what libSQL gives.
        { id: 1, n: 2, edge: 9007199254740991, z: 7, x: digits ? "7" : 7 },
        { id: 2, n: 2, edge: 9007199254740991, z: 2.5, x: "abc" },
        { id: 3, n: 2, edge: 9007199254740991, z: "abc", x: "7" },
        { id: 4, n: 2, edge: 9007199254740991, z: 9007199254740991, x: null },
      ],
      `libSQL with intMode ${intMode}`,
    );
    // One past 2^53 - 1: libSQL's intMode "number" refuses it itself; the others hand it over exact, and Reylo refuses it.
    const refused = intMode === "number" ? RangeError : /Column "edge" holds 9007199254740992, an integer past/;
    await assert.rejects(over.run(client), refused, `libSQL with intMode ${intMode}: a REAL's expression past 2^53 - 1`);
    await assert.rejects(past.run(client), intMode === "number" ? RangeError : /Column "z" holds 9007199254740992, an integer past/, `libSQL with intMode ${intMode}: a NUMERIC past 2^53 - 1`);
    client.close();
  }
});

test("SQLite: a column declared BIGINT or INT8 is a BigInt on every driver, exact past 2^53 or refused where the driver rounded it, an INTEGER a number; its value goes back in as a parameter", { skip: drivers === true ? false : drivers, timeout: 60_000 }, async () => {
  const Database = require("better-sqlite3");
  const { createClient } = require("@libsql/client");
  const sqlite3 = require("sqlite3");
  const { open } = require("sqlite");
  const root = project();
  const file = join(root, "app.db");
  const setup = new DatabaseSync(file);
  setup.exec(`create table acct (id integer primary key, n integer not null, v bigint, w Int8);
    insert into acct values (1, 5, 9007199254740993, 7), (2, 6, 5, null)`);
  setup.close();
  const all = await built(root, "file:./app.db", "acct_all", "select id, n, v, w, max(v, 0) as most, v + 0 as plus from acct group by id order by id");
  const byV = await built(root, "file:./app.db", "acct_by_v", "select id, v from acct where v = :v");
  const rows = [
    { id: 1, n: 5, v: 9007199254740993n, w: 7n, most: 9007199254740993n, plus: 9007199254740993 },
    { id: 2, n: 6, v: 5n, w: null, most: 5n, plus: 5 },
  ];
  // `v + 0` is an integer SQLite computes, a number, so row 1's is refused; the rest of the columns read alone.
  const plusError = /Column "plus" holds 9007199254740993, an integer past/;
  const exact = await built(root, "file:./app.db", "acct_exact", "select id, n, v, w, max(v, 0) as most from acct order by id");
  const exactRows = rows.map(({ plus: _, ...r }) => r);
  const rounded = /Column "v" is a 64-bit integer, and the driver already rounded its value to 9007199254740992/;
  // better-sqlite3 at its default, every integer a number: Reylo's statement reads the BIGINT
  // column exact, and the INTEGER column as a number, whatever the database says.
  for (const safe of [false, true]) {
    const db = new Database(file);
    db.defaultSafeIntegers(safe);
    assert.deepEqual(plain(exact.runSync(db)), exactRows, `better-sqlite3 with defaultSafeIntegers(${safe})`);
    assert.throws(() => all.runSync(db), plusError);
    const own = db.prepare("select n, v from acct where id = 1").get();
    assert.deepEqual(own, safe ? { n: 5n, v: 9007199254740993n } : { n: 5, v: 9007199254740992 }, "the control: the app's own statement keeps its setting, and rounds v without safeIntegers");
    // What `run` returns goes back in.
    assert.deepEqual(plain(byV.runSync(db, { v: exactRows[0]!.v })), [{ id: 1, v: 9007199254740993n }], "better-sqlite3: the BIGINT's value as a parameter");
    db.close();
  }
  for (const readBigInts of [false, true]) {
    const node = new DatabaseSync(file, { readBigInts });
    assert.deepEqual(plain(await exact.run(node)), exactRows, `node:sqlite with readBigInts ${readBigInts}`);
    await assert.rejects(all.run(node), plusError);
    assert.deepEqual(plain(await byV.run(node, { v: 9007199254740993n })), [{ id: 1, v: 9007199254740993n }], "node:sqlite: the BIGINT's value as a parameter");
    node.close();
  }
  for (const intMode of ["number", "bigint", "string"]) {
    const client = createClient({ url: `file:${file}`, intMode });
    // At its default intMode libSQL refuses an integer past 2^53 itself, before Reylo sees it.
    if (intMode === "number") await assert.rejects(exact.run(client), RangeError);
    else assert.deepEqual(plain(await exact.run(client)), exactRows, `libSQL with intMode ${intMode}`);
    assert.deepEqual(plain(await byV.run(client, { v: 5n })), [{ id: 2, v: 5n }], `libSQL with intMode ${intMode}: a BIGINT within 2^53`);
    if (intMode !== "number") assert.deepEqual(plain(await byV.run(client, { v: 9007199254740993n })), [{ id: 1, v: 9007199254740993n }], `libSQL with intMode ${intMode}: the BIGINT's value as a parameter`);
    client.close();
  }
  const turso = await import(pathToFileURL(require.resolve("@tursodatabase/database")).href);
  const { Database: TursoCompat } = await import(pathToFileURL(require.resolve("@tursodatabase/database/compat")).href);
  for (const safe of [false, true]) {
    const tdb = await turso.connect(file);
    tdb.defaultSafeIntegers(safe);
    assert.deepEqual(plain(await exact.run(tdb)), exactRows, `Turso's engine with defaultSafeIntegers(${safe})`);
    assert.deepEqual(plain(await byV.run(tdb, { v: 9007199254740993n })), [{ id: 1, v: 9007199254740993n }], "Turso's engine: the BIGINT's value as a parameter");
    assert.equal((await tdb.prepare("select v from acct where id = 1").get()).v, safe ? 9007199254740993n : 9007199254740992, "the control: the app's own statement");
    await tdb.close();
  }
  const compat = new TursoCompat(file);
  assert.deepEqual(plain(exact.runSync(compat)), exactRows, "Turso's compat Database");
  compat.close();
  // sqlite3 and sqlite round past 2^53 before Reylo sees the value: refused, naming the column; a
  // value within it is a BigInt, and goes back in (a larger one is refused before the statement runs).
  const s3 = new sqlite3.Database(file);
  await assert.rejects(exact.run(s3), rounded);
  assert.deepEqual(plain(await byV.run(s3, { v: 5n })), [{ id: 2, v: 5n }], "sqlite3: a BIGINT within 2^53, as a parameter and back");
  await new Promise((resolve) => s3.close(resolve));
  const wrapper = await open({ filename: file, driver: sqlite3.Database });
  await assert.rejects(exact.run(wrapper), rounded);
  assert.deepEqual(plain(await byV.run(wrapper, { v: 5n })), [{ id: 2, v: 5n }], "sqlite: a BIGINT within 2^53");
  await wrapper.close();
});

test("SQLite: JSON a statement builds is the object its type says on every driver, and a text column that holds JSON stays a string", { skip: drivers === true ? false : drivers, timeout: 60_000 }, async () => {
  const Database = require("better-sqlite3");
  const { createClient } = require("@libsql/client");
  const sqlite3 = require("sqlite3");
  const { open } = require("sqlite");
  const root = project();
  const file = join(root, "app.db");
  const setup = new DatabaseSync(file);
  setup.exec(`create table users (id integer primary key, name text not null, github_id bigint, score real, active boolean not null default 1, doc text);
    create table posts (id integer primary key, user_id integer not null references users (id), title text not null, body text) strict;
    insert into users values (1, 'ann', 9007199254740993, 1.5, 1, '[1]'), (2, 'bob', null, null, 0, null);
    insert into posts values (10, 1, 'hello', null), (11, 1, 'world', 'b')`);
  setup.close();
  const sql = {
    profile: `select json_object(
  'user', json_object('id', u.id, 'name', u.name),
  'posts', json((select json_group_array(json_object('id', p.id, 'title', p.title)) from posts p where p.user_id = u.id))
) as profile
from users u where u.id = :id::int`,
    // A BIGINT inside JSON is a JSON number, which JSON.parse rounds past 2^53, as on Postgres;
    // a truth value is 1 or 0 there, and a BOOLEAN column what it holds.
    values: `select json_object('big', github_id, 'score', score, 'flag', active, 'more', id > 1, 'pair', json_array(name, score), 'doc', doc) as o,
  doc, json_object('n', name) || '' as glued from users order by id`,
  };
  // The types the module declares, as the type check renders them.
  writeFileSync(join(root, ".env"), "DATABASE_URL=file:./app.db\n");
  const session = await Session.for(root);
  try {
    const types: Record<string, string> = {};
    for (const [name, text] of Object.entries(sql)) {
      const outcome = await session.analyze(text);
      assert.ok(outcome.ok, `${name}: ${outcome.ok ? "" : outcome.message}`);
      types[name] = outcome.analysis.columns.map((c) => `${c.name}: ${renderType(c.type, false, "sqlite")}${c.nullable ? " | null" : ""}`).join("; ");
    }
    assert.deepEqual(types, {
      profile: "profile: { user: { id: number; name: string }; posts: { id: number; title: string }[] }",
      values: "o: { big: number | null; score: number | null; flag: number | string; more: number; pair: [string, number | null]; doc: string | null }; doc: string | null; glued: string",
    });
  } finally {
    await session.close();
  }
  const profile = await built(root, "file:./app.db", "profile", sql.profile);
  const values = await built(root, "file:./app.db", "values", sql.values);
  const ann = { profile: { user: { id: 1, name: "ann" }, posts: [{ id: 10, title: "hello" }, { id: 11, title: "world" }] } };
  const bob = { profile: { user: { id: 2, name: "bob" }, posts: [] } };
  const rows = [
    { o: { big: 9007199254740992, score: 1.5, flag: 1, more: 0, pair: ["ann", 1.5], doc: "[1]" }, doc: "[1]", glued: '{"n":"ann"}' },
    { o: { big: null, score: null, flag: 0, more: 1, pair: ["bob", null], doc: null }, doc: null, glued: '{"n":"bob"}' },
  ];
  const check = async (label: string, run: (q: any, params?: object) => unknown) => {
    assert.deepEqual(plain(await run(profile, { id: 1 })), ann, `${label}: the profile`);
    assert.deepEqual(plain(await run(profile, { id: 2 })), bob, `${label}: a user with no posts`);
    assert.deepEqual(plain(await run(values)), rows, `${label}: the values`);
  };
  for (const safe of [false, true]) {
    const db = new Database(file);
    db.defaultSafeIntegers(safe);
    await check(`better-sqlite3 with defaultSafeIntegers(${safe})`, (q, p) => q.runSync(db, p));
    assert.equal(typeof db.prepare("select json_object('a', 1) as o").get().o, "string", "the control: the app's own statement");
    db.close();
  }
  for (const readBigInts of [false, true]) {
    const node = new DatabaseSync(file, { readBigInts });
    await check(`node:sqlite with readBigInts ${readBigInts}`, (q, p) => q.run(node, p));
    node.close();
  }
  for (const intMode of ["number", "bigint", "string"]) {
    const client = createClient({ url: `file:${file}`, intMode });
    await check(`libSQL with intMode ${intMode}`, (q, p) => q.run(client, p));
    client.close();
  }
  const turso = await import(pathToFileURL(require.resolve("@tursodatabase/database")).href);
  const { Database: TursoCompat } = await import(pathToFileURL(require.resolve("@tursodatabase/database/compat")).href);
  const tdb = await turso.connect(file);
  await check("Turso's engine", (q, p) => q.run(tdb, p));
  await tdb.close();
  const compat = new TursoCompat(file);
  await check("Turso's compat Database", (q, p) => q.runSync(compat, p));
  compat.close();
  const s3 = new sqlite3.Database(file);
  await check("sqlite3", (q, p) => q.run(s3, p));
  await new Promise((resolve) => s3.close(resolve));
  const wrapper = await open({ filename: file, driver: sqlite3.Database });
  await check("sqlite", (q, p) => q.run(wrapper, p));
  await wrapper.close();
});
