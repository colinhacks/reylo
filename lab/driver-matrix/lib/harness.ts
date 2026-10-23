// The scenario runner every driver job shares. A job describes one driver version against one
// server: the client shapes it offers (pool, connection, checked-out connection, transaction,
// savepoint, reserved connection, ...) and a raw row count that reads through the driver
// itself, never through reylo. Each scenario runs a statement through reylo's
// defineQuery(...).run(client, params) and asserts the result; a record per scenario is appended
// to the JSON Lines file named by DM_OUT.
//
// Everything here is erasable TypeScript, so Node runs it with its built-in type stripping and
// Bun runs it as is. The runtime under test is imported by relative path.
import { appendFileSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { defineQuery } from "../../../src/index.ts";

export type Dialect = "postgres" | "mysql" | "sqlite";
/** How `run` must answer on this client: the value itself (better-sqlite3, node:sqlite, bun:sqlite) or a promise. */
export type Kind = "sync" | "async";
export type Client = any;
export type Body = (client: Client) => Promise<unknown>;

/** The four transaction scenarios, written against the shape's own transaction API. */
export type TxOps = {
  /** Runs body in a transaction that commits. */
  commit(body: Body): Promise<unknown>;
  /** Runs body in a transaction that is then rolled back (a thrown error or an explicit rollback). */
  rollback(body: Body): Promise<unknown>;
  /** Runs outer, then inner inside a savepoint that is rolled back, then commits. */
  savepoint?(outer: Body, inner: Body): Promise<unknown>;
  /** Runs body, which rejects with a driver error, the way a user writes it: the transaction must roll back and the error reach the caller. */
  failing(body: Body): Promise<unknown>;
};

export type Shape = {
  name: string;
  kind: Kind;
  /** Runs fn with a fresh client object of this shape, and cleans up after it. Omitted for a shape that only has transaction scenarios. */
  scope?: (fn: (client: Client) => Promise<void>) => Promise<void>;
  tx?: TxOps;
  /** better-sqlite3 and bun:sqlite: the database whose synchronous db.transaction(fn) is under test. */
  syncTx?: Client;
  /** Scenarios that do not apply to this shape, with the reason. */
  skip?: Partial<Record<string, string>>;
  /** "shape only": the client is real but its transport is stubbed. */
  live?: boolean;
  /** The scope runs inside an open transaction, so a write is not visible to the raw count until it ends. */
  inTransaction?: boolean;
  /** reylo does not support this client: run() must reject with its own TypeError matching this. Only S9 runs. */
  refused?: RegExp;
  /** The statements without their `inputs`, as a query defined by hand or built before 2026-10-12 has them (a failing control). */
  untyped?: boolean;
  /** reylo refuses any statement with parameters on this client, with a TypeError matching this whose cause is the driver's error. */
  paramsRefused?: RegExp;
};

export type Job = {
  pkg: string;
  version: string;
  target: string;
  dialect: Dialect;
  shapes: Shape[];
  /** Rows in the scratch table with this tag, read through the driver without reylo. */
  count(tag: string): Promise<number>;
  close?(): Promise<unknown>;
  /** Scenarios that do not apply to the whole job, with the reason. */
  skip?: Partial<Record<string, string>>;
};

/** "unsupported": reylo refused the client or the statement with its own TypeError, as its contract says for this shape. */
export type Status = "pass" | "fail" | "skip" | "not run" | "unsupported";
export type Result = {
  pkg: string;
  version: string;
  runtime: string;
  target: string;
  shape: string;
  scenario: string;
  status: Status;
  live: boolean;
  error?: string;
  detail?: string;
};

export const runtime = typeof (globalThis as any).Bun !== "undefined" ? `bun ${(globalThis as any).Bun.version}` : `node ${process.versions.node}`;
export const here = fileURLToPath(new URL("..", import.meta.url));
export const requireFrom = createRequire(new URL("../package.json", import.meta.url));

/** --pkg <alias> --version <version> [--target <label>] [--from <dir>] [key value ...] */
export function args(): Record<string, string> {
  const out: Record<string, string> = {};
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i += 2) out[argv[i]!.replace(/^--/, "")] = argv[i + 1]!;
  return out;
}

export function seedSql(dialect: "postgres" | "mysql" | "sqlite"): string {
  return readFileSync(new URL(`../seed/${dialect}.sql`, import.meta.url), "utf8");
}

/** Splits a seed file into statements (no statement in them contains a semicolon). */
export function statements(sql: string): string[] {
  return sql
    .split("\n")
    .filter((line) => !line.startsWith("--"))
    .join("\n")
    .split(";")
    .map((s) => s.trim())
    .filter(Boolean);
}

const out = process.env.DM_OUT;
export function emit(result: Result): void {
  const line = JSON.stringify(result);
  if (out) appendFileSync(out, line + "\n");
  const mark = result.status === "pass" ? "ok  " : result.status === "fail" ? "FAIL" : result.status === "skip" ? "skip" : result.status === "unsupported" ? "unsp" : "----";
  console.log(`${mark} ${result.pkg}@${result.version} [${result.target}] ${result.shape} :: ${result.scenario}${result.error ? ` -- ${result.error}` : ""}`);
}

// ---------------------------------------------------------------------------------------------
// Assertions and error description

/** A breach of reylo's contract seen by the harness (for example run() throwing synchronously on an asynchronous client). */
export class Violation extends Error {}

class Mismatch extends Error {}

export function describe(e: unknown): string {
  if (e instanceof Error) {
    const fields = codeFields(e);
    const tags = Object.entries(fields)
      .filter(([k]) => k !== "message")
      .map(([k, v]) => `${k}=${String(v)}`)
      .join(" ");
    return `${e.constructor?.name ?? "Error"}: ${oneLine(e.message)}${tags ? ` [${tags}]` : ""}`;
  }
  try {
    return `non-Error thrown: ${oneLine(JSON.stringify(e))}`;
  } catch {
    return `non-Error thrown: ${String(e)}`;
  }
}

export function oneLine(s: string, max = 300): string {
  const t = String(s).replace(/\s+/g, " ").trim();
  return t.length > max ? t.slice(0, max) + "..." : t;
}

function codeFields(e: any): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const key of ["code", "errno", "sqlState", "sqlstate", "errcode", "errstr", "rawCode", "extendedCode", "severity"]) {
    if (e?.[key] !== undefined && e?.[key] !== null && e?.[key] !== "") out[key] = e[key];
  }
  if (e?.cause && typeof e.cause === "object") {
    for (const key of ["code", "errno", "sqlState"]) if (e.cause[key] !== undefined) out[`cause.${key}`] = e.cause[key];
  }
  if (e?.body && typeof e.body === "object" && e.body.code !== undefined) out["body.code"] = e.body.code;
  return out;
}

function eq(actual: unknown, expected: unknown, what: string): void {
  const a = JSON.stringify(normalize(actual));
  const b = JSON.stringify(normalize(expected));
  if (a !== b) throw new Mismatch(`${what}: expected ${b}, got ${oneLine(safeJson(actual), 400)}`);
}

function safeJson(v: unknown): string {
  try {
    return JSON.stringify(v, (_k, x) => (typeof x === "bigint" ? `${x}n` : x)) ?? String(v);
  } catch {
    return String(v);
  }
}

// Every scenario but S10 tests the dispatch, not the decoding: a BIGINT count, a TINYINT(1) boolean
// or a numeric string compares equal to the plain value, so a row fails only when it carries the
// wrong data. S10 compares each value with its JavaScript type (typedEq).
function normalize(v: unknown): unknown {
  if (typeof v === "bigint") return Number(v);
  if (typeof v === "boolean") return v ? 1 : 0;
  if (typeof v === "string" && /^-?\d+$/.test(v)) return Number(v);
  if (Array.isArray(v)) return v.map(normalize);
  if (v && typeof v === "object") return Object.fromEntries(Object.keys(v).sort().map((k) => [k, normalize((v as any)[k])]));
  return v;
}

