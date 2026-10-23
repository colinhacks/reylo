import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { execute, type QueryModule } from "../src/execute.ts";
import { parseArray } from "../src/decode.ts";
import type { Columns } from "../src/values.ts";

// Fakes of the Postgres drivers that decode as the real ones do (checked against
// lab/drivers/node_modules and wiki/driver-decoding/spike/postgres.md): a result asks a types
// table (the query's own, else the client's) for one parser per field and applies it to the
// field's text. The app's own parsers are hostile here: an int8 its text, a timestamptz tagged,
// an interval an object, an enum upper-cased. Reylo's rows must not see any of them.
type Parser = (text: string) => unknown;
function noParse(x: string) {
  return String(x);
}
const appTable: Record<number, Parser> = {
  20: (x) => x,
  23: (x) => parseInt(x, 10),
  1184: (x) => `app:${x}`,
  1186: (x) => ({ interval: x }),
  1231: (x) => parseArray(x, parseFloat),
};
const appTypes = { getTypeParser: (oid: number) => appTable[oid] ?? noParse };

// Oids the database created: an enum and its array, a domain's array, a row type's array, the
// array of a type whose delimiter is not a comma (PostGIS's geometry), pgvector's vector and an
// hstore and their arrays, and a type created after Reylo first read the catalog.
const ROLE_ARR = 70000, ROLE = 70001, POSINT_ARR = 70010, POSINT = 70011, PAIR_ARR = 70020, PAIR = 70021, LATE_ARR = 70030, LATE = 70031, SHAPE_ARR = 70040, SHAPE = 70041, VEC_ARR = 70050, VEC = 70051, HSTORE_ARR = 70060, HSTORE = 70061;
const COLUMNS: [string, number, string | null][] = [
  ["roles", ROLE_ARR, "{admin,member,NULL}"],
  ["role", ROLE, "admin"],
  ["names", 1003, "{a,b}"],
  ["posints", POSINT_ARR, "{1,2}"],
  ["pairs", PAIR_ARR, '{"(1,x)"}'],
  ["span", 1186, "1 year 2 mons"],
  ["spans", 1187, '{"1 day","02:00:00"}'],
  ["amounts", 1231, "{1.5,12345678901234567890.1}"],
  ["ints", 1007, "{1,NULL}"],
  ["big", 20, "9007199254740993"],
  ["at", 1184, "2024-01-15 10:30:00.123456+02"],
  ["wall", 1114, "2024-01-15 10:30:00.123456"],
  ["day", 1082, "2024-01-15"],
  ["doc", 3802, '{"a":[1,2]}'],
  ["bytes", 17, "\\x0102ff"],
  ["flag", 16, "t"],
  ["boxes", 1020, "{(1,1),(0,0);(2,2),(1,1)}"],
  ["shapes", SHAPE_ARR, "{0101:NULL:0102}"],
  ["embedding", VEC, "[1,2.5,-3,1e-07]"],
  ["embeddings", VEC_ARR, '{"[1,2]",NULL,"[3]"}'],
  ["prefs", HSTORE, String.raw`"a"=>"1", "b c"=>NULL, "q\""=>"\\"`],
  ["history", HSTORE_ARR, String.raw`{"\"a\"=>\"1\"",NULL,""}`],
  ["nothing", ROLE_ARR, null],
];
/** What Reylo returns for each column: src/values.ts. */
const declared = {
  roles: ["admin", "member", null],
  role: "admin",
  names: ["a", "b"],
  posints: [1, 2],
  pairs: '{"(1,x)"}',
  span: "P1Y2M",
  spans: ["P1D", "PT2H"],
  amounts: ["1.5", "12345678901234567890.1"],
  ints: [1, null],
  big: 9007199254740993n,
  at: "2024-01-15T08:30:00.123456Z",
  wall: "2024-01-15T10:30:00.123456",
  day: "2024-01-15",
  doc: { a: [1, 2] },
  bytes: Uint8Array.from([1, 2, 255]),
  flag: true,
  boxes: ["(1,1),(0,0)", "(2,2),(1,1)"],
  shapes: ["0101", null, "0102"],
  embedding: [1, 2.5, -3, 1e-7],
  embeddings: [[1, 2], null, [3]],
  prefs: { a: "1", "b c": null, 'q"': "\\" },
  history: [{ a: "1" }, null, {}],
  nothing: null,
};
/** The module's columns for the same statement, as a build step writes them (src/loader.ts). */
const columns: Columns = [
  ["roles", "text[]"], ["role", "text"], ["names", "text[]"], ["posints", "int[]"], ["pairs", "text"], ["span", "text"], ["spans", "text[]"], ["amounts", "decimal[]"], ["ints", "int[]"],
  ["big", "int8"], ["at", "timestamptz"], ["wall", "timestamp"], ["day", "date"], ["doc", "json"], ["bytes", "bytes"], ["flag", "bool"], ["boxes", "text[]"], ["shapes", "text[]", ":"], ["embedding", "vector"], ["embeddings", "vector[]"], ["prefs", "hstore"], ["history", "hstore[]"], ["nothing", "text[]"],
];

