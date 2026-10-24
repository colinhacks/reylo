---
name: using-reylo
description: "Use in a project that imports .sql files with Reylo, which a tsconfig.json whose contentMappers names reylo, a .reylo.yaml file or a reylo dependency shows: writing or changing a .sql query, calling its .run(), .runSync() or .with(), passing it a database client, or fixing an error the type check reports in a .sql file."
---

# Using Reylo

Reylo lets TypeScript import a `.sql` file as a typed query. The types come from the database itself: the parameters, the columns and their nullability, nested JSON and how many rows the query returns. Nothing is declared by hand, so a type is changed by changing the SQL or the schema, never by an annotation.

The docs are one page, https://reylo.dev, also served as Markdown at https://reylo.dev/llms.txt, with a page per database: https://reylo.dev/postgres, https://reylo.dev/mysql and https://reylo.dev/sqlite. Read the section a task touches before writing SQL for it.

## Before changing anything

- Find the project file, `.reylo.yaml` or `.reylo.yml`, beside `package.json`. Its `direct_url`, or `DATABASE_URL` without one, is the dev database the type check reads the schema from, and its scheme names the database: `postgres://`, `mysql://`, `file:` for SQLite.
- The type check is `tsc --noEmit --runExternalCode` on TypeScript 7.1, usually a `typecheck` script. Run it before and after a change.

## Writing a query

- One statement per `.sql` file, imported by a relative path: `import getUser from "./get_user.sql"`. The statement can be any one the database prepares: a query, an insert, an update, a delete. A file of several statements, such as a migration, is an error in the type check, and a migration's `create table` fails once the table exists; leave migrations out with `exclude` in `tsconfig.json`, as in `"exclude": ["db/**"]`.
- Parameters are `:name` on every database. Each appears once in a statement; to use a value twice, select it once, cast to its type, in a `with` and read it from there: `with input as (select :id::int as id) select … from input`. To let the caller pick the order, sort by a `case` on the parameter (`order by case when :sort = 'name' then name end`): a parameter alone as an `order by` or `group by` key is an error, since every row would sort by the same value (https://reylo.dev/#dynamic-sorting).
- A parameter the query does not type is an error that says to cast it: `:q::text`. Reylo writes each database's own cast (https://reylo.dev/#casts).
- A parameter accepts null only cast to `nullable(…)`, which keeps the type inside it and lets the caller leave the parameter out: `where user_id = coalesce(:user_id::nullable(int), user_id)`. A `coalesce` or a null test of a parameter alone does not make it nullable (https://reylo.dev/#nullable-parameters). A search form's or a list page's filters are one JSON parameter whose schema gives each filter its type, `enum` and default, read in a `with` (https://reylo.dev/#dynamic-filtering).
- A parameter written bare, with no cast, to a column that may hold null takes null too, and may be left out. Any other parameter takes no null: `.run()` throws before the query runs when one gets `null` or is left out, so pass every value its type asks for, even from an untyped request body.
- A list is one parameter: `= any(:ids::int[])` on Postgres, and a JSON parameter read with `json_table` on MySQL or `json_each` on SQLite. Copy each database's form from https://reylo.dev/#filtering-by-a-list.
- Many rows to insert are one JSON parameter whose cast carries their JSON Schema (https://reylo.dev/#bulk-inserts, https://reylo.dev/#typed-json).
- A column is nullable or not as the schema and the query make it: `not null`, a left join, `coalesce`. Change those to change the type.
- On Postgres, a JSON column a JSON Schema `CHECK` holds is typed by that schema, and so is a key read from it with a literal key, `payload -> 'x'` or `payload ->> 'x'`. Any other value read out of a document is `unknown` until a cast types it (https://reylo.dev/postgres#json).

## Running a query

- Pass the project's own database client to `.run()`: a pool, a client, a transaction or connection taken from one, or a Drizzle database over one (https://reylo.dev/#supported-drivers, https://reylo.dev/#transactions). At run time Reylo opens no connection of its own.
- `.runSync()` returns the result itself on a synchronous SQLite client (`better-sqlite3`, `node:sqlite`, a Durable Object's `ctx.storage.sql`), so it works inside their synchronous transactions. `.with(client)` binds the query to a client.
- The result's shape follows the row count Reylo infers: an array, one row, a row or `null`, or nothing for a write without `returning`. On SQLite, a subquery, a `with` query or a view that selects a table's key unchanged is matched on that key like the table.
- Every driver gives the same types: a 64-bit integer is a `bigint`, a decimal its exact text, an instant its ISO 8601 text in UTC, a pgvector `vector` a `number[]`, an `hstore` an object of its keys, and no value is a `Date`. Pass a parameter in the form `.run()` returns for its type (https://reylo.dev/#data-types).
- On SQLite, a table that is not `STRICT` can hold a value of another kind than its column's declared type, such as the text `'abc'` in an `INTEGER` column; `.run()` throws an error naming the column rather than return it. Replace the value, or read the column with `cast(col as text)` (https://reylo.dev/sqlite#declared-types-and-strict-tables).
- `.run()` throws the driver's own error. `.try()` returns `{ value }`, or `{ error }` for an error the database raised, read into one shape per database whichever driver ran the query: Postgres's `code` and `condition` (`unique_violation`) with the `constraint`, `table` and `column` it names; MySQL's `code` (`ER_DUP_ENTRY`), `errno` and `key`; SQLite's `code` (`SQLITE_CONSTRAINT_UNIQUE`), `table` and `columns`. The codes are typed, so a misspelled one is a type error, and where the query runs none of the project's own code (a trigger, a function it wrote), each error is narrowed to the constraints, keys and columns of the tables the query writes; any other error, such as a refused connection, still throws. `.trySync()` is the same on a synchronous SQLite client, and each `.sql` module exports the error's type as `QueryError` (https://reylo.dev/#try).

## After a change

- Run the type check. An error in a `.sql` file shows at its line and column, and `tsc` holds back the TypeScript errors while one remains, so fix the SQL first (https://reylo.dev/#errors).
- After a migration, the type check reads the new schema: a dropped column is an error in each `.sql` file that names it and in the code that reads it. Reylo runs no migrations; use the project's own tool.
- `.reylo/` is Reylo's cache of each query's analysis. Never edit it or commit it.
