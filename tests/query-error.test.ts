import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { DatabaseSync } from "node:sqlite";
import { join } from "node:path";
import { defineQuery, type MysqlClient, type MysqlCode, type MysqlQueryError, type PostgresClient, type PostgresCondition, type PostgresQueryError, type Query, type SqliteCode, type SqliteQueryError } from "../src/index.ts";
import { readQueryError, readQueryErrorSync } from "../src/query-error.ts";
import { DRIVERS, MYSQL, pgLive, mysqlLive, postgresDatabase, driver } from "./helpers.ts";

// `.try()` and the database's error it reads from each driver's own (src/query-error.ts). The
// drivers' errors below are the ones each throws, measured on the real package (lab/driver-matrix
// checks them through `.try()` on every driver). Postgres and MySQL run on throwaway servers
// (REYLO_TEST_PG, REYLO_TEST_MYSQL), each test in a database of its own.

/** A driver's error: an Error with the driver's own fields. */
const thrown = (message: string, fields: Record<string, unknown> = {}, Class: new (message: string) => Error = Error) => Object.assign(new Class(message), fields);

/** What a read error holds, its message and its fields. */
function fields(error: unknown) {
  assert.ok(error instanceof Error);
  assert.equal(error.name, "QueryError");
  return { ...error, message: error.message };
}

const PG_NONE = { severity: "ERROR", detail: undefined, hint: undefined, position: undefined, where: undefined, schema: undefined, table: undefined, column: undefined, dataType: undefined, constraint: undefined };

test("Postgres: the error each driver throws reads as one, with the server's fields under one name each", async () => {
  const cases: [string, Error, Record<string, unknown> | undefined][] = [
    [
      "pg (and pg-promise, Neon's Pool and PGlite, whose class name is minified)",
      thrown(`duplicate key value violates unique constraint "t_email_key"`, { length: 183, name: "error", severity: "ERROR", code: "23505", detail: "Key (email)=(a) already exists.", schema: "rs_probe", table: "t", constraint: "t_email_key", file: "nbtinsert.c", line: "673", routine: "_bt_check_unique" }),
      { ...PG_NONE, message: `duplicate key value violates unique constraint "t_email_key"`, code: "23505", condition: "unique_violation", detail: "Key (email)=(a) already exists.", schema: "rs_probe", table: "t", constraint: "t_email_key" },
    ],
    [
      "postgres.js, which names the fields as the protocol does",
      thrown(`null value in column "email" of relation "t" violates not-null constraint`, { name: "PostgresError", severity_local: "ERROR", severity: "ERROR", code: "23502", detail: "Failing row contains (3, null, 1, null).", schema_name: "rs_probe", table_name: "t", column_name: "email", file: "execMain.c", line: "2219", routine: "ReportNotNullViolationError", query: "insert …" }),
      { ...PG_NONE, message: `null value in column "email" of relation "t" violates not-null constraint`, code: "23502", condition: "not_null_violation", detail: "Failing row contains (3, null, 1, null).", schema: "rs_probe", table: "t", column: "email" },
    ],
    [
      "postgres.js on a domain, whose type is `data type_name`",
      thrown(`value for domain rs_probe.positive violates check constraint "positive_check"`, { severity: "ERROR", code: "23514", schema_name: "rs_probe", "data type_name": "positive", constraint_name: "positive_check" }),
      { ...PG_NONE, message: `value for domain rs_probe.positive violates check constraint "positive_check"`, code: "23514", condition: "check_violation", schema: "rs_probe", dataType: "positive", constraint: "positive_check" },
    ],
    [
      "Bun.sql, which keeps the SQLSTATE in errno and its own stack's column in column",
      thrown(`duplicate key value violates unique constraint "t_email_key"`, { name: "PostgresError", code: "ERR_POSTGRES_SERVER_ERROR", errno: "23505", detail: "Key (email)=(a) already exists.", severity: "ERROR", schema: "rs_probe", table: "t", constraint: "t_email_key", line: "673", column: 27 }),
      { ...PG_NONE, message: `duplicate key value violates unique constraint "t_email_key"`, code: "23505", condition: "unique_violation", detail: "Key (email)=(a) already exists.", schema: "rs_probe", table: "t", constraint: "t_email_key" },
    ],
    [
      "Bun.sql, where the server sent the column",
      thrown(`null value in column "email" of relation "t" violates not-null constraint`, { code: "ERR_POSTGRES_SERVER_ERROR", errno: "23502", severity: "ERROR", schema: "rs_probe", table: "t", column: "email", line: "2219" }),
      { ...PG_NONE, message: `null value in column "email" of relation "t" violates not-null constraint`, code: "23502", condition: "not_null_violation", schema: "rs_probe", table: "t", column: "email" },
    ],
    [
      "a syntax error, its position a number",
      thrown(`syntax error at or near "selec"`, { name: "error", severity: "ERROR", code: "42601", position: "1" }),
      { ...PG_NONE, message: `syntax error at or near "selec"`, code: "42601", condition: "syntax_error", position: 1 },
    ],
    [
      "a code a function raised, which Postgres does not define",
      thrown("custom", { severity: "ERROR", code: "U0001", detail: "d", where: "PL/pgSQL function rs_probe.boom() line 1 at RAISE" }),
      { ...PG_NONE, message: "custom", code: "U0001", condition: undefined, detail: "d", where: "PL/pgSQL function rs_probe.boom() line 1 at RAISE" },
    ],
    ["a refused connection", thrown("connect ECONNREFUSED 127.0.0.1:1", { errno: -61, code: "ECONNREFUSED", syscall: "connect" }), undefined],
    ["a broken pipe, whose code looks like a SQLSTATE", thrown("write EPIPE", { errno: -32, code: "EPIPE", syscall: "write" }), undefined],
    ["Bun.sql's refused connection", thrown("Failed to connect", { name: "PostgresError", code: "ERR_POSTGRES_CONNECTION_REFUSED" }), undefined],
    ["postgres.js's closed connection", thrown("write CONNECTION_CLOSED 127.0.0.1:5432", { code: "CONNECTION_CLOSED", errno: "CONNECTION_CLOSED" }), undefined],
  ];
  for (const [label, error, expected] of cases) {
    const read = await readQueryError("postgres", error);
    if (expected === undefined) {
      assert.equal(read, undefined, label);
      continue;
    }
    assert.deepEqual(fields(read), expected, label);
    assert.equal(read!.cause, error, label);
  }
});