// The catalog, as decode.ts reads it: [array oid, element's base oid, a row type's array (kept as
// text), delimiter, element oid, base type's name], for a type or an array type it asks about.
function database() {
  const db = {
    types: [
      [ROLE_ARR, ROLE, false, ",", ROLE, "role"],
      [POSINT_ARR, 23, false, ",", POSINT, "int4"],
      [PAIR_ARR, PAIR, true, ",", PAIR, "pair"],
      [SHAPE_ARR, SHAPE, false, ":", SHAPE, "geometry"],
      [VEC_ARR, VEC, false, ",", VEC, "vector"],
      [HSTORE_ARR, HSTORE, false, ",", HSTORE, "hstore"],
    ] as [number, number, boolean, string, number, string][],
    catalogQueries: [] as string[],
    late() {
      db.types.push([LATE_ARR, LATE, false, ",", LATE, "late"]);
    },
    answer(text: string): { fields: { name: string; dataTypeID: number }[]; raw: (string | null)[][] } {
      if (text.includes("pg_catalog.pg_type")) {
        db.catalogQueries.push(text);
        const wanted = /any\('\{([\d,]+)\}'/.exec(text)![1]!.split(",").map(Number);
        // An array oid asked about also brings the array type's own row after its element's, as a
        // query matching every requested oid would return it.
        const rows = db.types
          .flatMap(([array, base, isText, delim, element, name]) => [
            ...(wanted.includes(array) || wanted.includes(element) ? [[array, base, isText, delim, element, name]] : []),
            ...(wanted.includes(array) ? [[0, array, true, ",", array, `_${name}`]] : []),
          ])
          .map((row) => row.map(String));
        return { fields: [0, 1, 2, 3, 4, 5].map((i) => ({ name: `c${i}`, dataTypeID: 25 })), raw: rows };
      }
      const cols = [...COLUMNS];
      if (text.includes("late")) cols.push(["lates", LATE_ARR, "{x,y}"], ["late", LATE, "x"]);
      return { fields: cols.map(([name, dataTypeID]) => ({ name, dataTypeID })), raw: [cols.map(([, , v]) => v)] };
    },
  };
  return db;
}
type Db = ReturnType<typeof database>;

/** Rows compared by value, binary data by its bytes (a Buffer is a Uint8Array, with its own prototype). */
const bytes = (v: unknown): unknown => (v instanceof Uint8Array ? { bytes: [...v] } : Array.isArray(v) ? v.map(bytes) : v !== null && typeof v === "object" && !(v instanceof Date) ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, bytes(x)])) : v);
const same = (actual: unknown, expected: unknown) => assert.deepEqual(bytes(actual), bytes(expected));

type Config = { text: string; values?: unknown[]; types?: { getTypeParser(oid: number, format?: string): Parser }; rowMode?: string };

