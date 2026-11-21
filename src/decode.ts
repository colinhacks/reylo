// Reylo decodes every value of its own Postgres queries itself, on the app's own client, pool,
// connection or transaction, so that a row has the type the module declares however the app set
// up its driver (the representation is src/values.ts). It never opens
// a connection, never mutates the app's object, and never changes the app's other queries. Every
// driver but Bun.sql hands over each value as the server's text, and only the per-type choice that
// follows is Reylo's (wiki/driver-decoding/spike/postgres.md):
//
// - pg's Client, PoolClient and Pool, pg-promise, Neon's Pool and Client, @vercel/postgres and
//   Neon's neon() take a decoder table per query (`{ text, values, types }`). Reylo's answers every
//   oid itself: a parser the app registered (globally, on a pool or a client) never applies to
//   Reylo's query, and pg's own (an interval object, a lossy numeric[], an enum array as text)
//   neither.
// - PGlite takes a per-query `parsers` table, which it merges over the instance's: the table names
//   the statement's own oids, learned from its earlier runs, since PGlite copies the whole table on
//   every query (a table of every oid cost +70% on a one-row query).
// - postgres.js takes no decoder per query: `.raw()` hands over each value's bytes, past the app's
//   `types` and value and column transforms, and Reylo builds the rows with the module's column
//   names.
// - Bun.sql decodes in native code and takes nothing per query: Reylo converts the values it got
//   (src/values.ts, `coerce`), by each column's kind, and by the result's own columns where Bun
//   1.4.4 and later describe them.
//
// A built-in type (an oid below 10000, the same in every database and version) has its kind here.
// A type the database created (an enum, a domain's array, an extension's type) has the kind the
// module gives its column; for a query defined by hand, one catalog query per client reads which of
// those oids are arrays, cached in a WeakMap keyed by the app's object. A row value the module knows
// the fields of has the kind of its column whatever its oid: `record` is one oid for every shape of
// row, and a built-in catalog table's row type is below 10000 too.
import { civilFromDays, coerce, DecodeError, elementKind, isRowKind, numbersInJson, scalarKind, textDecoder, utf8, withFraction, withinDimensions, type Columns, type Decoder, type JsonNumbers, type Kind } from "./values.ts";
import { lastOfEach, rowOf, type RowBuilder } from "./row.ts";
import { arrayLiteral, asByteaHex, isJsonValue, jsonText, jsonValue, postgresText, type AnyClient as Client, type Bound, type Marks, type QueryModule, type Runtime } from "./execute.ts";
import { readPostgresError } from "./query-error.ts";

type AnyClient = Record<string, any>;
type Field = { name: string; oid: number };

/** Below it the built-in types, whose oids every database shares (FirstGenbkiObjectId). */
const FIRST_USER_OID = 10000;

const SCALAR_OIDS: Readonly<Record<number, Kind>> = {
  16: "bool", 17: "bytes", 20: "int8", 21: "int", 23: "int", 26: "int", 700: "float", 701: "float",
  1700: "decimal", 790: "money", 1082: "date", 1114: "timestamp", 1184: "timestamptz", 1186: "interval", 114: "json", 3802: "json",
  600: "point", 601: "lseg", 602: "path", 603: "box", 604: "polygon", 628: "line", 718: "circle",
  3904: "range:int", 3906: "range:decimal", 3908: "range:timestamp", 3910: "range:timestamptz", 3912: "range:date", 3926: "range:int8",
  4451: "multirange:int", 4532: "multirange:decimal", 4533: "multirange:timestamp", 4534: "multirange:timestamptz", 4535: "multirange:date", 4536: "multirange:int8",
};

// Every built-in array type and its element, as Postgres 19's pg_type lists them (none was ever
// renumbered): `_record` is left out, since an array of row values is its text until its column's
// kind says what its fields are.
const ARRAY_OIDS = new Map<number, number>([
  [143, 142], [199, 114], [271, 5069], [629, 628], [651, 650], [719, 718], [775, 774], [791, 790], [1000, 16], [1001, 17],
  [1002, 18], [1003, 19], [1005, 21], [1006, 22], [1007, 23], [1008, 24], [1009, 25], [1010, 27], [1011, 28], [1012, 29],
  [1013, 30], [1014, 1042], [1015, 1043], [1016, 20], [1017, 600], [1018, 601], [1019, 602], [1020, 603], [1021, 700],
  [1022, 701], [1027, 604], [1028, 26], [1034, 1033], [1040, 829], [1041, 869], [1115, 1114], [1182, 1082], [1183, 1083],
  [1185, 1184], [1187, 1186], [1231, 1700], [1263, 2275], [1270, 1266], [1561, 1560], [1563, 1562], [2201, 1790],
  [2207, 2202], [2208, 2203], [2209, 2204], [2210, 2205], [2211, 2206], [2949, 2970], [2951, 2950], [3221, 3220],
  [3643, 3614], [3644, 3642], [3645, 3615], [3735, 3734], [3770, 3769], [3807, 3802], [3905, 3904], [3907, 3906],
  [3909, 3908], [3911, 3910], [3913, 3912], [3927, 3926], [4073, 4072], [4090, 4089], [4097, 4096], [4192, 4191],
  [5039, 5038], [6150, 4451], [6151, 4532], [6152, 4533], [6153, 4534], [6155, 4535], [6157, 4536], [6442, 6437], [6491, 6490],
]);
const BOX_ARRAY = 1020;

/** A built-in type's kind and its array delimiter; undefined for a type the database created. */
function builtin(oid: number): { kind: Kind; delim: string } | undefined {
  const scalar = SCALAR_OIDS[oid];
  if (scalar) return { kind: scalar, delim: "," };
  const element = ARRAY_OIDS.get(oid);
  if (element !== undefined) return { kind: `${SCALAR_OIDS[element] ?? "text"}[]`, delim: oid === BOX_ARRAY ? ";" : "," };
  return oid < FIRST_USER_OID ? { kind: "text", delim: "," } : undefined;
}

const builtinDecoders = new Map<number, Decoder>();
function builtinDecoder(oid: number): Decoder | undefined {
  let d = builtinDecoders.get(oid);
  if (d === undefined) {
    const b = builtin(oid);
    if (!b) return undefined;
    builtinDecoders.set(oid, (d = textDecoder(b.kind, b.delim)));
  }
  return d;
}

// ---------------------------------------------------------------------------------------------
// The types a database created: their kind from the module's columns, or from the catalog.

/** What Reylo learned of one client's types the database created: each oid's kind and delimiter. */
type Learned = Map<number, { kind: Kind; delim: string }>;
const learned = new WeakMap<object, Learned>();
const learnedOf = (key: object): Learned => {
  let l = learned.get(key);
  if (!l) learned.set(key, (l = new Map()));
  return l;
};

/** Rows as arrays of text, from a query that reads the catalog. */
type Load = (sql: string) => Promise<(string | null)[][]>;

// Every type among `oids`, and every array type's element, with its base type through any number
// of domains and that base type's name (an array of a type whose own output is an array stays
// text), a row type's fields as a JSON array of each one's name and type, and a range's or a
// multirange's subtype. An oid goes out as int8, since one above 2^31 would wrap as int4.
const catalogSql = (oids: number[]) => {
  const list = `'{${oids.map((o) => Math.trunc(o)).join(",")}}'::pg_catalog.oid[]`;
  return `with recursive d(oid, base) as (
  select oid, typbasetype from pg_catalog.pg_type where typtype = 'd'
  union all
  select d.oid, t.typbasetype from d join pg_catalog.pg_type t on t.oid = d.base where t.typtype = 'd'
)
select e.typarray::int8::text, b.oid::int8::text, (b.typtype = 'c' or b.typcategory = 'A')::text, e.typdelim::text, e.oid::int8::text, b.typname::text,
  case when b.typtype = 'c' then coalesce((select pg_catalog.json_agg(pg_catalog.json_build_array(a.attname, a.atttypid::int8) order by a.attnum) from pg_catalog.pg_attribute a where a.attrelid = b.typrelid and a.attnum > 0 and not a.attisdropped)::text, '[]') end,
  b.typtype::text, (select r.rngsubtype::int8::text from pg_catalog.pg_range r where r.rngtypid = b.oid or r.rngmultitypid = b.oid)
from pg_catalog.pg_type e
join pg_catalog.pg_type b on b.oid = coalesce((select d.base from d join pg_catalog.pg_type t on t.oid = d.base where d.oid = e.oid and t.typtype <> 'd'), e.oid)
where e.typarray = any(${list}) or (e.oid = any(${list}) and e.typcategory <> 'A')`;
};