const MY_NONE = { key: undefined, constraint: undefined, table: undefined, column: undefined };

test("MySQL: the error each driver throws reads as one, named from MySQL's own list, with what its message names", async () => {
  const cases: [string, Error, Record<string, unknown> | undefined][] = [
    [
      "mysql2",
      thrown("Duplicate entry 'a' for key 't.email'", { code: "ER_DUP_ENTRY", errno: 1062, sqlState: "23000", sqlMessage: "Duplicate entry 'a' for key 't.email'", sql: "insert …" }),
      { ...MY_NONE, message: "Duplicate entry 'a' for key 't.email'", code: "ER_DUP_ENTRY", errno: 1062, sqlState: "23000", key: "t.email" },
    ],
    [
      "mariadb, whose message has a header of its own",
      thrown("(conn:111, no: 1452, SQLState: 23000) Cannot add or update a child row: a foreign key constraint fails (`rs_probe`.`t`, CONSTRAINT `t_pid` FOREIGN KEY (`pid`) REFERENCES `parent` (`id`))\nsql: insert … - parameters:[]", {
        name: "SqlError",
        sqlMessage: "Cannot add or update a child row: a foreign key constraint fails (`rs_probe`.`t`, CONSTRAINT `t_pid` FOREIGN KEY (`pid`) REFERENCES `parent` (`id`))",
        fatal: false,
        errno: 1452,
        sqlState: "23000",
        code: "ER_NO_REFERENCED_ROW_2",
      }),
      { ...MY_NONE, message: "Cannot add or update a child row: a foreign key constraint fails (`rs_probe`.`t`, CONSTRAINT `t_pid` FOREIGN KEY (`pid`) REFERENCES `parent` (`id`))", code: "ER_NO_REFERENCED_ROW_2", errno: 1452, sqlState: "23000", constraint: "t_pid", table: "t" },
    ],
    [
      "mariadb 2, which keeps the server's message in text",
      thrown("(conn=112, no: 1062, SQLState: 23000) Duplicate entry 'a' for key 't.email'\nsql: insert … - parameters:[]", { text: "Duplicate entry 'a' for key 't.email'", fatal: false, errno: 1062, sqlState: "23000", code: "ER_DUP_ENTRY" }),
      { ...MY_NONE, message: "Duplicate entry 'a' for key 't.email'", code: "ER_DUP_ENTRY", errno: 1062, sqlState: "23000", key: "t.email" },
    ],
    [
      "Bun.sql, which gives a name of its own",
      thrown("You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1", { name: "MySQLError", code: "ERR_MYSQL_SYNTAX_ERROR", errno: 1064, sqlState: "42000" }),
      { ...MY_NONE, message: "You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1", code: "ER_PARSE_ERROR", errno: 1064, sqlState: "42000" },
    ],
    [
      "@planetscale/database, which writes the number and the state into the message",
      thrown("Duplicate entry 'taken' for key 'uniq.PRIMARY' (errno 1062) (sqlstate 23000) during query: insert into uniq (k) values ('taken')", { name: "DatabaseError", status: 400, body: { message: "…", code: "UNKNOWN" } }),
      { ...MY_NONE, message: "Duplicate entry 'taken' for key 'uniq.PRIMARY'", code: "ER_DUP_ENTRY", errno: 1062, sqlState: "23000", key: "uniq.PRIMARY" },
    ],
    [
      "@planetscale/database on PlanetScale itself, behind Vitess's prefix",
      thrown(`target: app.-.primary: vttablet: rpc error: code = AlreadyExists desc = Duplicate entry '1' for key 'users.PRIMARY' (errno 1062) (sqlstate 23000) (CallerID: planetscale-admin): Sql: "insert into users(id) values (:vtg1 /* INT64 */)", BindVars: {REDACTED}`, { name: "DatabaseError", status: 400, body: { code: "UNKNOWN" } }),
      { ...MY_NONE, message: "Duplicate entry '1' for key 'users.PRIMARY'", code: "ER_DUP_ENTRY", errno: 1062, sqlState: "23000", key: "users.PRIMARY" },
    ],
    [
      "a number a signal statement set, which MySQL does not define",
      thrown("custom", { errno: 5001, sqlState: "45000", sqlMessage: "custom" }),
      { ...MY_NONE, message: "custom", code: undefined, errno: 5001, sqlState: "45000" },
    ],
    ["a check", thrown("Check constraint 'n_positive' is violated.", { code: "ER_CHECK_CONSTRAINT_VIOLATED", errno: 3819, sqlState: "HY000", sqlMessage: "Check constraint 'n_positive' is violated." }), { ...MY_NONE, message: "Check constraint 'n_positive' is violated.", code: "ER_CHECK_CONSTRAINT_VIOLATED", errno: 3819, sqlState: "HY000", constraint: "n_positive" }],
    ["a null", thrown("Column 'email' cannot be null", { errno: 1048, sqlState: "23000", sqlMessage: "Column 'email' cannot be null" }), { ...MY_NONE, message: "Column 'email' cannot be null", code: "ER_BAD_NULL_ERROR", errno: 1048, sqlState: "23000", column: "email" }],
    ["a missing value", thrown("Field 'email' doesn't have a default value", { errno: 1364, sqlState: "HY000", sqlMessage: "Field 'email' doesn't have a default value" }), { ...MY_NONE, message: "Field 'email' doesn't have a default value", code: "ER_NO_DEFAULT_FOR_FIELD", errno: 1364, sqlState: "HY000", column: "email" }],
    ["a value too long", thrown("Data too long for column 'email' at row 1", { errno: 1406, sqlState: "22001", sqlMessage: "Data too long for column 'email' at row 1" }), { ...MY_NONE, message: "Data too long for column 'email' at row 1", code: "ER_DATA_TOO_LONG", errno: 1406, sqlState: "22001", column: "email" }],
    ["mysql2's refused connection", thrown("connect ECONNREFUSED 127.0.0.1:1", { errno: -61, code: "ECONNREFUSED", syscall: "connect", fatal: true }), undefined],
    ["mariadb's refused connection, which has a state", thrown("connect ECONNREFUSED 127.0.0.1:1", { errno: -61, code: "ECONNREFUSED", fatal: true, sqlState: "HY000" }), undefined],
    ["mariadb's own error", thrown("(conn:-1, no: 45001, SQLState: 08S01) Cannot execute new commands: connection closed", { sqlMessage: "Cannot execute new commands: connection closed", fatal: true, errno: 45001, sqlState: "08S01", code: "ER_CONNECTION_ALREADY_CLOSED" }), undefined],
    ["mysql2's lost connection", thrown("Connection lost: The server closed the connection.", { code: "PROTOCOL_CONNECTION_LOST", fatal: true }), undefined],
  ];
  for (const [label, error, expected] of cases) {
    const read = await readQueryError("mysql", error);
    if (expected === undefined) {
      assert.equal(read, undefined, label);
      continue;
    }
    assert.deepEqual(fields(read), expected, label);
    assert.equal(read!.cause, error, label);
  }
});

