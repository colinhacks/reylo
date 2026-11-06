-- Fixture for the pg-forms lab: run once against the DATABASE_URL in .env.
-- psql "$DATABASE_URL" -f setup.sql
drop function if exists pgf_greet(text);
drop function if exists pgf_greet_strict(text);
drop function if exists pgf_user_posts(int);
drop function if exists pgf_stats();
drop function if exists pgf_double(int);
drop view if exists pgf_user_stats;
drop view if exists pgf_members;
drop table if exists pgf_things;
drop table if exists pgf_posts;
drop table if exists pgf_users;
drop type if exists pgf_role;
drop domain if exists pgf_slug;
drop type if exists pgf_pair;
create type pgf_role as enum ('admin', 'member');
create domain pgf_slug as text check (value ~ '^[a-z-]+$');
create type pgf_pair as (a int, b text);
create table pgf_users (
  id serial primary key,
  uid uuid not null default gen_random_uuid(),
  name text not null,
  email text,
  slug pgf_slug not null,
  role pgf_role not null default 'member',
  tags text[] not null default '{}',
  meta jsonb,
  score numeric,
  big int8 not null default 0,
  blob bytea,
  created_at timestamptz not null default now()
);
create table pgf_posts (
  id serial primary key,
  user_id int not null references pgf_users(id),
  title text not null,
  body text,
  published_at timestamptz,
  views int not null default 0
);
insert into pgf_users (name, email, slug, role, tags, meta, score, big, blob) values
  ('ada', 'ada@example.com', 'ada', 'admin', '{a,b}', '{"k": 1}', 1.5, 9007199254740993, '\x01'),
  ('bob', null, 'bob', 'member', '{}', null, null, 0, null),
  ('cyd', 'cyd@example.com', 'cyd', 'member', '{c}', '{"k": 2}', 2, 1, null);
insert into pgf_posts (user_id, title, body, published_at, views) values
  (1, 'hello', null, now(), 3),
  (1, 'second', 'text', null, 0),
  (2, 'third', 'more', now(), 1);
-- A SQL function returning a scalar.
create function pgf_greet(n text) returns text language sql as $$ select 'hi ' || n $$;
-- The same, STRICT: NULL in, NULL out.
create function pgf_greet_strict(n text) returns text language sql strict as $$ select 'hi ' || n $$;
-- A SQL function returning SETOF a table.
create function pgf_user_posts(uid int) returns setof pgf_posts language sql as $$ select * from pgf_posts where user_id = uid $$;
-- A SQL function returning TABLE(...).
create function pgf_stats() returns table(user_id int, n bigint) language sql as $$ select user_id, count(*) from pgf_posts group by user_id $$;
-- A PL/pgSQL function.
create function pgf_double(x int) returns int language plpgsql as $$ begin return x * 2; end $$;
-- A view over one table, and one over an outer join with an aggregate. The catalog carries no
-- NOT NULL for a view's columns; it comes from what the view selects.
create view pgf_members as select id, name, email from pgf_users where role = 'member';
create view pgf_user_stats as
  select u.id, u.name, count(p.id) as posts, max(p.published_at) as last_at
  from pgf_users u left join pgf_posts p on p.user_id = u.id group by u.id;
-- Types no driver parses, which Reylo reads from the text Postgres prints.
create table pgf_things (
  id serial primary key,
  span int4range not null,
  during tstzrange,
  addr inet not null,
  price money,
  bits bit(3),
  doc xml,
  search tsvector,
  pair pgf_pair,
  roles pgf_role[] not null default '{}'
);
insert into pgf_things (span, during, addr, price, bits, doc, search, pair, roles) values
  ('[1,5)', null, '10.0.0.1', 12.5, B'101', '<a/>', to_tsvector('simple', 'hello world'), row(1, 'x'), '{admin}');
