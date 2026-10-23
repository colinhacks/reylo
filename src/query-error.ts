// The error `.try()` gives: one the database raised, read from the driver's own error into one
// shape per database, whichever driver ran the statement. The codes are the database's own, every
// one it defines (scripts/error-codes.ts writes the lists). Any other error, a refused connection,
// a client Reylo cannot run on, a value Reylo refuses or a row count the analysis got wrong, is not
// the database's, and `.try()` throws it as `.run()` does.
//
// The drivers keep the same facts in different places, measured on each (lab/driver-matrix):
// - Postgres: pg, PGlite and Neon keep the server's fields under pg's names (`constraint`), and
//   postgres.js under the protocol's (`constraint_name`, and `data type_name` with its space).
//   Bun.sql keeps the SQLSTATE in `errno`, with a name of its own in `code`, and writes its own
//   stack's column over `column` where the server sent none.
// - MySQL: mysql2, mariadb and Bun.sql give the number and the state; mariadb's message starts with
//   a header of its own, and its `sqlMessage` (`text` before 3.0) is the server's. mariadb numbers
//   its own errors 45001 to 45999, with a state too. @planetscale/database writes the number and the
//   state into the message.
// - SQLite: better-sqlite3 names the extended code; bun:sqlite and Bun.sql name it and give its
//   number; node:sqlite gives the number; libsql the number and, from 0.17, the primary code's name
//   with the extended one beside it; sqlite3 and the sqlite package only the primary code. D1 and a
//   Durable Object write both codes at the end of the message, and Turso's engine none, or the
//   primary one's number before 0.8.
import type { Dialect } from "./execute.ts";
import type { POSTGRES_CODES } from "./error-codes-postgres.ts";
import type { MYSQL_CODES } from "./error-codes-mysql.ts";
import { SQLITE_CODES } from "./error-codes-sqlite.ts";

/** A SQLSTATE Postgres defines, as `"23505"`. */
export type PostgresCode = keyof typeof POSTGRES_CODES;
/** The condition name of a SQLSTATE Postgres defines, as `"unique_violation"`. */
export type PostgresCondition = (typeof POSTGRES_CODES)[PostgresCode];
/** The name of an error MySQL sends a client, as `"ER_DUP_ENTRY"`. */
export type MysqlCode = (typeof MYSQL_CODES)[number][1][number];
/** The name of a SQLite result code of an error, primary or extended, as `"SQLITE_CONSTRAINT_UNIQUE"`. */
export type SqliteCode = keyof typeof SQLITE_CODES;

/** An error Postgres raised. */
export interface PostgresQueryError extends Error {
  readonly name: "QueryError";
  /**
   * The SQLSTATE, as `"23505"`: one Postgres defines, or one a function raised itself
   * (`raise ... using errcode`), which no list holds.
   */
  readonly code: PostgresCode | (string & {});
  /** The code's condition name, as `"unique_violation"`; undefined for a code Postgres does not define. */
  readonly condition: PostgresCondition | undefined;
  /** `ERROR`, `FATAL` (the session ended) or `PANIC` (the server stopped), in the server's language with pg. */
  readonly severity: string;
  /** The server's message, as `duplicate key value violates unique constraint "users_email_key"`. */
  readonly message: string;
  readonly detail: string | undefined;
  readonly hint: string | undefined;
  /** Where in the statement the error is: a character's place, from 1. */
  readonly position: number | undefined;
  /** The functions the error was raised in, innermost first. */
  readonly where: string | undefined;
  readonly schema: string | undefined;
  readonly table: string | undefined;
  readonly column: string | undefined;
  readonly dataType: string | undefined;
  readonly constraint: string | undefined;
  /** The driver's own error. */
  readonly cause: unknown;
}