const SQ_NONE = { table: undefined, columns: undefined, constraint: undefined };

test("SQLite: the error each driver throws reads as one, its most specific code named and numbered, with what its message names", () => {
  const cases: [string, Error, Record<string, unknown> | undefined][] = [
    ["better-sqlite3, which names the extended code", thrown("UNIQUE constraint failed: t.id", { code: "SQLITE_CONSTRAINT_PRIMARYKEY" }), { ...SQ_NONE, message: "UNIQUE constraint failed: t.id", code: "SQLITE_CONSTRAINT_PRIMARYKEY", errcode: 1555, table: "t", columns: ["id"] }],
    ["bun:sqlite and Bun.sql, which name and number it", thrown("UNIQUE constraint failed: t.email", { name: "SQLiteError", code: "SQLITE_CONSTRAINT_UNIQUE", errno: 2067, byteOffset: -1 }), { ...SQ_NONE, message: "UNIQUE constraint failed: t.email", code: "SQLITE_CONSTRAINT_UNIQUE", errcode: 2067, table: "t", columns: ["email"] }],
    ["node:sqlite, which numbers it", thrown("UNIQUE constraint failed: index 't_lower'", { code: "ERR_SQLITE_ERROR", errcode: 2067, errstr: "constraint failed" }), { ...SQ_NONE, message: "UNIQUE constraint failed: index 't_lower'", code: "SQLITE_CONSTRAINT_UNIQUE", errcode: 2067, constraint: "t_lower" }],
    ["a unique constraint of two columns", thrown("UNIQUE constraint failed: t.a, t.b", { code: "ERR_SQLITE_ERROR", errcode: 2067 }), { ...SQ_NONE, message: "UNIQUE constraint failed: t.a, t.b", code: "SQLITE_CONSTRAINT_UNIQUE", errcode: 2067, table: "t", columns: ["a", "b"] }],
    ["a null", thrown("NOT NULL constraint failed: t.email", { code: "ERR_SQLITE_ERROR", errcode: 1299 }), { ...SQ_NONE, message: "NOT NULL constraint failed: t.email", code: "SQLITE_CONSTRAINT_NOTNULL", errcode: 1299, table: "t", columns: ["email"] }],
    ["a named check", thrown("CHECK constraint failed: n_positive", { code: "ERR_SQLITE_ERROR", errcode: 275 }), { ...SQ_NONE, message: "CHECK constraint failed: n_positive", code: "SQLITE_CONSTRAINT_CHECK", errcode: 275, constraint: "n_positive" }],
    ["a check with no name, which SQLite names by its expression", thrown("CHECK constraint failed: m < 5", { code: "ERR_SQLITE_ERROR", errcode: 275 }), { ...SQ_NONE, message: "CHECK constraint failed: m < 5", code: "SQLITE_CONSTRAINT_CHECK", errcode: 275, constraint: "m < 5" }],
    ["a value a strict table refuses", thrown("cannot store TEXT value in INTEGER column s.x", { code: "ERR_SQLITE_ERROR", errcode: 3091 }), { ...SQ_NONE, message: "cannot store TEXT value in INTEGER column s.x", code: "SQLITE_CONSTRAINT_DATATYPE", errcode: 3091, table: "s", columns: ["x"] }],
    ["a trigger's raise()", thrown("custom", { code: "ERR_SQLITE_ERROR", errcode: 1811 }), { ...SQ_NONE, message: "custom", code: "SQLITE_CONSTRAINT_TRIGGER", errcode: 1811 }],
    ["a foreign key, which SQLite does not name", thrown("FOREIGN KEY constraint failed", { code: "SQLITE_CONSTRAINT_FOREIGNKEY" }), { ...SQ_NONE, message: "FOREIGN KEY constraint failed", code: "SQLITE_CONSTRAINT_FOREIGNKEY", errcode: 787 }],
    ["a syntax error", thrown(`near "selec": syntax error`, { code: "ERR_SQLITE_ERROR", errcode: 1, errstr: "SQL logic error" }), { ...SQ_NONE, message: `near "selec": syntax error`, code: "SQLITE_ERROR", errcode: 1 }],
    [
      "libsql 0.17 and later, which name the primary code and the extended one beside it, and start the message with the code",
      thrown("SQLITE_CONSTRAINT: CHECK constraint failed: n_positive", { code: "SQLITE_CONSTRAINT", extendedCode: "SQLITE_CONSTRAINT_CHECK", rawCode: 275, name: "LibsqlError" }),
      { ...SQ_NONE, message: "CHECK constraint failed: n_positive", code: "SQLITE_CONSTRAINT_CHECK", errcode: 275, constraint: "n_positive" },
    ],
    ["libsql before 0.17, which names the extended code", thrown("SQLITE_CONSTRAINT_FOREIGNKEY: FOREIGN KEY constraint failed", { code: "SQLITE_CONSTRAINT_FOREIGNKEY", rawCode: 787, name: "LibsqlError" }), { ...SQ_NONE, message: "FOREIGN KEY constraint failed", code: "SQLITE_CONSTRAINT_FOREIGNKEY", errcode: 787 }],
    ["sqlite3 and the sqlite package, which give only the primary code", thrown("SQLITE_CONSTRAINT: UNIQUE constraint failed: index 't_lower'", { errno: 19, code: "SQLITE_CONSTRAINT" }), { ...SQ_NONE, message: "UNIQUE constraint failed: index 't_lower'", code: "SQLITE_CONSTRAINT", errcode: 19, constraint: "t_lower" }],
    ["D1, which writes the codes into the message", thrown("D1_ERROR: NOT NULL constraint failed: t.email: SQLITE_CONSTRAINT (extended: SQLITE_CONSTRAINT_NOTNULL)"), { ...SQ_NONE, message: "NOT NULL constraint failed: t.email", code: "SQLITE_CONSTRAINT_NOTNULL", errcode: 1299, table: "t", columns: ["email"] }],
    ["a Durable Object, which writes them there too", thrown(`near "selec": syntax error at offset 0: SQLITE_ERROR`), { ...SQ_NONE, message: `near "selec": syntax error at offset 0`, code: "SQLITE_ERROR", errcode: 1 }],
    ["Turso's engine from 0.8, which gives no code", thrown("step failed: NOT NULL constraint failed: t.email", { code: "GenericFailure" }), { ...SQ_NONE, message: "NOT NULL constraint failed: t.email", code: undefined, errcode: undefined }],
    ["Turso's engine before 0.8, which gives the primary code's number", thrown("step failed: Runtime error: UNIQUE constraint failed: uniq.k (19)", { code: "GenericFailure" }), { ...SQ_NONE, message: "UNIQUE constraint failed: uniq.k", code: "SQLITE_CONSTRAINT", errcode: 19, table: "uniq", columns: ["k"] }],
    ["better-sqlite3's closed database", thrown("The database connection is not open", {}, TypeError), undefined],
    ["node:sqlite's closed database", thrown("database is not open", { code: "ERR_INVALID_STATE" }), undefined],
    ["D1's lost connection", thrown("D1_ERROR: Network connection lost."), undefined],
    ["Turso's own error", thrown("database is closed", { code: "GenericFailure" }), undefined],
  ];
  for (const [label, error, expected] of cases) {
    const read = readQueryErrorSync("sqlite", error);
    if (expected === undefined) {
      assert.equal(read, undefined, label);
      continue;
    }
    assert.deepEqual(fields(read), expected, label);
    assert.equal(read!.cause, error, label);
  }
});

