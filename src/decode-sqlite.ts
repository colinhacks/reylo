// Reylo gives every value of its own SQLite queries its type itself (the
// representation is src/values.ts). Every SQLite driver already turns each storage class into
// a JavaScript value in native code, and they agree but on two: an INTEGER (a number that may be
// rounded past 2^53, an error, a BigInt, or libSQL's string) and a BLOB (a Buffer, a Uint8Array, an
// ArrayBuffer or D1's array of numbers). And only the analysis knows that a TEXT holds JSON or that
// an INTEGER is a truth value. So Reylo normalizes the driver's rows, by each column's kind where the
// module carries it (wiki/driver-decoding/spike/sqlite.md):
//
// - better-sqlite3, node:sqlite and bun:sqlite: Reylo prepares its own statement on the app's
//   database, once per database and text (a WeakMap keyed by the database), and sets that
//   statement's own integer mode, whatever the database's, and no other statement of the app's
//   changes. node:sqlite's statement, and better-sqlite3's and bun:sqlite's where a column has no
//   type the module knows or is a BIGINT's (`int8`, a BigInt), read every INTEGER as a BigInt: an
//   INTEGER's is then a number, or an error naming its column past 2^53. Where every column is
//   typed and none is a BIGINT's, better-sqlite3's and bun:sqlite's read numbers (a BigInt per
//   integer cost +70% on 10,000 rows when `exactInteger` compared BigInts, and +17 to 20% since it
//   converts first, which is also what a five-column row costs with one of its INTEGER columns
//   declared BIGINT), and a number past 2^53 in a column that may hold an INTEGER can only be a
//   rounded one, which is refused. bun:sqlite's statement comes from
//   `prepare`, never `query`, whose cache by text would carry Reylo's mode into the app's statement.
// - Turso's new engine (@tursodatabase/database): Reylo prepares its statement and sets its integer
//   mode as on better-sqlite3, per call and closed after it, since a Transaction's statements end
//   with it and a 0.4 statement run twice could lose its rows; the promise API's statement is
//   awaited, the compat Database's answers at once. Its BLOB is a Buffer and its BOOLEAN 0 or 1, as
//   on better-sqlite3; at its defaults an INTEGER past 2^53 comes back rounded, with no error.
// - A Durable Object's `ctx.storage.sql`: exec()'s cursor, read whole. An INTEGER past 2^53 comes
//   back rounded, with no error, and a BLOB as an ArrayBuffer.
// - sqlite3 and the sqlite wrapper, libSQL, D1 and Bun.SQL's SQLite: the rows the driver gives,
//   normalized. sqlite3 and sqlite are not cached: a statement left open makes the app's close()
//   fail with SQLITE_BUSY. An INTEGER the driver already rounded (sqlite3, sqlite, D1, Bun.SQL
//   without safeIntegers) is refused in an INTEGER or BIGINT column, where a number past 2^53 can
//   only be one; a BIGINT's number within 2^53, or libSQL's BigInt or digits, is its BigInt.
import { rowOf } from "./row.ts";
import { bigInteger, coerce, DecodeError, exactInteger, refuseTruth, truth, type Columns, type Kind } from "./values.ts";

type AnyClient = Record<string, any>;
type Statement = AnyClient;

/** Reylo's statement, whether it reads every INTEGER as a BigInt, and whether it is kept for the next call. */
type Kept = { st: Statement; exact: boolean; kept: boolean };
const statements = new WeakMap<object, Map<string, Kept>>();
/** How many statements Reylo keeps per database: more distinct texts than that drop the oldest. */
const KEPT = 500;

/**
 * Reylo's own statement for `text` on the app's database, with integers read as BigInts. Turso's
 * compat Database (maxWriteReplicationIndex()) gets a new statement per call, closed after it: on
 * 0.4 a statement with a subquery run a second time returned no rows.
 */
