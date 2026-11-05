/** Examples of Reylo Type Defs, the type descriptor format (`index.ts`), database by database. `nub run typecheck` checks each against the types. */

import type { Column, FloatType, InstantType, IntegerType, Param, RecordType, StatementDescriptor, TextType } from "./index.ts";

const int8: IntegerType = { kind: "integer", bits: 64, name: "int8" };

/** A first example, on Postgres, over `create table users (id int8 primary key, email varchar(255) not null, bio text); comment on column users.bio is 'Shown on the profile page';`:
 *
 *   -- One user, by id.
 *   select id, email, bio from users where id = :id
 */
export const first_example: StatementDescriptor = {
  $schema: "https://reylo.dev/typedefs/0.1.0.json",
  dialect: { name: "postgres", version: "18.1" },
  sql: "select id, email, bio from users where id = $1",
  kind: "select",
  comment: "One user, by id.",
  params: [{ name: "id", type: int8 }],
  columns: [
    { name: "id", type: int8 },
    { name: "email", type: { kind: "text", size: { max: 255 }, name: "varchar" } },
    { name: "bio", nullable: true, type: { kind: "text", name: "text" }, comment: "Shown on the profile page" },
  ],
  cardinality: "at_most_one",
  diagnostics: [],
};

const int64: IntegerType = { kind: "integer", bits: 64 };
const float8: FloatType = { kind: "float", exponent_bits: 11, mantissa_bits: 52, infinity: true, nan: true, name: "float8" };
const timestamptz: InstantType = { kind: "instant", tick: "1/1000000", min: "-4713-11-24T00:00:00Z", max: "+294276-12-31T23:59:59.999999Z", infinity: true, name: "timestamptz" };
const point: RecordType = { kind: "record", name: "point", fields: [{ name: "x", type: float8 }, { name: "y", type: float8 }] };
const sqlite_text: TextType = { kind: "text", ill_formed: "utf8" };

/**
 * Postgres.
 *
 *   create type mood as enum ('sad', 'ok', 'happy');
 *   create domain email as varchar(255) check (value like '%@%');
 *   create table t (price numeric(10,2), total numeric, at timestamptz, day date, span interval, m mood, e email, tags varchar(20)[], flags bit(7), emb vector(3), body jsonb, during tstzrange, pos point, ring circle, addr inet, tod time);
 */
export const postgres_columns: Column[] = [
  { name: "price", nullable: true, type: { kind: "decimal", precision: 10, min_scale: 2, max_scale: 2, nan: true, name: "numeric" } },
  { name: "total", nullable: true, type: { kind: "decimal", min_scale: 0, max_scale: 16383, exclusive_min: "-1e131072", exclusive_max: "1e131072", infinity: true, nan: true, name: "numeric" } },
  { name: "at", nullable: true, type: timestamptz },
  { name: "day", nullable: true, type: { kind: "date", min: "-4713-11-24", max: "+5874897-12-31", infinity: true, name: "date" } },
  { name: "span", nullable: true, type: { kind: "duration", months: { max: "P2147483648M" }, days: { max: "P2147483648D" }, time: { tick: "1/1000000", max: "PT2562047788H54.775808S" }, infinity: true, name: "interval" } },
  { name: "m", nullable: true, type: { kind: "text", enum: ["sad", "ok", "happy"], name: "public.mood" } },
  { name: "e", nullable: true, type: { kind: "text", size: { max: 255 }, pattern: { dialect: "like", source: "%@%" }, name: "public.email" } },
  { name: "tags", nullable: true, type: { kind: "array", any_depth: true, name: "_varchar", element: { nullable: true, type: { kind: "text", size: { max: 20 }, name: "varchar" } } } },
  { name: "flags", nullable: true, type: { kind: "array", size: { exact: 7 }, name: "bit", element: { type: { kind: "boolean" } } } },
  { name: "emb", nullable: true, type: { kind: "array", size: { exact: 3 }, name: "public.vector", element: { type: { kind: "float", exponent_bits: 8, mantissa_bits: 23 } } } },
  { name: "body", nullable: true, type: { kind: "json", name: "jsonb" } },
  { name: "during", nullable: true, type: { kind: "range", subtype: timestamptz, name: "tstzrange" } },
  { name: "pos", nullable: true, type: point },
  { name: "ring", nullable: true, type: { kind: "record", name: "circle", fields: [{ name: "center", type: point }, { name: "radius", type: float8 }] } },
  {
    name: "addr", nullable: true,
    type: { kind: "union", name: "inet", members: [
      { kind: "record", fields: [{ name: "address", type: { kind: "bytes", size: { exact: 4 } } }, { name: "prefix", type: { kind: "integer", bits: 8, unsigned: true, max: "32" } }] },
      { kind: "record", fields: [{ name: "address", type: { kind: "bytes", size: { exact: 16 } } }, { name: "prefix", type: { kind: "integer", bits: 8, unsigned: true, max: "128" } }] },
    ] },
  },
  { name: "tod", nullable: true, type: { kind: "time", tick: "1/1000000", max: "24:00:00", name: "time" } },
];

