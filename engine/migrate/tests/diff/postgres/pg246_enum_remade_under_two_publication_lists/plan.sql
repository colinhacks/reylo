alter publication pub drop table only t;
alter publication pub drop table only h;
alter type e rename to e__old;
create type e as enum ('a', 'x', 'c');
alter table h alter column c type e using c::text::e;
alter table t alter column c type e using c::text::e;
drop type e__old;
alter publication pub set table only h (c);
alter publication pub set table only h (c);
