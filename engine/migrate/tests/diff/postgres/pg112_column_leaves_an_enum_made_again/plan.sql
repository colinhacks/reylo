alter table t alter column v type f using v::text::f;
alter type e rename to e__old;
create type e as enum ('a');
alter table t alter column w type e using w::text::e;
drop type e__old;