type ErrorExpectation = { codes: (string | number | RegExp)[]; message: RegExp };
const expected: Record<Dialect, { syntax: ErrorExpectation; dup: ErrorExpectation }> = {
  postgres: {
    syntax: { codes: ["42601"], message: /syntax error/i },
    dup: { codes: ["23505"], message: /duplicate key/i },
  },
  mysql: {
    syntax: { codes: ["ER_PARSE_ERROR", 1064, "1064"], message: /errno 1064|SQL syntax/i },
    dup: { codes: ["ER_DUP_ENTRY", 1062, "1062"], message: /errno 1062|Duplicate entry/i },
  },
  sqlite: {
    // Turso's engine 0.4 words its syntax error "unexpected token 'selec' at offset 0".
    syntax: { codes: [/^(ERR_)?SQLITE_ERROR$/, 1], message: /syntax error|unexpected token/i },
    dup: { codes: [/SQLITE_CONSTRAINT/, 19, 1555, 2067], message: /UNIQUE constraint failed|SQLITE_CONSTRAINT/i },
  },
};

/** The rejection must be the driver's own error with its code, not reylo's TypeError or a harness violation. */
function expectDriverError(e: unknown, dialect: Dialect, which: "syntax" | "dup"): string {
  if (e instanceof Violation) throw e;
  if (e instanceof Mismatch) throw e;
  if (e instanceof TypeError && /reylo/.test(e.message)) throw new Mismatch(`reylo's TypeError instead of the driver's error: ${describe(e)}`);
  if (!(e instanceof Error)) throw new Mismatch(`rejected with a non-Error: ${describe(e)}`);
  const want = expected[dialect][which];
  const fields = codeFields(e);
  const values = Object.values(fields);
  const byCode = values.some((v) => want.codes.some((c) => (c instanceof RegExp ? c.test(String(v)) : c === v)));
  if (byCode) return describe(e);
  if (want.message.test(e.message)) return `${describe(e)} (matched by message: the error carries no code field)`;
  throw new Mismatch(`not the driver's ${which === "dup" ? "unique-violation" : "syntax"} error: ${describe(e)}`);
}

// ---------------------------------------------------------------------------------------------
// Calling reylo, with its return discipline checked on every call

type Ctx = { job: Job; shape: Shape; kind: Kind; refusals: unknown[] };

function isThenable(v: unknown): v is PromiseLike<unknown> {
  return v != null && (typeof v === "object" || typeof v === "function") && typeof (v as any).then === "function";
}

/** q.run(client, params), or q.with(client)(params) when via is "with". */
export async function call(ctx: Ctx, q: any, client: Client, params?: Record<string, unknown>, via: "run" | "with" = "run"): Promise<any> {
  let r: unknown;
  try {
    if (via === "with") r = params === undefined ? q.with(client)() : q.with(client)(params);
    else r = params === undefined ? q.run(client) : q.run(client, params);
  } catch (e) {
    throw new Violation(`run threw synchronously: ${describe(e)}`);
  }
  if (!isThenable(r)) throw new Violation(`run returned ${r === null ? "null" : typeof r} instead of a promise`);
  try {
    return await r;
  } catch (e) {
    if (ctx.shape.paramsRefused && e instanceof TypeError && ctx.shape.paramsRefused.test(e.message)) ctx.refusals.push(e);
    throw e;
  }
}

// ---------------------------------------------------------------------------------------------
// The statements, per dialect

const array = (name: string) => ({ kind: "array", of: { kind: "scalar", name } });

export function queries(d: Dialect, typed = true) {
  const t = (n: string) => (d === "sqlite" ? n : `driver_matrix.${n}`);
  const cast = (p: string, ty: string) => (d === "postgres" ? `${p}::${ty}` : p);
  const q = (sql: string, params: string[], cardinality: "many" | "one" | "maybeOne" | "none" = "many", extra: Record<string, unknown> = {}) =>
    defineQuery<any, any, any, any>({ dialect: d, sql, params, cardinality, ...extra, ...(typed ? {} : { inputs: undefined }) });
  const repeatedSql = `select id, name from ${t("people")} where id = :id and name = (select name from ${t("people")} where id = :id)`;
  const repeatedText =
    d === "postgres"
      ? `select id, name from ${t("people")} where id = $1 and name = (select name from ${t("people")} where id = $1)`
      : `select id, name from ${t("people")} where id = ? and name = (select name from ${t("people")} where id = ?)`;
  return {
    params: q(`select id, name, active, score from ${t("people")} where id = :i and name = :t and active = :b and ${cast(":nul", "int")} is null`, ["i", "t", "b", "nul"], "one"),
    omitted: q(`select id from ${t("people")} where id = :i and ${cast(":omit", "text")} is null`, ["i", "omit"], "many"),
    // `inputs` as the loader writes it from the analysis (src/loader.ts); without it reylo converts no array.
    list: q(`select id from ${t("people")} where id = any(:ids) and name = any(:names) order by id`, ["ids", "names"], "many", { inputs: { ids: { type: array("int4") }, names: { type: array("text") } } }),
    typedArrays: q(`select :b::bool[]::text as b, extract(epoch from (:d::timestamptz[])[1])::text as d, array(select encode(x, 'hex') from unnest(:x::bytea[]) as x)::text as x`, ["b", "d", "x"], "one", { inputs: { b: { type: array("bool") }, d: { type: array("timestamptz") }, x: { type: array("bytea") } } }),
    jsonArrays: q(`select (:a::jsonb)::text as a, (:e::jsonb)::text as e`, ["a", "e"], "one", { inputs: { a: { type: { kind: "json" } }, e: { type: { kind: "json" } } } }),
    // Postgres and MySQL type a parameter read as JSON as JSON (`::jsonb`, json_extract's document), and the text comes back to be parsed, since each database spaces it its own way.
    jsonValues: q(
      ["s", "n", "b", "t", "o", "a", "e", "z"].map((p) => (d === "postgres" ? `(:${p}::jsonb)::text as ${p}` : `cast(json_extract(:${p}, '$') as char) as ${p}`)).join(", ").replace(/^/, "select "),
      ["s", "n", "b", "t", "o", "a", "e", "z"],
      "one",
      { inputs: Object.fromEntries(["s", "n", "b", "t", "o", "a", "e", "z"].map((p) => [p, { type: { kind: "json" } }])) },
    ),
    // A timestamptz is an instant and takes a Date; a timestamp or a date has no zone and takes its text (MySQL has no timestamptz).
    times: q(
      d === "postgres"
        ? `select (:tz::timestamptz = '2026-01-02T03:04:05.006Z'::timestamptz and :named::timestamptz = '2026-01-02T03:04:05.006Z'::timestamptz) as tz, :at::timestamp::text as at, :day::date::text as day`
        : `select true as tz, date_format(cast(:at as datetime(3)), '%Y-%m-%d %H:%i:%s.%f') as at, cast(cast(:day as date) as char) as day`,
      d === "postgres" ? ["tz", "named", "at", "day"] : ["at", "day"],
      "one",
      { inputs: { tz: { type: { kind: "scalar", name: "timestamptz" } }, named: { type: { kind: "scalar", name: "timestamptz" } }, at: { type: { kind: "scalar", name: "timestamp" } }, day: { type: { kind: "scalar", name: "date" } } } },
    ),
    binary: q(d === "postgres" ? `select encode(:a::bytea, 'hex') as a, encode(:b::bytea, 'hex') as b` : `select lower(hex(:a)) as a, lower(hex(:b)) as b`, ["a", "b"], "one"),
    many: q(`select id, name from ${t("people")} order by id`, [], "many"),
    one: q(`select id, name from ${t("people")} where id = :id`, ["id"], "one"),
    maybeOne: q(`select id, name from ${t("people")} where id = :id`, ["id"], "maybeOne"),
    maybeOneOfTwo: q(`select id from ${t("people")} where id <= :id order by id`, ["id"], "maybeOne"),
    insert: q(`insert into ${t("scratch")} (tag, n) values (:tag, :n)`, ["tag", "n"], "none"),
    returning: q(`insert into ${t("scratch")} (tag, n) values (:tag, :n) returning tag, n`, ["tag", "n"], "one"),
    byTag: q(`select n from ${t("scratch")} where tag = :tag`, ["tag"], "one"),
    repeated: q(repeatedSql, ["id"], "one"),
    repeatedMapper: q(repeatedSql, ["id"], "one", { text: repeatedText, order: d === "postgres" ? ["id"] : ["id", "id"] }),
    syntax: q(`selec 1`, [], "many"),
    // The columns a build step writes for the statement (src/loader.ts, src/values.ts).
    typed: q(`select ${Object.keys(TYPED[d]).join(", ")} from ${t("typed")} where id = 1`, [], "one", { columns: TYPED_COLUMNS[d] }),
    // Each column of the `typed` row compared with a parameter, with the types the engine gives
    // those parameters (src/render.ts's round-trip table): the row `typed` returns goes back in.
    roundTrip: q(
      `select ${Object.keys(ROUND_TRIP[d]).map((c) => `${ROUND_TRIP_SQL[d](c)} as ${c}`).join(", ")} from ${t("typed")} where id = :id`,
      [...Object.keys(ROUND_TRIP[d]), "id"],
      "one",
      { inputs: Object.fromEntries([...Object.entries(ROUND_TRIP[d]), ["id", { kind: "scalar", name: "int4" }]].map(([c, type]) => [c, { type }])), columns: Object.keys(ROUND_TRIP[d]).map((c) => [c, "bool"]) },
    ),
    // An instant past 9999 and one before 1, which come back as `+010000-…Z` and `-000001-…Z`.
    farInstants: q(`select '10000-01-01 00:00:00+00'::timestamptz as far, '0002-01-01 00:00:00+00 BC'::timestamptz as bc`, [], "one", { columns: [["far", "timestamptz"], ["bc", "timestamptz"]] }),
    farBack: q(`select :far::timestamptz = '10000-01-01 00:00:00+00'::timestamptz as far, :bc::timestamptz = '0002-01-01 00:00:00+00 BC'::timestamptz as bc`, ["far", "bc"], "one", { inputs: { far: { type: { kind: "scalar", name: "timestamptz" } }, bc: { type: { kind: "scalar", name: "timestamptz" } } }, columns: [["far", "bool"], ["bc", "bool"]] }),
    typedBig: q(`select id, big from ${t("typed")} where id = 1`, [], "one", { columns: [["id", "int"], ["big", d === "sqlite" ? "int" : "int8"]] }),
    // SQLite's column declared BIGINT, which the engine types int8.
    typedWide: q(`select id, wide from ${t("typed")} where id = 1`, [], "one", { columns: [["id", "int"], ["wide", "int8"]] }),
    // SQLite: JSON the statement builds, with the kinds the loader writes from the engine's types
    // (tests/representation.test.ts pins those types): `json` for each column a constructor
    // builds, `text` for one that holds JSON text the statement did not build.
    builtJson: q(
      `select json_object('user', json_object('id', p.id, 'name', p.name), 'scores', json((select json_group_array(json_object('id', s.id, 'score', s.score)) from people s where s.active = 1)), 'none', json((select json_group_array(s.id) from people s where s.id < 0))) as profile, json_array(p.id, p.name, p.score) as pair, json_object('n', p.name) || '' as glued, t.doc as doc from people p, typed t where p.id = 1`,
      [],
      "one",
      { columns: [["profile", "json"], ["pair", "json"], ["glued", "text"], ["doc", "text"]] },
    ),
    dup: q(`insert into ${t("uniq")} (k) values (:k)`, ["k"], "none"),
  };
}