function statementFor(db: AnyClient, text: string, typed: boolean, proto: boolean): Kept {
  const keep = typeof db.maxWriteReplicationIndex !== "function";
  let kept = statements.get(db);
  if (!kept && keep) statements.set(db, (kept = new Map()));
  const key = `${typed ? "n" : "b"}${proto ? "p" : ""}:${text}`;
  const found = kept?.get(key);
  if (found) return found;
  const st = db.prepare(text) as Statement;
  let exact = false;
  // node:sqlite's StatementSync; better-sqlite3's and bun:sqlite's Statement.
  // Turso's engine names a column with no alias its own way (`select a + 1 from t` is `t.a + 1` on 0.4, `select (a) from t` is `(a)` on 0.8), and only the embedded SQLite's analysis says what the row's type declares: its rows are read as arrays, by position.
  if (!keep && typeof st.raw === "function") st.raw(true);
  // better-sqlite3 (its pluck() tells it from bun:sqlite, whose raw() runs the statement) builds a row by assigning each name, and a column named `__proto__` sets the object's prototype instead. Such a statement is read by position.
  else if (proto && typeof st.pluck === "function" && typeof st.raw === "function") st.raw(true);
  if (typeof st.setReadBigInts === "function") {
    st.setReadBigInts(true);
    if (typeof st.setReturnArrays === "function") st.setReturnArrays(false);
    exact = true;
  } else if (typeof st.safeIntegers === "function") {
    st.safeIntegers(!typed);
    exact = !typed;
  }
  const entry = { st, exact, kept: keep };
  if (!kept) return entry;
  if (kept.size >= KEPT) kept.delete(kept.keys().next().value!);
  kept.set(key, entry);
  return entry;
}

/**
 * Runs `text` on a synchronous SQLite database (better-sqlite3, node:sqlite, bun:sqlite, Turso's
 * compat Database) and returns its normalized rows; a statement that answers through a promise (no
 * such database) gives the promise, which `runSync` refuses.
 */
export function runSyncSqlite(db: AnyClient, text: string, values: unknown[], columns: Columns | undefined): unknown[] | Promise<unknown[]> {
  const entry = statementFor(db, text, everyColumnTyped(columns), hasProto(columns));
  if (entry.kept) return runStatement(entry, values, columns);
  try {
    return runStatement(entry, values, columns);
  } finally {
    entry.st.close?.();
  }
}

/** Rows read as arrays, as objects: under the names the module declares where it has them for every value, else under the driver's. */
function byPosition(rows: unknown[][], columns: Columns | undefined, driver: () => string[]): unknown[] {
  const build = rowOf(columns?.length === rows[0]!.length ? columns.map(([name]) => name) : driver());
  return rows.map(build);
}

/**
 * libSQL's rows by position, where the module declares their columns. A libSQL row holds each value under its index as well as its name, and the index comes first: `select 1, 2 + 3` has a column named `1`, and `row["1"]` is the second column's 5.
 */
export function libsqlRows(rows: unknown[], columns: Columns | undefined): unknown[] {
  const n = columns?.length;
  if (!n || !rows.length || (rows[0] as { length?: unknown }).length !== n) return rows;
  const build = rowOf(columns!.map(([name]) => name));
  return rows.map((row) => build(row as ArrayLike<unknown> as readonly unknown[]));
}

function runStatement({ st, exact }: Kept, values: unknown[], columns: Columns | undefined): unknown[] | Promise<unknown[]> {
  // better-sqlite3 refuses all() on a statement that returns no rows.
  if (reader(st) === false) {
    st.run(...values);
    return [];
  }
  const all = st.all(...values) as unknown[] | Promise<unknown[]>;
  if (!Array.isArray(all)) return Promise.resolve(all).then((rows) => normalize(rows, columns));
  let rows = all;
  // Turso's rows, or a database opened with returnArrays, on a Node whose statements cannot turn it off.
  if (rows.length && Array.isArray(rows[0])) rows = byPosition(rows as unknown[][], columns, () => (st.columns() as { name: string }[]).map((c) => c.name));
  return normalize(rows, columns, { exact });
}