/** An error MySQL raised. */
export interface MysqlQueryError extends Error {
  readonly name: "QueryError";
  /** The error's name, as `"ER_DUP_ENTRY"`; undefined for a number MySQL does not define, as one a `signal` statement sets. */
  readonly code: MysqlCode | undefined;
  /** The error's number, as `1062`. */
  readonly errno: number;
  /** The SQLSTATE, as `"23000"`. */
  readonly sqlState: string;
  /** The server's message, as `Duplicate entry 'ada@example.com' for key 'users.email'`. */
  readonly message: string;
  /** The key a duplicate entry is for, as the message names it: `users.email`, its table and its name. */
  readonly key: string | undefined;
  /** The foreign key or check constraint the row fails. */
  readonly constraint: string | undefined;
  /** The table of the foreign key a row fails. */
  readonly table: string | undefined;
  /** The column a value does not fit, or a null or a missing value is refused for. */
  readonly column: string | undefined;
  /** The driver's own error. */
  readonly cause: unknown;
}

/** An error SQLite raised. */
export interface SqliteQueryError extends Error {
  readonly name: "QueryError";
  /**
   * The result code's name: the extended code, as `"SQLITE_CONSTRAINT_UNIQUE"`, where the driver
   * gives it; the primary code, as `"SQLITE_CONSTRAINT"`, from sqlite3 and the sqlite package,
   * which give only that; undefined where the driver gives none, as Turso's engine.
   */
  readonly code: SqliteCode | undefined;
  /** The result code's number, as `2067`. */
  readonly errcode: number | undefined;
  /** SQLite's message, as `UNIQUE constraint failed: users.email`. */
  readonly message: string;
  /** The table of the columns a unique, not-null or type constraint fails on. */
  readonly table: string | undefined;
  /** The columns a unique, not-null or type constraint fails on. */
  readonly columns: readonly string[] | undefined;
  /** The check constraint the row fails (its expression, where it has no name), or the index of an expression a unique row fails. */
  readonly constraint: string | undefined;
  /** The driver's own error. */
  readonly cause: unknown;
}

/** The error a database of the dialect raised. */
export type QueryError<D extends Dialect = Dialect> = D extends "postgres" ? PostgresQueryError : D extends "mysql" ? MysqlQueryError : SqliteQueryError;

class DatabaseError extends Error {
  constructor(message: string, cause: unknown, fields: Record<string, unknown>) {
    super(message, { cause });
    Object.assign(this, fields);
  }
}
Object.defineProperty(DatabaseError.prototype, "name", { value: "QueryError", writable: true, configurable: true });

const SQLSTATE = /^[0-9A-Z]{5}$/;
const text = (v: unknown) => (typeof v === "string" && v !== "" ? v : undefined);
const record = (e: unknown): Record<string, unknown> | undefined => (typeof e === "object" && e !== null ? (e as Record<string, unknown>) : undefined);

/** The database's error `error` is, or undefined where it is another. */
export async function readQueryError(dialect: Dialect, error: unknown): Promise<QueryError | undefined> {
  if (dialect === "sqlite") return readQueryErrorSync(dialect, error);
  if (dialect === "postgres") {
    const read = postgresError(error);
    if (!read) return undefined;
    // The lists are loaded where an error needs one, so a bundle carries them apart from the run time.
    const { POSTGRES_CODES } = await import("./error-codes-postgres.ts");
    return new DatabaseError(read.message, error, { code: read.code, condition: (POSTGRES_CODES as Record<string, string>)[read.code], ...read.fields }) as PostgresQueryError;
  }
  const read = mysqlError(error);
  if (!read) return undefined;
  const { MYSQL_CODES } = await import("./error-codes-mysql.ts");
  let code: string | undefined;
  for (const [first, names] of MYSQL_CODES) if (read.errno >= first && read.errno < first + names.length) code = names[read.errno - first];
  return new DatabaseError(read.message, error, { code, errno: read.errno, sqlState: read.sqlState, ...mysqlNames(read.errno, read.message) }) as MysqlQueryError;
}