// Reylo's one representation per SQL type (src/values.ts) of the `typed` fixture's row, on every driver.
const TYPED: Record<Dialect, Record<string, unknown>> = {
  postgres: {
    id: 1, big: 9007199254740993n, amount: "12345678901234567890.123456789", day: "2026-10-20", at: "2026-10-20T10:00:00.123", stamp: "2026-10-20T08:00:00.123Z",
    doc: { a: [1, 2] }, docb: { b: true }, raw: Uint8Array.of(1, 2, 255), flag: true, f8: 2.5, tags: ["a", null, "NULL", "b,c"], ints: [1, 2, 3], moods: ["ok", "sad"], span: "1 day 02:00:00", uid: "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11",
  },
  mysql: {
    id: 1, big: -9007199254740993n, ubig: 18446744073709551615n, amount: "1.100000000000000001", day: "2026-10-20", at: "2026-10-20T10:00:00.123", doc: { a: [1, 2] },
    bit1: true, raw: Uint8Array.of(1, 2, 255), tags: "x,z", kind: "b", flag: true, yr: 2026, f8: 2.5, tm: "10:00:00", s: "hi",
  },
  sqlite: { id: 1, n: 5, r: 1.5, s: "hi", b: Uint8Array.of(1, 2, 255), doc: { a: [1, 2] }, flag: true, x: 7 },
};
const TYPED_COLUMNS: Record<Dialect, [string, string][]> = {
  postgres: [["id", "int"], ["big", "int8"], ["amount", "decimal"], ["day", "date"], ["at", "timestamp"], ["stamp", "timestamptz"], ["doc", "json"], ["docb", "json"], ["raw", "bytes"], ["flag", "bool"], ["f8", "float"], ["tags", "text[]"], ["ints", "int[]"], ["moods", "text[]"], ["span", "text"], ["uid", "text"]],
  // A BIT(1) and a TINYINT(1) column are truth values, as the engine types them (engine/mysql).
  mysql: [["id", "int"], ["big", "int8"], ["ubig", "int8"], ["amount", "decimal"], ["day", "date"], ["at", "timestamp"], ["doc", "json"], ["bit1", "bool"], ["raw", "bytes"], ["tags", "text"], ["kind", "text"], ["flag", "bool"], ["yr", "int"], ["f8", "float"], ["tm", "text"], ["s", "text"]],
  // A SQLite JSON column and truth value as the engine types them where it can tell, a column with no type unknown.
  sqlite: [["id", "int"], ["n", "int"], ["r", "float"], ["s", "text"], ["b", "bytes"], ["doc", "json"], ["flag", "bool"], ["x", "unknown"]],
};

const sc = (name: string) => ({ kind: "scalar", name });
const enumArray = { kind: "array", of: { kind: "enum", name: "mood", labels: ["ok", "sad"] } };
/** The parameter types the engine gives `c = :c` for each `typed` column (tests/render.test.ts, ROUND_TRIPS): on MySQL by the column (engine/mysql `column_types`), on SQLite by the column's affinity. */
const ROUND_TRIP: Record<Dialect, Record<string, unknown>> = {
  postgres: {
    big: sc("int8"), amount: sc("numeric"), day: sc("date"), at: sc("timestamp"), stamp: sc("timestamptz"), doc: { kind: "json" }, docb: { kind: "json" }, raw: sc("bytea"), flag: sc("bool"), f8: sc("float8"),
    tags: { kind: "array", of: sc("text") }, ints: { kind: "array", of: sc("int4") }, moods: enumArray, span: sc("interval"), uid: sc("uuid"),
  },
  mysql: { big: sc("int8"), ubig: sc("int8"), amount: sc("numeric"), day: sc("date"), at: sc("timestamp"), doc: { kind: "json" }, bit1: sc("bool|int4"), raw: sc("bytea"), tags: sc("text"), kind: sc("text"), flag: sc("bool|int4"), yr: sc("int4"), f8: sc("float8"), tm: sc("time"), s: sc("text") },
  sqlite: { n: sc("integer"), r: sc("real"), s: sc("text"), b: sc("blob"), flag: sc("numeric"), x: { kind: "unknown" } },
};
// An array goes back in without its NULL elements, which a parameter refuses (src/render.ts).
const ROUND_TRIP_ARRAYS = new Set(["tags", "ints", "moods"]);
/** The comparison for each column: Postgres's json has no `=`, and MySQL's JSON column is compared with json_extract of the JSON parameter. */
const ROUND_TRIP_SQL: Record<Dialect, (c: string) => string> = {
  postgres: (c) => (c === "doc" ? `doc::jsonb = :doc::jsonb` : ROUND_TRIP_ARRAYS.has(c) ? `array_remove(${c}, null) = :${c}` : `${c} = :${c}`),
  mysql: (c) => (c === "doc" ? `doc = json_extract(:doc, '$')` : `${c} = :${c}`),
  sqlite: (c) => `${c} = :${c}`,
};