/** pg's Client: a query's own table, else the client's setTypeParser over the app's global table. */
function pgClient(db: Db, options: { binary?: boolean } = {}) {
  const overrides: Record<number, Parser> = {};
  const client = Object.assign(new EventEmitter(), {
    binary: options.binary ?? false,
    calls: [] as unknown[],
    setTypeParser: (oid: number, fn: Parser) => void (overrides[oid] = fn),
    getTypeParser: (oid: number, _format?: string) => overrides[oid] ?? appTypes.getTypeParser(oid),
    async query(config: string | Config, values?: unknown[]) {
      client.calls.push(config);
      const c: Config = typeof config === "string" ? { text: config, ...(values ? { values } : {}) } : config;
      const types = c.types ?? client;
      const answer = db.answer(c.text);
      const parsers = answer.fields.map((f) => types.getTypeParser(f.dataTypeID, client.binary ? "binary" : "text"));
      const rows = answer.raw.map((raw) => {
        const values = raw.map((v, i) => (v === null ? null : parsers[i]!(v)));
        return c.rowMode === "array" ? values : Object.fromEntries(answer.fields.map((f, i) => [f.name, values[i]]));
      });
      return { fields: answer.fields, rows };
    },
  });
  return client;
}

const probe: QueryModule<"postgres"> = { dialect: "postgres", sql: "select * from probe where id = :id", params: ["id"] };
const built: QueryModule<"postgres"> = { ...probe, columns };
const lateProbe: QueryModule<"postgres"> = { dialect: "postgres", sql: "select *, late from probe where id = :id", params: ["id"] };

test("parseArray: quoting, escapes, NULL, nesting, bounds and box's delimiter", () => {
  assert.deepEqual(parseArray("{}", String), []);
  assert.deepEqual(parseArray('{a,"b,c","NULL",NULL,"",\"q\\"x\\\\y\"}', String), ["a", "b,c", "NULL", null, "", 'q"x\\y']);
  assert.deepEqual(parseArray("{{1,2},{3,NULL}}", Number), [[1, 2], [3, null]]);
  assert.deepEqual(parseArray("[0:1]={7,8}", Number), [7, 8]);
  assert.deepEqual(parseArray('{"(1,x)","(2,\\"y,z\\")"}', String), ["(1,x)", '(2,"y,z")']);
  assert.deepEqual(parseArray("{(1,1),(0,0);(2,2),(1,1)}", String, ";"), ["(1,1),(0,0)", "(2,2),(1,1)"]);
});

test("pg Client: Reylo's table answers every oid, so no parser of the app's applies to its query, and the app's next query is unchanged", async () => {
  const db = database();
  const client = pgClient(db);
  client.setTypeParser(1082, (x) => `client:${x}`);
  client.setTypeParser(ROLE, (x) => x.toUpperCase());
  const control = (await client.query("select * from probe")).rows[0] as Record<string, unknown>;
  // The failing control: the app's own table, whose int8 parser returns the text.
  assert.equal(control.big, "9007199254740993");
  assert.equal(control.at, "app:2024-01-15 10:30:00.123456+02");
  assert.equal(control.day, "client:2024-01-15");
  assert.equal(control.roles, "{admin,member,NULL}");
  assert.deepEqual(control.span, { interval: "1 year 2 mons" });
  // A query defined by hand: the types the database created through one catalog query.
  const [row] = (await execute(client, probe, { id: 1 })) as Record<string, unknown>[];
  same(row, declared);
  assert.equal(db.catalogQueries.length, 1);
  assert.match(db.catalogQueries[0]!, new RegExp(`any\\('\\{${ROLE_ARR},${ROLE},${POSINT_ARR},${PAIR_ARR},${SHAPE_ARR},${VEC},${VEC_ARR},${HSTORE},${HSTORE_ARR}\\}'`));
  await execute(client, probe, { id: 1 });
  assert.equal(db.catalogQueries.length, 1, "what the catalog said is kept per client");
  assert.deepEqual((await client.query("select * from probe")).rows[0], control, "the app's own queries keep its parsers");
  const sent = client.calls.find((c) => typeof c === "object" && (c as Config).text === "select * from probe where id = $1") as Config;
  assert.deepEqual(sent.values, [1]);
});

test("pg Client: a module's columns give the created types their kinds, so no catalog query runs", async () => {
  const db = database();
  const client = pgClient(db);
  const [row] = (await execute(client, built, { id: 1 })) as Record<string, unknown>[];
  same(row, declared);
  assert.equal(db.catalogQueries.length, 0);
  // What the columns said is kept for the client, delimiter and all, for a query defined by hand.
  same((await execute(client, probe, { id: 1 }))[0], declared);
  assert.equal(db.catalogQueries.length, 0);
});

