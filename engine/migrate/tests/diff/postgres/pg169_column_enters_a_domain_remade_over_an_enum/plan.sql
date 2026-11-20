alter type mood rename to mood__old;
create type mood as enum ('a');
alter domain d rename to d__old;
create domain d as mood;
alter table t alter column c type mood using c::text::mood;
drop domain d__old;
drop type mood__old;
alter table t alter column c type d using c::text::d;
