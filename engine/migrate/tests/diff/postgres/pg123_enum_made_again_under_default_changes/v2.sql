create type e as enum ('b', 'a');
create table t (id integer primary key, v e);
create type g as enum ('b', 'a');
create function f() returns text language sql immutable as $$ select 'a'::text $$;
create table u (id integer primary key, v g default f()::g);
