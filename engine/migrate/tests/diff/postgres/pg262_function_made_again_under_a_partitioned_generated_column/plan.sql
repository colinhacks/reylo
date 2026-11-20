set check_function_bodies = off;
alter table t drop column g;
drop function f(integer);
create function f(a int) returns text language sql immutable return '7';
alter table t add column g integer generated always as (f(id)::int) stored;
reset check_function_bodies;