/**
 * Reads the kinds of the oids the client has not met yet: a type the database created by its
 * name, as the engine names it (pgvector's `vector` is its numbers, an enum its text), a row type
 * by its fields, each field's type read the same way where the client has not met it either, and
 * an array of one its element's kind.
 */
async function learn(key: object, oids: Iterable<number>, load: Load): Promise<void> {
  const known = learnedOf(key);
  const missing = new Set([...oids].filter((oid) => !known.has(oid)));
  if (!missing.size) return;
  const rows = await load(catalogSql([...missing]));
  // A row type's fields, by its base type's oid; the catalog forbids a row type that holds itself.
  const rowTypes = new Map<number, [string, number][]>();
  for (const row of rows) if (row[6]) rowTypes.set(Number(row[1]), JSON.parse(row[6]) as [string, number][]);
  // A range's or a multirange's subtype, by its base type's oid.
  const subtypes = new Map<number, { range: "range" | "multirange"; subtype: number }>();
  for (const row of rows) if (row[8] && (row[7] === "r" || row[7] === "m")) subtypes.set(Number(row[1]), { range: row[7] === "r" ? "range" : "multirange", subtype: Number(row[8]) });
  const unmet = [...new Set([...[...rowTypes.values()].flat().map(([, oid]) => Number(oid)), ...[...subtypes.values()].map((s) => s.subtype)])].filter((oid) => !builtin(oid) && !known.has(oid));
  if (unmet.length) await learn(key, unmet, load);
  const rowKind = (fields: [string, number][]) => `(${fields.map(([field, oid]) => {
    const k = builtin(Number(oid)) ?? known.get(Number(oid)) ?? { kind: "text", delim: "," };
    return JSON.stringify(k.kind.endsWith("[]") && k.delim !== "," ? [field, k.kind, k.delim] : [field, k.kind]);
  }).join(",")})`;
  // An array oid takes its element's kind, never the text its own row as a type would give it.
  const arrays = new Set<number>();
  for (const [array, base, isText, delim, oid, name] of rows) {
    const fields = rowTypes.get(Number(base));
    const range = subtypes.get(Number(base));
    const element = fields !== undefined ? rowKind(fields) : range !== undefined ? `${range.range}:${(builtin(range.subtype) ?? known.get(range.subtype))?.kind ?? "text"}` : isText === "true" ? undefined : (builtin(Number(base))?.kind ?? (name ? scalarKind(name) : "text"));
    if (missing.has(Number(array))) {
      arrays.add(Number(array));
      known.set(Number(array), element === undefined || element.endsWith("[]") ? { kind: "text", delim: "," } : { kind: `${element}[]`, delim: delim ?? "," });
    }
    if (missing.has(Number(oid)) && !arrays.has(Number(oid))) known.set(Number(oid), { kind: element ?? "text", delim: "," });
  }
  for (const oid of missing) if (!known.has(oid)) known.set(oid, { kind: "text", delim: "," });
}

/**
 * The decoder of each field a result named: a row value's by the module's column, a built-in type's
 * by its oid, a created type's by the module's column (when the module's columns match the result's)
 * or by what the catalog said.
 */
async function fieldDecoders(fields: Field[], columns: Columns | undefined, key: object, load: Load): Promise<Decoder[]> {
  const byColumn = columns && columns.length === fields.length ? columns : undefined;
  const created = fields.filter((f) => f.oid >= FIRST_USER_OID);
  const known = learnedOf(key);
  if (byColumn) created.forEach((f) => known.has(f.oid) || known.set(f.oid, { kind: byColumn[fields.indexOf(f)]![1], delim: byColumn[fields.indexOf(f)]![2] ?? "," }));
  else if (created.length) await learn(key, created.map((f) => f.oid), load);
  return fields.map((f, i) => {
    const column = byColumn?.[i];
    if (column && isRowKind(column[1])) return textDecoder(column[1], column[2]);
    const b = builtinDecoder(f.oid);
    if (b) return b;
    const k = known.get(f.oid) ?? { kind: "text", delim: "," };
    return textDecoder(k.kind, k.delim);
  });
}

// ---------------------------------------------------------------------------------------------
// pg and everything built on it: a per-query table that answers every oid.

/** The text of a value whose type the database created and Reylo had not met; decoded before `run` returns. */
class Pending {
  readonly oid: number;
  readonly text: string;
  constructor(oid: number, text: string) {
    this.oid = oid;
    this.text = text;
  }
}
/** A value pg read in the binary format, which pg 8.23 cannot read (it corrupts an int8). */
const BINARY = Symbol("binary");

/** The decoder table for one query on a pg-family client, which never falls back to the client's own parsers. */
function pgTypes(key: object) {
  const known = learnedOf(key);
  let binary = false;
  let pending = false;
  const table = {
    getTypeParser(oid: number, format?: string): Decoder {
      if (format !== undefined && format !== "text") {
        binary = true;
        return () => BINARY;
      }
      const b = builtinDecoder(oid);
      if (b) return b;
      const k = known.get(oid);
      if (k) return textDecoder(k.kind, k.delim);
      pending = true;
      return (text) => new Pending(oid, text);
    },
  };
  return { table, sawBinary: () => binary, sawPending: () => pending };
}

const refuseBinary = () =>
  new TypeError(`Reylo cannot read this pg client's results: it was set up with binary: true (or pg.defaults.binary), which pg 8 reads wrong (an int8 comes back as another number). Reylo decodes every value from the server's text; leave binary off.`);

// pg's native client (pg.native, over pg-native) ignores the `types` a query passes, so Reylo's
// decoders would be skipped and every value come back as pg's defaults. Its Client keeps the
// native binding on `native`, an object pg's JavaScript Client never has; a pool of them makes
// clients whose prototype has `_hasActiveQuery`, which only the native Client defines (pg 8.0 to 8.23).
function isPgNative(c: AnyClient): boolean {
  if (typeof c.getTypeParser === "function" && c.native != null && typeof c.native === "object") return true;
  return typeof c.connect === "function" && typeof c.Client?.prototype?._hasActiveQuery === "function";
}

const refusePgNative = () =>
  new TypeError(`Reylo cannot read this pg client's results: it is pg's native client (pg.native, over pg-native), which ignores the type parsers a query passes, so Reylo cannot decode its values. Use pg's JavaScript Client or Pool instead.`);

type PgResult = { rows: Record<string, unknown>[]; fields?: { name: string; dataTypeID: number }[] };

/**
 * Runs the query with Reylo's table through `run`, then decodes the values of the types the
 * database created that Reylo met for the first time, by the module's columns or the catalog.
 */
