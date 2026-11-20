create type e as enum ('a', 'x', 'c');
create domain d as e;
create function f(a d) returns numeric language sql immutable as $$ select 7.5 $$;