test("try resolves to the value or to the database's error, and rejects with every other error, as run does", async () => {
  const unique = thrown(`duplicate key value violates unique constraint "users_email_key"`, { severity: "ERROR", code: "23505", table: "users", constraint: "users_email_key" });
  const refused = thrown("connect ECONNREFUSED 127.0.0.1:1", { errno: -61, code: "ECONNREFUSED" });
  const answering = (rows: unknown[]) => ({ query: async () => ({ rows }) });
  const failing = (error: Error) => ({ query: async () => Promise.reject(error) });
  const many = defineQuery<{ id: number }, { id: number }, "many", "postgres">({ dialect: "postgres", sql: "select id from users where id = :id", params: ["id"], cardinality: "many" });
  const one = defineQuery<{ id: number }, { id: number }, "one", "postgres">({ dialect: "postgres", sql: "select id from users where id = :id", params: ["id"], cardinality: "one", source: "get_user.sql" });

  const ok = await many.try(answering([{ id: 1 }]), { id: 1 });
  assert.deepEqual(ok, { value: [{ id: 1 }] });
  assert.ok(!("error" in ok));
  const failed = await many.try(failing(unique), { id: 1 });
  assert.ok(failed.error);
  assert.equal(failed.error.code, "23505");
  assert.equal(failed.error.condition, "unique_violation");
  assert.equal(failed.error.constraint, "users_email_key");
  assert.equal(failed.error.cause, unique);
  assert.ok(!("value" in failed));
  // A refused connection, and a row count the analysis got wrong, are not the database's.
  await assert.rejects(many.try(failing(refused), { id: 1 }), (e) => e === refused);
  await assert.rejects(one.try(answering([]), { id: 1 }), /^Error: get_user\.sql: Expected exactly one row, got 0$/);
  // A value Reylo refuses rejects with Reylo's own error, and never reaches the client.
  await assert.rejects(many.try(answering([]), 1 as never), /The parameters must be an object/);
});