/** Postgres: `select json_agg(p.title) filter (where p.id is not null) as titles from users u left join posts p on p.author_id = u.id group by u.id`, over a NOT NULL `title`. */
export const json_column: Column = {
  name: "titles",
  nullable: true,
  type: { kind: "json", name: "json", content: { type: { kind: "array", element: { type: { kind: "text", name: "text" } } } } },
};

/**
 * MySQL.
 *
 *   create table m (id bigint unsigned primary key, flag boolean not null, at timestamp(3) null, status enum('b','a') not null, born date, name varchar(100), body text, tags set('a','b'));
 *   select id, flag, at, status, born, name, body, tags, :n + 1 next from m
 */
export const mysql_columns: Column[] = [
  { name: "id", type: { kind: "integer", bits: 64, unsigned: true, name: "bigint" } },
  { name: "flag", type: { kind: "integer", bits: 8, name: "tinyint(1)" } },
  { name: "at", nullable: true, type: { kind: "instant", tick: "1/1000", min: "1970-01-01T00:00:01Z", max: "2038-01-19T03:14:07.999Z", name: "timestamp" } },
  { name: "status", type: { kind: "text", enum: ["b", "a"], name: "enum" } },
  { name: "born", nullable: true, type: { kind: "date", name: "date" } },
  { name: "name", nullable: true, type: { kind: "text", size: { max: 100 }, name: "varchar" } },
  { name: "body", nullable: true, type: { kind: "text", size: { unit: "byte", encoding: "UTF-8", max: 65535 }, name: "text" } },
  { name: "tags", nullable: true, type: { kind: "set", size: { max: 2 }, name: "set", element: { type: { kind: "text", enum: ["a", "b"] } } } },
  { name: "next", nullable: true, type: { kind: "union", members: [int64, { kind: "float", exponent_bits: 11, mantissa_bits: 52 }, { kind: "decimal" }] } },
];

/** SQL Server: `create table s (at datetimeoffset(3), label nvarchar(10))`. `at` keeps the offset each value was written with, and `label` may hold a lone surrogate. */
export const sqlserver_columns: Column[] = [
  { name: "at", nullable: true, type: { kind: "instant", tick: "1/1000", zone: "offset", name: "datetimeoffset" } },
  { name: "label", nullable: true, type: { kind: "text", ill_formed: "utf16", size: { max: 10 }, name: "nvarchar" } },
];

/**
 * SQLite.
 *
 *   create table s (id integer primary key, price numeric, name text, x);
 *   create table t (n integer) strict;
 *   select price, name, x, n from s, t
 *
 * A NUMERIC column stores `'0012'` as 12, `'abc'` as text and a blob as a blob; a TEXT column stores 12 as `'12'`. In the STRICT table, `n` is an integer alone.
 */
export const sqlite_columns: Column[] = [
  { name: "price", nullable: true, type: { kind: "union", name: "NUMERIC", members: [int64, { kind: "float", exponent_bits: 11, mantissa_bits: 52, infinity: true }, sqlite_text, { kind: "bytes" }] } },
  { name: "name", nullable: true, type: { kind: "union", name: "TEXT", members: [sqlite_text, { kind: "bytes" }] } },
  { name: "x", nullable: true, type: { kind: "union", members: [int64, { kind: "float", exponent_bits: 11, mantissa_bits: 52, infinity: true }, sqlite_text, { kind: "bytes" }] } },
  { name: "n", nullable: true, type: { ...int64, name: "INTEGER" } },
];

/** Oracle: `create type shape_t as object (name varchar2(20 char)) not final; create type circle_t under shape_t (radius number); create table shapes (s shape_t);` Oracle stores `''` as null, so its text is at least 1 code point. */
export const oracle_column: Column = {
  name: "S",
  nullable: true,
  type: { kind: "union", name: "R.SHAPE_T", members: [
    { kind: "record", name: "R.SHAPE_T", fields: [{ name: "NAME", nullable: true, type: { kind: "text", size: { min: 1, max: 20 }, name: "VARCHAR2" } }] },
    { kind: "record", name: "R.CIRCLE_T", fields: [
      { name: "NAME", nullable: true, type: { kind: "text", size: { min: 1, max: 20 }, name: "VARCHAR2" } },
      { name: "RADIUS", nullable: true, type: { kind: "decimal", precision: 40, exclusive_min: "-1e126", exclusive_max: "1e126", name: "NUMBER" } },
    ] },
  ] },
};

/** DuckDB: `select union_value(num := 1)::union(num integer, str varchar) as u`, a tagged union whose every alternative may hold NULL, as DuckDB lets any value inside a nested type be. */
export const duckdb_column: Column = {
  name: "u",
  nullable: true,
  type: { kind: "tagged_union", name: "union", alternatives: [
    { tag: "num", nullable: true, type: { kind: "integer", bits: 32, name: "integer" } },
    { tag: "str", nullable: true, type: { kind: "text", name: "varchar" } },
  ] },
};

/** A structured parameter: `insert into events (kind, at) select * from :events::rows(kind varchar(32), at timestamptz?)` */
export const structured_param: Param = {
  name: "events",
  type: { kind: "array", element: { type: { kind: "record", fields: [
    { name: "kind", type: { kind: "text", size: { max: 32 }, name: "varchar" } },
    { name: "at", nullable: true, optional: true, type: timestamptz },
  ] } } },
};
