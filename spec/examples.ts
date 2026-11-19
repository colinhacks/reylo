/**
 * Examples of Reylo Type Defs, the type descriptor format (`index.ts`), database by database: each a statement, the schema it reads, and what introspecting it gives, whole or the part the example is about. `nub run typecheck` checks each against the types, and `tests/typedefs.test.ts` against the JSON Schema.
 *
 * Those of Postgres, MySQL, SQLite and SQL Server are what Reylo's engine writes: `tests/spec-examples.test.ts` runs each one's setup and statement on its database and compares every field the example shows, all but the server's version. Reylo runs no Oracle or DuckDB, so those are written by hand. `nub run typedefs` copies every example into `examples.json`, which the engine's own tests read.
 */

import type { FloatType, InstantType, IntegerType, StructType, StatementDescriptor, TextType } from "./index.ts";

/** One example: a statement on one database, the schema it reads, and what introspecting it gives. */
export interface Example {
  /** The database, as `dialect.name` names it. */
  database: "postgres" | "mysql" | "sqlite" | "duckdb" | "sqlserver" | "oracle";
  /** The schema the statement reads, run first on an empty database. */
  setup?: string;
  /** The statement as written, with `:name` parameters. */
  statement: string;
  /** What introspecting the statement gives: the whole descriptor, or the fields the example is about. */
  descriptor: Partial<StatementDescriptor>;
}

const int8: IntegerType = { kind: "integer", bits: 64, label: "int8" };
/** Postgres's text, which holds every character but NUL. */
const pg_text: TextType = { kind: "text", code_points: [[1, 1114111]], label: "text" };

/** A first example, on Postgres: one user, by id. The statement's leading comment is its `comment`, and `sql` is the statement as the database runs it, with Postgres's own placeholders. */
export const first_example: Example = {
  database: "postgres",
  setup: "create table users (id int8 primary key, email varchar(255) not null, bio text); comment on column users.bio is 'Shown on the profile page'",
  statement: "-- One user, by id.\nselect id, email, bio from users where id = :id",
  descriptor: {
    $schema: "https://reylo.dev/typedefs/0.1.1.json",
    dialect: { name: "postgres", version: "18.1" },
    sql: "-- One user, by id.\nselect id, email, bio from users where id = $1",
    kind: "select",
    comment: "One user, by id.",
    params: [{ name: "id", type: int8 }],
    result: {
      kind: "table",
      cardinality: "at_most_one",
      columns: [
        { name: "id", type: int8 },
        { name: "email", type: { kind: "text", code_points: [[1, 1114111]], size: { max: 255 }, label: "varchar" } },
        { name: "bio", nullable: true, type: pg_text, comment: "Shown on the profile page" },
      ],
    },
    diagnostics: [],
  },
};

const int64: IntegerType = { kind: "integer", bits: 64 };
const float8: FloatType = { kind: "float", exponent_bits: 11, mantissa_bits: 52, infinity: true, nan: true, label: "float8" };
const timestamptz: InstantType = { kind: "instant", tick: "1/1000000", min: "-4713-11-24T00:00:00Z", max: "+294276-12-31T23:59:59.999999Z", infinity: true, label: "timestamptz" };
const point: StructType = { kind: "struct", label: "point", fields: [{ name: "x", type: float8 }, { name: "y", type: float8 }] };
const sqlite_text: TextType = { kind: "text", encoding: "UTF-8", ill_formed: true };

/** Postgres: the columns the rows are sorted on. `id` follows `at`, and NULLs sort first when descending unless the key says otherwise. */
export const sorted_rows: Example = {
  database: "postgres",
  setup: "create table events (id int8 primary key, at timestamptz)",
  statement: "select id, at from events order by at desc, id",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [
        { name: "id", type: int8 },
        { name: "at", nullable: true, type: { kind: "instant", tick: "1/1000000", min: "-4713-11-24T00:00:00Z", max: "+294276-12-31T23:59:59.999999Z", infinity: true, label: "timestamptz" } },
      ],
      unique: [["id"]],
      order: [
        { column: "at", desc: true, nulls: "first" },
        { column: "id", nulls: "last" },
      ],
    },
  },
};

/** Postgres: the sets of columns no two rows agree on, a table's primary key first. A nullable column of a unique index is not one, since NULLs may repeat there. */
export const unique_columns: Example = {
  database: "postgres",
  setup: "create table accounts (id int8 primary key, email text not null unique, handle text unique)",
  statement: "select id, email, handle from accounts",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [
        { name: "id", type: int8 },
        { name: "email", type: pg_text },
        { name: "handle", nullable: true, type: pg_text },
      ],
      unique: [["id"], ["email"]],
    },
  },
};

