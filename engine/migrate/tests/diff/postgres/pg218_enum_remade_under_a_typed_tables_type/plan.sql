alter type e rename to e__old;
create type e as enum ('a', 'b', 'c');
alter table t not of;
alter type ty alter attribute c type e;
alter table t alter column c type e using c::text::e;
alter table t of ty;
drop type e__old;
