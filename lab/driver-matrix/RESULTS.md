# Driver matrix results

Generated 2026-10-23T20:01:54.254Z by `./run.sh` against the working tree at e0f00cd7 (uncommitted changes in src/: no; src/execute.ts e2622fcee6d6, src/index.ts 6da7f60125ad). The report and the diagnoses are in [README.md](README.md).

4869 rows: 4793 pass, 4 fail, 54 unsupported (reylo refuses them with its own TypeError, as its contract says), 15 skip, 3 not run. Failing controls: 36 of 113 control rows fail, as designed.

## Per job

| Package@version | Runtime | Target | Pass | Fail | Unsupported | Skip | Not run |
| --- | --- | --- | --- | --- | --- | --- | --- |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | 113 | 0 | 0 | 0 | 0 |
| pg@7.18.2 | node 26.11.1 | Postgres 14 (local) | 0 | 0 | 0 | 0 | 1 |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | 123 | 0 | 0 | 0 | 0 |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | 59 | 0 | 0 | 0 | 0 |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | 59 | 0 | 0 | 0 | 0 |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | 59 | 0 | 0 | 0 | 0 |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | 59 | 0 | 0 | 0 | 0 |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | 167 | 0 | 0 | 0 | 0 |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | 167 | 0 | 0 | 0 | 0 |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | 167 | 0 | 0 | 0 | 0 |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | 118 | 0 | 0 | 0 | 0 |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | 119 | 0 | 0 | 4 | 0 |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | 159 | 0 | 0 | 0 | 0 |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | 159 | 0 | 0 | 0 | 0 |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | 159 | 0 | 0 | 0 | 0 |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | 159 | 0 | 0 | 0 | 0 |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | 159 | 0 | 0 | 0 | 0 |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | 159 | 0 | 0 | 0 | 0 |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | 164 | 0 | 0 | 0 | 0 |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | 164 | 0 | 0 | 0 | 0 |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | 164 | 0 | 0 | 0 | 0 |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | 103 | 0 | 0 | 0 | 0 |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | 103 | 0 | 0 | 0 | 0 |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | 19 | 0 | 52 | 0 | 0 |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | 69 | 2 | 0 | 0 | 0 |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | 86 | 0 | 0 | 0 | 0 |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | 86 | 0 | 0 | 0 | 0 |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | 71 | 0 | 0 | 0 | 0 |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | 22 | 0 | 0 | 0 | 0 |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | 71 | 0 | 0 | 0 | 0 |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | 86 | 0 | 0 | 0 | 0 |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | 27 | 0 | 0 | 0 | 0 |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | 86 | 0 | 0 | 0 | 0 |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | 83 | 0 | 0 | 3 | 0 |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | 69 | 2 | 0 | 0 | 0 |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | 22 | 0 | 0 | 0 | 0 |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | 22 | 0 | 0 | 0 | 0 |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | 27 | 0 | 0 | 0 | 0 |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | 27 | 0 | 0 | 0 | 0 |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | 27 | 0 | 0 | 0 | 0 |
| better-sqlite3@9.6.0 | node 24.19.0 | SQLite file | 0 | 0 | 0 | 0 | 1 |
| better-sqlite3@8.7.0 | node 24.19.0 | SQLite file | 0 | 0 | 0 | 0 | 1 |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | 71 | 0 | 1 | 0 | 0 |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | 71 | 0 | 1 | 0 | 0 |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | 27 | 0 | 0 | 0 | 0 |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | 76 | 0 | 0 | 0 | 0 |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | 49 | 0 | 0 | 0 | 0 |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | 49 | 0 | 0 | 0 | 0 |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | 49 | 0 | 0 | 0 | 0 |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | 49 | 0 | 0 | 0 | 0 |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | 76 | 0 | 0 | 0 | 0 |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | 26 | 0 | 0 | 1 | 0 |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | 76 | 0 | 0 | 0 | 0 |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | 26 | 0 | 0 | 1 | 0 |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | 49 | 0 | 0 | 0 | 0 |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | 26 | 0 | 0 | 1 | 0 |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | 46 | 0 | 0 | 3 | 0 |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | 25 | 0 | 0 | 2 | 0 |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | 49 | 0 | 0 | 0 | 0 |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | 49 | 0 | 0 | 0 | 0 |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | 49 | 0 | 0 | 0 | 0 |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | 49 | 0 | 0 | 0 | 0 |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | 44 | 0 | 0 | 0 | 0 |

## Failures, grouped

One row per package, shape and set of failing scenarios, across the versions, runtimes and targets that fail the same way. The error is the one of the first failing scenario; every row is in the last table.

| Package | Versions | Runtimes | Targets | Shape | Failing scenarios | First error |
| --- | --- | --- | --- | --- | --- | --- |
| pg | 7.18.2 | node 26.11.1 | Postgres 14 (local) | (job) | preflight | not run: the driver cannot run a query without reylo: Error: Connection terminated due to connection timeout |
| @planetscale/database | 1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S8 | UnknownError: Expected JSON response from database API, got HTTP 403 Forbidden [body.code=UNKNOWN] |
| @planetscale/database | 1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S8 | UnknownError: Expected JSON response from database API, got HTTP 403 Forbidden [body.code=UNKNOWN] |
| drizzle-orm (planetscale-serverless) | 0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S8 | UnknownError: Expected JSON response from database API, got HTTP 403 Forbidden [body.code=UNKNOWN] |
| drizzle-orm (planetscale-serverless) | 0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S8 | UnknownError: Expected JSON response from database API, got HTTP 403 Forbidden [body.code=UNKNOWN] |
| better-sqlite3 | 9.6.0, 8.7.0 | node 24.19.0 | SQLite file | (job) | setup | not run: no prebuilt binary for Node 24.19.0 (ABI 137), and native-old/ has no source build of it against that Node's headers (see README) |

## Unsupported by design