/** Postgres: a column of each kind its catalog holds. The engine types a domain's column as the base type the server describes it by, `varchar(255)`, so `e`'s type does not carry the domain's name, `public.email`, and takes its LIKE CHECK as the I-Regexp `pattern` that matches the same values. An enum's labels are its `values`. Postgres lets `tags` hold arrays of any depth, and it is typed as the one-dimensional array a query reads. */
export const postgres_columns: Example = {
  database: "postgres",
  setup: "create type mood as enum ('sad', 'ok', 'happy'); create domain email as varchar(255) check (value like '%@%'); create table t (price numeric(10,2), total numeric, at timestamptz, day date, span interval, m mood, e email, tags varchar(20)[], flags bit(7), body jsonb, during tstzrange, pos point, ring circle, addr inet, tod time)",
  statement: "select * from t",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [
        { name: "price", nullable: true, type: { kind: "decimal", precision: 10, min_scale: 2, max_scale: 2, nan: true, label: "numeric" } },
        { name: "total", nullable: true, type: { kind: "decimal", min_scale: 0, max_scale: 16383, exclusive_min: "-1e131072", exclusive_max: "1e131072", infinity: true, nan: true, label: "numeric" } },
        { name: "at", nullable: true, type: timestamptz },
        { name: "day", nullable: true, type: { kind: "date", min: "-4713-11-24", max: "+5874897-12-31", infinity: true, label: "date" } },
        { name: "span", nullable: true, type: { kind: "duration", max: "P2147483648M2147483648DT2562047788H54.775808S", tick: "1/1000000", infinity: true, label: "interval" } },
        { name: "m", nullable: true, type: { kind: "enum", type: { kind: "text" }, values: ["sad", "ok", "happy"], label: "public.mood" } },
        { name: "e", nullable: true, type: { kind: "text", code_points: [[1, 1114111]], size: { max: 255 }, pattern: "(.|\\n|\\r)*@(.|\\n|\\r)*", label: "varchar" } },
        { name: "tags", nullable: true, type: { kind: "array", label: "_varchar", element: { nullable: true, type: { kind: "text", code_points: [[1, 1114111]], size: { max: 20 }, label: "varchar" } } } },
        { name: "flags", nullable: true, type: { kind: "array", size: { exact: 7 }, label: "bit", element: { type: { kind: "boolean" } } } },
        { name: "body", nullable: true, type: { kind: "json", label: "jsonb" } },
        { name: "during", nullable: true, type: { kind: "range", subtype: timestamptz, label: "tstzrange" } },
        { name: "pos", nullable: true, type: point },
        { name: "ring", nullable: true, type: { kind: "struct", label: "circle", fields: [{ name: "center", type: point }, { name: "radius", type: float8 }] } },
        {
          name: "addr",
          nullable: true,
          type: {
            kind: "union",
            label: "inet",
            members: [
              { kind: "struct", fields: [{ name: "address", type: { kind: "bytes", size: { exact: 4 } } }, { name: "prefix", type: { kind: "integer", bits: 8, unsigned: true, max: "32" } }] },
              { kind: "struct", fields: [{ name: "address", type: { kind: "bytes", size: { exact: 16 } } }, { name: "prefix", type: { kind: "integer", bits: 8, unsigned: true, max: "128" } }] },
            ],
          },
        },
        { name: "tod", nullable: true, type: { kind: "time", tick: "1/1000000", max: "24:00:00", label: "time" } },
      ],
    },
  },
};

/** Postgres: an extension's type, pgvector's `vector(3)`, under the schema the extension is installed in. */
export const pgvector_column: Example = {
  database: "postgres",
  setup: "create extension vector; create table items (emb vector(3))",
  statement: "select emb from items",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [{ name: "emb", nullable: true, type: { kind: "array", size: { exact: 3 }, label: "public.vector", element: { type: { kind: "float", exponent_bits: 8, mantissa_bits: 23 } } } }],
    },
  },
};

/** Postgres: the JSON a statement builds, typed inside, over a NOT NULL `title`. */
export const json_column: Example = {
  database: "postgres",
  setup: "create table users (id int8 primary key); create table posts (id int8 primary key, author_id int8 references users, title text not null)",
  statement: "select json_agg(p.title) filter (where p.id is not null) as titles from users u left join posts p on p.author_id = u.id group by u.id",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [{ name: "titles", nullable: true, type: { kind: "json", label: "json", content: { type: { kind: "array", element: { type: pg_text } } } } }],
    },
  },
};

