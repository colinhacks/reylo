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
//   (src/values.ts, `coerce`), by each column's kind.
//
// A built-in type (an oid below 10000, the same in every database and version) has its kind here.
// A type the database created (an enum, a domain's array, an extension's type) has the kind the
// module gives its column; for a query defined by hand, one catalog query per client reads which of
// those oids are arrays, cached in a WeakMap keyed by the app's object.
import { coerce, DecodeError, scalarKind, textDecoder, utf8, type Columns, type Decoder, type Kind, type ScalarKind } from "./values.ts";
import { lexPlaceholders } from "./lex.ts";

type AnyClient = Record<string, any>;
type Field = { name: string; oid: number };

/** Below it the built-in types, whose oids every database shares (FirstGenbkiObjectId). */
const FIRST_USER_OID = 10000;

const SCALAR_OIDS: Readonly<Record<number, ScalarKind>> = {
  16: "bool", 17: "bytes", 20: "int8", 21: "int", 23: "int", 26: "int", 700: "float", 701: "float",
  1700: "decimal", 1082: "date", 1114: "timestamp", 1184: "timestamptz", 1186: "interval", 114: "json", 3802: "json",
};

// Every built-in array type and its element, as Postgres 18's pg_type lists them (none was ever
// renumbered): `_record` is left out, since an array of row values is its text.
const ARRAY_OIDS = new Map<number, number>([
  [143, 142], [199, 114], [271, 5069], [629, 628], [651, 650], [719, 718], [775, 774], [791, 790], [1000, 16], [1001, 17],
  [1002, 18], [1003, 19], [1005, 21], [1006, 22], [1007, 23], [1008, 24], [1009, 25], [1010, 27], [1011, 28], [1012, 29],
  [1013, 30], [1014, 1042], [1015, 1043], [1016, 20], [1017, 600], [1018, 601], [1019, 602], [1020, 603], [1021, 700],
  [1022, 701], [1027, 604], [1028, 26], [1034, 1033], [1040, 829], [1041, 869], [1115, 1114], [1182, 1082], [1183, 1083],
  [1185, 1184], [1187, 1186], [1231, 1700], [1263, 2275], [1270, 1266], [1561, 1560], [1563, 1562], [2201, 1790],
  [2207, 2202], [2208, 2203], [2209, 2204], [2210, 2205], [2211, 2206], [2949, 2970], [2951, 2950], [3221, 3220],
  [3643, 3614], [3644, 3642], [3645, 3615], [3735, 3734], [3770, 3769], [3807, 3802], [3905, 3904], [3907, 3906],
  [3909, 3908], [3911, 3910], [3913, 3912], [3927, 3926], [4073, 4072], [4090, 4089], [4097, 4096], [4192, 4191],
  [5039, 5038], [6150, 4451], [6151, 4532], [6152, 4533], [6153, 4534], [6155, 4535], [6157, 4536],
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
// of domains and that base type's name (an array of row values, or of a type whose own output is
// an array, stays text). An oid goes out as int8, since one above 2^31 would wrap as int4.
const catalogSql = (oids: number[]) => {
  const list = `'{${oids.map((o) => Math.trunc(o)).join(",")}}'::pg_catalog.oid[]`;
  return `with recursive d(oid, base) as (
  select oid, typbasetype from pg_catalog.pg_type where typtype = 'd'
  union all
  select d.oid, t.typbasetype from d join pg_catalog.pg_type t on t.oid = d.base where t.typtype = 'd'
)
select e.typarray::int8::text, b.oid::int8::text, (b.typtype = 'c' or b.typcategory = 'A')::text, e.typdelim::text, e.oid::int8::text, b.typname::text
from pg_catalog.pg_type e
join pg_catalog.pg_type b on b.oid = coalesce((select d.base from d join pg_catalog.pg_type t on t.oid = d.base where d.oid = e.oid and t.typtype <> 'd'), e.oid)
where e.typarray = any(${list}) or (e.oid = any(${list}) and e.typcategory <> 'A')`;
};

/**
 * Reads the kinds of the oids the client has not met yet: a type the database created by its
 * name, as the engine names it (pgvector's `vector` is its numbers, an enum its text), and an array
 * of one its element's kind.
 */
async function learn(key: object, oids: Iterable<number>, load: Load): Promise<void> {
  const known = learnedOf(key);
  const missing = new Set([...oids].filter((oid) => !known.has(oid)));
  if (!missing.size) return;
  const rows = await load(catalogSql([...missing]));
  // An array oid takes its element's kind, never the text its own row as a type would give it.
  const arrays = new Set<number>();
  for (const [array, base, isText, delim, oid, name] of rows) {
    const element = isText === "true" ? undefined : (builtin(Number(base))?.kind ?? (name ? scalarKind(name, "postgres") : "text"));
    if (missing.has(Number(array))) {
      arrays.add(Number(array));
      known.set(Number(array), element === undefined || element.endsWith("[]") ? { kind: "text", delim: "," } : { kind: `${element}[]`, delim: delim ?? "," });
    }
    if (missing.has(Number(oid)) && !arrays.has(Number(oid))) known.set(Number(oid), { kind: element ?? "text", delim: "," });
  }
  for (const oid of missing) if (!known.has(oid)) known.set(oid, { kind: "text", delim: "," });
}

/**
 * The decoder of each field a result named: a built-in type's by its oid, a created type's by the
 * module's column (when the module's columns match the result's) or by what the catalog said.
 */
async function fieldDecoders(fields: Field[], columns: Columns | undefined, key: object, load: Load): Promise<Decoder[]> {
  const byColumn = columns && columns.length === fields.length ? columns : undefined;
  const created = fields.filter((f) => f.oid >= FIRST_USER_OID);
  const known = learnedOf(key);
  if (byColumn) created.forEach((f) => known.has(f.oid) || known.set(f.oid, { kind: byColumn[fields.indexOf(f)]![1], delim: byColumn[fields.indexOf(f)]![2] ?? "," }));
  else if (created.length) await learn(key, created.map((f) => f.oid), load);
  return fields.map((f) => {
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

/**
 * Runs a Postgres statement on a client whose query() Reylo calls: pg's Client, PoolClient and
 * Pool, Neon's, @vercel/postgres's, pg-promise's database, tasks and transactions, PGlite and its
 * transactions, and Neon's neon(). Each is told apart by what it has.
 */
export async function queryPostgres(c: AnyClient, text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  // pg-promise formats the values into the text itself (reading $1 inside a string literal
  // too) unless it is handed a { text, values } object, which it binds.
  if (typeof c.manyOrNone === "function") return pgPromise(c, text, values, columns);
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
  if (typeof c.transaction === "function") {
    const load: Load = async (sql) => (await c.query(sql, [], { arrayMode: true, fullResults: true, types: asText })).rows;
    return pgFamily(c, (types) => c.query(text, values, { fullResults: true, arrayMode: false, types }), columns, load);
  }
  const result = await c.query(text, values);
  return Array.isArray(result) ? result : result.rows;
}

// pg-promise sends a { text, values, types } object to pg as it is, on the database, a task or a
// transaction. Its `receive` event runs on every result, Reylo's too, and may rewrite the rows: on
// the database Reylo reads it from the options and refuses; a task exposes no options, so a
// receive that renamed the columns is caught by the rows' keys.
async function pgPromise(db: AnyClient, text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  if (typeof db.$config?.options?.receive === "function") throw refuseReceive();
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
const builtinOids = [...Object.keys(SCALAR_OIDS).map(Number), ...ARRAY_OIDS.keys()];
const keepText = (x: string) => x;

async function pglite(db: AnyClient, text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  // A statement met for the first time names every built-in type Reylo decodes, and every oid the
  // instance has a parser for (its arrays, and the app's); a transaction has no table to read.
  let named = pgliteOids.get(text);
  const oids = named ?? new Set([...builtinOids, ...Object.keys(db.parsers ?? {}).map(Number)]);
  const parsers: Record<number, Decoder> = {};
  for (const oid of oids) parsers[oid] = builtinDecoder(oid) ?? keepText;
  const result = await db.query(text, values, { rowMode: "object", parsers });
  const rows: Record<string, unknown>[] = result.rows;
  const fields: Field[] = (result.fields ?? []).map((f: { name: string; dataTypeID: number }) => ({ name: f.name, oid: f.dataTypeID }));
  // Created types, which the table kept as text, and an oid the table did not name, which went
  // through the instance's parser.
  const after = fields.map((f, i) => [f, i] as const).filter(([f]) => f.oid >= FIRST_USER_OID || !oids.has(f.oid));
  if (after.length) {
    const load: Load = async (sql) => (await db.query(sql, [], { rowMode: "array", parsers: { 25: keepText } })).rows;
    const decoders = await fieldDecoders(fields, columns, db, load);
    const kinds = fields.map((f, i) => (columns && columns.length === fields.length ? columns[i]![1] : (builtin(f.oid)?.kind ?? learnedOf(db).get(f.oid)?.kind ?? "text")));
    for (const [f, i] of after) {
      const decode = decoders[i]!;
      for (const row of rows) {
        const v = row[f.name];
        if (v !== null && v !== undefined) row[f.name] = typeof v === "string" ? decode(v) : coerce(kinds[i]!, v, f.name);
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

export async function postgresJs(c: AnyClient, text: string, params: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  // Prepared, as postgres.js prepares its own queries: its unsafe() prepares nothing by default,
  // so every run with a parameter took two round trips (Parse and Describe, then Bind and Execute)
  // where a statement prepared on the connection takes one. A client made with `prepare: false`,
  // as one behind a pooler in transaction mode is, still prepares nothing: its option wins.
  const pending = c.unsafe(text, params, { prepare: true });
  // A client whose query has no raw() (a wrapper of postgres.js's shape): its values, given their kinds.
  if (typeof pending?.raw !== "function") return bunRows(await pending, columns);
  const raw = await pending.raw();
  const fields: Field[] = (raw.columns ?? []).map((f: { name: string; type: number }) => ({ name: f.name, oid: f.type }));
  if (!raw.length) return [];
  // A row transform (`transform: { row }`) runs on `.raw()` rows too, and leaves them not an array of each value's bytes.
  for (const row of raw) {
    if (!Array.isArray(row) || row.length !== fields.length || !row.every((v) => v === null || v instanceof Uint8Array)) {
      throw new TypeError(`Reylo cannot read this postgres.js client's rows: its transform.row rewrites every row, Reylo's too, so they would not have the types the query declares. Run Reylo's queries on a postgres() instance without transform.row.`);
    }
  }
  const load: Load = async (sql) => ((await c.unsafe(sql).raw()) as (BufferLike | null)[][]).map((r) => r.map((v) => (v === null ? null : textOf(v))));
  const decoders = await fieldDecoders(fields, columns, c, load);
  // The module's names, which a `transform.column` of the app's does not rename; a query defined by
  // hand has the server's.
  const names = columns && columns.length === fields.length ? columns.map(([name]) => name) : fields.map((f) => f.name);
  const n = names.length;
  const out = new Array(raw.length);
  for (let r = 0; r < raw.length; r++) {
    const values = raw[r] as (BufferLike | null)[];
    const row: Record<string, unknown> = {};
    for (let i = 0; i < n; i++) {
      const v = values[i];
      row[names[i]!] = v === null || v === undefined ? null : decoders[i]!(textOf(v));
    }
    out[r] = row;
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// Bun.sql (Postgres and MySQL): Bun decodes in native code, so its values are converted to Reylo's.

/**
 * The kinds Bun.sql can give in another form: a BIGINT as its digits (a BigInt with `bigint: true`),
 * a date and a timestamp as a Date read as UTC, a timestamptz as a Date, an int4[] and float4[] as a
 * typed array, an array of a type Bun does not parse (an enum's, uuid, name, a range) as its text,
 * a pgvector vector and an hstore as their text, MySQL's TINYINT(1) and a comparison as a number or
 * a BIGINT's digits, and its BIT(1) as a boolean; and an interval as Postgres's text, which Reylo writes as
 * an ISO 8601 duration.
 */
const BUN_KINDS = /^(bool|int8|date|timestamp|timestamptz|interval|bytes|vector|hstore)$|\[\]$|\|/;

export function bunRows(rows: unknown[], columns: Columns | undefined): unknown[] {
  if (!Array.isArray(rows) || !rows.length) return Array.isArray(rows) ? rows : [];
  const first = rows[0] as Record<string, unknown>;
  // A query defined by hand carries no kinds: only what Bun's own types say is converted.
  const work: (readonly [string, Kind, string?])[] = columns ? columns.filter(([, kind]) => BUN_KINDS.test(kind)) : Object.keys(first).map((name) => [name, "unknown"] as const);
  if (!work.length) return rows;
  for (const row of rows as Record<string, unknown>[]) {
    for (const [name, kind, delim] of work) {
      const v = row[name];
      if (v === null || v === undefined) continue;
      row[name] = kind === "unknown" ? byType(v, name) : bunValue(kind, v, name, delim);
    }
  }
  return rows;
}

/** A value of `kind` as Bun.sql gave it: an either's digits are a MySQL BIGINT's, which Bun gives as a string. */
function bunValue(kind: Kind, v: unknown, name: string, delim?: string): unknown {
  if (typeof v === "string" && kind.includes("|") && kind.split("|").includes("int8") && /^[+-]?\d+$/.test(v)) return BigInt(v);
  return coerce(kind, v, name, delim);
}

/** A value whose kind is not known: a typed array an array, and a Date, which no kind is, the instant's text. */
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
  throw new TypeError(`Reylo runs a query on Slonik with a sql tag from the app's own slonik package, which it could not load: ${(failure as Error).message}`);
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

/** `text`, a statement with `$n` placeholders each used once, as a template literal's strings, and its values in that order. */
function slonikTemplate(text: string, values: readonly unknown[]): [TemplateStringsArray, unknown[]] {
  const { params, placeholders } = lexPlaceholders(text, "postgres");
  // A query defined by hand may use a parameter twice, which the type check refuses: Slonik's tag
  // binds every value on its own, so the second use would be a parameter of its own type.
  if (placeholders.length > params.length) {
    throw new TypeError(`Reylo cannot run this statement on Slonik: it uses a parameter more than once, and Slonik's sql tag binds each use on its own, so the server would type each one separately. Use each parameter once, and select it in a with to read it twice.`);
  }
  const strings: string[] = [];
  let last = 0;
  for (const p of placeholders) {
    strings.push(text.slice(last, p.start));
    last = p.end;
  }
  strings.push(text.slice(last));
  // A tag takes only what a template literal gives it: frozen strings and their raw form.
  const template = Object.freeze(Object.assign(strings, { raw: Object.freeze([...strings]) })) as unknown as TemplateStringsArray;
  return [template, placeholders.map((p) => values[Number(p.name) - 1])];
}

export async function slonik(c: AnyClient, text: string, values: readonly unknown[], columns: Columns | undefined): Promise<unknown[]> {
  const tag = slonikTag(c);
  const [template, ordered] = slonikTemplate(text, values);
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
  // An interceptor's transformRow renames the columns (slonik-interceptor-field-name-transformation)
  // or rewrites the row, which Reylo's types would no longer describe.
  if (columns && !columns.every(([name]) => Object.prototype.hasOwnProperty.call(rows[0], name))) throw refuseSlonikRows();
  for (const row of rows) {
    for (const [name, kind, delim] of work) row[name] = slonikValue(kind, row[name], name, delim);
  }
  return rows;
}

/** A value as Slonik gave it: the server's text for every type Reylo's tag names, a value pg converted for the rest. */
function slonikValue(kind: Kind, v: unknown, name: string, delim?: string): unknown {
  if (v === null || v === undefined) return null;
  if (typeof v === "string") return textDecoder(kind, delim)(v);
  // pg parses an array's text into an array, each element by the element type's parser, which
  // Reylo's tag passes through for the types it names.
  if (Array.isArray(v) && kind.endsWith("[]")) return v.map((e) => (Array.isArray(e) ? slonikValue(kind, e, name, delim) : slonikValue(kind.slice(0, -2), e, name, delim)));
  if (SLONIK_STRICT.test(kind)) {
    throw new DecodeError(`Column "${name}" reached Reylo already converted by Slonik's driver (${v instanceof Date ? "a Date" : typeof v}), which ignored the query's type parsers, as a custom driverFactory may: Reylo refuses it rather than return a value that may have been rounded.`);
  }
  return coerce(kind, v, name, delim);
}

const refuseSlonikRows = () =>
  new TypeError(`Reylo cannot run on this Slonik pool: an interceptor rewrites every row (transformRow, such as slonik-interceptor-field-name-transformation), Reylo's too, so they would not have the columns the query declares. Run Reylo's queries on a pool made without that interceptor.`);

export { DecodeError, parseArray } from "./values.ts";
