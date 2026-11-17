---
name: reylo
description: "Use in a project that imports .sql files with Reylo, which a tsconfig.json whose contentMappers names reylo, a .reylo.yaml file or a reylo dependency shows: writing or changing a .sql query, calling its .run(), .runSync() or .with(), passing it a database client, tracing or logging its queries, fixing an error the type check reports in a .sql file, or evaluating, planning or carrying out a move of a project's queries to Reylo from Prisma Client."
---

# Reylo

Reylo lets TypeScript import a `.sql` file as a typed query. The types come from the database itself: the parameters, the columns and their nullability, nested JSON and how many rows the query returns. Nothing is declared by hand, so a type is changed by changing the SQL or the schema, never by an annotation.

This file and its `references/` cover what a project needs. The docs are https://reylo.dev, with a page per database, https://reylo.dev/postgres, https://reylo.dev/mysql, https://reylo.dev/sqlite and https://reylo.dev/sqlserver, all served as Markdown at https://reylo.dev/llms.txt: read them for what this file and its references don't cover.

## The whole loop

A query, its import, a run, a database error as a value, and a transaction, on Postgres with `pg`:

```sql
-- src/sql/create_user.sql
insert into users (name, email) values (:name, :email) returning id, name, email
```

```ts
import pg from "pg";
import createUser, { type Row as User } from "./sql/create_user.sql";
import getUser from "./sql/get_user.sql";

const db = new pg.Pool({ connectionString: process.env.DATABASE_URL });

const user = await getUser.run(db, { id: 1 }); // the row, or null where the query matches at most one row

const created = await createUser.try(db, { name: "Ada", email: "ada@example.com" });
if (created.error?.condition === "unique_violation") {
  created.error.constraint; // "users_pkey" | "users_email_key" | undefined, the keys of the table it writes
} else if (created.value) {
  const ada: User = created.value;
}

// a transaction is a client of its own, passed to .run() the same way; await each query in turn
const client = await db.connect();
try {
  await client.query("begin");
  await createUser.run(client, { name: "Grace", email: "grace@example.com" });
  await client.query("commit");
} catch (error) {
  await client.query("rollback");
  throw error;
} finally {
  client.release();
}
```

On an error the database raised, `.run()` throws the driver's own error and `.try()` returns `{ error }`, where a query that succeeds gives `{ value }`. On Postgres an error aborts the transaction it is in, even one `.try()` returned. On postgres.js and Bun's `SQL`, a transaction is `db.begin(async (tx) => …)`, with `tx` passed to `.run()`.

## Before changing anything

- Find the project file, `.reylo.yaml` or `.reylo.yml`, beside `package.json`. Its `direct_url`, or `DATABASE_URL` without one, is the dev database the type check reads the schema from, and its scheme names the database: `postgres://`, `mysql://`, `file:` for SQLite, `sqlserver://` for SQL Server. Reylo reads `.env.local` and `.env` as `dotenv` does, so a value there that holds a `#` is quoted.
- The type check is `tsc --noEmit --runExternalCode` on TypeScript 7.1, usually a `typecheck` script. Run it before and after a change.

## Writing a query