// Whether the statement returns rows: undefined where it cannot say, as Turso 0.4's compat statement,
// whose `reader` throws "not implemented" and whose all() runs any statement.
function reader(st: Statement): boolean | undefined {
  try {
    return st.reader;
  } catch {
    return undefined;
  }
}

/** Whether a column is named `__proto__`, which only an own property of a row can hold (src/row.ts). */
function hasProto(columns: Columns | undefined): boolean {
  return columns?.some(([name]) => name === "__proto__") ?? false;
}

// Every column typed: a number past 2^53 in one can be told as rounded (src/values.ts). A column
// read as a BigInt (`int8`) needs the statement's integers exact.
function everyColumnTyped(columns: Columns | undefined): boolean {
  return columns !== undefined && columns.length > 0 && !columns.some(([, kind]) => kind === "unknown" || kind.split("|").includes("int8"));
}

/**
 * Runs `text` on Turso's promise API (a Database from `connect()`, or the Transaction
 * `transactionAsync()` hands its callback) and returns its normalized rows. Its prepare() returns
 * the statement (its types say a promise of it from 0.6, so it is awaited), whose all() runs a
 * statement that returns no rows as well; the statement is closed after its one run.
 */
export async function runTurso(db: AnyClient, text: string, values: unknown[], columns: Columns | undefined): Promise<unknown[]> {
  const typed = everyColumnTyped(columns);
  const st = (await db.prepare(text)) as Statement;
  try {
    if (typeof st.safeIntegers === "function") st.safeIntegers(!typed);
    // By position, as on the compat Database (`statementFor`). A statement that returns no rows gives none under either.
    if (typeof st.raw === "function") st.raw(true);
    const rows = (await st.all(...values)) as unknown[];
    const objects = rows.length && Array.isArray(rows[0]) ? byPosition(rows as unknown[][], columns, () => (st.columns() as { name: string }[]).map((c) => c.name)) : rows;
    return normalize(objects, columns, { exact: !typed });
  } finally {
    if (typeof st.close === "function") st.close();
  }
}

/**
 * Runs `text` on a SQLite-backed Durable Object's SQL storage (`ctx.storage.sql`), whose exec()
 * answers at once with a cursor, and returns its normalized rows. Its integers are numbers, so one
 * past 2^53 in an INTEGER column can only be a rounded one, and is refused.
 */
export function runDurableObject(sql: AnyClient, text: string, values: unknown[], columns: Columns | undefined): unknown[] {
  return normalize(sql.exec(text, ...values).toArray(), columns);
}

type Plan = {
  name: string;
  /** An INTEGER the driver read as a BigInt. */
  big: (v: bigint) => unknown;
  /** A number, where it may need converting: a truth value, or an INTEGER a driver may have rounded. */
  num: ((v: number) => unknown) | undefined;
  /** A string, where it may need converting: JSON, a truth value, libSQL's integer under intMode "string". */
  str: ((v: string) => unknown) | undefined;
  /** A BLOB in a column whose type holds none. */
  blob?: ((v: object) => never) | undefined;
};

/** An integer's digits, as libSQL under intMode "string" hands one over. */
const DIGITS = /^[+-]?\d+$/;

/**
 * What each column's values need, worked out once per call rather than per value. SQLite holds a
 * column to its declared type only in a STRICT table: elsewhere it stores a value of any storage
 * class in any column, the text 'abc' in an INTEGER column and a BLOB in a TEXT one, and a column
 * declared BOOLEAN holds whatever was stored in it. A value the column's type does not allow is
 * refused, naming the column, rather than returned as what the type says it is not (`refuseClass`,
 * and `truth` in src/values.ts, since a truth value is 0 or 1 in every SQLite driver).
 */