Rows where reylo refuses the client or the statement with its own TypeError, which is its contract for that shape: the row checks the refusal (its message, and for PlanetScale 2.0 that its cause is the driver's error).

| Package | Versions | Runtimes | Shape | Scenarios | reylo's refusal |
| --- | --- | --- | --- | --- | --- |
| @planetscale/database | 2.0.0 | node 26.11.1 | connect() Connection | S1a, S1b, S1c, S1g, S1h, S2b, S2c, S2d, S2e, S3, S5a, S5b, S6b, S6c, S11, S8 | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database | 2.0.0 | node 26.11.1 | Client | S1a, S1b, S1c, S1g, S1h, S2b, S2c, S2d, S2e, S3, S5a, S5b, S6b, S6c, S11, S8 | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database | 2.0.0 | node 26.11.1 | conn.transaction(tx) | S1a, S1b, S1c, S1g, S1h, S2b, S2c, S2d, S2e, S3, S5a, S5b, S6b, S6c, S7a, S7b, S7c, S7d, S11, S8 | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| node:sqlite | 26.11.1, 24.22.0 | node 26.11.1, node 24.22.0 | db.createTagStore() | S9 | reylo refused it: reylo cannot run a sqlite statement on an object with get, all, iterate, run, clear. It takes a client with prepare(sql) as on better-sqlite3, node:sqlite, bun:sqlite, Turso's @tursodatabase/database and Cloudflare D1; exec(sql, ...values) as on a Durable Object's ctx.storage.sql; execute({ sql, args }) as on @libsql/client; all(sql, params) as on sqlite3 and sqlite; unsafe(text, values) as on Bun.sql; or a Drizzle database over one of them. A tag store is not a client: pass its database, store.db. |

## Failing controls

Deliberately broken clients (drivers/control.ts). Every row marked FAIL here is a scenario that caught the breakage; a pass is a scenario the breakage does not touch.

| Control | Shape | Scenario | Result | Error |
| --- | --- | --- | --- | --- |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S1a params: int, text, bool, null | FAIL | Error: Expected exactly one row, got 0 |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S1b omitted param (undefined) binds NULL | FAIL | rows: expected [{"id":1}], got [{"id":2}] |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | FAIL | Error: Invalid input for bytea type |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S1d array params: = any(:ids) over an int and a text array | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S1f array params: bool[], timestamptz[] and bytea[] | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S2a many: 3 rows | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S2b one: the row | FAIL | row: expected {"id":2,"name":"bob"}, got {"id":3,"name":"cy"} |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S2c maybeOne: zero rows gives null | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S2d maybeOne: two rows throw reylo's error | FAIL | wrong error: Error: Expected at most one row, got 3 |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S2e with(client)(params) | FAIL | Error: Expected exactly one row, got 0 |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S3 write without RETURNING (none), then a separate read | FAIL | read back through reylo: expected {"n":3}, got {"n":4} |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S3n write without parameters (none), then a separate read | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S4 INSERT ... RETURNING | FAIL | returned row: expected {"n":4,"tag":"control@pg/parameters corrupted: an integer plus one, a Uint8Array as JSON/S4/mvlebm8x77z8/3"}, got {"tag":"control@pg/parameters corrupted: an integer plus one, a Uint8Array as JSON/S4/mvlebm8x77z8/3","n":5} |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S5a repeated :id (runtime lexer) | FAIL | Error: Expected exactly one row, got 0 |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S5b repeated :id (mapper text/order) | FAIL | Error: Expected exactly one row, got 0 |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S6a syntax error rejects with the driver's error | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S6b unique violation rejects with the driver's error | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S6c try: a unique violation resolves to the database's error, read the same on every driver | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | PASS |  |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | FAIL | row: expected {"amount":"string(12345678901234567890.123456789)","at":"string(2026-10-20T10:00:00.123)","big":"bigint(9007199254740993)","day":"string(2026-10-20)","doc":{"a":["number(1)","number(2)"]},"docb":{"b":"boolean(true)"},"f8":"number(2.5)","flag":"boolean(true)","id":"number(1)","ints":["number(1)","number(2)","number(3)"],"moods":["string(ok)","string(sad)"],"raw":"Uint8Array(1,2,255)","span":"string(P1DT2H)","stamp":"string(2026-10-20T08:00:00.123Z)","tags":["string(a)",null,"string(NULL)","string(b,c)"],"uid":"string(a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11)"}, got {"amount":"string(12345678901234567890.123456789)","at":"Date(2026-10-20T17:00:00.123Z)","big":"bigint(9007199254740993)","day":"Date(2026-10-20T00:00:00.000Z)","doc":{"a":["number(1)","number(2)"]},"docb":{"b":"boolean(true)"},"f8":"number(2.5)","flag":"boolean(true)","id":"number(1)","ints":["number(1)","number(2)","number(3)"],"moods":"string({ok,sad})","raw":"Uint8Array(1,2,255)","span":"string(P1DT2H)","stamp":"Date(2026-10-20T08:00:00.123Z)","tags":["string(a)",null,"string(NULL)","string(b,c)"],"uid":"string(a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11)"} |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | FAIL | ParameterError: Parameter "day" is a date without a time zone, so it takes the wall-clock text ("2026-10-12 19:06:48.282", "2026-10-12"), not a Date: drivers write a Date's time in different zones (the app's, the server's or UTC). |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S11b round trip: an instant past 9999 and one before 1 go back in | FAIL | the instants past 9999 and before 1: expected {"bc":"-000001-01-01T00:00:00Z","far":"+010000-01-01T00:00:00Z"}, got {"far":"+010000-01-01T00:00:00.000Z","bc":null} |
| control/pg | parameters corrupted: an integer plus one, a Uint8Array as JSON | S8 20 concurrent queries resolve with the right rows | FAIL | Error: Expected exactly one row, got 0 |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S1a params: int, text, bool, null | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S1b omitted param (undefined) binds NULL | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S1d array params: = any(:ids) over an int and a text array | FAIL | me: malformed array literal: "1,3,4" [code=22P02 severity=ERROR] |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | FAIL | me: invalid input syntax for type json [code=22P02 severity=ERROR] |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | FAIL | me: invalid input syntax for type json [code=22P02 severity=ERROR] |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | FAIL | reylo contract: a Date was not refused with reylo's TypeError /^Parameter "day" is a date without a time zone/: non-Error thrown: null |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S1f array params: bool[], timestamptz[] and bytea[] | FAIL | me: malformed array literal: "true,false" [code=22P02 severity=ERROR] |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S2a many: 3 rows | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S2b one: the row | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S2c maybeOne: zero rows gives null | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S2d maybeOne: two rows throw reylo's error | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S2e with(client)(params) | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S3 write without RETURNING (none), then a separate read | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S3n write without parameters (none), then a separate read | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S4 INSERT ... RETURNING | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S5a repeated :id (runtime lexer) | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S5b repeated :id (mapper text/order) | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S6a syntax error rejects with the driver's error | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S6b unique violation rejects with the driver's error | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S6c try: a unique violation resolves to the database's error, read the same on every driver | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | PASS |  |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | FAIL | row: expected {"amount":"string(12345678901234567890.123456789)","at":"string(2026-10-20T10:00:00.123)","big":"bigint(9007199254740993)","day":"string(2026-10-20)","doc":{"a":["number(1)","number(2)"]},"docb":{"b":"boolean(true)"},"f8":"number(2.5)","flag":"boolean(true)","id":"number(1)","ints":["number(1)","number(2)","number(3)"],"moods":["string(ok)","string(sad)"],"raw":"Uint8Array(1,2,255)","span":"string(P1DT2H)","stamp":"string(2026-10-20T08:00:00.123Z)","tags":["string(a)",null,"string(NULL)","string(b,c)"],"uid":"string(a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11)"}, got {"amount":"string(12345678901234567890.123456789)","at":"string(2026-10-20T17:00:00.123)","big":"bigint(9007199254740993)","day":"string(2026-10-20)","doc":{"a":["number(1)","number(2)"]},"docb":{"b":"boolean(true)"},"f8":"number(2.5)","flag":"boolean(true)","id":"number(1)","ints":["number(1)","number(2)","number(3)"],"moods":["string(ok)","string(sad)"],"raw":"Uint8Array(1,2,255)","span":"string(P1DT2H)","stamp":"string(2026-10-20T08:00:00.123Z)","tags":["string(a)",null,"string(NULL)","string(b,c)"],"uid":"string(a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11)"} |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | FAIL | me: malformed array literal: "a,NULL,b,c" [code=22P02 severity=ERROR] |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S11b round trip: an instant past 9999 and one before 1 go back in | FAIL | DecodeError: Column "bc": the driver returned an Invalid Date, which is how Bun.sql decodes a date or timestamp before 1 AD. Reylo refuses it rather than return a wrong value. |
| control/pg | unsafe() binding an array as a,b, statements without inputs | S8 20 concurrent queries resolve with the right rows | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S1a params: int, text, bool, null | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S1b omitted param (undefined) binds NULL | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S1d array params: = any(:ids) over an int and a text array | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | FAIL | me: invalid input syntax for type json [code=22P02 severity=ERROR] |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | FAIL | me: invalid input syntax for type json [code=22P02 severity=ERROR] |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S1f array params: bool[], timestamptz[] and bytea[] | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S2a many: 3 rows | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S2b one: the row | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S2c maybeOne: zero rows gives null | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S2d maybeOne: two rows throw reylo's error | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S2e with(client)(params) | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S3 write without RETURNING (none), then a separate read | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S3n write without parameters (none), then a separate read | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S4 INSERT ... RETURNING | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S5a repeated :id (runtime lexer) | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S5b repeated :id (mapper text/order) | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S6a syntax error rejects with the driver's error | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S6b unique violation rejects with the driver's error | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S6c try: a unique violation resolves to the database's error, read the same on every driver | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | PASS |  |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | FAIL | row: expected {"amount":"string(12345678901234567890.123456789)","at":"string(2026-10-20T10:00:00.123)","big":"bigint(9007199254740993)","day":"string(2026-10-20)","doc":{"a":["number(1)","number(2)"]},"docb":{"b":"boolean(true)"},"f8":"number(2.5)","flag":"boolean(true)","id":"number(1)","ints":["number(1)","number(2)","number(3)"],"moods":["string(ok)","string(sad)"],"raw":"Uint8Array(1,2,255)","span":"string(P1DT2H)","stamp":"string(2026-10-20T08:00:00.123Z)","tags":["string(a)",null,"string(NULL)","string(b,c)"],"uid":"string(a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11)"}, got {"amount":"string(12345678901234567890.123456789)","at":"string(2026-10-20T17:00:00.123)","big":"bigint(9007199254740993)","day":"string(2026-10-20)","doc":{"a":["number(1)","number(2)"]},"docb":{"b":"boolean(true)"},"f8":"number(2.5)","flag":"boolean(true)","id":"number(1)","ints":["number(1)","number(2)","number(3)"],"moods":["string(ok)","string(sad)"],"raw":"Uint8Array(1,2,255)","span":"string(P1DT2H)","stamp":"string(2026-10-20T08:00:00.123Z)","tags":["string(a)",null,"string(NULL)","string(b,c)"],"uid":"string(a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11)"} |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | FAIL | each column equal to the value it returned: expected {"amount":1,"at":1,"big":1,"day":1,"doc":1,"docb":1,"f8":1,"flag":1,"ints":1,"moods":1,"raw":1,"span":1,"stamp":1,"tags":1,"uid":1}, got {"big":true,"amount":true,"day":true,"at":false,"stamp":true,"doc":true,"docb":true,"raw":true,"flag":true,"f8":true,"tags":true,"ints":true,"moods":true,"span":true,"uid":true} |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S11b round trip: an instant past 9999 and one before 1 go back in | FAIL | DecodeError: Column "bc": the driver returned an Invalid Date, which is how Bun.sql decodes a date or timestamp before 1 AD. Reylo refuses it rather than return a wrong value. |
| control/pg | unsafe() binding every array as its array literal, a JSON parameter's too | S8 20 concurrent queries resolve with the right rows | PASS |  |
| control/pg | rollback that commits | S7a tx: write then commit is visible | PASS |  |
| control/pg | rollback that commits | S7b tx: write then rollback is not visible | FAIL | raw count after rollback: expected 0, got 1 |
| control/pg | rollback that commits | S7c tx: savepoint rollback keeps the outer write | FAIL | raw counts [outer, inner]: expected [1,0], got [1,1] |
| control/pg | rollback that commits | S7d tx: a failing statement rolls the transaction back | FAIL | the transaction did not reject |
| control/pg | rollback that commits | S7n tx: a write without parameters, then commit, is visible | PASS |  |
| control/sqlite | sync client declared async | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | FAIL | reylo contract: runSync on an asynchronous client did not throw reylo's TypeError: non-Error thrown: null |
| control/sqlite | sync client declared async | S1a params: int, text, bool, null | PASS |  |
| control/sqlite | sync client declared async | S1b omitted param (undefined) binds NULL | PASS |  |
| control/sqlite | sync client declared async | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | PASS |  |
| control/sqlite | sync client declared async | S2a many: 3 rows | PASS |  |
| control/sqlite | sync client declared async | S2b one: the row | PASS |  |
| control/sqlite | sync client declared async | S2c maybeOne: zero rows gives null | PASS |  |
| control/sqlite | sync client declared async | S2d maybeOne: two rows throw reylo's error | PASS |  |
| control/sqlite | sync client declared async | S2e with(client)(params) | PASS |  |
| control/sqlite | sync client declared async | S3 write without RETURNING (none), then a separate read | PASS |  |
| control/sqlite | sync client declared async | S3n write without parameters (none), then a separate read | PASS |  |
| control/sqlite | sync client declared async | S4 INSERT ... RETURNING | PASS |  |
| control/sqlite | sync client declared async | S5a repeated :id (runtime lexer) | PASS |  |
| control/sqlite | sync client declared async | S5b repeated :id (mapper text/order) | PASS |  |
| control/sqlite | sync client declared async | S6a syntax error rejects with the driver's error | PASS |  |
| control/sqlite | sync client declared async | S6b unique violation rejects with the driver's error | PASS |  |
| control/sqlite | sync client declared async | S6c try: a unique violation resolves to the database's error, read the same on every driver | PASS |  |
| control/sqlite | sync client declared async | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | PASS |  |
| control/sqlite | sync client declared async | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | PASS |  |
| control/sqlite | sync client declared async | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | PASS |  |
| control/sqlite | sync client declared async | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | PASS |  |
| control/sqlite | sync client declared async | S8 20 concurrent queries resolve with the right rows | PASS |  |
| control/sqlite | db.transaction(fn) whose statements answer through a promise | S7a tx: write then commit is visible | FAIL | TypeError: runSync takes a synchronous SQLite client (better-sqlite3, node:sqlite, bun:sqlite, the Database of @tursodatabase/database/compat or a Durable Object's ctx.storage.sql); this client's statement answered with a promise. run takes every client. |
| control/sqlite | db.transaction(fn) whose statements answer through a promise | S7b tx: write then rollback is not visible | FAIL | TypeError: runSync takes a synchronous SQLite client (better-sqlite3, node:sqlite, bun:sqlite, the Database of @tursodatabase/database/compat or a Durable Object's ctx.storage.sql); this client's statement answered with a promise. run takes every client. |
| control/sqlite | db.transaction(fn) whose statements answer through a promise | S7c tx: savepoint rollback keeps the outer write | FAIL | TypeError: runSync takes a synchronous SQLite client (better-sqlite3, node:sqlite, bun:sqlite, the Database of @tursodatabase/database/compat or a Durable Object's ctx.storage.sql); this client's statement answered with a promise. run takes every client. |
| control/sqlite | db.transaction(fn) whose statements answer through a promise | S7d tx: a failing statement rolls the transaction back | FAIL | not the driver's unique-violation error: TypeError: runSync takes a synchronous SQLite client (better-sqlite3, node:sqlite, bun:sqlite, the Database of @tursodatabase/database/compat or a Durable Object's ctx.storage.sql); this client's statement answered with a promise. run takes every client. |
| control/sqlite | db.transaction(fn) whose statements answer through a promise | S7e tx: a read inside transaction(fn) is usable synchronously | FAIL | TypeError: runSync takes a synchronous SQLite client (better-sqlite3, node:sqlite, bun:sqlite, the Database of @tursodatabase/database/compat or a Durable Object's ctx.storage.sql); this client's statement answered with a promise. run takes every client. |

## Type check

`tsc` Version 7.1.0-dev.20260918.1 over typecheck/cases.ts: every client and transaction object passed to a query's run (ClientFor<D>), and the type run returns. Details in [typecheck/RESULTS.md](typecheck/RESULTS.md).

714 cases over 121 client types: 714 as expected, 0 wrongly accepted, 0 wrongly rejected.

## Every row

| Package@version | Runtime | Target | Shape | Scenario | Result | Error or detail |
| --- | --- | --- | --- | --- | --- | --- |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S1a params: int, text, bool, null | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S1b omitted param (undefined) binds NULL | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S1d array params: = any(:ids) over an int and a text array | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S2a many: 3 rows | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S2b one: the row | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S2c maybeOne: zero rows gives null | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S2d maybeOne: two rows throw reylo's error | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S2e with(client)(params) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S3 write without RETURNING (none), then a separate read | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S3n write without parameters (none), then a separate read | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S4 INSERT ... RETURNING | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S5a repeated :id (runtime lexer) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S5b repeated :id (mapper text/order) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Pool | S8 20 concurrent queries resolve with the right rows | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S1a params: int, text, bool, null | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S1b omitted param (undefined) binds NULL | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S2a many: 3 rows | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S2b one: the row | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S2c maybeOne: zero rows gives null | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S2d maybeOne: two rows throw reylo's error | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S2e with(client)(params) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S3 write without RETURNING (none), then a separate read | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S3n write without parameters (none), then a separate read | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S4 INSERT ... RETURNING | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S5a repeated :id (runtime lexer) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S5b repeated :id (mapper text/order) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | Client (connected) | S8 20 concurrent queries resolve with the right rows | pass | warning: DeprecationWarning: Calling client.query() when the client is already executing a query is deprecated and will be removed in pg@9.0. Use async/await or an external async flow control mechanism instead. |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S1a params: int, text, bool, null | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S1b omitted param (undefined) binds NULL | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S2a many: 3 rows | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S2b one: the row | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S2c maybeOne: zero rows gives null | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S2e with(client)(params) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S3 write without RETURNING (none), then a separate read | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S3n write without parameters (none), then a separate read | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S4 INSERT ... RETURNING | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S5a repeated :id (runtime lexer) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S5b repeated :id (mapper text/order) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient (pool.connect()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S1a params: int, text, bool, null | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S1b omitted param (undefined) binds NULL | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S1d array params: = any(:ids) over an int and a text array | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S2a many: 3 rows | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S2b one: the row | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S2c maybeOne: zero rows gives null | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S2d maybeOne: two rows throw reylo's error | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S2e with(client)(params) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S3 write without RETURNING (none), then a separate read | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S3n write without parameters (none), then a separate read | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S4 INSERT ... RETURNING | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S5a repeated :id (runtime lexer) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S5b repeated :id (mapper text/order) | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S7a tx: write then commit is visible | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S7b tx: write then rollback is not visible | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S7c tx: savepoint rollback keeps the outer write | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S7d tx: a failing statement rolls the transaction back | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S7n tx: a write without parameters, then commit, is visible | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| pg@8.23.0 | node 26.11.1 | Postgres 14 (local) | PoolClient in BEGIN/COMMIT | S8 20 concurrent queries resolve with the right rows | pass |  |
| pg@7.18.2 | node 26.11.1 | Postgres 14 (local) | (job) | preflight | NOT RUN | the driver cannot run a query without reylo: Error: Connection terminated due to connection timeout |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S1a params: int, text, bool, null | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S1b omitted param (undefined) binds NULL | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S1d array params: = any(:ids) over an int and a text array | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S2a many: 3 rows | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S2b one: the row | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S2c maybeOne: zero rows gives null | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S2d maybeOne: two rows throw reylo's error | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S2e with(client)(params) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S3 write without RETURNING (none), then a separate read | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S3n write without parameters (none), then a separate read | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S4 INSERT ... RETURNING | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S5a repeated :id (runtime lexer) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S5b repeated :id (mapper text/order) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql | S8 20 concurrent queries resolve with the right rows | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S1a params: int, text, bool, null | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S2a many: 3 rows | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S2b one: the row | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S2c maybeOne: zero rows gives null | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S2e with(client)(params) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S3n write without parameters (none), then a separate read | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S4 INSERT ... RETURNING | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S5a repeated :id (runtime lexer) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S5b repeated :id (mapper text/order) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S7a tx: write then commit is visible | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S7b tx: write then rollback is not visible | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S7d tx: a failing statement rolls the transaction back | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.begin(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S1a params: int, text, bool, null | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S1b omitted param (undefined) binds NULL | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S2a many: 3 rows | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S2b one: the row | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S2c maybeOne: zero rows gives null | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S2d maybeOne: two rows throw reylo's error | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S2e with(client)(params) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S3 write without RETURNING (none), then a separate read | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S3n write without parameters (none), then a separate read | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S4 INSERT ... RETURNING | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S5a repeated :id (runtime lexer) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S5b repeated :id (mapper text/order) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S7a tx: write then commit is visible | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S7b tx: write then rollback is not visible | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S7d tx: a failing statement rolls the transaction back | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | tx.savepoint(sp) | S8 20 concurrent queries resolve with the right rows | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S1a params: int, text, bool, null | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S1b omitted param (undefined) binds NULL | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S1d array params: = any(:ids) over an int and a text array | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S2a many: 3 rows | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S2b one: the row | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S2c maybeOne: zero rows gives null | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S2d maybeOne: two rows throw reylo's error | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S2e with(client)(params) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S3 write without RETURNING (none), then a separate read | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S3n write without parameters (none), then a separate read | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S4 INSERT ... RETURNING | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S5a repeated :id (runtime lexer) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S5b repeated :id (mapper text/order) | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S7a tx: write then commit is visible | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S7b tx: write then rollback is not visible | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S7d tx: a failing statement rolls the transaction back | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| postgres@3.4.9 | node 26.11.1 | Postgres 14 (local) | sql.reserve() | S8 20 concurrent queries resolve with the right rows | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S1a params: int, text, bool, null | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S1b omitted param (undefined) binds NULL | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S2a many: 3 rows | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S2b one: the row | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S2c maybeOne: zero rows gives null | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S2d maybeOne: two rows throw reylo's error | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S2e with(client)(params) | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S3 write without RETURNING (none), then a separate read | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S3n write without parameters (none), then a separate read | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S4 INSERT ... RETURNING | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S5a repeated :id (runtime lexer) | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S5b repeated :id (mapper text/order) | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S6a syntax error rejects with the driver's error | pass | me: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S6b unique violation rejects with the driver's error | pass | me: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | PGlite | S8 20 concurrent queries resolve with the right rows | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2a many: 3 rows | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2b one: the row | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2e with(client)(params) | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S4 INSERT ... RETURNING | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | me: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | me: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | me: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @electric-sql/pglite@0.5.8 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S1a params: int, text, bool, null | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S1b omitted param (undefined) binds NULL | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S2a many: 3 rows | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S2b one: the row | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S2c maybeOne: zero rows gives null | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S2d maybeOne: two rows throw reylo's error | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S2e with(client)(params) | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S3 write without RETURNING (none), then a separate read | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S3n write without parameters (none), then a separate read | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S4 INSERT ... RETURNING | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S5a repeated :id (runtime lexer) | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S5b repeated :id (mapper text/order) | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S6a syntax error rejects with the driver's error | pass | me: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S6b unique violation rejects with the driver's error | pass | me: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | PGlite | S8 20 concurrent queries resolve with the right rows | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2a many: 3 rows | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2b one: the row | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2e with(client)(params) | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S4 INSERT ... RETURNING | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | me: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | me: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | me: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @electric-sql/pglite@0.4.6 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S1a params: int, text, bool, null | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S1b omitted param (undefined) binds NULL | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S2a many: 3 rows | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S2b one: the row | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S2c maybeOne: zero rows gives null | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S2d maybeOne: two rows throw reylo's error | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S2e with(client)(params) | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S3 write without RETURNING (none), then a separate read | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S3n write without parameters (none), then a separate read | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S4 INSERT ... RETURNING | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S5a repeated :id (runtime lexer) | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S5b repeated :id (mapper text/order) | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S6a syntax error rejects with the driver's error | pass | ne: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S6b unique violation rejects with the driver's error | pass | ne: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | PGlite | S8 20 concurrent queries resolve with the right rows | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2a many: 3 rows | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2b one: the row | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2e with(client)(params) | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S4 INSERT ... RETURNING | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | ne: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | ne: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | ne: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @electric-sql/pglite@0.3.16 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S1a params: int, text, bool, null | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S1b omitted param (undefined) binds NULL | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S2a many: 3 rows | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S2b one: the row | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S2c maybeOne: zero rows gives null | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S2d maybeOne: two rows throw reylo's error | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S2e with(client)(params) | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S3 write without RETURNING (none), then a separate read | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S3n write without parameters (none), then a separate read | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S4 INSERT ... RETURNING | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S5a repeated :id (runtime lexer) | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S5b repeated :id (mapper text/order) | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S6a syntax error rejects with the driver's error | pass | xe: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S6b unique violation rejects with the driver's error | pass | xe: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | PGlite | S8 20 concurrent queries resolve with the right rows | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2a many: 3 rows | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2b one: the row | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2e with(client)(params) | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S4 INSERT ... RETURNING | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | xe: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | xe: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | xe: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @electric-sql/pglite@0.2.17 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2b one: the row | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2b one: the row | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2b one: the row | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2b one: the row | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7a tx: write then commit is visible | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7b tx: write then rollback is not visible | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7d tx: a failing statement rolls the transaction back | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2b one: the row | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6a syntax error rejects with the driver's error | pass | NeonDbError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6b unique violation rejects with the driver's error | pass | NeonDbError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2b one: the row | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6a syntax error rejects with the driver's error | pass | NeonDbError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6b unique violation rejects with the driver's error | pass | NeonDbError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@1.1.0 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2b one: the row | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2b one: the row | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2b one: the row | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2b one: the row | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7a tx: write then commit is visible | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7b tx: write then rollback is not visible | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7d tx: a failing statement rolls the transaction back | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called the client as a function (or a method the harness could not observe); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2b one: the row | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6a syntax error rejects with the driver's error | pass | NeonDbError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6b unique violation rejects with the driver's error | pass | NeonDbError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called the client as a function (or a method the harness could not observe); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2b one: the row | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6a syntax error rejects with the driver's error | pass | NeonDbError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6b unique violation rejects with the driver's error | pass | NeonDbError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.10.4 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2b one: the row | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Pool (WebSocket) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2b one: the row | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | Client (WebSocket, connected) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2b one: the row | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient (pool.connect()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2b one: the row | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7a tx: write then commit is visible | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7b tx: write then rollback is not visible | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7d tx: a failing statement rolls the transaction back | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | PoolClient in BEGIN/COMMIT | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called the client as a function (or a method the harness could not observe); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2b one: the row | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6a syntax error rejects with the driver's error | pass | NeonDbError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6b unique violation rejects with the driver's error | pass | NeonDbError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon() HTTP function | S8 20 concurrent queries resolve with the right rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called the client as a function (or a method the harness could not observe); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1a params: int, text, bool, null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1b omitted param (undefined) binds NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2a many: 3 rows | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2b one: the row | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2c maybeOne: zero rows gives null | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S2e with(client)(params) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S3 write without RETURNING (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S3n write without parameters (none), then a separate read | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S4 INSERT ... RETURNING | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S5a repeated :id (runtime lexer) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S5b repeated :id (mapper text/order) | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6a syntax error rejects with the driver's error | pass | NeonDbError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6b unique violation rejects with the driver's error | pass | NeonDbError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| @neondatabase/serverless@0.9.5 | node 26.11.1 | Postgres 14 via local Neon proxy | neon(url, { fullResults: true }) | S8 20 concurrent queries resolve with the right rows | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S1a params: int, text, bool, null | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S1b omitted param (undefined) binds NULL | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S1d array params: = any(:ids) over an int and a text array | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S2a many: 3 rows | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S2b one: the row | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S2c maybeOne: zero rows gives null | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S2d maybeOne: two rows throw reylo's error | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S2e with(client)(params) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S3 write without RETURNING (none), then a separate read | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S3n write without parameters (none), then a separate read | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S4 INSERT ... RETURNING | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S5a repeated :id (runtime lexer) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S5b repeated :id (mapper text/order) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db | S8 20 concurrent queries resolve with the right rows | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S1a params: int, text, bool, null | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S1b omitted param (undefined) binds NULL | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S2a many: 3 rows | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S2b one: the row | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S2c maybeOne: zero rows gives null | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S2d maybeOne: two rows throw reylo's error | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S2e with(client)(params) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S3 write without RETURNING (none), then a separate read | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S3n write without parameters (none), then a separate read | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S4 INSERT ... RETURNING | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S5a repeated :id (runtime lexer) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S5b repeated :id (mapper text/order) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.task(t) | S8 20 concurrent queries resolve with the right rows | pass | warning: DeprecationWarning: Calling client.query() when the client is already executing a query is deprecated and will be removed in pg@9.0. Use async/await or an external async flow control mechanism instead. |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S1a params: int, text, bool, null | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S1b omitted param (undefined) binds NULL | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S2a many: 3 rows | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S2b one: the row | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S2c maybeOne: zero rows gives null | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S2d maybeOne: two rows throw reylo's error | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S2e with(client)(params) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S3 write without RETURNING (none), then a separate read | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S3n write without parameters (none), then a separate read | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S4 INSERT ... RETURNING | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S5a repeated :id (runtime lexer) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S5b repeated :id (mapper text/order) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S7a tx: write then commit is visible | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S7b tx: write then rollback is not visible | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S7d tx: a failing statement rolls the transaction back | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | db.tx(t) | S8 20 concurrent queries resolve with the right rows | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: DatabaseError: current transaction is aborted, commands ignored until end of transaction block [code=25P02 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S1a params: int, text, bool, null | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S1b omitted param (undefined) binds NULL | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S2a many: 3 rows | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S2b one: the row | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S2c maybeOne: zero rows gives null | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S2d maybeOne: two rows throw reylo's error | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S2e with(client)(params) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S3 write without RETURNING (none), then a separate read | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S3n write without parameters (none), then a separate read | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S4 INSERT ... RETURNING | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S5a repeated :id (runtime lexer) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S5b repeated :id (mapper text/order) | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: DatabaseError: current transaction is aborted, commands ignored until end of transaction block [code=25P02 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: DatabaseError: current transaction is aborted, commands ignored until end of transaction block [code=25P02 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: DatabaseError: current transaction is aborted, commands ignored until end of transaction block [code=25P02 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: DatabaseError: current transaction is aborted, commands ignored until end of transaction block [code=25P02 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S7a tx: write then commit is visible | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S7b tx: write then rollback is not visible | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S7d tx: a failing statement rolls the transaction back | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| pg-promise@12.7.1 | node 26.11.1 | Postgres 14 (local) | t.tx(t2) (savepoint) | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S1a params: int, text, bool, null | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S1d array params: = any(:ids) over an int and a text array | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S2a many: 3 rows | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S2b one: the row | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S2e with(client)(params) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S4 INSERT ... RETURNING | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=ERR_POSTGRES_SYNTAX_ERROR errno=42601 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S11b round trip: an instant past 9999 and one before 1 go back in | SKIP | Bun.sql 1.4.3 decodes a timestamptz before 1 AD as an Invalid Date, which Reylo refuses (44ac8d4), so the instant never comes back to pass in |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: COMMIT was rolled back by the server because the transaction was already aborted [code=ERR_POSTGRES_COMMIT_ROLLED_BACK] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S1a params: int, text, bool, null | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S2a many: 3 rows | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S2b one: the row | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S2e with(client)(params) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S4 INSERT ... RETURNING | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=ERR_POSTGRES_SYNTAX_ERROR errno=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: COMMIT was rolled back by the server because the transaction was already aborted [code=ERR_POSTGRES_COMMIT_ROLLED_BACK] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: COMMIT was rolled back by the server because the transaction was already aborted [code=ERR_POSTGRES_COMMIT_ROLLED_BACK] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: COMMIT was rolled back by the server because the transaction was already aborted [code=ERR_POSTGRES_COMMIT_ROLLED_BACK] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: COMMIT was rolled back by the server because the transaction was already aborted [code=ERR_POSTGRES_COMMIT_ROLLED_BACK] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S7a tx: write then commit is visible | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S7d tx: a failing statement rolls the transaction back | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | SKIP | Bun.sql 1.4.3 decodes a timestamptz before 1 AD as an Invalid Date, which Reylo refuses (44ac8d4), so the instant never comes back to pass in |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.begin(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: current transaction is aborted, commands ignored until end of transaction block [code=ERR_POSTGRES_SERVER_ERROR errno=25P02 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S1a params: int, text, bool, null | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S2a many: 3 rows | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S2b one: the row | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S2e with(client)(params) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S4 INSERT ... RETURNING | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=ERR_POSTGRES_SYNTAX_ERROR errno=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: current transaction is aborted, commands ignored until end of transaction block [code=ERR_POSTGRES_SERVER_ERROR errno=25P02 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: current transaction is aborted, commands ignored until end of transaction block [code=ERR_POSTGRES_SERVER_ERROR errno=25P02 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: current transaction is aborted, commands ignored until end of transaction block [code=ERR_POSTGRES_SERVER_ERROR errno=25P02 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: current transaction is aborted, commands ignored until end of transaction block [code=ERR_POSTGRES_SERVER_ERROR errno=25P02 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S7a tx: write then commit is visible | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S7d tx: a failing statement rolls the transaction back | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S11b round trip: an instant past 9999 and one before 1 go back in | SKIP | Bun.sql 1.4.3 decodes a timestamptz before 1 AD as an Invalid Date, which Reylo refuses (44ac8d4), so the instant never comes back to pass in |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | tx.savepoint(sp) | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S1a params: int, text, bool, null | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S1d array params: = any(:ids) over an int and a text array | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S2a many: 3 rows | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S2b one: the row | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S2e with(client)(params) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S4 INSERT ... RETURNING | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=ERR_POSTGRES_SYNTAX_ERROR errno=42601 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S7a tx: write then commit is visible | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S7d tx: a failing statement rolls the transaction back | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR] |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S11b round trip: an instant past 9999 and one before 1 go back in | SKIP | Bun.sql 1.4.3 decodes a timestamptz before 1 AD as an Invalid Date, which Reylo refuses (44ac8d4), so the instant never comes back to pass in |
| Bun.sql (postgres)@1.4.3 | bun 1.4.3 | Postgres 14 (local) | sql.reserve() | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Pool | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise Connection | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7a tx: write then commit is visible | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7b tx: write then rollback is not visible | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7d tx: a failing statement rolls the transaction back | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Pool | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise Connection | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7a tx: write then commit is visible | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7b tx: write then rollback is not visible | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7d tx: a failing statement rolls the transaction back | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.24.5 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7a tx: write then commit is visible | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7b tx: write then rollback is not visible | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7d tx: a failing statement rolls the transaction back | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7a tx: write then commit is visible | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7b tx: write then rollback is not visible | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7d tx: a failing statement rolls the transaction back | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2b one: the row | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@3.15.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Pool | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise Connection | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection (getConnection()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7a tx: write then commit is visible | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7b tx: write then rollback is not visible | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7d tx: a failing statement rolls the transaction back | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | promise PoolConnection in beginTransaction() | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Pool (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback Connection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.4 | callback PoolConnection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Pool | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise Connection | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection (getConnection()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7a tx: write then commit is visible | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7b tx: write then rollback is not visible | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7d tx: a failing statement rolls the transaction back | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | promise PoolConnection in beginTransaction() | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Pool (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback Connection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called promise() then execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1a params: int, text, bool, null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1b omitted param (undefined) binds NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2a many: 3 rows | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2b one: the row | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2c maybeOne: zero rows gives null | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S2e with(client)(params) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S3n write without parameters (none), then a separate read | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S5a repeated :id (runtime lexer) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S5b repeated :id (mapper text/order) | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mysql2@2.3.3 | node 26.11.1 | MySQL 8.0 | callback PoolConnection (no .promise()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S1a params: int, text, bool, null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S2a many: 3 rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S2b one: the row | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S2e with(client)(params) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S6a syntax error rejects with the driver's error | pass | we: (conn:61, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S6b unique violation rejects with the driver's error | pass | we: (conn:66, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Pool | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S1a params: int, text, bool, null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S2a many: 3 rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S2b one: the row | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S2e with(client)(params) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S6a syntax error rejects with the driver's error | pass | we: (conn:76, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S6b unique violation rejects with the driver's error | pass | we: (conn:76, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | Connection | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1a params: int, text, bool, null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2a many: 3 rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2b one: the row | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2e with(client)(params) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6a syntax error rejects with the driver's error | pass | we: (conn:71, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6b unique violation rejects with the driver's error | pass | we: (conn:73, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1a params: int, text, bool, null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2a many: 3 rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2b one: the row | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2e with(client)(params) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6a syntax error rejects with the driver's error | pass | we: (conn:71, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6b unique violation rejects with the driver's error | pass | we: (conn:73, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7a tx: write then commit is visible | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7b tx: write then rollback is not visible | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7d tx: a failing statement rolls the transaction back | pass | we: (conn:72, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1a params: int, text, bool, null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2a many: 3 rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2b one: the row | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2e with(client)(params) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6a syntax error rejects with the driver's error | pass | we: (conn:77, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6b unique violation rejects with the driver's error | pass | we: (conn:77, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1a params: int, text, bool, null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2a many: 3 rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2b one: the row | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2e with(client)(params) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6a syntax error rejects with the driver's error | pass | we: (conn:65, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6b unique violation rejects with the driver's error | pass | we: (conn:67, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1a params: int, text, bool, null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2a many: 3 rows | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2b one: the row | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2e with(client)(params) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6a syntax error rejects with the driver's error | pass | we: (conn:67, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6b unique violation rejects with the driver's error | pass | we: (conn:62, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7a tx: write then commit is visible | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7b tx: write then rollback is not visible | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7d tx: a failing statement rolls the transaction back | pass | we: (conn:67, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.5.4 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S1a params: int, text, bool, null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S2a many: 3 rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S2b one: the row | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S2e with(client)(params) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S6a syntax error rejects with the driver's error | pass | SqlError: (conn:89, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S6b unique violation rejects with the driver's error | pass | SqlError: (conn:86, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Pool | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S1a params: int, text, bool, null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S2a many: 3 rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S2b one: the row | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S2e with(client)(params) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S6a syntax error rejects with the driver's error | pass | SqlError: (conn:93, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S6b unique violation rejects with the driver's error | pass | SqlError: (conn:93, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | Connection | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1a params: int, text, bool, null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2a many: 3 rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2b one: the row | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2e with(client)(params) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6a syntax error rejects with the driver's error | pass | SqlError: (conn:91, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6b unique violation rejects with the driver's error | pass | SqlError: (conn:88, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1a params: int, text, bool, null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2a many: 3 rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2b one: the row | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2e with(client)(params) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6a syntax error rejects with the driver's error | pass | SqlError: (conn:91, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6b unique violation rejects with the driver's error | pass | SqlError: (conn:88, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7a tx: write then commit is visible | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7b tx: write then rollback is not visible | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7d tx: a failing statement rolls the transaction back | pass | SqlError: (conn:84, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1a params: int, text, bool, null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2a many: 3 rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2b one: the row | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2e with(client)(params) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6a syntax error rejects with the driver's error | pass | SqlError: (conn:94, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6b unique violation rejects with the driver's error | pass | SqlError: (conn:94, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1a params: int, text, bool, null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2a many: 3 rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2b one: the row | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2e with(client)(params) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6a syntax error rejects with the driver's error | pass | SqlError: (conn:83, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6b unique violation rejects with the driver's error | pass | SqlError: (conn:85, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1a params: int, text, bool, null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2a many: 3 rows | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2b one: the row | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2e with(client)(params) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6a syntax error rejects with the driver's error | pass | SqlError: (conn:78, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6b unique violation rejects with the driver's error | pass | SqlError: (conn:87, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7a tx: write then commit is visible | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7b tx: write then rollback is not visible | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7d tx: a failing statement rolls the transaction back | pass | SqlError: (conn:78, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@3.4.5 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S1a params: int, text, bool, null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S2a many: 3 rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S2b one: the row | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S2e with(client)(params) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S6a syntax error rejects with the driver's error | pass | SqlError: (conn=99, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S6b unique violation rejects with the driver's error | pass | SqlError: (conn=95, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Pool | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S1a params: int, text, bool, null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S2a many: 3 rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S2b one: the row | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S2e with(client)(params) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S6a syntax error rejects with the driver's error | pass | SqlError: (conn=106, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S6b unique violation rejects with the driver's error | pass | SqlError: (conn=106, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | Connection | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1a params: int, text, bool, null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2a many: 3 rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2b one: the row | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S2e with(client)(params) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6a syntax error rejects with the driver's error | pass | SqlError: (conn=101, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6b unique violation rejects with the driver's error | pass | SqlError: (conn=103, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection (getConnection()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1a params: int, text, bool, null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2a many: 3 rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2b one: the row | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S2e with(client)(params) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6a syntax error rejects with the driver's error | pass | SqlError: (conn=101, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6b unique violation rejects with the driver's error | pass | SqlError: (conn=103, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7a tx: write then commit is visible | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7b tx: write then rollback is not visible | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7d tx: a failing statement rolls the transaction back | pass | SqlError: (conn=109, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | PoolConnection in beginTransaction() | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1a params: int, text, bool, null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2a many: 3 rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2b one: the row | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S2e with(client)(params) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6a syntax error rejects with the driver's error | pass | SqlError: (conn=111, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6b unique violation rejects with the driver's error | pass | SqlError: (conn=111, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Connection (mariadb/callback) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1a params: int, text, bool, null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2a many: 3 rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2b one: the row | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S2e with(client)(params) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6a syntax error rejects with the driver's error | pass | SqlError: (conn=100, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6b unique violation rejects with the driver's error | pass | SqlError: (conn=102, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback Pool (mariadb/callback) | S8 20 concurrent queries resolve with the right rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called query(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1a params: int, text, bool, null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1b omitted param (undefined) binds NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2a many: 3 rows | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2b one: the row | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2c maybeOne: zero rows gives null | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2d maybeOne: two rows throw reylo's error | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S2e with(client)(params) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S3 write without RETURNING (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S3n write without parameters (none), then a separate read | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S5a repeated :id (runtime lexer) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S5b repeated :id (mapper text/order) | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6a syntax error rejects with the driver's error | pass | SqlError: (conn=104, no: 1064, SQLState: 42000) You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 sql: selec 1 - parameters:[] [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6b unique violation rejects with the driver's error | pass | SqlError: (conn=97, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7a tx: write then commit is visible | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7b tx: write then rollback is not visible | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7d tx: a failing statement rolls the transaction back | pass | SqlError: (conn=104, no: 1062, SQLState: 23000) Duplicate entry 'taken' for key 'uniq.PRIMARY' sql: insert into driver_matrix.uniq (k) values (?) - parameters:['taken'] [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| mariadb@2.5.6 | node 26.11.1 | MySQL 8.4 | callback PoolConnection in beginTransaction() (mariadb/callback) | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S1a params: int, text, bool, null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S2a many: 3 rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S2b one: the row | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S2e with(client)(params) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S6a syntax error rejects with the driver's error | pass | MySQLError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ERR_MYSQL_SYNTAX_ERROR errno=1064 sqlState=42000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S6b unique violation rejects with the driver's error | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S1a params: int, text, bool, null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S2a many: 3 rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S2b one: the row | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S2e with(client)(params) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S6a syntax error rejects with the driver's error | pass | MySQLError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ERR_MYSQL_SYNTAX_ERROR errno=1064 sqlState=42000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S6b unique violation rejects with the driver's error | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S7a tx: write then commit is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S7d tx: a failing statement rolls the transaction back | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.begin(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S1a params: int, text, bool, null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S2a many: 3 rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S2b one: the row | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S2e with(client)(params) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S6a syntax error rejects with the driver's error | pass | MySQLError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ERR_MYSQL_SYNTAX_ERROR errno=1064 sqlState=42000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S6b unique violation rejects with the driver's error | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S7a tx: write then commit is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S7d tx: a failing statement rolls the transaction back | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | tx.savepoint(sp) | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S1a params: int, text, bool, null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S2a many: 3 rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S2b one: the row | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S2e with(client)(params) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S6a syntax error rejects with the driver's error | pass | MySQLError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ERR_MYSQL_SYNTAX_ERROR errno=1064 sqlState=42000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S6b unique violation rejects with the driver's error | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S7a tx: write then commit is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S7d tx: a failing statement rolls the transaction back | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.4 | sql.reserve() | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S1a params: int, text, bool, null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S2a many: 3 rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S2b one: the row | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S2e with(client)(params) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S6a syntax error rejects with the driver's error | pass | MySQLError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ERR_MYSQL_SYNTAX_ERROR errno=1064 sqlState=42000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S6b unique violation rejects with the driver's error | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S1a params: int, text, bool, null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S2a many: 3 rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S2b one: the row | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S2e with(client)(params) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S6a syntax error rejects with the driver's error | pass | MySQLError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ERR_MYSQL_SYNTAX_ERROR errno=1064 sqlState=42000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S6b unique violation rejects with the driver's error | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S7a tx: write then commit is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S7d tx: a failing statement rolls the transaction back | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.begin(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S1a params: int, text, bool, null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S2a many: 3 rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S2b one: the row | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S2e with(client)(params) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S6a syntax error rejects with the driver's error | pass | MySQLError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ERR_MYSQL_SYNTAX_ERROR errno=1064 sqlState=42000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S6b unique violation rejects with the driver's error | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S7a tx: write then commit is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S7d tx: a failing statement rolls the transaction back | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | tx.savepoint(sp) | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S1a params: int, text, bool, null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S2a many: 3 rows | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S2b one: the row | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S2e with(client)(params) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S6a syntax error rejects with the driver's error | pass | MySQLError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ERR_MYSQL_SYNTAX_ERROR errno=1064 sqlState=42000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S6b unique violation rejects with the driver's error | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S7a tx: write then commit is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S7d tx: a failing statement rolls the transaction back | pass | MySQLError: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ERR_MYSQL_SERVER_ERROR errno=1062 sqlState=23000] |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (mysql)@1.4.3 | bun 1.4.3 | MySQL 8.0 | sql.reserve() | S8 20 concurrent queries resolve with the right rows | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S1a params: int, text, bool, null | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S1b omitted param (undefined) binds NULL | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S2a many: 3 rows | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S2b one: the row | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S2c maybeOne: zero rows gives null | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S2d maybeOne: two rows throw reylo's error | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S2e with(client)(params) | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S3 write without RETURNING (none), then a separate read | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S3n write without parameters (none), then a separate read | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S5a repeated :id (runtime lexer) | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S5b repeated :id (mapper text/order) | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S6a syntax error rejects with the driver's error | pass | DatabaseError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 (errno 1064) (sqlstate 42000) during query: selec 1 [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S6b unique violation rejects with the driver's error | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S8 20 concurrent queries resolve with the right rows | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S1a params: int, text, bool, null | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S1b omitted param (undefined) binds NULL | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S2a many: 3 rows | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S2b one: the row | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S2c maybeOne: zero rows gives null | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S2d maybeOne: two rows throw reylo's error | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S2e with(client)(params) | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S3 write without RETURNING (none), then a separate read | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S3n write without parameters (none), then a separate read | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S5a repeated :id (runtime lexer) | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S5b repeated :id (mapper text/order) | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S6a syntax error rejects with the driver's error | pass | DatabaseError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 (errno 1064) (sqlstate 42000) during query: selec 1 [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S6b unique violation rejects with the driver's error | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S6c try: a unique violation resolves to the database's error, read the same on every driver | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S8 20 concurrent queries resolve with the right rows | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S1a params: int, text, bool, null | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S1b omitted param (undefined) binds NULL | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S2a many: 3 rows | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S2b one: the row | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S2c maybeOne: zero rows gives null | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S2d maybeOne: two rows throw reylo's error | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S2e with(client)(params) | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S3 write without RETURNING (none), then a separate read | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S5a repeated :id (runtime lexer) | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S5b repeated :id (mapper text/order) | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S6a syntax error rejects with the driver's error | pass | DatabaseError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 (errno 1064) (sqlstate 42000) during query: selec 1 [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S6b unique violation rejects with the driver's error | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S7a tx: write then commit is visible | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S7b tx: write then rollback is not visible | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S7d tx: a failing statement rolls the transaction back | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@2.0.0 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S8 20 concurrent queries resolve with the right rows | UNSUPPORTED | reylo refused it: @planetscale/database 2.0 binds no parameters, so reylo cannot pass this statement's values to it; use its 1.x line, or mysql2 on PlanetScale's MySQL endpoint. (cause: Error: Query parameters are not supported: pass a complete SQL string and options to execute(query, options)) |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S1a params: int, text, bool, null | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S1b omitted param (undefined) binds NULL | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S2a many: 3 rows | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S2b one: the row | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S2c maybeOne: zero rows gives null | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S2d maybeOne: two rows throw reylo's error | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S2e with(client)(params) | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S3 write without RETURNING (none), then a separate read | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S3n write without parameters (none), then a separate read | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S5a repeated :id (runtime lexer) | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S5b repeated :id (mapper text/order) | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S6a syntax error rejects with the driver's error | pass | DatabaseError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 (errno 1064) (sqlstate 42000) during query: selec 1 [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S6b unique violation rejects with the driver's error | pass | DatabaseError: Duplicate entry 'taken' for key 'uniq.PRIMARY' (errno 1062) (sqlstate 23000) during query: insert into driver_matrix.uniq (k) values ('taken') [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | connect() Connection | S8 20 concurrent queries resolve with the right rows | FAIL | UnknownError: Expected JSON response from database API, got HTTP 403 Forbidden [body.code=UNKNOWN] |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S1a params: int, text, bool, null | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S1b omitted param (undefined) binds NULL | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S2a many: 3 rows | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S2b one: the row | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S2c maybeOne: zero rows gives null | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S2d maybeOne: two rows throw reylo's error | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S2e with(client)(params) | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S3 write without RETURNING (none), then a separate read | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S3n write without parameters (none), then a separate read | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S5a repeated :id (runtime lexer) | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S5b repeated :id (mapper text/order) | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S6a syntax error rejects with the driver's error | pass | DatabaseError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 (errno 1064) (sqlstate 42000) during query: selec 1 [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S6b unique violation rejects with the driver's error | pass | DatabaseError: Duplicate entry 'taken' for key 'uniq.PRIMARY' (errno 1062) (sqlstate 23000) during query: insert into driver_matrix.uniq (k) values ('taken') [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | Client | S8 20 concurrent queries resolve with the right rows | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S2a many: 3 rows | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S2b one: the row | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S2e with(client)(params) | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S6a syntax error rejects with the driver's error | pass | DatabaseError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 (errno 1064) (sqlstate 42000) during query: selec 1 [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S6b unique violation rejects with the driver's error | pass | DatabaseError: Duplicate entry 'taken' for key 'uniq.PRIMARY' (errno 1062) (sqlstate 23000) during query: insert into driver_matrix.uniq (k) values ('taken') [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | DatabaseError: Duplicate entry 'taken' for key 'uniq.PRIMARY' (errno 1062) (sqlstate 23000) during query: insert into driver_matrix.uniq (k) values ('taken') [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @planetscale/database@1.20.2 | node 26.11.1 | MySQL 8.4 via ps-http-sim | conn.transaction(tx) | S8 20 concurrent queries resolve with the right rows | FAIL | UnknownError: Expected JSON response from database API, got HTTP 403 Forbidden [body.code=UNKNOWN] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S2a many: 3 rows | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S2b one: the row | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S2e with(client)(params) | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(pool) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S2a many: 3 rows | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S2b one: the row | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S2e with(client)(params) | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass | warning: DeprecationWarning: Calling client.query() when the client is already executing a query is deprecated and will be removed in pg@9.0. Use async/await or an external async flow control mechanism instead. |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2a many: 3 rows | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2b one: the row | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2e with(client)(params) | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (node-postgres)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S2a many: 3 rows | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S2b one: the row | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S2e with(client)(params) | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | drizzle(sql) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S2a many: 3 rows | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S2b one: the row | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S2e with(client)(params) | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2a many: 3 rows | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2b one: the row | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2e with(client)(params) | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (postgres-js)@0.45.4 | node 26.11.1 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S2a many: 3 rows | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S2b one: the row | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S2e with(client)(params) | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | drizzle(pool) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S2a many: 3 rows | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S2b one: the row | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S2e with(client)(params) | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S2a many: 3 rows | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S2b one: the row | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S2e with(client)(params) | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S6a syntax error rejects with the driver's error | pass | Error: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 [code=ER_PARSE_ERROR errno=1064 sqlState=42000] |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S6b unique violation rejects with the driver's error | pass | Error: Duplicate entry 'taken' for key 'uniq.PRIMARY' [code=ER_DUP_ENTRY errno=1062 sqlState=23000] |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (mysql2)@0.45.4 | node 26.11.1 | MySQL 8.4 | tx.transaction(sp) (savepoint) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S2a many: 3 rows | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S2b one: the row | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S2e with(client)(params) | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S6a syntax error rejects with the driver's error | pass | SqliteError: near "selec": syntax error [code=SQLITE_ERROR] |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S6b unique violation rejects with the driver's error | pass | SqliteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (better-sqlite3)@0.45.4 | node 26.11.1 | SQLite file | drizzle(database) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S2a many: 3 rows | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S2b one: the row | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S2e with(client)(params) | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 extendedCode=SQLITE_ERROR cause.code=SQLITE_ERROR] |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT rawCode=1555 extendedCode=SQLITE_CONSTRAINT_PRIMARYKEY cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | drizzle(client) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S2a many: 3 rows | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S2b one: the row | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S2e with(client)(params) | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 extendedCode=SQLITE_ERROR cause.code=SQLITE_ERROR] |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT rawCode=1555 extendedCode=SQLITE_CONSTRAINT_PRIMARYKEY cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | LibsqlError: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT rawCode=1555 extendedCode=SQLITE_CONSTRAINT_PRIMARYKEY cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S2a many: 3 rows | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S2b one: the row | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S2e with(client)(params) | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 extendedCode=SQLITE_ERROR cause.code=SQLITE_ERROR] |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT rawCode=1555 extendedCode=SQLITE_CONSTRAINT_PRIMARYKEY cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (libsql)@0.45.4 | node 26.11.1 | SQLite file (file: URL) | tx.transaction(sp) (savepoint) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S2a many: 3 rows | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S2b one: the row | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S2e with(client)(params) | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(pool) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S2a many: 3 rows | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S2b one: the row | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S2e with(client)(params) | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S2a many: 3 rows | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S2b one: the row | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S2e with(client)(params) | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S6a syntax error rejects with the driver's error | pass | DatabaseError: syntax error at or near "selec" [code=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S6b unique violation rejects with the driver's error | pass | DatabaseError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (neon-serverless)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | tx.transaction(sp) (savepoint) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S2a many: 3 rows | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S2b one: the row | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S2e with(client)(params) | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S6a syntax error rejects with the driver's error | pass | NeonDbError: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S6b unique violation rejects with the driver's error | pass | NeonDbError: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (neon-http)@0.45.4 | node 26.11.1 | Postgres 14 via local Neon proxy | drizzle(neon()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S2a many: 3 rows | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S2b one: the row | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S2e with(client)(params) | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S6a syntax error rejects with the driver's error | pass | me: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S6b unique violation rejects with the driver's error | pass | me: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | drizzle(pglite) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2a many: 3 rows | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2b one: the row | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S2e with(client)(params) | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | me: syntax error at or near "selec" [code=42601 severity=ERROR] |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | me: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | me: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR] |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S2a many: 3 rows | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S2b one: the row | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S2e with(client)(params) | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S6a syntax error rejects with the driver's error | pass | me: syntax error at or near "selec" [code=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S6b unique violation rejects with the driver's error | pass | me: duplicate key value violates unique constraint "uniq_pkey" [code=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: DrizzleQueryError: Failed query: release savepoint sp1 params: [cause.code=25P02] |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S11b round trip: an instant past 9999 and one before 1 go back in | pass |  |
| drizzle-orm (pglite)@0.45.4 | node 26.11.1 | PGlite (in-process) | tx.transaction(sp) (savepoint) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S2a many: 3 rows | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S2b one: the row | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S2e with(client)(params) | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=ERR_POSTGRES_SYNTAX_ERROR errno=42601 severity=ERROR] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey" |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec" |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S11b round trip: an instant past 9999 and one before 1 go back in | SKIP | Bun.sql 1.4.3 decodes a timestamptz before 1 AD as an Invalid Date, which Reylo refuses (44ac8d4), so the instant never comes back to pass in |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | drizzle(sql) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: COMMIT was rolled back by the server because the transaction was already aborted [code=ERR_POSTGRES_COMMIT_ROLLED_BACK] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S2a many: 3 rows | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S2b one: the row | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S2e with(client)(params) | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=ERR_POSTGRES_SYNTAX_ERROR errno=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: COMMIT was rolled back by the server because the transaction was already aborted [code=ERR_POSTGRES_COMMIT_ROLLED_BACK] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: COMMIT was rolled back by the server because the transaction was already aborted [code=ERR_POSTGRES_COMMIT_ROLLED_BACK] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: COMMIT was rolled back by the server because the transaction was already aborted [code=ERR_POSTGRES_COMMIT_ROLLED_BACK] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: COMMIT was rolled back by the server because the transaction was already aborted [code=ERR_POSTGRES_COMMIT_ROLLED_BACK] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S11b round trip: an instant past 9999 and one before 1 go back in | SKIP | Bun.sql 1.4.3 decodes a timestamptz before 1 AD as an Invalid Date, which Reylo refuses (44ac8d4), so the instant never comes back to pass in |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: current transaction is aborted, commands ignored until end of transaction block [code=ERR_POSTGRES_SERVER_ERROR errno=25P02 severity=ERROR] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1d array params: = any(:ids) over an int and a text array | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1e JSON params: an array bound to :a::jsonb is stored as that JSON array | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S1f array params: bool[], timestamptz[] and bytea[] | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2a many: 3 rows | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2b one: the row | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S2e with(client)(params) | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6a syntax error rejects with the driver's error | pass | PostgresError: syntax error at or near "selec" [code=ERR_POSTGRES_SYNTAX_ERROR errno=42601 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: current transaction is aborted, commands ignored until end of transaction block [code=ERR_POSTGRES_SERVER_ERROR errno=25P02 severity=ERROR] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6b unique violation rejects with the driver's error | pass | PostgresError: duplicate key value violates unique constraint "uniq_pkey" [code=ERR_POSTGRES_SERVER_ERROR errno=23505 severity=ERROR]; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: current transaction is aborted, commands ignored until end of transaction block [code=ERR_POSTGRES_SERVER_ERROR errno=25P02 severity=ERROR] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=23505 condition=unique_violation schema=driver_matrix table=uniq constraint=uniq_pkey detail=Key (k)=(taken) already exists.: duplicate key value violates unique constraint "uniq_pkey"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: current transaction is aborted, commands ignored until end of transaction block [code=ERR_POSTGRES_SERVER_ERROR errno=25P02 severity=ERROR] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=42601 condition=syntax_error position=1: syntax error at or near "selec"; then the enclosing transaction failed, as the database aborts it after an error: PostgresError: current transaction is aborted, commands ignored until end of transaction block [code=ERR_POSTGRES_SERVER_ERROR errno=25P02 severity=ERROR] |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S11b round trip: an instant past 9999 and one before 1 go back in | SKIP | Bun.sql 1.4.3 decodes a timestamptz before 1 AD as an Invalid Date, which Reylo refuses (44ac8d4), so the instant never comes back to pass in |
| drizzle-orm (bun-sql)@0.45.4 | bun 1.4.3 | Postgres 14 (local) | tx.transaction(sp) (savepoint) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S2a many: 3 rows | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S2b one: the row | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S2e with(client)(params) | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S6a syntax error rejects with the driver's error | pass | DatabaseError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 (errno 1064) (sqlstate 42000) during query: selec 1 [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S6b unique violation rejects with the driver's error | pass | DatabaseError: Duplicate entry 'taken' for key 'uniq.PRIMARY' (errno 1062) (sqlstate 23000) during query: insert into driver_matrix.uniq (k) values ('taken') [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | drizzle(client) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S2a many: 3 rows | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S2b one: the row | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S2e with(client)(params) | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S6a syntax error rejects with the driver's error | pass | DatabaseError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 (errno 1064) (sqlstate 42000) during query: selec 1 [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S6b unique violation rejects with the driver's error | pass | DatabaseError: Duplicate entry 'taken' for key 'uniq.PRIMARY' (errno 1062) (sqlstate 23000) during query: insert into driver_matrix.uniq (k) values ('taken') [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S7a tx: write then commit is visible | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S7b tx: write then rollback is not visible | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S7d tx: a failing statement rolls the transaction back | pass | DatabaseError: Duplicate entry 'taken' for key 'uniq.PRIMARY' (errno 1062) (sqlstate 23000) during query: insert into driver_matrix.uniq (k) values ('taken') [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | db.transaction(tx) | S8 20 concurrent queries resolve with the right rows | FAIL | UnknownError: Expected JSON response from database API, got HTTP 403 Forbidden [body.code=UNKNOWN] |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S1g JSON params: a string, a number, a boolean, JSON text, an object and arrays are stored as their JSON, null as NULL | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S1h date and time params: a timestamptz takes its zoned text, timestamp and date their text, and a Date for any of them is refused before the driver is called | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S2a many: 3 rows | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S2b one: the row | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S2e with(client)(params) | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S6a syntax error rejects with the driver's error | pass | DatabaseError: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right syntax to use near 'selec 1' at line 1 (errno 1064) (sqlstate 42000) during query: selec 1 [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S6b unique violation rejects with the driver's error | pass | DatabaseError: Duplicate entry 'taken' for key 'uniq.PRIMARY' (errno 1062) (sqlstate 23000) during query: insert into driver_matrix.uniq (k) values ('taken') [body.code=UNKNOWN] (matched by message: the error carries no code field) |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=ER_DUP_ENTRY errno=1062 sqlState=23000 key=uniq.PRIMARY: Duplicate entry 'taken' for key 'uniq.PRIMARY' |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=ER_PARSE_ERROR errno=1064 sqlState=42000: You have an error in your SQL syntax; check the manual that corresponds to your MySQL server version for the right synta... |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (planetscale-serverless)@0.45.4 | node 26.11.1 | MySQL 8.4 via ps-http-sim | tx.transaction(sp) (savepoint) | S8 20 concurrent queries resolve with the right rows | FAIL | UnknownError: Expected JSON response from database API, got HTTP 403 Forbidden [body.code=UNKNOWN] |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S2a many: 3 rows | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S2b one: the row | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S2e with(client)(params) | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S6a syntax error rejects with the driver's error | pass | Error: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S6b unique violation rejects with the driver's error | pass | Error: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY errno=1555] |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (bun-sqlite)@0.45.4 | bun 1.4.3 | SQLite file | drizzle(database) | S8 20 concurrent queries resolve with the right rows | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S1a params: int, text, bool, null | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S1b omitted param (undefined) binds NULL | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S2a many: 3 rows | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S2b one: the row | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S2c maybeOne: zero rows gives null | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S2d maybeOne: two rows throw reylo's error | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S2e with(client)(params) | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S3 write without RETURNING (none), then a separate read | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S3n write without parameters (none), then a separate read | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S4 INSERT ... RETURNING | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S5a repeated :id (runtime lexer) | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S5b repeated :id (mapper text/order) | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S6a syntax error rejects with the driver's error | pass | Error: D1_ERROR: near "selec": syntax error at offset 0: SQLITE_ERROR (matched by message: the error carries no code field) |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S6b unique violation rejects with the driver's error | pass | Error: D1_ERROR: UNIQUE constraint failed: uniq.k: SQLITE_CONSTRAINT (extended: SQLITE_CONSTRAINT_PRIMARYKEY) (matched by message: the error carries no code field) |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error at offset 0 |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| drizzle-orm (d1)@0.45.4 | node 26.11.1 | D1 (Miniflare, local workerd) | drizzle(d1) | S8 20 concurrent queries resolve with the right rows | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S1a params: int, text, bool, null | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S1b omitted param (undefined) binds NULL | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S2a many: 3 rows | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S2b one: the row | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S2c maybeOne: zero rows gives null | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S2e with(client)(params) | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S3 write without RETURNING (none), then a separate read | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S3n write without parameters (none), then a separate read | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S4 INSERT ... RETURNING | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S5a repeated :id (runtime lexer) | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S5b repeated :id (mapper text/order) | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S6a syntax error rejects with the driver's error | pass | SqliteError: near "selec": syntax error [code=SQLITE_ERROR] |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S6b unique violation rejects with the driver's error | pass | SqliteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | db.transaction(fn) | S7a tx: write then commit is visible | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | db.transaction(fn) | S7b tx: write then rollback is not visible | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | db.transaction(fn) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | db.transaction(fn) | S7d tx: a failing statement rolls the transaction back | pass | SqliteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| better-sqlite3@13.0.3 | node 26.11.1 | SQLite file | db.transaction(fn) | S7e tx: a read inside transaction(fn) is usable synchronously | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S1a params: int, text, bool, null | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S1b omitted param (undefined) binds NULL | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S2a many: 3 rows | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S2b one: the row | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S2c maybeOne: zero rows gives null | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S2e with(client)(params) | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S3 write without RETURNING (none), then a separate read | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S3n write without parameters (none), then a separate read | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S4 INSERT ... RETURNING | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S5a repeated :id (runtime lexer) | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S5b repeated :id (mapper text/order) | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S6a syntax error rejects with the driver's error | pass | SqliteError: near "selec": syntax error [code=SQLITE_ERROR] |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S6b unique violation rejects with the driver's error | pass | SqliteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | db.transaction(fn) | S7a tx: write then commit is visible | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | db.transaction(fn) | S7b tx: write then rollback is not visible | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | db.transaction(fn) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | db.transaction(fn) | S7d tx: a failing statement rolls the transaction back | pass | SqliteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| better-sqlite3@12.11.1 | node 26.11.1 | SQLite file | db.transaction(fn) | S7e tx: a read inside transaction(fn) is usable synchronously | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S1a params: int, text, bool, null | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S1b omitted param (undefined) binds NULL | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S2a many: 3 rows | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S2b one: the row | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S2c maybeOne: zero rows gives null | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S2e with(client)(params) | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S3 write without RETURNING (none), then a separate read | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S3n write without parameters (none), then a separate read | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S4 INSERT ... RETURNING | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S5a repeated :id (runtime lexer) | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S5b repeated :id (mapper text/order) | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S6a syntax error rejects with the driver's error | pass | SqliteError: near "selec": syntax error [code=SQLITE_ERROR] |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S6b unique violation rejects with the driver's error | pass | SqliteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | db.transaction(fn) | S7a tx: write then commit is visible | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | db.transaction(fn) | S7b tx: write then rollback is not visible | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | db.transaction(fn) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | db.transaction(fn) | S7d tx: a failing statement rolls the transaction back | pass | SqliteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| better-sqlite3@11.10.0 | node 24.22.0 | SQLite file | db.transaction(fn) | S7e tx: a read inside transaction(fn) is usable synchronously | pass |  |
| better-sqlite3@9.6.0 | node 24.19.0 | SQLite file | (job) | setup | NOT RUN | no prebuilt binary for Node 24.19.0 (ABI 137), and native-old/ has no source build of it against that Node's headers (see README) |
| better-sqlite3@8.7.0 | node 24.19.0 | SQLite file | (job) | setup | NOT RUN | no prebuilt binary for Node 24.19.0 (ABI 137), and native-old/ has no source build of it against that Node's headers (see README) |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S1a params: int, text, bool, null | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S1b omitted param (undefined) binds NULL | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S2a many: 3 rows | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S2b one: the row | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S2c maybeOne: zero rows gives null | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S2d maybeOne: two rows throw reylo's error | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S2e with(client)(params) | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S3 write without RETURNING (none), then a separate read | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S3n write without parameters (none), then a separate read | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S4 INSERT ... RETURNING | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S5a repeated :id (runtime lexer) | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S5b repeated :id (mapper text/order) | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S6a syntax error rejects with the driver's error | pass | Error: near "selec": syntax error [code=ERR_SQLITE_ERROR errcode=1 errstr=SQL logic error] |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S6b unique violation rejects with the driver's error | pass | Error: UNIQUE constraint failed: uniq.k [code=ERR_SQLITE_ERROR errcode=1555 errstr=constraint failed] |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds 9007199254740993, an integer past ±(2^53 - 1), which a number cannot hold exactly, so Reylo refuses i |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync | S8 20 concurrent queries resolve with the right rows | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S1a params: int, text, bool, null | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S1b omitted param (undefined) binds NULL | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S2a many: 3 rows | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S2b one: the row | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S2c maybeOne: zero rows gives null | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S2d maybeOne: two rows throw reylo's error | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S2e with(client)(params) | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S3 write without RETURNING (none), then a separate read | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S3n write without parameters (none), then a separate read | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S4 INSERT ... RETURNING | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S5a repeated :id (runtime lexer) | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S5b repeated :id (mapper text/order) | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S6a syntax error rejects with the driver's error | pass | Error: near "selec": syntax error [code=ERR_SQLITE_ERROR errcode=1 errstr=SQL logic error] |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S6b unique violation rejects with the driver's error | pass | Error: UNIQUE constraint failed: uniq.k [code=ERR_SQLITE_ERROR errcode=1555 errstr=constraint failed] |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S7a tx: write then commit is visible | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S7b tx: write then rollback is not visible | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S7d tx: a failing statement rolls the transaction back | pass | Error: UNIQUE constraint failed: uniq.k [code=ERR_SQLITE_ERROR errcode=1555 errstr=constraint failed] |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds 9007199254740993, an integer past ±(2^53 - 1), which a number cannot hold exactly, so Reylo refuses i |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S8 20 concurrent queries resolve with the right rows | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore() | S9 unsupported client: run() rejects with reylo's TypeError | UNSUPPORTED | reylo refused it: reylo cannot run a sqlite statement on an object with get, all, iterate, run, clear. It takes a client with prepare(sql) as on better-sqlite3, node:sqlite, bun:sqlite, Turso's @tursodatabase/database and Cloudflare D1; exec(sql, ...values) as on a Durable Object's ctx.storage.sql; execute({ sql, args }) as on @libsql/client; all(sql, params) as on sqlite3 and sqlite; unsafe(text, values) as on Bun.sql; or a Drizzle database over one of them. A tag store is not a client: pass its database, store.db. |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S1a params: int, text, bool, null | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S1b omitted param (undefined) binds NULL | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S2a many: 3 rows | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S2b one: the row | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S2c maybeOne: zero rows gives null | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S2d maybeOne: two rows throw reylo's error | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S2e with(client)(params) | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S3 write without RETURNING (none), then a separate read | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S3n write without parameters (none), then a separate read | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S4 INSERT ... RETURNING | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S5a repeated :id (runtime lexer) | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S5b repeated :id (mapper text/order) | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S6a syntax error rejects with the driver's error | pass | Error: near "selec": syntax error [code=ERR_SQLITE_ERROR errcode=1 errstr=SQL logic error] |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S6b unique violation rejects with the driver's error | pass | Error: UNIQUE constraint failed: uniq.k [code=ERR_SQLITE_ERROR errcode=1555 errstr=constraint failed] |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds 9007199254740993, an integer past ±(2^53 - 1), which a number cannot hold exactly, so Reylo refuses i |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| node:sqlite@26.11.1 | node 26.11.1 | SQLite file | db.createTagStore().db | S8 20 concurrent queries resolve with the right rows | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S1a params: int, text, bool, null | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S1b omitted param (undefined) binds NULL | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S2a many: 3 rows | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S2b one: the row | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S2c maybeOne: zero rows gives null | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S2d maybeOne: two rows throw reylo's error | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S2e with(client)(params) | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S3 write without RETURNING (none), then a separate read | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S3n write without parameters (none), then a separate read | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S4 INSERT ... RETURNING | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S5a repeated :id (runtime lexer) | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S5b repeated :id (mapper text/order) | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S6a syntax error rejects with the driver's error | pass | Error: near "selec": syntax error [code=ERR_SQLITE_ERROR errcode=1 errstr=SQL logic error] |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S6b unique violation rejects with the driver's error | pass | Error: UNIQUE constraint failed: uniq.k [code=ERR_SQLITE_ERROR errcode=1555 errstr=constraint failed] |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds 9007199254740993, an integer past ±(2^53 - 1), which a number cannot hold exactly, so Reylo refuses i |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync | S8 20 concurrent queries resolve with the right rows | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S1a params: int, text, bool, null | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S1b omitted param (undefined) binds NULL | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S2a many: 3 rows | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S2b one: the row | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S2c maybeOne: zero rows gives null | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S2d maybeOne: two rows throw reylo's error | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S2e with(client)(params) | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S3 write without RETURNING (none), then a separate read | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S3n write without parameters (none), then a separate read | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S4 INSERT ... RETURNING | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S5a repeated :id (runtime lexer) | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S5b repeated :id (mapper text/order) | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S6a syntax error rejects with the driver's error | pass | Error: near "selec": syntax error [code=ERR_SQLITE_ERROR errcode=1 errstr=SQL logic error] |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S6b unique violation rejects with the driver's error | pass | Error: UNIQUE constraint failed: uniq.k [code=ERR_SQLITE_ERROR errcode=1555 errstr=constraint failed] |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S7a tx: write then commit is visible | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S7b tx: write then rollback is not visible | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S7d tx: a failing statement rolls the transaction back | pass | Error: UNIQUE constraint failed: uniq.k [code=ERR_SQLITE_ERROR errcode=1555 errstr=constraint failed] |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds 9007199254740993, an integer past ±(2^53 - 1), which a number cannot hold exactly, so Reylo refuses i |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | DatabaseSync in BEGIN/COMMIT (exec) | S8 20 concurrent queries resolve with the right rows | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore() | S9 unsupported client: run() rejects with reylo's TypeError | UNSUPPORTED | reylo refused it: reylo cannot run a sqlite statement on an object with get, all, iterate, run, clear. It takes a client with prepare(sql) as on better-sqlite3, node:sqlite, bun:sqlite, Turso's @tursodatabase/database and Cloudflare D1; exec(sql, ...values) as on a Durable Object's ctx.storage.sql; execute({ sql, args }) as on @libsql/client; all(sql, params) as on sqlite3 and sqlite; unsafe(text, values) as on Bun.sql; or a Drizzle database over one of them. A tag store is not a client: pass its database, store.db. |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S1a params: int, text, bool, null | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S1b omitted param (undefined) binds NULL | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S2a many: 3 rows | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S2b one: the row | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S2c maybeOne: zero rows gives null | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S2d maybeOne: two rows throw reylo's error | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S2e with(client)(params) | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S3 write without RETURNING (none), then a separate read | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S3n write without parameters (none), then a separate read | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S4 INSERT ... RETURNING | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S5a repeated :id (runtime lexer) | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S5b repeated :id (mapper text/order) | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S6a syntax error rejects with the driver's error | pass | Error: near "selec": syntax error [code=ERR_SQLITE_ERROR errcode=1 errstr=SQL logic error] |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S6b unique violation rejects with the driver's error | pass | Error: UNIQUE constraint failed: uniq.k [code=ERR_SQLITE_ERROR errcode=1555 errstr=constraint failed] |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds 9007199254740993, an integer past ±(2^53 - 1), which a number cannot hold exactly, so Reylo refuses i |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| node:sqlite@24.22.0 | node 24.22.0 | SQLite file | db.createTagStore().db | S8 20 concurrent queries resolve with the right rows | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S1a params: int, text, bool, null | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S1b omitted param (undefined) binds NULL | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S2a many: 3 rows | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S2b one: the row | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S2c maybeOne: zero rows gives null | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S2e with(client)(params) | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S3 write without RETURNING (none), then a separate read | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S3n write without parameters (none), then a separate read | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S4 INSERT ... RETURNING | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S5a repeated :id (runtime lexer) | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S5b repeated :id (mapper text/order) | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S6a syntax error rejects with the driver's error | pass | Error: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S6b unique violation rejects with the driver's error | pass | Error: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY errno=1555] |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | db.transaction(fn) | S7a tx: write then commit is visible | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | db.transaction(fn) | S7b tx: write then rollback is not visible | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | db.transaction(fn) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | db.transaction(fn) | S7d tx: a failing statement rolls the transaction back | pass | Error: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY errno=1555] |
| bun:sqlite@1.4.3 | bun 1.4.3 | SQLite file | db.transaction(fn) | S7e tx: a read inside transaction(fn) is usable synchronously | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S1a params: int, text, bool, null | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S2a many: 3 rows | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S2b one: the row | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S2e with(client)(params) | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S4 INSERT ... RETURNING | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S6a syntax error rejects with the driver's error | pass | SQLiteError: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S6b unique violation rejects with the driver's error | pass | SQLiteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY errno=1555] |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S1a params: int, text, bool, null | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S2a many: 3 rows | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S2b one: the row | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S2e with(client)(params) | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S4 INSERT ... RETURNING | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S6a syntax error rejects with the driver's error | pass | SQLiteError: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S6b unique violation rejects with the driver's error | pass | SQLiteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY errno=1555] |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S7a tx: write then commit is visible | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S7d tx: a failing statement rolls the transaction back | pass | SQLiteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY errno=1555] |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | sql.begin(tx) | S8 20 concurrent queries resolve with the right rows | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called unsafe(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S1a params: int, text, bool, null | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S1b omitted param (undefined) binds NULL | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S2a many: 3 rows | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S2b one: the row | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S2c maybeOne: zero rows gives null | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S2d maybeOne: two rows throw reylo's error | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S2e with(client)(params) | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S3 write without RETURNING (none), then a separate read | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S3n write without parameters (none), then a separate read | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S4 INSERT ... RETURNING | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S5a repeated :id (runtime lexer) | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S5b repeated :id (mapper text/order) | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S6a syntax error rejects with the driver's error | pass | SQLiteError: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S6b unique violation rejects with the driver's error | pass | SQLiteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY errno=1555] |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S7a tx: write then commit is visible | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S7b tx: write then rollback is not visible | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S7d tx: a failing statement rolls the transaction back | pass | SQLiteError: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY errno=1555] |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| Bun.sql (sqlite)@1.4.3 | bun 1.4.3 | SQLite file | tx.savepoint(sp) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S1a params: int, text, bool, null | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S1b omitted param (undefined) binds NULL | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S2a many: 3 rows | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S2b one: the row | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S2c maybeOne: zero rows gives null | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S2d maybeOne: two rows throw reylo's error | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S2e with(client)(params) | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S3 write without RETURNING (none), then a separate read | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S3n write without parameters (none), then a separate read | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S4 INSERT ... RETURNING | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S5a repeated :id (runtime lexer) | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S5b repeated :id (mapper text/order) | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 extendedCode=SQLITE_ERROR cause.code=SQLITE_ERROR] |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT rawCode=1555 extendedCode=SQLITE_CONSTRAINT_PRIMARYKEY cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | Client | S8 20 concurrent queries resolve with the right rows | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1a params: int, text, bool, null | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1b omitted param (undefined) binds NULL | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2a many: 3 rows | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2b one: the row | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2c maybeOne: zero rows gives null | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2d maybeOne: two rows throw reylo's error | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2e with(client)(params) | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S3 write without RETURNING (none), then a separate read | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S3n write without parameters (none), then a separate read | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S4 INSERT ... RETURNING | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S5a repeated :id (runtime lexer) | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S5b repeated :id (mapper text/order) | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 extendedCode=SQLITE_ERROR cause.code=SQLITE_ERROR] |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT rawCode=1555 extendedCode=SQLITE_CONSTRAINT_PRIMARYKEY cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7a tx: write then commit is visible | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7b tx: write then rollback is not visible | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7d tx: a failing statement rolls the transaction back | pass | LibsqlError: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT rawCode=1555 extendedCode=SQLITE_CONSTRAINT_PRIMARYKEY cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @libsql/client@0.17.4 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S8 20 concurrent queries resolve with the right rows | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S1a params: int, text, bool, null | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S1b omitted param (undefined) binds NULL | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S2a many: 3 rows | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S2b one: the row | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S2c maybeOne: zero rows gives null | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S2d maybeOne: two rows throw reylo's error | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S2e with(client)(params) | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S3 write without RETURNING (none), then a separate read | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S3n write without parameters (none), then a separate read | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S4 INSERT ... RETURNING | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S5a repeated :id (runtime lexer) | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S5b repeated :id (mapper text/order) | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 extendedCode=SQLITE_ERROR cause.code=SQLITE_ERROR] |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT rawCode=1555 extendedCode=SQLITE_CONSTRAINT_PRIMARYKEY cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | Client | S8 20 concurrent queries resolve with the right rows | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1a params: int, text, bool, null | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1b omitted param (undefined) binds NULL | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2a many: 3 rows | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2b one: the row | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2c maybeOne: zero rows gives null | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2d maybeOne: two rows throw reylo's error | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2e with(client)(params) | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S3 write without RETURNING (none), then a separate read | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S3n write without parameters (none), then a separate read | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S4 INSERT ... RETURNING | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S5a repeated :id (runtime lexer) | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S5b repeated :id (mapper text/order) | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 extendedCode=SQLITE_ERROR cause.code=SQLITE_ERROR] |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT rawCode=1555 extendedCode=SQLITE_CONSTRAINT_PRIMARYKEY cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7a tx: write then commit is visible | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7b tx: write then rollback is not visible | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7d tx: a failing statement rolls the transaction back | pass | LibsqlError: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT rawCode=1555 extendedCode=SQLITE_CONSTRAINT_PRIMARYKEY cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @libsql/client@0.18.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S8 20 concurrent queries resolve with the right rows | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S1a params: int, text, bool, null | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S1b omitted param (undefined) binds NULL | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S2a many: 3 rows | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S2b one: the row | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S2c maybeOne: zero rows gives null | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S2d maybeOne: two rows throw reylo's error | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S2e with(client)(params) | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S3 write without RETURNING (none), then a separate read | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S3n write without parameters (none), then a separate read | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S4 INSERT ... RETURNING | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S5a repeated :id (runtime lexer) | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S5b repeated :id (mapper text/order) | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 cause.code=SQLITE_ERROR] |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT_PRIMARYKEY: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY rawCode=1555 cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | Client | S8 20 concurrent queries resolve with the right rows | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1a params: int, text, bool, null | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1b omitted param (undefined) binds NULL | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2a many: 3 rows | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2b one: the row | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2c maybeOne: zero rows gives null | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2d maybeOne: two rows throw reylo's error | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2e with(client)(params) | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S3 write without RETURNING (none), then a separate read | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S3n write without parameters (none), then a separate read | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S4 INSERT ... RETURNING | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S5a repeated :id (runtime lexer) | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S5b repeated :id (mapper text/order) | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 cause.code=SQLITE_ERROR] |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT_PRIMARYKEY: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY rawCode=1555 cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7a tx: write then commit is visible | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7b tx: write then rollback is not visible | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7d tx: a failing statement rolls the transaction back | pass | LibsqlError: SQLITE_CONSTRAINT_PRIMARYKEY: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY rawCode=1555 cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @libsql/client@0.15.15 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S8 20 concurrent queries resolve with the right rows | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S1a params: int, text, bool, null | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S1b omitted param (undefined) binds NULL | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S2a many: 3 rows | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S2b one: the row | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S2c maybeOne: zero rows gives null | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S2d maybeOne: two rows throw reylo's error | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S2e with(client)(params) | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S3 write without RETURNING (none), then a separate read | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S3n write without parameters (none), then a separate read | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S4 INSERT ... RETURNING | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S5a repeated :id (runtime lexer) | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S5b repeated :id (mapper text/order) | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 cause.code=SQLITE_ERROR] |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT_PRIMARYKEY: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY rawCode=1555 cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | Client | S8 20 concurrent queries resolve with the right rows | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called execute(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1a params: int, text, bool, null | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1b omitted param (undefined) binds NULL | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2a many: 3 rows | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2b one: the row | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2c maybeOne: zero rows gives null | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2d maybeOne: two rows throw reylo's error | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S2e with(client)(params) | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S3 write without RETURNING (none), then a separate read | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S3n write without parameters (none), then a separate read | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S4 INSERT ... RETURNING | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S5a repeated :id (runtime lexer) | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S5b repeated :id (mapper text/order) | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6a syntax error rejects with the driver's error | pass | LibsqlError: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR rawCode=1 cause.code=SQLITE_ERROR] |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6b unique violation rejects with the driver's error | pass | LibsqlError: SQLITE_CONSTRAINT_PRIMARYKEY: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY rawCode=1555 cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7a tx: write then commit is visible | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7b tx: write then rollback is not visible | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7d tx: a failing statement rolls the transaction back | pass | LibsqlError: SQLITE_CONSTRAINT_PRIMARYKEY: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT_PRIMARYKEY rawCode=1555 cause.code=SQLITE_CONSTRAINT_PRIMARYKEY] |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | libSQL's own: Received integer which cannot be safely represented as a JavaScript number |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @libsql/client@0.14.0 | node 26.11.1 | SQLite file (file: URL) | client.transaction("write") | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2a many: 3 rows | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2b one: the row | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2e with(client)(params) | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6b unique violation rejects with the driver's error | pass | Error: step failed: UNIQUE constraint failed: uniq.k [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=undefined errcode=undefined table=undefined columns=undefined: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2a many: 3 rows | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2b one: the row | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2e with(client)(params) | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6b unique violation rejects with the driver's error | pass | Error: step failed: UNIQUE constraint failed: uniq.k [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=undefined errcode=undefined table=undefined columns=undefined: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7a tx: write then commit is visible | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7b tx: write then rollback is not visible | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7d tx: a failing statement rolls the transaction back | pass | Error: step failed: UNIQUE constraint failed: uniq.k [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S2a many: 3 rows | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S2b one: the row | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S2e with(client)(params) | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S6b unique violation rejects with the driver's error | pass | Error: step failed: UNIQUE constraint failed: uniq.k [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=undefined errcode=undefined table=undefined columns=undefined: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S7a tx: write then commit is visible | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S7b tx: write then rollback is not visible | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S7d tx: a failing statement rolls the transaction back | pass | Error: step failed: UNIQUE constraint failed: uniq.k [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2a many: 3 rows | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2b one: the row | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2e with(client)(params) | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6b unique violation rejects with the driver's error | pass | Error: step failed: UNIQUE constraint failed: uniq.k [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=undefined errcode=undefined table=undefined columns=undefined: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7a tx: write then commit is visible | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7b tx: write then rollback is not visible | pass |  |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7c tx: savepoint rollback keeps the outer write | SKIP | the compat Database's db.transaction(fn) inside another fails ("cannot start a transaction within a transaction"), where better-sqlite3's runs as a savepoint |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7d tx: a failing statement rolls the transaction back | pass | Error: step failed: UNIQUE constraint failed: uniq.k [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.8.3 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7e tx: a read inside transaction(fn) is usable synchronously | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2a many: 3 rows | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2b one: the row | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2e with(client)(params) | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6b unique violation rejects with the driver's error | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2a many: 3 rows | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2b one: the row | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2e with(client)(params) | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6b unique violation rejects with the driver's error | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7a tx: write then commit is visible | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7b tx: write then rollback is not visible | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7d tx: a failing statement rolls the transaction back | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S2a many: 3 rows | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S2b one: the row | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S2e with(client)(params) | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S6b unique violation rejects with the driver's error | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S7a tx: write then commit is visible | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S7b tx: write then rollback is not visible | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S7d tx: a failing statement rolls the transaction back | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | db.transactionAsync(fn) Transaction | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2a many: 3 rows | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2b one: the row | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2e with(client)(params) | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6b unique violation rejects with the driver's error | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7a tx: write then commit is visible | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7b tx: write then rollback is not visible | pass |  |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7c tx: savepoint rollback keeps the outer write | SKIP | the compat Database's db.transaction(fn) inside another fails ("cannot start a transaction within a transaction"), where better-sqlite3's runs as a savepoint |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7d tx: a failing statement rolls the transaction back | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.7.2 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7e tx: a read inside transaction(fn) is usable synchronously | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2a many: 3 rows | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2b one: the row | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2e with(client)(params) | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6b unique violation rejects with the driver's error | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2a many: 3 rows | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2b one: the row | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2e with(client)(params) | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6b unique violation rejects with the driver's error | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field); then the enclosing transaction failed, as the database aborts it after an error: Error: step failed: Transaction error: cannot rollback - no transaction is active [code=GenericFailure] |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k; then the enclosing transaction failed, as the database aborts it after an error: Error: step failed: Transaction error: cannot rollback - no transaction is active [code=GenericFailure] |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7a tx: write then commit is visible | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7b tx: write then rollback is not visible | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7d tx: a failing statement rolls the transaction back | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2a many: 3 rows | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2b one: the row | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2e with(client)(params) | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: near "selec": syntax error [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6b unique violation rejects with the driver's error | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: near "selec": syntax error |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7a tx: write then commit is visible | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7b tx: write then rollback is not visible | pass |  |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7c tx: savepoint rollback keeps the outer write | SKIP | the compat Database's db.transaction(fn) inside another fails ("cannot start a transaction within a transaction"), where better-sqlite3's runs as a savepoint |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7d tx: a failing statement rolls the transaction back | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.6.1 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7e tx: a read inside transaction(fn) is usable synchronously | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2a many: 3 rows | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2b one: the row | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S2e with(client)(params) | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: unexpected token 'selec' at offset 0 [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6b unique violation rejects with the driver's error | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: unexpected token 'selec' at offset 0 |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | SKIP | Turso 0.4 answers json_group_array over no rows with NULL, where SQLite and Turso 0.6 and later answer [] |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database (connect()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2a many: 3 rows | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2b one: the row | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S2e with(client)(params) | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: unexpected token 'selec' at offset 0 [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6b unique violation rejects with the driver's error | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: unexpected token 'selec' at offset 0 |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7a tx: write then commit is visible | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7b tx: write then rollback is not visible | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7c tx: savepoint rollback keeps the outer write | SKIP | Turso 0.4 has no SAVEPOINT ("SAVEPOINT not supported yet") |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7d tx: a failing statement rolls the transaction back | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | SKIP | Turso 0.4 answers json_group_array over no rows with NULL, where SQLite and Turso 0.6 and later answer [] |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | Database in BEGIN/COMMIT (exec) | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called prepare(); run returned a promise and rejected on a failing statement; runSync returned the value and threw |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1a params: int, text, bool, null | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1b omitted param (undefined) binds NULL | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2a many: 3 rows | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2b one: the row | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2c maybeOne: zero rows gives null | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S2e with(client)(params) | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S3 write without RETURNING (none), then a separate read | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S3n write without parameters (none), then a separate read | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S4 INSERT ... RETURNING | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S5a repeated :id (runtime lexer) | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S5b repeated :id (mapper text/order) | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6a syntax error rejects with the driver's error | pass | SqliteError: prepare failed: unexpected token 'selec' at offset 0 [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6b unique violation rejects with the driver's error | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=undefined errcode=undefined: unexpected token 'selec' at offset 0 |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | SKIP | Turso 0.4 answers json_group_array over no rows with NULL, where SQLite and Turso 0.6 and later answer [] |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7a tx: write then commit is visible | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7b tx: write then rollback is not visible | pass |  |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7c tx: savepoint rollback keeps the outer write | SKIP | the compat Database's db.transaction(fn) inside another fails ("cannot start a transaction within a transaction"), where better-sqlite3's runs as a savepoint |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7d tx: a failing statement rolls the transaction back | pass | Error: step failed: Runtime error: UNIQUE constraint failed: uniq.k (19) [code=GenericFailure] (matched by message: the error carries no code field) |
| @tursodatabase/database/compat@0.4.4 | node 26.11.1 | SQLite file (Turso's engine) | compat db.transaction(fn) | S7e tx: a read inside transaction(fn) is usable synchronously | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called all(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S1a params: int, text, bool, null | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S1b omitted param (undefined) binds NULL | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S2a many: 3 rows | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S2b one: the row | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S2c maybeOne: zero rows gives null | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S2e with(client)(params) | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S3 write without RETURNING (none), then a separate read | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S3n write without parameters (none), then a separate read | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S4 INSERT ... RETURNING | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S5a repeated :id (runtime lexer) | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S5b repeated :id (mapper text/order) | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S6a syntax error rejects with the driver's error | pass | Error: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S6b unique violation rejects with the driver's error | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called all(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S1a params: int, text, bool, null | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S1b omitted param (undefined) binds NULL | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S2a many: 3 rows | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S2b one: the row | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S2c maybeOne: zero rows gives null | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S2d maybeOne: two rows throw reylo's error | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S2e with(client)(params) | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S3 write without RETURNING (none), then a separate read | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S3n write without parameters (none), then a separate read | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S4 INSERT ... RETURNING | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S5a repeated :id (runtime lexer) | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S5b repeated :id (mapper text/order) | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S6a syntax error rejects with the driver's error | pass | Error: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S6b unique violation rejects with the driver's error | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S7a tx: write then commit is visible | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S7b tx: write then rollback is not visible | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S7d tx: a failing statement rolls the transaction back | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| sqlite3@5.1.7 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S8 20 concurrent queries resolve with the right rows | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called all(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S1a params: int, text, bool, null | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S1b omitted param (undefined) binds NULL | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S2a many: 3 rows | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S2b one: the row | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S2c maybeOne: zero rows gives null | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S2e with(client)(params) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S3 write without RETURNING (none), then a separate read | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S3n write without parameters (none), then a separate read | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S4 INSERT ... RETURNING | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S5a repeated :id (runtime lexer) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S5b repeated :id (mapper text/order) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S6a syntax error rejects with the driver's error | pass | Error: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S6b unique violation rejects with the driver's error | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database (open()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called all(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S1a params: int, text, bool, null | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S1b omitted param (undefined) binds NULL | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S2a many: 3 rows | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S2b one: the row | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S2c maybeOne: zero rows gives null | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S2d maybeOne: two rows throw reylo's error | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S2e with(client)(params) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S3 write without RETURNING (none), then a separate read | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S3n write without parameters (none), then a separate read | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S4 INSERT ... RETURNING | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S5a repeated :id (runtime lexer) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S5b repeated :id (mapper text/order) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S6a syntax error rejects with the driver's error | pass | Error: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S6b unique violation rejects with the driver's error | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S7a tx: write then commit is visible | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S7b tx: write then rollback is not visible | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S7d tx: a failing statement rolls the transaction back | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 5.1.7) | Database in BEGIN/COMMIT (exec) | S8 20 concurrent queries resolve with the right rows | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called all(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S1a params: int, text, bool, null | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S1b omitted param (undefined) binds NULL | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S2a many: 3 rows | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S2b one: the row | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S2c maybeOne: zero rows gives null | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S2e with(client)(params) | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S3 write without RETURNING (none), then a separate read | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S3n write without parameters (none), then a separate read | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S4 INSERT ... RETURNING | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S5a repeated :id (runtime lexer) | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S5b repeated :id (mapper text/order) | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S6a syntax error rejects with the driver's error | pass | Error: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S6b unique violation rejects with the driver's error | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called all(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S1a params: int, text, bool, null | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S1b omitted param (undefined) binds NULL | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S2a many: 3 rows | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S2b one: the row | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S2c maybeOne: zero rows gives null | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S2d maybeOne: two rows throw reylo's error | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S2e with(client)(params) | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S3 write without RETURNING (none), then a separate read | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S3n write without parameters (none), then a separate read | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S4 INSERT ... RETURNING | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S5a repeated :id (runtime lexer) | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S5b repeated :id (mapper text/order) | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S6a syntax error rejects with the driver's error | pass | Error: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S6b unique violation rejects with the driver's error | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S7a tx: write then commit is visible | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S7b tx: write then rollback is not visible | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S7d tx: a failing statement rolls the transaction back | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| sqlite3@6.0.1 | node 26.11.1 | SQLite file | Database in BEGIN/COMMIT (serialize) | S8 20 concurrent queries resolve with the right rows | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called all(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S1a params: int, text, bool, null | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S1b omitted param (undefined) binds NULL | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S2a many: 3 rows | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S2b one: the row | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S2c maybeOne: zero rows gives null | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S2d maybeOne: two rows throw reylo's error | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S2e with(client)(params) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S3 write without RETURNING (none), then a separate read | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S3n write without parameters (none), then a separate read | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S4 INSERT ... RETURNING | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S5a repeated :id (runtime lexer) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S5b repeated :id (mapper text/order) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S6a syntax error rejects with the driver's error | pass | Error: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S6b unique violation rejects with the driver's error | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database (open()) | S8 20 concurrent queries resolve with the right rows | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called all(); run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S1a params: int, text, bool, null | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S1b omitted param (undefined) binds NULL | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S2a many: 3 rows | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S2b one: the row | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S2c maybeOne: zero rows gives null | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S2d maybeOne: two rows throw reylo's error | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S2e with(client)(params) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S3 write without RETURNING (none), then a separate read | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S3n write without parameters (none), then a separate read | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S4 INSERT ... RETURNING | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S5a repeated :id (runtime lexer) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S5b repeated :id (mapper text/order) | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S6a syntax error rejects with the driver's error | pass | Error: SQLITE_ERROR: near "selec": syntax error [code=SQLITE_ERROR errno=1] |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S6b unique violation rejects with the driver's error | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT errcode=19 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S7a tx: write then commit is visible | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S7b tx: write then rollback is not visible | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S7c tx: savepoint rollback keeps the outer write | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S7d tx: a failing statement rolls the transaction back | pass | Error: SQLITE_CONSTRAINT: UNIQUE constraint failed: uniq.k [code=SQLITE_CONSTRAINT errno=19] |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S7n tx: a write without parameters, then commit, is visible | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| sqlite@5.1.1 | node 26.11.1 | SQLite file (over sqlite3 6.0.1) | Database in BEGIN/COMMIT (exec) | S8 20 concurrent queries resolve with the right rows | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S1a params: int, text, bool, null | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S1b omitted param (undefined) binds NULL | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S2a many: 3 rows | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S2b one: the row | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S2c maybeOne: zero rows gives null | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S2d maybeOne: two rows throw reylo's error | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S2e with(client)(params) | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S3 write without RETURNING (none), then a separate read | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S3n write without parameters (none), then a separate read | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S4 INSERT ... RETURNING | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S5a repeated :id (runtime lexer) | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S5b repeated :id (mapper text/order) | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S6a syntax error rejects with the driver's error | pass | Error: D1_ERROR: near "selec": syntax error at offset 0: SQLITE_ERROR (matched by message: the error carries no code field) |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S6b unique violation rejects with the driver's error | pass | Error: D1_ERROR: UNIQUE constraint failed: uniq.k: SQLITE_CONSTRAINT (extended: SQLITE_CONSTRAINT_PRIMARYKEY) (matched by message: the error carries no code field) |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error at offset 0 |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | D1Database | S8 20 concurrent queries resolve with the right rows | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S0 run() returns a promise on every client; runSync() returns the value on a sync client and is refused elsewhere | pass | reylo called no method the harness could observe; run returned a promise and rejected on a failing statement; runSync refused the client before calling it |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S1a params: int, text, bool, null | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S1b omitted param (undefined) binds NULL | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S1c binary params: a plain Uint8Array and a view into a larger buffer bind as their bytes | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S2a many: 3 rows | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S2b one: the row | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S2c maybeOne: zero rows gives null | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S2d maybeOne: two rows throw reylo's error | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S2e with(client)(params) | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S3 write without RETURNING (none), then a separate read | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S3n write without parameters (none), then a separate read | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S4 INSERT ... RETURNING | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S5a repeated :id (runtime lexer) | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S5b repeated :id (mapper text/order) | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S6a syntax error rejects with the driver's error | pass | Error: D1_ERROR: near "selec": syntax error at offset 0: SQLITE_ERROR (matched by message: the error carries no code field) |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S6b unique violation rejects with the driver's error | pass | Error: D1_ERROR: UNIQUE constraint failed: uniq.k: SQLITE_CONSTRAINT (extended: SQLITE_CONSTRAINT_PRIMARYKEY) (matched by message: the error carries no code field) |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S6c try: a unique violation resolves to the database's error, read the same on every driver | pass | QueryError code=SQLITE_CONSTRAINT_PRIMARYKEY errcode=1555 table=uniq columns=k: UNIQUE constraint failed: uniq.k |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S6d try: rows resolve to the value; Reylo's own error rejects; a syntax error resolves to the database's error | pass | QueryError code=SQLITE_ERROR errcode=1: near "selec": syntax error at offset 0 |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S10 representation: every value is Reylo's one type for its SQL type, whatever the driver gives by itself | pass | Column "big" holds an integer past ±(2^53 - 1), which the driver already rounded to 9007199254740992: a number cannot ho |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S10j built JSON: SQLite's text of the JSON a statement builds is parsed into the object its type says, other JSON text stays a string | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S11 round trip: the row S10 returns goes back in as the parameters it came from, every column equal | pass |  |
| miniflare D1@4.20260730.0 | node 26.11.1 | D1 (Miniflare, local workerd) | db.withSession() | S8 20 concurrent queries resolve with the right rows | pass |  |