async function pgFamily(key: object, run: (types: object) => Promise<PgResult>, columns: Columns | undefined, load: Load): Promise<unknown[]> {
  const types = pgTypes(key);
  const result = await run(types.table);
  const rows = Array.isArray(result) ? (result as Record<string, unknown>[]) : result.rows;
  if (types.sawBinary()) throw refuseBinary();
  // The table goes by oid, which says nothing of a built-in row's fields, so those are still text.
  const rowColumns = columns?.filter(([, kind]) => isRowKind(kind));
  if (rowColumns?.length) {
    const decoders = rowColumns.map(([name, kind, delim]) => [name, textDecoder(kind, delim)] as const);
    for (const row of rows) {
      for (const [name, decode] of decoders) {
        const v = row[name];
        if (typeof v === "string") row[name] = decode(v);
      }
    }
  }
  if (!types.sawPending()) return rows;
  // Learned from the module's columns where the result has their fields, else from the catalog.
  if (result.fields && !Array.isArray(result)) await fieldDecoders(result.fields.map((f) => ({ name: f.name, oid: f.dataTypeID })), columns, key, load);
  const oids = new Set<number>();
  for (const row of rows) for (const v of Object.values(row)) if (v instanceof Pending) oids.add(v.oid);
  await learn(key, oids, load);
  const known = learnedOf(key);
  const decoders = new Map([...oids].map((oid) => [oid, textDecoder(known.get(oid)!.kind, known.get(oid)!.delim)]));
  for (const row of rows) {
    for (const name in row) {
      const v = row[name];
      if (v instanceof Pending) row[name] = decoders.get(v.oid)!(v.text);
    }
  }
  return rows;
}

/** A decoder table that keeps every value its text: the catalog's rows, whatever the app registered. */
const asText = { getTypeParser: () => (x: string) => x };

/** Neon's neon(), a function in every version, with transaction(): an object with a query() and a transaction() is somebody's own. */
const isNeonFunction = (c: AnyClient) => typeof c === "function" && typeof (c as AnyClient).transaction === "function";

/**
 * Whether a client with query() is one whose results Reylo decodes (`queryPostgres`). Any other,
 * such as a wrapper that passes its calls on to a pool, hands back what its driver made of each
 * value (a Date for a timestamp, a string for an int8), which no kind reads back, so it is
 * refused before it runs anything.
 */
export function takesQuery(c: AnyClient): boolean {
  return typeof c.manyOrNone === "function" || typeof c.getTypeParser === "function" || isPglite(c) || (typeof c.connect === "function" && typeof c.Client === "function") || isNeonFunction(c);
}

/**
 * Runs a Postgres statement on a client whose query() Reylo calls: pg's Client, PoolClient and
 * Pool, Neon's, @vercel/postgres's, pg-promise's database, tasks and transactions, PGlite and its
 * transactions, and Neon's neon(). Each is told apart by what it has, and a client that is none
 * of them is refused (`takesQuery`).
 */
export async function queryPostgres(c: AnyClient, text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  // pg-promise formats the values into the text itself (reading $1 inside a string literal
  // too) unless it is handed a { text, values } object, which it binds.
  if (typeof c.manyOrNone === "function") return pgPromise(c, text, values, columns);
  if (isPgNative(c)) throw refusePgNative();
  // pg's Client and PoolClient, Neon's, and @vercel/postgres's VercelClient.
  if (typeof c.getTypeParser === "function") {
    if (c.binary === true) throw refuseBinary();
    const load: Load = async (sql) => (await c.query({ text: sql, rowMode: "array", types: asText })).rows;
    return pgFamily(c, (types) => c.query({ text, values, types }), columns, load);
  }
  // PGlite and its transactions.
  if (isPglite(c)) return pglite(c, text, values, columns);
  // pg's Pool, Neon's and @vercel/postgres's: query() checks a client out of the pool, and the
  // table goes with the query, so a `connect` listener's parsers do not apply either.
  if (typeof c.connect === "function" && typeof c.Client === "function") {
    if (c.options?.binary === true) throw refuseBinary();
    const load: Load = async (sql) => (await c.query({ text: sql, rowMode: "array", types: asText })).rows;
    return pgFamily(c, (types) => c.query({ text, values, types }), columns, load);
  }
  // Neon's neon() from 1.0: query(text, params, options) replaces Neon's whole table with the
  // query's, and returns the fields with `fullResults`.
  if (isNeonFunction(c)) {
    const load: Load = async (sql) => (await c.query(sql, [], { arrayMode: true, fullResults: true, types: asText })).rows;
    return pgFamily(c, (types) => c.query(text, values, { fullResults: true, arrayMode: false, types }), columns, load);
  }
  throw new TypeError("Reylo cannot read this client's results: its query() is none of pg's, Neon's, pg-promise's and PGlite's.");
}

// pg-promise sends a { text, values, types } object to pg as it is, on the database, a task or a
// transaction. Its `receive` event runs on every result, Reylo's too, and may rewrite the rows: on
// the database Reylo reads it from the options and refuses; a task exposes no options, so a
// receive that renamed the columns is caught by the rows' keys.
async function pgPromise(db: AnyClient, text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  if (typeof db.$config?.options?.receive === "function") throw refuseReceive();
  if (db.$config?.options?.pgNative === true) throw refusePgNative();
  const run = (query: object) => (typeof db.result === "function" ? db.result(query) : db.query(query));
  const load: Load = async (sql) => {
    const r = await run({ text: sql, rowMode: "array", types: asText });
    return Array.isArray(r) ? r : r.rows;
  };
  // A task's or transaction's connection is not reachable, so what Reylo learns is kept per object.
  const rows = await pgFamily(db, (types) => run({ text, values, types }), columns, load);
  const first = rows[0] as Record<string, unknown> | undefined;
  if (first && columns && columns.length && !columns.every(([name]) => Object.prototype.hasOwnProperty.call(first, name))) throw refuseReceive();
  return rows;
}

const refuseReceive = () =>
  new TypeError(`Reylo cannot run on this pg-promise database: its receive event rewrites every result, Reylo's rows too, so they would not have the types the query declares. Run Reylo's queries on a database initialized without receive (pgp({ ... }) with no receive), or on its pool (db.$pool).`);

/** Neon's neon() before 1.0, a function called with (text, params, options), whose options take `types` (0.9 too, undocumented). */
export function neonCallable(c: AnyClient & ((...args: unknown[]) => Promise<any>), text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  const load: Load = async (sql) => (await c(sql, [], { arrayMode: true, fullResults: true, types: asText })).rows;
  return pgFamily(c, (types) => c(text, values, { fullResults: true, arrayMode: false, types }), columns, load);
}

// ---------------------------------------------------------------------------------------------
// PGlite

const isPglite = (c: AnyClient) => typeof c.query === "function" && typeof c.exec === "function" && (typeof c.refreshArrayTypes === "function" || (typeof c.rollback === "function" && "closed" in c));

/** The oids each statement's result had on its earlier runs, by the statement's text: PGlite's per-query table names only those. */
const pgliteOids = new Map<string, Set<number>>();
const PGLITE_STATEMENTS = 2000;
// Listed on PGlite's first statement, so that a bundle of another database's queries holds neither table.
let builtinOids: number[] | undefined;
const keepText = (x: string) => x;

async function pglite(db: AnyClient, text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  // A statement met for the first time names every built-in type Reylo decodes, and every oid the
  // instance has a parser for (its arrays, and the app's); a transaction has no table to read.
  let named = pgliteOids.get(text);
  builtinOids ??= [...Object.keys(SCALAR_OIDS).map(Number), ...ARRAY_OIDS.keys()];
  const oids = named ?? new Set([...builtinOids, ...Object.keys(db.parsers ?? {}).map(Number)]);
  const parsers: Record<number, Decoder> = {};
  for (const oid of oids) parsers[oid] = builtinDecoder(oid) ?? keepText;
  const result = await db.query(text, values, { rowMode: "object", parsers });
  const rows: Record<string, unknown>[] = result.rows;
  const fields: Field[] = (result.fields ?? []).map((f: { name: string; dataTypeID: number }) => ({ name: f.name, oid: f.dataTypeID }));
  // Created types, which the table kept as text, an oid the table did not name, which went through
  // the instance's parser, and a row value, whose oid says nothing of its fields.
  const byColumn = columns && columns.length === fields.length ? columns : undefined;
  const after = fields.map((f, i) => [f, i] as const).filter(([f, i]) => f.oid >= FIRST_USER_OID || !oids.has(f.oid) || (byColumn !== undefined && isRowKind(byColumn[i]![1])));
  if (after.length) {
    const load: Load = async (sql) => (await db.query(sql, [], { rowMode: "array", parsers: { 25: keepText } })).rows;
    const decoders = await fieldDecoders(fields, columns, db, load);
    const kinds = fields.map((f, i) => byColumn?.[i]![1] ?? builtin(f.oid)?.kind ?? learnedOf(db).get(f.oid)?.kind ?? "text");
    for (const [f, i] of after) {
      const decode = decoders[i]!;
      for (const row of rows) {
        const v = row[f.name];
        if (v !== null && v !== undefined) row[f.name] = typeof v === "string" ? decode(v) : coerce(kinds[i]!, v, f.name, ",", true);
      }
    }
  }
  if (!named || fields.some((f) => !named!.has(f.oid))) {
    if (pgliteOids.size >= PGLITE_STATEMENTS) pgliteOids.clear();
    pgliteOids.set(text, (named = new Set([...(named ?? []), ...fields.map((f) => f.oid)])));
  }
  return rows;
}