test("pg Client: a type created after Reylo read the catalog comes back decoded, after one lookup of that oid", async () => {
  const db = database();
  const client = pgClient(db);
  await execute(client, probe, { id: 1 });
  db.late();
  const [row] = (await execute(client, lateProbe, { id: 1 })) as Record<string, unknown>[];
  assert.deepEqual([row!.lates, row!.late], [["x", "y"], "x"]);
  assert.equal(db.catalogQueries.length, 2);
  assert.match(db.catalogQueries[1]!, /any\('\{70030,70031\}'::pg_catalog\.oid\[\]\)/);
  await execute(client, lateProbe, { id: 1 });
  assert.equal(db.catalogQueries.length, 2, "a learned oid is not looked up again");
});

test("pg: a client set up with binary: true is refused, before the query and if pg asks for a binary parser", async () => {
  const db = database();
  await assert.rejects(execute(pgClient(db, { binary: true }), built, { id: 1 }) as Promise<unknown>, /binary: true .*pg 8 reads wrong/);
  // pg.defaults.binary, which the client does not show: pg asks the table for the binary format.
  const sneaky = pgClient(db, { binary: true });
  Object.defineProperty(sneaky, "binary", { get: () => false });
  const asked: string[] = [];
  const original = sneaky.query;
  sneaky.query = async (config: string | Config, values?: unknown[]) => {
    if (typeof config === "object" && config.types) {
      const types = config.types;
      config = { ...config, types: { getTypeParser: (oid: number) => (asked.push("binary"), types.getTypeParser(oid, "binary")) } };
    }
    return original(config, values);
  };
  await assert.rejects(execute(sneaky, built, { id: 1 }) as Promise<unknown>, /binary: true/);
  assert.ok(asked.length > 0);
});

/** pg's Pool: no getTypeParser; query() runs on a client it checks out, whose `connect` listeners set parsers. */
function pgPool(db: Db) {
  const clients: ReturnType<typeof pgClient>[] = [];
  const pool = Object.assign(new EventEmitter(), {
    options: { max: 2 } as { max: number; binary?: boolean },
    Client: class {},
    clients,
    async connect() {
      const c = pgClient(db);
      pool.emit("connect", c);
      clients.push(c);
      return Object.assign(c, { release: () => {} });
    },
    async query(config: string | Config, values?: unknown[]) {
      const c = await pool.connect();
      return c.query(config, values);
    },
  });
  return pool;
}

test("pg Pool: pool.query with Reylo's table, so a connect listener's parsers do not apply either", async () => {
  const db = database();
  const pool = pgPool(db);
  pool.on("connect", (c: ReturnType<typeof pgClient>) => c.setTypeParser(1184, (x) => `conn:${x}`));
  const [row] = (await execute(pool, built, { id: 1 })) as Record<string, unknown>[];
  same(row, declared);
  assert.ok((pool.clients[0]!.calls[0] as Config).types, "the statement went through pool.query with types");
  pool.options.binary = true;
  await assert.rejects(execute(pool, built, { id: 1 }) as Promise<unknown>, /binary: true/);
});

test("pg-promise: result({ text, values, types }) on the database, a task and a transaction; its receive event is refused", async () => {
  const db = database();
  const conn = pgClient(db);
  const calls: unknown[] = [];
  const make = (receive?: (e: { data: Record<string, unknown>[] }) => void) => ({
    manyOrNone: async () => [],
    query: async (q: Config): Promise<any> => (calls.push(q), (await conn.query(q)).rows),
    result: async (q: Config): Promise<any> => {
      calls.push(q);
      const r = await conn.query(q);
      receive?.({ data: r.rows as Record<string, unknown>[] });
      return r;
    },
  });
  const pgp = Object.assign(make(), { $config: { options: {} } });
  const [row] = (await execute(pgp as never, built, { id: 1 }) as Record<string, unknown>[]);
  same(row, declared);
  assert.deepEqual(Object.keys(calls.at(-1) as object), ["text", "values", "types"]);
  // On the database the receive event can be read: refused before anything runs.
  const camel = (e: { data: Record<string, unknown>[] }) => e.data.forEach((r) => Object.keys(r).forEach((k) => ((r[k.toUpperCase()] = r[k]), delete r[k])));
  const withReceive = Object.assign(make(camel), { $config: { options: { receive: camel } } });
  const before = calls.length;
  await assert.rejects(execute(withReceive as never, built, { id: 1 }) as Promise<unknown>, /receive event rewrites every result/);
  assert.equal(calls.length, before);
  // A task exposes no options; a receive that renamed the columns is caught by the rows' keys.
  const task = make(camel);
  await assert.rejects(execute(task as never, built, { id: 1 }) as Promise<unknown>, /receive event rewrites every result/);
});