function plan(name: string, kind: Kind, exact: boolean): Plan {
  if (kind === "bool") return { name, big: (v) => truth(v, name), num: (v) => truth(v, name), str: (v) => truth(v, name), blob: () => refuseTruth(name, "a BLOB") };
  if (kind === "json") return { name, big: (v) => exactInteger(v, name), num: undefined, str: (v) => JSON.parse(v) };
  const kinds = kind.split("|");
  const refuse = (v: unknown): never => refuseClass(name, kinds, v);
  // A column with no type, or ANY in a STRICT table, is `unknown`, and takes any value.
  const any = kinds.includes("unknown");
  const texts = any || kinds.includes("text");
  const numbers = any || kinds.some((k) => k === "int" || k === "float" || k === "int8");
  const blob = any || kinds.includes("bytes") ? undefined : refuse;
  // A column declared BIGINT, INT8 or UNSIGNED BIG INT, and what passes one through: a BigInt. A
  // number past 2^53 is a rounded one where the driver did not read integers exact (sqlite3, D1,
  // libSQL's intMode "number"), and is refused; a number is a REAL where it did, and a REAL with a
  // fraction is one either way, which no BigInt holds.
  if (kind === "int8") return { name, big: (v) => v, num: exact ? refuse : (v) => (Number.isInteger(v) ? bigInteger(v, name) : refuse(v)), str: (v) => (DIGITS.test(v) ? BigInt(v) : refuse(v)), blob };
  if (kind === "text") return { name, big: String, num: undefined, str: undefined, blob };
  if (kind === "int") return { name, big: (v) => exactInteger(v, name), num: exact ? undefined : (v) => exactInteger(v, name), str: (v) => (DIGITS.test(v) ? exactInteger(BigInt(v), name) : refuse(v)), blob };
  // libSQL under intMode "string" hands an INTEGER over as its digits, and says nothing of the storage
  // class or of its mode. Where the column is a REAL's, or a NUMERIC's, those digits can only be an
  // INTEGER's: SQLite stores text that reads as an integer as that integer in a column of REAL or
  // NUMERIC affinity, and a REAL reaches JavaScript as a number. So they are the integer, exact or
  // refused. In a column with no type, or one built from what may be TEXT, the digits may be a
  // TEXT's, and stay as the driver gave them (wiki/driver-decoding.md). Any other text in a column of
  // numbers or BLOBs alone is a TEXT the column's type does not allow.
  const digits = (v: string) => (DIGITS.test(v) ? exactInteger(BigInt(v), name) : texts ? v : refuse(v));
  const str = numericOnly(kinds) || (numbers && !texts) ? digits : texts ? undefined : refuse;
  // A NUMERIC's either: read as numbers, one past 2^53 may be a rounded INTEGER, which is refused.
  if (!exact && kinds.includes("int")) return { name, big: (v) => exactInteger(v, name), num: (v) => (Number.isInteger(v) ? exactInteger(v, name) : v), str, blob };
  // A REAL, a NUMERIC read exact, a column with no type: an integer is exact or refused, a number is
  // as it is. A BLOB's column holds no number.
  if (!numbers && !texts) return { name, big: refuse, num: refuse, str, blob };
  return { name, big: (v) => exactInteger(v, name), num: undefined, str, blob };
}

const TYPE_NAMES: Readonly<Record<string, string>> = { int: "number", float: "number", int8: "bigint", text: "string", bytes: "Uint8Array" };

/**
 * The error for a value of a storage class the column's type does not allow, which SQLite stores in
 * a table that is not STRICT, as the type and the value the message names them.
 */
function refuseClass(column: string, kinds: string[], v: unknown): never {
  const type = [...new Set(kinds.map((k) => TYPE_NAMES[k] ?? k))].join(" | ");
  const held =
    typeof v === "string"
      ? `the text ${JSON.stringify(v.length > 40 ? `${v.slice(0, 40)}…` : v)}`
      : typeof v === "number" || typeof v === "bigint"
        ? `the number ${v}`
        : `a BLOB of ${byteLength(v)} byte${byteLength(v) === 1 ? "" : "s"}`;
  throw new DecodeError(
    `Column "${column}" is typed ${type} and holds ${held}: outside a STRICT table SQLite stores a value of any type in any column, and Reylo refuses one its type does not allow rather than return it. Replace the value, or read what the column holds with cast(${column} as text).`,
  );
}

