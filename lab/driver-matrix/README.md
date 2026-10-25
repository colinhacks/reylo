# Driver matrix

A live test of the client dispatch in [src/execute.ts](../../src/execute.ts): every driver version and every client object a user can hold, run through `defineQuery(...).run(client, params)` against real servers. The matrix imports the runtime by relative path, so it tests whatever `src/` holds when it runs.

```sh
./run.sh                    # servers, type check, every job; writes results.json and RESULTS.md
./run.sh --only mysql2      # only the jobs whose id contains "mysql2" (the result files are not rewritten)
node typecheck/check.ts     # the type check alone; writes typecheck/results.json and typecheck/RESULTS.md
```

Each job runs in its own process: Node 26.7.0 by default, Node 24.19.0 for the second node:sqlite job, Bun 1.3.14 for Bun.sql and bun:sqlite. The jobs call those binaries directly rather than through nub, because nub adds polyfills (a global `WebSocket`, for one) that a user's plain Node does not have. The binaries can be changed with `DM_NODE26`, `DM_NODE24` and `DM_BUN`.

## Coverage

| Package | Versions | Target | Client shapes |
| --- | --- | --- | --- |
| pg | 8.23.0, 7.18.2 | Postgres 14 (local) | Pool, connected Client, PoolClient from `pool.connect()`, PoolClient inside BEGIN/COMMIT |
| postgres | 3.4.9 | Postgres 14 (local) | `sql`, `sql.begin(tx)`, `tx.savepoint(sp)`, `sql.reserve()` |
| @electric-sql/pglite | 0.5.8, 0.4.6, 0.3.16, 0.2.17 | in-process | PGlite, `db.transaction(tx)` |
| @neondatabase/serverless | 1.1.0, 0.10.4, 0.9.5 | Postgres 14 behind a local Neon proxy | Pool, Client, PoolClient, PoolClient inside BEGIN/COMMIT (all over WebSocket), `neon()` over HTTP, `neon(url, { fullResults: true })` |
| pg-promise | 12.7.1 | Postgres 14 (local) | `db`, `db.task(t)`, `db.tx(t)`, nested `t.tx(t2)` (a savepoint) |
| slonik | 49.12.0 | Postgres 14 (local), with Slonik's own type parsers | the pool, a connection from `pool.connect(c)`, `pool.transaction(tx)`, nested `tx.transaction(sp)` (a savepoint) |
| Bun.sql | Bun 1.3.14 | Postgres 14, MySQL 8.4, MySQL 8.0, SQLite file | `sql`, `sql.begin(tx)`, `tx.savepoint(sp)`, `sql.reserve()` (not on SQLite) |
| mysql2 | 3.24.5, 3.15.3, 2.3.3 | MySQL 8.4, MySQL 8.0 | promise Pool, Connection, PoolConnection, PoolConnection inside `beginTransaction()`; callback Pool, Connection and PoolConnection passed without `.promise()` |
| mariadb (a MySQL client) | 3.5.4, 3.4.5, 2.5.6 | MySQL 8.4 | Pool, Connection, PoolConnection, PoolConnection inside `beginTransaction()`; `mariadb/callback` Connection, Pool, and PoolConnection inside `beginTransaction()` |
| @planetscale/database | 2.0.0, 1.20.2 | MySQL 8.4 behind ps-http-sim | `connect()` Connection, Client, `conn.transaction(tx)` |
| better-sqlite3 | 13.0.3, 12.11.1; 11.10.0 on Node 24 from a source build (9.6.0, 8.7.0: see below) | SQLite file | Database, `db.transaction(fn)` |
| node:sqlite | Node 26.7.0, Node 24.19.0 | SQLite file | DatabaseSync, DatabaseSync inside BEGIN/COMMIT, `db.createTagStore()`, `db.createTagStore().db` |
| bun:sqlite | Bun 1.3.14 | SQLite file | Database, `db.transaction(fn)` |
| @libsql/client | 0.17.4, 0.18.0, 0.15.15, 0.14.0 | SQLite file (`file:` URL) | Client, `client.transaction("write")` |
| @tursodatabase/database | 0.8.3, 0.7.2, 0.6.1, 0.4.4 | SQLite file, opened by Turso's engine | Database from `connect()`, Database inside BEGIN/COMMIT (exec), the Transaction `db.transactionAsync(fn)` hands its callback (0.7 and later); the synchronous Database of `@tursodatabase/database/compat` and its `db.transaction(fn)` |
| sqlite3 | 5.1.7, 6.0.1 | SQLite file | Database, BEGIN/COMMIT inside `db.serialize()` |
| sqlite | 5.1.1 over sqlite3 5.1.7 and 6.0.1 | SQLite file | Database from `open()`, Database inside BEGIN/COMMIT |
| Cloudflare D1 | miniflare 4.20260730.0 | local workerd | D1Database, `db.withSession()` |
| drizzle-orm | 0.45.4, over pg, postgres.js, mysql2, better-sqlite3 and @libsql/client | Postgres 14 (local), MySQL 8.4, SQLite file | `drizzle(client)`, `db.transaction(tx)`, the savepoint `tx.transaction(sp)` inside it (not on better-sqlite3, whose transaction is synchronous) |

The packages are installed side by side under npm aliases (`"pg-7": "npm:pg@7.18.2"`) in [package.json](package.json); nub wrote the lockfile as pnpm-lock.yaml. The three old better-sqlite3 lines live in [native-old/](native-old/package.json), a project of their own, because their source build must not block the rest.

## Scenarios

Every scenario asserts its result. Each call also checks Reylo's return contract: `run` returns a promise on every client and never throws synchronously. S0 checks `runSync` as well: it returns the value itself and throws on a synchronous SQLite client (better-sqlite3, node:sqlite, bun:sqlite, Turso's compat Database), and on any other client it throws Reylo's TypeError before it calls the client.