/** PGlite: a query's parsers merged over the instance's, as its parse.ts does. */
function pglite(db: Db, instance: Record<number, Parser> | undefined) {
  const lite = {
    ...(instance ? { parsers: instance, refreshArrayTypes: async () => {} } : { rollback: async () => {}, closed: false }),
    exec: async () => [],
    seen: [] as Record<number, Parser>[],
    async query(text: string, _values?: unknown[], options?: { parsers?: Record<number, Parser>; rowMode?: string }) {
      if (options?.parsers) lite.seen.push(options.parsers);
      const answer = db.answer(text);
      const parsers = { ...(instance ?? {}), ...options?.parsers };
      const rows = answer.raw.map((raw) => {
        const values = raw.map((v, i) => (v === null ? null : parsers[answer.fields[i]!.dataTypeID] ? parsers[answer.fields[i]!.dataTypeID]!(v) : v));
        return options?.rowMode === "array" ? values : Object.fromEntries(answer.fields.map((f, i) => [f.name, values[i]]));
      });
      return { fields: answer.fields, rows };
    },
  };
  return lite;
}

test("PGlite: per-query parsers that name the statement's oids, every oid the instance parses on a first run; a transaction too", async () => {
  const db = database();
  // The instance's table: the app's parsers, and PGlite's array parsers built at start.
  const instance: Record<number, Parser> = { ...appTable, 1082: (x) => new Date(x), 1184: (x) => new Date(x), [POSINT_ARR]: (x) => parseArray(x, String), [ROLE]: (x) => x.toUpperCase() };
  const lite = pglite(db, instance);
  const q = { ...built, sql: "select * from probe where id = :id -- pglite" };
  const [row] = (await execute(lite as never, q, { id: 1 })) as Record<string, unknown>[];
  same(row, declared);
  const first = Object.keys(lite.seen[0]!).map(Number);
  assert.ok(first.includes(20) && first.includes(1082) && first.includes(POSINT_ARR) && first.includes(ROLE), "a first run names every oid the instance parses");
  await execute(lite as never, q, { id: 1 });
  assert.deepEqual(Object.keys(lite.seen[1]!).map(Number).sort(), COLUMNS.map(([, oid]) => oid).filter((o, i, all) => all.indexOf(o) === i).sort(), "a later run names only the statement's oids");
  // A transaction exposes no table; what it does not name went through the instance's parser, and is decoded from what it gave.
  const [inTx] = (await execute(pglite(db, undefined) as never, { ...built, sql: "select * from probe where id = :id -- tx" }, { id: 1 })) as Record<string, unknown>[];
  same(inTx, declared);
});

test("Neon's neon(): query(text, params, { fullResults, types }), which replaces Neon's whole table", async () => {
  const db = database();
  const conn = pgClient(db);
  const calls: unknown[][] = [];
  const neon = Object.assign(() => Promise.reject(new Error("not a template")), {
    transaction: () => {},
    unsafe: () => ({}),
    query: async (text: string, values: unknown[], options: { fullResults?: boolean; arrayMode?: boolean; types?: Config["types"] }) => {
      calls.push([text, values, options]);
      const r = await conn.query({ text, values, ...(options.types ? { types: options.types } : {}), ...(options.arrayMode ? { rowMode: "array" } : {}) });
      return options.fullResults ? r : r.rows;
    },
  });
  const [row] = (await execute(neon as never, probe, { id: 1 })) as Record<string, unknown>[];
  same(row, declared);
  assert.deepEqual(Object.keys(calls[0]![2] as object), ["fullResults", "arrayMode", "types"]);
});