/** Postgres: a constant the statement writes is an enum of its one value, over the type the server describes it by. A `case` whose every result is a constant is an enum of them all, and so is a column every arm of a `union` writes a constant to. */
export const literal_columns: Example = {
  database: "postgres",
  setup: "create table users (id int8 primary key, banned boolean not null)",
  statement: "select id, 'user' as kind, case when banned then 'banned' else 'active' end as status, 1 as version from users",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [
        { name: "id", type: int8 },
        { name: "kind", type: { kind: "enum", type: pg_text, values: ["user"] } },
        { name: "status", type: { kind: "enum", type: pg_text, values: ["banned", "active"] } },
        { name: "version", type: { kind: "enum", type: { kind: "integer", bits: 32, label: "int4" }, values: ["1"] } },
      ],
      unique: [["id"]],
    },
  },
};

/** Postgres: a UUID's version, where the database guarantees it. `id`'s CHECKs hold it to version 7 and its version non-null, and `gen_random_uuid()` makes version 4. `source` may hold any UUID: its `default gen_random_uuid()` does not stop an insert writing another. Every function it calls is in PostgreSQL 17, where `uuidv7()` is not. */
export const uuid_versions: Example = {
  database: "postgres",
  setup: "create table events (id uuid primary key check (uuid_extract_version(id) = 7 and uuid_extract_version(id) is not null), source uuid default gen_random_uuid())",
  statement: "select id, source, gen_random_uuid() as request from events",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [
        { name: "id", type: { kind: "uuid", versions: [7], label: "uuid" } },
        { name: "source", nullable: true, type: { kind: "uuid", label: "uuid" } },
        { name: "request", type: { kind: "uuid", versions: [4], label: "uuid" } },
      ],
      unique: [["id"]],
    },
  },
};

/** MySQL. `:n + 1` is a `BIGINT`, a `DOUBLE` or a `DECIMAL` by what is bound to `:n` and how the driver sends it, so its type is a union of the three. */
export const mysql_columns: Example = {
  database: "mysql",
  setup: "create table m (id bigint unsigned primary key, flag boolean not null, at timestamp(3) null, status enum('b','a') not null, born date, name varchar(100), body text, tags set('a','b'))",
  statement: "select id, flag, at, status, born, name, body, tags, :n + 1 next from m",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [
        { name: "id", type: { kind: "integer", bits: 64, unsigned: true, label: "bigint" } },
        { name: "flag", type: { kind: "integer", bits: 8, label: "tinyint(1)" } },
        { name: "at", nullable: true, type: { kind: "instant", tick: "1/1000", min: "1970-01-01T00:00:01Z", max: "2038-01-19T03:14:07.999Z", label: "timestamp" } },
        { name: "status", type: { kind: "enum", type: { kind: "text" }, values: ["b", "a"], label: "enum" } },
        { name: "born", nullable: true, type: { kind: "date", min: "0000-01-01", label: "date" } },
        { name: "name", nullable: true, type: { kind: "text", size: { max: 100 }, label: "varchar" } },
        { name: "body", nullable: true, type: { kind: "text", encoding: "UTF-8", size: { unit: "byte", max: 65535 }, label: "text" } },
        { name: "tags", nullable: true, type: { kind: "array", label: "set", distinct: true, element: { type: { kind: "enum", type: { kind: "text" }, values: ["a", "b"] } } } },
        { name: "next", nullable: true, type: { kind: "union", members: [{ ...int64, label: "bigint" }, { kind: "float", exponent_bits: 11, mantissa_bits: 52, label: "double" }, { kind: "decimal", label: "decimal" }] } },
      ],
    },
  },
};

/** SQL Server. `at` keeps the offset each value was written with, and `label` may hold a lone surrogate, counted in UTF-16 units. */
export const sqlserver_columns: Example = {
  database: "sqlserver",
  setup: "create table s (at datetimeoffset(3), label nvarchar(10))",
  statement: "select at, label from s",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [
        { name: "at", nullable: true, type: { kind: "zoned_datetime", tick: "1/1000", offset_only: true, label: "datetimeoffset" } },
        { name: "label", nullable: true, type: { kind: "text", encoding: "UTF-16", ill_formed: true, size: { unit: "code_unit", max: 10 }, label: "nvarchar" } },
      ],
    },
  },
};

/** Postgres: a range spans whatever the database orders, an array here. */
export const range_over_array: Example = {
  database: "postgres",
  setup: "create type arrayrange as range (subtype = int4[]); create table t (r arrayrange)",
  statement: "select r from t",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [{ name: "r", nullable: true, type: { kind: "range", label: "public.arrayrange", subtype: { kind: "array", label: "_int4", element: { nullable: true, type: { kind: "integer", bits: 32, label: "int4" } } } } }],
    },
  },
};