/** A value with its JavaScript type spelled out, so that 1 and "1", or a Buffer and a Uint8Array of the same bytes, compare as they are. */
function typedValue(v: unknown): unknown {
  if (v === null || v === undefined) return v ?? null;
  if (v instanceof Date) return `Date(${Number.isNaN(v.getTime()) ? "Invalid" : v.toISOString()})`;
  if (v instanceof Uint8Array) return `Uint8Array(${[...v]})`;
  if (Array.isArray(v)) return v.map(typedValue);
  if (typeof v === "object") return Object.fromEntries(Object.keys(v).sort().map((k) => [k, typedValue((v as Record<string, unknown>)[k])]));
  return `${typeof v}(${String(v)})`;
}
function typedEq(actual: unknown, expected: unknown, what: string): void {
  const a = JSON.stringify(typedValue(actual));
  const b = JSON.stringify(typedValue(expected));
  if (a !== b) throw new Mismatch(`${what}: expected ${b}, got ${oneLine(a, 600)}`);
}

const PEOPLE = [
  { id: 1, name: "ada" },
  { id: 2, name: "bob" },
  { id: 3, name: "cy" },
];

let tagCounter = 0;
const runId = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
function newTag(ctx: Ctx, scenario: string): string {
  return `${ctx.job.pkg}@${ctx.job.version}/${ctx.shape.name}/${scenario}/${runId}/${++tagCounter}`.slice(0, 190);
}

// ---------------------------------------------------------------------------------------------
// Scenarios

type Scenario = {
  id: string;
  /** "client" scenarios run inside shape.scope; "tx" ones use shape.tx; "syncTx" ones use shape.syncTx. */
  on: "client" | "tx" | "syncTx";
  dialects?: Dialect[];
  /** false for a scenario whose statements take no parameters. */
  params?: false;
  run(ctx: Ctx, client: Client, q: ReturnType<typeof queries>): Promise<string | void>;
};

const SENTINEL = "dm-sentinel-rollback";
export function sentinel(): Error {
  return new Error(SENTINEL);
}
export function isSentinel(e: unknown): boolean {
  return e instanceof Error && e.message.includes(SENTINEL);
}
export function ignoreSentinel(e: unknown): void {
  if (!isSentinel(e)) throw e;
}

/** An INSERT with its values written into the text, so the statement takes no parameters. */
function literalInsert(d: Dialect, tag: string, n: number) {
  const table = d === "sqlite" ? "scratch" : "driver_matrix.scratch";
  return defineQuery<any, any, any, any>({ dialect: d, sql: `insert into ${table} (tag, n) values ('${tag.replace(/'/g, "''")}', ${n})`, params: [], cardinality: "none" });
}

const S9 = "S9 unsupported client: run() rejects with reylo's TypeError";