// ---------------------------------------------------------------------------------------------
// postgres.js: `.raw()`, and the rows built by Reylo.

type BufferLike = Uint8Array;
const textOf = utf8;

export async function postgresJs(c: AnyClient, text: string, params: unknown[], columns: Columns | undefined, row?: RowBuilder): Promise<unknown[]> {
  // Prepared, as postgres.js prepares its own queries: its unsafe() prepares nothing by default,
  // so every run with a parameter took two round trips (Parse and Describe, then Bind and Execute)
  // where a statement prepared on the connection takes one. A client made with `prepare: false`,
  // as one behind a pooler in transaction mode is, still prepares nothing: its option wins.
  const pending = c.unsafe(text, params, { prepare: true });
  // postgres.js's own query has raw(). One without it comes from a wrapper, whose values the driver already converted.
  if (typeof pending?.raw !== "function") throw new TypeError("Reylo cannot read this client's results: its unsafe() returns no postgres.js query, whose raw() hands over the server's text.");
  const raw = await pending.raw();
  // A row transform (`transform: { row }`) runs on `.raw()` rows too, and leaves them not an array of each value's bytes.
  const transformed = `Reylo cannot read this postgres.js client's rows: its transform.row rewrites every row, Reylo's too, so they would not have the types the query declares. Run Reylo's queries on a postgres() instance without transform.row.`;
  const load: Load = async (sql) => ((await c.unsafe(sql).raw()) as (BufferLike | null)[][]).map((r) => r.map((v) => (v === null ? null : textOf(v))));
  return await rawRows(raw, columns, c, load, row, transformed);
}

// ---------------------------------------------------------------------------------------------
// Drizzle's Postgres driver (`drizzle-orm/postgres/client`, and `@drizzle-team/postgres`): a query
// in its `buffer` mode hands over each value as the bytes of the server's text, past every decoder
// the app set up (`types`, `int8`, `temporal`, `jsonBigints`), and `debug` names each column's type.

/** Drizzle's Pool (acquire() and outsideTransaction()) and its Connection (queryTyped()), on every runtime it connects from: a transaction's and an acquired connection are Connections too. */
const isDrizzlePostgres = (c: AnyClient) => typeof c.query === "function" && (typeof c.queryTyped === "function" || (typeof c.acquire === "function" && typeof c.outsideTransaction === "function"));

/** Drizzle's HTTP clients: `neon-http`'s and `http`'s (parallel()), and `aurora`'s (endpoint and inTransaction), whose results name no column's type. */
const isDrizzleHttp = (c: AnyClient) => typeof c.query === "function" && (typeof c.parallel === "function" || (typeof c.endpoint === "string" && typeof c.inTransaction === "boolean"));

const refuseDrizzleHttp = () =>
  new TypeError("Reylo cannot read the results of Drizzle's HTTP clients (neon-http, http and aurora), which name no column's type. Pass Drizzle's Pool or a Connection, from neon-ws for Neon.");

async function drizzlePostgres(c: AnyClient, text: string, params: unknown[], columns: Columns | undefined, row?: RowBuilder): Promise<unknown[]> {
  // Unnamed, so the server sends every column as text: only a statement the app named takes binary columns on its later runs.
  const result = await c.query(text, params, { mode: "buffer", debug: true });
  const described: { name: string; oid: number }[] | undefined = result?.debug?.columns;
  if (!Array.isArray(result?.rows) || !Array.isArray(described)) throw new TypeError("Reylo cannot read this client's results: its query() in buffer mode named no column's type.");
  const raw = Object.assign(result.rows as unknown[], { columns: described.map((f) => ({ name: f.name, type: f.oid })) });
  const load: Load = async (sql) => ((await c.query(sql, [], { mode: "buffer" })).rows as (BufferLike | null)[][]).map((r) => r.map((v) => (v === null ? null : textOf(v))));
  return await rawRows(raw, columns, c, load, row, "Reylo cannot read this client's rows: its query() in buffer mode handed over no array of each value's bytes.");
}

/**
 * Rows from a `.raw()` result, an array of each value's bytes per row with the result's columns
 * (postgres.js's, and Bun.sql's from 1.4.4): each value by its column's decoder, the text the server
 * wrote, or for the columns `binary` names their binary form, which Reylo writes as that text first.
 */