/** `readQueryError` for the synchronous SQLite clients, whose list is no import away. */
export function readQueryErrorSync(dialect: Dialect, error: unknown): QueryError | undefined {
  if (dialect !== "sqlite") return undefined;
  const read = sqliteError(error);
  if (!read) return undefined;
  return new DatabaseError(read.message, error, { code: read.code, errcode: read.errcode, ...sqliteNames(read.errcode, read.message) }) as SqliteQueryError;
}

function postgresError(error: unknown) {
  const e = record(error);
  if (!e || typeof e.severity !== "string" || typeof e.message !== "string") return undefined;
  const code = typeof e.code === "string" && SQLSTATE.test(e.code) ? e.code : typeof e.errno === "string" && SQLSTATE.test(e.errno) ? e.errno : undefined;
  if (code === undefined) return undefined;
  const position = text(e.position);
  return {
    code,
    message: e.message,
    fields: {
      severity: e.severity,
      detail: text(e.detail),
      hint: text(e.hint),
      position: position !== undefined && /^\d+$/.test(position) ? Number(position) : undefined,
      where: text(e.where),
      schema: text(e.schema ?? e.schema_name),
      table: text(e.table ?? e.table_name),
      column: text(e.column ?? e.column_name),
      dataType: text(e.dataType ?? e["data type_name"]),
      constraint: text(e.constraint ?? e.constraint_name),
    },
  };
}

