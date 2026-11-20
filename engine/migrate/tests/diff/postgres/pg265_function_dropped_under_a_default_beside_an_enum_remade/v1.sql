create type e as enum ('a', 'b', 'c');
create function f() returns e language sql immutable as $$ select 'a'::e $$;
create table t (id int, c e default f());