export const SCENARIOS: Scenario[] = [
  {
    id: "S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere",
    on: "client",
    params: false,
    async run(ctx, client, q) {
      let r: unknown;
      let threw: unknown = null;
      const via = spy(client);
      try {
        r = q.many.run(client);
      } catch (e) {
        threw = e;
      }
      if (threw) throw new Violation(`run threw synchronously: ${describe(threw)}`);
      if (!isThenable(r)) throw new Violation(`run returned ${r === null ? "null" : typeof r}; expected a promise`);
      await r;
      const dispatched = via.stop();
      let errThrew = false;
      let e2: unknown;
      try {
        e2 = q.syntax.run(client);
      } catch {
        errThrew = true;
      }
      if (errThrew) throw new Violation("run threw synchronously on a failing statement; expected a rejected promise");
      await (e2 as Promise<unknown>).then(() => {}, () => {});
      if (ctx.kind === "sync") {
        const rows = q.many.runSync(client);
        if (isThenable(rows)) throw new Violation("runSync returned a promise on a synchronous SQLite client");
        let syncThrew = false;
        try {
          q.syntax.runSync(client);
        } catch {
          syncThrew = true;
        }
        if (!syncThrew) throw new Violation("runSync did not throw on a failing statement");
        return `reylo called ${dispatched}; run returned a promise and rejected on a failing statement; runSync returned the value and threw`;
      }
      const sent = spy(client);
      let refused: unknown = null;
      try {
        q.many.runSync(client);
      } catch (e) {
        refused = e;
      }
      const calledByRunSync = sent.stop();
      if (!(refused instanceof TypeError) || !/^runSync takes a synchronous SQLite client/.test(refused.message)) throw new Violation(`runSync on an asynchronous client did not throw reylo's TypeError: ${describe(refused)}`);
      if (!calledByRunSync.startsWith("no method") && !calledByRunSync.startsWith("the client as a function")) throw new Violation(`runSync called ${calledByRunSync} before refusing the client`);
      return `reylo called ${dispatched}; run returned a promise and rejected on a failing statement; runSync refused the client before calling it`;
    },
  },
  {
    id: "S1a params: int, text, bool, null",
    on: "client",
    async run(ctx, client, q) {
      const d = ctx.job.dialect;
      // SQLite has no boolean type (reylo types the column as a number, and better-sqlite3 and node:sqlite refuse a JS boolean).
      const row = await call(ctx, q.params, client, { i: 1, t: "ada", b: d === "sqlite" ? 1 : true, nul: null });
      eq(row, { id: 1, name: "ada", active: true, score: 10 }, "row");
    },
  },
  {
    id: "S1b omitted param (undefined) binds NULL",
    on: "client",
    async run(ctx, client, q) {
      const rows = await call(ctx, q.omitted, client, { i: 1 });
      eq(rows, [{ id: 1 }], "rows");
    },
  },
  {
    id: "S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes",
    on: "client",
    async run(ctx, client, q) {
      // Not Buffers: what TextEncoder and crypto.getRandomValues return. mariadb writes such a
      // value as JSON ({"0":1,"1":2,"2":255}) unless reylo hands it a Buffer.
      const plain = Uint8Array.of(1, 2, 255);
      const view = new Uint8Array([9, 1, 2, 255, 9]).subarray(1, 4);
      eq(await call(ctx, q.binary, client, { a: plain, b: view }), { a: "0102ff", b: "0102ff" }, "hex of the bound bytes");
    },
  },
  {
    id: "S1d array params: = any(:ids) over an int and a text array",
    on: "client",
    dialects: ["postgres"],
    async run(ctx, client, q) {
      // Bun.sql binds no JavaScript array, so reylo hands it the array's literal text.
      eq(await call(ctx, q.list, client, { ids: [1, 3, 4], names: ["ada", "cy", 'say "hi" \\ {x,y}'] }), [PEOPLE[0], PEOPLE[2]].map(({ id }) => ({ id })), "rows");
    },
  },
  {
    id: "S1e JSON params: an array bound to :a::jsonb is stored as that JSON array",
    on: "client",
    dialects: ["postgres"],
    async run(ctx, client, q) {
      // pg writes any array as an array literal, so `[]` became `{}` and `[1]` an error; reylo
      // sends it the JSON text, and leaves the array to the drivers that bind it as JSON.
      eq(await call(ctx, q.jsonArrays, client, { a: [1, { k: 'say "hi"' }, null], e: [] }), { a: '[1, {"k": "say \\"hi\\""}, null]', e: "[]" }, "the stored JSON");
    },
  },
  {
    id: "S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL",
    on: "client",
    dialects: ["postgres", "mysql"],
    async run(ctx, client, q) {
      // Reylo binds a JSON parameter's value as its JSON text. Left to themselves, pg and mariadb
      // passed a string through as raw JSON text, mariadb expanded an array into a list, and
      // postgres.js and Bun.sql typed a boolean (Bun.sql a number too) by its JavaScript type.
      const values = { s: 'say "hi"', n: 5, b: true, t: '{"a":1}', o: { k: [1, null] }, a: [1, 2], e: [], z: null };
      const row = (await call(ctx, q.jsonValues, client, values)) as Record<string, string | null>;
      eq(Object.fromEntries(Object.entries(row).map(([k, v]) => [k, v == null ? v : JSON.parse(v)])), values, "the stored JSON, parsed");
    },
  },
  {
    id: "S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called",
    on: "client",
    dialects: ["postgres", "mysql"],
    async run(ctx, client, q) {
      // The drivers wrote a Date's wall-clock time in the app's zone (pg, mysql2), the server's
      // (postgres.js) or UTC (PGlite, Bun.sql), and Bun.sql's `date` failed in every zone.
      // postgres.js read a date or time string through new Date(): the timestamp moved by the
      // app's offset, and the zone name failed with "Invalid time value".
      const at = "2026-01-02 03:04:05.006";
      const row = await call(ctx, q.times, client, { tz: "2026-01-02T03:04:05.006Z", named: "2026-01-02 12:04:05.006 Asia/Tokyo", at, day: "2026-01-02" });
      eq({ ...row, tz: Boolean(Number(row.tz)) }, { tz: true, at: ctx.job.dialect === "postgres" ? at : `${at}000`, day: "2026-01-02" }, "row");
      // A Date is refused for a date and, since 2026-10-20, for a
      // timestamptz too, which takes the zoned text run returns for one.
      const refusals: [Record<string, unknown>, RegExp][] = [[{ tz: "1970-01-01T00:00:00Z", named: "2026-01-02T03:04:05Z", at, day: new Date(Date.UTC(2026, 0, 2)) }, /^Parameter "day" is a date without a time zone/]];
      if (ctx.job.dialect === "postgres") refusals.push([{ tz: new Date(0), named: "2026-01-02T03:04:05Z", at, day: "2026-01-02" }, /^Parameter "tz" is a timestamptz, .*not a Date: pass date\.toISOString\(\)/]);
      for (const [params, message] of refusals) {
        const sent = spy(client);
        let refused: unknown = null;
        try {
          await call(ctx, q.times, client, params);
        } catch (e) {
          refused = e;
        }
        const called = sent.stop();
        if (!(refused instanceof TypeError) || !message.test(refused.message)) throw new Violation(`a Date was not refused with reylo's TypeError ${message}: ${describe(refused)}`);
        if (!called.startsWith("no method") && !called.startsWith("the client as a function")) throw new Violation(`reylo called ${called} before refusing the Date`);
      }
    },
  },
  {
    id: "S1f array params: bool[], timestamptz[] and bytea[]",
    on: "client",
    dialects: ["postgres"],
    async run(ctx, client, q) {
      // postgres.js types an array by its first element, and Bun.sql binds none.
      const row = await call(ctx, q.typedArrays, client, { b: [true, false], d: ["2026-01-02T03:04:05.006Z"], x: [Uint8Array.of(1, 2), Uint8Array.of(255)] });
      eq(row, { b: "{t,f}", d: "1767323045.006000", x: "{0102,ff}" }, "row");
    },
  },
  {
    id: "S2a many: 3 rows",
    on: "client",
    params: false,
    async run(ctx, client, q) {
      eq(await call(ctx, q.many, client), PEOPLE, "rows");
    },
  },
  {
    id: "S2b one: the row",
    on: "client",
    async run(ctx, client, q) {
      eq(await call(ctx, q.one, client, { id: 2 }), PEOPLE[1], "row");
    },
  },
  {
    id: "S2c maybeOne: zero rows gives null",
    on: "client",
    async run(ctx, client, q) {
      const r = await call(ctx, q.maybeOne, client, { id: 999 });
      if (r !== null) throw new Mismatch(`expected null, got ${oneLine(safeJson(r))}`);
    },
  },
  {
    id: "S2d maybeOne: two rows throw reylo's error",
    on: "client",
    async run(ctx, client, q) {
      try {
        const r = await call(ctx, q.maybeOneOfTwo, client, { id: 2 });
        throw new Mismatch(`expected reylo's "at most one row" error, got ${oneLine(safeJson(r))}`);
      } catch (e) {
        if (e instanceof Mismatch || e instanceof Violation) throw e;
        if (!(e instanceof Error) || !/Expected at most one row, got 2/.test(e.message)) throw new Mismatch(`wrong error: ${describe(e)}`);
      }
    },
  },
  {
    id: "S2e with(client)(params)",
    on: "client",
    async run(ctx, client, q) {
      eq(await call(ctx, q.one, client, { id: 3 }, "with"), PEOPLE[2], "row");
    },
  },
  {
    id: "S3 write without RETURNING (none), then a separate read",
    on: "client",
    async run(ctx, client, q) {
      const tag = newTag(ctx, "S3");
      const r = await call(ctx, q.insert, client, { tag, n: 3 });
      if (r !== undefined) throw new Mismatch(`expected undefined for cardinality none, got ${oneLine(safeJson(r))}`);
      // Inside a transaction scope the row is not committed yet, so read it through the same client.
      eq(await call(ctx, q.byTag, client, { tag }), { n: 3 }, "read back through reylo");
      if (!ctx.shape.inTransaction) eq(await ctx.job.count(tag), 1, "raw count");
    },
  },
  {
    id: "S3n write without parameters (none), then a separate read",
    on: "client",
    params: false,
    async run(ctx, client) {
      const tag = newTag(ctx, "S3n");
      const r = await call(ctx, literalInsert(ctx.job.dialect, tag, 7), client);
      if (r !== undefined) throw new Mismatch(`expected undefined for cardinality none, got ${oneLine(safeJson(r))}`);
      if (!ctx.shape.inTransaction) eq(await ctx.job.count(tag), 1, "raw count");
    },
  },
  {
    id: "S4 INSERT ... RETURNING",
    on: "client",
    dialects: ["postgres", "sqlite"],
    async run(ctx, client, q) {
      const tag = newTag(ctx, "S4");
      eq(await call(ctx, q.returning, client, { tag, n: 4 }), { tag, n: 4 }, "returned row");
    },
  },
  {
    id: "S5a repeated :id (runtime lexer)",
    on: "client",
    async run(ctx, client, q) {
      eq(await call(ctx, q.repeated, client, { id: 3 }), PEOPLE[2], "row");
    },
  },
  {
    id: "S5b repeated :id (mapper text/order)",
    on: "client",
    async run(ctx, client, q) {
      eq(await call(ctx, q.repeatedMapper, client, { id: 3 }), PEOPLE[2], "row");
    },
  },
  {
    id: "S6a syntax error rejects with the driver's error",
    on: "client",
    params: false,
    async run(ctx, client, q) {
      try {
        const r = await call(ctx, q.syntax, client);
        throw new Mismatch(`expected a syntax error, got ${oneLine(safeJson(r))}`);
      } catch (e) {
        return expectDriverError(e, ctx.job.dialect, "syntax");
      }
    },
  },
  {
    id: "S6b unique violation rejects with the driver's error",
    on: "client",
    async run(ctx, client, q) {
      try {
        const r = await call(ctx, q.dup, client, { k: "taken" });
        throw new Mismatch(`expected a unique violation, got ${oneLine(safeJson(r))}`);
      } catch (e) {
        return expectDriverError(e, ctx.job.dialect, "dup");
      }
    },
  },
  {
    id: "S7a tx: write then commit is visible",
    on: "tx",
    async run(ctx, _client, q) {
      const tag = newTag(ctx, "S7a");
      await ctx.shape.tx!.commit((c) => call(ctx, q.insert, c, { tag, n: 1 }));
      eq(await ctx.job.count(tag), 1, "raw count after commit");
    },
  },
  {
    id: "S7b tx: write then rollback is not visible",
    on: "tx",
    async run(ctx, _client, q) {
      const tag = newTag(ctx, "S7b");
      await ctx.shape.tx!.rollback((c) => call(ctx, q.insert, c, { tag, n: 1 }));
      eq(await ctx.job.count(tag), 0, "raw count after rollback");
    },
  },
  {
    id: "S7c tx: savepoint rollback keeps the outer write",
    on: "tx",
    async run(ctx, _client, q) {
      if (!ctx.shape.tx!.savepoint) return "skip: no savepoint API on this shape";
      const outer = newTag(ctx, "S7c-outer");
      const inner = newTag(ctx, "S7c-inner");
      await ctx.shape.tx!.savepoint(
        (c) => call(ctx, q.insert, c, { tag: outer, n: 1 }),
        (c) => call(ctx, q.insert, c, { tag: inner, n: 2 }),
      );
      eq([await ctx.job.count(outer), await ctx.job.count(inner)], [1, 0], "raw counts [outer, inner]");
    },
  },
  {
    id: "S7d tx: a failing statement rolls the transaction back",
    on: "tx",
    async run(ctx, _client, q) {
      const tag = newTag(ctx, "S7d");
      let err: unknown = null;
      try {
        await ctx.shape.tx!.failing(async (c) => {
          await call(ctx, q.insert, c, { tag, n: 1 });
          await call(ctx, q.dup, c, { k: "taken" });
        });
      } catch (e) {
        err = e;
      }
      if (err === null) throw new Mismatch("the transaction did not reject");
      const seen = expectDriverError(err, ctx.job.dialect, "dup");
      eq(await ctx.job.count(tag), 0, "raw count after the failed transaction");
      return seen;
    },
  },
  {
    id: "S7n tx: a write without parameters, then commit, is visible",
    on: "tx",
    params: false,
    async run(ctx, _client) {
      const tag = newTag(ctx, "S7n");
      await ctx.shape.tx!.commit((c) => call(ctx, literalInsert(ctx.job.dialect, tag, 1), c));
      eq(await ctx.job.count(tag), 1, "raw count after commit");
    },
  },
  {
    id: "S7a tx: write then commit is visible",
    on: "syncTx",
    async run(ctx, db, q) {
      const tag = newTag(ctx, "S7a");
      const trap = promiseTrap();
      db.transaction(() => {
        trap.check(q.insert.runSync(db, { tag, n: 1 }));
      })();
      trap.assertNone();
      eq(await ctx.job.count(tag), 1, "raw count after commit");
    },
  },
  {
    id: "S7b tx: write then rollback is not visible",
    on: "syncTx",
    async run(ctx, db, q) {
      const tag = newTag(ctx, "S7b");
      const trap = promiseTrap();
      try {
        db.transaction(() => {
          trap.check(q.insert.runSync(db, { tag, n: 1 }));
          throw sentinel();
        })();
      } catch (e) {
        ignoreSentinel(e);
      }
      trap.assertNone();
      eq(await ctx.job.count(tag), 0, "raw count after rollback");
    },
  },
  {
    id: "S7c tx: savepoint rollback keeps the outer write",
    on: "syncTx",
    async run(ctx, db, q) {
      const outer = newTag(ctx, "S7c-outer");
      const inner = newTag(ctx, "S7c-inner");
      const trap = promiseTrap();
      db.transaction(() => {
        trap.check(q.insert.runSync(db, { tag: outer, n: 1 }));
        try {
          // A transaction function called inside another runs as a savepoint.
          db.transaction(() => {
            trap.check(q.insert.runSync(db, { tag: inner, n: 2 }));
            throw sentinel();
          })();
        } catch (e) {
          ignoreSentinel(e);
        }
      })();
      trap.assertNone();
      eq([await ctx.job.count(outer), await ctx.job.count(inner)], [1, 0], "raw counts [outer, inner]");
    },
  },
  {
    id: "S7d tx: a failing statement rolls the transaction back",
    on: "syncTx",
    async run(ctx, db, q) {
      const tag = newTag(ctx, "S7d");
      const trap = promiseTrap();
      let err: unknown = null;
      try {
        db.transaction(() => {
          trap.check(q.insert.runSync(db, { tag, n: 1 }));
          trap.check(q.dup.runSync(db, { k: "taken" }));
        })();
      } catch (e) {
        err = e;
      }
      const leaked = await trap.settle();
      const count = await ctx.job.count(tag);
      if (err === null) {
        throw new Mismatch(`transaction() did not throw, so it committed (raw count ${count})${leaked.length ? `; runSync returned promises and the error arrived as a rejection instead: ${leaked.join("; ")}` : ""}`);
      }
      const seen = expectDriverError(err, ctx.job.dialect, "dup");
      eq(count, 0, "raw count after the failed transaction");
      return seen;
    },
  },
  {
    id: "S7e tx: a read inside transaction(fn) is usable synchronously",
    on: "syncTx",
    async run(ctx, db, q) {
      const tag = newTag(ctx, "S7e");
      const got = db.transaction(() => {
        q.insert.runSync(db, { tag, n: 5 });
        const row = q.byTag.runSync(db, { tag });
        if (isThenable(row)) {
          (row as Promise<unknown>).then(
            () => {},
            () => {},
          );
          return "a promise";
        }
        return row;
      })();
      if (got === "a promise") throw new Violation("runSync returned a promise inside transaction(fn); the row is not usable there");
      eq(got, { n: 5 }, "row read inside the transaction");
    },
  },
  {
    id: "S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself",
    on: "client",
    params: false,
    async run(ctx, client, q) {
      const d = ctx.job.dialect;
      typedEq(await call(ctx, q.typed, client), TYPED[d], "row");
      if (d !== "sqlite") {
        typedEq(await call(ctx, q.typedBig, client), { id: 1, big: TYPED[d].big }, "an int8 past 2^53");
        return;
      }
      // A column declared BIGINT is a BigInt, exact past 2^53 where the driver reads the integer
      // itself (Reylo's own statement on better-sqlite3, node:sqlite, bun:sqlite and Turso's
      // engine); where the driver rounds it first, Reylo's error naming the column (D1's job, or
      // Drizzle's over D1); libSQL's own error at its default intMode.
      const rounds = /^(sqlite3|sqlite|miniflare D1|Bun\.sql)\b|^drizzle-orm \(d1\)/.test(ctx.job.pkg);
      // libSQL's job, or Drizzle's over libSQL.
      const libsql = /^(@libsql\/|drizzle-orm \(libsql\))/.test(ctx.job.pkg);
      try {
        const r = await call(ctx, q.typedWide, client);
        if (rounds || libsql) throw new Mismatch(`expected the BIGINT past 2^53 to be refused, got ${oneLine(safeJson(r))}`);
        typedEq(r, { id: 1, wide: 9007199254740993n }, "a BIGINT past 2^53");
      } catch (e) {
        if (e instanceof Mismatch || e instanceof Violation) throw e;
        const refused = (rounds && e instanceof TypeError && /^Column "wide" is a 64-bit integer, and the driver already rounded its value to 9007199254740992/.test(e.message)) || (libsql && e instanceof RangeError && /cannot be safely represented/.test(e.message));
        if (!refused) throw new Mismatch(`not the BIGINT past 2^53 read exactly or refused as rounded: ${describe(e)}`);
      }
      // An INTEGER past 2^53, which a number cannot hold: Reylo's error naming the column, or libSQL's own at its default intMode.
      try {
        const r = await call(ctx, q.typedBig, client);
        throw new Mismatch(`expected the integer past 2^53 to be refused, got ${oneLine(safeJson(r))}`);
      } catch (e) {
        if (e instanceof Mismatch || e instanceof Violation) throw e;
        if (e instanceof TypeError && /^Column "big" holds/.test(e.message)) return e.message.slice(0, 120);
        if (e instanceof RangeError && /cannot be safely represented/.test(e.message)) return `libSQL's own: ${e.message}`;
        throw new Mismatch(`not the refusal of an integer past 2^53: ${describe(e)}`);
      }
    },
  },
  {
    id: "S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string",
    on: "client",
    params: false,
    dialects: ["sqlite"],
    async run(ctx, client, q) {
      typedEq(
        await call(ctx, q.builtJson, client),
        { profile: { user: { id: 1, name: "ada" }, scores: [{ id: 1, score: 10 }, { id: 3, score: 30 }], none: [] }, pair: [1, "ada", 10], glued: '{"n":"ada"}', doc: '{"a":[1,2]}' },
        "row",
      );
    },
  },
  {
    id: "S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal",
    on: "client",
    async run(ctx, client, q) {
      const d = ctx.job.dialect;
      // What run returns for each type, which the parameter of that type takes (src/render.ts):
      // a BIGINT's BigInt, a TINYINT(1)'s and a BIT(1)'s boolean on MySQL, a SQLite truth value
      // (bound as 1 or 0), an int8, a numeric's text, a timestamptz's UTC text, parsed JSON, bytes.
      // A Postgres array may come back with NULL elements, which a parameter refuses: it goes back
      // in without them, and with them it is refused before the statement runs.
      const row = (await call(ctx, q.typed, client)) as Record<string, unknown>;
      const strict = (c: string) => (ROUND_TRIP_ARRAYS.has(c) && Array.isArray(row[c]) ? (row[c] as unknown[]).filter((x) => x !== null) : row[c]);
      const params = Object.fromEntries([...Object.keys(ROUND_TRIP[d]).map((c) => [c, strict(c)]), ["id", 1]]);
      eq(await call(ctx, q.roundTrip, client, params), Object.fromEntries(Object.keys(ROUND_TRIP[d]).map((c) => [c, true])), "each column equal to the value it returned");
      const withNulls = Object.keys(ROUND_TRIP[d]).find((c) => ROUND_TRIP_ARRAYS.has(c) && Array.isArray(row[c]) && (row[c] as unknown[]).includes(null));
      if (withNulls) {
        try {
          await call(ctx, q.roundTrip, client, { ...params, [withNulls]: row[withNulls] });
        } catch (e) {
          if (e instanceof TypeError && /elements may not be null/.test(e.message)) return;
          throw new Mismatch(`not the refusal of an array parameter's NULL element: ${describe(e)}`);
        }
        throw new Mismatch(`${withNulls}, an array with a NULL element, went back in as a parameter`);
      }
    },
  },
  {
    id: "S11b round trip: an instant past 9999 and one before 1 go back in",
    on: "client",
    dialects: ["postgres"],
    async run(ctx, client, q) {
      // They come back as `+010000-…Z` and `-000001-…Z`, which Postgres refuses as input
      // ("time zone displacement out of range"); Reylo writes them as `10000-…` and `0002-… BC`.
      const far = (await call(ctx, q.farInstants, client)) as Record<string, unknown>;
      eq(far, { far: "+010000-01-01T00:00:00Z", bc: "-000001-01-01T00:00:00Z" }, "the instants past 9999 and before 1");
      eq(await call(ctx, q.farBack, client, far), { far: true, bc: true }, "the same instants passed back");
    },
  },
  {
    id: "S8 20 concurrent queries resolve with the right rows",
    on: "client",
    async run(ctx, client, q) {
      const ids = Array.from({ length: 20 }, (_, i) => (i % 3) + 1);
      const rows = await Promise.all(ids.map((id) => call(ctx, q.one, client, { id })));
      eq(
        rows.map((r: any) => r?.name),
        ids.map((id) => PEOPLE[id - 1]!.name),
        "names in order",
      );
    },
  },
];

