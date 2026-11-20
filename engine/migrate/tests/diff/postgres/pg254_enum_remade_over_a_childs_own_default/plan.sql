alter type e rename to e__old;
create type e as enum ('a', 'b', 'c');
alter table only h alter column c drop default;
alter table p alter column c type e using c::text::e;
alter table only h alter column c set default 'a'::e;
drop type e__old;