/**
 * SQLite. A NUMERIC column stores `'0012'` as 12, `'abc'` as text and a blob as a blob; a TEXT column stores 12 as `'12'`. A type is named as SQLite reports the column's declared type: one of its own words (`INT`, `INTEGER`, `REAL`, `TEXT`, `BLOB`) in capitals, any other as written. The engine leaves `x`, declared with no type, `unknown`, though it holds what a NUMERIC column does. In the STRICT table, `n` is an integer alone.
 */
export const sqlite_columns: Example = {
  database: "sqlite",
  setup: "create table s (id integer primary key, price numeric, name text, x); create table t (n integer) strict",
  statement: "select price, name, x, n from s, t",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [
        { name: "price", nullable: true, type: { kind: "union", label: "numeric", members: [int64, { kind: "float", exponent_bits: 11, mantissa_bits: 52, infinity: true }, sqlite_text, { kind: "bytes" }] } },
        { name: "name", nullable: true, type: { kind: "union", label: "TEXT", members: [sqlite_text, { kind: "bytes" }] } },
        { name: "x", nullable: true, type: { kind: "unknown" } },
        { name: "n", nullable: true, type: { ...int64, label: "INTEGER" } },
      ],
    },
  },
};

/** Oracle, written by hand. Oracle stores `''` as null, so its text is at least 1 code point. */
export const oracle_column: Example = {
  database: "oracle",
  setup: "create type shape_t as object (name varchar2(20 char)) not final; create type circle_t under shape_t (radius number); create table shapes (s shape_t)",
  statement: "select s from shapes",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "many",
      columns: [
        {
          name: "S",
          nullable: true,
          type: {
            kind: "union",
            label: "R.SHAPE_T",
            members: [
              { kind: "struct", label: "R.SHAPE_T", fields: [{ name: "NAME", nullable: true, type: { kind: "text", size: { min: 1, max: 20 }, label: "VARCHAR2" } }] },
              {
                kind: "struct",
                label: "R.CIRCLE_T",
                fields: [
                  { name: "NAME", nullable: true, type: { kind: "text", size: { min: 1, max: 20 }, label: "VARCHAR2" } },
                  { name: "RADIUS", nullable: true, type: { kind: "decimal", precision: 40, exclusive_min: "-1e126", exclusive_max: "1e126", label: "NUMBER" } },
                ],
              },
            ],
          },
        },
      ],
    },
  },
};

/** DuckDB: a tagged union whose every alternative may hold NULL, as DuckDB lets any value inside a nested type be. */
export const duckdb_column: Example = {
  database: "duckdb",
  statement: "select union_value(num := 1)::union(num integer, str varchar) as u",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "one",
      columns: [
        {
          name: "u",
          nullable: true,
          type: {
            kind: "tagged_union",
            label: "union",
            alternatives: [
              { tag: "num", nullable: true, type: { kind: "integer", bits: 32, label: "integer" } },
              { tag: "str", nullable: true, type: { kind: "text", label: "varchar" } },
            ],
          },
        },
      ],
    },
  },
};

/** DuckDB: a struct whose fields have no names, which DuckDB reads by their places, is a tuple. */
export const duckdb_tuple: Example = {
  database: "duckdb",
  statement: "select row(1, 'a') as pair",
  descriptor: {
    result: {
      kind: "table",
      cardinality: "one",
      columns: [
        {
          name: "pair",
          type: {
            kind: "tuple",
            elements: [
              { nullable: true, type: { kind: "integer", bits: 32, label: "integer" } },
              { nullable: true, type: { kind: "text", label: "varchar" } },
            ],
          },
        },
      ],
    },
  },
};

/** Postgres: a structured parameter, a list of records. `at` may be null, so the caller may leave it out. */
export const structured_param: Example = {
  database: "postgres",
  setup: "create table events (kind varchar(32) not null, at timestamptz)",
  statement: "insert into events (kind, at) select * from :events::rows(kind varchar(32) not null, at timestamptz)",
  descriptor: {
    params: [
      {
        name: "events",
        type: {
          kind: "array",
          element: {
            type: {
              kind: "struct",
              fields: [
                { name: "kind", type: { kind: "text", code_points: [[1, 1114111]], size: { max: 32 }, label: "varchar" } },
                { name: "at", nullable: true, optional: true, type: timestamptz },
              ],
            },
          },
        },
      },
    ],
  },
};