/** Records which of the client's methods reylo calls, by shadowing them with own properties until stop(). */
function spy(client: Client): { stop(): string } {
  const names = ["query", "unsafe", "execute", "all", "prepare", "batch", "promise", "manyOrNone"];
  const calls: string[] = [];
  const restore: (() => void)[] = [];
  for (const name of names) {
    try {
      const orig = client?.[name];
      if (typeof orig !== "function") continue;
      const own = Object.getOwnPropertyDescriptor(client, name);
      if (own && !own.configurable) continue;
      Object.defineProperty(client, name, {
        configurable: true,
        writable: true,
        value: function (this: unknown, ...rest: unknown[]) {
          calls.push(`${name}()`);
          return orig.apply(this, rest);
        },
      });
      restore.push(() => (own ? Object.defineProperty(client, name, own) : delete client[name]));
    } catch {
      // A native or frozen object: leave it alone.
    }
  }
  return {
    stop() {
      for (const undo of restore) undo();
      const seen = [...new Set(calls)];
      if (seen.length) return seen.join(" then ");
      return typeof client === "function" ? "the client as a function (or a method the harness could not observe)" : "no method the harness could observe";
    },
  };
}

/** Collects promises that runSync() returned inside a synchronous transaction(fn), where it must return values. */
function promiseTrap() {
  const promises: Promise<unknown>[] = [];
  return {
    check(v: unknown) {
      if (isThenable(v)) promises.push(Promise.resolve(v));
    },
    assertNone() {
      for (const p of promises) p.then(() => {}, () => {});
      if (promises.length) throw new Violation(`runSync returned ${promises.length} promise(s) inside transaction(fn); expected values`);
    },
    async settle(): Promise<string[]> {
      const out: string[] = [];
      for (const p of promises) {
        try {
          await p;
          out.push("resolved");
        } catch (e) {
          out.push(`rejected: ${describe(e)}`);
        }
      }
      return out;
    },
  };
}

