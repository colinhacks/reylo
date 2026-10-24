// Reylo decodes every value of its own Postgres queries itself (src/decode.ts), against real
// drivers each set up by the app against it: pg's Client, Pool and a
// PoolClient in a transaction, pg-promise, Neon's Pool and Client through a local stand-in for its
// WebSocket proxy, PGlite, and postgres.js. Each runs the same statement through Reylo's `run` and
// directly (the control, which shows the app's own decoding, its parsers and the driver's), and the
// app's own queries must keep the app's decoding after Reylo's.
// Needs a Postgres server (DECODE_SERVER, default postgres://localhost:5432; the database
// porg_decode is created and dropped there): DECODE_SERVER=postgres://localhost:5432 nub decode.mjs
import net from "node:net";
import { createRequire } from "node:module";
import { isDeepStrictEqual, inspect } from "node:util";
import pg from "pg";
import pgPromise from "pg-promise";
import postgres from "postgres";
import { PGlite } from "@electric-sql/pglite";
import { defineQuery } from "../../src/index.ts";

const require = createRequire(import.meta.url);
const server = process.env.DECODE_SERVER ?? "postgres://localhost:5432";
const URL = `${server}/porg_decode`;
// Where the Neon stand-in below forwards its WebSocket traffic: the same server.
const { hostname: serverHost, port: serverPort, username: serverUser } = new globalThis.URL(server);

const schema = `
  create type role as enum ('admin', 'member');
  create domain posint as int check (value > 0);
  create type pair as (a int, b text);`;
const sql = `select array['admin', 'member', null]::role[] as roles, array['a', 'b']::name[] as names,
  array[1, 2]::posint[] as posints, array[row(1, 'x'), row(2, 'y,z')]::pair[] as pairs,
  interval '1 year 2 mons 3 days 04:05:06.789' as span, array[interval '1 day', interval '2 hours'] as spans,
  array[1.5, 12345678901234567890.1]::numeric[] as amounts, 9007199254740993::int8 as big,
  timestamptz '2024-01-15 10:30:00.123+00' as at, timestamp '2024-01-15 10:30:00.123456' as wall, date '2024-01-15' as day, :id::int as id`;
// A query defined by hand: the types the database created come from one catalog query per client.
const q = defineQuery({ dialect: "postgres", sql, params: ["id"], cardinality: "one" });
// The same with the columns a build step writes (src/loader.ts): no catalog query.
const built = defineQuery({ dialect: "postgres", sql, params: ["id"], cardinality: "one", columns: [["roles", "text[]"], ["names", "text[]"], ["posints", "int[]"], ["pairs", "text"], ["span", "text"], ["spans", "text[]"], ["amounts", "decimal[]"], ["big", "int8"], ["at", "timestamptz"], ["wall", "timestamp"], ["day", "date"], ["id", "int"]] });
const direct = sql.replace(":id", "$1");

// What Reylo returns for each column (src/values.ts): one representation per type on every driver.
const declared = {
  roles: ["admin", "member", null],
  names: ["a", "b"],
  posints: [1, 2],
  pairs: '{"(1,x)","(2,\\"y,z\\")"}',
  span: "P1Y2M3DT4H5M6.789S",
  spans: ["P1D", "PT2H"],
  amounts: ["1.5", "12345678901234567890.1"],
  big: 9007199254740993n,
  at: "2024-01-15T10:30:00.123Z",
  wall: "2024-01-15T10:30:00.123456",
  day: "2024-01-15",
  id: 1,
};

let failures = 0;
const show = (v) => inspect(v, { breakLength: Infinity, depth: 4 });
function check(label, ok, detail = "") {
  if (!ok) failures++;
  console.log(`${ok ? "ok  " : "FAIL"} ${label}${detail ? `: ${detail}` : ""}`);
}
function same(label, actual, expected) {
  const diff = Object.keys(expected).filter((k) => !isDeepStrictEqual(actual?.[k], expected[k]));
  check(label, diff.length === 0, diff.length ? diff.map((k) => `${k}=${show(actual?.[k])} (expected ${show(expected[k])})`).join("; ") : "");
}
/** Both forms of the query: by hand and as built. */
async function both(label, client) {
  same(`${label} through run`, await q.run(client, { id: 1 }), declared);
  same(`${label} through run, as built`, await built.run(client, { id: 1 }), declared);
}

