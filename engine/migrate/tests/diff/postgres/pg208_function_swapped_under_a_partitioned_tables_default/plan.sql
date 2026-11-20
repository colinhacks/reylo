set check_function_bodies = off;
alter table only t alter column n drop default;
alter table t_p0 alter column n drop default;
drop function f(bigint);
create function f(a bigint, b numeric) returns text language sql immutable return '7';
alter table only t alter column n set default f(7, 7.5)::numeric;
alter table t_p0 alter column n set default (f((7)::bigint, 7.5))::numeric;
reset check_function_bodies;