// ---------------------------------------------------------------------------------------------
// Running a job

const SCENARIO_TIMEOUT = Number(process.env.DM_SCENARIO_TIMEOUT ?? 20000);

function timeout<T>(p: Promise<T>, ms: number, what: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  return Promise.race([
    p.finally(() => clearTimeout(timer)),
    new Promise<T>((_, reject) => {
      timer = setTimeout(() => reject(new TimedOut(`timed out after ${ms / 1000} s: ${what} never settled`)), ms);
    }),
  ]);
}
class TimedOut extends Error {}

/** Lazily creates a resource once, so a shape whose setup hangs does not take the others with it. */
export function lazy<T>(make: () => Promise<T> | T): () => Promise<T> {
  let p: Promise<T> | undefined;
  return () => (p ??= Promise.resolve().then(make));
}

export type JobMeta = { pkg: string; version: string; target: string; live?: boolean };

/** Reports a job that could not start (the package does not load, the server is unreachable). */
export function notRun(meta: JobMeta, why: string): void {
  emit({ pkg: meta.pkg, version: meta.version, runtime, target: meta.target, shape: "(job)", scenario: "setup", status: "not run", live: meta.live ?? true, error: why });
}

export async function runJob(meta: JobMeta, setup: () => Promise<Job>): Promise<void> {
  process.on("unhandledRejection", (e) => {
    console.error(`unhandled rejection (ignored): ${describe(e)}`);
  });
  // A driver's process warnings (a deprecation, say) are attached to the scenario that raised them.
  const warnings: string[] = [];
  process.on("warning", (w) => {
    warnings.push(`${w.name}: ${oneLine(w.message, 200)}`);
  });
  let job: Job;
  try {
    job = await timeout(setup(), 60000, "setup");
  } catch (e) {
    emit({ pkg: meta.pkg, version: meta.version, runtime, target: meta.target, shape: "(job)", scenario: "setup", status: "fail", live: meta.live ?? true, error: describe(e) });
    finish(1);
    return;
  }
  // The raw count, without reylo, first: a driver that cannot query the server at all (pg 7 on
  // Node 26, say) is reported once, not as a failure of every reylo scenario.
  try {
    await timeout(job.count("dm-preflight"), 20000, "the raw count");
  } catch (e) {
    emit({ pkg: job.pkg, version: job.version, runtime, target: job.target, shape: "(job)", scenario: "preflight", status: "not run", live: meta.live ?? true, error: `the driver cannot run a query without reylo: ${e instanceof TimedOut ? e.message : describe(e)}` });
    try {
      await timeout(Promise.resolve(job.close?.()), 5000, "close");
    } catch {}
    finish(0);
    return;
  }
  const typed = queries(job.dialect);
  for (const shape of job.shapes) {
    const q = shape.untyped ? queries(job.dialect, false) : typed;
    const ctx: Ctx = { job, shape, kind: shape.kind, refusals: [] };
    const base = { pkg: job.pkg, version: job.version, runtime, target: job.target, shape: shape.name, live: shape.live ?? meta.live ?? true };
    if (shape.refused) {
      emit({ ...base, scenario: S9, ...(await checkRefused(shape, q)) });
      continue;
    }
    let timeouts = 0;
    for (const s of SCENARIOS) {
      if (s.dialects && !s.dialects.includes(job.dialect)) continue;
      if (s.on === "client" && !shape.scope) continue;
      if (s.on === "tx" && !shape.tx) continue;
      if (s.on === "syncTx" && !shape.syncTx) continue;
      const reason = shape.skip?.[s.id.split(" ")[0]!] ?? job.skip?.[s.id.split(" ")[0]!];
      if (reason) {
        emit({ ...base, scenario: s.id, status: "skip", detail: reason });
        continue;
      }
      if (timeouts >= 2) {
        emit({ ...base, scenario: s.id, status: "not run", error: "earlier scenarios on this shape timed out" });
        continue;
      }
      warnings.length = 0;
      ctx.refusals.length = 0;
      const refusalExpected = !!shape.paramsRefused && s.params !== false;
      try {
        let detail: string | void = undefined;
        let done = false;
        const work =
          s.on === "client"
            ? shape.scope!(async (client) => {
                detail = await s.run(ctx, client, q);
                done = true;
              }).catch((e) => {
                if (!done) throw e;
                // A statement that failed inside an open transaction aborts it (Postgres), so the
                // shape's own commit then fails: the driver's semantics, not reylo's.
                if (shape.inTransaction) {
                  detail = [detail, `then the enclosing transaction failed, as the database aborts it after an error: ${describe(e)}`].filter(Boolean).join("; ");
                  return;
                }
                throw new Error(`the scenario passed, then the shape's teardown failed: ${describe(e)}`);
              })
            : s.run(ctx, s.on === "syncTx" ? shape.syncTx : null, q).then((d) => {
                detail = d;
              });
        await timeout(work, SCENARIO_TIMEOUT, "the scenario");
        await new Promise((r) => setImmediate(r));
        const d = [detail as string | void, ...warnings.map((w) => `warning: ${w}`)].filter(Boolean).join("; ") || undefined;
        if (refusalExpected) emit({ ...base, scenario: s.id, ...refusalResult(ctx, null) });
        else if (typeof d === "string" && d.startsWith("skip: ")) emit({ ...base, scenario: s.id, status: "skip", detail: d.slice(6) });
        else emit({ ...base, scenario: s.id, status: "pass", ...(d ? { detail: d } : {}) });
      } catch (e) {
        if (refusalExpected) {
          emit({ ...base, scenario: s.id, ...refusalResult(ctx, e) });
          continue;
        }
        if (e instanceof TimedOut) timeouts++;
        const msg = e instanceof Mismatch || e instanceof TimedOut ? e.message : e instanceof Violation ? `reylo contract: ${e.message}` : describe(e);
        emit({ ...base, scenario: s.id, status: "fail", error: msg, ...(warnings.length ? { detail: warnings.map((w) => `warning: ${w}`).join("; ") } : {}) });
      }
    }
  }
  try {
    await timeout(Promise.resolve(job.close?.()), 10000, "close");
  } catch (e) {
    console.error(`close failed: ${describe(e)}`);
  }
  finish(0);
}

