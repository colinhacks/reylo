alter domain d rename to d__old;
create domain d as bigint;
alter table t alter column c type text using c::text;
alter type p alter attribute a type d;
alter table t alter column c type p using c::text::p;
drop domain d__old;
