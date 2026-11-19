-- file 1 of 2
set check_function_bodies = off;
drop function gone(mood);
drop view v;
alter table t alter column m drop default;
drop function pick();
alter table t alter column s drop default;
drop function score(mood);
alter table t drop constraint t_s_check;
alter type mood rename to mood__old;
create type mood as enum ('a');
alter table t alter column m type mood using m::text::mood;
alter table t add constraint t_s_check check (s > 0) not valid;
drop type mood__old;
create function pick() returns mood language sql immutable as $$select 'a'::mood$$;
alter table t alter column m set default pick();
create function score(m mood) returns integer language sql immutable as 'select 2';
alter table t alter column s set default score('a'::mood);
create view v as select id, pick() as p from t;
revoke execute on function score(mood) from public;
comment on function score(mood) is 'scores a mood';
reset check_function_bodies;
-- file 2 of 2
alter table t validate constraint t_s_check;