/** A statement with parameters on a client that binds none: reylo must refuse it with its TypeError, carrying the driver's error as cause. */
function refusalResult(ctx: Ctx, e: unknown): { status: Status; error?: string; detail?: string } {
  const refusal = ctx.refusals[0] as (TypeError & { cause?: unknown }) | undefined;
  if (refusal) {
    if (!(refusal.cause instanceof Error)) return { status: "fail", error: `reylo refused the statement, but its TypeError carries no driver error as cause: ${describe(refusal)}` };
    return { status: "unsupported", detail: `reylo refused it: ${oneLine(refusal.message, 160)} (cause: ${describe(refusal.cause)})` };
  }
  if (e === null) return { status: "fail", error: "expected reylo to refuse a statement with parameters on this client, but the scenario ran" };
  return { status: "fail", error: `expected reylo's refusal of a statement with parameters, got: ${e instanceof Mismatch || e instanceof TimedOut ? e.message : describe(e)}` };
}

async function checkRefused(shape: Shape, q: ReturnType<typeof queries>): Promise<{ status: Status; error?: string; detail?: string }> {
  let r: unknown;
  try {
    r = q.many.run(shape.scope ? await captureClient(shape) : null);
  } catch (e) {
    return { status: "fail", error: `run threw synchronously instead of rejecting: ${describe(e)}` };
  }
  if (!isThenable(r)) return { status: "fail", error: `run returned ${typeof r}, not a rejected promise` };
  try {
    const rows = await timeout(Promise.resolve(r), SCENARIO_TIMEOUT, "run");
    return { status: "fail", error: `reylo ran the statement on an unsupported client and resolved ${oneLine(safeJson(rows))}` };
  } catch (e) {
    if (e instanceof TypeError && shape.refused!.test(e.message)) return { status: "unsupported", detail: `reylo refused it: ${oneLine(e.message, 1000)}` };
    return { status: "fail", error: `not reylo's refusal: ${describe(e)}` };
  }
}

/** The client object a shape's scope hands out, for a check that runs outside it. */
function captureClient(shape: Shape): Promise<Client> {
  return new Promise((resolve, reject) => {
    shape.scope!(async (c) => resolve(c)).catch(reject);
  });
}

function finish(code: number): void {
  if (out) appendFileSync(out, JSON.stringify({ done: true }) + "\n");
  // Some drivers keep sockets or workers open; the job is over.
  setTimeout(() => process.exit(code), 50);
}

// ---------------------------------------------------------------------------------------------
// Transaction helpers shared by the drivers

/** BEGIN / COMMIT / ROLLBACK issued by hand on one connection (pg PoolClient, mysql2 and mariadb beginTransaction, node:sqlite exec). */
export function manualTx(opts: {
  open: () => Promise<{ client: Client; release?: () => unknown }>;
  begin: (c: Client) => Promise<unknown> | unknown;
  commit: (c: Client) => Promise<unknown> | unknown;
  rollback: (c: Client) => Promise<unknown> | unknown;
  exec?: (c: Client, sql: string) => Promise<unknown> | unknown;
}): TxOps {
  const inTx = async (body: (c: Client) => Promise<unknown>) => {
    const { client, release } = await opts.open();
    try {
      await opts.begin(client);
      try {
        await body(client);
        await opts.commit(client);
      } catch (e) {
        await Promise.resolve(opts.rollback(client)).catch(() => {});
        throw e;
      }
    } finally {
      await release?.();
    }
  };
  const ops: TxOps = {
    commit: (body) => inTx(body),
    rollback: async (body) => {
      const { client, release } = await opts.open();
      try {
        await opts.begin(client);
        await body(client);
        await opts.rollback(client);
      } finally {
        await release?.();
      }
    },
    failing: (body) => inTx(body),
  };
  if (opts.exec) {
    const exec = opts.exec;
    ops.savepoint = (outer, inner) =>
      inTx(async (c) => {
        await outer(c);
        await exec(c, "SAVEPOINT dm_sp");
        await inner(c);
        await exec(c, "ROLLBACK TO SAVEPOINT dm_sp");
      });
  }
  return ops;
}

/** A driver whose transaction takes a callback and commits when it resolves (postgres.js begin, PGlite transaction, pg-promise tx, PlanetScale transaction). */
export function callbackTx(begin: (fn: (tx: Client) => Promise<unknown>) => Promise<unknown>, savepoint?: (tx: Client, fn: (sp: Client) => Promise<unknown>) => Promise<unknown>): TxOps {
  const ops: TxOps = {
    commit: (body) => begin((tx) => body(tx)),
    rollback: (body) =>
      begin(async (tx) => {
        await body(tx);
        throw sentinel();
      }).catch(ignoreSentinel),
    failing: (body) => begin((tx) => body(tx)),
  };
  if (savepoint) {
    ops.savepoint = (outer, inner) =>
      begin(async (tx) => {
        await outer(tx);
        await savepoint(tx, async (sp) => {
          await inner(sp);
          throw sentinel();
        }).catch(ignoreSentinel);
      });
  }
  return ops;
}

/** A savepoint by hand inside a callback transaction whose API has none (PGlite, PlanetScale, libSQL). */
export function rawSavepoint(exec: (tx: Client, sql: string) => Promise<unknown>) {
  return async (tx: Client, fn: (sp: Client) => Promise<unknown>) => {
    await exec(tx, "SAVEPOINT dm_sp");
    try {
      await fn(tx);
      await exec(tx, "RELEASE SAVEPOINT dm_sp");
    } catch (e) {
      await exec(tx, "ROLLBACK TO SAVEPOINT dm_sp");
      throw e;
    }
  };
}