/** A BLOB's length, as each driver hands one over: a Uint8Array or Buffer, libSQL's ArrayBuffer, D1's array of numbers. */
function byteLength(v: unknown): number {
  if (v instanceof ArrayBuffer || ArrayBuffer.isView(v)) return v.byteLength;
  return Array.isArray(v) ? v.length : 0;
}

/** A REAL's, an INTEGER's or a NUMERIC's column (`int|float|text`, the one kind a NUMERIC has), where integer digits can only be an INTEGER's. */
function numericOnly(kinds: string[]): boolean {
  return kinds.every((k) => k === "int" || k === "float") || (kinds.length === 3 && kinds.includes("int") && kinds.includes("float") && kinds.includes("text"));
}

/**
 * The rows with each value in Reylo's form: a BigInt a number (an error past 2^53), a number past
 * 2^53 in an INTEGER column refused as rounded, a BLOB a Uint8Array, a JSON column parsed and a
 * truth value a boolean. In place, since every driver builds new rows per call. `exact`: the driver
 * read every INTEGER as a BigInt, so a number is a REAL, never a rounded integer. `d1`: an array is
 * D1's BLOB. A row is a plain object on every driver: node:sqlite builds rows with a null
 * prototype, which `assert.deepStrictEqual` and `row.hasOwnProperty` tell apart.
 */
export function normalize(rows: unknown[], columns: Columns | undefined, { exact = false, d1 = false } = {}): unknown[] {
  if (!Array.isArray(rows) || !rows.length) return Array.isArray(rows) ? rows : [];
  // sqlite3 and the sqlite wrapper assign each name to the row they build, with no way to read by position, so the value of this column is gone. `row.__proto__` would read Object.prototype.
  if (hasProto(columns) && !Object.hasOwn(rows[0] as object, "__proto__")) throw new DecodeError(`This client cannot return a column named __proto__: it builds each row by assigning each name, which sets the object's prototype and loses the value. Give the column an alias in the statement.`);
  const plans = (columns ? [...columns] : Object.keys(rows[0] as object).map((name) => [name, "unknown"] as const)).map(([name, kind]) => plan(name, kind, exact));
  const n = plans.length;
  for (const row of rows as Record<string, unknown>[]) {
    if (Object.getPrototypeOf(row) === null) Object.setPrototypeOf(row, Object.prototype);
    for (let i = 0; i < n; i++) {
      const p = plans[i]!;
      const v = row[p.name];
      if (v === null || v === undefined) continue;
      const w = normalized(p, v, d1);
      if (w !== v) row[p.name] = w;
    }
  }
  return rows;
}

/** One non-null value as `normalize` leaves it: converted by the column's plan, refused, or as the driver gave it. */
function normalized(p: Plan, v: unknown, d1: boolean): unknown {
  if (v === null) return v;
  switch (typeof v) {
    case "bigint":
      return p.big(v);
    case "number":
      return p.num ? p.num(v) : v;
    case "string":
      return p.str ? p.str(v) : v;
    case "object":
      if (p.blob) p.blob(v);
      // libSQL's ArrayBuffer, D1's array of numbers.
      if (!(v instanceof Uint8Array) && (v instanceof ArrayBuffer || (d1 && Array.isArray(v)))) return coerce("bytes", v, p.name);
      return v;
  }
  return v;
}

/** One column's non-null value as `normalize` gives it, with `normalize`'s default reading. */
export function sqliteDecoder(name: string, kind: Kind): (v: unknown) => unknown {
  const p = plan(name, kind, false);
  return (v) => normalized(p, v, false);
}
