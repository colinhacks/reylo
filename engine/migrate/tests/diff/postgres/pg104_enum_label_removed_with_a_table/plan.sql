-- loses data: created_with_e
drop table created_with_e;
alter type e rename to e__old;
create type e as enum ('a', 'b', 'c');
alter table t alter column category type e using category::text::e;
drop type e__old;
