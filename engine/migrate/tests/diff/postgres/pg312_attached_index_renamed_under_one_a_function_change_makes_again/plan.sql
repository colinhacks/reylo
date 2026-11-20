-- file 1 of 3
set check_function_bodies = off;
drop index public.pi;
drop function f(integer);
create function f(i int) returns bigint language sql immutable as 'select i::bigint';
reset check_function_bodies;
-- file 2 of 3
create index concurrently new_child on h (f(id));
-- file 3 of 3
set check_function_bodies = off;
create index pi on p using btree (f(id));
reset check_function_bodies;
