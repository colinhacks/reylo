-- A parameter written whole and bare to a column takes NULL where the column does, an element of a
-- row value `SET (a, b) = (:a, :b)` included; into a NOT NULL column it is a value. Under a cast
-- it is the cast's type and takes no NULL; until 2026-10-21 it took NULL
-- where the bare one did.
create table users (id int primary key, name text not null, email text unique, score numeric, n int, doc jsonb, meta json not null default '{}');
insert into users (id, name, email) values (1, 'Ada', 'ada@example.com');
-- A table named with its schema is the same table: `public.users` as `users`, and a table outside
-- the search path by its schema, whose CHECKs and domains refuse null as a visible table's do.
create schema other;
create domain other.required_text as text not null;
create table other.notes (id int primary key, body text, title text check (title is not null), slug other.required_text, users_name text);
create table other.users (id int primary key, name text);
