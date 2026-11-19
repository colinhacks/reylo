drop function show(d);
drop view v;
alter domain d rename to d__old;
create domain d as text not null default '1';
comment on domain d is 'a code';
alter table t alter column c type d using c::text::d;
drop domain d__old;
create view v as select id, c from t;
create function show(x d) returns text language sql immutable as 'select x::text';