test("trySync returns the value or the database's error on a synchronous SQLite client, and throws every other error", () => {
  const db = new DatabaseSync(":memory:");
  db.exec("create table users (id integer primary key, email text unique not null)");
  const insert = defineQuery<{ email: string | null }, never, "none", "sqlite">({ dialect: "sqlite", sql: "insert into users (email) values (:email)", params: ["email"], cardinality: "none" });
  const get = defineQuery<{ id: number }, { id: number }, "one", "sqlite">({ dialect: "sqlite", sql: "select id from users where id = :id", params: ["id"], cardinality: "one" });
  try {
    assert.deepEqual(insert.trySync(db, { email: "a" }), { value: undefined });
    const unique = insert.trySync(db, { email: "a" });
    assert.ok(unique.error);
    assert.deepEqual(fields(unique.error), { message: "UNIQUE constraint failed: users.email", code: "SQLITE_CONSTRAINT_UNIQUE", errcode: 2067, table: "users", columns: ["email"], constraint: undefined });
    assert.equal(insert.trySync(db, { email: null }).error?.code, "SQLITE_CONSTRAINT_NOTNULL");
    assert.deepEqual(get.trySync(db, { id: 1 }), { value: { id: 1 } });
    assert.throws(() => get.trySync(db, { id: 2 }), /^Error: Expected exactly one row, got 0$/);
    // A closed database is the driver's own error.
    db.close();
    const fresh = defineQuery<{}, { n: number }, "one", "sqlite">({ dialect: "sqlite", sql: "select 1 as n", params: [], cardinality: "one" });
    assert.throws(() => fresh.trySync(db), /database is not open/);
  } finally {
    if (db.isOpen) db.close();
  }
});

