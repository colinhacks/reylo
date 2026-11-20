set check_function_bodies = off;
alter table t alter column c1 drop default;
drop function f(integer,integer);
create function f(s integer, k text = 'x') returns integer language sql as $$ select 1 $$;
alter table t alter column c1 set default f(1);
reset check_function_bodies;
