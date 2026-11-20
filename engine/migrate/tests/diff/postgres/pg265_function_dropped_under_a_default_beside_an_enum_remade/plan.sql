alter table t alter column c drop default;
drop function f();
alter type e rename to e__old;
create type e as enum ('a', 'c', 'b');
alter table t alter column c type e using c::text::e;
drop type e__old;
alter table t alter column c drop default;