- One statement per `.sql` file, imported by a relative path: `import getUser from "./get_user.sql"`. The statement can be any one the database prepares: a query, an insert, an update, a delete. A file of several statements, such as a migration, is an error in the type check, and a migration's `create table` fails once the table exists; leave migrations out with `exclude` in `tsconfig.json`, as in `"exclude": ["db/**"]`. On SQL Server a file holds a T-SQL batch, whose one result set is typed (`references/running.md`).
- Each `.sql` module also exports its types: `Row`, one row of its result, and `Params`, the parameters `.run()` takes, as in `import type { Row } from "./get_user.sql"`. Name a query's row or parameters with them rather than declaring the shape again, which the type check can't keep in step with the query.
- Parameters are `:name` on every database, and `@name`, T-SQL's own form, on SQL Server. Each appears once in a statement; to use a value twice, select it once, cast to its type, in a `with` and read it from there: `with input as (select :id::int as id) select … from input`. To let the caller pick the order, sort by a `case` on the parameter (`order by case when :sort = 'name' then name end`): a parameter alone as an `order by` or `group by` key is an error, since every row would sort by the same value (https://reylo.dev/#dynamic-sorting).
- A parameter the query does not type is an error that says to cast it: `:q::text`. Reylo writes each database's own cast (https://reylo.dev/#casts).
- A parameter accepts null only cast to `nullable(…)`, which keeps the type inside it and lets the caller leave the parameter out: `where user_id = coalesce(:user_id::nullable(int), user_id)`. A `coalesce` or a null test of a parameter alone does not make it nullable (https://reylo.dev/#nullable-parameters). A search form's or a list page's filters are one JSON parameter whose schema gives each filter its type, `enum` and default, each read with `->>` in a `with`, so Postgres and MySQL plan the query with its values and a filter given can use an index; `jsonb_to_record` and `json_table` read the whole table (https://reylo.dev/#dynamic-filtering).
- A parameter written bare, with no cast, to a column that may hold null takes null too, and may be left out. Any other parameter takes no null: `.run()` throws before the query runs when one gets `null` or is left out, so pass every value its type asks for, even from an untyped request body. A JSON parameter there takes `null` as JSON's `null` instead, in a `coalesce` or a null test too: cast it to `nullable(…)` to pass SQL NULL, as `coalesce(:meta::nullable(jsonb), meta)` needs to keep `meta`.
- A list is one parameter: `= any(:ids::int[])` on Postgres, and a JSON parameter read with `json_table` on MySQL or `json_each` on SQLite. Copy each database's form from https://reylo.dev/#filtering-by-a-list.
- A statement whose one column is an expression with no `as` returns the column's values rather than rows: `select count(*) from users` is a `bigint`, and `select lower(name) from users` a `string[]`. An `as`, or a column read as it is (`select id`), keeps the rows. Where that value may be null and the query may match no row, `null` stands for both, so name the column to tell them apart. Among several columns, an expression whose database-given name isn't an identifier, like `count(*)` or `?column?`, is a type error: name it with `as`.
- A column is nullable or not as the schema and the query make it: `not null`, a left join, `coalesce`. Change those to change the type. A write's `returning` reads a column it set to a value as that value, on Postgres where no `BEFORE` row trigger, rule or view can change the row: `update users set email = :email::text … returning email` returns a string. On Postgres a stored array's elements are nullable unless a `CHECK` keeps them out: `check (array_position(tags, null) is null)`.
- Fetch each row's related rows with a subquery in the same statement, which Reylo types field by field. On Postgres it is an array of rows, `array(select p from (select id, title from posts where user_id = u.id order by id) p) as posts`, and one related row is `(select p from (select … limit 1) p)`, null when none matches; on MySQL and SQLite it is a JSON array, `json_arrayagg` or `json_group_array` of `json_object(…)`. Copy each database's form from https://reylo.dev/#nested-fetching. On Postgres keep the array of rows rather than `jsonb_agg`: inside JSON a `bigint` past 2^53 rounds and a `timestamptz` takes the session's time zone (https://reylo.dev/postgres#json).
- A JSON value with no schema, such as a `jsonb` column or a value read out of a document, is `Json`, any JSON value, which `reylo` exports (`import type { Json } from "reylo"`). Narrow it or validate it before reading it as a shape. A JSON column a JSON Schema `CHECK` holds is typed by that schema (`references/writing.md`).
- To read how the project's database types a query, run `npx reylo analyze <file.sql>`: it prints the statement's parameters, columns, types, nullability, cardinality and diagnostics as YAML. The type check reports every query at once.

## Running a query

- Pass the project's own database client to `.run()`: a pool, a client, a transaction or connection taken from one, or a Drizzle database or Knex instance over one (https://reylo.dev/#run shows each driver's client). At run time Reylo opens no connection of its own. On Postgres, pass the driver's own object, never a hand-written wrapper with a `query()` or `unsafe()` that calls the pool: `.run()` refuses a client that is none of the drivers', since a wrapper returns values the driver already converted. Keep a wrapper a `Pool`, as a subclass or a proxy.
- The result's shape follows the row count Reylo infers: an array, one row, a row or `null`, or nothing for a write without `returning`. On Postgres, a write whose table has a `BEFORE` row trigger is a row or `null` unless every `return` of the trigger's PL/pgSQL returns the row, so look there when a write is unexpectedly nullable.
- Every driver gives the same types: a 64-bit integer is a `bigint`, which `JSON.stringify` refuses (cast it in the SQL, as `count(*)::int`, where it fits a `number`), a decimal its exact text, an instant its ISO 8601 text in UTC with six fraction digits, and no value is a `Date`. Pass a parameter in the form `.run()` returns for its type (https://reylo.dev/#data-types; every other type in `references/running.md`).
- The error `.try()` returns is read into one shape per database whichever driver ran the query: Postgres's `code` and `condition` (`unique_violation`) with the `constraint`, `table` and `column` it names; MySQL's `code` (`ER_DUP_ENTRY`), `errno` and `key`; SQLite's `code` (`SQLITE_CONSTRAINT_UNIQUE`), `table` and `columns`; SQL Server's `number` (`2627`) with the `constraint`, `table` and `column` its message names. The codes are typed, so a misspelled one is a type error, and where the query runs none of the project's own code (a trigger, a function it wrote), each error is narrowed to the constraints, keys and columns of the tables the query writes; any other error, such as a refused connection, still throws. Each `.sql` module exports the error's type as `QueryError` (https://reylo.dev/#try).

## After a change

- Run the type check. An error in a `.sql` file shows at its line and column, and `tsc` holds back the TypeScript errors while one remains, so fix the SQL first (https://reylo.dev/#errors).
- After a migration, the type check reads the new schema: a dropped column is an error in each `.sql` file that names it and in the code that reads it.
- `reylo/.cache/` is Reylo's cache of each query's analysis. Never edit it or commit it.

## References

Open one when the task touches it:

- The rarer forms of a query, in `references/writing.md`: slices, bulk inserts, constants, reusable SQL functions, JSON Schema checks, and the playground's API.
- Running queries, in `references/running.md`: each driver's details (postgres.js and poolers, MySQL through Hyperdrive, SQLite's `.runSync()`, `STRICT` tables and quotes, SQL Server's batches, sign-in and `mssql` pool), every value type, arrays of more than one dimension, `.readOnly`, `.with()`, tracing, row keys and decoders, the cache, and the schema check at run time.
- Migrations, in `references/migrations.md`: `reylo migrate` and `reylo db`, the schema file, writing and running migrations, the lock, and deploying.
- Moving a project's queries from Prisma Client, in `references/prisma.md`.