function mysqlError(error: unknown) {
  const e = record(error);
  if (!e || typeof e.message !== "string") return undefined;
  let { errno, sqlState } = e;
  let message = text(e.sqlMessage) ?? text(e.text) ?? e.message;
  if (errno === undefined) {
    // `<message> (errno 1062) (sqlstate 23000) during query: …`, the message behind Vitess's
    // `target: …: desc = ` on PlanetScale itself.
    const m = /^(?:[\s\S]*?desc = )?([\s\S]*?) \(errno (\d+)\) \(sqlstate ([0-9A-Z]{5})\)/.exec(e.message);
    if (!m) return undefined;
    [message, errno, sqlState] = [m[1]!, Number(m[2]), m[3]];
  }
  // A system error's number is negative (mysql2 and mariadb give ECONNREFUSED as -61).
  if (typeof errno !== "number" || !Number.isInteger(errno) || errno <= 0 || typeof sqlState !== "string" || !SQLSTATE.test(sqlState)) return undefined;
  if (errno > 45000 && errno < 46000 && /^\(conn[:=]/.test(e.message)) return undefined;
  return { errno, sqlState, message };
}

/** What a MySQL message names, for the errors that name a key, a constraint or a column. */
function mysqlNames(errno: number, message: string) {
  const names: { key?: string | undefined; constraint?: string | undefined; table?: string | undefined; column?: string | undefined } = {};
  let m: RegExpExecArray | null;
  switch (errno) {
    case 1062: // ER_DUP_ENTRY
    case 1586: // ER_DUP_ENTRY_WITH_KEY_NAME
      if ((m = /^Duplicate entry '[\s\S]*' for key '([\s\S]*)'$/.exec(message))) names.key = m[1];
      break;
    case 1451: // ER_ROW_IS_REFERENCED_2
    case 1452: // ER_NO_REFERENCED_ROW_2
      if ((m = /a foreign key constraint fails \(`(?:[^`]|``)*`\.`((?:[^`]|``)*)`, CONSTRAINT `((?:[^`]|``)*)`/.exec(message))) {
        names.table = m[1]!.replaceAll("``", "`");
        names.constraint = m[2]!.replaceAll("``", "`");
      }
      break;
    case 3819: // ER_CHECK_CONSTRAINT_VIOLATED
      if ((m = /^Check constraint '([\s\S]*)' is violated\.$/.exec(message))) names.constraint = m[1];
      break;
    case 1048: // ER_BAD_NULL_ERROR
      if ((m = /^Column '([\s\S]*)' cannot be null$/.exec(message))) names.column = m[1];
      break;
    case 1364: // ER_NO_DEFAULT_FOR_FIELD
      if ((m = /^Field '([\s\S]*)' doesn't have a default value$/.exec(message))) names.column = m[1];
      break;
    case 1264: // ER_WARN_DATA_OUT_OF_RANGE
    case 1265: // WARN_DATA_TRUNCATED
    case 1292: // ER_TRUNCATED_WRONG_VALUE
    case 1366: // ER_TRUNCATED_WRONG_VALUE_FOR_FIELD
    case 1406: // ER_DATA_TOO_LONG
      if ((m = / for column '([\s\S]*)' at row \d+$/.exec(message))) names.column = m[1];
      break;
  }
  return { key: names.key, constraint: names.constraint, table: names.table, column: names.column };
}

const SQLITE_NAMES = new Map<number, SqliteCode>(Object.entries(SQLITE_CODES).map(([name, code]) => [code, name as SqliteCode]));
const SQLITE_NAME = /^SQLITE_[A-Z_]+$/;

function sqliteError(error: unknown) {
  const e = record(error);
  if (!e || typeof e.message !== "string") return undefined;
  let message = e.message;
  const codes: number[] = [];
  let named = false;
  const name = (v: unknown) => {
    if (typeof v !== "string" || !SQLITE_NAME.test(v)) return;
    named = true;
    const code = (SQLITE_CODES as Record<string, number>)[v];
    if (code !== undefined) codes.push(code);
  };
  name(e.extendedCode);
  name(e.code);
  if (typeof e.errcode === "number" && e.code === "ERR_SQLITE_ERROR") codes.push(e.errcode);
  if (typeof e.rawCode === "number") codes.push(e.rawCode);
  if (typeof e.errno === "number" && named) codes.push(e.errno);
  if (named && typeof e.code === "string" && message.startsWith(`${e.code}: `)) message = message.slice(e.code.length + 2);
  if (!named && !codes.length) {
    const tail = /^(?:D1_ERROR: )?([\s\S]*): (SQLITE_[A-Z_]+)(?: \(extended: (SQLITE_[A-Z_]+)\))?$/.exec(message);
    const turso = e.code === "GenericFailure" ? /^(?:prepare|step) failed: (?:Runtime error: )?([\s\S]*?)(?: \((\d+)\))?$/.exec(message) : null;
    if (tail) {
      message = tail[1]!;
      name(tail[3]);
      name(tail[2]);
    } else if (turso) {
      message = turso[1]!;
      if (turso[2]) codes.push(Number(turso[2]));
    } else return undefined;
  }
  // The most specific code given: an extended one where any is.
  const errcode = codes.find((c) => c > 0xff) ?? codes[0];
  return { code: errcode === undefined ? undefined : SQLITE_NAMES.get(errcode), errcode, message };
}

/** What SQLite's message names, for a constraint it failed. */
function sqliteNames(errcode: number | undefined, message: string) {
  const names: { table?: string | undefined; columns?: string[] | undefined; constraint?: string | undefined } = {};
  let m: RegExpExecArray | null;
  if (errcode !== undefined && (errcode & 0xff) === SQLITE_CODES.SQLITE_CONSTRAINT) {
    if ((m = /^UNIQUE constraint failed: index '([\s\S]*)'$/.exec(message))) names.constraint = m[1];
    else if ((m = /^(?:UNIQUE|NOT NULL) constraint failed: ([\s\S]+)$/.exec(message))) {
      const columns = m[1]!.split(", ").map((c) => /^([^.]+)\.([\s\S]+)$/.exec(c));
      const table = columns[0]?.[1];
      if (columns.every((c) => c && c[1] === table)) {
        names.table = table;
        names.columns = columns.map((c) => c![2]!);
      }
    } else if ((m = /^CHECK constraint failed: ([\s\S]*)$/.exec(message))) names.constraint = m[1];
    else if ((m = /^cannot store \S+ value in \S+ column ([^.]+)\.([\s\S]+)$/.exec(message))) {
      names.table = m[1];
      names.columns = [m[2]!];
    }
  }
  return { table: names.table, columns: names.columns, constraint: names.constraint };
}
