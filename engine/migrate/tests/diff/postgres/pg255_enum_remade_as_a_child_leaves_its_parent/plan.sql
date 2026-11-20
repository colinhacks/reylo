alter table h no inherit p;
alter table h drop column if exists id;
alter type e rename to e__old;
create type e as enum ('a', 'b');
alter table p alter column c type e using c::text::e;
alter table h alter column c type e using c::text::e;
drop type e__old;