test("Postgres: try reads each constraint, the syntax error's position and a function's own code, on pg and postgres.js", { skip: pgLive }, async () => {
  const pg = driver("pg");
  const own = await postgresDatabase("reylo_try");
  const url = new URL(own.url);
  const sql = own.sql;
  const client = new pg.Client({ connectionString: url.href });
  await client.connect();
  try {
    await sql.unsafe(`
      create domain positive as int check (value > 0);
      create table parents (id int primary key);
      create table users (id int primary key, email text not null constraint users_email_key unique, age positive, parent int references parents (id), constraint adult check (age >= 18));
      create function boom() returns int language plpgsql as $$ begin raise exception 'custom' using errcode = 'U0001'; end $$;
      insert into users (id, email, age) values (1, 'a', 20);
    `);
    const write = (statement: string) => defineQuery({ dialect: "postgres", sql: statement, params: [] });
    const cases: [string, Record<string, unknown>][] = [
      ["insert into users (id, email, age) values (2, 'a', 20)", { code: "23505", condition: "unique_violation", schema: "public", table: "users", column: undefined, constraint: "users_email_key", detail: "Key (email)=(a) already exists." }],
      ["insert into users (id, email, age) values (3, null, 20)", { code: "23502", condition: "not_null_violation", schema: "public", table: "users", column: "email", constraint: undefined }],
      ["insert into users (id, email, age, parent) values (4, 'b', 20, 9)", { code: "23503", condition: "foreign_key_violation", table: "users", constraint: "users_parent_fkey" }],
      ["insert into users (id, email, age) values (5, 'c', 17)", { code: "23514", condition: "check_violation", table: "users", constraint: "adult" }],
      ["insert into users (id, email, age) values (6, 'd', -1)", { code: "23514", condition: "check_violation", dataType: "positive", constraint: "positive_check" }],
      // `frm` reads as the column's alias, so the error is at `users`.
      ["select 1 frm users", { code: "42601", condition: "syntax_error", position: 14 }],
      ["select boom()", { code: "U0001", condition: undefined, where: "PL/pgSQL function boom() line 1 at RAISE" }],
    ];
    for (const [statement, expected] of cases) {
      for (const [driver, db] of [["postgres.js", sql], ["pg", client]] as const) {
        const result = await write(statement).try(db);
        assert.ok(result.error, `${driver}: ${statement}`);
        assert.equal(result.error.severity, "ERROR");
        assert.deepEqual(Object.fromEntries(Object.keys(expected).map((k) => [k, (result.error as unknown as Record<string, unknown>)[k]])), expected, `${driver}: ${statement}`);
      }
    }
  } finally {
    await client.end();
    await own.drop();
  }
});

