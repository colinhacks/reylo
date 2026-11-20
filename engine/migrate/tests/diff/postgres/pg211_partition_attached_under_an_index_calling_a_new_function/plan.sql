-- file 1 of 3
set check_function_bodies = off;
alter table p add column b text;
create function f(int) returns int language sql immutable return $1 + 1;
alter table child add column b text;
reset check_function_bodies;
-- file 2 of 3
create index concurrently child_f_idx on child using btree (f(a));
-- file 3 of 3
set check_function_bodies = off;
alter table p attach partition child for values from (0) to (10);
create index pi on p using btree (f(a));
reset check_function_bodies;