async function rawRows(raw: unknown[] & { columns?: { name: string; type: number }[] | null }, columns: Columns | undefined, key: object, load: Load, row: RowBuilder | undefined, transformed: string, binary?: (oid: number) => boolean): Promise<unknown[]> {
  const fields: Field[] = (raw.columns ?? []).map((f) => ({ name: f.name, oid: f.type }));
  if (!raw.length) return [];
  for (const r of raw) {
    if (!Array.isArray(r) || r.length !== fields.length || !r.every((v) => v === null || v instanceof Uint8Array)) throw new TypeError(transformed);
  }
  const decoders = await fieldDecoders(fields, columns, key, load);
  const fromBytes = fields.map((f, i): ((v: BufferLike) => unknown) => {
    const decode = decoders[i]!;
    return binary?.(f.oid) ? binaryDecoder(f.oid, decode, f.name) : (v) => decode(textOf(v));
  });
  // The module's names, which a `transform.column` of the app's does not rename; a query defined by
  // hand has the server's.
  const byColumns = columns !== undefined && columns.length === fields.length;
  const build = byColumns && row ? row : rowOf(byColumns ? columns.map(([name]) => name) : fields.map((f) => f.name));
  const n = fields.length;
  // Each row's values, decoded into one array the builder copies from.
  const decoded = new Array<unknown>(n);
  const out = new Array(raw.length);
  for (let r = 0; r < raw.length; r++) {
    const values = raw[r] as (BufferLike | null)[];
    for (let i = 0; i < n; i++) {
      const v = values[i];
      decoded[i] = v === null || v === undefined ? null : fromBytes[i]!(v);
    }
    out[r] = build(decoded);
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// Postgres's binary format, which Bun.sql's `.raw()` hands over for some types: each value as the
// text Postgres writes for it, so that the column's decoder gives what every other driver's text
// gives, or for a boolean, an int4, a float8 and a bytea the value itself.

/**
 * The types Bun.sql 1.4.4 and later read in binary in a `.raw()` query that has parameters, on a
 * client that prepares its statements; a simple query's results, and every other type, are text
 * (Bun's docs, "`.raw()` format", and its `Tag::is_binary_in`).
 */
const BUN_BINARY = new Set([16, 17, 23, 700, 701, 1083, 1114, 1184, 1007, 1021, 1700]);

const notBinary = (name: string, oid: number, bytes: number) =>
  new DecodeError(`Column "${name}" came from Bun.sql in ${bytes} bytes, which is not the binary form of its type (oid ${oid}) that Bun's .raw() hands over for it. Reylo refuses it rather than misread it: run the query on a client made with prepare: false, whose results are text.`);

function binaryDecoder(oid: number, decode: Decoder, name: string): (v: BufferLike) => unknown {
  const view = (v: BufferLike, length?: number) => {
    if (length !== undefined && v.byteLength !== length) throw notBinary(name, oid, v.byteLength);
    return new DataView(v.buffer, v.byteOffset, v.byteLength);
  };
  switch (oid) {
    case 16:
      return (v) => view(v, 1).getUint8(0) !== 0;
    case 17:
      return (v) => v;
    case 23:
      return (v) => view(v, 4).getInt32(0);
    case 701:
      return (v) => view(v, 8).getFloat64(0);
    case 700:
      return (v) => decode(float4Text(view(v, 4).getFloat32(0)));
    case 1083:
      return (v) => decode(timeText(view(v, 8)));
    case 1114:
    case 1184:
      return (v) => decode(timestampText(view(v, 8), oid === 1184));
    case 1700:
      return (v) => decode(numericText(view(v), name));
    case 1007:
    case 1021:
      return (v) => decode(arrayText(view(v), oid === 1007 ? (d, at) => String(d.getInt32(at)) : (d, at) => float4Text(d.getFloat32(at)), oid === 1007 ? 23 : 700, name));
    default:
      throw notBinary(name, oid, 0);
  }
}

/** A float4 as Postgres writes it: the shortest text that reads back as the same 32-bit float. */
function float4Text(f: number): string {
  if (Number.isNaN(f)) return "NaN";
  if (!Number.isFinite(f)) return f > 0 ? "Infinity" : "-Infinity";
  if (Object.is(f, -0)) return "-0";
  for (let p = 1; p <= 9; p++) {
    const text = f.toPrecision(p);
    if (Math.fround(+text) === f) return String(+text);
  }
  return String(f);
}

/** A signed 64-bit integer, exact: as a number where it fits, else through a BigInt. */
function int64(d: DataView, at = 0): number | bigint {
  const hi = d.getInt32(at);
  return hi > -0x200000 && hi < 0x200000 ? hi * 4294967296 + d.getUint32(at + 4) : d.getBigInt64(at);
}

/** Whole units of `per` in `v` and what is left, floored, for a number or a BigInt. */
function divide(v: number | bigint, per: number): [number, number] {
  if (typeof v === "number") {
    const q = Math.floor(v / per);
    return [q, v - q * per];
  }
  const p = BigInt(per);
  let q = v / p;
  if (v % p < 0n) q -= 1n;
  return [Number(q), Number(v - q * p)];
}

const two = (n: number) => (n < 10 ? `0${n}` : String(n));
const fraction = (us: number) => (us === 0 ? "" : `.${String(us).padStart(6, "0")}`);
const clock = (seconds: number, us: number) => `${two(Math.floor(seconds / 3600))}:${two(Math.floor(seconds / 60) % 60)}:${two(seconds % 60)}${fraction(us)}`;

/** A time, microseconds since midnight, as Postgres writes it: `12:00:01.5`, up to `24:00:00`. */
function timeText(d: DataView): string {
  const [seconds, us] = divide(int64(d), 1e6);
  return clock(seconds, us);
}

/** Days from 1970-01-01 to 2000-01-01, Postgres's epoch. */
const POSTGRES_EPOCH_DAYS = 10957;

/**
 * A timestamp, microseconds since 2000-01-01, as Postgres writes it in DateStyle ISO, a year before 1
 * with ` BC`; a timestamptz in UTC with `+00`; and each infinity as its word.
 */
function timestampText(d: DataView, zoned: boolean): string {
  const hi = d.getInt32(0), lo = d.getUint32(4);
  if (hi === 0x7fffffff && lo === 0xffffffff) return "infinity";
  if (hi === -0x80000000 && lo === 0) return "-infinity";
  const [seconds, us] = divide(int64(d), 1e6);
  const [days, sod] = divide(seconds, 86400);
  const [y, m, day] = civilFromDays(days + POSTGRES_EPOCH_DAYS);
  const year = y > 0 ? String(y).padStart(4, "0") : String(1 - y).padStart(4, "0");
  return `${year}-${two(m)}-${two(day)} ${clock(sod, us)}${zoned ? "+00" : ""}${y > 0 ? "" : " BC"}`;
}

/** A numeric as Postgres writes it: its base-10000 digits, its weight and its display scale. */
function numericText(d: DataView, name: string): string {
  if (d.byteLength < 8) throw notBinary(name, 1700, d.byteLength);
  const ndigits = d.getUint16(0), weight = d.getInt16(2), sign = d.getUint16(4), dscale = d.getUint16(6);
  if (d.byteLength !== 8 + 2 * ndigits) throw notBinary(name, 1700, d.byteLength);
  if (sign === 0xc000) return "NaN";
  if (sign === 0xd000) return "Infinity";
  if (sign === 0xf000) return "-Infinity";
  const digit = (k: number) => (k >= 0 && k < ndigits ? d.getUint16(8 + 2 * k) : 0);
  let whole = "";
  for (let k = 0; k <= weight; k++) whole += k === 0 ? String(digit(k)) : String(digit(k)).padStart(4, "0");
  let frac = "";
  for (let k = weight + 1; frac.length < dscale; k++) frac += String(digit(k)).padStart(4, "0");
  return `${sign === 0x4000 ? "-" : ""}${whole || "0"}${dscale > 0 ? `.${frac.slice(0, dscale)}` : ""}`;
}

/** An array of int4 or float4 in Postgres's binary form as its text: `{1,NULL}`, nested by dimension, with its bounds where one is not 1. */
function arrayText(d: DataView, element: (d: DataView, at: number) => string, elementOid: number, name: string): string {
  const fail = () => notBinary(name, elementOid === 23 ? 1007 : 1021, d.byteLength);
  if (d.byteLength < 12) throw fail();
  const ndim = d.getInt32(0);
  if (ndim === 0) return "{}";
  if (d.getInt32(8) !== elementOid || d.byteLength < 12 + 8 * ndim) throw fail();
  const dims: number[] = [], lower: number[] = [];
  for (let k = 0; k < ndim; k++) {
    dims.push(d.getInt32(12 + 8 * k));
    lower.push(d.getInt32(16 + 8 * k));
  }
  let at = 12 + 8 * ndim;
  const level = (k: number): string => {
    const parts: string[] = [];
    for (let i = 0; i < dims[k]!; i++) {
      if (k + 1 < ndim) {
        parts.push(level(k + 1));
        continue;
      }
      if (at + 4 > d.byteLength) throw fail();
      const length = d.getInt32(at);
      at += 4;
      if (length === -1) parts.push("NULL");
      else {
        if (length !== 4 || at + 4 > d.byteLength) throw fail();
        parts.push(element(d, at));
        at += 4;
      }
    }
    return `{${parts.join(",")}}`;
  };
  const text = level(0);
  if (at !== d.byteLength) throw fail();
  return lower.every((l) => l === 1) ? text : `${lower.map((l, k) => `[${l}:${l + dims[k]! - 1}]`).join("")}=${text}`;
}

/** Bun 1.4.4 or later, whose `.raw()` results Reylo reads (its format rule and `result.columns`). */
function bunReadsRaw(): boolean {
  const version = (globalThis as { Bun?: { version?: string } }).Bun?.version;
  const m = typeof version === "string" ? /^(\d+)\.(\d+)\.(\d+)/.exec(version) : null;
  if (!m) return false;
  const [major, minor, patch] = [+m[1]!, +m[2]!, +m[3]!];
  return major > 1 || (major === 1 && (minor > 4 || (minor === 4 && patch >= 4)));
}

/**
 * Bun.sql on Postgres from 1.4.4: `.raw()`, whose bytes are text, or binary for the types
 * `BUN_BINARY` names in a query with parameters on a client that prepares its statements, which
 * its `options` say (on the pool, a transaction, a savepoint and a reserved connection alike).
 * Reylo decodes every value itself, so a timestamp keeps its microseconds and a year past 275760
 * reads, which a Date cannot hold. Undefined where Bun is older, or its client does not say whether
 * it prepares.
 */
async function bunRaw(c: AnyClient, text: string, params: unknown[], columns: Columns | undefined, row: RowBuilder | undefined): Promise<unknown[] | undefined> {
  const prepare = (c.options as { prepare?: unknown } | undefined)?.prepare;
  if (typeof prepare !== "boolean" || !bunReadsRaw()) return undefined;
  const pending = c.unsafe(text, params);
  if (typeof pending?.raw !== "function") return undefined;
  // A query without parameters runs as a simple query, whose results are text on every run: a
  // statement Bun prepared would come back in binary from its second run on its connection.
  if (params.length === 0 && typeof pending.simple === "function") pending.simple();
  const raw = await pending.raw();
  if (!Array.isArray(raw.columns)) throw new TypeError("Reylo cannot read this Bun.sql client's rows: its .raw() result describes no columns, which Bun 1.4.4 and later do.");
  const binary = prepare && params.length > 0 ? (oid: number) => BUN_BINARY.has(oid) : undefined;
  const load: Load = async (sql) => ((await c.unsafe(sql).raw()) as (BufferLike | null)[][]).map((r) => r.map((v) => (v === null ? null : textOf(v))));
  return await rawRows(raw, columns, c, load, row, "Reylo cannot read this Bun.sql client's rows: they are not an array of each value's bytes.", binary);
}

// ---------------------------------------------------------------------------------------------
// Bun.sql (Postgres and MySQL): Bun decodes in native code, so its values are converted to Reylo's.

/**
 * The kinds Bun.sql can give in another form: a BIGINT as its digits (a BigInt with `bigint: true`),
 * a date and a timestamp as a Date read as UTC, a timestamptz as a Date, an int4[] and float4[] as a
 * typed array, an array of a type Bun does not parse (an enum's, uuid, name, a range) as its text,
 * a pgvector vector and an hstore as their text, MySQL's TINYINT(1) and a comparison as a number or
 * a BIGINT's digits, and its BIT(1) as a boolean; and an interval as Postgres's text, which Reylo writes as
 * an ISO 8601 duration. A row value, a geometric type, a range and a MySQL SET are their text, which
 * Reylo reads as an object, an array or a Set, and a MySQL ENUM's text is checked.
 */
const BUN_KINDS = /^(bool|int8|money|date|timestamp|timestamptz|interval|bytes|vector|hstore|point|line|lseg|box|path|polygon|circle|enum|set)$|^\(|^(multi)?range:|\[\]$|\|/;

/**
 * A column of a result as Bun.sql describes it from 1.4.4 on (`result.columns`): a Postgres oid, or
 * a MySQL protocol type and display length. Older releases have no `columns`.
 */
type BunColumn = { name: string; type: number; length?: number };

// MySQL's protocol types whose Bun.sql value needs the column's type to be read.
const MYSQL_TIMESTAMP = 7, MYSQL_LONGLONG = 8, MYSQL_DATE = 10, MYSQL_DATETIME = 12, MYSQL_NEWDATE = 14;

/** A column to convert: its kind, its array delimiter, and on MySQL its protocol type and a DATETIME's fraction digits. */
type BunWork = readonly [name: string, kind: Kind, delim?: string | undefined, type?: number, digits?: number | undefined];

export function bunRows(rows: unknown[], columns: Columns | undefined, postgres = false): unknown[] {
  if (!Array.isArray(rows) || !rows.length) return Array.isArray(rows) ? rows : [];
  const work = columns ? declaredWork(rows, columns, postgres) : describedWork(rows, postgres);
  if (!work.length) return rows;
  for (const row of rows as Record<string, unknown>[]) {
    for (const [name, kind, delim, type, digits] of work) {
      const v = row[name];
      if (v === null || v === undefined) continue;
      row[name] = kind === "unknown" ? byType(v, name) : bunValue(kind, v, name, delim, postgres, type, digits);
    }
  }
  return rows;
}

/** The columns Bun.sql describes, where it does and they are the module's (by count). */
function described(rows: unknown[], count?: number): BunColumn[] | undefined {
  const d = (rows as { columns?: unknown }).columns;
  return Array.isArray(d) && (count === undefined || d.length === count) && d.every((c) => typeof c?.type === "number" && typeof c.name === "string") ? (d as BunColumn[]) : undefined;
}

/**
 * The module's columns whose kind Bun.sql gives in another form, the last of each name. On MySQL an
 * either that may be a BIGINT, and a DATETIME or TIMESTAMP, also take the result's own column: its
 * type says whether the digits are a BIGINT's or a DECIMAL's, and its length a DATETIME's fraction
 * digits, which a Date drops when they are zeros.
 */
function declaredWork(rows: unknown[], columns: Columns, postgres: boolean): BunWork[] {
  const kept = lastOfEach(columns.map(([name, kind, delim], i) => [name, kind, delim, i] as const)).filter(([, kind]) => BUN_KINDS.test(kind));
  const needsType = (kind: Kind) => kind.split("|").some((k) => k === "int8" || k === "timestamp");
  const meta = !postgres && kept.some(([, kind]) => needsType(kind)) ? described(rows, columns.length) : undefined;
  return kept.map(([name, kind, delim, i]) => {
    const c = meta && needsType(kind) ? meta[i] : undefined;
    return c ? [name, kind, delim, c.type, fractionDigits(c)] : [name, kind, delim];
  });
}

/**
 * A query defined by hand: each column by the type Bun.sql 1.4.4 and later describe, as other
 * drivers' results give them (a Postgres oid's kind, a MySQL BIGINT, DATE, DATETIME and TIMESTAMP);
 * with no description, a Date and a typed array by what they are.
 */
function describedWork(rows: unknown[], postgres: boolean): BunWork[] {
  const meta = described(rows);
  if (!meta) return Object.keys(rows[0] as object).map((name) => [name, "unknown"]);
  const work: BunWork[] = [];
  for (const [name, i] of lastOfEach(meta.map((c, i) => [c.name, i] as const))) {
    const c = meta[i]!;
    if (postgres) {
      const b = builtin(c.type);
      if (!b) work.push([name, "unknown"]);
      else if (BUN_KINDS.test(b.kind)) work.push([name, b.kind, b.delim]);
    } else if (c.type === MYSQL_LONGLONG) work.push([name, "int8"]);
    else if (c.type === MYSQL_DATE || c.type === MYSQL_NEWDATE) work.push([name, "date"]);
    else if (c.type === MYSQL_DATETIME || c.type === MYSQL_TIMESTAMP) work.push([name, "timestamp", undefined, c.type, fractionDigits(c)]);
  }
  return work;
}

/** A MySQL DATETIME's or TIMESTAMP's fraction digits, from its display length: 19 characters, and a point and the digits. */
function fractionDigits(c: BunColumn): number | undefined {
  if ((c.type !== MYSQL_DATETIME && c.type !== MYSQL_TIMESTAMP) || typeof c.length !== "number") return undefined;
  return c.length > 20 ? Math.min(c.length - 20, 6) : 0;
}

/**
 * A value of `kind` as Bun.sql gave it. An either's digits are a MySQL BIGINT's, which Bun gives as
 * text like a DECIMAL's: by the result's column type where Bun describes it, else when they have no
 * point. A MySQL DATETIME's text has its column's fraction digits.
 */
function bunValue(kind: Kind, v: unknown, name: string, delim: string | undefined, postgres: boolean, type?: number, digits?: number): unknown {
  if (typeof v === "string" && kind.includes("|") && kind.split("|").includes("int8") && (type === undefined ? /^[+-]?\d+$/.test(v) : type === MYSQL_LONGLONG)) return BigInt(v);
  const out = coerce(kind, v, name, delim, postgres);
  return digits !== undefined && v instanceof Date && typeof out === "string" ? withFraction(out, digits) : out;
}

function byType(v: unknown, name: string): unknown {
  if (v instanceof Date) return coerce("timestamptz", v, name);
  if (ArrayBuffer.isView(v) && !(v instanceof Uint8Array) && !(v instanceof DataView)) return coerce("float[]", v, name);
  return v;
}

// ---------------------------------------------------------------------------------------------
// Slonik: its query() runs only the queries its `sql` tag builds, and its type parsers are set on
// the pool's connections, where they turn a numeric into a float, a timestamp into epoch
// milliseconds and an interval into seconds. A tag made by the app's own `createSqlTag` with
// `typeParsers` (Slonik 49.12.0 and later) overrides them for its queries alone, so Reylo's passes
// every type that pg or Slonik converts through as the server's text, and decodes the rows itself.
// Each parameter appears once in a statement (engine/core/src/lex.rs), so the statement is the
// tag's template as it stands: its text between the placeholders, and the values in their order.

type SlonikTypeParser = { name: string; parse(value: string): unknown };
type SlonikTag = { unsafe(strings: TemplateStringsArray, ...values: unknown[]): { typeParsers?: unknown } };
type SlonikModule = { createSqlTag(configuration: { typeParsers: SlonikTypeParser[] }): SlonikTag };

/**
 * The built-in types pg's parsers (pg-types 4) or Slonik's own convert, each passed through as its
 * text; arrays of them follow. pg leaves every other type, and an array of one, as text or as an
 * array of its elements' text.
 */
const SLONIK_TEXT = [
  "bool", "bytea", "int2", "int4", "int8", "oid", "float4", "float8", "numeric", "money", "date", "timestamp", "timestamptz", "interval",
  "json", "jsonb", "point", "circle", "int4range", "int8range", "numrange", "tsrange", "tstzrange", "daterange",
];

/** Kinds whose value must reach Reylo as text: anything else is one a driver already converted, and may have rounded. */
const SLONIK_STRICT = /^(decimal|date|timestamp|timestamptz|interval)(\[\])*$/;

let slonikModule: SlonikModule | undefined;
const slonikTags = new Map<string, SlonikTag>();

type NodeProcess = { argv?: string[]; cwd(): string; getBuiltinModule?(id: string): { createRequire(from: string): (id: string) => unknown } | undefined };

/**
 * The app's own `slonik`, the package its pool came from, loaded the first time a Slonik client is
 * passed. It is required through Node's `createRequire`, reached with `process.getBuiltinModule`
 * (Node 20.16 and Bun) rather than an import, so that a bundler finds nothing to resolve or warn
 * about in an app without Slonik; from the app's entry script, then from its working directory.
 */
function loadSlonik(): SlonikModule {
  if (slonikModule) return slonikModule;
  const proc = (globalThis as { process?: NodeProcess }).process;
  const createRequire = proc?.getBuiltinModule?.("node:module")?.createRequire;
  if (!proc || !createRequire) throw new TypeError(`Reylo runs a query on Slonik with a sql tag from the app's own slonik package, which it loads through Node's process.getBuiltinModule, and this runtime has none.`);
  let failure: unknown;
  for (const from of [proc.argv?.[1], `${proc.cwd()}/`]) {
    if (!from) continue;
    try {
      return (slonikModule = createRequire(from)("slonik") as SlonikModule);
    } catch (e) {
      failure = e;
    }
  }
  throw new TypeError(`Reylo runs a query on Slonik with a sql tag from the app's own slonik package, which it could not load (a server that is bundled keeps slonik external, so that it loads from node_modules at run time): ${(failure as Error).message}`);
}

function slonikTag(c: AnyClient): SlonikTag {
  const { createSqlTag } = loadSlonik();
  // The parsers the app made its pool with, which a pool names on `configuration` and its
  // connections, transactions and savepoints on `clientConfiguration` (not in Slonik's types).
  const own = ((c.configuration ?? c.clientConfiguration)?.typeParsers as SlonikTypeParser[] | undefined)?.map((p) => p.name) ?? [];
  const names = [...new Set([...SLONIK_TEXT, ...own])];
  const key = names.join(",");
  let tag = slonikTags.get(key);
  if (tag === undefined) slonikTags.set(key, (tag = createSqlTag({ typeParsers: names.map((name) => ({ name, parse: (value: string) => value })) })));
  return tag;
}

/**
 * `text`, a statement with `$n` placeholders each used once, as a template literal's strings, and
 * its values in that order: the strings are the text between the marks, the engine's places of
 * the placeholders, and each value is the one its placeholder's number names.
 */
function slonikTemplate(text: string, marks: Marks | undefined, values: readonly unknown[]): [TemplateStringsArray, unknown[]] {
  if (marks === undefined) throw new TypeError(`Reylo cannot run this statement on Slonik, whose sql tag takes a statement as the strings between its values: a query defined by hand needs \`marks\` there, the place of each placeholder in its text, which a build step writes.`);
  const numbers = marks.map(([start, end]) => {
    const written = text.slice(start, end);
    if (!/^\$[0-9]+$/.test(written)) throw new TypeError(`Reylo cannot run this statement on Slonik: its mark at ${start} covers ${JSON.stringify(written)}, not a placeholder such as $1.`);
    return Number(written.slice(1));
  });
  // A query defined by hand may use a parameter twice, which the type check refuses: Slonik's tag
  // binds every value on its own, so the second use would be a parameter of its own type.
  if (new Set(numbers).size < numbers.length) {
    throw new TypeError(`Reylo cannot run this statement on Slonik: it uses a parameter more than once, and Slonik's sql tag binds each use on its own, so the server would type each one separately. Use each parameter once, and select it in a with to read it twice.`);
  }
  const strings: string[] = [];
  let last = 0;
  for (const [start, end] of marks) {
    strings.push(text.slice(last, start));
    last = end;
  }
  strings.push(text.slice(last));
  // A tag takes only what a template literal gives it: frozen strings and their raw form.
  const template = Object.freeze(Object.assign(strings, { raw: Object.freeze([...strings]) })) as unknown as TemplateStringsArray;
  return [template, numbers.map((n) => values[n - 1])];
}

export async function slonik(c: AnyClient, text: string, marks: Marks | undefined, values: readonly unknown[], columns: Columns | undefined): Promise<unknown[]> {
  // An interceptor's transformRow or transformRowAsync renames the columns
  // (slonik-interceptor-field-name-transformation) or rewrites each row, which Reylo's types would
  // no longer describe. It is refused before the statement runs, since a write that committed and
  // then reported an error invites a retry.
  const interceptors = (c.configuration ?? c.clientConfiguration)?.interceptors as { transformRow?: unknown; transformRowAsync?: unknown }[] | undefined;
  if (interceptors?.some((i) => typeof i?.transformRow === "function" || typeof i?.transformRowAsync === "function")) throw refuseSlonikRows();
  const tag = slonikTag(c);
  const [template, ordered] = slonikTemplate(text, marks, values);
  const query = tag.unsafe(template, ...ordered);
  if (query.typeParsers === undefined) {
    throw new TypeError(`Reylo cannot run on this Slonik: its sql tag takes no type parsers per query (createSqlTag's typeParsers option), so its pool's parsers would turn numeric, timestamp and interval values into lossy numbers. Upgrade slonik to 49.12.0 or later.`);
  }
  const result = (await c.query(query)) as { rows: Record<string, unknown>[]; fields: { name: string; dataTypeId: number }[] };
  return slonikRows(result.rows, result.fields, columns);
}

/** Slonik's rows in Reylo's representation, by each column's kind, or for a query defined by hand by its type's oid. */
function slonikRows(rows: Record<string, unknown>[], fields: { name: string; dataTypeId: number }[], columns: Columns | undefined): unknown[] {
  if (!rows.length) return rows;
  const work: readonly (readonly [string, Kind, string?])[] =
    columns ??
    fields.map((f) => {
      const b = builtin(f.dataTypeId);
      return b ? ([f.name, b.kind, b.delim] as const) : ([f.name, "text"] as const);
    });
  // The last column of each name, whose value the row holds: decoded once, by that column's kind.
  const kept = lastOfEach(work);
  for (const row of rows) {
    for (const [name, kind, delim] of kept) row[name] = slonikValue(kind, row[name], name, delim);
  }
  return rows;
}

/** A value as Slonik gave it: the server's text for every type Reylo's tag names, a value pg converted for the rest. */
function slonikValue(kind: Kind, v: unknown, name: string, delim?: string): unknown {
  if (v === null || v === undefined) return null;
  if (typeof v === "string") return textDecoder(kind, delim)(v);
  // pg parses an array's text into an array, each element by the element type's parser, which
  // Reylo's tag passes through for the types it names.
  if (Array.isArray(v) && kind.endsWith("[]")) return v.map((e) => (Array.isArray(e) ? slonikValue(kind, e, name, delim) : slonikValue(elementKind(kind), e, name, delim)));
  if (SLONIK_STRICT.test(kind)) {
    throw new DecodeError(`Column "${name}" reached Reylo already converted by Slonik's driver (${v instanceof Date ? "a Date" : typeof v}), which ignored the query's type parsers, as a custom driverFactory may: Reylo refuses it rather than return a value that may have been rounded.`);
  }
  return coerce(kind, v, name, delim, true);
}

const refuseSlonikRows = () =>
  new TypeError(`Reylo cannot run on this Slonik client: one of its interceptors rewrites every row (transformRow or transformRowAsync, such as slonik-interceptor-field-name-transformation), Reylo's too, so they would not have the columns the query declares. Run Reylo's queries on a pool made without that interceptor.`);

// Slonik's pool, connections, transactions and savepoints, which have query() as pg's do but take
// only the queries its sql tag builds (`slonik`).
function isSlonik(c: AnyClient): boolean {
  return typeof c.maybeOneFirst === "function" && typeof c.anyFirst === "function";
}

// A value as Slonik's sql tag takes it, which is a string, a number, a boolean, a BigInt or null:
// the rest as pg would write them, binary data as `\x` and its hex, an array as its literal, a
// Date as its instant, and an object as its JSON. An omitted parameter binds NULL, as on pg.
function slonikParam(v: unknown): unknown {
  if (v === undefined || v === null) return null;
  if (v instanceof Uint8Array) return asByteaHex(v);
  if (Array.isArray(v)) return arrayLiteral(v);
  if (v instanceof Date) return v.toISOString();
  return typeof v === "object" ? JSON.stringify(v) : v;
}

/** A Postgres statement's rows on each Postgres driver's client, or undefined on any other client. */
async function runPostgres(c: Client, query: QueryModule<"postgres">, { text, values, names, marks }: Bound): Promise<unknown[] | undefined> {
  const type = (i: number) => query.inputs?.[names[i]!]?.type;
  const isJsonParam = (v: unknown, i: number) => isJsonValue(query, names[i]!, v);
  // An array for a SQL array parameter goes as its literal text, which the server reads as
  // the parameter's type, written as pg writes it: Bun.sql binds no JavaScript array
  // (`any($1)` failed with "insufficient data left in message"), postgres.js types an
  // array by its first element ("cannot cast type boolean to boolean[]" for `$1::bool[]`),
  // and the copy of pg inside Neon's Pool and Client threw on a Uint8Array element and
  // left the connection out of step. Each element of a `jsonb[]` is a JSON value.
  const bound = values.map((v, i) => {
    const t = type(i);
    return Array.isArray(v) && t?.kind === "array" && !isJsonParam(v, i) ? arrayLiteral(t.element.type.kind === "json" ? v.map((e) => (e == null ? e : JSON.stringify(e))) : v, query.inputs?.[names[i]!]?.delimiter) : v;
  });
  // A JSON parameter goes to pg's family, PGlite and Neon as its JSON text, and to postgres.js
  // as its JSON text declared `unknown` (below). Bun.sql serializes a value for a JSON
  // parameter itself, so JSON text would be stored as a JSON string, and it types a boolean,
  // a number or an array of either by its JavaScript type ("cannot cast type boolean to
  // jsonb"): it gets an object whose toJSON() returns the value, which it binds as JSON and
  // serializes to exactly its JSON text. So does an unsafe() client without typed().
  const json = () => bound.map((v, i) => (isJsonParam(v, i) ? jsonText(v) : v));
  // Slonik before every other query(): its query() takes only its sql tag's queries.
  if (isSlonik(c)) return await slonik(c, text, marks, json().map(slonikParam), query.columns);
  // Drizzle's driver writes an array as its literal and binds a Uint8Array as bytea, as pg does.
  if (isDrizzlePostgres(c)) return await drizzlePostgres(c, text, json(), query.columns, query.row);
  if (isDrizzleHttp(c)) throw refuseDrizzleHttp();
  // query() before unsafe(): Neon's neon() has both, and its unsafe() builds a fragment
  // for a template instead of running anything. Reylo decodes every value itself, so a client
  // that is none of the drivers' is refused before it runs anything (`takesQuery`).
  if (typeof c.query === "function") return takesQuery(c) ? await queryPostgres(c, text, json(), query.columns) : undefined;
  if (typeof c.unsafe === "function") {
    // Bun.sql's client, transactions, savepoints and reserved connections have commitDistributed(),
    // and postgres.js's have typed(). Anything else with unsafe() is a wrapper around one of them.
    if (typeof c.commitDistributed !== "function" && typeof c.typed !== "function") return undefined;
    // postgres.js reads a string for a date or time parameter through `new Date(x)` and binds
    // its toISOString(), so a timestamp's wall-clock time moved by the app's offset, and
    // `infinity` or a zone name failed with "Invalid time value". Declared `unknown` (705), it
    // is sent as the text, which the server reads as the parameter's type, as it would an
    // undeclared one. Bun.sql (commitDistributed()) sends the text and has no typed().
    // A JSON parameter goes to postgres.js as its JSON text declared `unknown` too: its own
    // serializer would write a value as JSON, but a client Drizzle wraps has that serializer
    // replaced by one that passes the value through (drizzle-orm/postgres-js), and a
    // transaction shows neither. Bun.sql serializes the value itself.
    // postgres.js 4 serializes the values of a statement with an untyped parameter (a number,
    // null) by the types the server described for it, a value declared `unknown` too: JSON text
    // went through the jsonb serializer and was stored as a JSON string from the statement's second
    // run, and an array literal for a `jsonb[]` went through its element's on the first. So where
    // the statement has inputs, every parameter goes to postgres.js as the text Postgres reads,
    // declared `unknown`, and postgres.js never learns its types. postgres.js 3 sends such a value
    // as it is either way. A query without inputs gets its values as they are, as on every driver.
    const bun = typeof c.commitDistributed === "function";
    const mapped = bun ? bound.map((v, i) => (isJsonParam(v, i) ? { toJSON: () => jsonValue(v) } : v)) : query.inputs ? bound.map((v, i) => c.typed(postgresText(query, names[i]!, v), 705)) : bound;
    if (!bun) return await postgresJs(c, text, mapped, query.columns, query.row);
    return (await bunRaw(c, text, mapped, query.columns, query.row)) ?? bunRows(await c.unsafe(text, mapped), query.columns, true);
  }
  if (isNeonFunction(c)) return await neonCallable(c, text, json().map(asByteaHex), query.columns);
  return undefined;
}

/** A Postgres column's text as its kind, with the non-finite numbers inside its JSON as numbers and an array held to its dimensions, as `.run()` gives them (src/execute.ts). */
function postgresDecoder(name: string, kind: Kind, delim: string | undefined, nonFinite: JsonNumbers | undefined, nestedArrays: boolean | undefined): (v: string) => unknown {
  const decode = textDecoder(kind, delim);
  const numbers = nonFinite === undefined ? decode : (v: string) => numbersInJson(decode(v), nonFinite, name);
  return kind.endsWith("[]") && !nestedArrays ? (v) => withinDimensions(numbers(v), kind, name) : numbers;
}

/** Postgres's half of the run time (src/execute.ts, `Runtime`), which `definePostgresQuery` passes. */
export const postgres: Runtime<"postgres"> = { dialect: "postgres", run: runPostgres, readError: readPostgresError, decoder: postgresDecoder };

export { DecodeError, parseArray } from "./values.ts";