| Id | Checks |
| --- | --- |
| S0 | The return contract of `run` and `runSync` on a select and on a syntax error, and which client method Reylo called |
| S1a | Integer, text, boolean and null parameters; the returned row (SQLite gets 1 for the boolean, since it has no boolean type) |
| S1b | An omitted (undefined) parameter binds NULL |
| S1c | A plain `Uint8Array` and a view into a larger buffer, neither a Buffer, bind as their bytes (`hex` of each) |
| S1d | Postgres only: `id = any(:ids) and name = any(:names)` over an int array and a text array whose last element holds a quote, a backslash, a space and braces |
| S1e | Postgres only: an array bound to `:a::jsonb` (`[1, {"k": "say \"hi\""}, null]`) and `[]` to `:e::jsonb` are stored as those JSON arrays |
| S1f | Postgres only: `bool[]`, `timestamptz[]` and `bytea[]` parameters |
| S1g | Postgres and MySQL: a string with a quote, a number, a boolean, JSON text, an object, an array and `[]` bound to JSON parameters (`:s::jsonb`, `json_extract(:s, '$')`) are stored as their JSON, and null as NULL; the stored text is parsed, since each database spaces it its own way |

S1d to S1g give each statement the `inputs` the loader writes from the analysis (each parameter's type), since Reylo converts an array by it and converts none for a query without it.
| S2a–S2e | Three rows for `many`; the row for `one`; null for `maybeOne` on zero rows; Reylo's "Expected at most one row, got 2" on two; `with(client)(params)` |
| S3 | An INSERT without RETURNING (cardinality `none`) resolves undefined; the row is read back through Reylo and counted through the driver |
| S3n | The same with the values written into the SQL, so Reylo passes no parameters |
| S4 | INSERT ... RETURNING (Postgres and SQLite) |
| S5a, S5b | One named parameter used twice, through the runtime lexer and through the mapper's `text` and `order` |
| S6a, S6b | A syntax error and a unique violation reject with the driver's own error and code (SQLSTATE 42601 and 23505, ER_PARSE_ERROR and ER_DUP_ENTRY, SQLITE_ERROR and SQLITE_CONSTRAINT), never Reylo's TypeError |
| S6c, S6d | `try` resolves to `{ value }` for the rows and rejects with Reylo's own row-count error; and it resolves to `{ error }` for the two errors above, read into its dialect's fields whichever driver ran them (SQLSTATE, condition, schema, table and constraint; MySQL's name, number, state and key; SQLite's most specific code the driver gives and its number, and the table and columns its message names), the cause the driver's own error. On a synchronous client `trySync` must give the same |
| S7a–S7d, S7n | On a transaction: a committed write is visible; a rolled-back write is not; a savepoint rollback keeps the outer write; a failing statement rolls the whole transaction back and its error reaches the caller; a write without parameters commits |
| S7e | On better-sqlite3 and bun:sqlite `db.transaction(fn)`, where S7a–S7d call `runSync`: a row read inside the function is usable synchronously |
| S8 | 20 queries in flight on the shape resolve with the right rows |
| S9 | An unsupported client: `run` rejects with Reylo's own TypeError |
| S10 | Reylo's one representation per SQL type (decided 2026-10-20, `src/values.ts`): the `typed` fixture's row, read through a statement with the `columns` a build step writes, has every value of its exact JavaScript type and value (an int8 or BIGINT, signed and unsigned, a BigInt, a decimal its text, a timestamp or DATETIME its text with `T`, a date its text, a timestamptz its ISO 8601 text in UTC, JSON parsed, binary a Uint8Array, an array of enums, of text with a NULL and a `"NULL"`, and of int4 parsed, a SET as `x,z`, a MySQL TINYINT(1) and BIT(1) and a SQLite truth value a boolean), compared with types, not normalized; and an int8 past 2^53 is a BigInt, or on SQLite an INTEGER past 2^53 is refused, with Reylo's error naming the column or libSQL's own RangeError at its default `intMode` |
| S10j | SQLite only: JSON a statement builds (`json_object` nesting `json_object` and a correlated `json_group_array`, an empty `json_group_array`, a `json_array` tuple), read through the `columns` the loader writes for it (`json`), is the object, the empty list and the tuple, compared with types; a column built and then concatenated (`json_object(…) \|\| ''`) and a TEXT column holding JSON stay strings |
| S11 | The round-trip rule (decided 2026-10-20): the `typed` row S10 reads goes back in as the parameters of a statement comparing every column with one (`big = :big`, …), each parameter of the type the engine gives it, and every comparison is true: a BigInt, a numeric's text, a timestamptz's UTC text, parsed JSON, bytes, a Postgres array with a NULL and a quoted `"NULL"`, an enum array, a MySQL TINYINT(1)'s and BIT(1)'s boolean, a SQLite truth value |
| S11b | Postgres only: the instants past 9999 and before 1, which come back as `+010000-01-01T00:00:00Z` and `-000001-01-01T00:00:00Z`, go back in (Postgres refuses both forms as input) |

Visibility is always checked with a raw count through the driver, never through Reylo. A raw count also runs before each job as a preflight, so a driver that cannot reach its server at all is reported once as "not run" instead of failing every scenario. [drivers/control.ts](drivers/control.ts) holds the failing controls: a client that binds every integer plus one and a plain `Uint8Array` as its JSON (as mariadb does); an `unsafe()` client that binds an array as its elements joined by commas (as Bun.sql does), running the statements without their `inputs`, so that Reylo converts no array; an `unsafe()` client that binds every array as its array literal, a JSON parameter's too, as Reylo did on Bun.sql at 0d82fea; a transaction whose rollback commits; a synchronous database declared asynchronous; and a `db.transaction(fn)` whose statements answer through a promise. 24 of their 87 rows fail, and each of the 63 that pass is a scenario that the breakage does not touch. Since `run` returns a promise on every client, a synchronous database declared asynchronous changes only what S0 expects of `runSync`, so 16 of the 63 are its rows.

## Servers

[setup.sh](setup.sh) starts the containers idempotently and seeds every fixture; [run.sh](run.sh) calls it first. The containers are left running.

| Server | Where | Used by |
| --- | --- | --- |
| Postgres 14 (shared, local) | `postgres://localhost:5432/postgres`, schema `driver_matrix` only | pg, postgres, pg-promise, Bun.sql |
| porg-dm-mysql84 (mysql:8.4) | 127.0.0.1:33406, root/porg | mysql2, mariadb, Bun.sql, ps-http-sim |
| porg-dm-mysql80 (mysql:8.0) | 127.0.0.1:33380, root/porg | mysql2, Bun.sql |
| porg-dm-neon-pg (postgres:14) | 127.0.0.1:55414, postgres/postgres | behind the Neon proxy |
| porg-dm-neon-proxy (ghcr.io/timowilhelm/local-neon-http-proxy:main) | 127.0.0.1:54444, HTTP `/sql` and WebSocket `/v1` | @neondatabase/serverless |
| porg-dm-ps-sim (ghcr.io/mattrobenolt/ps-http-sim:latest) | 127.0.0.1:38080 | @planetscale/database |

The Neon proxy creates a `neon_control_plane` schema in the database it fronts, so it has a Postgres container of its own instead of the shared server. The connection string names the host `db.localtest.me`, which the proxy uses as the endpoint name. That host does not resolve here, and it does not need to, because `fetchEndpoint` and `wsProxy` send every request to 127.0.0.1:54444.

## First run

The first run tested src/execute.ts 1b03ae48c322: 46 jobs and 2495 rows, with 2335 passing, 156 failing and 4 not run.

| Failure | Rows | Diagnosis |
| --- | --- | --- |
| mariadb 3.5.4, 3.4.5 and 2.5.6 through `mariadb/callback` (Connection) | 13 per job | Reylo saw `batch` and `query` and called `query(text, values)` without a callback. The callback API then returns a command object, not a promise, and `Array.isArray(result) ? result : []` turned it into `[]`. Reads came back empty, writes still ran, and errors were lost: S6a resolved `[]` and S6b resolved undefined |
| @planetscale/database 2.0.0, all three shapes | 46 | Version 2.0 removed parameter binding from `execute`: it throws "Query parameters are not supported: pass a complete SQL string and options to execute(query, options)" for any array second argument, including the empty array Reylo passed for a statement without parameters |
| node:sqlite `db.createTagStore()`, Node 26.7 and 24.19 | 15 per Node | Reylo saw `all` and took the store for a sqlite3 Database, calling `all(text, values, callback)`. Node threw its own TypeError, "First argument must be an array of strings (template literal)", instead of Reylo's unsupported-client error |
| @planetscale/database 1.20.2, S8 on the Connection and inside `conn.transaction` | 2 | Not Reylo. ps-http-sim answers HTTP 403 ("session already in use") to concurrent requests on one session. The driver alone does the same: [probes/planetscale-session.mjs](probes/planetscale-session.mjs) |
| pg 7.18.2 (not run) | 1 | Not Reylo. The preflight, a raw `pool.query`, ends in "Connection terminated due to connection timeout" on Node 26: pg 7 predates the stream changes of Node 14 ([node-postgres#2317](https://github.com/brianc/node-postgres/issues/2317)). No Node version runs both Reylo (type stripping needs Node 22.6 or later; Reylo's `engines` says 24) and pg 7 |
| better-sqlite3 11.10.0, 9.6.0 and 8.7.0 (not run) | 3 | Not Reylo. None of them has a prebuilt binary for Node 26 (ABI 147), so they build from source (see below) |

Everything else passed on the first run, including every transaction shape listed above.

## Re-run

The re-run tested src/execute.ts c46b8b73fe74 and src/index.ts 0dac6252558a, after the orchestrator's fixes for the three Reylo failures: 46 jobs, 179 client shapes and 2913 rows. The results are 2868 pass, 2 fail, 39 unsupported and 4 not run. A run over src/execute.ts 3292aea4ccac, which changes only the client types (see the type check below), gives the same rows. The run after it adds better-sqlite3 11.10.0 on Node 24 from a source build (below): 46 jobs and 2933 rows, with 2889 pass, 2 fail, 39 unsupported and 3 not run. The last run, over src/execute.ts 332f629f3f63, where `run` returns a promise on every client and `runSync` is new, gives the same counts. [RESULTS.md](RESULTS.md) has every row.

| Rows | Result | Diagnosis |
| --- | --- | --- |
| mariadb 3.5.4, 3.4.5 and 2.5.6, every shape | pass | Reylo now calls `query(text, values, callback)` and takes whichever answers: the callback on `mariadb/callback`, the returned promise on the promise API. S0 shows `query()`; S6a and S6b get SqlError 1064 and 1062 |
| @planetscale/database 2.0.0: S0, S2a, S3n, S6a, S7n on all three shapes | pass (13 rows) | A statement without parameters now reaches `execute(text)` with no second argument |
| @planetscale/database 2.0.0: every scenario with parameters | unsupported (37 rows) | Reylo refuses the statement with its TypeError "@planetscale/database 2.0 binds no parameters, ..." and the row checks that its `cause` is PlanetScale's error. By design: 2.0 takes no parameters, and Reylo does not escape values into SQL. Version 2.0 has 1,217 downloads a week against 457,670 for 1.x |
| node:sqlite `db.createTagStore()`, Node 26.7 and 24.19 | unsupported (2 rows, S9) | Reylo refuses the store with its own TypeError, which ends "A tag store is not a client: pass its database, store.db." |
| node:sqlite `db.createTagStore().db`, Node 26.7 and 24.19 | pass (32 rows) | The store's database is a DatabaseSync |
| @planetscale/database 1.20.2, S8 on the Connection and inside `conn.transaction` | fail (2 rows) | ps-http-sim's session lock, as in the first run. The raw probe fails the same way |
| pg 7.18.2; better-sqlite3 9.6.0, 8.7.0 | not run (3 rows) | As in the first run |
| better-sqlite3 11.10.0 on Node 24.19 | pass (21 rows) | From a source build against Node 24's headers; see below |

Two driver behaviours show up as details on passing rows:

- On a pg connection (a Client, a PoolClient, a pg-promise task or tx), S8's 20 concurrent queries work, but pg 8.23 prints "Calling client.query() when the client is already executing a query is deprecated and will be removed in pg@9.0". Reylo issues one query per `run`, so the concurrency is the caller's.
- A statement that fails inside an open Postgres transaction aborts it. On postgres.js `sql.begin` and `tx.savepoint`, pg-promise's nested `t.tx`, and Bun.sql's `tx.savepoint`, the enclosing commit then fails with the driver's error. The 12 rows where this happens record it and pass, because Reylo already returned the driver's error.

## Re-run (2026-10-11)

The whole matrix ran again against src/ at 1c8809d, on Node 26.11.1 and 24.21.0 (nub's, through `DM_NODE26` and `DM_NODE24`) and Bun 1.4.3, with the containers in colima 0.10.3 (Docker 29.5) and a throwaway Postgres 18 standing in for the shared Postgres 14 (`DM_PG_URL`): 2933 rows, 2889 pass, 2 fail, 39 unsupported and 3 not run, the counts recorded above, and 19 of 42 control rows failing as designed. [compare.ts](compare.ts) set every row of every job beside its row in [RESULTS.md](RESULTS.md), keyed by package, version, target, shape and scenario: all 2921 keys give the recorded result. RESULTS.md was left as it was; the error and detail texts differ only by connection ids and the drivers' wording (Bun 1.4 adds the aborted COMMIT to three rows' detail). better-sqlite3 11.10.0 ran from a source build (above); 9.6.0 and 8.7.0 do not compile against Node 24 and stay "not run".

One change was needed: Bun 1.4's MySQL adapter refuses to fetch the server's RSA key for `caching_sha2_password` over a connection without TLS ("The server requested RSA public key retrieval …", `ERR_MYSQL_PUBLIC_KEY_RETRIEVAL_NOT_ALLOWED`), which Bun 1.3 and mysql2 do unasked, so both Bun.sql MySQL jobs were "not run" until [drivers/bunsql.ts](drivers/bunsql.ts) passed `allowPublicKeyRetrieval: true`; then they give their 75 recorded passes each.

## Re-run (2026-10-12): binary, array and JSON parameters

S1c to S1f are new, after the coordinator measured drivers that corrupt or refuse a parameter of a type Reylo declares. Each ran in the same environment as the re-run of 2026-10-11 (Node 26.11.1 and 24.21.0, Bun 1.4.3, colima, a throwaway Postgres 18 through `DM_PG_URL`). S1c and S1d first ran against src/execute.ts at 9fdb724, S1e and S1f against 0d82fea, the first version of the fix:

| Rows | Result | Diagnosis |
| --- | --- | --- |
| mariadb 3.5.4, 3.4.5 and 2.5.6, S1c on every shape (9fdb724) | fail | mariadb writes any value that is not a Buffer as JSON: `hex(?)` of `Uint8Array.of(1, 2, 255)` was `7B2230223A312C2231223A322C2232223A3235357D`, the hex of `{"0":1,"1":2,"2":255}` |
| @neondatabase/serverless 0.9.5, S1c on `neon()` and `neon(url, { fullResults: true })` (9fdb724) | fail (2 rows) | Its HTTP path sends a Buffer as JSON, `{"type":"Buffer","data":[1,2,255]}`, which pg's own `prepareValue` makes of a `Uint8Array` first; 0.10 added `encodeBuffersAsBytea` |
| Bun.sql 1.4.3 on Postgres, S1d on all four shapes (9fdb724) | fail (4 rows, in a run of that job alone) | Bun.sql binds no JavaScript array: "insufficient data left in message" (08P01) |
| Bun.sql 1.4.3 on Postgres, S1e on all four shapes (0d82fea) | fail (4 rows) | 0d82fea wrote every array as an array literal for Bun.sql, and Bun.sql stores a string bound to `$1::jsonb` as a JSON string: `[1, 2, 3]` was stored as `"{\"1\",\"2\",\"3\"}"` |
| pg 8.23.0, pg-promise 12.7.1 and @neondatabase/serverless 1.1.0, 0.10.4 and 0.9.5, S1e (0d82fea) | fail (4, 4 and 2 per Neon version) | pg writes any array as an array literal, a JSON parameter's too: "invalid input syntax for type json", and `[]` is stored as `{}` |
| postgres 3.4.9, S1f on all four shapes (0d82fea) | fail (4 rows) | postgres.js types an array by its first element: "cannot cast type boolean to boolean[]" |
| @neondatabase/serverless 1.1.0, 0.10.4 and 0.9.5, S1f on the Pool and Client (0d82fea) | fail (2 per version) | The copy of pg inside Neon's Pool and Client has no case for a `Uint8Array` element of an array and throws "r.replace is not a function" while it writes the Bind message, which leaves the connection out of step: the scenarios after it on the same Client time out, and on the Pool the next statement read the previous one's values ("invalid input syntax for type integer: \"{\"true\",\"false\"}\"") |
| pg-promise 12.7.1, S1c (9fdb724) | pass | pg-promise formats a plain `Uint8Array` as JSON when it writes the values into the text, but Reylo hands it a `{ text, values }` object, which pg binds |

Reylo now gives mariadb a Buffer over the same memory and Neon's `neon()` before 1.0 a binary value as `\x` and its hex; and, by the parameter types the module carries as `inputs`, every Postgres client an array for a SQL array parameter as its array literal, written as pg writes one, and pg's family, PGlite and Neon an array for a JSON parameter as its JSON text, while postgres.js and Bun.sql get that one as it is (`src/execute.ts`). The run after the fix wrote [RESULTS.md](RESULTS.md): 46 jobs and 3230 rows, with 3183 pass, 2 fail (PlanetScale's simulator, as before), 42 unsupported (PlanetScale 2.0 refuses S1c as it refuses every statement with parameters) and 3 not run, and 24 of 87 control rows failing as designed. Every row that was in RESULTS.md before has the result it had; the control shapes are the only keys that changed.

## Re-run (2026-10-12): JSON and date parameters

S1g and S1h are new, after every driver was measured binding a JSON value and only a `timestamptz` parameter came to take a `Date`. They ran in the same environment (Node 26.11.1 and 24.21.0, Bun 1.4.3, colima, a throwaway Postgres 18 through `DM_PG_URL`, the process in America/Los_Angeles), first against src/execute.ts at 39737bf, where every row of both failed on every Postgres and MySQL job:

| Rows | Result at 39737bf | Diagnosis |
| --- | --- | --- |
| pg 8.23.0, pg-promise 12.7.1, PGlite 0.2 to 0.5 and Neon 0.9.5, 0.10.4 and 1.1.0, S1g | fail (every row) | pg passes a string through as raw JSON text, so `'say "hi"'` into `::jsonb` failed with "invalid input syntax for type json" |
| postgres 3.4.9, S1g | fail (4 rows) | postgres.js declares a boolean by its JavaScript type: "cannot cast type boolean to jsonb" |
| Bun.sql 1.4.3 on Postgres, S1g | fail (4 rows) | Bun.sql declares a number and a boolean by their JavaScript type: "cannot cast type integer to jsonb" |
| mysql2 2.3.3, 3.15.3 and 3.24.5 and Bun.sql 1.4.3 on MySQL, S1g | fail (every row) | A string is bound as text, which `json_extract` refuses: "Invalid JSON text in argument 1" |
| mariadb 2.5.6, 3.4.5 and 3.5.4, S1g | fail | mariadb expands an array into a list: a syntax error |
| @planetscale/database 1.20.2, S1g | fail (3 rows) | "Cannot format query: empty arrays are not valid SQL values" |
| Every Postgres and MySQL job, S1h | fail (every row) | Reylo passed a `Date` for a `date` parameter to the driver; on Bun.sql it failed with `time zone "gmt-0800" not recognized` |
| postgres 3.4.9, S1h | fail (4 rows) | postgres.js reads a string for a date or time parameter through `new Date(x)`: the zone name failed with "Invalid time value", and `2026-01-02 03:04:05.006` into `timestamp` was stored at 11:04:05.006 |

Reylo now binds a JSON parameter's value as its JSON text (to postgres.js and Bun.sql as an object whose `toJSON()` returns the value), refuses a `Date` for a `timestamp` or `date` parameter and a `timestamptz` string without its zone before the driver is called, and gives postgres.js a date or time string declared `unknown` (`sql.typed(value, 705)`). The run after the fix wrote [RESULTS.md](RESULTS.md): 46 jobs and 3510 rows, with 3457 pass, 2 fail (PlanetScale's simulator, as before), 48 unsupported (PlanetScale 2.0 refuses S1g and S1h as it refuses every statement with parameters) and 3 not run, and 27 of 93 control rows failing as designed: the `unsafe()` control without inputs fails S1g and S1h, and the one that writes every array as its literal fails S1g. Every row that was in RESULTS.md before has the result it had.

## Re-run (2026-10-20): Reylo decodes every value

S10 is new, since `run` sets every value's type itself on every driver (wiki/driver-decoding.md): one statement per dialect over a `typed` table added to each seed, with values every driver reads exactly (Bun.sql keeps a timestamp's milliseconds, not its microseconds, and throws on a NULL inside an `int4[]`), compared value by value with its JavaScript type. It ran against src/ at 2a111c9 and its successors on Node 26.11.1 and 24.21.0, Bun 1.4.3 and Miniflare 4.20260730.0, with the containers in colima (Docker 29.5.2) and a throwaway Postgres 18.6 standing in for the shared Postgres 14 (`DM_PG_URL`): 47 jobs and 3681 rows, with 3628 pass, 2 fail (PlanetScale's simulator, as before), 48 unsupported and 3 not run, and 30 of 97 control rows failing as designed. All 172 S10 rows pass, on every version, shape and target, mariadb 2.5.6 included. [compare.ts](compare.ts) against the RESULTS.md before it: every row that was there has the result it had. The three new failing control rows are S10 on the PGlite wrappers of `control/pg`: a client of no driver's shape is called as it is, and its values are its own, which S10 catches.

Two run-time faults the scenario found on its first run were fixed before that run: on `mariadb/callback` the rows carry no `meta` (it is the callback's third argument), and mariadb 2's columns have no `isSet()`, so a SET stayed an array and, on the callback API, a DATETIME kept its space; Reylo now takes those columns from the module's kinds. And a synchronous SQLite client whose statement answers through a promise (a control) came back as no rows instead of being refused by `runSync`.

Since the same day a 64-bit integer is a BigInt, an instant its ISO 8601 text in UTC, and a MySQL TINYINT(1) or BIT(1) a boolean (wiki/driver-decoding.md), and S10 expects them: the Postgres `big` is `9007199254740993n` and `stamp` `"2026-10-20T08:00:00.123Z"` (Bun.sql's Date to its millisecond, which the fixture's value has), the MySQL `big` and `ubig` `-9007199254740993n` and `18446744073709551615n`, and its `flag` and `bit1` `true`. Its first run found that mariadb 2.5.6, which has no native BigInt by default, returned the BIGINT rounded, which Reylo refused on its 14 S10 rows; Reylo now passes mariadb 2's own `supportBigInt` and `bigNumberStrings` per query too. The run after it, on Node 26.11.1 and 24.22.0, Bun 1.4.3 and Miniflare 4.20260730.0, colima (Docker 29.5.2) and a throwaway Postgres 18.6, wrote [RESULTS.md](RESULTS.md): 47 jobs and 3681 rows, 3628 pass, 2 fail (PlanetScale's simulator, as before), 48 unsupported and 3 not run, and 30 of 97 control rows failing as designed, every row with the result it had in the RESULTS.md before it ([compare.ts](compare.ts)).

## Re-run (2026-10-20): the round-trip rule

S11 and S11b are new, for the round-trip rule: a parameter takes what `run` returns for its SQL type, on every driver (wiki/driver-decoding.md). S11 reads the `typed` row as S10 does and passes it back into a statement that compares every column with a parameter, each parameter of the type the engine gives it (`ROUND_TRIP` in [lib/harness.ts](lib/harness.ts), the pairs tests/render.test.ts pins); S11b passes back the instants past 9999 and before 1. They first ran against main's src/ of 2026-10-20, where these rows failed:

| Rows | Result before the fix | Diagnosis |
| --- | --- | --- |
| pg 8.23.0, pg-promise 12.7.1, postgres 3.4.9, PGlite 0.2 to 0.5, Neon 0.9.5, 0.10.4 and 1.1.0, S11b | fail (every row) | Postgres refuses the extended year a timestamptz past 9999 comes back with: "time zone displacement out of range: "+010000-01-01T00:00:00Z"" |
| better-sqlite3 13.0.3, 12.11.1 and 11.10.0, S11 | fail (every row) | better-sqlite3 refuses the `true` a truth value comes back as: "SQLite3 can only bind numbers, strings, bigints, buffers, and null" |
| Bun.sql 1.4.3 on Postgres, S11b | skip (4 rows) | Bun.sql decodes a timestamptz before 1 AD as an Invalid Date, which Reylo refuses since 44ac8d4, so the instant never comes back to pass in |

Reylo now writes an extended or BC year in Postgres's own form (`10000-…`, `0002-… BC`) and binds a SQLite truth value as 1 or 0; the engine types a MySQL parameter by the column it is compared with or written to, so `big = :big` takes the BigInt a BIGINT returns and `flag = :flag` the boolean a TINYINT(1) returns, which mysql2, mariadb, PlanetScale and Bun.sql bind as 1. On MySQL the statement compares the JSON column with `json_extract(:doc, '$')`. The run after the fix wrote [RESULTS.md](RESULTS.md): 46 jobs and 3894 rows, with 3834 pass, 2 fail (PlanetScale's simulator, as before), 51 unsupported (PlanetScale 2.0 refuses S11 as it refuses every statement with parameters), 4 skip (S11b on Bun.sql) and 3 not run, and 36 of 104 control rows failing as designed. Every row that was in RESULTS.md before has the result it had; the 217 others are S11's and S11b's.

Since the evening of 2026-10-20 a `timestamptz` parameter takes only the string that names its zone, which `.run()` returns, and no `Date`, so S1h binds `"2026-01-02T03:04:05.006Z"` and checks that a `Date` for a `timestamptz` is refused before the driver is called, as one for a `date` is, and S1f binds a `timestamptz[]` of strings; S1h's name changed with it, so its rows are recorded under the new name, each with the result it had, and the counts are as above.

`scripts/gates.sh` runs every job and `node compare.ts` after them, the container jobs when Docker or colima is there; `node runner.ts --only <id>` runs one job, and `node compare.ts` compares whatever `.out/` holds.

## Re-run (2026-10-20): Turso's new engine

The Turso jobs are new, since Reylo's run time supports Turso's new engine, `@tursodatabase/database`, beside Turso Cloud through `@libsql/client`. [drivers/turso.ts](drivers/turso.ts) runs every scenario on one release of each minor line in use, 0.8.3 (`latest`), 0.7.2, 0.6.1 and 0.4.4 (13,464, 11,054, 5,758 and 559 downloads in the week before), each as two jobs: its promise API (`connect()`) and its compat Database. Before the fix, against main's src/ of 2026-10-20, Reylo could not run a statement on any of them:

| Rows | Result before the fix | Diagnosis |
| --- | --- | --- |
| 0.6.1, 0.7.2 and 0.8.3, the Database and the Transaction | `run()` never settled | The Database has `all()` and `exec()`, so Reylo took it for sqlite3's and called `db.all(text, values, callback)`, whose callback Turso never calls (wiki/structured-params/positions.md, S1) |
| 0.4.4, the Database | `run()` rejected with "not implemented" | It has no `all()`, so Reylo took it for a synchronous database and read its statement's `reader`, which 0.4 has not implemented |
| every release, the compat Database | `runSync` refused it | It has `batch()`, which Reylo took for D1's |
| every release, any statement that reads a table | "The database changed after …: t.id was INTEGER … and is now integer" | Turso's engine gives a declared type as written, where SQLite's `pragma_table_xinfo` upper-cases the names it knows, and the freshness check compared the text exactly (S2) |

Reylo now tells Turso's Database by its `connect()` beside `maxWriteReplicationIndex()`, which every release has, and its Transaction by `all()` beside `batch()`, which sqlite3's database lacks, prepares a statement per call, sets its integer mode as on better-sqlite3 and awaits it (`runTurso` in [src/decode-sqlite.ts](../../src/decode-sqlite.ts)); it runs the compat Database as better-sqlite3, with a new statement per call, since a 0.4 statement with a subquery returned no rows the second time it ran (S5b, which reuses S5a's text), and a `reader` it cannot read is taken as unknown; and the freshness check compares a SQLite type without its case ([src/fresh.ts](../../src/fresh.ts)). Two scenarios are skipped for Turso's own reasons: S7c on 0.4, which has no SAVEPOINT ("SAVEPOINT not supported yet"), and S7c on every compat `db.transaction(fn)`, whose nested call fails ("cannot start a transaction within a transaction") where better-sqlite3's runs as a savepoint. And the SQLite syntax error S6a expects matches Turso 0.4's wording, "unexpected token 'selec' at offset 0", as well. The run after the fix, on Node 26.11.1 and 24.22.0, Bun 1.4.3 and Miniflare 4.20260730.0, with the containers in colima and a throwaway Postgres 18.6, wrote [RESULTS.md](RESULTS.md): 4210 rows over eight more jobs, with 4145 pass, 2 fail (PlanetScale's simulator, as before), 51 unsupported, 9 skip and 3 not run, and 36 of 104 control rows failing as designed. All 316 Turso rows pass but the five skipped, and every row that was in RESULTS.md before has the result it had ([compare.ts](compare.ts)). The type check takes the ten Turso client types too ([typecheck/RESULTS.md](typecheck/RESULTS.md), 612 cases over 104 client types, all as expected).

## Re-run (2026-10-20): mariadb on MySQL alone

The mariadb jobs run on MySQL 8.4 alone: the matrix checks Reylo's run time on MySQL servers, and mariadb is one of the MySQL clients it runs. The run after the change, on Node 26.11.1 and 24.22.0, Bun 1.4.3 and Miniflare 4.20260730.0, with the containers in colima (Docker 29.5.2) and a throwaway Postgres 18.6 standing in for the shared Postgres 14 (`DM_PG_URL`), wrote [RESULTS.md](RESULTS.md): 51 jobs and 3760 rows, with 3695 pass, 2 fail (PlanetScale's simulator, as before), 51 unsupported, 9 skip and 3 not run, and 36 of 104 control rows failing as designed. [compare.ts](compare.ts) against the RESULTS.md before it: 3801 rows compared over 52 jobs, 0 different; the 450 rows that left are those of the three mariadb jobs on the second server, which no longer run.

## Re-run (2026-10-20): SQLite's built JSON

S10j is new, after the engine typed the JSON a SQLite statement builds as that JSON, which the run time parses (wiki/driver-decoding.md). It passes on every SQLite job (every better-sqlite3 line, `node:sqlite` on both Nodes, `bun:sqlite`, Bun.sql's SQLite, every libSQL, Turso's engine 0.6.1, 0.7.2 and 0.8.3 through both its APIs, sqlite3, the sqlite wrapper and D1) but Turso's engine 0.4.4, which answers `json_group_array` over no rows with NULL where SQLite and Turso 0.6 and later answer `[]`: its three shapes skip S10j ([drivers/turso.ts](drivers/turso.ts)). The run, on Node 26.11.1 and 24.22.0, Bun 1.4.3 and Miniflare 4.20260730.0, with the containers in colima and a throwaway Postgres 18 standing in for the shared Postgres 14 (`DM_PG_URL`), wrote [RESULTS.md](RESULTS.md): 3805 rows, with 3737 pass, 2 fail (PlanetScale's simulator, as before), 51 unsupported, 12 skip and 3 not run, and 36 of 105 control rows failing as designed (the synchronous SQLite control's S10j passes, since its statements answer at once). [compare.ts](compare.ts) against the RESULTS.md before it: 3844 rows over 52 jobs, and the 43 that differ are S10j's, new.

## Run (2026-10-22): Drizzle

Reylo now takes a Drizzle database and the transactions it hands out, and runs on the driver client Drizzle's session holds: `session.client`, the pool for a database and the transaction's own connection inside one, or `session.tx` inside a libSQL transaction, where `client` stays the base client ([src/execute.ts](../../src/execute.ts)). Run on `session.client` there, the libSQL job failed 13 rows with `SQLITE_BUSY`, the base client waiting on the transaction's lock. Over postgres.js, Drizzle replaces the client's `json` and `jsonb` serializers with ones that pass a value through, so Reylo now gives every postgres.js client a JSON parameter as its JSON text declared `unknown` (`sql.typed(text, 705)`). The harness takes libSQL's own `RangeError` in S10 on Drizzle's libSQL job as on libSQL's. The five Drizzle jobs, on Node 26.11.1 against a throwaway Postgres 18 (`DM_PG_URL`) and MySQL 8.4 (`DM_MYSQL_URL`), add 310 rows to [RESULTS.md](RESULTS.md), all of which pass; every other job's rows are as they were, and the gates run them again.

## Type check

[typecheck/check.ts](typecheck/check.ts) generates [typecheck/cases.ts](typecheck/cases.ts), which declares one value of each client type and passes it to `run` on a Postgres, a MySQL and a SQLite query. It also checks the type `run`, `with(...)(...)` and `runSync` return (a promise from `run` and `with` on every client, the value from `runSync` on a synchronous SQLite client) and that `runSync` rejects every other client, then runs the repository's tsc (7.1.0-dev.20260918.1) and maps each diagnostic back to its line. Against the final src (src/execute.ts 332f629f3f63), all 552 cases over 94 client types are as expected: none is wrongly accepted and none is wrongly rejected; with the ten Turso client types of 2026-10-20, all 612 over 104 are. Turso's shape needs `exec()` beside `prepare()`, because postgres.js's TransactionSql has a `prepare(name)` whose result is of any type, which the first try at the type took for Turso's. [typecheck/RESULTS.md](typecheck/RESULTS.md) lists every case.

Against the re-run's src (c46b8b73fe74), 21 were wrongly accepted. Each was closed with a property that only the right client has, used in the type alone: `PgQueryClient` has `escapeId?: never` (mariadb has `escapeId()`), `LibsqlClient` requires `executeMultiple()` (libSQL 0.14 to 0.18 have it, mariadb does not), and the MySQL and SQLite unions take `BunSqlClient`, which requires Bun's `commitDistributed()` beside `unsafe()` (postgres.js has no such method).

| Wrongly accepted before the fix | Why the type admitted it | At run time |
| --- | --- | --- |
| postgres 3.4.9 `Sql`, `TransactionSql` and `ReservedSql` on a MySQL or a SQLite query (6 cases) | `UnsafeClient` is in all three unions for Bun.sql, whose one class serves every adapter. postgres.js has `unsafe` too, and the two cannot be told apart by shape | The MySQL or SQLite text reaches Postgres: "PostgresError: syntax error at end of input" ([probes/cross-dialect.ts](probes/cross-dialect.ts), with postgres 3.4.9) |
| mariadb 3.5.4, 3.4.5 and 2.5.6 Pool, Connection and PoolConnection on a Postgres query (9 cases) | mariadb types `query<T = any>(...): Promise<T>`, so it satisfies `PgQueryClient`'s `query(...): PromiseLike<{ rows: unknown[] }>` | "Unknown column '$1' in 'where clause'" (SqlError 1054, the same probe with mariadb 3.5.4 on MySQL 8.4) |
| mariadb 3.5.4 and 3.4.5 Pool, Connection and PoolConnection on a SQLite query (6 cases) | mariadb 3 has `execute<T = any>(sql: string \| QueryOptions, ...)`, which satisfies `LibsqlClient`'s `execute({ sql, args })` (2.5.6 has no `execute`) | "Parameter at position 0 is not set" (SqlError 45016) |

These were checked as expected:

- mysql2/promise Pool, Connection and PoolConnection are rejected on a Postgres query, though pg's query type now admits a rows array.
- @planetscale/database 2.0.0's Connection, Client and Transaction are rejected on every dialect.
- The `mariadb/callback` Pool, Connection and PoolConnection are accepted on MySQL only.
- node:sqlite's `SQLTagStore` is rejected everywhere.
- On better-sqlite3's Database, node:sqlite's DatabaseSync and bun:sqlite's Database, `run` and `with(...)(...)` are typed as the row. On every other accepted client they are typed as a promise.

## Not tested

- **pg 7.18.2.** It cannot connect on any Node that runs Reylo (see the first run). A shape-only test would add nothing, because its API is the same `query(text, values)` → `{ rows }` as pg 8.
- **better-sqlite3 9.6.0 and 8.7.0** (1.8 and 1.3 percent of the package's downloads). No prebuilt binary exists for Node 24 (ABI 137) or 26 (ABI 147), so they could run only from a source build in native-old/ against Node 24's headers, and neither compiles there (tried 2026-10-11 with Node 24.21.0 and node-gyp 12.4): their binding asks for C++17 where Node 24's V8 headers require C++20, and with `CXXFLAGS=-std=c++20` the sources still call `ObjectTemplate::SetAccessor`, `AccessorGetterCallback` and `CopyablePersistentTraits`, which that V8 removed. 11.10.0 (18 percent) is in the same position and runs that way: its source does not compile against Node 26's V8 (it calls `Object::GetPrototype`, `Context::GetIsolate` and `PropertyCallbackInfo::This`, which V8 removed), and it compiles against Node 24's (`node <npm's node-gyp.js> rebuild --release` with Node 24.19 in `native-old/node_modules/bs3-11`, about 85 minutes at a load average near 100; 22 seconds on 2026-10-11 with Node 24.21.0, after `nub install --ignore-scripts` in native-old/), so the runner runs these lines on Node 24. The first install had failed because a global node-gyp 3.8.0 on PATH still expects Python 2, so native-old/ builds with npm's node-gyp 13.
- **miniflare 5.** The version npm tags `latest` (5.20260926.1-alpha and later, which wrangler 4.144 depends on) rejects the v4 constructor options (`modules`, `script`, `d1Databases`) and wants a new `workers: [{ config }]` format, so the D1 job uses miniflare 4.20260730.0. D1 is reached through Miniflare's Node proxy, not from inside a Worker.
- **The real Neon and PlanetScale services.** Both are live against local stand-ins: Neon's proxy image (built 2026-03-27) and ps-http-sim. The real services could differ in details such as PlanetScale's session concurrency.
- **Neon's HTTP `transaction()`** takes an array of queries built by the `sql` function, not a client, so a Reylo query cannot join it. D1 has no interactive transaction either (`batch()` takes prepared statements).
- **Other server and driver pairs:** @libsql/client against a sqld server (`http:` or `ws:` URLs), Turso's `@tursodatabase/serverless`, `@tursodatabase/sync` and `@tursodatabase/database-wasm`, PGlite through `PGliteWorker`, `pg-native`, Neon's `arrayMode`, and better-sqlite3 or the npm drivers under Bun.
- **Other runtimes:** Deno, and a Cloudflare Worker's own D1 binding.

## Layout

| Path | Contents |
| --- | --- |
| [run.sh](run.sh) | The whole matrix |
| [setup.sh](setup.sh), [seed/](seed/) | Servers and fixtures (tables `people`, `scratch`, `uniq`) |
| [runner.ts](runner.ts) | The job list, the process pool (4 at a time, 300 s per job), results.json and RESULTS.md |
| [lib/harness.ts](lib/harness.ts) | Scenarios, assertions, the return-contract checks, transaction helpers |
| [lib/tagged.ts](lib/tagged.ts) | The shapes shared by postgres.js and Bun.sql |
| [drivers/](drivers/) | One file per driver family |
| [typecheck/](typecheck/) | The type check |
| [probes/](probes/) | The controls behind two diagnoses: PlanetScale's session lock and the cross-dialect run-time errors |
| .out/, .data/ | Per-job logs and JSON Lines, the SQLite files (gitignored) |
