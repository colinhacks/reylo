create function f(s integer, k text = 'x') returns integer language sql as $$ select 1 $$;
create table t (c1 integer default f(1));
