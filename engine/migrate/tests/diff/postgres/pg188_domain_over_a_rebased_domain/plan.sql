alter domain d rename to d__old;
create domain d as bigint;
alter domain child rename to child__old;
create domain child as d default 1;
alter table t alter column c type child using c::text::child;
alter table t alter column e type d using e::text::d;
drop domain child__old;
drop domain d__old;