const admin = new pg.Client(`${server}/postgres`);
await admin.connect();
await admin.query("drop database if exists porg_decode");
await admin.query("create database porg_decode");
try {
  const setup = new pg.Client(URL);
  await setup.connect();
  await setup.query(schema);
  await setup.end();

  // pg's global table, set up by the app: an int8 its text, a numeric a float, a timestamptz tagged.
  const defaults = { 20: pg.types.getTypeParser(20), 1700: pg.types.getTypeParser(1700), 1184: pg.types.getTypeParser(1184) };
  pg.types.setTypeParser(20, String);
  pg.types.setTypeParser(1700, parseFloat);
  pg.types.setTypeParser(1184, (x) => `global:${x}`);

  // pg Client, with a client parser and an enum parser too.
  {
    const client = new pg.Client(URL);
    await client.connect();
    client.setTypeParser(1082, (x) => `client:${x}`);
    const control = (await client.query(direct, [1])).rows[0];
    check("pg Client control (no reylo): the app's parsers", control.big === "9007199254740993" && String(control.at).startsWith("global:") && control.day === "client:2024-01-15" && control.roles === "{admin,member,NULL}" && typeof control.span === "object", show(control));
    await both("pg Client", client);
    same("pg Client, the app's own query after reylo's", (await client.query(direct, [1])).rows[0], control);
    // A type created after reylo read the catalog: decoded after one lookup of its oid.
    await client.query("create type late as enum ('x', 'y')");
    const late = defineQuery({ dialect: "postgres", sql: "select array['x', 'y']::late[] as lates, 'x'::late as one", params: [], cardinality: "one" });
    same("pg Client: an enum created after the catalog was read", await late.run(client), { lates: ["x", "y"], one: "x" });
    await client.end();
    const binary = new pg.Client({ connectionString: URL, binary: true });
    await binary.connect();
    const refused = await q.run(binary, { id: 1 }).then(() => null, (e) => e);
    check("pg Client with binary: true is refused", /binary: true/.test(refused?.message), String(refused?.message));
    await binary.end();
  }

  // pg Pool with a pool table and a connect listener, and a PoolClient in a transaction.
  {
    const pool = new pg.Pool({ connectionString: URL, max: 2, types: { getTypeParser: (oid, format) => (oid === 1114 ? (x) => `pool:${x}` : pg.types.getTypeParser(oid, format)) } });
    pool.on("connect", (c) => c.setTypeParser(1082, (x) => `connect:${x}`));
    const control = (await pool.query(direct, [1])).rows[0];
    check("pg Pool control (no reylo): the pool's and the listener's parsers", control.wall === "pool:2024-01-15 10:30:00.123456" && control.day === "connect:2024-01-15", show(control));
    await both("pg Pool", pool);
    // As pool.query does, a failing statement releases the client with its error.
    const failing = defineQuery({ dialect: "postgres", sql: "select 1 / 0 as x", params: [], cardinality: "one" });
    const error = await failing.run(pool).then(() => null, (e) => e);
    check("pg Pool: the driver's error comes through", error?.code === "22012", String(error?.message));
    const tx = await pool.connect();
    await tx.query("begin");
    await tx.query("create type in_tx as enum ('p', 'q')");
    await both("pg PoolClient inside a transaction", tx);
    const inTx = defineQuery({ dialect: "postgres", sql: "select array['p']::in_tx[] as v", params: [], cardinality: "one" });
    same("pg PoolClient: an enum created inside the transaction", await inTx.run(tx), { v: ["p"] });
    await tx.query("rollback");
    same("pg PoolClient, the app's own query after reylo's", (await tx.query(direct, [1])).rows[0], control);
    tx.release();
    await pool.end();
  }

  // pg-promise: the database, a task and a transaction; its receive event is refused.
  {
    const pgp = pgPromise();
    const db = pgp(URL);
    const control = await db.one({ text: direct, values: [1] });
    await both("pg-promise database", db);
    same("pg-promise task through run", await db.task((t) => q.run(t, { id: 1 })), declared);
    same("pg-promise transaction through run", await db.tx((t) => built.run(t, { id: 1 })), declared);
    same("pg-promise, the app's own query after reylo's", await db.one({ text: direct, values: [1] }), control);
    pgp.end();
    const camel = pgPromise({ receive: ({ data }) => data.forEach((r) => ((r.Roles = r.roles), delete r.roles)) });
    const cdb = camel(URL);
    const refused = await q.run(cdb, { id: 1 }).then(() => null, (e) => e);
    check("pg-promise with a receive event is refused", /receive event/.test(refused?.message), String(refused?.message));
    camel.end();
  }

  // postgres.js with BigInt and camelCase: .raw() and Reylo's parsers, the module's names.
  {
    const sql = postgres(URL, { types: { bigint: postgres.BigInt }, transform: postgres.camel, onnotice: () => {} });
    const [control] = await sql.unsafe(direct, [1]);
    check("postgres.js control (no reylo): BigInt, and a NULL element read as the text NULL", control.big === 9007199254740993n && control.roles[2] === "NULL", show(control));
    await both("postgres.js", sql);
    same("postgres.js transaction through run", await sql.begin((tx) => built.run(tx, { id: 1 })), declared);
    same("postgres.js, the app's own query after reylo's", (await sql.unsafe(direct, [1]))[0], control);
    await sql.end();
  }

  // Neon's Pool and Client, through a local stand-in for Neon's WebSocket proxy (as the 2026-10-02 study ran it).
  {
    const { WebSocketServer } = require("./node_modules/.store/ws@8.22.0/node_modules/ws");
    const wss = new WebSocketServer({ port: 18433 });
    wss.on("connection", (ws) => {
      const sock = net.connect(Number(serverPort || 5432), serverHost);
      ws.on("message", (d) => sock.write(d));
      sock.on("data", (d) => ws.send(d));
      ws.on("close", () => sock.end());
      sock.on("close", () => ws.close());
      sock.on("error", () => ws.close());
    });
    const neon = await import("@neondatabase/serverless");
    neon.neonConfig.wsProxy = (host, port) => `localhost:18433/v2?address=${host}:${port}`;
    neon.neonConfig.useSecureWebSocket = false;
    neon.neonConfig.pipelineConnect = false;
    neon.neonConfig.webSocketConstructor = globalThis.WebSocket;
    const neonUrl = `postgres://${serverUser || process.env.USER}@${serverHost}:${serverPort || 5432}/porg_decode`;
    // Neon's global table is its own copy of pg-types.
    neon.types.setTypeParser(1184, (x) => `neon-global:${x}`);
    const pool = new neon.Pool({ connectionString: neonUrl });
    const control = (await pool.query(direct, [1])).rows[0];
    check("Neon Pool control (no reylo): Neon's global parser", String(control.at).startsWith("neon-global:"), show(control.at));
    await both("Neon Pool", pool);
    same("Neon Pool, the app's own query after reylo's", (await pool.query(direct, [1])).rows[0], control);
    const client = new neon.Client({ connectionString: neonUrl });
    await client.connect();
    await both("Neon Client", client);
    await client.end();
    await pool.end();
    wss.close();
  }

  // PGlite with the app's parsers on the instance, a later run and a transaction.
  {
    const lite = await PGlite.create({ parsers: { 20: (x) => Number(x), 1184: (x) => `lite:${x}` } });
    await lite.exec(schema);
    const control = (await lite.query(direct, [1])).rows[0];
    check("PGlite control (no reylo): the instance's parsers", control.big === 9007199254740992 && String(control.at).startsWith("lite:"), show(control));
    await both("PGlite", lite);
    same("PGlite, a later run", await q.run(lite, { id: 1 }), declared);
    same("PGlite transaction through run", await lite.transaction((tx) => built.run(tx, { id: 1 })), declared);
    same("PGlite, the app's own query after reylo's", (await lite.query(direct, [1])).rows[0], control);
    await lite.close();
  }
  for (const [oid, parser] of Object.entries(defaults)) pg.types.setTypeParser(Number(oid), parser);
} finally {
  await admin.query("drop database if exists porg_decode with (force)");
  await admin.end();
}
console.log(failures ? `${failures} failed` : "all passed");
process.exitCode = failures ? 1 : 0;
