alter table t not of;
drop type ty;
alter type e rename to e__old;
create type e as enum ('a', 'c');
alter domain d rename to d__old;
create domain d as e;
alter table t alter column n type d using n::text::d;
drop domain d__old;
drop type e__old;
