-- loses data: h
drop table h;
alter type e rename to e__old;
create type e as enum ('a', 'c');
alter table only k alter column c drop default;
alter table p alter column c drop default;
alter table p alter column c type e using c::text::e;
alter table p alter column c set default 'a'::e;
alter table only k alter column c set default 'a'::e;
drop type e__old;