/** postgres.js: `.raw()` gives each value's bytes and the result's columns; a row transform runs on raw rows too. */
function postgresJs(db: Db, rowTransform?: (row: unknown[]) => unknown) {
  return {
    typed: (value: unknown, type: number) => ({ value, type }),
    unsafe: (text: string) => ({
      raw: async () => {
        const answer = db.answer(text);
        const rows = answer.raw.map((r) => r.map((v) => (v === null ? null : Buffer.from(v))));
        return Object.assign(rowTransform ? rows.map(rowTransform) : rows, { columns: answer.fields.map((f) => ({ name: f.name.toUpperCase(), type: f.dataTypeID })) });
      },
    }),
  };
}

test("postgres.js: .raw() and Reylo's text parsers, the rows built with the module's names, which a column transform does not rename", async () => {
  const db = database();
  // The fake's columns are upper-cased, as an app's transform.column would rename them.
  const [row] = (await execute(postgresJs(db) as never, built, { id: 1 })) as Record<string, unknown>[];
  same(row, declared);
  // A query defined by hand has the server's names, and the catalog for the created types.
  const [bare] = (await execute(postgresJs(db) as never, probe, { id: 1 })) as Record<string, unknown>[];
  assert.deepEqual(bare!.ROLES, declared.roles);
  await assert.rejects(execute(postgresJs(db, (r) => [...r, "ROW-TRANSFORMED"]) as never, built, { id: 1 }) as Promise<unknown>, /transform\.row rewrites every row/);
});

test("Bun.sql: its values are given their kinds after the fact", async () => {
  const bunRow = {
    big: 9007199254740993n,
    day: new Date("2024-01-15T00:00:00Z"),
    wall: new Date("2024-01-15T10:30:00.120Z"),
    old: new Date("-000043-03-15T12:00:00Z"),
    ints: Int32Array.of(1, 2),
    roles: "{admin,member}",
    shapes: "{0101:0102}",
    embedding: "[1,2.5]",
    prefs: '"a"=>"1"',
    at: new Date("2024-01-15T08:30:00Z"),
    forever: Infinity,
  };
  const bun = { unsafe: async () => [{ ...bunRow }], commitDistributed: async () => {} };
  const q: QueryModule<"postgres"> = { ...probe, columns: [["big", "int8"], ["day", "date"], ["wall", "timestamp"], ["old", "timestamp"], ["ints", "int[]"], ["roles", "text[]"], ["shapes", "text[]", ":"], ["embedding", "vector"], ["prefs", "hstore"], ["at", "timestamptz"], ["forever", "date"]] };
  // The failing control: Bun.sql's timestamptz is a Date; Reylo's the instant's text, to Bun's millisecond.
  assert.ok(bunRow.at instanceof Date);
  assert.deepEqual(await execute(bun, q, { id: 1 }), [{ big: 9007199254740993n, day: "2024-01-15", wall: "2024-01-15T10:30:00.12", old: "0044-03-15T12:00:00 BC", ints: [1, 2], roles: ["admin", "member"], shapes: ["0101", "0102"], embedding: [1, 2.5], prefs: { a: "1" }, at: "2024-01-15T08:30:00Z", forever: "infinity" }]);
  // Without bigint: true, Bun's int8 is its text.
  const plain = { unsafe: async () => [{ big: "9007199254740993", at: new Date("2024-01-15T08:30:00.5Z") }], commitDistributed: async () => {} };
  assert.deepEqual(await execute(plain, { ...probe, columns: [["big", "int8"], ["at", "timestamptz"]] }, { id: 1 }), [{ big: 9007199254740993n, at: "2024-01-15T08:30:00.5Z" }]);
  // A query defined by hand: a typed array an array, and a Date, which no value is, the instant's text.
  assert.deepEqual(await execute(bun, probe, { id: 1 }), [{ ...bunRow, day: "2024-01-15T00:00:00Z", wall: "2024-01-15T10:30:00.12Z", old: "-000043-03-15T12:00:00Z", at: "2024-01-15T08:30:00Z", ints: [1, 2] }]);
});
