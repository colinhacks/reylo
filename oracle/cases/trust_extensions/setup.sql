-- Extensions define operators and functions of their own under the names the inference rules read
-- (citext `=` and `max`, hstore `?` and `||`, cube and ltree `@>`, isn `=` and `<`, seg `<<`): a
-- call over built-in types still resolves to the built-in, one over an extension's type to the
-- extension's, which proves nothing.
create extension citext;
create extension hstore;
create extension cube;
create extension ltree;
create extension isn;
create extension seg;
create table users (
  id bigint primary key,
  email text not null,
  name text,
  age int not null,
  score numeric not null,
  code varchar(10) not null,
  handle citext,
  prefs hstore,
  box cube,
  path ltree,
  isbn isbn13,
  span seg
);
-- A CHECK the seeded values break: its own table, with rows of its own.
create table accounts (id int primary key, status text not null check (status in ('active', 'banned')));
insert into accounts values (1, 'active'), (2, 'banned');
create table posts (id bigint primary key, author_id bigint, title text not null);
