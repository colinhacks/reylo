drop index i;
alter type e rename to e__old;
create type e as enum ('a', 'b', 'c');
alter table p alter column c type e using c::text::e;
create index i on p (c) where c is not null;
drop type e__old;