test("MySQL: try reads each constraint and a signal's own number, on mysql2 and mariadb", { skip: mysqlLive }, async () => {
  const mysql = createRequire(join(DRIVERS, "mysql2", "package.json"))("mysql2/promise");
  const mariadb = createRequire(join(DRIVERS, "mariadb", "package.json"))("mariadb");
  const server = new URL(MYSQL!);
  const options = { host: server.hostname, port: Number(server.port), user: decodeURIComponent(server.username) || "root", password: decodeURIComponent(server.password) };
  const name = `reylo_try_${process.pid}`;
  const admin = await mysql.createConnection(options);
  await admin.query(`drop database if exists ${name}`);
  await admin.query(`create database ${name}`);
  const db = await mysql.createConnection({ ...options, database: name, multipleStatements: true });
  const maria = await mariadb.createConnection({ ...options, database: name, allowPublicKeyRetrieval: true });
  try {
    await db.query(`
      create table parents (id int primary key);
      create table users (id int primary key, email varchar(10) not null unique, age int, parent int, constraint adult check (age >= 18), constraint users_parent foreign key (parent) references parents (id));
      create procedure boom() signal sqlstate '45000' set mysql_errno = 5001, message_text = 'custom';
      insert into users (id, email, age) values (1, 'a', 20);
    `);
    const write = (statement: string) => defineQuery({ dialect: "mysql", sql: statement, params: [] });
    const cases: [string, Record<string, unknown>][] = [
      ["insert into users (id, email, age) values (2, 'a', 20)", { code: "ER_DUP_ENTRY", errno: 1062, sqlState: "23000", key: "users.email" }],
      ["insert into users (id, email, age) values (1, 'z', 20)", { code: "ER_DUP_ENTRY", key: "users.PRIMARY" }],
      ["insert into users (id, email, age) values (3, null, 20)", { code: "ER_BAD_NULL_ERROR", column: "email" }],
      ["insert into users (id, age) values (3, 20)", { code: "ER_NO_DEFAULT_FOR_FIELD", column: "email" }],
      ["insert into users (id, email, age, parent) values (4, 'b', 20, 9)", { code: "ER_NO_REFERENCED_ROW_2", errno: 1452, table: "users", constraint: "users_parent" }],
      ["insert into users (id, email, age) values (5, 'c', 17)", { code: "ER_CHECK_CONSTRAINT_VIOLATED", errno: 3819, constraint: "adult" }],
      ["insert into users (id, email, age) values (6, 'abcdefghijklmnop', 20)", { code: "ER_DATA_TOO_LONG", column: "email" }],
      ["selec 1", { code: "ER_PARSE_ERROR", errno: 1064, sqlState: "42000" }],
      ["call boom()", { code: undefined, errno: 5001, sqlState: "45000", message: "custom" }],
    ];
    for (const [statement, expected] of cases) {
      for (const [driver, client] of [["mysql2", db], ["mariadb", maria]] as const) {
        const result = await write(statement).try(client);
        assert.ok(result.error, `${driver}: ${statement}`);
        assert.deepEqual(Object.fromEntries(Object.keys(expected).map((k) => [k, (result.error as unknown as Record<string, unknown>)[k]])), expected, `${driver}: ${statement}`);
      }
    }
  } finally {
    await maria.end();
    await db.end();
    await admin.query(`drop database if exists ${name}`);
    await admin.end();
  }
});

// The types alone: never run, checked by the type check.
export async function types(pg: Query<{}, { id: number }, "one", "postgres">, my: Query<{}, never, "none", "mysql">, lite: Query<{}, never, "none", "sqlite">, pgDb: PostgresClient, myDb: MysqlClient, liteDb: DatabaseSync) {
  const a = await pg.try(pgDb);
  if (a.error) {
    a.value satisfies undefined;
    a.error.condition satisfies PostgresCondition | undefined;
    // A code any function may raise compares, and a condition name Postgres does not define does not.
    if (a.error.code === "23505" || a.error.code === "P0001" || a.error.code === "U0001") {}
    // @ts-expect-error: no such condition
    if (a.error.condition === "unique_violaton") {}
  } else {
    a.value.id satisfies number;
    a.error satisfies undefined;
  }
  // Either field can be read first.
  const id: number | undefined = a.value?.id;
  void id;
  const b = await my.try(myDb);
  if (b.error) {
    b.error.code satisfies MysqlCode | undefined;
    b.error.errno satisfies number;
    // @ts-expect-error: no such name in MySQL's list
    if (b.error.code === "ER_DUP_ENTY") {}
  }
  // A statement that runs only the server's own code: a code Postgres defines, and the names its
  // writes reach.
  const narrowed = null as unknown as PostgresQueryError<{ "23505": { constraint: "users_email_key" | "users_pkey"; table: "users" } }>;
  if (narrowed.condition === "unique_violation") {
    narrowed.code satisfies "23505";
    narrowed.constraint satisfies "users_email_key" | "users_pkey" | undefined;
    // @ts-expect-error: no such constraint
    if (narrowed.constraint === "posts_pkey") {}
  } else {
    narrowed.constraint satisfies string | undefined;
  }
  // @ts-expect-error: a code Postgres does not define, which only a user's code raises
  if (narrowed.code === "U0001") {}
  const myNarrowed = null as unknown as MysqlQueryError<{ ER_DUP_ENTRY: { key: "users.PRIMARY" | "users.email" } }>;
  if (myNarrowed.code === "ER_DUP_ENTRY") {
    myNarrowed.key satisfies "users.PRIMARY" | "users.email" | undefined;
    // @ts-expect-error: no such key
    if (myNarrowed.key === "posts.PRIMARY") {}
  }
  const liteNarrowed = null as unknown as SqliteQueryError<{ SQLITE_CONSTRAINT_UNIQUE: { table: "posts"; columns: readonly ["email"] | readonly ["slug", "lang"] } }>;
  if (liteNarrowed.code === "SQLITE_CONSTRAINT_UNIQUE") {
    liteNarrowed.columns satisfies readonly ["email"] | readonly ["slug", "lang"] | undefined;
    // @ts-expect-error: no such table
    if (liteNarrowed.table === "users") {}
  }
  const c = lite.trySync(liteDb);
  if (c.error) {
    c.error.code satisfies SqliteCode | undefined;
    // @ts-expect-error: no such result code
    if (c.error.code === "SQLITE_CONSTRAINT_UNIQ") {}
  }
}
